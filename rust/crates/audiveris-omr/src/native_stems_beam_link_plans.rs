// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure plans for the beam-origin `VLinker.link` prefix and `VLinker.expand`.
//!
//! Every plan starts from the immutable post-inspection `StemBuilder` product,
//! runs one `stemProfile` against the system's effective `linkProfile`, and
//! stops immediately before `StemBuilder.createStem`. Java's downward
//! expansion aliases and shifts the mutable `VLinker.theoLine`; this boundary
//! does not mutate its predecessor, but exposes the exact stored-line delta a
//! serial scheduler must apply later.

use std::{error::Error, fmt};

use audiveris_core::java_math::java_positive_pow;
use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::Bounds,
};

use crate::{
    beam_recognizer::run_table_center_line,
    native_stem_seeds::NativeStemSeedRecognition,
    native_stems_beam_builders::{
        NativeStemsBeamBuilder, NativeStemsBeamBuilderGlyphRef, NativeStemsBeamBuilderItem,
        NativeStemsBeamBuilderItemKind, NativeStemsBeamBuilderRecognition,
        NativeStemsBeamBuilderSystem, NativeStemsBeamBuilderTargetRef,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamHeadCornerRef, NativeStemsBeamReachabilityRecognition,
        NativeStemsBeamReachabilityTarget,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamSource, NativeStemsBeamStumpBeam,
        NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        NativeStemsBeamVLinkerRef, generic_intersection, relative_ccw,
    },
    native_stems_head_builders::{NativeStemsHeadBuilderRecognition, NativeStemsHeadBuilderSystem},
    native_stems_head_corner_reachability::NativeStemsHeadCornerRef,
    native_stems_head_corners::{
        NativeStemsHeadCorner, NativeStemsHeadCornerHead, NativeStemsHeadCornerRecognition,
        NativeStemsHeadCornerSystem,
    },
    native_stems_head_stumps::{
        NativeStemsHeadStumpBuild, NativeStemsHeadStumpOutcome, NativeStemsHeadStumpRecognition,
        NativeStemsHeadStumpSystem,
    },
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const STANDARD_PROFILE: i32 = 1;
const MIN_LINKER_LENGTH_RATIO: f64 = 0.85;
const HEAD_RELATION_MIN_GRADE: f64 = 0.1;
const HEAD_ANCHOR_HEIGHT_RATIO: f64 = 0.275;

/// All isolated profile plans. The predecessor products remain immutable.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamLinkPlanRecognition {
    pub systems: Vec<NativeStemsBeamLinkPlanSystem>,
    pub builder_count: usize,
    pub attempt_count: usize,
    pub relation_count: usize,
    pub selected_glyph_count: usize,
    pub no_head_target_count: usize,
    pub expand_failed_count: usize,
    pub no_relations_count: usize,
    pub no_glyphs_count: usize,
    pub ready_for_create_stem_count: usize,
    pub relations_past_return_count: usize,
    pub rollback_line_divergence_count: usize,
    pub beam_side_ready_without_stopping_head_count: usize,
    pub beam_side_ready_beyond_stopping_head_count: usize,
    pub beam_side_ready_at_stopping_head_count: usize,
    /// Plans never alter registry, SIG, system stems, or link flags.
    pub forbidden_mutation_count: usize,
    /// Downward Java calls which would shift the aliased stored/theo attachment.
    pub stored_theoretical_line_delta_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamLinkPlanSystem {
    pub system_id: usize,
    pub interline: i32,
    pub link_profile: i32,
    pub min_linker_length: i32,
    pub builders: Vec<NativeStemsBeamLinkPlanBuilder>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamLinkPlanBuilder {
    pub builder_ordinal: usize,
    pub start: NativeStemsBeamVLinkerRef,
    /// `inspectVLinkers` construction maximum (3 for a stump, 4 for a side).
    pub construction_max_profile: i32,
    /// This pure boundary deliberately does not claim which profile the live
    /// scheduler will select. A side decision can depend on a SIG exclusion
    /// edge and canonical glyph object identity not retained upstream.
    pub attempts: Vec<NativeStemsBeamLinkPlanAttempt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamLinkPlanAttempt {
    pub stem_profile: i32,
    pub link_profile: i32,
    pub head_target_count: usize,
    pub initial_stem_line: NativeStemLine,
    pub trace: Vec<NativeStemsBeamExpandStep>,
    /// Latest payload for each C key, in first-insertion order, matching
    /// `LinkedHashMap.put`.
    pub relations: Vec<NativeStemsBeamHeadRelation>,
    /// Structural `LinkedHashSet<Glyph>` order after any asymmetric rollback.
    pub glyphs: Vec<NativeStemsBeamSelectedGlyph>,
    pub expand_last_index: Option<i32>,
    pub stopping_head_item_index: Option<usize>,
    pub stop_cause: Option<NativeStemsBeamExpandStopCause>,
    pub outcome: NativeStemsBeamLinkPlanOutcome,
    pub relations_past_return_count: usize,
    pub rollback_line_diverges_from_restored_glyphs: bool,
    pub beam_side_ready_without_stopping_head: bool,
    pub beam_side_ready_beyond_stopping_head: bool,
    pub beam_side_ready_at_stopping_head: bool,
    pub final_stem_line: NativeStemLine,
    pub stored_theoretical_line_before: NativeStemLine,
    pub stored_theoretical_line_after: NativeStemLine,
    pub stored_theoretical_line_would_mutate: bool,
    pub builder_line_aliases_stored_theoretical_line: bool,
    pub attachment_aliases_stored_theoretical_line: bool,
    pub attachment_alias_would_mutate: bool,
    pub registry_mutation_count: usize,
    pub sig_mutation_count: usize,
    pub system_stem_mutation_count: usize,
    pub link_flag_mutation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamLinkPlanOutcome {
    NoHeadTarget,
    ExpandFailed,
    NoRelations,
    NoGlyphs,
    ReadyForCreateStem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamExpandStopCause {
    CompletedAllItems,
    ShowStoppingGapBeforeHead,
    ShowStoppingGapRestoredHead,
    SeparatedBeforeHead,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamExpandStep {
    pub item_index: usize,
    pub item_kind: NativeStemsBeamBuilderItemKind,
    pub max_y_gap_before: i32,
    pub max_y_gap_after: i32,
    pub relation_count_before: usize,
    pub relation_count_after: usize,
    pub glyph_count_before: usize,
    pub glyph_count_after: usize,
    pub stem_line_before: NativeStemLine,
    pub stem_line_after: NativeStemLine,
    pub gap: Option<NativeStemsBeamGapControl>,
    pub separation: Option<NativeStemsBeamHeadSeparation>,
    pub relation_check: Option<NativeStemsBeamHeadRelationCheck>,
    pub stopping_check: Option<NativeStemsBeamStoppingHeadCheck>,
    pub glyph_update: Option<NativeStemsBeamGlyphUpdate>,
    pub exits_expand: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamGapControl {
    pub contribution: i32,
    pub threshold: i32,
    pub show_stopping: bool,
    pub action: NativeStemsBeamGapAction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamGapAction {
    Continue,
    FailBeforeStoppingHead,
    RestoreStoppingHead {
        item_index: usize,
        restored_glyphs: Vec<NativeStemsBeamSelectedGlyph>,
        /// Java intentionally does not restore either collection.
        relation_count_left_live: usize,
        stem_line_left_shifted: NativeStemLine,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadSeparation {
    pub last_gap_index: Option<usize>,
    pub directed_distance: Option<f64>,
    pub min_linker_length: i32,
    pub close_before_head: bool,
    pub opposite_corner: Option<NativeStemsHeadCornerRef>,
    pub opposite_length: Option<i32>,
    pub opposite_has_concrete_start: Option<bool>,
    pub separated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadRelationCheck {
    pub encountered_corner: NativeStemsBeamHeadCornerRef,
    pub encountered_horizontal: NativeStemHeadSide,
    pub derived_horizontal: NativeStemHeadSide,
    pub horizontal_side_diverges: bool,
    pub vertical: NativeStemVerticalSide,
    pub head_center: (i32, i32),
    pub reference_point: NativeStemPoint,
    pub stump_bounds: Option<Bounds>,
    pub x_stem: f64,
    pub x_gap_pixels: f64,
    pub y_gap_pixels: f64,
    /// Stored relation gaps in interline fractions.
    pub dx: f64,
    pub dy: f64,
    pub horizontal_gap_kind: NativeStemsBeamHorizontalGapKind,
    pub x_maximum: f64,
    pub y_maximum: f64,
    pub x_weight: f64,
    pub y_weight: f64,
    pub raw_x_impact: f64,
    pub raw_y_impact: f64,
    pub x_impact: f64,
    pub y_impact: f64,
    pub grade: f64,
    pub extension_point: Option<NativeStemPoint>,
    pub accepted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHorizontalGapKind {
    In,
    Out,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadRelation {
    pub corner: NativeStemsBeamHeadCornerRef,
    pub map_ordinal: usize,
    pub first_item_index: usize,
    pub latest_item_index: usize,
    pub replaced_existing_payload: bool,
    pub check: NativeStemsBeamHeadRelationCheck,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStoppingHeadCheck {
    pub required_horizontal: NativeStemHeadSide,
    pub relation_horizontal_matches: bool,
    pub glyphs_nonempty: bool,
    pub composite_center_line: Option<NativeStemLine>,
    pub stem_portion: Option<NativeStemsBeamStemPortion>,
    pub is_required_end: bool,
    pub became_stopping_head: bool,
    /// Captured before the encountered C stump is added.
    pub captured_glyphs: Vec<NativeStemsBeamSelectedGlyph>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamStemPortion {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamGlyphUpdate {
    NoGlyph,
    DuplicateStructuralGlyph {
        attempted: NativeStemsBeamBuilderGlyphRef,
        retained: NativeStemsBeamBuilderGlyphRef,
        structural_digest: u64,
    },
    Added {
        glyph: NativeStemsBeamBuilderGlyphRef,
        structural_digest: u64,
        insertion_ordinal: usize,
        composite_bounds: Bounds,
        composite_weight: usize,
        composite_key: NativeStemsBeamLinkGlyphKey,
        composite_digest: u64,
        composite_centroid: NativeStemPoint,
        line_intersection: NativeStemPoint,
        shift_x: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSelectedGlyph {
    pub reference: NativeStemsBeamBuilderGlyphRef,
    pub bounds: Bounds,
    pub weight: usize,
    pub structural_key: NativeStemsBeamLinkGlyphKey,
    pub structural_digest: u64,
}

/// Collision-free Java `Glyph.equals` key: top-left plus exact `RunTable`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamLinkGlyphKey {
    pub left: usize,
    pub top: usize,
    pub run_table: RunTable,
}

#[derive(Debug)]
pub enum NativeStemsBeamLinkPlanError {
    SystemOrder,
    MissingSystemProduct {
        system_id: usize,
        product: &'static str,
    },
    InvalidParameters {
        system_id: usize,
    },
    BuilderInvariant {
        system_id: usize,
        builder_ordinal: usize,
        phase: &'static str,
    },
    MissingBeam {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingGlyph {
        system_id: usize,
        reference: NativeStemsBeamBuilderGlyphRef,
    },
    MissingHead {
        system_id: usize,
        corner: NativeStemsBeamHeadCornerRef,
    },
    MissingHeadBuilder {
        system_id: usize,
        corner: NativeStemsHeadCornerRef,
        profile: i32,
    },
    Geometry {
        system_id: usize,
    },
    RunTable(RunTableError),
}

impl fmt::Display for NativeStemsBeamLinkPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid beam VLinker expand/link plan: {self:?}")
    }
}

impl Error for NativeStemsBeamLinkPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsBeamLinkPlanError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

#[derive(Clone)]
struct ResolvedGlyph {
    reference: NativeStemsBeamBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    structural_digest: u64,
}

impl ResolvedGlyph {
    fn selected(&self) -> NativeStemsBeamSelectedGlyph {
        NativeStemsBeamSelectedGlyph {
            reference: self.reference,
            bounds: self.bounds,
            weight: self.weight,
            structural_key: NativeStemsBeamLinkGlyphKey {
                left: self.bounds.x,
                top: self.bounds.y,
                run_table: self.run_table.clone(),
            },
            structural_digest: self.structural_digest,
        }
    }

    fn structurally_equals(&self, other: &Self) -> bool {
        self.bounds.x == other.bounds.x
            && self.bounds.y == other.bounds.y
            && self.run_table == other.run_table
    }
}

struct SystemContext<'a> {
    system_id: usize,
    seed_system: &'a crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    stump_system: &'a crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    v_system: &'a crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    corner_system: &'a NativeStemsHeadCornerSystem,
    head_stump_system: &'a NativeStemsHeadStumpSystem,
    builder_system: &'a NativeStemsBeamBuilderSystem,
    reach_system: &'a crate::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    head_builder_system: &'a NativeStemsHeadBuilderSystem,
    link_profile: i32,
    min_linker_length: i32,
}

#[derive(Default)]
struct Totals {
    builders: usize,
    attempts: usize,
    relations: usize,
    glyphs: usize,
    no_head: usize,
    expand_failed: usize,
    no_relations: usize,
    no_glyphs: usize,
    ready: usize,
    stored_line_deltas: usize,
    relations_past_return: usize,
    rollback_line_divergences: usize,
    beam_side_without_stopping: usize,
    beam_side_beyond_stopping: usize,
    beam_side_at_stopping: usize,
}

/// Materialize an isolated exact plan for every valid beam-builder stem
/// profile. `linkProfile` is the effective system profile used by Java's live
/// scheduler; variants never feed their would-be `theoLine` shifts into one
/// another.
#[allow(clippy::too_many_arguments)]
pub fn materialize_native_stems_beam_link_plans(
    stem_seeds: &NativeStemSeedRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    head_corners: &NativeStemsHeadCornerRecognition,
    head_stumps: &NativeStemsHeadStumpRecognition,
    reachability: &NativeStemsBeamReachabilityRecognition,
    beam_builders: &NativeStemsBeamBuilderRecognition,
    head_builders: &NativeStemsHeadBuilderRecognition,
) -> Result<NativeStemsBeamLinkPlanRecognition, NativeStemsBeamLinkPlanError> {
    let ids = beam_builders
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let same = |actual: Vec<usize>| actual == ids;
    if !same(
        stem_seeds
            .systems
            .iter()
            .map(|system| system.raw.system_id)
            .collect(),
    ) || !same(
        beam_stumps
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) || !same(
        beam_vlinkers
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) || !same(
        head_corners
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) || !same(
        head_stumps
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) || !same(
        reachability
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) || !same(
        head_builders
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ) {
        return Err(NativeStemsBeamLinkPlanError::SystemOrder);
    }

    let mut totals = Totals::default();
    let mut systems = Vec::with_capacity(ids.len());
    for (index, &system_id) in ids.iter().enumerate() {
        let seed_system = &stem_seeds.systems[index];
        let stump_system = &beam_stumps.systems[index];
        let v_system = &beam_vlinkers.systems[index];
        let corner_system = &head_corners.systems[index];
        let head_stump_system = &head_stumps.systems[index];
        let builder_system = &beam_builders.systems[index];
        let reach_system = &reachability.systems[index];
        let head_builder_system = &head_builders.systems[index];
        let interline = builder_system.interline;
        let link_profile = corner_system.profile;
        if interline <= 0
            || !(0..=4).contains(&link_profile)
            || seed_system.raw.interline != interline
            || stump_system.interline != interline
            || v_system.interline != interline
            || corner_system.interline != interline
            || reach_system.interline != interline
            || head_builder_system.interline != interline
            || stump_system.profile != link_profile
            || v_system.profile != link_profile
            || head_builder_system.system_profile != link_profile
        {
            return Err(NativeStemsBeamLinkPlanError::InvalidParameters { system_id });
        }
        let min_linker_length =
            (f64::from(interline) * MIN_LINKER_LENGTH_RATIO).round_ties_even() as i32;
        let context = SystemContext {
            system_id,
            seed_system,
            stump_system,
            v_system,
            corner_system,
            head_stump_system,
            builder_system,
            reach_system,
            head_builder_system,
            link_profile,
            min_linker_length,
        };
        validate_system(&context)?;

        let mut builders = Vec::with_capacity(builder_system.builders.len());
        for builder in &builder_system.builders {
            let mut attempts = Vec::with_capacity((builder.max_stem_profile + 1) as usize);
            for stem_profile in 0..=builder.max_stem_profile {
                let attempt = materialize_attempt(&context, builder, stem_profile)?;
                totals.attempts += 1;
                totals.relations += attempt.relations.len();
                totals.glyphs += attempt.glyphs.len();
                totals.stored_line_deltas +=
                    usize::from(attempt.stored_theoretical_line_would_mutate);
                totals.relations_past_return += attempt.relations_past_return_count;
                totals.rollback_line_divergences +=
                    usize::from(attempt.rollback_line_diverges_from_restored_glyphs);
                totals.beam_side_without_stopping +=
                    usize::from(attempt.beam_side_ready_without_stopping_head);
                totals.beam_side_beyond_stopping +=
                    usize::from(attempt.beam_side_ready_beyond_stopping_head);
                totals.beam_side_at_stopping +=
                    usize::from(attempt.beam_side_ready_at_stopping_head);
                match attempt.outcome {
                    NativeStemsBeamLinkPlanOutcome::NoHeadTarget => totals.no_head += 1,
                    NativeStemsBeamLinkPlanOutcome::ExpandFailed => totals.expand_failed += 1,
                    NativeStemsBeamLinkPlanOutcome::NoRelations => totals.no_relations += 1,
                    NativeStemsBeamLinkPlanOutcome::NoGlyphs => totals.no_glyphs += 1,
                    NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem => totals.ready += 1,
                }
                attempts.push(attempt);
            }
            totals.builders += 1;
            builders.push(NativeStemsBeamLinkPlanBuilder {
                builder_ordinal: builder.builder_ordinal,
                start: builder.start,
                construction_max_profile: builder.max_stem_profile,
                attempts,
            });
        }
        systems.push(NativeStemsBeamLinkPlanSystem {
            system_id,
            interline,
            link_profile,
            min_linker_length,
            builders,
        });
    }

    Ok(NativeStemsBeamLinkPlanRecognition {
        systems,
        builder_count: totals.builders,
        attempt_count: totals.attempts,
        relation_count: totals.relations,
        selected_glyph_count: totals.glyphs,
        no_head_target_count: totals.no_head,
        expand_failed_count: totals.expand_failed,
        no_relations_count: totals.no_relations,
        no_glyphs_count: totals.no_glyphs,
        ready_for_create_stem_count: totals.ready,
        relations_past_return_count: totals.relations_past_return,
        rollback_line_divergence_count: totals.rollback_line_divergences,
        beam_side_ready_without_stopping_head_count: totals.beam_side_without_stopping,
        beam_side_ready_beyond_stopping_head_count: totals.beam_side_beyond_stopping,
        beam_side_ready_at_stopping_head_count: totals.beam_side_at_stopping,
        forbidden_mutation_count: 0,
        stored_theoretical_line_delta_count: totals.stored_line_deltas,
    })
}

fn validate_system(context: &SystemContext<'_>) -> Result<(), NativeStemsBeamLinkPlanError> {
    let inspections = context
        .reach_system
        .beam_inspections
        .iter()
        .flat_map(|beam| &beam.b_visits)
        .flat_map(|visit| &visit.v_inspections)
        .collect::<Vec<_>>();
    if inspections.len() != context.builder_system.builders.len() {
        return Err(NativeStemsBeamLinkPlanError::MissingSystemProduct {
            system_id: context.system_id,
            product: "beam reachability inspection",
        });
    }
    for (expected_ordinal, (inspection, builder)) in inspections
        .into_iter()
        .zip(&context.builder_system.builders)
        .enumerate()
    {
        let expected_targets = inspection
            .ordered_targets
            .iter()
            .copied()
            .map(|target| match target {
                NativeStemsBeamReachabilityTarget::Beam(reference) => {
                    NativeStemsBeamBuilderTargetRef::Beam(reference)
                }
                NativeStemsBeamReachabilityTarget::Head(reference) => {
                    NativeStemsBeamBuilderTargetRef::Head(reference)
                }
            })
            .collect::<Vec<_>>();
        if builder.builder_ordinal != expected_ordinal
            || builder.start != inspection.reference
            || builder.v_builder_assignment != inspection.reference
            || builder.max_stem_profile != inspection.max_stem_profile
            || builder.v_y_direction != inspection.y_direction
            || builder.theoretical_line != inspection.theoretical_line
            || builder.lookup_quadrilateral != inspection.lookup_quadrilateral
            || builder.lookup_bounds != inspection.lookup_bounds
            || builder.target_input != expected_targets
            || builder.sig_mutation_count != 0
            || builder.system_stem_mutation_count != 0
            || builder.link_mutation_count != 0
        {
            return Err(NativeStemsBeamLinkPlanError::BuilderInvariant {
                system_id: context.system_id,
                builder_ordinal: builder.builder_ordinal,
                phase: "reachability/build",
            });
        }
        if !(0..=4).contains(&builder.max_stem_profile) {
            return Err(NativeStemsBeamLinkPlanError::BuilderInvariant {
                system_id: context.system_id,
                builder_ordinal: builder.builder_ordinal,
                phase: "construction profile",
            });
        }
        for profile in 0..=builder.max_stem_profile {
            if !context.builder_system.gap_map.contains_key(&profile) {
                return Err(NativeStemsBeamLinkPlanError::BuilderInvariant {
                    system_id: context.system_id,
                    builder_ordinal: builder.builder_ordinal,
                    phase: "gap profile",
                });
            }
        }
    }
    Ok(())
}

fn find_b_linker(
    system: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    reference: NativeStemsBeamBLinkerRef,
) -> Option<&NativeStemsBeamBLinker> {
    system
        .constructors
        .iter()
        .find(|constructor| constructor.source == reference.beam)
        .and_then(|constructor| {
            constructor
                .b_linkers
                .iter()
                .find(|linker| linker.reference == reference)
        })
}

fn find_stump_beam(
    system: &crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Option<&NativeStemsBeamStumpBeam> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
}

fn find_head<'a>(
    context: &'a SystemContext<'_>,
    reference: NativeStemsBeamHeadCornerRef,
) -> Result<&'a NativeStemsHeadCornerHead, NativeStemsBeamLinkPlanError> {
    let head = context
        .corner_system
        .heads_in_sig_order
        .get(reference.sig_ordinal)
        .ok_or(NativeStemsBeamLinkPlanError::MissingHead {
            system_id: context.system_id,
            corner: reference,
        })?;
    if head.reference != reference.head
        || context
            .corner_system
            .heads_by_abscissa
            .get(reference.x_ordinal)
            .copied()
            != Some(reference.sig_ordinal)
    {
        return Err(NativeStemsBeamLinkPlanError::MissingHead {
            system_id: context.system_id,
            corner: reference,
        });
    }
    Ok(head)
}

fn find_corner<'a>(
    context: &'a SystemContext<'_>,
    reference: NativeStemsBeamHeadCornerRef,
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> Result<&'a NativeStemsHeadCorner, NativeStemsBeamLinkPlanError> {
    find_head(context, reference)?
        .corners_in_constructor_order
        .iter()
        .find(|corner| corner.horizontal == horizontal && corner.vertical == vertical)
        .ok_or(NativeStemsBeamLinkPlanError::MissingHead {
            system_id: context.system_id,
            corner: reference,
        })
}

fn beam_stump_glyph(
    context: &SystemContext<'_>,
    beam: &NativeStemsBeamStumpBeam,
    stump: &NativeStemsBeamStumpRef,
) -> Result<NativeStemsBeamGlyph, NativeStemsBeamLinkPlanError> {
    match stump {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let glyph = context
                .seed_system
                .free_glyphs
                .get(*free_glyph_ordinal)
                .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference: NativeStemsBeamBuilderGlyphRef::StemSeed {
                        free_glyph_ordinal: *free_glyph_ordinal,
                    },
                })?;
            Ok(NativeStemsBeamGlyph {
                bounds: glyph.bounds,
                weight: glyph.weight,
                run_table: glyph.run_table.clone(),
            })
        }
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => beam
            .sides
            .iter()
            .filter_map(|side| side.build.as_ref())
            .find(|build| {
                build.candidate.is_some()
                    && build.canonical_glyph_index == Some(*canonical_glyph_index)
            })
            .and_then(|build| build.candidate.clone())
            .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                system_id: context.system_id,
                reference: NativeStemsBeamBuilderGlyphRef::BeamStump {
                    b_linker: NativeStemsBeamBLinkerRef {
                        beam: beam.source,
                        id: 0,
                    },
                },
            }),
    }
}

fn glyph_from_head_build(
    context: &SystemContext<'_>,
    reference: NativeStemsBeamBuilderGlyphRef,
    build: &NativeStemsHeadStumpBuild,
) -> Result<ResolvedGlyph, NativeStemsBeamLinkPlanError> {
    let glyph = build
        .candidate
        .as_ref()
        .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
            system_id: context.system_id,
            reference,
        })?;
    resolved_glyph(
        context.system_id,
        reference,
        glyph.bounds,
        glyph.weight,
        glyph.run_table.clone(),
    )
}

fn resolve_glyph(
    context: &SystemContext<'_>,
    builder: &NativeStemsBeamBuilder,
    reference: NativeStemsBeamBuilderGlyphRef,
) -> Result<ResolvedGlyph, NativeStemsBeamLinkPlanError> {
    match reference {
        NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
            let glyph = context
                .seed_system
                .free_glyphs
                .get(free_glyph_ordinal)
                .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference,
                })?;
            resolved_glyph(
                context.system_id,
                reference,
                glyph.bounds,
                glyph.weight,
                glyph.run_table.clone(),
            )
        }
        NativeStemsBeamBuilderGlyphRef::BeamStump { b_linker } => {
            let linker = find_b_linker(context.v_system, b_linker).ok_or(
                NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference,
                },
            )?;
            let stump =
                linker
                    .stump
                    .as_ref()
                    .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                        system_id: context.system_id,
                        reference,
                    })?;
            let beam = find_stump_beam(context.stump_system, b_linker.beam).ok_or(
                NativeStemsBeamLinkPlanError::MissingBeam {
                    system_id: context.system_id,
                    source: b_linker.beam,
                },
            )?;
            let glyph = beam_stump_glyph(context, beam, stump)?;
            resolved_glyph(
                context.system_id,
                reference,
                glyph.bounds,
                glyph.weight,
                glyph.run_table,
            )
        }
        NativeStemsBeamBuilderGlyphRef::HeadStump { corner } => {
            let head = context
                .head_stump_system
                .heads_by_abscissa
                .iter()
                .find(|head| {
                    head.sig_ordinal == corner.sig_ordinal && head.x_ordinal == corner.x_ordinal
                })
                .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference,
                })?;
            let constructor_ordinal =
                find_corner(context, corner, corner.horizontal, corner.vertical)?
                    .constructor_ordinal;
            let stump = head
                .corners_in_constructor_order
                .iter()
                .find(|stump| stump.constructor_ordinal == constructor_ordinal)
                .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference,
                })?;
            match stump.outcome {
                NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal } => {
                    let source = context
                        .seed_system
                        .free_glyphs
                        .get(free_glyph_ordinal)
                        .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                            system_id: context.system_id,
                            reference,
                        })?;
                    resolved_glyph(
                        context.system_id,
                        reference,
                        source.bounds,
                        source.weight,
                        source.run_table.clone(),
                    )
                }
                NativeStemsHeadStumpOutcome::Built { .. } => glyph_from_head_build(
                    context,
                    reference,
                    stump
                        .build
                        .as_ref()
                        .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                            system_id: context.system_id,
                            reference,
                        })?,
                ),
                NativeStemsHeadStumpOutcome::None => {
                    Err(NativeStemsBeamLinkPlanError::MissingGlyph {
                        system_id: context.system_id,
                        reference,
                    })
                }
            }
        }
        NativeStemsBeamBuilderGlyphRef::Chunk { .. } => {
            let registration = builder
                .glyph_registrations
                .iter()
                .find(|registration| registration.glyph == reference)
                .ok_or(NativeStemsBeamLinkPlanError::MissingGlyph {
                    system_id: context.system_id,
                    reference,
                })?;
            resolved_glyph(
                context.system_id,
                reference,
                registration.bounds,
                registration.weight,
                registration.run_table.clone(),
            )
        }
    }
}

