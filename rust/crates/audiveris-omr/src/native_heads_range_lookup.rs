// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-shaped, identity-free port of Java `Scanner.lookupRange()`.
//!
//! The pass streams the very large x-scan and shape-attempt traces into the
//! oracle's canonical FNV hashes. Only compact spot provenance and provisional
//! pre-aggregation heads are retained. Aggregation, seed-conflict filtering,
//! glyph retrieval, and SIG identity are intentionally later boundaries.
//! Finalized seed heads are likewise absent here: Java inserts them into the
//! live competitor list, but `Scanner.overlap` explicitly skips every
//! `HeadInter`; they first affect the later seed-conflict filter.

use std::{error::Error, fmt};

use audiveris_image::{glyph_factory::GlyphComponent, run_table::Orientation};

use crate::{
    head_scanner::{HeadScannerError, HeadScannerParameters},
    head_scanner_slices::JavaRectangle,
    head_template::{
        HEAD_INTRINSIC_RATIO, HeadTemplateAnchor, HeadTemplateBounds, HeadTemplateEvaluationError,
        HeadTemplateShape, MIN_INTER_GRADE, head_grade_of, impact_of,
    },
    head_template_overlap::{
        horizontal_parallelogram_intersects_rectangle, vertical_ribbon_intersects_rectangle,
    },
    heads_step::NeutralDistanceTable,
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
    native_heads_seed_lookup::MIN_HOLE_WHITE_RATIO,
};

/// Java `ChamferDistance.DEFAULT_NORMALIZER`.
pub const DEFAULT_CHAMFER_NORMALIZER: i32 = 3;
/// Java `NoteHeadsBuilder.Constants.stemLessBoost`.
pub const STEMLESS_BOOST: f64 = 0.0;
/// Java `Grades.minContextualGrade`.
pub const MIN_CONTEXTUAL_GRADE: f64 = 0.5;

/// Immutable products consumed before range aggregation.
pub struct NativeHeadsRangeLookupInput<'a> {
    pub heads: &'a NativeHeadsPrologRecognition,
    pub scanners: &'a NativeHeadsScannerRecognition,
    pub pools: &'a NativeHeadScannerPoolsRecognition,
    pub obstacles: &'a NativeHeadsBarObstaclePool,
    pub bar_slices: &'a NativeHeadScannerBarSlicesRecognition,
    pub competitors: &'a NativeHeadsCompetitorPool,
    pub competitor_slices: &'a NativeHeadScannerCompetitorSlicesRecognition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsRangeLookupRecognition {
    pub systems: Vec<NativeHeadRangeLookupSystem>,
    pub performance: NativeHeadRangePerformance,
    pub counts: NativeHeadRangeCounts,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadRangeLookupSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadRangeLookupStaff>,
    pub performance: NativeHeadRangePerformance,
    pub counts: NativeHeadRangeCounts,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadRangeLookupStaff {
    pub staff_id: usize,
    pub scans: Vec<NativeHeadRangeLookupScan>,
    pub performance: NativeHeadRangePerformance,
    pub counts: NativeHeadRangeCounts,
    pub hashes: NativeHeadRangeKernelHashes,
}

/// One range-phase scanner invocation. Empty integer ranges are omitted, as in Java.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadRangeLookupScan {
    pub geometry_ordinal: usize,
    pub source: NativeHeadScannerSource,
    pub pitch: i32,
    pub scan_left: i32,
    pub scan_right: i32,
    pub spots: Vec<NativeHeadRangeSpotEvidence>,
    pub candidates: Vec<NativeHeadRangeCandidate>,
    /// Best below-floor matches, one per interline-wide x bucket.
    pub subfloor: Vec<NativeHeadRangeSubfloor>,
    pub performance: NativeHeadRangePerformance,
    pub counts: NativeHeadRangeCounts,
    pub hashes: NativeHeadRangeKernelHashes,
}

/// The best below-floor match at one scan position: template evidence that
/// Java silently discards. Retained (bucketed per interline of x) so a
/// recall investigation can distinguish "never proposed" from "proposed but
/// under the floor" without a recognition re-run. Never enters the pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeSubfloor {
    pub shape: HeadTemplateShape,
    pub pitch: i32,
    pub x0: i32,
    pub y0: i32,
    pub bounds: HeadTemplateBounds,
    pub grade: f64,
    pub reason: NativeHeadSubfloorReason,
}

/// Which floor rejected the retained match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadSubfloorReason {
    /// Intrinsic grade under `MIN_INTER_GRADE`.
    BelowMinimumGrade,
    /// A stemless shape (whole/breve) under its stricter distance floor.
    WeakStemless,
}

/// One spot from Java's stable x-sorted scanner slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadRangeSpotEvidence {
    pub geometry_ordinal: usize,
    pub spot_ordinal: usize,
    pub pool_index: usize,
    pub source_ordinal: usize,
    pub bounds: JavaRectangle,
    pub weight: usize,
    pub run_digest: u64,
    /// Exact OpenJDK `Rectangle.grow(interline, 0)` result.
    pub expanded_bounds: JavaRectangle,
    /// Inclusive intersection with the scanner range, when non-empty.
    pub effective_x: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadRangeShapeChoice {
    All,
    Hollow,
}

impl NativeHeadRangeShapeChoice {
    const fn java_name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Hollow => "hollow",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeBestLocation {
    pub y_offset_ordinal: usize,
    pub y_offset: i32,
    pub x: i32,
    pub y: i32,
    pub distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeHoleEvidence {
    pub ratio: f64,
    pub threshold: f64,
    pub converted_to_void: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeStemlessEvidence {
    pub applicable: bool,
    pub intrinsic_grade: f64,
    pub boost: f64,
    pub contextual_grade: f64,
    pub minimum_contextual_grade: f64,
    pub rejected: bool,
}

/// A provisional `HeadInter` immediately before Java `aggregateMatches`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeCandidate {
    /// Dense raw-candidate ordinal within the staff.
    pub ordinal: usize,
    /// Dense shape-attempt ordinal within the staff.
    pub attempt_ordinal: usize,
    pub geometry_ordinal: usize,
    pub x0: i32,
    pub y0: i32,
    pub black_relevant: bool,
    pub shape_choice: NativeHeadRangeShapeChoice,
    pub shape_ordinal: usize,
    pub original_shape: HeadTemplateShape,
    pub shape: HeadTemplateShape,
    pub best: NativeHeadRangeBestLocation,
    pub hole: Option<NativeHeadRangeHoleEvidence>,
    pub stemless: NativeHeadRangeStemlessEvidence,
    pub anchor: HeadTemplateAnchor,
    pub pitch: f64,
    pub bounds: HeadTemplateBounds,
    pub raw_distance_impact: f64,
    pub distance_impact: f64,
    pub grade: f64,
    pub minimum_grade: f64,
}

/// Java range debug counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeHeadRangePerformance {
    pub bar_blocks: usize,
    pub competitor_blocks: usize,
    pub evaluations: usize,
    pub nominal_abandons: usize,
}

impl NativeHeadRangePerformance {
    fn include(&mut self, other: Self) {
        self.bar_blocks += other.bar_blocks;
        self.competitor_blocks += other.competitor_blocks;
        self.evaluations += other.evaluations;
        self.nominal_abandons += other.nominal_abandons;
    }

