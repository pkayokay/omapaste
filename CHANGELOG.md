# Changelog

## 0.1.0

- Paste-style bottom clipboard history bar for Omarchy / Hyprland
- Text and PNG history with per-clip keep times, search, and Enter-to-paste
- Omarchy Quattro plugin wrapper (`io.github.pkayokay.omapaste`): service starts the daemon; overlay summon maps to show/hide/toggle
- Service leaves the daemon running across Quattro plugin reloads (`omapaste quit` stops it)
- Overlay summon/hide follows shell `openPanelIds` (no local `opened` flag)
