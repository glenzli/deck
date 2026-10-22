#include "card_layout_preset_store_smoke.hpp"
#include "card_layout_preset_store.hpp"
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLockFile>
#include <QStringList>

namespace {
QByteArray bytes(const QString& path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return {};
    return file.readAll();
}
QVariantMap layout() {
    return {
        {"mode", "image_text"},
        {"image_fit", "contain"},
        {"background_color", "#aAbBcC"},
        {"text_color", "#202020"},
        {"border", true},
        {"padding_percent", 8},
        {"text_align", "center"},
        {"text_scale_percent", 120},
        {"image_position", "left"},
        {"image_share_percent", 40},
        {"text_vertical_align", "center"},
        {"text_offset_x_percent", 2},
        {"text_offset_y_percent", -1}
    };
}
QVariantMap block(const QString& id, const QString& field) {
    return {
        {"id", id},
        {"field", field},
        {"x", 0},
        {"y", 0},
        {"width", 10000},
        {"height", 10000},
        {"text_scale_percent", 100},
        {"text_align", "left"},
        {"text_vertical_align", "top"},
        {"bold", false},
        {"italic", false},
        {"text_color", "#aAbBcC"},
        {"image_fit", "contain"}
    };
}
QVariantMap blockLayout() {
    auto result = layout();
    result["mode"] = "blocks";
    auto heading = block("heading", "title");
    heading["height"] = 2000;
    heading["bold"] = true;
    auto body = block("body", "rules");
    body["y"] = 2000;
    body["height"] = 6000;
    auto repeated = block("repeated_title", "title");
    repeated["y"] = 8000;
    repeated["height"] = 2000;
    repeated["italic"] = true;
    result["blocks"] = QVariantList{heading, body, repeated};
    return result;
}
bool writeBytes(const QString& path, const QByteArray& data) {
    QFile file(path);
    return file.open(QIODevice::WriteOnly | QIODevice::Truncate) && file.write(data) == data.size();
}
bool verifyBlocks(const QString& path) {
    CardLayoutPresetStore store(path);
    const auto oldId = store.savePreset("Old 13-field preset", layout());
    const auto oldConfiguration = store.configuration(oldId);
    if (oldId.isEmpty() || oldConfiguration.size() != 13)
        return false;
    CardLayoutPresetStore oldReader(path);
    if (oldReader.configuration(oldId) != oldConfiguration)
        return false;
    auto source = blockLayout();
    source["canvas_width"] = 1200;
    source["canvas_height"] = 800;
    source["legacy"] = false;
    source["title"] = "content-must-not-enter-presets";
    source["artwork"] = QVariantMap{{"artifact_id", "content-must-not-enter-presets"}};
    const auto id = store.savePreset("Bound blocks", source);
    const auto configuration = store.configuration(id);
    const auto ordered = configuration.value("blocks").toList();
    if (id.isEmpty() || configuration.size() != 14 || ordered.size() != 3
        || ordered[0].toMap().value("id") != "heading"
        || ordered[2].toMap().value("field") != "title"
        || ordered[0].toMap().value("text_color") != "#aabbcc"
        || bytes(path).contains("content-must-not-enter-presets"))
        return false;
    CardLayoutPresetStore reopened(path);
    if (reopened.configuration(id) != configuration
        || reopened.validatedConfiguration(configuration) != configuration
        || reopened.configuration(oldId) != oldConfiguration)
        return false;
    auto empty = blockLayout();
    empty["blocks"] = QVariantList{};
    const auto emptyId = store.savePreset("Empty blocks draft", empty);
    auto dormant = blockLayout();
    dormant["mode"] = "classic";
    const auto dormantId = store.savePreset("Dormant blocks", dormant);
    QVariantList maximum;
    const QStringList fields{"title", "rules", "artwork", "card_type", "cost", "stats", "flavor"};
    for (int index = 0; index < 16; ++index) {
        auto value = block("block_" + QString::number(index), fields[index % fields.size()]);
        value["text_scale_percent"] = index == 0 ? 50 : 300;
        value["image_fit"] = index % 2 == 0 ? "contain" : "cover";
        maximum.push_back(value);
    }
    auto full = blockLayout();
    full["blocks"] = maximum;
    const auto maximumId = store.savePreset("Sixteen blocks", full);
    if (emptyId.isEmpty() || dormantId.isEmpty() || maximumId.isEmpty())
        return false;
    CardLayoutPresetStore allReopened(path);
    if (!allReopened.configuration(emptyId).contains("blocks")
        || !allReopened.configuration(emptyId).value("blocks").toList().isEmpty()
        || allReopened.configuration(dormantId).value("mode") != "classic"
        || allReopened.configuration(dormantId).value("blocks").toList() != ordered
        || allReopened.configuration(maximumId).value("blocks").toList().size() != 16
        || allReopened.configuration(oldId) != oldConfiguration)
        return false;
    const auto stable = bytes(path);
    const auto lastGood = store.presets();
    QList<QVariantMap> invalidLayouts;
    auto invalid = blockLayout();
    invalid.remove("blocks");
    invalidLayouts.push_back(invalid);
    for (const auto& value :
         QVariantList{QVariant{}, QString("[]"), QVariantMap{}, QVariantList{QString("block")}}) {
        invalid = blockLayout();
        invalid["blocks"] = value;
        invalidLayouts.push_back(invalid);
    }
    maximum.push_back(block("seventeenth", "rules"));
    invalid = blockLayout();
    invalid["blocks"] = maximum;
    invalidLayouts.push_back(invalid);
    invalid = blockLayout();
    invalid["blocks"] = QVariantList{block("duplicate", "title"), block("duplicate", "rules")};
    invalidLayouts.push_back(invalid);
    for (const auto* key : {"background_color", "text_color"}) {
        invalid = blockLayout();
        invalid[key] = "#123456\n";
        invalidLayouts.push_back(invalid);
    }
    const auto addInvalidBlock = [&](const QVariantMap& value) {
        auto candidate = blockLayout();
        candidate["blocks"] = QVariantList{value};
        invalidLayouts.push_back(candidate);
    };
    const auto baseline = block("valid", "title");
    for (auto it = baseline.cbegin(); it != baseline.cend(); ++it) {
        auto missing = baseline;
        missing.remove(it.key());
        addInvalidBlock(missing);
    }
    for (const auto& value : QVariantList{
             QString{},
             QString("two words"),
             QString("valid\n"),
             QString(65, 'x'),
             QString::fromUtf8("标题"),
             123,
             true
         }) {
        auto bad = baseline;
        bad["id"] = value;
        addInvalidBlock(bad);
    }
    const QVariantList invalidValues{
        QVariantMap{{"x", -1}},
        QVariantMap{{"y", -1}},
        QVariantMap{{"width", 0}},
        QVariantMap{{"height", 10001}},
        QVariantMap{{"x", 9000}, {"width", 2000}},
        QVariantMap{{"y", 9000}, {"height", 2000}},
        QVariantMap{{"x", 0.5}},
        QVariantMap{{"width", "10000"}},
        QVariantMap{{"height", true}},
        QVariantMap{{"text_scale_percent", 49}},
        QVariantMap{{"text_scale_percent", 301}},
        QVariantMap{{"field", "custom_text"}},
        QVariantMap{{"text_align", "auto"}},
        QVariantMap{{"text_vertical_align", "auto"}},
        QVariantMap{{"bold", "true"}},
        QVariantMap{{"italic", 1}},
        QVariantMap{{"text_color", "#123456\n"}},
        QVariantMap{{"image_fit", "stretch"}},
        QVariantMap{{"text", "content-must-not-enter-presets"}},
        QVariantMap{{"artwork", QVariantMap{{"artifact_id", "foreign-pin"}}}}
    };
    for (const auto& value : invalidValues) {
        auto bad = baseline;
        const auto changes = value.toMap();
        for (auto it = changes.cbegin(); it != changes.cend(); ++it)
            bad[it.key()] = it.value();
        addInvalidBlock(bad);
    }
    for (const auto& candidate : invalidLayouts)
        if (!store.savePreset("Invalid blocks", candidate).isEmpty() || bytes(path) != stable
            || store.presets() != lastGood || !store.validatedConfiguration(candidate).isEmpty())
            return false;
    // Disk input is exact: projection-only fields must not be silently accepted from a file.
    invalid = configuration;
    invalid["rules"] = "content-must-not-enter-presets";
    invalidLayouts.push_back(invalid);
    const auto root = QJsonDocument::fromJson(stable).object();
    const auto rows = root.value("presets").toArray();
    int rowIndex = -1;
    for (qsizetype index = 0; index < rows.size(); ++index)
        if (rows[index].toObject().value("id") == id)
            rowIndex = int(index);
    if (rowIndex < 0 || root.value("schema") != "deck.card-layout-presets/1")
        return false;
    for (const auto& candidate : invalidLayouts) {
        auto corrupted = root;
        auto changedRows = rows;
        auto row = changedRows[rowIndex].toObject();
        row["configuration"] = QJsonObject::fromVariantMap(candidate);
        changedRows[rowIndex] = row;
        corrupted["presets"] = changedRows;
        const auto damaged = QJsonDocument(corrupted).toJson(QJsonDocument::Compact);
        if (!writeBytes(path, damaged) || store.reload() || store.presets() != lastGood
            || !store.savePreset("Preserve damaged file", blockLayout()).isEmpty()
            || store.removePreset(id) || bytes(path) != damaged)
            return false;
    }
    return writeBytes(path, stable) && store.reload() && store.presets() == lastGood;
}
} // namespace
bool card_layout_preset_store_smoke::verify(const QString& directory) {
    QDir().mkpath(directory);
    const auto path = QDir(directory).filePath("presets.json");
    CardLayoutPresetStore store(path);
    auto resolved = layout();
    resolved["canvas_width"] = 750;
    resolved["canvas_height"] = 1050;
    resolved["legacy"] = false;
    resolved["artwork"] = QVariantMap{{"artifact_id", "must-not-be-stored"}};
    resolved["rules"] = "must-not-be-stored";
    const auto first = store.savePreset("  Study notes  ", resolved);
    if (first.isEmpty() || store.presets().size() != 1 || store.configuration(first).size() != 13
        || store.configuration(first).value("background_color") != "#aabbcc"
        || bytes(path).contains("must-not-be-stored"))
        return false;
    CardLayoutPresetStore reopened(path);
    if (reopened.presets() != store.presets() || reopened.configuration(first).isEmpty())
        return false;
    const auto before = bytes(path);
    if (!store.savePreset("study NOTES", layout()).isEmpty() || store.error().isEmpty()
        || bytes(path) != before || store.presets().size() != 1)
        return false;
    for (const auto& invalid : {QString{}, QString("hidden") + QChar(0x0085), QString(81, 'x')})
        if (!store.savePreset(invalid, layout()).isEmpty() || bytes(path) != before)
            return false;
    for (const auto& key :
         {"mode", "image_fit", "text_align", "image_position", "text_vertical_align", "border"}) {
        auto invalid = layout();
        invalid.remove(key);
        if (!store.savePreset("Incomplete", invalid).isEmpty())
            return false;
    }
    for (const auto& key :
         {"padding_percent",
          "text_scale_percent",
          "image_share_percent",
          "text_offset_x_percent",
          "text_offset_y_percent"}) {
        auto invalid = layout();
        invalid[key] = 999;
        if (!store.savePreset("Unbounded", invalid).isEmpty())
            return false;
        invalid[key] = "100";
        if (!store.savePreset("Numeric text", invalid).isEmpty())
            return false;
    }
    auto invalid = layout();
    invalid["background_color"] = "red";
    if (!store.savePreset("Invalid color", invalid).isEmpty())
        return false;
    invalid = layout();
    invalid["legacy"] = true;
    if (!store.savePreset("Original layout", invalid).isEmpty())
        return false;
    const auto second = reopened.savePreset("Second", layout());
    if (second.isEmpty() || second == first)
        return false;
    // A stale in-memory store merges the latest library instead of dropping another preset.
    auto changed = layout();
    changed["padding_percent"] = 10;
    if (store.savePreset("Study notes", changed, first) != first || store.presets().size() != 2
        || store.configuration(second) != reopened.configuration(second))
        return false;
    const auto stable = bytes(path);
    if (!store.savePreset("Second", changed, first).isEmpty() || bytes(path) != stable)
        return false;
    {
        QLockFile lock(path + ".lock");
        if (!lock.tryLock(0) || !store.savePreset("Blocked", layout()).isEmpty()
            || bytes(path) != stable || store.presets().size() != 2)
            return false;
    }
    if (!store.removePreset(first) || store.presets().size() != 1
        || store.configuration(second).isEmpty())
        return false;
    const auto lastGood = store.presets();
    {
        QFile file(path);
        if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate) || file.write("not json") != 8)
            return false;
    }
    if (store.reload() || store.presets() != lastGood
        || !store.savePreset("Keep editing", layout()).isEmpty() || bytes(path) != "not json")
        return false;
    return verifyBlocks(QDir(directory).filePath("block-presets.json"));
}
