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
    signal editCardRequested(string cardId)
    property string pendingExportPath: ""
    property var pendingExportSource: ({})
    property var sourcePin: ({})
    property var options: ({})
    property var cardChoices: []
    property string selectedCardId: ""
    property string projectIdentity: ""
    property string sizeAnchor: ""
    property bool keepProportions: true
    property int gridColumns: 3
    property int gridRows: 3
    property int contentMode: 0
    property bool fillSheet: true
    property int repeatCopies: 9
    property bool advanced: false
    readonly property string problemTitle: {
        const card = cardChoices.find(c => c.id === controller.problemCardId)
        return card ? card.title || card.id : ""
    }
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: parent ? Math.min(1180, parent.width - 40) : 1100
    height: parent ? Math.min(840, parent.height - 40) : 780
    title: qsTr("Print & arrange")
    modal: true
    closePolicy: Popup.NoAutoClose
    function openForSource(source, defaults, cards, selectedId) {
        if (controller.running) return false
        sourcePin = JSON.parse(JSON.stringify(source))
        projectIdentity = backend.projectId + "|" + backend.bundlePath
        cardChoices = (cards || []).map(c => ({id: c.id, title: c.title}))
        selectedCardId = cardChoices.some(c => c.id === selectedId) ? selectedId : cardChoices.length ? cardChoices[0].id : ""
        options = Object.assign({paper: "a4", margin_mm: 5, gap_mm: 3,
            crop_mark_offset_mm: 0.5, crop_mark_length_mm: 1.5, stroke_width_mm: 0.2}, defaults || {})
        sizeAnchor = ""; keepProportions = true; gridColumns = 3; gridRows = 3
        contentMode = 0; fillSheet = true; repeatCopies = 9; advanced = false
        const result = refresh()
        open(); return result
    }
    function chooseGrid(columns, rows) {
        if (controller.running) return
        gridColumns = columns; gridRows = rows; refreshTimer.restart()
    }
    function setOption(key, value) {
        if (controller.running || options[key] === value) return
        options = Object.assign({}, options, {[key]: value})
        if (key === "card_width_mm" || key === "card_height_mm") {
            gridColumns = 0; gridRows = 0
            sizeAnchor = keepProportions ? (key === "card_width_mm" ? "width" : "height") : "both"
        }
        refreshTimer.restart()
    }
    function refresh() {
        refreshTimer.stop()
        const request = Object.assign({}, options)
        if (gridColumns > 0 || sizeAnchor === "" || sizeAnchor === "height") delete request.card_width_mm
        if (gridColumns > 0 || sizeAnchor === "" || sizeAnchor === "width") delete request.card_height_mm
        const imposition = {content: ["quantities", "one_each", "repeat"][contentMode]}
        if (gridColumns > 0) { imposition.columns = gridColumns; imposition.rows = gridRows }
        if (contentMode === 2) {
            imposition.card_id = selectedCardId
            if (!fillSheet) imposition.copies = repeatCopies
        }
        const result = controller.prepare(sourcePin, request, imposition)
        if (result) options = Object.assign({}, controller.plan.options)
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
        spacing: 12
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 20
            ScrollView {
                id: settingsScroll
                Layout.preferredWidth: 264; Layout.fillHeight: true
                contentWidth: availableWidth; clip: true; ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ColumnLayout {
                    width: settingsScroll.availableWidth; spacing: 10
                    enabled: !root.controller.running
                    Label { text: qsTr("Paper"); color: Theme.text; font.weight: Font.DemiBold }
                    ShapeComboBox {
                        objectName: "cardPrintPaper"; Layout.fillWidth: true
                        model: [qsTr("A4 · 210 × 297 mm"), qsTr("US Letter · 8.5 × 11 in")]
                        selectedIndex: root.options.paper === "letter" ? 1 : 0
                        onActivated: index => root.setOption("paper", index === 1 ? "letter" : "a4")
                    }
                    Label { text: qsTr("Cards on the sheet"); color: Theme.text; font.weight: Font.DemiBold; Layout.topMargin: 4 }
                    ShapeComboBox {
                        objectName: "cardPrintContent"; Layout.fillWidth: true
                        model: [qsTr("Mix cards · use their quantities"), qsTr("Mix cards · one of each"), qsTr("Repeat one card")]
                        selectedIndex: root.contentMode
                        onActivated: index => { root.contentMode = index; refreshTimer.restart() }
                    }
                    ColumnLayout {
                        visible: root.contentMode === 2; Layout.fillWidth: true; spacing: 7
                        ShapeComboBox {
                            objectName: "cardPrintSelectedCard"; Layout.fillWidth: true
                            model: root.cardChoices.map(c => c.title || c.id)
                            selectedIndex: Math.max(0, root.cardChoices.findIndex(c => c.id === root.selectedCardId))
                            onActivated: index => { root.selectedCardId = root.cardChoices[index].id; refreshTimer.restart() }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            ShapeCheckBox { objectName: "cardPrintFillSheet"; Layout.fillWidth: true; text: qsTr("Fill one sheet"); checked: root.fillSheet; onToggled: { root.fillSheet = checked; refreshTimer.restart() } }
                            ShapeSpinBox { objectName: "cardPrintCopies"; visible: !root.fillSheet; from: 1; to: 4096; value: root.repeatCopies; onValueModified: { root.repeatCopies = value; refreshTimer.restart() } }
                        }
                    }
                    Label { text: qsTr("Quick arrangement"); color: Theme.text; font.weight: Font.DemiBold; Layout.topMargin: 4 }
                    CardSheetPresets {
                        Layout.fillWidth: true
                        selectedColumns: root.gridColumns; selectedRows: root.gridRows
                        onChosen: (columns, rows) => root.chooseGrid(columns, rows)
                    }
                    RowLayout {
                        Layout.fillWidth: true; spacing: 7
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 3
                            Label { text: qsTr("Columns"); color: Theme.muted; font.pixelSize: 11 }
                            ShapeSpinBox { objectName: "cardPrintColumns"; Layout.fillWidth: true; from: 1; to: 8; value: root.gridColumns || root.controller.plan.columns || 3; onValueModified: root.chooseGrid(value, root.gridRows || root.controller.plan.rows || 3) }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 3
                            Label { text: qsTr("Rows"); color: Theme.muted; font.pixelSize: 11 }
                            ShapeSpinBox { objectName: "cardPrintRows"; Layout.fillWidth: true; from: 1; to: 8; value: root.gridRows || root.controller.plan.rows || 3; onValueModified: root.chooseGrid(root.gridColumns || root.controller.plan.columns || 3, value) }
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: root.gridColumns > 0 ? qsTr("Cards fit proportionally inside this grid.") : qsTr("Using a fixed card size; the sheet fills automatically.")
                        color: Theme.muted; font.pixelSize: 11; wrapMode: Text.WordWrap
                    }
                    ShapeButton { objectName: "cardPrintActualSize"; text: qsTr("Use a fixed card size"); quiet: true; selected: root.gridColumns === 0; onClicked: { root.chooseGrid(0, 0); root.advanced = true; root.sizeAnchor = "width" } }
                    Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
                    ShapeButton { objectName: "cardPrintAdvanced"; quiet: true; iconSource: "../../icons/settings.svg"; text: root.advanced ? qsTr("Hide fine adjustments") : qsTr("Fine adjustments…"); onClicked: root.advanced = !root.advanced }
                    ColumnLayout {
                        visible: root.advanced; Layout.fillWidth: true; spacing: 8
                        ShapeCheckBox {
                            objectName: "cardPrintKeepProportions"
                            text: qsTr("Keep the card proportions"); checked: root.keepProportions; enabled: root.gridColumns === 0
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
                            delegate: RowLayout {
                                id: optionRow
                                required property var modelData
                                Layout.fillWidth: true; spacing: 8
                                Label { Layout.fillWidth: true; text: optionRow.modelData.label; color: Theme.textSoft; font.pixelSize: 11; wrapMode: Text.WordWrap }
                                ShapeSpinBox {
                                    id: dimension
                                    objectName: "cardPrint-" + optionRow.modelData.key
                                    from: optionRow.modelData.min; to: optionRow.modelData.max; stepSize: optionRow.modelData.step
                                    value: Math.round((root.options[optionRow.modelData.key] || 0) * 10)
                                    enabled: root.gridColumns === 0 || !optionRow.modelData.key.startsWith("card_")
                                    validator: DoubleValidator { bottom: dimension.from / 10; top: dimension.to / 10; decimals: 1; notation: DoubleValidator.StandardNotation }
                                    textFromValue: (value, locale) => Number(value / 10).toLocaleString(locale, 'f', 1)
                                    valueFromText: (text, locale) => Math.round(Number.fromLocaleString(locale, text) * 10)
                                    onValueModified: root.setOption(optionRow.modelData.key, value / 10)
                                }
                            }
                        }
                        Label { Layout.fillWidth: true; text: qsTr("Crop marks stay outside the finished card. Increase the spacing if longer marks do not fit."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                    }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: 8
                RowLayout {
                    Layout.fillWidth: true
                    Label { Layout.fillWidth: true; text: qsTr("Sheet preview"); color: Theme.text; font.weight: Font.DemiBold }
                    Label { objectName: "cardPrintSizeSummary"; text: root.controller.ready ? qsTr("%1 × %2 mm per card").arg(Number(root.options.card_width_mm).toFixed(1)).arg(Number(root.options.card_height_mm).toFixed(1)) : qsTr("Layout guide"); color: Theme.muted; font.pixelSize: 11 }
                }
                Rectangle {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    color: Theme.panelInset; radius: Theme.panelRadius; border.color: Theme.border
                    Image { objectName: "cardPrintSheetPreview"; anchors.fill: parent; anchors.margins: 16; source: root.controller.previewSource; fillMode: Image.PreserveAspectFit; opacity: refreshTimer.running ? 0.55 : 1 }
                    Rectangle {
                        visible: !root.controller.ready
                        width: Math.min(parent.width - 40, (parent.height - 40) * 210 / 297); height: width * 297 / 210
                        anchors.centerIn: parent; color: "#ffffff"; border.color: "#d0d0d0"
                        Grid {
                            anchors.fill: parent; anchors.margins: 16; spacing: 4
                            columns: root.gridColumns || 3
                            Repeater {
                                model: (root.gridColumns || 3) * (root.gridRows || 3)
                                Rectangle { width: (parent.width - ((root.gridColumns || 3) - 1) * 4) / (root.gridColumns || 3); height: (parent.height - ((root.gridRows || 3) - 1) * 4) / (root.gridRows || 3); color: "#f2f2f2"; border.color: "#bcbcbc"; border.width: 1 }
                            }
                        }
                    }
                    BusyIndicator { anchors.centerIn: parent; width: 32; height: 32; running: refreshTimer.running; visible: running }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Label { objectName: "cardPrintSummary"; Layout.fillWidth: true; text: root.controller.ready ? qsTr("%1 cards / sheet · %2 copies · %3 pages").arg(root.controller.plan.slots_per_page).arg(root.controller.plan.total_copies).arg(root.controller.plan.page_count) : qsTr("Fix the card content to preview and export."); color: Theme.muted; font.pixelSize: 11; wrapMode: Text.WordWrap }
                    ShapeIconButton { objectName: "cardPrintPrevious"; source: "../../icons/back.svg"; toolTipText: qsTr("Previous page"); enabled: root.controller.ready && root.controller.pageIndex > 0; onClicked: root.controller.showPage(root.controller.pageIndex - 1) }
                    Label { text: root.controller.ready ? (root.controller.pageIndex + 1) + " / " + root.controller.plan.page_count : "—"; color: Theme.text; font.pixelSize: 11 }
                    ShapeIconButton { objectName: "cardPrintNext"; source: "../../icons/back.svg"; iconRotation: 180; toolTipText: qsTr("Next page"); enabled: root.controller.ready && root.controller.pageIndex + 1 < (root.controller.plan.page_count || 0); onClicked: root.controller.showPage(root.controller.pageIndex + 1) }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true; visible: root.controller.error.length > 0
            Label { Layout.fillWidth: true; text: (root.problemTitle.length ? qsTr("Card: %1").arg(root.problemTitle) + " · " : "") + root.controller.error; color: Theme.danger; wrapMode: Text.WordWrap; font.pixelSize: 11 }
            ShapeButton { objectName: "cardPrintFixCard"; visible: root.controller.problemCardId.length > 0; text: qsTr("Edit this card"); onClicked: { refreshTimer.stop(); root.close(); root.editCardRequested(root.controller.problemCardId) } }
        }
        RowLayout {
            Layout.fillWidth: true; spacing: 14
            ShapeButton { objectName: "cardPrintClose"; text: qsTr("Back to cards"); enabled: !root.controller.running; onClicked: { refreshTimer.stop(); root.close() } }
            Label { Layout.fillWidth: true; text: root.controller.status || qsTr("Crop marks included · print at 100% / Actual size."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
            ShapeButton { objectName: "cardPrintExport"; iconSource: "../../icons/print.svg"; text: root.controller.running ? qsTr("Exporting…") : qsTr("Export print PDF…"); primary: true; enabled: root.controller.ready && !root.controller.running && !refreshTimer.running; onClicked: { destination.planVersion = root.controller.version; destination.open() } }
        }
    }
}
