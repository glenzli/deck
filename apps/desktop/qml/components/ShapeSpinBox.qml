import QtQuick
import QtQuick.Controls
import Shape.Desktop

SpinBox {
    id: control
    implicitWidth: 108
    implicitHeight: Theme.compactControlHeight
    font.pixelSize: Theme.fontBody
    editable: true
    wheelEnabled: false
    leftPadding: 27
    rightPadding: 27
    topPadding: 0
    bottomPadding: 0
    contentItem: TextInput {
        text: control.displayText
        font: control.font
        color: control.enabled ? Theme.text : Theme.disabled
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        readOnly: !control.editable
        validator: control.validator
        inputMethodHints: control.inputMethodHints
        selectByMouse: true
        selectionColor: Theme.accentSoft
        selectedTextColor: Theme.text
    }
    down.indicator: Rectangle {
        x: 1; y: 1; width: 25; height: control.height - 2
        radius: Theme.compactControlRadius
        color: control.down.pressed ? Theme.buttonPressedSurface : control.down.hovered ? Theme.buttonHoverSurface : "transparent"
        Text { anchors.centerIn: parent; text: "−"; font.pixelSize: 15; color: control.down.enabled ? Theme.textSoft : Theme.disabled }
    }
    up.indicator: Rectangle {
        x: control.width - width - 1; y: 1; width: 25; height: control.height - 2
        radius: Theme.compactControlRadius
        color: control.up.pressed ? Theme.buttonPressedSurface : control.up.hovered ? Theme.buttonHoverSurface : "transparent"
        Text { anchors.centerIn: parent; text: "+"; font.pixelSize: 15; color: control.up.enabled ? Theme.textSoft : Theme.disabled }
    }
    background: Rectangle {
        radius: Theme.compactControlRadius
        color: control.enabled ? Theme.control : Theme.controlQuiet
        border.color: control.activeFocus ? Theme.focusRing : Theme.border
    }
}
