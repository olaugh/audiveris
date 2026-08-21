#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic full-page Java evidence for Batuque finalizeStems.
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
tmp_dir=$(mktemp -d /private/tmp/stems-finalize-batuque.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows_one="$tmp_dir/rows1"
rows_two="$tmp_dir/rows2"
probe="$script_dir/StemsFinalizePageProbe.java"
init="$script_dir/stems-finalize-page.init.gradle"
input="$repo_root/data/examples/batuque.png"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"

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
    echo "fresh Batuque finalizeStems Java passes are not byte-identical" >&2
    diff "$rows_one" "$rows_two" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsfinalizepage ' "$rows_one")" -ne 1 ] || \
        [ "$(grep -c '^stemsfinalizesystem ' "$rows_one")" -ne 3 ] || \
        ! grep -q '^stemsfinalizesystem page batuque.png#1 system 1 heads 93 undefs \[\] multipleBefore \[\] noStemBefore \[\] abnormalBefore \[\] removedHeadStem \[\] abnormalAfter \[\] abnormalChanges \[\] sigVerticesBefore 232 sigVerticesAfter 232 sigEdgesBefore 301 sigEdgesAfter 301 systemStemsBefore 42 systemStemsAfter 42 allocatorBefore 2360 allocatorAfter 2360$' "$rows_one" || \
        ! grep -q '^stemsfinalizesystem page batuque.png#1 system 2 heads 122 undefs \[\] multipleBefore \[\] noStemBefore \[x108:sig115:id1708,x109:sig121:id1720\] abnormalBefore \[x108:sig115:id1708,x109:sig121:id1720\] removedHeadStem \[\] abnormalAfter \[x108:sig115:id1708,x109:sig121:id1720\] abnormalChanges \[\] sigVerticesBefore 293 sigVerticesAfter 293 sigEdgesBefore 406 sigEdgesAfter 406 systemStemsBefore 54 systemStemsAfter 54 allocatorBefore 2826 allocatorAfter 2826$' "$rows_one" || \
        ! grep -q '^stemsfinalizesystem page batuque.png#1 system 3 heads 112 undefs \[x107:sig47:id1817:\[RIGHT\],x108:sig2:id1727:\[LEFT\]\] multipleBefore \[\] noStemBefore \[x107:sig47:id1817,x108:sig2:id1727\] abnormalBefore \[x107:sig47:id1817,x108:sig2:id1727\] removedHeadStem \[\] abnormalAfter \[x107:sig47:id1817,x108:sig2:id1727\] abnormalChanges \[\] sigVerticesBefore 268 sigVerticesAfter 268 sigEdgesBefore 344 sigEdgesAfter 344 systemStemsBefore 52 systemStemsAfter 52 allocatorBefore 3228 allocatorAfter 3228$' "$rows_one"; then
    echo "Batuque page finalizeStems Java contract differs" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
if [ "$input_sha" != "11d983fe6173ae4569dbd7100a07234f446e1caaba3f57769154ad29fda93523" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ]; then
    echo "Batuque input or Java StemsRetriever source drifted" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows_one" | awk '{print $1}')
row_count=$(wc -l < "$rows_one" | tr -d ' ')
out="$repo_root/rust/oracle/stems-finalize-batuque-v1.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Batuque page finalizeStems.'
    echo '# schema: stems-finalize-page-v1'
    cat "$rows_one"
    printf '%s\n' \
        "stemsfinalizepagesummary schema stems-finalize-page-v1 page batuque.png#1 rows $row_count inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullPageAllSystemsGenericFinalize javaEvidence ReturnedAfterFinalPageFinalize"
} > "$out"
echo "wrote $out"
