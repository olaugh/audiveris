// SPDX-License-Identifier: AGPL-3.0-or-later

//! Grades production native HEADERS against Java on the whole corpus.
//!
//! The production call accepts only a live GRID result. Java records are read afterwards and are
//! used solely to grade the selected clef/key/time state and the downstream header erases.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audiveris_omr::header_time_column::NeutralSpecificTimeShape;
use audiveris_omr::recognize::recognize_grid_lines;

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

#[derive(Debug, Clone)]
struct OracleStaff {
    id: usize,
    specific_interline: i32,
    header_start: i32,
    /// The final `header.stop` after all three columns -- what `getHeaderStop()` returns and
    /// what the BEAMS/STEM_SEEDS header erase reads.
    header_stop: i32,
    /// `None` where Java found no key on this staff, which the port must also produce.
    key: Option<OracleKey>,
    /// `None` where Java found no time signature, which the port must also produce.
    time: Option<OracleTime>,
}

#[derive(Debug, Clone, PartialEq)]
struct OracleTime {
    specific: Option<NeutralSpecificTimeShape>,
    numerator: i32,
    denominator: i32,
    bounds: (i32, i32, i32, i32),
    stop: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
struct OracleKey {
    fifths: i32,
    bounds: (i32, i32, i32, i32),
    stop: Option<i32>,
}

fn parse_oracle() -> Vec<(String, i32, Vec<OracleStaff>)> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/clef-headers.txt"))
        .expect("the clef/key oracle is checked in");
    let mut pages: Vec<(String, i32, Vec<OracleStaff>)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("page") => pages.push((f[1].to_owned(), f[2].parse().unwrap(), Vec::new())),
            Some("staff") => {
                let page = pages.last_mut().expect("a staff row follows a page row");
                page.2.push(OracleStaff {
                    id: f[1].parse().unwrap(),
                    specific_interline: f[2].parse().unwrap(),
                    header_start: f[3].parse().unwrap(),
                    header_stop: f[4].parse().unwrap(),
                    key: None,
                    time: None,
                });
            }
            Some("key") => {
                let page = pages.last_mut().expect("a key row follows a page row");
                let id: usize = f[1].parse().unwrap();
                let staff = page
                    .2
                    .iter_mut()
                    .find(|staff| staff.id == id)
                    .expect("a key row names a staff already seen");
                if f[2] != "NONE" {
                    staff.key = Some(OracleKey {
                        fifths: f[2].parse().unwrap(),
                        bounds: (
                            f[4].parse().unwrap(),
                            f[5].parse().unwrap(),
                            f[6].parse().unwrap(),
                            f[7].parse().unwrap(),
                        ),
                        stop: f[8].parse().ok(),
                    });
                }
            }
            Some("time") => {
                let page = pages.last_mut().expect("a time row follows a page row");
                let id: usize = f[1].parse().unwrap();
                let staff = page
                    .2
                    .iter_mut()
                    .find(|staff| staff.id == id)
                    .expect("a time row names a staff already seen");
                if f[2] != "NONE" {
                    let specific = match f[2] {
                        "COMMON_TIME" => Some(NeutralSpecificTimeShape::Common),
                        "CUT_TIME" => Some(NeutralSpecificTimeShape::Cut),
                        _ => None,
                    };
                    let (numerator, denominator) = f[3]
                        .split_once('/')
                        .map(|(n, d)| (n.parse().unwrap(), d.parse().unwrap()))
                        .expect("a rational like 2/4");
                    staff.time = Some(OracleTime {
                        specific,
                        numerator,
                        denominator,
                        bounds: (
                            f[5].parse().unwrap(),
                            f[6].parse().unwrap(),
                            f[7].parse().unwrap(),
                            f[8].parse().unwrap(),
                        ),
                        stop: f[9].parse().ok(),
                    });
                }
            }
            _ => {}
        }
    }
    pages
}

/// One `erase` row: `(system, x, stop, first line y, last line y)`.
type EraseRow = (usize, i32, i32, i32, i32);

