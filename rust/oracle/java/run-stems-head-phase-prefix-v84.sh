#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Read-only snapshot-minimized order84 continuation from v83; prior orders mutate without snapshots.
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
tmp_base=/private/tmp/stems-head-phase-prefix-v84
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
trap ':' EXIT
fragment18="$script_dir/stems-head-phase-v28-fragment.java"

awk -v fragment="$fragment18" '
{
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        print "            emitHeadPhaseContinuation(ordered, 6, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 7, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 8, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 9, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 10, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 11, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 12, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 13, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 14, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 15, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 16, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 17, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 18, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 19, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 20, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 21, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 22, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 23, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 24, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 25, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 26, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 27, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 28, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 29, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 30, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 31, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 32, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 33, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 34, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 35, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 36, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 37, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 38, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 39, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 40, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 41, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 42, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 43, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 44, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 45, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 46, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 47, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 48, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 49, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 50, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 51, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 52, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 53, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 54, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 55, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 56, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 57, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 58, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 59, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 60, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 61, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 62, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 63, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 64, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 65, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 66, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 67, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 68, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 69, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 70, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 71, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 72, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 73, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 74, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 75, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 76, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 77, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 78, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 79, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 80, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 81, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 82, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 83, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 84, undefs);"
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) {
        in_continuation = 1
    }
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 84) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 7 || headOrder == 22 || headOrder == 27 || headOrder == 34 || headOrder == 36 || headOrder == 37 || headOrder == 38 || headOrder == 39 || headOrder == 40 || headOrder == 41 || headOrder == 42 || headOrder == 43 || headOrder == 44 || headOrder == 45 || headOrder == 46 || headOrder == 47 || headOrder == 48 || headOrder == 49 || headOrder == 50 || headOrder == 51 || headOrder == 52 || headOrder == 53 || headOrder == 54 || headOrder == 55 || headOrder == 56 || headOrder == 57 || headOrder == 58 || headOrder == 59 || headOrder == 60 || headOrder == 61 || headOrder == 62 || headOrder == 63 || headOrder == 64 || headOrder == 65 || headOrder == 66 || headOrder == 67 || headOrder == 68 || headOrder == 69 || headOrder == 70 || headOrder == 71 || headOrder == 72 || headOrder == 73 || headOrder == 74 || headOrder == 75 || headOrder == 76 || headOrder == 77 || headOrder == 78 || headOrder == 79 || headOrder == 80 || headOrder == 81 || headOrder == 82 || headOrder == 83 || headOrder == 84) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 7 || headOrder == 22 || headOrder == 27 || headOrder == 34 || headOrder == 36 || headOrder == 37 || headOrder == 38 || headOrder == 39 || headOrder == 40 || headOrder == 41 || headOrder == 42 || headOrder == 43 || headOrder == 44 || headOrder == 45 || headOrder == 46 || headOrder == 47 || headOrder == 48 || headOrder == 49 || headOrder == 50 || headOrder == 51 || headOrder == 52 || headOrder == 53 || headOrder == 54 || headOrder == 55 || headOrder == 56 || headOrder == 57 || headOrder == 58 || headOrder == 59 || headOrder == 60 || headOrder == 61 || headOrder == 62 || headOrder == 63 || headOrder == 64 || headOrder == 65 || headOrder == 66 || headOrder == 67 || headOrder == 68 || headOrder == 69 || headOrder == 70 || headOrder == 71 || headOrder == 72 || headOrder == 73 || headOrder == 74 || headOrder == 75 || headOrder == 76 || headOrder == 77 || headOrder == 78 || headOrder == 79 || headOrder == 80 || headOrder == 81 || headOrder == 82 || headOrder == 83 || headOrder == 84) emitHeadCLinkResult(before, after, headOrder);"
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
    echo "two fresh post-STUMPS v27 semantic passes are not byte-identical" >&2
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
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 1 ] || \
        ! grep -q 'headOrder 84 headX 94 headSig 99 headInterId 1487 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:SkipClosed\].*incident \[stem2364:headSideLEFT:heads\[x91:sig98:id1485:sideLEFT,x94:sig99:id1487:sideLEFT,x95:sig100:id1489:sideLEFT\].*closureWrites \[x91:sig98:LEFT:true->true,x91:sig98:RIGHT:true->true,x95:sig100:LEFT:false->true,x95:sig100:RIGHT:false->true\].*closedValueChanges 2 .*nextHeadOrder 85 nextHeadX 79 nextHeadSig 40 nextHeadInterId 1365 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .*headOrder 84 headX 94 headSig 99 .*cAlias h:94:LEFT:BOTTOM .*lastIndex 1 maxIndex 1 relations 2 .*glyphs 1 .*candidateIdBefore 297 existingGlyph glyph:297 existingActive true existingStem 2364 ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 84 allocatorBefore 2385 allocatorAfter 2385 .*addedVertices - addedEdges - addedSystemStems - .*linkerChanges \[h:95:LEFT:BOTTOM:true:false->true:true' "$rows"; then
    echo "v84 three-head existing-stem reconciliation contract differs" >&2
    exit 1
