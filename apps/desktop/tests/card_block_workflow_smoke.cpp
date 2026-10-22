//! Block editing crosses the real QML, draft, personal-preset and native PDF boundaries.
#include "card_block_workflow_smoke.hpp"
#include "card_kit_controller.hpp"
#include "card_layout_preset_store.hpp"
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
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QMouseEvent>
#include <QPainter>
#include <QPointer>
#include <QQmlEngine>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSvgRenderer>
#include <QTimer>
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
    require(file.open(QIODevice::WriteOnly), "write block workflow evidence");
    const auto bytes = QJsonDocument(value).toJson();
    require(file.write(bytes) == bytes.size(), "complete block workflow evidence");
}
QString literal(const QString& value) {
    const auto json = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
}
QJsonObject block(
    const char* id,
    const char* field,
    int x,
    int y,
    int width,
    int height,
    const char* fit = "contain"
) {
    return {
        {"id", id},
        {"field", field},
        {"x", x},
        {"y", y},
        {"width", width},
        {"height", height},
        {"text_scale_percent", 100},
        {"text_align", "left"},
        {"text_vertical_align", "top"},
        {"bold", false},
        {"italic", false},
        {"text_color", "#202020"},
        {"image_fit", fit}
    };
}
QVariantMap composition(CardKitController& cards) {
    return cards.draft().value("composition").toMap();
}
QVariantMap card(CardKitController& cards, const QString& id) {
    for (const auto& value : composition(cards).value("cards").toList())
        if (value.toMap().value("id").toString() == id)
            return value.toMap();
    throw std::runtime_error("block workflow stable card ID missing");
}
QVariantMap geometry(CardKitController& cards, const QString& id) {
    for (const auto& value : card(cards, "a").value("layout").toMap().value("blocks").toList())
        if (value.toMap().value("id").toString() == id)
            return value.toMap();
    throw std::runtime_error("block workflow stable block ID missing");
}
QQuickItem* visualChild(QQuickItem* item, const QString& name) {
    if (!item)
        return nullptr;
    if (item->objectName() == name)
        return item;
    for (auto* child : item->childItems())
        if (auto* found = visualChild(child, name))
            return found;
    return nullptr;
}
void pointer(QQuickWindow& window, QEvent::Type type, const QPointF& position) {
    const auto button = type == QEvent::MouseMove ? Qt::NoButton : Qt::LeftButton;
    const auto buttons = type == QEvent::MouseButtonRelease ? Qt::NoButton : Qt::LeftButton;
    QMouseEvent event(
        type,
        position,
        window.mapToGlobal(position.toPoint()),
        button,
        buttons,
        Qt::NoModifier
    );
    QCoreApplication::sendEvent(&window, &event);
    QCoreApplication::processEvents();
}
void waitForPreview(QObject& preview) {
    QElapsedTimer elapsed;
    elapsed.start();
    while (!preview.property("previewReady").toBool() && elapsed.elapsed() < 6000)
        settle();
    require(preview.property("previewReady").toBool(), "current card preview reaches Image.Ready");
}
int artworkPixels(QQuickWindow& window, QQuickItem& face, const QString& path = {}) {
    window.update();
    const auto frame = window.grabWindow();
    require(!frame.isNull(), "capture actual block editor pixels");
    if (!path.isEmpty())
        require(frame.save(path), "save block editor window evidence");
    const auto bounds = face.mapRectToScene(face.boundingRect());
    const auto scale = double(frame.width()) / window.width();
    int count = 0;
    for (int y = std::max(0, int(bounds.top() * scale));
         y < std::min(frame.height(), int(bounds.bottom() * scale));
         y += 3)
        for (int x = std::max(0, int(bounds.left() * scale));
             x < std::min(frame.width(), int(bounds.right() * scale));
             x += 3) {
            const auto pixel = frame.pixelColor(x, y);
            if (pixel.red() < 100 && pixel.green() > 120 && pixel.blue() > 100
                && pixel.green() > pixel.red() + 50)
                ++count;
        }
    return count;
}
} // namespace