fn resolved_glyph(
    system_id: usize,
    reference: NativeStemsBeamBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
) -> Result<ResolvedGlyph, NativeStemsBeamLinkPlanError> {
    if run_table.width() != bounds.width
        || run_table.height() != bounds.height
        || run_table.weight() != weight
        || weight == 0
    {
        return Err(NativeStemsBeamLinkPlanError::Geometry { system_id });
    }
    let structural_digest = structural_digest(bounds, &run_table);
    Ok(ResolvedGlyph {
        reference,
        bounds,
        weight,
        run_table,
        structural_digest,
    })
}

fn structural_digest(bounds: Bounds, run_table: &RunTable) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut put = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    };
    put(bounds.x as u64);
    put(bounds.y as u64);
    put(bounds.width as u64);
    put(bounds.height as u64);
    put(match run_table.orientation() {
        Orientation::Horizontal => 0,
        Orientation::Vertical => 1,
    });
    for sequence in 0..run_table.sequence_count() {
        put(sequence as u64);
        for run in run_table.sequence(sequence).unwrap_or_default() {
            put(run.start as u64);
            put(run.length as u64);
        }
        put(u64::MAX);
    }
    hash
}

struct CompositeGlyph {
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    centroid: NativeStemPoint,
    center_line: Option<NativeStemLine>,
}

