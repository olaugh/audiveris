// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `StemsStep`.
//!
//! Legacy beam-group upgrade is a neutral deterministic precondition. The
//! first injected geometry is `StemsRetriever.process`; after every system has
//! been attempted, Java revisits systems for `finalizeBeams` and contextualizes
//! each successful SIG immediately.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

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

/// One Java `GlyphGroup.VERTICAL_SEED` candidate. Source order is retained for
/// equal abscissae; `bar_overlap` is the geometric result of
/// `purgeNoStemSeeds`, before linker inspection starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStemSeed {
    pub glyph_id: usize,
    pub x: i32,
    pub bar_overlap: bool,
}

/// Geometry-independent fields used by Java's two beam orderings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStemBeam {
    pub inter_id: usize,
    pub x: i32,
    pub width: i32,
}

/// Geometry-independent fields used by Java's two head orderings and final
/// abnormal-head pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemHead {
    pub inter_id: usize,
    pub x: i32,
    pub grade: f64,
    pub requires_stem: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeStemHeadSide {
    Left,
    Right,
}

/// The relation measurements consumed by `HeadStemsCleaner`. Link geometry
/// supplies these measurements; contribution pruning and mutation order are
/// native. `partition` is Java `SIGraph.getPartitions` pre-resolved from graph
/// exclusions, since that partitioning depends on the full SIG implementation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadStemLink {
    pub relation_id: usize,
    pub head_id: usize,
    pub stem_id: usize,
    pub partition: usize,
    pub dy: f64,
    pub head_side: NativeStemHeadSide,
    pub stem_grade: f64,
    pub target_ratio: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemLine {
    pub start: NativeStemPoint,
    pub stop: NativeStemPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeStemVerticalSide {
    Top,
    Bottom,
}

impl NativeStemVerticalSide {
    const fn direction(self) -> i32 {
        match self {
            Self::Top => -1,
            Self::Bottom => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBeamPortion {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStemGeometrySiteKind {
    BeamSide(NativeStemHeadSide),
    BeamStump {
        source_order: usize,
    },
    HeadCorner {
        horizontal: NativeStemHeadSide,
        vertical: NativeStemVerticalSide,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemGeometrySite {
    pub owner_inter_id: usize,
    pub kind: NativeStemGeometrySiteKind,
    pub reference: NativeStemPoint,
    /// Head lookup starts with `out_point -> in_point`. Beam lookup derives
    /// these from `beam_border` and `half_beam_lookup_dx`.
    pub out_point: Option<NativeStemPoint>,
    pub in_point: Option<NativeStemPoint>,
    pub y_limit: f64,
    pub profile: u8,
    pub beam_median: Option<NativeStemLine>,
    pub beam_border: Option<NativeStemLine>,
    pub beam_height: f64,
    pub head_bounds: Option<NativeStemRect>,
    pub stump_bounds: Option<NativeStemRect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemGeometryCandidateLine {
    pub candidate_id: usize,
    pub glyph_id: Option<usize>,
    pub median: NativeStemLine,
    pub bounds: NativeStemRect,
    /// Java collection/SIG insertion order, used for exact stable ties.
    pub source_order: usize,
    pub existing_stem: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsLinkGeometrySource {
    pub system_id: usize,
    pub interline: f64,
    pub stem_thickness: f64,
    pub sheet_slope: f64,
    pub slope_margin: f64,
    pub half_beam_lookup_dx: f64,
    pub max_beam_seed_dy_ratio: f64,
    pub sites: Vec<NativeStemGeometrySite>,
    pub candidates: Vec<NativeStemGeometryCandidateLine>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStemRelationGeometryKind {
    BeamStem { portion: NativeBeamPortion },
    HeadStem { side: NativeStemHeadSide },
}

/// Accepted candidate evidence. The injected collaborator now decides only
/// classifier/materializer policy; it no longer recomputes lookup geometry or
/// relation impacts.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemLinkCandidate {
    pub owner_inter_id: usize,
    pub candidate_id: usize,
    pub glyph_id: Option<usize>,
    pub site: NativeStemGeometrySiteKind,
    pub lookup_polygon: [NativeStemPoint; 4],
    pub relation: NativeStemRelationGeometryKind,
    pub dx: f64,
    pub dy: f64,
    pub x_impact: f64,
    pub y_impact: f64,
    pub grade: f64,
    pub extension: NativeStemPoint,
    pub distance_to_theoretical: f64,
    pub source_order: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSystemSource {
    pub system_id: usize,
    pub seeds: Vec<NativeStemSeed>,
    pub beams: Vec<NativeStemBeam>,
    pub heads: Vec<NativeStemHead>,
    /// Existing SIG relations in graph iteration order.
    pub head_stem_links: Vec<NativeHeadStemLink>,
    /// Optional native lookup/impact kernel input. Absence keeps the previous
    /// dependency-injected behavior and supplies an empty candidate slice.
    pub link_geometry: Option<NativeStemsLinkGeometrySource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStemsPhase {
    InspectBeam,
    InspectHead,
    LinkBeamSides,
    LinkBeamStumps,
    LinkHeadSides,
    RelinkHeadSides,
    CanonicalHeadShare,
    FinalizeBeams,
}

impl fmt::Display for NativeStemsPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InspectBeam => "beam inspection",
            Self::InspectHead => "head inspection",
            Self::LinkBeamSides => "beam-side linking",
            Self::LinkBeamStumps => "beam-stump linking",
            Self::LinkHeadSides => "head linking phase 1",
            Self::RelinkHeadSides => "head linking phase 2",
            Self::CanonicalHeadShare => "head-stem cleanup",
            Self::FinalizeBeams => "beam finalization",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsDecision<Value, VisualError> {
    pub value: Value,
    pub delta: StemsDelta,
    /// A Java-visible mutation prefix can accompany a checked failure.
    pub error: Option<VisualError>,
}

impl<Value, VisualError> NativeStemsDecision<Value, VisualError> {
    #[must_use]
    pub fn success(value: Value) -> Self {
        Self {
            value,
            delta: StemsDelta::default(),
            error: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeStemLinkResult {
    pub linked: bool,
    /// Measurements for HeadStem relations added by this call, in relation
    /// insertion order.
    pub head_stem_links: Vec<NativeHeadStemLink>,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeStemCandidateInput<'a, Candidate> {
    pub sheet_id: usize,
    pub system: &'a NeutralStemsSystem,
    pub seeds: &'a [NativeStemSeed],
    pub beams: &'a [NativeStemBeam],
    pub heads: &'a [NativeStemHead],
    pub link_candidates: &'a [NativeStemLinkCandidate],
    pub candidate: Candidate,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeHeadLinkInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralStemsSystem,
    pub seeds: &'a [NativeStemSeed],
    pub beams: &'a [NativeStemBeam],
    pub heads: &'a [NativeStemHead],
    pub link_candidates: &'a [NativeStemLinkCandidate],
    pub head: NativeStemHead,
    /// False in Java phase 1; true when retrying the unlinked list.
    pub existing_stems_only: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeCanonicalShareInput<'a> {
    pub sheet_id: usize,
    pub system: &'a NeutralStemsSystem,
    pub head: NativeStemHead,
    pub left: NativeHeadStemLink,
    pub right: NativeHeadStemLink,
}

/// Missing `BeamLinker`/`HeadLinker` geometry and stem grading seam. Everything
/// around these calls—including candidate ordering, phase traversal, relation
/// cleanup, abnormal flags, and retained failure prefixes—is native Rust.
pub trait VisualStemLinker {
    type Error;

    fn inspect_beam(
        &mut self,
        input: NativeStemCandidateInput<'_, NativeStemBeam>,
    ) -> NativeStemsDecision<bool, Self::Error>;

    fn inspect_head(
        &mut self,
        input: NativeStemCandidateInput<'_, NativeStemHead>,
    ) -> NativeStemsDecision<(), Self::Error>;

    fn link_beam_sides(
        &mut self,
        input: NativeStemCandidateInput<'_, NativeStemBeam>,
    ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error>;

    fn link_beam_stumps(
        &mut self,
        input: NativeStemCandidateInput<'_, NativeStemBeam>,
    ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error>;

    fn link_head_sides(
        &mut self,
        input: NativeHeadLinkInput<'_>,
    ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error>;

    fn is_canonical_share(
        &mut self,
        input: NativeCanonicalShareInput<'_>,
    ) -> Result<bool, Self::Error>;

    fn finalize_beams(
        &mut self,
        input: StemsSystemInput<'_>,
        seeds: &[NativeStemSeed],
    ) -> StemsStageOutcome<Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsError<VisualError> {
    MissingSystemSource(usize),
    DuplicateSystemSource(usize),
    DuplicateCandidate {
        system_id: usize,
        inter_id: usize,
    },
    MissingCandidate {
        system_id: usize,
        inter_id: usize,
    },
    WrongCandidateKind {
        system_id: usize,
        inter_id: usize,
    },
    InvalidLinkEvidence {
        system_id: usize,
        relation_id: usize,
    },
    Contract {
        phase: NativeStemsPhase,
        source: StemsContractError,
    },
    Visual {
        phase: NativeStemsPhase,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for NativeStemsError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystemSource(id) => {
                write!(formatter, "missing stems source for system {id}")
            }
            Self::DuplicateSystemSource(id) => {
                write!(formatter, "duplicate stems source for system {id}")
            }
            Self::DuplicateCandidate {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "duplicate stem candidate {inter_id} in system {system_id}"
            ),
            Self::MissingCandidate {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "missing stem candidate {inter_id} in system {system_id}"
            ),
            Self::WrongCandidateKind {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "wrong stem candidate kind for {inter_id} in system {system_id}"
            ),
            Self::InvalidLinkEvidence {
                system_id,
                relation_id,
            } => write!(
                formatter,
                "invalid head-stem evidence {relation_id} in system {system_id}"
            ),
            Self::Contract { phase, source } => {
                write!(formatter, "stems {phase} contract failed: {source}")
            }
            Self::Visual { phase, source } => write!(formatter, "stems {phase} failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for NativeStemsError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract { source, .. } => Some(source),
            Self::Visual { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Dependency-light native port of `StemsRetriever`.
pub struct NativeVisualStems<Linker> {
    sources: Vec<NativeStemsSystemSource>,
    linker: Linker,
}

struct PreparedNativeStems {
    seeds: Vec<NativeStemSeed>,
    beams: Vec<NativeStemBeam>,
    heads: Vec<NativeStemHead>,
}

impl<Linker> NativeVisualStems<Linker> {
    #[must_use]
    pub const fn new(sources: Vec<NativeStemsSystemSource>, linker: Linker) -> Self {
        Self { sources, linker }
    }

    #[must_use]
    pub const fn linker(&self) -> &Linker {
        &self.linker
    }
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

impl NativeStemsLinkGeometrySource {
    fn candidates_for(
        &self,
        owner_inter_id: usize,
        phase: NativeStemsPhase,
    ) -> Vec<NativeStemLinkCandidate> {
        let mut accepted = Vec::new();
        let mut sites = self
            .sites
            .iter()
            .filter(|site| {
                site.owner_inter_id == owner_inter_id
                    && matches!(
                        (phase, site.kind),
                        (
                            NativeStemsPhase::LinkBeamSides,
                            NativeStemGeometrySiteKind::BeamSide(_)
                        ) | (
                            NativeStemsPhase::LinkBeamStumps,
                            NativeStemGeometrySiteKind::BeamStump { .. },
                        ) | (
                            NativeStemsPhase::LinkHeadSides | NativeStemsPhase::RelinkHeadSides,
                            NativeStemGeometrySiteKind::HeadCorner { .. },
                        )
                    )
            })
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| stem_geometry_site_order(site.kind));
        for site in sites {
            let Some(polygon) = lookup_polygon(self, site) else {
                continue;
            };
            let theoretical = theoretical_line(self.sheet_slope, site.reference, site.y_limit);
            let mut site_accepted = Vec::new();
            for candidate in self.candidates.iter().filter(|candidate| match phase {
                NativeStemsPhase::RelinkHeadSides => candidate.existing_stem,
                NativeStemsPhase::LinkHeadSides => !candidate.existing_stem,
                NativeStemsPhase::LinkBeamSides | NativeStemsPhase::LinkBeamStumps => true,
                _ => false,
            }) {
                if !polygon_intersects_rect(polygon, candidate.bounds) {
                    continue;
                }
                let evidence = match site.kind {
                    NativeStemGeometrySiteKind::BeamSide(_)
                    | NativeStemGeometrySiteKind::BeamStump { .. } => {
                        beam_relation_evidence(self, site, *candidate, polygon, theoretical)
                    }
                    NativeStemGeometrySiteKind::HeadCorner { .. } => {
                        head_relation_evidence(self, site, *candidate, polygon, theoretical)
                    }
                };
                if let Some(evidence) = evidence {
                    site_accepted.push(evidence);
                }
            }
            // Java traversal keeps site side/stump/corner order. Within one
            // site, distance order is stable and exact ties retain source/SIG
            // insertion order.
            site_accepted.sort_by(|left, right| {
                left.distance_to_theoretical
                    .total_cmp(&right.distance_to_theoretical)
                    .then(left.source_order.cmp(&right.source_order))
            });
            accepted.extend(site_accepted);
        }
        accepted
    }
}

fn stem_geometry_site_order(kind: NativeStemGeometrySiteKind) -> (u8, usize) {
    match kind {
        NativeStemGeometrySiteKind::BeamSide(NativeStemHeadSide::Left) => (0, 0),
        NativeStemGeometrySiteKind::BeamSide(NativeStemHeadSide::Right) => (0, 1),
        NativeStemGeometrySiteKind::BeamStump { source_order } => (1, source_order),
        NativeStemGeometrySiteKind::HeadCorner {
            horizontal,
            vertical,
        } => {
            let horizontal = match horizontal {
                NativeStemHeadSide::Left => 0,
                NativeStemHeadSide::Right => 2,
            };
            let vertical = match vertical {
                NativeStemVerticalSide::Top => 0,
                NativeStemVerticalSide::Bottom => 1,
            };
            (2, horizontal + vertical)
        }
    }
}

fn lookup_polygon(
    geometry: &NativeStemsLinkGeometrySource,
    site: &NativeStemGeometrySite,
) -> Option<[NativeStemPoint; 4]> {
    let slope = -geometry.sheet_slope;
    match site.kind {
        NativeStemGeometrySiteKind::BeamSide(_) | NativeStemGeometrySiteKind::BeamStump { .. } => {
            let border = site.beam_border?;
            let y_dir = beam_site_vertical_direction(site)?;
            let left_x = site.reference.x - geometry.half_beam_lookup_dx;
            let right_x = site.reference.x + geometry.half_beam_lookup_dx;
            let y_offset = f64::from(y_dir)
                * geometry.max_beam_seed_dy_ratio
                * geometry.interline
                * beam_y_gap_max(site.profile);
            let left = NativeStemPoint {
                x: left_x,
                y: line_y_at_x(border, left_x)? + y_offset,
            };
            let right = NativeStemPoint {
                x: right_x,
                y: line_y_at_x(border, right_x)? + y_offset,
            };
            let dy = site.y_limit - site.reference.y;
            let d_slope = f64::from(y_dir) * geometry.slope_margin;
            Some([
                left,
                right,
                NativeStemPoint {
                    x: right.x + ((slope + d_slope) * dy),
                    y: site.y_limit,
                },
                NativeStemPoint {
                    x: left.x + ((slope - d_slope) * dy),
                    y: site.y_limit,
                },
            ])
        }
        NativeStemGeometrySiteKind::HeadCorner {
            horizontal,
            vertical,
        } => {
            let out_point = site.out_point?;
            let in_point = site.in_point?;
            let x_dir = match horizontal {
                NativeStemHeadSide::Left => -1,
                NativeStemHeadSide::Right => 1,
            };
            let y_dir = vertical.direction();
            let d_slope = f64::from(x_dir * y_dir) * geometry.slope_margin;
            let dy = site.y_limit - out_point.y;
            Some([
                out_point,
                in_point,
                NativeStemPoint {
                    x: in_point.x + ((slope - d_slope) * dy),
                    y: site.y_limit,
                },
                NativeStemPoint {
                    x: out_point.x + ((slope + d_slope) * dy),
                    y: site.y_limit,
                },
            ])
        }
    }
}

fn beam_site_vertical_direction(site: &NativeStemGeometrySite) -> Option<i32> {
    let border = site.beam_border?;
    let border_y = line_y_at_x(border, site.reference.x)?;
    Some(if site.y_limit < border_y { -1 } else { 1 })
}

fn theoretical_line(sheet_slope: f64, reference: NativeStemPoint, y_limit: f64) -> NativeStemLine {
    NativeStemLine {
        start: reference,
        stop: NativeStemPoint {
            x: reference.x - (sheet_slope * (y_limit - reference.y)),
            y: y_limit,
        },
    }
}

fn beam_relation_evidence(
    geometry: &NativeStemsLinkGeometrySource,
    site: &NativeStemGeometrySite,
    candidate: NativeStemGeometryCandidateLine,
    polygon: [NativeStemPoint; 4],
    theoretical: NativeStemLine,
) -> Option<NativeStemLinkCandidate> {
    let border = site.beam_border?;
    let median = site.beam_median?;
    let cross = line_intersection(candidate.median, border)?;
    let max_dx = (0.5 * geometry.interline).round_ties_even();
    let portion = if cross.x < median.start.x + max_dx {
        NativeBeamPortion::Left
    } else if cross.x > median.stop.x - max_dx {
        NativeBeamPortion::Right
    } else {
        NativeBeamPortion::Center
    };
    let half_stem = geometry.stem_thickness / 2.0;
    let dx = match portion {
        NativeBeamPortion::Center => 0.0,
        NativeBeamPortion::Left => border.start.x + half_stem - cross.x,
        NativeBeamPortion::Right => cross.x - border.stop.x + half_stem,
    };
    let dy = 0_f64
        .max(candidate.median.start.y - cross.y)
        .max(cross.y - candidate.median.stop.y);
    let dx_fraction = dx.abs() / geometry.interline;
    let dy_fraction = dy / geometry.interline;
    let (x_impact, y_impact, grade) = relation_impacts(
        dx_fraction,
        dy_fraction,
        beam_x_out_max(site.profile),
        beam_y_gap_max(site.profile),
        1.0,
        4.0,
    );
    if grade < 0.1 {
        return None;
    }
    let y_dir = beam_site_vertical_direction(site)?;
    Some(NativeStemLinkCandidate {
        owner_inter_id: site.owner_inter_id,
        candidate_id: candidate.candidate_id,
        glyph_id: candidate.glyph_id,
        site: site.kind,
        lookup_polygon: polygon,
        relation: NativeStemRelationGeometryKind::BeamStem { portion },
        dx: dx_fraction,
        dy: dy_fraction,
        x_impact,
        y_impact,
        grade,
        extension: NativeStemPoint {
            x: cross.x,
            y: cross.y + (f64::from(y_dir) * (site.beam_height - 1.0)),
        },
        distance_to_theoretical: point_line_distance(line_midpoint(candidate.median), theoretical),
        source_order: candidate.source_order,
    })
}

fn head_relation_evidence(
    geometry: &NativeStemsLinkGeometrySource,
    site: &NativeStemGeometrySite,
    candidate: NativeStemGeometryCandidateLine,
    polygon: [NativeStemPoint; 4],
    theoretical: NativeStemLine,
) -> Option<NativeStemLinkCandidate> {
    let NativeStemGeometrySiteKind::HeadCorner {
        horizontal,
        vertical,
    } = site.kind
    else {
        return None;
    };
    let x_dir = match horizontal {
        NativeStemHeadSide::Left => -1.0,
        NativeStemHeadSide::Right => 1.0,
    };
    let y_dir = f64::from(vertical.direction());
    let x_stem = line_x_at_y(candidate.median, site.reference.y)?;
    let dx = x_dir * (x_stem - site.reference.x);
    let dy = if let Some(stump) = site.stump_bounds {
        let overlap = if y_dir > 0.0 {
            stump.y + stump.height - candidate.median.start.y
        } else {
            candidate.median.stop.y - stump.y
        };
        overlap.min(0.0).abs()
    } else if site.reference.y < candidate.median.start.y {
        candidate.median.start.y - site.reference.y
    } else if site.reference.y > candidate.median.stop.y {
        site.reference.y - candidate.median.stop.y
    } else {
        0.0
    };
    let (x_max, x_weight) = if dx >= 0.0 {
        (head_x_out_max(site.profile), 2.0)
    } else {
        (head_x_in_max(site.profile), 1.0)
    };
    let dx_fraction = dx / geometry.interline;
    let dy_fraction = dy / geometry.interline;
    let (x_impact, y_impact, grade) = relation_impacts(
        dx_fraction,
        dy_fraction,
        x_max,
        head_y_gap_max(site.profile),
        x_weight,
        1.0,
    );
    if grade < 0.1 {
        return None;
    }
    let head_bounds = site.head_bounds?;
    Some(NativeStemLinkCandidate {
        owner_inter_id: site.owner_inter_id,
        candidate_id: candidate.candidate_id,
        glyph_id: candidate.glyph_id,
        site: site.kind,
        lookup_polygon: polygon,
        relation: NativeStemRelationGeometryKind::HeadStem { side: horizontal },
        dx: dx_fraction,
        dy: dy_fraction,
        x_impact,
        y_impact,
        grade,
        extension: NativeStemPoint {
            x: x_stem,
            y: if y_dir > 0.0 {
                head_bounds.y
            } else {
                head_bounds.y + head_bounds.height - 1.0
            },
        },
        distance_to_theoretical: point_line_distance(line_midpoint(candidate.median), theoretical),
        source_order: candidate.source_order,
    })
}

fn relation_impacts(
    dx: f64,
    dy: f64,
    x_max: f64,
    y_max: f64,
    x_weight: f64,
    y_weight: f64,
) -> (f64, f64, f64) {
    let x_impact = if dx >= 0.0 {
        ((x_max - dx) / x_max).clamp(0.0, 1.0)
    } else {
        ((x_max + dx) / x_max).clamp(0.0, 1.0)
    };
    let y_impact = ((y_max - dy) / y_max).clamp(0.0, 1.0);
    let product = x_impact.powf(x_weight) * y_impact.powf(y_weight);
    let grade = 0.8 * product.powf(1.0 / (x_weight + y_weight));
    (x_impact, y_impact, grade)
}

fn beam_x_out_max(profile: u8) -> f64 {
    if profile == 0 { 0.15 } else { 0.2 }
}

fn beam_y_gap_max(profile: u8) -> f64 {
    match profile {
        0 => 0.8,
        1 => 1.2,
        2 => 2.0,
        _ => 4.0,
    }
}

fn head_x_in_max(profile: u8) -> f64 {
    if profile == 0 { 0.2 } else { 0.4 }
}

fn head_x_out_max(profile: u8) -> f64 {
    match profile {
        0 => 0.15,
        1 => 0.25,
        _ => 0.35,
    }
}

fn head_y_gap_max(profile: u8) -> f64 {
    if profile == 0 { 0.8 } else { 1.2 }
}

fn line_y_at_x(line: NativeStemLine, x: f64) -> Option<f64> {
    let dx = line.stop.x - line.start.x;
    (dx.abs() > f64::EPSILON)
        .then(|| line.start.y + ((x - line.start.x) * (line.stop.y - line.start.y) / dx))
}

fn line_x_at_y(line: NativeStemLine, y: f64) -> Option<f64> {
    let dy = line.stop.y - line.start.y;
    (dy.abs() > f64::EPSILON)
        .then(|| line.start.x + ((y - line.start.y) * (line.stop.x - line.start.x) / dy))
}

fn line_intersection(one: NativeStemLine, two: NativeStemLine) -> Option<NativeStemPoint> {
    let x1 = one.start.x;
    let y1 = one.start.y;
    let x2 = one.stop.x;
    let y2 = one.stop.y;
    let x3 = two.start.x;
    let y3 = two.start.y;
    let x4 = two.stop.x;
    let y4 = two.stop.y;
    let denominator = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let cross_one = (x1 * y2) - (y1 * x2);
    let cross_two = (x3 * y4) - (y3 * x4);
    Some(NativeStemPoint {
        x: ((cross_one * (x3 - x4)) - ((x1 - x2) * cross_two)) / denominator,
        y: ((cross_one * (y3 - y4)) - ((y1 - y2) * cross_two)) / denominator,
    })
}

fn line_midpoint(line: NativeStemLine) -> NativeStemPoint {
    NativeStemPoint {
        x: (line.start.x + line.stop.x) / 2.0,
        y: (line.start.y + line.stop.y) / 2.0,
    }
}

fn point_line_distance(point: NativeStemPoint, line: NativeStemLine) -> f64 {
    let dx = line.stop.x - line.start.x;
    let dy = line.stop.y - line.start.y;
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return f64::INFINITY;
    }
    ((dy * point.x) - (dx * point.y) + (line.stop.x * line.start.y) - (line.stop.y * line.start.x))
        .abs()
        / length
}

fn polygon_intersects_rect(polygon: [NativeStemPoint; 4], rect: NativeStemRect) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }
    let mut clipped = polygon.to_vec();
    for (axis, boundary, keep_greater) in [
        (0_u8, rect.x, true),
        (0, rect.x + rect.width, false),
        (1, rect.y, true),
        (1, rect.y + rect.height, false),
    ] {
        clipped = clip_polygon(&clipped, axis, boundary, keep_greater);
        if clipped.len() < 3 {
            return false;
        }
    }
    polygon_area(&clipped) > f64::EPSILON
}

fn clip_polygon(
    polygon: &[NativeStemPoint],
    axis: u8,
    boundary: f64,
    keep_greater: bool,
) -> Vec<NativeStemPoint> {
    let mut output = Vec::new();
    let Some(mut previous) = polygon.last().copied() else {
        return output;
    };
    let mut previous_inside = point_inside_boundary(previous, axis, boundary, keep_greater);
    for current in polygon.iter().copied() {
        let current_inside = point_inside_boundary(current, axis, boundary, keep_greater);
        if current_inside != previous_inside {
            output.push(boundary_intersection(previous, current, axis, boundary));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output
}

fn point_inside_boundary(
    point: NativeStemPoint,
    axis: u8,
    boundary: f64,
    keep_greater: bool,
) -> bool {
    let value = if axis == 0 { point.x } else { point.y };
    if keep_greater {
        value >= boundary
    } else {
        value <= boundary
    }
}

fn boundary_intersection(
    start: NativeStemPoint,
    stop: NativeStemPoint,
    axis: u8,
    boundary: f64,
) -> NativeStemPoint {
    let start_value = if axis == 0 { start.x } else { start.y };
    let stop_value = if axis == 0 { stop.x } else { stop.y };
    let ratio = (boundary - start_value) / (stop_value - start_value);
    NativeStemPoint {
        x: start.x + (ratio * (stop.x - start.x)),
        y: start.y + (ratio * (stop.y - start.y)),
    }
}

fn polygon_area(polygon: &[NativeStemPoint]) -> f64 {
    let mut twice_area = 0.0;
    for (start, stop) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        twice_area += (start.x * stop.y) - (start.y * stop.x);
    }
    twice_area.abs() / 2.0
}

struct NativeStemsContext {
    working: NeutralStemsSystem,
    delta: StemsDelta,
    links: Vec<NativeHeadStemLink>,
}

impl NativeStemsContext {
    fn new(system: &NeutralStemsSystem, links: Vec<NativeHeadStemLink>) -> Self {
        Self {
            working: system.clone(),
            delta: StemsDelta::default(),
            links,
        }
    }

    fn absorb<VisualError>(
        &mut self,
        delta: StemsDelta,
        phase: NativeStemsPhase,
    ) -> Result<(), NativeStemsError<VisualError>> {
        for mutation in delta.mutations {
            apply_native_mutation(&mut self.working, &mutation)
                .map_err(|source| NativeStemsError::Contract { phase, source })?;
            self.delta.mutations.push(mutation);
        }
        Ok(())
    }

    fn push<VisualError>(
        &mut self,
        mutation: StemsDeltaMutation,
        phase: NativeStemsPhase,
    ) -> Result<(), NativeStemsError<VisualError>> {
        self.absorb(
            StemsDelta {
                mutations: vec![mutation],
            },
            phase,
        )
    }

    fn remove_inter<VisualError>(
        &mut self,
        inter_id: usize,
        phase: NativeStemsPhase,
    ) -> Result<(), NativeStemsError<VisualError>> {
        let incident = self
            .working
            .relations
            .iter()
            .filter(|relation| {
                relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
            })
            .map(|relation| relation.id)
            .collect::<Vec<_>>();
        for relation_id in incident {
            self.push(StemsDeltaMutation::RemoveRelation(relation_id), phase)?;
        }
        self.push(StemsDeltaMutation::RemoveInter(inter_id), phase)
    }

    fn relation_is_live(&self, relation_id: usize) -> bool {
        self.working
            .relations
            .iter()
            .any(|relation| relation.id == relation_id)
    }

    fn inter_is_live(&self, inter_id: usize) -> bool {
        self.working
            .inters
            .iter()
            .any(|inter| inter.id == inter_id && !inter.removed)
    }

    fn add_link_evidence<VisualError>(
        &mut self,
        system_id: usize,
        links: Vec<NativeHeadStemLink>,
    ) -> Result<(), NativeStemsError<VisualError>> {
        for link in links {
            validate_native_link(&self.working, link).map_err(|_| {
                NativeStemsError::InvalidLinkEvidence {
                    system_id,
                    relation_id: link.relation_id,
                }
            })?;
            self.links.push(link);
        }
        Ok(())
    }
}

fn apply_native_mutation(
    system: &mut NeutralStemsSystem,
    mutation: &StemsDeltaMutation,
) -> Result<(), StemsContractError> {
    let system_id = system.id;
    match mutation {
        StemsDeltaMutation::RegisterGlyph(_) => {}
        StemsDeltaMutation::AddInter(inter) => {
            if system.inters.iter().any(|existing| existing.id == inter.id) {
                return Err(StemsContractError::DuplicateInter {
                    system_id,
                    inter_id: inter.id,
                });
            }
            system.inters.push(*inter);
        }
        StemsDeltaMutation::RemoveInter(inter_id) => {
            let inter = system
                .inters
                .iter_mut()
                .find(|inter| inter.id == *inter_id)
                .ok_or(StemsContractError::UnknownInter {
                    system_id,
                    inter_id: *inter_id,
                })?;
            inter.removed = true;
        }
        StemsDeltaMutation::AddRelation(relation) => {
            for inter_id in [relation.source_inter_id, relation.target_inter_id] {
                if !system
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
            if system
                .relations
                .iter()
                .any(|existing| existing.id == relation.id)
            {
                return Err(StemsContractError::DuplicateRelation {
                    system_id,
                    relation_id: relation.id,
                });
            }
            system.relations.push(*relation);
        }
        StemsDeltaMutation::RemoveRelation(relation_id) => {
            let Some(index) = system
                .relations
                .iter()
                .position(|relation| relation.id == *relation_id)
            else {
                return Err(StemsContractError::UnknownRelation {
                    system_id,
                    relation_id: *relation_id,
                });
            };
            system.relations.remove(index);
        }
        StemsDeltaMutation::SetAbnormal { inter_id, abnormal } => {
            let inter = system
                .inters
                .iter_mut()
                .find(|inter| inter.id == *inter_id)
                .ok_or(StemsContractError::UnknownInter {
                    system_id,
                    inter_id: *inter_id,
                })?;
            inter.abnormal = *abnormal;
        }
    }
    Ok(())
}

fn validate_native_link(system: &NeutralStemsSystem, link: NativeHeadStemLink) -> Result<(), ()> {
    let Some(relation) = system
        .relations
        .iter()
        .find(|relation| relation.id == link.relation_id)
    else {
        return Err(());
    };
    if relation.kind != NeutralStemsRelationKind::HeadStem
        || relation.source_inter_id != link.head_id
        || relation.target_inter_id != link.stem_id
    {
        return Err(());
    }
    let head_ok = system.inters.iter().any(|inter| {
        inter.id == link.head_id && !inter.removed && inter.kind == NeutralStemsInterKind::Head
    });
    let stem_ok = system.inters.iter().any(|inter| {
        inter.id == link.stem_id && !inter.removed && inter.kind == NeutralStemsInterKind::Stem
    });
    (head_ok && stem_ok).then_some(()).ok_or(())
}

impl<Linker> NativeVisualStems<Linker>
where
    Linker: VisualStemLinker,
{
    fn source_for(
        &self,
        system_id: usize,
    ) -> Result<&NativeStemsSystemSource, NativeStemsError<Linker::Error>> {
        let mut matches = self
            .sources
            .iter()
            .filter(|source| source.system_id == system_id);
        let Some(source) = matches.next() else {
            return Err(NativeStemsError::MissingSystemSource(system_id));
        };
        if matches.next().is_some() {
            return Err(NativeStemsError::DuplicateSystemSource(system_id));
        }
        Ok(source)
    }

    fn prepare_source(
        system: &NeutralStemsSystem,
        source: &NativeStemsSystemSource,
    ) -> Result<PreparedNativeStems, NativeStemsError<Linker::Error>> {
        let mut seen = BTreeSet::new();
        for (inter_id, expected_kind) in source
            .beams
            .iter()
            .map(|beam| (beam.inter_id, 0_u8))
            .chain(source.heads.iter().map(|head| (head.inter_id, 1_u8)))
        {
            if !seen.insert(inter_id) {
                return Err(NativeStemsError::DuplicateCandidate {
                    system_id: system.id,
                    inter_id,
                });
            }
            let Some(inter) = system
                .inters
                .iter()
                .find(|inter| inter.id == inter_id && !inter.removed)
            else {
                return Err(NativeStemsError::MissingCandidate {
                    system_id: system.id,
                    inter_id,
                });
            };
            let kind_ok = matches!(
                (expected_kind, inter.kind),
                (0, NeutralStemsInterKind::Beam { .. }) | (1, NeutralStemsInterKind::Head)
            );
            if !kind_ok {
                return Err(NativeStemsError::WrongCandidateKind {
                    system_id: system.id,
                    inter_id,
                });
            }
        }
        for link in &source.head_stem_links {
            validate_native_link(system, *link).map_err(|_| {
                NativeStemsError::InvalidLinkEvidence {
                    system_id: system.id,
                    relation_id: link.relation_id,
                }
            })?;
        }

        let mut seeds = source
            .seeds
            .iter()
            .copied()
            .filter(|seed| !seed.bar_overlap)
            .collect::<Vec<_>>();
        seeds.sort_by_key(|seed| seed.x);
        let mut beams = source.beams.clone();
        beams.sort_by_key(|beam| beam.x);
        let mut heads = source.heads.clone();
        heads.sort_by_key(|head| head.x);
        Ok(PreparedNativeStems {
            seeds,
            beams,
            heads,
        })
    }

    fn absorb_decision<Value>(
        context: &mut NativeStemsContext,
        decision: NativeStemsDecision<Value, Linker::Error>,
        phase: NativeStemsPhase,
    ) -> Result<Value, NativeStemsError<Linker::Error>> {
        context.absorb(decision.delta, phase)?;
        if let Some(source) = decision.error {
            return Err(NativeStemsError::Visual { phase, source });
        }
        Ok(decision.value)
    }

    fn failure(
        context: NativeStemsContext,
        error: NativeStemsError<Linker::Error>,
    ) -> StemsStageOutcome<NativeStemsError<Linker::Error>> {
        StemsStageOutcome {
            delta: context.delta,
            error: Some(error),
        }
    }

    fn cleanup_head(
        &mut self,
        sheet_id: usize,
        head: NativeStemHead,
        context: &mut NativeStemsContext,
    ) -> Result<(), NativeStemsError<Linker::Error>> {
        let mut partitions: Vec<(usize, Vec<NativeHeadStemLink>)> = Vec::new();
        for link in context.links.iter().copied().filter(|link| {
            link.head_id == head.inter_id && context.relation_is_live(link.relation_id)
        }) {
            if let Some((_, links)) = partitions
                .iter_mut()
                .find(|(partition, _)| *partition == link.partition)
            {
                links.push(link);
            } else {
                partitions.push((link.partition, vec![link]));
            }
        }

        for (_, mut links) in partitions {
            while links.len() > 2 {
                remove_worst_contribution(context, &mut links)?;
            }
            if links.len() != 2 {
                continue;
            }
            let mut left = None;
            let mut right = None;
            let mut canonical = true;
            for link in &links {
                if link.dy > 0.2 {
                    canonical = false;
                    break;
                }
                match link.head_side {
                    NativeStemHeadSide::Left => left = Some(*link),
                    NativeStemHeadSide::Right => right = Some(*link),
                }
            }
            if canonical {
                canonical = match (left, right) {
                    (Some(left), Some(right)) => self
                        .linker
                        .is_canonical_share(NativeCanonicalShareInput {
                            sheet_id,
                            system: &context.working,
                            head,
                            left,
                            right,
                        })
                        .map_err(|source| NativeStemsError::Visual {
                            phase: NativeStemsPhase::CanonicalHeadShare,
                            source,
                        })?,
                    _ => false,
                };
            }
            if !canonical {
                remove_worst_contribution(context, &mut links)?;
            }
        }
        Ok(())
    }

    fn process_native(
        &mut self,
        input: StemsSystemInput<'_>,
    ) -> StemsStageOutcome<NativeStemsError<Linker::Error>> {
        let source = match self.source_for(input.system.id) {
            Ok(source) => source.clone(),
            Err(error) => {
                return StemsStageOutcome {
                    delta: StemsDelta::default(),
                    error: Some(error),
                };
            }
        };
        let prepared = match Self::prepare_source(input.system, &source) {
            Ok(prepared) => prepared,
            Err(error) => {
                return StemsStageOutcome {
                    delta: StemsDelta::default(),
                    error: Some(error),
                };
            }
        };
        let seeds = prepared.seeds;
        let beams_by_x = prepared.beams;
        let heads_by_x = prepared.heads;
        let mut context = NativeStemsContext::new(input.system, source.head_stem_links.clone());

        // Java inspectStems: beams by abscissa, then heads by abscissa.
        for beam in &beams_by_x {
            let decision = self.linker.inspect_beam(NativeStemCandidateInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &[],
                candidate: *beam,
            });
            let tremolo = match Self::absorb_decision(
                &mut context,
                decision,
                NativeStemsPhase::InspectBeam,
            ) {
                Ok(value) => value,
                Err(error) => return Self::failure(context, error),
            };
            if tremolo {
                if let Err(error) =
                    context.remove_inter(beam.inter_id, NativeStemsPhase::InspectBeam)
                {
                    return Self::failure(context, error);
                }
            }
        }
        for head in &heads_by_x {
            let decision = self.linker.inspect_head(NativeStemCandidateInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &[],
                candidate: *head,
            });
            if let Err(error) =
                Self::absorb_decision(&mut context, decision, NativeStemsPhase::InspectHead)
            {
                return Self::failure(context, error);
            }
        }

        // Java linkStems: current live beams by decreasing width. Stable sort
        // preserves SIG/source order for equal widths.
        let mut beams_by_width = source
            .beams
            .iter()
            .copied()
            .filter(|beam| context.inter_is_live(beam.inter_id))
            .collect::<Vec<_>>();
        beams_by_width.sort_by_key(|beam| Reverse(beam.width));
        let mut linked_beams = Vec::new();
        for beam in beams_by_width {
            let link_candidates = source
                .link_geometry
                .as_ref()
                .map(|geometry| {
                    geometry.candidates_for(beam.inter_id, NativeStemsPhase::LinkBeamSides)
                })
                .unwrap_or_default();
            let decision = self.linker.link_beam_sides(NativeStemCandidateInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &link_candidates,
                candidate: beam,
            });
            let result = match Self::absorb_decision(
                &mut context,
                decision,
                NativeStemsPhase::LinkBeamSides,
            ) {
                Ok(result) => result,
                Err(error) => return Self::failure(context, error),
            };
            if let Err(error) = context.add_link_evidence(input.system.id, result.head_stem_links) {
                return Self::failure(context, error);
            }
            if result.linked {
                linked_beams.push(beam);
            }
        }
        for beam in linked_beams {
            let link_candidates = source
                .link_geometry
                .as_ref()
                .map(|geometry| {
                    geometry.candidates_for(beam.inter_id, NativeStemsPhase::LinkBeamStumps)
                })
                .unwrap_or_default();
            let decision = self.linker.link_beam_stumps(NativeStemCandidateInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &link_candidates,
                candidate: beam,
            });
            let result = match Self::absorb_decision(
                &mut context,
                decision,
                NativeStemsPhase::LinkBeamStumps,
            ) {
                Ok(result) => result,
                Err(error) => return Self::failure(context, error),
            };
            if let Err(error) = context.add_link_evidence(input.system.id, result.head_stem_links) {
                return Self::failure(context, error);
            }
        }

        // Heads are recreated from SIG insertion/source order and stably sorted
        // by decreasing intrinsic grade.
        let mut heads_by_grade = source.heads.clone();
        heads_by_grade.sort_by(|left, right| right.grade.total_cmp(&left.grade));
        let mut unlinked = Vec::new();
        for head in &heads_by_grade {
            let link_candidates = source
                .link_geometry
                .as_ref()
                .map(|geometry| {
                    geometry.candidates_for(head.inter_id, NativeStemsPhase::LinkHeadSides)
                })
                .unwrap_or_default();
            let decision = self.linker.link_head_sides(NativeHeadLinkInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &link_candidates,
                head: *head,
                existing_stems_only: false,
            });
            let result = match Self::absorb_decision(
                &mut context,
                decision,
                NativeStemsPhase::LinkHeadSides,
            ) {
                Ok(result) => result,
                Err(error) => return Self::failure(context, error),
            };
            if let Err(error) = context.add_link_evidence(input.system.id, result.head_stem_links) {
                return Self::failure(context, error);
            }
            if !result.linked {
                unlinked.push(*head);
            }
        }
        for head in unlinked {
            let link_candidates = source
                .link_geometry
                .as_ref()
                .map(|geometry| {
                    geometry.candidates_for(head.inter_id, NativeStemsPhase::RelinkHeadSides)
                })
                .unwrap_or_default();
            let decision = self.linker.link_head_sides(NativeHeadLinkInput {
                sheet_id: input.sheet_id,
                system: &context.working,
                seeds: &seeds,
                beams: &beams_by_x,
                heads: &heads_by_x,
                link_candidates: &link_candidates,
                head,
                existing_stems_only: true,
            });
            let result = match Self::absorb_decision(
                &mut context,
                decision,
                NativeStemsPhase::RelinkHeadSides,
            ) {
                Ok(result) => result,
                Err(error) => return Self::failure(context, error),
            };
            if let Err(error) = context.add_link_evidence(input.system.id, result.head_stem_links) {
                return Self::failure(context, error);
            }
        }

        // finalizeStems uses the reverse-grade list left by linkStems.
        for head in &heads_by_grade {
            if let Err(error) = self.cleanup_head(input.sheet_id, *head, &mut context) {
                return Self::failure(context, error);
            }
        }
        for head in heads_by_grade {
            if head.requires_stem
                && !context.links.iter().any(|link| {
                    link.head_id == head.inter_id && context.relation_is_live(link.relation_id)
                })
            {
                if let Err(error) = context.push(
                    StemsDeltaMutation::SetAbnormal {
                        inter_id: head.inter_id,
                        abnormal: true,
                    },
                    NativeStemsPhase::CanonicalHeadShare,
                ) {
                    return Self::failure(context, error);
                }
            }
        }

        StemsStageOutcome::success(context.delta)
    }
}

fn remove_worst_contribution<VisualError>(
    context: &mut NativeStemsContext,
    links: &mut Vec<NativeHeadStemLink>,
) -> Result<(), NativeStemsError<VisualError>> {
    let mut worst_index = 0;
    let mut worst = f64::INFINITY;
    for (index, link) in links.iter().enumerate() {
        let contribution = link.stem_grade * (link.target_ratio - 1.0);
        if contribution < worst {
            worst = contribution;
            worst_index = index;
        }
    }
    let discarded = links.remove(worst_index);
    context.push(
        StemsDeltaMutation::RemoveRelation(discarded.relation_id),
        NativeStemsPhase::CanonicalHeadShare,
    )
}

impl<Linker> VisualStemsRetriever for NativeVisualStems<Linker>
where
    Linker: VisualStemLinker,
{
    type Error = NativeStemsError<Linker::Error>;

    fn process_stems(&mut self, input: StemsSystemInput<'_>) -> StemsStageOutcome<Self::Error> {
        self.process_native(input)
    }

    fn finalize_beams(&mut self, input: StemsSystemInput<'_>) -> StemsStageOutcome<Self::Error> {
        let source = match self.source_for(input.system.id) {
            Ok(source) => source.clone(),
            Err(error) => {
                return StemsStageOutcome {
                    delta: StemsDelta::default(),
                    error: Some(error),
                };
            }
        };
        // Java finalizeBeams starts a fresh retriever and only recollects
        // vertical seeds. Beams/heads or relations legitimately removed by
        // process() must not be revalidated here.
        let mut seeds = source
            .seeds
            .iter()
            .copied()
            .filter(|seed| !seed.bar_overlap)
            .collect::<Vec<_>>();
        seeds.sort_by_key(|seed| seed.x);
        let outcome = self.linker.finalize_beams(input, &seeds);
        let mut context = NativeStemsContext::new(input.system, source.head_stem_links);
        if let Err(error) = context.absorb(outcome.delta, NativeStemsPhase::FinalizeBeams) {
            return Self::failure(context, error);
        }
        if let Some(source) = outcome.error {
            return Self::failure(
                context,
                NativeStemsError::Visual {
                    phase: NativeStemsPhase::FinalizeBeams,
                    source,
                },
            );
        }
        StemsStageOutcome::success(context.delta)
    }
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

    #[derive(Default)]
    struct FakeNativeLinker {
        calls: Vec<String>,
        fail_inspect_head: Option<usize>,
        tremolo_beam: Option<usize>,
    }

    impl FakeNativeLinker {
        fn record_candidates<Candidate>(
            &mut self,
            label: &str,
            input: NativeStemCandidateInput<'_, Candidate>,
            candidate_id: usize,
        ) {
            let seeds = input
                .seeds
                .iter()
                .map(|seed| seed.glyph_id)
                .collect::<Vec<_>>();
            self.calls
                .push(format!("{label}:{candidate_id}:seeds={seeds:?}"));
        }
    }

    impl VisualStemLinker for FakeNativeLinker {
        type Error = &'static str;

        fn inspect_beam(
            &mut self,
            input: NativeStemCandidateInput<'_, NativeStemBeam>,
        ) -> NativeStemsDecision<bool, Self::Error> {
            let beam = input.candidate;
            self.record_candidates("inspect-beam", input, beam.inter_id);
            NativeStemsDecision::success(self.tremolo_beam == Some(beam.inter_id))
        }

        fn inspect_head(
            &mut self,
            input: NativeStemCandidateInput<'_, NativeStemHead>,
        ) -> NativeStemsDecision<(), Self::Error> {
            let head = input.candidate;
            self.record_candidates("inspect-head", input, head.inter_id);
            if self.fail_inspect_head == Some(head.inter_id) {
                NativeStemsDecision {
                    value: (),
                    delta: StemsDelta {
                        mutations: vec![StemsDeltaMutation::AddInter(stem(40))],
                    },
                    error: Some("inspect failed"),
                }
            } else {
                NativeStemsDecision::success(())
            }
        }

        fn link_beam_sides(
            &mut self,
            input: NativeStemCandidateInput<'_, NativeStemBeam>,
        ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error> {
            let beam = input.candidate;
            self.record_candidates("beam-sides", input, beam.inter_id);
            NativeStemsDecision::success(NativeStemLinkResult {
                linked: true,
                head_stem_links: Vec::new(),
            })
        }

        fn link_beam_stumps(
            &mut self,
            input: NativeStemCandidateInput<'_, NativeStemBeam>,
        ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error> {
            let beam = input.candidate;
            self.record_candidates("beam-stumps", input, beam.inter_id);
            NativeStemsDecision::success(NativeStemLinkResult::default())
        }

        fn link_head_sides(
            &mut self,
            input: NativeHeadLinkInput<'_>,
        ) -> NativeStemsDecision<NativeStemLinkResult, Self::Error> {
            self.calls.push(format!(
                "head-{}:{}:relations={}",
                if input.existing_stems_only {
                    "existing"
                } else {
                    "new"
                },
                input.head.inter_id,
                input.system.relations.len()
            ));
            NativeStemsDecision::success(NativeStemLinkResult {
                linked: input.head.inter_id == 20,
                head_stem_links: Vec::new(),
            })
        }

        fn is_canonical_share(
            &mut self,
            input: NativeCanonicalShareInput<'_>,
        ) -> Result<bool, Self::Error> {
            self.calls.push(format!(
                "canonical:{}:{}:{}",
                input.head.inter_id, input.left.relation_id, input.right.relation_id
            ));
            Ok(true)
        }

        fn finalize_beams(
            &mut self,
            input: StemsSystemInput<'_>,
            seeds: &[NativeStemSeed],
        ) -> StemsStageOutcome<Self::Error> {
            self.calls.push(format!(
                "finalize:{}:seeds={:?}",
                input.system.id,
                seeds.iter().map(|seed| seed.glyph_id).collect::<Vec<_>>()
            ));
            StemsStageOutcome::success(StemsDelta::default())
        }
    }

    fn head(id: usize) -> NeutralStemsInter {
        NeutralStemsInter {
            id,
            kind: NeutralStemsInterKind::Head,
            glyph_id: Some(id + 2_000),
            abnormal: false,
            removed: false,
        }
    }

    fn native_system() -> NeutralStemsSystem {
        NeutralStemsSystem {
            id: 7,
            inters: vec![
                beam(10, Some(99), 1, 2),
                beam(11, Some(99), 1, 1),
                NeutralStemsInter {
                    id: 99,
                    kind: NeutralStemsInterKind::BeamGroup,
                    glyph_id: None,
                    abnormal: false,
                    removed: false,
                },
                head(20),
                head(21),
                stem(30),
                stem(31),
                stem(32),
            ],
            relations: vec![
                NeutralStemsRelation {
                    id: 100,
                    source_inter_id: 20,
                    target_inter_id: 30,
                    kind: NeutralStemsRelationKind::HeadStem,
                },
                NeutralStemsRelation {
                    id: 101,
                    source_inter_id: 20,
                    target_inter_id: 31,
                    kind: NeutralStemsRelationKind::HeadStem,
                },
                NeutralStemsRelation {
                    id: 102,
                    source_inter_id: 20,
                    target_inter_id: 32,
                    kind: NeutralStemsRelationKind::HeadStem,
                },
            ],
            contextualized: false,
        }
    }

    fn native_source() -> NativeStemsSystemSource {
        NativeStemsSystemSource {
            system_id: 7,
            seeds: vec![
                NativeStemSeed {
                    glyph_id: 1,
                    x: 30,
                    bar_overlap: false,
                },
                NativeStemSeed {
                    glyph_id: 2,
                    x: 5,
                    bar_overlap: false,
                },
                NativeStemSeed {
                    glyph_id: 3,
                    x: 1,
                    bar_overlap: true,
                },
            ],
            beams: vec![
                NativeStemBeam {
                    inter_id: 10,
                    x: 20,
                    width: 5,
                },
                NativeStemBeam {
                    inter_id: 11,
                    x: 10,
                    width: 9,
                },
            ],
            heads: vec![
                NativeStemHead {
                    inter_id: 20,
                    x: 30,
                    grade: 0.9,
                    requires_stem: true,
                },
                NativeStemHead {
                    inter_id: 21,
                    x: 5,
                    grade: 0.4,
                    requires_stem: true,
                },
            ],
            head_stem_links: vec![
                NativeHeadStemLink {
                    relation_id: 100,
                    head_id: 20,
                    stem_id: 30,
                    partition: 8,
                    dy: 0.0,
                    head_side: NativeStemHeadSide::Left,
                    stem_grade: 0.1,
                    target_ratio: 2.0,
                },
                NativeHeadStemLink {
                    relation_id: 101,
                    head_id: 20,
                    stem_id: 31,
                    partition: 8,
                    dy: 0.0,
                    head_side: NativeStemHeadSide::Left,
                    stem_grade: 0.9,
                    target_ratio: 2.0,
                },
                NativeHeadStemLink {
                    relation_id: 102,
                    head_id: 20,
                    stem_id: 32,
                    partition: 8,
                    dy: 0.0,
                    head_side: NativeStemHeadSide::Right,
                    stem_grade: 0.8,
                    target_ratio: 2.0,
                },
            ],
            link_geometry: None,
        }
    }

    #[test]
    fn native_retriever_owns_java_candidate_order_cleanup_and_epilog() {
        let visual = NativeVisualStems::new(vec![native_source()], FakeNativeLinker::default());
        let mut step = HeadlessStemsStep::new(visual);
        let mut sheet = NeutralStemsSheet {
            id: 4,
            next_inter_id: 500,
            next_relation_id: 600,
            registered_glyphs: Vec::new(),
            systems: vec![native_system()],
            mutations: Vec::new(),
        };

        let report = step.process(&mut sheet).unwrap();

        assert!(report.system_errors.is_empty());
        assert_eq!(
            step.visual().linker().calls,
            vec![
                "inspect-beam:11:seeds=[2, 1]",
                "inspect-beam:10:seeds=[2, 1]",
                "inspect-head:21:seeds=[2, 1]",
                "inspect-head:20:seeds=[2, 1]",
                "beam-sides:11:seeds=[2, 1]",
                "beam-sides:10:seeds=[2, 1]",
                "beam-stumps:11:seeds=[2, 1]",
                "beam-stumps:10:seeds=[2, 1]",
                "head-new:20:relations=3",
                "head-new:21:relations=3",
                "head-existing:21:relations=3",
                "canonical:20:101:102",
                "finalize:7:seeds=[2, 1]",
            ]
        );
        assert_eq!(
            sheet.systems[0]
                .relations
                .iter()
                .map(|relation| relation.id)
                .collect::<Vec<_>>(),
            vec![101, 102]
        );
        assert!(
            sheet.systems[0]
                .inters
                .iter()
                .find(|inter| inter.id == 21)
                .unwrap()
                .abnormal
        );
        assert!(sheet.systems[0].contextualized);
        assert!(sheet.mutations.contains(&StemsMutation::RelationRemoved {
            system_id: 7,
            relation_id: 100,
        }));
    }

    #[test]
    fn native_visual_failure_retains_exact_mutation_prefix() {
        let linker = FakeNativeLinker {
            fail_inspect_head: Some(20),
            ..FakeNativeLinker::default()
        };
        let mut visual = NativeVisualStems::new(vec![native_source()], linker);
        let system = native_system();

        let outcome = visual.process_stems(StemsSystemInput {
            sheet_id: 4,
            system: &system,
        });

        assert_eq!(
            outcome.delta.mutations,
            vec![StemsDeltaMutation::AddInter(stem(40))]
        );
        assert_eq!(
            outcome.error,
            Some(NativeStemsError::Visual {
                phase: NativeStemsPhase::InspectHead,
                source: "inspect failed",
            })
        );
        assert_eq!(
            visual.linker().calls,
            vec![
                "inspect-beam:11:seeds=[2, 1]",
                "inspect-beam:10:seeds=[2, 1]",
                "inspect-head:21:seeds=[2, 1]",
                "inspect-head:20:seeds=[2, 1]",
            ]
        );
    }

    #[test]
    fn native_tremolo_removal_precedes_live_beam_link_order() {
        let mut system = native_system();
        system.relations.push(NeutralStemsRelation {
            id: 90,
            source_inter_id: 99,
            target_inter_id: 11,
            kind: NeutralStemsRelationKind::BeamGroupMember,
        });
        let linker = FakeNativeLinker {
            tremolo_beam: Some(11),
            ..FakeNativeLinker::default()
        };
        let mut visual = NativeVisualStems::new(vec![native_source()], linker);

        let outcome = visual.process_stems(StemsSystemInput {
            sheet_id: 4,
            system: &system,
        });

        assert!(outcome.error.is_none());
        assert_eq!(
            &outcome.delta.mutations[..2],
            &[
                StemsDeltaMutation::RemoveRelation(90),
                StemsDeltaMutation::RemoveInter(11),
            ]
        );
        assert!(
            !visual
                .linker()
                .calls
                .iter()
                .any(|call| call.starts_with("beam-sides:11"))
        );
    }

    fn geometry_point(x: f64, y: f64) -> NativeStemPoint {
        NativeStemPoint { x, y }
    }

    fn geometry_line(x1: f64, y1: f64, x2: f64, y2: f64) -> NativeStemLine {
        NativeStemLine {
            start: geometry_point(x1, y1),
            stop: geometry_point(x2, y2),
        }
    }

    fn geometry_rect(x: f64, y: f64, width: f64, height: f64) -> NativeStemRect {
        NativeStemRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn native_beam_link_geometry_matches_lookup_impacts_extension_and_ties() {
        let geometry = NativeStemsLinkGeometrySource {
            system_id: 7,
            interline: 10.0,
            stem_thickness: 2.0,
            sheet_slope: 0.0,
            slope_margin: 0.015,
            half_beam_lookup_dx: 3.0,
            max_beam_seed_dy_ratio: 0.25,
            sites: vec![NativeStemGeometrySite {
                owner_inter_id: 10,
                kind: NativeStemGeometrySiteKind::BeamSide(NativeStemHeadSide::Left),
                reference: geometry_point(1.0, 12.0),
                out_point: None,
                in_point: None,
                y_limit: 30.0,
                profile: 0,
                beam_median: Some(geometry_line(0.0, 10.0, 20.0, 10.0)),
                beam_border: Some(geometry_line(0.0, 12.0, 20.0, 12.0)),
                beam_height: 4.0,
                head_bounds: None,
                stump_bounds: None,
            }],
            candidates: vec![
                NativeStemGeometryCandidateLine {
                    candidate_id: 501,
                    glyph_id: Some(601),
                    median: geometry_line(1.0, 12.0, 1.0, 30.0),
                    bounds: geometry_rect(0.5, 12.0, 1.0, 18.0),
                    source_order: 9,
                    existing_stem: false,
                },
                NativeStemGeometryCandidateLine {
                    candidate_id: 500,
                    glyph_id: Some(600),
                    median: geometry_line(1.0, 12.0, 1.0, 30.0),
                    bounds: geometry_rect(0.5, 12.0, 1.0, 18.0),
                    source_order: 2,
                    existing_stem: false,
                },
                NativeStemGeometryCandidateLine {
                    candidate_id: 999,
                    glyph_id: None,
                    median: geometry_line(30.0, 12.0, 30.0, 30.0),
                    bounds: geometry_rect(29.5, 12.0, 1.0, 18.0),
                    source_order: 0,
                    existing_stem: true,
                },
            ],
        };

        let candidates = geometry.candidates_for(10, NativeStemsPhase::LinkBeamSides);
        assert_eq!(
            candidates
                .iter()
                .map(|item| item.candidate_id)
                .collect::<Vec<_>>(),
            vec![500, 501]
        );
        let item = &candidates[0];
        assert_eq!(item.lookup_polygon[0], geometry_point(-2.0, 14.0));
        assert_eq!(item.lookup_polygon[1], geometry_point(4.0, 14.0));
        assert_eq!(
            (item.dx, item.dy, item.x_impact, item.y_impact),
            (0.0, 0.0, 1.0, 1.0)
        );
        assert!((item.grade - 0.8).abs() < 1e-12);
        assert_eq!(item.extension, geometry_point(1.0, 15.0));
        assert_eq!(
            item.relation,
            NativeStemRelationGeometryKind::BeamStem {
                portion: NativeBeamPortion::Left
            }
        );
    }

    #[test]
    fn native_head_link_geometry_splits_new_existing_and_rejects_bad_impact() {
        let site = NativeStemGeometrySite {
            owner_inter_id: 20,
            kind: NativeStemGeometrySiteKind::HeadCorner {
                horizontal: NativeStemHeadSide::Right,
                vertical: NativeStemVerticalSide::Bottom,
            },
            reference: geometry_point(10.0, 10.0),
            out_point: Some(geometry_point(20.0, 10.0)),
            in_point: Some(geometry_point(0.0, 10.0)),
            y_limit: 30.0,
            profile: 0,
            beam_median: None,
            beam_border: None,
            beam_height: 0.0,
            head_bounds: Some(geometry_rect(8.0, 8.0, 4.0, 5.0)),
            stump_bounds: None,
        };
        let candidate = |id, x, existing, source_order| NativeStemGeometryCandidateLine {
            candidate_id: id,
            glyph_id: Some(id + 100),
            median: geometry_line(x, 10.0, x, 30.0),
            bounds: geometry_rect(x - 0.5, 10.0, 1.0, 20.0),
            source_order,
            existing_stem: existing,
        };
        let geometry = NativeStemsLinkGeometrySource {
            system_id: 7,
            interline: 10.0,
            stem_thickness: 2.0,
            sheet_slope: 0.0,
            slope_margin: 0.015,
            half_beam_lookup_dx: 3.0,
            max_beam_seed_dy_ratio: 0.25,
            sites: vec![site],
            candidates: vec![
                candidate(700, 11.0, false, 0),
                candidate(701, 10.5, true, 1),
                candidate(702, 13.0, false, 2),
            ],
        };

        let new = geometry.candidates_for(20, NativeStemsPhase::LinkHeadSides);
        let existing = geometry.candidates_for(20, NativeStemsPhase::RelinkHeadSides);
        assert_eq!(
            new.iter().map(|item| item.candidate_id).collect::<Vec<_>>(),
            vec![700]
        );
        assert_eq!(
            existing
                .iter()
                .map(|item| item.candidate_id)
                .collect::<Vec<_>>(),
            vec![701]
        );
        assert_eq!((new[0].dx, new[0].dy), (0.1, 0.0));
        assert!((new[0].x_impact - (1.0 / 3.0)).abs() < 1e-12);
        assert_eq!(new[0].extension, geometry_point(11.0, 8.0));
    }
}
