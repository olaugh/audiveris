#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Freeze the first post-SIDES chula system-1 STUMPS scheduler prefix.
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
warmup=$(mktemp /private/tmp/stems-stumps-prefix-warmup.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-stumps-prefix-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-stumps-prefix-pass2.XXXXXX)
semantic_one=$(mktemp /private/tmp/stems-stumps-prefix-semantic1.XXXXXX)
semantic_two=$(mktemp /private/tmp/stems-stumps-prefix-semantic2.XXXXXX)
rows=$(mktemp /private/tmp/stems-stumps-prefix-rows.XXXXXX)
txn_rows=$(mktemp /private/tmp/stems-stumps-transaction-rows.XXXXXX)
sides=$(mktemp /private/tmp/stems-stumps-prefix-sides.XXXXXX)
frozen_sides_rows=$(mktemp /private/tmp/stems-stumps-prefix-sides-frozen.XXXXXX)
trap 'rm -f "$warmup" "$pass_one" "$pass_two" "$semantic_one" "$semantic_two" "$rows" "$txn_rows" "$sides" "$frozen_sides_rows"' EXIT

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -I "$script_dir/stems-stumps-prefix.init.gradle" \
            :app:stemsStumpsPrefixProbe
    ) > "$target"
}
run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsbeam' "$pass_one" > "$semantic_one"
grep '^stemsbeam' "$pass_two" > "$semantic_two"
if ! cmp -s "$semantic_one" "$semantic_two"; then
    echo "two fresh STUMPS semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

# Adding this lane must not re-freeze or perturb the historical SIDES contract.
grep '^stemsbeamsidesloop' "$pass_one" > "$sides"
frozen_sides="$repo_root/rust/oracle/stems-beam-sides-pass-chula-system1.txt"
grep '^stemsbeamsidesloop' "$frozen_sides" > "$frozen_sides_rows"
if ! cmp -s "$sides" "$frozen_sides_rows"; then
    echo "existing chula SIDES rows drifted" >&2
    diff "$frozen_sides_rows" "$sides" | head -8 >&2
    exit 1
fi

grep '^stemsbeamstumpsprefix' "$pass_one" > "$rows"
if [ "$(grep -c '^stemsbeamstumpsprefixbaseline ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsbeamstumpsprefixterminal ' "$rows")" -ne 1 ] || \
        ! grep -q ' terminal AwaitingVLinkTransaction stopBeforeCreateStem true$' "$rows"; then
    echo "bounded chula STUMPS prefix contract differs" >&2
    exit 1
fi
grep '^stemsbeamstumpstxn' "$pass_one" > "$txn_rows"
if [ "$(grep -c '^stemsbeamstumpstxnresult ' "$txn_rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsbeamstumpstxnresumeterminal ' "$txn_rows")" -ne 1 ] || \
        ! grep -q ' transaction 1 plan 147 ' "$txn_rows" || \
        ! grep -q ' terminal AwaitingVLinkTransaction stopBeforeCreateStem true$' "$txn_rows"; then
    echo "bounded first STUMPS transaction/resume contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
sides_sha=$(shasum -a 256 "$frozen_sides" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-beam-stumps-prefix-chula-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) post-SIDES STUMPS prefix.'
    echo '# schema: stems-beam-stumps-prefix-v1'
    echo '# Exact chula system-1 retained-worklist traversal through the first ready stump VLinker.'
    echo '# Stops before createStem; two fresh semantic JVM passes were byte-identical.'
    cat "$rows"
    printf '%s\n' \
        "stemsbeamstumpsprefixsummary schema stems-beam-stumps-prefix-v1 page chula.png#1 system 1 rows $row_count probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha sidesFixtureSha256 $sides_sha sidesRowsByteIdentical true emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true stopBeforeCreateStem true"
} > "$out"
echo "wrote $out ($row_count rows)"

txn_body_sha=$(shasum -a 256 "$txn_rows" | awk '{print $1}')
txn_row_count=$(wc -l < "$txn_rows" | tr -d ' ')
txn_out="$repo_root/rust/oracle/stems-beam-stumps-transaction-chula-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) first STUMPS transaction.'
    echo '# schema: stems-beam-stumps-transaction-v1'
    echo '# Executes chula system-1 plan 147 through B12-B19 and resumes STUMPS.'
    echo '# Stops at the next ready stump VLinker before its createStem.'
    cat "$txn_rows"
    printf '%s\n' \
        "stemsbeamstumpstxnsummary schema stems-beam-stumps-transaction-v1 page chula.png#1 system 1 rows $txn_row_count probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha prefixFixtureSha256 $(shasum -a 256 "$out" | awk '{print $1}') sidesFixtureSha256 $sides_sha sidesRowsByteIdentical true emittedBodySha256 $txn_body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true stopBeforeSecondCreateStem true"
} > "$txn_out"
echo "wrote $txn_out ($txn_row_count rows)"
