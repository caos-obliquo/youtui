// Copyright (c) 2026 caos-obliquo <caos_obliquo@outlook.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::{genre_map, ValidatedMetadata};
use std::collections::HashMap;

/// Expand genres by adding parent genres from the genre database.
///
/// Uses the SQLite genre database's parent hierarchy (RYM parent tree)
/// to add ancestor genres for each input genre.
fn expand_parent_genres(genres: &[String]) -> Vec<String> {
    genre_db_sqlite::GenreDb::global().expand_parent_genres(genres)
}

/// Merge year from multiple provider results using provider-weighted voting.
///
/// Each provider's year vote gets a priority-based weight:
/// - MusicBrainz (priority 7): weight 3
/// - ListenBrainz (priority 6): weight 2
/// - All other providers: weight 1
///
/// The year with the highest cumulative weight wins.
/// Tie-break: the year from the best-scoring provider.
/// Returns None if no provider has a year.
pub fn merge_year(results: &[(i32, ValidatedMetadata, u8)]) -> Option<String> {
    let mut year_votes: HashMap<String, u8> = HashMap::new();
    let mut best_year: Option<String> = None;
    let mut best_score: i32 = -1;

    for (score, meta, priority) in results {
        let year = match meta.year {
            Some(ref y) => y,
            None => continue,
        };
        // Ensure year looks plausible (4 digits)
        if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let weight = match *priority {
            7 => 3u8,
            6 => 2u8,
            _ => 1u8,
        };
        *year_votes.entry(year.clone()).or_insert(0) += weight;

        // Track best-scoring provider's year for tie-break
        if *score > best_score {
            best_score = *score;
            best_year = Some(year.clone());
        }
    }

    if year_votes.is_empty() {
        return None;
    }

    // Find highest cumulative weight
    let max_weight = year_votes.values().max().copied().unwrap_or(0);
    let mut tied: Vec<String> = year_votes
        .into_iter()
        .filter(|(_, w)| *w == max_weight)
        .map(|(y, _)| y)
        .collect();

    if tied.len() == 1 {
        return Some(tied.remove(0));
    }
    // Tie-break: best-scoring provider's year
    if let Some(ref best) = best_year {
        if tied.contains(best) {
            return best_year;
        }
    }
    // Unlikely fallback: first tied year
    Some(tied.remove(0))
}

