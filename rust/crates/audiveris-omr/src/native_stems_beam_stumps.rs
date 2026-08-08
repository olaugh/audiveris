// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact constructor-time stump preparation for Java `BeamLinker`.
//!
//! This boundary runs after HEADS has removed conflicting small beams and
//! after STEMS has purged seeds which overlap connected barlines.  It retains
//! the complete read/mutation evidence of `BeamLinker.retrieveStumps()`:
//! vicinity and seed-area filtering, stable abscissa sorting, the in-place
//! duplicate purge, side classification, section-built side stumps, direction
//! checks against the live beam group, canonical fixed-glyph registration and
//! the final tremolo predicate.  It deliberately stops before `equipStumps`
//! creates any `BLinker` or `VLinker`.

use std::{cmp::Ordering, collections::BTreeSet, error::Error, fmt};

use audiveris_image::{
    beam_structure::{Segment, line_util_intersection},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, Section},
};

use crate::{
    beam_inters::{BeamKind, RegisteredBeamGlyph},
    beam_recognizer::run_table_center_line,
    head_scanner_slices::{JavaRectangle, population_system_area_integer_bounds},
    head_template_overlap::horizontal_parallelogram_intersects_rectangle,
    native_heads::NativeHeadsRecognition,
    native_heads_competitors::NativeHeadsCompetitorSource,
    native_stem_seeds::{
        NativeStemSeedGlyph, NativeStemSeedRecognition, contains_section_centroid,
    },
    native_stems_head_seeds::NativeStemsHeadSeedRecognition,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemVerticalSide},
};

