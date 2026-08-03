// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_core::{
    basic_line::BasicLine, grade, histogram::Histogram, injection_solver,
    integer_function::IntegerFunction, natural_spec, natural_spline::NaturalSpline,
    rational::Rational, step::OmrStep,
};
use audiveris_image::{
    adaptive,
    bar_column::{BarColumn, BarPeak, PeakId, PeakRelation, StaffId},
    chamfer::ChamferDistance,
    cluster_coordinator::{RecursiveCombSnapshot, include_from_combs},
    cluster_ownership::{ClusterOwnership, CombId},
    comb_builder::{CombFilament, popular_comb_size, retrieve_combs},
    filament::{FilamentError, StaffFilament},
    filament_comb::FilamentComb,
    filament_factory::{FilamentFactory, FilamentFactoryParams, OverlapParams},
    global_filter, ingest,
    line_cluster::{FilamentId, LineCluster},
    median,
    projection::{
        BraceSearchRequest, NeutralStaffProjectorRequest, PeakConstructionParams,
        PeakConstructionRequest, PeakCoreGeometry, PeakCoreParams, PeakCoreRejection,
        PeakRefinementParams, PeakRefinementRequest, PeakScanRequest, ProjectionPeakMode,
        ShortProjection, StaffProjectionRequest, check_lines_root_transition, select_blank,
    },
    run_table::{Orientation, Run, RunTable},
    scale_estimate::{ScaleOptions, estimate_scale},
    scale_runs::{VerticalRunHistograms, vertical_run_histograms},
    section::{JunctionPolicy, Section, build_sections},
    staff_pattern::StaffPattern,
    staff_peak::{HorizontalSide, StaffPeakAttribute},
    target_line::TargetLine,
    watershed,
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

const VECTOR_KEYS: [&str; 57] = [
    "natural.decode=",
    "natural.encode=",
    "rational.sum=",
    "rational.gcd=",
    "histogram.data=",
    "histogram.summary=",
    "line.origin=",
    "line.one-ten=",
    "grade.contextual=",
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
    "runs=",
    "grid.sections.synthetic=",
    "grid.filament.synthetic=",
    "grid.filament-factory.synthetic=",
    "grid.filament-factory.overlap=",
    "grid.line-cluster.synthetic=",
    "grid.line-cluster-index.synthetic=",
    "grid.line-cluster-lifecycle.synthetic=",
    "grid.line-cluster-recursive.synthetic=",
    "grid.bar-column.synthetic=",
    "grid.combs.synthetic=",
    "grid.target-line.synthetic=",
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
