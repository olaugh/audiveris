// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page Java differential for post-aggregation range-head composition.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use audiveris_omr::{
    head_template::{HeadTemplateBounds, HeadTemplateShape},
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_prolog,
    native_heads_bar_slices::materialize_native_head_scanner_bar_slices,
    native_heads_competitor_slices::materialize_native_head_scanner_competitor_slices,
    native_heads_competitors::materialize_native_heads_competitors,
    native_heads_obstacles::materialize_native_heads_bar_obstacles,
    native_heads_range_glyphs::{
        NativeHeadRangePostOutcome, NativeHeadsRangeGlyphsInput, retrieve_native_heads_range_glyphs,
    },
    native_heads_range_lookup::{NativeHeadsRangeLookupInput, recognize_native_heads_range_lookup},
    native_heads_scanner::recognize_native_heads_scanner_context,
    native_heads_scanner_pools::materialize_native_head_scanner_pools,
    native_heads_seed_glyphs::{NativeHeadsSeedGlyphsInput, retrieve_native_heads_seed_glyphs},
    native_heads_seed_lookup::{NativeHeadsSeedLookupInput, recognize_native_heads_seed_lookup},
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
};

const ORACLE: &str = include_str!("../../../oracle/heads-range-pass.txt");

type StaffKey = (String, usize, usize);
type SeedKey = (String, usize, usize);

#[derive(Debug)]
struct ExpectedStaff {
    seed_heads: usize,
    candidates: usize,
    seed_conflicts: usize,
    glyph_empty: usize,
    heads: usize,
    candidate_rows: Vec<ExpectedCandidate>,
    head_rows: Vec<ExpectedHead>,
}

#[derive(Debug)]
struct ExpectedCandidate {
    ordinal: usize,
    raw: usize,
    attempt: usize,
    scanner: usize,
    x: i32,
    y: i32,
    original_shape: HeadTemplateShape,
    shape: HeadTemplateShape,
    selected: [i32; 4],
    distance_bits: u64,
    black_to_void: Option<bool>,
    slim: HeadTemplateBounds,
    pitch_bits: u64,
    grade_bits: u64,
    distance_impact_bits: u64,
    impact_weight_bits: u64,
    aggregate: usize,
    members: Vec<usize>,
    conflicts: Vec<ExpectedConflict>,
    glyph: Option<ExpectedGlyph>,
    final_ordinal: Option<usize>,
    outcome: &'static str,
}

#[derive(Debug)]
struct ExpectedConflict {
    slice_ordinal: usize,
    live_ordinal: usize,
    sig_id: usize,
    bounds: HeadTemplateBounds,
    grade_bits: u64,
}

#[derive(Clone, Copy, Debug)]
struct ExpectedSeed {
    staff_id: usize,
    ordinal: usize,
    live_ordinal: usize,
    bounds: HeadTemplateBounds,
    grade_bits: u64,
}

#[derive(Clone, Copy, Debug)]
struct ExpectedGlyph {
    template_bounds: HeadTemplateBounds,
    foreground_bounds: HeadTemplateBounds,
    glyph_bounds: HeadTemplateBounds,
    weight: usize,
    run_digest: u64,
}

#[derive(Debug)]
struct ExpectedHead {
    ordinal: usize,
    candidate: usize,
    raw: usize,
    attempt: usize,
    scanner: usize,
    shape: HeadTemplateShape,
    pitch_bits: u64,
    bounds: HeadTemplateBounds,
    grade_bits: u64,
    distance_impact_bits: u64,
    impact_weight_bits: u64,
    good: bool,
    glyph_bounds: HeadTemplateBounds,
    glyph_weight: usize,
    glyph_run_digest: u64,
}

