// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compact-oracle differential for Java HEADS range post-processing.
//!
//! The fixture deliberately omits raw range candidates. It can therefore
//! prove the post-aggregation seed filter and invariants recorded on aggregate
//! winners, but it cannot replay `aggregateMatches` from its raw input.

use std::collections::{BTreeMap, BTreeSet};

use audiveris_omr::{
    head_range_postprocess::{HeadPostprocessGeometry, filter_seed_conflicts},
    head_scanner_slices::JavaRectangle,
};

const ORACLE: &str = include_str!("../../../oracle/heads-range-pass.txt");
const CORPUS_SUMMARY: &str = "headrangecorpussummary pages 8 systems 30 staffs 55 \
    spotSlices 6759 scans:skips:attempts:rawCandidates:seedConflicts:glyphEmpty \
    921558:5389:3119882:34101:3376:0 seedHeads 3435 candidates 3550 rangeHeads 174 \
    emittedBodySha256 46e62aaafff97ca4c239c1dcd925308e0ebb706c67d4d7cab8f8669549f11a05";

#[derive(Debug, Default)]
struct Corpus {
    pages: BTreeMap<String, Page>,
    summary_seen: bool,
    row_counts: RowCounts,
}

#[derive(Debug, Default)]
struct Page {
    declared_systems: usize,
    declared_staffs: usize,
    systems: BTreeMap<usize, System>,
    summary: Option<PageSummary>,
}

#[derive(Debug, Default)]
struct System {
    staffs: BTreeMap<usize, Staff>,
    summary: Option<SystemSummary>,
}

#[derive(Debug, Default)]
struct Staff {
    seeds: Vec<SeedHead>,
    candidates: Vec<Candidate>,
    spot_rows: usize,
    head_candidate_ordinals: Vec<usize>,
    summary: Option<StaffSummary>,
}

#[derive(Clone, Debug)]
struct SeedHead {
    ordinal: usize,
    sig_id: usize,
    sig_ordinal: usize,
    competitor_ordinal: usize,
    bounds: JavaRectangle,
    grade: f64,
    grade_bits: u64,
    good: bool,
}

