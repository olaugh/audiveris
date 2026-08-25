// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use audiveris_omr::head_template::{
    HeadTemplate, HeadTemplateAnchor, HeadTemplateCatalog, HeadTemplateFamily, HeadTemplateShape,
};
use audiveris_omr::head_template_catalog::{
    BRAVURA_HEAD_TEMPLATE_POINT_SIZES, load_bravura_head_template_catalogs,
};

const ORACLE: &str = include_str!("../../../oracle/head-template-catalog.txt");
const SMALL_HEADS_ORACLE: &str =
    include_str!("../../../oracle/head-template-catalog-small-heads.txt");

struct CurrentTemplate<'a> {
    page: String,
    catalog: i32,
    shape: HeadTemplateShape,
    template: &'a HeadTemplate,
    expected_anchors: usize,
    expected_pixels: usize,
    seen_anchors: usize,
    seen_pixels: usize,
}

impl CurrentTemplate<'_> {
    fn assert_complete(&self) {
        assert_eq!(
            self.seen_anchors, self.expected_anchors,
            "{} {:?} anchor count",
            self.page, self.shape
        );
        assert_eq!(
            self.seen_pixels, self.expected_pixels,
            "{} {:?} pixel count",
            self.page, self.shape
        );
    }
}

#[test]
fn native_catalog_asset_matches_every_fresh_jvm_page_record() {
    assert_oracle(ORACLE, 32, 192, 27_207);
}

#[test]
fn native_catalog_asset_matches_every_small_heads_page_record() {
    assert_oracle(SMALL_HEADS_ORACLE, 64, 384, 41_492);
}

#[test]
fn small_heads_oracle_pins_live_java_shape_census() {
    for expected in [
        "headtemplateliveheads chula.png#1 total 730 shapes NOTEHEAD_BLACK:153,NOTEHEAD_VOID:171,NOTEHEAD_BLACK_SMALL:254,NOTEHEAD_VOID_SMALL:152",
        "headtemplateliveheads allegretto.png#1 total 709 shapes NOTEHEAD_BLACK:150,NOTEHEAD_VOID:175,NOTEHEAD_BLACK_SMALL:228,NOTEHEAD_VOID_SMALL:156",
        "headtemplateliveheads batuque.png#1 total 696 shapes NOTEHEAD_BLACK:155,NOTEHEAD_VOID:170,NOTEHEAD_BLACK_SMALL:221,NOTEHEAD_VOID_SMALL:150",
        "headtemplateliveheads carmen.png#1 total 980 shapes NOTEHEAD_BLACK:191,NOTEHEAD_VOID:232,NOTEHEAD_BLACK_SMALL:367,NOTEHEAD_VOID_SMALL:190",
        "headtemplateliveheads cucaracha.png#1 total 847 shapes NOTEHEAD_BLACK:170,NOTEHEAD_VOID:230,NOTEHEAD_BLACK_SMALL:250,NOTEHEAD_VOID_SMALL:197",
        "headtemplateliveheads hove.png#1 total 719 shapes NOTEHEAD_BLACK:143,NOTEHEAD_VOID:197,NOTEHEAD_BLACK_SMALL:231,NOTEHEAD_VOID_SMALL:148",
        "headtemplateliveheads zizi.png#1 total 477 shapes NOTEHEAD_BLACK:104,NOTEHEAD_VOID:116,NOTEHEAD_BLACK_SMALL:152,NOTEHEAD_VOID_SMALL:105",
        "headtemplateliveheads BachInvention5.jpg#1 total 2480 shapes NOTEHEAD_BLACK:507,NOTEHEAD_VOID:595,NOTEHEAD_BLACK_SMALL:965,NOTEHEAD_VOID_SMALL:413",
    ] {
        assert!(
            SMALL_HEADS_ORACLE.lines().any(|line| line == expected),
            "missing exact Java live-head census: {expected}"
        );
    }
}

