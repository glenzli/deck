#include "card_print_controller.hpp"
#include "deck_cli_client.hpp"
#include "desktop_backend.hpp"
#include <QBuffer>
#include <QDebug>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QTemporaryFile>
#include <QtConcurrent/QtConcurrentRun>
#include <stdexcept>

namespace {
QString outputFolder(const QString& path, const QString& project) {
    const QFileInfo info(path);
    if (!info.isAbsolute() || !info.isDir() || !info.isReadable() || !info.isWritable())
        return {};
    const auto canonical = info.canonicalFilePath();
    if (canonical.isEmpty())
        return {};
    const auto projectPath = QFileInfo(project).canonicalFilePath();
    QDir directory(canonical);
    do {
        if ((!projectPath.isEmpty() && directory.absolutePath() == projectPath)
            || directory.dirName().endsWith(".deck", Qt::CaseInsensitive)
            || directory.dirName().endsWith(".shape", Qt::CaseInsensitive))
            return {};
    } while (directory.cdUp());
    return canonical;
}
QString pdfBaseName(QString name) {
    static const QRegularExpression invalid(QStringLiteral(R"([/\\:*?"<>|\p{Cc}\p{Cf}])"));
    name.replace(invalid, QStringLiteral("_"));
    name = name.simplified().left(64);
    if (!name.isEmpty() && name.at(name.size() - 1).isHighSurrogate())
        name.chop(1);
    while (!name.isEmpty() && (name.endsWith('.') || name.endsWith(' ')))
        name.chop(1);
    while (name.startsWith('.') || name.startsWith(' '))
        name.remove(0, 1);
    if (name.isEmpty())
        return QStringLiteral("Cards");
    static const QRegularExpression reserved(
        QStringLiteral("^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\\..*)?$"),
        QRegularExpression::CaseInsensitiveOption
    );
    if (reserved.match(name).hasMatch())
        name.prepend('_');
    return name;
}
} // namespace

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
        const auto result = watcher_.result();
        const auto destination = pending_export_pdf_;
        const bool current = pending_export_version_ == version_ && !destination.isEmpty()
                             && project_id_ == backend_.projectId()
                             && project_path_ == backend_.bundlePath() && document_;
        pending_export_pdf_.clear();
        pending_export_version_ = -1;
        running_ = false;
        if (current) {
            if (result.isEmpty()) {
                exported_pages_ = document_->pages.size();
                exported_pdf_ = destination;
                error_.clear();
            } else {
                qWarning().noquote() << "card print:" << result;
                error_ = QString::fromUtf8(QT_TR_NOOP(
                    "Could not export PDF. Choose a new file outside the project and try again."
                ));
                exported_pages_ = 0;
                exported_pdf_.clear();
            }
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
    exported_pdf_.clear();
    pending_export_pdf_.clear();
    pending_export_version_ = -1;
    emit documentChanged();
    emit previewChanged();
    emit statusChanged();
}
bool CardPrintController::fail(const char* message, const QString& detail) {
    qWarning().noquote() << "card print:" << detail;
    error_ = QString::fromUtf8(message);
    exported_pages_ = 0;
    exported_pdf_.clear();
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
        if (detail.contains("card_print.cut_clearance"))
            return fail(
                QT_TR_NOOP("The card gap must be at least as wide as the cut line."),
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
QUrl CardPrintController::suggestedPdfDestination(const QUrl& folder) const {
    auto directory = folder.isLocalFile()
                         ? outputFolder(folder.toLocalFile(), backend_.bundlePath())
                         : QString{};
    if (directory.isEmpty())
        directory = outputFolder(
            QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation),
            backend_.bundlePath()
        );
    if (directory.isEmpty())
        return {};
    const auto name = pdfBaseName(document_ ? document_->name : QString{});
    auto path = QDir(directory).filePath(name + ".pdf");
    quint64 copy = 2;
    while (QFileInfo::exists(path) || QFileInfo(path).isSymLink())
        path = QDir(directory).filePath(name + " (" + QString::number(copy++) + ").pdf");
    return QUrl::fromLocalFile(path);
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
    const QFileInfo requested(destination.toLocalFile());
    if (!requested.isAbsolute())
        return fail(
            QT_TR_NOOP(
                "Could not export PDF. Choose a new file outside the project and try again."
            ),
            "absolute PDF destination required"
        );
    const auto directory = requested.dir().canonicalPath();
    const auto path = directory.isEmpty() ? requested.absoluteFilePath()
                                          : QDir(directory).filePath(requested.fileName());
    running_ = true;
    error_.clear();
    exported_pages_ = 0;
    exported_pdf_.clear();
    pending_export_pdf_ = QUrl::fromLocalFile(path);
    pending_export_version_ = version_;
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
bool CardPrintController::openExportedPdf() {
    const QFileInfo file(exported_pdf_.toLocalFile());
    if (running_ || !document_ || !exported_pdf_.isLocalFile()
        || project_id_ != backend_.projectId() || project_path_ != backend_.bundlePath()
        || !file.isAbsolute() || !file.isFile() || file.isSymLink() || !file.isReadable())
        return fail(
            QT_TR_NOOP("The exported PDF is no longer available. Export it again before opening."),
            "exported PDF is unavailable"
        );
    if (!QDesktopServices::openUrl(exported_pdf_))
        return fail(
            QT_TR_NOOP("Could not open the exported PDF. Open it from the destination folder."),
            "PDF viewer could not open the exported file"
        );
    return true;
}
