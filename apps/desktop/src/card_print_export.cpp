#include "card_print_export.hpp"
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QPageSize>
#include <QPainter>
#include <QPainterStateGuard>
#include <QPdfWriter>
#include <QSvgRenderer>
#include <QTemporaryFile>
#include <algorithm>
#include <cmath>
#include <filesystem>
#include <map>
#include <stdexcept>

namespace card_print {
namespace {
double number(const QJsonObject& object, const char* key) {
    const auto value = object[key];
    if (!value.isDouble() || !std::isfinite(value.toDouble()))
        throw std::runtime_error("invalid print plan number");
    return value.toDouble();
}
int integer(const QJsonObject& object, const char* key, int minimum, int maximum) {
    const auto value = number(object, key);
    if (value != std::floor(value) || value < minimum || value > maximum)
        throw std::runtime_error("invalid print plan count");
    return int(value);
}
QRectF rect(const QJsonObject& object) {
    const QRectF result(
        number(object, "x"),
        number(object, "y"),
        number(object, "width"),
        number(object, "height")
    );
    if (result.width() <= 0 || result.height() <= 0)
        throw std::runtime_error("invalid print rectangle");
    return result;
}
// A single resident SVG renderer bounds decoded bitmap residency. Consecutive copies reuse it.
class SheetPainter {
  public:
    explicit SheetPainter(const Document& document) : document_(document) {}
    void paint(QPainter& painter, int index) {
        if (index < 0 || index >= document_.pages.size())
            throw std::runtime_error("invalid print page");
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setRenderHint(QPainter::SmoothPixmapTransform);
        painter.fillRect(QRectF(QPointF(0, 0), document_.sheet_mm), Qt::white);
        const auto& page = document_.pages[index];
        for (const auto& slot : page.placements) {
            const auto& resource = slot.resource_id.isEmpty() ? slot.card_id : slot.resource_id;
            if (loaded_resource_ != resource) {
                renderer_ = std::make_unique<QSvgRenderer>(document_.cards.value(resource));
                if (!renderer_->isValid())
                    throw std::runtime_error("invalid card SVG");
                loaded_resource_ = resource;
            }
            const QPainterStateGuard state(&painter);
            painter.setClipRect(slot.rect_mm);
            painter.fillRect(slot.rect_mm, Qt::white);
            renderer_->render(&painter, slot.content_rect_mm);
        }
        QPen cut_lines(
            document_.dashed_cut_grid ? QColor(QStringLiteral("#b0b0b0")) : QColor(Qt::black),
            document_.stroke_mm,
            Qt::SolidLine,
            Qt::FlatCap
        );
        if (document_.dashed_cut_grid) {
            // Dash lengths use physical millimetres in both the preview and PDF painter.
            cut_lines.setDashPattern({1.2 / document_.stroke_mm, 0.8 / document_.stroke_mm});
        }
        painter.setPen(cut_lines);
        painter.drawLines(page.crop_marks);
    }

