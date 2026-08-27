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
    // RYM genre data fallback - covers 5,977+ genres from RateYourMusic hierarchy.
    // Skip for strings containing '/' - normalize_genres handles split separately.
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

/// Expand genres by adding parent genres from the SQLite genre database hierarchy.
pub fn expand_parent_genres(genres: &mut Vec<String>, styles: &mut Vec<String>) {
    let expanded = genre_db_sqlite::GenreDb::global().expand_parent_genres(genres);
    *genres = expanded;
    let expanded_styles = genre_db_sqlite::GenreDb::global().expand_parent_genres(styles);
    *styles = expanded_styles;
}

/// Attach RYM descriptor/subgenre enrichment derived purely from local taxonomy.
///
/// For each tag in `genres` then `styles`:
/// - `find_genre(tag)` -> if found, the most-specific leaf (last path segment) is
///   pushed to `subgenres` and `(tag, full_rym_path)` to `genre_paths`.
/// - if not found, the original tag is kept in both `subgenres` and `genre_paths`.
///
/// For each tag and each resolved leaf, `find_descriptor` is checked and any match
/// is appended to `descriptors` (deduped case-insensitively).
///
/// `genres`/`styles` are never mutated - provider-weighted output is preserved.
pub fn attach_rym_enrichment(
    genres: &[String],
    styles: &[String],
) -> (Vec<String>, Vec<(String, String)>, Vec<String>) {
    let db = genre_db_sqlite::GenreDb::global();
    let mut subgenres: Vec<String> = Vec::new();
    let mut genre_paths: Vec<(String, String)> = Vec::new();
    // Resolved leaves and original tags feed descriptor matching.
    let mut descriptor_candidates: Vec<String> = Vec::new();

    for tag in genres.iter().chain(styles.iter()) {
        descriptor_candidates.push(tag.clone());
        match db.find_genre(tag) {
            Some(g) => {
                let leaf = g
                    .path
                    .as_ref()
                    .and_then(|p| p.last().cloned())
                    .unwrap_or_else(|| g.name.clone());
                subgenres.push(leaf.clone());
                let full_path = g
                    .path
                    .as_ref()
                    .map(|p| p.join(" / "))
                    .unwrap_or_else(|| g.name.clone());
                genre_paths.push((tag.clone(), full_path));
                descriptor_candidates.push(leaf);
            }
            None => {
                subgenres.push(tag.clone());
                genre_paths.push((tag.clone(), tag.clone()));
            }
        }
    }

    let mut descriptors: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for candidate in &descriptor_candidates {
        if let Some(d) = db.find_descriptor(candidate) {
            let lower = d.name.to_lowercase();
            if seen.insert(lower) {
                descriptors.push(d.name);
            }
        }
    }

    (subgenres, genre_paths, descriptors)
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

    #[test]
    fn test_attach_rym_known_leaf_and_path() {
        let db = genre_db_sqlite::GenreDb::global();
        let known = db.find_genre("Heavy metal");
        if let Some(g) = known {
            let leaf = g
                .path
                .as_ref()
                .and_then(|p| p.last().cloned())
                .unwrap_or_else(|| g.name.clone());
            let (subgenres, genre_paths, _descriptors) =
                attach_rym_enrichment(&["Heavy metal".to_string()], &[]);
            assert!(
                subgenres.contains(&leaf),
                "expected leaf {:?} in subgenres {:?}",
                leaf,
                subgenres
            );
            assert!(
                genre_paths
                    .iter()
                    .any(|(t, p)| t == "Heavy metal" && !p.is_empty()),
                "expected (Heavy metal, full_path) in genre_paths {:?}",
                genre_paths
            );
        } else {
            panic!("Heavy metal should exist in seeded genre db");
        }
    }

    #[test]
    fn test_attach_rym_unknown_tag_kept() {
        let tag = "Zzxqw bogus genre 9999".to_string();
        let (subgenres, genre_paths, _descriptors) =
            attach_rym_enrichment(&[tag.clone()], &[]);
        assert!(subgenres.contains(&tag));
        assert!(genre_paths
            .iter()
            .any(|(t, p)| t == &tag && p == &tag));
    }

    #[test]
    fn test_attach_rym_descriptor_attached() {
        let db = genre_db_sqlite::GenreDb::global();
        let desc = match db.all_descriptors().into_iter().next() {
            Some(d) => d,
            None => return,
        };
        let (_subgenres, _genre_paths, descriptors) =
            attach_rym_enrichment(&[desc.name.clone()], &[]);
        assert!(
            descriptors.iter().any(|x| x.eq_ignore_ascii_case(&desc.name)),
            "expected descriptor {:?} in {:?}",
            desc.name,
            descriptors
        );
    }

    #[test]
    fn test_attach_rym_no_descriptor_for_unknown() {
        let tag = "Zzxqw bogus desc 9999".to_string();
        let (_subgenres, _genre_paths, descriptors) =
            attach_rym_enrichment(&[tag.clone()], &[]);
        assert!(
            !descriptors.iter().any(|x| x.eq_ignore_ascii_case(&tag)),
            "unknown tag should yield no descriptor"
        );
    }

    #[test]
    fn test_attach_rym_does_not_mutate_inputs() {
        let genres = vec!["Heavy metal".to_string()];
        let styles = vec!["Thrash".to_string()];
        let before_g = genres.clone();
        let before_s = styles.clone();
        let _ = attach_rym_enrichment(&genres, &styles);
        assert_eq!(genres, before_g);
        assert_eq!(styles, before_s);
    }
}
