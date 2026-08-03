// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `HeadsStep`.
//!
//! Distance construction, transient head-spot extraction, and template-based
//! classification remain injected visual collaborators. This module owns the
//! observable Java lifecycle around them: one sheet prolog, systems in sheet
//! order, checked per-system continuation, then HEAD_SPOTS cleanup followed by
//! sheet-wide seed/head tally analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NeutralHeadShape(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HeadHorizontalSide {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralDistanceTable {
    pub id: usize,
    pub width: usize,
    pub height: usize,
}

/// One transient glyph produced from the HEAD_SPOTS run table.
///
/// `relevant_system_ids` is the visual `SystemManager.getSystemsOf` result.
/// The neutral layer still owns Java's inclusive horizontal-system test and
/// the possibility that one transient object is dispatched to two systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralHeadSpot {
    pub id: usize,
    pub center_x: i32,
    pub center_y: i32,
    pub relevant_system_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralHeadGlyph {
    pub id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NeutralHeadInterKind {
    Existing,
    Head {
        glyph_id: usize,
        staff_id: usize,
        shape: NeutralHeadShape,
        grade: f64,
        pitch: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralHeadInter {
    pub id: usize,
    pub kind: NeutralHeadInterKind,
}

impl NeutralHeadInter {
    #[must_use]
    pub const fn staff_id(&self) -> Option<usize> {
        match self.kind {
            NeutralHeadInterKind::Existing => None,
            NeutralHeadInterKind::Head { staff_id, .. } => Some(staff_id),
        }
    }

    #[must_use]
    pub const fn head_shape(&self) -> Option<NeutralHeadShape> {
        match self.kind {
            NeutralHeadInterKind::Existing => None,
            NeutralHeadInterKind::Head { shape, .. } => Some(shape),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralHeadRelationKind {
    Existing(u16),
    OverlapExclusion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralHeadRelation {
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralHeadRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralHeadStaff {
    pub id: usize,
    pub tablature: bool,
    /// Java `Staff.notes`, in insertion order.
    pub note_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralHeadSystem {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub staves: Vec<NeutralHeadStaff>,
    /// Live Java SIG vertices in insertion order.
    pub inters: Vec<NeutralHeadInter>,
    pub relations: Vec<NeutralHeadRelation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeadSeedScaleKey {
    pub shape: NeutralHeadShape,
    pub side: HeadHorizontalSide,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeutralHeadSeedScale {
    pub dx: BTreeMap<HeadSeedScaleKey, f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralHeadSheet {
    pub id: usize,
    pub systems: Vec<NeutralHeadSystem>,
    /// Java sheet GlyphIndex, which deliberately retains glyphs of removed heads.
    pub registered_glyphs: Vec<NeutralHeadGlyph>,
    /// Java sheet InterIndex live ownership, in registration order.
    pub live_inter_ids: Vec<usize>,
    /// Consumed IDs of removed heads/beams. IDs are never reused.
    pub retired_inter_ids: Vec<usize>,
    pub next_glyph_id: usize,
    pub next_inter_id: usize,
    /// `discardImage(HEAD_SPOTS)` marks persisted data discarded but does not
    /// require destruction of an already loaded in-memory run table.
    pub head_spots_discarded: bool,
    pub head_seed_scale: Option<NeutralHeadSeedScale>,
    pub mutations: Vec<HeadsMutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadsMutation {
    GlyphRegistered {
        system_id: usize,
        glyph_id: usize,
    },
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    StaffNoteAttached {
        staff_id: usize,
        head_id: usize,
    },
    StaffNoteDetached {
        staff_id: usize,
        head_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    RelationAdded {
        system_id: usize,
        relation: NeutralHeadRelation,
    },
    RelationRemoved {
        system_id: usize,
        relation: NeutralHeadRelation,
    },
    HeadGradeSet {
        system_id: usize,
        head_id: usize,
        grade: f64,
    },
    TallyRecorded {
        system_id: usize,
        head_id: usize,
        side: HeadHorizontalSide,
        dx: f64,
    },
    SystemFailed {
        system_id: usize,
    },
    HeadSpotsDiscarded,
    HeadSeedScaleSet(NeutralHeadSeedScale),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadTallySample {
    pub head_id: usize,
    pub side: HeadHorizontalSide,
    pub dx: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeadSystemTally {
    pub system_id: usize,
    /// Java uses one insertion-preserving map per side. Recording the same
    /// head/side replaces its value without moving it.
    pub samples: Vec<HeadTallySample>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeadDelta {
    /// Exact successful mutation prefix, retained before either checked or
    /// fatal classifier failure is observed.
    pub mutations: Vec<HeadDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadDeltaMutation {
    RegisterGlyph(NeutralHeadGlyph),
    AddHead(NeutralHeadInter),
    RemoveInter(usize),
    AddRelation(NeutralHeadRelation),
    RemoveRelation(NeutralHeadRelation),
    SetHeadGrade { head_id: usize, grade: f64 },
    RecordTally(HeadTallySample),
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadSystemFailure<VisualError> {
    /// Java `AbstractSystemStep` logs this checked failure and continues.
    Checked(VisualError),
    /// Sequential unchecked failures abort before later systems and epilog.
    Fatal(VisualError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadSystemOutcome<VisualError> {
    pub delta: HeadDelta,
    pub failure: Option<HeadSystemFailure<VisualError>>,
}

impl<VisualError> HeadSystemOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: HeadDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HeadDistanceInput<'a> {
    pub sheet: &'a NeutralHeadSheet,
}

#[derive(Clone, Copy, Debug)]
pub struct HeadSpotInput<'a> {
    pub sheet: &'a NeutralHeadSheet,
    pub distance_table: &'a NeutralDistanceTable,
}

#[derive(Clone, Copy, Debug)]
pub struct HeadSystemInput<'a> {
    pub sheet: &'a NeutralHeadSheet,
    pub system_id: usize,
    pub distance_table: &'a NeutralDistanceTable,
    pub spots: &'a [NeutralHeadSpot],
}

/// First visual seam in Java source order: distance construction, then spot
/// extraction, then template/classifier work per system.
pub trait VisualHeads {
    type Error;

    fn build_distance_table(
        &mut self,
        input: HeadDistanceInput<'_>,
    ) -> Result<NeutralDistanceTable, Self::Error>;

    fn retrieve_head_spots(
        &mut self,
        input: HeadSpotInput<'_>,
    ) -> Result<Vec<NeutralHeadSpot>, Self::Error>;

    fn classify_system(&mut self, input: HeadSystemInput<'_>) -> HeadSystemOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadsPrologStage {
    DistanceTable,
    HeadSpots,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeadsStepError<VisualError> {
    Prolog {
        stage: HeadsPrologStage,
        source: VisualError,
    },
    FatalSystem {
        system_id: usize,
        source: VisualError,
        checked_errors: Vec<(usize, VisualError)>,
    },
    Contract(HeadsContractError),
}

impl<VisualError: fmt::Display> fmt::Display for HeadsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prolog { stage, source } => {
                write!(formatter, "heads prolog {stage:?} failed: {source}")
            }
            Self::FatalSystem {
                system_id, source, ..
            } => write!(formatter, "heads system {system_id} failed: {source}"),
            Self::Contract(source) => write!(formatter, "heads contract failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for HeadsStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Prolog { source, .. } | Self::FatalSystem { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadsContractError {
    DuplicateSystem(usize),
    DuplicateStaff(usize),
    InvalidSystemBounds(usize),
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
    DuplicateStaffNote {
        staff_id: usize,
        head_id: usize,
    },
    InvalidStaffNote {
        staff_id: usize,
        head_id: usize,
    },
    DuplicateRelation(NeutralHeadRelation),
    UnknownRelation(NeutralHeadRelation),
    UnknownRelationEndpoint(usize),
    InvalidExclusionOrder {
        source_inter_id: usize,
        target_inter_id: usize,
    },
    DuplicateSpot(usize),
    DuplicateSpotSystem {
        spot_id: usize,
        system_id: usize,
    },
    ReorderedSpotSystems(usize),
}

impl fmt::Display for HeadsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for HeadsContractError {}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadsReport<VisualError> {
    pub system_errors: Vec<(usize, VisualError)>,
    pub distance_table: NeutralDistanceTable,
    /// TreeMap order used by Java's final `HeadSeedTally.analyze` traversal.
    pub tallies: Vec<HeadSystemTally>,
}

pub struct HeadlessHeadsStep<Visual> {
    visual: Visual,
    tally_quorum: usize,
}

impl<Visual> HeadlessHeadsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            tally_quorum: 10,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualHeads> HeadlessHeadsStep<Visual> {
    /// Java `AbstractSystemStep.doit`: prolog, systems, then epilog. Checked
    /// system failures are the only failures that still reach epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralHeadSheet,
    ) -> Result<HeadsReport<Visual::Error>, HeadsStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(HeadsStepError::Contract)?;
        let distance_table = self
            .visual
            .build_distance_table(HeadDistanceInput { sheet })
            .map_err(|source| HeadsStepError::Prolog {
                stage: HeadsPrologStage::DistanceTable,
                source,
            })?;
        let spots = self
            .visual
            .retrieve_head_spots(HeadSpotInput {
                sheet,
                distance_table: &distance_table,
            })
            .map_err(|source| HeadsStepError::Prolog {
                stage: HeadsPrologStage::HeadSpots,
                source,
            })?;
        let dispatched = dispatch_head_spots(sheet, spots).map_err(HeadsStepError::Contract)?;
        let mut tallies = sheet
            .systems
            .iter()
            .map(|system| {
                (
                    system.id,
                    HeadSystemTally {
                        system_id: system.id,
                        samples: Vec::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut system_errors = Vec::new();

        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.classify_system(HeadSystemInput {
                sheet,
                system_id,
                distance_table: &distance_table,
                spots: &dispatched[&system_id],
            });
            apply_delta(
                sheet,
                system_id,
                tallies
                    .get_mut(&system_id)
                    .expect("every validated system has a tally"),
                outcome.delta,
            )
            .map_err(HeadsStepError::Contract)?;
            match outcome.failure {
                None => {}
                Some(HeadSystemFailure::Checked(source)) => {
                    sheet
                        .mutations
                        .push(HeadsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                }
                Some(HeadSystemFailure::Fatal(source)) => {
                    return Err(HeadsStepError::FatalSystem {
                        system_id,
                        source,
                        checked_errors: system_errors,
                    });
                }
            }
        }

        // Java epilog order is observable and is intentionally not a finally.
        sheet.head_spots_discarded = true;
        sheet.mutations.push(HeadsMutation::HeadSpotsDiscarded);
        let scale = analyze_tallies(sheet, &tallies, self.tally_quorum);
        sheet.head_seed_scale = Some(scale.clone());
        sheet.mutations.push(HeadsMutation::HeadSeedScaleSet(scale));

        Ok(HeadsReport {
            system_errors,
            distance_table,
            tallies: tallies.into_values().collect(),
        })
    }
}

fn dispatch_head_spots(
    sheet: &NeutralHeadSheet,
    spots: Vec<NeutralHeadSpot>,
) -> Result<BTreeMap<usize, Vec<NeutralHeadSpot>>, HeadsContractError> {
    let system_positions = sheet
        .systems
        .iter()
        .enumerate()
        .map(|(index, system)| (system.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut result = sheet
        .systems
        .iter()
        .map(|system| (system.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut spot_ids = BTreeSet::new();
    for spot in spots {
        if !spot_ids.insert(spot.id) {
            return Err(HeadsContractError::DuplicateSpot(spot.id));
        }
        let mut relevant = BTreeSet::new();
        let mut previous_position = None;
        for &system_id in &spot.relevant_system_ids {
            let Some(&position) = system_positions.get(&system_id) else {
                return Err(HeadsContractError::UnknownSystem(system_id));
            };
            if !relevant.insert(system_id) {
                return Err(HeadsContractError::DuplicateSpotSystem {
                    spot_id: spot.id,
                    system_id,
                });
            }
            if previous_position.is_some_and(|previous| position <= previous) {
                return Err(HeadsContractError::ReorderedSpotSystems(spot.id));
            }
            previous_position = Some(position);
            let system = &sheet.systems[position];
            if spot.center_x >= system.left && spot.center_x <= system.right {
                result
                    .get_mut(&system_id)
                    .expect("validated system bucket")
                    .push(spot.clone());
            }
        }
    }
    Ok(result)
}

fn apply_delta(
    sheet: &mut NeutralHeadSheet,
    system_id: usize,
    tally: &mut HeadSystemTally,
    delta: HeadDelta,
) -> Result<(), HeadsContractError> {
    let system_index = sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(HeadsContractError::UnknownSystem(system_id))?;
    for mutation in delta.mutations {
        match mutation {
            HeadDeltaMutation::RegisterGlyph(glyph) => {
                if glyph.id != sheet.next_glyph_id {
                    return Err(HeadsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    });
                }
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|candidate| candidate.id == glyph.id)
                {
                    return Err(HeadsContractError::DuplicateGlyph(glyph.id));
                }
                sheet.next_glyph_id = sheet.next_glyph_id.checked_add(1).ok_or(
                    HeadsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    },
                )?;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(HeadsMutation::GlyphRegistered {
                    system_id,
                    glyph_id: glyph.id,
                });
            }
            HeadDeltaMutation::AddHead(head) => {
                if head.id != sheet.next_inter_id {
                    return Err(HeadsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: head.id,
                    });
                }
                let NeutralHeadInterKind::Head {
                    glyph_id, staff_id, ..
                } = head.kind
                else {
                    return Err(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id: head.id,
                    });
                };
                if !sheet
                    .registered_glyphs
                    .iter()
                    .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(HeadsContractError::UnknownGlyph(glyph_id));
                }
                let staff_index = sheet.systems[system_index]
                    .staves
                    .iter()
                    .position(|staff| staff.id == staff_id)
                    .ok_or(HeadsContractError::UnknownStaff {
                        system_id,
                        staff_id,
                    })?;
                if sheet
                    .systems
                    .iter()
                    .flat_map(|system| &system.inters)
                    .any(|inter| inter.id == head.id)
                    || sheet.retired_inter_ids.contains(&head.id)
                {
                    return Err(HeadsContractError::DuplicateInter(head.id));
                }
                sheet.next_inter_id = sheet.next_inter_id.checked_add(1).ok_or(
                    HeadsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: head.id,
                    },
                )?;
                sheet.systems[system_index].inters.push(head.clone());
                sheet.live_inter_ids.push(head.id);
                sheet.mutations.push(HeadsMutation::InterAdded {
                    system_id,
                    inter_id: head.id,
                });
                sheet.systems[system_index].staves[staff_index]
                    .note_ids
                    .push(head.id);
                sheet.mutations.push(HeadsMutation::StaffNoteAttached {
                    staff_id,
                    head_id: head.id,
                });
            }
            HeadDeltaMutation::RemoveInter(inter_id) => {
                let inter_index = sheet.systems[system_index]
                    .inters
                    .iter()
                    .position(|inter| inter.id == inter_id)
                    .ok_or(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id,
                    })?;
                if let Some(staff_id) = sheet.systems[system_index].inters[inter_index].staff_id() {
                    let staff = sheet.systems[system_index]
                        .staves
                        .iter_mut()
                        .find(|staff| staff.id == staff_id)
                        .ok_or(HeadsContractError::UnknownStaff {
                            system_id,
                            staff_id,
                        })?;
                    let note_index = staff.note_ids.iter().position(|id| *id == inter_id).ok_or(
                        HeadsContractError::InvalidStaffNote {
                            staff_id,
                            head_id: inter_id,
                        },
                    )?;
                    staff.note_ids.remove(note_index);
                    sheet.mutations.push(HeadsMutation::StaffNoteDetached {
                        staff_id,
                        head_id: inter_id,
                    });
                }
                sheet.systems[system_index].relations.retain(|relation| {
                    relation.source_inter_id != inter_id && relation.target_inter_id != inter_id
                });
                sheet.systems[system_index].inters.remove(inter_index);
                let live_index = sheet
                    .live_inter_ids
                    .iter()
                    .position(|id| *id == inter_id)
                    .ok_or(HeadsContractError::InvalidLiveInterIndex)?;
                sheet.live_inter_ids.remove(live_index);
                sheet.retired_inter_ids.push(inter_id);
                sheet.mutations.push(HeadsMutation::InterRemoved {
                    system_id,
                    inter_id,
                });
            }
            HeadDeltaMutation::AddRelation(relation) => {
                validate_relation(&sheet.systems[system_index], relation)?;
                if sheet.systems[system_index].relations.contains(&relation) {
                    return Err(HeadsContractError::DuplicateRelation(relation));
                }
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(HeadsMutation::RelationAdded {
                    system_id,
                    relation,
                });
            }
            HeadDeltaMutation::RemoveRelation(relation) => {
                let relation_index = sheet.systems[system_index]
                    .relations
                    .iter()
                    .position(|candidate| *candidate == relation)
                    .ok_or(HeadsContractError::UnknownRelation(relation))?;
                sheet.systems[system_index].relations.remove(relation_index);
                sheet.mutations.push(HeadsMutation::RelationRemoved {
                    system_id,
                    relation,
                });
            }
            HeadDeltaMutation::SetHeadGrade { head_id, grade } => {
                let inter = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == head_id)
                    .ok_or(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id: head_id,
                    })?;
                let NeutralHeadInterKind::Head {
                    grade: head_grade, ..
                } = &mut inter.kind
                else {
                    return Err(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id: head_id,
                    });
                };
                *head_grade = grade;
                sheet.mutations.push(HeadsMutation::HeadGradeSet {
                    system_id,
                    head_id,
                    grade,
                });
            }
            HeadDeltaMutation::RecordTally(sample) => {
                let Some(inter) = sheet.systems[system_index]
                    .inters
                    .iter()
                    .find(|inter| inter.id == sample.head_id)
                else {
                    return Err(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id: sample.head_id,
                    });
                };
                if inter.head_shape().is_none() {
                    return Err(HeadsContractError::UnknownInter {
                        system_id,
                        inter_id: sample.head_id,
                    });
                }
                if let Some(existing) = tally.samples.iter_mut().find(|existing| {
                    existing.head_id == sample.head_id && existing.side == sample.side
                }) {
                    existing.dx = sample.dx;
                } else {
                    tally.samples.push(sample);
                }
                sheet.mutations.push(HeadsMutation::TallyRecorded {
                    system_id,
                    head_id: sample.head_id,
                    side: sample.side,
                    dx: sample.dx,
                });
            }
        }
    }
    Ok(())
}

fn analyze_tallies(
    sheet: &NeutralHeadSheet,
    tallies: &BTreeMap<usize, HeadSystemTally>,
    quorum: usize,
) -> NeutralHeadSeedScale {
    let mut populations = BTreeMap::<HeadSeedScaleKey, Vec<f64>>::new();
    for tally in tallies.values() {
        let Some(system) = sheet
            .systems
            .iter()
            .find(|system| system.id == tally.system_id)
        else {
            continue;
        };
        for side in [HeadHorizontalSide::Left, HeadHorizontalSide::Right] {
            for sample in tally.samples.iter().filter(|sample| sample.side == side) {
                let Some(shape) = system
                    .inters
                    .iter()
                    .find(|inter| inter.id == sample.head_id)
                    .and_then(NeutralHeadInter::head_shape)
                else {
                    // Java keeps object references in a tally but ignores removed heads.
                    continue;
                };
                populations
                    .entry(HeadSeedScaleKey { shape, side })
                    .or_default()
                    .push(sample.dx);
            }
        }
    }
    let dx = populations
        .into_iter()
        .filter_map(|(key, values)| {
            (values.len() >= quorum).then(|| {
                (
                    key,
                    values.iter().copied().sum::<f64>() / values.len() as f64,
                )
            })
        })
        .collect();
    NeutralHeadSeedScale { dx }
}

fn validate_relation(
    system: &NeutralHeadSystem,
    relation: NeutralHeadRelation,
) -> Result<(), HeadsContractError> {
    for endpoint in [relation.source_inter_id, relation.target_inter_id] {
        if !system.inters.iter().any(|inter| inter.id == endpoint) {
            return Err(HeadsContractError::UnknownRelationEndpoint(endpoint));
        }
    }
    if relation.kind == NeutralHeadRelationKind::OverlapExclusion
        && relation.source_inter_id >= relation.target_inter_id
    {
        return Err(HeadsContractError::InvalidExclusionOrder {
            source_inter_id: relation.source_inter_id,
            target_inter_id: relation.target_inter_id,
        });
    }
    Ok(())
}

fn validate_sheet(sheet: &NeutralHeadSheet) -> Result<(), HeadsContractError> {
    let mut system_ids = BTreeSet::new();
    let mut staff_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    let glyph_ids = sheet
        .registered_glyphs
        .iter()
        .map(|glyph| glyph.id)
        .collect::<BTreeSet<_>>();
    if glyph_ids.len() != sheet.registered_glyphs.len() {
        let duplicate = sheet
            .registered_glyphs
            .iter()
            .find(|glyph| {
                sheet
                    .registered_glyphs
                    .iter()
                    .filter(|candidate| candidate.id == glyph.id)
                    .count()
                    > 1
            })
            .map_or(0, |glyph| glyph.id);
        return Err(HeadsContractError::DuplicateGlyph(duplicate));
    }
    if glyph_ids.iter().any(|id| *id >= sheet.next_glyph_id) {
        return Err(HeadsContractError::InvalidGlyphAllocator {
            expected: sheet.next_glyph_id,
            actual: *glyph_ids
                .iter()
                .find(|id| **id >= sheet.next_glyph_id)
                .expect("predicate found one"),
        });
    }
    let mut expected_notes = BTreeMap::<usize, usize>::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(HeadsContractError::DuplicateSystem(system.id));
        }
        if system.left > system.right {
            return Err(HeadsContractError::InvalidSystemBounds(system.id));
        }
        for staff in &system.staves {
            if !staff_ids.insert(staff.id) {
                return Err(HeadsContractError::DuplicateStaff(staff.id));
            }
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(HeadsContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(HeadsContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let NeutralHeadInterKind::Head {
                glyph_id, staff_id, ..
            } = inter.kind
            {
                if !glyph_ids.contains(&glyph_id) {
                    return Err(HeadsContractError::UnknownGlyph(glyph_id));
                }
                if !system.staves.iter().any(|staff| staff.id == staff_id) {
                    return Err(HeadsContractError::UnknownStaff {
                        system_id: system.id,
                        staff_id,
                    });
                }
                expected_notes.insert(inter.id, staff_id);
            }
        }
        for relation in &system.relations {
            validate_relation(system, *relation)?;
        }
    }
    let live_ids = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live_ids.len() != sheet.live_inter_ids.len() || live_ids != inter_ids {
        return Err(HeadsContractError::InvalidLiveInterIndex);
    }
    let mut retired = BTreeSet::new();
    for &id in &sheet.retired_inter_ids {
        if id >= sheet.next_inter_id || inter_ids.contains(&id) || !retired.insert(id) {
            return Err(HeadsContractError::InvalidRetiredInter(id));
        }
    }
    let mut actual_notes = BTreeMap::new();
    for system in &sheet.systems {
        for staff in &system.staves {
            for &head_id in &staff.note_ids {
                if actual_notes.insert(head_id, staff.id).is_some() {
                    return Err(HeadsContractError::DuplicateStaffNote {
                        staff_id: staff.id,
                        head_id,
                    });
                }
            }
        }
    }
    for (&head_id, &staff_id) in &expected_notes {
        if actual_notes.get(&head_id) != Some(&staff_id) {
            return Err(HeadsContractError::InvalidStaffNote { staff_id, head_id });
        }
    }
    if actual_notes.len() != expected_notes.len() {
        let (&head_id, &staff_id) = actual_notes
            .iter()
            .find(|(head_id, _)| !expected_notes.contains_key(head_id))
            .expect("different cardinality supplies an extra note");
        return Err(HeadsContractError::InvalidStaffNote { staff_id, head_id });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeVisual {
        calls: Vec<String>,
        distance: Option<Result<NeutralDistanceTable, &'static str>>,
        spots: Option<Result<Vec<NeutralHeadSpot>, &'static str>>,
        outcomes: BTreeMap<usize, HeadSystemOutcome<&'static str>>,
        spot_inputs: Vec<(usize, Vec<usize>)>,
    }

    impl VisualHeads for FakeVisual {
        type Error = &'static str;

        fn build_distance_table(
            &mut self,
            _input: HeadDistanceInput<'_>,
        ) -> Result<NeutralDistanceTable, Self::Error> {
            self.calls.push("distance".into());
            self.distance.take().unwrap_or(Ok(NeutralDistanceTable {
                id: 7,
                width: 100,
                height: 80,
            }))
        }

        fn retrieve_head_spots(
            &mut self,
            _input: HeadSpotInput<'_>,
        ) -> Result<Vec<NeutralHeadSpot>, Self::Error> {
            self.calls.push("spots".into());
            self.spots.take().unwrap_or(Ok(Vec::new()))
        }

        fn classify_system(
            &mut self,
            input: HeadSystemInput<'_>,
        ) -> HeadSystemOutcome<Self::Error> {
            self.calls.push(format!("system{}", input.system_id));
            self.spot_inputs.push((
                input.system_id,
                input.spots.iter().map(|spot| spot.id).collect(),
            ));
            self.outcomes
                .remove(&input.system_id)
                .unwrap_or_else(|| HeadSystemOutcome::success(HeadDelta::default()))
        }
    }

    fn staff(id: usize) -> NeutralHeadStaff {
        NeutralHeadStaff {
            id,
            tablature: false,
            note_ids: Vec::new(),
        }
    }

    fn system(id: usize, left: i32, right: i32, staff_id: usize) -> NeutralHeadSystem {
        NeutralHeadSystem {
            id,
            left,
            right,
            staves: vec![staff(staff_id)],
            inters: Vec::new(),
            relations: Vec::new(),
        }
    }

    fn sheet() -> NeutralHeadSheet {
        NeutralHeadSheet {
            id: 3,
            systems: vec![system(2, 0, 50, 20), system(1, 50, 100, 10)],
            registered_glyphs: Vec::new(),
            live_inter_ids: Vec::new(),
            retired_inter_ids: Vec::new(),
            next_glyph_id: 1,
            next_inter_id: 1,
            head_spots_discarded: false,
            head_seed_scale: None,
            mutations: Vec::new(),
        }
    }

    fn head(id: usize, glyph_id: usize, staff_id: usize) -> NeutralHeadInter {
        NeutralHeadInter {
            id,
            kind: NeutralHeadInterKind::Head {
                glyph_id,
                staff_id,
                shape: NeutralHeadShape(4),
                grade: 0.8,
                pitch: 0.0,
            },
        }
    }

    fn add_head_delta(glyph_id: usize, head_id: usize, staff_id: usize) -> HeadDelta {
        HeadDelta {
            mutations: vec![
                HeadDeltaMutation::RegisterGlyph(NeutralHeadGlyph { id: glyph_id }),
                HeadDeltaMutation::AddHead(head(head_id, glyph_id, staff_id)),
            ],
        }
    }

    #[test]
    fn runs_exact_lifecycle_and_dispatches_shared_boundary_spot() {
        let mut visual = FakeVisual {
            spots: Some(Ok(vec![NeutralHeadSpot {
                id: 9,
                center_x: 50,
                center_y: 20,
                relevant_system_ids: vec![2, 1],
            }])),
            ..FakeVisual::default()
        };
        visual
            .outcomes
            .insert(2, HeadSystemOutcome::success(add_head_delta(1, 1, 20)));
        visual
            .outcomes
            .insert(1, HeadSystemOutcome::success(add_head_delta(2, 2, 10)));
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert!(report.system_errors.is_empty());
        assert_eq!(
            step.visual().calls,
            ["distance", "spots", "system2", "system1"]
        );
        assert_eq!(step.visual().spot_inputs, [(2, vec![9]), (1, vec![9])]);
        assert_eq!(sheet.live_inter_ids, [1, 2]);
        assert_eq!(sheet.systems[0].staves[0].note_ids, [1]);
        assert_eq!(sheet.systems[1].staves[0].note_ids, [2]);
        assert!(sheet.head_spots_discarded);
        assert_eq!(sheet.head_seed_scale, Some(NeutralHeadSeedScale::default()));
        assert!(matches!(
            sheet.mutations.as_slice(),
            [
                ..,
                HeadsMutation::HeadSpotsDiscarded,
                HeadsMutation::HeadSeedScaleSet(_)
            ]
        ));
    }

    #[test]
    fn checked_failure_retains_prefix_continues_and_epilogs() {
        let mut visual = FakeVisual::default();
        visual.outcomes.insert(
            2,
            HeadSystemOutcome {
                delta: add_head_delta(1, 1, 20),
                failure: Some(HeadSystemFailure::Checked("template failed")),
            },
        );
        visual
            .outcomes
            .insert(1, HeadSystemOutcome::success(add_head_delta(2, 2, 10)));
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, [(2, "template failed")]);
        assert_eq!(
            step.visual().calls,
            ["distance", "spots", "system2", "system1"]
        );
        assert_eq!(sheet.live_inter_ids, [1, 2]);
        assert!(sheet.head_spots_discarded);
        assert!(sheet
            .mutations
            .contains(&HeadsMutation::SystemFailed { system_id: 2 }));
    }

    #[test]
    fn fatal_system_keeps_delta_but_skips_later_system_and_epilog() {
        let mut visual = FakeVisual::default();
        visual.outcomes.insert(
            2,
            HeadSystemOutcome {
                delta: add_head_delta(1, 1, 20),
                failure: Some(HeadSystemFailure::Fatal("unchecked classifier")),
            },
        );
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(HeadsStepError::FatalSystem {
                system_id: 2,
                source: "unchecked classifier",
                checked_errors: Vec::new(),
            })
        );
        assert_eq!(step.visual().calls, ["distance", "spots", "system2"]);
        assert_eq!(sheet.live_inter_ids, [1]);
        assert!(!sheet.head_spots_discarded);
        assert!(sheet.head_seed_scale.is_none());
    }

    #[test]
    fn prolog_failure_has_zero_mutation_prefix() {
        let visual = FakeVisual {
            distance: Some(Err("no binary")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(HeadsStepError::Prolog {
                stage: HeadsPrologStage::DistanceTable,
                source: "no binary",
            })
        );
        assert_eq!(step.visual().calls, ["distance"]);
        assert!(sheet.mutations.is_empty());
        assert!(!sheet.head_spots_discarded);
    }

    #[test]
    fn removed_head_leaves_ids_and_glyph_but_not_sig_or_staff_ownership() {
        let mut visual = FakeVisual::default();
        visual.outcomes.insert(
            2,
            HeadSystemOutcome::success(HeadDelta {
                mutations: vec![
                    HeadDeltaMutation::RegisterGlyph(NeutralHeadGlyph { id: 1 }),
                    HeadDeltaMutation::AddHead(head(1, 1, 20)),
                    HeadDeltaMutation::RecordTally(HeadTallySample {
                        head_id: 1,
                        side: HeadHorizontalSide::Left,
                        dx: -1.0,
                    }),
                    HeadDeltaMutation::RemoveInter(1),
                    HeadDeltaMutation::RegisterGlyph(NeutralHeadGlyph { id: 2 }),
                    HeadDeltaMutation::AddHead(head(2, 2, 20)),
                ],
            }),
        );
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.registered_glyphs.len(), 2);
        assert_eq!(sheet.next_inter_id, 3);
        assert_eq!(sheet.retired_inter_ids, [1]);
        assert_eq!(sheet.live_inter_ids, [2]);
        assert_eq!(sheet.systems[0].staves[0].note_ids, [2]);
        assert_eq!(sheet.head_seed_scale, Some(NeutralHeadSeedScale::default()));
    }

    #[test]
    fn tally_requires_ten_live_heads_and_uses_java_system_id_order() {
        let mut visual = FakeVisual::default();
        let mut first = Vec::new();
        for id in 1..=10 {
            first.push(HeadDeltaMutation::RegisterGlyph(NeutralHeadGlyph { id }));
            first.push(HeadDeltaMutation::AddHead(head(id, id, 20)));
            first.push(HeadDeltaMutation::RecordTally(HeadTallySample {
                head_id: id,
                side: HeadHorizontalSide::Right,
                dx: id as f64,
            }));
        }
        visual.outcomes.insert(
            2,
            HeadSystemOutcome::success(HeadDelta { mutations: first }),
        );
        let mut step = HeadlessHeadsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(
            sheet
                .head_seed_scale
                .as_ref()
                .unwrap()
                .dx
                .get(&HeadSeedScaleKey {
                    shape: NeutralHeadShape(4),
                    side: HeadHorizontalSide::Right,
                }),
            Some(&5.5)
        );
    }
}
