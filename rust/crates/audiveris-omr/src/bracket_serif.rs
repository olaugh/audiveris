// SPDX-License-Identifier: AGPL-3.0-or-later

//! Section-backed evidence for Java `BarsRetriever.detectBracketEnds`.
//!
//! The dependency-light predicates live in `audiveris-image::bars_logic`.
//! This adapter supplies the production filament bounds and the two section
//! lags, preserves component registration before the final weight gate, and
//! mutates projector and graph copies in Java traversal order.

use std::{error::Error, fmt};

use audiveris_image::{
    bar_alignment::{BarAlignment, VerticalSide},
    bar_sticks::{BarStick, BarStickBuildState},
    bars_logic::{
        BracketEndDetection, LocatedSectionId, SectionLag, SerifCompoundCandidate, peak_extension,
        select_serif_compounds, serif_lookup_bounds,
    },
    peak_graph::PeakGraph,
    section::{Bounds, Section},
    staff_peak::{PeakBounds, StaffPeak, StaffPeakKey},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BracketSerifParameters {
    pub interline: usize,
    pub maximum_foreground_thickness: i32,
    pub minimum_bracket_width: i32,
    pub maximum_bracket_extension: f64,
    pub serif_roi_width: i32,
    pub serif_roi_height: i32,
    pub serif_minimum_weight: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredSerifFilament {
    pub id: usize,
    pub interline: usize,
    pub members: Vec<LocatedSectionId>,
    pub bounds: PeakBounds,
    pub weight: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracketSerifAttachment {
    pub peak: StaffPeakKey,
    pub side: VerticalSide,
    pub roi: PeakBounds,
    pub filament_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BracketSerifMutation {
    FilamentRegistered(usize),
    Attached {
        peak: StaffPeakKey,
        side: VerticalSide,
        filament_id: usize,
    },
}

/// Sheet-global serif filament registrations and accepted peak attachments.
///
/// Java uses the same `FilamentIndex` as bar sticks. Callers therefore start
/// this store at `BarStickBuildState::next_filament_id()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BracketSerifStore {
    next_filament_id: usize,
    filaments: Vec<RegisteredSerifFilament>,
    attachments: Vec<BracketSerifAttachment>,
    mutations: Vec<BracketSerifMutation>,
}

impl BracketSerifStore {
    pub fn new(first_filament_id: usize) -> Result<Self, BracketSerifError> {
        if first_filament_id == 0 {
            return Err(BracketSerifError::InvalidFirstFilamentId);
        }
        Ok(Self {
            next_filament_id: first_filament_id,
            filaments: Vec::new(),
            attachments: Vec::new(),
            mutations: Vec::new(),
        })
    }

    #[must_use]
    pub const fn next_filament_id(&self) -> usize {
        self.next_filament_id
    }

    #[must_use]
    pub fn filaments(&self) -> &[RegisteredSerifFilament] {
        &self.filaments
    }

    #[must_use]
    pub fn attachments(&self) -> &[BracketSerifAttachment] {
        &self.attachments
    }

    #[must_use]
    pub fn mutations(&self) -> &[BracketSerifMutation] {
        &self.mutations
    }

    fn register(
        &mut self,
        interline: usize,
        members: Vec<LocatedSectionId>,
        sections: &[LocatedSection<'_>],
    ) -> Result<usize, BracketSerifError> {
        let id = self.next_filament_id;
        self.next_filament_id = id
            .checked_add(1)
            .ok_or(BracketSerifError::FilamentIdExhausted)?;
        let (bounds, weight) = compound_geometry(&members, sections)?;
        self.filaments.push(RegisteredSerifFilament {
            id,
            interline,
            members,
            bounds,
            weight,
        });
        self.mutations
            .push(BracketSerifMutation::FilamentRegistered(id));
        Ok(id)
    }

    fn attach(&mut self, attachment: BracketSerifAttachment) {
        self.attachments.push(attachment);
        self.mutations.push(BracketSerifMutation::Attached {
            peak: attachment.peak,
            side: attachment.side,
            filament_id: attachment.filament_id,
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BracketSerifError {
    InvalidFirstFilamentId,
    InvalidParameters,
    InvalidStartIndex(usize),
    MissingPeak(StaffPeakKey),
    MissingBarStick(StaffPeakKey),
    MissingSection(LocatedSectionId),
    CoordinateOverflow,
    FilamentIdExhausted,
}

impl fmt::Display for BracketSerifError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFirstFilamentId => {
                formatter.write_str("first serif-filament ID must be positive")
            }
            Self::InvalidParameters => formatter.write_str("invalid bracket-serif parameters"),
            Self::InvalidStartIndex(index) => write!(formatter, "invalid start peak index {index}"),
            Self::MissingPeak(peak) => write!(formatter, "bracket peak is absent: {peak:?}"),
            Self::MissingBarStick(peak) => {
                write!(formatter, "bracket candidate has no bar filament: {peak:?}")
            }
            Self::MissingSection(section) => {
                write!(formatter, "bracket serif section is absent: {section:?}")
            }
            Self::CoordinateOverflow => formatter.write_str("bracket serif coordinate overflow"),
            Self::FilamentIdExhausted => formatter.write_str("serif filament ID exhausted"),
        }
    }
}

impl Error for BracketSerifError {}

/// Run one staff's Java bracket-end traversal against real bar filaments and
/// section lags. Top is attempted before bottom for every candidate.
///
/// Mutations are deliberately non-transactional. All connected components are
/// registered before selection and the minimum-weight rejection. Earlier
/// registrations and accepted sides remain visible if a later registration
/// fails.
#[allow(clippy::too_many_arguments)]
pub fn detect_bracket_ends_from_sections(
    peaks: &mut [StaffPeak],
    graph: &mut PeakGraph<BarAlignment>,
    start_peak_index: Option<usize>,
    sticks: &BarStickBuildState,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    parameters: BracketSerifParameters,
    store: &mut BracketSerifStore,
) -> Result<Vec<BracketEndDetection>, BracketSerifError> {
    validate_parameters(parameters)?;
    let Some(start_index) = start_peak_index else {
        return Ok(Vec::new());
    };
    if start_index >= peaks.len() {
        return Err(BracketSerifError::InvalidStartIndex(start_index));
    }

    let source = locate_sections(horizontal_sections, vertical_sections);
    let mut detections = Vec::new();
    for index in (0..start_index).rev() {
        if peaks[index].is_brace() {
            break;
        }
        if peaks[index].width() < parameters.minimum_bracket_width {
            continue;
        }

        let peak = peaks[index].key();
        let right = peaks[index + 1].key();
        if graph.vertex(peak).is_none() {
            return Err(BracketSerifError::MissingPeak(peak));
        }
        let bar_stick = stick_of(sticks, peak)?;
        let right_stick = stick_of(sticks, right)?;
        for side in [VerticalSide::Top, VerticalSide::Bottom] {
            let bounds = bounds_to_peak(bar_stick.bounds)?;
            let extension = peak_extension(
                &peaks[index],
                bounds,
                parameters.maximum_foreground_thickness,
                side,
            );
            if extension > parameters.maximum_bracket_extension {
                continue;
            }
            let roi = serif_lookup_bounds(
                &peaks[index],
                parameters.maximum_foreground_thickness,
                parameters.serif_roi_width,
                parameters.serif_roi_height,
                side,
            );
            let Some(filament_id) = build_serif(
                roi,
                side,
                bar_stick,
                right_stick,
                &source,
                parameters,
                store,
            )?
            else {
                continue;
            };

            // Graph presence was checked before section/index mutations, so
            // these two writes form the neutral equivalent of mutating the one
            // Java StaffPeak object shared by projector and graph.
            peaks[index].set_bracket_end(side);
            graph
                .vertex_mut(peak)
                .expect("candidate graph presence was validated")
                .set_bracket_end(side);
            store.attach(BracketSerifAttachment {
                peak,
                side,
                roi,
                filament_id,
            });
            detections.push(BracketEndDetection { peak, side });
        }
    }
    Ok(detections)
}

fn validate_parameters(parameters: BracketSerifParameters) -> Result<(), BracketSerifError> {
    if parameters.interline == 0
        || parameters.maximum_foreground_thickness < 0
        || parameters.minimum_bracket_width < 0
        || !parameters.maximum_bracket_extension.is_finite()
        || parameters.maximum_bracket_extension < 0.0
        || parameters.serif_roi_width <= 0
        || parameters.serif_roi_height <= 0
        || parameters.serif_minimum_weight < 0
    {
        return Err(BracketSerifError::InvalidParameters);
    }
    Ok(())
}

fn stick_of(
    sticks: &BarStickBuildState,
    peak: StaffPeakKey,
) -> Result<&BarStick, BracketSerifError> {
    sticks
        .sticks()
        .iter()
        .find(|stick| stick.peak == peak)
        .ok_or(BracketSerifError::MissingBarStick(peak))
}

#[derive(Clone, Copy)]
struct LocatedSection<'a> {
    location: LocatedSectionId,
    section: &'a Section,
}

fn locate_sections<'a>(
    horizontal: &'a [Section],
    vertical: &'a [Section],
) -> Vec<LocatedSection<'a>> {
    horizontal
        .iter()
        .map(|section| LocatedSection {
            location: LocatedSectionId {
                lag: SectionLag::Horizontal,
                id: section.id(),
            },
            section,
        })
        .chain(vertical.iter().map(|section| LocatedSection {
            location: LocatedSectionId {
                lag: SectionLag::Vertical,
                id: section.id(),
            },
            section,
        }))
        .collect()
}

fn build_serif(
    roi: PeakBounds,
    side: VerticalSide,
    bar: &BarStick,
    right: &BarStick,
    source: &[LocatedSection<'_>],
    parameters: BracketSerifParameters,
    store: &mut BracketSerifStore,
) -> Result<Option<usize>, BracketSerifError> {
    let excluded = bar
        .members
        .iter()
        .chain(&right.members)
        .copied()
        .collect::<Vec<_>>();
    let candidates = source
        .iter()
        .copied()
        .filter(|candidate| {
            rectangle_intersects(roi, candidate.section.bounds())
                && !excluded.contains(&candidate.location)
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let components = connected_components(&candidates);
    let mut registered = Vec::with_capacity(components.len());
    for members in components {
        let id = store.register(parameters.interline, members.clone(), source)?;
        let filament = store
            .filaments
            .last()
            .expect("registration appends one filament");
        let (centroid_x, centroid_y) = compound_centroid(&members, source)?;
        registered.push(SerifCompoundCandidate {
            id,
            centroid_x,
            centroid_y,
            weight: filament.weight,
        });
    }

    let selected = select_serif_compounds(&registered, roi, side, parameters.serif_minimum_weight);
    let final_id = if selected.len() > 1 {
        let mut members = Vec::new();
        for selected in &selected {
            let filament = store
                .filaments
                .iter()
                .find(|filament| filament.id == selected.id)
                .expect("selected compound was registered above");
            members.extend(filament.members.iter().copied());
        }
        sort_and_dedup_members(&mut members, source)?;
        store.register(parameters.interline, members, source)?
    } else {
        selected[0].id
    };
    let final_weight = store
        .filaments
        .iter()
        .find(|filament| filament.id == final_id)
        .expect("final serif was registered")
        .weight;
    Ok((final_weight >= parameters.serif_minimum_weight).then_some(final_id))
}

fn connected_components(candidates: &[LocatedSection<'_>]) -> Vec<Vec<LocatedSectionId>> {
    let mut visited = vec![false; candidates.len()];
    let mut components = Vec::new();
    for seed in 0..candidates.len() {
        if visited[seed] {
            continue;
        }
        visited[seed] = true;
        let mut queue = vec![seed];
        let mut component = Vec::new();
        while let Some(index) = queue.pop() {
            component.push(candidates[index].location);
            for other in 0..candidates.len() {
                if !visited[other] && candidates[index].section.touches(candidates[other].section) {
                    visited[other] = true;
                    queue.push(other);
                }
            }
        }
        components.push(component);
    }
    components
}

fn compound_geometry(
    members: &[LocatedSectionId],
    source: &[LocatedSection<'_>],
) -> Result<(PeakBounds, i32), BracketSerifError> {
    let mut sections = members
        .iter()
        .map(|member| section_of(*member, source))
        .collect::<Result<Vec<_>, _>>()?;
    sections.sort_by_key(|section| {
        let bounds = section.bounds();
        (bounds.x, bounds.y, section.id())
    });
    let first = sections
        .first()
        .expect("component construction never emits an empty member set")
        .bounds();
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x.saturating_add(first.width);
    let mut bottom = first.y.saturating_add(first.height);
    let mut weight = 0_i32;
    for section in sections {
        let bounds = section.bounds();
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x.saturating_add(bounds.width));
        bottom = bottom.max(bounds.y.saturating_add(bounds.height));
        weight = weight.wrapping_add(section.weight() as i32);
    }
    Ok((
        PeakBounds {
            x: i32::try_from(left).map_err(|_| BracketSerifError::CoordinateOverflow)?,
            y: i32::try_from(top).map_err(|_| BracketSerifError::CoordinateOverflow)?,
            width: i32::try_from(right - left)
                .map_err(|_| BracketSerifError::CoordinateOverflow)?,
            height: i32::try_from(bottom - top)
                .map_err(|_| BracketSerifError::CoordinateOverflow)?,
        },
        weight,
    ))
}

fn compound_centroid(
    members: &[LocatedSectionId],
    source: &[LocatedSection<'_>],
) -> Result<(f64, f64), BracketSerifError> {
    let mut weight = 0_usize;
    let mut x = 0.0;
    let mut y = 0.0;
    for &member in members {
        let section = section_of(member, source)?;
        let section_weight = section.weight();
        let centroid = section.centroid_2d();
        weight += section_weight;
        x += centroid.0 * section_weight as f64;
        y += centroid.1 * section_weight as f64;
    }
    Ok((x / weight as f64, y / weight as f64))
}

fn sort_and_dedup_members(
    members: &mut Vec<LocatedSectionId>,
    source: &[LocatedSection<'_>],
) -> Result<(), BracketSerifError> {
    let mut keyed = members
        .drain(..)
        .map(|member| {
            let section = section_of(member, source)?;
            let bounds = section.bounds();
            Ok(((bounds.x, bounds.y, section.id()), member))
        })
        .collect::<Result<Vec<_>, BracketSerifError>>()?;
    keyed.sort_by_key(|(key, _)| *key);
    keyed.dedup_by_key(|(_, member)| *member);
    members.extend(keyed.into_iter().map(|(_, member)| member));
    Ok(())
}

fn section_of<'a>(
    member: LocatedSectionId,
    source: &'a [LocatedSection<'a>],
) -> Result<&'a Section, BracketSerifError> {
    source
        .iter()
        .find(|candidate| candidate.location == member)
        .map(|candidate| candidate.section)
        .ok_or(BracketSerifError::MissingSection(member))
}

fn rectangle_intersects(roi: PeakBounds, section: Bounds) -> bool {
    let left = i128::from(roi.x);
    let top = i128::from(roi.y);
    let right = left + i128::from(roi.width);
    let bottom = top + i128::from(roi.height);
    let section_left = section.x as i128;
    let section_top = section.y as i128;
    let section_right = section_left + section.width as i128;
    let section_bottom = section_top + section.height as i128;
    section_right > left && section_bottom > top && section_left < right && section_top < bottom
}

fn bounds_to_peak(bounds: Bounds) -> Result<PeakBounds, BracketSerifError> {
    Ok(PeakBounds {
        x: i32::try_from(bounds.x).map_err(|_| BracketSerifError::CoordinateOverflow)?,
        y: i32::try_from(bounds.y).map_err(|_| BracketSerifError::CoordinateOverflow)?,
        width: i32::try_from(bounds.width).map_err(|_| BracketSerifError::CoordinateOverflow)?,
        height: i32::try_from(bounds.height).map_err(|_| BracketSerifError::CoordinateOverflow)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_column::StaffId,
        bar_sticks::{BarStickParameters, build_bar_sticks},
        peak_graph::PeakGraph,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections_from_id},
        staff_peak::{HorizontalSide, StaffPeak},
    };

    fn rectangle(
        id: usize,
        orientation: Orientation,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Section {
        let mut table = RunTable::new(orientation, x + width + 1, y + height + 1).unwrap();
        match orientation {
            Orientation::Vertical => {
                for column in x..(x + width) {
                    table.add_run(column, Run::new(y, height)).unwrap();
                }
            }
            Orientation::Horizontal => {
                for row in y..(y + height) {
                    table.add_run(row, Run::new(x, width)).unwrap();
                }
            }
        }
        build_sections_from_id(&table, JunctionPolicy::All, id)
            .into_iter()
            .find(|section| section.id() == id)
            .unwrap()
    }

    fn peak(start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::new(StaffId::new(1), 30, 40, start, stop).unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn fixture(
        serif_sections: Vec<Section>,
    ) -> (
        Vec<StaffPeak>,
        PeakGraph<BarAlignment>,
        BarStickBuildState,
        Vec<Section>,
        Vec<Section>,
    ) {
        let vertical = vec![
            rectangle(1, Orientation::Vertical, 10, 30, 4, 11),
            rectangle(2, Orientation::Vertical, 24, 30, 2, 11),
            rectangle(3, Orientation::Vertical, 30, 30, 2, 11),
        ];
        let horizontal = serif_sections;
        let candidate = peak(10, 13);
        let right = peak(24, 25);
        let mut start = peak(30, 31);
        start.set_staff_end(HorizontalSide::Left);
        let mut graph = PeakGraph::new();
        for value in [&candidate, &right, &start] {
            graph.add_vertex(value.clone());
        }
        let mut sticks = BarStickBuildState::new(1).unwrap();
        build_bar_sticks(
            &mut graph,
            &[candidate.key(), right.key(), start.key()],
            &vertical,
            &horizontal,
            BarStickParameters {
                vertical_extension: 0,
                minimum_core_section_length: 4,
                probe_width: 2,
                minimum_probe_weight: 1,
                segment_length: 4,
                minimum_mean_curvature: 0.0,
                first_filament_id: 1,
            },
            &mut sticks,
        )
        .unwrap();
        (
            vec![candidate, right, start],
            graph,
            sticks,
            vertical,
            horizontal,
        )
    }

    fn parameters(minimum_weight: i32) -> BracketSerifParameters {
        BracketSerifParameters {
            interline: 10,
            maximum_foreground_thickness: 2,
            minimum_bracket_width: 4,
            maximum_bracket_extension: 2.0,
            serif_roi_width: 10,
            serif_roi_height: 10,
            serif_minimum_weight: minimum_weight,
        }
    }

    #[test]
    fn real_section_components_register_then_merge_before_top_attachment() {
        let serif_sections = vec![
            rectangle(10, Orientation::Horizontal, 14, 27, 3, 1),
            rectangle(11, Orientation::Horizontal, 19, 27, 3, 1),
        ];
        let (mut peaks, mut graph, sticks, vertical, horizontal) = fixture(serif_sections);
        let candidate = peaks[0].key();
        let mut store = BracketSerifStore::new(sticks.next_filament_id()).unwrap();

        let detections = detect_bracket_ends_from_sections(
            &mut peaks,
            &mut graph,
            Some(2),
            &sticks,
            &vertical,
            &horizontal,
            parameters(6),
            &mut store,
        )
        .unwrap();

        assert_eq!(
            detections,
            [BracketEndDetection {
                peak: candidate,
                side: VerticalSide::Top,
            }]
        );
        assert_eq!(store.filaments().len(), 3);
        assert_eq!(store.filaments()[0].weight, 3);
        assert_eq!(store.filaments()[1].weight, 3);
        assert_eq!(store.filaments()[2].weight, 6);
        assert_eq!(store.attachments()[0].filament_id, store.filaments()[2].id);
        assert!(peaks[0].is_bracket_end(VerticalSide::Top));
        assert!(
            graph
                .vertex(candidate)
                .unwrap()
                .is_bracket_end(VerticalSide::Top)
        );
        assert!(!peaks[0].is_bracket_end(VerticalSide::Bottom));
    }

    #[test]
    fn rejected_light_serif_keeps_registered_component_without_peak_mutation() {
        let serif_sections = vec![rectangle(10, Orientation::Horizontal, 14, 27, 3, 1)];
        let (mut peaks, mut graph, sticks, vertical, horizontal) = fixture(serif_sections);
        let candidate = peaks[0].key();
        let mut store = BracketSerifStore::new(sticks.next_filament_id()).unwrap();

        assert!(
            detect_bracket_ends_from_sections(
                &mut peaks,
                &mut graph,
                Some(2),
                &sticks,
                &vertical,
                &horizontal,
                parameters(4),
                &mut store,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(store.filaments().len(), 1);
        assert!(store.attachments().is_empty());
        assert!(!peaks[0].is_bracket());
        assert!(!graph.vertex(candidate).unwrap().is_bracket());
    }

    #[test]
    fn later_registration_failure_keeps_earlier_component() {
        let serif_sections = vec![
            rectangle(10, Orientation::Horizontal, 14, 27, 3, 1),
            rectangle(11, Orientation::Horizontal, 19, 27, 3, 1),
        ];
        let (mut peaks, mut graph, sticks, vertical, horizontal) = fixture(serif_sections);
        let mut store = BracketSerifStore::new(usize::MAX - 1).unwrap();

        assert_eq!(
            detect_bracket_ends_from_sections(
                &mut peaks,
                &mut graph,
                Some(2),
                &sticks,
                &vertical,
                &horizontal,
                parameters(6),
                &mut store,
            ),
            Err(BracketSerifError::FilamentIdExhausted)
        );
        assert_eq!(store.filaments().len(), 1);
        assert_eq!(store.filaments()[0].id, usize::MAX - 1);
        assert!(store.attachments().is_empty());
        assert!(!peaks[0].is_bracket());
    }
}
