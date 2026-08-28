// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{
    fmt::{self, Debug, Write as _},
    path::PathBuf,
    process::Command,
};

use audiveris_omr::{
    beam_inters::BeamKind,
    cue_beams_step::{
        NativeCueAggregateRecognition, NativeCueBeamsOptions, apply_native_cue_beam_stem_relations,
        check_native_cue_beam_spots, check_native_cue_beam_stem_links, extract_native_cue_spots,
        group_native_cue_beams, materialize_native_cue_aggregates,
        materialize_native_cue_beam_mutations, plan_native_cue_aggregate_processing,
        plan_native_cue_beam_stem_links, recognize_native_cue_beams_with_options,
    },
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_with_small_heads,
    native_ledgers::recognize_native_ledgers,
    native_reduction::{NativeReductionRecognition, recognize_native_reduction},
    native_sig::NativeSigRelationKind,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems::recognize_native_stems,
    recognize::{
        GridLinesRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
    stems_step::NativeBeamPortion,
};

const ORACLE: &str = include_str!("../../../oracle/cue-aggregates.txt");
const CHOPIN_CUE_ORACLE: &str = include_str!("../../../oracle/cue-beams-chopin-page23-system4.txt");
const CUE_AGGREGATE_PAGE_ENV: &str = "AUDIVERIS_CUE_AGGREGATE_TEST_PAGE";
const CHOPIN_OP9_NO1_S1A6_ENV: &str = "AUDIVERIS_CHOPIN_OP9_NO1_S1A6_TEST";
const CHOPIN_OP9_NO1_S1A6_PAGE: &str =
    "rust/oracle/chopin-nocturne-op9-no1-page1-stage-aligner-96.png";
const CUE_AGGREGATE_CORPUS: [&str; 8] = [
    "data/examples/chula.png",
    "data/examples/allegretto.png",
    "data/examples/batuque.png",
    "data/examples/carmen.png",
    "data/examples/cucaracha.png",
    "data/examples/hove.png",
    "data/examples/zizi.png",
    "data/examples/BachInvention5.jpg",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DebugFingerprint {
    fnv1a128: u128,
    byte_len: u64,
}

struct DebugFingerprintWriter {
    fingerprint: DebugFingerprint,
}

impl fmt::Write for DebugFingerprintWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        const FNV_PRIME_128: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        for byte in text.bytes() {
            self.fingerprint.fnv1a128 ^= u128::from(byte);
            self.fingerprint.fnv1a128 = self.fingerprint.fnv1a128.wrapping_mul(FNV_PRIME_128);
            self.fingerprint.byte_len += 1;
        }
        Ok(())
    }
}

fn debug_fingerprint(value: &impl Debug) -> DebugFingerprint {
    let mut writer = DebugFingerprintWriter {
        fingerprint: DebugFingerprint {
            fnv1a128: 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d,
            byte_len: 0,
        },
    };
    write!(&mut writer, "{value:?}").expect("fingerprinting Debug output cannot fail");
    writer.fingerprint
}