fi
: <<'DISABLED_V27_ACTIVE'
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 10 ] || \
        ! grep -q 'headOrder 27 headX 33 headSig 26 headInterId 1337 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\].*nextHeadOrder 28 nextHeadX 85 nextHeadSig 87 nextHeadInterId 1463 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .*headOrder 27 headX 33 headSig 26 .*cAlias h:33:LEFT:BOTTOM .*lastIndex 1 maxIndex 1 relations 1 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:314 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 27 allocatorBefore 2382 allocatorAfter 2383 .*addedVertices \[id2383:.*addedEdges \[system1:sourceId1337:targetId2383:.*addedSystemStems \[g:1019:388:3:89:.*:stemId2383\]' "$rows"; then
    echo "v27 both-open C-link contract differs" >&2
    exit 1
fi
DISABLED_V27_ACTIVE
: <<'DISABLED_V26_CONTRACT'
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 10 ] || \
        ! grep -q 'headOrder 25 headX 59 headSig 74 headInterId 1437 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2363:headSideLEFT:heads\[x58:sig73:id1435:sideLEFT,x59:sig74:id1437:sideLEFT\]\].*closureWrites \[x58:sig73:LEFT:false->true,x58:sig73:RIGHT:false->true\].*closedValueChanges 2 .*nextHeadOrder 26 nextHeadX 61 nextHeadSig 31 nextHeadInterId 1347 ' "$rows" || \
        ! grep -q 'headOrder 26 headX 61 headSig 31 headInterId 1347 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2345:headSideLEFT:heads\[x60:sig30:id1345:sideLEFT,x61:sig31:id1347:sideLEFT\]\].*closureWrites \[x60:sig30:LEFT:false->true,x60:sig30:RIGHT:false->true\].*closedValueChanges 2 .*nextHeadOrder 27 nextHeadX 33 nextHeadSig 26 nextHeadInterId 1337 ' "$rows" || \
        ! grep -q 'headOrder 27 headX 33 headSig 26 headInterId 1337 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\].*nextHeadOrder 28 nextHeadX 85 nextHeadSig 87 nextHeadInterId 1463 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .* headOrder 27 headX 33 headSig 26 .*cAlias h:33:LEFT:BOTTOM .*lastIndex 1 maxIndex 1 relations 1 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:314 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 27 allocatorBefore 2382 allocatorAfter 2383 .*addedVertices \[id2383:.*addedEdges \[system1:sourceId1337:targetId2383:.*addedSystemStems \[g:1019:388:3:89:.*:stemId2383\].*linkerChanges \[h:33:LEFT:BOTTOM:false:false->true:false,h:33:LEFT:TOP:false:false->true:false,linker:SLinker:head:33:false:false->true:false\]' "$rows"; then
    echo "v27 both-open C-link contract differs" >&2
    exit 1
fi
DISABLED_V26_CONTRACT
if false; then
    :
