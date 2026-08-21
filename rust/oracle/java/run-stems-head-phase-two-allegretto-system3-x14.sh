#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Allegretto system 3's first real phase-two reused-stem append mutation.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-x14.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
transform="$script_dir/stems-head-phase-two-x14.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/allegretto.png"

cp "$base_probe" "$probe"
awk -f "$transform" "$base_head_linker" > "$head_linker"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseTwoPage="$input" \
            -PphaseTwoProbeSource="$probe" \
            -PphaseTwoHeadLinkerSource="$head_linker" \
            -I "$init" :app:stemsHeadPhaseTwoX14Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E '^(stemsheadphase2x14|stemsheadphase2retry .* system 3 queueIndex 1 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2x14|stemsheadphase2retry .* system 3 queueIndex 1 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Allegretto x14 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 3 ] || \
        ! grep -q '^stemsheadphase2x14frontier headInterId 1777 corner BR hSide RIGHT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 2 .*glyphs 2 .*existingStem id3148:' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2x14result headInterId 1777 .*reusedExisting true .*verticesBefore 267 verticesAfter 267 edgesBefore 317 edgesAfter 318 allocatorBefore 3170 allocatorAfter 3170 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page allegretto.png#1 system 3 queueIndex 1 headX 14 headSig 50 headInterId 1777 .*returned true .*sideChanges \[x14:sig50:RIGHT:false:true->true:true\] sigVerticesBefore 267 sigVerticesAfter 267 sigEdgesBefore 317 sigEdgesAfter 318 systemStemsBefore 52 systemStemsAfter 52 allocatorBefore 3170 allocatorAfter 3170$' "$tmp_dir/rows1"; then
    echo "Allegretto x14 phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-allegretto.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-allegretto.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "9196aa6841aba9d234c4a82d21185c4ed1367b0329fcfca9930c14f0c6a15331" ] || \
        [ "$base_fixture_sha" != "242260a9fe7b873ca8597840ea7253d45d6518742e924496ccc4a14bb2a8c41c" ]; then
    echo "Allegretto x14 strict phase-two predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
transform_sha=$(shasum -a 256 "$transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-allegretto-system3-x14.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto system-3 phase-two x14 C-link.'
    echo '# schema: stems-head-phase-two-allegretto-system3-x14-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2x14summary schema stems-head-phase-two-allegretto-system3-x14-v1 page allegretto.png#1 system 3 rows 3 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha transformSourceSha256 $transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha basePhaseTwoRunnerSha256 $base_runner_sha basePhaseTwoFixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedAllegrettoSystem3PhaseTwoX14ReusedStemAppend javaEvidence ReturnedBeforeSystem3RetryIndex2"
} > "$out"
echo "wrote $out"
