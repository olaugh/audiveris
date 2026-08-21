#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Allegretto system-3 final phase-1 head / prelinked no-op closure replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-allegretto-system3-order117.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
glyph_index_source="$tmp_dir/GlyphIndex.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-allegretto-system3.init.gradle"
glyph_index="$repo_root/app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java"
trap 'rm -rf "$tmp_dir"' EXIT

# G1 is required for this page-wide replay. Compile a measurement-only classpath overlay that
# keeps every registered glyph strongly reachable from the start of HEADS, preserving the exact
# weak-index identities and persistent allocator that the Epsilon runs establish on smaller cases.
awk '
{
    if (index($0, "private static final Logger logger") != 0) {
        print
        print ""
        print "    private static final List<Glyph> rustPortRetainedGlyphs ="
        print "            Collections.synchronizedList(new ArrayList<>());"
        next
    }
    if (index($0, "final Glyph orgGlyph = (orgWeak != null) ? orgWeak.get() : null;") != 0) {
        print
        print ""
        print "        if (Boolean.getBoolean(\"audiveris.rustport.retainGlyphsForProbe\")) {"
        print "            rustPortRetainedGlyphs.add((orgGlyph != null) ? orgGlyph : glyph);"
        print "        }"
        next
    }
    if (index($0, "public void setEntities (Collection<Glyph> glyphs)") != 0) in_set_entities = 1
    if (in_set_entities && index($0, "originals.putIfAbsent(weak, weak);") != 0) {
        print
        print ""
        print "            if (Boolean.getBoolean(\"audiveris.rustport.retainGlyphsForProbe\")) {"
        print "                rustPortRetainedGlyphs.add(glyph);"
        print "            }"
        in_set_entities = 0
        next
    }
    print
}' "$glyph_index" > "$glyph_index_source"

