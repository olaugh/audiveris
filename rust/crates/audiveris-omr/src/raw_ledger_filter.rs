// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete dependency-light prefix of Java `LedgersFilter` and
//! `LedgersBuilder.lookupLine`.
//!
//! The raw no-staff raster, zero-shift sections, staff/system dispatch, beam
//! purge, deterministic candidate gates, and intrinsic ledger grade are native.
//! `HorizontalLedgerGlyphFactory` is the first injected geometric collaborator:
//! Java's horizontal `StickFactory`/`StraightFilament` construction.

use std::{error::Error, fmt};

use audiveris_image::{
    run_table::{Orientation, RunTable, RunTableError},
    section::{Bounds, JunctionPolicy, Section, build_sections_from_id},
    system_population::PopulationSystemArea,
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
    pub first_line: LedgerLineSegment,
    pub last_line: LedgerLineSegment,
}

impl RawLedgerStaffZone {
    #[must_use]
    pub fn distance_to(&self, x: f64, y: f64) -> f64 {
        (self.first_line.y_at(x) - y).max(y - self.last_line.y_at(x))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerBeamArea {
    pub bounds: Bounds,
    pub area: PopulationSystemArea,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerSystemZone {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub area: PopulationSystemArea,
    /// Good full beams only; hooks are deliberately absent.
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
                    .good_full_beams
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
    for (offset, run) in section.runs().iter().enumerate() {
        let y = section.first_pos() + offset;
        for x in run.start..=run.stop() {
            if beam.area.contains(x as f64, y as f64) {
                return true;
            }
        }
    }
    false
}

fn bounds_intersect(one: Bounds, two: Bounds) -> bool {
    one.x < two.x.saturating_add(two.width)
        && two.x < one.x.saturating_add(one.width)
        && one.y < two.y.saturating_add(two.height)
        && two.y < one.y.saturating_add(one.height)
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawLedgerCandidate {
    pub id: usize,
    pub glyph_id: usize,
    pub inter_id: usize,
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
    if staff.tablature || index == 0 || staff.specific_interline <= 0 {
        return Vec::new();
    }
    if staff.merged_part
        && ((staff.first_in_part && index > 1) || (staff.last_in_part && index < 0))
    {
        return Vec::new();
    }
    let interline = f64::from(staff.specific_interline);
    let margin = f64::from(java_rint(interline * parameters.ledger_margin_y));
    let reference_line = if index < 0 {
        staff.first_line
    } else {
        staff.last_line
    };
    let mut virtual_bounds = reference_line.bounds();
    virtual_bounds.y += f64::from(index) * interline;
    // Rectangle.grow(0, 2*yMargin) grows each vertical side by 2*yMargin.
    virtual_bounds.y -= 2.0 * margin;
    virtual_bounds.height += 4.0 * margin;
    let minimum_overlap = f64::from(scale.large_interline) * parameters.minimum_abscissa_overlap;
    let minimum_wide = f64::from(java_rint(interline * parameters.minimum_wide_ledger_length));
    let mut accepted = Vec::new();

    for candidate in candidates {
        let middle = (
            (candidate.start.0 + candidate.stop.0) / 2.0,
            (candidate.start.1 + candidate.stop.1) / 2.0,
        );
        if !virtual_bounds.contains(middle.0, middle.1) {
            continue;
        }
        let (y_reference, wide_reference) = if index.abs() == 1 {
            (reference_line.y_at(middle.0), false)
        } else {
            let Some(reference) = previous
                .iter()
                .find(|reference| reference.bounds.x_overlap(candidate.bounds) > minimum_overlap)
            else {
                continue;
            };
            let y = y_on_reference(reference, middle.0);
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
            accepted.push(LedgerLineCandidate {
                candidate: candidate.clone(),
                grade,
            });
        }
    }

    accepted.sort_by(|one, two| {
        one.candidate
            .bounds
            .x
            .total_cmp(&two.candidate.bounds.x)
            .then_with(|| one.candidate.id.cmp(&two.candidate.id))
    });
    reduce_overlaps(accepted)
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
            || candidates[left].candidate.inter_id < candidates[right].candidate.inter_id
        {
            right
        } else {
            left
        };
        candidates.remove(remove);
    }
    candidates
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
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
            first_line: LedgerLineSegment {
                start_x: 0.0,
                start_y: 20.0,
                stop_x: 39.0,
                stop_y: 20.0,
            },
            last_line: LedgerLineSegment {
                start_x: 0.0,
                start_y: 24.0,
                stop_x: 39.0,
                stop_y: 24.0,
            },
        }
    }

    #[test]
    fn raw_filter_keeps_exact_distance_boundary_builds_zero_shift_sections_and_purges_beams() {
        let mut raster = RunTable::new(Orientation::Horizontal, 40, 40).unwrap();
        // At y=15, distance is 5 == rint(10 * .5): retained. Identical
        // preceding run joins it; shifted endpoint at y=13 starts a section.
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
            good_full_beams: vec![RawLedgerBeamArea {
                bounds: Bounds {
                    x: 5,
                    y: 13,
                    width: 8,
                    height: 1,
                },
                area: system_area,
            }],
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
            glyph_id: id + 100,
            inter_id,
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
}
