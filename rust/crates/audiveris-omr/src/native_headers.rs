// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production native HEADERS, derived only from GRID state.
//!
//! The visual clef/key/time recognizers need a precise header start and a
//! staff's first good connected barline after that start.  The corpus harness
//! used to supply both from its Java oracle. This module closes that input
//! seam, adapts the live GRID sheet/SIG into [`HeadlessHeaderSystem`] state,
//! and composes the concrete clef, key, and time recognizers in Java order.

use std::{collections::BTreeMap, convert::Infallible, error::Error, fmt};

use audiveris_image::{
    bars_logic::VerticalInterKind,
    grid_sig::{GridInterId, GridSigNode, GridSigRelation},
    lines_coordinator::StaffCandidateKind,
    run_table::RunTable,
    spots::HeaderErase,
};

use crate::{
    clef_classifier::{BundledClefClassifier, ClefClassificationError},
    clef_column::{
        ClefColumnError, ClefLifecycleRecognizer, ClefLookupParameters, ClefLookupStaffGeometry,
        HeadlessClefColumn, NativeClefError, NativeClefParameters, NativeClefProposalRecognizer,
        NeutralClefCandidate, StaffLineOrdinates, build_clef_lookup_contexts_at,
    },
    clef_parameters::SheetClefParameters,
    header_builder::{
        HeaderBarGroupRelation, HeaderBarline, HeaderBuilderError, HeaderHorizontalSide,
        compute_header_starts,
    },
    header_time_builder::{
        HeaderTimeRasterContext, NativeHeaderTimeError, NativeHeaderTimeParameters,
        NativeHeaderTimeRecognizer,
    },
    header_time_column::{
        HeaderTimeColumnError, HeaderTimeLifecycleContext, HeaderTimeLifecycleRecognizer,
        HeadlessHeaderTimeColumn, NeutralTimeCandidate, NeutralTimeValue,
        VisualHeaderTimeProposalRecognizer,
    },
    headers_step::{HeadlessHeaderStaff, HeadlessHeaderSystem},
    key_classifier::{BundledKeyClassifier, KeyClassificationError},
    key_column::{
        HeadlessKeyColumn, KeyClefSupport, KeyColumnError, KeyLifecycleContext,
        KeyLifecycleRecognizer, NativeKeyError, NativeKeyParameters, NativeKeyProposalRecognizer,
        NativeKeyStaffContext, NeutralKeyCandidate, StaffPitchGeometry,
    },
    key_parameters::{
        KeyExtractorParameters, KeyPipelineParameters, browse_envelope, max_eval_rank,
        max_header_width, max_slice_distance,
    },
    recognize::{GridLinesRecognition, StaffLineGeometry},
    staff_header::{HeaderBounds, StaffHeader},
    time_classifier::{BundledTimeClassifier, TimeClassificationError},
};

/// Java `BarlineInter.isGood()`'s hard-coded threshold.
///
/// This deliberately is not `Grades.goodInterGrade` (0.4).  BarlineInter
/// overrides the general predicate with `getGrade() >= 0.6`.
pub const GOOD_BARLINE_GRADE: f64 = 0.6;

/// All production HEADERS positioning inputs for one sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridContext {
    pub sheet_interline: i32,
    /// Systems in Java/GRID source order.
    pub systems: Vec<NativeHeaderGridSystem>,
}

/// GRID-derived state for one invocation of Java `HeaderBuilder(system)`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridSystem {
    pub system_id: usize,
    /// Barline/SIG state with `StaffHeader.start` already computed.
    pub header_system: HeadlessHeaderSystem,
    /// Staff inputs in system staff order.
    pub staffs: Vec<NativeHeaderGridStaff>,
}

/// Geometry and browse-stop inputs retained past header-start construction.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderGridStaff {
    pub staff_id: usize,
    pub specific_interline: i32,
    pub lines: StaffLineGeometry,
    /// Ordered `BarlineInter.getBounds().x` values accepted by
    /// `Staff.getBrowseStop`: grade at least 0.6 and a bar-connection edge.
    pub good_connected_bar_starts: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeHeaderGridContextError {
    InvalidSheetInterline(i32),
    MissingSystem(usize),
    MissingStaff {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffLines {
        system_id: usize,
        staff_id: usize,
    },
    InvalidStaffInterline {
        system_id: usize,
        staff_id: usize,
        value: usize,
    },
    MissingStaffLineOrdinate {
        system_id: usize,
        staff_id: usize,
        x: i32,
    },
    MissingBarline {
        system_id: usize,
        staff_id: usize,
        inter_id: usize,
    },
    HeaderStart {
        system_id: usize,
        source: HeaderBuilderError<Infallible>,
    },
}

impl fmt::Display for NativeHeaderGridContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSheetInterline(value) => {
                write!(formatter, "invalid HEADERS sheet interline {value}")
            }
            Self::MissingSystem(id) => write!(formatter, "GRID system {id} is missing"),
            Self::MissingStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "GRID system {system_id} is missing staff {staff_id}"
            ),
            Self::MissingStaffLines {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has no line geometry"
            ),
            Self::InvalidStaffInterline {
                system_id,
                staff_id,
                value,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has invalid interline {value}"
            ),
            Self::MissingStaffLineOrdinate {
                system_id,
                staff_id,
                x,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} has no line ordinate at x={x}"
            ),
            Self::MissingBarline {
                system_id,
                staff_id,
                inter_id,
            } => write!(
                formatter,
                "GRID system {system_id} staff {staff_id} is missing barline inter {inter_id}"
            ),
            Self::HeaderStart { system_id, source } => {
                write!(
                    formatter,
                    "HEADERS system {system_id} start failed: {source}"
                )
            }
        }
    }
}

