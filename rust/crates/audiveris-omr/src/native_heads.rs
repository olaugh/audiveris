// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of native upstream state through the complete HEADS
//! step.
//!
//! The public entry point retains every independently useful intermediate
//! product while running the same typed kernels used by the focused seed,
//! range, glyph-retrieval, and epilog differential tests.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    error::Error,
    fmt,
};

use audiveris_image::{
    lines_coordinator::StaffCandidateKind, staff_line_conversion::PersistentStaffLine,
};

use crate::{
    grid_executor::HeadlessStaffLine,
    head_template::HeadTemplateCatalog,
    head_template_catalog::{HeadTemplateCatalogAssetError, load_bravura_head_template_catalogs},
    heads_step::{
        NativeHeadRasterGlyph, NativeHeadStaffRaster, NativeHeadSystemRaster, NativeHeadsError,
        NativeHeadsPrologRaster, NeutralDistanceTable, NeutralHeadSpot,
        build_native_distance_table, retrieve_native_head_spots,
    },
    native_headers::NativeHeaderRecognition,
    native_heads_bar_slices::{
        NativeHeadScannerBarSliceError, NativeHeadScannerBarSlicesRecognition,
        materialize_native_head_scanner_bar_slices,
    },
    native_heads_competitor_slices::{
        NativeHeadScannerCompetitorSliceError, NativeHeadScannerCompetitorSlicesRecognition,
        materialize_native_head_scanner_competitor_slices,
    },
    native_heads_competitors::{
        NativeHeadsCompetitorError, NativeHeadsCompetitorPool, materialize_native_heads_competitors,
    },
    native_heads_epilog::{
        NativeHeadsEpilogError, NativeHeadsEpilogInput, NativeHeadsEpilogRecognition,
        compose_native_heads_epilog,
    },
    native_heads_obstacles::{
        NativeHeadsBarObstacleError, NativeHeadsBarObstaclePool,
        materialize_native_heads_bar_obstacles,
    },
    native_heads_range_glyphs::{
        NativeHeadsRangeGlyphError, NativeHeadsRangeGlyphRecognition, NativeHeadsRangeGlyphsInput,
        retrieve_native_heads_range_glyphs,
    },
    native_heads_range_lookup::{
        NativeHeadsRangeLookupError, NativeHeadsRangeLookupInput,
        NativeHeadsRangeLookupRecognition, recognize_native_heads_range_lookup,
    },
    native_heads_scanner::{
        NativeHeadsScannerRecognition, NativeHeadsScannerRecognitionError,
        recognize_native_heads_scanner_context,
    },
    native_heads_scanner_pools::{
        NativeHeadScannerPoolError, NativeHeadScannerPoolsRecognition,
        materialize_native_head_scanner_pools,
    },
    native_heads_seed_glyphs::{
        NativeHeadsSeedGlyphError, NativeHeadsSeedGlyphRecognition, NativeHeadsSeedGlyphsInput,
        retrieve_native_heads_seed_glyphs,
    },
    native_heads_seed_lookup::{
        NativeHeadsSeedLookupError, NativeHeadsSeedLookupInput, NativeHeadsSeedLookupRecognition,
        recognize_native_heads_seed_lookup,
    },
    native_ledgers::NativeLedgerRecognition,
    native_stem_seeds::NativeStemSeedRecognition,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
};

/// Complete native product of Java's HEADS step.
///
/// The fields retain each independently useful production boundary without
/// borrowing the upstream GRID, HEADERS, STEM_SEEDS, BEAMS, or LEDGERS state.
/// Their vector order is the order established by the production calls: sheet
/// system order, then staff/scanner/candidate source order unless a field's
/// narrower contract documents Java's explicit stable sort.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsRecognition {
    pub obstacles: NativeHeadsBarObstaclePool,
    pub prolog: NativeHeadsPrologRecognition,
    pub scanners: NativeHeadsScannerRecognition,
    pub scanner_pools: NativeHeadScannerPoolsRecognition,
    pub bar_slices: NativeHeadScannerBarSlicesRecognition,
    pub competitors: NativeHeadsCompetitorPool,
    pub competitor_slices: NativeHeadScannerCompetitorSlicesRecognition,
    pub seed_lookup: NativeHeadsSeedLookupRecognition,
    pub seed_glyphs: NativeHeadsSeedGlyphRecognition,
    pub range_lookup: NativeHeadsRangeLookupRecognition,
    pub range_glyphs: NativeHeadsRangeGlyphRecognition,
    pub epilog: NativeHeadsEpilogRecognition,
}

