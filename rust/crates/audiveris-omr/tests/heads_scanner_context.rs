// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;

use audiveris_omr::{
    head_scanner_geometry::HeadScannerAxis,
    head_template::HeadTemplateShape,
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_scanner::{
        NativeHeadScannerPhase, NativeHeadScannerSource, recognize_native_heads_scanner_context,
    },
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-scanner-context.txt");
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug)]
struct ExpectedOracle {
    pages: Vec<ExpectedPage>,
}

#[derive(Clone, Debug)]
struct ExpectedPage {
    key: String,
    width: usize,
    height: usize,
    declared_systems: usize,
    declared_staves: usize,
    family: String,
    systems: Vec<ExpectedSystem>,
    summary: Option<PageSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct ExpectedSystem {
    id: usize,
    params: ScannerParams,
    staves: Vec<ExpectedStaff>,
    summary: Option<SystemSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct ScannerParams {
    main_interline: i32,
    max_stem: usize,
    max_distance_low: OracleDouble,
    really_bad_distance: OracleDouble,
    max_template_dx: i32,
    max_closed_dy: i32,
    max_open_dy: i32,
    min_beam_width: i32,
    v_bar_margin: OracleDouble,
    min_template_width: i32,
    template_half: i32,
    x_offsets: Vec<i32>,
}

#[derive(Clone, Debug)]
struct ExpectedStaff {
    id: usize,
    tablature: bool,
    drum: bool,
    line_count: usize,
    interline: i32,
    header_stop: i32,
    part_id: Option<usize>,
    merged: bool,
    part_first: Option<usize>,
    part_last: Option<usize>,
    point_size: i32,
    catalog_family: String,
    catalog_point_size: i32,
    geometries: Vec<ExpectedGeometry>,
    schedule: Vec<ExpectedSchedule>,
    summary: Option<StaffSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct ExpectedGeometry {
    ordinal: usize,
    source: GeometrySource,
    direction: i32,
    pitch: i32,
    open: bool,
    interline: i32,
    line: InclusiveRange,
    line2: Option<InclusiveRange>,
    y_offsets: Vec<i32>,
    all_shapes: Vec<HeadShape>,
    stem_shapes: Vec<HeadShape>,
    hollow_shapes: Vec<HeadShape>,
    farther_axes: Vec<Axis>,
    ordinate: Vec<RleSpan>,
    range: InclusiveRange,
    range_ordinate: Vec<RleSpan>,
}

#[derive(Clone, Debug)]
enum GeometrySource {
    StaffLine {
        line_index: usize,
        axis: Axis,
    },
    Ledger {
        ledger_index: i32,
        ordinal: usize,
        bounds: Bounds,
        weight: usize,
        run_hash: u64,
        axis: Axis,
    },
}

#[derive(Clone, Debug)]
struct Bounds {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
}

#[derive(Clone, Debug)]
struct Axis {
    left: OraclePoint,
    right: OraclePoint,
}

#[derive(Clone, Debug)]
struct OraclePoint {
    x: OracleDouble,
    y: OracleDouble,
}

#[derive(Clone, Debug)]
struct OracleDouble {
    java_hex: String,
    bits: u64,
    value: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeadShape {
    Breve,
    WholeNote,
    NoteheadVoid,
    NoteheadBlack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScannerPhase {
    Seed,
    Range,
}

#[derive(Clone, Debug)]
struct ExpectedSchedule {
    phase: ScannerPhase,
    ordinal: usize,
    geometry: usize,
}

#[derive(Clone, Copy, Debug)]
struct InclusiveRange {
    left: i32,
    right: i32,
}

#[derive(Clone, Copy, Debug)]
struct RleSpan {
    ordinate: i32,
    length: usize,
}

#[derive(Clone, Copy, Debug)]
struct StaffSummary {
    geometries: usize,
    schedules: usize,
    hash: u64,
}

#[derive(Clone, Copy, Debug)]
struct SystemSummary {
    staves: usize,
    geometries: usize,
    schedules: usize,
    hash: u64,
}

#[derive(Clone, Copy, Debug)]
struct PageSummary {
    standard_staves: usize,
    geometries: usize,
    schedules: usize,
    hash: u64,
}

#[derive(Clone, Copy, Debug)]
struct Fnv1a64(u64);

impl Fnv1a64 {
    fn new() -> Self {
        Self(FNV_OFFSET)
    }

    fn add_line(&mut self, line: &str) {
        for byte in line.bytes().chain(std::iter::once(b'\n')) {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
}

impl ExpectedOracle {
    fn parse(text: &str) -> Self {
        let mut oracle = Self { pages: Vec::new() };

        for (line_index, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = Fields::new(line, line_index + 1);
            match fields.take() {
                "headscannerpage" => parse_page(&mut oracle, &mut fields),
                "headscannerparams" => parse_params(&mut oracle, &mut fields, line),
                "headscannerstaff" => parse_staff(&mut oracle, &mut fields, line),
                "headscannergeometry" => parse_geometry(&mut oracle, &mut fields, line),
                "headscannerschedule" => parse_schedule(&mut oracle, &mut fields, line),
                "headscannerstaffsummary" => parse_staff_summary(&mut oracle, &mut fields, line),
                "headscannersystemsummary" => parse_system_summary(&mut oracle, &mut fields, line),
                "headscannerpagesummary" => parse_page_summary(&mut oracle, &mut fields),
                kind => panic!("unexpected row kind {kind} on line {}", fields.line_number),
            }
            fields.finish();
        }

        oracle.validate();
        oracle
    }

    fn validate(&self) {
        assert!(!self.pages.is_empty(), "scanner oracle contains pages");
        let mut page_keys = BTreeSet::new();

        for page in &self.pages {
            assert!(page_keys.insert(&page.key), "duplicate page {}", page.key);
            assert!(page.width > 0 && page.height > 0, "{} dimensions", page.key);
            assert!(!page.family.is_empty(), "{} music family", page.key);
            assert_eq!(
                page.systems.len(),
                page.declared_systems,
                "{} systems",
                page.key
            );
            let staff_count = page
                .systems
                .iter()
                .map(|system| system.staves.len())
                .sum::<usize>();
            assert_eq!(staff_count, page.declared_staves, "{} staves", page.key);

            for (system_index, system) in page.systems.iter().enumerate() {
                assert_eq!(system.id, system_index + 1, "{} system order", page.key);
                system.params.validate(&page.key, system.id);
                for staff in &system.staves {
                    staff.validate(page, system);
                }

                let summary = system
                    .summary
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} system {} lacks summary", page.key, system.id));
                let geometries = system
                    .staves
                    .iter()
                    .map(|staff| staff.geometries.len())
                    .sum::<usize>();
                let schedules = system
                    .staves
                    .iter()
                    .map(|staff| staff.schedule.len())
                    .sum::<usize>();
                assert_eq!(
                    summary.staves,
                    system.staves.len(),
                    "{} system {} staves",
                    page.key,
                    system.id
                );
                assert_eq!(
                    summary.geometries, geometries,
                    "{} system {} geometries",
                    page.key, system.id
                );
                assert_eq!(
                    summary.schedules, schedules,
                    "{} system {} schedules",
                    page.key, system.id
                );
                assert_eq!(
                    summary.hash, system.hash.0,
                    "{} system {} hash",
                    page.key, system.id
                );
            }

            let summary = page
                .summary
                .as_ref()
                .unwrap_or_else(|| panic!("{} lacks page summary", page.key));
            let standard_staves = page
                .systems
                .iter()
                .flat_map(|system| &system.staves)
                .filter(|staff| !staff.tablature)
                .count();
            let geometries = page
                .systems
                .iter()
                .flat_map(|system| &system.staves)
                .map(|staff| staff.geometries.len())
                .sum::<usize>();
            let schedules = page
                .systems
                .iter()
                .flat_map(|system| &system.staves)
                .map(|staff| staff.schedule.len())
                .sum::<usize>();
            assert_eq!(
                summary.standard_staves, standard_staves,
                "{} standard staves",
                page.key
            );
            assert_eq!(summary.geometries, geometries, "{} geometries", page.key);
            assert_eq!(summary.schedules, schedules, "{} schedules", page.key);
            assert_eq!(summary.hash, page.hash.0, "{} page hash", page.key);
        }
    }
}

impl ScannerParams {
    fn validate(&self, page: &str, system: usize) {
        assert!(self.main_interline > 0, "{page} system {system} interline");
        assert!(self.max_stem > 0, "{page} system {system} max stem");
        assert!(
            self.max_distance_low.value >= 0.0,
            "{page} system {system} low distance"
        );
        assert!(
            self.really_bad_distance.value >= self.max_distance_low.value,
            "{page} system {system} bad distance"
        );
        assert!(
            self.max_template_dx >= 0,
            "{page} system {system} template dx"
        );
        assert!(self.max_closed_dy >= 0, "{page} system {system} closed dy");
        assert!(
            self.max_open_dy >= self.max_closed_dy,
            "{page} system {system} open dy"
        );
        assert!(self.min_beam_width > 0, "{page} system {system} beam width");
        assert!(
            self.v_bar_margin.value >= 0.0,
            "{page} system {system} bar margin"
        );
        assert!(
            self.min_template_width > 0,
            "{page} system {system} template width"
        );
        assert!(
            self.template_half > 0,
            "{page} system {system} template half"
        );
        assert_eq!(
            self.x_offsets.len(),
            self.max_stem + usize::from(self.max_stem & 1 == 0),
            "{page} system {system} x offsets"
        );
        for (index, offset) in self.x_offsets.iter().copied().enumerate() {
            let magnitude = i32::try_from(index.div_ceil(2)).expect("offset index fits i32");
            let expected = if index == 0 {
                0
            } else if index % 2 == 1 {
                magnitude
            } else {
                -magnitude
            };
            assert_eq!(offset, expected, "{page} system {system} x offset {index}");
        }
        self.max_distance_low.validate();
        self.really_bad_distance.validate();
        self.v_bar_margin.validate();
    }
}

impl ExpectedStaff {
    fn validate(&self, page: &ExpectedPage, system: &ExpectedSystem) {
        assert!(self.id > 0, "{} system {} staff id", page.key, system.id);
        assert!(self.line_count > 0, "{} staff {} lines", page.key, self.id);
        assert!(
            self.interline > 0,
            "{} staff {} interline",
            page.key,
            self.id
        );
        assert!(
            self.header_stop >= 0,
            "{} staff {} header stop",
            page.key,
            self.id
        );
        assert_eq!(
            self.part_id.is_some(),
            self.part_first.is_some(),
            "{} staff {} part first",
            page.key,
            self.id
        );
        assert_eq!(
            self.part_id.is_some(),
            self.part_last.is_some(),
            "{} staff {} part last",
            page.key,
            self.id
        );
        if let (Some(first), Some(last)) = (self.part_first, self.part_last) {
            assert!(
                first <= self.id && self.id <= last,
                "{} staff {} part range",
                page.key,
                self.id
            );
            assert!(first <= last, "{} staff {} part order", page.key, self.id);
        }
        assert!(
            self.point_size > 0,
            "{} staff {} point size",
            page.key,
            self.id
        );
        assert_eq!(
            self.catalog_family, page.family,
            "{} staff {} catalog family",
            page.key, self.id
        );
        assert_eq!(
            self.catalog_point_size, self.point_size,
            "{} staff {} catalog size",
            page.key, self.id
        );
        let _ = (self.drum, self.merged);

        if self.tablature {
            assert!(
                self.geometries.is_empty(),
                "{} tablature staff {} geometries",
                page.key,
                self.id
            );
            assert!(
                self.schedule.is_empty(),
                "{} tablature staff {} schedule",
                page.key,
                self.id
            );
            assert!(
                self.summary.is_none(),
                "{} tablature staff {} summary",
                page.key,
                self.id
            );
            return;
        }

        for (ordinal, geometry) in self.geometries.iter().enumerate() {
            assert_eq!(
                geometry.ordinal, ordinal,
                "{} staff {} geometry order",
                page.key, self.id
            );
            geometry.validate(page, system, self);
        }
        let expected_schedule_len = self.geometries.len() * 2;
        assert_eq!(
            self.schedule.len(),
            expected_schedule_len,
            "{} staff {} schedule count",
            page.key,
            self.id
        );
        for (index, schedule) in self.schedule.iter().enumerate() {
            let geometry = index % self.geometries.len();
            let phase = if index < self.geometries.len() {
                ScannerPhase::Seed
            } else {
                ScannerPhase::Range
            };
            assert_eq!(
                schedule.phase, phase,
                "{} staff {} schedule phase",
                page.key, self.id
            );
            assert_eq!(
                schedule.ordinal, geometry,
                "{} staff {} schedule ordinal",
                page.key, self.id
            );
            assert_eq!(
                schedule.geometry, geometry,
                "{} staff {} schedule geometry",
                page.key, self.id
            );
        }

        let summary = self
            .summary
            .as_ref()
            .unwrap_or_else(|| panic!("{} staff {} lacks summary", page.key, self.id));
        assert_eq!(
            summary.geometries,
            self.geometries.len(),
            "{} staff {} geometry summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.schedules,
            self.schedule.len(),
            "{} staff {} schedule summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.hash, self.hash.0,
            "{} staff {} hash",
            page.key, self.id
        );
    }
}

impl ExpectedGeometry {
    fn validate(&self, page: &ExpectedPage, system: &ExpectedSystem, staff: &ExpectedStaff) {
        let label = format!(
            "{} system {} staff {} geometry {}",
            page.key, system.id, staff.id, self.ordinal
        );
        assert!((-1..=1).contains(&self.direction), "{label} direction");
        assert_eq!(self.interline, staff.interline, "{label} interline");
        assert!(
            !self.y_offsets.is_empty() && self.y_offsets[0] == 0,
            "{label} y offsets"
        );
        assert_eq!(self.open, self.y_offsets.len() > 2, "{label} open offsets");
        assert_eq!(
            self.all_shapes,
            [
                HeadShape::Breve,
                HeadShape::WholeNote,
                HeadShape::NoteheadVoid,
                HeadShape::NoteheadBlack
            ],
            "{label} all shapes"
        );
        assert_eq!(
            self.stem_shapes,
            [HeadShape::NoteheadVoid, HeadShape::NoteheadBlack],
            "{label} stem shapes"
        );
        assert_eq!(
            self.hollow_shapes,
            [
                HeadShape::Breve,
                HeadShape::WholeNote,
                HeadShape::NoteheadVoid
            ],
            "{label} hollow shapes"
        );
        assert!(self.line.left <= self.line.right, "{label} line range");
        if self.range.is_empty() {
            assert!(
                self.range.left > self.range.right,
                "{label} empty scanner range"
            );
        } else {
            assert!(
                self.range.left >= self.line.left && self.range.right <= self.line.right,
                "{label} scanner range"
            );
        }
        validate_rle(
            &self.ordinate,
            self.line.len(),
            &format!("{label} ordinate"),
        );
        validate_rle(
            &self.range_ordinate,
            self.range.len(),
            &format!("{label} range ordinate"),
        );
        if let Some(line2) = self.line2 {
            assert!(line2.left <= line2.right, "{label} line2");
        }
        for axis in &self.farther_axes {
            axis.validate();
        }

        match &self.source {
            GeometrySource::StaffLine { line_index, axis } => {
                assert!(*line_index < staff.line_count, "{label} staff-line index");
                assert_eq!(
                    axis.left.x.value,
                    f64::from(self.line.left),
                    "{label} line left"
                );
                assert_eq!(
                    axis.right.x.value,
                    f64::from(self.line.right),
                    "{label} line right"
                );
                axis.validate();
            }
            GeometrySource::Ledger {
                ledger_index,
                ordinal,
                bounds,
                weight,
                run_hash,
                axis,
            } => {
                assert_ne!(*ledger_index, 0, "{label} ledger index");
                let _ = ordinal;
                assert!(
                    bounds.width > 0 && bounds.height > 0,
                    "{label} ledger bounds"
                );
                assert!(bounds.top >= 0 && *weight > 0, "{label} ledger metrics");
                assert_ne!(*run_hash, 0, "{label} ledger run hash");
                assert_eq!(bounds.left, self.line.left, "{label} ledger left");
                assert_eq!(
                    bounds.left + i32::try_from(bounds.width).unwrap() - 1,
                    self.line.right,
                    "{label} ledger right"
                );
                axis.validate();
            }
        }
        let _ = self.pitch;
    }
}

impl InclusiveRange {
    fn len(self) -> usize {
        if self.is_empty() {
            0
        } else {
            usize::try_from(self.right - self.left + 1)
                .expect("inclusive oracle range length fits usize")
        }
    }

    fn is_empty(self) -> bool {
        self.right < self.left
    }
}

impl Axis {
    fn validate(&self) {
        self.left.x.validate();
        self.left.y.validate();
        self.right.x.validate();
        self.right.y.validate();
        assert!(
            self.left.x.value <= self.right.x.value,
            "axis endpoint order"
        );
    }
}

impl OracleDouble {
    fn validate(&self) {
        assert!(self.java_hex.starts_with("0x") || self.java_hex.starts_with("-0x"));
        assert_eq!(
            self.value.to_bits(),
            self.bits,
            "{} double bits",
            self.java_hex
        );
    }
}

fn validate_rle(spans: &[RleSpan], expected_len: usize, label: &str) {
    assert_eq!(spans.is_empty(), expected_len == 0, "{label} emptiness");
    assert_eq!(
        spans.iter().map(|span| span.length).sum::<usize>(),
        expected_len,
        "{label} length"
    );
    for span in spans {
        assert!(span.length > 0, "{label} zero run");
    }
    for pair in spans.windows(2) {
        assert_ne!(
            pair[0].ordinate, pair[1].ordinate,
            "{label} is not canonical RLE"
        );
    }
}

fn parse_page(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>) {
    if let Some(previous) = oracle.pages.last() {
        assert!(
            previous.summary.is_some(),
            "{} missing summary before next page",
            previous.key
        );
    }
    let key = fields.string();
    fields.keyword("width");
    let width = fields.number();
    fields.keyword("height");
    let height = fields.number();
    fields.keyword("systems");
    let declared_systems = fields.number();
    fields.keyword("staves");
    let declared_staves = fields.number();
    fields.keyword("family");
    let family = fields.string();
    oracle.pages.push(ExpectedPage {
        key,
        width,
        height,
        declared_systems,
        declared_staves,
        family,
        systems: Vec::new(),
        summary: None,
        hash: Fnv1a64::new(),
    });
}

fn parse_params(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let page_key = fields.string();
    fields.keyword("system");
    let system_id = fields.number();
    fields.keyword("mainInterline");
    let main_interline = fields.number();
    fields.keyword("maxStem");
    let max_stem = fields.number();
    fields.keyword("maxDistanceLow");
    let max_distance_low = fields.double();
    fields.keyword("reallyBadDistance");
    let really_bad_distance = fields.double();
    fields.keyword("maxTemplateDx");
    let max_template_dx = fields.number();
    fields.keyword("maxClosedDy");
    let max_closed_dy = fields.number();
    fields.keyword("maxOpenDy");
    let max_open_dy = fields.number();
    fields.keyword("minBeamWidth");
    let min_beam_width = fields.number();
    fields.keyword("vBarMargin");
    let v_bar_margin = fields.double();
    fields.keyword("minTemplateWidth");
    let min_template_width = fields.number();
    fields.keyword("templateHalf");
    let template_half = fields.number();
    fields.keyword("xOffsets");
    let x_offsets = fields.csv_numbers();

    let page = current_page(oracle, &page_key);
    if let Some(previous) = page.systems.last() {
        assert!(
            previous.summary.is_some(),
            "{} system {} missing summary",
            page.key,
            previous.id
        );
    }
    assert_eq!(
        system_id,
        page.systems.len() + 1,
        "{} system order",
        page.key
    );
    page.hash.add_line(line);
    page.systems.push(ExpectedSystem {
        id: system_id,
        params: ScannerParams {
            main_interline,
            max_stem,
            max_distance_low,
            really_bad_distance,
            max_template_dx,
            max_closed_dy,
            max_open_dy,
            min_beam_width,
            v_bar_margin,
            min_template_width,
            template_half,
            x_offsets,
        },
        staves: Vec::new(),
        summary: None,
        hash: Fnv1a64::new(),
    });
}

fn parse_staff(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("tablature");
    let tablature = fields.boolean();
    fields.keyword("drum");
    let drum = fields.boolean();
    fields.keyword("lines");
    let line_count = fields.number();
    fields.keyword("interline");
    let interline = fields.number();
    fields.keyword("headerStop");
    let header_stop = fields.number();
    fields.keyword("part");
    let part_id = fields.optional_number();
    fields.keyword("merged");
    let merged = fields.boolean();
    fields.keyword("partFirst");
    let part_first = fields.optional_number();
    fields.keyword("partLast");
    let part_last = fields.optional_number();
    fields.keyword("pointSize");
    let point_size = fields.number();
    fields.keyword("catalog");
    let (catalog_family, catalog_point_size) = fields.catalog();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    if let Some(previous) = system.staves.last() {
        assert!(
            previous.tablature || previous.summary.is_some(),
            "{} system {} staff {} missing summary",
            page_key,
            system_id,
            previous.id
        );
    }
    system.hash.add_line(line);
    system.staves.push(ExpectedStaff {
        id: staff_id,
        tablature,
        drum,
        line_count,
        interline,
        header_stop,
        part_id,
        merged,
        part_first,
        part_last,
        point_size,
        catalog_family,
        catalog_point_size,
        geometries: Vec::new(),
        schedule: Vec::new(),
        summary: None,
        hash: Fnv1a64::new(),
    });
}

fn parse_geometry(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("ordinal");
    let ordinal = fields.number();
    fields.keyword("source");
    let source = fields.source();
    fields.keyword("dir");
    let direction = fields.number();
    fields.keyword("pitch");
    let pitch = fields.number();
    fields.keyword("open");
    let open = fields.boolean();
    fields.keyword("interline");
    let interline = fields.number();
    fields.keyword("line");
    let line_range = fields.range();
    fields.keyword("line2");
    let line2 = fields.optional_range();
    fields.keyword("yOffsets");
    let y_offsets = fields.csv_numbers();
    fields.keyword("all");
    let all_shapes = fields.shapes();
    fields.keyword("stem");
    let stem_shapes = fields.shapes();
    fields.keyword("hollow");
    let hollow_shapes = fields.shapes();
    fields.keyword("farther");
    let farther_axes = fields.axes();
    fields.keyword("ordinate");
    let ordinate = fields.rle();
    fields.keyword("range");
    let range = fields.range();
    fields.keyword("rangeOrdinate");
    let range_ordinate = fields.rle();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    let staff = current_staff(system, staff_id);
    assert!(
        staff.schedule.is_empty(),
        "geometry after schedule on {} staff {}",
        page_key,
        staff_id
    );
    staff.hash.add_line(line);
    staff.geometries.push(ExpectedGeometry {
        ordinal,
        source,
        direction,
        pitch,
        open,
        interline,
        line: line_range,
        line2,
        y_offsets,
        all_shapes,
        stem_shapes,
        hollow_shapes,
        farther_axes,
        ordinate,
        range,
        range_ordinate,
    });
}

fn parse_schedule(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("phase");
    let phase = fields.phase();
    fields.keyword("ordinal");
    let ordinal = fields.number();
    fields.keyword("geometry");
    let geometry = fields.number();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    let staff = current_staff(system, staff_id);
    staff.hash.add_line(line);
    staff.schedule.push(ExpectedSchedule {
        phase,
        ordinal,
        geometry,
    });
}

fn parse_staff_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    fields.keyword("schedules");
    let schedules = fields.number();
    let hash = fields.hexadecimal();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    let staff = current_staff(system, staff_id);
    assert!(
        staff.summary.is_none(),
        "duplicate staff summary for {} staff {}",
        page_key,
        staff_id
    );
    assert_eq!(
        hash, staff.hash.0,
        "{} staff {} immediate hash",
        page_key, staff_id
    );
    staff.summary = Some(StaffSummary {
        geometries,
        schedules,
        hash,
    });
}

fn parse_system_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staves");
    let staves = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    fields.keyword("schedules");
    let schedules = fields.number();
    let hash = fields.hexadecimal();

    let page = current_page(oracle, &page_key);
    let system = current_system(page, system_id);
    assert!(
        system.summary.is_none(),
        "duplicate system summary for {} system {}",
        page_key,
        system_id
    );
    assert_eq!(
        hash, system.hash.0,
        "{} system {} immediate hash",
        page_key, system_id
    );
    system.summary = Some(SystemSummary {
        staves,
        geometries,
        schedules,
        hash,
    });
    page.hash.add_line(line);
}

fn parse_page_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>) {
    let page_key = fields.string();
    fields.keyword("standardStaves");
    let standard_staves = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    fields.keyword("schedules");
    let schedules = fields.number();
    let hash = fields.hexadecimal();

    let page = current_page(oracle, &page_key);
    assert!(
        page.summary.is_none(),
        "duplicate page summary for {page_key}"
    );
    assert_eq!(hash, page.hash.0, "{page_key} immediate page hash");
    page.summary = Some(PageSummary {
        standard_staves,
        geometries,
        schedules,
        hash,
    });
}

fn current_page<'a>(oracle: &'a mut ExpectedOracle, key: &str) -> &'a mut ExpectedPage {
    let page = oracle
        .pages
        .last_mut()
        .expect("a page row before detail rows");
    assert_eq!(page.key, key, "oracle pages remain contiguous");
    assert!(page.summary.is_none(), "detail row after {key} summary");
    page
}

fn current_system(page: &mut ExpectedPage, id: usize) -> &mut ExpectedSystem {
    let system = page
        .systems
        .last_mut()
        .expect("a params row before system rows");
    assert_eq!(system.id, id, "{} systems remain contiguous", page.key);
    assert!(
        system.summary.is_none(),
        "detail row after {} system {id} summary",
        page.key
    );
    system
}

fn current_staff(system: &mut ExpectedSystem, id: usize) -> &mut ExpectedStaff {
    let staff = system
        .staves
        .last_mut()
        .expect("a staff row before staff rows");
    assert_eq!(
        staff.id, id,
        "system {} staves remain contiguous",
        system.id
    );
    assert!(
        staff.summary.is_none(),
        "detail row after system {} staff {id} summary",
        system.id
    );
    staff
}

struct Fields<'a> {
    values: Vec<&'a str>,
    index: usize,
    line_number: usize,
}

