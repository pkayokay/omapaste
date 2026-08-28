# Changelog

## 0.2.0

- Drag-and-drop from clip cards into other apps
- Custom search field with caret, stable header layout, and in-place card filtering
- Double-click rename for clip kind labels with edit shortcuts and scroll
- Ctrl+A select-all with visible highlight in search and rename
- Ctrl+C copies the highlighted clip and closes the bar
- PNG sample images on first launch
- Security hardening: streamed `wl-paste` capture, private file permissions, safe drag temps, `cargo build --locked` in `install.sh`
- User docs in `docs/` (plugin install, configuration, release checklist, marketplace workflow)

## 0.1.0

- Paste-style bottom clipboard history bar for Omarchy / Hyprland
- Text and PNG history with per-clip keep times, search, and Enter-to-paste
- Omarchy Quattro plugin wrapper (`io.github.pkayokay.omapaste`): service starts the daemon; overlay summon maps to show/hide/toggle
- Service leaves the daemon running across Quattro plugin reloads (`omapaste quit` stops it)
- Overlay summon/hide follows shell `openPanelIds` (no local `opened` flag)
