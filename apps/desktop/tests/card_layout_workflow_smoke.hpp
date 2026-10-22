#pragma once
#include <QJsonObject>
#include <QString>
class QObject;
class DesktopBackend;
class CardKitController;
class CardPrintController;
class DeckDocumentController;
namespace card_layout_workflow_smoke {
QJsonObject verify(
    QObject&,
    DesktopBackend&,
    CardKitController&,
    CardPrintController&,
    DeckDocumentController&,
    const QString& directory
);
}
