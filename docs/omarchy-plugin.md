# Omarchy plugin (Quattro)

Install omapaste as an [Omarchy](https://omarchy.org) shell plugin. Plugin ID: `io.github.pkayokay.omapaste`.

The repo ships both the **Rust GTK daemon** and a **Quattro wrapper** (`manifest.json`, `Service.qml`, `Overlay.qml`). You need both: the plugin starts and summons the bar; the binary watches the clipboard and draws the UI.

## Requirements

- **Rust (stable)** with `cargo` — `./install.sh` builds the binary
- GTK 4 and gtk4-layer-shell
- `wl-clipboard` (`wl-copy` / `wl-paste`)
- `wtype` (for paste into the last focused app)
- Hyprland (default keybind and focus restore)

On Omarchy / Arch:

```bash
sudo pacman -S --needed rust gtk4 gtk4-layer-shell wl-clipboard wtype pkgconf python
```

`./install.sh --hypr` uses `python3` only to edit `~/.config/hypr/*.lua`. The app is not Python.

## Install (plugin + binary)

`omarchy plugin add` clones the Quattro wrapper only — it does **not** build Rust. Run `install.sh` after adding the plugin (or from a git clone).

### From Omarchy or the marketplace

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
~/.config/omarchy/plugins/io.github.pkayokay.omapaste/install.sh --hypr
```

### From GitHub

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr          # builds binary; opt-in Hyprland bind + autostart
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

`./install.sh --hypr` is the **only** step that edits Hyprland config (`bindings.lua`, `autostart.lua`, layer rule). It writes `.bak.*` backups first. Plugin enable/disable alone does not touch those files.

If the binary is already on `PATH`:

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

### What `--hypr` sets up

- **Super+Shift+V** → `omapaste toggle` (Super+Ctrl+V stays on Omarchy’s built-in clipboard picker)
- Autostart `omapaste daemon`
- Layer rule so Hyprland does not fade the bar (it slides itself)

Without `--hypr`, `./install.sh` only installs `~/.local/bin/omapaste`. Bind and autostart yourself.

## From source only (no plugin)

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr
omapaste daemon
```

## Update

After pulling a new release:

```bash
cd ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
git pull
./install.sh
omapaste quit && omapaste daemon
```

Or remove and re-add:

```bash
omarchy plugin remove io.github.pkayokay.omapaste
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

Rebuild the binary whenever Rust sources change.

## Remove

```bash
omarchy plugin remove io.github.pkayokay.omapaste
omapaste quit
```

This removes the plugin listing only. It does not uninstall `~/.local/bin/omapaste`, delete history, or undo Hyprland edits from `./install.sh --hypr`. Undo Super+Shift+V and autostart in `~/.config/hypr/` manually if you added them with `--hypr`.

## Summon from the shell

```bash
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

Enabling the plugin also starts the daemon via `Service.qml`.

## Security and scope

- Runs unsandboxed inside `omarchy-shell` and shells out to `omapaste`
- Clipboard history stays local in SQLite under `~/.local/share/omapaste/`
- Super+Ctrl+V remains Omarchy’s built-in clipboard overlay — use **Super+Shift+V** for omapaste

## Marketplace listing

Listed on [omarchyplugins.com](https://omarchyplugins.com/plugin.html?id=io.github.pkayokay.omapaste) (manual setup — `install.sh` is still required after `omarchy plugin add`). Maintainer workflow: [omarchy-marketplace.md](omarchy-marketplace.md).
