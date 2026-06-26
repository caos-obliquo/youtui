use std::collections::HashMap;
use std::sync::OnceLock;

static GENRE_HIERARCHY: &str = include_str!("Enhanced genre hierarchy browser.txt");

fn build_genre_map() -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for line in GENRE_HIERARCHY.lines() {
        let trimmed = line.trim();
        // Lines ending with ::genre or ::album genre are actual genre entries
        let canonical = if let Some(name) = trimmed.strip_suffix("::genre") {
            name.trim().to_string()
        } else if let Some(name) = trimmed.strip_suffix("::album genre") {
            name.trim().to_string()
        } else {
            continue;
        };
        if canonical.is_empty() { continue; }
        // Index by lowercase form for case-insensitive matching
        map.entry(canonical.to_lowercase()).or_insert_with(|| canonical.clone());
        // Also index normalized form (strip " music" suffix)
        if let Some(stripped) = canonical.strip_suffix(" music") {
            if !stripped.is_empty() {
                map.entry(stripped.to_lowercase()).or_insert_with(|| canonical.clone());
            }
        }
    }
    // Add common Discogs style variants that map to canonical genres
    let discogs_overrides: &[(&str, &str)] = &[
        ("heavy metal", "Heavy metal"),
        ("thrash", "Thrash metal"),
        ("death", "Death metal"),
        ("black", "Black metal"),
        ("doom", "Doom metal"),
        ("drone", "Drone"),
        ("speed metal", "Speed metal"),
        ("power metal", "Power metal"),
        ("prog rock", "Progressive rock"),
        ("prog metal", "Progressive metal"),
        ("alt rock", "Alternative rock"),
        ("alt metal", "Alternative metal"),
        ("industrial", "Industrial"),
        ("electronic", "Electronic"),
        ("ambient", "Ambient"),
        ("hip hop", "Hip hop"),
        ("rnb", "R&B"),
        ("r&b", "R&B"),
        ("soul", "Soul"),
        ("funk", "Funk"),
        ("blues", "Blues"),
        ("jazz", "Jazz"),
        ("classical", "Classical"),
        ("folk", "Folk"),
        ("country", "Country"),
        ("punk", "Punk"),
        ("reggae", "Reggae"),
        ("ska", "Ska"),
        ("pop", "Pop"),
        ("rock", "Rock"),
        ("indie", "Indie"),
    ];
    for (key, val) in discogs_overrides {
        map.entry(key.to_string()).or_insert_with(|| val.to_string());
    }
    map
}

fn genre_map() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(build_genre_map)
}

/// Normalize a genre name using the MusicBee hierarchy.
/// Returns the canonical form if found, otherwise returns the original.
pub fn normalize_genre(name: &str) -> String {
    let lowered = name.to_lowercase().trim().to_string();
    let map = genre_map();
    // Exact match
    if let Some(canonical) = map.get(&lowered) {
        // Prefer shorter form if input ends with " music" and shorter exists
        if lowered.ends_with(" music") {
            let without_music = lowered.strip_suffix(" music").unwrap().trim().to_string();
            if let Some(shorter) = map.get(&without_music) {
                return shorter.clone();
            }
        }
        return canonical.clone();
    }
    // Match after stripping parenthetical qualifiers
    if let Some(paren) = lowered.find('(') {
        let base = lowered[..paren].trim().to_string();
        if let Some(canonical) = map.get(&base) {
            return format!("{} {}", canonical, &lowered[paren..]);
        }
    }
    // Match after stripping " music" suffix
    if let Some(stripped) = lowered.strip_suffix(" music") {
        if let Some(canonical) = map.get(stripped.trim()) {
            return canonical.clone();
        }
    }
    // Match after stripping trailing spaces/slashes
    let cleaned = lowered.trim_end_matches(&[' ', '/'] as &[_]).to_string();
    if let Some(canonical) = map.get(&cleaned) {
        return canonical.clone();
    }
    // RYM genre data fallback — covers 5,977+ genres from RateYourMusic hierarchy.
    // Skip for strings containing '/' — normalize_genres handles split separately.
    if !name.contains('/') {
        if let Some(rym_name) = rym_genre_data::normalize_style(name) {
            return rym_name.to_string();
        }
    }
    name.to_string()
}

/// Return all canonical genre names known to the hierarchy.
pub fn all_genres() -> Vec<String> {
    let map = genre_map();
    let mut genres: Vec<String> = map.values().cloned().collect();
    genres.sort();
    genres.dedup();
    genres
}

/// Check if a genre name is known to the hierarchy.
pub fn is_known_genre(name: &str) -> bool {
    let lowered = name.to_lowercase().trim().to_string();
    let map = genre_map();
    map.contains_key(&lowered)
        || map.contains_key(&lowered.trim_end_matches(&[' ', '/'] as &[_]).to_string())
}

/// Normalize all genres in a list, deduplicating and sorting.
pub fn normalize_genres(genres: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = genres.iter()
        .map(|g| normalize_genre(g))
        .collect();
    // Also add any genre that contains a '/' by splitting it
    let mut split: Vec<String> = Vec::new();
    for g in &normalized {
        if g.contains(" / ") {
            for part in g.split(" / ") {
                let n = normalize_genre(part.trim());
                if !n.is_empty() { split.push(n); }
            }
        } else if g.contains('/') {
            for part in g.split('/') {
                let n = normalize_genre(part.trim());
                if !n.is_empty() { split.push(n); }
            }
        }
    }
    normalized.extend(split);
    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_metal() {
        assert_eq!(normalize_genre("Heavy metal"), "Heavy metal");
        assert_eq!(normalize_genre("heavy metal"), "Heavy metal");
        assert_eq!(normalize_genre("Black Metal"), "Black metal");
        assert_eq!(normalize_genre("death metal"), "Death metal");
    }

    #[test]
    fn test_normalize_suffix() {
        assert_eq!(normalize_genre("Classical music"), "Classical");
        assert_eq!(normalize_genre("Electronic music"), "Electronic");
    }

    #[test]
    fn test_normalize_unknown() {
        assert_eq!(normalize_genre(""), "");
        assert_eq!(normalize_genre("Super obscure genre 3000"), "Super obscure genre 3000");
    }

    #[test]
    fn test_normalize_list() {
        let input = vec!["Heavy metal".to_string(), "Black metal".to_string(), "Heavy metal".to_string()];
        let result = normalize_genres(&input);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"Heavy metal".to_string()));
        assert!(result.contains(&"Black metal".to_string()));
    }

    #[test]
    fn test_split_slash_genres() {
        let input = vec!["Doom Metal / Drone".to_string()];
        let result = normalize_genres(&input);
        assert!(result.contains(&"Doom metal".to_string()) || result.contains(&"Doom Metal".to_string()));
        assert!(result.contains(&"Drone".to_string()));
    }

    #[test]
    fn test_is_known() {
        assert!(is_known_genre("Heavy metal"));
        assert!(is_known_genre("Black metal"));
        assert!(!is_known_genre("FakeGenre123"));
    }

    #[test]
    fn test_all_genres_loaded() {
        let all = all_genres();
        assert!(all.len() > 3000);
        assert!(all.contains(&"Heavy metal".to_string()));
        assert!(all.contains(&"Black metal".to_string()));
    }
}