#[derive(Debug)]
struct Candidate {
    ordinal: usize,
    raw_ordinal: usize,
    scanner_ordinal: usize,
    bounds: JavaRectangle,
    grade: f64,
    aggregate_ordinal: usize,
    aggregate_members: Vec<usize>,
    seed_conflicts: Vec<ConflictEvidence>,
    has_glyph: bool,
    final_ordinal: Option<usize>,
    outcome: CandidateOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateOutcome {
    SeedConflict,
    Final,
}

#[derive(Debug)]
struct ConflictEvidence {
    local_ordinal: usize,
    system_competitor_ordinal: usize,
    sig_id: usize,
    bounds: JavaRectangle,
    grade_bits: u64,
}

#[derive(Clone, Copy, Debug)]
struct StaffSummary {
    seed_heads: usize,
    spot_slices: usize,
    scans: usize,
    safety_skips: usize,
    attempts: usize,
    raw_candidates: usize,
    candidates: usize,
    seed_conflicts: usize,
    glyph_empty: usize,
    heads: usize,
}

#[derive(Clone, Copy, Debug)]
struct SystemSummary {
    staffs: usize,
    spot_slices: usize,
    scans: usize,
    attempts: usize,
    raw_candidates: usize,
    candidates: usize,
    seed_heads: usize,
    range_heads: usize,
}

#[derive(Clone, Copy, Debug)]
struct PageSummary {
    staffs: usize,
    spot_slices: usize,
    scans: usize,
    attempts: usize,
    raw_candidates: usize,
    candidates: usize,
    seed_heads: usize,
    range_heads: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RowCounts {
    pages: usize,
    systems: usize,
    staffs: usize,
    spots: usize,
    seed_heads: usize,
    candidates: usize,
    heads: usize,
}

#[test]
fn compact_range_postprocess_matches_all_java_seed_conflicts() {
    let corpus = parse_oracle();
    assert!(corpus.summary_seen, "missing corpus summary");
    assert_eq!(
        corpus.row_counts,
        RowCounts {
            pages: 8,
            systems: 30,
            staffs: 55,
            spots: 6_759,
            seed_heads: 3_435,
            candidates: 3_550,
            heads: 174,
        }
    );

    let mut conflicts_checked = 0_usize;
    let mut retained_checked = 0_usize;
    let mut aggregate_members_checked = 0_usize;

    for (page_key, page) in &corpus.pages {
        assert_eq!(page.systems.len(), page.declared_systems, "{page_key}");
        let page_summary = page
            .summary
            .unwrap_or_else(|| panic!("{page_key}: missing page summary"));
        let mut page_totals = PageSummary {
            staffs: 0,
            spot_slices: 0,
            scans: 0,
            attempts: 0,
            raw_candidates: 0,
            candidates: 0,
            seed_heads: 0,
            range_heads: 0,
        };

        for (system_id, system) in &page.systems {
            let label = format!("{page_key} system {system_id}");
            let system_summary = system
                .summary
                .unwrap_or_else(|| panic!("{label}: missing system summary"));
            assert_eq!(system.staffs.len(), system_summary.staffs, "{label}");
            let mut accumulated_seeds = Vec::<SeedHead>::new();
            let mut system_totals = SystemSummary {
                staffs: 0,
                spot_slices: 0,
                scans: 0,
                attempts: 0,
                raw_candidates: 0,
                candidates: 0,
                seed_heads: 0,
                range_heads: 0,
            };

            for (staff_id, staff) in &system.staffs {
                let label = format!("{label} staff {staff_id}");
                let summary = staff
                    .summary
                    .unwrap_or_else(|| panic!("{label}: missing staff summary"));
                validate_staff_fixture(&label, staff, summary);
                validate_compact_aggregate_invariants(
                    &label,
                    &staff.candidates,
                    summary.raw_candidates,
                );
                aggregate_members_checked += summary.raw_candidates;

                accumulated_seeds.extend(staff.seeds.iter().cloned());
                accumulated_seeds.sort_by_key(|seed| seed.bounds.y);
                let (staff_conflicts, staff_retained) =
                    validate_seed_filter(&label, &staff.candidates, &accumulated_seeds);
                conflicts_checked += staff_conflicts;
                retained_checked += staff_retained;

                system_totals.staffs += 1;
                system_totals.spot_slices += summary.spot_slices;
                system_totals.scans += summary.scans;
                system_totals.attempts += summary.attempts;
                system_totals.raw_candidates += summary.raw_candidates;
                system_totals.candidates += summary.candidates;
                system_totals.seed_heads += summary.seed_heads;
                system_totals.range_heads += summary.heads;
            }

            assert_system_totals(&label, system_totals, system_summary);
            page_totals.staffs += system_totals.staffs;
            page_totals.spot_slices += system_totals.spot_slices;
            page_totals.scans += system_totals.scans;
            page_totals.attempts += system_totals.attempts;
            page_totals.raw_candidates += system_totals.raw_candidates;
            page_totals.candidates += system_totals.candidates;
            page_totals.seed_heads += system_totals.seed_heads;
            page_totals.range_heads += system_totals.range_heads;
        }

        assert_eq!(page_totals.staffs, page.declared_staffs, "{page_key}");
        assert_page_totals(page_key, page_totals, page_summary);
    }

    assert_eq!(conflicts_checked, 3_376);
    assert_eq!(retained_checked, 174);
    assert_eq!(aggregate_members_checked, 34_101);
}

fn validate_staff_fixture(label: &str, staff: &Staff, summary: StaffSummary) {
    assert_eq!(staff.seeds.len(), summary.seed_heads, "{label}: seed rows");
    assert_eq!(staff.spot_rows, summary.spot_slices, "{label}: spot rows");
    assert_eq!(
        staff.candidates.len(),
        summary.candidates,
        "{label}: candidates"
    );
    assert_eq!(
        staff
            .candidates
            .iter()
            .filter(|candidate| candidate.outcome == CandidateOutcome::SeedConflict)
            .count(),
        summary.seed_conflicts,
        "{label}: conflicts"
    );
    assert_eq!(
        summary.glyph_empty, 0,
        "{label}: compact fixture assumption"
    );
    assert_eq!(
        staff.head_candidate_ordinals.len(),
        summary.heads,
        "{label}: heads"
    );

    let mut competitor_ordinals = BTreeSet::new();
    for (ordinal, seed) in staff.seeds.iter().enumerate() {
        assert_eq!(seed.ordinal, ordinal, "{label}: seed ordinal");
        assert!(
            seed.sig_ordinal >= seed.ordinal,
            "{label}: seed SIG ordinal"
        );
        assert!(
            competitor_ordinals.insert(seed.competitor_ordinal),
            "{label}: duplicate system competitor ordinal"
        );
    }
    let final_candidates = staff
        .candidates
        .iter()
        .filter(|candidate| candidate.outcome == CandidateOutcome::Final)
        .collect::<Vec<_>>();
    for (final_ordinal, candidate) in final_candidates.iter().enumerate() {
        assert!(candidate.has_glyph, "{label}: final candidate glyph");
        assert_eq!(candidate.final_ordinal, Some(final_ordinal), "{label}");
    }
    let expected_heads = final_candidates
        .into_iter()
        .map(|candidate| candidate.ordinal)
        .collect::<Vec<_>>();
    assert_eq!(staff.head_candidate_ordinals, expected_heads, "{label}");
}

fn validate_compact_aggregate_invariants(
    label: &str,
    candidates: &[Candidate],
    raw_candidates: usize,
) {
    let mut members = BTreeSet::new();
    let mut previous: Option<&Candidate> = None;
    for (ordinal, candidate) in candidates.iter().enumerate() {
        assert_eq!(candidate.ordinal, ordinal, "{label}: candidate ordinal");
        assert!(
            candidate.aggregate_members.contains(&candidate.raw_ordinal),
            "{label}: aggregate main raw {} absent from members",
            candidate.raw_ordinal
        );
        assert!(
            candidate
                .aggregate_members
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "{label}: aggregate members are not in raw order"
        );
        for &member in &candidate.aggregate_members {
            assert!(
                members.insert(member),
                "{label}: duplicate raw member {member}"
            );
        }
        assert!(
            candidate
                .seed_conflicts
                .windows(2)
                .all(|pair| pair[0].local_ordinal < pair[1].local_ordinal),
            "{label}: conflict evidence local order"
        );
        let system_ordinals = candidate
            .seed_conflicts
            .iter()
            .map(|evidence| evidence.system_competitor_ordinal)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            system_ordinals.len(),
            candidate.seed_conflicts.len(),
            "{label}: duplicate conflict system competitor ordinal"
        );

        if let Some(previous) = previous {
            assert!(
                candidate.scanner_ordinal >= previous.scanner_ordinal,
                "{label}: scanner order"
            );
            if candidate.scanner_ordinal == previous.scanner_ordinal {
                assert_eq!(
                    candidate.aggregate_ordinal,
                    previous.aggregate_ordinal + 1,
                    "{label}: aggregate order"
                );
                assert!(
                    java_double_compare(previous.grade, candidate.grade).is_ge(),
                    "{label}: aggregate mains are not reverse-grade ordered"
                );
            } else {
                assert_eq!(candidate.aggregate_ordinal, 0, "{label}: aggregate reset");
            }
        } else {
            assert_eq!(candidate.aggregate_ordinal, 0, "{label}: first aggregate");
        }
        previous = Some(candidate);
    }
    assert_eq!(
        members.len(),
        raw_candidates,
        "{label}: raw member coverage"
    );
    assert_eq!(
        members.into_iter().collect::<Vec<_>>(),
        (0..raw_candidates).collect::<Vec<_>>(),
        "{label}: raw members must partition the compact raw ordinal range"
    );
}

fn validate_seed_filter(
    label: &str,
    candidates: &[Candidate],
    accumulated_seeds: &[SeedHead],
) -> (usize, usize) {
    let seeds_by_sig_id = accumulated_seeds
        .iter()
        .map(|seed| (seed.sig_id, seed))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        seeds_by_sig_id.len(),
        accumulated_seeds.len(),
        "{label}: duplicate seed SIG id"
    );

