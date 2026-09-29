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

/// Parse yt-dlp `--flat-playlist --dump-json` output into the list of track
/// URLs it names.
///
/// Every line is one JSON object (flat-playlist mode). Collects each line's
/// `url` field, skipping playlist entries (`_type == "playlist"`) and empty
/// lines. Bandcamp album entries are plain track URLs.
pub fn parse_bandcamp_album_entries(stdout: &str) -> Vec<String> {
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
        if let Some(url) = value.get("url").and_then(|u| u.as_str()) {
            if !url.is_empty() {
                entries.push(url.to_string());
            }
        }
    }
    entries
}

fn url_to_host(url: &str) -> Option<&str> {
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
/// 1. album-name prefix when it differs from the page owner (label-hosted
///    albums, e.g. "Putrefação Humana - Colhendo Desespero EP (SPHC)")
/// 2. title prefix when it differs from the page owner
/// 3. title prefix (artist pages self-prefix, e.g. "D.O.M. - Song")
/// 4. uploader as last resort
///
/// Title prefers the `track` field (yt-dlp's definitive song name), else the
/// raw title with the owner/artist prefix stripped. Album keeps its name
/// with the resolved artist prefix removed.
pub fn resolve_bandcamp_metadata(
    raw_title: &str,
    uploader: &str,
    album: Option<&str>,
    track: Option<&str>,
) -> ResolvedBandcampMetadata {
    fn split_prefix(s: &str) -> Option<(&str, &str)> {
        s.split_once(" - ")
            .map(|(a, b)| (a.trim(), b.trim()))
            .filter(|(a, b)| !a.is_empty() && !b.is_empty())
    }
    let title_prefix = split_prefix(raw_title);
    let album_prefix = album.and_then(split_prefix);

    let artist = match (album_prefix, title_prefix) {
        (Some((ap, _)), _) if ap != uploader => ap.to_string(),
        (_, Some((tp, _))) if tp != uploader => tp.to_string(),
        (_, Some((tp, _))) => tp.to_string(),
        _ => uploader.to_string(),
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
        let stdout = r#"{"_type":"url","url":"https://domnoise.bandcamp.com/track/one","title":"One"}
{"_type":"url","url":"https://domnoise.bandcamp.com/track/two","title":"Two"}
{"_type":"playlist","title":"Nested album","entries":[]}
{"_type":"url","url":"https://domnoise.bandcamp.com/track/three"}
"#;
        let entries = parse_bandcamp_album_entries(stdout);
        assert_eq!(
            entries,
            vec![
                "https://domnoise.bandcamp.com/track/one".to_string(),
                "https://domnoise.bandcamp.com/track/two".to_string(),
                "https://domnoise.bandcamp.com/track/three".to_string(),
            ]
        );
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
        );
        assert_eq!(m.artist, "Artist");
        assert_eq!(m.title, "Song");
        assert_eq!(m.album.as_deref(), Some("Album Name"));
    }
}