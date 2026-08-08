// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use audiveris_image::{glyph_factory::GlyphComponent, run_table::Orientation};
use audiveris_omr::{
    black_head_sizer::{
        BlackHeadCandidateDecision, BlackHeadCheckRejection, BlackHeadSizingResult,
    },
    native_headers::recognize_native_headers,
    recognize::{
        GridLinesRecognition, NativeBeamRecognition, recognize_grid_lines, recognize_native_beams,
    },
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

fn oracle_rows(prefix: &str, page: &str) -> Vec<&'static str> {
    include_str!("../../../oracle/black-head-sizer.txt")
        .lines()
        .filter(|line| {
            let mut fields = line.split_whitespace();
            fields.next() == Some(prefix) && fields.next() == Some(page)
        })
        .collect()
}

fn assert_rows(page: &str, kind: &str, expected: &[&str], actual: &[String]) {
    assert_eq!(actual.len(), expected.len(), "{page} {kind} row count");
    for (ordinal, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        assert_eq!(actual, expected, "{page} {kind} row {ordinal}");
    }
}

fn hash_rows(rows: &[String]) -> u64 {
    let mut hash = FNV_OFFSET;
    for row in rows {
        for byte in row.bytes().chain(std::iter::once(b'\n')) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn orientation_name(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    }
}

fn component_run_hash(component: &GlyphComponent) -> u64 {
    // Source components use sheet-relative runs, while a component rebuilt
    // from `Glyph.getBuffer()` retains crop-relative runs plus an absolute
    // offset. Java `Glyph.getRunTable()` crops either representation to the
    // component's own minimum run coordinates.
    let sequence_origin = component
        .runs
        .iter()
        .map(|entry| entry.sequence)
        .min()
        .expect("a component has runs");
    let coordinate_origin = component
        .runs
        .iter()
        .map(|entry| entry.run.start)
        .min()
        .expect("a component has runs");
    let sequence_count = match component.orientation {
        Orientation::Horizontal => component.height,
        Orientation::Vertical => component.width,
    };
    let mut rows = Vec::with_capacity(sequence_count + 1);
    rows.push(format!(
        "{} {} {}",
        orientation_name(component.orientation),
        component.width,
        component.height
    ));
    for local_sequence in 0..sequence_count {
        let source_sequence = sequence_origin + local_sequence;
        let mut row = local_sequence.to_string();
        for entry in component
            .runs
            .iter()
            .filter(|entry| entry.sequence == source_sequence)
        {
            let local_start = entry
                .run
                .start
                .checked_sub(coordinate_origin)
                .expect("component run starts within its bounds");
            row.push(' ');
            row.push_str(&format!("{local_start}:{}", entry.run.length));
        }
        rows.push(row);
    }
    hash_rows(&rows)
}

fn check_reason(reason: BlackHeadCheckRejection) -> &'static str {
    match reason {
        BlackHeadCheckRejection::WidthBelowMinimum => "width_low",
        BlackHeadCheckRejection::WidthAboveMaximum => "width_high",
        BlackHeadCheckRejection::WeightBelowMinimum => "weight_low",
        BlackHeadCheckRejection::WeightAboveMaximum => "weight_high",
        BlackHeadCheckRejection::HeightBelowMinimum => "height_low",
        BlackHeadCheckRejection::HeightAboveMaximum => "height_high",
    }
}

fn registration_eligible(grid: &GridLinesRecognition, glyph: &GlyphComponent) -> bool {
    grid.system_areas.iter().any(|area| {
        let bounds = grid
            .system_bounds
            .iter()
            .find(|bounds| bounds.system_id == area.system_id)
            .expect("every system area has bounds");
        let x = glyph.rounded_centroid_x;
        let y = glyph.rounded_centroid_y;
        area.contains(f64::from(x), f64::from(y)) && x >= bounds.left && x <= bounds.java_right()
    })
}

