//! Bandcamp URL resolution helpers.
//!
//! Pure functions only: URL classification, normalization, and parsing of
//! yt-dlp `--flat-playlist --dump-json` output. No I/O here, so every
//! function is unit-testable without network access.

/// True when `url` points at a bandcamp artist subdomain page
/// (`<artist>.bandcamp.com/...`). Rejects the main site and weekly radio:
/// those are not resolvable to playable tracks by the yt-dlp pipeline.
pub fn is_bandcamp_url(url: &str) -> bool {
    let Some(host) = url_to_host(url) else {
        return false;
    };
    host.ends_with(".bandcamp.com") && host != "bandcamp.com"
}

/// Strip query string, fragment, and trailing slash from a bandcamp URL.
///
/// Keeps `scheme://host/path`. Tracking junk like `?fbclid=...` is dropped
/// so the same album pasted twice (with different queries) dedups to one
/// queue entry. Non-bandcamp input is returned unchanged.
pub fn normalize_bandcamp_url(url: &str) -> String {
    if !is_bandcamp_url(url) {
        return url.to_string();
    }
    let without_query = url.split('?').next().unwrap_or(url);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    let trimmed = without_fragment.trim_end_matches('/');
    trimmed.to_string()
}

/// What kind of playable entity a bandcamp URL names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandcampKind {
    /// A single track page (`/track/<slug>`).
    Track,
    /// An album page (`/album/<slug>`).
    Album,
    /// Artist root (`/<artist>.bandcamp.com`) or `/music` page: the whole
    /// discography.
    Discography,
}

/// Classify a normalized bandcamp URL. Returns `None` for URLs that are not
/// bandcamp or do not name a playable entity.
pub fn bandcamp_kind(url: &str) -> Option<BandcampKind> {
    if !is_bandcamp_url(url) {
        return None;
    }
    let path = url.split('?').next().unwrap_or(url).split('#').next().unwrap_or(url);
    let path = path.trim_end_matches('/');
    let Some((_, rest)) = path.split_once("://") else {
        return None;
    };
    let Some((_, path)) = rest.split_once('/') else {
        // No path at all: `<artist>.bandcamp.com` -> discography.
        return Some(BandcampKind::Discography);
    };
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [] => Some(BandcampKind::Discography),
        ["music"] => Some(BandcampKind::Discography),
        ["track", _] => Some(BandcampKind::Track),
        ["album", _] => Some(BandcampKind::Album),
        _ => None,
    }
}

/// One track entry parsed from yt-dlp `--flat-playlist --dump-json` output.
///
/// The flat-playlist probe returns all track metadata in a single yt-dlp call.
/// Parsing it here avoids 349 individual `--dump-json` probes (one per track)
/// which trigger Bandcamp HTTP 429 rate limiting on large compilation albums.
#[derive(Debug, Clone, PartialEq)]
pub struct BandcampTrackEntry {
    pub url: String,
    pub title: String,
    pub duration_secs: f64,
    pub uploader: String,
    pub album: Option<String>,
    pub track: Option<String>,
    pub track_no: Option<String>,
    /// Real artist from the tralbum page: per-track `artist` on compilations,
    /// album-level `current.artist` on single-band releases. Flat-playlist JSON
    /// often carries neither (`playlist_uploader: null`), so without this the
    /// queue artist column stays empty for artist-owned album pages.
    pub artist: Option<String>,
    /// Album art URL shared by every track on the page.
    pub cover_url: Option<String>,
    /// Year published by Bandcamp for the album, shared by every track.
    pub year: Option<String>,
}

/// Type of entity returned by `bcsearch_public_api` autocomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandcampType {
    /// A single track (`type: "t"` in API response).
    Track,
    /// An album (`type: "a"`).
    Album,
    /// A band/artist (`type: "b"`).
    Band,
}

/// One search result from `bcsearch_public_api`.
///
/// The endpoint requires an explicit `search_filter` (`"t"` tracks, `"a"` albums,
/// `"b"` bands) and returns up to 50 homogeneous results per call, so callers
/// issue one request per type. Each result carries a `type` field.
#[derive(Debug, Clone, PartialEq)]
pub struct BandcampSearchResult {
    pub type_: BandcampType,
    /// Track title, album title, or band name.
    pub name: String,
    /// Artist/band name (same as `name` for bands, which have no `band_name`).
    pub band_name: String,
    /// Full URL to the item page.
    pub url: String,
    /// Album name (only for track results).
    pub album_name: Option<String>,
}

