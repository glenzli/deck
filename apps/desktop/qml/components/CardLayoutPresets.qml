pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

ColumnLayout {
    id: root
    property var configuration: ({})
    property var savedPresets: []
    property CardLayoutPresetStore presetStore: null
    property string deleteId: ""
    property string deleteName: ""
    property string deleteError: ""
    signal chosen(var configuration)
    signal picked(var configuration, string name)
    readonly property var presets: [
        {key: "clean-text", name: qsTr("Focus"), mode: "text", align: "center", scale: 120, padding: 8, border: false, fit: "contain"},
        {key: "study", vertical: "top", name: qsTr("Study notes"), mode: "text", align: "left", scale: 100, padding: 6, border: false, fit: "contain"},
        {key: "quote", name: qsTr("Quote"), mode: "text", align: "center", scale: 100, padding: 12, border: true, fit: "contain"},
        {key: "picture", name: qsTr("Full picture"), mode: "image", align: "left", scale: 100, padding: 0, border: false, fit: "contain"},
        {key: "photo", name: qsTr("Photo crop"), mode: "image", align: "left", scale: 100, padding: 0, border: false, fit: "cover"},
        {key: "caption", position: "top", name: qsTr("Picture & note"), mode: "image_text", align: "left", scale: 100, padding: 4, border: false, fit: "contain"},
        {key: "framed", name: qsTr("Framed picture"), mode: "image_text", align: "center", scale: 100, padding: 8, border: true, fit: "contain"},
        {key: "bottom", name: qsTr("Text above picture"), mode: "image_text", position: "bottom", share: 55, align: "left", scale: 100, padding: 5, border: false, fit: "contain"},
        {key: "left", name: qsTr("Picture left"), mode: "image_text", position: "left", share: 40, vertical: "center", align: "left", scale: 100, padding: 5, border: false, fit: "contain"},
        {key: "right", name: qsTr("Picture right"), mode: "image_text", position: "right", share: 40, vertical: "center", align: "left", scale: 100, padding: 5, border: false, fit: "contain"},
        {key: "photo-caption", name: qsTr("Photo & caption"), mode: "image_text", position: "background", share: 65, align: "left", scale: 100, padding: 4, border: false, fit: "cover"},
        {key: "game", position: "top", share: 56, name: qsTr("Game card"), mode: "classic", align: "left", scale: 100, padding: 4, border: true, fit: "contain"}
    ]
    readonly property var allPresets: presets.concat(savedPresets.map(item => {
        const value = item.configuration
        return {key: "user-" + item.id, id: item.id, name: item.name, configuration: value, mode: value.mode,
            align: value.text_align, vertical: value.text_vertical_align, scale: value.text_scale_percent,
            padding: value.padding_percent, border: value.border, fit: value.image_fit,
            position: value.image_position, share: value.image_share_percent,
            background: value.background_color, textColor: value.text_color, blocks: value.blocks || []}
    }))
    spacing: 7
    function settings(preset) {
        if (preset.configuration) return JSON.parse(JSON.stringify(preset.configuration))
        return {mode: preset.mode, image_fit: preset.fit, text_align: preset.align,
            text_scale_percent: preset.scale, padding_percent: preset.padding, border: preset.border,
            background_color: "#ffffff", text_color: "#202020",
            image_position: preset.position || "auto", image_share_percent: preset.share || (preset.mode === "classic" ? 56 : 50),
            text_vertical_align: preset.vertical || "auto", text_offset_x_percent: 0, text_offset_y_percent: 0}
    }
    function selectPreset(preset) {
        const value = settings(preset)
        chosen(value); picked(value, preset.name)
    }
    function requestDelete(id, name) {
        if (!presetStore || !savedPresets.some(item => item.id === id)) return false
        deleteId = id; deleteName = name; deleteError = ""; deleteDialog.open(); return true
    }
    function confirmDelete() {
        if (!presetStore || !deleteId.length) return false
        if (!presetStore.removePreset(deleteId)) { deleteError = presetStore.error; return false }
        deleteDialog.close(); return true
    }
    ShapeDialog {
        id: deleteDialog
        objectName: "cardPresetDeleteDialog"
        parent: Overlay.overlay; anchors.centerIn: parent
        width: parent ? Math.min(460, parent.width - 40) : 460
        modal: true; title: qsTr("Delete preset?"); closePolicy: Popup.CloseOnEscape
        contentItem: ColumnLayout {
            spacing: 14
            Label { Layout.fillWidth: true; text: qsTr("Delete %1? Cards that already use this layout will stay unchanged.").arg(root.deleteName); textFormat: Text.PlainText; color: Theme.textSoft; wrapMode: Text.WordWrap }
            Label { Layout.fillWidth: true; visible: root.deleteError.length > 0; text: root.deleteError; color: Theme.danger; wrapMode: Text.WordWrap }
            RowLayout {
                Layout.fillWidth: true
                ShapeButton { text: qsTr("Cancel"); quiet: true; onClicked: deleteDialog.close() }
                Item { Layout.fillWidth: true }
                ShapeButton { objectName: "cardPresetDeleteConfirm"; text: qsTr("Delete preset"); onClicked: root.confirmDelete() }
            }
        }
    }
    function matches(preset) {
        const target = settings(preset)
        return Object.keys(target).every(key => root.configuration && JSON.stringify(root.configuration[key]) === JSON.stringify(target[key]))
    }
    Label { text: qsTr("Layout presets"); color: Theme.text; font.weight: Font.DemiBold }
    GridLayout {
        Layout.fillWidth: true
        columns: 3; rowSpacing: 5; columnSpacing: 5
        Repeater {
            model: root.allPresets
            delegate: Button {
                id: tile
                required property var modelData
                objectName: "cardPreset-" + modelData.key
                Layout.fillWidth: true; Layout.preferredWidth: 70
                implicitHeight: 84
                padding: 5
                Accessible.name: modelData.name
                onClicked: root.selectPreset(modelData)
                ToolTip.visible: hovered
                ToolTip.text: modelData.name
                background: Rectangle {
                    radius: 7
                    color: root.matches(tile.modelData) ? Theme.accentSoft : tile.hovered ? Theme.buttonHoverSurface : Theme.controlQuiet
                    border.color: root.matches(tile.modelData) ? Theme.accentBorder : "transparent"
                }
                contentItem: Column {
                    spacing: 4
                    Rectangle {
                        id: mini
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 32; height: 44; radius: 2; color: tile.modelData.background || "#ffffff"; border.color: "#bcbcbc"
                        readonly property int inset: tile.modelData.padding > 0 ? 4 : 1
                        readonly property string position: tile.modelData.position || "top"
                        readonly property bool side: position === "left" || position === "right"
                        readonly property bool full: tile.modelData.mode === "image" || position === "background"
                        readonly property real share: (tile.modelData.share || 50) / 100
                        Rectangle {
                            anchors.fill: parent; anchors.margins: mini.inset
                            color: "transparent"; border.width: tile.modelData.border ? 1 : 0; border.color: "#8b8b8b"
                        }
                        Rectangle {
                            id: miniImage
                            visible: tile.modelData.mode !== "text" && tile.modelData.mode !== "blocks"
                            width: mini.side && !mini.full ? (mini.width - 2 * mini.inset) * mini.share : mini.width - 2 * mini.inset
                            height: mini.full || mini.side ? mini.height - 2 * mini.inset : (mini.height - 2 * mini.inset) * mini.share
                            x: mini.position === "right" ? mini.width - mini.inset - width : mini.inset
                            y: mini.position === "bottom" ? mini.height - mini.inset - height : mini.inset
                            color: tile.modelData.fit === "cover" ? "#999999" : "#dedede"
                            ShapeIcon { anchors.centerIn: parent; source: "../../icons/image.svg"; size: mini.side ? 8 : 13; color: "#666666" }
                        }
                        Rectangle { visible: tile.modelData.mode !== "blocks" && mini.position === "background"; x: mini.inset; y: 29; width: mini.width - mini.inset * 2; height: 11; color: "#ffffff"; opacity: 0.94 }
                        Column {
                            visible: tile.modelData.mode !== "image" && tile.modelData.mode !== "blocks"
                            x: mini.position === "left" ? miniImage.x + miniImage.width + 2 : 6
                            y: tile.modelData.mode === "text" ? (tile.modelData.vertical === "top" ? 7 : 15) : mini.position === "bottom" ? 7 : mini.side ? 17 : 30
                            spacing: 3
                            Repeater {
                                model: tile.modelData.mode === "text" ? 3 : 2
                                Rectangle { required property int index; width: mini.side ? 9 : index === 2 ? 11 : 18; height: index === 0 ? 2 : 1; color: tile.modelData.textColor || "#777777" }
                            }
                        }
                        Repeater {
                            model: tile.modelData.mode === "blocks" ? tile.modelData.blocks || [] : []
                            delegate: Rectangle {
                                required property var modelData
                                x: mini.width * modelData.x / 10000; y: mini.height * modelData.y / 10000
                                width: Math.max(1, mini.width * modelData.width / 10000)
                                height: Math.max(1, mini.height * modelData.height / 10000)
                                color: modelData.field === "artwork" ? "#d4d4d4" : "transparent"
                                border.width: 0.5; border.color: modelData.field === "artwork" ? "#999999" : modelData.text_color
                                Rectangle {
                                    visible: parent.modelData.field !== "artwork"
                                    x: 1; y: Math.min(2, parent.height / 3); width: Math.max(1, parent.width - 2); height: 0.75
                                    color: parent.modelData.text_color
                                }
                            }
                        }
                    }
                    Text { width: parent.width; text: tile.modelData.name; textFormat: Text.PlainText; color: Theme.textSoft; font.pixelSize: 10; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; maximumLineCount: 2; elide: Text.ElideRight }
                }
                ShapeIconButton {
                    objectName: "cardPresetDelete-" + (tile.modelData.id || "")
                    anchors.top: parent.top; anchors.right: parent.right
                    visible: !!tile.modelData.id && root.presetStore !== null
                    buttonSize: 22; iconSize: 12; source: "../../icons/trash.svg"; toolTipText: qsTr("Delete preset")
                    onClicked: root.requestDelete(tile.modelData.id, tile.modelData.name)
                }
            }
        }
    }
}
