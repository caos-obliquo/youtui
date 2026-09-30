//! YouTube description / chapters timestamp parser for album-split fallback.
//!
//! When metadata providers return 0 tracks for a full-album YouTube upload,
//! this parser reads the yt-dlp JSON `description` field (and optionally
//! `chapters`) and produces a `Vec<AlbumTrack>` that drives `insert_album_tracks`.
//! See study report for the YVdaCDJ1s-E case (3847s, 10 tracks from description).

use crate::app::server::AlbumTrack;
use crate::app::server::yt_dlp_target_arg;
use serde_json;
use std::time::Duration;
use tracing::{debug, info};

/// Fetch yt-dlp JSON for `video_id` and parse chapters/description into AlbumTracks.
///
/// `cookie_path`/`cookie_browser` mirror the probe (`FetchYtVideoMetadata`):
/// age-restricted uploads fail with "Sign in to confirm your age" without
/// cookies, so pass `--cookies-from-browser` when cookie support is configured.
pub async fn fetch_yt_dlp_album_tracks(
    video_id: &str,
    yt_dlp_command: &str,
    cookie_path: Option<&str>,
    cookie_browser: &str,
) -> Vec<AlbumTrack> {
    match fetch_yt_dlp_json(video_id, yt_dlp_command, cookie_path, cookie_browser).await {
        Some(json) => album_tracks_from_json(&json, video_id),
        None => Vec::new(),
    }
}

/// Spawn yt-dlp `--dump-json` for `video_id` and parse stdout into JSON.
/// Returns None on spawn failure, 60s timeout, non-zero exit, or parse error
/// (each logged at info). Cookie args as in `fetch_yt_dlp_album_tracks`.
///
/// Retries up to 5 times with exponential backoff (3s, 6s, 12s, 24s, 48s) on
/// failure. Bandcamp returns HTTP 429 for concurrent metadata probes; the
/// backoff lets the rate limit window reset before the next attempt.
pub async fn fetch_yt_dlp_json(
    video_id: &str,
    yt_dlp_command: &str,
    cookie_path: Option<&str>,
    cookie_browser: &str,
) -> Option<serde_json::Value> {
    let use_cookie = cookie_path.is_some() && !cookie_browser.is_empty();
    let mut args: Vec<String> = vec![
        "--dump-json".into(),
        "--no-warnings".into(),
        yt_dlp_target_arg(video_id),
    ];
    if use_cookie {
        args.push("--cookies-from-browser".into());
        args.push(cookie_browser.to_string());
        info!("yt-dlp fallback: using --cookies-from-browser {} for video {}", cookie_browser, video_id);
    }
    const MAX_RETRIES: u32 = 5;
    const BASE_DELAY_SECS: u64 = 3;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = BASE_DELAY_SECS * 2_u64.pow(attempt - 1);
            info!("yt-dlp fallback: retry {}/{} for video {} after {}s", attempt, MAX_RETRIES, video_id, delay);
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }
        let output = match tokio::time::timeout(
            Duration::from_secs(60),
            tokio::process::Command::new(yt_dlp_command)
                .args(&args)
                .kill_on_drop(true)
                .output(),
        )
        .await
        {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                info!("yt-dlp fallback: failed to spawn yt-dlp for video {}: {}", video_id, e);
                continue;
            }
            Err(_) => {
                info!("yt-dlp fallback: timed out after 60s for video {}", video_id);
                continue;
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("yt-dlp fallback: yt-dlp failed for video {}: {}", video_id, stderr.trim());
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        return match serde_json::from_str(&stdout) {
            Ok(v) => Some(v),
            Err(e) => {
                info!("yt-dlp fallback: JSON parse failed for video {}: {}", video_id, e);
                None
            }
        };
    }
    info!("yt-dlp fallback: exhausted {} retries for video {}", MAX_RETRIES, video_id);
    None
}

/// Pick an album tracklist from yt-dlp JSON: uploader-authored description
/// first, chapters last. YouTube auto-generated chapters can be garbage:
/// Vomitoma CDsJBLrT_UM ships 25 chapters whose `start_time`s are the
/// tracklist DURATION values sorted ascending (23 after dedup, wrong track
/// order, final pseudo-track 32:25) while the description lists all 27
/// tracks in correct order with correct per-track durations.
pub fn album_tracks_from_json(json: &serde_json::Value, video_id: &str) -> Vec<AlbumTrack> {
    let duration_secs: u64 = json
        .get("duration")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
        if let Some(tracks) = parse_description_timestamps(desc, duration_secs) {
            if !tracks.is_empty() {
                info!("yt-dlp fallback: parsed {} tracks from description for video {}", tracks.len(), video_id);
                return tracks;
            }
        }
        // Some channel uploads list per-track durations at line end ("01. Untitled 00:59").
        if let Some(tracks) = parse_description_durations(desc, duration_secs) {
            if !tracks.is_empty() {
                info!("yt-dlp fallback: parsed {} tracks from description durations for video {}", tracks.len(), video_id);
                return tracks;
            }
        }
    }

    if let Some(chapters_arr) = json.get("chapters").and_then(|c| c.as_array()) {
        let chapters: Vec<(f64, &str)> = chapters_arr
            .iter()
            .filter_map(|c| {
                let start = c.get("start_time").and_then(|v| v.as_f64())?;
                let title = c.get("title").and_then(|v| v.as_str())?;
                Some((start, title))
            })
            .collect();
        if chapters.len() >= 2 {
            if let Some(tracks) = parse_chapters_timestamps(&chapters, duration_secs) {
                if !tracks.is_empty() {
                    info!("yt-dlp fallback: parsed {} tracks from chapters for video {}", tracks.len(), video_id);
                    return tracks;
                }
            }
        }
    }

    Vec::new()
}

