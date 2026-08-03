// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral orchestration port of Java `ClefBuilder.Column`.
//!
//! Pixel extraction, glyph clustering, shape classification, font bounds,
//! and clef-kind inference remain one injected visual dependency.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use audiveris_image::{
    glyph_factory::{GlyphComponent, build_glyph_components},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

use crate::{
    header_builder::HeaderSigExclusion,
    headers_step::HeadlessHeaderSystem,
    staff_header::{HeaderBounds, HeaderComponent, StaffHeaderRange},
};

/// Java `Constants.maxClefEnd` before staff-interline scale conversion.
pub const MAXIMUM_CLEF_END_INTERLINES: f64 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NeutralClefKind {
    Treble,
    Bass,
    Baritone,
    Tenor,
    Alto,
    MezzoSoprano,
    Soprano,
    Percussion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralClefShape {
    Treble,
    TrebleOttavaAlta,
    TrebleOttavaBassa,
    Bass,
    C,
    Percussion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClefRecognitionPass {
    OuterAndInner,
    InnerOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupContext {
    pub outer: ClefLookupRect,
    pub inner: ClefLookupRect,
    pub percussion_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupStaffGeometry {
    pub staff_id: usize,
    pub browse_start: i32,
    pub browse_stop: i32,
    pub left_abscissa: i32,
    pub right_abscissa: i32,
    pub first_line_y: i32,
    pub last_line_y: i32,
    pub percussion_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefLookupParameters {
    pub sheet_height: i32,
    pub above_staff: i32,
    pub below_staff: i32,
    pub belt_margin: i32,
    pub x_core_margin: i32,
    pub y_core_margin: i32,
}

/// Java `getOuterRect` + `getInnerRect`, using the dependency-light constant
/// staff-line geometry supplied by the headless system.
#[must_use]
pub fn build_clef_lookup_contexts(
    staffs: &[ClefLookupStaffGeometry],
    parameters: ClefLookupParameters,
) -> BTreeMap<usize, ClefLookupContext> {
    let mut contexts = BTreeMap::new();
    for staff in staffs {
        let x_mid = staff.browse_start.wrapping_add(staff.browse_stop) / 2;
        let mut y_min = 0.max(staff.first_line_y.wrapping_sub(parameters.above_staff));
        let mut y_max = (parameters.sheet_height - 1)
            .min(staff.last_line_y.wrapping_add(parameters.below_staff));
        for neighbor in staffs {
            if neighbor.staff_id == staff.staff_id
                || neighbor.left_abscissa >= x_mid
                || neighbor.right_abscissa <= x_mid
            {
                continue;
            }
            if neighbor.last_line_y < staff.first_line_y {
                y_min = y_min.max(div_ceil_two(neighbor.last_line_y + staff.first_line_y + 1));
            } else if neighbor.first_line_y > staff.last_line_y {
                y_max = y_max.min((staff.last_line_y + neighbor.first_line_y - 1) / 2);
            }
        }
        let outer = ClefLookupRect {
            x: staff.browse_start + parameters.belt_margin,
            y: y_min,
            width: staff.browse_stop - staff.browse_start + 1 - (2 * parameters.belt_margin),
            height: y_max - y_min + 1,
        };
        let inner = ClefLookupRect {
            x: outer.x + parameters.x_core_margin,
            y: outer.y + parameters.y_core_margin,
            width: outer.width - parameters.x_core_margin,
            height: outer.height - (2 * parameters.y_core_margin),
        };
        contexts.insert(
            staff.staff_id,
            ClefLookupContext {
                outer,
                inner,
                percussion_only: staff.percussion_only,
            },
        );
    }
    contexts
}

fn div_ceil_two(value: i32) -> i32 {
    (value / 2) + i32::from(value % 2 != 0 && value > 0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClefClassifierProposal {
    pub id: usize,
    pub glyph_id: usize,
    pub shape: NeutralClefShape,
    /// Raw classifier grade; Rust applies the intrinsic ratio.
    pub classifier_grade: f64,
    /// Deterministic target pitch derived from glyph center/staff geometry.
    pub reference_pitch: i32,
    pub symbol_bounds: HeaderBounds,
    pub glyph_bounds: HeaderBounds,
}

pub trait VisualClefProposalRecognizer {
    type Error;

    fn existing_clef(
        &mut self,
        system_id: usize,
        staff_id: usize,
        outer: ClefLookupRect,
    ) -> Result<Option<NeutralClefCandidate>, Self::Error>;

    fn classify_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        pass: ClefRecognitionPass,
        lookup: ClefLookupRect,
    ) -> Result<Vec<ClefClassifierProposal>, Self::Error>;
}

/// Scale-dependent Java `ClefBuilder.Parameters` needed before the neural
/// classifier. Pixel values are supplied after the same large/specific
/// interline conversions used by Java.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeClefParameters {
    pub staff_interline: i32,
    pub first_line_y: f64,
    pub last_line_y: f64,
    pub min_part_weight: usize,
    pub max_part_count: usize,
    pub max_part_gap: f64,
    pub max_glyph_height: f64,
    pub min_glyph_weight: usize,
    pub max_eval_rank: usize,
    pub minimum_classifier_grade: f64,
    pub f_area_pitch_offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeClefGlyph {
    pub id: usize,
    pub part_ids: Vec<usize>,
    pub bounds: HeaderBounds,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub raster: RunTable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClefShapeEvaluation {
    pub shape: NeutralClefShape,
    pub grade: f64,
    /// Font-derived theoretical bounds, before Java's x-centroid correction.
    pub symbol_bounds: HeaderBounds,
}

/// The sole remaining production dependency: Audiveris `ShapeClassifier`.
pub trait ClefShapeClassifier {
    type Error;

    fn evaluate(
        &mut self,
        glyph: &NativeClefGlyph,
        staff_interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<ClefShapeEvaluation>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeClefMutation {
    PartRegistered { staff_id: usize, glyph_id: usize },
    CompoundRegistered { staff_id: usize, glyph_id: usize },
    GlyphEvaluated { staff_id: usize, glyph_id: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeClefError<ClassifierError> {
    MissingSource(usize),
    MissingParameters(usize),
    MissingContext(usize),
    InvalidLookup(ClefLookupRect),
    GlyphIdExhausted,
    InterIdExhausted,
    RunTable(RunTableError),
    Classifier(ClassifierError),
}

impl<ClassifierError: fmt::Display> fmt::Display for NativeClefError<ClassifierError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(id) => write!(formatter, "missing no-staff raster for system {id}"),
            Self::MissingParameters(id) => {
                write!(formatter, "missing clef parameters for staff {id}")
            }
            Self::MissingContext(id) => write!(formatter, "missing clef context for staff {id}"),
            Self::InvalidLookup(rect) => {
                write!(formatter, "invalid clef lookup rectangle {rect:?}")
            }
            Self::GlyphIdExhausted => formatter.write_str("clef glyph ID exhausted"),
            Self::InterIdExhausted => formatter.write_str("clef inter ID exhausted"),
            Self::RunTable(source) => write!(formatter, "clef run table failed: {source}"),
            Self::Classifier(source) => write!(formatter, "clef classifier failed: {source}"),
        }
    }
}

/// Concrete Java lookup → vertical runs → parts → near-graph → connected
/// subset decomposition. The classifier sees registered glyphs in Java trial
/// order; all filtering around it remains native.
pub struct NativeClefProposalRecognizer<Classifier> {
    classifier: Classifier,
    sources: BTreeMap<usize, RunTable>,
    contexts: BTreeMap<usize, ClefLookupContext>,
    parameters: BTreeMap<usize, NativeClefParameters>,
    next_glyph_id: usize,
    next_inter_id: usize,
    mutations: Vec<NativeClefMutation>,
}

impl<Classifier> NativeClefProposalRecognizer<Classifier> {
    #[must_use]
    pub fn new(
        classifier: Classifier,
        sources: BTreeMap<usize, RunTable>,
        contexts: BTreeMap<usize, ClefLookupContext>,
        parameters: BTreeMap<usize, NativeClefParameters>,
        next_glyph_id: usize,
        next_inter_id: usize,
    ) -> Self {
        Self {
            classifier,
            sources,
            contexts,
            parameters,
            next_glyph_id,
            next_inter_id,
            mutations: Vec::new(),
        }
    }

    #[must_use]
    pub fn mutations(&self) -> &[NativeClefMutation] {
        &self.mutations
    }

    #[must_use]
    pub const fn classifier(&self) -> &Classifier {
        &self.classifier
    }
}

#[derive(Clone)]
struct RegisteredPart {
    id: usize,
    component: GlyphComponent,
    pixels: Vec<(i32, i32)>,
}

#[derive(Clone, Copy)]
struct SubsetContext<'a> {
    staff_id: usize,
    parts: &'a [RegisteredPart],
    graph: &'a [Vec<usize>],
    parameters: NativeClefParameters,
}

impl<Classifier: ClefShapeClassifier> VisualClefProposalRecognizer
    for NativeClefProposalRecognizer<Classifier>
{
    type Error = NativeClefError<Classifier::Error>;

    fn existing_clef(
        &mut self,
        _system_id: usize,
        _staff_id: usize,
        _outer: ClefLookupRect,
    ) -> Result<Option<NeutralClefCandidate>, Self::Error> {
        // Existing artificial clefs are owned by the SIG-facing column and
        // should be supplied by a wrapper. Raster recognition creates none.
        Ok(None)
    }

    fn classify_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        pass: ClefRecognitionPass,
        lookup: ClefLookupRect,
    ) -> Result<Vec<ClefClassifierProposal>, Self::Error> {
        let source = self
            .sources
            .get(&system_id)
            .ok_or(NativeClefError::MissingSource(system_id))?;
        let parameters = *self
            .parameters
            .get(&staff_id)
            .ok_or(NativeClefError::MissingParameters(staff_id))?;
        let context = *self
            .contexts
            .get(&staff_id)
            .ok_or(NativeClefError::MissingContext(staff_id))?;
        let table = crop_vertical(source, lookup).map_err(NativeClefError::RunTable)?;
        let mut components = build_glyph_components(&table, lookup.x, lookup.y)
            .into_iter()
            .filter(|part| {
                part.weight >= parameters.min_part_weight
                    && (pass == ClefRecognitionPass::InnerOnly
                        || bounds_intersect(component_bounds(part), context.inner))
            })
            .collect::<Vec<_>>();
        if components.len() > parameters.max_part_count {
            components.sort_by_key(|component| std::cmp::Reverse(component.weight));
            components.truncate(parameters.max_part_count);
        }

        let mut parts = Vec::with_capacity(components.len());
        for component in components {
            let id = self.allocate_glyph_id()?;
            self.mutations.push(NativeClefMutation::PartRegistered {
                staff_id,
                glyph_id: id,
            });
            parts.push(RegisteredPart {
                id,
                pixels: component_pixels(&component),
                component,
            });
        }
        let graph = near_graph(&parts, parameters.max_part_gap);
        let subset_context = SubsetContext {
            staff_id,
            parts: &parts,
            graph: &graph,
            parameters,
        };
        let sets = connected_sets(&graph, &parts);
        let mut proposals = Vec::new();
        for set in sets {
            let mut seeds = set.clone();
            seeds.sort_by(|&one, &two| {
                parts[two]
                    .component
                    .weight
                    .cmp(&parts[one].component.weight)
            });
            let mut considered = Vec::new();
            for seed in seeds {
                push_unique(&mut considered, seed);
                self.process_subset(
                    subset_context,
                    vec![seed],
                    considered.clone(),
                    &mut proposals,
                )?;
            }
        }
        Ok(proposals)
    }
}

impl<Classifier: ClefShapeClassifier> NativeClefProposalRecognizer<Classifier> {
    fn allocate_glyph_id(&mut self) -> Result<usize, NativeClefError<Classifier::Error>> {
        self.next_glyph_id = self
            .next_glyph_id
            .checked_add(1)
            .ok_or(NativeClefError::GlyphIdExhausted)?;
        Ok(self.next_glyph_id)
    }

    fn process_subset(
        &mut self,
        context: SubsetContext<'_>,
        subset: Vec<usize>,
        seen: Vec<usize>,
        proposals: &mut Vec<ClefClassifierProposal>,
    ) -> Result<(), NativeClefError<Classifier::Error>> {
        let weight = subset
            .iter()
            .map(|&index| context.parts[index].component.weight)
            .sum::<usize>();
        let bounds = union_bounds(context.parts, &subset);
        if f64::from(bounds.height) > context.parameters.max_glyph_height {
            return Ok(());
        }
        if weight >= context.parameters.min_glyph_weight {
            let glyph_id = if subset.len() == 1 {
                context.parts[subset[0]].id
            } else {
                let id = self.allocate_glyph_id()?;
                self.mutations.push(NativeClefMutation::CompoundRegistered {
                    staff_id: context.staff_id,
                    glyph_id: id,
                });
                id
            };
            let glyph = compound_glyph(glyph_id, context.parts, &subset, bounds)
                .map_err(NativeClefError::RunTable)?;
            self.mutations.push(NativeClefMutation::GlyphEvaluated {
                staff_id: context.staff_id,
                glyph_id,
            });
            let evaluations = self
                .classifier
                .evaluate(
                    &glyph,
                    context.parameters.staff_interline,
                    context.parameters.max_eval_rank,
                    context.parameters.minimum_classifier_grade,
                )
                .map_err(NativeClefError::Classifier)?;
            for evaluation in evaluations
                .into_iter()
                .take(context.parameters.max_eval_rank)
            {
                if evaluation.grade < context.parameters.minimum_classifier_grade {
                    continue;
                }
                self.next_inter_id = self
                    .next_inter_id
                    .checked_add(1)
                    .ok_or(NativeClefError::InterIdExhausted)?;
                proposals.push(ClefClassifierProposal {
                    id: self.next_inter_id,
                    glyph_id,
                    shape: evaluation.shape,
                    classifier_grade: evaluation.grade,
                    reference_pitch: target_pitch(evaluation.shape, &glyph, context.parameters),
                    symbol_bounds: evaluation.symbol_bounds,
                    glyph_bounds: bounds,
                });
            }
        }

        let mut outliers = Vec::new();
        for &part in &subset {
            for &neighbor in &context.graph[part] {
                if !subset.contains(&neighbor) {
                    push_unique(&mut outliers, neighbor);
                }
            }
        }
        outliers.retain(|part| !seen.contains(part));
        let mut newly_seen = seen;
        for outlier in outliers {
            push_unique(&mut newly_seen, outlier);
            let mut larger = subset.clone();
            larger.push(outlier);
            if f64::from(union_bounds(context.parts, &larger).height)
                <= context.parameters.max_glyph_height
            {
                self.process_subset(context, larger, newly_seen.clone(), proposals)?;
            }
        }
        Ok(())
    }
}

fn crop_vertical(source: &RunTable, rect: ClefLookupRect) -> Result<RunTable, RunTableError> {
    let x = usize::try_from(rect.x).map_err(|_| RunTableError::OutOfBounds)?;
    let y = usize::try_from(rect.y).map_err(|_| RunTableError::OutOfBounds)?;
    let width = usize::try_from(rect.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(rect.height).map_err(|_| RunTableError::InvalidDimensions)?;
    if width == 0
        || height == 0
        || x.checked_add(width)
            .is_none_or(|right| right > source.width())
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > source.height())
    {
        return Err(RunTableError::OutOfBounds);
    }
    let mut pixels = vec![BACKGROUND; width * height];
    for row in 0..height {
        for column in 0..width {
            pixels[(row * width) + column] = source.get(x + column, y + row);
        }
    }
    RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)
}

fn component_bounds(component: &GlyphComponent) -> HeaderBounds {
    HeaderBounds {
        x: component.left,
        y: component.top,
        width: i32::try_from(component.width).unwrap_or(i32::MAX),
        height: i32::try_from(component.height).unwrap_or(i32::MAX),
    }
}

fn bounds_intersect(one: HeaderBounds, two: ClefLookupRect) -> bool {
    one.x < two.x + two.width
        && two.x < one.x + one.width
        && one.y < two.y + two.height
        && two.y < one.y + one.height
}

fn component_pixels(component: &GlyphComponent) -> Vec<(i32, i32)> {
    let mut pixels = Vec::with_capacity(component.weight);
    let min_sequence = component
        .runs
        .iter()
        .map(|entry| entry.sequence)
        .min()
        .unwrap_or(0);
    let min_coordinate = component
        .runs
        .iter()
        .map(|entry| entry.run.start)
        .min()
        .unwrap_or(0);
    for entry in &component.runs {
        match component.orientation {
            Orientation::Horizontal => {
                let y = component.top + i32::try_from(entry.sequence - min_sequence).unwrap();
                for coordinate in entry.run.start..=entry.run.stop() {
                    pixels.push((
                        component.left + i32::try_from(coordinate - min_coordinate).unwrap(),
                        y,
                    ));
                }
            }
            Orientation::Vertical => {
                let x = component.left + i32::try_from(entry.sequence - min_sequence).unwrap();
                for coordinate in entry.run.start..=entry.run.stop() {
                    pixels.push((
                        x,
                        component.top + i32::try_from(coordinate - min_coordinate).unwrap(),
                    ));
                }
            }
        }
    }
    pixels
}

fn near_graph(parts: &[RegisteredPart], maximum_gap: f64) -> Vec<Vec<usize>> {
    let mut order = (0..parts.len()).collect::<Vec<_>>();
    order.sort_by_key(|&index| parts[index].component.left);
    let mut graph = vec![Vec::new(); parts.len()];
    for (position, &one) in order.iter().enumerate() {
        for &two in &order[position + 1..] {
            if chamfer_gap(&parts[one].pixels, &parts[two].pixels) <= maximum_gap {
                graph[one].push(two);
                graph[two].push(one);
            }
        }
    }
    graph
}

fn chamfer_gap(one: &[(i32, i32)], two: &[(i32, i32)]) -> f64 {
    one.iter()
        .flat_map(|&(one_x, one_y)| {
            two.iter().map(move |&(two_x, two_y)| {
                let dx = (one_x - two_x).unsigned_abs();
                let dy = (one_y - two_y).unsigned_abs();
                let diagonal = dx.min(dy);
                ((4 * diagonal) + (3 * (dx.max(dy) - diagonal))) as f64 / 3.0
            })
        })
        .fold(f64::INFINITY, f64::min)
}

fn connected_sets(graph: &[Vec<usize>], parts: &[RegisteredPart]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; graph.len()];
    let mut sets = Vec::new();
    // `Glyphs.buildLinks` inserts vertices after stable left-abscissa sort;
    // ConnectivityInspector consequently discovers components in this order.
    let mut roots = (0..graph.len()).collect::<Vec<_>>();
    roots.sort_by_key(|&index| parts[index].component.left);
    for root in roots {
        if seen[root] {
            continue;
        }
        let mut stack = vec![root];
        let mut set = Vec::new();
        seen[root] = true;
        while let Some(current) = stack.pop() {
            set.push(current);
            for &neighbor in graph[current].iter().rev() {
                if !seen[neighbor] {
                    seen[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        sets.push(set);
    }
    sets
}

fn push_unique(items: &mut Vec<usize>, item: usize) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn union_bounds(parts: &[RegisteredPart], subset: &[usize]) -> HeaderBounds {
    let first = component_bounds(&parts[subset[0]].component);
    subset[1..].iter().fold(first, |bounds, &index| {
        let other = component_bounds(&parts[index].component);
        let right = bounds.right().max(other.right());
        let bottom = (bounds.y + bounds.height).max(other.y + other.height);
        HeaderBounds {
            x: bounds.x.min(other.x),
            y: bounds.y.min(other.y),
            width: right - bounds.x.min(other.x) + 1,
            height: bottom - bounds.y.min(other.y),
        }
    })
}

fn compound_glyph(
    id: usize,
    parts: &[RegisteredPart],
    subset: &[usize],
    bounds: HeaderBounds,
) -> Result<NativeClefGlyph, RunTableError> {
    let width = usize::try_from(bounds.width).map_err(|_| RunTableError::InvalidDimensions)?;
    let height = usize::try_from(bounds.height).map_err(|_| RunTableError::InvalidDimensions)?;
    let mut pixels = vec![BACKGROUND; width * height];
    let mut weight = 0_usize;
    let mut sum_x = 0_f64;
    let mut sum_y = 0_f64;
    for &index in subset {
        for &(x, y) in &parts[index].pixels {
            let local_x = usize::try_from(x - bounds.x).map_err(|_| RunTableError::OutOfBounds)?;
            let local_y = usize::try_from(y - bounds.y).map_err(|_| RunTableError::OutOfBounds)?;
            if pixels[(local_y * width) + local_x] != FOREGROUND {
                pixels[(local_y * width) + local_x] = FOREGROUND;
                weight += 1;
                sum_x += f64::from(x);
                sum_y += f64::from(y);
            }
        }
    }
    Ok(NativeClefGlyph {
        id,
        part_ids: subset.iter().map(|&index| parts[index].id).collect(),
        bounds,
        weight,
        centroid_x: sum_x / weight as f64,
        centroid_y: sum_y / weight as f64,
        raster: RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)?,
    })
}

fn target_pitch(
    shape: NeutralClefShape,
    glyph: &NativeClefGlyph,
    parameters: NativeClefParameters,
) -> i32 {
    match shape {
        NeutralClefShape::Treble
        | NeutralClefShape::TrebleOttavaAlta
        | NeutralClefShape::TrebleOttavaBassa => 2,
        NeutralClefShape::Percussion => 0,
        NeutralClefShape::Bass | NeutralClefShape::C => {
            let center_pitch = 4.0
                * ((2.0 * glyph.centroid_y) - parameters.last_line_y - parameters.first_line_y)
                / (parameters.last_line_y - parameters.first_line_y);
            let offset = if shape == NeutralClefShape::Bass {
                parameters.f_area_pitch_offset
            } else {
                0.0
            };
            let even = 2 * ((center_pitch + offset) / 2.0).round_ties_even() as i32;
            if shape == NeutralClefShape::Bass {
                even.clamp(-2, 0)
            } else {
                even.clamp(-2, 4)
            }
        }
    }
}

/// Native candidate lifecycle around the injected glyph/classifier seam.
pub struct ClefLifecycleRecognizer<Visual> {
    visual: Visual,
    contexts: BTreeMap<usize, ClefLookupContext>,
    intrinsic_ratio: f64,
    maximum_key_contribution: f64,
}

impl<Visual> ClefLifecycleRecognizer<Visual> {
    #[must_use]
    pub fn new(
        visual: Visual,
        contexts: BTreeMap<usize, ClefLookupContext>,
        intrinsic_ratio: f64,
        maximum_key_contribution: f64,
    ) -> Self {
        Self {
            visual,
            contexts,
            intrinsic_ratio,
            maximum_key_contribution,
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }
}

impl<Visual> VisualClefRecognizer for ClefLifecycleRecognizer<Visual>
where
    Visual: VisualClefProposalRecognizer,
{
    type Error = Visual::Error;

    fn find_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        _range: &StaffHeaderRange,
    ) -> Result<Vec<NeutralClefCandidate>, Self::Error> {
        let context = self.contexts[&staff_id];
        if let Some(existing) = self
            .visual
            .existing_clef(system_id, staff_id, context.outer)?
        {
            return Ok(vec![existing]);
        }
        let mut best = self.best_map(
            system_id,
            staff_id,
            context,
            ClefRecognitionPass::OuterAndInner,
        )?;
        if best.is_empty() {
            best = self.best_map(system_id, staff_id, context, ClefRecognitionPass::InnerOnly)?;
        }
        purge_clef_candidates(&mut best, self.maximum_key_contribution);
        Ok(best.into_values().collect())
    }
}

impl<Visual> ClefLifecycleRecognizer<Visual>
where
    Visual: VisualClefProposalRecognizer,
{
    fn best_map(
        &mut self,
        system_id: usize,
        staff_id: usize,
        context: ClefLookupContext,
        pass: ClefRecognitionPass,
    ) -> Result<BTreeMap<NeutralClefKind, NeutralClefCandidate>, Visual::Error> {
        let lookup = match pass {
            ClefRecognitionPass::OuterAndInner => context.outer,
            ClefRecognitionPass::InnerOnly => context.inner,
        };
        let proposals = self
            .visual
            .classify_clefs(system_id, staff_id, pass, lookup)?;
        let mut best = BTreeMap::new();
        for proposal in proposals {
            if context.percussion_only && proposal.shape != NeutralClefShape::Percussion {
                continue;
            }
            let Some(kind) = clef_kind(proposal.shape, proposal.reference_pitch) else {
                continue;
            };
            let grade = self.intrinsic_ratio * proposal.classifier_grade;
            if best
                .get(&kind)
                .is_none_or(|candidate: &NeutralClefCandidate| candidate.grade < grade)
            {
                best.insert(
                    kind,
                    NeutralClefCandidate {
                        id: proposal.id,
                        kind,
                        grade,
                        contextual_grade: None,
                        bounds: proposal.symbol_bounds,
                        glyph_id: Some(proposal.glyph_id),
                        glyph_bounds: Some(proposal.glyph_bounds),
                        in_sig: false,
                        staff_id: None,
                        original_glyph_registered: false,
                        removed: false,
                    },
                );
            }
        }
        Ok(best)
    }
}

fn clef_kind(shape: NeutralClefShape, pitch: i32) -> Option<NeutralClefKind> {
    match shape {
        NeutralClefShape::Treble
        | NeutralClefShape::TrebleOttavaAlta
        | NeutralClefShape::TrebleOttavaBassa => Some(NeutralClefKind::Treble),
        NeutralClefShape::Bass => match pitch {
            -2 => Some(NeutralClefKind::Bass),
            0 => Some(NeutralClefKind::Baritone),
            _ => None,
        },
        NeutralClefShape::C => match pitch {
            -2 => Some(NeutralClefKind::Tenor),
            0 => Some(NeutralClefKind::Alto),
            2 => Some(NeutralClefKind::MezzoSoprano),
            4 => Some(NeutralClefKind::Soprano),
            _ => None,
        },
        NeutralClefShape::Percussion => Some(NeutralClefKind::Percussion),
    }
}

fn purge_clef_candidates(
    best: &mut BTreeMap<NeutralClefKind, NeutralClefCandidate>,
    maximum_key_contribution: f64,
) {
    if best.len() <= 1 {
        return;
    }
    let mut ordered = best.values().cloned().collect::<Vec<_>>();
    ordered.sort_by(|one, two| two.grade.total_cmp(&one.grade));
    for one in 0..ordered.len() {
        for two in (one + 1)..ordered.len() {
            let maximum_other = contextual_grade(ordered[two].grade, maximum_key_contribution);
            if ordered[one].grade > maximum_other {
                for poor in &ordered[two..] {
                    best.remove(&poor.kind);
                }
                return;
            }
        }
    }
}

fn contextual_grade(intrinsic: f64, contribution: f64) -> f64 {
    1.0 - ((1.0 - intrinsic) * (1.0 - contribution))
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralClefCandidate {
    pub id: usize,
    pub kind: NeutralClefKind,
    pub grade: f64,
    /// Set by later key-support work before `selectClefs`, if available.
    pub contextual_grade: Option<f64>,
    pub bounds: HeaderBounds,
    pub glyph_id: Option<usize>,
    pub glyph_bounds: Option<HeaderBounds>,
    /// Artificial clefs found in the lookup area are already SIG vertices.
    pub in_sig: bool,
    pub staff_id: Option<usize>,
    pub original_glyph_registered: bool,
    pub removed: bool,
}

impl NeutralClefCandidate {
    #[must_use]
    pub fn best_grade(&self) -> f64 {
        self.contextual_grade.unwrap_or(self.grade)
    }
}

/// First visual dependency corresponding to per-staff `ClefBuilder.findClefs`.
/// Returned candidates are the already classified/purged best-per-kind set.
pub trait VisualClefRecognizer {
    type Error;

    fn find_clefs(
        &mut self,
        system_id: usize,
        staff_id: usize,
        range: &StaffHeaderRange,
    ) -> Result<Vec<NeutralClefCandidate>, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClefColumnError<VisualError> {
    MissingHeader {
        staff_id: usize,
    },
    DuplicateInterId {
        staff_id: usize,
        inter_id: usize,
    },
    Visual {
        staff_id: usize,
        source: VisualError,
    },
}

impl<VisualError: fmt::Display> fmt::Display for ClefColumnError<VisualError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { staff_id } => {
                write!(
                    formatter,
                    "staff {staff_id} has no header for clef retrieval"
                )
            }
            Self::DuplicateInterId { staff_id, inter_id } => write!(
                formatter,
                "staff {staff_id} clef candidate duplicates live SIG inter {inter_id}"
            ),
            Self::Visual { staff_id, source } => {
                write!(
                    formatter,
                    "staff {staff_id} visual clef recognition failed: {source}"
                )
            }
        }
    }
}

impl<VisualError: Error + 'static> Error for ClefColumnError<VisualError> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Visual { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeutralClefBuilderState {
    pub staff_id: usize,
    pub candidates: Vec<NeutralClefCandidate>,
}

pub struct HeadlessClefColumn<Visual> {
    visual: Visual,
    /// Java `TreeMap<Staff, ClefBuilder>(Staff.byId)`.
    builders: BTreeMap<usize, NeutralClefBuilderState>,
}

impl<Visual> HeadlessClefColumn<Visual> {
    #[must_use]
    pub const fn new(visual: Visual) -> Self {
        Self {
            visual,
            builders: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn visual(&self) -> &Visual {
        &self.visual
    }

    #[must_use]
    pub fn builders(&self) -> &BTreeMap<usize, NeutralClefBuilderState> {
        &self.builders
    }
}

impl<Visual> HeadlessClefColumn<Visual>
where
    Visual: VisualClefRecognizer,
{
    /// Java `Column.retrieveClefs`, retaining every completed staff prefix.
    pub fn retrieve_clefs(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<i32, ClefColumnError<Visual::Error>> {
        let mut maximum_offset = 0;
        for staff_index in 0..system.staffs.len() {
            if system.staffs[staff_index].tablature {
                continue;
            }
            let staff_id = system.staffs[staff_index].id;
            let measure_start = system.staffs[staff_index]
                .header
                .as_ref()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?
                .start;
            let maximum_end = system.staffs[staff_index].maximum_clef_end;
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .expect("header existence was checked");
            let range = header
                .clef_range
                .get_or_insert_with(StaffHeaderRange::default);
            range.browse_start = measure_start;
            range.browse_stop = measure_start.wrapping_add(maximum_end);

            // Java inserts the builder before `findClefs`; a visual failure
            // must retain this empty builder and initialized range.
            self.builders.insert(
                staff_id,
                NeutralClefBuilderState {
                    staff_id,
                    candidates: Vec::new(),
                },
            );
            let candidates = self
                .visual
                .find_clefs(system.id, staff_id, range)
                .map_err(|source| ClefColumnError::Visual { staff_id, source })?;
            register_candidates(system, staff_index, candidates, &mut self.builders)?;

            let precise_stop = system.staffs[staff_index]
                .header
                .as_ref()
                .and_then(|header| header.clef_range.as_ref())
                .and_then(StaffHeaderRange::precise_stop);
            if let Some(stop) = precise_stop {
                maximum_offset = maximum_offset.max(stop.wrapping_sub(measure_start));
            }
        }
        Ok(maximum_offset)
    }

    /// Java `Column.selectClefs`, traversing builder values by staff ID.
    pub fn select_clefs(
        &mut self,
        system: &mut HeadlessHeaderSystem,
    ) -> Result<(), ClefColumnError<Visual::Error>> {
        let staff_ids = self.builders.keys().copied().collect::<Vec<_>>();
        for staff_id in staff_ids {
            let staff_index = system
                .staffs
                .iter()
                .position(|staff| staff.id == staff_id)
                .ok_or(ClefColumnError::MissingHeader { staff_id })?;
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?;
            let range_stop = header
                .clef_range
                .as_ref()
                .ok_or(ClefColumnError::MissingHeader { staff_id })?
                .stop();
            let builder = self.builders.get_mut(&staff_id).expect("key came from map");
            // Java first takes the last clef whose abscissa precedes the
            // range stop, then includes every same-staff exclusion peer.
            let Some(anchor) = builder
                .candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| !candidate.removed && candidate.bounds.x < range_stop)
                .max_by_key(|(index, candidate)| (candidate.bounds.x, *index))
                .map(|(index, _)| index)
            else {
                continue;
            };
            let anchor_id = builder.candidates[anchor].id;
            let mut active = builder
                .candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    !candidate.removed
                        && (candidate.id == anchor_id
                            || system.sig_exclusions.iter().any(|exclusion| {
                                (exclusion.one == anchor_id && exclusion.two == candidate.id)
                                    || (exclusion.two == anchor_id && exclusion.one == candidate.id)
                            }))
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            active.sort_by(|one, two| {
                builder.candidates[*two]
                    .best_grade()
                    .total_cmp(&builder.candidates[*one].best_grade())
            });
            let winner = active[0];
            if builder.candidates[winner].glyph_id.is_some() {
                builder.candidates[winner].original_glyph_registered = true;
            }
            let selected = &builder.candidates[winner];
            header.clef = Some(HeaderComponent::new(selected.id, selected.bounds));

            for &loser in &active[1..] {
                let id = builder.candidates[loser].id;
                builder.candidates[loser].removed = true;
                system.sig_vertex_ids.retain(|candidate| *candidate != id);
                system
                    .sig_exclusions
                    .retain(|exclusion| exclusion.one != id && exclusion.two != id);
            }
        }
        Ok(())
    }
}

fn register_candidates<VisualError>(
    system: &mut HeadlessHeaderSystem,
    staff_index: usize,
    mut candidates: Vec<NeutralClefCandidate>,
    builders: &mut BTreeMap<usize, NeutralClefBuilderState>,
) -> Result<(), ClefColumnError<VisualError>> {
    let staff_id = system.staffs[staff_index].id;
    candidates.sort_by(|one, two| two.grade.total_cmp(&one.grade));
    for (index, candidate) in candidates.iter_mut().enumerate() {
        if candidate.glyph_id.is_some() && !candidate.in_sig {
            if system.sig_vertex_ids.contains(&candidate.id) {
                return Err(ClefColumnError::DuplicateInterId {
                    staff_id,
                    inter_id: candidate.id,
                });
            }
            system.sig_vertex_ids.push(candidate.id);
            candidate.in_sig = true;
        }
        candidate.staff_id = Some(staff_id);
        if index == 0 {
            let stop_bounds = candidate.glyph_bounds.map_or(candidate.bounds, |glyph| {
                intersection(glyph, candidate.bounds)
            });
            let header = system.staffs[staff_index]
                .header
                .as_mut()
                .expect("caller checked header");
            let range = header
                .clef_range
                .as_mut()
                .expect("caller initialized clef range");
            range.set_stop(stop_bounds.right());
            range.valid = true;
        }
    }
    for one in 0..candidates.len() {
        for two in (one + 1)..candidates.len() {
            system.sig_exclusions.push(HeaderSigExclusion {
                one: candidates[one].id,
                two: candidates[two].id,
            });
        }
    }
    builders
        .get_mut(&staff_id)
        .expect("builder inserted before recognition")
        .candidates = candidates;
    Ok(())
}

fn intersection(one: HeaderBounds, two: HeaderBounds) -> HeaderBounds {
    let x = one.x.max(two.x);
    let y = one.y.max(two.y);
    let right = one.right().min(two.right());
    let bottom = one
        .y
        .wrapping_add(one.height)
        .wrapping_sub(1)
        .min(two.y.wrapping_add(two.height).wrapping_sub(1));
    HeaderBounds {
        x,
        y,
        width: right.wrapping_sub(x).wrapping_add(1),
        height: bottom.wrapping_sub(y).wrapping_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{headers_step::HeadlessHeaderStaff, staff_header::StaffHeader};
    use std::convert::Infallible;

    #[derive(Default)]
    struct RecordingClassifier {
        calls: Vec<(usize, Vec<usize>, HeaderBounds, usize)>,
        fail_on_call: Option<usize>,
    }

    impl ClefShapeClassifier for RecordingClassifier {
        type Error = &'static str;

        fn evaluate(
            &mut self,
            glyph: &NativeClefGlyph,
            _staff_interline: i32,
            _maximum_rank: usize,
            _minimum_grade: f64,
        ) -> Result<Vec<ClefShapeEvaluation>, Self::Error> {
            self.calls
                .push((glyph.id, glyph.part_ids.clone(), glyph.bounds, glyph.weight));
            if self.fail_on_call == Some(self.calls.len()) {
                return Err("classifier");
            }
            Ok(vec![ClefShapeEvaluation {
                shape: NeutralClefShape::Treble,
                grade: 0.75,
                symbol_bounds: glyph.bounds,
            }])
        }
    }

    fn native_parameters() -> NativeClefParameters {
        NativeClefParameters {
            staff_interline: 4,
            first_line_y: 4.0,
            last_line_y: 12.0,
            min_part_weight: 1,
            max_part_count: 8,
            max_part_gap: 2.0,
            max_glyph_height: 20.0,
            min_glyph_weight: 1,
            max_eval_rank: 3,
            minimum_classifier_grade: 0.1,
            f_area_pitch_offset: 0.0,
        }
    }

    fn native_context() -> ClefLookupContext {
        ClefLookupContext {
            outer: ClefLookupRect {
                x: 0,
                y: 0,
                width: 12,
                height: 16,
            },
            inner: ClefLookupRect {
                x: 1,
                y: 2,
                width: 10,
                height: 12,
            },
            percussion_only: false,
        }
    }

    fn native_recognizer(
        black: &[(usize, usize)],
        classifier: RecordingClassifier,
        parameters: NativeClefParameters,
    ) -> NativeClefProposalRecognizer<RecordingClassifier> {
        let mut pixels = vec![BACKGROUND; 12 * 16];
        for &(x, y) in black {
            pixels[(y * 12) + x] = FOREGROUND;
        }
        let source = RunTable::from_pixels(Orientation::Horizontal, 12, 16, &pixels).unwrap();
        NativeClefProposalRecognizer::new(
            classifier,
            BTreeMap::from([(7, source)]),
            BTreeMap::from([(1, native_context())]),
            BTreeMap::from([(1, parameters)]),
            10,
            20,
        )
    }

    #[derive(Default)]
    struct FakeProposalVisual {
        existing: Option<NeutralClefCandidate>,
        passes: Vec<Vec<ClefClassifierProposal>>,
        calls: Vec<ClefRecognitionPass>,
    }

    impl VisualClefProposalRecognizer for FakeProposalVisual {
        type Error = Infallible;

        fn existing_clef(
            &mut self,
            _system_id: usize,
            _staff_id: usize,
            _outer: ClefLookupRect,
        ) -> Result<Option<NeutralClefCandidate>, Self::Error> {
            Ok(self.existing.take())
        }

        fn classify_clefs(
            &mut self,
            _system_id: usize,
            _staff_id: usize,
            pass: ClefRecognitionPass,
            _lookup: ClefLookupRect,
        ) -> Result<Vec<ClefClassifierProposal>, Self::Error> {
            self.calls.push(pass);
            Ok(self.passes.remove(0))
        }
    }

    #[derive(Default)]
    struct FakeVisual {
        by_staff: BTreeMap<usize, Result<Vec<NeutralClefCandidate>, &'static str>>,
        calls: Vec<usize>,
    }

    impl VisualClefRecognizer for FakeVisual {
        type Error = &'static str;

        fn find_clefs(
            &mut self,
            _system_id: usize,
            staff_id: usize,
            _range: &StaffHeaderRange,
        ) -> Result<Vec<NeutralClefCandidate>, Self::Error> {
            self.calls.push(staff_id);
            self.by_staff.remove(&staff_id).unwrap_or(Ok(Vec::new()))
        }
    }

    fn bounds(x: i32, y: i32, width: i32, height: i32) -> HeaderBounds {
        HeaderBounds {
            x,
            y,
            width,
            height,
        }
    }

    fn candidate(id: usize, grade: f64, x: i32) -> NeutralClefCandidate {
        NeutralClefCandidate {
            id,
            kind: NeutralClefKind::Treble,
            grade,
            contextual_grade: None,
            bounds: bounds(x, 4, 8, 14),
            glyph_id: Some(id + 100),
            glyph_bounds: None,
            in_sig: false,
            staff_id: None,
            original_glyph_registered: false,
            removed: false,
        }
    }

    fn staff(id: usize, start: i32, maximum_clef_end: i32) -> HeadlessHeaderStaff {
        let mut staff = HeadlessHeaderStaff::new(id);
        staff.maximum_clef_end = maximum_clef_end;
        staff.header = Some(StaffHeader::new(start));
        staff
    }

    #[test]
    fn retrieve_preserves_source_order_skips_tablature_and_registers_by_grade() {
        let mut tablature = staff(2, 20, 9);
        tablature.tablature = true;
        let mut system =
            HeadlessHeaderSystem::new(7, vec![staff(5, 10, 12), tablature, staff(3, 30, 10)]);
        let mut visual = FakeVisual::default();
        let mut low = candidate(51, 0.2, 13);
        low.glyph_bounds = Some(bounds(14, 4, 3, 14));
        visual
            .by_staff
            .insert(5, Ok(vec![low, candidate(50, 0.9, 11)]));
        visual.by_staff.insert(3, Ok(vec![candidate(30, 0.8, 31)]));
        let mut column = HeadlessClefColumn::new(visual);

        assert_eq!(column.retrieve_clefs(&mut system), Ok(8));
        assert_eq!(column.visual().calls, vec![5, 3]);
        assert_eq!(
            column.builders().keys().copied().collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert_eq!(system.sig_vertex_ids, vec![50, 51, 30]);
        assert_eq!(
            system.sig_exclusions,
            vec![HeaderSigExclusion { one: 50, two: 51 }]
        );
        assert!(
            system.staffs[1]
                .header
                .as_ref()
                .unwrap()
                .clef_range
                .is_none()
        );
        let five = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef_range
            .as_ref()
            .unwrap();
        assert_eq!(
            (five.browse_start, five.browse_stop, five.precise_stop()),
            (10, 22, Some(18))
        );
        assert_eq!(column.builders()[&5].candidates[0].staff_id, Some(5));
    }

    #[test]
    fn visual_failure_retains_initialized_range_and_empty_builder() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 12)]);
        let mut visual = FakeVisual::default();
        visual.by_staff.insert(1, Err("classifier unavailable"));
        let mut column = HeadlessClefColumn::new(visual);

        assert_eq!(
            column.retrieve_clefs(&mut system),
            Err(ClefColumnError::Visual {
                staff_id: 1,
                source: "classifier unavailable"
            })
        );
        assert!(column.builders()[&1].candidates.is_empty());
        let range = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef_range
            .as_ref()
            .unwrap();
        assert_eq!((range.browse_start, range.browse_stop), (10, 22));
    }

    #[test]
    fn selection_uses_contextual_grade_and_exclusion_peers_beyond_stop() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(5, 10, 12)]);
        let mut visual = FakeVisual::default();
        let first = candidate(50, 0.9, 11);
        let mut peer = candidate(51, 0.2, 30);
        peer.contextual_grade = Some(1.1);
        visual.by_staff.insert(5, Ok(vec![first, peer]));
        let mut column = HeadlessClefColumn::new(visual);
        column.retrieve_clefs(&mut system).unwrap();

        column.select_clefs(&mut system).unwrap();
        let selected = system.staffs[0]
            .header
            .as_ref()
            .unwrap()
            .clef
            .as_ref()
            .unwrap();
        assert_eq!(selected.id, 51);
        assert_eq!(system.sig_vertex_ids, vec![51]);
        assert!(column.builders()[&5].candidates[1].original_glyph_registered);
        assert!(column.builders()[&5].candidates[0].removed);
        assert!(system.sig_exclusions.is_empty());
    }

    #[test]
    fn empty_visual_result_keeps_range_invalid_and_returns_zero_offset() {
        let mut system = HeadlessHeaderSystem::new(7, vec![staff(1, 10, 12)]);
        let mut column = HeadlessClefColumn::new(FakeVisual::default());
        assert_eq!(column.retrieve_clefs(&mut system), Ok(0));
        assert!(
            !system.staffs[0]
                .header
                .as_ref()
                .unwrap()
                .clef_range
                .as_ref()
                .unwrap()
                .valid
        );
    }

    fn _assert_infallible_is_error(value: ClefColumnError<Infallible>) -> impl Error {
        value
    }

    fn proposal(
        id: usize,
        shape: NeutralClefShape,
        pitch: i32,
        grade: f64,
    ) -> ClefClassifierProposal {
        ClefClassifierProposal {
            id,
            glyph_id: id + 100,
            shape,
            classifier_grade: grade,
            reference_pitch: pitch,
            symbol_bounds: bounds(12, 4, 8, 14),
            glyph_bounds: bounds(13, 4, 6, 14),
        }
    }

    fn context(percussion_only: bool) -> BTreeMap<usize, ClefLookupContext> {
        BTreeMap::from([(
            1,
            ClefLookupContext {
                outer: ClefLookupRect {
                    x: 10,
                    y: 20,
                    width: 30,
                    height: 50,
                },
                inner: ClefLookupRect {
                    x: 12,
                    y: 25,
                    width: 28,
                    height: 40,
                },
                percussion_only,
            },
        )])
    }

    #[test]
    fn lookup_geometry_clamps_to_neighbor_gutters_and_builds_asymmetric_inner() {
        let contexts = build_clef_lookup_contexts(
            &[
                ClefLookupStaffGeometry {
                    staff_id: 1,
                    browse_start: 10,
                    browse_stop: 40,
                    left_abscissa: 0,
                    right_abscissa: 100,
                    first_line_y: 20,
                    last_line_y: 40,
                    percussion_only: false,
                },
                ClefLookupStaffGeometry {
                    staff_id: 2,
                    browse_start: 10,
                    browse_stop: 40,
                    left_abscissa: 0,
                    right_abscissa: 100,
                    first_line_y: 60,
                    last_line_y: 80,
                    percussion_only: false,
                },
            ],
            ClefLookupParameters {
                sheet_height: 100,
                above_staff: 30,
                below_staff: 32,
                belt_margin: 2,
                x_core_margin: 3,
                y_core_margin: 4,
            },
        );
        assert_eq!(
            contexts[&1].outer,
            ClefLookupRect {
                x: 12,
                y: 0,
                width: 27,
                height: 50
            }
        );
        assert_eq!(
            contexts[&1].inner,
            ClefLookupRect {
                x: 15,
                y: 4,
                width: 24,
                height: 42
            }
        );
        assert_eq!(contexts[&2].outer.y, 51);
    }

    #[test]
    fn artificial_clef_short_circuits_both_classifier_passes() {
        let visual = FakeProposalVisual {
            existing: Some(candidate(9, 0.3, 12)),
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(false), 0.8, 0.2);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 9);
        assert!(lifecycle.visual().calls.is_empty());
    }

    #[test]
    fn lifecycle_retries_inner_maps_kinds_deduplicates_and_purges_unbeatable_tail() {
        let visual = FakeProposalVisual {
            passes: vec![
                Vec::new(),
                vec![
                    proposal(1, NeutralClefShape::C, -2, 0.9),
                    proposal(2, NeutralClefShape::C, -2, 0.7),
                    proposal(3, NeutralClefShape::Bass, -2, 0.2),
                    proposal(4, NeutralClefShape::Bass, 7, 1.0),
                ],
            ],
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(false), 0.8, 0.1);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(
            lifecycle.visual().calls,
            vec![
                ClefRecognitionPass::OuterAndInner,
                ClefRecognitionPass::InnerOnly
            ]
        );
        assert_eq!(result.len(), 1);
        assert_eq!((result[0].id, result[0].kind), (1, NeutralClefKind::Tenor));
        assert!((result[0].grade - 0.72).abs() < f64::EPSILON);
    }

    #[test]
    fn percussion_context_rejects_pitched_shapes_before_kind_competition() {
        let visual = FakeProposalVisual {
            passes: vec![vec![
                proposal(1, NeutralClefShape::Treble, 2, 0.9),
                proposal(2, NeutralClefShape::Percussion, 0, 0.5),
            ]],
            ..FakeProposalVisual::default()
        };
        let mut lifecycle = ClefLifecycleRecognizer::new(visual, context(true), 1.0, 0.0);
        let result = lifecycle
            .find_clefs(7, 1, &StaffHeaderRange::default())
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, NeutralClefKind::Percussion);
    }

    #[test]
    fn native_lookup_builds_vertical_parts_and_connected_subsets_in_java_order() {
        let mut native = native_recognizer(
            &[(2, 6), (4, 6)],
            RecordingClassifier::default(),
            native_parameters(),
        );
        let proposals = native
            .classify_clefs(
                7,
                1,
                ClefRecognitionPass::OuterAndInner,
                native_context().outer,
            )
            .unwrap();
        assert_eq!(
            native.classifier().calls,
            vec![
                (11, vec![11], bounds(2, 6, 1, 1), 1),
                (13, vec![11, 12], bounds(2, 6, 3, 1), 2),
                (12, vec![12], bounds(4, 6, 1, 1), 1),
            ]
        );
        assert_eq!(
            native.mutations(),
            &[
                NativeClefMutation::PartRegistered {
                    staff_id: 1,
                    glyph_id: 11
                },
                NativeClefMutation::PartRegistered {
                    staff_id: 1,
                    glyph_id: 12
                },
                NativeClefMutation::GlyphEvaluated {
                    staff_id: 1,
                    glyph_id: 11
                },
                NativeClefMutation::CompoundRegistered {
                    staff_id: 1,
                    glyph_id: 13
                },
                NativeClefMutation::GlyphEvaluated {
                    staff_id: 1,
                    glyph_id: 13
                },
                NativeClefMutation::GlyphEvaluated {
                    staff_id: 1,
                    glyph_id: 12
                },
            ]
        );
        assert_eq!(
            proposals
                .iter()
                .map(|proposal| proposal.id)
                .collect::<Vec<_>>(),
            vec![21, 22, 23]
        );
    }

    #[test]
    fn native_first_pass_purges_parts_outside_inner_and_keeps_registered_order() {
        let mut parameters = native_parameters();
        parameters.min_part_weight = 2;
        let mut native = native_recognizer(
            &[(0, 0), (0, 1), (3, 5), (3, 6), (8, 7)],
            RecordingClassifier::default(),
            parameters,
        );
        native
            .classify_clefs(
                7,
                1,
                ClefRecognitionPass::OuterAndInner,
                native_context().outer,
            )
            .unwrap();
        assert_eq!(native.classifier().calls.len(), 1);
        assert_eq!(native.classifier().calls[0].2, bounds(3, 5, 1, 2));
        assert_eq!(
            native.mutations()[0],
            NativeClefMutation::PartRegistered {
                staff_id: 1,
                glyph_id: 11
            }
        );
    }

    #[test]
    fn native_part_cap_uses_stable_reverse_weight_before_registration() {
        let mut parameters = native_parameters();
        parameters.max_part_count = 2;
        parameters.max_part_gap = 0.0;
        let mut native = native_recognizer(
            &[(2, 5), (5, 5), (5, 6), (8, 5), (8, 6), (8, 7)],
            RecordingClassifier::default(),
            parameters,
        );
        native
            .classify_clefs(
                7,
                1,
                ClefRecognitionPass::OuterAndInner,
                native_context().outer,
            )
            .unwrap();
        assert_eq!(native.classifier().calls.len(), 2);
        assert_eq!(native.classifier().calls[0].2, bounds(5, 5, 1, 2));
        assert_eq!(native.classifier().calls[1].2, bounds(8, 5, 1, 3));
    }

    #[test]
    fn classifier_failure_retains_registered_and_evaluated_prefix() {
        let classifier = RecordingClassifier {
            fail_on_call: Some(2),
            ..RecordingClassifier::default()
        };
        let mut native = native_recognizer(&[(2, 6), (4, 6)], classifier, native_parameters());
        assert!(matches!(
            native.classify_clefs(
                7,
                1,
                ClefRecognitionPass::OuterAndInner,
                native_context().outer,
            ),
            Err(NativeClefError::Classifier("classifier"))
        ));
        assert_eq!(native.classifier().calls.len(), 2);
        assert_eq!(
            native.mutations().last(),
            Some(&NativeClefMutation::GlyphEvaluated {
                staff_id: 1,
                glyph_id: 13
            })
        );
    }
}
