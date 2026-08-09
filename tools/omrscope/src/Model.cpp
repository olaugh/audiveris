// SPDX-License-Identifier: AGPL-3.0-or-later
#include "Model.h"

#include <QHash>
#include <cmath>

namespace omrscope {

namespace {

QString interKey(int system, int id)
{
    return QStringLiteral("%1:%2").arg(system).arg(id);
}

bool finite(const QPointF &point)
{
    return std::isfinite(point.x()) && std::isfinite(point.y());
}

} // namespace

std::optional<QPointF> interCenter(const Inter &inter)
{
    if (inter.median) {
        const QPointF point = inter.median->center();
        return finite(point) ? std::optional<QPointF>(point) : std::nullopt;
    }
    if (inter.bounds) {
        const QPointF point = inter.bounds->center();
        return finite(point) ? std::optional<QPointF>(point) : std::nullopt;
    }
    return std::nullopt;
}

QVector<RelationLine> resolvedRelationLines(const EngineResult &result)
{
    // An inter id is only unique inside a SIG/system. Keep the collection
    // engine-local too: Rust and Java use independent id allocators.
    QHash<QString, QPointF> uniqueCenters;
    QHash<QString, int> occurrences;
    for (const Inter &inter : result.inters) {
        if (inter.system < 0 || inter.id < 0) {
            continue;
        }
        const QString key = interKey(inter.system, inter.id);
        ++occurrences[key];
        const auto center = interCenter(inter);
        if (!center) {
            continue;
        }
        uniqueCenters.insert(key, *center);
    }

    QVector<RelationLine> lines;
    for (const Relation &relation : result.relations) {
        if (relation.system < 0 || relation.sourceId < 0 || relation.targetId < 0
            || relation.sourceId == relation.targetId || relation.kind.isEmpty()) {
            continue;
        }
        const QString sourceKey = interKey(relation.system, relation.sourceId);
        const QString targetKey = interKey(relation.system, relation.targetId);
        if (occurrences.value(sourceKey) != 1 || occurrences.value(targetKey) != 1
            || !uniqueCenters.contains(sourceKey) || !uniqueCenters.contains(targetKey)) {
            continue;
        }
        const QLineF line(uniqueCenters.value(sourceKey), uniqueCenters.value(targetKey));
        if (line.length() <= 0.0 || !finite(line.p1()) || !finite(line.p2())) {
            continue;
        }
        lines << RelationLine{line, relation.kind};
    }
    return lines;
}

} // namespace omrscope
