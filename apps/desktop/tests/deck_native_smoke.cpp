#include "deck_native_smoke.hpp"
#include "card_block_workflow_smoke.hpp"
#include "card_draft_assistant_smoke.hpp"
#include "card_kit_controller.hpp"
#include "card_layout_workflow_smoke.hpp"
#include "card_print_controller.hpp"
#include "card_print_export.hpp"
#include "deck_cli_client.hpp"
#include "deck_document_controller.hpp"
#include "deck_exchange_controller.hpp"
#include "desktop_backend.hpp"
#include "external_image_task_controller.hpp"
#include "material_edit_controller.hpp"
#include "ui_preferences.hpp"
#include <QCoreApplication>
#include <QCryptographicHash>
#include <QDebug>
#include <QDesktopServices>
#include <QDir>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QFile>
#include <QGuiApplication>
#include <QIcon>
#include <QImage>
#include <QJSValue>
#include <QJsonArray>
#include <QJsonDocument>
#include <QMouseEvent>
#include <QPainter>
#include <QProcess>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QSvgRenderer>
#include <QTimer>
#include <algorithm>
#include <cmath>
#include <filesystem>
#include <functional>
#include <stdexcept>
namespace {
QString sha256(const QString& path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return {};
    QCryptographicHash hash(QCryptographicHash::Sha256);
    if (!hash.addData(&file))
        return {};
    return QString::fromLatin1(hash.result().toHex());
}
void require(bool value, const char* message) {
    if (!value)
        throw std::runtime_error(message);
}
void verifyDashedCutGuides(const card_print::Document& document) {
    auto isolated = document;
    for (auto it = isolated.cards.begin(); it != isolated.cards.end(); ++it)
        it.value() = QByteArrayLiteral(
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100'>"
            "<rect width='100' height='100' fill='white'/></svg>"
        );
    const auto image = card_print::preview(isolated, 0);
    int inkPixels = 0;
    for (int y = 0; y < image.height(); ++y)
        for (int x = 0; x < image.width(); ++x) {
            const auto pixel = image.pixelColor(x, y);
            require(pixel.red() >= 130, "cut guides remain light rather than solid black");
            if (pixel.red() < 245)
                ++inkPixels;
        }
    require(inkPixels > 100, "light cut guides are visible on paper");
    const auto line = isolated.pages.first().crop_marks.first();
    const auto scale = 1200. / std::max(document.sheet_mm.width(), document.sheet_mm.height());
    const auto samples = int(line.length() * scale);
    int drawn = 0, breaks = 0;
    for (int i = 1; i < samples; ++i) {
        const auto point = line.pointAt(double(i) / samples) * scale;
        const auto pixel = image.pixelColor(
            qBound(0, qRound(point.x()), image.width() - 1),
            qBound(0, qRound(point.y()), image.height() - 1)
        );
        if (pixel.red() < 245)
            ++drawn;
        if (pixel.red() > 250)
            ++breaks;
    }
    require(drawn > 2 && breaks > 2, "cut guides contain both dashes and paper breaks");
}
void settle(int milliseconds = 100) {
    QEventLoop loop;
    QTimer::singleShot(milliseconds, &loop, &QEventLoop::quit);
    loop.exec();
}
bool wait(const std::function<bool()>& done, int limit = 45000) {
    QElapsedTimer elapsed;
    elapsed.start();
    while (!done() && elapsed.elapsed() < limit)
        settle(20);
    return done();
}
QVariant evaluate(QObject& owner, const QString& script) {
    QQmlExpression expression(QQmlEngine::contextForObject(&owner), &owner, script);
    const auto result = expression.evaluate();
    if (expression.hasError())
        throw std::runtime_error(expression.error().toString().toStdString());
    QCoreApplication::processEvents();
    return result;
}
QVariant value(QObject& owner, const char* name) {
    const auto result = owner.property(name);
    return result.canConvert<QJSValue>() ? result.value<QJSValue>().toVariant() : result;
}
QString literal(const QString& input) {
    const auto json = QJsonDocument(QJsonArray{input}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
}
QQuickItem* visualChild(QQuickItem* owner, const QString& name) {
    if (!owner)
        return nullptr;
    if (owner->objectName() == name)
        return owner;
    for (auto* item : owner->childItems())
        if (auto* found = visualChild(item, name))
            return found;
    return nullptr;
}
QObject& child(QObject& owner, const char* name) {
    QObject* found = owner.findChild<QObject*>(QString::fromLatin1(name));
    if (!found) {
        auto* window = qobject_cast<QQuickWindow*>(&owner);
        found = visualChild(
            window ? window->contentItem() : qobject_cast<QQuickItem*>(&owner),
            QString::fromLatin1(name)
        );
    }
    require(found, name);
    return *found;
}
QJsonObject
verifyIcon(QObject& button, QQuickWindow& window, const QImage& capture, const QString& evidence) {
    auto* hitArea = qobject_cast<QQuickItem*>(&button);
    auto* glyph = qobject_cast<QQuickItem*>(&child(button, "buttonIcon"));
    const auto size = button.property("iconSize").toInt();
    require(
        hitArea && glyph && glyph->width() == size && glyph->height() == size
            && hitArea->width() >= size + 10 && hitArea->height() >= size + 10,
        "fixed-size icon leaves a generous hit area without stretching"
    );
    const auto scale = double(capture.width()) / window.width();
    const auto logical = glyph->mapRectToScene(glyph->boundingRect());
    const QRectF pixels(
        logical.x() * scale,
        logical.y() * scale,
        logical.width() * scale,
        logical.height() * scale
    );
    const auto bounds = pixels.toAlignedRect();
    const auto backgroundPoint = hitArea->mapToScene(QPointF(2, hitArea->height() / 2));
    const auto background = capture.pixelColor(
        qRound(backgroundPoint.x() * scale),
        qRound(backgroundPoint.y() * scale)
    );
    QImage reference(bounds.size(), QImage::Format_ARGB32_Premultiplied);
    reference.fill(Qt::transparent);
    const auto source =
        QQmlEngine::contextForObject(glyph)->resolvedUrl(glyph->property("source").toUrl());
    QSvgRenderer renderer(":" + source.path());
    require(renderer.isValid(), "toolbar icon is a packaged vector");
    {
        QPainter painter(&reference);
        painter.setRenderHint(QPainter::Antialiasing);
        renderer.render(&painter, pixels.translated(-bounds.topLeft()));
        painter.setCompositionMode(QPainter::CompositionMode_SourceIn);
        painter.fillRect(reference.rect(), glyph->property("color").value<QColor>());
    }
    QImage expected(bounds.size(), QImage::Format_ARGB32_Premultiplied);
    expected.fill(background);
    {
        QPainter painter(&expected);
        painter.drawImage(0, 0, reference);
    }
    // Compare actual pixels to a direct SVG render at the window's physical scale.
    // This detects clipped, stretched or low-resolution cached icon artwork.
    double error = 0;
    for (int y = 0; y < bounds.height(); ++y)
        for (int x = 0; x < bounds.width(); ++x) {
            const auto actual = capture.pixelColor(bounds.x() + x, bounds.y() + y);
            const auto wanted = expected.pixelColor(x, y);
            error +=
                (std::abs(actual.red() - wanted.red()) + std::abs(actual.green() - wanted.green())
                 + std::abs(actual.blue() - wanted.blue()))
                / 3.0;
        }
    error /= bounds.width() * bounds.height();
    if (error >= 6) {
        capture.copy(bounds).save(QDir(evidence).filePath("icon-actual.png"));
        expected.save(QDir(evidence).filePath("icon-expected.png"));
        throw std::runtime_error(
            QString("Icon %1 at %2x (source DPR %3) differs from the native SVG render: %4")
                .arg(button.objectName())
                .arg(scale)
                .arg(glyph->property("pixelRatio").toDouble())
                .arg(error)
                .toStdString()
        );
    }
    return {
        {"control", button.objectName()},
        {"glyph_size", size},
        {"pixel_ratio", scale},
        {"mean_pixel_error", error}
    };
}
void click(QQuickWindow& window, QObject& control) {
    auto* item = qobject_cast<QQuickItem*>(&control);
    if (!item || !item->isVisible() || !item->isEnabled())
        throw std::runtime_error(
            (QStringLiteral("visible interactive control: ") + control.objectName()).toStdString()
        );
    const auto position = item->mapToScene(item->boundingRect().center());
    require(
        QRectF(0, 0, window.width(), window.height()).contains(position),
        "interaction within window"
    );
    const auto global = window.mapToGlobal(position.toPoint());
    QMouseEvent
        move(QEvent::MouseMove, position, global, Qt::NoButton, Qt::NoButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&window, &move);
    QMouseEvent press(
        QEvent::MouseButtonPress,
        position,
        global,
        Qt::LeftButton,
        Qt::LeftButton,
        Qt::NoModifier
    );
    QCoreApplication::sendEvent(&window, &press);
    QMouseEvent release(
        QEvent::MouseButtonRelease,
        position,
        global,
        Qt::LeftButton,
        Qt::NoButton,
        Qt::NoModifier
    );
    QCoreApplication::sendEvent(&window, &release);
    settle(80);
}
int visibleButtons(QQuickItem* item, QQuickWindow& window) {
    if (!item || !item->isVisible())
        return 0;
    int count = item->metaObject()->indexOfSignal("clicked()") >= 0 && item->width() > 0
                        && item->height() > 0
                        && QRectF(0, 0, window.width(), window.height())
                               .intersects(item->mapRectToScene(item->boundingRect()))
                    ? 1
                    : 0;
    for (auto* child : item->childItems())
        count += visibleButtons(child, window);
    return count;
}
void write(const QString& path, const QJsonObject& object) {
    QFile file(path);
    require(file.open(QIODevice::WriteOnly), "evidence file");
    file.write(QJsonDocument(object).toJson(QJsonDocument::Indented));
}
QJsonObject pin(DesktopBackend& backend, const QString& id) {
    for (const auto& value : backend.artifacts()) {
        const auto item = value.toMap();
        if (item["id"].toString() == id)
            return {{"artifact_id", id}, {"revision_id", item["acceptedRevisionId"].toString()}};
    }
    return {};
}
} // namespace
bool deck_native_smoke::verify(
    QObject& root,
    DesktopBackend& backend,
    CardKitController& cards,
    CardPrintController& printing,
    MaterialEditController& material,
    DeckDocumentController& documents,
    ExternalImageTaskController& imageTask,
    UiPreferences& preferences,
    const QString& directory
) {
    try {
        require(QDir().mkpath(directory), "evidence directory");
        auto* window = qobject_cast<QQuickWindow*>(&root);
        require(window, "actual Deck window");
        require(
            window->flags().testFlag(Qt::ExpandedClientAreaHint)
                && window->flags().testFlag(Qt::NoTitleBarBackgroundHint),
            "native title area and toolbar share the window surface"
        );
        auto& titleBar = child(root, "titleToolBar");
        require(
            preferences.appearanceMode() == 1 && !preferences.dark(),
            "fresh Deck profile defaults to light appearance"
        );
        const QImage branding(QStringLiteral(":/branding/app-icon.png"));
        require(
            !branding.isNull() && branding.size() == QSize(1024, 1024)
                && !QGuiApplication::windowIcon().isNull() && !window->icon().isNull(),
            "approved application icon is packaged and assigned to the window"
        );
        for (const auto size : {16, 32, 64, 128}) {
            const auto icon = QGuiApplication::windowIcon().pixmap(QSize(size, size));
            require(
                !icon.isNull()
                    && icon.save(QDir(directory).filePath(QString("qt-app-icon-%1.png").arg(size))),
                "Qt application icon resolves at small sizes"
            );
        }
        auto& workspace = child(root, "cardKitWorkspace");
        require(qobject_cast<QQuickItem*>(&workspace), "card editor is embedded Page");
        require(
            documents.createProject(QUrl::fromLocalFile(directory), "Deck 原生 # cards"),
            "new card-only Deck project"
        );
        evaluate(workspace, "openWorkspace('')");
        require(!cards.draft().value("artifact_id").toString().isEmpty(), "new recoverable kit");
        const auto kit = cards.draft().value("artifact_id").toString();
        const auto project = backend.bundlePath();
        const auto initialState = documents.cli().run({"status", project});
        require(
            initialState["open_guard"].toObject()["allowed"].toBool(),
            "card-only open admitted"
        );
        for (const auto& artifact : backend.artifacts())
            require(
                artifact.toMap()["kindKey"] == "card_kit",
                "new Deck has no speech defaults or unrelated artifacts"
            );
        window->resize(1280, 840);
        preferences.setAppearanceMode(1);
        preferences.setLanguageMode("en");
        settle(150);
        evaluate(root, "showProjects()");
        require(
            !documents.openLocalProject(QDir(directory).filePath("missing.deck"))
                && child(root, "deckProjectOpenError").property("visible").toBool()
                && !child(root, "deckProjectOpenError").property("text").toString().isEmpty(),
            "project-open failures are visible inside the active project picker"
        );
        evaluate(child(root, "deckProjectsDialog"), "close()");
        evaluate(root, "showProjects()");
        require(documents.error().isEmpty(), "a fresh project attempt clears the old error");
        evaluate(child(root, "deckProjectsDialog"), "close()");
        require(
            child(root, "cardKitPrint").property("text").toString().isEmpty()
                && child(root, "cardKitPrint").property("toolTipText").toString()
                       == "Accept & print…"
                && child(root, "cardKitPrint").property("accessibleName").toString()
                       == "Accept & print…",
            "icon-only printing keeps explicit acceptance in its tooltip and accessible name"
        );
        click(*window, child(root, "cardKitAddMenuButton"));
        require(
            child(root, "cardKitAddMenu").property("visible").toBool(),
            "single labeled add menu opens"
        );
        click(*window, child(root, "cardKitAddCard"));
        const auto addedId = workspace.property("selectedCardId").toString();
        require(value(workspace, "cards").toList().size() == 2, "menu adds a blank stable-ID card");
        click(*window, child(root, ("cardKitRowMenu-" + addedId).toUtf8().constData()));
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-card-menu.png")),
            "styled card menu screenshot"
        );
        // Popup coordinates are relative to their visual parent; compare in window space.
        auto* rowAnchor = qobject_cast<QQuickItem*>(
            &child(root, ("cardKitRowMenu-" + addedId).toUtf8().constData())
        );
        auto* firstAction = qobject_cast<QQuickItem*>(&child(root, "deckDuplicateCardMenu"));
        const auto anchorRect = rowAnchor->mapRectToScene(rowAnchor->boundingRect());
        const auto actionRect = firstAction->mapRectToScene(firstAction->boundingRect());
        require(
            std::abs(actionRect.left() - anchorRect.left()) <= 16
                && actionRect.top() >= anchorRect.bottom()
                && actionRect.top() <= anchorRect.bottom() + 20,
            "card action menu stays anchored beside its row"
        );
        click(*window, child(root, "cardKitMoveUp"));
        require(
            value(workspace, "cards").toList().first().toMap().value("id").toString() == addedId
                && workspace.property("selectedCardId").toString() == addedId,
            "row menu reorders the selected stable card"
        );
        click(*window, child(root, ("cardKitRowMenu-" + addedId).toUtf8().constData()));
        click(*window, child(root, "cardKitRemoveCard"));
        require(
            value(workspace, "cards").toList().size() == 1,
            "row action removes only the selected card"
        );
        click(*window, child(root, "deckLayoutTab"));
        click(*window, child(root, "cardKitSetLayout"));
        require(
            qobject_cast<QQuickItem*>(&child(root, "cardKitSetLayoutDialog"))
                && child(root, "cardKitShape").property("visible").toBool(),
            "whole-set layout is editable inline beside the card"
        );
        const auto cardIdsBeforePreset = value(workspace, "cards").toList();
        click(*window, child(child(root, "cardKitDefaultLayoutOptions"), "cardPreset-study"));
        require(
            value(workspace, "composition").toMap().value("layout").toMap().value("text_align")
                    == "left"
                && value(workspace, "cards").toList() == cardIdsBeforePreset,
            "visual preset changes only layout and preserves card content and IDs"
        );
        click(*window, child(root, "deckCardLayoutScope"));
        click(*window, child(child(root, "cardKitCardLayoutOptions"), "cardPreset-caption"));
        require(
            evaluate(workspace, "flush()").toBool()
                && cards.review(workspace.property("selectedCardId").toString()),
            "missing-art preset saves through the ordinary draft owner"
        );
        settle(150);
        require(
            cards.preview().value("svg").toString().contains("image-placeholder")
                && !cards.preview().value("ready_to_export").toBool(),
            "the native preview reserves missing artwork without claiming export readiness"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-image-placeholder.png")),
            "actual missing-art placeholder screenshot"
        );
        click(*window, child(child(root, "cardKitCardLayoutOptions"), "cardPreset-clean-text"));
        click(*window, child(root, "deckContentTab"));
        click(*window, child(root, "deckImageMenuButton"));
        click(*window, child(root, "cardKitGenerateArtwork"));
        require(
            child(root, "deckShapeImageDialog").property("visible").toBool(),
            "image menu reaches real Shape task UI"
        );
        click(*window, child(root, "deckShapeBack"));
        click(*window, child(root, "deckProjectMenuButton"));
        click(*window, child(root, "deckSettings"));
        require(
            child(root, "deckSettingsDialog").property("visible").toBool(),
            "project menu reaches settings"
        );
        click(*window, child(root, "deckSettingsDone"));
        QImage fixture(640, 400, QImage::Format_RGBA8888);
        fixture.fill(QColor("#e9c967"));
        {
            QPainter painter(&fixture);
            painter.fillRect(0, 0, 640, 36, QColor("#dc4747"));
            painter.fillRect(0, 364, 640, 36, QColor("#486bc7"));
            painter.fillRect(0, 0, 36, 400, QColor("#4da66b"));
            painter.fillRect(604, 0, 36, 400, QColor("#9b5ec7"));
            painter.fillRect(160, 90, 320, 220, QColor("#4b9ba4"));
        }
        const auto imagePath = QDir(directory).filePath("card-face.png");
        require(fixture.save(imagePath), "bounded four-edge source image");
        require(backend.importRaster(QUrl::fromLocalFile(imagePath)), "actual raster import");
        QJsonObject artwork;
        for (const auto& artifact : backend.artifacts()) {
            const auto map = artifact.toMap();
            if (map["kindKey"] == "image_raster")
                artwork = pin(backend, map["id"].toString());
        }
        require(!artwork["revision_id"].toString().isEmpty(), "exact accepted local image pin");
        auto composition = QJsonObject::fromVariantMap(cards.draft().value("composition").toMap());
        composition["name"] = "Season cards · 季节卡";
        composition["layout"] = QJsonObject{{"mode", "auto"}};
        composition["cards"] = QJsonArray{
            QJsonObject{
                {"id", "spring"},
                {"title", "SPRING"},
                {"rules", "春"},
                {"quantity", 6},
                {"layout", QJsonObject{{"mode", "text"}, {"text_scale_percent", 200}}}
            },
            QJsonObject{
                {"id", "landscape"},
                {"title", "A complete image"},
                {"rules", "Preserved hidden text"},
                {"quantity", 4},
                {"artwork", artwork},
                {"layout", QJsonObject{{"mode", "image"}, {"image_fit", "contain"}}}
            },
            QJsonObject{
                {"id", "picture-text"},
                {"title", "Look and learn"},
                {"rules", "Find the colored edges.\n寻找四边的颜色。"},
                {"quantity", 2},
                {"artwork", artwork},
                {"layout",
                 QJsonObject{
                     {"mode", "image_text"},
                     {"image_fit", "cover"},
                     {"image_position", "left"},
                     {"image_share_percent", 35},
                     {"text_vertical_align", "center"},
                     {"text_offset_x_percent", 1},
                     {"text_offset_y_percent", -1},
                     {"padding_percent", 10}
                 }}
            }
        };
        require(
            cards.save(composition.toVariantMap()) && cards.review("spring") && cards.accept(),
            "save review explicit accept"
        );
        composition = QJsonObject::fromVariantMap(cards.draft().value("composition").toMap());
        const auto source = cards.source();
        require(cards.currentAccepted(), "exact accepted state");
        const auto count = backend.artifactCount();
        evaluate(workspace, "selectedCardId = 'picture-text'; synchronize()");
        QJsonArray uiEvidence;
        for (const auto& size : {QSize(1280, 840), QSize(1120, 720), QSize(980, 680)}) {
            window->resize(size);
            for (const auto appearance : {1, 2}) {
                preferences.setAppearanceMode(appearance);
                for (const auto& language : {QStringLiteral("en"), QStringLiteral("zh_CN")}) {
                    preferences.setLanguageMode(language);
                    settle(200);
                    const auto chromeColor = window->color();
                    require(
                        chromeColor.red() == chromeColor.green()
                            && chromeColor.green() == chromeColor.blue(),
                        "main UI is neutral in both appearances"
                    );
                    const auto controls = visibleButtons(window->contentItem(), *window);
                    const auto* projectButton =
                        qobject_cast<QQuickItem*>(&child(root, "deckProjectMenuButton"));
                    const auto* moreButton =
                        qobject_cast<QQuickItem*>(&child(root, "deckMoreMenuButton"));
                    require(
                        projectButton->mapToScene(QPointF(0, 0)).x()
                                >= titleBar.property("leftPadding").toDouble()
                            && moreButton->mapToScene(QPointF(moreButton->width(), 0)).x()
                                   <= window->width() - titleBar.property("rightPadding").toDouble()
                                          + 0.5,
                        "toolbar actions remain within the native window control safe areas"
                    );
                    auto* heading = qobject_cast<QQuickItem*>(&child(root, "deckProjectHeading"));
                    auto* print = qobject_cast<QQuickItem*>(&child(root, "cardKitPrint"));
                    require(
                        heading->property("lineCount").toInt() == 1 && heading->height() <= 20
                            && print->width() == 32 && print->height() == 32
                            && std::abs(
                                   projectButton->mapToScene(projectButton->boundingRect().center())
                                       .y()
                                   - print->mapToScene(print->boundingRect().center()).y()
                               ) < 0.5,
                        "project heading and compact print action occupy one aligned toolbar row"
                    );
                    const auto& preview = child(root, "cardKitPreviewImage");
                    const auto previewWidth = preview.property("paintedWidth").toDouble();
                    const auto previewHeight = preview.property("paintedHeight").toDouble();
                    require(
                        previewWidth > 300 && previewHeight > 400,
                        "card face dominates the workbench"
                    );
                    require(controls <= 23, "compact controls keep the resting workbench bounded");
                    const auto path =
                        QDir(directory).filePath(QString("deck-%1-%2-%3.png")
                                                     .arg(language == "en" ? "en" : "zh")
                                                     .arg(size.width())
                                                     .arg(appearance == 1 ? "light" : "dark"));
                    window->update();
                    settle(100);
                    const auto capture = window->grabWindow();
                    QJsonArray iconEvidence;
                    for (const auto* name :
                         {"deckLanguageButton",
                          "deckAppearanceButton",
                          "deckSettingsButton",
                          "cardKitPrint",
                          "deckMoreMenuButton",
                          "deckMoveCardUp",
                          "deckMoveCardDown",
                          "deckDuplicateCard",
                          "cardKitEnlargePreview",
                          "cardKitRowMenu-picture-text"})
                        iconEvidence.append(
                            verifyIcon(child(root, name), *window, capture, directory)
                        );
                    auto* languageButton =
                        qobject_cast<QQuickItem*>(&child(root, "deckLanguageButton"));
                    const auto iconCenter = languageButton->mapToScene(
                        QPointF(languageButton->width() / 2, languageButton->height() / 2)
                    );
                    const auto captureScale = double(capture.width()) / window->width();
                    int iconPixels = 0;
                    for (int y = int((iconCenter.y() - 9) * captureScale);
                         y < int((iconCenter.y() + 9) * captureScale);
                         ++y)
                        for (int x = int((iconCenter.x() - 9) * captureScale);
                             x < int((iconCenter.x() + 9) * captureScale);
                             ++x) {
                            const auto pixel = capture.pixelColor(x, y);
                            if (appearance == 2 ? pixel.lightness() > 140 : pixel.lightness() < 120)
                                ++iconPixels;
                        }
                    require(iconPixels > 12, "toolbar icons contrast with both appearances");
                    auto* face = qobject_cast<QQuickItem*>(&child(root, "cardKitPreviewImage"));
                    const auto bounds = face->mapRectToScene(QRectF(
                        (face->width() - previewWidth) / 2,
                        (face->height() - previewHeight) / 2,
                        previewWidth,
                        previewHeight
                    ));
                    const QRectF physicalBounds(
                        bounds.topLeft() * captureScale,
                        bounds.size() * captureScale
                    );
                    const auto sampleStep = qMax(1, qRound(4 * captureScale));
                    int artworkPixels = 0;
                    for (int y = std::max(0, int(physicalBounds.top()));
                         y < std::min(capture.height(), int(physicalBounds.bottom()));
                         y += sampleStep)
                        for (int x = std::max(0, int(physicalBounds.left()));
                             x < std::min(capture.width(), int(physicalBounds.right()));
                             x += sampleStep) {
                            const auto pixel = capture.pixelColor(x, y);
                            if (std::max({pixel.red(), pixel.green(), pixel.blue()})
                                    - std::min({pixel.red(), pixel.green(), pixel.blue()})
                                > 30)
                                ++artworkPixels;
                        }
                    require(capture.save(path), "bilingual workbench screenshot");
                    require(
                        artworkPixels > 100,
                        "card artwork stays visible across language and appearance switches"
                    );
                    uiEvidence.append(
                        QJsonObject{
                            {"width", size.width()},
                            {"language", language},
                            {"appearance", appearance == 1 ? "light" : "dark"},
                            {"visible_buttons", controls},
                            {"card_width", previewWidth},
                            {"card_height", previewHeight},
                            {"icons", iconEvidence},
                            {"screenshot", path}
                        }
                    );
                }
            }
        }
        preferences.setAppearanceMode(1);
        preferences.setLanguageMode("en");
        click(*window, child(root, "deckImageMenuButton"));
        evaluate(workspace, "selectedCardId = 'landscape'");
        evaluate(child(root, "cardKitGenerateArtwork"), "triggered()");
        require(
            !child(root, "deckShapeImageDialog").property("visible").toBool()
                && workspace.property("selectedCardId").toString() == "landscape",
            "late image-menu action cannot retarget another card"
        );
        click(*window, child(root, "cardKitRow-landscape"));
        require(
            child(root, "deckTextFields").property("visible").toBool() == false,
            "complete-image mode starts with image and copies rather than hidden text forms"
        );
        click(*window, child(root, "deckHiddenTextToggle"));
        require(
            child(root, "deckTextFields").property("visible").toBool()
                && child(root, "cardKitRules").property("text").toString()
                       == "Preserved hidden text",
            "hidden text remains available without changing layout or contents"
        );
        click(*window, child(root, "deckHiddenTextToggle"));
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-image-en-1120.png")),
            "image-first inspector screenshot"
        );
        click(*window, child(root, "cardKitRow-picture-text"));
        click(*window, child(root, "deckLayoutTab"));
        click(*window, child(root, "deckCardLayoutScope"));
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-layout-en-1120.png")),
            "inline card layout inspector screenshot"
        );
        click(*window, child(root, "deckContentTab"));
        auto* printButton = qobject_cast<QQuickItem*>(&child(root, "cardKitPrint"));
        require(printButton && printButton->isVisible(), "print entry visible");
        const auto printRect = printButton->mapRectToScene(printButton->boundingRect());
        require(
            printRect.bottom() <= window->height() && printRect.right() <= window->width(),
            "1120 print action within window"
        );
        preferences.setLanguageMode("zh_CN");
        settle(200);
        require(
            child(root, "cardKitPrint").property("toolTipText").toString()
                    == QString::fromUtf8("打印…")
                && child(root, "cardKitPrint").property("accessibleName").toString()
                       == QString::fromUtf8("打印…"),
            "live Chinese print tooltip and accessible name"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-zh-1120.png")),
            "Chinese actual window screenshot"
        );
        preferences.setLanguageMode("en");
        settle();
        click(*window, child(root, "deckMoreMenuButton"));
        require(
            child(root, "cardKitExport").property("enabled").toBool(),
            "exact accepted files remain available in the more menu"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-more-en-1120.png")),
            "version and archive menu screenshot"
        );
        evaluate(child(root, "deckMoreMenu"), "close()");
        click(*window, child(root, "cardKitPrint"));
        settle(200);
        require(
            cards.source() == source && backend.artifactCount() == count,
            "opening print does not reaccept or switch content"
        );
        require(
            printing.ready() && printing.plan().value("page_count").toInt() == 2
                && printing.plan().value("total_copies").toInt() == 12,
            "12 copies create full and partial pages"
        );
        auto& printDialog = child(root, "cardPrintDialog");
        const auto tightOptions = printing.plan().value("options").toMap();
        require(
            tightOptions.value("cut_line_style").toString() == "dashed_grid"
                && std::abs(tightOptions.value("gap_mm").toDouble() - 0.2) < 0.0001
                && tightOptions.value("gap_mm") == tightOptions.value("stroke_width_mm"),
            "default sheet uses a single line-width gap and shared dashed cut guides"
        );
        auto& marginControl = child(root, "cardPrint-margin_mm");
        require(
            marginControl.property("visible").toBool()
                && !printDialog.property("advanced").toBool(),
            "paper margin is available without opening fine adjustments"
        );
        marginControl.setProperty("value", 120);
        require(QMetaObject::invokeMethod(&marginControl, "valueModified"), "edit paper margin");
        settle(240);
        const auto insetOptions = printing.plan().value("options").toMap();
        require(
            printing.ready() && insetOptions.value("margin_mm").toDouble() == 12.0
                && insetOptions.value("card_height_mm").toDouble()
                       < tightOptions.value("card_height_mm").toDouble(),
            "increasing the paper margin recomputes the physical card grid"
        );
        evaluate(printDialog, "contentMode = 1; chooseGrid(2,3); closeLayout()");
        settle(100);
        click(*window, child(root, "cardKitPrint"));
        require(
            printing.ready()
                && printing.plan().value("options").toMap().value("margin_mm").toDouble() == 12.0
                && printing.plan().value("columns").toInt() == 2
                && printing.plan().value("rows").toInt() == 3
                && printing.plan().value("total_copies").toInt() == 3,
            "returning to cards before the refresh timer fires preserves sheet choices"
        );
        require(evaluate(printDialog, "resetLayout()").toBool(), "reset saved sheet choices");
        require(
            printing.plan().value("options").toMap().value("margin_mm").toDouble() == 5.0
                && printing.plan().value("columns").toInt() == 3
                && printing.plan().value("total_copies").toInt() == 12,
            "reset restores the default layout and authored quantities"
        );
        const auto planRequestPath = QDir(directory).filePath("print-plan-request.json");
        write(
            planRequestPath,
            {{"action", "print_plan"},
             {"source", QJsonObject::fromVariantMap(source)},
             {"options", QJsonObject::fromVariantMap(printing.plan().value("options").toMap())},
             {"imposition",
              QJsonObject::fromVariantMap(printing.plan().value("imposition").toMap())}}
        );
        const auto fullPlan =
            documents.cli().run({"card-kit", project, planRequestPath})["print_plan"].toObject();
        require(
            fullPlan["page_count"].toInt() == printing.plan().value("page_count").toInt(),
            "CLI and desktop consume the same print plan"
        );
        write(QDir(directory).filePath("print-plan.json"), fullPlan);
        const auto plan = card_print::prepare(fullPlan);
        verifyDashedCutGuides(*plan);
        require(
            card_print::preview(*plan, 0).save(QDir(directory).filePath("print-page-1.png"))
                && card_print::preview(*plan, 1).save(QDir(directory).filePath("print-page-2.png")),
            "actual shared painter previews"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-print-en-1120.png")),
            "print layout actual window"
        );
        const auto previousPlanVersion = printing.version();
        click(*window, child(root, "cardSheetPreset-2x3"));
        settle(240);
        require(
            printing.ready() && printing.plan().value("columns").toInt() == 2
                && printing.plan().value("rows").toInt() == 3
                && printing.plan().value("page_count").toInt() == 2,
            "quick grid applies exact two-column three-row layout"
        );
        const auto stalePdf = QDir(directory).filePath("stale-print.pdf");
        require(
            !printing.exportPdf(QUrl::fromLocalFile(stalePdf), previousPlanVersion)
                && !QFileInfo::exists(stalePdf),
            "changed sheet rejects stale export snapshot"
        );
        evaluate(printDialog, "contentMode = 1; refresh()");
        require(
            printing.ready() && printing.plan().value("total_copies").toInt() == 3
                && printing.plan().value("page_count").toInt() == 1,
            "different designs mix on one A4 sheet without changing authored quantities"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-print-mixed.png")),
            "mixed sheet screenshot"
        );
        const auto mixedPdf = QDir(directory).filePath("mixed-native.pdf");
        require(
            printing.exportPdf(QUrl::fromLocalFile(mixedPdf), printing.version())
                && wait([&] { return !printing.running(); }) && printing.error().isEmpty(),
            "mixed-card sheet exports through the same native PDF writer"
        );
        {
            PdfOpenProbe probe;
            QDesktopServices::setUrlHandler("file", &probe, "open");
            struct ResetPdfHandler {
                ~ResetPdfHandler() {
                    QDesktopServices::unsetUrlHandler("file");
                }
            } resetHandler;
            require(
                printing.exportedPdf() == QUrl::fromLocalFile(mixedPdf)
                    && child(root, "cardPrintOpenPdf").property("visible").toBool(),
                "successful export exposes the exact PDF and open action"
            );
            click(*window, child(root, "cardPrintOpenPdf"));
            require(
                probe.opened == QUrl::fromLocalFile(mixedPdf),
                "open action hands the exact PDF to its reader"
            );
            probe.opened.clear();
            const auto movedPdf = QDir(directory).filePath("mixed-moved.pdf");
            require(QFile::rename(mixedPdf, movedPdf), "move the temporary exported PDF");
            require(
                !printing.openExportedPdf() && probe.opened.isEmpty(),
                "missing PDF cannot open a stale destination"
            );
            require(QFile::rename(movedPdf, mixedPdf), "restore the temporary PDF fixture");
        }
        {
            const auto folder = QUrl::fromLocalFile(directory);
            const auto suggestion = printing.suggestedPdfDestination(folder);
            const QFileInfo proposed(suggestion.toLocalFile());
            require(
                suggestion.isLocalFile()
                    && proposed.dir().canonicalPath() == QFileInfo(directory).canonicalFilePath()
                    && proposed.fileName().endsWith(".pdf") && !proposed.exists(),
                "PDF suggestion uses a fresh name in the chosen output folder"
            );
            QFile existing(suggestion.toLocalFile());
            require(
                existing.open(QIODevice::WriteOnly) && existing.write("existing export") == 15,
                "create a prior filename fixture"
            );
            existing.close();
            const auto next = printing.suggestedPdfDestination(folder);
            require(
                next != suggestion && QFileInfo(next.toLocalFile()).fileName().endsWith(" (2).pdf")
                    && existing.size() == 15,
                "repeated export suggests a numbered filename without altering the existing file"
            );
            const auto fallback = printing.suggestedPdfDestination(QUrl::fromLocalFile(project));
            require(
                fallback.isEmpty()
                    || (QFileInfo(fallback.toLocalFile()).dir().canonicalPath()
                            == QFileInfo(
                                   QStandardPaths::writableLocation(
                                       QStandardPaths::DocumentsLocation
                                   )
                            )
                                   .canonicalFilePath()
                        && !fallback.toLocalFile().startsWith(project + "/")),
                "project bundles are never suggested as PDF destinations"
            );
        }
        evaluate(
            printDialog,
            "contentMode = 2; selectedCardId = 'landscape'; fillSheet = true; refresh()"
        );
        require(
            printing.ready() && printing.plan().value("total_copies").toInt() == 6,
            "one selected card repeats across the complete six-up sheet"
        );
        require(
            printing.exportedPdf().isEmpty()
                && !child(root, "cardPrintOpenPdf").property("visible").toBool(),
            "changing the sheet clears the old PDF open action"
        );
        auto* gridNumber = qobject_cast<QQuickItem*>(&child(root, "cardPrintColumns"));
        auto* fillCheck = qobject_cast<QQuickItem*>(&child(root, "cardPrintFillSheet"));
        require(
            gridNumber && fillCheck && gridNumber->height() <= 30 && fillCheck->height() <= 30,
            "numeric and checkbox controls use compact desktop density"
        );
        window->resize(980, 720);
        preferences.setAppearanceMode(2);
        preferences.setLanguageMode("zh_CN");
        settle(200);
        require(
            child(root, "cardPrintContent").property("currentIndex").toInt() == 2
                && child(root, "cardPrintContent").property("displayText").toString()
                       == QString::fromUtf8("单卡重复")
                && child(root, "cardPrintSelectedCard").property("currentIndex").toInt() == 1,
            "language switch preserves the displayed repeat mode and selected card"
        );
        require(
            window->grabWindow().save(
                QDir(directory).filePath("deck-print-repeat-zh-dark-980.png")
            ),
            "compact dark Chinese sheet screenshot"
        );
        const auto repeatPdf = QDir(directory).filePath("repeat-native.pdf");
        require(
            printing.exportPdf(QUrl::fromLocalFile(repeatPdf), printing.version())
                && wait([&] { return !printing.running(); }) && printing.error().isEmpty(),
            "single-card repeat exports a real PDF"
        );
        window->resize(1120, 780);
        preferences.setAppearanceMode(1);
        preferences.setLanguageMode("en");
        evaluate(printDialog, "contentMode = 0; chooseGrid(3,3); refresh()");
        settle(150);
        require(
            cards.source() == source && backend.artifactCount() == count,
            "imposition never creates a revision or changes the accepted source"
        );
        const auto pdf = QDir(directory).filePath("cards-native.pdf");
        require(
            printing.exportPdf(QUrl::fromLocalFile(pdf), printing.version())
                && wait([&] { return !printing.running(); }),
            "same native PDF writer complete"
        );
        require(QFileInfo(pdf).size() > 1000 && printing.error().isEmpty(), "real PDF published");
        require(
            !printing.exportPdf(QUrl::fromLocalFile(pdf), printing.version())
                || wait([&] { return !printing.running(); }),
            "repeated export settles"
        );
        require(!printing.error().isEmpty(), "existing PDF is never overwritten");
        require(
            printing.exportedPdf().isEmpty(),
            "failed export cannot retain a successful PDF action"
        );
        const auto pdfRequest = QDir(directory).filePath("pdf-request.json");
        write(
            pdfRequest,
            {{"schema", "deck.pdf-export/1"},
             {"project", QString(project).replace("/private/tmp/", "/tmp/")},
             {"source", QJsonObject::fromVariantMap(source)},
             {"imposition", QJsonObject{{"columns", 2}, {"rows", 2}, {"content", "one_each"}}},
             {"destination", QDir(directory).filePath("cards-cli.pdf")}}
        );
        const auto exported = documents.cli().run(
            {"print-pdf",
             project,
             pdfRequest,
             "--desktop",
             QCoreApplication::applicationFilePath()},
            60000
        );
        require(
            exported["independently_verified"].toBool()
                && exported["output"].toObject()["blake3"].toString().size() == 64,
            "AI CLI produces independently verified actual PDF with alias path and omitted options"
        );
        evaluate(child(root, "cardPrintDialog"), "close()");
        evaluate(root, "openDocument(" + literal(project) + ", " + literal(kit) + ")");
        require(
            backend.bundlePath() == QFileInfo(project).canonicalFilePath()
                && cards.draft().value("artifact_id").toString() == kit,
            "Unicode space hash path reopens through production navigation"
        );
        require(
            QJsonObject::fromVariantMap(cards.draft().value("composition").toMap()) == composition,
            "full card data persists after reopen"
        );
        {
            require(cards.beginNew(), "separate selected-print regression kit");
            const QJsonObject selectedComposition{
                {"name", "Selected copy regression"},
                {"layout", QJsonObject{{"mode", "text"}}},
                {"cards",
                 QJsonArray{
                     QJsonObject{{"id", "many"}, {"title", "Many"}, {"quantity", 300}},
                     QJsonObject{{"id", "one"}, {"title", "One"}, {"quantity", 1}}
                 }}
            };
            require(
                cards.save(selectedComposition.toVariantMap()) && cards.review("one")
                    && cards.accept(),
                "accept selected-print regression cards"
            );
            const auto selectedSource = cards.source();
            const QVariantMap each{{"columns", 1}, {"rows", 1}, {"content", "one_each"}};
            require(
                printing.prepare(selectedSource, {}, each)
                    && printing.plan().value("page_count").toInt() == 2,
                "one-each ignores the authored 300-copy quantity in the native consumer"
            );
            const QVariantMap repeated{
                {"columns", 1},
                {"rows", 1},
                {"content", "repeat"},
                {"card_id", "one"},
                {"copies", 1}
            };
            require(
                printing.prepare(selectedSource, {}, repeated)
                    && printing.plan().value("total_copies").toInt() == 1,
                "one selected copy prepares through the packaged CLI"
            );
            const auto selectedPdf = QDir(directory).filePath("selected-copy-native.pdf");
            require(
                printing.exportPdf(QUrl::fromLocalFile(selectedPdf), printing.version())
                    && wait([&] { return !printing.running(); }) && printing.error().isEmpty()
                    && QFileInfo(selectedPdf).size() > 1000,
                "selected copy exports a real native PDF"
            );
            require(
                !printing.prepare(selectedSource, {}, {{"columns", 1}, {"rows", 1}}),
                "authored quantities still enforce the physical page limit"
            );
            require(
                cards.source() == selectedSource
                    && cards.draft()
                               .value("composition")
                               .toMap()
                               .value("cards")
                               .toList()
                               .first()
                               .toMap()
                               .value("quantity")
                               .toInt()
                           == 300,
                "selected printing never rewrites authored quantities or accepted identity"
            );
            const auto openCurrentSheet = [&] {
                const auto sourceJson =
                    QString::fromUtf8(QJsonDocument(QJsonObject::fromVariantMap(cards.source()))
                                          .toJson(QJsonDocument::Compact));
                const auto cardsJson = QString::fromUtf8(
                    QJsonDocument(
                        QJsonArray::fromVariantList(
                            cards.draft().value("composition").toMap().value("cards").toList()
                        )
                    )
                        .toJson(QJsonDocument::Compact)
                );
                return evaluate(
                           printDialog,
                           "openForSource(" + sourceJson + ", {}, " + cardsJson + ", 'many')"
                )
                    .toBool();
            };
            require(openCurrentSheet(), "open a different card set in the layout dialog");
            require(
                printDialog.property("gridColumns").toInt() == 3
                    && printDialog.property("contentMode").toInt() == 0,
                "new card sets receive their own default layout"
            );
            evaluate(
                printDialog,
                "keepProportions = false; setOption('card_width_mm', 50); "
                "setOption('card_height_mm', 70); "
                "setOption('margin_mm', 8); setOption('stroke_width_mm', 0.4); "
                "contentMode = 2; selectedCardId = 'one'; fillSheet = false; repeatCopies = 5; "
                "closeLayout()"
            );
            settle(100);
            auto changed = selectedComposition;
            changed["name"] = QString::fromUtf8("更新 / Cards: title");
            changed["cards"] =
                QJsonArray{QJsonObject{{"id", "many"}, {"title", "Updated"}, {"quantity", 300}}};
            require(
                cards.save(changed.toVariantMap()) && cards.review("many") && cards.accept(),
                "accept revised card content with the old repeat selection removed"
            );
            require(
                cards.source() != selectedSource && openCurrentSheet(),
                "reopen layout after accepting a new card version"
            );
            const auto restoredOptions = printing.plan().value("options").toMap();
            require(
                printing.plan().value("source").toMap() == cards.source()
                    && printing.plan().value("name").toString()
                           == QString::fromUtf8("更新 / Cards: title")
                    && printDialog.property("selectedCardId").toString() == "many"
                    && printing.plan().value("total_copies").toInt() == 5
                    && restoredOptions.value("card_width_mm").toDouble() == 50.0
                    && restoredOptions.value("card_height_mm").toDouble() == 70.0
                    && restoredOptions.value("margin_mm").toDouble() == 8.0
                    && restoredOptions.value("stroke_width_mm").toDouble() == 0.4,
                "session layout follows the latest accepted content and safely replaces a removed "
                "repeat card"
            );
            const auto safeName =
                QFileInfo(
                    printing.suggestedPdfDestination(QUrl::fromLocalFile(directory)).toLocalFile()
                )
                    .fileName();
            require(
                safeName.startsWith(QString::fromUtf8("更新")) && !safeName.contains('/')
                    && !safeName.contains(':'),
                "suggested PDF names retain Chinese while removing path and filename separators"
            );
            evaluate(printDialog, "closeLayout()");
            settle(100);
            require(cards.load(kit), "restore original kit after selected-print regression");
            click(*window, child(root, "cardKitPrint"));
            require(
                printing.ready() && printDialog.property("gridColumns").toInt() == 3
                    && printDialog.property("contentMode").toInt() == 0
                    && printing.plan().value("options").toMap().value("margin_mm").toDouble()
                           == 5.0,
                "switching back to the original card set restores its own layout"
            );
            evaluate(printDialog, "closeLayout()");
            settle(100);
        }
        const auto exchangeSettings = QDir(directory).filePath("exchange-settings.ini");
        const auto externalRequest = QDir(directory).filePath("external-request.json");
        const auto externalResult = QDir(directory).filePath("external-result.json");
        const QJsonObject origin{
            {"producer", "shape"},
            {"project_id", backend.projectId()},
            {"kit_id", kit},
            {"expected_head", QJsonValue::Null},
            {"draft_digest", cards.draft().value("draft_digest").toString()}
        };
        write(
            externalRequest,
            {{"schema", "deck.external-edit/1"},
             {"request_id", "native-exchange"},
             {"origin", origin},
             {"input",
              QJsonObject{
                  {"kind", "project"},
                  {"project_path", project},
                  {"project_id", backend.projectId()},
                  {"kit_id", kit}
              }}}
        );
        {
            DeckExchangeController exchange(backend, cards, documents, exchangeSettings);
            require(
                exchange.load(externalRequest, externalResult) && exchange.bindCurrent(kit),
                "strict external project request"
            );
            require(
                exchange.recordOutput(pdf, "pdf", source),
                "explicit PDF becomes eligible output"
            );
            require(exchange.finish(source), "explicit return publishes completed receipt");
        }
        {
            DeckExchangeController resumed(backend, cards, documents, exchangeSettings);
            require(
                resumed.load(
                    externalRequest,
                    QDir(directory).filePath("external-retry-result.json")
                ) && resumed.projectPath() == project
                    && resumed.kitId() == kit,
                "same exact handoff recovers own saved project without importing again"
            );
            require(!resumed.finish(source), "opening or saving alone never completes a return");
            require(resumed.cancel(), "explicit cancel preserves own project identity");
        }
        auto changed = composition;
        auto changedCards = changed["cards"].toArray();
        auto changedCard = changedCards[0].toObject();
        changedCard["title"] = "Changed draft";
        changedCards[0] = changedCard;
        changed["cards"] = changedCards;
        {
            DeckExchangeController stale(backend, cards, documents, exchangeSettings);
            const auto staleResult = QDir(directory).filePath("external-stale-result.json");
            require(
                stale.load(externalRequest, staleResult) && stale.bindCurrent(kit)
                    && stale.recordOutput(pdf, "pdf", source),
                "known accepted output recorded"
            );
            require(
                cards.save(changed.toVariantMap()) && !cards.currentAccepted(),
                "saved changed draft is not accepted"
            );
            require(
                !stale.finish(source) && !QFileInfo::exists(staleResult),
                "changed visible draft cannot return old accepted outputs"
            );
            require(stale.cancel(), "stale return can be cancelled without losing draft");
        }
        require(cards.save(composition.toVariantMap()), "restore original test draft");
        const auto mixedPath = QDir(directory).filePath("mixed-source.shape");
        {
            DesktopBackend mixed(
                shape::desktop::create_desktop_project(
                    mixedPath.toStdString(),
                    std::string("Mixed test project")
                )
            );
            require(
                mixed.createSoundScene("Unrelated sound"),
                "mixed project fixture without provider execution"
            );
        }
        require(
            !documents.openLocalProject(mixedPath) && backend.bundlePath() == project,
            "desktop rejects mixed project before opening a mutable session and preserves current "
            "document"
        );
        require(
            documents.openLocalProject(project),
            "own project remains usable after rejected open"
        );
        require(cards.load(kit), "reload original kit after admission check");
        const auto before = backend.artifactCount();
        require(cards.beginNew(), "second kit for late callback fence");
        evaluate(
            workspace,
            "importArtwork(" + literal(QUrl::fromLocalFile(imagePath).toString()) + ", 'spring', "
                + literal(backend.projectId()) + ", " + literal(kit) + ", " + literal(project) + ")"
        );
        require(backend.artifactCount() == before + 1, "old kit callback creates no image orphan");
        require(cards.load(kit), "return to original kit");
        evaluate(workspace, "selectedCardId = 'landscape'");
        click(*window, child(root, "deckImageMenuButton"));
        click(*window, child(root, "cardKitRefineArtwork"));
        const auto stage = value(workspace, "editingStage").toMap();
        require(
            !stage["contextArtifactId"].toString().isEmpty(),
            "Shadow stage targets original card image"
        );
        const auto originalPin = pin(backend, artwork["artifact_id"].toString());
        const auto shadow = qEnvironmentVariable("DECK_SHADOW_BINARY");
        if (!shadow.isEmpty()) {
            require(
                material.setShadowExecutable(QUrl::fromLocalFile(shadow)),
                "selected real Shadow executable"
            );
            qputenv("SHADOW_PIPELINE_SMOKE_ACTION", "complete");
            qputenv(
                "SHADOW_DESKTOP_DATA_ROOT",
                QDir(directory).filePath("shadow-profile").toUtf8()
            );
            qputenv(
                "SHADOW_PAINT_BRUSH_SETTINGS_FILE",
                QDir(directory).filePath("shadow-brush.ini").toUtf8()
            );
            material.openShadow(project, stage["contextArtifactId"].toString());
            require(
                wait([&] { return !material.running() && !material.shadowActive(); })
                    && material.errorCode().isEmpty(),
                "real Shadow process returned a verified candidate"
            );
            require(
                pin(backend, artwork["artifact_id"].toString()) == originalPin,
                "Shadow leaves original source unchanged"
            );
            require(
                !value(workspace, "artworkCandidate").toMap().isEmpty(),
                "returned Shadow candidate visible in original card context"
            );
            click(*window, child(root, "cardArtworkUseRefined"));
            const auto revised =
                QJsonObject::fromVariantMap(cards.draft().value("composition").toMap())["cards"]
                    .toArray()[1]
                    .toObject()["artwork"]
                    .toObject();
            require(
                revised != artwork
                    && pin(backend, artwork["artifact_id"].toString()) == originalPin,
                "only explicit Use binds changed image to original card"
            );
        } else {
            click(*window, child(root, "cardArtworkBack"));
        }
