#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-188 multi-head existing-stem replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q188.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
base_probe="$script_dir/StemsHeadMultiBeamCLinkPageProbe.java"
transform="$script_dir/stems-head-phase-bach-system2-order188.transform.awk"
probe="$tmp_dir/StemsHeadBachSystem2Order188CLinkProbe.java"
init="$script_dir/stems-head-phase-bach-system2-order188.init.gradle"
base_runner="$script_dir/run-stems-head-phase-bach-system2-order187.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-bach-system2-order187.txt"
input="$repo_root/data/examples/BachInvention5.jpg"

base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_probe_sha" != "72e85d0de1838664db221fa890917b83a1140bf6ee5ea99b0a1f6bc1839fec33" ] || \
        [ "$base_runner_sha" != "b5f3635b1c364ead19243eb9c25d5388e558ee0ee268e54c63dc7a3c69111fad" ] || \
        [ "$base_fixture_sha" != "62acbdbea32f228e829d9b49cec8b795308ab77307aea358091e446daf8820c8" ]; then
    echo "strict Bach system-2 queue-187 predecessor pins differ" >&2
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
            -Porder188Probe="$probe" -Porder188Page="$input" \
            -I "$init" :app:stemsHeadBachSystem2Order188CLinkProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsheadbachs2q188' "$pass_one" > "$rows_one"
grep '^stemsheadbachs2q188' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Bach system-2 queue-188 passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadbachs2q188frontier ' "$rows_one")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadbachs2q188result ' "$rows_one")" -ne 1 ] || \
        ! grep -q 'headOrder 188 headX 47 headSig 57 .*stemProfile 0 cAlias h:47:LEFT:TOP ' "$rows_one" || \
        ! grep -q 'lastIndex 1 maxIndex 1 .*relations 2 .*glyphs 1 selected \[id485:g:540:722:5:73:0b7943a4496a48853182077400a16c28ba226f29a865c861e6e340210226a311\]' "$rows_one" || \
        ! grep -q 'existingCandidateStem true .*existingBeamRelations \[\]' "$rows_one" || \
        ! grep -q '^stemsheadbachs2q188result .*headOrder 188 returned true undefs \[\] allocatorDelta 0 sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 594 sigEdgesAfter 596 systemStemsBefore 77 systemStemsAfter 77 addedVertices \[\] addedEdges \[source=headX47:target=existingCandidateStem:HeadStemRelation.*source=headX48:target=existingCandidateStem:HeadStemRelation' "$rows_one" || \
        ! grep -q 'addedSystemStems \[\].*nextHeadOrder 189 nextHeadX 164 nextHeadSig 51 nextHeadInterId 3707 terminal ReturnedMultiBeamCLinkTransaction$' "$rows_one"; then
    echo "Bach system-2 queue-188 contract differs" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order188.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 188 multi-head existing-stem C-link.'
    echo '# schema: stems-head-phase-bach-system2-order188-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadbachs2q188summary schema stems-head-phase-bach-system2-order188-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseOrder187RunnerSha256 $base_runner_sha baseOrder187FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOneExistingStemMultiHeadCLink javaEvidence ReturnedBeforeHeadOrder189"
} > "$out"
echo "wrote $out"
