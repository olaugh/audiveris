#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Allegretto system-3 HEADS queue-29 multi-head replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-allegretto-system3-order29.XXXXXX)
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
        for (i = 6; i <= 29; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 29) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 29) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 29) emitHeadCLinkResult(before, after, headOrder);"
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
        ! grep -q '^stemsheadclinkfrontier allegretto.png#1 system 3 headOrder 29 headX 114 headSig 76 headInterId 1828 cAlias h:114:RIGHT:TOP .*lastIndex 1 maxIndex 1 relations 2 .*glyphs 1 selected \[glyph:397:active:id=397:g:2198:1806:4:107:.*candidateIdBefore 397 existingGlyph glyph:397 existingActive true existingStem - lineChanged false terminal ReadyForHeadCreateStem$' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 29 allocatorBefore 2397 allocatorAfter 2398 registeredGlyphs - addedVertices \[id2398:org.audiveris.omr.sig.inter.StemInter:shape=STEM:grade=.*bounds=2198:1806:3:107:.*\] addedEdges \[system3:sourceId1812:targetId2398:org.audiveris.omr.sig.relation.HeadStemRelation:.*system3:sourceId1828:targetId2398:org.audiveris.omr.sig.relation.HeadStemRelation:.*\] addedSystemStems \[g:2198:1806:4:107:.*:stemId2398\] .*sigHashBefore 81c956983d90d48098a770435aa31974bd7edf4b16fc81229e40b62cd59dde7e sigHashAfter 8e9623ef40ef6b71df12cf5e3f9e57d1426efb902b7c6d6c8d43c6f9c4bb4add .*relationStateHashBefore 7f4e89a9eca73b8b523b88a480e584b7f89f211faa4fc27241a7589ffb1e86cb relationStateHashAfter 6c495d3e0b8d807cc25ef12f91ced3a4d1b95dfaaea25d6c5c0e5da282cc1e59 terminal ReturnedHeadCLinkTransaction$' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 3 headOrder 29 headX 114 headSig 76 headInterId 1828 .*decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=true:bottom=false:branch=TopOnly\] incident - returned true sidesAfter \[LEFT:false:false,RIGHT:true:false\] undefs \[\] closureWrites - closedValueChanges 0 unlinkedCount 0 sigVerticesBefore 644 sigVerticesAfter 645 sigEdgesBefore 567 sigEdgesAfter 569 systemStemsBefore 47 systemStemsAfter 48 .*nextHeadOrder 30 nextHeadX 25 nextHeadSig 4 nextHeadInterId 1683 .*terminal ReturnedBeforeNextHead$' "$rows"; then
    echo "Allegretto system-3 order-29 multi-head creation contract differs" >&2
    cat "$rows" >&2
    exit 1
fi
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
glyph_index_sha=$(shasum -a 256 "$glyph_index" | awk '{print $1}')
overlay_sha=$(shasum -a 256 "$glyph_index_source" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner="$script_dir/run-stems-head-phase-prefix-allegretto-system2-order111.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system2-order111.txt"
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
        [ "$base_runner_sha" != "7b3bb33bccecdaad6a7e813201a263bc622b8a6cf4635340bedabfe523bfdefc" ] || \
        [ "$base_fixture_sha" != "48c9adda2137582df18b43698f07cf2d65eb7156c726c9834d9ecafa0e908025" ] || \
        [ "$probe_sha" != "d9e98b372c7baa03cdb0473162127793ef295538c9021bb7f58025d94f2d9731" ] || \
        [ "$body_sha" != "01f3f63014ec22a8ee275d98848e0d73b52296b9b4b0cf94184091a48a9c73ce" ] || \
        [ "$semantic_sha" != "55b823afaf2dc7bb6bcefc7cfcb6421f81594105e98f54e08cd21db9e5599c63" ]; then
    echo "Allegretto system-3 order-29 provenance drifted" >&2
    echo "runner $runner_sha body $body_sha semantic $semantic_sha" >&2
    exit 1
fi
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order29.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto system-3 post-STUMPS HEADS order 29.'
    echo '# Snapshot-minimized G1 replay: generated GlyphIndex overlay retains weak identities; orders 0-28 mutate without snapshots; selected RIGHT/TOP order 29 emits.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-allegretto-system3-order29 page allegretto.png#1 system 3 rows $row_count baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha allegrettoSystem3InitSha256 $init_sha baseSystem2Order111RunnerSha256 $base_runner_sha baseSystem2Order111FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedG1RetainedGlyphAllegrettoSystem3Order29MultiHead javaEvidence ReturnedBeforeThirtiethHead"
} > "$out"
echo "wrote $out"
