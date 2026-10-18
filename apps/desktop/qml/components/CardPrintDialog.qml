pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop

ShapeDialog {
    id: root
    objectName: "cardPrintDialog"
    required property DesktopBackend backend
    required property CardPrintController controller
    signal fileExported(string path, var source)
    property string pendingExportPath: ""
    property var pendingExportSource: ({})
    property var sourcePin: ({})
    property var options: ({})
    property string projectIdentity: ""
    property string sizeAnchor: ""
    property bool keepProportions: true
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: parent ? parent.width - 48 : 1100
    height: parent ? parent.height - 48 : 780
    title: qsTr("Print card layout")
    modal: true
    closePolicy: Popup.NoAutoClose
    function openForSource(source, defaults) {
        if (controller.running) return false
        sourcePin = JSON.parse(JSON.stringify(source))
        projectIdentity = backend.projectId + "|" + backend.bundlePath
        options = Object.assign({paper: "a4", margin_mm: 5, gap_mm: 3,
            crop_mark_offset_mm: 0.5, crop_mark_length_mm: 1.5, stroke_width_mm: 0.2}, defaults || {})
        sizeAnchor = ""; keepProportions = true
        const result = refresh()
        open(); return result
    }
    function setOption(key, value) {
        if (controller.running || options[key] === value) return
        const next = JSON.parse(JSON.stringify(options)); next[key] = value; options = next
        if (key === "card_width_mm" || key === "card_height_mm")
            sizeAnchor = keepProportions ? (key === "card_width_mm" ? "width" : "height") : "both"
        refreshTimer.restart()
    }
    function refresh() {
        refreshTimer.stop()
        const request = JSON.parse(JSON.stringify(options))
        if (sizeAnchor === "" || sizeAnchor === "height") delete request.card_width_mm
        if (sizeAnchor === "" || sizeAnchor === "width") delete request.card_height_mm
        const result = controller.prepare(sourcePin, request)
        if (result) options = JSON.parse(JSON.stringify(controller.plan.options))
        return result
    }
    Timer { id: refreshTimer; interval: 180; onTriggered: root.refresh() }
    Connections {
        target: root.backend
        function onProjectChanged() {
            if (root.projectIdentity !== root.backend.projectId + "|" + root.backend.bundlePath) {
                refreshTimer.stop(); destination.close(); root.close()
            }
        }
    }
    Connections {
        target: root.controller
        function onFinished(success) {
            if (success && root.pendingExportPath.length)
                root.fileExported(root.pendingExportPath, root.pendingExportSource)
            root.pendingExportPath = ""; root.pendingExportSource = ({})
        }
    }
    FileDialog {
        id: destination
        property int planVersion: -1
        title: qsTr("Export card print PDF")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "pdf"
        nameFilters: [qsTr("PDF documents (*.pdf)")]
        onAccepted: {
            root.pendingExportPath = selectedFile.toString()
            root.pendingExportSource = JSON.parse(JSON.stringify(root.sourcePin))
            if (!root.controller.exportPdf(selectedFile, planVersion)) root.pendingExportPath = ""
        }
    }
    contentItem: ColumnLayout {
        spacing: 10
        Label {
            Layout.fillWidth: true
            text: root.controller.ready ? qsTr("%1 · %2 copies · %3 pages · %4 cards per full page")
                .arg(root.controller.plan.name).arg(root.controller.plan.total_copies)
                .arg(root.controller.plan.page_count).arg(root.controller.plan.slots_per_page) : qsTr("Choose a layout for the accepted card version.")
            color: Theme.text; wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 18
            ScrollView {
                Layout.preferredWidth: 250; Layout.fillHeight: true
                contentWidth: availableWidth; clip: true; ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ColumnLayout {
                    width: parent.width; spacing: 10
                    Label { text: qsTr("Paper"); color: Theme.text; font.weight: Font.DemiBold }
                    ShapeComboBox {
                        objectName: "cardPrintPaper"
                        Layout.fillWidth: true
                        model: [qsTr("A4 · 210 × 297 mm"), qsTr("US Letter · 8.5 × 11 in")]
                        currentIndex: root.options.paper === "letter" ? 1 : 0
                        enabled: !root.controller.running
                        onActivated: index => root.setOption("paper", index === 1 ? "letter" : "a4")
                    }
                    CheckBox {
                        objectName: "cardPrintKeepProportions"
                        text: qsTr("Keep the card proportions")
                        checked: root.keepProportions; enabled: !root.controller.running
                        onToggled: { root.keepProportions = checked; root.sizeAnchor = checked ? "width" : "both"; refreshTimer.restart() }
                    }
                    Repeater {
                        model: [
                            {key: "card_width_mm", label: qsTr("Card width (mm)"), min: 100, max: 3000, step: 10},
                            {key: "card_height_mm", label: qsTr("Card height (mm)"), min: 100, max: 3000, step: 10},
                            {key: "margin_mm", label: qsTr("Paper margin (mm)"), min: 0, max: 500, step: 10},
                            {key: "gap_mm", label: qsTr("Space between cards (mm)"), min: 0, max: 300, step: 5},
                            {key: "crop_mark_offset_mm", label: qsTr("Mark distance from card (mm)"), min: 1, max: 100, step: 1},
                            {key: "crop_mark_length_mm", label: qsTr("Mark length (mm)"), min: 1, max: 100, step: 1},
                            {key: "stroke_width_mm", label: qsTr("Mark line width (mm)"), min: 1, max: 5, step: 1}
                        ]
                        delegate: ColumnLayout {
                            id: optionRow
                            required property var modelData
                            Layout.fillWidth: true; spacing: 3
                            Label { Layout.fillWidth: true; text: optionRow.modelData.label; color: Theme.text; wrapMode: Text.WordWrap }
                            SpinBox {
                                id: dimension
                                objectName: "cardPrint-" + optionRow.modelData.key
                                Layout.fillWidth: true
                                from: optionRow.modelData.min; to: optionRow.modelData.max; stepSize: optionRow.modelData.step
                                value: Math.round((root.options[optionRow.modelData.key] || 0) * 10)
                                editable: true; enabled: !root.controller.running
                                validator: DoubleValidator { bottom: dimension.from / 10; top: dimension.to / 10; decimals: 1; notation: DoubleValidator.StandardNotation }
                                textFromValue: (value, locale) => Number(value / 10).toLocaleString(locale, 'f', 1)
                                valueFromText: (text, locale) => Math.round(Number.fromLocaleString(locale, text) * 10)
                                onValueModified: root.setOption(optionRow.modelData.key, value / 10)
                            }
                        }
                    }
                    Label { Layout.fillWidth: true; text: qsTr("Crop marks stay outside the finished card. Increase the spacing if longer marks do not fit."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 12 }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true
                Rectangle {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    color: Theme.panelInset; radius: Theme.panelRadius; border.color: Theme.border
                    Image { objectName: "cardPrintSheetPreview"; anchors.fill: parent; anchors.margins: 12; source: root.controller.previewSource; fillMode: Image.PreserveAspectFit }
                    Label { anchors.centerIn: parent; width: parent.width - 40; visible: !root.controller.ready; text: qsTr("Adjust the layout to see the paper preview."); color: Theme.muted; wrapMode: Text.WordWrap; horizontalAlignment: Text.AlignHCenter }
                }
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    ShapeButton { objectName: "cardPrintPrevious"; text: qsTr("Previous page"); enabled: root.controller.ready && root.controller.pageIndex > 0; onClicked: root.controller.showPage(root.controller.pageIndex - 1) }
                    Label { text: root.controller.ready ? qsTr("Page %1 of %2").arg(root.controller.pageIndex + 1).arg(root.controller.plan.page_count) : ""; color: Theme.text }
                    ShapeButton { objectName: "cardPrintNext"; text: qsTr("Next page"); enabled: root.controller.ready && root.controller.pageIndex + 1 < (root.controller.plan.page_count || 0); onClicked: root.controller.showPage(root.controller.pageIndex + 1) }
                }
            }
        }
        Label { Layout.fillWidth: true; text: root.controller.error || root.controller.status; visible: text.length > 0; color: root.controller.error.length ? Theme.danger : Theme.accent; wrapMode: Text.WordWrap }
        Label { Layout.fillWidth: true; text: qsTr("Print the PDF at 100% or Actual size. Turn off Fit to page. This layout is for single-sided printing."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 12 }
        RowLayout {
            Layout.fillWidth: true
            ShapeButton { objectName: "cardPrintClose"; text: qsTr("Back to cards"); enabled: !root.controller.running; onClicked: { refreshTimer.stop(); root.close() } }
            Item { Layout.fillWidth: true }
            ShapeButton { objectName: "cardPrintExport"; text: root.controller.running ? qsTr("Exporting…") : qsTr("Export print PDF…"); primary: true; enabled: root.controller.ready && !root.controller.running && !refreshTimer.running; onClicked: { destination.planVersion = root.controller.version; destination.open() } }
        }
    }
}