/// Album-release year for a full-album upload: `release_year` if yt-dlp
/// parsed one, else the first 1900-2099 token in the description (e.g.
/// "'' Nuclear Cesspool Of Parasitic Scum '' 2009"). Deliberately ignores
/// `upload_date`: that is when the video was posted, not when the album
/// dropped (CDsJBLrT_UM: upload 2014-10-05, album 2009), and the upload
/// year leaked into the queue as the track year.
pub fn year_from_dlp_json(json: &serde_json::Value) -> Option<String> {
    if let Some(y) = json.get("release_year").and_then(|v| v.as_i64()).filter(|y| (1900..=2099).contains(y)) {
        return Some(y.to_string());
    }
    let desc = json.get("description").and_then(|v| v.as_str())?;
    extract_year_from_description(desc)
}

/// First standalone 1900-2099 digit run in `description`.
/// Splits on non-digits so timestamps ("00:59", "02;26") and track numbers
/// ("01.") never yield a false year (all 2-digit tokens).
/// Requires word boundaries (non-alphanumeric before/after) to avoid false
/// positives like "2000 copies" or "2400bps".
pub fn extract_year_from_description(description: &str) -> Option<String> {
    let bytes = description.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let run = &description[start..i];
        if run.len() != 4 {
            continue;
        }
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = i >= bytes.len() || !bytes[i].is_ascii_alphanumeric();
        if !before_ok || !after_ok {
            continue;
        }
        if i < bytes.len() && bytes[i] == b' ' && bytes.get(i + 1).is_some_and(|&b| b.is_ascii_lowercase()) {
            continue;
        }
        if let Ok(y) = run.parse::<u32>() {
            if (1900..=2099).contains(&y) {
                return Some(y.to_string());
            }
        }
    }
    None
}

/// True when `title` is a placeholder rather than a real track name:
/// empty, "Untitled", "NN. Untitled", "Track N", bare numbers, or an
/// auto-chapter marker ("<Untitled Chapter 1>").
pub fn is_placeholder_track_title(title: &str) -> bool {
    let t = title.trim().to_lowercase();
    t.is_empty()
        || t == "untitled"
        || t.ends_with(" untitled")
        || t.contains(".untitled")
        || t.contains("untitled chapter")
        || t.starts_with("track ")
        || (t.len() >= 2 && t.chars().all(|c| c.is_ascii_digit()))
}

/// Duration-alignment score for one (dlp, provider) pair (integer, never 0
/// so ties stay meaningful): both known and <= 2s apart = +20, <= 10s = +10,
/// either duration unknown = +5, farther apart = -20.
fn pair_score(dlp_dur: f64, prov_dur: f64) -> i32 {
    if dlp_dur <= 0.0 || prov_dur <= 0.0 {
        return 5;
    }
    let diff = (dlp_dur - prov_dur).abs();
    if diff <= 2.0 {
        20
    } else if diff <= 10.0 {
        10
    } else {
        -20
    }
}

/// Rename placeholder ("Untitled") yt-dlp track titles from a provider
/// tracklist by aligning the two lists on per-track durations
/// (Needleman-Wunsch style DP: match score above, skip provider = -1,
/// skip dlp = -10; traceback prefers skipping provider on ties so every
/// match lands on the earliest reachable column).
///
/// Rationale: channel uploads list "01. Untitled 00:59" while a provider
/// (e.g. Last.fm) has the real names, often as a longer tracklist merged
/// from several releases. Only placeholder dlp titles are overwritten, so
/// an uploader-authored real tracklist is never clobbered. A wrong-album
/// provider scores poorly (mismatch -20 < drop -10) and yields few renames.
/// Returns the number of titles renamed. Pure, no I/O.
pub fn fill_untitled_titles(dlp: &mut [AlbumTrack], provider: &[AlbumTrack]) -> usize {
    if provider.is_empty() || dlp.is_empty() {
        return 0;
    }
    if !dlp.iter().any(|t| is_placeholder_track_title(&t.title)) {
        debug!("fill_untitled: no placeholder titles in {} dlp tracks, skipping", dlp.len());
        return 0;
    }
    if !provider.iter().any(|t| !is_placeholder_track_title(&t.title)) {
        debug!("fill_untitled: all {} provider titles are placeholders, skipping", provider.len());
        return 0;
    }
    let n = dlp.len();
    let m = provider.len();
    debug!("fill_untitled: aligning {} dlp tracks against {} provider tracks", n, m);
    // score[i][j] = best alignment of dlp[..i] against provider[..j].
    let mut score = vec![vec![0i32; m + 1]; n + 1];
    for i in 1..=n {
        score[i][0] = score[i - 1][0] - 10;
    }
    for j in 1..=m {
        score[0][j] = score[0][j - 1] - 1;
    }
    for i in 1..=n {
        for j in 1..=m {
            let matched = score[i - 1][j - 1] + pair_score(dlp[i - 1].duration_secs, provider[j - 1].duration_secs);
            let skip_prov = score[i][j - 1] - 1;
            let skip_dlp = score[i - 1][j] - 10;
            score[i][j] = if matched >= skip_prov && matched >= skip_dlp {
                matched
            } else if skip_prov >= skip_dlp {
                skip_prov
            } else {
                skip_dlp
            };
        }
    }
    // Traceback. On ties prefer skip-provider over match: the score table
    // allows the junk block to sit anywhere (all skips + junk matches score
    // identically), and skipping first pins each dlp track to the EARLIEST
    // provider column it can reach - which is the correct one when the
    // provider list has a junk block before the real tail (Vomitoma case:
    // match-first put dlp[7] on LFM[50] instead of LFM[7]).
    let mut renamed = 0usize;
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let matched = score[i - 1][j - 1] + pair_score(dlp[i - 1].duration_secs, provider[j - 1].duration_secs);
        let skip_prov = score[i][j - 1] - 1;
        if score[i][j] == skip_prov {
            j -= 1;
        } else if score[i][j] == matched {
            if is_placeholder_track_title(&dlp[i - 1].title)
                && !is_placeholder_track_title(&provider[j - 1].title)
            {
                debug!(
                    "fill_untitled: dlp[{}] '{}' <- provider[{}] '{}'",
                    i - 1,
                    dlp[i - 1].title,
                    j - 1,
                    provider[j - 1].title
                );
                dlp[i - 1].title = provider[j - 1].title.clone();
                renamed += 1;
            }
            i -= 1;
            j -= 1;
        } else {
            debug_assert!(score[i][j] == score[i - 1][j] - 10, "traceback: impossible state at ({}, {})", i, j);
            i -= 1;
        }
    }
    info!("fill_untitled: renamed {} of {} placeholder titles (score {})", renamed, n, score[n][m]);
    renamed
}