/// Failure at one concrete production boundary in [`recognize_native_heads`].
#[derive(Debug)]
pub enum NativeHeadsRecognitionError {
    Obstacles(NativeHeadsBarObstacleError),
    Prolog(NativeHeadsPrologRecognitionError),
    Scanners(NativeHeadsScannerRecognitionError),
    ScannerPools(NativeHeadScannerPoolError),
    BarSlices(NativeHeadScannerBarSliceError),
    Competitors(NativeHeadsCompetitorError),
    CompetitorSlices(NativeHeadScannerCompetitorSliceError),
    SeedLookup(NativeHeadsSeedLookupError),
    SeedGlyphs(NativeHeadsSeedGlyphError),
    RangeLookup(NativeHeadsRangeLookupError),
    RangeGlyphs(NativeHeadsRangeGlyphError),
    Epilog(NativeHeadsEpilogError),
}

impl fmt::Display for NativeHeadsRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (boundary, source): (&str, &dyn fmt::Display) = match self {
            Self::Obstacles(source) => ("bar obstacles", source),
            Self::Prolog(source) => ("prolog", source),
            Self::Scanners(source) => ("scanner context", source),
            Self::ScannerPools(source) => ("scanner pools", source),
            Self::BarSlices(source) => ("bar slices", source),
            Self::Competitors(source) => ("competitors", source),
            Self::CompetitorSlices(source) => ("competitor slices", source),
            Self::SeedLookup(source) => ("seed lookup", source),
            Self::SeedGlyphs(source) => ("seed glyph retrieval", source),
            Self::RangeLookup(source) => ("range lookup", source),
            Self::RangeGlyphs(source) => ("range glyph retrieval", source),
            Self::Epilog(source) => ("epilog", source),
        };
        write!(formatter, "HEADS {boundary} failed: {source}")
    }
}

impl Error for NativeHeadsRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Obstacles(source) => source,
            Self::Prolog(source) => source,
            Self::Scanners(source) => source,
            Self::ScannerPools(source) => source,
            Self::BarSlices(source) => source,
            Self::Competitors(source) => source,
            Self::CompetitorSlices(source) => source,
            Self::SeedLookup(source) => source,
            Self::SeedGlyphs(source) => source,
            Self::RangeLookup(source) => source,
            Self::RangeGlyphs(source) => source,
            Self::Epilog(source) => source,
        })
    }
}

