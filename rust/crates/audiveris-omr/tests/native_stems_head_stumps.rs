// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page differential gate for section-built STEMS head stumps.

use std::{collections::HashMap, path::PathBuf};

use audiveris_image::{
    run_table::{Orientation, Run},
    section::{Bounds, Section},
};

use audiveris_omr::{
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems_head_corners::materialize_native_stems_head_corners,
    native_stems_head_seeds::materialize_native_stems_head_seeds,
    native_stems_head_stumps::{
        NativeStemsHeadStumpOutcome, NativeStemsHeadStumpRegistration, NativeStemsHeadStumpSystem,
        materialize_native_stems_head_stumps,
    },
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemRect, NativeStemVerticalSide},
};

const PAGES: &[&str] = &[
    "chula.png",
    "allegretto.png",
    "batuque.png",
    "carmen.png",
    "cucaracha.png",
    "hove.png",
    "zizi.png",
    "BachInvention5.jpg",
];
const ORACLE_PATH: &str = "rust/oracle/stems-head-stumps.txt";

#[derive(Default, Debug, Eq, PartialEq)]
struct Totals {
    systems: usize,
    corners: usize,
    seeds: usize,
    fallbacks: usize,
    empty: usize,
    candidates: usize,
    accepted: usize,
    rejected: usize,
    new_builds: usize,
    reused_builds: usize,
    sections: usize,
    steps: usize,
    subsections: usize,
}

#[test]
fn native_stems_head_stumps_match_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH)).expect("head-stump oracle");
    assert!(oracle.contains(
        "stemsheadstumpcorpussummary pages 8 systems 30 beams 803 tremolos 0 beamRegs 5 heads 3521 corners 14084 seeds 4182 fallbacks 9902 emptyBuilds 969 buildCandidates 8933 acceptedBuilds 758 rejectedBuilds 8175 headRegs 5591 newBuilds 5591 reusedBuilds 3342 sections 18398 steps 18398 subsections 3660 probeSourceSha256 101cea60bc9445407333b31121d6bd774e72a7148f8f53faa865468a35105e59 emittedBodySha256 15e72fc97b017e475ce1bd03f396329ca15fddd1a114e2ab033e7cc091bf563a"
    ));
    let mut totals = Totals::default();
    for image in PAGES {
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
            .unwrap_or_else(|error| panic!("{image}: GRID failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{image}: HEADERS failed: {error}"));
        let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
            .unwrap_or_else(|error| panic!("{image}: STEM_SEEDS failed: {error}"));
        let beams =
            recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
                .unwrap_or_else(|error| panic!("{image}: BEAMS failed: {error}"));
        let ledgers = recognize_native_ledgers(&grid, &beams)
            .unwrap_or_else(|error| panic!("{image}: LEDGERS failed: {error}"));
        let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
            .unwrap_or_else(|error| panic!("{image}: HEADS failed: {error}"));
        let corners = materialize_native_stems_head_corners(&heads, &stem_seeds)
            .unwrap_or_else(|error| panic!("{image}: STEMS corners failed: {error}"));
        let head_seeds = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corners)
            .unwrap_or_else(|error| panic!("{image}: STEMS head seeds failed: {error}"));
        let stumps =
            materialize_native_stems_head_stumps(&grid, &stem_seeds, &corners, &head_seeds)
                .unwrap_or_else(|error| panic!("{image}: STEMS head stumps failed: {error}"));

        let page = format!("{image}#1");
        let actual =
            native_projected_rows(&page, &grid, &stem_seeds, &corners, &head_seeds, &stumps);
        let expected = oracle_projected_rows(&oracle, &page);
        if actual != expected {
            report_first_mismatches(&page, &actual, &expected);
            panic!("{page}: projected native stump evidence differs from Java");
        }

        totals.systems += stumps.systems.len();
        totals.corners += stumps.corner_count;
        totals.seeds += stumps.seed_count;
        totals.fallbacks += stumps.fallback_count;
        totals.empty += stumps.empty_build_count;
        totals.candidates += stumps.build_candidate_count;
        totals.accepted += stumps.accepted_build_count;
        totals.rejected += stumps.rejected_build_count;
        totals.new_builds += stumps.new_build_count;
        totals.reused_builds += stumps.reused_build_count;
        totals.sections += stumps
            .systems
            .iter()
            .flat_map(|system| &system.heads_by_abscissa)
            .flat_map(|head| &head.corners_in_constructor_order)
            .filter_map(|corner| corner.build.as_ref())
            .map(|build| build.sections.len())
            .sum::<usize>();
        totals.steps += stumps
            .systems
            .iter()
            .flat_map(|system| &system.heads_by_abscissa)
            .flat_map(|head| &head.corners_in_constructor_order)
            .filter_map(|corner| corner.build.as_ref())
            .map(|build| build.steps.len())
            .sum::<usize>();
        totals.subsections += stumps
            .systems
            .iter()
            .flat_map(|system| &system.heads_by_abscissa)
            .flat_map(|head| &head.corners_in_constructor_order)
            .filter_map(|corner| corner.build.as_ref())
            .filter(|build| build.subsection.is_some())
            .count();
    }

    assert_eq!(
        totals,
        Totals {
            systems: 30,
            corners: 14_084,
            seeds: 4_182,
            fallbacks: 9_902,
            empty: 969,
            candidates: 8_933,
            accepted: 758,
            rejected: 8_175,
            new_builds: 5_591,
            reused_builds: 3_342,
            sections: 18_398,
            steps: 18_398,
            subsections: 3_660,
        }
    );
}

