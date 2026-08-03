// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `StemSeedsStep` and the neutral
//! orchestration boundary around `VerticalsBuilder`.

use std::error::Error;
use std::fmt;

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
pub struct NeutralStemCandidate {
    pub glyph_id: usize,
    pub center_x: f64,
    /// Result of Java `getClosestStaff`, or `None` when outside every staff.
    pub closest_staff_id: Option<usize>,
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
    pub staves: Vec<NeutralStemStaff>,
    pub sections: Vec<NeutralStemSection>,
    /// Java system free-glyph collection, in accepted candidate order.
    pub free_glyphs: Vec<NeutralStemSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralStemSheet {
    pub id: usize,
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
pub struct StemSeedRetrievalInput<'a> {
    pub sheet_id: usize,
    pub system_id: usize,
    pub profile: i32,
    pub stem_scale: NeutralStemScale,
    /// Strictly in-bound vertical sections, source order preserved.
    pub vertical_sections: &'a [NeutralStemSection],
    /// Strictly in-bound one-pixel horizontal sections, source order preserved.
    pub horizontal_sections: &'a [NeutralStemSection],
}

/// First visual boundary: global stem-width measurement and StickFactory /
/// StemChecker candidate production. Neither output is fabricated by Rust.
pub trait VisualStemSeeds {
    type Error;

    fn retrieve_stem_scale(
        &mut self,
        sheet: &NeutralStemSheet,
    ) -> Result<NeutralStemScale, Self::Error>;

    fn retrieve_candidates(
        &mut self,
        input: StemSeedRetrievalInput<'_>,
    ) -> Result<Vec<NeutralStemCandidate>, Self::Error>;
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
}

impl<Visual> HeadlessStemSeedsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual, minimum_seed_grade: f64) -> Self {
        Self {
            visual,
            minimum_seed_grade,
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
            let measured = self
                .visual
                .retrieve_stem_scale(sheet)
                .map_err(StemSeedsStepError::StemScale)?;
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
            let input = StemSeedRetrievalInput {
                sheet_id: sheet.id,
                system_id: system.id,
                profile: system.profile,
                stem_scale,
                vertical_sections: &vertical_sections,
                horizontal_sections: &horizontal_sections,
            };
            let candidates = match self.visual.retrieve_candidates(input) {
                Ok(candidates) => candidates,
                Err(source) => {
                    let system_id = sheet.systems[system_index].id;
                    sheet
                        .mutations
                        .push(StemSeedsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                    continue;
                }
            };
            // Java registers every checked candidate glyph before the grade
            // threshold, but only accepted seeds become system free glyphs.
            for candidate in candidates {
                let system_id = sheet.systems[system_index].id;
                sheet.registered_glyph_ids.push(candidate.glyph_id);
                sheet.mutations.push(StemSeedsMutation::GlyphRegistered {
                    system_id,
                    glyph_id: candidate.glyph_id,
                });
                let system = &sheet.systems[system_index];
                let Some(staff_id) = candidate.closest_staff_id else {
                    continue;
                };
                let Some(staff) = system.staves.iter().find(|staff| staff.id == staff_id) else {
                    continue;
                };
                if staff.tablature
                    || candidate.center_x < f64::from(staff.header_stop)
                    || candidate.grade < self.minimum_seed_grade
                {
                    continue;
                }
                let system_id = system.id;
                sheet.systems[system_index]
                    .free_glyphs
                    .push(NeutralStemSeed {
                        glyph_id: candidate.glyph_id,
                        vertical_seed_group: true,
                    });
                sheet.mutations.push(StemSeedsMutation::SeedAdded {
                    system_id,
                    glyph_id: candidate.glyph_id,
                });
            }
        }
        Ok(StemSeedsReport { system_errors })
    }
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

    #[derive(Default)]
    struct FakeVisual {
        scale: Option<Result<NeutralStemScale, &'static str>>,
        by_system: BTreeMap<usize, Result<Vec<NeutralStemCandidate>, &'static str>>,
        scale_calls: usize,
        section_calls: Vec<(usize, Vec<usize>, Vec<usize>)>,
    }

    impl VisualStemSeeds for FakeVisual {
        type Error = &'static str;

        fn retrieve_stem_scale(
            &mut self,
            _sheet: &NeutralStemSheet,
        ) -> Result<NeutralStemScale, Self::Error> {
            self.scale_calls += 1;
            self.scale.take().unwrap()
        }

        fn retrieve_candidates(
            &mut self,
            input: StemSeedRetrievalInput<'_>,
        ) -> Result<Vec<NeutralStemCandidate>, Self::Error> {
            self.section_calls.push((
                input.system_id,
                input
                    .vertical_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
                input
                    .horizontal_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
            ));
            self.by_system
                .remove(&input.system_id)
                .unwrap_or(Ok(Vec::new()))
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
            stem_scale,
            systems: vec![system(2), system(1)],
            registered_glyph_ids: Vec::new(),
            mutations: Vec::new(),
        }
    }

    fn candidate(
        glyph_id: usize,
        center_x: f64,
        staff: Option<usize>,
        grade: f64,
    ) -> NeutralStemCandidate {
        NeutralStemCandidate {
            glyph_id,
            center_x,
            closest_staff_id: staff,
            grade,
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
            vec![(2, vec![1], vec![3]), (1, vec![1], vec![3])]
        );
        assert!(sheet.mutations.is_empty());
    }

    #[test]
    fn measured_scale_is_committed_before_first_system_and_survives_failure() {
        let measured = NeutralStemScale {
            main: 3,
            maximum: 5,
        };
        let mut visual = FakeVisual {
            scale: Some(Ok(measured)),
            ..FakeVisual::default()
        };
        visual.by_system.insert(2, Err("stick factory failed"));
        visual
            .by_system
            .insert(1, Ok(vec![candidate(10, 40.0, Some(1), 0.9)]));
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
            scale: Some(Err("no histogram")),
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
                candidate(1, 40.0, None, 1.0),
                candidate(2, 40.0, Some(2), 1.0),
                candidate(3, 29.9, Some(1), 1.0),
                candidate(4, 30.0, Some(1), 0.49),
                candidate(5, 30.0, Some(1), 0.5),
            ]),
        );
        let mut step = HeadlessStemSeedsStep::new(visual, 0.5);
        let mut sheet = NeutralStemSheet {
            id: 5,
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
            vec![5]
        );
        assert!(sheet.systems[0].free_glyphs[0].vertical_seed_group);
        assert_eq!(sheet.registered_glyph_ids, vec![1, 2, 3, 4, 5]);
    }
}
