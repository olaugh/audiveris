// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of seed-head candidates through glyph retrieval.
//!
//! Java traverses provisional heads in scanner/candidate order, calls
//! `HeadInter.retrieveGlyph` against the original BINARY raster, drops a null
//! result, then inserts/tallies only the successful heads. This layer preserves
//! that boundary without allocating Java glyph or SIG identities.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    head_glyph_retrieval::{
        HeadGlyphRetrievalError, HeadGlyphRetrievalInput, RetrievedHeadGlyph, retrieve_head_glyph,
    },
    head_template::{HeadTemplateAnchor, HeadTemplateBounds, HeadTemplateShape},
    native_heads::{NativeHeadTemplateCatalogSelection, NativeHeadsPrologRecognition},
    native_heads_seed_lookup::{
        NativeHeadSeedCandidate, NativeHeadSeedSearchOutcome, NativeHeadSeedShapeSearch,
        NativeHeadSeedSide, NativeHeadsSeedLookupRecognition,
    },
    recognize::GridLinesRecognition,
};

pub struct NativeHeadsSeedGlyphsInput<'a> {
    pub grid: &'a GridLinesRecognition,
    pub heads: &'a NativeHeadsPrologRecognition,
    pub lookup: &'a NativeHeadsSeedLookupRecognition,
}

/// Successful seed heads before glyph-index registration and SIG insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadsSeedGlyphRecognition {
    pub systems: Vec<NativeHeadsSeedGlyphSystem>,
    pub candidate_count: usize,
    pub dropped_count: usize,
    pub head_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadsSeedGlyphSystem {
    pub system_id: usize,
    pub staffs: Vec<NativeHeadsSeedGlyphStaff>,
    pub candidate_count: usize,
    pub dropped_count: usize,
    pub head_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadsSeedGlyphStaff {
    pub staff_id: usize,
    pub candidate_count: usize,
    pub dropped_count: usize,
    /// Dense successful-head order returned by Java `lookupSeeds`.
    pub heads: Vec<NativeHeadSeedGlyph>,
}

/// One successful pre-registration head with all retained provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHeadSeedGlyph {
    /// Dense successful-head ordinal within the staff.
    pub ordinal: usize,
    pub candidate_ordinal: usize,
    pub search_ordinal: usize,
    pub geometry_ordinal: usize,
    pub seed_pool_index: usize,
    pub seed_source_ordinal: usize,
    pub side: NativeHeadSeedSide,
    pub anchor: HeadTemplateAnchor,
    pub original_shape: HeadTemplateShape,
    pub shape: HeadTemplateShape,
    pub x: i32,
    pub y: i32,
    pub distance_bits: u64,
    pub pitch_bits: u64,
    pub provisional_bounds: HeadTemplateBounds,
    /// Bounds after Java `HeadInter.retrieveGlyph` replaces the slim box.
    pub bounds: HeadTemplateBounds,
    pub raw_distance_impact_bits: u64,
    pub distance_impact_bits: u64,
    pub grade_bits: u64,
    pub minimum_grade_bits: u64,
    pub good: bool,
    pub glyph: RetrievedHeadGlyph,
    pub tally_left_bits: Option<u64>,
    pub tally_right_bits: Option<u64>,
}

impl NativeHeadSeedGlyph {
    #[must_use]
    pub fn distance(&self) -> f64 {
        f64::from_bits(self.distance_bits)
    }

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

    #[must_use]
    pub fn tally_left(&self) -> Option<f64> {
        self.tally_left_bits.map(f64::from_bits)
    }

    #[must_use]
    pub fn tally_right(&self) -> Option<f64> {
        self.tally_right_bits.map(f64::from_bits)
    }
}

