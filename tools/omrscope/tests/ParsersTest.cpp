// SPDX-License-Identifier: AGPL-3.0-or-later
#include "Parsers.h"

#include <QCoreApplication>
#include <QTextStream>
#include <cmath>

namespace {

bool close(double left, double right)
{
    return std::abs(left - right) <= 1e-12;
}

bool check(bool condition, const char *message)
{
    if (!condition) {
        QTextStream(stderr) << "FAIL: " << message << '\n';
    }
    return condition;
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    Q_UNUSED(application);

    const QString json = QStringLiteral(
        R"json({"schema":1,"image":{"width":100,"height":200},"inters":[{"id":1,"kind":"THIN_BARLINE","staff":2,"grade":0.9,"median":{"x":12.5,"top":20.25,"bottom":80.75}},{"id":2,"kind":"BEAM","staff":2,"grade":0.8,"median":{"x1":10.25,"y1":30.5,"x2":50.75,"y2":31.5}},{"id":3,"kind":"LEDGER","staff":2,"grade":0.7,"median":{"x1":60.0,"y1":90.0,"x2":80.0}},{"id":4,"kind":"F_CLEF","staff":2,"grade":0.95,"bounds":{"x":21.25,"y":40.5,"width":17.5,"height":42.25}},{"id":5,"kind":"KEY_SIGNATURE","staff":2,"grade":0.6,"bounds":{"x":50.0,"y":40.0,"width":12.0}}]})json");

    const omrscope::EngineResult result = omrscope::parseRustJson(json);
    bool ok = true;
    ok &= check(result.ran, "schema-1 document parses");
    ok &= check(result.inters.size() == 5, "all inter records are retained");

    const auto &vertical = result.inters[0].median;
    ok &= check(vertical.has_value(), "schema-1 GRID median is present");
    if (vertical) {
        ok &= check(close(vertical->x1(), 12.5) && close(vertical->x2(), 12.5)
                        && close(vertical->y1(), 20.25) && close(vertical->y2(), 80.75),
                    "schema-1 GRID median preserves x/top/bottom");
    }

    const auto &horizontal = result.inters[1].median;
    ok &= check(horizontal.has_value(), "BEAMS median is present");
    if (horizontal) {
        ok &= check(close(horizontal->x1(), 10.25) && close(horizontal->y1(), 30.5)
                        && close(horizontal->x2(), 50.75) && close(horizontal->y2(), 31.5),
                    "BEAMS median preserves both endpoints");
    }
    ok &= check(!result.inters[2].median.has_value(),
                "incomplete endpoint median is not fabricated with zeroes");

    const auto &header = result.inters[3].bounds;
    ok &= check(header.has_value(), "bounds-only HEADERS inter is present");
    if (header) {
        ok &= check(close(header->x(), 21.25) && close(header->y(), 40.5)
                        && close(header->width(), 17.5) && close(header->height(), 42.25),
                    "schema-1 HEADERS bounds preserve all four coordinates");
        ok &= check(close(header->center().x(), 30.0),
                    "bounds-only HEADERS display abscissa is the rectangle center");
    }
    ok &= check(!result.inters[4].bounds.has_value(),
                "incomplete HEADERS bounds are not fabricated with zeroes");

    omrscope::EngineResult java;
    omrscope::Inter javaBeam;
    javaBeam.kind = QStringLiteral("BeamInter");
    javaBeam.staff = 2;
    javaBeam.bounds = QRectF(29.5, 29.0, 2.0, 5.0);
    java.inters << javaBeam;
    const QVector<omrscope::Pairing> rows =
        omrscope::pair(result, java, QStringLiteral("beam"));
    ok &= check(rows.size() == 1 && rows[0].rust.has_value() && rows[0].java.has_value(),
                "horizontal median pairs by its center abscissa");

    omrscope::EngineResult javaHeaders;
    omrscope::Inter javaClef;
    javaClef.kind = QStringLiteral("ClefInter");
    javaClef.staff = 2;
    javaClef.bounds = QRectF(29.0, 39.0, 2.0, 45.0);
    javaHeaders.inters << javaClef;
    const QVector<omrscope::Pairing> headerRows =
        omrscope::pair(result, javaHeaders, QStringLiteral("clef"));
    ok &= check(headerRows.size() == 1 && headerRows[0].rust.has_value()
                    && headerRows[0].java.has_value(),
                "bounds-only HEADERS inter pairs by its center abscissa");

    return ok ? 0 : 1;
}
