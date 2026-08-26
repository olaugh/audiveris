#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Bach system 2 queue 9's RIGHT/BOTTOM reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-bach-s2-q9.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-bach-system2-order9.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/BachInvention5.jpg"

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
grep -E '^(stemsheadphase2bachs2q9|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 9 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2bachs2q9|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 9 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Bach system-2 queue-9 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2baseline page BachInvention5.jpg#1 system 2 heads 215 queueSize 15 queue \[x185:sig213:id4034,x159:sig164:id3939,x194:sig78:id3761,x163:sig170:id3951,x160:sig169:id3949,x162:sig168:id3947,x158:sig88:id3781,x152:sig90:id3784,x123:sig14:id3633,x149:sig18:id3641,x190:sig214:id4036,x98:sig136:id3878,x30:sig95:id3796,x118:sig211:id4031,x54:sig59:id3723\] sigVertices 394 sigEdges 600 systemStems 77 allocator 6815$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs2q9frontier headInterId 3641 corner BR hSide RIGHT vSide BOTTOM .*lastIndex 1 maxIndex 1 relations 2 .*glyphs 1 selected \[id497:1382:746:5:66:weight227\] .*existingStem id6786:.*verticesBefore 394 edgesBefore 601 allocatorBefore 6815 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs2q9reusematch headInterId 3641 sourceHeadId 3663 sourceCorner BL sourceSide LEFT relationGrade 3fe995c4c99d2d45 stem id6786:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs2q9reuse headInterId 3641 lastIndex 1 selectedStem id6786:.*terminal ReturnedFromReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs2q9commit headInterId 3641 linkedStem id6786:.*vertices 394 edges 602 allocator 6815 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs2q9result headInterId 3641 linkedStem id6786:.*reusedExisting true applied grade3fe3c8a4915237cf:dxbfcfa150d80c0969:.*verticesBefore 394 verticesAfter 394 edgesBefore 601 edgesAfter 602 allocatorBefore 6815 allocatorAfter 6815 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page BachInvention5.jpg#1 system 2 queueIndex 9 headX 149 headSig 18 headInterId 3641 grade 3fc9540d351f6384 append true .*decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=false:bottom=true:branch=BottomOnly\] returned true .*sideChanges \[x149:sig18:RIGHT:false:true->true:true\] sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 601 sigEdgesAfter 602 systemStemsBefore 77 systemStemsAfter 77 allocatorBefore 6815 allocatorAfter 6815$' "$tmp_dir/rows1"; then
    echo "Bach system-2 queue-9 phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-bach-system2-order8.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-bach-system2-order8.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "8c184f77b90aa0b102ca41daa3fb4c910ac0dbf3c8050bb994bbd60b72bc8be8" ] || \
        [ "$base_fixture_sha" != "cea0d597f1a5a77860f368424383536111d772ed58083f4d1d0331f43281daae" ]; then
    echo "Bach queue-9 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-bach-system2-order9.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 phase-two queue 9.'
    echo '# schema: stems-head-phase-two-bach-system2-order9-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2bachs2q9summary schema stems-head-phase-two-bach-system2-order9-v1 page BachInvention5.jpg#1 system 2 rows 7 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseBoundary250RunnerSha256 $base_runner_sha baseBoundary250FixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BachSystem2PhaseTwoOrder9RightReusedStemAppend javaEvidence ReturnedBeforeSystem2RetryIndex10"
} > "$out"
echo "wrote $out"
