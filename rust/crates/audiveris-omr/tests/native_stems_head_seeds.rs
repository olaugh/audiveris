// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page exact differential for the identity-free STEMS head-seed boundary.
//!
//! Java2D `Area` path winding/segments/path hashes are intentionally projected
//! out: the native boundary owns the exact ribbon bounds and clipped-intersection
//! evidence, but not a Java2D path object. Consequently Java's full-row summary
//! hash is also projected out. Every other reproducible row field is compared as
//! text, including the raw bits of every double. A projected FNV-1a hash is
//! compared per page in addition to row equality.

use std::{cmp::Ordering, path::PathBuf};

use audiveris_image::{
    beam_structure::Segment,
    system_population::{BoundarySegment, PopulationSystemArea},
};
use audiveris_omr::{
    beam_recognizer::run_table_center_line,
    head_scanner_slices::JavaRectangle,
    head_template::{HeadTemplateFamily, HeadTemplateShape},
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::{
        NativeStemSeedGlyph, NativeStemSeedSystemRecognition, recognize_native_stem_seeds,
    },
    native_stems_head_corners::{
        NativeStemsHeadCorner, NativeStemsHeadCornerHead, NativeStemsHeadCornerSystem,
        materialize_native_stems_head_corners,
    },
    native_stems_head_seeds::{
        NativeStemsHeadSeedCorner, NativeStemsHeadSeedHead, NativeStemsHeadSeedSystem,
        NativeStemsSeedPurge, materialize_native_stems_head_seeds,
    },
    recognize::{
        GridLinesRecognition, recognize_grid_lines, recognize_native_beams_with_stem_seeds,
    },
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemRect, NativeStemVerticalSide},
};

const ORACLE_PATH: &str = "rust/oracle/stems-head-seeds.txt";
const MAX_BAR_OVERLAP: f64 = 0.25;

#[test]
fn native_stems_head_seeds_match_java_corpus_exactly() {
    let oracle = std::fs::read_to_string(repo_path(ORACLE_PATH)).expect("head-seed oracle");
    assert!(oracle.contains(
        "stemsheadseedcorpussummary pages 8 systems 30 sourceSeeds 1906 keptSeeds 1749 \
         purgedSeeds 157 noStemAreas 483 purgeChecks 29394 heads 3521 corners 14084 \
         neighborRows 36736 candidates 7114 visited 7005 selected 4182 fallback 9902 \
         probeSourceSha256 d4ab3d3145673bbeb09194e01a227e2ec3422429a3fb6b079359519af8bea115 \
         emittedBodySha256 fea0a7a0012a27592c570f7fced0ba9f9e9955b71c196255835a6e5da88965e8"
    ));
    let pages = oracle
        .lines()
        .filter_map(|line| line.strip_prefix("stemsheadseedpage "))
        .map(|line| line.split_whitespace().next().expect("page token"))
        .collect::<Vec<_>>();
    assert_eq!(pages.len(), 8);

    let mut corpus = Totals::default();
    for page in pages {
        let (actual, totals) = native_page_rows(page);
        let expected = projected_oracle_page_rows(&oracle, page);
        if actual != expected {
            eprintln!(
                "{page}: projected hashes actual {:016x}, expected {:016x}",
                fnv_rows(&actual),
                fnv_rows(&expected)
            );
            report_first_mismatches(page, &actual, &expected);
            panic!("{page}: projected native STEMS head-seed rows differ from Java");
        }
        assert_eq!(
            fnv_rows(&actual),
            fnv_rows(&expected),
            "{page}: projected semantic hash"
        );
        corpus.include(totals);
    }

    assert_eq!(
        corpus,
        Totals {
            pages: 8,
            systems: 30,
            source_seeds: 1_906,
            kept_seeds: 1_749,
            purged_seeds: 157,
            no_stem_areas: 483,
            purge_checks: 29_394,
            heads: 3_521,
            corners: 14_084,
            neighbor_rows: 36_736,
            candidates: 7_114,
            visited: 7_005,
            selected: 4_182,
            fallback: 9_902,
        }
    );
}

