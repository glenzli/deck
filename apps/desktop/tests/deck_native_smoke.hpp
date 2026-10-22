#pragma once
#include <QObject>
#include <QString>
#include <QUrl>
class DesktopBackend;
class CardKitController;
class CardPrintController;
class MaterialEditController;
class DeckDocumentController;
class ExternalImageTaskController;
class UiPreferences;
namespace deck_native_smoke {
class PdfOpenProbe final : public QObject {
    Q_OBJECT
  public:
    QUrl opened;
    Q_INVOKABLE void open(const QUrl& url) {
        opened = url;
    }
};
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
} // namespace deck_native_smoke
