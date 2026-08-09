// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreView.h"

#include <QFile>
#include <QGraphicsScene>
#include <QGraphicsRectItem>
#include <QGraphicsSvgItem>
#include <QPainter>
#include <QRegularExpression>
#include <QResizeEvent>
#include <QSvgRenderer>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>

#include <algorithm>
#include <optional>

namespace omrscope {

namespace {

std::optional<QRectF> parseViewBox(QString value)
{
    const QStringList fields = value.split(QRegularExpression(QStringLiteral("[\\s,]+")),
                                           Qt::SkipEmptyParts);
    if (fields.size() != 4) {
        return std::nullopt;
    }
    double numbers[4];
    for (int index = 0; index < 4; ++index) {
        bool ok = false;
        numbers[index] = fields[index].toDouble(&ok);
        if (!ok || !qIsFinite(numbers[index])) {
            return std::nullopt;
        }
    }
    if (numbers[2] <= 0.0 || numbers[3] <= 0.0) {
        return std::nullopt;
    }
    return QRectF(numbers[0], numbers[1], numbers[2], numbers[3]);
}

std::optional<double> parsePixelLength(QString value)
{
    static const QRegularExpression length(
        QStringLiteral(R"(^\s*([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)\s*(?:px)?\s*$)"));
    const QRegularExpressionMatch match = length.match(value);
    if (!match.hasMatch()) {
        return std::nullopt;
    }
    bool ok = false;
    const double result = match.captured(1).toDouble(&ok);
    return ok && qIsFinite(result) && result > 0.0 ? std::optional<double>(result)
                                                   : std::nullopt;
}

bool isZeroPixelLength(QString value)
{
    value = value.trimmed();
    if (value.endsWith(QLatin1String("px"), Qt::CaseInsensitive)) {
        value.chop(2);
    }
    bool ok = false;
    const double result = value.trimmed().toDouble(&ok);
    return ok && qIsFinite(result) && qFuzzyIsNull(result);
}

std::optional<QRectF> rootViewport(const QXmlStreamAttributes &attributes)
{
    if (const auto viewBox = parseViewBox(attributes.value(QLatin1StringView("viewBox")).toString())) {
        return viewBox;
    }
    const auto width = parsePixelLength(attributes.value(QLatin1StringView("width")).toString());
    const auto height = parsePixelLength(attributes.value(QLatin1StringView("height")).toString());
    return width && height ? std::optional<QRectF>(QRectF(0.0, 0.0, *width, *height))
                           : std::nullopt;
}

bool hasClass(const QXmlStreamAttributes &attributes, QLatin1StringView wanted)
{
    const QStringList classes = attributes.value(QLatin1StringView("class"))
                                    .toString()
                                    .split(QRegularExpression(QStringLiteral("\\s+")),
                                           Qt::SkipEmptyParts);
    return classes.contains(wanted);
}

bool isVerovioTextWrapper(const QXmlStreamAttributes &attributes)
{
    if (!hasClass(attributes, QLatin1StringView("text"))) {
        return false;
    }
    return std::all_of(attributes.begin(), attributes.end(), [](const QXmlStreamAttribute &attribute) {
        return attribute.name() == QLatin1String("id") || attribute.name() == QLatin1String("class");
    });
}

QString number(double value)
{
    return QString::number(value, 'g', 17);
}

/// QtSvg intentionally supports the SVG Tiny profile and skips nested `<svg>` elements. Verovio
/// 6.x puts the complete page under one direct-child `definition-scale` SVG, whose viewBox is ten
/// times the outer page. Replace only that narrowly recognisable viewport with an equivalent group
/// transform. The renderer-owned source file remains untouched.
QByteArray svgForQt(const QByteArray &source)
{
    QXmlStreamReader xml(source);
    QByteArray output;
    QXmlStreamWriter writer(&output);
    int depth = 0;
    bool flattened = false;
    std::optional<QRectF> outerViewport;
    QVector<bool> skippedElements;

    const auto writeNamespaces = [&] {
        for (const QXmlStreamNamespaceDeclaration &declaration : xml.namespaceDeclarations()) {
            if (declaration.prefix().isEmpty()) {
                writer.writeDefaultNamespace(declaration.namespaceUri());
            } else {
                writer.writeNamespace(declaration.namespaceUri(), declaration.prefix());
            }
        }
    };
    const auto writeAttributes = [&](const QStringList &omitted = {}) {
        for (const QXmlStreamAttribute &attribute : xml.attributes()) {
            if (omitted.contains(attribute.name().toString())) {
                continue;
            }
            // Verovio puts zero on a parent text node and the real size on its tspans. Qt rejects
            // the parent font before it can reach those overrides; inheriting the normal default
            // here lets the explicitly sized children render as authored.
            if (flattened && xml.name() == QLatin1String("text")
                && attribute.name() == QLatin1String("font-size")
                && isZeroPixelLength(attribute.value().toString())) {
                continue;
            }
            writer.writeAttribute(attribute.qualifiedName().toString(), attribute.value().toString());
        }
    };

    while (!xml.atEnd()) {
        const QXmlStreamReader::TokenType token = xml.readNext();
        switch (token) {
        case QXmlStreamReader::StartDocument:
            writer.writeStartDocument(xml.documentVersion(), xml.isStandaloneDocument());
            break;
        case QXmlStreamReader::EndDocument:
            writer.writeEndDocument();
            break;
        case QXmlStreamReader::StartElement: {
            ++depth;
            const bool isSvg = xml.name().compare(QLatin1StringView("svg"),
                                                   Qt::CaseInsensitive) == 0;
            if (depth == 1 && isSvg) {
                outerViewport = rootViewport(xml.attributes());
            }

            bool flatten = false;
            QString transform;
            if (!flattened && depth == 2 && isSvg && outerViewport
                && hasClass(xml.attributes(), QLatin1StringView("definition-scale"))
                && !xml.attributes().hasAttribute(QLatin1StringView("x"))
                && !xml.attributes().hasAttribute(QLatin1StringView("y"))
                && !xml.attributes().hasAttribute(QLatin1StringView("width"))
                && !xml.attributes().hasAttribute(QLatin1StringView("height"))) {
                const auto inner = parseViewBox(
                    xml.attributes().value(QLatin1StringView("viewBox")).toString());
                const QString aspect = xml.attributes()
                                           .value(QLatin1StringView("preserveAspectRatio"))
                                           .toString()
                                           .trimmed();
                if (inner && (aspect.isEmpty() || aspect == QLatin1String("xMidYMid meet"))) {
                    const double scaleX = outerViewport->width() / inner->width();
                    const double scaleY = outerViewport->height() / inner->height();
                    const double tolerance = std::max(qAbs(scaleX), qAbs(scaleY)) * 1.0e-9;
                    if (qAbs(scaleX - scaleY) <= tolerance) {
                        transform = QStringLiteral("translate(%1 %2) scale(%3 %4) translate(%5 %6)")
                                        .arg(number(outerViewport->x()), number(outerViewport->y()),
                                             number(scaleX), number(scaleY), number(-inner->x()),
                                             number(-inner->y()));
                        const QString existing = xml.attributes()
                                                     .value(QLatin1StringView("transform"))
                                                     .toString()
                                                     .trimmed();
                        if (!existing.isEmpty()) {
                            transform += QLatin1Char(' ') + existing;
                        }
                        flatten = true;
                        flattened = true;
                    }
                }
            }

            // QtSvg also rejects Verovio's metadata-only tspan wrapper around the actually sized
            // tspan. Omitting that one exact wrapper preserves its text children as direct children
            // of the text element, which is the equivalent shape Qt accepts.
            const bool skip = flattened && xml.name() == QLatin1String("tspan")
                && isVerovioTextWrapper(xml.attributes());
            skippedElements.append(skip);
            if (skip) {
                break;
            }

            writer.writeStartElement(flatten ? QStringLiteral("g")
                                             : xml.qualifiedName().toString());
            writeNamespaces();
            if (flatten) {
                writeAttributes({QStringLiteral("viewBox"), QStringLiteral("preserveAspectRatio"),
                                 QStringLiteral("transform")});
                writer.writeAttribute(QStringLiteral("transform"), transform);
            } else {
                writeAttributes();
            }
            break;
        }
        case QXmlStreamReader::EndElement: {
            const bool skipped = skippedElements.takeLast();
            if (!skipped) {
                writer.writeEndElement();
            }
            --depth;
            break;
        }
        case QXmlStreamReader::Characters:
            if (xml.isCDATA()) {
                writer.writeCDATA(xml.text());
            } else {
                writer.writeCharacters(xml.text());
            }
            break;
        case QXmlStreamReader::Comment:
            writer.writeComment(xml.text());
            break;
        case QXmlStreamReader::DTD:
            writer.writeDTD(xml.text());
            break;
        case QXmlStreamReader::EntityReference:
            writer.writeEntityReference(xml.name());
            break;
        case QXmlStreamReader::ProcessingInstruction:
            writer.writeProcessingInstruction(xml.processingInstructionTarget(),
                                               xml.processingInstructionData());
            break;
        case QXmlStreamReader::NoToken:
        case QXmlStreamReader::Invalid:
            break;
        }
    }
    return flattened && !xml.hasError() ? output : source;
}

} // namespace

ScoreView::ScoreView(QWidget *parent)
    : QGraphicsView(parent)
    , scene_(new QGraphicsScene(this))
    , pageRenderer_(new QSvgRenderer(this))
{
    setScene(scene_);
    setRenderHint(QPainter::Antialiasing, true);
    setRenderHint(QPainter::TextAntialiasing, true);
    setBackgroundBrush(QColor(35, 35, 39));
    setMinimumSize(500, 360);
    setDragMode(QGraphicsView::ScrollHandDrag);
    setTransformationAnchor(QGraphicsView::AnchorUnderMouse);
}

void ScoreView::setPages(const QStringList &pagePaths)
{
    pagePaths_ = pagePaths;
    currentPage_ = pagePaths_.isEmpty() ? -1 : 0;
    loadCurrentPage();
    emit currentPageChanged(currentPage_, pagePaths_.size());
}

void ScoreView::clearPages()
{
    pagePaths_.clear();
    currentPage_ = -1;
    loadCurrentPage();
    emit currentPageChanged(currentPage_, 0);
}

void ScoreView::setCurrentPage(int page)
{
    if (page < 0 || page >= pagePaths_.size() || page == currentPage_) {
        return;
    }
    currentPage_ = page;
    loadCurrentPage();
    emit currentPageChanged(currentPage_, pagePaths_.size());
}

void ScoreView::setZoomPercent(int percent)
{
    zoomPercent_ = qBound(25, percent, 400);
    applyZoom();
}

int ScoreView::pageCount() const
{
    return pagePaths_.size();
}

int ScoreView::currentPage() const
{
    return currentPage_;
}

int ScoreView::zoomPercent() const
{
    return zoomPercent_;
}

void ScoreView::loadCurrentPage()
{
    scene_->clear();
    paperItem_ = nullptr;
    pageItem_ = nullptr;
    if (currentPage_ < 0 || currentPage_ >= pagePaths_.size()) {
        return;
    }
    QFile svg(pagePaths_.at(currentPage_));
    if (!svg.open(QIODevice::ReadOnly) || !pageRenderer_->load(svgForQt(svg.readAll()))) {
        return;
    }
    pageItem_ = new QGraphicsSvgItem;
    pageItem_->setSharedRenderer(pageRenderer_);
    // Verovio's default SVG uses black notation on a transparent canvas. Keep the application's
    // dark surrounding area while putting a literal white paper sheet behind each SVG; this is a
    // renderer presentation detail, not recognition/semantic colouring.
    paperItem_ = scene_->addRect(pageItem_->boundingRect(), Qt::NoPen, QBrush(Qt::white));
    paperItem_->setZValue(-1.0);
    scene_->addItem(pageItem_);
    scene_->setSceneRect(pageItem_->boundingRect());
    zoomPercent_ = 100;
    applyZoom();
}

void ScoreView::resizeEvent(QResizeEvent *event)
{
    QGraphicsView::resizeEvent(event);
    applyZoom();
}

void ScoreView::applyZoom()
{
    if (!pageItem_ || scene_->sceneRect().isEmpty() || viewport()->size().isEmpty()) {
        return;
    }
    // Rebuild the transform from the current viewport's fitted page before
    // applying the user zoom. 100% is therefore stable, and 175% means 1.75×
    // the fit-to-page transform rather than 1.75 SVG-intrinsic units.
    resetTransform();
    fitInView(scene_->sceneRect(), Qt::KeepAspectRatio);
    const qreal factor = qreal(zoomPercent_) / 100.0;
    QGraphicsView::scale(factor, factor);
}

} // namespace omrscope
