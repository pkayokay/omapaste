# README / marketplace screenshot

How to retake `share/screenshot.png` and root `preview.png` (keep them identical).

The shot is a fullscreen desktop: one terminal showing the backdrop text, Omapaste along the bottom, first card selected, search and shortcuts closed.

Do not print `README.md` as the backdrop. It embeds this screenshot, so it no longer matches. Use `share/screenshot-backdrop.md`.

Theme, terminal, font, prompt, and clock will not match a prior PNG. That is fine. The bar follows the current Omarchy theme. What should match is the seed cards, the backdrop file, and the layout (terminal above, bar below, first card selected).

## Seeds (QML)

The bar in the shot is a fresh seed, not live history. Clips come from `seedClips` / `seedImagePaths` in `Service.qml`. Seed only runs when the SQLite DB has no rows:

```bash
# Dev plugin checkout (this repo):
rm -f ~/.local/state/omapaste/history.sqlite \
      ~/.local/state/omapaste/history.sqlite.stamp \
      ~/.local/state/omapaste/qml-history.stage.json
# optional: clear sample image blobs too
rm -rf ~/.local/state/omapaste/qml-images
omarchy restart shell
```

Do not copy anything else before the shot. After summon you should see **History** with 7 clips (5 text seeds + 2 sample PNGs). Newest first: sample images, then the text tips/links/code.

Clip ages (just now vs minutes ago) do not need to match.

## Backdrop

From the repo root, in a single maximized terminal:

```bash
clear
bat --style=plain --paging=never -l markdown share/screenshot-backdrop.md
```

`bat --style=plain` is how the original got markdown colors without line numbers. `cat` is enough if `bat` is not installed. The shell prompt should sit in the empty space above the bar.

## Capture

Summon the bar:

```bash
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

Leave the first card selected. Search closed (no bordered field visible — default idle chrome).

On Omarchy, Print or:

```bash
omarchy capture screenshot fullscreen save
```

Copy the saved PNG over both:

```bash
cp /path/to/saved.png share/screenshot.png
cp /path/to/saved.png preview.png
```

Any fullscreen capture that includes the QML bar and the terminal is fine.
