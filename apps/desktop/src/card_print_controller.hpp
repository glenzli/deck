//! Transient exact-version print preparation, preview navigation and file-export lifecycle.
#pragma once
#include "card_print_export.hpp"
#include <QFutureWatcher>
#include <QObject>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
class DesktopBackend;
class CardPrintController : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Created by the Shape host")
    Q_PROPERTY(QVariantMap plan READ plan NOTIFY documentChanged)
    Q_PROPERTY(QString previewSource READ previewSource NOTIFY previewChanged)
    Q_PROPERTY(int pageIndex READ pageIndex NOTIFY previewChanged)
    Q_PROPERTY(int version READ version NOTIFY documentChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY documentChanged)
    Q_PROPERTY(bool running READ running NOTIFY statusChanged)
    Q_PROPERTY(QString error READ error NOTIFY statusChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)
  public:
    explicit CardPrintController(DesktopBackend&, QObject* parent = nullptr);
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
    Q_INVOKABLE bool prepare(const QVariantMap& source, const QVariantMap& options);
    Q_INVOKABLE bool showPage(int index);
    Q_INVOKABLE bool exportPdf(const QUrl& destination, int expectedVersion);
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
    std::shared_ptr<const card_print::Document> document_;
    QString project_id_, project_path_, preview_, error_;
    int page_index_ = 0, version_ = 0, exported_pages_ = 0;
    bool running_ = false;
    QFutureWatcher<QString> watcher_;
};
