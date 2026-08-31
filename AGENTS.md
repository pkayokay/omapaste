# AGENTS.md

Development contract for omapaste.

> **Product path:** Quattro-native QML plugin. Read [docs/omarchy-plugin.md](docs/omarchy-plugin.md) first. User docs: [README.md](README.md). Marketplace: [docs/omarchy-marketplace.md](docs/omarchy-marketplace.md).

## QML daily loop

```bash
# edit Service.qml Overlay.qml History.js Config.js capture.sh paste.sh history.py
omarchy plugin validate "$(pwd)"
# after .pragma library JS changes:
omarchy restart shell
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

Live plugin checkout (dev):

```bash
ln -sfn "$(pwd)" ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
omarchy plugin enable io.github.pkayokay.omapaste
```

Do **not** `pkill` Omarchy’s stock clipboard watchers.

## Where to change what (QML)

| Task | Files |
| --- | --- |
| Bar layout, keys, search, cards, drag | `Overlay.qml` |
| Clipboard watch / history write | `Service.qml` |
| History helpers (keep, filter, rename, drag/MIME guards) | `History.js` |
| History SQLite list/save | `history.py` |
| Config defaults / parse | `Config.js` |
| Capture from clipboard | `capture.sh` |
| Paste / copy / focus / ignore window | `paste.sh` |
| Manifest | `manifest.json` |

## Runtime files

| Path | What |
| --- | --- |
| `~/.config/omapaste/qml-config.json` | Keep default, caps, paste keys, secret skip |
| `~/.local/state/omapaste/history.sqlite` | Clip history (SQLite) |
| `~/.local/state/omapaste/qml-images/` | Image payloads |
| `~/.config/hypr/bindings.lua` | Optional Super+Shift+V |

## Testing

```bash
node tests/qml-parity.mjs
bash tests/qml-shell-parity.sh
omarchy plugin validate "$(pwd)"
```

When you add behavior, add a test next to it (JS helpers in `History.js` / `Config.js`, shell cases in `tests/qml-shell-parity.sh`).

## Invariants

- **Square chrome.** Omarchy rounding is 0.
- **Search is 28px.** Opening it replaces History in-place and must not push the card row down.
- **Keep Super+Ctrl+V for Omarchy.**
- **Seed only when the history DB has no rows.**
- **Copying / dragging sets ignore-hash + ignore-until** so re-copy does not spam history; omapaste image path/URI text is never stored as a clip.
- **Drag unmaps the overlay** so drops reach apps below.

## Style

- Match neighboring QML/JS. Small functions, no speculative helpers.
- Commit messages are one sentence (`Cover core behavior with unit tests.`).
- Do not push unless asked.
