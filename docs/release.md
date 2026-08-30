# Release checklist

Steps for shipping a new omapaste version.

**Releases are not tied to the marketplace.** Tag, push, and GitHub Release are enough for users (`omarchy plugin update` / `git pull` in the plugin checkout). [omarchyplugins.com](https://omarchyplugins.com/) re-verify is a separate, optional step — only when you want the catalog page updated (see [omarchy-marketplace.md](omarchy-marketplace.md)).

## Versioning (Option A)

Pre-1.0 semver for omapaste:

| Bump | When | Example |
| --- | --- | --- |
| **Patch** `0.1.x` | Bug fixes, security hardening, docs-only changes | `0.2.0` → `0.2.1` |
| **Minor** `0.x.0` | Any user-visible feature or behavior change | `0.1.0` → `0.2.0` |
| **Major** `1.0.0` | Stable, production-ready API and behavior | later |

If a release mixes features and fixes, bump **minor**. Use **patch** only when nothing user-facing changed.

## Before you tag

1. **Finish changes on `main`** — feature/fix commits merged or committed locally.
2. **Run tests**
   ```bash
   node tests/qml-parity.mjs
   bash tests/qml-shell-parity.sh
   omarchy plugin validate "$(pwd)"
   ```
   Legacy GTK tree (optional): `cargo test` / `cargo fmt --check`.
3. **Pick a version** using the table above.
4. **Bump versions** (must match):
   - `manifest.json` → `version`
   - `CHANGELOG.md` → new `## X.Y.Z` section with user-facing bullets
   - `Cargo.toml` → `version` (legacy GTK crate; keep in sync)
5. **Update docs if behavior changed**
   - `README.md` usage table or install blurb
   - `docs/configuration.md` for new config keys
   - `docs/omarchy-plugin.md` for install/plugin changes
   - `preview.png` if the bar UI changed materially ([share/screenshot.md](../share/screenshot.md))
6. **Validate the plugin manifest**
   ```bash
   omarchy plugin validate "$(pwd)"
   ```

## Commit, tag, push, and GitHub release

A release is **tag + GitHub Release**, not just the git tag. Users browsing [github.com/pkayokay/omapaste/releases](https://github.com/pkayokay/omapaste/releases) only see GitHub Releases.

```bash
git add manifest.json Cargo.toml CHANGELOG.md   # plus any doc/UI files
git commit -m "Release vX.Y.Z."
git tag -a vX.Y.Z -m "Release vX.Y.Z."
git push origin main
git push origin vX.Y.Z
```

Then publish the GitHub Release from that tag (paste the new `CHANGELOG.md` section for the notes):

```bash
gh release create vX.Y.Z --title "vX.Y.Z" --notes "$(sed -n '/^## X.Y.Z$/,/^## /p' CHANGELOG.md | sed '$d')"
```

Or write the notes manually:

```bash
gh release create vX.Y.Z --title "vX.Y.Z" --notes-file /path/to/notes.md
```

GitHub Releases do **not** affect the Omarchy marketplace — catalog verification uses the **commit SHA** of the version-bump commit, not release metadata.

To replace a bad tag (only before users depend on it):

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
gh release delete vX.Y.Z --yes   # if you already published one
# fix, commit, then tag and gh release create again
```

## Load locally

```bash
ln -sfn "$(pwd)" ~/.config/omarchy/plugins/io.github.pkayokay.omapaste
omarchy plugin enable io.github.pkayokay.omapaste
omarchy restart shell
# Super+Shift+V (if bound) or:
omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
```

## Marketplace (optional)

**Not part of the release checklist** unless you explicitly want the catalog updated. Skip this section for normal releases.

Only if you want [omarchyplugins.com](https://omarchyplugins.com/) to show the new version as verified:

1. Use the commit SHA **of the release tag** (the version-bump commit).
2. Open the [plugin verification form](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=verify-plugin.yml).
3. Prefer **Verify the listed snapshot and enable standard installation** (one-command `omarchy plugin add` is honest as of 0.3.0).
4. Plugin ID: `io.github.pkayokay.omapaste` · repo: `https://github.com/pkayokay/omapaste` · full 40-char SHA.
5. After approval, update **Approved snapshot** in [omarchy-marketplace.md](omarchy-marketplace.md).

Do not open a second submission issue. Close a mistaken verification issue before opening a new one for the correct tag.

## Quick reference

| File | What to update |
| --- | --- |
| `manifest.json` | `version` |
| `CHANGELOG.md` | Release notes |
| `Cargo.toml` | `version` (legacy crate sync) |
| `README.md` | Usage/features if user-visible |
| `docs/omarchy-marketplace.md` | Approved snapshot after catalog promotion |
