// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral `LinesRetriever` boundary from cluster passes to staff candidates.
//!
//! Raster-to-filament construction and later bar/system discovery remain
//! separate stages. This module executes the main and optional small-interline
//! cluster passes, then ports Java `buildStaves` ordering and rejection policy.

use std::{cmp::Ordering, collections::BTreeMap, error::Error, fmt};

use crate::{
    cluster_coordinator::RecursiveCombSnapshot,
    cluster_merge::{ClusterMergeError, cluster_deskewed_ordinate},
    cluster_ownership::{ClusterId, ClusterOwnership, ClusterOwnershipError, CombId},
    cluster_pipeline::{
        ClusterPipelineError, ClusterRetrievalParameters, ClusterRetrievalResult, retrieve_clusters,
    },
    filament::{FilamentError, StaffFilament},
    line_cluster::{FilamentId, LineCluster, LineClusterError},
    section::Bounds,
};

/// Mutable state owned by one Java `ClustersRetriever` pass.
#[derive(Clone, Debug)]
pub struct ClusterPassState {
    ownership: ClusterOwnership,
    clusters: BTreeMap<ClusterId, LineCluster>,
    filaments: BTreeMap<FilamentId, StaffFilament>,
    combs: BTreeMap<CombId, RecursiveCombSnapshot>,
    filament_order: Vec<FilamentId>,
    parameters: ClusterRetrievalParameters,
}

impl ClusterPassState {
    #[must_use]
    pub fn new(
        ownership: ClusterOwnership,
        filaments: BTreeMap<FilamentId, StaffFilament>,
        combs: BTreeMap<CombId, RecursiveCombSnapshot>,
        filament_order: Vec<FilamentId>,
        parameters: ClusterRetrievalParameters,
    ) -> Self {
        Self {
            ownership,
            clusters: BTreeMap::new(),
            filaments,
            combs,
            filament_order,
            parameters,
        }
    }

    #[must_use]
    pub const fn ownership(&self) -> &ClusterOwnership {
        &self.ownership
    }

    #[must_use]
    pub const fn clusters(&self) -> &BTreeMap<ClusterId, LineCluster> {
        &self.clusters
    }

    #[must_use]
    pub const fn filaments(&self) -> &BTreeMap<FilamentId, StaffFilament> {
        &self.filaments
    }

    /// Root order presented to this clustering pass after comb-network
    /// following. For Java's secondary pass this is derived from the same
    /// main-interline filament objects, merely interpreted with the small
    /// pass parameters.
    #[must_use]
    pub fn filament_order(&self) -> &[FilamentId] {
        &self.filament_order
    }

    #[must_use]
    pub const fn parameters(&self) -> &ClusterRetrievalParameters {
        &self.parameters
    }