fn candidate_rows(
    page: &str,
    grid: &GridLinesRecognition,
    sizing: &BlackHeadSizingResult,
) -> Vec<String> {
    sizing
        .traces
        .iter()
        .map(|trace| {
            let source = &trace.source_component;
            let (pre, close, post, class) = match trace.decision {
                BlackHeadCandidateDecision::Rejected { phase, reason } => match phase {
                    audiveris_omr::black_head_sizer::BlackHeadCheckPhase::BeforeClosing => {
                        (check_reason(reason), "skipped", "skipped", "reject")
                    }
                    audiveris_omr::black_head_sizer::BlackHeadCheckPhase::AfterClosing => {
                        ("pass", "one_component", check_reason(reason), "reject")
                    }
                },
                BlackHeadCandidateDecision::ClosingComponentCount(_) => {
                    ("pass", "component_count_not_one", "skipped", "reject")
                }
                BlackHeadCandidateDecision::Single => {
                    ("pass", "one_component", "pass", "single")
                }
                BlackHeadCandidateDecision::Stack => {
                    ("pass", "one_component", "pass", "stack")
                }
                BlackHeadCandidateDecision::Unclassified => {
                    ("pass", "one_component", "pass", "reject_neither")
                }
            };
            let close_components = trace
                .closed_component_count
                .map_or_else(|| "none".to_owned(), |count| count.to_string());
            let (closed, registered) = trace.closed_component.as_ref().map_or_else(
                || ("none".to_owned(), false),
                |glyph| {
                    (
                        format!(
                            "{} {} {} {} {} {} {} {} {:016x}",
                            glyph.left,
                            glyph.top,
                            glyph.width,
                            glyph.height,
                            glyph.weight,
                            glyph.rounded_centroid_x,
                            glyph.rounded_centroid_y,
                            glyph.runs.len(),
                            component_run_hash(glyph)
                        ),
                        registration_eligible(grid, glyph),
                    )
                },
            );
            format!(
                "blackheadcandidate {page} ordinal {} bounds {} {} {} {} weight {} centroid {} {} runs {} {:016x} pre {pre} close {close} closeComponents {close_components} closed {closed} registered {registered} post {post} class {class}",
                trace.source_index,
                source.left,
                source.top,
                source.width,
                source.height,
                source.weight,
                source.rounded_centroid_x,
                source.rounded_centroid_y,
                source.runs.len(),
                component_run_hash(source),
            )
        })
        .collect()
}

fn class_rows(page: &str, sizing: &BlackHeadSizingResult, single: bool) -> Vec<String> {
    let mut ordinal = 0;
    sizing
        .traces
        .iter()
        .filter_map(|trace| {
            let accepted = if single {
                trace.decision == BlackHeadCandidateDecision::Single
            } else {
                trace.decision == BlackHeadCandidateDecision::Stack
            };
            if !accepted {
                return None;
            }
            let glyph = trace
                .closed_component
                .as_ref()
                .expect("accepted candidates have one closed component");
            let kind = if single { "single" } else { "stack" };
            let row = format!(
                "blackhead{kind} {page} ordinal {ordinal} source {} bounds {} {} {} {} weight {} runs {} {:016x}",
                trace.source_index,
                glyph.left,
                glyph.top,
                glyph.width,
                glyph.height,
                glyph.weight,
                glyph.runs.len(),
                component_run_hash(glyph),
            );
            ordinal += 1;
            Some(row)
        })
        .collect()
}

fn sample_rows(page: &str, sizing: &BlackHeadSizingResult) -> Vec<String> {
    let core_start = sizing.singles.len() / 4;
    let core_stop = (3 * sizing.singles.len()) / 4;
    sizing
        .singles
        .iter()
        .enumerate()
        .map(|(ordinal, single)| {
            format!(
                "blackheadsample {page} sorted {ordinal} source {} width {} height {} core {}",
                single.source_index,
                single.glyph.width,
                single.glyph.height,
                ordinal >= core_start && ordinal < core_stop,
            )
        })
        .collect()
}

fn java_hex(value: f64) -> String {
    assert!(value.is_finite(), "the measured scale is finite");
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { "" } else { "-" };
    let magnitude = bits & !(1_u64 << 63);
    if magnitude == 0 {
        return format!("{sign}0x0.0p0");
    }
    let exponent_bits = ((magnitude >> 52) & 0x7ff) as i32;
    assert_ne!(exponent_bits, 0, "no corpus scale value is subnormal");
    let exponent = exponent_bits - 1023;
    let fraction_bits = magnitude & ((1_u64 << 52) - 1);
    let mut fraction = format!("{fraction_bits:013x}");
    while fraction.ends_with('0') && fraction.len() > 1 {
        fraction.pop();
    }
    format!("{sign}0x1.{fraction}p{exponent}")
}

