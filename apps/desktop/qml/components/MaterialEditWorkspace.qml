pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop

Item {
    id: workspace
    objectName: "materialEditWorkspace"
    required property DesktopBackend backend
    required property MaterialEditController controller
    required property AudioPreviewController audioPreview
    property string projectPath: ""
    property string outputArtifactId: ""
    property string operatorDraftId: ""
    property string inputArtifactId: ""
    property string inputRevisionId: ""
    property string sourceName: ""
    property bool imageMode: true
    property bool inputCurrent: false
    property var candidate: null
    property string acceptedImageSource: ""
    property string candidateImageSource: ""
    property string handoffFile: ""
    property string returnedFile: ""
    readonly property bool ownsRequest: controller.requestProjectPath === projectPath
        && controller.requestArtifactId === outputArtifactId
    readonly property string scopedError: ownsRequest ? controller.errorCode : ""
    readonly property bool ownRunning: ownsRequest && controller.running
    readonly property bool ownShadow: ownsRequest && controller.shadowActive
    readonly property bool hasCandidate: candidate !== null && candidate.artifactId === outputArtifactId

    function feedback() : string {
        if (ownRunning) return qsTr("Preparing edited material…")
        if (ownShadow) return qsTr("Edit in Shadow, then choose Return edited image. The result will appear here for review.")
        switch (scopedError) {
        case "material_session_changed": return qsTr("This stage changed. Reopen it and export a fresh working copy.")
        case "material_unchanged": return qsTr("The returned material is unchanged. No new version was created.")
        case "material_file_unavailable": return qsTr("The file or folder is unavailable. Choose a new writable folder or a valid file.")
        case "material_return_invalid": return imageMode ? qsTr("The Shadow return is incomplete or unsupported. Use an opaque 8-bit sRGB PNG, or choose a valid manual PNG and this session’s handoff.") : qsTr("Choose a valid WAV and this session’s handoff. Enable conversion for Echo PCM24 exports.")
        case "material_session_restarted": return qsTr("Previous returns were cancelled. Export a fresh copy to continue.")
        case "material_editor_unavailable": return qsTr("Choose the Shadow application or executable, then try opening it again. Manual working-copy import is also available.")
        case "material_editor_incomplete": return qsTr("Shadow closed without a completed return. Open it again to retry, or review a saved return manually.")
        default: return hasCandidate ? qsTr("Review the returned material, then adopt it below to continue downstream.") : ""
        }
    }
    onOutputArtifactIdChanged: {
        handoffFile = ""; returnedFile = ""; pcm24Conversion.checked = false
        const project = projectPath, output = outputArtifactId
        Qt.callLater(function() {
            if (workspace.projectPath === project && workspace.outputArtifactId === output)
                workspace.controller.restoreShadowSession(project, output)
        })
    }
    onInputRevisionIdChanged: { handoffFile = ""; returnedFile = ""; pcm24Conversion.checked = false }
    Connections {
        target: workspace.controller
        function onCopyCreated(path, artifactId): void {
            if (workspace.ownsRequest && artifactId === workspace.outputArtifactId)
                workspace.handoffFile = path
        }
    }
    FolderDialog {
        id: exportFolder
        title: qsTr("Choose a folder for the working copy")
        onAccepted: workspace.controller.exportCopy(workspace.projectPath, workspace.outputArtifactId, selectedFolder.toString())
    }
    FileDialog {
        id: shadowDialog
        title: qsTr("Choose Shadow application or executable")
        onAccepted: workspace.controller.setShadowExecutable(selectedFile)
    }
    FileDialog {
        id: handoffDialog
        title: qsTr("Choose the editing handoff")
        nameFilters: [qsTr("Shape handoff (handoff.json)")]
        onAccepted: workspace.handoffFile = selectedFile.toString()
    }
    FileDialog {
        id: returnDialog
        title: qsTr("Choose the edited material")
        nameFilters: workspace.imageMode ? [qsTr("Shadow result or PNG (*.json *.png)")] : ["WAV (*.wav)"]
        onAccepted: workspace.returnedFile = selectedFile.toString()
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12
        Text {
            Layout.fillWidth: true
            text: workspace.imageMode ? qsTr("Image editing in Shadow") : qsTr("Audio editing in Echo")
            color: Theme.text; font.pixelSize: Theme.fontHeading; font.weight: Font.DemiBold
        }
        Text {
            Layout.fillWidth: true
            text: qsTr("Original: %1 · Kept unchanged").arg(workspace.sourceName)
            color: Theme.muted; wrapMode: Text.Wrap; font.pixelSize: Theme.fontBody
        }
        Text {
            Layout.fillWidth: true
            text: workspace.imageMode
                ? qsTr("Open a separate working copy in Shadow. Known sRGB profiles use identical pixels in a compatible copy. Return the edited image here for review; the original stays unchanged.")
                : qsTr("Export a working copy, edit source.wav in Echo, then export a new WAV. Enable PCM24 conversion below to review Echo’s export.")
            color: Theme.muted; wrapMode: Text.Wrap; font.pixelSize: Theme.fontBody
        }
        Text {
            Layout.fillWidth: true; visible: !workspace.inputCurrent
            text: qsTr("The original has a newer version. Refresh the input explicitly before continuing.")
            color: Theme.accent; wrapMode: Text.Wrap
        }
        CheckBox {
            id: pcm24Conversion
            objectName: "materialPcm24Conversion"
            visible: !workspace.imageMode
            checked: false
            text: qsTr("Convert Echo PCM24 returns to PCM16 for Shape")
            ToolTip.visible: hovered
            ToolTip.text: qsTr("This reduces sample precision. The original export is kept, and conversion is recorded with the adopted version.")
        }
        Flow {
            Layout.fillWidth: true; spacing: 8
            ShapeButton {
                objectName: "materialOpenShadow"
                visible: workspace.imageMode
                text: qsTr("Open in Shadow")
                primary: true
                enabled: workspace.inputCurrent && !workspace.controller.running && !workspace.controller.shadowActive
                onClicked: workspace.controller.openShadow(workspace.projectPath, workspace.outputArtifactId)
            }
            ShapeButton {
                objectName: "materialChooseShadow"
                visible: workspace.imageMode
                text: qsTr("Choose Shadow…")
                enabled: !workspace.controller.running && !workspace.controller.shadowActive
                onClicked: shadowDialog.open()
            }
            ShapeButton {
                objectName: "materialExportCopy"
                text: qsTr("Export working copy")
                enabled: workspace.inputCurrent && !workspace.controller.running && !workspace.controller.shadowActive
                onClicked: exportFolder.open()
            }
            ShapeButton {
                text: qsTr("Choose handoff")
                enabled: !workspace.ownRunning && !workspace.ownShadow
                onClicked: handoffDialog.open()
            }
            ShapeButton {
                text: qsTr("Choose edited file")
                enabled: !workspace.ownRunning && !workspace.ownShadow
                onClicked: returnDialog.open()
            }
            ShapeButton {
                objectName: "materialReviewReturn"
                text: qsTr("Review returned material")
                enabled: workspace.inputCurrent && !workspace.controller.running && !workspace.controller.shadowActive && workspace.handoffFile.length > 0 && workspace.returnedFile.length > 0
                onClicked: workspace.controller.review(workspace.projectPath, workspace.outputArtifactId, workspace.handoffFile, workspace.returnedFile, pcm24Conversion.checked)
            }
        }
        Text {
            Layout.fillWidth: true
            visible: workspace.imageMode && workspace.controller.shadowExecutable.length > 0
            text: qsTr("Shadow: %1").arg(workspace.controller.shadowExecutable)
            elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: Theme.fontMeta
        }
        Text {
            Layout.fillWidth: true
            text: workspace.handoffFile.length > 0 ? qsTr("Handoff: %1").arg(workspace.handoffFile) : qsTr("A saved handoff lets you return to this stage after reopening Shape.")
            elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: Theme.fontMeta
        }
        Text {
            Layout.fillWidth: true; visible: workspace.returnedFile.length > 0
            text: qsTr("Returned file: %1").arg(workspace.returnedFile)
            elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: Theme.fontMeta
        }
        Text {
            objectName: "materialFeedback"
            Layout.fillWidth: true; text: workspace.feedback(); visible: text.length > 0
            color: Theme.accent; wrapMode: Text.Wrap; font.pixelSize: Theme.fontBody
        }
        ImageCompareWorkspace {
            Layout.fillWidth: true; Layout.fillHeight: true
            visible: workspace.imageMode
            acceptedSource: workspace.acceptedImageSource
            candidateSource: workspace.candidateImageSource
        }
        ColumnLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; visible: !workspace.imageMode
            ShapeButton {
                text: qsTr("Listen to original")
                onClicked: { if (workspace.audioPreview.loadPreview(workspace.inputArtifactId, "", workspace.inputRevisionId)) workspace.audioPreview.togglePlayback() }
            }
            ShapeButton {
                text: qsTr("Listen to returned audio"); enabled: workspace.hasCandidate
                onClicked: { if (workspace.audioPreview.loadPreview(workspace.outputArtifactId, workspace.candidate.id)) workspace.audioPreview.togglePlayback() }
            }
            Item { Layout.fillHeight: true }
        }
        Flow {
            Layout.fillWidth: true; spacing: 8
            ShapeButton {
                objectName: "materialCancelSession"
                text: qsTr("Cancel editing session")
                onClicked: { if (workspace.controller.restart(workspace.projectPath, workspace.outputArtifactId, false)) { workspace.handoffFile = ""; workspace.returnedFile = "" } }
            }
            ShapeButton {
                objectName: "materialRefreshInput"
                text: qsTr("Refresh input")
                enabled: !workspace.inputCurrent
                onClicked: workspace.controller.restart(workspace.projectPath, workspace.outputArtifactId, true)
            }
            Text {
                text: qsTr("Exported copies stay on disk after cancellation.")
                color: Theme.muted; font.pixelSize: Theme.fontMeta
            }
        }
    }
}
