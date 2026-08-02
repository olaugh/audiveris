// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_core::{
    basic_line::BasicLine, grade, histogram::Histogram, injection_solver,
    integer_function::IntegerFunction, natural_spec, natural_spline::NaturalSpline,
    rational::Rational, step::OmrStep,
};
use audiveris_image::{
    adaptive,
    chamfer::ChamferDistance,
    filament::{FilamentError, StaffFilament},
    filament_factory::{FilamentFactory, FilamentFactoryParams},
    global_filter, ingest, median,
    run_table::{Orientation, Run, RunTable},
    scale_estimate::{ScaleOptions, estimate_scale},
    scale_runs::{VerticalRunHistograms, vertical_run_histograms},
    section::{JunctionPolicy, Section, build_sections},
    watershed,
};
use audiveris_testkit::CanonicalVectors;
use std::{
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

const VECTOR_KEYS: [&str; 38] = [
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
    "runs=",
    "grid.sections.synthetic=",
    "grid.filament.synthetic=",
    "grid.filament-factory.synthetic=",
    "spline.synthetic=",
    "image.threshold=",
    "image.median=",
    "image.chamfer=",
    "watershed.synthetic=",
    "image.runs=",
    "image.adaptive=",
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
        min_core_section_length: 16,
        min_section_aspect: 3.0,
        max_coord_gap: 5.0,
        max_pos_gap: 2.0,
        max_pos_gap_for_slope: 1.0,
        max_gap_slope: 0.1,
        min_length_for_delta_slope: 20.0,
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