impl<'a> Fields<'a> {
    fn new(line: &'a str, line_number: usize) -> Self {
        Self {
            values: line.split_whitespace().collect(),
            index: 0,
            line_number,
        }
    }

    fn take(&mut self) -> &'a str {
        let value = self
            .values
            .get(self.index)
            .copied()
            .unwrap_or_else(|| panic!("missing field on oracle line {}", self.line_number));
        self.index += 1;
        value
    }

    fn keyword(&mut self, expected: &str) {
        assert_eq!(
            self.take(),
            expected,
            "oracle line {} label",
            self.line_number
        );
    }

    fn string(&mut self) -> String {
        self.take().to_owned()
    }

    fn number<T>(&mut self) -> T
    where
        T: FromStr,
        T::Err: Debug,
    {
        let value = self.take();
        value.parse().unwrap_or_else(|error| {
            panic!(
                "invalid number {value:?} on oracle line {}: {error:?}",
                self.line_number
            )
        })
    }

    fn optional_number<T>(&mut self) -> Option<T>
    where
        T: FromStr,
        T::Err: Debug,
    {
        let value = self.take();
        (value != "-").then(|| {
            value.parse().unwrap_or_else(|error| {
                panic!(
                    "invalid optional number {value:?} on oracle line {}: {error:?}",
                    self.line_number
                )
            })
        })
    }

    fn boolean(&mut self) -> bool {
        match self.take() {
            "true" => true,
            "false" => false,
            value => panic!(
                "invalid boolean {value:?} on oracle line {}",
                self.line_number
            ),
        }
    }

    fn hexadecimal(&mut self) -> u64 {
        let value = self.take();
        assert_eq!(
            value.len(),
            16,
            "hex width on oracle line {}",
            self.line_number
        );
        u64::from_str_radix(value, 16).unwrap_or_else(|_| {
            panic!(
                "invalid hexadecimal {value:?} on oracle line {}",
                self.line_number
            )
        })
    }

    fn identity(&mut self) -> (String, usize) {
        let page = self.string();
        self.keyword("system");
        let system = self.number();
        (page, system)
    }

    fn csv_numbers<T>(&mut self) -> Vec<T>
    where
        T: FromStr,
        T::Err: Debug,
    {
        let value = self.take();
        assert_ne!(
            value, "-",
            "non-empty list on oracle line {}",
            self.line_number
        );
        value
            .split(',')
            .map(|item| {
                item.parse().unwrap_or_else(|error| {
                    panic!(
                        "invalid list item {item:?} on oracle line {}: {error:?}",
                        self.line_number
                    )
                })
            })
            .collect()
    }

    fn catalog(&mut self) -> (String, i32) {
        let value = self.take();
        let (family, point_size) = value.split_once('/').unwrap_or_else(|| {
            panic!(
                "invalid catalog {value:?} on oracle line {}",
                self.line_number
            )
        });
        assert!(
            !family.is_empty(),
            "catalog family on oracle line {}",
            self.line_number
        );
        (
            family.to_owned(),
            parse_number(point_size, self.line_number),
        )
    }

    fn double(&mut self) -> OracleDouble {
        parse_double(self.take(), self.line_number)
    }

    fn range(&mut self) -> InclusiveRange {
        InclusiveRange {
            left: self.number(),
            right: self.number(),
        }
    }

    fn optional_range(&mut self) -> Option<InclusiveRange> {
        let value = self.take();
        if value == "-" {
            return None;
        }
        let (left, right) = value.split_once(':').unwrap_or_else(|| {
            panic!(
                "invalid range {value:?} on oracle line {}",
                self.line_number
            )
        });
        Some(InclusiveRange {
            left: parse_number(left, self.line_number),
            right: parse_number(right, self.line_number),
        })
    }

    fn shapes(&mut self) -> Vec<HeadShape> {
        self.take()
            .split(',')
            .map(|shape| match shape {
                "BREVE" => HeadShape::Breve,
                "WHOLE_NOTE" => HeadShape::WholeNote,
                "NOTEHEAD_VOID" => HeadShape::NoteheadVoid,
                "NOTEHEAD_BLACK" => HeadShape::NoteheadBlack,
                value => panic!(
                    "invalid shape {value:?} on oracle line {}",
                    self.line_number
                ),
            })
            .collect()
    }

    fn phase(&mut self) -> ScannerPhase {
        match self.take() {
            "seed" => ScannerPhase::Seed,
            "range" => ScannerPhase::Range,
            value => panic!(
                "invalid scanner phase {value:?} on oracle line {}",
                self.line_number
            ),
        }
    }

    fn rle(&mut self) -> Vec<RleSpan> {
        let value = self.take();
        if value == "-" {
            return Vec::new();
        }
        value
            .split(',')
            .map(|item| {
                let (ordinate, length) = item.split_once(':').unwrap_or_else(|| {
                    panic!(
                        "invalid RLE span {item:?} on oracle line {}",
                        self.line_number
                    )
                });
                RleSpan {
                    ordinate: parse_number(ordinate, self.line_number),
                    length: parse_number(length, self.line_number),
                }
            })
            .collect()
    }

    fn axes(&mut self) -> Vec<Axis> {
        let value = self.take();
        if value == "-" {
            Vec::new()
        } else {
            value
                .split(',')
                .map(|axis| parse_axis(axis, self.line_number))
                .collect()
        }
    }

    fn source(&mut self) -> GeometrySource {
        let value = self.take();
        let parts = value.split(':').collect::<Vec<_>>();
        match parts.first().copied() {
            Some("staff-line") => {
                assert_eq!(
                    parts.len(),
                    7,
                    "staff-line source field count on oracle line {}",
                    self.line_number
                );
                assert_eq!(
                    parts[2], "axis",
                    "staff-line source label on oracle line {}",
                    self.line_number
                );
                GeometrySource::StaffLine {
                    line_index: parse_number(parts[1], self.line_number),
                    axis: parse_axis_parts(&parts[3..], self.line_number),
                }
            }
            Some("ledger") => {
                assert_eq!(
                    parts.len(),
                    17,
                    "ledger source field count on oracle line {}",
                    self.line_number
                );
                assert_eq!(
                    [parts[3], parts[8], parts[10], parts[12]],
                    ["bounds", "weight", "runs", "axis"],
                    "ledger source labels on oracle line {}",
                    self.line_number
                );
                GeometrySource::Ledger {
                    ledger_index: parse_number(parts[1], self.line_number),
                    ordinal: parse_number(parts[2], self.line_number),
                    bounds: Bounds {
                        left: parse_number(parts[4], self.line_number),
                        top: parse_number(parts[5], self.line_number),
                        width: parse_number(parts[6], self.line_number),
                        height: parse_number(parts[7], self.line_number),
                    },
                    weight: parse_number(parts[9], self.line_number),
                    run_hash: parse_hex(parts[11], self.line_number),
                    axis: parse_axis_parts(&parts[13..], self.line_number),
                }
            }
            _ => panic!(
                "invalid source {value:?} on oracle line {}",
                self.line_number
            ),
        }
    }

    fn finish(&self) {
        assert_eq!(
            self.index,
            self.values.len(),
            "extra fields on oracle line {}",
            self.line_number
        );
    }
}

