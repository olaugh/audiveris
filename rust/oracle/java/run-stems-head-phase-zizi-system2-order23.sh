#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Zizi system-2 HEADS order-23 crossed-head replay.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-zizi-system2-order23.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
glyph_index_source="$tmp_dir/GlyphIndex.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-zizi-system2-order23.init.gradle"
glyph_index="$repo_root/app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java"
input="$repo_root/data/examples/zizi.png"
base_runner="$script_dir/run-stems-head-phase-zizi-system1-order34.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-zizi-system1-order34.txt"
out="$repo_root/rust/oracle/stems-head-phase-zizi-system2-order23.txt"
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
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 23; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 23) {"
        print "                linker.linkSides("
        print "                        Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (in_continuation && index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 23) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 23) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) {
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
            :app:stemsHeadPhaseZiziSystem2Order23Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass1" > "$tmp_dir/semantic1"
grep -E "^(stemsbeam|stemshead)" "$tmp_dir/pass2" > "$tmp_dir/semantic2"
if ! cmp -s "$tmp_dir/semantic1" "$tmp_dir/semantic2"; then
    echo "Zizi system-2 order-23 semantic rows are not byte-identical" >&2
    exit 1
fi
grep '^stemshead' "$tmp_dir/pass1" > "$tmp_dir/body"
row_count=$(wc -l < "$tmp_dir/body" | tr -d ' ')
if [ "$row_count" -ne 9 ] || \
        ! grep -Fq 'stemsheadclinkfrontier zizi.png#1 system 2 headOrder 23 headX 94 headSig 61 headInterId 1183' "$tmp_dir/body" || \
        ! grep -Fq 'cAlias h:94:LEFT:BOTTOM' "$tmp_dir/body" || \
        ! grep -Fq 'lastIndex -1 maxIndex 2 relations 2' "$tmp_dir/body" || \
        ! grep -Fq 'glyphs 2 selected [glyph:1448:active:id=1448:g:1951:940:3:12:' "$tmp_dir/body" || \
        ! grep -Fq 'glyph:245:active:id=245:g:1940:913:4:57:' "$tmp_dir/body" || \
        ! grep -Fq 'candidate g:1940:913:14:57:' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadclinkresult headOrder 23 allocatorBefore 1723 allocatorAfter 1723 registeredGlyphs - addedVertices - addedEdges - addedSystemStems -' "$tmp_dir/body" || \
        ! grep -Fq 'stemsheadphasecontinue zizi.png#1 system 2 headOrder 23 headX 94 headSig 61 headInterId 1183' "$tmp_dir/body" || \
        ! grep -Fq 'closureWrites [x93:sig60:LEFT:false->true,x93:sig60:RIGHT:false->true] closedValueChanges 2' "$tmp_dir/body" || \
        ! grep -Fq 'nextHeadOrder 24 nextHeadX 86 nextHeadSig 94 nextHeadInterId 1253' "$tmp_dir/body"; then
    echo "Zizi system-2 order-23 Java contract drifted" >&2
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
if [ "$input_sha" != "f6c613b3a60423dadde60d5e61ee7c1a641eef71c9fc6b6e8bdf5fab4c3c3e94" ] || \
        [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ] || \
        [ "$fragment_sha" != "4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c" ] || \
        [ "$glyph_index_sha" != "31f25c33d8f5fd5d8fc23fad69c81d7758596925922c932d71b41b85e2abccb2" ] || \
        [ "$init_sha" != "46241f0adbc0ef8746240567b2b54d09ffad062962e07f4deee9c745e6b43d97" ] || \
        [ "$base_runner_sha" != "3522c1cc1139b07988d03c897a363c7a5146e9997c59b48d7fc045c14ff4e586" ] || \
        [ "$base_fixture_sha" != "6cb0d6892e62c46b29c3aef267fec7780ff917ae68da79e64e3dc83fc1a16303" ]; then
    echo "Zizi source or Boundary-184 predecessor drifted" >&2
    exit 1
fi

{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Zizi system-2 head order 23.'
    echo '# schema: stems-head-phase-zizi-system2-order23-v1'
    cat "$tmp_dir/body"
    printf '%s\n' \
        "stemsheadzizisystem2order23summary schema stems-head-phase-zizi-system2-order23-v1 page zizi.png#1 system 2 rows $row_count inputSha256 $input_sha baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha glyphIndexSourceSha256 $glyph_index_sha retainedGlyphOverlaySha256 $overlay_sha ziziSystem2InitSha256 $init_sha baseSystem1Order34RunnerSha256 $base_runner_sha baseSystem1Order34FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope ZiziSystem2Order23CompoundChunkRejectionAndExistingStemClosure javaEvidence ReturnedBeforeTwentyFifthHead"
} > "$out"
echo "wrote $out"
