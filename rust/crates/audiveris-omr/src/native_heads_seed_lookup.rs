// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-shaped, identity-free port of Java `Scanner.lookupSeeds()`.
//!
//! This layer composes the frozen HEADS inputs through `HeadInter` construction,
//! but deliberately stops before `HeadInter.retrieveGlyph`, SIG insertion, and
//! relation creation. Every attempted location is retained so a differential
//! oracle can identify the first ordering, gate, or distance divergence.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    head_scanner::{HeadScannerError, HeadScannerLine},
    head_scanner_slices::JavaRectangle,
    head_template::{
        HeadTemplateAnchor, HeadTemplateBounds, HeadTemplateEvaluationError, HeadTemplateShape,
        MIN_INTER_GRADE, head_grade_of, impact_of,
    },
    head_template_overlap::{
        horizontal_parallelogram_intersects_rectangle, vertical_ribbon_intersects_rectangle,
    },
    native_heads::NativeHeadsPrologRecognition,
    native_heads_bar_slices::NativeHeadScannerBarSlicesRecognition,
    native_heads_competitor_slices::NativeHeadScannerCompetitorSlicesRecognition,
    native_heads_competitors::{NativeHeadsCompetitorGeometry, NativeHeadsCompetitorPool},
    native_heads_obstacles::NativeHeadsBarObstaclePool,
    native_heads_scanner::{
        NativeHeadScannerGeometry, NativeHeadScannerPhase, NativeHeadScannerSource,
        NativeHeadsScannerRecognition,
    },
    native_heads_scanner_pools::NativeHeadScannerPoolsRecognition,
    native_stem_seeds::{NativeStemSeedGlyph, NativeStemSeedRecognition},
};

/// Java `NoteHeadsBuilder.Constants.minHoleWhiteRatio`.
pub const MIN_HOLE_WHITE_RATIO: f64 = 0.2;
/// Java `Grades.goodInterGrade`.
pub const GOOD_INTER_GRADE: f64 = 0.4;

/// All immutable products consumed by the seed lookup pass.
pub struct NativeHeadsSeedLookupInput<'a> {
    pub heads: &'a NativeHeadsPrologRecognition,
    pub scanners: &'a NativeHeadsScannerRecognition,
    pub pools: &'a NativeHeadScannerPoolsRecognition,
    pub obstacles: &'a NativeHeadsBarObstaclePool,
    pub bar_slices: &'a NativeHeadScannerBarSlicesRecognition,
    pub competitors: &'a NativeHeadsCompetitorPool,
    pub competitor_slices: &'a NativeHeadScannerCompetitorSlicesRecognition,
    pub stem_seeds: &'a NativeStemSeedRecognition,
}

/// Identity-free result in Java system/staff/scanner order.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsSeedLookupRecognition {
    pub systems: Vec<NativeHeadSeedLookupSystem>,
    pub performance: NativeHeadSeedPerformance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadSeedLookupSystem {
    pub system_id: usize,
    /// Java `buildHeads` omits tablature staffs rather than emitting empty passes.
    pub staffs: Vec<NativeHeadSeedLookupStaff>,
    pub performance: NativeHeadSeedPerformance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadSeedLookupStaff {
    pub staff_id: usize,
    pub scans: Vec<NativeHeadSeedLookupScan>,
    pub performance: NativeHeadSeedPerformance,
    /// FNV-1a-64 over the oracle's identity-free `headseedkernel` rows.
    pub kernel_hash: u64,
}

/// One seed-phase scanner invocation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadSeedLookupScan {
    pub geometry_ordinal: usize,
    pub source: NativeHeadScannerSource,
    pub pitch: i32,
    pub seed_pool_indices: Vec<usize>,
    pub searches: Vec<NativeHeadSeedShapeSearch>,
    pub candidates: Vec<NativeHeadSeedCandidate>,
    pub performance: NativeHeadSeedPerformance,
}

/// Java's four seed lookup counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeHeadSeedPerformance {
    pub bar_blocks: usize,
    pub competitor_blocks: usize,
    pub evaluations: usize,
    pub nominal_abandons: usize,
}

impl NativeHeadSeedPerformance {
    fn include(&mut self, other: Self) {
        self.bar_blocks += other.bar_blocks;
        self.competitor_blocks += other.competitor_blocks;
        self.evaluations += other.evaluations;
        self.nominal_abandons += other.nominal_abandons;
    }

    fn observe_attempt(&mut self, outcome: NativeHeadSeedAttemptOutcome) {
        match outcome {
            NativeHeadSeedAttemptOutcome::BarBlocked { .. } => self.bar_blocks += 1,
            NativeHeadSeedAttemptOutcome::CompetitorBlocked { .. } => {
                self.competitor_blocks += 1;
            }
            NativeHeadSeedAttemptOutcome::Evaluated { .. } => self.evaluations += 1,
        }
    }
}

/// Exact stem-axis calculation shared by the four side/shape searches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadSeedAxis {
    pub rough_x: i32,
    pub scanner_line_y: f64,
    pub seed_start: (f64, f64),
    pub seed_stop: (f64, f64),
    pub precise_x: i32,
    pub theoretical_y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadSeedSide {
    Left,
    Right,
}

impl NativeHeadSeedSide {
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    #[must_use]
    pub const fn anchor(self) -> HeadTemplateAnchor {
        match self {
            Self::Left => HeadTemplateAnchor::LeftStem,
            Self::Right => HeadTemplateAnchor::RightStem,
        }
    }
}

/// One attempted template pivot, in y-offset then x-offset order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadSeedLocationAttempt {
    pub ordinal: usize,
    pub y_offset_ordinal: usize,
    pub y_offset: i32,
    pub x_offset_ordinal: usize,
    pub x_offset: i32,
    pub x: i32,
    pub y: i32,
    pub slim_bounds: HeadTemplateBounds,
    pub nominal: bool,
    pub outcome: NativeHeadSeedAttemptOutcome,
    /// Whether this acceptable distance strictly replaced the prior best.
    pub selected_as_best: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeHeadSeedAttemptOutcome {
    BarBlocked { obstacle_pool_index: usize },
    CompetitorBlocked { competitor_pool_index: usize },
    Evaluated { distance: f64 },
}

