/// RYM genre/descriptor data - parsed from RateYourMusic Hierarchy.txt
/// at compile time, indexed for fast runtime lookup.
use serde::Deserialize;

// ─── Data Structures ───

#[derive(Debug, Clone)]
pub struct Genre {
    pub name: String,
    pub path: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Descriptor {
    pub name: String,
    pub category: String,
    pub descriptor_type: String,
}

// ─── Parsed State (lazy init via OnceLock) ───

struct RymData {
    genres: Vec<Genre>,
    genres_by_name: std::collections::HashMap<String, usize>,
    descriptors: Vec<Descriptor>,
    descriptors_by_name: std::collections::HashMap<String, usize>,
}

static DATA: std::sync::OnceLock<RymData> = std::sync::OnceLock::new();

fn get_data() -> &'static RymData {
    DATA.get_or_init(|| {
        let raw = include_str!("../data/rym-hierarchy.txt");
        let descriptions = load_descriptions();
        parse_hierarchy(raw, &descriptions)
    })
}

// ─── Public API ───

/// All parsed genres with hierarchical paths.
pub fn all_genres() -> &'static [Genre] {
    &get_data().genres
}

/// All parsed descriptors with categories.
pub fn all_descriptors() -> &'static [Descriptor] {
    &get_data().descriptors
}

/// Find a genre by name (case-insensitive).
/// Returns the canonical RYM genre name with its hierarchy path.
pub fn find_genre(name: &str) -> Option<&'static Genre> {
    let key = name.to_lowercase();
    let data = get_data();
    data.genres_by_name.get(&key).map(|&i| &data.genres[i])
}

/// Normalize a free-text style/genre string to the canonical RYM name.
/// Returns Some(canonical_name) on match, None if no match found.
/// Matching rules:
///   1. Exact match (case-insensitive)
///   2. Substring match (free-text contains RYM name, or vice versa)
///   3. Trigram overlap >= 0.6
pub fn normalize_style(style: &str) -> Option<&'static str> {
    let style_lower = style.to_lowercase();
    let trimmed = style_lower.trim();
    if trimmed.is_empty() {
        return None;
    }

    let data = get_data();

    // 1. Exact match
    if let Some(&i) = data.genres_by_name.get(trimmed) {
        return Some(&data.genres[i].name);
    }

    // 2. Substring match: find best overlap
    let mut best: Option<(&Genre, f64)> = None;
    for genre in &data.genres {
        let gname = genre.name.to_lowercase();
        // Direct substring
        if gname.contains(trimmed) || trimmed.contains(&gname) {
            let score =
                trimmed.len().min(gname.len()) as f64 / trimmed.len().max(gname.len()) as f64;
            if best.as_ref().is_none_or(|(_, s)| score > *s) {
                best = Some((genre, score));
            }
            continue;
        }
        // Trigram overlap
        let overlap = trigram_overlap(trimmed, &gname);
        if overlap >= 0.6 && best.as_ref().is_none_or(|(_, s)| overlap > *s) {
            best = Some((genre, overlap));
        }
    }

    best.map(|(g, _)| &g.name as &str)
}

/// Find a descriptor by name (case-insensitive).
pub fn find_descriptor(name: &str) -> Option<&'static Descriptor> {
    let key = name.to_lowercase();
    let data = get_data();
    data.descriptors_by_name
        .get(&key)
        .map(|&i| &data.descriptors[i])
}

// ─── Descriptions loader ───

#[derive(Deserialize)]
struct DescriptionsIndex {
    genres: Vec<DescriptionEntry>,
}

#[derive(Deserialize)]
struct DescriptionEntry {
    name: String,
    description: String,
}

fn load_descriptions() -> std::collections::HashMap<String, String> {
    let json = include_str!("../data/rym-genre-descriptions.json");
    let index: DescriptionsIndex = match serde_json::from_str(json) {
        Ok(i) => i,
        Err(_) => return std::collections::HashMap::new(),
    };
    index
        .genres
        .into_iter()
        .map(|e| (e.name.to_lowercase(), e.description))
        .collect()
}

// ─── Hierarchy Parser ───

