//! End-to-end authoring and physical front/back pairing through the packaged application.
#include "card_layout_workflow_smoke.hpp"
#include "card_kit_controller.hpp"
#include "card_layout_preset_store.hpp"
#include "card_layout_preset_store_smoke.hpp"
#include "card_print_controller.hpp"
#include "card_print_export.hpp"
#include "deck_cli_client.hpp"
#include "deck_document_controller.hpp"
#include "desktop_backend.hpp"
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QQmlEngine>
#include <QQmlExpression>
#include <QQuickWindow>
#include <QTimer>
#include <cmath>
#include <stdexcept>
namespace {
void require(bool value, const char* message) {
    if (!value)
        throw std::runtime_error(message);
}
void settle(int milliseconds = 20) {
    QEventLoop loop;
    QTimer::singleShot(milliseconds, &loop, &QEventLoop::quit);
    loop.exec();
}
QVariant evaluate(QObject& owner, const QString& script) {
    QQmlExpression expression(QQmlEngine::contextForObject(&owner), &owner, script);
    const auto result = expression.evaluate();
    if (expression.hasError())
        throw std::runtime_error(expression.error().toString().toStdString());
    QCoreApplication::processEvents();
    return result;
}
void write(const QString& path, const QJsonObject& value) {
    QFile file(path);
    require(file.open(QIODevice::WriteOnly), "write layout workflow evidence");
    const auto bytes = QJsonDocument(value).toJson();
    require(file.write(bytes) == bytes.size(), "complete layout workflow evidence");
}
QString literal(const QString& value) {
    const auto json = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
}
} // namespace
QJsonObject card_layout_workflow_smoke::verify(
    QObject& root,
    DesktopBackend& backend,
    CardKitController& cards,
    CardPrintController& printing,
    DeckDocumentController& documents,
    const QString& directory
) {
    require(QDir().mkpath(directory), "layout workflow evidence directory");
    require(
        card_layout_preset_store_smoke::verify(QDir(directory).filePath("preset-store")),
        "personal presets persist and reject invalid or stale library writes"
    );
    auto* workspace = root.findChild<QObject*>("cardKitWorkspace");
    auto* dialog = root.findChild<QObject*>("cardPrintDialog");
    require(workspace && dialog, "real layout and sheet workspaces");
    require(
        documents.createProject(QUrl::fromLocalFile(directory), "Layout and duplex"),
        "isolated layout workflow project"
    );
    evaluate(*workspace, "openWorkspace('')");
    const auto kit = cards.draft().value("artifact_id").toString();
    QJsonObject composition{
        {"name", "Pairing · 正反面"},
        {"layout", QJsonObject{{"mode", "text"}}},
        {"back", QJsonObject{{"mode", "solid"}, {"color", "#313131"}}},
        {"cards",
         QJsonArray{
             QJsonObject{{"id", "a"}, {"title", "A"}, {"rules", "First · 第一"}, {"quantity", 1}},
             QJsonObject{{"id", "b"}, {"title", "B"}, {"rules", "Second · 第二"}, {"quantity", 1}},
             QJsonObject{{"id", "c"}, {"title", "C"}, {"rules", "Third · 第三"}, {"quantity", 1}}
         }}
    };
    require(
        cards.save(composition.toVariantMap()) && cards.review("a") && cards.accept(),
        "save and explicitly accept a shared card back"
    );
    const auto acceptedBeforeBatch = cards.source();
    evaluate(
        *workspace,
        "selectedCardId = 'a'; synchronize(); setCardField('layout', "
        "{mode:'text',text_scale_percent:130,text_offset_x_percent:2}); flush(); "
        "controller.showCard('a')"
    );
    require(
        evaluate(*workspace, "openPresetSave(true)").toBool(),
        "open save-resolved-layout dialog"
    );
    auto* saveDialog = root.findChild<QObject*>("cardPresetSaveDialog");
    auto* nameField = saveDialog ? saveDialog->findChild<QObject*>("cardPresetName") : nullptr;
    auto* library = root.property("layoutPresets").value<CardLayoutPresetStore*>();
    require(saveDialog && nameField && library, "actual preset saving UI and personal store");
    nameField->setProperty("text", "Native custom preset");
    require(evaluate(*saveDialog, "save()").toBool(), "save a named resolved layout from the UI");
    const auto preset = library->presets().last().toMap();
    require(
        preset.value("configuration").toMap().value("text_scale_percent").toInt() == 130,
        "preset captures the effective per-card layout"
    );
    const auto beforeBatch = cards.draft().value("composition").toMap();
    require(evaluate(*workspace, "openBatchLayout()").toBool(), "open real batch selector");
    auto* batchDialog = root.findChild<QObject*>("cardLayoutBatchDialog");
    require(batchDialog, "batch selector component");
    evaluate(
        *batchDialog,
        "selectAll(false); selectCard('b',true); selectCard('c',true); configuration = "
        "store.configuration("
            + literal(preset.value("id").toString()) + ")"
    );
    require(
        evaluate(
            *workspace,
            "applyLayoutToCards(layoutPresets.presets[layoutPresets.presets.length-1]."
            "configuration, ['b','c'], layoutContextIdentity())"
        )
            .toBool(),
        "apply the saved preset to two stable card IDs in one operation"
    );
    auto afterBatch = cards.draft().value("composition").toMap();
    const auto beforeCards = beforeBatch.value("cards").toList();
    const auto afterCards = afterBatch.value("cards").toList();
    require(
        beforeCards.size() == afterCards.size() && beforeCards[0] == afterCards[0]
            && cards.source() == acceptedBeforeBatch && !cards.currentAccepted(),
        "batch preserves the untargeted card and the accepted version"
    );
    for (int i = 1; i < afterCards.size(); ++i) {
        auto before = beforeCards[i].toMap();
        auto after = afterCards[i].toMap();
        require(
            after.value("layout").toMap() == preset.value("configuration").toMap(),
            "selected cards receive the complete saved configuration"
        );
        before.remove("layout");
        after.remove("layout");
        require(before == after, "batch retains IDs, text, quantities and material references");
    }
    require(
        evaluate(*workspace, "undoLayoutBatch()").toBool()
            && cards.draft().value("composition").toMap() == beforeBatch,
        "one undo restores every targeted layout together"
    );
    require(
        evaluate(
            *workspace,
            "applyLayoutToCards(layoutPresets.presets[layoutPresets.presets.length-1]."
            "configuration, ['b','c'], layoutContextIdentity())"
        )
            .toBool(),
        "reapply for stale undo verification"
    );
    const auto staleContext = evaluate(*workspace, "layoutContextIdentity()").toString();
    evaluate(
        *workspace,
        "selectedCardId = 'b'; setCardField('rules', 'Later edit must survive'); flush()"
    );
    const auto later = cards.draft().value("composition").toMap();
    require(
        !evaluate(*workspace, "undoLayoutBatch()").toBool()
            && !evaluate(
                    *workspace,
                    "applyLayoutToCards(layoutPresets.presets[layoutPresets.presets.length-1]."
                    "configuration, ['b'], "
                        + literal(staleContext) + ")"
            )
                    .toBool()
            && cards.draft().value("composition").toMap() == later,
        "later edits fence stale batch application and undo"
    );
    require(cards.save(composition.toVariantMap()), "restore independent duplex fixture");
    evaluate(*workspace, "openSetLayout()");
    auto* backEditor = root.findChild<QObject*>("cardKitBackEditor");
    require(backEditor, "whole-set shared back editor");
    evaluate(*backEditor, "chooseMode(1); setValue('color','#414141')");
    require(
        evaluate(*workspace, "flush()").toBool()
            && cards.draft().value("composition").toMap().value("back").toMap().value("color")
                   == "#414141",
        "whole-set back controls save through the same draft service"
    );
    require(cards.review("a") && cards.accept(), "explicitly accept the edited shared back");
    const auto source = cards.source();
    require(cards.load(kit), "reopen authored shared back");
    require(
        cards.draft().value("composition").toMap().value("back").toMap().value("color")
            == "#414141",
        "card back survives a saved draft reopen"
    );
    evaluate(*workspace, "selectedCardId = 'a'; synchronize(); acceptAndPrint()");
    evaluate(*dialog, "sideMode = 1; flipEdge = 0; gridColumns = 2; gridRows = 2; refresh()");
    require(
        printing.ready() && printing.plan().value("total_copies").toInt() == 3
            && printing.plan().value("total_placements").toInt() == 6
            && printing.plan().value("sheet_count").toInt() == 1
            && printing.plan().value("page_count").toInt() == 2,
        "duplex separates physical cards, placements, sheets and PDF pages"
    );
    const auto requestPath = QDir(directory).filePath("duplex-request.json");
    write(
        requestPath,
        {{"action", "print_plan"},
         {"source", QJsonObject::fromVariantMap(source)},
         {"options", QJsonObject::fromVariantMap(printing.plan().value("options").toMap())},
         {"imposition", QJsonObject::fromVariantMap(printing.plan().value("imposition").toMap())}}
    );
    const auto plan = documents.cli()
                          .run({"card-kit", backend.bundlePath(), requestPath})["print_plan"]
                          .toObject();
    write(QDir(directory).filePath("duplex-plan.json"), plan);
    const auto document = card_print::prepare(plan);
    auto rejects = [](const QJsonObject& input) {
        try {
            card_print::prepare(input);
        } catch (const std::exception&) {
            return true;
        }
        return false;
    };
    auto wrongCount = plan;
    wrongCount["total_placements"] = 5;
    require(rejects(wrongCount), "consumer rejects mismatched duplex placement counts");
    auto wrongFace = plan;
    auto wrongPages = plan["pages"].toArray();
    auto wrongBack = wrongPages[1].toObject();
    auto wrongSlots = wrongBack["slots"].toArray();
    auto wrongSlot = wrongSlots[0].toObject();
    wrongSlot["resource_id"] = "a";
    wrongSlots[0] = wrongSlot;
    wrongBack["slots"] = wrongSlots;
    wrongPages[1] = wrongBack;
    wrongFace["pages"] = wrongPages;
    require(rejects(wrongFace), "consumer cannot draw a front SVG as a shared card back");
    auto legacy = plan;
    auto legacyPage = plan["pages"].toArray()[0].toObject();
    legacyPage.remove("side");
    legacyPage.remove("sheet_index");
    QJsonArray legacySlots;
    for (const auto& value : legacyPage["slots"].toArray()) {
        auto slot = value.toObject();
        slot.remove("face");
        slot.remove("resource_id");
        legacySlots.append(slot);
    }
    legacyPage["slots"] = legacySlots;
    QJsonArray legacyResources;
    for (const auto& resource : plan["cards"].toArray())
        if (resource.toObject()["id"] != "@deck/back")
            legacyResources.append(resource);
    legacy["cards"] = legacyResources;
    legacy["pages"] = QJsonArray{legacyPage};
    legacy["page_count"] = 1;
    legacy["duplex"] = false;
    legacy.remove("imposition");
    legacy.remove("total_placements");
    legacy.remove("sheet_count");
    require(
        card_print::prepare(legacy)->pages.size() == 1,
        "consumer retains the original single-sided plan contract"
    );
    require(
        document->pages.size() == 2 && document->pages[0].side == "front"
            && document->pages[1].side == "back" && document->cards.size() == 4,
        "one shared back resource accompanies an interleaved front and back"
    );
    const auto& front = document->pages[0];
    const auto& back = document->pages[1];
    require(
        front.placements.size() == 3 && back.placements.size() == 3,
        "partial sheet retains an empty slot on both sides"
    );
    for (qsizetype i = 0; i < front.placements.size(); ++i) {
        const auto& f = front.placements[i];
        const auto& b = back.placements[i];
        require(
            f.card_id == b.card_id && f.copy_index == b.copy_index && b.resource_id == "@deck/back"
                && b.face == "back"
                && std::abs(b.rect_mm.x() - (document->sheet_mm.width() - f.rect_mm.right()))
                       < 0.002
                && std::abs(b.rect_mm.y() - f.rect_mm.y()) < 0.002,
            "long-edge backs preserve copy identity and mirror the physical slot, including holes"
        );
    }
    require(
        card_print::preview(*document, 0).save(QDir(directory).filePath("front.png"))
            && card_print::preview(*document, 1).save(QDir(directory).filePath("back.png")),
        "real Qt painter renders both sides"
    );
    require(
        printing.showPage(1) && printing.pageSide() == "back" && printing.sheetIndex() == 0,
        "preview navigation identifies the actual back of sheet one"
    );
    evaluate(*dialog, "flipEdge = 1; backOffsetX = 0.5; backOffsetY = -0.5; refresh()");
    require(printing.ready(), "small back calibration may use blank paper margin");
    evaluate(*dialog, "closeLayout()");
    evaluate(*workspace, "acceptAndPrint()");
    require(
        dialog->property("sideMode").toInt() == 1 && dialog->property("flipEdge").toInt() == 1
            && dialog->property("backOffsetX").toDouble() == 0.5
            && dialog->property("backOffsetY").toDouble() == -0.5,
        "duplex mode, binding edge and calibration survive returning to cards"
    );
    const auto pdf = QDir(directory).filePath("duplex-native.pdf");
    require(
        printing.exportPdf(QUrl::fromLocalFile(pdf), printing.version()),
        "start duplex PDF export"
    );
    QElapsedTimer elapsed;
    elapsed.start();
    while (printing.running() && elapsed.elapsed() < 45000)
        settle();
    require(
        !printing.running() && printing.error().isEmpty() && QFile::exists(pdf),
        "duplex PDF export completes through the same painter"
    );
    evaluate(*dialog, "closeLayout()");
    const QJsonObject report{
        {"status", "passed"},
        {"project", backend.bundlePath()},
        {"kit_id", kit},
        {"duplex_pdf", pdf},
        {"duplex_physical_identity", true},
        {"duplex_partial_sheet", true},
        {"back_persistence", true},
        {"back_calibration", true},
        {"duplex_session_choices", true},
        {"saved_preset_ui", true},
        {"batch_content_preserved", true},
        {"batch_atomic_undo", true},
        {"stale_batch_fenced", true}
    };
    write(QDir(directory).filePath("report.json"), report);
    return report;
}