/// Run the complete native HEADS step over live upstream recognition products.
///
/// No oracle state or reconstructed candidates enter this path. Each stage
/// consumes the preceding owned production product directly, so Java's source
/// and stable-sort ordering contracts remain visible in the returned value.
pub fn recognize_native_heads(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
) -> Result<NativeHeadsRecognition, NativeHeadsRecognitionError> {
    let obstacles = materialize_native_heads_bar_obstacles(grid)
        .map_err(NativeHeadsRecognitionError::Obstacles)?;
    let competitors = materialize_native_heads_competitors(grid, beams)
        .map_err(NativeHeadsRecognitionError::Competitors)?;
    let prolog = recognize_native_heads_prolog(grid, beams, ledgers, stem_seeds)
        .map_err(NativeHeadsRecognitionError::Prolog)?;
    let scanners =
        recognize_native_heads_scanner_context(grid, headers, ledgers, stem_seeds, &prolog)
            .map_err(NativeHeadsRecognitionError::Scanners)?;
    let scanner_pools = materialize_native_head_scanner_pools(stem_seeds, &prolog, &scanners)
        .map_err(NativeHeadsRecognitionError::ScannerPools)?;
    let bar_slices = materialize_native_head_scanner_bar_slices(&scanner_pools, &obstacles)
        .map_err(NativeHeadsRecognitionError::BarSlices)?;
    let competitor_slices =
        materialize_native_head_scanner_competitor_slices(&scanner_pools, &competitors)
            .map_err(NativeHeadsRecognitionError::CompetitorSlices)?;
    let seed_lookup = recognize_native_heads_seed_lookup(NativeHeadsSeedLookupInput {
        heads: &prolog,
        scanners: &scanners,
        pools: &scanner_pools,
        obstacles: &obstacles,
        bar_slices: &bar_slices,
        competitors: &competitors,
        competitor_slices: &competitor_slices,
        stem_seeds,
    })
    .map_err(NativeHeadsRecognitionError::SeedLookup)?;
    let seed_glyphs = retrieve_native_heads_seed_glyphs(NativeHeadsSeedGlyphsInput {
        grid,
        heads: &prolog,
        lookup: &seed_lookup,
    })
    .map_err(NativeHeadsRecognitionError::SeedGlyphs)?;
    let range_lookup = recognize_native_heads_range_lookup(NativeHeadsRangeLookupInput {
        heads: &prolog,
        scanners: &scanners,
        pools: &scanner_pools,
        obstacles: &obstacles,
        bar_slices: &bar_slices,
        competitors: &competitors,
        competitor_slices: &competitor_slices,
    })
    .map_err(NativeHeadsRecognitionError::RangeLookup)?;
    let range_glyphs = retrieve_native_heads_range_glyphs(NativeHeadsRangeGlyphsInput {
        grid,
        heads: &prolog,
        scanners: &scanners,
        pools: &scanner_pools,
        competitors: &competitors,
        range_lookup: &range_lookup,
        seed_glyphs: &seed_glyphs,
    })
    .map_err(NativeHeadsRecognitionError::RangeGlyphs)?;
    let epilog = compose_native_heads_epilog(NativeHeadsEpilogInput {
        seed_glyphs: &seed_glyphs,
        range_glyphs: &range_glyphs,
        competitors: &competitors,
        beams,
        beam_veto_scale: crate::beam_veto::BeamVetoScale::from_env(
            f64::from(grid.scale.scale.interline.main),
            f64::from(grid.scale.scale.beam.main),
        ),
    })
    .map_err(NativeHeadsRecognitionError::Epilog)?;

    Ok(NativeHeadsRecognition {
        obstacles,
        prolog,
        scanners,
        scanner_pools,
        bar_slices,
        competitors,
        competitor_slices,
        seed_lookup,
        seed_glyphs,
        range_lookup,
        range_glyphs,
        epilog,
    })
}

/// Bounded production result immediately before Java `NoteHeadsBuilder`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsPrologRecognition {
    /// Java's duplicated BINARY buffer after staff, ledger and seed erasure.
    pub binary_without_lines: Vec<u8>,
    /// Exact Chamfer-3 table after the same pixels are marked unknown.
    pub distance_table: NeutralDistanceTable,
    /// Transient HEAD_SPOT components in Java `GlyphFactory` order.
    pub spots: Vec<NeutralHeadSpot>,
    /// Zero-based entries in `spots`, dispatched in system/source order.
    pub system_spot_ordinals: Vec<(usize, Vec<usize>)>,
    /// Complete checked-in Bravura catalog set, sorted by point size.
    pub template_catalogs: Vec<HeadTemplateCatalog>,
    /// Exact catalog selected for each non-tablature staff, in system/staff order.
    pub staff_template_catalogs: Vec<NativeHeadTemplateCatalogSelection>,
}

/// One Java `TemplateFactory.getCatalog(family, staff.getHeadPointSize())` selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadTemplateCatalogSelection {
    pub system_id: usize,
    pub staff_id: usize,
    pub specific_interline: i32,
    pub point_size: i32,
    /// Zero-based entry in [`NativeHeadsPrologRecognition::template_catalogs`].
    pub catalog_ordinal: usize,
}