    let mut conflict_count = 0_usize;
    let mut retained_count = 0_usize;
    let mut first = 0_usize;
    while first < candidates.len() {
        let scanner_ordinal = candidates[first].scanner_ordinal;
        let end = candidates[first..]
            .iter()
            .position(|candidate| candidate.scanner_ordinal != scanner_ordinal)
            .map_or(candidates.len(), |offset| first + offset);
        let scanner_candidates = &candidates[first..end];
        let scanner_label = format!("{label} scanner {scanner_ordinal}");

        // Compact rows do not retain the scanner Area or non-conflicting
        // competitors. `seedConflicts`, however, is exhaustive: the probe
        // invokes overlapSeed against every singleton head in the scanner's
        // x-sorted slice. Its union is therefore the decision-complete seed
        // view: every omitted head is non-qualifying for every candidate in
        // this scanner and cannot affect filterSeedConflicts.
        let mut seed_records = BTreeMap::<usize, (usize, usize, JavaRectangle, u64)>::new();
        for candidate in scanner_candidates {
            for evidence in &candidate.seed_conflicts {
                let record = (
                    evidence.system_competitor_ordinal,
                    evidence.sig_id,
                    evidence.bounds,
                    evidence.grade_bits,
                );
                if let Some(previous) = seed_records.insert(evidence.local_ordinal, record) {
                    assert_eq!(
                        previous, record,
                        "{scanner_label}: inconsistent conflict provenance"
                    );
                }
            }
        }
        let system_ordinals = seed_records
            .values()
            .map(|record| record.0)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            system_ordinals.len(),
            seed_records.len(),
            "{scanner_label}: duplicate system competitor provenance"
        );

        let mut seed_view = Vec::with_capacity(seed_records.len());
        for (_, (_, sig_id, bounds, grade_bits)) in seed_records {
            let seed = seeds_by_sig_id
                .get(&sig_id)
                .copied()
                .unwrap_or_else(|| panic!("{scanner_label}: unknown seed SIG id {sig_id}"));
            assert!(
                seed.good,
                "{scanner_label}: non-good conflict seed {sig_id}"
            );
            assert_eq!(seed.bounds, bounds, "{scanner_label}: seed bounds {sig_id}");
            assert_eq!(
                seed.grade_bits, grade_bits,
                "{scanner_label}: seed grade {sig_id}"
            );
            seed_view.push(seed);
        }
        assert!(
            seed_view
                .windows(2)
                .all(|pair| pair[0].bounds.x <= pair[1].bounds.x),
            "{scanner_label}: conflict seed view is not x-sorted"
        );

