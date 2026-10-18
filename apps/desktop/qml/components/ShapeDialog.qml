import QtQuick
import QtQuick.Controls
import Shape.Desktop

Dialog {
    id: control
    padding: 20
    spacing: 18
    font.pixelSize: Theme.fontBody
    palette.window: Theme.panelRaised
    palette.windowText: Theme.text
    palette.text: Theme.text
    palette.button: Theme.buttonSurface
    palette.buttonText: Theme.text
    palette.base: Theme.control
    palette.highlight: Theme.accent
    palette.highlightedText: Theme.accentText
    background: Rectangle {
        radius: Theme.panelRadius
        color: Theme.panelRaised
        border.color: Theme.border
    }
    header: Label {
        visible: control.title.length > 0
        text: control.title
        color: Theme.text
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        leftPadding: 20
        rightPadding: 20
        topPadding: 20
        bottomPadding: 4
        elide: Text.ElideRight
    }
    Overlay.modal: Rectangle { color: Theme.shadow }
}
