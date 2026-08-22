#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java Hove system-5 queue 1's reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-hove-s5-q1.XXXXXX)
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
system2_order10_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order10.transform.awk"
system2_order16_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order16.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-hove-system5-order1.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/hove.png"

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
    awk -f "$system2_order10_transform" |
    awk -f "$system2_order16_transform" |
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
grep -E '^(stemsheadphase2hoves5q1|stemsheadphase2baseline .* system 5 |stemsheadphase2retry .* system 5 queueIndex 1 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2hoves5q1|stemsheadphase2baseline .* system 5 |stemsheadphase2retry .* system 5 queueIndex 1 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Hove system-5 queue-1 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 5 ] || \
        ! grep -q '^stemsheadphase2baseline page hove.png#1 system 5 heads 71 queueSize 2 queue \[x65:sig46:id1709,x67:sig52:id1721\] sigVertices 136 sigEdges 159 systemStems 32 allocator 2937$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2hoves5q1reusematch headInterId 1721 sourceHeadId 1709 sourceCorner TR sourceSide RIGHT relationGrade 3fefee72d76d6b41 stem id2931:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2hoves5q1reuse headInterId 1721 lastIndex 1 selectedStem id2931:.*terminal ReturnedFromReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2hoves5q1commit headInterId 1721 linkedStem id2931:.*relationRows \[head#1721-Clnk-TR:grade3fefab115e072942:dx3f6fc4514038cccd:.*head#1709-Clnk-TR:grade3fefee72d76d6940:.*\] vertices 136 edges 160 allocator 2937 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page hove.png#1 system 5 queueIndex 1 headX 67 headSig 52 headInterId 1721 grade 3fc74ccccccccccd append true .*decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=true:bottom=false:branch=TopOnly\] returned true .*sideChanges \[x67:sig52:RIGHT:false:true->true:true\] .*sigEdgesBefore 159 sigEdgesAfter 160 .*allocatorBefore 2937 allocatorAfter 2937$' "$tmp_dir/rows1"; then
    echo "Hove system-5 queue-1 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system3-order19.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system3-order19.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system3-order19.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "26af234811b815d1e2012311838045cd80adec4c3d67c3dd19c732160600fb34" ] || \
        [ "$base_fixture_sha" != "a4ede84ed937da65006924da3b3de35e24d33dd229d9391aae136e436b1477ff" ] || \
        [ "$base_retarget_transform_sha" != "35f69316834081b0e6f8354e0bfbb856952930941652ccd04db2ee23dcc1d432" ]; then
    echo "Hove system-5 queue-1 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-hove-system5-order1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Hove system-5 phase-two queue 1.'
    echo '# schema: stems-head-phase-two-hove-system5-order1-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2hoves5q1summary schema stems-head-phase-two-hove-system5-order1-v1 page hove.png#1 system 5 rows 5 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseBoundary205RunnerSha256 $base_runner_sha baseBoundary205FixtureSha256 $base_fixture_sha baseBoundary205RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope HoveSystem5PhaseTwoOrder1ReusedStemAppend javaEvidence ReturnedAfterSystem5RetryIndex1"
} > "$out"
echo "wrote $out"
