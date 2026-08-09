// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <QGraphicsView>
#include <QStringList>

class QGraphicsScene;
class QGraphicsRectItem;
class QGraphicsSvgItem;
class QResizeEvent;
class QSvgRenderer;

namespace omrscope {

/// A deliberately presentation-only view of a renderer-owned SVG page.
///
/// The view knows neither MusicXML nor recognition semantics.  It receives
/// SVG files that ScoreRenderer has already validated, displays one page at a
/// time, and exposes page navigation and zoom without implying that visual
/// similarity is a score-parity result.
class ScoreView final : public QGraphicsView
{
    Q_OBJECT

public:
    explicit ScoreView(QWidget *parent = nullptr);

    void setPages(const QStringList &pagePaths);
    void clearPages();
    void setCurrentPage(int page); ///< Zero-based; ignored when out of range.
    void setZoomPercent(int percent);

    [[nodiscard]] int pageCount() const;
    [[nodiscard]] int currentPage() const;
    [[nodiscard]] int zoomPercent() const;

signals:
    void currentPageChanged(int page, int pageCount);

protected:
    void resizeEvent(QResizeEvent *event) override;

private:
    void loadCurrentPage();
    void applyZoom();

    QGraphicsScene *scene_ = nullptr;
    QGraphicsRectItem *paperItem_ = nullptr;
    QGraphicsSvgItem *pageItem_ = nullptr;
    QSvgRenderer *pageRenderer_ = nullptr;
    QStringList pagePaths_;
    int currentPage_ = -1;
    int zoomPercent_ = 100;
};

} // namespace omrscope