fn composite_glyph(
    system_id: usize,
    glyphs: &[ResolvedGlyph],
) -> Result<CompositeGlyph, NativeStemsBeamLinkPlanError> {
    let first = glyphs
        .first()
        .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?;
    if glyphs.len() == 1 {
        let left = i32::try_from(first.bounds.x)
            .map_err(|_| NativeStemsBeamLinkPlanError::Geometry { system_id })?;
        let top = i32::try_from(first.bounds.y)
            .map_err(|_| NativeStemsBeamLinkPlanError::Geometry { system_id })?;
        let mut sum_x = 0_f64;
        let mut sum_y = 0_f64;
        for (x, y) in first.run_table.foreground_points((left, top)) {
            sum_x += f64::from(x);
            sum_y += f64::from(y);
        }
        let center_line = run_table_center_line(&first.run_table, left, top).map(line_from_segment);
        return Ok(CompositeGlyph {
            bounds: first.bounds,
            weight: first.weight,
            run_table: first.run_table.clone(),
            centroid: NativeStemPoint {
                x: sum_x / first.weight as f64,
                y: sum_y / first.weight as f64,
            },
            center_line,
        });
    }
    let mut min_x = first.bounds.x;
    let mut min_y = first.bounds.y;
    let mut max_x = first
        .bounds
        .x
        .checked_add(first.bounds.width)
        .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?;
    let mut max_y = first
        .bounds
        .y
        .checked_add(first.bounds.height)
        .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?;
    for glyph in &glyphs[1..] {
        min_x = min_x.min(glyph.bounds.x);
        min_y = min_y.min(glyph.bounds.y);
        max_x = max_x.max(
            glyph
                .bounds
                .x
                .checked_add(glyph.bounds.width)
                .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?,
        );
        max_y = max_y.max(
            glyph
                .bounds
                .y
                .checked_add(glyph.bounds.height)
                .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?,
        );
    }
    let bounds = Bounds {
        x: min_x,
        y: min_y,
        width: max_x
            .checked_sub(min_x)
            .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?,
        height: max_y
            .checked_sub(min_y)
            .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?,
    };
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(NativeStemsBeamLinkPlanError::Geometry { system_id })?
    ];
    for glyph in glyphs {
        for sequence in 0..glyph.run_table.sequence_count() {
            for run in glyph.run_table.sequence(sequence).unwrap_or_default() {
                for coordinate in run.start..=run.stop() {
                    let (local_x, local_y) = match glyph.run_table.orientation() {
                        Orientation::Horizontal => (coordinate, sequence),
                        Orientation::Vertical => (sequence, coordinate),
                    };
                    let x = glyph.bounds.x - bounds.x + local_x;
                    let y = glyph.bounds.y - bounds.y + local_y;
                    pixels[y * bounds.width + x] = FOREGROUND;
                }
            }
        }
    }
    // `GlyphFactory.buildGlyph` always rebuilds the compound vertically.
    let run_table =
        RunTable::from_pixels(Orientation::Vertical, bounds.width, bounds.height, &pixels)?;
    let weight = run_table.weight();
    if weight == 0 {
        return Err(NativeStemsBeamLinkPlanError::Geometry { system_id });
    }
    let left = i32::try_from(bounds.x)
        .map_err(|_| NativeStemsBeamLinkPlanError::Geometry { system_id })?;
    let top = i32::try_from(bounds.y)
        .map_err(|_| NativeStemsBeamLinkPlanError::Geometry { system_id })?;
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    for (x, y) in run_table.foreground_points((left, top)) {
        sum_x += f64::from(x);
        sum_y += f64::from(y);
    }
    let centroid = NativeStemPoint {
        x: sum_x / weight as f64,
        y: sum_y / weight as f64,
    };
    let center_line = run_table_center_line(&run_table, left, top).map(line_from_segment);
    Ok(CompositeGlyph {
        bounds,
        weight,
        run_table,
        centroid,
        center_line,
    })
}