#[test]
fn chopin_cue_fixture_exposes_terminal_active_evidence() {
    let (grid, reduction, _) = recognize_page("rust/oracle/chopin-nocturne-page23-system4-cue.png");
    let completed = recognize_native_cue_beams_with_options(
        &grid,
        reduction,
        true,
        NativeCueBeamsOptions::default(),
    )
    .expect("Chopin CUE_BEAMS");
    let active = completed.active.as_deref().expect("active CUE_BEAMS");
    assert_eq!(active.aggregates.systems[0].aggregates.len(), 2);
    assert_eq!(
        active
            .spots
            .aggregates
            .iter()
            .map(|spots| spots.glyphs.len())
            .sum::<usize>(),
        7
    );
    let cue_beams = &active.mutations.systems[0].beams;
    assert_eq!(cue_beams.len(), 1);
    let cue = cue_beams[0].beam;
    assert_eq!(cue.kind, BeamKind::SmallBeam);
    assert_eq!(cue.item.median.x1.to_bits(), 0x4082_5800_0000_0000);
    assert_eq!(cue.item.median.y1.to_bits(), 0x4054_d2a7_f7be_9065);
    assert_eq!(cue.item.median.x2.to_bits(), 0x4083_a800_0000_0000);
    assert_eq!(cue.item.median.y2.to_bits(), 0x4056_24d8_27a0_2247);
    assert_eq!(cue.item.height.to_bits(), 0x4015_84bb_b403_8f78);
    assert_eq!(cue.grade.to_bits(), 0x3fe1_c8ae_eec7_4dc1);
    assert_eq!(
        active.grouping.systems[0]
            .aggregates
            .iter()
            .map(|aggregate| aggregate.group_sig_ordinals.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(active.stem_checks.checks.len(), 4);
    assert!(active.stem_checks.checks.iter().all(|check| check.accepted));
    assert_eq!(
        active
            .stem_checks
            .checks
            .iter()
            .map(|check| check.beam_portion)
            .collect::<Vec<_>>(),
        vec![
            NativeBeamPortion::Left,
            NativeBeamPortion::Center,
            NativeBeamPortion::Center,
            NativeBeamPortion::Right,
        ]
    );
    assert_eq!(active.stem_relations.systems[0].mutations.len(), 4);
    assert!(CHOPIN_CUE_ORACLE.contains(
        "cueaggregatestagebeam chopin-nocturne-page23-system4-cue.png#1 step CUE_BEAMS \
         system 1 ordinal 212 bounds 587 80 42 12 medianBits 4082580000000000 \
         4054d2a7f7be9065 4083a80000000000 405624d827a02247 heightBits 401584bbb4038f78 \
         gradeBits 3fe1c8aeeec74dc1"
    ));
    assert!(CHOPIN_CUE_ORACLE.contains(
        "cueaggregatestagesummary chopin-nocturne-page23-system4-cue.png#1 step CUE_BEAMS \
         system 1 smallBlack 7 smallBeams 1 groups 12 beamStemRelations 4"
    ));

    let repeated = recognize_native_cue_beams_with_options(
        &grid,
        completed.reduction.clone(),
        true,
        NativeCueBeamsOptions::default(),
    )
    .expect("repeat Chopin CUE_BEAMS");
    assert_eq!(completed, repeated);
}

#[test]
fn chopin_op9_no1_page1_s1a6_reaches_terminal_cue_relations() {
    let test_binary = std::env::current_exe().expect("current cue aggregate test binary");
    let status = Command::new(test_binary)
        .args([
            "--exact",
            "chopin_op9_no1_page1_s1a6_reaches_terminal_cue_relations_page",
            "--nocapture",
        ])
        .env(CHOPIN_OP9_NO1_S1A6_ENV, "1")
        .env("AUDIVERIS_ENABLE_CURVED_BEAM_RECOVERY", "1")
        .env("AUDIVERIS_ENABLE_STEM_GUIDED_HOOK_RECOVERY", "1")
        .env("AUDIVERIS_ENABLE_CUE_STEM_GUIDED_HOOK_RECOVERY", "1")
        .status()
        .expect("run isolated Chopin Op. 9 No. 1 S1A6 regression");
    assert!(status.success(), "isolated Chopin S1A6 regression failed");
}

#[test]
fn chopin_op9_no1_page1_s1a6_reaches_terminal_cue_relations_page() {
    if std::env::var_os(CHOPIN_OP9_NO1_S1A6_ENV).is_none() {
        return;
    }

    let (grid, reduction, aggregates) = recognize_page(CHOPIN_OP9_NO1_S1A6_PAGE);
    let ordinary_fingerprint = debug_fingerprint(&reduction);
    let disabled = recognize_native_cue_beams_with_options(
        &grid,
        reduction.clone(),
        true,
        NativeCueBeamsOptions {
            enabled: false,
            supplemental_hook_recovery: true,
        },
    )
    .expect("disabled CUE_BEAMS must preserve ordinary recognition");
    assert!(disabled.active.is_none());
    assert_eq!(ordinary_fingerprint, debug_fingerprint(&disabled.reduction));

    // These are lossless pixels from the exact StageAligner 96% page. On the
    // full page this x869/y169 aggregate has zero-based ordinal 5 and renders
    // as S1A6. Cropping away later systems makes the corpus gate CI-safe and
    // removes one earlier aggregate, but retains the head/SIG identity split.
    let s1a6 = aggregates.systems[0]
        .aggregates
        .iter()
        .find(|aggregate| {
            aggregate.bounds.x == 869
                && aggregate.bounds.y == 79
                && aggregate.bounds.width == 15
                && aggregate.bounds.height == 54
        })
        .expect("Chopin S1A6 aggregate");
    assert_eq!(s1a6.ordinal, 4);
    assert_eq!(s1a6.members.len(), 3);
    let s1a6_ordinal = s1a6.ordinal;

    let completed = recognize_native_cue_beams_with_options(
        &grid,
        reduction,
        true,
        NativeCueBeamsOptions {
            enabled: true,
            supplemental_hook_recovery: true,
        },
    )
    .expect("Chopin Op. 9 No. 1 page 1 CUE_BEAMS");
    let active = completed.active.as_deref().expect("active CUE_BEAMS");
    let final_s1 = completed
        .reduction
        .stems
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("final S1 STEMS state");
    let bindings = &final_s1.transaction.state_after.beam_state.bindings;
    let corners = completed
        .reduction
        .stems
        .components
        .head_corners
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("S1 head corners");
    assert!(
        s1a6.members.iter().any(|(head_vertex, _)| {
            let Some((reference, _)) = bindings
                .head_vertices
                .iter()
                .find(|(_, vertex)| **vertex == *head_vertex)
            else {
                return false;
            };
            corners.heads_in_sig_order.iter().any(|head| {
                head.reference == *reference && head.system_creation_ordinal != head_vertex.0
            })
        }),
        "the cropped S1A6 evidence must retain the creation/live ordinal split"
    );
    let s1_mutations = active
        .mutations
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("S1 cue beam mutations");
    let s1a6_beam = s1_mutations
        .beams
        .iter()
        .find(|beam| beam.aggregate_ordinal == s1a6_ordinal)
        .expect("S1A6 cue beam");
    let s1_grouping = active
        .grouping
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("S1 cue grouping");
    let s1a6_group = s1_grouping
        .aggregates
        .iter()
        .find(|aggregate| aggregate.aggregate_ordinal == s1a6_ordinal)
        .expect("S1A6 cue beam group");
    assert_eq!(s1a6_group.group_sig_ordinals.len(), 1);
    let group_vertex = s1a6_group.group_sig_ordinals[0].0;
    assert!(
        s1_grouping.sig_after.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Containment
                && edge.source == group_vertex
                && edge.target == s1a6_beam.sig_ordinal.0
        }),
        "S1A6 must publish its cue beam containment relation"
    );

    let s1_lookup = active
        .stem_lookup
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("S1 cue stem lookup");
    assert_eq!(
        s1_lookup
            .plans
            .iter()
            .filter(|plan| plan.aggregate_ordinal == s1a6_ordinal)
            .count(),
        3,
        "every S1A6 head must resolve through its stable binding"
    );
    assert!(
        active.stem_relations.systems.iter().any(|system| {
            system.mutations.iter().any(|mutation| {
                mutation.first_relation_created || !mutation.extended_relation_edges.is_empty()
            })
        }),
        "the page must reach terminal cue BeamStem relation publication"
    );
}

