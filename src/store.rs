use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

pub const DEFAULT_KEEP: &str = "1d";

/// Text-only samples for a brand-new history database.
pub const SEED_CLIPS: &[(&str, &str)] = &[
    (
        "← → select a clip. Enter pastes it into the last app. Esc closes the bar.",
        "forever",
    ),
    (
        "Type to search, or click the magnifying glass. Ctrl+K cycles how long a clip is kept.",
        "forever",
    ),
    ("https://omarchy.org", "7d"),
    (
        "https://github.com/pkayokay/omapaste/issues",
        "forever",
    ),
    ("omarchy theme list", "7d"),
    (
        "fn greet(name: &str) -> String {\n    format!(\"hello, {name}\")\n}",
        "7d",
    ),
    ("ssh -o StrictHostKeyChecking=accept-new git@github.com", "7d"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeepPreset {
    pub key: &'static str,
    pub label: &'static str,
    pub seconds: Option<i64>,
}

pub const KEEP_PRESETS: [KeepPreset; 4] = [
    KeepPreset {
        key: "1h",
        label: "1 hour",
        seconds: Some(60 * 60),
    },
    KeepPreset {
        key: "1d",
        label: "1 day",
        seconds: Some(60 * 60 * 24),
    },
    KeepPreset {
        key: "7d",
        label: "7 days",
        seconds: Some(60 * 60 * 24 * 7),
    },
    KeepPreset {
        key: "forever",
        label: "Forever",
        seconds: None,
    },
];

pub fn keep_by_key(key: &str) -> Option<KeepPreset> {
    KEEP_PRESETS.iter().copied().find(|p| p.key == key)
}

pub fn next_keep(current: &str) -> KeepPreset {
    let index = KEEP_PRESETS
        .iter()
        .position(|p| p.key == current)
        .unwrap_or(1);
    KEEP_PRESETS[(index + 1) % KEEP_PRESETS.len()]
}

pub fn keep_until_from(preset: &str, now: i64) -> Option<i64> {
    let spec = keep_by_key(preset).unwrap_or(KEEP_PRESETS[1]);
    spec.seconds.map(|s| now + s)
}

pub fn content_hash(kind: &str, mime: &str, payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update([0u8]);
    digest.update(mime.as_bytes());
    digest.update([0u8]);
    digest.update(payload);
    hex::encode(digest.finalize())
}

pub fn make_preview(text: &str, limit: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= limit {
        collapsed
    } else {
        let mut cut = collapsed
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        cut.push('…');
        cut
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub id: i64,
    pub created_at: i64,
    pub last_used_at: i64,
    pub keep_preset: String,
    pub keep_until: Option<i64>,
    pub mime: String,
    pub kind: String,
    pub text: Option<String>,
    pub preview: String,
    pub hash: String,
    pub image_path: Option<String>,
    pub byte_size: i64,
}

impl Clip {
    pub fn keep_label(&self) -> String {
        keep_by_key(&self.keep_preset)
            .map(|p| p.label.to_string())
            .unwrap_or_else(|| self.keep_preset.clone())
    }

    pub fn keep_short(&self) -> &str {
        if self.keep_preset == "forever" {
            "∞"
        } else {
            &self.keep_preset
        }
    }

    pub fn kind_label(&self) -> String {
        match self.kind.as_str() {
            "image" => "Image".into(),
            "text" => "Text".into(),
            other => other.replace('_', " "),
        }
    }

    pub fn format_chars(&self) -> String {
        if self.kind == "image" {
            let size = self.byte_size;
            if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                let kb = size as f64 / 1024.0;
                if kb < 10.0 {
                    format!("{kb:.1} KB")
                } else {
                    format!("{kb:.0} KB")
                }
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            }
        } else {
            let n = self.text.as_deref().map(|t| t.chars().count()).unwrap_or(0);
            if n == 1 {
                "1 char".into()
            } else {
                format!("{n} chars")
            }
        }
    }
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY,
                created_at INTEGER NOT NULL,
                last_used_at INTEGER NOT NULL,
                keep_preset TEXT NOT NULL,
                keep_until INTEGER,
                mime TEXT NOT NULL,
                kind TEXT NOT NULL,
                text TEXT,
                preview TEXT NOT NULL,
                hash TEXT NOT NULL UNIQUE,
                image_path TEXT,
                byte_size INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_clips_last_used ON clips(last_used_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clips_keep_until ON clips(keep_until);
            ",
        )?;
        Ok(Self { conn })
    }

    pub fn add(
        &self,
        kind: &str,
        mime: &str,
        payload: &[u8],
        text: Option<&str>,
        preview: &str,
        image_path: Option<&str>,
        keep_preset: &str,
        max_items: i64,
        now: Option<i64>,
    ) -> rusqlite::Result<Option<Clip>> {
        if payload.is_empty() {
            return Ok(None);
        }
        let stamp = now.unwrap_or_else(now_secs);
        let digest = content_hash(kind, mime, payload);
        let until = keep_until_from(keep_preset, stamp);
        if let Some(id) = self
            .conn
            .query_row("SELECT id FROM clips WHERE hash = ?", [&digest], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
        {
            self.conn.execute(
                "UPDATE clips SET last_used_at = ? WHERE id = ?",
                params![stamp, id],
            )?;
            return self.get(id);
        }
        self.conn.execute(
            "INSERT INTO clips (
                created_at, last_used_at, keep_preset, keep_until,
                mime, kind, text, preview, hash, image_path, byte_size
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                stamp,
                stamp,
                keep_preset,
                until,
                mime,
                kind,
                text,
                preview,
                digest,
                image_path,
                payload.len() as i64,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.prune(max_items, Some(stamp))?;
        self.get(id)
    }

    pub fn get(&self, clip_id: i64) -> rusqlite::Result<Option<Clip>> {
        self.conn
            .query_row("SELECT * FROM clips WHERE id = ?", [clip_id], row_clip)
            .optional()
    }

    pub fn list(&self, query: &str, now: Option<i64>) -> rusqlite::Result<Vec<Clip>> {
        let stamp = now.unwrap_or_else(now_secs);
        let mut stmt = self.conn.prepare(
            "SELECT * FROM clips
             WHERE (keep_until IS NULL OR keep_until > ?)
             ORDER BY last_used_at DESC, id DESC",
        )?;
        let clips: Vec<Clip> = {
            let mapped = stmt.query_map([stamp], row_clip)?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(clips);
        }
        Ok(clips
            .into_iter()
            .filter(|c| {
                c.preview.to_lowercase().contains(&needle)
                    || c.text
                        .as_deref()
                        .map(|t| t.to_lowercase().contains(&needle))
                        .unwrap_or(false)
            })
            .collect())
    }

    pub fn delete(&self, clip_id: i64) -> rusqlite::Result<Option<PathBuf>> {
        let image: Option<String> = self
            .conn
            .query_row(
                "SELECT image_path FROM clips WHERE id = ?",
                [clip_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        self.conn
            .execute("DELETE FROM clips WHERE id = ?", [clip_id])?;
        Ok(image.map(PathBuf::from))
    }

    pub fn set_keep(
        &self,
        clip_id: i64,
        preset: &str,
        now: Option<i64>,
    ) -> rusqlite::Result<Option<Clip>> {
        if keep_by_key(preset).is_none() {
            return self.get(clip_id);
        }
        let stamp = now.unwrap_or_else(now_secs);
        let until = keep_until_from(preset, stamp);
        self.conn.execute(
            "UPDATE clips SET keep_preset = ?, keep_until = ? WHERE id = ?",
            params![preset, until, clip_id],
        )?;
        self.get(clip_id)
    }

    pub fn touch(&self, clip_id: i64, now: Option<i64>) -> rusqlite::Result<()> {
        let stamp = now.unwrap_or_else(now_secs);
        self.conn.execute(
            "UPDATE clips SET last_used_at = ? WHERE id = ?",
            params![stamp, clip_id],
        )?;
        Ok(())
    }

    pub fn prune(&self, max_items: i64, now: Option<i64>) -> rusqlite::Result<Vec<PathBuf>> {
        let stamp = now.unwrap_or_else(now_secs);
        let mut removed = Vec::new();
        let expired: Vec<(i64, Option<String>)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, image_path FROM clips
                 WHERE keep_until IS NOT NULL AND keep_until <= ?",
            )?;
            let mapped = stmt.query_map([stamp], |r| Ok((r.get(0)?, r.get(1)?)))?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        for (id, image) in expired {
            if let Some(p) = image {
                removed.push(PathBuf::from(p));
            }
            self.conn.execute("DELETE FROM clips WHERE id = ?", [id])?;
        }
        let overflow: Vec<(i64, Option<String>)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, image_path FROM clips
                 ORDER BY
                    CASE WHEN keep_preset = 'forever' THEN 1 ELSE 0 END,
                    last_used_at ASC,
                    id ASC",
            )?;
            let mapped = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let extra = (overflow.len() as i64 - max_items).max(0) as usize;
        for (id, image) in overflow.into_iter().take(extra) {
            if let Some(p) = image {
                removed.push(PathBuf::from(p));
            }
            self.conn.execute("DELETE FROM clips WHERE id = ?", [id])?;
        }
        Ok(removed)
    }

    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM clips", [], |r| r.get(0))
    }

    pub fn seed(&self) -> rusqlite::Result<()> {
        let now = now_secs();
        for (i, &(text, keep)) in SEED_CLIPS.iter().enumerate() {
            self.add(
                "text",
                "text/plain",
                text.as_bytes(),
                Some(text),
                &make_preview(text, 280),
                None,
                keep,
                200,
                Some(now - i as i64),
            )?;
        }
        Ok(())
    }
}

fn row_clip(row: &rusqlite::Row) -> rusqlite::Result<Clip> {
    Ok(Clip {
        id: row.get("id")?,
        created_at: row.get("created_at")?,
        last_used_at: row.get("last_used_at")?,
        keep_preset: row.get("keep_preset")?,
        keep_until: row.get("keep_until")?,
        mime: row.get("mime")?,
        kind: row.get("kind")?,
        text: row.get("text")?,
        preview: row.get("preview")?,
        hash: row.get("hash")?,
        image_path: row.get("image_path")?,
        byte_size: row.get("byte_size")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("history.sqlite")).unwrap();
        (dir, store)
    }

    fn add(store: &Store, text: &str, keep: &str, now: i64) -> Clip {
        store
            .add(
                "text",
                "text/plain",
                text.as_bytes(),
                Some(text),
                text,
                None,
                keep,
                50,
                Some(now),
            )
            .unwrap()
            .unwrap()
    }

    #[test]
    fn dedup_moves_to_front() {
        let (_d, store) = tmp_store();
        let first = add(&store, "hello", "1d", 10);
        add(&store, "world", "1d", 20);
        let again = add(&store, "hello", "1d", 30);
        assert_eq!(first.id, again.id);
        let clips = store.list("", Some(31)).unwrap();
        let texts: Vec<_> = clips.iter().map(|c| c.text.clone().unwrap()).collect();
        assert_eq!(texts, ["hello", "world"]);
    }

    #[test]
    fn expired_clips_are_hidden_and_pruned() {
        let (_d, store) = tmp_store();
        add(&store, "short", "1h", 100);
        add(&store, "kept", "forever", 100);
        let visible = store.list("", Some(100 + 60 * 60 + 1)).unwrap();
        assert_eq!(
            visible
                .iter()
                .map(|c| c.text.clone().unwrap())
                .collect::<Vec<_>>(),
            ["kept"]
        );
        store.prune(50, Some(100 + 60 * 60 + 1)).unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn set_keep_forever() {
        let (_d, store) = tmp_store();
        let clip = add(&store, "pin me", "1h", 50);
        let updated = store
            .set_keep(clip.id, "forever", Some(80))
            .unwrap()
            .unwrap();
        assert_eq!(updated.keep_preset, "forever");
        assert!(updated.keep_until.is_none());
    }

    #[test]
    fn max_items_keeps_forever_last() {
        let (_d, store) = tmp_store();
        add(&store, "a", "1d", 1);
        add(&store, "b", "forever", 2);
        add(&store, "c", "1d", 3);
        store.prune(2, Some(4)).unwrap();
        let texts: std::collections::HashSet<_> = store
            .list("", Some(4))
            .unwrap()
            .into_iter()
            .filter_map(|c| c.text)
            .collect();
        assert!(texts.contains("b"));
        assert_eq!(texts.len(), 2);
    }

    #[test]
    fn search_filters_preview() {
        let (_d, store) = tmp_store();
        add(&store, "https://omarchy.org", "1d", 1);
        add(&store, "invoice-42", "1d", 2);
        let hits = store.list("omarchy", Some(3)).unwrap();
        assert_eq!(hits[0].text.as_deref(), Some("https://omarchy.org"));
    }

    #[test]
    fn search_does_not_match_kind() {
        let (_d, store) = tmp_store();
        add(&store, "one", "1d", 1);
        add(&store, "two", "1d", 2);
        let hits = store.list("t", Some(3)).unwrap();
        assert_eq!(
            hits.iter()
                .map(|c| c.text.clone().unwrap())
                .collect::<Vec<_>>(),
            ["two"]
        );
        assert!(store.list("text", Some(3)).unwrap().is_empty());
    }

    #[test]
    fn hash_stable() {
        assert_eq!(
            content_hash("text", "text/plain", b"abc"),
            content_hash("text", "text/plain", b"abc")
        );
        assert_ne!(
            content_hash("text", "text/plain", b"abc"),
            content_hash("text", "text/plain", b"abcd")
        );
    }

    #[test]
    fn keep_cycle() {
        assert_eq!(next_keep("1h").key, "1d");
        assert_eq!(next_keep("1d").key, "7d");
        assert_eq!(next_keep("7d").key, "forever");
        assert_eq!(next_keep("forever").key, "1h");
    }

    #[test]
    fn keep_until_forever_is_none() {
        assert!(keep_until_from("forever", 10).is_none());
        assert_eq!(keep_until_from("1h", 10), Some(10 + 3600));
    }

    #[test]
    fn now_is_sane() {
        let _ = SystemTime::now().duration_since(UNIX_EPOCH);
    }

    #[test]
    fn seed_inserts_text_samples() {
        let (_d, store) = tmp_store();
        assert_eq!(store.count().unwrap(), 0);
        store.seed().unwrap();
        let clips = store.list("", None).unwrap();
        assert_eq!(clips.len(), SEED_CLIPS.len());
        assert!(clips.iter().all(|c| c.kind == "text"));
        assert_eq!(clips[0].text.as_deref(), Some(SEED_CLIPS[0].0));
        assert!(
            clips
                .iter()
                .any(|c| c.text.as_deref() == Some("https://omarchy.org"))
        );
    }
}
