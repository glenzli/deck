pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

ShapeDialog {
    id: root
    objectName: "cardPresetSaveDialog"
    required property CardLayoutPresetStore store
    property var configuration: ({})
    property string targetIdentity: ""
    property string currentIdentity: ""
    property string localError: ""
    readonly property bool targetCurrent: targetIdentity === currentIdentity
    parent: Overlay.overlay; anchors.centerIn: parent
    width: parent ? Math.min(460, parent.width - 40) : 460
    modal: true; title: qsTr("Save layout preset"); closePolicy: Popup.CloseOnEscape
    function openForLayout(value, identity) {
        configuration = JSON.parse(JSON.stringify(value)); targetIdentity = identity
        nameField.text = ""; localError = ""; open(); nameField.forceActiveFocus()
    }
    function save() {
        if (!targetCurrent) return false
        const id = store.savePreset(nameField.text, configuration)
        if (!id.length) { localError = store.error; return false }
        close(); return true
    }
    contentItem: ColumnLayout {
        spacing: 14
        Label { Layout.fillWidth: true; text: qsTr("Save the resolved layout only. Card text, images and the card back are not included."); color: Theme.muted; wrapMode: Text.WordWrap }
        ShapeTextField { id: nameField; objectName: "cardPresetName"; Layout.fillWidth: true; placeholderText: qsTr("Preset name"); maximumLength: 80; onTextEdited: root.localError = "" }
        Label { Layout.fillWidth: true; text: qsTr("Names are unique. Use a different name to keep an existing preset."); color: Theme.muted; font.pixelSize: 11; wrapMode: Text.WordWrap }
        Label { objectName: "cardPresetSaveError"; Layout.fillWidth: true; visible: root.localError.length > 0; text: root.localError; color: Theme.danger; wrapMode: Text.WordWrap }
        Label { Layout.fillWidth: true; visible: !root.targetCurrent; text: qsTr("The card set changed. Reopen this window for the current layout."); color: Theme.warning; wrapMode: Text.WordWrap }
        RowLayout {
            Layout.fillWidth: true
            ShapeButton { text: qsTr("Cancel"); quiet: true; onClicked: root.close() }
            Item { Layout.fillWidth: true }
            ShapeButton { objectName: "cardPresetSave"; text: qsTr("Save preset"); primary: true; enabled: root.targetCurrent && nameField.text.trim().length > 0; onClicked: root.save() }
        }
    }
}
