// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of provisional range heads through glyph retrieval.
//!
//! Java performs this work independently for every range scanner: provisional
//! matches are aggregated, aggregate winners are filtered against the current
//! semantic-area slice of already-created seed heads, and surviving winners
//! retrieve a glyph from the original BINARY raster.  Seed heads accumulate in
//! system/staff order, while range heads never enter the competitor pool.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    head_glyph_retrieval::{
        HeadGlyphRetrievalError, HeadGlyphRetrievalInput, RetrievedHeadGlyph, retrieve_head_glyph,
    },
    head_range_postprocess::{
        HeadPostprocessGeometry, MIN_SEED_HEAD_IOU, SEED_GRADE_MARGIN, aggregate_matches,
        filter_seed_conflicts, java_rectangle_iou,
    },
    head_scanner_slices::JavaRectangle,
    head_template::{HeadTemplateBounds, HeadTemplateShape},
    native_heads::{NativeHeadTemplateCatalogSelection, NativeHeadsPrologRecognition},
    native_heads_competitors::NativeHeadsCompetitorPool,
    native_heads_range_lookup::{NativeHeadRangeCandidate, NativeHeadsRangeLookupRecognition},
    native_heads_scanner::NativeHeadsScannerRecognition,
    native_heads_scanner_pools::NativeHeadScannerPoolsRecognition,
    native_heads_seed_glyphs::{NativeHeadSeedGlyph, NativeHeadsSeedGlyphRecognition},
    native_heads_seed_lookup::GOOD_INTER_GRADE,
    recognize::GridLinesRecognition,
};

/// Immutable products at Java's post-range-lookup composition boundary.
pub struct NativeHeadsRangeGlyphsInput<'a> {
    pub grid: &'a GridLinesRecognition,
    pub heads: &'a NativeHeadsPrologRecognition,
    pub scanners: &'a NativeHeadsScannerRecognition,
    pub pools: &'a NativeHeadScannerPoolsRecognition,
    pub competitors: &'a NativeHeadsCompetitorPool,
    pub range_lookup: &'a NativeHeadsRangeLookupRecognition,
    pub seed_glyphs: &'a NativeHeadsSeedGlyphRecognition,
}

/// Final identity-free range heads plus all scanner-local decision evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsRangeGlyphRecognition {
    pub systems: Vec<NativeHeadsRangeGlyphSystem>,
    pub candidate_count: usize,
    pub seed_conflict_count: usize,
    pub glyph_empty_count: usize,
    pub head_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsRangeGlyphSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadsRangeGlyphStaff>,
    pub candidate_count: usize,
    pub seed_conflict_count: usize,
    pub glyph_empty_count: usize,
    pub head_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsRangeGlyphStaff {
    pub staff_id: usize,
    pub scans: Vec<NativeHeadRangeGlyphScan>,
    /// Compact post-aggregation candidates in Java scanner order.
    pub candidates: Vec<NativeHeadRangePostCandidate>,
    /// Successful range heads in dense Java insertion order.
    pub heads: Vec<NativeHeadRangeGlyph>,
    pub seed_head_count: usize,
    pub candidate_count: usize,
    pub seed_conflict_count: usize,
    pub glyph_empty_count: usize,
}

/// One scanner after dynamic competitor slicing and aggregation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadRangeGlyphScan {
    pub geometry_ordinal: usize,
    /// Good competitors intersecting the exact semantic `Area`, stably x-sorted.
    pub competitors: Vec<NativeHeadRangeLiveCompetitor>,
    pub aggregates: Vec<NativeHeadRangeAggregateEvidence>,
    /// Staff-wide compact candidate ordinals owned by this scanner.
    pub candidate_ordinals: Vec<usize>,
    pub seed_conflict_count: usize,
    pub glyph_empty_count: usize,
    pub head_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadRangeLiveSource {
    Base {
        pool_index: usize,
    },
    Seed {
        staff_id: usize,
        seed_ordinal: usize,
    },
}