#[derive(Debug)]
pub enum NativeHeadsPrologRecognitionError {
    SystemAreas {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    SystemBounds {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    StemSystems {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    MissingStaff {
        system_id: usize,
        staff_id: usize,
    },
    NonPersistentStaffLine {
        system_id: usize,
        staff_id: usize,
        line_index: usize,
    },
    LedgerCount {
        inters: usize,
        glyphs: usize,
    },
    LedgerIdentity {
        ordinal: usize,
    },
    LedgerOwner {
        system_id: usize,
        staff_id: usize,
    },
    InvalidStemGlyph {
        system_id: usize,
        ordinal: usize,
    },
    CoordinateOutOfRange {
        kind: &'static str,
        system_id: usize,
        ordinal: usize,
    },
    UnknownSpotSystem {
        spot_ordinal: usize,
        system_id: usize,
    },
    HeadPointSizeCount {
        expected: usize,
        actual: usize,
    },
    HeadPointSizeIdentity {
        ordinal: usize,
        expected_system_id: usize,
        expected_staff_id: usize,
        expected_interline: i32,
        actual_system_id: usize,
        actual_staff_id: usize,
        actual_interline: i32,
    },
    DuplicateHeadPointSizeOwner {
        system_id: usize,
        staff_id: usize,
    },
    UnknownHeadPointSizeOwner {
        system_id: usize,
        staff_id: usize,
    },
    MissingHeadPointSize {
        system_id: usize,
        staff_id: usize,
    },
    HeadPointSizeValue {
        system_id: usize,
        staff_id: usize,
        expected: i32,
        actual: i32,
    },
    MissingTemplateCatalog {
        system_id: usize,
        staff_id: usize,
        point_size: i32,
    },
    TemplateCatalog(HeadTemplateCatalogAssetError),
    Prolog(NativeHeadsError<Infallible>),
}

impl fmt::Display for NativeHeadsPrologRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemAreas { expected, actual } => {
                write!(
                    formatter,
                    "HEADS system areas {actual:?}, expected {expected:?}"
                )
            }
            Self::SystemBounds { expected, actual } => {
                write!(
                    formatter,
                    "HEADS system bounds {actual:?}, expected {expected:?}"
                )
            }
            Self::StemSystems { expected, actual } => {
                write!(
                    formatter,
                    "HEADS stem systems {actual:?}, expected {expected:?}"
                )
            }
            Self::MissingStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS system {system_id} is missing GRID staff {staff_id}"
            ),
            Self::NonPersistentStaffLine {
                system_id,
                staff_id,
                line_index,
            } => write!(
                formatter,
                "HEADS system {system_id} staff {staff_id} line {line_index} is not persistent"
            ),
            Self::LedgerCount { inters, glyphs } => write!(
                formatter,
                "HEADS received {inters} live ledgers but {glyphs} fixed ledger glyphs"
            ),
            Self::LedgerIdentity { ordinal } => {
                write!(
                    formatter,
                    "HEADS ledger glyph {ordinal} does not match its live inter"
                )
            }
            Self::LedgerOwner {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS ledger glyph belongs to unknown system {system_id} staff {staff_id}"
            ),
            Self::InvalidStemGlyph { system_id, ordinal } => write!(
                formatter,
                "HEADS system {system_id} seed {ordinal} is not an accepted free vertical glyph"
            ),
            Self::CoordinateOutOfRange {
                kind,
                system_id,
                ordinal,
            } => write!(
                formatter,
                "HEADS {kind} {ordinal} in system {system_id} has coordinates outside Java int"
            ),
            Self::UnknownSpotSystem {
                spot_ordinal,
                system_id,
            } => write!(
                formatter,
                "HEADS spot {spot_ordinal} references unknown system {system_id}"
            ),
            Self::HeadPointSizeCount { expected, actual } => write!(
                formatter,
                "HEADS received {actual} staff head point sizes, expected {expected}"
            ),
            Self::HeadPointSizeIdentity {
                ordinal,
                expected_system_id,
                expected_staff_id,
                expected_interline,
                actual_system_id,
                actual_staff_id,
                actual_interline,
            } => write!(
                formatter,
                "HEADS staff point size {ordinal} identifies system {actual_system_id} staff \
                 {actual_staff_id} interline {actual_interline}, expected system \
                 {expected_system_id} staff {expected_staff_id} interline {expected_interline}"
            ),
            Self::DuplicateHeadPointSizeOwner {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS received duplicate point sizes for system {system_id} staff {staff_id}"
            ),
            Self::UnknownHeadPointSizeOwner {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS received a point size for unknown system {system_id} staff {staff_id}"
            ),
            Self::MissingHeadPointSize {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADS is missing the point size for system {system_id} staff {staff_id}"
            ),
            Self::HeadPointSizeValue {
                system_id,
                staff_id,
                expected,
                actual,
            } => write!(
                formatter,
                "HEADS system {system_id} staff {staff_id} point size is {actual}, expected \
                 {expected} from the retained music-font scale"
            ),
            Self::MissingTemplateCatalog {
                system_id,
                staff_id,
                point_size,
            } => write!(
                formatter,
                "HEADS system {system_id} staff {staff_id} needs unpinned Bravura template point \
                 size {point_size}"
            ),
            Self::TemplateCatalog(source) => {
                write!(formatter, "HEADS template catalog failed: {source}")
            }
            Self::Prolog(source) => write!(formatter, "HEADS prolog failed: {source}"),
        }
    }
}

