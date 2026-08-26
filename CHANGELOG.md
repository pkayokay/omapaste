# Changelog

## 0.1.1

- Quattro service no longer quits the daemon on plugin reload (disable/remove still leave the binary; use `omapaste quit`)
- Overlay summon/hide follows shell `openPanelIds` instead of a local `opened` flag

## 0.1.0

- Paste-style bottom clipboard history bar for Omarchy / Hyprland
- Text and PNG history with per-clip keep times, search, and Enter-to-paste
- Omarchy Quattro plugin wrapper (`io.github.pkayokay.omapaste`): service starts the daemon; overlay summon maps to show/hide/toggle