#ifdef Q_OS_UNIX
        {
            const auto taskRoot = QDir(directory).filePath("restored-image-task");
            require(QDir().mkpath(taskRoot), "restored task fixture directory");
            const auto request = QDir(taskRoot).filePath("request.json");
            write(request, {{"schema", "deck.material-task/1"}, {"request_id", "restored-task"}});
            QProcess external;
            external.start("/bin/sleep", {"10"});
            require(external.waitForStarted(1000), "real external-process fixture starts");
            const auto taskSettings = QDir(taskRoot).filePath("task.ini");
            QSettings stored(taskSettings, QSettings::IniFormat);
            const QJsonObject record{
                {"request", request},
                {"result", QDir(taskRoot).filePath("result.json")},
                {"digest", documents.cli().hashFile(request)["blake3"]},
                {"project", project},
                {"executable", "/bin/sleep"},
                {"pid", external.processId()},
                {"waiting", true}
            };
            stored.setValue(
                "deck/material_task",
                QJsonDocument(record).toJson(QJsonDocument::Compact)
            );
            stored.sync();
            ExternalImageTaskController resumed(backend, cards, documents, taskSettings);
            require(resumed.running(), "restored controller resumes a live external process");
            external.terminate();
            require(external.waitForFinished(1000), "external process exits without a result");
            require(
                wait([&] { return !resumed.running(); }, 2000) && !resumed.error().isEmpty()
                    && !resumed.resultReady(),
                "restored task stops waiting when its process exits without returning"
            );
            stored.sync();
            require(
                !QJsonDocument::fromJson(stored.value("deck/material_task").toByteArray())
                     .object()["waiting"]
                     .toBool(),
                "terminal restored-task state is persisted"
            );
        }
        {
            const auto aliasRoot = QDir(directory).filePath("shape-debug-alias");
            const auto alias = QDir(aliasRoot).filePath("current-debug");
            const auto applicationSuffix =
                QStringLiteral("/Shape.app/Contents/MacOS/shape-desktop");
            QStringList releases;
            for (const auto& name : {QStringLiteral("first"), QStringLiteral("second")}) {
                const auto release = QDir(aliasRoot).filePath(name);
                const auto executable = release + applicationSuffix;
                require(
                    QDir().mkpath(QFileInfo(executable).absolutePath()),
                    "alias release folder"
                );
                QFile file(executable);
                require(file.open(QIODevice::WriteOnly), "alias executable fixture");
                file.write(
                    "#!/bin/sh\nif [ \"$1\" = \"--handoff-capabilities\" ]; then\n"
                    "/bin/sleep 0.1\nprintf '%s\\n' "
                    "'{\"schema\":\"shape.handoff-capabilities/1\","
                    "\"deck_material_task\":\"deck.material-task/1\","
                    "\"deck_material_result\":\"deck.material-task-result/1\"}'\nelse\n"
                );
                file.write(
                    "printf '%s' '" + name.toUtf8() + "' > \"${2%/*}/launched-release\"\nfi\n"
                );
                file.close();
                require(
                    file.setPermissions(
                        QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner
                    ),
                    "executable fixture permissions"
                );
                releases.push_back(QFileInfo(release).canonicalFilePath());
            }
            const auto pointTo = [&](int index) {
                if (QFileInfo(alias).isSymLink())
                    require(QFile::remove(alias), "replace debug alias");
                std::filesystem::create_directory_symlink(
                    releases[index].toStdString(),
                    alias.toStdString()
                );
            };
            pointTo(0);
            const auto settingsPath = QDir(aliasRoot).filePath("documents.ini");
            DeckDocumentController selected(backend, documents.cli(), settingsPath);
#ifdef Q_OS_MACOS
            const auto chosen = alias + "/Shape.app";
#else
            const auto chosen = alias + applicationSuffix;
#endif
            require(
                selected.setShapeExecutable(QUrl::fromLocalFile(chosen)),
                "select current debug application"
            );
            require(
                selected.shapeExecutable() == releases[0] + applicationSuffix,
                "selected alias resolves first release"
            );
            QSettings stored(settingsPath, QSettings::IniFormat);
            require(
                stored.value("deck/shape_executable").toString() == alias + applicationSuffix,
                "settings preserve selected alias"
            );
            const auto taskSettings = QDir(aliasRoot).filePath("task.ini");
            ExternalImageTaskController captured(backend, cards, selected, taskSettings);
            require(
                captured.begin("A spring tree", 750, 1050, "spring"),
                "capture first executable before promotion"
            );
            pointTo(1);
            require(
                selected.shapeExecutable() == releases[1] + applicationSuffix,
                "existing selection follows promoted debug alias"
            );
            DeckDocumentController reopened(backend, documents.cli(), settingsPath);
            require(
                reopened.shapeExecutable() == releases[1] + applicationSuffix,
                "reopened settings follow promoted alias"
            );
            require(
                wait([&] { return !captured.running(); }, 6000),
                "captured task completes after alias promotion"
            );
            QSettings taskStored(taskSettings, QSettings::IniFormat);
            const auto record =
                QJsonDocument::fromJson(taskStored.value("deck/material_task").toByteArray())
                    .object();
            require(
                record["executable"].toString() == releases[0] + applicationSuffix,
                "active task retains captured executable identity"
            );
            QFile launched(
                QFileInfo(record["request"].toString()).dir().filePath("launched-release")
            );
            require(
                launched.open(QIODevice::ReadOnly) && launched.readAll() == "first",
                "probe and editor launch use captured old release"
            );
            require(
                selected.setShapeExecutable(QUrl::fromLocalFile(releases[1] + applicationSuffix)),
                "second executable selection"
            );
            require(
                selected.shapeExecutable() == releases[1] + applicationSuffix,
                "second selection resolves chosen executable"
            );
        }
