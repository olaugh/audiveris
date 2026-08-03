// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_classifier::{BasicClassifier, INPUT_SIZE};
use audiveris_core::{
    basic_line::BasicLine, grade, histogram::Histogram, injection_solver,
    integer_function::IntegerFunction, natural_spec, natural_spline::NaturalSpline,
    rational::Rational, step::OmrStep,
};
use audiveris_image::system_population::{
    BoundarySegment, PopulationPage, PopulationReferencePage, PopulationReferencePart,
    PopulationReferenceRegistry, PopulationReferenceStaff, PopulationSection,
    PopulationStaffConfig, PopulationSystem, PopulationSystemGeometry, PopulationSystemRefState,
    StaffBoundary, SystemSectionOwnership, SystemStaffBoundaries, allocate_population_pages,
};
use audiveris_image::{
    adaptive,
    bar_alignment::{AlignmentPeak, BarAlignment, BarAlignmentKind, BarImpacts},
    bar_alignments::{
        AlignmentBuildReport, AlignmentParameters, AlignmentStaff, find_all_alignments,
    },
    bar_column::{BarColumn, BarPeak, PeakId, PeakRelation, StaffId},
    bar_connections::{
        ConnectionBuildReport, ConnectionParameters, ConnectionRaster, find_connections,
    },
    bar_sticks::BarStick,
    bars_logic::{
        LocatedSectionId, PeakWidthClass, SectionLag, VerticalInterKind, VerticalInterPlan,
        VerticalMedian, aggregate_bar_chains, plan_connection_inters, start_column_candidate,
    },
    chamfer::ChamferDistance,
    cluster_coordinator::{RecursiveCombSnapshot, include_from_combs},
    cluster_expand::ClusterExpansionParameters,
    cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
    cluster_ownership::{ClusterOwnership, CombId},
    cluster_pipeline::ClusterRetrievalParameters,
    comb_builder::{CombFilament, popular_comb_size, retrieve_combs},
    filament::{FilamentError, StaffFilament},
    filament_comb::FilamentComb,
    filament_factory::{FilamentFactory, FilamentFactoryParams, OverlapParams},
    global_filter,
    grid_lifecycle::{GridBuildExecutor, GridBuildStage, GridStageFailure},
    grid_sig::{BarGroupPromotionError, BarTailResult, GridSig, GridSigNode, GridSigRelation},
    ingest,
    lag_rebuild::RegisteredHorizontalLag,
    line_cluster::{FilamentId, LineCluster},
    line_endpoints::{DefineEndPointsParameters, define_end_points},
    line_holes::{HoleInsertionSource, fill_holes_initial},
    line_short_sections::HorizontalSectionLag,
    lines_coordinator::{
        LinesCoordinatorParameters, StaffCandidateKind, retrieve_staff_candidates,
    },
    median,
    peak_graph::PeakGraph,
    prepared_completion::PreparedCompletionState,
    prepared_lines::{PreparedStaff, PreparedStaffLine},
    projection::{
        BraceSearchRequest, NeutralStaffProjectorRequest, PeakConstructionParams,
        PeakConstructionRequest, PeakCoreGeometry, PeakCoreParams, PeakCoreRejection,
        PeakRefinementParams, PeakRefinementRequest, PeakScanRequest, ProjectionPeakMode,
        ShortProjection, StaffProjectionRequest, check_lines_root_transition,
        refine_right_end_transition, select_blank,
    },
    raw_line_adapter::{RawPrimaryPassParameters, build_primary_cluster_pass},
    run_table::{Orientation, Run, RunTable, create_grid_run_tables, dispatch_grid_runs},
    scale_estimate::{ScaleOptions, estimate_scale},
    scale_runs::{VerticalRunHistograms, vertical_run_histograms},
    section::{Bounds, JunctionPolicy, Section, build_sections},
    staff_pattern::StaffPattern,
    staff_peak::{HorizontalSide, PeakPoint, StaffPeak, StaffPeakAttribute, StaffVerticalImpacts},
    target_line::TargetLine,
    watershed,
};
use audiveris_omr::grid_executor::{
    HeadlessBuildOtherError, HeadlessConnectionPlan, HeadlessGlyphRegistry, HeadlessGridBook,
    HeadlessGridExecutor, HeadlessGridPromotionError, HeadlessGridSheet, HeadlessGridSigState,
    HeadlessPopulationState, HeadlessSkew, HeadlessStaff, HeadlessStaffLine,
    HeadlessSystemSigState,
};
use audiveris_omr::score_update::{
    PageInput as ScorePageInput, PageKey as ScorePageKey, StubPages, create_scores, update_scores,
};
use audiveris_testkit::CanonicalVectors;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const EXPECTED_SUITES: u64 = 39;
const EXPECTED_TESTS: u64 = 212;
const EXPECTED_FAILURES: u64 = 0;
const EXPECTED_ERRORS: u64 = 0;
const EXPECTED_SKIPPED: u64 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct TestCounts {
    suites: u64,
    tests: u64,
    failures: u64,
    errors: u64,
    skipped: u64,
}

#[derive(Debug, Default)]
struct OutputBoundaryBuilder {
    stages: Vec<GridBuildStage>,
    finish_count: usize,
    warnings: Vec<GridBuildStage>,
}

impl GridBuildExecutor for OutputBoundaryBuilder {
    type StepError = &'static str;
    type OtherError = &'static str;

    fn run_stage(
        &mut self,
        stage: GridBuildStage,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.stages.push(stage);
        Ok(())
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, _error: &Self::OtherError) {
        self.warnings.push(stage);
    }

    fn finish(&mut self) {
        self.finish_count += 1;
    }
}

fn attribute(tag: &str, name: &str) -> Result<u64, Box<dyn Error>> {
    let marker = format!(r#"{name}=""#);
    let start = tag
        .find(&marker)
        .ok_or_else(|| format!("missing {name} attribute in {tag}"))?
        + marker.len();
    let tail = &tag[start..];
    let end = tail
        .find('"')
        .ok_or_else(|| format!("unterminated {name} attribute"))?;
    Ok(tail[..end].parse()?)
}

fn suite_counts(xml: &str) -> Result<TestCounts, Box<dyn Error>> {
    let start = xml
        .find("<testsuite ")
        .ok_or("JUnit XML has no testsuite element")?;
    let tail = &xml[start..];
    let end = tail.find('>').ok_or("unterminated testsuite element")?;
    let tag = &tail[..=end];
    Ok(TestCounts {
        suites: 1,
        tests: attribute(tag, "tests")?,
        failures: attribute(tag, "failures")?,
        errors: attribute(tag, "errors")?,
        skipped: attribute(tag, "skipped")?,
    })
}

fn reports(directory: &Path) -> Result<TestCounts, Box<dyn Error>> {
    let mut counts = TestCounts::default();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension() != Some(OsStr::new("xml")) {
            continue;
        }
        let suite = suite_counts(&fs::read_to_string(path)?)?;
        counts.suites += suite.suites;
        counts.tests += suite.tests;
        counts.failures += suite.failures;
        counts.errors += suite.errors;
        counts.skipped += suite.skipped;
    }
    Ok(counts)
}

fn expected() -> TestCounts {
    TestCounts {
        suites: EXPECTED_SUITES,
        tests: EXPECTED_TESTS,
        failures: EXPECTED_FAILURES,
        errors: EXPECTED_ERRORS,
        skipped: EXPECTED_SKIPPED,
    }
}

fn java_root(args: &[String]) -> Result<PathBuf, Box<dyn Error>> {
    let root = if let Some(index) = args.iter().position(|arg| arg == "--java-root") {
        args.get(index + 1)
            .ok_or("--java-root needs a path")?
            .into()
    } else if let Some(root) = env::var_os("AUDIVERIS_JAVA_ROOT") {
        root.into()
    } else {
        PathBuf::from("..")
    };
    Ok(fs::canonicalize(root)?)
}

fn run_java(root: &Path) -> Result<(), Box<dyn Error>> {
    let java_home = java_home(root);
    let status = Command::new(root.join("gradlew"))
        .args(["--no-daemon", ":app:test"])
        .current_dir(root)
        .env("JAVA_HOME", java_home)
        .status()?;
    if !status.success() {
        return Err(format!("Java baseline failed with {status}").into());
    }
    Ok(())
}

fn java_home(root: &Path) -> PathBuf {
    env::var_os("JAVA_HOME").map_or_else(
        || {
            root.parent()
                .unwrap_or_else(|| Path::new("."))
                .join("jdk25/Contents/Home")
        },
        PathBuf::from,
    )
}

fn java_vector_output(root: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new(root.join("gradlew"))
        .args([
            "--no-daemon",
            "-q",
            "-I",
            "rust/oracle/parity.init.gradle",
            ":app:rustParityProbe",
        ])
        .current_dir(root)
        .env("JAVA_HOME", java_home(root))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Java parity probe failed with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let canonical = stdout
        .lines()
        .filter(|line| VECTOR_KEYS.iter().any(|key| line.starts_with(key)))
        .collect::<Vec<_>>()
        .join("\n");
    if canonical.lines().count() != VECTOR_KEYS.len() {
        return Err(format!(
            "Java parity probe emitted {} canonical lines, expected {}. Full stdout:\n{stdout}",
            canonical.lines().count(),
            VECTOR_KEYS.len()
        )
        .into());
    }
    Ok(format!("{canonical}\n"))
}

fn baseline(args: &[String]) -> Result<(), Box<dyn Error>> {
    let root = java_root(args)?;
    if args.iter().any(|arg| arg == "--run-java") {
        run_java(&root)?;
    }
    let actual = reports(&root.join("app/build/test-results/test"))?;
    if actual != expected() {
        return Err(format!(
            "Java baseline mismatch: expected {:?}, got {actual:?}",
            expected()
        )
        .into());
    }
    println!(
        "Java oracle: {} suites, {} tests, {} failures, {} errors, {} skipped",
        actual.suites, actual.tests, actual.failures, actual.errors, actual.skipped
    );
    println!("Recognition pipeline parity: not implied by this unit baseline");
    Ok(())
}

const VECTOR_KEYS: [&str; 71] = [
    "natural.decode=",
    "natural.encode=",
    "rational.sum=",
    "rational.gcd=",
    "histogram.data=",
    "histogram.summary=",
    "line.origin=",
    "line.one-ten=",
    "grade.contextual=",
    "classifier.basic.synthetic=",
    "injection=",
    "integer.function=",
    "projection.short.synthetic=",
    "grid.staff-projector-threshold.synthetic=",
    "grid.staff-projector-blanks.synthetic=",
    "grid.staff-projector-peak-side.synthetic=",
    "grid.staff-projector-peak-candidate.synthetic=",
    "grid.staff-projector-core.synthetic=",
    "grid.staff-projector-ranges.synthetic=",
    "grid.staff-projector-brace.synthetic=",
    "grid.staff-projector-composed.synthetic=",
    "grid.staff-projector-lines-root.synthetic=",
    "grid.staff-projector-result-ops.synthetic=",
    "runs=",
    "grid.run-dispatch.synthetic=",
    "grid.sections.synthetic=",
    "grid.filament.synthetic=",
    "grid.filament-factory.synthetic=",
    "grid.filament-factory.overlap=",
    "grid.line-cluster.synthetic=",
    "grid.line-cluster-index.synthetic=",
    "grid.line-cluster-lifecycle.synthetic=",
    "grid.line-cluster-recursive.synthetic=",
    "grid.bar-column.synthetic=",
    "grid.bars-columns-start.synthetic=",
    "grid.combs.synthetic=",
    "grid.target-line.synthetic=",
    "grid.score-update.synthetic=",
    "grid.system-ref.synthetic=",
    "grid.skew.synthetic=",
    "grid.raw-lines.synthetic=",
    "grid.line-endpoints.synthetic=",
    "grid.line-holes.synthetic=",
    "grid.bar-alignments.synthetic=",
    "grid.bar-connections.synthetic=",
    "grid.output-boundary.synthetic=",
    "grid.contextualize.synthetic=",
    "spline.synthetic=",
    "image.threshold=",
    "image.median=",
    "image.chamfer=",
    "watershed.synthetic=",
    "image.runs=",
    "image.adaptive=",
    "staff-pattern.synthetic=",
    "load.chula=",
    "binary.chula=",
    "scale.vertical-runs=",
    "scale.chula=",
    "scale.chula.detail=",
    "grid.chula=",
    "grid.filament-factory.chula=",
    "scale.k545=",
    "scale.k545.detail=",
    "scale.essen=",
    "scale.essen.detail=",
    "scale.josquin=",
    "scale.josquin.detail=",
    "load.dichterliebe=",
    "binary.dichterliebe=",
    "pipeline=",
];

#[cfg(test)]
const ROOT_VECTOR_COUNT: usize = 15;

fn hash_u32(mut hash: u64, value: usize) -> u64 {
    for byte in u32::try_from(value)
        .expect("fixture coordinate fits u32")
        .to_be_bytes()
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn run_table_digest(table: &RunTable) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for sequence in 0..table.sequence_count() {
        for run in table.sequence(sequence).unwrap_or_default() {
            hash = hash_u32(hash, sequence);
            hash = hash_u32(hash, run.start);
            hash = hash_u32(hash, run.length);
        }
    }
    hash
}

