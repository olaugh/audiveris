#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Freeze the first real competing-hook removal reached by the carried Java
# SIDES replay: Allegretto system 1, after transaction 28.
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
warmup=$(mktemp /private/tmp/stems-hook-removal-warmup.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-hook-removal-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-hook-removal-pass2.XXXXXX)
semantic_one=$(mktemp /private/tmp/stems-hook-removal-semantic1.XXXXXX)
semantic_two=$(mktemp /private/tmp/stems-hook-removal-semantic2.XXXXXX)
rows=$(mktemp /private/tmp/stems-hook-removal-rows.XXXXXX)
predecessor_rows=$(mktemp /private/tmp/stems-hook-removal-predecessor.XXXXXX)
linked_rows=$(mktemp /private/tmp/stems-hook-removal-linked.XXXXXX)
frozen_linked=$(mktemp /private/tmp/stems-hook-removal-frozen-linked.XXXXXX)
trap 'rm -f "$warmup" "$pass_one" "$pass_two" "$semantic_one" "$semantic_two" \
    "$rows" "$predecessor_rows" "$linked_rows" "$frozen_linked"' EXIT

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -PhookRemovalProbe=true \
            -I "$script_dir/stems-linked-s.init.gradle" \
            :app:stemsLinkedSProbe
    ) > "$target"
}
run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsbeam' "$pass_one" > "$semantic_one"
grep '^stemsbeam' "$pass_two" > "$semantic_two"
if ! cmp -s "$semantic_one" "$semantic_two"; then
    echo "two fresh hook-removal semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

grep '^stemsbeamlinkeds' "$pass_one" > "$linked_rows"
grep '^stemsbeamlinkeds' "$repo_root/rust/oracle/stems-beam-linked-s-allegretto-system1.txt" \
    | grep -v '^stemsbeamlinkeds summary ' > "$frozen_linked"
if ! cmp -s "$linked_rows" "$frozen_linked"; then
    echo "hook-removal replay changed the frozen linked-S predecessor rows" >&2
    diff "$frozen_linked" "$linked_rows" | head -8 >&2
    exit 1
fi

grep -E '^(stemsbeamhookremoval|stemsbeamschedulerresumeexhausted|stemsbeamsidesloopcensus)' \
    "$pass_one" > "$rows"
grep '^stemsbeamsidesloopsibling ' "$pass_one" > "$predecessor_rows"
if [ "$(wc -l < "$predecessor_rows" | tr -d ' ')" -ne 28 ]; then
    echo "bounded Allegretto hook-removal predecessor differs" >&2
    exit 1
fi
awk '{ for (i = 1; i <= NF; i++) if ($i == "transaction") { if ($(i + 1) != NR) exit 1 } }' \
    "$predecessor_rows" || {
        echo "hook-removal predecessor transactions are not dense 1..28" >&2
        exit 1
    }
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 4 ] || \
        [ "$(grep -c '^stemsbeamhookremovalfrontier ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsbeamhookremovalresult ' "$rows")" -ne 1 ] || \
        ! grep -q '^stemsbeamhookremovalfrontier allegretto.png#1 system 1 transactions 28 event 64 workIndex 19 beamSig 25 hookSig 24 ' "$rows" || \
        ! grep -q ' group \[beam:21,beam:24,beam:25\] .* sigVertices 202 sigEdges 232 ' "$rows" || \
        ! grep -q '^stemsbeamhookremovalresult .* hookRemoved true ' "$rows" || \
        ! grep -q ' hookVertexMatches 0 hookIncidentAfter 0 ' "$rows" || \
        ! grep -q ' workUnchanged true group \[beam:21,beam:25\] ' "$rows" || \
        ! grep -q ' linkedBUnchanged true sigVertices 201 sigEdges 227 ' "$rows" || \
        ! grep -q ' interIndexUnchanged false .* terminal RemovedCompetingHook$' "$rows" || \
        ! grep -q '^stemsbeamschedulerresumeexhausted allegretto.png#1 system 1 event 110 phase SIDES terminal SidesWorklistExhaustedBeforeSecondFrontier$' "$rows" || \
        ! grep -q '^stemsbeamsidesloopcensus allegretto.png#1 system 1 freshTransactions 27 secondFrontier true sidesExhausted true$' "$rows"; then
    echo "bounded Allegretto hook-removal row contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
linked_sha=$(shasum -a 256 "$linked_rows" | awk '{print $1}')
predecessor_body_sha=$(shasum -a 256 "$predecessor_rows" | awk '{print $1}')
predecessor_out="$repo_root/rust/oracle/stems-beam-hook-removal-predecessor-allegretto-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) hook-removal scheduler input.'
    echo '# schema: stems-beam-hook-removal-predecessor-v1'
    echo '# Actual per-transaction B-linker and sibling writes through Allegretto system-1 transaction 28.'
    cat "$predecessor_rows"
    printf '%s\n' \
        "stemsbeamhookremovalpredecessor summary schema stems-beam-hook-removal-predecessor-v1 page allegretto.png#1 system 1 transactions 28 rows 28 probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $predecessor_body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true terminal AwaitingHookRemovalTransaction"
} > "$predecessor_out"
predecessor_fixture_sha=$(shasum -a 256 "$predecessor_out" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-beam-hook-removal-allegretto-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) real competing-hook removal.'
    echo '# schema: stems-beam-hook-removal-v1'
    echo '# Actual Allegretto system-1 SIDES replay through removal and exhaustion; not synthetic.'
    echo '# Two fresh foreground JVM passes were required byte-identical before extraction.'
    cat "$rows"
    printf '%s\n' \
        "stemsbeamhookremoval summary schema stems-beam-hook-removal-v1 page allegretto.png#1 system 1 transactions 28 rows 4 probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha linkedSPredecessorBodySha256 $linked_sha schedulerPredecessorFixtureSha256 $predecessor_fixture_sha freshRuns 2 freshRunsByteIdentical true terminal SidesWorklistExhaustedBeforeSecondFrontier"
} > "$out"
echo "wrote $predecessor_out"
echo "wrote $out"
