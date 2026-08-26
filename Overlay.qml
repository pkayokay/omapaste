import QtQuick
import Quickshell

// Summon bridge for the GTK bar. Quattro open/close/toggle map to the
// omapaste CLI; the real surface is the layer-shell window owned by the daemon.
Item {
  id: root

  property bool opened: false
  property var shell: null
  property var manifest: null

  function run(subcommand) {
    Quickshell.execDetached([
      "bash", "-lc",
      "command -v omapaste >/dev/null 2>&1 && omapaste " + subcommand
    ])
  }

  function open(payloadJson) {
    root.run("show")
    root.opened = true
  }

  function close() {
    root.run("hide")
    root.opened = false
  }

  function toggle() {
    root.run("toggle")
    root.opened = !root.opened
  }
}
