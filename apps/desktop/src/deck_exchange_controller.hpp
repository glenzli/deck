//! One Shape-to-Deck document handoff and explicit verified output return.
#pragma once
#include "deck_cli_client.hpp"
#include <QJsonArray>
#include <QObject>
#include <QSettings>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
class DesktopBackend;
class CardKitController;
class DeckDocumentController;
class DeckExchangeController final : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by Deck")
    Q_PROPERTY(bool active READ active NOTIFY changed)
    Q_PROPERTY(bool needsImport READ needsImport NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(int outputCount READ outputCount NOTIFY changed)
    Q_PROPERTY(QString outputRevision READ outputRevision NOTIFY changed)
    Q_PROPERTY(QString projectPath READ projectPath NOTIFY changed)
    Q_PROPERTY(QString kitId READ kitId NOTIFY changed)
  public:
    DeckExchangeController(
        DesktopBackend&,
        CardKitController&,
        DeckDocumentController&,
        const QString& settingsPath,
        QObject* parent = nullptr
    );
    bool load(const QString& requestPath, const QString& resultPath);
    bool active() const {
        return active_;
    }
    bool needsImport() const {
        return active_ && request_["input"].toObject()["kind"] == "transfer" && document_.isEmpty();
    }
    QString error() const {
        return tr(error_.toUtf8().constData());
    }
    int outputCount() const {
        return outputs_.size();
    }
    QString outputRevision() const {
        return outputs_.isEmpty() ? QString{}
                                  : outputs_.last().toObject()["revision_id"].toString();
    }
    QString projectPath() const;
    QString kitId() const;
    Q_INVOKABLE bool importTo(const QUrl& parentFolder, const QString& name);
    Q_INVOKABLE bool bindCurrent(const QString& kitId);
    Q_INVOKABLE bool
    recordOutput(const QString& path, const QString& kind, const QVariantMap& source);
    Q_INVOKABLE bool finish(const QVariantMap& source);
    Q_INVOKABLE bool cancel();
  signals:
    void changed();
    void completed();

  private:
    void rememberDocument();
    void restoreDocument();
    bool publish(const QString& outcome, const QJsonArray& outputs, const QVariantMap& source = {});
    bool fail(const QString&, const std::exception&);
    DesktopBackend& backend_;
    CardKitController& cards_;
    DeckDocumentController& documents_;
    QSettings settings_;
    QJsonObject request_, document_;
    QJsonArray outputs_;
    QString request_path_, result_path_, request_digest_, error_;
    bool active_ = false;
};