fn native_page_rows(page: &str) -> (Vec<String>, Totals) {
    let image = page.strip_suffix("#1").expect("page sheet suffix");
    let grid = recognize_grid_lines(repo_path(&format!("data/examples/{image}")))
        .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
    let headers = recognize_native_headers(&grid)
        .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers)
        .unwrap_or_else(|error| panic!("{page}: STEM_SEEDS failed: {error}"));
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
    let ledgers = recognize_native_ledgers(&grid, &beams)
        .unwrap_or_else(|error| panic!("{page}: LEDGERS failed: {error}"));
    let heads = recognize_native_heads(&grid, &headers, &stem_seeds, &beams, &ledgers)
        .unwrap_or_else(|error| panic!("{page}: HEADS failed: {error}"));
    let corner_product = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: STEMS corners failed: {error}"));
    let seed_product = materialize_native_stems_head_seeds(&grid, &stem_seeds, &corner_product)
        .unwrap_or_else(|error| panic!("{page}: STEMS head seeds failed: {error}"));

    let mut rows = vec![format!(
        "stemsheadseedpage {page} systems {} staves {} family {}",
        seed_product.systems.len(),
        heads.prolog.staff_template_catalogs.len(),
        family_name(
            heads
                .prolog
                .template_catalogs
                .first()
                .expect("template catalog")
                .family()
        )
    )];
    let mut totals = Totals {
        pages: 1,
        ..Totals::default()
    };

    for system in &seed_product.systems {
        let system_id = system.system_id;
        let area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .expect("system area");
        let area_bounds = population_system_area_integer_bounds(area);
        let seed_system = stem_seeds
            .systems
            .iter()
            .find(|candidate| candidate.raw.system_id == system_id)
            .expect("seed system");
        let corner_system = corner_product
            .systems
            .iter()
            .find(|candidate| candidate.system_id == system_id)
            .expect("corner system");
        let system_totals = append_system_rows(
            page,
            &grid,
            area_bounds,
            seed_system,
            corner_system,
            system,
            &mut rows,
        );
        totals.include(system_totals);
    }

    assert_eq!(seed_product.input_seed_count, totals.source_seeds);
    assert_eq!(seed_product.kept_seed_count, totals.kept_seeds);
    assert_eq!(seed_product.purged_seed_count, totals.purged_seeds);
    assert_eq!(seed_product.corner_count, totals.corners);
    assert_eq!(seed_product.selected_seed_count, totals.selected);
    assert_eq!(seed_product.section_fallback_count, totals.fallback);
    assert_eq!(seed_product.visited_candidate_count, totals.visited);
    rows.push(page_summary(page, &totals));
    (rows, totals)
}

#[allow(clippy::too_many_arguments)]
fn append_system_rows(
    page: &str,
    _grid: &GridLinesRecognition,
    area_bounds: JavaRectangle,
    seed_system: &NativeStemSeedSystemRecognition,
    corner_system: &NativeStemsHeadCornerSystem,
    system: &NativeStemsHeadSeedSystem,
    rows: &mut Vec<String>,
) -> Totals {
    let interline = corner_system.interline;
    let vicinity_margin = to_pixels(interline, 1.0);
    let max_head_seed_dy = to_pixels(interline, 0.25);
    let min_head_stump_dy = to_pixels(interline, 0.5);
    rows.push(format!(
        "stemsheadseedsystem {page} system {} profile {} interline {interline} bounds {} \
         maxBarOverlap {} vicinityMargin {vicinity_margin} maxHeadSeedDy {max_head_seed_dy} \
         maxHeadInDx {} maxHeadOutDx {} minHeadStumpDy {min_head_stump_dy} sourceSeeds {} \
         keptSeeds {} noStemAreas {} heads {}",
        system.system_id,
        corner_system.profile,
        java_rectangle(area_bounds),
        java_hex_double(MAX_BAR_OVERLAP),
        corner_system.max_head_in_dx,
        corner_system.max_head_out_dx,
        system.seeds_in_free_order.len(),
        system.kept_seed_ordinals.len(),
        system.no_stem_areas.len(),
        system.heads_by_abscissa.len()
    ));

    for area in &system.no_stem_areas {
        rows.push(format!(
            "stemsheadseednostem {page} system {} ordinal {} bounds {}",
            system.system_id,
            area.ordinal,
            rect(area.bounds)
        ));
    }

    let mut totals = Totals {
        systems: 1,
        source_seeds: system.seeds_in_free_order.len(),
        kept_seeds: system.kept_seed_ordinals.len(),
        purged_seeds: system.seeds_in_free_order.len() - system.kept_seed_ordinals.len(),
        no_stem_areas: system.no_stem_areas.len(),
        heads: system.heads_by_abscissa.len(),
        ..Totals::default()
    };
    for purge in &system.seeds_in_free_order {
        append_seed_rows(page, system, seed_system, purge, rows, &mut totals);
    }

    for head in &system.heads_by_abscissa {
        let corner_head = corner_system
            .heads_in_sig_order
            .get(head.sig_ordinal)
            .expect("SIG head ordinal");
        append_head_rows(
            page,
            area_bounds,
            seed_system,
            corner_system,
            system,
            head,
            corner_head,
            rows,
            &mut totals,
        );
    }
    assert_eq!(totals.corners, totals.heads * 4);
    assert_eq!(totals.corners, totals.selected + totals.fallback);
    rows.push(system_summary(page, system.system_id, &totals));
    totals
}

