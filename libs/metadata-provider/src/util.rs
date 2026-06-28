pub fn norm_for_lfm(s: &str) -> String {
    let mut out: &str = s.trim();
    let out_owned = out.replace(" & ", " and ").replace("&", "and");
    out = &out_owned;

    let patterns = [
        "full album",
        "full lp",
        "full ep",
        "full-length album",
        "full length",
        "official music video",
        "official video",
        "official audio",
        "music video",
        "lyric video",
        "audio",
        " - single",
        " - ep",
        " - lp",
        " - full album",
        " - full ep",
    ];
    let mut result = out.to_string();
    for pat in &patterns {
        if let Some(pos) = result.to_lowercase().find(pat) {
            result = result[..pos].trim().to_string();
        }
    }
    out = &result;

    if let Some(pos) = out.find(" (") {
        out = out[..pos].trim();
    }

    if let Some(pos) = out.find(" [") {
        out = out[..pos].trim();
    }

    out.to_string()
}

pub fn extract_year(s: &str) -> Option<String> {
    s.split(|c: char| !c.is_ascii_digit())
        .find(|part| part.len() == 4)
        .map(|s| s.to_string())
}

pub fn extract_duration(v: &serde_json::Value) -> f64 {
    v.as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| v.as_f64())
        .unwrap_or(0.0)
}

pub fn parse_discogs_duration(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        parts[0].parse::<f64>().unwrap_or(0.0) * 60.0 + parts[1].parse::<f64>().unwrap_or(0.0)
    } else if parts.len() == 3 {
        parts[0].parse::<f64>().unwrap_or(0.0) * 3600.0
            + parts[1].parse::<f64>().unwrap_or(0.0) * 60.0
            + parts[2].parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

pub fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", c as u8));
            }
        }
    }
    out
}

pub fn discogs_limiter() -> &'static tokio::sync::Semaphore {
    static S: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    S.get_or_init(|| tokio::sync::Semaphore::new(1))
}

pub fn musicbrainz_limiter() -> &'static tokio::sync::Semaphore {
    static S: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    S.get_or_init(|| tokio::sync::Semaphore::new(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_for_lfm_strips_full_album() {
        assert_eq!(norm_for_lfm("Album Title FULL ALBUM"), "Album Title");
        assert_eq!(norm_for_lfm("Song Name (Full Album)"), "Song Name");
        assert_eq!(norm_for_lfm("Track - Single"), "Track");
    }

    #[test]
    fn norm_for_lfm_replaces_and() {
        assert_eq!(norm_for_lfm("Rock & Roll"), "Rock and Roll");
    }

    #[test]
    fn norm_for_lfm_strips_parenthesized() {
        assert_eq!(norm_for_lfm("Song (2021 - Death Metal)"), "Song");
        assert_eq!(norm_for_lfm("Track (2003)"), "Track");
    }

    #[test]
    fn norm_for_lfm_strips_bracketed() {
        assert_eq!(norm_for_lfm("Song [HD]"), "Song");
        assert_eq!(norm_for_lfm("Track [grind]"), "Track");
    }

    #[test]
    fn norm_for_lfm_preserves_normal() {
        assert_eq!(norm_for_lfm("Master of Puppets"), "Master of Puppets");
        assert_eq!(norm_for_lfm("Raining Blood"), "Raining Blood");
    }

    #[test]
    fn extract_year_various_formats() {
        assert_eq!(extract_year("11 Nov 2007"), Some("2007".to_string()));
        assert_eq!(extract_year("2007-11-11"), Some("2007".to_string()));
        assert_eq!(extract_year("2007"), Some("2007".to_string()));
        assert_eq!(extract_year("November 2007"), Some("2007".to_string()));
    }

    #[test]
    fn extract_year_none() {
        assert_eq!(extract_year("no year here"), None);
        assert_eq!(extract_year(""), None);
        assert_eq!(extract_year("99"), None);
    }
}
