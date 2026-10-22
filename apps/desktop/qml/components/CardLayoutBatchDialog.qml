pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

ShapeDialog {
    id: root
    objectName: "cardLayoutBatchDialog"
    required property CardLayoutPresetStore store
    property var cards: []
    property var selectedIds: []
    property var configuration: null
    property string presetName: ""
    property string targetIdentity: ""
    property string currentIdentity: ""
    property string error: ""
    readonly property bool targetCurrent: targetIdentity === currentIdentity
    signal applyRequested(var configuration, var cardIds, string identity)
    parent: Overlay.overlay; anchors.centerIn: parent
    width: parent ? Math.min(900, parent.width - 40) : 900
    height: parent ? Math.min(760, parent.height - 40) : 760
    modal: true; title: qsTr("Apply layout to cards"); closePolicy: Popup.CloseOnEscape
    function openForCards(values, selectedId, identity) {
        cards = values.map(card => ({id: card.id, title: card.title || card.rules || card.id}))
        selectedIds = cards.some(card => card.id === selectedId) ? [selectedId] : []
        configuration = null; presetName = ""; error = ""; targetIdentity = identity; open()
    }
    function selectCard(cardId, selected) {
        if (!cards.some(card => card.id === cardId)) return false
        selectedIds = selected ? selectedIds.indexOf(cardId) < 0 ? selectedIds.concat([cardId]) : selectedIds
            : selectedIds.filter(id => id !== cardId)
        return true
    }
    function selectAll(selected) { selectedIds = selected ? cards.map(card => card.id) : [] }
    contentItem: ColumnLayout {
        spacing: 12
        Label { Layout.fillWidth: true; text: qsTr("Choose a preset and the cards to change. Their content, quantities and image references stay the same."); color: Theme.muted; wrapMode: Text.WordWrap }
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 18
            ScrollView {
                id: presetScroll
                Layout.preferredWidth: 320; Layout.fillHeight: true; clip: true; contentWidth: availableWidth
                CardLayoutPresets {
                    width: presetScroll.availableWidth
                    configuration: root.configuration; savedPresets: root.store.presets
                    presetStore: root.store
                    onPicked: (value, name) => { root.configuration = value; root.presetName = name; root.error = "" }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: 8
                Label { objectName: "cardLayoutBatchRange"; Layout.fillWidth: true; text: qsTr("Selected: %1 of %2 cards").arg(root.selectedIds.length).arg(root.cards.length); color: Theme.text; font.weight: Font.DemiBold; wrapMode: Text.WordWrap }
                ShapeCheckBox { objectName: "cardLayoutBatchAll"; text: qsTr("All cards"); checked: root.cards.length > 0 && root.selectedIds.length === root.cards.length; onToggled: root.selectAll(checked) }
                ListView {
                    Layout.fillWidth: true; Layout.fillHeight: true; clip: true; spacing: 4
                    model: root.cards
                    delegate: ShapeCheckBox {
                        id: cardRow
                        required property var modelData
                        width: ListView.view.width
                        objectName: "cardLayoutBatchCard-" + modelData.id
                        text: modelData.title; checked: root.selectedIds.indexOf(modelData.id) >= 0
                        onToggled: root.selectCard(modelData.id, checked)
                    }
                }
            }
        }
        Label { Layout.fillWidth: true; text: root.presetName.length ? qsTr("Layout: %1").arg(root.presetName) : qsTr("Choose a layout preset to continue."); color: Theme.muted; wrapMode: Text.WordWrap }
        Label { objectName: "cardLayoutBatchError"; Layout.fillWidth: true; visible: root.error.length > 0; text: root.error; color: Theme.danger; wrapMode: Text.WordWrap }
        Label { Layout.fillWidth: true; visible: !root.targetCurrent; text: qsTr("The card set changed. Reopen this window before applying a layout."); color: Theme.warning; wrapMode: Text.WordWrap }
        RowLayout {
            Layout.fillWidth: true
            ShapeButton { text: qsTr("Cancel"); quiet: true; onClicked: root.close() }
            Item { Layout.fillWidth: true }
            ShapeButton {
                objectName: "cardLayoutBatchApply"
                text: root.selectedIds.length === 1 ? qsTr("Apply to card") : qsTr("Apply to %1 cards").arg(root.selectedIds.length)
                primary: true; enabled: root.targetCurrent && root.configuration !== null && root.selectedIds.length > 0
                onClicked: root.applyRequested(root.configuration, root.selectedIds, root.targetIdentity)
            }
        }
    }
}
