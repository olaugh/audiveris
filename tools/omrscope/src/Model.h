// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <QLineF>
#include <QPair>
#include <QPointF>
#include <QPolygonF>
#include <QRectF>
#include <QString>
#include <QVector>
#include <optional>

namespace omrscope {

/// One term of a grade.
///
/// A grade is a weighted geometric mean, so the product alone says nothing
/// about *why* it came out where it did. Both engines report the terms; this
/// tool exists largely to put them side by side.
struct Impact
{
    QString name;
    double value = 0.0;
};

/// One interpretation, in whichever engine produced it.
///
/// Deliberately the intersection of what the two emit rather than a union of
/// their internals: anything only one side can say cannot be compared, and a
/// field that is present-but-meaningless on one side is worse than absent.
struct Inter
{
    /// `id` is scoped to a system. -1 means the published product is
    /// deliberately identity-free (for example final HEADS), not inter #0.
    int id = -1;
    int system = -1;
    QString kind;                   ///< BarlineInter, THIN_BARLINE, BEAM...
    QString shape;
    int staff = -1;
    std::optional<QRectF> bounds;
    /// Rust geometry. GRID uses a vertical x/top/bottom line; STEM_SEEDS,
    /// BEAMS, and LEDGERS use x1/y1/x2/y2 endpoints. Java's is in bounds.
    std::optional<QLineF> median;
    double grade = 0.0;
    std::optional<double> contextual;
    bool frozen = false;
    QVector<Impact> impacts;
    bool rejected = false;          ///< A candidate that lost.
    QString rejectedBy;             ///< The purge that dropped it.
};

/// A directed SIG edge as one engine reported it. IDs are intentionally kept
/// engine-local: a Rust id and a Java id do not identify the same inter.
struct Relation
{
    int system = -1;
    int sourceId = -1;
    int targetId = -1;
    QString kind;
};

/// A relation whose two reported endpoint IDs resolve to one drawable inter
/// each in the same result snapshot. Ambiguous and geometry-free endpoints
/// are deliberately absent rather than guessed from nearby marks.
struct RelationLine
{
    QLineF line;
    QString kind;
};

struct Staff
{
    int id = 0;
    double left = 0.0;
    double right = 0.0;
    int lineCount = 0;
    QVector<QPolygonF> lines;       ///< Empty until lines are persistent.

    /// The staff's vertical extent, or nothing if its lines are still
    /// filaments. A rejected candidate has no ordinates of its own -- it is a
    /// span and a staff id -- so this is what gives it somewhere to be drawn.
    std::optional<QPair<double, double>> verticalExtent() const;
};

/// What one engine made of one sheet.
struct EngineResult
{
    QString engine;                 ///< "rust" or "java"
    QString declaredStage;          ///< Stage named by the payload itself.
    bool ran = false;
    QString error;

    /// Milliseconds of *recognition*, not of process lifetime.
    ///
    /// Java is measured inside the probe because Gradle and JVM startup dwarf
    /// the work; Rust is process wall clock because its startup is noise. The
    /// UI says which is which rather than pretending they are the same
    /// measurement.
    double millis = 0.0;
    QString timingNote;

    QSize image;
    QVector<Staff> staves;
    QVector<Inter> inters;
    QVector<Relation> relations;
    int relationCount = 0;
    QString raw;                    ///< Verbatim output, for the log tab.
};

/// The producer of one stream of stage snapshots.
enum class Engine
{
    Rust,
    Java,
};

/// A stage is kept after it reaches a terminal state.  That is intentional:
/// losing GRID merely because a later HEADS attempt fails is the opposite of
/// useful when the window is being used to diagnose a port.
enum class StageState
{
    NotRequested,
    Queued,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
};

/// A marker emitted by a streaming engine process. The recognition payload is
/// deliberately not nested in the marker: it is the existing byte-stable
/// stage report framed between `stage_started` and `stage_completed` markers.
struct StreamEvent
{
    QString event;
    QString stage;
    int sequence = -1;
    std::optional<double> elapsedMillis;
    std::optional<bool> success;
    QString message;
};

/// One immutable observation of one engine at one stage in one attempt.
struct StageSnapshot
{
    StageState state = StageState::Queued;
    EngineResult result;
    QString message;
    int sequence = -1;
};

/// One row of the comparison: the same thing as each engine saw it.
struct Pairing
{
    std::optional<Inter> rust;
    std::optional<Inter> java;
    QString note;                   ///< Why they were paired, or why not.

    bool agrees() const;
};

/// The location that a concrete inter reports for an engine-local graph edge.
/// A candidate whose span has no ordinate has no relation endpoint geometry.
std::optional<QPointF> interCenter(const Inter &inter);

/// Resolve only unambiguous, engine-local relation endpoint ids. This is the
/// model boundary for the Page graph overlay: it cannot accidentally make a
/// cross-engine edge or infer an endpoint by spatial pairing.
QVector<RelationLine> resolvedRelationLines(const EngineResult &result);

} // namespace omrscope
