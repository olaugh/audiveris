#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-200 right-side reconciliation replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q200.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseOneBachSystem2Order183Probe.java"
transform="$script_dir/stems-head-phase-bach-system2-order200.transform.awk"
probe="$tmp_dir/StemsHeadPhaseOneBachSystem2Order200Probe.java"
init="$script_dir/stems-head-phase-bach-system2-order200.init.gradle"
base_runner="$script_dir/run-stems-head-phase-bach-system2-order199.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-bach-system2-order199.txt"
input="$repo_root/data/examples/BachInvention5.jpg"
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_probe_sha" != "05c2ff1c14f4f2284ffb80560c82fce4b66c5d41f8debc21e2f5d91fe910a7bb" ] || \
   [ "$base_runner_sha" != "ee0c3d5f3c80b6044c19d09fa9326c6723a979b746e8e956ad44c9bb102091d4" ] || \
   [ "$base_fixture_sha" != "6531be3251776a021495d19bc84ecb66d87fb706d4d992dc28023d663aefdb2f" ]; then
    echo "strict Bach system-2 queue-199 predecessor pins differ" >&2; exit 1
fi
awk -f "$transform" "$base_probe" > "$probe"
run_pass(){ (cd "$repo_root"; env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q -Porder200Probe="$probe" -PphaseOneOrder200Page="$input" -I "$init" :app:stemsHeadPhaseBachSystem2Order200Probe) > "$1"; }
run_pass "$tmp_dir/warmup"; run_pass "$tmp_dir/pass1"; run_pass "$tmp_dir/pass2"
grep '^stemsheadbachs2q200' "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep '^stemsheadbachs2q200' "$tmp_dir/pass2" > "$tmp_dir/rows2"
cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2" || { echo "fresh queue-200 passes differ" >&2; exit 1; }
rows="$tmp_dir/rows1"
if [ "$(grep -c '^stemsheadbachs2q200profile ' "$rows")" -ne 4 ] || \
   [ "$(grep -c '^stemsheadbachs2q200result ' "$rows")" -ne 1 ] || \
   ! grep -q 'headOrder 200 headX 42 headSig 66 .*stemProfile 0 decisions \[LEFT:SkipClosed,RIGHT:SkipAlreadyLinked\]' "$rows" || \
   ! grep -q 'headOrder 200 headX 42 headSig 66 .*stemProfile 3 decisions \[LEFT:SkipClosed,RIGHT:SkipAlreadyLinked\]' "$rows" || \
   ! grep -q '^stemsheadbachs2q200result .*returned true undefs \[\] sideChanges \[x47:sig57:LEFT:true:false->true:true,x47:sig57:RIGHT:false:false->false:true\] incidents \[existingStem:headSideRIGHT:heads\[x42:sig66:sideRIGHT,x45:sig58:sideRIGHT,x47:sig57:sideLEFT,x48:sig38:sideLEFT\]\] ' "$rows" || \
   ! grep -q 'sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 597 sigEdgesAfter 597 systemStemsBefore 77 systemStemsAfter 77 allocatorUnchanged true nextHeadOrder 201 nextHeadX 168 nextHeadSig 171$' "$rows"; then
    echo "Bach system-2 queue-200 contract differs" >&2; cat "$rows" >&2; exit 1
fi
input_sha=$(shasum -a 256 "$input"|awk '{print $1}'); transform_sha=$(shasum -a 256 "$transform"|awk '{print $1}'); probe_sha=$(shasum -a 256 "$probe"|awk '{print $1}'); init_sha=$(shasum -a 256 "$init"|awk '{print $1}'); runner_sha=$(shasum -a 256 "$0"|awk '{print $1}'); body_sha=$(shasum -a 256 "$rows"|awk '{print $1}'); row_count=$(wc -l < "$rows"|tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order200.txt"
{
 echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 200 right-side four-head reconciliation.'
 echo '# schema: stems-head-phase-bach-system2-order200-v1'
 cat "$rows"
 printf '%s\n' "stemsheadbachs2q200summary schema stems-head-phase-bach-system2-order200-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha baseOrder199RunnerSha256 $base_runner_sha baseOrder199FixtureSha256 $base_fixture_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOneRightSideFourHeadReconciliation javaEvidence ReturnedBeforeHeadOrder201"
} > "$out"
echo "wrote $out"