/// One member of Java `getCompetitorsSlice`, without SIG identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeLiveCompetitor {
    pub slice_ordinal: usize,
    /// Ordinal in the current system-wide stable ordinate sort.
    pub live_ordinal: usize,
    pub source: NativeHeadRangeLiveSource,
    pub bounds: JavaRectangle,
    pub grade: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHeadRangeAggregateEvidence {
    pub aggregate_ordinal: usize,
    pub main_scan_candidate_index: usize,
    pub main_raw_ordinal: usize,
    /// The stable reverse-grade traversal used to build the aggregate.
    pub member_scan_candidate_indices_by_grade: Vec<usize>,
    /// Members in the scanner's original raw-candidate order, as Java reports them.
    pub member_raw_ordinals: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeadRangeSeedConflict {
    pub slice_ordinal: usize,
    pub live_ordinal: usize,
    pub seed_staff_id: usize,
    pub seed_ordinal: usize,
    pub bounds: JavaRectangle,
    pub grade: f64,
    pub iou: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadRangePostOutcome {
    SeedConflict,
    GlyphEmpty,
    Final { head_ordinal: usize },
}

/// One aggregate winner, retained even when Java filters or drops it.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadRangePostCandidate {
    /// Dense compact post-aggregation ordinal within the staff.
    pub ordinal: usize,
    pub raw: NativeHeadRangeCandidate,
    pub aggregate_ordinal: usize,
    pub member_raw_ordinals: Vec<usize>,
    /// Every qualifying seed in the full local slice, in slice order.
    pub seed_conflicts: Vec<NativeHeadRangeSeedConflict>,
    pub glyph: Option<RetrievedHeadGlyph>,
    pub outcome: NativeHeadRangePostOutcome,
}

/// Successful range head before global glyph registration and SIG insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadRangeGlyph {
    pub ordinal: usize,
    pub candidate_ordinal: usize,
    pub raw_ordinal: usize,
    pub attempt_ordinal: usize,
    pub geometry_ordinal: usize,
    pub original_shape: HeadTemplateShape,
    pub shape: HeadTemplateShape,
    pub pitch_bits: u64,
    pub provisional_bounds: HeadTemplateBounds,
    pub bounds: HeadTemplateBounds,
    pub raw_distance_impact_bits: u64,
    pub distance_impact_bits: u64,
    pub grade_bits: u64,
    pub minimum_grade_bits: u64,
    pub good: bool,
    pub glyph: RetrievedHeadGlyph,
}

impl NativeHeadRangeGlyph {
    #[must_use]
    pub fn pitch(&self) -> f64 {
        f64::from_bits(self.pitch_bits)
    }

    #[must_use]
    pub fn raw_distance_impact(&self) -> f64 {
        f64::from_bits(self.raw_distance_impact_bits)
    }

    #[must_use]
    pub fn distance_impact(&self) -> f64 {
        f64::from_bits(self.distance_impact_bits)
    }

    #[must_use]
    pub fn grade(&self) -> f64 {
        f64::from_bits(self.grade_bits)
    }

    #[must_use]
    pub fn minimum_grade(&self) -> f64 {
        f64::from_bits(self.minimum_grade_bits)
    }
}

