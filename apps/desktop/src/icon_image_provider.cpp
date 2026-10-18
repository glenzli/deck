#include "icon_image_provider.hpp"
#include <QColor>
#include <QImage>
#include <QPainter>
#include <QSvgRenderer>
#include <QUrl>
#include <QUrlQuery>

DeckIconImageProvider::DeckIconImageProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}

QImage
DeckIconImageProvider::requestImage(const QString& id, QSize* size, const QSize& requestedSize) {
    const QUrlQuery query(QUrl("image://deck-icons/" + id));
    const QUrl source(query.queryItemValue("source", QUrl::FullyDecoded));
    const QColor color(query.queryItemValue("color", QUrl::FullyDecoded));
    if (size)
        *size = {};
    if (source.scheme() != "qrc" || !source.path().startsWith("/qt/qml/Shape/Desktop/icons/")
        || !source.path().endsWith(".svg") || !color.isValid())
        return {};
    QSvgRenderer renderer(":" + source.path());
    if (!renderer.isValid())
        return {};
    const auto pixels = requestedSize.isEmpty() ? QSize(24, 24) : requestedSize;
    QImage image(pixels.boundedTo(QSize(512, 512)), QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    QPainter painter(&image);
    painter.setRenderHint(QPainter::Antialiasing);
    renderer.render(&painter, QRectF(QPointF(), image.size()));
    painter.setCompositionMode(QPainter::CompositionMode_SourceIn);
    painter.fillRect(image.rect(), color);
    painter.end();
    if (size)
        *size = image.size();
    return image;
}
