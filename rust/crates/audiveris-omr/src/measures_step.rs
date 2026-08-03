// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle and ownership port of Java `MeasuresStep`.
//!
//! `MeasuresBuilder` grouping/alignment geometry remains injected. This module
//! owns system traversal, partial mutation prefixes, barline/SIG ownership,
//! part/measure/stack topology, courtesy cleanup mutations, and the exact
//! page-numbering/dump epilog order.

use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeutralMeasureInterKind {
    PhysicalBarline {
        staff_id: usize,
    },
    StaffBarline {
        staff_id: usize,
        physical_barline_ids: Vec<usize>,
    },
    Head,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasureInter {
    pub id: usize,
    pub kind: NeutralMeasureInterKind,
    pub removed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasureStaff {
    pub id: usize,
    pub part_id: usize,
    /// Physical barlines followed by created logical staff barlines, each in
    /// Java insertion order.
    pub physical_barline_ids: Vec<usize>,
    pub staff_barline_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralPartBarline {
    pub id: usize,
    pub staff_barline_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasure {
    pub id: usize,
    pub part_id: usize,
    pub left_part_barline_id: Option<usize>,
    pub right_part_barline_id: Option<usize>,
    pub stack_id: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasurePart {
    pub id: usize,
    pub staff_ids: Vec<usize>,
    pub left_part_barline_id: Option<usize>,
    pub measure_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasureStack {
    pub id: usize,
    pub measure_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasuresSystem {
    pub id: usize,
    pub staves: Vec<NeutralMeasureStaff>,
    pub parts: Vec<NeutralMeasurePart>,
    pub inters: Vec<NeutralMeasureInter>,
    pub part_barlines: Vec<NeutralPartBarline>,
    pub measures: Vec<NeutralMeasure>,
    pub stacks: Vec<NeutralMeasureStack>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasuresPage {
    pub id: usize,
    pub system_ids: Vec<usize>,
    pub numbering_runs: usize,
    pub dump_runs: usize,
    pub last_numbered_stack_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralMeasuresSheet {
    pub id: usize,
    pub systems: Vec<NeutralMeasuresSystem>,
    pub pages: Vec<NeutralMeasuresPage>,
    pub mutations: Vec<MeasuresMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeasuresDelta {
    pub mutations: Vec<MeasuresDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasuresDeltaMutation {
    AddInter(NeutralMeasureInter),
    RemoveInter(usize),
    AttachStaffBarline {
        staff_id: usize,
        inter_id: usize,
    },
    AddPartBarline(NeutralPartBarline),
    SetPartLeftBarline {
        part_id: usize,
        part_barline_id: usize,
    },
    AddMeasure(NeutralMeasure),
    AttachMeasureToPart {
        part_id: usize,
        measure_id: usize,
    },
    AddStack(NeutralMeasureStack),
    AttachMeasureToStack {
        stack_id: usize,
        measure_id: usize,
    },
    SetMeasureLeftBarline {
        measure_id: usize,
        part_barline_id: usize,
    },
    SetMeasureRightBarline {
        measure_id: usize,
        part_barline_id: usize,
    },
    RemoveStack(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasuresStageOutcome<VisualError> {
    pub delta: MeasuresDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> MeasuresStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: MeasuresDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MeasuresSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralMeasuresSystem,
}

/// First unavailable semantic/geometric seam: physical-bar grouping, missing
/// column expansion, logical bar construction, and courtesy classification.
pub trait VisualMeasuresBuilder {
    type Error;

    fn build_measures(
        &mut self,
        input: MeasuresSystemInput<'_>,
    ) -> MeasuresStageOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasuresReport<VisualError> {
    pub system_errors: Vec<(usize, VisualError)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasuresStepError {
    Contract(MeasuresContractError),
}

impl fmt::Display for MeasuresStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(source) => write!(formatter, "measures contract failed: {source}"),
        }
    }
}

impl Error for MeasuresStepError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasuresContractError {
    DuplicateSystem(usize),
    DuplicatePage(usize),
    UnknownPageSystem { page_id: usize, system_id: usize },
    DuplicateInter(usize),
    UnknownInter(usize),
    WrongStaffBarline { inter_id: usize, staff_id: usize },
    UnknownStaff(usize),
    DuplicateStaffOwnership { staff_id: usize, inter_id: usize },
    DuplicatePartBarline(usize),
    UnknownPartBarline(usize),
    InvalidPartBarlineMember(usize),
    UnknownPart(usize),
    DuplicateMeasure(usize),
    UnknownMeasure(usize),
    DuplicatePartMeasure { part_id: usize, measure_id: usize },
    DuplicateStack(usize),
    UnknownStack(usize),
    DuplicateStackMeasure { stack_id: usize, measure_id: usize },
}

impl fmt::Display for MeasuresContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for MeasuresContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasuresMutation {
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    StaffBarlineAttached {
        staff_id: usize,
        inter_id: usize,
    },
    PartBarlineAdded {
        system_id: usize,
        part_barline_id: usize,
    },
    PartLeftBarlineSet {
        part_id: usize,
        part_barline_id: usize,
    },
    MeasureAdded {
        system_id: usize,
        measure_id: usize,
    },
    PartMeasureAttached {
        part_id: usize,
        measure_id: usize,
    },
    StackAdded {
        system_id: usize,
        stack_id: usize,
    },
    StackMeasureAttached {
        stack_id: usize,
        measure_id: usize,
    },
    MeasureLeftBarlineSet {
        measure_id: usize,
        part_barline_id: usize,
    },
    MeasureRightBarlineSet {
        measure_id: usize,
        part_barline_id: usize,
    },
    StackRemoved {
        system_id: usize,
        stack_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
    PageNumbered {
        page_id: usize,
    },
    PageMeasureCountsDumped {
        page_id: usize,
    },
}

pub struct HeadlessMeasuresStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessMeasuresStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualMeasuresBuilder> HeadlessMeasuresStep<Visual> {
    /// Java `AbstractSystemStep`: systems in sheet order with checked
    /// continuation, then each page is numbered and immediately dumped.
    pub fn process(
        &mut self,
        sheet: &mut NeutralMeasuresSheet,
    ) -> Result<MeasuresReport<Visual::Error>, MeasuresStepError> {
        validate_sheet(sheet).map_err(MeasuresStepError::Contract)?;
        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.build_measures(MeasuresSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, outcome.delta).map_err(MeasuresStepError::Contract)?;
            if let Some(error) = outcome.error {
                sheet
                    .mutations
                    .push(MeasuresMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
            }
        }

        for page_index in 0..sheet.pages.len() {
            let page_id = sheet.pages[page_index].id;
            let system_ids = sheet.pages[page_index].system_ids.clone();
            let stack_count = sheet
                .systems
                .iter()
                .filter(|system| system_ids.contains(&system.id))
                .map(|system| system.stacks.len())
                .sum();
            sheet.pages[page_index].numbering_runs += 1;
            sheet.pages[page_index].last_numbered_stack_count = stack_count;
            sheet
                .mutations
                .push(MeasuresMutation::PageNumbered { page_id });
            sheet.pages[page_index].dump_runs += 1;
            sheet
                .mutations
                .push(MeasuresMutation::PageMeasureCountsDumped { page_id });
        }
        Ok(MeasuresReport { system_errors })
    }
}

fn validate_sheet(sheet: &NeutralMeasuresSheet) -> Result<(), MeasuresContractError> {
    let mut system_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(MeasuresContractError::DuplicateSystem(system.id));
        }
    }
    let mut page_ids = BTreeSet::new();
    for page in &sheet.pages {
        if !page_ids.insert(page.id) {
            return Err(MeasuresContractError::DuplicatePage(page.id));
        }
        for system_id in &page.system_ids {
            if !system_ids.contains(system_id) {
                return Err(MeasuresContractError::UnknownPageSystem {
                    page_id: page.id,
                    system_id: *system_id,
                });
            }
        }
    }
    Ok(())
}

fn apply_delta(
    sheet: &mut NeutralMeasuresSheet,
    system_index: usize,
    delta: MeasuresDelta,
) -> Result<(), MeasuresContractError> {
    for mutation in delta.mutations {
        apply_mutation(sheet, system_index, mutation)?;
    }
    Ok(())
}

fn apply_mutation(
    sheet: &mut NeutralMeasuresSheet,
    system_index: usize,
    mutation: MeasuresDeltaMutation,
) -> Result<(), MeasuresContractError> {
    let system_id = sheet.systems[system_index].id;
    match mutation {
        MeasuresDeltaMutation::AddInter(inter) => {
            if sheet.systems[system_index]
                .inters
                .iter()
                .any(|existing| existing.id == inter.id)
            {
                return Err(MeasuresContractError::DuplicateInter(inter.id));
            }
            let inter_id = inter.id;
            sheet.systems[system_index].inters.push(inter);
            sheet.mutations.push(MeasuresMutation::InterAdded {
                system_id,
                inter_id,
            });
        }
        MeasuresDeltaMutation::RemoveInter(inter_id) => {
            let inter = sheet.systems[system_index]
                .inters
                .iter_mut()
                .find(|inter| inter.id == inter_id)
                .ok_or(MeasuresContractError::UnknownInter(inter_id))?;
            inter.removed = true;
            for staff in &mut sheet.systems[system_index].staves {
                staff.physical_barline_ids.retain(|id| *id != inter_id);
                staff.staff_barline_ids.retain(|id| *id != inter_id);
            }
            sheet.mutations.push(MeasuresMutation::InterRemoved {
                system_id,
                inter_id,
            });
        }
        MeasuresDeltaMutation::AttachStaffBarline { staff_id, inter_id } => {
            let inter = sheet.systems[system_index]
                .inters
                .iter()
                .find(|inter| inter.id == inter_id && !inter.removed)
                .ok_or(MeasuresContractError::UnknownInter(inter_id))?;
            if !matches!(
                inter.kind,
                NeutralMeasureInterKind::StaffBarline { staff_id: owner, .. } if owner == staff_id
            ) {
                return Err(MeasuresContractError::WrongStaffBarline { inter_id, staff_id });
            }
            let staff = sheet.systems[system_index]
                .staves
                .iter_mut()
                .find(|staff| staff.id == staff_id)
                .ok_or(MeasuresContractError::UnknownStaff(staff_id))?;
            if staff.staff_barline_ids.contains(&inter_id) {
                return Err(MeasuresContractError::DuplicateStaffOwnership { staff_id, inter_id });
            }
            staff.staff_barline_ids.push(inter_id);
            sheet
                .mutations
                .push(MeasuresMutation::StaffBarlineAttached { staff_id, inter_id });
        }
        MeasuresDeltaMutation::AddPartBarline(part_barline) => {
            if sheet.systems[system_index]
                .part_barlines
                .iter()
                .any(|existing| existing.id == part_barline.id)
            {
                return Err(MeasuresContractError::DuplicatePartBarline(part_barline.id));
            }
            for inter_id in &part_barline.staff_barline_ids {
                if !sheet.systems[system_index].inters.iter().any(|inter| {
                    inter.id == *inter_id
                        && !inter.removed
                        && matches!(inter.kind, NeutralMeasureInterKind::StaffBarline { .. })
                }) {
                    return Err(MeasuresContractError::InvalidPartBarlineMember(*inter_id));
                }
            }
            let part_barline_id = part_barline.id;
            sheet.systems[system_index].part_barlines.push(part_barline);
            sheet.mutations.push(MeasuresMutation::PartBarlineAdded {
                system_id,
                part_barline_id,
            });
        }
        MeasuresDeltaMutation::SetPartLeftBarline {
            part_id,
            part_barline_id,
        } => {
            ensure_part_barline(&sheet.systems[system_index], part_barline_id)?;
            let part = part_mut(&mut sheet.systems[system_index], part_id)?;
            part.left_part_barline_id = Some(part_barline_id);
            sheet.mutations.push(MeasuresMutation::PartLeftBarlineSet {
                part_id,
                part_barline_id,
            });
        }
        MeasuresDeltaMutation::AddMeasure(measure) => {
            if sheet.systems[system_index]
                .measures
                .iter()
                .any(|existing| existing.id == measure.id)
            {
                return Err(MeasuresContractError::DuplicateMeasure(measure.id));
            }
            if !sheet.systems[system_index]
                .parts
                .iter()
                .any(|part| part.id == measure.part_id)
            {
                return Err(MeasuresContractError::UnknownPart(measure.part_id));
            }
            let measure_id = measure.id;
            sheet.systems[system_index].measures.push(measure);
            sheet.mutations.push(MeasuresMutation::MeasureAdded {
                system_id,
                measure_id,
            });
        }
        MeasuresDeltaMutation::AttachMeasureToPart {
            part_id,
            measure_id,
        } => {
            ensure_measure(&sheet.systems[system_index], measure_id)?;
            let part = part_mut(&mut sheet.systems[system_index], part_id)?;
            if part.measure_ids.contains(&measure_id) {
                return Err(MeasuresContractError::DuplicatePartMeasure {
                    part_id,
                    measure_id,
                });
            }
            part.measure_ids.push(measure_id);
            sheet.mutations.push(MeasuresMutation::PartMeasureAttached {
                part_id,
                measure_id,
            });
        }
        MeasuresDeltaMutation::AddStack(stack) => {
            if sheet.systems[system_index]
                .stacks
                .iter()
                .any(|existing| existing.id == stack.id)
            {
                return Err(MeasuresContractError::DuplicateStack(stack.id));
            }
            let stack_id = stack.id;
            sheet.systems[system_index].stacks.push(stack);
            sheet.mutations.push(MeasuresMutation::StackAdded {
                system_id,
                stack_id,
            });
        }
        MeasuresDeltaMutation::AttachMeasureToStack {
            stack_id,
            measure_id,
        } => {
            ensure_measure(&sheet.systems[system_index], measure_id)?;
            let stack = sheet.systems[system_index]
                .stacks
                .iter_mut()
                .find(|stack| stack.id == stack_id)
                .ok_or(MeasuresContractError::UnknownStack(stack_id))?;
            if stack.measure_ids.contains(&measure_id) {
                return Err(MeasuresContractError::DuplicateStackMeasure {
                    stack_id,
                    measure_id,
                });
            }
            stack.measure_ids.push(measure_id);
            let measure = sheet.systems[system_index]
                .measures
                .iter_mut()
                .find(|measure| measure.id == measure_id)
                .expect("measure was validated");
            measure.stack_id = Some(stack_id);
            sheet
                .mutations
                .push(MeasuresMutation::StackMeasureAttached {
                    stack_id,
                    measure_id,
                });
        }
        MeasuresDeltaMutation::SetMeasureLeftBarline {
            measure_id,
            part_barline_id,
        }
        | MeasuresDeltaMutation::SetMeasureRightBarline {
            measure_id,
            part_barline_id,
        } => {
            ensure_part_barline(&sheet.systems[system_index], part_barline_id)?;
            let measure = sheet.systems[system_index]
                .measures
                .iter_mut()
                .find(|measure| measure.id == measure_id)
                .ok_or(MeasuresContractError::UnknownMeasure(measure_id))?;
            if matches!(
                mutation,
                MeasuresDeltaMutation::SetMeasureLeftBarline { .. }
            ) {
                measure.left_part_barline_id = Some(part_barline_id);
                sheet
                    .mutations
                    .push(MeasuresMutation::MeasureLeftBarlineSet {
                        measure_id,
                        part_barline_id,
                    });
            } else {
                measure.right_part_barline_id = Some(part_barline_id);
                sheet
                    .mutations
                    .push(MeasuresMutation::MeasureRightBarlineSet {
                        measure_id,
                        part_barline_id,
                    });
            }
        }
        MeasuresDeltaMutation::RemoveStack(stack_id) => {
            let Some(index) = sheet.systems[system_index]
                .stacks
                .iter()
                .position(|stack| stack.id == stack_id)
            else {
                return Err(MeasuresContractError::UnknownStack(stack_id));
            };
            let stack = sheet.systems[system_index].stacks.remove(index);
            for measure_id in stack.measure_ids {
                if let Some(measure) = sheet.systems[system_index]
                    .measures
                    .iter_mut()
                    .find(|measure| measure.id == measure_id)
                {
                    measure.stack_id = None;
                }
            }
            sheet.mutations.push(MeasuresMutation::StackRemoved {
                system_id,
                stack_id,
            });
        }
    }
    Ok(())
}

fn ensure_part_barline(
    system: &NeutralMeasuresSystem,
    id: usize,
) -> Result<(), MeasuresContractError> {
    system
        .part_barlines
        .iter()
        .any(|barline| barline.id == id)
        .then_some(())
        .ok_or(MeasuresContractError::UnknownPartBarline(id))
}

fn ensure_measure(system: &NeutralMeasuresSystem, id: usize) -> Result<(), MeasuresContractError> {
    system
        .measures
        .iter()
        .any(|measure| measure.id == id)
        .then_some(())
        .ok_or(MeasuresContractError::UnknownMeasure(id))
}

fn part_mut(
    system: &mut NeutralMeasuresSystem,
    id: usize,
) -> Result<&mut NeutralMeasurePart, MeasuresContractError> {
    system
        .parts
        .iter_mut()
        .find(|part| part.id == id)
        .ok_or(MeasuresContractError::UnknownPart(id))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct FakeBuilder {
        outcomes: BTreeMap<usize, MeasuresStageOutcome<&'static str>>,
        calls: Vec<(usize, usize, usize)>,
    }

    impl VisualMeasuresBuilder for FakeBuilder {
        type Error = &'static str;

        fn build_measures(
            &mut self,
            input: MeasuresSystemInput<'_>,
        ) -> MeasuresStageOutcome<Self::Error> {
            self.calls.push((
                input.system.id,
                input.system.stacks.len(),
                input.system.inters.len(),
            ));
            self.outcomes
                .remove(&input.system.id)
                .unwrap_or_else(|| MeasuresStageOutcome::success(MeasuresDelta::default()))
        }
    }

    fn system(id: usize, staff_id: usize, part_id: usize) -> NeutralMeasuresSystem {
        NeutralMeasuresSystem {
            id,
            staves: vec![NeutralMeasureStaff {
                id: staff_id,
                part_id,
                physical_barline_ids: vec![10 + id],
                staff_barline_ids: Vec::new(),
            }],
            parts: vec![NeutralMeasurePart {
                id: part_id,
                staff_ids: vec![staff_id],
                left_part_barline_id: None,
                measure_ids: Vec::new(),
            }],
            inters: vec![NeutralMeasureInter {
                id: 10 + id,
                kind: NeutralMeasureInterKind::PhysicalBarline { staff_id },
                removed: false,
            }],
            part_barlines: Vec::new(),
            measures: Vec::new(),
            stacks: Vec::new(),
        }
    }

    fn sheet() -> NeutralMeasuresSheet {
        NeutralMeasuresSheet {
            id: 1,
            systems: vec![system(2, 20, 200), system(1, 10, 100)],
            pages: vec![
                NeutralMeasuresPage {
                    id: 8,
                    system_ids: vec![2],
                    numbering_runs: 0,
                    dump_runs: 0,
                    last_numbered_stack_count: 0,
                },
                NeutralMeasuresPage {
                    id: 7,
                    system_ids: vec![1],
                    numbering_runs: 0,
                    dump_runs: 0,
                    last_numbered_stack_count: 0,
                },
            ],
            mutations: Vec::new(),
        }
    }

    fn build_delta(staff_id: usize, part_id: usize, base: usize) -> MeasuresDelta {
        MeasuresDelta {
            mutations: vec![
                MeasuresDeltaMutation::AddInter(NeutralMeasureInter {
                    id: base,
                    kind: NeutralMeasureInterKind::StaffBarline {
                        staff_id,
                        physical_barline_ids: Vec::new(),
                    },
                    removed: false,
                }),
                MeasuresDeltaMutation::AttachStaffBarline {
                    staff_id,
                    inter_id: base,
                },
                MeasuresDeltaMutation::AddPartBarline(NeutralPartBarline {
                    id: base + 1,
                    staff_barline_ids: vec![base],
                }),
                MeasuresDeltaMutation::AddMeasure(NeutralMeasure {
                    id: base + 2,
                    part_id,
                    left_part_barline_id: None,
                    right_part_barline_id: None,
                    stack_id: None,
                }),
                MeasuresDeltaMutation::AttachMeasureToPart {
                    part_id,
                    measure_id: base + 2,
                },
                MeasuresDeltaMutation::SetMeasureRightBarline {
                    measure_id: base + 2,
                    part_barline_id: base + 1,
                },
                MeasuresDeltaMutation::AddStack(NeutralMeasureStack {
                    id: base + 3,
                    measure_ids: Vec::new(),
                }),
                MeasuresDeltaMutation::AttachMeasureToStack {
                    stack_id: base + 3,
                    measure_id: base + 2,
                },
            ],
        }
    }

    #[test]
    fn systems_then_page_number_and_dump_follow_java_source_order() {
        let mut visual = FakeBuilder::default();
        visual
            .outcomes
            .insert(2, MeasuresStageOutcome::success(build_delta(20, 200, 1000)));
        visual
            .outcomes
            .insert(1, MeasuresStageOutcome::success(build_delta(10, 100, 2000)));
        let mut step = HeadlessMeasuresStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert!(report.system_errors.is_empty());
        assert_eq!(step.visual().calls, vec![(2, 0, 1), (1, 0, 1)]);
        assert_eq!(sheet.pages[0].last_numbered_stack_count, 1);
        assert_eq!(sheet.pages[1].last_numbered_stack_count, 1);
        assert_eq!(
            &sheet.mutations[sheet.mutations.len() - 4..],
            &[
                MeasuresMutation::PageNumbered { page_id: 8 },
                MeasuresMutation::PageMeasureCountsDumped { page_id: 8 },
                MeasuresMutation::PageNumbered { page_id: 7 },
                MeasuresMutation::PageMeasureCountsDumped { page_id: 7 },
            ]
        );
    }

    #[test]
    fn checked_failure_keeps_prefix_continues_and_epilogs() {
        let mut visual = FakeBuilder::default();
        visual.outcomes.insert(
            2,
            MeasuresStageOutcome {
                delta: build_delta(20, 200, 1000),
                error: Some("partial column"),
            },
        );
        visual
            .outcomes
            .insert(1, MeasuresStageOutcome::success(build_delta(10, 100, 2000)));
        let mut step = HeadlessMeasuresStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "partial column")]);
        assert_eq!(sheet.systems[0].stacks.len(), 1);
        assert_eq!(sheet.systems[1].stacks.len(), 1);
        assert!(
            sheet
                .mutations
                .contains(&MeasuresMutation::SystemFailed { system_id: 2 })
        );
        assert!(sheet.pages.iter().all(|page| page.dump_runs == 1));
    }

    #[test]
    fn courtesy_cleanup_removes_stack_and_clears_measure_backlinks() {
        let mut delta = build_delta(20, 200, 1000);
        delta
            .mutations
            .push(MeasuresDeltaMutation::RemoveStack(1003));
        let mut visual = FakeBuilder::default();
        visual
            .outcomes
            .insert(2, MeasuresStageOutcome::success(delta));
        let mut step = HeadlessMeasuresStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert!(sheet.systems[0].stacks.is_empty());
        assert_eq!(sheet.systems[0].measures[0].stack_id, None);
        assert!(sheet.mutations.contains(&MeasuresMutation::StackRemoved {
            system_id: 2,
            stack_id: 1003,
        }));
    }

    #[test]
    fn contract_failure_retains_valid_prefix_and_skips_epilog() {
        let mut delta = build_delta(20, 200, 1000);
        delta
            .mutations
            .push(MeasuresDeltaMutation::AddStack(NeutralMeasureStack {
                id: 1003,
                measure_ids: Vec::new(),
            }));
        let mut visual = FakeBuilder::default();
        visual
            .outcomes
            .insert(2, MeasuresStageOutcome::success(delta));
        let mut step = HeadlessMeasuresStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(MeasuresStepError::Contract(
                MeasuresContractError::DuplicateStack(1003)
            ))
        );
        assert_eq!(sheet.systems[0].stacks.len(), 1);
        assert!(sheet.pages.iter().all(|page| page.numbering_runs == 0));
    }
}
