import QtQuick
import QtQuick.Controls
import Shape.Desktop

TextArea {
    id: control
    font.pixelSize: 14
    color: enabled ? Theme.text : Theme.disabled
    placeholderTextColor: Theme.textPlaceholder
    selectionColor: Theme.accentSoft
    selectedTextColor: Theme.text
    selectByMouse: true
    wrapMode: TextEdit.Wrap
    padding: 14
    background: Rectangle {
        radius: Theme.controlRadius
        color: control.enabled ? Theme.control : Theme.controlQuiet
        border.color: control.activeFocus ? Theme.focusRing : Theme.border
    }
}