fn parse_axis(value: &str, line_number: usize) -> Axis {
    let parts = value.split(':').collect::<Vec<_>>();
    parse_axis_parts(&parts, line_number)
}

fn parse_axis_parts(parts: &[&str], line_number: usize) -> Axis {
    assert_eq!(
        parts.len(),
        4,
        "axis field count on oracle line {line_number}"
    );
    Axis {
        left: OraclePoint {
            x: parse_double(parts[0], line_number),
            y: parse_double(parts[1], line_number),
        },
        right: OraclePoint {
            x: parse_double(parts[2], line_number),
            y: parse_double(parts[3], line_number),
        },
    }
}

fn parse_double(value: &str, line_number: usize) -> OracleDouble {
    let (java_hex, raw_bits) = value
        .split_once('/')
        .unwrap_or_else(|| panic!("invalid double {value:?} on oracle line {line_number}"));
    assert_eq!(
        raw_bits.len(),
        16,
        "double bit width on oracle line {line_number}"
    );
    let bits = parse_hex(raw_bits, line_number);
    let parsed = parse_java_hex_float(java_hex, line_number);
    assert_eq!(
        parsed.to_bits(),
        bits,
        "double text/bits mismatch on oracle line {line_number}"
    );
    OracleDouble {
        java_hex: java_hex.to_owned(),
        bits,
        value: f64::from_bits(bits),
    }
}

