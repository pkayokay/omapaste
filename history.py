#!/usr/bin/env python3
# SQLite history for the Quattro omapaste plugin (list / save / migrate).
# One-command install safe: stdlib only (no pip).

from __future__ import annotations

import json
import os
import sqlite3
import sys
import time

SCHEMA = """
CREATE TABLE IF NOT EXISTS clips (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL,
  hash TEXT NOT NULL,
  text TEXT,
  path TEXT,
  mime TEXT,
  kind TEXT,
  keep TEXT,
  keep_until REAL,
  ts REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clips_ts ON clips(ts DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_clips_hash ON clips(hash) WHERE hash != '';
"""


def default_db_path() -> str:
  state = os.environ.get("XDG_STATE_HOME") or os.path.join(
    os.path.expanduser("~"), ".local", "state"
  )
  return os.path.join(state, "omapaste", "history.sqlite")


def stamp_path(db: str) -> str:
  return db + ".stamp"


def json_legacy_path(db: str) -> str:
  return os.path.join(os.path.dirname(db), "qml-history.json")


def connect(db: str) -> sqlite3.Connection:
  parent = os.path.dirname(db)
  if parent:
    os.makedirs(parent, mode=0o700, exist_ok=True)
  conn = sqlite3.connect(db)
  conn.row_factory = sqlite3.Row
  conn.execute("PRAGMA journal_mode=WAL")
  conn.execute("PRAGMA synchronous=NORMAL")
  conn.executescript(SCHEMA)
  try:
    os.chmod(db, 0o600)
  except OSError:
    pass
  return conn


def bump_stamp(db: str) -> None:
  path = stamp_path(db)
  with open(path, "w", encoding="utf-8") as f:
    f.write(f"{time.time()}\n")
  try:
    os.chmod(path, 0o600)
  except OSError:
    pass


def row_to_entry(row: sqlite3.Row) -> dict:
  entry: dict = {
    "type": row["type"],
    "hash": row["hash"] or "",
    "ts": float(row["ts"] or 0),
    "kind": row["kind"]
    or ("Image" if row["type"] == "image" else "Text"),
    "keep": row["keep"] or "1d",
  }
  if row["keep_until"] is None:
    entry["keep_until"] = None
  else:
    entry["keep_until"] = float(row["keep_until"])
  if row["type"] == "image":
    entry["path"] = row["path"] or ""
    entry["mime"] = row["mime"] or "image/png"
  else:
    entry["text"] = row["text"] or ""
  return entry


def insert_entry(conn: sqlite3.Connection, item: dict) -> None:
  typ = str(item.get("type") or "")
  if typ not in ("text", "image"):
    return
  keep_until = item.get("keep_until", None)
  if keep_until is not None:
    try:
      keep_until = float(keep_until)
    except (TypeError, ValueError):
      keep_until = None
  conn.execute(
    """
    INSERT INTO clips
      (type, hash, text, path, mime, kind, keep, keep_until, ts)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """,
    (
      typ,
      str(item.get("hash") or ""),
      None if typ == "image" else str(item.get("text") or ""),
      None if typ != "image" else str(item.get("path") or ""),
      None if typ != "image" else str(item.get("mime") or "image/png"),
      str(
        item.get("kind")
        or ("Image" if typ == "image" else "Text")
      ),
      str(item.get("keep") or "1d"),
      keep_until,
      float(item.get("ts") or time.time()),
    ),
  )


def replace_all(conn: sqlite3.Connection, items: list) -> None:
  conn.execute("DELETE FROM clips")
  for item in items:
    if isinstance(item, dict):
      insert_entry(conn, item)
  conn.commit()


def migrate_json_if_needed(conn: sqlite3.Connection, db: str) -> bool:
  count = conn.execute("SELECT COUNT(*) AS n FROM clips").fetchone()["n"]
  if count:
    return False
  legacy = json_legacy_path(db)
  if not os.path.isfile(legacy):
    return False
  try:
    with open(legacy, encoding="utf-8") as f:
      data = json.load(f)
  except (OSError, json.JSONDecodeError):
    return False
  if not isinstance(data, list) or not data:
    return False
  replace_all(conn, data)
  migrated = legacy + ".migrated"
  try:
    os.replace(legacy, migrated)
  except OSError:
    pass
  bump_stamp(db)
  return True


def cmd_list(db: str) -> int:
  conn = connect(db)
  migrate_json_if_needed(conn, db)
  rows = conn.execute(
    "SELECT * FROM clips ORDER BY ts DESC, id DESC"
  ).fetchall()
  out = [row_to_entry(r) for r in rows]
  json.dump(out, sys.stdout, ensure_ascii=False)
  sys.stdout.write("\n")
  conn.close()
  return 0


def cmd_save(db: str, source: str) -> int:
  if source == "-":
    raw = sys.stdin.read()
  else:
    with open(source, encoding="utf-8") as f:
      raw = f.read()
  try:
    data = json.loads(raw or "[]")
  except json.JSONDecodeError:
    data = []
  if not isinstance(data, list):
    data = []
  conn = connect(db)
  replace_all(conn, data)
  conn.close()
  bump_stamp(db)
  return 0


def usage() -> None:
  print(
    "Usage: history.py list [db]\n"
    "       history.py save [db] [json-file|-]",
    file=sys.stderr,
  )


def main(argv: list[str]) -> int:
  if len(argv) < 2:
    usage()
    return 2
  cmd = argv[1]
  db = argv[2] if len(argv) > 2 else default_db_path()
  if cmd == "list":
    return cmd_list(db)
  if cmd == "save":
    source = argv[3] if len(argv) > 3 else "-"
    return cmd_save(db, source)
  usage()
  return 2


if __name__ == "__main__":
  sys.exit(main(sys.argv))
