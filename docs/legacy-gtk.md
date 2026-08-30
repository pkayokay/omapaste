# Legacy GTK / Rust daemon

**Status on `experiment/qml-feature-parity`:** inactive. The product path is the Quattro plugin (QML + `capture.sh` / `paste.sh`).

The `src/`, `Cargo.toml`, `install.sh`, and GTK-oriented tests remain in the tree as reference for behavior parity and for `main` until a merge decision. They are **not** required to install or run Omapaste on this branch.

| Old piece | Replacement |
| --- | --- |
| `omapaste daemon` | `Service.qml` watchers |
| GTK overlay | `Overlay.qml` |
| `history.sqlite` | `qml-history.json` |
| `config.toml` | `qml-config.json` |
| `install.sh` / `--hypr` | `omarchy plugin add` + optional hand-edited Hypr bind |
| Drag cards into apps | Not supported in QML port — copy/paste |

To run the old daemon (not needed for the plugin):

```bash
./install.sh
omapaste daemon
```

Do not run both the GTK daemon and the QML service against the same workflow while evaluating the experiment — quit the daemon with `omapaste quit`.
