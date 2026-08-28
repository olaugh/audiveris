// SPDX-License-Identifier: AGPL-3.0-or-later

//! Black-box compatibility pins for omrscope's additive stdout framing.

use std::{path::PathBuf, process::Command};

const STEMS_EPILOG_ORACLE: &str = include_str!("../../../oracle/stems-epilog-chula.txt");
const SMALL_HEADS_CONSTANT: &str = "org.audiveris.omr.sheet.ProcessingSwitches.smallHeads=true";
const CUE_RECOVERY_CONSTANT: &str =
    "org.audiveris.omr.sheet.beam.CueBeamsStep.supplementalHookRecovery=true";

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

fn chopin_cue() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust/oracle/chopin-nocturne-page23-system4-cue.png")
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
    let input = if matches!(stage, "STEMS" | "REDUCTION" | "CUE_BEAMS") {
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
    for stage in [
        "GRID",
        "LEDGERS",
        "HEADS",
        "STEMS",
        "REDUCTION",
        "CUE_BEAMS",
    ] {
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
            "REDUCTION" => &[
                "GRID",
                "HEADERS",
                "STEM_SEEDS",
                "BEAMS",
                "LEDGERS",
                "HEADS",
                "STEMS",
                "REDUCTION",
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
                "CUE_BEAMS",
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
        if stage == "CUE_BEAMS" {
            assert!(
                payload.contains("\"stage\":\"CUE_BEAMS\"")
                    && payload.contains("\"reduction\":")
                    && payload.contains("\"cue_beams\":"),
                "CUE_BEAMS default skip retains REDUCTION and adds its stage result"
            );
            assert!(payload.contains("\"status\":\"skipped_small_heads_disabled\""));
            assert!(payload.contains("\"small_heads_enabled\":false"));
            assert!(payload.contains("\"mutation_count\":0"));
            assert_eq!(payload.matches("\"cue_beams\":").count(), 1);
        }
    }
}

#[test]
fn active_cue_beams_switch_completes_the_native_lifecycle() {
    let output = Command::new(binary())
        .args([
            "-batch",
            "-step",
            "CUE_BEAMS",
            "-json",
            "-constant",
            SMALL_HEADS_CONSTANT,
        ])
        .arg(batuque())
        .output()
        .expect("run active CUE_BEAMS");
    assert!(
        output.status.success(),
        "active CUE_BEAMS failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    assert!(payload.contains("\"stage\":\"CUE_BEAMS\""));
    assert!(payload.contains("\"status\":\"completed\""));
    assert!(payload.contains("\"ordinary_enabled\":true"));
}

