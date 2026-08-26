#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Allegretto system-3 HEADS queue-61 two-chunk/beam replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-allegretto-system3-order61.XXXXXX)
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
        for (i = 6; i <= 61; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 61) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 61) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 61) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
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
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 9 ] || \
        ! grep -q '^stemsheadphaseprefixbaseline allegretto.png#1 system 3 heads 118 sigVertices 636 sigEdges 559 systemStems 39 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier allegretto.png#1 system 3 headOrder 61 headX 57 headSig 99 headInterId 1874 cAlias h:57:RIGHT:TOP .*lastIndex 3 maxIndex 3 relations 2 .*glyphs 2 selected \[glyph:410:active:id=410:g:1336:1857:3:92:.*glyph:2000:active:id=2000:g:1335:1938:2:11:.*\] candidate g:1335:1857:4:92:.*candidateIdBefore 0 existingGlyph - existingActive false existingStem - lineChanged false terminal ReadyForHeadCreateStem$' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 61 allocatorBefore 2400 allocatorAfter 2402 registeredGlyphs \[glyph:2401:g:1335:1857:4:92:.*:active=true\] addedVertices \[id2402:org.audiveris.omr.sig.inter.StemInter:shape=STEM:grade=0x1.8e911769616ccp-1/3fe8e911769616cc:bounds=1336:1857:4:92:.*\] addedEdges \[system3:sourceId1041:targetId2402:org.audiveris.omr.sig.relation.BeamStemRelation:.*system3:sourceId1874:targetId2402:org.audiveris.omr.sig.relation.HeadStemRelation:.*\] addedSystemStems \[g:1335:1857:4:92:.*:stemId2402\] .*sigHashBefore 49ae2b70c1d65455a40667fcd56d23dd9ed5b6e8208297e28e2fdad6c77a35a4 sigHashAfter 55ded9aad03b6ad16828f21c264194297cb29aa9d16cd0ee5c1f326bbac84475 .*relationStateHashBefore 514d6ed90bb80b00f8a6e528777c5dd5b889ce4c0c7a6bccb33caf01d6107a6d relationStateHashAfter 5c8587818ad813e4730c3e90517e1f598a92c84c74d9d46051aafbe57770392f terminal ReturnedHeadCLinkTransaction$' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 3 headOrder 61 headX 57 headSig 99 headInterId 1874 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=true:bottom=false:branch=TopOnly\] incident - returned true sidesAfter \[LEFT:false:false,RIGHT:true:false\] undefs \[\] closureWrites - closedValueChanges 0 unlinkedCount 0 sigVerticesBefore 647 sigVerticesAfter 648 sigEdgesBefore 579 sigEdgesAfter 581 systemStemsBefore 50 systemStemsAfter 51 .*nextHeadOrder 62 nextHeadX 54 nextHeadSig 97 nextHeadInterId 1870 .*terminal ReturnedBeforeNextHead$' "$rows"; then
    echo "Allegretto system-3 order-61 two-chunk/beam creation contract differs" >&2
    cat "$rows" >&2
    exit 1
fi
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
glyph_index_sha=$(shasum -a 256 "$glyph_index" | awk '{print $1}')
overlay_sha=$(shasum -a 256 "$glyph_index_source" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner="$script_dir/run-stems-head-phase-prefix-allegretto-system3-order29.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order29.txt"
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
        [ "$base_runner_sha" != "a3b14250711ef31a1709787893d13665fe08d6be9048acf9e9a04c4b2cdb79c5" ] || \
        [ "$base_fixture_sha" != "4ced3051f95f86951774d18a537e183967f689866cafcde853f47d8565610d76" ] || \
        [ "$probe_sha" != "3318d3d122240b9e10dee6573ac3fd3c95b99c640ff229405975771ef63c4666" ] || \
        [ "$body_sha" != "8ca888b1aaa74a62535616852e1bea4f9812051e6134e368745ee35589d57711" ] || \
        [ "$semantic_sha" != "b2df20a7c4f2261f63c1380569e7cd1d7b81f8a91aa5baf8cad0ef74b64400a6" ]; then
    echo "Allegretto system-3 order-61 provenance drifted" >&2
    echo "runner $runner_sha body $body_sha semantic $semantic_sha" >&2
    exit 1
fi
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order61.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto system-3 post-STUMPS HEADS order 61.'
    echo '# Snapshot-minimized G1 replay: generated GlyphIndex overlay retains weak identities; orders 0-60 mutate without snapshots; selected RIGHT/TOP order 61 emits.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-allegretto-system3-order61 page allegretto.png#1 system 3 rows $row_count baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha allegrettoSystem3InitSha256 $init_sha baseSystem3Order29RunnerSha256 $base_runner_sha baseSystem3Order29FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedG1RetainedGlyphAllegrettoSystem3Order61TwoChunkBeamCreatedStem javaEvidence ReturnedBeforeSixtyThirdHead"
} > "$out"
echo "wrote $out"
