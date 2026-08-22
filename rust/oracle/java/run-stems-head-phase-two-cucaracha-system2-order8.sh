#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java system-2 queue 8's reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s2-q8.XXXXXX)
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
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order8.transform.awk"
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
grep -E '^(stemsheadphase2cucarachas2q8|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 8 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas2q8|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 8 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-2 queue-8 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 8 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 2 heads 150 queueSize 24 .*sigVertices 255 sigEdges 347 systemStems 43 allocator 2659$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q8frontier headInterId 1388 corner BL hSide LEFT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 2 .*glyphs 2 selected \[id250:1076:1211:3:135:weight317,id2487:1078:1211:1:135:weight135\] .*existingStem id2647:.*verticesBefore 255 edgesBefore 347 allocatorBefore 2659 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q8reusematch headInterId 1388 sourceHeadId 1471 sourceCorner BL sourceSide LEFT relationGrade 3fefa1c8c523138f stem id2647:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q8result headInterId 1388 linkedStem id2647:.*reusedExisting true applied grade3feb7adfb837fb8d:dxbfbae2955082830c:.*verticesBefore 255 verticesAfter 255 edgesBefore 347 edgesAfter 348 allocatorBefore 2659 allocatorAfter 2659 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q8reuse headInterId 1388 lastIndex 0 selectedStem - terminal ReturnedFromReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 2 queueIndex 8 headX 56 headSig 78 headInterId 1388 grade 3fe14688c5a3a00b append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=true:branch=BottomOnly\] returned true .*sideChanges \[x56:sig78:LEFT:false:true->true:true\] .*sigEdgesBefore 347 sigEdgesAfter 348 .*allocatorBefore 2659 allocatorAfter 2659$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-2 queue-8 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system1-order21.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order21.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order21.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "3ad18d6e2db7b60980a27deef414bf54ac86df1fdfc127b26539172b4665e918" ] || \
        [ "$base_fixture_sha" != "457f8f28ca9a62fd085b27d5e574b1ff71a9f2f211dec9a0a82d4c30432c20d5" ] || \
        [ "$base_retarget_transform_sha" != "a9daae9d492b63c9b9e091f0522bf7e42d270ef113a6f63f5a323066764c0d01" ]; then
    echo "Cucaracha system-2 queue-8 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order8.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-2 phase-two queue 8.'
    echo '# schema: stems-head-phase-two-cucaracha-system2-order8-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas2q8summary schema stems-head-phase-two-cucaracha-system2-order8-v1 page cucaracha.png#1 system 2 rows 8 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder21RunnerSha256 $base_runner_sha baseOrder21FixtureSha256 $base_fixture_sha baseOrder21RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem2PhaseTwoOrder8ReusedStemAppend javaEvidence ReturnedAfterSystem2RetryIndex8"
} > "$out"
echo "wrote $out"