fn append_seed_rows(
    page: &str,
    system: &NativeStemsHeadSeedSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    purge: &NativeStemsSeedPurge,
    rows: &mut Vec<String>,
    totals: &mut Totals,
) {
    let seed = &seed_system.free_glyphs[purge.free_glyph_ordinal];
    let center_line = fixed_center_line(seed);
    assert_eq!(purge.bounds.x as usize, seed.bounds.x);
    assert_eq!(purge.bounds.y as usize, seed.bounds.y);
    assert_eq!(purge.bounds.width as usize, seed.bounds.width);
    assert_eq!(purge.bounds.height as usize, seed.bounds.height);
    rows.push(format!(
        "stemsheadseedfree {page} system {} sourceOrdinal {} kept {} keptOrdinal {} bounds {} \
         weight {} runs {}:{:016x} centerLine {}",
        system.system_id,
        purge.free_glyph_ordinal,
        purge.kept_ordinal.is_some(),
        optional_usize(purge.kept_ordinal),
        java_rectangle(purge.bounds),
        purge.weight,
        seed.run_count(),
        purge.run_digest,
        line(center_line)
    ));

    let checked_areas = purge
        .first_purging_area
        .map_or(system.no_stem_areas.len(), |ordinal| ordinal + 1);
    totals.purge_checks += checked_areas;
    let mut intersections = purge.intersections.iter().peekable();
    for area_ordinal in 0..checked_areas {
        let area = &system.no_stem_areas[area_ordinal];
        let intersection =
            intersections.next_if(|intersection| intersection.area_ordinal == area_ordinal);
        if let Some(intersection) = intersection {
            let intersection_size =
                intersection.intersection_bounds.width * intersection.intersection_bounds.height;
            let denominator = intersection.bar_bounds_size.min(intersection.seed_size);
            rows.push(format!(
                "stemsheadseedpurgecheck {page} system {} sourceOrdinal {} checkOrdinal \
                 {area_ordinal} areaOrdinal {area_ordinal} intersects true seedSize {} interBounds \
                 {} interSize {} barBounds {} barSize {} denominator {} ratio {} threshold {} \
                 purge {}",
                system.system_id,
                purge.free_glyph_ordinal,
                java_hex_double(intersection.seed_size),
                rect(intersection.intersection_bounds),
                java_hex_double(intersection_size),
                rect(area.bounds),
                java_hex_double(intersection.bar_bounds_size),
                java_hex_double(denominator),
                java_hex_double(intersection.ratio),
                java_hex_double(MAX_BAR_OVERLAP),
                intersection.purges
            ));
        } else {
            let seed_size = f64::from(purge.bounds.width) * f64::from(purge.bounds.height);
            rows.push(format!(
                "stemsheadseedpurgecheck {page} system {} sourceOrdinal {} checkOrdinal \
                 {area_ordinal} areaOrdinal {area_ordinal} intersects false seedSize {} purge false",
                system.system_id,
                purge.free_glyph_ordinal,
                java_hex_double(seed_size)
            ));
        }
    }
    assert!(
        intersections.next().is_none(),
        "unvisited native intersection"
    );
    rows.push(format!(
        "stemsheadseedpurgedecision {page} system {} sourceOrdinal {} checkedAreas \
         {checked_areas} outcome {}",
        system.system_id,
        purge.free_glyph_ordinal,
        purge.first_purging_area.map_or_else(
            || format!("kept:{}", purge.kept_ordinal.expect("kept ordinal")),
            |ordinal| format!("purged:{ordinal}")
        )
    ));
}