impl Error for NativeHeaderGridContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HeaderStart { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Build HEADERS' positioning state without consulting a Java oracle.
pub fn derive_native_header_grid_context(
    grid: &GridLinesRecognition,
) -> Result<NativeHeaderGridContext, NativeHeaderGridContextError> {
    let sheet_interline = grid.scale.scale.interline.main;
    if sheet_interline <= 0 {
        return Err(NativeHeaderGridContextError::InvalidSheetInterline(
            sheet_interline,
        ));
    }
    let maximum_clef_end = SheetClefParameters::new(sheet_interline).max_clef_end;
    let mut systems = Vec::with_capacity(grid.peak_graph.systems.len());

    for (system_index, staff_ids) in grid.peak_graph.systems.iter().enumerate() {
        let system_id = system_index + 1;
        let grid_system = grid
            .peak_graph
            .sig
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeHeaderGridContextError::MissingSystem(system_id))?;

        let mut header_staffs = Vec::with_capacity(staff_ids.len());
        let mut staff_inputs = Vec::with_capacity(staff_ids.len());
        let mut barlines = Vec::new();
        let sig_vertex_ids = grid_system
            .sig
            .nodes_in_order()
            .map(|(id, _)| id.value())
            .collect();

        for &staff_id in staff_ids {
            let sheet_staff = grid
                .peak_graph
                .sheet_staffs
                .iter()
                .find(|staff| staff.id == staff_id)
                .ok_or(NativeHeaderGridContextError::MissingStaff {
                    system_id,
                    staff_id,
                })?;
            let lines = grid
                .staff_lines
                .iter()
                .find(|lines| lines.staff_id == staff_id)
                .cloned()
                .ok_or(NativeHeaderGridContextError::MissingStaffLines {
                    system_id,
                    staff_id,
                })?;
            let first_line_y_at_left = lines.first_line_y_at(lines.left).ok_or(
                NativeHeaderGridContextError::MissingStaffLineOrdinate {
                    system_id,
                    staff_id,
                    x: lines.left,
                },
            )?;
            let last_line_y_at_left = lines.last_line_y_at(lines.left).ok_or(
                NativeHeaderGridContextError::MissingStaffLineOrdinate {
                    system_id,
                    staff_id,
                    x: lines.left,
                },
            )?;

            let mut header_staff = HeadlessHeaderStaff::new(staff_id);
            header_staff.part_id = part_id_for_staff(grid_system, staff_id);
            header_staff.tablature = sheet_staff.kind == StaffCandidateKind::Tablature;
            header_staff.one_line = sheet_staff.kind == StaffCandidateKind::OneLine;
            header_staff.maximum_clef_end = maximum_clef_end;
            header_staff.left_abscissa = lines.left;
            header_staff.right_abscissa = lines.right;
            header_staff.first_line_y_at_left = first_line_y_at_left;
            header_staff.last_line_y_at_left = last_line_y_at_left;

            let mut good_connected_bar_starts = Vec::new();
            for &inter_id in &sheet_staff.barlines {
                let Some(GridSigNode::Vertical { plan, .. }) = grid_system.sig.node(inter_id)
                else {
                    return Err(NativeHeaderGridContextError::MissingBarline {
                        system_id,
                        staff_id,
                        inter_id: inter_id.value(),
                    });
                };
                let VerticalInterKind::Barline {
                    left_staff_end,
                    right_staff_end,
                    ..
                } = plan.kind
                else {
                    return Err(NativeHeaderGridContextError::MissingBarline {
                        system_id,
                        staff_id,
                        inter_id: inter_id.value(),
                    });
                };
                let bounds = vertical_barline_bounds(plan.median, plan.width);
                let grade = grid_system
                    .sig
                    .node(inter_id)
                    .expect("the vertical node was just resolved")
                    .intrinsic_grade();
                let staff_end = if left_staff_end {
                    Some(HeaderHorizontalSide::Left)
                } else if right_staff_end {
                    Some(HeaderHorizontalSide::Right)
                } else {
                    None
                };
                let id = inter_id.value();
                header_staff.barline_ids.push(id);
                if left_staff_end {
                    header_staff.left_side_barline_id = Some(id);
                }
                if right_staff_end {
                    header_staff.right_side_barline_id = Some(id);
                }
                if is_good_connected_barline(grade, has_bar_connection(&grid_system.sig, inter_id))
                {
                    good_connected_bar_starts.push(bounds.x);
                }
                barlines.push(HeaderBarline {
                    id,
                    bounds: Some(bounds),
                    staff_id: Some(staff_id),
                    dummy: false,
                    grade,
                    staff_end,
                    removed: false,
                });
            }

            let specific_interline = i32::try_from(sheet_staff.interline).map_err(|_| {
                NativeHeaderGridContextError::InvalidStaffInterline {
                    system_id,
                    staff_id,
                    value: sheet_staff.interline,
                }
            })?;
            if specific_interline <= 0 {
                return Err(NativeHeaderGridContextError::InvalidStaffInterline {
                    system_id,
                    staff_id,
                    value: sheet_staff.interline,
                });
            }
            staff_inputs.push(NativeHeaderGridStaff {
                staff_id,
                specific_interline,
                lines,
                good_connected_bar_starts,
            });
            header_staffs.push(header_staff);
        }

        let bar_group_relations = grid_system
            .sig
            .edges()
            .iter()
            .filter_map(|edge| {
                matches!(edge.relation, GridSigRelation::BarGroup { .. }).then_some(
                    HeaderBarGroupRelation {
                        source: edge.source.value(),
                        target: edge.target.value(),
                    },
                )
            })
            .collect();
        let last_inter_id = grid_system
            .sig
            .nodes_in_order()
            .last()
            .map_or(0, |(id, _)| id.value());
        let mut header_system = HeadlessHeaderSystem::new(system_id, header_staffs);
        header_system.last_inter_id = last_inter_id;
        header_system.barlines = barlines;
        header_system.sig_vertex_ids = sig_vertex_ids;
        header_system.bar_group_relations = bar_group_relations;
        compute_header_starts::<Infallible>(&mut header_system)
            .map_err(|source| NativeHeaderGridContextError::HeaderStart { system_id, source })?;

        systems.push(NativeHeaderGridSystem {
            system_id,
            header_system,
            staffs: staff_inputs,
        });
    }

    Ok(NativeHeaderGridContext {
        sheet_interline,
        systems,
    })
}

