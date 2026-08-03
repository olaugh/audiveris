// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light orchestration port of Java `KeyColumn`.
//!
//! Projection, peak/slice extraction, glyph construction, shape recognition,
//! pitch compatibility, and missing-alter replication remain injected visual
//! operations. System ordering, global slice aggregation, part consistency,
//! SIG mutation, winner finalization, and offset reporting are native Rust.

use std::collections::BTreeMap;
use std::error::Error;
use std::f64::consts::TAU;
use std::fmt;

use crate::{
    clef_column::NeutralClefKind,
    headers_step::HeadlessHeaderSystem,
    staff_header::{HeaderBounds, HeaderComponent, StaffHeaderRange},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralKeyAlterShape {
    Flat,
    Sharp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyAlterClassifierProposal {
    pub id: usize,
    pub start: i32,
    pub width: i32,
    pub bounds: HeaderBounds,
    pub classifier_grade: f64,
    pub measured_pitch: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyShapeClassifierProposal {
    pub id: usize,
    pub shape: NeutralKeyAlterShape,
    pub range: StaffHeaderRange,
    pub alters: Vec<KeyAlterClassifierProposal>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyClefSupport {
    pub id: usize,
    pub kind: NeutralClefKind,
    pub grade: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyLifecycleContext {
    pub clefs: Vec<KeyClefSupport>,
    pub maximum_delta_pitch_one: f64,
    pub maximum_delta_pitch_four: f64,
    pub clef_key_source_ratio: f64,
    pub key_alters_source_ratio: f64,
}

pub trait VisualKeyProposalRecognizer {
    type Error;

    fn classify_key_shapes(
        &mut self,
        input: KeyRecognitionInput,
    ) -> Result<Vec<KeyShapeClassifierProposal>, Self::Error>;

    fn replicate_key(
        &mut self,
        system_id: usize,
        target_staff_id: usize,
        source: &NeutralKeyCandidate,
        global_offsets: &[i32],
    ) -> Result<KeyReplication, Self::Error>;
}

/// Native per-staff key lifecycle around injected accidental proposals.
pub struct KeyLifecycleRecognizer<Visual> {
    visual: Visual,
    contexts: BTreeMap<usize, KeyLifecycleContext>,
    selected_clefs: BTreeMap<usize, usize>,
}

impl<Visual> KeyLifecycleRecognizer<Visual> {
    #[must_use]
    pub fn new(visual: Visual, contexts: BTreeMap<usize, KeyLifecycleContext>) -> Self {
        Self {
            visual,
            contexts,
            selected_clefs: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }

    #[must_use]
    pub fn selected_clefs(&self) -> &BTreeMap<usize, usize> {
        &self.selected_clefs
    }
}

impl<Visual> VisualKeyRecognizer for KeyLifecycleRecognizer<Visual>
where
    Visual: VisualKeyProposalRecognizer,
{
    type Error = Visual::Error;

    fn recognize_keys(
        &mut self,
        input: KeyRecognitionInput,
    ) -> Result<Vec<NeutralKeyCandidate>, Self::Error> {
        let proposals = self.visual.classify_key_shapes(input)?;
        let context = &self.contexts[&input.staff_id];
        let mut candidates = Vec::new();
        for proposal in proposals {
            if let Some((candidate, clef_id)) = lifecycle_key_candidate(input, proposal, context) {
                self.selected_clefs.insert(input.staff_id, clef_id);
                candidates.push(candidate);
            }
        }
        Ok(candidates)
    }

    fn replicate_key(
        &mut self,
        system_id: usize,
        target_staff_id: usize,
        source: &NeutralKeyCandidate,
        global_offsets: &[i32],
    ) -> Result<KeyReplication, Self::Error> {
        self.visual
            .replicate_key(system_id, target_staff_id, source, global_offsets)
    }
}

fn lifecycle_key_candidate(
    input: KeyRecognitionInput,
    mut proposal: KeyShapeClassifierProposal,
    context: &KeyLifecycleContext,
) -> Option<(NeutralKeyCandidate, usize)> {
    proposal.alters.sort_by_key(|alter| alter.start);
    if proposal.alters.is_empty() || proposal.alters.len() > 7 || context.clefs.is_empty() {
        return None;
    }
    let mut best_compatible = None;
    let mut best_compatible_contextual = 0.0;
    let mut best_pitched = Vec::new();
    let mut best_key_grade = 0.0;
    for clef in &context.clefs {
        if clef.kind == NeutralClefKind::Percussion {
            continue;
        }
        let Some(pitched) = pitched_key_grades(&proposal, clef.kind, context) else {
            continue;
        };
        let key_grade = aggregate_key_grade(&pitched, context.key_alters_source_ratio);
        let contribution = contribution_of(key_grade, context.clef_key_source_ratio);
        let clef_contextual = contextual(clef.grade, contribution);
        if clef_contextual > best_compatible_contextual {
            best_compatible = Some(*clef);
            best_compatible_contextual = clef_contextual;
            best_pitched = pitched;
            best_key_grade = key_grade;
        }
    }
    let compatible = best_compatible?;
    let mut best_clef = None;
    let mut best_grade = -1.0;
    for clef in &context.clefs {
        let grade = if clef.id == compatible.id {
            best_compatible_contextual
        } else {
            clef.grade
        };
        if grade > best_grade {
            best_grade = grade;
            best_clef = Some(*clef);
        }
    }
    if best_clef?.id != compatible.id {
        return None;
    }
    let fifths = match proposal.shape {
        NeutralKeyAlterShape::Flat => -(proposal.alters.len() as i8),
        NeutralKeyAlterShape::Sharp => proposal.alters.len() as i8,
    };
    let mut bounds = proposal.alters[0].bounds;
    for alter in &proposal.alters[1..] {
        let right = bounds.right().max(alter.bounds.right());
        let bottom = (bounds.y + bounds.height - 1).max(alter.bounds.y + alter.bounds.height - 1);
        bounds.x = bounds.x.min(alter.bounds.x);
        bounds.y = bounds.y.min(alter.bounds.y);
        bounds.width = right - bounds.x + 1;
        bounds.height = bottom - bounds.y + 1;
    }
    let slices = proposal
        .alters
        .iter()
        .map(|alter| NeutralKeySlice {
            start: alter.start,
            width: alter.width,
            alter_id: Some(alter.id),
            alter_bounds: Some(alter.bounds),
        })
        .collect::<Vec<_>>();
    let intrinsic = best_pitched.iter().sum::<f64>() / best_pitched.len() as f64;
    Some((
        NeutralKeyCandidate {
            id: proposal.id,
            fifths,
            grade: intrinsic,
            contextual_grade: Some(best_key_grade),
            bounds,
            range: proposal.range,
            slices,
            in_sig: false,
            staff_id: Some(input.staff_id),
            frozen: false,
            removed: false,
        },
        compatible.id,
    ))
}

fn pitched_key_grades(
    proposal: &KeyShapeClassifierProposal,
    clef: NeutralClefKind,
    context: &KeyLifecycleContext,
) -> Option<Vec<f64>> {
    let expected = key_pitches(clef, proposal.shape)?;
    let count = proposal.alters.len();
    let maximum = if count >= 4 {
        context.maximum_delta_pitch_four
    } else {
        context.maximum_delta_pitch_one
            + (((context.maximum_delta_pitch_four - context.maximum_delta_pitch_one)
                * (count - 1) as f64)
                / 3.0)
    };
    let mut grades = Vec::with_capacity(count);
    for (index, alter) in proposal.alters.iter().enumerate() {
        let delta = (alter.measured_pitch - f64::from(expected[index])).abs();
        if delta > maximum {
            return None;
        }
        grades.push(alter.classifier_grade * (1.0 - (delta / maximum)));
    }
    Some(grades)
}

fn aggregate_key_grade(grades: &[f64], ratio: f64) -> f64 {
    let contributions = grades
        .iter()
        .map(|grade| contribution_of(*grade, ratio))
        .collect::<Vec<_>>();
    let mut key_grade = 0.0;
    for (index, grade) in grades.iter().enumerate() {
        let contribution = contributions
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, contribution)| contribution)
            .sum::<f64>();
        key_grade += contextual(*grade, contribution);
    }
    key_grade / grades.len() as f64
}

fn contribution_of(partner: f64, ratio: f64) -> f64 {
    partner * (ratio - 1.0)
}

fn contextual(intrinsic: f64, contribution: f64) -> f64 {
    ((1.0 + contribution) * intrinsic) / (1.0 + (contribution * intrinsic))
}

fn key_pitches(kind: NeutralClefKind, shape: NeutralKeyAlterShape) -> Option<&'static [i32; 7]> {
    use NeutralClefKind::{Alto, Baritone, Bass, MezzoSoprano, Soprano, Tenor, Treble};
    match (shape, kind) {
        (NeutralKeyAlterShape::Sharp, Treble) => Some(&[-4, -1, -5, -2, 1, -3, 0]),
        (NeutralKeyAlterShape::Sharp, Bass) => Some(&[-2, 1, -3, 0, 3, -1, 2]),
        (NeutralKeyAlterShape::Sharp, Baritone) => Some(&[0, 3, -1, 2, -2, 1, -3]),
        (NeutralKeyAlterShape::Sharp, Tenor) => Some(&[2, -2, 1, -3, 0, -4, -1]),
        (NeutralKeyAlterShape::Sharp, Alto) => Some(&[-3, 0, -4, -1, 2, -2, 1]),
        (NeutralKeyAlterShape::Sharp, MezzoSoprano) => Some(&[-1, 2, -2, 1, -3, 0, -4]),
        (NeutralKeyAlterShape::Sharp, Soprano) => Some(&[1, 4, 0, 3, -1, 2, -2]),
        (NeutralKeyAlterShape::Flat, Treble) => Some(&[0, -3, 1, -2, 2, -1, 3]),
        (NeutralKeyAlterShape::Flat, Bass) => Some(&[2, -1, 3, 0, 4, 1, 5]),
        (NeutralKeyAlterShape::Flat, Baritone) => Some(&[-3, 1, -2, 2, -1, 3, 0]),
        (NeutralKeyAlterShape::Flat, Tenor) => Some(&[-1, -4, 0, -3, 1, -2, 2]),
        (NeutralKeyAlterShape::Flat, Alto) => Some(&[1, -2, 2, -1, 3, 0, 4]),
        (NeutralKeyAlterShape::Flat, MezzoSoprano) => Some(&[-4, 0, -3, 1, -2, 2, -1]),
        (NeutralKeyAlterShape::Flat, Soprano) => Some(&[-2, 2, -1, 3, 0, 4, 1]),
        _ => None,
    }
}

const EM_EPSILON: f64 = 1e-10;
const EM_MAX_ITERATIONS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralKeySlice {
    pub start: i32,
    pub width: i32,
    pub alter_id: Option<usize>,
    pub alter_bounds: Option<HeaderBounds>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralKeyCandidate {
    pub id: usize,
    /// Negative flats or positive sharps; a zero-fifths key has no key inter.
    pub fifths: i8,
    pub grade: f64,
    pub contextual_grade: Option<f64>,
    pub bounds: HeaderBounds,
    pub range: StaffHeaderRange,
    pub slices: Vec<NeutralKeySlice>,
    pub in_sig: bool,
    pub staff_id: Option<usize>,
    pub frozen: bool,
    pub removed: bool,
}

impl NeutralKeyCandidate {
    #[must_use]
    pub fn best_grade(&self) -> f64 {
        self.contextual_grade.unwrap_or(self.grade)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRecognitionInput {
    pub system_id: usize,
    pub staff_id: usize,
    pub projection_width: i32,
    pub measure_start: i32,
    pub browse_start: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyReplicationStatus {
    Ok,
    NoClef,
    NoReplicate,
    Shrink,
    Destroy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyReplication {
    pub status: KeyReplicationStatus,
    /// Present when replication creates/replaces the target key candidate.
    pub replacement: Option<NeutralKeyCandidate>,
}

pub trait VisualKeyRecognizer {
    type Error;

    /// Java per-staff `KeyBuilder.process`, through its first pixel seam.
    fn recognize_keys(
        &mut self,
        input: KeyRecognitionInput,
    ) -> Result<Vec<NeutralKeyCandidate>, Self::Error>;

    /// Java `ShapeBuilder.checkReplicate`, which may require fresh extraction.
    fn replicate_key(
        &mut self,
        system_id: usize,
        target_staff_id: usize,
        source: &NeutralKeyCandidate,
        global_offsets: &[i32],
    ) -> Result<KeyReplication, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyVisualPhase {
    Recognize,
    Replicate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyColumnError<VisualError> {
    MissingHeader {
        staff_id: usize,
    },
    DuplicateInterId {
        staff_id: usize,
        inter_id: usize,
    },
    InvalidCandidate {
        staff_id: usize,
        inter_id: usize,
    },
    Visual {
        staff_id: usize,
        phase: KeyVisualPhase,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for KeyColumnError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { staff_id } => write!(formatter, "staff {staff_id} has no header"),
            Self::DuplicateInterId { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} key recognition duplicates live SIG inter {inter_id}"
            ),
            Self::InvalidCandidate { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} key candidate {inter_id} has zero fifths or no slices"
            ),
            Self::Visual {
                staff_id,
                phase,
                source,
            } => write!(
                formatter,
                "staff {staff_id} visual key {phase:?} failed: {source}"
            ),
        }
    }
}

impl<VisualError: Error + 'static> Error for KeyColumnError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Visual { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralKeyBuilderState {
    pub input: KeyRecognitionInput,
    /// Java `EnumMap<Shape, ShapeBuilder>` order: FLAT, then SHARP.
    pub candidates: Vec<NeutralKeyCandidate>,
}

pub struct HeadlessKeyColumn<Visual> {
    visual: Visual,
    max_slice_distance: i32,
    builders: BTreeMap<usize, NeutralKeyBuilderState>,
    global_offsets: Vec<i32>,
}

impl<Visual> HeadlessKeyColumn<Visual> {
    #[must_use]
    pub const fn new(visual: Visual, max_slice_distance: i32) -> Self {
        Self {
            visual,
            max_slice_distance,
            builders: BTreeMap::new(),
            global_offsets: Vec::new(),
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }

    #[must_use]
    pub fn builders(&self) -> &BTreeMap<usize, NeutralKeyBuilderState> {
        &self.builders
    }

    #[must_use]
    pub fn global_offsets(&self) -> &[i32] {
        &self.global_offsets
    }

    #[must_use]
    pub fn global_index(&self, offset: i32) -> Option<usize> {
        let mut best_index = None;
        let mut best_distance = f64::MAX;
        for (index, global) in self.global_offsets.iter().enumerate() {
            let distance = f64::from(global.wrapping_sub(offset)).abs();
            if best_distance > distance {
                best_distance = distance;
                best_index = Some(index);
            }
        }
        (best_distance <= f64::from(self.max_slice_distance))
            .then_some(best_index)
            .flatten()
    }
}

impl<Visual> HeadlessKeyColumn<Visual>
where
    Visual: VisualKeyRecognizer,
{
    /// Java `retrieveKeys`; all mutations before an injected failure remain.
    pub fn retrieve_keys(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        projection_width: i32,
    ) -> Result<i32, KeyColumnError<Visual::Error>> {
        self.builders.clear();
        self.global_offsets.clear();

        // Construct every builder first, in source staff order.
        for staff in &system.staffs {
            if staff.tablature {
                continue;
            }
            let header = staff
                .header
                .as_ref()
                .ok_or(KeyColumnError::MissingHeader { staff_id: staff.id })?;
            let browse_start = header
                .clef_range
                .as_ref()
                .and_then(StaffHeaderRange::precise_stop)
                .map_or_else(|| header.stop.wrapping_add(1), |stop| stop.wrapping_add(1));
            self.builders.insert(
                staff.id,
                NeutralKeyBuilderState {
                    input: KeyRecognitionInput {
                        system_id: system.id,
                        staff_id: staff.id,
                        projection_width,
                        measure_start: header.start,
                        browse_start,
                    },
                    candidates: Vec::new(),
                },
            );
        }

        // Java processes builders in Staff.byId TreeMap order.
        for staff_id in self.builders.keys().copied().collect::<Vec<_>>() {
            let input = self.builders[&staff_id].input;
            let candidates =
                self.visual
                    .recognize_keys(input)
                    .map_err(|source| KeyColumnError::Visual {
                        staff_id,
                        phase: KeyVisualPhase::Recognize,
                        source,
                    })?;
            self.register_candidates(system, staff_id, candidates)?;
        }

        if system.staffs.len() > 1 && !self.check_system_slices(system)? {
            self.destroy_all(system);
            return Ok(0);
        }

        // Finalize in TreeMap order, then compute the maximum in source order.
        for staff_id in self.builders.keys().copied().collect::<Vec<_>>() {
            self.finalize_staff(system, staff_id)?;
        }
        Ok(system
            .staffs
            .iter()
            .filter(|staff| !staff.tablature)
            .filter_map(|staff| {
                let header = staff.header.as_ref()?;
                let stop = header.key.as_ref()?.bounds.right();
                Some(stop.wrapping_sub(header.start))
            })
            .max()
            .unwrap_or(0))
    }

    fn register_candidates(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        staff_id: usize,
        mut candidates: Vec<NeutralKeyCandidate>,
    ) -> Result<(), KeyColumnError<Visual::Error>> {
        // Shape enum order is FLAT before SHARP, independent of recognizer order.
        candidates.sort_by_key(|candidate| candidate.fifths > 0);
        for mut candidate in candidates {
            if candidate.fifths == 0 || candidate.slices.is_empty() {
                return Err(KeyColumnError::InvalidCandidate {
                    staff_id,
                    inter_id: candidate.id,
                });
            }
            let ids = std::iter::once(candidate.id)
                .chain(candidate.slices.iter().filter_map(|slice| slice.alter_id));
            for id in ids {
                if system.sig_vertex_ids.contains(&id) {
                    return Err(KeyColumnError::DuplicateInterId {
                        staff_id,
                        inter_id: id,
                    });
                }
                system.sig_vertex_ids.push(id);
            }
            candidate.in_sig = true;
            candidate.staff_id = Some(staff_id);
            self.builders
                .get_mut(&staff_id)
                .expect("builder was initialized")
                .candidates
                .push(candidate);
        }
        Ok(())
    }

    fn check_system_slices(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<bool, KeyColumnError<Visual::Error>> {
        self.global_offsets = self.compute_global_offsets();
        if self.global_offsets.is_empty() {
            return Ok(false);
        }

        let mut part_order = Vec::new();
        for staff in &system.staffs {
            if !part_order.contains(&staff.part_id) {
                part_order.push(staff.part_id);
            }
        }
        for part_id in part_order {
            let staves = system
                .staffs
                .iter()
                .filter(|staff| staff.part_id == part_id && !staff.tablature)
                .map(|staff| staff.id)
                .collect::<Vec<_>>();
            if staves.len() <= 1 {
                continue;
            }
            let Some((source_staff, source_index)) = self.best_in(&staves) else {
                continue;
            };
            loop {
                let source = self.builders[&source_staff].candidates[source_index].clone();
                let mut modified = false;
                for &staff_id in &staves {
                    if staff_id == source_staff {
                        self.destroy_opposite(system, staff_id, source.fifths);
                        continue;
                    }
                    if self.builders[&staff_id]
                        .candidates
                        .iter()
                        .any(|candidate| !candidate.removed && candidate.fifths == source.fifths)
                    {
                        continue;
                    }
                    let result = self
                        .visual
                        .replicate_key(system.id, staff_id, &source, &self.global_offsets)
                        .map_err(|source| KeyColumnError::Visual {
                            staff_id,
                            phase: KeyVisualPhase::Replicate,
                            source,
                        })?;
                    match result.status {
                        KeyReplicationStatus::Ok => {
                            if let Some(replacement) = result.replacement {
                                self.destroy_staff_candidates(system, staff_id);
                                self.register_candidates(system, staff_id, vec![replacement])?;
                            }
                        }
                        KeyReplicationStatus::NoClef | KeyReplicationStatus::NoReplicate => {}
                        KeyReplicationStatus::Shrink => {
                            self.global_offsets.pop();
                            self.shrink_candidate(system, source_staff, source_index);
                            modified = true;
                            break;
                        }
                        KeyReplicationStatus::Destroy => return Ok(false),
                    }
                }
                if !modified {
                    break;
                }
            }
        }
        Ok(true)
    }

    fn compute_global_offsets(&self) -> Vec<i32> {
        let mut populations: Vec<Vec<f64>> = Vec::new();
        let mut values = Vec::new();
        for builder in self.builders.values() {
            let Some(best) = best_candidate_index(&builder.candidates) else {
                continue;
            };
            for (index, slice) in builder.candidates[best].slices.iter().enumerate() {
                let offset = f64::from(slice.start.wrapping_sub(builder.input.measure_start));
                if index == populations.len() {
                    populations.push(Vec::new());
                }
                populations[index].push(offset);
                values.push(offset);
            }
        }
        if populations.is_empty() {
            return Vec::new();
        }
        let means = populations
            .iter()
            .map(|population| population.iter().sum::<f64>() / population.len() as f64)
            .collect::<Vec<_>>();
        gaussian_em(&values, means)
            .into_iter()
            .map(java_rint_to_i32)
            .collect()
    }

    fn best_in(&self, staves: &[usize]) -> Option<(usize, usize)> {
        let mut best = None;
        let mut best_grade = -1.0;
        for &staff_id in staves {
            let Some(index) = best_candidate_index(&self.builders[&staff_id].candidates) else {
                continue;
            };
            let grade = self.builders[&staff_id].candidates[index].best_grade();
            if best.is_none() || grade > best_grade {
                best = Some((staff_id, index));
                best_grade = grade;
            }
        }
        best
    }

    fn finalize_staff(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        staff_id: usize,
    ) -> Result<(), KeyColumnError<Visual::Error>> {
        let Some(best) = best_candidate_index(&self.builders[&staff_id].candidates) else {
            return Ok(());
        };
        let fifths = self.builders[&staff_id].candidates[best].fifths;
        self.destroy_opposite(system, staff_id, fifths);
        let candidate = &mut self.builders.get_mut(&staff_id).unwrap().candidates[best];
        candidate.frozen = true;
        let staff = system
            .staffs
            .iter_mut()
            .find(|staff| staff.id == staff_id)
            .ok_or(KeyColumnError::MissingHeader { staff_id })?;
        let header = staff
            .header
            .as_mut()
            .ok_or(KeyColumnError::MissingHeader { staff_id })?;
        header.key = Some(HeaderComponent::new(candidate.id, candidate.bounds));
        header.key_range = Some(candidate.range.clone());
        header.alter_starts = Some(candidate.slices.iter().map(|slice| slice.start).collect());
        Ok(())
    }

    fn destroy_opposite(&mut self, system: &mut HeadlessHeaderSystem, staff_id: usize, fifths: i8) {
        let indices = self.builders[&staff_id]
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                !candidate.removed && candidate.fifths.signum() != fifths.signum()
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        for index in indices {
            destroy_candidate(
                system,
                &mut self.builders.get_mut(&staff_id).unwrap().candidates[index],
            );
        }
    }

    fn destroy_staff_candidates(&mut self, system: &mut HeadlessHeaderSystem, staff_id: usize) {
        for candidate in &mut self.builders.get_mut(&staff_id).unwrap().candidates {
            destroy_candidate(system, candidate);
        }
    }

    fn destroy_all(&mut self, system: &mut HeadlessHeaderSystem) {
        for builder in self.builders.values_mut() {
            for candidate in &mut builder.candidates {
                destroy_candidate(system, candidate);
            }
        }
    }

    fn shrink_candidate(
        &mut self,
        system: &mut HeadlessHeaderSystem,
        staff_id: usize,
        index: usize,
    ) {
        let candidate = &mut self.builders.get_mut(&staff_id).unwrap().candidates[index];
        if let Some(slice) = candidate.slices.pop() {
            if let Some(alter_id) = slice.alter_id {
                remove_sig_id(system, alter_id);
            }
            candidate.fifths = candidate.fifths.signum() * candidate.slices.len() as i8;
            if let Some(last) = candidate.slices.last().and_then(|slice| slice.alter_bounds) {
                candidate.bounds.width = last
                    .right()
                    .wrapping_sub(candidate.bounds.x)
                    .wrapping_add(1);
            }
        }
    }
}

fn best_candidate_index(candidates: &[NeutralKeyCandidate]) -> Option<usize> {
    let mut best = None;
    let mut best_grade = 0.0;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.removed {
            continue;
        }
        let grade = candidate.best_grade();
        if best.is_none() || best_grade < grade {
            best = Some(index);
            best_grade = grade;
        }
    }
    best
}

fn destroy_candidate(system: &mut HeadlessHeaderSystem, candidate: &mut NeutralKeyCandidate) {
    if candidate.removed {
        return;
    }
    remove_sig_id(system, candidate.id);
    for alter_id in candidate.slices.iter().filter_map(|slice| slice.alter_id) {
        remove_sig_id(system, alter_id);
    }
    candidate.in_sig = false;
    candidate.removed = true;
}

fn remove_sig_id(system: &mut HeadlessHeaderSystem, id: usize) {
    system.sig_vertex_ids.retain(|candidate| *candidate != id);
    system
        .sig_exclusions
        .retain(|exclusion| exclusion.one != id && exclusion.two != id);
}

fn gaussian_em(values: &[f64], mut means: Vec<f64>) -> Vec<f64> {
    let group_count = means.len();
    let sample_count = values.len();
    let mut sigmas = vec![1.0; group_count];
    let mut proportions = vec![1.0 / group_count as f64; group_count];
    let mut memberships = vec![vec![0.0; sample_count]; group_count];
    for _ in 0..EM_MAX_ITERATIONS {
        for (sample, value) in values.iter().copied().enumerate() {
            let denominator = (0..group_count)
                .map(|group| {
                    proportions[group] * gaussian_probability(value, means[group], sigmas[group])
                })
                .sum::<f64>();
            for group in 0..group_count {
                memberships[group][sample] = proportions[group]
                    * gaussian_probability(value, means[group], sigmas[group])
                    / denominator;
            }
        }
        let mut convergence = 0.0;
        for group in 0..group_count {
            let next = memberships[group].iter().sum::<f64>() / sample_count as f64;
            convergence += (next - proportions[group]).powi(2);
            proportions[group] = next;
            let weight = memberships[group].iter().sum::<f64>();
            means[group] = memberships[group]
                .iter()
                .zip(values)
                .map(|(membership, value)| membership * value)
                .sum::<f64>()
                / weight;
            sigmas[group] = (memberships[group]
                .iter()
                .zip(values)
                .map(|(membership, value)| membership * (value - means[group]).powi(2))
                .sum::<f64>()
                / weight)
                .sqrt();
        }
        if convergence < EM_EPSILON {
            break;
        }
    }
    means
}

fn gaussian_probability(value: f64, mean: f64, sigma: f64) -> f64 {
    let distance = value - mean;
    if sigma <= EM_EPSILON {
        if distance.abs() <= EM_EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        (-distance.powi(2) / (2.0 * sigma.powi(2))).exp() / (sigma * TAU.sqrt())
    }
}

fn java_rint_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headers_step::HeadlessHeaderStaff, staff_header::StaffHeader};

    #[derive(Default)]
    struct FakeVisual {
        recognition: BTreeMap<usize, Result<Vec<NeutralKeyCandidate>, &'static str>>,
        replications: Vec<KeyReplication>,
        recognized: Vec<KeyRecognitionInput>,
        replicated: Vec<(usize, i8, Vec<i32>)>,
    }

    #[derive(Default)]
    struct FakeProposalVisual {
        proposals: BTreeMap<usize, Vec<KeyShapeClassifierProposal>>,
    }

    impl VisualKeyProposalRecognizer for FakeProposalVisual {
        type Error = &'static str;

        fn classify_key_shapes(
            &mut self,
            input: KeyRecognitionInput,
        ) -> Result<Vec<KeyShapeClassifierProposal>, Self::Error> {
            Ok(self.proposals.remove(&input.staff_id).unwrap_or_default())
        }

        fn replicate_key(
            &mut self,
            _system_id: usize,
            _target_staff_id: usize,
            _source: &NeutralKeyCandidate,
            _global_offsets: &[i32],
        ) -> Result<KeyReplication, Self::Error> {
            Ok(KeyReplication {
                status: KeyReplicationStatus::NoReplicate,
                replacement: None,
            })
        }
    }

    impl VisualKeyRecognizer for FakeVisual {
        type Error = &'static str;

        fn recognize_keys(
            &mut self,
            input: KeyRecognitionInput,
        ) -> Result<Vec<NeutralKeyCandidate>, Self::Error> {
            self.recognized.push(input);
            self.recognition
                .remove(&input.staff_id)
                .unwrap_or(Ok(Vec::new()))
        }

        fn replicate_key(
            &mut self,
            _system_id: usize,
            target_staff_id: usize,
            source: &NeutralKeyCandidate,
            global_offsets: &[i32],
        ) -> Result<KeyReplication, Self::Error> {
            self.replicated
                .push((target_staff_id, source.fifths, global_offsets.to_vec()));
            Ok(self.replications.remove(0))
        }
    }

    fn bounds(x: i32, width: i32) -> HeaderBounds {
        HeaderBounds {
            x,
            y: 2,
            width,
            height: 20,
        }
    }

    fn key(id: usize, fifths: i8, grade: f64, start: i32) -> NeutralKeyCandidate {
        let count = fifths.unsigned_abs() as usize;
        let slices = (0..count)
            .map(|index| {
                let x = start + (index as i32 * 10);
                NeutralKeySlice {
                    start: x,
                    width: 5,
                    alter_id: Some(id + index + 1),
                    alter_bounds: Some(bounds(x, 5)),
                }
            })
            .collect::<Vec<_>>();
        let right = slices.last().unwrap().alter_bounds.unwrap().right();
        let mut range = StaffHeaderRange::default();
        range.valid = true;
        range.browse_start = start - 2;
        range.browse_stop = right + 8;
        range.set_start(start);
        range.set_stop(right);
        NeutralKeyCandidate {
            id,
            fifths,
            grade,
            contextual_grade: None,
            bounds: bounds(start, right - start + 1),
            range,
            slices,
            in_sig: false,
            staff_id: None,
            frozen: false,
            removed: false,
        }
    }

    fn staff(id: usize, part_id: usize, start: i32, clef_stop: Option<i32>) -> HeadlessHeaderStaff {
        let mut staff = HeadlessHeaderStaff::new(id);
        staff.part_id = part_id;
        let mut header = StaffHeader::new(start);
        header.stop = start + 2;
        if let Some(stop) = clef_stop {
            let mut range = StaffHeaderRange::default();
            range.set_stop(stop);
            header.clef_range = Some(range);
        }
        staff.header = Some(header);
        staff
    }

    #[test]
    fn builders_construct_in_source_order_then_recognize_in_staff_id_order() {
        let mut tablature = staff(2, 2, 30, None);
        tablature.tablature = true;
        let mut system = HeadlessHeaderSystem::new(
            9,
            vec![staff(5, 5, 10, Some(18)), tablature, staff(3, 3, 20, None)],
        );
        let mut visual = FakeVisual::default();
        visual.recognition.insert(5, Ok(vec![key(50, 2, 0.8, 20)]));
        visual.recognition.insert(3, Ok(vec![key(30, -1, 0.7, 24)]));
        let mut column = HeadlessKeyColumn::new(visual, 4);

        assert_eq!(column.retrieve_keys(&mut system, 80), Ok(24));
        assert_eq!(
            column
                .visual()
                .recognized
                .iter()
                .map(|input| input.staff_id)
                .collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert_eq!(column.builders()[&5].input.browse_start, 19);
        assert_eq!(column.builders()[&3].input.browse_start, 23);
        assert!(system.staffs[1].header.as_ref().unwrap().key.is_none());
        assert_eq!(
            system.staffs[0]
                .header
                .as_ref()
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .id,
            50
        );
        assert_eq!(
            system.staffs[2]
                .header
                .as_ref()
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .id,
            30
        );
    }

    #[test]
    fn flat_sharp_candidates_finalize_contextual_winner_and_remove_loser_sig() {
        let mut system = HeadlessHeaderSystem::new(9, vec![staff(1, 1, 10, None)]);
        let mut sharp = key(20, 2, 0.9, 15);
        let mut flat = key(10, -2, 0.4, 15);
        flat.contextual_grade = Some(1.1);
        sharp.contextual_grade = Some(0.95);
        let mut visual = FakeVisual::default();
        visual.recognition.insert(1, Ok(vec![sharp, flat]));
        let mut column = HeadlessKeyColumn::new(visual, 4);

        assert_eq!(column.retrieve_keys(&mut system, 80), Ok(19));
        assert_eq!(column.builders()[&1].candidates[0].fifths, -2);
        assert!(column.builders()[&1].candidates[0].frozen);
        assert!(column.builders()[&1].candidates[1].removed);
        assert_eq!(system.sig_vertex_ids, vec![10, 11, 12]);
        assert_eq!(
            system.staffs[0].header.as_ref().unwrap().alter_starts,
            Some(vec![15, 25])
        );
    }

    #[test]
    fn multi_staff_part_replicates_best_and_uses_aggregated_offsets() {
        let mut system =
            HeadlessHeaderSystem::new(9, vec![staff(5, 1, 10, None), staff(3, 1, 20, None)]);
        let mut visual = FakeVisual::default();
        visual.recognition.insert(5, Ok(vec![key(50, 2, 0.9, 20)]));
        visual.recognition.insert(3, Ok(Vec::new()));
        visual.replications.push(KeyReplication {
            status: KeyReplicationStatus::Ok,
            replacement: Some(key(30, 2, 0.8, 30)),
        });
        let mut column = HeadlessKeyColumn::new(visual, 4);

        assert_eq!(column.retrieve_keys(&mut system, 80), Ok(24));
        assert_eq!(column.global_offsets(), &[10, 20]);
        assert_eq!(column.visual().replicated, vec![(3, 2, vec![10, 20])]);
        assert_eq!(
            system.staffs[1]
                .header
                .as_ref()
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .id,
            30
        );
    }

    #[test]
    fn incompatible_system_destroys_every_registered_candidate_and_returns_zero() {
        let mut system =
            HeadlessHeaderSystem::new(9, vec![staff(1, 1, 10, None), staff(2, 1, 20, None)]);
        let mut visual = FakeVisual::default();
        visual.recognition.insert(1, Ok(vec![key(10, 1, 0.9, 15)]));
        visual.recognition.insert(2, Ok(Vec::new()));
        visual.replications.push(KeyReplication {
            status: KeyReplicationStatus::Destroy,
            replacement: None,
        });
        let mut column = HeadlessKeyColumn::new(visual, 4);

        assert_eq!(column.retrieve_keys(&mut system, 80), Ok(0));
        assert!(system.sig_vertex_ids.is_empty());
        assert!(column.builders()[&1].candidates[0].removed);
        assert!(
            system
                .staffs
                .iter()
                .all(|staff| staff.header.as_ref().unwrap().key.is_none())
        );
    }

    #[test]
    fn recognition_failure_keeps_completed_staff_prefix_and_empty_failed_builder() {
        let mut system =
            HeadlessHeaderSystem::new(9, vec![staff(1, 1, 10, None), staff(2, 2, 20, None)]);
        let mut visual = FakeVisual::default();
        visual.recognition.insert(1, Ok(vec![key(10, 1, 0.9, 15)]));
        visual.recognition.insert(2, Err("projection failed"));
        let mut column = HeadlessKeyColumn::new(visual, 4);

        assert_eq!(
            column.retrieve_keys(&mut system, 80),
            Err(KeyColumnError::Visual {
                staff_id: 2,
                phase: KeyVisualPhase::Recognize,
                source: "projection failed"
            })
        );
        assert_eq!(system.sig_vertex_ids, vec![10, 11]);
        assert_eq!(column.builders()[&1].candidates.len(), 1);
        assert!(column.builders()[&2].candidates.is_empty());
    }

    #[test]
    fn nearest_global_index_uses_strict_first_tie_and_distance_gate() {
        let mut column = HeadlessKeyColumn::new(FakeVisual::default(), 3);
        column.global_offsets = vec![10, 20];
        assert_eq!(column.global_index(15), None);
        assert_eq!(column.global_index(12), Some(0));
        column.max_slice_distance = 5;
        assert_eq!(column.global_index(15), Some(0));
    }

    fn shape_proposal(
        id: usize,
        shape: NeutralKeyAlterShape,
        pitches: &[f64],
    ) -> KeyShapeClassifierProposal {
        let mut range = StaffHeaderRange::default();
        range.browse_start = 12;
        range.browse_stop = 50;
        range.set_start(15);
        range.set_stop(44);
        KeyShapeClassifierProposal {
            id,
            shape,
            range,
            alters: pitches
                .iter()
                .enumerate()
                .rev()
                .map(|(index, pitch)| KeyAlterClassifierProposal {
                    id: id + index + 1,
                    start: 15 + index as i32 * 8,
                    width: 5,
                    bounds: bounds(15 + index as i32 * 8, 5),
                    classifier_grade: 0.8,
                    measured_pitch: *pitch,
                })
                .collect(),
        }
    }

    fn lifecycle_context(clefs: Vec<KeyClefSupport>) -> KeyLifecycleContext {
        KeyLifecycleContext {
            clefs,
            maximum_delta_pitch_one: 0.5,
            maximum_delta_pitch_four: 0.8,
            clef_key_source_ratio: 2.0,
            key_alters_source_ratio: 2.0,
        }
    }

    #[test]
    fn key_lifecycle_orders_alters_applies_pitch_impact_and_selects_compatible_clef() {
        let mut visual = FakeProposalVisual::default();
        visual.proposals.insert(
            1,
            vec![shape_proposal(
                100,
                NeutralKeyAlterShape::Sharp,
                &[-4.0, -0.9],
            )],
        );
        let contexts = BTreeMap::from([(
            1,
            lifecycle_context(vec![
                KeyClefSupport {
                    id: 10,
                    kind: NeutralClefKind::Treble,
                    grade: 0.7,
                },
                KeyClefSupport {
                    id: 11,
                    kind: NeutralClefKind::Bass,
                    grade: 0.6,
                },
            ]),
        )]);
        let mut lifecycle = KeyLifecycleRecognizer::new(visual, contexts);
        let candidates = lifecycle
            .recognize_keys(KeyRecognitionInput {
                system_id: 7,
                staff_id: 1,
                projection_width: 80,
                measure_start: 10,
                browse_start: 12,
            })
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fifths, 2);
        assert_eq!(
            candidates[0]
                .slices
                .iter()
                .map(|slice| slice.start)
                .collect::<Vec<_>>(),
            vec![15, 23]
        );
        assert_eq!(lifecycle.selected_clefs()[&1], 10);
        assert!(candidates[0].contextual_grade.unwrap() > candidates[0].grade);
    }

    #[test]
    fn key_lifecycle_rejects_pitch_outside_java_count_scaled_window() {
        let mut visual = FakeProposalVisual::default();
        visual.proposals.insert(
            1,
            vec![shape_proposal(100, NeutralKeyAlterShape::Flat, &[2.0, 0.0])],
        );
        let contexts = BTreeMap::from([(
            1,
            lifecycle_context(vec![KeyClefSupport {
                id: 10,
                kind: NeutralClefKind::Treble,
                grade: 0.7,
            }]),
        )]);
        let mut lifecycle = KeyLifecycleRecognizer::new(visual, contexts);
        assert!(
            lifecycle
                .recognize_keys(KeyRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    projection_width: 80,
                    measure_start: 10,
                    browse_start: 12,
                })
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn stronger_incompatible_clef_defeats_key_supported_clef_with_strict_java_comparison() {
        let mut visual = FakeProposalVisual::default();
        visual.proposals.insert(
            1,
            vec![shape_proposal(100, NeutralKeyAlterShape::Sharp, &[-4.0])],
        );
        let contexts = BTreeMap::from([(
            1,
            lifecycle_context(vec![
                KeyClefSupport {
                    id: 10,
                    kind: NeutralClefKind::Treble,
                    grade: 0.2,
                },
                KeyClefSupport {
                    id: 11,
                    kind: NeutralClefKind::Bass,
                    grade: 0.99,
                },
            ]),
        )]);
        let mut lifecycle = KeyLifecycleRecognizer::new(visual, contexts);
        assert!(
            lifecycle
                .recognize_keys(KeyRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    projection_width: 80,
                    measure_start: 10,
                    browse_start: 12,
                })
                .unwrap()
                .is_empty()
        );
    }
}
