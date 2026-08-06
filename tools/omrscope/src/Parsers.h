// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QSize>
#include <QString>
#include <QVector>

namespace omrscope {

/// Parses the Rust CLI's `-json` output. One document per sheet, per line.
EngineResult parseRustJson(const QString &text);

/// Parses `SigProbe`'s line-oriented records.
EngineResult parseSigProbe(const QString &text);

/// Matches the two engines' inters by staff and abscissa.
///
/// Not by id: they number independently, so an id match would be coincidence.
/// `kindFilter` is a case-insensitive substring, empty for everything.
QVector<Pairing> pair(const EngineResult &rust, const EngineResult &java,
                      const QString &kindFilter);

/// A missing optional reads as an em dash, never as zero.
QString describe(const std::optional<double> &value);

} // namespace omrscope
