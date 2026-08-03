// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `LedgersStep`.
//!
//! Ledger-section extraction and geometric interpretation remain explicit
//! visual seams. This module owns the Java lifecycle around them: one sheet
//! prolog, systems in sheet order, sheet-wide cleanup/rebuild, then ledger-line
//! construction for every staff in system/staff order.

use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerSection {
    pub id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerGlyph {
    pub id: usize,
    pub section_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralLedgerInter {
    pub id: usize,
    pub glyph_id: usize,
    pub staff_id: usize,
    /// Ledger line relative to the staff: negative above, positive below.
    pub index: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralLedgerRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerLine {
    pub index: i32,
    pub inter_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerStaff {
    pub id: usize,
    /// Java `Staff.ledgerMap`, preserving inter order at each line index.
    pub ledgers: BTreeMap<i32, Vec<usize>>,
    /// Java `Staff.buildAllLedgerLines()` output.
    pub ledger_lines: Vec<NeutralLedgerLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerSystem {
    pub id: usize,
    pub staves: Vec<NeutralLedgerStaff>,
    /// Java system SIG ownership, in insertion order.
    pub inters: Vec<NeutralLedgerInter>,
    pub relations: Vec<NeutralLedgerRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralLedgerSheet {
    pub id: usize,
    pub systems: Vec<NeutralLedgerSystem>,
    /// Java sheet `GlyphIndex` ownership, in registration order.
    pub registered_glyphs: Vec<NeutralLedgerGlyph>,
    pub mutations: Vec<LedgersMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerSystemSections {
    pub system_id: usize,
    pub sections: Vec<NeutralLedgerSection>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerDelta {
    /// Exact mutation order; a checked failure retains this prefix.
    pub mutations: Vec<LedgerDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerDeltaMutation {
    RegisterGlyph(NeutralLedgerGlyph),
    AddInter(NeutralLedgerInter),
    RemoveInter {
        system_id: usize,
        inter_id: usize,
    },
    AddRelation(NeutralLedgerRelation),
    RemoveRelation {
        system_id: usize,
        relation_id: usize,
    },
    AttachToStaff {
        system_id: usize,
        staff_id: usize,
        index: i32,
        inter_id: usize,
    },
    DetachFromStaff {
        system_id: usize,
        staff_id: usize,
        index: i32,
        inter_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerStageOutcome<VisualError> {
    pub delta: LedgerDelta,
    pub error: Option<VisualError>,
}

impl<VisualError> LedgerStageOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: LedgerDelta) -> Self {
        Self { delta, error: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LedgerFilterInput<'a> {
    pub sheet: &'a NeutralLedgerSheet,
}

#[derive(Clone, Copy, Debug)]
pub struct LedgerSystemInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralLedgerSystem,
    pub sections: &'a [NeutralLedgerSection],
}

#[derive(Clone, Copy, Debug)]
pub struct LedgerPostAnalysisInput<'a> {
    pub sheet: &'a NeutralLedgerSheet,
    /// Builders are created before each Java `buildLedgers` invocation, so
    /// failed systems remain available to post-analysis.
    pub builder_system_ids: &'a [usize],
}

#[derive(Clone, Copy, Debug)]
pub struct LedgerLinesInput<'a> {
    pub sheet_id: usize,
    pub system_id: usize,
    pub staff: &'a NeutralLedgerStaff,
}

/// Visual boundaries in Java source order. The first is
/// `new LedgersFilter(sheet).process()`.
pub trait VisualLedgers {
    type Error;

    fn filter_ledger_sections(
        &mut self,
        input: LedgerFilterInput<'_>,
    ) -> Result<Vec<LedgerSystemSections>, Self::Error>;

    fn build_system_ledgers(
        &mut self,
        input: LedgerSystemInput<'_>,
    ) -> LedgerStageOutcome<Self::Error>;

    /// Java `collect`, `filter`, and impacted-system `rebuildLedgers`.
    fn post_analyze(
        &mut self,
        input: LedgerPostAnalysisInput<'_>,
    ) -> LedgerStageOutcome<Self::Error>;

    fn build_staff_ledger_lines(
        &mut self,
        input: LedgerLinesInput<'_>,
    ) -> Result<Vec<NeutralLedgerLine>, Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgersReport<VisualError> {
    /// Java catches checked per-system failures and continues later systems.
    pub system_errors: Vec<(usize, VisualError)>,
    pub builder_system_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgersStepError<VisualError> {
    Prolog(VisualError),
    Epilog {
        source: VisualError,
        system_errors: Vec<(usize, VisualError)>,
    },
    Contract(LedgersContractError),
}

impl<VisualError: fmt::Display> fmt::Display for LedgersStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prolog(source) => write!(formatter, "ledger filtering failed: {source}"),
            Self::Epilog { source, .. } => {
                write!(formatter, "ledger post-analysis failed: {source}")
            }
            Self::Contract(source) => write!(formatter, "ledger contract failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for LedgersStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Prolog(source) | Self::Epilog { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgersContractError {
    DuplicateSystem(usize),
    DuplicateStaff(usize),
    MissingSectionOwner(usize),
    DuplicateSectionOwner(usize),
    UnknownSystem(usize),
    UnknownStaff {
        system_id: usize,
        staff_id: usize,
    },
    DuplicateGlyph(usize),
    DuplicateInter {
        system_id: usize,
        inter_id: usize,
    },
    UnknownInter {
        system_id: usize,
        inter_id: usize,
    },
    InterStaffMismatch {
        inter_id: usize,
        staff_id: usize,
    },
    InterIndexMismatch {
        inter_id: usize,
        index: i32,
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
    DuplicateStaffLedger {
        staff_id: usize,
        inter_id: usize,
    },
    MissingStaffLedger {
        staff_id: usize,
        inter_id: usize,
    },
    AttachedInter {
        system_id: usize,
        inter_id: usize,
    },
    InvalidLedgerLine {
        staff_id: usize,
        inter_id: usize,
    },
}

impl fmt::Display for LedgersContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSystem(id) => write!(formatter, "duplicate system {id}"),
            Self::DuplicateStaff(id) => write!(formatter, "duplicate staff {id}"),
            Self::MissingSectionOwner(id) => write!(formatter, "missing sections for system {id}"),
            Self::DuplicateSectionOwner(id) => {
                write!(formatter, "duplicate section owner system {id}")
            }
            Self::UnknownSystem(id) => write!(formatter, "unknown system {id}"),
            Self::UnknownStaff {
                system_id,
                staff_id,
            } => write!(formatter, "unknown staff {staff_id} in system {system_id}"),
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
            Self::InterStaffMismatch { inter_id, staff_id } => {
                write!(
                    formatter,
                    "inter {inter_id} does not belong to staff {staff_id}"
                )
            }
            Self::InterIndexMismatch { inter_id, index } => {
                write!(
                    formatter,
                    "inter {inter_id} does not belong to ledger index {index}"
                )
            }
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
            Self::DuplicateStaffLedger { staff_id, inter_id } => {
                write!(formatter, "staff {staff_id} already owns inter {inter_id}")
            }
            Self::MissingStaffLedger { staff_id, inter_id } => {
                write!(formatter, "staff {staff_id} does not own inter {inter_id}")
            }
            Self::AttachedInter {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "inter {inter_id} remains attached in system {system_id}"
            ),
            Self::InvalidLedgerLine { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} line references foreign inter {inter_id}"
            ),
        }
    }
}

impl Error for LedgersContractError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgersMutation {
    BuilderCreated {
        system_id: usize,
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
    StaffLedgerAttached {
        staff_id: usize,
        index: i32,
        inter_id: usize,
    },
    StaffLedgerDetached {
        staff_id: usize,
        index: i32,
        inter_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
    StaffLedgerLinesBuilt {
        system_id: usize,
        staff_id: usize,
    },
}

pub struct HeadlessLedgersStep<Visual> {
    visual: Visual,
}

impl<Visual> HeadlessLedgersStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self { visual }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualLedgers> HeadlessLedgersStep<Visual> {
    /// Java `AbstractSystemStep.doit`: prolog, systems in sheet order, epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralLedgerSheet,
    ) -> Result<LedgersReport<Visual::Error>, LedgersStepError<Visual::Error>> {
        validate_sheet_ownership(sheet).map_err(LedgersStepError::Contract)?;
        let section_map = self
            .visual
            .filter_ledger_sections(LedgerFilterInput { sheet })
            .map_err(LedgersStepError::Prolog)?;
        let section_map =
            validate_section_map(sheet, section_map).map_err(LedgersStepError::Contract)?;

        let mut system_errors = Vec::new();
        let mut builder_system_ids = Vec::with_capacity(sheet.systems.len());
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            // Java inserts the builder in Context before `buildLedgers`.
            builder_system_ids.push(system_id);
            sheet
                .mutations
                .push(LedgersMutation::BuilderCreated { system_id });
            let outcome = self.visual.build_system_ledgers(LedgerSystemInput {
                sheet_id: sheet.id,
                system: &sheet.systems[system_index],
                sections: &section_map[&system_id],
            });
            apply_delta(sheet, Some(system_id), outcome.delta)
                .map_err(LedgersStepError::Contract)?;
            if let Some(error) = outcome.error {
                sheet
                    .mutations
                    .push(LedgersMutation::SystemFailed { system_id });
                system_errors.push((system_id, error));
            }
        }

        let epilog = self.visual.post_analyze(LedgerPostAnalysisInput {
            sheet,
            builder_system_ids: &builder_system_ids,
        });
        apply_delta(sheet, None, epilog.delta).map_err(LedgersStepError::Contract)?;
        if let Some(source) = epilog.error {
            return Err(LedgersStepError::Epilog {
                source,
                system_errors,
            });
        }

        // Final Java nesting is systems first, then each system's staves.
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            for staff_index in 0..sheet.systems[system_index].staves.len() {
                let lines = match self.visual.build_staff_ledger_lines(LedgerLinesInput {
                    sheet_id: sheet.id,
                    system_id,
                    staff: &sheet.systems[system_index].staves[staff_index],
                }) {
                    Ok(lines) => lines,
                    Err(source) => {
                        return Err(LedgersStepError::Epilog {
                            source,
                            system_errors,
                        });
                    }
                };
                validate_lines(
                    &sheet.systems[system_index],
                    &sheet.systems[system_index].staves[staff_index],
                    &lines,
                )
                .map_err(LedgersStepError::Contract)?;
                let staff_id = sheet.systems[system_index].staves[staff_index].id;
                sheet.systems[system_index].staves[staff_index].ledger_lines = lines;
                sheet
                    .mutations
                    .push(LedgersMutation::StaffLedgerLinesBuilt {
                        system_id,
                        staff_id,
                    });
            }
        }

        Ok(LedgersReport {
            system_errors,
            builder_system_ids,
        })
    }
}

fn validate_sheet_ownership(sheet: &NeutralLedgerSheet) -> Result<(), LedgersContractError> {
    let mut system_ids = Vec::new();
    let mut staff_ids = Vec::new();
    for system in &sheet.systems {
        if system_ids.contains(&system.id) {
            return Err(LedgersContractError::DuplicateSystem(system.id));
        }
        system_ids.push(system.id);
        for staff in &system.staves {
            if staff_ids.contains(&staff.id) {
                return Err(LedgersContractError::DuplicateStaff(staff.id));
            }
            staff_ids.push(staff.id);
        }
    }
    Ok(())
}

fn validate_section_map(
    sheet: &NeutralLedgerSheet,
    entries: Vec<LedgerSystemSections>,
) -> Result<BTreeMap<usize, Vec<NeutralLedgerSection>>, LedgersContractError> {
    let mut map = BTreeMap::new();
    for entry in entries {
        if !sheet
            .systems
            .iter()
            .any(|system| system.id == entry.system_id)
        {
            return Err(LedgersContractError::UnknownSystem(entry.system_id));
        }
        if map.insert(entry.system_id, entry.sections).is_some() {
            return Err(LedgersContractError::DuplicateSectionOwner(entry.system_id));
        }
    }
    for system in &sheet.systems {
        if !map.contains_key(&system.id) {
            return Err(LedgersContractError::MissingSectionOwner(system.id));
        }
    }
    Ok(map)
}

fn system_index(sheet: &NeutralLedgerSheet, id: usize) -> Result<usize, LedgersContractError> {
    sheet
        .systems
        .iter()
        .position(|system| system.id == id)
        .ok_or(LedgersContractError::UnknownSystem(id))
}

fn staff_index(
    system: &NeutralLedgerSystem,
    staff_id: usize,
) -> Result<usize, LedgersContractError> {
    system
        .staves
        .iter()
        .position(|staff| staff.id == staff_id)
        .ok_or(LedgersContractError::UnknownStaff {
            system_id: system.id,
            staff_id,
        })
}

fn apply_delta(
    sheet: &mut NeutralLedgerSheet,
    target_system_id: Option<usize>,
    delta: LedgerDelta,
) -> Result<(), LedgersContractError> {
    for mutation in delta.mutations {
        match mutation {
            LedgerDeltaMutation::RegisterGlyph(glyph) => {
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|existing| existing.id == glyph.id)
                {
                    return Err(LedgersContractError::DuplicateGlyph(glyph.id));
                }
                let system_id = target_system_id.ok_or(LedgersContractError::UnknownSystem(0))?;
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(LedgersMutation::GlyphRegistered {
                    system_id,
                    glyph_id,
                });
            }
            LedgerDeltaMutation::AddInter(inter) => {
                let system_id = target_system_id.ok_or(LedgersContractError::UnknownSystem(0))?;
                let index = system_index(sheet, system_id)?;
                staff_index(&sheet.systems[index], inter.staff_id)?;
                if sheet.systems[index]
                    .inters
                    .iter()
                    .any(|existing| existing.id == inter.id)
                {
                    return Err(LedgersContractError::DuplicateInter {
                        system_id,
                        inter_id: inter.id,
                    });
                }
                let inter_id = inter.id;
                sheet.systems[index].inters.push(inter);
                sheet.mutations.push(LedgersMutation::InterAdded {
                    system_id,
                    inter_id,
                });
            }
            LedgerDeltaMutation::RemoveInter {
                system_id,
                inter_id,
            } => {
                let index = system_index(sheet, system_id)?;
                if sheet.systems[index]
                    .staves
                    .iter()
                    .any(|staff| staff.ledgers.values().flatten().any(|id| *id == inter_id))
                {
                    return Err(LedgersContractError::AttachedInter {
                        system_id,
                        inter_id,
                    });
                }
                let Some(inter_index) = sheet.systems[index]
                    .inters
                    .iter()
                    .position(|inter| inter.id == inter_id)
                else {
                    return Err(LedgersContractError::UnknownInter {
                        system_id,
                        inter_id,
                    });
                };
                sheet.systems[index].inters.remove(inter_index);
                sheet.mutations.push(LedgersMutation::InterRemoved {
                    system_id,
                    inter_id,
                });
            }
            LedgerDeltaMutation::AddRelation(relation) => {
                let system_id = target_system_id.ok_or(LedgersContractError::UnknownSystem(0))?;
                let index = system_index(sheet, system_id)?;
                for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                    if !sheet.systems[index]
                        .inters
                        .iter()
                        .any(|inter| inter.id == inter_id)
                    {
                        return Err(LedgersContractError::UnknownRelationEndpoint {
                            system_id,
                            inter_id,
                        });
                    }
                }
                if sheet.systems[index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(LedgersContractError::DuplicateRelation {
                        system_id,
                        relation_id: relation.id,
                    });
                }
                let relation_id = relation.id;
                sheet.systems[index].relations.push(relation);
                sheet.mutations.push(LedgersMutation::RelationAdded {
                    system_id,
                    relation_id,
                });
            }
            LedgerDeltaMutation::RemoveRelation {
                system_id,
                relation_id,
            } => {
                let index = system_index(sheet, system_id)?;
                let Some(relation_index) = sheet.systems[index]
                    .relations
                    .iter()
                    .position(|relation| relation.id == relation_id)
                else {
                    return Err(LedgersContractError::UnknownRelation {
                        system_id,
                        relation_id,
                    });
                };
                sheet.systems[index].relations.remove(relation_index);
                sheet.mutations.push(LedgersMutation::RelationRemoved {
                    system_id,
                    relation_id,
                });
            }
            LedgerDeltaMutation::AttachToStaff {
                system_id,
                staff_id,
                index,
                inter_id,
            } => {
                let system_index = system_index(sheet, system_id)?;
                let staff_index = staff_index(&sheet.systems[system_index], staff_id)?;
                let Some(inter) = sheet.systems[system_index]
                    .inters
                    .iter()
                    .find(|inter| inter.id == inter_id)
                else {
                    return Err(LedgersContractError::UnknownInter {
                        system_id,
                        inter_id,
                    });
                };
                if inter.staff_id != staff_id {
                    return Err(LedgersContractError::InterStaffMismatch { inter_id, staff_id });
                }
                if inter.index != index {
                    return Err(LedgersContractError::InterIndexMismatch { inter_id, index });
                }
                let ledgers = sheet.systems[system_index].staves[staff_index]
                    .ledgers
                    .entry(index)
                    .or_default();
                if ledgers.contains(&inter_id) {
                    return Err(LedgersContractError::DuplicateStaffLedger { staff_id, inter_id });
                }
                ledgers.push(inter_id);
                sheet.mutations.push(LedgersMutation::StaffLedgerAttached {
                    staff_id,
                    index,
                    inter_id,
                });
            }
            LedgerDeltaMutation::DetachFromStaff {
                system_id,
                staff_id,
                index,
                inter_id,
            } => {
                let system_index = system_index(sheet, system_id)?;
                let staff_index = staff_index(&sheet.systems[system_index], staff_id)?;
                let ledgers = sheet.systems[system_index].staves[staff_index]
                    .ledgers
                    .get_mut(&index)
                    .ok_or(LedgersContractError::MissingStaffLedger { staff_id, inter_id })?;
                let position = ledgers
                    .iter()
                    .position(|id| *id == inter_id)
                    .ok_or(LedgersContractError::MissingStaffLedger { staff_id, inter_id })?;
                ledgers.remove(position);
                if ledgers.is_empty() {
                    sheet.systems[system_index].staves[staff_index]
                        .ledgers
                        .remove(&index);
                }
                sheet.mutations.push(LedgersMutation::StaffLedgerDetached {
                    staff_id,
                    index,
                    inter_id,
                });
            }
        }
    }
    Ok(())
}

fn validate_lines(
    system: &NeutralLedgerSystem,
    staff: &NeutralLedgerStaff,
    lines: &[NeutralLedgerLine],
) -> Result<(), LedgersContractError> {
    for line in lines {
        for inter_id in &line.inter_ids {
            if !staff
                .ledgers
                .get(&line.index)
                .is_some_and(|ids| ids.contains(inter_id))
                || !system.inters.iter().any(|inter| inter.id == *inter_id)
            {
                return Err(LedgersContractError::InvalidLedgerLine {
                    staff_id: staff.id,
                    inter_id: *inter_id,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeVisual {
        filter: Option<Result<Vec<LedgerSystemSections>, &'static str>>,
        systems: BTreeMap<usize, LedgerStageOutcome<&'static str>>,
        post: Option<LedgerStageOutcome<&'static str>>,
        lines: BTreeMap<usize, Result<Vec<NeutralLedgerLine>, &'static str>>,
        calls: Vec<(&'static str, usize)>,
        post_builder_ids: Vec<usize>,
        post_inter_ids: Vec<(usize, Vec<usize>)>,
    }

    impl VisualLedgers for FakeVisual {
        type Error = &'static str;

        fn filter_ledger_sections(
            &mut self,
            input: LedgerFilterInput<'_>,
        ) -> Result<Vec<LedgerSystemSections>, Self::Error> {
            self.calls.push(("filter", input.sheet.id));
            self.filter.take().unwrap()
        }

        fn build_system_ledgers(
            &mut self,
            input: LedgerSystemInput<'_>,
        ) -> LedgerStageOutcome<Self::Error> {
            self.calls.push(("system", input.system.id));
            assert_eq!(input.sections[0].id, input.system.id * 10);
            self.systems
                .remove(&input.system.id)
                .unwrap_or_else(|| LedgerStageOutcome::success(LedgerDelta::default()))
        }

        fn post_analyze(
            &mut self,
            input: LedgerPostAnalysisInput<'_>,
        ) -> LedgerStageOutcome<Self::Error> {
            self.calls.push(("post", input.sheet.id));
            self.post_builder_ids = input.builder_system_ids.to_vec();
            self.post_inter_ids = input
                .sheet
                .systems
                .iter()
                .map(|system| {
                    (
                        system.id,
                        system.inters.iter().map(|inter| inter.id).collect(),
                    )
                })
                .collect();
            self.post
                .take()
                .unwrap_or_else(|| LedgerStageOutcome::success(LedgerDelta::default()))
        }

        fn build_staff_ledger_lines(
            &mut self,
            input: LedgerLinesInput<'_>,
        ) -> Result<Vec<NeutralLedgerLine>, Self::Error> {
            self.calls.push(("lines", input.staff.id));
            self.lines.remove(&input.staff.id).unwrap_or(Ok(Vec::new()))
        }
    }

    fn staff(id: usize) -> NeutralLedgerStaff {
        NeutralLedgerStaff {
            id,
            ledgers: BTreeMap::new(),
            ledger_lines: Vec::new(),
        }
    }

    fn system(id: usize, staff_ids: &[usize]) -> NeutralLedgerSystem {
        NeutralLedgerSystem {
            id,
            staves: staff_ids.iter().copied().map(staff).collect(),
            inters: Vec::new(),
            relations: Vec::new(),
        }
    }

    fn sheet() -> NeutralLedgerSheet {
        NeutralLedgerSheet {
            id: 7,
            systems: vec![system(2, &[20, 21]), system(1, &[10])],
            registered_glyphs: Vec::new(),
            mutations: Vec::new(),
        }
    }

    fn sections() -> Vec<LedgerSystemSections> {
        // The filter's map order does not control Java sheet traversal.
        vec![
            LedgerSystemSections {
                system_id: 1,
                sections: vec![NeutralLedgerSection { id: 10 }],
            },
            LedgerSystemSections {
                system_id: 2,
                sections: vec![NeutralLedgerSection { id: 20 }],
            },
        ]
    }

    fn inter(id: usize, staff_id: usize, index: i32) -> NeutralLedgerInter {
        NeutralLedgerInter {
            id,
            glyph_id: id + 1_000,
            staff_id,
            index,
        }
    }

    fn built_ledger_delta(glyph_id: usize, inter: NeutralLedgerInter) -> LedgerDelta {
        LedgerDelta {
            mutations: vec![
                LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                    id: glyph_id,
                    section_ids: vec![glyph_id + 10],
                }),
                LedgerDeltaMutation::AddInter(inter),
                LedgerDeltaMutation::AttachToStaff {
                    system_id: if inter.staff_id >= 20 { 2 } else { 1 },
                    staff_id: inter.staff_id,
                    index: inter.index,
                    inter_id: inter.id,
                },
            ],
        }
    }

    #[test]
    fn runs_filter_systems_cleanup_and_staff_lines_in_java_order() {
        let inter_two = inter(200, 20, -1);
        let inter_one = inter(100, 10, 1);
        let mut visual = FakeVisual {
            filter: Some(Ok(sections())),
            post: Some(LedgerStageOutcome::success(LedgerDelta {
                mutations: vec![
                    LedgerDeltaMutation::DetachFromStaff {
                        system_id: 2,
                        staff_id: 20,
                        index: -1,
                        inter_id: 200,
                    },
                    LedgerDeltaMutation::RemoveInter {
                        system_id: 2,
                        inter_id: 200,
                    },
                ],
            })),
            ..FakeVisual::default()
        };
        visual.systems.insert(
            2,
            LedgerStageOutcome::success(built_ledger_delta(1200, inter_two)),
        );
        visual.systems.insert(
            1,
            LedgerStageOutcome::success(built_ledger_delta(1100, inter_one)),
        );
        visual.lines.insert(
            10,
            Ok(vec![NeutralLedgerLine {
                index: 1,
                inter_ids: vec![100],
            }]),
        );
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, Vec::new());
        assert_eq!(report.builder_system_ids, vec![2, 1]);
        assert_eq!(step.visual().post_builder_ids, vec![2, 1]);
        assert_eq!(
            step.visual().post_inter_ids,
            vec![(2, vec![200]), (1, vec![100])]
        );
        assert_eq!(
            step.visual().calls,
            vec![
                ("filter", 7),
                ("system", 2),
                ("system", 1),
                ("post", 7),
                ("lines", 20),
                ("lines", 21),
                ("lines", 10),
            ]
        );
        assert!(sheet.systems[0].inters.is_empty());
        assert!(sheet.systems[0].staves[0].ledgers.is_empty());
        assert_eq!(sheet.systems[1].inters, vec![inter_one]);
        assert_eq!(
            sheet.systems[1].staves[0].ledger_lines,
            vec![NeutralLedgerLine {
                index: 1,
                inter_ids: vec![100],
            }]
        );
        assert_eq!(
            sheet.mutations,
            vec![
                LedgersMutation::BuilderCreated { system_id: 2 },
                LedgersMutation::GlyphRegistered {
                    system_id: 2,
                    glyph_id: 1200,
                },
                LedgersMutation::InterAdded {
                    system_id: 2,
                    inter_id: 200,
                },
                LedgersMutation::StaffLedgerAttached {
                    staff_id: 20,
                    index: -1,
                    inter_id: 200,
                },
                LedgersMutation::BuilderCreated { system_id: 1 },
                LedgersMutation::GlyphRegistered {
                    system_id: 1,
                    glyph_id: 1100,
                },
                LedgersMutation::InterAdded {
                    system_id: 1,
                    inter_id: 100,
                },
                LedgersMutation::StaffLedgerAttached {
                    staff_id: 10,
                    index: 1,
                    inter_id: 100,
                },
                LedgersMutation::StaffLedgerDetached {
                    staff_id: 20,
                    index: -1,
                    inter_id: 200,
                },
                LedgersMutation::InterRemoved {
                    system_id: 2,
                    inter_id: 200,
                },
                LedgersMutation::StaffLedgerLinesBuilt {
                    system_id: 2,
                    staff_id: 20,
                },
                LedgersMutation::StaffLedgerLinesBuilt {
                    system_id: 2,
                    staff_id: 21,
                },
                LedgersMutation::StaffLedgerLinesBuilt {
                    system_id: 1,
                    staff_id: 10,
                },
            ]
        );
    }

    #[test]
    fn checked_system_failure_retains_prefix_and_later_systems_and_epilog_run() {
        let mut visual = FakeVisual {
            filter: Some(Ok(sections())),
            ..FakeVisual::default()
        };
        visual.systems.insert(
            2,
            LedgerStageOutcome {
                delta: LedgerDelta {
                    mutations: vec![LedgerDeltaMutation::RegisterGlyph(NeutralLedgerGlyph {
                        id: 99,
                        section_ids: vec![20],
                    })],
                },
                error: Some("system-two"),
            },
        );
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, vec![(2, "system-two")]);
        assert_eq!(report.builder_system_ids, vec![2, 1]);
        assert_eq!(step.visual().post_builder_ids, vec![2, 1]);
        assert_eq!(
            step.visual().calls,
            vec![
                ("filter", 7),
                ("system", 2),
                ("system", 1),
                ("post", 7),
                ("lines", 20),
                ("lines", 21),
                ("lines", 10),
            ]
        );
        assert_eq!(sheet.registered_glyphs[0].id, 99);
        assert_eq!(
            &sheet.mutations[..4],
            &[
                LedgersMutation::BuilderCreated { system_id: 2 },
                LedgersMutation::GlyphRegistered {
                    system_id: 2,
                    glyph_id: 99,
                },
                LedgersMutation::SystemFailed { system_id: 2 },
                LedgersMutation::BuilderCreated { system_id: 1 },
            ]
        );
    }

    #[test]
    fn prolog_failure_has_no_lifecycle_mutation() {
        let visual = FakeVisual {
            filter: Some(Err("filter")),
            ..FakeVisual::default()
        };
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(LedgersStepError::Prolog("filter"))
        );
        assert!(sheet.mutations.is_empty());
        assert_eq!(step.visual().calls, vec![("filter", 7)]);
    }

    #[test]
    fn epilog_failure_retains_cleanup_prefix_and_system_errors() {
        let mut visual = FakeVisual {
            filter: Some(Ok(sections())),
            post: Some(LedgerStageOutcome {
                delta: LedgerDelta {
                    mutations: vec![LedgerDeltaMutation::DetachFromStaff {
                        system_id: 2,
                        staff_id: 20,
                        index: -1,
                        inter_id: 200,
                    }],
                },
                error: Some("post"),
            }),
            ..FakeVisual::default()
        };
        visual.systems.insert(
            2,
            LedgerStageOutcome {
                delta: built_ledger_delta(1200, inter(200, 20, -1)),
                error: Some("system-two"),
            },
        );
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(LedgersStepError::Epilog {
                source: "post",
                system_errors: vec![(2, "system-two")],
            })
        );
        assert!(sheet.systems[0].staves[0].ledgers.is_empty());
        assert_eq!(
            step.visual().calls,
            vec![("filter", 7), ("system", 2), ("system", 1), ("post", 7)]
        );
        assert_eq!(
            sheet.mutations.last(),
            Some(&LedgersMutation::StaffLedgerDetached {
                staff_id: 20,
                index: -1,
                inter_id: 200,
            })
        );
    }

    #[test]
    fn contract_failure_keeps_only_the_valid_delta_prefix() {
        let mut visual = FakeVisual {
            filter: Some(Ok(sections())),
            ..FakeVisual::default()
        };
        let duplicate = NeutralLedgerGlyph {
            id: 88,
            section_ids: vec![20],
        };
        visual.systems.insert(
            2,
            LedgerStageOutcome::success(LedgerDelta {
                mutations: vec![
                    LedgerDeltaMutation::RegisterGlyph(duplicate.clone()),
                    LedgerDeltaMutation::RegisterGlyph(duplicate),
                ],
            }),
        );
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(LedgersStepError::Contract(
                LedgersContractError::DuplicateGlyph(88)
            ))
        );
        assert_eq!(sheet.registered_glyphs.len(), 1);
        assert_eq!(
            sheet.mutations,
            vec![
                LedgersMutation::BuilderCreated { system_id: 2 },
                LedgersMutation::GlyphRegistered {
                    system_id: 2,
                    glyph_id: 88,
                },
            ]
        );
        assert_eq!(step.visual().calls, vec![("filter", 7), ("system", 2)]);
    }

    #[test]
    fn ledger_line_failure_retains_prior_staff_completion_prefix() {
        let mut visual = FakeVisual {
            filter: Some(Ok(sections())),
            ..FakeVisual::default()
        };
        visual.lines.insert(21, Err("line-21"));
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(LedgersStepError::Epilog {
                source: "line-21",
                system_errors: Vec::new(),
            })
        );
        assert_eq!(
            sheet.mutations.last(),
            Some(&LedgersMutation::StaffLedgerLinesBuilt {
                system_id: 2,
                staff_id: 20,
            })
        );
        assert_eq!(
            step.visual().calls,
            vec![
                ("filter", 7),
                ("system", 2),
                ("system", 1),
                ("post", 7),
                ("lines", 20),
                ("lines", 21),
            ]
        );
    }

    #[test]
    fn section_map_requires_exact_system_ownership_before_any_builder() {
        let visual = FakeVisual {
            filter: Some(Ok(vec![LedgerSystemSections {
                system_id: 2,
                sections: vec![NeutralLedgerSection { id: 20 }],
            }])),
            ..FakeVisual::default()
        };
        let mut step = HeadlessLedgersStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(LedgersStepError::Contract(
                LedgersContractError::MissingSectionOwner(1)
            ))
        );
        assert!(sheet.mutations.is_empty());
        assert_eq!(step.visual().calls, vec![("filter", 7)]);
    }
}
