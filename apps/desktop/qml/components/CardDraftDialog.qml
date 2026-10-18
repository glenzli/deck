pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Shape.Desktop

ShapeDialog {
    id: root
    objectName: "deckAssistantDialog"
    required property CardDraftAssistant assistant
    property string currentIdentity: ""
    property int availableCards: 16
    property string language: "en"
    readonly property bool currentTarget: assistant.targetIdentity === currentIdentity
    signal appendRequested(var rows, string identity)
    signal settingsRequested()
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(760, parent.width - 40)
    height: Math.min(700, parent.height - 40)
    modal: true
    title: qsTr("Create cards with AI")
    onClosed: assistant.discard()
    contentItem: ColumnLayout {
        spacing: 14
        Label { Layout.fillWidth: true; text: qsTr("Describe your topic, audience and card format. Review the suggestions before adding them to this set."); color: Theme.muted; wrapMode: Text.WordWrap }
        ShapeTextEditor { id: brief; objectName: "deckAssistantBrief"; Layout.fillWidth: true; Layout.preferredHeight: 110; enabled: !root.assistant.busy; wrapMode: TextEdit.Wrap; placeholderText: qsTr("For example: six vocabulary cards about the seasons for a seven-year-old…") }
        RowLayout {
            Label { text: qsTr("Cards"); color: Theme.text }
            ShapeSpinBox { id: count; objectName: "deckAssistantCount"; from: 1; to: Math.max(1, Math.min(16, root.availableCards)); value: Math.min(6,to); enabled: !root.assistant.busy; editable: true }
            ShapeComboBox { objectName: "deckAssistantLanguage"; model: ["English", "简体中文"]; selectedIndex: root.language === "zh_CN" ? 1 : 0; onActivated: index => root.language = index === 1 ? "zh_CN" : "en"; enabled: !root.assistant.busy }
            Item { Layout.fillWidth: true }
            ShapeButton { objectName: "deckAssistantGenerate"; iconSource: "../../icons/sparkle.svg"; primary: true; text: root.assistant.cards.length ? qsTr("Generate again") : qsTr("Generate cards"); enabled: !root.assistant.busy && brief.text.trim().length > 0 && root.availableCards > 0; onClicked: root.assistant.generate(brief.text, count.value, root.language, root.currentIdentity) }
        }
        RowLayout {
            Layout.fillWidth: true
            BusyIndicator { implicitWidth: 20; implicitHeight: 20; visible: root.assistant.busy; running: visible }
            Label { Layout.fillWidth: true; text: root.assistant.error || root.assistant.status; color: root.assistant.error.length ? Theme.danger : Theme.muted; wrapMode: Text.WordWrap; font.pixelSize: 11 }
            ShapeIconButton { source: "../../icons/settings.svg"; toolTipText: qsTr("Model connection"); enabled: !root.assistant.busy; onClicked: root.settingsRequested() }
        }
        Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
        ScrollView {
            id: candidateScroll
            Layout.fillWidth: true; Layout.fillHeight: true; clip: true
            contentWidth: availableWidth
            ColumnLayout {
                width: candidateScroll.availableWidth; spacing: 12
                Repeater {
                    model: root.assistant.cards
                    delegate: Rectangle {
                        id: candidateRow
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true; implicitHeight: copy.implicitHeight + 24; radius: 8; color: Theme.panelRaised; border.color: Theme.border
                        ColumnLayout {
                            id: copy; anchors.fill: parent; anchors.margins: 12; spacing: 6
                            Label { Layout.fillWidth: true; text: (candidateRow.index + 1) + ". " + candidateRow.modelData.title; color: Theme.text; font.weight: Font.DemiBold; wrapMode: Text.WordWrap }
                            Label { Layout.fillWidth: true; text: candidateRow.modelData.rules; color: Theme.textSoft; wrapMode: Text.WordWrap }
                        }
                    }
                }
                Label { Layout.fillWidth: true; visible: !root.assistant.cards.length; text: qsTr("Suggestions appear here. Your existing cards stay as they are."); color: Theme.muted; wrapMode: Text.WordWrap; topPadding: 30; horizontalAlignment: Text.AlignHCenter }
            }
        }
        Label { Layout.fillWidth: true; visible: root.assistant.cards.length > 0 && !root.currentTarget; text: qsTr("The card set changed. Generate again for the current draft."); color: Theme.warning; wrapMode: Text.WordWrap }
        RowLayout {
            Layout.fillWidth: true
            ShapeButton { objectName: "deckAssistantClose"; quiet: true; text: root.assistant.running ? qsTr("Cancel generation") : qsTr("Back to cards"); onClicked: root.close() }
            Item { Layout.fillWidth: true }
            ShapeButton { objectName: "deckAssistantApply"; iconSource: "../../icons/add.svg"; primary: true; text: root.assistant.cards.length === 1 ? qsTr("Add 1 card") : qsTr("Add %1 cards").arg(root.assistant.cards.length); enabled: !root.assistant.busy && root.assistant.cards.length > 0 && root.currentTarget; onClicked: root.appendRequested(root.assistant.cards, root.assistant.targetIdentity) }
        }
    }
}
