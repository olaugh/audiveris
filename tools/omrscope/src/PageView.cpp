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
const QColor kRustRelation(41, 128, 185, 150);
const QColor kJavaRelation(192, 57, 43, 150);
const QColor kVisibleUnderlay(241, 196, 15, 72);
const QColor kSelectedRust(23, 162, 184);
const QColor kSelectedJava(155, 89, 182);

double abscissaOf(const Inter &inter)
{
    if (inter.median) {
        return inter.median->center().x();
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
    selectedPairing_.reset();
    update();
}

void PageView::setShowRust(bool show) { showRust_ = show; update(); }
void PageView::setShowJava(bool show) { showJava_ = show; update(); }
void PageView::setShowRejected(bool show) { showRejected_ = show; update(); }
void PageView::setShowStaves(bool show) { showStaves_ = show; update(); }
void PageView::setShowRelations(bool show) { showRelations_ = show; update(); }
void PageView::setShowVisiblePairs(bool show) { showVisiblePairs_ = show; update(); }

void PageView::setVisiblePairs(const QVector<Pairing> &pairs)
{
    visiblePairs_ = pairs;
    update();
}

void PageView::setSelectedPairing(const std::optional<Pairing> &pairing)
{
    selectedPairing_ = pairing;
    if (selectedPairing_) {
        if (const auto focus = pairingFocus(*selectedPairing_)) {
            centerOn(*focus);
            return;
        }
    }
    update();
}

std::optional<QPointF> PageView::selectedFocus() const
{
    return selectedPairing_ ? pairingFocus(*selectedPairing_) : std::nullopt;
}

std::optional<QPointF> PageView::selectedFocusInViewport() const
{
    const auto focus = selectedFocus();
    return focus ? std::optional<QPointF>(sheetToWidget().map(*focus)) : std::nullopt;
}

int PageView::drawableRelationCount(Engine engine) const
{
    return resolvedRelationLines(engine == Engine::Rust ? rust_ : java_).size();
}

std::optional<QRectF> PageView::visualBounds(const Inter &inter,
                                              const EngineResult &result) const
{
    if (inter.rejected) {
        if (!inter.bounds) {
            return std::nullopt;
        }
        const QRectF &span = *inter.bounds;
        double top = 0.0;
        double bottom = std::max(24.0, double(result.image.height()) * 0.02);
        for (const Staff &staff : result.staves) {
            if (staff.id != inter.staff) {
                continue;
            }
            if (const auto extent = staff.verticalExtent()) {
                top = extent->first;
                bottom = extent->second;
            }
            break;
        }
        return QRectF(span.left(), top, span.width(), bottom - top).normalized();
    }
    if (inter.median) {
        return QRectF(inter.median->p1(), inter.median->p2()).normalized();
    }
    return inter.bounds ? std::optional<QRectF>(inter.bounds->normalized()) : std::nullopt;
}

std::optional<QPointF> PageView::pairingFocus(const Pairing &pairing) const
{
    std::optional<QRectF> extent;
    const auto include = [&](const std::optional<Inter> &inter, const EngineResult &result) {
        if (!inter) {
            return;
        }
        const auto bounds = visualBounds(*inter, result);
        if (!bounds) {
            return;
        }
        extent = extent ? extent->united(*bounds) : *bounds;
    };
    include(pairing.rust, rust_);
    include(pairing.java, java_);
    return extent ? std::optional<QPointF>(extent->center()) : std::nullopt;
}

void PageView::centerOn(const QPointF &point)
{
    if (width() <= 0 || height() <= 0) {
        return;
    }
    const QTransform transform = sheetToWidget();
    if (transform.m11() <= 0.0 || transform.m22() <= 0.0) {
        return;
    }
    pan_ += rect().center() - transform.map(point);
    update();
}

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

    const auto drawBounds = [&](const Inter &inter, const EngineResult &result,
                                const QPen &outline, const QColor &fill = QColor()) {
        const auto bounds = visualBounds(inter, result);
        if (!bounds) {
            return false;
        }
        painter.setPen(outline);
        painter.setBrush(fill);
        if (inter.median && !inter.rejected) {
            painter.drawLine(*inter.median);
        } else {
            painter.drawRect(*bounds);
        }
        return true;
    };

    // Relations are producer-owned engine-local graph edges. They are drawn
    // beneath the inter marks and only after their reported ids resolve
    // uniquely in this exact stage snapshot.
    if (showRelations_) {
        const auto drawRelations = [&](const EngineResult &result, const QColor &colour) {
            QPen relationPen(colour, pen * 1.25);
            relationPen.setStyle(Qt::DashLine);
            painter.setPen(relationPen);
            for (const RelationLine &relation : resolvedRelationLines(result)) {
                painter.drawLine(relation.line);
            }
        };
        if (showJava_) {
            drawRelations(java_, kJavaRelation);
        }
        if (showRust_) {
            drawRelations(rust_, kRustRelation);
        }
    }

    // The filtered table list is a context layer, not a new comparison
    // result. Paint it first so green/blue/red agreement strokes remain the
    // visual answer underneath a translucent yellow halo.
    if (showVisiblePairs_) {
        QPen underlay(kVisibleUnderlay, pen * 7.0);
        for (const Pairing &pairing : visiblePairs_) {
            if (showJava_ && pairing.java) {
                drawBounds(*pairing.java, java_, underlay, QColor());
            }
            if (showRust_ && pairing.rust) {
                drawBounds(*pairing.rust, rust_, underlay, QColor());
            }
        }
    }

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
            drawBounds(inter, java_, QPen(kJavaOnly, pen * 2.0));
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
                drawBounds(inter, rust_, dashed);
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
            } else {
                drawBounds(inter, rust_, QPen(matched ? kAgree : kRustOnly, pen * 2.0));
            }
        }
    }

    // Selection is an interaction overlay, deliberately independent of the
    // engine visibility toggles: selecting a Java-only table row must still
    // identify the Java geometry. Its labels make the two endpoint overlays
    // unambiguous when their bounds are near one another.
    if (selectedPairing_) {
        const auto drawSelected = [&](const std::optional<Inter> &inter,
                                      const EngineResult &result, const QColor &colour,
                                      const QString &label, double labelOffset) {
            if (!inter) {
                return;
            }
            const auto bounds = visualBounds(*inter, result);
            if (!bounds) {
                return;
            }
            QPen outline(colour, pen * 3.5);
            drawBounds(*inter, result, outline, QColor(colour.red(), colour.green(), colour.blue(), 48));
            QFont font = painter.font();
            font.setPointSizeF(std::max(1.0, 9.0 * pen));
            painter.setFont(font);
            painter.setPen(QPen(colour, pen));
            painter.setBrush(Qt::NoBrush);
            const QPointF anchor(bounds->left(), bounds->top() - (3.0 + labelOffset) * pen);
            painter.drawText(anchor, label);
        };
        const bool both = selectedPairing_->rust && selectedPairing_->java;
        drawSelected(selectedPairing_->rust, rust_, kSelectedRust,
                     tr("Selected Rust · %1").arg(selectedPairing_->rust
                         ? selectedPairing_->rust->kind : QString()), 0.0);
        drawSelected(selectedPairing_->java, java_, kSelectedJava,
                     tr("Selected Java · %1").arg(selectedPairing_->java
                         ? selectedPairing_->java->kind : QString()), both ? 12.0 : 0.0);
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
