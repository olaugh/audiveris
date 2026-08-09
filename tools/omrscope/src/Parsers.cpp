// SPDX-License-Identifier: AGPL-3.0-or-later
#include "Parsers.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <cmath>
#include <limits>

namespace omrscope {

namespace {

std::optional<double> optionalDouble(const QJsonValue &value)
{
    return value.isDouble() ? std::optional<double>(value.toDouble()) : std::nullopt;
}

std::optional<QRectF> parseBounds(const QJsonObject &bounds)
{
    // Selected HEADERS inters have bounds but no median. Require all four
    // schema-1 coordinates: QJsonValue::toDouble() would otherwise turn a
    // missing height into a plausible zero-height symbol at the origin.
    const auto x = optionalDouble(bounds.value(QStringLiteral("x")));
    const auto y = optionalDouble(bounds.value(QStringLiteral("y")));
    const auto width = optionalDouble(bounds.value(QStringLiteral("width")));
    const auto height = optionalDouble(bounds.value(QStringLiteral("height")));
    if (x && y && width && height) {
        return QRectF(*x, *y, *width, *height);
    }
    return std::nullopt;
}

std::optional<QLineF> parseMedian(const QJsonObject &median)
{
    // Schema 1 GRID records use a vertical median. Keep this branch explicit:
    // QJsonValue::toDouble() defaults missing fields to zero, which could turn
    // an unknown future shape into a plausible-looking line at the origin.
    const auto x = optionalDouble(median.value(QStringLiteral("x")));
    const auto top = optionalDouble(median.value(QStringLiteral("top")));
    const auto bottom = optionalDouble(median.value(QStringLiteral("bottom")));
    if (x && top && bottom) {
        return QLineF(*x, *top, *x, *bottom);
    }

    // BEAMS and LEDGERS report the two endpoints of their horizontal median.
    const auto x1 = optionalDouble(median.value(QStringLiteral("x1")));
    const auto y1 = optionalDouble(median.value(QStringLiteral("y1")));
    const auto x2 = optionalDouble(median.value(QStringLiteral("x2")));
    const auto y2 = optionalDouble(median.value(QStringLiteral("y2")));
    if (x1 && y1 && x2 && y2) {
        return QLineF(*x1, *y1, *x2, *y2);
    }

    return std::nullopt;
}

Inter parseRustInter(const QJsonObject &object)
{
    Inter inter;
    inter.id = object.value(QStringLiteral("id")).toInt();
    inter.kind = object.value(QStringLiteral("kind")).toString();
    inter.shape = inter.kind;
    inter.staff = object.value(QStringLiteral("staff")).toInt(-1);
    inter.grade = object.value(QStringLiteral("grade")).toDouble();
    inter.contextual = optionalDouble(object.value(QStringLiteral("contextual_grade")));

    const QJsonObject bounds = object.value(QStringLiteral("bounds")).toObject();
    if (!bounds.isEmpty()) {
        inter.bounds = parseBounds(bounds);
    }

    const QJsonObject median = object.value(QStringLiteral("median")).toObject();
    if (!median.isEmpty()) {
        inter.median = parseMedian(median);
    }

    const QJsonObject evidence = object.value(QStringLiteral("evidence")).toObject();
    inter.frozen = evidence.value(QStringLiteral("frozen")).toBool();
    const QJsonObject impacts = evidence.value(QStringLiteral("impacts")).toObject();
    for (auto it = impacts.begin(); it != impacts.end(); ++it) {
        inter.impacts << Impact{it.key(), it.value().toDouble()};
    }
    return inter;
}

Inter parseRustFinalHead(const QJsonObject &object)
{
    Inter head;
    // Final HEADS deliberately do not claim Java's sparse Inter ids.  Their
    // published source ordinal remains provenance, not a viewer-made id.
    head.kind = QStringLiteral("HEAD");
    head.shape = object.value(QStringLiteral("shape")).toString();
    head.staff = object.value(QStringLiteral("staff")).toInt(-1);
    head.grade = object.value(QStringLiteral("grade")).toDouble();
    const QJsonObject bounds = object.value(QStringLiteral("bounds")).toObject();
    if (!bounds.isEmpty()) {
        head.bounds = parseBounds(bounds);
    }
    return head;
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

std::optional<StreamEvent> parseStreamMarker(const QString &json, Engine expectedEngine,
                                             QString *error)
{
    if (error) {
        error->clear();
    }
    QJsonParseError parseError{};
    const QJsonDocument document = QJsonDocument::fromJson(json.toUtf8(), &parseError);
    if (document.isNull() || !document.isObject()) {
        if (error) {
            *error = QStringLiteral("unparsable stream marker: %1").arg(parseError.errorString());
        }
        return std::nullopt;
    }

    const QJsonObject object = document.object();
    const QJsonValue schema = object.contains(QStringLiteral("schema"))
        ? object.value(QStringLiteral("schema"))
        : object.value(QStringLiteral("stream_schema"));
    if (!schema.isDouble() || schema.toInt() != 1) {
        if (error) {
            *error = QStringLiteral("unsupported stream marker schema");
        }
        return std::nullopt;
    }
    const QString expected = expectedEngine == Engine::Rust
        ? QStringLiteral("rust") : QStringLiteral("java");
    if (object.value(QStringLiteral("engine")).toString() != expected) {
        if (error) {
            *error = QStringLiteral("stream marker belongs to a different engine");
        }
        return std::nullopt;
    }
    StreamEvent marker;
    marker.event = object.value(QStringLiteral("event")).toString();
    marker.stage = object.value(QStringLiteral("stage")).toString();
    marker.sequence = object.value(QStringLiteral("sequence")).toInt(-1);
    if (object.value(QStringLiteral("elapsed_ms")).isDouble()) {
        marker.elapsedMillis = object.value(QStringLiteral("elapsed_ms")).toDouble();
    }
    marker.message = object.value(QStringLiteral("message")).toString();
    if (marker.message.isEmpty()) {
        marker.message = object.value(QStringLiteral("error")).toString();
    }
    if (object.value(QStringLiteral("success")).isBool()) {
        marker.success = object.value(QStringLiteral("success")).toBool();
    }
    const bool hasStage = marker.event == QLatin1String("stage_started")
        || marker.event == QLatin1String("stage_completed")
        || marker.event == QLatin1String("stage_failed");
    if (marker.event.isEmpty() || (hasStage && marker.stage.isEmpty())) {
        if (error) {
            *error = QStringLiteral("stream marker is missing its event or stage");
        }
        return std::nullopt;
    }
    return marker;
}

QVector<QString> takeCompleteLines(QByteArray &pending)
{
    QVector<QString> lines;
    for (;;) {
        const qsizetype newline = pending.indexOf('\n');
        if (newline < 0) {
            return lines;
        }
        lines << QString::fromUtf8(pending.left(newline));
        pending.remove(0, newline + 1);
    }
}

std::optional<QPair<double, double>> Staff::verticalExtent() const
{
    if (lines.isEmpty()) {
        return std::nullopt;
    }
    double top = std::numeric_limits<double>::max();
    double bottom = std::numeric_limits<double>::lowest();
    for (const QPolygonF &line : lines) {
        for (const QPointF &point : line) {
            top = std::min(top, point.y());
            bottom = std::max(bottom, point.y());
        }
    }
    if (top > bottom) {
        return std::nullopt;
    }
    return QPair<double, double>{top, bottom};
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
    result.declaredStage = root.value(QStringLiteral("producer"))
                               .toObject()
                               .value(QStringLiteral("stage"))
                               .toString();
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
        result.inters << parseRustInter(value.toObject());
    }

    // STEM_SEEDS creates accepted free glyphs rather than SIG inters, so the
    // schema keeps them in a stage-owned top-level array. Adapt them into the
    // viewer's common geometry record without inventing an id. Only accepted
    // records are displayable products; a future rejected-candidate array is
    // a separate concern.
    for (const QJsonValue &value : root.value(QStringLiteral("stem_seeds")).toArray()) {
        const QJsonObject object = value.toObject();
        if (object.value(QStringLiteral("status")).toString() != QLatin1String("accepted")) {
            continue;
        }
        result.inters << parseRustInter(object);
    }

    // HEADS publish final, identity-free head products under a stage-owned
    // systems array rather than pretending to have allocated Java SIG ids.
    // They are still ordinary displayable/pairable interpretations.
    const QJsonObject heads = root.value(QStringLiteral("heads")).toObject();
    for (const QJsonValue &systemValue : heads.value(QStringLiteral("systems")).toArray()) {
        const QJsonObject system = systemValue.toObject();
        for (const QJsonValue &headValue : system.value(QStringLiteral("final_heads")).toArray()) {
            const Inter head = parseRustFinalHead(headValue.toObject());
            if (head.bounds) {
                result.inters << head;
            }
        }
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
    bool sawSheet = false;

    for (const QString &line : text.split(QLatin1Char('\n'))) {
        const QStringList fields = line.trimmed().split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (fields.isEmpty()) {
            continue;
        }
        const QString &record = fields.first();

        if (record == QLatin1String("sheet") && fields.size() >= 3) {
            sawSheet = true;
            result.declaredStage = fields[2];
            continue;
        }
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

    // A Gradle banner or an accidental marker-only frame is not a recognition
    // result. SigProbe's first record is its explicit sheet/stage identity, so
    // require it before a streamed completion can become a green snapshot.
    result.ran = result.error.isEmpty() && sawSheet;
    if (!result.ran && result.error.isEmpty()) {
        result.error = QStringLiteral("no sheet record");
    }
    return result;
}

QVector<Pairing> pair(const EngineResult &rust, const EngineResult &java, const QString &kindFilter)
{
    // Paired by staff and abscissa rather than by id: the two engines number
    // their inters independently, and an id match would be a coincidence.
    auto abscissa = [](const Inter &inter) {
        if (inter.median) {
            return inter.median->center().x();
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