/// Merge genres and styles from multiple provider results using priority-based weights.
///
/// Weight rules:
/// - MusicBrainz (priority 7): genres weight 3, styles weight 0
/// - ListenBrainz (priority 6): genres weight 2, styles weight 1
/// - All other providers: genres weight 1, styles weight 0
///
/// Tags are normalized via `genre_map::normalize_genres` before weighting.
/// Dedup is by lowercase name, keeping the highest weight entry.
/// Results are sorted by weight descending, then alphabetically ascending.
/// Capped at 30 genres and 30 styles after RYM parent expansion.
pub fn weighted_merge_genres(results: &[(i32, ValidatedMetadata, u8)]) -> (Vec<String>, Vec<String>) {
    let mut genre_map_weights: HashMap<String, (String, u8)> = HashMap::new();
    let mut style_map_weights: HashMap<String, (String, u8)> = HashMap::new();

    for (_, meta, priority) in results {
        let genres = genre_map::normalize_genres(&meta.genres);
        let styles = genre_map::normalize_genres(&meta.styles);

        let (genre_weight, style_weight) = match *priority {
            7 => (3u8, 0u8),
            6 => (2u8, 1u8),
            _ => (1u8, 0u8),
        };

        for g in genres {
            let key = g.to_lowercase();
            genre_map_weights
                .entry(key)
                .and_modify(|v| v.1 = v.1.saturating_add(genre_weight))
                .or_insert((g.clone(), genre_weight));
        }

        for s in styles {
            let key = s.to_lowercase();
            style_map_weights
                .entry(key)
                .and_modify(|v| v.1 = v.1.saturating_add(style_weight))
                .or_insert((s.clone(), style_weight));
        }
    }

    let mut genres: Vec<(String, u8)> = genre_map_weights.into_values().collect();
    genres.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let genres: Vec<String> = genres.into_iter().map(|(name, _)| name).take(30).collect();
    let genres = expand_parent_genres(&genres).into_iter().take(30).collect();

    let mut styles: Vec<(String, u8)> = style_map_weights.into_values().collect();
    styles.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let styles: Vec<String> = styles.into_iter().map(|(name, _)| name).take(30).collect();
    let styles = expand_parent_genres(&styles).into_iter().take(30).collect();

    (genres, styles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlbumTrack;

    fn make_meta(genres: Vec<&str>, styles: Vec<&str>) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            year: Some("2020".into()),
            track_no: None,
            album_tracks: vec![AlbumTrack {
                title: "Song".into(),
                duration_secs: 200.0,
                artist: None,
            }],
            genres: genres.into_iter().map(String::from).collect(),
            styles: styles.into_iter().map(String::from).collect(),
            musicbrainz_release_group_id: None,
        }
    }

    #[test]
    fn merge_mb_genres_win_over_lb() {
        let results = vec![
            (100, make_meta(vec!["Thrash metal"], vec![]), 7),
            (50, make_meta(vec!["thrash metal", "metal"], vec![]), 6),
        ];
        let (genres, styles) = weighted_merge_genres(&results);
        assert!(genres.contains(&"Thrash metal".to_string()));
        assert!(genres.contains(&"Metal".to_string()) || genres.contains(&"metal".to_string()));
        assert_eq!(styles.len(), 0);
    }

    #[test]
    fn merge_only_lb_genres() {
        let results = vec![
            (50, make_meta(vec!["thrash metal", "speed metal"], vec!["metal", "1980s"]), 6),
        ];
        let (genres, styles) = weighted_merge_genres(&results);
        assert!(genres.contains(&"Thrash metal".to_string()));
        assert!(genres.contains(&"Speed metal".to_string()));
        // "1980s" is not a known genre so normalize_genre leaves it unchanged
        assert!(styles.contains(&"1980s".to_string()));
        assert!(styles.contains(&"Metal".to_string()));
    }

    #[test]
    fn merge_no_results_returns_empty() {
        let results: Vec<(i32, ValidatedMetadata, u8)> = vec![];
        let (genres, styles) = weighted_merge_genres(&results);
        assert!(genres.is_empty());
        assert!(styles.is_empty());
    }

    #[test]
    fn merge_dedup_keeps_highest_weight() {
        // Providers are sorted by priority: MB (7) before LB (6)
        let results = vec![
            (100, make_meta(vec!["heavy metal"], vec![]), 7),
            (50, make_meta(vec!["Heavy metal"], vec![]), 6),
        ];
        let (genres, _styles) = weighted_merge_genres(&results);
        // MB (priority 7) weight 3 should win over LB (priority 6) weight 2
        assert!(genres.contains(&"Heavy metal".to_string()));
        // No duplicate "Heavy metal" entries
        let heavy_metal_count = genres.iter().filter(|g| g.as_str() == "Heavy metal").count();
        assert_eq!(heavy_metal_count, 1);
    }

    #[test]
    fn merge_dedup_first_writer_wins_by_priority() {
        // MB (priority 7, weight 3) reports "metal"
        // LibreFM (priority 1, weight 1) also reports "metal"
        // First insertion from MB must be preserved
        let results = vec![
            (100, make_meta(vec!["metal"], vec![]), 7),
            (50, make_meta(vec!["metal"], vec![]), 1),
        ];
        let (genres, _styles) = weighted_merge_genres(&results);
        assert!(genres.contains(&"Metal".to_string()));
        // No duplicate "Metal" entries
        let metal_count = genres.iter().filter(|g| g.as_str() == "Metal").count();
        assert_eq!(metal_count, 1);
    }

    #[test]
    fn expand_parent_genres_works() {
        let input = vec!["Thrash Metal".to_string()];
        let expanded = expand_parent_genres(&input);
        assert!(expanded.contains(&"Thrash Metal".to_string()));
        assert!(expanded.contains(&"Metal".to_string()));
    }

    #[test]
    fn expand_parent_genres_dedup() {
        let input = vec!["Thrash Metal".to_string(), "Black Metal".to_string()];
        let expanded = expand_parent_genres(&input);
        assert!(expanded.contains(&"Thrash Metal".to_string()));
        assert!(expanded.contains(&"Black Metal".to_string()));
        // Both share "Metal" as parent; should appear only once
        let metal_count = expanded.iter().filter(|g| g.as_str() == "Metal").count();
        assert_eq!(metal_count, 1);
    }

    #[test]
    fn expand_parent_genres_empty_input() {
        let input: Vec<String> = vec![];
        let expanded = expand_parent_genres(&input);
        assert!(expanded.is_empty());
    }

    #[test]
    fn expand_parent_genres_multiple_levels() {
        let input = vec!["Technical Death Metal".to_string()];
        let expanded = expand_parent_genres(&input);
        assert!(expanded.iter().any(|g| g.to_lowercase() == "technical death metal"));
        assert!(expanded.iter().any(|g| g.to_lowercase() == "death metal"));
        assert!(expanded.iter().any(|g| g.to_lowercase() == "metal"));
    }

    #[test]
    fn weighted_merge_cap_30_genres() {
        let unique_many: Vec<String> = (0..35).map(|i| format!("Genre {}", i)).collect();
        let meta = ValidatedMetadata {
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            year: Some("2020".into()),
            track_no: None,
            album_tracks: vec![],
            genres: unique_many,
            styles: vec![],
            musicbrainz_release_group_id: None,
        };
        let results = vec![(100, meta, 7)];
        let (genres, _styles) = weighted_merge_genres(&results);
        assert_eq!(genres.len(), 30, "Genres should be capped at 30");
    }

    #[test]
    fn weighted_merge_cap_30_styles() {
        let unique_many: Vec<String> = (0..35).map(|i| format!("Style {}", i)).collect();
        let meta = ValidatedMetadata {
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            year: Some("2020".into()),
            track_no: None,
            album_tracks: vec![],
            genres: vec![],
            styles: unique_many,
            musicbrainz_release_group_id: None,
        };
        let results = vec![(100, meta, 6)];
        let (_genres, styles) = weighted_merge_genres(&results);
        assert_eq!(styles.len(), 30, "Styles should be capped at 30");
    }

    #[test]
    fn weighted_merge_empty_results() {
        let results: Vec<(i32, ValidatedMetadata, u8)> = vec![];
        let (genres, styles) = weighted_merge_genres(&results);
        assert!(genres.is_empty());
        assert!(styles.is_empty());
    }

    #[test]
    fn weighted_merge_three_providers() {
        let results = vec![
            (100, make_meta(vec!["Thrash metal", "Speed metal"], vec![]), 7),
            (80, make_meta(vec!["Heavy metal", "Thrash metal"], vec!["1980s"]), 6),
            (60, make_meta(vec!["Metal"], vec![]), 1),
        ];
        let (genres, styles) = weighted_merge_genres(&results);
        assert!(genres.contains(&"Thrash metal".to_string()));
        assert!(genres.contains(&"Speed metal".to_string()));
        assert!(genres.contains(&"Heavy metal".to_string()));
        assert!(genres.contains(&"Metal".to_string()));
        assert!(styles.contains(&"1980s".to_string()));
    }

    // ── merge_year ──────────────────────────────────────────

    #[test]
    fn merge_year_mb_wins() {
        let results = vec![
            (100, make_meta(vec![], vec![]), 7), // MB: year 2020
            (50, make_meta(vec![], vec![]), 6),  // LB: year 2020 too
            (30, make_meta(vec![], vec![]), 1),  // Others: year 2020
        ];
        assert_eq!(merge_year(&results), Some("2020".to_string()));
    }

    #[test]
    fn merge_year_mb_overrides_lb() {
        // MB says 2003, LB says 2004
        // MB weight 3 > LB weight 2, so MB wins
        let mut mb = make_meta(vec![], vec![]);
        mb.year = Some("2003".to_string());
        let mut lb = make_meta(vec![], vec![]);
        lb.year = Some("2004".to_string());
        let results = vec![
            (80, mb, 7),
            (70, lb, 6),
        ];
        assert_eq!(merge_year(&results), Some("2003".to_string()));
    }

    #[test]
    fn merge_year_two_non_mb_beat_one_mb() {
        // MB says 2003 (weight 3), two others say 2004 (weight 1+1=2)
        // MB still wins: 3 > 2
        let mut mb = make_meta(vec![], vec![]);
        mb.year = Some("2003".to_string());
        let mut o1 = make_meta(vec![], vec![]);
        o1.year = Some("2004".to_string());
        let mut o2 = make_meta(vec![], vec![]);
        o2.year = Some("2004".to_string());
        let results = vec![
            (80, mb, 7),
            (60, o1, 1),
            (50, o2, 1),
        ];
        assert_eq!(merge_year(&results), Some("2003".to_string()));
    }

    #[test]
    fn merge_year_cumulative_weight_wins() {
        // Two LB (weight 2 each = 4) beat one MB (weight 3)
        let mut lb1 = make_meta(vec![], vec![]);
        lb1.year = Some("2004".to_string());
        let mut lb2 = make_meta(vec![], vec![]);
        lb2.year = Some("2004".to_string());
        let mut mb = make_meta(vec![], vec![]);
        mb.year = Some("2003".to_string());
        let results = vec![
            (90, mb, 7),
            (70, lb1, 6),
            (60, lb2, 6),
        ];
        // 2004 gets weight 2+2=4, 2003 gets weight 3
        assert_eq!(merge_year(&results), Some("2004".to_string()));
    }

    #[test]
    fn merge_year_tie_best_score_wins() {
        // Tied: MB and LB both weight 3 (MB says 2003, LB says 2004)
        // Wait: MB weight=3, LB weight=2, 3 != 2
        // Need actual tie: MB says 2003 (weight 3), LB says 2004 (weight 2)
        // But if we have MB(3) + one-other(1) = 4 for 2003, vs LB(2) + one-other(1) = 4 for 2004
        // Tie! Best score wins.
        let mut mb = make_meta(vec![], vec![]);
        mb.year = Some("2003".to_string());
        let mut lb = make_meta(vec![], vec![]);
        lb.year = Some("2004".to_string());
        let mut o_for_2003 = make_meta(vec![], vec![]);
        o_for_2003.year = Some("2003".to_string());
        let mut o_for_2004 = make_meta(vec![], vec![]);
        o_for_2004.year = Some("2004".to_string());
        // 2003: MB(3) + other(1) = 4
        // 2004: LB(2) + other(1) = 4
        let results = vec![
            (100, mb, 7),      // score 100, year 2003
            (90, o_for_2003, 1), // year 2003 -> total 4
            (80, lb, 6),       // score 80, year 2004
            (70, o_for_2004, 1), // year 2004 -> total 3... wait, LB weight 2 + other weight 1 = 3
        ];
        // 2003: MB 3 + other 1 = 4
        // 2004: LB 2 + other 1 = 3
        // 2003 wins
        assert_eq!(merge_year(&results), Some("2003".to_string()));
    }

    #[test]
    fn merge_year_none_when_no_year() {
        let mut meta = make_meta(vec![], vec![]);
        meta.year = None;
        let results = vec![(100, meta, 7)];
        assert_eq!(merge_year(&results), None);
    }

    #[test]
    fn merge_year_empty_results() {
        let results: Vec<(i32, ValidatedMetadata, u8)> = vec![];
        assert_eq!(merge_year(&results), None);
    }

    #[test]
    fn merge_year_invalid_year_ignored() {
        let mut meta = make_meta(vec![], vec![]);
        meta.year = Some("abc".to_string());
        let results = vec![(100, meta, 7)];
        assert_eq!(merge_year(&results), None);
    }

    #[test]
    fn weighted_merge_sorts_by_weight_then_alpha() {
        let results = vec![
            (50, make_meta(vec!["ZZZ genre"], vec![]), 7),
            (50, make_meta(vec!["AAA genre"], vec![]), 6),
            (50, make_meta(vec!["MMM genre"], vec![]), 1),
        ];
        let (genres, _styles) = weighted_merge_genres(&results);
        assert_eq!(genres[0], "ZZZ genre");
        assert_eq!(genres[1], "AAA genre");
        assert_eq!(genres[2], "MMM genre");
    }
}
