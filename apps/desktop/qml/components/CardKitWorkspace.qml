pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop
import "CardListImport.js" as CardListImport

Page {
    id: root
    objectName: "cardKitWorkspace"
    required property DesktopBackend backend
    required property CardKitController controller
    required property CardPrintController cardPrint
    required property ExternalImageTaskController imageTask
    required property MaterialEditController materialEdit
    required property AudioPreviewController audioPreview
    property var composition: ({cards: []})
    property string selectedCardId: ""
    property bool dirty: false
    property string sessionProject: ""
    property string sessionPath: ""
    property string localError: ""
    property string artworkCardId: ""
    property string artworkProject: ""
    property string artworkPath: ""
    property string artworkKitId: ""
    property string artworkArtifactId: ""
    property string artworkDraftId: ""
    property string artworkCandidateId: ""
    property var artworkOriginal: null
    property var editingStage: ({})
    property bool editingArtwork: false
    readonly property var cards: composition.cards || []
    readonly property var currentCard: cards.find(c => c.id === selectedCardId) || null
    readonly property var images: [{name: qsTr("Choose accepted artwork…"), id: ""}].concat(
        backend.artifacts.filter(a => a.kindKey === "image_raster" && a.hasAcceptedRevision))
    readonly property var savedKits: {
        const result = [{name: qsTr("Continue saved cards…"), id: ""}]
        const seen = {}
        for (const item of controller.drafts) {
            result.push({name: item.composition.name, id: item.artifact_id})
            seen[item.artifact_id] = true
        }
        for (const item of backend.artifacts)
            if (item.kindKey === "card_kit" && item.hasAcceptedRevision && !seen[item.id])
                result.push({name: item.name, id: item.id})
        return result
    }
    readonly property var artworkCandidate: backend.candidates.find(c =>
        c.id === artworkCandidateId && c.artifactId === artworkArtifactId) || null
    readonly property bool artworkTargetExists: artworkProject === backend.projectId
        && artworkPath === backend.bundlePath
        && artworkKitId === (controller.draft.artifact_id || "")
        && cards.some(c => c.id === artworkCardId)
    readonly property bool artworkStageCurrent: !editingArtwork || (artworkOriginal !== null
        && backend.artifacts.some(a => a.id === artworkOriginal.artifact_id && a.acceptedRevisionId === artworkOriginal.revision_id)
        && backend.operatorDrafts.some(d => d.id === artworkDraftId && d.contextArtifactId === artworkArtifactId
            && d.operatorTypeKey === "image.external_edit" && d.inputArtifactId === artworkOriginal.artifact_id
            && d.inputRevisionId === artworkOriginal.revision_id))
    property string artworkCandidateSource: ""
    property bool listApplied: false
    property bool listFirstHeader: false
    property int cardIdSequence: 1
    property bool moreText: false
    property bool showHiddenText: false
    property bool showArtPrompt: false
    property int inspectorTab: 0
    property bool layoutForSet: false
    readonly property var currentPreview: (controller.preview.cards || []).find(c => c.id === selectedCardId) || ({})
    readonly property var currentLayout: currentPreview.resolved_layout || ({})
    readonly property var hiddenContent: currentPreview.hidden_content || ({artwork: false, text_fields: []})
    readonly property bool imageCropLimited: (controller.preview.cards || []).some(card => (card.issues || []).indexOf("image_crop_limit") >= 0)
    readonly property string imageCropMessage: qsTr("Some images are too narrow or wide for edge cropping. Choose Show the complete image, or use artwork with more suitable proportions.")
    readonly property var listPreview: CardListImport.parse(listText.text, composition, listFirstHeader)
    signal shapeRequested(string instruction)
    signal projectsRequested()
    signal fileExported(string path, string kind, var source)
    signal opened()
    signal closeRequested()
    function open() { opened(); return true }
    function close() { if (!flush()) return false; closeRequested(); return true }

    padding: 0
    title: qsTr("Make cards")
    background: Rectangle { color: Theme.window }

    function menuIdentity(includeCard) { return JSON.stringify([backend.projectId, backend.bundlePath, controller.draft.artifact_id || "", includeCard ? selectedCardId : ""]) }
    function copy(value) { return JSON.parse(JSON.stringify(value)) }
    function pin(item) { return {artifact_id: item.id, revision_id: item.acceptedRevisionId} }
    function synchronize() {
        composition = copy(controller.draft.composition || {cards: []})
        if (!cards.some(c => c.id === selectedCardId)) selectedCardId = cards.length ? cards[0].id : ""
        dirty = false
    }
    function changed(next) {
        composition = next; dirty = true; localError = ""
        controller.invalidateReview(); saveTimer.restart()
    }
    function setField(key, value) {
        if (composition[key] === value) return
        const next = copy(composition); next[key] = value; changed(next)
    }
    function setCardField(key, value) {
        if (!currentCard || currentCard[key] === value) return
        if (key === "layout" && value && !composition.layout) {
            localError = qsTr("Choose a flexible default layout for this set before changing a card’s layout."); return
        }
        const next = copy(composition), index = next.cards.findIndex(c => c.id === selectedCardId)
        next.cards[index][key] = value
        if (key === "rules") next.cards[index].text_material = null
        changed(next)
    }
    function flush() {
        saveTimer.stop()
        if (!dirty) return true
        if (!controller.save(composition)) return false
        dirty = false; if (visible) previewTimer.restart(); return true
    }
    function openWorkspace(artifactId) {
        if (sessionProject !== backend.projectId || sessionPath !== backend.bundlePath) {
            sessionProject = backend.projectId; sessionPath = backend.bundlePath; dirty = false; composition = ({cards: []})
            selectedCardId = ""; clearArtwork()
        }
        if (!flush()) return
        controller.refresh()
        if (artifactId && artifactId.length) {
            if (!controller.load(artifactId)) return
        } else if (!controller.draft.artifact_id) {
            if (savedKits.length > 1) { if (!controller.load(savedKits[1].id)) return }
            else if (!controller.beginNew()) return
        }
        synchronize(); resumeArtworkEdit(); controller.showCard(selectedCardId); open()
    }
    function newKit() {
        if (flush() && controller.beginNew()) { clearArtwork(); synchronize(); controller.showCard(selectedCardId) }
    }
    function requestReloadDraft() { if (dirty) reloadConfirmation.open(); else reloadSavedDraft() }
    function reloadSavedDraft() {
        saveTimer.stop()
        if (controller.load(controller.draft.artifact_id)) { clearArtwork(); synchronize(); resumeArtworkEdit(); controller.showCard(selectedCardId) }
    }
    function selectKit(artifactId) {
        if (artifactId && flush() && controller.load(artifactId)) { clearArtwork(); synchronize(); resumeArtworkEdit(); controller.showCard(selectedCardId) }
    }
    function addCard() {
        const next = copy(composition)
        const id = nextCardId(new Set(next.cards.map(c => c.id)))
        next.cards.push({id: id, title: "", card_type: "", cost: "", rules: "", stats: "", flavor: "", art_prompt: "", quantity: 1, artwork: null, text_material: null})
        changed(next); selectedCardId = id
    }
    function nextCardId(ids) {
        let id
        do { id = "card-" + Date.now().toString(36) + "-" + (cardIdSequence++).toString(36) } while (ids.has(id))
        return id
    }
    function openCardList() {
        if (!flush()) return
        listApplied = false; listFirstHeader = false; listText.text = ""; listDialog.open(); listText.focusEditor()
    }
    function cardListError() {
        const result = listPreview
        switch (result.error) {
        case "empty": return qsTr("Paste at least one card title or text to begin.")
        case "too_large": case "composition_size": return qsTr("This list is too large for these cards. Add a smaller group.")
        case "too_many_cards": return qsTr("A set can contain up to 64 cards. Add fewer rows or start a new set.")
        case "columns": return qsTr("Row %1 has more than three columns. Use title, text and image description.").arg(result.line)
        case "content": return qsTr("Row %1 needs a title or text. An image description alone is not printable content.").arg(result.line)
        case "heading": return qsTr("Use title, text or image description as column headings, or turn off the heading option.")
        case "field_length": return qsTr("Row %1 has too much text. Shorten its title, rules or artwork description.").arg(result.line)
        case "quote": return qsTr("Check the quotation marks around row %1. Copying the cells again may help.").arg(result.line)
        case "invalid_text": return qsTr("Row %1 contains unsupported text. Remove hidden control characters and try again.").arg(result.line)
        default: return ""
        }
    }
    function appendCardList() {
        const result = CardListImport.parse(listText.text, composition, listFirstHeader)
        if (listApplied || result.error.length || !result.rows.length) return false
        const next = copy(composition)
        next.cards = copy(CardListImport.effectiveCards(composition))
        const ids = new Set(next.cards.map(c => c.id))
        for (const row of result.rows) {
            const id = nextCardId(ids)
            ids.add(id)
            next.cards.push({id: id, title: row.title, rules: row.rules, art_prompt: row.art_prompt,
                card_type: "", cost: "", stats: "", flavor: "", quantity: 1, artwork: null, text_material: null})
        }
        listApplied = true; changed(next); selectedCardId = next.cards[next.cards.length - result.rows.length].id
        flush(); listDialog.close(); return true
    }
    function moveCard(delta) {
        const next = copy(composition), index = next.cards.findIndex(c => c.id === selectedCardId), to = index + delta
        if (index < 0 || to < 0 || to >= next.cards.length) return
        next.cards.splice(to, 0, next.cards.splice(index, 1)[0]); changed(next)
    }
    function removeCard() {
        if (cards.length < 2) return
        const next = copy(composition), index = next.cards.findIndex(c => c.id === selectedCardId)
        next.cards.splice(index, 1); changed(next)
        selectedCardId = next.cards[Math.min(index, next.cards.length - 1)].id
    }
    function cardState(card) {
        if (!card) return ""
        if (artworkTargetExists && editingArtwork && card.id === artworkCardId)
            return artworkCandidate ? qsTr("Review returned artwork") : qsTr("Artwork edit in progress")
        const state = (controller.preview.cards || []).find(item => item.id === card.id)
        const issues = state ? state.issues || [] : []
        if (issues.indexOf("image_crop_limit") >= 0) return qsTr("Use the full image or artwork with different proportions")
        if (issues.indexOf("layout_overflow") >= 0) return qsTr("Shorten text or change the layout")
        if (issues.indexOf("empty_content") >= 0) return qsTr("Add an image or text")
        if (issues.indexOf("missing_artwork") >= 0) return qsTr("Add an image")
        if (issues.indexOf("missing_text") >= 0 || issues.indexOf("missing_title") >= 0 || issues.indexOf("missing_body") >= 0) return qsTr("Add text")
        if (state && state.content_ready) return qsTr("Ready")
        return qsTr("Ready to check")
    }
    function cardName(card) {
        if ((card.title || "").trim().length) return card.title
        if ((card.rules || "").trim().length) return card.rules.trim().split("\n")[0]
        const image = card.artwork ? backend.artifacts.find(item => item.id === card.artwork.artifact_id) : null
        return image ? image.name : qsTr("Untitled card")
    }
    function cardNeedsAttention(card) {
        const state = (controller.preview.cards || []).find(item => item.id === card.id)
        return !!state && !state.content_ready && (state.issues || []).length > 0
    }
    function hiddenDescription() {
        const names = []
        if (hiddenContent.artwork) names.push(qsTr("image"))
        const labels = {title: qsTr("title"), rules: qsTr("body text"), text_material: qsTr("accepted text"),
            card_type: qsTr("type"), cost: qsTr("cost"), stats: qsTr("stats"), flavor: qsTr("additional text")}
        for (const key of hiddenContent.text_fields || []) if (labels[key]) names.push(labels[key])
        return names.length ? qsTr("This layout hides %1. The content is kept.").arg(names.join(" · ")) : ""
    }
    function removeArtwork() {
        if (!currentCard || !currentCard.artwork) return false
        const cardId = selectedCardId
        setCardField("artwork", null)
        if (artworkCardId === cardId) { clearArtwork(); controller.forgetArtworkEdit(cardId) }
        return flush()
    }
    function setCanvas(width, height) {
        const next = copy(composition)
        if (next.canvas_width === width && next.canvas_height === height) return
        next.canvas_width = width; next.canvas_height = height
        if (!next.layout) next.layout = {mode: "auto"}
        changed(next)
    }
    function beginCardImages() {
        if (!flush()) return false
        if (!composition.layout) {
            localError = qsTr("Choose a flexible layout or start a new set before adding complete card images. The saved cards keep their original layout until you choose.")
            setLayoutDialog.open(); return false
        }
        return imageIntake.chooseFiles()
    }
    function canAppendImage(cardId) {
        return !!composition.layout && (cards.some(card => card.id === cardId) || CardListImport.effectiveCards(composition).length < 64)
    }
    function appendImportedCard(source, cardId, width, height, useShape, projectId, projectPath, kitId, batchOrder) {
        if (projectId !== backend.projectId || projectPath !== backend.bundlePath || kitId !== controller.draft.artifact_id)
            return {ok: false, message: qsTr("The project or card set changed. Reopen the image list.")}
        const existing = cards.find(card => card.id === cardId)
        if (existing) {
            if (JSON.stringify(existing.artwork) !== JSON.stringify(source))
                return {ok: false, message: qsTr("This card changed. Keep its current content and reopen the image list.")}
            return {ok: flush(), message: controller.error}
        }
        if (!canAppendImage(cardId)) return {ok: false, message: qsTr("This set already contains 64 cards.")}
        const next = copy(composition)
        next.cards = copy(CardListImport.effectiveCards(composition))
        let notice = ""
        if (useShape && next.cards.length === 0 && width > 0 && height > 0) {
            if (width / height < 0.1 || width / height > 10) {
                notice = qsTr("This image is unusually narrow or wide. The set’s proportions were kept, and the complete image will fit inside the card.")
            } else {
                const scale = Math.min(2400 / Math.max(width, height), Math.max(1, 240 / Math.min(width, height)))
                next.canvas_width = Math.round(width * scale)
                next.canvas_height = Math.round(height * scale)
            }
        }
        const card = {id: cardId, title: "", card_type: "", cost: "", rules: "", stats: "", flavor: "", art_prompt: "",
            quantity: 1, artwork: source, text_material: null, layout: {mode: "image", image_fit: "contain", padding_percent: 0, border: false}}
        const later = (batchOrder || []).slice((batchOrder || []).indexOf(cardId) + 1)
        const insertion = next.cards.findIndex(item => later.indexOf(item.id) >= 0)
        next.cards.splice(insertion < 0 ? next.cards.length : insertion, 0, card)
        changed(next); selectedCardId = cardId
        const saved = flush()
        if (saved && notice.length) localError = notice
        return {ok: saved, message: saved ? notice : controller.error}
    }
    function acceptAndPrint() {
        if (!flush()) return false
        previewTimer.stop()
        let acceptedNow = false
        if (!controller.currentAccepted) {
            if (!controller.review(selectedCardId)) return false
            if (imageCropLimited) { localError = imageCropMessage; return false }
            if ((controller.preview.cards || []).some(card => (card.issues || []).some(issue => issue !== "layout_overflow"))) {
                localError = qsTr("Complete the content needed by each card’s layout before printing.")
                return false
            }
            if (!controller.accept()) return false
            acceptedNow = true
        }
        const opened = printDialog.openForSource(controller.source, controller.preview.print_defaults)
        if (!opened && acceptedNow) localError = qsTr("The card version was accepted. Adjust the print layout in the opened window to continue.")
        return opened
    }
    function saveVersion() {
        if (!flush() || !controller.review(selectedCardId)) return false
        return controller.accept()
    }
    function exportAcceptedFiles() {
        if (!flush() || !controller.canExport) return false
        zipExport.projectId = backend.projectId; zipExport.projectPath = backend.bundlePath
        zipExport.sourcePin = copy(controller.source); zipExport.open(); return true
    }
    function openSetLayout() { setLayoutDialog.open() }
    function artworkName() {
        if (!currentCard || !currentCard.artwork) return qsTr("Add an image")
        const artwork = backend.artifacts.find(item => item.id === currentCard.artwork.artifact_id)
        return artwork ? artwork.name : qsTr("Selected image")
    }
    function artworkIndex() {
        if (!currentCard || !currentCard.artwork) return 0
        return Math.max(0, images.findIndex(a => a.id === currentCard.artwork.artifact_id))
    }
    function bindArtwork(cardId, source) {
        if (sessionProject !== backend.projectId || sessionPath !== backend.bundlePath) return false
        const next = copy(composition), index = next.cards.findIndex(c => c.id === cardId)
        if (index < 0) return false
        next.cards[index].artwork = source; changed(next); return flush()
    }
    function chooseArtwork(index) {
        if (!currentCard || index < 1 || index >= images.length) return
        bindArtwork(selectedCardId, pin(images[index]))
    }
    function clearArtwork() {
        artworkCardId = ""; artworkProject = ""; artworkPath = ""; artworkKitId = ""; artworkArtifactId = ""
        artworkDraftId = ""; artworkCandidateId = ""; artworkCandidateSource = ""
        artworkOriginal = null; editingStage = ({}); editingArtwork = false
    }
    function openLargePreview() {
        if (!controller.previewSource.length) return false
        largePreview.svgSource = controller.previewSource
        largePreview.canvasWidth = currentLayout.canvas_width || composition.canvas_width || 750
        largePreview.canvasHeight = currentLayout.canvas_height || composition.canvas_height || 1050
        largePreview.cardTitle = currentCard ? cardName(currentCard) : ""
        largePreview.actualSize = true
        largePreview.open(); return true
    }
    function beginImageImport() {
        if (!currentCard || !flush()) return
        imageImport.cardId = selectedCardId; imageImport.projectId = backend.projectId
        imageImport.kitId = controller.draft.artifact_id; imageImport.projectPath = backend.bundlePath; imageImport.open()
    }
    function importArtwork(file, cardId, projectId, kitId, projectPath) {
        if (projectId !== backend.projectId || projectPath !== backend.bundlePath
            || kitId !== controller.draft.artifact_id || !cards.some(c => c.id === cardId)) return false
        const previous = new Set(backend.artifacts.map(a => a.id))
        if (!backend.importRaster(file)) return false
        const added = backend.artifacts.filter(a => !previous.has(a.id)
            && a.kindKey === "image_raster" && a.hasAcceptedRevision)
        if (added.length === 1) return bindArtwork(cardId, pin(added[0]))
        localError = qsTr("The image was imported. Choose it from accepted artwork to add it to this card.")
        return false
    }
    function openArtworkGeneration() {
        if (!currentCard || !flush()) return false
        shapePrompt.text = imageTask.target.card_id === selectedCardId && imageTask.target.kit_id === controller.draft.artifact_id
            && imageTask.target.project_id === backend.projectId && imageTask.prompt.length ? imageTask.prompt
            : [composition.brief, composition.style, currentCard.title, currentCard.art_prompt].filter(t => t && t.trim().length).join("\n\n")
        shapeTaskDialog.cardId = selectedCardId
        shapeTaskDialog.projectId = backend.projectId
        shapeTaskDialog.projectPath = backend.bundlePath
        shapeTaskDialog.kitId = controller.draft.artifact_id
        shapeTaskDialog.open()
        return true
    }
    function useShapeImage() {
        const target = imageTask.target
        if (!flush() || !imageTask.resultReady || !imageTask.targetCurrent
            || target.project_id !== backend.projectId || target.kit_id !== controller.draft.artifact_id
            || target.card_id !== selectedCardId) return false
        const source = imageTask.importReturnedImage()
        if (!source.artifact_id || !bindArtwork(target.card_id, source)) return false
        imageTask.consumed(); shapeTaskDialog.close(); return true
    }


    function beginArtworkEdit() {
        if (!currentCard || !currentCard.artwork || !flush()) return false
        const image = backend.artifacts.find(a => a.id === currentCard.artwork.artifact_id)
        if (!image || image.acceptedRevisionId !== currentCard.artwork.revision_id) {
            localError = qsTr("Choose the current accepted artwork explicitly before editing it."); return false
        }
        if (!editingArtwork || artworkCardId !== selectedCardId || !artworkTargetExists
            || JSON.stringify(artworkOriginal) !== JSON.stringify(currentCard.artwork)) {
            if (!restoreArtworkEdit(selectedCardId)) {
                const id = backend.beginOperatorDraft(image.id, "image.external_edit")
                const draft = backend.operatorDrafts.find(d => d.id === id)
                if (!draft || draft.inputRevisionId !== image.acceptedRevisionId) return false
                clearArtwork(); artworkCardId = selectedCardId; artworkProject = backend.projectId; artworkPath = backend.bundlePath; artworkKitId = controller.draft.artifact_id
                artworkOriginal = copy(currentCard.artwork); artworkArtifactId = draft.contextArtifactId
                artworkDraftId = id; editingStage = draft; editingArtwork = true
                if (!controller.rememberArtworkEdit(artworkCardId, artworkOriginal, draft))
                    localError = qsTr("The working copy is open. Its return location could not be saved for reopening Deck.")
            }
        }
        materialEdit.restoreShadowSession(backend.bundlePath, artworkArtifactId)
        refinementDialog.open(); return true
    }
    function restoreArtworkEdit(cardId) {
        const record = controller.recoverArtworkEdit(cardId)
        if (!record.stage || !record.stage.id) return false
        clearArtwork(); artworkCardId = cardId; artworkProject = backend.projectId; artworkPath = backend.bundlePath; artworkKitId = controller.draft.artifact_id
        artworkOriginal = copy(record.source); artworkArtifactId = record.stage.contextArtifactId
        artworkDraftId = record.stage.id; editingStage = record.stage; editingArtwork = true
        const candidate = backend.candidates.find(c => c.artifactId === artworkArtifactId)
        artworkCandidateId = candidate ? candidate.id : ""
        materialEdit.restoreShadowSession(backend.bundlePath, artworkArtifactId)
        return true
    }
    function resumeArtworkEdit() {
        for (const card of cards) {
            if (restoreArtworkEdit(card.id)) { selectedCardId = card.id; return true }
        }
        return false
    }
    function useArtwork() {
        if (!artworkTargetExists || !artworkStageCurrent || !artworkCandidate || materialEdit.running || materialEdit.shadowActive) return false
        const target = cards.find(c => c.id === artworkCardId)
        if (editingArtwork && JSON.stringify(target.artwork) !== JSON.stringify(artworkOriginal)) return false
        if (!backend.acceptCandidate(artworkCandidate.id)) return false
        const accepted = backend.artifacts.find(a => a.id === artworkArtifactId && a.hasAcceptedRevision)
        if (!accepted || !bindArtwork(artworkCardId, pin(accepted))) return false
        if (editingArtwork) controller.forgetArtworkEdit(artworkCardId)
        selectedCardId = artworkCardId; refinementDialog.close(); clearArtwork(); return true
    }
    function updateArtworkPreview() {
        if (artworkTargetExists && artworkCandidate && backend.prepareImagePreviews(artworkArtifactId, artworkCandidate.id))
            artworkCandidateSource = backend.candidateImageSource
        else artworkCandidateSource = ""
    }
    onArtworkCandidateChanged: updateArtworkPreview()
    onSelectedCardIdChanged: {
        showHiddenText = false
        imageMenu.close(); cardActions.close()
        if (!dirty && selectedCardId.length) controller.showCard(selectedCardId)
    }
    function resetInspectorScroll() {
        if (inspectorScroll.contentItem && inspectorScroll.contentItem.contentY !== undefined)
            inspectorScroll.contentItem.contentY = 0
    }
    onInspectorTabChanged: Qt.callLater(resetInspectorScroll)
    onLayoutForSetChanged: Qt.callLater(resetInspectorScroll)
    Connections {
        target: root.controller
        function onDocumentChanged() { root.synchronize() }
    }
    Connections {
        target: root.backend
        function onProjectChanged() {
            if (root.sessionProject !== root.backend.projectId || root.sessionPath !== root.backend.bundlePath) {
                addMenu.close(); imageMenu.close(); cardActions.close()
                saveTimer.stop(); previewTimer.stop(); root.dirty = false; root.close()
                shapeTaskDialog.close(); refinementDialog.close(); root.clearArtwork()
                listDialog.close(); largePreview.close(); imageImport.close(); zipExport.close(); printDialog.close(); setLayoutDialog.close()
            }
        }
    }

    Connections {
        target: root.materialEdit
        function onCandidateCreated(candidateId, artifactId) {
            if (root.artworkTargetExists && root.editingArtwork && artifactId === root.artworkArtifactId)
                root.artworkCandidateId = candidateId
        }
    }
    Timer { id: saveTimer; interval: 500; onTriggered: root.flush() }
    Timer { id: previewTimer; interval: 200; onTriggered: if (root.visible && !root.dirty && root.selectedCardId.length) root.controller.showCard(root.selectedCardId) }
    FileDialog {
        id: imageImport
        property string cardId: ""
        property string projectId: ""
        property string projectPath: ""
        property string kitId: ""
        title: qsTr("Choose card artwork")
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg)")]
        onAccepted: {
            root.importArtwork(selectedFile, cardId, projectId, kitId, projectPath)
        }
    }
    FileDialog {
        id: zipExport
        property string projectId: ""
        property string projectPath: ""
        property var sourcePin: ({})
        title: qsTr("Export accepted card files")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "zip"
        nameFilters: [qsTr("Card material archive (*.zip)")]
        onAccepted: root.exportKitFiles(selectedFile, projectId, projectPath, sourcePin)
    }
    contentItem: ColumnLayout {
        spacing: 0
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0
            Rectangle {
                Layout.preferredWidth: root.width < 1180 ? 184 : 204
                Layout.fillHeight: true
                color: Theme.chrome
                ColumnLayout {
                    anchors.fill: parent; anchors.margins: 14; spacing: 14
                    RowLayout {
                        Layout.fillWidth: true
                        Label { Layout.fillWidth: true; text: qsTr("Cards"); font.pixelSize: 13; font.weight: Font.DemiBold; color: Theme.text }
                        Label { text: root.cards.length; font.pixelSize: 12; color: Theme.muted }
                        ToolButton {
                            id: addButton
                            objectName: "cardKitAddMenuButton"
                            text: qsTr("+ Add"); font.pixelSize: 12; implicitWidth: 66; implicitHeight: 30
                            Accessible.name: qsTr("Add cards")
                            onClicked: { addMenu.targetIdentity = root.menuIdentity(false); addMenu.popup(addButton, 0, addButton.height) }
                            background: Rectangle { radius: 7; color: addButton.hovered ? Theme.surfaceSubtle : "transparent" }
                        }
                    }
                    ListView {
                        id: cardList
                        objectName: "cardKitCardList"
                        Layout.fillWidth: true; Layout.fillHeight: true
                        model: root.cards; spacing: 4; clip: true
                        currentIndex: root.cards.findIndex(c => c.id === root.selectedCardId)
                        function keepCurrentVisible() { if (currentIndex >= 0 && height > 0) positionViewAtIndex(currentIndex, ListView.Contain) }
                        onCurrentIndexChanged: Qt.callLater(keepCurrentVisible)
                        onCountChanged: Qt.callLater(keepCurrentVisible)
                        onHeightChanged: Qt.callLater(keepCurrentVisible)
                        onVisibleChanged: if (visible) Qt.callLater(keepCurrentVisible)
                        delegate: ItemDelegate {
                            id: cardRow
                            objectName: "cardKitRow-" + (cardRow.card.id || "")
                            required property var modelData
                            required property int index
                            readonly property var card: modelData || ({})
                            width: ListView.view.width; height: 72; padding: 9; hoverEnabled: true
                            onClicked: if (cardRow.card.id) root.selectedCardId = cardRow.card.id
                            background: Rectangle {
                                radius: 9
                                color: root.selectedCardId === cardRow.card.id ? Theme.surfaceSelected : cardRow.hovered ? Theme.surfaceSubtle : "transparent"
                            }
                            contentItem: RowLayout {
                                spacing: 10
                                Rectangle {
                                    Layout.preferredWidth: 29; Layout.preferredHeight: 39
                                    radius: 3; color: Theme.panelRaised; border.color: Theme.border
                                    Image { anchors.fill: parent; source: root.selectedCardId === cardRow.card.id ? root.controller.previewSource : ""; fillMode: Image.PreserveAspectFit }
                                    Label { anchors.centerIn: parent; visible: root.selectedCardId !== cardRow.card.id; text: cardRow.card.artwork ? "▧" : "Aa"; font.pixelSize: 11; color: Theme.muted }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true; spacing: 6
                                    Label { Layout.fillWidth: true; text: root.cardName(cardRow.card); elide: Text.ElideRight; color: Theme.text; font.pixelSize: 12; font.weight: root.selectedCardId === cardRow.card.id ? Font.DemiBold : Font.Normal }
                                    RowLayout {
                                        Label { text: String(cardRow.index + 1).padStart(2, "0") + "  ·  ×" + (cardRow.card.quantity || 1); color: Theme.muted; font.pixelSize: 10 }
                                        Rectangle { width: 5; height: 5; radius: 3; color: Theme.warning; visible: root.cardNeedsAttention(cardRow.card) }
                                    }
                                }
                                ToolButton {
                                    objectName: "cardKitRowMenu-" + (cardRow.card.id || "")
                                    visible: cardRow.hovered || root.selectedCardId === cardRow.card.id
                                    implicitWidth: 20; implicitHeight: 28; text: "⋮"; font.pixelSize: 18
                                    Accessible.name: qsTr("Card actions")
                                    onClicked: { root.selectedCardId = cardRow.card.id; cardActions.targetIdentity = root.menuIdentity(true); cardActions.popup() }
                                    background: Rectangle { color: parent.hovered ? Theme.buttonGhostHover : "transparent"; radius: 5 }
                                }
                            }
                        }
                        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    }
                    Label { Layout.fillWidth: true; text: qsTr("%1 copies in this set").arg(root.cards.reduce((sum, card) => sum + (card.quantity || 1), 0)); font.pixelSize: 11; color: Theme.muted; wrapMode: Text.WordWrap }
                }
            }
            Rectangle { Layout.fillHeight: true; implicitWidth: 1; color: Theme.border }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0
                Rectangle {
                    Layout.fillWidth: true; implicitHeight: 72; color: Theme.canvas
                    RowLayout {
                        anchors.fill: parent; anchors.leftMargin: 30; anchors.rightMargin: 24
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 5
                            Label { Layout.fillWidth: true; text: root.currentCard ? root.cardName(root.currentCard) : qsTr("Your card"); color: Theme.text; font.pixelSize: 20; font.weight: Font.DemiBold; elide: Text.ElideRight }
                            Label { text: qsTr("Card %1 of %2").arg(Math.max(1, root.cards.findIndex(c => c.id === root.selectedCardId) + 1)).arg(root.cards.length); color: Theme.muted; font.pixelSize: 11 }
                        }
                        ShapeButton { objectName: "cardKitEnlargePreview"; quiet: true; text: qsTr("Enlarge"); enabled: root.controller.previewSource.length > 0; onClicked: root.openLargePreview() }
                    }
                }
                Rectangle {
                    id: canvas
                    objectName: "deckCardCanvas"
                    Layout.fillWidth: true; Layout.fillHeight: true
                    color: Theme.canvas
                    Rectangle {
                        anchors.centerIn: parent; anchors.verticalCenterOffset: 5
                        width: cardFace.paintedWidth + 3; height: cardFace.paintedHeight + 3
                        radius: 2; visible: cardFace.status === Image.Ready; color: Theme.shadowSoft
                    }
                    Image {
                        id: cardFace
                        objectName: "cardKitPreviewImage"
                        anchors.fill: parent; anchors.leftMargin: 34; anchors.rightMargin: 34; anchors.topMargin: 8; anchors.bottomMargin: 28
                        source: root.controller.previewSource; fillMode: Image.PreserveAspectFit; asynchronous: false
                    }
                    Label { anchors.centerIn: parent; width: parent.width - 60; visible: !root.controller.previewSource.length; text: qsTr("Your card will appear here as you edit."); wrapMode: Text.WordWrap; horizontalAlignment: Text.AlignHCenter; color: Theme.muted }
                    MouseArea { anchors.fill: cardFace; enabled: root.controller.previewSource.length > 0; cursorShape: Qt.PointingHandCursor; onDoubleClicked: root.openLargePreview() }
                }
                Rectangle {
                    Layout.fillWidth: true; implicitHeight: 38; color: Theme.canvas
                    Label { anchors.centerIn: parent; text: root.currentCard ? root.cardState(root.currentCard) : ""; color: root.currentCard && root.cardNeedsAttention(root.currentCard) ? Theme.warning : Theme.muted; font.pixelSize: 11 }
                }
            }
            Rectangle { Layout.fillHeight: true; implicitWidth: 1; color: Theme.border }
            Rectangle {
                objectName: "deckInspector"
                Layout.preferredWidth: root.width < 1180 ? 292 : 316
                Layout.fillHeight: true; color: Theme.chrome
                ColumnLayout {
                    anchors.fill: parent; spacing: 0
                    RowLayout {
                        Layout.fillWidth: true; Layout.margins: 14; spacing: 4
                        ShapeButton { objectName: "deckContentTab"; Layout.fillWidth: true; expanded: true; quiet: true; selected: root.inspectorTab === 0; text: qsTr("Content"); onClicked: root.inspectorTab = 0 }
                        ShapeButton { objectName: "deckLayoutTab"; Layout.fillWidth: true; expanded: true; quiet: true; selected: root.inspectorTab === 1; text: qsTr("Layout"); onClicked: root.inspectorTab = 1 }
                    }
                    ScrollView {
                        id: inspectorScroll
                        objectName: "deckInspectorScroll"
                        Layout.fillWidth: true; Layout.fillHeight: true
                        contentWidth: availableWidth; clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                        ColumnLayout {
                            width: inspectorScroll.availableWidth; spacing: 18
                            ColumnLayout {
                                objectName: "deckContentInspector"
                                visible: root.inspectorTab === 0
                                Layout.fillWidth: true; Layout.margins: 18; spacing: 14
                                ColumnLayout {
                                    visible: root.currentLayout.mode !== "text"
                                    Layout.fillWidth: true; spacing: 9
                                    Label { text: qsTr("Image"); color: Theme.textSoft; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    ShapeButton {
                                        id: imageMenuButton
                                        objectName: root.currentLayout.mode !== "text" ? "deckImageMenuButton" : "deckHiddenImageMenuButton"
                                        Layout.fillWidth: true; expanded: true; quiet: true
                                        iconSource: "../../icons/open.svg"; text: root.artworkName() + "  ⌄"
                                        onClicked: imageMenu.openFor(imageMenuButton)
                                    }
                                    ToolButton { objectName: "cardKitImageInstructionToggle"; text: (root.showArtPrompt ? "⌄  " : "›  ") + qsTr("Image instruction"); font.pixelSize: 11; onClicked: root.showArtPrompt = !root.showArtPrompt }
                                    ShapeTextEditor {
                                        objectName: "cardKitArtPrompt"; visible: root.showArtPrompt
                                        Layout.fillWidth: true; Layout.preferredHeight: 105
                                        text: root.currentCard ? root.currentCard.art_prompt : ""; placeholderText: qsTr("Describe this card’s image…"); wrapMode: TextEdit.Wrap
                                        onTextChanged: if (editorActiveFocus) root.setCardField("art_prompt", text)
                                    }
                                    Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
                                }
                                ToolButton {
                                    objectName: "deckHiddenTextToggle"
                                    visible: root.currentLayout.mode === "image"
                                    text: (root.showHiddenText ? "⌄  " : "›  ") + qsTr("Hidden text")
                                    font.pixelSize: 11; onClicked: root.showHiddenText = !root.showHiddenText
                                }
                                ColumnLayout {
                                    objectName: "deckTextFields"
                                    visible: root.currentLayout.mode !== "image" || root.showHiddenText
                                    Layout.fillWidth: true; spacing: 12
                                    Label { text: qsTr("Title"); color: Theme.textSoft; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    ShapeTextField { objectName: "cardKitTitle"; Layout.fillWidth: true; text: root.currentCard ? root.currentCard.title : ""; placeholderText: qsTr("Title (optional)"); onTextEdited: root.setCardField("title", text) }
                                    Label { text: qsTr("Body text"); color: Theme.textSoft; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    ShapeTextEditor {
                                        objectName: "cardKitRules"
                                        Layout.fillWidth: true; Layout.preferredHeight: root.height < 740 ? 145 : 180
                                        text: root.currentCard ? root.currentCard.rules : ""
                                        placeholderText: qsTr("A word, question, explanation…"); wrapMode: TextEdit.Wrap
                                        onTextChanged: if (editorActiveFocus) root.setCardField("rules", text)
                                    }
                                    Label { Layout.fillWidth: true; visible: root.currentCard !== null && !!root.currentCard.text_material; text: qsTr("This card uses accepted text from the project. Writing here replaces that text binding."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                                    ToolButton { objectName: "cardKitMoreText"; text: (root.moreText ? "⌄  " : "›  ") + qsTr("More text"); font.pixelSize: 11; onClicked: root.moreText = !root.moreText }
                                    ColumnLayout {
                                        visible: root.moreText; Layout.fillWidth: true; spacing: 8
                                        RowLayout {
                                            Layout.fillWidth: true
                                            ShapeTextField { Layout.fillWidth: true; text: root.currentCard ? root.currentCard.card_type : ""; placeholderText: qsTr("Type (optional)"); onTextEdited: root.setCardField("card_type", text) }
                                            ShapeTextField { Layout.preferredWidth: 75; text: root.currentCard ? root.currentCard.cost : ""; placeholderText: qsTr("Cost"); onTextEdited: root.setCardField("cost", text) }
                                        }
                                        ShapeTextField { Layout.fillWidth: true; text: root.currentCard ? root.currentCard.stats : ""; placeholderText: qsTr("Stats (optional)"); onTextEdited: root.setCardField("stats", text) }
                                        ShapeTextField { Layout.fillWidth: true; text: root.currentCard ? root.currentCard.flavor : ""; placeholderText: qsTr("Additional text (optional)"); onTextEdited: root.setCardField("flavor", text) }
                                    }
                                }
                                RowLayout {
                                    Layout.fillWidth: true
                                    Label { Layout.fillWidth: true; text: qsTr("Copies"); color: Theme.textSoft; font.pixelSize: 11 }
                                    SpinBox { objectName: "cardKitQuantity"; from: 1; to: 999; editable: true; value: root.currentCard ? root.currentCard.quantity : 1; onValueModified: root.setCardField("quantity", value); implicitWidth: 112; implicitHeight: 32 }
                                }
                                ShapeButton {
                                    id: textImageMenuButton
                                    objectName: root.currentLayout.mode === "text" ? "deckImageMenuButton" : "deckHiddenTextImageMenuButton"
                                    visible: root.currentLayout.mode === "text"
                                    Layout.fillWidth: true; expanded: true; quiet: true
                                    text: qsTr("Image options…")
                                    onClicked: imageMenu.openFor(textImageMenuButton)
                                }
                                Label { objectName: "cardKitHiddenContent"; Layout.fillWidth: true; visible: text.length > 0; text: root.hiddenDescription(); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                            }
                            ColumnLayout {
                                visible: root.inspectorTab === 1
                                Layout.fillWidth: true; Layout.margins: 18; spacing: 16
                                RowLayout {
                                    Layout.fillWidth: true
                                    ShapeButton { objectName: "deckCardLayoutScope"; Layout.fillWidth: true; expanded: true; quiet: true; selected: !root.layoutForSet; text: qsTr("This card"); onClicked: root.layoutForSet = false }
                                    ShapeButton { objectName: "cardKitSetLayout"; Layout.fillWidth: true; expanded: true; quiet: true; selected: root.layoutForSet; text: qsTr("Whole set"); onClicked: root.layoutForSet = true }
                                }
                                CardLayoutOptions {
                                    objectName: "cardKitCardLayoutOptions"
                                    visible: !root.layoutForSet
                                    Layout.fillWidth: true; perCard: true; enabled: !!root.composition.layout
                                    configuration: root.currentCard ? root.currentCard.layout || null : null
                                    resolved: root.currentLayout
                                    editIdentity: root.backend.projectId + "|" + root.backend.bundlePath + "|" + (root.controller.draft.artifact_id || "") + "|" + root.selectedCardId
                                    editingEnabled: root.visible && root.inspectorTab === 1 && !root.layoutForSet && !!root.composition.layout
                                    onEdited: value => root.setCardField("layout", value)
                                }
                                Label { Layout.fillWidth: true; visible: !root.layoutForSet && !root.composition.layout; text: qsTr("These cards use their original layout. Choose Whole set to switch to a flexible layout."); color: Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                                ColumnLayout {
                                    id: setLayoutDialog
                                    objectName: "cardKitSetLayoutDialog"
                                    visible: root.layoutForSet
                                    Layout.fillWidth: true; spacing: 12
                                    property bool promptsExpanded: false
                                    property bool customRatioExpanded: false
                                    property string ratioError: ""
                                    function open() { root.inspectorTab = 1; root.layoutForSet = true }
                                    function close() { root.inspectorTab = 0; root.layoutForSet = false }
                                    function applyCustomRatio() {
                                        let width = NaN, height = NaN
                                        try { width = Number.fromLocaleString(Qt.locale(), customWidth.text); height = Number.fromLocaleString(Qt.locale(), customHeight.text) } catch (_) {}
                                        const ratio = width / height
                                        if (!customWidth.acceptableInput || !customHeight.acceptableInput || !isFinite(width) || !isFinite(height) || width <= 0 || height <= 0 || ratio < 0.1 || ratio > 10) {
                                            ratioError = qsTr("Enter a positive width and height. Neither side can be more than ten times the other."); return false
                                        }
                                        ratioError = ""
                                        root.setCanvas(ratio >= 1 ? 2400 : Math.round(2400 * ratio), ratio >= 1 ? Math.round(2400 / ratio) : 2400)
                                        return true
                                    }
                                    Label { text: qsTr("Card set name"); color: Theme.textSoft; font.pixelSize: 11 }
                                    ShapeTextField { objectName: "cardKitName"; Layout.fillWidth: true; text: root.composition.name || ""; placeholderText: qsTr("Name this card set"); onTextEdited: root.setField("name", text) }
                                    Label { text: qsTr("Card proportions"); color: Theme.textSoft; font.pixelSize: 11 }
                                    ShapeComboBox {
                                        objectName: "cardKitShape"; Layout.fillWidth: true
                                        model: [qsTr("Keep current proportions"), qsTr("Portrait · 5:7"), qsTr("Landscape · 7:5"), qsTr("Square · 1:1"), qsTr("Landscape · 3:2"), qsTr("Portrait · 2:3")]
                                        currentIndex: {
                                            const w = root.composition.canvas_width || 750, h = root.composition.canvas_height || 1050
                                            if (w * 7 === h * 5) return 1
                                            if (w * 5 === h * 7) return 2
                                            if (w === h) return 3
                                            if (w * 2 === h * 3) return 4
                                            if (w * 3 === h * 2) return 5
                                            return 0
                                        }
                                        onActivated: index => { const sizes = [[0, 0], [750, 1050], [1050, 750], [900, 900], [1200, 800], [800, 1200]]; if (index > 0) root.setCanvas(sizes[index][0], sizes[index][1]) }
                                    }
                                    ToolButton { objectName: "cardKitCustomRatio"; text: (setLayoutDialog.customRatioExpanded ? "⌄  " : "›  ") + qsTr("Custom proportions"); font.pixelSize: 11; onClicked: setLayoutDialog.customRatioExpanded = !setLayoutDialog.customRatioExpanded }
                                    RowLayout {
                                        visible: setLayoutDialog.customRatioExpanded; Layout.fillWidth: true
                                        ShapeTextField { id: customWidth; objectName: "cardKitRatioWidth"; Layout.fillWidth: true; placeholderText: qsTr("Width"); text: "3"; validator: DoubleValidator { bottom: 0.001; top: 10000; decimals: 3; notation: DoubleValidator.StandardNotation } }
                                        Label { text: ":"; color: Theme.muted }
                                        ShapeTextField { id: customHeight; objectName: "cardKitRatioHeight"; Layout.fillWidth: true; placeholderText: qsTr("Height"); text: "2"; validator: DoubleValidator { bottom: 0.001; top: 10000; decimals: 3; notation: DoubleValidator.StandardNotation } }
                                        ShapeButton { objectName: "cardKitRatioApply"; quiet: true; text: qsTr("Apply"); onClicked: setLayoutDialog.applyCustomRatio() }
                                    }
                                    Label { Layout.fillWidth: true; visible: setLayoutDialog.ratioError.length > 0; text: setLayoutDialog.ratioError; color: Theme.danger; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                                    Label { Layout.fillWidth: true; visible: !root.composition.layout; text: qsTr("Choosing new proportions or a flexible layout changes the appearance of these original cards. Their text and images are kept."); color: Theme.warning; wrapMode: Text.WordWrap; font.pixelSize: 11 }
                                    CardLayoutOptions {
                                        objectName: "cardKitDefaultLayoutOptions"; Layout.fillWidth: true
                                        configuration: root.composition.layout || null; resolved: root.controller.preview.default_layout || ({})
                                        editIdentity: root.backend.projectId + "|" + root.backend.bundlePath + "|" + (root.controller.draft.artifact_id || "") + "|default"
                                        editingEnabled: root.visible && root.inspectorTab === 1 && root.layoutForSet
                                        onEdited: value => root.setField("layout", value)
                                    }
                                    ToolButton { text: (setLayoutDialog.promptsExpanded ? "⌄  " : "›  ") + qsTr("Creative direction"); font.pixelSize: 11; onClicked: setLayoutDialog.promptsExpanded = !setLayoutDialog.promptsExpanded }
                                    ColumnLayout {
                                        visible: setLayoutDialog.promptsExpanded; Layout.fillWidth: true; spacing: 8
                                        Label { text: qsTr("Shared brief"); color: Theme.muted; font.pixelSize: 11 }
                                        ShapeTextEditor { objectName: "cardKitBrief"; Layout.fillWidth: true; Layout.preferredHeight: 95; text: root.composition.brief || ""; placeholderText: qsTr("What are these cards for?"); wrapMode: TextEdit.Wrap; onTextChanged: if (editorActiveFocus) root.setField("brief", text) }
                                        Label { text: qsTr("Visual style"); color: Theme.muted; font.pixelSize: 11 }
                                        ShapeTextEditor { objectName: "cardKitStyle"; Layout.fillWidth: true; Layout.preferredHeight: 80; text: root.composition.style || ""; placeholderText: qsTr("Palette, mood and illustration style…"); wrapMode: TextEdit.Wrap; onTextChanged: if (editorActiveFocus) root.setField("style", text) }
                                    }
                                }
                            }
                            Item { Layout.preferredHeight: 18 }
                        }
                    }
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true; implicitHeight: feedbackText.implicitHeight + 20
            visible: feedbackText.text.length > 0
            color: Theme.warningSoft
            Label {
                id: feedbackText; objectName: "cardKitFeedback"
                anchors.fill: parent; anchors.leftMargin: 20; anchors.rightMargin: 20; anchors.topMargin: 10; anchors.bottomMargin: 10
                text: root.localError || root.controller.error || (root.imageCropLimited ? root.imageCropMessage : (root.controller.preview.stale_inputs || []).length > 0 ? qsTr("Some chosen materials have newer versions. Choose them explicitly if you want to update these cards.") : "")
                color: root.localError.length || root.controller.error.length ? Theme.danger : Theme.warning
                wrapMode: Text.WordWrap; font.pixelSize: 11
            }
        }
    }
    ShapeDialog {
        id: reloadConfirmation
        parent: Overlay.overlay; anchors.centerIn: parent
        width: Math.min(450, parent.width - 40); modal: true
        title: qsTr("Reload the saved draft?")
        contentItem: ColumnLayout {
            spacing: 18
            Label { Layout.fillWidth: true; text: qsTr("Unsaved edits will be replaced. Copy the complete draft first if you want to keep them."); wrapMode: Text.WordWrap; color: Theme.muted }
            RowLayout {
                Layout.fillWidth: true
                ShapeButton { quiet: true; text: qsTr("Cancel"); onClicked: reloadConfirmation.close() }
                Item { Layout.fillWidth: true }
                ShapeButton { text: qsTr("Reload draft"); onClicked: { root.reloadSavedDraft(); reloadConfirmation.close() } }
            }
        }
    }
    Menu {
        popupType: Popup.Item
        id: addMenu; objectName: "cardKitAddMenu"
        property string targetIdentity: ""
        readonly property bool targetCurrent: targetIdentity === root.menuIdentity(false)
        MenuItem { objectName: "cardKitAddCard"; text: qsTr("Blank card"); enabled: addMenu.targetCurrent && root.cards.length < 64; onTriggered: if (addMenu.targetCurrent) root.addCard() }
        MenuItem { objectName: "cardKitAddImages"; text: qsTr("Existing card images…"); enabled: addMenu.targetCurrent && root.cards.length < 64; onTriggered: if (addMenu.targetCurrent) root.beginCardImages() }
        MenuItem { objectName: "cardKitAddFromList"; text: qsTr("Cards from a list…"); enabled: addMenu.targetCurrent && root.cards.length < 64; onTriggered: if (addMenu.targetCurrent) root.openCardList() }
    }
    Menu {
        popupType: Popup.Item
        id: cardActions; objectName: "cardKitCardActions"
        property string targetIdentity: ""
        readonly property bool targetCurrent: targetIdentity === root.menuIdentity(true)
        MenuItem { objectName: "cardKitMoveUp"; text: qsTr("Move up"); enabled: cardActions.targetCurrent && root.cards.findIndex(c => c.id === root.selectedCardId) > 0; onTriggered: if (cardActions.targetCurrent) root.moveCard(-1) }
        MenuItem { objectName: "cardKitMoveDown"; text: qsTr("Move down"); enabled: cardActions.targetCurrent && root.cards.findIndex(c => c.id === root.selectedCardId) < root.cards.length - 1; onTriggered: if (cardActions.targetCurrent) root.moveCard(1) }
        MenuSeparator {}
        MenuItem { objectName: "cardKitRemoveCard"; text: qsTr("Remove card"); enabled: cardActions.targetCurrent && root.cards.length > 1; onTriggered: if (cardActions.targetCurrent) root.removeCard() }
    }
    Menu {
        popupType: Popup.Item
        id: imageMenu; objectName: "deckImageMenu"
        property string targetIdentity: ""
        readonly property bool targetCurrent: targetIdentity === root.menuIdentity(true)
        function openFor(button) { targetIdentity = root.menuIdentity(true); popup(button, 0, button.height) }
        MenuItem { objectName: "cardKitImportArtwork"; text: qsTr("Import image…"); enabled: imageMenu.targetCurrent; onTriggered: if (imageMenu.targetCurrent) root.beginImageImport() }
        MenuItem { objectName: "cardKitGenerateArtwork"; text: qsTr("Create image in Shape…"); enabled: imageMenu.targetCurrent; onTriggered: if (imageMenu.targetCurrent) root.openArtworkGeneration() }
        Menu {
        popupType: Popup.Item
            id: acceptedImages; objectName: "cardKitArtworkPicker"; title: qsTr("Images in this project")
            enabled: imageMenu.targetCurrent && root.images.length > 1
            Instantiator {
                model: root.images.slice(1)
                delegate: MenuItem { required property var modelData; text: modelData.name; onTriggered: if (imageMenu.targetCurrent && root.currentCard) root.bindArtwork(root.selectedCardId, root.pin(modelData)) }
                onObjectAdded: (index, object) => acceptedImages.insertItem(index, object)
                onObjectRemoved: (index, object) => acceptedImages.removeItem(object)
            }
        }
        MenuItem { objectName: "cardKitRefineArtwork"; text: root.artworkTargetExists && root.editingArtwork && root.artworkCardId === root.selectedCardId ? qsTr("Continue in Shadow…") : qsTr("Refine in Shadow…"); enabled: imageMenu.targetCurrent && root.currentCard !== null && !!root.currentCard.artwork; onTriggered: if (imageMenu.targetCurrent) root.beginArtworkEdit() }
        MenuSeparator {}
        MenuItem { objectName: "cardKitRemoveImage"; text: qsTr("Remove image"); enabled: imageMenu.targetCurrent && root.currentCard !== null && !!root.currentCard.artwork; onTriggered: if (imageMenu.targetCurrent) root.removeArtwork() }
    }
    function exportKitFiles(file, projectId, projectPath, sourcePin) {
        if (projectId !== backend.projectId || projectPath !== backend.bundlePath
            || JSON.stringify(sourcePin) !== JSON.stringify(controller.source)) return false
        const success = controller.exportFiles(file)
        if (success) fileExported(file.toString(), "zip", sourcePin)
        return success
    }
    CardPrintDialog { id: printDialog; backend: root.backend; controller: root.cardPrint; onFileExported: (path, source) => root.fileExported(path, "pdf", source) }
    CardImageIntakeDialog {
        id: imageIntake
        backend: root.backend; controller: root.controller
        capacity: 64 - CardListImport.effectiveCards(root.composition).length
        canUseImageShape: !!root.composition.layout && CardListImport.effectiveCards(root.composition).length === 0
        appendImage: (source, cardId, width, height, useShape, projectId, projectPath, kitId, order) => root.appendImportedCard(source, cardId, width, height, useShape, projectId, projectPath, kitId, order)
        canAppend: cardId => root.canAppendImage(cardId)
    }
    ShapeDialog {
        id: largePreview
        objectName: "cardKitLargePreview"
        property string svgSource: ""
        property string cardTitle: ""
        property int canvasWidth: 750
        property int canvasHeight: 1050
        property bool actualSize: true
        parent: Overlay.overlay; anchors.centerIn: parent
        width: parent ? Math.min(1200, parent.width - 60) : 1100
        height: parent ? parent.height - 60 : 800
        modal: true; title: cardTitle || qsTr("Card preview"); closePolicy: Popup.CloseOnEscape
        contentItem: ColumnLayout {
            RowLayout {
                Layout.fillWidth: true
                Label { Layout.fillWidth: true; text: largePreview.actualSize ? qsTr("Actual size · Scroll to inspect the whole card") : qsTr("Fit to window"); color: Theme.muted; wrapMode: Text.WordWrap }
                ShapeButton { objectName: "cardKitPreviewFit"; text: qsTr("Fit"); onClicked: largePreview.actualSize = false }
                ShapeButton { objectName: "cardKitPreviewActual"; text: qsTr("Actual size"); onClicked: largePreview.actualSize = true }
            }
            ScrollView {
                id: previewScroll
                Layout.fillWidth: true; Layout.fillHeight: true
                contentWidth: largeImage.width; contentHeight: largeImage.height
                clip: true
                background: Rectangle { color: Theme.panelInset; radius: Theme.controlRadius }
                Image {
                    id: largeImage
                    objectName: "cardKitLargeImage"
                    readonly property real fitScale: Math.min(1, previewScroll.availableWidth / largePreview.canvasWidth, previewScroll.availableHeight / largePreview.canvasHeight)
                    width: largePreview.canvasWidth * (largePreview.actualSize ? 1 : fitScale)
                    height: largePreview.canvasHeight * (largePreview.actualSize ? 1 : fitScale)
                    source: largePreview.svgSource
                    sourceSize: Qt.size(largePreview.canvasWidth, largePreview.canvasHeight)
                    fillMode: Image.PreserveAspectFit
                }
            }
            ShapeButton { objectName: "cardKitPreviewClose"; Layout.alignment: Qt.AlignRight; text: qsTr("Back to cards"); onClicked: largePreview.close() }
        }
    }
    ShapeDialog {
        id: listDialog
        objectName: "cardKitListDialog"
        parent: Overlay.overlay; anchors.centerIn: parent
        width: parent ? Math.min(760, parent.width - 60) : 760
        height: parent ? Math.min(660, parent.height - 60) : 660
        modal: true; title: qsTr("Add cards from a list"); closePolicy: Popup.CloseOnEscape
        contentItem: ColumnLayout {
            spacing: 10
            Label { Layout.fillWidth: true; text: qsTr("Paste one card per row, or copy table columns for title, text and image description. A title or body text is enough. A single column keeps every row as a card."); wrapMode: Text.WordWrap; color: Theme.muted }
            ShapeTextEditor { id: listText; objectName: "cardKitListText"; Layout.fillWidth: true; Layout.preferredHeight: 200; wrapMode: TextEdit.NoWrap; placeholderText: qsTr("Paste your rows here…") }
            CheckBox { objectName: "cardKitListHasHeading"; text: qsTr("First row contains column headings"); checked: root.listFirstHeader; onToggled: root.listFirstHeader = checked }
            Label { objectName: "cardKitListSummary"; Layout.fillWidth: true; text: root.listPreview.error.length ? root.cardListError() : root.listPreview.replacesPlaceholder ? qsTr("New cards: %1. The empty starter card will be replaced.").arg(root.listPreview.rows.length) : qsTr("New cards: %1. Existing cards stay as they are.").arg(root.listPreview.rows.length); color: root.listPreview.error.length && root.listPreview.error !== "empty" ? Theme.danger : Theme.text; wrapMode: Text.WordWrap }
            Label { Layout.fillWidth: true; visible: root.listPreview.headerSkipped; text: qsTr("The column heading row is skipped."); color: Theme.muted }
            Label { Layout.fillWidth: true; visible: root.listPreview.duplicateCount > 0; text: qsTr("Repeated titles are kept as separate cards: %1.").arg(root.listPreview.duplicateCount); color: Theme.warning; wrapMode: Text.WordWrap }
            ListView {
                objectName: "cardKitListPreview"
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true; spacing: 5
                model: root.listPreview.rows
                delegate: Rectangle {
                    id: rowPreview
                    required property var modelData
                    required property int index
                    readonly property var row: modelData || ({})
                    width: ListView.view.width; height: 58
                    radius: 6; color: Theme.panel; border.color: Theme.border
                    ColumnLayout {
                        anchors.fill: parent; anchors.margins: 8; spacing: 3
                        Label { Layout.fillWidth: true; text: (rowPreview.index + 1) + ". " + (rowPreview.row.title || ""); color: Theme.text; elide: Text.ElideRight }
                        Label { Layout.fillWidth: true; text: rowPreview.row.rules || rowPreview.row.art_prompt || qsTr("Details can be added later."); color: Theme.muted; elide: Text.ElideRight }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                ShapeButton { objectName: "cardKitListCancel"; text: qsTr("Cancel"); onClicked: listDialog.close() }
                Item { Layout.fillWidth: true }
                ShapeButton { objectName: "cardKitListApply"; text: root.listPreview.rows.length === 1 ? qsTr("Add card") : qsTr("Add %1 cards").arg(root.listPreview.rows.length); primary: true; enabled: !root.listApplied && !root.listPreview.error.length && root.listPreview.rows.length > 0; onClicked: root.appendCardList() }
            }
        }
    }

    ShapeDialog {
        id: shapeTaskDialog
        objectName: "deckShapeImageDialog"
        property string cardId: ""
        property string projectId: ""
        property string projectPath: ""
        property string kitId: ""
        readonly property bool currentTarget: projectId === root.backend.projectId && projectPath === root.backend.bundlePath
            && kitId === root.controller.draft.artifact_id && cardId === root.selectedCardId
        readonly property bool ownsResult: root.imageTask.target.card_id === cardId && root.imageTask.target.kit_id === kitId
            && root.imageTask.target.project_id === projectId
        parent: Overlay.overlay; anchors.centerIn: parent
        width: parent ? Math.min(960, parent.width - 48) : 940
        height: parent ? Math.min(720, parent.height - 48) : 680
        modal: true; closePolicy: Popup.CloseOnEscape
        title: qsTr("Generate an image in Shape")
        contentItem: ColumnLayout {
            spacing: 12
            Label { Layout.fillWidth: true; text: qsTr("Shape opens with this instruction and uses its own image-generation access. Accept a result there, then choose Return to Deck. You decide when to use it on this card."); color: Theme.muted; wrapMode: Text.WordWrap }
            RowLayout {
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: 16
                ColumnLayout {
                    Layout.preferredWidth: 340; Layout.fillHeight: true
                    ShapeTextEditor { id: shapePrompt; objectName: "deckShapePrompt"; Layout.fillWidth: true; Layout.fillHeight: true; wrapMode: TextEdit.Wrap; readOnly: root.imageTask.running || root.imageTask.resultReady }
                    ShapeButton { objectName: "deckGenerateInShape"; Layout.fillWidth: true; text: root.imageTask.running ? qsTr("Waiting for Shape…") : qsTr("Generate image in Shape"); primary: true
                        enabled: shapeTaskDialog.currentTarget && !root.imageTask.running && !root.imageTask.resultReady && shapePrompt.text.trim().length > 0
                        onClicked: if (root.flush()) root.imageTask.begin(shapePrompt.text, root.composition.canvas_width || 750, root.composition.canvas_height || 1050, shapeTaskDialog.cardId) }
                    ShapeButton { Layout.fillWidth: true; visible: root.imageTask.running || root.imageTask.resultReady; text: qsTr("Stop waiting / discard return"); onClicked: root.imageTask.stopWaiting() }
                }
                ColumnLayout {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    Rectangle {
                        Layout.fillWidth: true; Layout.fillHeight: true; color: Theme.panelInset; radius: Theme.panelRadius
                        Image { objectName: "deckShapeReturnedImage"; anchors.fill: parent; anchors.margins: 8; source: shapeTaskDialog.ownsResult ? root.imageTask.previewSource : ""; fillMode: Image.PreserveAspectFit }
                    }
                    ShapeButton { objectName: "deckOpenReturnedImage"; Layout.fillWidth: true; visible: shapeTaskDialog.ownsResult && root.imageTask.resultReady; text: qsTr("Open returned image file"); onClicked: Qt.openUrlExternally(root.imageTask.previewSource) }
                    ShapeButton { Layout.fillWidth: true; visible: shapeTaskDialog.ownsResult && root.imageTask.resultReady && !root.imageTask.targetCurrent; text: qsTr("Choose an image to import explicitly…"); onClicked: root.beginImageImport() }
                    ShapeButton { objectName: "deckUseShapeImage"; Layout.fillWidth: true; text: qsTr("Use this image"); primary: true; enabled: shapeTaskDialog.currentTarget && shapeTaskDialog.ownsResult && root.imageTask.resultReady && root.imageTask.targetCurrent; onClicked: root.useShapeImage() }
                }
            }
            Label { Layout.fillWidth: true; text: root.imageTask.error || root.imageTask.status; color: root.imageTask.error.length ? Theme.danger : Theme.muted; wrapMode: Text.WordWrap }
            ShapeButton { objectName: "deckShapeBack"; Layout.alignment: Qt.AlignRight; quiet: true; text: qsTr("Back to cards"); onClicked: shapeTaskDialog.close() }
        }
    }
    ShapeDialog {
        id: refinementDialog
        objectName: "cardArtworkRefinementDialog"
        parent: Overlay.overlay; anchors.centerIn: parent
        width: parent ? parent.width - 60 : 1180; height: parent ? parent.height - 60 : 760
        modal: true; title: qsTr("Refine card artwork"); closePolicy: Popup.NoAutoClose
        contentItem: ColumnLayout {
            MaterialEditWorkspace {
                Layout.fillWidth: true; Layout.fillHeight: true
                backend: root.backend; controller: root.materialEdit; audioPreview: root.audioPreview
                projectPath: root.backend.bundlePath
                outputArtifactId: root.editingStage.contextArtifactId || ""
                operatorDraftId: root.editingStage.id || ""
                inputArtifactId: root.editingStage.inputArtifactId || ""
                inputRevisionId: root.editingStage.inputRevisionId || ""
                imageMode: true
                inputCurrent: root.artworkStageCurrent
                sourceName: (root.cards.find(c => c.id === root.artworkCardId) || {}).title || ""
                candidate: root.artworkCandidate
                acceptedImageSource: root.editingStage.contextArtifactId ? root.backend.materialSourceImage(root.editingStage.contextArtifactId) : ""
                candidateImageSource: root.artworkCandidateSource
            }
            RowLayout {
                ShapeButton { objectName: "cardArtworkBack"; quiet: true; text: qsTr("Back to cards"); onClicked: refinementDialog.close() }
                Item { Layout.fillWidth: true }
                ShapeButton { objectName: "cardArtworkUseRefined"; text: qsTr("Use this artwork for the card"); primary: true; enabled: root.artworkTargetExists && root.artworkStageCurrent && root.artworkCandidate !== null && !root.materialEdit.running && !root.materialEdit.shadowActive; onClicked: root.useArtwork() }
            }
        }
    }
}
