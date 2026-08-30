#!/bin/bash
# Capture clipboard as one JSON line for the QML omapaste experiment.
# Mirrors Omarchy's watcher shape, but writes under omapaste state paths.

set -o pipefail

STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/omapaste"
IMAGE_DIR="$STATE_DIR/qml-images"
IGNORE_FILE="$STATE_DIR/qml-ignore-hash"
IGNORE_UNTIL_FILE="$STATE_DIR/qml-ignore-until"
CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/omapaste/qml-config.json"
MAX_BYTES=8000000
if [[ -f "$CONFIG_FILE" ]]; then
  MAX_BYTES=$(jq -r '.max_bytes // 8000000' "$CONFIG_FILE" 2>/dev/null || echo 8000000)
fi
export OMAPASTE_MAX_BYTES="$MAX_BYTES"
export OMAPASTE_IMAGE_DIR="$IMAGE_DIR"
mkdir -p "$IMAGE_DIR"
chmod 700 "$STATE_DIR" "$IMAGE_DIR" 2>/dev/null || true

types=$(wl-paste --list-types 2>/dev/null || true)

if [[ ${CLIPBOARD_STATE:-} == "sensitive" ]] || grep -qx 'x-kde-passwordManagerHint' <<<"$types"; then
  exit 0
fi

ignore_all_active() {
  [[ -f "$IGNORE_UNTIL_FILE" ]] || return 1
  python3 -c '
import sys, time
path = sys.argv[1]
try:
    until = float(open(path).read().strip())
except (OSError, ValueError):
    sys.exit(1)
if time.time() < until:
    sys.exit(0)
sys.exit(1)
' "$IGNORE_UNTIL_FILE" && return 0
  rm -f "$IGNORE_UNTIL_FILE"
  return 1
}

if ignore_all_active; then
  exit 0
fi

maybe_ignore() {
  local hash="$1"
  if [[ -f "$IGNORE_FILE" ]]; then
    local ignored
    ignored=$(cat "$IGNORE_FILE" 2>/dev/null || true)
    rm -f "$IGNORE_FILE"
    if [[ -n "$ignored" && "$ignored" == "$hash" ]]; then
      exit 0
    fi
  fi
}

emit_image() {
  local mime="$1"
  local ext tmp hash file

  ext=${mime#image/}
  [[ $ext == jpeg ]] && ext=jpg

  tmp=$(mktemp --tmpdir="$IMAGE_DIR" clipboard.XXXXXX) || return 0
  # Cap image capture so a hostile source cannot fill disk in one paste.
  head -c 20971520 >"$tmp" || true
  if [[ ! -s $tmp ]]; then
    rm -f "$tmp"
    return 0
  fi
  # Reject truncated/corrupt images (Qt warns loudly on decode failure).
  if ! python3 -c '
import struct, sys, zlib
path, mime = sys.argv[1], sys.argv[2]
data = open(path, "rb").read()
if mime == "image/png":
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise SystemExit(1)
    i = 8
    saw_iend = False
    while i + 8 <= len(data):
        length = struct.unpack(">I", data[i:i+4])[0]
        ctype = data[i+4:i+8]
        end = i + 8 + length + 4
        if end > len(data):
            raise SystemExit(1)
        chunk = data[i+8:i+8+length]
        crc_got = struct.unpack(">I", data[i+8+length:end])[0]
        if (zlib.crc32(ctype + chunk) & 0xffffffff) != crc_got:
            raise SystemExit(1)
        i = end
        if ctype == b"IEND":
            saw_iend = True
            break
    raise SystemExit(0 if saw_iend else 1)
if mime == "image/jpeg":
    raise SystemExit(0 if data.startswith(b"\xff\xd8\xff") and data.endswith(b"\xff\xd9") else 1)
raise SystemExit(0)
' "$tmp" "$mime"; then
    rm -f "$tmp"
    return 0
  fi

  hash=$(sha256sum "$tmp" | awk '{print $1}')
  maybe_ignore "$hash"

  file="$IMAGE_DIR/$hash.$ext"
  if [[ -e $file ]]; then
    rm -f "$tmp"
  else
    mv "$tmp" "$file"
    chmod 600 "$file" 2>/dev/null || true
  fi

  jq -cn --arg mime "$mime" --arg path "$file" --arg hash "$hash" \
    --argjson ts "$(date +%s)" \
    '{type:"image", mime:$mime, path:$path, hash:$hash, ts:$ts}'
}

emit_text() {
  # Use -c so clipboard bytes on stdin are not eaten by a heredoc.
  python3 -c '
import hashlib, json, os, re, sys, time, urllib.parse
max_bytes = int(os.environ.get("OMAPASTE_MAX_BYTES", "8000000"))
raw = sys.stdin.buffer.read(max_bytes + 1)
if not raw:
    raise SystemExit(0)
if len(raw) > max_bytes:
    raise SystemExit(0)
try:
    text = raw.decode("utf-8")
except UnicodeDecodeError:
    text = raw.decode("utf-8", errors="replace")
if not text.strip():
    raise SystemExit(0)

def decoded_path(line):
    s = line.strip()
    if s.startswith("file://"):
        return urllib.parse.unquote(s[7:], errors="strict")
    return s

def line_is_omapaste_image_ref(s):
    s = s.strip()
    if not s:
        return False
    decoded = decoded_path(s)
    if "omapaste/qml-images/" in decoded or "/omapaste/qml-images/" in decoded:
        return True
    if "omapaste%2Fqml-images" in s or "omapaste%2fqml-images" in s:
        return True
    if re.fullmatch(r"[a-f0-9]{64}\.(png|jpe?g|webp|gif|bmp|tiff)", s, re.I):
        return True
    if re.fullmatch(r"[a-f0-9]{64}\.(png|jpe?g|webp|gif|bmp|tiff)", decoded, re.I):
        return True
    if s.startswith("file://"):
        return False
    if "://" in s:
        return False
    return "/omapaste/qml-images/" in s

def is_omapaste_image_ref(text):
    raw = str(text or "")
    if not raw.strip():
        return False
    refs = []
    for line in raw.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.lower() == "copy":
            continue
        refs.append(line)
    if not refs:
        return False
    return all(line_is_omapaste_image_ref(line) for line in refs)

if is_omapaste_image_ref(text):
    raise SystemExit(0)

digest = hashlib.sha256(raw).hexdigest()
print(json.dumps({"type": "text", "text": text, "hash": digest, "ts": time.time()}, separators=(",", ":")))
'
}

case "${1:-}" in
text)
  emit_text | {
    read -r line || exit 0
    hash=$(jq -r '.hash // empty' <<<"$line")
    maybe_ignore "$hash"
    printf '%s\n' "$line"
  }
  exit 0
  ;;
image/*)
  emit_image "$1"
  exit 0
  ;;
esac

for mime in image/png image/jpeg image/webp image/gif image/bmp image/tiff; do
  if grep -qx "$mime" <<<"$types"; then
    timeout 2s wl-paste --type "$mime" 2>/dev/null | emit_image "$mime"
    exit 0
  fi
done

if grep -q '^text/' <<<"$types" || grep -qx 'UTF8_STRING' <<<"$types" || grep -qx 'STRING' <<<"$types"; then
  wl-paste --type text --no-newline 2>/dev/null | emit_text | {
    read -r line || exit 0
    hash=$(jq -r '.hash // empty' <<<"$line")
    maybe_ignore "$hash"
    printf '%s\n' "$line"
  }
fi