    fn observe(&mut self, outcome: LocationOutcome) {
        match outcome {
            LocationOutcome::BarBlocked => self.bar_blocks += 1,
            LocationOutcome::CompetitorBlocked => self.competitor_blocks += 1,
            LocationOutcome::Evaluated { .. } => self.evaluations += 1,
        }
    }
}

/// Compact counters corresponding to the range oracle staff summary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeHeadRangeCounts {
    pub spot_slices: usize,
    pub x_scans: usize,
    pub safety_skips: usize,
    pub shape_attempts: usize,
    pub raw_candidates: usize,
    pub safe_all: usize,
    pub safe_hollow: usize,
    pub right_limit: usize,
    pub above_image: usize,
    pub below_image: usize,
    pub foreground_gap: usize,
    pub black_to_void: usize,
    pub abandoned_bar: usize,
    pub abandoned_competitor: usize,
    pub abandoned_nominal: usize,
    pub no_best: usize,
    pub weak_stemless: usize,
    pub below_minimum_grade: usize,
    pub provisional: usize,
}

impl NativeHeadRangeCounts {
    fn include(&mut self, other: Self) {
        self.spot_slices += other.spot_slices;
        self.x_scans += other.x_scans;
        self.safety_skips += other.safety_skips;
        self.shape_attempts += other.shape_attempts;
        self.raw_candidates += other.raw_candidates;
        self.safe_all += other.safe_all;
        self.safe_hollow += other.safe_hollow;
        self.right_limit += other.right_limit;
        self.above_image += other.above_image;
        self.below_image += other.below_image;
        self.foreground_gap += other.foreground_gap;
        self.black_to_void += other.black_to_void;
        self.abandoned_bar += other.abandoned_bar;
        self.abandoned_competitor += other.abandoned_competitor;
        self.abandoned_nominal += other.abandoned_nominal;
        self.no_best += other.no_best;
        self.weak_stemless += other.weak_stemless;
        self.below_minimum_grade += other.below_minimum_grade;
        self.provisional += other.provisional;
    }
}

/// FNV-1a-64 digests of the four identity-free oracle streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadRangeKernelHashes {
    pub spot: u64,
    pub scan: u64,
    pub attempt: u64,
    pub raw_candidate: u64,
}

impl Default for NativeHeadRangeKernelHashes {
    fn default() -> Self {
        Self {
            spot: FNV_OFFSET,
            scan: FNV_OFFSET,
            attempt: FNV_OFFSET,
            raw_candidate: FNV_OFFSET,
        }
    }
}

#[derive(Debug)]
pub enum NativeHeadsRangeLookupError {
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
    RangeSchedule {
        system_id: usize,
        staff_id: usize,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    RangeGeometry {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        expected_left: i32,
        actual_left: i32,
        expected_right: i32,
        actual_right: i32,
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
    InvalidDistanceTable {
        width: usize,
        height: usize,
        normalizer: i32,
        values: usize,
    },
    DistanceDimensionsTooLarge,
    RangeLength {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        left: i32,
        right: i32,
    },
    SpotPoolIndex {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        pool_index: usize,
    },
    SpotSource {
        system_id: usize,
        source_ordinal: usize,
    },
    MissingSpotComponent {
        system_id: usize,
        source_ordinal: usize,
    },
    SpotBounds {
        system_id: usize,
        source_ordinal: usize,
        pool: JavaRectangle,
        component: JavaRectangle,
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
    DistanceCoordinate {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        x: i32,
        y: i32,
    },
    ScanProgressOverflow {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        x: i32,
        next_x: i32,
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

impl fmt::Display for NativeHeadsRangeLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range lookup {source} system order {actual:?}, expected {expected:?}"
            ),
            Self::StaffOrder {
                source,
                system_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} {source} staff order {actual:?}, expected {expected:?}"
            ),
            Self::ScanOrder {
                source,
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} {source} scan order {actual:?}, expected {expected:?}"
            ),
            Self::RangeSchedule {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} schedule {actual:?}, expected {expected:?}"
            ),
            Self::RangeGeometry {
                system_id,
                staff_id,
                geometry_ordinal,
                expected_left,
                actual_left,
                expected_right,
                actual_right,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} range {actual_left}..{actual_right}, expected {expected_left}..{expected_right}"
            ),
            Self::MissingCatalogSelection {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS range lookup has no catalog selection for system {system_id} staff {staff_id}"
            ),
            Self::CatalogSelectionMismatch {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS range lookup catalog selection disagrees for system {system_id} staff {staff_id}"
            ),
            Self::MissingCatalog {
                system_id,
                staff_id,
                catalog_ordinal,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} references missing catalog {catalog_ordinal}"
            ),
            Self::InvalidDistanceTable {
                width,
                height,
                normalizer,
                values,
            } => write!(
                formatter,
                "HEADS range lookup distance table {width}x{height}, normalizer {normalizer}, has {values} values"
            ),
            Self::DistanceDimensionsTooLarge => {
                write!(
                    formatter,
                    "HEADS range lookup distance dimensions exceed Java int"
                )
            }
            Self::RangeLength {
                system_id,
                staff_id,
                geometry_ordinal,
                left,
                right,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} invalid range length {left}..{right}"
            ),
            Self::SpotPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} references missing spot-pool index {pool_index}"
            ),
            Self::SpotSource {
                system_id,
                source_ordinal,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} references missing spot source {source_ordinal}"
            ),
            Self::MissingSpotComponent {
                system_id,
                source_ordinal,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} spot {source_ordinal} has no native component"
            ),
            Self::SpotBounds {
                system_id,
                source_ordinal,
                pool,
                component,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} spot {source_ordinal} pool bounds {pool:?} disagree with component {component:?}"
            ),
            Self::BarPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} references missing bar-pool index {pool_index}"
            ),
            Self::CompetitorPoolIndex {
                system_id,
                staff_id,
                geometry_ordinal,
                pool_index,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} references missing competitor-pool index {pool_index}"
            ),
            Self::DistanceCoordinate {
                system_id,
                staff_id,
                geometry_ordinal,
                x,
                y,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} probes invalid distance coordinate ({x},{y})"
            ),
            Self::ScanProgressOverflow {
                system_id,
                staff_id,
                geometry_ordinal,
                x,
                next_x,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} cannot progress from {x} to {next_x}"
            ),
            Self::Geometry {
                system_id,
                staff_id,
                geometry_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} geometry failed: {source}"
            ),
            Self::Template {
                system_id,
                staff_id,
                geometry_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS range lookup system {system_id} staff {staff_id} scanner {geometry_ordinal} template failed: {source}"
            ),
        }
    }
}

impl Error for NativeHeadsRangeLookupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Geometry { source, .. } => Some(source),
            Self::Template { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Run Java's x-driven search through provisional `HeadInter` construction.
pub fn recognize_native_heads_range_lookup(
    input: NativeHeadsRangeLookupInput<'_>,
) -> Result<NativeHeadsRangeLookupRecognition, NativeHeadsRangeLookupError> {
    validate_distance_table(&input.heads.distance_table)?;
    validate_system_topology(&input)?;

    let mut recognition_performance = NativeHeadRangePerformance::default();
    let mut recognition_counts = NativeHeadRangeCounts::default();
    let mut systems = Vec::with_capacity(input.scanners.systems.len());

    for system_ordinal in 0..input.scanners.systems.len() {
        let scanner_system = &input.scanners.systems[system_ordinal];
        let pool_system = &input.pools.systems[system_ordinal];
        let obstacle_system = &input.obstacles.systems[system_ordinal];
        let bar_slice_system = &input.bar_slices.systems[system_ordinal];
        let competitor_system = &input.competitors.systems[system_ordinal];
        let competitor_slice_system = &input.competitor_slices.systems[system_ordinal];
        let system_id = scanner_system.system_id;

        validate_staff_topology(
            system_id,
            scanner_system,
            pool_system,
            bar_slice_system,
            competitor_slice_system,
        )?;

        let mut system_performance = NativeHeadRangePerformance::default();
        let mut system_counts = NativeHeadRangeCounts::default();
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
            if scanner_staff.tablature {
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
                    .ok_or(NativeHeadsRangeLookupError::MissingCatalogSelection {
                        system_id,
                        staff_id,
                    })?;
                if scanner_staff.catalog_ordinal != Some(selection.catalog_ordinal)
                    || scanner_staff.point_size != Some(selection.point_size)
                    || scanner_staff.specific_interline != selection.specific_interline
                {
                    return Err(NativeHeadsRangeLookupError::CatalogSelectionMismatch {
                        system_id,
                        staff_id,
                    });
                }
                Some(
                    input
                        .heads
                        .template_catalogs
                        .get(selection.catalog_ordinal)
                        .ok_or(NativeHeadsRangeLookupError::MissingCatalog {
                            system_id,
                            staff_id,
                            catalog_ordinal: selection.catalog_ordinal,
                        })?,
                )
            };

            let schedule = scanner_staff
                .schedule
                .iter()
                .filter(|entry| entry.phase == NativeHeadScannerPhase::Range)
                .map(|entry| entry.geometry)
                .collect::<Vec<_>>();
            let expected_schedule = (0..scanner_staff.geometries.len()).collect::<Vec<_>>();
            if schedule != expected_schedule {
                return Err(NativeHeadsRangeLookupError::RangeSchedule {
                    system_id,
                    staff_id,
                    expected: expected_schedule,
                    actual: schedule,
                });
            }

            let mut staff_performance = NativeHeadRangePerformance::default();
            let mut staff_counts = NativeHeadRangeCounts::default();
            let mut staff_hashers = KernelHashers::default();
            let mut scans = Vec::with_capacity(scanner_staff.geometries.len());
            let mut attempt_ordinal = 0;
            let mut candidate_ordinal = 0;
            for &geometry_ordinal in &schedule {
                let geometry = &scanner_staff.geometries[geometry_ordinal];
                let expected_left = geometry.line.left_abscissa().max(scanner_staff.header_stop);
                let expected_right = geometry
                    .line
                    .right_abscissa()
                    .wrapping_sub(scanner_system.parameters.min_template_width);
                if geometry.range_left != expected_left || geometry.range_right != expected_right {
                    return Err(NativeHeadsRangeLookupError::RangeGeometry {
                        system_id,
                        staff_id,
                        geometry_ordinal,
                        expected_left,
                        actual_left: geometry.range_left,
                        expected_right,
                        actual_right: geometry.range_right,
                    });
                }
                if geometry.range_right < geometry.range_left {
                    scans.push(NativeHeadRangeLookupScan {
                        geometry_ordinal,
                        source: geometry.source.clone(),
                        pitch: geometry.pitch,
                        scan_left: geometry.range_left,
                        scan_right: geometry.range_right,
                        spots: Vec::new(),
                        candidates: Vec::new(),
                        subfloor: Vec::new(),
                        performance: NativeHeadRangePerformance::default(),
                        counts: NativeHeadRangeCounts::default(),
                        hashes: NativeHeadRangeKernelHashes::default(),
                    });
                    continue;
                }

                let scan = lookup_scan(
                    ScanInput {
                        system_id,
                        staff_id,
                        geometry_ordinal,
                        geometry,
                        parameters: &scanner_system.parameters,
                        catalog: catalog.expect("non-empty scanner geometry has a checked catalog"),
                        distances: &input.heads.distance_table,
                        heads: input.heads,
                        pool_system,
                        spot_indices: &pool_staff.scans[geometry_ordinal].spot_indices,
                        obstacle_pool: &obstacle_system.frozen_by_ordinate,
                        bar_indices: &bar_slice_staff.scans[geometry_ordinal].frozen_bar_indices,
                        competitor_pool: &competitor_system.accepted_by_ordinate,
                        competitor_indices: &competitor_slice_staff.scans[geometry_ordinal]
                            .competitor_indices,
                        attempt_ordinal_base: attempt_ordinal,
                        candidate_ordinal_base: candidate_ordinal,
                    },
                    &mut staff_hashers,
                )?;
                attempt_ordinal += scan.counts.shape_attempts;
                candidate_ordinal += scan.candidates.len();
                staff_performance.include(scan.performance);
                staff_counts.include(scan.counts);
                scans.push(scan);
            }

            let hashes = staff_hashers.finish();
            system_performance.include(staff_performance);
            system_counts.include(staff_counts);
            staffs.push(NativeHeadRangeLookupStaff {
                staff_id,
                scans,
                performance: staff_performance,
                counts: staff_counts,
                hashes,
            });
        }

        recognition_performance.include(system_performance);
        recognition_counts.include(system_counts);
        systems.push(NativeHeadRangeLookupSystem {
            system_id,
            staffs,
            performance: system_performance,
            counts: system_counts,
        });
    }

    Ok(NativeHeadsRangeLookupRecognition {
        systems,
        performance: recognition_performance,
        counts: recognition_counts,
    })
}

struct ScanInput<'a> {
    system_id: usize,
    staff_id: usize,
    geometry_ordinal: usize,
    geometry: &'a NativeHeadScannerGeometry,
    parameters: &'a HeadScannerParameters,
    catalog: &'a crate::head_template::HeadTemplateCatalog,
    distances: &'a NeutralDistanceTable,
    heads: &'a NativeHeadsPrologRecognition,
    pool_system: &'a crate::native_heads_scanner_pools::NativeHeadScannerPoolSystem,
    spot_indices: &'a [usize],
    obstacle_pool: &'a [crate::native_heads_obstacles::NativeHeadsBarObstacle],
    bar_indices: &'a [usize],
    competitor_pool: &'a [crate::native_heads_competitors::NativeHeadsCompetitor],
    competitor_indices: &'a [usize],
    attempt_ordinal_base: usize,
    candidate_ordinal_base: usize,
}

