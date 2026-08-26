# README screenshot

How to retake `share/screenshot.png` for the README.

The shot is a fullscreen desktop: one terminal showing the backdrop text, Omapaste along the bottom, first seed card selected, search and shortcuts closed.

Do not print `README.md` as the backdrop. It now embeds this screenshot, so it no longer matches. Use `share/screenshot-backdrop.md` — that is the terminal text from the original shot.

Theme, terminal, font, prompt, and clock will not match the committed PNG. That is fine. The bar follows the current Omarchy theme from `colors.toml`. What should match is the seed cards, the backdrop file, and the layout (terminal above, bar below, first card selected).

## Seeds

The bar in the shot is a fresh seed, not live history. Clips come from `SEED_CLIPS` in `src/store.rs`. Seed only runs when the sqlite file is missing:

```bash
omapaste quit
rm -f ~/.local/share/omapaste/history.sqlite
omapaste daemon
```

Do not copy anything else before the shot. After toggle, you should see **History · 7 clips** in this order:

| Preview | Keep |
| --- | --- |
| `fn greet(name: &str)` / `-> String {` / `format!("hi {name}")` | 7d |
| `← → select a clip.` / `Enter pastes it.` / `Esc closes the bar.` | forever |
| `https://omarchy.org` | 7d |
| `omarchy theme list` | 7d |
| `ssh git@github.com` | 7d |
| `Type to search.` / `Ctrl+K cycles keep` / `time.` | forever |
| `https://github.com/pkayokay/omapaste/issues` | forever |

Clip ages (just now vs 8m ago) do not need to match.

## Backdrop

From the repo root, in a single maximized terminal:

```bash
clear
bat --style=plain --paging=never -l markdown share/screenshot-backdrop.md
```

`bat --style=plain` is how the original got markdown colors without line numbers. `cat` is enough if `bat` is not installed. The shell prompt should sit in the empty space above the bar.

## Capture

Toggle the bar (`omapaste toggle`, or Super+Shift+V if `./install.sh --hypr` bound it). Leave the first card selected.

On Omarchy, Print or:

```bash
omarchy capture screenshot fullscreen save
```

Copy the saved PNG over `share/screenshot.png`. Any fullscreen capture that includes the bar and the terminal is fine.
