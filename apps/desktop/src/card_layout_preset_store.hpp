//! Personal card-face layout library. Never owns project content or material references.
#pragma once
#include <QObject>
#include <QString>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class CardLayoutPresetStore final : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by Deck")
    Q_PROPERTY(QVariantList presets READ presets NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
  public:
    explicit CardLayoutPresetStore(const QString& settingsPath, QObject* parent = nullptr);
    QVariantList presets() const {
        return presets_;
    }
    QString error() const {
        return tr(error_.toUtf8().constData());
    }
    Q_INVOKABLE QString savePreset(
        const QString& name,
        const QVariantMap& resolvedConfiguration,
        const QString& replaceId = QString{}
    );
    Q_INVOKABLE bool removePreset(const QString& id);
    Q_INVOKABLE QVariantMap configuration(const QString& id) const;
    Q_INVOKABLE QVariantMap validatedConfiguration(const QVariantMap& source) const {
        return normalizedConfiguration(source);
    }
    Q_INVOKABLE bool reload();
    void retranslate() {
        emit changed();
    }
  signals:
    void changed();

  private:
    static QVariantMap normalizedConfiguration(const QVariantMap& source);
    static QString normalizedName(const QString& name);
    bool read(QVariantList& result) const;
    bool write(const QVariantList& presets) const;
    bool fail(const char* message);
    QString path_, error_;
    QVariantList presets_;
};
