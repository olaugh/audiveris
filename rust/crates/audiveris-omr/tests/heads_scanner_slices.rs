// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;

use audiveris_omr::{
    head_scanner_slices::{HeadScannerBand, JavaRectangle},
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-scanner-slices.txt");
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
    systems: Vec<ExpectedSystem>,
    summary: Option<PageSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct ExpectedSystem {
    id: usize,
    max_stem: Option<usize>,
    min_beam_width: Option<usize>,
    competitor_candidates: Vec<CompetitorCandidate>,
    competitors: Vec<InterRecord>,
    bar_candidates: Vec<BarCandidate>,
    bars: Vec<InterRecord>,
    staffs: Vec<ExpectedStaff>,
    summary: Option<SystemSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct CompetitorCandidate {
    input_ordinal: usize,
    decision: CompetitorDecision,
    accepted_ordinal: Option<usize>,
    max_stem: usize,
    min_beam_width: usize,
    evidence: CompetitorEvidence,
    inter: InterRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompetitorDecision {
    Accept,
    NotGood,
    ThinVertical,
    ShortBeamGroup,
}

#[derive(Clone, Debug)]
enum CompetitorEvidence {
    None,
    VerticalFloor(i32),
    BeamGroup {
        has_long_beam: bool,
        member_widths: Vec<usize>,
    },
}

#[derive(Clone, Debug)]
struct BarCandidate {
    input_ordinal: usize,
    decision: BarDecision,
    frozen_ordinal: Option<usize>,
    inter: InterRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BarDecision {
    Frozen,
    NotFrozen,
}

#[derive(Clone, Debug, PartialEq)]
struct InterRecord {
    class: String,
    shape: String,
    staff: Option<usize>,
    bounds: Rect,
    grade: OracleDouble,
    best_grade: OracleDouble,
    good: bool,
    frozen: bool,
    removed: bool,
    area: Rect,
    geometry: InterGeometry,
}

#[derive(Clone, Debug, PartialEq)]
enum InterGeometry {
    Vertical { axis: Axis, width: OracleDouble },
    Horizontal { axis: Axis, height: OracleDouble },
    None,
}

#[derive(Clone, Debug)]
struct ExpectedStaff {
    id: usize,
    scans: Vec<ExpectedScan>,
    summary: Option<StaffSummary>,
    hash: Fnv1a64,
}

#[derive(Clone, Debug)]
struct ExpectedScan {
    ordinal: usize,
    source: ScannerSource,
    seed_band: Band,
    competitor_band: Band,
    bar_band: Band,
    seeds_area: Rect,
    competitors_area: Rect,
    seeds: Vec<usize>,
    spots: Vec<usize>,
    competitors: Vec<usize>,
    bars: Vec<usize>,
}

#[derive(Clone, Debug)]
enum ScannerSource {
    StaffLine {
        line_index: usize,
        axis: Axis,
    },
    Ledger {
        ledger_index: i32,
        ordinal: usize,
        bounds: Rect,
        weight: usize,
        run_hash: u64,
        axis: Axis,
    },
}

#[derive(Clone, Debug)]
struct Band {
    above: OracleDouble,
    below: OracleDouble,
    effective_bottom: OracleDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rect {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Axis {
    left: Point,
    right: Point,
}

#[derive(Clone, Debug, PartialEq)]
struct Point {
    x: OracleDouble,
    y: OracleDouble,
}

#[derive(Clone, Debug)]
struct StaffSummary {
    geometries: usize,
    hash: u64,
}

#[derive(Clone, Debug)]
struct SystemSummary {
    seeds: usize,
    spots: usize,
    competitor_candidates: usize,
    competitors: usize,
    bar_candidates: usize,
    bars: usize,
    geometries: usize,
    hash: u64,
}

#[derive(Clone, Debug)]
struct PageSummary {
    competitor_candidates: usize,
    competitors: usize,
    bar_candidates: usize,
    bars: usize,
    geometries: usize,
    hash: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct OracleDouble {
    java_hex: String,
    bits: u64,
    value: f64,
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
                "headslicepage" => parse_page(&mut oracle, &mut fields),
                "headslicecompetitorcandidate" => {
                    parse_competitor_candidate(&mut oracle, &mut fields, line)
                }
                "headslicecompetitor" => parse_competitor(&mut oracle, &mut fields, line),
                "headslicebarcandidate" => parse_bar_candidate(&mut oracle, &mut fields, line),
                "headslicebar" => parse_bar(&mut oracle, &mut fields, line),
                "headslicescan" => parse_scan(&mut oracle, &mut fields, line),
                "headslicestaffsummary" => parse_staff_summary(&mut oracle, &mut fields, line),
                "headslicesystemsummary" => parse_system_summary(&mut oracle, &mut fields, line),
                "headslicepagesummary" => parse_page_summary(&mut oracle, &mut fields),
                kind => panic!("unexpected row kind {kind} on line {}", fields.line_number),
            }
            fields.finish();
        }
        oracle.validate();
        oracle
    }

    fn validate(&self) {
        let mut page_keys = BTreeSet::new();
        for page in &self.pages {
            assert!(page_keys.insert(&page.key), "duplicate page {}", page.key);
            assert!(page.width > 0 && page.height > 0, "{} dimensions", page.key);
            assert_eq!(
                page.systems.len(),
                page.declared_systems,
                "{} systems",
                page.key
            );

            let mut expected_staff_id = 1;
            for (system_index, system) in page.systems.iter().enumerate() {
                assert_eq!(system.id, system_index + 1, "{} system order", page.key);
                system.validate(page);
                for staff in &system.staffs {
                    assert_eq!(staff.id, expected_staff_id, "{} staff order", page.key);
                    expected_staff_id += 1;
                }
            }
            assert_eq!(
                expected_staff_id - 1,
                page.declared_staves,
                "{} staves",
                page.key
            );

            let summary = page
                .summary
                .as_ref()
                .unwrap_or_else(|| panic!("{} lacks page summary", page.key));
            let competitor_candidates = page
                .systems
                .iter()
                .map(|system| system.competitor_candidates.len())
                .sum::<usize>();
            let competitors = page
                .systems
                .iter()
                .map(|system| system.competitors.len())
                .sum::<usize>();
            let bar_candidates = page
                .systems
                .iter()
                .map(|system| system.bar_candidates.len())
                .sum::<usize>();
            let bars = page
                .systems
                .iter()
                .map(|system| system.bars.len())
                .sum::<usize>();
            let geometries = page
                .systems
                .iter()
                .flat_map(|system| &system.staffs)
                .map(|staff| staff.scans.len())
                .sum::<usize>();
            assert_eq!(
                summary.competitor_candidates, competitor_candidates,
                "{} competitor candidates",
                page.key
            );
            assert_eq!(summary.competitors, competitors, "{} competitors", page.key);
            assert_eq!(
                summary.bar_candidates, bar_candidates,
                "{} bar candidates",
                page.key
            );
            assert_eq!(summary.bars, bars, "{} bars", page.key);
            assert_eq!(summary.geometries, geometries, "{} geometries", page.key);
            assert_eq!(summary.hash, page.hash.0, "{} page hash", page.key);
        }
    }
}

impl ExpectedSystem {
    fn new(id: usize) -> Self {
        Self {
            id,
            max_stem: None,
            min_beam_width: None,
            competitor_candidates: Vec::new(),
            competitors: Vec::new(),
            bar_candidates: Vec::new(),
            bars: Vec::new(),
            staffs: Vec::new(),
            summary: None,
            hash: Fnv1a64::new(),
        }
    }

    fn validate(&self, page: &ExpectedPage) {
        let max_stem = self.max_stem.expect("competitor candidates set maxStem");
        let min_beam_width = self
            .min_beam_width
            .expect("competitor candidates set minBeamWidth");
        let mut accepted = BTreeSet::new();
        for (ordinal, candidate) in self.competitor_candidates.iter().enumerate() {
            assert_eq!(
                candidate.input_ordinal, ordinal,
                "{} system {} candidate order",
                page.key, self.id
            );
            assert_eq!(
                candidate.max_stem, max_stem,
                "{} system {} maxStem",
                page.key, self.id
            );
            assert_eq!(
                candidate.min_beam_width, min_beam_width,
                "{} system {} minBeamWidth",
                page.key, self.id
            );
            candidate.validate(page, self);
            if let Some(accepted_ordinal) = candidate.accepted_ordinal {
                assert!(
                    accepted.insert(accepted_ordinal),
                    "{} system {} duplicate accepted ordinal",
                    page.key,
                    self.id
                );
                let expected = self.competitors.get(accepted_ordinal).unwrap_or_else(|| {
                    panic!(
                        "{} system {} accepted competitor out of bounds",
                        page.key, self.id
                    )
                });
                assert_eq!(
                    &candidate.inter, expected,
                    "{} system {} accepted competitor identity",
                    page.key, self.id
                );
            }
        }
        assert_eq!(
            accepted.len(),
            self.competitors.len(),
            "{} system {} accepted coverage",
            page.key,
            self.id
        );
        assert_eq!(
            accepted.into_iter().collect::<Vec<_>>(),
            (0..self.competitors.len()).collect::<Vec<_>>(),
            "{} system {} accepted ordinal coverage",
            page.key,
            self.id
        );
        for competitor in &self.competitors {
            competitor.validate();
            assert!(
                competitor.good && !competitor.removed,
                "{} system {} accepted competitor state",
                page.key,
                self.id
            );
        }

        let mut frozen = BTreeSet::new();
        for (ordinal, candidate) in self.bar_candidates.iter().enumerate() {
            assert_eq!(
                candidate.input_ordinal, ordinal,
                "{} system {} bar candidate order",
                page.key, self.id
            );
            candidate.validate();
            if let Some(frozen_ordinal) = candidate.frozen_ordinal {
                assert!(
                    frozen.insert(frozen_ordinal),
                    "{} system {} duplicate frozen ordinal",
                    page.key,
                    self.id
                );
                let expected = self.bars.get(frozen_ordinal).unwrap_or_else(|| {
                    panic!("{} system {} frozen bar out of bounds", page.key, self.id)
                });
                assert_eq!(
                    &candidate.inter, expected,
                    "{} system {} frozen bar identity",
                    page.key, self.id
                );
            }
        }
        assert_eq!(
            frozen.into_iter().collect::<Vec<_>>(),
            (0..self.bars.len()).collect::<Vec<_>>(),
            "{} system {} frozen ordinal coverage",
            page.key,
            self.id
        );
        for bar in &self.bars {
            bar.validate();
            assert!(
                bar.frozen && !bar.removed,
                "{} system {} frozen bar state",
                page.key,
                self.id
            );
            assert!(
                bar.is_bar(),
                "{} system {} bar pool type",
                page.key,
                self.id
            );
        }

        for staff in &self.staffs {
            staff.validate(page, self);
        }
        let summary = self
            .summary
            .as_ref()
            .unwrap_or_else(|| panic!("{} system {} lacks summary", page.key, self.id));
        let geometries = self
            .staffs
            .iter()
            .map(|staff| staff.scans.len())
            .sum::<usize>();
        assert_eq!(
            summary.competitor_candidates,
            self.competitor_candidates.len(),
            "{} system {} candidate summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.competitors,
            self.competitors.len(),
            "{} system {} competitor summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.bar_candidates,
            self.bar_candidates.len(),
            "{} system {} bar candidate summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.bars,
            self.bars.len(),
            "{} system {} bar summary",
            page.key,
            self.id
        );
        assert_eq!(
            summary.geometries, geometries,
            "{} system {} geometry summary",
            page.key, self.id
        );
        assert_eq!(
            summary.hash, self.hash.0,
            "{} system {} hash",
            page.key, self.id
        );
        assert!(
            summary.seeds > 0 && summary.spots > 0,
            "{} system {} source pools",
            page.key,
            self.id
        );
    }
}

impl CompetitorCandidate {
    fn validate(&self, page: &ExpectedPage, system: &ExpectedSystem) {
        self.inter.validate();
        let expected_decision = if !self.inter.good {
            CompetitorDecision::NotGood
        } else {
            match (&self.inter.geometry, &self.evidence) {
                (
                    InterGeometry::Vertical { width, .. },
                    CompetitorEvidence::VerticalFloor(floor),
                ) => {
                    assert_eq!(
                        *floor,
                        width.value.floor() as i32,
                        "{} system {} vertical evidence",
                        page.key,
                        system.id
                    );
                    if usize::try_from(*floor).unwrap() <= self.max_stem {
                        CompetitorDecision::ThinVertical
                    } else {
                        CompetitorDecision::Accept
                    }
                }
                (
                    InterGeometry::Horizontal { .. },
                    CompetitorEvidence::BeamGroup {
                        has_long_beam,
                        member_widths,
                    },
                ) => {
                    assert!(
                        !member_widths.is_empty(),
                        "{} system {} beam evidence",
                        page.key,
                        system.id
                    );
                    assert_eq!(
                        *has_long_beam,
                        member_widths
                            .iter()
                            .any(|width| *width >= self.min_beam_width),
                        "{} system {} long-beam evidence",
                        page.key,
                        system.id
                    );
                    if *has_long_beam {
                        CompetitorDecision::Accept
                    } else {
                        CompetitorDecision::ShortBeamGroup
                    }
                }
                (InterGeometry::Horizontal { .. }, CompetitorEvidence::None) => {
                    assert_eq!(
                        self.inter.class, "MultipleRestInter",
                        "{} system {} evidence-free competitor",
                        page.key, system.id
                    );
                    CompetitorDecision::Accept
                }
                _ => panic!(
                    "{} system {} incompatible competitor evidence",
                    page.key, system.id
                ),
            }
        };
        assert_eq!(
            self.decision, expected_decision,
            "{} system {} competitor decision",
            page.key, system.id
        );
        assert_eq!(
            self.accepted_ordinal.is_some(),
            self.decision == CompetitorDecision::Accept,
            "{} system {} accepted ordinal",
            page.key,
            system.id
        );
        assert!(
            !self.inter.removed,
            "{} system {} removed candidate",
            page.key, system.id
        );
    }
}

impl BarCandidate {
    fn validate(&self) {
        self.inter.validate();
        assert!(self.inter.is_bar(), "bar candidate class/shape");
        assert_eq!(
            self.decision,
            if self.inter.frozen {
                BarDecision::Frozen
            } else {
                BarDecision::NotFrozen
            }
        );
        assert_eq!(
            self.frozen_ordinal.is_some(),
            self.decision == BarDecision::Frozen
        );
        assert!(!self.inter.removed, "removed bar candidate");
    }
}

impl InterRecord {
    fn validate(&self) {
        assert!(
            self.bounds.width > 0 && self.bounds.height > 0,
            "inter bounds"
        );
        assert_eq!(self.area, self.bounds, "inter Area bounds");
        self.grade.validate();
        self.best_grade.validate();
        assert_eq!(
            self.good,
            self.grade.value >= self.good_threshold(),
            "{} good flag",
            self.class
        );
        match &self.geometry {
            InterGeometry::Vertical { axis, width } => {
                assert!(matches!(
                    self.class.as_str(),
                    "BarlineInter" | "BarConnectorInter" | "VerticalSerifInter"
                ));
                axis.validate_vertical();
                width.validate();
                assert!(width.value > 0.0, "vertical width");
            }
            InterGeometry::Horizontal { axis, height } => {
                assert!(matches!(
                    self.class.as_str(),
                    "BeamInter" | "BeamHookInter" | "MultipleRestInter"
                ));
                axis.validate_horizontal();
                height.validate();
                assert!(height.value > 0.0, "horizontal height");
            }
            InterGeometry::None => panic!("{} unexpectedly lacks subtype geometry", self.class),
        }
        match self.class.as_str() {
            "BarlineInter" => assert!(matches!(
                self.shape.as_str(),
                "THIN_BARLINE" | "THICK_BARLINE" | "DUMMY_BARLINE"
            )),
            "BarConnectorInter" => assert!(matches!(
                self.shape.as_str(),
                "THIN_CONNECTOR" | "THICK_CONNECTOR"
            )),
            "BeamInter" => assert_eq!(self.shape, "BEAM"),
            "BeamHookInter" => assert_eq!(self.shape, "BEAM_HOOK"),
            "MultipleRestInter" => assert_eq!(self.shape, "MULTIPLE_REST"),
            "VerticalSerifInter" => assert_eq!(self.shape, "VERTICAL_SERIF"),
            class => panic!("unexpected competitor/bar class {class}"),
        }
        if let Some(staff) = self.staff {
            assert!(staff > 0, "positive staff id");
        }
    }

    fn is_bar(&self) -> bool {
        matches!(self.class.as_str(), "BarlineInter" | "BarConnectorInter")
    }

    fn good_threshold(&self) -> f64 {
        match self.class.as_str() {
            "BarlineInter" => 0.6,
            "BarConnectorInter" => 0.65,
            "BeamInter" | "BeamHookInter" => 0.35,
            _ => 0.4,
        }
    }
}

impl ExpectedStaff {
    fn new(id: usize) -> Self {
        Self {
            id,
            scans: Vec::new(),
            summary: None,
            hash: Fnv1a64::new(),
        }
    }

    fn validate(&self, page: &ExpectedPage, system: &ExpectedSystem) {
        for (ordinal, scan) in self.scans.iter().enumerate() {
            assert_eq!(
                scan.ordinal, ordinal,
                "{} staff {} scan order",
                page.key, self.id
            );
            scan.validate(page, system, self);
        }
        let summary = self.summary.as_ref().unwrap_or_else(|| {
            panic!(
                "{} system {} staff {} lacks summary",
                page.key, system.id, self.id
            )
        });
        assert_eq!(
            summary.geometries,
            self.scans.len(),
            "{} staff {} geometry summary",
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

impl ExpectedScan {
    fn validate(&self, page: &ExpectedPage, system: &ExpectedSystem, staff: &ExpectedStaff) {
        let label = format!(
            "{} system {} staff {} scan {}",
            page.key, system.id, staff.id, self.ordinal
        );
        self.source.validate(&label);
        self.seed_band.validate(&format!("{label} seed band"));
        self.competitor_band
            .validate(&format!("{label} competitor band"));
        self.bar_band.validate(&format!("{label} bar band"));
        assert!(
            self.seeds_area.width > 0 && self.seeds_area.height > 0,
            "{label} seeds Area"
        );
        assert!(
            self.competitors_area.width > 0 && self.competitors_area.height > 0,
            "{label} competitors Area"
        );
        let summary = system
            .summary
            .as_ref()
            .expect("system summary available during validation");
        validate_pool_indexes(&self.seeds, summary.seeds, &format!("{label} seeds"));
        validate_pool_indexes(&self.spots, summary.spots, &format!("{label} spots"));
        validate_pool_indexes(
            &self.competitors,
            system.competitors.len(),
            &format!("{label} competitors"),
        );
        validate_pool_indexes(&self.bars, system.bars.len(), &format!("{label} bars"));
        for pair in self.competitors.windows(2) {
            let left = system.competitors[pair[0]].bounds.left;
            let right = system.competitors[pair[1]].bounds.left;
            assert!(left <= right, "{label} competitor abscissa order");
        }
        assert!(
            self.bars.windows(2).all(|pair| pair[0] < pair[1]),
            "{label} frozen-bar source order"
        );
    }
}

impl ScannerSource {
    fn validate(&self, label: &str) {
        match self {
            Self::StaffLine { line_index, axis } => {
                assert!(*line_index < 5, "{label} staff-line index");
                axis.validate_horizontal();
            }
            Self::Ledger {
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
                assert!(*weight > 0, "{label} ledger weight");
                assert_ne!(*run_hash, 0, "{label} ledger run hash");
                axis.validate_horizontal();
            }
        }
    }
}

impl Band {
    fn validate(&self, label: &str) {
        self.above.validate();
        self.below.validate();
        self.effective_bottom.validate();
        assert!(self.above.value <= self.below.value, "{label} order");
        assert_eq!(
            self.effective_bottom.value.to_bits(),
            (self.below.value + 1.0).to_bits(),
            "{label} effective bottom"
        );
    }
}

impl Axis {
    fn validate_horizontal(&self) {
        self.validate();
        assert!(
            self.left.x.value <= self.right.x.value,
            "horizontal axis order"
        );
    }

    fn validate_vertical(&self) {
        self.validate();
        assert!(
            self.left.y.value <= self.right.y.value,
            "vertical axis order"
        );
    }

    fn validate(&self) {
        self.left.x.validate();
        self.left.y.validate();
        self.right.x.validate();
        self.right.y.validate();
    }
}

impl OracleDouble {
    fn validate(&self) {
        assert!(self.java_hex.starts_with("0x") || self.java_hex.starts_with("-0x"));
        assert_eq!(self.value.to_bits(), self.bits, "{} bits", self.java_hex);
        assert!(self.value.is_finite(), "finite oracle double");
    }
}

fn validate_pool_indexes(indexes: &[usize], pool_len: usize, label: &str) {
    let mut unique = BTreeSet::new();
    for index in indexes {
        assert!(*index < pool_len, "{label} pool bound");
        assert!(unique.insert(*index), "{label} duplicate index");
    }
}

fn parse_page(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>) {
    if let Some(previous) = oracle.pages.last() {
        assert!(
            previous.summary.is_some(),
            "{} missing page summary",
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
    oracle.pages.push(ExpectedPage {
        key,
        width,
        height,
        declared_systems,
        declared_staves,
        systems: Vec::new(),
        summary: None,
        hash: Fnv1a64::new(),
    });
}

fn parse_competitor_candidate(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("inputOrdinal");
    let input_ordinal = fields.number();
    fields.keyword("decision");
    let decision = fields.competitor_decision();
    fields.keyword("acceptedOrdinal");
    let accepted_ordinal = fields.optional_number();
    fields.keyword("maxStem");
    let max_stem = fields.number();
    fields.keyword("minBeamWidth");
    let min_beam_width = fields.number();
    fields.keyword("evidence");
    let evidence = fields.competitor_evidence();
    let inter = fields.inter_record();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = ensure_system(page, system_id);
    system.hash.add_line(line);
    if let Some(expected) = system.max_stem {
        assert_eq!(expected, max_stem, "{page_key} system {system_id} maxStem");
    } else {
        system.max_stem = Some(max_stem);
    }
    if let Some(expected) = system.min_beam_width {
        assert_eq!(
            expected, min_beam_width,
            "{page_key} system {system_id} minBeamWidth"
        );
    } else {
        system.min_beam_width = Some(min_beam_width);
    }
    system.competitor_candidates.push(CompetitorCandidate {
        input_ordinal,
        decision,
        accepted_ordinal,
        max_stem,
        min_beam_width,
        evidence,
        inter,
    });
}

fn parse_competitor(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("ordinal");
    let ordinal: usize = fields.number();
    let inter = fields.inter_record();
    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    assert_eq!(
        ordinal,
        system.competitors.len(),
        "{page_key} system {system_id} competitor order"
    );
    system.hash.add_line(line);
    system.competitors.push(inter);
}

fn parse_bar_candidate(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("inputOrdinal");
    let input_ordinal = fields.number();
    fields.keyword("decision");
    let decision = fields.bar_decision();
    fields.keyword("frozenOrdinal");
    let frozen_ordinal = fields.optional_number();
    let inter = fields.inter_record();
    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    system.bar_candidates.push(BarCandidate {
        input_ordinal,
        decision,
        frozen_ordinal,
        inter,
    });
}

fn parse_bar(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("ordinal");
    let ordinal: usize = fields.number();
    let inter = fields.inter_record();
    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    assert_eq!(
        ordinal,
        system.bars.len(),
        "{page_key} system {system_id} bar order"
    );
    system.hash.add_line(line);
    system.bars.push(inter);
}

fn parse_scan(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("ordinal");
    let ordinal = fields.number();
    fields.keyword("source");
    let source = fields.scanner_source();
    fields.keyword("seedBand");
    let seed_band = fields.band();
    fields.keyword("competitorBand");
    let competitor_band = fields.band();
    fields.keyword("barBand");
    let bar_band = fields.band();
    fields.keyword("seedsArea");
    let seeds_area = fields.rect();
    fields.keyword("competitorsArea");
    let competitors_area = fields.rect();
    fields.keyword("seeds");
    let seeds = fields.indexes();
    fields.keyword("spots");
    let spots = fields.indexes();
    fields.keyword("competitors");
    let competitors = fields.indexes();
    fields.keyword("bars");
    let bars = fields.indexes();

    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    let staff = ensure_staff(system, staff_id);
    staff.hash.add_line(line);
    staff.scans.push(ExpectedScan {
        ordinal,
        source,
        seed_band,
        competitor_band,
        bar_band,
        seeds_area,
        competitors_area,
        seeds,
        spots,
        competitors,
        bars,
    });
}

fn parse_staff_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("staff");
    let staff_id = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    let hash = fields.hexadecimal();
    let page = current_page(oracle, &page_key);
    page.hash.add_line(line);
    let system = current_system(page, system_id);
    system.hash.add_line(line);
    let staff = current_staff(system, staff_id);
    assert_eq!(
        hash, staff.hash.0,
        "{page_key} staff {staff_id} immediate hash"
    );
    assert!(staff.summary.is_none(), "duplicate staff summary");
    staff.summary = Some(StaffSummary { geometries, hash });
}

fn parse_system_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>, line: &str) {
    let (page_key, system_id) = fields.identity();
    fields.keyword("seeds");
    let seeds = fields.number();
    fields.keyword("spots");
    let spots = fields.number();
    fields.keyword("competitorCandidates");
    let competitor_candidates = fields.number();
    fields.keyword("competitors");
    let competitors = fields.number();
    fields.keyword("barCandidates");
    let bar_candidates = fields.number();
    fields.keyword("bars");
    let bars = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    let hash = fields.hexadecimal();
    let page = current_page(oracle, &page_key);
    {
        let system = current_system(page, system_id);
        assert_eq!(
            hash, system.hash.0,
            "{page_key} system {system_id} immediate hash"
        );
        assert!(system.summary.is_none(), "duplicate system summary");
        system.summary = Some(SystemSummary {
            seeds,
            spots,
            competitor_candidates,
            competitors,
            bar_candidates,
            bars,
            geometries,
            hash,
        });
    }
    page.hash.add_line(line);
}

fn parse_page_summary(oracle: &mut ExpectedOracle, fields: &mut Fields<'_>) {
    let page_key = fields.string();
    fields.keyword("competitorCandidates");
    let competitor_candidates = fields.number();
    fields.keyword("competitors");
    let competitors = fields.number();
    fields.keyword("barCandidates");
    let bar_candidates = fields.number();
    fields.keyword("bars");
    let bars = fields.number();
    fields.keyword("geometries");
    let geometries = fields.number();
    let hash = fields.hexadecimal();
    let page = current_page(oracle, &page_key);
    assert_eq!(hash, page.hash.0, "{page_key} immediate page hash");
    assert!(page.summary.is_none(), "duplicate page summary");
    page.summary = Some(PageSummary {
        competitor_candidates,
        competitors,
        bar_candidates,
        bars,
        geometries,
        hash,
    });
}

fn current_page<'a>(oracle: &'a mut ExpectedOracle, key: &str) -> &'a mut ExpectedPage {
    let page = oracle.pages.last_mut().expect("page row before details");
    assert_eq!(page.key, key, "pages remain contiguous");
    assert!(page.summary.is_none(), "detail after {key} page summary");
    page
}

fn ensure_system(page: &mut ExpectedPage, id: usize) -> &mut ExpectedSystem {
    if page.systems.last().is_some_and(|system| system.id == id) {
        return page.systems.last_mut().unwrap();
    }
    if let Some(previous) = page.systems.last() {
        assert!(
            previous.summary.is_some(),
            "{} system {} missing summary",
            page.key,
            previous.id
        );
    }
    assert_eq!(id, page.systems.len() + 1, "{} system order", page.key);
    page.systems.push(ExpectedSystem::new(id));
    page.systems.last_mut().unwrap()
}

fn current_system(page: &mut ExpectedPage, id: usize) -> &mut ExpectedSystem {
    let system = page
        .systems
        .last_mut()
        .expect("candidate row starts each system");
    assert_eq!(system.id, id, "{} systems remain contiguous", page.key);
    assert!(
        system.summary.is_none(),
        "detail after {} system {id} summary",
        page.key
    );
    system
}

fn ensure_staff(system: &mut ExpectedSystem, id: usize) -> &mut ExpectedStaff {
    if system.staffs.last().is_some_and(|staff| staff.id == id) {
        return system.staffs.last_mut().unwrap();
    }
    if let Some(previous) = system.staffs.last() {
        assert!(
            previous.summary.is_some(),
            "system {} staff {} missing summary",
            system.id,
            previous.id
        );
    }
    system.staffs.push(ExpectedStaff::new(id));
    system.staffs.last_mut().unwrap()
}

fn current_staff(system: &mut ExpectedSystem, id: usize) -> &mut ExpectedStaff {
    let staff = system.staffs.last_mut().expect("scan before staff summary");
    assert_eq!(
        staff.id, id,
        "system {} staffs remain contiguous",
        system.id
    );
    assert!(staff.summary.is_none(), "detail after staff {id} summary");
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
        parse_number(self.take(), self.line_number)
    }

    fn optional_number<T>(&mut self) -> Option<T>
    where
        T: FromStr,
        T::Err: Debug,
    {
        let value = self.take();
        (value != "-").then(|| parse_number(value, self.line_number))
    }

    fn boolean(&mut self) -> bool {
        match self.take() {
            "true" => true,
            "false" => false,
            value => panic!("invalid boolean {value:?} on line {}", self.line_number),
        }
    }

    fn hexadecimal(&mut self) -> u64 {
        parse_hex(self.take(), self.line_number)
    }

    fn identity(&mut self) -> (String, usize) {
        let page = self.string();
        self.keyword("system");
        (page, self.number())
    }

    fn rect(&mut self) -> Rect {
        parse_rect(self.take(), self.line_number)
    }

    fn indexes(&mut self) -> Vec<usize> {
        let value = self.take();
        if value == "-" {
            Vec::new()
        } else {
            value
                .split(',')
                .map(|item| parse_number(item, self.line_number))
                .collect()
        }
    }

    fn competitor_decision(&mut self) -> CompetitorDecision {
        match self.take() {
            "accept" => CompetitorDecision::Accept,
            "not-good" => CompetitorDecision::NotGood,
            "thin-vertical" => CompetitorDecision::ThinVertical,
            "short-beam-group" => CompetitorDecision::ShortBeamGroup,
            value => panic!(
                "invalid competitor decision {value:?} on line {}",
                self.line_number
            ),
        }
    }

    fn bar_decision(&mut self) -> BarDecision {
        match self.take() {
            "frozen" => BarDecision::Frozen,
            "not-frozen" => BarDecision::NotFrozen,
            value => panic!(
                "invalid bar decision {value:?} on line {}",
                self.line_number
            ),
        }
    }

    fn competitor_evidence(&mut self) -> CompetitorEvidence {
        let value = self.take();
        if value == "-" {
            return CompetitorEvidence::None;
        }
        if let Some(floor) = value.strip_prefix("vertical-floor:") {
            return CompetitorEvidence::VerticalFloor(parse_number(floor, self.line_number));
        }
        if let Some(rest) = value.strip_prefix("beam-group:") {
            let (has_long_beam, widths) = rest.split_once(':').unwrap_or_else(|| {
                panic!(
                    "invalid beam evidence {value:?} on line {}",
                    self.line_number
                )
            });
            let has_long_beam = match has_long_beam {
                "true" => true,
                "false" => false,
                _ => panic!("invalid beam evidence flag on line {}", self.line_number),
            };
            let member_widths = widths
                .split(',')
                .map(|width| parse_number(width, self.line_number))
                .collect();
            return CompetitorEvidence::BeamGroup {
                has_long_beam,
                member_widths,
            };
        }
        panic!(
            "invalid competitor evidence {value:?} on line {}",
            self.line_number
        )
    }

    fn inter_record(&mut self) -> InterRecord {
        self.keyword("class");
        let class = self.string();
        self.keyword("shape");
        let shape = self.string();
        self.keyword("staff");
        let staff = self.optional_number();
        self.keyword("bounds");
        let bounds = self.rect();
        self.keyword("grade");
        let grade = self.double();
        self.keyword("best");
        let best_grade = self.double();
        self.keyword("good");
        let good = self.boolean();
        self.keyword("frozen");
        let frozen = self.boolean();
        self.keyword("removed");
        let removed = self.boolean();
        self.keyword("area");
        let area = self.rect();
        let geometry = match self.take() {
            "vertical" => {
                let axis = parse_axis(self.take(), self.line_number);
                self.keyword("width");
                InterGeometry::Vertical {
                    axis,
                    width: self.double(),
                }
            }
            "horizontal" => {
                let axis = parse_axis(self.take(), self.line_number);
                self.keyword("height");
                InterGeometry::Horizontal {
                    axis,
                    height: self.double(),
                }
            }
            "geometry" => {
                self.keyword("-");
                InterGeometry::None
            }
            value => panic!(
                "invalid Inter geometry {value:?} on line {}",
                self.line_number
            ),
        };
        InterRecord {
            class,
            shape,
            staff,
            bounds,
            grade,
            best_grade,
            good,
            frozen,
            removed,
            area,
            geometry,
        }
    }

    fn scanner_source(&mut self) -> ScannerSource {
        let value = self.take();
        let parts = value.split(':').collect::<Vec<_>>();
        match parts.first().copied() {
            Some("staff-line") => {
                assert_eq!(
                    parts.len(),
                    7,
                    "staff source fields on line {}",
                    self.line_number
                );
                assert_eq!(
                    parts[2], "axis",
                    "staff source label on line {}",
                    self.line_number
                );
                ScannerSource::StaffLine {
                    line_index: parse_number(parts[1], self.line_number),
                    axis: parse_axis_parts(&parts[3..], self.line_number),
                }
            }
            Some("ledger") => {
                assert_eq!(
                    parts.len(),
                    17,
                    "ledger source fields on line {}",
                    self.line_number
                );
                assert_eq!(
                    [parts[3], parts[8], parts[10], parts[12]],
                    ["bounds", "weight", "runs", "axis"]
                );
                ScannerSource::Ledger {
                    ledger_index: parse_number(parts[1], self.line_number),
                    ordinal: parse_number(parts[2], self.line_number),
                    bounds: Rect {
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
                "invalid scanner source {value:?} on line {}",
                self.line_number
            ),
        }
    }

    fn band(&mut self) -> Band {
        let value = self.take();
        let parts = value.split(':').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3, "band fields on line {}", self.line_number);
        Band {
            above: parse_double(parts[0], self.line_number),
            below: parse_double(parts[1], self.line_number),
            effective_bottom: parse_double(parts[2], self.line_number),
        }
    }

    fn double(&mut self) -> OracleDouble {
        parse_double(self.take(), self.line_number)
    }

    fn finish(&self) {
        assert_eq!(
            self.index,
            self.values.len(),
            "extra fields on line {}",
            self.line_number
        );
    }
}

fn parse_rect(value: &str, line_number: usize) -> Rect {
    let parts = value.split(':').collect::<Vec<_>>();
    assert_eq!(parts.len(), 4, "rectangle fields on line {line_number}");
    Rect {
        left: parse_number(parts[0], line_number),
        top: parse_number(parts[1], line_number),
        width: parse_number(parts[2], line_number),
        height: parse_number(parts[3], line_number),
    }
}

fn parse_axis(value: &str, line_number: usize) -> Axis {
    let parts = value.split(':').collect::<Vec<_>>();
    parse_axis_parts(&parts, line_number)
}

fn parse_axis_parts(parts: &[&str], line_number: usize) -> Axis {
    assert_eq!(parts.len(), 4, "axis fields on line {line_number}");
    Axis {
        left: Point {
            x: parse_double(parts[0], line_number),
            y: parse_double(parts[1], line_number),
        },
        right: Point {
            x: parse_double(parts[2], line_number),
            y: parse_double(parts[3], line_number),
        },
    }
}

fn parse_double(value: &str, line_number: usize) -> OracleDouble {
    let (java_hex, raw_bits) = value
        .split_once('/')
        .unwrap_or_else(|| panic!("invalid double {value:?} on line {line_number}"));
    let bits = parse_hex(raw_bits, line_number);
    let parsed = parse_java_hex_float(java_hex, line_number);
    assert_eq!(
        parsed.to_bits(),
        bits,
        "double text/bits mismatch on line {line_number}"
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
        panic!("invalid Java hexadecimal double {value:?} on line {line_number}")
    });
    let (mantissa, exponent) = unsigned
        .split_once('p')
        .unwrap_or_else(|| panic!("missing exponent in {value:?} on line {line_number}"));
    let (whole, fraction) = mantissa
        .split_once('.')
        .unwrap_or_else(|| panic!("missing radix point in {value:?} on line {line_number}"));
    assert!(
        !whole.is_empty() && !fraction.is_empty(),
        "invalid mantissa on line {line_number}"
    );
    let significand = u64::from_str_radix(&format!("{whole}{fraction}"), 16)
        .unwrap_or_else(|_| panic!("invalid mantissa {mantissa:?} on line {line_number}"));
    let exponent: i32 = parse_number(exponent, line_number);
    let fractional_bits = i32::try_from(fraction.len() * 4).expect("hex precision fits i32");
    let magnitude = (significand as f64) * 2.0_f64.powi(exponent - fractional_bits);
    if negative { -magnitude } else { magnitude }
}

fn parse_number<T>(value: &str, line_number: usize) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    value
        .parse()
        .unwrap_or_else(|error| panic!("invalid number {value:?} on line {line_number}: {error:?}"))
}

fn parse_hex(value: &str, line_number: usize) -> u64 {
    assert_eq!(value.len(), 16, "hex width on line {line_number}");
    u64::from_str_radix(value, 16)
        .unwrap_or_else(|_| panic!("invalid hexadecimal {value:?} on line {line_number}"))
}

#[test]
fn scanner_slice_oracle_parser_freezes_exact_corpus_shape() {
    let oracle = ExpectedOracle::parse(ORACLE);
    let systems = oracle
        .pages
        .iter()
        .map(|page| page.systems.len())
        .sum::<usize>();
    let staffs = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .map(|system| system.staffs.len())
        .sum::<usize>();
    let scans = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.staffs)
        .flat_map(|staff| &staff.scans)
        .collect::<Vec<_>>();
    let competitor_candidates = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.competitor_candidates)
        .collect::<Vec<_>>();
    let bar_candidates = oracle
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .flat_map(|system| &system.bar_candidates)
        .collect::<Vec<_>>();

    let decisions = |decision| {
        competitor_candidates
            .iter()
            .filter(|candidate| candidate.decision == decision)
            .count()
    };
    let bar_decisions = |decision| {
        bar_candidates
            .iter()
            .filter(|candidate| candidate.decision == decision)
            .count()
    };
    let slice_totals = |select: fn(&ExpectedScan) -> &[usize]| {
        let nonempty = scans.iter().filter(|scan| !select(scan).is_empty()).count();
        let references = scans.iter().map(|scan| select(scan).len()).sum::<usize>();
        (nonempty, references)
    };

    assert_eq!(oracle.pages.len(), 8);
    assert_eq!(systems, 30);
    assert_eq!(staffs, 55);
    assert_eq!(scans.len(), 1_767);
    assert_eq!(competitor_candidates.len(), 1_334);
    assert_eq!(decisions(CompetitorDecision::Accept), 847);
    assert_eq!(decisions(CompetitorDecision::NotGood), 29);
    assert_eq!(decisions(CompetitorDecision::ThinVertical), 430);
    assert_eq!(decisions(CompetitorDecision::ShortBeamGroup), 28);
    assert_eq!(bar_candidates.len(), 533);
    assert_eq!(bar_decisions(BarDecision::Frozen), 474);
    assert_eq!(bar_decisions(BarDecision::NotFrozen), 59);
    assert_eq!(slice_totals(|scan| &scan.seeds), (1_455, 15_343));
    assert_eq!(slice_totals(|scan| &scan.spots), (1_455, 6_759));
    assert_eq!(slice_totals(|scan| &scan.competitors), (408, 1_944));
    assert_eq!(slice_totals(|scan| &scan.bars), (552, 5_060));
}

#[test]
fn native_seed_and_spot_slices_match_java_corpus_oracle() {
    let oracle = ExpectedOracle::parse(ORACLE);
    let mut compared_scans = 0_usize;
    let mut seed_nonempty = 0_usize;
    let mut seed_references = 0_usize;
    let mut spot_nonempty = 0_usize;
    let mut spot_references = 0_usize;

    for expected_page in &oracle.pages {
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
        let scanners =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| {
                    panic!("{}: scanner context failed: {error}", expected_page.key)
                });
        let actual = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
            .unwrap_or_else(|error| panic!("{}: scanner pools failed: {error}", expected_page.key));

        assert_eq!(actual.systems.len(), expected_page.systems.len());
        for (expected_system, actual_system) in expected_page.systems.iter().zip(&actual.systems) {
            let system_label = format!("{} system {}", expected_page.key, expected_system.id);
            assert_eq!(
                actual_system.system_id, expected_system.id,
                "{system_label} id"
            );
            let summary = expected_system.summary.as_ref().expect("system summary");
            assert_eq!(
                actual_system.seeds.len(),
                summary.seeds,
                "{system_label} seeds"
            );
            assert_eq!(
                actual_system.spots.len(),
                summary.spots,
                "{system_label} spots"
            );
            assert_eq!(actual_system.staffs.len(), expected_system.staffs.len());

            for (expected_staff, actual_staff) in
                expected_system.staffs.iter().zip(&actual_system.staffs)
            {
                let staff_label = format!("{system_label} staff {}", expected_staff.id);
                assert_eq!(actual_staff.staff_id, expected_staff.id, "{staff_label} id");
                assert_eq!(actual_staff.scans.len(), expected_staff.scans.len());
                for (expected_scan, actual_scan) in
                    expected_staff.scans.iter().zip(&actual_staff.scans)
                {
                    let scan_label = format!("{staff_label} scan {}", expected_scan.ordinal);
                    assert_eq!(
                        actual_scan.geometry_ordinal, expected_scan.ordinal,
                        "{scan_label} ordinal"
                    );
                    assert_band(
                        &actual_scan.seed_band,
                        &expected_scan.seed_band,
                        &scan_label,
                    );
                    assert_band(
                        &actual_scan.competitor_band,
                        &expected_scan.competitor_band,
                        &scan_label,
                    );
                    assert_band(&actual_scan.bar_band, &expected_scan.bar_band, &scan_label);
                    assert_eq!(
                        actual_scan.seed_band.integer_bounds(),
                        expected_rectangle(expected_scan.seeds_area),
                        "{scan_label} seed Area"
                    );
                    assert_eq!(
                        actual_scan.competitor_band.integer_bounds(),
                        expected_rectangle(expected_scan.competitors_area),
                        "{scan_label} competitor Area"
                    );
                    assert_eq!(
                        actual_scan.seed_indices, expected_scan.seeds,
                        "{scan_label} seed slice"
                    );
                    assert_eq!(
                        actual_scan.spot_indices, expected_scan.spots,
                        "{scan_label} spot slice"
                    );
                    compared_scans += 1;
                    seed_nonempty += usize::from(!actual_scan.seed_indices.is_empty());
                    seed_references += actual_scan.seed_indices.len();
                    spot_nonempty += usize::from(!actual_scan.spot_indices.is_empty());
                    spot_references += actual_scan.spot_indices.len();
                }
            }
        }
    }

    assert_eq!(compared_scans, 1_767);
    assert_eq!((seed_nonempty, seed_references), (1_455, 15_343));
    assert_eq!((spot_nonempty, spot_references), (1_455, 6_759));
}

fn assert_band(actual: &HeadScannerBand, expected: &Band, label: &str) {
    assert_eq!(
        actual.above().to_bits(),
        expected.above.bits,
        "{label} above"
    );
    assert_eq!(
        actual.effective_bottom().to_bits(),
        expected.effective_bottom.bits,
        "{label} effective bottom"
    );
}

fn expected_rectangle(expected: Rect) -> JavaRectangle {
    JavaRectangle::new(
        expected.left,
        expected.top,
        i32::try_from(expected.width).expect("oracle rectangle width fits Java int"),
        i32::try_from(expected.height).expect("oracle rectangle height fits Java int"),
    )
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
