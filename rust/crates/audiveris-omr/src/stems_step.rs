// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `StemsStep`.
//!
//! Legacy beam-group upgrade is a neutral deterministic precondition. The
//! first injected geometry is `StemsRetriever.process`; after every system has
//! been attempted, Java revisits systems for `finalizeBeams` and contextualizes
//! each successful SIG immediately.

use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralStemsGlyph {
    pub id: usize,
    pub section_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralStemsInterKind {
    Beam {
        group_id: Option<usize>,
        /// Pre-resolved compatibility class used only for old `.omr` upgrade.
        legacy_group_key: usize,
        ordinate_order: i32,
    },
    BeamGroup,
    Head,
    Stem,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralStemsInter {
    pub id: usize,
    pub kind: NeutralStemsInterKind,
    pub glyph_id: Option<usize>,
    pub abnormal: bool,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralStemsRelationKind {
    BeamGroupMember,
    HeadStem,
    BeamStem,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralStemsRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralStemsRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralStemsSystem {
    pub id: usize,
    pub inters: Vec<NeutralStemsInter>,
    pub relations: Vec<NeutralStemsRelation>,
    pub contextualized: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralStemsSheet {
    pub id: usize,
    pub next_inter_id: usize,
    pub next_relation_id: usize,
    pub registered_glyphs: Vec<NeutralStemsGlyph>,
    pub systems: Vec<NeutralStemsSystem>,
    pub mutations: Vec<StemsMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StemsDelta {
    /// Exact Java-visible prefix retained before a checked failure.
    pub mutations: Vec<StemsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemsDeltaMutation {
    RegisterGlyph(NeutralStemsGlyph),
    AddInter(NeutralStemsInter),
    RemoveInter(usize),
    AddRelation(NeutralStemsRelation),
    RemoveRelation(usize),
    SetAbnormal { inter_id: usize, abnormal: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemsStageOutcome<VisualError> {
    pub delta: StemsDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> StemsStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: StemsDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StemsSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralStemsSystem,
}

/// First geometric collaborator and its later sheet-epilog revisit.
pub trait VisualStemsRetriever {
    type Error;

    fn process_stems(&mut self, input: StemsSystemInput<'_>) -> StemsStageOutcome<Self::Error>;

    fn finalize_beams(&mut self, input: StemsSystemInput<'_>) -> StemsStageOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemsReport<VisualError> {
    pub system_errors: Vec<(usize, VisualError)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemsStepError<VisualError> {
    Prolog(StemsContractError),
    Contract(StemsContractError),
    Epilog {
        source: VisualError,
        system_errors: Vec<(usize, VisualError)>,
    },
}

impl<VisualError: fmt::Display> fmt::Display for StemsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prolog(source) => write!(formatter, "stems prolog failed: {source}"),
            Self::Contract(source) => write!(formatter, "stems contract failed: {source}"),
            Self::Epilog { source, .. } => write!(formatter, "stems epilog failed: {source}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemsContractError {
    DuplicateSystem(usize),
    DuplicateGlyph(usize),
    DuplicateInter {
        system_id: usize,
        inter_id: usize,
    },
    UnknownInter {
        system_id: usize,
        inter_id: usize,
    },
    DuplicateRelation {
        system_id: usize,
        relation_id: usize,
    },
    UnknownRelation {
        system_id: usize,
        relation_id: usize,
    },
    UnknownRelationEndpoint {
        system_id: usize,
        inter_id: usize,
    },
    InterIdentityOverflow,
    RelationIdentityOverflow,
}

impl fmt::Display for StemsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSystem(id) => write!(formatter, "duplicate system {id}"),
            Self::DuplicateGlyph(id) => write!(formatter, "duplicate glyph {id}"),
            Self::DuplicateInter {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "duplicate inter {inter_id} in system {system_id}"
            ),
            Self::UnknownInter {
                system_id,
                inter_id,
            } => write!(formatter, "unknown inter {inter_id} in system {system_id}"),
            Self::DuplicateRelation {
                system_id,
                relation_id,
            } => write!(
                formatter,
                "duplicate relation {relation_id} in system {system_id}"
            ),
            Self::UnknownRelation {
                system_id,
                relation_id,
            } => write!(
                formatter,
                "unknown relation {relation_id} in system {system_id}"
            ),
            Self::UnknownRelationEndpoint {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "unknown relation endpoint {inter_id} in system {system_id}"
            ),
            Self::InterIdentityOverflow => formatter.write_str("inter identity overflow"),
            Self::RelationIdentityOverflow => formatter.write_str("relation identity overflow"),
        }
    }
}

impl Error for StemsContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemsMutation {
    LegacyBeamGroupAdded {
        system_id: usize,
        group_id: usize,
    },
    BeamAssigned {
        system_id: usize,
        beam_id: usize,
        group_id: usize,
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
    RelationRemoved {
        system_id: usize,
        relation_id: usize,
    },
    AbnormalSet {
        system_id: usize,
        inter_id: usize,
        abnormal: bool,
    },
    SystemFailed {
        system_id: usize,
    },
    BeamsFinalized {
        system_id: usize,
    },
    Contextualized {
        system_id: usize,
    },
}

pub struct HeadlessStemsStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessStemsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualStemsRetriever> HeadlessStemsStep<Visual> {
    /// Java: legacy group prolog, checked-error system traversal, then a fresh
    /// finalize retriever and immediate contextualization per system.
    pub fn process(
        &mut self,
        sheet: &mut NeutralStemsSheet,
    ) -> Result<StemsReport<Visual::Error>, StemsStepError<Visual::Error>> {
        validate_system_ids(sheet).map_err(StemsStepError::Prolog)?;
        upgrade_legacy_beam_groups(sheet).map_err(StemsStepError::Prolog)?;

        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.process_stems(StemsSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, outcome.delta).map_err(StemsStepError::Contract)?;
            if let Some(error) = outcome.error {
                sheet
                    .mutations
                    .push(StemsMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
            }
        }

        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.finalize_beams(StemsSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
            });
            apply_delta(sheet, system_index, outcome.delta).map_err(StemsStepError::Contract)?;
            if let Some(source) = outcome.error {
                return Err(StemsStepError::Epilog {
                    source,
                    system_errors,
                });
            }
            sheet
                .mutations
                .push(StemsMutation::BeamsFinalized { system_id });
            sheet.systems[system_index].contextualized = true;
            sheet
                .mutations
                .push(StemsMutation::Contextualized { system_id });
        }
        Ok(StemsReport { system_errors })
    }
}

fn validate_system_ids(sheet: &NeutralStemsSheet) -> Result<(), StemsContractError> {
    let mut ids = Vec::new();
    for system in &sheet.systems {
        if ids.contains(&system.id) {
            return Err(StemsContractError::DuplicateSystem(system.id));
        }
        ids.push(system.id);
    }
    Ok(())
}

fn upgrade_legacy_beam_groups(sheet: &mut NeutralStemsSheet) -> Result<(), StemsContractError> {
    for system_index in 0..sheet.systems.len() {
        if !sheet.systems[system_index].inters.iter().any(|inter| {
            !inter.removed
                && matches!(
                    inter.kind,
                    NeutralStemsInterKind::Beam { group_id: None, .. }
                )
        }) {
            continue;
        }
        let system_id = sheet.systems[system_index].id;
        let mut beams = sheet.systems[system_index]
            .inters
            .iter()
            .filter_map(|inter| match inter.kind {
                NeutralStemsInterKind::Beam {
                    group_id,
                    legacy_group_key,
                    ordinate_order,
                } if !inter.removed => Some((inter.id, group_id, legacy_group_key, ordinate_order)),
                _ => None,
            })
            .collect::<Vec<_>>();
        beams.sort_by_key(|(id, _, _, ordinate)| (*ordinate, *id));
        let mut groups = BTreeMap::new();
        for (_, group_id, key, _) in &beams {
            if let Some(group_id) = group_id {
                groups.entry(*key).or_insert(*group_id);
            }
        }
        for (beam_id, group_id, key, _) in beams {
            if group_id.is_some() {
                continue;
            }
            let group_id = if let Some(group_id) = groups.get(&key) {
                *group_id
            } else {
                let id = allocate_inter_id(sheet)?;
                sheet.systems[system_index].inters.push(NeutralStemsInter {
                    id,
                    kind: NeutralStemsInterKind::BeamGroup,
                    glyph_id: None,
                    abnormal: false,
                    removed: false,
                });
                sheet.mutations.push(StemsMutation::LegacyBeamGroupAdded {
                    system_id,
                    group_id: id,
                });
                groups.insert(key, id);
                id
            };
            let beam = sheet.systems[system_index]
                .inters
                .iter_mut()
                .find(|inter| inter.id == beam_id)
                .expect("beam snapshot came from system");
            let NeutralStemsInterKind::Beam {
                group_id: beam_group,
                ..
            } = &mut beam.kind
            else {
                unreachable!()
            };
            *beam_group = Some(group_id);
            sheet.mutations.push(StemsMutation::BeamAssigned {
                system_id,
                beam_id,
                group_id,
            });

            let relation_id = allocate_relation_id(sheet)?;
            sheet.systems[system_index]
                .relations
                .push(NeutralStemsRelation {
                    id: relation_id,
                    source_inter_id: group_id,
                    target_inter_id: beam_id,
                    kind: NeutralStemsRelationKind::BeamGroupMember,
                });
            sheet.mutations.push(StemsMutation::RelationAdded {
                system_id,
                relation_id,
            });
        }
    }
    Ok(())
}

fn allocate_inter_id(sheet: &mut NeutralStemsSheet) -> Result<usize, StemsContractError> {
    let id = sheet.next_inter_id;
    sheet.next_inter_id = id
        .checked_add(1)
        .ok_or(StemsContractError::InterIdentityOverflow)?;
    Ok(id)
}

fn allocate_relation_id(sheet: &mut NeutralStemsSheet) -> Result<usize, StemsContractError> {
    let id = sheet.next_relation_id;
    sheet.next_relation_id = id
        .checked_add(1)
        .ok_or(StemsContractError::RelationIdentityOverflow)?;
    Ok(id)
}

fn apply_delta(
    sheet: &mut NeutralStemsSheet,
    system_index: usize,
    delta: StemsDelta,
) -> Result<(), StemsContractError> {
    let system_id = sheet.systems[system_index].id;
    for mutation in delta.mutations {
        match mutation {
            StemsDeltaMutation::RegisterGlyph(glyph) => {
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|existing| existing.id == glyph.id)
                {
                    return Err(StemsContractError::DuplicateGlyph(glyph.id));
                }
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(StemsMutation::GlyphRegistered {
                    system_id,
                    glyph_id,
                });
            }
            StemsDeltaMutation::AddInter(inter) => {
                if sheet.systems[system_index]
                    .inters
                    .iter()
                    .any(|existing| existing.id == inter.id)
                {
                    return Err(StemsContractError::DuplicateInter {
                        system_id,
                        inter_id: inter.id,
                    });
                }
                let inter_id = inter.id;
                sheet.systems[system_index].inters.push(inter);
                sheet.mutations.push(StemsMutation::InterAdded {
                    system_id,
                    inter_id,
                });
            }
            StemsDeltaMutation::RemoveInter(inter_id) => {
                let inter = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == inter_id)
                    .ok_or(StemsContractError::UnknownInter {
                        system_id,
                        inter_id,
                    })?;
                inter.removed = true;
                sheet.mutations.push(StemsMutation::InterRemoved {
                    system_id,
                    inter_id,
                });
            }
            StemsDeltaMutation::AddRelation(relation) => {
                for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                    if !sheet.systems[system_index]
                        .inters
                        .iter()
                        .any(|inter| inter.id == inter_id && !inter.removed)
                    {
                        return Err(StemsContractError::UnknownRelationEndpoint {
                            system_id,
                            inter_id,
                        });
                    }
                }
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(StemsContractError::DuplicateRelation {
                        system_id,
                        relation_id: relation.id,
                    });
                }
                let relation_id = relation.id;
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(StemsMutation::RelationAdded {
                    system_id,
                    relation_id,
                });
            }
            StemsDeltaMutation::RemoveRelation(relation_id) => {
                let Some(index) = sheet.systems[system_index]
                    .relations
                    .iter()
                    .position(|relation| relation.id == relation_id)
                else {
                    return Err(StemsContractError::UnknownRelation {
                        system_id,
                        relation_id,
                    });
                };
                sheet.systems[system_index].relations.remove(index);
                sheet.mutations.push(StemsMutation::RelationRemoved {
                    system_id,
                    relation_id,
                });
            }
            StemsDeltaMutation::SetAbnormal { inter_id, abnormal } => {
                let inter = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == inter_id)
                    .ok_or(StemsContractError::UnknownInter {
                        system_id,
                        inter_id,
                    })?;
                inter.abnormal = abnormal;
                sheet.mutations.push(StemsMutation::AbnormalSet {
                    system_id,
                    inter_id,
                    abnormal,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeVisual {
        process: BTreeMap<usize, StemsStageOutcome<&'static str>>,
        finalize: BTreeMap<usize, StemsStageOutcome<&'static str>>,
        calls: Vec<(&'static str, usize)>,
        process_snapshots: Vec<(usize, Vec<usize>)>,
        finalize_snapshots: Vec<(usize, Vec<usize>)>,
    }

    impl VisualStemsRetriever for FakeVisual {
        type Error = &'static str;

        fn process_stems(&mut self, input: StemsSystemInput<'_>) -> StemsStageOutcome<Self::Error> {
            self.calls.push(("process", input.system.id));
            self.process_snapshots.push((
                input.system.id,
                input.system.inters.iter().map(|inter| inter.id).collect(),
            ));
            self.process
                .remove(&input.system.id)
                .unwrap_or_else(|| StemsStageOutcome::success(StemsDelta::default()))
        }

        fn finalize_beams(
            &mut self,
            input: StemsSystemInput<'_>,
        ) -> StemsStageOutcome<Self::Error> {
            self.calls.push(("finalize", input.system.id));
            self.finalize_snapshots.push((
                input.system.id,
                input.system.inters.iter().map(|inter| inter.id).collect(),
            ));
            self.finalize
                .remove(&input.system.id)
                .unwrap_or_else(|| StemsStageOutcome::success(StemsDelta::default()))
        }
    }

    fn beam(id: usize, group_id: Option<usize>, key: usize, ordinate: i32) -> NeutralStemsInter {
        NeutralStemsInter {
            id,
            kind: NeutralStemsInterKind::Beam {
                group_id,
                legacy_group_key: key,
                ordinate_order: ordinate,
            },
            glyph_id: None,
            abnormal: false,
            removed: false,
        }
    }

    fn stem(id: usize) -> NeutralStemsInter {
        NeutralStemsInter {
            id,
            kind: NeutralStemsInterKind::Stem,
            glyph_id: Some(id + 1_000),
            abnormal: false,
            removed: false,
        }
    }

    fn sheet() -> NeutralStemsSheet {
        NeutralStemsSheet {
            id: 9,
            next_inter_id: 100,
            next_relation_id: 200,
            registered_glyphs: Vec::new(),
            systems: vec![
                NeutralStemsSystem {
                    id: 2,
                    inters: vec![beam(20, None, 7, 2), beam(21, None, 7, 1)],
                    relations: Vec::new(),
                    contextualized: false,
                },
                NeutralStemsSystem {
                    id: 1,
                    inters: vec![
                        beam(10, Some(50), 5, 1),
                        NeutralStemsInter {
                            id: 50,
                            kind: NeutralStemsInterKind::BeamGroup,
                            glyph_id: None,
                            abnormal: false,
                            removed: false,
                        },
                    ],
                    relations: vec![NeutralStemsRelation {
                        id: 150,
                        source_inter_id: 50,
                        target_inter_id: 10,
                        kind: NeutralStemsRelationKind::BeamGroupMember,
                    }],
                    contextualized: false,
                },
            ],
            mutations: Vec::new(),
        }
    }

    #[test]
    fn runs_legacy_upgrade_systems_finalize_and_contextualize_in_java_order() {
        let mut visual = FakeVisual::default();
        visual.process.insert(
            2,
            StemsStageOutcome::success(StemsDelta {
                mutations: vec![
                    StemsDeltaMutation::RegisterGlyph(NeutralStemsGlyph {
                        id: 300,
                        section_ids: vec![3],
                    }),
                    StemsDeltaMutation::AddInter(stem(30)),
                    StemsDeltaMutation::AddRelation(NeutralStemsRelation {
                        id: 400,
                        source_inter_id: 20,
                        target_inter_id: 30,
                        kind: NeutralStemsRelationKind::BeamStem,
                    }),
                ],
            }),
        );
        visual.finalize.insert(
            2,
            StemsStageOutcome::success(StemsDelta {
                mutations: vec![StemsDeltaMutation::SetAbnormal {
                    inter_id: 30,
                    abnormal: true,
                }],
            }),
        );
        let mut step = HeadlessStemsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, Vec::new());
        assert_eq!(sheet.next_inter_id, 101);
        assert_eq!(sheet.next_relation_id, 202);
        assert_eq!(
            step.visual().calls,
            vec![
                ("process", 2),
                ("process", 1),
                ("finalize", 2),
                ("finalize", 1),
            ]
        );
        assert_eq!(step.visual().process_snapshots[0], (2, vec![20, 21, 100]));
        assert_eq!(
            step.visual().finalize_snapshots[0],
            (2, vec![20, 21, 100, 30])
        );
        assert!(sheet.systems.iter().all(|system| system.contextualized));
        assert!(
            sheet.systems[0]
                .inters
                .iter()
                .find(|inter| inter.id == 30)
                .unwrap()
                .abnormal
        );
        assert_eq!(
            &sheet.mutations[..6],
            &[
                StemsMutation::LegacyBeamGroupAdded {
                    system_id: 2,
                    group_id: 100,
                },
                StemsMutation::BeamAssigned {
                    system_id: 2,
                    beam_id: 21,
                    group_id: 100,
                },
                StemsMutation::RelationAdded {
                    system_id: 2,
                    relation_id: 200,
                },
                StemsMutation::BeamAssigned {
                    system_id: 2,
                    beam_id: 20,
                    group_id: 100,
                },
                StemsMutation::RelationAdded {
                    system_id: 2,
                    relation_id: 201,
                },
                StemsMutation::GlyphRegistered {
                    system_id: 2,
                    glyph_id: 300,
                },
            ]
        );
        assert_eq!(
            &sheet.mutations[sheet.mutations.len() - 4..],
            &[
                StemsMutation::BeamsFinalized { system_id: 2 },
                StemsMutation::Contextualized { system_id: 2 },
                StemsMutation::BeamsFinalized { system_id: 1 },
                StemsMutation::Contextualized { system_id: 1 },
            ]
        );
    }

    #[test]
    fn checked_system_failure_retains_delta_and_later_systems_and_epilog_run() {
        let mut visual = FakeVisual::default();
        visual.process.insert(
            2,
            StemsStageOutcome {
                delta: StemsDelta {
                    mutations: vec![StemsDeltaMutation::AddInter(stem(30))],
                },
                error: Some("system-two"),
            },
        );
        let mut step = HeadlessStemsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "system-two")]);
        assert!(sheet.systems[0].inters.iter().any(|inter| inter.id == 30));
        assert_eq!(
            step.visual().calls,
            vec![
                ("process", 2),
                ("process", 1),
                ("finalize", 2),
                ("finalize", 1),
            ]
        );
        assert!(
            sheet
                .mutations
                .contains(&StemsMutation::SystemFailed { system_id: 2 })
        );
    }

    #[test]
    fn epilog_failure_retains_prefix_and_stops_before_contextualization() {
        let mut visual = FakeVisual::default();
        visual.finalize.insert(
            2,
            StemsStageOutcome {
                delta: StemsDelta {
                    mutations: vec![StemsDeltaMutation::SetAbnormal {
                        inter_id: 20,
                        abnormal: true,
                    }],
                },
                error: Some("finalize-two"),
            },
        );
        let mut step = HeadlessStemsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(StemsStepError::Epilog {
                source: "finalize-two",
                system_errors: Vec::new(),
            })
        );
        assert!(
            sheet.systems[0]
                .inters
                .iter()
                .find(|inter| inter.id == 20)
                .unwrap()
                .abnormal
        );
        assert!(!sheet.systems[0].contextualized);
        assert!(!sheet.systems[1].contextualized);
        assert_eq!(
            step.visual().calls,
            vec![("process", 2), ("process", 1), ("finalize", 2)]
        );
    }

    #[test]
    fn relation_overflow_retains_group_and_assignment_prolog_prefix() {
        let mut sheet = sheet();
        sheet.next_relation_id = usize::MAX;
        let mut step = HeadlessStemsStep::new(FakeVisual::default());

        assert_eq!(
            step.process(&mut sheet),
            Err(StemsStepError::Prolog(
                StemsContractError::RelationIdentityOverflow
            ))
        );
        assert_eq!(sheet.next_inter_id, 101);
        assert_eq!(sheet.next_relation_id, usize::MAX);
        assert!(sheet.systems[0].inters.iter().any(|inter| inter.id == 100));
        assert_eq!(
            sheet.mutations,
            vec![
                StemsMutation::LegacyBeamGroupAdded {
                    system_id: 2,
                    group_id: 100,
                },
                StemsMutation::BeamAssigned {
                    system_id: 2,
                    beam_id: 21,
                    group_id: 100,
                },
            ]
        );
        assert!(step.visual().calls.is_empty());
    }

    #[test]
    fn contract_failure_keeps_valid_visual_delta_prefix() {
        let mut visual = FakeVisual::default();
        visual.process.insert(
            2,
            StemsStageOutcome::success(StemsDelta {
                mutations: vec![
                    StemsDeltaMutation::AddInter(stem(30)),
                    StemsDeltaMutation::AddInter(stem(30)),
                ],
            }),
        );
        let mut step = HeadlessStemsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(StemsStepError::Contract(
                StemsContractError::DuplicateInter {
                    system_id: 2,
                    inter_id: 30,
                }
            ))
        );
        assert_eq!(
            sheet.mutations.last(),
            Some(&StemsMutation::InterAdded {
                system_id: 2,
                inter_id: 30,
            })
        );
        assert_eq!(step.visual().calls, vec![("process", 2)]);
    }
}
