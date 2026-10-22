//! Transient exact-version print preparation, preview navigation and file-export lifecycle.
#pragma once
#include "card_print_export.hpp"
#include <QFutureWatcher>
#include <QObject>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
class DesktopBackend;
class DeckCliClient;
class CardPrintController : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by the Shape host")
    Q_PROPERTY(QVariantMap plan READ plan NOTIFY documentChanged)
    Q_PROPERTY(QString previewSource READ previewSource NOTIFY previewChanged)
    Q_PROPERTY(int pageIndex READ pageIndex NOTIFY previewChanged)
    Q_PROPERTY(QString pageSide READ pageSide NOTIFY previewChanged)
    Q_PROPERTY(int sheetIndex READ sheetIndex NOTIFY previewChanged)
    Q_PROPERTY(int version READ version NOTIFY documentChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY documentChanged)
    Q_PROPERTY(bool running READ running NOTIFY statusChanged)
    Q_PROPERTY(QString error READ error NOTIFY statusChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)
    Q_PROPERTY(QUrl exportedPdf READ exportedPdf NOTIFY statusChanged)
    Q_PROPERTY(QString problemCardId READ problemCardId NOTIFY statusChanged)
  public:
    explicit CardPrintController(DesktopBackend&, const DeckCliClient&, QObject* parent = nullptr);
    ~CardPrintController() override;
    QVariantMap plan() const {
        return document_ ? document_->summary.toVariantMap() : QVariantMap{};
    }
    QString previewSource() const {
        return preview_;
    }
    int pageIndex() const {
        return page_index_;
    }
    QString pageSide() const {
        return document_ && page_index_ < document_->pages.size()
                   ? document_->pages[page_index_].side
                   : QString{};
    }
    int sheetIndex() const {
        return document_ && page_index_ < document_->pages.size()
                   ? document_->pages[page_index_].sheet_index
                   : -1;
    }
    int version() const {
        return version_;
    }
    bool ready() const {
        return bool(document_);
    }
    bool running() const {
        return running_;
    }
    QString error() const {
        return tr(error_.toUtf8().constData());
    }
    QString status() const;
    QUrl exportedPdf() const {
        return exported_pdf_;
    }
    QString problemCardId() const {
        return problem_card_id_;
    }
    Q_INVOKABLE bool prepare(
        const QVariantMap& source,
        const QVariantMap& options,
        const QVariantMap& imposition = {}
    );
    Q_INVOKABLE bool showPage(int index);
    Q_INVOKABLE QUrl suggestedPdfDestination(const QUrl& folder) const;
    Q_INVOKABLE bool exportPdf(const QUrl& destination, int expectedVersion);
    Q_INVOKABLE bool openExportedPdf();
    void retranslate() {
        emit statusChanged();
    }
  signals:
    void documentChanged();
    void previewChanged();
    void statusChanged();
    void finished(bool success);

  private:
    void clear();
    bool fail(const char* message, const QString& detail);
    DesktopBackend& backend_;
    const DeckCliClient& cli_;
    std::shared_ptr<const card_print::Document> document_;
    QString project_id_, project_path_, preview_, error_, problem_card_id_;
    QUrl exported_pdf_, pending_export_pdf_;
    int page_index_ = 0, version_ = 0, exported_pages_ = 0;
    int pending_export_version_ = -1;
    bool running_ = false;
    QFutureWatcher<QString> watcher_;
};
