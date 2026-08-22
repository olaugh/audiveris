#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Bach system-1 HEADS queue-37 higher-profile retry replay.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-bach37.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
probe="$script_dir/StemsHeadPhaseOneRetryPageProbe.java"
init="$script_dir/stems-head-phase-bach.init.gradle"
input="$repo_root/data/examples/BachInvention5.jpg"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseOneRetryPage="$input" \
            -I "$init" :app:stemsHeadPhaseBachProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsheadphase1' "$pass_one" > "$rows_one"
grep '^stemsheadphase1' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Bach phase-1 retry passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadphase1profile ' "$rows_one")" -ne 4 ] || \
        [ "$(grep -c '^stemsheadphase1retry ' "$rows_one")" -ne 1 ] || \
        ! grep -q 'headOrder 37 headX 3 headSig 95 .*stemProfile 0 decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=false:bottom=false:branch=Neither\]' "$rows_one" || \
        ! grep -q 'headOrder 37 headX 3 headSig 95 .*stemProfile 1 ' "$rows_one" || \
        ! grep -q '^stemsheadphase1retry page BachInvention5.jpg#1 system 1 headOrder 37 headX 3 headSig 95 ' "$rows_one"; then
    echo "Bach system-1 order-37 retry contract differs" >&2
    cat "$rows_one" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
row_count=$(wc -l < "$rows_one" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-bach-system1-order37-retry.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-1 HEADS order 37 retry.'
    echo '# schema: stems-head-phase-bach-system1-order37-retry-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsheadbachs1q37summary schema stems-head-phase-bach-system1-order37-retry-v1 page BachInvention5.jpg#1 system 1 rows $row_count inputSha256 $input_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullLifecycleBachSystem1PhaseOneRatherGoodProfileRetry javaEvidence ReturnedBeforeHeadOrder38"
} > "$out"
echo "wrote $out"