fn line_from_segment(line: Segment) -> NativeStemLine {
    NativeStemLine {
        start: NativeStemPoint {
            x: line.x1,
            y: line.y1,
        },
        stop: NativeStemPoint {
            x: line.x2,
            y: line.y2,
        },
    }
}

fn segment_from_line(line: NativeStemLine) -> Segment {
    Segment {
        x1: line.start.x,
        y1: line.start.y,
        x2: line.stop.x,
        y2: line.stop.y,
    }
}

fn java_intersection_at_y(line: NativeStemLine, y: f64) -> NativeStemPoint {
    generic_intersection(
        segment_from_line(line),
        Segment {
            x1: 0.0,
            y1: y,
            x2: 1000.0,
            y2: y,
        },
    )
}

fn selected_glyphs(glyphs: &[ResolvedGlyph]) -> Vec<NativeStemsBeamSelectedGlyph> {
    glyphs.iter().map(ResolvedGlyph::selected).collect()
}

fn update_stem_line(
    context: &SystemContext<'_>,
    builder: &NativeStemsBeamBuilder,
    reference: Option<NativeStemsBeamBuilderGlyphRef>,
    glyphs: &mut Vec<ResolvedGlyph>,
    stem_line: &mut NativeStemLine,
) -> Result<NativeStemsBeamGlyphUpdate, NativeStemsBeamLinkPlanError> {
    let Some(reference) = reference else {
        return Ok(NativeStemsBeamGlyphUpdate::NoGlyph);
    };
    let glyph = resolve_glyph(context, builder, reference)?;
    if let Some(retained) = glyphs
        .iter()
        .find(|retained| retained.structurally_equals(&glyph))
    {
        return Ok(NativeStemsBeamGlyphUpdate::DuplicateStructuralGlyph {
            attempted: reference,
            retained: retained.reference,
            structural_digest: glyph.structural_digest,
        });
    }
    glyphs.push(glyph);
    let compound = composite_glyph(context.system_id, glyphs)?;
    let crossing = java_intersection_at_y(*stem_line, compound.centroid.y);
    let shift_x = compound.centroid.x - crossing.x;
    stem_line.start.x += shift_x;
    stem_line.stop.x += shift_x;
    let added = glyphs
        .last()
        .ok_or(NativeStemsBeamLinkPlanError::Geometry {
            system_id: context.system_id,
        })?;
    Ok(NativeStemsBeamGlyphUpdate::Added {
        glyph: reference,
        structural_digest: added.structural_digest,
        insertion_ordinal: glyphs.len() - 1,
        composite_bounds: compound.bounds,
        composite_weight: compound.weight,
        composite_key: NativeStemsBeamLinkGlyphKey {
            left: compound.bounds.x,
            top: compound.bounds.y,
            run_table: compound.run_table.clone(),
        },
        composite_digest: structural_digest(compound.bounds, &compound.run_table),
        composite_centroid: compound.centroid,
        line_intersection: crossing,
        shift_x,
    })
}