/// The current best is replaced only when Java's strict `best.d > loc.d` holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadSeedBestLocation {
    pub attempt_ordinal: usize,
    pub x: i32,
    pub y: i32,
    pub distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadSeedHoleEvidence {
    pub ratio: f64,
    pub threshold: f64,
    pub converted_to_void: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeHeadSeedSearchOutcome {
    AbandonedAtNominal {
        reason: NativeHeadSeedNominalAbandonReason,
    },
    NoAcceptableLocation,
    BelowMinimumGrade,
    ProvisionalCandidate {
        candidate_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeHeadSeedNominalAbandonReason {
    Bar { obstacle_pool_index: usize },
    Competitor { competitor_pool_index: usize },
    Distance { distance: f64 },
}

/// One seed/side/original-shape search, in exact nested-loop order.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadSeedShapeSearch {
    pub ordinal: usize,
    pub seed_pool_index: usize,
    pub seed_source_ordinal: usize,
    pub seed_bounds: JavaRectangle,
    pub seed_weight: usize,
    pub seed_run_digest: u64,
    pub axis: NativeHeadSeedAxis,
    pub side: NativeHeadSeedSide,
    pub anchor: HeadTemplateAnchor,
    pub shape_ordinal: usize,
    pub original_shape: HeadTemplateShape,
    pub attempts: Vec<NativeHeadSeedLocationAttempt>,
    pub performance: NativeHeadSeedPerformance,
    pub best: Option<NativeHeadSeedBestLocation>,
    pub best_replacement_count: usize,
    pub hole: Option<NativeHeadSeedHoleEvidence>,
    /// The loop variable's final value: original shape unless black converted to void.
    pub final_shape: HeadTemplateShape,
    pub raw_distance_impact: Option<f64>,
    pub grade: Option<f64>,
    pub slim_bounds: Option<HeadTemplateBounds>,
    pub outcome: NativeHeadSeedSearchOutcome,
}

/// A `HeadInter` candidate immediately before glyph retrieval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadSeedCandidate {
    pub ordinal: usize,
    pub search_ordinal: usize,
    pub seed_pool_index: usize,
    pub seed_source_ordinal: usize,
    pub side: NativeHeadSeedSide,
    pub anchor: HeadTemplateAnchor,
    pub original_shape: HeadTemplateShape,
    pub shape: HeadTemplateShape,
    pub pitch: f64,
    pub x: i32,
    pub y: i32,
    pub distance: f64,
    pub bounds: HeadTemplateBounds,
    /// Unclamped `Template.impactOf(distance)` passed to `HeadInter.Impacts`.
    pub raw_distance_impact: f64,
    /// Clamped impact retained by `GradeImpacts`.
    pub distance_impact: f64,
    pub grade: f64,
    pub minimum_grade: f64,
    pub good_for_tally: bool,
    /// Java tally dx when `good_for_tally`; still provisional until glyph retrieval succeeds.
    pub tally_dx: Option<f64>,
}

#[derive(Debug)]
pub enum NativeHeadsSeedLookupError {
    SystemOrder {
        source: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    StaffOrder {
        source: &'static str,
        system_id: usize,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    ScanOrder {
        source: &'static str,
        system_id: usize,
        staff_id: usize,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    SeedSchedule {
        system_id: usize,
        staff_id: usize,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    MissingCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    CatalogSelectionMismatch {
        system_id: usize,
        staff_id: usize,
    },
    MissingCatalog {
        system_id: usize,
        staff_id: usize,
        catalog_ordinal: usize,
    },
    DuplicateSeedSource {
        system_id: usize,
        source_ordinal: usize,
    },
    MissingSeedSource {
        system_id: usize,
        source_ordinal: usize,
    },
    SeedPoolIndex {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        pool_index: usize,
    },
    BarPoolIndex {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        pool_index: usize,
    },
    CompetitorPoolIndex {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        pool_index: usize,
    },
    Geometry {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        source: HeadScannerError,
    },
    Template {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        source: HeadTemplateEvaluationError,
    },
}

impl fmt::Display for NativeHeadsSeedLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed lookup {source} system order {actual:?}, expected {expected:?}"
            ),
            Self::StaffOrder {
                source,
                system_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} {source} staff order {actual:?}, expected \
                 {expected:?}"
            ),
            Self::ScanOrder {
                source,
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} {source} scan order \
                 {actual:?}, expected {expected:?}"
            ),
            Self::SeedSchedule {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} seed schedule {actual:?}, \
                 expected {expected:?}"
            ),
            Self::MissingCatalogSelection {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS seed lookup has no template catalog selection for system {system_id} \
                 staff {staff_id}"
            ),
            Self::CatalogSelectionMismatch {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS seed lookup template catalog selection disagrees for system {system_id} \
                 staff {staff_id}"
            ),
            Self::MissingCatalog {
                system_id,
                staff_id,
                catalog_ordinal,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} references missing catalog \
                 {catalog_ordinal}"
            ),
            Self::DuplicateSeedSource {
                system_id,
                source_ordinal,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} contains duplicate free seed source \
                 {source_ordinal}"
            ),
            Self::MissingSeedSource {
                system_id,
                source_ordinal,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} has no free seed source {source_ordinal}"
            ),
            Self::SeedPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} \
                 references missing seed-pool index {pool_index}"
            ),
            Self::BarPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} \
                 references missing bar-pool index {pool_index}"
            ),
            Self::CompetitorPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} \
                 references missing competitor-pool index {pool_index}"
            ),
            Self::Geometry {
                system_id,
                staff_id,
                geometry_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} \
                 geometry failed: {source}"
            ),
            Self::Template {
                system_id,
                staff_id,
                geometry_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS seed lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} \
                 template failed: {source}"
            ),
        }
    }
}

impl Error for NativeHeadsSeedLookupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Geometry { source, .. } => Some(source),
            Self::Template { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Run Java's seed-driven template search through provisional `HeadInter`
/// construction. Glyph retrieval and SIG identity are the next port boundary.
pub fn recognize_native_heads_seed_lookup(
    input: NativeHeadsSeedLookupInput<'_>,
) -> Result<NativeHeadsSeedLookupRecognition, NativeHeadsSeedLookupError> {
    validate_system_topology(&input)?;

    let mut recognition_performance = NativeHeadSeedPerformance::default();
    let mut systems = Vec::with_capacity(input.scanners.systems.len());
    for system_ordinal in 0..input.scanners.systems.len() {
        let scanner_system = &input.scanners.systems[system_ordinal];
        let pool_system = &input.pools.systems[system_ordinal];
        let obstacle_system = &input.obstacles.systems[system_ordinal];
        let bar_slice_system = &input.bar_slices.systems[system_ordinal];
        let competitor_system = &input.competitors.systems[system_ordinal];
        let competitor_slice_system = &input.competitor_slices.systems[system_ordinal];
        let seed_system = &input.stem_seeds.systems[system_ordinal];
        let system_id = scanner_system.system_id;

        validate_staff_topology(
            system_id,
            scanner_system,
            pool_system,
            bar_slice_system,
            competitor_slice_system,
        )?;
        let seed_sources = seed_sources(system_id, &seed_system.free_glyphs)?;

        let mut system_performance = NativeHeadSeedPerformance::default();
        let mut staffs = Vec::with_capacity(scanner_system.staffs.len());
        for staff_ordinal in 0..scanner_system.staffs.len() {
            let scanner_staff = &scanner_system.staffs[staff_ordinal];
            let pool_staff = &pool_system.staffs[staff_ordinal];
            let bar_slice_staff = &bar_slice_system.staffs[staff_ordinal];
            let competitor_slice_staff = &competitor_slice_system.staffs[staff_ordinal];
            let staff_id = scanner_staff.staff_id;

            validate_scan_topology(
                system_id,
                staff_id,
                scanner_staff,
                pool_staff,
                bar_slice_staff,
                competitor_slice_staff,
            )?;

            // `NoteHeadsBuilder.buildHeads` never constructs a scanner for a
            // tablature staff. Keep validating its retained empty topology,
            // but do not invent an empty lookup result.
            if !staff_runs_seed_lookup(scanner_staff.tablature) {
                continue;
            }

            let catalog = if scanner_staff.geometries.is_empty() {
                None
            } else {
                let selection = input
                    .heads
                    .staff_template_catalogs
                    .iter()
                    .find(|selection| {
                        selection.system_id == system_id && selection.staff_id == staff_id
                    })
                    .ok_or(NativeHeadsSeedLookupError::MissingCatalogSelection {
                        system_id,
                        staff_id,
                    })?;
                if scanner_staff.catalog_ordinal != Some(selection.catalog_ordinal)
                    || scanner_staff.point_size != Some(selection.point_size)
                    || scanner_staff.specific_interline != selection.specific_interline
                {
                    return Err(NativeHeadsSeedLookupError::CatalogSelectionMismatch {
                        system_id,
                        staff_id,
                    });
                }
                Some(
                    input
                        .heads
                        .template_catalogs
                        .get(selection.catalog_ordinal)
                        .ok_or(NativeHeadsSeedLookupError::MissingCatalog {
                            system_id,
                            staff_id,
                            catalog_ordinal: selection.catalog_ordinal,
                        })?,
                )
            };

            let seed_schedule = scanner_staff
                .schedule
                .iter()
                .filter(|entry| entry.phase == NativeHeadScannerPhase::Seed)
                .map(|entry| entry.geometry)
                .collect::<Vec<_>>();
            let expected_schedule = (0..scanner_staff.geometries.len()).collect::<Vec<_>>();
            if seed_schedule != expected_schedule {
                return Err(NativeHeadsSeedLookupError::SeedSchedule {
                    system_id,
                    staff_id,
                    expected: expected_schedule,
                    actual: seed_schedule,
                });
            }

            let mut staff_performance = NativeHeadSeedPerformance::default();
            let mut scans = Vec::with_capacity(scanner_staff.geometries.len());
            let mut staff_search_count = 0;
            let mut staff_candidate_count = 0;
            for &geometry_ordinal in &seed_schedule {
                let geometry = &scanner_staff.geometries[geometry_ordinal];
                let pool_scan = &pool_staff.scans[geometry_ordinal];
                let bar_slice = &bar_slice_staff.scans[geometry_ordinal];
                let competitor_slice = &competitor_slice_staff.scans[geometry_ordinal];
                let scan = lookup_scan(ScanInput {
                    system_id,
                    staff_id,
                    geometry_ordinal,
                    geometry,
                    parameters: &scanner_system.parameters,
                    catalog: catalog.expect("non-empty scanner geometry has a checked catalog"),
                    distances: &input.heads.distance_table,
                    pool_system,
                    seed_sources: &seed_sources,
                    seed_indices: &pool_scan.seed_indices,
                    obstacle_pool: &obstacle_system.frozen_by_ordinate,
                    bar_indices: &bar_slice.frozen_bar_indices,
                    competitor_pool: &competitor_system.accepted_by_ordinate,
                    competitor_indices: &competitor_slice.competitor_indices,
                    search_ordinal_base: staff_search_count,
                    candidate_ordinal_base: staff_candidate_count,
                })?;
                staff_search_count += scan.searches.len();
                staff_candidate_count += scan.candidates.len();
                staff_performance.include(scan.performance);
                scans.push(scan);
            }
            system_performance.include(staff_performance);
            let kernel_hash = seed_kernel_hash(system_id, staff_id, &scans);
            staffs.push(NativeHeadSeedLookupStaff {
                staff_id,
                scans,
                performance: staff_performance,
                kernel_hash,
            });
        }
        recognition_performance.include(system_performance);
        systems.push(NativeHeadSeedLookupSystem {
            system_id,
            staffs,
            performance: system_performance,
        });
    }

    Ok(NativeHeadsSeedLookupRecognition {
        systems,
        performance: recognition_performance,
    })
}

const fn staff_runs_seed_lookup(tablature: bool) -> bool {
    !tablature
}

struct ScanInput<'a> {
    system_id: usize,
    staff_id: usize,
    geometry_ordinal: usize,
    geometry: &'a NativeHeadScannerGeometry,
    parameters: &'a crate::head_scanner::HeadScannerParameters,
    catalog: &'a crate::head_template::HeadTemplateCatalog,
    distances: &'a crate::heads_step::NeutralDistanceTable,
    pool_system: &'a crate::native_heads_scanner_pools::NativeHeadScannerPoolSystem,
    seed_sources: &'a BTreeMap<usize, &'a NativeStemSeedGlyph>,
    seed_indices: &'a [usize],
    obstacle_pool: &'a [crate::native_heads_obstacles::NativeHeadsBarObstacle],
    bar_indices: &'a [usize],
    competitor_pool: &'a [crate::native_heads_competitors::NativeHeadsCompetitor],
    competitor_indices: &'a [usize],
    search_ordinal_base: usize,
    candidate_ordinal_base: usize,
}

