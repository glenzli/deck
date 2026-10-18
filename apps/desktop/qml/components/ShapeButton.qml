import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

Button {
    id: control

    property bool primary: highlighted
    property bool selected: false
    property bool quiet: false
    property bool busy: false
    property url iconSource
    property int iconSize: 15
    property bool expanded: false

    // Action buttons use their content width; grouped navigation opts into equal widths.
    Layout.fillWidth: expanded
    Layout.maximumWidth: expanded ? 65535 : implicitWidth
    Layout.alignment: Qt.AlignLeft

    implicitHeight: Theme.controlHeight
    implicitWidth: Math.max(text.length > 0 ? 70 : implicitHeight,
                            buttonContent.implicitWidth + 24)
    leftPadding: 12
    rightPadding: 12
    focusPolicy: Qt.StrongFocus
    hoverEnabled: enabled

    background: Rectangle {
        radius: Theme.controlRadius
        border.width: control.primary || control.quiet ? 0 : 1
        border.color: control.selected ? Theme.accentBorder : Theme.buttonBorder
        color: {
            if (!control.enabled) {
                return control.quiet ? Theme.transparent : Theme.controlQuiet
            }
            if (control.primary) {
                return control.down ? Theme.accentPressed
                                    : control.hovered ? Theme.accentHover : Theme.accent
            }
            if (control.selected) {
                return Theme.accentSoft
            }
            if (control.quiet) {
                return control.down ? Theme.buttonGhostPressed
                                    : control.hovered ? Theme.buttonGhostHover
                                                      : Theme.transparent
            }
            return control.down ? Theme.buttonPressedSurface
                                : control.hovered ? Theme.buttonHoverSurface
                                                  : Theme.buttonSurface
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: -2
            radius: parent.radius + 2
            color: "transparent"
            border.width: control.visualFocus ? 1 : 0
            border.color: Theme.focusRing
        }
    }

    contentItem: Item {
        implicitWidth: buttonContent.implicitWidth
        implicitHeight: buttonContent.implicitHeight

        Row {
            id: buttonContent
            opacity: control.busy ? 0 : 1
            anchors.centerIn: parent
            spacing: control.iconSource.toString().length > 0 && control.text.length > 0 ? 7 : 0

            ShapeIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: control.iconSource.toString().length > 0
                source: control.iconSource
                size: control.iconSize
                color: !control.enabled ? Theme.disabled
                                        : control.primary ? Theme.accentText
                                                          : control.selected ? Theme.accent
                                                                             : Theme.textSoft
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: control.text.length > 0
                text: control.text
                color: !control.enabled ? Theme.disabled
                                        : control.primary ? Theme.accentText
                                                          : control.selected ? Theme.accent
                                                                             : Theme.textSoft
                font.pixelSize: Theme.fontBody
                font.weight: control.primary || control.selected ? Font.DemiBold : Font.Normal
                verticalAlignment: Text.AlignVCenter
            }
        }
        BusyIndicator {
            anchors.centerIn: parent
            width: 20
            height: 20
            visible: control.busy
            running: control.busy
        }
    }
}
