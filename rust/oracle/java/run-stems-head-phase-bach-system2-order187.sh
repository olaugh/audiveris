#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-187 existing-stem C-link replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q187.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
base_probe="$script_dir/StemsHeadMultiBeamCLinkPageProbe.java"
transform="$script_dir/stems-head-phase-bach-system2-order187.transform.awk"
probe="$tmp_dir/StemsHeadBachSystem2Order187CLinkProbe.java"
init="$script_dir/stems-head-phase-bach-system2-order187.init.gradle"
base_runner="$script_dir/run-stems-head-phase-bach-system2-order186.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-bach-system2-order186.txt"
input="$repo_root/data/examples/BachInvention5.jpg"

base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_probe_sha" != "72e85d0de1838664db221fa890917b83a1140bf6ee5ea99b0a1f6bc1839fec33" ] || \
        [ "$base_runner_sha" != "38b6854c8a1a58cc4e463f119bf60317a5fc4501cc22bd21c091850e3cb9558a" ] || \
        [ "$base_fixture_sha" != "729145d6ecd237c7cf420323f980384e119efac24eed97a2393bc1a91dbba8b9" ]; then
    echo "strict Bach system-2 queue-186 predecessor pins differ" >&2
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
            -Porder187Probe="$probe" -Porder187Page="$input" \
            -I "$init" :app:stemsHeadBachSystem2Order187CLinkProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsheadbachs2q187' "$pass_one" > "$rows_one"
grep '^stemsheadbachs2q187' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Bach system-2 queue-187 passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadbachs2q187frontier ' "$rows_one")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadbachs2q187result ' "$rows_one")" -ne 1 ] || \
        ! grep -q 'headOrder 187 headX 178 headSig 52 .*stemProfile 0 cAlias h:178:LEFT:BOTTOM ' "$rows_one" || \
        ! grep -q 'lastIndex 2 maxIndex 2 .*relations 3 .*glyphs 1 selected \[id535:g:1565:761:4:51:bfa592048742fe646fa2232ed47770a736ef385632de0c92ded257dca1d77fda\]' "$rows_one" || \
        ! grep -q 'existingCandidateStem true .*existingBeamRelations \[beam:sig11:inter2031:b3:BeamStemRelation.*beam:sig14:inter2037:b3:BeamStemRelation' "$rows_one" || \
        ! grep -q '^stemsheadbachs2q187result .*headOrder 187 returned true undefs \[\] allocatorDelta 0 sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 593 sigEdgesAfter 594 systemStemsBefore 77 systemStemsAfter 77 addedVertices \[\] addedEdges \[source=headX178:target=existingCandidateStem:HeadStemRelation' "$rows_one" || \
        ! grep -q 'addedSystemStems \[\].*nextHeadOrder 188 nextHeadX 47 nextHeadSig 57 nextHeadInterId 3719 terminal ReturnedMultiBeamCLinkTransaction$' "$rows_one"; then
    echo "Bach system-2 queue-187 contract differs" >&2
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
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order187.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 187 existing-stem C-link.'
    echo '# schema: stems-head-phase-bach-system2-order187-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadbachs2q187summary schema stems-head-phase-bach-system2-order187-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseOrder186RunnerSha256 $base_runner_sha baseOrder186FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOneExistingStemMultiBeamCLink javaEvidence ReturnedBeforeHeadOrder188"
} > "$out"
echo "wrote $out"
