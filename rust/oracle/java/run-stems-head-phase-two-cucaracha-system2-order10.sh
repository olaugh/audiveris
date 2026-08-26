#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java system-2 queue 10's reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s2-q10.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
order6_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order6.transform.awk"
order7_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order7.transform.awk"
order8_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order8.transform.awk"
order9_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order9.transform.awk"
order16_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order16.transform.awk"
order18_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order18.transform.awk"
order19_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order19.transform.awk"
order21_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order21.transform.awk"
system2_order8_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order8.transform.awk"
system2_order9_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order9.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order10.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/cucaracha.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" |
    awk -f "$order6_transform" |
    awk -f "$order7_transform" |
    awk -f "$order8_transform" |
    awk -f "$order9_transform" |
    awk -f "$order16_transform" |
    awk -f "$order18_transform" |
    awk -f "$order19_transform" |
    awk -f "$order21_transform" |
    awk -f "$system2_order8_transform" |
    awk -f "$system2_order9_transform" |
    awk -f "$retarget_transform" > "$head_linker"

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
grep -E '^(stemsheadphase2cucarachas2q10|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 10 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas2q10|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 10 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-2 queue-10 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 8 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 2 heads 150 queueSize 24 .*sigVertices 255 sigEdges 347 systemStems 43 allocator 2659$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q10frontier headInterId 1392 corner BL hSide LEFT vSide BOTTOM .*lastIndex 1 maxIndex 1 relations 2 .*glyphs 1 selected \[id252:1441:1211:3:129:weight301\] .*existingStem id2646:.*verticesBefore 255 edgesBefore 349 allocatorBefore 2659 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q10reusematch headInterId 1392 sourceHeadId 1475 sourceCorner BL sourceSide LEFT relationGrade 3fefa1a6bc8e8cfc stem id2646:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q10result headInterId 1392 linkedStem id2646:.*reusedExisting true applied grade3feb7b1081c1abf7:dxbfbae1892d23b6db:.*verticesBefore 255 verticesAfter 255 edgesBefore 349 edgesAfter 350 allocatorBefore 2659 allocatorAfter 2659 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q10reuse headInterId 1392 lastIndex 0 selectedStem - terminal ReturnedFromReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 2 queueIndex 10 headX 84 headSig 80 headInterId 1392 grade 3fe0c3dfeac5af12 append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=true:branch=BottomOnly\] returned true .*sideChanges \[x84:sig80:LEFT:false:true->true:true\] .*sigEdgesBefore 349 sigEdgesAfter 350 .*allocatorBefore 2659 allocatorAfter 2659$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-2 queue-10 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system2-order9.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order9.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order9.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "d08167704d3c14b81207d4f5959f3b7938cbc93d162b1eb1c3f269f1e7a801a8" ] || \
        [ "$base_fixture_sha" != "56ecc773a31b5bba11b9d22454519bee8444e6aa4270eedfada1e9e2976ce565" ] || \
        [ "$base_retarget_transform_sha" != "af763c75140add0f67a9ccb3b077797fdf7c640c5b80a122697de63f5beeb0a2" ]; then
    echo "Cucaracha system-2 queue-10 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order10.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-2 phase-two queue 10.'
    echo '# schema: stems-head-phase-two-cucaracha-system2-order10-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas2q10summary schema stems-head-phase-two-cucaracha-system2-order10-v1 page cucaracha.png#1 system 2 rows 8 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder9RunnerSha256 $base_runner_sha baseOrder9FixtureSha256 $base_fixture_sha baseOrder9RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem2PhaseTwoOrder10ReusedStemAppend javaEvidence ReturnedAfterSystem2RetryIndex10"
} > "$out"
echo "wrote $out"
