pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Shape.Desktop

ColumnLayout {
    id: root
    objectName: "cardBlockEditor"
    required property var commitConfiguration
    property var configuration: null
    property var resolvedLayout: ({})
    property var card: ({})
    property string targetIdentity: ""
    property string cardIdentity: ""
    property string svgSource: ""
    property bool editingEnabled: true
    property var draftLayout: ({blocks: []})
    property var gesture: null
    property var undoRecord: null
    property var pendingLayout: null
    property var pendingEdits: ({})
    property string pendingIdentity: ""
    property bool committing: false
    property string selectedBlockId: ""
    property string retainedSvg: ""
    property string retainedSvgIdentity: ""
    property string error: ""
    property int sequence: 1
    property int snapUnits: 10
    property string colorTarget: ""
    property string colorIdentity: ""
    readonly property var fields: ["title", "rules", "artwork", "card_type", "cost", "stats", "flavor"]
    readonly property var blocks: draftLayout.blocks || []
    readonly property var selectedBlock: blocks.find(item => item.id === selectedBlockId) || null
    readonly property bool gestureActive: gesture !== null
    readonly property string displaySource: retainedSvgIdentity === cardIdentity ? retainedSvg || svgSource : svgSource
    readonly property var stalePendingKeys: Object.keys(pendingEdits).filter(key =>
        pendingEdits[key].card === cardIdentity && key !== targetIdentity)
    readonly property bool hasPendingCapacity: !!pendingEdits[targetIdentity] || Object.keys(pendingEdits).length < 8
    readonly property bool canUndoBlockChange: !!undoRecord && !gestureActive && pendingLayout === null
        && undoRecord.card === cardIdentity && undoRecord.context === targetIdentity
        && undoRecord.after === JSON.stringify(configuration || null)
    signal chooseLayoutRequested()
    signal contentRequested()
    signal savePresetRequested()
    signal batchRequested()
    spacing: 9

    component StyleToggle: ToolButton {
        id: control
        property string toolTipText: ""
        implicitWidth: Theme.iconButtonSize; implicitHeight: Theme.iconButtonSize
        checkable: true; padding: 0; hoverEnabled: true
        Accessible.name: toolTipText
        contentItem: Text {
            text: control.text; font: control.font
            color: !control.enabled ? Theme.disabled : control.checked ? Theme.accent : Theme.textSoft
            horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: Theme.compactControlRadius
            color: control.checked ? Theme.accentSoft : control.down ? Theme.buttonGhostPressed : control.hovered ? Theme.buttonGhostHover : "transparent"
            border.color: control.visualFocus ? Theme.focusRing : control.checked ? Theme.accentBorder : "transparent"
        }
        ToolTip.visible: hovered || visualFocus
        ToolTip.delay: 450
        ToolTip.text: toolTipText
    }

    function copy(value) { return JSON.parse(JSON.stringify(value)) }
    function retainPreviewSource() {
        if (!svgSource.length) return false
        retainedSvg = svgSource; retainedSvgIdentity = cardIdentity; return true
    }
    function fieldName(field) {
        const names = {title: qsTr("Title"), rules: qsTr("Body text"), artwork: qsTr("Image"),
            card_type: qsTr("Type"), cost: qsTr("Cost"), stats: qsTr("Stats"), flavor: qsTr("Additional text")}
        return names[field] || field
    }
    function fullLayout() {
        const next = {mode: "blocks", image_fit: "contain", background_color: "#ffffff", text_color: "#202020",
            border: false, padding_percent: 4, text_align: "left", text_scale_percent: 100,
            image_position: "auto", image_share_percent: 50, text_vertical_align: "auto",
            text_offset_x_percent: 0, text_offset_y_percent: 0}
        for (const key of Object.keys(next)) {
            if (resolvedLayout[key] !== undefined) next[key] = resolvedLayout[key]
            if (configuration && configuration[key] !== undefined) next[key] = configuration[key]
        }
        next.mode = "blocks"
        const stored = configuration && configuration.blocks !== undefined ? configuration.blocks : resolvedLayout.blocks
        next.blocks = copy(stored || [])
        return next
    }
    function synchronizeLayout() {
        draftLayout = fullLayout()
        if (!blocks.some(item => item.id === selectedBlockId)) selectedBlockId = blocks.length ? blocks[0].id : ""
    }
    function rememberPending(next, identity) {
        const records = copy(pendingEdits)
        delete records[identity]
        records[identity] = {card: cardIdentity, layout: copy(next)}
        pendingEdits = records; pendingIdentity = identity
    }
    function clearPending(identity) {
        const records = copy(pendingEdits); delete records[identity]; pendingEdits = records
        pendingIdentity = ""
    }
    function discardEarlierChanges() {
        const records = copy(pendingEdits)
        for (const key of stalePendingKeys) delete records[key]
        pendingEdits = records; error = ""
    }
    function requirePendingCapacity() {
        if (hasPendingCapacity) return true
        error = qsTr("Eight block changes are waiting to be saved. Return to those cards and retry or discard them before editing another card.")
        return false
    }
    function resetTarget() {
        if (committing) return
        cancelGesture(); pendingLayout = null; pendingIdentity = ""; undoRecord = null; error = ""; colors.close(); synchronizeLayout()
        const pending = pendingEdits[targetIdentity]
        if (pending && pending.card === cardIdentity) {
            pendingLayout = copy(pending.layout); draftLayout = copy(pending.layout); pendingIdentity = targetIdentity
            if (!blocks.some(item => item.id === selectedBlockId)) selectedBlockId = blocks.length ? blocks[0].id : ""
            error = qsTr("Could not save this block change. Keep editing here or retry.")
        }
    }
    function enter() {
        if (pendingLayout !== null && pendingIdentity === targetIdentity) return true
        synchronizeLayout()
        if ((configuration && configuration.mode === "blocks") || resolvedLayout.mode === "blocks") return true
        const next = fullLayout()
        if (!next.blocks.length) next.blocks = arrange("auto")
        return submit(next)
    }
    function submit(next) {
        if (!editingEnabled || gestureActive || !requirePendingCapacity()) return false
        const before = copy(configuration || null), identity = targetIdentity
        committing = true
        const saved = commitConfiguration(copy(next), identity)
        committing = false
        if (!saved) {
            pendingLayout = copy(next); draftLayout = copy(next)
            rememberPending(next, identity)
            error = qsTr("Could not save this block change. Keep editing here or retry."); return false
        }
        undoRecord = {card: cardIdentity, context: targetIdentity, before: before,
            after: JSON.stringify(configuration || null)}
        clearPending(identity); pendingLayout = null; error = ""; synchronizeLayout()
        retainPreviewSource()
        return true
    }
    function retryChange() {
        if (!pendingLayout) return false
        return submit(pendingLayout)
    }
    function discardChange() {
        clearPending(pendingIdentity || targetIdentity); pendingLayout = null; error = ""; synchronizeLayout()
    }
    function undoBlockChange() {
        if (!editingEnabled || !canUndoBlockChange) return false
        const previous = copy(undoRecord.before)
        committing = true
        const saved = commitConfiguration(previous, targetIdentity)
        committing = false
        if (!saved) { error = qsTr("Could not undo this change. Your current draft was kept."); return false }
        undoRecord = null; pendingLayout = null; error = ""; synchronizeLayout(); return true
    }
    function selectBlock(id) {
        if (gestureActive || !blocks.some(item => item.id === id)) return false
        selectedBlockId = id; return true
    }
    function nextId(field) {
        let id
        do { id = field + "-" + sequence++ } while (blocks.some(item => item.id === id))
        return id
    }
    function makeBlock(field, x, y, width, height) {
        return {id: nextId(field), field: field, x: x, y: y, width: width, height: height,
            text_scale_percent: Math.round(Math.max(50, Math.min(300, draftLayout.text_scale_percent || 100))),
            text_align: draftLayout.text_align || "left", text_vertical_align: "top", bold: false,
            italic: false, text_color: draftLayout.text_color || "#202020", image_fit: "contain"}
    }
    function contentFields(visibleOnly) {
        let available = fields.filter(field => field === "artwork" ? !!card.artwork
            : field === "rules" ? !!card.text_material || !!(card.rules || "").trim().length : !!(card[field] || "").trim().length)
        if (visibleOnly && resolvedLayout.mode === "image") available = available.filter(field => field === "artwork")
        if (visibleOnly && resolvedLayout.mode === "text") available = available.filter(field => field !== "artwork")
        return available.length ? available : ["title", "rules"]
    }
    function arrange(kind) {
        const result = [], available = contentFields(kind === "auto")
        if (kind === "game") {
            const positions = {title:[600,500,6900,1000], cost:[7800,500,1600,1000], card_type:[600,1700,8800,600],
                artwork:[600,2600,8800,3400], rules:[600,6300,8800,1900], stats:[600,8500,8800,500], flavor:[600,9200,8800,500]}
            for (const field of fields) if (available.indexOf(field) >= 0) {
                const box = positions[field], block = makeBlock(field, box[0], box[1], box[2], box[3])
                block.bold = field === "title"; block.italic = field === "flavor"; result.push(block)
            }
            return result
        }
        const hasImage = kind === "split" || available.indexOf("artwork") >= 0
        const text = available.filter(field => field !== "artwork")
        if (hasImage) result.push(makeBlock("artwork", 500, 500, kind === "split" ? 4300 : 9000, text.length ? kind === "split" ? 9000 : 4300 : 9000))
        const left = kind === "split" ? 5300 : 700, top = kind === "split" || !hasImage ? 700 : 5200
        const width = kind === "split" ? 4000 : 8600, height = 9300 - top
        const weights = text.map(field => field === "rules" ? 4 : field === "title" ? 1.4 : 1)
        const total = weights.reduce((sum, value) => sum + value, 0)
        let position = top
        for (let index = 0; index < text.length; ++index) {
            const share = Math.max(100, Math.floor(height * weights[index] / total)), field = text[index]
            const block = makeBlock(field, left, position, width, share - 100)
            block.bold = field === "title"; result.push(block); position += share
        }
        return result
    }
    function quickStart(kind) {
        if (!editingEnabled || gestureActive || ["auto", "split", "game"].indexOf(kind) < 0) return false
        const next = copy(draftLayout); next.blocks = arrange(kind)
        return submit(next)
    }
    function addBlock(field) {
        if (!editingEnabled || gestureActive || fields.indexOf(field) < 0 || blocks.length >= 16) return false
        const next = copy(draftLayout), block = makeBlock(field, 1000, 1000, 8000, field === "artwork" ? 6000 : 1800)
        next.blocks.push(block)
        const saved = submit(next); selectedBlockId = block.id; return saved
    }
    function removeSelectedBlock() {
        if (!selectedBlock || gestureActive) return false
        const next = copy(draftLayout); next.blocks = next.blocks.filter(item => item.id !== selectedBlockId)
        return submit(next)
    }
    function moveSelectedBlock(delta) {
        const from = blocks.findIndex(item => item.id === selectedBlockId), to = from + delta
        if (gestureActive || from < 0 || to < 0 || to >= blocks.length) return false
        const next = copy(draftLayout); next.blocks.splice(to, 0, next.blocks.splice(from, 1)[0])
        return submit(next)
    }
    function setBlockValue(key, value) {
        if (!selectedBlock || gestureActive || !editingEnabled || selectedBlock[key] === value) return false
        if (["field", "text_scale_percent", "text_align", "text_vertical_align", "bold", "italic", "text_color", "image_fit"].indexOf(key) < 0) return false
        const next = copy(draftLayout), block = next.blocks.find(item => item.id === selectedBlockId)
        block[key] = value; return submit(next)
    }
    function setGeometry(key, value) {
        if (!selectedBlock || gestureActive || !editingEnabled || !isFinite(value)) return false
        const next = copy(draftLayout), block = next.blocks.find(item => item.id === selectedBlockId)
        const limits = {x: 10000 - block.width, y: 10000 - block.height, width: 10000 - block.x, height: 10000 - block.y}
        if (limits[key] === undefined) return false
        block[key] = Math.max(key === "width" || key === "height" ? 1 : 0, Math.min(limits[key], Math.round(value)))
        if (block[key] === selectedBlock[key]) return true
        return submit(next)
    }
    function beginGesture(id, mode = "move") {
        if (!editingEnabled || gestureActive || !requirePendingCapacity()
            || ["move","nw","ne","sw","se"].indexOf(mode) < 0 || !selectBlock(id)) return false
        gesture = {identity: targetIdentity, card: cardIdentity, mode: mode, before: copy(draftLayout), block: copy(selectedBlock)}
        retainPreviewSource()
        return true
    }
    function updateGesture(dx, dy) {
        if (!gesture || gesture.identity !== targetIdentity || gesture.card !== cardIdentity || !isFinite(dx) || !isFinite(dy)) return false
        const start = gesture.block, step = Math.max(1, snapUnits), snap = value => Math.round(value / step) * step
        const clamp = (value, low, high) => Math.max(low, Math.min(high, value))
        const next = copy(gesture.before), block = next.blocks.find(item => item.id === start.id)
        if (gesture.mode === "move") {
            block.x = clamp(snap(start.x + dx), 0, 10000 - start.width)
            block.y = clamp(snap(start.y + dy), 0, 10000 - start.height)
        } else {
            let left = start.x, top = start.y, right = start.x + start.width, bottom = start.y + start.height
            if (gesture.mode.indexOf("w") >= 0) left = clamp(snap(left + dx), 0, right - 1)
            if (gesture.mode.indexOf("e") >= 0) right = clamp(snap(right + dx), left + 1, 10000)
            if (gesture.mode.indexOf("n") >= 0) top = clamp(snap(top + dy), 0, bottom - 1)
            if (gesture.mode.indexOf("s") >= 0) bottom = clamp(snap(bottom + dy), top + 1, 10000)
            block.x = left; block.y = top; block.width = right - left; block.height = bottom - top
        }
        draftLayout = next; return true
    }
    function finishGesture() {
        if (!gesture) return false
        const captured = gesture, next = copy(draftLayout); gesture = null
        if (captured.identity !== targetIdentity || captured.card !== cardIdentity) { synchronizeLayout(); return false }
        if (JSON.stringify(next) === JSON.stringify(captured.before)) return true
        return submit(next)
    }
    function cancelGesture() {
        if (!gesture) return false
        draftLayout = gesture.before; gesture = null; return true
    }
    function chooseTextColor() {
        if (!selectedBlock || !editingEnabled) return
        colorTarget = selectedBlockId; colorIdentity = targetIdentity
        colors.selectedColor = selectedBlock.text_color; colors.open()
    }
    onTargetIdentityChanged: resetTarget()
    onCardIdentityChanged: { retainedSvg = ""; retainedSvgIdentity = ""; resetTarget(); retainPreviewSource() }
    onConfigurationChanged: if (!committing && !gestureActive && pendingLayout === null) synchronizeLayout()
    onResolvedLayoutChanged: if (!committing && !gestureActive && pendingLayout === null) synchronizeLayout()
    onSvgSourceChanged: if (!gestureActive) retainPreviewSource()
    onEditingEnabledChanged: if (!editingEnabled) { cancelGesture(); colors.close() }
    Component.onCompleted: synchronizeLayout()
    ColorDialog {
        id: colors; objectName: "cardBlockColorDialog"; title: qsTr("Choose text color")
        onAccepted: if (root.colorIdentity === root.targetIdentity && root.colorTarget === root.selectedBlockId)
            root.setBlockValue("text_color", "#" + String(selectedColor).slice(-6))
    }
    RowLayout {
        Layout.fillWidth: true; spacing: 4
        Label { Layout.fillWidth: true; text: qsTr("Blocks"); color: Theme.text; font.weight: Font.DemiBold }
        ShapeIconButton { objectName: "cardBlockUndo"; source: "../../icons/back.svg"; toolTipText: qsTr("Undo block change"); enabled: root.canUndoBlockChange && root.editingEnabled; onClicked: root.undoBlockChange() }
        ShapeIconButton { source: "../../icons/edit.svg"; toolTipText: qsTr("Edit card content"); onClicked: root.contentRequested() }
        ShapeIconButton { objectName: "cardBlockChooseLayout"; source: "../../icons/layout.svg"; toolTipText: qsTr("Choose another layout"); onClicked: root.chooseLayoutRequested() }
    }
    Label { Layout.fillWidth: true; text: qsTr("Drag blocks on the card. Resize from a corner. Edit words and images in Content."); color: Theme.muted; font.pixelSize: 11; wrapMode: Text.WordWrap }
    RowLayout {
        Layout.fillWidth: true; spacing: 4
        ShapeComboBox { id: addField; objectName: "cardBlockAddField"; Layout.fillWidth: true; model: root.fields.map(field => root.fieldName(field)) }
        ShapeIconButton { objectName: "cardBlockAdd"; source: "../../icons/add.svg"; toolTipText: qsTr("Add field block"); enabled: root.blocks.length < 16 && root.editingEnabled && root.hasPendingCapacity && !root.gestureActive; onClicked: root.addBlock(root.fields[addField.selectedIndex]) }
    }
    ShapeComboBox {
        objectName: "cardBlockSelection"; Layout.fillWidth: true
        model: root.blocks.map((block, index) => (index + 1) + " · " + root.fieldName(block.field))
        selectedIndex: Math.max(0, root.blocks.findIndex(block => block.id === root.selectedBlockId))
        onActivated: index => root.selectBlock(root.blocks[index].id)
    }
    RowLayout {
        Layout.fillWidth: true; spacing: 5
        Label { Layout.fillWidth: true; text: qsTr("%1 of 16 blocks").arg(root.blocks.length); color: Theme.muted; font.pixelSize: 11 }
        ShapeIconButton { objectName: "cardBlockBackward"; source: "../../icons/arrow-down.svg"; toolTipText: qsTr("Send backward"); enabled: !!root.selectedBlock && !root.gestureActive; onClicked: root.moveSelectedBlock(-1) }
        ShapeIconButton { objectName: "cardBlockForward"; source: "../../icons/arrow-up.svg"; toolTipText: qsTr("Bring forward"); enabled: !!root.selectedBlock && !root.gestureActive; onClicked: root.moveSelectedBlock(1) }
        ShapeIconButton { objectName: "cardBlockRemove"; source: "../../icons/trash.svg"; toolTipText: qsTr("Remove block"); enabled: !!root.selectedBlock && !root.gestureActive; onClicked: root.removeSelectedBlock() }
    }
    ColumnLayout {
        visible: root.selectedBlock !== null; Layout.fillWidth: true; spacing: 8
        enabled: root.editingEnabled && root.hasPendingCapacity && !root.gestureActive
        GridLayout {
            Layout.fillWidth: true; columns: 2; columnSpacing: 8; rowSpacing: 6
            Repeater {
                model: [{key:"x",label:qsTr("X (%)")},{key:"y",label:qsTr("Y (%)")},{key:"width",label:qsTr("Width (%)")},{key:"height",label:qsTr("Height (%)")}]
                delegate: ColumnLayout {
                    id: geometry
                    required property var modelData
                    Layout.fillWidth: true; spacing: 3
                    Label { text: geometry.modelData.label; color: Theme.muted; font.pixelSize: 11 }
                    ShapeSpinBox {
                        id: measurement
                        objectName: "cardBlock-" + geometry.modelData.key
                        Layout.fillWidth: true; from: geometry.modelData.key === "x" || geometry.modelData.key === "y" ? 0 : 1; to: 10000; stepSize: root.snapUnits
                        value: root.selectedBlock ? root.selectedBlock[geometry.modelData.key] : from
                        validator: DoubleValidator { bottom: measurement.from / 100; top: 100; decimals: 2; notation: DoubleValidator.StandardNotation }
                        textFromValue: (value, locale) => Number(value / 100).toLocaleString(locale, 'f', 2)
                        valueFromText: (text, locale) => Math.round(Number.fromLocaleString(locale, text) * 100)
                        onValueModified: root.setGeometry(geometry.modelData.key, value)
                    }
                }
            }
        }
        ShapeComboBox { objectName: "cardBlockField"; Layout.fillWidth: true; model: root.fields.map(field => root.fieldName(field)); selectedIndex: root.selectedBlock ? Math.max(0, root.fields.indexOf(root.selectedBlock.field)) : 0; onActivated: index => root.setBlockValue("field", root.fields[index]) }
        ColumnLayout {
            visible: root.selectedBlock !== null && root.selectedBlock.field !== "artwork"; Layout.fillWidth: true; spacing: 8
            RowLayout {
                Layout.fillWidth: true
                Label { Layout.fillWidth: true; text: qsTr("Text size (%)"); color: Theme.muted; font.pixelSize: 11 }
                ShapeSpinBox { objectName: "cardBlockTextSize"; from: 50; to: 300; stepSize: 10; value: root.selectedBlock ? root.selectedBlock.text_scale_percent : 100; onValueModified: root.setBlockValue("text_scale_percent", value) }
            }
            ShapeComboBox { objectName: "cardBlockTextAlign"; Layout.fillWidth: true; model: [qsTr("Left"),qsTr("Center"),qsTr("Right")]; selectedIndex: root.selectedBlock ? Math.max(0,["left","center","right"].indexOf(root.selectedBlock.text_align)) : 0; onActivated: index => root.setBlockValue("text_align",["left","center","right"][index]) }
            ShapeComboBox { objectName: "cardBlockTextVertical"; Layout.fillWidth: true; model: [qsTr("Top"),qsTr("Center"),qsTr("Bottom")]; selectedIndex: root.selectedBlock ? Math.max(0,["top","center","bottom"].indexOf(root.selectedBlock.text_vertical_align)) : 0; onActivated: index => root.setBlockValue("text_vertical_align",["top","center","bottom"][index]) }
            RowLayout {
                Layout.fillWidth: true; spacing: 6
                StyleToggle { objectName: "cardBlockBold"; text: "B"; font.bold: true; checked: !!root.selectedBlock && root.selectedBlock.bold; toolTipText: qsTr("Bold"); onClicked: root.setBlockValue("bold", checked) }
                StyleToggle { objectName: "cardBlockItalic"; text: "I"; font.italic: true; checked: !!root.selectedBlock && root.selectedBlock.italic; toolTipText: qsTr("Italic"); onClicked: root.setBlockValue("italic", checked) }
                ToolButton { objectName: "cardBlockColor"; implicitWidth: 34; implicitHeight: 34; Accessible.name: qsTr("Choose text color"); onClicked: root.chooseTextColor(); background: Rectangle { radius: 6; color: root.selectedBlock ? root.selectedBlock.text_color : "#202020"; border.color: Theme.borderStrong } }
                ShapeTextField {
                    objectName: "cardBlockTextColor"; Layout.fillWidth: true; maximumLength: 7
                    text: root.selectedBlock ? root.selectedBlock.text_color : "#202020"
                    validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
                    onEditingFinished: if (acceptableInput) root.setBlockValue("text_color", text)
                }
            }
        }
        ShapeComboBox { objectName: "cardBlockImageFit"; visible: !!root.selectedBlock && root.selectedBlock.field === "artwork"; Layout.fillWidth: true; model: [qsTr("Show the complete image"),qsTr("Fill the block · crop edges")]; selectedIndex: root.selectedBlock && root.selectedBlock.image_fit === "cover" ? 1 : 0; onActivated: index => root.setBlockValue("image_fit",index === 1 ? "cover" : "contain") }
    }
    RowLayout {
        Layout.fillWidth: true
        Label { Layout.fillWidth: true; text: qsTr("Snap"); color: Theme.muted; font.pixelSize: 11 }
        ShapeComboBox { objectName: "cardBlockSnap"; model: ["0.1%","1%"] ; selectedIndex: root.snapUnits === 100 ? 1 : 0; onActivated: index => root.snapUnits = index === 1 ? 100 : 10 }
    }
    RowLayout {
        Layout.fillWidth: true; spacing: 5
        ShapeComboBox { id: quick; objectName: "cardBlockQuickStart"; Layout.fillWidth: true; model: [qsTr("Content layout"),qsTr("Picture left"),qsTr("Game notes")] }
        ShapeIconButton { objectName: "cardBlockApplyQuickStart"; source: "../../icons/layout.svg"; toolTipText: qsTr("Use quick start"); enabled: root.editingEnabled && root.hasPendingCapacity && !root.gestureActive; onClicked: root.quickStart(["auto","split","game"][quick.selectedIndex]) }
    }
    ShapeButton { objectName: "cardBlockSavePreset"; text: qsTr("Save as preset…"); quiet: true; enabled: root.editingEnabled && !root.gestureActive && root.pendingLayout === null; onClicked: root.savePresetRequested() }
    ShapeButton { objectName: "cardBlockApplyToCards"; text: qsTr("Apply preset to cards…"); quiet: true; enabled: root.editingEnabled && !root.gestureActive && root.pendingLayout === null; onClicked: root.batchRequested() }
    Label { Layout.fillWidth: true; visible: !root.hasPendingCapacity; text: qsTr("Eight block changes are waiting to be saved. Return to those cards and retry or discard them before editing another card."); color: Theme.warning; font.pixelSize: 11; wrapMode: Text.WordWrap }
    Label { objectName: "cardBlockEarlierChange"; Layout.fillWidth: true; visible: root.stalePendingKeys.length > 0; text: qsTr("An unsaved block change belongs to an earlier draft of this card. It was kept separately and cannot replace your newer edits."); color: Theme.warning; font.pixelSize: 11; wrapMode: Text.WordWrap }
    ShapeButton { objectName: "cardBlockDiscardEarlier"; visible: root.stalePendingKeys.length > 0; text: qsTr("Discard earlier change"); quiet: true; onClicked: root.discardEarlierChanges() }
    Label { objectName: "cardBlockError"; Layout.fillWidth: true; visible: root.error.length > 0; text: root.error; color: Theme.danger; font.pixelSize: 11; wrapMode: Text.WordWrap }
    RowLayout {
        visible: root.pendingLayout !== null; Layout.fillWidth: true
        ShapeButton { text: qsTr("Discard change"); quiet: true; onClicked: root.discardChange() }
        Item { Layout.fillWidth: true }
        ShapeButton { objectName: "cardBlockRetry"; text: qsTr("Retry"); onClicked: root.retryChange() }
    }
}
