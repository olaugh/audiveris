// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `ReductionStep`.
//!
//! Foundation reduction, stem endpoint geometry, and beam-group splitting
//! remain injected semantic collaborators. This module owns the observable
//! sheet/system order, live SIG and ownership mutations, checked-system
//! continuation, epilog order, stem-free-length median, and glyph cleanup.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralReductionGlyph {
    pub id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NeutralReductionInterKind {
    Stem {
        free_length: Option<i32>,
        head_end: f64,
        tail_end: f64,
    },
    BeamGroup,
    Note,
    Other(u16),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralReductionInter {
    pub id: usize,
    pub kind: NeutralReductionInterKind,
    pub glyph_id: Option<usize>,
    pub staff_id: Option<usize>,
    /// Java ensemble membership, in insertion order.
    pub member_ids: Vec<usize>,
}

impl NeutralReductionInter {
    #[must_use]
    pub const fn free_stem_length(&self) -> Option<i32> {
        match self.kind {
            NeutralReductionInterKind::Stem { free_length, .. } => free_length,
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_stem(&self) -> bool {
        matches!(self.kind, NeutralReductionInterKind::Stem { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralReductionRelationKind {
    Existing(u16),
    ExclusionOverlap,
    ExclusionIncompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralReductionRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralReductionRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralReductionStaff {
    pub id: usize,
    /// Persistent staff-line glyphs protected by Reduction epilog cleanup.
    pub line_glyph_ids: Vec<usize>,
    /// Live `AbstractNoteInter` ownership, in insertion order.
    pub note_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralReductionSystem {
    pub id: usize,
    pub staves: Vec<NeutralReductionStaff>,
    /// Java SIG vertex iteration order.
    pub inters: Vec<NeutralReductionInter>,
    pub relations: Vec<NeutralReductionRelation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralReductionSheet {
    pub id: usize,
    pub interline: f64,
    pub systems: Vec<NeutralReductionSystem>,
    /// Java GlyphIndex entity order, which is ascending persistent ID.
    pub registered_glyphs: Vec<NeutralReductionGlyph>,
    /// Sheet InterIndex live ownership and consumed allocator holes.
    pub live_inter_ids: Vec<usize>,
    pub retired_inter_ids: Vec<usize>,
    pub next_inter_id: usize,
    pub stem_free_length_median: Option<StemFreeLengthMedian>,
    pub mutations: Vec<ReductionMutation>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemFreeLengthMedian {
    pub pixels: i32,
    pub interlines: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReductionMutation {
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    StaffNoteAttached {
        staff_id: usize,
        inter_id: usize,
    },
    StaffNoteDetached {
        staff_id: usize,
        inter_id: usize,
    },
    RelationAdded {
        system_id: usize,
        relation: NeutralReductionRelation,
    },
    RelationRemoved {
        system_id: usize,
        relation: NeutralReductionRelation,
    },
    MemberAttached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    MemberDetached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    StemGeometrySet {
        system_id: usize,
        stem_id: usize,
        head_end: f64,
        tail_end: f64,
    },
    StemFreeLengthSet {
        system_id: usize,
        stem_id: usize,
        free_length: Option<i32>,
    },
    SystemFailed {
        system_id: usize,
    },
    BeamGroupsChecked {
        system_id: usize,
    },
    StemFreeLengthMedianSet(StemFreeLengthMedian),
    GlyphRemoved {
        glyph_id: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReductionDelta {
    /// Exact successful mutation prefix before any reported failure.
    pub mutations: Vec<ReductionDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReductionDeltaMutation {
    AddInter(NeutralReductionInter),
    RemoveInter(usize),
    AddRelation(NeutralReductionRelation),
    RemoveRelation(usize),
    AttachMember {
        owner_id: usize,
        member_id: usize,
    },
    DetachMember {
        owner_id: usize,
        member_id: usize,
    },
    SetStemGeometry {
        stem_id: usize,
        head_end: f64,
        tail_end: f64,
    },
    SetStemFreeLength {
        stem_id: usize,
        free_length: Option<i32>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReductionFailure<VisualError> {
    Checked(VisualError),
    Fatal(VisualError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReductionStageOutcome<VisualError> {
    pub delta: ReductionDelta,
    pub failure: Option<ReductionFailure<VisualError>>,
}

impl<VisualError> ReductionStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: ReductionDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReductionSystemInput<'a> {
    pub sheet: &'a NeutralReductionSheet,
    pub system_id: usize,
    /// Java `new SigReducer(system, true)`.
    pub purge_weaks: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ReductionStemInput<'a> {
    pub sheet: &'a NeutralReductionSheet,
    pub system_id: usize,
    pub stem_id: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ReductionBeamGroupsInput<'a> {
    pub sheet: &'a NeutralReductionSheet,
    pub system_id: usize,
}

/// First unavailable semantic dependencies in Java source order.
pub trait VisualReduction {
    type Error;

    fn reduce_foundations(
        &mut self,
        input: ReductionSystemInput<'_>,
    ) -> ReductionStageOutcome<Self::Error>;

    fn refine_stem_head_end(
        &mut self,
        input: ReductionStemInput<'_>,
    ) -> ReductionStageOutcome<Self::Error>;

    fn refine_stem_tail_end(
        &mut self,
        input: ReductionStemInput<'_>,
    ) -> ReductionStageOutcome<Self::Error>;

    fn check_beam_groups(
        &mut self,
        input: ReductionBeamGroupsInput<'_>,
    ) -> ReductionStageOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReductionStage {
    Foundations,
    RefineStemHead,
    RefineStemTail,
    BeamGroups,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReductionStepError<VisualError> {
    FatalSystem {
        system_id: usize,
        stage: ReductionStage,
        source: VisualError,
        checked_errors: Vec<(usize, VisualError)>,
    },
    Epilog {
        system_id: usize,
        source: VisualError,
        checked_errors: Vec<(usize, VisualError)>,
    },
    Contract(ReductionContractError),
}

impl<VisualError: fmt::Display> fmt::Display for ReductionStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FatalSystem {
                system_id,
                stage,
                source,
                ..
            } => write!(
                formatter,
                "reduction system {system_id} {stage:?} failed: {source}"
            ),
            Self::Epilog {
                system_id, source, ..
            } => write!(
                formatter,
                "reduction beam-group epilog failed in system {system_id}: {source}"
            ),
            Self::Contract(source) => write!(formatter, "reduction contract failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for ReductionStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FatalSystem { source, .. } | Self::Epilog { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReductionContractError {
    InvalidInterline,
    DuplicateSystem(usize),
    DuplicateStaff(usize),
    DuplicateGlyph(usize),
    UnorderedGlyphIndex,
    DuplicateInter(usize),
    InvalidLiveInterIndex,
    InvalidRetiredInter(usize),
    InvalidInterAllocator {
        expected: usize,
        actual: usize,
    },
    UnknownSystem(usize),
    UnknownStaff {
        system_id: usize,
        staff_id: usize,
    },
    UnknownGlyph(usize),
    UnknownInter {
        system_id: usize,
        inter_id: usize,
    },
    UnknownRelation {
        system_id: usize,
        relation_id: usize,
    },
    DuplicateRelation {
        system_id: usize,
        relation_id: usize,
    },
    UnknownRelationEndpoint(usize),
    InvalidStaffNote {
        staff_id: usize,
        inter_id: usize,
    },
    DuplicateStaffNote {
        staff_id: usize,
        inter_id: usize,
    },
    DuplicateMember {
        owner_id: usize,
        member_id: usize,
    },
    UnknownMember {
        owner_id: usize,
        member_id: usize,
    },
    InvalidStem(usize),
}

impl fmt::Display for ReductionContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ReductionContractError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ReductionReport<VisualError> {
    pub system_errors: Vec<(usize, VisualError)>,
    pub stem_free_length_median: Option<StemFreeLengthMedian>,
    pub removed_glyph_ids: Vec<usize>,
}

pub struct HeadlessReductionStep<Visual> {
    visual: Visual,
    refine_stem_head_end: bool,
    refine_stem_tail_end: bool,
}

impl<Visual> HeadlessReductionStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            refine_stem_head_end: true,
            refine_stem_tail_end: false,
        }
    }

    #[must_use]
    pub const fn with_refinements(
        visual: Visual,
        refine_stem_head_end: bool,
        refine_stem_tail_end: bool,
    ) -> Self {
        Self {
            visual,
            refine_stem_head_end,
            refine_stem_tail_end,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualReduction> HeadlessReductionStep<Visual> {
    /// Java `AbstractSystemStep.doit`: no prolog, system tasks, then epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralReductionSheet,
    ) -> Result<ReductionReport<Visual::Error>, ReductionStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(ReductionStepError::Contract)?;
        let mut system_errors = Vec::new();

        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.reduce_foundations(ReductionSystemInput {
                sheet,
                system_id,
                purge_weaks: true,
            });
            apply_delta(sheet, system_id, outcome.delta).map_err(ReductionStepError::Contract)?;
            match outcome.failure {
                Some(ReductionFailure::Checked(source)) => {
                    sheet
                        .mutations
                        .push(ReductionMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                    continue;
                }
                Some(ReductionFailure::Fatal(source)) => {
                    return Err(ReductionStepError::FatalSystem {
                        system_id,
                        stage: ReductionStage::Foundations,
                        source,
                        checked_errors: system_errors,
                    });
                }
                None => {}
            }

            if self.refine_stem_head_end {
                let stem_ids = stem_ids(&sheet.systems[system_index]);
                for stem_id in stem_ids {
                    let outcome = self.visual.refine_stem_head_end(ReductionStemInput {
                        sheet,
                        system_id,
                        stem_id,
                    });
                    apply_delta(sheet, system_id, outcome.delta)
                        .map_err(ReductionStepError::Contract)?;
                    if let Some(failure) = outcome.failure {
                        let source = match failure {
                            ReductionFailure::Checked(source) | ReductionFailure::Fatal(source) => {
                                source
                            }
                        };
                        return Err(ReductionStepError::FatalSystem {
                            system_id,
                            stage: ReductionStage::RefineStemHead,
                            source,
                            checked_errors: system_errors,
                        });
                    }
                }
            }

            if self.refine_stem_tail_end {
                let stem_ids = stem_ids(&sheet.systems[system_index]);
                for stem_id in stem_ids {
                    let outcome = self.visual.refine_stem_tail_end(ReductionStemInput {
                        sheet,
                        system_id,
                        stem_id,
                    });
                    apply_delta(sheet, system_id, outcome.delta)
                        .map_err(ReductionStepError::Contract)?;
                    if let Some(failure) = outcome.failure {
                        let source = match failure {
                            ReductionFailure::Checked(source) | ReductionFailure::Fatal(source) => {
                                source
                            }
                        };
                        return Err(ReductionStepError::FatalSystem {
                            system_id,
                            stage: ReductionStage::RefineStemTail,
                            source,
                            checked_errors: system_errors,
                        });
                    }
                }
            }
        }

        // Java epilog first checks every system's beam groups.
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self
                .visual
                .check_beam_groups(ReductionBeamGroupsInput { sheet, system_id });
            apply_delta(sheet, system_id, outcome.delta).map_err(ReductionStepError::Contract)?;
            if let Some(failure) = outcome.failure {
                let source = match failure {
                    ReductionFailure::Checked(source) | ReductionFailure::Fatal(source) => source,
                };
                return Err(ReductionStepError::Epilog {
                    system_id,
                    source,
                    checked_errors: system_errors,
                });
            }
            sheet
                .mutations
                .push(ReductionMutation::BeamGroupsChecked { system_id });
        }

        let median = measure_stem_free_length(sheet);
        if let Some(median) = median {
            sheet.stem_free_length_median = Some(median);
            sheet
                .mutations
                .push(ReductionMutation::StemFreeLengthMedianSet(median));
        }
        let removed_glyph_ids = cleanup_glyphs(sheet);

        Ok(ReductionReport {
            system_errors,
            stem_free_length_median: median,
            removed_glyph_ids,
        })
    }
}

fn stem_ids(system: &NeutralReductionSystem) -> Vec<usize> {
    system
        .inters
        .iter()
        .filter(|inter| inter.is_stem())
        .map(|inter| inter.id)
        .collect()
}

fn measure_stem_free_length(sheet: &NeutralReductionSheet) -> Option<StemFreeLengthMedian> {
    let mut lengths = sheet
        .systems
        .iter()
        .flat_map(|system| &system.inters)
        .filter_map(NeutralReductionInter::free_stem_length)
        .collect::<Vec<_>>();
    if lengths.is_empty() {
        return None;
    }
    lengths.sort_unstable();
    let pixels = lengths[lengths.len() / 2];
    Some(StemFreeLengthMedian {
        pixels,
        interlines: f64::from(pixels) / sheet.interline,
    })
}

fn cleanup_glyphs(sheet: &mut NeutralReductionSheet) -> Vec<usize> {
    let mut keep = BTreeSet::new();
    for system in &sheet.systems {
        for staff in &system.staves {
            keep.extend(staff.line_glyph_ids.iter().copied());
        }
        for inter in &system.inters {
            if let Some(glyph_id) = inter.glyph_id {
                keep.insert(glyph_id);
            }
        }
    }
    let mut removed = Vec::new();
    sheet.registered_glyphs.retain(|glyph| {
        if keep.contains(&glyph.id) {
            true
        } else {
            removed.push(glyph.id);
            false
        }
    });
    for &glyph_id in &removed {
        sheet
            .mutations
            .push(ReductionMutation::GlyphRemoved { glyph_id });
    }
    removed
}

fn apply_delta(
    sheet: &mut NeutralReductionSheet,
    system_id: usize,
    delta: ReductionDelta,
) -> Result<(), ReductionContractError> {
    let system_index = sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(ReductionContractError::UnknownSystem(system_id))?;
    for mutation in delta.mutations {
        match mutation {
            ReductionDeltaMutation::AddInter(inter) => {
                if inter.id != sheet.next_inter_id {
                    return Err(ReductionContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    });
                }
                if sheet.live_inter_ids.contains(&inter.id)
                    || sheet.retired_inter_ids.contains(&inter.id)
                {
                    return Err(ReductionContractError::DuplicateInter(inter.id));
                }
                if let Some(glyph_id) = inter.glyph_id
                    && !sheet
                        .registered_glyphs
                        .iter()
                        .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(ReductionContractError::UnknownGlyph(glyph_id));
                }
                let staff_index = if let Some(staff_id) = inter.staff_id {
                    let index = sheet.systems[system_index]
                        .staves
                        .iter()
                        .position(|staff| staff.id == staff_id)
                        .ok_or(ReductionContractError::UnknownStaff {
                            system_id,
                            staff_id,
                        })?;
                    matches!(inter.kind, NeutralReductionInterKind::Note).then_some(index)
                } else {
                    None
                };
                let mut members = BTreeSet::new();
                for &member_id in &inter.member_ids {
                    if !members.insert(member_id) {
                        return Err(ReductionContractError::DuplicateMember {
                            owner_id: inter.id,
                            member_id,
                        });
                    }
                    require_inter(&sheet.systems[system_index], member_id)?;
                }
                let member_ids = inter.member_ids.clone();
                sheet.next_inter_id = sheet.next_inter_id.checked_add(1).ok_or(
                    ReductionContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    },
                )?;
                let inter_id = inter.id;
                sheet.systems[system_index].inters.push(inter);
                sheet.live_inter_ids.push(inter_id);
                sheet.mutations.push(ReductionMutation::InterAdded {
                    system_id,
                    inter_id,
                });
                for member_id in member_ids {
                    sheet.mutations.push(ReductionMutation::MemberAttached {
                        system_id,
                        owner_id: inter_id,
                        member_id,
                    });
                }
                if let Some(staff_index) = staff_index {
                    let staff_id = sheet.systems[system_index].staves[staff_index].id;
                    sheet.systems[system_index].staves[staff_index]
                        .note_ids
                        .push(inter_id);
                    sheet
                        .mutations
                        .push(ReductionMutation::StaffNoteAttached { staff_id, inter_id });
                }
            }
            ReductionDeltaMutation::RemoveInter(inter_id) => {
                remove_inter(sheet, system_index, inter_id)?;
            }
            ReductionDeltaMutation::AddRelation(relation) => {
                for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                    if !sheet.systems[system_index]
                        .inters
                        .iter()
                        .any(|inter| inter.id == endpoint)
                    {
                        return Err(ReductionContractError::UnknownRelationEndpoint(endpoint));
                    }
                }
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(ReductionContractError::DuplicateRelation {
                        system_id,
                        relation_id: relation.id,
                    });
                }
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(ReductionMutation::RelationAdded {
                    system_id,
                    relation,
                });
            }
            ReductionDeltaMutation::RemoveRelation(relation_id) => {
                let relation_index = sheet.systems[system_index]
                    .relations
                    .iter()
                    .position(|relation| relation.id == relation_id)
                    .ok_or(ReductionContractError::UnknownRelation {
                        system_id,
                        relation_id,
                    })?;
                let relation = sheet.systems[system_index].relations.remove(relation_index);
                sheet.mutations.push(ReductionMutation::RelationRemoved {
                    system_id,
                    relation,
                });
            }
            ReductionDeltaMutation::AttachMember {
                owner_id,
                member_id,
            } => {
                require_inter(&sheet.systems[system_index], member_id)?;
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(ReductionContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                if owner.member_ids.contains(&member_id) {
                    return Err(ReductionContractError::DuplicateMember {
                        owner_id,
                        member_id,
                    });
                }
                owner.member_ids.push(member_id);
                sheet.mutations.push(ReductionMutation::MemberAttached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
            ReductionDeltaMutation::DetachMember {
                owner_id,
                member_id,
            } => {
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(ReductionContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                let member_index = owner
                    .member_ids
                    .iter()
                    .position(|id| *id == member_id)
                    .ok_or(ReductionContractError::UnknownMember {
                        owner_id,
                        member_id,
                    })?;
                owner.member_ids.remove(member_index);
                sheet.mutations.push(ReductionMutation::MemberDetached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
            ReductionDeltaMutation::SetStemGeometry {
                stem_id,
                head_end,
                tail_end,
            } => {
                let stem = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == stem_id)
                    .ok_or(ReductionContractError::UnknownInter {
                        system_id,
                        inter_id: stem_id,
                    })?;
                let NeutralReductionInterKind::Stem {
                    head_end: current_head,
                    tail_end: current_tail,
                    ..
                } = &mut stem.kind
                else {
                    return Err(ReductionContractError::InvalidStem(stem_id));
                };
                *current_head = head_end;
                *current_tail = tail_end;
                sheet.mutations.push(ReductionMutation::StemGeometrySet {
                    system_id,
                    stem_id,
                    head_end,
                    tail_end,
                });
            }
            ReductionDeltaMutation::SetStemFreeLength {
                stem_id,
                free_length,
            } => {
                let stem = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == stem_id)
                    .ok_or(ReductionContractError::UnknownInter {
                        system_id,
                        inter_id: stem_id,
                    })?;
                let NeutralReductionInterKind::Stem {
                    free_length: current,
                    ..
                } = &mut stem.kind
                else {
                    return Err(ReductionContractError::InvalidStem(stem_id));
                };
                *current = free_length;
                sheet.mutations.push(ReductionMutation::StemFreeLengthSet {
                    system_id,
                    stem_id,
                    free_length,
                });
            }
        }
    }
    Ok(())
}

fn require_inter(
    system: &NeutralReductionSystem,
    inter_id: usize,
) -> Result<(), ReductionContractError> {
    system
        .inters
        .iter()
        .any(|inter| inter.id == inter_id)
        .then_some(())
        .ok_or(ReductionContractError::UnknownInter {
            system_id: system.id,
            inter_id,
        })
}

fn remove_inter(
    sheet: &mut NeutralReductionSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), ReductionContractError> {
    let system_id = sheet.systems[system_index].id;
    let inter_index = sheet.systems[system_index]
        .inters
        .iter()
        .position(|inter| inter.id == inter_id)
        .ok_or(ReductionContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    if matches!(
        sheet.systems[system_index].inters[inter_index].kind,
        NeutralReductionInterKind::Note
    ) && let Some(staff_id) = sheet.systems[system_index].inters[inter_index].staff_id
    {
        let staff = sheet.systems[system_index]
            .staves
            .iter_mut()
            .find(|staff| staff.id == staff_id)
            .ok_or(ReductionContractError::UnknownStaff {
                system_id,
                staff_id,
            })?;
        let note_index = staff
            .note_ids
            .iter()
            .position(|id| *id == inter_id)
            .ok_or(ReductionContractError::InvalidStaffNote { staff_id, inter_id })?;
        staff.note_ids.remove(note_index);
        sheet
            .mutations
            .push(ReductionMutation::StaffNoteDetached { staff_id, inter_id });
    }
    let owned_member_ids = sheet.systems[system_index].inters[inter_index]
        .member_ids
        .clone();
    for member_id in owned_member_ids {
        sheet.mutations.push(ReductionMutation::MemberDetached {
            system_id,
            owner_id: inter_id,
            member_id,
        });
    }
    let relations = sheet.systems[system_index]
        .relations
        .iter()
        .copied()
        .filter(|relation| {
            relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
        })
        .collect::<Vec<_>>();
    for relation in relations {
        let index = sheet.systems[system_index]
            .relations
            .iter()
            .position(|candidate| *candidate == relation)
            .expect("relation snapshot remains live");
        sheet.systems[system_index].relations.remove(index);
        sheet.mutations.push(ReductionMutation::RelationRemoved {
            system_id,
            relation,
        });
    }
    let owner_ids = sheet.systems[system_index]
        .inters
        .iter()
        .filter(|owner| owner.member_ids.contains(&inter_id))
        .map(|owner| owner.id)
        .collect::<Vec<_>>();
    for owner_id in owner_ids {
        let owner = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|owner| owner.id == owner_id)
            .expect("owner snapshot remains live");
        owner.member_ids.retain(|id| *id != inter_id);
        sheet.mutations.push(ReductionMutation::MemberDetached {
            system_id,
            owner_id,
            member_id: inter_id,
        });
    }
    sheet.systems[system_index].inters.remove(inter_index);
    let live_index = sheet
        .live_inter_ids
        .iter()
        .position(|id| *id == inter_id)
        .ok_or(ReductionContractError::InvalidLiveInterIndex)?;
    sheet.live_inter_ids.remove(live_index);
    sheet.retired_inter_ids.push(inter_id);
    sheet.mutations.push(ReductionMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn validate_sheet(sheet: &NeutralReductionSheet) -> Result<(), ReductionContractError> {
    if !sheet.interline.is_finite() || sheet.interline <= 0.0 {
        return Err(ReductionContractError::InvalidInterline);
    }
    let mut glyph_ids = BTreeSet::new();
    let mut previous_glyph = None;
    for glyph in &sheet.registered_glyphs {
        if !glyph_ids.insert(glyph.id) {
            return Err(ReductionContractError::DuplicateGlyph(glyph.id));
        }
        if previous_glyph.is_some_and(|previous| previous >= glyph.id) {
            return Err(ReductionContractError::UnorderedGlyphIndex);
        }
        previous_glyph = Some(glyph.id);
    }
    let mut system_ids = BTreeSet::new();
    let mut staff_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    let mut expected_notes = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(ReductionContractError::DuplicateSystem(system.id));
        }
        for staff in &system.staves {
            if !staff_ids.insert(staff.id) {
                return Err(ReductionContractError::DuplicateStaff(staff.id));
            }
            for &glyph_id in &staff.line_glyph_ids {
                if !glyph_ids.contains(&glyph_id) {
                    return Err(ReductionContractError::UnknownGlyph(glyph_id));
                }
            }
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(ReductionContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(ReductionContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let Some(glyph_id) = inter.glyph_id
                && !glyph_ids.contains(&glyph_id)
            {
                return Err(ReductionContractError::UnknownGlyph(glyph_id));
            }
            if let Some(staff_id) = inter.staff_id {
                if !system.staves.iter().any(|staff| staff.id == staff_id) {
                    return Err(ReductionContractError::UnknownStaff {
                        system_id: system.id,
                        staff_id,
                    });
                }
                if matches!(inter.kind, NeutralReductionInterKind::Note) {
                    expected_notes.insert((staff_id, inter.id));
                }
            }
            let mut members = BTreeSet::new();
            for &member_id in &inter.member_ids {
                if !members.insert(member_id) {
                    return Err(ReductionContractError::DuplicateMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
                if !system.inters.iter().any(|member| member.id == member_id) {
                    return Err(ReductionContractError::UnknownMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
            }
        }
        let mut relation_ids = BTreeSet::new();
        for relation in &system.relations {
            if !relation_ids.insert(relation.id) {
                return Err(ReductionContractError::DuplicateRelation {
                    system_id: system.id,
                    relation_id: relation.id,
                });
            }
            for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                if !system.inters.iter().any(|inter| inter.id == endpoint) {
                    return Err(ReductionContractError::UnknownRelationEndpoint(endpoint));
                }
            }
        }
    }
    let live = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live.len() != sheet.live_inter_ids.len() || live != inter_ids {
        return Err(ReductionContractError::InvalidLiveInterIndex);
    }
    let mut retired = BTreeSet::new();
    for &inter_id in &sheet.retired_inter_ids {
        if inter_id >= sheet.next_inter_id
            || inter_ids.contains(&inter_id)
            || !retired.insert(inter_id)
        {
            return Err(ReductionContractError::InvalidRetiredInter(inter_id));
        }
    }
    let mut actual_notes = BTreeSet::new();
    for system in &sheet.systems {
        for staff in &system.staves {
            for &inter_id in &staff.note_ids {
                if !actual_notes.insert((staff.id, inter_id)) {
                    return Err(ReductionContractError::DuplicateStaffNote {
                        staff_id: staff.id,
                        inter_id,
                    });
                }
            }
        }
    }
    if actual_notes != expected_notes {
        let &(staff_id, inter_id) = expected_notes
            .symmetric_difference(&actual_notes)
            .next()
            .expect("different sets have a difference");
        return Err(ReductionContractError::InvalidStaffNote { staff_id, inter_id });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeReduction {
        calls: Vec<String>,
        foundations: BTreeMap<usize, ReductionStageOutcome<&'static str>>,
        heads: BTreeMap<usize, ReductionStageOutcome<&'static str>>,
        tails: BTreeMap<usize, ReductionStageOutcome<&'static str>>,
        groups: BTreeMap<usize, ReductionStageOutcome<&'static str>>,
    }

    impl VisualReduction for FakeReduction {
        type Error = &'static str;

        fn reduce_foundations(
            &mut self,
            input: ReductionSystemInput<'_>,
        ) -> ReductionStageOutcome<Self::Error> {
            assert!(input.purge_weaks);
            self.calls.push(format!("reduce{}", input.system_id));
            self.foundations
                .remove(&input.system_id)
                .unwrap_or_else(|| ReductionStageOutcome::success(ReductionDelta::default()))
        }

        fn refine_stem_head_end(
            &mut self,
            input: ReductionStemInput<'_>,
        ) -> ReductionStageOutcome<Self::Error> {
            self.calls.push(format!("head{}", input.stem_id));
            self.heads
                .remove(&input.stem_id)
                .unwrap_or_else(|| ReductionStageOutcome::success(ReductionDelta::default()))
        }

        fn refine_stem_tail_end(
            &mut self,
            input: ReductionStemInput<'_>,
        ) -> ReductionStageOutcome<Self::Error> {
            self.calls.push(format!("tail{}", input.stem_id));
            self.tails
                .remove(&input.stem_id)
                .unwrap_or_else(|| ReductionStageOutcome::success(ReductionDelta::default()))
        }

        fn check_beam_groups(
            &mut self,
            input: ReductionBeamGroupsInput<'_>,
        ) -> ReductionStageOutcome<Self::Error> {
            self.calls.push(format!("groups{}", input.system_id));
            self.groups
                .remove(&input.system_id)
                .unwrap_or_else(|| ReductionStageOutcome::success(ReductionDelta::default()))
        }
    }

    fn stem(id: usize, glyph_id: usize, free_length: Option<i32>) -> NeutralReductionInter {
        NeutralReductionInter {
            id,
            kind: NeutralReductionInterKind::Stem {
                free_length,
                head_end: 0.0,
                tail_end: 10.0,
            },
            glyph_id: Some(glyph_id),
            staff_id: None,
            member_ids: Vec::new(),
        }
    }

    fn sheet() -> NeutralReductionSheet {
        NeutralReductionSheet {
            id: 7,
            interline: 10.0,
            systems: vec![
                NeutralReductionSystem {
                    id: 2,
                    staves: vec![NeutralReductionStaff {
                        id: 20,
                        line_glyph_ids: vec![1],
                        note_ids: Vec::new(),
                    }],
                    inters: vec![stem(1, 2, Some(4)), stem(2, 4, Some(8))],
                    relations: Vec::new(),
                },
                NeutralReductionSystem {
                    id: 1,
                    staves: vec![NeutralReductionStaff {
                        id: 10,
                        line_glyph_ids: vec![5],
                        note_ids: Vec::new(),
                    }],
                    inters: vec![stem(3, 6, None)],
                    relations: Vec::new(),
                },
            ],
            registered_glyphs: (1..=7).map(|id| NeutralReductionGlyph { id }).collect(),
            live_inter_ids: vec![1, 2, 3],
            retired_inter_ids: Vec::new(),
            next_inter_id: 4,
            stem_free_length_median: None,
            mutations: Vec::new(),
        }
    }

    #[test]
    fn exact_system_refinement_epilog_median_and_cleanup_order() {
        let visual = FakeReduction::default();
        let mut step = HeadlessReductionStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            step.visual().calls,
            [
                "reduce2", "head1", "head2", "reduce1", "head3", "groups2", "groups1"
            ]
        );
        assert_eq!(
            report.stem_free_length_median,
            Some(StemFreeLengthMedian {
                pixels: 8,
                interlines: 0.8,
            })
        );
        assert_eq!(report.removed_glyph_ids, [3, 7]);
        assert_eq!(
            sheet
                .registered_glyphs
                .iter()
                .map(|glyph| glyph.id)
                .collect::<Vec<_>>(),
            [1, 2, 4, 5, 6]
        );
    }

    #[test]
    fn checked_foundation_failure_keeps_prefix_skips_refinement_and_continues() {
        let mut visual = FakeReduction::default();
        visual.foundations.insert(
            2,
            ReductionStageOutcome {
                delta: ReductionDelta {
                    mutations: vec![ReductionDeltaMutation::SetStemGeometry {
                        stem_id: 1,
                        head_end: 2.0,
                        tail_end: 10.0,
                    }],
                },
                failure: Some(ReductionFailure::Checked("checked")),
            },
        );
        let mut step = HeadlessReductionStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, [(2, "checked")]);
        assert_eq!(
            step.visual().calls,
            ["reduce2", "reduce1", "head3", "groups2", "groups1"]
        );
        assert!(sheet.mutations.iter().any(|mutation| matches!(
            mutation,
            ReductionMutation::StemGeometrySet {
                stem_id: 1,
                head_end: 2.0,
                ..
            }
        )));
    }

    #[test]
    fn fatal_foundation_keeps_prefix_and_skips_later_system_and_epilog() {
        let mut visual = FakeReduction::default();
        visual.foundations.insert(
            2,
            ReductionStageOutcome {
                delta: ReductionDelta {
                    mutations: vec![ReductionDeltaMutation::RemoveInter(2)],
                },
                failure: Some(ReductionFailure::Fatal("boom")),
            },
        );
        let mut step = HeadlessReductionStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(ReductionStepError::FatalSystem {
                system_id: 2,
                stage: ReductionStage::Foundations,
                source: "boom",
                checked_errors: Vec::new(),
            })
        );
        assert_eq!(step.visual().calls, ["reduce2"]);
        assert_eq!(sheet.live_inter_ids, [1, 3]);
        assert_eq!(sheet.retired_inter_ids, [2]);
        assert_eq!(
            sheet
                .registered_glyphs
                .iter()
                .map(|glyph| glyph.id)
                .collect::<Vec<_>>(),
            (1..=7).collect::<Vec<_>>()
        );
    }

    #[test]
    fn removal_cascades_relation_staff_and_ensemble_ownership_before_hole() {
        let mut sheet = sheet();
        let note = NeutralReductionInter {
            id: 4,
            kind: NeutralReductionInterKind::Note,
            glyph_id: Some(3),
            staff_id: Some(20),
            member_ids: Vec::new(),
        };
        sheet.systems[0].inters.push(note);
        sheet.systems[0].staves[0].note_ids.push(4);
        sheet.systems[0].inters.push(NeutralReductionInter {
            id: 5,
            kind: NeutralReductionInterKind::BeamGroup,
            glyph_id: None,
            staff_id: None,
            member_ids: vec![4],
        });
        sheet.systems[0].relations.push(NeutralReductionRelation {
            id: 1,
            source_inter_id: 4,
            target_inter_id: 1,
            kind: NeutralReductionRelationKind::ExclusionOverlap,
        });
        sheet.live_inter_ids.extend([4, 5]);
        sheet.next_inter_id = 6;
        let mut visual = FakeReduction::default();
        visual.foundations.insert(
            2,
            ReductionStageOutcome::success(ReductionDelta {
                mutations: vec![ReductionDeltaMutation::RemoveInter(4)],
            }),
        );
        let mut step = HeadlessReductionStep::new(visual);

        step.process(&mut sheet).unwrap();

        assert!(!sheet.systems[0].staves[0].note_ids.contains(&4));
        assert!(!sheet.systems[0].inters[2].member_ids.contains(&4));
        assert!(sheet.systems[0].relations.is_empty());
        assert_eq!(sheet.retired_inter_ids, [4]);
        let suffix = sheet
            .mutations
            .iter()
            .filter(|mutation| {
                matches!(
                    mutation,
                    ReductionMutation::StaffNoteDetached { inter_id: 4, .. }
                        | ReductionMutation::RelationRemoved { .. }
                        | ReductionMutation::MemberDetached { member_id: 4, .. }
                        | ReductionMutation::InterRemoved { inter_id: 4, .. }
                )
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            suffix.as_slice(),
            [
                ReductionMutation::StaffNoteDetached { .. },
                ReductionMutation::RelationRemoved { .. },
                ReductionMutation::MemberDetached { .. },
                ReductionMutation::InterRemoved { .. }
            ]
        ));
    }

    #[test]
    fn epilog_failure_keeps_group_prefix_and_skips_median_and_cleanup() {
        let mut visual = FakeReduction::default();
        visual.groups.insert(
            2,
            ReductionStageOutcome {
                delta: ReductionDelta {
                    mutations: vec![ReductionDeltaMutation::SetStemGeometry {
                        stem_id: 1,
                        head_end: 1.0,
                        tail_end: 9.0,
                    }],
                },
                failure: Some(ReductionFailure::Fatal("group split")),
            },
        );
        let mut step = HeadlessReductionStep::new(visual);
        let mut sheet = sheet();

        assert!(matches!(
            step.process(&mut sheet),
            Err(ReductionStepError::Epilog {
                system_id: 2,
                source: "group split",
                ..
            })
        ));
        assert_eq!(
            step.visual().calls,
            ["reduce2", "head1", "head2", "reduce1", "head3", "groups2"]
        );
        assert!(sheet.stem_free_length_median.is_none());
        assert_eq!(sheet.registered_glyphs.len(), 7);
    }
}
