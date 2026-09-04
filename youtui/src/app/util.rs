//! YouTube description / chapters timestamp parser for album-split fallback.
//!
//! When metadata providers return 0 tracks for a full-album YouTube upload,
//! this parser reads the yt-dlp JSON `description` field (and optionally
//! `chapters`) and produces a `Vec<AlbumTrack>` that drives `insert_album_tracks`.
//! See study report for the YVdaCDJ1s-E case (3847s, 10 tracks from description).

use crate::app::server::AlbumTrack;
use serde_json;
use tracing::{debug, info};

/// Fetch yt-dlp JSON for `video_id` and parse chapters/description into AlbumTracks.
pub async fn fetch_yt_dlp_album_tracks(video_id: &str) -> Vec<AlbumTrack> {
    let output = match tokio::process::Command::new("yt-dlp")
        .args(["--dump-json", "--no-warnings", &format!("https://youtu.be/{}", video_id)])
        .output()
        .await
    {
        Ok(out) => out,
        Err(e) => {
            info!("yt-dlp fallback: failed to spawn yt-dlp for video {}: {}", video_id, e);
            return Vec::new();
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        info!("yt-dlp fallback: yt-dlp failed for video {}: {}", video_id, stderr.trim());
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            info!("yt-dlp fallback: JSON parse failed for video {}: {}", video_id, e);
            return Vec::new();
        }
    };
    let duration_secs: u64 = json
        .get("duration")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Chapters first
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

    if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
        if let Some(tracks) = parse_description_timestamps(desc, duration_secs) {
            if !tracks.is_empty() {
                info!("yt-dlp fallback: parsed {} tracks from description for video {}", tracks.len(), video_id);
                return tracks;
            }
        }
    }

    Vec::new()
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
}
