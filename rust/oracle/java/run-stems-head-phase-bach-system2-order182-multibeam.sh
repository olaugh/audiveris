#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-2 HEADS queue-182 multi-beam C-link replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach-s2-q182.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
probe="$script_dir/StemsHeadMultiBeamCLinkPageProbe.java"
init="$script_dir/stems-head-multibeam-bach.init.gradle"
input="$repo_root/data/examples/BachInvention5.jpg"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PmultiBeamPage="$input" \
            -I "$init" :app:stemsHeadMultiBeamBachProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsheadmultibeam' "$pass_one" > "$rows_one"
grep '^stemsheadmultibeam' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Bach system-2 multi-beam passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadmultibeamfrontier ' "$rows_one")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadmultibeamresult ' "$rows_one")" -ne 1 ] || \
        ! grep -q 'system 2 headOrder 182 headX 138 headSig 149 .*stemProfile 0 .*lastIndex 2 maxIndex 2 ' "$rows_one" || \
        ! grep -q 'relations 3 .*glyphs 1 .*existingCandidateStem true ' "$rows_one" || \
        ! grep -q '^stemsheadmultibeamresult .*headOrder 182 returned true .*allocatorDelta 0 .*sigVerticesBefore 394 sigVerticesAfter 394 .*sigEdgesBefore 592 sigEdgesAfter 593 .*systemStemsBefore 77 systemStemsAfter 77 ' "$rows_one"; then
    echo "Bach system-2 queue-182 multi-beam contract differs" >&2
    cat "$rows_one" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
row_count=$(wc -l < "$rows_one" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-bach-system2-order182-multibeam.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 HEADS queue 182 multi-beam C-link.'
    echo '# schema: stems-head-phase-bach-system2-order182-multibeam-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadbachs2q182summary schema stems-head-phase-bach-system2-order182-multibeam-v1 page BachInvention5.jpg#1 system 2 rows $row_count inputSha256 $input_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem2PhaseOneMultiBeamExistingStemCLink javaEvidence ReturnedBeforeHeadOrder183"
} > "$out"
echo "wrote $out"
