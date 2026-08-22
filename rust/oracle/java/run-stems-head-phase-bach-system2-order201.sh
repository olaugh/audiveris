#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-201 existing-stem C-link replay.
set -eu
if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2; exit 2
fi
release_field(){ awk -F= -v name="$1" '$1 == name { value=$2; gsub(/^"|"$/, "", value); print value }' "$JAVA_HOME/release"; }
if [ "$(release_field IMPLEMENTOR)" != "Eclipse Adoptium" ] || \
   [ "$(release_field IMPLEMENTOR_VERSION)" != "Temurin-25.0.3+9" ] || \
   [ "$(release_field JAVA_RUNTIME_VERSION)" != "25.0.3+9-LTS" ] || \
   [ "$(release_field OS_NAME)" != "Darwin" ] || \
   [ "$(release_field OS_ARCH)" != "aarch64" ] || \
   [ "$(release_field JVM_VARIANT)" != "Hotspot" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS aarch64 HotSpot" >&2; exit 2
fi
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q201.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadMultiBeamCLinkPageProbe.java"
transform="$script_dir/stems-head-phase-bach-system2-order201.transform.awk"
probe="$tmp_dir/StemsHeadBachSystem2Order201CLinkProbe.java"
init="$script_dir/stems-head-phase-bach-system2-order201.init.gradle"
base_runner="$script_dir/run-stems-head-phase-bach-system2-order200.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-bach-system2-order200.txt"
input="$repo_root/data/examples/BachInvention5.jpg"
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_probe_sha" != "72e85d0de1838664db221fa890917b83a1140bf6ee5ea99b0a1f6bc1839fec33" ] || \
   [ "$base_runner_sha" != "6168f1942b210ba6c36c1f884a12b527128d3adc53e4b7e021a8532e8092b7a0" ] || \
   [ "$base_fixture_sha" != "e3821fb3ec68f13b384cbf96d4f94817e21cfd83172e484c01b134097be619b2" ]; then
    echo "strict Bach system-2 queue-200 predecessor pins differ" >&2; exit 1
fi
awk -f "$transform" "$base_probe" > "$probe"
run_pass(){ (cd "$repo_root"; env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q -Porder201Probe="$probe" -Porder201Page="$input" -I "$init" :app:stemsHeadBachSystem2Order201CLinkProbe) > "$1"; }
run_pass "$tmp_dir/warmup"; run_pass "$tmp_dir/pass1"; run_pass "$tmp_dir/pass2"
grep '^stemsheadbachs2q201' "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep '^stemsheadbachs2q201' "$tmp_dir/pass2" > "$tmp_dir/rows2"
cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2" || { echo "fresh queue-201 passes differ" >&2; exit 1; }
rows="$tmp_dir/rows1"
if [ "$(grep -c '^stemsheadbachs2q201frontier ' "$rows")" -ne 1 ] || \
   [ "$(grep -c '^stemsheadbachs2q201result ' "$rows")" -ne 1 ] || \
   ! grep -q 'headOrder 201 headX 168 headSig 171 headInterId 3953 .*cAlias h:168:LEFT:TOP .*lastIndex 0 maxIndex 0' "$rows" || \
   ! grep -q 'relations 1 .*glyphs 1 selected \[candidateGlyph:g:1481:878:5:82:1d8993673315f6376fb30a5878b5eb8283eae21de1f5c9b85141b6bfa2d25a81\]' "$rows" || \
   ! grep -q 'candidateIdBefore 471 .*existingCandidateStem true .*existingBeamRelations \[\].*terminal ReadyForMultiBeamCLink$' "$rows" || \
   ! grep -q '^stemsheadbachs2q201result .*returned true undefs \[\] allocatorDelta 0 sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 597 sigEdgesAfter 598 systemStemsBefore 77 systemStemsAfter 77 addedVertices \[\] addedEdges \[source=headX168:target=existingCandidateStem:HeadStemRelation' "$rows" || \
   ! grep -q 'addedSystemStems \[\].*nextHeadOrder 202 nextHeadX 64 nextHeadSig 61 nextHeadInterId 3727 terminal ReturnedMultiBeamCLinkTransaction$' "$rows"; then
    echo "Bach system-2 queue-201 contract differs" >&2; cat "$rows" >&2; exit 1
fi
input_sha=$(shasum -a 256 "$input"|awk '{print $1}'); transform_sha=$(shasum -a 256 "$transform"|awk '{print $1}'); probe_sha=$(shasum -a 256 "$probe"|awk '{print $1}'); init_sha=$(shasum -a 256 "$init"|awk '{print $1}'); runner_sha=$(shasum -a 256 "$0"|awk '{print $1}'); body_sha=$(shasum -a 256 "$rows"|awk '{print $1}'); row_count=$(wc -l < "$rows"|tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order201.txt"
{
 echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 201 existing-stem C-link.'
 echo '# schema: stems-head-phase-bach-system2-order201-v1'
 cat "$rows"
 printf '%s\n' "stemsheadbachs2q201summary schema stems-head-phase-bach-system2-order201-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseOrder200RunnerSha256 $base_runner_sha baseOrder200FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOneExistingStemSingleHeadCLink javaEvidence ReturnedBeforeHeadOrder202"
} > "$out"
echo "wrote $out"
