//! Persistent SQLite-backed cache for F4 recommendations (survives app restarts).
use crate::lastfm_recommend::RecItem;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

const TTL_SECS: i64 = 24 * 60 * 60; // 24h rotation like Last.fm homepage

pub struct RecommendationStore { conn: Connection }

impl RecommendationStore {
    pub fn open_default() -> Option<Self> {
        let dir = crate::get_data_dir().ok()?;
        Some(Self::open(dir.join("recommendations_cache.db")).ok()?)
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("open recommendation cache at {:?}", path.as_ref()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS recommendations_cache (
                cache_key TEXT PRIMARY KEY,
                items_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }
    /// Return cached items if present AND fetched within the last 24h. None otherwise/on error.
    pub fn load(&self, cache_key: &str) -> Option<Vec<RecItem>> {
        let row: Option<(String, i64)> = self.conn
            .query_row(
                "SELECT items_json, fetched_at FROM recommendations_cache WHERE cache_key = ?1",
                params![cache_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).ok();
        let (items_json, fetched_at) = row?;
        if (now_secs() - fetched_at) >= TTL_SECS { return None; }
        serde_json::from_str::<Vec<RecItem>>(&items_json).ok()
    }
    /// Upsert cached items with current timestamp.
    pub fn save(&self, cache_key: &str, items: &[RecItem]) -> Result<()> {
        let items_json = serde_json::to_string(items)?;
        self.conn.execute(
            "INSERT INTO recommendations_cache (cache_key, items_json, fetched_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(cache_key) DO UPDATE SET items_json = excluded.items_json, fetched_at = excluded.fetched_at",
            params![cache_key, items_json, now_secs()],
        )?;
        Ok(())
    }
    pub fn clear(&self, cache_key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM recommendations_cache WHERE cache_key = ?1", params![cache_key])?;
        Ok(())
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