impl Error for NativeHeadsPrologRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TemplateCatalog(source) => Some(source),
            Self::Prolog(source) => Some(source),
            _ => None,
        }
    }
}

/// Compose GRID, BEAMS, LEDGERS and STEM_SEEDS into the exact raster inputs
/// consumed by Java `DistancesBuilder` and `HeadSpotsBuilder`.
pub fn compose_native_heads_prolog_raster(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    stem_seeds: &NativeStemSeedRecognition,
) -> Result<NativeHeadsPrologRaster, NativeHeadsPrologRecognitionError> {
    let expected = (1..=grid.peak_graph.systems.len()).collect::<Vec<_>>();
    let areas = grid
        .system_areas
        .iter()
        .map(|area| area.system_id)
        .collect::<Vec<_>>();
    if areas != expected {
        return Err(NativeHeadsPrologRecognitionError::SystemAreas {
            expected,
            actual: areas,
        });
    }
    let expected = (1..=grid.peak_graph.systems.len()).collect::<Vec<_>>();
    let bounds = grid
        .system_bounds
        .iter()
        .map(|bounds| bounds.system_id)
        .collect::<Vec<_>>();
    if bounds != expected {
        return Err(NativeHeadsPrologRecognitionError::SystemBounds {
            expected,
            actual: bounds,
        });
    }
    let expected = (1..=grid.peak_graph.systems.len()).collect::<Vec<_>>();
    let stem_systems = stem_seeds
        .systems
        .iter()
        .map(|system| system.raw.system_id)
        .collect::<Vec<_>>();
    if stem_systems != expected {
        return Err(NativeHeadsPrologRecognitionError::StemSystems {
            expected,
            actual: stem_systems,
        });
    }

    let live_ledgers = ledgers.ledgers();
    if live_ledgers.len() != ledgers.ledger_glyphs.len() {
        return Err(NativeHeadsPrologRecognitionError::LedgerCount {
            inters: live_ledgers.len(),
            glyphs: ledgers.ledger_glyphs.len(),
        });
    }
    let mut ledger_rasters = BTreeMap::<(usize, usize), Vec<NativeHeadRasterGlyph>>::new();
    for (ordinal, (inter, glyph)) in live_ledgers.iter().zip(&ledgers.ledger_glyphs).enumerate() {
        if inter.system_id != glyph.system_id
            || inter.staff_id != glyph.staff_id
            || inter.id != glyph.inter_id
            || inter.glyph_id != glyph.glyph_id
            || inter.filament_id != glyph.filament_id
        {
            return Err(NativeHeadsPrologRecognitionError::LedgerIdentity { ordinal });
        }
        let left = i32::try_from(glyph.bounds.x).map_err(|_| {
            NativeHeadsPrologRecognitionError::CoordinateOutOfRange {
                kind: "ledger glyph",
                system_id: glyph.system_id,
                ordinal,
            }
        })?;
        let top = i32::try_from(glyph.bounds.y).map_err(|_| {
            NativeHeadsPrologRecognitionError::CoordinateOutOfRange {
                kind: "ledger glyph",
                system_id: glyph.system_id,
                ordinal,
            }
        })?;
        ledger_rasters
            .entry((glyph.system_id, glyph.staff_id))
            .or_default()
            .push(NativeHeadRasterGlyph {
                left,
                top,
                runs: glyph.run_table.clone(),
            });
    }

    let mut systems = Vec::with_capacity(grid.peak_graph.systems.len());
    for (system_index, staff_ids) in grid.peak_graph.systems.iter().enumerate() {
        let system_id = system_index + 1;
        let seed_system = &stem_seeds.systems[system_index];
        let mut vertical_seed_glyphs = Vec::with_capacity(seed_system.free_glyphs.len());
        for (ordinal, glyph) in seed_system.free_glyphs.iter().enumerate() {
            if !glyph.free || !glyph.vertical_seed_group {
                return Err(NativeHeadsPrologRecognitionError::InvalidStemGlyph {
                    system_id,
                    ordinal,
                });
            }
            let left = i32::try_from(glyph.bounds.x).map_err(|_| {
                NativeHeadsPrologRecognitionError::CoordinateOutOfRange {
                    kind: "stem glyph",
                    system_id,
                    ordinal,
                }
            })?;
            let top = i32::try_from(glyph.bounds.y).map_err(|_| {
                NativeHeadsPrologRecognitionError::CoordinateOutOfRange {
                    kind: "stem glyph",
                    system_id,
                    ordinal,
                }
            })?;
            vertical_seed_glyphs.push(NativeHeadRasterGlyph {
                left,
                top,
                runs: glyph.run_table.clone(),
            });
        }

        let mut staves = Vec::with_capacity(staff_ids.len());
        for &staff_id in staff_ids {
            let staff = grid
                .peak_graph
                .sheet_staffs
                .iter()
                .find(|staff| staff.id == staff_id)
                .ok_or(NativeHeadsPrologRecognitionError::MissingStaff {
                    system_id,
                    staff_id,
                })?;
            let mut lines = Vec::<PersistentStaffLine>::with_capacity(staff.lines.len());
            for (line_index, line) in staff.lines.iter().enumerate() {
                let HeadlessStaffLine::Persistent { line, .. } = line else {
                    return Err(NativeHeadsPrologRecognitionError::NonPersistentStaffLine {
                        system_id,
                        staff_id,
                        line_index,
                    });
                };
                lines.push(line.clone());
            }
            staves.push(NativeHeadStaffRaster {
                tablature: staff.kind == StaffCandidateKind::Tablature,
                lines,
                ledger_glyphs: ledger_rasters
                    .remove(&(system_id, staff_id))
                    .unwrap_or_default(),
            });
        }
        systems.push(NativeHeadSystemRaster {
            system_id,
            staves,
            vertical_seed_glyphs,
        });
    }
    if let Some((&(system_id, staff_id), _)) = ledger_rasters.first_key_value() {
        return Err(NativeHeadsPrologRecognitionError::LedgerOwner {
            system_id,
            staff_id,
        });
    }

    Ok(NativeHeadsPrologRaster {
        binary: grid.scale.vertical_runs.clone(),
        head_spots: beams.head_spot_runs.clone(),
        systems,
        system_areas: grid.system_areas.clone(),
    })
}

