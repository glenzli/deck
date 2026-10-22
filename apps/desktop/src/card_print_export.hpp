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
    QString resource_id;
    QString face = QStringLiteral("front");
};
struct Page {
    QVector<Slot> placements;
    QVector<QLineF> crop_marks;
    QString side = QStringLiteral("front");
    int sheet_index = 0;
};
struct Document {
    QString name;
    QJsonObject summary;
    QSizeF sheet_mm;
    double stroke_mm = 0;
    bool dashed_cut_grid = false;
    // Front resources and one shared back are never expanded per physical copy.
    QHash<QString, QByteArray> cards;
    QVector<Page> pages;
};
std::shared_ptr<const Document> prepare(const QJsonObject& plan);
QImage preview(const Document&, int pageIndex);
void exportPdf(const Document&, const QString& destination, const QString& projectPath);
} // namespace card_print
