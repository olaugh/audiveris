#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Cucaracha system 1 queue 7's LEFT/BOTTOM reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s1-q7.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
order6_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order6.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order7.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker_order6="$tmp_dir/HeadLinker-order6.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/cucaracha.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$order6_transform" "$head_linker_x14" > "$head_linker_order6"
awk -f "$retarget_transform" "$head_linker_order6" > "$head_linker"

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
grep -E '^(stemsheadphase2cucarachas1q7|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 7 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas1q7|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 7 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-1 queue-7 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 1 heads 142 queueSize 22 queue \[x49:sig117:id1181,.*x52:sig75:id1095,.*sigVertices 232 sigEdges 337 systemStems 38 allocator 2216$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q7frontier headInterId 1095 corner BL hSide LEFT vSide BOTTOM .*lastIndex 1 maxIndex 1 relations 2 .*glyphs 1 selected \[id202:1067:622:4:125:weight320\] .*existingStem id2205:.*verticesBefore 232 edgesBefore 338 allocatorBefore 2216 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q7reusematch headInterId 1095 sourceHeadId 1185 sourceCorner BL sourceSide LEFT relationGrade 3feefc117520bff0 stem id2205:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q7reuse headInterId 1095 lastIndex 1 selectedStem id2205:.*terminal ReturnedFromReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q7commit headInterId 1095 linkedStem id2205:.*vertices 232 edges 339 allocator 2216 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q7result headInterId 1095 linkedStem id2205:.*reusedExisting true applied grade3fec70c151460e9d:dxbfb5839ad98ec925:.*verticesBefore 232 verticesAfter 232 edgesBefore 338 edgesAfter 339 allocatorBefore 2216 allocatorAfter 2216 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 7 headX 52 headSig 75 headInterId 1095 grade 3fe07a30cb045e42 append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=true:branch=BottomOnly\] returned true .*sideChanges \[x52:sig75:LEFT:false:true->true:true\] sigVerticesBefore 232 sigVerticesAfter 232 sigEdgesBefore 338 sigEdgesAfter 339 systemStemsBefore 38 systemStemsAfter 38 allocatorBefore 2216 allocatorAfter 2216$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-1 queue-7 phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system1-order6.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order6.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order6.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "0f47ae8f886f5ab28d69ef04c1214a69e16fc22493c59d8a442e44f11b0d8c18" ] || \
        [ "$base_fixture_sha" != "b8f37f279d7361fe92b6cf17c0b9e7376bc744db30e7fc162ce2e9df10669e07" ] || \
        [ "$base_retarget_transform_sha" != "69955a68e2acfada60b7e245dbb9eb636f1beb84d3020682364002179f61ced1" ]; then
    echo "Cucaracha queue-7 strict predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
base_transform_sha=$(shasum -a 256 "$base_transform" | awk '{print $1}')
order6_transform_sha=$(shasum -a 256 "$order6_transform" | awk '{print $1}')
retarget_transform_sha=$(shasum -a 256 "$retarget_transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order7.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-1 phase-two queue 7.'
    echo '# schema: stems-head-phase-two-cucaracha-system1-order7-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas1q7summary schema stems-head-phase-two-cucaracha-system1-order7-v1 page cucaracha.png#1 system 1 rows 7 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha order6TransformSourceSha256 $order6_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder6RunnerSha256 $base_runner_sha baseOrder6FixtureSha256 $base_fixture_sha baseOrder6RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem1PhaseTwoOrder7LeftReusedStemAppend javaEvidence ReturnedBeforeSystem1RetryIndex8"
} > "$out"
echo "wrote $out"