struct NativeHeaderPage {
    name: String,
    recognition: audiveris_omr::recognize::GridLinesRecognition,
    header_erases: Vec<audiveris_image::spots::HeaderErase>,
}

/// The `erase system <s> x <x> stop <x2> firstline <y1> lastline <y2>` rows of the beam oracle,
/// keyed by page name: what Java's `SpotsBuilder.eraseHeaderAreas` actually erased.
fn parse_beam_erases() -> BTreeMap<String, Vec<EraseRow>> {
    let text = std::fs::read_to_string(repo_path("rust/oracle/beam-spots.txt"))
        .expect("the beam-spots oracle is checked in");
    let mut map: BTreeMap<String, Vec<EraseRow>> = BTreeMap::new();
    let mut page = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("sheet ") {
            page = rest.split('#').next().unwrap_or("").trim().to_owned();
        } else if line.starts_with("erase ") {
            let f: Vec<&str> = line.split_whitespace().collect();
            map.entry(page.clone()).or_default().push((
                f[2].parse().unwrap(),
                f[4].parse().unwrap(),
                f[6].parse().unwrap(),
                f[8].parse().unwrap(),
                f[10].parse().unwrap(),
            ));
        }
    }
    map
}

fn run_native_headers() -> Vec<NativeHeaderPage> {
    let pages = parse_oracle();
    let beam_erases = parse_beam_erases();
    let mut checked = 0;
    let mut with_key = 0;
    let mut with_time = 0;
    let mut erases_checked = 0;
    let mut mismatches = Vec::new();
    let mut native_pages = Vec::new();

    for (name, expected_interline, oracle_staves) in pages {
        let recognition = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
            .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));
        // This is the only input to production HEADERS. Everything below this call is grading.
        let native = audiveris_omr::native_headers::recognize_native_headers(&recognition)
            .unwrap_or_else(|error| panic!("{name}: native HEADERS failed: {error}"));
        if native.sheet_interline != expected_interline {
            mismatches.push(format!(
                "{name}: sheet interline {}, Java {expected_interline}",
                native.sheet_interline
            ));
        }

        for oracle in &oracle_staves {
            checked += 1;
            let produced = native
                .systems
                .iter()
                .flat_map(|system| &system.staffs)
                .find(|staff| staff.staff_id == oracle.id)
                .unwrap_or_else(|| panic!("{name}: native result omitted staff {}", oracle.id));
            let header = produced
                .header
                .as_ref()
                .unwrap_or_else(|| panic!("{name}: staff {} has no native header", oracle.id));
            if produced.specific_interline != oracle.specific_interline {
                mismatches.push(format!(
                    "{name} staff {}: specific interline {}, Java {}",
                    oracle.id, produced.specific_interline, oracle.specific_interline
                ));
            }
            if header.start != oracle.header_start {
                mismatches.push(format!(
                    "{name} staff {}: header start {}, Java {}",
                    oracle.id, header.start, oracle.header_start
                ));
            }
            if header.stop != oracle.header_stop {
                mismatches.push(format!(
                    "{name} staff {}: header stop {}, Java {}",
                    oracle.id, header.stop, oracle.header_stop
                ));
            }

            let key_bounds = header.key.as_ref().map(|key| {
                (
                    key.bounds.x,
                    key.bounds.y,
                    key.bounds.width,
                    key.bounds.height,
                )
            });
            match (&oracle.key, key_bounds) {
                (None, None) => {}
                (None, Some(bounds)) => mismatches.push(format!(
                    "{name} staff {}: produced a key at {bounds:?}, Java found none",
                    oracle.id
                )),
                (Some(expected), None) => {
                    with_key += 1;
                    mismatches.push(format!(
                        "{name} staff {}: no key, Java found {} fifths at {:?}",
                        oracle.id, expected.fifths, expected.bounds
                    ));
                }
                (Some(expected), Some(bounds)) => {
                    with_key += 1;
                    if bounds != expected.bounds {
                        mismatches.push(format!(
                            "{name} staff {}: key box {bounds:?}, Java {:?}",
                            oracle.id, expected.bounds
                        ));
                    }
                    let stop = bounds.0 + bounds.2 - 1;
                    if Some(stop) != expected.stop {
                        mismatches.push(format!(
                            "{name} staff {}: keyStop {stop}, Java {:?}",
                            oracle.id, expected.stop
                        ));
                    }
                    let fifths = produced
                        .selected_key_id
                        .and_then(|id| produced.key_candidates.iter().find(|key| key.id == id))
                        .map(|key| i32::from(key.fifths));
                    if fifths != Some(expected.fifths) {
                        mismatches.push(format!(
                            "{name} staff {}: selected key evidence {fifths:?}, Java {}",
                            oracle.id, expected.fifths
                        ));
                    }
                }
            }

            let time_bounds = header.time.as_ref().map(|time| {
                (
                    time.bounds.x,
                    time.bounds.y,
                    time.bounds.width,
                    time.bounds.height,
                )
            });
            match (&oracle.time, time_bounds) {
                (None, None) => {}
                (None, Some(bounds)) => mismatches.push(format!(
                    "{name} staff {}: produced a time at {bounds:?}, Java found none",
                    oracle.id
                )),
                (Some(expected), None) => {
                    with_time += 1;
                    mismatches.push(format!(
                        "{name} staff {}: no time, Java found {}/{} at {:?}",
                        oracle.id, expected.numerator, expected.denominator, expected.bounds
                    ));
                }
                (Some(expected), Some(bounds)) => {
                    with_time += 1;
                    if bounds != expected.bounds {
                        mismatches.push(format!(
                            "{name} staff {}: time box {bounds:?}, Java {:?}",
                            oracle.id, expected.bounds
                        ));
                    }
                    let stop = bounds.0 + bounds.2 - 1;
                    if Some(stop) != expected.stop {
                        mismatches.push(format!(
                            "{name} staff {}: timeStop {stop}, Java {:?}",
                            oracle.id, expected.stop
                        ));
                    }
                    let value = produced
                        .selected_time_id
                        .and_then(|id| produced.time_candidates.iter().find(|time| time.id == id))
                        .map(|time| time.value);
                    let matches = value.is_some_and(|value| {
                        value.specific_shape == expected.specific
                            && value.numerator == expected.numerator
                            && value.denominator == expected.denominator
                    });
                    if !matches {
                        mismatches.push(format!(
                            "{name} staff {}: selected time evidence {value:?}, Java {:?}/{}/{}",
                            oracle.id, expected.specific, expected.numerator, expected.denominator
                        ));
                    }
                }
            }
        }

        let margin = (2.0 * f64::from(expected_interline)).round_ties_even() as i32;
        if let Some(expected_rows) = beam_erases.get(&name) {
            for &(system_id, x, stop, first, last) in expected_rows {
                let Some(erase) = native
                    .header_erases
                    .iter()
                    .find(|item| item.system_id == system_id)
                    .map(|item| item.erase)
                else {
                    mismatches.push(format!("{name} system {system_id}: no header erase"));
                    continue;
                };
                erases_checked += 1;
                let actual = (
                    erase.x,
                    erase.stop,
                    erase.top + margin,
                    erase.bottom - margin,
                );
                let expected = (x, stop, first, last);
                if actual != expected {
                    mismatches.push(format!(
                        "{name} system {system_id}: header erase {actual:?}, Java {expected:?}"
                    ));
                }
            }
        }

        native_pages.push(NativeHeaderPage {
            name,
            recognition,
            header_erases: native.beam_erases(),
        });
    }

    assert_eq!(checked, 65, "every oracle staff was compared");
    assert_eq!(with_key, 34, "the 34 key-bearing staves were reached");
    assert_eq!(with_time, 17, "the 17 time-bearing staves were reached");
    assert_eq!(
        erases_checked,
        beam_erases.values().map(Vec::len).sum::<usize>(),
        "every Java header-erase rectangle was compared against the native one"
    );
    assert!(
        mismatches.is_empty(),
        "{} of {checked} staves disagree with Java:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    native_pages
}

