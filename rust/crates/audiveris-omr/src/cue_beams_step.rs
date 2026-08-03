// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `CueBeamsStep`.
//!
//! Cue aggregate detection, morphology, beam grading, and linking remain one
//! injected visual/geometric seam. This module owns the two prolog gates, the
//! shared vertical SPOT_LAG and spot list, system traversal, SIG/index/member
//! mutations, checked continuation, fatal prefixes, and Java's empty epilog.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueSpotOrientation {
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralCueSpotSection {
    pub id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueGlyph {
    pub id: usize,
    pub beam_spot_group: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCueInterKind {
    SmallHead,
    Stem,
    SmallBeam,
    BeamGroup,
    Other(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueInter {
    pub id: usize,
    pub kind: NeutralCueInterKind,
    pub glyph_id: Option<usize>,
    /// Beam-group membership in Java insertion order.
    pub member_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCueRelationKind {
    BeamStem,
    HeadStem,
    Exclusion,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralCueRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralCueRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueSystem {
    pub id: usize,
    pub inters: Vec<NeutralCueInter>,
    pub relations: Vec<NeutralCueRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueBeamsSheet {
    pub id: usize,
    pub small_heads_enabled: bool,
    pub small_beam_scale: Option<i32>,
    pub systems: Vec<NeutralCueSystem>,
    pub registered_glyphs: Vec<NeutralCueGlyph>,
    pub next_glyph_id: usize,
    pub live_inter_ids: Vec<usize>,
    pub retired_inter_ids: Vec<usize>,
    pub next_inter_id: usize,
    pub mutations: Vec<CueBeamsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsContext {
    pub spot_orientation: CueSpotOrientation,
    /// Java shared `List<Glyph> spots`, in append order.
    pub spot_glyph_ids: Vec<usize>,
    /// Java shared BasicLag entity order.
    pub spot_lag_sections: Vec<NeutralCueSpotSection>,
}

impl Default for CueBeamsContext {
    fn default() -> Self {
        Self {
            spot_orientation: CueSpotOrientation::Vertical,
            spot_glyph_ids: Vec::new(),
            spot_lag_sections: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsMutation {
    GlyphRegistered {
        system_id: usize,
        glyph_id: usize,
    },
    SpotAppended {
        system_id: usize,
        glyph_id: usize,
    },
    SpotLagSectionAdded {
        system_id: usize,
        section_id: usize,
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
        relation: NeutralCueRelation,
    },
    RelationRemoved {
        system_id: usize,
        relation: NeutralCueRelation,
    },
    MemberAttached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    MemberDetached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueBeamsDelta {
    /// Exact successful Java mutation prefix before failure.
    pub mutations: Vec<CueBeamsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsDeltaMutation {
    RegisterGlyph(NeutralCueGlyph),
    AppendSpot(usize),
    AddSpotLagSection(NeutralCueSpotSection),
    AddInter(NeutralCueInter),
    RemoveInter(usize),
    AddRelation(NeutralCueRelation),
    RemoveRelation(usize),
    AttachMember { owner_id: usize, member_id: usize },
    DetachMember { owner_id: usize, member_id: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsFailure<VisualError> {
    Checked(VisualError),
    Fatal(VisualError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsSystemOutcome<VisualError> {
    pub delta: CueBeamsDelta,
    pub failure: Option<CueBeamsFailure<VisualError>>,
}

impl<VisualError> CueBeamsSystemOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: CueBeamsDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CueBeamsSystemInput<'a> {
    pub sheet: &'a NeutralCueBeamsSheet,
    pub system_id: usize,
    /// Snapshot of the context accumulated by all prior systems.
    pub context: &'a CueBeamsContext,
    /// Java `cueBeamRatio`, resolved here for the visual collaborator.
    pub cue_beam_ratio: f64,
}

/// First unavailable dependency: `BeamsBuilder.buildCueBeams`.
pub trait VisualCueBeams {
    type Error;

    fn build_system_cue_beams(
        &mut self,
        input: CueBeamsSystemInput<'_>,
    ) -> CueBeamsSystemOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueBeamsSkipReason {
    SmallHeadsDisabled,
    SmallBeamScaleAlreadySet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsReport<VisualError> {
    pub skip_reason: Option<CueBeamsSkipReason>,
    pub system_errors: Vec<(usize, VisualError)>,
    /// Active Java context at end of the empty epilog; `None` when skipped.
    pub context: Option<CueBeamsContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsStepError<VisualError> {
    FatalSystem {
        system_id: usize,
        source: VisualError,
        checked_errors: Vec<(usize, VisualError)>,
        context: CueBeamsContext,
    },
    Contract(CueBeamsContractError),
}

impl<VisualError: fmt::Display> fmt::Display for CueBeamsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FatalSystem {
                system_id, source, ..
            } => write!(formatter, "cue beams system {system_id} failed: {source}"),
            Self::Contract(source) => write!(formatter, "cue beams contract failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for CueBeamsStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FatalSystem { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsContractError {
    DuplicateSystem(usize),
    DuplicateGlyph(usize),
    InvalidGlyphAllocator {
        expected: usize,
        actual: usize,
    },
    DuplicateInter(usize),
    InvalidInterAllocator {
        expected: usize,
        actual: usize,
    },
    InvalidLiveInterIndex,
    InvalidRetiredInter(usize),
    UnknownSystem(usize),
    UnknownGlyph(usize),
    SpotGlyphMissingGroup(usize),
    DuplicateSpot(usize),
    UnknownSpot(usize),
    DuplicateSpotLagSection(usize),
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
    UnknownRelationEndpoint(usize),
    DuplicateMember {
        owner_id: usize,
        member_id: usize,
    },
    UnknownMember {
        owner_id: usize,
        member_id: usize,
    },
}

impl fmt::Display for CueBeamsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for CueBeamsContractError {}

pub struct HeadlessCueBeamsStep<Visual> {
    visual: Visual,
    cue_beam_ratio: f64,
}

impl<Visual> HeadlessCueBeamsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            cue_beam_ratio: 0.6,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualCueBeams> HeadlessCueBeamsStep<Visual> {
    /// Java `AbstractSystemStep.doit`: gated prolog, every system, empty epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralCueBeamsSheet,
    ) -> Result<CueBeamsReport<Visual::Error>, CueBeamsStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(CueBeamsStepError::Contract)?;
        let skip_reason = if !sheet.small_heads_enabled {
            Some(CueBeamsSkipReason::SmallHeadsDisabled)
        } else if sheet.small_beam_scale.is_some() {
            Some(CueBeamsSkipReason::SmallBeamScaleAlreadySet)
        } else {
            None
        };
        if let Some(skip_reason) = skip_reason {
            // AbstractSystemStep still traverses systems, but doSystem is a no-op
            // for null context and the inherited epilog is empty.
            return Ok(CueBeamsReport {
                skip_reason: Some(skip_reason),
                system_errors: Vec::new(),
                context: None,
            });
        }

        let mut context = CueBeamsContext::default();
        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.build_system_cue_beams(CueBeamsSystemInput {
                sheet,
                system_id,
                context: &context,
                cue_beam_ratio: self.cue_beam_ratio,
            });
            apply_delta(sheet, system_id, &mut context, outcome.delta)
                .map_err(CueBeamsStepError::Contract)?;
            match outcome.failure {
                None => {}
                Some(CueBeamsFailure::Checked(source)) => {
                    sheet
                        .mutations
                        .push(CueBeamsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                }
                Some(CueBeamsFailure::Fatal(source)) => {
                    return Err(CueBeamsStepError::FatalSystem {
                        system_id,
                        source,
                        checked_errors: system_errors,
                        context,
                    });
                }
            }
        }

        Ok(CueBeamsReport {
            skip_reason: None,
            system_errors,
            context: Some(context),
        })
    }
}

fn apply_delta(
    sheet: &mut NeutralCueBeamsSheet,
    system_id: usize,
    context: &mut CueBeamsContext,
    delta: CueBeamsDelta,
) -> Result<(), CueBeamsContractError> {
    let system_index = sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(CueBeamsContractError::UnknownSystem(system_id))?;
    for mutation in delta.mutations {
        match mutation {
            CueBeamsDeltaMutation::RegisterGlyph(glyph) => {
                if glyph.id != sheet.next_glyph_id {
                    return Err(CueBeamsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    });
                }
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|existing| existing.id == glyph.id)
                {
                    return Err(CueBeamsContractError::DuplicateGlyph(glyph.id));
                }
                sheet.next_glyph_id = sheet.next_glyph_id.checked_add(1).ok_or(
                    CueBeamsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    },
                )?;
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(CueBeamsMutation::GlyphRegistered {
                    system_id,
                    glyph_id,
                });
            }
            CueBeamsDeltaMutation::AppendSpot(glyph_id) => {
                let glyph = sheet
                    .registered_glyphs
                    .iter()
                    .find(|glyph| glyph.id == glyph_id)
                    .ok_or(CueBeamsContractError::UnknownGlyph(glyph_id))?;
                if !glyph.beam_spot_group {
                    return Err(CueBeamsContractError::SpotGlyphMissingGroup(glyph_id));
                }
                if context.spot_glyph_ids.contains(&glyph_id) {
                    return Err(CueBeamsContractError::DuplicateSpot(glyph_id));
                }
                context.spot_glyph_ids.push(glyph_id);
                sheet.mutations.push(CueBeamsMutation::SpotAppended {
                    system_id,
                    glyph_id,
                });
            }
            CueBeamsDeltaMutation::AddSpotLagSection(section) => {
                if context
                    .spot_lag_sections
                    .iter()
                    .any(|existing| existing.id == section.id)
                {
                    return Err(CueBeamsContractError::DuplicateSpotLagSection(section.id));
                }
                let section_id = section.id;
                context.spot_lag_sections.push(section);
                sheet.mutations.push(CueBeamsMutation::SpotLagSectionAdded {
                    system_id,
                    section_id,
                });
            }
            CueBeamsDeltaMutation::AddInter(inter) => {
                if inter.id != sheet.next_inter_id {
                    return Err(CueBeamsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    });
                }
                if sheet.live_inter_ids.contains(&inter.id)
                    || sheet.retired_inter_ids.contains(&inter.id)
                {
                    return Err(CueBeamsContractError::DuplicateInter(inter.id));
                }
                if let Some(glyph_id) = inter.glyph_id
                    && !sheet
                        .registered_glyphs
                        .iter()
                        .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(CueBeamsContractError::UnknownGlyph(glyph_id));
                }
                let mut members = BTreeSet::new();
                for &member_id in &inter.member_ids {
                    if !members.insert(member_id) {
                        return Err(CueBeamsContractError::DuplicateMember {
                            owner_id: inter.id,
                            member_id,
                        });
                    }
                    require_inter(&sheet.systems[system_index], member_id)?;
                }
                let member_ids = inter.member_ids.clone();
                let inter_id = inter.id;
                sheet.next_inter_id = sheet.next_inter_id.checked_add(1).ok_or(
                    CueBeamsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    },
                )?;
                sheet.systems[system_index].inters.push(inter);
                sheet.live_inter_ids.push(inter_id);
                sheet.mutations.push(CueBeamsMutation::InterAdded {
                    system_id,
                    inter_id,
                });
                for member_id in member_ids {
                    sheet.mutations.push(CueBeamsMutation::MemberAttached {
                        system_id,
                        owner_id: inter_id,
                        member_id,
                    });
                }
            }
            CueBeamsDeltaMutation::RemoveInter(inter_id) => {
                remove_inter(sheet, system_index, inter_id)?;
            }
            CueBeamsDeltaMutation::AddRelation(relation) => {
                for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                    require_inter(&sheet.systems[system_index], endpoint)?;
                }
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(CueBeamsContractError::DuplicateRelation {
                        system_id,
                        relation_id: relation.id,
                    });
                }
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(CueBeamsMutation::RelationAdded {
                    system_id,
                    relation,
                });
            }
            CueBeamsDeltaMutation::RemoveRelation(relation_id) => {
                let relation_index = sheet.systems[system_index]
                    .relations
                    .iter()
                    .position(|relation| relation.id == relation_id)
                    .ok_or(CueBeamsContractError::UnknownRelation {
                        system_id,
                        relation_id,
                    })?;
                let relation = sheet.systems[system_index].relations.remove(relation_index);
                sheet.mutations.push(CueBeamsMutation::RelationRemoved {
                    system_id,
                    relation,
                });
            }
            CueBeamsDeltaMutation::AttachMember {
                owner_id,
                member_id,
            } => {
                require_inter(&sheet.systems[system_index], member_id)?;
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(CueBeamsContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                if owner.member_ids.contains(&member_id) {
                    return Err(CueBeamsContractError::DuplicateMember {
                        owner_id,
                        member_id,
                    });
                }
                owner.member_ids.push(member_id);
                sheet.mutations.push(CueBeamsMutation::MemberAttached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
            CueBeamsDeltaMutation::DetachMember {
                owner_id,
                member_id,
            } => {
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(CueBeamsContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                let member_index = owner
                    .member_ids
                    .iter()
                    .position(|id| *id == member_id)
                    .ok_or(CueBeamsContractError::UnknownMember {
                        owner_id,
                        member_id,
                    })?;
                owner.member_ids.remove(member_index);
                sheet.mutations.push(CueBeamsMutation::MemberDetached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
        }
    }
    Ok(())
}

fn require_inter(system: &NeutralCueSystem, inter_id: usize) -> Result<(), CueBeamsContractError> {
    system
        .inters
        .iter()
        .any(|inter| inter.id == inter_id)
        .then_some(())
        .ok_or(CueBeamsContractError::UnknownInter {
            system_id: system.id,
            inter_id,
        })
}

fn remove_inter(
    sheet: &mut NeutralCueBeamsSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), CueBeamsContractError> {
    let system_id = sheet.systems[system_index].id;
    let inter_index = sheet.systems[system_index]
        .inters
        .iter()
        .position(|inter| inter.id == inter_id)
        .ok_or(CueBeamsContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    let relations = sheet.systems[system_index]
        .relations
        .iter()
        .copied()
        .filter(|relation| {
            relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
        })
        .collect::<Vec<_>>();
    for relation in relations {
        let relation_index = sheet.systems[system_index]
            .relations
            .iter()
            .position(|candidate| *candidate == relation)
            .expect("relation snapshot remains live");
        sheet.systems[system_index].relations.remove(relation_index);
        sheet.mutations.push(CueBeamsMutation::RelationRemoved {
            system_id,
            relation,
        });
    }
    let owner_ids = sheet.systems[system_index]
        .inters
        .iter()
        .filter(|owner| owner.member_ids.contains(&inter_id))
        .map(|owner| owner.id)
        .collect::<Vec<_>>();
    for owner_id in owner_ids {
        let owner = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|owner| owner.id == owner_id)
            .expect("owner snapshot remains live");
        owner.member_ids.retain(|id| *id != inter_id);
        sheet.mutations.push(CueBeamsMutation::MemberDetached {
            system_id,
            owner_id,
            member_id: inter_id,
        });
    }
    for member_id in sheet.systems[system_index].inters[inter_index]
        .member_ids
        .clone()
    {
        sheet.mutations.push(CueBeamsMutation::MemberDetached {
            system_id,
            owner_id: inter_id,
            member_id,
        });
    }
    sheet.systems[system_index].inters.remove(inter_index);
    let live_index = sheet
        .live_inter_ids
        .iter()
        .position(|id| *id == inter_id)
        .ok_or(CueBeamsContractError::InvalidLiveInterIndex)?;
    sheet.live_inter_ids.remove(live_index);
    sheet.retired_inter_ids.push(inter_id);
    sheet.mutations.push(CueBeamsMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn validate_sheet(sheet: &NeutralCueBeamsSheet) -> Result<(), CueBeamsContractError> {
    let mut glyph_ids = BTreeSet::new();
    for glyph in &sheet.registered_glyphs {
        if !glyph_ids.insert(glyph.id) {
            return Err(CueBeamsContractError::DuplicateGlyph(glyph.id));
        }
        if glyph.id >= sheet.next_glyph_id {
            return Err(CueBeamsContractError::InvalidGlyphAllocator {
                expected: sheet.next_glyph_id,
                actual: glyph.id,
            });
        }
    }
    let mut system_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(CueBeamsContractError::DuplicateSystem(system.id));
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(CueBeamsContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(CueBeamsContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let Some(glyph_id) = inter.glyph_id
                && !glyph_ids.contains(&glyph_id)
            {
                return Err(CueBeamsContractError::UnknownGlyph(glyph_id));
            }
            let mut members = BTreeSet::new();
            for &member_id in &inter.member_ids {
                if !members.insert(member_id) {
                    return Err(CueBeamsContractError::DuplicateMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
                if !system.inters.iter().any(|member| member.id == member_id) {
                    return Err(CueBeamsContractError::UnknownMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
            }
        }
        let mut relation_ids = BTreeSet::new();
        for relation in &system.relations {
            if !relation_ids.insert(relation.id) {
                return Err(CueBeamsContractError::DuplicateRelation {
                    system_id: system.id,
                    relation_id: relation.id,
                });
            }
            for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                if !system.inters.iter().any(|inter| inter.id == endpoint) {
                    return Err(CueBeamsContractError::UnknownRelationEndpoint(endpoint));
                }
            }
        }
    }
    let live = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live.len() != sheet.live_inter_ids.len() || live != inter_ids {
        return Err(CueBeamsContractError::InvalidLiveInterIndex);
    }
    let mut retired = BTreeSet::new();
    for &inter_id in &sheet.retired_inter_ids {
        if inter_id >= sheet.next_inter_id
            || inter_ids.contains(&inter_id)
            || !retired.insert(inter_id)
        {
            return Err(CueBeamsContractError::InvalidRetiredInter(inter_id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeCueBeams {
        calls: Vec<(usize, Vec<usize>, Vec<usize>, f64)>,
        outcomes: BTreeMap<usize, CueBeamsSystemOutcome<&'static str>>,
    }

    impl VisualCueBeams for FakeCueBeams {
        type Error = &'static str;

        fn build_system_cue_beams(
            &mut self,
            input: CueBeamsSystemInput<'_>,
        ) -> CueBeamsSystemOutcome<Self::Error> {
            self.calls.push((
                input.system_id,
                input.context.spot_glyph_ids.clone(),
                input
                    .context
                    .spot_lag_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
                input.cue_beam_ratio,
            ));
            self.outcomes
                .remove(&input.system_id)
                .unwrap_or_else(|| CueBeamsSystemOutcome::success(CueBeamsDelta::default()))
        }
    }

    fn inter(id: usize, kind: NeutralCueInterKind) -> NeutralCueInter {
        NeutralCueInter {
            id,
            kind,
            glyph_id: None,
            member_ids: Vec::new(),
        }
    }

    fn sheet() -> NeutralCueBeamsSheet {
        NeutralCueBeamsSheet {
            id: 3,
            small_heads_enabled: true,
            small_beam_scale: None,
            systems: vec![
                NeutralCueSystem {
                    id: 2,
                    inters: vec![inter(1, NeutralCueInterKind::SmallHead)],
                    relations: Vec::new(),
                },
                NeutralCueSystem {
                    id: 1,
                    inters: vec![inter(2, NeutralCueInterKind::Stem)],
                    relations: Vec::new(),
                },
            ],
            registered_glyphs: Vec::new(),
            next_glyph_id: 1,
            live_inter_ids: vec![1, 2],
            retired_inter_ids: Vec::new(),
            next_inter_id: 3,
            mutations: Vec::new(),
        }
    }

    fn first_system_delta() -> CueBeamsDelta {
        CueBeamsDelta {
            mutations: vec![
                CueBeamsDeltaMutation::RegisterGlyph(NeutralCueGlyph {
                    id: 1,
                    beam_spot_group: true,
                }),
                CueBeamsDeltaMutation::AppendSpot(1),
                CueBeamsDeltaMutation::AddSpotLagSection(NeutralCueSpotSection { id: 9 }),
                CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                    id: 3,
                    kind: NeutralCueInterKind::SmallBeam,
                    glyph_id: Some(1),
                    member_ids: Vec::new(),
                }),
                CueBeamsDeltaMutation::AddRelation(NeutralCueRelation {
                    id: 1,
                    source_inter_id: 3,
                    target_inter_id: 1,
                    kind: NeutralCueRelationKind::BeamStem,
                }),
            ],
        }
    }

    #[test]
    fn exact_active_lifecycle_shares_vertical_context_across_systems() {
        let mut visual = FakeCueBeams::default();
        visual
            .outcomes
            .insert(2, CueBeamsSystemOutcome::success(first_system_delta()));
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            step.visual().calls,
            [(2, vec![], vec![], 0.6), (1, vec![1], vec![9], 0.6)]
        );
        assert_eq!(
            report.context,
            Some(CueBeamsContext {
                spot_orientation: CueSpotOrientation::Vertical,
                spot_glyph_ids: vec![1],
                spot_lag_sections: vec![NeutralCueSpotSection { id: 9 }],
            })
        );
        assert_eq!(sheet.live_inter_ids, [1, 2, 3]);
        assert_eq!(sheet.registered_glyphs.len(), 1);
    }

    #[test]
    fn skip_gate_priority_matches_java_and_has_no_visual_or_context() {
        let mut sheet = sheet();
        sheet.small_heads_enabled = false;
        sheet.small_beam_scale = Some(3);
        let mut step = HeadlessCueBeamsStep::new(FakeCueBeams::default());

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.skip_reason,
            Some(CueBeamsSkipReason::SmallHeadsDisabled)
        );
        assert!(report.context.is_none());
        assert!(step.visual().calls.is_empty());
        assert!(sheet.mutations.is_empty());
    }

    #[test]
    fn existing_small_beam_scale_is_second_no_op_gate() {
        let mut sheet = sheet();
        sheet.small_beam_scale = Some(3);
        let mut step = HeadlessCueBeamsStep::new(FakeCueBeams::default());

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.skip_reason,
            Some(CueBeamsSkipReason::SmallBeamScaleAlreadySet)
        );
        assert!(step.visual().calls.is_empty());
    }

    #[test]
    fn checked_failure_keeps_registered_spot_prefix_and_continues() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome {
                delta: first_system_delta(),
                failure: Some(CueBeamsFailure::Checked("checked")),
            },
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, [(2, "checked")]);
        assert_eq!(step.visual().calls[1].1, [1]);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
        assert!(
            sheet
                .mutations
                .contains(&CueBeamsMutation::SystemFailed { system_id: 2 })
        );
    }

    #[test]
    fn fatal_failure_returns_shared_context_and_skips_later_system() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome {
                delta: CueBeamsDelta {
                    mutations: first_system_delta().mutations[..3].to_vec(),
                },
                failure: Some(CueBeamsFailure::Fatal("fatal")),
            },
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(CueBeamsStepError::FatalSystem {
                system_id: 2,
                source: "fatal",
                checked_errors: Vec::new(),
                context: CueBeamsContext {
                    spot_orientation: CueSpotOrientation::Vertical,
                    spot_glyph_ids: vec![1],
                    spot_lag_sections: vec![NeutralCueSpotSection { id: 9 }],
                },
            })
        );
        assert_eq!(step.visual().calls.len(), 1);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
    }

    #[test]
    fn removal_preserves_allocator_hole_and_cascades_relation_and_group_ownership() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome::success(CueBeamsDelta {
                mutations: vec![
                    CueBeamsDeltaMutation::RegisterGlyph(NeutralCueGlyph {
                        id: 1,
                        beam_spot_group: true,
                    }),
                    CueBeamsDeltaMutation::AppendSpot(1),
                    CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                        id: 3,
                        kind: NeutralCueInterKind::SmallBeam,
                        glyph_id: Some(1),
                        member_ids: Vec::new(),
                    }),
                    CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                        id: 4,
                        kind: NeutralCueInterKind::BeamGroup,
                        glyph_id: None,
                        member_ids: vec![3],
                    }),
                    CueBeamsDeltaMutation::AddRelation(NeutralCueRelation {
                        id: 1,
                        source_inter_id: 3,
                        target_inter_id: 1,
                        kind: NeutralCueRelationKind::BeamStem,
                    }),
                    CueBeamsDeltaMutation::RemoveInter(3),
                ],
            }),
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.retired_inter_ids, [3]);
        assert_eq!(sheet.next_inter_id, 5);
        assert_eq!(sheet.live_inter_ids, [1, 2, 4]);
        assert!(sheet.systems[0].relations.is_empty());
        assert!(sheet.systems[0].inters[1].member_ids.is_empty());
        assert!(sheet.mutations.iter().any(|mutation| matches!(
            mutation,
            CueBeamsMutation::MemberDetached {
                owner_id: 4,
                member_id: 3,
                ..
            }
        )));
    }
}