#[allow(clippy::too_many_arguments)]
fn append_head_rows(
    page: &str,
    area_bounds: JavaRectangle,
    seed_system: &NativeStemSeedSystemRecognition,
    corner_system: &NativeStemsHeadCornerSystem,
    system: &NativeStemsHeadSeedSystem,
    head: &NativeStemsHeadSeedHead,
    corner_head: &NativeStemsHeadCornerHead,
    rows: &mut Vec<String>,
    totals: &mut Totals,
) {
    assert_eq!(head.x_ordinal, totals.corners / 4);
    assert_eq!(
        head.system_creation_ordinal,
        corner_head.system_creation_ordinal
    );
    let vicinity_margin = to_pixels(corner_system.interline, 1.0);
    let fat_box = (
        corner_head.bounds.x - vicinity_margin,
        area_bounds.y,
        corner_head.bounds.width + (2 * vicinity_margin),
        area_bounds.height,
    );
    let neighbor_count = head
        .corners_in_constructor_order
        .first()
        .map_or(0, |corner| corner.neighbor_seed_ordinals.len());
    assert!(
        head.corners_in_constructor_order
            .iter()
            .all(|corner| { corner.neighbor_seed_ordinals.len() == neighbor_count })
    );
    rows.push(format!(
        "stemsheadseedhead {page} system {} ordinal {} sigOrdinal {} staff {} shape {} bounds {} \
         grade {} fatBox {}:{}:{}:{} neighbors {neighbor_count}",
        system.system_id,
        head.x_ordinal,
        head.sig_ordinal,
        corner_head.staff_id,
        shape_name(corner_head.shape),
        java_rectangle(corner_head.bounds),
        java_hex_double(f64::from_bits(corner_head.grade_bits)),
        fat_box.0,
        fat_box.1,
        fat_box.2,
        fat_box.3
    ));

    for (corner, geometry) in head
        .corners_in_constructor_order
        .iter()
        .zip(&corner_head.corners_in_constructor_order)
    {
        append_corner_rows(
            page,
            system,
            seed_system,
            head,
            corner,
            geometry,
            to_pixels(corner_system.interline, 0.5),
            rows,
            totals,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_corner_rows(
    page: &str,
    system: &NativeStemsHeadSeedSystem,
    seed_system: &NativeStemSeedSystemRecognition,
    head: &NativeStemsHeadSeedHead,
    corner: &NativeStemsHeadSeedCorner,
    geometry: &NativeStemsHeadCorner,
    min_head_stump_dy: i32,
    rows: &mut Vec<String>,
    totals: &mut Totals,
) {
    assert_eq!(corner.constructor_ordinal, geometry.constructor_ordinal);
    let x_direction = match geometry.horizontal {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    };
    let y_direction = match geometry.vertical {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    };
    let mut candidates = corner
        .intersected_seed_ordinals
        .iter()
        .enumerate()
        .map(|(input_ordinal, &free_glyph_ordinal)| {
            let seed = &seed_system.free_glyphs[free_glyph_ordinal];
            let center_line = fixed_center_line(seed);
            Candidate {
                free_glyph_ordinal,
                input_ordinal,
                distance_sq: point_segment_distance_sq(
                    (center_line.x1, center_line.y1),
                    (center_line.x2, center_line.y2),
                    geometry.reference,
                ),
                center_line,
                seed,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.distance_sq
            .partial_cmp(&right.distance_sq)
            .unwrap_or(Ordering::Equal)
    });
    let selected_sort_ordinal = corner.selected_seed_ordinal.and_then(|selected| {
        candidates
            .iter()
            .position(|candidate| candidate.free_glyph_ordinal == selected)
    });
    let visited = selected_sort_ordinal.map_or(candidates.len(), |ordinal| ordinal + 1);
    assert_eq!(corner.checks.len(), candidates.len());
    assert_eq!(
        corner.checks.iter().filter(|check| check.visited).count(),
        visited
    );

    totals.corners += 1;
    totals.neighbor_rows += corner.neighbor_seed_ordinals.len();
    totals.candidates += candidates.len();
    totals.visited += visited;
    if corner.selected_seed_ordinal.is_some() {
        totals.selected += 1;
    } else {
        totals.fallback += 1;
    }
    assert_eq!(
        corner.section_fallback_required,
        corner.selected_seed_ordinal.is_none()
    );

    rows.push(format!(
        "stemsheadseedcorner {page} system {} head {} ctorOrdinal {} inspectOrdinal {} corner {} \
         xDir {x_direction} yDir {y_direction} ref {} out {} in {} seedRect {} neighbors {} \
         intersected {} candidates {}",
        system.system_id,
        head.x_ordinal,
        geometry.constructor_ordinal,
        geometry.inspection_ordinal,
        corner_name(geometry.horizontal, geometry.vertical),
        point(geometry.reference),
        point(geometry.out_point),
        point(geometry.in_point),
        rect(corner.seed_area),
        corner.neighbor_seed_ordinals.len(),
        corner.intersected_seed_ordinals.len(),
        candidates.len()
    ));

    for (neighbor_ordinal, &free_glyph_ordinal) in corner.neighbor_seed_ordinals.iter().enumerate()
    {
        let intersection_ordinal = corner
            .intersected_seed_ordinals
            .iter()
            .position(|&candidate| candidate == free_glyph_ordinal);
        let kept_ordinal = system
            .kept_seed_ordinals
            .iter()
            .position(|&candidate| candidate == free_glyph_ordinal)
            .expect("neighbor kept ordinal");
        let seed = &seed_system.free_glyphs[free_glyph_ordinal];
        rows.push(format!(
            "stemsheadseedneighbor {page} system {} head {} ctorOrdinal {} neighborOrdinal \
             {neighbor_ordinal} sourceOrdinal {free_glyph_ordinal} keptOrdinal {kept_ordinal} bounds \
             {} intersects {} intersectionOrdinal {}",
            system.system_id,
            head.x_ordinal,
            geometry.constructor_ordinal,
            bounds(seed),
            intersection_ordinal.is_some(),
            optional_usize(intersection_ordinal)
        ));
    }

    for (sort_ordinal, candidate) in candidates.iter().enumerate() {
        let seed_x = java_line_x_at_y(
            (candidate.center_line.x1, candidate.center_line.y1),
            (candidate.center_line.x2, candidate.center_line.y2),
            geometry.reference.y,
        );
        let signed_delta = f64::from(x_direction) * (seed_x - geometry.reference.x);
        let rounded_dx = java_round_to_i32(signed_delta);
        let horizontal_gate = (rounded_dx >= 0 && rounded_dx <= system_max_out(geometry))
            || (rounded_dx <= 0 && -rounded_dx <= system_max_in(geometry));
        let ref_y = geometry.reference.y.round_ties_even() as i32;
        let seed_bounds = candidate.seed.bounds;
        let extension_dy = match geometry.vertical {
            NativeStemVerticalSide::Bottom => {
                seed_bounds.y as i32 + seed_bounds.height as i32 - 1 - ref_y
            }
            NativeStemVerticalSide::Top => ref_y - seed_bounds.y as i32,
        };
        let stands_out = extension_dy >= min_head_stump_dy;
        let would_accept = horizontal_gate && stands_out;
        let is_visited = sort_ordinal < visited;
        let selected = selected_sort_ordinal == Some(sort_ordinal);

        let check = &corner.checks[sort_ordinal];
        assert_eq!(check.free_glyph_ordinal, candidate.free_glyph_ordinal);
        assert_eq!(check.pre_sort_ordinal, candidate.input_ordinal);
        assert_bits(check.distance_sq, candidate.distance_sq);
        assert_bits(check.seed_x, seed_x);
        assert_bits(check.signed_delta, signed_delta);
        assert_eq!(check.rounded_dx, rounded_dx);
        assert_eq!(check.ref_y, ref_y);
        assert_eq!(check.extension_dy, extension_dy);
        assert_eq!(check.horizontal_gate, horizontal_gate);
        assert_eq!(check.stands_out, stands_out);
        assert_eq!(check.stands_out_evaluated, is_visited && horizontal_gate);
        assert_eq!(check.accepted, would_accept);
        assert_eq!(check.visited, is_visited);
        assert_eq!(check.selected, selected);
        rows.push(format!(
            "stemsheadseedcandidate {page} system {} head {} ctorOrdinal {} sortOrdinal \
             {sort_ordinal} inputOrdinal {} sourceOrdinal {} keptOrdinal {} bounds {} centerLine {} \
             distanceSq {} seedX {} signedDelta {} dx {rounded_dx} refY {ref_y} extDy \
             {extension_dy} horizontalGate {horizontal_gate} standsOut {stands_out} \
             standsOutEvaluated {} wouldAccept {would_accept} visited {is_visited} selected \
             {selected}",
            system.system_id,
            head.x_ordinal,
            geometry.constructor_ordinal,
            candidate.input_ordinal,
            candidate.free_glyph_ordinal,
            system
                .kept_seed_ordinals
                .iter()
                .position(|&ordinal| ordinal == candidate.free_glyph_ordinal)
                .expect("candidate kept ordinal"),
            bounds(candidate.seed),
            line(candidate.center_line),
            java_hex_double(candidate.distance_sq),
            java_hex_double(seed_x),
            java_hex_double(signed_delta),
            is_visited && horizontal_gate
        ));
    }

    rows.push(format!(
        "stemsheadseeddecision {page} system {} head {} ctorOrdinal {} corner {} visited {visited} \
         outcome {} selectedSortOrdinal {}",
        system.system_id,
        head.x_ordinal,
        geometry.constructor_ordinal,
        corner_name(geometry.horizontal, geometry.vertical),
        corner.selected_seed_ordinal.map_or_else(
            || "fallback".to_owned(),
            |ordinal| format!("selected:{ordinal}")
        ),
        optional_usize(selected_sort_ordinal)
    ));
}

// These gaps are fixed by Java `HeadStemRelation` profile parameters in the
// frozen corpus and are already frozen by the preceding corner boundary.
fn system_max_out(geometry: &NativeStemsHeadCorner) -> i32 {
    (geometry.out_point.x - geometry.reference.x).abs() as i32
}

fn system_max_in(geometry: &NativeStemsHeadCorner) -> i32 {
    (geometry.in_point.x - geometry.reference.x).abs() as i32
}

fn projected_oracle_page_rows(oracle: &str, page: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| semantic_prefix(line).is_some())
        .filter(|line| line.split_whitespace().nth(1) == Some(page))
        .map(project_oracle_row)
        .collect()
}

fn semantic_prefix(line: &str) -> Option<&'static str> {
    [
        "stemsheadseedpage ",
        "stemsheadseedsystem ",
        "stemsheadseednostem ",
        "stemsheadseedfree ",
        "stemsheadseedpurgecheck ",
        "stemsheadseedpurgedecision ",
        "stemsheadseedhead ",
        "stemsheadseedcorner ",
        "stemsheadseedneighbor ",
        "stemsheadseedcandidate ",
        "stemsheadseeddecision ",
        "stemsheadseedsystemsummary ",
        "stemsheadseedpagesummary ",
    ]
    .into_iter()
    .find(|prefix| line.starts_with(prefix))
}

fn project_oracle_row(line: &str) -> String {
    if line.starts_with("stemsheadseednostem ") {
        line.split_once(" winding ")
            .expect("no-stem path evidence")
            .0
            .to_owned()
    } else if line.starts_with("stemsheadseedsystemsummary ")
        || line.starts_with("stemsheadseedpagesummary ")
    {
        line.split_once(" hash ")
            .expect("Java full-row summary hash")
            .0
            .to_owned()
    } else {
        line.to_owned()
    }
}

fn system_summary(page: &str, system_id: usize, totals: &Totals) -> String {
    format!(
        "stemsheadseedsystemsummary {page} system {system_id} sourceSeeds {} keptSeeds {} \
         purgedSeeds {} noStemAreas {} purgeChecks {} heads {} corners {} neighborRows {} \
         candidates {} visited {} selected {} fallback {}",
        totals.source_seeds,
        totals.kept_seeds,
        totals.purged_seeds,
        totals.no_stem_areas,
        totals.purge_checks,
        totals.heads,
        totals.corners,
        totals.neighbor_rows,
        totals.candidates,
        totals.visited,
        totals.selected,
        totals.fallback
    )
}

fn page_summary(page: &str, totals: &Totals) -> String {
    format!(
        "stemsheadseedpagesummary {page} systems {} sourceSeeds {} keptSeeds {} purgedSeeds {} \
         noStemAreas {} purgeChecks {} heads {} corners {} neighborRows {} candidates {} visited \
         {} selected {} fallback {}",
        totals.systems,
        totals.source_seeds,
        totals.kept_seeds,
        totals.purged_seeds,
        totals.no_stem_areas,
        totals.purge_checks,
        totals.heads,
        totals.corners,
        totals.neighbor_rows,
        totals.candidates,
        totals.visited,
        totals.selected,
        totals.fallback
    )
}

fn report_first_mismatches(page: &str, actual: &[String], expected: &[String]) {
    eprintln!(
        "{page}: actual rows {}, expected rows {}",
        actual.len(),
        expected.len()
    );
    for index in 0..actual.len().max(expected.len()) {
        let actual_row = actual.get(index).map(String::as_str).unwrap_or("<missing>");
        let expected_row = expected
            .get(index)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if actual_row != expected_row {
            eprintln!("row {index}\n  actual:   {actual_row}\n  expected: {expected_row}");
            if index > 20 {
                break;
            }
        }
    }
}

fn assert_bits(actual: f64, expected: f64) {
    assert_eq!(actual.to_bits(), expected.to_bits());
}

fn to_pixels(interline: i32, fraction: f64) -> i32 {
    (f64::from(interline) * fraction).round_ties_even() as i32
}

// Test-local copy of the production Java `Area.getBounds()` adapter. The
// adapter is crate-private because it is not part of the public OMR API, while
// this integration test is compiled as a separate crate.
fn population_system_area_integer_bounds(area: &PopulationSystemArea) -> JavaRectangle {
    if area.right <= area.left {
        return JavaRectangle::default();
    }
    let left = f64::from(area.left);
    let right = f64::from(area.right);
    let mut minimum_y = f64::INFINITY;
    let mut maximum_y = f64::NEG_INFINITY;
    let mut found = false;
    for segment in area.outline_segments() {
        let (start, end) = segment_endpoints(segment);
        let segment_left = start.0.min(end.0);
        let segment_right = start.0.max(end.0);
        let clipped_left = segment_left.max(left);
        let clipped_right = segment_right.min(right);
        if clipped_right < clipped_left {
            continue;
        }
        let (segment_minimum_y, segment_maximum_y) = if start.0 == end.0 {
            if start.0 < left || start.0 > right {
                continue;
            }
            segment_y_range(segment, 0.0, 1.0)
        } else {
            let mut start_t = parameter_at_x(segment, clipped_left);
            let mut end_t = parameter_at_x(segment, clipped_right);
            if start_t > end_t {
                std::mem::swap(&mut start_t, &mut end_t);
            }
            segment_y_range(segment, start_t, end_t)
        };
        minimum_y = minimum_y.min(segment_minimum_y);
        maximum_y = maximum_y.max(segment_maximum_y);
        found = true;
    }
    if !found {
        return JavaRectangle::default();
    }
    enclosing_integer_bounds(left, minimum_y, right, maximum_y)
}

fn enclosing_integer_bounds(
    minimum_x: f64,
    minimum_y: f64,
    maximum_x: f64,
    maximum_y: f64,
) -> JavaRectangle {
    let x = minimum_x.floor();
    let y = minimum_y.floor();
    let right = maximum_x.ceil();
    let bottom = maximum_y.ceil();
    JavaRectangle::new(x as i32, y as i32, (right - x) as i32, (bottom - y) as i32)
}

fn segment_y_range(segment: BoundarySegment, start_t: f64, end_t: f64) -> (f64, f64) {
    let start_y = point_at(segment, start_t).1;
    let end_y = point_at(segment, end_t).1;
    let mut minimum_y = start_y.min(end_y);
    let mut maximum_y = start_y.max(end_y);
    for parameter in segment_extrema_parameters(segment, false) {
        if parameter > start_t && parameter < end_t {
            let y = point_at(segment, parameter).1;
            minimum_y = minimum_y.min(y);
            maximum_y = maximum_y.max(y);
        }
    }
    (minimum_y, maximum_y)
}

fn segment_extrema_parameters(segment: BoundarySegment, horizontal: bool) -> Vec<f64> {
    let coordinate = |point: (f64, f64)| if horizontal { point.0 } else { point.1 };
    match segment {
        BoundarySegment::Line { .. } => Vec::new(),
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => {
            let start = coordinate(start);
            let control = coordinate(control);
            let end = coordinate(end);
            let denominator = start - control - control + end;
            if denominator == 0.0 {
                Vec::new()
            } else {
                vec![(start - control) / denominator]
            }
        }
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => {
            let start = coordinate(start);
            let control1 = coordinate(control1);
            let control2 = coordinate(control2);
            let end = coordinate(end);
            let a = end - control2 + control1 - control2 + control1 - control2 + control1 - start;
            let b = 2.0 * (control2 - control1 - control1 + start);
            let c = control1 - start;
            solve_quadratic(c, b, a)
        }
    }
}

fn solve_quadratic(c: f64, b: f64, a: f64) -> Vec<f64> {
    if a == 0.0 {
        return if b == 0.0 { Vec::new() } else { vec![-c / b] };
    }
    let mut discriminant = (b * b) - (4.0 * a * c);
    if discriminant < 0.0 {
        return Vec::new();
    }
    discriminant = discriminant.sqrt();
    if b < 0.0 {
        discriminant = -discriminant;
    }
    let q = (b + discriminant) / -2.0;
    let mut roots = vec![q / a];
    if q != 0.0 {
        roots.push(c / q);
    }
    roots
}

fn parameter_at_x(segment: BoundarySegment, x: f64) -> f64 {
    let (start, end) = segment_endpoints(segment);
    if x == start.0 {
        return 0.0;
    }
    if x == end.0 {
        return 1.0;
    }
    let increasing = start.0 < end.0;
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..=52 {
        let middle = (low + high) / 2.0;
        let middle_x = point_at(segment, middle).0;
        if (middle_x < x) == increasing {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

fn point_at(segment: BoundarySegment, t: f64) -> (f64, f64) {
    let one_minus_t = 1.0 - t;
    match segment {
        BoundarySegment::Line { start, end } => (
            (one_minus_t * start.0) + (t * end.0),
            (one_minus_t * start.1) + (t * end.1),
        ),
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => (
            (one_minus_t * one_minus_t * start.0)
                + (2.0 * one_minus_t * t * control.0)
                + (t * t * end.0),
            (one_minus_t * one_minus_t * start.1)
                + (2.0 * one_minus_t * t * control.1)
                + (t * t * end.1),
        ),
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => (
            (one_minus_t * one_minus_t * one_minus_t * start.0)
                + (3.0 * one_minus_t * one_minus_t * t * control1.0)
                + (3.0 * one_minus_t * t * t * control2.0)
                + (t * t * t * end.0),
            (one_minus_t * one_minus_t * one_minus_t * start.1)
                + (3.0 * one_minus_t * one_minus_t * t * control1.1)
                + (3.0 * one_minus_t * t * t * control2.1)
                + (t * t * t * end.1),
        ),
    }
}

const fn segment_endpoints(segment: BoundarySegment) -> ((f64, f64), (f64, f64)) {
    match segment {
        BoundarySegment::Line { start, end }
        | BoundarySegment::Quadratic { start, end, .. }
        | BoundarySegment::Cubic { start, end, .. } => (start, end),
    }
}

fn java_rectangle(bounds: JavaRectangle) -> String {
    format!(
        "{}:{}:{}:{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    )
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

fn bounds(seed: &NativeStemSeedGlyph) -> String {
    format!(
        "{}:{}:{}:{}",
        seed.bounds.x, seed.bounds.y, seed.bounds.width, seed.bounds.height
    )
}

fn point(point: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(point.x), java_hex_double(point.y))
}

fn segment(start: (f64, f64), stop: (f64, f64)) -> String {
    format!(
        "{}:{}:{}:{}",
        java_hex_double(start.0),
        java_hex_double(start.1),
        java_hex_double(stop.0),
        java_hex_double(stop.1)
    )
}

fn line(line: Segment) -> String {
    segment((line.x1, line.y1), (line.x2, line.y2))
}

fn fixed_center_line(seed: &NativeStemSeedGlyph) -> Segment {
    run_table_center_line(
        &seed.run_table,
        i32::try_from(seed.bounds.x).expect("seed left"),
        i32::try_from(seed.bounds.y).expect("seed top"),
    )
    .expect("fixed seed center line")
}

fn optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn family_name(family: HeadTemplateFamily) -> &'static str {
    match family {
        HeadTemplateFamily::Bravura => "Bravura",
    }
}

fn shape_name(shape: HeadTemplateShape) -> &'static str {
    match shape {
        HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
        HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
        HeadTemplateShape::WholeNote => "WHOLE_NOTE",
        HeadTemplateShape::Breve => "BREVE",
    }
}

fn corner_name(horizontal: NativeStemHeadSide, vertical: NativeStemVerticalSide) -> &'static str {
    match (horizontal, vertical) {
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top) => "TL",
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom) => "BL",
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top) => "TR",
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom) => "BR",
    }
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

