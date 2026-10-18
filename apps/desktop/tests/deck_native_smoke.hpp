#pragma once
#include <QString>
class QObject;
class DesktopBackend;
class CardKitController;
class CardPrintController;
class MaterialEditController;
class DeckDocumentController;
class ExternalImageTaskController;
class UiPreferences;
namespace deck_native_smoke {
bool verify(
    QObject&,
    DesktopBackend&,
    CardKitController&,
    CardPrintController&,
    MaterialEditController&,
    DeckDocumentController&,
    ExternalImageTaskController&,
    UiPreferences&,
    const QString& evidence
);
}