fi
: <<'DISABLED_V23_CONTRACT'
        ! grep -q 'headOrder 22 headX 4 headSig 7 headInterId 1299 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\]' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .* headOrder 22 .* relations 2 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:315 existingActive true existingStem 2354 ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 22 allocatorBefore 2382 allocatorAfter 2382 registeredGlyphs - addedVertices - addedEdges - addedSystemStems - linkerChanges \[h:3:LEFT:BOTTOM:true:false->true:true,h:3:LEFT:TOP:true:false->true:true,h:3:RIGHT:BOTTOM:false:false->false:true,h:3:RIGHT:TOP:false:false->false:true,linker:SLinker:head:3:false:false->false:true,linker:SLinker:head:3:true:false->true:true\] ' "$rows" || \
        ! grep -q 'headOrder 23 headX 78 headSig 39 headInterId 1363 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2370:headSideLEFT:heads\[x77:sig38:id1361:sideLEFT,x78:sig39:id1363:sideLEFT\]\].*closureWrites \[x77:sig38:LEFT:false->true,x77:sig38:RIGHT:false->true\].*closedValueChanges 2 unlinkedCount 0 sigVerticesBefore 682 sigVerticesAfter 682 sigEdgesBefore 693 sigEdgesAfter 693 relationStateHashBefore 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d relationStateHashAfter 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d .*nextHeadOrder 24 nextHeadX 93 nextHeadSig 25 nextHeadInterId 1335 .*nextSides \[LEFT:true:false,RIGHT:false:false\]' "$rows" || \
        ! grep -q 'headOrder 24 headX 93 headSig 25 headInterId 1335 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2342:headSideLEFT:heads\[x92:sig24:id1333:sideLEFT,x93:sig25:id1335:sideLEFT\]\].*closureWrites \[x92:sig24:LEFT:false->true,x92:sig24:RIGHT:false->true\].*closedValueChanges 2 unlinkedCount 0 sigVerticesBefore 682 sigVerticesAfter 682 sigEdgesBefore 693 sigEdgesAfter 693 relationStateHashBefore 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d relationStateHashAfter 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d .*nextHeadOrder 25 nextHeadX 59 nextHeadSig 74 nextHeadInterId 1437 .*nextSides \[LEFT:true:false,RIGHT:false:false\]' "$rows"; then
    echo "v24 bounded prelinked-closure contract differs" >&2
    exit 1
fi
DISABLED_V23_CONTRACT

probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ]; then
    echo "base probe drifted from the frozen v6 source" >&2
    exit 1
fi
v83_runner_sha=$(shasum -a 256 "$script_dir/run-stems-head-phase-prefix-v83.sh" | awk '{print $1}')
if [ "$v83_runner_sha" != "5d1a030d4d98807e022bed40cd4fa44b4057dad9f123b537bdeb17b48fd97a90" ]; then
    echo "v84 base v83 runner drifted" >&2
    exit 1
fi
v83_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1-v83.txt"
v83_fixture_sha=$(shasum -a 256 "$v83_fixture" | awk '{print $1}')
if [ "$v83_fixture_sha" != "c37d7e4015e34d2c0d61cd7c4159ccdd6834b4d575f650d612a5c1d6f94d8cb1" ]; then
    echo "v84 base v83 fixture drifted" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
stumps_sha=$(shasum -a 256 "$complete_fixture" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment18" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out="/private/tmp/stems-head-phase-prefix-chula-system1-v84-audit.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) post-STUMPS head phase v84.'
    echo '# schema: stems-head-phase-prefix-v84'
    echo '# Snapshot-minimized order-84 derivative: orders 1-83 mutate without snapshots; order 84 emits the three-head existing-stem reconciliation.'
    echo '# This bounded scope is intentional for deterministic replay under the full-snapshot heap limit.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v84 page chula.png#1 system 1 rows $row_count baseProbeSourceSha256 $base_probe_sha baseV83RunnerSourceSha256 $v83_runner_sha baseV83FixtureSha256 $v83_fixture_sha fragmentSourceSha256 $fragment_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedOrder84ExistingStemReconciliation javaEvidence ReturnedBeforeEightyFifthHead"
} > "$out"
echo "wrote $out"
