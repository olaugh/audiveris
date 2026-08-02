// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
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
    if let Some(index) = args.iter().position(|arg| arg == "--java-root") {
        return Ok(args
            .get(index + 1)
            .ok_or("--java-root needs a path")?
            .into());
    }
    if let Some(root) = env::var_os("AUDIVERIS_JAVA_ROOT") {
        return Ok(root.into());
    }
    Ok(PathBuf::from(".."))
}

fn run_java(root: &Path) -> Result<(), Box<dyn Error>> {
    let java_home = env::var_os("JAVA_HOME").map_or_else(
        || {
            root.parent()
                .unwrap_or_else(|| Path::new("."))
                .join("jdk25/Contents/Home")
        },
        PathBuf::from,
    );
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

fn usage() {
    println!(
        "cargo xtask equivalent:\n  cargo run -p xtask -- baseline [--run-java] [--java-root PATH]"
    );
}

fn execute() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("baseline") => baseline(&args[1..]),
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
}
