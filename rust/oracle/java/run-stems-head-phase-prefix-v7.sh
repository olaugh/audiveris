#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze the v6 head prefix plus the order-7 C-link frontier and one-item create.
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
tmp_base=/private/tmp/stems-head-phase-prefix-v7
tmp_dir=$(mktemp -d "$tmp_base.XXXXXX")
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
semantic_one="$tmp_dir/semantic1"
semantic_two="$tmp_dir/semantic2"
rows="$tmp_dir/rows"
stumps_actual="$tmp_dir/stumps-actual"
stumps_frozen="$tmp_dir/stumps-frozen"
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
trap 'rm -rf "$tmp_dir"' EXIT

awk -v fragment="$script_dir/stems-head-phase-v7-fragment.java" '
{
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        print "            emitHeadPhaseContinuation(ordered, 6, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 7, undefs);"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 7) emitHeadCLinkEnvelope(head, linker, before);"
        print
        in_linker = 1
        next
    }
    if (in_linker && index($0, "final PersistentSnapshot after = snapshot(") != 0) {
        print
        capture_after = 1
        next
    }
    if (capture_after && index($0, "heads, allLinkers);") != 0) {
        print
        print "            if (headOrder == 7) emitHeadCLinkResult(before, after);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) print line
        close(fragment)
    }
    print
}' "$script_dir/StemsBeamSidesLoopProbe.java" > "$probe_source"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -PstumpsTransactionLimit=7 -PheadPhasePrefixProbe=true \
            -PheadPhaseV7ProbeSource="$probe_source" \
            -I "$script_dir/stems-head-phase-v7.init.gradle" \
            :app:stemsHeadPhaseV7Probe
    ) > "$target"
}
run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep -E '^(stemsbeam|stemshead)' "$pass_one" > "$semantic_one"
grep -E '^(stemsbeam|stemshead)' "$pass_two" > "$semantic_two"
if ! cmp -s "$semantic_one" "$semantic_two"; then
    echo "two fresh post-STUMPS v7 semantic passes are not byte-identical" >&2
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
if ! cmp -s "$stumps_frozen" "$stumps_actual"; then
    echo "post-STUMPS probe changed the frozen complete-STUMPS predecessor" >&2
    diff "$stumps_frozen" "$stumps_actual" | head -8 >&2
    exit 1
fi

grep '^stemshead' "$pass_one" > "$rows"
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 15 ] || \
        [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 7 ] || \
        [ "$(grep -c '^stemsheadclinkfrontier ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadclinkresult ' "$rows")" -ne 1 ] || \
        ! grep -q 'headOrder 5 headX 99 headSig 61 headInterId 1411 .*nextHeadOrder 6 nextHeadX 22 nextHeadSig 12 nextHeadInterId 1309 ' "$rows" || \
        ! grep -q 'headOrder 6 headX 22 headSig 12 headInterId 1309 .*nextHeadOrder 7 nextHeadX 76 nextHeadSig 97 nextHeadInterId 1483 ' "$rows" || \
        ! grep -q 'headOrder 7 headX 76 headSig 97 headInterId 1483 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\] incident - returned true .*sigVerticesBefore 679 sigVerticesAfter 680 sigEdgesBefore 690 sigEdgesAfter 691 systemStemsBefore 40 systemStemsAfter 41 ' "$rows" || \
        ! grep -q 'headOrder 7 .*nextHeadOrder 8 nextHeadX 95 nextHeadSig 100 nextHeadInterId 1489 ' "$rows" || \
        ! grep -q 'headOrder 7 headX 76 headSig 97 headInterId 1483 cAlias h:76:LEFT:BOTTOM .*lastIndex 0 maxIndex 0 relations 1 .*glyphs 1 .*candidateIdBefore 319 existingGlyph glyph:319 existingActive true existingStem - ' "$rows" || \
        ! grep -q 'headOrder 7 allocatorBefore 2379 allocatorAfter 2380 registeredGlyphs - .*addedVertices \[id2380:.*addedEdges \[system1:sourceId1483:targetId2380:.*addedSystemStems \[.*:stemId2380\] linkerChanges \[h:76:LEFT:BOTTOM:false:false->true:false,h:76:LEFT:TOP:false:false->true:false,linker:SLinker:head:76:false:false->true:false\] ' "$rows"; then
    echo "v7 order-7 C-link contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ]; then
    echo "v7 base probe drifted from the frozen v6 source" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
stumps_sha=$(shasum -a 256 "$complete_fixture" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1-v7.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) post-STUMPS head phase v7.'
    echo '# schema: stems-head-phase-prefix-v7'
    echo '# First CLinker transaction, seven real phase-1 linkSides calls, and order-7 C-link envelope.'
    echo '# Expected rows stay unread until the native transaction returns.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v7 page chula.png#1 system 1 rows 15 baseProbeSourceSha256 $base_probe_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope ReturnedOrder7HeadCLink javaEvidence ReturnedHeadCLinkTransaction"
} > "$out"
echo "wrote $out"
