# AGENTS.md

Notes for people and coding agents working on omapaste.

Omapaste is a **Rust GTK4** clipboard-history bar for Omarchy / Hyprland. It is one GTK application: the daemon owns the window and clipboard watcher; `toggle` / `show` / `hide` talk to that process over the session bus.

The user-facing overview is in [README.md](README.md). This file is the development contract.

## Commands

```bash
cargo test
cargo test -- --ignored --test-threads=1   # GTK overlay smoke; needs a display
cargo fmt
./install.sh                               # release build → ~/.local/bin/omapaste
systemctl --user restart omapaste.service  # pick up a new binary
journalctl --user -u omapaste.service -n 30 --no-pager
```

Live check after UI work: Super+Shift+V. Do not use Super+Ctrl+V — that is Omarchy's built-in picker.

**Never** run `omapaste toggle` / `show` / `hide` from tests. Those commands attach to the live daemon (`io.github.pkayokay.omapaste`) and will move the real bar.

## Layout

| Path | Role |
| --- | --- |
| `src/main.rs` | CLI entry: `--version` / `--help` / `daemon\|toggle\|show\|hide\|quit` |
| `src/cli.rs` | Arg parsing (unit-tested) |
| `src/app.rs` | GTK `Application`, D-Bus commands, theme watch, prune timer |
| `src/ui.rs` | Layer-shell overlay, cards, search, key handling |
| `src/clipboard.rs` | `wl-paste` watchers, secret MIME skip, ingest into the store |
| `src/store.rs` | SQLite history, keep times, search, seed clips |
| `src/paste.rs` | Hyprland focus, `wl-copy`, `wtype` paste keys |
| `src/theme.rs` | Omarchy `colors.toml` → GTK CSS |
| `src/config.rs` | `~/.config/omapaste/config.toml` |
| `src/paths.rs` | XDG paths, `APP_ID`, issue tracker URL |
| `install.sh` | Release build + optional Hyprland bind / autostart |
| `tests/cli.rs` | Process tests for `--version`, `-h`, unknown command |

Python appears only in `install.sh --hypr` (edits `~/.config/hypr/*.lua`). The app is not Python.

## Invariants

- **Square chrome.** Omarchy's live `decoration:rounding` is 0. Bar, cards, search, buttons, and popovers use `border-radius: 0`.
- **Search must not shove the cards.** Header controls and the search entry are 28px tall. Opening search replaces History in-place.
- **GTK CSS has no `overflow`.** That property is invalid here and logs a parser error. Clip children with `WidgetExt::set_overflow`.
- **Keep Super+Ctrl+V for Omarchy.** Default bind is Super+Shift+V via `./install.sh --hypr`.
- **One application id:** `io.github.pkayokay.omapaste`. GTK widget tests must use a different id and `ApplicationFlags::NON_UNIQUE`, and must `register` before creating windows.
- **Do not present the overlay in tests.** Construct it, `refresh`, maybe open search. Never `show_rc` / `present` — that maps a layer-shell bar on the user's desktop.
- **Env mutations need `crate::env_lock()`.** PATH, HOME, XDG_* tests share process env.
- **Subprocess I/O is fakeable.** `paste.rs` takes a `Proc` in tests (`hyprctl` / `wtype` / `wl-copy`). Clipboard capture is `ingest(...)` plus PATH stubs for `wl-paste`. Prefer that over talking to the real clipboard.
- **Seed only on a missing DB.** Sample clips are text-only; do not add images.
- **Theme files** live at `~/.local/state/omarchy/current/theme` (`colors.toml`, `theme.name`). Do not edit `/usr/share/omarchy/`.
- **Toggle key is a Hyprland bind**, not `config.toml`. There is no `toggle_key` setting.

## Testing

Default `cargo test` should stay headless and off the session bus.

- Logic, store, config, theme, CLI parse, key intent, paste fakes: `src/*/tests`.
- Binary flags: `tests/cli.rs`.
- GTK overlay smoke: `ui::tests::overlay_builds_and_opens_search`, `#[ignore]`, `--test-threads=1`.
- Clip timestamps in store tests must be current (or `None`) if you expect `list()` to return them — keep-until is compared to wall-clock time.

When adding behavior, add a test next to it. If it shells out, inject a fake or a stub binary; do not require Hyprland in unit tests.

## UI changes

1. Edit `src/theme.rs` / `src/ui.rs`.
2. `./install.sh && systemctl --user restart omapaste.service`
3. Super+Shift+V. Check search open/close (cards must stay put), selected card contrast, shortcuts popover, Esc layering.
4. `journalctl --user -u omapaste.service` — no CSS parser errors, no panics.

## Style

- Match neighboring Rust. Small functions, no speculative helpers.
- Commit messages are one sentence, like the existing log (`Cover core behavior with unit tests.`).
- Do not push unless asked.
- v0.1 does not include pinboards, categories, drag-and-drop, or sync.