fn lookup_scan(
    input: ScanInput<'_>,
) -> Result<NativeHeadSeedLookupScan, NativeHeadsSeedLookupError> {
    let mut searches = Vec::new();
    let mut candidates = Vec::new();
    let mut performance = NativeHeadSeedPerformance::default();

    for &seed_pool_index in input.seed_indices {
        let pool_seed = input.pool_system.seeds.get(seed_pool_index).ok_or(
            NativeHeadsSeedLookupError::SeedPoolIndex {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                pool_index: seed_pool_index,
            },
        )?;
        let seed = input
            .seed_sources
            .get(&pool_seed.source_ordinal)
            .copied()
            .ok_or(NativeHeadsSeedLookupError::MissingSeedSource {
                system_id: input.system_id,
                source_ordinal: pool_seed.source_ordinal,
            })?;
        let axis = seed_axis(input.geometry, seed).map_err(|source| {
            NativeHeadsSeedLookupError::Geometry {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                source,
            }
        })?;
        let seed_run_digest = seed.run_digest();

        for side in NativeHeadSeedSide::ALL {
            let anchor = side.anchor();
            for (shape_ordinal, &original_shape) in input.geometry.stem_shapes.iter().enumerate() {
                let search_ordinal = input.search_ordinal_base + searches.len();
                let template = input.catalog.template(original_shape);
                let exploration = explore_locations(
                    axis.precise_x,
                    axis.theoretical_y,
                    &input.geometry.y_offsets,
                    &input.parameters.x_offsets,
                    input.parameters.max_distance_low,
                    input.parameters.really_bad_distance,
                    |x, y| {
                        let slim_bounds =
                            template.slim_bounds_at(x, y, anchor).map_err(|source| {
                                NativeHeadsSeedLookupError::Template {
                                    system_id: input.system_id,
                                    staff_id: input.staff_id,
                                    geometry_ordinal: input.geometry_ordinal,
                                    source,
                                }
                            })?;
                        let rectangle = java_rectangle(slim_bounds);
                        if let Some(obstacle_pool_index) = first_bar_overlap(
                            rectangle,
                            input.obstacle_pool,
                            input.bar_indices,
                            input.system_id,
                            input.staff_id,
                            input.geometry_ordinal,
                        )? {
                            return Ok(LocationEvaluation {
                                slim_bounds,
                                outcome: NativeHeadSeedAttemptOutcome::BarBlocked {
                                    obstacle_pool_index,
                                },
                            });
                        }
                        if let Some(competitor_pool_index) = first_competitor_overlap(
                            rectangle,
                            input.competitor_pool,
                            input.competitor_indices,
                            input.system_id,
                            input.staff_id,
                            input.geometry_ordinal,
                        )? {
                            return Ok(LocationEvaluation {
                                slim_bounds,
                                outcome: NativeHeadSeedAttemptOutcome::CompetitorBlocked {
                                    competitor_pool_index,
                                },
                            });
                        }
                        let distance = template
                            .evaluate(x, y, Some(anchor), input.distances)
                            .map_err(|source| NativeHeadsSeedLookupError::Template {
                                system_id: input.system_id,
                                staff_id: input.staff_id,
                                geometry_ordinal: input.geometry_ordinal,
                                source,
                            })?;
                        Ok(LocationEvaluation {
                            slim_bounds,
                            outcome: NativeHeadSeedAttemptOutcome::Evaluated { distance },
                        })
                    },
                )?;

                performance.include(exploration.performance);
                let best = exploration.best;
                let mut final_shape = original_shape;
                let mut hole = None;
                let mut raw_distance_impact = None;
                let mut grade = None;
                let mut slim_bounds = None;
                let outcome;

                if exploration.abandoned {
                    let reason = match exploration
                        .attempts
                        .last()
                        .expect("abandoned search records its nominal attempt")
                        .outcome
                    {
                        NativeHeadSeedAttemptOutcome::BarBlocked {
                            obstacle_pool_index,
                        } => NativeHeadSeedNominalAbandonReason::Bar {
                            obstacle_pool_index,
                        },
                        NativeHeadSeedAttemptOutcome::CompetitorBlocked {
                            competitor_pool_index,
                        } => NativeHeadSeedNominalAbandonReason::Competitor {
                            competitor_pool_index,
                        },
                        NativeHeadSeedAttemptOutcome::Evaluated { distance } => {
                            NativeHeadSeedNominalAbandonReason::Distance { distance }
                        }
                    };
                    outcome = NativeHeadSeedSearchOutcome::AbandonedAtNominal { reason };
                } else if let Some(best_location) = best {
                    let selected_shape = if original_shape == HeadTemplateShape::NoteheadBlack {
                        let void_shape = HeadTemplateShape::NoteheadVoid;
                        let void_template = input.catalog.template(void_shape);
                        let ratio = void_template
                            .evaluate_hole(
                                best_location.x,
                                best_location.y,
                                Some(anchor),
                                input.distances,
                            )
                            .map_err(|source| NativeHeadsSeedLookupError::Template {
                                system_id: input.system_id,
                                staff_id: input.staff_id,
                                geometry_ordinal: input.geometry_ordinal,
                                source,
                            })?;
                        let converted_to_void = ratio >= MIN_HOLE_WHITE_RATIO;
                        hole = Some(NativeHeadSeedHoleEvidence {
                            ratio,
                            threshold: MIN_HOLE_WHITE_RATIO,
                            converted_to_void,
                        });
                        if converted_to_void {
                            void_shape
                        } else {
                            original_shape
                        }
                    } else {
                        original_shape
                    };
                    final_shape = selected_shape;
                    let raw_impact = impact_of(best_location.distance);
                    let candidate_grade = head_grade_of(best_location.distance);
                    raw_distance_impact = Some(raw_impact);
                    grade = Some(candidate_grade);
                    if candidate_grade < MIN_INTER_GRADE {
                        outcome = NativeHeadSeedSearchOutcome::BelowMinimumGrade;
                    } else {
                        let candidate_ordinal = input.candidate_ordinal_base + candidates.len();
                        let final_template = input.catalog.template(selected_shape);
                        let bounds = final_template
                            .slim_bounds_at(best_location.x, best_location.y, anchor)
                            .map_err(|source| NativeHeadsSeedLookupError::Template {
                                system_id: input.system_id,
                                staff_id: input.staff_id,
                                geometry_ordinal: input.geometry_ordinal,
                                source,
                            })?;
                        slim_bounds = Some(bounds);
                        let good_for_tally = candidate_grade >= GOOD_INTER_GRADE;
                        candidates.push(NativeHeadSeedCandidate {
                            ordinal: candidate_ordinal,
                            search_ordinal,
                            seed_pool_index,
                            seed_source_ordinal: pool_seed.source_ordinal,
                            side,
                            anchor,
                            original_shape,
                            shape: selected_shape,
                            pitch: f64::from(input.geometry.pitch),
                            x: best_location.x,
                            y: best_location.y,
                            distance: best_location.distance,
                            bounds,
                            raw_distance_impact: raw_impact,
                            distance_impact: raw_impact.clamp(0.0, 1.0),
                            grade: candidate_grade,
                            minimum_grade: MIN_INTER_GRADE,
                            good_for_tally,
                            tally_dx: good_for_tally
                                .then(|| tally_dx(side, bounds, axis.precise_x)),
                        });
                        outcome =
                            NativeHeadSeedSearchOutcome::ProvisionalCandidate { candidate_ordinal };
                    }
                } else {
                    outcome = NativeHeadSeedSearchOutcome::NoAcceptableLocation;
                }

                searches.push(NativeHeadSeedShapeSearch {
                    ordinal: search_ordinal,
                    seed_pool_index,
                    seed_source_ordinal: pool_seed.source_ordinal,
                    seed_bounds: pool_seed.bounds,
                    seed_weight: seed.weight,
                    seed_run_digest,
                    axis,
                    side,
                    anchor,
                    shape_ordinal,
                    original_shape,
                    best,
                    best_replacement_count: exploration.best_replacement_count,
                    attempts: exploration.attempts,
                    performance: exploration.performance,
                    hole,
                    final_shape,
                    raw_distance_impact,
                    grade,
                    slim_bounds,
                    outcome,
                });
            }
        }
    }

    Ok(NativeHeadSeedLookupScan {
        geometry_ordinal: input.geometry_ordinal,
        source: input.geometry.source.clone(),
        pitch: input.geometry.pitch,
        seed_pool_indices: input.seed_indices.to_vec(),
        searches,
        candidates,
        performance,
    })
}

