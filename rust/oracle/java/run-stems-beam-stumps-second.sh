#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Freeze the second post-SIDES chula system-1 STUMPS transaction and resume.
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
warmup=$(mktemp /private/tmp/stems-stumps-second-warmup.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-stumps-second-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-stumps-second-pass2.XXXXXX)
semantic_one=$(mktemp /private/tmp/stems-stumps-second-semantic1.XXXXXX)
semantic_two=$(mktemp /private/tmp/stems-stumps-second-semantic2.XXXXXX)
rows=$(mktemp /private/tmp/stems-stumps-second-rows.XXXXXX)
first_actual=$(mktemp /private/tmp/stems-stumps-second-first-actual.XXXXXX)
first_frozen=$(mktemp /private/tmp/stems-stumps-second-first-frozen.XXXXXX)
sides=$(mktemp /private/tmp/stems-stumps-second-sides.XXXXXX)
frozen_sides_rows=$(mktemp /private/tmp/stems-stumps-second-sides-frozen.XXXXXX)
trap 'rm -f "$warmup" "$pass_one" "$pass_two" "$semantic_one" "$semantic_two" "$rows" "$first_actual" "$first_frozen" "$sides" "$frozen_sides_rows"' EXIT

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -PstumpsTransactionLimit=2 \
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
    echo "two fresh second-STUMPS semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

frozen_sides="$repo_root/rust/oracle/stems-beam-sides-pass-chula-system1.txt"
grep '^stemsbeamsidesloop' "$pass_one" > "$sides"
grep '^stemsbeamsidesloop' "$frozen_sides" > "$frozen_sides_rows"
if ! cmp -s "$sides" "$frozen_sides_rows"; then
    echo "existing chula SIDES rows drifted" >&2
    diff "$frozen_sides_rows" "$sides" | head -8 >&2
    exit 1
fi

first_fixture="$repo_root/rust/oracle/stems-beam-stumps-transaction-chula-system1.txt"
grep -E '^stemsbeamstumpstxn(result|resumestep|resumebeam) ' "$pass_one" \
    | awk '/ transaction 2 plan 622 / { exit } { print }' > "$first_actual"
grep -E '^stemsbeamstumpstxn(result|resumestep|resumebeam) ' "$first_fixture" \
    > "$first_frozen"
if ! cmp -s "$first_actual" "$first_frozen"; then
    echo "first STUMPS transaction/pre-second traversal drifted" >&2
    diff "$first_frozen" "$first_actual" | head -8 >&2
    exit 1
fi

awk '
    /^stemsbeamstumpstxnresult / && / transaction 2 plan 622 / { emit = 1 }
    emit { print }
    /^stemsbeamstumpstxnresumeterminal / && / transactions 2 / { exit }
' "$pass_one" > "$rows"
if [ "$(grep -c '^stemsbeamstumpstxnresult ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsbeamstumpstxnresumeterminal ' "$rows")" -ne 1 ] || \
        ! grep -q ' transaction 2 plan 622 ' "$rows" || \
        ! grep -q ' plan 404 terminal AwaitingVLinkTransaction stopBeforeCreateStem true$' "$rows"; then
    echo "bounded second STUMPS transaction/resume contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
first_sha=$(shasum -a 256 "$first_fixture" | awk '{print $1}')
sides_sha=$(shasum -a 256 "$frozen_sides" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-beam-stumps-second-transaction-chula-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) second STUMPS transaction.'
    echo '# schema: stems-beam-stumps-second-transaction-v1'
    echo '# Executes chula system-1 plan 622 through B12-B17 and resumes STUMPS.'
    echo '# Stops at plan 404 before its createStem; two fresh JVM passes were byte-identical.'
    cat "$rows"
    printf '%s\n' \
        "stemsbeamstumpssecondsummary schema stems-beam-stumps-second-transaction-v1 page chula.png#1 system 1 rows $row_count probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha firstTransactionFixtureSha256 $first_sha sidesFixtureSha256 $sides_sha sidesRowsByteIdentical true firstTransactionPrefixByteIdentical true emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true stopBeforeThirdCreateStem true"
} > "$out"
echo "wrote $out ($row_count rows)"
