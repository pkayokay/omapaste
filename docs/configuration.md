# Configuration (QML / Quattro)

## `qml-config.json`

Optional settings at `~/.config/omapaste/qml-config.json`. If missing, defaults apply.

```json
{
  "default_keep": "1d",
  "max_items": 300,
  "max_bytes": 8000000,
  "paste_keys": "auto",
  "ignore_secrets": true
}
```

| Key | Values | Meaning |
| --- | --- | --- |
| `default_keep` | `1h`, `1d`, `7d`, `forever` | Keep preset for new clips |
| `max_items` | positive int | Cap on stored clips |
| `max_bytes` | positive int | Max bytes read per clipboard capture (clamped in `capture.sh`) |
| `paste_keys` | `auto`, `shift-insert`, `ctrl-v` | Keys sent after Enter-to-paste (`auto` uses Shift+Insert in terminals, Ctrl+V elsewhere) |
| `ignore_secrets` | bool | Skip password-manager / sensitive clipboard (`false` to allow) |

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
| `~/.local/state/omapaste/qml-images/` | Stored image clips (PNG, JPEG, WebP, …) |
| `~/.local/state/omapaste/qml-ignore-hash` | Ephemeral hash so self-copies are not re-ingested |

To reset sample tips: disable the plugin, delete `history.sqlite` (and the `.stamp` / stage files if present), enable again (seeds only when the DB has no rows).
