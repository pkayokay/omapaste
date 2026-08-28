# Configuration

Runtime settings, keep time, data paths, and CLI commands.

## Config file

Created on first launch at `~/.config/omapaste/config.toml`:

```toml
default_keep = "1d"      # 1h | 1d | 7d | forever
max_items = 200
max_bytes = 8000000
ignore_secrets = true
paste_keys = "auto"      # auto | shift-insert | ctrl-v
```

| Key | Default | Purpose |
| --- | --- | --- |
| `default_keep` | `1d` | Keep time for new clips (`1h`, `1d`, `7d`, `forever`) |
| `max_items` | `200` | Hard cap on stored clips; forever clips are pruned last |
| `max_bytes` | `8000000` | Skip clipboard payloads larger than this (bytes) |
| `ignore_secrets` | `true` | Skip password-manager / secret MIME types |
| `paste_keys` | `auto` | Keys sent after Enter when pasting into the last app |

`paste_keys = "auto"` sends Shift+Insert in terminals and Ctrl+V everywhere else, matching Omarchy’s universal clipboard.

## Toggle shortcut

Omapaste does **not** register a global hotkey and there is no `toggle_key` in `config.toml`. On Hyprland, bind `omapaste toggle` yourself:

```lua
-- ~/.config/hypr/bindings.lua
o.bind("SUPER + ALT + V", "Omapaste", "omapaste toggle")
```

`./install.sh --hypr` adds **Super+Shift+V** once (with a `.bak.*` backup). Edit or remove that line to use a different key.

Without a Hyprland bind: `omapaste toggle` from a terminal, script, or your compositor’s shortcut system. With the Quattro plugin enabled, the shell can also summon the overlay (`omarchy-shell shell summon io.github.pkayokay.omapaste '{}'`).

**In-bar shortcuts** (search, paste, Ctrl+K, drag, etc.) are fixed — see the keyboard icon in the bar.

## Keep time

New clips use `default_keep` until you change an individual card.

- Press **Ctrl+K** in the bar to cycle: 1h → 1d → 7d → forever
- Expired clips are deleted in the background
- Forever clips stay until you delete them and are dropped last when `max_items` is exceeded

## Runtime files

| Path | What |
| --- | --- |
| `~/.config/omapaste/config.toml` | Settings above |
| `~/.local/share/omapaste/history.sqlite` | Clip database |
| `~/.local/share/omapaste/images/` | PNG payloads as `{hash}.bin` |
| `~/.local/state/omarchy/current/theme/` | Omarchy theme (`colors.toml`) — do not edit `/usr/share/omarchy/` |

To reset sample clips (text only): quit the daemon, delete `history.sqlite`, start again. Seeding runs only when the DB file is missing.

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

The daemon is a single GTK application. `toggle` / `show` / `hide` talk to it over the session bus (`io.github.pkayokay.omapaste`). A second `omapaste daemon` does not replace a running process — `quit` first.

Debug:

```bash
RUST_LOG=debug omapaste daemon
```
