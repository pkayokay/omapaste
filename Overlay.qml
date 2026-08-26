import QtQuick
import Quickshell

// Summon bridge for the GTK bar. Quattro open/close map to the omapaste CLI;
// the real surface is the layer-shell window owned by the daemon.
//
// Do not declare `opened`: shell.isPluginOpen then tracks openPanelIds, which
// is what summon/hide already maintain. A local opened flag would desync when
// the user closes the GTK bar with Esc.
Item {
  id: root

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
  }

  function close() {
    root.run("hide")
  }

  function toggle() {
    root.run("toggle")
  }
}