/// OpenJDK `Line2D.ptSegDistSq`, retaining its operation order.
fn point_segment_distance_sq(start: (f64, f64), stop: (f64, f64), point: NativeStemPoint) -> f64 {
    let mut x = point.x - start.0;
    let mut y = point.y - start.1;
    let px = stop.0 - start.0;
    let py = stop.1 - start.1;
    let mut dot_product = (x * px) + (y * py);
    let projection_sq = if dot_product <= 0.0 {
        0.0
    } else {
        x = px - x;
        y = py - y;
        dot_product = (x * px) + (y * py);
        if dot_product <= 0.0 {
            0.0
        } else {
            (dot_product * dot_product) / ((px * px) + (py * py))
        }
    };
    (((x * x) + (y * y)) - projection_sq).max(0.0)
}

/// Java `LineUtil.xAtY` determinant intersection helper.
fn java_line_x_at_y(start: (f64, f64), stop: (f64, f64), y: f64) -> f64 {
    let (x1, y1) = start;
    let (x2, y2) = stop;
    let x3 = 0.0;
    let y3 = y;
    let x4 = 1_000.0;
    let y4 = y;
    let denominator = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    let value12 = (x1 * y2) - (y1 * x2);
    let value34 = (x3 * y4) - (y3 * x4);
    ((value12 * (x3 - x4)) - ((x1 - x2) * value34)) / denominator
}