/// Run the concrete native HEADS prolog over real upstream recognition products.
pub fn recognize_native_heads_prolog(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    stem_seeds: &NativeStemSeedRecognition,
) -> Result<NativeHeadsPrologRecognition, NativeHeadsPrologRecognitionError> {
    let raster = compose_native_heads_prolog_raster(grid, beams, ledgers, stem_seeds)?;
    let (template_catalogs, staff_template_catalogs) =
        select_native_head_template_catalogs(grid, beams)?;
    if raster.head_spots.width() != raster.binary.width()
        || raster.head_spots.height() != raster.binary.height()
    {
        return Err(NativeHeadsPrologRecognitionError::Prolog(
            NativeHeadsError::HeadSpotDimensions {
                binary_width: raster.binary.width(),
                binary_height: raster.binary.height(),
                spot_width: raster.head_spots.width(),
                spot_height: raster.head_spots.height(),
            },
        ));
    }
    let (binary_without_lines, distance_table) =
        build_native_distance_table::<Infallible>(&raster, |_| {})
            .map_err(NativeHeadsPrologRecognitionError::Prolog)?;
    let spots = retrieve_native_head_spots(&raster);

    let bounds_by_system = grid
        .system_bounds
        .iter()
        .map(|bounds| (bounds.system_id, bounds))
        .collect::<BTreeMap<_, _>>();
    let mut system_spot_ordinals = grid
        .system_bounds
        .iter()
        .map(|bounds| (bounds.system_id, Vec::new()))
        .collect::<Vec<_>>();
    for (spot_ordinal, spot) in spots.iter().enumerate() {
        for &system_id in &spot.relevant_system_ids {
            let bounds = bounds_by_system.get(&system_id).ok_or(
                NativeHeadsPrologRecognitionError::UnknownSpotSystem {
                    spot_ordinal,
                    system_id,
                },
            )?;
            if spot.center_x >= bounds.left && spot.center_x <= bounds.java_right() {
                system_spot_ordinals[system_id - 1].1.push(spot_ordinal);
            }
        }
    }

    Ok(NativeHeadsPrologRecognition {
        binary_without_lines,
        distance_table,
        spots,
        system_spot_ordinals,
        template_catalogs,
        staff_template_catalogs,
    })
}

