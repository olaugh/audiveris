// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light lifecycle port of Java `CueBeamsStep`.
//!
//! Cue aggregate detection, morphology, beam grading, and linking remain one
//! injected visual/geometric seam. This module owns the two prolog gates, the
//! shared vertical SPOT_LAG and spot list, system traversal, SIG/index/member
//! mutations, checked continuation, fatal prefixes, and Java's empty epilog.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_image::{
    glyph_factory::{GlyphComponent, build_glyph_components},
    morphology::{BEAM_CIRCLE_DIAMETER_RATIO, close_with_disk, digest},
    run_table::{Orientation, RunTable, RunTableError},
    spots::BEAM_BINARIZATION_THRESHOLD,
};

use crate::beam_parameters::SheetParameters;
use crate::head_scanner_slices::JavaRectangle;
use crate::native_heads_range_lookup::java_rectangle_grow;
use crate::native_reduction::NativeReductionRecognition;
use crate::native_sig::{
    NativeSigBounds, NativeSigInterKind, NativeSigRelationKind, NativeSigVertexId,
};
use crate::recognize::GridLinesRecognition;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueSpotOrientation {
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralCueSpotSection {
    pub id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueGlyph {
    pub id: usize,
    pub beam_spot_group: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCueInterKind {
    SmallHead,
    Stem,
    SmallBeam,
    BeamGroup,
    Other(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueInter {
    pub id: usize,
    pub kind: NeutralCueInterKind,
    pub glyph_id: Option<usize>,
    /// Beam-group membership in Java insertion order.
    pub member_ids: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralCueRelationKind {
    BeamStem,
    HeadStem,
    Exclusion,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeutralCueRelation {
    pub id: usize,
    pub source_inter_id: usize,
    pub target_inter_id: usize,
    pub kind: NeutralCueRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueSystem {
    pub id: usize,
    pub inters: Vec<NeutralCueInter>,
    pub relations: Vec<NeutralCueRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralCueBeamsSheet {
    pub id: usize,
    pub small_heads_enabled: bool,
    pub small_beam_scale: Option<i32>,
    pub systems: Vec<NeutralCueSystem>,
    pub registered_glyphs: Vec<NeutralCueGlyph>,
    pub next_glyph_id: usize,
    pub live_inter_ids: Vec<usize>,
    pub retired_inter_ids: Vec<usize>,
    pub next_inter_id: usize,
    pub mutations: Vec<CueBeamsMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsContext {
    pub spot_orientation: CueSpotOrientation,
    /// Java shared `List<Glyph> spots`, in append order.
    pub spot_glyph_ids: Vec<usize>,
    /// Java shared BasicLag entity order.
    pub spot_lag_sections: Vec<NeutralCueSpotSection>,
}

impl Default for CueBeamsContext {
    fn default() -> Self {
        Self {
            spot_orientation: CueSpotOrientation::Vertical,
            spot_glyph_ids: Vec::new(),
            spot_lag_sections: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsMutation {
    GlyphRegistered {
        system_id: usize,
        glyph_id: usize,
    },
    SpotAppended {
        system_id: usize,
        glyph_id: usize,
    },
    SpotLagSectionAdded {
        system_id: usize,
        section_id: usize,
    },
    InterAdded {
        system_id: usize,
        inter_id: usize,
    },
    InterRemoved {
        system_id: usize,
        inter_id: usize,
    },
    RelationAdded {
        system_id: usize,
        relation: NeutralCueRelation,
    },
    RelationRemoved {
        system_id: usize,
        relation: NeutralCueRelation,
    },
    MemberAttached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    MemberDetached {
        system_id: usize,
        owner_id: usize,
        member_id: usize,
    },
    SystemFailed {
        system_id: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueBeamsDelta {
    /// Exact successful Java mutation prefix before failure.
    pub mutations: Vec<CueBeamsDeltaMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsDeltaMutation {
    RegisterGlyph(NeutralCueGlyph),
    AppendSpot(usize),
    AddSpotLagSection(NeutralCueSpotSection),
    AddInter(NeutralCueInter),
    RemoveInter(usize),
    AddRelation(NeutralCueRelation),
    RemoveRelation(usize),
    AttachMember { owner_id: usize, member_id: usize },
    DetachMember { owner_id: usize, member_id: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsFailure<VisualError> {
    Checked(VisualError),
    Fatal(VisualError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsSystemOutcome<VisualError> {
    pub delta: CueBeamsDelta,
    pub failure: Option<CueBeamsFailure<VisualError>>,
}

impl<VisualError> CueBeamsSystemOutcome<VisualError> {
    #[must_use]
    pub fn success(delta: CueBeamsDelta) -> Self {
        Self {
            delta,
            failure: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CueBeamsSystemInput<'a> {
    pub sheet: &'a NeutralCueBeamsSheet,
    pub system_id: usize,
    /// Snapshot of the context accumulated by all prior systems.
    pub context: &'a CueBeamsContext,
    /// Java `cueBeamRatio`, resolved here for the visual collaborator.
    pub cue_beam_ratio: f64,
}

/// First unavailable dependency: `BeamsBuilder.buildCueBeams`.
pub trait VisualCueBeams {
    type Error;

    fn build_system_cue_beams(
        &mut self,
        input: CueBeamsSystemInput<'_>,
    ) -> CueBeamsSystemOutcome<Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueBeamsSkipReason {
    SmallHeadsDisabled,
    SmallBeamScaleAlreadySet,
}

/// Native page state after the exact CUE_BEAMS prolog gate.
///
/// Java defaults `smallHeads` to false, so ordinary image recognition reaches
/// this completed no-op result without constructing a `BeamsBuilder`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueBeamsRecognition {
    pub reduction: NativeReductionRecognition,
    pub skip_reason: CueBeamsSkipReason,
    pub small_heads_enabled: bool,
    pub detected_small_beam_height: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCueBeamsRecognitionError {
    ActiveRecognitionUnavailable,
}

impl fmt::Display for NativeCueBeamsRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActiveRecognitionUnavailable => formatter.write_str(
                "active cue-beam recognition requires the unported BeamsBuilder.buildCueBeams",
            ),
        }
    }
}

impl Error for NativeCueBeamsRecognitionError {}

/// Execute Java's CUE_BEAMS prolog gate over a completed REDUCTION page.
///
/// Gate priority is source-exact: a disabled small-head switch wins even when
/// SCALE already found a small-beam height. Active recognition fails typed at
/// the first unavailable visual dependency before any downstream mutation.
pub fn recognize_native_cue_beams(
    grid: &GridLinesRecognition,
    reduction: NativeReductionRecognition,
    small_heads_enabled: bool,
) -> Result<NativeCueBeamsRecognition, NativeCueBeamsRecognitionError> {
    let detected_small_beam_height = grid.scale.scale.small_beam.map(|scale| scale.main);
    let skip_reason = if !small_heads_enabled {
        CueBeamsSkipReason::SmallHeadsDisabled
    } else if detected_small_beam_height.is_some() {
        CueBeamsSkipReason::SmallBeamScaleAlreadySet
    } else {
        return Err(NativeCueBeamsRecognitionError::ActiveRecognitionUnavailable);
    };
    Ok(NativeCueBeamsRecognition {
        reduction,
        skip_reason,
        small_heads_enabled,
        detected_small_beam_height,
    })
}

/// One contextual-grade-qualified small black head considered by Java's
/// `BeamsBuilder.getCueAggregates()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateHead {
    pub sig_ordinal: NativeSigVertexId,
    pub stem_sig_ordinal: NativeSigVertexId,
    pub bounds: NativeSigBounds,
    pub grade: f64,
    pub contextual_grade: f64,
    /// Final aggregate ordinal, or `None` when Java purges its singleton.
    pub aggregate_ordinal: Option<usize>,
}

/// One retained Java cue aggregate after singleton purge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeCueAggregate {
    pub ordinal: usize,
    pub bounds: NativeSigBounds,
    /// Parallel head/stem identity pairs in stable head-abscissa order.
    pub members: Vec<(NativeSigVertexId, NativeSigVertexId)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateSystem {
    pub system_id: usize,
    pub interline: i32,
    pub cue_x_margin: i32,
    pub cue_y_margin: i32,
    pub small_black_count: usize,
    pub qualified_heads: Vec<NativeCueAggregateHead>,
    pub aggregates: Vec<NativeCueAggregate>,
}

/// Read-only active CUE_BEAMS frontier immediately before
/// `CueAggregate.process()` performs morphology and graph mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateRecognition {
    pub systems: Vec<NativeCueAggregateSystem>,
}

/// Exact mutation-free prefix of Java `CueAggregate.process()`.
///
/// A zero direction is terminal for this aggregate: Java logs the mixed or
/// unknown layout and returns before copying or morphologically closing any
/// raster pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateProcessPlan {
    pub system_id: usize,
    pub aggregate_ordinal: usize,
    pub aggregate_bounds: NativeSigBounds,
    /// Up stems are `-1`, down stems are `1`, mixed or unknown is `0`.
    pub global_direction: i32,
    /// Clipped NO_STAFF crop. Absent exactly when `global_direction == 0`.
    pub cue_box: Option<NativeSigBounds>,
    pub cue_box_dx: i32,
    pub cue_box_dy: i32,
    /// The unrounded height passed to `SpotsBuilder.buildSpots`.
    pub cue_beam_height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateProcessSystem {
    pub system_id: usize,
    pub plans: Vec<NativeCueAggregateProcessPlan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateProcessRecognition {
    pub systems: Vec<NativeCueAggregateProcessSystem>,
}

/// Cue glyphs returned by Java `CueAggregate.getCueGlyphs()` before they are
/// registered as BEAM_SPOT glyphs or graded as beam interpretations.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueAggregateSpots {
    pub system_id: usize,
    pub aggregate_ordinal: usize,
    pub cue_box: NativeSigBounds,
    pub closing_radius: f32,
    pub closed_digest: String,
    pub glyphs: Vec<GlyphComponent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCueSpotRecognition {
    pub aggregates: Vec<NativeCueAggregateSpots>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeCueAggregateError {
    MissingStem {
        system_id: usize,
        head_sig_ordinal: usize,
    },
    InvalidStem {
        system_id: usize,
        head_sig_ordinal: usize,
        stem_sig_ordinal: usize,
    },
    MissingSystem {
        system_id: usize,
    },
    MissingMember {
        system_id: usize,
        aggregate_ordinal: usize,
        sig_ordinal: usize,
    },
}

impl fmt::Display for NativeCueAggregateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native cue aggregates failed: {self:?}")
    }
}

impl Error for NativeCueAggregateError {}

#[derive(Clone, Debug)]
struct WorkingCueAggregate {
    bounds: NativeSigBounds,
    member_indices: Vec<usize>,
}

/// Materialize Java's complete `getCueAggregates()` result from the live,
/// reduced native SIG without performing cue-spot morphology or mutation.
pub fn materialize_native_cue_aggregates(
    reduction: &NativeReductionRecognition,
) -> Result<NativeCueAggregateRecognition, NativeCueAggregateError> {
    const MIN_CONTEXTUAL_GRADE: f64 = 0.5;
    const CUE_X_MARGIN: f64 = 2.0;
    const CUE_Y_MARGIN: f64 = 3.0;

    let interline = reduction.stems.reduction_interline;
    let cue_x_margin = (f64::from(interline) * CUE_X_MARGIN).round_ties_even() as i32;
    let cue_y_margin = (f64::from(interline) * CUE_Y_MARGIN).round_ties_even() as i32;
    let mut systems = Vec::with_capacity(reduction.stems.systems.len());

    for finalized in &reduction.stems.systems {
        let system_id = finalized.system_id;
        let mut sig = finalized.transaction.state_after.beam_state.sig.clone();
        if sig.vertices.iter().any(|vertex| {
            vertex.active
                && vertex.kind == NativeSigInterKind::Head
                && vertex.shape.as_deref() == Some("NOTEHEAD_BLACK_SMALL")
                && vertex.contextual_grade.is_none()
        }) {
            sig.contextualize();
        }

        let small_black_count = sig
            .vertices
            .iter()
            .filter(|vertex| {
                vertex.active
                    && vertex.kind == NativeSigInterKind::Head
                    && vertex.shape.as_deref() == Some("NOTEHEAD_BLACK_SMALL")
            })
            .count();
        let mut qualified_heads = Vec::new();
        for head in sig.vertices.iter().filter(|vertex| {
            vertex.active
                && vertex.kind == NativeSigInterKind::Head
                && vertex.shape.as_deref() == Some("NOTEHEAD_BLACK_SMALL")
                && vertex
                    .contextual_grade
                    .is_some_and(|grade| grade >= MIN_CONTEXTUAL_GRADE)
        }) {
            let stem = sig
                .incident_edges(head.ordinal)
                .expect("active head has valid incident-edge lookup")
                .into_iter()
                .find(|edge| edge.kind == NativeSigRelationKind::HeadStem)
                .map(|edge| edge.target)
                .ok_or(NativeCueAggregateError::MissingStem {
                    system_id,
                    head_sig_ordinal: head.ordinal,
                })?;
            let stem_vertex = sig
                .vertex(stem)
                .ok_or(NativeCueAggregateError::InvalidStem {
                    system_id,
                    head_sig_ordinal: head.ordinal,
                    stem_sig_ordinal: stem,
                })?;
            if stem_vertex.kind != NativeSigInterKind::Stem || stem == head.ordinal {
                return Err(NativeCueAggregateError::InvalidStem {
                    system_id,
                    head_sig_ordinal: head.ordinal,
                    stem_sig_ordinal: stem,
                });
            }
            qualified_heads.push(NativeCueAggregateHead {
                sig_ordinal: NativeSigVertexId(head.ordinal),
                stem_sig_ordinal: NativeSigVertexId(stem),
                bounds: head.bounds,
                grade: head.grade,
                contextual_grade: head
                    .contextual_grade
                    .expect("qualified head has a contextual grade"),
                aggregate_ordinal: None,
            });
        }
        qualified_heads.sort_by_key(|head| head.bounds.x);

        let stem_bounds = qualified_heads
            .iter()
            .map(|head| {
                sig.vertex(head.stem_sig_ordinal.0)
                    .expect("validated cue stem remains active")
                    .bounds
            })
            .collect::<Vec<_>>();
        let aggregates = group_cue_candidates(
            &mut qualified_heads,
            &stem_bounds,
            cue_x_margin,
            cue_y_margin,
        );

        systems.push(NativeCueAggregateSystem {
            system_id,
            interline,
            cue_x_margin,
            cue_y_margin,
            small_black_count,
            qualified_heads,
            aggregates,
        });
    }

    Ok(NativeCueAggregateRecognition { systems })
}

/// Plan Java `CueAggregate.process()` through its direction gate and exact
/// sheet-clipped NO_STAFF crop, without yet running morphology or mutating the
/// graph.
pub fn plan_native_cue_aggregate_processing(
    grid: &GridLinesRecognition,
    reduction: &NativeReductionRecognition,
    aggregates: &NativeCueAggregateRecognition,
) -> Result<NativeCueAggregateProcessRecognition, NativeCueAggregateError> {
    let sheet_width = i32::try_from(grid.scale.width).unwrap_or(i32::MAX);
    let sheet_height = i32::try_from(grid.scale.height).unwrap_or(i32::MAX);
    let params = SheetParameters::new(reduction.stems.reduction_interline);
    let cue_beam_height = f64::from(grid.scale.scale.beam.main) * params.cue_beam_ratio;
    let mut systems = Vec::with_capacity(aggregates.systems.len());

    for aggregate_system in &aggregates.systems {
        let finalized = reduction
            .stems
            .systems
            .iter()
            .find(|system| system.system_id == aggregate_system.system_id)
            .ok_or(NativeCueAggregateError::MissingSystem {
                system_id: aggregate_system.system_id,
            })?;
        let sig = &finalized.transaction.state_after.beam_state.sig;
        let mut plans = Vec::with_capacity(aggregate_system.aggregates.len());
        for aggregate in &aggregate_system.aggregates {
            let mut member_bounds = Vec::with_capacity(aggregate.members.len());
            for &(head, stem) in &aggregate.members {
                let head_bounds = sig.vertex(head.0).map(|vertex| vertex.bounds).ok_or(
                    NativeCueAggregateError::MissingMember {
                        system_id: aggregate_system.system_id,
                        aggregate_ordinal: aggregate.ordinal,
                        sig_ordinal: head.0,
                    },
                )?;
                let stem_bounds = sig.vertex(stem.0).map(|vertex| vertex.bounds).ok_or(
                    NativeCueAggregateError::MissingMember {
                        system_id: aggregate_system.system_id,
                        aggregate_ordinal: aggregate.ordinal,
                        sig_ordinal: stem.0,
                    },
                )?;
                member_bounds.push((head_bounds, stem_bounds));
            }
            let (global_direction, cue_box) = cue_process_geometry(
                aggregate.bounds,
                &member_bounds,
                params.cue_box_dx,
                params.cue_box_dy,
                sheet_width,
                sheet_height,
            );
            plans.push(NativeCueAggregateProcessPlan {
                system_id: aggregate_system.system_id,
                aggregate_ordinal: aggregate.ordinal,
                aggregate_bounds: aggregate.bounds,
                global_direction,
                cue_box,
                cue_box_dx: params.cue_box_dx,
                cue_box_dy: params.cue_box_dy,
                cue_beam_height,
            });
        }
        systems.push(NativeCueAggregateProcessSystem {
            system_id: aggregate_system.system_id,
            plans,
        });
    }

    Ok(NativeCueAggregateProcessRecognition { systems })
}

/// Execute Java `CueAggregate.getCueGlyphs()` over every processable plan.
///
/// This is deliberately still read-only: glyph registration, BEAM_SPOT
/// retention, beam grading, grouping, and stem links belong to the subsequent
/// `CueAggregate.process()` mutation slice.
pub fn extract_native_cue_spots(
    grid: &GridLinesRecognition,
    processing: &NativeCueAggregateProcessRecognition,
) -> Result<NativeCueSpotRecognition, RunTableError> {
    let pixels = grid.no_staff.to_pixels();
    let mut aggregates = Vec::new();
    for system in &processing.systems {
        for plan in &system.plans {
            let Some(cue_box) = plan.cue_box else {
                continue;
            };
            let (closing_radius, closed_digest, glyphs) = extract_cue_spot_components(
                &pixels,
                grid.scale.width,
                grid.scale.height,
                cue_box,
                plan.cue_beam_height,
            )?;
            aggregates.push(NativeCueAggregateSpots {
                system_id: system.system_id,
                aggregate_ordinal: plan.aggregate_ordinal,
                cue_box,
                closing_radius,
                closed_digest,
                glyphs,
            });
        }
    }
    Ok(NativeCueSpotRecognition { aggregates })
}

fn extract_cue_spot_components(
    page: &[u8],
    page_width: usize,
    page_height: usize,
    cue_box: NativeSigBounds,
    cue_beam_height: f64,
) -> Result<(f32, String, Vec<GlyphComponent>), RunTableError> {
    if page.len() != page_width.saturating_mul(page_height)
        || cue_box.width <= 0
        || cue_box.height <= 0
    {
        return Err(RunTableError::InvalidDimensions);
    }
    let width = usize::try_from(cue_box.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(cue_box.height).map_err(|_| RunTableError::InvalidDimensions)?;
    let left = usize::try_from(cue_box.x).map_err(|_| RunTableError::OutOfBounds)?;
    let top = usize::try_from(cue_box.y).map_err(|_| RunTableError::OutOfBounds)?;
    if left.saturating_add(width) > page_width || top.saturating_add(height) > page_height {
        return Err(RunTableError::OutOfBounds);
    }

    let mut buffer = Vec::with_capacity(width.saturating_mul(height));
    for y in top..top + height {
        buffer.extend_from_slice(&page[(y * page_width) + left..(y * page_width) + left + width]);
    }
    let diameter = cue_beam_height * BEAM_CIRCLE_DIAMETER_RATIO;
    let closing_radius = (diameter - 1.0) as f32 / 2.0;
    close_with_disk(&mut buffer, width, height, closing_radius);
    let closed_digest = digest(&buffer);
    let binary = buffer
        .into_iter()
        .map(|pixel| {
            if pixel <= BEAM_BINARIZATION_THRESHOLD {
                0
            } else {
                255
            }
        })
        .collect::<Vec<_>>();
    let table = RunTable::from_pixels(Orientation::Vertical, width, height, &binary)?;
    let glyphs = build_glyph_components(&table, cue_box.x, cue_box.y);
    Ok((closing_radius, closed_digest, glyphs))
}

fn cue_process_geometry(
    aggregate_bounds: NativeSigBounds,
    members: &[(NativeSigBounds, NativeSigBounds)],
    cue_box_dx: i32,
    cue_box_dy: i32,
    sheet_width: i32,
    sheet_height: i32,
) -> (i32, Option<NativeSigBounds>) {
    let mut direction: Option<i32> = None;
    for &(head, stem) in members {
        let head_y = f64::from(head.y) + (f64::from(head.height) / 2.0);
        let quarter = f64::from(stem.height) / 4.0;
        let member_direction = if head_y >= f64::from(stem.y) + f64::from(stem.height) - quarter {
            Some(-1)
        } else if head_y <= f64::from(stem.y) + quarter {
            Some(1)
        } else {
            None
        };
        if let Some(member_direction) = member_direction {
            if direction.is_some_and(|direction| direction != member_direction) {
                return (0, None);
            }
            direction = Some(member_direction);
        }
    }

    let Some(direction) = direction else {
        return (0, None);
    };
    let mut box_ = java_rectangle_grow(rectangle(aggregate_bounds), cue_box_dx, 0);
    box_.y = box_.y.saturating_add(direction.saturating_mul(cue_box_dy));
    let left = box_.x.max(0).min(sheet_width);
    let top = box_.y.max(0).min(sheet_height);
    let right = box_.x.saturating_add(box_.width).max(0).min(sheet_width);
    let bottom = box_.y.saturating_add(box_.height).max(0).min(sheet_height);
    (
        direction,
        Some(NativeSigBounds {
            x: left,
            y: top,
            width: right.saturating_sub(left),
            height: bottom.saturating_sub(top),
        }),
    )
}

fn group_cue_candidates(
    qualified_heads: &mut [NativeCueAggregateHead],
    stem_bounds: &[NativeSigBounds],
    cue_x_margin: i32,
    cue_y_margin: i32,
) -> Vec<NativeCueAggregate> {
    debug_assert_eq!(qualified_heads.len(), stem_bounds.len());
    let mut working = Vec::<WorkingCueAggregate>::new();
    for (head_index, (head, &stem_bounds)) in qualified_heads.iter().zip(stem_bounds).enumerate() {
        let head_box = java_rectangle_grow(rectangle(head.bounds), cue_x_margin, cue_y_margin);
        let aggregate = working
            .iter()
            .position(|aggregate| rectangle(aggregate.bounds).intersects(head_box));
        if let Some(aggregate) = aggregate {
            working[aggregate].bounds = bounds_union(working[aggregate].bounds, head.bounds);
            working[aggregate].bounds = bounds_union(working[aggregate].bounds, stem_bounds);
            working[aggregate].member_indices.push(head_index);
        } else {
            working.push(WorkingCueAggregate {
                bounds: bounds_union(head.bounds, stem_bounds),
                member_indices: vec![head_index],
            });
        }
    }

    let mut aggregates = Vec::new();
    for aggregate in working
        .into_iter()
        .filter(|aggregate| aggregate.member_indices.len() > 1)
    {
        let ordinal = aggregates.len();
        let members = aggregate
            .member_indices
            .iter()
            .map(|&index| {
                qualified_heads[index].aggregate_ordinal = Some(ordinal);
                (
                    qualified_heads[index].sig_ordinal,
                    qualified_heads[index].stem_sig_ordinal,
                )
            })
            .collect();
        aggregates.push(NativeCueAggregate {
            ordinal,
            bounds: aggregate.bounds,
            members,
        });
    }
    aggregates
}

fn rectangle(bounds: NativeSigBounds) -> JavaRectangle {
    JavaRectangle::new(bounds.x, bounds.y, bounds.width, bounds.height)
}

fn bounds_union(one: NativeSigBounds, two: NativeSigBounds) -> NativeSigBounds {
    let left = i64::from(one.x).min(i64::from(two.x));
    let top = i64::from(one.y).min(i64::from(two.y));
    let right =
        (i64::from(one.x) + i64::from(one.width)).max(i64::from(two.x) + i64::from(two.width));
    let bottom =
        (i64::from(one.y) + i64::from(one.height)).max(i64::from(two.y) + i64::from(two.height));
    NativeSigBounds {
        x: left.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y: top.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        width: (right - left).clamp(0, i64::from(i32::MAX)) as i32,
        height: (bottom - top).clamp(0, i64::from(i32::MAX)) as i32,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueBeamsReport<VisualError> {
    pub skip_reason: Option<CueBeamsSkipReason>,
    pub system_errors: Vec<(usize, VisualError)>,
    /// Active Java context at end of the empty epilog; `None` when skipped.
    pub context: Option<CueBeamsContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsStepError<VisualError> {
    FatalSystem {
        system_id: usize,
        source: VisualError,
        checked_errors: Vec<(usize, VisualError)>,
        context: CueBeamsContext,
    },
    Contract(CueBeamsContractError),
}

impl<VisualError: fmt::Display> fmt::Display for CueBeamsStepError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FatalSystem {
                system_id, source, ..
            } => write!(formatter, "cue beams system {system_id} failed: {source}"),
            Self::Contract(source) => write!(formatter, "cue beams contract failed: {source}"),
        }
    }
}

impl<VisualError: Error + 'static> Error for CueBeamsStepError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FatalSystem { source, .. } => Some(source),
            Self::Contract(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CueBeamsContractError {
    DuplicateSystem(usize),
    DuplicateGlyph(usize),
    InvalidGlyphAllocator {
        expected: usize,
        actual: usize,
    },
    DuplicateInter(usize),
    InvalidInterAllocator {
        expected: usize,
        actual: usize,
    },
    InvalidLiveInterIndex,
    InvalidRetiredInter(usize),
    UnknownSystem(usize),
    UnknownGlyph(usize),
    SpotGlyphMissingGroup(usize),
    DuplicateSpot(usize),
    UnknownSpot(usize),
    DuplicateSpotLagSection(usize),
    UnknownInter {
        system_id: usize,
        inter_id: usize,
    },
    DuplicateRelation {
        system_id: usize,
        relation_id: usize,
    },
    UnknownRelation {
        system_id: usize,
        relation_id: usize,
    },
    UnknownRelationEndpoint(usize),
    DuplicateMember {
        owner_id: usize,
        member_id: usize,
    },
    UnknownMember {
        owner_id: usize,
        member_id: usize,
    },
}

impl fmt::Display for CueBeamsContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for CueBeamsContractError {}

pub struct HeadlessCueBeamsStep<Visual> {
    visual: Visual,
    cue_beam_ratio: f64,
}

impl<Visual> HeadlessCueBeamsStep<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            cue_beam_ratio: 0.6,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual: VisualCueBeams> HeadlessCueBeamsStep<Visual> {
    /// Java `AbstractSystemStep.doit`: gated prolog, every system, empty epilog.
    pub fn process(
        &mut self,
        sheet: &mut NeutralCueBeamsSheet,
    ) -> Result<CueBeamsReport<Visual::Error>, CueBeamsStepError<Visual::Error>> {
        validate_sheet(sheet).map_err(CueBeamsStepError::Contract)?;
        let skip_reason = if !sheet.small_heads_enabled {
            Some(CueBeamsSkipReason::SmallHeadsDisabled)
        } else if sheet.small_beam_scale.is_some() {
            Some(CueBeamsSkipReason::SmallBeamScaleAlreadySet)
        } else {
            None
        };
        if let Some(skip_reason) = skip_reason {
            // AbstractSystemStep still traverses systems, but doSystem is a no-op
            // for null context and the inherited epilog is empty.
            return Ok(CueBeamsReport {
                skip_reason: Some(skip_reason),
                system_errors: Vec::new(),
                context: None,
            });
        }

        let mut context = CueBeamsContext::default();
        let mut system_errors = Vec::new();
        for system_index in 0..sheet.systems.len() {
            let system_id = sheet.systems[system_index].id;
            let outcome = self.visual.build_system_cue_beams(CueBeamsSystemInput {
                sheet,
                system_id,
                context: &context,
                cue_beam_ratio: self.cue_beam_ratio,
            });
            apply_delta(sheet, system_id, &mut context, outcome.delta)
                .map_err(CueBeamsStepError::Contract)?;
            match outcome.failure {
                None => {}
                Some(CueBeamsFailure::Checked(source)) => {
                    sheet
                        .mutations
                        .push(CueBeamsMutation::SystemFailed { system_id });
                    system_errors.push((system_id, source));
                }
                Some(CueBeamsFailure::Fatal(source)) => {
                    return Err(CueBeamsStepError::FatalSystem {
                        system_id,
                        source,
                        checked_errors: system_errors,
                        context,
                    });
                }
            }
        }

        Ok(CueBeamsReport {
            skip_reason: None,
            system_errors,
            context: Some(context),
        })
    }
}

fn apply_delta(
    sheet: &mut NeutralCueBeamsSheet,
    system_id: usize,
    context: &mut CueBeamsContext,
    delta: CueBeamsDelta,
) -> Result<(), CueBeamsContractError> {
    let system_index = sheet
        .systems
        .iter()
        .position(|system| system.id == system_id)
        .ok_or(CueBeamsContractError::UnknownSystem(system_id))?;
    for mutation in delta.mutations {
        match mutation {
            CueBeamsDeltaMutation::RegisterGlyph(glyph) => {
                if glyph.id != sheet.next_glyph_id {
                    return Err(CueBeamsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    });
                }
                if sheet
                    .registered_glyphs
                    .iter()
                    .any(|existing| existing.id == glyph.id)
                {
                    return Err(CueBeamsContractError::DuplicateGlyph(glyph.id));
                }
                sheet.next_glyph_id = sheet.next_glyph_id.checked_add(1).ok_or(
                    CueBeamsContractError::InvalidGlyphAllocator {
                        expected: sheet.next_glyph_id,
                        actual: glyph.id,
                    },
                )?;
                let glyph_id = glyph.id;
                sheet.registered_glyphs.push(glyph);
                sheet.mutations.push(CueBeamsMutation::GlyphRegistered {
                    system_id,
                    glyph_id,
                });
            }
            CueBeamsDeltaMutation::AppendSpot(glyph_id) => {
                let glyph = sheet
                    .registered_glyphs
                    .iter()
                    .find(|glyph| glyph.id == glyph_id)
                    .ok_or(CueBeamsContractError::UnknownGlyph(glyph_id))?;
                if !glyph.beam_spot_group {
                    return Err(CueBeamsContractError::SpotGlyphMissingGroup(glyph_id));
                }
                if context.spot_glyph_ids.contains(&glyph_id) {
                    return Err(CueBeamsContractError::DuplicateSpot(glyph_id));
                }
                context.spot_glyph_ids.push(glyph_id);
                sheet.mutations.push(CueBeamsMutation::SpotAppended {
                    system_id,
                    glyph_id,
                });
            }
            CueBeamsDeltaMutation::AddSpotLagSection(section) => {
                if context
                    .spot_lag_sections
                    .iter()
                    .any(|existing| existing.id == section.id)
                {
                    return Err(CueBeamsContractError::DuplicateSpotLagSection(section.id));
                }
                let section_id = section.id;
                context.spot_lag_sections.push(section);
                sheet.mutations.push(CueBeamsMutation::SpotLagSectionAdded {
                    system_id,
                    section_id,
                });
            }
            CueBeamsDeltaMutation::AddInter(inter) => {
                if inter.id != sheet.next_inter_id {
                    return Err(CueBeamsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    });
                }
                if sheet.live_inter_ids.contains(&inter.id)
                    || sheet.retired_inter_ids.contains(&inter.id)
                {
                    return Err(CueBeamsContractError::DuplicateInter(inter.id));
                }
                if let Some(glyph_id) = inter.glyph_id
                    && !sheet
                        .registered_glyphs
                        .iter()
                        .any(|glyph| glyph.id == glyph_id)
                {
                    return Err(CueBeamsContractError::UnknownGlyph(glyph_id));
                }
                let mut members = BTreeSet::new();
                for &member_id in &inter.member_ids {
                    if !members.insert(member_id) {
                        return Err(CueBeamsContractError::DuplicateMember {
                            owner_id: inter.id,
                            member_id,
                        });
                    }
                    require_inter(&sheet.systems[system_index], member_id)?;
                }
                let member_ids = inter.member_ids.clone();
                let inter_id = inter.id;
                sheet.next_inter_id = sheet.next_inter_id.checked_add(1).ok_or(
                    CueBeamsContractError::InvalidInterAllocator {
                        expected: sheet.next_inter_id,
                        actual: inter.id,
                    },
                )?;
                sheet.systems[system_index].inters.push(inter);
                sheet.live_inter_ids.push(inter_id);
                sheet.mutations.push(CueBeamsMutation::InterAdded {
                    system_id,
                    inter_id,
                });
                for member_id in member_ids {
                    sheet.mutations.push(CueBeamsMutation::MemberAttached {
                        system_id,
                        owner_id: inter_id,
                        member_id,
                    });
                }
            }
            CueBeamsDeltaMutation::RemoveInter(inter_id) => {
                remove_inter(sheet, system_index, inter_id)?;
            }
            CueBeamsDeltaMutation::AddRelation(relation) => {
                for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                    require_inter(&sheet.systems[system_index], endpoint)?;
                }
                if sheet.systems[system_index]
                    .relations
                    .iter()
                    .any(|existing| existing.id == relation.id)
                {
                    return Err(CueBeamsContractError::DuplicateRelation {
                        system_id,
                        relation_id: relation.id,
                    });
                }
                sheet.systems[system_index].relations.push(relation);
                sheet.mutations.push(CueBeamsMutation::RelationAdded {
                    system_id,
                    relation,
                });
            }
            CueBeamsDeltaMutation::RemoveRelation(relation_id) => {
                let relation_index = sheet.systems[system_index]
                    .relations
                    .iter()
                    .position(|relation| relation.id == relation_id)
                    .ok_or(CueBeamsContractError::UnknownRelation {
                        system_id,
                        relation_id,
                    })?;
                let relation = sheet.systems[system_index].relations.remove(relation_index);
                sheet.mutations.push(CueBeamsMutation::RelationRemoved {
                    system_id,
                    relation,
                });
            }
            CueBeamsDeltaMutation::AttachMember {
                owner_id,
                member_id,
            } => {
                require_inter(&sheet.systems[system_index], member_id)?;
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(CueBeamsContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                if owner.member_ids.contains(&member_id) {
                    return Err(CueBeamsContractError::DuplicateMember {
                        owner_id,
                        member_id,
                    });
                }
                owner.member_ids.push(member_id);
                sheet.mutations.push(CueBeamsMutation::MemberAttached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
            CueBeamsDeltaMutation::DetachMember {
                owner_id,
                member_id,
            } => {
                let owner = sheet.systems[system_index]
                    .inters
                    .iter_mut()
                    .find(|inter| inter.id == owner_id)
                    .ok_or(CueBeamsContractError::UnknownInter {
                        system_id,
                        inter_id: owner_id,
                    })?;
                let member_index = owner
                    .member_ids
                    .iter()
                    .position(|id| *id == member_id)
                    .ok_or(CueBeamsContractError::UnknownMember {
                        owner_id,
                        member_id,
                    })?;
                owner.member_ids.remove(member_index);
                sheet.mutations.push(CueBeamsMutation::MemberDetached {
                    system_id,
                    owner_id,
                    member_id,
                });
            }
        }
    }
    Ok(())
}

fn require_inter(system: &NeutralCueSystem, inter_id: usize) -> Result<(), CueBeamsContractError> {
    system
        .inters
        .iter()
        .any(|inter| inter.id == inter_id)
        .then_some(())
        .ok_or(CueBeamsContractError::UnknownInter {
            system_id: system.id,
            inter_id,
        })
}

fn remove_inter(
    sheet: &mut NeutralCueBeamsSheet,
    system_index: usize,
    inter_id: usize,
) -> Result<(), CueBeamsContractError> {
    let system_id = sheet.systems[system_index].id;
    let inter_index = sheet.systems[system_index]
        .inters
        .iter()
        .position(|inter| inter.id == inter_id)
        .ok_or(CueBeamsContractError::UnknownInter {
            system_id,
            inter_id,
        })?;
    let relations = sheet.systems[system_index]
        .relations
        .iter()
        .copied()
        .filter(|relation| {
            relation.source_inter_id == inter_id || relation.target_inter_id == inter_id
        })
        .collect::<Vec<_>>();
    for relation in relations {
        let relation_index = sheet.systems[system_index]
            .relations
            .iter()
            .position(|candidate| *candidate == relation)
            .expect("relation snapshot remains live");
        sheet.systems[system_index].relations.remove(relation_index);
        sheet.mutations.push(CueBeamsMutation::RelationRemoved {
            system_id,
            relation,
        });
    }
    let owner_ids = sheet.systems[system_index]
        .inters
        .iter()
        .filter(|owner| owner.member_ids.contains(&inter_id))
        .map(|owner| owner.id)
        .collect::<Vec<_>>();
    for owner_id in owner_ids {
        let owner = sheet.systems[system_index]
            .inters
            .iter_mut()
            .find(|owner| owner.id == owner_id)
            .expect("owner snapshot remains live");
        owner.member_ids.retain(|id| *id != inter_id);
        sheet.mutations.push(CueBeamsMutation::MemberDetached {
            system_id,
            owner_id,
            member_id: inter_id,
        });
    }
    for member_id in sheet.systems[system_index].inters[inter_index]
        .member_ids
        .clone()
    {
        sheet.mutations.push(CueBeamsMutation::MemberDetached {
            system_id,
            owner_id: inter_id,
            member_id,
        });
    }
    sheet.systems[system_index].inters.remove(inter_index);
    let live_index = sheet
        .live_inter_ids
        .iter()
        .position(|id| *id == inter_id)
        .ok_or(CueBeamsContractError::InvalidLiveInterIndex)?;
    sheet.live_inter_ids.remove(live_index);
    sheet.retired_inter_ids.push(inter_id);
    sheet.mutations.push(CueBeamsMutation::InterRemoved {
        system_id,
        inter_id,
    });
    Ok(())
}

fn validate_sheet(sheet: &NeutralCueBeamsSheet) -> Result<(), CueBeamsContractError> {
    let mut glyph_ids = BTreeSet::new();
    for glyph in &sheet.registered_glyphs {
        if !glyph_ids.insert(glyph.id) {
            return Err(CueBeamsContractError::DuplicateGlyph(glyph.id));
        }
        if glyph.id >= sheet.next_glyph_id {
            return Err(CueBeamsContractError::InvalidGlyphAllocator {
                expected: sheet.next_glyph_id,
                actual: glyph.id,
            });
        }
    }
    let mut system_ids = BTreeSet::new();
    let mut inter_ids = BTreeSet::new();
    for system in &sheet.systems {
        if !system_ids.insert(system.id) {
            return Err(CueBeamsContractError::DuplicateSystem(system.id));
        }
        for inter in &system.inters {
            if !inter_ids.insert(inter.id) {
                return Err(CueBeamsContractError::DuplicateInter(inter.id));
            }
            if inter.id >= sheet.next_inter_id {
                return Err(CueBeamsContractError::InvalidInterAllocator {
                    expected: sheet.next_inter_id,
                    actual: inter.id,
                });
            }
            if let Some(glyph_id) = inter.glyph_id
                && !glyph_ids.contains(&glyph_id)
            {
                return Err(CueBeamsContractError::UnknownGlyph(glyph_id));
            }
            let mut members = BTreeSet::new();
            for &member_id in &inter.member_ids {
                if !members.insert(member_id) {
                    return Err(CueBeamsContractError::DuplicateMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
                if !system.inters.iter().any(|member| member.id == member_id) {
                    return Err(CueBeamsContractError::UnknownMember {
                        owner_id: inter.id,
                        member_id,
                    });
                }
            }
        }
        let mut relation_ids = BTreeSet::new();
        for relation in &system.relations {
            if !relation_ids.insert(relation.id) {
                return Err(CueBeamsContractError::DuplicateRelation {
                    system_id: system.id,
                    relation_id: relation.id,
                });
            }
            for endpoint in [relation.source_inter_id, relation.target_inter_id] {
                if !system.inters.iter().any(|inter| inter.id == endpoint) {
                    return Err(CueBeamsContractError::UnknownRelationEndpoint(endpoint));
                }
            }
        }
    }
    let live = sheet
        .live_inter_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if live.len() != sheet.live_inter_ids.len() || live != inter_ids {
        return Err(CueBeamsContractError::InvalidLiveInterIndex);
    }
    let mut retired = BTreeSet::new();
    for &inter_id in &sheet.retired_inter_ids {
        if inter_id >= sheet.next_inter_id
            || inter_ids.contains(&inter_id)
            || !retired.insert(inter_id)
        {
            return Err(CueBeamsContractError::InvalidRetiredInter(inter_id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeCueBeams {
        calls: Vec<(usize, Vec<usize>, Vec<usize>, f64)>,
        outcomes: BTreeMap<usize, CueBeamsSystemOutcome<&'static str>>,
    }

    impl VisualCueBeams for FakeCueBeams {
        type Error = &'static str;

        fn build_system_cue_beams(
            &mut self,
            input: CueBeamsSystemInput<'_>,
        ) -> CueBeamsSystemOutcome<Self::Error> {
            self.calls.push((
                input.system_id,
                input.context.spot_glyph_ids.clone(),
                input
                    .context
                    .spot_lag_sections
                    .iter()
                    .map(|section| section.id)
                    .collect(),
                input.cue_beam_ratio,
            ));
            self.outcomes
                .remove(&input.system_id)
                .unwrap_or_else(|| CueBeamsSystemOutcome::success(CueBeamsDelta::default()))
        }
    }

    fn inter(id: usize, kind: NeutralCueInterKind) -> NeutralCueInter {
        NeutralCueInter {
            id,
            kind,
            glyph_id: None,
            member_ids: Vec::new(),
        }
    }

    fn sheet() -> NeutralCueBeamsSheet {
        NeutralCueBeamsSheet {
            id: 3,
            small_heads_enabled: true,
            small_beam_scale: None,
            systems: vec![
                NeutralCueSystem {
                    id: 2,
                    inters: vec![inter(1, NeutralCueInterKind::SmallHead)],
                    relations: Vec::new(),
                },
                NeutralCueSystem {
                    id: 1,
                    inters: vec![inter(2, NeutralCueInterKind::Stem)],
                    relations: Vec::new(),
                },
            ],
            registered_glyphs: Vec::new(),
            next_glyph_id: 1,
            live_inter_ids: vec![1, 2],
            retired_inter_ids: Vec::new(),
            next_inter_id: 3,
            mutations: Vec::new(),
        }
    }

    fn first_system_delta() -> CueBeamsDelta {
        CueBeamsDelta {
            mutations: vec![
                CueBeamsDeltaMutation::RegisterGlyph(NeutralCueGlyph {
                    id: 1,
                    beam_spot_group: true,
                }),
                CueBeamsDeltaMutation::AppendSpot(1),
                CueBeamsDeltaMutation::AddSpotLagSection(NeutralCueSpotSection { id: 9 }),
                CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                    id: 3,
                    kind: NeutralCueInterKind::SmallBeam,
                    glyph_id: Some(1),
                    member_ids: Vec::new(),
                }),
                CueBeamsDeltaMutation::AddRelation(NeutralCueRelation {
                    id: 1,
                    source_inter_id: 3,
                    target_inter_id: 1,
                    kind: NeutralCueRelationKind::BeamStem,
                }),
            ],
        }
    }

    #[test]
    fn cue_aggregate_grouping_uses_first_intersection_and_purges_singletons() {
        let make_head = |sig, stem, x| NativeCueAggregateHead {
            sig_ordinal: NativeSigVertexId(sig),
            stem_sig_ordinal: NativeSigVertexId(stem),
            bounds: NativeSigBounds {
                x,
                y: 50,
                width: 12,
                height: 10,
            },
            grade: 0.7,
            contextual_grade: 0.8,
            aggregate_ordinal: None,
        };
        let mut heads = vec![
            make_head(10, 20, 100),
            make_head(11, 21, 130),
            make_head(12, 22, 300),
        ];
        let stems = vec![
            NativeSigBounds {
                x: 110,
                y: 20,
                width: 4,
                height: 80,
            },
            NativeSigBounds {
                x: 140,
                y: 25,
                width: 4,
                height: 75,
            },
            NativeSigBounds {
                x: 310,
                y: 20,
                width: 4,
                height: 80,
            },
        ];

        let aggregates = group_cue_candidates(&mut heads, &stems, 20, 30);

        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].ordinal, 0);
        assert_eq!(
            aggregates[0].members,
            [
                (NativeSigVertexId(10), NativeSigVertexId(20)),
                (NativeSigVertexId(11), NativeSigVertexId(21))
            ]
        );
        assert_eq!(
            aggregates[0].bounds,
            NativeSigBounds {
                x: 100,
                y: 20,
                width: 44,
                height: 80,
            }
        );
        assert_eq!(heads[0].aggregate_ordinal, Some(0));
        assert_eq!(heads[1].aggregate_ordinal, Some(0));
        assert_eq!(heads[2].aggregate_ordinal, None);
    }

    #[test]
    fn cue_process_direction_gates_before_the_exact_clipped_crop() {
        let bounds = NativeSigBounds {
            x: 5,
            y: 10,
            width: 40,
            height: 80,
        };
        let stem = NativeSigBounds {
            x: 20,
            y: 0,
            width: 4,
            height: 80,
        };
        let up_head_at_inclusive_quarter = NativeSigBounds {
            x: 10,
            y: 55,
            width: 12,
            height: 10,
        };
        let (direction, crop) = cue_process_geometry(
            bounds,
            &[(up_head_at_inclusive_quarter, stem)],
            5,
            20,
            40,
            75,
        );
        assert_eq!(direction, -1);
        assert_eq!(
            crop,
            Some(NativeSigBounds {
                x: 0,
                y: 0,
                width: 40,
                height: 70,
            })
        );

        let down_head_at_inclusive_quarter = NativeSigBounds {
            y: 15,
            ..up_head_at_inclusive_quarter
        };
        let (direction, crop) = cue_process_geometry(
            bounds,
            &[(down_head_at_inclusive_quarter, stem)],
            5,
            20,
            100,
            100,
        );
        assert_eq!(direction, 1);
        assert_eq!(crop.unwrap().y, 30);

        let middle_head = NativeSigBounds {
            y: 35,
            ..up_head_at_inclusive_quarter
        };
        assert_eq!(
            cue_process_geometry(bounds, &[(middle_head, stem)], 5, 20, 100, 100),
            (0, None)
        );
        assert_eq!(
            cue_process_geometry(
                bounds,
                &[
                    (up_head_at_inclusive_quarter, stem),
                    (down_head_at_inclusive_quarter, stem),
                ],
                5,
                20,
                100,
                100,
            ),
            (0, None)
        );
    }

    #[test]
    fn cue_spot_extraction_closes_thresholds_and_restores_sheet_coordinates() {
        let width = 20;
        let height = 20;
        let mut page = vec![255_u8; width * height];
        for y in 8..10 {
            for x in 6..12 {
                page[(y * width) + x] = 0;
            }
        }
        let (radius, _, glyphs) = extract_cue_spot_components(
            &page,
            width,
            height,
            NativeSigBounds {
                x: 4,
                y: 6,
                width: 12,
                height: 8,
            },
            1.25,
        )
        .unwrap();

        assert_eq!(radius.to_bits(), 0.0_f32.to_bits());
        assert_eq!(glyphs.len(), 1);
        let glyph = &glyphs[0];
        assert_eq!((glyph.left, glyph.top), (6, 8));
        assert_eq!((glyph.width, glyph.height, glyph.weight), (6, 2, 12));
        assert_eq!((glyph.centroid_x, glyph.centroid_y), (8.5, 8.5));
        assert_eq!(glyph.orientation, Orientation::Vertical);
    }

    #[test]
    fn exact_active_lifecycle_shares_vertical_context_across_systems() {
        let mut visual = FakeCueBeams::default();
        visual
            .outcomes
            .insert(2, CueBeamsSystemOutcome::success(first_system_delta()));
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            step.visual().calls,
            [(2, vec![], vec![], 0.6), (1, vec![1], vec![9], 0.6)]
        );
        assert_eq!(
            report.context,
            Some(CueBeamsContext {
                spot_orientation: CueSpotOrientation::Vertical,
                spot_glyph_ids: vec![1],
                spot_lag_sections: vec![NeutralCueSpotSection { id: 9 }],
            })
        );
        assert_eq!(sheet.live_inter_ids, [1, 2, 3]);
        assert_eq!(sheet.registered_glyphs.len(), 1);
    }

    #[test]
    fn skip_gate_priority_matches_java_and_has_no_visual_or_context() {
        let mut sheet = sheet();
        sheet.small_heads_enabled = false;
        sheet.small_beam_scale = Some(3);
        let mut step = HeadlessCueBeamsStep::new(FakeCueBeams::default());

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.skip_reason,
            Some(CueBeamsSkipReason::SmallHeadsDisabled)
        );
        assert!(report.context.is_none());
        assert!(step.visual().calls.is_empty());
        assert!(sheet.mutations.is_empty());
    }

    #[test]
    fn existing_small_beam_scale_is_second_no_op_gate() {
        let mut sheet = sheet();
        sheet.small_beam_scale = Some(3);
        let mut step = HeadlessCueBeamsStep::new(FakeCueBeams::default());

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(
            report.skip_reason,
            Some(CueBeamsSkipReason::SmallBeamScaleAlreadySet)
        );
        assert!(step.visual().calls.is_empty());
    }

    #[test]
    fn checked_failure_keeps_registered_spot_prefix_and_continues() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome {
                delta: first_system_delta(),
                failure: Some(CueBeamsFailure::Checked("checked")),
            },
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        let report = step.process(&mut sheet).unwrap();

        assert_eq!(report.system_errors, [(2, "checked")]);
        assert_eq!(step.visual().calls[1].1, [1]);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
        assert!(
            sheet
                .mutations
                .contains(&CueBeamsMutation::SystemFailed { system_id: 2 })
        );
    }

    #[test]
    fn fatal_failure_returns_shared_context_and_skips_later_system() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome {
                delta: CueBeamsDelta {
                    mutations: first_system_delta().mutations[..3].to_vec(),
                },
                failure: Some(CueBeamsFailure::Fatal("fatal")),
            },
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        assert_eq!(
            step.process(&mut sheet),
            Err(CueBeamsStepError::FatalSystem {
                system_id: 2,
                source: "fatal",
                checked_errors: Vec::new(),
                context: CueBeamsContext {
                    spot_orientation: CueSpotOrientation::Vertical,
                    spot_glyph_ids: vec![1],
                    spot_lag_sections: vec![NeutralCueSpotSection { id: 9 }],
                },
            })
        );
        assert_eq!(step.visual().calls.len(), 1);
        assert_eq!(sheet.registered_glyphs[0].id, 1);
    }

    #[test]
    fn removal_preserves_allocator_hole_and_cascades_relation_and_group_ownership() {
        let mut visual = FakeCueBeams::default();
        visual.outcomes.insert(
            2,
            CueBeamsSystemOutcome::success(CueBeamsDelta {
                mutations: vec![
                    CueBeamsDeltaMutation::RegisterGlyph(NeutralCueGlyph {
                        id: 1,
                        beam_spot_group: true,
                    }),
                    CueBeamsDeltaMutation::AppendSpot(1),
                    CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                        id: 3,
                        kind: NeutralCueInterKind::SmallBeam,
                        glyph_id: Some(1),
                        member_ids: Vec::new(),
                    }),
                    CueBeamsDeltaMutation::AddInter(NeutralCueInter {
                        id: 4,
                        kind: NeutralCueInterKind::BeamGroup,
                        glyph_id: None,
                        member_ids: vec![3],
                    }),
                    CueBeamsDeltaMutation::AddRelation(NeutralCueRelation {
                        id: 1,
                        source_inter_id: 3,
                        target_inter_id: 1,
                        kind: NeutralCueRelationKind::BeamStem,
                    }),
                    CueBeamsDeltaMutation::RemoveInter(3),
                ],
            }),
        );
        let mut step = HeadlessCueBeamsStep::new(visual);
        let mut sheet = sheet();

        step.process(&mut sheet).unwrap();

        assert_eq!(sheet.retired_inter_ids, [3]);
        assert_eq!(sheet.next_inter_id, 5);
        assert_eq!(sheet.live_inter_ids, [1, 2, 4]);
        assert!(sheet.systems[0].relations.is_empty());
        assert!(sheet.systems[0].inters[1].member_ids.is_empty());
        assert!(sheet.mutations.iter().any(|mutation| matches!(
            mutation,
            CueBeamsMutation::MemberDetached {
                owner_id: 4,
                member_id: 3,
                ..
            }
        )));
    }
}
