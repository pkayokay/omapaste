# TEMP: QML port plan (handoff)

**Branch:** `experiment/qml-feature-parity` → ready to merge to `main` as **0.3.0**  
**Status:** Feature parity + ship prep done (2026-08-30).

## Goals (both required)

1. Feature parity with GTK omapaste (user-facing behavior; not pixel-identical).
2. Honest non-manual install: `omarchy plugin add … --enable` with no `./install.sh` / no Rust binary.

Both met. After merge: tag `v0.3.0`, GitHub Release, optional marketplace re-verify for **standard installation**.

## How to test

```bash
cd /home/paulkim/Projects/omapaste
node tests/qml-parity.mjs
bash tests/qml-shell-parity.sh
omarchy plugin validate "$(pwd)"
# live:
ln -sfn "$PWD" ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
omarchy restart shell && omarchy plugin enable io.github.pkayokay.omapaste
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

After `History.js` / `Config.js` edits: always `omarchy restart shell`.

## Feature parity matrix

| Feature | GTK | QML | Tested |
| --- | --- | --- | --- |
| Watch text clipboard | yes | yes | live + shell |
| Watch PNG images | yes | yes | live + shell (CRC-validated) |
| Skip secrets / sensitive | yes | yes | shell |
| Persist history | sqlite | sqlite | live |
| Bottom card bar | yes | yes | live |
| Select → copy | yes | yes | paste.sh + live |
| Ignore-hash / ignore-until | yes | yes | shell |
| Enter paste → last window | yes | yes | paste.sh |
| Search / filter | yes | yes | unit + Overlay |
| Delete / Backspace | yes | yes | unit + Overlay |
| Esc stack | yes | yes | Overlay |
| Keep cycle + Ctrl+K | yes | yes | unit |
| Char / age labels | yes | yes | unit |
| Rename kind | yes | yes | unit + Overlay |
| Ctrl+1–9 / Ctrl+C | yes | yes | Overlay |
| Shortcuts / help | yes | yes | Overlay |
| Config | toml | qml-config.json | unit |
| One-command install | no | yes | validate |
| Optional Hypr bind | --hypr | docs | docs |
| Drag card into apps | yes | yes | unit helpers + manual |
| Standalone without Omarchy | yes | out of scope | accepted gap |
| Sample image seeds | yes | yes | Service |

## After merge (Phase 3)

1. Merge this branch to `main` and push.
2. Tag `v0.3.0` + GitHub Release from [CHANGELOG.md](../CHANGELOG.md) / [release.md](release.md).
3. Marketplace: verify SHA + **enable standard installation** ([omarchy-marketplace.md](omarchy-marketplace.md)).
4. Delete or archive this TEMP doc once main is QML-only in practice.

## Session log

| Date | Note |
| --- | --- |
| 2026-08-29 | MVP + parity review + drag spike |
| 2026-08-30 | Drag MIME filters, click-after-search fix, unit tests, 0.3.0 ship prep |
