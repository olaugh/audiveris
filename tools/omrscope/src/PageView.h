// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QImage>
#include <QWidget>
#include <optional>

namespace omrscope {

/// The sheet, with what each engine found drawn over it.
///
/// Colour carries the only thing worth seeing at a glance: green where the two
/// engines agree, and red or amber where they do not. Rejected candidates are
/// dashed, because "we considered this and dropped it" is a different claim
/// from "we never saw it" and the difference is what makes a miss diagnosable.
class PageView : public QWidget
{
    Q_OBJECT

public:
    explicit PageView(QWidget *parent = nullptr);

    void setImage(const QImage &image);
    void setResults(const EngineResult &rust, const EngineResult &java);
    void setShowRust(bool show);
    void setShowJava(bool show);
    void setShowRejected(bool show);
    void setShowStaves(bool show);
    void setShowRelations(bool show);
    /// A translucent underlay for all rows in the current table filter. The
    /// ordinary agreement colours are painted after it and remain authoritative.
    void setShowVisiblePairs(bool show);
    void setVisiblePairs(const QVector<Pairing> &pairs);
    /// Select exactly the already paired same-stage row from the Inters table.
    /// A drawable endpoint is centered; geometry-free rows are left unplaced.
    void setSelectedPairing(const std::optional<Pairing> &pairing);

    [[nodiscard]] std::optional<QPointF> selectedFocus() const;
    [[nodiscard]] std::optional<QPointF> selectedFocusInViewport() const;
    [[nodiscard]] int drawableRelationCount(Engine engine) const;

protected:
    void paintEvent(QPaintEvent *event) override;
    void wheelEvent(QWheelEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;

private:
    QTransform sheetToWidget() const;
    std::optional<QRectF> visualBounds(const Inter &inter, const EngineResult &result) const;
    std::optional<QPointF> pairingFocus(const Pairing &pairing) const;
    void centerOn(const QPointF &point);

    QImage image_;
    EngineResult rust_;
    EngineResult java_;
    bool showRust_ = true;
    bool showJava_ = true;
    bool showRejected_ = true;
    bool showStaves_ = true;
    bool showRelations_ = false;
    bool showVisiblePairs_ = false;
    QVector<Pairing> visiblePairs_;
    std::optional<Pairing> selectedPairing_;
    double zoom_ = 0.0; ///< Zero means fit.
    QPointF pan_;
    QPoint dragFrom_;
};

} // namespace omrscope