fn java_round_to_i32(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

fn fnv_rows(rows: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for row in rows {
        for byte in row.bytes().chain(std::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}

struct Candidate<'a> {
    free_glyph_ordinal: usize,
    input_ordinal: usize,
    distance_sq: f64,
    center_line: Segment,
    seed: &'a NativeStemSeedGlyph,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Totals {
    pages: usize,
    systems: usize,
    source_seeds: usize,
    kept_seeds: usize,
    purged_seeds: usize,
    no_stem_areas: usize,
    purge_checks: usize,
    heads: usize,
    corners: usize,
    neighbor_rows: usize,
    candidates: usize,
    visited: usize,
    selected: usize,
    fallback: usize,
}

impl Totals {
    fn include(&mut self, other: Self) {
        self.pages += other.pages;
        self.systems += other.systems;
        self.source_seeds += other.source_seeds;
        self.kept_seeds += other.kept_seeds;
        self.purged_seeds += other.purged_seeds;
        self.no_stem_areas += other.no_stem_areas;
        self.purge_checks += other.purge_checks;
        self.heads += other.heads;
        self.corners += other.corners;
        self.neighbor_rows += other.neighbor_rows;
        self.candidates += other.candidates;
        self.visited += other.visited;
        self.selected += other.selected;
        self.fallback += other.fallback;
    }
}
