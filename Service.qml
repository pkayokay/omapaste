import QtQuick
import Quickshell
import Quickshell.Io
import "History.js" as History
import "Config.js" as Config

// Clipboard watcher — Quattro-native omapaste (no Rust daemon).
Item {
  id: root

  property var shell: null
  property var manifest: null
  property var config: Config.normalize(null)
  property bool historyReady: false
  property var seedImageQueue: []

  readonly property string pluginDir: {
    var fromManifest = manifest && manifest.__sourceDir ? String(manifest.__sourceDir) : ""
    if (fromManifest !== "")
      return fromManifest
    var resolved = String(Qt.resolvedUrl("."))
    if (resolved.indexOf("file://") === 0)
      resolved = resolved.substring(7)
    if (resolved.length && resolved.charAt(resolved.length - 1) === "/")
      resolved = resolved.substring(0, resolved.length - 1)
    return resolved
  }
  readonly property string captureScript: root.pluginDir + "/capture.sh"
  readonly property string historyScript: root.pluginDir + "/history.py"
  readonly property string launcherScript: root.pluginDir + "/install-launcher.sh"
  readonly property string home: Quickshell.env("HOME") || ""
  readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (home + "/.local/state")
  readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || (home + "/.config")
  readonly property string historyDbPath: stateHome + "/omapaste/history.sqlite"
  readonly property string historyStagePath: stateHome + "/omapaste/qml-history.stage.json"
  readonly property string historyStampPath: stateHome + "/omapaste/history.sqlite.stamp"
  readonly property string configPath: configHome + "/omapaste/qml-config.json"
  readonly property string imageDir: stateHome + "/omapaste/qml-images"
  property var history: []
  property bool historyListQueued: false
  property bool historySaveQueued: false

  // Text samples aligned with src/store.rs SEED_CLIPS (text only in GTK seed).
  readonly property var seedClips: [
    {
      type: "text",
      text: "fn greet(name: &str)\n  -> String {\n  format!(\"hi {name}\")\n}",
      kind: "Code",
      keep: "7d",
      hash: "seed-rust-greet"
    },
    {
      type: "text",
      text: "← → select a clip.\nEnter pastes it.\nEsc closes the bar.",
      kind: "Tip",
      keep: "forever",
      hash: "seed-tip-nav"
    },
    {
      type: "text",
      text: "https://omarchy.org",
      kind: "Link",
      keep: "7d",
      hash: "seed-omarchy"
    },
    {
      type: "text",
      text: "Type to search.\nCtrl+K cycles keep\ntime.",
      kind: "Tip",
      keep: "forever",
      hash: "seed-tip-search"
    },
    {
      type: "text",
      text: "https://github.com/pkayokay/omapaste/issues",
      kind: "Help",
      keep: "forever",
      hash: "seed-issues"
    }
  ]

  readonly property var seedImagePaths: [
    "share/sample-images/sample-red.png",
    "share/sample-images/sample-blue.png"
  ]
  readonly property var seedImageKeeps: ["7d", "forever"]

  function loadHistory(raw) {
    var parsed = History.parseHistory(raw)
    root.history = parsed
    root.historyReady = true
    if (parsed.length === 0)
      root.seedIfEmpty()
  }

  function reloadHistory() {
    if (root.historyScript === "/history.py" || root.historyDbPath === "")
      return
    if (historyListProc.running) {
      root.historyListQueued = true
      return
    }
    historyListProc.command = [root.historyScript, "list", root.historyDbPath]
    historyListProc.running = true
  }

  function seedIfEmpty() {
    if (root.history.length > 0)
      return
    var now = Date.now() / 1000
    var next = []
    for (var i = 0; i < root.seedClips.length; i++) {
      var clip = JSON.parse(JSON.stringify(root.seedClips[i]))
      clip.ts = now - (root.seedClips.length - i) - root.seedImagePaths.length
      next = History.addEntry(next, clip, root.config.max_items, clip.keep || root.config.default_keep)
    }
    root.history = next
    root.seedImageQueue = root.seedImagePaths.slice()
    root.runNextSeedImage()
  }

  function runNextSeedImage() {
    if (root.seedImageQueue.length === 0) {
      root.saveHistory()
      return
    }
    var rel = root.seedImageQueue[0]
    var abs = root.pluginDir + "/" + rel
    var esc = abs.replace(/'/g, "'\\''")
    var cap = root.captureScript.replace(/'/g, "'\\''")
    seedImageProc.command = ["bash", "-lc", "test -f '" + esc + "' && cat '" + esc + "' | '" + cap + "' image/png"]
    seedImageProc.running = true
  }

  function unlinkImagePaths(paths) {
    var safe = History.managedImagePathsOnly(paths, root.imageDir)
    if (!safe || safe.length === 0)
      return
    Quickshell.execDetached(["rm", "-f"].concat(safe))
  }

  function persistHistoryDb(text) {
    historyStageFile.setText(text)
    root.historySaveQueued = true
    historySaveTimer.restart()
  }

  function flushHistoryDb() {
    if (!root.historySaveQueued)
      return
    root.historySaveQueued = false
    Quickshell.execDetached(["chmod", "600", root.historyStagePath])
    Quickshell.execDetached([root.historyScript, "save", root.historyDbPath, root.historyStagePath])
  }

  function saveHistory() {
    var now = Date.now() / 1000
    var before = root.history
    var pruned = History.pruneOmapasteImageRefClips(History.visibleHistory(root.history, now))
    var capped = pruned.slice(0, root.config.max_items)
    root.unlinkImagePaths(History.imagePathsRemoved(before, capped))
    root.history = capped
    root.persistHistoryDb(JSON.stringify(capped, null, 2) + "\n")
  }

  function addClipboardEntry(entry) {
    var normalized = History.normalizeEntry(entry)
    if (!normalized)
      return
    root.history = History.addEntry(root.history, normalized, root.config.max_items, root.config.default_keep)
    root.saveHistory()
  }

  function addClipboardJson(line) {
    var raw = String(line || "").trim()
    if (!raw)
      return
    if (raw.charAt(0) !== "{") {
      if (History.isOmapasteImageRef(raw))
        return
      return
    }
    root.addClipboardEntry(History.parseEntryJson(raw))
  }

  function stopWatchers() {
    clipboardWatchProc.running = false
  }

  function startWatchers() {
    if (root.pluginDir === "" || root.captureScript === "/capture.sh") {
      startWatchersTimer.interval = 500
      startWatchersTimer.start()
      return
    }
    // One watch covers text + any image/* the compositor advertises (not PNG-only).
    if (!clipboardWatchProc.running)
      clipboardWatchProc.running = true
  }

  function installLauncher() {
    if (root.pluginDir === "" || root.launcherScript === "/install-launcher.sh")
      return
    if (launcherProc.running)
      return
    var esc = root.launcherScript.replace(/'/g, "'\\''")
    launcherProc.command = ["bash", "-lc", "QUIET=1 exec '" + esc + "'"]
    launcherProc.running = true
  }

  Component.onCompleted: {
    root.reloadHistory()
    startWatchersTimer.start()
    installLauncherTimer.start()
  }
  Component.onDestruction: root.stopWatchers()

  Timer {
    id: historySaveTimer
    interval: 40
    repeat: false
    onTriggered: root.flushHistoryDb()
  }

  Timer {
    id: startWatchersTimer
    interval: 200
    repeat: false
    onTriggered: root.startWatchers()
  }

  Timer {
    id: installLauncherTimer
    interval: 300
    repeat: false
    onTriggered: root.installLauncher()
  }

  Process {
    id: launcherProc
    running: false
    command: []
  }

  Timer {
    interval: 60000
    running: true
    repeat: true
    onTriggered: {
      if (root.historyReady)
        root.saveHistory()
    }
  }

  FileView {
    id: configFile
    path: root.configPath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onLoaded: root.config = Config.parse(text())
    onLoadFailed: root.config = Config.normalize(null)
    onFileChanged: reload()
  }

  FileView {
    id: historyStageFile
    path: root.historyStagePath
    watchChanges: false
    atomicWrites: true
    printErrors: false
  }

  // Stamp bumps when history.py save completes so Service/Overlay stay aligned.
  FileView {
    id: historyStampFile
    path: root.historyStampPath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onLoaded: root.reloadHistory()
    onLoadFailed: root.reloadHistory()
    onFileChanged: reload()
  }

  Process {
    id: historyListProc
    running: false
    command: []
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.loadHistory(text)
    }
    onExited: {
      if (root.historyListQueued) {
        root.historyListQueued = false
        root.reloadHistory()
      }
    }
  }

  Process {
    id: seedImageProc
    running: false
    command: []
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var line = text.trim()
        if (line.length)
          root.addClipboardJson(line)
        if (root.seedImageQueue.length)
          root.seedImageQueue.shift()
        root.runNextSeedImage()
      }
    }
    onExited: {
      if (root.seedImageQueue.length)
        root.seedImageQueue.shift()
      root.runNextSeedImage()
    }
  }

  Process {
    id: clipboardWatchProc
    command: ["setpriv", "--pdeathsig", "TERM", "wl-paste", "--watch", root.captureScript]
    onExited: {
      if (root.pluginDir !== "")
        watchRestartTimer.restart()
    }
    stdout: SplitParser {
      onRead: function (data) {
        root.addClipboardJson(data)
      }
    }
  }

  Timer {
    id: watchRestartTimer
    interval: 1000
    repeat: false
    onTriggered: root.startWatchers()
  }
}
