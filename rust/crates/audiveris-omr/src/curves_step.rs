// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `CurvesStep` and `Curves.buildCurves`.
//!
//! Skeleton/arc retrieval and curve classification remain one injected visual
//! seam. This module owns exact phase order, temporary segment state, glyph and
//! SIG ownership, slur/tie relations, the two Java-caught builder failures,
//! fatal mutation prefixes, and cleanup of sheet-local working state.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CurvesPhase {
    BuildSkeleton,
    RetrieveJunctions,
    RetrieveArcs,
    BuildSlurs,
    BuildSegments,
    BuildWedges,
    BuildEndings,
    BuildRehearsals,
}

impl CurvesPhase {
    pub const ORDER: [Self; 8] = [
        Self::BuildSkeleton,
        Self::RetrieveJunctions,
        Self::RetrieveArcs,
        Self::BuildSlurs,
        Self::BuildSegments,
        Self::BuildWedges,
        Self::BuildEndings,
        Self::BuildRehearsals,
    ];

    const fn catches_throwable(self) -> bool {
        matches!(self, Self::BuildSlurs | Self::BuildSegments)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveGlyphKind {
    Slur,
    Segment,
    Wedge,
    Ending,
    Rehearsal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCurveGlyph {
    pub id: usize,
    pub kind: CurveGlyphKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCurveInterKind {
    Slur { tie: bool },
    Wedge,
    Ending,
    Rehearsal,
    Word,
    Head,
    Barline,
    Other(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCurveInter {
    pub id: usize,
    pub kind: NeutralCurveInterKind,
    pub glyph_id: Option<usize>,
    pub staff_id: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCurveRelationKind {
    SlurHead(CurveSide),
    EndingBarline(CurveSide),
    EndingSentence,
    Containment,
    NoExclusion,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralCurveRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralCurveRelationKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralWorkingSegment {
    pub id: usize,
    pub glyph_id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCurvesSystem {
    pub id: usize,
    pub staff_ids: Vec<usize>,
    pub inters: Vec<NeutralCurveInter>,
    pub relations: Vec<NeutralCurveRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCurvesSheet {
    pub id: usize,
    pub systems: Vec<NeutralCurvesSystem>,
    pub registered_glyphs: Vec<NeutralCurveGlyph>,
    pub next_glyph_id: usize,
    pub live_inter_ids: Vec<usize>,
    pub retired_inter_ids: Vec<usize>,
    pub next_inter_id: usize,
    pub live_relation_ids: Vec<usize>,
    pub retired_relation_ids: Vec<usize>,
    pub next_relation_id: usize,
    pub mutations: Vec<CurvesMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurvesContext {
    pub skeleton_ready: bool,
    pub junctions_ready: bool,
    pub arcs_ready: bool,
    pub working_segments: Vec<NeutralWorkingSegment>,
    pub next_segment_id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurvesMutation {
    SkeletonBuilt,
    JunctionsRetrieved,
    ArcsRetrieved,
    GlyphRegistered {
        glyph_id: usize,
    },
    SegmentAdded {
        segment_id: usize,
    },
    SegmentConsumed {
        segment_id: usize,
    },
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterGlyphSet {
        system_id: usize,
        inter_id: usize,
        glyph_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    TieSet {
        system_id: usize,
        inter_id: usize,
        tie: bool,
    },
    RelationAdded {
        system_id: usize,
        relation: NeutralCurveRelation,
    },
    RelationRemoved {
        system_id: usize,
        relation: NeutralCurveRelation,
    },
    PhaseCaught {
        phase: CurvesPhase,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurvesDelta {
    /// Exact successful mutation prefix before a caught or fatal failure.
    pub mutations: Vec<CurvesDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurvesDeltaMutation {
    MarkSkeletonBuilt,
    MarkJunctionsRetrieved,
    MarkArcsRetrieved,
    RegisterGlyph(NeutralCurveGlyph),
    AddSegment(NeutralWorkingSegment),
    ConsumeSegment(usize),
    AddInter {
        system_id: usize,
        inter: NeutralCurveInter,
    },
    SetInterGlyph {
        system_id: usize,
        inter_id: usize,
        glyph_id: usize,
    },
    RemoveInter {
        system_id: usize,
        inter_id: usize,
    },
    SetTie {
        system_id: usize,
        inter_id: usize,
        tie: bool,
    },
    AddRelation {
        system_id: usize,
        relation: NeutralCurveRelation,
    },
    RemoveRelation {
        system_id: usize,
        relation_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurvesFailure<E> {
    /// Java catches `Throwable` inside SlursBuilder and SegmentsBuilder only.
    Caught(E),
    Fatal(E),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurvesPhaseOutcome<E> {
    pub delta: CurvesDelta,
    pub failure: Option<CurvesFailure<E>>,
}

impl<E> CurvesPhaseOutcome<E> {
    #[must_use]
    pub fn success(delta: CurvesDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CurvesPhaseInput<'a> {
    pub sheet: &'a NeutralCurvesSheet,
    pub phase: CurvesPhase,
    pub context: &'a CurvesContext,
}

/// First unavailable dependency: construction of the cleaned sheet skeleton.
pub trait VisualCurves {
    type Error;

    fn run_phase(&mut self, input: CurvesPhaseInput<'_>) -> CurvesPhaseOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurvesCleanup {
    pub skeleton_discarded: bool,
    pub arcs_discarded: bool,
    pub discarded_segment_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurvesReport<E> {
    pub caught_errors: Vec<(CurvesPhase, E)>,
    pub cleanup: CurvesCleanup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurvesStepError<E> {
    FatalPhase {
        phase: CurvesPhase,
        source: E,
        caught_errors: Vec<(CurvesPhase, E)>,
        context: CurvesContext,
    },
    Contract(CurvesContractError),
}

impl<E: fmt::Display> fmt::Display for CurvesStepError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FatalPhase { phase, source, .. } => {
                write!(f, "curves phase {phase:?} failed: {source}")
            }
            Self::Contract(source) => write!(f, "curves contract failed: {source}"),
        }
    }
}

impl<E: Error + 'static> Error for CurvesStepError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FatalPhase { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurvesContractError {
    DuplicateSystem(usize),
    DuplicateStaff(usize),
    DuplicateGlyph(usize),
    InvalidGlyphAllocator {
        expected: usize,
        actual: usize,
    },
    DuplicateInter(usize),
    InvalidInterAllocator {
        expected: usize,
        actual: usize,
    },
    InvalidLiveInterIndex,
    InvalidRetiredInter(usize),
    DuplicateRelation(usize),
    InvalidRelationAllocator {
        expected: usize,
        actual: usize,
    },
    InvalidLiveRelationIndex,
    InvalidRetiredRelation(usize),
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
    UnknownRelationEndpoint(usize),
    InvalidTieTarget(usize),
    InvalidPhaseMutation {
        phase: CurvesPhase,
    },
    InvalidCaughtPhase(CurvesPhase),
    DuplicateSegment(usize),
    InvalidSegmentAllocator {
        expected: usize,
        actual: usize,
    },
    UnknownSegment(usize),
}

impl fmt::Display for CurvesContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for CurvesContractError {}

pub struct HeadlessCurvesStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessCurvesStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualCurves> HeadlessCurvesStep<Visual> {
    /// Java `new Curves(sheet).buildCurves()` in exact phase order.
    pub fn process(
        &mut self,
        sheet: &mut NeutralCurvesSheet,
    ) -> Result<CurvesReport<Visual::Error>, CurvesStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(CurvesStepError::Contract)?;
        let mut context = CurvesContext {
            next_segment_id: 1,
            ..CurvesContext::default()
        };
        let mut caught_errors = Vec::new();
        for phase in CurvesPhase::ORDER {
            let outcome = self.visual.run_phase(CurvesPhaseInput {
                sheet,
                phase,
                context: &context,
            });
            apply_delta(sheet, phase, &mut context, outcome.delta)
                .map_err(CurvesStepError::Contract)?;
            match outcome.failure {
                None => {}
                Some(CurvesFailure::Caught(source)) => {
                    if !phase.catches_throwable() {
                        return Err(CurvesStepError::Contract(
                            CurvesContractError::InvalidCaughtPhase(phase),
                        ));
                    }
                    sheet.mutations.push(CurvesMutation::PhaseCaught { phase });
                    caught_errors.push((phase, source));
                }
                Some(CurvesFailure::Fatal(source)) => {
                    return Err(CurvesStepError::FatalPhase {
                        phase,
                        source,
                        caught_errors,
                        context,
                    });
                }
            }
        }
        Ok(CurvesReport {
            caught_errors,
            cleanup: CurvesCleanup {
                skeleton_discarded: context.skeleton_ready,
                arcs_discarded: context.arcs_ready,
                discarded_segment_ids: context
                    .working_segments
                    .iter()
                    .map(|segment| segment.id)
                    .collect(),
            },
        })
    }
}

fn apply_delta(
    sheet: &mut NeutralCurvesSheet,
    phase: CurvesPhase,
    context: &mut CurvesContext,
    delta: CurvesDelta,
) -> Result<(), CurvesContractError> {
    for mutation in delta.mutations {
        match mutation {
            CurvesDeltaMutation::MarkSkeletonBuilt => {
                if phase != CurvesPhase::BuildSkeleton || context.skeleton_ready {
                    return Err(CurvesContractError::InvalidPhaseMutation { phase });
                }
                context.skeleton_ready = true;
                sheet.mutations.push(CurvesMutation::SkeletonBuilt);
            }
            CurvesDeltaMutation::MarkJunctionsRetrieved => {
                if phase != CurvesPhase::RetrieveJunctions
                    || !context.skeleton_ready
                    || context.junctions_ready
                {
                    return Err(CurvesContractError::InvalidPhaseMutation { phase });
                }
                context.junctions_ready = true;
                sheet.mutations.push(CurvesMutation::JunctionsRetrieved);
            }
            CurvesDeltaMutation::MarkArcsRetrieved => {
                if phase != CurvesPhase::RetrieveArcs
                    || !context.junctions_ready
                    || context.arcs_ready
                {
                    return Err(CurvesContractError::InvalidPhaseMutation { phase });
                }
                context.arcs_ready = true;
                sheet.mutations.push(CurvesMutation::ArcsRetrieved);
            }
            CurvesDeltaMutation::RegisterGlyph(glyph) => {
                if glyph.id != sheet.next_glyph_id {
                    return Err(CurvesContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    });
                }
                if sheet.registered_glyphs.iter().any(|old| old.id == glyph.id) {
                    return Err(CurvesContractError::DuplicateGlyph(glyph.id));
                }
                sheet.next_glyph_id = sheet.next_glyph_id.checked_add(1).ok_or(
                    CurvesContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    },
                )?;
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet
                    .mutations
                    .push(CurvesMutation::GlyphRegistered { glyph_id });
            }
            CurvesDeltaMutation::AddSegment(segment) => {
                if phase != CurvesPhase::BuildSegments {
                    return Err(CurvesContractError::InvalidPhaseMutation { phase });
                }
                if segment.id != context.next_segment_id {
                    return Err(CurvesContractError::InvalidSegmentAllocator {
                        expected: context.next_segment_id,
                        actual: segment.id,
                    });
                }
                if !sheet
                    .registered_glyphs
                    .iter()
                    .any(|glyph| glyph.id == segment.glyph_id)
                {
                    return Err(CurvesContractError::UnknownGlyph(segment.glyph_id));
                }
                if context
                    .working_segments
                    .iter()
                    .any(|old| old.id == segment.id)
                {
                    return Err(CurvesContractError::DuplicateSegment(segment.id));
                }
                context.next_segment_id = context.next_segment_id.checked_add(1).ok_or(
                    CurvesContractError::InvalidSegmentAllocator {
                        expected: context.next_segment_id,
                        actual: segment.id,
                    },
                )?;
                let segment_id = segment.id;
                context.working_segments.push(segment);
                sheet
                    .mutations
                    .push(CurvesMutation::SegmentAdded { segment_id });
            }
            CurvesDeltaMutation::ConsumeSegment(segment_id) => {
                if phase != CurvesPhase::BuildWedges {
                    return Err(CurvesContractError::InvalidPhaseMutation { phase });
                }
                let index = context
                    .working_segments
                    .iter()
                    .position(|segment| segment.id == segment_id)
                    .ok_or(CurvesContractError::UnknownSegment(segment_id))?;
                context.working_segments.remove(index);
                sheet
                    .mutations
                    .push(CurvesMutation::SegmentConsumed { segment_id });
            }
            CurvesDeltaMutation::AddInter { system_id, inter } => {
                add_inter(sheet, system_id, inter)?;
            }
            CurvesDeltaMutation::SetInterGlyph {
                system_id,
                inter_id,
                glyph_id,
            } => set_inter_glyph(sheet, system_id, inter_id, glyph_id)?,
            CurvesDeltaMutation::RemoveInter {
                system_id,
                inter_id,
            } => remove_inter(sheet, system_id, inter_id)?,
            CurvesDeltaMutation::SetTie {
                system_id,
                inter_id,
                tie,
            } => set_tie(sheet, system_id, inter_id, tie)?,
            CurvesDeltaMutation::AddRelation {
                system_id,
                relation,
            } => add_relation(sheet, system_id, relation)?,
            CurvesDeltaMutation::RemoveRelation {
                system_id,
                relation_id,
            } => remove_relation(sheet, system_id, relation_id)?,
        }
    }
    Ok(())
}

fn system_index(
    sheet: &NeutralCurvesSheet,
    system_id: usize,
) -> Result<usize, CurvesContractError> {
    sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(CurvesContractError::UnknownSystem(system_id))
}

fn add_inter(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    inter: NeutralCurveInter,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    if inter.id != sheet.next_inter_id {
        return Err(CurvesContractError::InvalidInterAllocator {
            expected: sheet.next_inter_id,
            actual: inter.id,
        });
    }
    if sheet.live_inter_ids.contains(&inter.id) || sheet.retired_inter_ids.contains(&inter.id) {
        return Err(CurvesContractError::DuplicateInter(inter.id));
    }
    if let Some(glyph_id) = inter.glyph_id
        && !sheet
            .registered_glyphs
            .iter()
            .any(|glyph| glyph.id == glyph_id)
    {
        return Err(CurvesContractError::UnknownGlyph(glyph_id));
    }
    if let Some(staff_id) = inter.staff_id
        && !sheet.systems[index].staff_ids.contains(&staff_id)
    {
        return Err(CurvesContractError::UnknownStaff {
            system_id,
            staff_id,
        });
    }
    let inter_id = inter.id;
    sheet.next_inter_id =
        sheet
            .next_inter_id
            .checked_add(1)
            .ok_or(CurvesContractError::InvalidInterAllocator {
                expected: sheet.next_inter_id,
                actual: inter.id,
            })?;
    sheet.systems[index].inters.push(inter);
    sheet.live_inter_ids.push(inter_id);
    sheet.mutations.push(CurvesMutation::InterAdded {
        system_id,
        inter_id,
    });
    Ok(())
}

fn set_inter_glyph(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    inter_id: usize,
    glyph_id: usize,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    if !sheet
        .registered_glyphs
        .iter()
        .any(|glyph| glyph.id == glyph_id)
    {
        return Err(CurvesContractError::UnknownGlyph(glyph_id));
    }
    let inter = sheet.systems[index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == inter_id)
        .ok_or(CurvesContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    inter.glyph_id = Some(glyph_id);
    sheet.mutations.push(CurvesMutation::InterGlyphSet {
        system_id,
        inter_id,
        glyph_id,
    });
    Ok(())
}

fn set_tie(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    inter_id: usize,
    tie: bool,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    let inter = sheet.systems[index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == inter_id)
        .ok_or(CurvesContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    if !matches!(inter.kind, NeutralCurveInterKind::Slur { .. }) {
        return Err(CurvesContractError::InvalidTieTarget(inter_id));
    }
    inter.kind = NeutralCurveInterKind::Slur { tie };
    sheet.mutations.push(CurvesMutation::TieSet {
        system_id,
        inter_id,
        tie,
    });
    Ok(())
}

fn add_relation(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    relation: NeutralCurveRelation,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    if relation.id != sheet.next_relation_id {
        return Err(CurvesContractError::InvalidRelationAllocator {
            expected: sheet.next_relation_id,
            actual: relation.id,
        });
    }
    if sheet.live_relation_ids.contains(&relation.id)
        || sheet.retired_relation_ids.contains(&relation.id)
    {
        return Err(CurvesContractError::DuplicateRelation(relation.id));
    }
    for endpoint in [relation.source_inter_id, relation.target_inter_id] {
        if !sheet.systems[index]
            .inters
            .iter()
            .any(|inter| inter.id == endpoint)
        {
            return Err(CurvesContractError::UnknownRelationEndpoint(endpoint));
        }
    }
    sheet.next_relation_id = sheet.next_relation_id.checked_add(1).ok_or(
        CurvesContractError::InvalidRelationAllocator {
            expected: sheet.next_relation_id,
            actual: relation.id,
        },
    )?;
    sheet.systems[index].relations.push(relation);
    sheet.live_relation_ids.push(relation.id);
    sheet.mutations.push(CurvesMutation::RelationAdded {
        system_id,
        relation,
    });
    Ok(())
}

fn remove_relation(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    relation_id: usize,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    let relation_index = sheet.systems[index]
        .relations
        .iter()
        .position(|relation| relation.id == relation_id)
        .ok_or(CurvesContractError::UnknownRelation {
            system_id,
            relation_id,
        })?;
    let relation = sheet.systems[index].relations.remove(relation_index);
    let live_index = sheet
        .live_relation_ids
        .iter()
        .position(|id| *id == relation_id)
        .ok_or(CurvesContractError::InvalidLiveRelationIndex)?;
    sheet.live_relation_ids.remove(live_index);
    sheet.retired_relation_ids.push(relation_id);
    sheet.mutations.push(CurvesMutation::RelationRemoved {
        system_id,
        relation,
    });
    Ok(())
}

fn remove_inter(
    sheet: &mut NeutralCurvesSheet,
    system_id: usize,
    inter_id: usize,
) -> Result<(), CurvesContractError> {
    let index = system_index(sheet, system_id)?;
    let inter_index = sheet.systems[index]
        .inters
        .iter()
        .position(|inter| inter.id == inter_id)
        .ok_or(CurvesContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    let relation_ids = sheet.systems[index]
        .relations
        .iter()
        .filter(|relation| {
            relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
        })
        .map(|relation| relation.id)
        .collect::<Vec<_>>();
    for relation_id in relation_ids {
        remove_relation(sheet, system_id, relation_id)?;
    }
    sheet.systems[index].inters.remove(inter_index);
    let live_index = sheet
        .live_inter_ids
        .iter()
        .position(|id| *id == inter_id)
        .ok_or(CurvesContractError::InvalidLiveInterIndex)?;
    sheet.live_inter_ids.remove(live_index);
    sheet.retired_inter_ids.push(inter_id);
    sheet.mutations.push(CurvesMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn validate_sheet(sheet: &NeutralCurvesSheet) -> Result<(), CurvesContractError> {
    let mut glyph_ids = BTreeSet::new();
    for glyph in &sheet.registered_glyphs {
        if !glyph_ids.insert(glyph.id) {
            return Err(CurvesContractError::DuplicateGlyph(glyph.id));
        }
        if glyph.id >= sheet.next_glyph_id {
            return Err(CurvesContractError::InvalidGlyphAllocator {
                expected: sheet.next_glyph_id,
                actual: glyph.id,
            });
        }
    }
    let mut system_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    let mut relation_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(CurvesContractError::DuplicateSystem(system.id));
        }
        let mut staff_ids = BTreeSet::new();
        for &staff_id in &system.staff_ids {
            if !staff_ids.insert(staff_id) {
                return Err(CurvesContractError::DuplicateStaff(staff_id));
            }
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(CurvesContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(CurvesContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let Some(glyph_id) = inter.glyph_id
                && !glyph_ids.contains(&glyph_id)
            {
                return Err(CurvesContractError::UnknownGlyph(glyph_id));
            }
            if let Some(staff_id) = inter.staff_id
                && !staff_ids.contains(&staff_id)
            {
                return Err(CurvesContractError::UnknownStaff {
                    system_id: system.id,
                    staff_id,
                });
            }
        }
        for relation in &system.relations {
            if !relation_ids.insert(relation.id) {
                return Err(CurvesContractError::DuplicateRelation(relation.id));
            }
            if relation.id >= sheet.next_relation_id {
                return Err(CurvesContractError::InvalidRelationAllocator {
                    expected: sheet.next_relation_id,
                    actual: relation.id,
                });
            }
            for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                if !system.inters.iter().any(|inter| inter.id == endpoint) {
                    return Err(CurvesContractError::UnknownRelationEndpoint(endpoint));
                }
            }
        }
    }
    let live_inters = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live_inters.len() != sheet.live_inter_ids.len() || live_inters != inter_ids {
        return Err(CurvesContractError::InvalidLiveInterIndex);
    }
    let mut retired_inters = BTreeSet::new();
    for &id in &sheet.retired_inter_ids {
        if id >= sheet.next_inter_id || inter_ids.contains(&id) || !retired_inters.insert(id) {
            return Err(CurvesContractError::InvalidRetiredInter(id));
        }
    }
    let live_relations = sheet
        .live_relation_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live_relations.len() != sheet.live_relation_ids.len() || live_relations != relation_ids {
        return Err(CurvesContractError::InvalidLiveRelationIndex);
    }
    let mut retired_relations = BTreeSet::new();
    for &id in &sheet.retired_relation_ids {
        if id >= sheet.next_relation_id
            || relation_ids.contains(&id)
            || !retired_relations.insert(id)
        {
            return Err(CurvesContractError::InvalidRetiredRelation(id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeCurves {
        calls: Vec<(CurvesPhase, bool, bool, bool, Vec<usize>)>,
        outcomes: BTreeMap<CurvesPhase, CurvesPhaseOutcome<&'static str>>,
    }

    impl VisualCurves for FakeCurves {
        type Error = &'static str;

        fn run_phase(&mut self, input: CurvesPhaseInput<'_>) -> CurvesPhaseOutcome<Self::Error> {
            self.calls.push((
                input.phase,
                input.context.skeleton_ready,
                input.context.junctions_ready,
                input.context.arcs_ready,
                input
                    .context
                    .working_segments
                    .iter()
                    .map(|segment| segment.id)
                    .collect(),
            ));
            self.outcomes.remove(&input.phase).unwrap_or_else(|| {
                let mutation = match input.phase {
                    CurvesPhase::BuildSkeleton => Some(CurvesDeltaMutation::MarkSkeletonBuilt),
                    CurvesPhase::RetrieveJunctions => {
                        Some(CurvesDeltaMutation::MarkJunctionsRetrieved)
                    }
                    CurvesPhase::RetrieveArcs => Some(CurvesDeltaMutation::MarkArcsRetrieved),
                    _ => None,
                };
                CurvesPhaseOutcome::success(CurvesDelta {
                    mutations: mutation.into_iter().collect(),
                })
            })
        }
    }

    fn inter(id: usize, kind: NeutralCurveInterKind) -> NeutralCurveInter {
        NeutralCurveInter {
            id,
            kind,
            glyph_id: None,
            staff_id: Some(10),
        }
    }

    fn sheet() -> NeutralCurvesSheet {
        NeutralCurvesSheet {
            id: 7,
            systems: vec![
                NeutralCurvesSystem {
                    id: 9,
                    staff_ids: vec![10],
                    inters: vec![
                        inter(1, NeutralCurveInterKind::Head),
                        inter(2, NeutralCurveInterKind::Barline),
                    ],
                    relations: Vec::new(),
                },
                NeutralCurvesSystem {
                    id: 4,
                    staff_ids: vec![20],
                    inters: vec![NeutralCurveInter {
                        id: 3,
                        kind: NeutralCurveInterKind::Head,
                        glyph_id: None,
                        staff_id: Some(20),
                    }],
                    relations: Vec::new(),
                },
            ],
            registered_glyphs: Vec::new(),
            next_glyph_id: 1,
            live_inter_ids: vec![1, 2, 3],
            retired_inter_ids: Vec::new(),
            next_inter_id: 4,
            live_relation_ids: Vec::new(),
            retired_relation_ids: Vec::new(),
            next_relation_id: 1,
            mutations: Vec::new(),
        }
    }

    fn slur_delta() -> CurvesDelta {
        CurvesDelta {
            mutations: vec![
                CurvesDeltaMutation::RegisterGlyph(NeutralCurveGlyph {
                    id: 1,
                    kind: CurveGlyphKind::Slur,
                }),
                CurvesDeltaMutation::AddInter {
                    system_id: 9,
                    inter: NeutralCurveInter {
                        id: 4,
                        kind: NeutralCurveInterKind::Slur { tie: false },
                        glyph_id: Some(1),
                        staff_id: Some(10),
                    },
                },
                CurvesDeltaMutation::SetTie {
                    system_id: 9,
                    inter_id: 4,
                    tie: true,
                },
                CurvesDeltaMutation::AddRelation {
                    system_id: 9,
                    relation: NeutralCurveRelation {
                        id: 1,
                        source_inter_id: 4,
                        target_inter_id: 1,
                        kind: NeutralCurveRelationKind::SlurHead(CurveSide::Left),
                    },
                },
            ],
        }
    }

    #[test]
    fn exact_phase_order_mutates_tie_and_cleans_temporary_state() {
        let mut visual = FakeCurves::default();
        visual.outcomes.insert(
            CurvesPhase::BuildSlurs,
            CurvesPhaseOutcome::success(slur_delta()),
        );
        visual.outcomes.insert(
            CurvesPhase::BuildSegments,
            CurvesPhaseOutcome::success(CurvesDelta {
                mutations: vec![
                    CurvesDeltaMutation::RegisterGlyph(NeutralCurveGlyph {
                        id: 2,
                        kind: CurveGlyphKind::Segment,
                    }),
                    CurvesDeltaMutation::AddSegment(NeutralWorkingSegment { id: 1, glyph_id: 2 }),
                ],
            }),
        );
        visual.outcomes.insert(
            CurvesPhase::BuildWedges,
            CurvesPhaseOutcome::success(CurvesDelta {
                mutations: vec![
                    CurvesDeltaMutation::AddInter {
                        system_id: 9,
                        inter: NeutralCurveInter {
                            id: 5,
                            kind: NeutralCurveInterKind::Wedge,
                            glyph_id: None,
                            staff_id: Some(10),
                        },
                    },
                    CurvesDeltaMutation::RegisterGlyph(NeutralCurveGlyph {
                        id: 3,
                        kind: CurveGlyphKind::Wedge,
                    }),
                    CurvesDeltaMutation::SetInterGlyph {
                        system_id: 9,
                        inter_id: 5,
                        glyph_id: 3,
                    },
                    CurvesDeltaMutation::ConsumeSegment(1),
                ],
            }),
        );
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            step.visual()
                .calls
                .iter()
                .map(|call| call.0)
                .collect::<Vec<_>>(),
            CurvesPhase::ORDER
        );
        assert_eq!(
            (
                step.visual().calls[1].1,
                step.visual().calls[1].2,
                step.visual().calls[1].3,
            ),
            (true, false, false)
        );
        assert_eq!(
            (
                step.visual().calls[2].1,
                step.visual().calls[2].2,
                step.visual().calls[2].3,
            ),
            (true, true, false)
        );
        assert_eq!(step.visual().calls[5].4, [1]);
        assert!(report.cleanup.skeleton_discarded);
        assert!(report.cleanup.arcs_discarded);
        assert!(report.cleanup.discarded_segment_ids.is_empty());
        assert_eq!(
            sheet.systems[0].inters[2].kind,
            NeutralCurveInterKind::Slur { tie: true }
        );
        assert_eq!(sheet.systems[0].relations[0].target_inter_id, 1);
    }

    #[test]
    fn caught_slur_and_segment_failures_keep_prefixes_and_continue() {
        let mut visual = FakeCurves::default();
        visual.outcomes.insert(
            CurvesPhase::BuildSlurs,
            CurvesPhaseOutcome {
                delta: CurvesDelta {
                    mutations: slur_delta().mutations[..1].to_vec(),
                },
                failure: Some(CurvesFailure::Caught("slur throwable")),
            },
        );
        visual.outcomes.insert(
            CurvesPhase::BuildSegments,
            CurvesPhaseOutcome {
                delta: CurvesDelta::default(),
                failure: Some(CurvesFailure::Caught("segment throwable")),
            },
        );
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.caught_errors,
            [
                (CurvesPhase::BuildSlurs, "slur throwable"),
                (CurvesPhase::BuildSegments, "segment throwable"),
            ]
        );
        assert_eq!(step.visual().calls.len(), 8);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
    }

    #[test]
    fn fatal_phase_returns_exact_context_prefix_and_skips_later_phases() {
        let mut visual = FakeCurves::default();
        visual.outcomes.insert(
            CurvesPhase::RetrieveArcs,
            CurvesPhaseOutcome {
                delta: CurvesDelta::default(),
                failure: Some(CurvesFailure::Fatal("arc crash")),
            },
        );
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        let error = step.process(&mut sheet).unwrap_err();

        assert!(matches!(
            error,
            CurvesStepError::FatalPhase {
                phase: CurvesPhase::RetrieveArcs,
                source: "arc crash",
                context: CurvesContext {
                    skeleton_ready: true,
                    junctions_ready: true,
                    arcs_ready: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!(step.visual().calls.len(), 3);
    }

    #[test]
    fn caught_failure_is_rejected_outside_java_catching_builders() {
        let mut visual = FakeCurves::default();
        visual.outcomes.insert(
            CurvesPhase::BuildWedges,
            CurvesPhaseOutcome {
                delta: CurvesDelta::default(),
                failure: Some(CurvesFailure::Caught("not caught by Java")),
            },
        );
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(CurvesStepError::Contract(
                CurvesContractError::InvalidCaughtPhase(CurvesPhase::BuildWedges)
            ))
        );
    }

    #[test]
    fn inter_removal_cascades_relation_and_preserves_id_holes() {
        let mut visual = FakeCurves::default();
        let mut delta = slur_delta();
        delta.mutations.push(CurvesDeltaMutation::RemoveInter {
            system_id: 9,
            inter_id: 4,
        });
        visual
            .outcomes
            .insert(CurvesPhase::BuildSlurs, CurvesPhaseOutcome::success(delta));
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.next_inter_id, 5);
        assert_eq!(sheet.retired_inter_ids, [4]);
        assert_eq!(sheet.next_relation_id, 2);
        assert_eq!(sheet.retired_relation_ids, [1]);
        assert!(sheet.systems[0].relations.is_empty());
    }

    #[test]
    fn unused_segments_are_reported_as_discarded_at_cleanup() {
        let mut visual = FakeCurves::default();
        visual.outcomes.insert(
            CurvesPhase::BuildSegments,
            CurvesPhaseOutcome::success(CurvesDelta {
                mutations: vec![
                    CurvesDeltaMutation::RegisterGlyph(NeutralCurveGlyph {
                        id: 1,
                        kind: CurveGlyphKind::Segment,
                    }),
                    CurvesDeltaMutation::AddSegment(NeutralWorkingSegment { id: 1, glyph_id: 1 }),
                ],
            }),
        );
        let mut step = HeadlessCurvesStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.cleanup.discarded_segment_ids, [1]);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
    }
}