        let seed_geometry = seed_view
            .iter()
            .map(|seed| HeadPostprocessGeometry {
                bounds: seed.bounds,
                grade: seed.grade,
            })
            .collect::<Vec<_>>();
        let range_geometry = scanner_candidates
            .iter()
            .map(|candidate| HeadPostprocessGeometry {
                bounds: candidate.bounds,
                grade: candidate.grade,
            })
            .collect::<Vec<_>>();
        let actual = filter_seed_conflicts(&range_geometry, &seed_geometry);
        let expected_retained = scanner_candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (candidate.outcome == CandidateOutcome::Final).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual.retained_range_indices, expected_retained,
            "{scanner_label}: retained range candidate order"
        );
        retained_count += actual.retained_range_indices.len();

        let expected_conflicts = scanner_candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.outcome == CandidateOutcome::SeedConflict)
            .collect::<Vec<_>>();
        assert_eq!(
            actual.conflicts.len(),
            expected_conflicts.len(),
            "{scanner_label}"
        );
        for (actual_conflict, (range_index, candidate)) in
            actual.conflicts.iter().zip(expected_conflicts)
        {
            assert_eq!(actual_conflict.range_index, range_index, "{scanner_label}");
            let expected = candidate
                .seed_conflicts
                .first()
                .unwrap_or_else(|| panic!("{scanner_label}: missing first conflict evidence"));
            let selected = seed_view[actual_conflict.seed_index];
            assert_eq!(
                selected.sig_id, expected.sig_id,
                "{scanner_label}: conflict SIG id"
            );
            assert_eq!(
                selected.bounds, expected.bounds,
                "{scanner_label}: conflict bounds"
            );
            assert_eq!(
                selected.grade_bits, expected.grade_bits,
                "{scanner_label}: conflict grade"
            );
            assert_eq!(
                actual_conflict.iou.to_bits(),
                expected_java_iou_bits(expected.bounds, candidate.bounds),
                "{scanner_label}: conflict IoU bits"
            );
            conflict_count += 1;
        }
        first = end;
    }

    (conflict_count, retained_count)
}

fn assert_system_totals(label: &str, actual: SystemSummary, expected: SystemSummary) {
    assert_eq!(actual.staffs, expected.staffs, "{label}: staffs");
    assert_eq!(actual.spot_slices, expected.spot_slices, "{label}: spots");
    assert_eq!(actual.scans, expected.scans, "{label}: scans");
    assert_eq!(actual.attempts, expected.attempts, "{label}: attempts");
    assert_eq!(
        actual.raw_candidates, expected.raw_candidates,
        "{label}: raw"
    );
    assert_eq!(
        actual.candidates, expected.candidates,
        "{label}: candidates"
    );
    assert_eq!(actual.seed_heads, expected.seed_heads, "{label}: seeds");
    assert_eq!(actual.range_heads, expected.range_heads, "{label}: heads");
}

fn assert_page_totals(label: &str, actual: PageSummary, expected: PageSummary) {
    assert_eq!(actual.staffs, expected.staffs, "{label}: staffs");
    assert_eq!(actual.spot_slices, expected.spot_slices, "{label}: spots");
    assert_eq!(actual.scans, expected.scans, "{label}: scans");
    assert_eq!(actual.attempts, expected.attempts, "{label}: attempts");
    assert_eq!(
        actual.raw_candidates, expected.raw_candidates,
        "{label}: raw"
    );
    assert_eq!(
        actual.candidates, expected.candidates,
        "{label}: candidates"
    );
    assert_eq!(actual.seed_heads, expected.seed_heads, "{label}: seeds");
    assert_eq!(actual.range_heads, expected.range_heads, "{label}: heads");
}

fn parse_oracle() -> Corpus {
    let mut corpus = Corpus::default();
    for (line_index, line) in ORACLE.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens = line.split_ascii_whitespace().collect::<Vec<_>>();
        match tokens.first().copied() {
            Some("headrangepage") => parse_page(&mut corpus, &tokens),
            Some("headrangeseedhead") => parse_seed_head(&mut corpus, &tokens),
            Some("headrangespot") => parse_spot(&mut corpus, &tokens),
            Some("headrangecandidate") => parse_candidate(&mut corpus, &tokens),
            Some("headrangehead") => parse_head(&mut corpus, &tokens),
            Some("headrangestaffsummary") => parse_staff_summary(&mut corpus, &tokens),
            Some("headrangesystemsummary") => parse_system_summary(&mut corpus, &tokens),
            Some("headrangepagesummary") => parse_page_summary(&mut corpus, &tokens),
            Some("headrangecorpussummary") => {
                assert_eq!(line, CORPUS_SUMMARY, "line {}", line_index + 1);
                assert!(!corpus.summary_seen, "duplicate corpus summary");
                corpus.summary_seen = true;
            }
            Some(other) => panic!("line {}: unexpected compact row {other}", line_index + 1),
            None => unreachable!(),
        }
    }
    corpus
}

