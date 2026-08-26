import QtQuick
import Quickshell

// Starts the omapaste GTK daemon while this plugin is enabled.
// The clipboard UI lives in that process, not in Quattro.
Item {
  id: root

  Component.onCompleted: root.ensureDaemon()
  Component.onDestruction: root.quitDaemon()

  function ensureDaemon() {
    Quickshell.execDetached([
      "bash", "-lc",
      "command -v omapaste >/dev/null 2>&1 && omapaste daemon"
    ])
  }

  function quitDaemon() {
    Quickshell.execDetached([
      "bash", "-lc",
      "command -v omapaste >/dev/null 2>&1 && omapaste quit"
    ])
  }
}