fn assert_oracle(
    oracle: &str,
    expected_templates: usize,
    expected_anchors: usize,
    expected_pixels: usize,
) {
    let catalogs = load_bravura_head_template_catalogs().unwrap();
    let mut current: Option<CurrentTemplate<'_>> = None;
    let mut pages = BTreeSet::new();
    let mut point_sizes = BTreeSet::new();
    let mut catalog_rows = 0;
    let mut template_rows = 0;
    let mut anchor_rows = 0;
    let mut pixel_rows = 0;

    for (line_index, line) in oracle.lines().enumerate() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some(kind) = fields.first().copied() else {
            continue;
        };
        match kind {
            "headtemplatecatalog" => {
                assert_eq!(fields[5], "Bravura");
                pages.insert(fields[1]);
                point_sizes.insert(parse_i32(fields[7], line_index));
                catalog_rows += 1;
            }
            "headtemplate" => {
                if let Some(previous) = current.take() {
                    previous.assert_complete();
                }
                let page = fields[1].to_owned();
                let catalog_ordinal = parse_i32(fields[3], line_index);
                let factory_ordinal = parse_usize(fields[5], line_index);
                let active_ordinal = parse_usize(fields[7], line_index);
                let shape = parse_shape(fields[9]);
                assert_eq!(factory_ordinal, shape.factory_ordinal());
                assert_eq!(active_ordinal, shape.factory_ordinal());
                assert_eq!(fields[11], "Bravura");
                let point_size = parse_i32(fields[13], line_index);
                let native_catalog = catalog(&catalogs, point_size);
                let template = native_catalog.template(shape);
                assert_eq!(template.family(), HeadTemplateFamily::Bravura);
                assert_eq!(template.point_size(), point_size);
                assert_eq!(template.width(), parse_i32(fields[15], line_index));
                assert_eq!(template.height(), parse_i32(fields[17], line_index));
                let slim = template.slim_bounds();
                assert_eq!(slim.x, parse_i32(fields[19], line_index));
                assert_eq!(slim.y, parse_i32(fields[20], line_index));
                assert_eq!(slim.width, parse_i32(fields[21], line_index));
                assert_eq!(slim.height, parse_i32(fields[22], line_index));

                let expected_anchors = parse_usize(fields[24], line_index);
                let expected_pixels = parse_usize(fields[26], line_index);
                assert_eq!(template.anchors().len(), expected_anchors);
                assert_eq!(template.key_points().len(), expected_pixels);
                current = Some(CurrentTemplate {
                    page,
                    catalog: catalog_ordinal,
                    shape,
                    template,
                    expected_anchors,
                    expected_pixels,
                    seen_anchors: 0,
                    seen_pixels: 0,
                });
                template_rows += 1;
            }
            "headtemplateanchor" => {
                let state = current.as_mut().expect("anchor must follow a template");
                assert_eq!(fields[1], state.page);
                assert_eq!(parse_i32(fields[3], line_index), state.catalog);
                assert_eq!(parse_shape(fields[5]), state.shape);
                let ordinal = parse_usize(fields[7], line_index);
                assert_eq!(ordinal, state.seen_anchors);
                let anchor = parse_anchor(fields[9]);
                let actual = state.template.anchors()[ordinal];
                assert_eq!(actual.anchor, anchor);
                assert_eq!(actual.dx.to_bits(), parse_bits(fields[15], line_index));
                assert_eq!(actual.dy.to_bits(), parse_bits(fields[17], line_index));
                assert_eq!(
                    state.template.rounded_anchor_offset(anchor).unwrap(),
                    (
                        parse_i32(fields[19], line_index),
                        parse_i32(fields[20], line_index)
                    )
                );
                state.seen_anchors += 1;
                anchor_rows += 1;
            }
            "headtemplatepixel" => {
                let state = current.as_mut().expect("pixel must follow a template");
                assert_eq!(fields[1], state.page);
                assert_eq!(parse_i32(fields[3], line_index), state.catalog);
                assert_eq!(parse_shape(fields[5]), state.shape);
                let ordinal = parse_usize(fields[7], line_index);
                assert_eq!(ordinal, state.seen_pixels);
                let actual = state.template.key_points()[ordinal];
                assert_eq!(actual.x, parse_i32(fields[9], line_index));
                assert_eq!(actual.y, parse_i32(fields[11], line_index));
                assert_eq!(
                    actual.distance,
                    f64::from(parse_i32(fields[13], line_index))
                );
                assert_eq!(
                    actual.distance.to_bits(),
                    parse_bits(fields[17], line_index)
                );
                state.seen_pixels += 1;
                pixel_rows += 1;
            }
            _ => {}
        }
    }
    current
        .expect("oracle must contain templates")
        .assert_complete();

    assert_eq!(catalog_rows, 8);
    assert_eq!(pages.len(), 8);
    assert_eq!(template_rows, expected_templates);
    assert_eq!(anchor_rows, expected_anchors);
    assert_eq!(pixel_rows, expected_pixels);
    assert_eq!(
        point_sizes.into_iter().collect::<Vec<_>>(),
        BRAVURA_HEAD_TEMPLATE_POINT_SIZES
    );
}