#[derive(Debug)]
pub enum NativeHeadsRangeGlyphError {
    SystemTopology {
        source: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    StaffTopology {
        system_id: usize,
        source: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    CatalogSelectionOrder,
    MissingCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    MissingCatalog {
        system_id: usize,
        staff_id: usize,
        catalog_ordinal: usize,
    },
    CatalogPointSize {
        system_id: usize,
        staff_id: usize,
        expected: i32,
        actual: i32,
    },
    ScanOrder {
        system_id: usize,
        staff_id: usize,
        expected: usize,
        actual: usize,
    },
    ScanCount {
        system_id: usize,
        staff_id: usize,
        source: &'static str,
        expected: usize,
        actual: usize,
    },
    RawCandidateOrder {
        system_id: usize,
        staff_id: usize,
        expected: usize,
        actual: usize,
    },
    SeedOrder {
        system_id: usize,
        staff_id: usize,
        expected: usize,
        actual: usize,
    },
    AggregateMain {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        aggregate_ordinal: usize,
    },
    ConflictFilter {
        system_id: usize,
        staff_id: usize,
        geometry_ordinal: usize,
        aggregate_ordinal: usize,
    },
    Retrieval {
        system_id: usize,
        staff_id: usize,
        candidate_ordinal: usize,
        source: HeadGlyphRetrievalError,
    },
}

impl fmt::Display for NativeHeadsRangeGlyphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemTopology {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph {source} system order {actual:?}, expected {expected:?}"
            ),
            Self::StaffTopology {
                system_id,
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} {source} staff order {actual:?}, expected {expected:?}"
            ),
            Self::CatalogSelectionOrder => write!(
                formatter,
                "HEADS range glyph catalog selections are not strictly ordered"
            ),
            Self::MissingCatalogSelection {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} has no catalog selection"
            ),
            Self::MissingCatalog {
                system_id,
                staff_id,
                catalog_ordinal,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} references missing catalog {catalog_ordinal}"
            ),
            Self::CatalogPointSize {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} catalog point size {actual}, expected {expected}"
            ),
            Self::ScanOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} scanner ordinal {actual}, expected {expected}"
            ),
            Self::ScanCount {
                system_id,
                staff_id,
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} has {actual} {source} scans, expected {expected}"
            ),
            Self::RawCandidateOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} raw candidate ordinal {actual}, expected {expected}"
            ),
            Self::SeedOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} seed ordinal {actual}, expected {expected}"
            ),
            Self::AggregateMain {
                system_id,
                staff_id,
                geometry_ordinal,
                aggregate_ordinal,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} scanner {geometry_ordinal} aggregate {aggregate_ordinal} has no main member"
            ),
            Self::ConflictFilter {
                system_id,
                staff_id,
                geometry_ordinal,
                aggregate_ordinal,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} scanner {geometry_ordinal} aggregate {aggregate_ordinal} disagrees with seed-conflict filter"
            ),
            Self::Retrieval {
                system_id,
                staff_id,
                candidate_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS range glyph system {system_id} staff {staff_id} candidate {candidate_ordinal} retrieval: {source}"
            ),
        }
    }
}

impl Error for NativeHeadsRangeGlyphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Retrieval { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct LiveCompetitor {
    source: NativeHeadRangeLiveSource,
    bounds: JavaRectangle,
    grade: f64,
    good: bool,
}

/// Compose scanner-local range winners and retrieve their concrete glyphs.
pub fn retrieve_native_heads_range_glyphs(
    input: NativeHeadsRangeGlyphsInput<'_>,
) -> Result<NativeHeadsRangeGlyphRecognition, NativeHeadsRangeGlyphError> {
    let selections = validate_topology(&input)?;
    let mut systems = Vec::with_capacity(input.range_lookup.systems.len());
    let mut total_candidates = 0;
    let mut total_conflicts = 0;
    let mut total_empty = 0;
    let mut total_heads = 0;

    for system_ordinal in 0..input.range_lookup.systems.len() {
        let range_system = &input.range_lookup.systems[system_ordinal];
        let scanner_system = &input.scanners.systems[system_ordinal];
        let pool_system = &input.pools.systems[system_ordinal];
        let competitor_system = &input.competitors.systems[system_ordinal];
        let seed_system = &input.seed_glyphs.systems[system_ordinal];
        let system_id = range_system.system_id;

        let mut live = competitor_system
            .accepted_by_ordinate
            .iter()
            .enumerate()
            .map(|(pool_index, competitor)| LiveCompetitor {
                source: NativeHeadRangeLiveSource::Base { pool_index },
                bounds: competitor.bounds,
                grade: competitor.grade,
                good: competitor.good,
            })
            .collect::<Vec<_>>();
        let mut staffs = Vec::with_capacity(range_system.staffs.len());
        let mut system_candidates = 0;
        let mut system_conflicts = 0;
        let mut system_empty = 0;
        let mut system_heads = 0;

        for staff_ordinal in 0..range_system.staffs.len() {
            let range_staff = &range_system.staffs[staff_ordinal];
            let scanner_staff = &scanner_system.staffs[staff_ordinal];
            let pool_staff = &pool_system.staffs[staff_ordinal];
            let seed_staff = &seed_system.staffs[staff_ordinal];
            let staff_id = range_staff.staff_id;
            let selection = selections.get(&(system_id, staff_id)).copied().ok_or(
                NativeHeadsRangeGlyphError::MissingCatalogSelection {
                    system_id,
                    staff_id,
                },
            )?;
            let catalog = input
                .heads
                .template_catalogs
                .get(selection.catalog_ordinal)
                .ok_or(NativeHeadsRangeGlyphError::MissingCatalog {
                    system_id,
                    staff_id,
                    catalog_ordinal: selection.catalog_ordinal,
                })?;
            if catalog.point_size() != selection.point_size {
                return Err(NativeHeadsRangeGlyphError::CatalogPointSize {
                    system_id,
                    staff_id,
                    expected: selection.point_size,
                    actual: catalog.point_size(),
                });
            }

            for (expected, seed) in seed_staff.heads.iter().enumerate() {
                if seed.ordinal != expected {
                    return Err(NativeHeadsRangeGlyphError::SeedOrder {
                        system_id,
                        staff_id,
                        expected,
                        actual: seed.ordinal,
                    });
                }
                live.push(live_seed(staff_id, seed));
            }
            // `List.sort` is stable, and Java `Inters.byOrdinate` compares top y only.
            live.sort_by_key(|competitor| competitor.bounds.y);

            let expected_scans = scanner_staff.geometries.len();
            for (source, actual) in [
                ("range", range_staff.scans.len()),
                ("pool", pool_staff.scans.len()),
            ] {
                if actual != expected_scans {
                    return Err(NativeHeadsRangeGlyphError::ScanCount {
                        system_id,
                        staff_id,
                        source,
                        expected: expected_scans,
                        actual,
                    });
                }
            }

            let mut scans = Vec::with_capacity(expected_scans);
            let mut candidates = Vec::new();
            let mut heads = Vec::new();
            let mut staff_raw_ordinal = 0;
            let mut staff_conflicts = 0;
            let mut staff_empty = 0;

            for geometry_ordinal in 0..expected_scans {
                let range_scan = &range_staff.scans[geometry_ordinal];
                let pool_scan = &pool_staff.scans[geometry_ordinal];
                if range_scan.geometry_ordinal != geometry_ordinal {
                    return Err(NativeHeadsRangeGlyphError::ScanOrder {
                        system_id,
                        staff_id,
                        expected: geometry_ordinal,
                        actual: range_scan.geometry_ordinal,
                    });
                }
                if pool_scan.geometry_ordinal != geometry_ordinal {
                    return Err(NativeHeadsRangeGlyphError::ScanOrder {
                        system_id,
                        staff_id,
                        expected: geometry_ordinal,
                        actual: pool_scan.geometry_ordinal,
                    });
                }
                for raw in &range_scan.candidates {
                    if raw.ordinal != staff_raw_ordinal {
                        return Err(NativeHeadsRangeGlyphError::RawCandidateOrder {
                            system_id,
                            staff_id,
                            expected: staff_raw_ordinal,
                            actual: raw.ordinal,
                        });
                    }
                    staff_raw_ordinal += 1;
                }

                let local_competitors = slice_live_competitors(&live, pool_scan);
                let seed_entries = local_competitors
                    .iter()
                    .filter(|entry| matches!(entry.source, NativeHeadRangeLiveSource::Seed { .. }))
                    .copied()
                    .collect::<Vec<_>>();
                let seed_geometry = seed_entries
                    .iter()
                    .map(|entry| HeadPostprocessGeometry {
                        bounds: entry.bounds,
                        grade: entry.grade,
                    })
                    .collect::<Vec<_>>();
                let raw_geometry = range_scan
                    .candidates
                    .iter()
                    .map(candidate_geometry)
                    .collect::<Vec<_>>();
                let aggregates =
                    aggregate_matches(&raw_geometry, scanner_system.parameters.max_template_dx);
                let winner_geometry = aggregates
                    .iter()
                    .map(|aggregate| raw_geometry[aggregate.main_index])
                    .collect::<Vec<_>>();
                let filtered = filter_seed_conflicts(&winner_geometry, &seed_geometry);

                let retained = filtered
                    .retained_range_indices
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>();
                let first_conflicts = filtered
                    .conflicts
                    .iter()
                    .map(|conflict| (conflict.range_index, conflict.seed_index))
                    .collect::<BTreeMap<_, _>>();
                let mut aggregate_evidence = Vec::with_capacity(aggregates.len());
                let mut scan_candidate_ordinals = Vec::with_capacity(aggregates.len());
                let scan_head_start = heads.len();
                let scan_conflict_start = staff_conflicts;
                let scan_empty_start = staff_empty;

                for (aggregate_ordinal, aggregate) in aggregates.iter().enumerate() {
                    let raw = *range_scan.candidates.get(aggregate.main_index).ok_or(
                        NativeHeadsRangeGlyphError::AggregateMain {
                            system_id,
                            staff_id,
                            geometry_ordinal,
                            aggregate_ordinal,
                        },
                    )?;
                    let mut members = aggregate.member_indices.clone();
                    members.sort_unstable();
                    let member_raw_ordinals = members
                        .iter()
                        .map(|&index| range_scan.candidates[index].ordinal)
                        .collect::<Vec<_>>();
                    aggregate_evidence.push(NativeHeadRangeAggregateEvidence {
                        aggregate_ordinal,
                        main_scan_candidate_index: aggregate.main_index,
                        main_raw_ordinal: raw.ordinal,
                        member_scan_candidate_indices_by_grade: aggregate.member_indices.clone(),
                        member_raw_ordinals: member_raw_ordinals.clone(),
                    });

                    let ordinal = candidates.len();
                    scan_candidate_ordinals.push(ordinal);
                    let seed_conflicts =
                        all_seed_conflicts(winner_geometry[aggregate_ordinal], &local_competitors);
                    let expected_first = first_conflicts.get(&aggregate_ordinal).copied();
                    let actual_first = seed_conflicts.first().and_then(|conflict| {
                        seed_entries.iter().position(|seed| {
                            seed.live_ordinal == conflict.live_ordinal
                                && seed.source
                                    == NativeHeadRangeLiveSource::Seed {
                                        staff_id: conflict.seed_staff_id,
                                        seed_ordinal: conflict.seed_ordinal,
                                    }
                        })
                    });
                    if expected_first != actual_first
                        || retained.contains(&aggregate_ordinal) != seed_conflicts.is_empty()
                    {
                        return Err(NativeHeadsRangeGlyphError::ConflictFilter {
                            system_id,
                            staff_id,
                            geometry_ordinal,
                            aggregate_ordinal,
                        });
                    }

                    let (glyph, outcome) = if !seed_conflicts.is_empty() {
                        staff_conflicts += 1;
                        (None, NativeHeadRangePostOutcome::SeedConflict)
                    } else {
                        let glyph = retrieve_head_glyph(HeadGlyphRetrievalInput {
                            template: catalog.template(raw.shape),
                            slim_inter_bounds: raw.bounds,
                            raster_width: input.grid.scale.width,
                            raster_height: input.grid.scale.height,
                            binary: &input.grid.scale.binary,
                        })
                        .map_err(|source| {
                            NativeHeadsRangeGlyphError::Retrieval {
                                system_id,
                                staff_id,
                                candidate_ordinal: ordinal,
                                source,
                            }
                        })?;
                        if let Some(glyph) = glyph {
                            let head_ordinal = heads.len();
                            heads.push(final_head(head_ordinal, ordinal, raw, glyph.clone()));
                            (
                                Some(glyph),
                                NativeHeadRangePostOutcome::Final { head_ordinal },
                            )
                        } else {
                            staff_empty += 1;
                            (None, NativeHeadRangePostOutcome::GlyphEmpty)
                        }
                    };
                    candidates.push(NativeHeadRangePostCandidate {
                        ordinal,
                        raw,
                        aggregate_ordinal,
                        member_raw_ordinals,
                        seed_conflicts,
                        glyph,
                        outcome,
                    });
                }

                scans.push(NativeHeadRangeGlyphScan {
                    geometry_ordinal,
                    competitors: local_competitors,
                    aggregates: aggregate_evidence,
                    candidate_ordinals: scan_candidate_ordinals,
                    seed_conflict_count: staff_conflicts - scan_conflict_start,
                    glyph_empty_count: staff_empty - scan_empty_start,
                    head_count: heads.len() - scan_head_start,
                });
            }

            let staff_candidates = candidates.len();
            let staff_heads = heads.len();
            system_candidates += staff_candidates;
            system_conflicts += staff_conflicts;
            system_empty += staff_empty;
            system_heads += staff_heads;
            staffs.push(NativeHeadsRangeGlyphStaff {
                staff_id,
                scans,
                candidates,
                heads,
                seed_head_count: seed_staff.heads.len(),
                candidate_count: staff_candidates,
                seed_conflict_count: staff_conflicts,
                glyph_empty_count: staff_empty,
            });
        }

        total_candidates += system_candidates;
        total_conflicts += system_conflicts;
        total_empty += system_empty;
        total_heads += system_heads;
        systems.push(NativeHeadsRangeGlyphSystem {
            system_id,
            staffs,
            candidate_count: system_candidates,
            seed_conflict_count: system_conflicts,
            glyph_empty_count: system_empty,
            head_count: system_heads,
        });
    }

    Ok(NativeHeadsRangeGlyphRecognition {
        systems,
        candidate_count: total_candidates,
        seed_conflict_count: total_conflicts,
        glyph_empty_count: total_empty,
        head_count: total_heads,
    })
}

fn validate_topology<'a>(
    input: &'a NativeHeadsRangeGlyphsInput<'a>,
) -> Result<
    BTreeMap<(usize, usize), &'a NativeHeadTemplateCatalogSelection>,
    NativeHeadsRangeGlyphError,
