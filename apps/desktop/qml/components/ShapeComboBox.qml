pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Shape.Desktop

ComboBox {
    id: control
    implicitHeight: Theme.controlHeight
    implicitWidth: 160
    font.pixelSize: Theme.fontBody
    leftPadding: 11
    rightPadding: 30
    hoverEnabled: true
    contentItem: Text {
        text: control.displayText
        font: control.font
        color: control.enabled ? Theme.text : Theme.disabled
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    indicator: Text {
        x: control.width - width - 11
        y: (control.height - height) / 2 - 2
        text: "⌄"
        color: Theme.muted
        font.pixelSize: 16
    }
    background: Rectangle {
        radius: Theme.controlRadius
        color: control.down ? Theme.controlPressed : control.hovered ? Theme.raisedHover : Theme.control
        border.color: control.activeFocus ? Theme.focusRing : Theme.border
    }
    delegate: ItemDelegate {
        required property int index
        width: control.width - 8
        implicitHeight: Theme.controlHeight
        highlighted: control.highlightedIndex === index
        contentItem: Text {
            text: control.textAt(parent.index)
            color: parent.highlighted ? Theme.accentSelectionText : Theme.text
            font.pixelSize: Theme.fontBody
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
        background: Rectangle {
            radius: Theme.compactControlRadius
            color: parent.highlighted ? Theme.accentSoft : "transparent"
        }
    }
    popup: Popup {
        y: control.height + 4
        width: control.width
        padding: 4
        implicitHeight: Math.min(300, contentItem.implicitHeight + 8)
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }
        background: Rectangle {
            radius: Theme.controlRadius
            color: Theme.panelRaised
            border.color: Theme.borderStrong
        }
    }
}
