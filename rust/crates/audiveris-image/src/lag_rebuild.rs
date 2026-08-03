// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless lifecycle parity for Java `LagManager.rebuildHLag`.
//!
//! The no-staff raster is optional.  Once it exists, Java dispatches its runs,
//! requires the previously registered horizontal lag, resets that same lag,
//! and repopulates it.  A missing scale still resets the lag but leaves it
//! empty because `dispatchRuns`/`buildHorizontalLag` return `null`.

use std::{error::Error, fmt};

use crate::{
    line_short_sections::{AddShortSectionsError, HorizontalSectionLag, VipSectionHook},
    run_table::{RunTable, RunTableError, dispatch_grid_runs},
};

/// Registered horizontal-lag contents.  `Empty` is distinct from an absent
/// registration and mirrors `BasicLag.reset()` with a null run table.
#[derive(Debug, Clone, PartialEq)]
pub enum RegisteredHorizontalLag {
    Empty,
    Populated(HorizontalSectionLag),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildHorizontalLagOutcome {
    NoStaffTable,
    ResetWithoutScale,
    Rebuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildHorizontalLagError {
    Dispatch(RunTableError),
    MissingRegisteredLag,
    Build(AddShortSectionsError),
}

impl fmt::Display for RebuildHorizontalLagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch(error) => write!(formatter, "no-staff run dispatch failed: {error}"),
            Self::MissingRegisteredLag => write!(formatter, "horizontal lag is not registered"),
            Self::Build(error) => write!(formatter, "horizontal lag rebuild failed: {error}"),
        }
    }
}

impl Error for RebuildHorizontalLagError {}

/// Rebuild a previously registered horizontal lag from a vertical no-staff
/// run table, preserving Java's observable ordering and partial mutation.
pub fn rebuild_horizontal_lag(
    no_staff_table: Option<&RunTable>,
    max_fore: Option<usize>,
    ledger_thickness: f64,
    registered: &mut Option<RegisteredHorizontalLag>,
    vip_hook: &mut impl VipSectionHook,
) -> Result<RebuildHorizontalLagOutcome, RebuildHorizontalLagError> {
    let Some(source) = no_staff_table else {
        return Ok(RebuildHorizontalLagOutcome::NoStaffTable);
    };

    // LagManager.dispatchRuns returns null when the sheet has no scale.  Java
    // nevertheless looks up and resets hLag before buildHorizontalLag(null).
    let horizontal = match max_fore {
        Some(max_fore) => Some(
            dispatch_grid_runs(source, max_fore, ledger_thickness)
                .map_err(RebuildHorizontalLagError::Dispatch)?
                .0,
        ),
        None => None,
    };

    let lag = registered
        .as_mut()
        .ok_or(RebuildHorizontalLagError::MissingRegisteredLag)?;
    *lag = RegisteredHorizontalLag::Empty;

    let Some(horizontal) = horizontal else {
        return Ok(RebuildHorizontalLagOutcome::ResetWithoutScale);
    };
    let rebuilt = HorizontalSectionLag::from_long_runs(horizontal)
        .map_err(RebuildHorizontalLagError::Build)?;
    vip_hook.set_horizontal_vip_sections(rebuilt.sections(), rebuilt.run_table());
    *lag = RegisteredHorizontalLag::Populated(rebuilt);
    Ok(RebuildHorizontalLagOutcome::Rebuilt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        line_short_sections::VipSectionHook,
        run_table::{Orientation, Run},
        section::Section,
    };

    fn vertical(runs: &[(usize, Run)]) -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 8, 8).unwrap();
        for &(position, run) in runs {
            assert!(table.add_run(position, run).unwrap());
        }
        table
    }

    fn populated() -> RegisteredHorizontalLag {
        let mut table = RunTable::new(Orientation::Horizontal, 8, 8).unwrap();
        assert!(table.add_run(2, Run::new(1, 5)).unwrap());
        RegisteredHorizontalLag::Populated(HorizontalSectionLag::from_long_runs(table).unwrap())
    }

    #[derive(Default)]
    struct RecordingVip {
        calls: Vec<(Vec<usize>, usize)>,
    }

    impl VipSectionHook for RecordingVip {
        fn set_horizontal_vip_sections(&mut self, sections: &[Section], table: &RunTable) {
            self.calls.push((
                sections.iter().map(Section::id).collect(),
                table.total_run_count(),
            ));
        }
    }

    #[test]
    fn missing_no_staff_table_is_an_exact_no_op() {
        let original = populated();
        let mut registered = Some(original.clone());
        let mut vip = RecordingVip::default();
        assert_eq!(
            rebuild_horizontal_lag(None, Some(2), 1.0, &mut registered, &mut vip),
            Ok(RebuildHorizontalLagOutcome::NoStaffTable)
        );
        assert_eq!(registered, Some(original));
        assert!(vip.calls.is_empty());
    }

    #[test]
    fn missing_scale_resets_registered_lag_without_repopulation_or_vip() {
        let source = vertical(&[(1, Run::new(1, 2))]);
        let mut registered = Some(populated());
        let mut vip = RecordingVip::default();
        assert_eq!(
            rebuild_horizontal_lag(Some(&source), None, 1.0, &mut registered, &mut vip),
            Ok(RebuildHorizontalLagOutcome::ResetWithoutScale)
        );
        assert_eq!(registered, Some(RegisteredHorizontalLag::Empty));
        assert!(vip.calls.is_empty());
    }

    #[test]
    fn rebuild_replaces_old_state_restarts_ids_and_invokes_vip_last() {
        let source = vertical(&[
            (1, Run::new(1, 2)),
            (2, Run::new(1, 2)),
            (4, Run::new(5, 1)),
        ]);
        let mut registered = Some(populated());
        let mut vip = RecordingVip::default();
        assert_eq!(
            rebuild_horizontal_lag(Some(&source), Some(3), 1.0, &mut registered, &mut vip),
            Ok(RebuildHorizontalLagOutcome::Rebuilt)
        );
        let Some(RegisteredHorizontalLag::Populated(rebuilt)) = registered else {
            panic!("lag was not rebuilt")
        };
        assert_eq!(rebuilt.sections().first().map(Section::id), Some(1));
        assert_eq!(vip.calls.len(), 1);
        assert_eq!(
            vip.calls[0].0,
            rebuilt
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>()
        );
        assert_eq!(vip.calls[0].1, rebuilt.run_table().total_run_count());
    }

    #[test]
    fn dispatch_failure_precedes_lookup_and_preserves_existing_state() {
        let horizontal = RunTable::new(Orientation::Horizontal, 8, 8).unwrap();
        let original = populated();
        let mut registered = Some(original.clone());
        let mut vip = RecordingVip::default();
        assert_eq!(
            rebuild_horizontal_lag(Some(&horizontal), Some(2), 1.0, &mut registered, &mut vip),
            Err(RebuildHorizontalLagError::Dispatch(
                RunTableError::IncompatibleTable
            ))
        );
        assert_eq!(registered, Some(original));
        assert!(vip.calls.is_empty());
    }

    #[test]
    fn existing_source_requires_a_registered_horizontal_lag() {
        let source = vertical(&[(1, Run::new(1, 2))]);
        let mut registered = None;
        let mut vip = RecordingVip::default();
        assert_eq!(
            rebuild_horizontal_lag(Some(&source), Some(2), 1.0, &mut registered, &mut vip),
            Err(RebuildHorizontalLagError::MissingRegisteredLag)
        );
        assert!(vip.calls.is_empty());
    }
}