fn lookup_scan(
    input: ScanInput<'_>,
    staff_hashers: &mut KernelHashers,
) -> Result<NativeHeadRangeLookupScan, NativeHeadsRangeLookupError> {
    let span = i64::from(input.geometry.range_right) - i64::from(input.geometry.range_left) + 1;
    let relevant_len = i32::try_from(span).and_then(usize::try_from).map_err(|_| {
        NativeHeadsRangeLookupError::RangeLength {
            system_id: input.system_id,
            staff_id: input.staff_id,
            geometry_ordinal: input.geometry_ordinal,
            left: input.geometry.range_left,
            right: input.geometry.range_right,
        }
    })?;
    let mut black_relevants = vec![false; relevant_len];
    let mut spots = Vec::with_capacity(input.spot_indices.len());
    let mut scan_hashers = KernelHashers::default();

    for (spot_ordinal, &pool_index) in input.spot_indices.iter().enumerate() {
        let pool_spot = input.pool_system.spots.get(pool_index).ok_or(
            NativeHeadsRangeLookupError::SpotPoolIndex {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                pool_index,
            },
        )?;
        let spot = input.heads.spots.get(pool_spot.source_ordinal).ok_or(
            NativeHeadsRangeLookupError::SpotSource {
                system_id: input.system_id,
                source_ordinal: pool_spot.source_ordinal,
            },
        )?;
        let component =
            spot.component
                .as_ref()
                .ok_or(NativeHeadsRangeLookupError::MissingSpotComponent {
                    system_id: input.system_id,
                    source_ordinal: pool_spot.source_ordinal,
                })?;
        let component_bounds = component_rectangle(component);
        if component_bounds != pool_spot.bounds {
            return Err(NativeHeadsRangeLookupError::SpotBounds {
                system_id: input.system_id,
                source_ordinal: pool_spot.source_ordinal,
                pool: pool_spot.bounds,
                component: component_bounds,
            });
        }

        let expanded_bounds = java_rectangle_grow(pool_spot.bounds, input.geometry.interline, 0);
        mark_black_relevants(
            &mut black_relevants,
            input.geometry.range_left,
            expanded_bounds,
        );
        let effective_left = input.geometry.range_left.max(expanded_bounds.x);
        let expanded_right = expanded_bounds
            .x
            .wrapping_add(expanded_bounds.width)
            .wrapping_sub(1);
        let effective_right = input.geometry.range_right.min(expanded_right);
        let effective_x =
            (effective_right >= effective_left).then_some((effective_left, effective_right));
        let evidence = NativeHeadRangeSpotEvidence {
            geometry_ordinal: input.geometry_ordinal,
            spot_ordinal,
            pool_index,
            source_ordinal: pool_spot.source_ordinal,
            bounds: pool_spot.bounds,
            weight: component.weight,
            run_digest: glyph_component_run_digest(component),
            expanded_bounds,
            effective_x,
        };
        let record = native_head_range_spot_kernel_record(
            input.system_id,
            input.staff_id,
            &input.geometry.source,
            &evidence,
        );
        scan_hashers.add_spot(&record);
        staff_hashers.add_spot(&record);
        spots.push(evidence);
    }

    let image_width = i32::try_from(input.distances.width)
        .map_err(|_| NativeHeadsRangeLookupError::DistanceDimensionsTooLarge)?;
    let image_height = i32::try_from(input.distances.height)
        .map_err(|_| NativeHeadsRangeLookupError::DistanceDimensionsTooLarge)?;
    let mut performance = NativeHeadRangePerformance::default();
    let mut counts = NativeHeadRangeCounts {
        spot_slices: spots.len(),
        ..NativeHeadRangeCounts::default()
    };
    let mut candidates = Vec::new();
    let mut subfloor_raw: Vec<NativeHeadRangeSubfloor> = Vec::new();
    let mut x0 = input.geometry.range_left;

    loop {
        counts.x_scans += 1;
        let y0 = input.geometry.theoretical_ordinate(x0).map_err(|source| {
            NativeHeadsRangeLookupError::Geometry {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                source,
            }
        })?;
        let probe_x = x0.wrapping_add(input.parameters.template_half);
        let (safety, probe_value) = classify_safety(
            probe_x,
            y0,
            image_width,
            image_height,
            input.parameters.template_half,
            input.distances,
            input.system_id,
            input.staff_id,
            input.geometry_ordinal,
        )?;

        if safety != RangeSafety::Safe {
            counts.safety_skips += 1;
            counts.observe_safety(safety, None);
            let jump = input.parameters.template_half.wrapping_mul(2);
            let next_x = x0.wrapping_add(jump);
            let record = range_scan_kernel_record(
                &input,
                x0,
                y0,
                probe_x,
                probe_value,
                safety,
                next_x,
                false,
                None,
                &[],
            );
            scan_hashers.add_scan(&record);
            staff_hashers.add_scan(&record);
            if next_x <= x0 && x0 < input.geometry.range_right {
                return Err(NativeHeadsRangeLookupError::ScanProgressOverflow {
                    system_id: input.system_id,
                    staff_id: input.staff_id,
                    geometry_ordinal: input.geometry_ordinal,
                    x: x0,
                    next_x,
                });
            }
            if next_x > input.geometry.range_right {
                break;
            }
            x0 = next_x;
            continue;
        }

        let relevant_index = usize::try_from(x0.wrapping_sub(input.geometry.range_left))
            .expect("x remains within the validated scanner range");
        let black_relevant = black_relevants[relevant_index];
        let (shape_choice, shapes) = if black_relevant {
            (
                NativeHeadRangeShapeChoice::All,
                input.geometry.all_shapes.as_slice(),
            )
        } else {
            (
                NativeHeadRangeShapeChoice::Hollow,
                input.geometry.hollow_shapes.as_slice(),
            )
        };
        counts.observe_safety(safety, Some(shape_choice));
        let next_x = x0.wrapping_add(1);
        let record = range_scan_kernel_record(
            &input,
            x0,
            y0,
            probe_x,
            probe_value,
            safety,
            next_x,
            black_relevant,
            Some(shape_choice),
            shapes,
        );
        scan_hashers.add_scan(&record);
        staff_hashers.add_scan(&record);

        for (shape_ordinal, &original_shape) in shapes.iter().enumerate() {
            let attempt_ordinal = input.attempt_ordinal_base + counts.shape_attempts;
            counts.shape_attempts += 1;
            let attempt = search_shape(
                &input,
                x0,
                y0,
                black_relevant,
                shape_choice,
                shape_ordinal,
                original_shape,
                attempt_ordinal,
                input.candidate_ordinal_base + candidates.len(),
            )?;
            performance.include(attempt.performance);
            counts.observe_outcome(attempt.outcome, attempt.black_to_void);
            let record = range_attempt_kernel_record(
                &input,
                x0,
                y0,
                black_relevant,
                shape_choice,
                shape_ordinal,
                original_shape,
                &attempt,
            );
            scan_hashers.add_attempt(&record);
            staff_hashers.add_attempt(&record);
            if let Some(candidate) = attempt.candidate {
                let record = native_head_range_raw_kernel_record(
                    input.system_id,
                    input.staff_id,
                    &input.geometry.source,
                    &candidate,
                );
                scan_hashers.add_raw_candidate(&record);
                staff_hashers.add_raw_candidate(&record);
                candidates.push(candidate);
            }
            if let Some(record) = attempt.subfloor {
                subfloor_raw.push(record);
            }
        }

        if x0 == input.geometry.range_right {
            break;
        }
        if next_x <= x0 {
            return Err(NativeHeadsRangeLookupError::ScanProgressOverflow {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                x: x0,
                next_x,
            });
        }
        x0 = next_x;
    }

    counts.raw_candidates = candidates.len();
    // Keep only the strongest below-floor match per interline of x, so the
    // retained evidence stays a bounded shadow of the scan, not a firehose.
    let bucket_width = input.geometry.interline.max(1);
    let mut best_per_bucket: std::collections::BTreeMap<i32, NativeHeadRangeSubfloor> =
        std::collections::BTreeMap::new();
    for record in subfloor_raw {
        let bucket = record.x0.div_euclid(bucket_width);
        match best_per_bucket.entry(bucket) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if record.grade > slot.get().grade {
                    slot.insert(record);
                }
            }
        }
    }
    let subfloor = best_per_bucket.into_values().collect();
    Ok(NativeHeadRangeLookupScan {
        geometry_ordinal: input.geometry_ordinal,
        source: input.geometry.source.clone(),
        pitch: input.geometry.pitch,
        scan_left: input.geometry.range_left,
        scan_right: input.geometry.range_right,
        spots,
        candidates,
        subfloor,
        performance,
        counts,
        hashes: scan_hashers.finish(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RangeSafety {
    Safe,
    RightLimit,
    AboveImage,
    BelowImage,
    ForegroundGap,
}

impl RangeSafety {
    const fn java_name(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::RightLimit => "right-limit",
            Self::AboveImage => "above-image",
            Self::BelowImage => "below-image",
            Self::ForegroundGap => "foreground-gap",
        }
    }
}

impl NativeHeadRangeCounts {
    fn observe_safety(&mut self, safety: RangeSafety, choice: Option<NativeHeadRangeShapeChoice>) {
        match (safety, choice) {
            (RangeSafety::Safe, Some(NativeHeadRangeShapeChoice::All)) => self.safe_all += 1,
            (RangeSafety::Safe, Some(NativeHeadRangeShapeChoice::Hollow)) => {
                self.safe_hollow += 1;
            }
            (RangeSafety::RightLimit, _) => self.right_limit += 1,
            (RangeSafety::AboveImage, _) => self.above_image += 1,
            (RangeSafety::BelowImage, _) => self.below_image += 1,
            (RangeSafety::ForegroundGap, _) => self.foreground_gap += 1,
            (RangeSafety::Safe, None) => unreachable!("safe scan has a shape choice"),
        }
    }

    fn observe_outcome(&mut self, outcome: AttemptOutcome, black_to_void: Option<bool>) {
        if black_to_void == Some(true) {
            self.black_to_void += 1;
        }
        match outcome {
            AttemptOutcome::AbandonedBar => self.abandoned_bar += 1,
            AttemptOutcome::AbandonedCompetitor => self.abandoned_competitor += 1,
            AttemptOutcome::AbandonedNominal => self.abandoned_nominal += 1,
            AttemptOutcome::NoBest => self.no_best += 1,
            AttemptOutcome::WeakStemless => self.weak_stemless += 1,
            AttemptOutcome::BelowMinimumGrade => self.below_minimum_grade += 1,
            AttemptOutcome::Provisional => self.provisional += 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_safety(
    probe_x: i32,
    y0: i32,
    image_width: i32,
    image_height: i32,
    template_half: i32,
    distances: &NeutralDistanceTable,
    system_id: usize,
    staff_id: usize,
    geometry_ordinal: usize,
) -> Result<(RangeSafety, Option<i32>), NativeHeadsRangeLookupError> {
    if probe_x >= image_width {
        return Ok((RangeSafety::RightLimit, None));
    }
    if y0 < 0 {
        return Ok((RangeSafety::AboveImage, None));
    }
    if y0 >= image_height {
        return Ok((RangeSafety::BelowImage, None));
    }
    if probe_x < 0 {
        return Err(NativeHeadsRangeLookupError::DistanceCoordinate {
            system_id,
            staff_id,
            geometry_ordinal,
            x: probe_x,
            y: y0,
        });
    }
    let value = distances
        .value(
            usize::try_from(probe_x).expect("non-negative probe x"),
            usize::try_from(y0).expect("non-negative probe y"),
        )
        .ok_or(NativeHeadsRangeLookupError::DistanceCoordinate {
            system_id,
            staff_id,
            geometry_ordinal,
            x: probe_x,
            y: y0,
        })?;
    if value / DEFAULT_CHAMFER_NORMALIZER > template_half {
        Ok((RangeSafety::ForegroundGap, Some(value)))
    } else {
        Ok((RangeSafety::Safe, Some(value)))
    }
}

#[derive(Clone, Copy)]
enum LocationOutcome {
    BarBlocked,
    CompetitorBlocked,
    Evaluated { distance: f64 },
}

struct ShapeSearch {
    evaluations: String,
    performance: NativeHeadRangePerformance,
    best_updates: usize,
    best: Option<NativeHeadRangeBestLocation>,
    final_shape: HeadTemplateShape,
    black_to_void: Option<bool>,
    weak_stemless: bool,
    outcome: AttemptOutcome,
    candidate: Option<NativeHeadRangeCandidate>,
    subfloor: Option<NativeHeadRangeSubfloor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttemptOutcome {
    AbandonedBar,
    AbandonedCompetitor,
    AbandonedNominal,
    NoBest,
    WeakStemless,
    BelowMinimumGrade,
    Provisional,
}

impl AttemptOutcome {
    const fn java_name(self) -> &'static str {
        match self {
            Self::AbandonedBar => "abandoned-bar",
            Self::AbandonedCompetitor => "abandoned-competitor",
            Self::AbandonedNominal => "abandoned-nominal",
            Self::NoBest => "no-best",
            Self::WeakStemless => "weak-stemless",
            Self::BelowMinimumGrade => "min-grade",
            Self::Provisional => "provisional",
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn search_shape(
    input: &ScanInput<'_>,
    x0: i32,
    y0: i32,
    black_relevant: bool,
    shape_choice: NativeHeadRangeShapeChoice,
    shape_ordinal: usize,
    original_shape: HeadTemplateShape,
    attempt_ordinal: usize,
    candidate_ordinal: usize,
) -> Result<ShapeSearch, NativeHeadsRangeLookupError> {
    let template = input.catalog.template(original_shape);
    let anchor = HeadTemplateAnchor::MiddleLeft;
    let mut evaluations = String::new();
    let mut performance = NativeHeadRangePerformance::default();
    let mut best = None;
    let mut best_updates = 0;
    let mut abandoned = None;

    for (y_offset_ordinal, &y_offset) in input.geometry.y_offsets.iter().enumerate() {
        let y = y0.wrapping_add(y_offset);
        let slim_bounds = template
            .slim_bounds_at(x0, y, anchor)
            .map_err(|source| template_error(input, source))?;
        let rectangle = java_rectangle(slim_bounds);
        let outcome = if let Some(_pool_index) =
            first_bar_overlap(rectangle, input.obstacle_pool, input.bar_indices, input)?
        {
            LocationOutcome::BarBlocked
        } else if let Some(_pool_index) = first_competitor_overlap(
            rectangle,
            input.competitor_pool,
            input.competitor_indices,
            input,
        )? {
            LocationOutcome::CompetitorBlocked
        } else {
            let distance = template
                .evaluate(x0, y, Some(anchor), input.distances)
                .map_err(|source| template_error(input, source))?;
            LocationOutcome::Evaluated { distance }
        };
        performance.observe(outcome);
        if !evaluations.is_empty() {
            evaluations.push(',');
        }
        use std::fmt::Write as _;
        match outcome {
            LocationOutcome::BarBlocked => {
                write!(evaluations, "{y_offset_ordinal}:{y_offset}:{y}:bar")
                    .expect("write to String");
            }
            LocationOutcome::CompetitorBlocked => {
                write!(evaluations, "{y_offset_ordinal}:{y_offset}:{y}:competitor")
                    .expect("write to String");
            }
            LocationOutcome::Evaluated { distance } => {
                write!(
                    evaluations,
                    "{y_offset_ordinal}:{y_offset}:{y}:evaluated:{}",
                    java_hex_double(distance)
                )
                .expect("write to String");
            }
        }

        let distance = match outcome {
            LocationOutcome::Evaluated { distance } => Some(distance),
            LocationOutcome::BarBlocked | LocationOutcome::CompetitorBlocked => None,
        };
        if let Some(distance) = distance
            && distance <= input.parameters.max_distance_low
            && best.is_none_or(|current: NativeHeadRangeBestLocation| current.distance > distance)
        {
            best = Some(NativeHeadRangeBestLocation {
                y_offset_ordinal,
                y_offset,
                x: x0,
                y,
                distance,
            });
            best_updates += 1;
        } else if y == y0
            && distance.is_none_or(|distance| distance >= input.parameters.really_bad_distance)
        {
            performance.nominal_abandons += 1;
            abandoned = Some(match outcome {
                LocationOutcome::BarBlocked => AttemptOutcome::AbandonedBar,
                LocationOutcome::CompetitorBlocked => AttemptOutcome::AbandonedCompetitor,
                LocationOutcome::Evaluated { .. } => AttemptOutcome::AbandonedNominal,
            });
            break;
        }
    }

    let mut final_shape = original_shape;
    let mut black_to_void = None;
    let mut weak_stemless = false;
    let mut candidate = None;
    let mut subfloor = None;
    let outcome = if let Some(outcome) = abandoned {
        outcome
    } else if let Some(best) = best {
        let hole = if original_shape == HeadTemplateShape::NoteheadBlack {
            let ratio = input
                .catalog
                .template(HeadTemplateShape::NoteheadVoid)
                .evaluate_hole(best.x, best.y, Some(anchor), input.distances)
                .map_err(|source| template_error(input, source))?;
            let converted_to_void = ratio >= MIN_HOLE_WHITE_RATIO;
            black_to_void = Some(converted_to_void);
            if converted_to_void {
                final_shape = HeadTemplateShape::NoteheadVoid;
            }
            Some(NativeHeadRangeHoleEvidence {
                ratio,
                threshold: MIN_HOLE_WHITE_RATIO,
                converted_to_void,
            })
        } else {
            None
        };
        let stemless = stemless_evidence(final_shape, best.distance);
        weak_stemless = stemless.rejected;
        if weak_stemless {
            let bounds = input
                .catalog
                .template(final_shape)
                .slim_bounds_at(best.x, best.y, anchor)
                .map_err(|source| template_error(input, source))?;
            subfloor = Some(NativeHeadRangeSubfloor {
                shape: final_shape,
                pitch: input.geometry.pitch,
                x0,
                y0,
                bounds,
                grade: head_grade_of(best.distance),
                reason: NativeHeadSubfloorReason::WeakStemless,
            });
            AttemptOutcome::WeakStemless
        } else {
            let raw_distance_impact = impact_of(best.distance);
            let distance_impact = raw_distance_impact.clamp(0.0, 1.0);
            let grade = head_grade_of(best.distance);
            if grade < MIN_INTER_GRADE {
                let bounds = input
                    .catalog
                    .template(final_shape)
                    .slim_bounds_at(best.x, best.y, anchor)
                    .map_err(|source| template_error(input, source))?;
                subfloor = Some(NativeHeadRangeSubfloor {
                    shape: final_shape,
                    pitch: input.geometry.pitch,
                    x0,
                    y0,
                    bounds,
                    grade,
                    reason: NativeHeadSubfloorReason::BelowMinimumGrade,
                });
                AttemptOutcome::BelowMinimumGrade
            } else {
                let bounds = input
                    .catalog
                    .template(final_shape)
                    .slim_bounds_at(best.x, best.y, anchor)
                    .map_err(|source| template_error(input, source))?;
                candidate = Some(NativeHeadRangeCandidate {
                    ordinal: candidate_ordinal,
                    attempt_ordinal,
                    geometry_ordinal: input.geometry_ordinal,
                    x0,
                    y0,
                    black_relevant,
                    shape_choice,
                    shape_ordinal,
                    original_shape,
                    shape: final_shape,
                    best,
                    hole,
                    stemless,
                    anchor,
                    pitch: f64::from(input.geometry.pitch),
                    bounds,
                    raw_distance_impact,
                    distance_impact,
                    grade,
                    minimum_grade: MIN_INTER_GRADE,
                });
                AttemptOutcome::Provisional
            }
        }
    } else {
        AttemptOutcome::NoBest
    };

    Ok(ShapeSearch {
        evaluations,
        performance,
        best_updates,
        best,
        final_shape,
        black_to_void,
        weak_stemless,
        outcome,
        candidate,
        subfloor,
    })
}

fn stemless_evidence(shape: HeadTemplateShape, distance: f64) -> NativeHeadRangeStemlessEvidence {
    let applicable = matches!(
        shape,
        HeadTemplateShape::WholeNote | HeadTemplateShape::Breve
    );
    let intrinsic_grade = head_grade_of(distance);
    let mut contextual_grade = intrinsic_grade;
    if applicable && contextual_grade < HEAD_INTRINSIC_RATIO {
        contextual_grade += STEMLESS_BOOST * (HEAD_INTRINSIC_RATIO - contextual_grade);
    }
    NativeHeadRangeStemlessEvidence {
        applicable,
        intrinsic_grade,
        boost: STEMLESS_BOOST,
        contextual_grade,
        minimum_contextual_grade: MIN_CONTEXTUAL_GRADE,
        rejected: applicable && contextual_grade < MIN_CONTEXTUAL_GRADE,
    }
}

fn template_error(
    input: &ScanInput<'_>,
    source: HeadTemplateEvaluationError,
) -> NativeHeadsRangeLookupError {
    NativeHeadsRangeLookupError::Template {
        system_id: input.system_id,
        staff_id: input.staff_id,
        geometry_ordinal: input.geometry_ordinal,
        source,
    }
}

fn first_bar_overlap(
    rectangle: JavaRectangle,
    obstacles: &[crate::native_heads_obstacles::NativeHeadsBarObstacle],
    indices: &[usize],
    input: &ScanInput<'_>,
) -> Result<Option<usize>, NativeHeadsRangeLookupError> {
    for &pool_index in indices {
        let obstacle =
            obstacles
                .get(pool_index)
                .ok_or(NativeHeadsRangeLookupError::BarPoolIndex {
                    system_id: input.system_id,
                    staff_id: input.staff_id,
                    geometry_ordinal: input.geometry_ordinal,
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
    input: &ScanInput<'_>,
) -> Result<Option<usize>, NativeHeadsRangeLookupError> {
    for &pool_index in indices {
        let competitor = competitors.get(pool_index).ok_or(
            NativeHeadsRangeLookupError::CompetitorPoolIndex {
                system_id: input.system_id,
                staff_id: input.staff_id,
                geometry_ordinal: input.geometry_ordinal,
                pool_index,
            },
        )?;
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

fn mark_black_relevants(relevants: &mut [bool], scan_left: i32, expanded: JavaRectangle) {
    let stop = expanded.x.wrapping_add(expanded.width);
    if expanded.x >= stop {
        return;
    }
    let start = expanded.x.max(scan_left);
    let scan_stop = scan_left.wrapping_add(relevants.len() as i32);
    let stop = stop.min(scan_stop);
    if start >= stop {
        return;
    }
    for x in start..stop {
        let index = usize::try_from(x.wrapping_sub(scan_left)).expect("clipped black relevance");
        relevants[index] = true;
    }
}

/// OpenJDK `java.awt.Rectangle.grow`, including clipping and negative dimensions.
#[must_use]
pub fn java_rectangle_grow(
    mut rectangle: JavaRectangle,
    horizontal: i32,
    vertical: i32,
) -> JavaRectangle {
    let (x, width) = grow_axis(rectangle.x, rectangle.width, horizontal);
    let (y, height) = grow_axis(rectangle.y, rectangle.height, vertical);
    rectangle.x = x;
    rectangle.y = y;
    rectangle.width = width;
    rectangle.height = height;
    rectangle
}

fn grow_axis(location: i32, dimension: i32, amount: i32) -> (i32, i32) {
    let mut start = i64::from(location) - i64::from(amount);
    let mut end = i64::from(dimension) + i64::from(location) + i64::from(amount);
    if end < start {
        end -= start;
        end = end.max(i64::from(i32::MIN));
        start = start.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    } else {
        start = start.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
        end -= start;
        end = end.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    }
    (start as i32, end as i32)
}

fn component_rectangle(component: &GlyphComponent) -> JavaRectangle {
    JavaRectangle::new(
        component.left,
        component.top,
        i32::try_from(component.width).expect("validated component width"),
        i32::try_from(component.height).expect("validated component height"),
    )
}

fn glyph_component_run_digest(component: &GlyphComponent) -> u64 {
    let (orientation, sequence_min, coordinate_min, sequence_count) = match component.orientation {
        Orientation::Horizontal => (
            "HORIZONTAL",
            component
                .runs
                .iter()
                .map(|entry| entry.sequence)
                .min()
                .expect("glyph component has runs"),
            component
                .runs
                .iter()
                .map(|entry| entry.run.start)
                .min()
                .expect("glyph component has runs"),
            component.height,
        ),
        Orientation::Vertical => (
            "VERTICAL",
            component
                .runs
                .iter()
                .map(|entry| entry.sequence)
                .min()
                .expect("glyph component has runs"),
            component
                .runs
                .iter()
                .map(|entry| entry.run.start)
                .min()
                .expect("glyph component has runs"),
            component.width,
        ),
    };
    let mut hash = FNV_OFFSET;
    hash_record(
        &mut hash,
        &format!("{orientation} {} {}", component.width, component.height),
    );
    for local_sequence in 0..sequence_count {
        let source_sequence = sequence_min + local_sequence;
        let mut row = local_sequence.to_string();
        use std::fmt::Write as _;
        for entry in component
            .runs
            .iter()
            .filter(|entry| entry.sequence == source_sequence)
        {
            write!(
                row,
                " {}:{}",
                entry.run.start - coordinate_min,
                entry.run.length
            )
            .expect("write to String");
        }
        hash_record(&mut hash, &row);
    }
    hash
}

fn java_rectangle(bounds: HeadTemplateBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

/// Canonical identity-free oracle record for one retained scanner spot.
#[must_use]
pub fn native_head_range_spot_kernel_record(
    system_id: usize,
    staff_id: usize,
    source: &NativeHeadScannerSource,
    spot: &NativeHeadRangeSpotEvidence,
) -> String {
    let effective = spot
        .effective_x
        .map_or_else(|| "-".to_owned(), |(left, right)| format!("{left}:{right}"));
    format!(
        "headrangespotkernel system {system_id} staff {staff_id} scanner {} source {} spot {} pool {} bounds {} weight {} runs {:016x} expanded {} effective {effective}",
        spot.geometry_ordinal,
        source_string(source),
        spot.spot_ordinal,
        spot.pool_index,
        rectangle_string(spot.bounds),
        spot.weight,
        spot.run_digest,
        rectangle_string(spot.expanded_bounds),
    )
}

#[allow(clippy::too_many_arguments)]
fn range_scan_kernel_record(
    input: &ScanInput<'_>,
    x: i32,
    y: i32,
    probe_x: i32,
    probe_value: Option<i32>,
    safety: RangeSafety,
    next_x: i32,
    black_relevant: bool,
    shape_choice: Option<NativeHeadRangeShapeChoice>,
    shapes: &[HeadTemplateShape],
) -> String {
    let probe_value = probe_value.map_or_else(|| "-".to_owned(), |value| value.to_string());
    let shape_choice = shape_choice.map_or("-", NativeHeadRangeShapeChoice::java_name);
    format!(
        "headrangescankernel system {} staff {} scanner {} source {} x {x} y {y} probeX {probe_x} probeValue {probe_value} safety {} nextX {next_x} blackRelevant {black_relevant} shapeChoice {shape_choice} shapes {}",
        input.system_id,
        input.staff_id,
        input.geometry_ordinal,
        source_string(&input.geometry.source),
        safety.java_name(),
        shape_names(shapes),
    )
}

#[allow(clippy::too_many_arguments)]
fn range_attempt_kernel_record(
    input: &ScanInput<'_>,
    x: i32,
    y: i32,
    black_relevant: bool,
    shape_choice: NativeHeadRangeShapeChoice,
    shape_ordinal: usize,
    original_shape: HeadTemplateShape,
    attempt: &ShapeSearch,
) -> String {
    let selected = attempt.best.map_or_else(
        || "-".to_owned(),
        |best| {
            format!(
                "{}:{}:{}:{}:{}",
                best.y_offset_ordinal,
                best.y_offset,
                best.x,
                best.y,
                java_hex_double(best.distance)
            )
        },
    );
    let black_to_void = attempt
        .black_to_void
        .map_or_else(|| "-".to_owned(), |value| value.to_string());
    let raw_candidate = attempt
        .candidate
        .map_or_else(|| "-".to_owned(), |candidate| candidate.ordinal.to_string());
    format!(
        "headrangeattemptkernel system {} staff {} scanner {} source {} x {x} y {y} blackRelevant {black_relevant} shapeChoice {} shapeOrdinal {shape_ordinal} original {} finalShape {} evaluations {} calls {}:{}:{} bestUpdates {} selected {selected} blackToVoid {black_to_void} weakStemless {} rawCandidate {raw_candidate} outcome {}",
        input.system_id,
        input.staff_id,
        input.geometry_ordinal,
        source_string(&input.geometry.source),
        shape_choice.java_name(),
        shape_name(original_shape),
        shape_name(attempt.final_shape),
        if attempt.evaluations.is_empty() {
            "-"
        } else {
            &attempt.evaluations
        },
        attempt.performance.bar_blocks,
        attempt.performance.competitor_blocks,
        attempt.performance.evaluations,
        attempt.best_updates,
        attempt.weak_stemless,
        attempt.outcome.java_name(),
    )
}

/// Canonical identity-free oracle record for one raw pre-aggregation candidate.
#[must_use]
pub fn native_head_range_raw_kernel_record(
    system_id: usize,
    staff_id: usize,
    source: &NativeHeadScannerSource,
    candidate: &NativeHeadRangeCandidate,
) -> String {
    let black_to_void = candidate
        .hole
        .map_or_else(|| "-".to_owned(), |hole| hole.converted_to_void.to_string());
    format!(
        "headrangerawkernel system {system_id} staff {staff_id} scanner {} source {} x {} y {} shapeOrdinal {} original {} finalShape {} selected {}:{}:{}:{}:{} blackToVoid {black_to_void} slim {} pitch {} grade {} impacts dist:{}:{} outcome provisional",
        candidate.geometry_ordinal,
        source_string(source),
        candidate.x0,
        candidate.y0,
        candidate.shape_ordinal,
        shape_name(candidate.original_shape),
        shape_name(candidate.shape),
        candidate.best.y_offset_ordinal,
        candidate.best.y_offset,
        candidate.best.x,
        candidate.best.y,
        java_hex_double(candidate.best.distance),
        rectangle_string(java_rectangle(candidate.bounds)),
        java_hex_double(candidate.pitch),
        java_hex_double(candidate.grade),
        java_hex_double(candidate.distance_impact),
        java_hex_double(1.0),
    )
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

fn shape_names(shapes: &[HeadTemplateShape]) -> String {
    if shapes.is_empty() {
        return "-".to_owned();
    }
    shapes
        .iter()
        .map(|&shape| shape_name(shape))
        .collect::<Vec<_>>()
        .join(",")
}

const fn shape_name(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
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

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

fn hash_record(hash: &mut u64, record: &str) {
    for byte in record.bytes().chain(std::iter::once(b'\n')) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

#[derive(Default)]
struct KernelHashers {
    hashes: NativeHeadRangeKernelHashes,
}

impl KernelHashers {
    fn add_spot(&mut self, record: &str) {
        hash_record(&mut self.hashes.spot, record);
    }

    fn add_scan(&mut self, record: &str) {
        hash_record(&mut self.hashes.scan, record);
    }

    fn add_attempt(&mut self, record: &str) {
        hash_record(&mut self.hashes.attempt, record);
    }

    fn add_raw_candidate(&mut self, record: &str) {
        hash_record(&mut self.hashes.raw_candidate, record);
    }

    const fn finish(self) -> NativeHeadRangeKernelHashes {
        self.hashes
    }
}

fn validate_distance_table(
    distances: &NeutralDistanceTable,
) -> Result<(), NativeHeadsRangeLookupError> {
    let expected = distances.width.checked_mul(distances.height);
    if distances.width == 0
        || distances.height == 0
        || distances.normalizer != DEFAULT_CHAMFER_NORMALIZER
        || expected != Some(distances.values.len())
    {
        return Err(NativeHeadsRangeLookupError::InvalidDistanceTable {
            width: distances.width,
            height: distances.height,
            normalizer: distances.normalizer,
            values: distances.values.len(),
        });
    }
    i32::try_from(distances.width)
        .and_then(|_| i32::try_from(distances.height))
        .map_err(|_| NativeHeadsRangeLookupError::DistanceDimensionsTooLarge)?;
    Ok(())
}

fn validate_system_topology(
    input: &NativeHeadsRangeLookupInput<'_>,
) -> Result<(), NativeHeadsRangeLookupError> {
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
            return Err(NativeHeadsRangeLookupError::SystemOrder {
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
) -> Result<(), NativeHeadsRangeLookupError> {
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
            return Err(NativeHeadsRangeLookupError::StaffOrder {
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
) -> Result<(), NativeHeadsRangeLookupError> {
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
            return Err(NativeHeadsRangeLookupError::ScanOrder {
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

    #[test]
    fn rectangle_grow_matches_openjdk_normal_and_clipped_cases() {
        assert_eq!(
            java_rectangle_grow(JavaRectangle::new(10, 20, 5, 7), 3, 0),
            JavaRectangle::new(7, 20, 11, 7)
        );
        assert_eq!(
            java_rectangle_grow(JavaRectangle::new(i32::MIN, 2, 1, 3), 1, 0),
            JavaRectangle::new(i32::MIN, 2, 2, 3)
        );
        assert_eq!(
            java_rectangle_grow(JavaRectangle::new(i32::MAX, 2, 1, 3), 1, 0),
            JavaRectangle::new(i32::MAX - 1, 2, 3, 3)
        );
        assert_eq!(
            java_rectangle_grow(JavaRectangle::new(4, 5, -3, -7), 1, 2),
            JavaRectangle::new(3, 3, -1, -3)
        );
    }

    #[test]
    fn black_relevance_uses_half_open_grown_interval_and_clips_to_scan() {
        let mut relevants = [false; 7];
        mark_black_relevants(&mut relevants, 10, JavaRectangle::new(8, 0, 6, 1));
        assert_eq!(relevants, [true, true, true, true, false, false, false]);

        mark_black_relevants(&mut relevants, 10, JavaRectangle::new(15, 0, 5, 1));
        assert_eq!(relevants, [true, true, true, true, false, true, true]);
    }

    #[test]
    fn stemless_gate_applies_zero_boost_and_strict_contextual_threshold() {
        let weak = stemless_evidence(HeadTemplateShape::WholeNote, 0.35);
        assert!(weak.applicable);
        assert_eq!(weak.contextual_grade, weak.intrinsic_grade);
        assert!(weak.rejected);

        let retained = stemless_evidence(HeadTemplateShape::Breve, 0.18_75);
        assert_eq!(retained.contextual_grade, 0.5);
        assert!(!retained.rejected);

        let stemmed = stemless_evidence(HeadTemplateShape::NoteheadVoid, 0.4);
        assert!(!stemmed.applicable);
        assert!(!stemmed.rejected);
    }

    #[test]
    fn range_hashes_start_at_fnv_offset_and_chain_trailing_newlines() {
        let mut hashers = KernelHashers::default();
        hashers.add_scan("one");
        hashers.add_scan("two");
        let hashes = hashers.finish();
        let mut expected = FNV_OFFSET;
        hash_record(&mut expected, "one");
        hash_record(&mut expected, "two");
        assert_eq!(hashes.scan, expected);
        assert_eq!(hashes.spot, FNV_OFFSET);
        assert_eq!(hashes.attempt, FNV_OFFSET);
        assert_eq!(hashes.raw_candidate, FNV_OFFSET);
    }

    #[test]
    fn java_hex_format_preserves_java_text_and_canonical_nan_bits() {
        assert_eq!(java_hex_double(1.0), "0x1.0p0/3ff0000000000000".to_owned());
        assert_eq!(
            java_hex_double(f64::from_bits(0x7ff0_0000_0000_0001)),
            "NaN/7ff8000000000000".to_owned()
        );
    }
}