#[test]
fn active_cue_aggregate_corpus_matches_java() {
    let test_binary = std::env::current_exe().expect("current cue aggregate test binary");
    for path in CUE_AGGREGATE_CORPUS {
        let status = Command::new(&test_binary)
            .args(["--exact", "active_cue_aggregate_corpus_page", "--nocapture"])
            .env(CUE_AGGREGATE_PAGE_ENV, path)
            .status()
            .unwrap_or_else(|error| panic!("run isolated cue aggregate page {path}: {error}"));
        assert!(
            status.success(),
            "isolated cue aggregate page failed: {path}"
        );
    }
}

#[test]
fn active_cue_aggregate_corpus_page() {
    let Some(path) = std::env::var_os(CUE_AGGREGATE_PAGE_ENV) else {
        return;
    };
    let path = path
        .to_str()
        .expect("cue aggregate test page path is UTF-8");
    assert_cue_aggregate_page(path);
}

fn assert_cue_aggregate_page(path: &str) {
    let (grid, reduction, recognition) = recognize_page(path);
    let page = format!(
        "{}#1",
        PathBuf::from(path).file_name().unwrap().to_string_lossy()
    );
    let rows = canonical_native_rows(&page, &recognition);
    let expected = canonical_java_rows(&page);
    assert_eq!(rows, expected, "{page}");
    assert!(
        recognition
            .systems
            .iter()
            .all(|system| system.aggregates.is_empty()),
        "{page}"
    );
    // Every stage owns page-sized SIG snapshots. Bound the manual
    // lifecycle's drop scope before constructing the composed results so
    // the corpus gate also fits GitHub's smaller Linux runners.
    let relation_mutations = {
        let processing = plan_native_cue_aggregate_processing(&grid, &reduction, &recognition)
            .expect("native cue process plans");
        assert!(
            processing
                .systems
                .iter()
                .all(|system| system.plans.is_empty()),
            "{page}"
        );
        let spots =
            extract_native_cue_spots(&grid, &processing).expect("native cue spot extraction");
        assert!(spots.aggregates.is_empty(), "{page}");
        let checks =
            check_native_cue_beam_spots(&grid, &spots, reduction.stems.reduction_interline)
                .expect("native cue beam checks");
        assert!(checks.aggregates.is_empty(), "{page}");
        let mutations = materialize_native_cue_beam_mutations(&grid, &reduction, &spots, &checks)
            .expect("native cue beam mutations");
        assert!(mutations.registered_spots.is_empty(), "{page}");
        assert!(
            mutations.systems.iter().all(|system| {
                system.beams.is_empty()
                    && system.sig_after.vertices.len() == system.sig_before_vertex_count
            }),
            "{page}"
        );
        let grouping =
            group_native_cue_beams(&mutations, &checks, reduction.stems.reduction_interline)
                .expect("native cue beam grouping");
        assert!(
            grouping.systems.iter().all(|system| {
                system.aggregates.is_empty()
                    && system.sig_after.vertices.len() == system.sig_before_grouping_vertex_count
                    && system.sig_after.edges
                        == mutations
                            .systems
                            .iter()
                            .find(|mutation| mutation.system_id == system.system_id)
                            .expect("aligned mutation system")
                            .sig_after
                            .edges
            }),
            "{page}"
        );
        let link_plans = plan_native_cue_beam_stem_links(
            &reduction,
            &recognition,
            &processing,
            &mutations,
            &grouping,
        )
        .expect("native cue beam-stem lookup");
        assert!(
            link_plans
                .systems
                .iter()
                .all(|system| system.plans.is_empty()),
            "{page}"
        );
        let relation_checks =
            check_native_cue_beam_stem_links(&reduction, &mutations, &grouping, &link_plans)
                .expect("native cue BeamStem checks");
        assert!(relation_checks.checks.is_empty(), "{page}");
        let relation_mutations = apply_native_cue_beam_stem_relations(
            &mutations,
            &grouping,
            &link_plans,
            &relation_checks,
            reduction.stems.sheet_skew_slope,
            reduction.stems.reduction_interline,
        )
        .expect("native cue BeamStem mutations");
        assert!(
            relation_mutations.systems.iter().all(|system| {
                system.mutations.is_empty()
                    && system.sig_after.edges.len() == system.sig_before_relation_count
                    && system.sig_after
                        == grouping
                            .systems
                            .iter()
                            .find(|grouped| grouped.system_id == system.system_id)
                            .expect("aligned grouping system")
                            .sig_after
            }),
            "{page}"
        );
        relation_mutations
    };

    let completed = recognize_native_cue_beams_with_options(
        &grid,
        reduction,
        true,
        NativeCueBeamsOptions::default(),
    )
    .expect("composed native CUE_BEAMS");
    assert_eq!(completed.skip_reason, None, "{page}");
    assert_eq!(
        completed
            .active
            .as_deref()
            .expect("active CUE_BEAMS")
            .stem_relations,
        relation_mutations,
        "terminal relation application must be the published SIG"
    );
    drop(relation_mutations);
    drop(recognition);

    // Comparing the two page-sized results directly retains both full active
    // SIG histories at once. Stream every derived Debug field through a
    // 128-bit fingerprint so the baseline can be dropped before its replay.
    let completed_active_fingerprint = debug_fingerprint(&completed.active);
    let completed_gate = (
        completed.skip_reason,
        completed.small_heads_enabled,
        completed.detected_small_beam_height,
        completed.ordinary_enabled,
        completed.supplemental_hook_recovery_enabled,
    );
    let baseline_reduction = completed.reduction.clone();
    drop(completed);
    let repeated = recognize_native_cue_beams_with_options(
        &grid,
        baseline_reduction.clone(),
        true,
        NativeCueBeamsOptions::default(),
    )
    .expect("repeat composed native CUE_BEAMS");
    assert_eq!(
        completed_gate,
        (
            repeated.skip_reason,
            repeated.small_heads_enabled,
            repeated.detected_small_beam_height,
            repeated.ordinary_enabled,
            repeated.supplemental_hook_recovery_enabled,
        ),
        "{page} CUE_BEAMS gate is deterministic"
    );
    assert_eq!(
        completed_active_fingerprint,
        debug_fingerprint(&repeated.active),
        "{page} active lifecycle is deterministic"
    );
    drop(repeated);

    let disabled = recognize_native_cue_beams_with_options(
        &grid,
        baseline_reduction.clone(),
        true,
        NativeCueBeamsOptions {
            enabled: false,
            supplemental_hook_recovery: true,
        },
    )
    .expect("disabled ordinary CUE_BEAMS");
    assert!(disabled.active.is_none(), "{page}");
    assert!(disabled.supplemental_hook_recovery_enabled, "{page}");
    assert_eq!(
        debug_fingerprint(&baseline_reduction),
        debug_fingerprint(&disabled.reduction),
        "{page} disabled CUE_BEAMS must preserve ordinary STEMS/BEAMS"
    );
    drop(disabled);

    let recovery_enabled = recognize_native_cue_beams_with_options(
        &grid,
        baseline_reduction,
        true,
        NativeCueBeamsOptions {
            enabled: true,
            supplemental_hook_recovery: true,
        },
    )
    .expect("independently enabled supplemental recovery");
    assert_eq!(
        completed_active_fingerprint,
        debug_fingerprint(&recovery_enabled.active),
        "{page} recovery control must not perturb ordinary cue recognition"
    );
    drop(recovery_enabled);
}