const VICINITY_MARGIN_RATIO: f64 = 1.0;
const MAX_BEAM_SIDE_DX_RATIO: f64 = 0.25;
const MAX_BEAM_SEED_DX_RATIO: f64 = 0.1;
const MAX_BEAM_SEED_DY_RATIO: f64 = 0.25;
const MIN_BEAM_STEMS_DX_RATIO: f64 = 1.0;
const MIN_BEAM_STUMP_DY_RATIO: f64 = 0.5;
const BEAM_PORTION_DX_RATIO: f64 = 0.5;
const TREMOLO_WIDTH_RATIO: f64 = 1.35;
const TREMOLO_WIDTH_MARGIN_RATIO: f64 = 0.25;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpRecognition {
    pub systems: Vec<NativeStemsBeamStumpSystem>,
    pub beam_count: usize,
    pub side_count: usize,
    pub neighbor_seed_count: usize,
    pub intersected_seed_count: usize,
    pub purged_seed_count: usize,
    pub build_attempt_count: usize,
    pub empty_build_count: usize,
    pub direction_rejected_build_count: usize,
    pub build_candidate_count: usize,
    pub rejected_build_count: usize,
    pub accepted_build_count: usize,
    pub new_build_count: usize,
    pub reused_build_count: usize,
    pub tremolo_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpSystem {
    pub system_id: usize,
    pub profile: i32,
    pub interline: i32,
    pub maximum_stem_thickness: i32,
    pub vicinity_margin: i32,
    pub max_beam_side_dx: i32,
    pub max_beam_seed_dx: i32,
    pub min_beam_stems_dx: i32,
    pub min_beam_stump_dy: i32,
    pub system_bounds: JavaRectangle,
    /// Complete system VLAG in persistent lag order.
    pub vertical_section_source_ordinals: Vec<usize>,
    /// Live beams in Java's stable `Inters.byAbscissa` order.
    pub beams_by_abscissa: Vec<NativeStemsBeamStumpBeam>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeStemsBeamSource {
    RawBeam(usize),
    Hook(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpBeam {
    pub x_ordinal: usize,
    /// Ordinal in the live post-HEADS beam-only SIG stream, before x sorting.
    pub sig_ordinal: usize,
    pub source: NativeStemsBeamSource,
    pub kind: BeamKind,
    pub group_ordinal: usize,
    pub bounds: JavaRectangle,
    pub median: Segment,
    pub height: f64,
    pub beam_glyph: NativeStemsBeamGlyph,
    /// `AbstractInter.getProfile()` for a non-manual beam.
    pub beam_profile: i32,
    /// `max(beam.profile, system.profile)` used by `getSeedArea()`.
    pub effective_profile: i32,
    /// Rounded `scale.toPixels(BeamStemRelation.getYGapMaximum(profile))`.
    pub seed_y_gap: i32,
    /// The unrounded vertical expansion on each side of the beam.
    pub seed_area_dy: f64,
    pub seed_area: NativeStemsBeamArea,
    /// Post-noStem seeds in `GlyphGroup.VERTICAL_SEED`/free-glyph order.
    pub neighbor_seed_ordinals: Vec<usize>,
    /// Seed-area hits before Java's stable intersection-x sort.
    pub intersected_seeds: Vec<NativeStemsBeamSeed>,
    /// Same hits after that stable sort and before the duplicate purge.
    pub seeds_before_purge: Vec<usize>,
    pub purge_steps: Vec<NativeStemsBeamSeedPurgeStep>,
    /// Surviving free-glyph ordinals after the duplicate purge.
    pub surviving_seed_ordinals: Vec<usize>,
    /// Always LEFT then RIGHT, Java enum declaration order.
    pub sides: Vec<NativeStemsBeamSide>,
    /// Final stump list after a built left stump is prepended and a built right
    /// stump is appended.
    pub stumps: Vec<NativeStemsBeamStump>,
    pub looks_like_tremolo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamArea {
    pub median: Segment,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSeed {
    pub pre_sort_ordinal: usize,
    pub sorted_ordinal: usize,
    /// Dense ordinal in the post-noStem kept-seed list.
    pub kept_ordinal: usize,
    pub free_glyph_ordinal: usize,
    pub bounds: JavaRectangle,
    pub center_line: Segment,
    pub intersection: (f64, f64),
    pub distance_to_seed_segment_sq: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamSeedPurgeAction {
    BreakAtMinimumDx,
    RemoveFirstForHeight,
    RemoveSecondForHeight,
    RemoveFirstForDistance,
    RemoveSecondForDistance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSeedPurgeStep {
    pub first_index: usize,
    pub second_index: usize,
    pub first_kept_ordinal: usize,
    pub second_kept_ordinal: usize,
    pub first_free_glyph_ordinal: usize,
    pub second_free_glyph_ordinal: usize,
    pub first_intersection_x: f64,
    pub second_intersection_x: f64,
    pub delta_x: f64,
    pub vertical_overlap: i32,
    pub first_height: i32,
    pub second_height: i32,
    pub first_distance_sq: f64,
    pub second_distance_sq: f64,
    pub action: NativeStemsBeamSeedPurgeAction,
    pub remaining_seed_ordinals: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSide {
    pub side: NativeStemHeadSide,
    pub edge_kept_ordinal: Option<usize>,
    pub edge_seed_ordinal: Option<usize>,
    pub edge_intersection_x: Option<f64>,
    pub edge_portion: Option<NativeBeamPortion>,
    pub classified_kept_ordinal: Option<usize>,
    pub classified_seed_ordinal: Option<usize>,
    pub build: Option<NativeStemsBeamSideBuild>,
    pub final_stump: Option<NativeStemsBeamStumpRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSideBuild {
    pub area: NativeStemsBeamArea,
    pub reference_x: f64,
    pub sections: Vec<NativeStemsBeamSection>,
    pub steps: Vec<NativeStemsBeamSectionStep>,
    pub compound_weight: usize,
    pub compound_bounds: Option<Bounds>,
    pub candidate: Option<NativeStemsBeamGlyph>,
    pub directions: Option<NativeStemsBeamDirectionEvidence>,
    pub canonical_glyph_index: Option<usize>,
    pub registration: Option<NativeStemsBeamRegistration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSection {
    pub pre_sort_ordinal: usize,
    pub sorted_ordinal: usize,
    /// Ordinal within the complete system-dispatched VLAG.
    pub source_ordinal: usize,
    pub bounds: Bounds,
    pub weight: usize,
    pub first_pos: usize,
    pub run_count: usize,
    pub area_center: (usize, usize),
    pub distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSectionStep {
    pub sorted_ordinal: usize,
    pub source_ordinal: usize,
    pub after_add_width: usize,
    pub too_wide: bool,
    pub removed: bool,
    pub breaks: bool,
    pub final_weight: usize,
    pub final_bounds: Option<Bounds>,
    /// Members in Java `Section.byFullAbscissa` order.
    pub member_source_ordinals: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamGlyph {
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
}

impl NativeStemsBeamGlyph {
    #[must_use]
    pub fn run_count(&self) -> usize {
        (0..self.run_table.sequence_count())
            .map(|index| self.run_table.sequence(index).map_or(0, <[_]>::len))
            .sum()
    }

    #[must_use]
    pub fn run_digest(&self) -> u64 {
        fixed_run_digest(&self.run_table)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamDirectionEvidence {
    pub stump_center: (f64, f64),
    pub stump_center_line: Segment,
    pub siblings: Vec<NativeStemsBeamSibling>,
    pub top_extreme: Option<NativeStemsBeamSource>,
    pub bottom_extreme: Option<NativeStemsBeamSource>,
    pub beam_is_extreme: bool,
    pub beam_glyph_is_top_extreme: bool,
    pub beam_glyph_is_bottom_extreme: bool,
    pub beam_glyph_is_extreme: bool,
    pub top_border_y: Option<f64>,
    pub bottom_border_y: Option<f64>,
    pub top_dy: Option<f64>,
    pub bottom_dy: Option<f64>,
    /// `None` is Java's null result for no siblings or an interior beam.
    pub directions: Option<Vec<NativeStemVerticalSide>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamSibling {
    pub source: NativeStemsBeamSource,
    pub cross: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStump {
    pub list_ordinal: usize,
    pub reference: NativeStemsBeamStumpRef,
    pub side: Option<NativeStemHeadSide>,
    /// The second `getStumpDirections` call made by `equipStumps`.
    pub directions: NativeStemsBeamDirectionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamStumpRef {
    Seed {
        kept_ordinal: usize,
        free_glyph_ordinal: usize,
        canonical_glyph_index: usize,
    },
    Built {
        canonical_glyph_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamRegistration {
    New { origin: NativeStemsBeamGlyphOrigin },
    Reused { origin: NativeStemsBeamGlyphOrigin },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamGlyphOrigin {
    StemSeed {
        system_id: usize,
        registered_glyph_ordinal: usize,
    },
    RawBeam {
        source_ordinal: usize,
    },
    Hook {
        source_ordinal: usize,
    },
    BeamSideBuild {
        system_id: usize,
        beam_x_ordinal: usize,
        side: NativeStemHeadSide,
    },
}

#[derive(Debug)]
pub enum NativeStemsBeamStumpError {
    SystemOrder,
    MissingSystemArea(usize),
    MissingBeamGroupSystem(usize),
    MissingBeamGroup {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    InvalidBeamGroupMember {
        system_id: usize,
        member_ordinal: usize,
    },
    InvalidBeamSource {
        system_id: usize,
        source: NativeHeadsCompetitorSource,
    },
    InvalidParameters {
        system_id: usize,
        profile: i32,
        interline: i32,
        maximum_stem_thickness: i32,
    },
    MissingSeedGlyph {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingSeedCenterLine {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingCanonicalSeed {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingStumpCenterLine,
    InvalidGeometry,
    RunTable(RunTableError),
}

impl fmt::Display for NativeStemsBeamStumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder => formatter.write_str("STEMS beam-stump system order differs"),
            Self::MissingSystemArea(system_id) => {
                write!(
                    formatter,
                    "STEMS beam-stump system {system_id} has no GRID area"
                )
            }
            Self::MissingBeamGroupSystem(system_id) => write!(
                formatter,
                "STEMS beam-stump system {system_id} has no BEAMS group state"
            ),
            Self::MissingBeamGroup { system_id, source } => write!(
                formatter,
                "STEMS beam-stump {source:?} in system {system_id} has no live group"
            ),
            Self::InvalidBeamGroupMember {
                system_id,
                member_ordinal,
            } => write!(
                formatter,
                "STEMS beam-stump system {system_id} group names invalid member {member_ordinal}"
            ),
            Self::InvalidBeamSource { system_id, source } => write!(
                formatter,
                "STEMS beam-stump system {system_id} has invalid live source {source:?}"
            ),
            Self::InvalidParameters {
                system_id,
                profile,
                interline,
                maximum_stem_thickness,
            } => write!(
                formatter,
                "STEMS beam-stump system {system_id} has profile {profile}, interline {interline}, maximum stem {maximum_stem_thickness}"
            ),
            Self::MissingSeedGlyph {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS beam-stump system {system_id} has no free seed {free_glyph_ordinal}"
            ),
            Self::MissingSeedCenterLine {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS beam-stump system {system_id} seed {free_glyph_ordinal} has no center line"
            ),
            Self::MissingCanonicalSeed {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS beam-stump system {system_id} seed {free_glyph_ordinal} is absent from the canonical glyph arena"
            ),
            Self::MissingStumpCenterLine => {
                formatter.write_str("STEMS beam-stump fixed glyph has no center line")
            }
            Self::InvalidGeometry => formatter.write_str("invalid STEMS beam-stump geometry"),
            Self::RunTable(source) => write!(formatter, "invalid beam-stump run table: {source}"),
        }
    }
}

impl Error for NativeStemsBeamStumpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunTable(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsBeamStumpError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

/// Materialize every live `BeamLinker` constructor through stump preparation.
pub fn materialize_native_stems_beam_stumps(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    heads: &NativeHeadsRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    head_seeds: &NativeStemsHeadSeedRecognition,
) -> Result<NativeStemsBeamStumpRecognition, NativeStemsBeamStumpError> {
    let ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if ids
        != heads
            .epilog
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>()
        || ids
            != stem_seeds
                .systems
                .iter()
                .map(|system| system.raw.system_id)
                .collect::<Vec<_>>()
        || ids
            != head_seeds
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
    {
        return Err(NativeStemsBeamStumpError::SystemOrder);
    }

    let mut registry = initial_registry(stem_seeds, beams)?;
    let mut systems = Vec::with_capacity(ids.len());
    let mut totals = [0_usize; 12];
    for ((head_system, seed_system), purge_system) in heads
        .epilog
        .systems
        .iter()
        .zip(&stem_seeds.systems)
        .zip(&head_seeds.systems)
    {
        let system_id = head_system.system_id;
        let profile = seed_system.raw.profile;
        let interline = seed_system.raw.interline;
        let maximum_stem_thickness = stem_seeds.maximum_stem_thickness;
        if profile < 0 || interline <= 0 || maximum_stem_thickness <= 0 {
            return Err(NativeStemsBeamStumpError::InvalidParameters {
                system_id,
                profile,
                interline,
                maximum_stem_thickness,
            });
        }
        let system_area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(NativeStemsBeamStumpError::MissingSystemArea(system_id))?;
        // `getNeighboringSeeds` uses `SystemInfo.getBounds()`, whose current
        // implementation returns `system.area.getBounds()`. Keep this lookup
        // explicit: staff-extreme `grid.system_bounds` is a different box.
        let system_bounds = population_system_area_integer_bounds(system_area);
        let vertical_section_source_ordinals = grid
            .peak_graph
            .vertical_sections
            .iter()
            .enumerate()
            .filter_map(|(ordinal, section)| {
                let (x, y) = section.centroid();
                contains_section_centroid(system_area, x as f64, y as f64).then_some(ordinal)
            })
            .collect::<Vec<_>>();
        let group_state = beams
            .group_memberships
            .iter()
            .find(|group| group.system_id == system_id)
            .ok_or(NativeStemsBeamStumpError::MissingBeamGroupSystem(system_id))?;
        let mut live_beams = live_beams_for_system(beams, head_system)?;
        let live_sources = live_beams
            .iter()
            .map(|beam| beam.source)
            .collect::<BTreeSet<_>>();
        let group_members = group_members_for_system(beams, system_id)?;
        let live_groups = group_state
            .groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|&ordinal| {
                        group_members.get(ordinal).copied().ok_or(
                            NativeStemsBeamStumpError::InvalidBeamGroupMember {
                                system_id,
                                member_ordinal: ordinal,
                            },
                        )
                    })
                    .filter_map(|result| match result {
                        Ok(source) if live_sources.contains(&source) => Some(Ok(source)),
                        Ok(_) => None,
                        Err(error) => Some(Err(error)),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        for beam in &mut live_beams {
            beam.group_ordinal = live_groups
                .iter()
                .position(|group| group.contains(&beam.source))
                .ok_or(NativeStemsBeamStumpError::MissingBeamGroup {
                    system_id,
                    source: beam.source,
                })?;
        }
        live_beams.sort_by_key(|beam| beam.bounds.x);

        let vicinity_margin = to_pixels(interline, VICINITY_MARGIN_RATIO);
        let max_beam_side_dx = to_pixels(interline, MAX_BEAM_SIDE_DX_RATIO);
        let max_beam_seed_dx = to_pixels(interline, MAX_BEAM_SEED_DX_RATIO);
        let min_beam_stems_dx = to_pixels(interline, MIN_BEAM_STEMS_DX_RATIO);
        let min_beam_stump_dy = to_pixels(interline, MIN_BEAM_STUMP_DY_RATIO);
        let kept_seeds = kept_seeds(system_id, seed_system, purge_system, &registry)?;
        let mut system_beams = Vec::with_capacity(live_beams.len());
        for (x_ordinal, beam) in live_beams.iter().enumerate() {
            let context = BeamContext {
                system_id,
                profile,
                interline,
                maximum_stem_thickness,
                vicinity_margin,
                max_beam_side_dx,
                max_beam_seed_dx,
                min_beam_stems_dx,
                min_beam_stump_dy,
                system_bounds,
                system_section_ordinals: &vertical_section_source_ordinals,
                live_beams: &live_beams,
                live_group: &live_groups[beam.group_ordinal],
            };
            let result = build_beam_stumps(
                grid,
                beams,
                beam,
                x_ordinal,
                &kept_seeds,
                &context,
                &mut registry,
            )?;
            totals[0] += 1;
            totals[1] += 2;
            totals[2] += result.neighbor_seed_ordinals.len();
            totals[3] += result.intersected_seeds.len();
            totals[4] += result
                .purge_steps
                .iter()
                .filter(|step| {
                    !matches!(
                        step.action,
                        NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx
                    )
                })
                .count();
            for build in result.sides.iter().filter_map(|side| side.build.as_ref()) {
                totals[5] += 1;
                if build.candidate.is_none() {
                    totals[6] += 1;
                } else if build.registration.is_none() {
                    totals[7] += 1;
                } else {
                    totals[8] += 1;
                    if matches!(
                        build.registration,
                        Some(NativeStemsBeamRegistration::New { .. })
                    ) {
                        totals[9] += 1;
                    } else {
                        totals[10] += 1;
                    }
                }
            }
            if result.looks_like_tremolo {
                totals[11] += 1;
            }
            system_beams.push(result);
        }
        systems.push(NativeStemsBeamStumpSystem {
            system_id,
            profile,
            interline,
            maximum_stem_thickness,
            vicinity_margin,
            max_beam_side_dx,
            max_beam_seed_dx,
            min_beam_stems_dx,
            min_beam_stump_dy,
            system_bounds,
            vertical_section_source_ordinals,
            beams_by_abscissa: system_beams,
        });
    }

    Ok(NativeStemsBeamStumpRecognition {
        systems,
        beam_count: totals[0],
        side_count: totals[1],
        neighbor_seed_count: totals[2],
        intersected_seed_count: totals[3],
        purged_seed_count: totals[4],
        build_attempt_count: totals[5],
        empty_build_count: totals[6],
        direction_rejected_build_count: totals[7],
        build_candidate_count: totals[7] + totals[8],
        rejected_build_count: totals[7],
        accepted_build_count: totals[8],
        new_build_count: totals[9],
        reused_build_count: totals[10],
        tremolo_count: totals[11],
    })
}

#[derive(Clone)]
struct LiveBeam {
    sig_ordinal: usize,
    source: NativeStemsBeamSource,
    kind: BeamKind,
    group_ordinal: usize,
    bounds: JavaRectangle,
    median: Segment,
    height: f64,
    glyph: NativeStemsBeamGlyph,
}

struct KeptSeed {
    kept_ordinal: usize,
    free_glyph_ordinal: usize,
    bounds: JavaRectangle,
    center_line: Segment,
    canonical_glyph_index: usize,
}

struct BeamContext<'a> {
    system_id: usize,
    profile: i32,
    interline: i32,
    maximum_stem_thickness: i32,
    vicinity_margin: i32,
    max_beam_side_dx: i32,
    max_beam_seed_dx: i32,
    min_beam_stems_dx: i32,
    min_beam_stump_dy: i32,
    system_bounds: JavaRectangle,
    system_section_ordinals: &'a [usize],
    live_beams: &'a [LiveBeam],
    live_group: &'a [NativeStemsBeamSource],
}

#[derive(Clone)]
struct RegistryEntry {
    glyph: NativeStemsBeamGlyph,
    origin: NativeStemsBeamGlyphOrigin,
}

fn initial_registry(
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
) -> Result<Vec<RegistryEntry>, NativeStemsBeamStumpError> {
    let mut registry = Vec::new();
    for system in &stem_seeds.systems {
        for (ordinal, glyph) in system.registered_glyphs.iter().enumerate() {
            insert_existing(
                &mut registry,
                stem_glyph(glyph),
                NativeStemsBeamGlyphOrigin::StemSeed {
                    system_id: system.raw.system_id,
                    registered_glyph_ordinal: ordinal,
                },
            );
        }
    }
    for (ordinal, (_, glyph)) in beams.raw_beam_glyphs.iter().enumerate() {
        insert_existing(
            &mut registry,
            beam_glyph(glyph)?,
            NativeStemsBeamGlyphOrigin::RawBeam {
                source_ordinal: ordinal,
            },
        );
    }
    for (ordinal, (_, glyph)) in beams.hook_glyphs.iter().enumerate() {
        insert_existing(
            &mut registry,
            beam_glyph(glyph)?,
            NativeStemsBeamGlyphOrigin::Hook {
                source_ordinal: ordinal,
            },
        );
    }
    Ok(registry)
}

fn insert_existing(
    registry: &mut Vec<RegistryEntry>,
    glyph: NativeStemsBeamGlyph,
    origin: NativeStemsBeamGlyphOrigin,
) {
    if !registry.iter().any(|entry| entry.glyph == glyph) {
        registry.push(RegistryEntry { glyph, origin });
    }
}

fn register_candidate(
    registry: &mut Vec<RegistryEntry>,
    glyph: &NativeStemsBeamGlyph,
    origin: NativeStemsBeamGlyphOrigin,
) -> (usize, NativeStemsBeamRegistration) {
    if let Some((index, entry)) = registry
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.glyph == *glyph)
    {
        return (
            index,
            NativeStemsBeamRegistration::Reused {
                origin: entry.origin.clone(),
            },
        );
    }
    let index = registry.len();
    registry.push(RegistryEntry {
        glyph: glyph.clone(),
        origin: origin.clone(),
    });
    (index, NativeStemsBeamRegistration::New { origin })
}

fn live_beams_for_system(
    beams: &NativeBeamRecognition,
    system: &crate::native_heads_epilog::NativeHeadsEpilogSystem,
) -> Result<Vec<LiveBeam>, NativeStemsBeamStumpError> {
    let mut result = Vec::new();
    for ((provenance, beam), &removed) in system
        .small_beams
        .beam_provenance
        .iter()
        .zip(&system.small_beams.beam_inputs)
        .zip(&system.small_beams.arbitration.beam_removed)
    {
        if removed || beam.removed {
            continue;
        }
        let source = match provenance.source {
            NativeHeadsCompetitorSource::RawBeam(ordinal) => {
                NativeStemsBeamSource::RawBeam(ordinal)
            }
            NativeHeadsCompetitorSource::Hook(ordinal) => NativeStemsBeamSource::Hook(ordinal),
            source => {
                return Err(NativeStemsBeamStumpError::InvalidBeamSource {
                    system_id: system.system_id,
                    source,
                });
            }
        };
        let (owner, raw, glyph) = resolve_source(beams, source)?;
        if owner != system.system_id {
            return Err(NativeStemsBeamStumpError::InvalidBeamSource {
                system_id: system.system_id,
                source: provenance.source,
            });
        }
        result.push(LiveBeam {
            // `sig.inters(AbstractBeamInter.class)` has already omitted
            // removed vertices before the probe assigns its beam-only ordinal.
            sig_ordinal: result.len(),
            source,
            kind: raw.kind,
            group_ordinal: usize::MAX,
            bounds: beam.bounds,
            median: beam.median,
            height: beam.height,
            glyph: beam_glyph(glyph)?,
        });
    }
    Ok(result)
}

fn group_members_for_system(
    beams: &NativeBeamRecognition,
    system_id: usize,
) -> Result<Vec<NativeStemsBeamSource>, NativeStemsBeamStumpError> {
    Ok(beams
        .raw_beams
        .iter()
        .enumerate()
        .filter_map(|(ordinal, (owner, _))| {
            (*owner == system_id).then_some(NativeStemsBeamSource::RawBeam(ordinal))
        })
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter_map(|(ordinal, (owner, _))| {
                    (*owner == system_id).then_some(NativeStemsBeamSource::Hook(ordinal))
                }),
        )
        .collect())
}

fn resolve_source(
    beams: &NativeBeamRecognition,
    source: NativeStemsBeamSource,
) -> Result<(usize, &crate::beam_inters::RawBeam, &RegisteredBeamGlyph), NativeStemsBeamStumpError>
{
    let pair = match source {
        NativeStemsBeamSource::RawBeam(ordinal) => beams
            .raw_beams
            .get(ordinal)
            .zip(beams.raw_beam_glyphs.get(ordinal)),
        NativeStemsBeamSource::Hook(ordinal) => {
            beams.hooks.get(ordinal).zip(beams.hook_glyphs.get(ordinal))
        }
    }
    .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
    if pair.0.0 != pair.1.0 {
        return Err(NativeStemsBeamStumpError::InvalidGeometry);
    }
    Ok((pair.0.0, &pair.0.1, &pair.1.1))
}

fn kept_seeds(
    system_id: usize,
    system: &crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    purge: &crate::native_stems_head_seeds::NativeStemsHeadSeedSystem,
    registry: &[RegistryEntry],
) -> Result<Vec<KeptSeed>, NativeStemsBeamStumpError> {
    purge
        .kept_seed_ordinals
        .iter()
        .enumerate()
        .map(|(kept_ordinal, &free_glyph_ordinal)| {
            let glyph = system.free_glyphs.get(free_glyph_ordinal).ok_or(
                NativeStemsBeamStumpError::MissingSeedGlyph {
                    system_id,
                    free_glyph_ordinal,
                },
            )?;
            let bounds = java_bounds(glyph.bounds)?;
            let center_line = run_table_center_line(&glyph.run_table, bounds.x, bounds.y).ok_or(
                NativeStemsBeamStumpError::MissingSeedCenterLine {
                    system_id,
                    free_glyph_ordinal,
                },
            )?;
            let canonical_glyph_index = registry
                .iter()
                .position(|registered| {
                    registered.glyph.bounds == glyph.bounds
                        && registered.glyph.run_table == glyph.run_table
                })
                .ok_or(NativeStemsBeamStumpError::MissingCanonicalSeed {
                    system_id,
                    free_glyph_ordinal,
                })?;
            Ok(KeptSeed {
                kept_ordinal,
                free_glyph_ordinal,
                bounds,
                center_line,
                canonical_glyph_index,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_beam_stumps(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    beam: &LiveBeam,
    x_ordinal: usize,
    kept_seeds: &[KeptSeed],
    context: &BeamContext<'_>,
    registry: &mut Vec<RegistryEntry>,
) -> Result<NativeStemsBeamStumpBeam, NativeStemsBeamStumpError> {
    let vicinity = JavaRectangle::new(
        beam.bounds.x.wrapping_sub(context.vicinity_margin),
        context.system_bounds.y,
        beam.bounds
            .width
            .wrapping_add(context.vicinity_margin.wrapping_mul(2)),
        context.system_bounds.height,
    );
    let neighbors = kept_seeds
        .iter()
        .filter(|seed| vicinity.intersects(seed.bounds))
        .collect::<Vec<_>>();
    let neighbor_seed_ordinals = neighbors
        .iter()
        .map(|seed| seed.free_glyph_ordinal)
        .collect::<Vec<_>>();

    let slope = beam.median.slope();
    let dx = f64::from(context.max_beam_seed_dx);
    let beam_profile = 0;
    let effective_profile = beam_profile.max(context.profile);
    let y_gap = to_pixels(context.interline, beam_y_gap_max(effective_profile));
    let dy = MAX_BEAM_SEED_DY_RATIO * f64::from(y_gap);
    let seed_area = NativeStemsBeamArea {
        median: Segment {
            x1: beam.median.x1 - dx,
            y1: beam.median.y1 - (slope * dx),
            x2: beam.median.x2 + dx,
            y2: beam.median.y2 + (slope * dx),
        },
        height: beam.height + (2.0 * dy),
    };
    let mut seed_rows = neighbors
        .into_iter()
        .filter(|seed| {
            horizontal_parallelogram_intersects_rectangle(
                seed_area.median,
                seed_area.height,
                seed.bounds,
            )
        })
        .enumerate()
        .map(|(pre_sort_ordinal, seed)| {
            let intersection = line_intersection(seed.center_line, beam.median);
            NativeStemsBeamSeed {
                pre_sort_ordinal,
                sorted_ordinal: 0,
                kept_ordinal: seed.kept_ordinal,
                free_glyph_ordinal: seed.free_glyph_ordinal,
                bounds: seed.bounds,
                center_line: seed.center_line,
                intersection,
                distance_to_seed_segment_sq: point_segment_distance_sq(
                    seed.center_line,
                    intersection,
                ),
            }
        })
        .collect::<Vec<_>>();
    seed_rows.sort_by(|left, right| left.intersection.0.total_cmp(&right.intersection.0));
    for (ordinal, seed) in seed_rows.iter_mut().enumerate() {
        seed.sorted_ordinal = ordinal;
    }
    let seeds_before_purge = seed_rows
        .iter()
        .map(|seed| seed.free_glyph_ordinal)
        .collect::<Vec<_>>();
    let (survivors, purge_steps) = purge_seeds(seed_rows.clone(), context.min_beam_stems_dx);
    let surviving_seed_ordinals = survivors
        .iter()
        .map(|seed| seed.free_glyph_ordinal)
        .collect::<Vec<_>>();

    let max_portion_dx = to_pixels(context.interline, BEAM_PORTION_DX_RATIO);
    let mut side_seed = [None, None];
    let mut sides = Vec::with_capacity(2);
    for (side_index, side) in [NativeStemHeadSide::Left, NativeStemHeadSide::Right]
        .into_iter()
        .enumerate()
    {
        let edge = if side == NativeStemHeadSide::Left {
            survivors.first()
        } else {
            survivors.last()
        };
        let edge_portion =
            edge.map(|seed| beam_portion(seed.intersection.0, beam.median, max_portion_dx));
        let classified = edge.and_then(|seed| {
            (beam_portion_side(edge_portion.expect("edge portion exists")) == Some(side))
                .then_some(seed.free_glyph_ordinal)
        });
        side_seed[side_index] = classified;
        sides.push(NativeStemsBeamSide {
            side,
            edge_kept_ordinal: edge.map(|seed| seed.kept_ordinal),
            edge_seed_ordinal: edge.map(|seed| seed.free_glyph_ordinal),
            edge_intersection_x: edge.map(|seed| seed.intersection.0),
            edge_portion,
            classified_kept_ordinal: classified.and_then(|free_glyph_ordinal| {
                kept_seeds
                    .iter()
                    .find(|seed| seed.free_glyph_ordinal == free_glyph_ordinal)
                    .map(|seed| seed.kept_ordinal)
            }),
            classified_seed_ordinal: classified,
            build: None,
            final_stump: classified.map(|free_glyph_ordinal| {
                let seed = kept_seeds
                    .iter()
                    .find(|seed| seed.free_glyph_ordinal == free_glyph_ordinal)
                    .expect("surviving seed came from kept seed input");
                NativeStemsBeamStumpRef::Seed {
                    kept_ordinal: seed.kept_ordinal,
                    free_glyph_ordinal,
                    canonical_glyph_index: seed.canonical_glyph_index,
                }
            }),
        });
    }

    for side_index in 0..2 {
        if side_seed[side_index].is_some() {
            continue;
        }
        let side = sides[side_index].side;
        let mut build = build_side_stump(grid, beams, beam, side, context)?;
        if let (Some(candidate), Some(directions)) = (&build.candidate, &build.directions)
            && directions
                .directions
                .as_ref()
                .is_some_and(|directions| !directions.is_empty())
        {
            let origin = NativeStemsBeamGlyphOrigin::BeamSideBuild {
                system_id: context.system_id,
                beam_x_ordinal: x_ordinal,
                side,
            };
            let (canonical_glyph_index, registration) =
                register_candidate(registry, candidate, origin);
            build.canonical_glyph_index = Some(canonical_glyph_index);
            build.registration = Some(registration);
            sides[side_index].final_stump = Some(NativeStemsBeamStumpRef::Built {
                canonical_glyph_index,
            });
        }
        sides[side_index].build = Some(build);
    }

    let mut final_refs = survivors
        .iter()
        .map(|seed| {
            let kept = kept_seeds
                .iter()
                .find(|kept| kept.free_glyph_ordinal == seed.free_glyph_ordinal)
                .expect("surviving seed came from kept seed input");
            NativeStemsBeamStumpRef::Seed {
                kept_ordinal: kept.kept_ordinal,
                free_glyph_ordinal: seed.free_glyph_ordinal,
                canonical_glyph_index: kept.canonical_glyph_index,
            }
        })
        .collect::<Vec<_>>();
    if sides[0].classified_seed_ordinal.is_none()
        && let Some(reference) = sides[0].final_stump.clone()
    {
        final_refs.insert(0, reference);
    }
    if sides[1].classified_seed_ordinal.is_none()
        && let Some(reference) = sides[1].final_stump.clone()
    {
        final_refs.push(reference);
    }

    let mut stumps = Vec::with_capacity(final_refs.len());
    for (list_ordinal, reference) in final_refs.into_iter().enumerate() {
        let glyph = glyph_for_reference(&reference, registry)?;
        // Java scans the EnumMap in LEFT, RIGHT order and overwrites `hSide`
        // on every identity match, so an alias on both sides resolves RIGHT.
        let side = sides
            .iter()
            .filter_map(|side| (side.final_stump.as_ref() == Some(&reference)).then_some(side.side))
            .next_back();
        let directions = stump_directions(grid, beams, beam, glyph, context)?;
        stumps.push(NativeStemsBeamStump {
            list_ordinal,
            reference,
            side,
            directions,
        });
    }
    let beam_width = beam.median.x2 - beam.median.x1;
    let looks_like_tremolo = tremolo_width_gate(
        stumps.len(),
        sides
            .iter()
            .filter(|side| side.final_stump.is_some())
            .count(),
        beam_width,
        context.interline,
    );

    Ok(NativeStemsBeamStumpBeam {
        x_ordinal,
        sig_ordinal: beam.sig_ordinal,
        source: beam.source,
        kind: beam.kind,
        group_ordinal: beam.group_ordinal,
        bounds: beam.bounds,
        median: beam.median,
        height: beam.height,
        beam_glyph: beam.glyph.clone(),
        beam_profile,
        effective_profile,
        seed_y_gap: y_gap,
        seed_area_dy: dy,
        seed_area,
        neighbor_seed_ordinals,
        intersected_seeds: seed_rows,
        seeds_before_purge,
        purge_steps,
        surviving_seed_ordinals,
        sides,
        stumps,
        looks_like_tremolo,
    })
}

fn purge_seeds(
    mut seeds: Vec<NativeStemsBeamSeed>,
    min_beam_stems_dx: i32,
) -> (Vec<NativeStemsBeamSeed>, Vec<NativeStemsBeamSeedPurgeStep>) {
    let mut steps = Vec::new();
    let mut first = 0_usize;
    'next_seed: while first < seeds.len() {
        let second = first + 1;
        while second < seeds.len() {
            let one = &seeds[first];
            let two = &seeds[second];
            let delta_x = two.intersection.0 - one.intersection.0;
            let overlap = y_overlap(one.bounds, two.bounds);
            let action = if delta_x >= f64::from(min_beam_stems_dx) {
                NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx
            } else if overlap > 0 {
                if one.bounds.height >= two.bounds.height {
                    NativeStemsBeamSeedPurgeAction::RemoveSecondForHeight
                } else {
                    NativeStemsBeamSeedPurgeAction::RemoveFirstForHeight
                }
            } else if one.distance_to_seed_segment_sq <= two.distance_to_seed_segment_sq {
                NativeStemsBeamSeedPurgeAction::RemoveSecondForDistance
            } else {
                NativeStemsBeamSeedPurgeAction::RemoveFirstForDistance
            };
            let first_kept_ordinal = one.kept_ordinal;
            let second_kept_ordinal = two.kept_ordinal;
            let first_ordinal = one.free_glyph_ordinal;
            let second_ordinal = two.free_glyph_ordinal;
            let first_x = one.intersection.0;
            let second_x = two.intersection.0;
            let first_height = one.bounds.height;
            let second_height = two.bounds.height;
            let first_distance_sq = one.distance_to_seed_segment_sq;
            let second_distance_sq = two.distance_to_seed_segment_sq;
            match action {
                NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx => {}
                NativeStemsBeamSeedPurgeAction::RemoveFirstForHeight
                | NativeStemsBeamSeedPurgeAction::RemoveFirstForDistance => {
                    seeds.remove(first);
                }
                NativeStemsBeamSeedPurgeAction::RemoveSecondForHeight
                | NativeStemsBeamSeedPurgeAction::RemoveSecondForDistance => {
                    seeds.remove(second);
                }
            }
            steps.push(NativeStemsBeamSeedPurgeStep {
                first_index: first,
                second_index: second,
                first_kept_ordinal,
                second_kept_ordinal,
                first_free_glyph_ordinal: first_ordinal,
                second_free_glyph_ordinal: second_ordinal,
                first_intersection_x: first_x,
                second_intersection_x: second_x,
                delta_x,
                vertical_overlap: overlap,
                first_height,
                second_height,
                first_distance_sq,
                second_distance_sq,
                action,
                remaining_seed_ordinals: seeds.iter().map(|seed| seed.free_glyph_ordinal).collect(),
            });
            match action {
                NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx => break,
                NativeStemsBeamSeedPurgeAction::RemoveFirstForHeight
                | NativeStemsBeamSeedPurgeAction::RemoveFirstForDistance => {
                    continue 'next_seed;
                }
                NativeStemsBeamSeedPurgeAction::RemoveSecondForHeight
                | NativeStemsBeamSeedPurgeAction::RemoveSecondForDistance => {}
            }
            // Removal shifted the next candidate into this same position.
            if second >= seeds.len() {
                break;
            }
        }
        first += 1;
    }
    (seeds, steps)
}

fn build_side_stump(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    beam: &LiveBeam,
    side: NativeStemHeadSide,
    context: &BeamContext<'_>,
) -> Result<NativeStemsBeamSideBuild, NativeStemsBeamStumpError> {
    let direction = horizontal_direction(side);
    let side_x = if direction < 0 {
        beam.median.x1
    } else {
        beam.median.x2
    };
    let inner_x = side_x - f64::from(direction * context.maximum_stem_thickness);
    // Java `intersectionAtX` returns the determinant-computed abscissa as
    // well as ordinate. Its x can differ from the query in the low bits.
    let inner = line_util_intersection(
        beam.median.x1,
        beam.median.y1,
        beam.median.x2,
        beam.median.y2,
        inner_x,
    );
    let area = NativeStemsBeamArea {
        median: if direction < 0 {
            Segment {
                x1: beam.median.x1,
                y1: beam.median.y1,
                x2: inner.0,
                y2: inner.1,
            }
        } else {
            Segment {
                x1: inner.0,
                y1: inner.1,
                x2: beam.median.x2,
                y2: beam.median.y2,
            }
        },
        height: beam.height,
    };
    let reference_x = side_x - f64::from(direction * context.maximum_stem_thickness) / 2.0;
    let mut candidates = context
        .system_section_ordinals
        .iter()
        .enumerate()
        .filter_map(|(source_ordinal, &global_ordinal)| {
            let section = &grid.peak_graph.vertical_sections[global_ordinal];
            section_intersects_area(section, area).then(|| {
                let bounds = section.bounds();
                let area_center = (
                    bounds.x + (bounds.width / 2),
                    bounds.y + (bounds.height / 2),
                );
                SectionCandidate {
                    pre_sort_ordinal: 0,
                    source_ordinal,
                    global_ordinal,
                    area_center,
                    distance: (area_center.0 as f64 - reference_x).abs(),
                }
            })
        })
        .collect::<Vec<_>>();
    for (ordinal, candidate) in candidates.iter_mut().enumerate() {
        candidate.pre_sort_ordinal = ordinal;
    }
    candidates.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    let sections = candidates
        .iter()
        .enumerate()
        .map(|(sorted_ordinal, candidate)| {
            let section = &grid.peak_graph.vertical_sections[candidate.global_ordinal];
            NativeStemsBeamSection {
                pre_sort_ordinal: candidate.pre_sort_ordinal,
                sorted_ordinal,
                source_ordinal: candidate.source_ordinal,
                bounds: section.bounds(),
                weight: section.weight(),
                first_pos: section.first_pos(),
                run_count: section.run_count(),
                area_center: candidate.area_center,
                distance: candidate.distance,
            }
        })
        .collect::<Vec<_>>();
    let mut build = NativeStemsBeamSideBuild {
        area,
        reference_x,
        sections,
        steps: Vec::new(),
        compound_weight: 0,
        compound_bounds: None,
        candidate: None,
        directions: None,
        canonical_glyph_index: None,
        registration: None,
    };
    if candidates.is_empty() {
        return Ok(build);
    }

    let mut members = Vec::new();
    for (sorted_ordinal, candidate) in candidates.iter().enumerate() {
        members.push(candidate.global_ordinal);
        sort_source_members(&mut members, &grid.peak_graph.vertical_sections);
        let after_add = compound_bounds(&members, &grid.peak_graph.vertical_sections)
            .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
        let too_wide = after_add.width > context.maximum_stem_thickness as usize;
        if too_wide {
            members.retain(|&ordinal| ordinal != candidate.global_ordinal);
        }
        let final_weight = members
            .iter()
            .map(|&ordinal| grid.peak_graph.vertical_sections[ordinal].weight())
            .sum();
        let final_bounds = compound_bounds(&members, &grid.peak_graph.vertical_sections);
        build.steps.push(NativeStemsBeamSectionStep {
            sorted_ordinal,
            source_ordinal: candidate.source_ordinal,
            after_add_width: after_add.width,
            too_wide,
            removed: too_wide,
            breaks: too_wide,
            final_weight,
            final_bounds,
            member_source_ordinals: members
                .iter()
                .map(|&global| {
                    context
                        .system_section_ordinals
                        .iter()
                        .position(|&ordinal| ordinal == global)
                        .expect("compound member belongs to system VLAG")
                })
                .collect(),
        });
        if too_wide {
            break;
        }
    }
    build.compound_weight = members
        .iter()
        .map(|&ordinal| grid.peak_graph.vertical_sections[ordinal].weight())
        .sum();
    build.compound_bounds = compound_bounds(&members, &grid.peak_graph.vertical_sections);
    if build.compound_weight == 0 {
        return Ok(build);
    }
    let candidate = materialize_compound(&members, &grid.peak_graph.vertical_sections)?;
    let directions = stump_directions(grid, beams, beam, &candidate, context)?;
    build.candidate = Some(candidate);
    build.directions = Some(directions);
    Ok(build)
}

#[derive(Clone)]
struct SectionCandidate {
    pre_sort_ordinal: usize,
    source_ordinal: usize,
    global_ordinal: usize,
    area_center: (usize, usize),
    distance: f64,
}

fn stump_directions(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    beam: &LiveBeam,
    stump: &NativeStemsBeamGlyph,
    context: &BeamContext<'_>,
) -> Result<NativeStemsBeamDirectionEvidence, NativeStemsBeamStumpError> {
    let stump_center = (
        stump.bounds.x as f64 + stump.bounds.width as f64 / 2.0,
        stump.bounds.y as f64 + stump.bounds.height as f64 / 2.0,
    );
    let stump_center_line = run_table_center_line(
        &stump.run_table,
        i32::try_from(stump.bounds.x).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        i32::try_from(stump.bounds.y).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
    )
    .ok_or(NativeStemsBeamStumpError::MissingStumpCenterLine)?;
    let vertical = Segment {
        x1: stump_center.0,
        y1: stump_center.1,
        x2: stump_center.0 - (1_000.0 * grid.global_slope),
        y2: stump_center.1 + 1_000.0,
    };
    let mut siblings = context
        .live_group
        .iter()
        .filter_map(|&source| {
            let sibling = context
                .live_beams
                .iter()
                .find(|beam| beam.source == source)?;
            let cross = line_intersection(vertical, sibling.median);
            (sibling.median.x1 - f64::from(context.max_beam_side_dx) <= cross.0
                && cross.0 <= sibling.median.x2 + f64::from(context.max_beam_side_dx))
            .then_some(NativeStemsBeamSibling { source, cross })
        })
        .collect::<Vec<_>>();
    siblings.sort_by(|left, right| left.cross.1.total_cmp(&right.cross.1));
    let top_extreme = siblings.first().map(|sibling| sibling.source);
    let bottom_extreme = siblings.last().map(|sibling| sibling.source);
    let beam_is_extreme = top_extreme == Some(beam.source) || bottom_extreme == Some(beam.source);
    let same_beam_glyph = |source| {
        resolve_source(beams, source)
            .ok()
            .and_then(|(_, _, glyph)| beam_glyph(glyph).ok())
            .is_some_and(|glyph| glyph == beam.glyph)
    };
    let beam_glyph_is_top_extreme = top_extreme.is_some_and(same_beam_glyph);
    let beam_glyph_is_bottom_extreme = bottom_extreme.is_some_and(same_beam_glyph);
    let beam_glyph_is_extreme = beam_glyph_is_top_extreme || beam_glyph_is_bottom_extreme;
    let mut evidence = NativeStemsBeamDirectionEvidence {
        stump_center,
        stump_center_line,
        siblings,
        top_extreme,
        bottom_extreme,
        beam_is_extreme,
        beam_glyph_is_top_extreme,
        beam_glyph_is_bottom_extreme,
        beam_glyph_is_extreme,
        top_border_y: None,
        bottom_border_y: None,
        top_dy: None,
        bottom_dy: None,
        directions: None,
    };
    let (Some(top_source), Some(bottom_source)) = (top_extreme, bottom_extreme) else {
        return Ok(evidence);
    };
    if !beam_is_extreme && !beam_glyph_is_extreme {
        return Ok(evidence);
    }
    let top_beam = context
        .live_beams
        .iter()
        .find(|beam| beam.source == top_source)
        .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
    let bottom_beam = context
        .live_beams
        .iter()
        .find(|beam| beam.source == bottom_source)
        .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
    let x = stump_center.0;
    // `getBorder` translates both endpoint ordinates first; `LineUtil.yAtX`
    // then runs its determinant formula on that translated line.
    let top_border = Segment {
        x1: top_beam.median.x1,
        y1: top_beam.median.y1 - (top_beam.height / 2.0),
        x2: top_beam.median.x2,
        y2: top_beam.median.y2 - (top_beam.height / 2.0),
    };
    let bottom_border = Segment {
        x1: bottom_beam.median.x1,
        y1: bottom_beam.median.y1 + (bottom_beam.height / 2.0),
        x2: bottom_beam.median.x2,
        y2: bottom_beam.median.y2 + (bottom_beam.height / 2.0),
    };
    let top_border_y = top_border.y_at_x(x);
    let bottom_border_y = bottom_border.y_at_x(x);
    let top_dy = 0_f64.max(top_border_y - stump_center_line.y1);
    let bottom_dy = 0_f64.max(stump_center_line.y2 - bottom_border_y);
    let mut directions = Vec::new();
    if top_dy >= f64::from(context.min_beam_stump_dy) {
        directions.push(NativeStemVerticalSide::Top);
    }
    if bottom_dy >= f64::from(context.min_beam_stump_dy) {
        directions.push(NativeStemVerticalSide::Bottom);
    }
    evidence.top_border_y = Some(top_border_y);
    evidence.bottom_border_y = Some(bottom_border_y);
    evidence.top_dy = Some(top_dy);
    evidence.bottom_dy = Some(bottom_dy);
    evidence.directions = Some(directions);
    Ok(evidence)
}

fn glyph_for_reference<'a>(
    reference: &NativeStemsBeamStumpRef,
    registry: &'a [RegistryEntry],
) -> Result<&'a NativeStemsBeamGlyph, NativeStemsBeamStumpError> {
    match reference {
        NativeStemsBeamStumpRef::Seed {
            canonical_glyph_index,
            ..
        }
        | NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => registry
            .get(*canonical_glyph_index)
            .map(|entry| &entry.glyph)
            .ok_or(NativeStemsBeamStumpError::InvalidGeometry),
    }
}

fn beam_portion(x: f64, median: Segment, max_dx: i32) -> NativeBeamPortion {
    if x < median.x1 + f64::from(max_dx) {
        NativeBeamPortion::Left
    } else if x > median.x2 - f64::from(max_dx) {
        NativeBeamPortion::Right
    } else {
        NativeBeamPortion::Center
    }
}

fn beam_portion_side(portion: NativeBeamPortion) -> Option<NativeStemHeadSide> {
    match portion {
        NativeBeamPortion::Left => Some(NativeStemHeadSide::Left),
        NativeBeamPortion::Right => Some(NativeStemHeadSide::Right),
        NativeBeamPortion::Center => None,
    }
}

fn tremolo_width_gate(
    stump_count: usize,
    side_stump_count: usize,
    beam_width: f64,
    interline: i32,
) -> bool {
    stump_count == 1
        && side_stump_count == 0
        && (beam_width - (f64::from(interline) * TREMOLO_WIDTH_RATIO)).abs()
            <= f64::from(interline) * TREMOLO_WIDTH_MARGIN_RATIO
}

fn horizontal_direction(side: NativeStemHeadSide) -> i32 {
    match side {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    }
}

fn beam_y_gap_max(profile: i32) -> f64 {
    match profile {
        0 => 0.8,
        1 => 1.2,
        2 => 2.0,
        _ => 4.0,
    }
}

fn to_pixels(interline: i32, ratio: f64) -> i32 {
    (f64::from(interline) * ratio).round_ties_even() as i32
}

fn y_overlap(one: JavaRectangle, two: JavaRectangle) -> i32 {
    let common_top = one.y.max(two.y);
    let common_bottom = one
        .y
        .wrapping_add(one.height)
        .min(two.y.wrapping_add(two.height));
    common_bottom.wrapping_sub(common_top)
}

fn line_intersection(one: Segment, two: Segment) -> (f64, f64) {
    let denominator =
        ((one.x1 - one.x2) * (two.y1 - two.y2)) - ((one.y1 - one.y2) * (two.x1 - two.x2));
    let one_cross = (one.x1 * one.y2) - (one.y1 * one.x2);
    let two_cross = (two.x1 * two.y2) - (two.y1 * two.x2);
    (
        ((one_cross * (two.x1 - two.x2)) - ((one.x1 - one.x2) * two_cross)) / denominator,
        ((one_cross * (two.y1 - two.y2)) - ((one.y1 - one.y2) * two_cross)) / denominator,
    )
}

/// `Line2D.ptSegDistSq`, preserving its endpoint clamping arithmetic.
fn point_segment_distance_sq(line: Segment, point: (f64, f64)) -> f64 {
    let mut x = point.0 - line.x1;
    let mut y = point.1 - line.y1;
    let px = line.x2 - line.x1;
    let py = line.y2 - line.y1;
    let dot = (x * px) + (y * py);
    let projection_sq = if dot <= 0.0 {
        0.0
    } else {
        x = px - x;
        y = py - y;
        let dot_after = (x * px) + (y * py);
        if dot_after <= 0.0 {
            0.0
        } else {
            dot_after * dot_after / ((px * px) + (py * py))
        }
    };
    let mut distance = (x * x) + (y * y) - projection_sq;
    if distance < 0.0 {
        distance = 0.0;
    }
    distance
}

fn java_bounds(bounds: Bounds) -> Result<JavaRectangle, NativeStemsBeamStumpError> {
    Ok(JavaRectangle::new(
        i32::try_from(bounds.x).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        i32::try_from(bounds.y).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        i32::try_from(bounds.width).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        i32::try_from(bounds.height).map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
    ))
}

fn stem_glyph(glyph: &NativeStemSeedGlyph) -> NativeStemsBeamGlyph {
    NativeStemsBeamGlyph {
        bounds: glyph.bounds,
        weight: glyph.weight,
        run_table: glyph.run_table.clone(),
    }
}

fn beam_glyph(
    glyph: &RegisteredBeamGlyph,
) -> Result<NativeStemsBeamGlyph, NativeStemsBeamStumpError> {
    let bounds = Bounds {
        x: usize::try_from(glyph.bounds.x)
            .map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        y: usize::try_from(glyph.bounds.y)
            .map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        width: usize::try_from(glyph.bounds.width)
            .map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
        height: usize::try_from(glyph.bounds.height)
            .map_err(|_| NativeStemsBeamStumpError::InvalidGeometry)?,
    };
    Ok(NativeStemsBeamGlyph {
        bounds,
        weight: glyph.weight(),
        run_table: glyph.run_table.clone(),
    })
}

fn section_intersects_area(section: &Section, area: NativeStemsBeamArea) -> bool {
    section.runs().iter().enumerate().any(|(offset, run)| {
        let position = section.first_pos() + offset;
        let (x, y, width, height) = match section.orientation() {
            Orientation::Horizontal => (run.start, position, run.length, 1),
            Orientation::Vertical => (position, run.start, 1, run.length),
        };
        let Ok(rectangle) = java_bounds(Bounds {
            x,
            y,
            width,
            height,
        }) else {
            return false;
        };
        horizontal_parallelogram_intersects_rectangle(area.median, area.height, rectangle)
    })
}

fn sort_source_members(members: &mut [usize], sections: &[Section]) {
    members.sort_by(|&left, &right| full_abscissa_cmp(&sections[left], &sections[right]));
}

fn full_abscissa_cmp(left: &Section, right: &Section) -> Ordering {
    let left_bounds = left.bounds();
    let right_bounds = right.bounds();
    left_bounds
        .x
        .cmp(&right_bounds.x)
        .then_with(|| left_bounds.y.cmp(&right_bounds.y))
        .then_with(|| left.id().cmp(&right.id()))
}

fn compound_bounds(members: &[usize], sections: &[Section]) -> Option<Bounds> {
    let mut bounds = members.iter().map(|&ordinal| sections[ordinal].bounds());
    let mut union = bounds.next()?;
    for bounds in bounds {
        let x = union.x.min(bounds.x);
        let y = union.y.min(bounds.y);
        let right = union
            .x
            .checked_add(union.width)?
            .max(bounds.x.checked_add(bounds.width)?);
        let bottom = union
            .y
            .checked_add(union.height)?
            .max(bounds.y.checked_add(bounds.height)?);
        union = Bounds {
            x,
            y,
            width: right.checked_sub(x)?,
            height: bottom.checked_sub(y)?,
        };
    }
    Some(union)
}

fn materialize_compound(
    members: &[usize],
    sections: &[Section],
) -> Result<NativeStemsBeamGlyph, NativeStemsBeamStumpError> {
    let bounds =
        compound_bounds(members, sections).ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?
    ];
    for &ordinal in members {
        for (offset, run) in sections[ordinal].runs().iter().enumerate() {
            let x = sections[ordinal].first_pos() + offset;
            for y in run.start..=run.stop() {
                let relative_x = x
                    .checked_sub(bounds.x)
                    .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
                let relative_y = y
                    .checked_sub(bounds.y)
                    .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
                let index = relative_y
                    .checked_mul(bounds.width)
                    .and_then(|row| row.checked_add(relative_x))
                    .filter(|&index| index < pixels.len())
                    .ok_or(NativeStemsBeamStumpError::InvalidGeometry)?;
                pixels[index] = FOREGROUND;
            }
        }
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let run_table = RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?;
    let weight = pixels.iter().filter(|&&pixel| pixel == FOREGROUND).count();
    Ok(NativeStemsBeamGlyph {
        bounds,
        weight,
        run_table,
    })
}

fn fixed_run_digest(run_table: &RunTable) -> u64 {
    let orientation = match run_table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_line(
        &mut hash,
        &format!("{orientation} {} {}", run_table.width(), run_table.height()),
    );
    for sequence in 0..run_table.sequence_count() {
        let mut row = sequence.to_string();
        for run in run_table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        hash_line(&mut hash, &row);
    }
    hash
}

fn hash_line(hash: &mut u64, line: &str) {
    for byte in line.bytes().chain([b'\n']) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(
        kept_ordinal: usize,
        free_glyph_ordinal: usize,
        x: f64,
        y: i32,
        height: i32,
        distance_sq: f64,
    ) -> NativeStemsBeamSeed {
        NativeStemsBeamSeed {
            pre_sort_ordinal: kept_ordinal,
            sorted_ordinal: kept_ordinal,
            kept_ordinal,
            free_glyph_ordinal,
            bounds: JavaRectangle::new(x as i32, y, 1, height),
            center_line: Segment {
                x1: x,
                y1: f64::from(y),
                x2: x,
                y2: f64::from(y + height),
            },
            intersection: (x, 0.0),
            distance_to_seed_segment_sq: distance_sq,
        }
    }

    #[test]
    fn point_segment_distance_clamps_to_endpoint_without_extra_unit() {
        let line = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 10.0,
        };
        assert_eq!(point_segment_distance_sq(line, (0.0, -2.0)), 4.0);
        assert_eq!(point_segment_distance_sq(line, (0.0, 12.0)), 4.0);
        assert_eq!(point_segment_distance_sq(line, (0.0, 4.0)), 0.0);
    }

    #[test]
    fn purge_distance_and_height_ties_remove_the_second_seed() {
        let (survivors, steps) = purge_seeds(
            vec![seed(0, 10, 0.0, 0, 4, 9.0), seed(1, 11, 2.0, 8, 4, 9.0)],
            10,
        );
        assert_eq!(survivors[0].free_glyph_ordinal, 10);
        assert_eq!(
            steps[0].action,
            NativeStemsBeamSeedPurgeAction::RemoveSecondForDistance
        );

        let (survivors, steps) = purge_seeds(
            vec![seed(0, 20, 0.0, 0, 4, 9.0), seed(1, 21, 2.0, 2, 4, 1.0)],
            10,
        );
        assert_eq!(survivors[0].free_glyph_ordinal, 20);
        assert_eq!(
            steps[0].action,
            NativeStemsBeamSeedPurgeAction::RemoveSecondForHeight
        );
    }

    #[test]
    fn purge_minimum_dx_threshold_is_inclusive_break() {
        let (survivors, steps) = purge_seeds(
            vec![seed(0, 30, 0.0, 0, 4, 0.0), seed(1, 31, 10.0, 0, 9, 0.0)],
            10,
        );
        assert_eq!(survivors.len(), 2);
        assert_eq!(
            steps[0].action,
            NativeStemsBeamSeedPurgeAction::BreakAtMinimumDx
        );
    }

    #[test]
    fn tremolo_width_gate_includes_margins_and_rejects_side_stumps() {
        let interline = 20;
        let typical = f64::from(interline) * TREMOLO_WIDTH_RATIO;
        let margin = f64::from(interline) * TREMOLO_WIDTH_MARGIN_RATIO;
        assert!(tremolo_width_gate(1, 0, typical - margin, interline));
        assert!(tremolo_width_gate(1, 0, typical + margin, interline));
        assert!(!tremolo_width_gate(
            1,
            0,
            typical + margin + f64::EPSILON * typical,
            interline
        ));
        assert!(!tremolo_width_gate(1, 1, typical, interline));
        assert!(!tremolo_width_gate(2, 0, typical, interline));
    }
}
