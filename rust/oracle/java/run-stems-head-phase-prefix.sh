#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze the first exact head-phase-1 decision and five prelinked continuations after chula system-1 STUMPS.
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

grep '^stemshead' "$pass_one" > "$rows"
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 11 ] || \
        [ "$(grep -c '^stemsheadphaseprefixbaseline ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphaseprefixfrontier ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphaseprefixresult ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadclinkexpand ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadclinkcreate ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadclinkapply ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 5 ] || \
        ! grep -q 'headOrder 0 headSig 45 headInterId 1375 ' "$rows" || \
        ! grep -q 'decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=true:bottom=false:branch=TopOnly\] selectedC ' "$rows" || \
        ! grep -q 'terminal AwaitingHeadCLinkTransaction$' "$rows" || \
        ! grep -q 'relationsBefore 0 relationsAfter 1 linked true undefs \[\] ' "$rows" || \
        ! grep -q 'sigVerticesBefore 678 sigVerticesAfter 679 sigEdgesBefore 689 sigEdgesAfter 690 ' "$rows" || \
        ! grep -q 'systemStemsBefore 39 systemStemsAfter 40 ' "$rows"; then
    echo "bounded first-head phase contract differs" >&2
    exit 1
fi
if ! grep -q 'lastIndex 0 maxIndex 0 relations 1 .* glyphs 1 .*candidateIdBefore 307 existingGlyph glyph:307 existingActive true existingStem - ' "$rows" || \
        ! grep -q 'registeredAlias glyph:307 registeredId 307 registration ReuseActive disposition CreatedChecked stemId 2379 stemVertex 260 ' "$rows" || \
        ! grep -q 'allocatorBefore 2378 allocatorAfter 2379 systemStemsBefore 39 systemStemsAfter 40 interIndexBefore 678 interIndexAfter 679 ' "$rows" || \
        ! grep -q 'addedVertices 1 addedEdges 1 .*linkerChanges \[h:38:RIGHT:BOTTOM:false:false->true:false,h:38:RIGHT:TOP:false:false->true:false,linker:SLinker:head:38:false:false->true:false\] ' "$rows" || \
        ! grep -q 'dirtyBefore true:true:true dirtyAfter true:true:true nextHeadOrder 1 nextHeadSig 23 nextHeadInterId 1331 nextSides \[LEFT:true:false,RIGHT:false:false\] ' "$rows"; then
    echo "complete first-head CLinker transaction contract differs" >&2
    exit 1
fi
if ! grep -q 'headOrder 1 headX 90 headSig 23 headInterId 1331 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] incident \[stem2359:headSideLEFT:heads\[x89:sig22:id1329:sideLEFT,x90:sig23:id1331:sideLEFT\]\] returned true .*closureWrites \[x89:sig22:LEFT:false->true,x89:sig22:RIGHT:false->true\] closedValueChanges 2 unlinkedCount 0 ' "$rows" || \
        ! grep -q 'headOrder 2 headX 81 headSig 33 headInterId 1351 grade .*3fe901efd26d99b1 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] incident \[stem2371:headSideLEFT:heads\[x79:sig40:id1365:sideLEFT,x80:sig32:id1349:sideLEFT,x81:sig33:id1351:sideLEFT\]\] returned true .*closureWrites \[x79:sig40:LEFT:false->true,x79:sig40:RIGHT:false->true,x80:sig32:LEFT:false->true,x80:sig32:RIGHT:false->true\] closedValueChanges 4 unlinkedCount 0 .*nextHeadOrder 3 nextHeadX 20 nextHeadSig 65 nextHeadInterId 1419 ' "$rows" || \
        ! grep -q 'headOrder 3 headX 20 headSig 65 headInterId 1419 grade .*3fe8e97b8a9fa8ca .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] incident \[stem2361:headSideLEFT:heads\[x19:sig64:id1417:sideLEFT,x20:sig65:id1419:sideLEFT\]\] returned true .*closureWrites \[x19:sig64:LEFT:false->true,x19:sig64:RIGHT:false->true\] closedValueChanges 2 unlinkedCount 0 .*nextHeadOrder 4 nextHeadX 36 nextHeadSig 69 nextHeadInterId 1427 ' "$rows" || \
        ! grep -q 'headOrder 4 headX 36 headSig 69 headInterId 1427 grade .*3fe8e37718100f0c .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] incident \[stem2369:headSideLEFT:heads\[x35:sig68:id1425:sideLEFT,x36:sig69:id1427:sideLEFT\]\] returned true .*closureWrites \[x35:sig68:LEFT:false->true,x35:sig68:RIGHT:false->true\] closedValueChanges 2 unlinkedCount 0 .*nextHeadOrder 5 nextHeadX 99 nextHeadSig 61 nextHeadInterId 1411 ' "$rows" || \
        ! grep -q 'headOrder 5 headX 99 headSig 61 headInterId 1411 grade .*3fe8b9e1faa76070 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] incident \[stem2365:headSideLEFT:heads\[x98:sig60:id1409:sideLEFT,x99:sig61:id1411:sideLEFT\]\] returned true .*closureWrites \[x98:sig60:LEFT:false->true,x98:sig60:RIGHT:false->true\] closedValueChanges 2 unlinkedCount 0 .*nextHeadOrder 6 nextHeadX 22 nextHeadSig 12 nextHeadInterId 1309 ' "$rows"; then
    echo "post-first-head continuation contract differs" >&2
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
    echo '# schema: stems-head-phase-prefix-v6'
    echo '# First CLinker transaction plus the next five real phase-1 linkSides calls.'
    echo '# Expected rows stay unread until the native transaction returns.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v6 page chula.png#1 system 1 rows 11 probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope ReturnedFiveHeadPhaseContinuations javaEvidence ReturnedBeforeSeventhHead"
} > "$out"
echo "wrote $out"