fn parse_page(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        12,
        &[
            (2, "width"),
            (4, "height"),
            (6, "systems"),
            (8, "staves"),
            (10, "family"),
        ],
    );
    parse_usize(tokens[3]);
    parse_usize(tokens[5]);
    assert_eq!(tokens[11], "Bravura");
    let page = Page {
        declared_systems: parse_usize(tokens[7]),
        declared_staffs: parse_usize(tokens[9]),
        ..Page::default()
    };
    assert!(corpus.pages.insert(tokens[1].to_owned(), page).is_none());
    corpus.row_counts.pages += 1;
}

fn parse_seed_head(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        34,
        &[
            (2, "system"),
            (4, "staff"),
            (6, "ordinal"),
            (8, "sigId"),
            (10, "sigOrdinal"),
            (12, "competitorOrdinal"),
            (14, "shape"),
            (16, "pitch"),
            (18, "bounds"),
            (20, "grade"),
            (22, "impacts"),
            (24, "good"),
            (26, "glyphId"),
            (28, "glyphBounds"),
            (30, "glyphWeight"),
            (32, "glyphRuns"),
        ],
    );
    parse_head_shape(tokens[15]);
    parse_oracle_double(tokens[17]);
    let bounds = parse_rectangle(tokens[19]);
    let (grade, grade_bits) = parse_oracle_double(tokens[21]);
    parse_impacts(tokens[23]);
    let good = parse_bool(tokens[25]);
    parse_usize(tokens[27]);
    parse_rectangle(tokens[29]);
    parse_usize(tokens[31]);
    parse_hex_u64(tokens[33]);
    let seed = SeedHead {
        ordinal: parse_usize(tokens[7]),
        sig_id: parse_usize(tokens[9]),
        sig_ordinal: parse_usize(tokens[11]),
        competitor_ordinal: parse_usize(tokens[13]),
        bounds,
        grade,
        grade_bits,
        good,
    };
    staff_mut(corpus, tokens).seeds.push(seed);
    corpus.row_counts.seed_heads += 1;
}

fn parse_spot(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        24,
        &[
            (2, "system"),
            (4, "staff"),
            (6, "scanner"),
            (8, "source"),
            (10, "spot"),
            (12, "pool"),
            (14, "bounds"),
            (16, "weight"),
            (18, "runs"),
            (20, "expanded"),
            (22, "effective"),
        ],
    );
    parse_usize(tokens[7]);
    parse_source(tokens[9]);
    parse_usize(tokens[11]);
    parse_usize(tokens[13]);
    parse_rectangle(tokens[15]);
    parse_usize(tokens[17]);
    parse_hex_u64(tokens[19]);
    parse_rectangle(tokens[21]);
    if tokens[23] != "-" {
        parse_i32_tuple(tokens[23], 2);
    }
    staff_mut(corpus, tokens).spot_rows += 1;
    corpus.row_counts.spots += 1;
}

fn parse_candidate(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        48,
        &[
            (2, "system"),
            (4, "staff"),
            (6, "ordinal"),
            (8, "raw"),
            (10, "attempt"),
            (12, "scanner"),
            (14, "source"),
            (16, "x"),
            (18, "y"),
            (20, "original"),
            (22, "finalShape"),
            (24, "selected"),
            (26, "blackToVoid"),
            (28, "slim"),
            (30, "pitch"),
            (32, "grade"),
            (34, "impacts"),
            (36, "aggregate"),
            (38, "members"),
            (40, "seedConflicts"),
            (42, "glyph"),
            (44, "finalOrdinal"),
            (46, "outcome"),
        ],
    );
    parse_usize(tokens[11]);
    parse_source(tokens[15]);
    parse_i32(tokens[17]);
    parse_i32(tokens[19]);
    parse_candidate_shape(tokens[21]);
    parse_candidate_shape(tokens[23]);
    parse_selected(tokens[25]);
    if tokens[27] != "-" {
        parse_bool(tokens[27]);
    }
    let bounds = parse_rectangle(tokens[29]);
    parse_oracle_double(tokens[31]);
    let (grade, _) = parse_oracle_double(tokens[33]);
    parse_impacts(tokens[35]);
    let conflicts = parse_conflicts(tokens[41]);
    let has_glyph = parse_glyph(tokens[43]);
    let final_ordinal = (tokens[45] != "-").then(|| parse_usize(tokens[45]));
    let outcome = match tokens[47] {
        "seed-conflict" => CandidateOutcome::SeedConflict,
        "final" => CandidateOutcome::Final,
        other => panic!("unexpected compact candidate outcome {other}"),
    };
    match outcome {
        CandidateOutcome::SeedConflict => {
            assert!(!conflicts.is_empty());
            assert!(!has_glyph);
            assert!(final_ordinal.is_none());
        }
        CandidateOutcome::Final => {
            assert!(conflicts.is_empty());
            assert!(has_glyph);
            assert!(final_ordinal.is_some());
        }
    }
    let candidate = Candidate {
        ordinal: parse_usize(tokens[7]),
        raw_ordinal: parse_usize(tokens[9]),
        scanner_ordinal: parse_usize(tokens[13]),
        bounds,
        grade,
        aggregate_ordinal: parse_usize(tokens[37]),
        aggregate_members: parse_usize_list(tokens[39]),
        seed_conflicts: conflicts,
        has_glyph,
        final_ordinal,
        outcome,
    };
    staff_mut(corpus, tokens).candidates.push(candidate);
    corpus.row_counts.candidates += 1;
}

