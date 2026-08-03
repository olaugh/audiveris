// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless orchestration boundary for Java `HeadersStep`.
//!
//! Actual clef/key/time recognition remains an injected dependency. This
//! module ports only `AbstractSystemStep<Void>` traversal and error semantics.

use std::error::Error;
use std::fmt;

use crate::staff_header::StaffHeader;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessHeaderStaff {
    pub id: usize,
    pub tablature: bool,
    pub header: Option<StaffHeader>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessHeaderSystem {
    pub id: usize,
    /// Preserved source order; `HeaderBuilder` relies on system staff order.
    pub staffs: Vec<HeadlessHeaderStaff>,
}

/// Explicit boundary where Java constructs `HeaderBuilder(system)` and calls
/// `processHeader`. Implementations may retain partial system mutations on an
/// error; the step deliberately performs no rollback or cleanup.
pub trait HeaderRecognizer {
    type Error;

    fn process_header(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderRecognitionUnavailable {
    pub system_id: usize,
}

impl fmt::Display for HeaderRecognitionUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "header recognition is not ported for system {}",
            self.system_id
        )
    }
}

impl Error for HeaderRecognitionUnavailable {}

/// Honest default collaborator: it marks the first unported recognition
/// dependency instead of fabricating clefs, keys, or time signatures.
#[derive(Clone, Copy, Debug, Default)]
pub struct MissingHeaderRecognizer;

impl HeaderRecognizer for MissingHeaderRecognizer {
    type Error = HeaderRecognitionUnavailable;