#endif
        require(
            documents.setShapeExecutable(QUrl::fromLocalFile("/usr/bin/true")),
            "legacy executable fixture"
        );
        evaluate(workspace, "selectedCardId = 'spring'; flush()");
        require(
            imageTask.begin("A spring tree", 750, 1050, "spring"),
            "Shape check starts asynchronously"
        );
        imageTask.stopWaiting();
        require(!imageTask.running(), "stop during probe immediately stops waiting");
        require(wait([&] { return !imageTask.running(); }, 5000), "cancelled probe settles");
        settle(300);
        require(
            imageTask.error().isEmpty() && !imageTask.resultReady(),
            "cancelled probe cannot surface stale card error or image"
        );
        require(
            imageTask.begin("A spring tree", 750, 1050, "spring"),
            "another Shape capability check"
        );
        require(
            wait([&] { return !imageTask.running(); }, 6000) && !imageTask.error().isEmpty(),
            "old executable rejected before image launch"
        );
        click(*window, child(root, "deckCreateWithAI"));
        require(
            child(root, "deckAssistantDialog").property("visible").toBool(),
            "AI workflow opens from the card list"
        );
        require(
            !child(root, "deckAssistantGenerate").property("enabled").toBool(),
            "empty brief cannot start a model request"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-assistant.png")),
            "assistant review workflow screenshot"
        );
        click(*window, child(root, "deckAssistantClose"));
        click(*window, child(root, "deckSettingsButton"));
        require(
            child(root, "deckSettingsDialog").property("visible").toBool(),
            "settings stay reachable without the project menu"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-settings.png")),
            "appearance language and model settings screenshot"
        );
        evaluate(child(root, "deckSettingsDialog"), "close()");
        require(
            card_draft_assistant_smoke::verify(QDir(directory).filePath("assistant-lifecycle")),
            "assistant candidate and cancellation lifecycle"
        );
        const auto beforeAssistant =
            cards.draft().value("composition").toMap().value("cards").toList().size();
        require(
            !evaluate(
                 workspace,
                 "appendAssistantCards([{title:'Moon',rules:'A satellite'}], 'stale-target')"
            )
                 .toBool(),
            "stale assistant target cannot append cards"
        );
        require(
            cards.draft().value("composition").toMap().value("cards").toList().size()
                == beforeAssistant,
            "stale candidate leaves saved draft intact"
        );
        require(
            evaluate(
                workspace,
                "appendAssistantCards([{title:'Moon',rules:'A satellite'}], assistantIdentity())"
            )
                .toBool(),
            "explicit candidate append uses CardKit save service"
        );
        require(
            cards.draft().value("composition").toMap().value("cards").toList().size()
                    == beforeAssistant + 1
                && !cards.currentAccepted(),
            "AI suggestions append a draft without accepting a version"
        );
        {
            const auto before = cards.draft().value("composition").toMap();
            evaluate(
                workspace,
                "setField('layout', Object.assign({}, composition.layout, {mode:'image'})); "
                "flush(); openCardList()"
            );
            const auto imageComposition = cards.draft().value("composition").toMap();
            child(root, "cardKitListText")
                .setProperty(
                    "text",
                    QString::fromUtf8("月亮\t今晚抬头看一眼月亮。\n引力\t让小球落向桌面。")
                );
            require(
                evaluate(workspace, "appendCardList()").toBool(),
                "text list saves into an image-default kit"
            );
            const auto after = cards.draft().value("composition").toMap();
            auto originalCards = after.value("cards").toList();
            originalCards.removeLast();
            originalCards.removeLast();
            require(
                originalCards == imageComposition.value("cards").toList()
                    && after.value("layout") == imageComposition.value("layout")
                    && cards.review(workspace.property("selectedCardId").toString())
                    && cards.preview().value("svg").toString().contains(QString::fromUtf8("月亮"))
                    && !cards.preview().value("svg").toString().contains("image-placeholder"),
                "list cards show text while existing cards and image defaults remain intact"
            );
            require(
                evaluate(
                    workspace,
                    "CardListImport.parse('Moon\\u0085light', composition, false).error === "
                    "'invalid_text'"
                )
                    .toBool(),
                "C1 control text is rejected before adding cards"
            );
            auto legacy = before;
            legacy.remove("layout");
            auto legacyCards = legacy.value("cards").toList();
            for (auto& item : legacyCards) {
                auto card = item.toMap();
                card.remove("layout");
                item = card;
            }
            legacy["cards"] = legacyCards;
            require(cards.save(legacy), "legacy layout regression draft");
            const auto legacyDraft = cards.draft();
            require(
                !evaluate(workspace, "openAssistant()").toBool() && cards.draft() == legacyDraft
                    && workspace.property("layoutForSet").toBool(),
                "AI on legacy cards requests an explicit layout choice without altering content"
            );
            require(cards.save(before), "restore original cards after intake regressions");
            evaluate(workspace, "inspectorTab = 0; layoutForSet = false");
        }
        evaluate(workspace, "setCardField(\"rules\", \"Unfinished observation\")");
        click(*window, child(root, "deckAppearanceButton"));
        click(*window, child(root, "deckAppearanceDark"));
        require(
            preferences.appearanceMode() == 2 && preferences.dark(),
            "toolbar appearance switch changes live preferences"
        );
        click(*window, child(root, "deckLanguageButton"));
        click(*window, child(root, "deckChinese"));
        require(
            preferences.languageMode() == "zh_CN",
            "toolbar language switch changes live preferences"
        );
        require(
            evaluate(workspace, "currentCard.rules === 'Unfinished observation'").toBool(),
            "unsaved card text survives live language and appearance changes"
        );
        evaluate(workspace, "flush()");
        {
            UiPreferences reopened(
                *qobject_cast<QGuiApplication*>(QCoreApplication::instance()),
                QDir(directory).filePath("deck.conf")
            );
            require(
                reopened.appearanceMode() == 2 && reopened.dark()
                    && reopened.languageMode() == "zh_CN",
                "language and appearance persist across preferences reopening"
            );
        }
        preferences.setLanguageMode("en");
        preferences.setAppearanceMode(1);
        const auto layoutWorkflow = card_layout_workflow_smoke::verify(
            root,
            backend,
            cards,
            printing,
            documents,
            QDir(directory).filePath("layout-workflow")
        );
        const auto blockWorkflow = card_block_workflow_smoke::verify(
            root,
            backend,
            cards,
            printing,
            documents,
            QDir(directory).filePath("block-workflow")
        );
        write(
            QDir(directory).filePath("report.json"),
            {{"status", "passed"},
             {"binary_sha256", sha256(QCoreApplication::applicationFilePath())},
             {"cli_sha256", sha256(documents.cli().executable())},
             {"project", project},
             {"kit_id", kit},
             {"source", QJsonObject::fromVariantMap(source)},
             {"gui_pdf", pdf},
             {"cli_pdf", exported},
             {"native_shadow", !shadow.isEmpty()},
             {"dedicated_main_page", true},
             {"bilingual_1120", true},
             {"late_import_fenced", true},
             {"cancelled_probe_fenced", true},
             {"shape_debug_alias_follows_promotion", true},
             {"calm_workbench_interactions", true},
             {"application_icon_loaded", true},
             {"monochrome_both_appearances", true},
             {"toolbar_icon_contrast_both_appearances", true},
             {"preferences_live_and_persisted", true},
             {"fresh_profile_defaults_to_light", true},
             {"integrated_titlebar_safe_at_980", true},
             {"assistant_stale_candidate_fenced", true},
             {"assistant_cancel_before_start_acknowledgement", true},
             {"project_open_error_in_active_dialog", true},
             {"selected_copy_print_native_pdf", true},
             {"restored_image_task_exit_detected", true},
             {"image_default_text_list_visible", true},
             {"legacy_assistant_layout_preserved", true},
             {"control_character_intake_rejected", true},
             {"tight_shared_dashed_cut_guides", true},
             {"paper_margin_directly_adjustable", true},
             {"sheet_session_choices_retained", true},
             {"sheet_session_follows_new_card_revision", true},
             {"sheet_session_card_set_isolation", true},
             {"pdf_filename_collision_suggestion", true},
             {"pdf_open_exact_successful_destination", true},
             {"pdf_missing_or_stale_destination_rejected", true},
             {"layout_workflow", layoutWorkflow},
             {"block_workflow", blockWorkflow},
             {"workbench_screenshots", uiEvidence}}
        );
        qInfo().noquote() << "Deck native smoke passed:" << directory;
        return true;
    } catch (const std::exception& error) {
        qCritical().noquote() << "Deck native smoke failed:" << error.what();
        if (auto* window = qobject_cast<QQuickWindow*>(&root))
            window->grabWindow().save(QDir(directory).filePath("failure.png"));
        write(
            QDir(directory).filePath("report.json"),
            {{"status", "failed"}, {"error", QString::fromUtf8(error.what())}}
        );
        return false;
    }
}