fn select_native_head_template_catalogs(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
) -> Result<
    (
        Vec<HeadTemplateCatalog>,
        Vec<NativeHeadTemplateCatalogSelection>,
    ),
    NativeHeadsPrologRecognitionError,
> {
    let expected_staffs = grid
        .peak_graph
        .systems
        .iter()
        .enumerate()
        .flat_map(|(system_index, staff_ids)| {
            staff_ids
                .iter()
                .map(move |&staff_id| (system_index + 1, staff_id))
        })
        .collect::<Vec<_>>();
    if beams.staff_head_point_sizes.len() != expected_staffs.len() {
        return Err(NativeHeadsPrologRecognitionError::HeadPointSizeCount {
            expected: expected_staffs.len(),
            actual: beams.staff_head_point_sizes.len(),
        });
    }

    let expected_owners = expected_staffs.iter().copied().collect::<BTreeSet<_>>();
    let mut point_sizes_by_owner = BTreeMap::new();
    for point_size in &beams.staff_head_point_sizes {
        let owner = (point_size.system_id, point_size.staff_id);
        if !expected_owners.contains(&owner) {
            return Err(
                NativeHeadsPrologRecognitionError::UnknownHeadPointSizeOwner {
                    system_id: point_size.system_id,
                    staff_id: point_size.staff_id,
                },
            );
        }
        if point_sizes_by_owner.insert(owner, point_size).is_some() {
            return Err(
                NativeHeadsPrologRecognitionError::DuplicateHeadPointSizeOwner {
                    system_id: point_size.system_id,
                    staff_id: point_size.staff_id,
                },
            );
        }
    }

    let template_catalogs = load_bravura_head_template_catalogs()
        .map_err(NativeHeadsPrologRecognitionError::TemplateCatalog)?;
    let mut selections = Vec::with_capacity(expected_staffs.len());
    for (ordinal, (system_id, staff_id)) in expected_staffs.into_iter().enumerate() {
        let point_size = point_sizes_by_owner.remove(&(system_id, staff_id)).ok_or(
            NativeHeadsPrologRecognitionError::MissingHeadPointSize {
                system_id,
                staff_id,
            },
        )?;
        let staff = grid
            .peak_graph
            .sheet_staffs
            .iter()
            .find(|staff| staff.id == staff_id)
            .ok_or(NativeHeadsPrologRecognitionError::MissingStaff {
                system_id,
                staff_id,
            })?;
        let specific_interline = i32::try_from(staff.interline).map_err(|_| {
            NativeHeadsPrologRecognitionError::CoordinateOutOfRange {
                kind: "staff interline",
                system_id,
                ordinal,
            }
        })?;
        if point_size.system_id != system_id
            || point_size.staff_id != staff_id
            || point_size.specific_interline != specific_interline
        {
            return Err(NativeHeadsPrologRecognitionError::HeadPointSizeIdentity {
                ordinal,
                expected_system_id: system_id,
                expected_staff_id: staff_id,
                expected_interline: specific_interline,
                actual_system_id: point_size.system_id,
                actual_staff_id: point_size.staff_id,
                actual_interline: point_size.specific_interline,
            });
        }
        let expected_point_size = audiveris_music_font::head_point_size(
            beams.music_font_scale.map(|scale| scale.point_size),
            grid.scale.scale.interline.main,
            f64::from(specific_interline),
        );
        if point_size.point_size != expected_point_size {
            return Err(NativeHeadsPrologRecognitionError::HeadPointSizeValue {
                system_id,
                staff_id,
                expected: expected_point_size,
                actual: point_size.point_size,
            });
        }
        if staff.kind == StaffCandidateKind::Tablature {
            continue;
        }

        let catalog_ordinal = template_catalogs
            .iter()
            .position(|catalog| catalog.point_size() == point_size.point_size)
            .ok_or(NativeHeadsPrologRecognitionError::MissingTemplateCatalog {
                system_id,
                staff_id,
                point_size: point_size.point_size,
            })?;
        selections.push(NativeHeadTemplateCatalogSelection {
            system_id,
            staff_id,
            specific_interline,
            point_size: point_size.point_size,
            catalog_ordinal,
        });
    }
    debug_assert!(point_sizes_by_owner.is_empty());
    Ok((template_catalogs, selections))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_headers::recognize_native_headers,
        native_ledgers::recognize_native_ledgers,
        native_stem_seeds::recognize_native_stem_seeds,
        recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
    };
    use std::path::PathBuf;

    fn repo_path(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(path)
    }

    #[test]
    fn chula_reaches_the_real_note_heads_builder_boundary() {
        let grid = recognize_grid_lines(repo_path("data/examples/chula.png")).expect("GRID");
        let headers = recognize_native_headers(&grid).expect("HEADERS");
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .expect("BEAMS");
        let ledgers = recognize_native_ledgers(&grid, &beams).expect("LEDGERS");
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .expect("HEADS prolog");

        assert_eq!(heads.distance_table.width, grid.scale.width);
        assert_eq!(heads.distance_table.height, grid.scale.height);
        assert_eq!(heads.distance_table.normalizer, 3);
        assert_eq!(
            heads.binary_without_lines.len(),
            grid.scale.width * grid.scale.height
        );
        assert!(!heads.spots.is_empty());
        assert_eq!(
            heads.system_spot_ordinals.len(),
            grid.peak_graph.systems.len()
        );
        assert!(
            heads
                .system_spot_ordinals
                .iter()
                .any(|(_, ordinals)| !ordinals.is_empty())
        );
        assert_eq!(
            heads
                .template_catalogs
                .iter()
                .map(HeadTemplateCatalog::point_size)
                .collect::<Vec<_>>(),
            [24, 25, 26, 27, 28, 29, 30, 78, 83, 84, 85, 87]
        );
        assert_eq!(heads.staff_template_catalogs.len(), 6);
        assert!(heads.staff_template_catalogs.iter().all(|selection| {
            selection.point_size == 84
                && selection.catalog_ordinal == 9
                && heads.template_catalogs[selection.catalog_ordinal].point_size()
                    == selection.point_size
        }));

        let mut wrong_valid_catalog = beams.clone();
        wrong_valid_catalog.staff_head_point_sizes[0].point_size = 85;
        assert!(matches!(
            select_native_head_template_catalogs(&grid, &wrong_valid_catalog),
            Err(NativeHeadsPrologRecognitionError::HeadPointSizeValue {
                system_id: 1,
                staff_id: 1,
                expected: 84,
                actual: 85,
            })
        ));

        let mut unsupported = beams.clone();
        unsupported.music_font_scale.as_mut().unwrap().point_size = 79;
        for point_size in &mut unsupported.staff_head_point_sizes {
            point_size.point_size = 79;
        }
        assert!(matches!(
            select_native_head_template_catalogs(&grid, &unsupported),
            Err(NativeHeadsPrologRecognitionError::MissingTemplateCatalog {
                system_id: 1,
                staff_id: 1,
                point_size: 79,
            })
        ));
    }
}