#[test]
fn native_range_aggregation_conflicts_and_glyphs_match_java_corpus() {
    let (mut expected, seeds) = parse_oracle();
    let pages = expected
        .keys()
        .map(|(page, _, _)| page.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(pages.len(), 8);
    assert_eq!(expected.len(), 55);

    let mut totals = [0_usize; 4];
    for page in pages {
        let image = page.strip_suffix("#1").unwrap();
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let obstacles = materialize_native_heads_bar_obstacles(&grid)
            .unwrap_or_else(|error| panic!("{page}: obstacles failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
        let competitors = materialize_native_heads_competitors(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page}: competitors failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads_prolog(&grid, &beams, &ledgers, &stem_seeds)
            .unwrap_or_else(|error| panic!("{page}: HEADS prolog failed: {error}"));
        let scanners =
            recognize_native_heads_scanner_context(&grid, &headers, &ledgers, &stem_seeds, &heads)
                .unwrap_or_else(|error| panic!("{page}: scanner context failed: {error}"));
        let pools = materialize_native_head_scanner_pools(&stem_seeds, &heads, &scanners)
            .unwrap_or_else(|error| panic!("{page}: scanner pools failed: {error}"));
        let bar_slices = materialize_native_head_scanner_bar_slices(&pools, &obstacles)
            .unwrap_or_else(|error| panic!("{page}: bar slices failed: {error}"));
        let competitor_slices =
            materialize_native_head_scanner_competitor_slices(&pools, &competitors)
                .unwrap_or_else(|error| panic!("{page}: competitor slices failed: {error}"));
        let seed_lookup = recognize_native_heads_seed_lookup(NativeHeadsSeedLookupInput {
            heads: &heads,
            scanners: &scanners,
            pools: &pools,
            obstacles: &obstacles,
            bar_slices: &bar_slices,
            competitors: &competitors,
            competitor_slices: &competitor_slices,
            stem_seeds: &stem_seeds,
        })
        .unwrap_or_else(|error| panic!("{page}: seed lookup failed: {error}"));
        let seed_glyphs = retrieve_native_heads_seed_glyphs(NativeHeadsSeedGlyphsInput {
            grid: &grid,
            heads: &heads,
            lookup: &seed_lookup,
        })
        .unwrap_or_else(|error| panic!("{page}: seed glyphs failed: {error}"));
        let range_lookup = recognize_native_heads_range_lookup(NativeHeadsRangeLookupInput {
            heads: &heads,
            scanners: &scanners,
            pools: &pools,
            obstacles: &obstacles,
            bar_slices: &bar_slices,
            competitors: &competitors,
            competitor_slices: &competitor_slices,
        })
        .unwrap_or_else(|error| panic!("{page}: range lookup failed: {error}"));
        let actual = retrieve_native_heads_range_glyphs(NativeHeadsRangeGlyphsInput {
            grid: &grid,
            heads: &heads,
            scanners: &scanners,
            pools: &pools,
            competitors: &competitors,
            range_lookup: &range_lookup,
            seed_glyphs: &seed_glyphs,
        })
        .unwrap_or_else(|error| panic!("{page}: range glyphs failed: {error}"));

        for system in &actual.systems {
            for staff in &system.staffs {
                let key = (page.clone(), system.system_id, staff.staff_id);
                let oracle = expected
                    .remove(&key)
                    .unwrap_or_else(|| panic!("missing {key:?}"));
                let label = format!(
                    "{page} system {} staff {}",
                    system.system_id, staff.staff_id
                );
                assert_eq!(staff.seed_head_count, oracle.seed_heads, "{label} seeds");
                assert_eq!(
                    staff.candidate_count, oracle.candidates,
                    "{label} candidates"
                );
                assert_eq!(
                    staff.seed_conflict_count, oracle.seed_conflicts,
                    "{label} conflicts"
                );
                assert_eq!(staff.glyph_empty_count, oracle.glyph_empty, "{label} empty");
                assert_eq!(staff.heads.len(), oracle.heads, "{label} heads");
                assert_candidates(
                    staff,
                    &oracle.candidate_rows,
                    &seeds,
                    &page,
                    system.system_id,
                    &label,
                );
                assert_heads(staff, &oracle.head_rows, &label);
                totals[0] += staff.candidate_count;
                totals[1] += staff.seed_conflict_count;
                totals[2] += staff.glyph_empty_count;
                totals[3] += staff.heads.len();
            }
        }
    }
    assert!(
        expected.is_empty(),
        "unmatched staff summaries: {}",
        expected.len()
    );
    assert_eq!(totals, [3_550, 3_376, 0, 174]);
}

fn assert_candidates(
    staff: &audiveris_omr::native_heads_range_glyphs::NativeHeadsRangeGlyphStaff,
    expected: &[ExpectedCandidate],
    seeds: &BTreeMap<SeedKey, ExpectedSeed>,
    page: &str,
    system_id: usize,
    label: &str,
) {
    assert_eq!(staff.candidates.len(), expected.len(), "{label}");
    for (actual, expected) in staff.candidates.iter().zip(expected) {
        let raw = &actual.raw;
        let item = format!("{label} candidate {}", expected.ordinal);
        assert_eq!(actual.ordinal, expected.ordinal, "{item} ordinal");
        assert_eq!(raw.ordinal, expected.raw, "{item} raw");
        assert_eq!(raw.attempt_ordinal, expected.attempt, "{item} attempt");
        assert_eq!(raw.geometry_ordinal, expected.scanner, "{item} scanner");
        assert_eq!((raw.x0, raw.y0), (expected.x, expected.y), "{item} origin");
        assert_eq!(
            raw.original_shape, expected.original_shape,
            "{item} original shape"
        );
        assert_eq!(raw.shape, expected.shape, "{item} shape");
        assert_eq!(
            [
                raw.best.y_offset_ordinal as i32,
                raw.best.y_offset,
                raw.best.x,
                raw.best.y
            ],
            expected.selected,
            "{item} selected"
        );
        assert_eq!(
            raw.best.distance.to_bits(),
            expected.distance_bits,
            "{item} distance"
        );
        assert_eq!(
            raw.hole.map(|hole| hole.converted_to_void),
            expected.black_to_void,
            "{item} black-to-void"
        );
        assert_eq!(raw.bounds, expected.slim, "{item} slim");
        assert_eq!(raw.pitch.to_bits(), expected.pitch_bits, "{item} pitch");
        assert_eq!(raw.grade.to_bits(), expected.grade_bits, "{item} grade");
        assert_eq!(
            raw.distance_impact.to_bits(),
            expected.distance_impact_bits,
            "{item} impact"
        );
        assert_eq!(
            1.0_f64.to_bits(),
            expected.impact_weight_bits,
            "{item} weight impact"
        );
        assert_eq!(
            actual.aggregate_ordinal, expected.aggregate,
            "{item} aggregate"
        );
        assert_eq!(
            actual.member_raw_ordinals, expected.members,
            "{item} members"
        );
        assert_eq!(
            actual.seed_conflicts.len(),
            expected.conflicts.len(),
            "{item} conflict count"
        );
        for (conflict, expected_conflict) in actual.seed_conflicts.iter().zip(&expected.conflicts) {
            let seed = seeds
                .get(&(page.to_owned(), system_id, expected_conflict.sig_id))
                .unwrap_or_else(|| panic!("{item}: missing seed SIG {}", expected_conflict.sig_id));
            assert_eq!(
                conflict.slice_ordinal, expected_conflict.slice_ordinal,
                "{item} slice ordinal"
            );
            assert_eq!(
                conflict.live_ordinal, expected_conflict.live_ordinal,
                "{item} live ordinal"
            );
            assert_eq!(
                (conflict.seed_staff_id, conflict.seed_ordinal),
                (seed.staff_id, seed.ordinal),
                "{item} seed origin"
            );
            assert_eq!(
                conflict.live_ordinal, seed.live_ordinal,
                "{item} seed live provenance"
            );
            assert_eq!(
                rect(conflict.bounds),
                expected_conflict.bounds,
                "{item} conflict bounds"
            );
            assert_eq!(
                conflict.bounds,
                java_rect(seed.bounds),
                "{item} seed bounds"
            );
            assert_eq!(
                conflict.grade.to_bits(),
                expected_conflict.grade_bits,
                "{item} conflict grade"
            );
            assert_eq!(
                conflict.grade.to_bits(),
                seed.grade_bits,
                "{item} seed grade"
            );
        }
        assert_glyph(actual.glyph.as_ref(), expected.glyph, &item);
        let actual_final = match actual.outcome {
            NativeHeadRangePostOutcome::SeedConflict | NativeHeadRangePostOutcome::GlyphEmpty => {
                None
            }
            NativeHeadRangePostOutcome::Final { head_ordinal } => Some(head_ordinal),
        };
        assert_eq!(actual_final, expected.final_ordinal, "{item} final ordinal");
        let actual_outcome = match actual.outcome {
            NativeHeadRangePostOutcome::SeedConflict => "seed-conflict",
            NativeHeadRangePostOutcome::GlyphEmpty => "glyph-empty",
            NativeHeadRangePostOutcome::Final { .. } => "final",
        };
        assert_eq!(actual_outcome, expected.outcome, "{item} outcome");
    }
}

fn assert_heads(
    staff: &audiveris_omr::native_heads_range_glyphs::NativeHeadsRangeGlyphStaff,
    expected: &[ExpectedHead],
    label: &str,
) {
    assert_eq!(staff.heads.len(), expected.len(), "{label}");
    for (actual, expected) in staff.heads.iter().zip(expected) {
        let item = format!("{label} head {}", expected.ordinal);
        assert_eq!(actual.ordinal, expected.ordinal, "{item} ordinal");
        assert_eq!(
            actual.candidate_ordinal, expected.candidate,
            "{item} candidate"
        );
        assert_eq!(actual.raw_ordinal, expected.raw, "{item} raw");
        assert_eq!(actual.attempt_ordinal, expected.attempt, "{item} attempt");
        assert_eq!(actual.geometry_ordinal, expected.scanner, "{item} scanner");
        assert_eq!(actual.shape, expected.shape, "{item} shape");
        assert_eq!(actual.pitch_bits, expected.pitch_bits, "{item} pitch");
        assert_eq!(actual.bounds, expected.bounds, "{item} bounds");
        assert_eq!(actual.grade_bits, expected.grade_bits, "{item} grade");
        assert_eq!(
            actual.distance_impact_bits, expected.distance_impact_bits,
            "{item} impact"
        );
        assert_eq!(
            1.0_f64.to_bits(),
            expected.impact_weight_bits,
            "{item} impact weight"
        );
        assert_eq!(actual.good, expected.good, "{item} good");
        assert_eq!(
            actual.glyph.glyph_bounds, expected.glyph_bounds,
            "{item} glyph bounds"
        );
        assert_eq!(
            actual.glyph.weight, expected.glyph_weight,
            "{item} glyph weight"
        );
        assert_eq!(
            actual.glyph.run_digest, expected.glyph_run_digest,
            "{item} glyph runs"
        );
    }
}

fn assert_glyph(
    actual: Option<&audiveris_omr::head_glyph_retrieval::RetrievedHeadGlyph>,
    expected: Option<ExpectedGlyph>,
    label: &str,
) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) => {
            assert_eq!(
                actual.template_bounds, expected.template_bounds,
                "{label} template bounds"
            );
            assert_eq!(
                actual.foreground_bounds, expected.foreground_bounds,
                "{label} foreground bounds"
            );
            assert_eq!(
                actual.glyph_bounds, expected.glyph_bounds,
                "{label} glyph bounds"
            );
            assert_eq!(actual.weight, expected.weight, "{label} glyph weight");
            assert_eq!(actual.run_digest, expected.run_digest, "{label} glyph runs");
        }
        _ => panic!("{label}: glyph presence mismatch"),
    }
}