QJsonObject card_block_workflow_smoke::verify(
    QObject& root,
    DesktopBackend& backend,
    CardKitController& cards,
    CardPrintController& printing,
    DeckDocumentController& documents,
    const QString& directory
) {
    require(QDir().mkpath(directory), "block workflow evidence directory");
    auto* workspace = root.findChild<QObject*>("cardKitWorkspace");
    auto* dialog = root.findChild<QObject*>("cardPrintDialog");
    require(workspace && dialog, "real block workspace and sheet dialog");
    require(
        documents.createProject(QUrl::fromLocalFile(directory), "Block authoring"),
        "isolated block authoring project"
    );
    evaluate(*workspace, "openWorkspace('')");
    const auto kit = cards.draft().value("artifact_id").toString();
    QImage image(800, 400, QImage::Format_RGBA8888);
    image.fill(QColor("#32a697"));
    {
        QPainter painter(&image);
        painter.fillRect(0, 0, 100, 400, QColor("#d66650"));
        painter.fillRect(700, 0, 100, 400, QColor("#c9ac59"));
    }
    const auto imagePath = QDir(directory).filePath("block-artwork.png");
    require(
        image.save(imagePath) && backend.importRaster(QUrl::fromLocalFile(imagePath)),
        "import an accepted image for duplicate artwork bindings"
    );
    QJsonObject artwork;
    for (const auto& value : backend.artifacts()) {
        const auto item = value.toMap();
        if (item.value("kindKey") == "image_raster")
            artwork = {
                {"artifact_id", item.value("id").toString()},
                {"revision_id", item.value("acceptedRevisionId").toString()}
            };
    }
    require(!artwork.value("revision_id").toString().isEmpty(), "block artwork exact accepted pin");
    const QJsonObject layout{
        {"mode", "blocks"},
        {"blocks",
         QJsonArray{
             block("cover", "artwork", 1000, 2500, 8000, 3500),
             block("detail", "artwork", 500, 7000, 2000, 2200, "cover"),
             block("title", "title", 1000, 600, 8000, 1300),
             block("body", "rules", 3000, 7000, 6500, 2200)
         }}
    };
    const QJsonObject request{
        {"name", "Block composition · 区块卡"},
        {"canvas_width", 750},
        {"canvas_height", 1050},
        {"layout", QJsonObject{{"mode", "text"}}},
        {"back", QJsonObject{{"mode", "solid"}, {"color", "#313131"}}},
        {"cards",
         QJsonArray{
             QJsonObject{
                 {"id", "a"},
                 {"title", "ORBIT 07"},
                 {"rules", "Quiet motion.\nA curved path."},
                 {"flavor", "Around the centre"},
                 {"quantity", 1},
                 {"artwork", artwork},
                 {"layout", layout}
             },
             QJsonObject{
                 {"id", "b"},
                 {"title", "FORM 08"},
                 {"rules", "A different card."},
                 {"quantity", 2},
                 {"artwork", artwork},
                 {"layout", QJsonObject{{"mode", "text"}}}
             }
         }}
    };
    require(
        cards.save(request.toVariantMap()) && cards.review("a") && cards.accept(),
        "block request accepted by linked Rust bridge"
    );
    const auto accepted = cards.source();
    evaluate(*workspace, "selectedCardId = 'a'; synchronize(); controller.showCard('a')");
    require(evaluate(*workspace, "openBlockEditor()").toBool(), "open actual block inspector");
    auto* editor = root.findChild<QObject*>("cardBlockEditor");
    auto* overlay = root.findChild<QObject*>("cardBlockOverlay");
    require(editor && overlay, "packaged block editor and direct manipulation overlay");

    const auto beforeGesture = composition(cards);
    settle(80);
    auto* window = qobject_cast<QQuickWindow*>(&root);
    auto* face = qobject_cast<QQuickItem*>(root.findChild<QObject*>("cardKitPreviewImage"));
    require(window && face, "real window and card preview image");
    waitForPreview(*face);
    require(
        artworkPixels(*window, *face, QDir(directory).filePath("block-editor.png")) > 100,
        "block artwork is visible in the actual editor, not only the SVG renderer"
    );
    const auto oldSource = face->property("source").toUrl();
    evaluate(*editor, "selectBlock('title'); setGeometry('x', 1100)");
    require(
        !face->property("source").toUrl().isEmpty() && artworkPixels(*window, *face) > 100,
        "saving geometry keeps the visible card while its preview is invalidated"
    );
    QElapsedTimer previewRefresh;
    previewRefresh.start();
    while (
        (face->property("source").toUrl() == oldSource || !face->property("previewReady").toBool())
        && previewRefresh.elapsed() < 6000) {
        settle();
        require(
            artworkPixels(*window, *face) > 100,
            "preview refresh never clears the current card artwork"
        );
    }
    require(
        face->property("source").toUrl() != oldSource && face->property("previewReady").toBool(),
        "refreshed preview replaces the retained image"
    );
    require(
        evaluate(*editor, "undoBlockChange()").toBool() && composition(cards) == beforeGesture,
        "preview refresh check restores its geometry change"
    );
    QPointer<QQuickItem> titleItem =
        visualChild(qobject_cast<QQuickItem*>(overlay), "cardBlockRect-title");
    require(window && titleItem && titleItem->isVisible(), "actual title hit target is visible");
    const auto start = titleItem->mapToScene(titleItem->boundingRect().center());
    pointer(*window, QEvent::MouseButtonPress, start);
    pointer(*window, QEvent::MouseMove, start + QPointF(12, 8));
    pointer(*window, QEvent::MouseMove, start + QPointF(24, 16));
    require(
        titleItem && editor->property("gestureActive").toBool()
            && composition(cards) == beforeGesture,
        "real pointer retains its delegate and mouse grab across multiple transient samples"
    );
    pointer(*window, QEvent::MouseButtonRelease, start + QPointF(24, 16));
    require(
        !editor->property("gestureActive").toBool()
            && geometry(cards, "title").value("x").toInt() > 1000,
        "real pointer release commits the displayed rectangle"
    );
    require(
        evaluate(*editor, "undoBlockChange()").toBool() && composition(cards) == beforeGesture,
        "pointer edit can be undone before exact geometry contract checks"
    );
    require(evaluate(*editor, "beginGesture('title','move')").toBool(), "begin block drag");
    evaluate(*editor, "updateGesture(350,450)");
    require(
        composition(cards) == beforeGesture && editor->property("gestureActive").toBool(),
        "gesture samples stay transient until pointer release"
    );
    require(evaluate(*editor, "finishGesture()").toBool(), "release saves block geometry once");
    require(
        geometry(cards, "title").value("x").toInt() == 1350
            && geometry(cards, "title").value("y").toInt() == 1050 && cards.source() == accepted
            && !cards.currentAccepted(),
        "basis units survive QML to Rust without replacing accepted version"
    );
    require(
        evaluate(*editor, "undoBlockChange()").toBool() && composition(cards) == beforeGesture,
        "block undo restores the complete prior layout"
    );
    require(evaluate(*editor, "beginGesture('title','move')").toBool(), "begin stale target drag");
    evaluate(*editor, "updateGesture(400,400)");
    evaluate(*workspace, "selectedCardId = 'b'");
    require(
        !evaluate(*editor, "finishGesture()").toBool() && composition(cards) == beforeGesture,
        "changing selected card cancels the old block gesture"
    );
    waitForPreview(*face);
    require(
        artworkPixels(*window, *face) == 0,
        "switching to the text-only card does not retain the previous card artwork"
    );
    evaluate(*workspace, "selectedCardId = 'a'; controller.showCard('a'); openBlockEditor()");
    evaluate(*editor, "selectBlock('title'); setGeometry('x', 1200); setBlockValue('bold', true)");
    require(
        geometry(cards, "title").value("x").toInt() == 1200
            && geometry(cards, "title").value("bold").toBool(),
        "precise geometry and typography controls use the real save service"
    );
    require(evaluate(*editor, "beginGesture('title','se')").toBool(), "begin corner resize");
    evaluate(*editor, "updateGesture(-600,300)");
    require(
        evaluate(*editor, "finishGesture()").toBool()
            && geometry(cards, "title").value("width").toInt() == 7400
            && geometry(cards, "title").value("height").toInt() == 1600,
        "corner resize changes geometry in whole-card units"
    );
    const auto beforeExtraBlock = card(cards, "a").value("layout").toMap();
    evaluate(*editor, "addBlock('flavor')");
    require(
        card(cards, "a").value("layout").toMap().value("blocks").toList().size() == 5,
        "adding a bound field block keeps existing blocks"
    );
    evaluate(*editor, "removeSelectedBlock()");
    require(
        card(cards, "a").value("layout").toMap() == beforeExtraBlock,
        "removing a block does not remove card content"
    );
    require(
        card(cards, "a").value("flavor") == "Around the centre",
        "hidden flavor content survives block removal"
    );
    evaluate(*editor, "selectBlock('title'); moveSelectedBlock(1)");
    require(
        card(cards, "a").value("layout").toMap().value("blocks").toList().last().toMap().value("id")
            == "title",
        "layer order reaches persisted layout"
    );
    require(evaluate(*editor, "undoBlockChange()").toBool(), "layer order has guarded undo");

    require(evaluate(*workspace, "openPresetSave(true)").toBool(), "save a real block preset");
    auto* saveDialog = root.findChild<QObject*>("cardPresetSaveDialog");
    auto* nameField = saveDialog ? saveDialog->findChild<QObject*>("cardPresetName") : nullptr;
    auto* library = root.property("layoutPresets").value<CardLayoutPresetStore*>();
    require(nameField && library, "block preset dialog reaches personal library");
    nameField->setProperty("text", "Native block composition");
    require(evaluate(*saveDialog, "save()").toBool(), "personal library accepts resolved blocks");
    const auto preset = library->presets().last().toMap();
    const auto presetLayout = preset.value("configuration").toMap();
    require(
        presetLayout.value("mode") == "blocks"
            && presetLayout.value("blocks")
                   == card(cards, "a").value("layout").toMap().value("blocks"),
        "saved block preset keeps complete ordered geometry"
    );
    const auto beforeBatch = card(cards, "b");
    require(
        evaluate(
            *workspace,
            "applyLayoutToCards(layoutPresets.configuration("
                + literal(preset.value("id").toString()) + "), ['b'], layoutContextIdentity())"
        )
            .toBool(),
        "block preset applies through existing atomic batch workflow"
    );
    auto originalContent = beforeBatch;
    auto appliedContent = card(cards, "b");
    require(
        appliedContent.value("layout").toMap() == presetLayout,
        "block template geometry reaches selected card"
    );
    originalContent.remove("layout");
    appliedContent.remove("layout");
    require(
        originalContent == appliedContent && cards.source() == accepted,
        "block template preserves target text, quantity and exact accepted image pins"
    );
    const auto saved = composition(cards);
    require(
        cards.load(kit) && composition(cards) == saved,
        "reopened block drafts retain geometry and per-card bindings"
    );
    evaluate(*workspace, "selectedCardId = 'a'; synchronize(); acceptAndPrint()");
    evaluate(*dialog, "sideMode = 1; flipEdge = 0; gridColumns = 2; gridRows = 2; refresh()");
    require(
        printing.ready() && printing.plan().value("page_count").toInt() == 2,
        "block faces use the existing duplex planner"
    );
    const auto requestPath = QDir(directory).filePath("block-print-request.json");
    write(
        requestPath,
        {{"action", "print_plan"},
         {"source", QJsonObject::fromVariantMap(cards.source())},
         {"options", QJsonObject::fromVariantMap(printing.plan().value("options").toMap())},
         {"imposition", QJsonObject::fromVariantMap(printing.plan().value("imposition").toMap())}}
    );
    const auto plan = documents.cli()
                          .run({"card-kit", backend.bundlePath(), requestPath})["print_plan"]
                          .toObject();
    write(QDir(directory).filePath("block-print-plan.json"), plan);
    const auto document = card_print::prepare(plan);
    const auto svg = document->cards.value("a");
    QSvgRenderer renderer(svg);
    require(renderer.isValid(), "Qt consumes core block SVG");
    QImage rendered(750, 1050, QImage::Format_RGBA8888);
    rendered.fill(Qt::white);
    {
        QPainter painter(&rendered);
        renderer.render(&painter);
    }
    const auto teal = QColor("#32a697");
    require(
        rendered.pixelColor(375, 440) == teal && rendered.pixelColor(112, 840) == teal,
        "both contain and cover artwork bindings paint through Qt SVG"
    );
    require(
        rendered.pixelColor(375, 270) == QColor(Qt::white) && rendered.pixelColor(112, 740) == teal,
        "independent artwork block fitting preserves contain margin and cover crop"
    );
    require(
        rendered.save(QDir(directory).filePath("block-face.png"))
            && card_print::preview(*document, 0).save(QDir(directory).filePath("block-sheet.png")),
        "block face and print painter evidence"
    );
    const auto pdf = QDir(directory).filePath("blocks-duplex.pdf");
    require(
        printing.exportPdf(QUrl::fromLocalFile(pdf), printing.version()),
        "export block duplex PDF"
    );
    QElapsedTimer elapsed;
    elapsed.start();
    while (printing.running() && elapsed.elapsed() < 45000)
        settle();
    require(
        !printing.running() && printing.error().isEmpty() && QFile::exists(pdf),
        "block PDF completes through the native PDF writer"
    );
    evaluate(*dialog, "closeLayout()");
    const QJsonObject report{
        {"status", "passed"},
        {"project", backend.bundlePath()},
        {"kit_id", kit},
        {"pdf", pdf},
        {"transient_gestures", true},
        {"real_pointer_delegate_retained", true},
        {"actual_editor_preview_pixels", true},
        {"preview_refresh_continuity", true},
        {"preview_card_identity_fenced", true},
        {"stale_target_fenced", true},
        {"precise_geometry_and_styles", true},
        {"block_undo", true},
        {"block_preset_batch_content_preserved", true},
        {"block_draft_reopened", true},
        {"duplicate_image_binding_qt_fit", true},
        {"block_duplex_pdf", true}
    };
    write(QDir(directory).filePath("report.json"), report);
    return report;
}