> {
    let expected = input
        .range_lookup
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let sources = [
        (
            "scanner",
            input
                .scanners
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>(),
        ),
        (
            "pool",
            input
                .pools
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "competitor",
            input
                .competitors
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
        (
            "seed",
            input
                .seed_glyphs
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect(),
        ),
    ];
    for (source, actual) in sources {
        if actual != expected {
            return Err(NativeHeadsRangeGlyphError::SystemTopology {
                source,
                expected,
                actual,
            });
        }
    }

    let mut selections = BTreeMap::new();
    let mut prior = None;
    for selection in &input.heads.staff_template_catalogs {
        let owner = (selection.system_id, selection.staff_id);
        if prior.is_some_and(|prior| prior >= owner)
            || selections.insert(owner, selection).is_some()
        {
            return Err(NativeHeadsRangeGlyphError::CatalogSelectionOrder);
        }
        prior = Some(owner);
    }

    for ordinal in 0..input.range_lookup.systems.len() {
        let system_id = input.range_lookup.systems[ordinal].system_id;
        let expected = input.range_lookup.systems[ordinal]
            .staffs
            .iter()
            .map(|staff| staff.staff_id)
            .collect::<Vec<_>>();
        let sources = [
            (
                "scanner",
                input.scanners.systems[ordinal]
                    .staffs
                    .iter()
                    .map(|staff| staff.staff_id)
                    .collect::<Vec<_>>(),
            ),
            (
                "pool",
                input.pools.systems[ordinal]
                    .staffs
                    .iter()
                    .map(|staff| staff.staff_id)
                    .collect(),
            ),
            (
                "seed",
                input.seed_glyphs.systems[ordinal]
                    .staffs
                    .iter()
                    .map(|staff| staff.staff_id)
                    .collect(),
            ),
        ];
        for (source, actual) in sources {
            if actual != expected {
                return Err(NativeHeadsRangeGlyphError::StaffTopology {
                    system_id,
                    source,
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(selections)
}

fn live_seed(staff_id: usize, seed: &NativeHeadSeedGlyph) -> LiveCompetitor {
    LiveCompetitor {
        source: NativeHeadRangeLiveSource::Seed {
            staff_id,
            seed_ordinal: seed.ordinal,
        },
        bounds: java_rectangle(seed.bounds),
        grade: seed.grade(),
        good: seed.good,
    }
}

fn slice_live_competitors(
    live: &[LiveCompetitor],
    pool_scan: &crate::native_heads_scanner_pools::NativeHeadScannerPoolScan,
) -> Vec<NativeHeadRangeLiveCompetitor> {
    let mut sliced = live
        .iter()
        .enumerate()
        .filter(|(_, competitor)| {
            competitor.good
                && pool_scan
                    .competitor_band
                    .intersects_rectangle(competitor.bounds)
        })
        .map(|(live_ordinal, competitor)| NativeHeadRangeLiveCompetitor {
            slice_ordinal: 0,
            live_ordinal,
            source: competitor.source,
            bounds: competitor.bounds,
            grade: competitor.grade,
        })
        .collect::<Vec<_>>();
    // Java's object sort is stable and `Inters.byAbscissa` compares left x only.
    sliced.sort_by_key(|competitor| competitor.bounds.x);
    for (slice_ordinal, competitor) in sliced.iter_mut().enumerate() {
        competitor.slice_ordinal = slice_ordinal;
    }
    sliced
}

fn all_seed_conflicts(
    range: HeadPostprocessGeometry,
    local_competitors: &[NativeHeadRangeLiveCompetitor],
) -> Vec<NativeHeadRangeSeedConflict> {
    let lowered_grade = range.grade * (1.0 - SEED_GRADE_MARGIN);
    let x_max = f64::from(range.bounds.x) + f64::from(range.bounds.width);
    let mut conflicts = Vec::new();
    for competitor in local_competitors {
        let NativeHeadRangeLiveSource::Seed {
            staff_id,
            seed_ordinal,
        } = competitor.source
        else {
            continue;
        };
        if competitor.bounds.intersects(range.bounds) {
            let iou = java_rectangle_iou(competitor.bounds, range.bounds);
            if iou >= MIN_SEED_HEAD_IOU && competitor.grade >= lowered_grade {
                conflicts.push(NativeHeadRangeSeedConflict {
                    slice_ordinal: competitor.slice_ordinal,
                    live_ordinal: competitor.live_ordinal,
                    seed_staff_id: staff_id,
                    seed_ordinal,
                    bounds: competitor.bounds,
                    grade: competitor.grade,
                    iou,
                });
            }
        } else if f64::from(competitor.bounds.x) > x_max {
            break;
        }
    }
    conflicts
}

fn candidate_geometry(candidate: &NativeHeadRangeCandidate) -> HeadPostprocessGeometry {
    HeadPostprocessGeometry {
        bounds: java_rectangle(candidate.bounds),
        grade: candidate.grade,
    }
}

fn final_head(
    ordinal: usize,
    candidate_ordinal: usize,
    raw: NativeHeadRangeCandidate,
    glyph: RetrievedHeadGlyph,
) -> NativeHeadRangeGlyph {
    NativeHeadRangeGlyph {
        ordinal,
        candidate_ordinal,
        raw_ordinal: raw.ordinal,
        attempt_ordinal: raw.attempt_ordinal,
        geometry_ordinal: raw.geometry_ordinal,
        original_shape: raw.original_shape,
        shape: raw.shape,
        pitch_bits: raw.pitch.to_bits(),
        provisional_bounds: raw.bounds,
        bounds: glyph.new_inter_bounds,
        raw_distance_impact_bits: raw.raw_distance_impact.to_bits(),
        distance_impact_bits: raw.distance_impact.to_bits(),
        grade_bits: raw.grade.to_bits(),
        minimum_grade_bits: raw.minimum_grade.to_bits(),
        good: raw.grade >= GOOD_INTER_GRADE,
        glyph,
    }
}

const fn java_rectangle(bounds: HeadTemplateBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_conflicts_preserve_full_slice_ordinals_and_skip_base_entries() {
        let local = [
            NativeHeadRangeLiveCompetitor {
                slice_ordinal: 0,
                live_ordinal: 8,
                source: NativeHeadRangeLiveSource::Base { pool_index: 2 },
                bounds: JavaRectangle::new(0, 0, 10, 10),
                grade: 1.0,
            },
            NativeHeadRangeLiveCompetitor {
                slice_ordinal: 1,
                live_ordinal: 11,
                source: NativeHeadRangeLiveSource::Seed {
                    staff_id: 3,
                    seed_ordinal: 4,
                },
                bounds: JavaRectangle::new(0, 0, 10, 2),
                grade: 0.8,
            },
            NativeHeadRangeLiveCompetitor {
                slice_ordinal: 2,
                live_ordinal: 14,
                source: NativeHeadRangeLiveSource::Seed {
                    staff_id: 3,
                    seed_ordinal: 7,
                },
                bounds: JavaRectangle::new(1, 0, 9, 2),
                grade: 0.9,
            },
        ];
        let conflicts = all_seed_conflicts(
            HeadPostprocessGeometry {
                bounds: JavaRectangle::new(0, 0, 10, 10),
                grade: 0.8,
            },
            &local,
        );
        assert_eq!(conflicts.len(), 2);
        assert_eq!(
            (conflicts[0].slice_ordinal, conflicts[0].live_ordinal),
            (1, 11)
        );
        assert_eq!(
            (conflicts[1].slice_ordinal, conflicts[1].live_ordinal),
            (2, 14)
        );
    }

    #[test]
    fn dense_final_head_copies_identity_free_provenance() {
        let raw = NativeHeadRangeCandidate {
            ordinal: 9,
            attempt_ordinal: 27,
            geometry_ordinal: 2,
            x0: 0,
            y0: 0,
            black_relevant: false,
            shape_choice: crate::native_heads_range_lookup::NativeHeadRangeShapeChoice::All,
            shape_ordinal: 0,
            original_shape: HeadTemplateShape::NoteheadBlack,
            shape: HeadTemplateShape::NoteheadBlack,
            best: crate::native_heads_range_lookup::NativeHeadRangeBestLocation {
                y_offset_ordinal: 0,
                y_offset: 0,
                x: 0,
                y: 0,
                distance: 0.0,
            },
            hole: None,
            stemless: crate::native_heads_range_lookup::NativeHeadRangeStemlessEvidence {
                applicable: false,
                intrinsic_grade: 0.5,
                boost: 0.0,
                contextual_grade: 0.5,
                minimum_contextual_grade: 0.5,
                rejected: false,
            },
            anchor: crate::head_template::HeadTemplateAnchor::MiddleLeft,
            pitch: 1.0,
            bounds: HeadTemplateBounds {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            raw_distance_impact: 0.7,
            distance_impact: 0.7,
            grade: 0.4,
            minimum_grade: 0.1,
        };
        let glyph = RetrievedHeadGlyph {
            template_bounds: raw.bounds,
            foreground_bounds: HeadTemplateBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            glyph_bounds: HeadTemplateBounds {
                x: 2,
                y: 3,
                width: 2,
                height: 2,
            },
            weight: 4,
            run_digest: 5,
            run_table: audiveris_image::run_table::RunTable::new(
                audiveris_image::run_table::Orientation::Vertical,
                2,
                2,
            )
            .unwrap(),
            new_inter_bounds: HeadTemplateBounds {
                x: 2,
                y: 3,
                width: 2,
                height: 2,
            },
        };
        let head = final_head(3, 8, raw, glyph);
        assert_eq!(
            (head.ordinal, head.candidate_ordinal, head.raw_ordinal),
            (3, 8, 9)
        );
        assert_eq!(
            head.bounds,
            HeadTemplateBounds {
                x: 2,
                y: 3,
                width: 2,
                height: 2
            }
        );
        assert!(head.good);
    }
}