fn native_projected_rows(
    page: &str,
    grid: &audiveris_omr::recognize::GridLinesRecognition,
    stem_seeds: &audiveris_omr::native_stem_seeds::NativeStemSeedRecognition,
    corners: &audiveris_omr::native_stems_head_corners::NativeStemsHeadCornerRecognition,
    head_seeds: &audiveris_omr::native_stems_head_seeds::NativeStemsHeadSeedRecognition,
    stumps: &audiveris_omr::native_stems_head_stumps::NativeStemsHeadStumpRecognition,
) -> Vec<String> {
    let mut rows = Vec::new();
    for system in &stumps.systems {
        let system_id = system.system_id;
        let corner_system = corners
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("corner system");
        let seed_system = head_seeds
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("head-seed system");
        let interline = corner_system.interline;
        let stem_thickness = stem_seeds.main_stem_thickness;
        rows.push(format!(
            "stemsheadstumpsystem {page} system {system_id} profile {} interline {interline} \
             stemThickness {stem_thickness} sourceSeeds {} keptSeeds {} verticalSections {} \
             maxHeadSeedDy {} maxHeadInDx {} maxHeadOutDx {} minHeadStumpDy {} \
             stumpAreaDxIn {} stumpAreaDxOut {} stumpAreaDy {} mainStemThickness {stem_thickness}",
            corner_system.profile,
            seed_system.seeds_in_free_order.len(),
            seed_system.kept_seed_ordinals.len(),
            system.vertical_section_source_ordinals.len(),
            to_pixels(interline, 0.25),
            corner_system.max_head_in_dx,
            corner_system.max_head_out_dx,
            to_pixels(interline, 0.5),
            java_hex_double(f64::from(interline) * 0.1),
            java_hex_double(f64::from(interline) * 0.1),
            java_hex_double(f64::from(to_pixels(interline, 0.2))),
        ));

        let mut build_aliases = HashMap::<usize, usize>::new();
        for head in &system.heads_by_abscissa {
            let seed_head = &seed_system.heads_by_abscissa[head.x_ordinal];
            let corner_head = &corner_system.heads_in_sig_order[head.sig_ordinal];
            for ((stump_corner, seed_corner), geometry) in head
                .corners_in_constructor_order
                .iter()
                .zip(&seed_head.corners_in_constructor_order)
                .zip(&corner_head.corners_in_constructor_order)
            {
                let kind = match stump_corner.outcome {
                    NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal } => format!(
                        "seed:{}",
                        seed_system
                            .kept_seed_ordinals
                            .iter()
                            .position(|&ordinal| ordinal == free_glyph_ordinal)
                            .expect("selected seed is kept")
                    ),
                    NativeStemsHeadStumpOutcome::Built { .. } => "built".to_owned(),
                    NativeStemsHeadStumpOutcome::None => "none".to_owned(),
                };
                rows.push(format!(
                    "stemsheadstumpcorner {page} system {system_id} head {} ctorOrdinal {} \
                     corner {} xDir {} yDir {} ref {} kind {kind}",
                    head.x_ordinal,
                    stump_corner.constructor_ordinal,
                    corner_name(geometry.horizontal, geometry.vertical),
                    horizontal_direction(geometry.horizontal),
                    vertical_direction(geometry.vertical),
                    point(geometry.reference),
                ));
                let Some(build) = &stump_corner.build else {
                    assert!(seed_corner.selected_seed_ordinal.is_some());
                    continue;
                };

                for section in &build.sections {
                    let source = source_section(grid, system, section.source_ordinal);
                    rows.push(format!(
                        "stemsheadstumpsection {page} system {system_id} head {} ctorOrdinal {} \
                         sortOrdinal {} inputOrdinal {} sourceOrdinal {} bounds {} weight {} \
                         firstPos {} runs {}:{:016x} areaCenter {}:{} distance {} containsRef {}",
                        head.x_ordinal,
                        stump_corner.constructor_ordinal,
                        section.sort_ordinal,
                        section.input_ordinal,
                        section.source_ordinal,
                        bounds(section.bounds),
                        section.weight,
                        section.first_pos,
                        section.run_count,
                        section_run_hash(source),
                        section.area_center.0,
                        section.area_center.1,
                        java_hex_double(section.distance),
                        section.contains_reference,
                    ));
                }
                for step in &build.steps {
                    rows.push(format!(
                        "stemsheadstumpstep {page} system {system_id} head {} ctorOrdinal {} \
                         sortOrdinal {} sourceOrdinal {} preMember {} afterAddWidth {} tooWide {} \
                         removed {} finalWeight {} finalBounds {} memberSources {}",
                        head.x_ordinal,
                        stump_corner.constructor_ordinal,
                        step.sort_ordinal,
                        step.source_ordinal,
                        step.pre_member,
                        step.after_add_width,
                        step.too_wide,
                        step.removed,
                        step.final_weight,
                        optional_bounds(step.final_bounds),
                        optional_ordinals(&step.member_source_ordinals),
                    ));
                }
                if let Some(subsection) = &build.subsection {
                    rows.push(format!(
                        "stemsheadstumpsubsection {page} system {system_id} head {} ctorOrdinal {} \
                         sourceOrdinal {} stemWidth {} x0 {} i0 {} x1 {} i1 {} bounds {} weight {} \
                         firstPos {} runs {}:{:016x}",
                        head.x_ordinal,
                        stump_corner.constructor_ordinal,
                        subsection.source_ordinal,
                        subsection.stem_width,
                        subsection.x0,
                        subsection.i0,
                        subsection.x1,
                        subsection.i1,
                        optional_bounds(subsection.bounds),
                        subsection.weight,
                        subsection.first_pos,
                        subsection.runs.len(),
                        owned_section_run_hash(subsection.first_pos, &subsection.runs),
                    ));
                }
                let common = format!(
                    "stemsheadstumpbuild {page} system {system_id} head {} ctorOrdinal {} area {} \
                     sections {} preAdded {} compoundWeight {} compoundBounds {}",
                    head.x_ordinal,
                    stump_corner.constructor_ordinal,
                    rect(build.area),
                    build.sections.len(),
                    optional_ordinal(build.pre_added_source_ordinal),
                    build.compound_weight,
                    optional_bounds(build.compound_bounds),
                );
                if let Some(candidate) = &build.candidate {
                    let canonical = build.canonical_glyph_index.expect("registered candidate");
                    let next_alias = build_aliases.len();
                    let alias = *build_aliases.entry(canonical).or_insert(next_alias);
                    let registration = match build.registration.as_ref().expect("registration") {
                        NativeStemsHeadStumpRegistration::New { .. } => "new",
                        NativeStemsHeadStumpRegistration::Reused { .. } => "reuse",
                    };
                    let returned = if matches!(
                        stump_corner.outcome,
                        NativeStemsHeadStumpOutcome::Built { .. }
                    ) {
                        "built"
                    } else {
                        "none"
                    };
                    rows.push(format!(
                        "{common} candidate bounds:{},weight:{},runs:{}:{:016x} refY {} extDy {} \
                         standsOut {} buildAlias {alias} registration {registration} returned {returned}",
                        bounds(candidate.bounds),
                        candidate.weight,
                        candidate.run_count(),
                        candidate.run_digest(),
                        build.ref_y,
                        build.extension_dy,
                        build.stands_out,
                    ));
                } else {
                    rows.push(format!("{common} candidate none returned none"));
                }
            }
        }
    }
    rows
}

