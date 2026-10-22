//! Deck only assembles card, material handoff and print owners; no Shape workbench.
#include "card_draft_assistant.hpp"
#include "card_kit_controller.hpp"
#include "card_layout_preset_store.hpp"
#include "card_print_controller.hpp"
#include "deck_cli_client.hpp"
#include "deck_document_controller.hpp"
#include "deck_exchange_controller.hpp"
#include "deck_native_smoke.hpp"
#include "desktop_backend.hpp"
#include "external_image_task_controller.hpp"
#include "icon_image_provider.hpp"
#include "image_preview_provider.hpp"
#include "material_edit_controller.hpp"
#include "pdf_command.hpp"
#include "ui_preferences.hpp"
#if defined(Q_OS_MACOS)
#include "mac_titlebar.hpp"
#include <QQuickWindow>
#endif
#include <QCommandLineParser>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QSettings>
#include <QStandardPaths>
#include <QTimer>
#include <QUuid>
#include <iostream>
#include <string_view>
int main(int argc, char* argv[]) {
    // Version negotiation must work without a GUI session or opening any project.
    if (argc == 2 && std::string_view(argv[1]) == "--handoff-capabilities") {
        std::cout
            << R"({"schema":"deck.handoff-capabilities/1","external_edit":"deck.external-edit/1","material_result":"deck.material-task-result/1"})"
            << '\n';
        return 0;
    }
    for (int index = 1; index < argc; ++index)
        if (std::string_view(argv[index]) == "--export-pdf"
            && qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM"))
            qputenv("QT_QPA_PLATFORM", "offscreen");
    QGuiApplication app(argc, argv);
    app.setWindowIcon(QIcon(QStringLiteral(":/branding/app-icon.png")));
    QCoreApplication::setOrganizationName("Glenzli");
    QCoreApplication::setApplicationName("Deck");
    QCoreApplication::setApplicationVersion("0.1.0");
    QQuickStyle::setStyle("Basic");
    QCommandLineParser parser;
    parser.setApplicationDescription("Deck card authoring and physical print layout");
    parser.addHelpOption();
    parser.addVersionOption();
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("project")},
        QStringLiteral("Open a Deck project"),
        QStringLiteral("path")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("kit")},
        QStringLiteral("Open an exact card set"),
        QStringLiteral("id")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("external-edit")},
        QStringLiteral("Open a Shape handoff"),
        QStringLiteral("request")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("result")},
        QStringLiteral("Write a new result receipt"),
        QStringLiteral("path")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("export-pdf")},
        QStringLiteral("Export an exact accepted version without opening the editor"),
        QStringLiteral("request")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("deck-cli")},
        QStringLiteral("Use the packaged Deck CLI"),
        QStringLiteral("path")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("settings")},
        QStringLiteral("Use an isolated settings file"),
        QStringLiteral("path")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("smoke-native")},
        QStringLiteral("Run the production-linked native consumer smoke")
    ));
    parser.addOption(QCommandLineOption(
        QStringList{QStringLiteral("evidence")},
        QStringLiteral("Write smoke evidence in a new directory"),
        QStringLiteral("path")
    ));
    parser.process(app);
#ifdef Q_OS_WIN
    const auto cliName = QStringLiteral("deck-cli.exe");
#else
    const auto cliName = QStringLiteral("deck-cli");
