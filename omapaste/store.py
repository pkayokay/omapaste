from __future__ import annotations

import hashlib
import sqlite3
import time
from dataclasses import dataclass
from pathlib import Path

DEFAULT_KEEP = "1d"


@dataclass(frozen=True)
class KeepPreset:
    key: str
    label: str
    seconds: int | None


KEEP_PRESETS: tuple[KeepPreset, ...] = (
    KeepPreset("1h", "1 hour", 60 * 60),
    KeepPreset("1d", "1 day", 60 * 60 * 24),
    KeepPreset("7d", "7 days", 60 * 60 * 24 * 7),
    KeepPreset("forever", "Forever", None),
)

KEEP_BY_KEY = {p.key: p for p in KEEP_PRESETS}


def next_keep(current: str) -> KeepPreset:
    keys = [p.key for p in KEEP_PRESETS]
    try:
        index = keys.index(current)
    except ValueError:
        index = 1
    return KEEP_PRESETS[(index + 1) % len(KEEP_PRESETS)]


def keep_until_from(preset: str, now: int | None = None) -> int | None:
    spec = KEEP_BY_KEY.get(preset) or KEEP_BY_KEY[DEFAULT_KEEP]
    if spec.seconds is None:
        return None
    return (now if now is not None else int(time.time())) + spec.seconds


def content_hash(kind: str, mime: str, payload: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(kind.encode())
    digest.update(b"\0")
    digest.update(mime.encode())
    digest.update(b"\0")
    digest.update(payload)
    return digest.hexdigest()


def make_preview(text: str, limit: int = 280) -> str:
    collapsed = " ".join(text.split())
    if len(collapsed) <= limit:
        return collapsed
    return collapsed[: limit - 1] + "…"


@dataclass
class Clip:
    id: int
    created_at: int
    last_used_at: int
    keep_preset: str
    keep_until: int | None
    mime: str
    kind: str
    text: str | None
    preview: str
    hash: str
    image_path: str | None
    byte_size: int

    @property
    def keep_label(self) -> str:
        spec = KEEP_BY_KEY.get(self.keep_preset)
        return spec.label if spec else self.keep_preset

    @property
    def keep_short(self) -> str:
        if self.keep_preset == "forever":
            return "∞"
        return self.keep_preset


class Store:
    def __init__(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.path = path
        self.conn = sqlite3.connect(path)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA journal_mode=WAL")
        self.conn.execute("PRAGMA foreign_keys=ON")
        self._migrate()

    def close(self) -> None:
        self.conn.close()

    def _migrate(self) -> None:
        self.conn.executescript(
            """
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
            """
        )
        self.conn.commit()

    def add(
        self,
        *,
        kind: str,
        mime: str,
        payload: bytes,
        text: str | None,
        preview: str,
        image_path: str | None,
        keep_preset: str,
        max_items: int,
        now: int | None = None,
    ) -> Clip | None:
        if not payload:
            return None
        stamp = now if now is not None else int(time.time())
        digest = content_hash(kind, mime, payload)
        until = keep_until_from(keep_preset, stamp)
        existing = self.conn.execute(
            "SELECT id FROM clips WHERE hash = ?", (digest,)
        ).fetchone()
        if existing:
            self.conn.execute(
                "UPDATE clips SET last_used_at = ? WHERE id = ?",
                (stamp, existing["id"]),
            )
            self.conn.commit()
            return self.get(existing["id"])

        cursor = self.conn.execute(
            """
            INSERT INTO clips (
                created_at, last_used_at, keep_preset, keep_until,
                mime, kind, text, preview, hash, image_path, byte_size
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
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
                len(payload),
            ),
        )
        self.conn.commit()
        self.prune(max_items=max_items, now=stamp)
        return self.get(cursor.lastrowid)

    def get(self, clip_id: int) -> Clip | None:
        row = self.conn.execute(
            "SELECT * FROM clips WHERE id = ?", (clip_id,)
        ).fetchone()
        return self._clip(row) if row else None

    def list(self, query: str = "", now: int | None = None) -> list[Clip]:
        stamp = now if now is not None else int(time.time())
        rows = self.conn.execute(
            """
            SELECT * FROM clips
            WHERE (keep_until IS NULL OR keep_until > ?)
            ORDER BY last_used_at DESC, id DESC
            """,
            (stamp,),
        ).fetchall()
        clips = [self._clip(row) for row in rows]
        needle = query.strip().casefold()
        if not needle:
            return clips
        return [
            clip
            for clip in clips
            if needle in (clip.preview or "").casefold()
            or needle in (clip.text or "").casefold()
        ]

    def delete(self, clip_id: int) -> Path | None:
        row = self.conn.execute(
            "SELECT image_path FROM clips WHERE id = ?", (clip_id,)
        ).fetchone()
        self.conn.execute("DELETE FROM clips WHERE id = ?", (clip_id,))
        self.conn.commit()
        if row and row["image_path"]:
            return Path(row["image_path"])
        return None

    def set_keep(self, clip_id: int, preset: str, now: int | None = None) -> Clip | None:
        if preset not in KEEP_BY_KEY:
            return self.get(clip_id)
        stamp = now if now is not None else int(time.time())
        until = keep_until_from(preset, stamp)
        self.conn.execute(
            "UPDATE clips SET keep_preset = ?, keep_until = ? WHERE id = ?",
            (preset, until, clip_id),
        )
        self.conn.commit()
        return self.get(clip_id)

    def touch(self, clip_id: int, now: int | None = None) -> None:
        stamp = now if now is not None else int(time.time())
        self.conn.execute(
            "UPDATE clips SET last_used_at = ? WHERE id = ?", (stamp, clip_id)
        )
        self.conn.commit()

    def prune(self, max_items: int, now: int | None = None) -> list[Path]:
        stamp = now if now is not None else int(time.time())
        removed: list[Path] = []

        expired = self.conn.execute(
            """
            SELECT id, image_path FROM clips
            WHERE keep_until IS NOT NULL AND keep_until <= ?
            """,
            (stamp,),
        ).fetchall()
        for row in expired:
            if row["image_path"]:
                removed.append(Path(row["image_path"]))
            self.conn.execute("DELETE FROM clips WHERE id = ?", (row["id"],))

        overflow = self.conn.execute(
            """
            SELECT id, image_path FROM clips
            ORDER BY
                CASE WHEN keep_preset = 'forever' THEN 1 ELSE 0 END,
                last_used_at ASC,
                id ASC
            """
        ).fetchall()
        extra = max(0, len(overflow) - max_items)
        for row in overflow[:extra]:
            if row["image_path"]:
                removed.append(Path(row["image_path"]))
            self.conn.execute("DELETE FROM clips WHERE id = ?", (row["id"],))

        self.conn.commit()
        return removed

    def count(self) -> int:
        row = self.conn.execute("SELECT COUNT(*) AS n FROM clips").fetchone()
        return int(row["n"]) if row else 0

    @staticmethod
    def _clip(row: sqlite3.Row) -> Clip:
        return Clip(
            id=row["id"],
            created_at=row["created_at"],
            last_used_at=row["last_used_at"],
            keep_preset=row["keep_preset"],
            keep_until=row["keep_until"],
            mime=row["mime"],
            kind=row["kind"],
            text=row["text"],
            preview=row["preview"],
            hash=row["hash"],
            image_path=row["image_path"],
            byte_size=row["byte_size"],
        )
