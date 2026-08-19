// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete dependency-light prefix of Java `LedgersFilter` and
//! `LedgersBuilder.lookupLine`.
//!
//! The raw no-staff raster, zero-shift sections, staff/system dispatch, beam
//! purge, deterministic candidate gates, and intrinsic ledger grade are native.
//! `HorizontalLedgerGlyphFactory` is the first injected geometric collaborator:
//! Java's horizontal `StickFactory`/`StraightFilament` construction.

use std::{collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    beam_structure::BeamItem,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, JunctionPolicy, Section, build_sections_from_id},
    stick_factory::{HorizontalStickFactory, HorizontalStickParameters, StraightStickError},
    system_population::{PopulationSystemArea, StaffBoundary},
};

use crate::ledgers_step::{
    LedgerDelta, LedgerDeltaMutation, NeutralLedgerGlyph, NeutralLedgerInter, NeutralLedgerRelation,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgerLineSegment {
    pub start_x: f64,
    pub start_y: f64,
    pub stop_x: f64,
    pub stop_y: f64,
}

impl LedgerLineSegment {
    #[must_use]
    pub fn y_at(self, x: f64) -> f64 {
        let dx = self.stop_x - self.start_x;
        if dx == 0.0 {
            return self.start_y;
        }
        self.start_y + ((x - self.start_x) * (self.stop_y - self.start_y) / dx)
    }

    #[must_use]
    pub fn bounds(self) -> LedgerFloatBounds {
        LedgerFloatBounds {
            x: self.start_x.min(self.stop_x),
            y: self.start_y.min(self.stop_y),
            width: (self.stop_x - self.start_x).abs(),
            height: (self.stop_y - self.start_y).abs().max(1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgerFloatBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LedgerFloatBounds {
    #[must_use]
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    #[must_use]
    pub fn x_overlap(self, other: Self) -> f64 {
        ((self.x + self.width).min(other.x + other.width) - self.x.max(other.x)).max(0.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerStaffZone {
    pub id: usize,
    pub system_id: usize,
    pub specific_interline: i32,
    pub tablature: bool,
    pub merged_part: bool,
    pub first_in_part: bool,
    pub last_in_part: bool,
    /// Java `Staff.area`, used by `StaffManager.getClosestStaff`.
    pub area: PopulationSystemArea,
    pub first_line: StaffBoundary,
    pub last_line: StaffBoundary,
}

impl RawLedgerStaffZone {
    #[must_use]
    pub fn distance_to(&self, x: f64, y: f64) -> f64 {
        (self.first_line.y_at_x_ext(x) - y).max(y - self.last_line.y_at_x_ext(x))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerBeamArea {
    pub bounds: Bounds,
    pub item: BeamItem,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerSystemZone {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub area: PopulationSystemArea,
    /// Every `AbstractBeamInter`, used by `LedgersFilter`'s section purge.
    pub all_beams: Vec<RawLedgerBeamArea>,
    /// Good full `BeamInter`s only, used by `LedgersBuilder`'s candidate purge.
    pub good_full_beams: Vec<RawLedgerBeamArea>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerSystemSections {
    pub system_id: usize,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerFilterOutput {
    pub run_table: RunTable,
    pub sections: Vec<Section>,
    pub by_system: Vec<RawLedgerSystemSections>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawLedgerFilterParameters {
    pub minimum_staff_distance: f64,
}

impl Default for RawLedgerFilterParameters {
    fn default() -> Self {
        Self {
            minimum_staff_distance: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawLedgerFilterError {
    InvalidSource(RunTableError),
    InvalidInterline(usize),
}

impl fmt::Display for RawLedgerFilterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource(source) => write!(formatter, "invalid no-staff raster: {source}"),
            Self::InvalidInterline(id) => write!(formatter, "staff {id} has invalid interline"),
        }
    }
}

impl Error for RawLedgerFilterError {}

/// Java `LedgersFilter.process`, excluding debug/UI side effects.
pub fn filter_raw_ledger_sections(
    no_staff: &RunTable,
    staves: &[RawLedgerStaffZone],
    systems: &[RawLedgerSystemZone],
    parameters: RawLedgerFilterParameters,
) -> Result<RawLedgerFilterOutput, RawLedgerFilterError> {
    let source = RunTable::from_pixels(
        Orientation::Horizontal,
        no_staff.width(),
        no_staff.height(),
        &no_staff.to_pixels(),
    )
    .map_err(RawLedgerFilterError::InvalidSource)?;
    let mut filtered = RunTable::new(Orientation::Horizontal, source.width(), source.height())
        .map_err(RawLedgerFilterError::InvalidSource)?;

    for y in 0..source.sequence_count() {
        for run in source.sequence(y).unwrap_or_default() {
            let center_x = run.start + (run.length / 2);
            let Some(staff) = closest_staff(staves, center_x as f64, y as f64) else {
                continue;
            };
            if staff.specific_interline <= 0 {
                return Err(RawLedgerFilterError::InvalidInterline(staff.id));
            }
            let minimum_distance =
                java_rint(f64::from(staff.specific_interline) * parameters.minimum_staff_distance);
            if staff.distance_to(center_x as f64, y as f64) >= f64::from(minimum_distance) {
                filtered
                    .add_run(y, *run)
                    .map_err(RawLedgerFilterError::InvalidSource)?;
            }
        }
    }

    // Java `JunctionShiftPolicy(0)`: only identical run endpoints continue.
    let sections = build_sections_from_id(&filtered, JunctionPolicy::Shift { max_shift: 0 }, 1);
    let mut by_system = systems
        .iter()
        .map(|system| RawLedgerSystemSections {
            system_id: system.id,
            sections: Vec::new(),
        })
        .collect::<Vec<_>>();
    for section in &sections {
        let (center_x, center_y) = section.centroid();
        for (system_index, system) in systems.iter().enumerate() {
            if system.area.contains(center_x as f64, center_y as f64)
                && i32::try_from(center_x).is_ok_and(|x| x >= system.left && x <= system.right)
                && !system
                    .all_beams
                    .iter()
                    .any(|beam| beam_intersects_section(beam, section))
            {
                by_system[system_index].sections.push(section.clone());
            }
        }
    }

    Ok(RawLedgerFilterOutput {
        run_table: filtered,
        sections,
        by_system,
    })
}

fn closest_staff(staves: &[RawLedgerStaffZone], x: f64, y: f64) -> Option<&RawLedgerStaffZone> {
    let mut best = None;
    let mut best_distance = f64::MAX;
    for staff in staves {
        if !staff.area.contains(x, y) {
            continue;
        }
        let distance = staff.distance_to(x, y);
        if distance < best_distance {
            best_distance = distance;
            best = Some(staff);
        }
    }
    best
}

fn beam_intersects_section(beam: &RawLedgerBeamArea, section: &Section) -> bool {
    let section_bounds = section.bounds();
    if !bounds_intersect(beam.bounds, section_bounds) {
        return false;
    }
    let x = section_bounds.x as f64;
    let y = section_bounds.y as f64;
    let rectangle = [
        (x, y),
        (x + section_bounds.width as f64, y),
        (
            x + section_bounds.width as f64,
            y + section_bounds.height as f64,
        ),
        (x, y + section_bounds.height as f64),
    ];
    // Java deliberately tests the beam area against `Section.getBounds()`,
    // rather than against the individual foreground runs in the section.
    convex_areas_intersect(beam_corners(beam.item), rectangle)
}

fn beam_contains(item: BeamItem, x: f64, y: f64) -> bool {
    let corners = beam_corners(item);
    let mut crossings = 0;
    for index in 0..corners.len() {
        let (ax, ay) = corners[index];
        let (bx, by) = corners[(index + 1) % corners.len()];
        if (ay <= y) == (by <= y) {
            continue;
        }
        if ax + ((y - ay) * (bx - ax) / (by - ay)) > x {
            crossings += 1;
        }
    }
    crossings % 2 == 1
}

fn beam_corners(item: BeamItem) -> [(f64, f64); 4] {
    let half_height = item.height / 2.0;
    [
        (item.median.x1, item.median.y1 - half_height),
        (item.median.x2, item.median.y2 - half_height),
        (item.median.x2, item.median.y2 + half_height),
        (item.median.x1, item.median.y1 + half_height),
    ]
}

/// Positive-area intersection of two convex Java2D `Area` polygons.
fn convex_areas_intersect(one: [(f64, f64); 4], two: [(f64, f64); 4]) -> bool {
    for polygon in [&one, &two] {
        for index in 0..polygon.len() {
            let start = polygon[index];
            let stop = polygon[(index + 1) % polygon.len()];
            let axis = (-(stop.1 - start.1), stop.0 - start.0);
            let project = |points: &[(f64, f64); 4]| {
                points
                    .iter()
                    .map(|point| (point.0 * axis.0) + (point.1 * axis.1))
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                        (min.min(value), max.max(value))
                    })
            };
            let (one_min, one_max) = project(&one);
            let (two_min, two_max) = project(&two);
            if one_max <= two_min || two_max <= one_min {
                return false;
            }
        }
    }
    true
}

fn bounds_intersect(one: Bounds, two: Bounds) -> bool {
    one.x < two.x.saturating_add(two.width)
        && two.x < one.x.saturating_add(one.width)
        && one.y < two.y.saturating_add(two.height)
        && two.y < one.y.saturating_add(one.height)
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerCandidate {
    /// Sheet-global filament registration identity.
    pub id: usize,
    /// Populated only by the later glyph/SIG materializer.
    pub glyph_id: Option<usize>,
    /// Populated only by the later glyph/SIG materializer.
    pub inter_id: Option<usize>,
    pub section_ids: Vec<usize>,
    pub bounds: LedgerFloatBounds,
    pub start: (f64, f64),
    pub stop: (f64, f64),
    pub mean_thickness: f64,
    pub mean_distance: f64,
    pub convex_end_count: i32,
    pub overlaps_good_beam: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgerGlyphFactoryInput<'a> {
    pub system_id: usize,
    pub sections: &'a [Section],
    pub maximum_thickness: i32,
    pub minimum_core_section_length: i32,
    pub minimum_side_ratio: f64,
}

/// First unavailable collaborator: Java horizontal `StickFactory` plus
/// `StraightFilament` geometry materialization.
pub trait HorizontalLedgerGlyphFactory {
    type Error;

    fn build_horizontal_candidates(
        &mut self,
        input: LedgerGlyphFactoryInput<'_>,
    ) -> Result<Vec<RawLedgerCandidate>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawLedgerScale {
    pub large_interline: i32,
    pub mean_line_thickness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawLedgerCandidateParameters {
    pub maximum_thickness_line_fraction: f64,
    pub maximum_thickness_interline_fraction: f64,
    pub minimum_core_section_length: f64,
    pub minimum_side_ratio: f64,
    pub maximum_ledger_length: f64,
    pub ledger_margin_y: f64,
    pub minimum_abscissa_overlap: f64,
    pub minimum_wide_ledger_length: f64,
    pub minimum_length_low: f64,
    pub minimum_length_high: f64,
    pub minimum_length_low_after_wide: f64,
    pub minimum_length_high_after_wide: f64,
    pub minimum_thickness_high: f64,
    pub maximum_thickness_low: f64,
    pub straightness_high: f64,
    pub intrinsic_ratio: f64,
    pub minimum_grade: f64,
}

impl Default for RawLedgerCandidateParameters {
    fn default() -> Self {
        Self {
            maximum_thickness_line_fraction: 3.25,
            maximum_thickness_interline_fraction: 0.4,
            minimum_core_section_length: 1.0,
            minimum_side_ratio: 0.9,
            maximum_ledger_length: 20.0,
            ledger_margin_y: 0.35,
            minimum_abscissa_overlap: 0.75,
            minimum_wide_ledger_length: 1.5,
            minimum_length_low: 1.0,
            minimum_length_high: 1.5,
            minimum_length_low_after_wide: 1.4,
            minimum_length_high_after_wide: 2.0,
            minimum_thickness_high: 0.25,
            maximum_thickness_low: 1.0,
            straightness_high: 0.3,
            intrinsic_ratio: 0.8,
            minimum_grade: 0.08,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgerCandidateImpact {
    pub value: f64,
    pub grade: f64,
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LedgerCandidateGrade {
    pub candidate_id: usize,
    pub staff_id: usize,
    pub index: i32,
    pub y_target: f64,
    /// Min thickness, max thickness, length, convexity, straightness,
    /// left pitch, right pitch—in Java suite order.
    pub impacts: [LedgerCandidateImpact; 7],
    pub grade: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LedgerLineCandidate {
    pub candidate: RawLedgerCandidate,
    pub grade: LedgerCandidateGrade,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LedgerPreviousReference {
    pub candidate_id: usize,
    pub bounds: LedgerFloatBounds,
    pub start: (f64, f64),
    pub stop: (f64, f64),
}

#[derive(Debug, PartialEq)]
pub enum RawLedgerCandidateError<FactoryError> {
    InvalidScale,
    Factory(FactoryError),
}

#[derive(Clone, Debug)]
pub struct NativeLedgerCandidateOutcome {
    pub candidates: Vec<RawLedgerCandidate>,
    /// All StickFactory registrations, including sticks swallowed before a
    /// later partial failure.
    pub registered_filament_ids: Vec<u64>,
    pub next_filament_id: u64,
    pub error: Option<StraightStickError>,
}

/// Concrete horizontal StickFactory adapter. The returned candidates retain
/// filament ownership but deliberately have no glyph or inter identity; that
/// materializer is the next production seam.
#[must_use]
pub fn source_native_ledger_candidates(
    no_staff: &RunTable,
    system: &RawLedgerSystemZone,
    sections: &[Section],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
    first_filament_id: u64,
) -> NativeLedgerCandidateOutcome {
    if scale.large_interline <= 0 || scale.mean_line_thickness <= 0.0 {
        return NativeLedgerCandidateOutcome {
            candidates: Vec::new(),
            registered_filament_ids: Vec::new(),
            next_filament_id: first_filament_id,
            error: Some(StraightStickError::InvalidParameters),
        };
    }
    let maximum_thickness = java_rint(
        (scale.mean_line_thickness * parameters.maximum_thickness_line_fraction).min(
            f64::from(scale.large_interline) * parameters.maximum_thickness_interline_fraction,
        ),
    );
    let minimum_core_section_length =
        java_rint(f64::from(scale.large_interline) * parameters.minimum_core_section_length);
    let factory = HorizontalStickFactory::new(HorizontalStickParameters {
        interline: usize::try_from(scale.large_interline).unwrap_or(0),
        maximum_stick_thickness: usize::try_from(maximum_thickness).unwrap_or(0),
        minimum_core_section_length: usize::try_from(minimum_core_section_length).unwrap_or(0),
        minimum_side_ratio: parameters.minimum_side_ratio,
    });
    let outcome = factory.retrieve_sticks(sections, first_filament_id);
    let maximum_length = f64::from(java_rint(
        f64::from(scale.large_interline) * parameters.maximum_ledger_length,
    ));
    let mut candidates = Vec::new();
    let mut conversion_error = None;
    for stick in outcome.result.survivors() {
        let geometry = match stick.straight_geometry() {
            Ok(geometry) => geometry,
            Err(error) => {
                conversion_error = Some(error);
                break;
            }
        };
        let center = (
            (geometry.start.0 + geometry.stop.0) / 2.0,
            (geometry.start.1 + geometry.stop.1) / 2.0,
        );
        let overlaps_good_beam = system.good_full_beams.iter().any(|beam| {
            beam_contains(beam.item, center.0, center.1)
                && point_in_bounds(center.0, center.1, beam.bounds)
        });
        let candidate = RawLedgerCandidate {
            id: usize::try_from(stick.id()).unwrap_or(usize::MAX),
            glyph_id: None,
            inter_id: None,
            section_ids: stick
                .filament()
                .sections()
                .iter()
                .map(Section::id)
                .collect(),
            bounds: LedgerFloatBounds {
                x: geometry.bounds.x as f64,
                y: geometry.bounds.y as f64,
                width: geometry.bounds.width as f64,
                height: geometry.bounds.height as f64,
            },
            start: geometry.start,
            stop: geometry.stop,
            mean_thickness: geometry.mean_thickness,
            mean_distance: geometry.mean_distance,
            convex_end_count: convex_end_count(no_staff, geometry.bounds),
            overlaps_good_beam,
        };
        // Java performs beam purge first, then too-long purge, both without
        // disturbing the order returned by StickFactory.
        if !candidate.overlaps_good_beam && candidate.bounds.width <= maximum_length {
            candidates.push(candidate);
        }
    }
    NativeLedgerCandidateOutcome {
        candidates,
        registered_filament_ids: outcome.result.creation_ids().to_vec(),
        next_filament_id: outcome.result.next_creation_id(),
        error: outcome.error.or(conversion_error),
    }
}

fn point_in_bounds(x: f64, y: f64, bounds: Bounds) -> bool {
    x >= bounds.x as f64
        && x < bounds.x.saturating_add(bounds.width) as f64
        && y >= bounds.y as f64
        && y < bounds.y.saturating_add(bounds.height) as f64
}

fn convex_end_count(no_staff: &RunTable, bounds: Bounds) -> i32 {
    let pixels = no_staff.to_pixels();
    let width = no_staff.width();
    let height = no_staff.height();
    let mut convexities = 0;
    for x in [bounds.x, bounds.x + bounds.width - 1] {
        let top_foreground = bounds.y > 0 && pixels[(bounds.y - 1) * width + x] == 0;
        let below = bounds.y.saturating_add(bounds.height);
        let bottom_foreground = below < height && pixels[below * width + x] == 0;
        if !(top_foreground || bottom_foreground) {
            convexities += 1;
        }
    }
    convexities
}

impl<FactoryError: fmt::Display> fmt::Display for RawLedgerCandidateError<FactoryError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("invalid ledger scale"),
            Self::Factory(source) => write!(formatter, "ledger glyph factory failed: {source}"),
        }
    }
}

impl<FactoryError: Error + 'static> Error for RawLedgerCandidateError<FactoryError> {}

/// Invoke the first visual seam with Java's resolved factory parameters, then
/// apply the two source-order system-wide purges (`beamOverlap`, `tooLong`).
pub fn source_ledger_candidates<Factory: HorizontalLedgerGlyphFactory>(
    factory: &mut Factory,
    system_id: usize,
    sections: &[Section],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> Result<Vec<RawLedgerCandidate>, RawLedgerCandidateError<Factory::Error>> {
    if scale.large_interline <= 0 || scale.mean_line_thickness <= 0.0 {
        return Err(RawLedgerCandidateError::InvalidScale);
    }
    let maximum_thickness = java_rint(
        (scale.mean_line_thickness * parameters.maximum_thickness_line_fraction).min(
            f64::from(scale.large_interline) * parameters.maximum_thickness_interline_fraction,
        ),
    );
    let minimum_core_section_length =
        java_rint(f64::from(scale.large_interline) * parameters.minimum_core_section_length);
    let maximum_length =
        java_rint(f64::from(scale.large_interline) * parameters.maximum_ledger_length);
    let candidates = factory
        .build_horizontal_candidates(LedgerGlyphFactoryInput {
            system_id,
            sections,
            maximum_thickness,
            minimum_core_section_length,
            minimum_side_ratio: parameters.minimum_side_ratio,
        })
        .map_err(RawLedgerCandidateError::Factory)?;
    Ok(candidates
        .into_iter()
        .filter(|candidate| !candidate.overlaps_good_beam)
        .filter(|candidate| candidate.bounds.width <= f64::from(maximum_length))
        .collect())
}

/// Java `lookupLine` through grading and overlap reduction for one staff line.
/// Candidate visitation remains factory order; accepted output is the Java
/// abscissa sort used immediately before exclusion reduction.
#[must_use]
pub fn evaluate_ledger_line(
    staff: &RawLedgerStaffZone,
    index: i32,
    candidates: &[RawLedgerCandidate],
    previous: &[LedgerPreviousReference],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> Vec<LedgerLineCandidate> {
    let mut accepted =
        evaluate_ledger_line_unreduced(staff, index, candidates, previous, scale, parameters);
    accepted.sort_by(|one, two| {
        one.candidate
            .bounds
            .x
            .total_cmp(&two.candidate.bounds.x)
            .then_with(|| one.candidate.id.cmp(&two.candidate.id))
    });
    reduce_overlaps(accepted)
}

/// Java candidate visitation through intrinsic grade, before glyph/SIG
/// materialization and the later abscissa sort/exclusion reduction.
#[must_use]
pub fn evaluate_ledger_line_unreduced(
    staff: &RawLedgerStaffZone,
    index: i32,
    candidates: &[RawLedgerCandidate],
    previous: &[LedgerPreviousReference],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> Vec<LedgerLineCandidate> {
    evaluate_ledger_line_audited(staff, index, candidates, previous, scale, parameters).accepted
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerLineRejection {
    /// `|index| >= 2` and no previous-rung survivor overlapped the candidate
    /// by more than the minimum abscissa overlap, so it was never graded.
    NoPreviousOverlap,
    BelowMinimumGrade,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RejectedLedgerLineCandidate {
    pub candidate: RawLedgerCandidate,
    pub reason: LedgerLineRejection,
    /// Present only for [`LedgerLineRejection::BelowMinimumGrade`].
    pub grade: Option<LedgerCandidateGrade>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuditedLedgerLine {
    pub accepted: Vec<LedgerLineCandidate>,
    /// In-band candidates that were visited and rejected. Candidates whose
    /// midpoint falls outside the line's virtual band are not recorded; they
    /// belong to other lines or to no line at all.
    pub rejected: Vec<RejectedLedgerLineCandidate>,
}

/// [`evaluate_ledger_line_unreduced`] with every in-band candidate's fate.
#[must_use]
pub fn evaluate_ledger_line_audited(
    staff: &RawLedgerStaffZone,
    index: i32,
    candidates: &[RawLedgerCandidate],
    previous: &[LedgerPreviousReference],
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> AuditedLedgerLine {
    if staff.tablature || index == 0 || staff.specific_interline <= 0 {
        return AuditedLedgerLine::default();
    }
    if staff.merged_part
        && ((staff.first_in_part && index > 1) || (staff.last_in_part && index < 0))
    {
        return AuditedLedgerLine::default();
    }
    let interline = f64::from(staff.specific_interline);
    let margin = f64::from(java_rint(interline * parameters.ledger_margin_y));
    let reference_line = if index < 0 {
        &staff.first_line
    } else {
        &staff.last_line
    };
    let (x, y, width, height) = reference_line.defining_point_bounds();
    let mut virtual_bounds = LedgerFloatBounds {
        x: f64::from(x),
        y: f64::from(y),
        width: f64::from(width),
        height: f64::from(height),
    };
    virtual_bounds.y += f64::from(index) * interline;
    // Rectangle.grow(0, 2*yMargin) grows each vertical side by 2*yMargin.
    virtual_bounds.y -= 2.0 * margin;
    virtual_bounds.height += 4.0 * margin;
    let minimum_overlap = f64::from(scale.large_interline) * parameters.minimum_abscissa_overlap;
    let minimum_wide = f64::from(java_rint(interline * parameters.minimum_wide_ledger_length));
    let mut audit = AuditedLedgerLine::default();

    for candidate in candidates {
        let middle = (
            (candidate.start.0 + candidate.stop.0) / 2.0,
            (candidate.start.1 + candidate.stop.1) / 2.0,
        );
        if !virtual_bounds.contains(middle.0, middle.1) {
            continue;
        }
        // Java uses endpoint midpoint only for the rough containment above.
        // `getYReference` then asks `SectionCompound.getCenter2D()`, which is
        // the contour-bounds centre and is half a pixel farther right for an
        // even-width inclusive stick.
        let reference_x = candidate.bounds.x + (candidate.bounds.width / 2.0);
        let (y_reference, wide_reference) = if index.abs() == 1 {
            (reference_line.y_at_x_ext(reference_x), false)
        } else {
            let Some(reference) = previous
                .iter()
                .find(|reference| reference.bounds.x_overlap(candidate.bounds) > minimum_overlap)
            else {
                audit.rejected.push(RejectedLedgerLineCandidate {
                    candidate: candidate.clone(),
                    reason: LedgerLineRejection::NoPreviousOverlap,
                    grade: None,
                });
                continue;
            };
            let y = y_on_reference(reference, reference_x);
            (y, reference.bounds.width >= minimum_wide)
        };
        let y_target = y_reference + f64::from(index.signum()) * interline;
        let grade = grade_candidate(
            candidate,
            staff,
            index,
            y_target,
            wide_reference,
            scale,
            parameters,
        );
        if grade.grade >= parameters.minimum_grade {
            audit.accepted.push(LedgerLineCandidate {
                candidate: candidate.clone(),
                grade,
            });
        } else {
            audit.rejected.push(RejectedLedgerLineCandidate {
                candidate: candidate.clone(),
                reason: LedgerLineRejection::BelowMinimumGrade,
                grade: Some(grade),
            });
        }
    }

    audit
}

fn y_on_reference(reference: &LedgerPreviousReference, x: f64) -> f64 {
    let dx = reference.stop.0 - reference.start.0;
    if dx == 0.0 {
        reference.start.1
    } else {
        reference.start.1 + ((x - reference.start.0) * (reference.stop.1 - reference.start.1) / dx)
    }
}

fn grade_candidate(
    candidate: &RawLedgerCandidate,
    staff: &RawLedgerStaffZone,
    index: i32,
    y_target: f64,
    wide_reference: bool,
    scale: RawLedgerScale,
    parameters: RawLedgerCandidateParameters,
) -> LedgerCandidateGrade {
    let interline = f64::from(staff.specific_interline);
    let (length_low, length_high) = if wide_reference {
        (
            parameters.minimum_length_low_after_wide,
            parameters.minimum_length_high_after_wide,
        )
    } else {
        (
            parameters.minimum_length_low,
            parameters.minimum_length_high,
        )
    };
    let specs = [
        (
            candidate.mean_thickness / f64::from(scale.large_interline),
            0.0,
            parameters.minimum_thickness_high,
            true,
            0.5,
        ),
        (
            candidate.mean_thickness / scale.mean_line_thickness,
            parameters.maximum_thickness_low,
            parameters.maximum_thickness_line_fraction,
            false,
            0.0,
        ),
        (
            candidate.bounds.width / interline,
            length_low,
            length_high,
            true,
            4.0,
        ),
        (f64::from(candidate.convex_end_count), -0.5, 2.0, true, 2.0),
        (
            candidate.mean_distance / f64::from(scale.large_interline),
            0.0,
            parameters.straightness_high,
            false,
            1.0,
        ),
        (
            (candidate.start.1 - y_target).abs() / interline,
            0.0,
            parameters.ledger_margin_y,
            false,
            0.5,
        ),
        (
            (candidate.stop.1 - y_target).abs() / interline,
            0.0,
            parameters.ledger_margin_y,
            false,
            0.5,
        ),
    ];
    let impacts = specs.map(
        |(value, low, high, covariant, weight)| LedgerCandidateImpact {
            value,
            grade: check_grade(value, low, high, covariant),
            weight,
        },
    );
    let total_weight = impacts
        .iter()
        .filter(|impact| impact.weight >= 0.0)
        .map(|impact| impact.weight)
        .sum::<f64>();
    let mut product = 1.0;
    for impact in &impacts {
        if impact.weight < 0.0 {
            continue;
        }
        if impact.grade == 0.0 {
            product = 0.0;
        } else if impact.weight != 0.0 {
            product *= impact.grade.powf(impact.weight);
        }
    }
    LedgerCandidateGrade {
        candidate_id: candidate.id,
        staff_id: staff.id,
        index,
        y_target,
        impacts,
        grade: product.powf(1.0 / total_weight) * parameters.intrinsic_ratio,
    }
}

fn check_grade(value: f64, low: f64, high: f64, covariant: bool) -> f64 {
    if covariant {
        if value < low {
            0.0
        } else if value >= high {
            1.0
        } else {
            ((value - low) / (high - low)).clamp(0.0, 1.0)
        }
    } else if value > high {
        0.0
    } else if value <= low {
        1.0
    } else {
        ((high - value) / (high - low)).clamp(0.0, 1.0)
    }
}

fn reduce_overlaps(mut candidates: Vec<LedgerLineCandidate>) -> Vec<LedgerLineCandidate> {
    loop {
        let mut best: Option<(usize, usize, f64)> = None;
        for left in 0..candidates.len() {
            for right in left + 1..candidates.len() {
                if candidates[left]
                    .candidate
                    .bounds
                    .x_overlap(candidates[right].candidate.bounds)
                    <= 0.0
                {
                    break;
                }
                let strongest = candidates[left]
                    .grade
                    .grade
                    .max(candidates[right].grade.grade);
                if best.is_none_or(|(_, _, best_grade)| best_grade < strongest) {
                    best = Some((left, right, strongest));
                }
            }
        }
        let Some((left, right, _)) = best else {
            break;
        };
        // Java exclusion source has the lower inter ID and removes target on
        // an exact tie. Express that directly rather than relying on x order.
        let remove = if candidates[left].grade.grade < candidates[right].grade.grade {
            left
        } else if candidates[right].grade.grade < candidates[left].grade.grade
            || candidate_tie_id(&candidates[left].candidate)
                < candidate_tie_id(&candidates[right].candidate)
        {
            right
        } else {
            left
        };
        candidates.remove(remove);
    }
    candidates
}

fn candidate_tie_id(candidate: &RawLedgerCandidate) -> usize {
    candidate.inter_id.unwrap_or(candidate.id)
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterializedLedgerGlyph {
    pub id: usize,
    pub filament_id: usize,
    pub section_ids: Vec<usize>,
    pub bounds: LedgerFloatBounds,
}

/// Positioned fixed glyph produced by Java `StraightFilament.toGlyph(null)`.
///
/// The table is cropped to the exact union of the referenced section pixels;
/// `bounds` retains its absolute sheet position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedLedgerGlyphRaster {
    pub bounds: Bounds,
    pub run_table: RunTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerGlyphRasterError {
    EmptySections,
    MissingSection(usize),
    NonHorizontalSection(usize),
    RunTable(RunTableError),
}

impl fmt::Display for LedgerGlyphRasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySections => formatter.write_str("ledger glyph has no sections"),
            Self::MissingSection(id) => write!(formatter, "ledger glyph section {id} is missing"),
            Self::NonHorizontalSection(id) => {
                write!(formatter, "ledger glyph section {id} is not horizontal")
            }
            Self::RunTable(source) => write!(formatter, "invalid ledger glyph raster: {source}"),
        }
    }
}

impl Error for LedgerGlyphRasterError {}

/// Materialize the exact filtered-section pixels used by a ledger filament.
///
/// Java's `SectionCompound.toGlyph(null)` computes the minimal member bounds,
/// paints every member into a cropped buffer, and chooses horizontal runs only
/// when the crop is wider than it is tall. The candidate's fitted median and
/// floating-point bounds are intentionally not inputs to this operation.
pub fn materialize_fixed_ledger_glyph(
    section_ids: &[usize],
    filtered_sections: &[Section],
) -> Result<FixedLedgerGlyphRaster, LedgerGlyphRasterError> {
    if section_ids.is_empty() {
        return Err(LedgerGlyphRasterError::EmptySections);
    }

    let mut members = Vec::with_capacity(section_ids.len());
    let mut left = usize::MAX;
    let mut top = usize::MAX;
    let mut right = 0;
    let mut bottom = 0;
    for &section_id in section_ids {
        let section = filtered_sections
            .iter()
            .find(|section| section.id() == section_id)
            .ok_or(LedgerGlyphRasterError::MissingSection(section_id))?;
        if section.orientation() != Orientation::Horizontal {
            return Err(LedgerGlyphRasterError::NonHorizontalSection(section_id));
        }
        let bounds = section.bounds();
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x.checked_add(bounds.width).ok_or(
            LedgerGlyphRasterError::RunTable(RunTableError::InvalidDimensions),
        )?);
        bottom = bottom.max(bounds.y.checked_add(bounds.height).ok_or(
            LedgerGlyphRasterError::RunTable(RunTableError::InvalidDimensions),
        )?);
        members.push(section);
    }

    let width = right
        .checked_sub(left)
        .ok_or(LedgerGlyphRasterError::RunTable(
            RunTableError::InvalidDimensions,
        ))?;
    let height = bottom
        .checked_sub(top)
        .ok_or(LedgerGlyphRasterError::RunTable(
            RunTableError::InvalidDimensions,
        ))?;
    let mut pixels = vec![
        BACKGROUND;
        width
            .checked_mul(height)
            .ok_or(LedgerGlyphRasterError::RunTable(
                RunTableError::InvalidDimensions,
            ))?
    ];
    for section in members {
        for (offset, run) in section.runs().iter().enumerate() {
            let y = section.first_pos() + offset;
            let relative_y = y
                .checked_sub(top)
                .ok_or(LedgerGlyphRasterError::RunTable(RunTableError::OutOfBounds))?;
            for x in run.start..=run.stop() {
                let relative_x = x
                    .checked_sub(left)
                    .ok_or(LedgerGlyphRasterError::RunTable(RunTableError::OutOfBounds))?;
                let pixel = relative_y
                    .checked_mul(width)
                    .and_then(|row| row.checked_add(relative_x))
                    .filter(|&index| index < pixels.len())
                    .ok_or(LedgerGlyphRasterError::RunTable(RunTableError::OutOfBounds))?;
                pixels[pixel] = FOREGROUND;
            }
        }
    }

    let orientation = if width > height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let run_table = RunTable::from_pixels(orientation, width, height, &pixels)
        .map_err(LedgerGlyphRasterError::RunTable)?;
    Ok(FixedLedgerGlyphRaster {
        bounds: Bounds {
            x: left,
            y: top,
            width,
            height,
        },
        run_table,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterializedLedgerInter {
    pub id: usize,
    pub glyph_id: usize,
    pub filament_id: usize,
    pub system_id: usize,
    pub staff_id: usize,
    pub ledger_index: i32,
    pub pitch: i32,
    pub bounds: LedgerFloatBounds,
    pub median: ((f64, f64), (f64, f64)),
    pub thickness: f64,
    pub grade: f64,
    pub impacts: [LedgerCandidateImpact; 7],
    pub removed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedLedgerRelation {
    pub id: usize,
    pub system_id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerIdentityKind {
    Glyph,
    Inter,
    Relation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerMaterializationError {
    IdentityOverflow(LedgerIdentityKind),
    PreMaterializedFilament(usize),
}

impl fmt::Display for LedgerMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityOverflow(kind) => write!(formatter, "{kind:?} identity overflow"),
            Self::PreMaterializedFilament(id) => {
                write!(
                    formatter,
                    "filament {id} already carries glyph/SIG identity"
                )
            }
        }
    }
}

impl Error for LedgerMaterializationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerMaterializationOutcome {
    pub delta: LedgerDelta,
    pub survivor_inter_ids: Vec<usize>,
    /// Exact registration/SIG prefix is retained in both `delta` and state.
    pub error: Option<LedgerMaterializationError>,
}

#[derive(Clone, Debug)]
pub struct LedgerMaterializer {
    next_glyph_id: usize,
    next_inter_id: usize,
    next_relation_id: usize,
    glyphs: Vec<MaterializedLedgerGlyph>,
    inters: Vec<MaterializedLedgerInter>,
    relations: Vec<MaterializedLedgerRelation>,
    by_filament: BTreeMap<usize, (usize, usize)>,
    staff_ownership: BTreeMap<(usize, usize, i32), Vec<usize>>,
}

impl LedgerMaterializer {
    #[must_use]
    pub const fn new(next_glyph_id: usize, next_inter_id: usize, next_relation_id: usize) -> Self {
        Self {
            next_glyph_id,
            next_inter_id,
            next_relation_id,
            glyphs: Vec::new(),
            inters: Vec::new(),
            relations: Vec::new(),
            by_filament: BTreeMap::new(),
            staff_ownership: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn glyphs(&self) -> &[MaterializedLedgerGlyph] {
        &self.glyphs
    }

    #[must_use]
    pub fn glyph_by_id(&self, id: usize) -> Option<&MaterializedLedgerGlyph> {
        self.glyphs.iter().find(|glyph| glyph.id == id)
    }

    #[must_use]
    pub fn inters(&self) -> &[MaterializedLedgerInter] {
        &self.inters
    }

    #[must_use]
    pub fn inter_by_id(&self, id: usize) -> Option<&MaterializedLedgerInter> {
        self.inter(id)
    }

    #[must_use]
    pub fn relations(&self) -> &[MaterializedLedgerRelation] {
        &self.relations
    }

    #[must_use]
    pub const fn next_ids(&self) -> (usize, usize, usize) {
        (
            self.next_glyph_id,
            self.next_inter_id,
            self.next_relation_id,
        )
    }

    #[must_use]
    pub fn staff_inter_ids(&self, system_id: usize, staff_id: usize, index: i32) -> &[usize] {
        self.staff_ownership
            .get(&(system_id, staff_id, index))
            .map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn staff_ledger_indexes(&self, system_id: usize, staff_id: usize) -> Vec<i32> {
        self.staff_ownership
            .keys()
            .filter_map(|&(owner_system, owner_staff, index)| {
                (owner_system == system_id && owner_staff == staff_id).then_some(index)
            })
            .collect()
    }

    /// Java `lookupLine` from `registerOriginal(stick.toGlyph(null))` through
    /// SIG insertion, overlap exclusions/reduction and staff attachment.
    #[must_use]
    pub fn materialize_line(
        &mut self,
        system_id: usize,
        staff_id: usize,
        index: i32,
        candidates_in_factory_order: &[LedgerLineCandidate],
    ) -> LedgerMaterializationOutcome {
        let mut delta = LedgerDelta::default();
        let mut line_inter_ids = Vec::new();
        for evaluated in candidates_in_factory_order {
            let filament_id = evaluated.candidate.id;
            if evaluated.candidate.glyph_id.is_some() || evaluated.candidate.inter_id.is_some() {
                return LedgerMaterializationOutcome {
                    delta,
                    survivor_inter_ids: Vec::new(),
                    error: Some(LedgerMaterializationError::PreMaterializedFilament(
                        filament_id,
                    )),
                };
            }
            let inter_id = if let Some((_, inter_id)) = self.by_filament.get(&filament_id) {
                *inter_id
            } else {
                let Some(glyph_id) = allocate_identity(&mut self.next_glyph_id) else {
                    return identity_failure(delta, LedgerIdentityKind::Glyph);
                };
                self.glyphs.push(MaterializedLedgerGlyph {
                    id: glyph_id,
                    filament_id,
                    section_ids: evaluated.candidate.section_ids.clone(),
                    bounds: evaluated.candidate.bounds,
                });
                delta
                    .mutations
                    .push(LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                        id: glyph_id,
                        section_ids: evaluated.candidate.section_ids.clone(),
                    }));

                let Some(inter_id) = allocate_identity(&mut self.next_inter_id) else {
                    return identity_failure(delta, LedgerIdentityKind::Inter);
                };
                self.inters.push(MaterializedLedgerInter {
                    id: inter_id,
                    glyph_id,
                    filament_id,
                    system_id,
                    staff_id,
                    ledger_index: index,
                    pitch: index,
                    bounds: evaluated.candidate.bounds,
                    // The checks above read `StraightFilament`'s inclusive
                    // pixel regression. Only after acceptance does Java call
                    // `stick.toGlyph(null)` and `Glyph.getCenterLine()`, whose
                    // contour uses the exclusive far x edge and row centres.
                    median: (
                        (
                            evaluated.candidate.start.0,
                            evaluated.candidate.start.1 + 0.5,
                        ),
                        (
                            evaluated.candidate.stop.0 + 1.0,
                            evaluated.candidate.stop.1 + 0.5,
                        ),
                    ),
                    thickness: evaluated.candidate.mean_thickness,
                    grade: evaluated.grade.grade,
                    impacts: evaluated.grade.impacts,
                    removed: false,
                });
                self.by_filament.insert(filament_id, (glyph_id, inter_id));
                delta
                    .mutations
                    .push(LedgerDeltaMutation::AddInter(NeutralLedgerInter {
                        id: inter_id,
                        glyph_id,
                        staff_id,
                        index,
                    }));
                inter_id
            };
            if !self.inter(inter_id).is_some_and(|inter| inter.removed)
                && !line_inter_ids.contains(&inter_id)
            {
                line_inter_ids.push(inter_id);
            }
        }

        line_inter_ids.sort_by(|one, two| {
            self.inter(*one)
                .expect("line inter exists")
                .bounds
                .x
                .total_cmp(&self.inter(*two).expect("line inter exists").bounds.x)
                .then_with(|| one.cmp(two))
        });
        let mut line_relation_ids = Vec::new();
        for left in 0..line_inter_ids.len() {
            for right in left + 1..line_inter_ids.len() {
                let left_inter = self.inter(line_inter_ids[left]).expect("line inter exists");
                let right_inter = self
                    .inter(line_inter_ids[right])
                    .expect("line inter exists");
                if left_inter.bounds.x_overlap(right_inter.bounds) <= 0.0 {
                    break;
                }
                let (source_inter_id, target_inter_id) = if left_inter.id < right_inter.id {
                    (left_inter.id, right_inter.id)
                } else {
                    (right_inter.id, left_inter.id)
                };
                if let Some(relation) = self.relations.iter().find(|relation| {
                    !relation.removed
                        && relation.source_inter_id == source_inter_id
                        && relation.target_inter_id == target_inter_id
                }) {
                    line_relation_ids.push(relation.id);
                    continue;
                }
                let Some(relation_id) = allocate_identity(&mut self.next_relation_id) else {
                    return identity_failure(delta, LedgerIdentityKind::Relation);
                };
                self.relations.push(MaterializedLedgerRelation {
                    id: relation_id,
                    system_id,
                    source_inter_id,
                    target_inter_id,
                    removed: false,
                });
                line_relation_ids.push(relation_id);
                delta
                    .mutations
                    .push(LedgerDeltaMutation::AddRelation(NeutralLedgerRelation {
                        id: relation_id,
                        source_inter_id,
                        target_inter_id,
                    }));
            }
        }

        self.reduce_line_exclusions(system_id, &line_relation_ids, &mut delta);
        let mut survivors = Vec::new();
        for inter_id in line_inter_ids {
            if self.inter(inter_id).is_some_and(|inter| inter.removed) {
                continue;
            }
            self.staff_ownership
                .entry((system_id, staff_id, index))
                .or_default()
                .push(inter_id);
            delta.mutations.push(LedgerDeltaMutation::AttachToStaff {
                system_id,
                staff_id,
                index,
                inter_id,
            });
            survivors.push(inter_id);
        }
        LedgerMaterializationOutcome {
            delta,
            survivor_inter_ids: survivors,
            error: None,
        }
    }

    fn inter(&self, id: usize) -> Option<&MaterializedLedgerInter> {
        self.inters.iter().find(|inter| inter.id == id)
    }

    fn reduce_line_exclusions(
        &mut self,
        system_id: usize,
        line_relation_ids: &[usize],
        delta: &mut LedgerDelta,
    ) {
        loop {
            let mut best = None;
            let mut best_grade = 0.0;
            for (relation_index, relation) in self.relations.iter().enumerate() {
                if relation.removed
                    || relation.system_id != system_id
                    || !line_relation_ids.contains(&relation.id)
                {
                    continue;
                }
                let grade = self
                    .inter(relation.source_inter_id)
                    .map_or(0.0, |inter| inter.grade)
                    .max(
                        self.inter(relation.target_inter_id)
                            .map_or(0.0, |inter| inter.grade),
                    );
                if best_grade < grade {
                    best_grade = grade;
                    best = Some(relation_index);
                }
            }
            let Some(relation_index) = best else {
                break;
            };
            let relation = self.relations[relation_index].clone();
            let source_grade = self
                .inter(relation.source_inter_id)
                .expect("relation source exists")
                .grade;
            let target_grade = self
                .inter(relation.target_inter_id)
                .expect("relation target exists")
                .grade;
            let weaker = if source_grade < target_grade {
                relation.source_inter_id
            } else {
                relation.target_inter_id
            };

            let incident = self
                .relations
                .iter()
                .enumerate()
                .filter(|(_, relation)| {
                    !relation.removed
                        && (relation.source_inter_id == weaker
                            || relation.target_inter_id == weaker)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            for incident_index in incident {
                self.relations[incident_index].removed = true;
                delta.mutations.push(LedgerDeltaMutation::RemoveRelation {
                    system_id,
                    relation_id: self.relations[incident_index].id,
                });
            }
            let ownership_keys = self
                .staff_ownership
                .iter()
                .filter(|(_, inter_ids)| inter_ids.contains(&weaker))
                .map(|(key, _)| *key)
                .collect::<Vec<_>>();
            for (owner_system, staff_id, index) in ownership_keys {
                if let Some(inter_ids) =
                    self.staff_ownership
                        .get_mut(&(owner_system, staff_id, index))
                    && let Some(position) = inter_ids.iter().position(|id| *id == weaker)
                {
                    inter_ids.remove(position);
                }
                delta.mutations.push(LedgerDeltaMutation::DetachFromStaff {
                    system_id: owner_system,
                    staff_id,
                    index,
                    inter_id: weaker,
                });
            }
            self.inters
                .iter_mut()
                .find(|inter| inter.id == weaker)
                .expect("weaker inter exists")
                .removed = true;
            delta.mutations.push(LedgerDeltaMutation::RemoveInter {
                system_id,
                inter_id: weaker,
            });
        }
    }
}

fn allocate_identity(next: &mut usize) -> Option<usize> {
    let id = *next;
    *next = id.checked_add(1)?;
    Some(id)
}

fn identity_failure(delta: LedgerDelta, kind: LedgerIdentityKind) -> LedgerMaterializationOutcome {
    LedgerMaterializationOutcome {
        delta,
        survivor_inter_ids: Vec::new(),
        error: Some(LedgerMaterializationError::IdentityOverflow(kind)),
    }
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        beam_structure::Segment,
        run_table::Run,
        system_population::{
            BoundarySegment, PopulationSystemGeometry, StaffBoundary, SystemStaffBoundaries,
            build_population_system_areas,
        },
    };

    fn boundary(width: i32, y: i32) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (0.0, f64::from(y)),
                end: (f64::from(width - 1), f64::from(y)),
            }],
        }
    }

    fn area(id: usize, width: i32, height: i32) -> PopulationSystemArea {
        build_population_system_areas(
            &[PopulationSystemGeometry {
                system_id: id,
                left: 0,
                width,
                top: 0,
                bottom: height - 1,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            }],
            &[SystemStaffBoundaries {
                first_line: boundary(width, 0),
                last_line: boundary(width, height - 1),
            }],
            width,
            height,
            0,
        )[0]
        .clone()
    }

    fn staff() -> RawLedgerStaffZone {
        RawLedgerStaffZone {
            id: 5,
            system_id: 1,
            specific_interline: 10,
            tablature: false,
            merged_part: false,
            first_in_part: true,
            last_in_part: true,
            area: area(5, 40, 40),
            first_line: boundary(40, 20),
            last_line: boundary(40, 24),
        }
    }

    #[test]
    fn raw_filter_keeps_exact_distance_boundary_builds_zero_shift_sections_and_purges_beams() {
        let mut raster = RunTable::new(Orientation::Horizontal, 40, 40).unwrap();
        // At y=15, distance is 5 == rint(10 * .5): retained. Identical
        // preceding run joins it; shifted endpoint at y=13 starts a section.
        // The beam covers y=[13,14), so its positive-area overlap with that
        // first section's box is detected even though no sampled pixel centre
        // lies inside the beam.
        assert!(raster.add_run(13, Run::new(5, 8)).unwrap());
        assert!(raster.add_run(14, Run::new(4, 8)).unwrap());
        assert!(raster.add_run(15, Run::new(4, 8)).unwrap());
        // Inside the staff core: rejected before section construction.
        assert!(raster.add_run(22, Run::new(20, 5)).unwrap());
        let staff = staff();
        let system_area = area(1, 40, 40);
        let system = RawLedgerSystemZone {
            id: 1,
            left: 0,
            right: 39,
            area: system_area.clone(),
            all_beams: vec![RawLedgerBeamArea {
                bounds: Bounds {
                    x: 5,
                    y: 13,
                    width: 8,
                    height: 1,
                },
                item: BeamItem {
                    median: Segment {
                        x1: 5.0,
                        y1: 13.5,
                        x2: 13.0,
                        y2: 13.5,
                    },
                    height: 1.0,
                },
            }],
            good_full_beams: Vec::new(),
        };

        let output = filter_raw_ledger_sections(
            &raster,
            &[staff],
            &[system],
            RawLedgerFilterParameters::default(),
        )
        .unwrap();

        assert_eq!(output.run_table.total_run_count(), 3);
        assert_eq!(output.sections.len(), 2);
        assert_eq!(output.sections[0].id(), 1);
        assert_eq!(output.sections[0].run_count(), 1);
        assert_eq!(output.sections[1].id(), 2);
        assert_eq!(output.sections[1].run_count(), 2);
        assert_eq!(output.by_system[0].system_id, 1);
        assert_eq!(
            output.by_system[0]
                .sections
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn fixed_ledger_glyph_uses_exact_referenced_sections_and_minimal_bounds() {
        let mut raster = RunTable::new(Orientation::Horizontal, 12, 8).unwrap();
        assert!(raster.add_run(2, Run::new(3, 4)).unwrap());
        assert!(raster.add_run(3, Run::new(3, 4)).unwrap());
        assert!(raster.add_run(5, Run::new(8, 2)).unwrap());
        let sections = build_sections_from_id(&raster, JunctionPolicy::Shift { max_shift: 0 }, 1);
        assert_eq!(sections.len(), 2);

        // Member order does not alter the painted pixels. The crop comes from
        // the sections themselves, never from fitted candidate geometry.
        let glyph = materialize_fixed_ledger_glyph(&[2, 1], &sections).unwrap();
        assert_eq!(
            glyph.bounds,
            Bounds {
                x: 3,
                y: 2,
                width: 7,
                height: 4,
            }
        );
        assert_eq!(glyph.run_table.orientation(), Orientation::Horizontal);
        assert_eq!(
            (0..glyph.run_table.sequence_count())
                .map(|sequence| {
                    glyph
                        .run_table
                        .sequence(sequence)
                        .unwrap_or_default()
                        .to_vec()
                })
                .collect::<Vec<_>>(),
            vec![
                vec![Run::new(0, 4)],
                vec![Run::new(0, 4)],
                Vec::new(),
                vec![Run::new(5, 2)],
            ]
        );
        assert_eq!(
            materialize_fixed_ledger_glyph(&[3], &sections),
            Err(LedgerGlyphRasterError::MissingSection(3))
        );
        assert_eq!(
            materialize_fixed_ledger_glyph(&[], &sections),
            Err(LedgerGlyphRasterError::EmptySections)
        );
    }

    #[derive(Default)]
    struct Factory {
        candidates: Vec<RawLedgerCandidate>,
        inputs: Vec<(usize, i32, i32, f64, Vec<usize>)>,
    }

    impl HorizontalLedgerGlyphFactory for Factory {
        type Error = &'static str;

        fn build_horizontal_candidates(
            &mut self,
            input: LedgerGlyphFactoryInput<'_>,
        ) -> Result<Vec<RawLedgerCandidate>, Self::Error> {
            self.inputs.push((
                input.system_id,
                input.maximum_thickness,
                input.minimum_core_section_length,
                input.minimum_side_ratio,
                input.sections.iter().map(Section::id).collect(),
            ));
            Ok(std::mem::take(&mut self.candidates))
        }
    }

    fn candidate(id: usize, inter_id: usize, x: f64, y: f64) -> RawLedgerCandidate {
        RawLedgerCandidate {
            id,
            glyph_id: Some(id + 100),
            inter_id: Some(inter_id),
            section_ids: vec![id],
            bounds: LedgerFloatBounds {
                x,
                y: y - 1.0,
                width: 20.0,
                height: 2.0,
            },
            start: (x, y),
            stop: (x + 19.0, y),
            mean_thickness: 2.5,
            mean_distance: 0.0,
            convex_end_count: 2,
            overlaps_good_beam: false,
        }
    }

    #[test]
    fn factory_is_first_seam_and_system_purges_preserve_candidate_order() {
        let mut beam = candidate(1, 1, 0.0, 10.0);
        beam.overlaps_good_beam = true;
        let mut long = candidate(2, 2, 0.0, 10.0);
        long.bounds.width = 201.0;
        let kept_two = candidate(4, 4, 15.0, 10.0);
        let kept_one = candidate(3, 3, 5.0, 10.0);
        let mut factory = Factory {
            candidates: vec![beam, long, kept_two.clone(), kept_one.clone()],
            ..Factory::default()
        };
        let empty = RunTable::new(Orientation::Horizontal, 2, 2).unwrap();
        let sections = build_sections_from_id(&empty, JunctionPolicy::Shift { max_shift: 0 }, 1);

        let output = source_ledger_candidates(
            &mut factory,
            9,
            &sections,
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
        )
        .unwrap();

        assert_eq!(output, vec![kept_two, kept_one]);
        // min(rint(2*3.25)=6, rint(10*.4)=4), then one interline core.
        assert_eq!(factory.inputs, vec![(9, 4, 10, 0.9, Vec::new())]);
    }

    #[test]
    fn line_lookup_applies_position_grade_order_and_overlap_reduction() {
        let staff = staff();
        let left = candidate(2, 10, 5.0, 10.0);
        let right = candidate(1, 20, 15.0, 10.0);
        let outside = candidate(3, 30, 5.0, 30.0);
        let mut too_short = candidate(4, 40, 30.0, 10.0);
        too_short.bounds.width = 5.0;
        too_short.stop.0 = 34.0;

        let accepted = evaluate_ledger_line(
            &staff,
            -1,
            &[right, outside, too_short, left.clone()],
            &[],
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
        );

        // Left and right overlap. Equal perfect grade removes the higher inter
        // ID (20), regardless of factory order; bad-position/length probes fail.
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].candidate, left);
        assert_eq!(accepted[0].grade.staff_id, 5);
        assert_eq!(accepted[0].grade.index, -1);
        assert_eq!(accepted[0].grade.y_target, 10.0);
        assert!((accepted[0].grade.grade - 0.8).abs() < 1e-12);
        assert_eq!(
            accepted[0]
                .grade
                .impacts
                .iter()
                .map(|impact| impact.weight)
                .collect::<Vec<_>>(),
            vec![0.5, 0.0, 4.0, 2.0, 1.0, 0.5, 0.5]
        );
    }

    #[test]
    fn outer_lines_require_strict_previous_overlap_and_wide_reference_raises_length_gate() {
        let staff = staff();
        let candidate = candidate(7, 7, 10.0, 0.0);
        let exact_overlap = LedgerPreviousReference {
            candidate_id: 1,
            bounds: LedgerFloatBounds {
                x: 22.5,
                y: 9.0,
                width: 20.0,
                height: 2.0,
            },
            start: (22.5, 10.0),
            stop: (42.5, 10.0),
        };
        assert!(
            evaluate_ledger_line(
                &staff,
                -2,
                std::slice::from_ref(&candidate),
                &[exact_overlap],
                RawLedgerScale {
                    large_interline: 10,
                    mean_line_thickness: 2.0,
                },
                RawLedgerCandidateParameters::default(),
            )
            .is_empty()
        );
        let wide = LedgerPreviousReference {
            candidate_id: 2,
            bounds: LedgerFloatBounds {
                x: 10.0,
                y: 9.0,
                width: 15.0,
                height: 2.0,
            },
            start: (10.0, 10.0),
            stop: (25.0, 10.0),
        };
        let accepted = evaluate_ledger_line(
            &staff,
            -2,
            &[candidate],
            &[wide],
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
        );
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].grade.y_target, 0.0);
        assert_eq!(accepted[0].grade.impacts[2].value, 2.0);
        assert_eq!(accepted[0].grade.impacts[2].grade, 1.0);
    }

    #[test]
    fn tablature_and_merged_grand_staff_limits_precede_candidate_grading() {
        let candidate = candidate(1, 1, 5.0, 10.0);
        let scale = RawLedgerScale {
            large_interline: 10,
            mean_line_thickness: 2.0,
        };
        let mut tablature = staff();
        tablature.tablature = true;
        assert!(
            evaluate_ledger_line(
                &tablature,
                -1,
                std::slice::from_ref(&candidate),
                &[],
                scale,
                RawLedgerCandidateParameters::default(),
            )
            .is_empty()
        );
        let mut lower_merged_staff = staff();
        lower_merged_staff.merged_part = true;
        lower_merged_staff.first_in_part = false;
        lower_merged_staff.last_in_part = true;
        assert!(
            evaluate_ledger_line(
                &lower_merged_staff,
                -1,
                &[candidate],
                &[],
                scale,
                RawLedgerCandidateParameters::default(),
            )
            .is_empty()
        );
    }

    #[test]
    fn native_stick_factory_feeds_unmaterialized_grade_tail_in_registration_order() {
        let mut raster = RunTable::new(Orientation::Horizontal, 50, 40).unwrap();
        assert!(raster.add_run(10, Run::new(2, 20)).unwrap());
        let sections = build_sections_from_id(&raster, JunctionPolicy::Shift { max_shift: 0 }, 1);
        let system = RawLedgerSystemZone {
            id: 1,
            left: 0,
            right: 49,
            area: area(1, 50, 40),
            all_beams: Vec::new(),
            good_full_beams: Vec::new(),
        };

        let outcome = source_native_ledger_candidates(
            &raster,
            &system,
            &sections,
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
            50,
        );

        assert_eq!(outcome.error, None);
        assert_eq!(outcome.registered_filament_ids, vec![50]);
        assert_eq!(outcome.next_filament_id, 51);
        assert_eq!(outcome.candidates.len(), 1);
        assert_eq!(outcome.candidates[0].id, 50);
        assert_eq!(outcome.candidates[0].glyph_id, None);
        assert_eq!(outcome.candidates[0].inter_id, None);
        assert_eq!(outcome.candidates[0].section_ids, vec![1]);
        assert_eq!(outcome.candidates[0].convex_end_count, 2);

        let accepted = evaluate_ledger_line(
            &staff(),
            -1,
            &outcome.candidates,
            &[],
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
        );
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].candidate.id, 50);
        assert!(accepted[0].grade.grade >= 0.08);
    }

    #[test]
    fn native_factory_failure_keeps_registered_and_converted_candidate_prefix() {
        let mut raster = RunTable::new(Orientation::Horizontal, 50, 40).unwrap();
        assert!(raster.add_run(10, Run::new(2, 20)).unwrap());
        assert!(raster.add_run(10, Run::new(30, 12)).unwrap());
        let sections = build_sections_from_id(&raster, JunctionPolicy::Shift { max_shift: 0 }, 1);
        let system = RawLedgerSystemZone {
            id: 1,
            left: 0,
            right: 49,
            area: area(1, 50, 40),
            all_beams: Vec::new(),
            good_full_beams: Vec::new(),
        };

        let outcome = source_native_ledger_candidates(
            &raster,
            &system,
            &sections,
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
            u64::MAX - 1,
        );

        assert_eq!(outcome.error, Some(StraightStickError::IdentityOverflow));
        assert_eq!(outcome.registered_filament_ids, vec![u64::MAX - 1]);
        assert_eq!(outcome.next_filament_id, u64::MAX);
        assert_eq!(outcome.candidates.len(), 1);
        assert_eq!(outcome.candidates[0].id, usize::MAX - 1);
    }

    fn unresolved_candidate(id: usize, x: f64, y: f64) -> RawLedgerCandidate {
        let mut candidate = candidate(id, id, x, y);
        candidate.glyph_id = None;
        candidate.inter_id = None;
        candidate
    }

    fn evaluated_unresolved(candidates: &[RawLedgerCandidate]) -> Vec<LedgerLineCandidate> {
        evaluate_ledger_line_unreduced(
            &staff(),
            -1,
            candidates,
            &[],
            RawLedgerScale {
                large_interline: 10,
                mean_line_thickness: 2.0,
            },
            RawLedgerCandidateParameters::default(),
        )
    }

    #[test]
    fn materializer_registers_in_factory_order_then_reduces_in_abscissa_sig_order() {
        // Factory order is right then left. Exact equal grades make Java remove
        // the exclusion target (higher inter ID), not whichever is rightmost.
        let right = unresolved_candidate(50, 15.0, 10.0);
        let left = unresolved_candidate(51, 5.0, 10.0);
        let evaluated = evaluated_unresolved(&[right.clone(), left.clone()]);
        let mut materializer = LedgerMaterializer::new(10, 100, 500);

        let outcome = materializer.materialize_line(1, 5, -1, &evaluated);

        assert_eq!(outcome.error, None);
        assert_eq!(outcome.survivor_inter_ids, vec![100]);
        assert_eq!(materializer.next_ids(), (12, 102, 501));
        assert_eq!(
            materializer
                .glyphs()
                .iter()
                .map(|glyph| (glyph.id, glyph.filament_id, glyph.bounds.x))
                .collect::<Vec<_>>(),
            vec![(10, 50, 15.0), (11, 51, 5.0)]
        );
        assert_eq!(materializer.inters()[0].glyph_id, 10);
        assert_eq!(materializer.inters()[0].staff_id, 5);
        assert_eq!(materializer.inters()[0].ledger_index, -1);
        assert_eq!(materializer.inters()[0].pitch, -1);
        assert_eq!(materializer.inters()[0].bounds, right.bounds);
        assert_eq!(materializer.inters()[0].grade, evaluated[0].grade.grade);
        assert_eq!(materializer.inters()[0].impacts, evaluated[0].grade.impacts);
        assert!(!materializer.inters()[0].removed);
        assert!(materializer.inters()[1].removed);
        assert_eq!(
            materializer.relations(),
            &[MaterializedLedgerRelation {
                id: 500,
                system_id: 1,
                source_inter_id: 100,
                target_inter_id: 101,
                removed: true,
            }]
        );
        assert_eq!(materializer.staff_inter_ids(1, 5, -1), [100]);
        assert_eq!(
            outcome.delta.mutations,
            vec![
                LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                    id: 10,
                    section_ids: right.section_ids,
                }),
                LedgerDeltaMutation::AddInter(NeutralLedgerInter {
                    id: 100,
                    glyph_id: 10,
                    staff_id: 5,
                    index: -1,
                }),
                LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                    id: 11,
                    section_ids: left.section_ids,
                }),
                LedgerDeltaMutation::AddInter(NeutralLedgerInter {
                    id: 101,
                    glyph_id: 11,
                    staff_id: 5,
                    index: -1,
                }),
                LedgerDeltaMutation::AddRelation(NeutralLedgerRelation {
                    id: 500,
                    source_inter_id: 100,
                    target_inter_id: 101,
                }),
                LedgerDeltaMutation::RemoveRelation {
                    system_id: 1,
                    relation_id: 500,
                },
                LedgerDeltaMutation::RemoveInter {
                    system_id: 1,
                    inter_id: 101,
                },
                LedgerDeltaMutation::AttachToStaff {
                    system_id: 1,
                    staff_id: 5,
                    index: -1,
                    inter_id: 100,
                },
            ]
        );
    }

    #[test]
    fn reused_filament_keeps_creation_pitch_and_adds_adjacent_staff_ownership_only() {
        let raw = unresolved_candidate(7, 5.0, 10.0);
        let evaluated = evaluated_unresolved(std::slice::from_ref(&raw));
        let mut materializer = LedgerMaterializer::new(1, 20, 30);
        let first = materializer.materialize_line(1, 5, -1, &evaluated);
        assert_eq!(first.survivor_inter_ids, vec![20]);

        let mut outer = evaluated.clone();
        outer[0].grade.index = -2;
        outer[0].grade.y_target = 0.0;
        let second = materializer.materialize_line(1, 5, -2, &outer);

        assert_eq!(second.error, None);
        assert_eq!(second.survivor_inter_ids, vec![20]);
        assert_eq!(materializer.next_ids(), (2, 21, 30));
        assert_eq!(materializer.glyphs().len(), 1);
        assert_eq!(materializer.inters().len(), 1);
        assert_eq!(materializer.inters()[0].ledger_index, -1);
        assert_eq!(materializer.inters()[0].pitch, -1);
        assert_eq!(materializer.staff_inter_ids(1, 5, -1), [20]);
        assert_eq!(materializer.staff_inter_ids(1, 5, -2), [20]);
        assert_eq!(materializer.staff_ledger_indexes(1, 5), vec![-2, -1]);
        assert_eq!(
            second.delta.mutations,
            vec![LedgerDeltaMutation::AttachToStaff {
                system_id: 1,
                staff_id: 5,
                index: -2,
                inter_id: 20,
            }]
        );
    }

    #[test]
    fn inter_identity_failure_retains_registered_glyph_prefix() {
        let raw = unresolved_candidate(9, 5.0, 10.0);
        let evaluated = evaluated_unresolved(&[raw]);
        let mut materializer = LedgerMaterializer::new(4, usize::MAX, 30);

        let outcome = materializer.materialize_line(1, 5, -1, &evaluated);

        assert_eq!(
            outcome.error,
            Some(LedgerMaterializationError::IdentityOverflow(
                LedgerIdentityKind::Inter
            ))
        );
        assert_eq!(materializer.glyphs().len(), 1);
        assert!(materializer.inters().is_empty());
        assert_eq!(materializer.next_ids(), (5, usize::MAX, 30));
        assert_eq!(
            outcome.delta.mutations,
            vec![LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                id: 4,
                section_ids: vec![9],
            })]
        );
    }

    #[test]
    fn relation_identity_failure_retains_both_sig_vertices_without_attachment() {
        let evaluated = evaluated_unresolved(&[
            unresolved_candidate(1, 15.0, 10.0),
            unresolved_candidate(2, 5.0, 10.0),
        ]);
        let mut materializer = LedgerMaterializer::new(1, 10, usize::MAX);

        let outcome = materializer.materialize_line(1, 5, -1, &evaluated);

        assert_eq!(
            outcome.error,
            Some(LedgerMaterializationError::IdentityOverflow(
                LedgerIdentityKind::Relation
            ))
        );
        assert_eq!(materializer.glyphs().len(), 2);
        assert_eq!(materializer.inters().len(), 2);
        assert!(materializer.relations().is_empty());
        assert!(materializer.staff_inter_ids(1, 5, -1).is_empty());
        assert_eq!(outcome.delta.mutations.len(), 4);
    }

    #[test]
    fn pre_materialized_candidate_fails_before_any_registry_mutation() {
        let evaluated = evaluated_unresolved(&[unresolved_candidate(1, 5.0, 10.0)]);
        let mut invalid = evaluated;
        invalid[0].candidate.glyph_id = Some(99);
        let mut materializer = LedgerMaterializer::new(1, 10, 20);

        let outcome = materializer.materialize_line(1, 5, -1, &invalid);

        assert_eq!(
            outcome.error,
            Some(LedgerMaterializationError::PreMaterializedFilament(1))
        );
        assert!(outcome.delta.mutations.is_empty());
        assert!(materializer.glyphs().is_empty());
        assert!(materializer.inters().is_empty());
    }
}
