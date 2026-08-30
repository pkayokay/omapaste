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
  property var config: Config.normalize(null)

  // GTK parity: src/ui.rs BAR_HEIGHT, CARD_*, SLIDE_PX, ANIM_DURATION.
  readonly property int barHeight: 356
  readonly property int cardWidth: 210
  readonly property int cardHeight: 280
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
    root.searchOpen = true
    root.shortcutsOpen = false
    root.kindEditing = false
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
    root.shortcutsOpen = false
    root.searchOpen = false
    root.filterText = ""
    root.kindEditing = false
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
    })
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
    var pruned = History.visibleHistory(root.history, Date.now() / 1000)
    root.history = pruned
    historyFile.setText(JSON.stringify(pruned.slice(0, root.config.max_items), null, 2) + "\n")
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
    root.filterText = nextFilter
    root.selectedIndex = 0
    root.kindEditing = false
    root.rebuildDisplay()
  }

  function select(delta) {
    if (displayModel.count === 0)
      return
    root.kindEditing = false
    root.selectedIndex = (root.selectedIndex + delta + displayModel.count) % displayModel.count
    cardList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    root.copySelected(false)
  }

  function selectAbsolute(index) {
    if (displayModel.count === 0)
      return
    root.kindEditing = false
    root.selectedIndex = Math.max(0, Math.min(index, displayModel.count - 1))
    cardList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    root.copySelected(false)
  }

  function currentRow() {
    if (displayModel.count === 0 || root.selectedIndex < 0 || root.selectedIndex >= displayModel.count)
      return null
    return displayModel.get(root.selectedIndex)
  }

  function copySelected(closeAfter) {
    var row = root.currentRow()
    if (!row)
      return
    if (row.entryType === "image")
      Quickshell.execDetached([root.pasteScript, "copy-image", row.path, row.mime || "image/png", row.hash || ""])
    else
      Quickshell.execDetached([root.pasteScript, "copy-text", row.fullText, row.hash || ""])
    if (closeAfter)
      root.dismiss()
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
    if (row.entryType === "image") {
      Quickshell.execDetached([
        root.pasteScript, "paste-image", row.path, row.mime || "image/png",
        row.hash || "", root.priorAddress, keys
      ])
    } else {
      Quickshell.execDetached([
        root.pasteScript, "paste-text", row.fullText, row.hash || "",
        root.priorAddress, keys
      ])
    }
    root.dismiss()
  }

  function removeSelected() {
    var row = root.currentRow()
    if (!row)
      return
    root.history = History.removeEntryAt(root.history, row.historyIndex)
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

  function beginKindEdit() {
    var row = root.currentRow()
    if (!row)
      return
    root.kindEditing = true
    root.kindEditText = row.kind || ""
  }

  function commitKindEdit() {
    if (!root.kindEditing)
      return
    var row = root.currentRow()
    root.kindEditing = false
    if (!row)
      return
    root.history = History.renameKindAt(root.history, row.historyIndex, root.kindEditText)
    root.saveHistory()
    root.rebuildDisplay()
  }

  function cancelKindEdit() {
    root.kindEditing = false
    root.kindEditText = ""
  }

  function openIssues() {
    Quickshell.execDetached(["xdg-open", root.issuesUrl])
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
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Keys.onPressed: function (event) {
      if (event.key !== Qt.Key_Escape)
        return
      if (root.kindEditing || root.searchOpen)
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

      MouseArea {
        anchors.fill: parent
        onClicked: {}
      }

      Column {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 8

        Row {
          width: parent.width
          height: 28
          spacing: 10

          // Brand: History + clip count (GTK op-title / op-count).
          Row {
            visible: !root.searchOpen
            spacing: 8
            height: 28

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

          // Search stack — closed slot (magnifier on trailing edge).
          Item {
            visible: !root.searchOpen
            width: parent.width - 76
            height: 28

            Rectangle {
              id: searchOpenBtn
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              width: 28
              height: 28
              color: searchOpenMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
              radius: 0
              Text {
                anchors.centerIn: parent
                text: "⌕"
                color: Util.alpha(root.foreground, searchOpenMa.containsMouse ? 1 : 0.8)
                font.pixelSize: Style.font.icon
              }
              MouseArea {
                id: searchOpenMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.openSearch("")
              }
            }
          }

          // Search stack — open row (28px).
          Row {
            visible: root.searchOpen
            width: parent.width - 76
            height: 28
            spacing: 6

            Text {
              text: "⌕"
              color: root.metaColor
              font.pixelSize: Style.font.icon
              anchors.verticalCenter: parent.verticalCenter
            }

            Item {
              width: parent.width - 56
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

            Rectangle {
              width: 28
              height: 28
              color: searchCloseMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
              radius: 0
              Text {
                anchors.centerIn: parent
                text: "×"
                color: Util.alpha(root.foreground, searchCloseMa.containsMouse ? 1 : 0.8)
                font.pixelSize: Style.font.body
              }
              MouseArea {
                id: searchCloseMa
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.closeSearch()
              }
            }
          }

          Rectangle {
            width: 28
            height: 28
            color: shortcutsMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
            radius: 0
            Text {
              anchors.centerIn: parent
              text: "⌨"
              color: Util.alpha(root.foreground, shortcutsMa.containsMouse ? 1 : 0.8)
              font.pixelSize: Style.font.bodySmall
            }
            MouseArea {
              id: shortcutsMa
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.shortcutsOpen = !root.shortcutsOpen
            }
          }

          Rectangle {
            width: 28
            height: 28
            color: issuesMa.containsMouse ? Util.alpha(root.foreground, 0.08) : "transparent"
            radius: 0
            Text {
              anchors.centerIn: parent
              text: "?"
              color: Util.alpha(root.foreground, issuesMa.containsMouse ? 1 : 0.8)
              font.pixelSize: Style.font.body
            }
            MouseArea {
              id: issuesMa
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.openIssues()
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
                  height: 44
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
                    anchors.bottomMargin: 8
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
                        enabled: card.selected
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
                      Keys.onPressed: function (event) {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                          root.commitKindEdit()
                          event.accepted = true
                        } else if (event.key === Qt.Key_Escape) {
                          root.cancelKindEdit()
                          event.accepted = true
                        }
                      }
                      Component.onCompleted: if (card.editingKind) forceActiveFocus()
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
                  height: parent.height - 44 - 36

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
                  height: 36
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

              MouseArea {
                anchors.fill: parent
                enabled: !card.editingKind
                z: -1
                onClicked: {
                  root.selectedIndex = card.index
                  root.copySelected(false)
                }
                onDoubleClicked: root.pasteSelected()
              }
            }
          }
        }
      }

      // Shortcuts panel
      Rectangle {
        visible: root.shortcutsOpen
        anchors.right: parent.right
        anchors.bottom: parent.top
        anchors.bottomMargin: 8
        anchors.rightMargin: 8
        width: 260
        height: shortcutsCol.implicitHeight + 20
        color: root.background
        border.color: root.border
        border.width: 1
        radius: 0
        z: 5

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
        id: keyCatcher
        anchors.fill: parent
        focus: true
        Keys.priority: Keys.BeforeItem
      Keys.onPressed: function (event) {
        if (root.kindEditing)
          return
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
          root.copySelected(true)
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
