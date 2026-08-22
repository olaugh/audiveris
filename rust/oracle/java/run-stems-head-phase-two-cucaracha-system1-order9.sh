#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java queue 9's x42 append and queues 10-15's prelinked returns.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s1-q9.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
order6_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order6.transform.awk"
order7_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order7.transform.awk"
order8_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order8.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order9.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker_order6="$tmp_dir/HeadLinker-order6.java"
head_linker_order7="$tmp_dir/HeadLinker-order7.java"
head_linker_order8="$tmp_dir/HeadLinker-order8.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/cucaracha.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$order6_transform" "$head_linker_x14" > "$head_linker_order6"
awk -f "$order7_transform" "$head_linker_order6" > "$head_linker_order7"
awk -f "$order8_transform" "$head_linker_order7" > "$head_linker_order8"
awk -f "$retarget_transform" "$head_linker_order8" > "$head_linker"

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
grep -E '^(stemsheadphase2cucarachas1q9|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex (9|10|11|12|13|14|15) )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas1q9|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex (9|10|11|12|13|14|15) )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-1 queues 9-15 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 13 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 1 heads 142 queueSize 22 .*sigVertices 232 sigEdges 337 systemStems 38 allocator 2216$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q9frontier headInterId 1091 corner BL hSide LEFT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 3 .*glyphs 1 selected \[id200:969:621:3:128:weight320\] .*existingStem id2201:.*verticesBefore 232 edgesBefore 339 allocatorBefore 2216 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q9reusematch headInterId 1091 sourceHeadId 1127 sourceCorner BL sourceSide LEFT relationGrade 3fea30815bc44681 stem id2201:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q9result headInterId 1091 linkedStem id2201:.*reusedExisting true applied grade3fec68779e72330d:dxbfb5b2b864a38925:.*verticesBefore 232 verticesAfter 232 edgesBefore 339 edgesAfter 340 allocatorBefore 2216 allocatorAfter 2216 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 9 headX 42 headSig 73 headInterId 1091 grade 3fde87cd8a51e87d append true .*returned true .*sideChanges \[x42:sig73:LEFT:false:true->true:true\] .*sigEdgesBefore 339 sigEdgesAfter 340 .*allocatorBefore 2216 allocatorAfter 2216$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 10 headX 133 headSig 111 headInterId 1169 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 11 headX 58 headSig 118 headInterId 1183 .*returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 12 headX 125 headSig 126 headInterId 1199 .*returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 13 headX 138 headSig 128 headInterId 1203 .*returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 14 headX 48 headSig 116 headInterId 1179 .*returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 15 headX 17 headSig 112 headInterId 1171 .*returned true .*sideChanges \[\] .*sigEdgesBefore 340 sigEdgesAfter 340 ' "$tmp_dir/rows1"; then
    echo "Cucaracha system-1 queues 9-15 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system1-order8.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order8.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order8.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "e1fcae89507e31a8f5d43d2c0338e0f8ac3589c282fe02050c404e8248f71080" ] || \
        [ "$base_fixture_sha" != "475c4346f01be8331218cdbfb1f335c8df126ea79d9ec883b8006325869b1e3e" ] || \
        [ "$base_retarget_transform_sha" != "5722bbdc0861b87f04505aab5d08eed64add7cf3ff54b567a4a5435b2f24de7e" ]; then
    echo "Cucaracha queues 9-15 strict predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
base_transform_sha=$(shasum -a 256 "$base_transform" | awk '{print $1}')
order6_transform_sha=$(shasum -a 256 "$order6_transform" | awk '{print $1}')
order7_transform_sha=$(shasum -a 256 "$order7_transform" | awk '{print $1}')
order8_transform_sha=$(shasum -a 256 "$order8_transform" | awk '{print $1}')
retarget_transform_sha=$(shasum -a 256 "$retarget_transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order9.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-1 phase-two queues 9-15.'
    echo '# schema: stems-head-phase-two-cucaracha-system1-order9-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas1q9summary schema stems-head-phase-two-cucaracha-system1-order9-v1 page cucaracha.png#1 system 1 rows 13 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha order6TransformSourceSha256 $order6_transform_sha order7TransformSourceSha256 $order7_transform_sha order8TransformSourceSha256 $order8_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder8RunnerSha256 $base_runner_sha baseOrder8FixtureSha256 $base_fixture_sha baseOrder8RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem1PhaseTwoOrder10AppendAndQueues11Through16Prelinked javaEvidence ReturnedBeforeSystem1RetryIndex16"
} > "$out"
echo "wrote $out"
