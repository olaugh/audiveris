// SPDX-License-Identifier: AGPL-3.0-or-later

//! Adapter from a live horizontal section lag to the primary clustering pass.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    cluster_coordinator::{
        FollowCombsNetworkError, RecursiveCombSnapshot, follow_combs_network, popular_snapshot_size,
    },
    cluster_ownership::{ClusterOwnership, ClusterOwnershipError, CombId},
    cluster_pipeline::ClusterRetrievalParameters,
    comb_builder::{CombBuilderError, CombFilament, popular_comb_size, retrieve_combs},
    filament::FilamentError,
    filament_comb::{FilamentComb, FilamentCombError},
    filament_factory::{
        FilamentFactory, FilamentFactoryIdentityError, FilamentFactoryParams, OverlapParams,
    },
    line_cluster::FilamentId,
    line_rejection::{
        LineCandidate, LinePoint, LineRejectionError, LineRejectionParameters,
        reject_line_candidates, reject_line_candidates_projective,
        sort_by_reverse_horizontal_length,
    },
    line_short_sections::HorizontalSectionLag,
    lines_coordinator::{ClusterPassState, LinesCoordinatorError, StaffCandidate},
};

/// Pixel-resolved inputs needed before Java's primary `ClustersRetriever` pass.
#[derive(Clone, Debug)]
pub struct RawPrimaryPassParameters {
    pub factory: FilamentFactoryParams,
    pub overlap: OverlapParams,
    pub sampling_dx: usize,
    pub minimum_delta_y: isize,
    pub maximum_delta_y: isize,
    pub retrieval: ClusterRetrievalParameters,
    /// Opt-in local staff-line slope model for projective page captures.
    pub projective_slope: bool,
}

/// Sampling and cluster parameters for Java's optional small-interline pass.
///
/// The filaments themselves remain the objects created by the main-interline
/// factory. Only comb sampling and cluster interpretation use these values.
#[derive(Clone, Debug)]
pub struct RawSecondaryPassParameters {
    pub sampling_dx: usize,
    pub minimum_delta_y: isize,
    pub maximum_delta_y: isize,
    pub retrieval: ClusterRetrievalParameters,
}

/// Constructed pass plus the observable post-network root order.
#[derive(Clone, Debug)]
pub struct RawPrimaryPassBuild {
    state: ClusterPassState,
    survivor_order: Vec<FilamentId>,
    root_order: Vec<FilamentId>,
    popular_comb_size: Option<usize>,
    global_slope: f64,
    curved_ids: Vec<FilamentId>,
    sloped_ids: Vec<FilamentId>,
    sloped_filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
    factory_creation_ids: Vec<FilamentId>,
    recovered_composite_sources: Vec<FilamentId>,
    recovered_line_ids: Vec<FilamentId>,
    next_filament_id: FilamentId,
}

impl RawPrimaryPassBuild {
    #[must_use]
    pub const fn state(&self) -> &ClusterPassState {
        &self.state
    }

    /// All classified candidates in stable reverse-length order, before the
    /// comb network is allowed to merge roots. Curvature and slope outliers
    /// remain present; their IDs are available separately as diagnostics.
    #[must_use]
    pub fn survivor_order(&self) -> &[FilamentId] {
        &self.survivor_order
    }

    #[must_use]
    pub fn root_order(&self) -> &[FilamentId] {
        &self.root_order
    }

    #[must_use]
    pub const fn popular_comb_size(&self) -> Option<usize> {
        self.popular_comb_size
    }

    #[must_use]
    pub const fn global_slope(&self) -> f64 {
        self.global_slope
    }

    /// Filament IDs classified as curvature outliers, in pre-sort order.
    #[must_use]
    pub fn curved_ids(&self) -> &[FilamentId] {
        &self.curved_ids
    }

    /// Filament IDs classified as slope outliers, in post-length-sort order.
    #[must_use]
    pub fn sloped_ids(&self) -> &[FilamentId] {
        &self.sloped_ids
    }

    /// Legacy side channel for slope rejects excluded from primary clustering.
    ///
    /// The native pipeline now keeps every geometrically valid filament in the
    /// primary candidate pool, so this is empty. It remains temporarily for
    /// compatibility with the prepared-lines handoff.
    #[must_use]
    pub const fn sloped_filaments(&self) -> &BTreeMap<FilamentId, crate::filament::StaffFilament> {
        &self.sloped_filaments
    }

    /// Every ID registered by the factory, including swallowed cores and
    /// temporary expansion candidates.
    #[must_use]
    pub fn factory_creation_ids(&self) -> &[FilamentId] {
        &self.factory_creation_ids
    }

    /// Root filaments whose five periodic ridges were recovered after the
    /// comb network had irreversibly combined them.
    #[must_use]
    pub fn recovered_composite_sources(&self) -> &[FilamentId] {
        &self.recovered_composite_sources
    }

    /// Fresh identities allocated to the five recovered line filaments for
    /// each source in [`Self::recovered_composite_sources`].
    #[must_use]
    pub fn recovered_line_ids(&self) -> &[FilamentId] {
        &self.recovered_line_ids
    }

    /// Next ID for another factory sharing the same Java `FilamentIndex`.
    #[must_use]
    pub const fn next_filament_id(&self) -> FilamentId {
        self.next_filament_id
    }

    #[must_use]
    pub fn into_state(self) -> ClusterPassState {
        self.state
    }
}

/// Build Java's primary long-line state without mutating the source lag.
pub fn build_primary_cluster_pass(
    lag: &HorizontalSectionLag,
    parameters: RawPrimaryPassParameters,
) -> Result<RawPrimaryPassBuild, RawLineAdapterError> {
    build_primary_cluster_pass_with_first_filament_id(lag, parameters, FilamentId::new(1))
}

