# Legacy GTK / Rust daemon

**Status:** inactive. The product path is the Quattro plugin (QML + `capture.sh` / `paste.sh`).

The `src/`, `Cargo.toml`, `install.sh`, and GTK-oriented tests remain in the tree as reference for behavior parity. They are **not** required to install or run Omapaste.

| GTK | QML plugin |
| --- | --- |
| GTK overlay | `Overlay.qml` |
| `history.sqlite` | `qml-history.json` |
| `config.toml` | `qml-config.json` |
| `install.sh` / `--hypr` | `omarchy plugin add` + optional hand-edited Hypr bind |
| Drag cards into apps | Supported in QML plugin (Qt Wayland drag) |

To run the old daemon (not needed for the plugin):

```bash
./install.sh
omapaste quit && omapaste daemon
```

Do not run both the GTK daemon and the QML service against the same workflow — quit the daemon with `omapaste quit`.
