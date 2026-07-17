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

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track_no: Option<usize>,
    pub album_tracks: Vec<AlbumTrack>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    pub musicbrainz_release_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumTrack {
    pub title: String,
    pub duration_secs: f64,
    pub artist: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// SqliteCache
// ---------------------------------------------------------------------------

pub struct SqliteCache {
    conn: Connection,
}

impl SqliteCache {
    /// Open (or create) the SQLite database at `path` and ensure tables exist.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let conn = Connection::open(path.as_ref())?;
        let cache = Self { conn };
        cache.create_tables()?;
        Ok(cache)
    }

    /// Open an in-memory SQLite database (for testing).
    pub fn open_in_memory() -> Result<Self, CacheError> {
        let conn = Connection::open_in_memory()?;
        let cache = Self { conn };
        cache.create_tables()?;
        Ok(cache)
    }

    fn create_tables(&self) -> Result<(), CacheError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata_cache (
                cache_key   TEXT PRIMARY KEY,
                artist      TEXT,
                album       TEXT,
                year        TEXT,
                genres      TEXT,
                styles      TEXT,
                album_tracks TEXT,
                created_at  INT NOT NULL,
                accessed_at INT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cover_art_cache (
                mbid       TEXT PRIMARY KEY,
                artist     TEXT,
                album      TEXT,
                image_data BLOB,
                mime_type  TEXT,
                fetched_at INT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS caa_cache (
                release_mbid TEXT PRIMARY KEY,
                image_data   BLOB,
                not_found    BOOLEAN DEFAULT 0,
                fetched_at   INT NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Retrieve cached CAA image bytes by release MBID.
    /// Returns None if not cached, if the entry is marked not_found and
    /// has not expired (7-day TTL), or if the not_found entry has expired.
    pub fn get_caa_art(&self, mbid: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let now = now_secs();
        let row: std::result::Result<(Vec<u8>, bool, i64), rusqlite::Error> = self.conn.query_row(
            "SELECT image_data, not_found, fetched_at FROM caa_cache WHERE release_mbid = ?1",
            params![mbid],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0).unwrap_or_default(),
                    row.get::<_, bool>(1).unwrap_or(false),
                    row.get::<_, i64>(2).unwrap_or(0),
                ))
            },
        );

        match row {
            Ok((data, not_found, fetched_at)) => {
                if not_found {
                    // not_found entries expire after 7 days
                    let seven_days_secs = 7 * 24 * 60 * 60i64;
                    if now - fetched_at < seven_days_secs {
                        // Still within TTL — treat as not-found
                        return Ok(None);
                    }
                    // Expired — caller can retry
                    return Ok(None);
                }
                if data.is_empty() {
                    return Ok(None);
                }
                Ok(Some(data))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    /// Cache CAA image data for a release MBID.
    /// If `data` is None, marks the entry as not_found (so repeated lookups
    /// skip the network for 7 days).
    pub fn set_caa_art(&self, mbid: &str, data: Option<&[u8]>) -> Result<(), CacheError> {
        let now = now_secs();
        match data {
            Some(bytes) => {
                self.conn.execute(
                    "INSERT OR REPLACE INTO caa_cache (release_mbid, image_data, not_found, fetched_at)
                     VALUES (?1, ?2, 0, ?3)",
                    params![mbid, bytes, now],
                )?;
            }
            None => {
                self.conn.execute(
                    "INSERT OR REPLACE INTO caa_cache (release_mbid, image_data, not_found, fetched_at)
                     VALUES (?1, NULL, 1, ?2)",
                    params![mbid, now],
                )?;
            }
        }
        Ok(())
    }

    /// Retrieve cached metadata by cache key. Returns None if not found.
    pub fn get(&self, key: &str) -> Option<ValidatedMetadata> {
        let now = now_secs();
        let row: SqlResult<(String, Option<String>, Option<String>, Option<String>,
                           Option<String>, Option<String>, Option<String>, i64, i64)> =
            self.conn.query_row(
                "SELECT cache_key, artist, album, year, genres, styles, album_tracks,
                        created_at, accessed_at
                 FROM metadata_cache WHERE cache_key = ?1",
                params![key],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            );

        match row {
            Ok((_, artist, album, year, genres_json, styles_json, tracks_json, _created, _accessed)) => {
                // Update accessed_at
                let _ = self.conn.execute(
                    "UPDATE metadata_cache SET accessed_at = ?1 WHERE cache_key = ?2",
                    params![now, key],
                );

                let genres: Vec<String> = genres_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default();
                let styles: Vec<String> = styles_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default();
                let album_tracks: Vec<AlbumTrack> = tracks_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default();

                Some(ValidatedMetadata {
                    artist,
                    album,
                    year,
                    track_no: None,
                    album_tracks,
                    genres,
                    styles,
                    musicbrainz_release_group_id: None,
                })
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(_) => None,
        }
    }

    /// Insert or replace cached metadata.
    pub fn put(&self, key: &str, meta: &ValidatedMetadata) -> Result<(), CacheError> {
        let now = now_secs();
        let genres_json = serde_json::to_string(&meta.genres)?;
        let styles_json = serde_json::to_string(&meta.styles)?;
        let tracks_json = serde_json::to_string(&meta.album_tracks)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO metadata_cache
             (cache_key, artist, album, year, genres, styles, album_tracks, created_at, accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                key,
                meta.artist,
                meta.album,
                meta.year,
                genres_json,
                styles_json,
                tracks_json,
                now,
            ],
        )?;
        Ok(())
    }

    /// Delete a single entry by cache key.
    pub fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.conn
            .execute("DELETE FROM metadata_cache WHERE cache_key = ?1", params![key])?;
        Ok(())
    }

    /// Delete all entries from the cache.
    pub fn clear(&self) -> Result<(), CacheError> {
        self.conn.execute_batch("DELETE FROM metadata_cache")?;
        Ok(())
    }

    /// Return the number of cached entries.
    pub fn len(&self) -> Result<usize, CacheError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM metadata_cache", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Return true if the cache is empty.
    pub fn is_empty(&self) -> Result<bool, CacheError> {
        self.len().map(|n| n == 0)
    }

    /// Iterate over all (cache_key, metadata) pairs.
    pub fn iter(&self) -> Result<Vec<(String, ValidatedMetadata)>, CacheError> {
        let mut stmt = self.conn.prepare(
            "SELECT cache_key, artist, album, year, genres, styles, album_tracks
             FROM metadata_cache",
        )?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let artist: Option<String> = row.get(1)?;
            let album: Option<String> = row.get(2)?;
            let year: Option<String> = row.get(3)?;
            let genres_json: Option<String> = row.get(4)?;
            let styles_json: Option<String> = row.get(5)?;
            let tracks_json: Option<String> = row.get(6)?;

            let genres: Vec<String> = genres_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();
            let styles: Vec<String> = styles_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();
            let album_tracks: Vec<AlbumTrack> = tracks_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();

            Ok((
                key,
                ValidatedMetadata {
                    artist,
                    album,
                    year,
                    track_no: None,
                    album_tracks,
                    genres,
                    styles,
                    musicbrainz_release_group_id: None,
                },
            ))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Close the database connection.
    pub fn close(self) -> Result<(), CacheError> {
        self.conn.close().map_err(|(_, e)| CacheError::Sql(e))?;
        Ok(())
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta(artist: &str, album: &str, year: &str, n_tracks: usize) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: Some(artist.to_string()),
            album: Some(album.to_string()),
            year: Some(year.to_string()),
            track_no: None,
            album_tracks: (0..n_tracks)
                .map(|i| AlbumTrack {
                    title: format!("Track {}", i + 1),
                    duration_secs: 100.0 + i as f64,
                    artist: Some(artist.to_string()),
                })
                .collect(),
            genres: vec!["Rock".to_string()],
            styles: vec!["Hard Rock".to_string()],
            musicbrainz_release_group_id: None,
        }
    }

    #[test]
    fn test_open_in_memory_creates_tables() {
        let cache = SqliteCache::open_in_memory().expect("open in memory");
        // Tables exist if no error from a query on them
        let count: i64 = cache
            .conn
            .query_row("SELECT COUNT(*) FROM metadata_cache", [], |row| row.get(0))
            .expect("metadata_cache table exists");
        assert_eq!(count, 0);
        let count2: i64 = cache
            .conn
            .query_row("SELECT COUNT(*) FROM cover_art_cache", [], |row| row.get(0))
            .expect("cover_art_cache table exists");
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_put_and_get_roundtrip() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let key = "metallica::master of puppets";
        let meta = make_meta("Metallica", "Master of Puppets", "1986", 8);
        cache.put(key, &meta).expect("put");
        let got = cache.get(key);
        assert!(got.is_some());
        let got = got.unwrap();
        assert_eq!(got.artist, Some("Metallica".to_string()));
        assert_eq!(got.album, Some("Master of Puppets".to_string()));
        assert_eq!(got.year, Some("1986".to_string()));
        assert_eq!(got.album_tracks.len(), 8);
        assert_eq!(got.genres, vec!["Rock"]);
        assert_eq!(got.styles, vec!["Hard Rock"]);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let got = cache.get("nonexistent::key");
        assert!(got.is_none());
    }

    #[test]
    fn test_delete_removes_entry() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let key = "test::delete";
        cache.put(key, &make_meta("A", "B", "2020", 0)).expect("put");
        assert!(cache.get(key).is_some());
        cache.delete(key).expect("delete");
        assert!(cache.get(key).is_none());
    }

    #[test]
    fn test_clear_empties_all() {
        let cache = SqliteCache::open_in_memory().expect("open");
        cache.put("k1", &make_meta("A", "B", "2020", 0)).expect("put");
        cache.put("k2", &make_meta("C", "D", "2021", 0)).expect("put");
        assert_eq!(cache.len().unwrap(), 2);
        cache.clear().expect("clear");
        assert_eq!(cache.len().unwrap(), 0);
    }

    #[test]
    fn test_len_counts_entries() {
        let cache = SqliteCache::open_in_memory().expect("open");
        assert_eq!(cache.len().unwrap(), 0);
        cache.put("k1", &make_meta("A", "B", "2020", 0)).expect("put");
        assert_eq!(cache.len().unwrap(), 1);
        cache.put("k2", &make_meta("C", "D", "2021", 0)).expect("put");
        assert_eq!(cache.len().unwrap(), 2);
    }

    #[test]
    fn test_is_empty() {
        let cache = SqliteCache::open_in_memory().expect("open");
        assert!(cache.is_empty().unwrap());
        cache.put("k1", &make_meta("A", "B", "2020", 0)).expect("put");
        assert!(!cache.is_empty().unwrap());
    }

    #[test]
    fn test_iter_returns_all_entries() {
        let cache = SqliteCache::open_in_memory().expect("open");
        cache.put("k1", &make_meta("A", "B1", "2020", 1)).expect("put");
        cache.put("k2", &make_meta("C", "B2", "2021", 2)).expect("put");
        let entries = cache.iter().expect("iter");
        assert_eq!(entries.len(), 2);
        let keys: Vec<String> = entries.iter().map(|(k, _)| k.clone()).collect();
        assert!(keys.contains(&"k1".to_string()));
        assert!(keys.contains(&"k2".to_string()));
    }

    #[test]
    fn test_close_succeeds() {
        let cache = SqliteCache::open_in_memory().expect("open");
        cache.close().expect("close");
    }

    #[test]
    fn test_open_file_based_db() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_metadata_cache.sqlite");
        // Clean up from previous runs
        let _ = std::fs::remove_file(&path);
        {
            let cache = SqliteCache::open(&path).expect("open file DB");
            cache.put("k1", &make_meta("A", "B", "2020", 0)).expect("put");
            assert_eq!(cache.len().unwrap(), 1);
            cache.close().expect("close");
        }
        // Re-open and verify persistence
        {
            let cache = SqliteCache::open(&path).expect("re-open file DB");
            assert_eq!(cache.len().unwrap(), 1);
            let got = cache.get("k1");
            assert!(got.is_some());
            assert_eq!(got.unwrap().artist, Some("A".to_string()));
            cache.close().expect("close");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_put_overwrites_existing_key() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let key = "test::overwrite";
        cache
            .put(key, &make_meta("A", "First", "2020", 0))
            .expect("put first");
        cache
            .put(key, &make_meta("B", "Second", "2021", 1))
            .expect("put second");
        let got = cache.get(key).expect("get after overwrite");
        assert_eq!(got.artist, Some("B".to_string()));
        assert_eq!(got.album, Some("Second".to_string()));
        assert_eq!(got.album_tracks.len(), 1);
    }

    // -----------------------------------------------------------------------
    // CAA cache tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_caa_cache_put_get_roundtrip() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let mbid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let data: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]; // PNG header

        cache.set_caa_art(mbid, Some(&data)).expect("set caa art");
        let got = cache.get_caa_art(mbid).expect("get caa art");
        assert_eq!(got, Some(data));
    }

    #[test]
    fn test_caa_cache_not_found_entry() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let mbid = "bbbbbbbb-cccc-dddd-eeee-ffffffffffff";

        // Mark as not_found
        cache.set_caa_art(mbid, None).expect("set caa art not_found");
        let got = cache.get_caa_art(mbid).expect("get caa art");
        // Should return None because not_found and not expired yet
        assert!(got.is_none());
    }

    #[test]
    fn test_caa_cache_not_found_ttl_expiry() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let mbid = "cccccccc-dddd-eeee-ffff-aaaaaaaaaaaa";

        // Manually insert with fetched_at = 8 days ago (expired)
        let eight_days_ago = now_secs() - 8 * 24 * 60 * 60;
        cache
            .conn
            .execute(
                "INSERT INTO caa_cache (release_mbid, image_data, not_found, fetched_at)
                 VALUES (?1, NULL, 1, ?2)",
                params![mbid, eight_days_ago],
            )
            .expect("insert expired not_found entry");

        // Should return None (entry expired, caller should retry)
        let got = cache.get_caa_art(mbid).expect("get caa art");
        assert!(got.is_none());
    }

    #[test]
    fn test_caa_cache_found_entry_never_expires() {
        let cache = SqliteCache::open_in_memory().expect("open");
        let mbid = "dddddddd-eeee-ffff-aaaa-bbbbbbbbbbbb";
        let data: Vec<u8> = vec![0xff, 0xd8, 0xff, 0xe0]; // JPEG header

        // Manually insert with fetched_at = 30 days ago (long past)
        let thirty_days_ago = now_secs() - 30 * 24 * 60 * 60;
        cache
            .conn
            .execute(
                "INSERT INTO caa_cache (release_mbid, image_data, not_found, fetched_at)
                 VALUES (?1, ?2, 0, ?3)",
                params![mbid, data, thirty_days_ago],
            )
            .expect("insert old found entry");

        // Found entries never expire
        let got = cache.get_caa_art(mbid).expect("get caa art");
        assert_eq!(got, Some(vec![0xff, 0xd8, 0xff, 0xe0]));
    }

    #[test]
    fn test_caa_cache_missing_table_does_not_crash() {
        // Open without create_tables to simulate missing table
        let conn = rusqlite::Connection::open_in_memory().expect("open in memory");
        let cache = SqliteCache { conn };
        // Don't call create_tables — caa_cache doesn't exist

        // Both methods should gracefully handle missing table
        let got = cache.get_caa_art("some-mbid").expect("get caa art");
        assert!(got.is_none());

        // Also verify after creating tables it works
        cache.create_tables().expect("create tables");
        let mbid = "eeeeeeee-ffff-aaaa-bbbb-cccccccccccc";
        let data: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47];
        cache.set_caa_art(mbid, Some(&data)).expect("set caa art");
        let got = cache.get_caa_art(mbid).expect("get caa art");
        assert_eq!(got, Some(data));
    }
}
