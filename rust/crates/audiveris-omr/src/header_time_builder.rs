// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete raster front-end for Java `HeaderTimeBuilder`.
//!
//! Projection browsing, vertical-run component extraction, deterministic
//! compound enumeration and whole/number proposal selection are native. The
//! trained `ShapeClassifier` remains the first injected dependency.

use std::{collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    glyph_factory::{GlyphComponent, build_glyph_components},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

use crate::{
    header_time_column::{
        HeaderTimeClassifierProposals, HeaderTimeRecognitionInput, NeutralTimeValue,
        TimeNumberClassifierProposal, TimeWholeClassifierProposal,
        VisualHeaderTimeProposalRecognizer,
    },
    staff_header::{HeaderBounds, StaffHeaderRange},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderTimeGlyphKind {
    Whole,
    Numerator,
    Denominator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderTimeGlyph {
    pub id: usize,
    pub part_ids: Vec<usize>,
    pub bounds: HeaderBounds,
    pub weight: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub raster: RunTable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeaderTimeShapeEvaluation {
    Whole {
        value: NeutralTimeValue,
        grade: f64,
        symbol_bounds: HeaderBounds,
    },
    Number {
        value: i32,
        grade: f64,
        symbol_bounds: HeaderBounds,
    },
    Other {
        grade: f64,
    },
}

pub trait HeaderTimeShapeClassifier {
    type Error;

    fn evaluate(
        &mut self,
        glyph: &HeaderTimeGlyph,
        kind: HeaderTimeGlyphKind,
        staff_interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<HeaderTimeShapeEvaluation>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHeaderTimeParameters {
    pub staff_interline: i32,
    pub maximum_first_space_width: i32,
    pub maximum_space_cumul: usize,
    pub minimum_time_width: i32,
    pub vertical_margin: i32,
    pub minimum_part_weight: usize,
    pub maximum_part_gap: f64,
    pub maximum_time_width: i32,
    pub minimum_whole_weight: usize,
    pub minimum_half_weight: usize,
    pub maximum_eval_rank: usize,
    pub minimum_classifier_grade: f64,
    pub intrinsic_ratio: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderTimeRasterContext {
    pub roi: HeaderBounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHeaderTimeMutation {
    RangeBrowsed {
        staff_id: usize,
        start: i32,
        stop: i32,
    },
    PartRegistered {
        staff_id: usize,
        kind: HeaderTimeGlyphKind,
        glyph_id: usize,
    },
    CompoundRegistered {
        staff_id: usize,
        kind: HeaderTimeGlyphKind,
        glyph_id: usize,
    },
    GlyphEvaluated {
        staff_id: usize,
        kind: HeaderTimeGlyphKind,
        glyph_id: usize,
    },
    InterAllocated {
        staff_id: usize,
        inter_id: usize,
        glyph_id: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHeaderTimeError<ClassifierError> {
    MissingSource(usize),
    MissingContext(usize),
    MissingParameters(usize),
    InvalidRoi(HeaderBounds),
    GlyphIdExhausted,
    InterIdExhausted,
    RunTable(RunTableError),
    Classifier(ClassifierError),
}

impl<E: fmt::Display> fmt::Display for NativeHeaderTimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(id) => write!(formatter, "missing no-staff raster for system {id}"),
            Self::MissingContext(id) => {
                write!(formatter, "missing time raster context for staff {id}")
            }
            Self::MissingParameters(id) => {
                write!(formatter, "missing time parameters for staff {id}")
            }
            Self::InvalidRoi(roi) => write!(formatter, "invalid time ROI {roi:?}"),
            Self::GlyphIdExhausted => formatter.write_str("time glyph ID exhausted"),
            Self::InterIdExhausted => formatter.write_str("time inter ID exhausted"),
            Self::RunTable(source) => write!(formatter, "time run table failed: {source}"),
            Self::Classifier(source) => write!(formatter, "time classifier failed: {source}"),
        }
    }
}

impl<E: Error + 'static> Error for NativeHeaderTimeError<E> {}

pub struct NativeHeaderTimeRecognizer<Classifier> {
    classifier: Classifier,
    sources: BTreeMap<usize, RunTable>,
    contexts: BTreeMap<usize, HeaderTimeRasterContext>,
    parameters: BTreeMap<usize, NativeHeaderTimeParameters>,
    next_glyph_id: usize,
    next_inter_id: usize,
    mutations: Vec<NativeHeaderTimeMutation>,
}

impl<Classifier> NativeHeaderTimeRecognizer<Classifier> {
    #[must_use]
    pub fn new(
        classifier: Classifier,
        sources: BTreeMap<usize, RunTable>,
        contexts: BTreeMap<usize, HeaderTimeRasterContext>,
        parameters: BTreeMap<usize, NativeHeaderTimeParameters>,
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
    pub fn mutations(&self) -> &[NativeHeaderTimeMutation] {
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

type RegionProposals = (
    Vec<TimeWholeClassifierProposal>,
    Vec<TimeNumberClassifierProposal>,
);

impl<C: HeaderTimeShapeClassifier> VisualHeaderTimeProposalRecognizer
    for NativeHeaderTimeRecognizer<C>
{
    type Error = NativeHeaderTimeError<C::Error>;

    fn classify_time_parts(
        &mut self,
        input: HeaderTimeRecognitionInput,
        incoming: &StaffHeaderRange,
    ) -> Result<HeaderTimeClassifierProposals, Self::Error> {
        let source = self
            .sources
            .get(&input.system_id)
            .ok_or(Self::Error::MissingSource(input.system_id))?
            .clone();
        let context = *self
            .contexts
            .get(&input.staff_id)
            .ok_or(Self::Error::MissingContext(input.staff_id))?;
        let parameters = *self
            .parameters
            .get(&input.staff_id)
            .ok_or(Self::Error::MissingParameters(input.staff_id))?;
        validate_rect(&source, context.roi).map_err(|_| Self::Error::InvalidRoi(context.roi))?;

        let mut range = incoming.clone();
        if range.browse_start == 0 {
            range.browse_start = input.browse_start;
        }
        range.browse_stop = context.roi.right();
        if !range.has_start() {
            browse_projection(&source, context.roi, parameters, &mut range);
        }
        let mut proposals = HeaderTimeClassifierProposals {
            range: range.clone(),
            wholes: Vec::new(),
            numerators: Vec::new(),
            denominators: Vec::new(),
        };
        if !range.has_start() || range.width().unwrap_or(0) < parameters.minimum_time_width {
            return Ok(proposals);
        }
        self.mutations.push(NativeHeaderTimeMutation::RangeBrowsed {
            staff_id: input.staff_id,
            start: range.start().unwrap_or_default(),
            stop: range.stop(),
        });

        // Java HeaderTimeBuilder.findCandidates order is semantically visible.
        let whole_rect = search_rect(
            &range,
            context.roi,
            HeaderTimeGlyphKind::Whole,
            parameters.vertical_margin,
        );
        proposals.wholes = self
            .evaluate_region(
                &source,
                input.staff_id,
                whole_rect,
                HeaderTimeGlyphKind::Whole,
                parameters,
            )?
            .0;
        let num_rect = search_rect(
            &range,
            context.roi,
            HeaderTimeGlyphKind::Numerator,
            parameters.vertical_margin,
        );
        proposals.numerators = self
            .evaluate_region(
                &source,
                input.staff_id,
                num_rect,
                HeaderTimeGlyphKind::Numerator,
                parameters,
            )?
            .1;
        let den_rect = search_rect(
            &range,
            context.roi,
            HeaderTimeGlyphKind::Denominator,
            parameters.vertical_margin,
        );
        proposals.denominators = self
            .evaluate_region(
                &source,
                input.staff_id,
                den_rect,
                HeaderTimeGlyphKind::Denominator,
                parameters,
            )?
            .1;
        Ok(proposals)
    }
}

impl<C: HeaderTimeShapeClassifier> NativeHeaderTimeRecognizer<C> {
    fn evaluate_region(
        &mut self,
        source: &RunTable,
        staff_id: usize,
        rect: HeaderBounds,
        kind: HeaderTimeGlyphKind,
        parameters: NativeHeaderTimeParameters,
    ) -> Result<RegionProposals, NativeHeaderTimeError<C::Error>> {
        let table = crop_vertical(source, rect).map_err(NativeHeaderTimeError::RunTable)?;
        let components = build_glyph_components(&table, rect.x, rect.y)
            .into_iter()
            .filter(|part| part.weight >= parameters.minimum_part_weight)
            .collect::<Vec<_>>();
        let mut parts = Vec::with_capacity(components.len());
        for component in components {
            let id = self.allocate_glyph_id()?;
            self.mutations
                .push(NativeHeaderTimeMutation::PartRegistered {
                    staff_id,
                    kind,
                    glyph_id: id,
                });
            parts.push(RegisteredPart {
                id,
                pixels: component_pixels(&component),
                component,
            });
        }
        let graph = near_graph(&parts, parameters.maximum_part_gap);
        let mut subsets = Vec::new();
        enumerate_connected_subsets(&graph, &mut subsets);
        let mut best_wholes = BTreeMap::<
            (
                Option<crate::header_time_column::NeutralSpecificTimeShape>,
                i32,
                i32,
            ),
            TimeWholeClassifierProposal,
        >::new();
        let mut best_numbers = BTreeMap::<i32, TimeNumberClassifierProposal>::new();
        for subset in subsets {
            let bounds = union_bounds(&parts, &subset);
            if bounds.width > parameters.maximum_time_width {
                continue;
            }
            let weight = subset
                .iter()
                .map(|&index| parts[index].component.weight)
                .sum::<usize>();
            let minimum_weight = if kind == HeaderTimeGlyphKind::Whole {
                parameters.minimum_whole_weight
            } else {
                parameters.minimum_half_weight
            };
            if weight < minimum_weight {
                continue;
            }
            let glyph_id = if subset.len() == 1 {
                parts[subset[0]].id
            } else {
                let id = self.allocate_glyph_id()?;
                self.mutations
                    .push(NativeHeaderTimeMutation::CompoundRegistered {
                        staff_id,
                        kind,
                        glyph_id: id,
                    });
                id
            };
            let glyph = compound_glyph(glyph_id, &parts, &subset, bounds)
                .map_err(NativeHeaderTimeError::RunTable)?;
            self.mutations
                .push(NativeHeaderTimeMutation::GlyphEvaluated {
                    staff_id,
                    kind,
                    glyph_id,
                });
            let evaluations = self
                .classifier
                .evaluate(
                    &glyph,
                    kind,
                    parameters.staff_interline,
                    parameters.maximum_eval_rank,
                    parameters.minimum_classifier_grade,
                )
                .map_err(NativeHeaderTimeError::Classifier)?;
            for evaluation in evaluations.into_iter().take(parameters.maximum_eval_rank) {
                match evaluation {
                    HeaderTimeShapeEvaluation::Whole {
                        value,
                        grade,
                        symbol_bounds,
                    } if kind == HeaderTimeGlyphKind::Whole => {
                        if grade < parameters.minimum_classifier_grade {
                            continue;
                        }
                        let grade = grade * parameters.intrinsic_ratio;
                        let key = (value.specific_shape, value.numerator, value.denominator);
                        if best_wholes.get(&key).is_none_or(|old| old.grade < grade) {
                            let id = self.allocate_inter_id()?;
                            self.mutations
                                .push(NativeHeaderTimeMutation::InterAllocated {
                                    staff_id,
                                    inter_id: id,
                                    glyph_id,
                                });
                            best_wholes.insert(
                                key,
                                TimeWholeClassifierProposal {
                                    id,
                                    value,
                                    grade,
                                    symbol_bounds,
                                },
                            );
                        }
                    }
                    HeaderTimeShapeEvaluation::Number {
                        value,
                        grade,
                        symbol_bounds,
                    } if kind != HeaderTimeGlyphKind::Whole => {
                        if grade < parameters.minimum_classifier_grade {
                            continue;
                        }
                        let grade = grade * parameters.intrinsic_ratio;
                        if best_numbers.get(&value).is_none_or(|old| old.grade < grade) {
                            let id = self.allocate_inter_id()?;
                            self.mutations
                                .push(NativeHeaderTimeMutation::InterAllocated {
                                    staff_id,
                                    inter_id: id,
                                    glyph_id,
                                });
                            best_numbers.insert(
                                value,
                                TimeNumberClassifierProposal {
                                    id,
                                    value,
                                    grade,
                                    bounds: symbol_bounds,
                                    center_x: glyph.centroid_x.round() as i32,
                                },
                            );
                        }
                    }
                    // Java WholeAdapter stops at the first better non-whole.
                    HeaderTimeShapeEvaluation::Other { .. }
                        if kind == HeaderTimeGlyphKind::Whole =>
                    {
                        break;
                    }
                    HeaderTimeShapeEvaluation::Number { .. }
                        if kind == HeaderTimeGlyphKind::Whole =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
        }
        Ok((
            best_wholes.into_values().collect(),
            best_numbers.into_values().collect(),
        ))
    }

    fn allocate_glyph_id(&mut self) -> Result<usize, NativeHeaderTimeError<C::Error>> {
        self.next_glyph_id = self
            .next_glyph_id
            .checked_add(1)
            .ok_or(NativeHeaderTimeError::GlyphIdExhausted)?;
        Ok(self.next_glyph_id)
    }

    fn allocate_inter_id(&mut self) -> Result<usize, NativeHeaderTimeError<C::Error>> {
        self.next_inter_id = self
            .next_inter_id
            .checked_add(1)
            .ok_or(NativeHeaderTimeError::InterIdExhausted)?;
        Ok(self.next_inter_id)
    }
}

fn browse_projection(
    source: &RunTable,
    roi: HeaderBounds,
    parameters: NativeHeaderTimeParameters,
    range: &mut StaffHeaderRange,
) {
    let mut spaces = Vec::<(i32, i32)>::new();
    let mut start = None;
    let mut stop = roi.x;
    for x in roi.x..=roi.right() {
        let cumul = (roi.y..=roi.y + roi.height - 1)
            .filter(|&y| source.get(x as usize, y as usize) == FOREGROUND)
            .count();
        if cumul <= parameters.maximum_space_cumul {
            start.get_or_insert(x);
            stop = x;
        } else if let Some(space_start) = start.take() {
            spaces.push((space_start, stop));
        }
    }
    if let Some(space_start) = start {
        spaces.push((space_start, stop));
    }
    if spaces.len() >= 2 && spaces[0].1 - roi.x <= parameters.maximum_first_space_width {
        range.set_start(spaces[0].1);
        range.set_stop(spaces.last().expect("two spaces").0);
    }
}

fn search_rect(
    range: &StaffHeaderRange,
    roi: HeaderBounds,
    kind: HeaderTimeGlyphKind,
    margin: i32,
) -> HeaderBounds {
    let half_height = roi.height / 2;
    let (y, height) = match kind {
        HeaderTimeGlyphKind::Whole => (roi.y, roi.height),
        HeaderTimeGlyphKind::Numerator => (roi.y, half_height),
        HeaderTimeGlyphKind::Denominator => (roi.y + roi.height - half_height, half_height),
    };
    HeaderBounds {
        x: range.start().unwrap_or(range.browse_start),
        y: y + margin,
        width: range.width().unwrap_or(0),
        height: height - (2 * margin),
    }
}

fn validate_rect(source: &RunTable, rect: HeaderBounds) -> Result<(), RunTableError> {
    if rect.x < 0
        || rect.y < 0
        || rect.width <= 0
        || rect.height <= 0
        || rect.x as usize + rect.width as usize > source.width()
        || rect.y as usize + rect.height as usize > source.height()
    {
        Err(RunTableError::OutOfBounds)
    } else {
        Ok(())
    }
}

fn crop_vertical(source: &RunTable, rect: HeaderBounds) -> Result<RunTable, RunTableError> {
    validate_rect(source, rect)?;
    let width = rect.width as usize;
    let height = rect.height as usize;
    let mut pixels = vec![BACKGROUND; width * height];
    for y in 0..height {
        for x in 0..width {
            pixels[y * width + x] = source.get(rect.x as usize + x, rect.y as usize + y);
        }
    }
    RunTable::from_pixels(Orientation::Vertical, width, height, &pixels)
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
        for coordinate in entry.run.start..=entry.run.stop() {
            let (x, y) = match component.orientation {
                Orientation::Vertical => (
                    component.left + i32::try_from(entry.sequence - min_sequence).unwrap(),
                    component.top + i32::try_from(coordinate - min_coordinate).unwrap(),
                ),
                Orientation::Horizontal => (
                    component.left + i32::try_from(coordinate - min_coordinate).unwrap(),
                    component.top + i32::try_from(entry.sequence - min_sequence).unwrap(),
                ),
            };
            pixels.push((x, y));
        }
    }
    pixels
}

fn component_bounds(component: &GlyphComponent) -> HeaderBounds {
    HeaderBounds {
        x: component.left,
        y: component.top,
        width: component.width as i32,
        height: component.height as i32,
    }
}

fn gap(one: HeaderBounds, two: HeaderBounds) -> f64 {
    let dx = if one.right() < two.x {
        two.x - one.right() - 1
    } else if two.right() < one.x {
        one.x - two.right() - 1
    } else {
        0
    };
    let one_bottom = one.y + one.height - 1;
    let two_bottom = two.y + two.height - 1;
    let dy = if one_bottom < two.y {
        two.y - one_bottom - 1
    } else if two_bottom < one.y {
        one.y - two_bottom - 1
    } else {
        0
    };
    f64::from(dx).hypot(f64::from(dy))
}

fn near_graph(parts: &[RegisteredPart], maximum_gap: f64) -> Vec<Vec<usize>> {
    let mut graph = vec![Vec::new(); parts.len()];
    for one in 0..parts.len() {
        for two in one + 1..parts.len() {
            if gap(
                component_bounds(&parts[one].component),
                component_bounds(&parts[two].component),
            ) <= maximum_gap
            {
                graph[one].push(two);
                graph[two].push(one);
            }
        }
    }
    graph
}

fn enumerate_connected_subsets(graph: &[Vec<usize>], output: &mut Vec<Vec<usize>>) {
    let count = graph.len();
    if count >= usize::BITS as usize {
        return;
    }
    for mask in 1usize..(1usize << count) {
        let subset = (0..count)
            .filter(|index| mask & (1 << index) != 0)
            .collect::<Vec<_>>();
        let mut reached = vec![subset[0]];
        let mut cursor = 0;
        while cursor < reached.len() {
            for &neighbor in &graph[reached[cursor]] {
                if subset.contains(&neighbor) && !reached.contains(&neighbor) {
                    reached.push(neighbor);
                }
            }
            cursor += 1;
        }
        if reached.len() == subset.len() {
            output.push(subset);
        }
    }
}

fn union_bounds(parts: &[RegisteredPart], subset: &[usize]) -> HeaderBounds {
    let first = component_bounds(&parts[subset[0]].component);
    subset.iter().skip(1).fold(first, |bounds, &index| {
        let other = component_bounds(&parts[index].component);
        let x = bounds.x.min(other.x);
        let y = bounds.y.min(other.y);
        let right = bounds.right().max(other.right());
        let bottom = (bounds.y + bounds.height - 1).max(other.y + other.height - 1);
        HeaderBounds {
            x,
            y,
            width: right - x + 1,
            height: bottom - y + 1,
        }
    })
}

fn compound_glyph(
    id: usize,
    parts: &[RegisteredPart],
    subset: &[usize],
    bounds: HeaderBounds,
) -> Result<HeaderTimeGlyph, RunTableError> {
    let width = bounds.width as usize;
    let height = bounds.height as usize;
    let mut raster = vec![BACKGROUND; width * height];
    let mut sum_x = 0f64;
    let mut sum_y = 0f64;
    let mut weight = 0usize;
    let mut part_ids = Vec::new();
    for &index in subset {
        part_ids.push(parts[index].id);
        for &(x, y) in &parts[index].pixels {
            raster[(y - bounds.y) as usize * width + (x - bounds.x) as usize] = FOREGROUND;
            sum_x += f64::from(x);
            sum_y += f64::from(y);
            weight += 1;
        }
    }
    Ok(HeaderTimeGlyph {
        id,
        part_ids,
        bounds,
        weight,
        centroid_x: sum_x / weight as f64,
        centroid_y: sum_y / weight as f64,
        raster: RunTable::from_pixels(Orientation::Vertical, width, height, &raster)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header_time_column::NeutralSpecificTimeShape;

    #[derive(Default)]
    struct RecordingClassifier {
        calls: Vec<(HeaderTimeGlyphKind, usize, usize)>,
        fail_on: Option<HeaderTimeGlyphKind>,
    }

    impl HeaderTimeShapeClassifier for RecordingClassifier {
        type Error = &'static str;

        fn evaluate(
            &mut self,
            glyph: &HeaderTimeGlyph,
            kind: HeaderTimeGlyphKind,
            _staff_interline: i32,
            _maximum_rank: usize,
            _minimum_grade: f64,
        ) -> Result<Vec<HeaderTimeShapeEvaluation>, Self::Error> {
            self.calls.push((kind, glyph.id, glyph.weight));
            if self.fail_on == Some(kind) {
                return Err("classifier unavailable");
            }
            Ok(match kind {
                HeaderTimeGlyphKind::Whole if glyph.weight >= 12 => {
                    vec![HeaderTimeShapeEvaluation::Whole {
                        value: NeutralTimeValue {
                            specific_shape: None,
                            numerator: 3,
                            denominator: 4,
                        },
                        grade: 0.8,
                        symbol_bounds: glyph.bounds,
                    }]
                }
                HeaderTimeGlyphKind::Numerator => vec![HeaderTimeShapeEvaluation::Number {
                    value: 3,
                    grade: 0.7,
                    symbol_bounds: glyph.bounds,
                }],
                HeaderTimeGlyphKind::Denominator => vec![HeaderTimeShapeEvaluation::Number {
                    value: 4,
                    grade: 0.9,
                    symbol_bounds: glyph.bounds,
                }],
                _ => Vec::new(),
            })
        }
    }

    fn source() -> RunTable {
        let width = 30;
        let height = 20;
        let mut pixels = vec![BACKGROUND; width * height];
        for y in 3..=5 {
            for x in 6..=7 {
                pixels[y * width + x] = FOREGROUND;
            }
        }
        for y in 8..=10 {
            for x in 6..=7 {
                pixels[y * width + x] = FOREGROUND;
            }
        }
        RunTable::from_pixels(Orientation::Horizontal, width, height, &pixels).unwrap()
    }

    fn parameters() -> NativeHeaderTimeParameters {
        NativeHeaderTimeParameters {
            staff_interline: 4,
            maximum_first_space_width: 5,
            maximum_space_cumul: 0,
            minimum_time_width: 3,
            vertical_margin: 1,
            minimum_part_weight: 2,
            maximum_part_gap: 3.0,
            maximum_time_width: 8,
            minimum_whole_weight: 6,
            minimum_half_weight: 4,
            maximum_eval_rank: 3,
            minimum_classifier_grade: 0.5,
            intrinsic_ratio: 0.8,
        }
    }

    fn recognizer(
        classifier: RecordingClassifier,
    ) -> NativeHeaderTimeRecognizer<RecordingClassifier> {
        NativeHeaderTimeRecognizer::new(
            classifier,
            BTreeMap::from([(7, source())]),
            BTreeMap::from([(
                1,
                HeaderTimeRasterContext {
                    roi: HeaderBounds {
                        x: 2,
                        y: 2,
                        width: 19,
                        height: 10,
                    },
                },
            )]),
            BTreeMap::from([(1, parameters())]),
            100,
            200,
        )
    }

    #[test]
    fn projection_components_and_classifier_follow_whole_num_den_order() {
        let mut recognizer = recognizer(RecordingClassifier::default());
        let proposals = recognizer
            .classify_time_parts(
                HeaderTimeRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    browse_start: 2,
                },
                &StaffHeaderRange::default(),
            )
            .unwrap();

        assert_eq!(proposals.range.start(), Ok(5));
        assert_eq!(proposals.range.precise_stop(), Some(8));
        assert_eq!(proposals.wholes.len(), 1);
        assert!((proposals.wholes[0].grade - 0.64).abs() < f64::EPSILON);
        assert_eq!(proposals.numerators.len(), 1);
        assert_eq!(proposals.denominators.len(), 1);
        let kinds = recognizer
            .classifier()
            .calls
            .iter()
            .map(|call| call.0)
            .collect::<Vec<_>>();
        let first_num = kinds
            .iter()
            .position(|kind| *kind == HeaderTimeGlyphKind::Numerator)
            .unwrap();
        let first_den = kinds
            .iter()
            .position(|kind| *kind == HeaderTimeGlyphKind::Denominator)
            .unwrap();
        assert!(
            kinds[..first_num]
                .iter()
                .all(|kind| *kind == HeaderTimeGlyphKind::Whole)
        );
        assert!(
            kinds[first_num..first_den]
                .iter()
                .all(|kind| *kind == HeaderTimeGlyphKind::Numerator)
        );
        assert!(
            kinds[first_den..]
                .iter()
                .all(|kind| *kind == HeaderTimeGlyphKind::Denominator)
        );
        assert_eq!(proposals.wholes[0].value.specific_shape, None);
    }

    #[test]
    fn whole_adapter_stops_on_better_non_whole_evaluation() {
        struct BreakClassifier;
        impl HeaderTimeShapeClassifier for BreakClassifier {
            type Error = &'static str;
            fn evaluate(
                &mut self,
                glyph: &HeaderTimeGlyph,
                kind: HeaderTimeGlyphKind,
                _: i32,
                _: usize,
                _: f64,
            ) -> Result<Vec<HeaderTimeShapeEvaluation>, Self::Error> {
                Ok(if kind == HeaderTimeGlyphKind::Whole {
                    vec![
                        HeaderTimeShapeEvaluation::Other { grade: 0.95 },
                        HeaderTimeShapeEvaluation::Whole {
                            value: NeutralTimeValue {
                                specific_shape: Some(NeutralSpecificTimeShape::Common),
                                numerator: 4,
                                denominator: 4,
                            },
                            grade: 0.9,
                            symbol_bounds: glyph.bounds,
                        },
                    ]
                } else {
                    Vec::new()
                })
            }
        }
        let base = recognizer(RecordingClassifier::default());
        let mut recognizer = NativeHeaderTimeRecognizer::new(
            BreakClassifier,
            base.sources,
            base.contexts,
            base.parameters,
            0,
            0,
        );
        let proposals = recognizer
            .classify_time_parts(
                HeaderTimeRecognitionInput {
                    system_id: 7,
                    staff_id: 1,
                    browse_start: 2,
                },
                &StaffHeaderRange::default(),
            )
            .unwrap();
        assert!(proposals.wholes.is_empty());
    }

    #[test]
    fn classifier_failure_preserves_registered_and_evaluated_prefix() {
        let mut recognizer = recognizer(RecordingClassifier {
            fail_on: Some(HeaderTimeGlyphKind::Numerator),
            ..RecordingClassifier::default()
        });
        let result = recognizer.classify_time_parts(
            HeaderTimeRecognitionInput {
                system_id: 7,
                staff_id: 1,
                browse_start: 2,
            },
            &StaffHeaderRange::default(),
        );
        assert!(matches!(
            result,
            Err(NativeHeaderTimeError::Classifier("classifier unavailable"))
        ));
        assert!(
            recognizer.mutations().iter().any(|mutation| matches!(
                mutation,
                NativeHeaderTimeMutation::InterAllocated { .. }
            ))
        );
        assert!(matches!(
            recognizer.mutations().last(),
            Some(NativeHeaderTimeMutation::GlyphEvaluated {
                kind: HeaderTimeGlyphKind::Numerator,
                ..
            })
        ));
    }
}
