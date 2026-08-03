// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `BeamsStep`.
//!
//! Image closing, beam interpretation, and multiple-rest geometry remain typed
//! visual seams. This module owns their sheet/system ordering, mutations,
//! checked-error continuation, and unconditional BEAM_SPOT cleanup.

use std::{error::Error, fmt};

use audiveris_image::{
    beam_extension::{
        BeamExtensionEvidence, BeamExtensionInput as RasterBeamExtensionInput,
        BeamExtensionParameters, ExtensionBeam, ExtensionGlyph, beam_extension_evidence,
    },
    beam_hooks::{
        HookEvidence, HookGlyph, HookParameters, HookSearchInput, HookSide, hook_search_evidence,
    },
    beam_structure::{
        BeamBeltSides, BeamImpactParameters, BeamImpacts, BeamItem, BeamRaster,
        BeamStructureParameters, analyze_beam_structure, compute_beam_impacts,
    },
    global_filter::global_filter,
    glyph_factory::{GlyphComponent, build_glyph_components},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    system_population::PopulationSystemArea,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralBeamGlyphGroup {
    BeamSpot,
    VerticalSeed,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralBeamGlyph {
    pub id: usize,
    pub top: i32,
    pub left: i32,
    pub geometry: Option<NeutralGlyphGeometry>,
    /// Tight vertical run table retained for native `BeamStructure` analysis.
    pub raster: Option<RunTable>,
    pub groups: Vec<NeutralBeamGlyphGroup>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralGlyphGeometry {
    pub width: usize,
    pub height: usize,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub rounded_centroid_x: i32,
    pub rounded_centroid_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralBeamInterKind {
    Beam,
    SmallBeam,
    BeamHook,
    MultipleRest,
    VerticalSerif,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralBeamInter {
    pub id: usize,
    pub kind: NeutralBeamInterKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralBeamRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralBeamSystem {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
    pub area: PopulationSystemArea,
    pub free_glyphs: Vec<NeutralBeamGlyph>,
    pub inters: Vec<NeutralBeamInter>,
    pub relations: Vec<NeutralBeamRelation>,
    pub beam_group_ids: Vec<usize>,
    pub multiple_rest_min_length: i32,
    pub multiple_rest_evidence: Vec<MultipleRestBeamEvidence>,
    pub multiple_rest_sections: Vec<MultipleRestSection>,
    pub multiple_rests: Vec<NeutralMultipleRestRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralBeamSheet {
    pub id: usize,
    pub beam_height: Option<i32>,
    pub small_beam_height: Option<i32>,
    pub small_beams_enabled: bool,
    pub one_line_staves: bool,
    pub drum_notation: bool,
    pub systems: Vec<NeutralBeamSystem>,
    /// Java `Picture.SourceKey.HEAD_SPOTS`, saved before beam thresholding.
    pub head_spot_runs: Option<RunTable>,
    pub next_glyph_id: usize,
    /// Sheet-global GlyphIndex ownership, in first-registration order.
    pub registered_glyph_ids: Vec<usize>,
    pub mutations: Vec<BeamsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedBeamRaster {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamClosingInput {
    pub sheet_id: usize,
    pub beam_height: i32,
    pub circle_diameter: f64,
    pub circle_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BlackHeadSizingInput<'a> {
    pub sheet_id: usize,
    pub components: &'a [GlyphComponent],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeamSpotWarning<VisualError> {
    MissingBeamScale,
    Visual(VisualError),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeamSystemDelta {
    /// Exact Java mutation order, including prefixes completed before failure.
    pub mutations: Vec<BeamSystemMutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BeamSystemMutation {
    RegisterGlyph(NeutralBeamGlyph),
    AddInter(NeutralBeamInter),
    RemoveInter(usize),
    AddRelation(NeutralBeamRelation),
    AddBeamGroup(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamStageOutcome<VisualError> {
    /// Mutations completed before a checked failure are retained by Java.
    pub delta: BeamSystemDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> BeamStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: BeamSystemDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralBeamSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BeamCandidateClass {
    Standard,
    Small,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamSpotCandidateInput {
    pub sheet_id: usize,
    pub system_id: usize,
    pub glyph_id: usize,
    pub class: BeamCandidateClass,
    /// Java line-major/item-major order. Empty when the optional native kernel
    /// is not configured, preserving the legacy injected seam.
    pub native_candidates: Vec<NativeBeamCandidate>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBeamCandidate {
    pub line_index: usize,
    pub item_index: usize,
    pub item: BeamItem,
    pub impacts: BeamImpacts,
    pub grade: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBeamClassParameters {
    pub structure: BeamStructureParameters,
    pub impacts: BeamImpactParameters,
    pub distance_impact: f64,
    pub minimum_grade: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeBeamKernelConfig {
    /// Java `Picture.SourceKey.NO_STAFF`, not the morphologically closed spot image.
    pub pixel_filter: RunTable,
    pub pixel_filter_offset_x: i32,
    pub pixel_filter_offset_y: i32,
    pub standard: NativeBeamClassParameters,
    pub small: Option<NativeBeamClassParameters>,
    pub extension_systems: Vec<NativeBeamExtensionSystem>,
    pub hook_systems: Vec<NativeBeamHookSystem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeBeamExtensionSystem {
    pub system_id: usize,
    pub beams: Vec<ExtensionBeam>,
    pub seeds: Vec<ExtensionGlyph>,
    pub parameters: BeamExtensionParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeBeamHookSystem {
    pub system_id: usize,
    pub beams: Vec<ExtensionBeam>,
    pub parameters: HookParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamCandidateOutcome<VisualError> {
    pub delta: BeamSystemDelta,
    /// Java `checkBeamGlyph` returned null and appended this spot to assignedSpots.
    pub accepted: bool,
    pub error: Option<VisualError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BeamHookSide {
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamExtensionInput {
    pub sheet_id: usize,
    pub system_id: usize,
    pub remaining_spot_ids: Vec<usize>,
    pub vertical_seed_ids: Vec<usize>,
    pub raw_beam_ids: Vec<usize>,
    pub native_evidence: Vec<BeamExtensionEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamHookInput {
    pub sheet_id: usize,
    pub system_id: usize,
    pub beam_id: usize,
    pub side: BeamHookSide,
    /// Snapshot after initial assignment removal; Java does not remove newly
    /// assigned hook spots between beam/side probes.
    pub remaining_spot_ids: Vec<usize>,
    pub native_evidence: Vec<HookEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeamGroupingInput {
    pub sheet_id: usize,
    pub system_id: usize,
    pub raw_beam_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultipleRestBeamEvidence {
    pub beam_id: usize,
    pub width: i32,
    pub grade: f64,
    pub height: f64,
    pub staff_id: Option<usize>,
    pub staff_tablature: bool,
    pub start_pitch: f64,
    pub stop_pitch: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultipleRestSectionLag {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipleRestSection {
    pub id: usize,
    pub lag: MultipleRestSectionLag,
    pub horizontal_length: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipleRestPeak {
    pub id: usize,
    pub width: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipleRestCandidate {
    pub beam_id: usize,
    pub multiple_rest_inter_id: usize,
    pub left_peak: MultipleRestPeak,
    pub right_peak: MultipleRestPeak,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipleRestProbeOutcome<VisualError> {
    pub candidate: Option<MultipleRestCandidate>,
    pub error: Option<VisualError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BeamHorizontalSide {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipleRestSerifInput {
    pub sheet_id: usize,
    pub system_id: usize,
    pub multiple_rest_inter_id: usize,
    pub side: BeamHorizontalSide,
    pub peak: MultipleRestPeak,
    pub eligible_section_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultipleRestSerifOutcome<VisualError> {
    /// Ordered prefix: glyph registration, serif inter, then relation.
    pub delta: BeamSystemDelta,
    pub relation_id: Option<usize>,
    pub error: Option<VisualError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralMultipleRestRecord {
    pub inter_id: usize,
    pub source_beam_id: usize,
    pub grade: f64,
    pub height: f64,
    pub staff_id: usize,
    pub left_relation_id: Option<usize>,
    pub right_relation_id: Option<usize>,
}

/// First unavailable dependencies in Java: ImageJ morphology, optional black
/// head scale sizing, `BeamsBuilder`, and `MultipleRestsBuilder` geometry.
pub trait VisualBeams {
    type Error;

    fn close_beam_spots(
        &mut self,
        input: BeamClosingInput,
    ) -> Result<ClosedBeamRaster, Self::Error>;

    fn size_black_heads(&mut self, input: BlackHeadSizingInput<'_>) -> Result<(), Self::Error>;

    fn classify_beam_spot(
        &mut self,
        input: BeamSpotCandidateInput,
    ) -> BeamCandidateOutcome<Self::Error>;

    fn extend_beams(&mut self, input: BeamExtensionInput) -> BeamStageOutcome<Self::Error>;

    fn build_hooks(&mut self, input: BeamHookInput) -> BeamStageOutcome<Self::Error>;

    fn group_beams(&mut self, input: BeamGroupingInput) -> BeamStageOutcome<Self::Error>;

    fn probe_multiple_rest(
        &mut self,
        evidence: MultipleRestBeamEvidence,
    ) -> MultipleRestProbeOutcome<Self::Error>;

    fn materialize_multiple_rest_serif(
        &mut self,
        input: MultipleRestSerifInput,
    ) -> MultipleRestSerifOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeamsReport<VisualError> {
    pub spot_warning: Option<BeamSpotWarning<VisualError>>,
    pub system_errors: Vec<(usize, VisualError)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeamsContractError {
    UnknownSystem(usize),
    DuplicateGlyph(usize),
    DuplicateInter(usize),
    MissingRemovedInter(usize),
    DuplicateRelation(usize),
    DuplicateBeamGroup(usize),
    InvalidSpotRaster(RunTableError),
    MultipleRestBeamMismatch { expected: usize, actual: usize },
    MissingRelationEndpoint { relation_id: usize, inter_id: usize },
    InvalidMultipleRestRelation(usize),
    GlyphIdentityOverflow,
}

impl fmt::Display for BeamsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSystem(id) => write!(formatter, "unknown system {id}"),
            Self::DuplicateGlyph(id) => write!(formatter, "duplicate glyph {id}"),
            Self::DuplicateInter(id) => write!(formatter, "duplicate inter {id}"),
            Self::MissingRemovedInter(id) => write!(formatter, "missing removed inter {id}"),
            Self::DuplicateRelation(id) => write!(formatter, "duplicate relation {id}"),
            Self::DuplicateBeamGroup(id) => write!(formatter, "duplicate beam group {id}"),
            Self::InvalidSpotRaster(error) => write!(formatter, "invalid spot raster: {error}"),
            Self::MultipleRestBeamMismatch { expected, actual } => write!(
                formatter,
                "multiple-rest candidate beam {actual} does not match source {expected}"
            ),
            Self::MissingRelationEndpoint {
                relation_id,
                inter_id,
            } => write!(
                formatter,
                "relation {relation_id} references missing inter {inter_id}"
            ),
            Self::InvalidMultipleRestRelation(id) => {
                write!(formatter, "invalid multiple-rest serif relation {id}")
            }
            Self::GlyphIdentityOverflow => formatter.write_str("glyph identity overflow"),
        }
    }
}

impl Error for BeamsContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamsMutation {
    HeadSpotsSaved,
    SpotRegistered {
        system_id: usize,
        glyph_id: usize,
    },
    SpotAttached {
        system_id: usize,
        glyph_id: usize,
    },
    GlyphRegistered {
        system_id: usize,
        glyph_id: usize,
    },
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
    BeamGroupAdded {
        system_id: usize,
        group_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
    BeamSpotRemoved {
        system_id: usize,
        glyph_id: usize,
    },
}

pub struct HeadlessBeamsStep<Visual> {
    visual: Visual,
    native_kernel: Option<NativeBeamKernelConfig>,
}

impl<Visual> HeadlessBeamsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            native_kernel: None,
        }
    }

    #[must_use]
    pub fn with_native_beam_kernel(mut self, config: NativeBeamKernelConfig) -> Self {
        self.native_kernel = Some(config);
        self
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualBeams> HeadlessBeamsStep<Visual> {
    /// Java `AbstractSystemStep.doit`: prolog, systems in source order, epilog.
    /// Per-system checked errors are recorded and later systems continue.
    pub fn process(
        &mut self,
        sheet: &mut NeutralBeamSheet,
    ) -> Result<BeamsReport<Visual::Error>, BeamsContractError> {
        let spot_warning = self.build_and_dispatch_spots(sheet)?;

        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            if let Some(error) = self.build_system_beams(sheet, system_index)? {
                sheet
                    .mutations
                    .push(BeamsMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
                continue;
            }

            if let Some(error) = self.build_multiple_rests(sheet, system_index)? {
                sheet
                    .mutations
                    .push(BeamsMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
            }
        }

        // Java runs this epilog even after checked per-system failures.
        cleanup_beam_spots(sheet);
        Ok(BeamsReport {
            spot_warning,
            system_errors,
        })
    }

    fn build_multiple_rests(
        &mut self,
        sheet: &mut NeutralBeamSheet,
        system_index: usize,
    ) -> Result<Option<Visual::Error>, BeamsContractError> {
        let system_id = sheet.systems[system_index].id;
        let beam_ids = sheet.systems[system_index]
            .inters
            .iter()
            .filter(|inter| inter.kind == NeutralBeamInterKind::Beam)
            .map(|inter| inter.id)
            .collect::<Vec<_>>();
        let mut found = Vec::new();
        for beam_id in beam_ids {
            let Some(evidence) = sheet.systems[system_index]
                .multiple_rest_evidence
                .iter()
                .find(|evidence| evidence.beam_id == beam_id)
                .copied()
            else {
                continue;
            };
            if evidence.width < sheet.systems[system_index].multiple_rest_min_length
                || evidence.staff_id.is_none()
                || evidence.staff_tablature
                || evidence.start_pitch.abs() > 0.2
                || evidence.stop_pitch.abs() > 0.2
            {
                continue;
            }
            let probe = self.visual.probe_multiple_rest(evidence);
            if let Some(error) = probe.error {
                return Ok(Some(error));
            }
            if let Some(candidate) = probe.candidate {
                if candidate.beam_id != beam_id {
                    return Err(BeamsContractError::MultipleRestBeamMismatch {
                        expected: beam_id,
                        actual: candidate.beam_id,
                    });
                }
                found.push((candidate, evidence));
            }
        }
        if found.is_empty() {
            return Ok(None);
        }

        let max_width = found
            .iter()
            .flat_map(|(candidate, _)| [candidate.left_peak.width, candidate.right_peak.width])
            .max()
            .unwrap_or(0);
        let eligible_section_ids =
            eligible_multiple_rest_sections(&sheet.systems[system_index], max_width);
        for (candidate, evidence) in &found {
            let staff_id = evidence.staff_id.expect("filtered multiple-rest staff");
            let rest = NeutralBeamInter {
                id: candidate.multiple_rest_inter_id,
                kind: NeutralBeamInterKind::MultipleRest,
            };
            apply_delta(
                sheet,
                system_index,
                BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(rest)],
                },
            )?;
            sheet.systems[system_index]
                .multiple_rests
                .push(NeutralMultipleRestRecord {
                    inter_id: candidate.multiple_rest_inter_id,
                    source_beam_id: candidate.beam_id,
                    grade: evidence.grade,
                    height: evidence.height,
                    staff_id,
                    left_relation_id: None,
                    right_relation_id: None,
                });

            for (side, peak) in [
                (BeamHorizontalSide::Left, candidate.left_peak),
                (BeamHorizontalSide::Right, candidate.right_peak),
            ] {
                let serif = self
                    .visual
                    .materialize_multiple_rest_serif(MultipleRestSerifInput {
                        sheet_id: sheet.id,
                        system_id,
                        multiple_rest_inter_id: candidate.multiple_rest_inter_id,
                        side,
                        peak,
                        eligible_section_ids: eligible_section_ids.clone(),
                    });
                apply_delta(sheet, system_index, serif.delta)?;
                if serif.error.is_none() {
                    let Some(relation_id) = serif.relation_id else {
                        return Err(BeamsContractError::InvalidMultipleRestRelation(0));
                    };
                    let valid = sheet.systems[system_index]
                        .relations
                        .iter()
                        .find(|relation| relation.id == relation_id)
                        .is_some_and(|relation| {
                            relation.source_inter_id == candidate.multiple_rest_inter_id
                        });
                    if !valid {
                        return Err(BeamsContractError::InvalidMultipleRestRelation(relation_id));
                    }
                }
                let record = sheet.systems[system_index]
                    .multiple_rests
                    .last_mut()
                    .expect("record added before serifs");
                match side {
                    BeamHorizontalSide::Left => {
                        record.left_relation_id = serif.relation_id;
                    }
                    BeamHorizontalSide::Right => {
                        record.right_relation_id = serif.relation_id;
                    }
                }
                if let Some(error) = serif.error {
                    return Ok(Some(error));
                }
            }
        }

        // Java deletes every source beam only after all replacement inters and
        // both serif sides for every found candidate have been created.
        apply_delta(
            sheet,
            system_index,
            BeamSystemDelta {
                mutations: found
                    .iter()
                    .map(|(candidate, _)| BeamSystemMutation::RemoveInter(candidate.beam_id))
                    .collect(),
            },
        )?;
        Ok(None)
    }

    fn build_system_beams(
        &mut self,
        sheet: &mut NeutralBeamSheet,
        system_index: usize,
    ) -> Result<Option<Visual::Error>, BeamsContractError> {
        let system_id = sheet.systems[system_index].id;
        let mut spots = sheet.systems[system_index]
            .free_glyphs
            .iter()
            .filter(|glyph| glyph.groups.contains(&NeutralBeamGlyphGroup::BeamSpot))
            .cloned()
            .collect::<Vec<_>>();
        spots.sort_by(java_full_ordinate_order);
        let has_small = sheet.small_beam_height.is_some() || sheet.small_beams_enabled;
        let mut assigned = Vec::new();

        for spot in &spots {
            let standard_candidates = self.native_candidates(spot, BeamCandidateClass::Standard);
            if !standard_candidates.as_ref().is_some_and(Vec::is_empty) {
                let standard = self.visual.classify_beam_spot(BeamSpotCandidateInput {
                    sheet_id: sheet.id,
                    system_id,
                    glyph_id: spot.id,
                    class: BeamCandidateClass::Standard,
                    native_candidates: standard_candidates.unwrap_or_default(),
                });
                apply_delta(sheet, system_index, standard.delta)?;
                if let Some(error) = standard.error {
                    return Ok(Some(error));
                }
                if standard.accepted {
                    assigned.push(spot.id);
                    continue;
                }
            }
            if has_small {
                let small_candidates = self.native_candidates(spot, BeamCandidateClass::Small);
                if small_candidates.as_ref().is_some_and(Vec::is_empty) {
                    continue;
                }
                let small = self.visual.classify_beam_spot(BeamSpotCandidateInput {
                    sheet_id: sheet.id,
                    system_id,
                    glyph_id: spot.id,
                    class: BeamCandidateClass::Small,
                    native_candidates: small_candidates.unwrap_or_default(),
                });
                apply_delta(sheet, system_index, small.delta)?;
                if let Some(error) = small.error {
                    return Ok(Some(error));
                }
                if small.accepted {
                    assigned.push(spot.id);
                }
            }
        }

        let remaining_spot_ids = spots
            .iter()
            .filter(|spot| !assigned.contains(&spot.id))
            .map(|spot| spot.id)
            .collect::<Vec<_>>();
        let vertical_seed_ids = sheet.systems[system_index]
            .free_glyphs
            .iter()
            .filter(|glyph| glyph.groups.contains(&NeutralBeamGlyphGroup::VerticalSeed))
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>();
        let current_raw_beam_ids = raw_beam_ids(&sheet.systems[system_index]);
        let native_evidence = self.native_extension_evidence(
            &sheet.systems[system_index],
            &remaining_spot_ids,
            &vertical_seed_ids,
            &current_raw_beam_ids,
        );
        let extension = self.visual.extend_beams(BeamExtensionInput {
            sheet_id: sheet.id,
            system_id,
            remaining_spot_ids: remaining_spot_ids.clone(),
            vertical_seed_ids,
            raw_beam_ids: current_raw_beam_ids,
            native_evidence,
        });
        apply_delta(sheet, system_index, extension.delta)?;
        if let Some(error) = extension.error {
            return Ok(Some(error));
        }

        // Java snapshots BeamInter (not SmallBeamInter or BeamHookInter) in SIG
        // source order, then probes TOP followed by BOTTOM for each beam.
        let base_beam_ids = sheet.systems[system_index]
            .inters
            .iter()
            .filter(|inter| inter.kind == NeutralBeamInterKind::Beam)
            .map(|inter| inter.id)
            .collect::<Vec<_>>();
        for beam_id in base_beam_ids {
            for side in [BeamHookSide::Top, BeamHookSide::Bottom] {
                let native_evidence = self.native_hook_evidence(
                    &sheet.systems[system_index],
                    beam_id,
                    side,
                    &remaining_spot_ids,
                );
                let hooks = self.visual.build_hooks(BeamHookInput {
                    sheet_id: sheet.id,
                    system_id,
                    beam_id,
                    side,
                    remaining_spot_ids: remaining_spot_ids.clone(),
                    native_evidence,
                });
                apply_delta(sheet, system_index, hooks.delta)?;
                if let Some(error) = hooks.error {
                    return Ok(Some(error));
                }
            }
        }

        let grouping = self.visual.group_beams(BeamGroupingInput {
            sheet_id: sheet.id,
            system_id,
            raw_beam_ids: raw_beam_ids(&sheet.systems[system_index]),
        });
        apply_delta(sheet, system_index, grouping.delta)?;
        Ok(grouping.error)
    }

    fn native_candidates(
        &self,
        spot: &NeutralBeamGlyph,
        class: BeamCandidateClass,
    ) -> Option<Vec<NativeBeamCandidate>> {
        let config = self.native_kernel.as_ref()?;
        let glyph = spot.raster.as_ref()?;
        let parameters = match class {
            BeamCandidateClass::Standard => config.standard,
            BeamCandidateClass::Small => config.small?,
        };
        let mut structure =
            match analyze_beam_structure(glyph, spot.left, spot.top, parameters.structure) {
                Ok(structure) => structure,
                Err(_) => return Some(Vec::new()),
            };
        structure.adjust_sides(
            spot.left,
            glyph.width(),
            parameters.structure.min_beam_width_low,
        );
        structure.split_stuck_lines(parameters.structure.typical_height);
        let raster = BeamRaster {
            table: &config.pixel_filter,
            offset_x: config.pixel_filter_offset_x,
            offset_y: config.pixel_filter_offset_y,
        };
        let line_count = structure.lines.len();
        let mut candidates = Vec::new();
        for (line_index, line) in structure.lines.into_iter().enumerate() {
            for (item_index, item) in line.items.into_iter().enumerate() {
                let Ok(impacts) = compute_beam_impacts(
                    item,
                    BeamBeltSides {
                        above: line_index == 0,
                        below: line_index == line_count - 1,
                    },
                    raster,
                    parameters.distance_impact,
                    parameters.impacts,
                ) else {
                    continue;
                };
                let grade = native_beam_grade(impacts);
                if grade >= parameters.minimum_grade {
                    candidates.push(NativeBeamCandidate {
                        line_index,
                        item_index,
                        item,
                        impacts,
                        grade,
                    });
                }
            }
        }
        Some(candidates)
    }

    fn native_extension_evidence(
        &self,
        system: &NeutralBeamSystem,
        remaining_spot_ids: &[usize],
        vertical_seed_ids: &[usize],
        raw_beam_ids: &[usize],
    ) -> Vec<BeamExtensionEvidence> {
        let Some(config) = self.native_kernel.as_ref() else {
            return Vec::new();
        };
        let Some(scene) = config
            .extension_systems
            .iter()
            .find(|scene| scene.system_id == system.id)
        else {
            return Vec::new();
        };
        let beams = raw_beam_ids
            .iter()
            .filter_map(|id| scene.beams.iter().find(|beam| beam.id == *id).copied())
            .collect::<Vec<_>>();
        let seeds = vertical_seed_ids
            .iter()
            .filter_map(|id| scene.seeds.iter().find(|seed| seed.id == *id).copied())
            .collect::<Vec<_>>();
        let spots = remaining_spot_ids
            .iter()
            .filter_map(|id| {
                let glyph = system.free_glyphs.iter().find(|glyph| glyph.id == *id)?;
                let geometry = glyph.geometry?;
                Some(ExtensionGlyph {
                    id: glyph.id,
                    left: glyph.left,
                    top: glyph.top,
                    width: geometry.width,
                    height: geometry.height,
                    vertical_median: None,
                })
            })
            .collect::<Vec<_>>();
        beam_extension_evidence(
            RasterBeamExtensionInput {
                beams: &beams,
                seeds: &seeds,
                spots: &spots,
                raster: BeamRaster {
                    table: &config.pixel_filter,
                    offset_x: config.pixel_filter_offset_x,
                    offset_y: config.pixel_filter_offset_y,
                },
                parameters: scene.parameters,
            },
            |point, height| {
                [-1.0, 1.0].into_iter().all(|direction| {
                    system
                        .area
                        .contains(point.0, point.1 + (direction * height / 2.0))
                })
            },
        )
    }

    fn native_hook_evidence(
        &self,
        system: &NeutralBeamSystem,
        beam_id: usize,
        side: BeamHookSide,
        remaining_spot_ids: &[usize],
    ) -> Vec<HookEvidence> {
        let Some(config) = self.native_kernel.as_ref() else {
            return Vec::new();
        };
        let Some(scene) = config
            .hook_systems
            .iter()
            .find(|scene| scene.system_id == system.id)
        else {
            return Vec::new();
        };
        let Some(base) = scene.beams.iter().find(|beam| beam.id == beam_id).copied() else {
            return Vec::new();
        };
        let spots = remaining_spot_ids
            .iter()
            .filter_map(|id| {
                let glyph = system.free_glyphs.iter().find(|glyph| glyph.id == *id)?;
                let geometry = glyph.geometry?;
                Some(HookGlyph {
                    id: glyph.id,
                    left: glyph.left,
                    top: glyph.top,
                    width: geometry.width,
                    height: geometry.height,
                    weight: geometry.weight,
                    centroid_x: geometry.centroid_x,
                    centroid_y: geometry.centroid_y,
                })
            })
            .collect::<Vec<_>>();
        hook_search_evidence(
            HookSearchInput {
                base,
                spots: &spots,
                raw_beams: &scene.beams,
                raster: BeamRaster {
                    table: &config.pixel_filter,
                    offset_x: config.pixel_filter_offset_x,
                    offset_y: config.pixel_filter_offset_y,
                },
                parameters: scene.parameters,
            },
            match side {
                BeamHookSide::Top => HookSide::Top,
                BeamHookSide::Bottom => HookSide::Bottom,
            },
        )
    }

    fn dispatch_spots(
        &self,
        sheet: &mut NeutralBeamSheet,
        components: Vec<GlyphComponent>,
    ) -> Result<(), BeamsContractError> {
        for component in components {
            let relevant = sheet
                .systems
                .iter()
                .enumerate()
                .filter_map(|(index, system)| {
                    let x = component.rounded_centroid_x;
                    let y = component.rounded_centroid_y;
                    (system.area.contains(f64::from(x), f64::from(y))
                        && x >= system.left
                        && x <= system.right)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            if relevant.is_empty() {
                continue;
            }
            let glyph_id = sheet.next_glyph_id;
            sheet.next_glyph_id = sheet
                .next_glyph_id
                .checked_add(1)
                .ok_or(BeamsContractError::GlyphIdentityOverflow)?;
            if sheet.registered_glyph_ids.contains(&glyph_id) {
                return Err(BeamsContractError::DuplicateGlyph(glyph_id));
            }
            sheet.registered_glyph_ids.push(glyph_id);
            let geometry = NeutralGlyphGeometry {
                width: component.width,
                height: component.height,
                weight: component.weight,
                centroid_x: component.centroid_x,
                centroid_y: component.centroid_y,
                rounded_centroid_x: component.rounded_centroid_x,
                rounded_centroid_y: component.rounded_centroid_y,
            };
            for system_index in relevant {
                let system_id = sheet.systems[system_index].id;
                sheet.mutations.push(BeamsMutation::SpotRegistered {
                    system_id,
                    glyph_id,
                });
                sheet.systems[system_index]
                    .free_glyphs
                    .push(NeutralBeamGlyph {
                        id: glyph_id,
                        top: component.top,
                        left: component.left,
                        geometry: Some(geometry),
                        raster: Some(component_vertical_raster(&component)?),
                        groups: vec![NeutralBeamGlyphGroup::BeamSpot],
                    });
                sheet.mutations.push(BeamsMutation::SpotAttached {
                    system_id,
                    glyph_id,
                });
            }
        }
        Ok(())
    }

    fn build_and_dispatch_spots(
        &mut self,
        sheet: &mut NeutralBeamSheet,
    ) -> Result<Option<BeamSpotWarning<Visual::Error>>, BeamsContractError> {
        let Some(main_height) = sheet.beam_height else {
            return Ok(Some(BeamSpotWarning::MissingBeamScale));
        };
        let small_height = sheet.small_beam_height.or_else(|| {
            sheet
                .small_beams_enabled
                .then(|| java_rint(f64::from(main_height) * 0.6))
        });
        let beam_height = small_height.map_or(main_height, |small| main_height.min(small));
        let circle_diameter = f64::from(beam_height) * 0.8;
        let circle_radius = ((circle_diameter - 1.0) / 2.0) as f32;
        let closed = match self.visual.close_beam_spots(BeamClosingInput {
            sheet_id: sheet.id,
            beam_height,
            circle_diameter,
            circle_radius,
        }) {
            Ok(closed) => closed,
            Err(error) => return Ok(Some(BeamSpotWarning::Visual(error))),
        };

        // Java saves HEAD_SPOTS immediately after closing, before the beam
        // threshold and glyph construction that can still fail.
        let head_pixels = global_filter(&closed.pixels, 170);
        sheet.head_spot_runs = Some(
            RunTable::from_pixels(
                Orientation::Horizontal,
                closed.width,
                closed.height,
                &head_pixels,
            )
            .map_err(BeamsContractError::InvalidSpotRaster)?,
        );
        sheet.mutations.push(BeamsMutation::HeadSpotsSaved);

        let spot_pixels = global_filter(&closed.pixels, 140);
        let spot_runs = RunTable::from_pixels(
            Orientation::Horizontal,
            closed.width,
            closed.height,
            &spot_pixels,
        )
        .map_err(BeamsContractError::InvalidSpotRaster)?;
        let components = build_glyph_components(&spot_runs, 0, 0);
        if !sheet.one_line_staves
            && !sheet.drum_notation
            && let Err(error) = self.visual.size_black_heads(BlackHeadSizingInput {
                sheet_id: sheet.id,
                components: &components,
            })
        {
            return Ok(Some(BeamSpotWarning::Visual(error)));
        }
        self.dispatch_spots(sheet, components)?;
        Ok(None)
    }
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn native_beam_grade(impacts: BeamImpacts) -> f64 {
    let values = [
        impacts.width,
        impacts.min_height,
        impacts.max_height,
        impacts.core,
        impacts.belt,
        impacts.distance,
    ];
    let weights = [0.5, 1.0, 1.0, 2.0, 2.0, 2.0];
    let mut product = 1.0;
    let mut total_weight = 0.0;
    for (impact, weight) in values.into_iter().zip(weights) {
        total_weight += weight;
        if impact == 0.0 {
            product = 0.0;
        } else if weight != 0.0 {
            product *= impact.powf(weight);
        }
    }
    0.8 * product.powf(1.0 / total_weight)
}

fn component_vertical_raster(component: &GlyphComponent) -> Result<RunTable, BeamsContractError> {
    let mut pixels = vec![BACKGROUND; component.width * component.height];
    let minimum_sequence = component
        .runs
        .iter()
        .map(|entry| entry.sequence)
        .min()
        .expect("glyph component has at least one run");
    let minimum_coordinate = component
        .runs
        .iter()
        .map(|entry| entry.run.start)
        .min()
        .expect("glyph component has at least one run");
    for entry in &component.runs {
        match component.orientation {
            Orientation::Horizontal => {
                let y = entry.sequence - minimum_sequence;
                for source_x in entry.run.start..=entry.run.stop() {
                    let x = source_x - minimum_coordinate;
                    pixels[(y * component.width) + x] = FOREGROUND;
                }
            }
            Orientation::Vertical => {
                let x = entry.sequence - minimum_sequence;
                for source_y in entry.run.start..=entry.run.stop() {
                    let y = source_y - minimum_coordinate;
                    pixels[(y * component.width) + x] = FOREGROUND;
                }
            }
        }
    }
    RunTable::from_pixels(
        Orientation::Vertical,
        component.width,
        component.height,
        &pixels,
    )
    .map_err(BeamsContractError::InvalidSpotRaster)
}

fn java_full_ordinate_order(one: &NeutralBeamGlyph, two: &NeutralBeamGlyph) -> std::cmp::Ordering {
    let dy = one.top.wrapping_sub(two.top);
    if dy != 0 {
        return dy.cmp(&0);
    }
    let dx = one.left.wrapping_sub(two.left);
    if dx != 0 {
        return dx.cmp(&0);
    }
    one.id.cmp(&two.id)
}

fn raw_beam_ids(system: &NeutralBeamSystem) -> Vec<usize> {
    system
        .inters
        .iter()
        .filter(|inter| {
            matches!(
                inter.kind,
                NeutralBeamInterKind::Beam
                    | NeutralBeamInterKind::SmallBeam
                    | NeutralBeamInterKind::BeamHook
            )
        })
        .map(|inter| inter.id)
        .collect()
}

fn eligible_multiple_rest_sections(system: &NeutralBeamSystem, max_width: i32) -> Vec<usize> {
    let mut sections = [
        MultipleRestSectionLag::Vertical,
        MultipleRestSectionLag::Horizontal,
    ]
    .into_iter()
    .flat_map(|lag| {
        system
            .multiple_rest_sections
            .iter()
            .filter(move |section| section.lag == lag)
    })
    .filter(|section| {
        section.horizontal_length <= max_width
            && section.x < system.right.saturating_add(1)
            && section.x.saturating_add(section.width) > system.left
            && section.y < system.bottom.saturating_add(1)
            && section.y.saturating_add(section.height) > system.top
    })
    .collect::<Vec<_>>();
    sections.sort_by_key(|section| section.x);
    sections.into_iter().map(|section| section.id).collect()
}

fn apply_delta(
    sheet: &mut NeutralBeamSheet,
    system_index: usize,
    delta: BeamSystemDelta,
) -> Result<(), BeamsContractError> {
    let system_id = sheet.systems[system_index].id;
    for mutation in delta.mutations {
        match mutation {
            BeamSystemMutation::RegisterGlyph(glyph) => {
                if sheet.registered_glyph_ids.contains(&glyph.id) {
                    return Err(BeamsContractError::DuplicateGlyph(glyph.id));
                }
                sheet.registered_glyph_ids.push(glyph.id);
                sheet.mutations.push(BeamsMutation::GlyphRegistered {
                    system_id,
                    glyph_id: glyph.id,
                });
            }
            BeamSystemMutation::AddInter(inter) => {
                if sheet.systems[system_index]
                    .inters
                    .iter()
                    .any(|existing| existing.id == inter.id)
                {
                    return Err(BeamsContractError::DuplicateInter(inter.id));
                }
                sheet.systems[system_index].inters.push(inter);
                sheet.mutations.push(BeamsMutation::InterAdded {
                    system_id,
                    inter_id: inter.id,
                });
            }
            BeamSystemMutation::RemoveInter(inter_id) => {
                let Some(index) = sheet.systems[system_index]
                    .inters
                    .iter()
                    .position(|inter| inter.id == inter_id)
                else {
                    return Err(BeamsContractError::MissingRemovedInter(inter_id));
                };
                sheet.systems[system_index].inters.remove(index);
                sheet.mutations.push(BeamsMutation::InterRemoved {
                    system_id,
                    inter_id,
                });
            }
            BeamSystemMutation::AddRelation(relation) => {
                for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                    if !sheet.systems[system_index]
                        .inters
                        .iter()
                        .any(|inter| inter.id == inter_id)
                    {
                        return Err(BeamsContractError::MissingRelationEndpoint {
                            relation_id: relation.id,
                            inter_id,
                        });
                    }
                }
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(BeamsContractError::DuplicateRelation(relation.id));
                }
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(BeamsMutation::RelationAdded {
                    system_id,
                    relation_id: relation.id,
                });
            }
            BeamSystemMutation::AddBeamGroup(group_id) => {
                if sheet.systems[system_index]
                    .beam_group_ids
                    .contains(&group_id)
                {
                    return Err(BeamsContractError::DuplicateBeamGroup(group_id));
                }
                sheet.systems[system_index].beam_group_ids.push(group_id);
                sheet.mutations.push(BeamsMutation::BeamGroupAdded {
                    system_id,
                    group_id,
                });
            }
        }
    }
    Ok(())
}

fn cleanup_beam_spots(sheet: &mut NeutralBeamSheet) {
    for system in &mut sheet.systems {
        let system_id = system.id;
        system.free_glyphs.retain(|glyph| {
            let remove = glyph.groups == [NeutralBeamGlyphGroup::BeamSpot];
            if remove {
                sheet.mutations.push(BeamsMutation::BeamSpotRemoved {
                    system_id,
                    glyph_id: glyph.id,
                });
            }
            !remove
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::system_population::{
        BoundarySegment, PopulationSystemGeometry, StaffBoundary, SystemStaffBoundaries,
        build_population_system_areas,
    };
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeVisual {
        closed: Option<Result<ClosedBeamRaster, &'static str>>,
        head_sizing: Option<Result<(), &'static str>>,
        candidates:
            BTreeMap<(usize, usize, BeamCandidateClass), BeamCandidateOutcome<&'static str>>,
        extensions: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        hooks: BTreeMap<(usize, usize, BeamHookSide), BeamStageOutcome<&'static str>>,
        groups: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        rest_probes: BTreeMap<usize, MultipleRestProbeOutcome<&'static str>>,
        serifs: BTreeMap<(usize, BeamHorizontalSide), MultipleRestSerifOutcome<&'static str>>,
        calls: Vec<(&'static str, usize)>,
        closing_inputs: Vec<BeamClosingInput>,
        sized_components: Vec<Vec<(usize, i32, i32, usize)>>,
        candidate_inputs: Vec<BeamSpotCandidateInput>,
        extension_inputs: Vec<BeamExtensionInput>,
        hook_inputs: Vec<BeamHookInput>,
        grouping_inputs: Vec<BeamGroupingInput>,
        serif_inputs: Vec<MultipleRestSerifInput>,
    }

    impl VisualBeams for FakeVisual {
        type Error = &'static str;

        fn close_beam_spots(
            &mut self,
            input: BeamClosingInput,
        ) -> Result<ClosedBeamRaster, Self::Error> {
            self.calls.push(("close", input.sheet_id));
            self.closing_inputs.push(input);
            self.closed.take().unwrap()
        }

        fn size_black_heads(&mut self, input: BlackHeadSizingInput<'_>) -> Result<(), Self::Error> {
            self.calls.push(("heads", input.sheet_id));
            self.sized_components.push(
                input
                    .components
                    .iter()
                    .map(|component| {
                        (
                            component.ancestor_mark,
                            component.left,
                            component.top,
                            component.weight,
                        )
                    })
                    .collect(),
            );
            self.head_sizing.take().unwrap_or(Ok(()))
        }

        fn classify_beam_spot(
            &mut self,
            input: BeamSpotCandidateInput,
        ) -> BeamCandidateOutcome<Self::Error> {
            self.candidate_inputs.push(input.clone());
            self.calls.push((
                match input.class {
                    BeamCandidateClass::Standard => "standard",
                    BeamCandidateClass::Small => "small",
                },
                input.glyph_id,
            ));
            self.candidates
                .remove(&(input.system_id, input.glyph_id, input.class))
                .unwrap_or(BeamCandidateOutcome {
                    delta: BeamSystemDelta::default(),
                    accepted: false,
                    error: None,
                })
        }

        fn extend_beams(&mut self, input: BeamExtensionInput) -> BeamStageOutcome<Self::Error> {
            self.calls.push(("extend", input.system_id));
            self.extension_inputs.push(input.clone());
            self.extensions
                .remove(&input.system_id)
                .unwrap_or_else(|| BeamStageOutcome::success(BeamSystemDelta::default()))
        }

        fn build_hooks(&mut self, input: BeamHookInput) -> BeamStageOutcome<Self::Error> {
            self.calls.push((
                match input.side {
                    BeamHookSide::Top => "hook-top",
                    BeamHookSide::Bottom => "hook-bottom",
                },
                input.beam_id,
            ));
            self.hook_inputs.push(input.clone());
            self.hooks
                .remove(&(input.system_id, input.beam_id, input.side))
                .unwrap_or_else(|| BeamStageOutcome::success(BeamSystemDelta::default()))
        }

        fn group_beams(&mut self, input: BeamGroupingInput) -> BeamStageOutcome<Self::Error> {
            self.calls.push(("group", input.system_id));
            self.grouping_inputs.push(input.clone());
            self.groups
                .remove(&input.system_id)
                .unwrap_or_else(|| BeamStageOutcome::success(BeamSystemDelta::default()))
        }

        fn probe_multiple_rest(
            &mut self,
            evidence: MultipleRestBeamEvidence,
        ) -> MultipleRestProbeOutcome<Self::Error> {
            self.calls.push(("rest-probe", evidence.beam_id));
            self.rest_probes
                .remove(&evidence.beam_id)
                .unwrap_or(MultipleRestProbeOutcome {
                    candidate: None,
                    error: None,
                })
        }

        fn materialize_multiple_rest_serif(
            &mut self,
            input: MultipleRestSerifInput,
        ) -> MultipleRestSerifOutcome<Self::Error> {
            self.calls.push((
                match input.side {
                    BeamHorizontalSide::Left => "serif-left",
                    BeamHorizontalSide::Right => "serif-right",
                },
                input.multiple_rest_inter_id,
            ));
            self.serif_inputs.push(input.clone());
            self.serifs
                .remove(&(input.multiple_rest_inter_id, input.side))
                .unwrap()
        }
    }

    fn boundary(left: i32, right: i32, y: i32) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (f64::from(left), f64::from(y)),
                end: (f64::from(right), f64::from(y)),
            }],
        }
    }

    fn system(id: usize, left: i32, right: i32, area: PopulationSystemArea) -> NeutralBeamSystem {
        NeutralBeamSystem {
            id,
            left,
            right,
            top: 0,
            bottom: 99,
            area,
            free_glyphs: Vec::new(),
            inters: Vec::new(),
            relations: Vec::new(),
            beam_group_ids: Vec::new(),
            multiple_rest_min_length: 40,
            multiple_rest_evidence: Vec::new(),
            multiple_rest_sections: Vec::new(),
            multiple_rests: Vec::new(),
        }
    }

    fn sheet() -> NeutralBeamSheet {
        let geometries = [
            PopulationSystemGeometry {
                system_id: 2,
                left: 10,
                width: 41,
                top: 0,
                bottom: 60,
                area_left: 0,
                deskewed_upper_left_x: 10.0,
            },
            PopulationSystemGeometry {
                system_id: 1,
                left: 40,
                width: 51,
                top: 40,
                bottom: 100,
                area_left: 0,
                deskewed_upper_left_x: 40.0,
            },
        ];
        let staff_lines = [
            SystemStaffBoundaries {
                first_line: boundary(10, 50, 10),
                last_line: boundary(10, 50, 30),
            },
            SystemStaffBoundaries {
                first_line: boundary(40, 90, 70),
                last_line: boundary(40, 90, 90),
            },
        ];
        let areas = build_population_system_areas(&geometries, &staff_lines, 100, 100, 0);
        NeutralBeamSheet {
            id: 9,
            beam_height: Some(10),
            small_beam_height: None,
            small_beams_enabled: false,
            one_line_staves: false,
            drum_notation: false,
            systems: vec![
                system(2, 10, 50, areas[0].clone()),
                system(1, 40, 90, areas[1].clone()),
            ],
            head_spot_runs: None,
            next_glyph_id: 7,
            registered_glyph_ids: Vec::new(),
            mutations: Vec::new(),
        }
    }

    fn raster() -> ClosedBeamRaster {
        ClosedBeamRaster {
            width: 3,
            height: 2,
            pixels: vec![139, 140, 141, 169, 170, 171],
        }
    }

    #[derive(Clone, Copy)]
    struct TestSpot {
        x: usize,
        y: usize,
    }

    fn visual_with_spots(spots: Vec<TestSpot>) -> FakeVisual {
        let closed = if spots.is_empty() {
            ClosedBeamRaster {
                width: 3,
                height: 2,
                pixels: vec![255; 6],
            }
        } else {
            let mut pixels = vec![255; 100 * 100];
            for spot in spots {
                pixels[(spot.y * 100) + spot.x] = 0;
            }
            ClosedBeamRaster {
                width: 100,
                height: 100,
                pixels,
            }
        };
        FakeVisual {
            closed: Some(Ok(closed)),
            ..FakeVisual::default()
        }
    }

    fn beam_spot_glyph(id: usize, top: i32, left: i32) -> NeutralBeamGlyph {
        NeutralBeamGlyph {
            id,
            top,
            left,
            geometry: None,
            raster: None,
            groups: vec![NeutralBeamGlyphGroup::BeamSpot],
        }
    }

    fn native_kernel_config(standard_minimum_grade: f64) -> NativeBeamKernelConfig {
        let mut pixels = vec![BACKGROUND; 40 * 40];
        for y in 20..24 {
            for x in 10..18 {
                pixels[(y * 40) + x] = FOREGROUND;
            }
        }
        let class = NativeBeamClassParameters {
            structure: BeamStructureParameters {
                typical_height: 4.0,
                core_section_width: 2,
                min_hook_width_low: 2.0,
                max_item_x_gap: 1,
                min_beam_width_low: 4.0,
                max_hook_width: 7.0,
            },
            impacts: BeamImpactParameters {
                belt_margin_dx: 1,
                belt_margin_dy: 1,
                min_core_black_ratio: 0.75,
                max_belt_black_ratio: 0.25,
                min_width_low: 4.0,
                min_width_high: 10.0,
                min_height_low: 2.0,
                typical_height: 4.0,
                max_height_high: 6.0,
            },
            distance_impact: 1.0,
            minimum_grade: standard_minimum_grade,
        };
        NativeBeamKernelConfig {
            pixel_filter: RunTable::from_pixels(Orientation::Horizontal, 40, 40, &pixels).unwrap(),
            pixel_filter_offset_x: 0,
            pixel_filter_offset_y: 0,
            standard: class,
            small: Some(NativeBeamClassParameters {
                minimum_grade: 0.08,
                ..class
            }),
            extension_systems: Vec::new(),
            hook_systems: Vec::new(),
        }
    }

    fn native_beam_glyph(id: usize) -> NeutralBeamGlyph {
        NeutralBeamGlyph {
            id,
            top: 20,
            left: 10,
            geometry: Some(NeutralGlyphGeometry {
                width: 8,
                height: 4,
                weight: 32,
                centroid_x: 13.5,
                centroid_y: 21.5,
                rounded_centroid_x: 14,
                rounded_centroid_y: 22,
            }),
            raster: Some(
                RunTable::from_pixels(Orientation::Vertical, 8, 4, &[FOREGROUND; 32]).unwrap(),
            ),
            groups: vec![NeutralBeamGlyphGroup::BeamSpot],
        }
    }

    #[test]
    fn runs_prolog_system_stages_and_epilog_in_java_order() {
        let visual = visual_with_spots(vec![TestSpot { x: 44, y: 25 }]);
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, Vec::new());
        assert_eq!(
            step.visual().calls,
            vec![
                ("close", 9),
                ("heads", 9),
                ("standard", 7),
                ("extend", 2),
                ("group", 2),
                ("extend", 1),
                ("group", 1),
            ]
        );
        assert!(
            sheet
                .systems
                .iter()
                .all(|system| system.free_glyphs.is_empty())
        );
        assert_eq!(sheet.registered_glyph_ids, vec![7]);
        assert_eq!(
            step.visual().closing_inputs,
            vec![BeamClosingInput {
                sheet_id: 9,
                beam_height: 10,
                circle_diameter: 8.0,
                circle_radius: 3.5,
            }]
        );
        assert_eq!(step.visual().sized_components, vec![vec![(1, 44, 25, 1)]]);
        let head_pixels = sheet.head_spot_runs.as_ref().unwrap().to_pixels();
        assert_eq!(head_pixels[(25 * 100) + 44], 0);
        assert_eq!(head_pixels[(25 * 100) + 45], 255);
        assert_eq!(
            sheet.mutations,
            vec![
                BeamsMutation::HeadSpotsSaved,
                BeamsMutation::SpotRegistered {
                    system_id: 2,
                    glyph_id: 7
                },
                BeamsMutation::SpotAttached {
                    system_id: 2,
                    glyph_id: 7
                },
                BeamsMutation::BeamSpotRemoved {
                    system_id: 2,
                    glyph_id: 7
                },
            ]
        );
    }

    #[test]
    fn spot_failure_is_swallowed_and_systems_still_run() {
        let visual = FakeVisual {
            closed: Some(Err("closing failed")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.spot_warning,
            Some(BeamSpotWarning::Visual("closing failed"))
        );
        assert_eq!(step.visual().calls.len(), 5);
        assert!(sheet.registered_glyph_ids.is_empty());
    }

    #[test]
    fn scale_selection_uses_small_beam_fallback_and_java_morphology_parameters() {
        let visual = visual_with_spots(Vec::new());
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.small_beams_enabled = true;

        step.process(&mut sheet).unwrap();

        let input = step.visual().closing_inputs[0];
        assert_eq!(input.beam_height, 6);
        assert!((input.circle_diameter - 4.8).abs() < f64::EPSILON * 8.0);
        assert_eq!(input.circle_radius, 1.9_f32);
    }

    #[test]
    fn missing_beam_scale_is_swallowed_before_raster_but_systems_still_run() {
        let visual = FakeVisual::default();
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.beam_height = None;

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.spot_warning, Some(BeamSpotWarning::MissingBeamScale));
        assert_eq!(
            step.visual().calls,
            vec![("extend", 2), ("group", 2), ("extend", 1), ("group", 1),]
        );
        assert!(sheet.head_spot_runs.is_none());
    }

    #[test]
    fn black_head_sizing_failure_retains_saved_head_runs_and_aborts_spot_dispatch() {
        let visual = FakeVisual {
            closed: Some(Ok(raster())),
            head_sizing: Some(Err("head sizing failed")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.spot_warning,
            Some(BeamSpotWarning::Visual("head sizing failed"))
        );
        assert_eq!(step.visual().sized_components.len(), 1);
        assert!(sheet.head_spot_runs.is_some());
        assert_eq!(sheet.mutations, vec![BeamsMutation::HeadSpotsSaved]);
    }

    #[test]
    fn dispatch_keeps_spot_on_inclusive_system_left_boundary() {
        let visual = visual_with_spots(vec![TestSpot { x: 10, y: 25 }]);
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.next_glyph_id = 8;

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.registered_glyph_ids, vec![8]);
        assert!(
            sheet
                .systems
                .iter()
                .all(|system| system.free_glyphs.is_empty())
        );
    }

    #[test]
    fn native_component_geometry_survives_registration_and_shared_glyph_ownership() {
        let mut pixels = vec![255; 100 * 100];
        for (x, y) in [(20, 24), (21, 24), (20, 25), (21, 25)] {
            pixels[(y * 100) + x] = 0;
        }
        let runs = RunTable::from_pixels(Orientation::Horizontal, 100, 100, &pixels).unwrap();
        let components = build_glyph_components(&runs, 0, 0);
        let step = HeadlessBeamsStep::new(FakeVisual::default());
        let mut sheet = sheet();
        sheet.next_glyph_id = 40;

        step.dispatch_spots(&mut sheet, components).unwrap();

        assert_eq!(sheet.registered_glyph_ids, vec![40]);
        assert_eq!(sheet.next_glyph_id, 41);
        let glyph = &sheet.systems[0].free_glyphs[0];
        assert_eq!((glyph.left, glyph.top), (20, 24));
        assert_eq!(
            glyph.geometry,
            Some(NeutralGlyphGeometry {
                width: 2,
                height: 2,
                weight: 4,
                centroid_x: 20.5,
                centroid_y: 24.5,
                rounded_centroid_x: 20,
                rounded_centroid_y: 24,
            })
        );
    }

    #[test]
    fn one_line_staff_skips_optional_black_head_sizer_but_keeps_native_components() {
        let visual = FakeVisual {
            closed: Some(Ok(ClosedBeamRaster {
                width: 100,
                height: 100,
                pixels: {
                    let mut pixels = vec![255; 100 * 100];
                    pixels[(25 * 100) + 20] = 0;
                    pixels
                },
            })),
            head_sizing: Some(Err("must not be called")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.one_line_staves = true;

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.spot_warning, None);
        assert!(step.visual().sized_components.is_empty());
        assert_eq!(sheet.registered_glyph_ids, vec![7]);
    }

    #[test]
    fn per_system_builder_sorts_spots_retries_small_tracks_assignment_and_hooks_beams_only() {
        let mut visual = visual_with_spots(Vec::new());
        visual.candidates.insert(
            (2, 20, BeamCandidateClass::Standard),
            BeamCandidateOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![
                        BeamSystemMutation::AddInter(NeutralBeamInter {
                            id: 201,
                            kind: NeutralBeamInterKind::BeamHook,
                        }),
                        BeamSystemMutation::AddInter(NeutralBeamInter {
                            id: 202,
                            kind: NeutralBeamInterKind::Beam,
                        }),
                        BeamSystemMutation::AddRelation(NeutralBeamRelation {
                            id: 203,
                            source_inter_id: 201,
                            target_inter_id: 202,
                        }),
                    ],
                },
                accepted: true,
                error: None,
            },
        );
        visual.candidates.insert(
            (2, 30, BeamCandidateClass::Small),
            BeamCandidateOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 301,
                        kind: NeutralBeamInterKind::SmallBeam,
                    })],
                },
                accepted: true,
                error: None,
            },
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.small_beams_enabled = true;
        sheet.systems[0].free_glyphs = vec![
            beam_spot_glyph(10, 10, 5),
            beam_spot_glyph(30, 5, 20),
            beam_spot_glyph(20, 5, 10),
        ];

        step.process(&mut sheet).unwrap();

        assert_eq!(
            step.visual()
                .candidate_inputs
                .iter()
                .map(|input| (input.glyph_id, input.class))
                .collect::<Vec<_>>(),
            vec![
                (20, BeamCandidateClass::Standard),
                (30, BeamCandidateClass::Standard),
                (30, BeamCandidateClass::Small),
                (10, BeamCandidateClass::Standard),
                (10, BeamCandidateClass::Small),
            ]
        );
        assert_eq!(
            step.visual().extension_inputs[0].remaining_spot_ids,
            vec![10]
        );
        assert_eq!(
            step.visual()
                .hook_inputs
                .iter()
                .map(|input| (input.beam_id, input.side, input.remaining_spot_ids.clone()))
                .collect::<Vec<_>>(),
            vec![
                (202, BeamHookSide::Top, vec![10]),
                (202, BeamHookSide::Bottom, vec![10]),
            ]
        );
        assert_eq!(
            step.visual().grouping_inputs[0].raw_beam_ids,
            vec![201, 202, 301]
        );
        assert_eq!(sheet.systems[0].relations[0].id, 203);
    }

    #[test]
    fn classifier_failure_keeps_mutation_prefix_and_skips_remaining_system_stages() {
        let mut visual = visual_with_spots(Vec::new());
        visual.candidates.insert(
            (2, 10, BeamCandidateClass::Standard),
            BeamCandidateOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 100,
                        kind: NeutralBeamInterKind::BeamHook,
                    })],
                },
                accepted: false,
                error: Some("candidate geometry failed"),
            },
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.systems[0]
            .free_glyphs
            .push(beam_spot_glyph(10, 10, 5));

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "candidate geometry failed")]);
        assert_eq!(sheet.systems[0].inters[0].id, 100);
        assert!(
            !step
                .visual()
                .extension_inputs
                .iter()
                .any(|input| input.system_id == 2)
        );
        assert!(
            step.visual()
                .extension_inputs
                .iter()
                .any(|input| input.system_id == 1)
        );
    }

    #[test]
    fn native_core_belt_candidates_reach_classifier_in_java_item_order_with_grade() {
        let mut visual = visual_with_spots(Vec::new());
        visual.candidates.insert(
            (2, 10, BeamCandidateClass::Standard),
            BeamCandidateOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 100,
                        kind: NeutralBeamInterKind::Beam,
                    })],
                },
                accepted: true,
                error: None,
            },
        );
        let mut step =
            HeadlessBeamsStep::new(visual).with_native_beam_kernel(native_kernel_config(0.08));
        let mut sheet = sheet();
        sheet.systems[0].free_glyphs.push(native_beam_glyph(10));

        assert_eq!(step.build_system_beams(&mut sheet, 0).unwrap(), None);

        assert_eq!(step.visual().candidate_inputs.len(), 1);
        let input = &step.visual().candidate_inputs[0];
        assert_eq!(input.class, BeamCandidateClass::Standard);
        assert_eq!(input.native_candidates.len(), 1);
        let candidate = input.native_candidates[0];
        assert_eq!((candidate.line_index, candidate.item_index), (0, 0));
        assert_eq!(
            (
                candidate.impacts.raster.core_foreground,
                candidate.impacts.raster.core_count,
                candidate.impacts.raster.belt_foreground,
            ),
            (32, 32, 0)
        );
        assert!(candidate.grade >= 0.08);
        assert_eq!(sheet.systems[0].inters[0].id, 100);
    }

    #[test]
    fn native_rejection_skips_standard_then_small_failure_keeps_delta_prefix() {
        let mut visual = visual_with_spots(Vec::new());
        visual.candidates.insert(
            (2, 10, BeamCandidateClass::Small),
            BeamCandidateOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 101,
                        kind: NeutralBeamInterKind::SmallBeam,
                    })],
                },
                accepted: false,
                error: Some("small materializer failed"),
            },
        );
        let mut step =
            HeadlessBeamsStep::new(visual).with_native_beam_kernel(native_kernel_config(2.0));
        let mut sheet = sheet();
        sheet.small_beams_enabled = true;
        sheet.systems[0].free_glyphs.push(native_beam_glyph(10));

        assert_eq!(
            step.build_system_beams(&mut sheet, 0).unwrap(),
            Some("small materializer failed")
        );

        assert_eq!(
            step.visual()
                .candidate_inputs
                .iter()
                .map(|input| input.class)
                .collect::<Vec<_>>(),
            vec![BeamCandidateClass::Small]
        );
        assert_eq!(step.visual().candidate_inputs[0].native_candidates.len(), 1);
        assert_eq!(sheet.systems[0].inters[0].id, 101);
        assert!(step.visual().extension_inputs.is_empty());
    }

    #[test]
    fn native_extension_evidence_is_added_without_taking_mutation_authority() {
        let visual = visual_with_spots(Vec::new());
        let mut config = native_kernel_config(0.08);
        let class = audiveris_image::beam_extension::ExtensionClassParameters {
            impacts: config.standard.impacts,
            minimum_grade: 0.08,
        };
        config.extension_systems.push(NativeBeamExtensionSystem {
            system_id: 2,
            beams: vec![ExtensionBeam {
                id: 100,
                median: audiveris_image::beam_structure::Segment {
                    x1: 10.0,
                    y1: 22.0,
                    x2: 15.0,
                    y2: 22.0,
                },
                height: 4.0,
                distance_impact: 1.0,
                class: audiveris_image::beam_extension::BeamExtensionClass::Standard,
                glyph_id: Some(10),
                removed: false,
            }],
            seeds: vec![ExtensionGlyph {
                id: 50,
                left: 17,
                top: 17,
                width: 1,
                height: 10,
                vertical_median: Some(audiveris_image::beam_structure::Segment {
                    x1: 17.0,
                    y1: 17.0,
                    x2: 17.0,
                    y2: 26.0,
                }),
            }],
            parameters: BeamExtensionParameters {
                standard: class,
                small: None,
                max_side_beam_dx: 12.0,
                min_beams_gap_x: 2.0,
                max_beams_gap_y: 2.0,
                beams_x_margin: 1.0,
                max_extension_to_stem: 8.0,
                max_extension_to_spot: 10.0,
                max_stem_beam_gap_x: 2.0,
                max_stem_beam_gap_y: 2.0,
                min_extension_black_ratio: 0.5,
                min_neighbor_x_overlap: 2.0,
                max_neighbor_y_distance: 8.0,
                max_neighbor_slope_diff: 0.1,
            },
        });
        let mut step = HeadlessBeamsStep::new(visual).with_native_beam_kernel(config);
        let mut sheet = sheet();
        sheet.systems[0].inters.push(NeutralBeamInter {
            id: 100,
            kind: NeutralBeamInterKind::Beam,
        });
        let mut seed = beam_spot_glyph(50, 17, 17);
        seed.groups = vec![NeutralBeamGlyphGroup::VerticalSeed];
        sheet.systems[0].free_glyphs.push(seed);

        assert_eq!(step.build_system_beams(&mut sheet, 0).unwrap(), None);

        let extension = &step.visual().extension_inputs[0];
        assert_eq!(extension.vertical_seed_ids, vec![50]);
        assert_eq!(extension.native_evidence.len(), 1);
        assert_eq!(
            extension.native_evidence[0].mode,
            audiveris_image::beam_extension::BeamExtensionMode::Stem { seed_id: 50 }
        );
        // The injected seam still returned the empty delta, so no replacement
        // beam was materialized merely because native evidence existed.
        assert_eq!(
            sheet.systems[0].inters,
            vec![NeutralBeamInter {
                id: 100,
                kind: NeutralBeamInterKind::Beam,
            }]
        );
    }

    #[test]
    fn native_hook_evidence_reaches_top_then_bottom_injected_seams() {
        let visual = visual_with_spots(Vec::new());
        let mut config = native_kernel_config(0.08);
        config.hook_systems.push(NativeBeamHookSystem {
            system_id: 2,
            beams: vec![ExtensionBeam {
                id: 100,
                median: audiveris_image::beam_structure::Segment {
                    x1: 10.0,
                    y1: 22.0,
                    x2: 15.0,
                    y2: 22.0,
                },
                height: 4.0,
                distance_impact: 1.0,
                class: audiveris_image::beam_extension::BeamExtensionClass::Standard,
                glyph_id: Some(10),
                removed: false,
            }],
            parameters: HookParameters {
                min_hook_width_low: 3.0,
                minimum_grade: 0.08,
                impacts: BeamImpactParameters {
                    min_width_low: 3.0,
                    min_width_high: 8.0,
                    ..config.standard.impacts
                },
            },
        });
        let mut step = HeadlessBeamsStep::new(visual).with_native_beam_kernel(config);
        let mut sheet = sheet();
        sheet.systems[0].inters.push(NeutralBeamInter {
            id: 100,
            kind: NeutralBeamInterKind::Beam,
        });
        sheet.systems[0].free_glyphs.push(NeutralBeamGlyph {
            id: 60,
            top: 14,
            left: 11,
            geometry: Some(NeutralGlyphGeometry {
                width: 4,
                height: 4,
                weight: 16,
                centroid_x: 12.5,
                centroid_y: 15.5,
                rounded_centroid_x: 12,
                rounded_centroid_y: 16,
            }),
            raster: Some(
                RunTable::from_pixels(Orientation::Vertical, 4, 4, &[FOREGROUND; 16]).unwrap(),
            ),
            groups: vec![NeutralBeamGlyphGroup::BeamSpot],
        });

        assert_eq!(step.build_system_beams(&mut sheet, 0).unwrap(), None);

        assert_eq!(step.visual().hook_inputs.len(), 2);
        assert_eq!(step.visual().hook_inputs[0].side, BeamHookSide::Top);
        assert_eq!(step.visual().hook_inputs[0].native_evidence.len(), 1);
        assert_eq!(step.visual().hook_inputs[0].native_evidence[0].glyph_id, 60);
        assert!(step.visual().hook_inputs[1].native_evidence.is_empty());
        // Empty injected deltas leave the source beam untouched.
        assert_eq!(sheet.systems[0].inters[0].id, 100);
    }

    #[test]
    fn checked_beam_failure_keeps_prefix_skips_rests_continues_and_cleans_up() {
        let mut visual = visual_with_spots(Vec::new());
        visual.extensions.insert(
            2,
            BeamStageOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 20,
                        kind: NeutralBeamInterKind::Beam,
                    })],
                },
                error: Some("beam failure"),
            },
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "beam failure")]);
        assert_eq!(
            step.visual().calls,
            vec![
                ("close", 9),
                ("heads", 9),
                ("extend", 2),
                ("extend", 1),
                ("group", 1)
            ]
        );
        assert_eq!(sheet.systems[0].inters[0].id, 20);
        assert!(sheet.systems[1].inters.is_empty());
        assert_eq!(
            sheet.mutations,
            vec![
                BeamsMutation::HeadSpotsSaved,
                BeamsMutation::InterAdded {
                    system_id: 2,
                    inter_id: 20
                },
                BeamsMutation::SystemFailed { system_id: 2 },
            ]
        );
    }

    #[test]
    fn multiple_rest_delta_preserves_registration_add_remove_relation_order() {
        let mut visual = visual_with_spots(Vec::new());
        visual.extensions.insert(
            2,
            BeamStageOutcome::success(BeamSystemDelta {
                mutations: vec![
                    BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 20,
                        kind: NeutralBeamInterKind::Beam,
                    }),
                    BeamSystemMutation::AddBeamGroup(200),
                ],
            }),
        );
        visual.rest_probes.insert(
            20,
            MultipleRestProbeOutcome {
                candidate: Some(MultipleRestCandidate {
                    beam_id: 20,
                    multiple_rest_inter_id: 21,
                    left_peak: MultipleRestPeak { id: 40, width: 3 },
                    right_peak: MultipleRestPeak { id: 41, width: 5 },
                }),
                error: None,
            },
        );
        for (side, glyph_id, serif_id, relation_id) in [
            (BeamHorizontalSide::Left, 70, 22, 300),
            (BeamHorizontalSide::Right, 71, 23, 301),
        ] {
            visual.serifs.insert(
                (21, side),
                MultipleRestSerifOutcome {
                    delta: BeamSystemDelta {
                        mutations: vec![
                            BeamSystemMutation::RegisterGlyph(NeutralBeamGlyph {
                                id: glyph_id,
                                top: 0,
                                left: 0,
                                geometry: None,
                                raster: None,
                                groups: vec![NeutralBeamGlyphGroup::Other],
                            }),
                            BeamSystemMutation::AddInter(NeutralBeamInter {
                                id: serif_id,
                                kind: NeutralBeamInterKind::VerticalSerif,
                            }),
                            BeamSystemMutation::AddRelation(NeutralBeamRelation {
                                id: relation_id,
                                source_inter_id: 21,
                                target_inter_id: serif_id,
                            }),
                        ],
                    },
                    relation_id: Some(relation_id),
                    error: None,
                },
            );
        }
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.systems[0].multiple_rest_evidence = vec![MultipleRestBeamEvidence {
            beam_id: 20,
            width: 40,
            grade: 0.8,
            height: 8.0,
            staff_id: Some(5),
            staff_tablature: false,
            start_pitch: -0.2,
            stop_pitch: 0.2,
        }];
        sheet.systems[0].multiple_rest_sections = vec![
            MultipleRestSection {
                id: 1,
                lag: MultipleRestSectionLag::Vertical,
                horizontal_length: 5,
                x: 30,
                y: 20,
                width: 2,
                height: 10,
            },
            MultipleRestSection {
                id: 2,
                lag: MultipleRestSectionLag::Horizontal,
                horizontal_length: 4,
                x: 20,
                y: 20,
                width: 4,
                height: 2,
            },
            MultipleRestSection {
                id: 3,
                lag: MultipleRestSectionLag::Horizontal,
                horizontal_length: 6,
                x: 10,
                y: 20,
                width: 6,
                height: 2,
            },
        ];

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.registered_glyph_ids, vec![70, 71]);
        assert_eq!(sheet.systems[0].inters[0].id, 21);
        assert_eq!(sheet.systems[0].beam_group_ids, vec![200]);
        assert_eq!(sheet.systems[0].relations[0].id, 300);
        assert_eq!(
            step.visual()
                .serif_inputs
                .iter()
                .map(|input| (input.side, input.eligible_section_ids.clone()))
                .collect::<Vec<_>>(),
            vec![
                (BeamHorizontalSide::Left, vec![2, 1]),
                (BeamHorizontalSide::Right, vec![2, 1]),
            ]
        );
        assert_eq!(
            sheet.systems[0].multiple_rests,
            vec![NeutralMultipleRestRecord {
                inter_id: 21,
                source_beam_id: 20,
                grade: 0.8,
                height: 8.0,
                staff_id: 5,
                left_relation_id: Some(300),
                right_relation_id: Some(301),
            }]
        );
    }

    #[test]
    fn serif_failure_keeps_rest_and_side_prefix_without_deleting_source_beam() {
        let mut visual = visual_with_spots(Vec::new());
        visual.extensions.insert(
            2,
            BeamStageOutcome::success(BeamSystemDelta {
                mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                    id: 20,
                    kind: NeutralBeamInterKind::Beam,
                })],
            }),
        );
        visual.rest_probes.insert(
            20,
            MultipleRestProbeOutcome {
                candidate: Some(MultipleRestCandidate {
                    beam_id: 20,
                    multiple_rest_inter_id: 21,
                    left_peak: MultipleRestPeak { id: 40, width: 3 },
                    right_peak: MultipleRestPeak { id: 41, width: 3 },
                }),
                error: None,
            },
        );
        visual.serifs.insert(
            (21, BeamHorizontalSide::Left),
            MultipleRestSerifOutcome {
                delta: BeamSystemDelta {
                    mutations: vec![BeamSystemMutation::RegisterGlyph(NeutralBeamGlyph {
                        id: 70,
                        top: 0,
                        left: 0,
                        geometry: None,
                        raster: None,
                        groups: vec![NeutralBeamGlyphGroup::Other],
                    })],
                },
                relation_id: None,
                error: Some("filament failed after registration"),
            },
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.systems[0].multiple_rest_evidence = vec![MultipleRestBeamEvidence {
            beam_id: 20,
            width: 50,
            grade: 0.7,
            height: 8.0,
            staff_id: Some(5),
            staff_tablature: false,
            start_pitch: 0.0,
            stop_pitch: 0.0,
        }];

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.system_errors,
            vec![(2, "filament failed after registration")]
        );
        assert_eq!(sheet.registered_glyph_ids, vec![70]);
        assert_eq!(
            sheet.systems[0]
                .inters
                .iter()
                .map(|inter| inter.id)
                .collect::<Vec<_>>(),
            vec![20, 21]
        );
        assert_eq!(step.visual().serif_inputs.len(), 1);
        assert_eq!(step.visual().serif_inputs[0].side, BeamHorizontalSide::Left);
    }

    #[test]
    fn epilog_keeps_beam_spots_that_also_own_another_group() {
        let visual = visual_with_spots(Vec::new());
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.systems[0].free_glyphs.push(NeutralBeamGlyph {
            id: 5,
            top: 0,
            left: 0,
            geometry: None,
            raster: None,
            groups: vec![
                NeutralBeamGlyphGroup::BeamSpot,
                NeutralBeamGlyphGroup::Other,
            ],
        });

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.systems[0].free_glyphs.len(), 1);
    }
}
