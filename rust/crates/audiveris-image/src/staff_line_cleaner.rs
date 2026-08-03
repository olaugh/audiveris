// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-order headless boundary for Java `StaffLineCleaner.process`.

/// Original detailed staff line returned by Java `Staff.simplifyLines`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalStaffLine {
    pub line_id: usize,
    pub section_ids: Vec<usize>,
}

/// Adapter over sheet/staff/lag/system ownership mutated by the cleaner.
pub trait StaffLineCleanerExecutor {
    type Error;

    /// Staff identities in `StaffManager.getStaves()` order.
    fn staff_ids(&self) -> Vec<usize>;

    /// Replace one staff's detailed filaments with simplified line data and
    /// return the detailed originals in staff line order.
    fn simplify_staff_lines(
        &mut self,
        staff_id: usize,
    ) -> Result<Vec<OriginalStaffLine>, Self::Error>;

    /// Remove one original line's members from the global horizontal lag.
    fn remove_horizontal_sections(&mut self, section_ids: &[usize]) -> Result<(), Self::Error>;

    /// Rebuild the horizontal lag from the staff-free raster.
    fn rebuild_horizontal_lag(&mut self) -> Result<(), Self::Error>;

    /// Dispatch rebuilt vertical/horizontal sections into their systems.
    fn populate_systems(&mut self) -> Result<(), Self::Error>;

    /// Java prints its optional stopwatch only after successful completion.
    fn finish_successfully(&mut self);
}

/// Execute Java `StaffLineCleaner.process` exactly in non-UI order.
///
/// Any error propagates immediately. Earlier staff simplification and lag
/// removals remain applied; rebuild, population, and success finalization are
/// skipped when an earlier operation fails.
pub fn clean_staff_lines<Executor>(executor: &mut Executor) -> Result<(), Executor::Error>
where
    Executor: StaffLineCleanerExecutor,
{
    for staff_id in executor.staff_ids() {
        let originals = executor.simplify_staff_lines(staff_id)?;
        for original in originals {
            executor.remove_horizontal_sections(&original.section_ids)?;
        }
    }
    executor.rebuild_horizontal_lag()?;
    executor.populate_systems()?;
    executor.finish_successfully();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Simplify(usize),
        Remove(Vec<usize>),
        Rebuild,
        Populate,
        Finish,
    }

    #[derive(Debug, Default)]
    struct RecordingCleaner {
        calls: Vec<Call>,
        fail_remove: Option<Vec<usize>>,
    }

    impl StaffLineCleanerExecutor for RecordingCleaner {
        type Error = &'static str;

        fn staff_ids(&self) -> Vec<usize> {
            vec![3, 7]
        }

        fn simplify_staff_lines(
            &mut self,
            staff_id: usize,
        ) -> Result<Vec<OriginalStaffLine>, Self::Error> {
            self.calls.push(Call::Simplify(staff_id));
            Ok(match staff_id {
                3 => vec![
                    OriginalStaffLine {
                        line_id: 30,
                        section_ids: vec![1, 4],
                    },
                    OriginalStaffLine {
                        line_id: 31,
                        section_ids: vec![5],
                    },
                ],
                7 => vec![OriginalStaffLine {
                    line_id: 70,
                    section_ids: vec![8, 9],
                }],
                _ => unreachable!(),
            })
        }

        fn remove_horizontal_sections(&mut self, section_ids: &[usize]) -> Result<(), Self::Error> {
            self.calls.push(Call::Remove(section_ids.to_vec()));
            if self.fail_remove.as_deref() == Some(section_ids) {
                Err("remove failed")
            } else {
                Ok(())
            }
        }

        fn rebuild_horizontal_lag(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Rebuild);
            Ok(())
        }

        fn populate_systems(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Populate);
            Ok(())
        }

        fn finish_successfully(&mut self) {
            self.calls.push(Call::Finish);
        }
    }

    #[test]
    fn cleaner_simplifies_and_removes_per_staff_before_rebuild_and_population() {
        let mut cleaner = RecordingCleaner::default();
        assert_eq!(clean_staff_lines(&mut cleaner), Ok(()));
        assert_eq!(
            cleaner.calls,
            [
                Call::Simplify(3),
                Call::Remove(vec![1, 4]),
                Call::Remove(vec![5]),
                Call::Simplify(7),
                Call::Remove(vec![8, 9]),
                Call::Rebuild,
                Call::Populate,
                Call::Finish,
            ]
        );
    }

    #[test]
    fn cleaner_failure_retains_prior_mutation_and_skips_downstream_work() {
        let mut cleaner = RecordingCleaner {
            fail_remove: Some(vec![5]),
            ..RecordingCleaner::default()
        };
        assert_eq!(clean_staff_lines(&mut cleaner), Err("remove failed"));
        assert_eq!(
            cleaner.calls,
            [
                Call::Simplify(3),
                Call::Remove(vec![1, 4]),
                Call::Remove(vec![5]),
            ]
        );
    }
}
