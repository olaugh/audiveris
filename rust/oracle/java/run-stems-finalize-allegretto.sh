#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic full-page Java evidence for Allegretto finalizeStems.
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
tmp_dir=$(mktemp -d /private/tmp/stems-finalize-allegretto.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
probe="$script_dir/StemsFinalizePageProbe.java"
init="$script_dir/stems-finalize-page.init.gradle"
input="$repo_root/data/examples/allegretto.png"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"
base_runner="$script_dir/run-stems-head-phase-two-allegretto-system3-x113.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-allegretto-system3-x113.txt"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PfinalizePage="$input" \
            -I "$init" :app:stemsFinalizePageProbe
    ) > "$target"
}

run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep '^stemsfinalize' "$pass_one" > "$rows_one"
grep '^stemsfinalize' "$pass_two" > "$rows_two"
if ! cmp -s "$rows_one" "$rows_two"; then
    echo "fresh Allegretto finalizeStems Java passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsfinalizepage ' "$rows_one")" -ne 1 ] || \
        [ "$(grep -c '^stemsfinalizesystem ' "$rows_one")" -ne 3 ] || \
        ! grep -q '^stemsfinalizesystem page allegretto.png#1 system 1 heads 90 .*multipleBefore \[\] .*removedHeadStem \[\] .*sigVerticesBefore 215 sigVerticesAfter 215 sigEdgesBefore 273 sigEdgesAfter 273 systemStemsBefore 41 systemStemsAfter 41 allocatorBefore 2242 allocatorAfter 2242$' "$rows_one" || \
        ! grep -q '^stemsfinalizesystem page allegretto.png#1 system 2 heads 120 .*multipleBefore \[\] .*removedHeadStem \[\] .*sigVerticesBefore 264 sigVerticesAfter 264 sigEdgesBefore 338 sigEdgesAfter 338 systemStemsBefore 57 systemStemsAfter 57 allocatorBefore 2709 allocatorAfter 2709$' "$rows_one" || \
        ! grep -q '^stemsfinalizesystem page allegretto.png#1 system 3 heads 118 undefs \[x112:sig68:id1812:\[RIGHT\]\] multipleBefore \[x107:sig80:id1836\] noStemBefore \[x56:sig100:id1876\] abnormalBefore \[x56:sig100:id1876\] removedHeadStem \[\] abnormalAfter \[x56:sig100:id1876\] abnormalChanges \[\] sigVerticesBefore 267 sigVerticesAfter 267 sigEdgesBefore 320 sigEdgesAfter 320 systemStemsBefore 52 systemStemsAfter 52 allocatorBefore 3171 allocatorAfter 3171$' "$rows_one"; then
    echo "Allegretto page finalizeStems Java contract differs" >&2
    cat "$rows_one" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$input_sha" != "a9207f26b57415d8c54602881316c003319c5593ed8baf4c3af13715c41b3065" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ] || \
        [ "$base_runner_sha" != "4d26dd33041fe849dd7cb6ccb99270f9748235fc3a39c2d64dc79f678a1df823" ] || \
        [ "$base_fixture_sha" != "01a9c9b8a69c6c3305290903a0c05745e4b622be585003da0f4d1843f4b7411a" ]; then
    echo "Allegretto input, Java source, or phase-two predecessor drifted" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
row_count=$(wc -l < "$rows_one" | tr -d ' ')
out="$repo_root/rust/oracle/stems-finalize-allegretto-v1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto page finalizeStems.'
    echo '# schema: stems-finalize-page-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsfinalizepagesummary schema stems-finalize-page-v1 page allegretto.png#1 rows $row_count inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha basePhaseTwoRunnerSha256 $base_runner_sha basePhaseTwoFixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullPageAllSystemsGenericFinalizeAfterSystem3PhaseTwo javaEvidence ReturnedAfterFinalPageFinalize"
} > "$out"
echo "wrote $out"
