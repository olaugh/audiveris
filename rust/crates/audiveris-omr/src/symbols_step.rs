// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle and ownership port of Java `SymbolsStep`.
//!
//! Raster filtering, glyph clustering/classification, symbol construction,
//! rest-chord semantics, and late symbol checks remain injected. This module
//! owns the exact filter → per-system symbols → rest chords → late checks
//! lifecycle, optional-glyph context, glyph/SIG/system ownership, exclusions,
//! cascading cleanup, and retained checked-failure prefixes.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralSymbolGlyph {
    pub id: usize,
    pub section_ids: Vec<usize>,
    pub symbol_group: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeutralSymbolInterKind {
    Symbol { glyph_id: usize, shape: u16 },
    Rest { glyph_id: usize },
    RestChord { rest_ids: Vec<usize> },
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralSymbolInter {
    pub id: usize,
    pub kind: NeutralSymbolInterKind,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralSymbolRelationKind {
    OverlapExclusion,
    ChordMember,
    Support,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralSymbolRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralSymbolRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralSymbolsSystem {
    pub id: usize,
    /// SYMBOL glyphs dispatched by centroid/system-area order in the prolog.
    pub free_glyph_ids: Vec<usize>,
    pub inters: Vec<NeutralSymbolInter>,
    pub relations: Vec<NeutralSymbolRelation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NeutralSymbolsContext {
    /// Weak/transient glyphs rebuilt while erasing inters; system source order
    /// and glyph order are both significant.
    pub optionals_by_system: BTreeMap<usize, Vec<NeutralSymbolGlyph>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralSymbolsSheet {
    pub id: usize,
    pub registered_glyphs: Vec<NeutralSymbolGlyph>,
    pub systems: Vec<NeutralSymbolsSystem>,
    pub mutations: Vec<SymbolsMutation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolsDelta {
    pub mutations: Vec<SymbolsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolsDeltaMutation {
    RegisterGlyph(NeutralSymbolGlyph),
    AttachFreeGlyph { system_id: usize, glyph_id: usize },
    AddInter(NeutralSymbolInter),
    RemoveInter(usize),
    AddRelation(NeutralSymbolRelation),
    RemoveRelation(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolsStageOutcome<VisualError> {
    pub delta: SymbolsDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> SymbolsStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: SymbolsDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolsPrologOutcome<VisualError> {
    pub context: NeutralSymbolsContext,
    pub delta: SymbolsDelta,
    pub error: Option<VisualError>,
}

#[derive(Clone, Copy, Debug)]
pub struct SymbolsSheetInput<'a> {
    pub sheet: &'a NeutralSymbolsSheet,
}

#[derive(Clone, Copy, Debug)]
pub struct SymbolsSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralSymbolsSystem,
    pub optional_glyphs: &'a [NeutralSymbolGlyph],
}

/// First visual/semantic seam in each Java stage.
pub trait VisualSymbolsPipeline {
    type Error;

    fn filter_sheet(&mut self, input: SymbolsSheetInput<'_>) -> SymbolsPrologOutcome<Self::Error>;

    fn build_symbols(&mut self, input: SymbolsSystemInput<'_>) -> SymbolsStageOutcome<Self::Error>;

    fn build_rest_chords(
        &mut self,
        input: SymbolsSystemInput<'_>,
    ) -> SymbolsStageOutcome<Self::Error>;

    fn late_checks(&mut self, input: SymbolsSystemInput<'_>) -> SymbolsStageOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolsSystemPhase {
    BuildSymbols,
    BuildRestChords,
    LateChecks,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolsSystemFailure<VisualError> {
    pub system_id: usize,
    pub phase: SymbolsSystemPhase,
    pub source: VisualError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolsReport<VisualError> {
    pub context: NeutralSymbolsContext,
    pub system_errors: Vec<SymbolsSystemFailure<VisualError>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolsStepError<VisualError> {
    Prolog {
        source: VisualError,
        context: NeutralSymbolsContext,
    },
    Contract(SymbolsContractError),
}

impl<VisualError: fmt::Display> fmt::Display for SymbolsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prolog { source, .. } => write!(formatter, "symbols prolog failed: {source}"),
            Self::Contract(source) => write!(formatter, "symbols contract failed: {source}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolsContractError {
    DuplicateSystem(usize),
    DuplicateOptionalSystem(usize),
    DuplicateOptionalGlyph { system_id: usize, glyph_id: usize },
    UnknownMutationSystem(usize),
    DuplicateGlyph(usize),
    UnknownGlyph(usize),
    DuplicateFreeGlyph { system_id: usize, glyph_id: usize },
    DuplicateInter(usize),
    UnknownInter(usize),
    InvalidInterGlyph { inter_id: usize, glyph_id: usize },
    DuplicateRelation(usize),
    UnknownRelation(usize),
    UnknownRelationEndpoint { relation_id: usize, inter_id: usize },
    DuplicateRestMember { chord_id: usize, rest_id: usize },
}

impl fmt::Display for SymbolsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for SymbolsContractError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolsMutation {
    GlyphRegistered {
        glyph_id: usize,
    },
    FreeGlyphAttached {
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
    SystemFailed {
        system_id: usize,
        phase: SymbolsSystemPhase,
    },
}

pub struct HeadlessSymbolsStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessSymbolsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualSymbolsPipeline> HeadlessSymbolsStep<Visual> {
    pub fn process(
        &mut self,
        sheet: &mut NeutralSymbolsSheet,
    ) -> Result<SymbolsReport<Visual::Error>, SymbolsStepError<Visual::Error>> {
        validate_system_ids(sheet).map_err(SymbolsStepError::Contract)?;
        let prolog = self.visual.filter_sheet(SymbolsSheetInput { sheet });
        apply_delta(sheet, None, prolog.delta).map_err(SymbolsStepError::Contract)?;
        validate_context(sheet, &prolog.context).map_err(SymbolsStepError::Contract)?;
        if let Some(source) = prolog.error {
            return Err(SymbolsStepError::Prolog {
                source,
                context: prolog.context,
            });
        }
        let context = prolog.context;
        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let optional_glyphs = context
                .optionals_by_system
                .get(&system_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let stages = [
                SymbolsSystemPhase::BuildSymbols,
                SymbolsSystemPhase::BuildRestChords,
                SymbolsSystemPhase::LateChecks,
            ];
            for phase in stages {
                let input = SymbolsSystemInput {
                    sheet_id: sheet.id,
                    system: &sheet.systems[system_index],
                    optional_glyphs,
                };
                let outcome = match phase {
                    SymbolsSystemPhase::BuildSymbols => self.visual.build_symbols(input),
                    SymbolsSystemPhase::BuildRestChords => self.visual.build_rest_chords(input),
                    SymbolsSystemPhase::LateChecks => self.visual.late_checks(input),
                };
                apply_delta(sheet, Some(system_index), outcome.delta)
                    .map_err(SymbolsStepError::Contract)?;
                if let Some(source) = outcome.error {
                    sheet
                        .mutations
                        .push(SymbolsMutation::SystemFailed { system_id, phase });
                    system_errors.push(SymbolsSystemFailure {
                        system_id,
                        phase,
                        source,
                    });
                    break;
                }
            }
        }
        Ok(SymbolsReport {
            context,
            system_errors,
        })
    }
}

fn validate_system_ids(sheet: &NeutralSymbolsSheet) -> Result<(), SymbolsContractError> {
    let mut ids = BTreeSet::new();
    for system in &sheet.systems {
        if !ids.insert(system.id) {
            return Err(SymbolsContractError::DuplicateSystem(system.id));
        }
    }
    Ok(())
}

fn validate_context(
    sheet: &NeutralSymbolsSheet,
    context: &NeutralSymbolsContext,
) -> Result<(), SymbolsContractError> {
    let system_ids = sheet
        .systems
        .iter()
        .map(|system| system.id)
        .collect::<BTreeSet<_>>();
    for (system_id, glyphs) in &context.optionals_by_system {
        if !system_ids.contains(system_id) {
            return Err(SymbolsContractError::DuplicateOptionalSystem(*system_id));
        }
        let mut ids = BTreeSet::new();
        for glyph in glyphs {
            if !ids.insert(glyph.id) {
                return Err(SymbolsContractError::DuplicateOptionalGlyph {
                    system_id: *system_id,
                    glyph_id: glyph.id,
                });
            }
        }
    }
    Ok(())
}

fn apply_delta(
    sheet: &mut NeutralSymbolsSheet,
    system_index: Option<usize>,
    delta: SymbolsDelta,
) -> Result<(), SymbolsContractError> {
    for mutation in delta.mutations {
        match mutation {
            SymbolsDeltaMutation::RegisterGlyph(glyph) => {
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|item| item.id == glyph.id)
                {
                    return Err(SymbolsContractError::DuplicateGlyph(glyph.id));
                }
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet
                    .mutations
                    .push(SymbolsMutation::GlyphRegistered { glyph_id });
            }
            SymbolsDeltaMutation::AttachFreeGlyph {
                system_id,
                glyph_id,
            } => {
                if !sheet
                    .registered_glyphs
                    .iter()
                    .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(SymbolsContractError::UnknownGlyph(glyph_id));
                }
                let system = sheet
                    .systems
                    .iter_mut()
                    .find(|system| system.id == system_id)
                    .ok_or(SymbolsContractError::UnknownMutationSystem(system_id))?;
                if system.free_glyph_ids.contains(&glyph_id) {
                    return Err(SymbolsContractError::DuplicateFreeGlyph {
                        system_id,
                        glyph_id,
                    });
                }
                system.free_glyph_ids.push(glyph_id);
                sheet.mutations.push(SymbolsMutation::FreeGlyphAttached {
                    system_id,
                    glyph_id,
                });
            }
            other => {
                let index = system_index.ok_or(SymbolsContractError::UnknownMutationSystem(0))?;
                apply_system_mutation(sheet, index, other)?;
            }
        }
    }
    Ok(())
}

fn apply_system_mutation(
    sheet: &mut NeutralSymbolsSheet,
    system_index: usize,
    mutation: SymbolsDeltaMutation,
) -> Result<(), SymbolsContractError> {
    let system_id = sheet.systems[system_index].id;
    match mutation {
        SymbolsDeltaMutation::AddInter(inter) => {
            if sheet.systems[system_index]
                .inters
                .iter()
                .any(|item| item.id == inter.id)
            {
                return Err(SymbolsContractError::DuplicateInter(inter.id));
            }
            let glyph_id = match inter.kind {
                NeutralSymbolInterKind::Symbol { glyph_id, .. }
                | NeutralSymbolInterKind::Rest { glyph_id } => Some(glyph_id),
                _ => None,
            };
            if let Some(glyph_id) = glyph_id {
                if !sheet
                    .registered_glyphs
                    .iter()
                    .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(SymbolsContractError::InvalidInterGlyph {
                        inter_id: inter.id,
                        glyph_id,
                    });
                }
            }
            let inter_id = inter.id;
            sheet.systems[system_index].inters.push(inter);
            sheet.mutations.push(SymbolsMutation::InterAdded {
                system_id,
                inter_id,
            });
        }
        SymbolsDeltaMutation::RemoveInter(inter_id) => remove_inter(sheet, system_index, inter_id)?,
        SymbolsDeltaMutation::AddRelation(relation) => {
            add_relation(sheet, system_index, relation)?;
        }
        SymbolsDeltaMutation::RemoveRelation(relation_id) => {
            remove_relation(sheet, system_index, relation_id)?;
        }
        SymbolsDeltaMutation::RegisterGlyph(_) | SymbolsDeltaMutation::AttachFreeGlyph { .. } => {
            unreachable!("sheet mutations handled by apply_delta")
        }
    }
    Ok(())
}

fn add_relation(
    sheet: &mut NeutralSymbolsSheet,
    system_index: usize,
    relation: NeutralSymbolRelation,
) -> Result<(), SymbolsContractError> {
    let system_id = sheet.systems[system_index].id;
    if sheet.systems[system_index]
        .relations
        .iter()
        .any(|item| item.id == relation.id)
    {
        return Err(SymbolsContractError::DuplicateRelation(relation.id));
    }
    for inter_id in [relation.source_inter_id, relation.target_inter_id] {
        if !sheet.systems[system_index]
            .inters
            .iter()
            .any(|inter| inter.id == inter_id && !inter.removed)
        {
            return Err(SymbolsContractError::UnknownRelationEndpoint {
                relation_id: relation.id,
                inter_id,
            });
        }
    }
    if relation.kind == NeutralSymbolRelationKind::ChordMember {
        let chord = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|inter| inter.id == relation.source_inter_id)
            .expect("endpoint validated");
        let NeutralSymbolInterKind::RestChord { rest_ids } = &mut chord.kind else {
            return Err(SymbolsContractError::UnknownInter(relation.source_inter_id));
        };
        if rest_ids.contains(&relation.target_inter_id) {
            return Err(SymbolsContractError::DuplicateRestMember {
                chord_id: relation.source_inter_id,
                rest_id: relation.target_inter_id,
            });
        }
        rest_ids.push(relation.target_inter_id);
    }
    let relation_id = relation.id;
    sheet.systems[system_index].relations.push(relation);
    sheet.mutations.push(SymbolsMutation::RelationAdded {
        system_id,
        relation_id,
    });
    Ok(())
}

fn remove_relation(
    sheet: &mut NeutralSymbolsSheet,
    system_index: usize,
    relation_id: usize,
) -> Result<(), SymbolsContractError> {
    let system_id = sheet.systems[system_index].id;
    let Some(index) = sheet.systems[system_index]
        .relations
        .iter()
        .position(|relation| relation.id == relation_id)
    else {
        return Err(SymbolsContractError::UnknownRelation(relation_id));
    };
    let relation = sheet.systems[system_index].relations.remove(index);
    if relation.kind == NeutralSymbolRelationKind::ChordMember {
        if let Some(chord) = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|inter| inter.id == relation.source_inter_id)
        {
            if let NeutralSymbolInterKind::RestChord { rest_ids } = &mut chord.kind {
                rest_ids.retain(|id| *id != relation.target_inter_id);
            }
        }
    }
    sheet.mutations.push(SymbolsMutation::RelationRemoved {
        system_id,
        relation_id,
    });
    Ok(())
}

fn remove_inter(
    sheet: &mut NeutralSymbolsSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), SymbolsContractError> {
    let system_id = sheet.systems[system_index].id;
    if !sheet.systems[system_index]
        .inters
        .iter()
        .any(|inter| inter.id == inter_id && !inter.removed)
    {
        return Err(SymbolsContractError::UnknownInter(inter_id));
    }
    let relations = sheet.systems[system_index]
        .relations
        .iter()
        .filter(|relation| {
            relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
        })
        .map(|relation| relation.id)
        .collect::<Vec<_>>();
    for relation_id in relations {
        remove_relation(sheet, system_index, relation_id)?;
    }
    let inter = sheet.systems[system_index]
        .inters
        .iter_mut()
        .find(|inter| inter.id == inter_id)
        .expect("live inter validated");
    inter.removed = true;
    sheet.mutations.push(SymbolsMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeVisual {
        prolog: Option<SymbolsPrologOutcome<&'static str>>,
        outcomes: BTreeMap<(usize, SymbolsSystemPhase), SymbolsStageOutcome<&'static str>>,
        calls: Vec<(&'static str, usize, Vec<usize>, usize)>,
    }

    impl VisualSymbolsPipeline for FakeVisual {
        type Error = &'static str;

        fn filter_sheet(
            &mut self,
            input: SymbolsSheetInput<'_>,
        ) -> SymbolsPrologOutcome<Self::Error> {
            self.calls.push(("filter", input.sheet.id, Vec::new(), 0));
            self.prolog.take().unwrap()
        }

        fn build_symbols(
            &mut self,
            input: SymbolsSystemInput<'_>,
        ) -> SymbolsStageOutcome<Self::Error> {
            self.record("symbols", SymbolsSystemPhase::BuildSymbols, input)
        }

        fn build_rest_chords(
            &mut self,
            input: SymbolsSystemInput<'_>,
        ) -> SymbolsStageOutcome<Self::Error> {
            self.record("rests", SymbolsSystemPhase::BuildRestChords, input)
        }

        fn late_checks(
            &mut self,
            input: SymbolsSystemInput<'_>,
        ) -> SymbolsStageOutcome<Self::Error> {
            self.record("late", SymbolsSystemPhase::LateChecks, input)
        }
    }

    impl FakeVisual {
        fn record(
            &mut self,
            name: &'static str,
            phase: SymbolsSystemPhase,
            input: SymbolsSystemInput<'_>,
        ) -> SymbolsStageOutcome<&'static str> {
            self.calls.push((
                name,
                input.system.id,
                input.optional_glyphs.iter().map(|glyph| glyph.id).collect(),
                input.system.inters.len(),
            ));
            self.outcomes
                .remove(&(input.system.id, phase))
                .unwrap_or_else(|| SymbolsStageOutcome::success(SymbolsDelta::default()))
        }
    }

    fn glyph(id: usize) -> NeutralSymbolGlyph {
        NeutralSymbolGlyph {
            id,
            section_ids: vec![id + 100],
            symbol_group: true,
        }
    }

    fn sheet() -> NeutralSymbolsSheet {
        NeutralSymbolsSheet {
            id: 9,
            registered_glyphs: Vec::new(),
            systems: vec![
                NeutralSymbolsSystem {
                    id: 2,
                    free_glyph_ids: Vec::new(),
                    inters: Vec::new(),
                    relations: Vec::new(),
                },
                NeutralSymbolsSystem {
                    id: 1,
                    free_glyph_ids: Vec::new(),
                    inters: Vec::new(),
                    relations: Vec::new(),
                },
            ],
            mutations: Vec::new(),
        }
    }

    fn prolog() -> SymbolsPrologOutcome<&'static str> {
        SymbolsPrologOutcome {
            context: NeutralSymbolsContext {
                optionals_by_system: BTreeMap::from([(2, vec![glyph(90), glyph(91)])]),
            },
            delta: SymbolsDelta {
                mutations: vec![
                    SymbolsDeltaMutation::RegisterGlyph(glyph(10)),
                    SymbolsDeltaMutation::AttachFreeGlyph {
                        system_id: 2,
                        glyph_id: 10,
                    },
                ],
            },
            error: None,
        }
    }

    #[test]
    fn filter_then_three_system_phases_run_in_exact_java_order() {
        let mut visual = FakeVisual {
            prolog: Some(prolog()),
            ..FakeVisual::default()
        };
        visual.outcomes.insert(
            (2, SymbolsSystemPhase::BuildSymbols),
            SymbolsStageOutcome::success(SymbolsDelta {
                mutations: vec![SymbolsDeltaMutation::AddInter(NeutralSymbolInter {
                    id: 20,
                    kind: NeutralSymbolInterKind::Symbol {
                        glyph_id: 10,
                        shape: 3,
                    },
                    removed: false,
                })],
            }),
        );
        let mut step = HeadlessSymbolsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert!(report.system_errors.is_empty());
        assert_eq!(
            step.visual().calls,
            vec![
                ("filter", 9, vec![], 0),
                ("symbols", 2, vec![90, 91], 0),
                ("rests", 2, vec![90, 91], 1),
                ("late", 2, vec![90, 91], 1),
                ("symbols", 1, vec![], 0),
                ("rests", 1, vec![], 0),
                ("late", 1, vec![], 0),
            ]
        );
        assert_eq!(sheet.systems[0].free_glyph_ids, vec![10]);
    }

    #[test]
    fn system_failure_keeps_prefix_skips_later_phases_and_continues() {
        let mut visual = FakeVisual {
            prolog: Some(prolog()),
            ..FakeVisual::default()
        };
        visual.outcomes.insert(
            (2, SymbolsSystemPhase::BuildRestChords),
            SymbolsStageOutcome {
                delta: SymbolsDelta {
                    mutations: vec![SymbolsDeltaMutation::AddInter(NeutralSymbolInter {
                        id: 30,
                        kind: NeutralSymbolInterKind::RestChord {
                            rest_ids: Vec::new(),
                        },
                        removed: false,
                    })],
                },
                error: Some("rest chord"),
            },
        );
        let mut step = HeadlessSymbolsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors.len(), 1);
        assert_eq!(
            report.system_errors[0].phase,
            SymbolsSystemPhase::BuildRestChords
        );
        assert!(
            !step
                .visual()
                .calls
                .iter()
                .any(|call| call.0 == "late" && call.1 == 2)
        );
        assert!(
            step.visual()
                .calls
                .iter()
                .any(|call| call.0 == "symbols" && call.1 == 1)
        );
    }

    #[test]
    fn overlap_exclusions_and_rest_members_cleanup_before_vertex() {
        let mut visual = FakeVisual {
            prolog: Some(prolog()),
            ..FakeVisual::default()
        };
        visual.outcomes.insert(
            (2, SymbolsSystemPhase::BuildSymbols),
            SymbolsStageOutcome::success(SymbolsDelta {
                mutations: vec![
                    SymbolsDeltaMutation::AddInter(NeutralSymbolInter {
                        id: 40,
                        kind: NeutralSymbolInterKind::Other,
                        removed: false,
                    }),
                    SymbolsDeltaMutation::AddInter(NeutralSymbolInter {
                        id: 41,
                        kind: NeutralSymbolInterKind::Other,
                        removed: false,
                    }),
                    SymbolsDeltaMutation::AddRelation(NeutralSymbolRelation {
                        id: 50,
                        source_inter_id: 40,
                        target_inter_id: 41,
                        kind: NeutralSymbolRelationKind::OverlapExclusion,
                    }),
                    SymbolsDeltaMutation::RemoveInter(41),
                ],
            }),
        );
        let mut step = HeadlessSymbolsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert!(sheet.systems[0].relations.is_empty());
        assert_eq!(
            &sheet.mutations[4..7],
            &[
                SymbolsMutation::RelationAdded {
                    system_id: 2,
                    relation_id: 50,
                },
                SymbolsMutation::RelationRemoved {
                    system_id: 2,
                    relation_id: 50,
                },
                SymbolsMutation::InterRemoved {
                    system_id: 2,
                    inter_id: 41,
                },
            ]
        );
    }

    #[test]
    fn prolog_failure_retains_registration_dispatch_prefix_and_stops() {
        let mut failed = prolog();
        failed.error = Some("filter");
        let visual = FakeVisual {
            prolog: Some(failed),
            ..FakeVisual::default()
        };
        let mut step = HeadlessSymbolsStep::new(visual);
        let mut sheet = sheet();

        assert!(matches!(
            step.process(&mut sheet),
            Err(SymbolsStepError::Prolog {
                source: "filter",
                ..
            })
        ));
        assert_eq!(sheet.registered_glyphs.len(), 1);
        assert_eq!(sheet.systems[0].free_glyph_ids, vec![10]);
        assert_eq!(step.visual().calls, vec![("filter", 9, vec![], 0)]);
    }
}
