#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java system-2 queue 9's reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s2-q9.XXXXXX)
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
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order9.transform.awk"
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
grep -E '^(stemsheadphase2cucarachas2q9|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 9 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas2q9|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 9 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-2 queue-9 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 2 heads 150 queueSize 24 .*sigVertices 255 sigEdges 347 systemStems 43 allocator 2659$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q9frontier headInterId 1400 corner BL hSide LEFT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 3 .*glyphs 1 selected \[id251:2159:1213:4:130:weight273\] .*existingStem id2652:.*verticesBefore 255 edgesBefore 348 allocatorBefore 2659 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q9reusematch headInterId 1400 sourceHeadId 1438 sourceCorner BL sourceSide LEFT relationGrade 3feada1cc7b0da2e stem id2652:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas2q9result headInterId 1400 linkedStem id2652:.*reusedExisting true applied grade3fed051e7bce623f:dxbfb22f195fe0a492:.*verticesBefore 255 verticesAfter 255 edgesBefore 348 edgesAfter 349 allocatorBefore 2659 allocatorAfter 2659 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 2 queueIndex 9 headX 132 headSig 84 headInterId 1400 grade 3fe0ce6b7db6fd48 append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=true:branch=BottomOnly\] returned true .*sideChanges \[x132:sig84:LEFT:false:true->true:true\] .*sigEdgesBefore 348 sigEdgesAfter 349 .*allocatorBefore 2659 allocatorAfter 2659$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-2 queue-9 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system2-order8.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order8.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order8.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "e862cb9e24ca33a0f9381b1990b25a3a59c607337b60720930871b93936e5b7d" ] || \
        [ "$base_fixture_sha" != "5290a3261024d312098f1671c536df2bf2e89721e9b6713574c25d95107a58b5" ] || \
        [ "$base_retarget_transform_sha" != "3f696415a4450338b60c29d343aaccd7ba88772868abaf2deac3ea1c46272cbf" ]; then
    echo "Cucaracha system-2 queue-9 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order9.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-2 phase-two queue 9.'
    echo '# schema: stems-head-phase-two-cucaracha-system2-order9-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas2q9summary schema stems-head-phase-two-cucaracha-system2-order9-v1 page cucaracha.png#1 system 2 rows 7 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder8RunnerSha256 $base_runner_sha baseOrder8FixtureSha256 $base_fixture_sha baseOrder8RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem2PhaseTwoOrder9ReusedStemAppend javaEvidence ReturnedAfterSystem2RetryIndex9"
} > "$out"
echo "wrote $out"