#[test]
fn native_headers_match_java_on_every_corpus_staff() {
    assert_eq!(
        run_native_headers().len(),
        9,
        "every corpus page ran through GRID and HEADERS"
    );
}

/// Records grouped by the `sheet` row that starts each oracle segment.
fn beam_oracle_pages(text: &'static str) -> Vec<(String, Vec<Vec<&'static str>>)> {
    let mut pages: Vec<(String, Vec<Vec<&'static str>>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields[0] == "sheet" {
            pages.push((fields[1].to_owned(), Vec::new()));
        }
        pages
            .last_mut()
            .expect("an oracle record follows a sheet row")
            .1
            .push(fields);
    }
    pages
}

fn raw_beam_key(system_id: usize, raw: audiveris_omr::beam_inters::RawBeam) -> String {
    format!(
        "{system_id} {} {} {:.9} {:.9} {:.9} {:.9} {:.9} grade {:.9} impacts \
         wdth {:.9} minH {:.9} maxH {:.9} core {:.9} belt {:.9} jit {:.9}",
        raw.kind.class_name(),
        raw.kind.shape(),
        raw.item.median.x1,
        raw.item.median.y1,
        raw.item.median.x2,
        raw.item.median.y2,
        raw.item.height,
        raw.grade,
        raw.impacts.width,
        raw.impacts.min_height,
        raw.impacts.max_height,
        raw.impacts.core,
        raw.impacts.belt,
        raw.impacts.distance,
    )
}

