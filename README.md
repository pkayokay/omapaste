# Omapaste

Clipboard history for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

Omapaste sits in the background, remembers what you copy, and pops a card strip up from the **bottom of the screen** so you can grab an older clip without losing the current one.

This is v0.1. It is the bottom bar, clip history, per-clip keep time, select-to-copy, and Enter-to-paste. Pinboards, categories, and drag-and-drop are intentionally not in this release.

MIT licensed. https://github.com/pkayokay/omapaste

## What it does

- Watches the Wayland clipboard (`wl-paste`) for text and PNG images
- Stores history locally in SQLite (`~/.local/share/omapaste/history.sqlite`)
- Toggles a GTK4 layer-shell bar anchored to the bottom of the screen
- Cards are a visual timeline, most recently used first
- **Selecting** a card copies it back to the clipboard
- **Enter** (or double-click) pastes it into the window that was focused before the bar opened
- Each clip has a keep time: 1 hour, 1 day, 7 days, or forever
- Follows the current Omarchy theme from `colors.toml`
- Skips password-manager / secret MIME types

## Requirements

Omarchy already ships most of these:

- Python 3.11+
- GTK 4, PyGObject, gtk4-layer-shell
- `wl-clipboard` (`wl-copy` / `wl-paste`)
- `wtype` (for paste)
- Hyprland (for focus + the default keybind)

## Install

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr
omapaste daemon
```

`./install.sh --hypr` copies the app to `~/.local/share/omapaste`, puts `omapaste` on `~/.local/bin`, autostarts the daemon, and binds **Super+Shift+V**.

That key is free in Omarchy. Super+Ctrl+V stays on the built-in clipboard picker.

Without `--hypr`, install the binary only and bind it yourself.

## Usage

| Input | Action |
| --- | --- |
| Super+Shift+V | Toggle the bar |
| ← → | Select a clip (also copies it) |
| Click a card | Select and copy |
| Enter or double-click | Paste into the previous window and close |
| Ctrl+1–9 | Paste that card |
| Delete | Remove the selected clip |
| Ctrl+K | Cycle keep time (1h → 1d → 7d → forever) |
| Click the keep chip | Pick a keep time |
| Type | Search |
| Esc | Close (clears search first) |

## Keep time

New clips use the default from `~/.config/omapaste/config.toml` (`1d` unless you change it). Change any clip from the keep chip on its card, or press Ctrl+K.

Expired clips are deleted in the background. Forever clips are kept until you delete them, and they are the last to be dropped if you hit `max_items`.

## Config

Created on first launch at `~/.config/omapaste/config.toml`:

```toml
default_keep = "1d"      # 1h | 1d | 7d | forever
max_items = 200
max_bytes = 8000000
ignore_secrets = true
paste_keys = "auto"      # auto | shift-insert | ctrl-v
```

`paste_keys = "auto"` sends Shift+Insert in terminals and Ctrl+V everywhere else, matching Omarchy's universal clipboard.

## Commands

```bash
omapaste daemon    # start the watcher (autostart this)
omapaste toggle    # show / hide the bar
omapaste show
omapaste hide
omapaste quit
```

The daemon is a single GTK application. `toggle` / `show` / `hide` talk to it over the session bus.

## Why not the built-in Omarchy clipboard?

Omarchy's Super+Ctrl+V picker (the shell clipboard plugin) is a vertical list that copies onto the clipboard. Omapaste is the Paste.app-shaped alternative: a bottom card timeline, per-clip retention, and paste-on-Enter into the app you were already in.

## Roadmap

Not in v0.1, on purpose:

- Categories / pinboards
- Drag a card into another app
- Richer image and file clips
- Sync

## Development

```bash
python3 -m unittest discover -s tests -v
python3 -m omapaste daemon
python3 -m omapaste toggle
```

## License

MIT
