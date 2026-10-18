pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import Shape.Desktop

Menu {
    id: control
    popupType: Popup.Item
    implicitWidth: 236
    implicitHeight: Math.min(500, contentItem.implicitHeight + topPadding + bottomPadding)
    padding: 6
    spacing: 2
    margins: 8
    delegate: ShapeMenuItem {}
    background: Rectangle {
        radius: 10
        color: Theme.panelRaised
        border.color: Theme.border
    }
    contentItem: ListView {
        implicitHeight: contentHeight
        model: control.contentModel
        currentIndex: control.currentIndex
        spacing: 2
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollIndicator.vertical: ScrollIndicator {}
    }
}