#[derive(Debug)]
pub enum NativeHeadsSeedGlyphError {
    SystemOrder {
        source: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    CatalogSelectionOrder,
    StaffOrder {
        system_id: usize,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    CatalogStaffNotInGrid {
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
    SearchOrder {
        system_id: usize,
        staff_id: usize,
        expected: usize,
        actual: usize,
    },
    CandidateOrder {
        system_id: usize,
        staff_id: usize,
        expected: usize,
        actual: usize,
    },
    SearchReference {
        system_id: usize,
        staff_id: usize,
        candidate_ordinal: usize,
        search_ordinal: usize,
    },
    CandidateProvenance {
        system_id: usize,
        staff_id: usize,
        candidate_ordinal: usize,
        field: &'static str,
    },
    Retrieval {
        system_id: usize,
        staff_id: usize,
        candidate_ordinal: usize,
        source: HeadGlyphRetrievalError,
    },
}

impl fmt::Display for NativeHeadsSeedGlyphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph {source} system order {actual:?}, expected {expected:?}"
            ),
            Self::CatalogSelectionOrder => {
                write!(
                    formatter,
                    "HEADS seed glyph catalog selections are not strictly ordered"
                )
            }
            Self::StaffOrder {
                system_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff order {actual:?}, expected {expected:?}"
            ),
            Self::CatalogStaffNotInGrid {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS seed glyph catalog staff {staff_id} is absent from GRID system {system_id}"
            ),
            Self::MissingCatalog {
                system_id,
                staff_id,
                catalog_ordinal,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} references missing catalog \
                 {catalog_ordinal}"
            ),
            Self::CatalogPointSize {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} catalog point size {actual}, \
                 expected {expected}"
            ),
            Self::ScanOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} scanner ordinal {actual}, \
                 expected {expected}"
            ),
            Self::SearchOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} search ordinal {actual}, \
                 expected {expected}"
            ),
            Self::CandidateOrder {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} candidate ordinal {actual}, \
                 expected {expected}"
            ),
            Self::SearchReference {
                system_id,
                staff_id,
                candidate_ordinal,
                search_ordinal,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} candidate \
                 {candidate_ordinal} references missing search {search_ordinal}"
            ),
            Self::CandidateProvenance {
                system_id,
                staff_id,
                candidate_ordinal,
                field,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} candidate \
                 {candidate_ordinal} disagrees with its search on {field}"
            ),
            Self::Retrieval {
                system_id,
                staff_id,
                candidate_ordinal,
                source,
            } => write!(
                formatter,
                "HEADS seed glyph system {system_id} staff {staff_id} candidate \
                 {candidate_ordinal} retrieval failed: {source}"
            ),
        }
    }
}

impl Error for NativeHeadsSeedGlyphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Retrieval { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Retrieve concrete glyphs for every provisional seed head in Java order.
pub fn retrieve_native_heads_seed_glyphs(
    input: NativeHeadsSeedGlyphsInput<'_>,
) -> Result<NativeHeadsSeedGlyphRecognition, NativeHeadsSeedGlyphError> {
    let selections = validate_topology(&input)?;
    let mut systems = Vec::with_capacity(input.lookup.systems.len());
    let mut candidate_count = 0;
    let mut dropped_count = 0;
    let mut head_count = 0;

    for lookup_system in &input.lookup.systems {
        let system_id = lookup_system.system_id;
        let mut staffs = Vec::with_capacity(lookup_system.staffs.len());
        let mut system_candidate_count = 0;
        let mut system_dropped_count = 0;
        let mut system_head_count = 0;

        for lookup_staff in &lookup_system.staffs {
            let staff_id = lookup_staff.staff_id;
            let selection = selections[&(system_id, staff_id)];
            let catalog = input
                .heads
                .template_catalogs
                .get(selection.catalog_ordinal)
                .ok_or(NativeHeadsSeedGlyphError::MissingCatalog {
                    system_id,
                    staff_id,
                    catalog_ordinal: selection.catalog_ordinal,
                })?;
            if catalog.point_size() != selection.point_size {
                return Err(NativeHeadsSeedGlyphError::CatalogPointSize {
                    system_id,
                    staff_id,
                    expected: selection.point_size,
                    actual: catalog.point_size(),
                });
            }

            let searches = validate_staff_order(system_id, staff_id, lookup_staff)?;
            let mut heads = Vec::new();
            let mut staff_candidate_count = 0;
            let mut staff_dropped_count = 0;
            for scan in &lookup_staff.scans {
                for candidate in &scan.candidates {
                    let expected_candidate_ordinal = staff_candidate_count;
                    if candidate.ordinal != expected_candidate_ordinal {
                        return Err(NativeHeadsSeedGlyphError::CandidateOrder {
                            system_id,
                            staff_id,
                            expected: expected_candidate_ordinal,
                            actual: candidate.ordinal,
                        });
                    }
                    staff_candidate_count += 1;
                    let search = searches.get(candidate.search_ordinal).copied().ok_or(
                        NativeHeadsSeedGlyphError::SearchReference {
                            system_id,
                            staff_id,
                            candidate_ordinal: candidate.ordinal,
                            search_ordinal: candidate.search_ordinal,
                        },
                    )?;
                    validate_candidate(system_id, staff_id, scan, candidate, search)?;
                    let template = catalog.template(candidate.shape);
                    let glyph = retrieve_head_glyph(HeadGlyphRetrievalInput {
                        template,
                        slim_inter_bounds: candidate.bounds,
                        raster_width: input.grid.scale.width,
                        raster_height: input.grid.scale.height,
                        binary: &input.grid.scale.binary,
                    })
                    .map_err(|source| {
                        NativeHeadsSeedGlyphError::Retrieval {
                            system_id,
                            staff_id,
                            candidate_ordinal: candidate.ordinal,
                            source,
                        }
                    })?;
                    let Some(glyph) = glyph else {
                        staff_dropped_count += 1;
                        continue;
                    };

                    let good = candidate.good_for_tally;
                    let tally = good.then(|| {
                        tally_dx(
                            candidate.side,
                            glyph.new_inter_bounds,
                            search.axis.precise_x,
                        )
                    });
                    heads.push(NativeHeadSeedGlyph {
                        ordinal: heads.len(),
                        candidate_ordinal: candidate.ordinal,
                        search_ordinal: candidate.search_ordinal,
                        geometry_ordinal: scan.geometry_ordinal,
                        seed_pool_index: candidate.seed_pool_index,
                        seed_source_ordinal: candidate.seed_source_ordinal,
                        side: candidate.side,
                        anchor: candidate.anchor,
                        original_shape: candidate.original_shape,
                        shape: candidate.shape,
                        x: candidate.x,
                        y: candidate.y,
                        distance_bits: candidate.distance.to_bits(),
                        pitch_bits: candidate.pitch.to_bits(),
                        provisional_bounds: candidate.bounds,
                        bounds: glyph.new_inter_bounds,
                        raw_distance_impact_bits: candidate.raw_distance_impact.to_bits(),
                        distance_impact_bits: candidate.distance_impact.to_bits(),
                        grade_bits: candidate.grade.to_bits(),
                        minimum_grade_bits: candidate.minimum_grade.to_bits(),
                        good,
                        glyph,
                        tally_left_bits: match candidate.side {
                            NativeHeadSeedSide::Left => tally.map(f64::to_bits),
                            NativeHeadSeedSide::Right => None,
                        },
                        tally_right_bits: match candidate.side {
                            NativeHeadSeedSide::Left => None,
                            NativeHeadSeedSide::Right => tally.map(f64::to_bits),
                        },
                    });
                }
            }

            system_candidate_count += staff_candidate_count;
            system_dropped_count += staff_dropped_count;
            system_head_count += heads.len();
            staffs.push(NativeHeadsSeedGlyphStaff {
                staff_id,
                candidate_count: staff_candidate_count,
                dropped_count: staff_dropped_count,
                heads,
            });
        }

        candidate_count += system_candidate_count;
        dropped_count += system_dropped_count;
        head_count += system_head_count;
        systems.push(NativeHeadsSeedGlyphSystem {
            system_id,
            staffs,
            candidate_count: system_candidate_count,
            dropped_count: system_dropped_count,
            head_count: system_head_count,
        });
    }

    Ok(NativeHeadsSeedGlyphRecognition {
        systems,
        candidate_count,
        dropped_count,
        head_count,
    })
}

fn validate_topology<'a>(
    input: &'a NativeHeadsSeedGlyphsInput<'a>,
) -> Result<
    BTreeMap<(usize, usize), &'a NativeHeadTemplateCatalogSelection>,
    NativeHeadsSeedGlyphError,
