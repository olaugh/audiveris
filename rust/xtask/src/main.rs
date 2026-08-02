// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_core::{
    basic_line::BasicLine, grade, histogram::Histogram, injection_solver,
    integer_function::IntegerFunction, natural_spec, rational::Rational, step::OmrStep,
};
use audiveris_image::{
    adaptive,
    chamfer::ChamferDistance,
    global_filter, ingest, median,
    run_table::{Orientation, Run, RunTable},
    scale_estimate::{ScaleOptions, estimate_scale},
    scale_runs::{VerticalRunHistograms, vertical_run_histograms},
    watershed,
};
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

const VECTOR_KEYS: [&str; 32] = [
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
const ROOT_VECTOR_COUNT: usize = 13;

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
    if java != rust {
        let mut detail = String::new();
        for (index, (java_line, rust_line)) in java.lines().zip(rust.lines()).enumerate() {
            if java_line != rust_line {
                detail.push_str(&format!(
                    "\nline {}:\n  Java: {java_line}\n  Rust: {rust_line}",
                    index + 1
                ));
            }
        }
        return Err(format!("Java/Rust parity vector mismatch:{detail}").into());
    }
    print!("{rust}");
    println!(
        "Java/Rust parity: {} canonical vectors match",
        VECTOR_KEYS.len()
    );
    Ok(())
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
}