const INTRINSIC_RATIO: f64 = 0.8;
const MINIMUM_CLEF_GRADE: f64 = 0.03 / INTRINSIC_RATIO;
const MINIMUM_KEY_GRADE: f64 = 0.01 / INTRINSIC_RATIO;
const MAXIMUM_DELTA_PITCH_ONE: f64 = 0.5;
const MAXIMUM_DELTA_PITCH_FOUR: f64 = 2.0;
const CLEF_KEY_SOURCE_RATIO: f64 = 5.0;
const KEY_ALTERS_SOURCE_RATIO: f64 = 5.0;

/// Oracle-free result of composing the visual clef, key, and time columns.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderRecognition {
    pub sheet_interline: i32,
    /// Results in GRID/Java system order.
    pub systems: Vec<NativeHeaderSystemRecognition>,
    /// Rectangles consumed directly by the native beam spot chain.
    pub header_erases: Vec<NativeHeaderErase>,
}

impl NativeHeaderRecognition {
    /// Plain raster operations BEAMS consumes, preserving HEADERS' order.
    #[must_use]
    pub fn beam_erases(&self) -> Vec<HeaderErase> {
        self.header_erases.iter().map(|item| item.erase).collect()
    }
}

/// One HEADERS erase rectangle with explicit system ownership.
///
/// A tablature-only system can omit an erase, so vector position is not a
/// safe substitute for this identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHeaderErase {
    pub system_id: usize,
    pub erase: HeaderErase,
}

/// Selected header state and all retained candidate evidence for one system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderSystemRecognition {
    pub system_id: usize,
    /// Staff results in the system's source order.
    pub staffs: Vec<NativeHeaderStaffRecognition>,
    pub time_value: Option<NeutralTimeValue>,
}

/// The complete typed evidence left by the three Java-order column lifecycles.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeaderStaffRecognition {
    pub staff_id: usize,
    pub specific_interline: i32,
    pub header: Option<StaffHeader>,
    pub clef_candidates: Vec<NeutralClefCandidate>,
    pub selected_clef_id: Option<usize>,
    pub key_candidates: Vec<NeutralKeyCandidate>,
    pub selected_key_id: Option<usize>,
    pub time_candidates: Vec<NeutralTimeCandidate>,
    pub selected_time_id: Option<usize>,
}

/// A precise production failure. Missing geometry is never replaced by an ordinate of zero.
#[derive(Debug)]
pub enum NativeHeaderRecognitionError {
    GridContext(NativeHeaderGridContextError),
    SheetHeightOutOfRange(usize),
    MissingHeader {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffInput {
        system_id: usize,
        staff_id: usize,
    },
    MissingLineOrdinate {
        system_id: usize,
        staff_id: usize,
        x: i32,
        boundary: &'static str,
    },
    MissingSystemArea(usize),
    IdentifierOverflow {
        system_id: usize,
        stage: &'static str,
    },
    InvalidScaledParameter {
        system_id: usize,
        staff_id: usize,
        name: &'static str,
        value: i32,
    },
    ClefClassifier(ClefClassificationError),
    KeyClassifier(KeyClassificationError),
    TimeClassifier(TimeClassificationError),
    ClefColumn {
        system_id: usize,
        source: ClefColumnError<NativeClefError<ClefClassificationError>>,
    },
    KeyColumn {
        system_id: usize,
        source: KeyColumnError<NativeKeyError<KeyClassificationError>>,
    },
    TimeDiscovery {
        system_id: usize,
        staff_id: usize,
        source: NativeHeaderTimeError<TimeClassificationError>,
    },
    TimeColumn {
        system_id: usize,
        source: HeaderTimeColumnError<NativeHeaderTimeError<TimeClassificationError>>,
    },
}

impl fmt::Display for NativeHeaderRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GridContext(source) => write!(formatter, "HEADERS GRID context failed: {source}"),
            Self::SheetHeightOutOfRange(height) => {
                write!(formatter, "HEADERS sheet height {height} does not fit i32")
            }
            Self::MissingHeader {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADERS system {system_id} staff {staff_id} has no computed header"
            ),
            Self::MissingStaffInput {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "HEADERS system {system_id} staff {staff_id} has no GRID input"
            ),
            Self::MissingLineOrdinate {
                system_id,
                staff_id,
                x,
                boundary,
            } => write!(
                formatter,
                "HEADERS system {system_id} staff {staff_id} has no {boundary} line ordinate at x={x}"
            ),
            Self::MissingSystemArea(system_id) => {
                write!(formatter, "HEADERS system {system_id} has no GRID area")
            }
            Self::IdentifierOverflow { system_id, stage } => write!(
                formatter,
                "HEADERS system {system_id} cannot allocate the {stage} identifier range"
            ),
            Self::InvalidScaledParameter {
                system_id,
                staff_id,
                name,
                value,
            } => write!(
                formatter,
                "HEADERS system {system_id} staff {staff_id} has invalid {name} value {value}"
            ),
            Self::ClefClassifier(source) => {
                write!(
                    formatter,
                    "HEADERS clef classifier failed to load: {source}"
                )
            }
            Self::KeyClassifier(source) => {
                write!(formatter, "HEADERS key classifier failed to load: {source}")
            }
            Self::TimeClassifier(source) => {
                write!(
                    formatter,
                    "HEADERS time classifier failed to load: {source}"
                )
            }
            Self::ClefColumn { system_id, source } => {
                write!(
                    formatter,
                    "HEADERS system {system_id} clef column failed: {source}"
                )
            }
            Self::KeyColumn { system_id, source } => {
                write!(
                    formatter,
                    "HEADERS system {system_id} key column failed: {source}"
                )
            }
            Self::TimeDiscovery {
                system_id,
                staff_id,
                source,
            } => write!(
                formatter,
                "HEADERS system {system_id} staff {staff_id} time discovery failed: {source}"
            ),
            Self::TimeColumn { system_id, source } => {
                write!(
                    formatter,
                    "HEADERS system {system_id} time column failed: {source}"
                )
            }
        }
    }
}

impl Error for NativeHeaderRecognitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GridContext(source) => Some(source),
            Self::ClefClassifier(source) => Some(source),
            Self::KeyClassifier(source) => Some(source),
            Self::TimeClassifier(source) => Some(source),
            Self::ClefColumn { source, .. } => Some(source),
            Self::KeyColumn { source, .. } => Some(source),
            Self::TimeDiscovery { source, .. } => Some(source),
            Self::TimeColumn { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct GridOrdinates<'a>(&'a [StaffLineGeometry]);

