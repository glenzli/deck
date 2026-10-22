#include "card_layout_preset_store.hpp"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLockFile>
#include <QRegularExpression>
#include <QSaveFile>
#include <QSet>
#include <QStringList>
#include <QUuid>
#include <cmath>

namespace {
constexpr auto schema = "deck.card-layout-presets/1";
constexpr qsizetype maxPresets = 64;
constexpr qint64 maxBytes = 128 * 1024;
bool enumeration(const QVariantMap& source, const char* key, const QStringList& choices) {
    const auto value = source.value(QString::fromLatin1(key));
    return value.metaType().id() == QMetaType::QString && choices.contains(value.toString());
}
bool number(const QVariantMap& source, const char* key, int low, int high) {
    const auto value = QJsonValue::fromVariant(source.value(QString::fromLatin1(key)));
    if (!value.isDouble())
        return false;
    const auto n = value.toDouble();
    return std::isfinite(n) && std::floor(n) == n && n >= low && n <= high;
}
bool blocks(const QVariant& source, QVariantList& result) {
    const auto value = QJsonValue::fromVariant(source);
    if (!value.isArray() || value.toArray().size() > 16)
        return false;
    const QStringList keys{
        "id",
        "field",
        "x",
        "y",
        "width",
        "height",
        "text_scale_percent",
        "text_align",
        "text_vertical_align",
        "bold",
        "italic",
        "text_color",
        "image_fit"
    };
    const QRegularExpression id(QStringLiteral("\\A[A-Za-z0-9_-]{1,64}\\z"));
    const QRegularExpression color(QStringLiteral("\\A#[0-9a-fA-F]{6}\\z"));
    QSet<QString> ids;
    for (const auto& item : value.toArray()) {
        if (!item.isObject())
            return false;
        auto block = item.toObject().toVariantMap();
        if (block.size() != keys.size())
            return false;
        for (const auto& key : keys)
            if (!block.contains(key))
                return false;
        const auto identity = block.value("id").toString();
        if (block.value("id").metaType().id() != QMetaType::QString
            || !id.match(identity).hasMatch() || ids.contains(identity)
            || !enumeration(
                block,
                "field",
                {"title", "rules", "artwork", "card_type", "cost", "stats", "flavor"}
            )
            || !number(block, "x", 0, 10000) || !number(block, "y", 0, 10000)
            || !number(block, "width", 1, 10000) || !number(block, "height", 1, 10000)
            || block.value("x").toInt() + block.value("width").toInt() > 10000
            || block.value("y").toInt() + block.value("height").toInt() > 10000
            || !number(block, "text_scale_percent", 50, 300)
            || !enumeration(block, "text_align", {"left", "center", "right"})
            || !enumeration(block, "text_vertical_align", {"top", "center", "bottom"})
            || block.value("bold").metaType().id() != QMetaType::Bool
            || block.value("italic").metaType().id() != QMetaType::Bool
            || block.value("text_color").metaType().id() != QMetaType::QString
            || !color.match(block.value("text_color").toString()).hasMatch()
            || !enumeration(block, "image_fit", {"contain", "cover"}))
            return false;
        ids.insert(identity);
        block["text_color"] = block.value("text_color").toString().toLower();
        result.push_back(block);
    }
    return true;
}
} // namespace

