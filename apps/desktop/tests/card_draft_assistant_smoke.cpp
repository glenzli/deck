#include "card_draft_assistant_smoke.hpp"
#include "card_draft_assistant.hpp"
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QThread>
#include <QUrl>
#include <functional>

namespace {
bool waitFor(const std::function<bool()>& predicate) {
    QElapsedTimer timer;
    timer.start();
    while (!predicate() && timer.elapsed() < 5000) {
        QCoreApplication::processEvents();
        QThread::msleep(5);
    }
    return predicate();
}
} // namespace
bool card_draft_assistant_smoke::verify(const QString& directory) {
    QDir().mkpath(directory);
    const auto fakePath = QDir(directory).filePath("assistant-cli");
    QFile fake(fakePath);
    if (!fake.open(QIODevice::WriteOnly))
        return false;
    fake.write(R"SH(#!/bin/sh
case "$(cat "$2")" in
  *'"operation":"probe"'*) echo '{"schema":"deck.cli.result/1","status":"ok","result":{"connected":true,"authorized":true}}' ;;
  *'"operation":"start"'*) sleep 0.1; echo '{"schema":"deck.assistant.progress/1","response_id":"job_assistant_test"}'; sleep 0.15; echo '{"schema":"deck.cli.result/1","status":"ok","result":{"response_id":"job_assistant_test","status":"completed","cards":[{"title":"Moon","rules":"A natural satellite","art_prompt":"Moon"}]}}' ;;
  *'"operation":"cancel"'*) echo '{"schema":"deck.cli.result/1","status":"ok","result":{"response_id":"job_assistant_test","cancelled":true}}' ;;
esac
)SH");
    fake.close();
    fake.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    const auto tokenPath = QDir(directory).filePath("fixture.token");
    QFile token(tokenPath);
    if (!token.open(QIODevice::WriteOnly))
        return false;
    token.write("test-only-not-a-runtime-token");
    token.close();
    token.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    CardDraftAssistant assistant(fakePath, QDir(directory).filePath("assistant.conf"));
    if (assistant.generate("Moon", 1, "en", "project|draft-A") || assistant.error().isEmpty())
        return false;
    if (!assistant.setCredentialFile(QUrl::fromLocalFile(tokenPath)))
        return false;
    assistant.probe();
    if (!waitFor([&] { return !assistant.busy(); }) || !assistant.connected())
        return false;
    if (!assistant.generate("Moon", 1, "en", "project|draft-A"))
        return false;
    if (!waitFor([&] { return !assistant.busy(); }) || assistant.cards().size() != 1
        || assistant.targetIdentity() != "project|draft-A")
        return false;
    assistant.discard();
    if (!assistant.cards().isEmpty() || !assistant.targetIdentity().isEmpty())
        return false;
    if (!assistant.generate("Moon", 1, "en", "project|draft-B"))
        return false;
    assistant.discard();
    if (!waitFor([&] { return !assistant.busy(); }) || !assistant.cards().isEmpty()
        || !assistant.targetIdentity().isEmpty() || !assistant.error().isEmpty())
        return false;
    CardDraftAssistant reopened(fakePath, QDir(directory).filePath("assistant.conf"));
    return reopened.credentialFile() == tokenPath;
}
