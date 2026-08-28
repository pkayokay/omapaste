# Omapaste

Clipboard history for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

![Omapaste clipboard history bar](share/screenshot.png)

Omapaste is a **Rust** GTK4 app. It sits in the background, remembers what you copy, and pops a card strip up from the **bottom of the screen** so you can grab an older clip without losing the current one.

MIT licensed. Source and issues: https://github.com/pkayokay/omapaste

## What it does

- Watches the Wayland clipboard (`wl-paste`) for text and PNG images
- Stores history locally in SQLite
- Toggles a GTK4 layer-shell bar anchored to the bottom of the screen
- Cards are a visual timeline, most recently used first: kind, age, preview, character count
- **Selecting** a card copies it; **Enter** (or double-click) pastes into the window that was focused before the bar opened
- Per-clip keep time (1h / 1d / 7d / forever) and search
- Follows the current Omarchy theme from `colors.toml`
- Skips password-manager / secret MIME types

## Install

Listed on the [Omarchy plugin marketplace](https://omarchyplugins.com/plugin.html?id=io.github.pkayokay.omapaste). `omarchy plugin add` installs the Quattro wrapper only — run `install.sh` once to build the binary.

**From Omarchy (recommended):**

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
~/.config/omarchy/plugins/io.github.pkayokay.omapaste/install.sh --hypr
```

`--hypr` is optional: binds **Super+Shift+V**, autostarts the daemon, and adds a Hyprland layer rule.

**From GitHub:**

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
./install.sh --hypr
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

Full steps, update, remove, and summon: [docs/omarchy-plugin.md](docs/omarchy-plugin.md). Config, keep time, data paths, and CLI: [docs/configuration.md](docs/configuration.md).

**Binary only (no plugin):** `./install.sh --hypr`, then `omapaste daemon`.

## Usage

| Input | Action |
| --- | --- |
| Super+Shift+V | Toggle the bar |
| ← → | Select a clip (also copies it) |
| Click a card | Select and copy |
| Ctrl+C | Copy the highlighted clip and close |
| Enter or double-click | Paste into the previous window and close |
| Ctrl+1–9 | Paste that card |
| Delete or Backspace | Remove the highlighted clip (Backspace edits search if it has text) |
| Ctrl+K | Cycle keep time (1h → 1d → 7d → forever) |
| Type or click the magnifying glass | Search |
| Keyboard icon | Shortcut list |
| Help icon | Open the GitHub issue tracker |
| Esc | Close shortcuts, then search, then the bar |

## Why not the built-in Omarchy clipboard?

Omarchy’s Super+Ctrl+V picker is a vertical list that copies onto the clipboard. Omapaste is the Paste.app-shaped alternative: a bottom card timeline, per-clip retention, and paste-on-Enter into the app you were already in.

## Docs

| Doc | Contents |
| --- | --- |
| [docs/omarchy-plugin.md](docs/omarchy-plugin.md) | Plugin install, update, remove, Hyprland bind |
| [docs/configuration.md](docs/configuration.md) | `config.toml`, keep time, runtime files, CLI |
| [docs/omarchy-marketplace.md](docs/omarchy-marketplace.md) | Catalog listing and re-verify after releases |
| [docs/release.md](docs/release.md) | Version bump, changelog, tag, and release checklist |
| [AGENTS.md](AGENTS.md) | Development, tests, module map |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

## Issues

Bugs, ideas, and patches: [github.com/pkayokay/omapaste/issues](https://github.com/pkayokay/omapaste/issues).

## License

MIT
