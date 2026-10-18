#pragma once
#include <QQuickImageProvider>

// Rasterizes packaged UI vectors at the requested physical size and theme color.
// Qt Quick owns caching and display density; card artwork uses its own provider.
class DeckIconImageProvider final : public QQuickImageProvider {
  public:
    DeckIconImageProvider();
    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
