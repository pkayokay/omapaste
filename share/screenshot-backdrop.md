# Omapaste

Clipboard manager for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

It watches the Wayland clipboard, stores history in SQLite, and summons a bottom card bar so you can grab an older clip without losing the current one.

MIT licensed. Source and issue tracker: https://github.com/pkayokay/omapaste

## What it does

- Watches the Wayland clipboard (`wl-paste`) for text and common image types (PNG, JPEG, WebP, …)
- Stores history locally under `~/.local/state/omapaste/`
- Bottom card bar: kind, age, keep, preview, character count
- Select to copy; Enter pastes into the window that was focused before the bar opened
- Drag cards into other apps; search, rename, and per-clip keep times