/// Parse an ordered slice of yt-dlp chapters into AlbumTracks.
/// Each chapter is `(start_seconds, title)`. Returns None if < 2 chapters.
pub fn parse_chapters_timestamps(
    chapters: &[(f64, &str)],
    total_duration_secs: u64,
) -> Option<Vec<AlbumTrack>> {
    if chapters.len() < 2 {
        return None;
    }
    let total = total_duration_secs as f64;
    let mut parsed = Vec::with_capacity(chapters.len());
    let mut last_start: u64 = u64::MAX;
    let valid: Vec<(u64, String)> = chapters
        .iter()
        .filter_map(|(start_secs, title)| {
            let start = start_secs.round() as u64;
            if last_start != u64::MAX && start <= last_start {
                debug!("parse_chapters: skipping non-increasing start {start}");
                return None;
            }
            last_start = start;
            let t = title.trim().to_string();
            if t.len() < 1 {
                return None;
            }
            Some((start, t))
        })
        .collect();
    if valid.len() < 2 {
        return None;
    }
    for i in 0..valid.len() {
        let cur = valid[i].0 as f64;
        let next = if i + 1 < valid.len() {
            valid[i + 1].0 as f64
        } else {
            total
        };
        let dur = next - cur;
        let dur = if dur <= 0.0 {
            if total > 0.0 { total / valid.len() as f64 } else { 180.0 }
        } else if dur > total * 1.5 {
            debug!("parse_chapters: outlier at {i}, using avg");
            if total > 0.0 { total / valid.len() as f64 } else { 180.0 }
        } else {
            dur
        };
        parsed.push(AlbumTrack {
            title: valid[i].1.clone(),
            duration_secs: dur,
            artist: None,
        });
    }
    debug!("parse_chapters: parsed {} tracks", parsed.len());
    Some(parsed)
}

/// Try to extract a timestamp `(seconds, remainder_after_timestamp)` from the
/// start of `line`. Accepts `MM:SS` and `HH:MM:SS`, with optional surrounding
/// brackets, parentheses, or a leading numbering prefix before the timestamp.
fn try_extract_timestamp(line: &str) -> Option<(u64, &str)> {
    let line = line.trim_start();
    if line.is_empty() {
        return None;
    }
    // Find the colon that's part of the timestamp (preceded by digits).
    let colon_pos = {
        let mut found = None;
        for (i, _) in line.match_indices(':') {
            let before = &line[..i];
            if before.bytes().rev().take_while(|b| b.is_ascii_digit()).next().is_some() {
                found = Some(i);
                break;
            }
        }
        found?
    };
    if colon_pos == 0 {
        return None;
    }
    // Minutes are the rightmost digit-run immediately before the colon.
    // Handles "00:00", "[00:00]", "1. 00:00", "Track 1 - 00:00" etc.
    let before = &line[..colon_pos];
    let digits_start = before
        .bytes()
        .rev()
        .position(|b| !b.is_ascii_digit())
        .map(|p| before.len() - p)
        .unwrap_or(0);
    let minutes_str = &before[digits_start..];
    if minutes_str.is_empty() {
        return None;
    }
    let minutes: u64 = minutes_str.parse().ok()?;
    let after = &line[colon_pos + 1..];
    // HH:MM:SS case: look for a second colon in `after`.
    let second_colon = after.find(':');
    let (total, title_start) = match second_colon {
        Some(sc) => {
            let mins_str = &after[..sc];
            let hours: u64 = minutes_str.parse().ok()?;
            let mins: u64 = mins_str.parse().ok()?;
            let total = hours * 3600 + mins * 60;
            (total, &after[sc + 1..])
        }
        None => {
            // MM:SS case: consume trailing digits as seconds.
            let sec_digits: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if sec_digits.is_empty() {
                return None;
            }
            let secs: u64 = sec_digits.parse().ok()?;
            let total = minutes * 60 + secs;
            let sec_digits_end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(sec_digits.len());
            (total, &after[sec_digits_end..])
        }
    };
    // Title separator: accept '-', '–', '—', '|', '·', '.', ')', whitespace.
    let title = title_start.trim();
    let title = if let Some(idx) = title.find(|c: char| "-–—|·.".contains(c)) {
        let before_sep = &title[..idx].trim();
        let after_sep = &title[idx + 1..].trim();
        // If before_sep is numeric, it is part of the numbering, use after_sep.
        if before_sep.is_empty() || before_sep.parse::<u64>().is_ok() {
            after_sep
        } else {
            title
        }
    } else {
        title
    };
    let title = title.trim();
    if title.len() < 1 {
        return None;
    }
    // Reject false positives: lines like "R.I.P.", year ranges, manufacturer.
    if title.starts_with("R.I.P.") || title.starts_with("Manufactured") || title.starts_with("Music By") {
        return None;
    }
    Some((total, title))
}

