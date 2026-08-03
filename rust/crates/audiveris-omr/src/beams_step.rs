// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `BeamsStep`.
//!
//! Image closing, beam interpretation, and multiple-rest geometry remain typed
//! visual seams. This module owns their sheet/system ordering, mutations,
//! checked-error continuation, and unconditional BEAM_SPOT cleanup.

use std::{error::Error, fmt};

use audiveris_image::{
    global_filter::global_filter,
    run_table::{Orientation, RunTable, RunTableError},
    system_population::PopulationSystemArea,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralBeamGlyphGroup {
    BeamSpot,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralBeamGlyph {
    pub id: usize,
    pub groups: Vec<NeutralBeamGlyphGroup>,
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
    pub area: PopulationSystemArea,
    pub free_glyphs: Vec<NeutralBeamGlyph>,
    pub inters: Vec<NeutralBeamInter>,
    pub relations: Vec<NeutralBeamRelation>,
    pub beam_group_ids: Vec<usize>,
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
    /// Sheet-global GlyphIndex ownership, in first-registration order.
    pub registered_glyph_ids: Vec<usize>,
    pub mutations: Vec<BeamsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedBeamSpot {
    pub glyph_id: usize,
    pub center_x: i32,
    pub center_y: i32,
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
pub struct BeamGlyphBuildInput<'a> {
    pub sheet_id: usize,
    pub spot_runs: &'a RunTable,
    pub run_orientation: Orientation,
    pub compute_black_head_sizing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeamSpotWarning<VisualError> {
    MissingBeamScale,
    Visual(VisualError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BeamSystemDelta {
    /// Exact Java mutation order, including prefixes completed before failure.
    pub mutations: Vec<BeamSystemMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeamSystemMutation {
    RegisterGlyph(NeutralBeamGlyph),
    AddInter(NeutralBeamInter),
    RemoveInter(usize),
    AddRelation(NeutralBeamRelation),
    AddBeamGroup(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

/// First unavailable dependencies in Java: ImageJ morphology, connected-glyph
/// construction, `BeamsBuilder`, and `MultipleRestsBuilder` geometry.
pub trait VisualBeams {
    type Error;

    fn close_beam_spots(
        &mut self,
        input: BeamClosingInput,
    ) -> Result<ClosedBeamRaster, Self::Error>;

    fn build_spot_glyphs(
        &mut self,
        input: BeamGlyphBuildInput<'_>,
    ) -> Result<Vec<DetectedBeamSpot>, Self::Error>;

    fn build_beams(&mut self, input: BeamSystemInput<'_>) -> BeamStageOutcome<Self::Error>;

    fn build_multiple_rests(&mut self, input: BeamSystemInput<'_>)
    -> BeamStageOutcome<Self::Error>;
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
}

impl<Visual> HeadlessBeamsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
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
            let beams = self.visual.build_beams(BeamSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, beams.delta)?;
            if let Some(error) = beams.error {
                sheet
                    .mutations
                    .push(BeamsMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
                continue;
            }

            let rests = self.visual.build_multiple_rests(BeamSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, rests.delta)?;
            if let Some(error) = rests.error {
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

    fn dispatch_spots(
        &self,
        sheet: &mut NeutralBeamSheet,
        spots: Vec<DetectedBeamSpot>,
    ) -> Result<(), BeamsContractError> {
        for spot in spots {
            let mut registered = false;
            for system_index in 0..sheet.systems.len() {
                let system = &sheet.systems[system_index];
                if !system
                    .area
                    .contains(f64::from(spot.center_x), f64::from(spot.center_y))
                {
                    continue;
                }
                if spot.center_x < system.left || spot.center_x > system.right {
                    continue;
                }
                let system_id = system.id;
                if !registered {
                    if sheet.registered_glyph_ids.contains(&spot.glyph_id) {
                        return Err(BeamsContractError::DuplicateGlyph(spot.glyph_id));
                    }
                    sheet.registered_glyph_ids.push(spot.glyph_id);
                    registered = true;
                }
                sheet.mutations.push(BeamsMutation::SpotRegistered {
                    system_id,
                    glyph_id: spot.glyph_id,
                });
                sheet.systems[system_index]
                    .free_glyphs
                    .push(NeutralBeamGlyph {
                        id: spot.glyph_id,
                        groups: vec![NeutralBeamGlyphGroup::BeamSpot],
                    });
                sheet.mutations.push(BeamsMutation::SpotAttached {
                    system_id,
                    glyph_id: spot.glyph_id,
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
        let spots = match self.visual.build_spot_glyphs(BeamGlyphBuildInput {
            sheet_id: sheet.id,
            spot_runs: &spot_runs,
            run_orientation: Orientation::Horizontal,
            compute_black_head_sizing: !sheet.one_line_staves && !sheet.drum_notation,
        }) {
            Ok(spots) => spots,
            Err(error) => return Ok(Some(BeamSpotWarning::Visual(error))),
        };
        self.dispatch_spots(sheet, spots)?;
        Ok(None)
    }
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
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
        spots: Option<Result<Vec<DetectedBeamSpot>, &'static str>>,
        beams: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        rests: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        calls: Vec<(&'static str, usize)>,
        closing_inputs: Vec<BeamClosingInput>,
        head_sizing_inputs: Vec<bool>,
        spot_run_pixels: Vec<Vec<u8>>,
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

        fn build_spot_glyphs(
            &mut self,
            input: BeamGlyphBuildInput<'_>,
        ) -> Result<Vec<DetectedBeamSpot>, Self::Error> {
            self.calls.push(("glyphs", input.sheet_id));
            assert_eq!(input.run_orientation, Orientation::Horizontal);
            self.spot_run_pixels.push(input.spot_runs.to_pixels());
            self.head_sizing_inputs
                .push(input.compute_black_head_sizing);
            self.spots.take().unwrap()
        }

        fn build_beams(&mut self, input: BeamSystemInput<'_>) -> BeamStageOutcome<Self::Error> {
            self.calls.push(("beams", input.system.id));
            self.beams
                .remove(&input.system.id)
                .unwrap_or_else(|| BeamStageOutcome::success(BeamSystemDelta::default()))
        }

        fn build_multiple_rests(
            &mut self,
            input: BeamSystemInput<'_>,
        ) -> BeamStageOutcome<Self::Error> {
            self.calls.push(("rests", input.system.id));
            self.rests
                .remove(&input.system.id)
                .unwrap_or_else(|| BeamStageOutcome::success(BeamSystemDelta::default()))
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
            area,
            free_glyphs: Vec::new(),
            inters: Vec::new(),
            relations: Vec::new(),
            beam_group_ids: Vec::new(),
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

    fn visual_with_spots(spots: Vec<DetectedBeamSpot>) -> FakeVisual {
        FakeVisual {
            closed: Some(Ok(raster())),
            spots: Some(Ok(spots)),
            ..FakeVisual::default()
        }
    }

    #[test]
    fn runs_prolog_system_stages_and_epilog_in_java_order() {
        let visual = visual_with_spots(vec![DetectedBeamSpot {
            glyph_id: 7,
            center_x: 44,
            center_y: 25,
        }]);
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, Vec::new());
        assert_eq!(
            step.visual().calls,
            vec![
                ("close", 9),
                ("glyphs", 9),
                ("beams", 2),
                ("rests", 2),
                ("beams", 1),
                ("rests", 1)
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
        assert_eq!(step.visual().head_sizing_inputs, vec![true]);
        assert_eq!(
            step.visual().spot_run_pixels,
            vec![vec![0, 0, 255, 255, 255, 255]]
        );
        assert_eq!(
            sheet.head_spot_runs.as_ref().unwrap().to_pixels(),
            vec![0, 0, 0, 0, 0, 255]
        );
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
            vec![("beams", 2), ("rests", 2), ("beams", 1), ("rests", 1)]
        );
        assert!(sheet.head_spot_runs.is_none());
    }

    #[test]
    fn glyph_failure_retains_saved_head_runs_and_disables_sizing_for_one_line_staff() {
        let visual = FakeVisual {
            closed: Some(Ok(raster())),
            spots: Some(Err("glyph factory failed")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.one_line_staves = true;

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.spot_warning,
            Some(BeamSpotWarning::Visual("glyph factory failed"))
        );
        assert_eq!(step.visual().head_sizing_inputs, vec![false]);
        assert!(sheet.head_spot_runs.is_some());
        assert_eq!(sheet.mutations, vec![BeamsMutation::HeadSpotsSaved]);
    }

    #[test]
    fn dispatch_keeps_spot_on_inclusive_system_left_boundary() {
        let visual = visual_with_spots(vec![DetectedBeamSpot {
            glyph_id: 8,
            center_x: 10,
            center_y: 25,
        }]);
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

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
    fn checked_beam_failure_keeps_prefix_skips_rests_continues_and_cleans_up() {
        let mut visual = visual_with_spots(Vec::new());
        visual.beams.insert(
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
        visual.rests.insert(
            1,
            BeamStageOutcome::success(BeamSystemDelta {
                mutations: vec![BeamSystemMutation::AddInter(NeutralBeamInter {
                    id: 30,
                    kind: NeutralBeamInterKind::MultipleRest,
                })],
            }),
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "beam failure")]);
        assert_eq!(
            step.visual().calls,
            vec![
                ("close", 9),
                ("glyphs", 9),
                ("beams", 2),
                ("beams", 1),
                ("rests", 1)
            ]
        );
        assert_eq!(sheet.systems[0].inters[0].id, 20);
        assert_eq!(sheet.systems[1].inters[0].id, 30);
        assert_eq!(
            sheet.mutations,
            vec![
                BeamsMutation::HeadSpotsSaved,
                BeamsMutation::InterAdded {
                    system_id: 2,
                    inter_id: 20
                },
                BeamsMutation::SystemFailed { system_id: 2 },
                BeamsMutation::InterAdded {
                    system_id: 1,
                    inter_id: 30
                },
            ]
        );
    }

    #[test]
    fn multiple_rest_delta_preserves_registration_add_remove_relation_order() {
        let mut visual = visual_with_spots(Vec::new());
        visual.beams.insert(
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
        visual.rests.insert(
            2,
            BeamStageOutcome::success(BeamSystemDelta {
                mutations: vec![
                    BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 21,
                        kind: NeutralBeamInterKind::MultipleRest,
                    }),
                    BeamSystemMutation::RegisterGlyph(NeutralBeamGlyph {
                        id: 70,
                        groups: vec![NeutralBeamGlyphGroup::Other],
                    }),
                    BeamSystemMutation::AddInter(NeutralBeamInter {
                        id: 22,
                        kind: NeutralBeamInterKind::VerticalSerif,
                    }),
                    BeamSystemMutation::AddRelation(NeutralBeamRelation {
                        id: 300,
                        source_inter_id: 21,
                        target_inter_id: 22,
                    }),
                    BeamSystemMutation::RemoveInter(20),
                ],
            }),
        );
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.registered_glyph_ids, vec![70]);
        assert_eq!(sheet.systems[0].inters[0].id, 21);
        assert_eq!(sheet.systems[0].beam_group_ids, vec![200]);
        assert_eq!(sheet.systems[0].relations[0].id, 300);
    }

    #[test]
    fn epilog_keeps_beam_spots_that_also_own_another_group() {
        let visual = visual_with_spots(Vec::new());
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();
        sheet.systems[0].free_glyphs.push(NeutralBeamGlyph {
            id: 5,
            groups: vec![
                NeutralBeamGlyphGroup::BeamSpot,
                NeutralBeamGlyphGroup::Other,
            ],
        });

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.systems[0].free_glyphs.len(), 1);
    }
}
