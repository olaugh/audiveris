#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java queue 21's terminal system-1 reused-stem append.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s1-q21.XXXXXX)
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
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order21.transform.awk"
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
grep -E '^(stemsheadphase2cucarachas1q21|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 21 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas1q21|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 21 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-1 queue-21 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2cucarachas1q21frontier headInterId 1077 corner BL hSide LEFT vSide BOTTOM .*lastIndex 2 maxIndex 2 relations 3 .*glyphs 1 selected \[id198:1350:624:3:129:weight268\] .*existingStem id2208:.*verticesBefore 232 edgesBefore 343 allocatorBefore 2216 terminal ReadyForHeadCreateStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q21reusematch headInterId 1077 sourceHeadId 1155 sourceCorner BL sourceSide LEFT relationGrade 3fea00080e226d9e stem id2208:.*terminal SelectedReuseStem$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2cucarachas1q21result headInterId 1077 linkedStem id2208:.*reusedExisting true applied grade3fe5554e97ce0182:dx3fbd29be97edf3cf:.*verticesBefore 232 verticesAfter 232 edgesBefore 343 edgesAfter 344 allocatorBefore 2216 allocatorAfter 2216 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 21 headX 71 headSig 66 headInterId 1077 grade 3fc7471ed4f838da append true .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\] returned true .*sideChanges \[x71:sig66:LEFT:false:true->true:true\] .*sigEdgesBefore 343 sigEdgesAfter 344 .*allocatorBefore 2216 allocatorAfter 2216$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-1 queue-21 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system1-order19.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order19.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order19.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "29733c6d93a1d5642d24cfe742b9d3f9314230818ca5919acd1a5b21552e74a7" ] || \
        [ "$base_fixture_sha" != "59f27d582bda0a3a144a68b5dc37a0ac586ad89de19c64d993aed15cfdbed2c4" ] || \
        [ "$base_retarget_transform_sha" != "4b3029fec45ef99cdd24804ec7e88ac04578a62f2e1e71e127088ee5554c56ba" ]; then
    echo "Cucaracha queue-21 strict predecessor drifted" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order21.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-1 terminal phase-two queue 21.'
    echo '# schema: stems-head-phase-two-cucaracha-system1-order21-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas1q21summary schema stems-head-phase-two-cucaracha-system1-order21-v1 page cucaracha.png#1 system 1 rows 7 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder19RunnerSha256 $base_runner_sha baseOrder19FixtureSha256 $base_fixture_sha baseOrder19RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem1PhaseTwoOrder22TerminalAppend javaEvidence ReturnedAfterSystem1RetryIndex21"
} > "$out"
echo "wrote $out"
