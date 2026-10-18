pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Shape.Desktop

// A bounded editor owns scrolling; surrounding forms only scroll between fields.
ScrollView {
    id: control
    property alias text: editor.text
    property alias placeholderText: editor.placeholderText
    property alias readOnly: editor.readOnly
    property alias wrapMode: editor.wrapMode
    property alias selectByMouse: editor.selectByMouse
    property alias color: editor.color
    property alias placeholderTextColor: editor.placeholderTextColor
    property alias selectionColor: editor.selectionColor
    property alias selectedTextColor: editor.selectedTextColor
    property alias cursorPosition: editor.cursorPosition
    property alias textFormat: editor.textFormat
    readonly property bool editorActiveFocus: editor.activeFocus
    readonly property real documentHeight: editor.implicitHeight
    readonly property bool canScroll: contentHeight > availableHeight
    function selectAll() : void { editor.selectAll() }
    function focusEditor() : void { editor.forceActiveFocus() }

    font.pixelSize: 14
    implicitWidth: 240
    implicitHeight: 160
    contentWidth: availableWidth
    clip: true
    focusPolicy: Qt.StrongFocus
    onActiveFocusChanged: if (activeFocus && !editor.activeFocus) editor.forceActiveFocus()
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ScrollBar.vertical.policy: canScroll ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
    ScrollBar.vertical.interactive: true
    background: Rectangle {
        radius: Theme.controlRadius
        color: control.enabled ? Theme.control : Theme.controlQuiet
        border.color: control.editorActiveFocus ? Theme.focusRing : Theme.border
    }
    ShapeTextArea {
        id: editor
        width: control.availableWidth
        rightPadding: 24
        font: control.font
        background: null
        Accessible.name: control.Accessible.name
    }
}
