#!/usr/bin/env bash
# Register Omapaste in the Omarchy app launcher (desktop entry + menu route).
set -euo pipefail

SRC="$(cd "$(dirname "$0")" && pwd)"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}"
APP="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
MENU_EXT="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/extensions/omarchy-menu.jsonc"
STAMP="$STATE/omapaste/launcher-installed"
QUIET="${QUIET:-0}"

PLUGIN_ID=$(jq -r '.id // empty' "$SRC/manifest.json")
[[ -n "$PLUGIN_ID" ]] || { echo "register-launcher: manifest id missing" >&2; exit 1; }
VERSION=$(jq -r '.version // ""' "$SRC/manifest.json")
MENU_KEY="apps.omapaste"
TOGGLE="omarchy-shell shell toggle ${PLUGIN_ID} '{}'"

if [[ -f "$STAMP" && "$(cat "$STAMP")" == "$VERSION" ]]; then
  exit 0
fi

mkdir -p "$APP" "$ICON" "$(dirname "$MENU_EXT")"
install -Dm644 "$SRC/share/omapaste.desktop" "$APP/omapaste.desktop"
install -Dm644 "$SRC/share/omapaste.svg" "$ICON/omapaste.svg"

if [[ ! -f "$MENU_EXT" ]]; then
  cat >"$MENU_EXT" <<'EOF'
{
  // Extend the Omarchy menu — see /usr/share/omarchy/default/omarchy/omarchy-menu.jsonc
}
EOF
fi

python3 - "$MENU_EXT" "$MENU_KEY" "$TOGGLE" <<'PY'
import json
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
key = sys.argv[2]
toggle = sys.argv[3]
raw = path.read_text(encoding="utf-8")
stripped = re.sub(r"^\s*//[^\n]*(\n|$)", "", raw, flags=re.M)
stripped = re.sub(r",(\s*[}\]])", r"\1", stripped)
data = json.loads(stripped) if stripped.strip() else {}
data[key] = {
    "icon": "󰌪",
    "label": "Omapaste",
    "aliases": ["clipboard", "paste", "history", "omapaste"],
    "description": "Clipboard history bar",
    "action": toggle,
}
path.write_text(
    json.dumps(data, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
PY

if command -v omarchy >/dev/null; then
  omarchy menu refresh >/dev/null 2>&1 || true
fi

mkdir -p "$(dirname "$STAMP")"
printf '%s\n' "$VERSION" >"$STAMP"

if (( QUIET )); then
  exit 0
fi

echo "Launcher entries installed."
echo "  desktop: $APP/omapaste.desktop"
echo "  icon:    $ICON/omapaste.svg"
echo "  menu:    $MENU_EXT ($MENU_KEY)"
