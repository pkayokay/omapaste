# TEMP: QML port plan (handoff)

**Branch:** `experiment/qml-feature-parity`  
**Status:** Phases 0–2 done + **parity re-reviewed with automated tests** (2026-08-29).  
**Do not commit / push / merge / marketplace-verify unless the user asks.**

## Goals (both required)

1. Feature parity with GTK omapaste (user-facing behavior; not pixel-identical).
2. Honest non-manual install: `omarchy plugin add … --enable` with no `./install.sh` / no Rust binary.

## How to test (agents)

```bash
cd /home/paulkim/Projects/omapaste   # on this branch
node tests/qml-parity.mjs            # History.js + Config.js
bash tests/qml-shell-parity.sh       # capture.sh + paste.sh
omarchy plugin validate "$(pwd)"
# live (needs session):
omapaste quit 2>/dev/null || true
ln -sfn "$PWD" ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
omarchy restart shell && omarchy plugin enable io.github.pkayokay.omapaste
# copy text/image, then:
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

After `History.js` / `Config.js` edits: always `omarchy restart shell` (`.pragma library` does not hot-reload).

---

## Feature parity matrix

| Feature | GTK | QML | Tested |
| --- | --- | --- | --- |
| Watch text clipboard | yes | yes | live + shell |
| Watch PNG images | yes | yes | live + shell (CRC-validated) |
| Skip secrets / sensitive | yes | yes | shell (MIME stub + CLIPBOARD_STATE) |
| Persist history | sqlite | JSON | live |
| Bottom card bar | yes | yes | live summon |
| Select → copy | yes | yes | paste.sh + ignore-hash live |
| Ignore-hash (no re-ingest) | yes | yes | shell + live |
| Enter paste → last window | yes | yes | paste.sh (wtype); interactive focus eyeball |
| Search / filter | yes | yes | unit + Overlay searchOpen (28px header) |
| Delete / Backspace | yes | yes | unit remove; Backspace deletes clip when search closed |
| Esc: shortcuts → search → bar | yes | yes | Overlay keys + searchField Esc |
| Theme tokens | GTK CSS | Color.menu.* | visual |
| Keep 1h/1d/7d/forever + Ctrl+K | yes | yes | unit cycleKeepAt / nextKeep |
| Expire hidden clips | yes | yes | unit isExpired / visibleHistory |
| Char count on cards | yes | yes | unit |
| Rename kind | yes | yes (TextInput MVP) | unit renameKindAt |
| Ctrl+1–9 paste nth | yes | yes | code review |
| Ctrl+C copy+close | yes | yes; not while searching | code review |
| Shortcuts / help | yes | `?` panel + `↗` issues | code review |
| Config caps / paste_keys / default_keep / max_bytes | toml | qml-config.json | unit Config.js |
| One-command install | no | yes (docs) | validate; no binary refs in QML |
| Optional Hypr bind (no install.sh) | --hypr | `shell toggle` in docs | docs |
| Drag card into apps | yes | **unsupported** (documented) | accepted gap |
| Standalone without Omarchy | yes | **out of scope** | accepted gap |
| Sample image seeds | yes | yes (red + blue PNG) | Service seedImageProc + samples |

---

## Bugs found in this review (fixed)

1. **`default_keep` from config ignored** — `normalizeEntry` always forced `1d` before `applyDefaultKeep`. Fixed in `History.js`.
2. **Ctrl+C while searching** copied a clip (GTK does not). Fixed in `Overlay.qml`.
3. **Corrupt/truncated PNGs** entered history and spam Qt `Error decoding`. Capture now CRC-validates PNG chunks; shell test rejects corrupt PNG; text size cap aligned to 8MB like GTK.
4. **Broken test PNG** left in live history / clipboard re-ingested on enable — pruned; clear clipboard before re-enable when scrubbing.

---

## Automated results (last run)

- `node tests/qml-parity.mjs` — **all passed**
- `bash tests/qml-shell-parity.sh` — **11 passed, 0 failed**
- `omarchy plugin validate` — **exit 0**
- Live: watchers up, text+image ingest, ignore-hash stable, summon/hide OK, no decode errors after prune

## Still manual / eyeball

- Paste into a terminal vs GUI app (`paste_keys=auto`)
- Super+Shift+V bind (`shell toggle` in `bindings.lua`)
- Search header UX (History + ⌕ closed; 28px row when open)

## Phase 3 (blocked on user)

Merge to `main` → marketplace verification with **enable standard installation**. Not started.

## Session log

| Date | Note |
| --- | --- |
| 2026-08-29 | MVP + Phase 1–2 |
| 2026-08-29 | Parity finish: GTK search header (searchOpen, 28px), image seeds, max_bytes, Hypr toggle bind docs. **No commit / no push.** |
