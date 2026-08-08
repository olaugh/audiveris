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
        R"json({"schema":1,"image":{"width":100,"height":200},"inters":[{"id":1,"kind":"THIN_BARLINE","staff":2,"grade":0.9,"median":{"x":12.5,"top":20.25,"bottom":80.75}},{"id":2,"kind":"BEAM","staff":2,"grade":0.8,"median":{"x1":10.25,"y1":30.5,"x2":50.75,"y2":31.5}},{"id":3,"kind":"LEDGER","staff":2,"grade":0.7,"median":{"x1":60.0,"y1":90.0,"x2":80.0}}]})json");

    const omrscope::EngineResult result = omrscope::parseRustJson(json);
    bool ok = true;
    ok &= check(result.ran, "schema-1 document parses");
    ok &= check(result.inters.size() == 3, "all inter records are retained");

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

    return ok ? 0 : 1;
}
