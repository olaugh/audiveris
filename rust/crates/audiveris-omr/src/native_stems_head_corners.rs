// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identity-free production boundary for Java STEMS head-corner geometry.
//!
//! `StemsRetriever.inspectStems` recreates live heads in SIG insertion order,
//! stably sorts them by abscissa, and constructs one `HeadLinker` per head.
//! Each linker immediately computes four corner reference points plus the
//! outside/inside lookup limits. This module stops at that read-only boundary,
//! before Java retrieves a stump, registers a glyph, or mutates the SIG.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::run_table::RunTable;

use crate::{
    head_scanner_slices::JavaRectangle,
    head_seed_tally_analysis::HeadSeedTallySide,
    head_template::{
        HeadTemplate, HeadTemplateAnchor, HeadTemplateEvaluationError, HeadTemplateShape,
    },
    native_heads::NativeHeadsRecognition,
    native_heads_epilog::{JavaHeadSeedShape, java_shape},
    native_heads_staff_epilog::{
        NativeHeadStaffEpilogOrigin, NativeHeadStaffEpilogProvenance, NativeHeadStaffEpilogRef,
        NativeHeadsStaffEpilogSystem,
    },
    native_stem_seeds::NativeStemSeedRecognition,
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemVerticalSide},
};

/// Complete sheet boundary immediately before Java head-corner stump lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCornerRecognition {
    pub systems: Vec<NativeStemsHeadCornerSystem>,
    pub head_count: usize,
    pub corner_count: usize,
}

/// One system's live, stem-capable HEADS and Java order permutations.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCornerSystem {
    pub system_id: usize,
    pub profile: i32,
    pub interline: i32,
    pub max_head_in_dx: i32,
    pub max_head_out_dx: i32,
    /// Stem-capable heads in the SIG insertion order retained by HEADS.
    pub heads_in_sig_order: Vec<NativeStemsHeadCornerHead>,
    /// Indices into `heads_in_sig_order`, stably sorted by `bounds.x`.
    pub heads_by_abscissa: Vec<usize>,
    /// Indices into `heads_in_sig_order`, stably sorted by decreasing grade.
    pub heads_by_reverse_grade: Vec<usize>,
}

/// One live head with no fabricated Java inter or glyph identity.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCornerHead {
    pub reference: NativeHeadStaffEpilogRef,
    pub staff_id: usize,
    pub origin: NativeHeadStaffEpilogOrigin,
    pub provenance: NativeHeadStaffEpilogProvenance,
    pub system_creation_ordinal: usize,
    pub shape: HeadTemplateShape,
    pub pitch_bits: u64,
    pub bounds: JavaRectangle,
    pub grade_bits: u64,
    pub glyph_bounds: JavaRectangle,
    pub glyph_weight: usize,
    pub glyph_run_digest: u64,
    /// Exact registered foreground retained for REDUCTION overlap parity.
    pub glyph_run_table: RunTable,
    pub point_size: i32,
    pub catalog_ordinal: usize,
    /// `LEFT/TOP`, `LEFT/BOTTOM`, `RIGHT/TOP`, `RIGHT/BOTTOM`.
    pub corners_in_constructor_order: Vec<NativeStemsHeadCorner>,
}

/// Exact geometry Java computes at the start of one `HeadLinker.CLinker`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadCorner {
    pub constructor_ordinal: usize,
    /// Ordinal in later `HeadCorner.values()` inspection order.
    pub inspection_ordinal: usize,
    pub horizontal: NativeStemHeadSide,
    pub vertical: NativeStemVerticalSide,
    pub anchor: HeadTemplateAnchor,
    pub template_bounds: JavaRectangle,
    pub anchor_offset: (i32, i32),
    pub seed_dx_left: Option<f64>,
    pub seed_dx_right: Option<f64>,
    pub applied_seed_dx: Option<f64>,
    pub reference: NativeStemPoint,
    pub out_point: NativeStemPoint,
    pub in_point: NativeStemPoint,
}

