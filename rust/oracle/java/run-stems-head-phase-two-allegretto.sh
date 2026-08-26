#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic full-page Java evidence for Allegretto x0 and phase-2 retries.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-allegretto.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
base_init="$script_dir/stems-head-phase-two-page.init.gradle"
init="$tmp_dir/stems-head-phase-two-allegretto.init.gradle"
input="$repo_root/data/examples/allegretto.png"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"

# Add one bounded phase-1 transaction row to the shared full-page phase-2 probe.
# The transformed class still executes the real foreground Java lifecycle.
awk '
function emit_audit_loop() {
    print "        for (Inter inter : heads) {"
    print "            HeadInter head = (HeadInter) inter;"
    print "            boolean audit = system.getId() == 3 && xOrdinals.get(head) == 0;"
    print "            IdentityHashMap<StemInter, Boolean> stemsBefore = new IdentityHashMap<>();"
    print "            int verticesBefore = sig.vertexSet().size();"
    print "            int edgesBefore = sig.edgeSet().size();"
    print "            int systemStemsBefore = ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size();"
    print "            int allocatorBefore = system.getSheet().getPersistentIdGenerator().get();"
    print "            if (audit) {"
    print "                for (Inter stem : sig.inters(StemInter.class)) {"
    print "                    stemsBefore.put((StemInter) stem, true);"
    print "                }"
    print "            }"
    print "            boolean returned = head.getLinker().linkSides("
    print "                    Profiles.STRICT, system.getProfile(), undefs, false);"
    print "            if (!returned) unlinked.add(head);"
    print "            if (audit) {"
    print "                StemInter created = null;"
    print "                for (Inter stem : sig.inters(StemInter.class)) {"
    print "                    if (!stemsBefore.containsKey((StemInter) stem)) created = (StemInter) stem;"
    print "                }"
    print "                if (created == null || created.getGlyph() == null) {"
    print "                    throw new IllegalStateException(\"x0 did not create one glyph-backed stem\");"
    print "                }"
    print "                Rectangle box = created.getGlyph().getBounds();"
    print "                Line2D median = created.getMedian();"
    print "                var rightBottom = head.getLinker().getSLinkers().get(HorizontalSide.RIGHT)"
    print "                        .getCornerLinker(VerticalSide.BOTTOM);"
    print "                Rectangle startStump = rightBottom.getStump() != null"
    print "                        ? rightBottom.getStump().getBounds() : null;"
    print "                System.out.printf("
    print "                        \"stemsheadphase1audit page %s system %d headOrder %d headX %d headSig %d \""
    print "                                + \"returned %s glyph %d:%d:%d:%d weight %d stemId %d grade %s \""
    print "                                + \"bounds %d:%d:%d:%d median %s:%s:%s:%s width %s \""
    print "                                + \"rightBottomStump %s sigVerticesBefore %d sigVerticesAfter %d \""
    print "                                + \"sigEdgesBefore %d sigEdgesAfter %d systemStemsBefore %d \""
    print "                                + \"systemStemsAfter %d allocatorBefore %d allocatorAfter %d%n\","
    print "                        page, system.getId(), heads.indexOf(head), xOrdinals.get(head),"
    print "                        sigOrdinals.get(head), returned, box.x, box.y, box.width, box.height,"
    print "                        created.getGlyph().getWeight(), created.getId(), hex(created.getGrade()),"
    print "                        created.getBounds().x, created.getBounds().y, created.getBounds().width,"
    print "                        created.getBounds().height, hex(median.getX1()), hex(median.getY1()),"
    print "                        hex(median.getX2()), hex(median.getY2()), hex(created.getWidth()),"
    print "                        startStump == null ? \"-\" : startStump.x + \":\" + startStump.y + \":\""
    print "                                + startStump.width + \":\" + startStump.height,"
    print "                        verticesBefore, sig.vertexSet().size(), edgesBefore, sig.edgeSet().size(),"
    print "                        systemStemsBefore, ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(),"
    print "                        allocatorBefore, system.getSheet().getPersistentIdGenerator().get());"
    print "            }"
    print "        }"
}
{
    if (index($0, "import java.lang.reflect.Method;") != 0) {
        print
        print "import java.awt.Rectangle;"
        print "import java.awt.geom.Line2D;"
        next
    }
    if (index($0, "import org.audiveris.omr.sig.inter.Inters;") != 0) {
        print
        print "import org.audiveris.omr.sig.inter.StemInter;"
        next
    }
    if ($0 == "        List<HeadInter> unlinked = new ArrayList<>();") {
        in_phase_one = 1
        print
        next
    }
    if (in_phase_one && !skipping && $0 == "        for (Inter inter : heads) {") {
        emit_audit_loop()
        skipping = 1
        in_phase_one = 0
        depth = 1
        next
    }
    if (skipping) {
        opens = gsub(/\{/, "{")
        closes = gsub(/\}/, "}")
        depth += opens - closes
        if (depth == 0) skipping = 0
        next
    }
    print
}' "$base_probe" > "$probe"

