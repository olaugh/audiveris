// SPDX-License-Identifier: AGPL-3.0-or-later
#include "Parsers.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <cmath>

namespace omrscope {

namespace {

std::optional<double> optionalDouble(const QJsonValue &value)
{
    return value.isDouble() ? std::optional<double>(value.toDouble()) : std::nullopt;
}

/// A grade with no contextual value is not a grade of zero.
QString formatMissing()
{
    return QStringLiteral("—");
}

} // namespace

QString describe(const std::optional<double> &value)
{
    return value ? QString::number(*value, 'f', 6) : formatMissing();
}

bool Pairing::agrees() const
{
    if (!rust || !java) {
        return false;
    }
    // Grades are compared tightly on purpose. Both engines emit full
    // precision, and the port's claim is exactness rather than closeness, so a
    // difference of 1e-9 is a real disagreement worth showing rather than
    // rounding away.
    return std::abs(rust->grade - java->grade) <= 1e-9;
}

EngineResult parseRustJson(const QString &text)
{
    EngineResult result;
    result.engine = QStringLiteral("rust");
    result.raw = text;

    // One document per sheet, one per line. Only the first is shown; the
    // sheet picker selects which one was asked for.
    const QStringList lines = text.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    if (lines.isEmpty()) {
        result.error = QStringLiteral("no output");
        return result;
    }

    QJsonParseError parseError{};
    const QJsonDocument document = QJsonDocument::fromJson(lines.first().toUtf8(), &parseError);
    if (document.isNull()) {
        result.error = QStringLiteral("unparsable JSON: %1").arg(parseError.errorString());
        return result;
    }

    const QJsonObject root = document.object();
    const QJsonObject image = root.value(QStringLiteral("image")).toObject();
    result.image = QSize(image.value(QStringLiteral("width")).toInt(),
                         image.value(QStringLiteral("height")).toInt());

    const QJsonArray staves = root.value(QStringLiteral("staves")).toArray();
    for (const QJsonValue &value : staves) {
        const QJsonObject object = value.toObject();
        Staff staff;
        staff.id = object.value(QStringLiteral("id")).toInt();
        staff.left = object.value(QStringLiteral("left")).toDouble();
        staff.right = object.value(QStringLiteral("right")).toDouble();
        staff.lineCount = object.value(QStringLiteral("line_count")).toInt();
        for (const QJsonValue &lineValue : object.value(QStringLiteral("lines")).toArray()) {
            const QJsonObject line = lineValue.toObject();
            QPolygonF polygon;
            for (const QJsonValue &pointValue : line.value(QStringLiteral("points")).toArray()) {
                const QJsonObject point = pointValue.toObject();
                polygon << QPointF(point.value(QStringLiteral("x")).toDouble(),
                                   point.value(QStringLiteral("y")).toDouble());
            }
            if (!polygon.isEmpty()) {
                staff.lines << polygon;
            }
        }
        result.staves << staff;
    }

    for (const QJsonValue &value : root.value(QStringLiteral("inters")).toArray()) {
        const QJsonObject object = value.toObject();
        Inter inter;
        inter.id = object.value(QStringLiteral("id")).toInt();
        inter.kind = object.value(QStringLiteral("kind")).toString();
        inter.shape = inter.kind;
        inter.staff = object.value(QStringLiteral("staff")).toInt(-1);
        inter.grade = object.value(QStringLiteral("grade")).toDouble();
        inter.contextual = optionalDouble(object.value(QStringLiteral("contextual_grade")));

        const QJsonObject median = object.value(QStringLiteral("median")).toObject();
        if (!median.isEmpty()) {
            const double x = median.value(QStringLiteral("x")).toDouble();
            inter.median = QLineF(x, median.value(QStringLiteral("top")).toDouble(), x,
                                  median.value(QStringLiteral("bottom")).toDouble());
        }

        const QJsonObject evidence = object.value(QStringLiteral("evidence")).toObject();
        inter.frozen = evidence.value(QStringLiteral("frozen")).toBool();
        const QJsonObject impacts = evidence.value(QStringLiteral("impacts")).toObject();
        for (auto it = impacts.begin(); it != impacts.end(); ++it) {
            inter.impacts << Impact{it.key(), it.value().toDouble()};
        }
        result.inters << inter;
    }

    // The candidates that lost. A recogniser judged only on what it emitted
    // cannot be judged on what it missed, so they are first-class here.
    for (const QJsonValue &value : root.value(QStringLiteral("candidates")).toArray()) {
        const QJsonObject object = value.toObject();
        Inter inter;
        inter.kind = object.value(QStringLiteral("kind")).toString();
        inter.staff = object.value(QStringLiteral("staff")).toInt(-1);
        inter.rejected = true;
        inter.rejectedBy = object.value(QStringLiteral("evidence"))
                               .toObject()
                               .value(QStringLiteral("rejected_by"))
                               .toString();
        const QJsonObject span = object.value(QStringLiteral("span")).toObject();
        const double start = span.value(QStringLiteral("start")).toDouble();
        const double stop = span.value(QStringLiteral("stop")).toDouble();
        inter.bounds = QRectF(start, 0, std::max(1.0, stop - start + 1), 0);
        result.inters << inter;
    }

    result.relationCount = root.value(QStringLiteral("relations")).toArray().size();
    result.ran = true;
    return result;
}

EngineResult parseSigProbe(const QString &text)
{
    EngineResult result;
    result.engine = QStringLiteral("java");
    result.raw = text;

    for (const QString &line : text.split(QLatin1Char('\n'))) {
        const QStringList fields = line.trimmed().split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (fields.isEmpty()) {
            continue;
        }
        const QString &record = fields.first();

        if (record == QLatin1String("failed")) {
            result.error = fields.mid(1).join(QLatin1Char(' '));
            continue;
        }
        if (record == QLatin1String("timing") && fields.size() >= 2) {
            result.millis = fields[1].toDouble();
            continue;
        }
        if (record == QLatin1String("nostaff") && fields.size() >= 3) {
            result.image = QSize(fields[1].toInt(), fields[2].toInt());
            continue;
        }
        if (record == QLatin1String("relation")) {
            result.relationCount++;
            continue;
        }
        if (record == QLatin1String("staff") && fields.size() >= 5) {
            Staff staff;
            staff.id = fields[1].toInt();
            staff.left = fields[2].toDouble();
            staff.right = fields[3].toDouble();
            staff.lineCount = fields[4].toInt();
            result.staves << staff;
            continue;
        }
        if (record != QLatin1String("inter") || fields.size() < 6) {
            continue;
        }

        // inter <system> <id> <class> <shape> <staff> bounds x y w h
        //       grade <g> ctx <c> frozen <b> [impacts <name> <v> ...]
        Inter inter;
        inter.id = fields[2].toInt();
        inter.kind = fields[3];
        inter.shape = fields[4];
        inter.staff = fields[5].toInt();

        const int boundsAt = fields.indexOf(QStringLiteral("bounds"));
        if (boundsAt >= 0 && fields.size() > boundsAt + 4
            && fields[boundsAt + 1] != QLatin1String("none")) {
            inter.bounds = QRectF(fields[boundsAt + 1].toDouble(), fields[boundsAt + 2].toDouble(),
                                  fields[boundsAt + 3].toDouble(), fields[boundsAt + 4].toDouble());
        }
        const int gradeAt = fields.indexOf(QStringLiteral("grade"));
        if (gradeAt >= 0 && fields.size() > gradeAt + 1) {
            inter.grade = fields[gradeAt + 1].toDouble();
        }
        const int ctxAt = fields.indexOf(QStringLiteral("ctx"));
        if (ctxAt >= 0 && fields.size() > ctxAt + 1 && fields[ctxAt + 1] != QLatin1String("none")) {
            inter.contextual = fields[ctxAt + 1].toDouble();
        }
        const int frozenAt = fields.indexOf(QStringLiteral("frozen"));
        if (frozenAt >= 0 && fields.size() > frozenAt + 1) {
            inter.frozen = fields[frozenAt + 1] == QLatin1String("true");
        }
        const int impactsAt = fields.indexOf(QStringLiteral("impacts"));
        if (impactsAt >= 0) {
            for (int at = impactsAt + 1; at + 1 < fields.size(); at += 2) {
                inter.impacts << Impact{fields[at], fields[at + 1].toDouble()};
            }
        }
        result.inters << inter;
    }

    result.ran = result.error.isEmpty();
    return result;
}

QVector<Pairing> pair(const EngineResult &rust, const EngineResult &java, const QString &kindFilter)
{
    // Paired by staff and abscissa rather than by id: the two engines number
    // their inters independently, and an id match would be a coincidence.
    auto abscissa = [](const Inter &inter) {
        if (inter.median) {
            return inter.median->x1();
        }
        if (inter.bounds) {
            return inter.bounds->center().x();
        }
        return std::numeric_limits<double>::quiet_NaN();
    };
    auto wanted = [&](const Inter &inter) {
        return kindFilter.isEmpty() || inter.kind.contains(kindFilter, Qt::CaseInsensitive)
            || inter.shape.contains(kindFilter, Qt::CaseInsensitive);
    };

    QVector<Inter> javaLeft;
    for (const Inter &inter : java.inters) {
        if (wanted(inter)) {
            javaLeft << inter;
        }
    }

    // An inter with no abscissa -- a connector, a brace -- cannot be matched
    // by position, and calling it "unpaired" would read as a disagreement when
    // it is simply not a comparable quantity. They are listed and labelled.
    auto comparable = [&](const Inter &inter) { return !std::isnan(abscissa(inter)); };

    QVector<Pairing> rows;
    for (const Inter &left : rust.inters) {
        if (!wanted(left)) {
            continue;
        }
        Pairing row;
        row.rust = left;
        if (!comparable(left)) {
            row.note = QStringLiteral("no geometry to compare");
            rows << row;
            continue;
        }
        if (!left.rejected) {
            int best = -1;
            double bestDelta = 2.0; // Within two pixels, or it is not the same thing.
            for (int index = 0; index < javaLeft.size(); ++index) {
                const Inter &right = javaLeft[index];
                if (right.staff != left.staff) {
                    continue;
                }
                const double delta = std::abs(abscissa(right) - abscissa(left));
                if (delta < bestDelta) {
                    bestDelta = delta;
                    best = index;
                }
            }
            if (best >= 0) {
                row.java = javaLeft[best];
                javaLeft.remove(best);
            } else {
                row.note = QStringLiteral("only Rust");
            }
        } else {
            row.note = QStringLiteral("rejected: %1").arg(left.rejectedBy);
        }
        rows << row;
    }

    // Whatever Java found and Rust did not is the interesting direction, so it
    // is listed rather than dropped.
    for (const Inter &right : javaLeft) {
        Pairing row;
        row.java = right;
        row.note = comparable(right) ? QStringLiteral("only Java")
                                     : QStringLiteral("no geometry to compare");
        rows << row;
    }
    return rows;
}

} // namespace omrscope