impl StaffLineOrdinates for GridOrdinates<'_> {
    fn ordinates_at(&self, staff_id: usize, x: i32) -> Option<(i32, i32)> {
        let staff = self.0.iter().find(|staff| staff.staff_id == staff_id)?;
        Some((staff.first_line_y_at(x)?, staff.last_line_y_at(x)?))
    }
}

struct GridPitch<'a>(&'a [StaffLineGeometry]);

impl StaffPitchGeometry for GridPitch<'_> {
    fn line_span_at(&self, staff_id: usize, x: f64) -> Option<(f64, f64)> {
        let staff = self.0.iter().find(|staff| staff.staff_id == staff_id)?;
        Some((staff.first_line.y_at_x(x)?, staff.last_line.y_at_x(x)?))
    }
}

/// Run native HEADERS from live GRID state. No Java record is accepted as an input.
pub fn recognize_native_headers(
    grid: &GridLinesRecognition,
) -> Result<NativeHeaderRecognition, NativeHeaderRecognitionError> {
    let context = derive_native_header_grid_context(grid)
        .map_err(NativeHeaderRecognitionError::GridContext)?;
    let sheet_interline = context.sheet_interline;
    let sheet_parameters = SheetClefParameters::new(sheet_interline);
    let sheet_height = i32::try_from(grid.scale.height)
        .map_err(|_| NativeHeaderRecognitionError::SheetHeightOutOfRange(grid.scale.height))?;

    let mut clef_geometries = Vec::new();
    let mut clef_parameters = BTreeMap::new();
    for system_context in &context.systems {
        for staff_input in &system_context.staffs {
            let staff = header_staff(&system_context.header_system, staff_input.staff_id)?;
            let start = header_start(system_context.system_id, staff)?;
            let x_mid = start
                .wrapping_add(start)
                .wrapping_add(sheet_parameters.max_clef_end)
                / 2;
            let first = line_ordinate(system_context.system_id, staff_input, x_mid, true)?;
            let last = line_ordinate(system_context.system_id, staff_input, x_mid, false)?;
            clef_geometries.push(ClefLookupStaffGeometry {
                staff_id: staff_input.staff_id,
                browse_start: start,
                browse_stop: start.wrapping_add(sheet_parameters.max_clef_end),
                left_abscissa: staff_input.lines.left,
                right_abscissa: staff_input.lines.right,
                first_line_y: first,
                last_line_y: last,
                percussion_only: staff.one_line,
            });
            let parameters =
                crate::clef_parameters::StaffClefParameters::new(staff_input.specific_interline);
            clef_parameters.insert(
                staff_input.staff_id,
                NativeClefParameters {
                    staff_interline: staff_input.specific_interline,
                    first_line_y: f64::from(first),
                    last_line_y: f64::from(last),
                    min_part_weight: scaled_usize(
                        system_context.system_id,
                        staff_input.staff_id,
                        "minimum clef part weight",
                        parameters.min_part_weight,
                    )?,
                    max_part_count: crate::clef_parameters::max_part_count(),
                    max_part_gap: parameters.max_part_gap,
                    max_glyph_height: parameters.max_glyph_height,
                    min_glyph_weight: scaled_usize(
                        system_context.system_id,
                        staff_input.staff_id,
                        "minimum clef glyph weight",
                        parameters.min_glyph_weight,
                    )?,
                    max_eval_rank: crate::clef_parameters::max_eval_rank(),
                    minimum_classifier_grade: MINIMUM_CLEF_GRADE,
                    f_area_pitch_offset: 0.0,
                },
            );
        }
    }
    let clef_contexts = build_clef_lookup_contexts_at(
        &clef_geometries,
        ClefLookupParameters {
            sheet_height,
            above_staff: crate::clef_parameters::StaffClefParameters::new(sheet_interline)
                .above_staff,
            below_staff: crate::clef_parameters::StaffClefParameters::new(sheet_interline)
                .below_staff,
            belt_margin: sheet_parameters.belt_margin,
            x_core_margin: sheet_parameters.x_core_margin,
            y_core_margin: sheet_parameters.y_core_margin,
        },
        &GridOrdinates(&grid.staff_lines),
    );

    let sources = context
        .systems
        .iter()
        .map(|system| (system.system_id, grid.no_staff.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut system_results = Vec::with_capacity(context.systems.len());
    let mut header_erases = Vec::new();

    for system_context in context.systems {
        let system_id = system_context.system_id;
        let identifier_base = system_context.header_system.last_inter_id;
        let key_identifier_base = identifier_base.checked_add(100_000).ok_or(
            NativeHeaderRecognitionError::IdentifierOverflow {
                system_id,
                stage: "key",
            },
        )?;
        let time_identifier_base = identifier_base.checked_add(200_000).ok_or(
            NativeHeaderRecognitionError::IdentifierOverflow {
                system_id,
                stage: "time",
            },
        )?;
        let pair_identifier_base = identifier_base.checked_add(300_000).ok_or(
            NativeHeaderRecognitionError::IdentifierOverflow {
                system_id,
                stage: "time-pair",
            },
        )?;
        let mut system = system_context.header_system;
        let mut clef_column = HeadlessClefColumn::new(ClefLifecycleRecognizer::new(
            NativeClefProposalRecognizer::new(
                BundledClefClassifier::bundled()
                    .map_err(NativeHeaderRecognitionError::ClefClassifier)?,
                sources.clone(),
                clef_contexts.clone(),
                clef_parameters.clone(),
                identifier_base,
                identifier_base,
            ),
            clef_contexts.clone(),
            INTRINSIC_RATIO,
            0.0,
        ));
        let clef_offset = clef_column
            .retrieve_clefs(&mut system)
            .map_err(|source| NativeHeaderRecognitionError::ClefColumn { system_id, source })?;
        advance_header_stops(&mut system, clef_offset);

        let mut key_contexts = BTreeMap::new();
        let mut key_parameters = BTreeMap::new();
        let mut lifecycle_contexts = BTreeMap::new();
        for staff_input in &system_context.staffs {
            let staff = header_staff(&system, staff_input.staff_id)?;
            let header =
                staff
                    .header
                    .as_ref()
                    .ok_or(NativeHeaderRecognitionError::MissingHeader {
                        system_id,
                        staff_id: staff.id,
                    })?;
            let browse_start = header
                .clef_stop()
                .map_or(header.start, |stop| stop.wrapping_add(1));
            // `select_clefs` intentionally runs after key recognition because key support can
            // alter the clef winner. Its anchor glyph is already determined, however: the last
            // live clef candidate preceding the clef-range stop. Use that glyph's full ink edge
            // to keep its overhang out of key projection. Waiting for `header.clef` here would
            // always yield `None` and regress bass clefs whose glyph is wider than the symbol.
            let clef_ink_stop = clef_column.builders().get(&staff.id).and_then(|builder| {
                let range_stop = header.clef_range.as_ref()?.stop();
                builder
                    .candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| !candidate.removed && candidate.bounds.x < range_stop)
                    .max_by_key(|(index, candidate)| (candidate.bounds.x, *index))
                    .and_then(|(_, candidate)| candidate.glyph_bounds)
                    .map(HeaderBounds::right)
            });
            let first = line_ordinate(system_id, staff_input, browse_start, true)?;
            let last = line_ordinate(system_id, staff_input, browse_start, false)?;
            let (envelope_top, envelope_bottom) =
                browse_envelope(first, last, staff_input.specific_interline);
            key_contexts.insert(
                staff.id,
                NativeKeyStaffContext {
                    browse_stop: header
                        .start
                        .wrapping_add(max_header_width(sheet_interline))
                        .wrapping_sub(1),
                    clef_ink_stop,
                    envelope_top,
                    envelope_bottom,
                    staff_mid_y: f64::from(envelope_top + envelope_bottom) / 2.0,
                    classifier_interline: sheet_interline,
                    line_count: 5,
                    staff_interline: staff_input.specific_interline,
                },
            );
            let extractor = KeyExtractorParameters::new(staff_input.specific_interline);
            key_parameters.insert(
                staff.id,
                NativeKeyParameters {
                    minimum_component_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "minimum key component weight",
                        extractor.minimum_part_weight,
                    )?,
                    maximum_component_gap: extractor.maximum_part_gap,
                    minimum_glyph_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "minimum key glyph weight",
                        extractor.minimum_glyph_weight,
                    )?,
                    maximum_glyph_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "maximum key glyph weight",
                        extractor.maximum_glyph_weight,
                    )?,
                    minimum_alter_width: extractor.minimum_glyph_width,
                    minimum_alter_height: extractor.minimum_glyph_height,
                    maximum_alter_width: extractor.maximum_glyph_width,
                    maximum_alter_height: extractor.maximum_glyph_height,
                    maximum_alters: 7,
                    maximum_rank: max_eval_rank(),
                    minimum_classifier_grade: MINIMUM_KEY_GRADE,
                    pipeline: KeyPipelineParameters::new(
                        sheet_interline,
                        staff_input.specific_interline,
                    ),
                },
            );
            let clefs = clef_column
                .builders()
                .get(&staff.id)
                .map(|builder| {
                    builder
                        .candidates
                        .iter()
                        .map(|candidate| KeyClefSupport {
                            id: candidate.id,
                            kind: candidate.kind,
                            grade: candidate.grade,
                        })
                        .collect()
                })
                .unwrap_or_default();
            lifecycle_contexts.insert(
                staff.id,
                KeyLifecycleContext {
                    clefs,
                    maximum_delta_pitch_one: MAXIMUM_DELTA_PITCH_ONE,
                    maximum_delta_pitch_four: MAXIMUM_DELTA_PITCH_FOUR,
                    clef_key_source_ratio: CLEF_KEY_SOURCE_RATIO,
                    key_alters_source_ratio: KEY_ALTERS_SOURCE_RATIO,
                },
            );
        }
        let clef_supports = lifecycle_contexts
            .iter()
            .map(|(staff_id, context)| (*staff_id, context.clefs.clone()))
            .collect();
        let mut key_column = HeadlessKeyColumn::new(
            KeyLifecycleRecognizer::new(
                NativeKeyProposalRecognizer::new(
                    BundledKeyClassifier::bundled()
                        .map_err(NativeHeaderRecognitionError::KeyClassifier)?,
                    GridPitch(&grid.staff_lines),
                    sources.clone(),
                    key_contexts,
                    key_parameters,
                    key_identifier_base,
                    key_identifier_base,
                )
                .with_clef_supports(clef_supports),
                lifecycle_contexts,
            ),
            max_slice_distance(sheet_interline),
        );
        let key_offset = key_column
            .retrieve_keys(&mut system, max_header_width(sheet_interline))
            .map_err(|source| NativeHeaderRecognitionError::KeyColumn { system_id, source })?;
        advance_header_stops(&mut system, key_offset);

        let (time_value, time_candidates, selected_times, time_offset) = run_native_time_stage(
            grid,
            &sources,
            &system_context.staffs,
            &mut system,
            time_identifier_base,
            pair_identifier_base,
        )?;
        advance_header_stops(&mut system, time_offset);
        clef_column
            .select_clefs(&mut system)
            .map_err(|source| NativeHeaderRecognitionError::ClefColumn { system_id, source })?;

        let erase = build_header_erase(grid, &system_context.staffs, &system, sheet_interline)?;
        if let Some(erase) = erase {
            header_erases.push(NativeHeaderErase { system_id, erase });
        }

        let mut staffs = Vec::with_capacity(system.staffs.len());
        for staff in &system.staffs {
            let staff_input = system_context
                .staffs
                .iter()
                .find(|input| input.staff_id == staff.id)
                .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
                    system_id,
                    staff_id: staff.id,
                })?;
            let header = staff.header.clone();
            let selected_clef_id = header
                .as_ref()
                .and_then(|header| header.clef.as_ref())
                .map(|clef| clef.id);
            let selected_key_id = header
                .as_ref()
                .and_then(|header| header.key.as_ref())
                .map(|key| key.id);
            staffs.push(NativeHeaderStaffRecognition {
                staff_id: staff.id,
                specific_interline: staff_input.specific_interline,
                header,
                clef_candidates: clef_column
                    .builders()
                    .get(&staff.id)
                    .map(|builder| builder.candidates.clone())
                    .unwrap_or_default(),
                selected_clef_id,
                key_candidates: key_column
                    .builders()
                    .get(&staff.id)
                    .map(|builder| builder.candidates.clone())
                    .unwrap_or_default(),
                selected_key_id,
                time_candidates: time_candidates.get(&staff.id).cloned().unwrap_or_default(),
                selected_time_id: selected_times.get(&staff.id).copied().flatten(),
            });
        }
        system_results.push(NativeHeaderSystemRecognition {
            system_id,
            staffs,
            time_value,
        });
    }

    Ok(NativeHeaderRecognition {
        sheet_interline,
        systems: system_results,
        header_erases,
    })
}

