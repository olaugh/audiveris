// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle and ownership port of Java `ChordsStep`.
//!
//! Head/stem canonical-share decisions, whole-head clustering, and measure
//! lookup remain injected at the first semantic seam. This module owns exact
//! system traversal, live SIG identities and relations, chord/note/stem,
//! staff, measure and optional voice ownership, cascading cleanup, and partial
//! failure prefixes.

use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeutralChordInterKind {
    Head {
        staff_id: usize,
        mirror_head_id: Option<usize>,
    },
    Stem,
    Beam,
    Chord {
        staff_id: usize,
        part_id: usize,
        note_ids: Vec<usize>,
        stem_id: Option<usize>,
    },
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordInter {
    pub id: usize,
    pub kind: NeutralChordInterKind,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralChordRelationKind {
    ChordMember,
    ChordStem,
    HeadStem,
    BeamStem,
    BeamHead,
    HeadHead,
    NoExclusion,
    OtherSupport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralChordRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralChordRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordStaff {
    pub id: usize,
    pub part_id: usize,
    pub note_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordMeasure {
    pub id: usize,
    pub chord_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordVoice {
    pub id: usize,
    pub chord_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordsSystem {
    pub id: usize,
    /// Java part order, each containing staff IDs in part staff order.
    pub part_staff_ids: Vec<(usize, Vec<usize>)>,
    pub staves: Vec<NeutralChordStaff>,
    pub inters: Vec<NeutralChordInter>,
    pub relations: Vec<NeutralChordRelation>,
    pub measures: Vec<NeutralChordMeasure>,
    /// Normally unassigned at CHORDS, retained for existing/manual ownership.
    pub voices: Vec<NeutralChordVoice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralChordsSheet {
    pub id: usize,
    pub systems: Vec<NeutralChordsSystem>,
    pub mutations: Vec<ChordsMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordsDelta {
    pub mutations: Vec<ChordsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordsDeltaMutation {
    AddInter(NeutralChordInter),
    RemoveInter(usize),
    AddRelation(NeutralChordRelation),
    RemoveRelation(usize),
    AttachNoteToStaff {
        staff_id: usize,
        note_id: usize,
    },
    DetachNoteFromStaff {
        staff_id: usize,
        note_id: usize,
    },
    SetHeadMirror {
        head_id: usize,
        mirror_head_id: usize,
    },
    AttachChordToMeasure {
        measure_id: usize,
        chord_id: usize,
    },
    AttachChordToVoice {
        voice_id: usize,
        chord_id: usize,
    },
    DetachChordFromVoice {
        voice_id: usize,
        chord_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordsStageOutcome<VisualError> {
    pub delta: ChordsDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> ChordsStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: ChordsDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChordsSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralChordsSystem,
}

/// First semantic dependency: canonical shared-head selection, head ordering,
/// stem chord reuse, whole-note vertical clustering, and measure lookup.
pub trait VisualChordsBuilder {
    type Error;

    fn build_head_chords(
        &mut self,
        input: ChordsSystemInput<'_>,
    ) -> ChordsStageOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordsReport<VisualError> {
    pub system_errors: Vec<(usize, VisualError)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordsStepError {
    Contract(ChordsContractError),
}

impl fmt::Display for ChordsStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(source) => write!(formatter, "chords contract failed: {source}"),
        }
    }
}

impl Error for ChordsStepError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordsContractError {
    DuplicateSystem(usize),
    DuplicateInter(usize),
    UnknownInter(usize),
    WrongInterKind(usize),
    DuplicateRelation(usize),
    UnknownRelation(usize),
    UnknownRelationEndpoint { relation_id: usize, inter_id: usize },
    DuplicateChordMember { chord_id: usize, note_id: usize },
    ChordStemAlreadySet { chord_id: usize, stem_id: usize },
    UnknownStaff(usize),
    DuplicateStaffNote { staff_id: usize, note_id: usize },
    MissingStaffNote { staff_id: usize, note_id: usize },
    UnknownMeasure(usize),
    DuplicateMeasureChord { measure_id: usize, chord_id: usize },
    UnknownVoice(usize),
    DuplicateVoiceChord { voice_id: usize, chord_id: usize },
    MissingVoiceChord { voice_id: usize, chord_id: usize },
}

impl fmt::Display for ChordsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ChordsContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordsMutation {
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterRemoved {
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
    StaffNoteAttached {
        staff_id: usize,
        note_id: usize,
    },
    StaffNoteDetached {
        staff_id: usize,
        note_id: usize,
    },
    HeadMirrorSet {
        head_id: usize,
        mirror_head_id: usize,
    },
    MeasureChordAttached {
        measure_id: usize,
        chord_id: usize,
    },
    MeasureChordDetached {
        measure_id: usize,
        chord_id: usize,
    },
    VoiceChordAttached {
        voice_id: usize,
        chord_id: usize,
    },
    VoiceChordDetached {
        voice_id: usize,
        chord_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
}

pub struct HeadlessChordsStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessChordsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualChordsBuilder> HeadlessChordsStep<Visual> {
    /// Java `AbstractSystemStep`: systems in sheet order, checked failures keep
    /// their prefix and later systems continue. CHORDS has no prolog/epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralChordsSheet,
    ) -> Result<ChordsReport<Visual::Error>, ChordsStepError> {
        validate_system_ids(sheet).map_err(ChordsStepError::Contract)?;
        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.build_head_chords(ChordsSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, outcome.delta).map_err(ChordsStepError::Contract)?;
            if let Some(error) = outcome.error {
                sheet
                    .mutations
                    .push(ChordsMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
            }
        }
        Ok(ChordsReport { system_errors })
    }
}

fn validate_system_ids(sheet: &NeutralChordsSheet) -> Result<(), ChordsContractError> {
    let mut ids = BTreeSet::new();
    for system in &sheet.systems {
        if !ids.insert(system.id) {
            return Err(ChordsContractError::DuplicateSystem(system.id));
        }
    }
    Ok(())
}

fn apply_delta(
    sheet: &mut NeutralChordsSheet,
    system_index: usize,
    delta: ChordsDelta,
) -> Result<(), ChordsContractError> {
    for mutation in delta.mutations {
        apply_mutation(sheet, system_index, mutation)?;
    }
    Ok(())
}

fn apply_mutation(
    sheet: &mut NeutralChordsSheet,
    system_index: usize,
    mutation: ChordsDeltaMutation,
) -> Result<(), ChordsContractError> {
    let system_id = sheet.systems[system_index].id;
    match mutation {
        ChordsDeltaMutation::AddInter(inter) => {
            if sheet.systems[system_index]
                .inters
                .iter()
                .any(|existing| existing.id == inter.id)
            {
                return Err(ChordsContractError::DuplicateInter(inter.id));
            }
            let inter_id = inter.id;
            sheet.systems[system_index].inters.push(inter);
            sheet.mutations.push(ChordsMutation::InterAdded {
                system_id,
                inter_id,
            });
        }
        ChordsDeltaMutation::RemoveInter(inter_id) => {
            remove_inter(sheet, system_index, inter_id)?;
        }
        ChordsDeltaMutation::AddRelation(relation) => {
            add_relation(sheet, system_index, relation)?;
        }
        ChordsDeltaMutation::RemoveRelation(relation_id) => {
            remove_relation(sheet, system_index, relation_id)?;
        }
        ChordsDeltaMutation::AttachNoteToStaff { staff_id, note_id } => {
            ensure_kind(&sheet.systems[system_index], note_id, |kind| {
                matches!(kind, NeutralChordInterKind::Head { .. })
            })?;
            let staff = sheet.systems[system_index]
                .staves
                .iter_mut()
                .find(|staff| staff.id == staff_id)
                .ok_or(ChordsContractError::UnknownStaff(staff_id))?;
            if staff.note_ids.contains(&note_id) {
                return Err(ChordsContractError::DuplicateStaffNote { staff_id, note_id });
            }
            staff.note_ids.push(note_id);
            sheet
                .mutations
                .push(ChordsMutation::StaffNoteAttached { staff_id, note_id });
        }
        ChordsDeltaMutation::DetachNoteFromStaff { staff_id, note_id } => {
            let staff = sheet.systems[system_index]
                .staves
                .iter_mut()
                .find(|staff| staff.id == staff_id)
                .ok_or(ChordsContractError::UnknownStaff(staff_id))?;
            let Some(index) = staff.note_ids.iter().position(|id| *id == note_id) else {
                return Err(ChordsContractError::MissingStaffNote { staff_id, note_id });
            };
            staff.note_ids.remove(index);
            sheet
                .mutations
                .push(ChordsMutation::StaffNoteDetached { staff_id, note_id });
        }
        ChordsDeltaMutation::SetHeadMirror {
            head_id,
            mirror_head_id,
        } => {
            ensure_kind(&sheet.systems[system_index], mirror_head_id, |kind| {
                matches!(kind, NeutralChordInterKind::Head { .. })
            })?;
            let head = sheet.systems[system_index]
                .inters
                .iter_mut()
                .find(|inter| inter.id == head_id && !inter.removed)
                .ok_or(ChordsContractError::UnknownInter(head_id))?;
            let NeutralChordInterKind::Head {
                mirror_head_id: mirror,
                ..
            } = &mut head.kind
            else {
                return Err(ChordsContractError::WrongInterKind(head_id));
            };
            *mirror = Some(mirror_head_id);
            sheet.mutations.push(ChordsMutation::HeadMirrorSet {
                head_id,
                mirror_head_id,
            });
        }
        ChordsDeltaMutation::AttachChordToMeasure {
            measure_id,
            chord_id,
        } => {
            ensure_kind(&sheet.systems[system_index], chord_id, |kind| {
                matches!(kind, NeutralChordInterKind::Chord { .. })
            })?;
            let mut prior = None;
            for measure in &mut sheet.systems[system_index].measures {
                if let Some(index) = measure.chord_ids.iter().position(|id| *id == chord_id) {
                    if measure.id == measure_id {
                        return Err(ChordsContractError::DuplicateMeasureChord {
                            measure_id,
                            chord_id,
                        });
                    }
                    measure.chord_ids.remove(index);
                    prior = Some(measure.id);
                }
            }
            if let Some(prior_id) = prior {
                sheet.mutations.push(ChordsMutation::MeasureChordDetached {
                    measure_id: prior_id,
                    chord_id,
                });
            }
            let measure = sheet.systems[system_index]
                .measures
                .iter_mut()
                .find(|measure| measure.id == measure_id)
                .ok_or(ChordsContractError::UnknownMeasure(measure_id))?;
            measure.chord_ids.push(chord_id);
            sheet.mutations.push(ChordsMutation::MeasureChordAttached {
                measure_id,
                chord_id,
            });
        }
        ChordsDeltaMutation::AttachChordToVoice { voice_id, chord_id } => {
            ensure_kind(&sheet.systems[system_index], chord_id, |kind| {
                matches!(kind, NeutralChordInterKind::Chord { .. })
            })?;
            let voice = sheet.systems[system_index]
                .voices
                .iter_mut()
                .find(|voice| voice.id == voice_id)
                .ok_or(ChordsContractError::UnknownVoice(voice_id))?;
            if voice.chord_ids.contains(&chord_id) {
                return Err(ChordsContractError::DuplicateVoiceChord { voice_id, chord_id });
            }
            voice.chord_ids.push(chord_id);
            sheet
                .mutations
                .push(ChordsMutation::VoiceChordAttached { voice_id, chord_id });
        }
        ChordsDeltaMutation::DetachChordFromVoice { voice_id, chord_id } => {
            let voice = sheet.systems[system_index]
                .voices
                .iter_mut()
                .find(|voice| voice.id == voice_id)
                .ok_or(ChordsContractError::UnknownVoice(voice_id))?;
            let Some(index) = voice.chord_ids.iter().position(|id| *id == chord_id) else {
                return Err(ChordsContractError::MissingVoiceChord { voice_id, chord_id });
            };
            voice.chord_ids.remove(index);
            sheet
                .mutations
                .push(ChordsMutation::VoiceChordDetached { voice_id, chord_id });
        }
    }
    Ok(())
}

fn add_relation(
    sheet: &mut NeutralChordsSheet,
    system_index: usize,
    relation: NeutralChordRelation,
) -> Result<(), ChordsContractError> {
    let system_id = sheet.systems[system_index].id;
    if sheet.systems[system_index]
        .relations
        .iter()
        .any(|existing| existing.id == relation.id)
    {
        return Err(ChordsContractError::DuplicateRelation(relation.id));
    }
    for inter_id in [relation.source_inter_id, relation.target_inter_id] {
        if !sheet.systems[system_index]
            .inters
            .iter()
            .any(|inter| inter.id == inter_id && !inter.removed)
        {
            return Err(ChordsContractError::UnknownRelationEndpoint {
                relation_id: relation.id,
                inter_id,
            });
        }
    }
    match relation.kind {
        NeutralChordRelationKind::ChordMember => {
            let chord = chord_mut(&mut sheet.systems[system_index], relation.source_inter_id)?;
            if chord.note_ids.contains(&relation.target_inter_id) {
                return Err(ChordsContractError::DuplicateChordMember {
                    chord_id: relation.source_inter_id,
                    note_id: relation.target_inter_id,
                });
            }
            chord.note_ids.push(relation.target_inter_id);
        }
        NeutralChordRelationKind::ChordStem => {
            let chord = chord_mut(&mut sheet.systems[system_index], relation.source_inter_id)?;
            if let Some(stem_id) = *chord.stem_id {
                return Err(ChordsContractError::ChordStemAlreadySet {
                    chord_id: relation.source_inter_id,
                    stem_id,
                });
            }
            *chord.stem_id = Some(relation.target_inter_id);
        }
        _ => {}
    }
    let relation_id = relation.id;
    sheet.systems[system_index].relations.push(relation);
    sheet.mutations.push(ChordsMutation::RelationAdded {
        system_id,
        relation_id,
    });
    Ok(())
}

fn remove_relation(
    sheet: &mut NeutralChordsSheet,
    system_index: usize,
    relation_id: usize,
) -> Result<(), ChordsContractError> {
    let system_id = sheet.systems[system_index].id;
    let Some(index) = sheet.systems[system_index]
        .relations
        .iter()
        .position(|relation| relation.id == relation_id)
    else {
        return Err(ChordsContractError::UnknownRelation(relation_id));
    };
    let relation = sheet.systems[system_index].relations.remove(index);
    if relation.kind == NeutralChordRelationKind::ChordMember {
        let chord = chord_mut(&mut sheet.systems[system_index], relation.source_inter_id)?;
        chord.note_ids.retain(|id| *id != relation.target_inter_id);
    } else if relation.kind == NeutralChordRelationKind::ChordStem {
        let chord = chord_mut(&mut sheet.systems[system_index], relation.source_inter_id)?;
        *chord.stem_id = None;
    }
    sheet.mutations.push(ChordsMutation::RelationRemoved {
        system_id,
        relation_id,
    });
    Ok(())
}

fn remove_inter(
    sheet: &mut NeutralChordsSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), ChordsContractError> {
    let system_id = sheet.systems[system_index].id;
    if !sheet.systems[system_index]
        .inters
        .iter()
        .any(|inter| inter.id == inter_id && !inter.removed)
    {
        return Err(ChordsContractError::UnknownInter(inter_id));
    }
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
    for staff in &mut sheet.systems[system_index].staves {
        if let Some(index) = staff.note_ids.iter().position(|id| *id == inter_id) {
            staff.note_ids.remove(index);
            sheet.mutations.push(ChordsMutation::StaffNoteDetached {
                staff_id: staff.id,
                note_id: inter_id,
            });
        }
    }
    for measure in &mut sheet.systems[system_index].measures {
        if let Some(index) = measure.chord_ids.iter().position(|id| *id == inter_id) {
            measure.chord_ids.remove(index);
            sheet.mutations.push(ChordsMutation::MeasureChordDetached {
                measure_id: measure.id,
                chord_id: inter_id,
            });
        }
    }
    for voice in &mut sheet.systems[system_index].voices {
        if let Some(index) = voice.chord_ids.iter().position(|id| *id == inter_id) {
            voice.chord_ids.remove(index);
            sheet.mutations.push(ChordsMutation::VoiceChordDetached {
                voice_id: voice.id,
                chord_id: inter_id,
            });
        }
    }
    let inter = sheet.systems[system_index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == inter_id)
        .expect("live inter was validated");
    inter.removed = true;
    sheet.mutations.push(ChordsMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn chord_mut(
    system: &mut NeutralChordsSystem,
    chord_id: usize,
) -> Result<ChordFields<'_>, ChordsContractError> {
    let inter = system
        .inters
        .iter_mut()
        .find(|inter| inter.id == chord_id && !inter.removed)
        .ok_or(ChordsContractError::UnknownInter(chord_id))?;
    let NeutralChordInterKind::Chord {
        note_ids, stem_id, ..
    } = &mut inter.kind
    else {
        return Err(ChordsContractError::WrongInterKind(chord_id));
    };
    Ok(ChordFields { note_ids, stem_id })
}

struct ChordFields<'a> {
    note_ids: &'a mut Vec<usize>,
    stem_id: &'a mut Option<usize>,
}

fn ensure_kind(
    system: &NeutralChordsSystem,
    inter_id: usize,
    predicate: impl FnOnce(&NeutralChordInterKind) -> bool,
) -> Result<(), ChordsContractError> {
    let inter = system
        .inters
        .iter()
        .find(|inter| inter.id == inter_id && !inter.removed)
        .ok_or(ChordsContractError::UnknownInter(inter_id))?;
    predicate(&inter.kind)
        .then_some(())
        .ok_or(ChordsContractError::WrongInterKind(inter_id))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct FakeBuilder {
        outcomes: BTreeMap<usize, ChordsStageOutcome<&'static str>>,
        calls: Vec<(usize, usize, usize)>,
    }

    impl VisualChordsBuilder for FakeBuilder {
        type Error = &'static str;

        fn build_head_chords(
            &mut self,
            input: ChordsSystemInput<'_>,
        ) -> ChordsStageOutcome<Self::Error> {
            self.calls.push((
                input.system.id,
                input.system.inters.len(),
                input.system.relations.len(),
            ));
            self.outcomes
                .remove(&input.system.id)
                .unwrap_or_else(|| ChordsStageOutcome::success(ChordsDelta::default()))
        }
    }

    fn system(id: usize, staff_id: usize, part_id: usize) -> NeutralChordsSystem {
        NeutralChordsSystem {
            id,
            part_staff_ids: vec![(part_id, vec![staff_id])],
            staves: vec![NeutralChordStaff {
                id: staff_id,
                part_id,
                note_ids: vec![10 + id],
            }],
            inters: vec![
                NeutralChordInter {
                    id: 10 + id,
                    kind: NeutralChordInterKind::Head {
                        staff_id,
                        mirror_head_id: None,
                    },
                    removed: false,
                },
                NeutralChordInter {
                    id: 20 + id,
                    kind: NeutralChordInterKind::Stem,
                    removed: false,
                },
            ],
            relations: vec![NeutralChordRelation {
                id: 30 + id,
                source_inter_id: 10 + id,
                target_inter_id: 20 + id,
                kind: NeutralChordRelationKind::HeadStem,
            }],
            measures: vec![
                NeutralChordMeasure {
                    id: 40 + id,
                    chord_ids: Vec::new(),
                },
                NeutralChordMeasure {
                    id: 50 + id,
                    chord_ids: Vec::new(),
                },
            ],
            voices: vec![NeutralChordVoice {
                id: 60 + id,
                chord_ids: Vec::new(),
            }],
        }
    }

    fn sheet() -> NeutralChordsSheet {
        NeutralChordsSheet {
            id: 1,
            systems: vec![system(2, 20, 200), system(1, 10, 100)],
            mutations: Vec::new(),
        }
    }

    fn chord_delta(system_id: usize, staff_id: usize, part_id: usize, base: usize) -> ChordsDelta {
        ChordsDelta {
            mutations: vec![
                ChordsDeltaMutation::AddInter(NeutralChordInter {
                    id: base,
                    kind: NeutralChordInterKind::Chord {
                        staff_id,
                        part_id,
                        note_ids: Vec::new(),
                        stem_id: None,
                    },
                    removed: false,
                }),
                ChordsDeltaMutation::AddRelation(NeutralChordRelation {
                    id: base + 1,
                    source_inter_id: base,
                    target_inter_id: 20 + system_id,
                    kind: NeutralChordRelationKind::ChordStem,
                }),
                ChordsDeltaMutation::AddRelation(NeutralChordRelation {
                    id: base + 2,
                    source_inter_id: base,
                    target_inter_id: 10 + system_id,
                    kind: NeutralChordRelationKind::ChordMember,
                }),
                ChordsDeltaMutation::AttachChordToMeasure {
                    measure_id: 40 + system_id,
                    chord_id: base,
                },
                ChordsDeltaMutation::AttachChordToVoice {
                    voice_id: 60 + system_id,
                    chord_id: base,
                },
            ],
        }
    }

    #[test]
    fn systems_run_in_sheet_order_and_materialize_all_ownership() {
        let mut visual = FakeBuilder::default();
        visual.outcomes.insert(
            2,
            ChordsStageOutcome::success(chord_delta(2, 20, 200, 1000)),
        );
        visual.outcomes.insert(
            1,
            ChordsStageOutcome::success(chord_delta(1, 10, 100, 2000)),
        );
        let mut step = HeadlessChordsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert!(report.system_errors.is_empty());
        assert_eq!(step.visual().calls, vec![(2, 2, 1), (1, 2, 1)]);
        let NeutralChordInterKind::Chord {
            note_ids, stem_id, ..
        } = &sheet.systems[0]
            .inters
            .iter()
            .find(|inter| inter.id == 1000)
            .unwrap()
            .kind
        else {
            panic!("expected chord")
        };
        assert_eq!(note_ids, &vec![12]);
        assert_eq!(*stem_id, Some(22));
        assert_eq!(sheet.systems[0].measures[0].chord_ids, vec![1000]);
        assert_eq!(sheet.systems[0].voices[0].chord_ids, vec![1000]);
    }

    #[test]
    fn shared_head_duplicate_preserves_inter_relation_staff_and_mirror_order() {
        let mut delta = ChordsDelta::default();
        delta.mutations.extend([
            ChordsDeltaMutation::AddInter(NeutralChordInter {
                id: 90,
                kind: NeutralChordInterKind::Head {
                    staff_id: 20,
                    mirror_head_id: None,
                },
                removed: false,
            }),
            ChordsDeltaMutation::SetHeadMirror {
                head_id: 90,
                mirror_head_id: 12,
            },
            ChordsDeltaMutation::AttachNoteToStaff {
                staff_id: 20,
                note_id: 90,
            },
            ChordsDeltaMutation::AddRelation(NeutralChordRelation {
                id: 91,
                source_inter_id: 90,
                target_inter_id: 22,
                kind: NeutralChordRelationKind::HeadStem,
            }),
        ]);
        let mut visual = FakeBuilder::default();
        visual
            .outcomes
            .insert(2, ChordsStageOutcome::success(delta));
        let mut step = HeadlessChordsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.systems[0].staves[0].note_ids, vec![12, 90]);
        assert_eq!(
            &sheet.mutations[..4],
            &[
                ChordsMutation::InterAdded {
                    system_id: 2,
                    inter_id: 90,
                },
                ChordsMutation::HeadMirrorSet {
                    head_id: 90,
                    mirror_head_id: 12,
                },
                ChordsMutation::StaffNoteAttached {
                    staff_id: 20,
                    note_id: 90,
                },
                ChordsMutation::RelationAdded {
                    system_id: 2,
                    relation_id: 91,
                },
            ]
        );
    }

    #[test]
    fn checked_failure_keeps_prefix_and_continues_without_epilog() {
        let mut visual = FakeBuilder::default();
        visual.outcomes.insert(
            2,
            ChordsStageOutcome {
                delta: chord_delta(2, 20, 200, 1000),
                error: Some("canonical share"),
            },
        );
        visual.outcomes.insert(
            1,
            ChordsStageOutcome::success(chord_delta(1, 10, 100, 2000)),
        );
        let mut step = HeadlessChordsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "canonical share")]);
        assert!(sheet.systems.iter().all(|system| system.inters.len() == 3));
        assert!(
            sheet
                .mutations
                .contains(&ChordsMutation::SystemFailed { system_id: 2 })
        );
    }

    #[test]
    fn removing_chord_cascades_relations_measure_voice_before_vertex() {
        let mut delta = chord_delta(2, 20, 200, 1000);
        delta.mutations.push(ChordsDeltaMutation::RemoveInter(1000));
        let mut visual = FakeBuilder::default();
        visual
            .outcomes
            .insert(2, ChordsStageOutcome::success(delta));
        let mut step = HeadlessChordsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert!(
            sheet.systems[0]
                .inters
                .iter()
                .find(|inter| inter.id == 1000)
                .unwrap()
                .removed
        );
        assert!(sheet.systems[0].measures[0].chord_ids.is_empty());
        assert!(sheet.systems[0].voices[0].chord_ids.is_empty());
        let tail = &sheet.mutations[sheet.mutations.len() - 5..];
        assert_eq!(
            tail,
            &[
                ChordsMutation::RelationRemoved {
                    system_id: 2,
                    relation_id: 1001,
                },
                ChordsMutation::RelationRemoved {
                    system_id: 2,
                    relation_id: 1002,
                },
                ChordsMutation::MeasureChordDetached {
                    measure_id: 42,
                    chord_id: 1000,
                },
                ChordsMutation::VoiceChordDetached {
                    voice_id: 62,
                    chord_id: 1000,
                },
                ChordsMutation::InterRemoved {
                    system_id: 2,
                    inter_id: 1000,
                },
            ]
        );
    }
}
