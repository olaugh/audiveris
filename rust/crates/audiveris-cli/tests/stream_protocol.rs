// SPDX-License-Identifier: AGPL-3.0-or-later

//! Black-box compatibility pins for omrscope's additive stdout framing.

use std::{path::PathBuf, process::Command};

fn binary() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_audiveris-cli")
        .map(PathBuf::from)
        .expect("Cargo supplies the CLI binary to integration tests")
}

fn chula() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/examples/chula.png")
}

fn batuque() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/examples/batuque.png")
}

fn invoke(stage: &str, stream: bool) -> String {
    let mut command = Command::new(binary());
    command.args(["-batch", "-step", stage, "-json"]);
    if stream {
        command.arg("-stream-json");
    }
    let input = if stage == "STEMS" { batuque() } else { chula() };
    let output = command.arg(input).output().expect("run audiveris-cli");
    assert!(
        output.status.success(),
        "{stage} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("CLI stdout is UTF-8")
}

fn payload_for_stage(stream: &str, stage: &str) -> String {
    let lines: Vec<&str> = stream.lines().collect();
    let started = lines
        .iter()
        .position(|line| {
            line.contains("\"event\":\"stage_started\"")
                && line.contains(&format!("\"stage\":\"{stage}\""))
        })
        .unwrap_or_else(|| panic!("missing {stage} start marker"));
    let payload = lines
        .get(started + 1)
        .unwrap_or_else(|| panic!("missing {stage} payload"));
    assert!(
        payload.starts_with('{'),
        "payload is a schema-1 JSON document"
    );
    let completed = lines
        .get(started + 2)
        .unwrap_or_else(|| panic!("missing {stage} completion marker"));
    assert!(
        completed.contains("\"event\":\"stage_completed\"")
            && completed.contains(&format!("\"stage\":\"{stage}\"")),
        "payload remains strictly between its two markers"
    );
    (*payload).to_owned()
}

#[test]
fn stream_keeps_published_stage_payloads_byte_identical_to_ordinary_json() {
    for stage in ["GRID", "LEDGERS", "HEADS", "STEMS"] {
        let ordinary = invoke(stage, false);
        let stream = invoke(stage, true);
        let payload = payload_for_stage(&stream, stage);
        assert_eq!(
            ordinary,
            format!("{payload}\n"),
            "{stage} payload changed in stream mode"
        );

        assert!(stream.starts_with("@omrscope {\"stream_schema\":1"));
        let expected_stages: &[&str] = match stage {
            "GRID" => &["GRID"],
            "LEDGERS" => &["GRID", "HEADERS", "STEM_SEEDS", "BEAMS", "LEDGERS"],
            "HEADS" => &["GRID", "HEADERS", "STEM_SEEDS", "BEAMS", "LEDGERS", "HEADS"],
            _ => &[
                "GRID",
                "HEADERS",
                "STEM_SEEDS",
                "BEAMS",
                "LEDGERS",
                "HEADS",
                "STEMS",
            ],
        };
        let marker_lines: Vec<&str> = stream
            .lines()
            .filter(|line| line.starts_with("@omrscope "))
            .collect();
        assert_eq!(marker_lines.len(), 2 + 2 * expected_stages.len());
        for (index, marker) in marker_lines.iter().enumerate() {
            assert!(
                marker.contains(&format!("\"sequence\":{}", index + 1)),
                "markers stay monotonic even when a snapshot is large"
            );
        }
        let started: Vec<&str> = marker_lines
            .iter()
            .copied()
            .filter(|line| line.contains("\"event\":\"stage_started\""))
            .collect();
        let completed: Vec<&str> = marker_lines
            .iter()
            .copied()
            .filter(|line| line.contains("\"event\":\"stage_completed\""))
            .collect();
        for (index, expected) in expected_stages.iter().enumerate() {
            assert!(started[index].contains(&format!("\"stage\":\"{expected}\"")));
            assert!(completed[index].contains(&format!("\"stage\":\"{expected}\"")));
        }
        let finished = stream.lines().last().expect("run_finished marker");
        assert!(
            finished.contains("\"event\":\"run_finished\"")
                && finished.contains("\"success\":true"),
            "every stream ends with a successful terminal marker"
        );

        if stage == "LEDGERS" {
            assert!(
                payload.contains("\"stage\":\"LEDGERS\"")
                    && payload.contains("\"inters\":")
                    && payload.contains("\"relations\":"),
                "LEDGERS publication retains its ledger and relation payload"
            );
            assert_eq!(
                payload.matches("\"kind\":\"LEDGER\"").count(),
                18,
                "Chula publishes all 18 final ledger inters"
            );
            assert_eq!(
                payload.matches("\"ledger_lines\":").count(),
                1,
                "LEDGERS publication includes the inferred ledger-line collection"
            );
        }
        if stage == "STEMS" {
            assert!(
                payload.contains("\"stage\":\"STEMS\"")
                    && payload.contains("\"heads\":")
                    && payload.contains("\"stems\":"),
                "STEMS publication retains every upstream product and its stage-owned result"
            );
            assert!(payload.contains("\"system_count\":3"));
            assert!(payload.contains("\"stem_count\":148"));
            assert!(payload.contains("\"checked_head_count\":327"));
            assert!(payload.contains("\"abnormal_head_count\":4"));
            assert_eq!(
                payload.matches("\"grade_source\":").count(),
                148,
                "Batuque publishes every final native Stem exactly once"
            );
            assert_eq!(
                payload.matches("\"stems\":").count(),
                1,
                "STEMS stage-owned product is emitted exactly once"
            );
        }
    }
}
