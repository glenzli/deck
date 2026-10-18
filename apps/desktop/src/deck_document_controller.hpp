//! Deck document admission and application integration, separate from card composition.
#pragma once
#include "deck_cli_client.hpp"
#include <QObject>
#include <QSettings>
#include <QUrl>
#include <QtQml/qqmlregistration.h>
class DesktopBackend;
class DeckDocumentController final : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by Deck")
    Q_PROPERTY(QString error READ error NOTIFY statusChanged)
    Q_PROPERTY(QString shapeExecutable READ shapeExecutable NOTIFY statusChanged)
    Q_PROPERTY(QStringList recentProjects READ recentProjects NOTIFY recentProjectsChanged)
  public:
    DeckDocumentController(
        DesktopBackend&,
        const DeckCliClient&,
        const QString& settingsPath,
        QObject* parent = nullptr
    );
    QString error() const {
        return tr(error_.toUtf8().constData());
    }
    QString shapeExecutable() const;
    QStringList recentProjects() const;
    Q_INVOKABLE bool createProject(const QUrl& parent, const QString& name);
    Q_INVOKABLE bool openProject(const QUrl& path);
    Q_INVOKABLE bool openLocalProject(const QString& path) {
        return openProject(QUrl::fromLocalFile(path));
    }
    Q_INVOKABLE bool openShape();
    Q_INVOKABLE bool setShapeExecutable(const QUrl& path);
    Q_INVOKABLE void copyText(const QString& text);
    const DeckCliClient& cli() const {
        return cli_;
    }
  signals:
    void statusChanged();
    void recentProjectsChanged();

  private:
    bool fail(const QString& userMessage, const std::exception& error);
    void remember();
    DesktopBackend& backend_;
    const DeckCliClient& cli_;
    QSettings settings_;
    QString error_;
};
