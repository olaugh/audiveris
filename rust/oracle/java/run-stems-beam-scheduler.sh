#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

page_key=chula
if [ "$#" -ne 0 ]; then
    if [ "$#" -ne 2 ] || [ "$1" != "--page" ]; then
        echo "usage: $0 [--page chula|allegretto|batuque|carmen|cucaracha|hove|zizi|BachInvention5]" >&2
        exit 2
    fi
    page_key=$2
fi
if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$repo_root/rust/oracle/java/StemsBeamSchedulerProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-scheduler.sh"

case "$page_key" in
    chula)
        page_file=chula.png
        expected_row_counts=1:3:122:11:4:4:0:3:0:3:1
        expected_page_summary_sha256=61e0ed339a1d23468cc2ed56434c5912822449514c0ed175043ef8c9919abf1a
        expected_frontier_digest=684055121baf6841d2568b2c513eed254cd503c61d36e792abb4d5410923012c
        expected_body_sha256=fe036b4ad56d4e7012e04dd859382f275de4d824521149e1abc8ecf8917763a6
        expected_body_lines=157
        expected_body_bytes=73308
        ;;
    allegretto)
        page_file=allegretto.png
        expected_row_counts=1:3:82:5:1:1:0:3:0:3:1
        expected_page_summary_sha256=3ed1bd78bf54fb598000164e310d33702eb4ead272df4cf8f59dc69945d2e0f5
        expected_frontier_digest=f7035edb208d73618c07dbf8cd2babb2f96bf8af88592fc276fe815d6edbef6b
        expected_body_sha256=5fc54c7206cd34708826034c03dd45654ce5be43858fef8c419623480cf91a45
        expected_body_lines=105
        expected_body_bytes=46657
        ;;
    batuque)
        page_file=batuque.png
        expected_row_counts=1:3:108:5:1:1:0:3:0:3:1
        expected_page_summary_sha256=fbec40e47138960798232ee4badc42a14bccd47ee31f49eb3dbf87b1c15020a2
        expected_frontier_digest=9e3f09114916acbe72040f96b4901a88c2231cc68d5092ee4df6769603f3c070
        expected_body_sha256=0eb888b8c05d21e7f77c3c827c69627d31ff6fef85a686d935c4ea031260e0c3
        expected_body_lines=131
        expected_body_bytes=58408
        ;;
    carmen)
        page_file=carmen.png
        expected_row_counts=1:5:168:9:2:2:0:5:0:5:1
        expected_page_summary_sha256=37dc416e7b590cbe4eaf8213201228b92b016b1f0c1d2877ffd130ba2b651d69
        expected_frontier_digest=eb57a11c015d09573b6aa289279ab8c5ded267779daf184a3cdb370bcb94e9ac
        expected_body_sha256=c186eaf9f2060c2ffe7b9c9388e82b87d8d04d23d53be067998e94cb21c3c1da
        expected_body_lines=203
        expected_body_bytes=94046
        ;;
    cucaracha)
        page_file=cucaracha.png
        expected_row_counts=1:3:25:7:2:2:0:3:0:3:1
        expected_page_summary_sha256=e248edde26dbe1e9a1f925fb11fbb029282dc81b871cd1192dcfc0b4e947a15d
        expected_frontier_digest=773b49280bcaabcd304474be4fb50fd364d7e0ad31c3f3a57da3fbfaa9d2d6b4
        expected_body_sha256=3fd195d8b9c133bddf4e59cea3d6110b1acd2bbc57509b9fe26c942555ea6192
        expected_body_lines=52
        expected_body_bytes=21444
        ;;
    hove)
        page_file=hove.png
        expected_row_counts=1:5:52:11:4:4:0:5:0:5:1
        expected_page_summary_sha256=2fecbaf63aa6a9a6db06523c73430f628e15a0aa4de65a72507b8c6dcd748363
        expected_frontier_digest=0dbdc45daa720dcfab18bdc115e85c023deae4b4dd52d600bb543a74e3fe3715
        expected_body_sha256=40f57fba89c7a4e1e7b2c48e289b20282f7571a13252ba3f91bf5c46bb66c204
        expected_body_lines=93
        expected_body_bytes=39736
        ;;
    zizi)
        page_file=zizi.png
        expected_row_counts=1:2:52:2:0:0:0:2:0:2:1
        expected_page_summary_sha256=a94ea27d85110c702ed81b25018f2d2650f3cca629f7b13abbb78fc3b3a533ab
        expected_frontier_digest=015ee3f804ea8a95f67fe2026f6855c1d1c40023e3eb311b479ec54996a342da
        expected_body_sha256=675b8e7b13432f7016a7ab345a0b6b0fc61acd20f646aaa2a95ad478cc09290d
        expected_body_lines=67
        expected_body_bytes=28438
        ;;
    BachInvention5)
        page_file=BachInvention5.jpg
        expected_row_counts=1:6:194:6:0:0:0:6:0:6:1
        expected_page_summary_sha256=1eb7532a12d9eac3a5bf13f416f01ffd9a50c413d34f10a84e7cca0ff099641b
        expected_frontier_digest=da6c562086cf6c6db568da0af23cfa64073f043b7d99b7d2070b91f09787c94f
        expected_body_sha256=8ab73b8832ed9d63f081f9423e88a8380e8dc96ccb8b3d26047941e69f44bffa
        expected_body_lines=225
        expected_body_bytes=101092
        ;;
    *)
        echo "unknown beam-scheduler page key: $page_key" >&2
        exit 2
        ;;
esac

expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi
if [ ! -f "$expand_fixture" ]; then
    echo "missing frozen beam-expand Chula fixture" >&2
    exit 2
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
expected_probe_source_sha256=afb5c564a474bc0c227b9fdc886cf892c60ae39aa62c1d93cef8aaf610b90fba
if [ "$probe_source_sha256" != "$expected_probe_source_sha256" ]; then
    echo "unexpected beam-scheduler probe source fingerprint" >&2
    echo "observed: $probe_source_sha256" >&2
    exit 1
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-scheduler-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-scheduler-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -Xlint:all \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

# Exactly one fresh JVM owns this one-page run. It is foregrounded and awaited;
# no Java process is left behind for a second page or a header-only invocation.
(
    cd "$repo_root/app"
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx48g \
        -Djava.awt.headless=true \
        -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamSchedulerProbe \
        "$repo_root/data/examples/$page_file:1"
) > "$probe_output"

schema_count=$(awk '$0 == "# schema: stems-beam-scheduler-v1" { count++ } END { print count + 0 }' \
    "$probe_output")
if [ "$schema_count" -ne 1 ]; then
    echo "beam-scheduler schema header is missing or duplicated" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeamschedulerpage / { pages++ }
    /^stemsbeamschedulersystem / { systems++ }
    /^stemsbeamschedulerbeam / { beams++ }
    /^stemsbeamschedulerattempt / { attempts++ }
    /^stemsbeamschedulerside / { sides++ }
    /^stemsbeamschedulerbeamdecision / { decisions++ }
    /^stemsbeamschedulerstump / { stumps++ }
    /^stemsbeamschedulerfrontier / { frontiers++ }
    /^stemsbeamschedulercomplete / { completes++ }
    /^stemsbeamschedulersystemsummary / { systemSummaries++ }
    /^stemsbeamschedulerpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n",
                pages, systems, beams, attempts, sides, decisions, stumps,
                frontiers, completes, systemSummaries, pageSummaries
    }
' "$probe_output")
if [ "$row_counts" != "$expected_row_counts" ]; then
    echo "unexpected $page_key beam-scheduler row counts" >&2
    echo "observed: $row_counts" >&2
    exit 1
fi

observed_page_summary_sha256=$(awk '/^stemsbeamschedulerpagesummary / { print }' \
    "$probe_output" | shasum -a 256 | awk '{print $1}')
if [ "$observed_page_summary_sha256" != "$expected_page_summary_sha256" ]; then
    echo "unexpected $page_key beam-scheduler page summary" >&2
    echo "observed digest: $observed_page_summary_sha256" >&2
    exit 1
fi

# Every compact scheduler plan reference must resolve to the already frozen isolated
# expand matrix. Invoked rows must also carry its exact prefix outcome.
plan_mismatches=$(awk '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    FNR == NR {
        if ($1 == "stemsbeamexpandplan") {
            fields(3)
            key = f["system"] ":" f["plan"]
            seen[key] = 1
            builder[key] = f["builder"]
            beamSig[key] = f["beamSig"]
            bAlias[key] = f["bAlias"]
            vSide[key] = f["vSide"]
            stemProfile[key] = f["stemProfile"]
            linkProfile[key] = f["linkProfile"]
        } else if ($1 == "stemsbeamexpandend") {
            fields(3)
            outcome[f["system"] ":" f["plan"]] = f["outcome"]
        }
        next
    }
    $1 == "stemsbeamschedulerattempt" {
        fields(3)
        key = f["system"] ":" f["plan"]
        bad = !seen[key]
        if (builder[key] != f["builder"] || beamSig[key] != f["beamSig"] ||
                bAlias[key] != f["bAlias"] || vSide[key] != f["vSide"] ||
                stemProfile[key] != f["stemProfile"] ||
                linkProfile[key] != f["linkProfile"]) bad = 1
        if (f["outcome"] != "-" && outcome[key] != f["outcome"]) bad = 1
        if (bad) mismatches++
    }
    END { print mismatches + 0 }
' "$expand_fixture" "$probe_output")
if [ "$plan_mismatches" -ne 0 ]; then
    echo "$plan_mismatches scheduler plan references disagree with beam-expand fixture" >&2
    exit 1
fi

frontier_digest=$(awk '/^stemsbeamschedulerfrontier / { print }' "$probe_output" \
    | shasum -a 256 | awk '{print $1}')
if [ "$frontier_digest" != "$expected_frontier_digest" ]; then
    echo "unexpected $page_key scheduler frontiers" >&2
    echo "observed: $frontier_digest" >&2
    exit 1
fi

body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
body_lines=$(wc -l < "$probe_output" | tr -d ' ')
body_bytes=$(wc -c < "$probe_output" | tr -d ' ')
if [ "$body_sha256:$body_lines:$body_bytes" != \
     "$expected_body_sha256:$expected_body_lines:$expected_body_bytes" ]; then
    echo "unexpected $page_key beam-scheduler body fingerprint" >&2
    echo "observed: $body_sha256:$body_lines:$body_bytes" >&2
    exit 1
fi

runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
cat "$probe_output"
printf 'stemsbeamschedulercorpussummary schema stems-beam-scheduler-v1 mode %s pages 1 pageRefs %s#1 rowCounts %s probeSourceSha256 %s runnerSourceSha256 %s expandFixtureSha256 %s emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshJvmPerPage true runnerJavaProcessReaped true backgroundJavaProcessesStarted 0\n' \
    "$page_key" "$page_file" "$row_counts" "$probe_source_sha256" "$runner_source_sha256" \
    "$(shasum -a 256 "$expand_fixture" | awk '{print $1}')" \
    "$body_sha256" "$body_lines" "$body_bytes"
