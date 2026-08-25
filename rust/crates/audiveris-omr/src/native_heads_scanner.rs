// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production construction of Java `NoteHeadsBuilder.Scanner` geometry.
//!
//! This layer deliberately stops before image lookup. It composes the exact
//! staff splines, fixed ledger glyph axes, HEADERS state, part topology,
//! per-staff template selection, and HEADS builder scale into the immutable
//! contexts shared by Java's seed and range passes.

use std::{collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    bars_logic::{PlannedPart, StaffBoundaryLine, is_merged_two_staff_part},
    glyph_factory::build_glyph_components,
    lines_coordinator::StaffCandidateKind,
    section::Bounds,
};

use crate::{
    beam_recognizer::pixel_line,
    clef_column::NeutralClefKind,
    grid_executor::{HeadlessStaff, HeadlessStaffLine},
    head_scanner::{
        HeadScannerError, HeadScannerLedger, HeadScannerLine, HeadScannerParameters,
        HeadTheoreticalOrdinateInput, NORMAL_HEAD_SCANNER_SHAPES, SMALL_HEAD_SCANNER_SHAPES,
        compute_y_offsets, is_open_scanner, theoretical_ordinate,
    },
    head_scanner_geometry::{
        HeadScannerAxis, HeadScannerGeometryError, LedgerScannerAdapter, StaffLineScannerAdapter,
    },
    head_template::HeadTemplateShape,
    native_headers::{NativeHeaderRecognition, NativeHeaderStaffRecognition},
    native_heads::{NativeHeadTemplateCatalogSelection, NativeHeadsPrologRecognition},
    native_ledgers::{NativeLedgerGlyph, NativeLedgerRecognition},
    native_stem_seeds::NativeStemSeedRecognition,
    recognize::GridLinesRecognition,
};