> {
    let expected = input
        .lookup
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let sources = [
        (
            "GRID",
            input
                .grid
                .peak_graph
                .sig
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>(),
        ),
        (
            "HEADS prolog",
            input
                .heads
                .system_spot_ordinals
                .iter()
                .map(|(system_id, _)| *system_id)
                .collect(),
        ),
    ];
    for (source, actual) in sources {
        if actual != expected {
            return Err(NativeHeadsSeedGlyphError::SystemOrder {
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
            return Err(NativeHeadsSeedGlyphError::CatalogSelectionOrder);
        }
        prior = Some(owner);
    }

    for (lookup_system, grid_system) in input
        .lookup
        .systems
        .iter()
        .zip(&input.grid.peak_graph.sig.systems)
    {
        let expected_staffs = selections
            .keys()
            .filter_map(|&(system_id, staff_id)| {
                (system_id == lookup_system.system_id).then_some(staff_id)
            })
            .collect::<Vec<_>>();
        let actual_staffs = lookup_system
            .staffs
            .iter()
            .map(|staff| staff.staff_id)
            .collect::<Vec<_>>();
        if actual_staffs != expected_staffs {
            return Err(NativeHeadsSeedGlyphError::StaffOrder {
                system_id: lookup_system.system_id,
                expected: expected_staffs,
                actual: actual_staffs,
            });
        }
        for &staff_id in &actual_staffs {
            if !grid_system.staff_ids.contains(&staff_id) {
                return Err(NativeHeadsSeedGlyphError::CatalogStaffNotInGrid {
                    system_id: lookup_system.system_id,
                    staff_id,
                });
            }
        }
    }
    Ok(selections)
}

fn validate_staff_order(
    system_id: usize,
    staff_id: usize,
    staff: &crate::native_heads_seed_lookup::NativeHeadSeedLookupStaff,
) -> Result<Vec<&NativeHeadSeedShapeSearch>, NativeHeadsSeedGlyphError> {
    let mut searches = Vec::new();
    for (expected_scan, scan) in staff.scans.iter().enumerate() {
        if scan.geometry_ordinal != expected_scan {
            return Err(NativeHeadsSeedGlyphError::ScanOrder {
                system_id,
                staff_id,
                expected: expected_scan,
                actual: scan.geometry_ordinal,
            });
        }
        for search in &scan.searches {
            let expected_search = searches.len();
            if search.ordinal != expected_search {
                return Err(NativeHeadsSeedGlyphError::SearchOrder {
                    system_id,
                    staff_id,
                    expected: expected_search,
                    actual: search.ordinal,
                });
            }
            searches.push(search);
        }
    }
    Ok(searches)
}

fn validate_candidate(
    system_id: usize,
    staff_id: usize,
    scan: &crate::native_heads_seed_lookup::NativeHeadSeedLookupScan,
    candidate: &NativeHeadSeedCandidate,
    search: &NativeHeadSeedShapeSearch,
) -> Result<(), NativeHeadsSeedGlyphError> {
    let mismatch = |field| NativeHeadsSeedGlyphError::CandidateProvenance {
        system_id,
        staff_id,
        candidate_ordinal: candidate.ordinal,
        field,
    };
    if candidate.search_ordinal != search.ordinal {
        return Err(mismatch("search ordinal"));
    }
    if candidate.seed_pool_index != search.seed_pool_index
        || candidate.seed_source_ordinal != search.seed_source_ordinal
    {
        return Err(mismatch("seed identity"));
    }
    if candidate.side != search.side || candidate.anchor != search.anchor {
        return Err(mismatch("stem side"));
    }
    if candidate.original_shape != search.original_shape || candidate.shape != search.final_shape {
        return Err(mismatch("shape"));
    }
    if candidate.pitch.to_bits() != f64::from(scan.pitch).to_bits() {
        return Err(mismatch("pitch"));
    }
    let Some(best) = search.best else {
        return Err(mismatch("best location"));
    };
    if candidate.x != best.x
        || candidate.y != best.y
        || candidate.distance.to_bits() != best.distance.to_bits()
    {
        return Err(mismatch("selected location"));
    }
    if candidate.bounds != search.slim_bounds.ok_or_else(|| mismatch("slim bounds"))? {
        return Err(mismatch("slim bounds"));
    }
    if search.outcome
        != (NativeHeadSeedSearchOutcome::ProvisionalCandidate {
            candidate_ordinal: candidate.ordinal,
        })
    {
        return Err(mismatch("search outcome"));
    }
    if candidate.anchor != candidate.side.anchor() {
        return Err(mismatch("anchor"));
    }
    Ok(())
}

fn tally_dx(side: NativeHeadSeedSide, bounds: HeadTemplateBounds, x0: i32) -> f64 {
    match side {
        NativeHeadSeedSide::Left => f64::from(bounds.x.wrapping_sub(x0)) + 0.5,
        NativeHeadSeedSide::Right => {
            let inclusive_right = bounds.x.wrapping_add(bounds.width).wrapping_sub(1);
            f64::from(x0) + 0.5 - f64::from(inclusive_right)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tally_uses_the_post_retrieval_bounds_and_only_the_candidate_side() {
        let bounds = HeadTemplateBounds {
            x: 10,
            y: 20,
            width: 5,
            height: 7,
        };
        assert_eq!(tally_dx(NativeHeadSeedSide::Left, bounds, 15), -4.5);
        assert_eq!(tally_dx(NativeHeadSeedSide::Right, bounds, 15), 1.5);
    }
}