fn oracle_projected_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .filter_map(project_oracle_row)
        .collect()
}

fn project_oracle_row(line: &str) -> Option<String> {
    let words = line.split_whitespace().collect::<Vec<_>>();
    let prefix = words.first().copied()?;
    match prefix {
        "stemsheadstumpsystem" => Some(format!(
            "{prefix} {} system {} profile {} interline {} stemThickness {} sourceSeeds {} \
             keptSeeds {} verticalSections {} maxHeadSeedDy {} maxHeadInDx {} maxHeadOutDx {} \
             minHeadStumpDy {} stumpAreaDxIn {} stumpAreaDxOut {} stumpAreaDy {} \
             mainStemThickness {}",
            words[1],
            field(&words, "system"),
            field(&words, "profile"),
            field(&words, "interline"),
            field(&words, "stemThickness"),
            field(&words, "sourceSeeds"),
            field(&words, "keptSeeds"),
            field(&words, "verticalSections"),
            field(&words, "maxHeadSeedDy"),
            field(&words, "maxHeadInDx"),
            field(&words, "maxHeadOutDx"),
            field(&words, "minHeadStumpDy"),
            field(&words, "stumpAreaDxIn"),
            field(&words, "stumpAreaDxOut"),
            field(&words, "stumpAreaDy"),
            field(&words, "mainStemThickness"),
        )),
        "stemsheadstumpcorner" => {
            let kind = field(&words, "kind");
            let kind = if kind.starts_with("built:") {
                "built"
            } else {
                kind
            };
            Some(format!(
                "{prefix} {} system {} head {} ctorOrdinal {} corner {} xDir {} yDir {} ref {} \
                 kind {kind}",
                words[1],
                field(&words, "system"),
                field(&words, "head"),
                field(&words, "ctorOrdinal"),
                field(&words, "corner"),
                field(&words, "xDir"),
                field(&words, "yDir"),
                field(&words, "ref"),
            ))
        }
        "stemsheadstumpsection" | "stemsheadstumpstep" | "stemsheadstumpsubsection" => {
            Some(line.to_owned())
        }
        "stemsheadstumpbuild" => {
            let common = format!(
                "{prefix} {} system {} head {} ctorOrdinal {} area {} sections {} preAdded {} \
                 compoundWeight {} compoundBounds {}",
                words[1],
                field(&words, "system"),
                field(&words, "head"),
                field(&words, "ctorOrdinal"),
                field(&words, "area"),
                field(&words, "sections"),
                field(&words, "preAdded"),
                field(&words, "compoundWeight"),
                field(&words, "compoundBounds"),
            );
            let candidate = field(&words, "candidate");
            if candidate == "none" {
                Some(format!("{common} candidate none returned none"))
            } else {
                Some(format!(
                    "{common} candidate {candidate} refY {} extDy {} standsOut {} buildAlias {} \
                     registration {} returned {}",
                    field(&words, "refY"),
                    field(&words, "extDy"),
                    field(&words, "standsOut"),
                    field(&words, "buildAlias"),
                    field(&words, "registration")
                        .split_once(':')
                        .map_or(field(&words, "registration"), |pair| pair.0),
                    field(&words, "returned"),
                ))
            }
        }
        _ => None,
    }
}

