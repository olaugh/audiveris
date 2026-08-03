// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact headless boundary for Java `LinesRetriever.addShortSections`.
//!
//! The long horizontal sections already registered in the lag remain intact.
//! A second `SectionFactory` pass registers sections from the saved short-run
//! table with `JunctionRatioPolicy.DEFAULT`, includes those runs in the lag's
//! existing run table, and only then invokes the horizontal VIP-section hook.

use std::{error::Error, fmt};

use crate::run_table::{Orientation, RunTable, RunTableError};
use crate::section::{JunctionPolicy, Section, build_sections_from_id};

/// Boundary for `LagManager.setVipSections(HORIZONTAL)`.
///
/// Production UI integration can implement this hook using configured VIP IDs.
/// The headless recognizer deliberately uses [`NoopVipSectionHook`].
pub trait VipSectionHook {
    fn set_horizontal_vip_sections(&mut self, sections: &[Section], run_table: &RunTable);
}

/// Headless equivalent of the optional debug-only VIP assignment.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopVipSectionHook;

impl VipSectionHook for NoopVipSectionHook {
    fn set_horizontal_vip_sections(&mut self, _sections: &[Section], _run_table: &RunTable) {}
}

/// Horizontal lag state needed by the two `SectionFactory` passes.
#[derive(Debug, Clone, PartialEq)]
pub struct HorizontalSectionLag {
    run_table: RunTable,
    sections: Vec<Section>,
    last_registered_id: usize,
}

impl HorizontalSectionLag {
    /// Mirror `LagManager.buildHorizontalLag` for the initial long-run table.
    pub fn from_long_runs(long_runs: RunTable) -> Result<Self, AddShortSectionsError> {
        require_horizontal(&long_runs)?;
        let sections = build_sections_from_id(&long_runs, JunctionPolicy::DEFAULT_RATIO, 1);
        let last_registered_id = sections.last().map_or(0, Section::id);
        Ok(Self {
            run_table: long_runs,
            sections,
            last_registered_id,
        })
    }

    #[must_use]
    pub const fn run_table(&self) -> &RunTable {
        &self.run_table
    }

    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    #[must_use]
    pub const fn last_registered_id(&self) -> usize {
        self.last_registered_id
    }

    /// Mirror `LinesRetriever.addShortSections` in its observable order.
    pub fn add_short_sections(
        &mut self,
        short_runs: &RunTable,
        vip_hook: &mut impl VipSectionHook,
    ) -> Result<&[Section], AddShortSectionsError> {
        require_horizontal(short_runs)?;
        if self.run_table.width() != short_runs.width()
            || self.run_table.height() != short_runs.height()
        {
            return Err(AddShortSectionsError::IncompatibleRunTable);
        }

        // SectionFactory(hLag, JunctionRatioPolicy.DEFAULT).createSections(..., true)
        let created = build_sections_from_id(
            short_runs,
            JunctionPolicy::DEFAULT_RATIO,
            self.last_registered_id + 1,
        );
        let created_count = created.len();
        self.sections.extend(created);
        self.last_registered_id += created_count;
        self.run_table.include(short_runs)?;

        // sheet.getLagManager().setVipSections(HORIZONTAL)
        vip_hook.set_horizontal_vip_sections(&self.sections, &self.run_table);
        Ok(&self.sections[self.sections.len() - created_count..])
    }
}

fn require_horizontal(table: &RunTable) -> Result<(), AddShortSectionsError> {
    if table.orientation() == Orientation::Horizontal {
        Ok(())
    } else {
        Err(AddShortSectionsError::NotHorizontal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddShortSectionsError {
    NotHorizontal,
    IncompatibleRunTable,
    RunTable(RunTableError),
}

impl From<RunTableError> for AddShortSectionsError {
    fn from(value: RunTableError) -> Self {
        Self::RunTable(value)
    }
}

impl fmt::Display for AddShortSectionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotHorizontal => write!(formatter, "section lag requires horizontal runs"),
            Self::IncompatibleRunTable => write!(formatter, "short-run table dimensions differ"),
            Self::RunTable(error) => write!(formatter, "run-table inclusion failed: {error}"),
        }
    }
}

impl Error for AddShortSectionsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::Run;

    fn table(runs: &[(usize, Run)]) -> RunTable {
        let mut table = RunTable::new(Orientation::Horizontal, 20, 5).unwrap();
        for &(position, run) in runs {
            assert!(table.add_run(position, run).unwrap());
        }
        table
    }

    #[derive(Default)]
    struct RecordingVipHook {
        calls: Vec<Vec<usize>>,
        run_counts: Vec<usize>,
    }

    impl VipSectionHook for RecordingVipHook {
        fn set_horizontal_vip_sections(&mut self, sections: &[Section], run_table: &RunTable) {
            self.calls
                .push(sections.iter().map(Section::id).collect::<Vec<_>>());
            self.run_counts.push(run_table.total_run_count());
        }
    }

    #[test]
    fn appends_short_sections_and_runs_without_replacing_long_state() {
        let long = table(&[(1, Run::new(1, 8)), (2, Run::new(1, 8))]);
        let short = table(&[(0, Run::new(12, 2)), (3, Run::new(14, 2))]);
        let mut lag = HorizontalSectionLag::from_long_runs(long.clone()).unwrap();
        let original_sections = lag.sections().to_vec();
        let mut hook = RecordingVipHook::default();

        let created = lag.add_short_sections(&short, &mut hook).unwrap();
        assert_eq!(created.iter().map(Section::id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(
            &lag.sections()[..original_sections.len()],
            original_sections
        );
        assert_eq!(lag.last_registered_id(), 3);

        let mut expected_runs = long;
        expected_runs.include(&short).unwrap();
        assert_eq!(lag.run_table(), &expected_runs);
        assert_eq!(hook.calls, [vec![1, 2, 3]]);
        assert_eq!(hook.run_counts, [4]);
    }

    #[test]
    fn default_ratio_policy_controls_creation_and_ids_follow_factory_order() {
        let long = table(&[(4, Run::new(0, 10))]);
        let short = table(&[
            (0, Run::new(8, 4)),
            // Ratio 6/4 exceeds DEFAULT 1.25, so this starts a new section.
            (1, Run::new(8, 6)),
            // Sorted coordinate order, regardless of insertion order.
            (3, Run::new(15, 1)),
            (3, Run::new(2, 1)),
        ]);
        let mut lag = HorizontalSectionLag::from_long_runs(long).unwrap();
        let mut hook = NoopVipSectionHook;

        let created = lag.add_short_sections(&short, &mut hook).unwrap();
        assert_eq!(
            created.iter().map(Section::id).collect::<Vec<_>>(),
            [2, 3, 4, 5]
        );
        assert_eq!(created[0].runs(), &[Run::new(8, 4)]);
        assert_eq!(created[1].runs(), &[Run::new(8, 6)]);
        assert_eq!(created[2].runs(), &[Run::new(2, 1)]);
        assert_eq!(created[3].runs(), &[Run::new(15, 1)]);
    }

    #[test]
    fn vip_hook_runs_after_registration_and_empty_pass_still_calls_it() {
        let long = table(&[(2, Run::new(3, 8))]);
        let empty = table(&[]);
        let mut lag = HorizontalSectionLag::from_long_runs(long).unwrap();
        let mut hook = RecordingVipHook::default();

        let created = lag.add_short_sections(&empty, &mut hook).unwrap();
        assert!(created.is_empty());
        assert_eq!(hook.calls, [vec![1]]);
        assert_eq!(hook.run_counts, [1]);
        assert_eq!(lag.last_registered_id(), 1);
    }
}
