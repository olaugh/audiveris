// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eight-page exact differential for the first production STEMS boundary.

use std::path::PathBuf;

use audiveris_omr::{
    head_template::{HeadTemplateAnchor, HeadTemplateFamily, HeadTemplateShape},
    native_headers::recognize_native_headers,
    native_heads::recognize_native_heads,
    native_ledgers::recognize_native_ledgers,
    native_stem_seeds::recognize_native_stem_seeds,
    native_stems_head_corners::{
        NativeStemsHeadCorner, NativeStemsHeadCornerRecognition,
        materialize_native_stems_head_corners,
    },
    recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds},
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemVerticalSide},
};

const ORACLE: &str = include_str!("../../../oracle/stems-head-corners.txt");

#[test]
fn native_stems_head_corners_match_java_corpus_exactly() {
    let expected = semantic_rows(ORACLE);
    let pages = expected
        .iter()
        .filter_map(|row| row.strip_prefix("stemcornerpage "))
        .map(|row| row.split_whitespace().next().expect("page token"))
        .collect::<Vec<_>>();
    assert_eq!(pages.len(), 8);

    let mut actual = Vec::new();
    let mut totals = [0_usize; 3];
    for page in pages {
        append_native_page_rows(page, &mut actual, &mut totals);
    }

    assert_eq!(totals, [30, 3_521, 14_084]);
    assert_eq!(actual, expected);
    assert!(ORACLE.contains(
        "stemcornercorpussummary pages 8 systems 30 heads 3521 corners 14084 \
         probeSourceSha256 4180de0596c3580fbef45ee12b6ec05f0dee17ef9e7267531e62efabb28d9c40 \
         emittedBodySha256 485544ae74a08d2a4d5c2a0de0030db67eec0086bd370d4eb6e2680917d0572a"
    ));
}

fn append_native_page_rows(page: &str, rows: &mut Vec<String>, totals: &mut [usize; 3]) {
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
    let stems = materialize_native_stems_head_corners(&heads, &stem_seeds)
        .unwrap_or_else(|error| panic!("{page}: STEMS corners failed: {error}"));

    rows.push(format!(
        "stemcornerpage {page} systems {} staves {} family {} headSeedScale {}",
        stems.systems.len(),
        heads.prolog.staff_template_catalogs.len(),
        family_name(
            heads
                .prolog
                .template_catalogs
                .first()
                .expect("catalog")
                .family()
        ),
        !heads.epilog.scale_analysis.is_empty()
    ));
    append_system_rows(page, &heads, &stems, rows);

    totals[0] += stems.systems.len();
    totals[1] += stems.head_count;
    totals[2] += stems.corner_count;
}

fn append_system_rows(
    page: &str,
    heads: &audiveris_omr::native_heads::NativeHeadsRecognition,
    stems: &NativeStemsHeadCornerRecognition,
    rows: &mut Vec<String>,
) {
    for system in &stems.systems {
        rows.push(format!(
            "stemcornersystem {page} system {} profile {} interline {} maxHeadInDx {} \
             maxHeadOutDx {} heads {}",
            system.system_id,
            system.profile,
            system.interline,
            system.max_head_in_dx,
            system.max_head_out_dx,
            system.heads_in_sig_order.len()
        ));
        for (ordinal, &sig_ordinal) in system.heads_by_abscissa.iter().enumerate() {
            let head = &system.heads_in_sig_order[sig_ordinal];
            let catalog = &heads.prolog.template_catalogs[head.catalog_ordinal];
            let template = catalog.template(head.shape);
            let slim = template.slim_bounds();
            let bounds = head.bounds;
            let glyph = head.glyph_bounds;
            rows.push(format!(
                "stemcornerhead {page} system {} ordinal {ordinal} sigOrdinal {sig_ordinal} staff \
                 {} shape {} bounds {}:{}:{}:{} grade {} glyph bounds:{}:{}:{}:{}:weight:{}:runs:{:016x} \
                 catalog {}:{} template {}:{}:{}:{}:{}:slim:{}:{}:{}:{}",
                system.system_id,
                head.staff_id,
                shape_name(head.shape),
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                java_hex_double(f64::from_bits(head.grade_bits)),
                glyph.x,
                glyph.y,
                glyph.width,
                glyph.height,
                head.glyph_weight,
                head.glyph_run_digest,
                family_name(catalog.family()),
                head.point_size,
                shape_name(head.shape),
                family_name(template.family()),
                template.point_size(),
                template.width(),
                template.height(),
                slim.x,
                slim.y,
                slim.width,
                slim.height
            ));
            for corner in &head.corners_in_constructor_order {
                rows.push(corner_row(page, system.system_id, ordinal, corner));
            }
        }
    }
}

fn corner_row(
    page: &str,
    system_id: usize,
    head_ordinal: usize,
    corner: &NativeStemsHeadCorner,
) -> String {
    let bounds = corner.template_bounds;
    let uncorrected = NativeStemPoint {
        x: f64::from(bounds.x) + f64::from(corner.anchor_offset.0),
        y: f64::from(bounds.y) + f64::from(corner.anchor_offset.1),
    };
    format!(
        "stemcorner {page} system {system_id} head {head_ordinal} ctorOrdinal {} inspectOrdinal {} \
         corner {} anchor {} templateBounds {}:{}:{}:{} offset {}:{} seedDx {}:{} appliedDx {} \
         uncorrected {} ref {} out {} in {}",
        corner.constructor_ordinal,
        corner.inspection_ordinal,
        corner_name(corner.horizontal, corner.vertical),
        anchor_name(corner.anchor),
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        corner.anchor_offset.0,
        corner.anchor_offset.1,
        optional_double(corner.seed_dx_left),
        optional_double(corner.seed_dx_right),
        optional_double(corner.applied_seed_dx),
        point(uncorrected),
        point(corner.reference),
        point(corner.out_point),
        point(corner.in_point)
    )
}

fn semantic_rows(oracle: &str) -> Vec<String> {
    oracle
        .lines()
        .filter(|line| {
            line.starts_with("stemcornerpage ")
                || line.starts_with("stemcornersystem ")
                || line.starts_with("stemcornerhead ")
                || line.starts_with("stemcorner ")
        })
        .map(str::to_owned)
        .collect()
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

fn anchor_name(anchor: HeadTemplateAnchor) -> &'static str {
    match anchor {
        HeadTemplateAnchor::TopLeftStem => "TOP_LEFT_STEM",
        HeadTemplateAnchor::BottomLeftStem => "BOTTOM_LEFT_STEM",
        HeadTemplateAnchor::TopRightStem => "TOP_RIGHT_STEM",
        HeadTemplateAnchor::BottomRightStem => "BOTTOM_RIGHT_STEM",
        other => panic!("unexpected STEMS anchor {other:?}"),
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

fn optional_double(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), java_hex_double)
}

fn point(point: NativeStemPoint) -> String {
    format!("{}:{}", java_hex_double(point.x), java_hex_double(point.y))
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

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(path)
}