fn parse_oracle() -> (
    BTreeMap<StaffKey, ExpectedStaff>,
    BTreeMap<SeedKey, ExpectedSeed>,
) {
    let mut staffs = BTreeMap::<StaffKey, ExpectedStaff>::new();
    let mut seeds = BTreeMap::<SeedKey, ExpectedSeed>::new();
    for line in ORACLE.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.first().copied() {
            Some("headrangestaffsummary") => {
                let key = staff_key(&fields);
                let value = ExpectedStaff {
                    seed_heads: usize_field(&fields, "seedHeads"),
                    candidates: usize_field(&fields, "candidates"),
                    seed_conflicts: usize_field(&fields, "seedConflicts"),
                    glyph_empty: usize_field(&fields, "glyphEmpty"),
                    heads: usize_field(&fields, "heads"),
                    candidate_rows: Vec::new(),
                    head_rows: Vec::new(),
                };
                if let Some(existing) = staffs.get_mut(&key) {
                    existing.seed_heads = value.seed_heads;
                    existing.candidates = value.candidates;
                    existing.seed_conflicts = value.seed_conflicts;
                    existing.glyph_empty = value.glyph_empty;
                    existing.heads = value.heads;
                } else {
                    staffs.insert(key, value);
                }
            }
            Some("headrangecandidate") => {
                staffs
                    .entry(staff_key(&fields))
                    .or_insert_with(empty_staff)
                    .candidate_rows
                    .push(parse_candidate(&fields));
            }
            Some("headrangehead") => {
                staffs
                    .entry(staff_key(&fields))
                    .or_insert_with(empty_staff)
                    .head_rows
                    .push(parse_head(&fields));
            }
            Some("headrangeseedhead") => {
                let page = fields[1].to_owned();
                let system_id = usize_field(&fields, "system");
                let sig_id = usize_field(&fields, "sigId");
                let seed = ExpectedSeed {
                    staff_id: usize_field(&fields, "staff"),
                    ordinal: usize_field(&fields, "ordinal"),
                    live_ordinal: usize_field(&fields, "competitorOrdinal"),
                    bounds: parse_rect(field(&fields, "bounds")),
                    grade_bits: parse_double_bits(field(&fields, "grade")),
                };
                assert!(seeds.insert((page, system_id, sig_id), seed).is_none());
            }
            _ => {}
        }
    }
    for staff in staffs.values() {
        assert_eq!(staff.candidate_rows.len(), staff.candidates);
        assert_eq!(staff.head_rows.len(), staff.heads);
    }
    (staffs, seeds)
}

