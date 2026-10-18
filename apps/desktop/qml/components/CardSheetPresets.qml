pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

GridLayout {
    id: root
    property int selectedColumns: 0
    property int selectedRows: 0
    signal chosen(int columns, int rows)
    columns: 3; columnSpacing: 6; rowSpacing: 6
    Repeater {
        model: [{c: 1, r: 1}, {c: 2, r: 2}, {c: 2, r: 3}, {c: 3, r: 3}, {c: 3, r: 4}, {c: 4, r: 4}]
        delegate: Button {
            id: tile
            required property var modelData
            readonly property bool selected: root.selectedColumns === modelData.c && root.selectedRows === modelData.r
            objectName: "cardSheetPreset-" + modelData.c + "x" + modelData.r
            Layout.fillWidth: true; Layout.preferredWidth: 70
            implicitHeight: 60; padding: 5
            Accessible.name: qsTr("%1 columns × %2 rows").arg(modelData.c).arg(modelData.r)
            onClicked: root.chosen(modelData.c, modelData.r)
            background: Rectangle { radius: 7; color: tile.selected ? Theme.accentSoft : tile.hovered ? Theme.buttonHoverSurface : Theme.control; border.color: tile.selected ? Theme.accentBorder : Theme.border }
            contentItem: Column {
                spacing: 3
                Item {
                    width: 23; height: 29; anchors.horizontalCenter: parent.horizontalCenter
                    Grid {
                        anchors.fill: parent; columns: tile.modelData.c; spacing: 1
                        Repeater {
                            model: tile.modelData.c * tile.modelData.r
                            Rectangle { width: (23 - tile.modelData.c + 1) / tile.modelData.c; height: (29 - tile.modelData.r + 1) / tile.modelData.r; radius: 1; color: Theme.muted; opacity: 0.6 }
                        }
                    }
                }
                Text { width: parent.width; text: tile.modelData.c + " × " + tile.modelData.r; horizontalAlignment: Text.AlignHCenter; color: Theme.text; font.pixelSize: 11 }
            }
        }
    }
}