awk -v fragment="$fragment" '
{
    if (index($0, "if (page.equals(\"chula.png#1\") && system.getId() == 1)") != 0) {
        sub(/if \(page.equals\("chula.png#1"\) && system.getId\(\) == 1\)/,
                "if (system.getId() == 3)")
    }
    if (index($0, "stumpsTransactionLimit > 7") != 0) {
        sub(/stumpsTransactionLimit > 7/, "stumpsTransactionLimit > 400")
    }
    if (index($0, "final HeadInter head = (HeadInter) ordered.get(0);") != 0) {
        print "            if (system.getId() == 2) {"
        for (i = 0; i <= 29; i++) {
            print "                emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        print "                return;"
        print "            }"
        print
        next
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 117; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 117 && headOrder != 53) {"
        print "                HeadInter debugX108 = null;"
        print "                for (Inter debugInter : ordered) {"
        print "                    final HeadInter debugHead = (HeadInter) debugInter;"
        print "                    if (headOrdinals.get(debugHead) == 108) debugX108 = debugHead;"
        print "                }"
        print "                final HeadLinker.SLinker debugX108Side = debugX108.getLinker()"
        print "                        .getSLinkers().get(HorizontalSide.RIGHT);"
        print "                final boolean debugLinkedBefore = debugX108Side.isLinked();"
        print "                final boolean debugClosedBefore = debugX108Side.isClosed();"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                if (debugLinkedBefore != debugX108Side.isLinked()"
        print "                        || debugClosedBefore != debugX108Side.isClosed()) {"
        print "                    System.out.printf("
        print "                            \"stemsheaddebug108change headOrder %d headX %d linked %s->%s closed %s->%s%n\","
        print "                            headOrder, headOrdinals.get(head), debugLinkedBefore,"
        print "                            debugX108Side.isLinked(), debugClosedBefore,"
        print "                            debugX108Side.isClosed());"
        print "                }"
        print "                return;"
        print "            }"
        next
    }
    if (in_continuation && index($0, "final List<String> incident = new ArrayList<>();") != 0) {
        print "            if (headOrder == 115) {"
        print "                for (HorizontalSide debugH : HorizontalSide.values()) {"
        print "                    for (VerticalSide debugV : VerticalSide.values()) {"
        print "                        final HeadLinker.SLinker debugS = linker.getSLinkers().get(debugH);"
        print "                        final HeadLinker.SLinker.CLinker debugC = debugS.getCornerLinker(debugV);"
        print "                        final StemBuilder debugBuilder = (StemBuilder) C_STEM_BUILDER.get(debugC);"
        print "                        final int debugIndex = debugBuilder.indexOf(debugC);"
        print "                        final HeadLinker.SLinker.CLinker debugFirst ="
        print "                                debugBuilder.getFirstCLinkerAfter(debugIndex, Profiles.STRICT);"
        print "                        System.out.printf("
        print "                                \"stemsheaddebug115 hSide %s vSide %s length %d myIndex %d first %s firstX %s firstLinked %s firstClosed %s%n\","
        print "                                debugH, debugV, debugBuilder.getLength(Profiles.STRICT), debugIndex,"
        print "                                debugFirst == null ? \"-\" : linkerAlias(debugFirst),"
        print "                                debugFirst == null ? \"-\" : headOrdinals.get(debugFirst.getSource()),"
        print "                                debugFirst == null ? \"-\" : debugFirst.isLinked(),"
        print "                                debugFirst == null ? \"-\" : debugFirst.isClosed());"
        print "                    }"
        print "                }"
        print "            }"
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 53) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 53) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "final HeadInter next = (HeadInter) ordered.get(headOrder + 1);") != 0) {
        print "            if (headOrder + 1 >= ordered.size()) {"
        print "                System.out.printf("
        print "                        \"stemsheadphasecontinue %s system %d headOrder %d headX %d headSig %d \""
        print "                                + \"headInterId %d grade %s stemProfile %d linkProfile %d append false \""
        print "                                + \"sidesBefore %s decisions %s incident %s returned %s sidesAfter %s \""
        print "                                + \"undefs %s closureWrites %s closedValueChanges %d unlinkedCount 0 \""
        print "                                + \"sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d \""
        print "                                + \"sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d \""
        print "                                + \"relationStateHashBefore %s relationStateHashAfter %s \""
        print "                                + \"linkerStateHashBefore %s linkerStateHashAfter %s \""
        print "                                + \"terminal ReturnedAfterLastHead%n\","
        print "                        page, system.getId(), headOrder, headOrdinals.get(head),"
        print "                        headSigOrdinals.get(head), head.getId(), hex(head.getGrade()),"
        print "                        Profiles.STRICT, system.getProfile(), sidesBefore, list(decisions),"
        print "                        list(incident), returned, headSideState(linker),"
        print "                        undefs.get(head) == null ? \"[]\" : undefs.get(head),"
        print "                        list(closureWrites), closedValueChanges, before.sig.vertices.size(),"
        print "                        after.sig.vertices.size(), before.sig.edges.size(),"
        print "                        after.sig.edges.size(), before.systemStems.entries.size(),"
        print "                        after.systemStems.entries.size(), before.sig.relationStateHash,"
        print "                        after.sig.relationStateHash, before.linkers.hash, after.linkers.hash);"
        print "                return;"
        print "            }"
        print ""
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) {
            if (index(line, "get(HorizontalSide.LEFT)") != 0) {
                sub(/get\(HorizontalSide.LEFT\)/, "get(HorizontalSide.RIGHT)", line)
            }
            if (index(line, "getCornerLinker(VerticalSide.BOTTOM)") != 0) {
                sub(/getCornerLinker\(VerticalSide.BOTTOM\)/,
                        "getCornerLinker(VerticalSide.TOP)", line)
            }
            if (index(line, "final LinkedHashSet<Glyph> planGlyphs") != 0) {
                print line
                print "            final StemBuilder measuredBuilder ="
                print "                    (StemBuilder) C_STEM_BUILDER.get(corner);"
                print "            final Line2D savedTheoreticalLine ="
                print "                    copy((Line2D) STEM_BUILDER_THEO_LINE.get(measuredBuilder));"
                continue
            }
            if (index(line, "before.assertOnlyLineChanged(expanded);") != 0) {
                print line
                print "            ((Line2D) STEM_BUILDER_THEO_LINE.get(measuredBuilder))"
                print "                    .setLine(savedTheoreticalLine);"
                continue
            }
            print line
        }
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
            -PheadPhaseV7ProbeSource="$probe_source" \
            -PheadPhaseGlyphIndexSource="$glyph_index_source" \
            -I "$init" \
            :app:stemsHeadPhaseAllegrettoSystem3Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass1" > "$tmp_dir/semantic1"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass2" > "$tmp_dir/semantic2"
