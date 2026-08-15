// SPDX-License-Identifier: AGPL-3.0-or-later

//! Black-box pins for deterministic page-level parallel recognition.

use std::{path::PathBuf, process::Command};

fn binary() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_audiveris-cli")
        .map(PathBuf::from)
        .expect("Cargo supplies the CLI binary to integration tests")
}

fn chula() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/examples/chula.png")
}

fn invoke(stage: &str, thread_count: usize, page_count: usize) -> std::process::Output {
    let page = chula();
    let mut command = Command::new(binary());
    command.args(["-batch", "-step", stage, "-json"]);
    for _ in 0..page_count {
        command.arg(&page);
    }
    command
        .env("AUDIVERIS_PAGE_THREADS", thread_count.to_string())
        .output()
        .expect("run audiveris-cli")
}

#[test]
fn parallel_pages_preserve_serial_output_order_and_bytes() {
    for (stage, page_count) in [("GRID", 4), ("HEADS", 2)] {
        let serial = invoke(stage, 1, page_count);
        assert!(
            serial.status.success(),
            "serial {stage} run failed: {}",
            String::from_utf8_lossy(&serial.stderr)
        );

        let parallel = invoke(stage, page_count, page_count);
        assert!(
            parallel.status.success(),
            "parallel {stage} run failed: {}",
            String::from_utf8_lossy(&parallel.stderr)
        );
        assert_eq!(parallel.stdout, serial.stdout, "{stage} stdout changed");
        assert_eq!(parallel.stderr, serial.stderr, "{stage} stderr changed");

        let lines = parallel.stdout.split(|byte| *byte == b'\n').count() - 1;
        assert_eq!(
            lines, page_count,
            "one schema-1 JSON document per input page"
        );
    }
}