#[derive(Clone, Copy)]
struct LocationEvaluation {
    slim_bounds: HeadTemplateBounds,
    outcome: NativeHeadSeedAttemptOutcome,
}

struct LocationExploration {
    attempts: Vec<NativeHeadSeedLocationAttempt>,
    best: Option<NativeHeadSeedBestLocation>,
    best_replacement_count: usize,
    abandoned: bool,
    performance: NativeHeadSeedPerformance,
}

fn explore_locations<E>(
    x0: i32,
    y0: i32,
    y_offsets: &[i32],
    x_offsets: &[i32],
    max_distance_low: f64,
    really_bad_distance: f64,
    mut evaluate: impl FnMut(i32, i32) -> Result<LocationEvaluation, E>,
) -> Result<LocationExploration, E> {
    let mut attempts = Vec::with_capacity(y_offsets.len() * x_offsets.len());
    let mut best: Option<NativeHeadSeedBestLocation> = None;
    let mut best_replacement_count = 0;
    let mut abandoned = false;
    let mut performance = NativeHeadSeedPerformance::default();

    'locations: for (y_offset_ordinal, &y_offset) in y_offsets.iter().enumerate() {
        let y = y0.wrapping_add(y_offset);
        for (x_offset_ordinal, &x_offset) in x_offsets.iter().enumerate() {
            let x = x0.wrapping_add(x_offset);
            let ordinal = attempts.len();
            let evaluation = evaluate(x, y)?;
            performance.observe_attempt(evaluation.outcome);
            let nominal = x == x0 && y == y0;
            let distance = match evaluation.outcome {
                NativeHeadSeedAttemptOutcome::Evaluated { distance } => Some(distance),
                NativeHeadSeedAttemptOutcome::BarBlocked { .. }
                | NativeHeadSeedAttemptOutcome::CompetitorBlocked { .. } => None,
            };
            let selected_as_best = distance.is_some_and(|distance| {
                distance <= max_distance_low
                    && best.is_none_or(|best_location| best_location.distance > distance)
            });
            if selected_as_best {
                let distance = distance.expect("selected evaluated location has a distance");
                best = Some(NativeHeadSeedBestLocation {
                    attempt_ordinal: ordinal,
                    x,
                    y,
                    distance,
                });
                best_replacement_count += 1;
            }
            attempts.push(NativeHeadSeedLocationAttempt {
                ordinal,
                y_offset_ordinal,
                y_offset,
                x_offset_ordinal,
                x_offset,
                x,
                y,
                slim_bounds: evaluation.slim_bounds,
                nominal,
                outcome: evaluation.outcome,
                selected_as_best,
            });

            let nominal_abandon = nominal
                && match distance {
                    None => true,
                    Some(distance) => distance >= really_bad_distance,
                };
            if nominal_abandon {
                abandoned = true;
                performance.nominal_abandons += 1;
                break 'locations;
            }
        }
    }

    Ok(LocationExploration {
        attempts,
        best,
        best_replacement_count,
        abandoned,
        performance,
    })
}

fn seed_axis(
    geometry: &NativeHeadScannerGeometry,
    seed: &NativeStemSeedGlyph,
) -> Result<NativeHeadSeedAxis, HeadScannerError> {
    let rough_center_x = seed.bounds.x as f64 + (seed.bounds.width as f64 / 2.0);
    let rough_x = java_rint_to_i32(rough_center_x);
    let scanner_line_y = geometry.line.y_at(f64::from(rough_x));
    let precise_x = java_rint_to_i32(java_line_x_at_y(seed.start, seed.stop, scanner_line_y));
    let theoretical_y = geometry.theoretical_ordinate(precise_x)?;
    Ok(NativeHeadSeedAxis {
        rough_x,
        scanner_line_y,
        seed_start: seed.start,
        seed_stop: seed.stop,
        precise_x,
        theoretical_y,
    })
}