fn header_staff(
    system: &HeadlessHeaderSystem,
    staff_id: usize,
) -> Result<&HeadlessHeaderStaff, NativeHeaderRecognitionError> {
    system
        .staffs
        .iter()
        .find(|staff| staff.id == staff_id)
        .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
            system_id: system.id,
            staff_id,
        })
}

fn header_start(
    system_id: usize,
    staff: &HeadlessHeaderStaff,
) -> Result<i32, NativeHeaderRecognitionError> {
    staff.header.as_ref().map(|header| header.start).ok_or(
        NativeHeaderRecognitionError::MissingHeader {
            system_id,
            staff_id: staff.id,
        },
    )
}

fn line_ordinate(
    system_id: usize,
    staff: &NativeHeaderGridStaff,
    x: i32,
    first: bool,
) -> Result<i32, NativeHeaderRecognitionError> {
    let ordinate = if first {
        staff.lines.first_line_y_at(x)
    } else {
        staff.lines.last_line_y_at(x)
    };
    ordinate.ok_or(NativeHeaderRecognitionError::MissingLineOrdinate {
        system_id,
        staff_id: staff.staff_id,
        x,
        boundary: if first { "first" } else { "last" },
    })
}

fn scaled_usize(
    system_id: usize,
    staff_id: usize,
    name: &'static str,
    value: i32,
) -> Result<usize, NativeHeaderRecognitionError> {
    usize::try_from(value).map_err(|_| NativeHeaderRecognitionError::InvalidScaledParameter {
        system_id,
        staff_id,
        name,
        value,
    })
}

