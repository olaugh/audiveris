#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze the first exact head-phase-1 decision after chula system-1 STUMPS.
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
tmp_base=/private/tmp/stems-head-phase-prefix
warmup=$(mktemp "$tmp_base-warmup.XXXXXX")
pass_one=$(mktemp "$tmp_base-pass1.XXXXXX")
pass_two=$(mktemp "$tmp_base-pass2.XXXXXX")
semantic_one=$(mktemp "$tmp_base-semantic1.XXXXXX")
semantic_two=$(mktemp "$tmp_base-semantic2.XXXXXX")
rows=$(mktemp "$tmp_base-rows.XXXXXX")
stumps_actual=$(mktemp "$tmp_base-stumps-actual.XXXXXX")
stumps_frozen=$(mktemp "$tmp_base-stumps-frozen.XXXXXX")
trap 'rm -f "$warmup" "$pass_one" "$pass_two" "$semantic_one" "$semantic_two" \
    "$rows" "$stumps_actual" "$stumps_frozen"' EXIT

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -PstumpsTransactionLimit=7 -PheadPhasePrefixProbe=true \
            -I "$script_dir/stems-stumps-prefix.init.gradle" \
            :app:stemsStumpsPrefixProbe
    ) > "$target"
}
run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep -E '^(stemsbeam|stemshead)' "$pass_one" > "$semantic_one"
grep -E '^(stemsbeam|stemshead)' "$pass_two" > "$semantic_two"
if ! cmp -s "$semantic_one" "$semantic_two"; then
    echo "two fresh post-STUMPS semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

complete_fixture="$repo_root/rust/oracle/stems-beam-stumps-complete-chula-system1.txt"
grep '^stemsbeamstumpstxn' "$pass_one" | awk '
    /^stemsbeamstumpstxnresult / && / transaction 4 plan 508 / { emit = 1 }
    emit { print }
    /^stemsbeamstumpstxnresumeterminal / && / transactions 7 terminal Completed / { exit }
' > "$stumps_actual"
grep '^stemsbeamstumpstxn' "$complete_fixture" > "$stumps_frozen"
if ! cmp -s "$stumps_actual" "$stumps_frozen"; then
    echo "post-STUMPS probe changed the frozen complete-STUMPS predecessor" >&2
    diff "$stumps_frozen" "$stumps_actual" | head -8 >&2
    exit 1
fi

grep '^stemsheadphaseprefix' "$pass_one" > "$rows"
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 3 ] || \
        [ "$(grep -c '^stemsheadphaseprefixbaseline ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphaseprefixfrontier ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphaseprefixresult ' "$rows")" -ne 1 ] || \
        ! grep -q 'headOrder 0 headSig 45 headInterId 1375 ' "$rows" || \
        ! grep -q 'decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=true:bottom=false:branch=TopOnly\] selectedC ' "$rows" || \
        ! grep -q 'terminal AwaitingHeadCLinkTransaction$' "$rows" || \
        ! grep -q 'relationsBefore 0 relationsAfter 1 linked true undefs \[\] ' "$rows" || \
        ! grep -q 'sigVerticesBefore 678 sigVerticesAfter 679 sigEdgesBefore 689 sigEdgesAfter 690 ' "$rows" || \
        ! grep -q 'systemStemsBefore 39 systemStemsAfter 40 ' "$rows"; then
    echo "bounded first-head phase contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
stumps_sha=$(shasum -a 256 "$complete_fixture" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) post-STUMPS head phase.'
    echo '# schema: stems-head-phase-prefix-v1'
    echo '# First reverse-grade head decision plus scoped Java post-call evidence.'
    echo '# Native consumption stops at AwaitingHeadCLinkTransaction.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v1 page chula.png#1 system 1 rows 3 probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope AwaitingHeadCLinkTransaction javaEvidence ReturnedBeforeSecondHead"
} > "$out"
echo "wrote $out"
