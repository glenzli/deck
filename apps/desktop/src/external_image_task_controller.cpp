#include "external_image_task_controller.hpp"
#include "card_kit_controller.hpp"
#include "deck_document_controller.hpp"
#include "desktop_backend.hpp"
#include "process_liveness.hpp"
#include <QDebug>
#include <QDir>
#include <QFileInfo>
#include <QImageReader>
#include <QJsonArray>
#include <QJsonDocument>
#include <QProcess>
#include <QStandardPaths>
#include <QTemporaryFile>
#include <QUrl>
#include <QUuid>
#include <QtConcurrent/QtConcurrentRun>
#include <stdexcept>
ExternalImageTaskController::ExternalImageTaskController(
    DesktopBackend& backend,
    CardKitController& cards,
    DeckDocumentController& documents,
    const QString& settings,
    QObject* parent
)
    : QObject(parent), backend_(backend), cards_(cards), documents_(documents),
      settings_(settings, QSettings::IniFormat) {
    poll_.setInterval(500);
    connect(&poll_, &QTimer::timeout, this, &ExternalImageTaskController::poll);
    connect(&backend_, &DesktopBackend::projectChanged, this, [this] { emit changed(); });
    connect(&cards_, &CardKitController::documentChanged, this, [this] { emit changed(); });
    connect(&probe_, &QFutureWatcher<QString>::finished, this, [this] {
        probing_ = false;
        if (probe_generation_ != generation_) {
            emit changed();
            return;
        }
        const auto result = probe_.result();
        if (!result.isEmpty()) {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "This Shape application does not support Deck image tasks. Choose an updated "
                "Shape in Settings, then try again."
            ));
            waiting_ = false;
            remember();
            emit changed();
            return;
        }
        if (!targetCurrent()) {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "The original card changed before Shape opened. Check the card and try again."
            ));
            waiting_ = false;
            remember();
            emit changed();
            return;
        }
        launch();
    });
    restore();
}
ExternalImageTaskController::~ExternalImageTaskController() {
    probe_.waitForFinished();
}
bool ExternalImageTaskController::fail(const QString& message, const std::exception& error) {
    qWarning().noquote() << "Deck image task:" << error.what();
    error_ = message;
    waiting_ = false;
    probing_ = false;
    ready_ = false;
    preview_.clear();
    remember();
    emit changed();
    return false;
}
QString ExternalImageTaskController::status() const {
    if (probing_)
        return tr("Checking Shape…");
    if (waiting_)
        return tr(
            "Create and accept an image in Shape, then choose Return to Deck. Your original card "
            "stays unchanged."
        );
    if (ready_)
        return targetCurrent() ? tr("Image returned. Review it, then choose Use this image.")
                               : tr("The image belongs to the original card. Reopen that card to "
                                    "review it; changed cards cannot use this return.");
    return {};
}
bool ExternalImageTaskController::targetCurrent() const {
    if (request_.isEmpty() || project_path_ != backend_.bundlePath())
        return false;
    const auto target = request_["target"].toObject();
    const auto draft = QJsonObject::fromVariantMap(cards_.draft());
    if (target["project_id"].toString() != backend_.projectId()
        || target["kit_id"] != draft["artifact_id"])
        return false;
    for (const auto& value : draft["composition"].toObject()["cards"].toArray()) {
        const auto card = value.toObject();
        if (card["id"] == target["card_id"]) {
            const auto artwork =
                card["artwork"].isUndefined() ? QJsonValue(QJsonValue::Null) : card["artwork"];
            return artwork == target["artwork"]
                   && (target.contains("card_snapshot")
                           ? card == target["card_snapshot"].toObject()
                           : target["draft_digest"] == draft["draft_digest"]);
        }
    }
    return false;
}
bool ExternalImageTaskController::begin(
    const QString& prompt,
    int width,
    int height,
    const QString& cardId
) {
    try {
        if (probe_.isRunning()) {
            error_ = QString::fromUtf8(
                QT_TR_NOOP("The previous Shape check is finishing. Try again in a moment.")
            );
            emit changed();
            return false;
        }
        if (running() || ready_) {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "Finish or stop waiting for the current Shape task before starting another."
            ));
            emit changed();
            return false;
        }
        const auto draft = QJsonObject::fromVariantMap(cards_.draft());
        if (prompt.trimmed().isEmpty() || prompt.toUtf8().size() > 16384 || width < 1 || height < 1
            || width > 4096 || height > 4096 || draft["artifact_id"].toString().isEmpty())
            throw std::runtime_error("invalid image task");
        QJsonValue artwork(QJsonValue::Null);
        QJsonObject snapshot;
        bool found = false;
        for (const auto& value : draft["composition"].toObject()["cards"].toArray()) {
            const auto card = value.toObject();
            if (card["id"].toString() == cardId) {
                found = true;
                snapshot = card;
                if (card.contains("artwork"))
                    artwork = card["artwork"];
            }
        }
        if (!found)
            throw std::runtime_error("card no longer exists");
        executable_ = documents_.shapeExecutable();
        if (!QFileInfo(executable_).isExecutable()) {
            error_ = QString::fromUtf8(
                QT_TR_NOOP("Choose the Shape application in Settings before generating an image.")
            );
            emit changed();
            return false;
        }
        ++generation_;
        pid_ = 0;
        imported_ = {};
        ready_ = false;
        preview_.clear();
        error_.clear();
        project_path_ = backend_.bundlePath();
        const auto id = QUuid::createUuid().toString(QUuid::WithoutBraces);
        const auto directory =
            QDir(QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation))
                .filePath("image-tasks/" + id);
        if (!QDir().mkpath(directory))
            throw std::runtime_error("task folder unavailable");
        request_path_ = QDir(directory).filePath("request.json");
        result_path_ = QDir(directory).filePath("result.json");
        request_ = {
            {"schema", "deck.material-task/1"},
            {"request_id", id},
            {"prompt", prompt},
            {"canvas", QJsonObject{{"width", width}, {"height", height}}},
            {"target",
             QJsonObject{
                 {"project_id", backend_.projectId()},
                 {"kit_id", draft["artifact_id"]},
                 {"card_id", cardId},
                 {"draft_digest", draft["draft_digest"]},
                 {"artwork", artwork},
                 {"card_snapshot", snapshot}
             }}
        };
        DeckCliClient::publishJson(request_path_, request_);
        request_digest_ = documents_.cli().hashFile(request_path_)["blake3"].toString();
        probe_generation_ = generation_;
        probing_ = true;
        waiting_ = false;
        remember();
        emit changed();
        const auto executable = executable_;
        probe_.setFuture(QtConcurrent::run([executable] {
            QProcess process;
            process.start(executable, {"--handoff-capabilities"});
            if (!process.waitForStarted(1000) || !process.waitForFinished(3000)) {
                process.kill();
                process.waitForFinished(1000);
                return QString("probe failed");
            }
            const auto bytes = process.readAllStandardOutput();
            if (bytes.size() > 65536 || process.exitStatus() != QProcess::NormalExit
                || process.exitCode() != 0)
                return QString("unsupported app");
            const auto reply = QJsonDocument::fromJson(bytes).object();
            return reply["schema"] == "shape.handoff-capabilities/1"
                           && reply["deck_material_task"] == "deck.material-task/1"
                           && reply["deck_material_result"] == "deck.material-task-result/1"
                       ? QString{}
                       : QString("unsupported protocol");
        }));
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(
                QT_TR_NOOP("Could not prepare this image task. Save the card and try again.")
            ),
            error
        );
    }
}
void ExternalImageTaskController::launch() {
    const auto generation = generation_;
    auto* process = new QProcess;
    process->setStandardOutputFile(QProcess::nullDevice());
    process->setStandardErrorFile(QFileInfo(request_path_).dir().filePath("shape-stderr.log"));
    connect(
        process,
        qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
        process,
        &QObject::deleteLater
    );
    connect(process, &QProcess::errorOccurred, process, [process](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart)
            process->deleteLater();
    });
    connect(process, &QProcess::started, this, [this, process, generation] {
        if (generation == generation_) {
            pid_ = process->processId();
            remember();
        }
    });
    connect(
        process,
        qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
        this,
        [this, generation](int, QProcess::ExitStatus) {
            if (generation == generation_)
                ended(true);
        }
    );
    connect(
        process,
        &QProcess::errorOccurred,
        this,
        [this, generation](QProcess::ProcessError error) {
            if (generation == generation_ && error == QProcess::FailedToStart)
                ended(false);
        }
    );
    waiting_ = true;
    remember();
    poll_.start();
    emit changed();
    process->start(
        executable_,
        {"--deck-material-task", request_path_, "--result", result_path_},
        QIODevice::NotOpen
    );
}
void ExternalImageTaskController::ended(bool started) {
    if (!waiting_)
        return;
    if (QFileInfo::exists(result_path_)) {
        poll();
        return;
    }
    waiting_ = false;
    pid_ = 0;
    remember();
    error_ =
        started
            ? QString::fromUtf8(QT_TR_NOOP(
                  "Shape closed without returning an image. Your card is unchanged. Reopen the "
                  "task "
                  "with an updated Shape and try again."
              ))
            : tr(
                  "Shape could not start. Choose the correct application in Settings and try again."
              );
    emit changed();
}
void ExternalImageTaskController::poll() {
    if (request_.isEmpty() || ready_ || !QFileInfo::exists(result_path_))
        return;
    try {
        const auto result = DeckCliClient::readJson(result_path_, 65536);
        const QStringList
            allowed{"schema", "request_id", "request_blake3", "outcome", "output", "error"};
        for (auto i = result.begin(); i != result.end(); ++i)
            if (!allowed.contains(i.key()))
                throw std::runtime_error("unknown result field");
        if (result["schema"] != "deck.material-task-result/1"
            || result["request_id"] != request_["request_id"]
            || result["request_blake3"].toString() != request_digest_)
            throw std::runtime_error("image return belongs to another request");
        waiting_ = false;
        pid_ = 0;
        if (result["outcome"] != "completed") {
            if (result["outcome"] != "cancelled" && result["outcome"] != "failed")
                throw std::runtime_error("unknown material outcome");
            if (!result["output"].isNull())
                throw std::runtime_error("cancelled return has an image");
            error_ =
                result["outcome"] == "cancelled"
                    ? QString::fromUtf8(
                          QT_TR_NOOP("The Shape task was cancelled. Your card is unchanged.")
                      )
                    : QString::fromUtf8(
                          QT_TR_NOOP("Shape could not complete this task. Your card is unchanged.")
                      );
            poll_.stop();
            remember();
            emit changed();
            return;
        }
        const auto output = result["output"].toObject();
        const auto path = output["path"].toString();
        const auto hash = documents_.cli().hashFile(path);
        if (output["media_type"] != "image/png" || hash["blake3"] != output["blake3"]
            || hash["byte_length"] != output["byte_length"]
            || hash["byte_length"].toInteger() > 20LL * 1024 * 1024)
            throw std::runtime_error("returned PNG bytes do not match");
        QImageReader reader(path, "PNG");
        const auto size = reader.size();
        if (!size.isValid() || size.width() > 4096 || size.height() > 4096
            || qint64(size.width()) * size.height() > 16777216)
            throw std::runtime_error("returned PNG dimensions exceed the task bounds");
        ready_ = true;
        preview_ = QUrl::fromLocalFile(path).toString();
        error_.clear();
        poll_.stop();
        remember();
        emit changed();
    } catch (const std::exception& error) {
        poll_.stop();
        fail(
            QString::fromUtf8(QT_TR_NOOP(
                "The returned image could not be verified. Keep your original card and start a new "
                "Shape task."
            )),
            error
        );
    }
}
QVariantMap ExternalImageTaskController::importReturnedImage() {
    try {
        if (!ready_ || !targetCurrent()) {
            error_ = tr(
                "The card or its artwork changed. This image cannot be applied to the current card."
            );
            emit changed();
            return {};
        }
        if (!imported_.isEmpty())
            return imported_.toVariantMap();
        QTemporaryFile request;
        if (!request.open())
            throw std::runtime_error("import request unavailable");
        const QJsonObject object{
            {"task_path", request_path_},
            {"result_path", result_path_},
            {"expected_request_blake3", request_digest_},
            {"name", tr("Image from Shape")}
        };
        const auto bytes = QJsonDocument(object).toJson(QJsonDocument::Compact);
        if (request.write(bytes) != bytes.size() || !request.flush())
            throw std::runtime_error("import request failed");
        const auto result =
            documents_.cli().run({"import-material-return", project_path_, request.fileName()});
        imported_ = result["source"].toObject();
        if (imported_["artifact_id"].toString().isEmpty()
            || imported_["revision_id"].toString().isEmpty())
            throw std::runtime_error("imported image identity unavailable");
        remember();
        if (!backend_.openProject(QUrl::fromLocalFile(project_path_)))
            throw std::runtime_error("imported image refresh failed");
        emit changed();
        return imported_.toVariantMap();
    } catch (const std::exception& error) {
        error_ = QString::fromUtf8(QT_TR_NOOP(
            "This card changed or the returned image is invalid. The image was not bound to a "
            "card. Start a new task, or import a valid image explicitly."
        ));
        qWarning().noquote() << error.what();
        emit changed();
        return {};
    }
}
void ExternalImageTaskController::remember() {
    if (request_.isEmpty()) {
        settings_.remove("deck/material_task");
        return;
    }
    const QJsonObject record{
        {"request", request_path_},
        {"result", result_path_},
        {"digest", request_digest_},
        {"project", project_path_},
        {"executable", executable_},
        {"pid", pid_},
        {"waiting", waiting_},
        {"imported", imported_}
    };
    settings_.setValue("deck/material_task", QJsonDocument(record).toJson(QJsonDocument::Compact));
    settings_.sync();
}
void ExternalImageTaskController::restore() {
    try {
        const auto record =
            QJsonDocument::fromJson(settings_.value("deck/material_task").toByteArray()).object();
        if (record.isEmpty())
            return;
        request_path_ = record["request"].toString();
        result_path_ = record["result"].toString();
        request_digest_ = record["digest"].toString();
        project_path_ = record["project"].toString();
        executable_ = record["executable"].toString();
        imported_ = record["imported"].toObject();
        pid_ = record["pid"].toInteger();
        request_ = DeckCliClient::readJson(request_path_, 65536);
        if (request_["schema"] != "deck.material-task/1"
            || documents_.cli().hashFile(request_path_)["blake3"].toString() != request_digest_)
            throw std::runtime_error("saved request changed");
        waiting_ = record["waiting"].toBool() && externalProcessAlive(pid_, executable_);
        if (QFileInfo::exists(result_path_))
            poll();
        else if (waiting_)
            poll_.start();
        else {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "The previous Shape task ended without an image. Your card is unchanged; start "
                "a new task to continue."
            ));
            remember();
        }
    } catch (const std::exception& error) {
        request_ = {};
        settings_.remove("deck/material_task");
        error_ = QString::fromUtf8(
            QT_TR_NOOP("The previous image task could not be restored. Your card is unchanged.")
        );
        qWarning().noquote() << error.what();
    }
}
void ExternalImageTaskController::stopWaiting() {
    ++generation_;
    poll_.stop();
    probing_ = false;
    waiting_ = false;
    ready_ = false;
    preview_.clear();
    request_ = {};
    imported_ = {};
    error_.clear();
    remember();
    emit changed();
}
void ExternalImageTaskController::consumed() {
    stopWaiting();
}
