use crate::DbError;
use rusqlite::Connection;
use std::collections::HashMap;

/// Insert a genre and return its row ID.
fn insert_genre(
    conn: &Connection,
    name: &str,
    source: &str,
    parent_id: Option<i64>,
    description: Option<&str>,
    path: Option<&str>,
) -> Result<i64, DbError> {
    let lower = name.to_lowercase();
    // UPSERT: if genre already exists (e.g. from MusicBee), update parent_id
    // from RYM hierarchy to enable parent expansion. Only set parent_id if
    // the existing row has no parent, preserving explicit MusicBee parents.
    conn.execute(
        "INSERT INTO genres (name, name_lower, source, parent_id, description, path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(name_lower) DO UPDATE SET
            parent_id = CASE WHEN genres.parent_id IS NULL THEN ?4 ELSE genres.parent_id END,
            description = CASE WHEN genres.description IS NULL THEN ?5 ELSE genres.description END,
            path = CASE WHEN genres.path IS NULL THEN ?6 ELSE genres.path END",
        rusqlite::params![name, lower, source, parent_id, description, path],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM genres WHERE name_lower = ?1",
        rusqlite::params![lower],
        |row| row.get(0),
    )?;
    Ok(id)
}

fn genre_id(conn: &Connection, name_lower: &str) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM genres WHERE name_lower = ?1",
        rusqlite::params![name_lower],
        |row| row.get(0),
    )
    .ok()
}

