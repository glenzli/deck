import QtQuick
import QtQuick.Controls
import Shape.Desktop

CheckBox {
    id: control
    implicitHeight: Theme.compactControlHeight
    spacing: 8
    padding: 0
    font.pixelSize: Theme.fontBody
    indicator: Rectangle {
        implicitWidth: 16; implicitHeight: 16
        x: control.leftPadding; y: (control.height - height) / 2
        radius: 4
        color: control.checked ? Theme.accent : Theme.control
        border.color: control.activeFocus ? Theme.focusRing : control.checked ? Theme.accent : Theme.borderStrong
        Text { anchors.centerIn: parent; text: "✓"; font.pixelSize: 12; font.weight: Font.DemiBold; color: Theme.accentText; visible: control.checked }
    }
    contentItem: Text {
        text: control.text; font: control.font
        color: control.enabled ? Theme.textSoft : Theme.disabled
        leftPadding: control.indicator.width + control.spacing
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
    }
}
