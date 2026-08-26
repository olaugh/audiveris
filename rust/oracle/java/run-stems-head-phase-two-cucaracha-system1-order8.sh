#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Cucaracha system 1 queue 8's prelinked mutation-free return.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-cucaracha-s1-q8.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
order6_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order6.transform.awk"
order7_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order7.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order8.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker_order6="$tmp_dir/HeadLinker-order6.java"
head_linker_order7="$tmp_dir/HeadLinker-order7.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/cucaracha.png"

cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$order6_transform" "$head_linker_x14" > "$head_linker_order6"
awk -f "$order7_transform" "$head_linker_order6" > "$head_linker_order7"
awk -f "$retarget_transform" "$head_linker_order7" > "$head_linker"

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
grep -E '^(stemsheadphase2cucarachas1q8|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 8 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2cucarachas1q8|stemsheadphase2baseline .* system 1 |stemsheadphase2retry .* system 1 queueIndex 8 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Cucaracha system-1 queue-8 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 2 ] || \
        ! grep -q '^stemsheadphase2baseline page cucaracha.png#1 system 1 heads 142 queueSize 22 queue \[x49:sig117:id1181,.*x119:sig110:id1166,.*sigVertices 232 sigEdges 337 systemStems 38 allocator 2216$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page cucaracha.png#1 system 1 queueIndex 8 headX 119 headSig 110 headInterId 1166 grade 3fdf2b6ee1da3008 append true sidesBefore \[LEFT:true:true,RIGHT:false:true\] decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] returned true sidesAfter \[LEFT:true:true,RIGHT:false:true\] undefs \[LEFT\] sideChanges \[\] sigVerticesBefore 232 sigVerticesAfter 232 sigEdgesBefore 339 sigEdgesAfter 339 systemStemsBefore 38 systemStemsAfter 38 allocatorBefore 2216 allocatorAfter 2216$' "$tmp_dir/rows1"; then
    echo "Cucaracha system-1 queue-8 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-cucaracha-system1-order7.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order7.txt"
base_retarget_transform="$script_dir/stems-head-phase-two-cucaracha-system1-order7.transform.awk"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
base_retarget_transform_sha=$(shasum -a 256 "$base_retarget_transform" | awk '{print $1}')
if [ "$base_runner_sha" != "2b16fbd09aab743733badbd53bda0aeff960800f518b9e52c4edaee24443aa3d" ] || \
        [ "$base_fixture_sha" != "76482e205aff34826b96d1a923f1a069b8927283b10bc286cb2478e0e0e1f077" ] || \
        [ "$base_retarget_transform_sha" != "009d2479d330f754c5603f0051ea40631ded4a0752798910aa2bea78707bfcd0" ]; then
    echo "Cucaracha queue-8 strict predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
base_transform_sha=$(shasum -a 256 "$base_transform" | awk '{print $1}')
order6_transform_sha=$(shasum -a 256 "$order6_transform" | awk '{print $1}')
order7_transform_sha=$(shasum -a 256 "$order7_transform" | awk '{print $1}')
retarget_transform_sha=$(shasum -a 256 "$retarget_transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-cucaracha-system1-order8.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-1 phase-two queue 8.'
    echo '# schema: stems-head-phase-two-cucaracha-system1-order8-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2cucarachas1q8summary schema stems-head-phase-two-cucaracha-system1-order8-v1 page cucaracha.png#1 system 1 rows 2 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha baseTransformSourceSha256 $base_transform_sha order6TransformSourceSha256 $order6_transform_sha order7TransformSourceSha256 $order7_transform_sha retargetTransformSourceSha256 $retarget_transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseOrder7RunnerSha256 $base_runner_sha baseOrder7FixtureSha256 $base_fixture_sha baseOrder7RetargetTransformSha256 $base_retarget_transform_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem1PhaseTwoQueuePosition8PrelinkedNoOp javaEvidence ReturnedBeforeSystem1RetryIndex9"
} > "$out"
echo "wrote $out"