fn parse_java_hex_float(value: &str, line_number: usize) -> f64 {
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let unsigned = unsigned.strip_prefix("0x").unwrap_or_else(|| {
        panic!("invalid Java hexadecimal double {value:?} on oracle line {line_number}")
    });
    let (mantissa, exponent) = unsigned.split_once('p').unwrap_or_else(|| {
        panic!("missing double exponent in {value:?} on oracle line {line_number}")
    });
    let (whole, fraction) = mantissa.split_once('.').unwrap_or_else(|| {
        panic!("missing double radix point in {value:?} on oracle line {line_number}")
    });
    assert!(
        !whole.is_empty() && !fraction.is_empty(),
        "invalid double mantissa on oracle line {line_number}"
    );
    let digits = format!("{whole}{fraction}");
    let significand = u64::from_str_radix(&digits, 16).unwrap_or_else(|_| {
        panic!("invalid double mantissa {mantissa:?} on oracle line {line_number}")
    });
    let exponent: i32 = parse_number(exponent, line_number);
    let fractional_bits = i32::try_from(fraction.len() * 4).expect("hex double precision fits i32");
    let magnitude = (significand as f64) * 2.0_f64.powi(exponent - fractional_bits);
    if negative { -magnitude } else { magnitude }
}

fn parse_number<T>(value: &str, line_number: usize) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    value.parse().unwrap_or_else(|error| {
        panic!("invalid number {value:?} on oracle line {line_number}: {error:?}")
    })
}