fn java_rint_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

/// Preserve `LineUtil.intersection(..., 0, y, 1000, y).x` operation order.
fn java_line_x_at_y(start: (f64, f64), stop: (f64, f64), y: f64) -> f64 {
    let (x1, y1) = start;
    let (x2, y2) = stop;
    let x3 = 0.0;
    let y3 = y;
    let x4 = 1_000.0;
    let y4 = y;
    let denominator = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    let value12 = (x1 * y2) - (y1 * x2);
    let value34 = (x3 * y4) - (y3 * x4);
    ((value12 * (x3 - x4)) - ((x1 - x2) * value34)) / denominator
}

fn tally_dx(side: NativeHeadSeedSide, bounds: HeadTemplateBounds, x0: i32) -> f64 {
    match side {
        NativeHeadSeedSide::Left => f64::from(bounds.x.wrapping_sub(x0)) + 0.5,
        NativeHeadSeedSide::Right => {
            let inclusive_right = bounds.x.wrapping_add(bounds.width).wrapping_sub(1);
            f64::from(x0) + 0.5 - f64::from(inclusive_right)
        }
    }
}

fn java_rectangle(bounds: HeadTemplateBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

/// Canonical identity-free oracle record for one seed/side/shape search.
#[must_use]
pub fn native_head_seed_kernel_record(
    system_id: usize,
    staff_id: usize,
    scan: &NativeHeadSeedLookupScan,
    search: &NativeHeadSeedShapeSearch,
) -> String {
    let nominal = search
        .attempts
        .iter()
        .find(|attempt| attempt.nominal)
        .expect("production offsets contain the nominal location");
    let (nominal_gate, nominal_distance) = match nominal.outcome {
        NativeHeadSeedAttemptOutcome::BarBlocked { .. } => ("bar", "-".to_owned()),
        NativeHeadSeedAttemptOutcome::CompetitorBlocked { .. } => ("competitor", "-".to_owned()),
        NativeHeadSeedAttemptOutcome::Evaluated { distance } => {
            ("evaluated", java_hex_double(distance))
        }
    };
    let selected = search.best.map_or_else(
        || "-".to_owned(),
        |best| {
            let attempt = &search.attempts[best.attempt_ordinal];
            format!(
                "{}:{}:{}:{}:{}:{}:{}",
                attempt.y_offset_ordinal,
                attempt.y_offset,
                attempt.x_offset_ordinal,
                attempt.x_offset,
                best.x,
                best.y,
                java_hex_double(best.distance)
            )
        },
    );
    let black_to_void = search
        .hole
        .map_or_else(|| "-".to_owned(), |hole| hole.converted_to_void.to_string());
    let minimum_grade_pass = search.slim_bounds.is_some();
    let slim = search.slim_bounds.map_or_else(
        || "-".to_owned(),
        |bounds| rectangle_string(java_rectangle(bounds)),
    );

    format!(
        "headseedkernel system {system_id} staff {staff_id} scanner {} source {} seed {} \
         seedBounds {} seedWeight {} seedRuns {:016x} side {} shapeOrdinal {} original {} \
         finalShape {} nominal {}:{} nominalGate {nominal_gate} nominalDistance \
         {nominal_distance} calls {}:{}:{} bestUpdates {} selected {selected} blackToVoid \
         {black_to_void} minGrade {minimum_grade_pass} slim {slim} outcome {}",
        scan.geometry_ordinal,
        source_string(&scan.source),
        search.seed_pool_index,
        rectangle_string(search.seed_bounds),
        search.seed_weight,
        search.seed_run_digest,
        anchor_name(search.anchor),
        search.shape_ordinal,
        shape_name(search.original_shape),
        shape_name(search.final_shape),
        search.axis.precise_x,
        search.axis.theoretical_y,
        search.performance.bar_blocks,
        search.performance.competitor_blocks,
        search.performance.evaluations,
        search.best_replacement_count,
        kernel_outcome_name(search.outcome),
    )
}

/// FNV-1a-64 over canonical `headseedkernel` records plus trailing newlines.
#[must_use]
pub fn seed_kernel_hash(
    system_id: usize,
    staff_id: usize,
    scans: &[NativeHeadSeedLookupScan],
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for scan in scans {
        for search in &scan.searches {
            let record = native_head_seed_kernel_record(system_id, staff_id, scan, search);
            for byte in record.bytes().chain(std::iter::once(b'\n')) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    hash
}

fn rectangle_string(rectangle: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        rectangle.x, rectangle.y, rectangle.width, rectangle.height
    )
}

fn source_string(source: &NativeHeadScannerSource) -> String {
    match source {
        NativeHeadScannerSource::StaffLine { line_index, axis } => format!(
            "staff-line:{line_index}:axis:{}:{}:{}:{}",
            java_hex_double(axis.x1),
            java_hex_double(axis.y1),
            java_hex_double(axis.x2),
            java_hex_double(axis.y2)
        ),
        NativeHeadScannerSource::Ledger {
            ledger_index,
            ordinal,
            bounds,
            weight,
            run_digest,
            axis,
            ..
        } => format!(
            "ledger:{ledger_index}:{ordinal}:bounds:{}:{}:{}:{}:weight:{weight}:runs:{run_digest:016x}:axis:{}:{}:{}:{}",
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            java_hex_double(axis.x1),
            java_hex_double(axis.y1),
            java_hex_double(axis.x2),
            java_hex_double(axis.y2)
        ),
    }
}

const fn anchor_name(anchor: HeadTemplateAnchor) -> &'static str {
    match anchor {
        HeadTemplateAnchor::Center => "CENTER",
        HeadTemplateAnchor::MiddleLeft => "MIDDLE_LEFT",
        HeadTemplateAnchor::MiddleRight => "MIDDLE_RIGHT",
        HeadTemplateAnchor::LeftStem => "LEFT_STEM",
        HeadTemplateAnchor::TopLeftStem => "TOP_LEFT_STEM",
        HeadTemplateAnchor::BottomLeftStem => "BOTTOM_LEFT_STEM",
        HeadTemplateAnchor::RightStem => "RIGHT_STEM",
        HeadTemplateAnchor::TopRightStem => "TOP_RIGHT_STEM",
        HeadTemplateAnchor::BottomRightStem => "BOTTOM_RIGHT_STEM",
    }
}

const fn shape_name(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
        HeadTemplateShape::NoteheadBlackSmall => "NOTEHEAD_BLACK_SMALL",
        HeadTemplateShape::NoteheadVoidSmall => "NOTEHEAD_VOID_SMALL",
        HeadTemplateShape::WholeNoteSmall => "WHOLE_NOTE_SMALL",
        HeadTemplateShape::BreveSmall => "BREVE_SMALL",
    }
}

