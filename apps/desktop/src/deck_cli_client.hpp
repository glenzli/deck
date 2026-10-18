//! Bounded invocation of Deck's packaged CLI and atomic new-file receipts.
#pragma once
#include <QJsonObject>
#include <QString>
#include <QStringList>
class DeckCliClient {
  public:
    explicit DeckCliClient(QString executable);
    const QString& executable() const {
        return executable_;
    }
    QJsonObject run(const QStringList& arguments, int timeoutMs = 30000) const;
    QJsonObject hashFile(const QString& path) const;
    static QJsonObject readJson(const QString& path, qint64 maximumBytes = 1024 * 1024);
    static void checkNewOutput(const QString& path);
    static void publishJson(const QString& path, const QJsonObject& object);

  private:
    QString executable_;
};
