# Omapaste

Clipboard history for [Omarchy](https://omarchy.org). Inspired by [Paste](https://pasteapp.io) for Mac.

Omapaste is an **Omarchy Quattro plugin**. It watches the Wayland clipboard, stores history as JSON, and summons a bottom card strip so you can grab an older clip without losing the current one. No separate daemon — install is one `omarchy plugin add` command.

MIT licensed. Source and issue tracker: https://github.com/pkayokay/omapaste

## What it does

- Watches the Wayland clipboard (`wl-paste`) for text and common image types (PNG, JPEG, WebP, …)
- Stores history locally under `~/.local/state/omapaste/`
- Bottom layer-shell bar: kind, age, keep, preview, character count
- Select to copy; Enter pastes into the window that was focused before the bar opened
- Drag cards into other apps; search, rename, and per-clip keep times