fn parse_hex(value: &str, line_number: usize) -> u64 {
    assert_eq!(value.len(), 16, "hex width on oracle line {line_number}");
    u64::from_str_radix(value, 16)
        .unwrap_or_else(|_| panic!("invalid hexadecimal {value:?} on oracle line {line_number}"))
}

#[test]
fn scanner_context_oracle_parser_freezes_exact_corpus_shape() {
    let oracle = ExpectedOracle::parse(ORACLE);
    let systems = oracle
        .pages
        .iter()
        .map(|page| page.systems.len())
        .sum::<usize>();
    let standard_staves = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.staves)
        .filter(|staff| !staff.tablature)
        .count();
    let geometries = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.staves)
        .map(|staff| staff.geometries.len())
        .sum::<usize>();
    let schedules = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.staves)
        .map(|staff| staff.schedule.len())
        .sum::<usize>();

    assert_eq!(oracle.pages.len(), 8);
    assert_eq!(systems, 30);
    assert_eq!(standard_staves, 55);
    assert_eq!(geometries, 1_767);
    assert_eq!(schedules, 3_534);
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}

fn native_shape(shape: HeadShape) -> HeadTemplateShape {
    match shape {
        HeadShape::Breve => HeadTemplateShape::Breve,
        HeadShape::WholeNote => HeadTemplateShape::WholeNote,
        HeadShape::NoteheadVoid => HeadTemplateShape::NoteheadVoid,
        HeadShape::NoteheadBlack => HeadTemplateShape::NoteheadBlack,
    }
}

