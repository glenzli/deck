#include "card_print_controller.hpp"
#include "deck_cli_client.hpp"
#include "desktop_backend.hpp"
#include <QBuffer>
#include <QDebug>
#include <QJsonDocument>
#include <QRegularExpression>
#include <QTemporaryFile>
#include <QtConcurrent/QtConcurrentRun>
#include <stdexcept>

CardPrintController::CardPrintController(
    DesktopBackend& backend,
    const DeckCliClient& cli,
    QObject* parent
)
    : QObject(parent), backend_(backend), cli_(cli) {
    connect(&backend_, &DesktopBackend::projectChanged, this, [this] {
        if (project_id_ != backend_.projectId() || project_path_ != backend_.bundlePath())
            clear();
    });
    connect(&watcher_, &QFutureWatcher<QString>::finished, this, [this] {
        running_ = false;
        const auto result = watcher_.result();
        if (project_id_ == backend_.projectId() && project_path_ == backend_.bundlePath()
            && document_) {
            if (result.isEmpty())
                exported_pages_ = document_->pages.size();
            else
                fail(
                    QT_TR_NOOP(
                        "Could not export PDF. Choose a new file outside the project and try again."
                    ),
                    result
                );
            emit finished(result.isEmpty());
        }
        emit statusChanged();
    });
}
CardPrintController::~CardPrintController() {
    watcher_.waitForFinished();
}
QString CardPrintController::status() const {
    if (running_)
        return tr("Exporting print PDF…");
    return exported_pages_
               ? tr("Exported %1 pages. Print at 100% or Actual size.").arg(exported_pages_)
               : QString{};
}
void CardPrintController::clear() {
    ++version_;
    document_.reset();
    preview_.clear();
    error_.clear();
    problem_card_id_.clear();
    page_index_ = 0;
    exported_pages_ = 0;
    emit documentChanged();
    emit previewChanged();
    emit statusChanged();
}
bool CardPrintController::fail(const char* message, const QString& detail) {
    qWarning().noquote() << "card print:" << detail;
    error_ = QString::fromUtf8(message);
    exported_pages_ = 0;
    emit statusChanged();
    return false;
}
bool CardPrintController::prepare(
    const QVariantMap& source,
    const QVariantMap& options,
    const QVariantMap& imposition
) {
    if (running_)
        return false;
    clear();
    project_id_ = backend_.projectId();
    project_path_ = backend_.bundlePath();
    try {
        if (project_id_.isEmpty() || source.value("artifact_id").toString().isEmpty()
            || source.value("revision_id").toString().isEmpty())
            throw std::runtime_error("accepted kit required");
        QJsonObject request{
            {"action", "print_plan"},
            {"source", QJsonObject::fromVariantMap(source)},
            {"options", QJsonObject::fromVariantMap(options)}
        };
        if (!imposition.isEmpty())
            request["imposition"] = QJsonObject::fromVariantMap(imposition);
        QTemporaryFile input;
        if (!input.open())
            throw std::runtime_error("print request unavailable");
        const auto bytes = QJsonDocument(request).toJson(QJsonDocument::Compact);
        if (input.write(bytes) != bytes.size() || !input.flush())
            throw std::runtime_error("print request write failed");
        const auto plan =
            cli_.run({"card-kit", project_path_, input.fileName()})["print_plan"].toObject();
        if (plan["source"].toObject() != QJsonObject::fromVariantMap(source))
            throw std::runtime_error("print source mismatch");
        document_ = card_print::prepare(plan);
        if (!showPage(0)) {
            document_.reset();
            emit documentChanged();
            return false;
        }
        emit documentChanged();
        return true;
    } catch (const std::exception& e) {
        document_.reset();
        emit documentChanged();
        const QString detail = QString::fromUtf8(e.what());
        const auto problem =
            QRegularExpression("card_print.text_overflow: card ([^ ]+)").match(detail);
        if (problem.hasMatch())
            problem_card_id_ = problem.captured(1);
        if (detail.contains("deck.print.grid") || detail.contains("deck.print.card"))
            return fail(
                QT_TR_NOOP("Choose valid rows, columns and a card from this accepted version."),
                detail
            );
        if (detail.contains("card_render.cover_limit:"))
            return fail(
                QT_TR_NOOP(
                    "An image is too narrow or wide for edge cropping. Go back to cards and "
                    "choose Show the complete image, or use artwork with more suitable proportions."
                ),
                detail
            );
        if (detail.contains("card_print.crop_clearance"))
            return fail(
                QT_TR_NOOP(
                    "The crop marks need more room. Increase the card spacing or shorten the "
                    "marks, and keep the mark offset at least half the line width."
                ),
                detail
            );
        if (detail.contains("card_print.paper_fit"))
            return fail(
                QT_TR_NOOP(
                    "The cards and crop marks do not fit on this paper. Use smaller cards or "
                    "margins, or shorter crop marks."
                ),
                detail
            );
        if (detail.contains("card_print.artwork_bytes")
            || detail.contains("card_print.artwork_pixels"))
            return fail(
                QT_TR_NOOP(
                    "This artwork is too large for a print job. Use smaller accepted images or "
                    "fewer card designs."
                ),
                detail
            );
        if (detail.contains("card_print.quantity_limit")
            || detail.contains("card_print.page_limit"))
            return fail(
                QT_TR_NOOP(
                    "This print job has too many copies or pages. Reduce the quantities or split "
                    "the cards into smaller sets."
                ),
                detail
            );
        if (detail.contains("card_print.slot_limit"))
            return fail(
                QT_TR_NOOP(
                    "Too many cards fit on one sheet. Use a larger card size or more spacing."
                ),
                detail
            );
        if (detail.contains("card_print.incomplete"))
            return fail(
                QT_TR_NOOP(
                    "Some cards still need content for their chosen layout. Check the unfinished "
                    "cards and accept the version before printing."
                ),
                detail
            );
        if (detail.contains("card_print.text_overflow"))
            return fail(
                QT_TR_NOOP(
                    "The text does not fit at a readable print size. Shorten it or use larger "
                    "cards, then check the layout again."
                ),
                detail
            );
        if (detail.contains("card_print.option_range"))
            return fail(
                QT_TR_NOOP(
                    "One of the print measurements is outside its allowed range. Check the paper "
                    "and crop-mark settings."
                ),
                detail
            );
        return fail(
            QT_TR_NOOP(
                "Could not prepare this print layout. Reopen the accepted card version and check "
                "its saved artwork, then try again."
            ),
            detail
        );
    }
}
bool CardPrintController::showPage(int index) {
    if (!document_ || index < 0 || index >= document_->pages.size())
        return false;
    try {
        const auto image = card_print::preview(*document_, index);
        QByteArray bytes;
        QBuffer buffer(&bytes);
        buffer.open(QIODevice::WriteOnly);
        if (!image.save(&buffer, "PNG"))
            throw std::runtime_error("preview encoding failed");
        preview_ = "data:image/png;base64," + QString::fromLatin1(bytes.toBase64());
        page_index_ = index;
        emit previewChanged();
        return true;
    } catch (const std::exception& e) {
        preview_.clear();
        emit previewChanged();
        return fail(
            QT_TR_NOOP("Could not show this print page. Reopen the print layout and try again."),
            QString::fromUtf8(e.what())
        );
    }
}
bool CardPrintController::exportPdf(const QUrl& destination, int expectedVersion) {
    if (running_)
        return false;
    if (!document_ || expectedVersion != version_ || !destination.isLocalFile()
        || project_id_ != backend_.projectId() || project_path_ != backend_.bundlePath())
        return fail(
            QT_TR_NOOP("The print layout changed. Choose the PDF destination again."),
            "stale print export"
        );
    const auto document = document_;
    const auto project = project_path_;
    const auto path = destination.toLocalFile();
    running_ = true;
    error_.clear();
    exported_pages_ = 0;
    emit statusChanged();
    watcher_.setFuture(QtConcurrent::run([document, path, project] {
        try {
            card_print::exportPdf(*document, path, project);
            return QString{};
        } catch (const std::exception& e) {
            return QString::fromUtf8(e.what());
        }
    }));
    return true;
}