fn vertical_direction(side: NativeStemVerticalSide) -> i32 {
    match side {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    }
}

fn opposite_horizontal(side: NativeStemHeadSide) -> NativeStemHeadSide {
    match side {
        NativeStemHeadSide::Left => NativeStemHeadSide::Right,
        NativeStemHeadSide::Right => NativeStemHeadSide::Left,
    }
}

fn required_stopping_side(y_direction: i32) -> NativeStemHeadSide {
    if y_direction < 0 {
        NativeStemHeadSide::Left
    } else {
        NativeStemHeadSide::Right
    }
}

fn head_x_in_max(profile: i32) -> f64 {
    if profile >= 1 { 0.4 } else { 0.2 }
}

fn head_x_out_max(profile: i32) -> f64 {
    if profile >= 2 {
        0.35
    } else if profile >= 1 {
        0.25
    } else {
        0.15
    }
}

fn head_y_gap_max(profile: i32) -> f64 {
    if profile >= 1 { 1.2 } else { 0.8 }
}

#[allow(clippy::manual_clamp)] // Exact GradeUtil two-comparison NaN contract.
fn java_clamp(mut value: f64) -> f64 {
    if value < 0.0 {
        value = 0.0;
    }
    if value > 1.0 {
        value = 1.0;
    }
    value
}

fn support_grade(x_impact: f64, x_weight: f64, y_impact: f64, y_weight: f64) -> f64 {
    if x_impact.is_nan() || y_impact.is_nan() {
        // Java `Math.pow` canonicalizes this path to `Double.NaN`; the
        // resulting relation then fails the minimum-grade comparison.
        return f64::NAN;
    }
    let mut global = 1.0;
    let mut total_weight = 0.0;
    for (impact, weight) in [(x_impact, x_weight), (y_impact, y_weight)] {
        total_weight += weight;
        if impact == 0.0 {
            global = 0.0;
        } else if weight != 0.0 {
            global *= java_positive_pow(impact, weight);
        }
    }
    java_positive_pow(global, 1.0 / total_weight)
}

fn relation_x_gap_pixels(x_direction: i32, x_stem: f64, reference_x: f64) -> f64 {
    f64::from(x_direction) * (x_stem - reference_x)
}

fn stump_y_gap_pixels(overlap: f64) -> f64 {
    // Java `Math.min` propagates NaN, unlike Rust's `f64::min`, which selects
    // the non-NaN operand. Keeping NaN here makes the relation fail through
    // the same grade-comparison path as Java.
    if overlap.is_nan() {
        overlap
    } else {
        overlap.min(0.0).abs()
    }
}

fn check_head_relation(
    context: &SystemContext<'_>,
    builder: &NativeStemsBeamBuilder,
    item: &NativeStemsBeamBuilderItem,
    corner: NativeStemsBeamHeadCornerRef,
    stem_line: NativeStemLine,
) -> Result<NativeStemsBeamHeadRelationCheck, NativeStemsBeamLinkPlanError> {
    let head = find_head(context, corner)?;
    let center = (
        head.bounds.x.wrapping_add(head.bounds.width / 2),
        head.bounds.y.wrapping_add(head.bounds.height / 2),
    );
    let x_direction = -relative_ccw(
        stem_line.start.x,
        stem_line.start.y,
        stem_line.stop.x,
        stem_line.stop.y,
        f64::from(center.0),
        f64::from(center.1),
    );
    let derived_horizontal = if x_direction < 0 {
        NativeStemHeadSide::Left
    } else {
        NativeStemHeadSide::Right
    };
    let dynamic_corner = find_corner(context, corner, derived_horizontal, corner.vertical)?;
    let reference_point = dynamic_corner.reference;
    let x_stem = java_intersection_at_y(stem_line, reference_point.y).x;
    let x_gap_pixels = relation_x_gap_pixels(x_direction, x_stem, reference_point.x);
    // The stump remains the encountered CLinker's stump even when the dynamic
    // relation side differs.
    let stump = item
        .glyph
        .map(|reference| resolve_glyph(context, builder, reference))
        .transpose()?;
    let stump_bounds = stump.as_ref().map(|glyph| glyph.bounds);
    let y_direction = vertical_direction(corner.vertical);
    let y_gap_pixels = if let Some(bounds) = stump_bounds {
        let stump_y =
            i32::try_from(bounds.y).map_err(|_| NativeStemsBeamLinkPlanError::Geometry {
                system_id: context.system_id,
            })?;
        let stump_height =
            i32::try_from(bounds.height).map_err(|_| NativeStemsBeamLinkPlanError::Geometry {
                system_id: context.system_id,
            })?;
        let overlap = if y_direction > 0 {
            f64::from(stump_y.wrapping_add(stump_height)) - stem_line.start.y
        } else {
            stem_line.stop.y - f64::from(stump_y)
        };
        stump_y_gap_pixels(overlap)
    } else if reference_point.y < stem_line.start.y {
        stem_line.start.y - reference_point.y
    } else if reference_point.y > stem_line.stop.y {
        reference_point.y - stem_line.stop.y
    } else {
        0.0
    };
    let dx = x_gap_pixels / f64::from(context.builder_system.interline);
    let dy = y_gap_pixels / f64::from(context.builder_system.interline);
    let (horizontal_gap_kind, x_maximum, x_weight, raw_x_impact) = if dx >= 0.0 {
        let maximum = head_x_out_max(context.link_profile);
        (
            NativeStemsBeamHorizontalGapKind::Out,
            maximum,
            2.0,
            (maximum - dx) / maximum,
        )
    } else {
        let maximum = head_x_in_max(context.link_profile);
        (
            NativeStemsBeamHorizontalGapKind::In,
            maximum,
            1.0,
            (maximum + dx) / maximum,
        )
    };
    let y_maximum = head_y_gap_max(context.link_profile);
    let y_weight = 1.0;
    let raw_y_impact = (y_maximum - dy) / y_maximum;
    let x_impact = java_clamp(raw_x_impact);
    let y_impact = java_clamp(raw_y_impact);
    let grade = support_grade(x_impact, x_weight, y_impact, y_weight);
    let accepted = grade >= HEAD_RELATION_MIN_GRADE;
    let extension_point = accepted.then_some(NativeStemPoint {
        x: x_stem,
        y: if y_direction > 0 {
            f64::from(head.bounds.y)
        } else {
            f64::from(
                head.bounds
                    .y
                    .wrapping_add(head.bounds.height)
                    .wrapping_sub(1),
            )
        },
    });
    Ok(NativeStemsBeamHeadRelationCheck {
        encountered_corner: corner,
        encountered_horizontal: corner.horizontal,
        derived_horizontal,
        horizontal_side_diverges: corner.horizontal != derived_horizontal,
        vertical: corner.vertical,
        head_center: center,
        reference_point,
        stump_bounds,
        x_stem,
        x_gap_pixels,
        y_gap_pixels,
        dx,
        dy,
        horizontal_gap_kind,
        x_maximum,
        y_maximum,
        x_weight,
        y_weight,
        raw_x_impact,
        raw_y_impact,
        x_impact,
        y_impact,
        grade,
        extension_point,
        accepted,
    })
}

