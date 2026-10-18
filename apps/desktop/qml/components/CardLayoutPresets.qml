pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

ColumnLayout {
    id: root
    property var configuration: ({})
    signal chosen(var configuration)
    readonly property var presets: [
        {key: "clean-text", name: qsTr("Focus"), mode: "text", align: "center", scale: 120, padding: 8, border: false, fit: "contain"},
        {key: "study", name: qsTr("Study notes"), mode: "text", align: "left", scale: 100, padding: 6, border: false, fit: "contain"},
        {key: "quote", name: qsTr("Quote"), mode: "text", align: "center", scale: 100, padding: 12, border: true, fit: "contain"},
        {key: "picture", name: qsTr("Full picture"), mode: "image", align: "left", scale: 100, padding: 0, border: false, fit: "contain"},
        {key: "photo", name: qsTr("Photo crop"), mode: "image", align: "left", scale: 100, padding: 0, border: false, fit: "cover"},
        {key: "caption", name: qsTr("Picture & note"), mode: "image_text", align: "left", scale: 100, padding: 4, border: false, fit: "contain"},
        {key: "framed", name: qsTr("Framed picture"), mode: "image_text", align: "center", scale: 100, padding: 8, border: true, fit: "contain"},
        {key: "game", name: qsTr("Game card"), mode: "classic", align: "left", scale: 100, padding: 4, border: true, fit: "contain"}
    ]
    spacing: 7
    function settings(preset) {
        return {mode: preset.mode, image_fit: preset.fit, text_align: preset.align,
            text_scale_percent: preset.scale, padding_percent: preset.padding, border: preset.border,
            background_color: "#ffffff", text_color: "#202020"}
    }
    function matches(preset) {
        const target = settings(preset)
        return Object.keys(target).every(key => root.configuration && root.configuration[key] === target[key])
    }
    Label { text: qsTr("Layout presets"); color: Theme.text; font.weight: Font.DemiBold }
    GridLayout {
        Layout.fillWidth: true
        columns: 3; rowSpacing: 5; columnSpacing: 5
        Repeater {
            model: root.presets
            delegate: Button {
                id: tile
                required property var modelData
                objectName: "cardPreset-" + modelData.key
                Layout.fillWidth: true; Layout.preferredWidth: 70
                implicitHeight: 91
                padding: 5
                Accessible.name: modelData.name
                onClicked: root.chosen(root.settings(modelData))
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
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 32; height: 44; radius: 2; color: "#ffffff"; border.color: "#bcbcbc"
                        Rectangle {
                            anchors.fill: parent; anchors.margins: tile.modelData.padding > 0 ? 4 : 0
                            color: "transparent"; border.width: tile.modelData.border ? 1 : 0; border.color: "#8b8b8b"
                        }
                        Rectangle {
                            visible: tile.modelData.mode !== "text"
                            x: tile.modelData.padding > 0 ? 5 : 1; y: x
                            width: parent.width - 2 * x
                            height: tile.modelData.mode === "image" ? parent.height - 2 * y : 19
                            color: tile.modelData.fit === "cover" ? "#999999" : "#dedede"
                            ShapeIcon { anchors.centerIn: parent; source: "../../icons/image.svg"; size: 13; color: "#666666" }
                        }
                        Column {
                            visible: tile.modelData.mode !== "image"
                            x: 7; y: tile.modelData.mode === "text" ? 15 : 29
                            spacing: 3
                            Repeater {
                                model: tile.modelData.mode === "text" ? 3 : 2
                                Rectangle { required property int index; width: index === 2 ? 11 : 18; height: index === 0 ? 2 : 1; color: "#777777" }
                            }
                        }
                    }
                    Text { width: parent.width; text: tile.modelData.name; color: Theme.textSoft; font.pixelSize: 10; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap; maximumLineCount: 2; elide: Text.ElideRight }
                }
            }
        }
    }
}