#[test]
fn active_chopin_cue_beams_publish_a_connected_stable_graph() {
    let run = || {
        let output = Command::new(binary())
            .args([
                "-batch",
                "-step",
                "CUE_BEAMS",
                "-json",
                "-constant",
                SMALL_HEADS_CONSTANT,
            ])
            .arg(chopin_cue())
            .output()
            .expect("run connected Chopin CUE_BEAMS");
        assert!(
            output.status.success(),
            "connected Chopin CUE_BEAMS failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("CLI stdout is UTF-8")
    };
    let payload = run();
    assert_eq!(payload, run(), "connected sidecar IDs and order are stable");
    for exact in [
        "\"aggregate_count\":2",
        "\"spot_count\":7",
        "\"beam_count\":1",
        "\"group_count\":1",
        "\"relation_count\":4",
        "\"id\":\"s1:i1016\",\"sig_ordinal\":1016,\"provenance\":\"cue\",\"shape\":\"BEAM_SMALL\"",
        "\"id\":\"s1:i1017\",\"sig_ordinal\":1017,\"provenance\":\"cue\"",
        "\"aggregate_id\":\"s1:a0\"",
        "\"source_id\":\"s1:i1016\",\"target_id\":\"s1:i996\",\"provenance\":\"cue\"",
        "\"source_id\":\"s1:i1016\",\"target_id\":\"s1:i990\",\"provenance\":\"cue\"",
        "\"source_id\":\"s1:i1016\",\"target_id\":\"s1:i994\",\"provenance\":\"cue\"",
        "\"source_id\":\"s1:i1016\",\"target_id\":\"s1:i974\",\"provenance\":\"cue\"",
        "\"kind\":\"Containment\",\"source_id\":\"s1:i1017\",\"target_id\":\"s1:i1016\",\"provenance\":\"cue\"",
    ] {
        assert!(
            payload.contains(exact),
            "connected sidecar is missing {exact}"
        );
    }
    assert_eq!(payload.matches("\"kind\":\"BeamStem\"").count(), 4);
    assert_eq!(payload.matches("\"kind\":\"HeadStem\"").count(), 7);
    assert_eq!(payload.matches("\"kind\":\"Containment\"").count(), 1);
    assert_eq!(payload.matches("\"shape\":\"BEAM_SMALL\"").count(), 1);
}

#[test]
fn supplemental_chopin_cue_hook_is_stable_connected_and_provenanced() {
    let run = || {
        let output = Command::new(binary())
            .args([
                "-batch",
                "-step",
                "CUE_BEAMS",
                "-json",
                "-constant",
                SMALL_HEADS_CONSTANT,
                "-constant",
                CUE_RECOVERY_CONSTANT,
            ])
            .arg(chopin_cue())
            .output()
            .expect("run Chopin supplemental cue recovery");
        assert!(
            output.status.success(),
            "supplemental CUE_BEAMS failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("CLI stdout is UTF-8")
    };
    let payload = run();
    assert_eq!(payload, run(), "supplemental cue recovery is deterministic");
    for exact in [
        "\"supplemental_hook_recovery_enabled\":true",
        "\"recovery_count\":1",
        "\"beam_count\":2",
        "\"group_count\":2",
        "\"relation_count\":5",
        "\"id\":\"s1:i1017\",\"sig_ordinal\":1017,\"provenance\":\"recovery\",\"shape\":\"BEAM_SMALL\"",
        "\"source_spot_ordinal\":null,\"recovery\":{\"source\":\"stem_guided_hook\",\"base_beam_ordinal\":0,\"stem_seed_id\":996,\"paired_stem_seed_id\":null,\"side\":\"below\",\"direction\":\"left\"}",
        "\"kind\":\"Containment\",\"source_id\":\"s1:i1018\",\"target_id\":\"s1:i1017\",\"provenance\":\"recovery\"",
        "\"kind\":\"BeamStem\",\"source_id\":\"s1:i1017\",\"target_id\":\"s1:i996\",\"provenance\":\"recovery\"",
    ] {
        assert!(
            payload.contains(exact),
            "recovery sidecar is missing {exact}"
        );
    }
    assert_eq!(payload.matches("\"provenance\":\"recovery\"").count(), 3);
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
            313,
            16,
        ),
        (example("batuque.png"), "batuque.png", 3, 148, 327, 323, 4),
        (example("carmen.png"), "carmen.png", 5, 178, 429, 403, 26),
        (
            example("cucaracha.png"),
            "cucaracha.png",
            3,
            115,
            405,
            400,
            5,
        ),
        (example("hove.png"), "hove.png", 5, 150, 343, 343, 0),
        (example("zizi.png"), "zizi.png", 2, 103, 221, 220, 6),
        (
            example("BachInvention5.jpg"),
            "BachInvention5.jpg",
            6,
            412,
            1142,
            1039,
            103,
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
                "\"contextual_grade_digest\":\"c42a5b6b287e28b0\"",
                // The remaining system-2/system-3 digest differences are exactly
                // four inherited HEADERS values and two inherited LEDGERS ULPs;
                // every STEMS-owned contextual result is Java-identical.
                "\"contextual_grade_digest\":\"0e6031e2d3db7942\"",
                "\"contextual_grade_digest\":\"58f7962d132bff94\"",
            ] {
                assert!(
                    payload.contains(exact),
                    "Chula STEMS epilog is missing {exact}"
                );
            }
            for exact in [
                "beamHeadCount 123 beamHeadGradeSha256 bf3e6389a88830b4598dcd20c82225abb150ed7d9c9c6b9bec3e234d035f10fe",
                "beamHeadCount 109 beamHeadGradeSha256 556ae5783a9c9450f26da1086a06ba2c75786952f0c7ba42156d1358a2e463bf",
                "beamHeadCount 110 beamHeadGradeSha256 e4c0fd107782f3bd5bcab379aebca6793a2f9ed2fc881456be1bd0d941518b3e",
                "contextualGradeFnv64 c42a5b6b287e28b0",
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
