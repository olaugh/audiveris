// SPDX-License-Identifier: AGPL-3.0-or-later
#include "PageView.h"

#include <QMouseEvent>
#include <QPainter>
#include <QWheelEvent>
#include <algorithm>

namespace omrscope {

namespace {

const QColor kAgree(38, 166, 91);
const QColor kRustOnly(41, 128, 185);
const QColor kJavaOnly(192, 57, 43);
const QColor kRejected(243, 156, 18);
const QColor kStaff(120, 120, 120);

double abscissaOf(const Inter &inter)
{
    if (inter.median) {
        return inter.median->x1();
    }
    if (inter.bounds) {
        return inter.bounds->center().x();
    }
    return std::numeric_limits<double>::quiet_NaN();
}

} // namespace

PageView::PageView(QWidget *parent)
    : QWidget(parent)
{
    setMinimumSize(480, 360);
    setMouseTracking(false);
    setCursor(Qt::OpenHandCursor);
}

void PageView::setImage(const QImage &image)
{
    image_ = image;
    zoom_ = 0.0;
    pan_ = QPointF();
    update();
}

void PageView::setResults(const EngineResult &rust, const EngineResult &java)
{
    rust_ = rust;
    java_ = java;
    update();
}

void PageView::setShowRust(bool show) { showRust_ = show; update(); }
void PageView::setShowJava(bool show) { showJava_ = show; update(); }
void PageView::setShowRejected(bool show) { showRejected_ = show; update(); }
void PageView::setShowStaves(bool show) { showStaves_ = show; update(); }

QTransform PageView::sheetToWidget() const
{
    const QSize sheet = image_.isNull()
        ? QSize(std::max(rust_.image.width(), java_.image.width()),
                std::max(rust_.image.height(), java_.image.height()))
        : image_.size();
    if (sheet.isEmpty()) {
        return {};
    }
    const double fit = std::min(double(width()) / sheet.width(), double(height()) / sheet.height());
    const double scale = zoom_ > 0.0 ? zoom_ : fit;
    QTransform transform;
    transform.translate(pan_.x(), pan_.y());
    transform.scale(scale, scale);
    return transform;
}

void PageView::paintEvent(QPaintEvent *)
{
    QPainter painter(this);
    painter.fillRect(rect(), QColor(24, 24, 27));
    const QTransform transform = sheetToWidget();
    if (transform.isIdentity() && image_.isNull()) {
        painter.setPen(Qt::lightGray);
        painter.drawText(rect(), Qt::AlignCenter, tr("Run an engine to see a sheet."));
        return;
    }

    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setTransform(transform);

    if (!image_.isNull()) {
        painter.drawImage(QPointF(0, 0), image_);
    }

    const double pen = 1.0 / std::max(0.0001, transform.m11());

    if (showStaves_) {
        painter.setPen(QPen(kStaff, pen));
        for (const Staff &staff : rust_.staves) {
            for (const QPolygonF &line : staff.lines) {
                painter.drawPolyline(line);
            }
            // Even before lines are persistent, the extent is worth seeing.
            if (staff.lines.isEmpty()) {
                painter.drawLine(QPointF(staff.left, 0), QPointF(staff.left, 12));
                painter.drawLine(QPointF(staff.right, 0), QPointF(staff.right, 12));
            }
        }
    }

    // Java first, so a disagreeing Rust mark draws over it and is visible.
    if (showJava_) {
        painter.setPen(QPen(kJavaOnly, pen * 2.0));
        for (const Inter &inter : java_.inters) {
            if (inter.bounds) {
                painter.drawRect(*inter.bounds);
            }
        }
    }

    if (showRust_) {
        for (const Inter &inter : rust_.inters) {
            if (inter.rejected) {
                if (!showRejected_) {
                    continue;
                }
                QPen dashed(kRejected, pen * 1.5);
                dashed.setStyle(Qt::DashLine);
                painter.setPen(dashed);
                if (inter.bounds) {
                    const QRectF &span = *inter.bounds;
                    painter.drawRect(QRectF(span.left(), 0, span.width(),
                                            std::max(24.0, double(rust_.image.height()) * 0.02)));
                }
                continue;
            }

            // Green when Java has something at the same place, blue when only
            // Rust does. That is the whole diff, visible without reading a
            // table.
            bool matched = false;
            const double x = abscissaOf(inter);
            for (const Inter &other : java_.inters) {
                if (other.staff == inter.staff && std::abs(abscissaOf(other) - x) < 2.0) {
                    matched = true;
                    break;
                }
            }
            painter.setPen(QPen(matched ? kAgree : kRustOnly, pen * 2.0));
            if (inter.median) {
                painter.drawLine(*inter.median);
            } else if (inter.bounds) {
                painter.drawRect(*inter.bounds);
            }
        }
    }
}

void PageView::wheelEvent(QWheelEvent *event)
{
    const QTransform current = sheetToWidget();
    const double scale = current.m11();
    if (scale <= 0.0) {
        return;
    }
    const double factor = event->angleDelta().y() > 0 ? 1.15 : 1.0 / 1.15;
    // Zoom about the cursor, so the thing under it stays under it.
    const QPointF before = current.inverted().map(QPointF(event->position()));
    zoom_ = std::clamp(scale * factor, 0.02, 40.0);
    const QTransform after = sheetToWidget();
    const QPointF moved = after.map(before);
    pan_ += QPointF(event->position()) - moved;
    update();
}

void PageView::mousePressEvent(QMouseEvent *event)
{
    dragFrom_ = event->pos();
}

void PageView::mouseMoveEvent(QMouseEvent *event)
{
    if (event->buttons() & Qt::LeftButton) {
        pan_ += QPointF(event->pos() - dragFrom_);
        dragFrom_ = event->pos();
        update();
    }
}

} // namespace omrscope