/// Parse per-track durations from a hand-typed tracklist line of the form
/// `NN. Title MM:SS` where the timestamp is a DURATION at line end (not a
/// position). Tolerates semicolon typos like `02;26`. Returns
/// `(duration_secs, title)`.
fn try_extract_trailing_duration(line: &str) -> Option<(u64, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Normalize semicolon typos: "02;26" -> "02:26"
    let norm = line.replace(';', ":");
    // Find the LAST "MM:SS" / "HH:MM:SS" at end of line.
    let colon = norm.rfind(':')?;
    let after_colon = &norm[colon + 1..];
    let secs: u64 = after_colon.trim().parse().ok()?;
    if secs >= 60 {
        return None;
    }
    let before = &norm[..colon];
    let before_trim = before.trim_end();
    let mins_start = before_trim
        .bytes()
        .rev()
        .position(|b| !b.is_ascii_digit())
        .map(|p| before_trim.len() - p)
        .unwrap_or(0);
    let mins: u64 = before_trim[mins_start..].parse().ok()?;
    if mins >= 100 {
        return None;
    }
    let total = mins * 60 + secs;
    // The prefix before the timestamp is the track title (strip "NN. " numbering).
    let mut title = before_trim[..mins_start].trim().to_string();
    let number_end = title.find(|c: char| !c.is_ascii_digit() && c != '.' && c != ')' && c != '-').unwrap_or(title.len());
    let after_number = title[number_end..].trim().to_string();
    if after_number.len() < title.len() {
        title = after_number;
    }
    if title.is_empty() || title.len() < 1 {
        return None;
    }
    // Reject metadata lines, not tracks.
    if title.starts_with("Tracklist") || title.starts_with("Track List") || title.starts_with("Total") {
        return None;
    }
    Some((total, title))
}

/// Parse a description whose tracklist uses per-track DURATIONS at line end
/// (`NN. Title MM:SS`). Sum of durations must reach at least 30% of total and
/// >= 2 tracks must be present.
pub fn parse_description_durations(
    description: &str,
    total_duration_secs: u64,
) -> Option<Vec<AlbumTrack>> {
    let mut tracks: Vec<AlbumTrack> = Vec::new();
    for line in description.lines() {
        if let Some((dur, title)) = try_extract_trailing_duration(line) {
            tracks.push(AlbumTrack {
                title,
                duration_secs: dur as f64,
                artist: None,
            });
        }
    }
    if tracks.len() < 2 {
        debug!("parse_durations: only {} duration lines found, need >= 2", tracks.len());
        return None;
    }
    let sum: u64 = tracks.iter().map(|t| t.duration_secs as u64).sum();
    let total = total_duration_secs as f64;
    if total > 0.0 && (sum as f64) < total * 0.3 {
        debug!("parse_durations: sum {sum}s < 30% of {total}s, rejecting");
        return None;
    }
    debug!("parse_durations: parsed {} tracks, sum {sum}s", tracks.len());
    Some(tracks)
}

