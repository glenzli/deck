pragma ComponentBehavior: Bound

import QtQuick
import Shape.Desktop

Item {
    id: root

    property url source
    property color color: Theme.textSoft
    property int size: Theme.iconSize
    readonly property real pixelRatio: Screen.devicePixelRatio

    implicitWidth: size
    implicitHeight: size

    // Rasterize and tint in one pass at physical size. A Canvas alpha-mask pass
    // can resample the SVG at logical resolution on high-DPI displays.
    Image {
        anchors.centerIn: parent
        width: Math.min(root.width, root.size)
        height: Math.min(root.height, root.size)
        source: root.source.toString().length === 0 ? ""
            : "image://deck-icons/icon?source=" + encodeURIComponent(Qt.resolvedUrl(root.source))
                + "&color=" + encodeURIComponent(root.color.toString())
        sourceSize: Qt.size(Math.ceil(width * root.pixelRatio), Math.ceil(height * root.pixelRatio))
        fillMode: Image.PreserveAspectFit
    }

    Behavior on color {
        ColorAnimation { duration: 80 }
    }
}
