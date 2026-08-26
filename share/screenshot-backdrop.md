# Omapaste

Clipboard history for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

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