fn scale_rows(page: &str, sizing: &BlackHeadSizingResult) -> Vec<String> {
    let scale = sizing
        .black_head_scale
        .expect("all oracle pages exceed the singles quorum");
    let count = sizing.core_single_source_indices.len();
    vec![
        format!(
            "blackheadpopulation {page} axis width count {count} mean {} deviation {}",
            java_hex(scale.width_mean),
            java_hex(scale.width_std)
        ),
        format!(
            "blackheadpopulation {page} axis height count {count} mean {} deviation {}",
            java_hex(scale.height_mean),
            java_hex(scale.height_std)
        ),
        format!(
            "blackheadscale {page} widthMean {} widthDeviation {} heightMean {} heightDeviation {}",
            java_hex(scale.width_mean),
            java_hex(scale.width_std),
            java_hex(scale.height_mean),
            java_hex(scale.height_std),
        ),
    ]
}

fn staff_rows(page: &str, beams: &NativeBeamRecognition) -> Vec<String> {
    beams
        .staff_head_point_sizes
        .iter()
        .map(|staff| {
            format!(
                "blackheadstaff {page} system {} staff {} specificInterline {} headPointSize {}",
                staff.system_id, staff.staff_id, staff.specific_interline, staff.point_size
            )
        })
        .collect()
}

#[test]
fn native_black_head_sizing_matches_java_on_every_beam_sheet() {
    let pages = [
        ("chula.png#1", "chula.png"),
        ("allegretto.png#1", "allegretto.png"),
        ("batuque.png#1", "batuque.png"),
        ("carmen.png#1", "carmen.png"),
        ("cucaracha.png#1", "cucaracha.png"),
        ("hove.png#1", "hove.png"),
        ("zizi.png#1", "zizi.png"),
        ("BachInvention5.jpg#1", "BachInvention5.jpg"),
    ];
    let mut totals = (0_usize, 0_usize, 0_usize, 0_usize, 0_usize);

    for (page, file) in pages {
        let grid = recognize_grid_lines(repo_path(&format!("data/examples/{file}")))
            .unwrap_or_else(|error| panic!("{page}: GRID failed: {error}"));
        let headers = recognize_native_headers(&grid)
            .unwrap_or_else(|error| panic!("{page}: HEADERS failed: {error}"));
        let beams = recognize_native_beams(&grid, headers.beam_erases())
            .unwrap_or_else(|error| panic!("{page}: BEAMS failed: {error}"));
        let sizing = &beams.black_head_sizing;

        let candidates = candidate_rows(page, &grid, sizing);
        assert_rows(
            page,
            "candidate",
            &oracle_rows("blackheadcandidate", page),
            &candidates,
        );
        let singles = class_rows(page, sizing, true);
        assert_rows(
            page,
            "single",
            &oracle_rows("blackheadsingle", page),
            &singles,
        );
        let stacks = class_rows(page, sizing, false);
        assert_rows(page, "stack", &oracle_rows("blackheadstack", page), &stacks);
        let samples = sample_rows(page, sizing);
        assert_rows(
            page,
            "sample",
            &oracle_rows("blackheadsample", page),
            &samples,
        );

        let scale = scale_rows(page, sizing);
        let mut expected_scale = oracle_rows("blackheadpopulation", page);
        expected_scale.extend(oracle_rows("blackheadscale", page));
        assert_rows(page, "scale", &expected_scale, &scale);

        let music_font = beams
            .music_font_scale
            .expect("all oracle pages install a MusicFontScale");
        let font = vec![format!(
            "blackheadfont {page} family Bravura name Bravura pointSize {}",
            music_font.point_size
        )];
        assert_rows(page, "font", &oracle_rows("blackheadfont", page), &font);
        let staff = staff_rows(page, &beams);
        assert_rows(page, "staff", &oracle_rows("blackheadstaff", page), &staff);

        totals.0 += candidates.len();
        totals.1 += singles.len();
        totals.2 += stacks.len();
        totals.3 += sizing.core_single_source_indices.len();
        totals.4 += staff.len();
    }

    assert_eq!(totals, (2_739, 936, 5, 470, 55));
}