fn multiset_difference<T: Clone + PartialEq>(mine: &[T], theirs: &[T]) -> (Vec<T>, Vec<T>) {
    let mut spurious = mine.to_vec();
    for expected in theirs {
        if let Some(index) = spurious.iter().position(|actual| actual == expected) {
            spurious.remove(index);
        }
    }
    let mut missing = theirs.to_vec();
    for actual in mine {
        if let Some(index) = missing.iter().position(|expected| expected == actual) {
            missing.remove(index);
        }
    }
    (spurious, missing)
}

/// A final beam keyed by system, class, bounds, grade, and all six impacts.
fn expected_final_beams(records: &[Vec<&'static str>]) -> Vec<(usize, String, Vec<String>)> {
    records
        .iter()
        .filter(|fields| fields.first() == Some(&"inter"))
        .filter(|fields| matches!(fields[3], "BeamInter" | "BeamHookInter" | "SmallBeamInter"))
        .map(|fields| {
            let bounds = fields
                .iter()
                .position(|field| *field == "bounds")
                .expect("a beam has bounds");
            let grade = fields
                .iter()
                .position(|field| *field == "grade")
                .expect("a beam has a grade");
            let impacts = fields
                .iter()
                .position(|field| *field == "impacts")
                .expect("a beam has impacts");
            let mut evidence = fields[bounds + 1..bounds + 5]
                .iter()
                .map(|field| (*field).to_owned())
                .collect::<Vec<_>>();
            evidence.extend(["grade".to_owned(), fields[grade + 1].to_owned()]);
            evidence.extend(
                fields[impacts + 1..impacts + 13]
                    .iter()
                    .map(|field| (*field).to_owned()),
            );
            (
                fields[1].parse().expect("a system id"),
                fields[3].to_owned(),
                evidence,
            )
        })
        .collect()
}

fn produced_final_beams(
    recognition: &audiveris_omr::recognize::NativeBeamRecognition,
) -> Vec<(usize, String, Vec<String>)> {
    recognition
        .raw_beams
        .iter()
        .chain(&recognition.hooks)
        .map(|(system_id, raw)| {
            let bounds = audiveris_omr::beam_inters::beam_bounds(raw.item);
            (
                *system_id,
                raw.kind.class_name().to_owned(),
                vec![
                    bounds.x.to_string(),
                    bounds.y.to_string(),
                    bounds.width.to_string(),
                    bounds.height.to_string(),
                    "grade".to_owned(),
                    format!("{:.9}", raw.grade),
                    "wdth".to_owned(),
                    format!("{:.9}", raw.impacts.width),
                    "minH".to_owned(),
                    format!("{:.9}", raw.impacts.min_height),
                    "maxH".to_owned(),
                    format!("{:.9}", raw.impacts.max_height),
                    "core".to_owned(),
                    format!("{:.9}", raw.impacts.core),
                    "belt".to_owned(),
                    format!("{:.9}", raw.impacts.belt),
                    "jit".to_owned(),
                    format!("{:.9}", raw.impacts.distance),
                ],
            )
        })
        .collect()
}

fn ledger_oracle_summaries(input: &str, record_name: &str) -> BTreeMap<String, (usize, String)> {
    input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.first() == Some(&record_name)).then(|| {
                (
                    fields[1].to_owned(),
                    (
                        fields[2].parse().expect("a ledger count"),
                        fields[3].to_owned(),
                    ),
                )
            })
        })
        .collect()
}

