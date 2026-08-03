// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `BeamsStep`.
//!
//! Image closing, beam interpretation, and multiple-rest geometry remain typed
//! visual seams. This module owns their sheet/system ordering, mutations,
//! checked-error continuation, and unconditional BEAM_SPOT cleanup.

use std::{error::Error, fmt};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralBeamSystem {
    pub id: usize,
    pub left: i32,
    pub right: i32,
    pub free_glyphs: Vec<NeutralBeamGlyph>,
    pub inters: Vec<NeutralBeamInter>,
    pub relations: Vec<NeutralBeamRelation>,
    pub beam_group_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralBeamSheet {
    pub id: usize,
    pub systems: Vec<NeutralBeamSystem>,
    /// Sheet-global GlyphIndex ownership, in first-registration order.
    pub registered_glyph_ids: Vec<usize>,
    pub mutations: Vec<BeamsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedBeamSpot {
    pub glyph_id: usize,
    pub center_x: i32,
    /// Java `SystemManager.getSystemsOf(center)` order.
    pub relevant_system_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeamSpotBuild<VisualError> {
    /// Output already produced before `warning`, if any.
    pub spots: Vec<DetectedBeamSpot>,
    /// Java `SpotsBuilder.buildSheetSpots` catches and logs every exception.
    pub warning: Option<VisualError>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralBeamSystem,
}

/// First unavailable dependencies in Java: `SpotsBuilder`, `BeamsBuilder`,
/// and `MultipleRestsBuilder` image/geometry work.
pub trait VisualBeams {
    type Error;

    fn build_sheet_spots(&mut self, sheet: &NeutralBeamSheet) -> BeamSpotBuild<Self::Error>;

    fn build_beams(&mut self, input: BeamSystemInput<'_>) -> BeamStageOutcome<Self::Error>;

    fn build_multiple_rests(&mut self, input: BeamSystemInput<'_>)
    -> BeamStageOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeamsReport<VisualError> {
    pub spot_warning: Option<VisualError>,
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
        }
    }
}

impl Error for BeamsContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamsMutation {
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
        let spot_build = self.visual.build_sheet_spots(sheet);
        self.dispatch_spots(sheet, spot_build.spots)?;

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
            spot_warning: spot_build.warning,
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
            for system_id in spot.relevant_system_ids {
                let Some(system_index) = sheet
                    .systems
                    .iter()
                    .position(|system| system.id == system_id)
                else {
                    return Err(BeamsContractError::UnknownSystem(system_id));
                };
                let system = &sheet.systems[system_index];
                if spot.center_x < system.left || spot.center_x > system.right {
                    continue;
                }
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
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeVisual {
        spots: Option<BeamSpotBuild<&'static str>>,
        beams: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        rests: BTreeMap<usize, BeamStageOutcome<&'static str>>,
        calls: Vec<(&'static str, usize)>,
    }

    impl VisualBeams for FakeVisual {
        type Error = &'static str;

        fn build_sheet_spots(&mut self, sheet: &NeutralBeamSheet) -> BeamSpotBuild<Self::Error> {
            self.calls.push(("spots", sheet.id));
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

    fn system(id: usize, left: i32, right: i32) -> NeutralBeamSystem {
        NeutralBeamSystem {
            id,
            left,
            right,
            free_glyphs: Vec::new(),
            inters: Vec::new(),
            relations: Vec::new(),
            beam_group_ids: Vec::new(),
        }
    }

    fn sheet() -> NeutralBeamSheet {
        NeutralBeamSheet {
            id: 9,
            systems: vec![system(2, 10, 50), system(1, 40, 90)],
            registered_glyph_ids: Vec::new(),
            mutations: Vec::new(),
        }
    }

    #[test]
    fn runs_prolog_system_stages_and_epilog_in_java_order() {
        let visual = FakeVisual {
            spots: Some(BeamSpotBuild {
                spots: vec![DetectedBeamSpot {
                    glyph_id: 7,
                    center_x: 45,
                    relevant_system_ids: vec![2, 1],
                }],
                warning: None,
            }),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, Vec::new());
        assert_eq!(
            step.visual().calls,
            vec![
                ("spots", 9),
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
            sheet.mutations,
            vec![
                BeamsMutation::SpotRegistered {
                    system_id: 2,
                    glyph_id: 7
                },
                BeamsMutation::SpotAttached {
                    system_id: 2,
                    glyph_id: 7
                },
                BeamsMutation::SpotRegistered {
                    system_id: 1,
                    glyph_id: 7
                },
                BeamsMutation::SpotAttached {
                    system_id: 1,
                    glyph_id: 7
                },
                BeamsMutation::BeamSpotRemoved {
                    system_id: 2,
                    glyph_id: 7
                },
                BeamsMutation::BeamSpotRemoved {
                    system_id: 1,
                    glyph_id: 7
                },
            ]
        );
    }

    #[test]
    fn spot_failure_is_warning_and_partial_spots_still_feed_every_system() {
        let visual = FakeVisual {
            spots: Some(BeamSpotBuild {
                spots: vec![DetectedBeamSpot {
                    glyph_id: 8,
                    center_x: 10,
                    relevant_system_ids: vec![2],
                }],
                warning: Some("closing failed late"),
            }),
            ..FakeVisual::default()
        };
        let mut step = HeadlessBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.spot_warning, Some("closing failed late"));
        assert_eq!(step.visual().calls.len(), 5);
        assert_eq!(sheet.registered_glyph_ids, vec![8]);
    }

    #[test]
    fn checked_beam_failure_keeps_prefix_skips_rests_continues_and_cleans_up() {
        let mut visual = FakeVisual {
            spots: Some(BeamSpotBuild {
                spots: Vec::new(),
                warning: None,
            }),
            ..FakeVisual::default()
        };
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
            vec![("spots", 9), ("beams", 2), ("beams", 1), ("rests", 1)]
        );
        assert_eq!(sheet.systems[0].inters[0].id, 20);
        assert_eq!(sheet.systems[1].inters[0].id, 30);
        assert_eq!(
            sheet.mutations,
            vec![
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
        let mut visual = FakeVisual {
            spots: Some(BeamSpotBuild {
                spots: Vec::new(),
                warning: None,
            }),
            ..FakeVisual::default()
        };
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
        let visual = FakeVisual {
            spots: Some(BeamSpotBuild {
                spots: Vec::new(),
                warning: None,
            }),
            ..FakeVisual::default()
        };
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
