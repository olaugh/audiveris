// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_image::beam_structure::Segment;
use audiveris_omr::head_scanner_slices::JavaRectangle;
use audiveris_omr::head_small_beam_purge::SmallBeamPurgeTarget;
use audiveris_omr::native_heads_competitors::{
    NativeHeadsCompetitor, NativeHeadsCompetitorClass, NativeHeadsCompetitorDecision,
    NativeHeadsCompetitorEvidence, NativeHeadsCompetitorGeometry, NativeHeadsCompetitorPool,
    NativeHeadsCompetitorShape, NativeHeadsCompetitorSource, NativeHeadsCompetitorSystem,
};
use audiveris_omr::native_heads_small_beams::{
    NativeHeadsSmallBeamError, NativeHeadsSmallBeamHead, NativeHeadsSmallBeamHeadSystem,
    purge_native_heads_small_beams, purge_native_heads_small_beams_system,
    purge_native_heads_small_beams_system_with_contextual_grades,
};

use audiveris_image::beam_structure::{BeamImpacts, BeamItem, BeamRasterEvidence};
use audiveris_image::run_table::{Orientation, RunTable};
use audiveris_omr::{
    beam_inters::{BeamKind, RawBeam},
    black_head_sizer::BlackHeadSizingResult,
    recognize::{NativeBeamGroupMembership, NativeBeamRecognition},
};

fn rect(x: i32, y: i32, width: i32, height: i32) -> JavaRectangle {
    JavaRectangle::new(x, y, width, height)
}

fn segment(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
    Segment { x1, y1, x2, y2 }
}

fn candidate(
    source_ordinal: usize,
    source: NativeHeadsCompetitorSource,
    shape: NativeHeadsCompetitorShape,
    bounds: JavaRectangle,
    geometry: NativeHeadsCompetitorGeometry,
    grade: f64,
    best_grade: f64,
) -> NativeHeadsCompetitor {
    let class = match shape {
        NativeHeadsCompetitorShape::Beam => NativeHeadsCompetitorClass::BeamInter,
        NativeHeadsCompetitorShape::BeamHook => NativeHeadsCompetitorClass::BeamHookInter,
        NativeHeadsCompetitorShape::BeamSmall => NativeHeadsCompetitorClass::SmallBeamInter,
        _ => NativeHeadsCompetitorClass::MultipleRestInter,
    };
    NativeHeadsCompetitor {
        source,
        source_ordinal,
        accepted_ordinal: None,
        class,
        shape,
        staff_id: None,
        bounds,
        grade,
        best_grade,
        good: true,
        frozen: false,
        geometry,
        glyph: None,
        evidence: NativeHeadsCompetitorEvidence::None,
        decision: NativeHeadsCompetitorDecision::Accept,
    }
}

fn system(system_id: usize, min_beam_width: i32) -> NativeHeadsCompetitorSystem {
    NativeHeadsCompetitorSystem {
        system_id,
        max_stem: 4,
        min_beam_width,
        candidates_in_sig_order: vec![],
        accepted_by_ordinate: vec![],
    }
}

fn explicit_context(
    system: &NativeHeadsCompetitorSystem,
) -> Vec<(NativeHeadsCompetitorSource, f64)> {
    system
        .candidates_in_sig_order
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.shape,
                NativeHeadsCompetitorShape::Beam
                    | NativeHeadsCompetitorShape::BeamHook
                    | NativeHeadsCompetitorShape::BeamSmall
            )
        })
        .map(|candidate| (candidate.source, candidate.best_grade))
        .collect()
}

fn recognition_with_empty_groups(system_ids: &[usize]) -> NativeBeamRecognition {
    NativeBeamRecognition {
        beam_rejections: Vec::new(),
        head_spot_runs: RunTable::new(Orientation::Vertical, 1, 1).unwrap(),
        black_head_sizing: BlackHeadSizingResult {
            traces: vec![],
            singles: vec![],
            stacks: vec![],
            core_single_source_indices: vec![],
            black_head_scale: None,
        },
        music_font_scale: None,
        staff_head_point_sizes: vec![],
        spot_count: 0,
        raw_beams: vec![],
        raw_beam_glyphs: vec![],
        beams_after_multiple_rests: vec![],
        multiple_rests: vec![],
        hooks: vec![],
        hook_glyphs: vec![],
        group_memberships: system_ids
            .iter()
            .map(|system_id| NativeBeamGroupMembership {
                system_id: *system_id,
                groups: vec![],
            })
            .collect(),
        group_counts: system_ids.iter().map(|system_id| (*system_id, 0)).collect(),
        group_count: 0,
    }
}

