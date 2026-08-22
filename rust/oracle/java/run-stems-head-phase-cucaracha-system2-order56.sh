#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Cucaracha system-2 HEADS order-56 rejected-stem replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-cucaracha-system2-order56.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
glyph_index_source="$tmp_dir/GlyphIndex.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-cucaracha-system2-order56.init.gradle"
glyph_index="$repo_root/app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java"
input="$repo_root/data/examples/cucaracha.png"
out="$repo_root/rust/oracle/stems-head-phase-cucaracha-system2-order56.txt"
trap 'rm -rf "$tmp_dir"' EXIT

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
    if (index($0, "phaseBefore.sig.relationStateHash, phaseBefore.linkers.hash);") != 0) {
        print
        print ""
        print "            if (system.getId() == 2) {"
        print "                for (int rustPortOrder = 0; rustPortOrder <= 56; rustPortOrder++) {"
        print "                    emitHeadPhaseContinuation(ordered, rustPortOrder, undefs);"
        print "                }"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 56; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 56) {"
        print "                linker.linkSides("
        print "                        Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (in_continuation && index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 56) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 56) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) {
            if (index(line, "getSLinkers().get(HorizontalSide.LEFT)") != 0) {
                sub(/HorizontalSide.LEFT/, "HorizontalSide.RIGHT", line)
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
            :app:stemsHeadPhaseCucarachaSystem2Order56Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass1" > "$tmp_dir/semantic1"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass2" > "$tmp_dir/semantic2"
if ! cmp -s "$tmp_dir/semantic1" "$tmp_dir/semantic2"; then
    echo "Cucaracha system-2 order-56 semantic rows are not byte-identical" >&2
    exit 1
fi
grep '^stemshead' "$tmp_dir/pass1" > "$tmp_dir/body"
row_count=$(wc -l < "$tmp_dir/body" | tr -d ' ')
if [ "$row_count" -ne 4 ] || \
        ! grep -Fq 'stemsheadclinkfrontier cucaracha.png#1 system 2 headOrder 56 headX 56 headSig 78 headInterId 1388 cAlias h:56:RIGHT:BOTTOM' "$tmp_dir/body" || \
        ! grep -Fq 'lastIndex 0 maxIndex 2 relations 1' "$tmp_dir/body" || \
        ! grep -Fq 'grade=0x1.0p0/3ff0000000000000' "$tmp_dir/body" || \
        ! grep -Fq 'glyphs 1 selected [glyph:1838:active:id=1838:g:1100:1221:1:15:643f0d2bde1937cd598dfc222af394289a860061d77ded9b37d5489e8f3df2c8]' "$tmp_dir/body" || \
        ! grep -Fq 'existingGlyph glyph:1838 existingActive true existingStem - lineChanged false' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadclinkresult headOrder 56 allocatorBefore 2207 allocatorAfter 2207 registeredGlyphs - addedVertices - addedEdges - addedSystemStems -' "$tmp_dir/body" || \
        ! grep -Fq 'sigHashBefore 922233cc69aa82ebb7cd15a9073e86c41cb21e270125564a60b9baab771b096b sigHashAfter 922233cc69aa82ebb7cd15a9073e86c41cb21e270125564a60b9baab771b096b' "$tmp_dir/body" || \
        ! grep -Fq 'relationStateHashBefore bc17a70e8d4204b060162783c88a8ddfa5daa635d3db313bb809e85de244b113 relationStateHashAfter bc17a70e8d4204b060162783c88a8ddfa5daa635d3db313bb809e85de244b113' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadphasecontinue cucaracha.png#1 system 2 headOrder 56 headX 56 headSig 78 headInterId 1388' "$tmp_dir/body" || \
        ! grep -Fq 'decisions [LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=false:bottom=true:branch=BottomOnly]' "$tmp_dir/body" || \
        ! grep -Fq 'returned false sidesAfter [LEFT:false:true,RIGHT:false:true] undefs [] closureWrites - closedValueChanges 0 unlinkedCount 0' "$tmp_dir/body" || \
        ! grep -Fq 'nextHeadOrder 57 nextHeadX 132 nextHeadSig 84 nextHeadInterId 1400' "$tmp_dir/body"; then
    echo "Cucaracha system-2 order-56 Java contract differs" >&2
    cat "$tmp_dir/body" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
glyph_index_sha=$(shasum -a 256 "$glyph_index" | awk '{print $1}')
overlay_sha=$(shasum -a 256 "$glyph_index_source" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/body" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$tmp_dir/semantic1" | awk '{print $1}')
predecessor_fixture_sha=$(
    shasum -a 256 \
        "$repo_root/rust/oracle/stems-beam-scheduler-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-expand-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-create-stem-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-reuse-check-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-base-apply-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-b-linker-flag-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-sibling-links-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-head-links-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-vlink-outer-blinker-cucaracha.txt" \
        "$repo_root/rust/oracle/stems-beam-scheduler-resume-cucaracha.txt" |
        sed "s|$repo_root/||" |
        shasum -a 256 |
        awk '{print $1}'
)
if [ "$input_sha" != "ab54d23f0fdcb17c2e5211db88692facae8cf99cd190d318174d7e3a8cc2d717" ] || \
        [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ] || \
        [ "$fragment_sha" != "4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c" ] || \
        [ "$glyph_index_sha" != "31f25c33d8f5fd5d8fc23fad69c81d7758596925922c932d71b41b85e2abccb2" ] || \
        [ "$init_sha" != "4a66495632f0e1a650e57e260e15c7a6f68370fbbaf4bf900b27aa643a2f26e0" ] || \
        [ "$predecessor_fixture_sha" != "e365077c7432b03f811987470a1f8c7b9666ffcea8135dd0b28b4e823cef0a1d" ]; then
    echo "Cucaracha order-56 source or predecessor fixtures drifted" >&2
    exit 1
fi
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Cucaracha system-2 head order 56.'
    echo '# schema: stems-head-phase-cucaracha-system2-order56-v1'
    cat "$tmp_dir/body"
    printf '%s\n' \
        "stemsheadcucarachasystem2order56summary schema stems-head-phase-cucaracha-system2-order56-v1 page cucaracha.png#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha cucarachaSystem2InitSha256 $init_sha predecessorFixtureSetSha256 $predecessor_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope CucarachaSystem2Order56RejectedStemNoLink javaEvidence ReturnedBeforeFiftyEighthHead"
} > "$out"
echo "wrote $out"
