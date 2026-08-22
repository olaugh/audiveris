#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-185 prelinked reconciliation replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q185.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
base_probe="$script_dir/StemsHeadPhaseOneBachSystem2Order183Probe.java"
transform="$script_dir/stems-head-phase-bach-system2-order185.transform.awk"
probe="$tmp_dir/StemsHeadPhaseOneBachSystem2Order185Probe.java"
init="$script_dir/stems-head-phase-bach-system2-order185.init.gradle"
base_runner="$script_dir/run-stems-head-phase-bach-system2-order184.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-bach-system2-order184.txt"
input="$repo_root/data/examples/BachInvention5.jpg"

base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_probe_sha" != "05c2ff1c14f4f2284ffb80560c82fce4b66c5d41f8debc21e2f5d91fe910a7bb" ] || \
        [ "$base_runner_sha" != "16c64b513e86df490b141cfa6189d3f80ac76c18ea483ae1d4d81325a2a3b805" ] || \
        [ "$base_fixture_sha" != "7a77078895e488d1be44d0f57c272d0d022fc278c86ba13d94925f8ff111aebe" ]; then
    echo "strict Bach system-2 queue-184 predecessor pins differ" >&2
    exit 1
fi

awk -f "$transform" "$base_probe" > "$probe"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -Porder185Probe="$probe" -PphaseOneOrder185Page="$input" \
            -I "$init" :app:stemsHeadPhaseBachSystem2Order185Probe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsheadbachs2q185' "$pass_one" > "$rows_one"
grep '^stemsheadbachs2q185' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Bach system-2 queue-185 passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadbachs2q185profile ' "$rows_one")" -ne 4 ] || \
        [ "$(grep -c '^stemsheadbachs2q185result ' "$rows_one")" -ne 1 ] || \
        ! grep -q 'headOrder 185 headX 192 headSig 76 .*stemProfile 0 decisions \[LEFT:SkipAlreadyLinked,RIGHT:SkipClosed\]' "$rows_one" || \
        ! grep -q 'headOrder 185 headX 192 headSig 76 .*stemProfile 3 decisions \[LEFT:SkipAlreadyLinked,RIGHT:SkipClosed\]' "$rows_one" || \
        ! grep -q '^stemsheadbachs2q185result .*returned true undefs \[\] sideChanges \[\] incidents \[existingStem:headSideLEFT:heads\[x191:sig75:sideLEFT,x192:sig76:sideLEFT,x193:sig77:sideLEFT\]\] ' "$rows_one" || \
        ! grep -q 'sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 593 sigEdgesAfter 593 systemStemsBefore 77 systemStemsAfter 77 allocatorUnchanged true nextHeadOrder 186 nextHeadX 190 nextHeadSig 214$' "$rows_one"; then
    echo "Bach system-2 queue-185 contract differs" >&2
    cat "$rows_one" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
transform_sha=$(shasum -a 256 "$transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
row_count=$(wc -l < "$rows_one" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order185.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 185 prelinked reconciliation.'
    echo '# schema: stems-head-phase-bach-system2-order185-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadbachs2q185summary schema stems-head-phase-bach-system2-order185-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseOrder184RunnerSha256 $base_runner_sha baseOrder184FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOnePrelinkedZeroChangeReconciliation javaEvidence ReturnedBeforeHeadOrder186"
} > "$out"
echo "wrote $out"
