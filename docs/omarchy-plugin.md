# Omarchy plugin (Quattro)

Plugin ID: `io.github.pkayokay.omapaste`.

Omapaste runs **entirely inside Omarchy shell** (service + overlay). Clipboard watch and UI live in the plugin checkout — there is no separate `omapaste` binary.

## Requirements

- Omarchy with Quattro / `omarchy-shell`
- `wl-clipboard` (`wl-copy` / `wl-paste`)
- `wtype` (paste into the last focused app)
- `python` and `jq` (capture helper)
- Hyprland (focus restore; optional toggle bind)

On Omarchy / Arch:

```bash
sudo pacman -S --needed wl-clipboard wtype python jq
```

## Install

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

That enables the plugin and registers Omapaste in app search (desktop entry + menu route) on first load. To reinstall launcher files manually: `./install-launcher.sh`.

Summon to confirm:

```bash
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

### Optional Super+Shift+V

Add to `~/.config/hypr/bindings.lua` (edit yourself; the plugin never rewrites Hyprland config):

```lua
o.bind("SUPER + SHIFT + V", "Omapaste", "omarchy-shell shell toggle io.github.pkayokay.omapaste '{}'")
```

Optional layer rule in `~/.config/hypr/hyprland.lua` if animations fight the bar:

```lua
hl.layer_rule({ match = { namespace = "omapaste" }, no_anim = true, animation = "none" })
```

Keep **Super+Ctrl+V** for Omarchy’s built-in clipboard.

## Update

```bash
cd ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
git pull
omarchy restart shell
```

Or:

```bash
omarchy plugin update io.github.pkayokay.omapaste
omarchy restart shell
```

## Remove

```bash
omarchy plugin remove io.github.pkayokay.omapaste
```

Watchers stop when the plugin is disabled/removed (and when the shell exits). This does **not** delete:

- `~/.local/state/omapaste/` (history + images)
- `~/.config/omapaste/qml-config.json`
- Any Hyprland bind you added by hand

Remove those manually if you want a full wipe. Remove the Super+Shift+V line from `bindings.lua` if you added it.

## Summon / hide / toggle

```bash
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
omarchy-shell shell hide io.github.pkayokay.omapaste
omarchy-shell shell toggle io.github.pkayokay.omapaste '{}'
```

Use **toggle** for Super+Shift+V so the bind opens and closes the bar.

## Security and scope

- Runs unsandboxed inside `omarchy-shell`; shells out to `wl-paste` / `wl-copy` / `wtype` / `hyprctl`
- History stays local under `~/.local/state/omapaste/`
- Drag cards into other apps (text and images)
- Super+Ctrl+V remains Omarchy’s built-in clipboard

## Marketplace

Catalog page: [omarchyplugins.com — Omapaste](https://omarchyplugins.com/plugin.html?id=io.github.pkayokay.omapaste). Maintainer re-verify workflow: [omarchy-marketplace.md](omarchy-marketplace.md).