#[derive(Debug)]
pub enum NativeStemsHeadCornerError {
    SystemOrder {
        epilog: Vec<usize>,
        staff_epilog: Vec<usize>,
        stem_seeds: Vec<usize>,
    },
    InvalidHeadReference {
        system_id: usize,
        reference: NativeHeadStaffEpilogRef,
    },
    MissingHeadGlyph {
        system_id: usize,
        staff_id: usize,
        origin: NativeHeadStaffEpilogOrigin,
    },
    FinalHeadSetMismatch {
        system_id: usize,
    },
    MissingCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    DuplicateCatalogSelection {
        system_id: usize,
        staff_id: usize,
    },
    InvalidCatalogOrdinal {
        system_id: usize,
        staff_id: usize,
        catalog_ordinal: usize,
    },
    DuplicateScaleEntry {
        shape: JavaHeadSeedShape,
        side: HeadSeedTallySide,
    },
    InvalidParameters {
        system_id: usize,
        profile: i32,
        interline: i32,
    },
    Template {
        system_id: usize,
        staff_id: usize,
        shape: HeadTemplateShape,
        anchor: HeadTemplateAnchor,
        source: HeadTemplateEvaluationError,
    },
}

impl fmt::Display for NativeStemsHeadCornerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                epilog,
                staff_epilog,
                stem_seeds,
            } => write!(
                formatter,
                "STEMS head-corner system order differs: epilog {epilog:?}, staff epilog \
                 {staff_epilog:?}, stem seeds {stem_seeds:?}"
            ),
            Self::InvalidHeadReference {
                system_id,
                reference,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} has invalid head reference {reference:?}"
            ),
            Self::MissingHeadGlyph {
                system_id,
                staff_id,
                origin,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} staff {staff_id} has no {origin:?} glyph"
            ),
            Self::FinalHeadSetMismatch { system_id } => write!(
                formatter,
                "STEMS head-corner system {system_id} live SIG heads differ from final HEADS"
            ),
            Self::MissingCatalogSelection {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} staff {staff_id} has no template catalog"
            ),
            Self::DuplicateCatalogSelection {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} staff {staff_id} has duplicate catalogs"
            ),
            Self::InvalidCatalogOrdinal {
                system_id,
                staff_id,
                catalog_ordinal,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} staff {staff_id} has invalid catalog \
                 ordinal {catalog_ordinal}"
            ),
            Self::DuplicateScaleEntry { shape, side } => write!(
                formatter,
                "STEMS head-corner scale has duplicate {shape:?}/{side:?} entry"
            ),
            Self::InvalidParameters {
                system_id,
                profile,
                interline,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} has profile {profile} and interline \
                 {interline}"
            ),
            Self::Template {
                system_id,
                staff_id,
                shape,
                anchor,
                source,
            } => write!(
                formatter,
                "STEMS head-corner system {system_id} staff {staff_id} {shape:?}/{anchor:?} \
                 failed: {source}"
            ),
        }
    }
}