fn advance_header_stops(system: &mut HeadlessHeaderSystem, offset: i32) {
    if offset > 0 {
        for staff in &mut system.staffs {
            if let Some(header) = staff.header.as_mut() {
                header.stop = header.start.wrapping_add(offset);
            }
        }
    }
}

fn fraction_int(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * value).round_ties_even() as i32
}

fn area_int(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * f64::from(interline) * value).round_ties_even() as i32
}

type NativeTimeCandidateMap = BTreeMap<usize, Vec<NeutralTimeCandidate>>;
type NativeTimeSelectionMap = BTreeMap<usize, Option<usize>>;

fn run_native_time_stage(
    grid: &GridLinesRecognition,
    sources: &BTreeMap<usize, RunTable>,
    staff_inputs: &[NativeHeaderGridStaff],
    system: &mut HeadlessHeaderSystem,
    identifier_base: usize,
    mut next_pair_id: usize,
) -> Result<
    (
        Option<NeutralTimeValue>,
        NativeTimeCandidateMap,
        NativeTimeSelectionMap,
        i32,
    ),
    NativeHeaderRecognitionError,
> {
    let system_id = system.id;
    let sheet_interline = grid.scale.scale.interline.main;
    let build_recognizer = || -> Result<_, NativeHeaderRecognitionError> {
        let mut raster_contexts = BTreeMap::new();
        let mut time_parameters = BTreeMap::new();
        for staff in &system.staffs {
            if staff.tablature {
                continue;
            }
            let input = staff_inputs
                .iter()
                .find(|input| input.staff_id == staff.id)
                .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
                    system_id,
                    staff_id: staff.id,
                })?;
            let header =
                staff
                    .header
                    .as_ref()
                    .ok_or(NativeHeaderRecognitionError::MissingHeader {
                        system_id,
                        staff_id: staff.id,
                    })?;
            let browse_start = header.stop;
            let mut stop = browse_start
                .wrapping_add(fraction_int(sheet_interline, 4.0))
                .wrapping_sub(1);
            for &bar_x in &input.good_connected_bar_starts {
                if bar_x > stop {
                    break;
                }
                if bar_x > browse_start {
                    stop = bar_x.wrapping_sub(1);
                    break;
                }
            }
            let top = line_ordinate(system_id, input, browse_start, true)?
                .min(line_ordinate(system_id, input, stop, true)?);
            // Java's implementation compares lastLine.yAt(stop) with itself; it does not read
            // the browse-start ordinate for the bottom edge.
            let bottom = line_ordinate(system_id, input, stop, false)?;
            raster_contexts.insert(
                staff.id,
                HeaderTimeRasterContext {
                    roi: HeaderBounds {
                        x: browse_start,
                        y: top,
                        width: stop.wrapping_sub(browse_start).wrapping_add(1),
                        height: bottom.wrapping_sub(top).wrapping_add(1),
                    },
                },
            );
            let staff_interline = input.specific_interline;
            time_parameters.insert(
                staff.id,
                NativeHeaderTimeParameters {
                    staff_interline,
                    maximum_first_space_width: fraction_int(sheet_interline, 2.5),
                    maximum_space_cumul: scaled_usize(
                        system_id,
                        staff.id,
                        "maximum time-space accumulation",
                        fraction_int(staff_interline, 0.4),
                    )?,
                    minimum_time_width: fraction_int(staff_interline, 1.0),
                    vertical_margin: fraction_int(staff_interline, 0.10),
                    minimum_part_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "minimum time part weight",
                        area_int(staff_interline, 0.01),
                    )?,
                    maximum_part_gap: f64::from(fraction_int(staff_interline, 1.0)),
                    maximum_time_width: fraction_int(staff_interline, 2.0),
                    minimum_whole_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "minimum whole-time weight",
                        area_int(staff_interline, 1.0),
                    )?,
                    minimum_half_weight: scaled_usize(
                        system_id,
                        staff.id,
                        "minimum half-time weight",
                        area_int(staff_interline, 0.75),
                    )?,
                    maximum_eval_rank: 3,
                    minimum_classifier_grade: 0.1 / INTRINSIC_RATIO,
                    intrinsic_ratio: INTRINSIC_RATIO,
                },
            );
        }
        Ok(NativeHeaderTimeRecognizer::new(
            BundledTimeClassifier::bundled()
                .map_err(NativeHeaderRecognitionError::TimeClassifier)?,
            sources.clone(),
            raster_contexts,
            time_parameters,
            identifier_base,
            identifier_base,
        ))
    };

    // Pair IDs depend on the concrete number proposals. Replay the exact TreeMap staff sequence
    // once, then run the real lifecycle with every permitted numerator/denominator crossing.
    let mut discovery = build_recognizer()?;
    let mut pair_ids_by_staff = BTreeMap::new();
    for staff in &system.staffs {
        if staff.tablature {
            continue;
        }
        let header = staff
            .header
            .as_ref()
            .ok_or(NativeHeaderRecognitionError::MissingHeader {
                system_id,
                staff_id: staff.id,
            })?;
        let input = crate::header_time_column::HeaderTimeRecognitionInput {
            system_id,
            staff_id: staff.id,
            browse_start: header.stop,
        };
        let mut range = crate::staff_header::StaffHeaderRange::default();
        range.browse_start = header.stop;
        let proposals = discovery
            .classify_time_parts(input, &range)
            .map_err(|source| NativeHeaderRecognitionError::TimeDiscovery {
                system_id,
                staff_id: staff.id,
                source,
            })?;
        let entry = pair_ids_by_staff
            .entry(staff.id)
            .or_insert_with(BTreeMap::new);
        for numerator in &proposals.numerators {
            for denominator in &proposals.denominators {
                entry.insert((numerator.id, denominator.id), next_pair_id);
                next_pair_id = next_pair_id.checked_add(1).ok_or(
                    NativeHeaderRecognitionError::IdentifierOverflow {
                        system_id,
                        stage: "time-pair",
                    },
                )?;
            }
        }
    }

    let mut lifecycle = BTreeMap::new();
    for staff in &system.staffs {
        if staff.tablature {
            continue;
        }
        let input = staff_inputs
            .iter()
            .find(|input| input.staff_id == staff.id)
            .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
                system_id,
                staff_id: staff.id,
            })?;
        lifecycle.insert(
            staff.id,
            HeaderTimeLifecycleContext {
                maximum_halves_dx: fraction_int(input.specific_interline, 1.0),
                top_bottom_source_ratio: 5.0,
                pair_ids: pair_ids_by_staff.remove(&staff.id).unwrap_or_default(),
                supported_values: vec![
                    (2, 2),
                    (3, 2),
                    (2, 4),
                    (3, 4),
                    (4, 4),
                    (5, 4),
                    (6, 4),
                    (1, 8),
                    (3, 8),
                    (6, 8),
                    (9, 8),
                    (12, 8),
                    (7, 8),
                ],
            },
        );
    }
    let mut column = HeadlessHeaderTimeColumn::new(HeaderTimeLifecycleRecognizer::new(
        build_recognizer()?,
        lifecycle,
    ));
    let offset = column
        .retrieve_time(system)
        .map_err(|source| NativeHeaderRecognitionError::TimeColumn { system_id, source })?;
    let candidates = column
        .builders()
        .iter()
        .map(|(staff_id, builder)| (*staff_id, builder.candidates.clone()))
        .collect();
    let selections = column
        .builders()
        .iter()
        .map(|(staff_id, builder)| (*staff_id, builder.selected_candidate_id))
        .collect();
    Ok((column.time_value(), candidates, selections, offset))
}