const fn kernel_outcome_name(outcome: NativeHeadSeedSearchOutcome) -> &'static str {
    match outcome {
        NativeHeadSeedSearchOutcome::AbandonedAtNominal {
            reason: NativeHeadSeedNominalAbandonReason::Bar { .. },
        } => "abandoned-bar",
        NativeHeadSeedSearchOutcome::AbandonedAtNominal {
            reason: NativeHeadSeedNominalAbandonReason::Competitor { .. },
        } => "abandoned-competitor",
        NativeHeadSeedSearchOutcome::AbandonedAtNominal {
            reason: NativeHeadSeedNominalAbandonReason::Distance { .. },
        } => "abandoned-nominal",
        NativeHeadSeedSearchOutcome::NoAcceptableLocation => "no-best",
        NativeHeadSeedSearchOutcome::BelowMinimumGrade => "min-grade",
        NativeHeadSeedSearchOutcome::ProvisionalCandidate { .. } => "provisional",
    }
}

fn java_hex_double(value: f64) -> String {
    let raw_bits = value.to_bits();
    let canonical_bits = if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        raw_bits
    };
    let java = if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "Infinity".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        let sign = if (raw_bits >> 63) != 0 { "-" } else { "" };
        let exponent_bits = ((raw_bits >> 52) & 0x7ff) as i32;
        let fraction_bits = raw_bits & 0x000f_ffff_ffff_ffff;
        if exponent_bits == 0 && fraction_bits == 0 {
            format!("{sign}0x0.0p0")
        } else {
            let mut fraction = format!("{fraction_bits:013x}");
            while fraction.len() > 1 && fraction.ends_with('0') {
                fraction.pop();
            }
            if exponent_bits == 0 {
                format!("{sign}0x0.{fraction}p-1022")
            } else {
                format!("{sign}0x1.{fraction}p{}", exponent_bits - 1023)
            }
        }
    };
    format!("{java}/{canonical_bits:016x}")
}

fn first_bar_overlap(
    rectangle: JavaRectangle,
    obstacles: &[crate::native_heads_obstacles::NativeHeadsBarObstacle],
    indices: &[usize],
    system_id: usize,
    staff_id: usize,
    geometry_ordinal: usize,
) -> Result<Option<usize>, NativeHeadsSeedLookupError> {
    for &pool_index in indices {
        let obstacle =
            obstacles
                .get(pool_index)
                .ok_or(NativeHeadsSeedLookupError::BarPoolIndex {
                    system_id,
                    staff_id,
                    geometry_ordinal,
                    pool_index,
                })?;
        if vertical_ribbon_intersects_rectangle(obstacle.median, obstacle.thickness, rectangle) {
            return Ok(Some(pool_index));
        }
    }
    Ok(None)
}

fn first_competitor_overlap(
    rectangle: JavaRectangle,
    competitors: &[crate::native_heads_competitors::NativeHeadsCompetitor],
    indices: &[usize],
    system_id: usize,
    staff_id: usize,
    geometry_ordinal: usize,
) -> Result<Option<usize>, NativeHeadsSeedLookupError> {
    for &pool_index in indices {
        let competitor =
            competitors
                .get(pool_index)
                .ok_or(NativeHeadsSeedLookupError::CompetitorPoolIndex {
                    system_id,
                    staff_id,
                    geometry_ordinal,
                    pool_index,
                })?;
        let intersects = match competitor.geometry {
            NativeHeadsCompetitorGeometry::Vertical { median, width } => {
                vertical_ribbon_intersects_rectangle(median, width, rectangle)
            }
            NativeHeadsCompetitorGeometry::Horizontal { median, height } => {
                horizontal_parallelogram_intersects_rectangle(median, height, rectangle)
            }
        };
        if intersects {
            return Ok(Some(pool_index));
        }
    }
    Ok(None)
}

fn seed_sources(
    system_id: usize,
    seeds: &[NativeStemSeedGlyph],
) -> Result<BTreeMap<usize, &NativeStemSeedGlyph>, NativeHeadsSeedLookupError> {
    let mut sources = BTreeMap::new();
    for seed in seeds {
        if sources.insert(seed.source_ordinal, seed).is_some() {
            return Err(NativeHeadsSeedLookupError::DuplicateSeedSource {
                system_id,
                source_ordinal: seed.source_ordinal,
            });
        }
    }
    Ok(sources)
}

