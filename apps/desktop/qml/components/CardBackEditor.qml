pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop

ColumnLayout {
    id: root
    objectName: "cardKitBackEditor"
    required property DesktopBackend backend
    property var configuration: null
    property string editIdentity: ""
    property bool editingEnabled: true
    property int canvasWidth: 750
    property int canvasHeight: 1050
    property string previewSvg: ""
    property string localError: ""
    property string colorIdentity: ""
    property string colorConfiguration: ""
    readonly property string mode: configuration ? configuration.mode : ""
    readonly property var images: [{id: "", name: qsTr("Choose back image…")}].concat(
        backend.artifacts.filter(item => item.kindKey === "image_raster" && item.hasAcceptedRevision))
    readonly property string colorValue: mode === "solid" ? configuration.color : configuration ? configuration.background_color || "#ffffff" : "#ffffff"
    signal edited(var configuration)
    signal importRequested()
    spacing: 8
    function chooseMode(index) {
        if (!editingEnabled) return false
        localError = ""
        if (index === 0) edited(null)
        else if (index === 1) edited({mode: "solid", color: colorValue})
        else edited({mode: "image", artwork: mode === "image" ? configuration.artwork || null : null,
            image_fit: mode === "image" ? configuration.image_fit || "contain" : "contain", background_color: colorValue})
        return true
    }
    function setValue(key, value) {
        if (!editingEnabled || !configuration) return false
        const next = JSON.parse(JSON.stringify(configuration)); next[key] = value
        localError = ""; edited(next); return true
    }
    function chooseImage(index) {
        if (index < 1 || index >= images.length || mode !== "image") return false
        const image = images[index]
        return setValue("artwork", {artifact_id: image.id, revision_id: image.acceptedRevisionId})
    }
    function chooseColor() {
        if (!editingEnabled || !configuration) return
        colorIdentity = editIdentity; colorConfiguration = JSON.stringify(configuration)
        colors.selectedColor = colorValue; colors.open()
    }
    onEditIdentityChanged: colors.close()
    onEditingEnabledChanged: if (!editingEnabled) colors.close()
    ColorDialog {
        id: colors
        title: qsTr("Choose card back color")
        onAccepted: if (root.editingEnabled && root.colorIdentity === root.editIdentity && root.colorConfiguration === JSON.stringify(root.configuration))
            root.setValue(root.mode === "solid" ? "color" : "background_color", "#" + String(selectedColor).slice(-6))
    }
    Label { text: qsTr("Card back"); color: Theme.text; font.weight: Font.DemiBold }
    Label { Layout.fillWidth: true; text: qsTr("One back design is shared by every card in this set."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
    ShapeComboBox {
        objectName: "cardBackMode"; Layout.fillWidth: true; enabled: root.editingEnabled
        model: [qsTr("No card back"), qsTr("Solid color"), qsTr("Image")]
        selectedIndex: root.mode === "solid" ? 1 : root.mode === "image" ? 2 : 0
        onActivated: index => root.chooseMode(index)
    }
    ColumnLayout {
        visible: root.mode.length > 0; Layout.fillWidth: true; spacing: 8
        enabled: root.editingEnabled
        RowLayout {
            Layout.fillWidth: true; spacing: 8
            ToolButton {
                objectName: "cardBackColorSwatch"; implicitWidth: 34; implicitHeight: 34
                Accessible.name: qsTr("Choose card back color"); onClicked: root.chooseColor()
                background: Rectangle { radius: 7; color: root.colorValue; border.color: Theme.borderStrong }
            }
            ShapeTextField {
                objectName: "cardBackColor"; Layout.fillWidth: true
                text: root.colorValue; maximumLength: 7; placeholderText: "#ffffff"
                validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
                onEditingFinished: {
                    if (acceptableInput) root.setValue(root.mode === "solid" ? "color" : "background_color", text)
                    else root.localError = qsTr("Use a color such as #ffffff.")
                }
            }
        }
        ShapeComboBox {
            objectName: "cardBackImage"; visible: root.mode === "image"; Layout.fillWidth: true
            model: root.images.map(item => item.name)
            selectedIndex: Math.max(0, root.images.findIndex(item => root.configuration && root.configuration.artwork && item.id === root.configuration.artwork.artifact_id))
            onActivated: index => root.chooseImage(index)
        }
        ShapeButton { objectName: "cardBackImport"; visible: root.mode === "image"; text: qsTr("Import back image…"); quiet: true; onClicked: root.importRequested() }
        ShapeComboBox {
            objectName: "cardBackImageFit"; visible: root.mode === "image"; Layout.fillWidth: true
            model: [qsTr("Show the complete image"), qsTr("Fill the card · crop edges")]
            selectedIndex: root.configuration && root.configuration.image_fit === "cover" ? 1 : 0
            onActivated: index => root.setValue("image_fit", index === 1 ? "cover" : "contain")
        }
        Label { text: qsTr("Card back preview"); color: Theme.textSoft; font.pixelSize: 11 }
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Math.min(176, 200 * root.canvasWidth / Math.max(1, root.canvasHeight))
            Layout.preferredHeight: Math.min(200, 176 * root.canvasHeight / Math.max(1, root.canvasWidth))
            color: root.colorValue; border.color: Theme.border
            Image {
                objectName: "cardBackPreview"; anchors.fill: parent
                source: root.previewSvg.length ? "data:image/svg+xml;charset=utf-8," + encodeURIComponent(root.previewSvg) : ""
                fillMode: Image.PreserveAspectFit
            }
        }
        Label { Layout.fillWidth: true; visible: root.mode === "image" && !root.configuration.artwork; text: qsTr("Choose an accepted image before exporting card backs."); color: Theme.warning; wrapMode: Text.WordWrap; font.pixelSize: 11 }
    }
    Label { Layout.fillWidth: true; visible: root.localError.length > 0; text: root.localError; color: Theme.danger; wrapMode: Text.WordWrap }
}
