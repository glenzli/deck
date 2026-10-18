#include "deck_exchange_controller.hpp"
#include "card_kit_controller.hpp"
#include "deck_document_controller.hpp"
#include "desktop_backend.hpp"
#include <QDebug>
#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QRegularExpression>
#include <stdexcept>
namespace {
void requireKeys(const QJsonObject& object, const QStringList& allowed) {
    if (object.size() != allowed.size())
        throw std::runtime_error("missing handoff field");
    for (auto i = object.begin(); i != object.end(); ++i)
        if (!allowed.contains(i.key()))
            throw std::runtime_error("unknown handoff field");
}
bool identity(const QString& value) {
    static const QRegularExpression pattern("^[A-Za-z0-9._-]{1,128}$");
    return pattern.match(value).hasMatch();
}
bool digest(const QString& value) {
    static const QRegularExpression pattern("^[0-9a-f]{64}$");
    return pattern.match(value).hasMatch();
}
} // namespace
DeckExchangeController::DeckExchangeController(
    DesktopBackend& backend,
    CardKitController& cards,
    DeckDocumentController& documents,
    const QString& settingsPath,
    QObject* parent
)
    : QObject(parent), backend_(backend), cards_(cards), documents_(documents),
      settings_(settingsPath, QSettings::IniFormat) {}
