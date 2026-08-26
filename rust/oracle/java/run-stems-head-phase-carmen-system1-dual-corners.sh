#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Java evidence for Carmen system-1's shared-stump dual corners.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi
release_field()
{
    awk -F= -v name="$1" '$1 == name { value = $2; gsub(/^"|"$/, "", value); print value }' \
        "$JAVA_HOME/release"
}
if [ "$(release_field IMPLEMENTOR)" != "Eclipse Adoptium" ] || \
        [ "$(release_field IMPLEMENTOR_VERSION)" != "Temurin-25.0.3+9" ] || \
        [ "$(release_field JAVA_RUNTIME_VERSION)" != "25.0.3+9-LTS" ] || \
        [ "$(release_field OS_NAME)" != "Darwin" ] || \
        [ "$(release_field OS_ARCH)" != "aarch64" ] || \
        [ "$(release_field JVM_VARIANT)" != "Hotspot" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS aarch64 HotSpot" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_dir=$(mktemp -d /private/tmp/stems-head-carmen-system1-dual.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
probe="$script_dir/StemsFinalizePageProbe.java"
init="$script_dir/stems-finalize-page.init.gradle"
input="$repo_root/data/examples/carmen.png"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"
base_runner="$script_dir/run-stems-head-phase-zizi-system2-order23.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-zizi-system2-order23.txt"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PfinalizePage="$input" \
            -I "$init" :app:stemsFinalizePageProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep -E '^stemsfinalize(page |system page carmen\.png#1 system 1 )' "$pass_one" > "$rows_one"
grep -E '^stemsfinalize(page |system page carmen\.png#1 system 1 )' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Carmen system-1 Java passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$rows_one" | tr -d ' ')" -ne 2 ] || \
        ! grep -q '^stemsfinalizepage page carmen.png#1 systems 5 mode ForegroundPageSerial$' "$rows_one" || \
        ! grep -q '^stemsfinalizesystem page carmen.png#1 system 1 heads 45 undefs \[x39:sig3:id2137:\[LEFT\],x38:sig2:id2135:\[LEFT\]\] multipleBefore \[\] noStemBefore \[x38:sig2:id2135,x39:sig3:id2137\] abnormalBefore \[x38:sig2:id2135,x39:sig3:id2137\] removedHeadStem \[\] abnormalAfter \[x38:sig2:id2135,x39:sig3:id2137\] abnormalChanges \[\] sigVerticesBefore 163 sigVerticesAfter 163 sigEdgesBefore 175 sigEdgesAfter 175 systemStemsBefore 18 systemStemsAfter 18 allocatorBefore 3253 allocatorAfter 3253$' "$rows_one"; then
    echo "Carmen system-1 shared-stump final contract differs" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$input_sha" != "249330d6558d410f64f550180d3a659dd3c9c340dcdcb5ae08e809c273fe2e44" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ] || \
        [ "$probe_sha" != "9b5e9dbefbf400887f49feba934c573d851c67e65b3e43bfaabc86d6f2c36714" ] || \
        [ "$init_sha" != "e0ff89792bf75286317ef011e079f338696d29cc14918f4a3018307ba4ed9548" ] || \
        [ "$base_runner_sha" != "e27ee7698499b6074866b3ff475632ca9620e6ac92f7d07dcd785c4ef6e8f431" ] || \
        [ "$base_fixture_sha" != "958080b017b8132b3e545296d537f2728ff2609d809312c4a88e569767864629" ]; then
    echo "Carmen input, Java source, probe, init, or Boundary-185 predecessor drifted" >&2
    exit 1
fi

runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-carmen-system1-dual-corners.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Carmen system-1 shared-stump dual corners.'
    echo '# schema: stems-head-phase-carmen-system1-dual-corners-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadcarmensystem1dualsummary schema stems-head-phase-carmen-system1-dual-corners-v1 page carmen.png#1 system 1 rows 2 inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseZiziOrder23RunnerSha256 $base_runner_sha baseZiziOrder23FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope CarmenSystem1SharedStumpDualCornerPrefix javaEvidence ReturnedAfterFinalPageFinalize"
} > "$out"
echo "wrote $out"
