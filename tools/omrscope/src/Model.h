// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <QLineF>
#include <QPair>
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
    int id = 0;
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
    int relationCount = 0;
    QString raw;                    ///< Verbatim output, for the log tab.
};

/// One row of the comparison: the same thing as each engine saw it.
struct Pairing
{
    std::optional<Inter> rust;
    std::optional<Inter> java;
    QString note;                   ///< Why they were paired, or why not.

    bool agrees() const;
};

} // namespace omrscope
