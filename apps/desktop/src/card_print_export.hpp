//! Physical card-sheet rendering and atomic file publication, independent of authoring state.
#pragma once
#include <QHash>
#include <QImage>
#include <QJsonObject>
#include <QLineF>
#include <QRectF>
#include <QString>
#include <QVector>
#include <memory>

namespace card_print {
struct Slot {
    QString card_id;
    int copy_index = 0;
    QRectF rect_mm, content_rect_mm;
};
struct Page {
    QVector<Slot> placements;
    QVector<QLineF> crop_marks;
};
struct Document {
    QString name;
    QJsonObject summary;
    QSizeF sheet_mm;
    double stroke_mm = 0;
    bool dashed_cut_grid = false;
    // Unique card resources are shared by every placement; no payload is expanded per copy.
    QHash<QString, QByteArray> cards;
    QVector<Page> pages;
};
std::shared_ptr<const Document> prepare(const QJsonObject& plan);
QImage preview(const Document&, int pageIndex);
void exportPdf(const Document&, const QString& destination, const QString& projectPath);
} // namespace card_print
