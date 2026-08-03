// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `StemSeedsStep` and the neutral
//! orchestration boundary around `VerticalsBuilder`.

use std::error::Error;
use std::fmt;

use audiveris_core::{
    integer_function::IntegerFunction,
    peak_finder::{HiLoPeakFinder, Quorum},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralStemScale {
    pub main: i32,
    pub maximum: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralSectionOrientation {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralStemSection {
    pub id: usize,
    pub orientation: NeutralSectionOrientation,
    pub center_x: i32,
    /// Java `Section.getLength(HORIZONTAL)`; relevant to horizontal sections.
    pub horizontal_length: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralStemStaff {
    pub id: usize,
    pub tablature: bool,
    pub header_stop: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStraightFilament {
    pub filament_id: usize,
    pub center_x: f64,
    /// Result of Java `getClosestStaff`, or `None` when outside every staff.
    pub closest_staff_id: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralCheckedStem {
    pub glyph_id: usize,
    /// Java `StemChecker.checkStem(...).getGrade()`.
    pub grade: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStemSeed {
    pub glyph_id: usize,
    pub vertical_seed_group: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStemSystem {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub profile: i32,
    /// Resolved Java profile-dependent `minCoreSectionLength`, in pixels.
    pub minimum_core_section_length: i32,
    pub staves: Vec<NeutralStemStaff>,
    pub sections: Vec<NeutralStemSection>,
    /// Java system free-glyph collection, in accepted candidate order.
    pub free_glyphs: Vec<NeutralStemSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStemSheet {
    pub id: usize,
    pub interline: i32,
    pub foreground_main: i32,
    pub foreground_maximum: i32,
    pub stem_scale: Option<NeutralStemScale>,
    pub systems: Vec<NeutralStemSystem>,
    /// Java sheet glyph-index registration order for checked candidates.
    pub registered_glyph_ids: Vec<usize>,
    pub mutations: Vec<StemSeedsMutation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemSeedsMutation {
    StemScaleSet(NeutralStemScale),
    GlyphRegistered { system_id: usize, glyph_id: usize },
    SeedAdded { system_id: usize, glyph_id: usize },
    SystemFailed { system_id: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalFilamentInput<'a> {
    pub sheet_id: usize,
    pub system_id: usize,
    pub profile: i32,
    pub stem_scale: NeutralStemScale,
    pub minimum_core_section_length: i32,
    pub minimum_side_ratio: f64,
    /// Strictly in-bound vertical sections, source order preserved.
    pub vertical_sections: &'a [NeutralStemSection],
    /// Strictly in-bound one-pixel horizontal sections, source order preserved.
    pub horizontal_sections: &'a [NeutralStemSection],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemCheckInput<'a> {
    pub sheet_id: usize,
    pub system_id: usize,
    pub filament: &'a NeutralStraightFilament,
}

/// Visual boundaries that begin at the first not-yet-ported geometric
/// collaborator: Java's vertical `StickFactory`, followed by `StemChecker`.
pub trait VisualStemSeeds {
    type Error;

    /// Prepare the staff-core, staff-line/barline-cleaned raster and return
    /// every horizontal foreground run length in raster traversal order.
    fn retrieve_horizontal_run_lengths(
        &mut self,
        sheet: &NeutralStemSheet,
    ) -> Result<Vec<i32>, Self::Error>;

    fn build_vertical_filaments(
        &mut self,
        input: VerticalFilamentInput<'_>,
    ) -> Result<Vec<NeutralStraightFilament>, Self::Error>;

    fn materialize_and_check_stem(
        &mut self,
        input: StemCheckInput<'_>,
    ) -> Result<NeutralCheckedStem, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum StemSeedsStepError<VisualError> {
    StemScale(VisualError),
}

impl<VisualError: fmt::Display> fmt::Display for StemSeedsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StemScale(source) => write!(formatter, "stem scale retrieval failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for StemSeedsStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StemScale(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StemSeedsReport<VisualError> {
    /// Java catches per-system `StepException` and proceeds with later systems.
    pub system_errors: Vec<(usize, VisualError)>,
}

pub struct HeadlessStemSeedsStep<Visual> {
    visual: Visual,
    minimum_seed_grade: f64,
    minimum_side_ratio: f64,
}

impl<Visual> HeadlessStemSeedsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual, minimum_seed_grade: f64) -> Self {
        Self {
            visual,
            minimum_seed_grade,
            minimum_side_ratio: 0.4,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual> HeadlessStemSeedsStep<Visual>
where
    Visual: VisualStemSeeds,
{
    /// Java `AbstractSystemStep.doit`: prolog once, systems in sheet source
    /// order, and no epilog mutation for this step.
    pub fn process(
        &mut self,
        sheet: &mut NeutralStemSheet,
    ) -> Result<StemSeedsReport<Visual::Error>, StemSeedsStepError<Visual::Error>> {
        let stem_scale = if let Some(user_scale) = sheet.stem_scale {
            user_scale
        } else {
            let runs = self
                .visual
                .retrieve_horizontal_run_lengths(sheet)
                .map_err(StemSeedsStepError::StemScale)?;
            let measured = compute_stem_scale(
                &runs,
                StemScaleComputation {
                    interline: sheet.interline,
                    foreground_main: sheet.foreground_main,
                    foreground_maximum: sheet.foreground_maximum,
                    minimum_value_ratio: 0.1,
                    minimum_derivative_ratio: 0.05,
                    minimum_gain_ratio: 0.1,
                    stem_as_foreground_ratio: 1.0,
                },
            );
            // Java sets the scale before any system task begins.
            sheet.stem_scale = Some(measured);
            sheet
                .mutations
                .push(StemSeedsMutation::StemScaleSet(measured));
            measured
        };

        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system = &sheet.systems[system_index];
            let vertical_sections = filter_sections(system, NeutralSectionOrientation::Vertical);
            let horizontal_sections =
                filter_sections(system, NeutralSectionOrientation::Horizontal);
            let input = VerticalFilamentInput {
                sheet_id: sheet.id,
                system_id: system.id,
                profile: system.profile,
                stem_scale,
                minimum_core_section_length: system.minimum_core_section_length,
                minimum_side_ratio: self.minimum_side_ratio,
                vertical_sections: &vertical_sections,
                horizontal_sections: &horizontal_sections,
            };
            let filaments = match self.visual.build_vertical_filaments(input) {
                Ok(filaments) => filaments,
                Err(source) => {
                    let system_id = sheet.systems[system_index].id;
                    sheet
                        .mutations
                        .push(StemSeedsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                    continue;
                }
            };
            // Java rejects candidates outside a usable staff/header before
            // converting the stick to a glyph. Registration then precedes the
            // grade threshold, and only accepted seeds become free glyphs.
            for filament in filaments {
                let system_id = sheet.systems[system_index].id;
                let system = &sheet.systems[system_index];
                let Some(staff_id) = filament.closest_staff_id else {
                    continue;
                };
                let Some(staff) = system.staves.iter().find(|staff| staff.id == staff_id) else {
                    continue;
                };
                if staff.tablature || filament.center_x < f64::from(staff.header_stop) {
                    continue;
                }
                let checked = match self.visual.materialize_and_check_stem(StemCheckInput {
                    sheet_id: sheet.id,
                    system_id,
                    filament: &filament,
                }) {
                    Ok(checked) => checked,
                    Err(source) => {
                        sheet
                            .mutations
                            .push(StemSeedsMutation::SystemFailed { system_id });
                        system_errors.push((system_id, source));
                        break;
                    }
                };
                sheet.registered_glyph_ids.push(checked.glyph_id);
                sheet.mutations.push(StemSeedsMutation::GlyphRegistered {
                    system_id,
                    glyph_id: checked.glyph_id,
                });
                if checked.grade < self.minimum_seed_grade {
                    continue;
                }
                let system_id = system.id;
                sheet.systems[system_index]
                    .free_glyphs
                    .push(NeutralStemSeed {
                        glyph_id: checked.glyph_id,
                        vertical_seed_group: true,
                    });
                sheet.mutations.push(StemSeedsMutation::SeedAdded {
                    system_id,
                    glyph_id: checked.glyph_id,
                });
            }
        }
        Ok(StemSeedsReport { system_errors })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemScaleComputation {
    pub interline: i32,
    pub foreground_main: i32,
    pub foreground_maximum: i32,
    pub minimum_value_ratio: f64,
    pub minimum_derivative_ratio: f64,
    pub minimum_gain_ratio: f64,
    pub stem_as_foreground_ratio: f64,
}

/// Java `HistoKeeper.populateFunction` + `StemScaler.computeStem` over concrete
/// horizontal run evidence.
#[must_use]
pub fn compute_stem_scale(
    horizontal_run_lengths: &[i32],
    parameters: StemScaleComputation,
) -> NeutralStemScale {
    let mut histogram = IntegerFunction::new(0, parameters.interline);
    for length in horizontal_run_lengths {
        if *length <= parameters.interline {
            histogram.add_value(*length, 1);
        }
    }
    let area = histogram.area();
    let quorum = java_rint(f64::from(area) * parameters.minimum_value_ratio);
    let derivative = java_rint(f64::from(area) * parameters.minimum_derivative_ratio);
    let mut finder = HiLoPeakFinder::new("stem", &histogram);
    finder.set_quorum(Quorum::new(quorum));
    let peak = finder
        .find_peaks(1, derivative, parameters.minimum_gain_ratio)
        .first()
        .copied();
    peak.map_or_else(
        || NeutralStemScale {
            main: java_rint(
                parameters.stem_as_foreground_ratio * f64::from(parameters.foreground_main),
            ),
            maximum: java_rint(
                parameters.stem_as_foreground_ratio * f64::from(parameters.foreground_maximum),
            ),
        },
        |peak| NeutralStemScale {
            main: java_rint(f64::from(peak.main)),
            maximum: java_rint(f64::from(peak.max)),
        },
    )
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn filter_sections(
    system: &NeutralStemSystem,
    orientation: NeutralSectionOrientation,
) -> Vec<NeutralStemSection> {
    system
        .sections
        .iter()
        .copied()
        .filter(|section| {
            section.orientation == orientation
                && section.center_x > system.left
                && section.center_x < system.right
                && (orientation != NeutralSectionOrientation::Horizontal
                    || section.horizontal_length == 1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Debug, PartialEq)]
    struct FactoryCall {
        system_id: usize,
        minimum_core_section_length: i32,
        minimum_side_ratio: f64,
        vertical_section_ids: Vec<usize>,
        horizontal_section_ids: Vec<usize>,
    }

    #[derive(Default)]
    struct FakeVisual {
        runs: Option<Result<Vec<i32>, &'static str>>,
        by_system: BTreeMap<usize, Result<Vec<NeutralStraightFilament>, &'static str>>,
        checked: BTreeMap<usize, Result<NeutralCheckedStem, &'static str>>,
        scale_calls: usize,
        section_calls: Vec<FactoryCall>,
        check_calls: Vec<usize>,
    }

    impl VisualStemSeeds for FakeVisual {
        type Error = &'static str;

        fn retrieve_horizontal_run_lengths(
            &mut self,
            _sheet: &NeutralStemSheet,
        ) -> Result<Vec<i32>, Self::Error> {
            self.scale_calls += 1;
            self.runs.take().unwrap()
        }

        fn build_vertical_filaments(
            &mut self,
            input: VerticalFilamentInput<'_>,
        ) -> Result<Vec<NeutralStraightFilament>, Self::Error> {
            self.section_calls.push(FactoryCall {
                system_id: input.system_id,
                minimum_core_section_length: input.minimum_core_section_length,
                minimum_side_ratio: input.minimum_side_ratio,
                vertical_section_ids: input
                    .vertical_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
                horizontal_section_ids: input
                    .horizontal_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
            });
            self.by_system
                .remove(&input.system_id)
                .unwrap_or(Ok(Vec::new()))
        }

        fn materialize_and_check_stem(
            &mut self,
            input: StemCheckInput<'_>,
        ) -> Result<NeutralCheckedStem, Self::Error> {
            self.check_calls.push(input.filament.filament_id);
            self.checked
                .remove(&input.filament.filament_id)
                .unwrap_or(Ok(NeutralCheckedStem {
                    glyph_id: input.filament.filament_id,
                    grade: 1.0,
                }))
        }
    }

    fn section(
        id: usize,
        orientation: NeutralSectionOrientation,
        center_x: i32,
        horizontal_length: i32,
    ) -> NeutralStemSection {
        NeutralStemSection {
            id,
            orientation,
            center_x,
            horizontal_length,
        }
    }

    fn system(id: usize) -> NeutralStemSystem {
        NeutralStemSystem {
            id,
            left: 10,
            right: 90,
            profile: id as i32,
            minimum_core_section_length: 15 + id as i32,
            staves: vec![
                NeutralStemStaff {
                    id: 1,
                    tablature: false,
                    header_stop: 30,
                },
                NeutralStemStaff {
                    id: 2,
                    tablature: true,
                    header_stop: 20,
                },
            ],
            sections: vec![
                section(1, NeutralSectionOrientation::Vertical, 11, 4),
                section(2, NeutralSectionOrientation::Vertical, 90, 4),
                section(3, NeutralSectionOrientation::Horizontal, 50, 1),
                section(4, NeutralSectionOrientation::Horizontal, 60, 2),
                section(5, NeutralSectionOrientation::Horizontal, 10, 1),
            ],
            free_glyphs: Vec::new(),
        }
    }

    fn sheet(stem_scale: Option<NeutralStemScale>) -> NeutralStemSheet {
        NeutralStemSheet {
            id: 5,
            interline: 10,
            foreground_main: 3,
            foreground_maximum: 5,
            stem_scale,
            systems: vec![system(2), system(1)],
            registered_glyph_ids: Vec::new(),
            mutations: Vec::new(),
        }
    }

    fn filament(
        filament_id: usize,
        center_x: f64,
        staff: Option<usize>,
    ) -> NeutralStraightFilament {
        NeutralStraightFilament {
            filament_id,
            center_x,
            closest_staff_id: staff,
        }
    }

    #[test]
    fn honors_user_scale_filters_sections_strictly_and_preserves_system_order() {
        let user = NeutralStemScale {
            main: 2,
            maximum: 4,
        };
        let mut visual = FakeVisual::default();
        visual.by_system.insert(2, Ok(Vec::new()));
        visual.by_system.insert(1, Ok(Vec::new()));
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = sheet(Some(user));

        assert!(step.process(&mut sheet).unwrap().system_errors.is_empty());
        assert_eq!(step.visual().scale_calls, 0);
        assert_eq!(
            step.visual().section_calls,
            vec![
                FactoryCall {
                    system_id: 2,
                    minimum_core_section_length: 17,
                    minimum_side_ratio: 0.4,
                    vertical_section_ids: vec![1],
                    horizontal_section_ids: vec![3],
                },
                FactoryCall {
                    system_id: 1,
                    minimum_core_section_length: 16,
                    minimum_side_ratio: 0.4,
                    vertical_section_ids: vec![1],
                    horizontal_section_ids: vec![3],
                }
            ]
        );
        assert!(sheet.mutations.is_empty());
    }

    #[test]
    fn measured_scale_is_committed_before_first_system_and_survives_failure() {
        let measured = NeutralStemScale {
            main: 2,
            maximum: 3,
        };
        let mut visual = FakeVisual {
            runs: Some(Ok(std::iter::repeat_n(2, 20)
                .chain(std::iter::repeat_n(3, 10))
                .chain([1, 1, 4])
                .collect())),
            ..FakeVisual::default()
        };
        visual.by_system.insert(2, Err("stick factory failed"));
        visual
            .by_system
            .insert(1, Ok(vec![filament(10, 40.0, Some(1))]));
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = sheet(None);

        let report = step.process(&mut sheet).unwrap();
        assert_eq!(report.system_errors, vec![(2, "stick factory failed")]);
        assert_eq!(sheet.stem_scale, Some(measured));
        assert_eq!(sheet.systems[1].free_glyphs[0].glyph_id, 10);
        assert_eq!(
            sheet.mutations,
            vec![
                StemSeedsMutation::StemScaleSet(measured),
                StemSeedsMutation::SystemFailed { system_id: 2 },
                StemSeedsMutation::GlyphRegistered {
                    system_id: 1,
                    glyph_id: 10
                },
                StemSeedsMutation::SeedAdded {
                    system_id: 1,
                    glyph_id: 10
                }
            ]
        );
    }

    #[test]
    fn scale_failure_aborts_before_any_system_mutation() {
        let visual = FakeVisual {
            runs: Some(Err("no histogram")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = sheet(None);
        assert_eq!(
            step.process(&mut sheet),
            Err(StemSeedsStepError::StemScale("no histogram"))
        );
        assert!(sheet.stem_scale.is_none());
        assert!(sheet.mutations.is_empty());
        assert!(step.visual().section_calls.is_empty());
    }

    #[test]
    fn candidate_checks_follow_java_staff_header_grade_order() {
        let user = NeutralStemScale {
            main: 2,
            maximum: 4,
        };
        let mut visual = FakeVisual::default();
        visual.by_system.insert(
            2,
            Ok(vec![
                filament(1, 40.0, None),
                filament(2, 40.0, Some(2)),
                filament(3, 29.9, Some(1)),
                filament(4, 30.0, Some(1)),
                filament(5, 30.0, Some(1)),
            ]),
        );
        visual.checked.insert(
            4,
            Ok(NeutralCheckedStem {
                glyph_id: 40,
                grade: 0.49,
            }),
        );
        visual.checked.insert(
            5,
            Ok(NeutralCheckedStem {
                glyph_id: 50,
                grade: 0.5,
            }),
        );
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = NeutralStemSheet {
            id: 5,
            interline: 10,
            foreground_main: 3,
            foreground_maximum: 5,
            stem_scale: Some(user),
            systems: vec![system(2)],
            registered_glyph_ids: Vec::new(),
            mutations: Vec::new(),
        };
        step.process(&mut sheet).unwrap();
        assert_eq!(
            sheet.systems[0]
                .free_glyphs
                .iter()
                .map(|seed| seed.glyph_id)
                .collect::<Vec<_>>(),
            vec![50]
        );
        assert!(sheet.systems[0].free_glyphs[0].vertical_seed_group);
        assert_eq!(sheet.registered_glyph_ids, vec![40, 50]);
        assert_eq!(step.visual().check_calls, vec![4, 5]);
    }

    #[test]
    fn check_failure_keeps_prefix_mutations_stops_system_and_continues_later_system() {
        let user = NeutralStemScale {
            main: 2,
            maximum: 4,
        };
        let mut visual = FakeVisual::default();
        visual.by_system.insert(
            2,
            Ok(vec![
                filament(10, 40.0, Some(1)),
                filament(11, 40.0, Some(1)),
                filament(12, 40.0, Some(1)),
            ]),
        );
        visual
            .by_system
            .insert(1, Ok(vec![filament(20, 40.0, Some(1))]));
        visual.checked.insert(11, Err("stem checker failed"));
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = sheet(Some(user));

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "stem checker failed")]);
        assert_eq!(step.visual().check_calls, vec![10, 11, 20]);
        assert_eq!(sheet.registered_glyph_ids, vec![10, 20]);
        assert_eq!(
            sheet.mutations,
            vec![
                StemSeedsMutation::GlyphRegistered {
                    system_id: 2,
                    glyph_id: 10,
                },
                StemSeedsMutation::SeedAdded {
                    system_id: 2,
                    glyph_id: 10,
                },
                StemSeedsMutation::SystemFailed { system_id: 2 },
                StemSeedsMutation::GlyphRegistered {
                    system_id: 1,
                    glyph_id: 20,
                },
                StemSeedsMutation::SeedAdded {
                    system_id: 1,
                    glyph_id: 20,
                },
            ]
        );
    }

    fn scale_parameters() -> StemScaleComputation {
        StemScaleComputation {
            interline: 10,
            foreground_main: 3,
            foreground_maximum: 5,
            minimum_value_ratio: 0.1,
            minimum_derivative_ratio: 0.05,
            minimum_gain_ratio: 0.1,
            stem_as_foreground_ratio: 1.0,
        }
    }

    #[test]
    fn concrete_histogram_selects_first_peak_and_ignores_runs_over_interline() {
        let runs = std::iter::repeat_n(2, 20)
            .chain(std::iter::repeat_n(3, 10))
            .chain([1, 1, 4, 11, 99])
            .collect::<Vec<_>>();
        assert_eq!(
            compute_stem_scale(&runs, scale_parameters()),
            NeutralStemScale {
                main: 2,
                maximum: 3
            }
        );
    }

    #[test]
    fn empty_histogram_uses_foreground_fallback_with_java_ties_to_even() {
        let mut parameters = scale_parameters();
        parameters.stem_as_foreground_ratio = 0.5;
        assert_eq!(
            compute_stem_scale(&[], parameters),
            NeutralStemScale {
                main: 2,
                maximum: 2
            }
        );
        assert_eq!(java_rint(2.5), 2);
        assert_eq!(java_rint(3.5), 4);
    }
}
