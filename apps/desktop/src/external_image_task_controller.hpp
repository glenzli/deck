//! A real Shape image task: captured card identity, independent process and explicit local use.
#pragma once
#include "deck_cli_client.hpp"
#include <QFutureWatcher>
#include <QJsonObject>
#include <QObject>
#include <QSettings>
#include <QTimer>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
class DesktopBackend;
class CardKitController;
class DeckDocumentController;
class ExternalImageTaskController final : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by Deck")
    Q_PROPERTY(bool running READ running NOTIFY changed)
    Q_PROPERTY(bool resultReady READ resultReady NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString previewSource READ previewSource NOTIFY changed)
    Q_PROPERTY(QVariantMap target READ target NOTIFY changed)
    Q_PROPERTY(bool targetCurrent READ targetCurrent NOTIFY changed)
    Q_PROPERTY(QString prompt READ prompt NOTIFY changed)
    Q_PROPERTY(QString projectPath READ projectPath NOTIFY changed)
  public:
    ExternalImageTaskController(
        DesktopBackend&,
        CardKitController&,
        DeckDocumentController&,
        const QString& settings,
        QObject* parent = nullptr
    );
    ~ExternalImageTaskController() override;
    bool running() const {
        return probing_ || waiting_;
    }
    bool resultReady() const {
        return ready_;
    }
    QString error() const {
        return tr(error_.toUtf8().constData());
    }
    QString status() const;
    QString previewSource() const {
        return preview_;
    }
    QVariantMap target() const {
        return request_["target"].toObject().toVariantMap();
    }
    bool targetCurrent() const;
    QString projectPath() const {
        return project_path_;
    }
    QString prompt() const {
        return request_["prompt"].toString();
    }
    Q_INVOKABLE bool begin(const QString& prompt, int width, int height, const QString& cardId);
    Q_INVOKABLE QVariantMap importReturnedImage();
    Q_INVOKABLE void stopWaiting();
    Q_INVOKABLE void consumed();
    void poll();
  signals:
    void changed();

  private:
    void launch();
    void ended(bool started);
    void remember();
    void restore();
    bool fail(const QString&, const std::exception&);
    DesktopBackend& backend_;
    CardKitController& cards_;
    DeckDocumentController& documents_;
    QSettings settings_;
    QTimer poll_;
    QFutureWatcher<QString> probe_;
    QJsonObject request_, imported_;
    QString request_path_, result_path_, request_digest_, project_path_, executable_, error_,
        preview_;
    qint64 pid_ = 0;
    quint64 generation_ = 0, probe_generation_ = 0;
    bool probing_ = false, waiting_ = false, ready_ = false;
};