    fn process_header(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error> {
        Err(HeaderRecognitionUnavailable {
            system_id: system.id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderSystemFailure<E> {
    pub system_id: usize,
    pub error: E,
}

/// One Java `HeadersStep.doit` lifecycle audit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadersStepReport<E> {
    pub prolog_completed: bool,
    pub attempted_system_ids: Vec<usize>,
    pub completed_system_ids: Vec<usize>,
    /// Java catches `StepException` per system, logs it, and continues.
    pub swallowed_failures: Vec<HeaderSystemFailure<E>>,
    pub epilog_completed: bool,
}

impl<E> Default for HeadersStepReport<E> {
    fn default() -> Self {
        Self {
            prolog_completed: false,
            attempted_system_ids: Vec::new(),
            completed_system_ids: Vec::new(),
            swallowed_failures: Vec::new(),
            epilog_completed: false,
        }
    }
}

/// Deterministic headless `HeadersStep` executor.
///
/// Java may dispatch systems in parallel based on a UI advanced-topic flag.
/// The headless port has no UI setting and therefore uses Java's sequential
/// branch, retaining exact source order and per-system `StepException`
/// swallowing. Panics, like unchecked Java exceptions in the sequential
/// branch, abort traversal and do not run the empty epilog.
pub struct HeadlessHeadersStep<Recognizer>
where
    Recognizer: HeaderRecognizer,
{
    recognizer: Recognizer,
    report: Option<HeadersStepReport<Recognizer::Error>>,
}

impl<Recognizer> HeadlessHeadersStep<Recognizer>
where
    Recognizer: HeaderRecognizer,
{
    #[must_use]
    pub const fn new(recognizer: Recognizer) -> Self {
        Self {
            recognizer,
            report: None,
        }
    }

    #[must_use]
    pub const fn recognizer(&self) -> &Recognizer {
        &self.recognizer
    }

    #[must_use]
    pub const fn report(&self) -> Option<&HeadersStepReport<Recognizer::Error>> {
        self.report.as_ref()
    }

    /// Java `HeadersStep.doSystem`, before `AbstractSystemStep` swallows a
    /// checked `StepException` at the per-system task boundary.
    pub fn do_system(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<(), Recognizer::Error> {
        self.recognizer.process_header(system)
    }

    /// Java `AbstractSystemStep.clearErrors` is intentionally void, since any
    /// GUI error clearing is performed system-by-system. Headless state is
    /// therefore unchanged.
    pub const fn clear_errors(&self) {}

    pub fn run(&mut self, systems: &mut [HeadlessHeaderSystem]) {
        self.report = Some(HeadersStepReport::default());
        self.report
            .as_mut()
            .expect("report was just installed")
            .prolog_completed = true;

        for system in systems {
            let system_id = system.id;
            self.report
                .as_mut()
                .expect("report remains installed")
                .attempted_system_ids
                .push(system_id);

            match self.recognizer.process_header(system) {
                Ok(()) => self
                    .report
                    .as_mut()
                    .expect("report remains installed")
                    .completed_system_ids
                    .push(system_id),
                Err(error) => self
                    .report
                    .as_mut()
                    .expect("report remains installed")
                    .swallowed_failures
                    .push(HeaderSystemFailure { system_id, error }),
            }
        }

        self.report
            .as_mut()
            .expect("report remains installed")
            .epilog_completed = true;
    }
}

impl Default for HeadlessHeadersStep<MissingHeaderRecognizer> {
    fn default() -> Self {
        Self::new(MissingHeaderRecognizer)
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    fn systems(ids: &[usize]) -> Vec<HeadlessHeaderSystem> {
        ids.iter()
            .map(|&id| HeadlessHeaderSystem {
                id,
                staffs: vec![HeadlessHeaderStaff {
                    id: id * 10,
                    tablature: false,
                    header: None,
                }],
            })
            .collect()
    }

    #[test]
    fn missing_recognizer_marks_every_system_without_fabricating_state() {
        let mut systems = systems(&[3, 1]);
        let mut step = HeadlessHeadersStep::default();
        step.run(&mut systems);

        let report = step.report().unwrap();
        assert!(report.prolog_completed);
        assert_eq!(report.attempted_system_ids, [3, 1]);
        assert!(report.completed_system_ids.is_empty());
        assert_eq!(
            report
                .swallowed_failures
                .iter()
                .map(|failure| failure.system_id)
                .collect::<Vec<_>>(),
            [3, 1]
        );
        assert!(report.epilog_completed);
        assert!(
            systems
                .iter()
                .all(|system| system.staffs[0].header.is_none())
        );
    }

    struct PartialRecognizer;

    impl HeaderRecognizer for PartialRecognizer {
        type Error = &'static str;

        fn process_header(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error> {
            system.staffs[0].header = Some(StaffHeader::new(system.id as i32));
            if system.id == 2 {
                Err("checked failure")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn checked_failure_retains_mutation_and_continues_in_source_order() {
        let mut systems = systems(&[1, 2, 3]);
        let mut step = HeadlessHeadersStep::new(PartialRecognizer);
        step.run(&mut systems);

        let report = step.report().unwrap();
        assert_eq!(report.attempted_system_ids, [1, 2, 3]);
        assert_eq!(report.completed_system_ids, [1, 3]);
        assert_eq!(
            report.swallowed_failures,
            [HeaderSystemFailure {
                system_id: 2,
                error: "checked failure",
            }]
        );
        assert!(report.epilog_completed);
        assert_eq!(systems[1].staffs[0].header.as_ref().unwrap().start, 2);
    }

    struct PanicRecognizer;

    impl HeaderRecognizer for PanicRecognizer {
        type Error = &'static str;

        fn process_header(&mut self, system: &mut HeadlessHeaderSystem) -> Result<(), Self::Error> {
            system.staffs[0].header = Some(StaffHeader::new(99));
            panic!("unchecked failure")
        }
    }

    #[test]
    fn unchecked_failure_has_no_cleanup_or_epilog() {
        let mut systems = systems(&[7, 8]);
        let mut step = HeadlessHeadersStep::new(PanicRecognizer);
        let result = catch_unwind(AssertUnwindSafe(|| step.run(&mut systems)));

        assert!(result.is_err());
        let report = step.report().unwrap();
        assert!(report.prolog_completed);
        assert_eq!(report.attempted_system_ids, [7]);
        assert!(report.completed_system_ids.is_empty());
        assert!(report.swallowed_failures.is_empty());
        assert!(!report.epilog_completed);
        assert_eq!(systems[0].staffs[0].header.as_ref().unwrap().start, 99);
        assert!(systems[1].staffs[0].header.is_none());
    }

    #[test]
    fn direct_do_system_exposes_recognition_boundary_error() {
        let mut system = systems(&[4]).remove(0);
        let mut step = HeadlessHeadersStep::default();
        assert_eq!(
            step.do_system(&mut system),
            Err(HeaderRecognitionUnavailable { system_id: 4 })
        );
        assert!(step.report().is_none());
    }
}
