#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Carmen system 3's first real phase-two reused-stem append mutation.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-carmen-s3-x1.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-carmen-system3-x1.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/carmen.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$retarget_transform" "$head_linker_x14" > "$head_linker"

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
grep -E '^(stemsheadphase2carmens3x1|stemsheadphase2retry .* system 3 queueIndex 1 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2carmens3x1|stemsheadphase2retry .* system 3 queueIndex 1 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Carmen system-3 x1 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 3 ] || \
        ! grep -q '^stemsheadphase2carmens3x1frontier headInterId 2505 corner BR hSide RIGHT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 2 .*glyphs 2 selected \[id495:304:1695:3:93:weight265,id3617:306:1695:1:93:weight93\] .*existingStem id3949:' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2carmens3x1result headInterId 2505 .*reusedExisting true .*verticesBefore 279 verticesAfter 279 edgesBefore 323 edgesAfter 324 allocatorBefore 3985 allocatorAfter 3985 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page carmen.png#1 system 3 queueIndex 1 headX 1 headSig 53 headInterId 2505 .*returned true .*sideChanges \[x1:sig53:RIGHT:false:true->true:true\] sigVerticesBefore 279 sigVerticesAfter 279 sigEdgesBefore 323 sigEdgesAfter 324 systemStemsBefore 43 systemStemsAfter 43 allocatorBefore 3985 allocatorAfter 3985$' "$tmp_dir/rows1"; then
    echo "Carmen system-3 x1 phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-carmen-system2-order0.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-carmen-system2-order0.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "7a340e2bc3fe7b6b67c8237ec07a114cc4d5538af49e0908367e17aa1656bb4a" ] || \
        [ "$base_fixture_sha" != "0db719a0e09c08fb8ae3c4e7c08a040329839e4b80e976339dbd89ae3be79827" ]; then
    echo "Carmen system-3 x1 strict phase-two predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
base_transform_sha=$(shasum -a 256 "$base_transform" | awk '{print $1}')
retarget_transform_sha=$(shasum -a 256 "$retarget_transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-carmen-system3-x1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Carmen system-3 phase-two x1 C-link.'
    echo '# schema: stems-head-phase-two-carmen-system3-x1-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2carmens3x1summary schema stems-head-phase-two-carmen-system3-x1-v1 page carmen.png#1 system 3 rows 3 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha basePhaseTwoRunnerSha256 $base_runner_sha basePhaseTwoFixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedCarmenSystem3PhaseTwoX1ReusedStemAppend javaEvidence ReturnedBeforeSystem3RetryIndex2"
} > "$out"
echo "wrote $out"
