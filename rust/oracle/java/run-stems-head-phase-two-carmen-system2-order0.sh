#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Carmen system 2's first phase-two append transaction.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-carmen-system2-order0.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
transform="$script_dir/stems-head-phase-two-carmen-system2-order0.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker="$tmp_dir/HeadLinker.java"
input="$repo_root/data/examples/carmen.png"

cp "$base_probe" "$probe"
awk -f "$transform" "$base_head_linker" > "$head_linker"

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
grep -E '^(stemsheadphase2carmens2q0|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 0 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2carmens2q0|stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 0 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Carmen system-2 queue-zero Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 4 ] || \
        ! grep -q '^stemsheadphase2baseline page carmen.png#1 system 2 heads 83 queueSize 9 queue \[x20:sig43:id2318,.*sigVertices 218 sigEdges 247 systemStems 33 allocator 3609$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2carmens2q0frontier headInterId 2318 corner TL hSide LEFT vSide TOP .*lastIndex -1 maxIndex 1 relations 1 .*glyphs 2 .*terminal ExpandMinusOne$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2carmens2q0frontier headInterId 2318 corner BR hSide RIGHT vSide BOTTOM .*lastIndex -1 maxIndex 1 relations 0 .*glyphs 1 .*terminal ExpandMinusOne$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page carmen.png#1 system 2 queueIndex 0 headX 20 headSig 43 headInterId 2318 .*returned false .*sigVerticesBefore 218 sigVerticesAfter 218 sigEdgesBefore 247 sigEdgesAfter 247 systemStemsBefore 33 systemStemsAfter 33 allocatorBefore 3609 allocatorAfter 3609$' "$tmp_dir/rows1"; then
    echo "Carmen system-2 queue-zero phase-two Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-carmen-system5-order62.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-carmen-system5-order62.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "c85a08ed03abe174bf8d2f3d6f478c59ac46efa15c4a9192411da748767cdcc0" ] || \
        [ "$base_fixture_sha" != "8af051e700bc0ef07ccf83f4037f0c358de67038073d8335676a050c166bb38f" ]; then
    echo "Carmen phase-two strict phase-one predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
head_linker_sha=$(shasum -a 256 "$base_head_linker" | awk '{print $1}')
transform_sha=$(shasum -a 256 "$transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
transformed_head_linker_sha=$(shasum -a 256 "$head_linker" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-carmen-system2-order0.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Carmen system-2 phase-two queue zero.'
    echo '# schema: stems-head-phase-two-carmen-system2-order0-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2carmens2q0summary schema stems-head-phase-two-carmen-system2-order0-v1 page carmen.png#1 system 2 rows 4 inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha headLinkerSourceSha256 $head_linker_sha transformSourceSha256 $transform_sha probeSourceSha256 $probe_sha transformedHeadLinkerSourceSha256 $transformed_head_linker_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseSystem5Order62RunnerSha256 $base_runner_sha baseSystem5Order62FixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedCarmenSystem2PhaseTwoOrder0FinalRelationNoLink javaEvidence ReturnedBeforeSystem2RetryIndex1"
} > "$out"
echo "wrote $out"
