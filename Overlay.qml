import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui
import "History.js" as History
import "Config.js" as Config

// Experimental Quattro-native omapaste bar (bottom cards).
// Summon: omarchy-shell shell summon io.github.pkayokay.omapaste '{}'
// Toggle (Super+Shift+V): omarchy-shell shell toggle io.github.pkayokay.omapaste '{}'
Item {
  id: root

  property var shell: null
  property var manifest: null
  property bool opened: false
  property bool shortcutsOpen: false
  property bool searchOpen: false
  property bool kindEditing: false
  property string kindEditText: ""
  property string filterText: ""
  property int selectedIndex: 0
  property string priorAddress: ""
  property var history: []
  property var pendingPasteArgv: null
  property int pendingTouchIndex: -1
  property var config: Config.normalize(null)
  property bool blockCardDrag: false
  property bool dragPanelHidden: false
  property int dragHistoryIndex: -1
  readonly property int pasteDelayMs: 160

  // GTK parity: src/ui.rs BAR_HEIGHT, CARD_*, SLIDE_PX, ANIM_DURATION.
  readonly property int barHeight: 356
  readonly property int cardWidth: 210
  readonly property int cardHeight: 280
  readonly property int cardHeaderHeight: 48
  readonly property int cardFooterHeight: 36
  readonly property int cardDragThresholdPx: 24
  readonly property int cardListHeight: cardHeight + 16
  readonly property int sideMargin: 18
  readonly property int visibleMargin: 14
  readonly property int slidePx: barHeight + 24
  property real slide: 1.0
  property bool hiding: false
  property var slideDoneCallback: null

  readonly property var activeScreen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null

  // Omarchy theme tokens (matches src/theme.rs CSS).
  readonly property color background: Util.alpha(Color.background, 0.96)
  readonly property color foreground: Color.foreground
  readonly property color border: Util.alpha(Color.foreground, 0.10)
  readonly property color accent: Color.accent
  readonly property color scrim: Util.alpha(Color.background, 0.5)
  readonly property color cardBackground: Util.alpha(Color.foreground, 0.04)
  readonly property color cardBorder: Util.alpha(Color.foreground, 0.08)
  readonly property color cardHeaderBackground: Util.alpha(Color.foreground, 0.07)
  readonly property color metaColor: Util.alpha(Color.foreground, 0.70)
  readonly property color selectedBackground: Util.alpha(Color.accent, 0.20)
  readonly property color selectedBorder: Color.accent
  readonly property color selectedHeaderBackground: Util.alpha(Color.foreground, 0.10)
  readonly property string fontFamily: Style.font.menuFamily
  readonly property string monoFamily: Style.font.resolvedFamily

  NumberAnimation {
    id: slideAnim
    target: root
    property: "slide"
    easing.type: Easing.OutQuint
    onStopped: {
      if (root.slideDoneCallback) {
        var cb = root.slideDoneCallback
        root.slideDoneCallback = null
        cb()
      }
    }
  }

  function clipCountLabel() {
    var n = History.visibleHistory(root.history, Date.now() / 1000).length
    return n + " clip" + (n === 1 ? "" : "s")
  }

  function animateSlide(target, onDone) {
    var start = root.slide
    var distance = Math.abs(target - start)
    if (distance < 0.01) {
      root.slide = target
      if (onDone)
        onDone()
      return
    }
    root.slideDoneCallback = onDone || null
    slideAnim.from = start
    slideAnim.to = target
    slideAnim.duration = Math.max(80, Math.round(220 * distance))
    slideAnim.start()
  }

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
  readonly property string pasteScript: root.pluginDir + "/paste.sh"
  readonly property string home: Quickshell.env("HOME") || ""
  readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (home + "/.local/state")
  readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || (home + "/.config")
  readonly property string historyPath: stateHome + "/omapaste/qml-history.json"
  readonly property string configPath: configHome + "/omapaste/qml-config.json"
  readonly property string issuesUrl: "https://github.com/pkayokay/omapaste/issues"

  function pluginId() {
    return (root.manifest && root.manifest.id) || "io.github.pkayokay.omapaste"
  }

  function open(payloadJson) {
    root.filterText = ""
    root.searchOpen = false
    root.selectedIndex = 0
    root.shortcutsOpen = false
    root.kindEditing = false
    root.pendingPasteArgv = null
    root.pendingTouchIndex = -1
    root.blockCardDrag = false
    root.dragPanelHidden = false
    root.dragHistoryIndex = -1
    if (dragAnchor.armed)
      dragAnchor.armed = false
    pasteAfterHideTimer.stop()
    root.opened = true
    root.slide = 1.0
    root.rebuildDisplay()
    priorWindowProc.running = true
    Qt.callLater(function () {
      root.animateSlide(0.0, function () {
        keyCatcher.forceActiveFocus()
      })
    })
  }

  function openSearch(prefix) {
    root.commitKindEditIfNeeded()
    root.searchOpen = true
    root.shortcutsOpen = false
    if (prefix && prefix.length)
      root.filterText = prefix
    root.rebuildDisplay()
    Qt.callLater(function () {
      searchField.forceActiveFocus()
      if (prefix && prefix.length)
        searchField.cursorPosition = searchField.text.length
    })
  }

  function closeSearch() {
    root.searchOpen = false
    root.filterText = ""
    root.rebuildDisplay()
    Qt.callLater(function () {
      keyCatcher.forceActiveFocus()
    })
  }

  function beginHide(onDone) {
    if (root.hiding)
      return
    root.hiding = true
    root.commitKindEditIfNeeded()
    root.shortcutsOpen = false
    root.searchOpen = false
    root.filterText = ""
    slideAnim.stop()
    root.animateSlide(1.0, function () {
      root.opened = false
      root.hiding = false
      root.slide = 1.0
      if (onDone)
        onDone()
    })
  }

  function close() {
    if (root.hiding)
      return
    if (!root.opened && root.slide >= 0.99)
      return
    root.beginHide(null)
  }

  function dismiss() {
    if (root.hiding)
      return
    if (!root.opened && root.slide >= 0.99)
      return
    root.beginHide(function () {
      if (root.shell && typeof root.shell.hide === "function")
        root.shell.hide(root.pluginId())
      root.applyPendingTouch()
      root.runPendingPaste()
    })
  }

  function applyPendingTouch() {
    if (root.pendingTouchIndex < 0)
      return
    var index = root.pendingTouchIndex
    root.pendingTouchIndex = -1
    root.history = History.touchEntryAt(root.history, index, Date.now() / 1000)
    root.saveHistory()
  }

  function runPendingPaste() {
    if (!root.pendingPasteArgv)
      return
    var argv = root.pendingPasteArgv
    root.pendingPasteArgv = null
    pasteAfterHideTimer.argv = argv
    pasteAfterHideTimer.restart()
  }

  function unlinkImagePaths(paths) {
    if (!paths || paths.length === 0)
      return
    Quickshell.execDetached(["rm", "-f"].concat(paths))
  }

  function toggle() {
    if (root.opened || root.slide < 0.99)
      root.dismiss()
    else
      root.open("{}")
  }

  function loadHistory(raw) {
    root.history = History.parseHistory(raw)
    if (root.opened)
      root.rebuildDisplay()
  }

  function saveHistory() {
    var now = Date.now() / 1000
    var before = root.history
    var pruned = History.pruneOmapasteImageRefClips(History.visibleHistory(root.history, now))
    var capped = pruned.slice(0, root.config.max_items)
    root.unlinkImagePaths(History.imagePathsRemoved(before, capped))
    root.history = capped
    historyFile.setText(JSON.stringify(capped, null, 2) + "\n")
  }

  function rebuildDisplay() {
    var query = root.searchOpen ? root.filterText : ""
    var rows = History.displayRows(root.history, query, 40, Date.now() / 1000)
    displayModel.clear()
    for (var i = 0; i < rows.length; i++) {
      var row = rows[i]
      displayModel.append({
        entryType: row.entryType,
        fullText: row.fullText,
        previewText: row.previewText,
        path: row.path,
        mime: row.mime,
        hash: row.hash,
        kind: row.kind,
        keepLabel: row.keepLabel,
        charLabel: row.charLabel,
        age: History.ageLabel(row.ts, Date.now() / 1000),
        historyIndex: row.historyIndex
      })
    }
    if (displayModel.count === 0)
      root.selectedIndex = 0
    else if (root.selectedIndex >= displayModel.count)
      root.selectedIndex = displayModel.count - 1
    Qt.callLater(function () {
      if (displayModel.count > 0)
        cardList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    })
  }

  function setFilter(nextFilter) {
    root.commitKindEditIfNeeded()
    root.filterText = nextFilter
    root.selectedIndex = 0
    root.rebuildDisplay()
  }

  function select(delta) {
    if (displayModel.count === 0)
      return
    root.commitKindEditIfNeeded()
    var next = root.selectedIndex + delta
    if (next < 0 || next >= displayModel.count)
      return
    root.selectedIndex = next
    cardList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    root.copySelected(false)
  }

  function selectAbsolute(index) {
    if (displayModel.count === 0)
      return
    root.commitKindEditIfNeeded()
    root.selectedIndex = Math.max(0, Math.min(index, displayModel.count - 1))
    cardList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    root.copySelected(false)
  }

  function currentRow() {
    if (displayModel.count === 0 || root.selectedIndex < 0 || root.selectedIndex >= displayModel.count)
      return null
    return displayModel.get(root.selectedIndex)
  }

  function copyRow(row, reorder, sync) {
    if (!row)
      return
    var argv
    if (row.entryType === "image")
      argv = [root.pasteScript, "copy-image", row.path, row.mime || "image/png", row.hash || ""]
    else
      argv = [root.pasteScript, "copy-text", row.fullText, row.hash || ""]
    if (sync)
      Util.execArgv(argv)
    else
      Quickshell.execDetached(argv)
    if (reorder)
      root.pendingTouchIndex = row.historyIndex
  }

  function copySelected(closeAfter, reorder) {
    root.copyRow(root.currentRow(), reorder)
    if (closeAfter)
      root.dismiss()
  }

  function cardDragAllowed() {
    return History.cardDragPrepareAllowed(root.kindEditing, root.blockCardDrag, root.dragPanelHidden)
  }

  function buildDragMime(row) {
    return History.dragMimeData(row.entryType, row.fullText, row.path, Util.fileUrl)
  }

  function hideNowForDrag() {
    root.shortcutsOpen = false
    root.searchOpen = false
    root.filterText = ""
    slideAnim.stop()
    root.slide = 1.0
    root.hiding = false
    root.opened = false
  }

  function reopenAfterDrag() {
    root.opened = true
    root.slide = 1.0
    root.rebuildDisplay()
    Qt.callLater(function () {
      root.animateSlide(0.0, function () {
        keyCatcher.forceActiveFocus()
      })
    })
  }

  function finishCardDrag(cancelled) {
    var hidden = root.dragPanelHidden
    root.dragPanelHidden = false
    dragAnchor.armed = false
    if (cancelled && hidden) {
      root.reopenAfterDrag()
      return
    }
    if (!hidden)
      return
    Util.execArgv([root.pasteScript, "arm-ignore", "", "5"])
    if (root.dragHistoryIndex >= 0) {
      root.pendingTouchIndex = root.dragHistoryIndex
      root.dragHistoryIndex = -1
      root.applyPendingTouch()
    }
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide(root.pluginId())
  }

  function beginCardDrag(index, pressX, pressY, cardItem) {
    if (dragAnchor.armed || root.dragPanelHidden)
      return
    if (!root.cardDragAllowed())
      return
    if (index < 0 || index >= displayModel.count)
      return
    var row = displayModel.get(index)
    if (!row)
      return
    if (row.entryType === "image" && !row.path)
      return
    root.selectedIndex = index
    Util.execArgv([root.pasteScript, "arm-ignore", row.hash || "", "15"])
    root.copyRow(row, false, true)
    root.dragHistoryIndex = row.historyIndex
    dragAnchor.mimeData = root.buildDragMime(row)
    var pos = cardItem.mapToItem(dragLayer, 0, 0)
    dragAnchor.x = pos.x
    dragAnchor.y = pos.y
    dragAnchor.width = root.cardWidth
    dragAnchor.height = root.cardHeight
    dragAnchor.hotSpotX = pressX
    dragAnchor.hotSpotY = pressY
    root.dragPanelHidden = true
    dragAnchor.armed = true
    Qt.callLater(function () {
      if (dragAnchor.armed)
        root.hideNowForDrag()
    })
  }

  function pasteSelected() {
    root.pasteIndex(root.selectedIndex)
  }

  function pasteIndex(index) {
    if (index < 0 || index >= displayModel.count)
      return
    var row = displayModel.get(index)
    if (!row)
      return
    var keys = root.config.paste_keys || "auto"
    root.pendingTouchIndex = row.historyIndex
    if (row.entryType === "image") {
      root.pendingPasteArgv = [
        root.pasteScript, "paste-image", row.path, row.mime || "image/png",
        row.hash || "", root.priorAddress, keys
      ]
    } else {
      root.pendingPasteArgv = [
        root.pasteScript, "paste-text", row.fullText, row.hash || "",
        root.priorAddress, keys
      ]
    }
    root.dismiss()
  }

  function removeSelected() {
    var row = root.currentRow()
    if (!row)
      return
    var before = root.history
    root.history = History.removeEntryAt(root.history, row.historyIndex)
    root.unlinkImagePaths(History.imagePathsRemoved(before, root.history))
    root.saveHistory()
    if (root.selectedIndex >= displayModel.count - 1)
      root.selectedIndex = Math.max(0, displayModel.count - 2)
    root.rebuildDisplay()
  }

  function cycleKeep() {
    var row = root.currentRow()
    if (!row)
      return
    root.history = History.cycleKeepAt(root.history, row.historyIndex, Date.now() / 1000)
    root.saveHistory()
    root.rebuildDisplay()
  }

  function commitKindEdit() {
    if (!root.kindEditing)
      return
    var row = root.currentRow()
    var text = root.kindEditText
    root.kindEditing = false
    root.kindEditText = ""
    if (!row) {
      root.restoreBarKeyFocus()
      return
    }
    root.history = History.renameKindAt(root.history, row.historyIndex, text)
    root.saveHistory()
    root.rebuildDisplay()
    root.restoreBarKeyFocus()
  }

  function commitKindEditIfNeeded() {
    if (root.kindEditing)
      root.commitKindEdit()
  }

  function cancelKindEdit() {
    root.kindEditing = false
    root.kindEditText = ""
    root.restoreBarKeyFocus()
  }

  function restoreBarKeyFocus() {
    Qt.callLater(function () {
      keyCatcher.forceActiveFocus()
    })
  }

  function beginKindEdit() {
    var row = root.currentRow()
    if (!row)
      return
    root.kindEditing = true
    root.kindEditText = row.kind || ""
  }

  function openIssues() {
    Util.execArgv(["xdg-open", root.issuesUrl])
    root.dismiss()
  }

  ListModel {
    id: displayModel
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
    id: historyFile
    path: root.historyPath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onLoaded: root.loadHistory(text())
    onLoadFailed: root.loadHistory("[]")
    onFileChanged: reload()
  }

  Process {
    id: priorWindowProc
    command: ["hyprctl", "activewindow", "-j"]
    running: false
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var win = JSON.parse(text)
          root.priorAddress = win && win.address ? String(win.address) : ""
        } catch (e) {
          root.priorAddress = ""
        }
      }
    }
  }

  Timer {
    id: pasteAfterHideTimer
    interval: root.pasteDelayMs
    repeat: false
    property var argv: []
    onTriggered: {
      if (argv.length)
        Quickshell.execDetached(argv)
      argv = []
    }
  }

  PanelWindow {
    id: panel
    screen: root.activeScreen
    visible: root.opened || root.hiding || root.slide < 0.99
    anchors {
      left: true
      right: true
      bottom: true
    }
    implicitHeight: root.activeScreen ? root.activeScreen.height : (root.slidePx + root.visibleMargin)
    color: "transparent"
    WlrLayershell.namespace: "omapaste"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: (root.opened && !root.dragPanelHidden)
        ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore

    Keys.onPressed: function (event) {
      if (event.key !== Qt.Key_Escape)
        return
      if (root.kindEditing) {
        root.cancelKindEdit()
        event.accepted = true
        return
      }
      if (root.searchOpen)
        return
      if (root.shortcutsOpen) {
        root.shortcutsOpen = false
        event.accepted = true
      } else if (root.opened) {
        root.dismiss()
        event.accepted = true
      }
    }

    Rectangle {
      anchors.fill: parent
      color: root.scrim
      opacity: (1.0 - root.slide) * 0.5
      MouseArea {
        anchors.fill: parent
        onClicked: {
          if (root.kindEditing)
            root.commitKindEdit()
          else if (root.shortcutsOpen)
            root.shortcutsOpen = false
          else
            root.dismiss()
        }
      }
    }

    Rectangle {
      id: barClipper
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.leftMargin: root.sideMargin
      anchors.rightMargin: root.sideMargin
      anchors.bottomMargin: root.visibleMargin
      height: root.barHeight
      color: "transparent"
      clip: true
      z: 1

      Rectangle {
        id: bar
        anchors.left: parent.left
        anchors.right: parent.right
        height: root.barHeight
        y: root.slide * root.slidePx
        color: root.background
        border.color: root.border
        border.width: 1
        radius: 0

      Column {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 8

        Row {
          id: headerRow
          width: parent.width
          height: 28
          spacing: 10

          Item {
            width: Math.max(0, headerRow.width - trailingIcons.width - headerRow.spacing)
            height: 28

            Row {
              visible: !root.searchOpen
              width: parent.width
              height: 28
              spacing: 8

              Text {
                text: "History"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.subtitle
                font.weight: Font.Bold
                anchors.verticalCenter: parent.verticalCenter
              }

              Text {
                text: root.clipCountLabel()
                color: root.metaColor
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                anchors.verticalCenter: parent.verticalCenter
              }
            }

            Row {
              visible: root.searchOpen
              width: parent.width
              height: 28
              spacing: 6

              Item {
                width: Style.font.iconLarge
                height: 28
                OpticalGlyph {
                  anchors.centerIn: parent
                  width: Style.font.iconLarge
                  height: Style.font.iconLarge
                  text: "󰍉"
                  fontFamily: root.fontFamily
                  fontSize: Style.font.icon
                  color: root.metaColor
                }
              }

              Item {
                width: parent.width - Style.font.iconLarge - 6
                height: 28

                Text {
                  anchors.fill: parent
                  visible: root.filterText.length === 0
                  text: "Search clips"
                  color: root.metaColor
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  verticalAlignment: Text.AlignVCenter
                }

                TextInput {
                  id: searchField
                  anchors.fill: parent
                  text: root.filterText
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  clip: true
                  selectByMouse: true
                  verticalAlignment: TextInput.AlignVCenter
                  onTextChanged: {
                    if (text !== root.filterText)
                      root.setFilter(text)
                  }
                  Keys.onPressed: function (event) {
                    if (event.key === Qt.Key_Escape) {
                      root.closeSearch()
                      event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                      root.pasteSelected()
                      event.accepted = true
                    } else if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_K) {
                      root.cycleKeep()
                      event.accepted = true
                    }
                  }
                }
              }
            }
          }

          Row {
            id: trailingIcons
            spacing: 0
            height: 28

            Rectangle {
              id: searchOpenBtn
              visible: !root.searchOpen
              width: Style.bar.iconSlot
              height: Style.bar.iconSlot
              color: searchOpenMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
              radius: 0
              Item {
                anchors.centerIn: parent
                width: Style.font.iconLarge
                height: Style.font.iconLarge
                OpticalGlyph {
                  anchors.fill: parent
                  text: "󰍉"
                  fontFamily: root.fontFamily
                  fontSize: Style.font.icon
                  color: Util.alpha(root.foreground, searchOpenMa.containsMouse ? 1 : 0.8)
                }
              }
              MouseArea {
                id: searchOpenMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                z: 1
                onClicked: root.openSearch("")
              }
            }

            Rectangle {
              id: shortcutsBtn
              width: Style.bar.iconSlot
              height: Style.bar.iconSlot
              color: shortcutsMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
              radius: 0
              Item {
                anchors.centerIn: parent
                width: Style.bar.iconCanvas
                height: Style.bar.iconCanvas
                OpticalGlyph {
                  anchors.fill: parent
                  text: "⌨"
                  fontFamily: root.fontFamily
                  fontSize: Style.bar.iconFont
                  color: Util.alpha(root.foreground, shortcutsMa.containsMouse ? 1 : 0.8)
                }
              }
              MouseArea {
                id: shortcutsMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                z: 1
                onClicked: {
                  root.commitKindEditIfNeeded()
                  root.shortcutsOpen = !root.shortcutsOpen
                }
              }
            }

            Rectangle {
              id: issuesBtn
              width: Style.bar.iconSlot
              height: Style.bar.iconSlot
              color: issuesMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
              radius: 0
              Item {
                anchors.centerIn: parent
                width: Style.bar.iconCanvas
                height: Style.bar.iconCanvas
                OpticalGlyph {
                  anchors.fill: parent
                  text: "?"
                  fontFamily: root.fontFamily
                  fontSize: Style.bar.iconFont
                  color: Util.alpha(root.foreground, issuesMa.containsMouse ? 1 : 0.8)
                }
              }
              MouseArea {
                id: issuesMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                z: 1
                onClicked: root.openIssues()
              }
            }
          }
        }

        Item {
          width: parent.width
          height: root.cardListHeight

          Text {
            anchors.centerIn: parent
            visible: displayModel.count === 0
            text: root.filterText.length ? "No matches" : "Copy something. It will show up here."
            color: root.metaColor
            font.family: root.fontFamily
            font.pixelSize: Style.font.subtitle
            horizontalAlignment: Text.AlignHCenter
          }

          ListView {
            id: cardList
            anchors.fill: parent
            visible: displayModel.count > 0
            orientation: ListView.Horizontal
            interactive: !dragAnchor.armed
            clip: true
            spacing: 10
            model: displayModel
            boundsBehavior: Flickable.StopAtBounds

            delegate: Rectangle {
              id: card
              required property int index
              required property string entryType
              required property string previewText
              required property string fullText
              required property string path
              required property string kind
              required property string keepLabel
              required property string charLabel
              required property string age

              readonly property bool selected: index === root.selectedIndex
              readonly property bool editingKind: selected && root.kindEditing
              readonly property color ink: card.selected ? Util.alpha(root.foreground, 0.92) : root.foreground

              width: root.cardWidth
              height: root.cardHeight
              color: card.selected ? root.selectedBackground : root.cardBackground
              border.color: card.selected ? root.selectedBorder : root.cardBorder
              border.width: card.selected ? 2 : 1
              radius: 0
              clip: true

              Column {
                anchors.fill: parent
                spacing: 0

                Rectangle {
                  width: parent.width
                  height: root.cardHeaderHeight
                  color: card.selected ? root.selectedHeaderBackground : root.cardHeaderBackground
                  border.color: Util.alpha(root.foreground, 0.12)
                  border.width: 0
                  Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: card.selected ? Util.alpha(root.foreground, 0.18) : Util.alpha(root.foreground, 0.12)
                  }

                  Column {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.topMargin: 8
                    anchors.bottomMargin: 12
                    spacing: 2

                    Text {
                      visible: !card.editingKind
                      width: parent.width
                      text: card.kind
                      color: card.ink
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.subtitle
                      font.weight: Font.DemiBold
                      elide: Text.ElideRight
                      MouseArea {
                        anchors.fill: parent
                        enabled: card.selected && !card.editingKind
                        // Let the card MouseArea own single-clicks; only steal doubles for rename.
                        propagateComposedEvents: true
                        onPressed: function (mouse) { mouse.accepted = false }
                        onClicked: function (mouse) { mouse.accepted = false }
                        onDoubleClicked: root.beginKindEdit()
                      }
                    }

                    TextInput {
                      visible: card.editingKind
                      width: parent.width
                      text: root.kindEditText
                      color: card.ink
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.subtitle
                      font.weight: Font.DemiBold
                      selectByMouse: true
                      onTextChanged: root.kindEditText = text
                      onVisibleChanged: {
                        if (visible)
                          Qt.callLater(function () { forceActiveFocus() })
                      }
                      Keys.onPressed: function (event) {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                          root.commitKindEdit()
                          event.accepted = true
                        } else if (event.key === Qt.Key_Escape) {
                          root.cancelKindEdit()
                          event.accepted = true
                        }
                      }
                    }

                    Text {
                      width: parent.width
                      text: card.age
                      color: card.selected ? Util.alpha(root.foreground, 0.92) : root.metaColor
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.bodySmall
                      elide: Text.ElideRight
                    }
                  }
                }

                Item {
                  width: parent.width
                  height: parent.height - root.cardHeaderHeight - root.cardFooterHeight

                  Image {
                    id: previewImage
                    anchors.centerIn: parent
                    visible: card.entryType === "image" && status === Image.Ready
                    width: 190
                    height: 140
                    source: card.entryType === "image" ? Util.fileUrl(card.path) : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    smooth: true
                    cache: false
                  }

                  Text {
                    anchors.centerIn: parent
                    visible: card.entryType === "image" && previewImage.status !== Image.Ready
                    width: parent.width - 24
                    text: "Image"
                    color: card.ink
                    opacity: 0.7
                    font.family: root.monoFamily
                    font.pixelSize: Style.font.body
                    horizontalAlignment: Text.AlignHCenter
                  }

                  Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 10
                    visible: card.entryType !== "image"
                    width: parent.width - 24
                    text: card.previewText
                    color: card.ink
                    font.family: root.monoFamily
                    font.pixelSize: Style.font.body
                    wrapMode: Text.WrapAnywhere
                    elide: Text.ElideRight
                    maximumLineCount: 7
                    lineHeight: 1.2
                  }
                }

                Rectangle {
                  width: parent.width
                  height: root.cardFooterHeight
                  color: "transparent"

                  Text {
                    anchors.centerIn: parent
                    text: card.charLabel
                    color: card.selected ? Util.alpha(root.foreground, 0.92) : root.metaColor
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.bodySmall
                  }
                }
              }

              // One MouseArea owns click + drag threshold. DragHandler fought TextInput
              // focus (search / rename) and ate subsequent card clicks.
              MouseArea {
                id: cardPointer
                anchors.fill: parent
                enabled: !card.editingKind
                z: 10
                hoverEnabled: false
                preventStealing: true
                property real pressX: 0
                property real pressY: 0
                property bool dragStarted: false
                property bool suppressClick: false

                onPressed: function (mouse) {
                  pressX = mouse.x
                  pressY = mouse.y
                  dragStarted = false
                  suppressClick = false
                }

                onPositionChanged: function (mouse) {
                  if (dragStarted || !(mouse.buttons & Qt.LeftButton))
                    return
                  if (!root.cardDragAllowed())
                    return
                  var dist = Math.abs(mouse.x - pressX) + Math.abs(mouse.y - pressY)
                  if (dist < root.cardDragThresholdPx)
                    return
                  dragStarted = true
                  suppressClick = true
                  root.beginCardDrag(card.index, pressX, pressY, card)
                }

                onClicked: {
                  if (suppressClick) {
                    suppressClick = false
                    return
                  }
                  var wasEditing = root.kindEditing
                  root.commitKindEditIfNeeded()
                  if (wasEditing)
                    root.blockCardDrag = true
                  root.selectedIndex = card.index
                  root.copySelected(false)
                  // Leave search/rename TextInput so later clicks and keys hit the bar.
                  if (!root.kindEditing)
                    root.restoreBarKeyFocus()
                }

                onDoubleClicked: root.pasteSelected()

                onReleased: {
                  root.blockCardDrag = false
                  dragStarted = false
                }

                onCanceled: {
                  dragStarted = false
                  suppressClick = false
                  root.blockCardDrag = false
                }
              }
            }
          }
        }
      }

      // Shortcuts panel — inside the bar; above the bar is clipped by barClipper.
      Rectangle {
        visible: root.shortcutsOpen
        anchors.top: parent.top
        anchors.topMargin: 48
        anchors.right: parent.right
        anchors.rightMargin: 8
        width: 260
        height: shortcutsCol.implicitHeight + 20
        color: root.background
        border.color: root.border
        border.width: 1
        radius: 0
        z: 30

        Column {
          id: shortcutsCol
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.top: parent.top
          anchors.margins: 10
          spacing: 4

          Text {
            text: "Shortcuts"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.title
          }
          Repeater {
            model: [
              { key: "← →", action: "Select" },
              { key: "Enter", action: "Paste" },
              { key: "Click", action: "Copy" },
              { key: "Ctrl+C", action: "Copy & close" },
              { key: "Drag", action: "Drop into apps" },
              { key: "Del", action: "Delete" },
              { key: "Ctrl+K", action: "Keep" },
              { key: "Type", action: "Search" },
              { key: "Esc", action: "Close" }
            ]
            delegate: Row {
              required property var modelData
              width: parent.width
              spacing: 20
              Text {
                text: modelData.key
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                width: 64
              }
              Text {
                text: modelData.action
                color: root.foreground
                opacity: 0.85
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                width: parent.width - 84
                wrapMode: Text.WordWrap
              }
            }
          }
          Text {
            width: parent.width
            text: "Report issues: " + root.issuesUrl
            color: root.metaColor
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            wrapMode: Text.WordWrap
          }
        }
      }

      Item {
        id: dragLayer
        anchors.fill: parent
        z: 200

        Item {
          id: dragAnchor
          visible: false
          property bool armed: false
          property var mimeData: ({})
          property real hotSpotX: 0
          property real hotSpotY: 0

          Drag.active: armed
          Drag.dragType: Drag.Automatic
          Drag.supportedActions: Drag.CopyAction
          Drag.mimeData: mimeData
          Drag.source: dragAnchor
          Drag.hotSpot.x: hotSpotX
          Drag.hotSpot.y: hotSpotY

          Drag.onDragFinished: function (dropAction) {
            // Wayland often reports IgnoreAction even on a successful drop.
            // GTK reopens only on drag_cancel, not drag_end — stay closed here.
            root.finishCardDrag(false)
          }
        }
      }

      Item {
        id: keyCatcher
        anchors.fill: parent
        z: -2
        focus: true
        Keys.priority: Keys.BeforeItem
      Keys.onPressed: function (event) {
        if (root.kindEditing) {
          if (event.key === Qt.Key_Escape) {
            root.cancelKindEdit()
            event.accepted = true
          }
          return
        }
        if (root.searchOpen)
          return

        var ctrl = !!(event.modifiers & Qt.ControlModifier)

        if (event.key === Qt.Key_Escape) {
          if (root.shortcutsOpen)
            root.shortcutsOpen = false
          else
            root.dismiss()
          event.accepted = true
        } else if (event.key === Qt.Key_Left) {
          root.select(-1)
          event.accepted = true
        } else if (event.key === Qt.Key_Right) {
          root.select(1)
          event.accepted = true
        } else if (event.key === Qt.Key_Home) {
          root.selectAbsolute(0)
          event.accepted = true
        } else if (event.key === Qt.Key_End) {
          root.selectAbsolute(displayModel.count - 1)
          event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          root.pasteSelected()
          event.accepted = true
        } else if (ctrl && (event.key === Qt.Key_C)) {
          root.copySelected(true, true)
          event.accepted = true
        } else if (ctrl && (event.key === Qt.Key_K)) {
          root.cycleKeep()
          event.accepted = true
        } else if (ctrl && event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
          root.pasteIndex(event.key - Qt.Key_1)
          event.accepted = true
        } else if (ctrl && (event.key === Qt.Key_F || event.key === Qt.Key_Slash)) {
          root.openSearch("")
          event.accepted = true
        } else if (event.key === Qt.Key_Slash && !ctrl) {
          root.openSearch("")
          event.accepted = true
        } else if (event.key === Qt.Key_Question || (event.key === Qt.Key_Slash && ctrl)) {
          root.shortcutsOpen = !root.shortcutsOpen
          event.accepted = true
        } else if (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) {
          root.removeSelected()
          event.accepted = true
        } else if (!ctrl && event.text && event.text.length === 1 && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127) {
          root.openSearch(event.text)
          event.accepted = true
        }
      }
      }
    }
  }
}
}
