// SPDX-License-Identifier: AGPL-3.0-or-later

//! Position-indexed access to sections sorted by their first run sequence.

use std::{error::Error, fmt};

use crate::section::Section;

/// Source-guided neutral form of Java `SectionTally`.
#[derive(Debug)]
pub struct SectionTally<'a> {
    sections: &'a [Section],
    starts: Vec<Option<usize>>,
}

impl<'a> SectionTally<'a> {
    /// Index a list ordered by nondecreasing [`Section::first_pos`].
    pub fn new(pos_count: usize, sections: &'a [Section]) -> Result<Self, SectionTallyError> {
        let mut starts = vec![None; pos_count + 1];
        let mut previous = None;
        for (index, section) in sections.iter().enumerate() {
            let position = section.first_pos();
            if position >= pos_count {
                return Err(SectionTallyError::PositionOutOfRange {
                    position,
                    pos_count,
                });
            }
            if previous.is_some_and(|previous| position < previous) {
                return Err(SectionTallyError::Unsorted);
            }
            if previous != Some(position) {
                starts[position] = Some(index);
                previous = Some(position);
            }
        }
        starts[pos_count] = Some(sections.len());
        Ok(Self { sections, starts })
    }

    /// Sections beginning at `position`, in their original stable order.
    pub fn at(&self, position: usize) -> Result<&'a [Section], SectionTallyError> {
        if position + 1 >= self.starts.len() {
            return Err(SectionTallyError::PositionOutOfRange {
                position,
                pos_count: self.starts.len() - 1,
            });
        }
        let Some(start) = self.starts[position] else {
            return Ok(&self.sections[0..0]);
        };
        let stop = self.starts[position + 1..]
            .iter()
            .flatten()
            .copied()
            .next()
            .expect("the tally stores a terminal marker");
        Ok(&self.sections[start..stop])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionTallyError {
    PositionOutOfRange { position: usize, pos_count: usize },
    Unsorted,
}

impl fmt::Display for SectionTallyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PositionOutOfRange {
                position,
                pos_count,
            } => write!(
                formatter,
                "section position {position} is outside 0..{pos_count}"
            ),
            Self::Unsorted => formatter.write_str("sections are not ordered by first position"),
        }
    }
}

impl Error for SectionTallyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn sections() -> Vec<Section> {
        let mut table = RunTable::new(Orientation::Horizontal, 12, 7).unwrap();
        table.add_run(1, Run::new(1, 2)).unwrap();
        table.add_run(1, Run::new(6, 2)).unwrap();
        table.add_run(4, Run::new(2, 2)).unwrap();
        table.add_run(6, Run::new(3, 2)).unwrap();
        build_sections(&table, JunctionPolicy::All)
    }

    #[test]
    fn returns_stable_position_slices_and_empty_gaps() {
        let sections = sections();
        let tally = SectionTally::new(7, &sections).unwrap();

        assert_eq!(
            tally
                .at(1)
                .unwrap()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(tally.at(2).unwrap().is_empty());
        assert_eq!(tally.at(4).unwrap()[0].id(), 3);
        assert_eq!(tally.at(6).unwrap()[0].id(), 4);
    }

    #[test]
    fn rejects_invalid_positions_and_unsorted_inputs() {
        let sections = sections();
        let tally = SectionTally::new(7, &sections).unwrap();
        assert_eq!(
            tally.at(7),
            Err(SectionTallyError::PositionOutOfRange {
                position: 7,
                pos_count: 7,
            })
        );
        assert_eq!(
            SectionTally::new(6, &sections).unwrap_err(),
            SectionTallyError::PositionOutOfRange {
                position: 6,
                pos_count: 6,
            }
        );

        let mut unsorted = sections;
        unsorted.swap(0, 2);
        assert_eq!(
            SectionTally::new(7, &unsorted).unwrap_err(),
            SectionTallyError::Unsorted
        );
    }
}