impl Error for NativeStemsHeadCornerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Template { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Materialize Java's read-only `HeadLinker.CLinker` corner prolog.
///
/// The function validates all relevant ordering domains and returns only
/// stage-local references into the owned HEADS result. Whole and breve heads
/// are omitted exactly as `ShapeSet.getTemplateNotesStem` omits stemless heads.
pub fn materialize_native_stems_head_corners(
    heads: &NativeHeadsRecognition,
    stem_seeds: &NativeStemSeedRecognition,
) -> Result<NativeStemsHeadCornerRecognition, NativeStemsHeadCornerError> {
    let epilog_ids = heads
        .epilog
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let staff_epilog_ids = heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let stem_seed_ids = stem_seeds
        .systems
        .iter()
        .map(|system| system.raw.system_id)
        .collect::<Vec<_>>();
    if epilog_ids != staff_epilog_ids || epilog_ids != stem_seed_ids {
        return Err(NativeStemsHeadCornerError::SystemOrder {
            epilog: epilog_ids,
            staff_epilog: staff_epilog_ids,
            stem_seeds: stem_seed_ids,
        });
    }

    let mut selections = BTreeMap::new();
    for &selection in &heads.prolog.staff_template_catalogs {
        let owner = (selection.system_id, selection.staff_id);
        if selections.insert(owner, selection).is_some() {
            return Err(NativeStemsHeadCornerError::DuplicateCatalogSelection {
                system_id: selection.system_id,
                staff_id: selection.staff_id,
            });
        }
    }

    let mut scale_dx = BTreeMap::new();
    for row in &heads.epilog.scale_analysis {
        if scale_dx.insert((row.shape, row.side), row.dx).is_some() {
            return Err(NativeStemsHeadCornerError::DuplicateScaleEntry {
                shape: row.shape,
                side: row.side,
            });
        }
    }

    let mut systems = Vec::with_capacity(heads.epilog.systems.len());
    let mut head_count = 0_usize;
    for ((epilog, staff_epilog), seed_system) in heads
        .epilog
        .systems
        .iter()
        .zip(&heads.epilog.staff_epilog.systems)
        .zip(&stem_seeds.systems)
    {
        let system_id = epilog.system_id;
        let profile = seed_system.raw.profile;
        let interline = seed_system.raw.interline;
        let (max_head_in_dx, max_head_out_dx) = stem_head_limits(profile, interline).ok_or(
            NativeStemsHeadCornerError::InvalidParameters {
                system_id,
                profile,
                interline,
            },
        )?;

        let removed = epilog
            .beam_removed_heads
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let live_references = epilog
            .heads_in_sig_order
            .iter()
            .copied()
            .filter(|reference| !removed.contains(reference))
            .collect::<Vec<_>>();
        let live_set = live_references.iter().copied().collect::<BTreeSet<_>>();
        let final_set = epilog.final_heads.iter().copied().collect::<BTreeSet<_>>();
        if live_set.len() != live_references.len()
            || final_set.len() != epilog.final_heads.len()
            || live_set != final_set
        {
            return Err(NativeStemsHeadCornerError::FinalHeadSetMismatch { system_id });
        }

        let mut system_heads = Vec::new();
        for reference in live_references {
            let (staff_id, head) = resolve_head(staff_epilog, reference).ok_or(
                NativeStemsHeadCornerError::InvalidHeadReference {
                    system_id,
                    reference,
                },
            )?;
            if !matches!(
                head.shape,
                HeadTemplateShape::NoteheadBlack | HeadTemplateShape::NoteheadVoid
            ) {
                continue;
            }
            let selection = selections.get(&(system_id, staff_id)).copied().ok_or(
                NativeStemsHeadCornerError::MissingCatalogSelection {
                    system_id,
                    staff_id,
                },
            )?;
            let catalog = heads
                .prolog
                .template_catalogs
                .get(selection.catalog_ordinal)
                .ok_or(NativeStemsHeadCornerError::InvalidCatalogOrdinal {
                    system_id,
                    staff_id,
                    catalog_ordinal: selection.catalog_ordinal,
                })?;
            let template = catalog.template(head.shape);
            let glyph_run_table = source_head_glyph(heads, system_id, staff_id, head.origin)
                .ok_or(NativeStemsHeadCornerError::MissingHeadGlyph {
                    system_id,
                    staff_id,
                    origin: head.origin,
                })?
                .run_table
                .clone();
            let shape = java_shape(head.shape);
            let seed_dx_left = scale_dx.get(&(shape, HeadSeedTallySide::Left)).copied();
            let seed_dx_right = scale_dx.get(&(shape, HeadSeedTallySide::Right)).copied();
            let applied_seed_dx = combined_seed_dx(seed_dx_left, seed_dx_right);
            let corners_in_constructor_order = constructor_corners()
                .into_iter()
                .enumerate()
                .map(|(constructor_ordinal, (horizontal, vertical, anchor))| {
                    build_corner(
                        system_id,
                        staff_id,
                        head.shape,
                        head.bounds,
                        template,
                        constructor_ordinal,
                        horizontal,
                        vertical,
                        anchor,
                        max_head_in_dx,
                        max_head_out_dx,
                        seed_dx_left,
                        seed_dx_right,
                        applied_seed_dx,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            system_heads.push(NativeStemsHeadCornerHead {
                reference,
                staff_id,
                origin: head.origin,
                provenance: head.provenance,
                system_creation_ordinal: head.system_creation_ordinal,
                shape: head.shape,
                pitch_bits: head.pitch_bits,
                bounds: head.bounds,
                grade_bits: head.grade_bits,
                glyph_bounds: head.glyph_bounds,
                glyph_weight: head.glyph_weight,
                glyph_run_digest: head.glyph_run_digest,
                glyph_run_table,
                point_size: selection.point_size,
                catalog_ordinal: selection.catalog_ordinal,
                corners_in_constructor_order,
            });
        }

        let mut heads_by_abscissa = (0..system_heads.len()).collect::<Vec<_>>();
        heads_by_abscissa.sort_by_key(|&index| system_heads[index].bounds.x);
        let mut heads_by_reverse_grade = (0..system_heads.len()).collect::<Vec<_>>();
        heads_by_reverse_grade.sort_by(|&left, &right| {
            f64::from_bits(system_heads[right].grade_bits)
                .total_cmp(&f64::from_bits(system_heads[left].grade_bits))
        });

        head_count += system_heads.len();
        systems.push(NativeStemsHeadCornerSystem {
            system_id,
            profile,
            interline,
            max_head_in_dx,
            max_head_out_dx,
            heads_in_sig_order: system_heads,
            heads_by_abscissa,
            heads_by_reverse_grade,
        });
    }

    Ok(NativeStemsHeadCornerRecognition {
        systems,
        head_count,
        corner_count: head_count * 4,
    })
}

fn source_head_glyph(
    heads: &NativeHeadsRecognition,
    system_id: usize,
    staff_id: usize,
    origin: NativeHeadStaffEpilogOrigin,
) -> Option<&crate::head_glyph_retrieval::RetrievedHeadGlyph> {
    match origin {
        NativeHeadStaffEpilogOrigin::Seed(ordinal) => heads
            .seed_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
        NativeHeadStaffEpilogOrigin::Range(ordinal) => heads
            .range_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
    }
}

fn resolve_head(
    system: &NativeHeadsStaffEpilogSystem,
    reference: NativeHeadStaffEpilogRef,
) -> Option<(
    usize,
    &crate::native_heads_staff_epilog::NativeHeadStaffEpilogHead,
)> {
    let staff = system.staffs.get(reference.staff_index)?;
    Some((staff.staff_id, staff.heads.get(reference.head_index)?))
}

fn stem_head_limits(profile: i32, interline: i32) -> Option<(i32, i32)> {
    if profile < 0 || interline <= 0 {
        return None;
    }
    let in_fraction = if profile >= 1 { 0.4 } else { 0.2 };
    let out_fraction = if profile >= 2 {
        0.35
    } else if profile >= 1 {
        0.25
    } else {
        0.15
    };
    let in_pixels = (f64::from(interline) * in_fraction).round_ties_even();
    let out_pixels = (f64::from(interline) * out_fraction).round_ties_even();
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&in_pixels)
        || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&out_pixels)
    {
        return None;
    }
    Some((in_pixels as i32, out_pixels as i32))
}

fn constructor_corners() -> [(
    NativeStemHeadSide,
    NativeStemVerticalSide,
    HeadTemplateAnchor,
); 4] {
    [
        (
            NativeStemHeadSide::Left,
            NativeStemVerticalSide::Top,
            HeadTemplateAnchor::TopLeftStem,
        ),
        (
            NativeStemHeadSide::Left,
            NativeStemVerticalSide::Bottom,
            HeadTemplateAnchor::BottomLeftStem,
        ),
        (
            NativeStemHeadSide::Right,
            NativeStemVerticalSide::Top,
            HeadTemplateAnchor::TopRightStem,
        ),
        (
            NativeStemHeadSide::Right,
            NativeStemVerticalSide::Bottom,
            HeadTemplateAnchor::BottomRightStem,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_corner(
    system_id: usize,
    staff_id: usize,
    shape: HeadTemplateShape,
    head_bounds: JavaRectangle,
    template: &HeadTemplate,
    constructor_ordinal: usize,
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
    anchor: HeadTemplateAnchor,
    max_head_in_dx: i32,
    max_head_out_dx: i32,
    seed_dx_left: Option<f64>,
    seed_dx_right: Option<f64>,
    applied_seed_dx: Option<f64>,
) -> Result<NativeStemsHeadCorner, NativeStemsHeadCornerError> {
    let slim = template.slim_bounds();
    let template_bounds = JavaRectangle::new(
        head_bounds.x.wrapping_sub(slim.x),
        head_bounds.y.wrapping_sub(slim.y),
        template.width(),
        template.height(),
    );
    let anchor_offset = template.rounded_anchor_offset(anchor).map_err(|source| {
        NativeStemsHeadCornerError::Template {
            system_id,
            staff_id,
            shape,
            anchor,
            source,
        }
    })?;
    let raw_reference = NativeStemPoint {
        x: f64::from(template_bounds.x) + f64::from(anchor_offset.0),
        y: f64::from(template_bounds.y) + f64::from(anchor_offset.1),
    };
    let reference = if let Some(dx) = applied_seed_dx {
        NativeStemPoint {
            x: match horizontal {
                NativeStemHeadSide::Left => f64::from(head_bounds.x) + 0.5 - dx,
                NativeStemHeadSide::Right => {
                    f64::from(head_bounds.x) + f64::from(head_bounds.width) - 1.0 + dx
                }
            },
            y: raw_reference.y,
        }
    } else {
        raw_reference
    };
    let x_direction = match horizontal {
        NativeStemHeadSide::Left => -1.0,
        NativeStemHeadSide::Right => 1.0,
    };
    Ok(NativeStemsHeadCorner {
        constructor_ordinal,
        inspection_ordinal: inspection_ordinal(horizontal, vertical),
        horizontal,
        vertical,
        anchor,
        template_bounds,
        anchor_offset,
        seed_dx_left,
        seed_dx_right,
        applied_seed_dx,
        reference,
        out_point: NativeStemPoint {
            x: reference.x + (x_direction * f64::from(max_head_out_dx)),
            y: reference.y,
        },
        in_point: NativeStemPoint {
            x: reference.x - (x_direction * f64::from(max_head_in_dx)),
            y: reference.y,
        },
    })
}

fn inspection_ordinal(horizontal: NativeStemHeadSide, vertical: NativeStemVerticalSide) -> usize {
    match (horizontal, vertical) {
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top) => 0,
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom) => 1,
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top) => 2,
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom) => 3,
    }
}

fn combined_seed_dx(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some((left + right) / 2.0),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::head_template_catalog::load_bravura_head_template_catalogs;

    #[test]
    fn constructor_and_inspection_orders_are_distinct_and_exact() {
        let actual = constructor_corners()
            .into_iter()
            .enumerate()
            .map(|(constructor, (horizontal, vertical, _))| {
                (constructor, inspection_ordinal(horizontal, vertical))
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, vec![(0, 2), (1, 1), (2, 0), (3, 3)]);
    }

    #[test]
    fn corner_uses_template_y_seed_scale_x_and_asymmetric_limits() {
        let catalogs = load_bravura_head_template_catalogs().expect("catalog asset");
        let template = catalogs
            .iter()
            .find(|catalog| catalog.point_size() == 84)
            .expect("84-point catalog")
            .template(HeadTemplateShape::NoteheadBlack);
        let corner = build_corner(
            1,
            1,
            HeadTemplateShape::NoteheadBlack,
            JavaRectangle::new(100, 200, 20, 18),
            template,
            0,
            NativeStemHeadSide::Left,
            NativeStemVerticalSide::Top,
            HeadTemplateAnchor::TopLeftStem,
            8,
            5,
            Some(1.0),
            Some(3.0),
            Some(2.0),
        )
        .expect("corner");

        assert_eq!(corner.reference.x, 98.5);
        assert_eq!(corner.out_point.x, 93.5);
        assert_eq!(corner.in_point.x, 106.5);
        assert_eq!(corner.reference.y, corner.out_point.y);
        assert_eq!(corner.reference.y, corner.in_point.y);
    }

    #[test]
    fn profile_limits_follow_head_stem_relation_overrides() {
        assert_eq!(stem_head_limits(0, 20), Some((4, 3)));
        assert_eq!(stem_head_limits(1, 20), Some((8, 5)));
        assert_eq!(stem_head_limits(2, 20), Some((8, 7)));
        assert_eq!(stem_head_limits(3, 20), Some((8, 7)));
        assert_eq!(stem_head_limits(-1, 20), None);
    }
}
