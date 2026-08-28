# AGENTS.md

Development contract for omapaste. User-facing docs: [README.md](README.md), [docs/](docs/) (start with [docs/release.md](docs/release.md) for shipping a version).

Omapaste is a **Rust GTK4** clipboard-history bar for Omarchy / Hyprland. It is **one GTK application**: the daemon owns the window and the clipboard watcher. `toggle` / `show` / `hide` / `quit` are extra invocations of the same binary; they talk to that process over the session bus (`io.github.pkayokay.omapaste`). A second `omapaste daemon` does not replace a running binary — quit first.

## First-time setup

Install omapaste per [README.md](README.md) (clone, `./install.sh`, optional plugin). You need a Wayland/Hyprland session to exercise the bar; `cargo test` does not.

Build dependencies on Omarchy / Arch: [docs/omarchy-plugin.md](docs/omarchy-plugin.md#requirements). Then:

```bash
git clone https://github.com/pkayokay/omapaste.git
cd omapaste
cargo test
```

If the binary is not on `PATH` yet: `./install.sh` (add `--hypr` for Super+Shift+V bind and autostart).

## Daily loop

```bash
cargo test
cargo fmt
./install.sh
omapaste quit && omapaste daemon     # load the new binary
# Super+Shift+V — do not use Super+Ctrl+V
```

GTK overlay smoke (needs a display, single-threaded):

```bash
cargo test -- --ignored --test-threads=1
```

Do **not** run `omapaste toggle` / `show` / `hide` from tests. Those hit the live daemon and move the real bar.

`install.sh` does not ship a systemd unit. If you started one yourself (`systemctl --user restart omapaste.service`), that works too; `omapaste quit && omapaste daemon` works everywhere.

Debug the running daemon:

```bash
RUST_LOG=debug omapaste daemon
# or, if you have a user unit:
journalctl --user -u omapaste.service -n 50 --no-pager
```

Look for panics and `Theme parser error`. Invalid GTK CSS (for example `overflow`) shows up there, not as a compile error.

## Where to change what

| Task | Files |
| --- | --- |
| Bar layout, keys, search, cards | `src/ui.rs` |
| Colors, radii, CSS | `src/theme.rs` |
| History, keep time, search query, seed clips | `src/store.rs` |
| Capture from clipboard, secret MIME skip | `src/clipboard.rs` |
| Paste into the last app, Hyprland focus | `src/paste.rs` |
| `config.toml` knobs | `src/config.rs` (`DEFAULT_CONFIG` + `load_config`) |
| CLI flags / commands | `src/cli.rs`, `src/main.rs` |
| XDG paths, `APP_ID`, issue URL | `src/paths.rs` |
| D-Bus commands, prune timer, theme watch | `src/app.rs` |
| Super+Shift+V, autostart, layer rule | `install.sh --hypr` (not `config.toml`) |
| Omarchy Quattro plugin wrapper | `manifest.json`, `Service.qml`, `Overlay.qml` |

| Path | Role |
| --- | --- |
| `src/main.rs` | Entry: `--version` / `--help` / `daemon\|toggle\|show\|hide\|quit` |
| `src/cli.rs` | Arg parsing |
| `tests/cli.rs` | Process tests for those flags |
| `share/` | Desktop file + SVG icon. README shot is `screenshot.png`; retake notes in `screenshot.md` |

## Runtime files

| Path | What |
| --- | --- |
| `~/.config/omapaste/config.toml` | Keep default, caps, paste keys, secret skip. Created on first launch. |
| `~/.local/share/omapaste/history.sqlite` | Clip DB. Delete it to re-seed samples (text only). |
| `~/.local/share/omapaste/images/` | PNG payloads as `{hash}.bin` |
| `~/.local/state/omarchy/current/theme/` | `colors.toml` + `theme.name`. Do not edit `/usr/share/omarchy/`. |
| `~/.config/hypr/bindings.lua` | Super+Shift+V |
| `~/.config/hypr/autostart.lua` | `omapaste daemon` |
| `~/.config/hypr/hyprland.lua` | `namespace = "omapaste"` layer rule |

There is no `toggle_key` in `config.toml`. Change the Hyprland bind.

## Testing

Default `cargo test` must stay headless and off the session bus.

- Store, config, theme, CLI parse, key intent, paste fakes: modules under `src/`.
- Binary flags: `tests/cli.rs`.
- GTK smoke: `ui::tests::overlay_builds_and_opens_search`, `#[ignore]`. Use a **different** application id and `ApplicationFlags::NON_UNIQUE`. `register` the app before creating windows. Construct, `refresh`, maybe open search. **Never** `show_rc` / `present`.
- Env (PATH, HOME, XDG_*): take `crate::env_lock()` first.
- Subprocess I/O: `paste.rs` `Proc` fake (`hyprctl` / `wtype` / `wl-copy`). Clipboard: `ingest(...)` plus a stub `wl-paste` on `PATH`. Do not require a real clipboard or Hyprland in unit tests.
- Store `list()` hides clips whose `keep_until` is in the past. Use `now: None` or a current timestamp if you expect rows back.

When you add behavior, add a test next to it.

## Building a feature

1. Pick the row in **Where to change what**.
2. Extract I/O behind a function or `Proc` so it can be faked. See `key_intent`, `ingest`, `window_from_hypr_json`.
3. Tests + `cargo test` (+ ignored GTK test if you touch overlay construction).
4. `./install.sh && omapaste quit && omapaste daemon`.
5. Super+Shift+V. For UI: search open/close (cards stay put), selected contrast, shortcuts popover, Esc (shortcuts → search → bar).

## Fixing a bug

1. Reproduce with Super+Shift+V (or the CLI). `RUST_LOG=debug`.
2. If it is store/config/theme/paste/CLI, write a failing test first.
3. Freeze / crash on click or arrows: check GTK CSS parser errors and anything that runs on the UI thread during select/copy.
4. Bar does not appear: is the daemon running? `omapaste daemon`. Did `quit` kill it? Is `~/.local/bin` on `PATH`? Another process already owns `io.github.pkayokay.omapaste`?
5. Paste goes to the wrong place: `paste_keys` in config; terminal detection in `paste.rs`; `wtype` installed.
6. Theme not updating: files under `~/.local/state/omarchy/current/theme/`.
7. History looks empty: expired clips are hidden; delete the sqlite file to re-seed.

## Invariants

- **Square chrome.** Omarchy rounding is 0. Bar, cards, search, buttons, popovers: `border-radius: 0`.
- **Search is 28px.** Opening it replaces History in-place and must not push the card row down.
- **GTK CSS has no `overflow`.** Invalid; logs a parser error. Clip children with `WidgetExt::set_overflow`.
- **Keep Super+Ctrl+V for Omarchy.**
- **Seed only when the DB file is missing.** Text samples only.
- **Copying a card sets an ignore-hash** so `wl-paste` does not record that copy as a new clip.

## Style

- Match neighboring Rust. Small functions, no speculative helpers.
- Commit messages are one sentence (`Cover core behavior with unit tests.`).
- Do not push unless asked.
