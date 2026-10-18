pragma ComponentBehavior: Bound

import QtQuick
import Shape.Desktop

Item {
    id: root

    property url source
    property color color: Theme.textSoft
    property int size: Theme.iconSize

    implicitWidth: size
    implicitHeight: size

    // Treat the SVG alpha as a mask so black source strokes receive the exact
    // theme color on both the GPU and software backends.
    Canvas {
        id: tintedIcon
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
            function onSourceChanged(): void { tintedIcon.reloadImage() }
            function onSizeChanged(): void { tintedIcon.reloadImage() }
            function onColorChanged(): void { tintedIcon.requestPaint() }
        }
    }

    Behavior on color {
        ColorAnimation { duration: 80 }
    }
}
