#include "deck_cli_client.hpp"
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QProcess>
#include <QTemporaryFile>
#include <filesystem>
#include <stdexcept>
namespace {
[[noreturn]] void fail(const QString& message) {
    throw std::runtime_error(message.toStdString());
}
} // namespace
DeckCliClient::DeckCliClient(QString executable) : executable_(std::move(executable)) {}
QJsonObject DeckCliClient::run(const QStringList& arguments, int timeoutMs) const {
    if (!QFileInfo(executable_).isExecutable())
        fail("The packaged Deck CLI is unavailable");
    QProcess child;
    child.setProgram(executable_);
    child.setArguments(arguments);
    child.start();
    if (!child.waitForStarted(5000))
        fail("The Deck CLI could not start");
    QElapsedTimer timer;
    timer.start();
    QByteArray output, errors;
    while (child.state() != QProcess::NotRunning) {
        child.waitForReadyRead(50);
        output += child.readAllStandardOutput();
        errors += child.readAllStandardError();
        if (output.size() > 64 * 1024 * 1024 || errors.size() > 1024 * 1024
            || timer.elapsed() > timeoutMs) {
            child.kill();
            child.waitForFinished(5000);
            fail("Deck command exceeded its resource limit");
        }
    }
    output += child.readAllStandardOutput();
    errors += child.readAllStandardError();
    QJsonParseError parse;
    const auto object = QJsonDocument::fromJson(output, &parse).object();
    if (parse.error != QJsonParseError::NoError || object["schema"] != "deck.cli.result/1")
        fail("Deck command returned an invalid response");
    if (child.exitStatus() != QProcess::NormalExit || child.exitCode() != 0
        || object["status"] != "ok") {
        const auto error = object.contains("message") ? object["message"] : object["error"];
        fail(
            error.isString()
                ? error.toString()
                : QString::fromUtf8(QJsonDocument(error.toObject()).toJson(QJsonDocument::Compact))
        );
    }
    return object["result"].toObject();
}
QJsonObject DeckCliClient::hashFile(const QString& path) const {
    return run({"hash-file", path});
}
QJsonObject DeckCliClient::readJson(const QString& path, qint64 maximumBytes) {
    QFileInfo info(path);
    if (!info.isAbsolute() || info.isSymLink() || !info.isFile() || info.size() <= 0
        || info.size() > maximumBytes)
        fail("The request is not a bounded regular file");
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        fail("The request could not be read");
    const auto bytes = file.read(maximumBytes + 1);
    QJsonParseError parse;
    const auto document = QJsonDocument::fromJson(bytes, &parse);
    if (bytes.size() > maximumBytes || parse.error != QJsonParseError::NoError
        || !document.isObject())
        fail("The request is not a valid JSON object");
    return document.object();
}
void DeckCliClient::checkNewOutput(const QString& path) {
    const QFileInfo output(path);
    if (!output.isAbsolute() || output.exists() || output.isSymLink())
        fail("Choose a new output file");
    QDir parent(output.absolutePath());
    if (!parent.exists())
        fail("The output folder is unavailable");
    QString current = QFileInfo(parent.absolutePath()).canonicalFilePath();
    if (current.isEmpty())
        fail("The output folder is unavailable");
    for (;;) {
        QFileInfo directory(current);
        if (directory.fileName().endsWith(".deck", Qt::CaseInsensitive)
            || directory.fileName().endsWith(".shape", Qt::CaseInsensitive))
            fail("Output files must be outside project bundles");
        const auto next = directory.dir().absolutePath();
        if (next == current)
            break;
        current = next;
    }
}
void DeckCliClient::publishJson(const QString& path, const QJsonObject& object) {
    checkNewOutput(path);
    QTemporaryFile temporary(QDir(QFileInfo(path).absolutePath()).filePath(".deck-result-XXXXXX"));
    if (!temporary.open())
        fail("The result folder is not writable");
    const auto bytes = QJsonDocument(object).toJson(QJsonDocument::Indented);
    if (temporary.write(bytes) != bytes.size() || !temporary.flush())
        fail("The result could not be written");
    temporary.close();
    std::error_code error;
    std::filesystem::create_hard_link(
        std::filesystem::path(temporary.fileName().toStdString()),
        std::filesystem::path(path.toStdString()),
        error
    );
    if (error)
        fail("The result could not be published as a new file");
}
