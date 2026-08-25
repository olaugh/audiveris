// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use audiveris_omr::{
    cue_beams_step::{
        NativeCueAggregateRecognition, extract_native_cue_spots, materialize_native_cue_aggregates,
        plan_native_cue_aggregate_processing,
    },
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads_with_small_heads,
    native_ledgers::recognize_native_ledgers,
    native_reduction::{NativeReductionRecognition, recognize_native_reduction},
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems::recognize_native_stems,
    recognize::{
        GridLinesRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
};

const ORACLE: &str = include_str!("../../../oracle/cue-aggregates.txt");

#[test]
fn active_cue_aggregate_corpus_matches_java() {
    for path in [
        "data/examples/chula.png",
        "data/examples/allegretto.png",
        "data/examples/batuque.png",
        "data/examples/carmen.png",
        "data/examples/cucaracha.png",
        "data/examples/hove.png",
        "data/examples/zizi.png",
        "data/examples/BachInvention5.jpg",
    ] {
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
        let processing = plan_native_cue_aggregate_processing(&grid, &reduction, &recognition)
            .expect("native cue process plans");
        assert!(
            processing
                .systems
                .iter()
                .all(|system| system.plans.is_empty()),
            "{page}"
        );
        assert!(
            extract_native_cue_spots(&grid, &processing)
                .expect("native cue spot extraction")
                .aggregates
                .is_empty(),
            "{page}"
        );
    }
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
