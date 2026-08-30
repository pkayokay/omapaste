#!/bin/bash
# Copy or paste a history entry for the QML omapaste experiment.

set -euo pipefail

STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/omapaste"
IGNORE_FILE="$STATE_DIR/qml-ignore-hash"
IGNORE_UNTIL_FILE="$STATE_DIR/qml-ignore-until"
IMAGE_DIR="$STATE_DIR/qml-images"
mkdir -p "$STATE_DIR" "$IMAGE_DIR"

mode="${1:-}"
shift || true

remember_ignore() {
  local hash="${1:-}"
  local seconds="${2:-1.5}"
  local new_until old
  new_until=$(python3 -c 'import sys, time; print(time.time() + float(sys.argv[1]))' "$seconds")
  if [[ -f "$IGNORE_UNTIL_FILE" ]]; then
    old=$(tr -d ' \n\r' <"$IGNORE_UNTIL_FILE")
    if [[ -n "$old" ]]; then
      new_until=$(python3 -c 'import sys; print(max(float(sys.argv[1]), float(sys.argv[2])))' "$old" "$new_until")
    fi
  fi
  if [[ -n "$hash" ]]; then
    printf '%s' "$hash" >"$IGNORE_FILE"
    chmod 600 "$IGNORE_FILE" 2>/dev/null || true
  fi
  printf '%s' "$new_until" >"$IGNORE_UNTIL_FILE"
  chmod 600 "$IGNORE_UNTIL_FILE" 2>/dev/null || true
}

# Reject path traversal / copies outside the plugin image store.
managed_image_path() {
  local path="$1"
  local real base
  [[ -n "$path" && -f "$path" ]] || return 1
  real=$(realpath -e "$path" 2>/dev/null || true)
  base=$(realpath -e "$IMAGE_DIR" 2>/dev/null || true)
  [[ -n "$real" && -n "$base" ]] || return 1
  [[ "$real" == "$base"/* ]] || return 1
  return 0
}

focus_address() {
  local address="${1:-}"
  if [[ -n "$address" ]]; then
    hyprctl dispatch focuswindow "address:$address" >/dev/null 2>&1 || true
    sleep 0.05
  fi
}

window_is_terminal() {
  local address="${1:-}"
  local json class
  json=$(hyprctl activewindow -j 2>/dev/null || true)
  if [[ -n "$address" ]]; then
    # Best-effort: after focus, active window should be the target.
    json=$(hyprctl activewindow -j 2>/dev/null || true)
  fi
  class=$(jq -r '.class // empty' <<<"$json" 2>/dev/null || true)
  case "${class,,}" in
    *kitty*|*alacritty*|*foot*|*wezterm*|*ghostty*|*konsole*|*gnome-terminal*|*tilix*|*termite*|*xdg-terminal-exec*)
      return 0
      ;;
  esac
  return 1
}

send_paste() {
  local paste_keys="${1:-auto}"
  local address="${2:-}"
  sleep 0.12
  case "$paste_keys" in
    shift-insert)
      wtype -M shift -k Insert -m shift 2>/dev/null || true
      ;;
    ctrl-v)
      wtype -M ctrl -k v -m ctrl 2>/dev/null || true
      ;;
    *)
      if window_is_terminal "$address"; then
        wtype -M shift -k Insert -m shift 2>/dev/null || wtype -M ctrl -k v -m ctrl 2>/dev/null || true
      else
        wtype -M ctrl -k v -m ctrl 2>/dev/null || wtype -M shift -k Insert -m shift 2>/dev/null || true
      fi
      ;;
  esac
}

case "$mode" in
arm-ignore)
  hash="${1:-}"
  seconds="${2:-1.5}"
  remember_ignore "$hash" "$seconds"
  ;;
copy-text)
  text="${1:-}"
  hash="${2:-}"
  remember_ignore "$hash"
  printf '%s' "$text" | wl-copy
  ;;
paste-text)
  text="${1:-}"
  hash="${2:-}"
  address="${3:-}"
  paste_keys="${4:-auto}"
  remember_ignore "$hash"
  printf '%s' "$text" | wl-copy
  focus_address "$address"
  send_paste "$paste_keys" "$address"
  ;;
copy-image)
  path="${1:-}"
  mime="${2:-image/png}"
  hash="${3:-}"
  managed_image_path "$path" || exit 1
  remember_ignore "$hash"
  wl-copy --type "$mime" <"$path"
  ;;
paste-image)
  path="${1:-}"
  mime="${2:-image/png}"
  hash="${3:-}"
  address="${4:-}"
  paste_keys="${5:-auto}"
  managed_image_path "$path" || exit 1
  remember_ignore "$hash"
  wl-copy --type "$mime" <"$path"
  focus_address "$address"
  send_paste "$paste_keys" "$address"
  ;;
*)
  echo "Usage: $0 arm-ignore|copy-text|paste-text|copy-image|paste-image ..." >&2
  exit 2
  ;;
esac