/// Parse `bcsearch_public_api` autocomplete response into typed results.
///
/// Each item in `auto.results[]` carries `type` (`"t"`/`"a"`/`"b"`) and `name`.
/// Tracks and albums expose `item_url_path` plus `band_name`; bands expose
/// `item_url_root` and no `band_name`, so the URL falls back to the root and the
/// band name defaults to `name`. Items lacking both URL fields are skipped.
pub fn parse_bandcamp_search_results_all_types(json: &serde_json::Value) -> Vec<BandcampSearchResult> {
    json.get("auto")
        .and_then(|a| a.get("results"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let type_str = v.get("type").and_then(|t| t.as_str())?;
                    let type_ = match type_str {
                        "t" => BandcampType::Track,
                        "a" => BandcampType::Album,
                        "b" => BandcampType::Band,
                        _ => return None,
                    };
                    let name = v.get("name")?.as_str()?.to_string();
                    let url = ["item_url_path", "item_url_root"]
                        .iter()
                        .filter_map(|k| v.get(*k).and_then(|u| u.as_str()))
                        .map(|s| s.to_string())
                        .find(|s| !s.is_empty())?;
                    let band_name = v
                        .get("band_name")
                        .and_then(|b| b.as_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&name)
                        .to_string();
                    let album_name = v.get("album_name").and_then(|a| a.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                    Some(BandcampSearchResult {
                        type_,
                        name,
                        band_name,
                        url,
                        album_name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Lowercase, alphanumeric-only. Bandcamp and Last.fm disagree constantly on
/// case, punctuation and spacing, so all matching goes through this first.
fn normalize_for_match(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Does a Bandcamp search result plausibly answer a request for
/// `request_artist` / `request_title`?
///
/// Bandcamp autocomplete is fuzzy and returns whatever loosely resembles the
/// query, so a raw first-hit would happily queue an unrelated track when a
/// user presses Enter on a recommendation. Both the artist and the title must
/// line up before a result is accepted:
///
/// - artist: `band_name` must equal the requested artist, or contain it, or be
///   contained by it. The containment cases matter because Bandcamp credits
///   vary ("V/A", "Artist feat. Someone", "Artist [Various]").
/// - title: `name` must equal the requested title or contain it. An **empty**
///   `request_title` skips the check, which is what artist-kind lookups want.
///
/// An empty `request_artist` skips the artist check for the same reason.
pub fn bandcamp_result_matches(
    request_artist: &str,
    request_title: &str,
    result: &BandcampSearchResult,
) -> bool {
    title_artist_matches(
        request_artist,
        request_title,
        &result.band_name,
        &result.name,
    )
}

/// True when a candidate's artist and title plausibly answer the request.
///
/// Shared by the Bandcamp fallback and the YouTube-side verification in
/// `ActOnRecommendation`: both need the same fuzzy containment rules, and
/// two copies of them would drift. Artist containment runs in BOTH
/// directions so credit variants (`V/A`, `Artist feat. X`) match. An empty
/// request field skips its check; an empty candidate field fails it.
pub fn title_artist_matches(
    request_artist: &str,
    request_title: &str,
    got_artist: &str,
    got_title: &str,
) -> bool {
    let want_artist = normalize_for_match(request_artist);
    if !want_artist.is_empty() {
        let got_artist = normalize_for_match(got_artist);
        let artist_ok = !got_artist.is_empty()
            && (got_artist == want_artist
                || got_artist.contains(&want_artist)
                || want_artist.contains(&got_artist));
        if !artist_ok {
            return false;
        }
    }

    let want_title = normalize_for_match(request_title);
    if !want_title.is_empty() {
        let got_title = normalize_for_match(got_title);
        if got_title.is_empty() || !(got_title == want_title || got_title.contains(&want_title)) {
            return false;
        }
    }

    true
}

/// Parse yt-dlp `--flat-playlist --dump-json` output into structured track
/// entries.
///
/// Every line is one JSON object (flat-playlist mode), skipping playlist
/// entries (`_type == "playlist"`) and empty lines. Bandcamp album entries are
/// plain track URLs. The per-item `duration` key is absent from flat-playlist
/// output, so durations come from `parse_tralbum_tracks` instead.
fn first_non_empty_str(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| {
        value
            .get(*k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    })
}

pub fn parse_bandcamp_album_entries(stdout: &str) -> Vec<BandcampTrackEntry> {
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            tracing::debug!("bandcamp: skipping non-JSON line in album entries");
            continue;
        };
        if value.get("_type").and_then(|t| t.as_str()) == Some("playlist") {
            tracing::debug!("bandcamp: skipping nested playlist entry");
            continue;
        }
        let Some(url) = value.get("url").and_then(|u| u.as_str()) else {
            continue;
        };
        if url.is_empty() {
            continue;
        }
        let title = value
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let duration_secs = value.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
        let uploader = first_non_empty_str(&value, &["playlist_uploader", "uploader"]).unwrap_or_default();
        let album = first_non_empty_str(&value, &["playlist_title", "album"]);
        let track = first_non_empty_str(&value, &["playlist_autonumber", "track"]);
        entries.push(BandcampTrackEntry {
            url: url.to_string(),
            title,
            duration_secs,
            uploader,
            album,
            track,
            track_no: None,
            artist: None,
            cover_url: None,
            year: None,
        });
    }
    entries
}

/// One track from the album page's `data-tralbum` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct TralbumTrack {
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    /// Path component of `title_link` (e.g. `/track/neutralize`), used to match
    /// against the flat-playlist track URL.
    pub url_path: Option<String>,
    /// Position within the compilation, absent on singles.
    pub track_num: Option<u32>,
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find('&') {
        out.push_str(&rest[..idx]);
        rest = &rest[idx..];
        let entity_end = rest.find(';').map(|e| e + 1).unwrap_or(0);
        let entity = &rest[..entity_end];
        let decoded = match entity {
            "&quot;" => Some('"'),
            "&amp;" => Some('&'),
            "&#39;" => Some('\''),
            "&lt;" => Some('<'),
            "&gt;" => Some('>'),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[entity.len()..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Slice out the `trackinfo` JSON array from a Bandcamp album page.
///
/// The array is embedded HTML-entity-encoded inside the `data-tralbum`
/// attribute, so the raw text is scanned with a bracket depth counter that
/// toggles on `&quot;` string delimiters, then entity-decoded and parsed.
/// Returns `None` when the attribute or the `trackinfo` key is absent, which
/// is how Bandcamp serves its anti-bot challenge page.
fn extract_trackinfo_array(html: &str) -> Option<&str> {
    const KEY: &str = "&quot;trackinfo&quot;:";
    let key_start = html.find(KEY)? + KEY.len();
    let body = &html[key_start..];
    if !body.starts_with('[') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    for (idx, c) in body.char_indices() {
        match c {
            '&' if !in_string && body[idx..].starts_with("&quot;") => {
                in_string = true;
            }
            ';' if in_string && body[idx - 5..idx].ends_with("quot") => {
                in_string = false;
            }
            '[' if !in_string => depth += 1,
            ']' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&body[..=idx]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a Bandcamp album page into its per-track metadata.
///
/// This is the only source of real track durations: yt-dlp's flat-playlist mode
/// omits the `duration` key entirely, while the album page's `data-tralbum`
/// payload carries `duration`, `artist`, and `title_link` for every track.
pub fn parse_tralbum_tracks(html: &str) -> Option<Vec<TralbumTrack>> {
    let raw = extract_trackinfo_array(html)?;
    let decoded = decode_html_entities(raw);
    let parsed: serde_json::Value = serde_json::from_str(&decoded).ok()?;
    let arr = parsed.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| {
                Some(TralbumTrack {
                    title: v.get("title")?.as_str()?.to_string(),
                    artist: v.get("artist").and_then(|a| a.as_str()).unwrap_or("").to_string(),
                    duration_secs: v.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0),
                    url_path: v
                        .get("title_link")
                        .and_then(|l| l.as_str())
                        .map(|l| l.split('?').next().unwrap_or(l).to_string()),
                    track_num: v
                        .get("track_num")
                        .and_then(|n| n.as_u64())
                        .and_then(|n| u32::try_from(n).ok()),
                })
            })
            .collect(),
    )
}

pub fn parse_tralbum_art_url(html: &str) -> Option<String> {
    const MARKER: &str = "<meta property=\"og:image\" content=\"";
    let start = html.find(MARKER)? + MARKER.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Album-level artist from the tralbum `current` blob (`current.artist`).
///
/// Per-track `artist` is null on single-band album pages (Bandcamp only fills
/// it for compilations), so this is the only artist source for releases like
/// `anavenus.bandcamp.com/album/anal-seppuku`. The flat-playlist probe also
/// reports `playlist_uploader: null` for those pages, which previously left
/// the queue artist column empty.
pub fn parse_tralbum_artist(html: &str) -> Option<String> {
    const CURRENT: &str = "&quot;current&quot;";
    const TRACKINFO: &str = "&quot;trackinfo&quot;";
    const KEY: &str = "&quot;artist&quot;:&quot;";
    let blob_start = html.find(CURRENT)?;
    let blob_end = html[blob_start..].find(TRACKINFO)? + blob_start;
    let start = html[blob_start..blob_end].find(KEY)? + blob_start + KEY.len();
    let rest = &html[start..blob_end];
    let end = rest.find("&quot;")?;
    let artist = decode_html_entities(&rest[..end]);
    if artist.trim().is_empty() { None } else { Some(artist) }
}

/// Year Bandcamp itself publishes for the album, read from the tralbum
/// `current.release_date` blob (e.g. `"28 Sep 2026 18:26:00 GMT"` -> `"2026"`).
///
/// Bandcamp's own published date is the authoritative album date and is what the
/// user chose to display. It is also the only date available: the flat playlist
/// carries no year, and providers frequently have no release group for
/// Bandcamp-only label compilations.
pub fn parse_tralbum_release_year(html: &str) -> Option<String> {
    const CURRENT: &str = "&quot;current&quot;";
    const TRACKINFO: &str = "&quot;trackinfo&quot;";
    const KEY: &str = "&quot;release_date&quot;:&quot;";
    let blob_start = html.find(CURRENT)?;
    let blob_end = html[blob_start..].find(TRACKINFO)? + blob_start;
    let start = html[blob_start..blob_end].find(KEY)? + blob_start + KEY.len();
    let rest = &html[start..blob_end];
    let end = rest.find("&quot;")?;
    let date = decode_html_entities(&rest[..end]);
    let year: String = date
        .split(|c: char| !c.is_ascii_digit())
        .find(|p| p.len() == 4 && (1900..=2099).contains(&p.parse::<u32>().unwrap_or(0)))
        .map(|p| p.to_string())?;
    Some(year)
}

/// Fill missing durations, track numbers, and artists on `entries` from the
/// album page's `trackinfo` payload, falling back to `album_artist` (the
/// tralbum `current.artist` blob) when a track carries no per-track artist.
///
/// Matches on the `/track/...` path so a re-ordered or partially-returned
/// flat-playlist cannot shift metadata onto the wrong tracks; falls back to
/// positional matching only when no path match exists. Returns how many
/// entries gained at least one field.
pub fn merge_tralbum_metadata(
    entries: &mut [BandcampTrackEntry],
    tracks: &[TralbumTrack],
    album_artist: Option<&str>,
) -> usize {
    fn url_path(url: &str) -> &str {
        url.split("://")
            .nth(1)
            .and_then(|r| r.find('/').map(|i| &r[i..]))
            .unwrap_or("")
    }
    let mut touched = 0;
    for (idx, entry) in entries.iter_mut().enumerate() {
        let path = url_path(&entry.url);
        let by_path = if path.is_empty() {
            None
        } else {
            tracks.iter().find(|t| t.url_path.as_deref() == Some(path))
        };
        let source = by_path.or_else(|| tracks.get(idx));
        let mut touched_entry = false;
        if let Some(t) = source {
            if entry.duration_secs <= 0.0 && t.duration_secs > 0.0 {
                entry.duration_secs = t.duration_secs;
                touched_entry = true;
            }
            if entry.track_no.is_none()
                && let Some(n) = t.track_num
            {
                entry.track_no = Some(n.to_string());
                touched_entry = true;
            }
            if entry.artist.is_none()
                && !t.artist.trim().is_empty()
            {
                entry.artist = Some(t.artist.clone());
                touched_entry = true;
            }
        }
        // Single-band album pages null out per-track artist; the album-level
        // `current.artist` still names the band. Split albums, however, set
        // album-level artist to the full roster joined by " X " or " / "
        // ("D.O.M. X ALMA SEBOSA X KKAAOOSS"); using that as every track's
        // artist erases the per-track artist that the title prefix carries.
        if entry.artist.is_none()
            && let Some(a) = album_artist
            && !a.trim().is_empty()
            && !is_multi_artist_roster(a)
        {
            entry.artist = Some(a.to_string());
            touched_entry = true;
        }
        if touched_entry {
            touched += 1;
        }
    }
    touched
}

/// True when an album-level artist string is a multi-band split roster rather
/// than a single artist. Bandcamp joins split artists with `X` or `/`
/// ("D.O.M. X ALMA SEBOSA X KKAAOOSS", "Alma Sebosa / Obscuruus HarshNoise").
/// Such rosters must not overwrite a per-track artist.
pub fn is_multi_artist_roster(artist: &str) -> bool {
    artist
        .split_whitespace()
        .any(|tok| tok == "X" || tok == "x" || tok == "/")
}

pub fn url_to_host(url: &str) -> Option<&str> {
    let after_scheme = url.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

/// Cleaned bandcamp track metadata, resolved from the raw yt-dlp fields.
///
/// Bandcamp pages are owned by either the artist (`domnoise.bandcamp.com`)
/// or a label hosting many artists (`sphcrecords.bandcamp.com`). When the
/// page owner is a label, yt-dlp reports the label as uploader/artist and
/// the real artist only appears as the leading segment of the album name
/// ("Putrefação Humana - Colhendo Desespero EP (SPHC)"). This resolves both
/// layouts and prefers yt-dlp's `track` field (the definitive song name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBandcampMetadata {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
}

/// Resolve real artist, clean title, and album for a bandcamp track.
///
/// Priority for artist:
/// 1. tralbum artist (per-track on compilations, album-level on single-band
///    pages) — Bandcamp's own ground truth, passed in by the enrichment step
/// 2. album-name prefix when it differs from the page owner (label-hosted
///    albums, e.g. "Putrefação Humana - Colhendo Desespero EP (SPHC)")
/// 3. title prefix when it differs from the page owner
/// 4. title prefix (artist pages self-prefix, e.g. "D.O.M. - Song")
/// 5. uploader as last resort
///
/// Title prefers the `track` field (yt-dlp's definitive song name), else the
/// raw title with the owner/artist prefix stripped. Album keeps its name
/// with the resolved artist prefix removed.
pub fn resolve_bandcamp_metadata(
    raw_title: &str,
    uploader: &str,
    album: Option<&str>,
    track: Option<&str>,
    tralbum_artist: Option<&str>,
) -> ResolvedBandcampMetadata {
    fn split_prefix(s: &str) -> Option<(&str, &str)> {
        s.split_once(" - ")
            .map(|(a, b)| (a.trim(), b.trim()))
            .filter(|(a, b)| !a.is_empty() && !b.is_empty())
    }
    let title_prefix = split_prefix(raw_title);
    let album_prefix = album.and_then(split_prefix);
    // A split album's leading segment is the full artist roster; it is not a
    // per-track artist, so never let it beat the title prefix ("KKAAOOSS - ...").
    let album_prefix = album_prefix.filter(|(ap, _)| !is_multi_artist_roster(ap));

    let artist = match tralbum_artist.map(str::trim).filter(|a| !a.is_empty()) {
        Some(a) => a.to_string(),
        None => match (album_prefix, title_prefix) {
            (Some((ap, _)), _) if ap != uploader => ap.to_string(),
            (_, Some((tp, _))) if tp != uploader => tp.to_string(),
            (_, Some((tp, _))) => tp.to_string(),
            _ => uploader.to_string(),
        },
    };

    let album = match (album, album_prefix) {
        (Some(a), Some((ap, _))) if ap != uploader => Some(
            a.split_once(" - ")
                .map(|(_, b)| b.trim().to_string())
                .unwrap_or_else(|| a.to_string()),
        ),
        (Some(a), _) => Some(a.to_string()),
        (None, _) => None,
    };

    let title = match track.filter(|t| !t.trim().is_empty()) {
        Some(t) => t.trim().to_string(),
        None => {
            let rest = raw_title
                .strip_prefix(uploader)
                .and_then(|r| r.strip_prefix(" - "))
                .or_else(|| raw_title.strip_prefix(&format!("{} - ", artist)))
                .unwrap_or(raw_title);
            rest.trim().to_string()
        }
    };

    ResolvedBandcampMetadata { artist, title, album }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMNOISE_ALBUM_WITH_FBCLID: &str = "https://domnoise.bandcamp.com/album/diariamente-obriga-o-maltrata?fbclid=PAT01DUAUnUoVleHRuA2FlbQIxMABwZG9mAnNydGMGYXBwX2lkDzU2NzA2NzM0MzM1MjQyNwABp1UNYQUtttq1F5LzBGjjafBuhcKk_wSfXiWHkg3Dum9-1msVmWv5Hj4b1dJK_aem_TPeiHeAL6WyZ9eRBySuBBQ";

    const TRALBUM_PAGE: &str = r#"<div data-tralbum="{&quot;current&quot;:{&quot;title&quot;:&quot;Noise As A Form Of Expression Vol. 4&quot;},&quot;trackinfo&quot;:[{&quot;track_num&quot;:1,&quot;title&quot;:&quot;Boredom Knife - Neutralize&quot;,&quot;artist&quot;:&quot;Boredom Knife&quot;,&quot;duration&quot;:243.435,&quot;title_link&quot;:&quot;/track/neutralize&quot;},{&quot;track_num&quot;:2,&quot;title&quot;:&quot;Flesh-Control - Temper Wrecked&quot;,&quot;artist&quot;:&quot;Flesh-Control&quot;,&quot;duration&quot;:364.308,&quot;title_link&quot;:&quot;/track/temper-wrecked?amp;from=embed&quot;}],&quot;id&quot;:42}"></div>"#;

    #[test]
    fn tralbum_trackinfo_yields_durations_and_paths() {
        let tracks = parse_tralbum_tracks(TRALBUM_PAGE).expect("trackinfo present");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Boredom Knife - Neutralize");
        assert_eq!(tracks[0].artist, "Boredom Knife");
        assert!((tracks[0].duration_secs - 243.435).abs() < f64::EPSILON);
        assert_eq!(tracks[0].url_path.as_deref(), Some("/track/neutralize"));
        assert_eq!(tracks[0].track_num, Some(1));
        assert_eq!(tracks[1].url_path.as_deref(), Some("/track/temper-wrecked"));
        assert_eq!(tracks[1].track_num, Some(2));
    }

    #[test]
    fn tralbum_missing_key_returns_none() {
        assert!(parse_tralbum_tracks("<html>challenge page</html>").is_none());
    }

    #[test]
    fn art_url_read_from_og_image_meta() {
        let page = r#"<head><meta property="og:image" content="https://f4.bcbits.com/img/a0489092809_5.jpg"><meta property="og:title" content="x"></head>"#;
        assert_eq!(
            parse_tralbum_art_url(page).as_deref(),
            Some("https://f4.bcbits.com/img/a0489092809_5.jpg")
        );
        assert!(parse_tralbum_art_url("<html>no art</html>").is_none());
    }

    #[test]
    fn release_year_read_from_tralbum_current_blob() {
        let page = r#"<div data-tralbum="{&quot;current&quot;:{&quot;title&quot;:&quot;NOISE AS A FORM OF EXPRESSION VOL.4&quot;,&quot;release_date&quot;:&quot;28 Sep 2026 18:26:00 GMT&quot;,&quot;id&quot;:2426189157},&quot;trackinfo&quot;:[]}"></div>"#;
        assert_eq!(parse_tralbum_release_year(page).as_deref(), Some("2026"));
        assert!(parse_tralbum_release_year("<html>no tralbum</html>").is_none());
        let no_year =
            r#"<div data-tralbum="{&quot;current&quot;:{&quot;release_date&quot;:&quot;soon&quot;}}"></div>"#;
        assert!(parse_tralbum_release_year(no_year).is_none());
    }

    #[test]
    fn album_artist_read_from_tralbum_current_blob() {
        // anavenus.bandcamp.com shape: current.artist present, per-track
        // artist null. (The literal page also nests `artist` objects after
        // current; the parser only scans the current..trackinfo slice.)
        let page = r#"<div data-tralbum="{&quot;current&quot;:{&quot;title&quot;:&quot;Anal Seppuku&quot;,&quot;artist&quot;:&quot;Ana Venus&quot;,&quot;release_date&quot;:&quot;01 Jan 2014 00:00:00 GMT&quot;},&quot;trackinfo&quot;:[{&quot;artist&quot;:null,&quot;title&quot;:&quot;Death By Cumshots&quot;}]}"></div>"#;
        assert_eq!(parse_tralbum_artist(page).as_deref(), Some("Ana Venus"));
        assert!(parse_tralbum_artist("<html>challenge</html>").is_none());
        let no_artist = r#"<div data-tralbum="{&quot;current&quot;:{&quot;title&quot;:&quot;X&quot;},&quot;trackinfo&quot;:[]}"></div>"#;
        assert!(parse_tralbum_artist(no_artist).is_none());
        let blank = r#"<div data-tralbum="{&quot;current&quot;:{&quot;artist&quot;:&quot;&quot;},&quot;trackinfo&quot;:[]}"></div>"#;
        assert!(parse_tralbum_artist(blank).is_none());
    }

    #[test]
    fn merge_metadata_matches_by_path_not_position() {
        let mut entries = vec![
            BandcampTrackEntry {
                url: "https://dramarecorder.bandcamp.com/track/temper-wrecked".to_string(),
                title: "Flesh-Control - Temper Wrecked".to_string(),
                duration_secs: 0.0,
                uploader: String::new(),
                album: None,
                track: None,
                track_no: None,
                artist: None,
                cover_url: None,
                year: None,
            },
            BandcampTrackEntry {
                url: "https://dramarecorder.bandcamp.com/track/neutralize".to_string(),
                title: "Boredom Knife - Neutralize".to_string(),
                duration_secs: 0.0,
                uploader: String::new(),
                album: None,
                track: None,
                track_no: None,
                artist: None,
                cover_url: None,
                year: None,
            },
        ];
        let tracks = parse_tralbum_tracks(TRALBUM_PAGE).expect("trackinfo present");
        assert_eq!(merge_tralbum_metadata(&mut entries, &tracks, None), 2);
        assert!((entries[0].duration_secs - 364.308).abs() < f64::EPSILON);
        assert!((entries[1].duration_secs - 243.435).abs() < f64::EPSILON);
        assert_eq!(entries[0].track_no.as_deref(), Some("2"));
        assert_eq!(entries[1].track_no.as_deref(), Some("1"));
        // Per-track tralbum artist is carried through.
        assert_eq!(entries[0].artist.as_deref(), Some("Flesh-Control"));
        assert_eq!(entries[1].artist.as_deref(), Some("Boredom Knife"));
    }

    #[test]
    fn merge_metadata_fills_artist_from_album_level_when_track_artist_null() {
        // anavenus.bandcamp.com/album/anal-seppuku: single-band page, every
        // per-track artist is null, but `current.artist` names the band.
        let mut entries = vec![BandcampTrackEntry {
            url: "https://anavenus.bandcamp.com/track/death-by-cumshots".to_string(),
            title: "Death By Cumshots".to_string(),
            duration_secs: 0.0,
            uploader: String::new(),
            album: Some("Anal Seppuku".to_string()),
            track: None,
            track_no: None,
            artist: None,
            cover_url: None,
            year: None,
        }];
        let tracks = vec![TralbumTrack {
            title: "Death By Cumshots".to_string(),
            artist: String::new(), // null in data-tralbum
            duration_secs: 131.805,
            url_path: Some("/track/death-by-cumshots".to_string()),
            track_num: Some(1),
        }];
        assert_eq!(
            merge_tralbum_metadata(&mut entries, &tracks, Some("Ana Venus")),
            1
        );
        assert_eq!(entries[0].artist.as_deref(), Some("Ana Venus"));
    }

    #[test]
    fn merge_metadata_skips_split_roster_artist() {
        // brutalizerecs 5-way split: tralbum per-track artist is null for
        // every track and `current.artist` is the full roster joined by " X ".
        // That roster must NOT become each track's artist; the title prefix
        // ("KKAAOOSS - ...") carries the real per-track artist.
        let roster = "D.O.M. X ALMA SEBOSA X KKAAOOSS X SINUSRORIS X ROUBE SUA VIDA";
        let mut entries = vec![BandcampTrackEntry {
            url: "https://brutalizerecs.bandcamp.com/track/a-nice-day-for-die-remix-i".to_string(),
            title: "KKAAOOSS - A NICE DAY FOR DIE (REMIX I)".to_string(),
            duration_secs: 0.0,
            uploader: String::new(),
            album: Some(format!("{} - \"Destroy Fascist Police\" - 5 Way Split", roster)),
            track: None,
            track_no: None,
            artist: None,
            cover_url: None,
            year: None,
        }];
        let tracks = vec![TralbumTrack {
            title: "KKAAOOSS - A NICE DAY FOR DIE (REMIX I)".to_string(),
            artist: String::new(), // null in data-tralbum
            duration_secs: 100.0,
            url_path: Some("/track/a-nice-day-for-die-remix-i".to_string()),
            track_num: Some(1),
        }];
        merge_tralbum_metadata(&mut entries, &tracks, Some(roster));
        // Roster rejected → artist stays None so title-prefix resolution wins.
        assert_eq!(entries[0].artist, None);

        // End-to-end: title prefix yields the per-track artist, not the roster.
        let resolved = resolve_bandcamp_metadata(
            &entries[0].title,
            "",
            entries[0].album.as_deref(),
            None,
            entries[0].artist.as_deref(),
        );
        assert_eq!(resolved.artist, "KKAAOOSS");
        assert_eq!(resolved.title, "A NICE DAY FOR DIE (REMIX I)");
    }

    #[test]
    fn five_way_split_all_titles_split_at_first_separator_only() {
        // Exact flat-playlist titles from the brutalizerecs 5-way split.
        // Several titles contain INNER " - " segments ("(REEE - MIX 3)",
        // "D.O.M. - - LIVE...", "Igualitarismo da tortura - Manifesto...");
        // only the FIRST separator is the artist boundary.
        let roster = "D.O.M. X ALMA SEBOSA X KKAAOOSS X SINUSRORIS X ROUBE SUA VIDA";
        let album = format!("{} - \"Destroy Fascist Police\" - 5 Way Split", roster);
        let cases: &[(&str, &str, &str)] = &[
            (
                "KKAAOOSS - A NICE DAY FOR DIE (REMIX I)",
                "KKAAOOSS",
                "A NICE DAY FOR DIE (REMIX I)",
            ),
            (
                "KKAAOOSS - A NICE DAY FOR DIE (REE-MIX 2)",
                "KKAAOOSS",
                "A NICE DAY FOR DIE (REE-MIX 2)",
            ),
            (
                "KKAAOOSS - A NICE DAY FOR DIE (REEE - MIX 3)",
                "KKAAOOSS",
                "A NICE DAY FOR DIE (REEE - MIX 3)",
            ),
            (
                "4KKAAOOSS - A NICE DAY FOR DIE (REEEE - MIX 4)",
                "4KKAAOOSS",
                "A NICE DAY FOR DIE (REEEE - MIX 4)",
            ),
            ("SINUSRORIS - Eles que Morram", "SINUSRORIS", "Eles que Morram"),
            (
                "ROUBESUAVIDA - Gravação live in Birigui-sp House of Noise 16\\5\\26",
                "ROUBESUAVIDA",
                "Gravação live in Birigui-sp House of Noise 16\\5\\26",
            ),
            (
                "D.O.M. - - LIVE CAFUNDÓ - ANEDONIA NOISE FEST - 20.09.2026",
                "D.O.M.",
                "- LIVE CAFUNDÓ - ANEDONIA NOISE FEST - 20.09.2026",
            ),
            (
                "D.O.M. - Igualitarismo da tortura - Manifesto Anti-Facho",
                "D.O.M.",
                "Igualitarismo da tortura - Manifesto Anti-Facho",
            ),
            (
                "ALMA SEBOSA feat. MÁGOA - LIVE CAFUNDÓ - ANEDONIA NOISE FEST - 20.09.2026",
                "ALMA SEBOSA feat. MÁGOA",
                "LIVE CAFUNDÓ - ANEDONIA NOISE FEST - 20.09.2026",
            ),
        ];
        for (raw, want_artist, want_title) in cases {
            let r = resolve_bandcamp_metadata(raw, "brutalizerecs", Some(&album), None, None);
            assert_eq!(r.artist, *want_artist, "artist for {raw:?}");
            assert_eq!(r.title, *want_title, "title for {raw:?}");
        }
    }

    #[test]
    fn resolve_split_album_prefix_does_not_beat_title_artist() {
        // Even when the album name leads with the roster, the per-track title
        // prefix must win as the artist.
        let roster = "D.O.M. X ALMA SEBOSA X KKAAOOSS X SINUSRORIS X ROUBE SUA VIDA";
        let album = format!("{} - \"Destroy Fascist Police\" - 5 Way Split", roster);
        let resolved = resolve_bandcamp_metadata(
            "SINUSRORIS - Eles que Morram",
            "",
            Some(&album),
            None,
            None,
        );
        assert_eq!(resolved.artist, "SINUSRORIS");
        assert_eq!(resolved.title, "Eles que Morram");
        // The roster album name is kept intact (no " - " prefix strip of artist).
        assert!(resolved.album.unwrap().contains("5 Way Split"));
    }

    #[test]
    fn single_slash_roster_also_rejected_but_plain_slash_band_kept() {
        assert!(is_multi_artist_roster("Alma Sebosa / Obscuruus HarshNoise / D.O.M. / Interzona"));
        assert!(is_multi_artist_roster("D.O.M. X ALMA SEBOSA X KKAAOOSS"));
        // A band whose name contains "/" but no spaced separator stays valid.
        assert!(!is_multi_artist_roster("Ana Venus"));
        assert!(!is_multi_artist_roster("AC/DC"));
    }

    #[test]
    fn merge_metadata_fills_track_num_when_duration_present() {
        let mut entries = vec![BandcampTrackEntry {
            url: "https://dramarecorder.bandcamp.com/track/neutralize".to_string(),
            title: "Boredom Knife - Neutralize".to_string(),
            duration_secs: 243.0,
            uploader: String::new(),
            album: None,
            track: None,
            track_no: None,
            artist: None,
            cover_url: None,
            year: None,
        }];
        let tracks = parse_tralbum_tracks(TRALBUM_PAGE).expect("trackinfo present");
        assert_eq!(merge_tralbum_metadata(&mut entries, &tracks, None), 1);
        assert!((entries[0].duration_secs - 243.0).abs() < f64::EPSILON);
        assert_eq!(entries[0].track_no.as_deref(), Some("1"));
    }

    #[test]
    fn merge_metadata_keeps_existing_duration() {
        let mut entries = vec![BandcampTrackEntry {
            url: "https://dramarecorder.bandcamp.com/track/neutralize".to_string(),
            title: "Boredom Knife - Neutralize".to_string(),
            duration_secs: 10.0,
            uploader: String::new(),
            album: None,
            track: None,
            track_no: None,
            artist: None,
            cover_url: None,
            year: None,
        }];
        let tracks = parse_tralbum_tracks(TRALBUM_PAGE).expect("trackinfo present");
        assert_eq!(merge_tralbum_metadata(&mut entries, &tracks, None), 1);
        assert!((entries[0].duration_secs - 10.0).abs() < f64::EPSILON);
        assert_eq!(entries[0].track_no.as_deref(), Some("1"));
    }

    #[test]
    fn merge_metadata_noop_when_nothing_missing() {
        let mut entries = vec![BandcampTrackEntry {
            url: "https://dramarecorder.bandcamp.com/track/neutralize".to_string(),
            title: "Boredom Knife - Neutralize".to_string(),
            duration_secs: 243.0,
            uploader: String::new(),
            album: None,
            track: Some("1".to_string()),
            track_no: Some("1".to_string()),
            artist: Some("Boredom Knife".to_string()),
            cover_url: None,
            year: None,
        }];
        let tracks = parse_tralbum_tracks(TRALBUM_PAGE).expect("trackinfo present");
        assert_eq!(merge_tralbum_metadata(&mut entries, &tracks, None), 0);
    }

    #[test]
    fn album_entries_take_album_and_uploader_from_playlist_keys() {
        let line = r#"{"url":"https://dramarecorder.bandcamp.com/track/stau","title":"Blaske Hill - Stau","playlist_title":"Noise As A Form Of Expression Vol. 4","playlist_uploader":"Drama Recorder","playlist_autonumber":"3","playlist_index":2}"#;
        let entries = parse_bandcamp_album_entries(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Blaske Hill - Stau");
        assert_eq!(entries[0].uploader, "Drama Recorder");
        assert_eq!(
            entries[0].album.as_deref(),
            Some("Noise As A Form Of Expression Vol. 4")
        );
        assert_eq!(entries[0].track.as_deref(), Some("3"));
    }

    #[test]
    fn album_entries_still_fall_back_to_per_item_keys() {
        let line = r#"{"url":"https://x.bandcamp.com/track/y","title":"A - B","album":"Real Album","uploader":"Real Uploader","track":"7"}"#;
        let entries = parse_bandcamp_album_entries(line);
        assert_eq!(entries[0].album.as_deref(), Some("Real Album"));
        assert_eq!(entries[0].uploader, "Real Uploader");
        assert_eq!(entries[0].track.as_deref(), Some("7"));
    }

    #[test]
    fn is_bandcamp_url_accepts_artist_subdomains() {
        assert!(is_bandcamp_url("https://domnoise.bandcamp.com/album/x"));
        assert!(is_bandcamp_url("https://some-artist.bandcamp.com/track/y"));
        assert!(is_bandcamp_url("https://a.bandcamp.com/music"));
        assert!(is_bandcamp_url("http://x.bandcamp.com/"));
    }

    #[test]
    fn is_bandcamp_url_rejects_main_site_and_non_bandcamp() {
        assert!(!is_bandcamp_url("https://bandcamp.com"));
        assert!(!is_bandcamp_url("https://bandcamp.com/?show=1"));
        assert!(!is_bandcamp_url("https://youtu.be/abc"));
        assert!(!is_bandcamp_url("https://example.com/album/x"));
        assert!(!is_bandcamp_url(""));
        assert!(!is_bandcamp_url("not a url"));
        assert!(!is_bandcamp_url("domnoise.bandcamp.com"));
    }

    #[test]
    fn normalize_strips_fbclid_query_fragment_and_trailing_slash() {
        assert_eq!(
            normalize_bandcamp_url(DOMNOISE_ALBUM_WITH_FBCLID),
            "https://domnoise.bandcamp.com/album/diariamente-obriga-o-maltrata"
        );
        assert_eq!(
            normalize_bandcamp_url("https://domnoise.bandcamp.com/album/x?utm_source=fb"),
            "https://domnoise.bandcamp.com/album/x"
        );
        assert_eq!(
            normalize_bandcamp_url("https://domnoise.bandcamp.com/album/x#frag"),
            "https://domnoise.bandcamp.com/album/x"
        );
        assert_eq!(
            normalize_bandcamp_url("https://domnoise.bandcamp.com/album/x/"),
            "https://domnoise.bandcamp.com/album/x"
        );
    }

    #[test]
    fn normalize_leaves_non_bandcamp_untouched() {
        assert_eq!(normalize_bandcamp_url("https://youtu.be/abc"), "https://youtu.be/abc");
        assert_eq!(normalize_bandcamp_url("not a url"), "not a url");
    }

    #[test]
    fn bandcamp_kind_classifies_paths() {
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com/track/x"), Some(BandcampKind::Track));
        assert_eq!(
            bandcamp_kind("https://domnoise.bandcamp.com/album/diariamente-obriga-o-maltrata"),
            Some(BandcampKind::Album)
        );
        assert_eq!(
            bandcamp_kind(DOMNOISE_ALBUM_WITH_FBCLID),
            Some(BandcampKind::Album)
        );
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com/"), Some(BandcampKind::Discography));
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com"), Some(BandcampKind::Discography));
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com/music"), Some(BandcampKind::Discography));
    }

    #[test]
    fn bandcamp_kind_rejects_garbage() {
        assert_eq!(bandcamp_kind("https://bandcamp.com/album/x"), None);
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com/xyz"), None);
        assert_eq!(bandcamp_kind("https://domnoise.bandcamp.com/track"), None);
        assert_eq!(bandcamp_kind("https://youtu.be/abc"), None);
        assert_eq!(bandcamp_kind(""), None);
    }

    #[test]
    fn parse_bandcamp_album_entries_collects_track_urls() {
        let stdout = r#"{"_type":"url","url":"https://domnoise.bandcamp.com/track/one","title":"One","duration":120.0,"uploader":"Artist"}
{"_type":"url","url":"https://domnoise.bandcamp.com/track/two","title":"Two","duration":180.0,"uploader":"Artist"}
{"_type":"playlist","title":"Nested album","entries":[]}
{"_type":"url","url":"https://domnoise.bandcamp.com/track/three","duration":200.0,"uploader":"Artist"}
"#;
        let entries = parse_bandcamp_album_entries(stdout);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].url, "https://domnoise.bandcamp.com/track/one");
        assert_eq!(entries[0].title, "One");
        assert_eq!(entries[0].duration_secs, 120.0);
        assert_eq!(entries[0].uploader, "Artist");
        assert_eq!(entries[1].url, "https://domnoise.bandcamp.com/track/two");
        assert_eq!(entries[2].url, "https://domnoise.bandcamp.com/track/three");
    }

    #[test]
    fn parse_bandcamp_album_entries_handles_empty_and_junk() {
        assert!(parse_bandcamp_album_entries("").is_empty());
        assert!(parse_bandcamp_album_entries("\n\n").is_empty());
        assert!(parse_bandcamp_album_entries("this is not json").is_empty());
        let only_playlist = r#"{"_type":"playlist","title":"Nested","entries":[]}"#;
        assert!(parse_bandcamp_album_entries(only_playlist).is_empty());
    }

    #[test]
    fn resolve_artist_page_uses_title_prefix() {
        // domnoise case: artist owns the page, title self-prefixes
        let m = resolve_bandcamp_metadata(
            "D.O.M. - Do Suor Do Teu Rosto Comerás O Pão (Versão Modificada)",
            "D.O.M.",
            Some("Diariamente Obrigação Maltrata"),
            Some("Do Suor Do Teu Rosto Comerás O Pão (Versão Modificada)"),
            None,
        );
        assert_eq!(m.artist, "D.O.M.");
        assert_eq!(m.title, "Do Suor Do Teu Rosto Comerás O Pão (Versão Modificada)");
        assert_eq!(m.album.as_deref(), Some("Diariamente Obrigação Maltrata"));
    }

    #[test]
    fn resolve_label_hosted_album_uses_album_prefix() {
        // SPHC case: label owns the page, real artist is album-name prefix
        let m = resolve_bandcamp_metadata(
            "SPHC Records - side A (50 songs)",
            "SPHC Records",
            Some("Putrefação Humana - Colhendo Desespero EP (SPHC)"),
            Some("side A (50 songs)"),
            None,
        );
        assert_eq!(m.artist, "Putrefação Humana");
        assert_eq!(m.title, "side A (50 songs)");
        assert_eq!(m.album.as_deref(), Some("Colhendo Desespero EP (SPHC)"));
    }

    #[test]
    fn resolve_label_hosted_without_track_field() {
        // No track field: title falls back to raw title with owner prefix stripped
        let m = resolve_bandcamp_metadata(
            "SPHC Records - side A (50 songs)",
            "SPHC Records",
            Some("Putrefação Humana - Colhendo Desespero EP (SPHC)"),
            None,
            None,
        );
        assert_eq!(m.artist, "Putrefação Humana");
        assert_eq!(m.title, "side A (50 songs)");
        assert_eq!(m.album.as_deref(), Some("Colhendo Desespero EP (SPHC)"));
    }

    #[test]
    fn resolve_no_prefixes_falls_back_to_uploader() {
        let m = resolve_bandcamp_metadata(
            "Just A Song",
            "Some Artist",
            None,
            None,
            None,
        );
        assert_eq!(m.artist, "Some Artist");
        assert_eq!(m.title, "Just A Song");
        assert_eq!(m.album, None);
    }

    #[test]
    fn resolve_empty_track_ignored() {
        let m = resolve_bandcamp_metadata(
            "Artist - Song",
            "Artist",
            Some("Album Name"),
            Some("   "),
            None,
        );
        assert_eq!(m.artist, "Artist");
        assert_eq!(m.title, "Song");
        assert_eq!(m.album.as_deref(), Some("Album Name"));
    }

    // anavenus.bandcamp.com/album/anal-seppuku: null uploader, null per-track
    // artist, no title prefix — tralbum album-level artist is the only source.
    #[test]
    fn resolve_tralbum_artist_wins_over_empty_uploader() {
        let m = resolve_bandcamp_metadata(
            "Death By Cumshots",
            "",
            Some("Anal Seppuku"),
            None,
            Some("Ana Venus"),
        );
        assert_eq!(m.artist, "Ana Venus");
        assert_eq!(m.title, "Death By Cumshots");
        assert_eq!(m.album.as_deref(), Some("Anal Seppuku"));
    }

    #[test]
    fn resolve_tralbum_artist_beats_prefix_heuristics() {
        // Bandcamp ground truth wins when prefix guessing would differ.
        let m = resolve_bandcamp_metadata(
            "Some Label - Song",
            "Some Label",
            Some("Real Artist - Album (Label)"),
            None,
            Some("Real Artist"),
        );
        assert_eq!(m.artist, "Real Artist");
    }

    #[test]
    fn resolve_blank_tralbum_artist_falls_through_to_prefix_rules() {
        let m = resolve_bandcamp_metadata(
            "D.O.M. - Song",
            "D.O.M.",
            Some("Album"),
            None,
            Some("   "),
        );
        assert_eq!(m.artist, "D.O.M.");
    }

    fn bcsearch_response() -> serde_json::Value {
        serde_json::json!({
            "auto": {
                "results": [
                    {"type": "t", "name": "Song One", "band_name": "Artist A", "item_url_path": "https://artista.bandcamp.com/track/song-one", "album_name": "EP One"},
                    {"type": "a", "name": "Album One", "band_name": "Artist B", "item_url_path": "https://artistb.bandcamp.com/album/album-one"},
                    {"type": "b", "name": "Artist C", "band_name": "Artist C", "item_url_path": "https://artistc.bandcamp.com"},
                    {"type": "x", "name": "Unknown", "band_name": "???", "item_url_path": "https://example.com"},
                    {"type": "t", "name": "No URL", "band_name": "???", "item_url_path": ""}
                ]
            }
        })
    }

    #[test]
    fn parse_all_types_returns_track_album_band() {
        let results = parse_bandcamp_search_results_all_types(&bcsearch_response());
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].type_, BandcampType::Track);
        assert_eq!(results[0].name, "Song One");
        assert_eq!(results[0].band_name, "Artist A");
        assert_eq!(results[0].url, "https://artista.bandcamp.com/track/song-one");
        assert_eq!(results[0].album_name.as_deref(), Some("EP One"));
        assert_eq!(results[1].type_, BandcampType::Album);
        assert_eq!(results[1].name, "Album One");
        assert_eq!(results[1].album_name, None);
        assert_eq!(results[2].type_, BandcampType::Band);
        assert_eq!(results[2].name, "Artist C");
        assert_eq!(results[2].band_name, "Artist C");
    }

    #[test]
    fn parse_all_types_skips_unknown_and_empty_url() {
        let results = parse_bandcamp_search_results_all_types(&bcsearch_response());
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| !r.url.is_empty()));
    }

    #[test]
    fn parse_all_types_handles_empty_response() {
        assert!(parse_bandcamp_search_results_all_types(&serde_json::json!({})).is_empty());
        assert!(parse_bandcamp_search_results_all_types(&serde_json::json!({"auto": {}})).is_empty());
        assert!(parse_bandcamp_search_results_all_types(&serde_json::json!({"auto": {"results": []}})).is_empty());
    }

    /// Live `search_filter:"b"` response: bands carry `item_url_root` and no
    /// `band_name`, unlike tracks/albums which carry `item_url_path`/`band_name`.
    #[test]
    fn parse_band_result_uses_url_root_and_defaults_band_name() {
        let live = serde_json::json!({
            "auto": {"results": [{
                "type": "b",
                "id": 2106188119,
                "art_id": null,
                "img_id": 11372027,
                "name": "VOMITOR",
                "item_url_root": "https://vomitor-australia.bandcamp.com",
                "location": "Brisbane, Australia",
                "is_label": false,
                "tag_names": ["Metal", "thrash", "death metal"],
                "img": "https://f4.bcbits.com/img/0011372027_23.jpg",
                "genre_name": "Metal",
                "stat_params": "search_item_id=2106188119&search_item_type=b"
            }]}
        });
        let results = parse_bandcamp_search_results_all_types(&live);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].type_, BandcampType::Band);
        assert_eq!(results[0].name, "VOMITOR");
        assert_eq!(results[0].url, "https://vomitor-australia.bandcamp.com");
        assert_eq!(results[0].band_name, "VOMITOR");
        assert!(is_bandcamp_url(&results[0].url));
    }

    fn bc_result(name: &str, band: &str) -> BandcampSearchResult {
        BandcampSearchResult {
            type_: BandcampType::Track,
            name: name.to_string(),
            band_name: band.to_string(),
            url: format!("https://{}.bandcamp.com/track/x", band.to_lowercase()),
            album_name: None,
        }
    }

    #[test]
    fn match_accepts_exact_artist_and_title() {
        let r = bc_result("Fake Plastic Trees", "Radiohead");
        assert!(bandcamp_result_matches("Radiohead", "Fake Plastic Trees", &r));
    }

    #[test]
    fn match_ignores_case_and_punctuation() {
        let r = bc_result("Fake Plastic Trees (Remastered)", "Radiohead");
        assert!(bandcamp_result_matches("radiohead", "fake plastic trees", &r));
    }

    #[test]
    fn match_accepts_artist_credit_variants() {
        assert!(bandcamp_result_matches(
            "Various Artists",
            "X",
            &bc_result("X", "V/A")
        ));
        assert!(bandcamp_result_matches(
            "Artist feat. Someone",
            "X",
            &bc_result("X", "Artist")
        ));
    }

    #[test]
    fn match_rejects_wrong_artist() {
        let r = bc_result("Fake Plastic Trees", "Radiohead");
        assert!(!bandcamp_result_matches("Nirvana", "Fake Plastic Trees", &r));
    }

    #[test]
    fn match_rejects_wrong_title() {
        let r = bc_result("Creep", "Radiohead");
        assert!(!bandcamp_result_matches("Radiohead", "Fake Plastic Trees", &r));
    }

    #[test]
    fn match_rejects_empty_candidate_fields() {
        assert!(!bandcamp_result_matches(
            "Radiohead",
            "Fake Plastic Trees",
            &bc_result("", "Radiohead")
        ));
        assert!(!bandcamp_result_matches(
            "Radiohead",
            "Fake Plastic Trees",
            &bc_result("Fake Plastic Trees", "")
        ));
    }

    #[test]
    fn empty_request_field_skips_that_check() {
        let r = bc_result("Cut You Into Pieces", "Bands of Mice");
        assert!(bandcamp_result_matches("Bands of Mice", "", &r));
        assert!(bandcamp_result_matches("", "Cut You Into Pieces", &r));
    }

    #[test]
    fn title_artist_matches_accepts_exact_pair() {
        assert!(title_artist_matches(
            "Boredom Knife",
            "Neutralize",
            "Boredom Knife",
            "Neutralize"
        ));
    }

    #[test]
    fn title_artist_matches_ignores_case_and_punctuation() {
        assert!(title_artist_matches(
            "BOREDOM knife",
            "neutral-ize",
            "Boredom Knife",
            "Neutralize"
        ));
    }

    #[test]
    fn title_artist_matches_accepts_credit_variants() {
        assert!(title_artist_matches("V/A", "Neutralize", "V/A", "Neutralize"));
        assert!(title_artist_matches(
            "Artist feat. Someone",
            "Song",
            "Artist",
            "Song"
        ));
    }

    // The live e2e that motivated this check: YouTube fuzzy search answered
    // "FAEX - strench of the chaos 3" with a completely unrelated video, and
    // taking that unverified hit is what silently swallowed every
    // Bandcamp-only recommendation.
    #[test]
    fn title_artist_matches_rejects_the_unrelated_youtube_hit() {
        assert!(!title_artist_matches(
            "FAEX",
            "strench of the chaos 3",
            "xxxcharacter",
            "CHAOS IN THE WORLD"
        ));
    }

    #[test]
    fn title_artist_matches_rejects_wrong_artist_and_wrong_title() {
        assert!(!title_artist_matches(
            "Boredom Knife",
            "Neutralize",
            "Paranoised",
            "Neutralize"
        ));
        assert!(!title_artist_matches(
            "Boredom Knife",
            "Neutralize",
            "Boredom Knife",
            "Riding a wild dragon"
        ));
    }

    #[test]
    fn title_artist_matches_rejects_empty_candidate_fields() {
        assert!(!title_artist_matches("Boredom Knife", "Neutralize", "", "Neutralize"));
        assert!(!title_artist_matches("Boredom Knife", "Neutralize", "Boredom Knife", ""));
    }

    #[test]
    fn title_artist_matches_skips_only_empty_request_fields() {
        assert!(title_artist_matches("", "Neutralize", "Anyone", "Neutralize"));
        assert!(title_artist_matches("Boredom Knife", "", "Boredom Knife", "Anything"));
        assert!(!title_artist_matches("", "Neutralize", "Anyone", "Something Else"));
    }

    #[test]
    fn a_track_hit_whose_url_is_not_a_track_is_not_queueable() {
        let mut r = bc_result("Neutralize", "Boredom Knife");
        r.url = "https://dramarecorder.bandcamp.com/album/noise-as-a-form-of-expression-vol-4"
            .to_string();
        assert_eq!(r.type_, BandcampType::Track);
        assert_ne!(bandcamp_kind(&r.url), Some(BandcampKind::Track));
    }

    #[test]
    fn a_track_hit_with_a_non_bandcamp_url_is_not_queueable() {
        let mut r = bc_result("Neutralize", "Boredom Knife");
        r.url = "https://example.com/track/neutralize".to_string();
        assert_ne!(bandcamp_kind(&r.url), Some(BandcampKind::Track));
    }
}