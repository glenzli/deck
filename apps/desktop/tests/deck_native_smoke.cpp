#include "deck_native_smoke.hpp"
#include "card_kit_controller.hpp"
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
#include <QQmlEngine>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTimer>
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
void evaluate(QObject& owner, const QString& script) {
    QQmlExpression expression(QQmlEngine::contextForObject(&owner), &owner, script);
    expression.evaluate();
    if (expression.hasError())
        throw std::runtime_error(expression.error().toString().toStdString());
    QCoreApplication::processEvents();
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
void click(QQuickWindow& window, QObject& control) {
    auto* item = qobject_cast<QQuickItem*>(&control);
    require(item && item->isVisible() && item->isEnabled(), "visible interactive control");
    const auto position = item->mapToScene(item->boundingRect().center());
    require(
        QRectF(0, 0, window.width(), window.height()).contains(position),
        "interaction within window"
    );
    const auto global = window.mapToGlobal(position.toPoint());
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
        require(
            child(root, "cardKitPrint").property("text").toString() == "Accept & print…",
            "unaccepted draft clearly names the explicit acceptance before printing"
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
        for (const auto& size : {QSize(1280, 840), QSize(1120, 720)}) {
            window->resize(size);
            for (const auto& language : {QStringLiteral("en"), QStringLiteral("zh_CN")}) {
                preferences.setLanguageMode(language);
                settle(200);
                const auto controls = visibleButtons(window->contentItem(), *window);
                const auto& preview = child(root, "cardKitPreviewImage");
                const auto previewWidth = preview.property("paintedWidth").toDouble();
                const auto previewHeight = preview.property("paintedHeight").toDouble();
                require(
                    previewWidth > 300 && previewHeight > 400,
                    "card face dominates the workbench"
                );
                require(controls <= 16, "secondary commands do not fill the resting workbench");
                const auto path = QDir(directory).filePath(
                    QString("deck-%1-%2.png").arg(language == "en" ? "en" : "zh").arg(size.width())
                );
                require(window->grabWindow().save(path), "bilingual workbench screenshot");
                uiEvidence.append(
                    QJsonObject{
                        {"width", size.width()},
                        {"language", language},
                        {"visible_buttons", controls},
                        {"card_width", previewWidth},
                        {"card_height", previewHeight},
                        {"screenshot", path}
                    }
                );
            }
        }
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
            child(root, "cardKitPrint").property("text").toString() == QString::fromUtf8("打印…"),
            "live Chinese card actions"
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
        const auto planRequestPath = QDir(directory).filePath("print-plan-request.json");
        write(
            planRequestPath,
            {{"action", "print_plan"},
             {"source", QJsonObject::fromVariantMap(source)},
             {"options", QJsonObject::fromVariantMap(printing.plan().value("options").toMap())}}
        );
        const auto fullPlan =
            documents.cli().run({"card-kit", project, planRequestPath})["print_plan"].toObject();
        require(
            fullPlan["page_count"].toInt() == printing.plan().value("page_count").toInt(),
            "CLI and desktop consume the same print plan"
        );
        write(QDir(directory).filePath("print-plan.json"), fullPlan);
        const auto plan = card_print::prepare(fullPlan);
        require(
            card_print::preview(*plan, 0).save(QDir(directory).filePath("print-page-1.png"))
                && card_print::preview(*plan, 1).save(QDir(directory).filePath("print-page-2.png")),
            "actual shared painter previews"
        );
        require(
            window->grabWindow().save(QDir(directory).filePath("deck-print-en-1120.png")),
            "print layout actual window"
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
        const auto pdfRequest = QDir(directory).filePath("pdf-request.json");
        write(
            pdfRequest,
            {{"schema", "deck.pdf-export/1"},
             {"project", QString(project).replace("/private/tmp/", "/tmp/")},
             {"source", QJsonObject::fromVariantMap(source)},
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
