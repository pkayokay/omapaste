# Omarchy plugin marketplace

How omapaste is listed on [omarchyplugins.com](https://omarchyplugins.com/), and what to do when we ship a new release.

The marketplace is **discovery and metadata**. Users still install and update from this GitHub repo. A catalog update is optional for day-to-day use.

## Plugin identity

| Field | Value |
| --- | --- |
| Plugin ID | `io.github.pkayokay.omapaste` |
| Marketplace repo | [HANCORE-linux/omarchy-plugin-marketplace](https://github.com/HANCORE-linux/omarchy-plugin-marketplace) |
| Listing issue | [#2893](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/2893) |
| Approved snapshot | `pending` (v0.2.0) |

Update the **Approved snapshot** line in this file after each successful catalog promotion.

## First-time listing (once)

1. Repo must have root `manifest.json`, `README.md`, `LICENSE`, QML entry points, and optional `preview.png`.
2. Run `omarchy plugin validate /path/to/omapaste` locally.
3. Open the [submit plugin form](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=submit-plugin.yml).
4. Wait for bot validation + security baseline, then maintainer `approved-and-verified`.
5. Listing appears on omarchyplugins.com after publication finishes.

Official guides: [Publish](https://omarchyplugins.com/publish) · [SUBMISSION.md](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/blob/main/SUBMISSION.md)

Do **not** open a second submission issue for the same plugin. Update the existing issue or use verification (below).

## What users actually do (no marketplace step)

See [omarchy-plugin.md](omarchy-plugin.md) for install, update, and remove. Short version:

```bash
./install.sh --hypr          # optional: Hyprland bind + autostart
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

Update after a release: `git pull` in the plugin dir, `./install.sh`, `omapaste quit && omapaste daemon`.

## When to open a marketplace issue again

| Goal | Marketplace issue? |
| --- | --- |
| Ship a new GitHub release / tag | **No** — push, tag, users `git pull` + rebuild |
| Update the catalog page (version, preview, verified badge on a new commit) | **Yes** — verification form |
| Fix a broken listing or failed publication | Comment on the existing issue; do not resubmit |

If `main` moves ahead of the approved snapshot without re-verification, the site may show **Update unverified**. The listing stays; only the verified metadata is stale.

## Re-publish the catalog after a release

See [release.md](release.md) for the full version-bump and tag workflow. When the catalog should match a new commit:

1. Open the [plugin verification form](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=verify-plugin.yml).
2. Choose **Verify and publish a newer upstream commit**.
3. Fill in:
   - Plugin ID: `io.github.pkayokay.omapaste`
   - Repository: `https://github.com/pkayokay/omapaste`
   - Full 40-character SHA of the commit to promote
4. Wait for bot checks and maintainer `approved-and-verified`.
5. Update **Approved snapshot** at the top of this file.

CLI alternative: see [SUBMISSION.md § Update an existing listing](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/blob/main/SUBMISSION.md#update-an-existing-listing).

## Maintainer notes worth repeating on updates

- `omarchy plugin add` installs the Quattro wrapper only; the `omapaste` binary must be on `PATH` (`./install.sh`).
- `./install.sh --hypr` is opt-in and is the only step that edits Hyprland config; plugin enable/disable does not.
- External deps: gtk4, gtk4-layer-shell, wl-clipboard, wtype.

## Checklist before any marketplace action

- [ ] `manifest.json` version matches the release we are promoting
- [ ] `omarchy plugin validate` passes on the repo root
- [ ] README install/remove instructions are current
- [ ] `preview.png` updated if the UI changed materially
- [ ] Full commit SHA copied (not a branch name)
