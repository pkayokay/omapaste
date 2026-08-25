#!/usr/bin/env bash
# Install omapaste for the current user.
set -euo pipefail

SRC="$(cd "$(dirname "$0")" && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/omapaste"
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
APP="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
HYPR=0

for arg in "$@"; do
  case "$arg" in
    --hypr) HYPR=1 ;;
    --help|-h)
      echo "Usage: ./install.sh [--hypr]"
      echo "  --hypr   also bind Super+Ctrl+V and autostart the daemon"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

mkdir -p "$DEST" "$BIN" "$APP" "$ICON"
rm -rf "$DEST/omapaste"
cp -a "$SRC/omapaste" "$DEST/omapaste"
cp -a "$SRC/LICENSE" "$SRC/README.md" "$DEST/"
install -Dm644 "$SRC/share/omapaste.desktop" "$APP/omapaste.desktop"
install -Dm644 "$SRC/share/omapaste.svg" "$ICON/omapaste.svg"

cat > "$BIN/omapaste" <<EOF
#!/usr/bin/env python3
import os
import sys
os.environ.setdefault("PYTHONUNBUFFERED", "1")
sys.path.insert(0, "$DEST")
from omapaste.cli import main
raise SystemExit(main())
EOF
chmod +x "$BIN/omapaste"

echo "Installed omapaste to $BIN/omapaste"

if [[ $HYPR -eq 1 ]]; then
  python3 - "$SRC" <<'PY'
from pathlib import Path
import time

home = Path.home()
bindings = home / ".config/hypr/bindings.lua"
autostart = home / ".config/hypr/autostart.lua"
stamp = str(int(time.time()))

bind_mark = "omapaste toggle"
auto_mark = 'o.launch_on_start("omapaste daemon")'
bind_block = """
-- Omapaste clipboard history (https://github.com/pkayokay/omapaste)
hl.unbind("SUPER + CTRL + V")
o.bind("SUPER + CTRL + V", "Omapaste", "omapaste toggle")
"""
auto_line = 'o.launch_on_start("omapaste daemon")\n'

if bindings.exists() and bind_mark not in bindings.read_text():
    backup = bindings.with_suffix(bindings.suffix + f".bak.{stamp}")
    backup.write_text(bindings.read_text())
    bindings.write_text(bindings.read_text().rstrip() + "\n" + bind_block)
    print(f"Updated {bindings} (backup {backup.name})")
    print("Note: SUPER+CTRL+V was previously bound to Omarchy clipboard manager. An unbind was added so Omapaste can use it.")
elif bindings.exists():
    print(f"{bindings} already has an omapaste bind")

if autostart.exists() and auto_mark not in autostart.read_text():
    backup = autostart.with_suffix(autostart.suffix + f".bak.{stamp}")
    backup.write_text(autostart.read_text())
    autostart.write_text(autostart.read_text().rstrip() + "\n\n-- Omapaste clipboard history\n" + auto_line)
    print(f"Updated {autostart} (backup {backup.name})")
elif autostart.exists():
    print(f"{autostart} already launches omapaste")
PY
fi

echo
echo "Start the daemon:  omapaste daemon"
echo "Toggle the bar:    omapaste toggle"