fn raw_beam(kind: BeamKind, x: f64, grade: f64) -> RawBeam {
    RawBeam {
        kind,
        item: BeamItem {
            median: segment(x, 1.0, x + 4.0, 1.0),
            height: 2.0,
        },
        impacts: BeamImpacts {
            width: 1.0,
            min_height: 1.0,
            max_height: 1.0,
            core: 1.0,
            belt: 1.0,
            distance: 1.0,
            raster: BeamRasterEvidence {
                core_foreground: 0,
                core_count: 0,
                belt_foreground: 0,
                belt_count: 0,
                core_ratio: 0.0,
                belt_ratio: 0.0,
                rounded_width: 0,
            },
        },
        grade,
    }
}

#[test]
fn adapts_all_beam_shapes_in_sig_order_without_acceptance_filtering() {
    let horizontal = |x: f64| NativeHeadsCompetitorGeometry::Horizontal {
        median: segment(x, 1.0, x + 4.0, 1.0),
        height: 2.0,
    };
    let mut system = system(7, 10);
    system.candidates_in_sig_order = vec![
        candidate(
            0,
            NativeHeadsCompetitorSource::GridInter(3),
            NativeHeadsCompetitorShape::ThinBarline,
            rect(30, 0, 2, 4),
            NativeHeadsCompetitorGeometry::Vertical {
                median: segment(31.0, 0.0, 31.0, 4.0),
                width: 2.0,
            },
            0.8,
            0.8,
        ),
        candidate(
            1,
            NativeHeadsCompetitorSource::RawBeam(11),
            NativeHeadsCompetitorShape::BeamHook,
            rect(0, 0, 4, 2),
            horizontal(0.0),
            0.1,
            0.6,
        ),
        candidate(
            2,
            NativeHeadsCompetitorSource::RawBeam(12),
            NativeHeadsCompetitorShape::Beam,
            rect(20, 0, 12, 2),
            horizontal(20.0),
            0.8,
            0.8,
        ),
        candidate(
            3,
            NativeHeadsCompetitorSource::Hook(4),
            NativeHeadsCompetitorShape::BeamSmall,
            rect(40, 0, 5, 2),
            horizontal(40.0),
            0.2,
            0.2,
        ),
    ];
    // Java still queries a live beam rejected by getSystemCompetitors.
    system.candidates_in_sig_order[1].decision = NativeHeadsCompetitorDecision::NotGood;

    let result = purge_native_heads_small_beams_system_with_contextual_grades::<u8>(
        &system,
        &[],
        &explicit_context(&system),
        None,
    )
    .unwrap();

    assert_eq!(
        result
            .beam_provenance
            .iter()
            .map(|beam| (beam.source, beam.source_ordinal))
            .collect::<Vec<_>>(),
        [
            (NativeHeadsCompetitorSource::RawBeam(11), 1),
            (NativeHeadsCompetitorSource::RawBeam(12), 2),
            (NativeHeadsCompetitorSource::Hook(4), 3),
        ],
    );
    assert_eq!(result.beam_inputs.len(), 3);
    assert_eq!(result.arbitration.candidate_beam_indices, [0, 2]);
    assert_eq!(result.min_beam_width, 10);
}

#[test]
fn best_grade_is_the_contextual_grade_used_for_arbitration() {
    let mut system = system(1, 10);
    system.candidates_in_sig_order.push(candidate(
        0,
        NativeHeadsCompetitorSource::RawBeam(0),
        NativeHeadsCompetitorShape::Beam,
        rect(0, 0, 4, 2),
        NativeHeadsCompetitorGeometry::Horizontal {
            median: segment(0.0, 1.0, 4.0, 1.0),
            height: 2.0,
        },
        0.2,
        0.8,
    ));
    let heads = [NativeHeadsSmallBeamHead {
        provenance: "head-A",
        bounds: rect(0, 0, 2, 2),
        grade: 0.5,
        removed: false,
    }];

    let result = purge_native_heads_small_beams_system_with_contextual_grades(
        &system,
        &heads,
        &[(NativeHeadsCompetitorSource::RawBeam(0), 0.8)],
        None,
    )
    .unwrap();

    assert_eq!(result.head_provenance, ["head-A"]);
    assert_eq!(result.head_inputs.len(), 1);
    assert_eq!(result.arbitration.decisions.len(), 1);
    assert_eq!(
        result.arbitration.decisions[0].target,
        SmallBeamPurgeTarget::Head
    );
    assert_eq!(
        result.arbitration.decisions[0]
            .beam_contextual_grade
            .to_bits(),
        0.8_f64.to_bits(),
    );
}

#[test]
fn malformed_beam_geometry_is_rejected_instead_of_silently_skipped() {
    let mut system = system(2, 10);
    system.candidates_in_sig_order.push(candidate(
        9,
        NativeHeadsCompetitorSource::RawBeam(3),
        NativeHeadsCompetitorShape::BeamHook,
        rect(0, 0, 4, 2),
        NativeHeadsCompetitorGeometry::Vertical {
            median: segment(1.0, 0.0, 1.0, 2.0),
            width: 1.0,
        },
        0.4,
        0.4,
    ));

    assert_eq!(
        purge_native_heads_small_beams_system_with_contextual_grades::<()>(
            &system,
            &[],
            &explicit_context(&system),
            None,
        ),
        Err(NativeHeadsSmallBeamError::NonHorizontalBeam {
            system_id: 2,
            source_ordinal: 9,
            shape: NativeHeadsCompetitorShape::BeamHook,
        }),
    );
}

