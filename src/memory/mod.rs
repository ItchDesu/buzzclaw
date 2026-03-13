use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// A single memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub category: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// SQLite-backed persistent memory store.
pub struct MemoryStore {
    conn: Mutex<Connection>,
}

impl MemoryStore {
    /// Open (or create) the memory database at the given path.
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let path = db_path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("opening memory db at {}", path.display()))?;

        // WAL mode for concurrent-safe reads
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                key         TEXT NOT NULL,
                category    TEXT NOT NULL DEFAULT 'general',
                value       TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL,
                PRIMARY KEY (key, category)
            );
            CREATE INDEX IF NOT EXISTS memories_category ON memories (category);",
        )?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Store or update a memory entry.
    pub fn store(&self, key: &str, value: &str, category: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (key, category, value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(key, category) DO UPDATE SET
               value = excluded.value,
               updated_at = excluded.updated_at",
            params![key, category, value, now],
        )?;
        Ok(())
    }

    /// Retrieve a memory entry by exact key (searches all categories).
    pub fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, value, category, created_at, updated_at
             FROM memories WHERE key = ?1 ORDER BY updated_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let entry = MemoryEntry {
                key: row.get(0)?,
                value: row.get(1)?,
                category: row.get(2)?,
                created_at: parse_dt(row.get::<_, String>(3)?.as_str()),
                updated_at: parse_dt(row.get::<_, String>(4)?.as_str()),
            };
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    /// Search memories by substring match on key or value.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{query}%");
        let mut stmt = conn.prepare(
            "SELECT key, value, category, created_at, updated_at
             FROM memories
             WHERE key LIKE ?1 OR value LIKE ?1
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let entries = stmt
            .query_map(params![pattern, limit as i64], |row| {
                Ok(MemoryEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    category: row.get(2)?,
                    created_at: parse_dt(row.get::<_, String>(3)?.as_str()),
                    updated_at: parse_dt(row.get::<_, String>(4)?.as_str()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    /// List all entries, optionally filtered by category.
    pub fn list(&self, category: Option<&str>, limit: usize, offset: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        if let Some(cat) = category {
            let mut stmt = conn.prepare(
                "SELECT key, value, category, created_at, updated_at
                 FROM memories WHERE category = ?1
                 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3",
            )?;
            let entries: Vec<MemoryEntry> = stmt
                .query_map(params![cat, limit as i64, offset as i64], row_to_entry)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(entries)
        } else {
            let mut stmt = conn.prepare(
                "SELECT key, value, category, created_at, updated_at
                 FROM memories ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
            )?;
            let entries: Vec<MemoryEntry> = stmt
                .query_map(params![limit as i64, offset as i64], row_to_entry)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(entries)
        }
    }

    /// Delete a memory entry.
    #[allow(dead_code)]
    pub fn forget(&self, key: &str, category: Option<&str>) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = if let Some(cat) = category {
            conn.execute(
                "DELETE FROM memories WHERE key = ?1 AND category = ?2",
                params![key, cat],
            )?
        } else {
            conn.execute("DELETE FROM memories WHERE key = ?1", params![key])?
        };
        Ok(deleted)
    }

    /// Total memory entry count.
    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Clear all entries in a category, or all entries if category is None.
    pub fn clear(&self, category: Option<&str>) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = if let Some(cat) = category {
            conn.execute("DELETE FROM memories WHERE category = ?1", params![cat])?
        } else {
            conn.execute("DELETE FROM memories", [])?
        };
        Ok(deleted)
    }
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEntry> {
    Ok(MemoryEntry {
        key: row.get(0)?,
        value: row.get(1)?,
        category: row.get(2)?,
        created_at: parse_dt(row.get::<_, String>(3)?.as_str()),
        updated_at: parse_dt(row.get::<_, String>(4)?.as_str()),
    })
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