/// Exact immutable input contexts constructed before either scanner lookup pass.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsScannerRecognition {
    pub systems: Vec<NativeHeadsScannerSystem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsScannerSystem {
    pub system_id: usize,
    pub parameters: HeadScannerParameters,
    pub staffs: Vec<NativeHeadsScannerStaff>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsScannerStaff {
    pub system_id: usize,
    pub staff_id: usize,
    pub tablature: bool,
    pub drum: bool,
    pub line_count: usize,
    pub specific_interline: i32,
    pub header_stop: i32,
    /// Operational merged-grand-staff flag. Part identity/range is deliberately
    /// not exposed until GRID's production brace path is complete.
    pub merged: bool,
    pub point_size: Option<i32>,
    pub catalog_ordinal: Option<usize>,
    pub geometries: Vec<NativeHeadScannerGeometry>,
    pub schedule: Vec<NativeHeadScannerSchedule>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadScannerGeometry {
    pub source: NativeHeadScannerSource,
    pub direction: i32,
    pub pitch: i32,
    pub open: bool,
    pub line_count: i32,
    pub interline: i32,
    pub line: NativeHeadScannerLine,
    pub line2: Option<NativeHeadScannerLine>,
    pub y_offsets: Vec<i32>,
    pub all_shapes: Vec<HeadTemplateShape>,
    pub stem_shapes: Vec<HeadTemplateShape>,
    pub hollow_shapes: Vec<HeadTemplateShape>,
    pub farther_ledgers: Vec<LedgerScannerAdapter>,
    pub range_left: i32,
    pub range_right: i32,
}

impl NativeHeadScannerGeometry {
    /// Java `Scanner.getTheoreticalOrdinate(x)` over retained production geometry.
    pub fn theoretical_ordinate(&self, x: i32) -> Result<i32, HeadScannerError> {
        let farther = self
            .farther_ledgers
            .iter()
            .map(|ledger| ledger as &dyn HeadScannerLedger)
            .collect::<Vec<_>>();
        Ok(theoretical_ordinate(HeadTheoreticalOrdinateInput {
            line: &self.line,
            line2: self.line2.as_ref().map(|line| line as &dyn HeadScannerLine),
            farther_ledgers: &farther,
            line_count: self.line_count,
            interline: self.interline,
            pitch: self.pitch,
            x,
        })?
        .y)
    }

    #[must_use]
    pub fn line_range(&self) -> (i32, i32) {
        (self.line.left_abscissa(), self.line.right_abscissa())
    }

    #[must_use]
    pub fn line2_range(&self) -> Option<(i32, i32)> {
        self.line2
            .as_ref()
            .map(|line| (line.left_abscissa(), line.right_abscissa()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHeadScannerLine {
    Staff(StaffLineScannerAdapter),
    Ledger(LedgerScannerAdapter),
}

impl NativeHeadScannerLine {
    #[must_use]
    pub fn left_abscissa(&self) -> i32 {
        match self {
            Self::Staff(line) => line.left_abscissa(),
            Self::Ledger(line) => line.left_abscissa(),
        }
    }

    #[must_use]
    pub fn right_abscissa(&self) -> i32 {
        match self {
            Self::Staff(line) => line.right_abscissa(),
            Self::Ledger(line) => line.right_abscissa(),
        }
    }
}

impl HeadScannerLine for NativeHeadScannerLine {
    fn y_at(&self, x: f64) -> f64 {
        match self {
            Self::Staff(line) => line.y_at(x),
            Self::Ledger(line) => line.y_at(x),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHeadScannerSource {
    StaffLine {
        line_index: usize,
        axis: HeadScannerAxis,
    },
    Ledger {
        ledger_index: i32,
        ordinal: usize,
        inter_id: usize,
        glyph_id: usize,
        filament_id: usize,
        bounds: Bounds,
        weight: usize,
        run_digest: u64,
        axis: HeadScannerAxis,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadScannerPhase {
    Seed,
    Range,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadScannerSchedule {
    pub phase: NativeHeadScannerPhase,
    pub ordinal: usize,
    pub geometry: usize,
}

#[derive(Debug)]
pub enum NativeHeadsScannerRecognitionError {
    SystemTopology,
    HeaderSystemOrder,
    HeaderStaffOrder {
        system_id: usize,
    },
    StemScale(i32),
    MissingStaff {
        system_id: usize,
        staff_id: usize,
    },
    NonPersistentStaffLine {
        system_id: usize,
        staff_id: usize,
        line_index: usize,
    },
    InvalidLineCount {
        system_id: usize,
        staff_id: usize,
        count: usize,
    },
    CoordinateOutOfRange {
        kind: &'static str,
        system_id: usize,
        staff_id: usize,
        ordinal: usize,
    },
    MissingHeader {
        system_id: usize,
        staff_id: usize,
    },
    HeaderInterline {
        system_id: usize,
        staff_id: usize,
        expected: i32,
        actual: i32,
    },
    MissingPart {
        system_id: usize,
        staff_id: usize,
    },
    DuplicatePart {
        system_id: usize,
        staff_id: usize,
    },
    UnsupportedDrumStaff {
        system_id: usize,
        staff_id: usize,
    },
    SelectedClefIdentity {
        system_id: usize,
        staff_id: usize,
        clef_id: usize,
    },
    CatalogSelectionOrder,
    MissingCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    UnexpectedCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    CatalogInterline {
        system_id: usize,
        staff_id: usize,
        expected: i32,
        actual: i32,
    },
    LedgerCount {
        inters: usize,
        glyphs: usize,
    },
    LedgerIdentity {
        ordinal: usize,
    },
    LedgerCoordinate {
        ordinal: usize,
    },
    LedgerComponents {
        ordinal: usize,
        count: usize,
    },
    LedgerAxis {
        ordinal: usize,
    },
    Scanner(HeadScannerError),
    Geometry(HeadScannerGeometryError),
    ArithmeticOverflow(&'static str),
}

impl fmt::Display for NativeHeadsScannerRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid production HEADS scanner context: {self:?}"
        )
    }
}

impl Error for NativeHeadsScannerRecognitionError {}

impl From<HeadScannerError> for NativeHeadsScannerRecognitionError {
    fn from(error: HeadScannerError) -> Self {
        Self::Scanner(error)
    }
}

impl From<HeadScannerGeometryError> for NativeHeadsScannerRecognitionError {
    fn from(error: HeadScannerGeometryError) -> Self {
        Self::Geometry(error)
    }
}

#[derive(Clone, Debug)]
struct LedgerRecord {
    ledger_index: i32,
    inter_id: usize,
    glyph_id: usize,
    filament_id: usize,
    bounds: Bounds,
    weight: usize,
    run_digest: u64,
    adapter: LedgerScannerAdapter,
}

/// Compose the exact production state consumed by Java scanner construction.
pub fn recognize_native_heads_scanner_context(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    ledgers: &NativeLedgerRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    heads: &NativeHeadsPrologRecognition,
) -> Result<NativeHeadsScannerRecognition, NativeHeadsScannerRecognitionError> {
    recognize_native_heads_scanner_context_with_small_heads(
        grid, headers, ledgers, stem_seeds, heads, false,
    )
}

/// Compose scanner context with Java's `smallHeads` processing switch.
pub fn recognize_native_heads_scanner_context_with_small_heads(
    grid: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    ledgers: &NativeLedgerRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    heads: &NativeHeadsPrologRecognition,
    small_heads: bool,
) -> Result<NativeHeadsScannerRecognition, NativeHeadsScannerRecognitionError> {
    let system_count = grid.peak_graph.systems.len();
    if grid.peak_graph.sig.systems.len() != system_count {
        return Err(NativeHeadsScannerRecognitionError::SystemTopology);
    }
    if headers
        .systems
        .iter()
        .map(|system| system.system_id)
        .ne(1..=system_count)
    {
        return Err(NativeHeadsScannerRecognitionError::HeaderSystemOrder);
    }
    if stem_seeds.maximum_stem_thickness <= 0 {
        return Err(NativeHeadsScannerRecognitionError::StemScale(
            stem_seeds.maximum_stem_thickness,
        ));
    }
    let parameters = HeadScannerParameters::derive(
        grid.scale.scale.interline.main,
        stem_seeds.maximum_stem_thickness,
    )?;

    let mut catalog_selections = catalog_selections(heads)?;
    let ledger_records = ledger_records(ledgers)?;
    let mut systems = Vec::with_capacity(system_count);

    for (system_index, staff_ids) in grid.peak_graph.systems.iter().enumerate() {
        let system_id = system_index + 1;
        let topology = &grid.peak_graph.sig.systems[system_index];
        if topology.system_id != system_id
            || topology.staff_ids != *staff_ids
            || topology.staff_limits.len() != staff_ids.len()
        {
            return Err(NativeHeadsScannerRecognitionError::SystemTopology);
        }
        let header_system = &headers.systems[system_index];
        if header_system
            .staffs
            .iter()
            .map(|staff| staff.staff_id)
            .ne(staff_ids.iter().copied())
        {
            return Err(NativeHeadsScannerRecognitionError::HeaderStaffOrder { system_id });
        }

        let mut staff_lines = BTreeMap::<usize, Vec<StaffLineScannerAdapter>>::new();
        let mut staffs_by_id = BTreeMap::<usize, &HeadlessStaff>::new();
        let mut staff_limits = BTreeMap::<usize, (i32, i32)>::new();
        for (&staff_id, &limits) in staff_ids.iter().zip(&topology.staff_limits) {
            let staff = grid
                .peak_graph
                .sheet_staffs
                .iter()
                .find(|staff| staff.id == staff_id)
                .ok_or(NativeHeadsScannerRecognitionError::MissingStaff {
                    system_id,
                    staff_id,
                })?;
            let mut lines = Vec::with_capacity(staff.lines.len());
            for (line_index, line) in staff.lines.iter().enumerate() {
                let HeadlessStaffLine::Persistent { line, .. } = line else {
                    return Err(NativeHeadsScannerRecognitionError::NonPersistentStaffLine {
                        system_id,
                        staff_id,
                        line_index,
                    });
                };
                lines.push(StaffLineScannerAdapter::from_persistent(line)?);
            }
            if lines.is_empty() {
                return Err(NativeHeadsScannerRecognitionError::InvalidLineCount {
                    system_id,
                    staff_id,
                    count: 0,
                });
            }
            staffs_by_id.insert(staff_id, staff);
            staff_limits.insert(staff_id, limits);
            staff_lines.insert(staff_id, lines);
        }

        let mut staffs = Vec::with_capacity(staff_ids.len());
        for (&staff_id, header_staff) in staff_ids.iter().zip(&header_system.staffs) {
            let staff = staffs_by_id[&staff_id];
            let lines = &staff_lines[&staff_id];
            let specific_interline = i32::try_from(staff.interline).map_err(|_| {
                NativeHeadsScannerRecognitionError::CoordinateOutOfRange {
                    kind: "staff interline",
                    system_id,
                    staff_id,
                    ordinal: 0,
                }
            })?;
            if header_staff.specific_interline != specific_interline {
                return Err(NativeHeadsScannerRecognitionError::HeaderInterline {
                    system_id,
                    staff_id,
                    expected: specific_interline,
                    actual: header_staff.specific_interline,
                });
            }
            let tablature = staff.kind == StaffCandidateKind::Tablature;
            let header_stop = if tablature {
                header_staff.header.as_ref().map_or(0, |header| header.stop)
            } else {
                header_staff
                    .header
                    .as_ref()
                    .ok_or(NativeHeadsScannerRecognitionError::MissingHeader {
                        system_id,
                        staff_id,
                    })?
                    .stop
            };
            let (part, merged) = if tablature {
                (None, false)
            } else {
                let part = staff_part(system_id, staff_id, &topology.bar_tail.parts)?;
                let merged = merged_part(
                    topology.staff_ids.len(),
                    part,
                    &staff_lines,
                    &staff_limits,
                    minimum_separate_staff_gutter(parameters.main_interline),
                )?;
                (Some(part), merged)
            };
            let drum = !tablature && is_drum_staff(system_id, staff, header_staff)?;
            if drum {
                return Err(NativeHeadsScannerRecognitionError::UnsupportedDrumStaff {
                    system_id,
                    staff_id,
                });
            }

            let selection = catalog_selections.remove(&(system_id, staff_id));
            if tablature && selection.is_some() {
                return Err(
                    NativeHeadsScannerRecognitionError::UnexpectedCatalogSelection {
                        system_id,
                        staff_id,
                    },
                );
            }
            let selection = if tablature {
                None
            } else {
                let selection = selection.ok_or(
                    NativeHeadsScannerRecognitionError::MissingCatalogSelection {
                        system_id,
                        staff_id,
                    },
                )?;
                if selection.specific_interline != specific_interline {
                    return Err(NativeHeadsScannerRecognitionError::CatalogInterline {
                        system_id,
                        staff_id,
                        expected: specific_interline,
                        actual: selection.specific_interline,
                    });
                }
                Some(selection)
            };

            let mut geometries = if tablature {
                Vec::new()
            } else {
                let staff_ledgers =
                    staff_ledger_records(ledgers, system_id, staff_id, &ledger_records)?;
                build_staff_geometries(
                    system_id,
                    staff_id,
                    lines,
                    &staff_ledgers,
                    specific_interline,
                    header_stop,
                    merged,
                    part.expect("non-tablature staff resolved its part"),
                    &parameters,
                )?
            };
            if small_heads {
                for geometry in &mut geometries {
                    geometry.all_shapes = SMALL_HEAD_SCANNER_SHAPES.all.to_vec();
                    geometry.stem_shapes = SMALL_HEAD_SCANNER_SHAPES.stem.to_vec();
                    geometry.hollow_shapes = SMALL_HEAD_SCANNER_SHAPES.hollow.to_vec();
                }
            }
            geometries.shrink_to_fit();
            let schedule = scanner_schedule(geometries.len());
            staffs.push(NativeHeadsScannerStaff {
                system_id,
                staff_id,
                tablature,
                drum,
                line_count: lines.len(),
                specific_interline,
                header_stop,
                merged,
                point_size: selection.map(|selection| selection.point_size),
                catalog_ordinal: selection.map(|selection| selection.catalog_ordinal),
                geometries,
                schedule,
            });
        }
        systems.push(NativeHeadsScannerSystem {
            system_id,
            parameters: parameters.clone(),
            staffs,
        });
    }

    if let Some((&(system_id, staff_id), _)) = catalog_selections.first_key_value() {
        return Err(
            NativeHeadsScannerRecognitionError::UnexpectedCatalogSelection {
                system_id,
                staff_id,
            },
        );
    }

    Ok(NativeHeadsScannerRecognition { systems })
}

fn catalog_selections(
    heads: &NativeHeadsPrologRecognition,
) -> Result<
    BTreeMap<(usize, usize), NativeHeadTemplateCatalogSelection>,
    NativeHeadsScannerRecognitionError,
> {
    let mut selections = BTreeMap::new();
    let mut prior = None;
    for &selection in &heads.staff_template_catalogs {
        let owner = (selection.system_id, selection.staff_id);
        if prior.is_some_and(|prior| prior >= owner)
            || selections.insert(owner, selection).is_some()
        {
            return Err(NativeHeadsScannerRecognitionError::CatalogSelectionOrder);
        }
        prior = Some(owner);
    }
    Ok(selections)
}

fn ledger_records(
    ledgers: &NativeLedgerRecognition,
) -> Result<BTreeMap<usize, LedgerRecord>, NativeHeadsScannerRecognitionError> {
    let live = ledgers.ledgers();
    if live.len() != ledgers.ledger_glyphs.len() {
        return Err(NativeHeadsScannerRecognitionError::LedgerCount {
            inters: live.len(),
            glyphs: ledgers.ledger_glyphs.len(),
        });
    }
    let mut records = BTreeMap::<usize, LedgerRecord>::new();
    for (ordinal, (inter, glyph)) in live.iter().zip(&ledgers.ledger_glyphs).enumerate() {
        validate_ledger_identity(ordinal, inter, glyph)?;
        let left = i32::try_from(glyph.bounds.x)
            .map_err(|_| NativeHeadsScannerRecognitionError::LedgerCoordinate { ordinal })?;
        let top = i32::try_from(glyph.bounds.y)
            .map_err(|_| NativeHeadsScannerRecognitionError::LedgerCoordinate { ordinal })?;
        let components = build_glyph_components(&glyph.run_table, left, top);
        if components.len() != 1 {
            return Err(NativeHeadsScannerRecognitionError::LedgerComponents {
                ordinal,
                count: components.len(),
            });
        }
        let axis = pixel_line(&components[0])
            .ok_or(NativeHeadsScannerRecognitionError::LedgerAxis { ordinal })?;
        let adapter = LedgerScannerAdapter::from_segment(axis)?;
        if records
            .insert(
                inter.id,
                LedgerRecord {
                    ledger_index: inter.ledger_index,
                    inter_id: inter.id,
                    glyph_id: inter.glyph_id,
                    filament_id: inter.filament_id,
                    bounds: glyph.bounds,
                    weight: glyph.run_table.weight(),
                    run_digest: glyph.run_digest(),
                    adapter,
                },
            )
            .is_some()
        {
            return Err(NativeHeadsScannerRecognitionError::LedgerIdentity { ordinal });
        }
    }
    Ok(records)
}

fn staff_ledger_records(
    ledgers: &NativeLedgerRecognition,
    system_id: usize,
    staff_id: usize,
    records: &BTreeMap<usize, LedgerRecord>,
) -> Result<BTreeMap<i32, Vec<LedgerRecord>>, NativeHeadsScannerRecognitionError> {
    let mut levels = BTreeMap::new();
    for index in ledgers
        .materializer
        .staff_ledger_indexes(system_id, staff_id)
    {
        let mut level = Vec::new();
        for &inter_id in ledgers
            .materializer
            .staff_inter_ids(system_id, staff_id, index)
        {
            let mut record = records
                .get(&inter_id)
                .cloned()
                .ok_or(NativeHeadsScannerRecognitionError::LedgerIdentity { ordinal: inter_id })?;
            let inter = ledgers
                .materializer
                .inter_by_id(inter_id)
                .ok_or(NativeHeadsScannerRecognitionError::LedgerIdentity { ordinal: inter_id })?;
            if inter.removed || inter.system_id != system_id {
                return Err(NativeHeadsScannerRecognitionError::LedgerIdentity {
                    ordinal: inter_id,
                });
            }
            record.ledger_index = index;
            level.push(record);
        }
        if !level.is_empty() {
            levels.insert(index, level);
        }
    }
    Ok(levels)
}

fn validate_ledger_identity(
    ordinal: usize,
    inter: &crate::raw_ledger_filter::MaterializedLedgerInter,
    glyph: &NativeLedgerGlyph,
) -> Result<(), NativeHeadsScannerRecognitionError> {
    if inter.system_id != glyph.system_id
        || inter.staff_id != glyph.staff_id
        || inter.id != glyph.inter_id
        || inter.glyph_id != glyph.glyph_id
        || inter.filament_id != glyph.filament_id
    {
        return Err(NativeHeadsScannerRecognitionError::LedgerIdentity { ordinal });
    }
    Ok(())
}

fn staff_part(
    system_id: usize,
    staff_id: usize,
    parts: &[PlannedPart],
) -> Result<PlannedPart, NativeHeadsScannerRecognitionError> {
    let staff_id_i32 = i32::try_from(staff_id).map_err(|_| {
        NativeHeadsScannerRecognitionError::CoordinateOutOfRange {
            kind: "staff id",
            system_id,
            staff_id,
            ordinal: 0,
        }
    })?;
    let mut found = parts.iter().copied().enumerate().filter(|(_, part)| {
        part.first_staff_id <= staff_id_i32 && staff_id_i32 <= part.last_staff_id
    });
    let (_, part) = found
        .next()
        .ok_or(NativeHeadsScannerRecognitionError::MissingPart {
            system_id,
            staff_id,
        })?;
    if found.next().is_some() {
        return Err(NativeHeadsScannerRecognitionError::DuplicatePart {
            system_id,
            staff_id,
        });
    }
    Ok(part)
}

fn merged_part(
    system_staff_count: usize,
    part: PlannedPart,
    staff_lines: &BTreeMap<usize, Vec<StaffLineScannerAdapter>>,
    staff_limits: &BTreeMap<usize, (i32, i32)>,
    minimum_gutter: i32,
) -> Result<bool, NativeHeadsScannerRecognitionError> {
    let first_id = usize::try_from(part.first_staff_id)
        .map_err(|_| NativeHeadsScannerRecognitionError::SystemTopology)?;
    let last_id = usize::try_from(part.last_staff_id)
        .map_err(|_| NativeHeadsScannerRecognitionError::SystemTopology)?;
    let first_left = staff_limits
        .get(&first_id)
        .ok_or(NativeHeadsScannerRecognitionError::SystemTopology)?
        .0;
    let last_left = staff_limits
        .get(&last_id)
        .ok_or(NativeHeadsScannerRecognitionError::SystemTopology)?
        .0;
    let mut failed = false;
    let merged = is_merged_two_staff_part(
        system_staff_count,
        part,
        first_left,
        last_left,
        minimum_gutter,
        |staff_id, boundary, x| {
            let Some(lines) = usize::try_from(staff_id)
                .ok()
                .and_then(|staff_id| staff_lines.get(&staff_id))
            else {
                failed = true;
                return 0;
            };
            let line = match boundary {
                StaffBoundaryLine::First => lines.first(),
                StaffBoundaryLine::Last => lines.last(),
            };
            let Some(line) = line else {
                failed = true;
                return 0;
            };
            line.y_at(f64::from(x)).round_ties_even() as i32
        },
    );
    if failed {
        return Err(NativeHeadsScannerRecognitionError::SystemTopology);
    }
    Ok(merged)
}

fn minimum_separate_staff_gutter(main_interline: i32) -> i32 {
    // Java `BarsRetriever.Constants.minSeparateStaffGutter`, independently of
    // HEADS' equal-valued minimum-beam-width constant.
    (f64::from(main_interline) * 2.5).round_ties_even() as i32
}

fn is_drum_staff(
    system_id: usize,
    staff: &HeadlessStaff,
    header: &NativeHeaderStaffRecognition,
) -> Result<bool, NativeHeadsScannerRecognitionError> {
    if staff.lines.len() == 1 {
        return Ok(true);
    }
    let Some(selected) = header.selected_clef_id else {
        return Ok(false);
    };
    let candidate = header
        .clef_candidates
        .iter()
        .find(|candidate| candidate.id == selected)
        .ok_or(NativeHeadsScannerRecognitionError::SelectedClefIdentity {
            system_id,
            staff_id: staff.id,
            clef_id: selected,
        })?;
    Ok(candidate.kind == NeutralClefKind::Percussion)
}

#[allow(clippy::too_many_arguments)]
fn build_staff_geometries(
    system_id: usize,
    staff_id: usize,
    staff_lines: &[StaffLineScannerAdapter],
    ledger_levels: &BTreeMap<i32, Vec<LedgerRecord>>,
    interline: i32,
    header_stop: i32,
    merged: bool,
    part: PlannedPart,
    parameters: &HeadScannerParameters,
) -> Result<Vec<NativeHeadScannerGeometry>, NativeHeadsScannerRecognitionError> {
    let line_count = i32::try_from(staff_lines.len()).map_err(|_| {
        NativeHeadsScannerRecognitionError::InvalidLineCount {
            system_id,
            staff_id,
            count: staff_lines.len(),
        }
    })?;
    if line_count != 5 {
        return Err(NativeHeadsScannerRecognitionError::InvalidLineCount {
            system_id,
            staff_id,
            count: staff_lines.len(),
        });
    }
    let levels = ledger_levels;
    let ledger_count = levels.values().map(Vec::len).sum::<usize>();
    let mut geometries = Vec::with_capacity(
        staff_lines
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_add(ledger_count.checked_mul(2)?))
            .ok_or(NativeHeadsScannerRecognitionError::ArithmeticOverflow(
                "scanner geometry capacity",
            ))?,
    );

    let mut pitch = -line_count;
    let mut previous = None::<StaffLineScannerAdapter>;
    for (line_index, line) in staff_lines.iter().cloned().enumerate() {
        push_geometry(
            &mut geometries,
            NativeHeadScannerSource::StaffLine {
                line_index,
                axis: line.axis(),
            },
            NativeHeadScannerLine::Staff(line.clone()),
            previous.clone().map(NativeHeadScannerLine::Staff),
            -1,
            pitch,
            line_count,
            interline,
            header_stop,
            levels,
            parameters,
        )?;
        pitch =
            pitch
                .checked_add(1)
                .ok_or(NativeHeadsScannerRecognitionError::ArithmeticOverflow(
                    "staff pitch",
                ))?;
        push_geometry(
            &mut geometries,
            NativeHeadScannerSource::StaffLine {
                line_index,
                axis: line.axis(),
            },
            NativeHeadScannerLine::Staff(line.clone()),
            None,
            0,
            pitch,
            line_count,
            interline,
            header_stop,
            levels,
            parameters,
        )?;
        pitch =
            pitch
                .checked_add(1)
                .ok_or(NativeHeadsScannerRecognitionError::ArithmeticOverflow(
                    "staff pitch",
                ))?;
        if pitch == line_count {
            push_geometry(
                &mut geometries,
                NativeHeadScannerSource::StaffLine {
                    line_index,
                    axis: line.axis(),
                },
                NativeHeadScannerLine::Staff(line.clone()),
                None,
                1,
                pitch,
                line_count,
                interline,
                header_stop,
                levels,
                parameters,
            )?;
            pitch = pitch.checked_add(1).ok_or(
                NativeHeadsScannerRecognitionError::ArithmeticOverflow("staff pitch"),
            )?;
        }
        previous = Some(line);
    }

    for direction in [-1_i32, 1_i32] {
        let look_farther = !(merged
            && ((direction > 0 && i32::try_from(staff_id).ok() == Some(part.first_staff_id))
                || (direction < 0 && i32::try_from(staff_id).ok() == Some(part.last_staff_id))));
        pitch = direction.checked_mul(4).ok_or(
            NativeHeadsScannerRecognitionError::ArithmeticOverflow("ledger pitch"),
        )?;
        let mut index = direction;
        while let Some(set) = levels.get(&index).filter(|set| !set.is_empty()) {
            pitch = pitch.checked_add(2 * direction).ok_or(
                NativeHeadsScannerRecognitionError::ArithmeticOverflow("ledger pitch"),
            )?;
            for (ordinal, record) in set.iter().enumerate() {
                let source = ledger_source(record, ordinal);
                push_geometry(
                    &mut geometries,
                    source.clone(),
                    NativeHeadScannerLine::Ledger(record.adapter.clone()),
                    None,
                    0,
                    pitch,
                    line_count,
                    interline,
                    header_stop,
                    levels,
                    parameters,
                )?;
                if look_farther {
                    push_geometry(
                        &mut geometries,
                        source,
                        NativeHeadScannerLine::Ledger(record.adapter.clone()),
                        None,
                        direction,
                        pitch.checked_add(direction).ok_or(
                            NativeHeadsScannerRecognitionError::ArithmeticOverflow(
                                "ledger outward pitch",
                            ),
                        )?,
                        line_count,
                        interline,
                        header_stop,
                        levels,
                        parameters,
                    )?;
                }
            }
            index = index.checked_add(direction).ok_or(
                NativeHeadsScannerRecognitionError::ArithmeticOverflow("ledger index"),
            )?;
        }
    }
    Ok(geometries)
}

fn ledger_source(record: &LedgerRecord, ordinal: usize) -> NativeHeadScannerSource {
    NativeHeadScannerSource::Ledger {
        ledger_index: record.ledger_index,
        ordinal,
        inter_id: record.inter_id,
        glyph_id: record.glyph_id,
        filament_id: record.filament_id,
        bounds: record.bounds,
        weight: record.weight,
        run_digest: record.run_digest,
        axis: record.adapter.axis(),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_geometry(
    geometries: &mut Vec<NativeHeadScannerGeometry>,
    source: NativeHeadScannerSource,
    line: NativeHeadScannerLine,
    line2: Option<NativeHeadScannerLine>,
    direction: i32,
    pitch: i32,
    line_count: i32,
    interline: i32,
    header_stop: i32,
    levels: &BTreeMap<i32, Vec<LedgerRecord>>,
    parameters: &HeadScannerParameters,
) -> Result<(), NativeHeadsScannerRecognitionError> {
    let open = is_open_scanner(pitch, line_count, line2.is_some())?;
    let y_offsets = compute_y_offsets(
        open,
        direction,
        parameters.max_open_dy,
        parameters.max_closed_dy,
    )?;
    let range_left = line.left_abscissa().max(header_stop);
    let range_right = line
        .right_abscissa()
        .checked_sub(parameters.min_template_width)
        .ok_or(NativeHeadsScannerRecognitionError::ArithmeticOverflow(
            "scanner range right",
        ))?;
    geometries.push(NativeHeadScannerGeometry {
        source,
        direction,
        pitch,
        open,
        line_count,
        interline,
        line,
        line2,
        y_offsets,
        all_shapes: NORMAL_HEAD_SCANNER_SHAPES.all.to_vec(),
        stem_shapes: NORMAL_HEAD_SCANNER_SHAPES.stem.to_vec(),
        hollow_shapes: NORMAL_HEAD_SCANNER_SHAPES.hollow.to_vec(),
        farther_ledgers: farther_ledgers(levels, pitch)?,
        range_left,
        range_right,
    });
    Ok(())
}

fn farther_ledgers(
    levels: &BTreeMap<i32, Vec<LedgerRecord>>,
    pitch: i32,
) -> Result<Vec<LedgerScannerAdapter>, NativeHeadsScannerRecognitionError> {
    let magnitude =
        pitch
            .checked_abs()
            .ok_or(NativeHeadsScannerRecognitionError::ArithmeticOverflow(
                "pitch magnitude",
            ))?;
    if pitch % 2 == 0 || magnitude <= 4 {
        return Ok(Vec::new());
    }
    let direction = pitch.signum();
    let target_pitch = pitch.checked_add(direction).ok_or(
        NativeHeadsScannerRecognitionError::ArithmeticOverflow("farther ledger pitch"),
    )?;
    let mut ledger_pitch = direction * 4;
    let mut index = direction;
    while let Some(set) = levels.get(&index).filter(|set| !set.is_empty()) {
        ledger_pitch = ledger_pitch.checked_add(2 * direction).ok_or(
            NativeHeadsScannerRecognitionError::ArithmeticOverflow("farther ledger pitch"),
        )?;
        if ledger_pitch == target_pitch {
            return Ok(set.iter().map(|record| record.adapter.clone()).collect());
        }
        index = index.checked_add(direction).ok_or(
            NativeHeadsScannerRecognitionError::ArithmeticOverflow("farther ledger index"),
        )?;
    }
    Ok(Vec::new())
}

fn scanner_schedule(geometry_count: usize) -> Vec<NativeHeadScannerSchedule> {
    let mut schedule = Vec::with_capacity(geometry_count.saturating_mul(2));
    for phase in [NativeHeadScannerPhase::Seed, NativeHeadScannerPhase::Range] {
        schedule.extend(
            (0..geometry_count).map(|geometry| NativeHeadScannerSchedule {
                phase,
                ordinal: geometry,
                geometry,
            }),
        );
    }
    schedule
}