fn parse_hierarchy(raw: &str, descriptions: &std::collections::HashMap<String, String>) -> RymData {
    let mut genres = Vec::new();
    let mut descriptors = Vec::new();
    let mut path_stack: Vec<(usize, String)> = Vec::new(); // (level, name)
    let mut current_section: Option<String> = None;

    for line in raw.lines() {
        // Skip empty lines
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Level-0 headers: section name (Descriptors, Genres, Scenes & Movements)
        if !line.starts_with(' ') {
            current_section = Some(trimmed.to_string());
            path_stack.clear();
            // Push section as root path so entries get complete hierarchy
            path_stack.push((0, trimmed.to_string()));
            continue;
        }

        // Compute indent level (4 spaces per level)
        let indent_chars = line.chars().take_while(|c| *c == ' ').count();
        let level = indent_chars / 4;

        // Parse entry name and suffix
        let entry_name = if let Some(idx) = trimmed.find("::") {
            trimmed[..idx].trim().to_string()
        } else {
            trimmed.trim().to_string()
        };
        let suffix = trimmed
            .find("::")
            .and_then(|idx| trimmed[idx + 2..].split_whitespace().next())
            .unwrap_or("");

        // Maintain the path stack for this level
        while path_stack.last().is_some_and(|(l, _)| *l >= level) {
            path_stack.pop();
        }

        // Build full hierarchical path
        let path: Vec<String> = path_stack
            .iter()
            .map(|(_, n)| n.clone())
            .chain(std::iter::once(entry_name.clone()))
            .collect();

        match current_section.as_deref() {
            Some("Genres") | Some("Scenes & Movements") => {
                // Only add entries with ::genre suffix (leaf entries)
                // Parent entries (no suffix) tracked in path_stack for hierarchy
                if suffix == "genre" {
                    let desc = descriptions.get(&entry_name.to_lowercase()).cloned();
                    genres.push(Genre {
                        name: entry_name.clone(),
                        path,
                        description: desc,
                    });
                }
                if !entry_name.is_empty() {
                    path_stack.push((level, entry_name));
                }
            }
            Some("Descriptors") => {
                if !suffix.is_empty() {
                    // Skip section root (index 0 = "Descriptors"), get first real category
                    let category = path_stack
                        .get(1)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_default();
                    descriptors.push(Descriptor {
                        name: entry_name.clone(),
                        category,
                        descriptor_type: suffix.to_string(),
                    });
                }
                if !entry_name.is_empty() && suffix.is_empty() && level > 0 {
                    path_stack.push((level, entry_name));
                }
            }
            _ => {}
        }
    }

    // Build by-name index for genres
    let genres_by_name: std::collections::HashMap<String, usize> = genres
        .iter()
        .enumerate()
        .map(|(i, g)| (g.name.to_lowercase(), i))
        .collect();

    // Build by-name index for descriptors
    let descriptors_by_name: std::collections::HashMap<String, usize> = descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.to_lowercase(), i))
        .collect();

    RymData {
        genres,
        genres_by_name,
        descriptors,
        descriptors_by_name,
    }
}

// ─── Utility ───

/// Sørensen-Dice trigram coefficient between two strings.
fn trigram_overlap(a: &str, b: &str) -> f64 {
    let trigrams_a: std::collections::HashSet<[char; 3]> = trigrams(a).into_iter().collect();
    let trigrams_b: std::collections::HashSet<[char; 3]> = trigrams(b).into_iter().collect();

    let intersection = trigrams_a.intersection(&trigrams_b).count();
    if intersection == 0 {
        return 0.0;
    }
    let denom = trigrams_a.len() + trigrams_b.len();
    if denom == 0 {
        return 0.0;
    }
    2.0 * intersection as f64 / denom as f64
}

fn trigrams(s: &str) -> Vec<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genres_loaded() {
        let all = all_genres();
        assert!(all.len() > 5000, "Expected 5000+ genres, got {}", all.len());
        assert!(find_genre("Ambient").is_some());
        assert!(find_genre("ambient").is_some());
        assert!(find_genre("Black Metal").is_some());
    }

    #[test]
    fn test_descriptors_loaded() {
        let all = all_descriptors();
        assert!(
            all.len() > 200,
            "Expected 200+ descriptors, got {}",
            all.len()
        );
        assert!(find_descriptor("Apocalyptic").is_some());
        assert!(find_descriptor("Dark").is_some());
    }

    #[test]
    fn test_normalize_exact() {
        assert_eq!(normalize_style("Black Metal").unwrap(), "Black Metal");
        assert_eq!(normalize_style("ambient").unwrap(), "Ambient");
        assert_eq!(normalize_style("death metal").unwrap(), "Death Metal");
    }

    #[test]
    fn test_normalize_substring() {
        // "depressive black metal" contains "Black Metal"
        let result = normalize_style("depressive black metal");
        assert!(result.is_some(), "depressive black metal should match");
        // "Melodic Death Metal" should match "Death Metal"
        let result2 = normalize_style("Melodic Death Metal");
        assert!(result2.is_some(), "Melodic Death Metal should match");
    }

    #[test]
    fn test_normalize_trigram() {
        // "neoclassic metal" → "Neoclassical Metal" via trigram overlap
        let result = normalize_style("neoclassic metal");
        assert!(
            result.is_some(),
            "neoclassic metal should match via trigram"
        );
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_style(""), None);
        assert_eq!(normalize_style("   "), None);
    }

    #[test]
    fn test_genre_path() {
        let genre = find_genre("Black Metal").unwrap();
        assert!(!genre.path.is_empty());
        assert_eq!(genre.path.last().unwrap(), "Black Metal");
    }

    #[test]
    fn test_genre_descriptions() {
        let ambient = find_genre("Ambient").unwrap();
        assert!(ambient.description.is_some());
        assert!(ambient.description.as_ref().unwrap().contains("texture"));
    }

    #[test]
    fn test_descriptor_category() {
        let apoc = find_descriptor("Apocalyptic").unwrap();
        assert_eq!(apoc.category, "Atmosphere");
    }

    #[test]
    fn test_scenes_and_movements_included() {
        // Scenes & Movements section should be parsed as genres
        assert!(
            find_genre("Riot Grrrl").is_some(),
            "Riot Grrrl (scene) should be present"
        );
        assert!(
            find_genre("Visual kei").is_some(),
            "Visual kei (scene) should be present"
        );
        assert!(
            find_genre("Straight Edge").is_some(),
            "Straight Edge (movement) should be present"
        );
        // Verify total includes scenes
        let total = all_genres().len();
        let scene_genres = all_genres()
            .iter()
            .filter(|g| {
                g.path
                    .first()
                    .map(|p| p == "Scenes & Movements")
                    .unwrap_or(false)
            })
            .count();
        assert!(
            scene_genres > 100,
            "Expected 100+ scenes/movements, got {}",
            scene_genres
        );
    }
}
