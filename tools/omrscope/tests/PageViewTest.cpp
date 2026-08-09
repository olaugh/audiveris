// SPDX-License-Identifier: AGPL-3.0-or-later
#include "PageView.h"
#include "Parsers.h"

#include <QImage>
#include <QtTest>

namespace {

omrscope::Inter inter(int id, int system, int staff, const QRectF &bounds,
                      const QString &kind = QStringLiteral("BEAM"))
{
    omrscope::Inter value;
    value.id = id;
    value.system = system;
    value.staff = staff;
    value.kind = kind;
    value.bounds = bounds;
    return value;
}

} // namespace

class PageViewTest : public QObject
{
    Q_OBJECT

private slots:
    void filteredPairSelectionCentersBothEngineGeometry();
    void relationEdgesRequireUniqueSameEngineEndpoints();
    void visibleRowsUnderlayKeepsAgreementStroke();
};

void PageViewTest::filteredPairSelectionCentersBothEngineGeometry()
{
    omrscope::EngineResult rust;
    rust.image = QSize(1000, 800);
    rust.inters << inter(10, 4, 2, QRectF(790, 480, 20, 40), QStringLiteral("BEAM"));
    rust.inters << inter(11, 4, 2, QRectF(100, 100, 10, 10), QStringLiteral("BARLINE"));

    omrscope::EngineResult java;
    java.image = rust.image;
    java.inters << inter(70, 4, 2, QRectF(791, 480, 20, 40), QStringLiteral("BEAM"));
    java.inters << inter(71, 4, 2, QRectF(100, 100, 10, 10), QStringLiteral("BARLINE"));

    const QVector<omrscope::Pairing> filtered = omrscope::pair(rust, java, QStringLiteral("BEAM"));
    QCOMPARE(filtered.size(), 1);
    QVERIFY(filtered[0].rust.has_value());
    QVERIFY(filtered[0].java.has_value());

    omrscope::PageView view;
    view.resize(400, 300);
    view.setImage(QImage(1000, 800, QImage::Format_ARGB32_Premultiplied));
    view.setResults(rust, java);
    view.setVisiblePairs(filtered);
    view.setSelectedPairing(filtered[0]);

    const auto focus = view.selectedFocus();
    const auto viewportFocus = view.selectedFocusInViewport();
    QVERIFY(focus.has_value());
    QVERIFY(viewportFocus.has_value());
    QCOMPARE(*focus, QPointF(800.5, 500));
    QVERIFY2(QLineF(*viewportFocus, view.rect().center()).length() <= 1.0,
             "selecting a filtered comparison row centers its same-stage pair on Page");
}

void PageViewTest::relationEdgesRequireUniqueSameEngineEndpoints()
{
    omrscope::EngineResult result;
    result.inters << inter(10, 3, 1, QRectF(10, 10, 8, 8));
    result.inters << inter(11, 3, 1, QRectF(40, 10, 8, 8));
    result.relations << omrscope::Relation{3, 10, 11, QStringLiteral("BeamStemRelation")};
    QCOMPARE(omrscope::resolvedRelationLines(result).size(), 1);

    // A geometry-free record with the same published id still makes the
    // endpoint identity ambiguous. Count identities before asking whether
    // either record happens to be drawable.
    omrscope::EngineResult mixedDuplicate = result;
    omrscope::Inter geometryFreeDuplicate;
    geometryFreeDuplicate.id = 10;
    geometryFreeDuplicate.system = 3;
    geometryFreeDuplicate.kind = QStringLiteral("BAR_CONNECTOR");
    mixedDuplicate.inters << geometryFreeDuplicate;
    QVERIFY(omrscope::resolvedRelationLines(mixedDuplicate).isEmpty());

    // A duplicate published id makes the source endpoint ambiguous. Do not
    // choose the nearer shape, which would create a viewer-made graph edge.
    result.inters << inter(10, 3, 1, QRectF(80, 10, 8, 8));
    QVERIFY(omrscope::resolvedRelationLines(result).isEmpty());

    omrscope::PageView view;
    view.setResults(result, {});
    QCOMPARE(view.drawableRelationCount(omrscope::Engine::Rust), 0);
}

void PageViewTest::visibleRowsUnderlayKeepsAgreementStroke()
{
    omrscope::EngineResult rust;
    rust.image = QSize(100, 100);
    omrscope::Inter rustBeam = inter(1, 1, 1, QRectF(10, 10, 80, 1));
    rustBeam.median = QLineF(10, 10, 90, 10);
    rust.inters << rustBeam;

    omrscope::EngineResult java;
    java.image = rust.image;
    omrscope::Inter javaBeam = inter(8, 1, 1, QRectF(10, 10, 80, 1));
    javaBeam.median = QLineF(10, 10, 90, 10);
    java.inters << javaBeam;

    omrscope::PageView view;
    view.setMinimumSize(0, 0);
    view.resize(200, 200);
    view.setImage(QImage(100, 100, QImage::Format_ARGB32_Premultiplied));
    view.setResults(rust, java);
    view.setVisiblePairs(omrscope::pair(rust, java, QStringLiteral("BEAM")));
    view.setShowVisiblePairs(true);

    const QImage rendered = view.grab().toImage();
    const QColor stroke = rendered.pixelColor(100, 20);
    QVERIFY2(stroke.green() > stroke.red() + 35 && stroke.green() > stroke.blue() + 35,
             "the visible-row underlay sits beneath the green agreement stroke");
}

QTEST_MAIN(PageViewTest)

#include "PageViewTest.moc"
