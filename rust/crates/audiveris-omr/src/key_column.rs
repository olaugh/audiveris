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

use audiveris_image::{
    glyph_factory::{GlyphComponent, build_glyph_components},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

use audiveris_core::integer_function::IntegerFunction;
use audiveris_core::peak_finder::{HiLoPeakFinder, Quorum};
use audiveris_music_font::{FLAT_MASS_PITCH_OFFSET, MusicFamily, area_pitch_offset};

use crate::clef_column::{chamfer_gap, component_pixels, push_unique};
use crate::key_parameters::KeyPipelineParameters;
use crate::key_peaks::{
    KeyPeak, KeyShapeKind, allocate_slices, browse_area, compute_starts, infer_signature,
    merge_peaks, purge_light_peaks, refine_signature,
};
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
            // Java `KeyColumn`: `browseStart = (clefStop != null) ? clefStop + 1
            // : staff.getHeaderStop() + 1`, where `clefStop` is `Staff.getClefStop()` — which
            // recomputes from the clef's bounds rather than returning the stored range value. See
            // `StaffHeader::clef_stop`; reading the stored value here was wrong on bass clefs.
            let browse_start = header
                .clef_stop()
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeKeyStaffContext {
    pub browse_stop: i32,
    pub envelope_top: i32,
    pub envelope_bottom: i32,
    pub staff_mid_y: f64,
    /// The interline handed to the **classifier**, which Java takes from the *sheet*.
    ///
    /// Named for its one use on purpose. A single `interline` field used to serve both this and
    /// the pitch computation, but Java takes them from different places -- `sheet.getInterline()`
    /// here, and the staff's own measured lines for the pitch -- so on a mixed-size sheet no
    /// single value is right for both. The pitch now comes from [`StaffPitchGeometry`] instead.
    pub classifier_interline: i32,
    /// Number of lines on this staff, Java's `lines.size()` in `pitchPositionOf`.
    pub line_count: usize,
    /// The staff's specific interline, which scales `refineAreaStart`'s envelope and the
    /// trailing-space rectangle. Distinct from `classifier_interline` on a mixed-size sheet.
    pub staff_interline: i32,
}

/// The staff geometry `Staff.pitchPositionOf` reads, at a given abscissa.
///
/// Java evaluates `getFirstLine().yAt(x)` and `getLastLine().yAt(x)` as **doubles** from the
/// splines, and derives the pitch from their separation rather than from any interline. Those
/// agree only where a staff's lines happen to be exactly `(lines - 1)` nominal interlines apart,
/// which no real staff is at every abscissa.
pub trait StaffPitchGeometry {
    /// `(first line y, last line y)` at `x`, unrounded.
    ///
    /// `None` suppresses the pitch rather than inventing one, matching the deliberate refusal to
    /// extrapolate outside a spline's range.
    fn line_span_at(&self, staff_id: usize, x: f64) -> Option<(f64, f64)>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeKeyParameters {
    pub minimum_component_weight: usize,
    /// Java `maxPartGap`, a `toPixelsDouble` distance. Kept fractional: it feeds a chamfer
    /// distance comparison, which Java performs in double.
    pub maximum_component_gap: f64,
    /// Java `isTooLight`.
    pub minimum_glyph_weight: usize,
    /// Java `isTooHeavy`. Previously absent, so an over-heavy compound was accepted.
    pub maximum_glyph_weight: usize,
    /// Java `isTooSmall`, width half. Previously absent.
    ///
    /// `f64` because Java compares the integer `Rectangle` dimension against an unrounded
    /// `toPixelsDouble` threshold; rounding it first moves the boundary by up to half a pixel.
    pub minimum_alter_width: f64,
    /// Java `isTooSmall`, height half. Previously absent.
    pub minimum_alter_height: f64,
    /// Java `isTooLarge`, width half.
    pub maximum_alter_width: f64,
    /// Java `isTooLarge`, height half.
    pub maximum_alter_height: f64,
    pub maximum_alters: usize,
    pub maximum_rank: usize,
    pub minimum_classifier_grade: f64,
    /// The projection-peak constants driving signature inference and slice allocation.
    pub pipeline: KeyPipelineParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeKeyGlyph {
    pub id: usize,
    pub part_ids: Vec<usize>,
    pub bounds: HeaderBounds,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub raster: RunTable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyShapeEvaluation {
    pub shape: NeutralKeyAlterShape,
    pub grade: f64,
}

/// The sole production seam: Audiveris `ShapeClassifier` output.
pub trait KeyShapeClassifier {
    type Error;
    fn evaluate(
        &mut self,
        glyph: &NativeKeyGlyph,
        interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<KeyShapeEvaluation>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeKeyMutation {
    ComponentRegistered {
        staff_id: usize,
        glyph_id: usize,
    },
    CompoundRegistered {
        staff_id: usize,
        glyph_id: usize,
    },
    GlyphEvaluated {
        staff_id: usize,
        glyph_id: usize,
        shape: NeutralKeyAlterShape,
    },
    ClassifierRejected {
        staff_id: usize,
        glyph_id: usize,
        shape: NeutralKeyAlterShape,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeKeyError<E> {
    MissingSource(usize),
    MissingContext(usize),
    MissingParameters(usize),
    InvalidBrowseRange {
        staff_id: usize,
    },
    GlyphIdExhausted,
    InterIdExhausted,
    RunTable(RunTableError),
    Classifier {
        staff_id: usize,
        glyph_id: usize,
        source: E,
    },
}

impl<E: fmt::Display> fmt::Display for NativeKeyError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(id) => write!(f, "missing key raster for system {id}"),
            Self::MissingContext(id) => write!(f, "missing key context for staff {id}"),
            Self::MissingParameters(id) => write!(f, "missing key parameters for staff {id}"),
            Self::InvalidBrowseRange { staff_id } => {
                write!(f, "invalid key browse range for staff {staff_id}")
            }
            Self::GlyphIdExhausted => f.write_str("key glyph ID exhausted"),
            Self::InterIdExhausted => f.write_str("key inter ID exhausted"),
            Self::RunTable(source) => write!(f, "key run table failed: {source}"),
            Self::Classifier {
                staff_id,
                glyph_id,
                source,
            } => {
                write!(
                    f,
                    "staff {staff_id} key glyph {glyph_id} classifier failed: {source}"
                )
            }
        }
    }
}

#[derive(Clone)]
struct NativeKeyPart {
    id: usize,
    component: GlyphComponent,
}

/// Concrete staff envelope → projection range → connected glyphs → accidental
/// proposals. Hypotheses are evaluated in Java `Shape` order: FLAT, then SHARP.
/// Raw classifier floors: `Grades.keyAlterMinGrade1 / intrinsicRatio` and the phase-2 twin.
const PHASE_ONE_MINIMUM_GRADE: f64 = 0.1 / 0.8;
const PHASE_TWO_MINIMUM_GRADE: f64 = 0.01 / 0.8;

/// One slot of Java's `KeyRoi`: a slice with its best glyph so far.
#[derive(Clone, Debug)]
struct SliceState {
    start: i32,
    stop: i32,
    glyph: Option<NativeKeyGlyph>,
    grade: Option<f64>,
    /// Measured pitch of the assigned glyph, captured at assignment so `checkWithClefs` can read
    /// it without re-deriving.
    pitch: Option<f64>,
}

/// Black-pixel count for one column over `height` rows, Java's projection cell.
fn column_cumul(source: &RunTable, x: i32, y_min: i32, height: i32) -> i32 {
    let Ok(x) = usize::try_from(x) else { return 0 };
    if x >= source.width() {
        return 0;
    }
    let mut cumul = 0;
    for y in y_min..y_min + height {
        let Ok(y) = usize::try_from(y) else { continue };
        if y < source.height() && source.get(x, y) == FOREGROUND {
            cumul += 1;
        }
    }
    cumul
}

/// Java `KeyExtractor.hasStem`: a sliding window of `core_length` rows must reach the black-row
/// quorum somewhere under the peak.
fn has_stem_under(
    source: &RunTable,
    x_min: i32,
    width: i32,
    y_min: i32,
    height: i32,
    core_length: i32,
    min_black_ratio: f64,
) -> bool {
    let height = usize::try_from(height).unwrap_or(0);
    let core = usize::try_from(core_length).unwrap_or(0);
    if core == 0 || height < core {
        return false;
    }
    let mut blacks = vec![false; height];
    for (row, black) in blacks.iter_mut().enumerate() {
        let y = y_min + i32::try_from(row).unwrap_or(0);
        for x in x_min..x_min + width {
            let (Ok(ux), Ok(uy)) = (usize::try_from(x), usize::try_from(y)) else {
                continue;
            };
            if ux < source.width() && uy < source.height() && source.get(ux, uy) == FOREGROUND {
                *black = true;
                break;
            }
        }
    }
    let quorum = (f64::from(core_length) * min_black_ratio).round_ties_even() as usize;
    let mut count = blacks[..core].iter().filter(|&&black| black).count();
    if count >= quorum {
        return true;
    }
    for y in 1..=(height - core) {
        if blacks[y - 1] {
            count -= 1;
        }
        if blacks[y + core - 1] {
            count += 1;
        }
        if count >= quorum {
            return true;
        }
    }
    false
}

/// Java `KeyExtractor.isRatherEmpty`: a run of at least `min_width` consecutive near-empty
/// columns inside the rectangle.
fn is_rather_empty(source: &RunTable, rect: HeaderBounds, max_cumul: i32, min_width: i32) -> bool {
    let mut space_start = None;
    for x in rect.x..=rect.right() {
        let cumul = column_cumul(source, x, rect.y, rect.height);
        if cumul <= max_cumul {
            match space_start {
                None => space_start = Some(x),
                Some(start) if (x - start + 1) >= min_width => return true,
                Some(_) => {}
            }
        } else {
            space_start = None;
        }
    }
    false
}

/// Java `AbstractKeyAdapter.embracesSlicePeaks`: every peak centre inside the slice must fall
/// within the glyph's abscissa span. The glyph test is `GeoUtil.xEmbraces(Rectangle, double)`,
/// which is half-open on the right — and the centre is a half-integer whenever a peak has even
/// width, so the open end is reachable.
fn embraces_slice_peaks(
    slice_start: i32,
    slice_stop: i32,
    glyph: HeaderBounds,
    peaks: &[KeyPeak],
) -> bool {
    for peak in peaks {
        let center = peak.center();
        if f64::from(slice_start) <= center && center <= f64::from(slice_stop) {
            let embraced =
                center >= f64::from(glyph.x) && center < f64::from(glyph.x + glyph.width);
            if !embraced {
                return false;
            }
        }
    }
    true
}

/// Java's four `AbstractKeyAdapter` size/weight predicates, together.
fn accepts_glyph(glyph: &NativeKeyGlyph, parameters: &NativeKeyParameters) -> bool {
    let width = f64::from(glyph.bounds.width);
    let height = f64::from(glyph.bounds.height);
    width >= parameters.minimum_alter_width
        && height >= parameters.minimum_alter_height
        && width <= parameters.maximum_alter_width
        && height <= parameters.maximum_alter_height
        && glyph.weight >= parameters.minimum_glyph_weight
        && glyph.weight <= parameters.maximum_glyph_weight
}

/// Java `KeyRoi.getSlicePixels`: the slice crop, with the chosen glyphs of adjacent slices
/// erased so a shared edge does not re-enter the extraction.
fn slice_crop(
    source: &RunTable,
    rect: HeaderBounds,
    neighbours: &[NativeKeyGlyph],
) -> Result<RunTable, RunTableError> {
    let x = usize::try_from(rect.x).map_err(|_| RunTableError::OutOfBounds)?;
    let y = usize::try_from(rect.y).map_err(|_| RunTableError::OutOfBounds)?;
    let width = usize::try_from(rect.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(rect.height).map_err(|_| RunTableError::InvalidDimensions)?;
    if x + width > source.width() || y + height > source.height() {
        return Err(RunTableError::OutOfBounds);
    }
    let mut pixels = vec![BACKGROUND; width * height];
    for local_y in 0..height {
        for local_x in 0..width {
            pixels[local_y * width + local_x] = source.get(x + local_x, y + local_y);
        }
    }
    for neighbour in neighbours {
        for (px, py) in neighbour
            .raster
            .foreground_points((neighbour.bounds.x, neighbour.bounds.y))
        {
            let local_x = px - rect.x;
            let local_y = py - rect.y;
            if local_x >= 0
                && local_y >= 0
                && (local_x as usize) < width
                && (local_y as usize) < height
            {
                pixels[local_y as usize * width + local_x as usize] = BACKGROUND;
            }
        }
    }
    RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)
}

/// Java `KeyExtractor.purgeCandidates`: no part may be shared by two surviving candidates.
///
/// Sort by decreasing grade, then walk in that order and drop every *later* candidate that shares
/// any part with the current one. The best reading of a piece of ink therefore wins outright and
/// every overlapping alternative built from the same components disappears.
///
/// Ties keep their enumeration order: Java sorts with `Double.compare` through
/// `Collections.sort`, which is stable.
fn purge_key_candidates(
    candidates: Vec<(Vec<usize>, NativeKeyGlyph, f64)>,
) -> Vec<(NativeKeyGlyph, f64)> {
    let mut candidates = candidates;
    candidates.sort_by(|one, two| two.2.total_cmp(&one.2));
    let mut kept: Vec<(Vec<usize>, NativeKeyGlyph, f64)> = Vec::new();
    for (parts, glyph, grade) in candidates {
        if kept
            .iter()
            .any(|(taken, _, _)| taken.iter().any(|part| parts.contains(part)))
        {
            continue;
        }
        kept.push((parts, glyph, grade));
    }
    kept.into_iter()
        .map(|(_, glyph, grade)| (glyph, grade))
        .collect()
}

/// Java `AlterInter.computePitch`, whose flat branch is nothing like its sharp branch.
///
/// A sharp is simply the pitch of its **mass centroid**. A flat is the average of two heuristics:
///
/// 1. the mass centroid's pitch plus `flatMassPitchOffset` (0.65);
/// 2. the **area centre's** pitch plus the font-derived `getAreaPitchOffset(FLAT)`.
///
/// Java's comment calls both "heuristic", and they exist because a flat's ink sits well below the
/// line it belongs to — its bowl hangs under the focus point while a sharp straddles it.
///
/// Omitting this is why every flat key on the corpus was rejected while sharps passed: a single
/// alteration is allowed only 0.5 pitch of error, and the uncorrected flat pitch misses by about
/// one. The classifier had already identified the flat at grade 0.97; it was the pitch check that
/// threw it away.
///
/// Note the two points are read differently on purpose. `glyph.getCentroid()` returns a **rounded**
/// `Point`, so the mass pitch is taken at integer coordinates, while `getCenter2D()` is exact.
fn measured_alter_pitch<Lines: StaffPitchGeometry>(
    lines: &Lines,
    staff_id: usize,
    context: NativeKeyStaffContext,
    shape: NeutralKeyAlterShape,
    glyph: &NativeKeyGlyph,
    flat_area_offset: f64,
) -> f64 {
    let centroid_x = glyph.centroid_x.round_ties_even();
    let centroid_y = glyph.centroid_y.round_ties_even();
    let mass_pitch = pitch_position_of(
        lines.line_span_at(staff_id, centroid_x),
        context,
        centroid_y,
    );
    if shape != NeutralKeyAlterShape::Flat {
        return mass_pitch;
    }
    let center_x = f64::from(glyph.bounds.x) + f64::from(glyph.bounds.width) / 2.0;
    let center_y = f64::from(glyph.bounds.y) + f64::from(glyph.bounds.height) / 2.0;
    let geo_pitch = pitch_position_of(lines.line_span_at(staff_id, center_x), context, center_y)
        + flat_area_offset;
    0.5 * ((mass_pitch + FLAT_MASS_PITCH_OFFSET) + geo_pitch)
}

/// Java `Staff.pitchPositionOf`, for a point inside the staff.
///
/// ```text
/// ((lines - 1) * (2y - bottom - top)) / (bottom - top)
/// ```
///
/// Falls back to the old interline approximation only when the splines cannot answer at this
/// abscissa, which for a key alteration means the glyph sat outside its own staff's horizontal
/// extent. That is degenerate rather than routine, and the fallback keeps it from becoming a hard
/// failure.
fn pitch_position_of(span: Option<(f64, f64)>, context: NativeKeyStaffContext, y: f64) -> f64 {
    match span {
        Some((top, bottom)) if bottom > top => {
            (context.line_count.saturating_sub(1) as f64) * ((2.0 * y) - bottom - top)
                / (bottom - top)
        }
        _ => (2.0 * (y - context.staff_mid_y)) / f64::from(context.classifier_interline),
    }
}

pub struct NativeKeyProposalRecognizer<Classifier, Lines> {
    classifier: Classifier,
    lines: Lines,
    sources: BTreeMap<usize, RunTable>,
    contexts: BTreeMap<usize, NativeKeyStaffContext>,
    parameters: BTreeMap<usize, NativeKeyParameters>,
    next_glyph_id: usize,
    next_inter_id: usize,
    mutations: Vec<NativeKeyMutation>,
    projections: BTreeMap<usize, Vec<usize>>,
    clef_supports: BTreeMap<usize, Vec<KeyClefSupport>>,
}

impl<Classifier: KeyShapeClassifier, Lines: StaffPitchGeometry>
    NativeKeyProposalRecognizer<Classifier, Lines>
{
    #[must_use]
    pub fn new(
        classifier: Classifier,
        lines: Lines,
        sources: BTreeMap<usize, RunTable>,
        contexts: BTreeMap<usize, NativeKeyStaffContext>,
        parameters: BTreeMap<usize, NativeKeyParameters>,
        next_glyph_id: usize,
        next_inter_id: usize,
    ) -> Self {
        Self {
            classifier,
            lines,
            sources,
            contexts,
            parameters,
            next_glyph_id,
            next_inter_id,
            mutations: Vec::new(),
            projections: BTreeMap::new(),
            clef_supports: BTreeMap::new(),
        }
    }

    /// Supplies the per-staff clef candidates Java reads via `staff.getCompetingClefs`.
    ///
    /// Without them the third extraction pass (`checkWithClefs` + `fillMissingAlters`) is
    /// skipped, exactly as Java skips it when a staff has no competing clef.
    #[must_use]
    pub fn with_clef_supports(mut self, supports: BTreeMap<usize, Vec<KeyClefSupport>>) -> Self {
        self.clef_supports = supports;
        self
    }

    #[must_use]
    pub fn mutations(&self) -> &[NativeKeyMutation] {
        &self.mutations
    }
    #[must_use]
    pub const fn classifier(&self) -> &Classifier {
        &self.classifier
    }
    #[must_use]
    pub fn projection(&self, staff_id: usize) -> Option<&[usize]> {
        self.projections.get(&staff_id).map(Vec::as_slice)
    }

    fn glyph_id(&mut self) -> Result<usize, NativeKeyError<Classifier::Error>> {
        self.next_glyph_id = self
            .next_glyph_id
            .checked_add(1)
            .ok_or(NativeKeyError::GlyphIdExhausted)?;
        Ok(self.next_glyph_id)
    }
    fn inter_id(&mut self) -> Result<usize, NativeKeyError<Classifier::Error>> {
        self.next_inter_id = self
            .next_inter_id
            .checked_add(1)
            .ok_or(NativeKeyError::InterIdExhausted)?;
        Ok(self.next_inter_id)
    }

    /// Java `ShapeBuilder.refineAreaStart`: advance the range start to the first non-space
    /// column of a *staff-band* projection (one interline above the first line, down to the
    /// last line) between the current start and the first peak.
    fn refine_area_start(
        &mut self,
        system_id: usize,
        staff_id: usize,
        context: NativeKeyStaffContext,
        pipeline: &KeyPipelineParameters,
        range: &mut StaffHeaderRange,
        peaks: &[KeyPeak],
    ) -> Result<(), NativeKeyError<Classifier::Error>> {
        if peaks.is_empty() {
            return Ok(());
        }
        let x_min = range
            .start()
            .map_err(|_| NativeKeyError::InvalidBrowseRange { staff_id })?;
        let x_max = peaks[0].min - 1;
        let Some((top, bottom)) = self.lines.line_span_at(staff_id, f64::from(x_min)) else {
            return Ok(());
        };
        let y_min = (top.round_ties_even() as i32) - context.staff_interline;
        let y_max = bottom.round_ties_even() as i32;
        let source = self
            .sources
            .get(&system_id)
            .ok_or(NativeKeyError::MissingSource(system_id))?;
        let mut start = x_max + 1;
        for x in x_min..=x_max {
            if column_cumul(source, x, y_min, y_max - y_min + 1) > pipeline.max_space_cumul {
                start = x;
                break;
            }
        }
        range.set_start(start);
        Ok(())
    }

    /// Java `Staff.pitchToOrdinate`, the inverse of `pitchPositionOf` for a point inside the
    /// staff, `rint`ed as the trailing-space check consumes it.
    fn pitch_to_ordinate(
        &self,
        staff_id: usize,
        context: NativeKeyStaffContext,
        x: f64,
        pitch: f64,
    ) -> Option<i32> {
        let (top, bottom) = self.lines.line_span_at(staff_id, x)?;
        let lines = context.line_count.saturating_sub(1) as f64;
        Some(((pitch * (bottom - top) / lines + top + bottom) / 2.0).round_ties_even() as i32)
    }

    /// Java `KeyExtractor.purgeParts` plus part registration: drop under-weight components and
    /// the quirky `bounds.x == xMax` case, cap at `maxPartCount` by descending weight, then
    /// register ids in left order.
    fn register_parts(
        &mut self,
        staff_id: usize,
        components: Vec<GlyphComponent>,
        minimum_weight: usize,
        x_max: i32,
    ) -> Result<Vec<NativeKeyPart>, NativeKeyError<Classifier::Error>> {
        let mut components: Vec<GlyphComponent> = components
            .into_iter()
            .filter(|component| component.weight >= minimum_weight && component.left != x_max)
            .collect();
        if components.len() > crate::key_parameters::max_part_count() {
            components.sort_by_key(|component| std::cmp::Reverse(component.weight));
            components.truncate(crate::key_parameters::max_part_count());
        }
        components.sort_by_key(|component| component.left);
        let mut parts = Vec::with_capacity(components.len());
        for component in components {
            let id = self.glyph_id()?;
            self.mutations.push(NativeKeyMutation::ComponentRegistered {
                staff_id,
                glyph_id: id,
            });
            parts.push(NativeKeyPart { id, component });
        }
        Ok(parts)
    }

    /// Java `ShapeBuilder.checkWithClefs` followed by `fillMissingAlters`, phase 3 of the key
    /// hunt.
    ///
    /// Selects the best *compatible* clef by contextual grade — a single alteration whose pitch
    /// misses its expected position by more than the delta budget invalidates the whole clef, and
    /// grades are read **intrinsic-scaled**, as `KeyAlterInter` stores them. If the best clef
    /// overall is not the compatible one, the key is destroyed (`false`). Otherwise every slice
    /// that is still empty, or whose pitched grade fell under `keyAlterMinGrade1`, is hunted once
    /// more — this time in a **pitch window**: the slice rectangle is re-centred on the
    /// alteration's theoretical ordinate for that clef, `stdGlyphHeight` tall, and the extraction
    /// reruns with the phase-2 grade floor and neighbours cropped away.
    #[expect(clippy::too_many_arguments, reason = "mirrors the Java method pair")]
    fn check_with_clefs_and_fill(
        &mut self,
        input: &KeyRecognitionInput,
        context: NativeKeyStaffContext,
        parameters: &NativeKeyParameters,
        pipeline: &KeyPipelineParameters,
        shape: NeutralKeyAlterShape,
        clefs: &[KeyClefSupport],
        peaks: &[KeyPeak],
        slices: &mut [SliceState],
        flat_area_offset: f64,
    ) -> Result<bool, NativeKeyError<Classifier::Error>> {
        // Java `ClefKeyRelation().getSourceRatio()` and `KeyAltersRelation().getSourceRatio()`.
        const CLEF_RATIO: f64 = 5.0;
        const ALTERS_RATIO: f64 = 5.0;
        // Java `Grades.intrinsicRatio` and the two alter grade floors.
        const INTRINSIC: f64 = 0.8;
        const MIN_GRADE_1: f64 = 0.1;

        let pitched_for = |kind: NeutralClefKind| -> Option<(Vec<f64>, usize)> {
            let expected = key_pitches(kind, shape)?;
            let n = slices.len();
            let max_delta = if n >= 4 {
                2.0
            } else {
                0.5 + (2.0 - 0.5) * (n - 1) as f64 / 3.0
            };
            let mut grades = vec![0.0; n];
            let mut count = 0;
            for (index, slice) in slices.iter().enumerate() {
                let (Some(_glyph), Some(grade)) = (&slice.glyph, slice.grade) else {
                    continue;
                };
                count += 1;
                let delta = (slice
                    .pitch
                    .expect("a graded slice carries its measured pitch")
                    - f64::from(expected[index]))
                .abs();
                if delta > max_delta {
                    return None;
                }
                grades[index] = INTRINSIC * grade * (1.0 - delta / max_delta);
            }
            (count > 0).then_some((grades, count))
        };

        let mut best_compatible: Option<(usize, Vec<f64>)> = None;
        let mut best_compatible_ctx = 0.0;
        for (clef_index, clef) in clefs.iter().enumerate() {
            if clef.kind == NeutralClefKind::Percussion {
                continue;
            }
            let Some((grades, count)) = pitched_for(clef.kind) else {
                continue;
            };
            // Java `computeKeyGrade`: mean over present alters of each grade's contextual value
            // against the sum of the *other* slices' contributions.
            let contribs: Vec<f64> = grades
                .iter()
                .map(|&grade| contribution_of(grade, ALTERS_RATIO))
                .collect();
            let mut key_grade = 0.0;
            for (index, &grade) in grades.iter().enumerate() {
                let contribution: f64 = contribs
                    .iter()
                    .enumerate()
                    .filter(|&(other, _)| other != index)
                    .map(|(_, &value)| value)
                    .sum();
                key_grade += contextual(grade, contribution);
            }
            key_grade /= count as f64;
            let clef_ctx = contextual(clef.grade, contribution_of(key_grade, CLEF_RATIO));
            if clef_ctx > best_compatible_ctx {
                best_compatible_ctx = clef_ctx;
                best_compatible = Some((clef_index, grades));
            }
        }

        let Some((compatible_index, pitched)) = best_compatible else {
            // No compatible clef at all: Java stuffs every slice and reports failure.
            return Ok(false);
        };
        let mut best_index = compatible_index;
        let mut best_grade = -1.0;
        for (clef_index, clef) in clefs.iter().enumerate() {
            let grade = if clef_index == compatible_index {
                best_compatible_ctx
            } else {
                clef.grade
            };
            if grade > best_grade {
                best_grade = grade;
                best_index = clef_index;
            }
        }
        if best_index != compatible_index {
            return Ok(false);
        }

        // ---- fillMissingAlters ----
        let kind = clefs[compatible_index].kind;
        let expected = key_pitches(kind, shape).expect("compatible clef has pitches");
        for index in 0..slices.len() {
            let needs = slices[index].glyph.is_none() || pitched[index] < MIN_GRADE_1;
            if !needs {
                continue;
            }
            // Java `KeySlice.setPitchRect`: centre the hunt on the theoretical ordinate. Two
            // integer quirks are load-bearing. `typicalHeight / 2` is integer division. And the
            // pitch itself is `int pitch = clefPitches[i]; pitch -= getAreaPitchOffset(shape);`
            // -- a compound assignment on an `int`, which Java silently narrows back: the
            // fractional area offset is applied and then *truncated toward zero*, so a bass-clef
            // third flat hunts at pitch (int)(3 - 1.0559) = 1, not 1.944. Reproducing the
            // truncation moved the window 7 px and closed the last corpus staff; "fixing" it
            // would diverge from Java on every flat key.
            let offset = match shape {
                NeutralKeyAlterShape::Flat => flat_area_offset,
                NeutralKeyAlterShape::Sharp => 0.0,
            };
            let pitch = f64::from((f64::from(expected[index]) - offset) as i32);
            let Some(y_center) = self.pitch_to_ordinate_double(
                input.staff_id,
                context,
                f64::from(slices[index].start),
                pitch,
            ) else {
                continue;
            };
            let y = (y_center - f64::from(pipeline.std_glyph_height / 2)).round_ties_even() as i32;
            let slice_rect = HeaderBounds {
                x: slices[index].start,
                y,
                width: slices[index].stop - slices[index].start + 1,
                height: pipeline.std_glyph_height,
            };
            let neighbours: Vec<NativeKeyGlyph> = [index.checked_sub(1), Some(index + 1)]
                .into_iter()
                .flatten()
                .filter_map(|other| slices.get(other).and_then(|slice| slice.glyph.clone()))
                .collect();
            let parts = {
                let source = self
                    .sources
                    .get(&input.system_id)
                    .ok_or(NativeKeyError::MissingSource(input.system_id))?;
                let crop = slice_crop(source, slice_rect, &neighbours)
                    .map_err(NativeKeyError::RunTable)?;
                self.register_parts(
                    input.staff_id,
                    build_glyph_components(&crop, slice_rect.x, slice_rect.y),
                    parameters.minimum_component_weight,
                    slice_rect.right(),
                )?
            };
            let groups = enumerate_key_subsets(&parts, *parameters);
            for group in &groups {
                let glyph = self.compound(input.staff_id, &parts, group)?;
                if !accepts_glyph(&glyph, parameters)
                    || !embraces_slice_peaks(
                        slices[index].start,
                        slices[index].stop,
                        glyph.bounds,
                        peaks,
                    )
                {
                    continue;
                }
                self.mutations.push(NativeKeyMutation::GlyphEvaluated {
                    staff_id: input.staff_id,
                    glyph_id: glyph.id,
                    shape,
                });
                let evaluations = self
                    .classifier
                    .evaluate(
                        &glyph,
                        context.classifier_interline,
                        parameters.maximum_rank,
                        PHASE_TWO_MINIMUM_GRADE,
                    )
                    .map_err(|source| NativeKeyError::Classifier {
                        staff_id: input.staff_id,
                        glyph_id: glyph.id,
                        source,
                    })?;
                if let Some(evaluation) = evaluations
                    .into_iter()
                    .take(parameters.maximum_rank)
                    .find(|evaluation| {
                        evaluation.shape == shape && evaluation.grade >= PHASE_TWO_MINIMUM_GRADE
                    })
                {
                    let pitch = measured_alter_pitch(
                        &self.lines,
                        input.staff_id,
                        context,
                        shape,
                        &glyph,
                        flat_area_offset,
                    );
                    let slice = &mut slices[index];
                    if slice.grade.is_none_or(|current| current < evaluation.grade) {
                        slice.grade = Some(evaluation.grade);
                        slice.glyph = Some(glyph);
                        slice.pitch = Some(pitch);
                    }
                }
            }
        }
        Ok(true)
    }

    /// `pitch_to_ordinate` without the final `rint`, for callers that subtract first.
    fn pitch_to_ordinate_double(
        &self,
        staff_id: usize,
        context: NativeKeyStaffContext,
        x: f64,
        pitch: f64,
    ) -> Option<f64> {
        let (top, bottom) = self.lines.line_span_at(staff_id, x)?;
        let lines = context.line_count.saturating_sub(1) as f64;
        Some((pitch * (bottom - top) / lines + top + bottom) / 2.0)
    }
}

impl<Classifier: KeyShapeClassifier, Lines: StaffPitchGeometry> VisualKeyProposalRecognizer
    for NativeKeyProposalRecognizer<Classifier, Lines>
{
    type Error = NativeKeyError<Classifier::Error>;

    fn classify_key_shapes(
        &mut self,
        input: KeyRecognitionInput,
    ) -> Result<Vec<KeyShapeClassifierProposal>, Self::Error> {
        let context = *self
            .contexts
            .get(&input.staff_id)
            .ok_or(NativeKeyError::MissingContext(input.staff_id))?;
        let parameters = *self
            .parameters
            .get(&input.staff_id)
            .ok_or(NativeKeyError::MissingParameters(input.staff_id))?;
        let pipeline = parameters.pipeline;
        let browse_start = input.browse_start.max(input.measure_start);
        let browse_stop = context
            .browse_stop
            .min(input.measure_start + input.projection_width - 1);
        if browse_start > browse_stop || context.envelope_top > context.envelope_bottom {
            return Err(NativeKeyError::InvalidBrowseRange {
                staff_id: input.staff_id,
            });
        }
        let envelope_top = context.envelope_top;
        let envelope_height = context.envelope_bottom - context.envelope_top + 1;

        // Java `KeyExtractor.getProjection`: black count per column over the browse envelope,
        // from `min(measureStart, browseRect.x)` so `refineAreaStart` can look left of the
        // browse start.
        let projection = {
            let source = self
                .sources
                .get(&input.system_id)
                .ok_or(NativeKeyError::MissingSource(input.system_id))?;
            let x_min = input.measure_start.min(browse_start);
            let mut function = IntegerFunction::new(x_min, browse_stop);
            for x in x_min..=browse_stop {
                function.set_value(x, column_cumul(source, x, envelope_top, envelope_height));
            }
            function
        };
        self.projections.insert(
            input.staff_id,
            (projection.x_min()..=projection.x_max())
                .map(|x| usize::try_from(projection.value(x)).unwrap_or(0))
                .collect(),
        );

        // Java `KeyBuilder.retrieveHiLoPeaks`, shared by both shape builders.
        let hilos = {
            let mut finder =
                HiLoPeakFinder::with_domain("Key", &projection, browse_start, browse_stop);
            finder.set_quorum(Quorum::new(pipeline.min_peak_cumul));
            let mut hilos = finder
                .find_peaks(
                    pipeline.max_space_cumul + 1,
                    pipeline.min_peak_derivative,
                    pipeline.min_gain_ratio,
                )
                .to_vec();
            hilos.sort_by_key(|hilo| hilo.main);
            hilos
        };

        // Font-derived and constant per family, so it is fetched once rather than per glyph.
        let flat_area_offset = area_pitch_offset(MusicFamily::Bravura, "FLAT").unwrap_or(0.0);
        let mut proposals = Vec::new();
        for shape in [NeutralKeyAlterShape::Flat, NeutralKeyAlterShape::Sharp] {
            let kind = match shape {
                NeutralKeyAlterShape::Flat => KeyShapeKind::Flat,
                NeutralKeyAlterShape::Sharp => KeyShapeKind::Sharp,
            };
            // Each Java `ShapeBuilder` owns a fresh range over the shared browse window.
            let mut range = StaffHeaderRange::default();
            range.browse_start = browse_start;
            range.browse_stop = browse_stop;
            let mut peaks: Vec<KeyPeak> = Vec::new();
            {
                let source = self
                    .sources
                    .get(&input.system_id)
                    .ok_or(NativeKeyError::MissingSource(input.system_id))?;
                let mut has_stem = |peak: &KeyPeak| {
                    // Java `isStemLike`: a peak of width <= 2 grows one pixel on each side.
                    let (x, width) = if peak.width() <= 2 {
                        (peak.min - 1, peak.width() + 2)
                    } else {
                        (peak.min, peak.width())
                    };
                    has_stem_under(
                        source,
                        x,
                        width,
                        envelope_top,
                        envelope_height,
                        pipeline.core_stem_length,
                        pipeline.min_black_ratio,
                    )
                };
                browse_area(
                    &hilos,
                    &projection,
                    &pipeline,
                    &mut range,
                    &mut peaks,
                    &mut has_stem,
                );
            }
            self.refine_area_start(
                input.system_id,
                input.staff_id,
                context,
                &pipeline,
                &mut range,
                &peaks,
            )?;
            merge_peaks(&mut peaks, &projection, &pipeline);
            purge_light_peaks(&mut peaks, kind, &pipeline);
            let signature = infer_signature(&mut peaks, &mut range, kind, &pipeline);
            let signature = refine_signature(signature, &mut peaks, &mut range, &pipeline);
            let starts = compute_starts(signature, &peaks, &projection, &pipeline, &mut range);
            if starts.is_empty() {
                continue;
            }
            let mut slices: Vec<SliceState> = allocate_slices(&starts, &range)
                .into_iter()
                .map(|(start, stop)| SliceState {
                    start,
                    stop,
                    glyph: None,
                    grade: None,
                    pitch: None,
                })
                .collect();

            // ---- Phase 1: Java `retrieveCandidates` over the whole key area ----
            let area_start = range
                .start()
                .map_err(|_| NativeKeyError::InvalidBrowseRange {
                    staff_id: input.staff_id,
                })?;
            let area_stop = range.stop();
            if area_start > area_stop {
                continue;
            }
            let area_rect = HeaderBounds {
                x: area_start,
                y: envelope_top,
                width: area_stop - area_start + 1,
                height: envelope_height,
            };
            let parts = {
                let source = self
                    .sources
                    .get(&input.system_id)
                    .ok_or(NativeKeyError::MissingSource(input.system_id))?;
                let crop = crop_key(source, area_rect).map_err(NativeKeyError::RunTable)?;
                self.register_parts(
                    input.staff_id,
                    build_glyph_components(&crop, area_rect.x, area_rect.y),
                    parameters.minimum_component_weight,
                    area_stop,
                )?
            };
            let groups = enumerate_key_subsets(&parts, parameters);
            let mut candidates: Vec<(Vec<usize>, NativeKeyGlyph, f64)> = Vec::new();
            for group in &groups {
                let glyph = self.compound(input.staff_id, &parts, group)?;
                if !accepts_glyph(&glyph, &parameters) {
                    continue;
                }
                // Java `MultipleAdapter.evaluateGlyph` routes through the slice under the glyph's
                // rounded centroid, and `evaluateSliceGlyph` requires the glyph to embrace every
                // peak centre falling inside that slice.
                let centroid_x = glyph.centroid_x.round_ties_even() as i32;
                if let Some(slice) = slices
                    .iter()
                    .find(|slice| slice.start <= centroid_x && centroid_x <= slice.stop)
                {
                    if !embraces_slice_peaks(slice.start, slice.stop, glyph.bounds, &peaks) {
                        continue;
                    }
                }
                self.mutations.push(NativeKeyMutation::GlyphEvaluated {
                    staff_id: input.staff_id,
                    glyph_id: glyph.id,
                    shape,
                });
                let evaluations = self
                    .classifier
                    .evaluate(
                        &glyph,
                        context.classifier_interline,
                        parameters.maximum_rank,
                        PHASE_ONE_MINIMUM_GRADE,
                    )
                    .map_err(|source| NativeKeyError::Classifier {
                        staff_id: input.staff_id,
                        glyph_id: glyph.id,
                        source,
                    })?;
                if let Some(evaluation) = evaluations
                    .into_iter()
                    .take(parameters.maximum_rank)
                    .find(|evaluation| {
                        evaluation.shape == shape && evaluation.grade >= PHASE_ONE_MINIMUM_GRADE
                    })
                {
                    candidates.push((group.clone(), glyph, evaluation.grade));
                } else {
                    self.mutations.push(NativeKeyMutation::ClassifierRejected {
                        staff_id: input.staff_id,
                        glyph_id: glyph.id,
                        shape,
                    });
                }
            }
            // Java `purgeCandidates` (no part shared), then best-per-slice assignment in
            // descending grade order.
            let candidates = purge_key_candidates(candidates);
            for (glyph, grade) in candidates {
                let centroid_x = glyph.centroid_x.round_ties_even() as i32;
                let pitch = measured_alter_pitch(
                    &self.lines,
                    input.staff_id,
                    context,
                    shape,
                    &glyph,
                    flat_area_offset,
                );
                if let Some(slice) = slices
                    .iter_mut()
                    .find(|slice| slice.start <= centroid_x && centroid_x <= slice.stop)
                {
                    if slice.grade.is_none_or(|current| current < grade) {
                        slice.grade = Some(grade);
                        slice.glyph = Some(glyph);
                        slice.pitch = Some(pitch);
                    }
                }
            }

            // ---- Phase 2: Java `extractEmptySlices`, with neighbours cropped away ----
            for index in 0..slices.len() {
                if slices[index].glyph.is_some() {
                    continue;
                }
                let slice_rect = HeaderBounds {
                    x: slices[index].start,
                    y: envelope_top,
                    width: slices[index].stop - slices[index].start + 1,
                    height: envelope_height,
                };
                let neighbours: Vec<NativeKeyGlyph> = [index.checked_sub(1), Some(index + 1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|other| slices.get(other).and_then(|slice| slice.glyph.clone()))
                    .collect();
                let parts = {
                    let source = self
                        .sources
                        .get(&input.system_id)
                        .ok_or(NativeKeyError::MissingSource(input.system_id))?;
                    let crop = slice_crop(source, slice_rect, &neighbours)
                        .map_err(NativeKeyError::RunTable)?;
                    self.register_parts(
                        input.staff_id,
                        build_glyph_components(&crop, slice_rect.x, slice_rect.y),
                        parameters.minimum_component_weight,
                        slice_rect.right(),
                    )?
                };
                let groups = enumerate_key_subsets(&parts, parameters);
                for group in &groups {
                    let glyph = self.compound(input.staff_id, &parts, group)?;
                    if !accepts_glyph(&glyph, &parameters)
                        || !embraces_slice_peaks(
                            slices[index].start,
                            slices[index].stop,
                            glyph.bounds,
                            &peaks,
                        )
                    {
                        continue;
                    }
                    self.mutations.push(NativeKeyMutation::GlyphEvaluated {
                        staff_id: input.staff_id,
                        glyph_id: glyph.id,
                        shape,
                    });
                    let evaluations = self
                        .classifier
                        .evaluate(
                            &glyph,
                            context.classifier_interline,
                            parameters.maximum_rank,
                            PHASE_TWO_MINIMUM_GRADE,
                        )
                        .map_err(|source| NativeKeyError::Classifier {
                            staff_id: input.staff_id,
                            glyph_id: glyph.id,
                            source,
                        })?;
                    if let Some(evaluation) = evaluations
                        .into_iter()
                        .take(parameters.maximum_rank)
                        .find(|evaluation| {
                            evaluation.shape == shape && evaluation.grade >= PHASE_TWO_MINIMUM_GRADE
                        })
                    {
                        let pitch = measured_alter_pitch(
                            &self.lines,
                            input.staff_id,
                            context,
                            shape,
                            &glyph,
                            flat_area_offset,
                        );
                        let slice = &mut slices[index];
                        if slice.grade.is_none_or(|current| current < evaluation.grade) {
                            slice.grade = Some(evaluation.grade);
                            slice.glyph = Some(glyph);
                            slice.pitch = Some(pitch);
                        }
                    }
                }
            }

            // ---- Phase 3: Java `checkWithClefs` + `fillMissingAlters` ----
            //
            // Java runs this only when the staff has competing clefs; with none, the key is kept
            // unchecked. A clef-incompatible key is destroyed outright (`stuffSlicesFrom(0)`).
            let clefs = self
                .clef_supports
                .get(&input.staff_id)
                .cloned()
                .unwrap_or_default();
            if !clefs.is_empty()
                && !self.check_with_clefs_and_fill(
                    &input,
                    context,
                    &parameters,
                    &pipeline,
                    shape,
                    &clefs,
                    &peaks,
                    &mut slices,
                    flat_area_offset,
                )?
            {
                continue;
            }

            // ---- Alters in slice order ----
            let mut alters = Vec::new();
            for slice in &slices {
                let (Some(glyph), Some(grade)) = (&slice.glyph, slice.grade) else {
                    continue;
                };
                let id = self.inter_id()?;
                alters.push(KeyAlterClassifierProposal {
                    id,
                    start: slice.start,
                    width: slice.stop - slice.start + 1,
                    bounds: glyph.bounds,
                    classifier_grade: grade,
                    measured_pitch: slice.pitch.expect("assigned with its glyph"),
                });
            }
            if alters.is_empty() {
                continue;
            }

            // Java: a one-item candidate must show trailing space right after its alter.
            let last_valid = slices.iter().rposition(|slice| slice.glyph.is_some());
            if last_valid == Some(0) {
                let slice = &slices[0];
                let glyph = slice.glyph.as_ref().expect("checked");
                let pitch = alters[0].measured_pitch;
                let x = glyph.bounds.x + glyph.bounds.width;
                let Some(y) = self.pitch_to_ordinate(input.staff_id, context, f64::from(x), pitch)
                else {
                    continue;
                };
                let source = self
                    .sources
                    .get(&input.system_id)
                    .ok_or(NativeKeyError::MissingSource(input.system_id))?;
                let rect = HeaderBounds {
                    x,
                    y: y - context.staff_interline / 2,
                    width: context.classifier_interline,
                    height: context.staff_interline,
                };
                if !is_rather_empty(
                    source,
                    rect,
                    pipeline.max_trailing_cumul,
                    pipeline.min_trailing_space,
                ) {
                    continue;
                }
            }

            let id = self.inter_id()?;
            proposals.push(KeyShapeClassifierProposal {
                id,
                shape,
                range: range.clone(),
                alters,
            });
        }
        Ok(proposals)
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

impl<Classifier: KeyShapeClassifier, Lines: StaffPitchGeometry>
    NativeKeyProposalRecognizer<Classifier, Lines>
{
    fn compound(
        &mut self,
        staff_id: usize,
        parts: &[NativeKeyPart],
        group: &[usize],
    ) -> Result<NativeKeyGlyph, NativeKeyError<Classifier::Error>> {
        let id = if group.len() == 1 {
            parts[group[0]].id
        } else {
            let id = self.glyph_id()?;
            self.mutations.push(NativeKeyMutation::CompoundRegistered {
                staff_id,
                glyph_id: id,
            });
            id
        };
        native_key_glyph(id, parts, group).map_err(NativeKeyError::RunTable)
    }
}

/// Java `Glyphs.buildLinks` + `ConnectivityInspector` + `GlyphCluster.decompose`, for key parts.
///
/// This replaces a left-to-right grouping that merged every part within `maxPartGap` into one
/// compound. Java does not group; it *enumerates subsets* of each connected set, so a run of six
/// sharps yields each sharp alone, each adjacent pair, and so on -- and the classifier decides.
/// Grouping instead collapsed a whole key signature into a single glyph whose width then exceeded
/// `maxGlyphWidth`, so every key on the corpus was rejected outright.
///
/// The order matters and is Java's: connected sets in left-abscissa order, seeds within a set by
/// **descending weight**, then depth-first growth through graph neighbours. Downstream keeps only
/// the first `maximum_alters` results, so a different order would keep different alterations.
fn enumerate_key_subsets(
    parts: &[NativeKeyPart],
    parameters: NativeKeyParameters,
) -> Vec<Vec<usize>> {
    let pixels: Vec<Vec<(i32, i32)>> = parts
        .iter()
        .map(|part| component_pixels(&part.component))
        .collect();
    let mut order = (0..parts.len()).collect::<Vec<_>>();
    order.sort_by_key(|&index| parts[index].component.left);

    let mut graph = vec![Vec::new(); parts.len()];
    for (position, &one) in order.iter().enumerate() {
        for &two in &order[position + 1..] {
            if chamfer_gap(&pixels[one], &pixels[two]) <= parameters.maximum_component_gap {
                graph[one].push(two);
                graph[two].push(one);
            }
        }
    }

    let mut seen = vec![false; parts.len()];
    let mut subsets = Vec::new();
    for &root in &order {
        if seen[root] {
            continue;
        }
        let mut stack = vec![root];
        let mut set = Vec::new();
        seen[root] = true;
        while let Some(current) = stack.pop() {
            set.push(current);
            for &neighbor in graph[current].iter().rev() {
                if !seen[neighbor] {
                    seen[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        let mut seeds = set;
        seeds.sort_by(|&one, &two| {
            parts[two]
                .component
                .weight
                .cmp(&parts[one].component.weight)
        });
        let mut considered = Vec::new();
        for seed in seeds {
            push_unique(&mut considered, seed);
            grow_key_subset(
                parts,
                &graph,
                parameters,
                vec![seed],
                considered.clone(),
                &mut subsets,
            );
        }
    }
    subsets
}

/// The union box of a subset, in the same `(width, height)` terms Java's `isTooLarge` reads.
fn key_subset_extent(parts: &[NativeKeyPart], subset: &[usize]) -> (f64, f64) {
    let mut left = i32::MAX;
    let mut top = i32::MAX;
    let mut right = i32::MIN;
    let mut bottom = i32::MIN;
    for &index in subset {
        let component = &parts[index].component;
        left = left.min(component.left);
        top = top.min(component.top);
        right = right.max(component.left + i32::try_from(component.width).unwrap_or(i32::MAX));
        bottom = bottom.max(component.top + i32::try_from(component.height).unwrap_or(i32::MAX));
    }
    (f64::from(right - left), f64::from(bottom - top))
}

/// Depth-first subset growth, Java `GlyphCluster.decompose`'s recursion.
///
/// Pruning uses **both** dimensions, because the key stage's `isTooLarge` tests width *and*
/// height. The clef stage prunes on height alone, because its adapter only tests height -- the
/// two adapters genuinely differ and the difference is not an oversight to harmonise.
fn grow_key_subset(
    parts: &[NativeKeyPart],
    graph: &[Vec<usize>],
    parameters: NativeKeyParameters,
    subset: Vec<usize>,
    seen: Vec<usize>,
    subsets: &mut Vec<Vec<usize>>,
) {
    let (width, height) = key_subset_extent(parts, &subset);
    if width > parameters.maximum_alter_width || height > parameters.maximum_alter_height {
        return;
    }
    subsets.push(subset.clone());

    let mut outliers = Vec::new();
    for &part in &subset {
        for &neighbor in &graph[part] {
            if !subset.contains(&neighbor) {
                push_unique(&mut outliers, neighbor);
            }
        }
    }
    outliers.retain(|part| !seen.contains(part));
    let mut newly_seen = seen;
    for outlier in outliers {
        push_unique(&mut newly_seen, outlier);
        let mut larger = subset.clone();
        larger.push(outlier);
        let (width, height) = key_subset_extent(parts, &larger);
        if width <= parameters.maximum_alter_width && height <= parameters.maximum_alter_height {
            grow_key_subset(
                parts,
                graph,
                parameters,
                larger,
                newly_seen.clone(),
                subsets,
            );
        }
    }
}

fn crop_key(source: &RunTable, rect: HeaderBounds) -> Result<RunTable, RunTableError> {
    let x = usize::try_from(rect.x).map_err(|_| RunTableError::OutOfBounds)?;
    let y = usize::try_from(rect.y).map_err(|_| RunTableError::OutOfBounds)?;
    let width = usize::try_from(rect.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(rect.height).map_err(|_| RunTableError::InvalidDimensions)?;
    if x + width > source.width() || y + height > source.height() {
        return Err(RunTableError::OutOfBounds);
    }
    let mut pixels = vec![BACKGROUND; width * height];
    for local_y in 0..height {
        for local_x in 0..width {
            pixels[local_y * width + local_x] = source.get(x + local_x, y + local_y);
        }
    }
    RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)
}

fn native_key_glyph(
    id: usize,
    parts: &[NativeKeyPart],
    group: &[usize],
) -> Result<NativeKeyGlyph, RunTableError> {
    let left = group
        .iter()
        .map(|index| parts[*index].component.left)
        .min()
        .unwrap();
    let top = group
        .iter()
        .map(|index| parts[*index].component.top)
        .min()
        .unwrap();
    let right = group
        .iter()
        .map(|index| {
            parts[*index].component.left + i32::try_from(parts[*index].component.width).unwrap() - 1
        })
        .max()
        .unwrap();
    let bottom = group
        .iter()
        .map(|index| {
            parts[*index].component.top + i32::try_from(parts[*index].component.height).unwrap() - 1
        })
        .max()
        .unwrap();
    let bounds = HeaderBounds {
        x: left,
        y: top,
        width: right - left + 1,
        height: bottom - top + 1,
    };
    let width = usize::try_from(bounds.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(bounds.height).map_err(|_| RunTableError::InvalidDimensions)?;
    let mut pixels = vec![BACKGROUND; width * height];
    let mut weight = 0usize;
    let mut sx = 0.0;
    let mut sy = 0.0;
    for index in group {
        let component = &parts[*index].component;
        weight += component.weight;
        sx += component.centroid_x * component.weight as f64;
        sy += component.centroid_y * component.weight as f64;
        let raster = component_key_pixels(component);
        for (x, y) in raster {
            pixels
                [usize::try_from(y - top).unwrap() * width + usize::try_from(x - left).unwrap()] =
                FOREGROUND;
        }
    }
    Ok(NativeKeyGlyph {
        id,
        part_ids: group.iter().map(|index| parts[*index].id).collect(),
        bounds,
        weight,
        centroid_x: sx / weight as f64,
        centroid_y: sy / weight as f64,
        raster: RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)?,
    })
}

fn component_key_pixels(component: &GlyphComponent) -> Vec<(i32, i32)> {
    let min_sequence = component
        .runs
        .iter()
        .map(|entry| entry.sequence)
        .min()
        .unwrap();
    let min_coordinate = component
        .runs
        .iter()
        .map(|entry| entry.run.start)
        .min()
        .unwrap();
    let mut pixels = Vec::with_capacity(component.weight);
    for entry in &component.runs {
        for coordinate in entry.run.start..=entry.run.stop() {
            match component.orientation {
                Orientation::Horizontal => pixels.push((
                    component.left + i32::try_from(coordinate - min_coordinate).unwrap(),
                    component.top + i32::try_from(entry.sequence - min_sequence).unwrap(),
                )),
                Orientation::Vertical => pixels.push((
                    component.left + i32::try_from(entry.sequence - min_sequence).unwrap(),
                    component.top + i32::try_from(coordinate - min_coordinate).unwrap(),
                )),
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headers_step::HeadlessHeaderStaff, staff_header::StaffHeader};

    #[derive(Default)]
    struct FakeKeyClassifier {
        calls: Vec<(usize, HeaderBounds, usize)>,
        fail_call: Option<usize>,
    }

    impl KeyShapeClassifier for FakeKeyClassifier {
        type Error = &'static str;

        fn evaluate(
            &mut self,
            glyph: &NativeKeyGlyph,
            _interline: i32,
            _maximum_rank: usize,
            _minimum_grade: f64,
        ) -> Result<Vec<KeyShapeEvaluation>, Self::Error> {
            self.calls.push((glyph.id, glyph.bounds, glyph.weight));
            if self.fail_call == Some(self.calls.len()) {
                return Err("classifier failed");
            }
            Ok(vec![
                KeyShapeEvaluation {
                    shape: NeutralKeyAlterShape::Flat,
                    grade: 0.8,
                },
                KeyShapeEvaluation {
                    shape: NeutralKeyAlterShape::Sharp,
                    grade: 0.7,
                },
            ])
        }
    }

    #[test]
    fn the_pitch_comes_from_the_measured_lines_not_the_nominal_interline() {
        // A staff whose lines are 79.2 px apart where four nominal interlines would be 80: a
        // 1% departure, which is unremarkable on a scanned page. Java divides by the *measured*
        // separation, so the two formulas disagree, and they disagree more the further from the
        // staff centre the alteration sits.
        let context = NativeKeyStaffContext {
            browse_stop: 0,
            envelope_top: 0,
            envelope_bottom: 0,
            staff_mid_y: 139.6,
            classifier_interline: 20,
            line_count: 5,
            staff_interline: 20,
        };
        let (top, bottom) = (100.0, 179.2);

        // Java: ((5-1) * (2y - bottom - top)) / (bottom - top).
        let measured = pitch_position_of(Some((top, bottom)), context, 120.0);
        assert!((measured - (4.0 * ((2.0 * 120.0) - bottom - top) / (bottom - top))).abs() < 1e-12);

        // The interline approximation this replaced, kept here as the thing being distinguished.
        let approximated =
            (2.0 * (120.0 - context.staff_mid_y)) / f64::from(context.classifier_interline);
        assert!(
            (measured - approximated).abs() > 0.01,
            "measured {measured} vs approximated {approximated} must differ"
        );

        // Where the lines *are* exactly four interlines apart, the two agree -- which is why the
        // approximation looked right for so long.
        let exact = pitch_position_of(Some((100.0, 180.0)), context, 120.0);
        assert!((exact - (2.0 * (120.0 - 140.0) / 20.0)).abs() < 1e-12);
    }

    /// Pitch geometry with one flat span, rows 5..15, matching the fixture envelope.
    ///
    /// It must answer rather than decline: a single-alteration candidate runs the trailing-space
    /// check, which needs `pitchToOrdinate`, and a `None` there drops the proposal -- correctly in
    /// production, fatally for a fixture that wants to observe the proposal.
    struct FlatPitchGeometry;

    impl StaffPitchGeometry for FlatPitchGeometry {
        fn line_span_at(&self, _staff_id: usize, _x: f64) -> Option<(f64, f64)> {
            Some((5.0, 15.0))
        }
    }

    fn native_recognizer(
        classifier: FakeKeyClassifier,
    ) -> NativeKeyProposalRecognizer<FakeKeyClassifier, FlatPitchGeometry> {
        // Two flats, drawn the way the pipeline reads them: a tall stem (columns 12-13 and
        // 22-23, rows 5-24, projection 20) plus a *bowl* to the right (rows 16-24, projection 9).
        // The bowl is not decoration -- `refineSignature` demands `minFlatTrail` of ink after the
        // last stem, and a bare stem fails it exactly as Java would fail it. At interline 8 the
        // constants line up as: stem height 20 within quorum 10 .. maxPeakCumul 34; the 10 px stem
        // spacing exceeds maxSharpDelta 5.6, so the sharp builder rejects the projection outright;
        // and the four space columns between the flats stay under maxInnerSpace 6.
        let mut pixels = vec![BACKGROUND; 60 * 40];
        for (stem, bowl) in [(12usize, 14usize), (22, 24)] {
            for y in 5..25 {
                pixels[y * 60 + stem] = FOREGROUND;
                pixels[y * 60 + stem + 1] = FOREGROUND;
            }
            for y in 16..25 {
                for x in bowl..bowl + 4 {
                    pixels[y * 60 + x] = FOREGROUND;
                }
            }
        }
        let source = RunTable::from_pixels(Orientation::Horizontal, 60, 40, &pixels).unwrap();
        NativeKeyProposalRecognizer::new(
            classifier,
            FlatPitchGeometry,
            BTreeMap::from([(7, source)]),
            BTreeMap::from([(
                1,
                NativeKeyStaffContext {
                    browse_stop: 35,
                    envelope_top: 5,
                    envelope_bottom: 27,
                    staff_mid_y: 16.0,
                    classifier_interline: 8,
                    line_count: 5,
                    staff_interline: 8,
                },
            )]),
            BTreeMap::from([(
                1,
                NativeKeyParameters {
                    minimum_component_weight: 2,
                    maximum_component_gap: 2.0,
                    minimum_glyph_weight: 4,
                    // Permissive bounds: this fixture exercises grouping and ranking, not the
                    // size gate, so the gate must not be what decides its outcome.
                    minimum_alter_width: 0.0,
                    minimum_alter_height: 0.0,
                    maximum_alter_width: 16.0,
                    maximum_alter_height: 24.0,
                    maximum_glyph_weight: usize::MAX,
                    maximum_alters: 7,
                    maximum_rank: 3,
                    pipeline: KeyPipelineParameters::new(8, 8),
                    minimum_classifier_grade: 0.5,
                },
            )]),
            100,
            200,
        )
    }

    fn native_input() -> KeyRecognitionInput {
        KeyRecognitionInput {
            system_id: 7,
            staff_id: 1,
            projection_width: 20,
            measure_start: 8,
            browse_start: 10,
        }
    }

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
            // Java `Staff.setClefStop` sets the stop *and* marks the range valid, and
            // `getClefStop` reads the stop only when it is valid. Constructing one without the
            // other produces a state the pipeline cannot reach.
            let mut range = StaffHeaderRange::default();
            range.set_stop(stop);
            range.valid = true;
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

    #[test]
    fn native_key_sources_staff_envelope_components_and_flat_then_sharp_hypotheses() {
        let mut recognizer = native_recognizer(FakeKeyClassifier::default());

        let proposals = recognizer.classify_key_shapes(native_input()).unwrap();

        // Only the flat proposal survives: the 4 px peak spacing exceeds maxSharpDelta, so
        // `inferSignature` returns 0 for the sharp builder before any glyph is classified. The
        // previous enumeration-based flow produced a sharp hypothesis here too; the projection
        // pipeline rejecting it from spacing alone is the point of the port.
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].shape, NeutralKeyAlterShape::Flat);
        assert_eq!(proposals[0].alters.len(), 2);
        // Alter starts are *slice* starts (Java `roi.getStarts()`), not glyph abscissae.
        assert_eq!(
            proposals[0]
                .alters
                .iter()
                .map(|alter| (alter.start, alter.width))
                .collect::<Vec<_>>(),
            [(12, 10), (22, 6)]
        );
        assert_eq!(
            proposals[0]
                .alters
                .iter()
                .map(|alter| alter.bounds)
                .collect::<Vec<_>>(),
            [
                HeaderBounds {
                    x: 12,
                    y: 5,
                    width: 6,
                    height: 20
                },
                HeaderBounds {
                    x: 22,
                    y: 5,
                    width: 6,
                    height: 20
                },
            ]
        );
        // Range start comes from the first space (columns 10-11), the stop from
        // `refineShapeStop`'s projection minimum inside the flat trail window.
        assert_eq!(proposals[0].range.start(), Ok(12));
        assert_eq!(proposals[0].range.stop(), 27);
        // The projection spans min(measureStart, browseStart) = 8, so x = 12 is index 4: the
        // stem column counts 20 rows, the bowl column at x = 14 counts 9.
        assert_eq!(recognizer.projection(1).unwrap()[4], 20);
        assert_eq!(recognizer.projection(1).unwrap()[6], 9);
        assert_eq!(recognizer.projection(1).unwrap()[0], 0);
        // Two glyphs classified, both during the flat pass; the sharp pass never reaches ink.
        assert_eq!(recognizer.classifier().calls.len(), 2);
        assert_eq!(
            recognizer
                .mutations()
                .iter()
                .filter_map(|mutation| match mutation {
                    NativeKeyMutation::GlyphEvaluated { shape, .. } => Some(*shape),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [NeutralKeyAlterShape::Flat, NeutralKeyAlterShape::Flat]
        );
    }

    #[test]
    fn native_key_preserves_registered_prefix_on_classifier_failure() {
        let mut recognizer = native_recognizer(FakeKeyClassifier {
            fail_call: Some(2),
            ..FakeKeyClassifier::default()
        });

        let error = recognizer.classify_key_shapes(native_input()).unwrap_err();

        assert!(matches!(
            error,
            NativeKeyError::Classifier {
                staff_id: 1,
                source: "classifier failed",
                ..
            }
        ));
        assert_eq!(recognizer.classifier().calls.len(), 2);
        assert_eq!(
            recognizer
                .mutations()
                .iter()
                .filter(|mutation| matches!(
                    mutation,
                    NativeKeyMutation::ComponentRegistered { .. }
                ))
                .count(),
            2
        );
    }

    #[test]
    fn native_key_clips_candidate_source_to_header_browse_range() {
        let mut recognizer = native_recognizer(FakeKeyClassifier::default());
        let mut input = native_input();
        input.browse_start = 16;

        let proposals = recognizer.classify_key_shapes(input).unwrap();

        assert_eq!(proposals[0].alters.len(), 1);
        // The first flat lies outside the clipped finder domain and must not contribute a peak;
        // its bowl does poke into the domain, but at projection 9 it stays under the quorum (10),
        // which is precisely the quorum doing its job.
        assert_eq!(proposals[0].alters[0].start, 22);
        assert_eq!(proposals[0].shape, NeutralKeyAlterShape::Flat);
        assert!(
            recognizer
                .classifier()
                .calls
                .iter()
                .all(|call| call.1.x >= 16)
        );
    }
}
