pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Shape.Desktop

Item {
    id: root
    objectName: "cardBlockOverlay"
    required property CardBlockEditor editor
    readonly property var selected: editor.selectedBlock
    Repeater {
        model: root.editor.blocks.length
        delegate: Rectangle {
            id: block
            required property int index
            readonly property var modelData: root.editor.blocks[index]
            objectName: "cardBlockRect-" + modelData.id
            x: root.width * modelData.x / 10000; y: root.height * modelData.y / 10000
            width: root.width * modelData.width / 10000; height: root.height * modelData.height / 10000
            z: index; color: "transparent"
            border.width: root.editor.selectedBlockId === modelData.id ? 0 : 1; border.color: Theme.borderStrong
            opacity: root.editor.selectedBlockId === modelData.id ? 1 : 0.55
            MouseArea {
                anchors.fill: parent; preventStealing: true; cursorShape: Qt.SizeAllCursor
                property real startX: 0
                property real startY: 0
                onPressed: mouse => {
                    const point = mapToItem(root, mouse.x, mouse.y); startX = point.x; startY = point.y
                    mouse.accepted = root.editor.beginGesture(block.modelData.id, "move")
                }
                onPositionChanged: mouse => {
                    if (!pressed || !root.editor.gestureActive || root.width <= 0 || root.height <= 0) return
                    const point = mapToItem(root, mouse.x, mouse.y)
                    root.editor.updateGesture((point.x - startX) * 10000 / root.width, (point.y - startY) * 10000 / root.height)
                }
                onReleased: root.editor.finishGesture()
                onCanceled: root.editor.cancelGesture()
            }
        }
    }
    Rectangle {
        objectName: "cardBlockSelectionOutline"
        visible: root.selected !== null
        x: root.selected ? root.width * root.selected.x / 10000 : 0
        y: root.selected ? root.height * root.selected.y / 10000 : 0
        width: root.selected ? root.width * root.selected.width / 10000 : 0
        height: root.selected ? root.height * root.selected.height / 10000 : 0
        color: "transparent"; border.width: 3; border.color: "#111111"; z: 100
        Rectangle {
            anchors.fill: parent
            color: "transparent"; border.width: 1; border.color: "#ffffff"
        }
    }
    Repeater {
        model: ["nw","ne","sw","se"]
        delegate: Item {
            id: handle
            required property string modelData
            objectName: "cardBlockHandle-" + modelData
            visible: root.selected !== null; width: 18; height: 18; z: 101
            x: root.selected ? root.width * (root.selected.x + (modelData.indexOf("e") >= 0 ? root.selected.width : 0)) / 10000 - 9 : 0
            y: root.selected ? root.height * (root.selected.y + (modelData.indexOf("s") >= 0 ? root.selected.height : 0)) / 10000 - 9 : 0
            Rectangle {
                anchors.centerIn: parent; width: 10; height: 10; radius: 2
                color: "#111111"; border.width: 1; border.color: "#ffffff"
                Rectangle { anchors.fill: parent; anchors.margins: 2; color: "#ffffff" }
            }
            MouseArea {
                anchors.fill: parent; preventStealing: true
                cursorShape: handle.modelData === "nw" || handle.modelData === "se" ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor
                property real startX: 0
                property real startY: 0
                onPressed: mouse => {
                    const point = mapToItem(root, mouse.x, mouse.y); startX = point.x; startY = point.y
                    mouse.accepted = root.selected !== null && root.editor.beginGesture(root.selected.id, handle.modelData)
                }
                onPositionChanged: mouse => {
                    if (!pressed || !root.editor.gestureActive || root.width <= 0 || root.height <= 0) return
                    const point = mapToItem(root, mouse.x, mouse.y)
                    root.editor.updateGesture((point.x - startX) * 10000 / root.width, (point.y - startY) * 10000 / root.height)
                }
                onReleased: root.editor.finishGesture()
                onCanceled: root.editor.cancelGesture()
            }
        }
    }
}
