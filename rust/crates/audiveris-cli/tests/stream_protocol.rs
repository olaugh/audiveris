// SPDX-License-Identifier: AGPL-3.0-or-later

//! Black-box compatibility pins for omrscope's additive stdout framing.

use std::{path::PathBuf, process::Command};

const STEMS_EPILOG_ORACLE: &str = include_str!("../../../oracle/stems-epilog-chula.txt");

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

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../../data/examples/{name}"))
}

fn test_image(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../../app/src/test/resources/org/audiveris/omr/image/{name}"
    ))
}

fn invoke(stage: &str, stream: bool) -> String {
    let mut command = Command::new(binary());
    command.args(["-batch", "-step", stage, "-json"]);
    if stream {
        command.arg("-stream-json");
    }
    let input = if matches!(stage, "STEMS" | "REDUCTION") {
        batuque()
    } else {
        chula()
    };
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
    for stage in ["GRID", "LEDGERS", "HEADS", "STEMS", "REDUCTION"] {
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
            "STEMS" => &[
                "GRID",
                "HEADERS",
                "STEM_SEEDS",
                "BEAMS",
                "LEDGERS",
                "HEADS",
                "STEMS",
            ],
            _ => &[
                "GRID",
                "HEADERS",
                "STEM_SEEDS",
                "BEAMS",
                "LEDGERS",
                "HEADS",
                "STEMS",
                "REDUCTION",
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
        if stage == "REDUCTION" {
            assert!(
                payload.contains("\"stage\":\"REDUCTION\"")
                    && payload.contains("\"heads\":")
                    && payload.contains("\"stems\":")
                    && payload.contains("\"reduction\":"),
                "REDUCTION publication retains every upstream product and its stage-owned trace"
            );
            assert!(payload.contains("\"system_count\":3"));
            assert!(payload.contains("\"glyph_registry_entry_count\":1820"));
            assert!(payload.contains("\"opaque_live_inter_glyph_count\":59"));
            assert!(payload.contains("\"active_glyph_count_after\":406"));
            assert_eq!(
                payload.matches("\"reduction\":").count(),
                1,
                "REDUCTION stage-owned product is emitted exactly once"
            );
        }
    }
}

#[test]
fn stems_json_completes_the_parity_corpus_and_beyond_corpus_scan() {
    for (input, page, systems, stems, checked_heads, relations, abnormal_heads) in [
        (example("chula.png"), "chula.png", 3, 151, 326, 319, 7),
        (
            example("allegretto.png"),
            "allegretto.png",
            3,
            150,
            328,
            314,
            15,
        ),
        (example("batuque.png"), "batuque.png", 3, 148, 327, 323, 4),
        (example("carmen.png"), "carmen.png", 5, 178, 429, 403, 26),
        (
            example("cucaracha.png"),
            "cucaracha.png",
            3,
            114,
            405,
            400,
            5,
        ),
        (example("hove.png"), "hove.png", 5, 150, 343, 343, 0),
        (example("zizi.png"), "zizi.png", 2, 104, 221, 221, 5),
        (
            example("BachInvention5.jpg"),
            "BachInvention5.jpg",
            6,
            412,
            1142,
            1040,
            102,
        ),
        (
            example("D0392410-1.256.png"),
            "D0392410-1.256.png",
            4,
            255,
            947,
            725,
            223,
        ),
        (
            test_image("Dichterliebe01-1.png"),
            "Dichterliebe01-1.png",
            3,
            177,
            449,
            413,
            66,
        ),
        (
            test_image("Dichterliebe01-2.png"),
            "Dichterliebe01-2.png",
            4,
            227,
            590,
            544,
            81,
        ),
    ] {
        let output = Command::new(binary())
            .args(["-batch", "-step", "STEMS", "-json"])
            .arg(input)
            .output()
            .unwrap_or_else(|error| panic!("run STEMS for {page}: {error}"));
        assert!(
            output.status.success(),
            "STEMS failed for {page}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let payload = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
        assert_eq!(payload.lines().count(), 1, "{page} emits one JSON document");
        for exact in [
            format!("\"system_count\":{systems}"),
            format!("\"stem_count\":{stems}"),
            format!("\"checked_head_count\":{checked_heads}"),
            format!("\"head_stem_relation_count\":{relations}"),
            format!("\"abnormal_head_count\":{abnormal_heads}"),
        ] {
            assert!(payload.contains(&exact), "{page} is missing {exact}");
        }
        assert_eq!(
            payload.matches("\"grade_source\":").count(),
            stems,
            "{page} publishes every final native Stem exactly once"
        );

        if page == "chula.png" {
            for exact in [
                "\"removed_orphan_beam_count\":12",
                "\"removed_empty_beam_group_count\":12",
                "\"beam_head_relation_count\":342",
                "\"contextualized_inter_count\":766",
                "\"contextual_grade_digest\":\"ba83426ee73b2b10\"",
                // The remaining system-2/system-3 digest differences are exactly
                // four inherited HEADERS values and two inherited LEDGERS ULPs;
                // every STEMS-owned contextual result is Java-identical.
                "\"contextual_grade_digest\":\"125acaf46320d86e\"",
                "\"contextual_grade_digest\":\"21bda55a2a32d2c4\"",
            ] {
                assert!(
                    payload.contains(exact),
                    "Chula STEMS epilog is missing {exact}"
                );
            }
            for exact in [
                "beamHeadCount 123 beamHeadGradeSha256 0f5d270e4fa00c861645e77257f2fa79325b8a0ad3ace617a86da8578d2769f1",
                "beamHeadCount 109 beamHeadGradeSha256 e14c28b3700ac34023baa529788df9c02cca8d6567e9df0237ca9c1a02619755",
                "beamHeadCount 110 beamHeadGradeSha256 f9d268028846f675aade61a319af4f4ff4be5012639c42227498053932c0f057",
                "contextualGradeFnv64 ba83426ee73b2b10",
                "freshRuns 2 freshRunsByteIdentical true",
                "nativeScope GenericFinalizeBeamsAndContextualization",
            ] {
                assert!(
                    STEMS_EPILOG_ORACLE.contains(exact),
                    "frozen Java epilog fixture is missing {exact}"
                );
            }
        }
    }
}
