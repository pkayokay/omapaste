# Omapaste

Clipboard history for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

![Omapaste clipboard history bar](share/screenshot.png)

Omapaste is a **Rust** GTK4 app. It sits in the background, remembers what you copy, and pops a card strip up from the **bottom of the screen** so you can grab an older clip without losing the current one.

This is v0.1. It is the bottom bar, clip history, per-clip keep time, select-to-copy, and Enter-to-paste. Pinboards, categories, and drag-and-drop are intentionally not in this release.

MIT licensed. Source and issue tracker: https://github.com/pkayokay/omapaste

## What it does

- Watches the Wayland clipboard (`wl-paste`) for text and PNG images
- Stores history locally in SQLite (`~/.local/share/omapaste/history.sqlite`)
- Toggles a GTK4 layer-shell bar anchored to the bottom of the screen
- Header shows **History** plus a search icon (type or click to expand search) and a keyboard-shortcuts popover
- Cards are a visual timeline, most recently used first: kind, age, preview, character count
- **Selecting** a card copies it back to the clipboard
- **Enter** (or double-click) pastes it into the window that was focused before the bar opened
- Each clip has a keep time: 1 hour, 1 day, 7 days, or forever (`Ctrl+K` cycles it)
- Follows the current Omarchy theme from `colors.toml`
- Skips password-manager / secret MIME types
- First launch seeds a few sample text clips (no images) so the bar is not empty

## Requirements

- **Rust (stable)** with `cargo` — `./install.sh` builds a release binary
- GTK 4 and gtk4-layer-shell
- `wl-clipboard` (`wl-copy` / `wl-paste`)
- `wtype` (for paste)
- Hyprland (for focus + the default keybind)

`./install.sh --hypr` still uses `python3` only to edit Hyprland lua config. The app itself is not Python.

## Install

### Omarchy plugin (Quattro)

The repo is also an Omarchy shell plugin (`io.github.pkayokay.omapaste`). Enabling it starts the GTK daemon from the shell and lets you summon the bar with `omarchy-shell shell summon io.github.pkayokay.omapaste '{}'`.

You still need the `omapaste` binary on `PATH` — `omarchy plugin add` only clones this repo; it does not build Rust.

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr          # builds binary, Super+Shift+V, Hyprland autostart
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

If the binary is already installed:

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

Remove the plugin listing (does not uninstall the binary or Hyprland bind):

```bash
omarchy plugin remove io.github.pkayokay.omapaste
omapaste quit                # stop the daemon if it is still running
```

This plugin runs unsandboxed inside `omarchy-shell` and shells out to `omapaste`. History stays local in SQLite. Super+Ctrl+V remains Omarchy’s built-in clipboard overlay.

### From source only

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr
omapaste daemon
```

`./install.sh --hypr` builds with Cargo, installs `omapaste` to `~/.local/bin`, autostarts the daemon, and binds **Super+Shift+V**.

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
| Delete or Backspace | Remove the highlighted clip (Backspace edits search if it has text) |
| Ctrl+K | Cycle keep time (1h → 1d → 7d → forever) |
| Type or click the magnifying glass | Search (History stays in the header) |
| Keyboard icon | Shortcut list |
| Help icon | Open the GitHub issue tracker |
| Esc | Close shortcuts, then search, then the bar |

## Keep time

New clips use the default from `~/.config/omapaste/config.toml` (`1d` unless you change it). Press Ctrl+K to cycle a clip's keep time.

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
omapaste toggle    # show / hide the bar (default if you pass no command)
omapaste show
omapaste hide
omapaste quit
omapaste --version
```

`start` is an alias for `daemon`. `stop` is an alias for `quit`.

The daemon is a single GTK application. `toggle` / `show` / `hide` talk to it over the session bus.

## Why not the built-in Omarchy clipboard?

Omarchy's Super+Ctrl+V picker (the shell clipboard plugin) is a vertical list that copies onto the clipboard. Omapaste is the Paste.app-shaped alternative: a bottom card timeline, per-clip retention, and paste-on-Enter into the app you were already in.

## Roadmap

Not in v0.1, on purpose:

- Categories / pinboards
- Drag a card into another app
- Richer image and file clips
- Sync

## Issues

Bugs, ideas, and patches: [github.com/pkayokay/omapaste/issues](https://github.com/pkayokay/omapaste/issues).

The bar’s help icon (next to shortcuts) opens that page. The shortcuts popover also shows the link, and a first-run sample clip is the same URL so you can copy it.

## Development

Contributor and agent notes (modules, tests, UI reload): [AGENTS.md](AGENTS.md).

```bash
cargo test
cargo test -- --ignored --test-threads=1   # GTK overlay smoke; needs a display
cargo fmt
./install.sh
omapaste quit && omapaste daemon           # load the new binary
# Super+Shift+V to open the bar
```

`./install.sh --hypr` also binds Super+Shift+V and autostarts the daemon. Do not run `omapaste toggle` from tests — it talks to the live daemon over D-Bus.

## License

MIT
