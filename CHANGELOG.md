# Changelog

## 0.3.0-qml-exp (experiment branch — unreleased)

- Quattro-native rewrite: clipboard watch + bottom bar in QML (no Rust daemon required)
- One-command install: `omarchy plugin add … --enable`
- JSON history, keep presets, search, paste-into-last-app, shortcuts panel
- Config: `~/.config/omapaste/qml-config.json`
- Drag-into-apps not supported on this path (copy/paste instead)
- Rust/GTK tree retained as legacy reference

## 0.2.1

- Report-issue icon closes the bar after opening the GitHub issue tracker
- README: marketplace-first install, v0.2.0 feature list, Hyprland toggle customization link
- Docs: contributor vs user split, listing live on omarchyplugins.com, agent re-verify workflow and prompt note for catalog updates, releases decoupled from marketplace

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