fn stem_portion(
    head: &NativeStemsHeadCornerHead,
    stem_line: NativeStemLine,
    extension_y: f64,
) -> NativeStemsBeamStemPortion {
    let margin = f64::from(head.bounds.height) * HEAD_ANCHOR_HEIGHT_RATIO;
    let middle = (stem_line.start.y + stem_line.stop.y) / 2.0;
    if extension_y >= middle {
        if extension_y > stem_line.stop.y - margin {
            NativeStemsBeamStemPortion::Bottom
        } else {
            NativeStemsBeamStemPortion::Middle
        }
    } else if extension_y < stem_line.start.y + margin {
        NativeStemsBeamStemPortion::Top
    } else {
        NativeStemsBeamStemPortion::Middle
    }
}

fn stopping_head_check(
    context: &SystemContext<'_>,
    corner: NativeStemsBeamHeadCornerRef,
    relation: &NativeStemsBeamHeadRelationCheck,
    y_direction: i32,
    glyphs: &[ResolvedGlyph],
) -> Result<NativeStemsBeamStoppingHeadCheck, NativeStemsBeamLinkPlanError> {
    let required_horizontal = required_stopping_side(y_direction);
    let relation_horizontal_matches = relation.derived_horizontal == required_horizontal;
    let glyphs_nonempty = !glyphs.is_empty();
    let mut composite_center_line = None;
    let mut portion = None;
    let mut is_required_end = false;
    if relation_horizontal_matches && glyphs_nonempty {
        let compound = composite_glyph(context.system_id, glyphs)?;
        let center_line = compound
            .center_line
            .ok_or(NativeStemsBeamLinkPlanError::Geometry {
                system_id: context.system_id,
            })?;
        composite_center_line = Some(center_line);
        let value = stem_portion(
            find_head(context, corner)?,
            center_line,
            relation
                .extension_point
                .ok_or(NativeStemsBeamLinkPlanError::Geometry {
                    system_id: context.system_id,
                })?
                .y,
        );
        is_required_end = value
            == if y_direction > 0 {
                NativeStemsBeamStemPortion::Bottom
            } else {
                NativeStemsBeamStemPortion::Top
            };
        portion = Some(value);
    }
    let became_stopping_head = relation_horizontal_matches && glyphs_nonempty && is_required_end;
    Ok(NativeStemsBeamStoppingHeadCheck {
        required_horizontal,
        relation_horizontal_matches,
        glyphs_nonempty,
        composite_center_line,
        stem_portion: portion,
        is_required_end,
        became_stopping_head,
        captured_glyphs: if became_stopping_head {
            selected_glyphs(glyphs)
        } else {
            Vec::new()
        },
    })
}

