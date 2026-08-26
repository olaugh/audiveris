#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Carmen system 3 queue 3's reused-stem append mutation.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-carmen-s3-x0.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
x1_transform="$script_dir/stems-head-phase-two-carmen-system3-x1.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-carmen-system3-x0.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker_x1="$tmp_dir/HeadLinker-x1.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/carmen.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$x1_transform" "$head_linker_x14" > "$head_linker_x1"
awk -f "$retarget_transform" "$head_linker_x1" > "$head_linker"

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
grep -E '^(stemsheadphase2carmens3x0|stemsheadphase2retry .* system 3 queueIndex 3 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2carmens3x0|stemsheadphase2retry .* system 3 queueIndex 3 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Carmen system-3 x0 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 3 ] || \
        ! grep -q '^stemsheadphase2carmens3x0frontier headInterId 2405 corner BR hSide RIGHT vSide BOTTOM .*lastIndex 0 maxIndex 2 relations 1 .*glyphs 1 selected \[id531:298:1680:2:50:weight99\] .*existingStem id3984:glyphid531:298:1680:2:50:weight99:grade3fe59495bdb6bfc6:.* verticesBefore 279 edgesBefore 324 allocatorBefore 3985 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2carmens3x0result headInterId 2405 linkedStem id3984:glyphid531:298:1680:2:50:weight99:grade3fe59495bdb6bfc6:.*reusedExisting true applied grade3fefffffffffffe1:dxbce8618618618618:.*consistency3feb35fc845a8ece .*verticesBefore 279 verticesAfter 279 edgesBefore 324 edgesAfter 325 allocatorBefore 3985 allocatorAfter 3985 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page carmen.png#1 system 3 queueIndex 3 headX 0 headSig 3 headInterId 2405 grade 3fca4063aab2cd80 append true .*returned true .*sideChanges \[x0:sig3:RIGHT:false:true->true:true,x2:sig2:LEFT:true:false->true:true,x2:sig2:RIGHT:false:false->false:true\] sigVerticesBefore 279 sigVerticesAfter 279 sigEdgesBefore 324 sigEdgesAfter 325 systemStemsBefore 43 systemStemsAfter 43 allocatorBefore 3985 allocatorAfter 3985$' "$tmp_dir/rows1"; then
    echo "Carmen system-3 x0 phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-carmen-system3-x1.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-carmen-system3-x1.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "7ca5c2ee389c328bafbb88f3c560c75179cc1421211645d606264e20d380ac5c" ] || \
        [ "$base_fixture_sha" != "bed8d9d4e8ca5a272f3ca3b26fbac36261ad1f1df344c83d178f5d36da7dcf66" ]; then
    echo "Carmen system-3 x0 strict phase-two predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
base_transform_sha=$(shasum -a 256 "$base_transform" | awk '{print $1}')
x1_transform_sha=$(shasum -a 256 "$x1_transform" | awk '{print $1}')
retarget_transform_sha=$(shasum -a 256 "$retarget_transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-carmen-system3-x0.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Carmen system-3 phase-two x0 C-link.'
    echo '# schema: stems-head-phase-two-carmen-system3-x0-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2carmens3x0summary schema stems-head-phase-two-carmen-system3-x0-v1 page carmen.png#1 system 3 rows 3 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha x1TransformSourceSha256 $x1_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha basePhaseTwoRunnerSha256 $base_runner_sha basePhaseTwoFixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedCarmenSystem3PhaseTwoX0ReusedStemAppend javaEvidence ReturnedBeforeSystem3RetryIndex4"
} > "$out"
echo "wrote $out"