fn axis_bits(axis: HeadScannerAxis) -> [u64; 4] {
    [
        axis.x1.to_bits(),
        axis.y1.to_bits(),
        axis.x2.to_bits(),
        axis.y2.to_bits(),
    ]
}

fn expected_axis_bits(axis: &Axis) -> [u64; 4] {
    [
        axis.left.x.bits,
        axis.left.y.bits,
        axis.right.x.bits,
        axis.right.y.bits,
    ]
}

fn expand_rle(spans: &[RleSpan]) -> Vec<i32> {
    spans
        .iter()
        .flat_map(|span| std::iter::repeat_n(span.ordinate, span.length))
        .collect()
}

#[test]
fn native_scanner_context_matches_java_on_every_beam_sheet() {
    let oracle = ExpectedOracle::parse(ORACLE);
    let mut compared_geometries = 0_usize;
    let mut compared_schedules = 0_usize;

    for expected_page in oracle.pages {
        let image = expected_page
            .key
            .split('#')
            .next()
            .expect("page key has an image name");
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{}: GRID failed: {error}", expected_page.key));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{}: HEADERS failed: {error}", expected_page.key));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{}: STEM_SEEDS failed: {error}", expected_page.key));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{}: BEAMS failed: {error}", expected_page.key));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{}: LEDGERS failed: {error}", expected_page.key));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{}: HEADS prolog failed: {error}", expected_page.key));
        let actual =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| {
                    panic!("{}: scanner context failed: {error}", expected_page.key)
                });

        assert_eq!(
            (grid.scale.width, grid.scale.height),
            (expected_page.width, expected_page.height),
            "{} page dimensions",
            expected_page.key
        );
        assert_eq!(
            actual.systems.len(),
            expected_page.systems.len(),
            "{} systems",
            expected_page.key
        );

        for (expected_system, actual_system) in expected_page.systems.iter().zip(&actual.systems) {
            let context = format!("{} system {}", expected_page.key, expected_system.id);
            assert_eq!(actual_system.system_id, expected_system.id, "{context} id");
            let expected_params = &expected_system.params;
            let params = &actual_system.parameters;
            assert_eq!(
                params.main_interline, expected_params.main_interline,
                "{context} interline"
            );
            assert_eq!(
                params.max_stem,
                i32::try_from(expected_params.max_stem).unwrap(),
                "{context} max stem"
            );
            assert_eq!(
                params.max_distance_low.to_bits(),
                expected_params.max_distance_low.bits,
                "{context} max distance low"
            );
            assert_eq!(
                params.really_bad_distance.to_bits(),
                expected_params.really_bad_distance.bits,
                "{context} really bad distance"
            );
            assert_eq!(
                params.max_template_dx, expected_params.max_template_dx,
                "{context} max template dx"
            );
            assert_eq!(
                params.max_closed_dy, expected_params.max_closed_dy,
                "{context} max closed dy"
            );
            assert_eq!(
                params.max_open_dy, expected_params.max_open_dy,
                "{context} max open dy"
            );
            assert_eq!(
                params.min_beam_width, expected_params.min_beam_width,
                "{context} min beam width"
            );
            assert_eq!(
                params.vertical_bar_margin.to_bits(),
                expected_params.v_bar_margin.bits,
                "{context} bar margin"
            );
            assert_eq!(
                params.min_template_width, expected_params.min_template_width,
                "{context} min template width"
            );
            assert_eq!(
                params.template_half, expected_params.template_half,
                "{context} template half"
            );
            assert_eq!(
                params.x_offsets, expected_params.x_offsets,
                "{context} x offsets"
            );
            assert_eq!(
                actual_system.staffs.len(),
                expected_system.staves.len(),
                "{context} staves"
            );

            for (expected_staff, actual_staff) in
                expected_system.staves.iter().zip(&actual_system.staffs)
            {
                let context = format!("{context} staff {}", expected_staff.id);
                assert_eq!(actual_staff.staff_id, expected_staff.id, "{context} id");
                assert_eq!(
                    actual_staff.tablature, expected_staff.tablature,
                    "{context} tablature"
                );
                assert_eq!(actual_staff.drum, expected_staff.drum, "{context} drum");
                assert_eq!(
                    actual_staff.line_count, expected_staff.line_count,
                    "{context} line count"
                );
                assert_eq!(
                    actual_staff.specific_interline, expected_staff.interline,
                    "{context} interline"
                );
                assert_eq!(
                    actual_staff.header_stop, expected_staff.header_stop,
                    "{context} header stop"
                );
                assert_eq!(
                    actual_staff.merged, expected_staff.merged,
                    "{context} merged"
                );
                assert_eq!(
                    actual_staff.point_size,
                    Some(expected_staff.point_size),
                    "{context} point size"
                );
                let catalog_ordinal = actual_staff
                    .catalog_ordinal
                    .expect("standard staff catalog");
                assert_eq!(
                    heads.template_catalogs[catalog_ordinal].point_size(),
                    expected_staff.catalog_point_size,
                    "{context} catalog point size"
                );
                assert_eq!(
                    expected_staff.catalog_family, expected_page.family,
                    "{context} catalog family"
                );
                assert_eq!(
                    actual_staff.geometries.len(),
                    expected_staff.geometries.len(),
                    "{context} geometries"
                );

                for (expected_geometry, actual_geometry) in expected_staff
                    .geometries
                    .iter()
                    .zip(&actual_staff.geometries)
                {
                    let context = format!("{context} geometry {}", expected_geometry.ordinal);
                    match (&expected_geometry.source, &actual_geometry.source) {
                        (
                            GeometrySource::StaffLine { line_index, axis },
                            NativeHeadScannerSource::StaffLine {
                                line_index: actual_index,
                                axis: actual_axis,
                            },
                        ) => {
                            assert_eq!(actual_index, line_index, "{context} line source");
                            assert_eq!(
                                axis_bits(*actual_axis),
                                expected_axis_bits(axis),
                                "{context} staff axis"
                            );
                        }
                        (
                            GeometrySource::Ledger {
                                ledger_index,
                                ordinal,
                                bounds,
                                weight,
                                run_hash,
                                axis,
                            },
                            NativeHeadScannerSource::Ledger {
                                ledger_index: actual_index,
                                ordinal: actual_ordinal,
                                bounds: actual_bounds,
                                weight: actual_weight,
                                run_digest,
                                axis: actual_axis,
                                ..
                            },
                        ) => {
                            assert_eq!(actual_index, ledger_index, "{context} ledger index");
                            assert_eq!(actual_ordinal, ordinal, "{context} ledger ordinal");
                            assert_eq!(
                                (
                                    actual_bounds.x,
                                    actual_bounds.y,
                                    actual_bounds.width,
                                    actual_bounds.height
                                ),
                                (
                                    usize::try_from(bounds.left).unwrap(),
                                    usize::try_from(bounds.top).unwrap(),
                                    bounds.width,
                                    bounds.height
                                ),
                                "{context} ledger bounds"
                            );
                            assert_eq!(actual_weight, weight, "{context} ledger weight");
                            assert_eq!(run_digest, run_hash, "{context} ledger run hash");
                            assert_eq!(
                                axis_bits(*actual_axis),
                                expected_axis_bits(axis),
                                "{context} ledger axis"
                            );
                        }
                        _ => panic!("{context} source kind"),
                    }

                    assert_eq!(
                        actual_geometry.direction, expected_geometry.direction,
                        "{context} direction"
                    );
                    assert_eq!(
                        actual_geometry.pitch, expected_geometry.pitch,
                        "{context} pitch"
                    );
                    assert_eq!(
                        actual_geometry.open, expected_geometry.open,
                        "{context} open"
                    );
                    assert_eq!(
                        actual_geometry.line_count,
                        i32::try_from(expected_staff.line_count).unwrap(),
                        "{context} line count"
                    );
                    assert_eq!(
                        actual_geometry.interline, expected_geometry.interline,
                        "{context} interline"
                    );
                    assert_eq!(
                        actual_geometry.line_range(),
                        (expected_geometry.line.left, expected_geometry.line.right),
                        "{context} line range"
                    );
                    assert_eq!(
                        actual_geometry.line2_range(),
                        expected_geometry
                            .line2
                            .map(|range| (range.left, range.right)),
                        "{context} line2 range"
                    );
                    assert_eq!(
                        actual_geometry.y_offsets, expected_geometry.y_offsets,
                        "{context} y offsets"
                    );
                    assert_eq!(
                        actual_geometry.all_shapes,
                        expected_geometry
                            .all_shapes
                            .iter()
                            .copied()
                            .map(native_shape)
                            .collect::<Vec<_>>(),
                        "{context} all shapes"
                    );
                    assert_eq!(
                        actual_geometry.stem_shapes,
                        expected_geometry
                            .stem_shapes
                            .iter()
                            .copied()
                            .map(native_shape)
                            .collect::<Vec<_>>(),
                        "{context} stem shapes"
                    );
                    assert_eq!(
                        actual_geometry.hollow_shapes,
                        expected_geometry
                            .hollow_shapes
                            .iter()
                            .copied()
                            .map(native_shape)
                            .collect::<Vec<_>>(),
                        "{context} hollow shapes"
                    );
                    assert_eq!(
                        actual_geometry
                            .farther_ledgers
                            .iter()
                            .map(|ledger| axis_bits(ledger.axis()))
                            .collect::<Vec<_>>(),
                        expected_geometry
                            .farther_axes
                            .iter()
                            .map(expected_axis_bits)
                            .collect::<Vec<_>>(),
                        "{context} farther ledgers"
                    );

                    let actual_ordinate = (expected_geometry.line.left
                        ..=expected_geometry.line.right)
                        .map(|x| {
                            actual_geometry
                                .theoretical_ordinate(x)
                                .unwrap_or_else(|error| panic!("{context} ordinate x={x}: {error}"))
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual_ordinate,
                        expand_rle(&expected_geometry.ordinate),
                        "{context} full ordinate"
                    );
                    assert_eq!(
                        (actual_geometry.range_left, actual_geometry.range_right),
                        (expected_geometry.range.left, expected_geometry.range.right),
                        "{context} scan range"
                    );
                    let actual_range_ordinate =
                        if actual_geometry.range_left <= actual_geometry.range_right {
                            (actual_geometry.range_left..=actual_geometry.range_right)
                                .map(|x| {
                                    actual_geometry.theoretical_ordinate(x).unwrap_or_else(
                                        |error| panic!("{context} range ordinate x={x}: {error}"),
                                    )
                                })
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                    assert_eq!(
                        actual_range_ordinate,
                        expand_rle(&expected_geometry.range_ordinate),
                        "{context} range ordinate"
                    );
                    compared_geometries += 1;
                }

                assert_eq!(
                    actual_staff.schedule.len(),
                    expected_staff.schedule.len(),
                    "{context} schedule"
                );
                for (expected, actual) in expected_staff.schedule.iter().zip(&actual_staff.schedule)
                {
                    let phase = match expected.phase {
                        ScannerPhase::Seed => NativeHeadScannerPhase::Seed,
                        ScannerPhase::Range => NativeHeadScannerPhase::Range,
                    };
                    assert_eq!(
                        (actual.phase, actual.ordinal, actual.geometry),
                        (phase, expected.ordinal, expected.geometry),
                        "{context} schedule record"
                    );
                    compared_schedules += 1;
                }
            }
        }
    }

    assert_eq!(compared_geometries, 1_767);
    assert_eq!(compared_schedules, 3_534);
}
