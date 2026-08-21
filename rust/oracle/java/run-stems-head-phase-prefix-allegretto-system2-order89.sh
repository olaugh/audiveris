#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Allegretto system-2 HEADS queue-89 created-stem C-link replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-allegretto-system2-order89.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
glyph_index_source="$tmp_dir/GlyphIndex.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-allegretto-system2.init.gradle"
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
                "if (system.getId() == 2)")
    }
    if (index($0, "stumpsTransactionLimit > 7") != 0) {
        sub(/stumpsTransactionLimit > 7/, "stumpsTransactionLimit > 400")
    }
    if (index($0, "final HeadInter head = (HeadInter) ordered.get(0);") != 0) {
        print "            if (system.getId() == 2) {"
        for (i = 0; i <= 89; i++) {
            print "                emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        print "                return;"
        print "            }"
        print
        next
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 89; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 89) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 89) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 89) emitHeadCLinkResult(before, after, headOrder);"
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
            :app:stemsHeadPhaseAllegrettoSystem2Probe
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
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 4 ] || \
        ! grep -q '^stemsheadphaseprefixbaseline allegretto.png#1 system 2 heads 120 sigVertices 651 sigEdges 614 systemStems 52 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier allegretto.png#1 system 2 headOrder 89 headX 52 headSig 43 headInterId 1521 cAlias h:52:RIGHT:TOP .*lastIndex 3 maxIndex 3 relations 3 .*glyphs 1 selected \[glyph:2206:active:id=2206:.*existingGlyph glyph:2206 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 89 allocatorBefore 2385 allocatorAfter 2386 .*addedVertices \[id2386:.*bounds=1183:965:3:104:.*addedEdges \[system2:sourceId1521:targetId2386:.*system2:sourceId937:targetId2386:.*system2:sourceId938:targetId2386:.*addedSystemStems \[g:1183:965:3:104:.*:stemId2386\] ' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 2 headOrder 89 headX 52 headSig 43 headInterId 1521 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=true:bottom=false:branch=TopOnly\].*returned true .*closedValueChanges 0 .*sigVerticesBefore 654 sigVerticesAfter 655 sigEdgesBefore 619 sigEdgesAfter 622 systemStemsBefore 55 systemStemsAfter 56 .*nextHeadOrder 90 nextHeadX 94 nextHeadSig 51 nextHeadInterId 1537 ' "$rows"; then
    echo "Allegretto system-2 order-89 created-stem C-link contract differs" >&2
    exit 1
fi

base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
glyph_index_sha=$(shasum -a 256 "$glyph_index" | awk '{print $1}')
overlay_sha=$(shasum -a 256 "$glyph_index_source" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner="$script_dir/run-stems-head-phase-prefix-allegretto-system1-order79.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system1-order79.txt"
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
        [ "$init_sha" != "9587d9c623beea6c7922dabf6b50cd4d315ed49f4bca28bcc430684362384035" ] || \
        [ "$base_runner_sha" != "bcbf729291881676df19a79e74a0fb4f2266d09f5c5de0565dedf4420759fd95" ] || \
        [ "$base_fixture_sha" != "63327c13e4ebba1873fb73d5507b5a34369027ca8c6a4abb60f377cebeee69ee" ] || \
        [ "$probe_sha" != "4e111715e281e58c51c724130dca44b6a9c0b3149188e3063f77abd3ab58280e" ] || \
        [ "$body_sha" != "218d8ecd1a889e0046a49594e675572cd2884bf3f8f3411a0d166b8c3b2cbb21" ] || \
        [ "$semantic_sha" != "01868de57f3a8f5eb42a3496c62cb141d034b85f0fdf0d3859fe37b7337bccae" ]; then
    echo "Allegretto system-2 order-89 provenance drifted" >&2
    exit 1
fi
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system2-order89.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto system-2 post-STUMPS HEADS order 89.'
    echo '# Snapshot-minimized G1 replay: generated GlyphIndex overlay retains weak identities; orders 0-88 mutate without snapshots; selected RIGHT/TOP order 89 emits.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-allegretto-system2-order89 page allegretto.png#1 system 2 rows $row_count baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha allegrettoSystem2InitSha256 $init_sha baseSystem1Order79RunnerSha256 $base_runner_sha baseSystem1Order79FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedG1RetainedGlyphAllegrettoSystem2Order89CreatedStem javaEvidence ReturnedBeforeNinetyFirstHead"
} > "$out"
echo "wrote $out"
