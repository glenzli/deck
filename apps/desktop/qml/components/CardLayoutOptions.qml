pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

ColumnLayout {
    id: root
    property var configuration: null
    property var resolved: ({})
    property bool perCard: false
    property bool expanded: false
    property string editIdentity: ""
    property bool editingEnabled: true
    property string colorIdentity: ""
    property string colorConfiguration: ""
    property string colorResolved: ""
    property string validationMessage: ""
    property string colorTarget: "background_color"
    readonly property var modes: ["auto", "image", "text", "image_text", "classic"]
    readonly property var choices: perCard || !configuration ? [""].concat(modes) : modes
    signal edited(var value)
    spacing: 8

    function value(key, fallback) {
        if (configuration && configuration[key] !== undefined) return configuration[key]
        return resolved && resolved[key] !== undefined ? resolved[key] : fallback
    }
    function setValue(key, value) {
        const defaults = {image_fit: "contain", background_color: "#ffffff", text_color: "#202020",
            border: false, padding_percent: 4, text_align: "left", text_scale_percent: 100}
        if (value === root.value(key, defaults[key])) return
        const next = configuration ? JSON.parse(JSON.stringify(configuration)) : ({})
        if (!perCard && !next.mode) next.mode = "auto"
        next[key] = value; validationMessage = ""; edited(next)
    }
    function modeIndex() {
        return Math.max(0, choices.indexOf(configuration && configuration.mode ? configuration.mode : ""))
    }
    function chooseMode(index) {
        const mode = choices[index]
        if (!mode && !perCard) return
        const next = configuration ? JSON.parse(JSON.stringify(configuration)) : ({})
        if (!mode) delete next.mode
        else next.mode = mode
        if (JSON.stringify(next) === JSON.stringify(configuration || {})) return
        edited(Object.keys(next).length ? next : null)
    }
    function chooseColor(key) {
        if (!editingEnabled) return
        colorTarget = key
        colorIdentity = editIdentity
        colorConfiguration = JSON.stringify(configuration || {})
        colorResolved = JSON.stringify(resolved || {})
        colorDialog.selectedColor = value(key, key === "text_color" ? "#202020" : "#ffffff")
        colorDialog.open()
    }
    function dismissColor() { colorIdentity = ""; colorDialog.close() }
    onEditIdentityChanged: dismissColor()
    onEditingEnabledChanged: if (!editingEnabled) dismissColor()
    ColorDialog {
        id: colorDialog
        objectName: "cardLayoutColorDialog"
        title: root.colorTarget === "text_color" ? qsTr("Choose text color") : qsTr("Choose background color")
        onAccepted: {
            if (root.editingEnabled && root.colorIdentity.length && root.colorIdentity === root.editIdentity
                && root.colorConfiguration === JSON.stringify(root.configuration || {})
                && root.colorResolved === JSON.stringify(root.resolved || {}))
                root.setValue(root.colorTarget, "#" + String(selectedColor).slice(-6))
            root.colorIdentity = ""
        }
        onRejected: root.colorIdentity = ""
    }
    Label { text: root.perCard ? qsTr("This card’s layout") : qsTr("Default layout"); color: Theme.text; font.weight: Font.DemiBold }
    ShapeComboBox {
        objectName: root.perCard ? "cardLayoutOverride" : "cardLayoutDefault"
        Layout.fillWidth: true
        model: {
            const names = [qsTr("Automatic"), qsTr("Complete image"), qsTr("Text only"), qsTr("Image and text"), qsTr("Game card")]
            return root.perCard || !root.configuration
                ? [root.perCard ? qsTr("Use the set’s layout type") : qsTr("Original game-card layout")].concat(names) : names
        }
        currentIndex: root.modeIndex()
        onActivated: index => root.chooseMode(index)
    }
    Label {
        Layout.fillWidth: true
        visible: !root.perCard && !root.configuration
        text: qsTr("These saved cards keep their original appearance. Choose another layout to use the flexible card settings.")
        color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 12
    }
    Label { text: qsTr("Image placement"); color: Theme.muted }
    ShapeComboBox {
        objectName: root.perCard ? "cardImageFitOverride" : "cardImageFitDefault"
        Layout.fillWidth: true
        model: [qsTr("Show the complete image"), qsTr("Fill the card · crop edges")]
        currentIndex: root.value("image_fit", "contain") === "cover" ? 1 : 0
        onActivated: index => root.setValue("image_fit", index === 1 ? "cover" : "contain")
    }
    Label {
        Layout.fillWidth: true
        visible: root.value("image_fit", "contain") === "cover"
        text: qsTr("Edges outside the image area will be cropped. Check the preview before printing.")
        color: Theme.warning; wrapMode: Text.WordWrap; font.pixelSize: 12
    }
    RowLayout {
        Layout.fillWidth: true
        Label { Layout.fillWidth: true; text: qsTr("Text size (%)"); color: Theme.muted }
        SpinBox {
            objectName: root.perCard ? "cardTextSizeOverride" : "cardTextSizeDefault"
            from: 50; to: 300; stepSize: 10; editable: true; value: root.value("text_scale_percent", 100)
            onValueModified: root.setValue("text_scale_percent", value)
        }
    }
    ToolButton { objectName: "cardLayoutAdvanced"; text: (root.expanded ? "⌄  " : "›  ") + qsTr("Colors, border & spacing"); font.pixelSize: 11; onClicked: root.expanded = !root.expanded }
    ColumnLayout {
        visible: root.expanded
        Layout.fillWidth: true; spacing: 8
        Label { text: qsTr("Background color"); color: Theme.muted; font.pixelSize: 11 }
        RowLayout {
            Layout.fillWidth: true; spacing: 8
            ToolButton {
                objectName: "cardBackgroundSwatch"
                implicitWidth: 34; implicitHeight: 34
                Accessible.name: qsTr("Choose background color")
                onClicked: root.chooseColor("background_color")
                background: Rectangle { radius: 7; color: root.value("background_color", "#ffffff"); border.color: Theme.borderStrong }
            }
            ShapeTextField {
                objectName: root.perCard ? "cardBackgroundOverride" : "cardBackgroundDefault"
                Layout.fillWidth: true
                text: root.value("background_color", "#ffffff"); placeholderText: "#ffffff"; maximumLength: 7
                validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
                onEditingFinished: {
                    if (acceptableInput) root.setValue("background_color", text)
                    else root.validationMessage = qsTr("Use a color such as #ffffff.")
                }
            }
        }
        Label { text: qsTr("Text color"); color: Theme.muted; font.pixelSize: 11 }
        RowLayout {
            Layout.fillWidth: true; spacing: 8
            ToolButton {
                objectName: "cardTextColorSwatch"
                implicitWidth: 34; implicitHeight: 34
                Accessible.name: qsTr("Choose text color")
                onClicked: root.chooseColor("text_color")
                background: Rectangle { radius: 7; color: root.value("text_color", "#202020"); border.color: Theme.borderStrong }
            }
            ShapeTextField {
                objectName: root.perCard ? "cardTextColorOverride" : "cardTextColorDefault"
                Layout.fillWidth: true
                text: root.value("text_color", "#202020"); placeholderText: "#202020"; maximumLength: 7
                validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
                onEditingFinished: {
                    if (acceptableInput) root.setValue("text_color", text)
                    else root.validationMessage = qsTr("Use a color such as #ffffff.")
                }
            }
        }
        CheckBox {
            objectName: root.perCard ? "cardBorderOverride" : "cardBorderDefault"
            text: qsTr("Show a border"); checked: root.value("border", false)
            onToggled: root.setValue("border", checked)
        }
        Label { text: qsTr("Text alignment"); color: Theme.muted }
        ShapeComboBox {
            objectName: root.perCard ? "cardTextAlignOverride" : "cardTextAlignDefault"
            Layout.fillWidth: true
            model: [qsTr("Left"), qsTr("Center"), qsTr("Right")]
            currentIndex: Math.max(0, ["left", "center", "right"].indexOf(root.value("text_align", "left")))
            onActivated: index => root.setValue("text_align", ["left", "center", "right"][index])
        }
        RowLayout {
            Layout.fillWidth: true
            Label { Layout.fillWidth: true; text: qsTr("Inner space (%)"); color: Theme.muted; wrapMode: Text.WordWrap }
            SpinBox {
                objectName: root.perCard ? "cardPaddingOverride" : "cardPaddingDefault"
                from: 0; to: 20; value: Math.round(root.value("padding_percent", 4))
                onValueModified: root.setValue("padding_percent", value)
            }
        }
        ShapeButton { quiet: true; visible: root.perCard && root.configuration !== null; text: qsTr("Reset this card’s layout options"); onClicked: root.edited(null) }
        Label { Layout.fillWidth: true; visible: root.validationMessage.length > 0; text: root.validationMessage; color: Theme.danger; wrapMode: Text.WordWrap }
    }
}