fn build_header_erase(
    grid: &GridLinesRecognition,
    inputs: &[NativeHeaderGridStaff],
    system: &HeadlessHeaderSystem,
    sheet_interline: i32,
) -> Result<Option<HeaderErase>, NativeHeaderRecognitionError> {
    let Some(stop) = system
        .staffs
        .iter()
        .filter(|staff| !staff.tablature)
        .find_map(|staff| staff.header.as_ref().map(|header| header.stop))
    else {
        return Ok(None);
    };
    let first = inputs
        .first()
        .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
            system_id: system.id,
            staff_id: 0,
        })?;
    let last = inputs
        .last()
        .ok_or(NativeHeaderRecognitionError::MissingStaffInput {
            system_id: system.id,
            staff_id: 0,
        })?;
    let top = line_ordinate(system.id, first, stop, true)?;
    let bottom = line_ordinate(system.id, last, stop, false)?;
    let x = grid
        .system_areas
        .iter()
        .find(|area| area.system_id == system.id)
        .map(|area| area.left)
        .ok_or(NativeHeaderRecognitionError::MissingSystemArea(system.id))?;
    let margin = fraction_int(sheet_interline, 2.0);
    Ok(Some(HeaderErase {
        x,
        stop,
        top: top.wrapping_sub(margin),
        bottom: bottom.wrapping_add(margin),
    }))
}

fn part_id_for_staff(
    system: &crate::grid_executor::HeadlessSystemSigState,
    staff_id: usize,
) -> usize {
    let Ok(staff_id_i32) = i32::try_from(staff_id) else {
        return staff_id;
    };
    system
        .bar_tail
        .parts
        .iter()
        .find(|part| part.first_staff_id <= staff_id_i32 && staff_id_i32 <= part.last_staff_id)
        .and_then(|part| usize::try_from(part.first_staff_id).ok())
        .unwrap_or(staff_id)
}

fn has_bar_connection(sig: &audiveris_image::grid_sig::GridSig, id: GridInterId) -> bool {
    sig.edges().iter().any(|edge| {
        (edge.source == id || edge.target == id)
            && matches!(edge.relation, GridSigRelation::BarConnectionSupport { .. })
    })
}