fn recognize_page(
    path: &str,
) -> (
    GridLinesRecognition,
    NativeReductionRecognition,
    NativeCueAggregateRecognition,
) {
    let grid = recognize_grid_lines(repo_path(path)).expect("GRID");
    let headers = recognize_native_headers(&grid).expect("HEADERS");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS");
    let ledgers = recognize_native_ledgers(&grid, &beams).expect("LEDGERS");
    let heads = recognize_native_heads_with_small_heads(
        &grid,
        &headers,
        &stem_seeds,
        &beams,
        &ledgers,
        true,
    )
    .expect("small-head HEADS");
    let stems = recognize_native_stems(&grid, &headers, &stem_seeds, &beams, &ledgers, &heads, 1)
        .expect("small-head STEMS");
    let reduction = recognize_native_reduction(&grid, stems).expect("small-head REDUCTION");
    let aggregates = materialize_native_cue_aggregates(&reduction).expect("native cue aggregates");
    (grid, reduction, aggregates)
}

fn canonical_native_rows(page: &str, recognition: &NativeCueAggregateRecognition) -> Vec<String> {
    let mut rows = Vec::new();
    for system in &recognition.systems {
        for aggregate in &system.aggregates {
            rows.push(format!(
                "cueaggregatecanonical {page} system {} ordinal {} bounds {} {} {} {} memberCount {}",
                system.system_id,
                aggregate.ordinal,
                aggregate.bounds.x,
                aggregate.bounds.y,
                aggregate.bounds.width,
                aggregate.bounds.height,
                aggregate.members.len()
            ));
        }
        for head in &system.qualified_heads {
            rows.push(format!(
                "cueaggregateheadcanonical {page} system {} bounds {} {} {} {} gradeBits {:016x} aggregate {}",
                system.system_id,
                head.bounds.x,
                head.bounds.y,
                head.bounds.width,
                head.bounds.height,
                head.grade.to_bits(),
                head.aggregate_ordinal.map_or(-1, |ordinal| ordinal as i32)
            ));
        }
        rows.push(format!(
            "cueaggregatesystem {page} system {} interline {} margins {} {} smallBlack {} qualified {} aggregates {}",
            system.system_id,
            system.interline,
            system.cue_x_margin,
            system.cue_y_margin,
            system.small_black_count,
            system.qualified_heads.len(),
            system.aggregates.len()
        ));
    }
    let summary = format!(
        "cueaggregatesummarycanonical {page} systems {} rows {} {:016x}",
        recognition.systems.len(),
        rows.len(),
        hash(&rows)
    );
    rows.push(summary);
    rows
}