#[test]
fn pool_adapter_preserves_pool_order_and_head_provenance() {
    let pool = NativeHeadsCompetitorPool {
        systems: vec![system(4, 12), system(2, 8)],
    };
    let heads = [
        NativeHeadsSmallBeamHeadSystem {
            system_id: 2,
            heads_in_sig_order: vec![NativeHeadsSmallBeamHead {
                provenance: 20,
                bounds: rect(0, 0, 2, 2),
                grade: 0.5,
                removed: false,
            }],
        },
        NativeHeadsSmallBeamHeadSystem {
            system_id: 4,
            heads_in_sig_order: vec![NativeHeadsSmallBeamHead {
                provenance: 40,
                bounds: rect(0, 0, 2, 2),
                grade: 0.5,
                removed: false,
            }],
        },
    ];

    let recognition = recognition_with_empty_groups(&[4, 2]);
    let result = purge_native_heads_small_beams(&pool, &recognition, &heads, None).unwrap();

    assert_eq!(
        result
            .iter()
            .map(|system| system.system_id)
            .collect::<Vec<_>>(),
        [4, 2],
    );
    assert_eq!(result[0].head_provenance, [40]);
    assert_eq!(result[1].head_provenance, [20]);
}

#[test]
fn pool_adapter_rejects_missing_duplicate_and_unknown_head_systems() {
    let pool = NativeHeadsCompetitorPool {
        systems: vec![system(1, 10)],
    };
    let recognition = recognition_with_empty_groups(&[1]);
    let empty = NativeHeadsSmallBeamHeadSystem::<()> {
        system_id: 1,
        heads_in_sig_order: vec![],
    };

    assert_eq!(
        purge_native_heads_small_beams::<()>(&pool, &recognition, &[], None),
        Err(NativeHeadsSmallBeamError::MissingHeadSystem(1)),
    );
    assert_eq!(
        purge_native_heads_small_beams(&pool, &recognition, &[empty.clone(), empty], None),
        Err(NativeHeadsSmallBeamError::DuplicateHeadSystem(1)),
    );
    assert_eq!(
        purge_native_heads_small_beams(
            &pool,
            &recognition,
            &[NativeHeadsSmallBeamHeadSystem::<()> {
                system_id: 99,
                heads_in_sig_order: vec![],
            }],
            None,
        ),
        Err(NativeHeadsSmallBeamError::UnexpectedHeadSystem(99)),
    );
}

#[test]
fn production_context_drops_support_from_a_beam_removed_earlier() {
    let mut system = system(1, 10);
    let raw = [
        raw_beam(BeamKind::Beam, 0.0, 0.4),
        raw_beam(BeamKind::Beam, 10.0, 0.4),
    ];
    system.candidates_in_sig_order = raw
        .iter()
        .enumerate()
        .map(|(ordinal, beam)| {
            candidate(
                ordinal,
                NativeHeadsCompetitorSource::RawBeam(ordinal),
                NativeHeadsCompetitorShape::Beam,
                rect((ordinal * 10) as i32, 0, 4, 2),
                NativeHeadsCompetitorGeometry::Horizontal {
                    median: beam.item.median,
                    height: beam.item.height,
                },
                beam.grade,
                beam.grade,
            )
        })
        .collect();
    let mut recognition = recognition_with_empty_groups(&[1]);
    recognition.raw_beams = raw.into_iter().map(|beam| (1, beam)).collect();
    recognition.beams_after_multiple_rests = recognition.raw_beams.clone();
    recognition.group_memberships[0].groups = vec![vec![0, 1]];
    recognition.group_counts[0] = (1, 1);
    recognition.group_count = 1;
    let heads = [
        NativeHeadsSmallBeamHead {
            provenance: 0,
            bounds: rect(0, 0, 2, 2),
            grade: 0.7,
            removed: false,
        },
        NativeHeadsSmallBeamHead {
            provenance: 1,
            bounds: rect(10, 0, 2, 2),
            grade: 0.5,
            removed: false,
        },
    ];

    let result = purge_native_heads_small_beams_system(&system, &recognition, &heads, None).unwrap();

    let supported = audiveris_core::grade::contextual(0.4, 0.4 * 3.0);
    assert!(supported > 0.5);
    assert_eq!(result.arbitration.beam_removed, [true, true]);
    assert_eq!(result.arbitration.head_removed, [false, false]);
    assert_eq!(
        result.arbitration.computed_contextual_grades,
        [Some(supported), Some(0.4)]
    );
}
