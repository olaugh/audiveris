// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QImage>
#include <QWidget>

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

protected:
    void paintEvent(QPaintEvent *event) override;
    void wheelEvent(QWheelEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;

private:
    QTransform sheetToWidget() const;

    QImage image_;
    EngineResult rust_;
    EngineResult java_;
    bool showRust_ = true;
    bool showJava_ = true;
    bool showRejected_ = true;
    bool showStaves_ = true;
    double zoom_ = 0.0; ///< Zero means fit.
    QPointF pan_;
    QPoint dragFrom_;
};

} // namespace omrscope