CardLayoutPresetStore::CardLayoutPresetStore(const QString& settingsPath, QObject* parent)
    : QObject(parent), path_(settingsPath) {
    reload();
}
QString CardLayoutPresetStore::normalizedName(const QString& name) {
    for (qsizetype i = 0; i < name.size(); ++i) {
        const auto character = name[i];
        if (character.category() == QChar::Other_Control
            || character.category() == QChar::Other_Format)
            return {};
        if (character.isHighSurrogate()) {
            if (++i >= name.size() || !name[i].isLowSurrogate())
                return {};
        } else if (character.isLowSurrogate())
            return {};
    }
    const auto result = name.normalized(QString::NormalizationForm_C).trimmed();
    if (result.isEmpty() || result.size() > 80)
        return {};
    return result;
}
QVariantMap CardLayoutPresetStore::normalizedConfiguration(const QVariantMap& source) {
    if (source.value("legacy").toBool()
        || !enumeration(
            source,
            "mode",
            {"auto", "image", "text", "image_text", "classic", "blocks"}
        )
        || !enumeration(source, "image_fit", {"contain", "cover"})
        || !enumeration(source, "text_align", {"left", "center", "right"})
        || !enumeration(
            source,
            "image_position",
            {"auto", "top", "bottom", "left", "right", "background"}
        )
        || !enumeration(source, "text_vertical_align", {"auto", "top", "center", "bottom"})
        || source.value("border").metaType().id() != QMetaType::Bool
        || !number(source, "padding_percent", 0, 20)
        || !number(source, "text_scale_percent", 50, 300)
        || !number(source, "image_share_percent", 20, 80)
        || !number(source, "text_offset_x_percent", -30, 30)
        || !number(source, "text_offset_y_percent", -30, 30))
        return {};
    const QRegularExpression color(QStringLiteral("\\A#[0-9a-fA-F]{6}\\z"));
    for (const auto* key : {"background_color", "text_color"})
        if (source.value(key).metaType().id() != QMetaType::QString
            || !color.match(source.value(key).toString()).hasMatch())
            return {};
    QVariantMap result;
    for (const auto* key :
         {"mode",
          "image_fit",
          "background_color",
          "text_color",
          "border",
          "padding_percent",
          "text_align",
          "text_scale_percent",
          "image_position",
          "image_share_percent",
          "text_vertical_align",
          "text_offset_x_percent",
          "text_offset_y_percent"})
        result.insert(QString::fromLatin1(key), source.value(key));
    result["background_color"] = result.value("background_color").toString().toLower();
    result["text_color"] = result.value("text_color").toString().toLower();
    if (source.contains("blocks")) {
        QVariantList normalized;
        if (!blocks(source.value("blocks"), normalized))
            return {};
        result["blocks"] = normalized;
    } else if (source.value("mode") == "blocks")
        return {};
    return result;
}
bool CardLayoutPresetStore::read(QVariantList& result) const {
    if (path_.isEmpty())
        return false;
    QFile file(path_);
    if (!file.exists()) {
        result.clear();
        return true;
    }
    if (!file.open(QIODevice::ReadOnly) || file.size() > maxBytes)
        return false;
    QJsonParseError error;
    const auto document = QJsonDocument::fromJson(file.readAll(), &error);
    if (error.error != QJsonParseError::NoError || !document.isObject())
        return false;
    const auto root = document.object();
    if (root.size() != 2 || root.value("schema") != QString::fromLatin1(schema)
        || !root.value("presets").isArray() || root.value("presets").toArray().size() > maxPresets)
        return false;
    QVariantList next;
    QSet<QString> ids, names;
    for (const auto& value : root.value("presets").toArray()) {
        if (!value.isObject())
            return false;
        const auto row = value.toObject();
        const auto id = row.value("id").toString();
        const auto name = normalizedName(row.value("name").toString());
        const auto configuration =
            normalizedConfiguration(row.value("configuration").toObject().toVariantMap());
        if (row.size() != 3 || QUuid(id).isNull() || QUuid(id).toString(QUuid::WithoutBraces) != id
            || name.isEmpty() || name != row.value("name").toString() || ids.contains(id)
            || names.contains(name.toCaseFolded()) || configuration.isEmpty()
            || row.value("configuration").toObject().size() != configuration.size())
            return false;
        ids.insert(id);
        names.insert(name.toCaseFolded());
        next.push_back(QVariantMap{{"id", id}, {"name", name}, {"configuration", configuration}});
    }
    result = next;
    return true;
}
bool CardLayoutPresetStore::write(const QVariantList& presets) const {
    QSaveFile file(path_);
    const auto bytes = QJsonDocument(
                           QJsonObject{
                               {"schema", QString::fromLatin1(schema)},
                               {"presets", QJsonArray::fromVariantList(presets)}
                           }
    ).toJson(QJsonDocument::Compact);
    if (bytes.size() > maxBytes || !file.open(QIODevice::WriteOnly))
        return false;
    if (file.write(bytes) != bytes.size()) {
        file.cancelWriting();
        return false;
    }
    return file.commit();
}
bool CardLayoutPresetStore::fail(const char* message) {
    error_ = QString::fromUtf8(message);
    emit changed();
    return false;
}
bool CardLayoutPresetStore::reload() {
    QVariantList latest;
    if (!read(latest))
        return fail(QT_TR_NOOP("Could not read the personal presets. The saved file was kept."));
    presets_ = latest;
    error_.clear();
    emit changed();
    return true;
}
QString CardLayoutPresetStore::savePreset(
    const QString& name,
    const QVariantMap& resolvedConfiguration,
    const QString& replaceId
) {
    const auto normalized = normalizedName(name);
    const auto configuration = normalizedConfiguration(resolvedConfiguration);
    if (normalized.isEmpty()) {
        fail(
            QT_TR_NOOP("Use a preset name of 1 to 80 characters without hidden control characters.")
        );
        return {};
    }
    if (configuration.isEmpty()) {
        fail(
            QT_TR_NOOP("Could not save this layout. Choose a complete flexible card layout first.")
        );
        return {};
    }
    if (path_.isEmpty() || !QDir().mkpath(QFileInfo(path_).absolutePath())) {
        fail(QT_TR_NOOP("Could not save the preset. Keep the name and try again."));
        return {};
    }
    QLockFile lock(path_ + ".lock");
    if (!lock.tryLock(0)) {
        fail(QT_TR_NOOP("The personal presets are busy. Try saving again."));
        return {};
    }
    QVariantList latest;
    if (!read(latest)) {
        fail(QT_TR_NOOP("Could not read the personal presets. The saved file was kept."));
        return {};
    }
    qsizetype replacement = -1;
    for (qsizetype i = 0; i < latest.size(); ++i) {
        const auto row = latest[i].toMap();
        if (row.value("id").toString() == replaceId)
            replacement = i;
        else if (row.value("name").toString().toCaseFolded() == normalized.toCaseFolded()) {
            fail(QT_TR_NOOP("That preset name already exists. Choose another name to keep it."));
            return {};
        }
    }
    if (!replaceId.isEmpty() && replacement < 0) {
        fail(QT_TR_NOOP("That preset was removed. Save this layout with a new name."));
        return {};
    }
    if (replacement < 0 && latest.size() >= maxPresets) {
        fail(
            QT_TR_NOOP("You can save up to 64 personal presets. Remove one before adding another.")
        );
        return {};
    }
    const auto id =
        replacement >= 0 ? replaceId : QUuid::createUuid().toString(QUuid::WithoutBraces);
    const QVariantMap row{{"id", id}, {"name", normalized}, {"configuration", configuration}};
    if (replacement >= 0)
        latest[replacement] = row;
    else
        latest.push_back(row);
    if (!write(latest)) {
        fail(QT_TR_NOOP("Could not save the preset. Keep the name and try again."));
        return {};
    }
    presets_ = latest;
    error_.clear();
    emit changed();
    return id;
}
bool CardLayoutPresetStore::removePreset(const QString& id) {
    QLockFile lock(path_ + ".lock");
    if (path_.isEmpty() || !lock.tryLock(0))
        return fail(QT_TR_NOOP("The personal presets are busy. Try saving again."));
    QVariantList latest;
    if (!read(latest))
        return fail(QT_TR_NOOP("Could not read the personal presets. The saved file was kept."));
    qsizetype index = -1;
    for (qsizetype i = 0; i < latest.size(); ++i)
        if (latest[i].toMap().value("id").toString() == id)
            index = i;
    if (index < 0)
        return fail(QT_TR_NOOP("That preset was removed. Save this layout with a new name."));
    latest.removeAt(index);
    if (!write(latest))
        return fail(QT_TR_NOOP("Could not save the preset. Keep the name and try again."));
    presets_ = latest;
    error_.clear();
    emit changed();
    return true;
}
QVariantMap CardLayoutPresetStore::configuration(const QString& id) const {
    for (const auto& value : presets_) {
        const auto row = value.toMap();
        if (row.value("id").toString() == id)
            return row.value("configuration").toMap();
    }
    return {};
}