cmp "$tmp_dir/semantic1" "$tmp_dir/semantic2"
rows="$tmp_dir/rows"
grep '^stemshead' "$tmp_dir/pass1" > "$rows"
if ! grep -q '^stemsheadclinkfrontier allegretto.png#1 system 3 headOrder 53 headX 107 headSig 80 headInterId 1836 cAlias h:107:RIGHT:TOP .*lastIndex 3 maxIndex 3 relations 4 .*existingStem 2398 .*terminal ReadyForHeadCreateStem$' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 53 allocatorBefore 2400 allocatorAfter 2400 registeredGlyphs - addedVertices - addedEdges \[.*sourceId1810:targetId2398:.*sourceId1818:targetId2398:.*sourceId1836:targetId2394:.*sourceId1836:targetId2398:.*\] addedSystemStems - .*terminal ReturnedHeadCLinkTransaction$' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 3 headOrder 53 headX 107 headSig 80 headInterId 1836 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=true:bottom=true:branch=Both\] incident - returned true sidesAfter \[LEFT:true:false,RIGHT:true:false\].*sigEdgesBefore 573 sigEdgesAfter 577 .*nextHeadOrder 54 nextHeadX 26 nextHeadSig 56 ' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 3 headOrder 117 headX 86 headSig 18 headInterId 1711 .*sidesBefore \[LEFT:true:true,RIGHT:false:true\] decisions \[LEFT:SkipAlreadyLinked,RIGHT:SkipClosed\] incident \[stem2368:headSideLEFT:heads\[x84:sig27:id1731:sideLEFT,x85:sig28:id1733:sideLEFT,x86:sig18:id1711:sideLEFT\]\] returned true sidesAfter \[LEFT:true:true,RIGHT:false:true\] undefs \[\] closureWrites \[x84:sig27:LEFT:true->true,x84:sig27:RIGHT:true->true,x85:sig28:LEFT:true->true,x85:sig28:RIGHT:true->true\] closedValueChanges 0 unlinkedCount 0 sigVerticesBefore 649 sigVerticesAfter 649 sigEdgesBefore 593 sigEdgesAfter 593 systemStemsBefore 52 systemStemsAfter 52 relationStateHashBefore 63240815cfaf84ffc2ec724c7da0de08d085ca149d6f754e79266e1fedfe6ceb relationStateHashAfter 63240815cfaf84ffc2ec724c7da0de08d085ca149d6f754e79266e1fedfe6ceb linkerStateHashBefore 8c951e957d7a414e47facb5e9217390f4d9c4de875467ee5b1bec1dcbdcfd3ac linkerStateHashAfter 8c951e957d7a414e47facb5e9217390f4d9c4de875467ee5b1bec1dcbdcfd3ac terminal ReturnedAfterLastHead$' "$rows"; then
    echo "Allegretto system-3 queue-117 audit contract differs" >&2
    exit 1
fi
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
glyph_index_sha=$(shasum -a 256 "$glyph_index" | awk '{print $1}')
overlay_sha=$(shasum -a 256 "$glyph_index_source" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner="$script_dir/run-stems-head-phase-prefix-allegretto-system3-order116.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order116.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$tmp_dir/semantic1" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ] || \
        [ "$fragment_sha" != "4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c" ] || \
        [ "$glyph_index_sha" != "31f25c33d8f5fd5d8fc23fad69c81d7758596925922c932d71b41b85e2abccb2" ] || \
        [ "$overlay_sha" != "f21487398d9ba162b6459f8f5e1265d56ffc6a8a58e6aa514a03553ee3d05df4" ] || \
        [ "$init_sha" != "c801a89d512ffc1751c178e41c6dee30a17d559bfe1b6b1822e6bc050f8b91b9" ] || \
        [ "$base_runner_sha" != "2e2c10929798d25ea10ec0b5912288db59e5feb71f806c784fd60b445fbe89f3" ] || \
        [ "$base_fixture_sha" != "cc6b2240cc6f6fa13fa294ef17eb01cae65afc8189fba4e4a244d99d76891a8e" ]; then
    echo "Allegretto system-3 queue-117 provenance drifted" >&2
    exit 1
fi
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order117.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto system-3 post-STUMPS HEADS queue 117.'
    echo '# Snapshot-minimized G1 replay: orders 0-116 mutate without snapshots except the authenticated queue-53 multi-side transaction; queue 117 emits the final prelinked no-op closure.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-allegretto-system3-order117 page allegretto.png#1 system 3 rows $row_count baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha allegrettoSystem3InitSha256 $init_sha baseSystem3Order116RunnerSha256 $base_runner_sha baseSystem3Order116FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedG1RetainedGlyphAllegrettoSystem3Order117PrelinkedNoOpClosure javaEvidence ReturnedAfterLastPhaseOneHead"
} > "$out"
echo "wrote $out"