fn field<'a>(words: &'a [&str], name: &str) -> &'a str {
    let index = words
        .iter()
        .position(|&word| word == name)
        .unwrap_or_else(|| panic!("missing {name} in {words:?}"));
    words[index + 1]
}

fn source_section<'a>(
    grid: &'a audiveris_omr::recognize::GridLinesRecognition,
    system: &NativeStemsHeadStumpSystem,
    source_ordinal: usize,
) -> &'a Section {
    &grid.peak_graph.vertical_sections[system.vertical_section_source_ordinals[source_ordinal]]
}

fn bounds(bounds: Bounds) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
}

fn optional_bounds(bounds: Option<Bounds>) -> String {
    bounds.map_or_else(|| "-".to_owned(), self::bounds)
}

fn optional_ordinal(ordinal: Option<usize>) -> String {
    ordinal.map_or_else(|| "-".to_owned(), |ordinal| ordinal.to_string())
}

fn optional_ordinals(ordinals: &[usize]) -> String {
    if ordinals.is_empty() {
        "-".to_owned()
    } else {
        ordinals
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn rect(bounds: NativeStemRect) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(bounds.x),
        java_hex_double(bounds.y),
        java_hex_double(bounds.width),
        java_hex_double(bounds.height)
    )
}

fn point(point: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(point.x), java_hex_double(point.y))
}