/// Identity-aware entry point for a sheet whose Java `FilamentIndex` may
/// already contain entities from an earlier factory.
pub fn build_primary_cluster_pass_with_first_filament_id(
    lag: &HorizontalSectionLag,
    parameters: RawPrimaryPassParameters,
    first_filament_id: FilamentId,
) -> Result<RawPrimaryPassBuild, RawLineAdapterError> {
    let factory = FilamentFactory::new(parameters.factory);
    let identified = factory.retrieve_filaments_with_ids(
        lag.sections(),
        parameters.overlap,
        first_filament_id.value(),
    )?;
    let factory_creation_ids = identified
        .creation_ids()
        .iter()
        .copied()
        .map(FilamentId::new)
        .collect::<Vec<_>>();
    let mut next_filament_id = identified.next_creation_id();
    let values = identified
        .into_survivors()
        .into_iter()
        .map(|entry| (entry.id(), entry.into_filament()))
        .collect();
    let rejection_parameters =
        LineRejectionParameters::java_defaults(parameters.factory.interline as f64);
    let FilteredRawFilaments {
        mut ownership,
        mut filaments,
        mut filament_order,
        global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
    } = filter_raw_filaments(values, rejection_parameters, parameters.projective_slope)?;

    let samples = filament_order
        .iter()
        .map(|id| {
            let filament = filaments
                .get(id)
                .ok_or(RawLineAdapterError::MissingFilament(*id))?;
            Ok(CombFilament::new(
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                filament.geometry()?,
            )?)
        })
        .collect::<Result<Vec<_>, RawLineAdapterError>>()?;
    let columns = retrieve_combs(
        lag.run_table().width(),
        parameters.sampling_dx,
        parameters.minimum_delta_y,
        parameters.maximum_delta_y,
        &samples,
    )?;
    let mut popular_size = popular_comb_size(&columns);
    let survivor_order = filament_order.clone();

    let mut combs = BTreeMap::new();
    let mut next_comb_id = 1_u64;
    for comb in columns.iter().flat_map(|column| column.combs()) {
        let id = CombId::new(next_comb_id);
        next_comb_id = next_comb_id
            .checked_add(1)
            .ok_or(RawLineAdapterError::IdentityOverflow)?;
        ownership.register_comb(id, comb)?;
        combs.insert(id, RecursiveCombSnapshot::from_comb(comb));
    }

    follow_combs_network(&mut ownership, &mut filaments, &combs, &mut filament_order)?;
    let recovery = recover_overgrown_staff_roots(
        &filaments,
        &filament_order,
        &parameters,
        &mut next_filament_id,
    )?;
    let recovered_groups = recovery.groups.clone();
    let recovered_source_set = recovery.source_ids.iter().copied().collect::<BTreeSet<_>>();
    let (recovered_composite_sources, recovered_line_ids) = if recovery.source_ids.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let followed_ownership = ownership.clone();
        filaments = recovery.filaments;
        filament_order = recovery.order;
        ownership = ClusterOwnership::new();
        for (&id, filament) in &filaments {
            ownership.register_filament(id, filament)?;
        }
        // Preserve the frozen post-follow graph everywhere except the
        // recovered composite's ancestry. Re-sampling the whole page here
        // subtly changes unrelated systems; retaining each original comb and
        // resolving its members to the already chosen roots makes recovery
        // local and deterministic.
        combs.clear();
        let mut rebuilt_index = 1_u64;
        for original in columns.iter().flat_map(|column| column.combs()) {
            let mut rebuilt = FilamentComb::new(original.column());
            let mut seen_roots = BTreeSet::new();
            for &raw_id in original.filament_ids() {
                let member = FilamentId::new(
                    u64::try_from(raw_id).map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                );
                let root = followed_ownership.filament_ancestor(member)?;
                if recovered_source_set.contains(&root) || !seen_roots.insert(root) {
                    continue;
                }
                let root_value = filaments
                    .get(&root)
                    .ok_or(RawLineAdapterError::MissingFilament(root))?;
                rebuilt.append_root(
                    usize::try_from(root.value())
                        .map_err(|_| RawLineAdapterError::IdentityOverflow)?,
                    root_value
                        .geometry()?
                        .position_at(f64::from(original.column()))?,
                )?;
            }
            if rebuilt.count() < 2 {
                continue;
            }
            let id = CombId::new(rebuilt_index);
            rebuilt_index = rebuilt_index
                .checked_add(1)
                .ok_or(RawLineAdapterError::IdentityOverflow)?;
            ownership.register_comb(id, &rebuilt)?;
            combs.insert(id, RecursiveCombSnapshot::from_comb(&rebuilt));
        }
        popular_size = popular_snapshot_size(&combs);
        (recovery.source_ids, recovery.recovered_ids)
    };
    let root_order = filament_order.clone();
    let mut state = ClusterPassState::new(
        ownership,
        filaments,
        combs,
        filament_order,
        parameters.retrieval,
    );
    for group in recovered_groups {
        state.seed_recovered_cluster(&group)?;
    }
    Ok(RawPrimaryPassBuild {
        state,
        survivor_order,
        root_order,
        popular_comb_size: popular_size,
        global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
        factory_creation_ids,
        recovered_composite_sources,
        recovered_line_ids,
        next_filament_id: FilamentId::new(next_filament_id),
    })
}

#[derive(Debug)]
struct RecoveredStaffRoots {
    filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
    order: Vec<FilamentId>,
    source_ids: Vec<FilamentId>,
    recovered_ids: Vec<FilamentId>,
    groups: Vec<[FilamentId; 5]>,
}

fn recover_overgrown_staff_roots(
    filaments: &BTreeMap<FilamentId, crate::filament::StaffFilament>,
    order: &[FilamentId],
    parameters: &RawPrimaryPassParameters,
    next_id: &mut u64,
) -> Result<RecoveredStaffRoots, RawLineAdapterError> {
    let mut recovered_filaments = BTreeMap::new();
    let mut recovered_order = Vec::with_capacity(order.len());
    let mut source_ids = Vec::new();
    let mut recovered_ids = Vec::new();
    let mut groups = Vec::new();

    for &id in order {
        let filament = filaments
            .get(&id)
            .ok_or(RawLineAdapterError::MissingFilament(id))?;
        let Some(line_groups) = split_staff_ridge_composite(filament, parameters)? else {
            recovered_filaments.insert(id, filament.clone());
            recovered_order.push(id);
            continue;
        };

        source_ids.push(id);
        for lines in line_groups {
            let mut group = [FilamentId::new(0); 5];
            for (index, line) in lines.into_iter().enumerate() {
                let line_id = FilamentId::new(*next_id);
                *next_id = next_id
                    .checked_add(1)
                    .ok_or(RawLineAdapterError::IdentityOverflow)?;
                recovered_filaments.insert(line_id, line);
                recovered_order.push(line_id);
                recovered_ids.push(line_id);
                group[index] = line_id;
            }
            groups.push(group);
        }
    }

    Ok(RecoveredStaffRoots {
        filaments: recovered_filaments,
        order: recovered_order,
        source_ids,
        recovered_ids,
        groups,
    })
}

#[derive(Clone, Copy, Debug)]
struct RidgeStaffPattern {
    first: usize,
    gap: usize,
    rank: usize,
}

impl RidgeStaffPattern {
    fn last(self) -> usize {
        self.first + (4 * self.gap)
    }

    fn overlaps(self, other: Self, margin: usize) -> bool {
        self.first <= other.last().saturating_add(margin)
            && other.first <= self.last().saturating_add(margin)
    }
}

