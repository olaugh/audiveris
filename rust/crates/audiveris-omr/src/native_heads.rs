// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production composition of native upstream state into the HEADS prolog.
//!
//! This stops at the first genuinely unported HEADS algorithm: template-based
//! note-head scanning and interpretation.  Everything before that boundary is
//! concrete page state rather than a dependency-injected test fixture.

use std::{collections::BTreeMap, convert::Infallible, error::Error, fmt};

use audiveris_image::{
    lines_coordinator::StaffCandidateKind, staff_line_conversion::PersistentStaffLine,
};

use crate::{
    grid_executor::HeadlessStaffLine,
    heads_step::{
        NativeHeadRasterGlyph, NativeHeadStaffRaster, NativeHeadSystemRaster, NativeHeadsError,
        NativeHeadsPrologRaster, NeutralDistanceTable, NeutralHeadSpot,
        build_native_distance_table, retrieve_native_head_spots,
    },
    native_ledgers::NativeLedgerRecognition,
    native_stem_seeds::NativeStemSeedRecognition,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
};

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
            Self::Prolog(source) => write!(formatter, "HEADS prolog failed: {source}"),
        }
    }
}

impl Error for NativeHeadsPrologRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
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
    })
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
    }
}