fn empty_staff() -> ExpectedStaff {
    ExpectedStaff {
        seed_heads: 0,
        candidates: 0,
        seed_conflicts: 0,
        glyph_empty: 0,
        heads: 0,
        candidate_rows: Vec::new(),
        head_rows: Vec::new(),
    }
}

fn parse_candidate(fields: &[&str]) -> ExpectedCandidate {
    let selected = field(fields, "selected").split(':').collect::<Vec<_>>();
    let impacts = field(fields, "impacts").split(':').collect::<Vec<_>>();
    ExpectedCandidate {
        ordinal: usize_field(fields, "ordinal"),
        raw: usize_field(fields, "raw"),
        attempt: usize_field(fields, "attempt"),
        scanner: usize_field(fields, "scanner"),
        x: i32_field(fields, "x"),
        y: i32_field(fields, "y"),
        original_shape: parse_shape(field(fields, "original")),
        shape: parse_shape(field(fields, "finalShape")),
        selected: [
            selected[0].parse().unwrap(),
            selected[1].parse().unwrap(),
            selected[2].parse().unwrap(),
            selected[3].parse().unwrap(),
        ],
        distance_bits: parse_double_bits(selected[4]),
        black_to_void: parse_optional_bool(field(fields, "blackToVoid")),
        slim: parse_rect(field(fields, "slim")),
        pitch_bits: parse_double_bits(field(fields, "pitch")),
        grade_bits: parse_double_bits(field(fields, "grade")),
        distance_impact_bits: parse_double_bits(impacts[1]),
        impact_weight_bits: parse_double_bits(impacts[2]),
        aggregate: usize_field(fields, "aggregate"),
        members: field(fields, "members")
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect(),
        conflicts: parse_conflicts(field(fields, "seedConflicts")),
        glyph: parse_glyph(field(fields, "glyph")),
        final_ordinal: parse_optional_usize(field(fields, "finalOrdinal")),
        outcome: match field(fields, "outcome") {
            "seed-conflict" => "seed-conflict",
            "glyph-empty" => "glyph-empty",
            "final" => "final",
            other => panic!("unknown outcome {other}"),
        },
    }
}

