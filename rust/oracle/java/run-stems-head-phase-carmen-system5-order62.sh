#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Carmen system-5 HEADS order-62 relation-only crossed-head replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-carmen-system5-order62.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
glyph_index_source="$tmp_dir/GlyphIndex.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-carmen-system5-order62.init.gradle"
glyph_index="$repo_root/app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java"
input="$repo_root/data/examples/carmen.png"
base_runner="$script_dir/run-stems-head-phase-carmen-system2-order70.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-carmen-system2-order70.txt"
out="$repo_root/rust/oracle/stems-head-phase-carmen-system5-order62.txt"
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
                "if (system.getId() == 5)")
    }
    if (index($0, "stumpsTransactionLimit > 7") != 0) {
        sub(/stumpsTransactionLimit > 7/, "stumpsTransactionLimit > 400")
    }
    if (index($0, "phaseBefore.sig.relationStateHash, phaseBefore.linkers.hash);") != 0) {
        print
        print ""
        print "            if (system.getId() == 5) {"
        print "                for (int rustPortOrder = 0; rustPortOrder <= 62; rustPortOrder++) {"
        print "                    emitHeadPhaseContinuation(ordered, rustPortOrder, undefs);"
        print "                }"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 62; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 62) {"
        print "                linker.linkSides("
        print "                        Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (in_continuation && index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 62) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 62) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) {
            if (index(line, "getCornerLinker(VerticalSide.BOTTOM)") != 0) {
                sub(/VerticalSide.BOTTOM/, "VerticalSide.TOP", line)
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
            :app:stemsHeadPhaseCarmenSystem5Order62Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass1" > "$tmp_dir/semantic1"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass2" > "$tmp_dir/semantic2"
if ! cmp -s "$tmp_dir/semantic1" "$tmp_dir/semantic2"; then
    echo "Carmen system-5 order-62 semantic rows are not byte-identical" >&2
    exit 1
fi
grep '^stemshead' "$tmp_dir/pass1" > "$tmp_dir/body"
row_count=$(wc -l < "$tmp_dir/body" | tr -d ' ')
if [ "$row_count" -ne 4 ] || \
        ! grep -Fq 'stemsheadclinkfrontier carmen.png#1 system 5 headOrder 62 headX 71 headSig 7' "$tmp_dir/body" || \
        ! grep -Fq 'cAlias h:71:LEFT:TOP' "$tmp_dir/body" || \
        ! grep -Fq 'lastIndex -1 maxIndex 2 relations 2' "$tmp_dir/body" || \
        ! grep -Fq 'glyphs 2 selected [glyph:614:active:id=614:g:1618:2886:4:108:' "$tmp_dir/body" || \
        ! grep -Fq 'existingGlyph glyph:614 existingActive true existingStem 3446' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadclinkresult headOrder 62 allocatorBefore 3446 allocatorAfter 3446 registeredGlyphs - addedVertices - addedEdges - addedSystemStems -' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadphasecontinue carmen.png#1 system 5 headOrder 62 headX 71 headSig 7 headInterId 2813' "$tmp_dir/body" || \
        ! grep -Fq 'decisions [LEFT:top=true:bottom=false:branch=TopOnly,RIGHT:top=false:bottom=false:branch=Neither]' "$tmp_dir/body" || \
        ! grep -Fq 'returned false sidesAfter [LEFT:false:true,RIGHT:false:true] undefs [] closureWrites - closedValueChanges 0' "$tmp_dir/body" || \
        ! grep -Fq 'nextHeadOrder 63 nextHeadX 45 nextHeadSig 95 nextHeadInterId 2990' "$tmp_dir/body"; then
    echo "Carmen system-5 order-62 Java contract drifted" >&2
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
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/body" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$tmp_dir/semantic1" | awk '{print $1}')
if [ "$input_sha" != "249330d6558d410f64f550180d3a659dd3c9c340dcdcb5ae08e809c273fe2e44" ] || \
        [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ] || \
        [ "$fragment_sha" != "4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c" ] || \
        [ "$glyph_index_sha" != "31f25c33d8f5fd5d8fc23fad69c81d7758596925922c932d71b41b85e2abccb2" ] || \
        [ "$init_sha" != "5c66ada545193659e444da598fc0924e7cd5c2463a7cd0db5a8e744431c6af07" ] || \
        [ "$base_runner_sha" != "894c95a300969a48918c6c610691009b72f2da53e72ed2bc64f027833549c18c" ] || \
        [ "$base_fixture_sha" != "5bfd19d1c1765dfb571a3ef8819b50f1bb50df4fd2fbe0af9a10950755019e09" ]; then
    echo "Carmen source or Boundary-187 predecessor drifted" >&2
    exit 1
fi

{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Carmen system-5 head order 62.'
    echo '# schema: stems-head-phase-carmen-system5-order62-v1'
    cat "$tmp_dir/body"
    printf '%s\n' \
        "stemsheadcarmensystem5order62summary schema stems-head-phase-carmen-system5-order62-v1 page carmen.png#1 system 5 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha carmenSystem5InitSha256 $init_sha baseSystem2Order70RunnerSha256 $base_runner_sha baseSystem2Order70FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope CarmenSystem5Order62StumplessCrossedHeadHardTailNoLink javaEvidence ReturnedBeforeSixtyFourthHead"
} > "$out"
echo "wrote $out"