fn as_head_builder_ref(
    reference: NativeStemsBeamHeadCornerRef,
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> NativeStemsHeadCornerRef {
    NativeStemsHeadCornerRef {
        head: reference.head,
        sig_ordinal: reference.sig_ordinal,
        x_ordinal: reference.x_ordinal,
        horizontal,
        vertical,
    }
}

fn opposite_concrete_start(
    context: &SystemContext<'_>,
    builder: &NativeStemsBeamBuilder,
    corner: NativeStemsBeamHeadCornerRef,
) -> Result<(NativeStemsHeadCornerRef, i32, bool), NativeStemsBeamLinkPlanError> {
    let opposite = as_head_builder_ref(
        corner,
        opposite_horizontal(corner.horizontal),
        builder.start.side,
    );
    let opposite_builder = context
        .head_builder_system
        .builders
        .iter()
        .find(|candidate| candidate.start == opposite)
        .ok_or(NativeStemsBeamLinkPlanError::MissingHeadBuilder {
            system_id: context.system_id,
            corner: opposite,
            profile: context.link_profile,
        })?;
    let length = *opposite_builder.lengths.get(&context.link_profile).ok_or(
        NativeStemsBeamLinkPlanError::MissingHeadBuilder {
            system_id: context.system_id,
            corner: opposite,
            profile: context.link_profile,
        },
    )?;
    Ok((opposite, length, length >= context.min_linker_length))
}

fn insert_relation(
    relations: &mut Vec<NativeStemsBeamHeadRelation>,
    corner: NativeStemsBeamHeadCornerRef,
    item_index: usize,
    check: NativeStemsBeamHeadRelationCheck,
) {
    if let Some(existing) = relations
        .iter_mut()
        .find(|relation| relation.corner == corner)
    {
        existing.latest_item_index = item_index;
        existing.replaced_existing_payload = true;
        existing.check = check;
    } else {
        relations.push(NativeStemsBeamHeadRelation {
            corner,
            map_ordinal: relations.len(),
            first_item_index: item_index,
            latest_item_index: item_index,
            replaced_existing_payload: false,
            check,
        });
    }
}

fn attachment_aliases_stored_line(
    context: &SystemContext<'_>,
    reference: NativeStemsBeamVLinkerRef,
    builder_ordinal: usize,
) -> Result<bool, NativeStemsBeamLinkPlanError> {
    let linker = find_b_linker(context.v_system, reference.b_linker).ok_or(
        NativeStemsBeamLinkPlanError::BuilderInvariant {
            system_id: context.system_id,
            builder_ordinal,
            phase: "attachment B linker",
        },
    )?;
    Ok(linker
        .v_linkers
        .last()
        .is_some_and(|candidate| candidate.reference == reference))
}

fn line_bits_equal(one: NativeStemLine, two: NativeStemLine) -> bool {
    one.start.x.to_bits() == two.start.x.to_bits()
        && one.start.y.to_bits() == two.start.y.to_bits()
        && one.stop.x.to_bits() == two.stop.x.to_bits()
        && one.stop.y.to_bits() == two.stop.y.to_bits()
}

fn classify_link_prefix_outcome(
    expand_last_index: i32,
    relation_count: usize,
    glyph_count: usize,
) -> NativeStemsBeamLinkPlanOutcome {
    if expand_last_index == -1 {
        NativeStemsBeamLinkPlanOutcome::ExpandFailed
    } else if relation_count == 0 {
        NativeStemsBeamLinkPlanOutcome::NoRelations
    } else if glyph_count == 0 {
        NativeStemsBeamLinkPlanOutcome::NoGlyphs
    } else {
        NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
    }
}

fn relation_is_past_return(latest_item_index: usize, expand_last_index: i32) -> bool {
    expand_last_index >= 0
        && i32::try_from(latest_item_index).map_or(true, |index| index > expand_last_index)
}

#[derive(Clone)]
struct StoppingState {
    item_index: usize,
    glyphs: Vec<ResolvedGlyph>,
    stem_line: NativeStemLine,
}

fn materialize_attempt(
    context: &SystemContext<'_>,
    builder: &NativeStemsBeamBuilder,
    stem_profile: i32,
) -> Result<NativeStemsBeamLinkPlanAttempt, NativeStemsBeamLinkPlanError> {
    let stored_before = builder.theoretical_line;
    let initial_stem_line = if builder.v_y_direction > 0 {
        stored_before
    } else {
        NativeStemLine {
            start: stored_before.stop,
            stop: stored_before.start,
        }
    };
    let attachment_aliases =
        attachment_aliases_stored_line(context, builder.start, builder.builder_ordinal)?;
    let head_target_count = builder
        .items
        .iter()
        .filter(|item| matches!(item.target, Some(NativeStemsBeamBuilderTargetRef::Head(_))))
        .count();
    if head_target_count == 0 {
        return Ok(NativeStemsBeamLinkPlanAttempt {
            stem_profile,
            link_profile: context.link_profile,
            head_target_count,
            initial_stem_line,
            trace: Vec::new(),
            relations: Vec::new(),
            glyphs: Vec::new(),
            expand_last_index: None,
            stopping_head_item_index: None,
            stop_cause: None,
            outcome: NativeStemsBeamLinkPlanOutcome::NoHeadTarget,
            relations_past_return_count: 0,
            rollback_line_diverges_from_restored_glyphs: false,
            beam_side_ready_without_stopping_head: false,
            beam_side_ready_beyond_stopping_head: false,
            beam_side_ready_at_stopping_head: false,
            final_stem_line: initial_stem_line,
            stored_theoretical_line_before: stored_before,
            stored_theoretical_line_after: stored_before,
            stored_theoretical_line_would_mutate: false,
            builder_line_aliases_stored_theoretical_line: true,
            attachment_aliases_stored_theoretical_line: attachment_aliases,
            attachment_alias_would_mutate: false,
            registry_mutation_count: 0,
            sig_mutation_count: 0,
            system_stem_mutation_count: 0,
            link_flag_mutation_count: 0,
        });
    }

    let mut max_y_gap = *context.builder_system.gap_map.get(&stem_profile).ok_or(
        NativeStemsBeamLinkPlanError::BuilderInvariant {
            system_id: context.system_id,
            builder_ordinal: builder.builder_ordinal,
            phase: "attempt gap profile",
        },
    )?;
    let standard_max_y_gap = *context
        .builder_system
        .gap_map
        .get(&STANDARD_PROFILE)
        .ok_or(NativeStemsBeamLinkPlanError::BuilderInvariant {
            system_id: context.system_id,
            builder_ordinal: builder.builder_ordinal,
            phase: "standard gap profile",
        })?;
    let mut stem_line = initial_stem_line;
    let mut glyphs = Vec::<ResolvedGlyph>::new();
    let mut relations = Vec::<NativeStemsBeamHeadRelation>::new();
    let mut trace = Vec::with_capacity(builder.items.len());
    let mut stopping = None::<StoppingState>;
    let mut expand_last_index =
        i32::try_from(builder.items.len().saturating_sub(1)).map_err(|_| {
            NativeStemsBeamLinkPlanError::Geometry {
                system_id: context.system_id,
            }
        })?;
    let mut stop_cause = NativeStemsBeamExpandStopCause::CompletedAllItems;
    let mut rollback_line_diverges = false;
    let mut exited = false;

    for (item_index, item) in builder.items.iter().enumerate() {
        let max_y_gap_before = max_y_gap;
        let relation_count_before = relations.len();
        let glyph_count_before = glyphs.len();
        let stem_line_before = stem_line;
        let mut gap_control = None;
        let mut separation = None;
        let mut relation_check = None;
        let mut stopping_check_value = None;
        let mut glyph_update = None;
        let mut exits_expand = false;
        let mut skip_trailing_update = false;

        if item.kind == NativeStemsBeamBuilderItemKind::Gap && item.contribution > max_y_gap {
            if let Some(state) = &stopping {
                glyphs.clone_from(&state.glyphs);
                rollback_line_diverges |= !line_bits_equal(stem_line, state.stem_line);
                expand_last_index = i32::try_from(state.item_index).map_err(|_| {
                    NativeStemsBeamLinkPlanError::Geometry {
                        system_id: context.system_id,
                    }
                })?;
                stop_cause = NativeStemsBeamExpandStopCause::ShowStoppingGapRestoredHead;
                gap_control = Some(NativeStemsBeamGapControl {
                    contribution: item.contribution,
                    threshold: max_y_gap,
                    show_stopping: true,
                    action: NativeStemsBeamGapAction::RestoreStoppingHead {
                        item_index: state.item_index,
                        restored_glyphs: selected_glyphs(&glyphs),
                        relation_count_left_live: relations.len(),
                        stem_line_left_shifted: stem_line,
                    },
                });
            } else {
                expand_last_index = -1;
                stop_cause = NativeStemsBeamExpandStopCause::ShowStoppingGapBeforeHead;
                gap_control = Some(NativeStemsBeamGapControl {
                    contribution: item.contribution,
                    threshold: max_y_gap,
                    show_stopping: true,
                    action: NativeStemsBeamGapAction::FailBeforeStoppingHead,
                });
            }
            exits_expand = true;
        } else {
            if item.kind == NativeStemsBeamBuilderItemKind::Gap {
                gap_control = Some(NativeStemsBeamGapControl {
                    contribution: item.contribution,
                    threshold: max_y_gap,
                    show_stopping: false,
                    action: NativeStemsBeamGapAction::Continue,
                });
            }

            if let Some(NativeStemsBeamBuilderTargetRef::Head(corner)) = item.target {
                if stopping.is_some() {
                    let last_gap = builder.items[..item_index].iter().rposition(|candidate| {
                        candidate.kind == NativeStemsBeamBuilderItemKind::Gap
                    });
                    let mut evidence = NativeStemsBeamHeadSeparation {
                        last_gap_index: last_gap,
                        directed_distance: None,
                        min_linker_length: context.min_linker_length,
                        close_before_head: false,
                        opposite_corner: None,
                        opposite_length: None,
                        opposite_has_concrete_start: None,
                        separated: false,
                    };
                    if let Some(gap_index) = last_gap {
                        let encountered =
                            find_corner(context, corner, corner.horizontal, corner.vertical)?;
                        let gap_line = builder.items[gap_index].line;
                        let distance = if builder.v_y_direction > 0 {
                            encountered.reference.y - gap_line.stop.y
                        } else {
                            gap_line.start.y - encountered.reference.y
                        };
                        evidence.directed_distance = Some(distance);
                        evidence.close_before_head =
                            distance < f64::from(context.min_linker_length);
                        if evidence.close_before_head {
                            let (opposite, length, concrete) =
                                opposite_concrete_start(context, builder, corner)?;
                            evidence.opposite_corner = Some(opposite);
                            evidence.opposite_length = Some(length);
                            evidence.opposite_has_concrete_start = Some(concrete);
                            if concrete {
                                let state = stopping.as_ref().ok_or(
                                    NativeStemsBeamLinkPlanError::BuilderInvariant {
                                        system_id: context.system_id,
                                        builder_ordinal: builder.builder_ordinal,
                                        phase: "stopping state",
                                    },
                                )?;
                                glyphs.clone_from(&state.glyphs);
                                rollback_line_diverges |=
                                    !line_bits_equal(stem_line, state.stem_line);
                                expand_last_index =
                                    i32::try_from(state.item_index).map_err(|_| {
                                        NativeStemsBeamLinkPlanError::Geometry {
                                            system_id: context.system_id,
                                        }
                                    })?;
                                stop_cause = NativeStemsBeamExpandStopCause::SeparatedBeforeHead;
                                evidence.separated = true;
                                exits_expand = true;
                            }
                        }
                    }
                    separation = Some(evidence);
                }

                if !exits_expand {
                    let check = check_head_relation(context, builder, item, corner, stem_line)?;
                    if check.accepted {
                        insert_relation(&mut relations, corner, item_index, check.clone());
                        let checked = stopping_head_check(
                            context,
                            corner,
                            &check,
                            builder.v_y_direction,
                            &glyphs,
                        )?;
                        if checked.became_stopping_head {
                            stopping = Some(StoppingState {
                                item_index,
                                glyphs: glyphs.clone(),
                                stem_line,
                            });
                            max_y_gap = standard_max_y_gap;
                        }
                        stopping_check_value = Some(checked);
                    } else {
                        // Java's `continue` skips the common updateStemLine
                        // tail for a rejected head relation.
                        skip_trailing_update = true;
                    }
                    relation_check = Some(check);
                }
            }

            if !exits_expand && !skip_trailing_update {
                glyph_update = Some(update_stem_line(
                    context,
                    builder,
                    item.glyph,
                    &mut glyphs,
                    &mut stem_line,
                )?);
            }
        }

        trace.push(NativeStemsBeamExpandStep {
            item_index,
            item_kind: item.kind,
            max_y_gap_before,
            max_y_gap_after: max_y_gap,
            relation_count_before,
            relation_count_after: relations.len(),
            glyph_count_before,
            glyph_count_after: glyphs.len(),
            stem_line_before,
            stem_line_after: stem_line,
            gap: gap_control,
            separation,
            relation_check,
            stopping_check: stopping_check_value,
            glyph_update,
            exits_expand,
        });
        if exits_expand {
            exited = true;
            break;
        }
    }

    if !exited {
        stop_cause = NativeStemsBeamExpandStopCause::CompletedAllItems;
    }
    let outcome = classify_link_prefix_outcome(expand_last_index, relations.len(), glyphs.len());
    let stopping_head_item_index = stopping.as_ref().map(|state| state.item_index);
    let relations_past_return_count = relations
        .iter()
        .filter(|relation| relation_is_past_return(relation.latest_item_index, expand_last_index))
        .count();
    let beam_side_ready_without_stopping_head = stem_profile == 4
        && outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        && stopping_head_item_index.is_none();
    let beam_side_ready_beyond_stopping_head = stem_profile == 4
        && outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        && stopping_head_item_index
            .is_some_and(|index| i32::try_from(index) != Ok(expand_last_index));
    let beam_side_ready_at_stopping_head = stem_profile == 4
        && outcome == NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        && stopping_head_item_index.is_some_and(|index| {
            i32::try_from(index).is_ok_and(|index| index == expand_last_index)
        });
    let stored_after = if builder.v_y_direction > 0 {
        stem_line
    } else {
        stored_before
    };
    let stored_would_mutate = !line_bits_equal(stored_before, stored_after);
    Ok(NativeStemsBeamLinkPlanAttempt {
        stem_profile,
        link_profile: context.link_profile,
        head_target_count,
        initial_stem_line,
        trace,
        relations,
        glyphs: selected_glyphs(&glyphs),
        expand_last_index: Some(expand_last_index),
        stopping_head_item_index,
        stop_cause: Some(stop_cause),
        outcome,
        relations_past_return_count,
        rollback_line_diverges_from_restored_glyphs: rollback_line_diverges,
        beam_side_ready_without_stopping_head,
        beam_side_ready_beyond_stopping_head,
        beam_side_ready_at_stopping_head,
        final_stem_line: stem_line,
        stored_theoretical_line_before: stored_before,
        stored_theoretical_line_after: stored_after,
        stored_theoretical_line_would_mutate: stored_would_mutate,
        builder_line_aliases_stored_theoretical_line: true,
        attachment_aliases_stored_theoretical_line: attachment_aliases,
        attachment_alias_would_mutate: attachment_aliases && stored_would_mutate,
        registry_mutation_count: 0,
        sig_mutation_count: 0,
        system_stem_mutation_count: 0,
        link_flag_mutation_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::run_table::Run;

    fn glyph(
        reference: NativeStemsBeamBuilderGlyphRef,
        bounds: Bounds,
        orientation: Orientation,
        rows: &[&[(usize, usize)]],
    ) -> ResolvedGlyph {
        let mut table = RunTable::new(orientation, bounds.width, bounds.height).unwrap();
        for (sequence, runs) in rows.iter().enumerate() {
            for &(start, length) in *runs {
                assert!(table.add_run(sequence, Run::new(start, length)).unwrap());
            }
        }
        resolved_glyph(1, reference, bounds, table.weight(), table).unwrap()
    }

    #[test]
    fn structural_equality_uses_exact_content_not_typed_reference() {
        let bounds = Bounds {
            x: 10,
            y: 20,
            width: 1,
            height: 3,
        };
        let first = glyph(
            NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 1,
            },
            bounds,
            Orientation::Vertical,
            &[&[(0, 3)]],
        );
        let duplicate = glyph(
            NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal: 4,
                filament_ordinal: 2,
            },
            bounds,
            Orientation::Vertical,
            &[&[(0, 3)]],
        );
        assert!(first.structurally_equals(&duplicate));
        assert_ne!(first.reference, duplicate.reference);
    }

    #[test]
    fn single_glyph_composite_preserves_original_orientation() {
        let source = glyph(
            NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 0,
            },
            Bounds {
                x: 3,
                y: 5,
                width: 3,
                height: 1,
            },
            Orientation::Horizontal,
            &[&[(0, 3)]],
        );
        let compound = composite_glyph(1, std::slice::from_ref(&source)).unwrap();
        assert_eq!(compound.run_table, source.run_table);
        assert_eq!(compound.centroid, NativeStemPoint { x: 4.0, y: 5.0 });
    }

    #[test]
    fn multi_glyph_composite_is_vertical_union() {
        let first = glyph(
            NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 0,
            },
            Bounds {
                x: 10,
                y: 10,
                width: 1,
                height: 3,
            },
            Orientation::Vertical,
            &[&[(0, 3)]],
        );
        let second = glyph(
            NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 1,
            },
            Bounds {
                x: 11,
                y: 11,
                width: 1,
                height: 3,
            },
            Orientation::Horizontal,
            &[&[(0, 1)], &[(0, 1)], &[(0, 1)]],
        );
        let compound = composite_glyph(1, &[first, second]).unwrap();
        assert_eq!(
            compound.bounds,
            Bounds {
                x: 10,
                y: 10,
                width: 2,
                height: 4
            }
        );
        assert_eq!(compound.weight, 6);
        assert_eq!(compound.run_table.orientation(), Orientation::Vertical);
        assert_eq!(compound.centroid, NativeStemPoint { x: 10.5, y: 11.5 });
    }

    #[test]
    fn support_grade_has_relation_intrinsic_ratio_one() {
        let grade = support_grade(0.5, 2.0, 0.25, 1.0);
        let expected = java_positive_pow(0.5 * 0.5 * 0.25, 1.0 / 3.0);
        assert_eq!(grade.to_bits(), expected.to_bits());
    }

    #[test]
    fn support_grade_preserves_java_nan_rejection_path() {
        assert_eq!(
            support_grade(f64::NAN, 2.0, 1.0, 1.0).to_bits(),
            f64::NAN.to_bits()
        );
        assert_eq!(
            support_grade(1.0, 2.0, f64::NAN, 1.0).to_bits(),
            f64::NAN.to_bits()
        );
    }

    #[test]
    fn stump_y_gap_preserves_java_math_min_nan_semantics() {
        assert_eq!(stump_y_gap_pixels(f64::NAN).to_bits(), f64::NAN.to_bits());
        assert_eq!(stump_y_gap_pixels(3.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(stump_y_gap_pixels(-3.0).to_bits(), 3.0_f64.to_bits());
    }

    #[test]
    fn one_pixel_composite_supports_centroid_only_update() {
        let source = glyph(
            NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: 0,
            },
            Bounds {
                x: 3,
                y: 5,
                width: 1,
                height: 1,
            },
            Orientation::Vertical,
            &[&[(0, 1)]],
        );
        let compound = composite_glyph(1, std::slice::from_ref(&source)).unwrap();
        assert_eq!(compound.centroid, NativeStemPoint { x: 3.0, y: 5.0 });
        assert_eq!(compound.center_line, None);
    }

    #[test]
    fn collinear_head_uses_zero_x_direction_but_derives_right_side() {
        let line = NativeStemLine {
            start: NativeStemPoint { x: 2.0, y: 1.0 },
            stop: NativeStemPoint { x: 6.0, y: 9.0 },
        };
        let center = NativeStemPoint { x: 4.0, y: 5.0 };
        let x_direction = -relative_ccw(
            line.start.x,
            line.start.y,
            line.stop.x,
            line.stop.y,
            center.x,
            center.y,
        );
        let derived_horizontal = if x_direction < 0 {
            NativeStemHeadSide::Left
        } else {
            NativeStemHeadSide::Right
        };

        assert_eq!(x_direction, 0);
        assert_eq!(derived_horizontal, NativeStemHeadSide::Right);
        assert_eq!(relation_x_gap_pixels(x_direction, 10.0, 7.0).to_bits(), 0);
    }

    #[test]
    fn link_prefix_outcome_precedence_matches_java() {
        assert_eq!(
            classify_link_prefix_outcome(-1, 2, 3),
            NativeStemsBeamLinkPlanOutcome::ExpandFailed
        );
        assert_eq!(
            classify_link_prefix_outcome(4, 0, 3),
            NativeStemsBeamLinkPlanOutcome::NoRelations
        );
        assert_eq!(
            classify_link_prefix_outcome(4, 2, 0),
            NativeStemsBeamLinkPlanOutcome::NoGlyphs
        );
        assert_eq!(
            classify_link_prefix_outcome(4, 2, 3),
            NativeStemsBeamLinkPlanOutcome::ReadyForCreateStem
        );
    }

    #[test]
    fn failed_expand_has_no_relation_past_return() {
        assert!(!relation_is_past_return(0, -1));
        assert!(relation_is_past_return(5, 4));
        assert!(!relation_is_past_return(4, 4));
    }

    #[test]
    fn determinant_intersection_uses_java_horizontal_probe() {
        let line = NativeStemLine {
            start: NativeStemPoint { x: 2.0, y: 1.0 },
            stop: NativeStemPoint { x: 6.0, y: 9.0 },
        };
        let point = java_intersection_at_y(line, 5.0);
        assert_eq!(point.x, 4.0);
        assert_eq!(point.y, 5.0);
    }
}
