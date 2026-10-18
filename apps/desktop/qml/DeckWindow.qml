pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtQuick.Window
import Shape.Desktop

ApplicationWindow {
    id: window
    objectName: "deckWindow"
    required property DesktopBackend backend
    required property CardKitController cardKit
    required property CardPrintController cardPrint
    required property MaterialEditController materialEdit
    required property UiPreferences preferences
    required property DeckDocumentController documents
    required property DeckExchangeController exchange
    required property ExternalImageTaskController imageTask
    required property CardDraftAssistant assistant
    property string initialProject: ""
    property string initialKit: ""
    property bool smokeMode: false
    property bool closeAfterAssistant: false
    width: 1280; height: 840
    minimumWidth: 980; minimumHeight: 680
    visible: true
    title: Qt.platform.os === "osx" ? "" : backend.projectOpen ? backend.projectName + " — Deck" : "Deck"
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    color: Theme.window
    palette.window: Theme.window; palette.windowText: Theme.text; palette.text: Theme.text
    palette.button: Theme.buttonSurface; palette.buttonText: Theme.text
    palette.base: Theme.control; palette.highlight: Theme.accent; palette.highlightedText: Theme.accentText

    function openDocument(path, kit) {
        if (!cards.flush()) return false
        if (!documents.openLocalProject(path)) return false
        cards.openWorkspace(kit || "")
        if (exchange.active && !exchange.needsImport) exchange.bindCurrent(cardKit.draft.artifact_id || "")
        return true
    }
    function showProjects() { if (cards.flush()) projectsDialog.open() }
    Component.onCompleted: {
        Theme.mode = Theme.AppearanceMode.System; Theme.systemDark = preferences.dark
        if (exchange.active && !exchange.needsImport) openDocument(exchange.projectPath, exchange.kitId)
        else if (initialProject.length) openDocument(initialProject, initialKit)
    }
    Connections { target: window.preferences; function onDarkChanged() { Theme.systemDark = window.preferences.dark } }
    Connections { target: window.exchange; function onCompleted() { window.close() } }
    onClosing: close => {
        if (assistant.running) { closeAfterAssistant = true; assistant.discard(); close.accepted = false; return }
        if (cardPrint.running || !cards.flush()) { close.accepted = false; return }
        if (exchange.active && !exchange.cancel()) close.accepted = false
    }
    Connections { target: window.assistant; function onChanged() { if (window.closeAfterAssistant && !window.assistant.busy) window.close() } }
    Shortcut { sequence: StandardKey.Open; enabled: !window.exchange.active; onActivated: window.showProjects() }
    Shortcut { sequence: StandardKey.New; enabled: !window.exchange.active; onActivated: if (cards.flush()) { newName.text = ""; newProjectDialog.open() } }
    Shortcut { sequence: StandardKey.Print; enabled: window.backend.projectOpen && !window.cardPrint.running; onActivated: cards.acceptAndPrint() }
    Shortcut { sequence: "Ctrl+,"; onActivated: settingsDialog.open() }
    header: ToolBar {
        id: titleBar
        objectName: "titleToolBar"
        implicitHeight: 56
        topPadding: 0; bottomPadding: 0
        leftPadding: Math.max(SafeArea.margins.left, Qt.platform.os === "osx" && window.visibility !== Window.FullScreen ? 96 : 16)
        rightPadding: Math.max(SafeArea.margins.right, Qt.platform.os === "windows" ? 152 : 16)
        background: Rectangle {
            color: Theme.chrome
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }
        contentItem: Item {
            RowLayout {
                anchors.fill: parent
                spacing: 10
                Label {
                    text: "Deck"; font.pixelSize: 20; font.weight: Font.DemiBold; color: Theme.text
                    DragHandler {
                        target: null
                        acceptedButtons: Qt.LeftButton
                        onActiveChanged: if (active) window.startSystemMove()
                    }
                }
                Rectangle { implicitWidth: 1; implicitHeight: 24; color: Theme.border; Layout.leftMargin: 6; Layout.rightMargin: 6 }
                ToolButton {
                    id: projectButton
                    objectName: "deckProjectMenuButton"
                    readonly property string heading: window.backend.projectOpen
                        ? window.backend.projectName + " / " + (cards.composition.name || qsTr("Card set"))
                        : qsTr("Cards, your way")
                    Layout.minimumWidth: 140
                    Layout.preferredWidth: Math.round(Math.min(400, window.width * 0.34))
                    implicitHeight: Theme.iconButtonSize; leftPadding: 8; rightPadding: 8
                    Accessible.name: qsTr("Project menu")
                    Accessible.description: heading
                    contentItem: RowLayout {
                        spacing: 6
                        Label {
                            objectName: "deckProjectHeading"
                            Layout.fillWidth: true
                            text: projectButton.heading; color: Theme.text
                            font.pixelSize: 13; font.weight: Font.Medium
                            elide: Text.ElideRight; maximumLineCount: 1; wrapMode: Text.NoWrap
                        }
                        ShapeIcon { source: "qrc:/qt/qml/Shape/Desktop/icons/chevron-down.svg"; size: Theme.compactIconSize }
                    }
                    ToolTip.visible: hovered
                    ToolTip.delay: 450
                    ToolTip.text: heading
                    background: Rectangle { radius: 8; color: projectButton.hovered ? Theme.surfaceSubtle : "transparent" }
                    onClicked: projectMenu.popup(projectButton, 0, projectButton.height)
                }
                Item {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    DragHandler {
                        target: null
                        acceptedButtons: Qt.LeftButton
                        onActiveChanged: if (active) window.startSystemMove()
                    }
                }
                ShapeIconButton {
                    id: languageButton
                    objectName: "deckLanguageButton"
                    source: "qrc:/qt/qml/Shape/Desktop/icons/language.svg"
                    toolTipText: qsTr("Language")
                    onClicked: languageMenu.popup(languageButton, 0, languageButton.height)
                }
                ShapeIconButton {
                    id: appearanceButton
                    objectName: "deckAppearanceButton"
                    source: window.preferences.dark ? "qrc:/qt/qml/Shape/Desktop/icons/moon.svg" : "qrc:/qt/qml/Shape/Desktop/icons/sun.svg"
                    toolTipText: qsTr("Appearance")
                    onClicked: appearanceMenu.popup(appearanceButton, 0, appearanceButton.height)
                }
                ShapeIconButton {
                    objectName: "deckSettingsButton"
                    source: "qrc:/qt/qml/Shape/Desktop/icons/settings.svg"
                    toolTipText: qsTr("Settings")
                    onClicked: settingsDialog.open()
                }
                Rectangle { visible: window.backend.projectOpen; implicitWidth: 1; implicitHeight: 22; color: Theme.border }
                Label {
                    objectName: "deckSaveStatus"
                    visible: window.backend.projectOpen
                    text: cards.dirty ? (window.cardKit.error.length ? qsTr("Not saved") : qsTr("Saving…")) : window.cardKit.currentAccepted ? qsTr("Version saved") : qsTr("Draft saved")
                    color: window.cardKit.error.length ? Theme.danger : Theme.muted; font.pixelSize: 11
                }
                ShapeIconButton {
                    objectName: "cardKitPrint"
                    visible: window.backend.projectOpen
                    toolTipText: !cards.dirty && window.cardKit.currentAccepted ? qsTr("Print…") : qsTr("Accept & print…")
                    primary: true; enabled: !window.cardPrint.running
                    source: "qrc:/qt/qml/Shape/Desktop/icons/print.svg"
                    onClicked: cards.acceptAndPrint()
                }
                ShapeIconButton {
                    id: moreButton
                    objectName: "deckMoreMenuButton"
                    source: "qrc:/qt/qml/Shape/Desktop/icons/more.svg"
                    toolTipText: qsTr("More actions")
                    enabled: window.backend.projectOpen
                    onClicked: moreMenu.popup(moreButton, moreButton.width - moreMenu.width, moreButton.height)
                }
            }
        }
    }
    ShapeMenu {
        id: languageMenu; objectName: "deckLanguageMenu"; popupType: Popup.Item
        ShapeMenuItem { text: qsTr("Follow system"); checkable: true; checked: window.preferences.languageMode === "system"; onTriggered: window.preferences.languageMode = "system" }
        ShapeMenuItem { objectName: "deckEnglish"; text: "English"; checkable: true; checked: window.preferences.languageMode === "en"; onTriggered: window.preferences.languageMode = "en" }
        ShapeMenuItem { objectName: "deckChinese"; text: "简体中文"; checkable: true; checked: window.preferences.languageMode === "zh_CN"; onTriggered: window.preferences.languageMode = "zh_CN" }
    }
    ShapeMenu {
        id: appearanceMenu; objectName: "deckAppearanceMenu"; popupType: Popup.Item
        ShapeMenuItem { objectName: "deckAppearanceSystem"; text: qsTr("Follow system"); icon.source: "qrc:/qt/qml/Shape/Desktop/icons/monitor.svg"; icon.color: Theme.text; checkable: true; checked: window.preferences.appearanceMode === 0; onTriggered: window.preferences.appearanceMode = 0 }
        ShapeMenuItem { objectName: "deckAppearanceLight"; text: qsTr("Light"); icon.source: "qrc:/qt/qml/Shape/Desktop/icons/sun.svg"; icon.color: Theme.text; checkable: true; checked: window.preferences.appearanceMode === 1; onTriggered: window.preferences.appearanceMode = 1 }
        ShapeMenuItem { objectName: "deckAppearanceDark"; text: qsTr("Dark"); icon.source: "qrc:/qt/qml/Shape/Desktop/icons/moon.svg"; icon.color: Theme.text; checkable: true; checked: window.preferences.appearanceMode === 2; onTriggered: window.preferences.appearanceMode = 2 }
    }
    ShapeMenu {
        popupType: Popup.Item
        id: projectMenu; objectName: "deckProjectMenu"
        ShapeMenuItem { objectName: "deckNewProject"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/add.svg"; icon.color: Theme.text; text: qsTr("New project…"); enabled: !window.exchange.active; onTriggered: { if (cards.flush()) { newName.text = ""; newProjectDialog.open() } } }
        ShapeMenuItem { objectName: "deckOpenProject"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/open.svg"; icon.color: Theme.text; text: qsTr("Open project…"); enabled: !window.exchange.active; onTriggered: window.showProjects() }
        ShapeMenuSeparator {}
        ShapeMenuItem { objectName: "cardKitNew"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/add.svg"; icon.color: Theme.text; text: qsTr("New card set"); enabled: window.backend.projectOpen; onTriggered: cards.newKit() }
        ShapeMenu {
        popupType: Popup.Item
            id: setsMenu; title: qsTr("Switch card set"); enabled: window.backend.projectOpen && cards.savedKits.length > 1
            Instantiator {
                model: cards.savedKits.slice(1)
                delegate: ShapeMenuItem { required property var modelData; text: modelData.name; checkable: true; checked: modelData.id === (window.cardKit.draft.artifact_id || ""); onTriggered: cards.selectKit(modelData.id) }
                onObjectAdded: (index, object) => setsMenu.insertItem(index, object)
                onObjectRemoved: (index, object) => setsMenu.removeItem(object)
            }
        }
        ShapeMenuItem { text: qsTr("Card set name & defaults…"); enabled: window.backend.projectOpen; onTriggered: cards.openSetLayout() }
        ShapeMenuSeparator {}
        ShapeMenuItem { objectName: "deckSettings"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/settings.svg"; icon.color: Theme.text; text: qsTr("Settings…"); onTriggered: settingsDialog.open() }
    }
    ShapeMenu {
        popupType: Popup.Item
        id: moreMenu; objectName: "deckMoreMenu"
        ShapeMenuItem { objectName: "cardKitExport"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/export.svg"; icon.color: Theme.text; text: qsTr("Export card files…"); enabled: !cards.dirty && window.cardKit.canExport; onTriggered: cards.exportAcceptedFiles() }
        ShapeMenuSeparator {}
        ShapeMenuItem { objectName: "cardKitAccept"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/verified.svg"; icon.color: Theme.text; text: qsTr("Save this version"); enabled: !window.cardKit.currentAccepted || cards.dirty; onTriggered: cards.saveVersion() }
        ShapeMenuItem { objectName: "cardKitReview"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/search.svg"; icon.color: Theme.text; text: qsTr("Check cards"); onTriggered: if (cards.flush()) window.cardKit.review(cards.selectedCardId) }
        ShapeMenuSeparator {}
        ShapeMenuItem { objectName: "cardKitCopyDraft"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/duplicate.svg"; icon.color: Theme.text; text: qsTr("Copy complete draft"); onTriggered: window.cardKit.copyDraft(cards.composition) }
        ShapeMenuItem { objectName: "cardKitReloadDraft"; icon.source: "qrc:/qt/qml/Shape/Desktop/icons/back.svg"; icon.color: Theme.text; text: qsTr("Reload saved draft…"); onTriggered: cards.requestReloadDraft() }
    }
    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            visible: window.exchange.active
            Layout.fillWidth: true; implicitHeight: exchangeBar.implicitHeight + 20
            color: Theme.accentSoft
            RowLayout {
                id: exchangeBar
                anchors.fill: parent; anchors.margins: 10; spacing: 12
                Label { Layout.fillWidth: true; text: window.exchange.needsImport ? qsTr("Cards from Shape · Save a local Deck project to continue. The original stays in Shape.") : (!window.cardKit.currentAccepted || cards.dirty ? qsTr("Finish and accept the current cards, then export a PDF or archive to return to Shape.") : qsTr("Editing cards for Shape · Export a PDF or archive, then return the outputs.")); color: Theme.text; wrapMode: Text.WordWrap }
                ShapeButton { objectName: "deckSaveTransferredCards"; visible: window.exchange.needsImport; text: qsTr("Save cards in Deck…"); primary: true; onClicked: { newName.text = qsTr("Cards from Shape"); newProjectDialog.open() } }
                ShapeButton { objectName: "deckReturnOutputs"; visible: !window.exchange.needsImport; text: qsTr("Return outputs"); primary: true; enabled: window.exchange.outputCount > 0 && !cards.dirty && window.cardKit.currentAccepted && window.exchange.outputRevision === window.cardKit.source.revision_id; onClicked: if (cards.flush()) window.exchange.finish(window.cardKit.source) }
                ShapeButton { text: qsTr("Cancel return"); onClicked: window.exchange.cancel() }
            }
        }
        Label {
            Layout.fillWidth: true; Layout.margins: 10
            visible: text.length > 0
            text: window.documents.error || window.exchange.error
            color: Theme.danger; wrapMode: Text.WordWrap
        }
        CardKitWorkspace {
            id: cards
            Layout.fillWidth: true; Layout.fillHeight: true
            visible: window.backend.projectOpen
            backend: window.backend; controller: window.cardKit; cardPrint: window.cardPrint
            materialEdit: window.materialEdit; audioPreview: null; imageTask: window.imageTask
            assistant: window.assistant
            assistantLanguage: window.preferences.languageMode === "zh_CN" || (window.preferences.languageMode === "system" && Qt.locale().name.startsWith("zh")) ? "zh_CN" : "en"
            onAssistantSettingsRequested: settingsDialog.open()
            onProjectsRequested: window.showProjects()
            onFileExported: (path, kind, source) => window.exchange.recordOutput(path, kind, source)
        }
        ColumnLayout {
            visible: !window.backend.projectOpen
            Layout.alignment: Qt.AlignHCenter | Qt.AlignVCenter
            Layout.maximumWidth: 640; Layout.margins: 36; spacing: 18
            Label { text: qsTr("Make cards. Arrange them. Print."); font.pixelSize: 30; font.weight: Font.DemiBold; color: Theme.text; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Label { text: qsTr("Start with complete card images, a list of words, or your own mix of text and pictures. Choose the card shape and print size when you need them."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 16; Layout.fillWidth: true }
            RowLayout {
                ShapeButton { objectName: "deckWelcomeNew"; iconSource: "qrc:/qt/qml/Shape/Desktop/icons/add.svg"; text: qsTr("New card project…"); primary: true; enabled: !window.exchange.active; onClicked: newProjectDialog.open() }
                ShapeButton { iconSource: "qrc:/qt/qml/Shape/Desktop/icons/open.svg"; text: qsTr("Open a Deck project…"); enabled: !window.exchange.active; onClicked: window.showProjects() }
            }
            Label { text: qsTr("Bringing cards from Shape? In Shape, choose Continue in Deck to copy the card set and its materials here."); color: Theme.muted; wrapMode: Text.WordWrap; Layout.fillWidth: true }
        }
        RowLayout {
            Layout.fillWidth: true; Layout.leftMargin: 16; Layout.rightMargin: 16; Layout.bottomMargin: 8
            visible: window.imageTask.running || window.imageTask.resultReady || window.imageTask.error.length > 0
            Label {
                Layout.fillWidth: true
                text: window.imageTask.error || window.imageTask.status
                color: window.imageTask.error.length ? Theme.danger : Theme.muted; wrapMode: Text.WordWrap
            }
            ShapeButton {
                objectName: "deckOpenImageTarget"
                text: qsTr("Open original card")
                visible: window.imageTask.target.card_id !== undefined && window.imageTask.target.card_id.length > 0
                onClicked: {
                    if (!window.openDocument(window.imageTask.projectPath, window.imageTask.target.kit_id)) return
                    cards.selectedCardId = window.imageTask.target.card_id
                    cards.openArtworkGeneration()
                }
            }
        }
    }
    ShapeDialog {
        id: newProjectDialog
        objectName: "deckNewProjectDialog"
        parent: Overlay.overlay; anchors.centerIn: parent; width: Math.min(540, parent.width - 40)
        modal: true; title: window.exchange.needsImport ? qsTr("Save cards from Shape") : qsTr("New Deck project")
        property url folder: ""
        contentItem: ColumnLayout {
            spacing: 12
            ShapeTextField { id: newName; objectName: "deckProjectName"; Layout.fillWidth: true; placeholderText: qsTr("Project name") }
            ShapeButton { text: qsTr("Choose project folder…"); onClicked: newFolder.open() }
            Label { Layout.fillWidth: true; text: newProjectDialog.folder.toString(); color: Theme.muted; wrapMode: Text.WrapAnywhere }
            Label { Layout.fillWidth: true; visible: text.length > 0; text: window.documents.error || window.exchange.error; color: Theme.danger; wrapMode: Text.WordWrap }
            RowLayout {
                Layout.fillWidth: true
                ShapeButton { text: qsTr("Cancel"); onClicked: newProjectDialog.close() }
                Item { Layout.fillWidth: true }
                ShapeButton { objectName: "deckCreateProject"; text: qsTr("Create project"); primary: true; enabled: newName.text.trim().length > 0 && newProjectDialog.folder.toString().length > 0
                    onClicked: {
                        if (!cards.flush()) return
                        if (window.exchange.needsImport) {
                            if (!window.exchange.importTo(newProjectDialog.folder, newName.text)) return
                            cards.openWorkspace(window.exchange.kitId)
                        } else {
                            if (!window.documents.createProject(newProjectDialog.folder, newName.text)) return
                            cards.openWorkspace("")
                        }
                        newProjectDialog.close()
                    }
                }
            }
        }
    }
    FolderDialog { id: newFolder; title: qsTr("Choose a folder for the project"); onAccepted: newProjectDialog.folder = selectedFolder }
    FolderDialog { id: openFolder; title: qsTr("Open a Deck project folder"); onAccepted: { if (cards.flush() && window.documents.openProject(selectedFolder)) { cards.openWorkspace(""); projectsDialog.close() } } }
    ShapeDialog {
        id: projectsDialog
        parent: Overlay.overlay; anchors.centerIn: parent; width: Math.min(680, parent.width - 40); height: Math.min(520, parent.height - 40)
        modal: true; title: qsTr("Open a Deck project")
        contentItem: ColumnLayout {
            ShapeButton { text: qsTr("Choose a project folder…"); onClicked: openFolder.open() }
            ListView {
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                model: window.documents.recentProjects
                delegate: ItemDelegate { required property string modelData; width: ListView.view.width; text: modelData; onClicked: if (window.openDocument(modelData, "")) projectsDialog.close() }
            }
            Label { Layout.fillWidth: true; text: qsTr("Mixed Shape projects must be transferred from Shape so other workflows remain unchanged."); color: Theme.muted; wrapMode: Text.WordWrap }
            ShapeButton { Layout.alignment: Qt.AlignRight; text: qsTr("Close"); onClicked: projectsDialog.close() }
        }
    }
    ShapeDialog {
        id: settingsDialog
        objectName: "deckSettingsDialog"
        parent: Overlay.overlay; anchors.centerIn: parent; width: Math.min(650, parent.width - 40); height: Math.min(740, parent.height - 40)
        modal: true; title: qsTr("Deck settings")
        contentItem: ScrollView {
            id: settingsScroll; contentWidth: availableWidth; clip: true
            ColumnLayout {
            width: settingsScroll.availableWidth; spacing: 12
            Label { text: qsTr("Language"); color: Theme.text }
            ShapeComboBox { objectName: "deckLanguage"; Layout.fillWidth: true; model: [qsTr("Follow system"), "English", "简体中文"]; selectedIndex: window.preferences.languageMode === "zh_CN" ? 2 : window.preferences.languageMode === "en" ? 1 : 0; onActivated: index => window.preferences.languageMode = ["system", "en", "zh_CN"][index] }
            Label { text: qsTr("Appearance"); color: Theme.text }
            ShapeComboBox { objectName: "deckAppearance"; Layout.fillWidth: true; model: [qsTr("Follow system"), qsTr("Light"), qsTr("Dark")]; selectedIndex: window.preferences.appearanceMode; onActivated: index => window.preferences.appearanceMode = index }
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
            Label { text: qsTr("Card text · Infer Runtime"); color: Theme.text; font.weight: Font.DemiBold }
            Label { Layout.fillWidth: true; text: qsTr("Generate card text with local models. Deck uses its own access; requests stay local with no cloud fallback."); color: Theme.muted; wrapMode: Text.WordWrap }
            Label { Layout.fillWidth: true; text: window.assistant.credentialFile.length ? qsTr("Deck credential selected") : qsTr("Connect a Deck credential from Infer Runtime Apps & Access."); color: Theme.muted; wrapMode: Text.WordWrap }
            RowLayout {
                ShapeButton { objectName: "deckInferCredential"; iconSource: "qrc:/qt/qml/Shape/Desktop/icons/open.svg"; text: qsTr("Connect access…"); enabled: !window.assistant.busy; onClicked: inferCredential.open() }
                ShapeButton { objectName: "deckInferCheck"; text: qsTr("Check connection"); enabled: !window.assistant.busy; busy: window.assistant.busy; onClicked: window.assistant.probe() }
            }
            Label { Layout.fillWidth: true; text: window.assistant.error || window.assistant.status; color: window.assistant.error.length ? Theme.danger : Theme.muted; wrapMode: Text.WordWrap }
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
            Label { Layout.fillWidth: true; text: qsTr("Choose Shape to create images. Model and generation access are configured in Shape."); color: Theme.muted; wrapMode: Text.WordWrap }
            Label { Layout.fillWidth: true; text: window.documents.shapeExecutable || qsTr("Shape has not been selected."); color: Theme.muted; wrapMode: Text.WrapAnywhere }
            RowLayout { ShapeButton { text: qsTr("Choose Shape…"); onClicked: shapeExecutable.open() } ShapeButton { text: qsTr("Open Shape"); onClicked: window.documents.openShape() } }
            Label { Layout.fillWidth: true; text: window.materialEdit.shadowExecutable; color: Theme.muted; wrapMode: Text.WrapAnywhere }
            ShapeButton { text: qsTr("Choose Shadow…"); onClicked: shadowExecutable.open() }
            ShapeButton { objectName: "deckSettingsDone"; Layout.alignment: Qt.AlignRight; quiet: true; text: qsTr("Done"); onClicked: settingsDialog.close() }
            }
        }
    }
    FileDialog { id: shapeExecutable; title: qsTr("Choose Shape application or executable"); onAccepted: window.documents.setShapeExecutable(selectedFile) }
    FileDialog { id: inferCredential; title: qsTr("Choose Deck's Infer Runtime credential file"); onAccepted: if (window.assistant.setCredentialFile(selectedFile)) window.assistant.probe() }
    FileDialog { id: shadowExecutable; title: qsTr("Choose Shadow application or executable"); onAccepted: window.materialEdit.setShadowExecutable(selectedFile) }
}
