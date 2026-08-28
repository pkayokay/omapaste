# Release checklist

Steps for shipping a new omapaste version. GitHub install/update does **not** require a marketplace issue; catalog re-verify is optional (see [omarchy-marketplace.md](omarchy-marketplace.md)).

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
   cargo test
   cargo fmt --check
   ```
   If you touched overlay construction: `cargo test -- --ignored --test-threads=1` (needs a display).
3. **Pick a version** using the table above.
4. **Bump versions** (all three must match):
   - `Cargo.toml` → `version`
   - `manifest.json` → `version`
   - `CHANGELOG.md` → new `## X.Y.Z` section with user-facing bullets
5. **Update docs if behavior changed**
   - `README.md` usage table or install blurb
   - `docs/configuration.md` for new config keys
   - `docs/omarchy-plugin.md` for install/plugin changes
   - `preview.png` if the bar UI changed materially ([share/screenshot.md](../share/screenshot.md))
6. **Validate the plugin manifest**
   ```bash
   omarchy plugin validate "$(pwd)"
   ```

## Commit, tag, push

```bash
git add Cargo.toml manifest.json CHANGELOG.md   # plus any doc/UI files
git commit -m "Release vX.Y.Z."
git tag -a vX.Y.Z -m "Release vX.Y.Z."
git push origin main
git push origin vX.Y.Z
```

To replace a bad tag (only before users depend on it):

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
# fix, commit, then tag again
```

Optional: create a GitHub Release from the tag (`gh release create vX.Y.Z --notes-file ...` or paste the CHANGELOG section).

## Load locally

```bash
./install.sh
omapaste quit && omapaste daemon
# Super+Shift+V
```

## Marketplace (optional)

Only if you want [omarchyplugins.com](https://omarchyplugins.com/) to show the new version as verified:

1. Use the commit SHA **of the release tag** (the version-bump commit).
2. Open the [plugin verification form](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=verify-plugin.yml).
3. Choose **Verify and publish a newer upstream commit**.
4. Plugin ID: `io.github.pkayokay.omapaste` · repo: `https://github.com/pkayokay/omapaste` · full 40-char SHA.
5. After approval, update **Approved snapshot** in [omarchy-marketplace.md](omarchy-marketplace.md).

Do not open a new submission issue — use verification only. Close a mistaken verification issue before opening a new one for the correct tag.

## Quick reference

| File | What to update |
| --- | --- |
| `Cargo.toml` | `version` |
| `manifest.json` | `version` |
| `CHANGELOG.md` | Release notes |
| `README.md` | Usage/features if user-visible |
| `docs/omarchy-marketplace.md` | Approved snapshot after catalog promotion |
