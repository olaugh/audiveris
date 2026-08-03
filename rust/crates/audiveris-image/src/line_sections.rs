// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light staff-line section selection from Java
//! `LinesRetriever.getAllStickers`.
//!
//! This slice begins after staff-line ownership is known. Excluded members are
//! identified by section ID; IDs must therefore be unique within `source`.
//! Glyph ownership, staff traversal, and later sticker inclusion remain with
//! the caller.

use std::collections::HashSet;
use std::{error::Error, fmt};

use crate::run_table::Orientation;
use crate::section::Section;
use crate::section_tally::{SectionTally, SectionTallyError};

/// Select one-run horizontal sections whose cumulative connection above and
/// below does not exceed `max_connection_length`.
///
/// Exclusion happens before connection measurement, matching Java's removal of
/// already-owned staff-line members. The result is stably ordered by first
/// position, then start coordinate (`Section.byFullPosition`); exact ties keep
/// source order.
pub fn all_stickers(
    source: &[Section],
    excluded_ids: &[usize],
    sheet_height: usize,
    max_connection_length: usize,
) -> Result<Vec<Section>, LineSectionsError> {
    let mut ids = HashSet::with_capacity(source.len());
    for section in source {
        if section.orientation() != Orientation::Horizontal {
            return Err(LineSectionsError::UnsupportedOrientation { id: section.id() });
        }
        if !ids.insert(section.id()) {
            return Err(LineSectionsError::DuplicateSectionId(section.id()));
        }
    }

    let excluded: HashSet<usize> = excluded_ids.iter().copied().collect();
    let mut sections: Vec<Section> = source
        .iter()
        .filter(|section| !excluded.contains(&section.id()))
        .cloned()
        .collect();
    // `sort_by` is stable. Java's comparator deliberately has no ID fallback.
    sections.sort_by(|one, two| {
        one.first_pos()
            .cmp(&two.first_pos())
            .then_with(|| one.start_coord().cmp(&two.start_coord()))
    });

    let tally = SectionTally::new(sheet_height, &sections)?;
    let mut connected = HashSet::new();

    // Connections below use the source's last run and each successor's first.
    for section in &sections {
        let predecessor = section.last_run();
        let next_position = section.first_pos() + section.run_count();
        if next_position < sheet_height
            && touching_length(
                predecessor.start,
                predecessor.stop(),
                tally.at(next_position)?,
                Section::first_run,
            ) > max_connection_length
        {
            connected.insert(section.id());
        }
    }

    // Connections above use the source's first run and each predecessor's last.
    for section in &sections {
        let predecessor = section.first_run();
        if section.first_pos() > 0
            && touching_length(
                predecessor.start,
                predecessor.stop(),
                tally.at(section.first_pos() - 1)?,
                Section::last_run,
            ) > max_connection_length
        {
            connected.insert(section.id());
        }
    }

    Ok(sections
        .into_iter()
        .filter(|section| !connected.contains(&section.id()) && section.run_count() == 1)
        .collect())
}

fn touching_length(
    source_start: usize,
    source_stop: usize,
    candidates: &[Section],
    edge_run: fn(&Section) -> crate::run_table::Run,
) -> usize {
    let mut touching = 0;
    for candidate in candidates {
        let run = edge_run(candidate);
        if run.start > source_stop {
            break;
        }
        if run.stop() >= source_start {
            let common_start = source_start.max(run.start);
            let common_stop = source_stop.min(run.stop());
            touching += common_stop - common_start + 1;
        }
    }
    touching
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineSectionsError {
    DuplicateSectionId(usize),
    UnsupportedOrientation { id: usize },
    Tally(SectionTallyError),
}

impl From<SectionTallyError> for LineSectionsError {
    fn from(value: SectionTallyError) -> Self {
        Self::Tally(value)
    }
}

impl fmt::Display for LineSectionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSectionId(id) => write!(formatter, "duplicate section ID {id}"),
            Self::UnsupportedOrientation { id } => {
                write!(formatter, "section {id} is not horizontal")
            }
            Self::Tally(error) => write!(formatter, "section tally error: {error}"),
        }
    }
}

impl Error for LineSectionsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{Run, RunTable};
    use crate::section::{JunctionPolicy, build_sections};

    fn fixture() -> Vec<Section> {
        let mut table = RunTable::new(Orientation::Horizontal, 30, 8).unwrap();
        for (position, start, length) in [
            (1, 0, 6),
            (1, 10, 4),
            (2, 0, 2),
            (2, 4, 2),
            (2, 10, 2),
            (2, 13, 1),
            (4, 0, 2),
            (4, 4, 2),
            (5, 0, 6),
            (6, 20, 4),
            (7, 20, 4),
        ] {
            table.add_run(position, Run::new(start, length)).unwrap();
        }
        build_sections(&table, JunctionPolicy::All)
    }

    #[test]
    fn rejects_cumulative_connections_above_and_below_but_not_exact_limit() {
        let mut source = fixture();
        source.reverse();
        let stickers = all_stickers(&source, &[], 8, 3).unwrap();

        assert_eq!(
            stickers.iter().map(Section::id).collect::<Vec<_>>(),
            [2, 3, 4, 5, 6, 7, 8]
        );
        assert!(stickers.iter().all(|section| section.run_count() == 1));

        // Both four-pixel cumulative contacts are retained at the exact limit.
        let relaxed = all_stickers(&source, &[], 8, 4).unwrap();
        assert!(relaxed.iter().any(|section| section.id() == 1));
        assert!(relaxed.iter().any(|section| section.id() == 9));
        assert!(!relaxed.iter().any(|section| section.id() == 10));
    }

    #[test]
    fn exclusion_precedes_tally_and_can_remove_a_connection() {
        let source = fixture();
        let stickers = all_stickers(&source, &[4], 8, 3).unwrap();

        assert!(stickers.iter().any(|section| section.id() == 1));
        assert!(!stickers.iter().any(|section| section.id() == 4));
        assert!(
            stickers
                .windows(2)
                .all(|pair| (pair[0].first_pos(), pair[0].start_coord())
                    <= (pair[1].first_pos(), pair[1].start_coord()))
        );
    }

    #[test]
    fn rejects_duplicate_ids_vertical_input_and_out_of_sheet_positions() {
        let one = fixture().remove(0);
        assert_eq!(
            all_stickers(&[one.clone(), one], &[], 8, 3),
            Err(LineSectionsError::DuplicateSectionId(1))
        );

        let mut vertical = RunTable::new(Orientation::Vertical, 2, 4).unwrap();
        vertical.add_run(0, Run::new(0, 2)).unwrap();
        let vertical = build_sections(&vertical, JunctionPolicy::All);
        assert_eq!(
            all_stickers(&vertical, &[], 4, 3),
            Err(LineSectionsError::UnsupportedOrientation { id: 1 })
        );

        let source = fixture();
        assert_eq!(
            all_stickers(&source, &[], 6, 3),
            Err(LineSectionsError::Tally(
                SectionTallyError::PositionOutOfRange {
                    position: 6,
                    pos_count: 6,
                }
            ))
        );
    }
}