fn parse_head(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        56,
        &[
            (2, "system"),
            (4, "staff"),
            (6, "ordinal"),
            (8, "candidate"),
            (10, "raw"),
            (12, "attempt"),
            (14, "scanner"),
            (16, "source"),
            (18, "x"),
            (20, "y"),
            (22, "original"),
            (24, "finalShape"),
            (26, "selected"),
            (28, "blackToVoid"),
            (30, "slim"),
            (32, "sigId"),
            (34, "sigOrdinal"),
            (36, "shape"),
            (38, "pitch"),
            (40, "bounds"),
            (42, "grade"),
            (44, "impacts"),
            (46, "good"),
            (48, "glyphId"),
            (50, "glyphBounds"),
            (52, "glyphWeight"),
            (54, "glyphRuns"),
        ],
    );
    for index in [7, 9, 11, 13, 15, 33, 35, 49, 53] {
        parse_usize(tokens[index]);
    }
    parse_source(tokens[17]);
    parse_i32(tokens[19]);
    parse_i32(tokens[21]);
    parse_head_shape(tokens[23]);
    parse_head_shape(tokens[25]);
    parse_selected(tokens[27]);
    if tokens[29] != "-" {
        parse_bool(tokens[29]);
    }
    parse_rectangle(tokens[31]);
    parse_head_shape(tokens[37]);
    parse_oracle_double(tokens[39]);
    parse_rectangle(tokens[41]);
    parse_oracle_double(tokens[43]);
    parse_impacts(tokens[45]);
    parse_bool(tokens[47]);
    parse_rectangle(tokens[51]);
    parse_hex_u64(tokens[55]);
    let staff = staff_mut(corpus, tokens);
    assert_eq!(parse_usize(tokens[7]), staff.head_candidate_ordinals.len());
    staff.head_candidate_ordinals.push(parse_usize(tokens[9]));
    corpus.row_counts.heads += 1;
}

fn parse_staff_summary(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        70,
        &[
            (2, "system"),
            (4, "staff"),
            (6, "scanners"),
            (8, "seedHeads"),
            (10, "spotSlices"),
            (12, "scans"),
            (14, "safetySkips"),
            (16, "attempts"),
            (18, "rawCandidates"),
            (20, "candidates"),
            (22, "seedConflicts"),
            (24, "glyphEmpty"),
            (26, "heads"),
            (28, "sigBefore"),
            (30, "sigAfterSeed"),
            (32, "sigAfterRange"),
            (34, "competitorsBefore"),
            (36, "competitorsAfterSeed"),
            (38, "scanKinds"),
            (40, "decisions"),
            (42, "perf"),
            (44, "outcomes"),
            (46, "seedHash"),
            (48, "spotHash"),
            (50, "spotKernelHash"),
            (52, "scanHash"),
            (54, "scanKernelHash"),
            (56, "attemptHash"),
            (58, "attemptKernelHash"),
            (60, "rawCandidateHash"),
            (62, "rawKernelHash"),
            (64, "candidateHash"),
            (66, "headHash"),
            (68, "hash"),
        ],
    );
    for index in [7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37] {
        parse_usize(tokens[index]);
    }
    parse_usize_tuple(tokens[39], 6);
    parse_usize_tuple(tokens[41], 5);
    parse_usize_tuple(tokens[43], 4);
    parse_usize_tuple(tokens[45], 7);
    for index in (47..=69).step_by(2) {
        parse_hex_u64(tokens[index]);
    }
    let summary = StaffSummary {
        seed_heads: parse_usize(tokens[9]),
        spot_slices: parse_usize(tokens[11]),
        scans: parse_usize(tokens[13]),
        safety_skips: parse_usize(tokens[15]),
        attempts: parse_usize(tokens[17]),
        raw_candidates: parse_usize(tokens[19]),
        candidates: parse_usize(tokens[21]),
        seed_conflicts: parse_usize(tokens[23]),
        glyph_empty: parse_usize(tokens[25]),
        heads: parse_usize(tokens[27]),
    };
    let scan_kinds = parse_usize_tuple(tokens[39], 6);
    assert_eq!(scan_kinds.iter().sum::<usize>(), summary.scans);
    assert_eq!(scan_kinds[5], summary.safety_skips);
    assert!(staff_mut(corpus, tokens).summary.replace(summary).is_none());
    corpus.row_counts.staffs += 1;
}

