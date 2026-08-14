mod schema;
mod seed;

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Thread-safe genre database backed by in-memory SQLite.
/// Populated from compile-time-embedded data on first access.
pub struct GenreDb {
    conn: Mutex<Connection>,
}

static DB: OnceLock<GenreDb> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Seed data error: {0}")]
    Seed(String),
}

/// Genre detail returned from find_genre
#[derive(Debug, Clone)]
pub struct GenreInfo {
    pub id: i64,
    pub name: String,
    pub source: String,
    pub parent_name: Option<String>,
    pub description: Option<String>,
    pub path: Option<Vec<String>>,
}

/// RYM descriptor info
#[derive(Debug, Clone)]
pub struct DescriptorInfo {
    pub name: String,
    pub category: String,
    pub descriptor_type: String,
}

impl GenreDb {
    /// Create a fresh in-memory genre database (for testing).
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(schema::CREATE_TABLES)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create + seed from compiled-in data (MusicBee + RYM + aliases).
    pub fn open_seeded() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        seed::seed_all(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open or create a persistent on-disk genre database.
    /// Auto-seeds from embedded data if the schema is missing.
    pub fn open_persistent(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        let is_seeded: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='genres')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !is_seeded {
            seed::seed_all(&conn)?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Global singleton — lazy-init on first access.
    pub fn global() -> &'static GenreDb {
        DB.get_or_init(|| {
            tracing::info!("Initializing genre database from embedded data");
            Self::open_seeded().expect("Failed to seed genre database")
        })
    }

    // ── Normalize ───────────────────────────────────────────────

    /// Check alias first, then direct genre match.
    /// Aliases represent explicit canonical mappings (Discogs overrides, " music" suffix variants).
    /// Checking them first matches the old HashMap behavior where aliases and genres
    /// lived in the same map, avoiding RYM genres shadowing Discogs aliases.
    fn try_normalize_exact(&self, conn: &Connection, lowered: &str) -> Option<String> {
        // 1. Alias match FIRST (prevents RYM genres like "Thrash" from shadowing
        //    Discogs override "thrash" -> "Thrash metal")
        if let Ok(name) = conn.query_row(
            "SELECT g.name FROM genre_aliases a JOIN genres g ON a.genre_id = g.id WHERE a.alias = ?1",
            rusqlite::params![lowered],
            |row| row.get(0),
        ) {
            return Some(name);
        }
        // 2. Direct match in genres
        if let Ok(name) = conn.query_row(
            "SELECT name FROM genres WHERE name_lower = ?1",
            rusqlite::params![lowered],
            |row| row.get::<_, String>(0),
        ) {
            return Some(name);
        }
        None
    }

    /// Normalize a genre name to its canonical form.
    /// Matching order: alias -> exact -> "music" suffix -> parenthetical strip
    ///   -> trailing slash/space strip -> RYM substring/trigram fallback.
    /// Returns original if nothing matches.
    pub fn normalize_genre(&self, name: &str) -> String {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase().trim().to_string();
        if lowered.is_empty() {
            return name.to_string();
        }

        // 1. Alias + exact match
        if let Some(canonical) = self.try_normalize_exact(&conn, &lowered) {
            // If input ends with " music", prefer shorter canonical
            if lowered.ends_with(" music") {
                let without = lowered.strip_suffix(" music").unwrap().trim().to_string();
                if let Some(shorter) = self.try_normalize_exact(&conn, &without) {
                    return shorter;
                }
            }
            return canonical;
        }

        // 2. Match after stripping parenthetical qualifiers
        if let Some(paren) = lowered.find('(') {
            let base = lowered[..paren].trim().to_string();
            if let Some(canonical) = self.try_normalize_exact(&conn, &base) {
                return format!("{} {}", canonical, &lowered[paren..]);
            }
        }

        // 3. Match after stripping " music" suffix
        if let Some(stripped) = lowered.strip_suffix(" music") {
            let trimmed = stripped.trim().to_string();
            if let Some(canonical) = self.try_normalize_exact(&conn, &trimmed) {
                return canonical;
            }
        }

        // 4. Match after stripping trailing spaces/slashes
        let cleaned = lowered.trim_end_matches(&[' ', '/'] as &[_]).to_string();
        if let Some(canonical) = self.try_normalize_exact(&conn, &cleaned) {
            return canonical;
        }

        // 5. RYM substring/trigram fallback (no '/' strings - handled by split)
        if !name.contains('/') {
            if let Some(rym_name) = self.rym_fallback(&conn, &lowered) {
                return rym_name;
            }
        }

        name.to_string()
    }

    /// RYM fuzzy fallback: substring match + trigram overlap.
    fn rym_fallback(&self, conn: &Connection, lowered: &str) -> Option<String> {
        let mut stmt = conn
            .prepare("SELECT name, name_lower FROM genres WHERE source = 'rym'")
            .ok()?;
        let rym_genres: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        if rym_genres.is_empty() {
            return None;
        }

        let trimmed = lowered.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Exact match within RYM
        for (name, name_lower) in &rym_genres {
            if name_lower == trimmed {
                return Some(name.clone());
            }
        }

        // Substring match: find best overlap
        let mut best: Option<(String, f64)> = None;
        for (name, name_lower) in &rym_genres {
            if name_lower.contains(trimmed) || trimmed.contains(name_lower.as_str()) {
                let score = trimmed.len().min(name_lower.len()) as f64
                    / trimmed.len().max(name_lower.len()) as f64;
                if best.as_ref().map_or(true, |(_, s)| score > *s) {
                    best = Some((name.clone(), score));
                }
                continue;
            }
            let overlap = trigram_overlap(trimmed, name_lower);
            if overlap >= 0.6 {
                if best.as_ref().map_or(true, |(_, s)| overlap > *s) {
                    best = Some((name.clone(), overlap));
                }
            }
        }

        best.map(|(name, _)| name)
    }

    /// Batch normalize, dedup, sort, and split '/' -separated entries.
    pub fn normalize_genres(&self, genres: &[String]) -> Vec<String> {
        let mut normalized: Vec<String> = genres
            .iter()
            .map(|g| self.normalize_genre(g))
            .collect();

        let mut split: Vec<String> = Vec::new();
        for g in &normalized {
            if g.contains(" / ") {
                for part in g.split(" / ") {
                    let n = self.normalize_genre(part.trim());
                    if !n.is_empty() { split.push(n); }
                }
            } else if g.contains('/') {
                for part in g.split('/') {
                    let n = self.normalize_genre(part.trim());
                    if !n.is_empty() { split.push(n); }
                }
            }
        }
        normalized.extend(split);
        normalized.sort();
        normalized.dedup();
        normalized
    }

    // ── Known check ─────────────────────────────────────────────

    /// Check if a genre name is known to any source.
    pub fn is_known_genre(&self, name: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase().trim().to_string();
        if lowered.is_empty() { return false; }

        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM genres WHERE name_lower = ?1
                 UNION ALL SELECT 1 FROM genre_aliases WHERE alias = ?1)",
                rusqlite::params![lowered],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists { return true; }

        let cleaned = lowered.trim_end_matches(&[' ', '/'] as &[_]).to_string();
        if cleaned != lowered {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM genres WHERE name_lower = ?1)",
                rusqlite::params![cleaned],
                |row| row.get(0),
            ).unwrap_or(false)
        } else {
            false
        }
    }

    // ── Parent expansion ────────────────────────────────────────

    /// Expand genres by adding parent hierarchy (RYM path traversal).
    pub fn expand_parent_genres(&self, genres: &[String]) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut expanded = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for genre in genres {
            let key = genre.to_lowercase();
            if seen.insert(key.clone()) {
                expanded.push(genre.clone());
            }

            if let Ok(mut stmt) = conn.prepare(
                "WITH RECURSIVE ancestors(id, name, name_lower, parent_id, depth) AS (
                    SELECT id, name, name_lower, parent_id, 0 FROM genres WHERE name_lower = ?1
                    UNION ALL
                    SELECT g.id, g.name, g.name_lower, g.parent_id, a.depth + 1
                    FROM genres g
                    JOIN ancestors a ON g.id = a.parent_id
                    WHERE a.parent_id IS NOT NULL
                )
                SELECT name, name_lower FROM ancestors WHERE depth > 0 ORDER BY depth",
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![key], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }) {
                    for row in rows.flatten() {
                        let (p_name, p_lower) = row;
                        if seen.insert(p_lower) {
                            expanded.push(p_name);
                        }
                    }
                }
            }
        }

        expanded
    }

    // ── All genres ──────────────────────────────────────────────

    /// All canonical genre names, sorted, deduplicated.
    pub fn all_genres(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT name FROM genres ORDER BY name COLLATE NOCASE")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    // ── Enhanced queries ────────────────────────────────────────

    /// Detailed info for a genre by name (case-insensitive).
    pub fn find_genre(&self, name: &str) -> Option<GenreInfo> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();

        conn.query_row(
            "SELECT g.id, g.name, g.source, g.description, g.path, p.name AS parent_name
             FROM genres g
             LEFT JOIN genres p ON g.parent_id = p.id
             WHERE g.name_lower = ?1",
            rusqlite::params![lowered],
            |row| {
                Ok(GenreInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    source: row.get(2)?,
                    parent_name: row.get(5)?,
                    description: row.get(3)?,
                    path: row.get::<_, Option<String>>(4)?.map(|p| {
                        p.split('/').map(|s| s.to_string()).collect()
                    }),
                })
            },
        )
        .ok()
    }

    /// Direct parent name for a genre.
    pub fn get_parent(&self, name: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        conn.query_row(
            "SELECT p.name FROM genres g
             JOIN genres p ON g.parent_id = p.id
             WHERE g.name_lower = ?1",
            rusqlite::params![lowered],
            |row| row.get(0),
        )
        .ok()
    }

    /// All ancestor names, from immediate parent up to root.
    pub fn get_ancestors(&self, name: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        let Ok(mut stmt) = conn
            .prepare(
                "WITH RECURSIVE ancestors(id, name, parent_id, depth) AS (
                    SELECT id, name, parent_id, 0 FROM genres WHERE name_lower = ?1
                    UNION ALL
                    SELECT g.id, g.name, g.parent_id, a.depth + 1
                    FROM genres g
                    JOIN ancestors a ON g.id = a.parent_id
                    WHERE a.parent_id IS NOT NULL
                )
                SELECT name FROM ancestors WHERE depth > 0 ORDER BY depth",
            ) else { return vec![]; };
        let Ok(rows) = stmt.query_map(rusqlite::params![lowered], |row| row.get::<_, String>(0))
            else { return vec![]; };
        rows.filter_map(|r| r.ok()).collect()
    }

    /// Direct children (subgenres) for a genre.
    pub fn get_subgenres(&self, name: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        let Ok(mut stmt) = conn.prepare(
            "SELECT g.name FROM genres g
             JOIN genres p ON g.parent_id = p.id
             WHERE p.name_lower = ?1
             ORDER BY g.name"
        ) else { return vec![]; };
        let Ok(rows) = stmt.query_map(rusqlite::params![lowered], |row| row.get::<_, String>(0))
            else { return vec![]; };
        rows.filter_map(|r| r.ok()).collect()
    }

    /// Direct children with their RYM descriptions.
    pub fn get_subgenres_with_descriptions(&self, name: &str) -> Vec<(String, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        let Ok(mut stmt) = conn.prepare(
            "SELECT g.name, g.description FROM genres g
             JOIN genres p ON g.parent_id = p.id
             WHERE p.name_lower = ?1
             ORDER BY g.name"
        ) else { return vec![]; };
        let Ok(rows) = stmt.query_map(rusqlite::params![lowered], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        }) else { return vec![]; };
        rows.filter_map(|r| r.ok()).collect()
    }

    /// RYM description for a genre (if available).
    pub fn get_description(&self, name: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        conn.query_row(
            "SELECT description FROM genres WHERE name_lower = ?1 AND description IS NOT NULL",
            rusqlite::params![lowered],
            |row| row.get(0),
        )
        .ok()
    }

    // ── Descriptor queries (RYM) ────────────────────────────────

    /// Find descriptors by name (case-insensitive).
    pub fn find_descriptor(&self, name: &str) -> Option<DescriptorInfo> {
        let conn = self.conn.lock().unwrap();
        let lowered = name.to_lowercase();
        conn.query_row(
            "SELECT name, category, descriptor_type FROM descriptors WHERE name_lower = ?1",
            rusqlite::params![lowered],
            |row| {
                Ok(DescriptorInfo {
                    name: row.get(0)?,
                    category: row.get(1)?,
                    descriptor_type: row.get(2)?,
                })
            },
        )
        .ok()
    }

    /// All descriptors in a category.
    /// All descriptors, grouped by descriptor_type (e.g. tone/rhythm/theme).
    pub fn all_descriptors(&self) -> Vec<DescriptorInfo> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, category, descriptor_type FROM descriptors ORDER BY descriptor_type, name")
            .unwrap();
        stmt.query_map([], |row| {
            Ok(DescriptorInfo {
                name: row.get(0)?,
                category: row.get(1)?,
                descriptor_type: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn descriptors_by_category(&self, category: &str) -> Vec<DescriptorInfo> {
        let conn = self.conn.lock().unwrap();
        let lower_cat = category.to_lowercase();
        let mut stmt = conn
            .prepare(
                "SELECT name, category, descriptor_type FROM descriptors WHERE LOWER(category) = ?1 ORDER BY name",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![lower_cat], |row| {
            Ok(DescriptorInfo {
                name: row.get(0)?,
                category: row.get(1)?,
                descriptor_type: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }
}

// ─── Utility ───

fn trigram_overlap(a: &str, b: &str) -> f64 {
    let trigrams_a: std::collections::HashSet<[char; 3]> = trigrams(a).into_iter().collect();
    let trigrams_b: std::collections::HashSet<[char; 3]> = trigrams(b).into_iter().collect();
    let intersection = trigrams_a.intersection(&trigrams_b).count();
    if intersection == 0 { return 0.0; }
    let denom = trigrams_a.len() + trigrams_b.len();
    if denom == 0 { return 0.0; }
    2.0 * intersection as f64 / denom as f64
}

fn trigrams(s: &str) -> Vec<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

// ─── Tests ───

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_db() -> GenreDb {
        GenreDb::open_seeded().unwrap()
    }

    #[test]
    fn verify_parent_propagation() {
        let db = seeded_db();
        let conn = db.conn.lock().unwrap();
        // Thrash Metal should have parent Metal after propagation
        let parent: Option<String> = conn.query_row(
            "SELECT p.name FROM genres g JOIN genres p ON g.parent_id = p.id WHERE g.name_lower = 'thrash metal'",
            [], |row| row.get(0),
        ).ok();
        assert_eq!(parent.as_deref(), Some("Metal"), "Thrash Metal -> Metal parent");
        // Technical Death Metal -> Death Metal -> Metal
        let ancestors: Vec<String> = {
            let mut stmt = conn.prepare(
                "WITH RECURSIVE a(id, name, parent_id, depth) AS (
                    SELECT id, name, parent_id, 0 FROM genres WHERE name_lower = ?1
                    UNION ALL
                    SELECT g.id, g.name, g.parent_id, a.depth+1 FROM genres g JOIN a ON g.id = a.parent_id WHERE a.parent_id IS NOT NULL
                ) SELECT name FROM a WHERE depth > 0 ORDER BY depth"
            ).unwrap();
            stmt.query_map(rusqlite::params!["technical death metal"], |row| row.get::<_, String>(0))
                .unwrap().filter_map(|r| r.ok()).collect()
        };
        assert!(ancestors.iter().any(|n| n.to_lowercase() == "death metal"), "Technical Death Metal -> Death Metal, got {:?}", ancestors);
        assert!(ancestors.iter().any(|n| n.to_lowercase() == "metal"), "Technical Death Metal -> Metal, got {:?}", ancestors);
    }

    // ── normalize_genre ────────────────────────────────────

    #[test]
    fn test_normalize_exact() {
        let db = seeded_db();
        assert_eq!(db.normalize_genre("Heavy metal"), "Heavy metal");
        assert_eq!(db.normalize_genre("heavy metal"), "Heavy metal");
        assert_eq!(db.normalize_genre("Black Metal"), "Black metal");
        assert_eq!(db.normalize_genre("death metal"), "Death metal");
    }

    #[test]
    fn test_normalize_suffix() {
        let db = seeded_db();
        assert_eq!(db.normalize_genre("Classical music"), "Classical");
        assert_eq!(db.normalize_genre("Electronic music"), "Electronic");
    }

    #[test]
    fn test_normalize_unknown() {
        let db = seeded_db();
        assert_eq!(db.normalize_genre(""), "");
        assert_eq!(db.normalize_genre("Super obscure genre 3000"), "Super obscure genre 3000");
    }

    #[test]
    fn test_normalize_discogs_alias() {
        let db = seeded_db();
        assert_eq!(db.normalize_genre("thrash"), "Thrash metal");
        assert_eq!(db.normalize_genre("death"), "Death metal");
        assert_eq!(db.normalize_genre("rnb"), "R&B");
    }

    #[test]
    fn test_normalize_parenthetical() {
        let db = seeded_db();
        // "Rock (Experimental)" should match "Rock" and preserve " (experimental)"
        // The old code lowercases the content inside parentheses
        let result = db.normalize_genre("Rock (Experimental)");
        assert!(result.contains("Rock"));
        assert!(result.contains("(experimental)") || result.contains("(Experimental)"),
            "Parenthetical content should be preserved, got: {:?}", result);
    }

    #[test]
    fn test_normalize_trailing_slash() {
        let db = seeded_db();
        assert_eq!(db.normalize_genre("Punk / Hardcore"), "Punk / Hardcore");
    }

    // ── normalize_genres ───────────────────────────────────

    #[test]
    fn test_normalize_genres_dedup_sort() {
        let db = seeded_db();
        let input = vec!["Heavy metal".to_string(), "heavy metal".to_string(), "Black metal".to_string()];
        let result = db.normalize_genres(&input);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"Heavy metal".to_string()));
        assert!(result.contains(&"Black metal".to_string()));
    }

    #[test]
    fn test_normalize_genres_split_slash() {
        let db = seeded_db();
        let input = vec!["Doom Metal / Drone".to_string()];
        let result = db.normalize_genres(&input);
        assert!(result.contains(&"Doom metal".to_string()) || result.contains(&"Doom Metal".to_string()));
        assert!(result.contains(&"Drone".to_string()));
    }

    // ── is_known_genre ─────────────────────────────────────

    #[test]
    fn test_is_known() {
        let db = seeded_db();
        assert!(db.is_known_genre("Heavy metal"));
        assert!(db.is_known_genre("Black metal"));
        assert!(!db.is_known_genre("FakeGenre123"));
    }

    #[test]
    fn test_is_known_discogs_alias() {
        let db = seeded_db();
        assert!(db.is_known_genre("thrash"));
        assert!(db.is_known_genre("death"));
    }

    #[test]
    fn test_is_known_rym() {
        let db = seeded_db();
        assert!(db.is_known_genre("2-Step"), "2-Step should exist via RYM/MB");
    }

    // ── expand_parent_genres ───────────────────────────────

    #[test]
    fn test_expand_parent_genres_basic() {
        let db = seeded_db();
        let expanded = db.expand_parent_genres(&["Thrash Metal".to_string()]);
        assert!(expanded.contains(&"Thrash Metal".to_string()));
    }

    #[test]
    fn test_expand_parent_genres_empty() {
        let db = seeded_db();
        let expanded: Vec<String> = db.expand_parent_genres(&[]);
        assert!(expanded.is_empty());
    }

    // ── all_genres ─────────────────────────────────────────

    #[test]
    fn test_all_genres_loaded() {
        let db = seeded_db();
        let all = db.all_genres();
        assert!(all.len() > 3000);
        assert!(all.contains(&"Heavy metal".to_string()));
        assert!(all.contains(&"Black metal".to_string()));
    }

    // ── find_genre ─────────────────────────────────────────

    #[test]
    fn test_find_genre() {
        let db = seeded_db();
        let info = db.find_genre("Ambient").unwrap();
        assert_eq!(info.name, "Ambient");
        assert!(info.source == "musicbee" || info.source == "rym");
    }

    #[test]
    fn test_find_genre_not_found() {
        let db = seeded_db();
        assert!(db.find_genre("NonExistentGenreXYZ").is_none());
    }

    // ── Descriptors ────────────────────────────────────────

    #[test]
    fn test_find_descriptor() {
        let db = seeded_db();
        assert!(db.find_descriptor("Apocalyptic").is_some());
    }

    #[test]
    fn test_descriptors_by_category() {
        let db = seeded_db();
        assert!(!db.descriptors_by_category("Atmosphere").is_empty());
    }

    // ── RYM fallback ───────────────────────────────────────

    #[test]
    fn test_rym_fallback_normalize() {
        let db = seeded_db();
        let result = db.normalize_genre("Shoegaze");
        assert_eq!(result, "Shoegaze");
    }

    // ── Global singleton ───────────────────────────────────

    #[test]
    fn test_global_singleton() {
        let db = GenreDb::global();
        assert!(db.is_known_genre("Heavy metal"));
        let db2 = GenreDb::global();
        assert!(std::ptr::eq(db, db2));
    }

}

