// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreView.h"

#include <QFile>
#include <QGraphicsRectItem>
#include <QGraphicsScene>
#include <QGraphicsSvgItem>
#include <QImage>
#include <QPainter>
#include <QTemporaryDir>
#include <QtTest>

namespace {

bool writeSvg(const QString &path, int width)
{
    QFile file(path);
    return file.open(QIODevice::WriteOnly)
        && file.write(QStringLiteral(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"%1\" height=\"100\">"
                "<text x=\"10\" y=\"20\">page</text></svg>")
                             .arg(width)
                             .toUtf8()) > 0;
}

bool writeVerovioSvg(const QString &path)
{
    QFile file(path);
    return file.open(QIODevice::WriteOnly)
        && file.write(QByteArrayLiteral(
               "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
               "<svg xmlns=\"http://www.w3.org/2000/svg\" "
               "xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"200px\" height=\"100px\">"
               "<defs><g id=\"note-glyph\"><rect width=\"600\" height=\"600\" "
               "fill=\"#000000\"/></g></defs>"
               "<svg class=\"definition-scale\" color=\"black\" viewBox=\"0 0 2000 1000\">"
               "<use xlink:href=\"#note-glyph\" transform=\"translate(200 200)\"/>"
               "<text x=\"1200\" y=\"500\" font-size=\"0px\"><tspan id=\"label\" class=\"text\">"
               "<tspan font-size=\"200px\">A</tspan></tspan></text>"
               "</svg></svg>")) > 0;
}

} // namespace

class ScoreViewTest : public QObject
{
    Q_OBJECT

private slots:
    void pagesNavigateAndZoomWithoutChangingTheArtifactList();
    void verovioDefinitionScaleRendersNotationPixels();
};

void ScoreViewTest::pagesNavigateAndZoomWithoutChangingTheArtifactList()
{
    QTemporaryDir temporary;
    QVERIFY(temporary.isValid());
    const QString first = temporary.filePath(QStringLiteral("page-1.svg"));
    const QString second = temporary.filePath(QStringLiteral("page-2.svg"));
    QVERIFY(writeSvg(first, 200));
    QVERIFY(writeSvg(second, 300));

    omrscope::ScoreView view;
    view.setMinimumSize(0, 0);
    view.resize(640, 480);
    view.setPages({first, second});
    QCOMPARE(view.pageCount(), 2);
    QCOMPARE(view.currentPage(), 0);

    QGraphicsRectItem *paper = nullptr;
    QGraphicsSvgItem *svg = nullptr;
    for (QGraphicsItem *item : view.scene()->items()) {
        if (!paper) {
            paper = qgraphicsitem_cast<QGraphicsRectItem *>(item);
        }
        if (!svg) {
            svg = qgraphicsitem_cast<QGraphicsSvgItem *>(item);
        }
    }
    QVERIFY(paper);
    QVERIFY(svg);
    QCOMPARE(paper->rect(), svg->boundingRect());
    QCOMPARE(paper->brush().color(), QColor(Qt::white));
    QVERIFY(paper->zValue() < svg->zValue());

    view.setCurrentPage(1);
    QCOMPARE(view.currentPage(), 1);
    view.setCurrentPage(5);
    QCOMPARE(view.currentPage(), 1);

    const qreal fittedScale = view.transform().m11();
    QVERIFY(fittedScale > 0.0);
    view.setZoomPercent(175);
    QCOMPARE(view.zoomPercent(), 175);
    QVERIFY(qAbs(view.transform().m11() - fittedScale * 1.75) < 0.0001);
    QCOMPARE(view.pageCount(), 2);

    view.clearPages();
    QCOMPARE(view.pageCount(), 0);
    QCOMPARE(view.currentPage(), -1);
}

void ScoreViewTest::verovioDefinitionScaleRendersNotationPixels()
{
    QTemporaryDir temporary;
    QVERIFY(temporary.isValid());
    const QString page = temporary.filePath(QStringLiteral("verovio-page.svg"));
    QVERIFY(writeVerovioSvg(page));

    omrscope::ScoreView view;
    view.setPages({page});
    QCOMPARE(view.pageCount(), 1);
    QCOMPARE(view.scene()->sceneRect(), QRectF(0, 0, 200, 100));

    QImage pixels(200, 100, QImage::Format_ARGB32_Premultiplied);
    pixels.fill(Qt::transparent);
    QPainter painter(&pixels);
    view.scene()->render(&painter, QRectF(pixels.rect()), view.scene()->sceneRect());
    painter.end();

    int black = 0;
    int blackInTextRegion = 0;
    int white = 0;
    for (int y = 0; y < pixels.height(); ++y) {
        for (int x = 0; x < pixels.width(); ++x) {
            const QColor colour = pixels.pixelColor(x, y);
            if (colour.red() < 16 && colour.green() < 16 && colour.blue() < 16
                && colour.alpha() > 240) {
                ++black;
                if (x > 100) {
                    ++blackInTextRegion;
                }
            }
            if (colour.red() > 240 && colour.green() > 240 && colour.blue() > 240
                && colour.alpha() > 240) {
                ++white;
            }
        }
    }
    QVERIFY2(black > 3'000, "the nested Verovio notation must render above the paper");
    QVERIFY2(blackInTextRegion > 20,
             "Verovio text with an explicitly sized tspan must not be rejected with its zero-size parent");
    QVERIFY2(white > 15'000, "the page must retain a white paper field around the notation");
}

QTEST_MAIN(ScoreViewTest)

#include "ScoreViewTest.moc"