fn parse_head(fields: &[&str]) -> ExpectedHead {
    let impacts = field(fields, "impacts").split(':').collect::<Vec<_>>();
    ExpectedHead {
        ordinal: usize_field(fields, "ordinal"),
        candidate: usize_field(fields, "candidate"),
        raw: usize_field(fields, "raw"),
        attempt: usize_field(fields, "attempt"),
        scanner: usize_field(fields, "scanner"),
        shape: parse_shape(field(fields, "shape")),
        pitch_bits: parse_double_bits(field(fields, "pitch")),
        bounds: parse_rect(field(fields, "bounds")),
        grade_bits: parse_double_bits(field(fields, "grade")),
        distance_impact_bits: parse_double_bits(impacts[1]),
        impact_weight_bits: parse_double_bits(impacts[2]),
        good: field(fields, "good").parse().unwrap(),
        glyph_bounds: parse_rect(field(fields, "glyphBounds")),
        glyph_weight: usize_field(fields, "glyphWeight"),
        glyph_run_digest: u64::from_str_radix(field(fields, "glyphRuns"), 16).unwrap(),
    }
}

fn parse_conflicts(value: &str) -> Vec<ExpectedConflict> {
    if value == "-" {
        return Vec::new();
    }
    value
        .split(',')
        .map(|record| {
            let fields = record.split(':').collect::<Vec<_>>();
            ExpectedConflict {
                slice_ordinal: fields[0].parse().unwrap(),
                live_ordinal: fields[1].parse().unwrap(),
                sig_id: fields[2].parse().unwrap(),
                bounds: HeadTemplateBounds {
                    x: fields[3].parse().unwrap(),
                    y: fields[4].parse().unwrap(),
                    width: fields[5].parse().unwrap(),
                    height: fields[6].parse().unwrap(),
                },
                grade_bits: parse_double_bits(fields[7]),
            }
        })
        .collect()
}