fn produced_final_ledgers(
    recognition: &audiveris_omr::native_ledgers::NativeLedgerRecognition,
) -> Vec<String> {
    recognition
        .ledgers()
        .iter()
        .map(|ledger| {
            format!(
                "ledger {} {} {} {:.9} {:.9} {:.9} {:.9} {:.9} {:.9} {}",
                ledger.system_id,
                ledger.staff_id,
                ledger.ledger_index,
                ledger.median.0.0,
                ledger.median.0.1,
                ledger.median.1.0,
                ledger.median.1.1,
                ledger.thickness,
                ledger.grade,
                ledger
                    .impacts
                    .iter()
                    .map(|impact| format!("{:.9}", impact.grade))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        })
        .collect()
}

fn produced_final_ledger_lines(
    recognition: &audiveris_omr::native_ledgers::NativeLedgerRecognition,
) -> Vec<String> {
    recognition
        .ledger_lines
        .iter()
        .map(|line| {
            format!(
                "ledgerline {} {} {} {:.9}",
                line.system_id, line.staff_id, line.index, line.translation_y
            )
        })
        .collect()
}

#[test]
fn native_grid_headers_and_beams_match_java_on_every_beam_sheet() {
    let structures = beam_oracle_pages(include_str!("../../../oracle/beam-structures.txt"));
    let spots = beam_oracle_pages(include_str!("../../../oracle/beam-spots.txt"));
    let sig = beam_oracle_pages(include_str!("../../../oracle/beams-sig.txt"));
    let ledger_oracle = include_str!("../../../oracle/ledgers-corpus.txt");
    let ledger_summaries = ledger_oracle_summaries(ledger_oracle, "ledgersummary");
    let ledger_line_summaries = ledger_oracle_summaries(ledger_oracle, "ledgerlinesummary");
    let native_pages = run_native_headers();

    assert_eq!(structures.len(), 8, "eight beam sheets are pinned");
    assert_eq!(ledger_summaries.len(), 8, "eight ledger sheets are pinned");
    assert_eq!(
        ledger_line_summaries.len(),
        8,
        "eight ledger-line sheets are pinned"
    );
    let mut ungraded_small = native_pages[0].recognition.clone();
    ungraded_small.scale.scale.small_beam = Some(audiveris_image::scale_estimate::BeamScale {
        main: 7,
        extrapolated: false,
    });
    assert!(matches!(
        audiveris_omr::recognize::recognize_native_beams(
            &ungraded_small,
            native_pages[0].header_erases.clone(),
        ),
        Err(
            audiveris_omr::recognize::NativeBeamRecognitionError::UnsupportedSmallBeam {
                small: 7,
                ..
            }
        )
    ));
    let mut checked = 0;
    let mut raw_checked = 0;
    let mut spots_checked = 0;
    let mut erases_checked = 0;
    let mut ledgers_checked = 0;
    let mut ledger_lines_checked = 0;
    let mut failures = Vec::new();
    let mut ledger_failures = Vec::new();

    for (page, structure_records) in &structures {
        let file = page.split('#').next().expect("a file name");
        let native = native_pages
            .iter()
            .find(|native| native.name == file)
            .unwrap_or_else(|| panic!("no native HEADERS result for {page}"));
        let spot_records = &spots
            .iter()
            .find(|(name, _)| name == page)
            .unwrap_or_else(|| panic!("no spot oracle for {page}"))
            .1;
        let sig_records = &sig
            .iter()
            .find(|(name, _)| name == page)
            .unwrap_or_else(|| panic!("no SIG oracle for {page}"))
            .1;

        let produced = audiveris_omr::recognize::recognize_native_beams(
            &native.recognition,
            native.header_erases.clone(),
        )
        .unwrap_or_else(|error| panic!("{page}: native BEAMS failed: {error}"));

        let ledgers =
            audiveris_omr::native_ledgers::recognize_native_ledgers(&native.recognition, &produced)
                .unwrap_or_else(|error| panic!("{page}: native LEDGERS failed: {error}"));
        let mut actual_ledgers = produced_final_ledgers(&ledgers);
        actual_ledgers.sort();
        let canonical = actual_ledgers
            .iter()
            .map(|ledger| format!("{ledger}\n"))
            .collect::<String>();
        let actual_summary = (
            actual_ledgers.len(),
            format!(
                "{:016x}",
                audiveris_image::ingest::fnv1a64_bytes(canonical.as_bytes())
            ),
        );
        ledgers_checked += actual_ledgers.len();
        let expected_summary = ledger_summaries
            .get(page)
            .unwrap_or_else(|| panic!("no ledger summary oracle for {page}"));
        if &actual_summary != expected_summary {
            ledger_failures.push(format!(
                "{page}: native {actual_summary:?}, Java {expected_summary:?}; {} candidates, {} builder survivors, {} direct rejects, rebuilt {:?}",
                ledgers.candidates.len(),
                ledgers.builder_survivor_count,
                ledgers.discarded_filament_ids.len(),
                ledgers.rebuilt_system_ids,
            ));
        }

        let mut actual_lines = produced_final_ledger_lines(&ledgers);
        actual_lines.sort();
        let canonical_lines = actual_lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        let actual_line_summary = (
            actual_lines.len(),
            format!(
                "{:016x}",
                audiveris_image::ingest::fnv1a64_bytes(canonical_lines.as_bytes())
            ),
        );
        ledger_lines_checked += actual_lines.len();
        let expected_line_summary = ledger_line_summaries
            .get(page)
            .unwrap_or_else(|| panic!("no ledger-line summary oracle for {page}"));
        if &actual_line_summary != expected_line_summary {
            ledger_failures.push(format!(
                "{page}: native ledger lines {actual_line_summary:?}, Java {expected_line_summary:?}"
            ));
        }

        if file == "chula.png" {
            assert_eq!(ledgers.filtered_run_count, 9_915);
            assert_eq!(ledgers.section_count, 4_052);
            assert_eq!(
                ledgers.system_section_counts,
                vec![(1, 2_039), (2, 577), (3, 961)]
            );
            assert_eq!(ledgers.registered_filament_count, 104);
            assert_eq!(ledgers.candidates.len(), 104);
            assert_eq!(ledgers.builder_survivor_count, 19);
            assert_eq!(ledgers.discarded_filament_ids.len(), 1);
            assert_eq!(ledgers.rebuilt_system_ids, vec![1]);

            let expected = include_str!("../../../oracle/ledgers-chula.txt")
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let missing = expected
                .iter()
                .filter(|ledger| !actual_ledgers.contains(ledger))
                .collect::<Vec<_>>();
            assert!(
                missing.is_empty(),
                "{page}: native builder missed exact Java ledgers {missing:?}"
            );
            assert_eq!(actual_ledgers.len(), expected.len());
        }

        let expected_spots = spot_records
            .iter()
            .find(|fields| fields.first() == Some(&"spotcount"))
            .expect("a spot count")[1]
            .parse::<usize>()
            .expect("a numeric spot count");
        assert_eq!(produced.spot_count, expected_spots, "{page}: spot count");
        spots_checked += produced.spot_count;
        erases_checked += native.header_erases.len();

        let mut expected_raw = structure_records
            .iter()
            .filter(|fields| fields.first() == Some(&"rawbeam"))
            .map(|fields| fields[1..].join(" "))
            .collect::<Vec<_>>();
        let mut actual_raw = produced
            .raw_beams
            .iter()
            .map(|(system_id, raw)| raw_beam_key(*system_id, *raw))
            .collect::<Vec<_>>();
        expected_raw.sort();
        actual_raw.sort();
        raw_checked += actual_raw.len();
        if actual_raw != expected_raw {
            let (spurious, missing) = multiset_difference(&actual_raw, &expected_raw);
            let differing = spurious.len().max(missing.len());
            failures.push(format!(
                "{page}: {differing} raw beams differ ({} vs Java {})",
                actual_raw.len(),
                expected_raw.len()
            ));
        }

        let expected = expected_final_beams(sig_records);
        let actual = produced_final_beams(&produced);
        for kind in ["BeamInter", "BeamHookInter", "SmallBeamInter"] {
            let mut theirs = expected
                .iter()
                .filter(|(_, class, _)| class == kind)
                .cloned()
                .collect::<Vec<_>>();
            let mut mine = actual
                .iter()
                .filter(|(_, class, _)| class == kind)
                .cloned()
                .collect::<Vec<_>>();
            theirs.sort();
            mine.sort();
            if mine.len() != theirs.len() {
                failures.push(format!(
                    "{page}: {kind} count {} vs Java {}",
                    mine.len(),
                    theirs.len()
                ));
            }
            if mine != theirs {
                let (spurious, missing) = multiset_difference(&mine, &theirs);
                let differing = spurious.len().max(missing.len());
                failures.push(format!(
                    "{page}: {kind} -- {differing} of {} differ",
                    theirs.len()
                ));
            }
        }

        let mut expected_groups = BTreeMap::new();
        for fields in sig_records
            .iter()
            .filter(|fields| fields.first() == Some(&"inter") && fields[3] == "BeamGroupInter")
        {
            *expected_groups
                .entry(fields[1].parse::<usize>().expect("a system id"))
                .or_insert(0usize) += 1;
        }
        let mut actual_groups = produced
            .group_counts
            .iter()
            .copied()
            .collect::<BTreeMap<_, _>>();
        for bounds in &native.recognition.system_bounds {
            expected_groups.entry(bounds.system_id).or_insert(0);
            actual_groups.entry(bounds.system_id).or_insert(0);
        }
        if actual_groups != expected_groups {
            failures.push(format!(
                "{page}: beam groups {actual_groups:?} vs Java {expected_groups:?}"
            ));
        }

        checked += 1;
    }

    assert_eq!(checked, 8, "every beam sheet was compared");
    assert_eq!(raw_checked, 787, "all raw beam records were compared");
    assert_eq!(
        spots_checked, 2_739,
        "all native spot components were counted"
    );
    assert_eq!(erases_checked, 30, "all native header erases reached BEAMS");
    assert_eq!(
        ledgers_checked, 581,
        "all final Java ledger inters were compared"
    );
    assert_eq!(
        ledger_lines_checked, 95,
        "all final Java inferred ledger lines were compared"
    );
    assert!(
        ledger_failures.is_empty(),
        "native end-to-end ledger divergences:\n{}",
        ledger_failures.join("\n")
    );

    // `MultipleRestsBuilder` runs after beam recognition and replaces one long
    // BachInvention5 beam with a MultipleRestInter. The native beam result is
    // therefore expected to retain exactly this one source beam until that
    // separate recognizer is wired.
    let known = [
        "BachInvention5.jpg#1: BeamInter count 193 vs Java 192",
        "BachInvention5.jpg#1: BeamInter -- 1 of 192 differ",
    ];
    let unexpected = failures
        .iter()
        .filter(|failure| !known.contains(&failure.as_str()))
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "new native end-to-end beam divergences:\n{}",
        unexpected
            .iter()
            .map(|failure| failure.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        failures.len(),
        known.len(),
        "a known divergence disappeared; remove it deliberately:\n{}",
        failures.join("\n")
    );
}