fn validate_system_topology(
    input: &NativeHeadsSeedLookupInput<'_>,
) -> Result<(), NativeHeadsSeedLookupError> {
    let expected = input
        .scanners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let sources = [
        (
            "scanner pools",
            input
                .pools
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>(),
        ),
        (
            "bar obstacles",
            input
                .obstacles
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "bar slices",
            input
                .bar_slices
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "competitors",
            input
                .competitors
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "competitor slices",
            input
                .competitor_slices
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "stem seeds",
            input
                .stem_seeds
                .systems
                .iter()
                .map(|system| system.raw.system_id)
                .collect(),
        ),
        (
            "HEADS prolog",
            input
                .heads
                .system_spot_ordinals
                .iter()
                .map(|(system_id, _)| *system_id)
                .collect(),
        ),
    ];
    for (source, actual) in sources {
        if actual != expected {
            return Err(NativeHeadsSeedLookupError::SystemOrder {
                source,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn validate_staff_topology(
    system_id: usize,
    scanners: &crate::native_heads_scanner::NativeHeadsScannerSystem,
    pools: &crate::native_heads_scanner_pools::NativeHeadScannerPoolSystem,
    bars: &crate::native_heads_bar_slices::NativeHeadScannerBarSliceSystem,
    competitors: &crate::native_heads_competitor_slices::NativeHeadScannerCompetitorSliceSystem,
) -> Result<(), NativeHeadsSeedLookupError> {
    let expected = scanners
        .staffs
        .iter()
        .map(|staff| staff.staff_id)
        .collect::<Vec<_>>();
    let sources = [
        (
            "scanner pools",
            pools.staffs.iter().map(|staff| staff.staff_id).collect(),
        ),
        (
            "bar slices",
            bars.staffs.iter().map(|staff| staff.staff_id).collect(),
        ),
        (
            "competitor slices",
            competitors
                .staffs
                .iter()
                .map(|staff| staff.staff_id)
                .collect(),
        ),
    ];
    for (source, actual) in sources {
        if actual != expected {
            return Err(NativeHeadsSeedLookupError::StaffOrder {
                source,
                system_id,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn validate_scan_topology(
    system_id: usize,
    staff_id: usize,
    scanners: &crate::native_heads_scanner::NativeHeadsScannerStaff,
    pools: &crate::native_heads_scanner_pools::NativeHeadScannerPoolStaff,
    bars: &crate::native_heads_bar_slices::NativeHeadScannerBarSliceStaff,
    competitors: &crate::native_heads_competitor_slices::NativeHeadScannerCompetitorSliceStaff,
) -> Result<(), NativeHeadsSeedLookupError> {
    let expected = (0..scanners.geometries.len()).collect::<Vec<_>>();
    let sources = [
        (
            "scanner pools",
            pools
                .scans
                .iter()
                .map(|scan| scan.geometry_ordinal)
                .collect::<Vec<_>>(),
        ),
        (
            "bar slices",
            bars.scans
                .iter()
                .map(|scan| scan.geometry_ordinal)
                .collect(),
        ),
        (
            "competitor slices",
            competitors
                .scans
                .iter()
                .map(|scan| scan.geometry_ordinal)
                .collect(),
        ),
    ];
    for (source, actual) in sources {
        if actual != expected {
            return Err(NativeHeadsSeedLookupError::ScanOrder {
                source,
                system_id,
                staff_id,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: HeadTemplateBounds = HeadTemplateBounds {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };

    #[test]
    fn location_search_is_y_then_x_strict_and_abandons_only_at_nominal() {
        let distances = [0.5, 0.3, 0.3, 0.2, 1.0, 0.1];
        let mut cursor = 0;
        let exploration = explore_locations(10, 20, &[0, -1], &[0, -1, 1], 0.4, 1.0, |_, _| {
            let distance = distances[cursor];
            cursor += 1;
            Ok::<_, ()>(LocationEvaluation {
                slim_bounds: BOX,
                outcome: NativeHeadSeedAttemptOutcome::Evaluated { distance },
            })
        })
        .unwrap();

        assert_eq!(
            exploration
                .attempts
                .iter()
                .map(|attempt| (attempt.x, attempt.y))
                .collect::<Vec<_>>(),
            vec![(10, 20), (9, 20), (11, 20), (10, 19), (9, 19), (11, 19)]
        );
        assert_eq!(exploration.best_replacement_count, 3);
        assert_eq!(
            exploration
                .attempts
                .iter()
                .filter(|attempt| attempt.selected_as_best)
                .map(|attempt| attempt.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
        assert_eq!(exploration.best.unwrap().attempt_ordinal, 5);
        assert!(!exploration.abandoned);

        let blocked = explore_locations(10, 20, &[0, 1], &[0, 1], 0.4, 1.0, |_, _| {
            Ok::<_, ()>(LocationEvaluation {
                slim_bounds: BOX,
                outcome: NativeHeadSeedAttemptOutcome::BarBlocked {
                    obstacle_pool_index: 7,
                },
            })
        })
        .unwrap();
        assert_eq!(blocked.attempts.len(), 1);
        assert!(blocked.abandoned);
        assert_eq!(blocked.performance.bar_blocks, 1);
        assert_eq!(blocked.performance.nominal_abandons, 1);
    }

    #[test]
    fn nominal_bad_distance_abandons_but_a_non_nominal_bad_distance_does_not() {
        let abandoned = explore_locations(0, 0, &[0], &[0, 1], 0.4, 1.0, |_, _| {
            Ok::<_, ()>(LocationEvaluation {
                slim_bounds: BOX,
                outcome: NativeHeadSeedAttemptOutcome::Evaluated { distance: 1.0 },
            })
        })
        .unwrap();
        assert!(abandoned.abandoned);
        assert_eq!(abandoned.attempts.len(), 1);

        let mut cursor = 0;
        let retained = explore_locations(0, 0, &[0], &[0, 1], 0.4, 1.0, |_, _| {
            let distance = [0.9, 1.0][cursor];
            cursor += 1;
            Ok::<_, ()>(LocationEvaluation {
                slim_bounds: BOX,
                outcome: NativeHeadSeedAttemptOutcome::Evaluated { distance },
            })
        })
        .unwrap();
        assert!(!retained.abandoned);
        assert_eq!(retained.attempts.len(), 2);
    }

    #[test]
    fn line_intersection_and_tally_preserve_java_arithmetic() {
        let x = java_line_x_at_y((10.0, 0.0), (14.0, 20.0), 7.5);
        assert_eq!(x, 11.5);
        assert_eq!(java_rint_to_i32(10.5), 10);
        assert_eq!(java_rint_to_i32(11.5), 12);

        let bounds = HeadTemplateBounds {
            x: 6,
            y: 0,
            width: 5,
            height: 1,
        };
        assert_eq!(tally_dx(NativeHeadSeedSide::Left, bounds, 11), -4.5);
        assert_eq!(tally_dx(NativeHeadSeedSide::Right, bounds, 11), 1.5);
    }

    #[test]
    fn canonical_double_format_matches_java_to_hex_string_and_long_bits() {
        assert_eq!(
            java_hex_double(0.4),
            "0x1.999999999999ap-2/3fd999999999999a"
        );
        assert_eq!(java_hex_double(1.0), "0x1.0p0/3ff0000000000000");
        assert_eq!(java_hex_double(-0.0), "-0x0.0p0/8000000000000000");
        assert_eq!(
            java_hex_double(f64::from_bits(1)),
            "0x0.0000000000001p-1022/0000000000000001"
        );
    }

    #[test]
    fn tablature_staffs_are_omitted_from_seed_lookup() {
        assert!(!staff_runs_seed_lookup(true));
        assert!(staff_runs_seed_lookup(false));
    }
}
