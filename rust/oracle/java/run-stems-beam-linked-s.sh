#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Freeze the first real linked-S Boundary-13 case. The full SIDES replay is run
# twice, but only Allegretto system 1 / plan 25's five read-only B13 rows land.
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
pass_one=$(mktemp /private/tmp/stems-linked-s-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-linked-s-pass2.XXXXXX)
warmup=$(mktemp /private/tmp/stems-linked-s-warmup.XXXXXX)
rows=$(mktemp /private/tmp/stems-linked-s-rows.XXXXXX)
semantic_one=$(mktemp /private/tmp/stems-linked-s-semantic1.XXXXXX)
semantic_two=$(mktemp /private/tmp/stems-linked-s-semantic2.XXXXXX)
trap 'rm -f "$pass_one" "$pass_two" "$warmup" "$rows" "$semantic_one" "$semantic_two"' EXIT

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
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
    echo "two fresh linked-S semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

grep '^stemsbeamlinkeds' "$pass_one" > "$rows"
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 5 ] || \
        [ "$(grep -c '^stemsbeamlinkedsentry ' "$rows")" -ne 2 ] || \
        ! grep -q ' mapOrdinal 0 .* sLinked true .* action SelectBreak$' "$rows" || \
        ! grep -q ' mapOrdinal 1 .* conditionRead false .* action UnreadAfterBreak$' "$rows"; then
    echo "bounded Allegretto linked-S row contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-beam-linked-s-allegretto-system1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) linked-S B13 lane.'
    echo '# schema: stems-beam-linked-s-v1'
    echo '# Full Allegretto system-1 SIDES replay; bounded to transaction 28 / plan 25.'
    echo '# Two fresh foreground JVM passes were required byte-identical before extraction.'
    cat "$rows"
    printf '%s\n' \
        "stemsbeamlinkeds summary schema stems-beam-linked-s-v1 page allegretto.png#1 system 1 transaction 28 plan 25 rows 5 probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true stopBeforeSigAddVertex true"
} > "$out"
echo "wrote $out"