/// Seed from MusicBee hierarchy. Two-pass: collect canonicals, then insert
/// with " music" dedup (if shorter canonical exists, only create alias).
pub fn seed_musicbee(conn: &Connection, text: &str) -> Result<usize, DbError> {
    let mut all_canonicals: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if let Some(name) = trimmed.strip_suffix("::genre") {
            all_canonicals.push(name.trim().to_string());
        } else if let Some(name) = trimmed.strip_suffix("::album genre") {
            all_canonicals.push(name.trim().to_string());
        }
    }

    let name_set: std::collections::HashSet<String> = all_canonicals
        .iter().map(|n| n.to_lowercase()).collect();

    let mut inserted = 0;
    for canonical in &all_canonicals {
        if canonical.is_empty() { continue; }

        // Handle " music" suffix: if shorter form exists, only create alias
        if let Some(stripped) = canonical.strip_suffix(" music") {
            let stripped_lower = stripped.trim().to_lowercase();
            if !stripped_lower.is_empty() && name_set.contains(&stripped_lower) {
                if let Some(target_id) = genre_id(conn, &stripped_lower) {
                    conn.execute(
                        "INSERT OR IGNORE INTO genre_aliases (alias, genre_id, alias_type) VALUES (?1, ?2, 'musicbee_suffix')",
                        rusqlite::params![canonical.to_lowercase(), target_id],
                    )?;
                }
                continue;
            }
        }

        conn.execute(
            "INSERT OR IGNORE INTO genres (name, name_lower, source) VALUES (?1, ?2, 'musicbee')",
            rusqlite::params![canonical, canonical.to_lowercase()],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

/// Seed Discogs override aliases. Creates target genres if they don't exist.
pub fn seed_discogs_overrides(conn: &Connection) -> Result<usize, DbError> {
    let overrides: &[(&str, &str)] = &[
        ("heavy metal", "Heavy metal"), ("thrash", "Thrash metal"),
        ("death", "Death metal"), ("black", "Black metal"),
        ("doom", "Doom metal"), ("drone", "Drone"),
        ("speed metal", "Speed metal"), ("power metal", "Power metal"),
        ("prog rock", "Progressive rock"), ("prog metal", "Progressive metal"),
        ("alt rock", "Alternative rock"), ("alt metal", "Alternative metal"),
        ("industrial", "Industrial"), ("electronic", "Electronic"),
        ("ambient", "Ambient"), ("hip hop", "Hip hop"),
        ("rnb", "R&B"), ("r&b", "R&B"),
        ("soul", "Soul"), ("funk", "Funk"),
        ("blues", "Blues"), ("jazz", "Jazz"),
        ("classical", "Classical"), ("folk", "Folk"),
        ("country", "Country"), ("punk", "Punk"),
        ("reggae", "Reggae"), ("ska", "Ska"),
        ("pop", "Pop"), ("rock", "Rock"),
        ("indie", "Indie"),
    ];

    let mut count = 0;
    for (alias, target) in overrides {
        let lower = target.to_lowercase();
        let target_id = match genre_id(conn, &lower) {
            Some(id) => id,
            None => insert_genre(conn, target, "discogs_override", None, None, None)?,
        };
        conn.execute(
            "INSERT OR IGNORE INTO genre_aliases (alias, genre_id, alias_type) VALUES (?1, ?2, 'discogs_override')",
            rusqlite::params![alias.to_string(), target_id],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Seed RYM genres, descriptors, and parent links from hierarchy text.
pub fn seed_rym(
    conn: &Connection,
    text: &str,
    descriptions_json: Option<&str>,
) -> Result<(usize, usize), DbError> {
    let descriptions: HashMap<String, String> = descriptions_json
        .and_then(|json| {
            #[derive(serde::Deserialize)]
            struct DescriptionsIndex { genres: Vec<DescriptionEntry> }
            #[derive(serde::Deserialize)]
            struct DescriptionEntry { name: String, description: String }
            serde_json::from_str::<DescriptionsIndex>(json).ok().map(|idx| {
                idx.genres.into_iter().map(|e| (e.name.to_lowercase(), e.description)).collect()
            })
        }).unwrap_or_default();

    let mut genre_count = 0;
    let mut descriptor_count = 0;
    let mut path_stack: Vec<(usize, String)> = Vec::new(); // (level, name)
    let mut current_section: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        if !line.starts_with(' ') {
            current_section = Some(trimmed.to_string());
            path_stack.clear();
            path_stack.push((0, trimmed.to_string()));
            continue;
        }

        let indent_chars = line.chars().take_while(|c| *c == ' ').count();
        let level = indent_chars / 4;

        let entry_name = if let Some(idx) = trimmed.find("::") {
            trimmed[..idx].trim().to_string()
        } else {
            trimmed.trim().to_string()
        };
        let suffix = trimmed.find("::")
            .and_then(|idx| trimmed[idx + 2..].split_whitespace().next())
            .unwrap_or("");

        while path_stack.last().map_or(false, |(l, _)| *l >= level) {
            path_stack.pop();
        }

        match current_section.as_deref() {
            Some("Genres") | Some("Scenes & Movements") => {
                if suffix == "genre" {
                    // Leaf genre entry
                    let desc = descriptions.get(&entry_name.to_lowercase()).map(|s| s.as_str());
                    let full_path = if path_stack.is_empty() {
                        entry_name.clone()
                    } else {
                        let base: Vec<&str> = path_stack.iter().map(|(_, n)| n.as_str()).collect();
                        base.join("/") + "/" + &entry_name
                    };

                    // Find parent, skipping self-referencing intermediates
                    let parent_id = SelfRefResolver::resolve(conn, &path_stack, &entry_name);
                    insert_genre(conn, &entry_name, "rym", parent_id, desc, Some(&full_path))?;
                    genre_count += 1;
                } else {
                    // Intermediate entry — insert as genre so parent links work
                    let parent_id = if path_stack.len() > 1 {
                        genre_id(conn, &path_stack.last().unwrap().1.to_lowercase())
                    } else {
                        None
                    };
                    // Only insert if not already present (from MusicBee or earlier RYM)
                    insert_genre(conn, &entry_name, "rym", parent_id, None, None)?;
                }

                if !entry_name.is_empty() {
                    path_stack.push((level, entry_name));
                }
            }
            Some("Descriptors") => {
                if !suffix.is_empty() {
                    let category = path_stack.get(1).map(|(_, n)| n.as_str()).unwrap_or("");
                    conn.execute(
                        "INSERT OR IGNORE INTO descriptors (name, name_lower, category, descriptor_type) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![entry_name, entry_name.to_lowercase(), category, suffix],
                    )?;
                    descriptor_count += 1;
                }
                if !entry_name.is_empty() && suffix.is_empty() && level > 0 {
                    path_stack.push((level, entry_name));
                }
            }
            _ => {}
        }
    }

    // Post-process: fix self-referencing parent_ids
    // (leaf genre with same name as intermediate parent)
    let mut stmt = conn.prepare(
        "SELECT id, name, path FROM genres WHERE source = 'rym' AND path IS NOT NULL"
    ).unwrap();
    let rows: Vec<(i64, String, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }).unwrap().filter_map(|r| r.ok()).collect();
    drop(stmt);

    let mut fix_count = 0;
    for (id, name, path) in &rows {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 3 { continue; }

        let lower = name.to_lowercase();
        // Walk up from last parent position, skip names matching this genre
        let mut parent_idx = parts.len() as isize - 2;
        while parent_idx > 0 && parts[parent_idx as usize].to_lowercase() == lower {
            parent_idx -= 1;
        }
        if parent_idx > 0 {
            let parent_name = parts[parent_idx as usize];
            if let Some(pid) = genre_id(conn, &parent_name.to_lowercase()) {
                // Only update if different from current parent_id
                let current_pid: Option<i64> = conn.query_row(
                    "SELECT parent_id FROM genres WHERE id = ?1", rusqlite::params![id], |row| row.get(0)
                ).ok().flatten();
                if current_pid != Some(pid) {
                    conn.execute(
                        "UPDATE genres SET parent_id = ?1 WHERE id = ?2",
                        rusqlite::params![pid, id],
                    )?;
                    fix_count += 1;
                }
            }
        }
    }
    if fix_count > 0 {
        tracing::debug!("Fixed {} RYM self-referencing parent_ids", fix_count);
    }

    Ok((genre_count, descriptor_count))
}

/// Resolve self-referencing hierarchy for RYM:
/// Walk path_stack from top to bottom, find first entry with name
/// different from the leaf (skip entries with same name as leaf).
struct SelfRefResolver;
impl SelfRefResolver {
    fn resolve(conn: &Connection, path_stack: &[(usize, String)], leaf_name: &str) -> Option<i64> {
        if path_stack.len() < 2 { return None; }
        let lower = leaf_name.to_lowercase();
        for i in (0..path_stack.len()).rev() {
            if path_stack[i].1.to_lowercase() != lower {
                return genre_id(conn, &path_stack[i].1.to_lowercase());
            }
        }
        None
    }
}

/// Full seed: MusicBee + Discogs + RYM.
pub fn seed_all(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(crate::schema::CREATE_TABLES)?;

    let musicbee_text = include_str!("../data/musicbee.txt");
    let mb_count = seed_musicbee(conn, musicbee_text)?;
    tracing::info!("Seeded {} MusicBee genres", mb_count);

    let d_count = seed_discogs_overrides(conn)?;
    tracing::info!("Seeded {} Discogs overrides", d_count);

    let rym_text = include_str!("../data/rym-hierarchy.txt");
    let rym_desc_json = include_str!("../data/rym-descriptions.json");
    let (rg_count, rd_count) = seed_rym(conn, rym_text, Some(rym_desc_json))?;
    tracing::info!("Seeded {} RYM genres and {} descriptors", rg_count, rd_count);

    Ok(())

}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::schema::CREATE_TABLES).unwrap();
        conn
    }

    fn genre_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM genres", [], |row| row.get(0)).unwrap()
    }

    fn alias_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM genre_aliases", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn test_seed_musicbee_basic() {
        let conn = make_conn();
        let text = "Classical\n    Classical::genre\n    Aria::genre\nElectronic\n    Electronic::genre\n    Electronic music::genre\n";
        let inserted = seed_musicbee(&conn, text).unwrap();
        assert_eq!(inserted, 3, "3 genres: Classical, Aria, Electronic");
        assert_eq!(genre_count(&conn), 3);

        let ec: i64 = conn.query_row(
            "SELECT COUNT(*) FROM genre_aliases WHERE alias = 'electronic music'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(ec, 1);
    }

    #[test]
    fn test_seed_musicbee_unique_count() {
        let conn = make_conn();
        let _inserted = seed_musicbee(&conn, include_str!("../data/musicbee.txt")).unwrap();
        let unique = genre_count(&conn);
        // MusicBee has 3721 unique genre names
        assert!(unique > 3000, "got {}", unique);
        assert!(unique < 4000, "got {}", unique);
    }

    #[test]
    fn test_seed_discogs_overrides() {
        let conn = make_conn();
        let count = seed_discogs_overrides(&conn).unwrap();
        assert_eq!(count, 31);

        let canonical: String = conn.query_row(
            "SELECT g.name FROM genre_aliases a JOIN genres g ON a.genre_id = g.id WHERE a.alias = 'thrash'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(canonical, "Thrash metal");
    }

    #[test]
    fn test_seed_rym_basic() {
        let conn = make_conn();
        let text = "Genres\n    Metal\n        Death Metal\n            Death Metal::genre\n        Black Metal\n            Black Metal::genre\n";
        let (g_count, _) = seed_rym(&conn, text, None).unwrap();
        // Metal, Death Metal (intermediate), Death Metal (leaf), Black Metal (intermediate), Black Metal (leaf)
        // Metal and Death Metal and Black Metal were inserted as intermediates
        // Death Metal and Black Metal also inserted as leaf genres (same name, INSERT OR IGNORE)
        // So unique inserts: Metal, Death Metal, Black Metal = 3 unique
        // But g_count counts leaf ::genre entries = 2
        assert_eq!(g_count, 2);

        // Death Metal's parent should be Metal (not itself)
        let parent_name: Option<String> = conn.query_row(
            "SELECT p.name FROM genres g JOIN genres p ON g.parent_id = p.id WHERE g.name_lower = 'death metal'",
            [], |row| row.get(0),
        ).ok();
        assert_eq!(parent_name.as_deref(), Some("Metal"),
            "Death Metal parent should be Metal, got {:?}", parent_name);
    }

    #[test]
    fn test_seed_rym_with_descriptors() {
        let conn = make_conn();
        let text = "Descriptors\n    Atmosphere\n        Dark::mood\n";
        let (g_count, d_count) = seed_rym(&conn, text, None).unwrap();
        assert_eq!(g_count, 0);
        assert_eq!(d_count, 1);
    }

    #[test]
    fn test_seed_all_total() {
        let conn = Connection::open_in_memory().unwrap();
        seed_all(&conn).unwrap();

        // MusicBee ~3721 unique + RYM ~6 unique = ~3727 total
        let total = genre_count(&conn);
        assert!(total > 3000, "Expected >3000 total genres, got {}", total);
        assert!(total < 4500, "Expected <4500 total genres, got {}", total);

        for name in &["black metal", "heavy metal", "ambient", "shoegaze"] {
            let exists: bool = conn.query_row(
                "SELECT 1 FROM genres WHERE name_lower = ?1",
                rusqlite::params![name], |_| Ok(()),
            ).is_ok();
            assert!(exists, "Genre '{}' should exist", name);
        }

        assert!(alias_count(&conn) > 0, "Should have aliases");
    }
}