fn is_good_connected_barline(grade: f64, connected: bool) -> bool {
    grade >= GOOD_BARLINE_GRADE && connected
}

fn vertical_barline_bounds(
    median: audiveris_image::bars_logic::VerticalMedian,
    width: f64,
) -> HeaderBounds {
    let half_width = width / 2.0;
    let x = (median.x - half_width).floor() as i32;
    let right = (median.x + half_width).ceil() as i32;
    let y = median.top.floor() as i32;
    let bottom = median.bottom.ceil() as i32;
    HeaderBounds {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::recognize::recognize_grid_lines;

    fn repo_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative)
    }

    #[derive(Debug)]
    struct OracleStaffInput {
        id: usize,
        specific_interline: i32,
        header_start: i32,
        good_connected_bars: Vec<i32>,
    }

    fn oracle_header_inputs() -> Vec<(String, i32, Vec<OracleStaffInput>)> {
        let mut pages = Vec::<(String, i32, Vec<OracleStaffInput>)>::new();
        for line in include_str!("../../../oracle/clef-headers.txt").lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            match fields.first().copied() {
                Some("page") => pages.push((
                    fields[1].to_owned(),
                    fields[2].parse().expect("sheet interline"),
                    Vec::new(),
                )),
                Some("staff") => {
                    pages
                        .last_mut()
                        .expect("staff follows page")
                        .2
                        .push(OracleStaffInput {
                            id: fields[1].parse().expect("staff id"),
                            specific_interline: fields[2].parse().expect("staff interline"),
                            header_start: fields[3].parse().expect("header start"),
                            good_connected_bars: Vec::new(),
                        })
                }
                Some("bars") => {
                    let page = pages.last_mut().expect("bars follow page");
                    let id = fields[1].parse::<usize>().expect("staff id");
                    page.2
                        .iter_mut()
                        .find(|staff| staff.id == id)
                        .expect("bars name an existing staff")
                        .good_connected_bars = fields[2..]
                        .iter()
                        .filter_map(|value| value.parse().ok())
                        .collect();
                }
                _ => {}
            }
        }
        pages
    }

    #[test]
    fn good_barline_threshold_is_inclusive_and_connection_is_required() {
        assert!(!is_good_connected_barline(0.6 - f64::EPSILON, true));
        assert!(is_good_connected_barline(0.6, true));
        assert!(is_good_connected_barline(0.9, true));
        assert!(!is_good_connected_barline(0.9, false));
    }

    #[test]
    fn chula_header_inputs_are_grid_only_exact_and_deterministic() {
        let grid = recognize_grid_lines(repo_path("data/examples/chula.png"))
            .expect("chula GRID recognition");
        let one = derive_native_header_grid_context(&grid).expect("HEADERS grid context");
        let two = derive_native_header_grid_context(&grid).expect("repeat derivation");
        assert_eq!(one, two);
        assert_eq!(one.sheet_interline, 21);
        assert_eq!(one.systems.len(), 3);

        let starts = one
            .systems
            .iter()
            .flat_map(|system| &system.header_system.staffs)
            .map(|staff| {
                (
                    staff.id,
                    staff.header.as_ref().expect("computed header").start,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            starts,
            [(1, 203), (2, 202), (3, 89), (4, 87), (5, 84), (6, 83)]
        );

        let good_bars = one
            .systems
            .iter()
            .flat_map(|system| &system.staffs)
            .map(|staff| (staff.staff_id, staff.good_connected_bar_starts.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            good_bars,
            [
                (
                    1,
                    vec![200, 464, 831, 1173, 1544, 1803, 1812, 1828, 1961, 2324]
                ),
                (
                    2,
                    vec![199, 463, 831, 1173, 1544, 1803, 1812, 1827, 1961, 2323]
                ),
                (3, vec![85, 557, 984, 1281, 1291, 1451, 1901, 2323]),
                (4, vec![84, 556, 984, 1281, 1290, 1450, 1900, 2323]),
                (5, vec![81, 606, 976, 1343, 1667, 2032, 2310, 2322]),
                (6, vec![80, 606, 975, 1343, 1666, 2032, 2310, 2321]),
            ]
        );
        assert!(
            one.systems
                .iter()
                .flat_map(|system| &system.staffs)
                .all(|staff| staff.specific_interline == 21)
        );
    }

    #[test]
    fn grid_derives_every_corpus_header_input_the_oracle_recorded() {
        let pages = oracle_header_inputs();
        let mut checked = 0;
        assert_eq!(pages.len(), 9, "all example pages are pinned");

        for (name, sheet_interline, oracle_staffs) in pages {
            let grid = recognize_grid_lines(repo_path(&format!("data/examples/{name}")))
                .unwrap_or_else(|error| panic!("{name}: GRID failed: {error}"));
            let context = derive_native_header_grid_context(&grid)
                .unwrap_or_else(|error| panic!("{name}: HEADERS context failed: {error}"));
            assert_eq!(context.sheet_interline, sheet_interline, "{name}");

            for oracle in oracle_staffs {
                let system = context
                    .systems
                    .iter()
                    .find(|system| {
                        system
                            .header_system
                            .staffs
                            .iter()
                            .any(|staff| staff.id == oracle.id)
                    })
                    .unwrap_or_else(|| panic!("{name}: no system owns staff {}", oracle.id));
                let header_staff = system
                    .header_system
                    .staffs
                    .iter()
                    .find(|staff| staff.id == oracle.id)
                    .expect("the owning system contains the staff");
                let input = system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == oracle.id)
                    .expect("the production context contains the staff");
                assert_eq!(
                    header_staff.header.as_ref().map(|header| header.start),
                    Some(oracle.header_start),
                    "{name} staff {} header start",
                    oracle.id
                );
                assert_eq!(
                    input.specific_interline, oracle.specific_interline,
                    "{name} staff {} specific interline",
                    oracle.id
                );
                assert_eq!(
                    input.good_connected_bar_starts, oracle.good_connected_bars,
                    "{name} staff {} good connected bars",
                    oracle.id
                );
                checked += 1;
            }
        }

        assert_eq!(checked, 65, "every corpus staff was compared");
    }
}