bool DeckExchangeController::fail(const QString& message, const std::exception& error) {
    qWarning().noquote() << "Deck exchange:" << error.what();
    error_ = message;
    emit changed();
    return false;
}
bool DeckExchangeController::load(const QString& requestPath, const QString& resultPath) {
    try {
        DeckCliClient::checkNewOutput(resultPath);
        const auto request = DeckCliClient::readJson(requestPath, 65536);
        requireKeys(request, {"schema", "request_id", "origin", "input"});
        if (request["schema"] != "deck.external-edit/1"
            || !identity(request["request_id"].toString()))
            throw std::runtime_error("unsupported handoff schema");
        const auto origin = request["origin"].toObject();
        requireKeys(origin, {"producer", "project_id", "kit_id", "expected_head", "draft_digest"});
        if (origin["producer"] != "shape" || !identity(origin["project_id"].toString())
            || !identity(origin["kit_id"].toString()) || !digest(origin["draft_digest"].toString()))
            throw std::runtime_error("invalid Shape origin");
        const auto input = request["input"].toObject();
        if (input["kind"] == "transfer") {
            requireKeys(input, {"kind", "manifest_path", "manifest_blake3"});
            if (!QFileInfo(input["manifest_path"].toString()).isAbsolute()
                || !digest(input["manifest_blake3"].toString()))
                throw std::runtime_error("invalid transfer input");
        } else if (input["kind"] == "project") {
            requireKeys(input, {"kind", "project_path", "project_id", "kit_id"});
            if (!QFileInfo(input["project_path"].toString()).isAbsolute()
                || !identity(input["project_id"].toString())
                || !identity(input["kit_id"].toString()))
                throw std::runtime_error("invalid project input");
        } else
            throw std::runtime_error("unsupported handoff input");
        const auto digest = documents_.cli().hashFile(requestPath)["blake3"].toString();
        if (digest.size() != 64 || DeckCliClient::readJson(requestPath, 65536) != request)
            throw std::runtime_error("request changed during admission");
        request_ = request;
        request_path_ = requestPath;
        result_path_ = resultPath;
        request_digest_ = digest;
        active_ = true;
        outputs_ = {};
        document_ = {};
        restoreDocument();
        error_.clear();
        emit changed();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "This Shape handoff is invalid or belongs to an unsupported version. Reopen the "
                "cards from an updated Shape."
            )),
            error
        );
    }
}
void DeckExchangeController::rememberDocument() {
    if (request_digest_.isEmpty() || document_.isEmpty())
        return;
    settings_.setValue(
        "deck/exchanges/" + request_digest_,
        QJsonDocument(document_).toJson(QJsonDocument::Compact)
    );
    settings_.sync();
}
void DeckExchangeController::restoreDocument() {
    const auto saved =
        QJsonDocument::fromJson(settings_.value("deck/exchanges/" + request_digest_).toByteArray())
            .object();
    if (saved.isEmpty())
        return;
    try {
        const auto inspected = documents_.cli().run({"status", saved["project_path"].toString()});
        if (!inspected["open_guard"].toObject()["allowed"].toBool()
            || inspected["project"].toObject()["id"] != saved["project_id"])
            return;
        for (const auto& value : inspected["kits"].toArray()) {
            if (value.toObject()["artifact_id"] == saved["kit_id"]) {
                document_ = saved;
                return;
            }
        }
    } catch (const std::exception&) {
        // An unavailable own project must never turn into a foreign or stale identity.
    }
}
QString DeckExchangeController::projectPath() const {
    return document_.isEmpty() ? request_["input"].toObject()["project_path"].toString()
                               : document_["project_path"].toString();
}
QString DeckExchangeController::kitId() const {
    return document_.isEmpty() ? request_["input"].toObject()["kit_id"].toString()
                               : document_["kit_id"].toString();
}
bool DeckExchangeController::importTo(const QUrl& parentFolder, const QString& name) {
    try {
        if (!needsImport() || !parentFolder.isLocalFile() || name.trimmed().isEmpty()
            || name.contains('/') || name.contains('\\'))
            throw std::runtime_error("invalid import destination");
        const auto destination =
            QDir(parentFolder.toLocalFile()).filePath(name.trimmed() + ".deck");
        const auto input = request_["input"].toObject();
        const auto manifestPath = input["manifest_path"].toString();
        const auto manifest = DeckCliClient::readJson(manifestPath, 4 * 1024 * 1024);
        const auto verified = documents_.cli().hashFile(manifestPath);
        if (verified["blake3"] != input["manifest_blake3"]
            || manifest["schema"] != "deck.card-transfer/1"
            || manifest["request_id"] != request_["request_id"]
            || manifest["origin"] != request_["origin"]
            || DeckCliClient::readJson(manifestPath, 4 * 1024 * 1024) != manifest)
            throw std::runtime_error("transfer does not match the requested Shape card set");
        const auto imported = documents_.cli().run(
            {"import-transfer",
             destination,
             manifestPath,
             "--expected-blake3",
             input["manifest_blake3"].toString()},
            120000
        );
        if (imported["request_id"] != request_["request_id"]
            || imported["origin"] != request_["origin"]
            || imported["manifest_blake3"] != input["manifest_blake3"])
            throw std::runtime_error("import receipt does not match this exact handoff");
        if (!documents_.openProject(QUrl::fromLocalFile(destination)))
            throw std::runtime_error("imported project could not open");
        const auto importedKit = imported["kit_id"].toString();
        if (imported["project_id"].toString() != backend_.projectId() || importedKit.isEmpty())
            throw std::runtime_error("imported identity mismatch");
        document_ = {
            {"project_path", backend_.bundlePath()},
            {"project_id", backend_.projectId()},
            {"kit_id", importedKit},
            {"accepted_revision_id", QJsonValue::Null}
        };
        rememberDocument();
        error_.clear();
        emit changed();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "Could not copy these cards into Deck. Choose a new project name or folder and try "
                "again. The Shape original is unchanged."
            )),
            error
        );
    }
}
bool DeckExchangeController::bindCurrent(const QString& kit) {
    if (!active_)
        return true;
    const auto input = request_["input"].toObject();
    const auto expected = document_.isEmpty() ? input : document_;
    if (QFileInfo(expected["project_path"].toString()).canonicalFilePath() != backend_.bundlePath()
        || expected["project_id"].toString() != backend_.projectId()
        || expected["kit_id"].toString() != kit) {
        error_ = QString::fromUtf8(QT_TR_NOOP(
            "This handoff belongs to another Deck project or card set. Reopen the requested "
            "cards before returning outputs."
        ));
        emit changed();
        return false;
    }
    document_ = {
        {"project_path", backend_.bundlePath()},
        {"project_id", backend_.projectId()},
        {"kit_id", kit},
        {"accepted_revision_id", QJsonValue::Null}
    };
    rememberDocument();
    error_.clear();
    emit changed();
    return true;
}
bool DeckExchangeController::recordOutput(
    const QString& rawPath,
    const QString& kind,
    const QVariantMap& source
) {
    try {
        if (!active_)
            return true;
        if (!bindCurrent(source.value("artifact_id").toString())
            || source.value("revision_id").toString().isEmpty())
            return false;
        if (outputs_.size() >= 16 || (kind != "pdf" && kind != "zip"))
            throw std::runtime_error("unsupported output");
        const auto url = QUrl(rawPath);
        const auto path = url.isLocalFile() ? url.toLocalFile() : rawPath;
        auto hash = documents_.cli().hashFile(path);
        hash["kind"] = kind == "pdf" ? "print_pdf" : "materials_zip";
        hash["media_type"] = kind == "pdf" ? "application/pdf" : "application/zip";
        hash["revision_id"] = source.value("revision_id").toString();
        for (qsizetype index = outputs_.size() - 1; index >= 0; --index)
            if (outputs_[index].toObject()["path"] == hash["path"])
                outputs_.removeAt(index);
        outputs_.append(hash);
        error_.clear();
        emit changed();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "The file was exported, but could not be verified for return to Shape. Export it "
                "again to a new file."
            )),
            error
        );
    }
}
bool DeckExchangeController::publish(
    const QString& outcome,
    const QJsonArray& outputs,
    const QVariantMap& source
) {
    try {
        auto document = document_;
        if (!document.isEmpty())
            document["accepted_revision_id"] =
                source.value("revision_id").toString().isEmpty()
                    ? QJsonValue::Null
                    : QJsonValue(source.value("revision_id").toString());
        DeckCliClient::publishJson(
            result_path_,
            {{"schema", "deck.external-edit-result/1"},
             {"request_id", request_["request_id"]},
             {"request_blake3", request_digest_},
             {"outcome", outcome},
             {"document", document.isEmpty() ? QJsonValue::Null : QJsonValue(document)},
             {"outputs", outputs},
             {"error", QJsonValue::Null}}
        );
        active_ = false;
        error_.clear();
        emit changed();
        emit completed();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "Could not return the result to Shape. Keep this window open and choose a new "
                "handoff from Shape."
            )),
            error
        );
    }
}
bool DeckExchangeController::finish(const QVariantMap& source) {
    try {
        if (!cards_.currentAccepted() || cards_.source() != source) {
            error_ = QString::fromUtf8(
                QT_TR_NOOP("Accept and export the current card version before returning outputs.")
            );
            emit changed();
            return false;
        }
        if (!active_ || !bindCurrent(source.value("artifact_id").toString())
            || source.value("revision_id").toString().isEmpty())
            return false;
        QJsonArray verified;
        qint64 total = 0;
        for (const auto& value : outputs_) {
            auto output = value.toObject();
            if (output["revision_id"].toString() != source.value("revision_id").toString())
                continue;
            const auto hash = documents_.cli().hashFile(output["path"].toString());
            if (hash["blake3"] != output["blake3"] || hash["byte_length"] != output["byte_length"])
                throw std::runtime_error("output changed since export");
            total += output["byte_length"].toInteger();
            output.remove("revision_id");
            verified.append(output);
        }
        if (verified.isEmpty() || total > 512LL * 1024 * 1024) {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "Export a print PDF or material archive for the accepted card version, then "
                "choose Return outputs."
            ));
            emit changed();
            return false;
        }
        return publish("completed", verified, source);
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "An exported file changed. Export the current accepted version again before "
                "returning it."
            )),
            error
        );
    }
}
bool DeckExchangeController::cancel() {
    return !active_ || publish("cancelled", {});
}
