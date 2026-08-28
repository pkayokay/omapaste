use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

pub const DEFAULT_KEEP: &str = "1d";

/// Sample clips for a brand-new history database.
pub const SEED_CLIPS: &[(&str, &str)] = &[
    (
        "fn greet(name: &str)\n  -> String {\n  format!(\"hi {name}\")\n}",
        "7d",
    ),
    (
        "← → select a clip.\nEnter pastes it.\nEsc closes the bar.",
        "forever",
    ),
    ("https://omarchy.org", "7d"),
    ("Type to search.\nCtrl+K cycles keep\ntime.", "forever"),
    (crate::paths::ISSUES_URL, "forever"),
];

/// PNG payloads shipped in `share/sample-images/` for first-run seeding.
pub const SEED_IMAGES: &[(&str, &[u8])] = &[
    (
        "7d",
        include_bytes!("../share/sample-images/sample-red.png"),
    ),
    (
        "forever",
        include_bytes!("../share/sample-images/sample-blue.png"),
    ),
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
    pub custom_label: Option<String>,
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

    pub fn display_label(&self) -> String {
        self.custom_label
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.kind_label())
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
    _db_file: File,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let (conn, db_file) = crate::secure_fs::open_sqlite_connection(path)?;
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
        crate::secure_fs::harden_sqlite_files(path)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        migrate(&conn)?;
        Ok(Self {
            conn,
            _db_file: db_file,
        })
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
            .filter(|c| clip_matches_query(c, &needle))
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

    pub fn set_custom_label(
        &self,
        clip_id: i64,
        label: Option<&str>,
    ) -> rusqlite::Result<Option<Clip>> {
        let value = label.map(str::trim).filter(|s| !s.is_empty());
        self.conn.execute(
            "UPDATE clips SET custom_label = ? WHERE id = ?",
            params![value, clip_id],
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

    pub fn seed(&self, images_dir: &Path) -> rusqlite::Result<()> {
        crate::secure_fs::ensure_private_dir(images_dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = now_secs();
        let mut order = 0i64;
        for &(text, keep) in SEED_CLIPS {
            self.add(
                "text",
                "text/plain",
                text.as_bytes(),
                Some(text),
                &make_preview(text, 280),
                None,
                keep,
                200,
                Some(now - order),
            )?;
            order += 1;
        }
        for &(keep, payload) in SEED_IMAGES {
            let mime = "image/png";
            let digest = content_hash("image", mime, payload);
            let image_path = images_dir.join(format!("{digest}.bin"));
            if !image_path.exists() {
                crate::secure_fs::write_private_file_if_missing(&image_path, payload)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }
            self.add(
                "image",
                mime,
                payload,
                None,
                "Image",
                Some(image_path.to_str().unwrap_or_default()),
                keep,
                200,
                Some(now - order),
            )?;
            order += 1;
        }
        Ok(())
    }
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let has_col = conn
        .prepare("PRAGMA table_info(clips)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "custom_label");
    if !has_col {
        conn.execute("ALTER TABLE clips ADD COLUMN custom_label TEXT", [])?;
    }
    Ok(())
}

fn clip_matches_query(clip: &Clip, needle: &str) -> bool {
    clip.preview.to_lowercase().contains(needle)
        || clip
            .text
            .as_deref()
            .map(|t| t.to_lowercase().contains(needle))
            .unwrap_or(false)
        || clip
            .custom_label
            .as_deref()
            .map(|l| l.to_lowercase().contains(needle))
            .unwrap_or(false)
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
        custom_label: row.get("custom_label")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn get_missing_and_empty_list() {
        let (_d, store) = tmp_store();
        assert!(store.get(99).unwrap().is_none());
        assert!(store.list("", Some(1)).unwrap().is_empty());
        assert_eq!(store.count().unwrap(), 0);
        assert!(store.delete(99).unwrap().is_none());
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("history.sqlite");
        {
            let store = Store::open(&path).unwrap();
            add(&store, "kept", "1d", 1);
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(
            store.list("", Some(2)).unwrap()[0].text.as_deref(),
            Some("kept")
        );
    }

    #[test]
    fn open_hardens_sqlite_wal_and_shm() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("history.sqlite");
        let store = Store::open(&path).unwrap();
        add(&store, "touch", "1d", 1);
        let base = path.to_string_lossy();
        for sidecar in [format!("{base}-wal"), format!("{base}-shm")] {
            let sidecar_path = std::path::Path::new(&sidecar);
            if sidecar_path.exists() {
                #[cfg(unix)]
                assert_eq!(
                    sidecar_path.metadata().unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn expiry_boundary_is_exclusive_in_list() {
        let (_d, store) = tmp_store();
        add(&store, "short", "1h", 100);
        assert_eq!(store.list("", Some(100 + 3600)).unwrap().len(), 0);
        assert_eq!(store.list("", Some(100 + 3599)).unwrap().len(), 1);
        assert_eq!(store.count().unwrap(), 1);
        store.prune(50, Some(100 + 3600)).unwrap();
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn dedup_does_not_change_keep() {
        let (_d, store) = tmp_store();
        let first = add(&store, "hello", "1h", 10);
        let again = add(&store, "hello", "forever", 40);
        assert_eq!(first.id, again.id);
        assert_eq!(again.keep_preset, "1h");
        assert_eq!(again.keep_until, Some(10 + 3600));
        assert_eq!(again.last_used_at, 40);
    }

    #[test]
    fn prune_returns_expired_image_paths() {
        let (_d, store) = tmp_store();
        store
            .add(
                "image",
                "image/png",
                b"png",
                None,
                "Image",
                Some("/tmp/expired.png"),
                "1h",
                50,
                Some(10),
            )
            .unwrap();
        let removed = store.prune(50, Some(10 + 3600)).unwrap();
        assert_eq!(removed, [std::path::PathBuf::from("/tmp/expired.png")]);
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn same_timestamp_orders_by_newer_id() {
        let (_d, store) = tmp_store();
        add(&store, "first", "1d", 5);
        add(&store, "second", "1d", 5);
        let texts: Vec<_> = store
            .list("", Some(6))
            .unwrap()
            .into_iter()
            .filter_map(|c| c.text)
            .collect();
        assert_eq!(texts, ["second", "first"]);
    }

    #[test]
    fn search_unicode_and_set_keep_recomputes_until() {
        let (_d, store) = tmp_store();
        add(&store, "こんにちは", "forever", 1);
        assert_eq!(store.list("こん", Some(2)).unwrap().len(), 1);
        let clip = add(&store, "pin", "forever", 1);
        let updated = store.set_keep(clip.id, "1h", Some(50)).unwrap().unwrap();
        assert_eq!(updated.keep_preset, "1h");
        assert_eq!(updated.keep_until, Some(50 + 3600));
    }

    #[test]
    fn seed_inserts_text_and_image_samples() {
        let (dir, store) = tmp_store();
        let images = dir.path().join("images");
        assert_eq!(store.count().unwrap(), 0);
        store.seed(&images).unwrap();
        let clips = store.list("", None).unwrap();
        assert_eq!(clips.len(), SEED_CLIPS.len() + SEED_IMAGES.len());
        assert!(clips.iter().any(|c| c.kind == "text"));
        assert_eq!(
            clips.iter().filter(|c| c.kind == "text").count(),
            SEED_CLIPS.len()
        );
        assert!(clips.iter().any(|c| c.kind == "image"));
        assert_eq!(
            clips.iter().filter(|c| c.kind == "image").count(),
            SEED_IMAGES.len()
        );
        assert_eq!(clips[0].text.as_deref(), Some(SEED_CLIPS[0].0));
        assert!(clips
            .iter()
            .any(|c| c.text.as_deref() == Some("https://omarchy.org")));
        assert!(clips
            .iter()
            .any(|c| c.text.as_deref() == Some(crate::paths::ISSUES_URL)));
        let image = clips.iter().find(|c| c.kind == "image").unwrap();
        assert_eq!(image.mime, "image/png");
        assert!(image
            .image_path
            .as_ref()
            .is_some_and(|p| Path::new(p).exists()));
        store.seed(&images).unwrap();
        assert_eq!(
            store.count().unwrap(),
            (SEED_CLIPS.len() + SEED_IMAGES.len()) as i64
        );
    }

    fn sample_clip(kind: &str, text: Option<&str>, bytes: i64, keep: &str) -> Clip {
        Clip {
            id: 1,
            created_at: 0,
            last_used_at: 0,
            keep_preset: keep.into(),
            keep_until: None,
            mime: "text/plain".into(),
            kind: kind.into(),
            text: text.map(str::to_string),
            preview: text.unwrap_or("").into(),
            hash: "x".into(),
            image_path: None,
            byte_size: bytes,
            custom_label: None,
        }
    }

    #[test]
    fn empty_payload_is_ignored() {
        let (_d, store) = tmp_store();
        let none = store
            .add(
                "text",
                "text/plain",
                b"",
                Some(""),
                "",
                None,
                "1d",
                50,
                Some(1),
            )
            .unwrap();
        assert!(none.is_none());
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn delete_removes_clip_and_returns_image_path() {
        let (_d, store) = tmp_store();
        let clip = add(&store, "bye", "1d", 1);
        assert!(store.delete(clip.id).unwrap().is_none());
        assert!(store.get(clip.id).unwrap().is_none());
        assert_eq!(store.count().unwrap(), 0);

        let image = store
            .add(
                "image",
                "image/png",
                b"png-bytes",
                None,
                "Image",
                Some("/tmp/clip.png"),
                "1d",
                50,
                Some(2),
            )
            .unwrap()
            .unwrap();
        assert_eq!(image.kind, "image");
        let path = store.delete(image.id).unwrap();
        assert_eq!(path.as_deref(), Some(std::path::Path::new("/tmp/clip.png")));
    }

    #[test]
    fn touch_reorders_by_last_used() {
        let (_d, store) = tmp_store();
        let a = add(&store, "a", "1d", 1);
        add(&store, "b", "1d", 2);
        store.touch(a.id, Some(9)).unwrap();
        let texts: Vec<_> = store
            .list("", Some(10))
            .unwrap()
            .into_iter()
            .filter_map(|c| c.text)
            .collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[test]
    fn invalid_keep_preset_is_ignored() {
        let (_d, store) = tmp_store();
        let clip = add(&store, "stay", "1h", 10);
        let same = store.set_keep(clip.id, "nope", Some(11)).unwrap().unwrap();
        assert_eq!(same.keep_preset, "1h");
        assert_eq!(same.keep_until, Some(10 + 3600));
    }

    #[test]
    fn add_prunes_to_max_items() {
        let (_d, store) = tmp_store();
        add(&store, "old", "1d", 1);
        add(&store, "mid", "1d", 2);
        store
            .add(
                "text",
                "text/plain",
                b"new",
                Some("new"),
                "new",
                None,
                "1d",
                2,
                Some(3),
            )
            .unwrap();
        let texts: Vec<_> = store
            .list("", Some(4))
            .unwrap()
            .into_iter()
            .filter_map(|c| c.text)
            .collect();
        assert_eq!(texts, ["new", "mid"]);
    }

    #[test]
    fn search_is_case_insensitive_and_uses_full_text() {
        let (_d, store) = tmp_store();
        store
            .add(
                "text",
                "text/plain",
                b"Hello Tokyo Night",
                Some("Hello Tokyo Night"),
                "Hello…",
                None,
                "1d",
                50,
                Some(1),
            )
            .unwrap();
        assert_eq!(store.list("tokyo", Some(2)).unwrap().len(), 1);
        assert_eq!(store.list("HELLO", Some(2)).unwrap().len(), 1);
        assert_eq!(store.list("night", Some(2)).unwrap().len(), 1);
        assert_eq!(store.list("   ", Some(2)).unwrap().len(), 1);
        assert!(store.list("missing", Some(2)).unwrap().is_empty());
    }

    #[test]
    fn preview_collapses_whitespace_and_ellipsizes() {
        assert_eq!(make_preview("  foo   bar  ", 280), "foo bar");
        assert_eq!(make_preview("abcd", 4), "abcd");
        assert_eq!(make_preview("abcde", 4), "abc…");
        let preview = make_preview("こんにちは世界", 4);
        assert!(preview.ends_with('…'));
        assert_eq!(preview.chars().count(), 4);
    }

    #[test]
    fn hash_includes_kind_and_mime() {
        assert_ne!(
            content_hash("text", "text/plain", b"abc"),
            content_hash("image", "text/plain", b"abc")
        );
        assert_ne!(
            content_hash("text", "text/plain", b"abc"),
            content_hash("text", "text/html", b"abc")
        );
    }

    #[test]
    fn keep_helpers() {
        assert!(keep_by_key("nope").is_none());
        assert_eq!(next_keep("nope").key, "7d");
        assert_eq!(keep_until_from("nope", 0), Some(86_400));
        assert_eq!(keep_until_from("7d", 0), Some(86_400 * 7));
    }

    #[test]
    fn clip_labels_and_sizes() {
        let one = sample_clip("text", Some("é"), 0, "1d");
        assert_eq!(one.kind_label(), "Text");
        assert_eq!(one.format_chars(), "1 char");
        assert_eq!(one.keep_short(), "1d");
        assert_eq!(one.keep_label(), "1 day");

        let many = sample_clip("text", Some("ab"), 0, "forever");
        assert_eq!(many.format_chars(), "2 chars");
        assert_eq!(many.keep_short(), "∞");
        assert_eq!(many.keep_label(), "Forever");

        let empty = sample_clip("text", None, 0, "7d");
        assert_eq!(empty.format_chars(), "0 chars");
        assert_eq!(empty.kind_label(), "Text");

        let image = sample_clip("image", None, 500, "1h");
        assert_eq!(image.kind_label(), "Image");
        assert_eq!(image.format_chars(), "500 B");
        assert_eq!(
            sample_clip("image", None, 1536, "1h").format_chars(),
            "1.5 KB"
        );
        assert_eq!(
            sample_clip("image", None, 20_480, "1h").format_chars(),
            "20 KB"
        );
        assert_eq!(
            sample_clip("image", None, 2 * 1024 * 1024, "1h").format_chars(),
            "2.0 MB"
        );
        assert_eq!(
            sample_clip("foo_bar", None, 0, "1d").kind_label(),
            "foo bar"
        );
        assert_eq!(
            sample_clip("image", None, 1023, "1h").format_chars(),
            "1023 B"
        );
        assert_eq!(
            sample_clip("image", None, 1024, "1h").format_chars(),
            "1.0 KB"
        );
        let unknown = sample_clip("text", Some("x"), 0, "custom");
        assert_eq!(unknown.keep_label(), "custom");
        assert_eq!(unknown.keep_short(), "custom");
    }

    #[test]
    fn custom_label_round_trip_and_search() {
        let (_d, store) = tmp_store();
        let clip = add(&store, "hello", "1d", 10);
        assert_eq!(clip.display_label(), "Text");

        let renamed = store
            .set_custom_label(clip.id, Some("  Notes  "))
            .unwrap()
            .unwrap();
        assert_eq!(renamed.custom_label.as_deref(), Some("Notes"));
        assert_eq!(renamed.display_label(), "Notes");

        let cleared = store
            .set_custom_label(clip.id, Some("  "))
            .unwrap()
            .unwrap();
        assert!(cleared.custom_label.is_none());
        assert_eq!(cleared.display_label(), "Text");

        store.set_custom_label(clip.id, Some("todo")).unwrap();
        assert_eq!(store.list("todo", Some(11)).unwrap().len(), 1);
        assert!(store.list("text", Some(11)).unwrap().is_empty());
    }

    #[test]
    fn preview_tiny_limit_is_ellipsis() {
        assert_eq!(make_preview("abc", 1), "…");
        assert_eq!(make_preview("abc", 0), "…");
    }

    #[test]
    fn seed_keep_mix() {
        let (dir, store) = tmp_store();
        store.seed(&dir.path().join("images")).unwrap();
        let clips = store.list("", None).unwrap();
        assert!(clips.iter().any(|c| c.keep_preset == "forever"));
        assert!(clips.iter().any(|c| c.keep_preset == "7d"));
        assert!(clips.iter().all(|c| c.keep_preset != "1h"));
    }
}
