pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import Shape.Desktop

Item {
    id: root

    property url source
    property color color: Theme.textSoft
    property int size: 18
    readonly property bool softwareRendering: GraphicsInfo.api === GraphicsInfo.Software

    implicitWidth: size
    implicitHeight: size

    Image {
        id: image

        anchors.centerIn: parent
        width: Math.min(root.width, root.size)
        height: Math.min(root.height, root.size)
        visible: !root.softwareRendering
        source: root.source
        sourceSize: Qt.size(
            Math.ceil(root.size * Screen.devicePixelRatio),
            Math.ceil(root.size * Screen.devicePixelRatio)
        )
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true

        layer.enabled: !root.softwareRendering && status === Image.Ready
        layer.smooth: true
        layer.effect: MultiEffect {
            colorization: 1.0
            colorizationColor: root.color
        }
    }

    // Shader effects have no software implementation. Tint only these tiny icons
    // in a CPU image so headless/native fallback actions remain visible in both themes.
    Loader {
        anchors.fill: parent
        active: root.softwareRendering
        sourceComponent: Canvas {
            id: softwareIcon
            anchors.centerIn: parent
            width: Math.min(root.width, root.size)
            height: Math.min(root.height, root.size)
            renderTarget: Canvas.Image
            property url loadedSource: ""

            function reloadImage(): void {
                if (!available) return
                if (loadedSource.toString().length > 0) unloadImage(loadedSource)
                loadedSource = root.source
                if (loadedSource.toString().length > 0)
                    loadImage(loadedSource, Qt.size(Math.ceil(root.size * Screen.devicePixelRatio),
                                                   Math.ceil(root.size * Screen.devicePixelRatio)))
                requestPaint()
            }
            onAvailableChanged: if (available) reloadImage()
            onImageLoaded: requestPaint()
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                const painter = getContext("2d")
                if (!painter) return
                painter.reset()
                painter.clearRect(0, 0, width, height)
                if (!isImageLoaded(loadedSource)) return
                painter.drawImage(loadedSource, 0, 0, width, height)
                painter.globalCompositeOperation = "source-in"
                painter.fillStyle = root.color
                painter.fillRect(0, 0, width, height)
            }
            Connections {
                target: root
                function onSourceChanged(): void { softwareIcon.reloadImage() }
                function onSizeChanged(): void { softwareIcon.reloadImage() }
                function onColorChanged(): void { softwareIcon.requestPaint() }
            }
        }
    }

    Behavior on color {
        ColorAnimation { duration: 80 }
    }
}