/// Split a tall root when it contains one or more independently strong,
/// near-equidistant five-line patterns.
///
/// Dense piano engraving can join *both* staves through repeated stems and
/// chords before the comb graph sees them. The old recovery accepted only a
/// component no taller than nine interlines, so a fused grand staff vanished
/// before system formation. This version keeps every non-overlapping five-line
/// hypothesis in the component. Sections between the selected ridges remain
/// unassigned, so stems, slurs, beams, and the inter-staff bridge do not pollute
/// either tentative staff.
fn split_staff_ridge_composite(
    filament: &crate::filament::StaffFilament,
    parameters: &RawPrimaryPassParameters,
) -> Result<Option<Vec<Vec<crate::filament::StaffFilament>>>, RawLineAdapterError> {
    const LINE_COUNT: usize = 5;
    let interline = parameters.factory.interline;
    let bounds = filament.bounds()?;
    let minimum_gap = parameters.minimum_delta_y.max(1) as usize;
    let maximum_gap = parameters
        .maximum_delta_y
        .max(parameters.minimum_delta_y)
        .max(1) as usize;
    if bounds.width < 20 * interline
        || bounds.height < (LINE_COUNT - 1) * minimum_gap
        || bounds.height > 30 * interline
    {
        return Ok(None);
    }

    let mut row_coverage = vec![0_usize; bounds.height];
    for section in filament.sections() {
        for (offset, run) in section.runs().iter().enumerate() {
            let y = section.first_pos() + offset;
            row_coverage[y - bounds.y] = row_coverage[y - bounds.y].saturating_add(run.length);
        }
    }

    let radius = (interline / 6).max(1);
    let last_y = bounds.y + bounds.height - 1;
    let mut patterns = Vec::<RidgeStaffPattern>::new();
    for gap in minimum_gap..=maximum_gap {
        let span = (LINE_COUNT - 1) * gap;
        if span >= bounds.height {
            continue;
        }
        for first in bounds.y..=last_y - span {
            let mut scores = [0_usize; LINE_COUNT];
            for (index, score) in scores.iter_mut().enumerate() {
                let center = first + (index * gap);
                let start = center.saturating_sub(radius).max(bounds.y);
                let stop = (center + radius).min(last_y);
                *score = (start..=stop).map(|y| row_coverage[y - bounds.y]).sum();
            }
            let minimum = scores.iter().copied().min().unwrap_or(0);
            let total: usize = scores.iter().sum();
            let rank = minimum.saturating_mul(LINE_COUNT).saturating_add(total);
            if minimum >= (bounds.width * 2) / 5 {
                patterns.push(RidgeStaffPattern { first, gap, rank });
            }
        }
    }
    if patterns.is_empty() {
        return Ok(None);
    }

    patterns.sort_by(|one, two| {
        two.rank
            .cmp(&one.rank)
            .then_with(|| one.first.cmp(&two.first))
            .then_with(|| one.gap.cmp(&two.gap))
    });
    let separation_margin = (interline / 2).max(1);
    let mut selected = Vec::<RidgeStaffPattern>::new();
    for pattern in patterns {
        if selected
            .iter()
            .all(|existing| !pattern.overlaps(*existing, separation_margin))
        {
            selected.push(pattern);
        }
    }
    selected.sort_by_key(|pattern| pattern.first);
    // A tall component plausibly contains more than one staff. Recovering
    // only the strongest five ridges would delete the remaining evidence, so
    // keep the composite intact until at least two independent hypotheses are
    // available.
    if bounds.height > 9 * interline && selected.len() < 2 {
        return Ok(None);
    }

    let mut recovered = Vec::with_capacity(selected.len());
    for pattern in selected {
        let Some(lines) = recovered_staff_lines(filament, parameters, pattern)? else {
            continue;
        };
        recovered.push(lines);
    }
    if recovered.is_empty() || (bounds.height > 9 * interline && recovered.len() < 2) {
        return Ok(None);
    }
    Ok(Some(recovered))
}

fn recovered_staff_lines(
    filament: &crate::filament::StaffFilament,
    parameters: &RawPrimaryPassParameters,
    pattern: RidgeStaffPattern,
) -> Result<Option<Vec<crate::filament::StaffFilament>>, RawLineAdapterError> {
    const LINE_COUNT: usize = 5;
    let interline = parameters.factory.interline;
    let bounds = filament.bounds()?;
    let centers =
        std::array::from_fn::<_, LINE_COUNT, _>(|index| pattern.first + (index * pattern.gap));
    let assignment_radius = (0.34 * interline as f64).max(1.5);
    let maximum_section_height = ((interline as f64) * 0.5).ceil() as usize;
    let mut lines = (0..LINE_COUNT)
        .map(|_| crate::filament::StaffFilament::new(interline))
        .collect::<Result<Vec<_>, _>>()?;
    for section in filament.sections() {
        if section.bounds().height > maximum_section_height {
            continue;
        }
        let (_, y) = section.centroid_2d();
        let (index, distance) = centers
            .iter()
            .enumerate()
            .map(|(index, center)| (index, (y - *center as f64).abs()))
            .min_by(|one, two| one.1.total_cmp(&two.1))
            .expect("five ridge centers");
        if distance <= assignment_radius {
            lines[index].add_section(section.clone())?;
        }
    }

    let minimum_line_width = (bounds.width * 3) / 4;
    let minimum_true_length = (bounds.width * 2) / 5;
    for line in &lines {
        if line.bounds()?.width < minimum_line_width
            || line.true_length()? < minimum_true_length
            || line.geometry().is_err()
        {
            return Ok(None);
        }
    }

    let shared_left = lines
        .iter()
        .map(|line| line.bounds().map(|value| value.x))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .expect("five recovered lines");
    let shared_right = lines
        .iter()
        .map(|line| line.bounds().map(|value| value.x + value.width - 1))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .expect("five recovered lines");
    if shared_right <= shared_left || shared_right - shared_left < minimum_line_width {
        return Ok(None);
    }
    let minimum_gap = parameters.minimum_delta_y.max(1) as f64;
    let maximum_gap = parameters
        .maximum_delta_y
        .max(parameters.minimum_delta_y)
        .max(1) as f64;
    for sample in 0..=6 {
        let x = shared_left as f64 + ((shared_right - shared_left) as f64 * sample as f64 / 6.0);
        let ordinates = lines
            .iter()
            .map(|line| line.geometry()?.position_at(x))
            .collect::<Result<Vec<_>, _>>()?;
        if ordinates.windows(2).any(|pair| {
            let observed = pair[1] - pair[0];
            observed < minimum_gap * 0.7 || observed > maximum_gap * 1.3
        }) {
            return Ok(None);
        }
    }

    Ok(Some(lines))
}

