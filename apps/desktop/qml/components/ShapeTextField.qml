import QtQuick
import QtQuick.Controls
import Shape.Desktop

TextField {
    id: control
    implicitHeight: Theme.controlHeight
    font.pixelSize: Theme.fontBody
    color: enabled ? Theme.text : Theme.disabled
    placeholderTextColor: Theme.textPlaceholder
    selectionColor: Theme.accentSoft
    selectedTextColor: Theme.text
    selectByMouse: true
    leftPadding: 11
    rightPadding: 11
    topPadding: 7
    bottomPadding: 7
    background: Rectangle {
        radius: Theme.controlRadius
        color: control.enabled ? Theme.control : Theme.controlQuiet
        border.color: control.activeFocus ? Theme.focusRing : Theme.border
    }
}