#endif
    const auto cliPath = parser.isSet("deck-cli")
                             ? parser.value("deck-cli")
                             : QDir(QCoreApplication::applicationDirPath()).filePath(cliName);
    const DeckCliClient cli(cliPath);
    if (parser.isSet("export-pdf")) {
        if (!parser.isSet("result"))
            return 2;
        return runPdfCommand(cli, parser.value("export-pdf"), parser.value("result"));
    }
    const bool smoke = parser.isSet("smoke-native");
    const auto evidence =
        parser.isSet("evidence") ? parser.value("evidence")
        : !qEnvironmentVariable("DECK_SMOKE_EVIDENCE").isEmpty()
            ? qEnvironmentVariable("DECK_SMOKE_EVIDENCE")
            : QDir::temp().filePath(
                  "deck-native-" + QUuid::createUuid().toString(QUuid::WithoutBraces)
              );
    const auto settings =
        parser.isSet("settings") ? parser.value("settings")
        : smoke ? QDir(evidence).filePath("deck.conf")
                : QDir(QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation))
                      .filePath("deck.conf");
    QDir().mkpath(QFileInfo(settings).absolutePath());
    DesktopBackend backend;
    const bool hasSavedAppearance =
        QSettings(settings, QSettings::IniFormat).contains("ui/appearanceMode");
    UiPreferences preferences(app, settings);
    if (!hasSavedAppearance)
        preferences.setAppearanceMode(static_cast<int>(UiPreferences::AppearanceMode::Light));
    CardKitController cards(backend, settings, &app);
    CardLayoutPresetStore layoutPresets(
        QDir(QFileInfo(settings).absolutePath()).filePath("card-layout-presets.json"),
        &app
    );
    CardPrintController print(backend, cli, &app);
    MaterialEditController material(backend, settings, &app);
    DeckDocumentController documents(backend, cli, settings, &app);
    DeckExchangeController exchange(backend, cards, documents, settings, &app);
    ExternalImageTaskController imageTask(backend, cards, documents, settings, &app);
    CardDraftAssistant assistant(cliPath, settings, &app);
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &assistant,
        &CardDraftAssistant::changed
    );
    if (parser.isSet("external-edit")
        && (!parser.isSet("result")
            || !exchange.load(parser.value("external-edit"), parser.value("result"))))
        return 1;
    QQmlApplicationEngine engine;
    preferences.attachEngine(engine);
    engine.addImageProvider("shape-preview", new ImagePreviewProvider(backend.imagePreviewStore()));
    engine.addImageProvider("deck-icons", new DeckIconImageProvider());
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &backend,
        &DesktopBackend::retranslate
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &cards,
        &CardKitController::retranslate
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &layoutPresets,
        &CardLayoutPresetStore::retranslate
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &print,
        &CardPrintController::retranslate
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &documents,
        &DeckDocumentController::statusChanged
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &exchange,
        &DeckExchangeController::changed
    );
    QObject::connect(
        &preferences,
        &UiPreferences::languageModeChanged,
        &imageTask,
        &ExternalImageTaskController::changed
    );
    engine.setInitialProperties(
        {{"backend", QVariant::fromValue(&backend)},
         {"cardKit", QVariant::fromValue(&cards)},
         {"layoutPresets", QVariant::fromValue(&layoutPresets)},
         {"cardPrint", QVariant::fromValue(&print)},
         {"materialEdit", QVariant::fromValue(&material)},
         {"preferences", QVariant::fromValue(&preferences)},
         {"documents", QVariant::fromValue(&documents)},
         {"exchange", QVariant::fromValue(&exchange)},
         {"imageTask", QVariant::fromValue(&imageTask)},
         {"assistant", QVariant::fromValue(&assistant)},
         {"initialProject", parser.value("project")},
         {"initialKit", parser.value("kit")},
         {"smokeMode", smoke}}
    );
    engine.loadFromModule("Shape.Desktop", "DeckWindow");
    if (engine.rootObjects().isEmpty())
        return 1;
#if defined(Q_OS_MACOS)
    const auto root = engine.rootObjects().first();
    const auto titleBar = root->findChild<QObject*>(QStringLiteral("titleToolBar"));
    const auto titleBarHeight = titleBar ? qRound(titleBar->property("height").toReal()) : 56;
    installMacTitleBarAlignment(qobject_cast<QQuickWindow*>(root), titleBarHeight);
#endif
    if (smoke)
        QTimer::singleShot(0, &app, [&] {
            const auto passed = deck_native_smoke::verify(
                *engine.rootObjects().first(),
                backend,
                cards,
                print,
                material,
                documents,
                imageTask,
                preferences,
                evidence
            );
            app.exit(passed ? 0 : 1);
        });
    return app.exec();
}