/// Recover strong five-line raster fields that survived run extraction but
/// were never assembled by the comb network.
///
/// This is deliberately late and additive. Ordinary accepted staves win; a
/// periodic raster field becomes a tentative candidate only when no accepted
/// five-line staff occupies the same ordinate. That keeps title text and
/// already-stable geometry from being promoted merely because early pruning
/// was relaxed.
pub fn recover_tentative_staffs_from_ridges(
    lag: &HorizontalSectionLag,
    accepted: &[StaffCandidate],
    parameters: &RawPrimaryPassParameters,
    next_id: &mut u64,
) -> Result<Vec<StaffCandidate>, RawLineAdapterError> {
    const LINE_COUNT: usize = 5;
    let table = lag.run_table();
    let interline = parameters.factory.interline;
    let minimum_gap = parameters.minimum_delta_y.max(1) as usize;
    let maximum_gap = parameters
        .maximum_delta_y
        .max(parameters.minimum_delta_y)
        .max(1) as usize;
    let radius = (interline / 6).max(1);
    let row_coverage = (0..table.height())
        .map(|y| {
            table
                .sequence(y)
                .unwrap_or_default()
                .iter()
                .map(|run| run.length)
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    let trace = std::env::var("AUDIVERIS_TRACE_TENTATIVE_STAVES")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"));
    if trace {
        let mut strongest = row_coverage.iter().copied().enumerate().collect::<Vec<_>>();
        strongest.sort_by_key(|(_, coverage)| std::cmp::Reverse(*coverage));
        eprintln!(
            "GRID ridge trace: strongest rows {:?}",
            strongest.into_iter().take(24).collect::<Vec<_>>()
        );
    }
    let mut patterns = Vec::<RidgeStaffPattern>::new();
    for gap in minimum_gap..=maximum_gap {
        let span = (LINE_COUNT - 1) * gap;
        if span >= table.height() {
            continue;
        }
        for first in 0..table.height() - span {
            let mut scores = [0_usize; LINE_COUNT];
            for (index, score) in scores.iter_mut().enumerate() {
                let center = first + (index * gap);
                let start = center.saturating_sub(radius);
                let stop = (center + radius).min(table.height() - 1);
                *score = row_coverage[start..=stop].iter().sum();
            }
            let minimum = scores.iter().copied().min().unwrap_or(0);
            // This is only a proposal gate. Dense engraving and page curl can
            // fragment one of the five staff rows below the ordinary 40%
            // cluster threshold even though the later per-line geometry is
            // coherent. Keep quarter-page ridge evidence here and let the
            // five independently materialized lines decide below.
            if minimum < table.width() / 4 {
                continue;
            }
            patterns.push(RidgeStaffPattern {
                first,
                gap,
                rank: minimum
                    .saturating_mul(LINE_COUNT)
                    .saturating_add(scores.iter().sum()),
            });
        }
    }
    patterns.sort_by(|one, two| {
        two.rank
            .cmp(&one.rank)
            .then_with(|| one.first.cmp(&two.first))
            .then_with(|| one.gap.cmp(&two.gap))
    });
    if trace {
        eprintln!("GRID ridge trace: {} thresholded patterns", patterns.len());
    }
    let mut selected = Vec::<RidgeStaffPattern>::new();
    for pattern in patterns {
        if selected
            .iter()
            .all(|existing| !pattern.overlaps(*existing, (interline / 2).max(1)))
        {
            selected.push(pattern);
        }
    }
    selected.sort_by_key(|pattern| pattern.first);
    if trace {
        eprintln!("GRID ridge trace: selected patterns {selected:?}");
    }

    let mut tentative = Vec::new();
    for pattern in selected {
        let center_y = pattern.first as f64 + (2.0 * pattern.gap as f64);
        let already_accepted = accepted.iter().any(|staff| {
            if staff.line_filaments().len() != LINE_COUNT {
                return false;
            }
            let x = (staff.left() + staff.right()) / 2.0;
            staff.line_filaments()[2]
                .geometry()
                .and_then(|geometry| geometry.position_at(x))
                // Dense noteheads can make the strongest periodic window
                // slide by one staff space. Treat that as the same accepted
                // staff rather than emitting a shifted duplicate hypothesis.
                .is_ok_and(|y| (y - center_y).abs() <= 2.0 * interline as f64)
        });
        if already_accepted {
            continue;
        }

        let centers =
            std::array::from_fn::<_, LINE_COUNT, _>(|index| pattern.first + (index * pattern.gap));
        let assignment_radius = (0.34 * interline as f64).max(1.5);
        let maximum_section_height = ((interline as f64) * 0.5).ceil() as usize;
        let minimum_section_width = (2 * interline).max(1);
        let mut lines = (0..LINE_COUNT)
            .map(|_| crate::filament::StaffFilament::new(interline))
            .collect::<Result<Vec<_>, _>>()?;
        for section in lag.sections() {
            let bounds = section.bounds();
            if bounds.height > maximum_section_height || bounds.width < minimum_section_width {
                continue;
            }
            let (_, y) = section.centroid_2d();
            let (index, distance) = centers
                .iter()
                .enumerate()
                .map(|(index, center)| (index, (y - *center as f64).abs()))
                .min_by(|one, two| one.1.total_cmp(&two.1))
                .expect("five raster ridge centers");
            if distance <= assignment_radius {
                lines[index].add_section(section.clone())?;
            }
        }
        // Requiring every recovered line to span three fifths of the entire
        // raster loses genuine indented or locally occluded piano staves. The
        // path remains tentative and still requires all five lines, coherent
        // spacing, valid geometry, and a quarter-page of actual ink each.
        let minimum_line_width = (table.width() * 2) / 5;
        // Dense chords, aligned stems, and clefs can fragment the physical
        // staff ink while all five ridge spans and the raster periodicity stay
        // unequivocal. This is a tentative path: require one quarter-page of
        // actual line ink, not the ordinary two-fifths acceptance threshold.
        let minimum_true_length = table.width() / 4;
        let line_metrics = lines
            .iter()
            .map(|line| {
                (
                    line.bounds().ok().map(|bounds| bounds.width),
                    line.true_length().ok(),
                    line.geometry().is_ok(),
                )
            })
            .collect::<Vec<_>>();
        if line_metrics.iter().any(|(width, true_length, geometry)| {
            width.is_none_or(|width| width < minimum_line_width)
                || true_length.is_none_or(|length| length < minimum_true_length)
                || !geometry
        }) {
            if trace {
                eprintln!(
                    "GRID ridge trace: rejected pattern {pattern:?} line metrics {line_metrics:?}"
                );
            }
            continue;
        }
        let line_ids = (0..LINE_COUNT)
            .map(|_| {
                let id = FilamentId::new(*next_id);
                *next_id = next_id
                    .checked_add(1)
                    .ok_or(RawLineAdapterError::IdentityOverflow)?;
                Ok(id)
            })
            .collect::<Result<Vec<_>, RawLineAdapterError>>()?;
        tentative.push(StaffCandidate::tentative_standard(
            interline, line_ids, lines,
        )?);
    }
    Ok(tentative)
}

/// Lazily construct Java's secondary/small-interline clustering state.
///
/// `primary_discarded` comes from a transactional preview of the primary
/// cluster pass. Every value is cloned from
/// the original main-interline [`StaffFilament`], preserving its ID, sections
/// and geometry; no filament is rebuilt with the small interline.
pub fn build_secondary_cluster_pass(
    picture_width: usize,
    primary: &ClusterPassState,
    primary_discarded: &[FilamentId],
    parameters: RawSecondaryPassParameters,
) -> Result<ClusterPassState, RawLineAdapterError> {
    let mut filaments = BTreeMap::new();
    for &id in primary_discarded {
        let filament = primary
            .filaments()
            .get(&id)
            .ok_or(RawLineAdapterError::MissingFilament(id))?;
        if filaments.insert(id, filament.clone()).is_some() {
            return Err(RawLineAdapterError::DuplicateSecondaryFilament(id));
        }
    }
    // Java sorts the second-pass input by entity ID before constructing its
    // comb network. BTreeMap iteration gives that exact deterministic order.
    let mut filament_order = filaments.keys().copied().collect::<Vec<_>>();
    let mut ownership = ClusterOwnership::new();
    for (&id, filament) in &filaments {
        ownership.register_filament(id, filament)?;
    }
    let samples = filament_order
        .iter()
        .map(|id| {
            let filament = filaments
                .get(id)
                .ok_or(RawLineAdapterError::MissingFilament(*id))?;
            let raw_id =
                usize::try_from(id.value()).map_err(|_| RawLineAdapterError::IdentityOverflow)?;
            Ok(CombFilament::new(raw_id, raw_id, filament.geometry()?)?)
        })
        .collect::<Result<Vec<_>, RawLineAdapterError>>()?;
    let columns = retrieve_combs(
        picture_width,
        parameters.sampling_dx,
        parameters.minimum_delta_y,
        parameters.maximum_delta_y,
        &samples,
    )?;
    let mut combs = BTreeMap::new();
    let mut next_comb_id = 1_u64;
    for comb in columns.iter().flat_map(|column| column.combs()) {
        let id = CombId::new(next_comb_id);
        next_comb_id = next_comb_id
            .checked_add(1)
            .ok_or(RawLineAdapterError::IdentityOverflow)?;
        ownership.register_comb(id, comb)?;
        combs.insert(id, RecursiveCombSnapshot::from_comb(comb));
    }
    follow_combs_network(&mut ownership, &mut filaments, &combs, &mut filament_order)?;

    Ok(ClusterPassState::new(
        ownership,
        filaments,
        combs,
        filament_order,
        parameters.retrieval,
    ))
}

#[derive(Clone, Debug)]
struct FilteredRawFilaments {
    ownership: ClusterOwnership,
    filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
    filament_order: Vec<FilamentId>,
    global_slope: f64,
    curved_ids: Vec<FilamentId>,
    sloped_ids: Vec<FilamentId>,
    sloped_filaments: BTreeMap<FilamentId, crate::filament::StaffFilament>,
}

fn filter_raw_filaments(
    values: Vec<(u64, crate::filament::StaffFilament)>,
    parameters: LineRejectionParameters,
    projective_slope: bool,
) -> Result<FilteredRawFilaments, RawLineAdapterError> {
    let mut all = BTreeMap::new();
    let mut candidates = Vec::with_capacity(values.len());
    for (factory_id, filament) in values {
        let raw_id =
            usize::try_from(factory_id).map_err(|_| RawLineAdapterError::IdentityOverflow)?;
        let id = FilamentId::new(factory_id);
        let geometry = filament.geometry()?;
        let start = geometry.start();
        let stop = geometry.stop();
        let x_mid = (start.0 + stop.0) / 2.0;
        candidates.push(LineCandidate::new(
            raw_id,
            LinePoint::new(start.0, start.1),
            LinePoint::new(stop.0, stop.1),
            geometry.position_at(x_mid)?,
            filament.bounds()?.width,
        )?);
        all.insert(id, filament);
    }

    let report = if projective_slope {
        reject_line_candidates_projective(candidates, parameters)?
    } else {
        reject_line_candidates(candidates, parameters)?
    };
    let curved_ids = report
        .curved
        .iter()
        .map(|rejected| FilamentId::new(rejected.candidate.id() as u64))
        .collect::<Vec<_>>();
    let sloped_ids = report
        .sloped
        .iter()
        .map(|rejected| FilamentId::new(rejected.candidate.id() as u64))
        .collect::<Vec<_>>();
    // Curvature and slope are weak priors, not proof that a filament cannot be
    // part of a staff. Dropping either category here is irrecoverable: no later
    // dense trace can grow a staff that never reaches comb formation. Preserve
    // the diagnostic categories above, but let all geometrically valid factory
    // filaments participate in clustering. The common reverse-length order
    // keeps seed selection deterministic and retains Java's survivor priority.
    let mut retained_candidates = report.survivors;
    retained_candidates.extend(report.curved.iter().map(|rejected| rejected.candidate));
    retained_candidates.extend(report.sloped.iter().map(|rejected| rejected.candidate));
    sort_by_reverse_horizontal_length(&mut retained_candidates);
    let filament_order = retained_candidates
        .iter()
        .map(|candidate| FilamentId::new(candidate.id() as u64))
        .collect::<Vec<_>>();

    let mut ownership = ClusterOwnership::new();
    let mut filaments = BTreeMap::new();
    for id in &filament_order {
        let filament = all
            .remove(id)
            .ok_or(RawLineAdapterError::MissingFilament(*id))?;
        ownership.register_filament(*id, &filament)?;
        filaments.insert(*id, filament);
    }
    // No candidate is held outside the clustering pass. Keeping the legacy map
    // empty also prevents a filament rejected by final cluster validation from
    // being appended twice to the later discarded-filament completion input.
    let sloped_filaments = BTreeMap::new();

    Ok(FilteredRawFilaments {
        ownership,
        filaments,
        filament_order,
        global_slope: report.global_slope,
        curved_ids,
        sloped_ids,
        sloped_filaments,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawLineAdapterError {
    Filament(FilamentError),
    FactoryIdentity(FilamentFactoryIdentityError),
    Rejection(LineRejectionError),
    Comb(CombBuilderError),
    FilamentComb(FilamentCombError),
    Ownership(ClusterOwnershipError),
    Follow(FollowCombsNetworkError),
    Coordinator(LinesCoordinatorError),
    MissingFilament(FilamentId),
    DuplicateSecondaryFilament(FilamentId),
    IdentityOverflow,
}

impl From<FilamentError> for RawLineAdapterError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl From<FilamentFactoryIdentityError> for RawLineAdapterError {
    fn from(value: FilamentFactoryIdentityError) -> Self {
        Self::FactoryIdentity(value)
    }
}

impl From<LineRejectionError> for RawLineAdapterError {
    fn from(value: LineRejectionError) -> Self {
        Self::Rejection(value)
    }
}

impl From<CombBuilderError> for RawLineAdapterError {
    fn from(value: CombBuilderError) -> Self {
        Self::Comb(value)
    }
}

impl From<FilamentCombError> for RawLineAdapterError {
    fn from(value: FilamentCombError) -> Self {
        Self::FilamentComb(value)
    }
}

impl From<ClusterOwnershipError> for RawLineAdapterError {
    fn from(value: ClusterOwnershipError) -> Self {
        Self::Ownership(value)
    }
}

impl From<FollowCombsNetworkError> for RawLineAdapterError {
    fn from(value: FollowCombsNetworkError) -> Self {
        Self::Follow(value)
    }
}

impl From<LinesCoordinatorError> for RawLineAdapterError {
    fn from(value: LinesCoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}

impl fmt::Display for RawLineAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament(error) => write!(formatter, "raw filament construction failed: {error}"),
            Self::FactoryIdentity(error) => {
                write!(formatter, "raw filament identity failed: {error}")
            }
            Self::Rejection(error) => write!(formatter, "raw filament rejection failed: {error}"),
            Self::Comb(error) => write!(formatter, "raw comb sampling failed: {error}"),
            Self::FilamentComb(error) => write!(formatter, "raw comb rebuild failed: {error}"),
            Self::Ownership(error) => write!(formatter, "raw ownership failed: {error}"),
            Self::Follow(error) => write!(formatter, "raw comb network failed: {error}"),
            Self::Coordinator(error) => write!(formatter, "raw staff recovery failed: {error}"),
            Self::MissingFilament(id) => write!(formatter, "missing raw filament {}", id.value()),
            Self::DuplicateSecondaryFilament(id) => {
                write!(formatter, "duplicate secondary raw filament {}", id.value())
            }
            Self::IdentityOverflow => formatter.write_str("raw adapter identity overflow"),
        }
    }
}

impl Error for RawLineAdapterError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{
        cluster_expand::ClusterExpansionParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        filament::StaffFilament,
        lines_coordinator::{LinesCoordinatorParameters, retrieve_staff_candidates},
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn retrieval_parameters() -> ClusterRetrievalParameters {
        retrieval_parameters_for(10, BTreeSet::from([5]))
    }

    fn retrieval_parameters_for(
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
            None,
            1,
        )
        .unwrap()
    }

    fn one_line_filament(interline: usize, y: usize, x: usize, length: usize) -> StaffFilament {
        let mut table = RunTable::new(Orientation::Horizontal, x + length + 1, y + 2).unwrap();
        table.add_run(y, Run::new(x, length)).unwrap();
        let section = build_sections(&table, JunctionPolicy::All).remove(0);
        let mut filament = StaffFilament::new(interline).unwrap();
        filament.add_section(section).unwrap();
        filament
    }

    fn parameters() -> RawPrimaryPassParameters {
        RawPrimaryPassParameters {
            factory: FilamentFactoryParams {
                interline: 10,
                min_core_section_length: 1,
                min_section_aspect: 1.0,
                max_coord_gap: 5.0,
                max_pos_gap: 1.0,
                max_pos_gap_for_slope: 1.0,
                max_gap_slope: 0.1,
                min_length_for_delta_slope: 10.0,
                max_delta_slope: 0.1,
            },
            overlap: OverlapParams {
                probe_width: 2,
                max_overlap_delta_pos: 1.0,
                max_thickness: 2.0,
                max_overlap_space: 0.0,
                max_expansion_space: 0.0,
                max_involving_length: 10.0,
                max_consistent_ratio: 2.0,
            },
            sampling_dx: 20,
            minimum_delta_y: 9,
            maximum_delta_y: 11,
            retrieval: retrieval_parameters(),
            projective_slope: false,
        }
    }

    fn identity_parameters() -> RawPrimaryPassParameters {
        let mut parameters = parameters();
        parameters.factory = FilamentFactoryParams {
            interline: 2,
            min_core_section_length: 6,
            min_section_aspect: 3.0,
            max_coord_gap: 10.0,
            max_pos_gap: 2.0,
            max_pos_gap_for_slope: 0.5,
            max_gap_slope: 0.5,
            min_length_for_delta_slope: 100.0,
            max_delta_slope: 0.01,
        };
        parameters.overlap = OverlapParams {
            probe_width: 2,
            max_overlap_delta_pos: 2.0,
            max_thickness: 4.0,
            max_overlap_space: 1.0,
            max_expansion_space: 0.0,
            max_involving_length: 10.0,
            max_consistent_ratio: 1.7,
        };
        parameters.sampling_dx = 10;
        parameters.minimum_delta_y = 1;
        parameters.maximum_delta_y = 2;
        parameters
    }

    fn rejection_fixture_filaments() -> Vec<StaffFilament> {
        let mut runs = RunTable::new(Orientation::Horizontal, 130, 200).unwrap();
        // ID 1: short horizontal, accepted by the sloped-page tolerance.
        runs.add_run(10, Run::new(0, 30)).unwrap();
        // ID 2: long bowed candidate, removed before slope estimation.
        for (y, x) in [(40, 0), (50, 40), (50, 80), (40, 115)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 3: longest line establishes a positive global slope.
        for (y, x) in [(90, 0), (92, 40), (94, 80), (96, 115)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 4: ordinary-length positive slope outlier.
        for (y, x) in [(130, 0), (135, 40), (139, 75)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }
        // ID 5: shorter line consistent with the global slope.
        for (y, x) in [(170, 0), (172, 50), (175, 95)] {
            runs.add_run(y, Run::new(x, 5)).unwrap();
        }

        let sections = build_sections(&runs, JunctionPolicy::All);
        let mut filaments = (0..5)
            .map(|_| StaffFilament::new(10).unwrap())
            .collect::<Vec<_>>();
        for section in sections {
            let index = match section.bounds().y {
                0..=29 => 0,
                30..=69 => 1,
                70..=119 => 2,
                120..=159 => 3,
                _ => 4,
            };
            filaments[index].add_section(section).unwrap();
        }
        filaments
    }

    #[test]
    fn split_middle_raw_lag_forms_one_five_line_staff_and_preserves_sections() {
        let mut runs = RunTable::new(Orientation::Horizontal, 100, 61).unwrap();
        for y in [10, 20, 40, 50] {
            runs.add_run(y, Run::new(0, 100)).unwrap();
        }
        runs.add_run(30, Run::new(0, 21)).unwrap();
        runs.add_run(30, Run::new(40, 60)).unwrap();
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let middle_section_ids = lag
            .sections()
            .iter()
            .filter(|section| section.bounds().y == 30)
            .map(|section| section.id())
            .collect::<Vec<_>>();
        assert_eq!(middle_section_ids.len(), 2);

        let built = build_primary_cluster_pass(&lag, parameters()).unwrap();
        assert_eq!(
            built.survivor_order(),
            [
                FilamentId::new(1),
                FilamentId::new(2),
                FilamentId::new(5),
                FilamentId::new(6),
                FilamentId::new(4),
                FilamentId::new(3),
            ]
        );
        assert_eq!(
            built.factory_creation_ids(),
            [
                FilamentId::new(1),
                FilamentId::new(2),
                FilamentId::new(3),
                FilamentId::new(4),
                FilamentId::new(5),
                FilamentId::new(6),
            ]
        );
        assert_eq!(built.next_filament_id(), FilamentId::new(7));
        assert_eq!(built.root_order().len(), 5);
        assert_eq!(built.popular_comb_size(), Some(5));
        assert_eq!(built.global_slope(), 0.0);
        assert!(built.curved_ids().is_empty());
        assert!(built.sloped_ids().is_empty());
        assert!(built.sloped_filaments().is_empty());
        let first_owner = built
            .state()
            .ownership()
            .section_owner(middle_section_ids[0]);
        let second_owner = built
            .state()
            .ownership()
            .section_owner(middle_section_ids[1]);
        assert_eq!(first_owner, second_owner);
        let owner = first_owner.unwrap();
        assert_eq!(built.state().filaments()[&owner].sections().len(), 2);

        let mut state = built.into_state();
        let result = retrieve_staff_candidates(
            &mut state,
            None,
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(result.staffs().len(), 1);
        assert_eq!(result.staffs()[0].line_ids().len(), 5);
    }

    #[test]
    fn strongly_bowed_five_line_staff_survives_early_classification() {
        let mut runs = RunTable::new(Orientation::Horizontal, 118, 70).unwrap();
        // Five parallel, piecewise-horizontal arches. Their roughly 0.48-rad
        // central rotation is far beyond the legacy 0.1-rad early gate, but
        // their mutual one-interline spacing makes them a strong staff group.
        let offsets = [0, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0];
        for base_y in [10, 20, 30, 40, 50] {
            for (index, dy) in offsets.into_iter().enumerate() {
                runs.add_run(base_y + dy, Run::new(index * 7, 9)).unwrap();
            }
        }
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let built = build_primary_cluster_pass(&lag, parameters()).unwrap();

        // The factory represents each arch as two monotone filaments. Five of
        // those halves are classified as slope outliers; all ten still reach
        // the comb network and can form one staff.
        assert_eq!(built.sloped_ids().len(), 5);
        assert_eq!(built.survivor_order().len(), 10);
        assert!(built.sloped_filaments().is_empty());

        let mut state = built.into_state();
        let result = retrieve_staff_candidates(
            &mut state,
            None,
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(result.staffs().len(), 1);
        assert_eq!(result.staffs()[0].line_ids().len(), 5);
    }

    #[test]
    fn overgrown_five_ridge_root_is_split_and_seeded_through_late_validation() {
        let mut runs = RunTable::new(Orientation::Horizontal, 240, 80).unwrap();
        for y in [15, 25, 35, 45, 55] {
            runs.add_run(y, Run::new(10, 220)).unwrap();
        }
        let mut composite = StaffFilament::new(10).unwrap();
        for section in build_sections(&runs, JunctionPolicy::All) {
            composite.add_section(section).unwrap();
        }

        let source = FilamentId::new(1);
        let mut next_id = 2;
        let recovered = recover_overgrown_staff_roots(
            &BTreeMap::from([(source, composite)]),
            &[source],
            &parameters(),
            &mut next_id,
        )
        .unwrap();
        assert_eq!(recovered.source_ids, [source]);
        assert_eq!(recovered.groups.len(), 1);
        assert_eq!(recovered.recovered_ids.len(), 5);
        assert_eq!(next_id, 7);
        let group = recovered.groups[0];

        let mut ownership = ClusterOwnership::new();
        for (&id, filament) in &recovered.filaments {
            ownership.register_filament(id, filament).unwrap();
        }
        let mut state = ClusterPassState::new(
            ownership,
            recovered.filaments,
            BTreeMap::new(),
            recovered.order,
            retrieval_parameters(),
        );
        state.seed_recovered_cluster(&group).unwrap();
        let result = retrieve_staff_candidates(
            &mut state,
            None,
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap(),
        )
        .unwrap();

        assert_eq!(result.staffs().len(), 1);
        assert_eq!(result.staffs()[0].line_ids(), group);
    }

    #[test]
    fn fused_grand_staff_root_preserves_two_tentative_five_line_hypotheses() {
        let mut runs = RunTable::new(Orientation::Horizontal, 240, 170).unwrap();
        for y in [15, 25, 35, 45, 55, 105, 115, 125, 135, 145] {
            runs.add_run(y, Run::new(10, 220)).unwrap();
        }
        let mut composite = StaffFilament::new(10).unwrap();
        for section in build_sections(&runs, JunctionPolicy::All) {
            composite.add_section(section).unwrap();
        }

        let source = FilamentId::new(1);
        let mut next_id = 2;
        let recovered = recover_overgrown_staff_roots(
            &BTreeMap::from([(source, composite)]),
            &[source],
            &parameters(),
            &mut next_id,
        )
        .unwrap();
        assert_eq!(recovered.source_ids, [source]);
        assert_eq!(recovered.groups.len(), 2);
        assert_eq!(recovered.recovered_ids.len(), 10);
        assert_eq!(next_id, 12);

        let mut ownership = ClusterOwnership::new();
        for (&id, filament) in &recovered.filaments {
            ownership.register_filament(id, filament).unwrap();
        }
        let mut state = ClusterPassState::new(
            ownership,
            recovered.filaments,
            BTreeMap::new(),
            recovered.order,
            retrieval_parameters(),
        );
        for group in &recovered.groups {
            state.seed_recovered_cluster(group).unwrap();
        }
        let result = retrieve_staff_candidates(
            &mut state,
            None,
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap(),
        )
        .unwrap();

        assert_eq!(result.staffs().len(), 2);
        assert_eq!(result.staffs()[0].line_ids(), recovered.groups[0]);
        assert_eq!(result.staffs()[1].line_ids(), recovered.groups[1]);
    }

    #[test]
    fn late_ridge_recovery_retains_missing_staves_without_duplicating_accepted_geometry() {
        let mut runs = RunTable::new(Orientation::Horizontal, 240, 180).unwrap();
        for y in [15, 25, 35, 45, 55, 105, 115, 125, 135, 145] {
            runs.add_run(y, Run::new(10, 220)).unwrap();
        }
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let mut next_id = 1;
        let recovered =
            recover_tentative_staffs_from_ridges(&lag, &[], &parameters(), &mut next_id).unwrap();

        assert_eq!(recovered.len(), 2);
        assert!(recovered.iter().all(StaffCandidate::is_tentative));
        assert_eq!(next_id, 11);

        let mut second_next_id = next_id;
        let duplicates = recover_tentative_staffs_from_ridges(
            &lag,
            &recovered,
            &parameters(),
            &mut second_next_id,
        )
        .unwrap();
        assert!(duplicates.is_empty());
        assert_eq!(second_next_id, next_id);
    }

    #[test]
    fn late_ridge_recovery_does_not_promote_four_text_like_rows() {
        let mut runs = RunTable::new(Orientation::Horizontal, 240, 90).unwrap();
        for y in [15, 25, 35, 45] {
            runs.add_run(y, Run::new(10, 220)).unwrap();
        }
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let mut next_id = 1;
        let recovered =
            recover_tentative_staffs_from_ridges(&lag, &[], &parameters(), &mut next_id).unwrap();

        assert!(recovered.is_empty());
        assert_eq!(next_id, 1);
    }

    #[test]
    fn adapter_failure_does_not_mutate_live_lag() {
        let mut runs = RunTable::new(Orientation::Horizontal, 20, 12).unwrap();
        runs.add_run(10, Run::new(0, 20)).unwrap();
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let before = lag.clone();
        let mut invalid = parameters();
        invalid.sampling_dx = 0;

        assert!(build_primary_cluster_pass(&lag, invalid).is_err());
        assert_eq!(lag, before);
    }

    #[test]
    fn secondary_pass_preserves_main_geometry_ids_and_sorted_discard_provenance() {
        let discarded_id = FilamentId::new(5);
        let other_discarded_id = FilamentId::new(2);
        let mut table = RunTable::new(Orientation::Horizontal, 41, 42).unwrap();
        table.add_run(20, Run::new(0, 40)).unwrap();
        table.add_run(40, Run::new(0, 40)).unwrap();
        let mut source_sections = build_sections(&table, JunctionPolicy::All);
        let discarded_section = source_sections.remove(0);
        let other_discarded_section = source_sections.remove(0);
        let mut discarded = StaffFilament::new(10).unwrap();
        discarded.add_section(discarded_section).unwrap();
        let mut other_discarded = StaffFilament::new(10).unwrap();
        other_discarded
            .add_section(other_discarded_section)
            .unwrap();
        let discarded_sections = discarded.sections().to_vec();
        let other_discarded_sections = other_discarded.sections().to_vec();

        let mut primary_ownership = ClusterOwnership::new();
        primary_ownership
            .register_filament(discarded_id, &discarded)
            .unwrap();
        primary_ownership
            .register_filament(other_discarded_id, &other_discarded)
            .unwrap();
        let primary = ClusterPassState::new(
            primary_ownership,
            BTreeMap::from([
                (discarded_id, discarded),
                (other_discarded_id, other_discarded),
            ]),
            BTreeMap::new(),
            vec![discarded_id],
            retrieval_parameters_for(10, BTreeSet::from([1])),
        );
        let secondary = build_secondary_cluster_pass(
            100,
            &primary,
            &[other_discarded_id, discarded_id],
            RawSecondaryPassParameters {
                sampling_dx: 20,
                minimum_delta_y: 4,
                maximum_delta_y: 6,
                retrieval: retrieval_parameters_for(5, BTreeSet::from([1])),
            },
        )
        .unwrap();

        assert_eq!(
            secondary.filament_order(),
            [other_discarded_id, discarded_id]
        );
        assert_eq!(secondary.parameters().interline(), 5);
        assert_eq!(secondary.filaments()[&discarded_id].interline(), 10);
        assert_eq!(secondary.filaments()[&other_discarded_id].interline(), 10);
        assert_eq!(
            secondary.filaments()[&discarded_id].sections(),
            discarded_sections
        );
        assert_eq!(
            secondary.filaments()[&other_discarded_id].sections(),
            other_discarded_sections
        );
        assert_eq!(
            secondary
                .ownership()
                .section_owner(secondary.filaments()[&discarded_id].sections()[0].id()),
            Some(discarded_id)
        );
    }

    #[test]
    fn duplicate_secondary_provenance_fails_without_mutating_sources() {
        let id = FilamentId::new(7);
        let filament = one_line_filament(10, 20, 0, 40);
        let mut ownership = ClusterOwnership::new();
        ownership.register_filament(id, &filament).unwrap();
        let primary = ClusterPassState::new(
            ownership,
            BTreeMap::from([(id, filament.clone())]),
            BTreeMap::new(),
            vec![id],
            retrieval_parameters_for(10, BTreeSet::from([1])),
        );
        let error = build_secondary_cluster_pass(
            100,
            &primary,
            &[id, id],
            RawSecondaryPassParameters {
                sampling_dx: 20,
                minimum_delta_y: 4,
                maximum_delta_y: 6,
                retrieval: retrieval_parameters_for(5, BTreeSet::from([1])),
            },
        )
        .unwrap_err();

        assert_eq!(error, RawLineAdapterError::DuplicateSecondaryFilament(id));
        assert_eq!(primary.filaments()[&id].interline(), filament.interline());
        assert_eq!(primary.filaments()[&id].sections(), filament.sections());
    }

    #[test]
    fn geometric_outliers_reach_combs_with_diagnostic_classes_intact() {
        let values = rejection_fixture_filaments();
        let survivor_sections = values
            .iter()
            .enumerate()
            .map(|(index, filament)| {
                (
                    FilamentId::new((index + 1) as u64),
                    filament
                        .sections()
                        .iter()
                        .map(|section| section.id())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let identified = values
            .into_iter()
            .enumerate()
            .map(|(index, filament)| ((index + 1) as u64, filament))
            .collect();
        let filtered = filter_raw_filaments(
            identified,
            LineRejectionParameters::java_defaults(10.0),
            false,
        )
        .unwrap();

        assert!((filtered.global_slope - (6.0 / 119.0)).abs() < 1e-12);
        assert_eq!(
            filtered.filament_order,
            [
                FilamentId::new(3),
                FilamentId::new(2),
                FilamentId::new(5),
                FilamentId::new(4),
                FilamentId::new(1),
            ]
        );
        assert_eq!(filtered.curved_ids, [FilamentId::new(2)]);
        assert_eq!(filtered.sloped_ids, [FilamentId::new(4)]);
        assert_eq!(
            filtered.filaments.keys().copied().collect::<Vec<_>>(),
            [
                FilamentId::new(1),
                FilamentId::new(2),
                FilamentId::new(3),
                FilamentId::new(4),
                FilamentId::new(5),
            ]
        );
        assert!(filtered.sloped_filaments.is_empty());

        for id in [
            FilamentId::new(1),
            FilamentId::new(2),
            FilamentId::new(3),
            FilamentId::new(4),
            FilamentId::new(5),
        ] {
            for section in &survivor_sections[&id] {
                assert_eq!(filtered.ownership.section_owner(*section), Some(id));
            }
        }
    }

    #[test]
    fn adapter_uses_factory_creation_ids_across_swallow_and_expansion() {
        let mut runs = RunTable::new(Orientation::Horizontal, 320, 8).unwrap();
        for (x, length) in [(0, 80), (91, 80), (220, 80)] {
            runs.add_run(2, Run::new(x, length)).unwrap();
        }
        for (x, length) in [(80, 5), (86, 5)] {
            runs.add_run(3, Run::new(x, length)).unwrap();
        }
        let lag = HorizontalSectionLag::from_long_runs(runs).unwrap();
        let built = build_primary_cluster_pass_with_first_filament_id(
            &lag,
            identity_parameters(),
            FilamentId::new(41),
        )
        .unwrap();

        assert_eq!(
            built.factory_creation_ids(),
            [
                FilamentId::new(41),
                FilamentId::new(42),
                FilamentId::new(43),
                FilamentId::new(44),
                FilamentId::new(45),
            ]
        );
        assert_eq!(built.next_filament_id(), FilamentId::new(46));
        assert_eq!(
            built.survivor_order(),
            [FilamentId::new(41), FilamentId::new(43)]
        );
        assert_eq!(
            built
                .state()
                .filaments()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [FilamentId::new(41), FilamentId::new(43)]
        );
        assert!(!built.state().filaments().contains_key(&FilamentId::new(42)));
        for section in lag.sections() {
            let expected = if section.bounds().x < 200 {
                FilamentId::new(41)
            } else {
                FilamentId::new(43)
            };
            assert_eq!(
                built.state().ownership().section_owner(section.id()),
                Some(expected)
            );
        }
    }
}
