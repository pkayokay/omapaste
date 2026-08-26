import QtQuick
import Quickshell

// Starts the omapaste GTK daemon while this plugin is enabled.
// The clipboard UI lives in that process, not in Quattro.
//
// Do not quit on destruction: Quattro destroys and recreates services on
// every plugin reload. Leaving the daemon up matches Hyprland autostart;
// `omapaste quit` is the explicit stop (see README remove steps).
Item {
  id: root

  property var shell: null
  property var manifest: null

  Component.onCompleted: root.ensureDaemon()

  function ensureDaemon() {
    Quickshell.execDetached([
      "bash", "-lc",
      "command -v omapaste >/dev/null 2>&1 && omapaste daemon"
    ])
  }
}