fn corner_name(horizontal: NativeStemHeadSide, vertical: NativeStemVerticalSide) -> &'static str {
    match (horizontal, vertical) {
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top) => "TL",
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom) => "BL",
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top) => "TR",
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom) => "BR",
    }
}

const fn horizontal_direction(side: NativeStemHeadSide) -> i32 {
    match side {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    }
}

const fn vertical_direction(side: NativeStemVerticalSide) -> i32 {
    match side {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    }
}

fn section_run_hash(section: &Section) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let orientation = match section.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    hash_row(&mut hash, &format!("{orientation} {}", section.first_pos()));
    for run in section.runs() {
        hash_row(&mut hash, &format!("{}:{}", run.start, run.length));
    }
    hash
}

fn owned_section_run_hash(first_pos: i32, runs: &[Run]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_row(&mut hash, &format!("VERTICAL {first_pos}"));
    for run in runs {
        hash_row(&mut hash, &format!("{}:{}", run.start, run.length));
    }
    hash
}

fn hash_row(hash: &mut u64, row: &str) {
    for byte in row.bytes().chain([b'\n']) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn to_pixels(interline: i32, fraction: f64) -> i32 {
    (f64::from(interline) * fraction).round_ties_even() as i32
}

fn java_hex_double(value: f64) -> String {
    let raw_bits = value.to_bits();
    let canonical_bits = if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        raw_bits
    };
    let java = if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "Infinity".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        let sign = if (raw_bits >> 63) != 0 { "-" } else { "" };
        let exponent_bits = ((raw_bits >> 52) & 0x7ff) as i32;
        let fraction_bits = raw_bits & 0x000f_ffff_ffff_ffff;
        if exponent_bits == 0 && fraction_bits == 0 {
            format!("{sign}0x0.0p0")
        } else {
            let mut fraction = format!("{fraction_bits:013x}");
            while fraction.len() > 1 && fraction.ends_with('0') {
                fraction.pop();
            }
            if exponent_bits == 0 {
                format!("{sign}0x0.{fraction}p-1022")
            } else {
                format!("{sign}0x1.{fraction}p{}", exponent_bits - 1023)
            }
        }
    };
    format!("{java}/{canonical_bits:016x}")
}

fn report_first_mismatches(page: &str, actual: &[String], expected: &[String]) {
    let count = actual.len().max(expected.len());
    let mut shown = 0;
    for index in 0..count {
        let actual = actual.get(index).map(String::as_str).unwrap_or("<missing>");
        let expected = expected
            .get(index)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if actual != expected {
            eprintln!("{page} row {index}\n  actual:   {actual}\n  expected: {expected}");
            shown += 1;
            if shown == 8 {
                break;
            }
        }
    }
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}