fn parse_glyph(value: &str) -> Option<ExpectedGlyph> {
    if value == "-" {
        return None;
    }
    let fields = value.split(':').collect::<Vec<_>>();
    assert_eq!(fields.len(), 19);
    assert_eq!(
        (fields[0], fields[5], fields[10], fields[15], fields[17]),
        ("template", "foreground", "bounds", "weight", "runs")
    );
    Some(ExpectedGlyph {
        template_bounds: rect_fields(&fields[1..5]),
        foreground_bounds: rect_fields(&fields[6..10]),
        glyph_bounds: rect_fields(&fields[11..15]),
        weight: fields[16].parse().unwrap(),
        run_digest: u64::from_str_radix(fields[18], 16).unwrap(),
    })
}

fn staff_key(fields: &[&str]) -> StaffKey {
    (
        fields[1].to_owned(),
        usize_field(fields, "system"),
        usize_field(fields, "staff"),
    )
}

fn field<'a>(fields: &[&'a str], name: &str) -> &'a str {
    let index = fields
        .iter()
        .position(|field| *field == name)
        .unwrap_or_else(|| panic!("missing {name}"));
    fields[index + 1]
}

fn usize_field(fields: &[&str], name: &str) -> usize {
    field(fields, name).parse().unwrap()
}
fn i32_field(fields: &[&str], name: &str) -> i32 {
    field(fields, name).parse().unwrap()
}

fn parse_double_bits(value: &str) -> u64 {
    u64::from_str_radix(value.rsplit_once('/').unwrap().1, 16).unwrap()
}

fn parse_optional_bool(value: &str) -> Option<bool> {
    (value != "-").then(|| value.parse().unwrap())
}

fn parse_optional_usize(value: &str) -> Option<usize> {
    (value != "-").then(|| value.parse().unwrap())
}

fn parse_shape(value: &str) -> HeadTemplateShape {
    match value {
        "NOTEHEAD_BLACK" => HeadTemplateShape::NoteheadBlack,
        "NOTEHEAD_VOID" => HeadTemplateShape::NoteheadVoid,
        "WHOLE_NOTE" => HeadTemplateShape::WholeNote,
        "BREVE" => HeadTemplateShape::Breve,
        other => panic!("unknown head shape {other}"),
    }
}

fn parse_rect(value: &str) -> HeadTemplateBounds {
    rect_fields(&value.split(':').collect::<Vec<_>>())
}

fn rect_fields(fields: &[&str]) -> HeadTemplateBounds {
    HeadTemplateBounds {
        x: fields[0].parse().unwrap(),
        y: fields[1].parse().unwrap(),
        width: fields[2].parse().unwrap(),
        height: fields[3].parse().unwrap(),
    }
}

fn rect(value: audiveris_omr::head_scanner_slices::JavaRectangle) -> HeadTemplateBounds {
    HeadTemplateBounds {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}

fn java_rect(value: HeadTemplateBounds) -> audiveris_omr::head_scanner_slices::JavaRectangle {
    audiveris_omr::head_scanner_slices::JavaRectangle::new(
        value.x,
        value.y,
        value.width,
        value.height,
    )
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