fn catalog(catalogs: &[HeadTemplateCatalog], point_size: i32) -> &HeadTemplateCatalog {
    catalogs
        .iter()
        .find(|catalog| catalog.point_size() == point_size)
        .unwrap_or_else(|| panic!("missing native catalog at point size {point_size}"))
}

fn parse_shape(value: &str) -> HeadTemplateShape {
    match value {
        "NOTEHEAD_BLACK" => HeadTemplateShape::NoteheadBlack,
        "NOTEHEAD_VOID" => HeadTemplateShape::NoteheadVoid,
        "WHOLE_NOTE" => HeadTemplateShape::WholeNote,
        "BREVE" => HeadTemplateShape::Breve,
        "NOTEHEAD_BLACK_SMALL" => HeadTemplateShape::NoteheadBlackSmall,
        "NOTEHEAD_VOID_SMALL" => HeadTemplateShape::NoteheadVoidSmall,
        "WHOLE_NOTE_SMALL" => HeadTemplateShape::WholeNoteSmall,
        "BREVE_SMALL" => HeadTemplateShape::BreveSmall,
        _ => panic!("unexpected template shape {value}"),
    }
}

fn parse_anchor(value: &str) -> HeadTemplateAnchor {
    match value {
        "CENTER" => HeadTemplateAnchor::Center,
        "MIDDLE_LEFT" => HeadTemplateAnchor::MiddleLeft,
        "MIDDLE_RIGHT" => HeadTemplateAnchor::MiddleRight,
        "LEFT_STEM" => HeadTemplateAnchor::LeftStem,
        "TOP_LEFT_STEM" => HeadTemplateAnchor::TopLeftStem,
        "BOTTOM_LEFT_STEM" => HeadTemplateAnchor::BottomLeftStem,
        "RIGHT_STEM" => HeadTemplateAnchor::RightStem,
        "TOP_RIGHT_STEM" => HeadTemplateAnchor::TopRightStem,
        "BOTTOM_RIGHT_STEM" => HeadTemplateAnchor::BottomRightStem,
        _ => panic!("unexpected template anchor {value}"),
    }
}

fn parse_i32(value: &str, line_index: usize) -> i32 {
    value
        .parse()
        .unwrap_or_else(|_| panic!("invalid integer on oracle line {}", line_index + 1))
}

fn parse_usize(value: &str, line_index: usize) -> usize {
    value
        .parse()
        .unwrap_or_else(|_| panic!("invalid integer on oracle line {}", line_index + 1))
}

fn parse_bits(value: &str, line_index: usize) -> u64 {
    u64::from_str_radix(value, 16)
        .unwrap_or_else(|_| panic!("invalid f64 bits on oracle line {}", line_index + 1))
}
