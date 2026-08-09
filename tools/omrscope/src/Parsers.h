// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QByteArray>
#include <QSize>
#include <QString>
#include <QVector>

namespace omrscope {

/// Parses the Rust CLI's `-json` output. One document per sheet, per line.
EngineResult parseRustJson(const QString &text);

/// Parses `SigProbe`'s line-oriented records.
EngineResult parseSigProbe(const QString &text);

/// Parses the JSON after an `@omrscope ` marker. The marker frames the
/// unchanged per-stage payload, so only marker lines go through this parser.
/// Both the early `stream_schema` spelling and the final `schema` spelling are
/// accepted while producers are upgraded together.
std::optional<StreamEvent> parseStreamMarker(const QString &json, Engine expectedEngine,
                                             QString *error = nullptr);

/// Pull complete UTF-8 lines from a chunked QProcess stream. The remaining
/// unterminated suffix stays in `pending` for the next readyRead callback.
QVector<QString> takeCompleteLines(QByteArray &pending);

/// Matches the two engines' inters by staff and abscissa.
///
/// Not by id: they number independently, so an id match would be coincidence.
/// `kindFilter` is a case-insensitive substring, empty for everything.
QVector<Pairing> pair(const EngineResult &rust, const EngineResult &java,
                      const QString &kindFilter);

/// A missing optional reads as an em dash, never as zero.
QString describe(const std::optional<double> &value);

} // namespace omrscope