  private:
    const Document& document_;
    QString loaded_resource_;
    std::unique_ptr<QSvgRenderer> renderer_;
};
QString destinationPath(const QString& destination, const QString& project) {
    const QFileInfo target(destination);
    const auto parent_path = target.dir().canonicalPath();
    if (!target.isAbsolute() || parent_path.isEmpty()
        || target.suffix().compare("pdf", Qt::CaseInsensitive) || target.exists()
        || target.isSymLink())
        throw std::runtime_error("new local PDF required");
    QDir parent(parent_path);
    const auto project_path = QFileInfo(project).canonicalFilePath();
    do {
        if (parent.absolutePath() == project_path
            || parent.dirName().endsWith(".shape", Qt::CaseInsensitive)
            || parent.dirName().endsWith(".deck", Qt::CaseInsensitive))
            throw std::runtime_error("PDF destination inside project");
    } while (parent.cdUp());
    return QDir(parent_path).filePath(target.fileName());
}
} // namespace
std::shared_ptr<const Document> prepare(const QJsonObject& plan) {
    if (plan["schema"] != "shape.card-print-plan/1" || plan["units"] != "mm"
        || plan["line_cap"] != "butt")
        throw std::runtime_error("unknown print plan");
    const auto pages = plan["pages"].toArray();
    const auto imposition = plan["imposition"].toObject();
    const auto inferredSides = plan["duplex"].toBool() ? QStringLiteral("duplex")
                               : !pages.isEmpty() && pages.first().toObject()["side"] == "back"
                                   ? QStringLiteral("backs")
                                   : QStringLiteral("fronts");
    const auto sides =
        imposition["sides"].isUndefined() ? inferredSides : imposition["sides"].toString();
    if (sides != "fronts" && sides != "duplex" && sides != "backs")
        throw std::runtime_error("invalid print sides");
    const bool duplex = sides == "duplex";
    const bool hasBack = sides != "fronts";
    if (!plan["duplex"].isUndefined()
        && (!plan["duplex"].isBool() || plan["duplex"].toBool() != duplex))
        throw std::runtime_error("inconsistent duplex plan");
    const auto flip = imposition["flip_edge"].isUndefined() ? QStringLiteral("long")
                                                            : imposition["flip_edge"].toString();
    if (flip != "long" && flip != "short")
        throw std::runtime_error("invalid print flip edge");
    for (const auto* key : {"back_offset_x_mm", "back_offset_y_mm"}) {
        if (imposition[key].isUndefined())
            continue;
        const auto value = number(imposition, key);
        if (std::abs(value) > 10 || std::abs(value * 1000 - std::round(value * 1000)) > 0.000001)
            throw std::runtime_error("invalid back alignment");
    }
    const int expectedCopies = integer(plan, "total_copies", 1, 4096);
    const int expectedPages = integer(plan, "page_count", 1, duplex ? 512 : 256);
    const int expectedSheets = hasBack ? integer(plan, "sheet_count", 1, 256) : expectedPages;
    if (expectedPages != expectedSheets * (duplex ? 2 : 1) || pages.size() != expectedPages)
        throw std::runtime_error("inconsistent print sheet count");
    const int expectedPlacements =
        hasBack ? integer(plan, "total_placements", 1, 8192) : expectedCopies;
    if (expectedPlacements != expectedCopies * (duplex ? 2 : 1))
        throw std::runtime_error("inconsistent print placement count");
    auto result = std::make_shared<Document>();
    result->name = plan["name"].toString();
    result->sheet_mm = QSizeF(number(plan, "sheet_width_mm"), number(plan, "sheet_height_mm"));
    result->stroke_mm = number(plan["options"].toObject(), "stroke_width_mm");
    const auto cut_style = plan["options"].toObject()["cut_line_style"];
    if (!cut_style.isUndefined() && cut_style != "corner_marks" && cut_style != "dashed_grid")
        throw std::runtime_error("unknown cut-line style");
    result->dashed_cut_grid = cut_style == "dashed_grid";
    if (result->sheet_mm.width() < 100 || result->sheet_mm.width() > 1000
        || result->sheet_mm.height() < 100 || result->sheet_mm.height() > 1000
        || result->stroke_mm <= 0 || result->stroke_mm > 2)
        throw std::runtime_error("invalid print sheet");
    qint64 bytes = 0;
    for (const auto& value : plan["cards"].toArray()) {
        const auto card = value.toObject();
        const auto id = card["id"].toString();
        const auto svg = card["svg"].toString().toUtf8();
        bytes += svg.size();
        if (id.isEmpty() || svg.isEmpty() || result->cards.contains(id)
            || bytes > 128 * 1024 * 1024)
            throw std::runtime_error("invalid print resource");
        result->cards.insert(id, svg);
    }
    if (result->cards.isEmpty() || result->cards.size() > (hasBack ? 65 : 64)
        || (sides == "backs" && result->cards.size() != 1)
        || (hasBack && !result->cards.contains(QStringLiteral("@deck/back"))))
        throw std::runtime_error("invalid card count");
    const QRectF sheet(QPointF(0, 0), result->sheet_mm);
    const auto bounds = sheet.adjusted(-0.001, -0.001, 0.001, 0.001);
    int placements = 0;
    using Copy = std::pair<QString, int>;
    std::map<Copy, int> fronts, backs;
    for (const auto& value : pages) {
        Page page;
        const auto input = value.toObject();
        if (number(input, "index") != result->pages.size())
            throw std::runtime_error("invalid print page order");
        const int index = result->pages.size();
        const auto expectedSide = sides == "backs" || (duplex && index % 2 == 1)
                                      ? QStringLiteral("back")
                                      : QStringLiteral("front");
        page.side = input["side"].isUndefined() && !hasBack ? QStringLiteral("front")
                                                            : input["side"].toString();
        page.sheet_index = input["sheet_index"].isUndefined() && !hasBack
                               ? index
                               : integer(input, "sheet_index", 0, 255);
        if (page.side != expectedSide || page.sheet_index != (duplex ? index / 2 : index))
            throw std::runtime_error("invalid print sheet side");
        for (const auto& placement : input["slots"].toArray()) {
            const auto item = placement.toObject();
            Slot slot;
            slot.card_id = item["card_id"].toString();
            slot.copy_index = integer(item, "copy_index", 1, 65535);
            slot.rect_mm = rect(item["rect_mm"].toObject());
            slot.content_rect_mm = rect(item["content_rect_mm"].toObject());
            slot.resource_id =
                item["resource_id"].isUndefined() ? slot.card_id : item["resource_id"].toString();
            slot.face = item["face"].isUndefined() && !hasBack ? QStringLiteral("front")
                                                               : item["face"].toString();
            if (slot.card_id.isEmpty() || !result->cards.contains(slot.resource_id)
                || slot.face != page.side
                || slot.resource_id
                       != (slot.face == "back" ? QStringLiteral("@deck/back") : slot.card_id)
                || !bounds.contains(slot.rect_mm)
                || !slot.rect_mm.adjusted(-0.001, -0.001, 0.001, 0.001)
                        .contains(slot.content_rect_mm))
                throw std::runtime_error("invalid card placement");
            auto& copies = slot.face == "back" ? backs : fronts;
            if (!copies.emplace(Copy{slot.card_id, slot.copy_index}, page.sheet_index).second)
                throw std::runtime_error("duplicate print copy");
            page.placements.push_back(slot);
            if (++placements > expectedPlacements || page.placements.size() > 512)
                throw std::runtime_error("too many print copies");
        }
        for (const auto& line : input["crop_marks"].toArray()) {
            const auto mark = line.toObject();
            QLineF segment(
                number(mark, "x1"),
                number(mark, "y1"),
                number(mark, "x2"),
                number(mark, "y2")
            );
            if (!bounds.contains(segment.p1()) || !bounds.contains(segment.p2()))
                throw std::runtime_error("invalid crop mark");
            page.crop_marks.push_back(segment);
            if (page.crop_marks.size() > page.placements.size() * 8)
                throw std::runtime_error("too many crop marks");
        }
        if (page.placements.isEmpty() || page.crop_marks.isEmpty())
            throw std::runtime_error("empty print page");
        result->pages.push_back(std::move(page));
        if (result->pages.size() > expectedPages)
            throw std::runtime_error("too many print pages");
    }
    if (result->pages.size() != expectedPages || placements != expectedPlacements
        || (sides != "backs" && fronts.size() != size_t(expectedCopies))
        || (hasBack && backs.size() != size_t(expectedCopies)) || (duplex && fronts != backs))
        throw std::runtime_error("incomplete print plan");
    for (const auto* key :
         {"name",
          "source",
          "options",
          "imposition",
          "sheet_width_mm",
          "sheet_height_mm",
          "columns",
          "rows",
          "slots_per_page",
          "total_copies",
          "page_count"})
        result->summary[key] = plan[key];
    result->summary["total_placements"] = placements;
    result->summary["sheet_count"] = expectedSheets;
    return result;
}
QImage preview(const Document& document, int index) {
    const double scale = 1200. / std::max(document.sheet_mm.width(), document.sheet_mm.height());
    QImage image(
        QSize(
            int(std::ceil(document.sheet_mm.width() * scale)),
            int(std::ceil(document.sheet_mm.height() * scale))
        ),
        QImage::Format_RGB32
    );
    if (image.isNull())
        throw std::runtime_error("print preview allocation failed");
    QPainter painter(&image);
    painter.scale(scale, scale);
    SheetPainter(document).paint(painter, index);
    return image;
}
void exportPdf(const Document& document, const QString& destination, const QString& project) {
    const auto path = destinationPath(destination, project);
    QTemporaryFile temporary(QFileInfo(path).dir().filePath(".deck-cards-XXXXXX.pdf"));
    if (!temporary.open())
        throw std::runtime_error("cannot create PDF temporary file");
    const auto temporary_path = temporary.fileName();
    temporary.close();
    {
        QPdfWriter writer(temporary_path);
        writer.setResolution(300);
        writer.setPageSize(
            QPageSize(document.sheet_mm, QPageSize::Millimeter, QString(), QPageSize::ExactMatch)
        );
        writer.setPageMargins(QMarginsF(0, 0, 0, 0));
        writer.setTitle(document.name);
        writer.setCreator(QStringLiteral("Deck"));
        QPainter painter(&writer);
        if (!painter.isActive())
            throw std::runtime_error("PDF writer unavailable");
        SheetPainter sheets(document);
        for (int page = 0; page < document.pages.size(); ++page) {
            if (page && !writer.newPage())
                throw std::runtime_error("PDF page unavailable");
            const QPainterStateGuard state(&painter);
            painter.scale(writer.resolution() / 25.4, writer.resolution() / 25.4);
            sheets.paint(painter, page);
        }
        if (!painter.end())
            throw std::runtime_error("PDF finalization failed");
    }
    if (QFileInfo(temporary_path).size() == 0)
        throw std::runtime_error("empty PDF");
    std::error_code error;
    std::filesystem::create_hard_link(temporary_path.toStdString(), path.toStdString(), error);
    if (error)
        throw std::runtime_error(error.message());
}
} // namespace card_print
