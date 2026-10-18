//! Side-by-side accepted and transient raster comparison.

import QtQuick
import QtQuick.Layouts
import Shape.Desktop

Item {
    id: comparison
    objectName: "imageCompareWorkspace"

    property string acceptedSource: ""
    property string candidateSource: ""

    RowLayout {
        anchors.fill: parent
        spacing: 12

        Repeater {
            model: [
                { label: qsTr("ACCEPTED"), source: comparison.acceptedSource, candidate: false },
                { label: qsTr("CANDIDATE"), source: comparison.candidateSource, candidate: true }
            ]

            delegate: Rectangle {
                id: compareCard

                required property var modelData

                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Theme.radiusMedium
                color: compareCard.modelData.candidate ? Theme.accentSoft : Theme.raised
                border.color: compareCard.modelData.candidate ? Theme.accent : Theme.border

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 10

                    Text {
                        text: compareCard.modelData.label
                        color: compareCard.modelData.candidate ? Theme.accent : Theme.muted
                        font.pixelSize: Theme.fontMeta
                        font.weight: Font.DemiBold
                        font.letterSpacing: 0.7
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: Theme.radiusSmall
                        color: Theme.effectiveDark ? "#151719" : "#e7e9eb"
                        clip: true

                        Image {
                            anchors.fill: parent
                            anchors.margins: 8
                            source: compareCard.modelData.source
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            cache: true
                        }
                    }
                }
            }
        }
    }
}
