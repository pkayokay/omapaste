# Configuration (QML / Quattro)

## `qml-config.json`

Optional settings at `~/.config/omapaste/qml-config.json`. If missing, defaults apply (same spirit as the old `config.toml`).

```json
{
  "default_keep": "1d",
  "max_items": 300,
  "paste_keys": "auto",
  "ignore_secrets": true
}
```

| Key | Values | Meaning |
| --- | --- | --- |
| `default_keep` | `1h`, `1d`, `7d`, `forever` | Keep preset for new clips |
| `max_items` | positive int | Cap on stored clips |
| `paste_keys` | `auto`, `shift-insert`, `ctrl-v` | Keys sent after Enter-to-paste (`auto` uses Shift+Insert in terminals, Ctrl+V elsewhere) |
| `ignore_secrets` | bool | Reserved; capture always skips `x-kde-passwordManagerHint` / sensitive clipboard state today |

Example file in the repo: [share/qml-config.example.json](../share/qml-config.example.json).

Per-clip keep is cycled in the UI with **Ctrl+K** (does not change `default_keep`).

## Toggle shortcut

Omapaste does **not** register a global hotkey. Bind toggle yourself in Hyprland — see [omarchy-plugin.md](omarchy-plugin.md#optional-supershiftv).

Without a bind: `omarchy-shell shell summon io.github.pkayokay.omapaste '{}'`.

## Runtime files

| Path | What |
| --- | --- |
| `~/.config/omapaste/qml-config.json` | Settings above |
| `~/.local/state/omapaste/history.sqlite` | Clip history (SQLite; auto-migrates old `qml-history.json`) |
| `~/.local/state/omapaste/qml-images/` | PNG payloads |
| `~/.local/state/omapaste/qml-ignore-hash` | Ephemeral hash so self-copies are not re-ingested |

To reset sample tips: disable the plugin, delete `history.sqlite` (and the `.stamp` / stage files if present), enable again (seeds only when the DB has no rows).

## Legacy GTK paths

The old daemon used `~/.config/omapaste/config.toml` and `~/.local/share/omapaste/history.sqlite`. Those are unused by the QML plugin. See [legacy-gtk.md](legacy-gtk.md).