fn parse_system_summary(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        33,
        &[
            (2, "system"),
            (4, "staffs"),
            (6, "spotSlices"),
            (8, "scans"),
            (10, "attempts"),
            (12, "rawCandidates"),
            (14, "candidates"),
            (16, "seedHeads"),
            (18, "rangeHeads"),
            (20, "sigBefore"),
            (22, "sigAfter"),
            (24, "competitorsBefore"),
            (26, "competitorsAfter"),
            (28, "seedPerf"),
            (30, "rangePerf"),
        ],
    );
    for index in (5..=27).step_by(2) {
        parse_usize(tokens[index]);
    }
    parse_usize_tuple(tokens[29], 4);
    parse_usize_tuple(tokens[31], 4);
    parse_hex_u64(tokens[32]);
    let summary = SystemSummary {
        staffs: parse_usize(tokens[5]),
        spot_slices: parse_usize(tokens[7]),
        scans: parse_usize(tokens[9]),
        attempts: parse_usize(tokens[11]),
        raw_candidates: parse_usize(tokens[13]),
        candidates: parse_usize(tokens[15]),
        seed_heads: parse_usize(tokens[17]),
        range_heads: parse_usize(tokens[19]),
    };
    let page = corpus
        .pages
        .get_mut(tokens[1])
        .unwrap_or_else(|| panic!("unknown page {}", tokens[1]));
    let system = page.systems.entry(parse_usize(tokens[3])).or_default();
    assert!(system.summary.replace(summary).is_none());
    corpus.row_counts.systems += 1;
}

fn parse_page_summary(corpus: &mut Corpus, tokens: &[&str]) {
    schema(
        tokens,
        19,
        &[
            (2, "staffs"),
            (4, "spotSlices"),
            (6, "scans"),
            (8, "attempts"),
            (10, "rawCandidates"),
            (12, "candidates"),
            (14, "seedHeads"),
            (16, "rangeHeads"),
        ],
    );
    for index in (3..=17).step_by(2) {
        parse_usize(tokens[index]);
    }
    parse_hex_u64(tokens[18]);
    let summary = PageSummary {
        staffs: parse_usize(tokens[3]),
        spot_slices: parse_usize(tokens[5]),
        scans: parse_usize(tokens[7]),
        attempts: parse_usize(tokens[9]),
        raw_candidates: parse_usize(tokens[11]),
        candidates: parse_usize(tokens[13]),
        seed_heads: parse_usize(tokens[15]),
        range_heads: parse_usize(tokens[17]),
    };
    let page = corpus
        .pages
        .get_mut(tokens[1])
        .unwrap_or_else(|| panic!("unknown page {}", tokens[1]));
    assert!(page.summary.replace(summary).is_none());
}

fn staff_mut<'a>(corpus: &'a mut Corpus, tokens: &[&str]) -> &'a mut Staff {
    let page = corpus
        .pages
        .get_mut(tokens[1])
        .unwrap_or_else(|| panic!("unknown page {}", tokens[1]));
    page.systems
        .entry(parse_usize(tokens[3]))
        .or_default()
        .staffs
        .entry(parse_usize(tokens[5]))
        .or_default()
}

fn schema(tokens: &[&str], length: usize, labels: &[(usize, &str)]) {
    assert_eq!(tokens.len(), length, "bad {} field count", tokens[0]);
    for &(index, label) in labels {
        assert_eq!(
            tokens[index],
            label,
            "bad {} schema at field {}",
            tokens[0],
            index + 1
        );
    }
}

fn parse_usize(value: &str) -> usize {
    value
        .parse()
        .unwrap_or_else(|_| panic!("invalid usize {value}"))
}

fn parse_i32(value: &str) -> i32 {
    value
        .parse()
        .unwrap_or_else(|_| panic!("invalid i32 {value}"))
}

fn parse_hex_u64(value: &str) -> u64 {
    assert_eq!(value.len(), 16, "invalid u64 hex width {value}");
    u64::from_str_radix(value, 16).unwrap_or_else(|_| panic!("invalid u64 hex {value}"))
}

fn parse_oracle_double(value: &str) -> (f64, u64) {
    let (java_hex, raw_bits) = value
        .split_once('/')
        .unwrap_or_else(|| panic!("invalid oracle double {value}"));
    assert!(java_hex.contains("0x"), "invalid Java hex double {value}");
    let bits = parse_hex_u64(raw_bits);
    (f64::from_bits(bits), bits)
}

fn parse_rectangle(value: &str) -> JavaRectangle {
    let values = parse_i32_tuple(value, 4);
    JavaRectangle::new(values[0], values[1], values[2], values[3])
}

