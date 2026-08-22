#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java system-3 queue 19's reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s3-q19.XXXXXX)
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
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system3-order19.transform.awk"
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
grep -E '^(stemsheadphase2cucarachas3q19|stemsheadphase2baseline .* system 3 |stemsheadphase2retry .* system 3 queueIndex 19 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas3q19|stemsheadphase2baseline .* system 3 |stemsheadphase2retry .* system 3 queueIndex 19 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-3 queue-19 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 3 heads 113 queueSize 20 .*sigVertices 198 sigEdges 250 systemStems 34 allocator 3009$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas3q19frontier headInterId 1555 corner BL hSide LEFT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 2 .*glyphs 2 selected \[id317:834:1560:4:91:weight307,id2868:834:1560:4:91:weight198\] .*existingStem id2989:.*verticesBefore 198 edgesBefore 250 allocatorBefore 3009 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas3q19reusematch headInterId 1555 sourceHeadId 1632 sourceCorner BL sourceSide LEFT relationGrade 3fedecef2ef1a8ba stem id2989:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas3q19result headInterId 1555 linkedStem id2989:.*reusedExisting true applied grade3fe4e1c61700dadc:dx3fbe433d3ee06618:.*verticesBefore 198 verticesAfter 198 edgesBefore 250 edgesAfter 251 allocatorBefore 3009 allocatorAfter 3009 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 3 queueIndex 19 headX 37 headSig 11 headInterId 1555 grade 3fc5874e6adca3c0 append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\] returned true .*sideChanges \[x37:sig11:LEFT:false:true->true:true\] .*sigEdgesBefore 250 sigEdgesAfter 251 .*allocatorBefore 3009 allocatorAfter 3009$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-3 queue-19 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system2-order16.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system2-order16.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system2-order16.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "0307f76f0da438d3609c1dcaa602656eca732de9fd377bd25325e94c78ffea77" ] || \
        [ "$base_fixture_sha" != "200afe8ef54faf6a11ecf094bc2394b485dee7f0eb6ed68aa632e4e4bdbbdd5d" ] || \
        [ "$base_retarget_transform_sha" != "bc9205d1e88c653d7d7cb553cc525d559a69e87b4736efe615c975daf82ae425" ]; then
    echo "Cucaracha system-3 queue-19 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system3-order19.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-3 phase-two queue 19.'
    echo '# schema: stems-head-phase-two-cucaracha-system3-order19-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas3q19summary schema stems-head-phase-two-cucaracha-system3-order19-v1 page cucaracha.png#1 system 3 rows 7 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseSystem2Order16RunnerSha256 $base_runner_sha baseSystem2Order16FixtureSha256 $base_fixture_sha baseSystem2Order16RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem3PhaseTwoOrder19ReusedStemAppend javaEvidence ReturnedAfterSystem3RetryIndex19"
} > "$out"
echo "wrote $out"