fn grid_skew_vector() -> String {
    [0.5_f64, -0.5, 0.0]
        .into_iter()
        .map(|slope| {
            let skew = HeadlessSkew::new(slope, 100, 50);
            let input = PeakPoint::new(10.0, 20.0);
            let deskewed = skew.deskewed(input);
            let roundtrip = skew.skewed(deskewed);
            format!(
                "{slope:.1}:point:{:.12},{:.12};size:{:.12},{:.12};back:{:.12},{:.12}",
                deskewed.x,
                deskewed.y,
                skew.deskewed_width(),
                skew.deskewed_height(),
                roundtrip.x,
                roundtrip.y,
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn grid_raw_lines_vector() -> Result<String, Box<dyn Error>> {
    const WIDTH: usize = 320;
    const HEIGHT: usize = 61;
    const INTERLINE: usize = 10;
    // This is the exact endpoint slope of each uninterrupted stepped line.
    const EXPECTED_SLOPE: f64 = 4.0 / 319.0;

    let mut source = RunTable::new(Orientation::Vertical, WIDTH, HEIGHT)?;
    for x in 0..WIDTH {
        for base in [10, 20, 30, 40, 50] {
            if base == 30 && (130..150).contains(&x) {
                continue;
            }
            source.add_run(x, Run::new(base + (x / 64), 1))?;
        }
    }

    // Numeric production defaults resolved at interline=10, line thickness=1.
    let tables = create_grid_run_tables(&source, 1, 1.2, 2)?;
    let lag = HorizontalSectionLag::from_long_runs(tables.long_horizontal.clone())?;
    let expansion = ClusterExpansionParameters::new(EXPECTED_SLOPE, 60, 20, 20, 2.0, 5, 1)?;
    let compatibility = ClusterMergeParameters::new(EXPECTED_SLOPE, 60, 4.0, 60, 5, 1)?;
    let retrieval = ClusterRetrievalParameters::new(
        INTERLINE,
        BTreeSet::from([5]),
        expansion,
        ClusterMergePassParameters::new(compatibility, 100, 20)?,
        EXPECTED_SLOPE,
        0.5,
        0.2,
        10,
        60,
        None,
        20,
    )?;
    let built = build_primary_cluster_pass(
        &lag,
        RawPrimaryPassParameters {
            factory: FilamentFactoryParams {
                interline: INTERLINE,
                min_core_section_length: 5,
                min_section_aspect: 3.0,
                max_coord_gap: 17.0,
                max_pos_gap: 1.0,
                max_pos_gap_for_slope: 1.0,
                max_gap_slope: 0.5,
                min_length_for_delta_slope: 100.0,
                max_delta_slope: 0.01,
            },
            overlap: OverlapParams {
                probe_width: 5,
                max_overlap_delta_pos: 2.0,
                max_thickness: 2.0,
                max_overlap_space: 2.0,
                max_expansion_space: 0.0,
                max_involving_length: 20.0,
                max_consistent_ratio: 1.7,
            },
            sampling_dx: 10,
            minimum_delta_y: 10,
            maximum_delta_y: 10,
            retrieval,
        },
    )?;
    let slope = built.global_slope();
    let factory_count = built.factory_creation_ids().len();
    let root_count = built.root_order().len();
    let sloped_count = built.sloped_ids().len();
    let mut primary = built.into_state();
    let result = retrieve_staff_candidates(
        &mut primary,
        None,
        LinesCoordinatorParameters::new(slope, 300, Some(1), 0.001, 0.3, 50.0)?,
    )?;
    let discarded_count = result.primary().discarded_filaments().len();
    let staves = result
        .staffs()
        .iter()
        .map(|staff| -> Result<String, Box<dyn Error>> {
            let members = staff
                .line_ids()
                .iter()
                .map(|id| {
                    primary
                        .filaments()
                        .get(id)
                        .map(|filament| filament.sections().len().to_string())
                        .ok_or_else(|| format!("raw staff line {} is missing", id.value()))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(",");
            Ok(format!(
                "{}:{:?}:{:.0}-{:.0}:i{}:small{}:short{}:members{}",
                staff.id(),
                staff.kind(),
                staff.left(),
                staff.right(),
                staff.interline(),
                staff.is_small(),
                staff.is_short(),
                members,
            )
            .replace(":Standard:", ":standard:"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "boundary:retrieveLines;source:{WIDTH}x{HEIGHT}/{}/{};lag:{}/{}/{};short:{}/{};factory:{factory_count};roots:{root_count};slope:{slope:.12};rejects:sloped{sloped_count},discarded{discarded_count};staffs:{}/{};next:addShortSections,bars,completeLines:not-compared",
        source.total_run_count(),
        source.weight(),
        lag.sections().len(),
        lag.run_table().total_run_count(),
        lag.run_table().weight(),
        tables.short_horizontal.total_run_count(),
        tables.short_horizontal.weight(),
        staves.len(),
        staves.join("|"),
    ))
}

fn grid_line_endpoints_vector() -> Result<String, Box<dyn Error>> {
    const WIDTH: usize = 120;
    const HEIGHT: usize = 70;
    const INTERLINE: usize = 10;

    let mut binary = RunTable::new(Orientation::Horizontal, WIDTH, HEIGHT)?;
    for y in [11, 21, 31, 41, 51] {
        binary.add_run(y, Run::new(90, 10))?;
    }
    let lines = [10, 20, 30, 40, 50]
        .into_iter()
        .enumerate()
        .map(|(index, y)| -> Result<PreparedStaffLine, Box<dyn Error>> {
            let stop = if index == 0 { 89 } else { 100 };
            let mut table = RunTable::new(Orientation::Horizontal, WIDTH, HEIGHT)?;
            table.add_run(y, Run::new(10, stop - 9))?;
            let section = build_sections(&table, JunctionPolicy::All)
                .into_iter()
                .next()
                .ok_or("endpoint fixture line produced no section")?;
            let mut filament = StaffFilament::new(INTERLINE)?;
            filament.add_section(section)?;
            Ok(PreparedStaffLine {
                id: index + 10,
                cluster_position: index as i32,
                filament,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut state = PreparedCompletionState {
        staffs: vec![PreparedStaff {
            id: 1,
            kind: StaffCandidateKind::Standard,
            left: 10.0,
            right: 100.0,
            interline: INTERLINE,
            small: false,
            short: false,
            lines,
        }],
        global_slope: None,
        completion_systems: None,
        discarded_filaments: Vec::new(),
        horizontal_sections: Vec::new(),
        binary_buffer: Some(binary),
        thick_section_ids: Vec::new(),
        thin_section_ids: Vec::new(),
        defined_endpoints: Vec::new(),
        discarded_filament_steals: Vec::new(),
        discarded_filament_recomputations: Vec::new(),
        fill_hole_invocations: Vec::new(),
        section_inclusion_batches: Vec::new(),
        sticker_section_ids: Vec::new(),
        sticker_inclusion_batches: Vec::new(),
        crossing_line_inspections: Vec::new(),
        curvature_removals: Vec::new(),
        curvature_recomputations: Vec::new(),
        completed_stages: Vec::new(),
    };
    define_end_points(
        &mut state,
        DefineEndPointsParameters {
            scale_interline: INTERLINE as i32,
            foreground_thickness: 1,
            maximum_ending_dx: 10,
            pattern_width: 10,
            pattern_jitter: 2,
        },
    )?;

    let resolved = state
        .defined_endpoints
        .first()
        .ok_or("endpoint fixture produced no resolved staff")?;
    let (fit_x, fit_y, fit_offset, fit_ratio) = resolved
        .right_pattern_fit
        .ok_or("endpoint fixture did not use the right raster pattern")?;
    let recovered = resolved
        .lines
        .first()
        .ok_or("endpoint fixture has no recovered line")?;
    let close = resolved
        .lines
        .get(1)
        .ok_or("endpoint fixture has no close line")?;
    let binary = state
        .binary_buffer
        .as_ref()
        .ok_or("endpoint fixture lost its binary raster")?;
    let pixels = binary.to_pixels();
    let pattern = StaffPattern::new(5, 10, 1, INTERLINE as f64);
    let fit_at_zero = pattern.evaluate((90.0, 10.0), WIDTH, HEIGHT, &pixels);
    let fit_at_one = pattern.evaluate((90.0, 11.0), WIDTH, HEIGHT, &pixels);
    let fit_at_minus_one = pattern.evaluate((90.0, 9.0), WIDTH, HEIGHT, &pixels);
    let geometry = state.staffs[0].lines[0].filament.geometry()?;
    Ok(format!(
        "mean:{:.12};slope:{:.12};right-fit:{fit_x},{fit_y:.12},{fit_offset},{fit_ratio:.12}/probes:{fit_at_zero:.12},{fit_at_one:.12},{fit_at_minus_one:.12};close:l1@{:.12},{:.12};recovered:l0@{:.12},{:.12};geometry:{:.12},{:.12}>{:.12},{:.12};at95:{:.12}/{:.12}",
        resolved.mean_interline,
        resolved.right_ending_slope,
        close.right.x,
        close.right.y,
        recovered.right.x,
        recovered.right.y,
        geometry.start().0,
        geometry.start().1,
        geometry.stop().0,
        geometry.stop().1,
        geometry.position_at(95.0)?,
        geometry.slope_at(95.0)?,
    ))
}

fn grid_hole_line(
    id: usize,
    cluster_position: i32,
    y: usize,
    defining_points: Vec<(f64, f64)>,
) -> Result<PreparedStaffLine, Box<dyn Error>> {
    let mut table = RunTable::new(Orientation::Horizontal, 63, y + 2)?;
    table.add_run(y, Run::new(0, 63))?;
    let section = build_sections(&table, JunctionPolicy::All)
        .into_iter()
        .next()
        .ok_or("hole fixture line produced no section")?;
    let mut filament = StaffFilament::new(2)?;
    filament.add_section(section)?;
    filament.set_ending_points((0.0, y as f64), (62.0, y as f64))?;
    filament.replace_defining_points(defining_points)?;
    Ok(PreparedStaffLine {
        id,
        cluster_position,
        filament,
    })
}

fn grid_line_holes_vector() -> Result<String, Box<dyn Error>> {
    let far_xs = [0.0, 12.0, 24.0, 36.0, 44.0, 56.0, 62.0];
    let reference = |id, cluster_position, y: usize, xs: &[f64]| {
        grid_hole_line(
            id,
            cluster_position,
            y,
            xs.iter().map(|x| (*x, y as f64)).collect(),
        )
    };
    let initial = vec![(0.0, 20.0), (12.0, 20.0), (37.0, 20.0), (62.0, 20.0)];
    let lines = vec![
        reference(1, 0, 10, &far_xs)?,
        reference(2, 1, 15, &[0.0, 6.0, 18.0, 30.0, 42.0, 50.0, 62.0])?,
        grid_hole_line(3, 2, 20, initial.clone())?,
        reference(4, 5, 60, &far_xs)?,
        reference(5, 6, 70, &far_xs)?,
    ];
    let fallback50 = lines[2].filament.geometry()?.position_at(50.0)?;
    let mut state = PreparedCompletionState {
        staffs: vec![PreparedStaff {
            id: 9,
            kind: StaffCandidateKind::Standard,
            left: 0.0,
            right: 62.0,
            interline: 2,
            small: false,
            short: false,
            lines,
        }],
        global_slope: None,
        completion_systems: None,
        discarded_filaments: Vec::new(),
        horizontal_sections: Vec::new(),
        binary_buffer: None,
        thick_section_ids: Vec::new(),
        thin_section_ids: Vec::new(),
        defined_endpoints: Vec::new(),
        fill_hole_invocations: Vec::new(),
        discarded_filament_steals: Vec::new(),
        discarded_filament_recomputations: Vec::new(),
        section_inclusion_batches: Vec::new(),
        sticker_section_ids: Vec::new(),
        sticker_inclusion_batches: Vec::new(),
        crossing_line_inspections: Vec::new(),
        curvature_removals: Vec::new(),
        curvature_recomputations: Vec::new(),
        completed_stages: Vec::new(),
    };
    fill_holes_initial(&mut state)?;

    let point12 = |point: (f64, f64)| format!("{:.12},{:.12}", point.0, point.1);
    let points6 = |points: &[(f64, f64)]| {
        points
            .iter()
            .map(|point| format!("{:.6},{:.6}", point.0, point.1))
            .collect::<Vec<_>>()
            .join(";")
    };
    let insertions = state
        .fill_hole_invocations
        .first()
        .ok_or("hole fixture produced no invocation audit")?
        .insertions
        .iter()
        .map(|insertion| {
            let source = match insertion.source {
                HoleInsertionSource::NeighborInterpolation => "neighbor",
                HoleInsertionSource::CurrentSplineFallback => "fallback",
            };
            format!("{}@{source}", point12(insertion.point))
        })
        .collect::<Vec<_>>()
        .join("|");
    let geometry = state.staffs[0].lines[2].filament.geometry()?;
    Ok(format!(
        "boundary:fillHoles;limits:12,10,5;initial:{};gaps:12->0,25@12->24,25@37->50;refs:24=A0/B5@r.4,50=N1/none;insert:{insertions};fallback50:{fallback50:.12};points:{};sample31:{:.12}/{:.12}",
        points6(&initial),
        points6(geometry.points()),
        geometry.position_at(31.0)?,
        geometry.slope_at(31.0)?,
    ))
}

fn grid_bar_alignments_vector() -> Result<String, Box<dyn Error>> {
    const SHEET_SLOPE: f64 = -0.02;
    const MAXIMUM_SLOPE: f64 = 0.06;
    const MAXIMUM_DELTA_WIDTH: i32 = 6;

    let make_peak = |staff_id, top, bottom, start, stop| -> Result<StaffPeak, Box<dyn Error>> {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff_id),
            top,
            bottom,
            start,
            stop,
            StaffVerticalImpacts::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )?;
        let skew = HeadlessSkew::new(SHEET_SLOPE, 120, 130);
        peak.compute_deskewed_center(|point| skew.deskewed(point))?;
        Ok(peak)
    };
    let top_left = make_peak(1, 10, 50, 20, 22)?;
    let top_right = make_peak(1, 10, 50, 80, 81)?;
    let left_first = make_peak(2, 70, 110, 21, 21)?;
    let left_second = make_peak(2, 70, 110, 20, 21)?;
    let right = make_peak(3, 70, 110, 80, 83)?;
    let staffs = [
        AlignmentStaff {
            staff_id: StaffId::new(1),
            left: 0.0,
            right: 100.0,
            top: 10.0,
            bottom: 50.0,
            short: false,
            peaks: vec![top_left.key(), top_right.key()],
        },
        AlignmentStaff {
            staff_id: StaffId::new(2),
            left: 0.0,
            right: 45.0,
            top: 70.0,
            bottom: 110.0,
            short: false,
            peaks: vec![left_first.key(), left_second.key()],
        },
        AlignmentStaff {
            staff_id: StaffId::new(3),
            left: 55.0,
            right: 100.0,
            top: 70.0,
            bottom: 110.0,
            short: false,
            peaks: vec![right.key()],
        },
    ];
    let parameters = AlignmentParameters {
        sheet_slope: SHEET_SLOPE,
        maximum_alignment_slope: MAXIMUM_SLOPE,
        maximum_alignment_delta_width: MAXIMUM_DELTA_WIDTH,
    };
    let mut graph: PeakGraph<BarAlignment> = PeakGraph::new();
    for peak in [
        top_left.clone(),
        top_right.clone(),
        left_first.clone(),
        left_second.clone(),
        right.clone(),
    ] {
        if !graph.add_vertex(peak) {
            return Err("alignment fixture peak collided".into());
        }
    }
    let mut report = AlignmentBuildReport::default();
    find_all_alignments(&mut graph, &staffs, parameters, &mut report)?;

    let name = |key| -> Result<&'static str, Box<dyn Error>> {
        if key == top_left.key() {
            Ok("tl")
        } else if key == top_right.key() {
            Ok("tr")
        } else if key == left_first.key() {
            Ok("a")
        } else if key == left_second.key() {
            Ok("b")
        } else if key == right.key() {
            Ok("c")
        } else {
            Err("alignment relation has an unknown peak".into())
        }
    };
    let relations = graph
        .edges()
        .iter()
        .map(|edge| -> Result<String, Box<dyn Error>> {
            let alignment = edge.relation();
            if alignment.kind() != BarAlignmentKind::Alignment {
                return Err("find_all_alignments promoted a connection".into());
            }
            Ok(format!(
                "{}>{}@s{:.12}/dw{:.12}/i{:.12},{:.12}/g{:.12}",
                name(edge.source())?,
                name(edge.target())?,
                alignment.slope(),
                alignment.delta_width(),
                alignment.impacts().align(),
                alignment.impacts().width(),
                alignment.grade(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let competitors = graph.outgoing_edges(top_left.key())?.count();
    if relations.len() != 3 || competitors != 2 || report.edge_ids().len() != 3 {
        return Err("unexpected find_all_alignments fixture cardinality".into());
    }
    if graph
        .edges()
        .iter()
        .map(|edge| edge.id())
        .ne(report.edge_ids().iter().copied())
    {
        return Err("alignment report lost relation insertion order".into());
    }

    let y_top = f64::from(top_left.bottom());
    let y_bottom = f64::from(left_first.top());
    let raw_left = f64::from(left_first.start() - top_left.start()) / (y_bottom - y_top);
    let raw_right = f64::from(left_first.stop() - top_left.stop()) / (y_bottom - y_top);
    Ok(format!(
        "boundary:findAllAlignments;neighbors:2,3;sheet:{SHEET_SLOPE:.12}/vert:{:.12};limits:{MAXIMUM_SLOPE:.12},{MAXIMUM_DELTA_WIDTH};raw:tl>a={raw_left:.12},{raw_right:.12};relations:{};competitors:tl={competitors};next:findConnections,purgeAlignments:not-run",
        -SHEET_SLOPE,
        relations.join("|"),
    ))
}

fn grid_bar_connections_vector() -> Result<String, Box<dyn Error>> {
    const WIDTH: usize = 60;
    const HEIGHT: usize = 70;
    const MAXIMUM_GAP: i32 = 10;
    const MAXIMUM_WHITE_RATIO: f64 = 0.25;
    const UNUSED_MINIMUM_GRADE: f64 = 0.5;

    let mut pixels = vec![255_u8; WIDTH * HEIGHT];
    let mut set_foreground = |x: usize, y: usize| {
        pixels[(y * WIDTH) + x] = 0;
    };
    for y in [10, 22, 23, 24, 25] {
        set_foreground(5, y);
    }
    let mut corridor_samples = Vec::new();
    for y in 10..=19 {
        let ratio = (y - 10) as f64 / 9.0;
        let left = (20.0 + (4.0 * ratio)).floor() as usize;
        let right = (21.0 + (5.0 * ratio)).ceil() as usize;
        let ink = if (y - 10) % 2 == 0 { left } else { right };
        set_foreground(ink, y);
        if [11, 14, 18].contains(&y) {
            corridor_samples.push(format!("{y}={left}..{right}@{ink}"));
        }
    }
    for y in 10..=49 {
        if !(11..=20).contains(&y) {
            set_foreground(40, y);
        }
    }

    let make_peak = |staff_id, top, bottom, start, stop| -> Result<StaffPeak, Box<dyn Error>> {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff_id),
            top,
            bottom,
            start,
            stop,
            StaffVerticalImpacts::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )?;
        peak.compute_deskewed_center(|point| point)?;
        Ok(peak)
    };
    let rejected_top = make_peak(1, 1, 10, 5, 6)?;
    let rejected_bottom = make_peak(2, 25, 34, 5, 6)?;
    let ordinary_top = make_peak(3, 1, 10, 20, 21)?;
    let ordinary_bottom = make_peak(4, 19, 28, 24, 26)?;
    let threshold_top = make_peak(5, 1, 10, 40, 41)?;
    let threshold_bottom = make_peak(6, 49, 58, 40, 41)?;

    let relation = |top: &StaffPeak,
                    bottom: &StaffPeak,
                    top_id: usize,
                    bottom_id: usize|
     -> Result<BarAlignment, Box<dyn Error>> {
        let alignment_peak = |peak: &StaffPeak, id| {
            AlignmentPeak::with_geometry(
                PeakId::new(id),
                peak.staff_id(),
                peak.start(),
                usize::try_from(peak.width()).expect("fixture peak width is positive"),
                peak.top(),
                peak.bottom(),
                peak.impacts().expect("fixture peak has impacts").grade(),
            )
        };
        Ok(BarAlignment::new(
            alignment_peak(top, top_id)?,
            alignment_peak(bottom, bottom_id)?,
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0)?,
        )?)
    };
    let mut graph: PeakGraph<BarAlignment> = PeakGraph::new();
    for peak in [
        rejected_top.clone(),
        rejected_bottom.clone(),
        ordinary_top.clone(),
        ordinary_bottom.clone(),
        threshold_top.clone(),
        threshold_bottom.clone(),
    ] {
        if !graph.add_vertex(peak) {
            return Err("connection fixture peak collided".into());
        }
    }
    let rejected_id = graph.add_edge(
        rejected_top.key(),
        rejected_bottom.key(),
        relation(&rejected_top, &rejected_bottom, 1, 2)?,
    )?;
    let ordinary_id = graph.add_edge(
        ordinary_top.key(),
        ordinary_bottom.key(),
        relation(&ordinary_top, &ordinary_bottom, 3, 4)?,
    )?;
    let threshold_id = graph.add_edge(
        threshold_top.key(),
        threshold_bottom.key(),
        relation(&threshold_top, &threshold_bottom, 5, 6)?,
    )?;

    let make_stick = |id: usize, peak: &StaffPeak| BarStick {
        id,
        peak: peak.key(),
        members: vec![LocatedSectionId {
            lag: SectionLag::Vertical,
            id,
        }],
        bounds: Bounds {
            x: usize::try_from(peak.start()).expect("fixture peak x is nonnegative"),
            y: usize::try_from(peak.top()).expect("fixture peak y is nonnegative"),
            width: usize::try_from(peak.width()).expect("fixture peak width is positive"),
            height: usize::try_from(peak.bottom() - peak.top() + 1)
                .expect("fixture peak height is positive"),
        },
        points: vec![
            PeakPoint::new(f64::from(peak.start()), f64::from(peak.top())),
            PeakPoint::new(f64::from(peak.stop()), f64::from(peak.bottom())),
        ],
        mean_curvature: f64::INFINITY,
        marked_brace: false,
    };
    let sticks = [
        make_stick(101, &rejected_top),
        make_stick(102, &rejected_bottom),
        make_stick(103, &ordinary_top),
        make_stick(104, &ordinary_bottom),
        make_stick(105, &threshold_top),
        make_stick(106, &threshold_bottom),
    ];
    let mut report = ConnectionBuildReport::default();
    find_connections(
        &mut graph,
        ConnectionRaster {
            width: WIDTH,
            height: HEIGHT,
            pixels: &pixels,
        },
        &sticks,
        ConnectionParameters {
            maximum_gap: MAXIMUM_GAP,
            maximum_white_ratio: MAXIMUM_WHITE_RATIO,
        },
        &mut report,
    )?;

    if [
        rejected_id.value(),
        ordinary_id.value(),
        threshold_id.value(),
    ] != [1, 2, 3]
        || report.decisions().len() != 3
        || report.promoted_count() != 2
    {
        return Err("unexpected find_connections initial identity or cardinality".into());
    }
    let label = |key| -> Result<&'static str, Box<dyn Error>> {
        if key == rejected_top.key() {
            Ok("r")
        } else if key == ordinary_top.key() {
            Ok("o")
        } else if key == threshold_top.key() {
            Ok("t")
        } else {
            Err("connection edge has an unknown source".into())
        }
    };
    let decisions = report
        .decisions()
        .iter()
        .map(|decision| -> Result<String, Box<dyn Error>> {
            Ok(match decision.promoted_edge {
                Some(promoted) => format!(
                    "{}#{}->#{}",
                    label(decision.source)?,
                    decision.alignment_edge.value(),
                    promoted.value()
                ),
                None => format!(
                    "{}#{}->reject",
                    label(decision.source)?,
                    decision.alignment_edge.value()
                ),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let core = report
        .decisions()
        .iter()
        .map(|decision| -> Result<String, Box<dyn Error>> {
            Ok(format!(
                "{}={}/{}/{:.12}",
                label(decision.source)?,
                decision.core.length,
                decision.core.gap,
                decision.core.white_ratio,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let final_relations = graph
        .edges()
        .iter()
        .map(|edge| -> Result<String, Box<dyn Error>> {
            let kind = match edge.relation().kind() {
                BarAlignmentKind::Alignment => "A",
                BarAlignmentKind::Connection => "C",
            };
            Ok(format!(
                "{}#{}:{kind}@g{:.12}",
                label(edge.source())?,
                edge.id().value(),
                edge.relation().grade(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let threshold_final = graph
        .edge_between(threshold_top.key(), threshold_bottom.key())
        .ok_or("threshold connection is absent")?
        .relation();
    if decisions != ["r#1->reject", "o#2->#4", "t#3->#5"]
        || final_relations
            != [
                "r#1:A@g0.800000000000",
                "o#4:C@g0.800000000000",
                "t#5:C@g0.000000000000",
            ]
    {
        return Err("find_connections replacement order or identity drifted".into());
    }
    Ok(format!(
        "boundary:findConnections;limits:gap{MAXIMUM_GAP},white{MAXIMUM_WHITE_RATIO:.12},minGrade{UNUSED_MINIMUM_GRADE:.12};corridor:{};core:{};initial:r#1,o#2,t#3;decisions:{};final:{};promoted:{};zeroBelowMin:{};next:splitMergedGroups,purgeAlignments:not-run",
        corridor_samples.join(","),
        core.join(","),
        decisions.join(","),
        final_relations.join(","),
        report.promoted_count(),
        threshold_final.grade() < UNUSED_MINIMUM_GRADE,
    ))
}

fn output_boundary_peak(staff_id: usize, top: i32, bottom: i32, x: i32) -> StaffPeak {
    StaffPeak::new(StaffId::new(staff_id), top, bottom, x, x)
        .expect("output-boundary peak geometry is valid")
}

fn output_boundary_vertical_plan(peak: &StaffPeak) -> VerticalInterPlan {
    VerticalInterPlan {
        peak: peak.key(),
        median: VerticalMedian {
            x: f64::from(peak.start()) + 0.5,
            top: f64::from(peak.top()),
            bottom: f64::from(peak.bottom()) + 1.0,
        },
        width: 1.0,
        impacts: None,
        kind: VerticalInterKind::Barline {
            width_class: PeakWidthClass::Thin,
            left_staff_end: false,
            right_staff_end: false,
        },
    }
}

fn output_boundary_staff_digest(staffs: &[HeadlessStaff]) -> Result<u64, Box<dyn Error>> {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for staff in staffs {
        for (ordinal, staff_line) in staff.lines.iter().enumerate() {
            let HeadlessStaffLine::Persistent { line, .. } = staff_line else {
                return Err("staff line was not simplified".into());
            };
            hash = hash_u32(hash, staff.id);
            hash = hash_u32(hash, ordinal);
            hash = hash_u32(hash, line.glyph.x);
            hash = hash_u32(hash, line.glyph.y);
            hash = hash_u32(hash, line.glyph.runs.width());
            hash = hash_u32(hash, line.glyph.runs.height());
            hash = hash_u32(hash, line.glyph.runs.weight());
            hash = hash_u32(
                hash,
                usize::from(line.glyph.runs.orientation() == Orientation::Vertical),
            );
            for sequence in 0..line.glyph.runs.sequence_count() {
                for run in line.glyph.runs.sequence(sequence).unwrap_or_default() {
                    hash = hash_u32(hash, sequence);
                    hash = hash_u32(hash, run.start);
                    hash = hash_u32(hash, run.length);
                }
            }
            hash = hash_u32(hash, line.points.len());
            for &(x, y) in &line.points {
                hash = hash_u32(hash, (x * 1_000_000.0).round() as usize);
                hash = hash_u32(hash, (y * 1_000_000.0).round() as usize);
            }
            hash = hash_u32(hash, (line.thickness * 1_000_000.0).round() as usize);
        }
    }
    Ok(hash)
}

fn output_boundary_vector() -> Result<String, Box<dyn Error>> {
    const WIDTH: usize = 120;
    const HEIGHT: usize = 280;
    const INTERLINE: usize = 10;
    let line_ys = [
        20, 30, 40, 50, 60, 90, 100, 110, 120, 130, 190, 200, 210, 220, 230,
    ];
    let mut staff_table = RunTable::new(Orientation::Horizontal, WIDTH, HEIGHT)?;
    for y in line_ys {
        staff_table.add_run(y, Run::new(10, 100))?;
    }
    let lag = HorizontalSectionLag::from_long_runs(staff_table)?;
    if lag.sections().len() != 15 {
        return Err("staff fixture did not create fifteen isolated sections".into());
    }
    let mut staffs = Vec::new();
    for staff_index in 0..3 {
        let mut lines = Vec::new();
        for line_index in 0..5 {
            let source_index = (staff_index * 5) + line_index;
            let mut filament = StaffFilament::new(INTERLINE)?;
            filament.add_section(lag.sections()[source_index].clone())?;
            lines.push(HeadlessStaffLine::Filament {
                line_id: source_index + 1,
                filament,
            });
        }
        staffs.push(HeadlessStaff {
            id: staff_index + 1,
            kind: StaffCandidateKind::Standard,
            left: 10.0,
            right: 109.0,
            interline: INTERLINE,
            small: false,
            short: false,
            barlines: Vec::new(),
            lines,
        });
    }

    let s1a = output_boundary_peak(1, 20, 60, 10);
    let s1b = output_boundary_peak(1, 20, 60, 12);
    let missing = output_boundary_peak(1, 20, 60, 14);
    let s2a = output_boundary_peak(2, 90, 130, 10);
    let s2b = output_boundary_peak(2, 90, 130, 12);
    let s3a = output_boundary_peak(3, 190, 230, 20);

    let mut peak_graph = PeakGraph::new();
    for peak in [&s1a, &s1b, &s2a, &s2b, &s3a] {
        peak_graph.add_vertex(peak.clone());
    }
    let alignment = BarAlignment::new(
        AlignmentPeak::new(PeakId::new(1), s1a.key().staff_id(), 10, 1.0)?,
        AlignmentPeak::new(PeakId::new(2), s2a.key().staff_id(), 10, 1.0)?,
        0.0,
        0.0,
        BarImpacts::alignment(1.0, 1.0)?,
    )?;
    peak_graph.add_edge(
        s1a.key(),
        s2a.key(),
        BarAlignment::connection(&alignment, 1.0, 1.0)?,
    )?;
    let connection_plans = plan_connection_inters(&peak_graph, |_| true);
    if connection_plans.len() != 1 {
        return Err("connection fixture did not create exactly one plan".into());
    }

    let staff_config = PopulationStaffConfig {
        line_count: 5,
        is_small: false,
    };
    let part = |part_id: usize, staff_ids: &[usize]| PopulationReferencePart {
        part_id,
        staves: staff_ids
            .iter()
            .copied()
            .map(|staff_id| PopulationReferenceStaff {
                staff_id,
                config: staff_config,
            })
            .collect(),
    };
    let boundary = |y: f64| StaffBoundary {
        segments: vec![BoundarySegment::Line {
            start: (10.0, y),
            end: (109.0, y),
        }],
    };
    let population = HeadlessPopulationState {
        sheet_width: WIDTH as i32,
        sheet_height: HEIGHT as i32,
        vertical_margin: 1,
        minimum_indentation: 4.0,
        geometries: vec![
            PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: WIDTH as i32,
                top: 20,
                bottom: 130,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            },
            PopulationSystemGeometry {
                system_id: 2,
                left: 10,
                width: 110,
                top: 190,
                bottom: 230,
                area_left: 0,
                deskewed_upper_left_x: 10.0,
            },
        ],
        staff_boundaries: vec![
            SystemStaffBoundaries {
                first_line: boundary(20.5),
                last_line: boundary(130.5),
            },
            SystemStaffBoundaries {
                first_line: boundary(190.5),
                last_line: boundary(230.5),
            },
        ],
        vertical_sections: Vec::<PopulationSection>::new(),
        section_ownership: vec![
            SystemSectionOwnership {
                system_id: 1,
                horizontal_sections: Vec::new(),
                vertical_sections: Vec::new(),
            },
            SystemSectionOwnership {
                system_id: 2,
                horizontal_sections: Vec::new(),
                vertical_sections: Vec::new(),
            },
        ],
        systems: vec![
            PopulationSystem {
                id: 1,
                indented: false,
                parts: vec![part(1, &[1, 2])],
                system_ref: PopulationSystemRefState::default(),
                page_id: None,
            },
            PopulationSystem {
                id: 2,
                indented: true,
                parts: vec![part(2, &[3])],
                system_ref: PopulationSystemRefState::default(),
                page_id: None,
            },
        ],
        areas: Vec::new(),
        staff_areas_computed: Vec::new(),
        pages: Vec::new(),
        page_refs: Vec::new(),
        references: PopulationReferenceRegistry::default(),
        reports: Vec::new(),
    };

    let mut executor = HeadlessGridExecutor::new(
        OutputBoundaryBuilder::default(),
        HeadlessGridSheet {
            sheet_number: 1,
            staffs,
            glyphs: HeadlessGlyphRegistry::default(),
            sig: HeadlessGridSigState {
                systems: vec![
                    HeadlessSystemSigState {
                        system_id: 1,
                        sig: GridSig::default(),
                        vertical_plans: [&s1a, &s1b, &s2a, &s2b]
                            .map(output_boundary_vertical_plan)
                            .to_vec(),
                        staff_peaks: vec![
                            vec![s1a.clone(), s1b.clone(), missing.clone()],
                            vec![s2a.clone(), s2b.clone()],
                        ],
                        brace_peaks: vec![None, None],
                        maximum_group_gap: 3,
                        interline: INTERLINE as f64,
                        bar_tail: BarTailResult::default(),
                    },
                    HeadlessSystemSigState {
                        system_id: 2,
                        sig: GridSig::default(),
                        vertical_plans: vec![output_boundary_vertical_plan(&s3a)],
                        staff_peaks: vec![vec![s3a.clone()]],
                        brace_peaks: vec![None],
                        maximum_group_gap: 3,
                        interline: INTERLINE as f64,
                        bar_tail: BarTailResult::default(),
                    },
                ],
                peak_graph,
                connections: vec![HeadlessConnectionPlan {
                    system_id: 1,
                    plan: connection_plans[0],
                }],
                connection_warnings: Vec::new(),
            },
            promotion_failure: None,
            no_staff_table: None,
            max_fore: Some(3),
            ledger_thickness: 1.0,
            vertical_lag: None,
            horizontal_lag: Some(RegisteredHorizontalLag::Populated(lag)),
            installed_raster_prefix: None,
            skew: None,
            sloped_line_fallbacks: Vec::new(),
            population,
        },
        HeadlessGridBook {
            stubs: vec![StubPages {
                number: 1,
                valid_selected: true,
                pages: Vec::new(),
            }],
            scores: Vec::new(),
        },
    );
    executor
        .run()
        .map_err(|error| format!("GRID output-boundary executor failed: {error:?}"))?;

    let expected_failure = HeadlessGridPromotionError::BarGroup {
        system_id: 1,
        source: BarGroupPromotionError::MissingInter(missing.key()),
    };
    if executor.sheet.promotion_failure != Some(expected_failure)
        || executor.builder.stages.last() != Some(&GridBuildStage::ProcessBars)
        || executor.builder.finish_count != 1
        || !executor.cleaner_finished
        || !executor.step_finished
    {
        return Err("GRID output-boundary lifecycle did not reach the expected state".into());
    }
    if !matches!(
        executor.build_outcome,
        Some(audiveris_image::grid_lifecycle::GridBuildOutcome::Swallowed {
            stage: GridBuildStage::ProcessBars,
            error: HeadlessBuildOtherError::Promotion(error),
        }) if error == expected_failure
    ) {
        return Err("GRID promotion failure was not swallowed at PROCESS_BARS".into());
    }

    let staff_glyphs = executor.sheet.glyphs.originals.len();
    let staff_digest = output_boundary_staff_digest(&executor.sheet.staffs)?;
    let bar_glyphs = executor
        .sheet
        .sig
        .systems
        .iter()
        .flat_map(|system| system.sig.nodes_in_order())
        .filter(|(_, node)| matches!(node, GridSigNode::Vertical { .. }))
        .count();
    let total_glyphs = staff_glyphs + bar_glyphs;

    let sig_name = |system: &HeadlessSystemSigState,
                    peaks: &[(&StaffPeak, &str)],
                    connector_name: Option<&str>|
     -> Result<String, Box<dyn Error>> {
        let mut names = BTreeMap::new();
        for &(peak, name) in peaks {
            let inter = system
                .sig
                .inter_of(peak.key())
                .ok_or("promoted peak has no SIG backlink")?;
            names.insert(inter, name.to_owned());
        }
        let connectors = system
            .sig
            .nodes_in_order()
            .filter_map(|(id, node)| matches!(node, GridSigNode::Connector { .. }).then_some(id))
            .collect::<Vec<_>>();
        match (connectors.as_slice(), connector_name) {
            ([connector], Some(name)) => {
                names.insert(*connector, name.to_owned());
            }
            ([], None) => {}
            _ => return Err("unexpected connector cardinality".into()),
        }
        let mut nodes = system
            .sig
            .nodes_in_order()
            .map(|(id, node)| -> Result<String, Box<dyn Error>> {
                let mut name = names
                    .get(&id)
                    .cloned()
                    .ok_or("SIG node lacks semantic identity")?;
                let frozen = match node {
                    GridSigNode::Vertical { frozen, .. }
                    | GridSigNode::Connector { frozen, .. } => *frozen,
                };
                if frozen {
                    name.push('*');
                }
                Ok(name)
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes.sort();
        let mut edges = system
            .sig
            .edges()
            .iter()
            .map(|edge| -> Result<String, Box<dyn Error>> {
                let source = names.get(&edge.source).ok_or("missing semantic source")?;
                let target = names.get(&edge.target).ok_or("missing semantic target")?;
                Ok(match edge.relation {
                    GridSigRelation::NoExclusion => format!("N:{source}>{target}"),
                    GridSigRelation::BarConnectionSupport { grade } => {
                        format!("C:{source}>{target}@{grade:.12}")
                    }
                    GridSigRelation::BarGroup { gap_fraction } => {
                        format!("G:{source}>{target}@{gap_fraction:.12}")
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        edges.sort();
        Ok(format!(
            "S{}{{nodes:{};edges:{}}}",
            system.system_id,
            nodes.join(","),
            if edges.is_empty() {
                "-".to_owned()
            } else {
                edges.join(",")
            }
        ))
    };
    let sigs = [
        sig_name(
            &executor.sheet.sig.systems[0],
            &[
                (&s1a, "b1.1@10"),
                (&s1b, "b1.1@12"),
                (&s2a, "b1.2@10"),
                (&s2b, "b1.2@12"),
            ],
            Some("c1.1-2@10"),
        )?,
        sig_name(&executor.sheet.sig.systems[1], &[(&s3a, "b2.3@20")], None)?,
    ]
    .join("|");

    let pages = executor
        .sheet
        .population
        .pages
        .iter()
        .zip(&executor.sheet.population.page_refs)
        .map(|(page, page_ref)| {
            let systems = page
                .system_ids
                .iter()
                .map(|system_id| format!("S{system_id}#1"))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "P{}[m{}:{systems}]",
                page.id,
                usize::from(page_ref.movement_start)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let refs = executor
        .sheet
        .population
        .systems
        .iter()
        .map(|system| -> Result<String, Box<dyn Error>> {
            let reference_id = system
                .system_ref
                .system_ref
                .ok_or("physical system has no soft reference")?;
            let page_id = system.page_id.ok_or("physical system has no page")?;
            let page_ref = executor
                .sheet
                .population
                .page_refs
                .iter()
                .find(|page| page.id == page_id)
                .ok_or("soft page reference is missing")?;
            let rank = page_ref
                .systems
                .iter()
                .position(|candidate| *candidate == reference_id)
                .ok_or("system reference is absent from page")?
                + 1;
            let reference = executor
                .sheet
                .population
                .references
                .get(reference_id)
                .ok_or("system reference is absent from registry")?;
            let mut back = reference.page_ref_id == page_ref.object_id;
            let parts = reference
                .parts
                .iter()
                .map(|part| {
                    back &= part.system_ref == reference_id;
                    part.staff_configs
                        .iter()
                        .map(|config| {
                            format!(
                                "{}{}",
                                config.line_count,
                                if config.is_small { "s" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>()
                .join("|");
            Ok(format!(
                "S{}#{rank}[p{page_id};parts:{parts};back{}]",
                system.id,
                usize::from(back)
            ))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(",");

    Ok(format!(
        "build:swallowed@PROCESS_BARS/missing:s1.staff1.x14;staffs:{staff_glyphs}/{staff_digest:016x};glyphs:staff{staff_glyphs},bar{bar_glyphs},total{total_glyphs};sig:{sigs};pages:{pages};refs:{refs};scores:{};done:builder1,cleaner1,step1",
        format_score_topology(&executor.book.scores)
    ))
}

fn grid_contextualize_vector() -> Result<String, Box<dyn Error>> {
    fn peak(staff: usize, x: i32) -> Result<StaffPeak, Box<dyn Error>> {
        Ok(StaffPeak::new(StaffId::new(staff), 10, 30, x, x)?)
    }

    fn vertical_plan(peak: &StaffPeak, impact: f64) -> VerticalInterPlan {
        VerticalInterPlan {
            peak: peak.key(),
            median: VerticalMedian {
                x: f64::from(peak.start()) + 0.5,
                top: 9.5,
                bottom: 31.5,
            },
            width: 1.0,
            impacts: Some(StaffVerticalImpacts::new(
                impact, impact, impact, impact, impact, impact,
            )),
            kind: VerticalInterKind::Barline {
                width_class: PeakWidthClass::Thin,
                left_staff_end: false,
                right_staff_end: false,
            },
        }
    }

    fn connection(
        top: &StaffPeak,
        bottom: &StaffPeak,
        first_id: usize,
        impact: f64,
    ) -> Result<BarAlignment, Box<dyn Error>> {
        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(first_id), top.staff_id(), top.start(), 1.0)?,
            AlignmentPeak::new(
                PeakId::new(first_id + 1),
                bottom.staff_id(),
                bottom.start(),
                1.0,
            )?,
            0.0,
            0.0,
            BarImpacts::alignment(impact, impact)?,
        )?;
        Ok(BarAlignment::connection(&alignment, impact, impact)?)
    }

    let a = peak(1, 10)?;
    let b = peak(2, 10)?;
    let c = peak(3, 10)?;
    let group_left = peak(4, 40)?;
    let group_right = peak(4, 42)?;
    let mut graph = PeakGraph::new();
    for peak in [&a, &b, &c] {
        graph.add_vertex(peak.clone());
    }
    graph.add_edge(a.key(), b.key(), connection(&a, &b, 1, 0.25)?)?;
    graph.add_edge(b.key(), c.key(), connection(&b, &c, 2, 0.875)?)?;

    let mut sig = GridSig::default();
    sig.promote_vertical_inters(&[
        vertical_plan(&a, 0.25),
        vertical_plan(&b, 0.5),
        vertical_plan(&c, 0.75),
        vertical_plan(&group_left, 0.375),
        vertical_plan(&group_right, 0.625),
    ]);
    let connection_plans = plan_connection_inters(&graph, |_| true);
    sig.promote_connection_inters(&graph, &connection_plans)?;
    sig.group_barlines(&[vec![group_left, group_right]], 3, |gap| {
        f64::from(gap) / 10.0
    })
    .map_err(|error| format!("contextualization fixture grouping failed: {error:?}"))?;

    let frozen_bits = |sig: &GridSig| {
        sig.nodes_in_order()
            .map(|(_, node)| {
                if matches!(
                    node,
                    GridSigNode::Vertical { frozen: true, .. }
                        | GridSigNode::Connector { frozen: true, .. }
                ) {
                    '1'
                } else {
                    '0'
                }
            })
            .collect::<String>()
    };
    let frozen_before = frozen_bits(&sig);
    let edge_count_before = sig.edges().len();
    sig.contextualize();
    let grades = sig
        .nodes_in_order()
        .map(|(_, node)| {
            format!(
                "{:.12}>{:.12}",
                node.intrinsic_grade(),
                node.contextual_grade()
                    .expect("contextualization assigns every GRID node")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "grades:{grades};frozen:{frozen_before}>{};edges:{edge_count_before}>{}",
        frozen_bits(&sig),
        sig.edges().len()
    ))
}

fn section_shape(section: &Section) -> String {
    let bounds = section.bounds();
    let runs = section
        .runs()
        .iter()
        .map(|run| format!("{}+{}", run.start, run.length))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}-{}/{}/{}/{}/{},{},{},{}/{}",
        section.first_pos(),
        section.last_pos(),
        section.run_count(),
        section.weight(),
        section.max_run_length(),
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        runs
    )
}

fn section_digest(sections: &[Section]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for section in sections {
        hash = hash_section(hash, section);
    }
    hash
}

fn hash_section(mut hash: u64, section: &Section) -> u64 {
    hash = hash_u32(hash, section.first_pos());
    hash = hash_u32(hash, section.run_count());
    hash = hash_u32(hash, section.weight());
    hash = hash_u32(hash, section.max_run_length());
    for run in section.runs() {
        hash = hash_u32(hash, run.start);
        hash = hash_u32(hash, run.length);
    }
    hash
}

fn filament_digest(filaments: &[StaffFilament]) -> Result<u64, FilamentError> {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for filament in filaments {
        let bounds = filament.bounds()?;
        hash = hash_u32(hash, filament.sections().len());
        hash = hash_u32(hash, bounds.x);
        hash = hash_u32(hash, bounds.y);
        hash = hash_u32(hash, bounds.width);
        hash = hash_u32(hash, bounds.height);
        hash = hash_u32(hash, filament.weight());
        hash = hash_u32(hash, filament.true_length()?);
        for section in filament.sections() {
            hash = hash_section(hash, section);
        }
    }
    Ok(hash)
}

fn staff_filament(
    x: usize,
    y: usize,
    length: usize,
    interline: usize,
) -> Result<StaffFilament, FilamentError> {
    staff_filament_band(x, y, length, 1, interline)
}

fn staff_filament_band(
    x: usize,
    y: usize,
    length: usize,
    thickness: usize,
    interline: usize,
) -> Result<StaffFilament, FilamentError> {
    let mut table = RunTable::new(Orientation::Horizontal, x + length + 1, y + thickness + 1)
        .expect("fixture dimensions are valid");
    for row in y..(y + thickness) {
        table
            .add_run(row, Run::new(x, length))
            .expect("fixture run is in bounds");
    }
    let mut filament = StaffFilament::new(interline)?;
    filament.add_section(build_sections(&table, JunctionPolicy::DEFAULT_RATIO).remove(0))?;
    Ok(filament)
}

fn cluster_points(points: &[Option<(f64, f64)>]) -> String {
    points
        .iter()
        .map(|point| point.map_or_else(|| "null".to_owned(), |(x, y)| format!("{x:.6},{y:.6}")))
        .collect::<Vec<_>>()
        .join(";")
}

fn target_point(point: (f64, f64)) -> String {
    format!("{:.12},{:.12}", point.0, point.1)
}

fn format_score_topology(scores: &[audiveris_omr::score_update::ScoreTopology]) -> String {
    let topologies = scores
        .iter()
        .map(|score| {
            score
                .pages
                .iter()
                .map(|page| format!("{}.{}", page.sheet_number, page.page_id))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("|");
    format!("{}:{topologies}", scores.len())
}

fn optional_i32(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

fn range_text(value: audiveris_core::range::Range) -> String {
    format!("{},{},{}", value.min, value.main, value.max)
}

fn append_scale_vectors(
    lines: &mut Vec<String>,
    slug: &str,
    histograms: &VerticalRunHistograms,
    image_size: (usize, usize),
) -> Result<(), Box<dyn Error>> {
    let scale = estimate_scale(
        histograms,
        ScaleOptions {
            image_size: Some(image_size),
            ..ScaleOptions::default()
        },
    )?;
    lines.push(format!(
        "scale.{slug}={}/{}/{}/{}/{}",
        scale.line.main,
        scale.interline.main,
        optional_i32(scale.small_interline.map(|value| value.main)),
        scale.beam.main,
        optional_i32(scale.small_beam.map(|value| value.main))
    ));
    lines.push(format!(
        "scale.{slug}.detail=black:{};combo:{};combo2:{};beam:{};beam2:{};guess:{};areas:{},{}",
        range_text(audiveris_core::range::Range::new(
            scale.line.min,
            scale.line.main,
            scale.line.max
        )),
        range_text(scale.primary_combo_peak),
        scale
            .secondary_combo_peak
            .map_or_else(|| "null".to_owned(), range_text),
        optional_i32(scale.beam_key),
        optional_i32(scale.beam_key2),
        optional_i32(scale.beam_guess),
        histograms.black.iter().sum::<usize>(),
        histograms.combo.iter().sum::<usize>()
    ));
    Ok(())
}

fn append_page_scale_vectors(
    lines: &mut Vec<String>,
    root: &Path,
    slug: &str,
    relative_path: &str,
) -> Result<(), Box<dyn Error>> {
    let loaded = ingest::load_max_channel_gray(root.join(relative_path))?;
    let binary =
        adaptive::default_adaptive_filter(loaded.width(), loaded.height(), loaded.pixels());
    let vertical = RunTable::from_pixels(
        Orientation::Vertical,
        loaded.width(),
        loaded.height(),
        &binary,
    )?;
    let histograms = vertical_run_histograms(&vertical);
    append_scale_vectors(lines, slug, &histograms, (loaded.width(), loaded.height()))
}

fn rust_vectors(root: Option<&Path>) -> Result<String, Box<dyn Error>> {
    let mut lines = Vec::with_capacity(VECTOR_KEYS.len());
    lines.push(format!(
        "natural.decode={:?}",
        natural_spec::decode(Some("1 - 3 , 6"), true, None)?
    ));
    lines.push(format!(
        "natural.encode={}",
        natural_spec::encode(&[5, 2, 4, 6, 7, 8, 10, 12])
    ));

    let two_thirds = Rational::new(2, 3)?;
    let one_half = Rational::new(1, 2)?;
    lines.push(format!("rational.sum={}", two_thirds.plus(one_half)?));
    lines.push(format!(
        "rational.gcd={}",
        Rational::gcd_pair(two_thirds, Rational::new(5, 4)?)?
    ));

    let mut histogram = Histogram::default();
    for (bucket, count) in [(3, 2), (4, 10), (5, 12), (8, 3), (10, 6), (11, 0)] {
        histogram.increase_count(bucket, count);
    }
    lines.push(format!("histogram.data={}", histogram.data_string()));
    lines.push(format!(
        "histogram.summary={}/{}/{}",
        histogram.total_count(),
        histogram.max_bucket().ok_or("histogram has no maximum")?,
        histogram.max_count().ok_or("histogram has no count")?
    ));

    let line = BasicLine::from_coordinates(&[1., 2., 3., 4., 5.], &[4., 9., 14., 19., 24.])?;
    // Java and Rust hypot implementations can differ by one ULP. Geometry is
    // canonicalized at the explicitly declared 1e-15 comparison boundary.
    lines.push(format!("line.origin={:.15}", line.distance_of(0., 0.)?));
    lines.push(format!("line.one-ten={:.15}", line.distance_of(1., 10.)?));

    let contextual = grade::contextual_from_partners(0.2, &[0.5, 0.8], &[5.0, 2.0])
        .map_err(|error| error.to_owned())?;
    lines.push(format!("grade.contextual={contextual:.17}"));

    // This fixture deliberately enters the native classifier below feature
    // extraction. The production Java probe obtains the same bundled
    // BasicClassifier singleton, normalizes this exact vector in situ, and
    // invokes its private NeuralNetwork model. Keeping every output in model
    // order detects weight orientation, normalization, accumulation, and
    // sigmoid differences without entangling an unported descriptor stage.
    let classifier = BasicClassifier::bundled()?;
    let features: [f64; INPUT_SIZE] =
        std::array::from_fn(|index| ((index * 17 % 23) as f64 - 11.0) / 7.0);
    let grades = classifier.evaluate(&features);
    if grades.len() != audiveris_classifier::OUTPUT_SIZE {
        return Err("bundled classifier returned an unexpected output count".into());
    }
    lines.push(format!(
        "classifier.basic.synthetic={}",
        grades
            .iter()
            .map(|grade| format!("{}:{:.17}", grade.shape, grade.grade))
            .collect::<Vec<_>>()
            .join(";")
    ));

    let (mapping, cost) = injection_solver::solve(3, 3, |domain, range| {
        (i32::try_from(domain + 1).expect("small fixture")
            - i32::try_from(range).expect("small fixture"))
        .abs()
    })?;
    lines.push(format!("injection={mapping:?}/{cost}"));

    let mut integer = IntegerFunction::new(2, 9);
    for (x, value) in [
        (2, 1),
        (3, 4),
        (4, 4),
        (5, 2),
        (6, 5),
        (7, 1),
        (8, 3),
        (9, 3),
    ] {
        integer.set_value(x, value);
    }
    lines.push(format!(
        "integer.function={}/{}/{:?}/{}",
        integer.arg_max(2, 9),
        integer.area(),
        integer.local_maxima(0, 20),
        integer.derivative(3)
    ));

    let mut short_projection = ShortProjection::new(-3, 1)?;
    short_projection.increment(-3, i32::from(i16::MAX));
    short_projection.increment_one(-3);
    short_projection.increment(-2, 65_537);
    short_projection.increment(-1, -65_537);
    short_projection.increment(0, i32::MAX);
    short_projection.increment(1, i32::from(i16::MIN));
    let short_values = (short_projection.start()..=short_projection.stop())
        .map(|position| short_projection.value(position))
        .collect::<Vec<_>>();
    let short_derivatives = ((short_projection.start() - 1)..=short_projection.stop())
        .map(|position| short_projection.derivative(position))
        .collect::<Vec<_>>();
    lines.push(format!(
        "projection.short.synthetic={}:{}:{}/{short_values:?}/{short_derivatives:?}",
        short_projection.start(),
        short_projection.stop(),
        short_projection.len()
    ));

    let staff_threshold = |counts: &[i32], top_count| -> Result<i32, Box<dyn Error>> {
        let stop = i32::try_from(counts.len() - 1)?;
        let mut projection = ShortProjection::new(0, stop)?;
        for (position, count) in counts.iter().copied().enumerate() {
            projection.increment(i32::try_from(position)?, count);
        }
        Ok(projection.staff_derivative_threshold(0, stop, top_count, 0.3)?)
    };
    let round_up_threshold = staff_threshold(&[0, 5, 10, 15, 20, 25], 5)?;
    let round_down_threshold = staff_threshold(&[0, 15, 30, 45, 60, 75], 5)?;
    let zero_top_threshold = staff_threshold(&[0, 5, 10, 15, 20, 25], 0)?;
    lines.push(format!(
        "grid.staff-projector-threshold.synthetic=ties:{round_up_threshold},{round_down_threshold};top0:{zero_top_threshold}"
    ));

    let mut blank_projection = ShortProjection::new(0, 9)?;
    blank_projection.increment_one(2);
    blank_projection.increment_one(7);
    let blank_regions = blank_projection.blank_regions(0);
    let blank_name = |blank: Option<audiveris_image::projection::ProjectionBlank>| {
        blank.map_or_else(
            || "null".to_owned(),
            |blank| format!("{}-{}", blank.start(), blank.stop()),
        )
    };
    let all_blanks = blank_regions
        .iter()
        .map(|blank| blank_name(Some(*blank)))
        .collect::<Vec<_>>()
        .join(",");
    let right_from_two = select_blank(&blank_regions, HorizontalSide::Right, 2, 2);
    let right_from_four = select_blank(&blank_regions, HorizontalSide::Right, 4, 2);
    let left_from_seven = select_blank(&blank_regions, HorizontalSide::Left, 7, 2);
    lines.push(format!(
        "grid.staff-projector-blanks.synthetic=all:{all_blanks};right2:{};right4:{};left7:{}",
        blank_name(right_from_two),
        blank_name(right_from_four),
        blank_name(left_from_seven)
    ));

    let peak_params = PeakRefinementParams::new(25, 2, 5, 2, 2)?;
    let mut tied_peak_projection = ShortProjection::new(0, 10)?;
    for (position, value) in [(5, 10), (6, 10), (7, 10), (8, 5)] {
        tied_peak_projection.increment(position, value);
    }
    let tied_peak_side = tied_peak_projection
        .refine_peak_side(
            PeakRefinementRequest::new(4, 7, 1, false, 4, 0),
            peak_params,
        )?
        .ok_or("tied peak side fixture was rejected")?;
    let mut border_peak_projection = ShortProjection::new(0, 5)?;
    border_peak_projection.increment(4, 4);
    border_peak_projection.increment(5, 4);
    let border_peak_side = border_peak_projection
        .refine_peak_side(
            PeakRefinementRequest::new(4, 5, 1, false, 3, 0),
            peak_params,
        )?
        .ok_or("border peak side fixture was rejected")?;
    let peak_side_name = |side: audiveris_image::projection::PeakSide| {
        format!(
            "{},{:.12},{:.12}",
            side.abscissa, side.derivative_grade, side.chunk_grade
        )
    };
    lines.push(format!(
        "grid.staff-projector-peak-side.synthetic=tie:{};border:{}",
        peak_side_name(tied_peak_side),
        peak_side_name(border_peak_side)
    ));

    let peak_candidate_params = PeakConstructionParams::new(peak_params, 15)?;
    let mut accepted_peak_projection = ShortProjection::new(0, 12)?;
    for (position, value) in [(3, 10), (4, 40), (5, 40), (6, 40), (7, 40), (8, 10)] {
        accepted_peak_projection.increment(position, value);
    }
    let accepted_peak = accepted_peak_projection.construct_peak_candidate(
        PeakConstructionRequest::new(4, 7, false, 10, 10, 0),
        peak_candidate_params,
    )?;
    let mut wide_peak_projection = ShortProjection::new(0, 25)?;
    wide_peak_projection.increment(3, 10);
    for position in 4..=20 {
        wide_peak_projection.increment(position, 40);
    }
    wide_peak_projection.increment(21, 10);
    let wide_peak = wide_peak_projection.construct_peak_candidate(
        PeakConstructionRequest::new(4, 20, false, 10, 10, 0),
        peak_candidate_params,
    )?;
    let missing_peak = ShortProjection::new(0, 9)?.construct_peak_candidate(
        PeakConstructionRequest::new(2, 3, false, 10, 10, 0),
        peak_candidate_params,
    )?;
    let candidate_name =
        |candidate: Option<audiveris_image::projection::ProjectionPeakCandidate>| {
            candidate.map_or_else(
                || "null".to_owned(),
                |candidate| format!("{}-{}", candidate.start, candidate.stop),
            )
        };
    lines.push(format!(
        "grid.staff-projector-peak-candidate.synthetic=accepted:{};overWidth:{};missing:{}",
        candidate_name(accepted_peak),
        candidate_name(wide_peak),
        candidate_name(missing_peak)
    ));

    let accepted_candidate = accepted_peak.ok_or("accepted core fixture has no candidate")?;
    let geometry = PeakCoreGeometry::new(0, 40, 20);
    let core_params = PeakCoreParams::new(6, 0.3)?;
    let mut accepted_core_pixels = vec![255; 13 * 41];
    for y in 0..40 {
        for x in 4..=7 {
            accepted_core_pixels[(y * 13) + x] = 0;
        }
    }
    let accepted_core = accepted_candidate.validate_core(
        13,
        41,
        &accepted_core_pixels,
        geometry,
        0,
        core_params,
    )?;
    if !accepted_core.is_accepted() {
        return Err("accepted core fixture was rejected".into());
    }

    let mut gap_projection = ShortProjection::new(0, 12)?;
    gap_projection.increment(3, 10);
    for position in 4..=7 {
        gap_projection.increment(position, 30);
    }
    gap_projection.increment(8, 10);
    let gap_candidate = gap_projection
        .construct_peak_candidate(
            PeakConstructionRequest::new(4, 7, false, 10, 10, 0),
            peak_candidate_params,
        )?
        .ok_or("gap core fixture has no candidate")?;
    let mut gap_core_pixels = vec![255; 13 * 41];
    for x in 4..=7 {
        for y in 0..10 {
            gap_core_pixels[(y * 13) + x] = 0;
        }
        for y in 20..40 {
            gap_core_pixels[(y * 13) + x] = 0;
        }
    }
    let gap_core =
        gap_candidate.validate_core(13, 41, &gap_core_pixels, geometry, 0, core_params)?;
    if gap_core.rejection != Some(PeakCoreRejection::GapTooLarge) {
        return Err("gap core fixture took the wrong rejection branch".into());
    }

    let serif_candidate = accepted_peak_projection
        .construct_peak_candidate(
            PeakConstructionRequest::new(4, 7, false, 10, 10, 4),
            peak_candidate_params,
        )?
        .ok_or("serif core fixture has no candidate")?;
    let serif_core =
        serif_candidate.validate_core(13, 41, &accepted_core_pixels, geometry, 4, core_params)?;
    if serif_core.rejection != Some(PeakCoreRejection::InsufficientWhiteBeyondSerif) {
        return Err("serif core fixture took the wrong rejection branch".into());
    }
    lines.push(format!(
        "grid.staff-projector-core.synthetic=accepted:{}:gap{};gap:null:gap{};serif:null:white{:.12}",
        candidate_name(Some(accepted_candidate)),
        accepted_core.core.gap,
        gap_core.core.gap,
        serif_core
            .full_height_core
            .ok_or("serif fixture has no full-height core")?
            .white_ratio
    ));

    let mut rejected_range_projection = ShortProjection::new(0, 22)?;
    for position in 2..=18 {
        rejected_range_projection.increment(position, 30);
    }
    rejected_range_projection.increment(20, 40);
    let rejected_browse = rejected_range_projection.browse_peak_range(
        audiveris_image::projection::PeakRangeRequest::new(2, 18, false, 20, 20, 0),
        peak_candidate_params,
    )?;
    let after_rejected = rejected_range_projection.find_peaks_in_range(
        PeakScanRequest::new(0, 21, ProjectionPeakMode::Full, 25, 20, 20, 0),
        peak_candidate_params,
        |candidate| Ok(Some(candidate)),
    )?;
    let mut right_edge_projection = ShortProjection::new(0, 4)?;
    for position in 2..=4 {
        right_edge_projection.increment(position, 20);
    }
    let initial_half_edge = right_edge_projection.find_peaks_in_range(
        PeakScanRequest::new(0, 4, ProjectionPeakMode::InitialHalf, 12, 10, 10, 0),
        peak_candidate_params,
        |candidate| Ok(Some(candidate)),
    )?;
    let full_edge = right_edge_projection.find_peaks_in_range(
        PeakScanRequest::new(0, 4, ProjectionPeakMode::Full, 25, 10, 10, 0),
        peak_candidate_params,
        |candidate| Ok(Some(candidate)),
    )?;
    let candidate_ranges =
        |candidates: &[audiveris_image::projection::ProjectionPeakCandidate]| {
            if candidates.is_empty() {
                "none".to_owned()
            } else {
                candidates
                    .iter()
                    .map(|candidate| format!("{}-{}", candidate.start, candidate.stop))
                    .collect::<Vec<_>>()
                    .join(",")
            }
        };
    lines.push(format!(
        "grid.staff-projector-ranges.synthetic=rejectedBrowse:{};afterRejected:{};initialHalfEdge:{};fullEdge:{}",
        candidate_ranges(&rejected_browse),
        candidate_ranges(&after_rejected),
        candidate_ranges(&initial_half_edge),
        candidate_ranges(&full_edge)
    ));

    let mut brace_projection = ShortProjection::new(0, 20)?;
    for (position, value) in [(0, 1), (1, 1), (2, 1), (6, 6), (7, 7), (9, 8)] {
        brace_projection.increment(position, value);
    }
    for position in 13..=20 {
        brace_projection.increment(position, 1);
    }
    let brace_blanks = brace_projection.blank_regions(0);
    let left_ending_brace_blank = select_blank(&brace_blanks, HorizontalSide::Left, 15, 3);
    let brace_candidate = brace_projection
        .find_brace_candidate(&brace_blanks, BraceSearchRequest::new(15, 0, 14, 3, 5))?
        .ok_or("brace fixture produced no candidate")?;
    let brace_peak =
        brace_candidate.into_staff_peak(StaffId::new(1), |_| (0, 40), |point| point)?;
    let brace_blanks_name = brace_blanks
        .iter()
        .map(|blank| blank_name(Some(*blank)))
        .collect::<Vec<_>>()
        .join(",");
    lines.push(format!(
        "grid.staff-projector-brace.synthetic=blanks:{brace_blanks_name};leftEnding:{};peak:{}-{}:{}-{}:brace{}",
        blank_name(left_ending_brace_blank),
        brace_peak.start(),
        brace_peak.stop(),
        brace_peak.top(),
        brace_peak.bottom(),
        brace_peak.is_brace()
    ));

    let composed_width = 20;
    let composed_height = 6;
    let mut composed_pixels = vec![255; composed_width * composed_height];
    for x in 0..composed_width {
        composed_pixels[x] = 0;
        composed_pixels[((composed_height - 1) * composed_width) + x] = 0;
    }
    for x in 5..=6 {
        for y in 0..composed_height {
            composed_pixels[(y * composed_width) + x] = 0;
        }
    }
    let composed_accumulation = ShortProjection::from_staff_raster(
        composed_width,
        composed_height,
        &composed_pixels,
        StaffProjectionRequest::new(5, 6, 20),
        |_| 0,
        |_| 5,
    )?;
    let composed_result = composed_accumulation.finish_neutral(
        composed_width,
        composed_height,
        &composed_pixels,
        NeutralStaffProjectorRequest {
            staff_id: StaffId::new(1),
            staff_left: 5,
            staff_right: 6,
            blank_threshold: 2,
            minimum_wide_blank_width: 2,
            top_derivative_count: 5,
            minimum_derivative_ratio: 0.3,
            use_one_line_half_mode: false,
            is_one_line_staff: false,
            bar_threshold: 4,
            total_height: 10,
            peak_construction: PeakConstructionParams::new(
                PeakRefinementParams::new(4, 2, 4, 2, 1)?,
                4,
            )?,
            peak_core: PeakCoreParams::new(1, 0.3)?,
            brace_search: Some(BraceSearchRequest::new(5, 0, 7, 2, 4)),
        },
        |_| PeakCoreGeometry::new(0, 5, 2),
    )?;
    let mut composed_counts_digest = 0xcbf2_9ce4_8422_2325;
    for x in composed_result.projection.start()..=composed_result.projection.stop() {
        composed_counts_digest = hash_u32(
            composed_counts_digest,
            usize::try_from(composed_result.projection.value(x))?,
        );
    }
    let composed_blanks = composed_result
        .all_blanks
        .iter()
        .map(|blank| blank_name(Some(*blank)))
        .collect::<Vec<_>>()
        .join(",");
    let composed_peaks = if composed_result.peaks.is_empty() {
        "none".to_owned()
    } else {
        composed_result
            .peaks
            .iter()
            .map(|peak| {
                format!(
                    "{}-{}:{:.12}",
                    peak.start(),
                    peak.stop(),
                    peak.impacts().expect("graded composed peak").grade()
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let composed_brace = composed_result.brace_candidate.map_or_else(
        || "null".to_owned(),
        |brace| format!("{}-{}", brace.start, brace.stop),
    );
    lines.push(format!(
        "grid.staff-projector-composed.synthetic=bounds:{}-{};counts:{composed_counts_digest:016x};derivative:{};blanks:{composed_blanks};search:{}-{};peaks:{composed_peaks};brace:{composed_brace}",
        composed_result.projection.start(),
        composed_result.projection.stop(),
        composed_result.derivative_threshold,
        composed_result.peak_search_bounds.x_min,
        composed_result.peak_search_bounds.x_max
    ));

    let mut lines_root_projection = ShortProjection::new(0, 29)?;
    for position in 0..=29 {
        if !matches!(position, 0..=1 | 5..=10 | 12..=13) {
            lines_root_projection.increment_one(position);
        }
    }
    let lines_root_blanks = lines_root_projection.blank_regions(0);
    let selected_lines_root_blank = select_blank(&lines_root_blanks, HorizontalSide::Left, 20, 4);
    let mut lines_root_peaks = [
        audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 0, 4, 20, 21)?,
        audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 0, 4, 25, 26)?,
    ];
    lines_root_peaks[1].set(StaffPeakAttribute::StaffLeftEnd);
    let changed = check_lines_root_transition(
        &lines_root_peaks,
        &lines_root_blanks,
        false,
        Some(1),
        3,
        4,
        8,
    );
    let changed_start_marked = changed.clear_staff_left_end_at != Some(1);
    let boundary = check_lines_root_transition(
        &lines_root_peaks,
        &lines_root_blanks,
        false,
        Some(1),
        3,
        4,
        9,
    );
    let brace_noop = check_lines_root_transition(
        &lines_root_peaks,
        &lines_root_blanks,
        true,
        Some(1),
        3,
        4,
        8,
    );
    lines.push(format!(
        "grid.staff-projector-lines-root.synthetic=selected:{};changed:left{}:start{}:first{};boundary:left{}:start{};brace:left{}:start{}",
        blank_name(selected_lines_root_blank),
        changed.staff_left,
        changed_start_marked,
        lines_root_peaks[0].is_staff_end(HorizontalSide::Left),
        boundary.staff_left,
        boundary.clear_staff_left_end_at != Some(1),
        brace_noop.staff_left,
        brace_noop.clear_staff_left_end_at != Some(1)
    ));

    let mut result_first =
        audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 0, 4, 10, 11)?;
    let mut result_last =
        audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 0, 4, 20, 21)?;
    result_first.set_staff_end(HorizontalSide::Right);
    result_last.set_staff_end(HorizontalSide::Left);
    let first_key = result_first.key();
    let mut result_operations = audiveris_image::projection::NeutralStaffProjectorResult {
        projection: ShortProjection::new(0, 99)?,
        derivative_threshold: 0,
        all_blanks: Vec::new(),
        peak_search_bounds: audiveris_image::projection::PeakSearchBounds {
            x_min: 0,
            x_max: 99,
        },
        peaks: vec![result_first, result_last],
        brace_candidate: None,
    };
    let initial_start = result_operations
        .start_peak_index()
        .ok_or("missing start peak")?;
    let initial_last = result_operations
        .last_peak()
        .ok_or("missing last peak")?
        .start();
    let inserted = audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 0, 4, 15, 16)?;
    let inserted_key = inserted.key();
    let equal_anchor = audiveris_image::staff_peak::StaffPeak::new(StaffId::new(1), 9, 12, 20, 21)?;
    result_operations.insert_peak_before(inserted, equal_anchor.key())?;
    let inserted_order = result_operations
        .peaks
        .iter()
        .map(|peak| peak.start().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let inserted_start = result_operations
        .start_peak_index()
        .ok_or("missing inserted start")?;
    result_operations.remove_peaks(&[inserted_key, first_key]);
    let remaining_order = result_operations
        .peaks
        .iter()
        .map(|peak| peak.start().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let remaining_last = result_operations
        .last_peak()
        .ok_or("missing remaining peak")?
        .start();

    let mut right_projection = ShortProjection::new(0, 99)?;
    for position in 0..=99 {
        if !(40..=50).contains(&position) {
            right_projection.increment_one(position);
        }
    }
    let right_blanks = right_projection.blank_regions(0);
    let right_peaks = [audiveris_image::staff_peak::StaffPeak::new(
        StaffId::new(1),
        0,
        4,
        30,
        32,
    )?];
    let over = refine_right_end_transition(&right_peaks, &right_blanks, 25, 99, 3, 6);
    let exact = refine_right_end_transition(&right_peaks, &right_blanks, 25, 99, 3, 7);
    lines.push(format!(
        "grid.staff-projector-result-ops.synthetic=initial:start{initial_start}:last{initial_last};insert:{inserted_order}:start{inserted_start};remove:{remaining_order}:last{remaining_last};rightBlank:40-50;over:right{}:marked{};boundary:right{}:marked{}",
        over.staff_right,
        over.set_staff_right_end_at == Some(0),
        exact.staff_right,
        exact.set_staff_right_end_at == Some(0)
    ));

    let mut runs = RunTable::new(Orientation::Horizontal, 10, 5)?;
    runs.add_run(0, Run::new(1, 2))?;
    runs.add_run(0, Run::new(5, 3))?;
    runs.add_run(1, Run::new(0, 1))?;
    runs.add_run(1, Run::new(4, 2))?;
    lines.push(format!(
        "runs={}/{}/{}",
        runs.total_run_count(),
        runs.weight(),
        runs.run_at(6, 0).ok_or("fixture run not found")?
    ));

    let mut dispatch_source = RunTable::new(Orientation::Vertical, 5, 8)?;
    for (position, run) in [
        (0, Run::new(0, 2)),
        (0, Run::new(4, 3)),
        (1, Run::new(1, 4)),
        (3, Run::new(0, 1)),
        (3, Run::new(3, 2)),
        (4, Run::new(2, 5)),
    ] {
        dispatch_source.add_run(position, run)?;
    }
    let (dispatch_horizontal, dispatch_long) = dispatch_grid_runs(&dispatch_source, 2, 1.2)?;
    lines.push(format!(
        "grid.run-dispatch.synthetic=source:{}/{};long:{}/{};horizontal:{}/{}",
        dispatch_source.total_run_count(),
        dispatch_source.weight(),
        dispatch_long.total_run_count(),
        dispatch_long.weight(),
        dispatch_horizontal.total_run_count(),
        dispatch_horizontal.weight()
    ));

    let mut section_runs = RunTable::new(Orientation::Horizontal, 9, 6)?;
    for (position, run) in [
        (0, Run::new(1, 3)),
        (0, Run::new(6, 2)),
        (1, Run::new(1, 3)),
        (1, Run::new(6, 2)),
        (2, Run::new(1, 7)),
        (3, Run::new(2, 5)),
        (4, Run::new(2, 2)),
        (4, Run::new(5, 2)),
        (5, Run::new(2, 2)),
        (5, Run::new(5, 2)),
    ] {
        section_runs.add_run(position, run)?;
    }
    let sections = build_sections(&section_runs, JunctionPolicy::DEFAULT_RATIO);
    let mut section_shapes = sections.iter().map(section_shape).collect::<Vec<_>>();
    section_shapes.sort_unstable();
    lines.push(format!(
        "grid.sections.synthetic={}/{}/{}/{}/{}",
        sections.len(),
        sections.len(),
        section_runs.total_run_count(),
        section_runs.weight(),
        section_shapes.join("|")
    ));
    let mut filament_runs = RunTable::new(Orientation::Horizontal, 165, 15)?;
    for (row, start) in [(2, 0), (5, 40), (8, 80), (11, 120)] {
        filament_runs.add_run(row, Run::new(start, 45))?;
        filament_runs.add_run(row + 1, Run::new(start, 45))?;
    }
    let mut filament = StaffFilament::new(10)?;
    for section in build_sections(&filament_runs, JunctionPolicy::DEFAULT_RATIO) {
        filament.add_section(section)?;
    }
    let bounds = filament.bounds()?;
    let geometry = filament.geometry()?;
    let start = geometry.start();
    let stop = geometry.stop();
    let samples = [0.0, 40.0, 80.0, 120.0, 164.0]
        .into_iter()
        .map(|x| {
            Ok(format!(
                "{x:.0}:{:.12}:{:.12}",
                geometry.position_at(x)?,
                geometry.slope_at(x)?
            ))
        })
        .collect::<Result<Vec<_>, FilamentError>>()?
        .join(",");
    let within = [-1.0, 0.0, 164.0, 165.0]
        .into_iter()
        .map(|x| {
            if geometry.is_within_range(x) {
                '1'
            } else {
                '0'
            }
        })
        .collect::<String>();
    lines.push(format!(
        "grid.filament.synthetic={}/{},{},{},{}/{}/{}/{:.12},{:.12}/{:.12},{:.12}/{:.12}/{}/{}",
        filament.sections().len(),
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        filament.weight(),
        filament.true_length()?,
        start.0,
        start.1,
        stop.0,
        stop.1,
        filament.thickness()?,
        samples,
        within
    ));
    let mut factory_runs = RunTable::new(Orientation::Horizontal, 85, 14)?;
    for row in [2, 3] {
        factory_runs.add_run(row, Run::new(0, 40))?;
        factory_runs.add_run(row, Run::new(45, 40))?;
    }
    for row in [10, 11] {
        factory_runs.add_run(row, Run::new(0, 40))?;
    }
    let factory_sections = build_sections(&factory_runs, JunctionPolicy::DEFAULT_RATIO);
    let factory = FilamentFactory::new(FilamentFactoryParams {
        interline: 10,
        min_core_section_length: 5,
        min_section_aspect: 3.0,
        max_coord_gap: 17.0,
        max_pos_gap: 1.0,
        max_pos_gap_for_slope: 1.0,
        max_gap_slope: 0.5,
        min_length_for_delta_slope: 100.0,
        max_delta_slope: 0.01,
    });
    let factory_filaments = factory.retrieve_core_filaments(&factory_sections)?;
    let mut factory_shapes = factory_filaments
        .iter()
        .map(|factory_filament| -> Result<String, FilamentError> {
            let mut member_shapes = factory_filament
                .sections()
                .iter()
                .map(|member| {
                    let member_bounds = member.bounds();
                    format!(
                        "{},{},{},{},{}",
                        member_bounds.x,
                        member_bounds.y,
                        member_bounds.width,
                        member_bounds.height,
                        member.weight()
                    )
                })
                .collect::<Vec<_>>();
            member_shapes.sort();
            let factory_bounds = factory_filament.bounds()?;
            let factory_geometry = factory_filament.geometry()?;
            let factory_start = factory_geometry.start();
            let factory_stop = factory_geometry.stop();
            Ok(format!(
                "{}/{},{},{},{}/{}/{}/{}/{:.12},{:.12}/{:.12},{:.12}/{:.12}",
                factory_filament.sections().len(),
                factory_bounds.x,
                factory_bounds.y,
                factory_bounds.width,
                factory_bounds.height,
                factory_filament.weight(),
                factory_filament.true_length()?,
                member_shapes.join(";"),
                factory_start.0,
                factory_start.1,
                factory_stop.0,
                factory_stop.1,
                factory_filament.thickness()?
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    factory_shapes.sort();
    lines.push(format!(
        "grid.filament-factory.synthetic={}/{}",
        factory_filaments.len(),
        factory_shapes.join("|")
    ));
    let mut overlap_sections = Vec::new();
    for (x, y, length) in [(0, 2, 40), (10, 3, 40), (5, 8, 40)] {
        let mut overlap_runs = RunTable::new(Orientation::Horizontal, 55, 12)?;
        overlap_runs.add_run(y, Run::new(x, length))?;
        overlap_sections
            .push(build_sections(&overlap_runs, JunctionPolicy::DEFAULT_RATIO).remove(0));
    }
    let overlap_factory = FilamentFactory::new(FilamentFactoryParams {
        interline: 10,
        min_core_section_length: 5,
        min_section_aspect: 3.0,
        max_coord_gap: 17.0,
        max_pos_gap: 1.0,
        max_pos_gap_for_slope: 1.0,
        max_gap_slope: 0.5,
        min_length_for_delta_slope: 100.0,
        max_delta_slope: 0.01,
    });
    let overlap_filaments = overlap_factory.retrieve_core_filaments_with_overlap(
        &overlap_sections,
        OverlapParams {
            probe_width: 5,
            max_overlap_delta_pos: 2.0,
            max_thickness: 2.0,
            max_overlap_space: 2.0,
            max_expansion_space: 0.0,
            max_involving_length: 20.0,
            max_consistent_ratio: 1.7,
        },
    )?;
    let mut overlap_member_counts = overlap_filaments
        .iter()
        .map(|filament| filament.sections().len())
        .collect::<Vec<_>>();
    overlap_member_counts.sort_unstable();
    lines.push(format!(
        "grid.filament-factory.overlap={}/{:016x}/{}/{:?}/{:016x}",
        overlap_sections.len(),
        section_digest(&overlap_sections),
        overlap_filaments.len(),
        overlap_member_counts,
        filament_digest(&overlap_filaments)?
    ));
    let mut line_cluster =
        LineCluster::new(10, FilamentId::new(1), staff_filament(0, 12, 40, 10)?)?;
    line_cluster.include_line(0, FilamentId::new(2), staff_filament(45, 12, 40, 10)?)?;
    line_cluster.include_line(-1, FilamentId::new(3), staff_filament(10, 2, 40, 10)?)?;
    line_cluster.include_line(1, FilamentId::new(4), staff_filament(10, 22, 44, 10)?)?;
    let cluster_lines = line_cluster
        .lines()
        .map(|(position, line)| format!("{position}:{}", line.filament().sections().len()))
        .collect::<Vec<_>>()
        .join(",");
    let first_bounds = line_cluster.first_line().filament().bounds()?;
    let last_bounds = line_cluster.last_line().filament().bounds()?;
    let cluster_bounds = line_cluster.bounds()?;
    lines.push(format!(
        "grid.line-cluster.synthetic={}/{}/{},{},{},{}/{},{},{},{}/{},{},{},{}/{}/{}/{}",
        line_cluster.size(),
        cluster_lines,
        first_bounds.x,
        first_bounds.y,
        first_bounds.width,
        first_bounds.height,
        last_bounds.x,
        last_bounds.y,
        last_bounds.width,
        last_bounds.height,
        cluster_bounds.x,
        cluster_bounds.y,
        cluster_bounds.width,
        cluster_bounds.height,
        line_cluster.true_length()?,
        cluster_points(&line_cluster.points_at(5.0, 3, 0.25)?),
        cluster_points(&line_cluster.points_at(-3.0, 3, 0.25)?)
    ));
    let mut indexed_cluster =
        LineCluster::new(10, FilamentId::new(10), staff_filament(0, 12, 40, 10)?)?;
    indexed_cluster.include_line(-1, FilamentId::new(11), staff_filament(0, 2, 40, 10)?)?;
    indexed_cluster.include_line(1, FilamentId::new(12), staff_filament(0, 22, 40, 10)?)?;
    let at_limit_accepted = indexed_cluster.include_filament_by_index(
        FilamentId::new(13),
        staff_filament(10, 13, 19, 10)?,
        1,
        5,
        2,
    )?;
    let above_accepted = indexed_cluster.include_filament_by_index(
        FilamentId::new(14),
        staff_filament(10, 4, 19, 10)?,
        0,
        5,
        2,
    )?;
    let indexed_lines = indexed_cluster
        .lines()
        .map(|(position, line)| format!("{position}:{}", line.filament().sections().len()))
        .collect::<Vec<_>>()
        .join(",");
    lines.push(format!(
        "grid.line-cluster-index.synthetic=max:2;limitAccepted:{at_limit_accepted};aboveAccepted:{above_accepted};lines:{indexed_lines};starts:{};stops:{}",
        indexed_cluster
            .starts()?
            .into_iter()
            .map(|(x, y)| format!("{x:.6},{y:.6}"))
            .collect::<Vec<_>>()
            .join(";"),
        indexed_cluster
            .stops()?
            .into_iter()
            .map(|(x, y)| format!("{x:.6},{y:.6}"))
            .collect::<Vec<_>>()
            .join(";")
    ));
    let mut merge_destination =
        LineCluster::new(10, FilamentId::new(10), staff_filament(0, 10, 20, 10)?)?;
    let merge_destination_bottom =
        LineCluster::new(10, FilamentId::new(30), staff_filament(0, 30, 20, 10)?)?;
    merge_destination.merge_with(merge_destination_bottom, 2)?;
    let mut merge_source =
        LineCluster::new(10, FilamentId::new(40), staff_filament(25, 20, 20, 10)?)?;
    let merge_source_absorbed =
        LineCluster::new(10, FilamentId::new(41), staff_filament(50, 20, 20, 10)?)?;
    merge_source.merge_with(merge_source_absorbed, 0)?;
    let merge_source_bottom =
        LineCluster::new(10, FilamentId::new(50), staff_filament(25, 30, 20, 10)?)?;
    merge_source.merge_with(merge_source_bottom, 1)?;
    merge_destination.merge_with(merge_source, 1)?;
    let merged_lifecycle_lines = merge_destination
        .lines()
        .map(|(position, line)| {
            Ok(format!(
                "{position}:{}:{}",
                line.filament().sections().len(),
                line.filament().true_length()?
            ))
        })
        .collect::<Result<Vec<_>, FilamentError>>()?
        .join(",");

    let mut renumber_cluster =
        LineCluster::new(10, FilamentId::new(10), staff_filament(0, 20, 20, 10)?)?;
    renumber_cluster.merge_with(
        LineCluster::new(10, FilamentId::new(1), staff_filament(0, 0, 20, 10)?)?,
        -2,
    )?;
    renumber_cluster.merge_with(
        LineCluster::new(10, FilamentId::new(40), staff_filament(0, 50, 20, 10)?)?,
        5,
    )?;
    renumber_cluster.renumber_lines();
    let renumbered_positions = renumber_cluster
        .lines()
        .map(|(position, _)| position.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let trim_specs = [
        (1, 0, 10),
        (2, 10, 20),
        (3, 20, 30),
        (4, 30, 40),
        (5, 40, 20),
        (6, 50, 20),
        (7, 60, 20),
    ];
    let trim_filament = |index: usize| -> Result<StaffFilament, FilamentError> {
        let (_, y, length) = trim_specs[index];
        staff_filament(0, y, length, 10)
    };
    let mut trim_cluster = LineCluster::new(10, FilamentId::new(3), trim_filament(2)?)?;
    for (index, delta) in [(0, -2), (1, 1), (3, 3), (4, 4), (5, 5), (6, 6)] {
        let (id, _, _) = trim_specs[index];
        trim_cluster.merge_with(
            LineCluster::new(10, FilamentId::new(id), trim_filament(index)?)?,
            delta,
        )?;
    }
    let trimmed = trim_cluster.trim(&BTreeSet::from([5]), 0.5)?;
    let trimmed_ids = trimmed
        .iter()
        .map(|line| line.primary_id().value().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let kept_trim_lines = trim_cluster
        .lines()
        .map(|(position, line)| format!("{position}:{}", line.primary_id().value()))
        .collect::<Vec<_>>()
        .join(",");
    lines.push(format!(
        "grid.line-cluster-lifecycle.synthetic=merge:{merged_lifecycle_lines};renumber:{renumbered_positions};trimRemoved:{trimmed_ids};trimKept:{kept_trim_lines}"
    ));

    let coordinator_filaments = |count: u64| -> Result<
        (ClusterOwnership, BTreeMap<FilamentId, StaffFilament>),
        Box<dyn Error>,
    > {
        let count = usize::try_from(count)?;
        let mut table = RunTable::new(Orientation::Horizontal, 160, (count * 3) + 2)?;
        for index in 0..count {
            table.add_run((index * 3) + 1, Run::new(index * 20, 18))?;
        }
        let mut ownership = ClusterOwnership::new();
        let mut filaments = BTreeMap::new();
        for (index, section) in build_sections(&table, JunctionPolicy::All)
            .into_iter()
            .enumerate()
        {
            let mut filament = StaffFilament::new(10)?;
            filament.add_section(section)?;
            let filament_id = FilamentId::new(u64::try_from(index + 1)?);
            ownership.register_filament(filament_id, &filament)?;
            filaments.insert(filament_id, filament);
        }
        Ok((ownership, filaments))
    };
    let add_coordinator_comb = |ownership: &mut ClusterOwnership,
                                snapshots: &mut BTreeMap<CombId, RecursiveCombSnapshot>,
                                id: u64,
                                column: i32,
                                members: &[u64]|
     -> Result<(), Box<dyn Error>> {
        let mut comb = FilamentComb::new(column);
        for (index, member) in members.iter().copied().enumerate() {
            comb.append_root(usize::try_from(member)?, (index * 10) as f64)?;
        }
        let comb_id = CombId::new(id);
        ownership.register_comb(comb_id, &comb)?;
        snapshots.insert(comb_id, RecursiveCombSnapshot::from_comb(&comb));
        Ok(())
    };

    let (mut cycle_ownership, cycle_filaments) = coordinator_filaments(3)?;
    let mut cycle_combs = BTreeMap::new();
    add_coordinator_comb(&mut cycle_ownership, &mut cycle_combs, 1, 1, &[1, 2])?;
    add_coordinator_comb(&mut cycle_ownership, &mut cycle_combs, 2, 2, &[2, 3])?;
    add_coordinator_comb(&mut cycle_ownership, &mut cycle_combs, 3, 3, &[3, 1])?;
    let cycle_seed = FilamentId::new(1);
    let cycle_cluster_id = cycle_ownership.register_cluster(cycle_seed)?;
    let mut cycle_clusters = BTreeMap::from([(
        cycle_cluster_id,
        LineCluster::new(10, cycle_seed, cycle_filaments[&cycle_seed].clone())?,
    )]);
    include_from_combs(
        &mut cycle_ownership,
        &mut cycle_clusters,
        &cycle_filaments,
        &mut cycle_combs,
        cycle_cluster_id,
        cycle_seed,
        0,
    )?;
    let cycle_processed = cycle_combs
        .values()
        .map(|comb| comb.is_processed().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let cycle_lines = cycle_clusters[&cycle_cluster_id]
        .lines()
        .map(|(position, line)| format!("{position}-{}", line.primary_id().value()))
        .collect::<Vec<_>>()
        .join(",");

    let (mut collision_ownership, collision_filaments) = coordinator_filaments(4)?;
    let one = FilamentId::new(1);
    let two = FilamentId::new(2);
    let three = FilamentId::new(3);
    let four = FilamentId::new(4);
    let collision_destination = collision_ownership.register_cluster(one)?;
    collision_ownership.assign_filament(four, collision_destination, 1)?;
    let mut collision_destination_value =
        LineCluster::new(10, one, collision_filaments[&one].clone())?;
    collision_destination_value.include_line(1, four, collision_filaments[&four].clone())?;
    let collision_swallowed = collision_ownership.register_cluster(two)?;
    collision_ownership.assign_filament(three, collision_swallowed, 1)?;
    let mut collision_swallowed_value =
        LineCluster::new(10, two, collision_filaments[&two].clone())?;
    collision_swallowed_value.include_line(1, three, collision_filaments[&three].clone())?;
    let mut collision_clusters = BTreeMap::from([
        (collision_destination, collision_destination_value),
        (collision_swallowed, collision_swallowed_value),
    ]);
    let mut collision_combs = BTreeMap::new();
    add_coordinator_comb(
        &mut collision_ownership,
        &mut collision_combs,
        1,
        1,
        &[1, 2],
    )?;
    add_coordinator_comb(
        &mut collision_ownership,
        &mut collision_combs,
        2,
        1,
        &[2, 3],
    )?;
    include_from_combs(
        &mut collision_ownership,
        &mut collision_clusters,
        &collision_filaments,
        &mut collision_combs,
        collision_destination,
        one,
        0,
    )?;
    let collision_lines = collision_clusters[&collision_destination]
        .lines()
        .map(|(position, line)| format!("{position}-{}", line.primary_id().value()))
        .collect::<Vec<_>>()
        .join(",");
    let collision_processed = collision_combs
        .values()
        .map(|comb| comb.is_processed().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let cluster_merged =
        collision_ownership.cluster_parent(collision_swallowed)? == Some(collision_destination);
    let filament_parent = collision_ownership
        .filament_parent(two)?
        .ok_or("collision filament was not absorbed")?
        .value();
    lines.push(format!(
        "grid.line-cluster-recursive.synthetic=cycleProcessed:{cycle_processed};cycleLines:{cycle_lines};clusterMerged:{cluster_merged};filamentParent:{filament_parent};collisionLines:{collision_lines};collisionProcessed:{collision_processed}"
    ));
    let staff_ids = [4, 5, 6].map(StaffId::new);
    let top_peak = BarPeak::new(PeakId::new(1), staff_ids[0], 2.0, 10.5, false, true)?;
    let middle_peak = BarPeak::new(PeakId::new(2), staff_ids[1], 3.0, 13.0, false, false)?;
    let bottom_peak = BarPeak::new(PeakId::new(3), staff_ids[2], 4.0, 15.5, false, false)?;
    let relations = [
        PeakRelation::connection(top_peak.id(), middle_peak.id()),
        PeakRelation::connection(middle_peak.id(), bottom_peak.id()),
    ];
    let mut bar_column = BarColumn::new(staff_ids.to_vec())?;
    bar_column.add_peak(bottom_peak)?;
    bar_column.add_peak(top_peak)?;
    bar_column.add_peak(middle_peak)?;
    let initial_width = bar_column.mean_width();
    let initial_x = bar_column.deskewed_x();
    let initial_full = bar_column.is_full();
    let initial_connected = bar_column.is_fully_connected(&relations);
    let replacement = BarPeak::new(PeakId::new(4), staff_ids[1], 5.0, 22.0, false, false)?;
    bar_column.add_peak(replacement)?;
    let replacement_width = bar_column.mean_width();
    let replacement_x = bar_column.deskewed_x();
    let replacement_full = bar_column.is_full();
    let replacement_connected = bar_column.is_fully_connected(&relations);
    let brace_replacement = BarPeak::new(PeakId::new(4), staff_ids[1], 5.0, 22.0, true, false)?;
    bar_column.add_peak(brace_replacement)?;
    let column_slots = bar_column
        .peaks()
        .iter()
        .map(|peak| {
            let peak = peak.expect("full fixture column");
            format!("{}@{:.1}", peak.staff_id().value(), peak.deskewed_x())
        })
        .collect::<Vec<_>>()
        .join(",");
    lines.push(format!(
        "grid.bar-column.synthetic=slots:{column_slots};initial:{initial_width:.12},{initial_x:.12},{initial_full},{initial_connected},{};overwrite:{replacement_width:.12},{replacement_x:.12},{replacement_full},{replacement_connected};brace:{},{}",
        bar_column.is_start(),
        brace_replacement.is_brace(),
        bar_column.is_full()
    ));
    let bars_staff_ids = [StaffId::new(10), StaffId::new(11)];
    let c0_top = BarPeak::new(PeakId::new(1), bars_staff_ids[0], 2.0, 10.5, false, false)?;
    let c0_bottom = BarPeak::new(PeakId::new(2), bars_staff_ids[1], 2.0, 10.5, false, false)?;
    let c1_top = BarPeak::new(PeakId::new(3), bars_staff_ids[0], 2.0, 30.5, false, false)?;
    let c1_bottom = BarPeak::new(PeakId::new(4), bars_staff_ids[1], 2.0, 30.5, false, false)?;
    let c2_bottom = BarPeak::new(PeakId::new(5), bars_staff_ids[1], 2.0, 35.5, false, false)?;
    let c2_top = BarPeak::new(PeakId::new(6), bars_staff_ids[0], 2.0, 36.5, false, false)?;
    let c3_top = BarPeak::new(PeakId::new(7), bars_staff_ids[0], 2.0, 50.5, false, false)?;
    let c3_bottom = BarPeak::new(PeakId::new(8), bars_staff_ids[1], 2.0, 50.5, false, false)?;
    let bars_chains = vec![
        vec![c2_bottom],
        vec![c3_top, c3_bottom],
        vec![c0_top, c0_bottom],
        vec![c2_top],
        vec![c1_top, c1_bottom],
    ];
    let bars_relations = [
        PeakRelation::connection(c0_top.id(), c0_bottom.id()),
        PeakRelation::connection(c1_top.id(), c1_bottom.id()),
        PeakRelation::connection(c3_top.id(), c3_bottom.id()),
    ];
    let mut bars_columns = aggregate_bar_chains(&bars_staff_ids, &bars_chains, 8)?;
    let bars_start = start_column_candidate(&mut bars_columns, &bars_relations, 20, 8)
        .map_or(-1, |index| index as i32);
    let built_columns = bars_columns
        .iter_mut()
        .map(|column| {
            let slots = column
                .peaks()
                .iter()
                .map(|peak| {
                    peak.map_or_else(
                        || "-".to_owned(),
                        |peak| format!("{}@{:.1}", peak.staff_id().value(), peak.deskewed_x()),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "[{slots}|{:.1}|{}|{}]",
                column.deskewed_x(),
                column.is_full(),
                column.is_fully_connected(&bars_relations)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    lines.push(format!(
        "grid.bars-columns-start.synthetic=max:8,20,8;columns:{built_columns};start:{bars_start}"
    ));
    let comb_filaments = [
        CombFilament::new(1, 1, staff_filament(0, 2, 110, 10)?.geometry()?)?,
        CombFilament::new(2, 2, staff_filament(0, 12, 110, 10)?.geometry()?)?,
        CombFilament::new(3, 3, staff_filament(0, 22, 41, 10)?.geometry()?)?,
        CombFilament::new(4, 4, staff_filament(0, 45, 110, 10)?.geometry()?)?,
    ];
    let comb_columns = retrieve_combs(110, 10, 10, 10, &comb_filaments)?;
    let comb_shapes = comb_columns
        .iter()
        .map(|column| {
            let shapes = column
                .combs()
                .iter()
                .map(|comb| {
                    format!(
                        "[{}]",
                        comb.filament_ids()
                            .iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join("+");
            format!("{}{shapes}", column.x())
        })
        .collect::<Vec<_>>()
        .join(";");
    lines.push(format!(
        "grid.combs.synthetic={comb_shapes};popular:{}",
        popular_comb_size(&comb_columns).expect("fixture has combs")
    ));
    let target_line = TargetLine::new(filament.geometry()?, 75.0, 100.0, 300.0)?;
    lines.push(format!(
        "grid.target-line.synthetic=y:{:.12};left:{};mid:{};right:{};above:{};below:{};extra:{}",
        target_line.target_y(),
        target_point(target_line.source_of_x(100.0)?),
        target_point(target_line.source_of_x(200.0)?),
        target_point(target_line.source_of_x(300.0)?),
        target_point(target_line.source_of_point((200.0, 65.0))?),
        target_point(target_line.source_of_point((200.0, 85.0))?),
        target_point(target_line.source_of_x(350.0)?)
    ));
    let score_page = |sheet_number, page_id, movement_start| ScorePageInput {
        key: ScorePageKey {
            sheet_number,
            page_id,
        },
        movement_start,
    };
    let mut score_stubs = vec![
        StubPages {
            number: 1,
            valid_selected: true,
            pages: vec![score_page(1, 1, false)],
        },
        StubPages {
            number: 2,
            valid_selected: true,
            pages: vec![score_page(2, 1, false), score_page(2, 2, true)],
        },
        StubPages {
            number: 3,
            valid_selected: true,
            pages: vec![score_page(3, 1, false)],
        },
    ];
    let mut score_topology = create_scores(&score_stubs);
    let initial_score_topology = format_score_topology(&score_topology);
    score_stubs[1].pages = vec![score_page(2, 1, false)];
    update_scores(&score_stubs, 2, &mut score_topology)?;
    lines.push(format!(
        "grid.score-update.synthetic=initial:{initial_score_topology};updated:{}",
        format_score_topology(&score_topology)
    ));
    let mut reference_registry = PopulationReferenceRegistry::default();
    let reference_parts = [
        PopulationReferencePart {
            part_id: 1,
            staves: vec![
                PopulationReferenceStaff {
                    staff_id: 1,
                    config: PopulationStaffConfig {
                        line_count: 5,
                        is_small: false,
                    },
                },
                PopulationReferenceStaff {
                    staff_id: 2,
                    config: PopulationStaffConfig {
                        line_count: 1,
                        is_small: true,
                    },
                },
            ],
        },
        PopulationReferencePart {
            part_id: 2,
            staves: vec![PopulationReferenceStaff {
                staff_id: 3,
                config: PopulationStaffConfig {
                    line_count: 6,
                    is_small: false,
                },
            }],
        },
    ];
    let mut reference_systems = [PopulationSystem {
        id: 1,
        indented: false,
        parts: reference_parts.to_vec(),
        system_ref: PopulationSystemRefState::default(),
        page_id: None,
    }];
    let mut reference_pages: Vec<PopulationPage> = Vec::new();
    let mut reference_page_refs: Vec<PopulationReferencePage> = Vec::new();
    allocate_population_pages(
        &mut reference_systems,
        &mut reference_pages,
        &mut reference_page_refs,
        &mut reference_registry,
    );
    let reference_id = reference_systems[0]
        .system_ref
        .system_ref
        .expect("population builds a system ref");
    let reference = reference_registry
        .get(reference_id)
        .expect("fresh system ref");
    let reference_parts = reference
        .parts
        .iter()
        .map(|part| {
            let configs = part
                .staff_configs
                .iter()
                .map(|config| {
                    format!(
                        "{}{}",
                        config.line_count,
                        if config.is_small { "s" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{configs}:{}:{}:{}:back{}",
                part.name.as_deref().unwrap_or("null"),
                part.logical_id
                    .map_or_else(|| "null".to_owned(), |id| id.to_string()),
                part.manual,
                part.system_ref == reference_id
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    lines.push(format!(
        "grid.system-ref.synthetic=parts:{reference_parts};pageSystems:{};same:{};field:{}",
        reference_page_refs[0].systems.len(),
        reference_page_refs[0].systems.first() == Some(&reference_id),
        reference_systems[0].system_ref.system_ref == Some(reference_id)
    ));
    lines.push(format!("grid.skew.synthetic={}", grid_skew_vector()));
    lines.push(format!(
        "grid.raw-lines.synthetic={}",
        grid_raw_lines_vector()?
    ));
    lines.push(format!(
        "grid.line-endpoints.synthetic={}",
        grid_line_endpoints_vector()?
    ));
    lines.push(format!(
        "grid.line-holes.synthetic={}",
        grid_line_holes_vector()?
    ));
    lines.push(format!(
        "grid.bar-alignments.synthetic={}",
        grid_bar_alignments_vector()?
    ));
    lines.push(format!(
        "grid.bar-connections.synthetic={}",
        grid_bar_connections_vector()?
    ));
    lines.push(format!(
        "grid.output-boundary.synthetic={}",
        output_boundary_vector()?
    ));
    lines.push(format!(
        "grid.contextualize.synthetic={}",
        grid_contextualize_vector()?
    ));
    let line_spline = NaturalSpline::interpolate(&[(0.0, 1.0), (10.0, 6.0)])?;
    let quadratic_spline = NaturalSpline::interpolate(&[(0.0, 0.0), (20.0, 10.0), (30.0, 10.0)])?;
    let cubic_spline =
        NaturalSpline::interpolate(&[(0.0, 0.0), (12.0, 1.0), (19.0, 2.0), (30.0, 3.0)])?;
    let upper_exception = if line_spline.y_at_x(10.000_001).is_err() {
        "RuntimeException"
    } else {
        "none"
    };
    // HotSpot and Rust can differ by one ULP in the quadratic expression;
    // spline geometry is canonicalized at this explicit 1e-14 boundary.
    lines.push(format!(
        "spline.synthetic=line:{:.14},{:.14};quadratic:{:.14},{:.14};cubic:{:.14},{:.14};lower:{:.14};upper:{}",
        line_spline.y_at_x(4.0)?,
        line_spline.y_derivative_at_x(4.0)?,
        quadratic_spline.y_at_x(20.0)?,
        quadratic_spline.y_derivative_at_x(20.0)?,
        cubic_spline.y_at_x(24.5)?,
        cubic_spline.y_derivative_at_x(12.0)?,
        line_spline.y_at_x(-2.0)?,
        upper_exception
    ));

    let pixels = [
        0, 126, 127, 128, 255, 255, 0, 255, 0, 255, 10, 20, 200, 210, 220,
    ];
    let binary = global_filter::global_filter(&pixels, 127);
    lines.push(format!("image.threshold={binary:?}"));
    lines.push(format!(
        "image.median={:?}",
        median::median_gray(5, 3, &pixels, 1)
    ));
    let distances = ChamferDistance::default().compute_to_fore(5, 3, &binary);
    let mut distance_values = Vec::with_capacity(15);
    for y in 0..3 {
        for x in 0..5 {
            distance_values.push(distances.get(x, y));
        }
    }
    lines.push(format!("image.chamfer={distance_values:?}"));
    let watershed_profile = [3, 2, 1, 2, 3];
    let watershed = watershed::watershed_gray_level(5, 1, &watershed_profile, true, 1);
    let watershed_bits: String = watershed
        .lines
        .iter()
        .map(|&line| if line { '1' } else { '0' })
        .collect();
    lines.push(format!(
        "watershed.synthetic={}/{watershed_bits}",
        watershed.region_count
    ));
    let extracted = RunTable::from_pixels(Orientation::Horizontal, 5, 3, &binary)?;
    lines.push(format!(
        "image.runs={}/{}/{}/{}",
        extracted.total_run_count(),
        extracted.weight(),
        extracted
            .run_at(1, 0)
            .ok_or("first extracted run missing")?,
        extracted
            .run_at(4, 2)
            .map_or_else(|| "null".to_owned(), |run| run.to_string())
    ));
    lines.push(format!(
        "image.adaptive={:?}",
        adaptive::default_adaptive_filter(5, 3, &pixels)
    ));

    let mut staff_pixels = vec![255; 8 * 10];
    for (x, y) in [(2, 1), (3, 1), (4, 1), (2, 4), (3, 4), (2, 8)] {
        staff_pixels[y * 8 + x] = 0;
    }
    let fractional_pattern = StaffPattern::new(3, 3, 1, 3.5);
    let tie_pixels = [0, 255, 255, 255];
    let tie_pattern = StaffPattern::new(1, 2, 1, 4.0);
    let inclusive_pixels = [0; 3 * 3];
    let inclusive_pattern = StaffPattern::new(1, 1, 2, 4.0);
    let empty_pixels = [255];
    lines.push(format!(
        "staff-pattern.synthetic={:.12}/{:.12}/{:.12}/{:.12}/{:.12}",
        fractional_pattern.evaluate((2.0, 1.0), 8, 10, &staff_pixels),
        tie_pattern.evaluate((0.5, 0.0), 4, 1, &tie_pixels),
        inclusive_pattern.evaluate((1.0, 1.0), 3, 3, &inclusive_pixels),
        inclusive_pattern.evaluate((0.0, 0.0), 1, 1, &empty_pixels),
        tie_pattern.evaluate((-1.0, 0.0), 4, 1, &tie_pixels)
    ));

    if let Some(root) = root {
        let loaded = ingest::load_max_channel_gray(root.join("data/examples/chula.png"))?;
        lines.push(format!(
            "load.chula={}x{}/{:016x}",
            loaded.width(),
            loaded.height(),
            loaded.fnv1a64()
        ));
        let binary =
            adaptive::default_adaptive_filter(loaded.width(), loaded.height(), loaded.pixels());
        lines.push(format!(
            "binary.chula={:016x}",
            ingest::fnv1a64_bytes(&binary)
        ));
        let vertical = RunTable::from_pixels(
            Orientation::Vertical,
            loaded.width(),
            loaded.height(),
            &binary,
        )?;
        lines.push(format!(
            "scale.vertical-runs={}/{}/{:016x}",
            vertical.total_run_count(),
            vertical.weight(),
            run_table_digest(&vertical)
        ));
        let histograms = vertical_run_histograms(&vertical);
        append_scale_vectors(
            &mut lines,
            "chula",
            &histograms,
            (loaded.width(), loaded.height()),
        )?;
        let scale = estimate_scale(
            &histograms,
            ScaleOptions {
                image_size: Some((loaded.width(), loaded.height())),
                ..ScaleOptions::default()
            },
        )?;
        let mut short_vertical = vertical.clone();
        let mut long_vertical =
            RunTable::new(Orientation::Vertical, loaded.width(), loaded.height())?;
        let min_vertical_run_length =
            1 + (f64::from(scale.line.max) * 1.2).round_ties_even().max(0.0) as usize;
        short_vertical.purge(
            |run| run.length >= min_vertical_run_length,
            Some(&mut long_vertical),
        )?;
        let mut long_horizontal = RunTable::from_pixels(
            Orientation::Horizontal,
            loaded.width(),
            loaded.height(),
            &short_vertical.to_pixels(),
        )?;
        let mut short_horizontal =
            RunTable::new(Orientation::Horizontal, loaded.width(), loaded.height())?;
        let min_horizontal_run_length = (f64::from(scale.interline.main) * 0.25)
            .round_ties_even()
            .max(0.0) as usize;
        long_horizontal.purge(
            |run| run.length < min_horizontal_run_length,
            Some(&mut short_horizontal),
        )?;
        let horizontal_sections = build_sections(&long_horizontal, JunctionPolicy::DEFAULT_RATIO);
        let max_vertical_shift = (f64::from(scale.interline.main) * 0.05)
            .round_ties_even()
            .max(0.0) as usize;
        let vertical_sections = build_sections(
            &long_vertical,
            JunctionPolicy::Shift {
                max_shift: max_vertical_shift,
            },
        );
        lines.push(format!(
            "grid.chula={}/{}/{}/{:016x}/{}/{}/{}/{:016x}",
            horizontal_sections.len(),
            long_horizontal.total_run_count(),
            long_horizontal.weight(),
            section_digest(&horizontal_sections),
            vertical_sections.len(),
            long_vertical.total_run_count(),
            long_vertical.weight(),
            section_digest(&vertical_sections)
        ));

        // Bounded live-page coverage for the scoped factory. Expanded coordinate
        // intervals are disjoint, isolating core filtering and the real-gap branch;
        // overlap behavior has its own synthetic fixture and expansion remains queued.
        let page_min_core = (f64::from(scale.interline.main) * 0.5).round_ties_even() as usize;
        let page_max_length = 4 * usize::try_from(scale.interline.main)?;
        let page_max_coord_gap = (f64::from(scale.interline.main) * 1.7).round_ties_even() as usize;
        let mut page_factory_sections = Vec::new();
        for section in &horizontal_sections {
            let bounds = section.bounds();
            if bounds.width < page_min_core
                || bounds.width > page_max_length
                || section.mean_thickness(Orientation::Horizontal) > 1.0
            {
                continue;
            }
            let separated = page_factory_sections.iter().all(|accepted: &Section| {
                let other = accepted.bounds();
                bounds.x > other.x + other.width - 1 + page_max_coord_gap
                    || other.x > bounds.x + bounds.width - 1 + page_max_coord_gap
            });
            if separated {
                page_factory_sections.push(section.clone());
                if page_factory_sections.len() == 8 {
                    break;
                }
            }
        }
        let page_factory = FilamentFactory::new(FilamentFactoryParams {
            interline: usize::try_from(scale.interline.main)?,
            min_core_section_length: page_min_core,
            min_section_aspect: 3.0,
            max_coord_gap: page_max_coord_gap as f64,
            max_pos_gap: (f64::from(scale.line.main) * 0.75).round_ties_even(),
            max_pos_gap_for_slope: (f64::from(scale.interline.main) * 0.1).round_ties_even(),
            max_gap_slope: 0.5,
            min_length_for_delta_slope: f64::from(scale.interline.main) * 10.0,
            max_delta_slope: 0.01,
        });
        let page_filaments = page_factory.retrieve_core_filaments(&page_factory_sections)?;
        lines.push(format!(
            "grid.filament-factory.chula={}/{:016x}/{}/{:016x}",
            page_factory_sections.len(),
            section_digest(&page_factory_sections),
            page_filaments.len(),
            filament_digest(&page_filaments)?
        ));

        append_page_scale_vectors(
            &mut lines,
            root,
            "k545",
            "../../data/synth/k545-movement1-exposition/page-001.png",
        )?;
        append_page_scale_vectors(
            &mut lines,
            root,
            "essen",
            "../../data/synth/essenfolksong-erk20/page-001.png",
        )?;
        append_page_scale_vectors(
            &mut lines,
            root,
            "josquin",
            "../../data/synth/josquin-4vperilludaveprolatum/page-001.png",
        )?;

        let loaded = ingest::load_max_channel_gray(
            root.join("app/src/test/resources/org/audiveris/omr/image/Dichterliebe01-1.png"),
        )?;
        lines.push(format!(
            "load.dichterliebe={}x{}/{:016x}",
            loaded.width(),
            loaded.height(),
            loaded.fnv1a64()
        ));
        let binary =
            adaptive::default_adaptive_filter(loaded.width(), loaded.height(), loaded.pixels());
        lines.push(format!(
            "binary.dichterliebe={:016x}",
            ingest::fnv1a64_bytes(&binary)
        ));
    }

    lines.push(format!(
        "pipeline={}",
        OmrStep::ALL
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    ));
    Ok(format!("{}\n", lines.join("\n")))
}

fn vectors(args: &[String]) -> Result<(), Box<dyn Error>> {
    let root = java_root(args)?;
    let rust = rust_vectors(Some(&root))?;
    if args.iter().any(|arg| arg == "--rust-only") {
        print!("{rust}");
        return Ok(());
    }

    let java = java_vector_output(&root)?;
    let java_vectors = parse_canonical_vectors(&java)?;
    let rust_vectors = parse_canonical_vectors(&rust)?;
    if let Some(difference) = java_vectors.first_difference(&rust_vectors) {
        return Err(format!("Java/Rust parity vector mismatch: {difference}").into());
    }
    print!("{rust}");
    println!(
        "Java/Rust parity: {} canonical vectors match",
        VECTOR_KEYS.len()
    );
    Ok(())
}

fn parse_canonical_vectors(text: &str) -> Result<CanonicalVectors, Box<dyn Error>> {
    let mut vectors = CanonicalVectors::new();
    for (index, line) in text.lines().enumerate() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("canonical vector line {} has no '='", index + 1))?;
        vectors.insert(key, value)?;
    }
    Ok(vectors)
}

fn manifest_count(contents: &str) -> Result<usize, Box<dyn Error>> {
    let mut count = 0;
    for (index, line) in contents.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, path) = line
            .split_once("  ")
            .ok_or_else(|| format!("invalid manifest line {}", index + 1))?;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("invalid SHA-256 on manifest line {}", index + 1).into());
        }
        if path.is_empty() || Path::new(path).is_absolute() {
            return Err(format!("invalid path on manifest line {}", index + 1).into());
        }
        count += 1;
    }
    Ok(count)
}

fn manifest(args: &[String]) -> Result<(), Box<dyn Error>> {
    let root = java_root(args)?;
    let relative = Path::new("rust/oracle/manifest.sha256");
    let count = manifest_count(&fs::read_to_string(root.join(relative))?)?;
    let mut command = Command::new("shasum");
    command
        .args(["-a", "256", "-c"])
        .arg(relative)
        .current_dir(&root);
    let status = match command.status() {
        Ok(status) => status,
        Err(error) if error.kind() == ErrorKind::NotFound => Command::new("sha256sum")
            .arg("-c")
            .arg(relative)
            .current_dir(&root)
            .status()?,
        Err(error) => return Err(error.into()),
    };
    if !status.success() {
        return Err(format!("oracle manifest verification failed with {status}").into());
    }
    println!("Oracle manifest: {count} files match their frozen SHA-256 digests");
    Ok(())
}

fn usage() {
    println!(
        "cargo xtask equivalent:\n  cargo run -p xtask -- baseline [--run-java] [--java-root PATH]\n  cargo run -p xtask -- vectors [--rust-only] [--java-root PATH]\n  cargo run -p xtask -- manifest [--java-root PATH]"
    );
}

fn execute() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("baseline") => baseline(&args[1..]),
        Some("vectors") => vectors(&args[1..]),
        Some("manifest") => manifest(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(format!("unknown xtask command: {command}").into()),
    }
}

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gradle_junit_suite() {
        let xml = r#"<?xml version="1.0"?><testsuite name="x" tests="7" skipped="1" failures="2" errors="3"></testsuite>"#;
        assert_eq!(
            suite_counts(xml).unwrap(),
            TestCounts {
                suites: 1,
                tests: 7,
                failures: 2,
                errors: 3,
                skipped: 1,
            }
        );
    }

    #[test]
    fn rejects_incomplete_suite_tag() {
        assert!(suite_counts("<testsuite tests=\"1\">").is_err());
    }

    #[test]
    fn rust_vector_contract_is_stable() {
        let vectors = rust_vectors(None).unwrap();
        assert_eq!(
            vectors.lines().count(),
            VECTOR_KEYS.len() - ROOT_VECTOR_COUNT
        );
        assert!(vectors.starts_with("natural.decode=[1, 2, 3, 6]\n"));
        assert!(vectors.ends_with("LINKS,RHYTHMS,PAGE\n"));
    }

    #[test]
    fn validates_sha256_manifest_shape() {
        let manifest = "# metadata\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  fixture.png\n";
        assert_eq!(manifest_count(manifest).unwrap(), 1);
        assert!(manifest_count("abc  fixture.png\n").is_err());
        assert!(
            manifest_count(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  /absolute\n"
            )
            .is_err()
        );
    }

    #[test]
    fn canonical_vector_parser_rejects_malformed_and_duplicate_lines() {
        assert!(parse_canonical_vectors("missing-delimiter\n").is_err());
        assert!(parse_canonical_vectors("a=1\na=2\n").is_err());
        let parsed = parse_canonical_vectors("b=2\na=1\n").unwrap();
        assert_eq!(parsed.render(), "a=1\nb=2\n");
    }
}