fn parse_i32_tuple(value: &str, length: usize) -> Vec<i32> {
    let values = value.split(':').map(parse_i32).collect::<Vec<_>>();
    assert_eq!(values.len(), length, "bad i32 tuple {value}");
    values
}

fn parse_usize_tuple(value: &str, length: usize) -> Vec<usize> {
    let values = value.split(':').map(parse_usize).collect::<Vec<_>>();
    assert_eq!(values.len(), length, "bad usize tuple {value}");
    values
}

fn parse_usize_list(value: &str) -> Vec<usize> {
    let values = value.split(',').map(parse_usize).collect::<Vec<_>>();
    assert!(!values.is_empty(), "empty aggregate member list");
    values
}

fn parse_bool(value: &str) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => panic!("invalid boolean {value}"),
    }
}

fn parse_head_shape(value: &str) {
    assert!(
        matches!(value, "NOTEHEAD_BLACK" | "NOTEHEAD_VOID"),
        "unexpected head shape {value}"
    );
}

fn parse_candidate_shape(value: &str) {
    assert!(
        matches!(value, "NOTEHEAD_BLACK" | "NOTEHEAD_VOID" | "WHOLE_NOTE"),
        "unexpected range-candidate shape {value}"
    );
}

fn parse_source(value: &str) {
    assert!(
        value.starts_with("staff-line:") || value.starts_with("ledger:"),
        "bad source {value}"
    );
}

fn parse_selected(value: &str) {
    let fields = value.split(':').collect::<Vec<_>>();
    assert_eq!(fields.len(), 5, "bad selected tuple {value}");
    for field in &fields[..4] {
        parse_i32(field);
    }
    parse_oracle_double(fields[4]);
}

fn parse_impacts(value: &str) {
    let fields = value.split(':').collect::<Vec<_>>();
    assert_eq!(fields.len(), 3, "bad impacts {value}");
    assert_eq!(fields[0], "dist");
    parse_oracle_double(fields[1]);
    parse_oracle_double(fields[2]);
}

fn parse_glyph(value: &str) -> bool {
    if value == "-" {
        return false;
    }
    let fields = value.split(':').collect::<Vec<_>>();
    assert_eq!(fields.len(), 19, "bad glyph {value}");
    assert_eq!(fields[0], "template");
    assert_eq!(fields[5], "foreground");
    assert_eq!(fields[10], "bounds");
    assert_eq!(fields[15], "weight");
    assert_eq!(fields[17], "runs");
    for index in [1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14] {
        parse_i32(fields[index]);
    }
    parse_usize(fields[16]);
    parse_hex_u64(fields[18]);
    true
}

fn parse_conflicts(value: &str) -> Vec<ConflictEvidence> {
    if value == "-" {
        return Vec::new();
    }
    value
        .split(',')
        .map(|record| {
            let fields = record.split(':').collect::<Vec<_>>();
            assert_eq!(fields.len(), 8, "bad conflict evidence {record}");
            let (_, grade_bits) = parse_oracle_double(fields[7]);
            ConflictEvidence {
                local_ordinal: parse_usize(fields[0]),
                system_competitor_ordinal: parse_usize(fields[1]),
                sig_id: parse_usize(fields[2]),
                bounds: JavaRectangle::new(
                    parse_i32(fields[3]),
                    parse_i32(fields[4]),
                    parse_i32(fields[5]),
                    parse_i32(fields[6]),
                ),
                grade_bits,
            }
        })
        .collect()
}

fn java_double_compare(left: f64, right: f64) -> std::cmp::Ordering {
    if left < right {
        std::cmp::Ordering::Less
    } else if left > right {
        std::cmp::Ordering::Greater
    } else {
        java_double_to_long_bits(left).cmp(&java_double_to_long_bits(right))
    }
}

fn java_double_to_long_bits(value: f64) -> i64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000_i64
    } else {
        value.to_bits() as i64
    }
}

fn expected_java_iou_bits(one: JavaRectangle, two: JavaRectangle) -> u64 {
    let left = one.x.max(two.x);
    let top = one.y.max(two.y);
    let right =
        (i64::from(one.x) + i64::from(one.width)).min(i64::from(two.x) + i64::from(two.width));
    let bottom =
        (i64::from(one.y) + i64::from(one.height)).min(i64::from(two.y) + i64::from(two.height));
    let width = (right - i64::from(left)).max(i64::from(i32::MIN)) as i32;
    let height = (bottom - i64::from(top)).max(i64::from(i32::MIN)) as i32;
    let intersection_area = width.wrapping_mul(height);
    if intersection_area == 0 {
        return 0.0_f64.to_bits();
    }
    let union_area = one
        .width
        .wrapping_mul(one.height)
        .wrapping_add(two.width.wrapping_mul(two.height))
        .wrapping_sub(intersection_area);
    (f64::from(intersection_area) / f64::from(union_area)).to_bits()
}
