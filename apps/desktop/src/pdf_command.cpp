#include "pdf_command.hpp"
#include "card_print_export.hpp"
#include "deck_cli_client.hpp"
#include <QDebug>
#include <QFileInfo>
#include <QJsonDocument>
#include <QTemporaryFile>
#include <stdexcept>
int runPdfCommand(const DeckCliClient& cli, const QString& requestPath, const QString& resultPath) {
    try {
        DeckCliClient::checkNewOutput(resultPath);
        const auto request = DeckCliClient::readJson(requestPath);
        const QStringList fields{"schema", "project", "source", "options", "destination"};
        for (auto i = request.begin(); i != request.end(); ++i)
            if (!fields.contains(i.key()))
                throw std::runtime_error("unknown PDF request field");
        if (request["schema"] != "deck.pdf-export/1" || !request["source"].isObject()
            || (!request["options"].isUndefined() && !request["options"].isObject()))
            throw std::runtime_error("invalid PDF request schema");
        const auto project = QFileInfo(request["project"].toString()).canonicalFilePath();
        const auto destination = request["destination"].toString();
        if (project.isEmpty() || !QFileInfo(project).isDir())
            throw std::runtime_error("project unavailable");
        DeckCliClient::checkNewOutput(destination);
        QTemporaryFile planRequest;
        if (!planRequest.open())
            throw std::runtime_error("temporary request unavailable");
        const QJsonObject action{
            {"action", "print_plan"},
            {"source", request["source"]},
            {"options", request["options"].toObject()}
        };
        const auto bytes = QJsonDocument(action).toJson(QJsonDocument::Compact);
        if (planRequest.write(bytes) != bytes.size() || !planRequest.flush())
            throw std::runtime_error("temporary request write failed");
        const auto result = cli.run({"card-kit", project, planRequest.fileName()});
        const auto plan = result["print_plan"].toObject();
        if (plan["source"] != request["source"])
            throw std::runtime_error("print source mismatch");
        const auto document = card_print::prepare(plan);
        card_print::exportPdf(*document, destination, project);
        auto output = cli.hashFile(destination);
        output["kind"] = "pdf";
        output["media_type"] = "application/pdf";
        DeckCliClient::publishJson(
            resultPath,
            {{"schema", "deck.pdf-export-result/1"},
             {"status", "completed"},
             {"project", project},
             {"source", request["source"]},
             {"output", output}}
        );
        return 0;
    } catch (const std::exception& error) {
        qCritical().noquote() << "Deck PDF export:" << error.what();
        return 1;
    }
}