awk '
{
    if (index($0, "source = file(\"${repoRoot}/rust/oracle/java/StemsHeadPhaseTwoPageProbe.java\")") != 0) {
        print "        source = file(providers.gradleProperty(\047phaseTwoProbeSource\047).get())"
        next
    }
    print
}' "$base_init" > "$init"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseTwoPage="$input" \
            -PphaseTwoProbeSource="$probe" \
            -I "$init" :app:stemsHeadPhaseTwoPageProbe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep '^stemsheadphase' "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep '^stemsheadphase' "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Allegretto head-phase Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(grep -c '^stemsheadphase1audit ' "$tmp_dir/rows1")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphase2page ' "$tmp_dir/rows1")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphase2baseline ' "$tmp_dir/rows1")" -ne 3 ] || \
        [ "$(grep -c '^stemsheadphase2retry ' "$tmp_dir/rows1")" -ne 24 ] || \
        ! grep -q '^stemsheadphase1audit page allegretto.png#1 system 3 headOrder 100 headX 0 headSig 19 returned true glyph 369:1595:2:48 weight 63 stemId 3171 grade 3fe49d64653090d5 bounds 368:1595:3:48 median 40771723de22d21c:4098ec0000000000:40771f7fd38ffa01:4099ac0000000000 width 3ff5000000000000 rightBottomStump 369:1595:2:48 sigVerticesBefore 266 sigVerticesAfter 267 sigEdgesBefore 315 sigEdgesAfter 316 systemStemsBefore 51 systemStemsAfter 52 allocatorBefore 3170 allocatorAfter 3171$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2baseline page allegretto.png#1 system 3 heads 118 queueSize 5 queue \[x112:sig68:id1812,x14:sig50:id1777,x13:sig0:id1675,x56:sig100:id1876,x113:sig75:id1826\] sigVertices 267 sigEdges 317 systemStems 52 allocator 3171$' "$tmp_dir/rows1"; then
    echo "Allegretto head-phase Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$base_init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$script_dir/run-stems-head-phase-prefix-allegretto-system3-order117.sh" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system3-order117.txt" | awk '{print $1}')
row_count=$(wc -l < "$tmp_dir/rows1" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-two-allegretto.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto x0 and phase-2 retries.'
    echo '# schema: stems-head-phase-two-allegretto-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2summary schema stems-head-phase-two-allegretto-v1 page allegretto.png#1 rows $row_count inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha baseProbeSourceSha256 $base_probe_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseSystem3Order117RunnerSha256 $base_runner_sha baseSystem3Order117FixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope FullPageSystem3X0AndAllSystemsPhaseTwoAppendRetry javaEvidence ReturnedAfterFinalPageRetry"
} > "$out"
echo "wrote $out"
