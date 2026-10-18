pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop

ShapeDialog {
    id: root
    objectName: "cardImageIntakeDialog"
    required property DesktopBackend backend
    required property CardKitController controller
    required property var appendImage
    required property var canAppend
    property int capacity: 0
    property bool canUseImageShape: false
    property bool useImageShape: true
    property var entries: []
    property string targetProject: ""
    property string targetPath: ""
    property string targetKit: ""
    property string error: ""
    property bool running: false
    property bool stopped: false
    property int idSequence: 0
    readonly property bool targetCurrent: targetProject === backend.projectId && targetPath === backend.bundlePath
        && targetKit === (controller.draft.artifact_id || "")
    readonly property int addedCount: entries.filter(item => item.state === "added").length
    readonly property int failedCount: entries.filter(item => item.state === "failed").length
    readonly property int waitingCount: entries.length - addedCount - failedCount

    parent: Overlay.overlay; anchors.centerIn: parent
    width: parent ? Math.min(790, parent.width - 60) : 790
    height: parent ? Math.min(680, parent.height - 60) : 680
    modal: true; title: qsTr("Add existing card images"); closePolicy: Popup.NoAutoClose
    function chooseFiles() {
        if (running) return false
        targetProject = backend.projectId; targetPath = backend.bundlePath
        targetKit = controller.draft.artifact_id || ""
        if (!targetProject.length || !targetKit.length) return false
        files.open(); return true
    }
    function prepareFiles(selected) {
        if (!targetCurrent || running) return false
        const seen = new Set(), next = []
        for (const file of selected) {
            const url = file.toString()
            if (seen.has(url)) continue
            seen.add(url)
            let name = url.split("/").pop()
            try { name = decodeURIComponent(name) } catch (_) {}
            next.push({file: url, name: name, id: "card-image-" + Date.now().toString(36) + "-" + (++idSequence).toString(36),
                state: "waiting", message: "", source: null, width: 0, height: 0, retryable: true})
        }
        entries = next; error = ""; stopped = false; useImageShape = canUseImageShape
        if (entries.length > capacity) error = qsTr("There is room for %1 more cards. Choose fewer images or start a new set.").arg(capacity)
        if (!entries.length) return false
        open(); return !error.length
    }
    function replace(index, entry) {
        const next = entries.slice(); next[index] = entry; entries = next
    }
    function beginAdding() {
        if (running || !targetCurrent || error.length || !entries.some(item => item.state !== "added" && item.retryable)) return false
        entries = entries.map(item => { const next = Object.assign({}, item); if (next.state === "failed" && next.retryable) { next.state = "waiting"; next.message = "" } return next })
        stopped = false; running = true; stepTimer.start(); return true
    }
    function stopAdding() { stepTimer.stop(); running = false; stopped = true }
    function step() {
        if (!running) return
        if (!targetCurrent) { stopAdding(); close(); return }
        const index = entries.findIndex(item => item.state === "waiting")
        if (index < 0) { running = false; return }
        const entry = Object.assign({}, entries[index])
        if (!canAppend(entry.id)) {
            entry.state = "failed"; entry.message = qsTr("The card list changed or is full. Keep the added cards and reopen this list.")
            replace(index, entry); running = false; return
        }
        if (!entry.source) {
            const before = new Set(backend.artifacts.map(item => item.id))
            if (!backend.importRaster(entry.file)) {
                entry.state = "failed"; entry.message = backend.lastError
                replace(index, entry); stepTimer.restart(); return
            }
            const imported = backend.artifacts.filter(item => !before.has(item.id)
                && item.kindKey === "image_raster" && item.hasAcceptedRevision)
            if (imported.length !== 1) {
                entry.state = "failed"; entry.message = qsTr("The image was imported, but its card could not be identified. Choose it from the project’s images.")
                entry.retryable = false
                replace(index, entry); running = false; return
            }
            entry.source = {artifact_id: imported[0].id, revision_id: imported[0].acceptedRevisionId}
            entry.width = imported[0].imageWidth; entry.height = imported[0].imageHeight
        }
        // Keep an imported source for a failed draft save; retry never imports it twice.
        const result = appendImage(entry.source, entry.id, entry.width, entry.height,
            useImageShape && addedCount === 0, targetProject, targetPath, targetKit, entries.map(item => item.id))
        entry.state = result.ok ? "added" : "failed"
        entry.message = result.message || ""
        replace(index, entry)
        if (!result.ok) { running = false; return }
        stepTimer.restart()
    }
    Timer { id: stepTimer; interval: 30; onTriggered: root.step() }
    onTargetCurrentChanged: if (!targetCurrent) { stopAdding(); files.close(); close() }
    FileDialog {
        id: files
        objectName: "cardImagesFileDialog"
        title: qsTr("Choose complete card images")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg)")]
        onAccepted: root.prepareFiles(selectedFiles)
    }
    contentItem: ColumnLayout {
        spacing: 10
        Label {
            Layout.fillWidth: true
            text: qsTr("Each image becomes one complete card with one copy, in the order shown below. Existing cards stay. Images are shown in full without cropping; file names are not printed on the cards.")
            wrapMode: Text.WordWrap; color: Theme.muted
        }
        ShapeCheckBox {
            objectName: "cardImagesUseShape"
            visible: root.canUseImageShape
            text: qsTr("Use the first added image’s proportions for this set")
            checked: root.useImageShape; enabled: !root.running && root.addedCount === 0
            onToggled: root.useImageShape = checked
        }
        Label { Layout.fillWidth: true; visible: root.error.length; text: root.error; color: Theme.danger; wrapMode: Text.WordWrap }
        Label {
            objectName: "cardImagesSummary"
            Layout.fillWidth: true
            text: qsTr("%1 added · %2 failed · %3 waiting").arg(root.addedCount).arg(root.failedCount).arg(root.waitingCount)
            color: Theme.text
        }
        Label { Layout.fillWidth: true; visible: root.stopped; text: qsTr("Adding stopped. Cards already added are kept; you can continue the remaining images."); color: Theme.muted; wrapMode: Text.WordWrap }
        ListView {
            objectName: "cardImagesPendingList"
            Layout.fillWidth: true; Layout.fillHeight: true; clip: true; spacing: 6
            model: root.entries
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                width: ListView.view.width
                height: rowContent.implicitHeight + 16; radius: 7; color: Theme.panel; border.color: Theme.border
                ColumnLayout {
                    id: rowContent
                    anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 8
                    Label { Layout.fillWidth: true; text: (row.index + 1) + ". " + row.modelData.name; color: Theme.text; elide: Text.ElideMiddle }
                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.state === "added" ? qsTr("Added") + (row.modelData.message ? " · " + row.modelData.message : "") : row.modelData.state === "failed" ? row.modelData.message : qsTr("Ready to add")
                        color: row.modelData.state === "failed" ? Theme.danger : Theme.muted; wrapMode: Text.WordWrap
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            ShapeButton { objectName: "cardImagesCancel"; text: root.running ? qsTr("Stop adding") : root.addedCount ? qsTr("Back to cards") : qsTr("Cancel"); onClicked: { if (root.running) root.stopAdding(); else root.close() } }
            Item { Layout.fillWidth: true }
            ShapeButton {
                objectName: "cardImagesApply"
                text: root.failedCount ? qsTr("Retry remaining images") : root.stopped ? qsTr("Continue adding") : qsTr("Add %1 cards").arg(root.entries.length)
                primary: true; enabled: !root.running && !root.error.length && root.targetCurrent && root.entries.some(item => item.state !== "added" && item.retryable)
                onClicked: root.beginAdding()
            }
        }
    }
}