/// Parse a YouTube description string into an ordered tracklist.
/// Returns Some only if >= 2 tracks with increasing timestamps are found and
/// they reach at least 30% of `total_duration_secs`.
pub fn parse_description_timestamps(
    description: &str,
    total_duration_secs: u64,
) -> Option<Vec<AlbumTrack>> {
    let lines: Vec<&str> = description.lines().collect();
    let mut candidates: Vec<(u64, String)> = Vec::new();
    for line in &lines {
        if let Some((secs, title)) = try_extract_timestamp(line) {
            candidates.push((secs, title.to_string()));
        }
    }
    if candidates.len() < 2 {
        debug!("parse_description: only {} timestamp lines found, need >= 2", candidates.len());
        return None;
    }
    // Sort by timestamp ascending, dedupe by exact second.
    candidates.sort_by_key(|(s, _)| *s);
    candidates.dedup_by(|a, b| a.0 == b.0);
    if candidates.len() < 2 {
        return None;
    }
    // Validate strictly increasing.
    for i in 1..candidates.len() {
        if candidates[i].0 <= candidates[i - 1].0 {
            debug!("parse_description: non-increasing at {i}, rejecting");
            return None;
        }
    }
    // Coverage check: last timestamp must reach at least 30% of total.
    // This rejects short preview lists that do not cover the bulk of the video.
    let total = total_duration_secs as f64;
    let last_timestamp = candidates.last().unwrap().0 as f64;
    if total > 0.0 && last_timestamp < total * 0.3 {
        debug!("parse_description: last timestamp {last_timestamp}s < 30% of {total}s, rejecting");
        return None;
    }
    // Build AlbumTrack list.
    let mut tracks = Vec::with_capacity(candidates.len());
    for i in 0..candidates.len() {
        let cur = candidates[i].0 as f64;
        let next = if i + 1 < candidates.len() {
            candidates[i + 1].0 as f64
        } else {
            total
        };
        let dur = next - cur;
        let dur = if dur <= 0.0 {
            if total > 0.0 { total / candidates.len() as f64 } else { 180.0 }
        } else if dur > total * 1.5 {
            debug!("parse_description: outlier duration {dur} at {i}, using avg");
            if total > 0.0 { total / candidates.len() as f64 } else { 180.0 }
        } else {
            dur
        };
        tracks.push(AlbumTrack {
            title: candidates[i].1.clone(),
            duration_secs: dur,
            artist: None,
        });
    }
    debug!("parse_description: parsed {} tracks from description", tracks.len());
    Some(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yvda_case() {
        let desc = "1. 00:00 - El Niño\n2. 04:56 - Slash-And-Burn\n3. 09:30 - NOx Over Europe\n4. 15:57 - Encore\n5. 21:03 - Erosion\n6. 26:51 - Cool Down\n7. 34:21 - Incinerator (Green Point Mix)\n8. 39:47 - Smoky Mountains\n9. 44:37 - Modulation One\n10. 51:50 - Maximum Credible Accident\n\nManufactured By - House-Audio Studios\nMusic By [All Tracks By] - Winterkaelte\nPhotography By [Photo], Artwork - Nicola Bork\nHoused in a SmartPac.\nR.I.P. Eric de Vries\n04.07.1959 - 28.10.2024";
        let total = 3847;
        let tracks = parse_description_timestamps(desc, total).expect("should parse");
        assert_eq!(tracks.len(), 10);
        assert_eq!(tracks[0].title, "El Niño");
        assert_eq!(tracks[0].duration_secs, 296.0);
        assert_eq!(tracks[1].title, "Slash-And-Burn");
        assert_eq!(tracks[1].duration_secs, 274.0);
        assert_eq!(tracks[9].title, "Maximum Credible Accident");
        assert_eq!(tracks[9].duration_secs, 737.0);
    }

    #[test]
    fn mm_ss_dash() {
        let desc = "00:00 - A\n3:00 - B\n6:00 - C";
        let t = parse_description_timestamps(desc, 360).expect("should parse");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].duration_secs, 180.0);
        assert_eq!(t[1].duration_secs, 180.0);
    }

    #[test]
    fn hh_mm_ss() {
        let desc = "01:00:00 - Long\n02:00:00 - End";
        let t = parse_description_timestamps(desc, 7200).expect("should parse");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].duration_secs, 3600.0);
    }

    #[test]
    fn bracket_no_number() {
        let desc = "[00:00] A\n[3:00] B";
        let t = parse_description_timestamps(desc, 180).expect("should parse");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn no_number_prefix() {
        let desc = "0:00 A\n3:00 B";
        let t = parse_description_timestamps(desc, 180).expect("should parse");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn en_dash_separator() {
        let desc = "00:00 - A\n3:00 - B";
        let t = parse_description_timestamps(desc, 180).expect("should parse");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn mixed_formats() {
        let desc = "1. 00:00 - A\n03:00 B\n[6:00] C";
        let t = parse_description_timestamps(desc, 360).expect("should parse");
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn single_track_rejected() {
        let desc = "00:00 - A";
        assert!(parse_description_timestamps(desc, 60).is_none());
    }

    #[test]
    fn duplicate_timestamp_rejected() {
        let desc = "00:00 - A\n00:00 - B";
        assert!(parse_description_timestamps(desc, 60).is_none());
    }

    #[test]
    fn non_track_lines_ignored() {
        let desc = "1. 00:00 - First\n2. 03:00 - Second\n\nManufactured By - Foo\nMusic By [All Tracks By] - Bar\nR.I.P. Someone\nHoused in a SmartPac.";
        let t = parse_description_timestamps(desc, 180).expect("should parse");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn empty_description_none() {
        assert!(parse_description_timestamps("", 60).is_none());
    }

    #[test]
    fn coverage_rejected() {
        // Only 2 tracks: last timestamp is 30s out of 600s total = 5% -> None
        let desc = "00:00 - A\n00:30 - B";
        assert!(parse_description_timestamps(desc, 600).is_none());
    }

    #[test]
    fn chapters_fallback() {
        let chapters = [(0.0, "A"), (180.0, "B"), (360.0, "C")];
        let t = parse_chapters_timestamps(&chapters, 360).expect("should parse");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].duration_secs, 180.0);
        assert_eq!(t[1].duration_secs, 180.0);
        assert_eq!(t[2].duration_secs, 120.0);
    }

    #[test]
    fn trailing_duration_format() {
        let desc = "01. Untitled 00:59\n02. Untitled 00:59\n03. Untitled 00:43";
        let t = parse_description_durations(desc, 161).expect("should parse");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].title, "Untitled");
        assert_eq!(t[0].duration_secs, 59.0);
        assert_eq!(t[2].duration_secs, 43.0);
    }

    #[test]
    fn trailing_duration_semicolon_typo() {
        let desc = "01. Untitled 00:59\n09. Untitled 02;26\n27. Untitled 01:06";
        let t = parse_description_durations(desc, 300).expect("should parse");
        assert_eq!(t.len(), 3);
        assert_eq!(t[1].duration_secs, 146.0);
    }

    #[test]
    fn trailing_duration_rejects_tracklist_header() {
        let desc = "Tracklist:\n01. Untitled 00:59";
        assert!(parse_description_durations(desc, 60).is_none());
    }

    #[test]
    fn trailing_duration_coverage_rejected() {
        // Only 2 tracks, sum 90s out of 900s total = 10% -> None
        let desc = "01. Untitled 00:30\n02. Untitled 01:00";
        assert!(parse_description_durations(desc, 900).is_none());
    }

    #[test]
    fn vomitoma_full_description_parses() {
        let desc = "'' Nuclear Cesspool Of Parasitic Scum '' 2009\nTracklist:\n01. Untitled 00:59\n02. Untitled 00:59\n03. Untitled 00:43\n04. Untitled 00:58\n05. Untitled 00:50\n06. Untitled 00:55\n07. Untitled 01:09\n08. Untitled 04:59\n09. Untitled 02;26\n10. Untitled 04:33\n11. Untitled 02;12\n12. Untitled 02:31\n13. Untitled 01:01\n14. Untitled 00;35\n15. Untitled 00:34\n16. Untitled 00:48\n17. Untitled 01:14\n18. Untitled 00:37\n19. Untitled 00:43\n20. Untitled 00:57\n21. Untitled 01:20\n22. Untitled 00:26\n23. Untitled 01:07\n24. Untitled 01:24\n25. Untitled 01:00\n26. Untitled 00:54\n27. Untitled 01:06";
        let t = parse_description_durations(desc, 2244).expect("should parse");
        assert_eq!(t.len(), 27);
        // Sum of durations 2220s, within 30s of the 2244s video.
        let sum: u64 = t.iter().map(|x| x.duration_secs as u64).sum();
        assert!((sum as i64 - 2244).abs() <= 30);
    }

    #[test]
    fn vomitoma_description_beats_garbage_chapters() {
        // Real yt-dlp JSON shape for CDsJBLrT_UM: YouTube auto-generated
        // chapters whose starts are the tracklist duration values sorted
        // ascending (25 entries, duplicates at 43 and 59), while the
        // description carries the correct 27-track order and durations.
        let json: serde_json::Value = serde_json::json!({
            "duration": 2244,
            "chapters": [
                {"start_time": 0, "title": "<Untitled Chapter 1>"},
                {"start_time": 26, "title": "22. Untitled"},
                {"start_time": 34, "title": "15. Untitled"},
                {"start_time": 37, "title": "18. Untitled"},
                {"start_time": 43, "title": "03. Untitled"},
                {"start_time": 43, "title": "19. Untitled"},
                {"start_time": 48, "title": "16. Untitled"},
                {"start_time": 50, "title": "05. Untitled"},
                {"start_time": 54, "title": "26. Untitled"},
                {"start_time": 55, "title": "06. Untitled"},
                {"start_time": 57, "title": "20. Untitled"},
                {"start_time": 58, "title": "04. Untitled"},
                {"start_time": 59, "title": "01. Untitled"},
                {"start_time": 59, "title": "02. Untitled"},
                {"start_time": 60, "title": "25. Untitled"},
                {"start_time": 61, "title": "13. Untitled"},
                {"start_time": 66, "title": "27. Untitled"},
                {"start_time": 67, "title": "23. Untitled"},
                {"start_time": 69, "title": "07. Untitled"},
                {"start_time": 74, "title": "17. Untitled"},
                {"start_time": 80, "title": "21. Untitled"},
                {"start_time": 84, "title": "24. Untitled"},
                {"start_time": 151, "title": "12. Untitled"},
                {"start_time": 273, "title": "10. Untitled"},
                {"start_time": 299, "title": "08. Untitled"}
            ],
            "description": "'' Nuclear Cesspool Of Parasitic Scum '' 2009\nTracklist:\n01. Untitled 00:59\n02. Untitled 00:59\n03. Untitled 00:43\n04. Untitled 00:58\n05. Untitled 00:50\n06. Untitled 00:55\n07. Untitled 01:09\n08. Untitled 04:59\n09. Untitled 02;26\n10. Untitled 04:33\n11. Untitled 02;12\n12. Untitled 02:31\n13. Untitled 01:01\n14. Untitled 00;35\n15. Untitled 00:34\n16. Untitled 00:48\n17. Untitled 01:14\n18. Untitled 00:37\n19. Untitled 00:43\n20. Untitled 00:57\n21. Untitled 01:20\n22. Untitled 00:26\n23. Untitled 01:07\n24. Untitled 01:24\n25. Untitled 01:00\n26. Untitled 00:54\n27. Untitled 01:06"
        });
        let t = album_tracks_from_json(&json, "CDsJBLrT_UM");
        assert_eq!(t.len(), 27, "must prefer description's 27 tracks over chapters' 23");
        // Correct order: track 01 first (59s), not chapter-garbage order.
        assert_eq!(t[0].title, "Untitled");
        assert_eq!(t[0].duration_secs, 59.0);
        assert_eq!(t[1].duration_secs, 59.0);
        assert_eq!(t[2].duration_secs, 43.0);
        // Track 09 (semicolon typo 02;26 = 146s) survives at position 8.
        assert_eq!(t[8].duration_secs, 146.0);
        // Last track 27 = 66s, not the 32:25 pseudo-track from chapters.
        assert_eq!(t[26].duration_secs, 66.0);
    }

    #[test]
    fn year_from_description_ignores_timestamps() {
        let desc = "'' Nuclear Cesspool Of Parasitic Scum '' 2009\n01. Untitled 00:59\n09. Untitled 02;26\n14. Untitled 00;35";
        assert_eq!(extract_year_from_description(desc).as_deref(), Some("2009"));
    }

    #[test]
    fn year_from_description_none_without_year() {
        let desc = "01. Untitled 00:59\n02. Untitled 01:00";
        assert_eq!(extract_year_from_description(desc), None);
    }

    #[test]
    fn year_from_dlp_json_prefers_release_year() {
        let json = serde_json::json!({"release_year": 2011, "description": "Album 2009"});
        assert_eq!(year_from_dlp_json(&json).as_deref(), Some("2011"));
    }

    #[test]
    fn year_from_dlp_json_never_uses_upload_date() {
        // CDsJBLrT_UM: upload 2014-10-05, album 2009. Upload year must not win.
        let json = serde_json::json!({"upload_date": "20141005", "description": "'' Nuclear Cesspool Of Parasitic Scum '' 2009"});
        assert_eq!(year_from_dlp_json(&json).as_deref(), Some("2009"));
        let json_no_desc = serde_json::json!({"upload_date": "20141005"});
        assert_eq!(year_from_dlp_json(&json_no_desc), None);
    }

    // CDsJBLrT_UM description durations, 27 "Untitled" tracks.
    const VOMITOMA_DLP_DURS: &[f64] = &[
        59.0, 59.0, 43.0, 58.0, 50.0, 55.0, 69.0, 299.0, 146.0, 273.0, 132.0, 151.0, 61.0, 35.0,
        34.0, 48.0, 74.0, 37.0, 43.0, 57.0, 80.0, 26.0, 67.0, 84.0, 60.0, 54.0, 66.0,
    ];

    // Live Last.fm response for "Vomitoma - Nuclear Cesspool Of Parasitic Scum"
    // (fetched 2026-09-29): 70 entries, merged from several releases.
    // Entries 0-6 = core album, 7-54 = other release (no durations), 55-69 =
    // outro tracks that sit at description positions 12-26. 0.0 = no duration.
    const LFM_70: &[(&str, f64)] = &[
        ("Gritty & Greasy", 60.0),
        ("To Regurgitate Live Insects", 0.0),
        ("Bags Of Discarded Sarcoma Tissue", 44.0),
        ("Pulsating Teratoma Membrane", 59.0),
        ("Bloated, Busting wall Of Intestines", 51.0),
        ("Coughed Up. Chewed Up, And Swallowed", 56.0),
        ("Miscarriage Through The Colostomy Bag", 69.0),
        ("Skinflap", 0.0),
        ("Acidic Abortion", 0.0),
        ("Suppurative Gastro-Sewage", 0.0),
        ("Slough Blisters", 0.0),
        ("Green/White/Purple", 0.0),
        ("Autoerotic Laceration", 0.0),
        ("They Will All Die At My Hands...", 0.0),
        ("Poxed & Perforated", 0.0),
        ("Expulsed Esophageal Elements", 0.0),
        ("Septicoccus", 0.0),
        ("Pre-masticated stagnant chyme", 0.0),
        ("Fill The Mouth With Broken Glass, Sew It Shut", 0.0),
        ("Stench Of A Female", 0.0),
        ("Siamese Miscarriage", 0.0),
        ("Grave Wax", 0.0),
        ("Enterobacteriaceae", 0.0),
        ("Soft Vibration Of Bones Breaking", 0.0),
        ("Septic Convulsions", 0.0),
        ("Over-Inhalation Of A Poisonous Purulent Mist, Causing Death", 0.0),
        ("I Vomit Into Her Suppurated Throat", 0.0),
        ("Melting Into Carbonized Paste", 0.0),
        ("Erotic penetration of worms", 0.0),
        ("Eaten Alive For The Thrill", 0.0),
        ("Chunks Of Uric Sludge", 0.0),
        ("Systematic Devourment Of The Ant Covered Carcass", 0.0),
        ("Spiders Flow From The Wounds", 0.0),
        ("Adipocesspool", 0.0),
        ("The Dead Look So Alive With All The Insects Feeding...", 0.0),
        ("She Was Wet With Rot", 0.0),
        ("Darkened From Decomposure", 0.0),
        ("Cemetery Landscape", 0.0),
        ("Diseasewagenitals", 0.0),
        ("Curdling Of The Maggot Skins", 0.0),
        ("Pus-filled abortion tomb", 0.0),
        ("Busted Sores & Bloated Bodies", 0.0),
        ("Nasophatyngeal Myasis", 0.0),
        ("Fetal Poisoning", 0.0),
        ("Histerectomeat", 0.0),
        ("Embalmed And Plasticised For Necrophilic Experimentation", 0.0),
        ("Fragmented Fly Egg Regurgitation", 0.0),
        ("The still-birth of the sarcomaggot", 0.0),
        ("Growth From Septic Matter", 0.0),
        ("Worms Flooded Out Of Her Cervical Orifice", 0.0),
        ("Green Cadaver Honey", 0.0),
        ("Stagnant Tombs Overgrown With Mold", 0.0),
        ("Jelly-Like Ebola Patient", 0.0),
        ("Fetal Deformities Caused By Parasites", 0.0),
        ("Shit Forced Back Into The Body Through The Colostomy Hole", 0.0),
        ("Green showers (Intro)/Sandblasted with bile", 62.0),
        ("Gelatinous Pusfilled Membranic Cysts", 35.0),
        ("Rotting in the oven", 34.0),
        ("Reverse vomit osmosis", 48.0),
        ("Mass of purulent bodies adhesing to one another", 75.0),
        ("Ejaculate in her bladder", 37.0),
        ("Boiling septic tank vapors", 0.0),
        ("Smothered in busted glass and rusted razors", 57.0),
        ("Scrape fetish", 81.0),
        ("Spider nest in the nasal cavity", 27.0),
        ("Cunt skin condom", 68.0),
        ("Fragmented stomachal slop", 84.0),
        ("Slow Styric-P Slop", 60.0),
        ("Submerged in tapeworm guts", 54.0),
        ("20 chunks of human scrap", 66.0),
    ];

    fn untitled_dlp() -> Vec<AlbumTrack> {
        VOMITOMA_DLP_DURS
            .iter()
            .map(|d| AlbumTrack {
                title: "Untitled".into(),
                duration_secs: *d,
                artist: None,
            })
            .collect()
    }

    fn lfm_provider() -> Vec<AlbumTrack> {
        LFM_70
            .iter()
            .map(|(n, d)| AlbumTrack {
                title: (*n).to_string(),
                duration_secs: *d,
                artist: None,
            })
            .collect()
    }

    #[test]
    fn fill_untitled_real_lfm_fixture_renames_all_27() {
        let mut dlp = untitled_dlp();
        let provider = lfm_provider();
        let renamed = fill_untitled_titles(&mut dlp, &provider);
        assert_eq!(renamed, 27, "all 27 placeholders must be renamed");
        // video[0..12] aligns to LFM[0..12] by ~1s duration diffs.
        assert_eq!(dlp[0].title, "Gritty & Greasy");
        assert_eq!(dlp[7].title, "Skinflap");
        // video[12..27] skips the 43-entry junk block, aligns to LFM[55..70].
        assert_eq!(dlp[12].title, "Green showers (Intro)/Sandblasted with bile");
        assert_eq!(dlp[18].title, "Boiling septic tank vapors");
        assert_eq!(dlp[26].title, "20 chunks of human scrap");
        // dlp durations untouched: rename only.
        assert_eq!(dlp[7].duration_secs, 299.0);
        assert_eq!(dlp[26].duration_secs, 66.0);
    }

    #[test]
    fn fill_untitled_keeps_real_dlp_titles() {
        let mut dlp = vec![
            AlbumTrack { title: "Real Song".into(), duration_secs: 100.0, artist: None },
            AlbumTrack { title: "Other Real".into(), duration_secs: 200.0, artist: None },
        ];
        let provider = vec![
            AlbumTrack { title: "Provider One".into(), duration_secs: 101.0, artist: None },
            AlbumTrack { title: "Provider Two".into(), duration_secs: 199.0, artist: None },
        ];
        assert_eq!(fill_untitled_titles(&mut dlp, &provider), 0);
        assert_eq!(dlp[0].title, "Real Song");
        assert_eq!(dlp[1].title, "Other Real");
    }

    #[test]
    fn fill_untitled_empty_provider_noop() {
        let mut dlp = vec![AlbumTrack {
            title: "Untitled".into(),
            duration_secs: 59.0,
            artist: None,
        }];
        assert_eq!(fill_untitled_titles(&mut dlp, &[]), 0);
        assert_eq!(dlp[0].title, "Untitled");
    }

    #[test]
    fn fill_untitled_never_copies_provider_placeholders() {
        let mut dlp = vec![
            AlbumTrack { title: "Untitled".into(), duration_secs: 59.0, artist: None },
            AlbumTrack { title: "Untitled".into(), duration_secs: 60.0, artist: None },
        ];
        let provider = vec![
            AlbumTrack { title: "Untitled".into(), duration_secs: 59.0, artist: None },
            AlbumTrack { title: "Real Name".into(), duration_secs: 60.0, artist: None },
        ];
        assert_eq!(fill_untitled_titles(&mut dlp, &provider), 1);
        assert_eq!(dlp[0].title, "Untitled");
        assert_eq!(dlp[1].title, "Real Name");
    }

    #[test]
    fn fill_untitled_wrong_album_yields_zero_renames() {
        // Provider tracklist for a different album: every duration mismatch
        // scores -20 while dropping both sides scores -11, so the DP skips
        // everything instead of renaming from wrong names.
        let mut dlp = untitled_dlp();
        let provider: Vec<AlbumTrack> = (0..27)
            .map(|i| AlbumTrack {
                title: format!("Wrong Track {}", i),
                duration_secs: 500.0 + i as f64,
                artist: None,
            })
            .collect();
        assert_eq!(fill_untitled_titles(&mut dlp, &provider), 0);
        assert_eq!(dlp[0].title, "Untitled");
    }

    #[test]
    fn placeholder_title_detection() {
        assert!(is_placeholder_track_title("Untitled"));
        assert!(is_placeholder_track_title("  untitled "));
        assert!(is_placeholder_track_title("01. Untitled"));
        assert!(is_placeholder_track_title("01.Untitled"));
        assert!(is_placeholder_track_title("<Untitled Chapter 1>"));
        assert!(is_placeholder_track_title(""));
        assert!(is_placeholder_track_title("Track 1"));
        assert!(is_placeholder_track_title("Track 12"));
        assert!(is_placeholder_track_title("01"));
        assert!(is_placeholder_track_title("123"));
        assert!(!is_placeholder_track_title("Gritty & Greasy"));
        assert!(!is_placeholder_track_title("Untitled Pleasures"));
        assert!(!is_placeholder_track_title("1"));
    }

    #[test]
    fn extract_year_word_boundaries() {
        assert_eq!(extract_year_from_description("'' Nuclear Cesspool Of Parasitic Scum '' 2009"), Some("2009".into()));
        assert_eq!(extract_year_from_description("Limited to 2000 copies"), None);
        assert_eq!(extract_year_from_description("Recorded at 2400bps"), None);
        assert_eq!(extract_year_from_description("2000copies"), None);
        assert_eq!(extract_year_from_description("Album 2009."), Some("2009".into()));
        assert_eq!(extract_year_from_description(""), None);
    }

    #[test]
    fn fill_untitled_all_provider_placeholders_noop() {
        let mut dlp = vec![
            AlbumTrack { title: "Untitled".into(), duration_secs: 59.0, artist: None },
        ];
        let provider = vec![
            AlbumTrack { title: "Untitled".into(), duration_secs: 59.0, artist: None },
            AlbumTrack { title: "".into(), duration_secs: 60.0, artist: None },
        ];
        assert_eq!(fill_untitled_titles(&mut dlp, &provider), 0);
        assert_eq!(dlp[0].title, "Untitled");
    }
}
