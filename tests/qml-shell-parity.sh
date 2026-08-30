#!/usr/bin/env bash
# Shell-level parity checks for capture.sh / paste.sh.
# Run from repo root: bash tests/qml-shell-parity.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

export XDG_STATE_HOME="$TMP/state"
mkdir -p "$XDG_STATE_HOME"
STATE="$XDG_STATE_HOME/omapaste"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); echo "ok: $*"; }
bad() { FAIL=$((FAIL + 1)); echo "FAIL: $*" >&2; }

# --- text capture ---
line=$(printf 'hello-parity' | "$ROOT/capture.sh" text)
type=$(jq -r '.type' <<<"$line")
text=$(jq -r '.text' <<<"$line")
hash=$(jq -r '.hash' <<<"$line")
[[ "$type" == "text" && "$text" == "hello-parity" && -n "$hash" ]] && ok "capture text json" || bad "capture text json ($line)"

# --- ignore-hash skips re-capture ---
printf '%s' "$hash" >"$STATE/qml-ignore-hash"
out=$(printf 'hello-parity' | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "ignore-hash skips text" || bad "ignore-hash still emitted: $out"
[[ ! -f "$STATE/qml-ignore-hash" ]] && ok "ignore-hash consumed" || bad "ignore-hash file left behind"

# --- empty / whitespace skipped ---
out=$(printf '   ' | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "blank text skipped" || bad "blank text emitted"

# --- image capture (valid PNG from repo samples) ---
png="$ROOT/share/sample-images/sample-red.png"
[[ -f "$png" ]] || { bad "missing sample-red.png"; exit 1; }
line=$(cat "$png" | "$ROOT/capture.sh" "image/png")
itype=$(jq -r '.type' <<<"$line")
ipath=$(jq -r '.path' <<<"$line")
ihash=$(jq -r '.hash' <<<"$line")
[[ "$itype" == "image" && -f "$ipath" && -n "$ihash" ]] && ok "capture image json+file" || bad "capture image ($line)"

# corrupt PNG rejected
badpng="$TMP/corrupt.png"
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82' >"$badpng"
out=$(cat "$badpng" | "$ROOT/capture.sh" "image/png" || true)
[[ -z "$out" ]] && ok "corrupt PNG rejected" || bad "corrupt PNG accepted: $out"

# --- paste.sh copy-text writes ignore + clipboard ---
if command -v wl-copy >/dev/null && command -v wl-paste >/dev/null; then
  "$ROOT/paste.sh" copy-text "paste-check-body" "pastehash123"
  [[ -f "$STATE/qml-ignore-hash" ]] || bad "paste copy-text missing ignore-hash"
  got=$(cat "$STATE/qml-ignore-hash")
  [[ "$got" == "pastehash123" ]] && ok "paste copy-text ignore-hash" || bad "paste ignore-hash content=$got"
  clip=$(wl-paste --type text --no-newline 2>/dev/null || true)
  [[ "$clip" == "paste-check-body" ]] && ok "paste copy-text wl-copy" || bad "clipboard=$clip"
  [[ -f "$STATE/qml-ignore-until" ]] && ok "paste copy-text ignore-until" || bad "paste missing ignore-until"
else
  bad "wl-copy/wl-paste missing"
fi

out=$(printf 'during-ignore-window' | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "ignore-until skips capture" || bad "ignore-until still emitted: $out"

"$ROOT/paste.sh" arm-ignore "" 3
out=$(printf 'still-ignored' | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "arm-ignore extends window" || bad "arm-ignore still emitted: $out"

omapaste_uri="file://$STATE/qml-images/sample-ref.png"
out=$(printf '%s' "$omapaste_uri" | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "omapaste file uri skipped" || bad "omapaste uri captured: $out"
omapaste_path="$STATE/qml-images/sample-ref.png"
out=$(printf '%s' "$omapaste_path" | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "omapaste image path skipped" || bad "omapaste path captured: $out"
gnome_clip=$'copy\nfile://'"$omapaste_path"
out=$(printf '%s' "$gnome_clip" | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "omapaste gnome copy block skipped" || bad "gnome block captured: $out"
out=$(printf '%s' "$(basename "$omapaste_path")" | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "omapaste hash basename skipped" || bad "hash basename captured: $out"
enc_uri=$(python3 -c "import urllib.parse; print('file://' + urllib.parse.quote('$omapaste_path', safe=''))")
out=$(printf '%s' "$enc_uri" | "$ROOT/capture.sh" text || true)
[[ -z "$out" ]] && ok "encoded file uri skipped" || bad "encoded uri captured: $out"
rm -f "$STATE/qml-ignore-until" "$STATE/qml-ignore-hash"
out=$(printf 'file:///tmp/not-omapaste.png' | "$ROOT/capture.sh" text || true)
[[ -n "$out" ]] && ok "other file uri still captured" || bad "external file uri skipped"

# --- paste.sh copy-image ---
if [[ -f "$ipath" ]]; then
  "$ROOT/paste.sh" copy-image "$ipath" "image/png" "$ihash"
  [[ "$(cat "$STATE/qml-ignore-hash")" == "$ihash" ]] && ok "paste copy-image ignore-hash" || bad "copy-image ignore"
fi

# --- secret MIME: simulate list-types containing password hint ---
# capture.sh calls wl-paste --list-types; stub PATH
STUB="$TMP/bin"
mkdir -p "$STUB"
cat >"$STUB/wl-paste" <<'EOF'
#!/bin/bash
if [[ "${1:-}" == "--list-types" ]]; then
  printf '%s\n' 'text/plain' 'x-kde-passwordManagerHint'
  exit 0
fi
exit 1
EOF
chmod +x "$STUB/wl-paste"
out=$(PATH="$STUB:$PATH" "$ROOT/capture.sh" 2>/dev/null || true)
[[ -z "$out" ]] && ok "secret MIME skipped" || bad "secret MIME emitted: $out"

# --- CLIPBOARD_STATE=sensitive ---
out=$(CLIPBOARD_STATE=sensitive printf 'secret' | PATH="$STUB:$PATH" "$ROOT/capture.sh" text 2>/dev/null || true)
# text mode still reads stdin and doesn't re-check list-types the same way —
# sensitive is checked at top via list-types OR CLIPBOARD_STATE before case.
# For `text` subcommand, list-types still runs at top — stub has password hint so exit 0.
out=$(CLIPBOARD_STATE=sensitive "$ROOT/capture.sh" 2>/dev/null || true)
[[ -z "$out" ]] && ok "CLIPBOARD_STATE=sensitive skipped" || bad "sensitive emitted"

echo
echo "shell parity: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