    /// Seed one independently verified five-line staff hypothesis before the
    /// ordinary comb traversal.
    ///
    /// This is used only after a tall, overgrown filament has been split into
    /// five strong periodic ridges. The provisional cluster still goes through
    /// the normal expansion, merge, trim, desired-size, consistency, and staff
    /// rejection passes; seeding merely prevents the generic comb graph from
    /// losing all five lines a second time.
    pub fn seed_recovered_cluster(
        &mut self,
        line_ids: &[FilamentId],
    ) -> Result<ClusterId, LinesCoordinatorError> {
        if line_ids.len() != 5 {
            return Err(LinesCoordinatorError::InvalidRecoveredCluster);
        }
        let distinct = line_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if distinct.len() != line_ids.len() {
            return Err(LinesCoordinatorError::InvalidRecoveredCluster);
        }

        let mut next = self.clone();
        let seed = line_ids[0];
        let seed_value = next
            .filaments
            .get(&seed)
            .cloned()
            .ok_or(LinesCoordinatorError::MissingRecoveredFilament(seed))?;
        let cluster_id = next.ownership.register_cluster(seed)?;
        let mut cluster = LineCluster::new(next.parameters.interline(), seed, seed_value)?;
        for (position, &id) in line_ids.iter().enumerate().skip(1) {
            let value = next
                .filaments
                .get(&id)
                .cloned()
                .ok_or(LinesCoordinatorError::MissingRecoveredFilament(id))?;
            cluster.include_line(position as i32, id, value)?;
            next.ownership
                .assign_filament(id, cluster_id, position as i32)?;
        }
        if next.clusters.insert(cluster_id, cluster).is_some() {
            return Err(LinesCoordinatorError::DuplicateClusterValue(cluster_id));
        }
        *self = next;
        Ok(cluster_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterPass {
    Main,
    Small,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocatedClusterId {
    pass: ClusterPass,
    cluster: ClusterId,
}

impl LocatedClusterId {
    #[must_use]
    pub const fn pass(self) -> ClusterPass {
        self.pass
    }

    #[must_use]
    pub const fn cluster(self) -> ClusterId {
        self.cluster
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaffCandidateKind {
    Standard,
    OneLine,
    Tablature,
}

/// Preliminary Java `Staff` allocation before barlines refine side limits.
#[derive(Clone, Debug)]
pub struct StaffCandidate {
    id: usize,
    source: Option<LocatedClusterId>,
    kind: StaffCandidateKind,
    left: f64,
    right: f64,
    interline: usize,
    line_ids: Vec<FilamentId>,
    /// The cluster's line filaments, each already carrying every section the
    /// cluster merged into it.
    ///
    /// `line_ids` names only each line's *primary* filament, and looking that id
    /// back up in the factory map returns the filament as it was before any
    /// merge. That is not the staff's line: when a cluster absorbs another, the
    /// resident line keeps its primary id and gains the incoming sections, so on
    /// a staff whose line was seeded by a short fragment the pristine filament is
    /// that fragment and nothing else.
    line_filaments: Vec<StaffFilament>,
    small: bool,
    short: bool,
    tentative: bool,
}

/// Equality ignores [`Self::line_filaments`], which is derived rather than
/// independent: `source` names the cluster the candidate came from, and that
/// cluster determines its lines. `StaffFilament` has no `PartialEq` of its own,
/// and giving it one for this would mean comparing cached spline geometry.
impl PartialEq for StaffCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.source == other.source
            && self.kind == other.kind
            && self.left == other.left
            && self.right == other.right
            && self.interline == other.interline
            && self.line_ids == other.line_ids
            && self.small == other.small
            && self.short == other.short
            && self.tentative == other.tentative
    }
}

impl StaffCandidate {
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    #[must_use]
    pub const fn source(&self) -> Option<LocatedClusterId> {
        self.source
    }

    #[must_use]
    pub const fn kind(&self) -> StaffCandidateKind {
        self.kind
    }

    #[must_use]
    pub const fn left(&self) -> f64 {
        self.left
    }

    #[must_use]
    pub const fn right(&self) -> f64 {
        self.right
    }

    #[must_use]
    pub const fn interline(&self) -> usize {
        self.interline
    }

    #[must_use]
    pub fn line_ids(&self) -> &[FilamentId] {
        &self.line_ids
    }

    /// The staff's line filaments, merged sections included.
    ///
    /// Prefer this over resolving [`Self::line_ids`] against the filament
    /// factory map: the ids name pre-merge filaments, these are the lines.
    #[must_use]
    pub fn line_filaments(&self) -> &[StaffFilament] {
        &self.line_filaments
    }

    #[must_use]
    pub const fn is_small(&self) -> bool {
        self.small
    }

    #[must_use]
    pub const fn is_short(&self) -> bool {
        self.short
    }

    /// Whether this staff was recovered from strong five-line raster evidence
    /// after ordinary comb clustering failed to assemble it.
    #[must_use]
    pub const fn is_tentative(&self) -> bool {
        self.tentative
    }

    /// Build a recoverable staff hypothesis directly from five traced line
    /// filaments. It deliberately has no cluster source: callers must keep the
    /// tentative bit visible rather than pretending that ordinary clustering
    /// accepted it.
    pub fn tentative_standard(
        interline: usize,
        line_ids: Vec<FilamentId>,
        line_filaments: Vec<StaffFilament>,
    ) -> Result<Self, LinesCoordinatorError> {
        if line_ids.len() != 5 || line_filaments.len() != 5 {
            return Err(LinesCoordinatorError::InvalidRecoveredCluster);
        }
        let mut starts = Vec::with_capacity(5);
        let mut stops = Vec::with_capacity(5);
        for line in &line_filaments {
            let geometry = line.geometry()?;
            starts.push(geometry.start().0);
            stops.push(geometry.stop().0);
        }
        starts.sort_by(|one, two| one.total_cmp(two));
        stops.sort_by(|one, two| one.total_cmp(two));
        Ok(Self {
            id: 0,
            source: None,
            kind: StaffCandidateKind::Standard,
            left: starts[2],
            right: stops[2],
            interline,
            line_ids,
            line_filaments,
            small: false,
            short: false,
            tentative: true,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterRejectionReason {
    TooShort,
    /// Five ordered filaments span implausibly different adjacent gaps and
    /// therefore mix ink from separate staves (or non-staff text rows).
    CrossStaffMixture,
    OneLineSlope,
    OneLineTrueLength,
    OneLineRightIndentation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RejectedCluster {
    source: LocatedClusterId,
    reason: ClusterRejectionReason,
}

impl RejectedCluster {
    #[must_use]
    pub const fn source(self) -> LocatedClusterId {
        self.source
    }

    #[must_use]
    pub const fn reason(self) -> ClusterRejectionReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinesCoordinatorParameters {
    global_slope: f64,
    minimum_staff_width: usize,
    foreground_thickness: Option<usize>,
    maximum_one_line_slope: f64,
    minimum_true_length_ratio: f64,
    maximum_right_indentation: f64,
}

impl LinesCoordinatorParameters {
    pub fn new(
        global_slope: f64,
        minimum_staff_width: usize,
        foreground_thickness: Option<usize>,
        maximum_one_line_slope: f64,
        minimum_true_length_ratio: f64,
        maximum_right_indentation: f64,
    ) -> Result<Self, LinesCoordinatorError> {
        if !global_slope.is_finite()
            || foreground_thickness == Some(0)
            || !maximum_one_line_slope.is_finite()
            || maximum_one_line_slope < 0.0
            || !minimum_true_length_ratio.is_finite()
            || minimum_true_length_ratio < 0.0
            || !maximum_right_indentation.is_finite()
            || maximum_right_indentation < 0.0
        {
            return Err(LinesCoordinatorError::InvalidParameters);
        }
        Ok(Self {
            global_slope,
            minimum_staff_width,
            foreground_thickness,
            maximum_one_line_slope,
            minimum_true_length_ratio,
            maximum_right_indentation,
        })
    }

    /// Return these line-layout thresholds with a newly measured sheet slope.
    pub fn with_global_slope(mut self, global_slope: f64) -> Result<Self, LinesCoordinatorError> {
        if !global_slope.is_finite() {
            return Err(LinesCoordinatorError::InvalidParameters);
        }
        self.global_slope = global_slope;
        Ok(self)
    }

    /// Override Java's minimum accepted five-line cluster width.
    ///
    /// Production parity uses 30 interlines.  Difficult piano pages can carry
    /// legitimate very short final systems, so an opt-in caller may retain a
    /// smaller cluster for late staff pairing and bar evidence instead of
    /// irreversibly discarding it here.
    pub fn with_minimum_staff_width(
        mut self,
        minimum_staff_width: usize,
    ) -> Result<Self, LinesCoordinatorError> {
        if minimum_staff_width == 0 {
            return Err(LinesCoordinatorError::InvalidParameters);
        }
        self.minimum_staff_width = minimum_staff_width;
        Ok(self)
    }

    #[must_use]
    pub const fn global_slope(self) -> f64 {
        self.global_slope
    }

    #[must_use]
    pub const fn minimum_staff_width(self) -> usize {
        self.minimum_staff_width
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinesRetrievalResult {
    primary: ClusterRetrievalResult,
    secondary: Option<ClusterRetrievalResult>,
    staffs: Vec<StaffCandidate>,
    rejected_clusters: Vec<RejectedCluster>,
}

impl LinesRetrievalResult {
    #[must_use]
    pub const fn primary(&self) -> &ClusterRetrievalResult {
        &self.primary
    }

    #[must_use]
    pub const fn secondary(&self) -> Option<&ClusterRetrievalResult> {
        self.secondary.as_ref()
    }

    #[must_use]
    pub fn staffs(&self) -> &[StaffCandidate] {
        &self.staffs
    }

    #[must_use]
    pub fn rejected_clusters(&self) -> &[RejectedCluster] {
        &self.rejected_clusters
    }

    /// Preserve late five-line hypotheses that ordinary clustering could not
    /// assemble, then restore deterministic page order and staff identities.
    pub fn retain_tentative_staffs(
        &mut self,
        tentative: impl IntoIterator<Item = StaffCandidate>,
        global_slope: f64,
    ) -> Result<(), LinesCoordinatorError> {
        self.staffs.extend(tentative);
        let mut ordinates = BTreeMap::new();
        for (index, staff) in self.staffs.iter().enumerate() {
            let line = staff
                .line_filaments()
                .first()
                .ok_or(LinesCoordinatorError::InvalidRecoveredCluster)?;
            let geometry = line.geometry()?;
            let x = (staff.left() + staff.right()) / 2.0;
            ordinates.insert(index, geometry.position_at(x)? - (global_slope * x));
        }
        let mut indexed = self.staffs.drain(..).enumerate().collect::<Vec<_>>();
        indexed.sort_by(|(one_index, _), (two_index, _)| {
            ordinates[one_index].total_cmp(&ordinates[two_index])
        });
        self.staffs = indexed
            .into_iter()
            .enumerate()
            .map(|(index, (_, mut staff))| {
                staff.id = index + 1;
                staff
            })
            .collect();
        Ok(())
    }
}

/// Execute main clustering, optional small-interline clustering, and
/// `buildStaves` in Java order. Both pass states are transactional together.
pub fn retrieve_staff_candidates(
    primary: &mut ClusterPassState,
    secondary: Option<&mut ClusterPassState>,
    parameters: LinesCoordinatorParameters,
) -> Result<LinesRetrievalResult, LinesCoordinatorError> {
    retrieve_staff_candidates_inner(primary, secondary, parameters, None)
}

/// Raw-adapter variant for a secondary state whose ID-sorted primary discards
/// have already been comb-followed.
/// Existing prepared callers deliberately retain Java's discard-only order.
pub fn retrieve_staff_candidates_with_secondary_order(
    primary: &mut ClusterPassState,
    secondary: &mut ClusterPassState,
    secondary_order: &[FilamentId],
    parameters: LinesCoordinatorParameters,
) -> Result<LinesRetrievalResult, LinesCoordinatorError> {
    retrieve_staff_candidates_inner(primary, Some(secondary), parameters, Some(secondary_order))
}

fn retrieve_staff_candidates_inner(
    primary: &mut ClusterPassState,
    secondary: Option<&mut ClusterPassState>,
    parameters: LinesCoordinatorParameters,
    explicit_secondary_order: Option<&[FilamentId]>,
) -> Result<LinesRetrievalResult, LinesCoordinatorError> {
    let mut next_primary = primary.clone();
    let mut next_secondary = secondary.as_deref().cloned();
    let primary_result = run_pass(&mut next_primary, None)?;
    let mut located = located_clusters(&next_primary, &primary_result, ClusterPass::Main)?;

    let mut secondary_result = None;
    let mut small_interline = None;
    let should_run_secondary = explicit_secondary_order.map_or_else(
        || !primary_result.discarded_filaments().is_empty(),
        |order| !order.is_empty(),
    );
    if should_run_secondary && let Some(state) = next_secondary.as_mut() {
        let mut java_order;
        let order = if let Some(order) = explicit_secondary_order {
            // Even when comb following has absorbed one of these into another
            // root, the exact primary-discard objects must remain available in
            // the secondary pass for provenance and later completion.
            for &id in primary_result.discarded_filaments() {
                if !state.filaments.contains_key(&id) {
                    return Err(LinesCoordinatorError::MissingSecondaryFilament(id));
                }
            }
            order
        } else {
            java_order = primary_result.discarded_filaments().to_vec();
            java_order.sort_by_key(|id| id.value());
            &java_order
        };
        for &id in order {
            if !state.filaments.contains_key(&id) {
                return Err(LinesCoordinatorError::MissingSecondaryFilament(id));
            }
        }
        let result = run_pass(state, Some(order))?;
        located.extend(located_clusters(state, &result, ClusterPass::Small)?);
        small_interline = Some(
            next_primary
                .parameters
                .interline()
                .min(state.parameters.interline()),
        );
        secondary_result = Some(result);
    }

    let mut rejected_clusters = purge_clusters(&mut located, parameters)?;
    rejected_clusters.extend(purge_cross_staff_mixtures(&mut located)?);
    sort_by_layout(&mut located, parameters.global_slope)?;
    rejected_clusters.extend(purge_right_indented_one_lines(
        &mut located,
        parameters.global_slope,
        parameters.maximum_right_indentation,
    )?);
    let mut staffs = allocate_staffs(&located, small_interline)?;
    mark_short_staves(&located, &mut staffs)?;

    *primary = next_primary;
    if let (Some(destination), Some(value)) = (secondary, next_secondary) {
        *destination = value;
    }
    Ok(LinesRetrievalResult {
        primary: primary_result,
        secondary: secondary_result,
        staffs,
        rejected_clusters,
    })
}

/// Execute one pass against a clone, exposing its discarded-filament set
/// without committing any ownership, cluster, or comb mutation. The raw
/// adapter uses this to construct Java's secondary pass lazily.
pub fn preview_cluster_pass(
    state: &ClusterPassState,
) -> Result<ClusterRetrievalResult, LinesCoordinatorError> {
    let mut preview = state.clone();
    run_pass(&mut preview, None)
}

fn run_pass(
    state: &mut ClusterPassState,
    order: Option<&[FilamentId]>,
) -> Result<ClusterRetrievalResult, LinesCoordinatorError> {
    let order = order.unwrap_or(&state.filament_order);
    Ok(retrieve_clusters(
        &mut state.ownership,
        &mut state.clusters,
        &state.filaments,
        &mut state.combs,
        order,
        &state.parameters,
    )?)
}

#[derive(Clone, Debug)]
struct LocatedCluster {
    id: LocatedClusterId,
    value: LineCluster,
}

fn located_clusters(
    state: &ClusterPassState,
    result: &ClusterRetrievalResult,
    pass: ClusterPass,
) -> Result<Vec<LocatedCluster>, LinesCoordinatorError> {
    result
        .cluster_ids()
        .iter()
        .copied()
        .map(|cluster| {
            let value = state.clusters.get(&cluster).cloned().ok_or(
                LinesCoordinatorError::MissingClusterValue(LocatedClusterId { pass, cluster }),
            )?;
            Ok(LocatedCluster {
                id: LocatedClusterId { pass, cluster },
                value,
            })
        })
        .collect()
}

fn purge_clusters(
    clusters: &mut Vec<LocatedCluster>,
    parameters: LinesCoordinatorParameters,
) -> Result<Vec<RejectedCluster>, LinesCoordinatorError> {
    let mut rejected = Vec::new();
    retain_with_reason(
        clusters,
        &mut rejected,
        ClusterRejectionReason::TooShort,
        |cluster| Ok(cluster.value.bounds()?.width < parameters.minimum_staff_width),
    )?;

    if parameters.global_slope == 0.0
        && let Some(line) = parameters.foreground_thickness
    {
        retain_with_reason(
            clusters,
            &mut rejected,
            ClusterRejectionReason::OneLineSlope,
            |cluster| {
                if !cluster.value.is_one_line() {
                    return Ok(false);
                }
                let bounds = cluster.value.bounds()?;
                let slope = (bounds.height as i64 - line as i64) as f64 / bounds.width as f64;
                Ok(slope.abs() > parameters.maximum_one_line_slope)
            },
        )?;
    }

    let maximum_true_length = clusters.iter().try_fold(0_usize, |maximum, cluster| {
        Ok::<_, LinesCoordinatorError>(maximum.max(cluster.value.true_length()?))
    })?;
    let minimum_true_length = (maximum_true_length as f64 * parameters.minimum_true_length_ratio)
        .round_ties_even() as usize;
    retain_with_reason(
        clusters,
        &mut rejected,
        ClusterRejectionReason::OneLineTrueLength,
        |cluster| {
            Ok(cluster.value.is_one_line() && cluster.value.true_length()? < minimum_true_length)
        },
    )?;
    Ok(rejected)
}

/// Reject only fully assembled five-line clusters whose physical spacing is
/// incompatible with one staff. This runs after all cluster expansion/merge
/// passes, rather than pruning a filament before the recognizer has had a
/// chance to complete its hypothesis.
fn purge_cross_staff_mixtures(
    clusters: &mut Vec<LocatedCluster>,
) -> Result<Vec<RejectedCluster>, LinesCoordinatorError> {
    let mut rejected = Vec::new();
    retain_with_reason(
        clusters,
        &mut rejected,
        ClusterRejectionReason::CrossStaffMixture,
        |cluster| five_line_cross_staff_mixture(&cluster.value),
    )?;
    Ok(rejected)
}

fn five_line_cross_staff_mixture(cluster: &LineCluster) -> Result<bool, LinesCoordinatorError> {
    if cluster.size() != 5 {
        return Ok(false);
    }
    let positions = cluster
        .lines()
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    if positions.windows(2).any(|pair| pair[1] != pair[0] + 1) {
        return Ok(true);
    }

    let bounds = cluster.bounds()?;
    let left = bounds.x as f64;
    let span = bounds.width.saturating_sub(1) as f64;
    let expected = cluster.interline() as f64;
    let mut implausible_samples = 0_usize;
    for fraction in [0.25, 0.5, 0.75] {
        let x = left + (span * fraction);
        let ordinates = cluster
            .lines()
            .map(|(_, line)| Ok(line.filament().geometry()?.position_at(x)?))
            .collect::<Result<Vec<_>, LinesCoordinatorError>>()?;
        let implausible = ordinates.windows(2).any(|pair| {
            let gap = pair[1] - pair[0];
            gap <= 0.0 || gap < 0.35 * expected || gap > 2.25 * expected
        });
        implausible_samples += usize::from(implausible);
    }
    Ok(implausible_samples >= 2)
}

fn retain_with_reason(
    clusters: &mut Vec<LocatedCluster>,
    rejected: &mut Vec<RejectedCluster>,
    reason: ClusterRejectionReason,
    mut predicate: impl FnMut(&LocatedCluster) -> Result<bool, LinesCoordinatorError>,
) -> Result<(), LinesCoordinatorError> {
    let mut survivors = Vec::with_capacity(clusters.len());
    for cluster in clusters.drain(..) {
        if predicate(&cluster)? {
            rejected.push(RejectedCluster {
                source: cluster.id,
                reason,
            });
        } else {
            survivors.push(cluster);
        }
    }
    *clusters = survivors;
    Ok(())
}

fn sort_by_layout(
    clusters: &mut [LocatedCluster],
    global_slope: f64,
) -> Result<(), LinesCoordinatorError> {
    let mut geometry = BTreeMap::new();
    for cluster in clusters.iter() {
        let bounds = cluster.value.bounds()?;
        let ordinate = cluster_deskewed_ordinate(&cluster.value, global_slope)?;
        geometry.insert(cluster.id_key(), (bounds, ordinate));
    }
    clusters.sort_by(|one, two| {
        let (one_bounds, one_ordinate) = geometry[&one.id_key()];
        let (two_bounds, two_ordinate) = geometry[&two.id_key()];
        if x_overlap(one_bounds, two_bounds) < 0 {
            center_x(one_bounds).cmp(&center_x(two_bounds))
        } else {
            one_ordinate
                .partial_cmp(&two_ordinate)
                .unwrap_or(Ordering::Equal)
        }
    });
    Ok(())
}

impl LocatedCluster {
    fn id_key(&self) -> (u8, u64) {
        let pass = match self.id.pass {
            ClusterPass::Main => 0,
            ClusterPass::Small => 1,
        };
        (pass, self.id.cluster.value())
    }
}

fn purge_right_indented_one_lines(
    clusters: &mut Vec<LocatedCluster>,
    global_slope: f64,
    maximum_right_indentation: f64,
) -> Result<Vec<RejectedCluster>, LinesCoordinatorError> {
    let mut half = vec![false; clusters.len()];
    for index in 0..clusters.len() {
        let bounds = clusters[index].value.bounds()?;
        half[index] = (index > 0 && x_overlap(bounds, clusters[index - 1].value.bounds()?) < 0)
            || (index + 1 < clusters.len()
                && x_overlap(bounds, clusters[index + 1].value.bounds()?) < 0);
    }
    let mut rejected_ids = Vec::new();
    for (index, cluster) in clusters.iter().enumerate() {
        if half[index] || !cluster.value.is_one_line() {
            continue;
        }
        let bounds = cluster.value.bounds()?;
        let mut below = None;
        for candidate in &clusters[index + 1..] {
            if x_overlap(bounds, candidate.value.bounds()?) > 0 {
                below = Some(candidate);
                break;
            }
        }
        let Some(below) = below else {
            continue;
        };
        let end = cluster.value.last_line().filament().geometry()?.stop();
        let below_end = below.value.first_line().filament().geometry()?.stop();
        if deskew_x(below_end, global_slope) - deskew_x(end, global_slope)
            > maximum_right_indentation
        {
            rejected_ids.push(cluster.id);
        }
    }
    clusters.retain(|cluster| !rejected_ids.contains(&cluster.id));
    Ok(rejected_ids
        .into_iter()
        .map(|source| RejectedCluster {
            source,
            reason: ClusterRejectionReason::OneLineRightIndentation,
        })
        .collect())
}

fn allocate_staffs(
    clusters: &[LocatedCluster],
    small_interline: Option<usize>,
) -> Result<Vec<StaffCandidate>, LinesCoordinatorError> {
    clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            let mut starts = Vec::with_capacity(cluster.value.size());
            let mut stops = Vec::with_capacity(cluster.value.size());
            let mut line_ids = Vec::with_capacity(cluster.value.size());
            let mut line_filaments = Vec::with_capacity(cluster.value.size());
            for (_, line) in cluster.value.lines() {
                let geometry = line.filament().geometry()?;
                starts.push(geometry.start().0);
                stops.push(geometry.stop().0);
                line_ids.push(line.primary_id());
                line_filaments.push(line.filament().clone());
            }
            starts.sort_by(|one, two| one.partial_cmp(two).unwrap_or(Ordering::Equal));
            stops.sort_by(|one, two| one.partial_cmp(two).unwrap_or(Ordering::Equal));
            let kind = match cluster.value.size() {
                5 => StaffCandidateKind::Standard,
                1 => StaffCandidateKind::OneLine,
                _ => StaffCandidateKind::Tablature,
            };
            Ok(StaffCandidate {
                id: index + 1,
                source: Some(cluster.id),
                kind,
                left: starts[starts.len() / 2],
                right: stops[stops.len() / 2],
                interline: cluster.value.interline(),
                line_ids,
                line_filaments,
                small: small_interline == Some(cluster.value.interline()),
                short: false,
                tentative: false,
            })
        })
        .collect()
}

fn mark_short_staves(
    clusters: &[LocatedCluster],
    staffs: &mut [StaffCandidate],
) -> Result<(), LinesCoordinatorError> {
    let spans = clusters
        .iter()
        .map(|cluster| {
            let first = cluster.value.first_line().filament().geometry()?.start().1;
            let last = cluster.value.last_line().filament().geometry()?.start().1;
            Ok::<_, LinesCoordinatorError>((first, last))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for index in 0..staffs.len() {
        staffs[index].short = spans
            .iter()
            .enumerate()
            .any(|(other, &span)| other != index && y_overlaps(spans[index], span));
    }
    Ok(())
}

fn x_overlap(one: Bounds, two: Bounds) -> i64 {
    let left = one.x.max(two.x) as i64;
    let right = (one.x + one.width).min(two.x + two.width) as i64;
    right - left
}

fn center_x(bounds: Bounds) -> usize {
    bounds.x + (bounds.width / 2)
}

fn deskew_x((x, y): (f64, f64), slope: f64) -> f64 {
    let angle = -slope.atan();
    (x * angle.cos()) - (y * angle.sin())
}

fn y_overlaps(one: (f64, f64), two: (f64, f64)) -> bool {
    one.0.max(two.0) < one.1.min(two.1)
}

#[derive(Clone, Debug, PartialEq)]
pub enum LinesCoordinatorError {
    InvalidParameters,
    InvalidRecoveredCluster,
    MissingRecoveredFilament(FilamentId),
    MissingSecondaryFilament(FilamentId),
    MissingClusterValue(LocatedClusterId),
    DuplicateClusterValue(ClusterId),
    Pipeline(ClusterPipelineError),
    Merge(ClusterMergeError),
    Ownership(ClusterOwnershipError),
    Cluster(LineClusterError),
    Filament(FilamentError),
}

macro_rules! coordinator_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for LinesCoordinatorError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

coordinator_from!(ClusterPipelineError, Pipeline);
coordinator_from!(ClusterMergeError, Merge);
coordinator_from!(ClusterOwnershipError, Ownership);
coordinator_from!(LineClusterError, Cluster);
coordinator_from!(FilamentError, Filament);

impl fmt::Display for LinesCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => {
                formatter.write_str("line coordinator parameters are invalid")
            }
            Self::InvalidRecoveredCluster => {
                formatter.write_str("recovered staff cluster must contain five distinct lines")
            }
            Self::MissingRecoveredFilament(id) => write!(
                formatter,
                "recovered staff cluster is missing filament {}",
                id.value()
            ),
            Self::MissingSecondaryFilament(id) => write!(
                formatter,
                "small-interline pass is missing discarded filament {}",
                id.value()
            ),
            Self::MissingClusterValue(id) => write!(
                formatter,
                "missing {:?} cluster value {}",
                id.pass,
                id.cluster.value()
            ),
            Self::DuplicateClusterValue(id) => {
                write!(
                    formatter,
                    "duplicate recovered cluster value {}",
                    id.value()
                )
            }
            Self::Pipeline(error) => write!(formatter, "cluster pass failed: {error}"),
            Self::Merge(error) => write!(formatter, "cluster layout failed: {error}"),
            Self::Ownership(error) => write!(formatter, "cluster ownership failed: {error}"),
            Self::Cluster(error) => write!(formatter, "cluster geometry failed: {error}"),
            Self::Filament(error) => write!(formatter, "filament geometry failed: {error}"),
        }
    }
}

impl Error for LinesCoordinatorError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{
        cluster_expand::ClusterExpansionParameters,
        cluster_finalize::ClusterConsistencyParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        filament_comb::FilamentComb,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, Section, build_sections},
    };

    fn sections() -> Vec<Section> {
        let mut table = RunTable::new(Orientation::Horizontal, 42, 62).unwrap();
        for y in [10, 50, 60] {
            table.add_run(y, Run::new(0, 40)).unwrap();
        }
        build_sections(&table, JunctionPolicy::All)
    }

    fn filament(interline: usize, section: Section) -> StaffFilament {
        let mut value = StaffFilament::new(interline).unwrap();
        value.add_section(section).unwrap();
        value
    }

    fn retrieval_parameters(
        interline: usize,
        desired_sizes: BTreeSet<usize>,
    ) -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        ClusterRetrievalParameters::new(
            interline,
            desired_sizes,
            expansion,
            merge,
            0.0,
            0.0,
            0.0,
            1,
            0,
            None::<ClusterConsistencyParameters>,
            1,
        )
        .unwrap()
    }

    fn five_line_cluster(seed: u64, ordinates: [usize; 5]) -> LineCluster {
        let mut table = RunTable::new(Orientation::Horizontal, 42, 128).unwrap();
        for y in ordinates {
            table.add_run(y, Run::new(0, 40)).unwrap();
        }
        let source = build_sections(&table, JunctionPolicy::All);
        let seed_id = FilamentId::new(seed);
        let mut cluster = LineCluster::new(10, seed_id, filament(10, source[0].clone())).unwrap();
        for (index, section) in source.into_iter().enumerate().skip(1) {
            cluster
                .include_line(
                    index as i32,
                    FilamentId::new(seed + index as u64),
                    filament(10, section),
                )
                .unwrap();
        }
        cluster
    }

    #[test]
    fn five_line_guard_rejects_cross_staff_gap_after_full_clustering() {
        let good_seed = FilamentId::new(10);
        let mixed_seed = FilamentId::new(20);
        let mut located = vec![
            LocatedCluster {
                id: LocatedClusterId {
                    pass: ClusterPass::Main,
                    cluster: ClusterId::from_seed(good_seed),
                },
                value: five_line_cluster(good_seed.value(), [10, 20, 30, 40, 50]),
            },
            LocatedCluster {
                id: LocatedClusterId {
                    pass: ClusterPass::Main,
                    cluster: ClusterId::from_seed(mixed_seed),
                },
                value: five_line_cluster(mixed_seed.value(), [10, 20, 30, 40, 100]),
            },
        ];

        let rejected = purge_cross_staff_mixtures(&mut located).unwrap();

        assert_eq!(located.len(), 1);
        assert_eq!(located[0].id.cluster(), ClusterId::from_seed(good_seed));
        assert_eq!(rejected.len(), 1);
        assert_eq!(
            rejected[0].source.cluster(),
            ClusterId::from_seed(mixed_seed)
        );
        assert_eq!(
            rejected[0].reason(),
            ClusterRejectionReason::CrossStaffMixture
        );
    }

    #[test]
    fn main_then_small_pass_builds_staffs_in_layout_order() {
        let source = sections();
        let mut primary_ownership = ClusterOwnership::new();
        let primary_filaments = [
            (FilamentId::new(1), filament(10, source[0].clone())),
            (FilamentId::new(2), filament(10, source[1].clone())),
            (FilamentId::new(3), filament(10, source[2].clone())),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        for (&id, value) in &primary_filaments {
            primary_ownership.register_filament(id, value).unwrap();
        }
        let mut comb = FilamentComb::new(0);
        comb.append_root(2, 50.0).unwrap();
        comb.append_root(3, 60.0).unwrap();
        let comb_id = CombId::new(1);
        primary_ownership.register_comb(comb_id, &comb).unwrap();
        let mut primary = ClusterPassState::new(
            primary_ownership,
            primary_filaments,
            BTreeMap::from([(comb_id, RecursiveCombSnapshot::from_comb(&comb))]),
            vec![FilamentId::new(1), FilamentId::new(2), FilamentId::new(3)],
            retrieval_parameters(10, BTreeSet::from([2])),
        );

        let mut secondary_ownership = ClusterOwnership::new();
        // Java reuses the exact main-interline filament object in the small
        // pass; only ClusterRetrievalParameters carries the small interline.
        let secondary_value = filament(10, source[0].clone());
        secondary_ownership
            .register_filament(FilamentId::new(1), &secondary_value)
            .unwrap();
        let mut secondary = ClusterPassState::new(
            secondary_ownership,
            BTreeMap::from([(FilamentId::new(1), secondary_value)]),
            BTreeMap::new(),
            vec![],
            retrieval_parameters(5, BTreeSet::from([1])),
        );
        let parameters = LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap();

        let result =
            retrieve_staff_candidates(&mut primary, Some(&mut secondary), parameters).unwrap();

        assert!(result.secondary().is_some());
        assert_eq!(result.staffs().len(), 2);
        assert_eq!(
            result.staffs()[0].source().unwrap().pass(),
            ClusterPass::Small
        );
        assert_eq!(result.staffs()[0].kind(), StaffCandidateKind::OneLine);
        assert!(result.staffs()[0].is_small());
        assert_eq!(result.staffs()[0].interline(), 5);
        assert_eq!(secondary.filaments()[&FilamentId::new(1)].interline(), 10);
        assert_eq!(
            result.staffs()[1].source().unwrap().pass(),
            ClusterPass::Main
        );
        assert_eq!(result.staffs()[1].kind(), StaffCandidateKind::Tablature);
        assert!(!result.staffs()[1].is_small());
        assert_eq!(result.staffs()[0].id(), 1);
        assert_eq!(result.staffs()[1].id(), 2);
    }

    #[test]
    fn minimum_staff_width_override_is_explicit_and_positive() {
        let parameters = LinesCoordinatorParameters::new(0.0, 300, None, 1.0, 0.0, 100.0)
            .unwrap()
            .with_minimum_staff_width(150)
            .unwrap();
        assert_eq!(parameters.minimum_staff_width(), 150);
        assert_eq!(
            parameters.with_minimum_staff_width(0),
            Err(LinesCoordinatorError::InvalidParameters)
        );
    }

    #[test]
    fn right_indentation_purge_runs_after_layout_sort() {
        let source = sections();
        let one_id = FilamentId::new(1);
        let below_a = FilamentId::new(2);
        let below_b = FilamentId::new(3);
        let one = LineCluster::new(10, one_id, filament(10, source[0].clone())).unwrap();
        let mut first_table = RunTable::new(Orientation::Horizontal, 102, 62).unwrap();
        first_table.add_run(50, Run::new(0, 100)).unwrap();
        let first = filament(
            10,
            build_sections(&first_table, JunctionPolicy::All).remove(0),
        );
        let mut below = LineCluster::new(10, below_a, first).unwrap();
        let mut table = RunTable::new(Orientation::Horizontal, 102, 62).unwrap();
        table.add_run(60, Run::new(0, 100)).unwrap();
        let extended = filament(10, build_sections(&table, JunctionPolicy::All).remove(0));
        below.include_line(1, below_b, extended).unwrap();
        let mut located = vec![
            LocatedCluster {
                id: LocatedClusterId {
                    pass: ClusterPass::Main,
                    cluster: ClusterId::from_seed(below_a),
                },
                value: below,
            },
            LocatedCluster {
                id: LocatedClusterId {
                    pass: ClusterPass::Main,
                    cluster: ClusterId::from_seed(one_id),
                },
                value: one,
            },
        ];
        sort_by_layout(&mut located, 0.0).unwrap();
        let rejected = purge_right_indented_one_lines(&mut located, 0.0, 20.0).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].source.cluster(), ClusterId::from_seed(one_id));
        assert_eq!(
            rejected[0].reason,
            ClusterRejectionReason::OneLineRightIndentation
        );
        assert_eq!(located.len(), 1);
    }
}
