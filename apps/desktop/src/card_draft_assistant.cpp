#include "card_draft_assistant.hpp"
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcessEnvironment>
#include <QUrl>
#include <algorithm>

CardDraftAssistant::CardDraftAssistant(QString cli, const QString& settings, QObject* parent)
    : QObject(parent), settings_(settings, QSettings::IniFormat), cli_(std::move(cli)) {
    credential_ = settings_.value("infer/credentialFile").toString();
    const auto local = QDir(QFileInfo(settings).absolutePath()).filePath("infer.token");
    if (credential_.isEmpty() && QFileInfo(local).isFile())
        credential_ = local;
    timeout_.setSingleShot(true);
    cancelTimeout_.setSingleShot(true);
    connect(&timeout_, &QTimer::timeout, this, [this] {
        if (running_ && !response_.isEmpty()) {
            cancelRequested_ = true;
            cancelRemote();
        } else {
            process_.kill();
            fail("infer_timeout");
        }
    });
    connect(&cancelTimeout_, &QTimer::timeout, this, [this] {
        cancel_.kill();
        finishCancellation(false);
    });
    connect(&process_, &QProcess::readyReadStandardOutput, this, &CardDraftAssistant::readOutput);
    connect(&process_, &QProcess::readyReadStandardError, this, [this] {
        process_.readAllStandardError();
    });
    connect(&process_, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart)
            fail("cli_unavailable");
    });
    connect(
        &process_,
        qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
        this,
        &CardDraftAssistant::finished
    );
    connect(&cancel_, &QProcess::readyReadStandardOutput, this, [this] {
        cancelOutput_ += cancel_.readAllStandardOutput();
        if (cancelOutput_.size() > 65536) {
            cancel_.kill();
            finishCancellation(false);
        }
    });
    connect(&cancel_, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart)
            finishCancellation(false);
    });
    connect(
        &cancel_,
        qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
        this,
        [this](int code, QProcess::ExitStatus status) {
            cancelOutput_ += cancel_.readAllStandardOutput();
            const auto result = QJsonDocument::fromJson(cancelOutput_).object();
            finishCancellation(
                status == QProcess::NormalExit && code == 0
                && result["schema"] == "deck.cli.result/1" && result["status"] == "ok"
                && result["result"].toObject()["cancelled"].toBool()
            );
        }
    );
}
QString CardDraftAssistant::error() const {
    if (errorCode_.isEmpty())
        return {};
    if (errorCode_ == "credential_unavailable" || errorCode_ == "infer_access_denied")
        return tr("Connect Deck's own Infer Runtime access in Settings, then try again.");
    if (errorCode_ == "invalid_cards")
        return tr("The model returned an invalid card list. Try fewer cards or a clearer brief.");
    if (errorCode_ == "generation_cancelled")
        return tr("Generation cancelled.");
    if (errorCode_ == "cancel_unconfirmed")
        return tr("Stopped waiting. The model may still be finishing this request.");
    if (errorCode_ == "invalid_prompt")
        return tr("Enter a short description and choose between 1 and 16 cards.");
    return tr("Infer Runtime could not complete this request. Check the connection and try again.");
}
QString CardDraftAssistant::status() const {
    if (running_)
        return cancelRequested_ ? tr("Cancelling…") : tr("Generating cards locally…");
    if (busy())
        return tr("Checking connection…");
    if (connected_)
        return tr("Infer Runtime connected");
    return tr("Local text generation via Infer Runtime");
}
bool CardDraftAssistant::setCredentialFile(const QUrl& path) {
    if (busy() || !path.isLocalFile() || !QFileInfo(path.toLocalFile()).isFile())
        return false;
    credential_ = QFileInfo(path.toLocalFile()).absoluteFilePath();
    settings_.setValue("infer/credentialFile", credential_);
    connected_ = false;
    errorCode_.clear();
    emit changed();
    return true;
}
void CardDraftAssistant::probe() {
    if (busy())
        return;
    errorCode_.clear();
    cancelRequested_ = false;
    dispatch("probe");
}
bool CardDraftAssistant::generate(
    const QString& prompt,
    int count,
    const QString& language,
    const QString& identity
) {
    if (busy())
        return false;
    cards_.clear();
    response_.clear();
    errorCode_.clear();
    connected_ = false;
    if (credential_.isEmpty()) {
        fail("credential_unavailable");
        return false;
    }
    if (prompt.trimmed().isEmpty() || prompt.toUtf8().size() > 8192 || count < 1 || count > 16
        || identity.isEmpty()) {
        fail("invalid_prompt");
        return false;
    }
    prompt_ = prompt;
    count_ = count;
    language_ = language == "zh_CN" ? language : "en";
    identity_ = identity;
    running_ = true;
    cancelRequested_ = false;
    dispatch("start");
    return true;
}
namespace {
std::unique_ptr<QTemporaryFile> requestFile(const QJsonObject& object) {
    auto file =
        std::make_unique<QTemporaryFile>(QDir::temp().filePath("deck-assistant-XXXXXX.json"));
    if (!file->open())
        return {};
    const auto bytes = QJsonDocument(object).toJson(QJsonDocument::Compact);
    if (file->write(bytes) != bytes.size() || !file->flush())
        return {};
    return file;
}
QProcessEnvironment environment(const QString& credential) {
    auto result = QProcessEnvironment::systemEnvironment();
    result.remove("DECK_INFER_CREDENTIAL_FILE");
    if (!credential.isEmpty())
        result.insert("DECK_INFER_CREDENTIAL_FILE", credential);
    return result;
}
} // namespace
void CardDraftAssistant::dispatch(const QString& operation) {
    operation_ = operation;
    output_.clear();
    outputBytes_ = 0;
    terminal_ = {};
    request_ = requestFile(
        {{"schema", "deck.assistant/1"},
         {"operation", operation},
         {"prompt", prompt_},
         {"count", count_},
         {"language", language_.isEmpty() ? "en" : language_}}
    );
    if (!request_) {
        fail("cli_unavailable");
        return;
    }
    process_.setProcessEnvironment(environment(credential_));
    process_.setProgram(cli_);
    process_.setArguments({"assist", request_->fileName()});
    process_.start();
    timeout_.start(operation == "start" ? 190000 : 20000);
    emit changed();
}
void CardDraftAssistant::readOutput() {
    const auto bytes = process_.readAllStandardOutput();
    output_ += bytes;
    outputBytes_ += bytes.size();
    if (outputBytes_ > 1024 * 1024) {
        process_.kill();
        fail("invalid_cards");
        return;
    }
    for (;;) {
        const auto end = output_.indexOf('\n');
        if (end < 0)
            break;
        const auto line = output_.left(end).trimmed();
        output_.remove(0, end + 1);
        if (line.isEmpty())
            continue;
        QJsonParseError parse;
        const auto item = QJsonDocument::fromJson(line, &parse).object();
        if (parse.error != QJsonParseError::NoError) {
            process_.kill();
            fail("invalid_cards");
            return;
        }
        if (item["schema"] == "deck.assistant.progress/1" && operation_ == "start") {
            const auto id = item["response_id"].toString();
            const bool valid =
                !id.isEmpty() && id.size() <= 128 && std::all_of(id.begin(), id.end(), [](QChar c) {
                    return (c.isLetterOrNumber() && c.unicode() < 128) || c == '_' || c == '-';
                });
            if (!valid || (!response_.isEmpty() && response_ != id)) {
                process_.kill();
                fail("invalid_cards");
                return;
            }
            response_ = id;
            if (cancelRequested_)
                cancelRemote();
        } else if (item["schema"] == "deck.cli.result/1" && terminal_.isEmpty())
            terminal_ = item;
        else {
            process_.kill();
            fail("invalid_cards");
            return;
        }
    }
}
void CardDraftAssistant::finished(int code, QProcess::ExitStatus status) {
    timeout_.stop();
    readOutput();
    if (cancelRequested_ && !running_) {
        emit changed();
        return;
    }
    if (!errorCode_.isEmpty()) {
        emit changed();
        return;
    }
    if (cancelRequested_) {
        if (!response_.isEmpty())
            cancelRemote();
        else
            finishCancellation(false);
        return;
    }
    if (terminal_.isEmpty() || status != QProcess::NormalExit || code != 0
        || terminal_["status"] != "ok") {
        fail(terminal_["code"].toString());
        return;
    }
    const auto result = terminal_["result"].toObject();
    if (operation_ == "probe") {
        connected_ = result["connected"].toBool() && result["authorized"].toBool();
        emit changed();
        return;
    }
    if (result["status"] != "completed" || result["response_id"].toString() != response_) {
        fail("invalid_cards");
        return;
    }
    running_ = false;
    connected_ = true;
    cards_ = result["cards"].toArray().toVariantList();
    emit changed();
}
void CardDraftAssistant::cancelRemote() {
    if (cancel_.state() != QProcess::NotRunning || !running_ || response_.isEmpty())
        return;
    cancelOutput_.clear();
    cancelRequest_ = requestFile(
        {{"schema", "deck.assistant/1"}, {"operation", "cancel"}, {"response_id", response_}}
    );
    if (!cancelRequest_) {
        finishCancellation(false);
        return;
    }
    cancel_.setProcessEnvironment(environment(credential_));
    cancel_.setProgram(cli_);
    cancel_.setArguments({"assist", cancelRequest_->fileName()});
    cancel_.start();
    cancelTimeout_.start(20000);
    emit changed();
}
void CardDraftAssistant::finishCancellation(bool confirmed) {
    timeout_.stop();
    cancelTimeout_.stop();
    running_ = false;
    cards_.clear();
    response_.clear();
    identity_.clear();
    errorCode_ = confirmed ? QString{} : QStringLiteral("cancel_unconfirmed");
    if (process_.state() != QProcess::NotRunning)
        process_.kill();
    emit changed();
}
void CardDraftAssistant::discard() {
    cards_.clear();
    identity_.clear();
    if (running_) {
        cancelRequested_ = true;
        if (!response_.isEmpty())
            cancelRemote();
    } else if (!busy()) {
        response_.clear();
        errorCode_.clear();
    }
    emit changed();
}
void CardDraftAssistant::fail(const QString& code) {
    timeout_.stop();
    running_ = false;
    cards_.clear();
    connected_ = false;
    errorCode_ = code.isEmpty() ? "infer_unavailable" : code;
    emit changed();
}
