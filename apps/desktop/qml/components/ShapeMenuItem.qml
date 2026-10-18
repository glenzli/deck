pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

MenuItem {
    id: control
    property bool destructive: false
    implicitHeight: 32
    implicitWidth: 224
    leftPadding: 9; rightPadding: 9; topPadding: 0; bottomPadding: 0
    font.pixelSize: Theme.fontBody
    icon.width: 18; icon.height: 18
    indicator: null
    arrow: null
    contentItem: RowLayout {
        spacing: 9
        Item {
            implicitWidth: 18; implicitHeight: 18
            ShapeIcon { anchors.centerIn: parent; source: control.icon.source; size: 18; visible: control.icon.source.toString().length > 0; color: !control.enabled ? Theme.disabled : control.destructive ? Theme.danger : Theme.textSoft }
            Text { anchors.centerIn: parent; text: "✓"; visible: control.checkable && control.checked && control.icon.source.toString().length === 0; font.pixelSize: 13; color: Theme.text }
        }
        Text {
            Layout.fillWidth: true
            text: control.text; font: control.font
            color: !control.enabled ? Theme.disabled : control.destructive ? Theme.danger : Theme.text
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        Text { visible: control.checkable && control.checked && control.icon.source.toString().length > 0; text: "✓"; color: Theme.text; font.pixelSize: 12 }
        ShapeIcon { visible: !!control.subMenu; source: "../../icons/chevron-down.svg"; size: 12; rotation: -90; color: Theme.muted }
    }
    background: Rectangle {
        radius: 6
        color: control.highlighted ? (control.destructive ? Theme.dangerSoft : Theme.buttonHoverSurface) : "transparent"
        border.width: control.visualFocus ? 1 : 0
        border.color: Theme.focusRing
    }
}
