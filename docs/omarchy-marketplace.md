# Omarchy plugin marketplace

Optional catalog metadata on [omarchyplugins.com](https://omarchyplugins.com/). **Independent of releases** — shipping a version does not require any step here. Users install and update from this GitHub repo.

**Agent note:** If the user says *re-verify on omarchyplugins* / *update the marketplace listing*, read [§ Agent: re-verify the catalog](#agent-re-verify-the-catalog) below. Do not use README.md or [release.md](release.md) for that workflow unless the release is not pushed yet.

## Plugin identity

| Field | Value |
| --- | --- |
| Plugin ID | `io.github.pkayokay.omapaste` |
| Marketplace repo | [HANCORE-linux/omarchy-plugin-marketplace](https://github.com/HANCORE-linux/omarchy-plugin-marketplace) |
| Listing issue | [#2893](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/2893) (closed) |
| Catalog page | [omarchyplugins.com](https://omarchyplugins.com/plugin.html?id=io.github.pkayokay.omapaste) |
| Approved snapshot | `3a0f91363022ba20e3e2677a58e4e61e7c441758` (v0.2.1, pending [#3040](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/3040)) |

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

See [omarchy-plugin.md](omarchy-plugin.md). On the **QML experiment branch**:

```bash
omarchy plugin add https://github.com/pkayokay/omapaste.git --enable
```

Optional hand-edited Hyprland bind for Super+Shift+V (see plugin doc). No `./install.sh`.

On **main** (GTK era, until merge), users still need `install.sh` after `plugin add` — catalog may show **Manual setup**.

## When to open a marketplace issue again

| Goal | Marketplace issue? |
| --- | --- |
| Ship a new GitHub release / tag | **No** — push, tag, users `git pull` + rebuild |
| Update the catalog page (version, preview, verified badge on a new commit) | **Yes** — verification form (below) |
| Enable standard `omarchy plugin add` install command on the catalog | **Yes** — verification form, “enable standard installation” (after code supports it) |
| Fix a broken listing | Comment on [#2893](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/2893) or open a verification issue; do not resubmit |

If `main` moves ahead of the approved snapshot without re-verification, the site may show **Update unverified**. The listing stays; only the verified metadata is stale.

## Re-publish the catalog after a release

See [release.md](release.md) for the full version-bump and tag workflow.

When the catalog should match a new commit:

1. Open the [plugin verification form](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/new?template=verify-plugin.yml).
2. Choose **Verify and publish a newer upstream commit**.
3. Fill in:
   - Plugin ID: `io.github.pkayokay.omapaste`
   - Repository: `https://github.com/pkayokay/omapaste`
   - Full 40-character SHA of the commit to promote
4. Wait for bot checks and maintainer `approved-and-verified`.
5. Update **Approved snapshot** at the top of this file.

CLI alternative: see [SUBMISSION.md § Update an existing listing](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/blob/main/SUBMISSION.md#update-an-existing-listing).

## Agent: re-verify the catalog

**README.md** is for end users (install, usage, remove). Agents use this file plus [release.md](release.md).

Omapaste is **already listed**. Only when the user asks to update the catalog — not on every release. After `main` is pushed, open a **plugin update** verification issue — not a new submission ([#2893](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/2893) is closed).

1. Push `main` and tag `vX.Y.Z` (see [release.md](release.md)).
2. Copy the **full 40-character SHA** of the `Release vX.Y.Z.` commit: `git rev-parse HEAD` on that commit.
3. Create the issue with **exact form headings** — no extra sections (no “Release notes”). Extra headings fail validation.
4. Wait for bot `validated` + maintainer `approved-and-verified`.
5. Update **Approved snapshot** at the top of this file.

Title: `[Verify]: Omapaste`

Body (replace `TARGET_SHA`):

```markdown
### Verification action

Verify and publish a newer upstream commit

### Plugin ID

io.github.pkayokay.omapaste

### Repository URL

https://github.com/pkayokay/omapaste

### Target commit

TARGET_SHA

### Verification acknowledgment

- [x] I understand that only the exact target commit can become a verified marketplace snapshot and that verification is not a security audit.
```

Do **not** add `### Standard installation acknowledgment` on main’s GTK listing. After the **QML port is merged** and one-command install is honest, use verification action **Verify the listed snapshot and enable standard installation** plus the standard-installation acknowledgment (see [TEMP-qml-port-plan.md](TEMP-qml-port-plan.md) Phase 3).

```bash
gh issue create \
  --repo HANCORE-linux/omarchy-plugin-marketplace \
  --title "[Verify]: Omapaste" \
  --body-file /tmp/omapaste-verify.md
```

Pre-flight: `omarchy plugin validate "$(pwd)"`, `manifest.json` version matches the release, README install/remove instructions current.

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