fn canonical_java_rows(page: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut systems = None;
    for line in ORACLE.lines().filter(|line| line.contains(page)) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if line.starts_with("cueaggregatehead ") {
            rows.push(format!(
                "cueaggregateheadcanonical {page} system {} bounds {} {} {} {} gradeBits {} aggregate {}",
                value(&fields, "system"),
                value(&fields, "bounds"),
                fields[field_index(&fields, "bounds") + 2],
                fields[field_index(&fields, "bounds") + 3],
                fields[field_index(&fields, "bounds") + 4],
                value(&fields, "gradeBits"),
                value(&fields, "aggregate")
            ));
        } else if line.starts_with("cueaggregate ") {
            let members = value(&fields, "members");
            let member_count = if members == "-" {
                0
            } else {
                members.split(',').count()
            };
            rows.push(format!(
                "cueaggregatecanonical {page} system {} ordinal {} bounds {} {} {} {} memberCount {member_count}",
                value(&fields, "system"),
                value(&fields, "ordinal"),
                value(&fields, "bounds"),
                fields[field_index(&fields, "bounds") + 2],
                fields[field_index(&fields, "bounds") + 3],
                fields[field_index(&fields, "bounds") + 4]
            ));
        } else if line.starts_with("cueaggregatesystem ") {
            rows.push(line.to_owned());
        } else if line.starts_with("cueaggregatesummary ") {
            systems = Some(value(&fields, "systems").to_owned());
        }
    }
    rows.push(format!(
        "cueaggregatesummarycanonical {page} systems {} rows {} {:016x}",
        systems.unwrap_or_else(|| panic!("missing Java summary for {page}")),
        rows.len(),
        hash(&rows)
    ));
    rows
}

fn value<'a>(fields: &'a [&str], name: &str) -> &'a str {
    fields[field_index(fields, name) + 1]
}

fn field_index(fields: &[&str], name: &str) -> usize {
    fields
        .iter()
        .position(|field| *field == name)
        .unwrap_or_else(|| panic!("missing {name} in {fields:?}"))
}

fn hash(records: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for record in records {
        for byte in format!("{record}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
