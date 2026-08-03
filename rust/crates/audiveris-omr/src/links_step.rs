// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle and SIG ownership port of Java `LinksStep`.
//!
//! Symbol geometry and musical association remain injected at the first
//! semantic seam. This module owns Java pass order, system continuation,
//! relation/inter identity, cascading removal, free-glyph cleanup, and the
//! sheet-wide tie epilog.

use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralLinkInterKind {
    HeadChord,
    Slur,
    Symbol,
    Number,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLinkInter {
    pub id: usize,
    pub kind: NeutralLinkInterKind,
    pub removed: bool,
    pub abnormal: bool,
    /// Fixed-point grade selected by the injected matcher/reducer.
    pub contextual_grade: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralLinkRelationKind {
    Support,
    Exclusion,
    Containment,
    BeamHead,
    SlurHead,
    ChordGrace,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralLinkRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralLinkRelationKind,
    pub grade: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLinksSystem {
    pub id: usize,
    pub inters: Vec<NeutralLinkInter>,
    pub relations: Vec<NeutralLinkRelation>,
    pub free_glyph_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLinksSheet {
    pub id: usize,
    pub systems: Vec<NeutralLinksSystem>,
    pub mutations: Vec<LinksMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinksDelta {
    pub mutations: Vec<LinksDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinksDeltaMutation {
    AddInter(NeutralLinkInter),
    RemoveInter(usize),
    AddRelation(NeutralLinkRelation),
    RemoveRelation(usize),
    SetAbnormal { inter_id: usize, abnormal: bool },
    SetContextualGrade { inter_id: usize, grade: Option<i32> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinksStageOutcome<VisualError> {
    pub delta: LinksDelta,
    /// A checked stage failure retains the valid delta prefix, skips the
    /// remaining phases of this system, and continues with the next system.
    pub error: Option<VisualError>,
}

impl<VisualError> LinksStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: LinksDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinksPhase {
    InitialMeasureFill,
    Dynamics,
    Texts,
    Pedals,
    Wedges,
    Fermatas,
    Graces,
    AugmentationDots,
    Tuplets,
    OctaveShifts,
    Numbers,
    KeyCandidates,
    ReduceLinks,
    AggregateTremolos,
    FinalMeasureFill,
    PurgeContainers,
    BeamHeadCleanup,
}

const SYSTEM_PHASES: [LinksPhase; 17] = [
    LinksPhase::InitialMeasureFill,
    LinksPhase::Dynamics,
    LinksPhase::Texts,
    LinksPhase::Pedals,
    LinksPhase::Wedges,
    LinksPhase::Fermatas,
    LinksPhase::Graces,
    LinksPhase::AugmentationDots,
    LinksPhase::Tuplets,
    LinksPhase::OctaveShifts,
    LinksPhase::Numbers,
    LinksPhase::KeyCandidates,
    LinksPhase::ReduceLinks,
    LinksPhase::AggregateTremolos,
    LinksPhase::FinalMeasureFill,
    LinksPhase::PurgeContainers,
    LinksPhase::BeamHeadCleanup,
];

#[derive(Clone, Copy, Debug)]
pub struct LinksSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralLinksSystem,
    pub phase: LinksPhase,
}

#[derive(Clone, Copy, Debug)]
pub struct TieCheckInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralLinksSystem,
    /// Live head chords, in SIG source order, as passed to every slur check.
    pub head_chord_ids: &'a [usize],
}

/// Geometric and semantic seam. Calls arrive in exact Java source order and
/// always observe graph mutations committed by the preceding call.
pub trait VisualLinksPipeline {
    type Error;

    fn run_phase(&mut self, input: LinksSystemInput<'_>) -> LinksStageOutcome<Self::Error>;

    fn check_staff_ties(&mut self, input: TieCheckInput<'_>) -> LinksStageOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinksOptions {
    pub remove_free_glyphs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinksSystemFailure<VisualError> {
    pub system_id: usize,
    pub phase: LinksPhase,
    pub source: VisualError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinksReport<VisualError> {
    pub system_errors: Vec<LinksSystemFailure<VisualError>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinksStepError<VisualError> {
    Contract(LinksContractError),
    Epilog {
        system_id: usize,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for LinksStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(source) => write!(formatter, "links contract failed: {source}"),
            Self::Epilog { system_id, source } => {
                write!(
                    formatter,
                    "links tie epilog failed for system {system_id}: {source}"
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinksContractError {
    DuplicateSystem(usize),
    DuplicateInter(usize),
    UnknownInter(usize),
    DuplicateRelation(usize),
    UnknownRelation(usize),
    UnknownRelationEndpoint { relation_id: usize, inter_id: usize },
}

impl fmt::Display for LinksContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for LinksContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinksMutation {
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    RelationAdded {
        system_id: usize,
        relation_id: usize,
    },
    RelationRemoved {
        system_id: usize,
        relation_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    AbnormalSet {
        system_id: usize,
        inter_id: usize,
        abnormal: bool,
    },
    ContextualGradeSet {
        system_id: usize,
        inter_id: usize,
    },
    PhaseCompleted {
        system_id: usize,
        phase: LinksPhase,
    },
    SystemFailed {
        system_id: usize,
        phase: LinksPhase,
    },
    FreeGlyphsCleared {
        system_id: usize,
    },
    TiesChecked {
        system_id: usize,
    },
}

pub struct HeadlessLinksStep<Visual> {
    visual: Visual,
    options: LinksOptions,
}

impl<Visual> HeadlessLinksStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual, options: LinksOptions) -> Self {
        Self { visual, options }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualLinksPipeline> HeadlessLinksStep<Visual> {
    pub fn process(
        &mut self,
        sheet: &mut NeutralLinksSheet,
    ) -> Result<LinksReport<Visual::Error>, LinksStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(LinksStepError::Contract)?;
        let mut system_errors = Vec::new();

        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            for phase in SYSTEM_PHASES {
                let outcome = self.visual.run_phase(LinksSystemInput {
                    sheet_id: sheet.id,
                    system: &sheet.systems[system_index],
                    phase,
                });
                apply_delta(sheet, system_index, outcome.delta)
                    .map_err(LinksStepError::Contract)?;
                if let Some(source) = outcome.error {
                    sheet
                        .mutations
                        .push(LinksMutation::SystemFailed { system_id, phase });
                    system_errors.push(LinksSystemFailure {
                        system_id,
                        phase,
                        source,
                    });
                    break;
                }
                sheet
                    .mutations
                    .push(LinksMutation::PhaseCompleted { system_id, phase });
            }

            // This is after BeamHeadCleaner in Java, regardless of watch output.
            if self.options.remove_free_glyphs {
                sheet.systems[system_index].free_glyph_ids.clear();
                sheet
                    .mutations
                    .push(LinksMutation::FreeGlyphsCleared { system_id });
            }
        }

        // Java doEpilog: systems, then live head chords and slurs in SIG order.
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let head_chord_ids = sheet.systems[system_index]
                .inters
                .iter()
                .filter(|inter| !inter.removed && inter.kind == NeutralLinkInterKind::HeadChord)
                .map(|inter| inter.id)
                .collect::<Vec<_>>();
            let outcome = self.visual.check_staff_ties(TieCheckInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
                head_chord_ids: &head_chord_ids,
            });
            apply_delta(sheet, system_index, outcome.delta).map_err(LinksStepError::Contract)?;
            if let Some(source) = outcome.error {
                return Err(LinksStepError::Epilog { system_id, source });
            }
            sheet
                .mutations
                .push(LinksMutation::TiesChecked { system_id });
        }

        Ok(LinksReport { system_errors })
    }
}

fn validate_sheet(sheet: &NeutralLinksSheet) -> Result<(), LinksContractError> {
    let mut system_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(LinksContractError::DuplicateSystem(system.id));
        }
        validate_system(system)?;
    }
    Ok(())
}

fn validate_system(system: &NeutralLinksSystem) -> Result<(), LinksContractError> {
    let mut inter_ids = BTreeSet::new();
    for inter in &system.inters {
        if !inter_ids.insert(inter.id) {
            return Err(LinksContractError::DuplicateInter(inter.id));
        }
    }
    let mut relation_ids = BTreeSet::new();
    for relation in &system.relations {
        if !relation_ids.insert(relation.id) {
            return Err(LinksContractError::DuplicateRelation(relation.id));
        }
        for inter_id in [relation.source_inter_id, relation.target_inter_id] {
            if !inter_ids.contains(&inter_id) {
                return Err(LinksContractError::UnknownRelationEndpoint {
                    relation_id: relation.id,
                    inter_id,
                });
            }
        }
    }
    Ok(())
}

fn apply_delta(
    sheet: &mut NeutralLinksSheet,
    system_index: usize,
    delta: LinksDelta,
) -> Result<(), LinksContractError> {
    for mutation in delta.mutations {
        let system_id = sheet.systems[system_index].id;
        match mutation {
            LinksDeltaMutation::AddInter(inter) => {
                if sheet.systems[system_index]
                    .inters
                    .iter()
                    .any(|item| item.id == inter.id)
                {
                    return Err(LinksContractError::DuplicateInter(inter.id));
                }
                let inter_id = inter.id;
                sheet.systems[system_index].inters.push(inter);
                sheet.mutations.push(LinksMutation::InterAdded {
                    system_id,
                    inter_id,
                });
            }
            LinksDeltaMutation::RemoveInter(inter_id) => {
                let inter_index = live_inter_index(&sheet.systems[system_index], inter_id)?;
                // SIG vertex removal first removes incident edges in graph order.
                let relation_ids = sheet.systems[system_index]
                    .relations
                    .iter()
                    .filter(|relation| {
                        relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
                    })
                    .map(|relation| relation.id)
                    .collect::<Vec<_>>();
                for relation_id in relation_ids {
                    remove_relation(sheet, system_index, relation_id)?;
                }
                sheet.systems[system_index].inters[inter_index].removed = true;
                sheet.mutations.push(LinksMutation::InterRemoved {
                    system_id,
                    inter_id,
                });
            }
            LinksDeltaMutation::AddRelation(relation) => {
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|item| item.id == relation.id)
                {
                    return Err(LinksContractError::DuplicateRelation(relation.id));
                }
                for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                    live_inter_index(&sheet.systems[system_index], inter_id).map_err(|_| {
                        LinksContractError::UnknownRelationEndpoint {
                            relation_id: relation.id,
                            inter_id,
                        }
                    })?;
                }
                let relation_id = relation.id;
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(LinksMutation::RelationAdded {
                    system_id,
                    relation_id,
                });
            }
            LinksDeltaMutation::RemoveRelation(relation_id) => {
                remove_relation(sheet, system_index, relation_id)?;
            }
            LinksDeltaMutation::SetAbnormal { inter_id, abnormal } => {
                let index = live_inter_index(&sheet.systems[system_index], inter_id)?;
                sheet.systems[system_index].inters[index].abnormal = abnormal;
                sheet.mutations.push(LinksMutation::AbnormalSet {
                    system_id,
                    inter_id,
                    abnormal,
                });
            }
            LinksDeltaMutation::SetContextualGrade { inter_id, grade } => {
                let index = live_inter_index(&sheet.systems[system_index], inter_id)?;
                sheet.systems[system_index].inters[index].contextual_grade = grade;
                sheet.mutations.push(LinksMutation::ContextualGradeSet {
                    system_id,
                    inter_id,
                });
            }
        }
    }
    Ok(())
}

fn live_inter_index(
    system: &NeutralLinksSystem,
    inter_id: usize,
) -> Result<usize, LinksContractError> {
    system
        .inters
        .iter()
        .position(|inter| inter.id == inter_id && !inter.removed)
        .ok_or(LinksContractError::UnknownInter(inter_id))
}

fn remove_relation(
    sheet: &mut NeutralLinksSheet,
    system_index: usize,
    relation_id: usize,
) -> Result<(), LinksContractError> {
    let relation_index = sheet.systems[system_index]
        .relations
        .iter()
        .position(|relation| relation.id == relation_id)
        .ok_or(LinksContractError::UnknownRelation(relation_id))?;
    sheet.systems[system_index].relations.remove(relation_index);
    let system_id = sheet.systems[system_index].id;
    sheet.mutations.push(LinksMutation::RelationRemoved {
        system_id,
        relation_id,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    type Calls = Rc<RefCell<Vec<(usize, Option<LinksPhase>)>>>;

    #[derive(Clone)]
    struct Scripted {
        calls: Calls,
        fail: Option<(usize, LinksPhase)>,
    }

    impl VisualLinksPipeline for Scripted {
        type Error = &'static str;

        fn run_phase(&mut self, input: LinksSystemInput<'_>) -> LinksStageOutcome<Self::Error> {
            self.calls
                .borrow_mut()
                .push((input.system.id, Some(input.phase)));
            let mut delta = LinksDelta::default();
            if input.system.id == 1 && input.phase == LinksPhase::Dynamics {
                delta
                    .mutations
                    .push(LinksDeltaMutation::AddRelation(relation(10)));
            }
            if input.system.id == 1 && input.phase == LinksPhase::ReduceLinks {
                delta.mutations.push(LinksDeltaMutation::RemoveInter(1));
            }
            LinksStageOutcome {
                delta,
                error: (self.fail == Some((input.system.id, input.phase))).then_some("checked"),
            }
        }

        fn check_staff_ties(&mut self, input: TieCheckInput<'_>) -> LinksStageOutcome<Self::Error> {
            self.calls.borrow_mut().push((input.system.id, None));
            assert_eq!(input.head_chord_ids, &[2]);
            LinksStageOutcome::success(LinksDelta::default())
        }
    }

    fn inter(id: usize, kind: NeutralLinkInterKind) -> NeutralLinkInter {
        NeutralLinkInter {
            id,
            kind,
            removed: false,
            abnormal: false,
            contextual_grade: None,
        }
    }

    fn relation(id: usize) -> NeutralLinkRelation {
        NeutralLinkRelation {
            id,
            source_inter_id: 1,
            target_inter_id: 2,
            kind: NeutralLinkRelationKind::Support,
            grade: Some(800),
        }
    }

    fn system(id: usize) -> NeutralLinksSystem {
        NeutralLinksSystem {
            id,
            inters: vec![
                inter(1, NeutralLinkInterKind::Symbol),
                inter(2, NeutralLinkInterKind::HeadChord),
            ],
            relations: Vec::new(),
            free_glyph_ids: vec![90, 91],
        }
    }

    #[test]
    fn preserves_java_phase_order_then_sheet_wide_tie_epilog() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let visual = Scripted {
            calls: Rc::clone(&calls),
            fail: None,
        };
        let mut sheet = NeutralLinksSheet {
            id: 7,
            systems: vec![system(1), system(2)],
            mutations: Vec::new(),
        };
        let mut step = HeadlessLinksStep::new(visual, LinksOptions::default());
        step.process(&mut sheet).unwrap();

        let calls = calls.borrow();
        assert_eq!(
            &calls[..SYSTEM_PHASES.len()],
            &SYSTEM_PHASES.map(|phase| (1, Some(phase)))
        );
        assert_eq!(
            &calls[SYSTEM_PHASES.len()..SYSTEM_PHASES.len() * 2],
            &SYSTEM_PHASES.map(|phase| (2, Some(phase)))
        );
        assert_eq!(&calls[SYSTEM_PHASES.len() * 2..], &[(1, None), (2, None)]);
    }

    #[test]
    fn checked_failure_retains_prefix_and_continues_next_system() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let visual = Scripted {
            calls: Rc::clone(&calls),
            fail: Some((1, LinksPhase::Texts)),
        };
        let mut sheet = NeutralLinksSheet {
            id: 7,
            systems: vec![system(1), system(2)],
            mutations: Vec::new(),
        };
        let mut step = HeadlessLinksStep::new(visual, LinksOptions::default());
        let report = step.process(&mut sheet).unwrap();
        assert_eq!(report.system_errors[0].phase, LinksPhase::Texts);
        assert!(
            calls
                .borrow()
                .contains(&(2, Some(LinksPhase::BeamHeadCleanup)))
        );
        assert!(!calls.borrow().contains(&(1, Some(LinksPhase::Pedals))));
        assert!(
            sheet.systems[0]
                .relations
                .iter()
                .any(|relation| relation.id == 10)
        );
    }

    #[test]
    fn inter_removal_cascades_relations_before_vertex_and_keeps_ids_stable() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let visual = Scripted { calls, fail: None };
        let mut sheet = NeutralLinksSheet {
            id: 7,
            systems: vec![system(1)],
            mutations: Vec::new(),
        };
        let mut step = HeadlessLinksStep::new(visual, LinksOptions::default());
        step.process(&mut sheet).unwrap();
        let removed_relation = sheet
            .mutations
            .iter()
            .position(|m| {
                *m == LinksMutation::RelationRemoved {
                    system_id: 1,
                    relation_id: 10,
                }
            })
            .unwrap();
        let removed_inter = sheet
            .mutations
            .iter()
            .position(|m| {
                *m == LinksMutation::InterRemoved {
                    system_id: 1,
                    inter_id: 1,
                }
            })
            .unwrap();
        assert!(removed_relation < removed_inter);
        assert!(sheet.systems[0].relations.is_empty());
    }

    #[test]
    fn optional_free_glyph_cleanup_follows_last_system_phase() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let visual = Scripted { calls, fail: None };
        let mut sheet = NeutralLinksSheet {
            id: 7,
            systems: vec![system(1)],
            mutations: Vec::new(),
        };
        let mut step = HeadlessLinksStep::new(
            visual,
            LinksOptions {
                remove_free_glyphs: true,
            },
        );
        step.process(&mut sheet).unwrap();
        assert!(sheet.systems[0].free_glyph_ids.is_empty());
        let beam = sheet
            .mutations
            .iter()
            .position(|m| {
                *m == LinksMutation::PhaseCompleted {
                    system_id: 1,
                    phase: LinksPhase::BeamHeadCleanup,
                }
            })
            .unwrap();
        let clear = sheet
            .mutations
            .iter()
            .position(|m| *m == LinksMutation::FreeGlyphsCleared { system_id: 1 })
            .unwrap();
        assert!(beam < clear);
    }

    #[test]
    fn rejects_relation_to_unknown_inter_before_any_visual_work() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let visual = Scripted {
            calls: Rc::clone(&calls),
            fail: None,
        };
        let mut broken = system(1);
        broken.relations.push(NeutralLinkRelation {
            target_inter_id: 999,
            ..relation(4)
        });
        let mut sheet = NeutralLinksSheet {
            id: 7,
            systems: vec![broken],
            mutations: Vec::new(),
        };
        let mut step = HeadlessLinksStep::new(visual, LinksOptions::default());
        assert!(matches!(
            step.process(&mut sheet),
            Err(LinksStepError::Contract(
                LinksContractError::UnknownRelationEndpoint {
                    relation_id: 4,
                    inter_id: 999
                }
            ))
        ));
        assert!(calls.borrow().is_empty());
    }
}
