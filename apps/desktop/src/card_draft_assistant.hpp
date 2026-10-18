//! Owns bounded Infer Runtime candidate requests. Never writes project content.
#pragma once
#include <QJsonObject>
#include <QObject>
#include <QProcess>
#include <QSettings>
#include <QTemporaryFile>
#include <QTimer>
#include <QUrl>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>
#include <memory>

class CardDraftAssistant final : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by Deck")
    Q_PROPERTY(bool running READ running NOTIFY changed)
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
    Q_PROPERTY(bool connected READ connected NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString credentialFile READ credentialFile NOTIFY changed)
    Q_PROPERTY(QVariantList cards READ cards NOTIFY changed)
    Q_PROPERTY(QString targetIdentity READ targetIdentity NOTIFY changed)
  public:
    CardDraftAssistant(QString cli, const QString& settings, QObject* parent = nullptr);
    bool running() const {
        return running_;
    }
    bool busy() const {
        return running_ || process_.state() != QProcess::NotRunning
               || cancel_.state() != QProcess::NotRunning;
    }
    bool connected() const {
        return connected_;
    }
    QString error() const;
    QString status() const;
    QString credentialFile() const {
        return credential_;
    }
    QVariantList cards() const {
        return cards_;
    }
    QString targetIdentity() const {
        return identity_;
    }
    Q_INVOKABLE void probe();
    Q_INVOKABLE bool setCredentialFile(const QUrl& path);
    Q_INVOKABLE bool
    generate(const QString& prompt, int count, const QString& language, const QString& identity);
    Q_INVOKABLE void discard();
  signals:
    void changed();

  private:
    void dispatch(const QString& operation);
    void finished(int code, QProcess::ExitStatus status);
    void readOutput();
    void cancelRemote();
    void finishCancellation(bool confirmed);
    void fail(const QString& code);
    QSettings settings_;
    QString cli_, credential_, operation_, prompt_, language_, identity_, response_, errorCode_;
    int count_ = 6;
    bool running_ = false, connected_ = false, cancelRequested_ = false;
    QVariantList cards_;
    QProcess process_, cancel_;
    QByteArray output_, cancelOutput_;
    QJsonObject terminal_;
    qint64 outputBytes_ = 0;
    QTimer timeout_, cancelTimeout_;
    std::unique_ptr<QTemporaryFile> request_, cancelRequest_;
};
