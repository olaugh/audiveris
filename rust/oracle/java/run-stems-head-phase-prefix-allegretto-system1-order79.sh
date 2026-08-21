#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Allegretto system-1 HEADS queue-79 crossed-side C-link replay.
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
script_dir="$repo_root/rust/oracle/java"
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-allegretto79.XXXXXX)
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
fragment="$script_dir/stems-head-phase-v28-fragment.java"
init="$script_dir/stems-head-phase-allegretto.init.gradle"
trap 'rm -rf "$tmp_dir"' EXIT

awk -v fragment="$fragment" '
{
    if (index($0, "if (page.equals(\"chula.png#1\") && system.getId() == 1)") != 0) {
        sub(/if \(page.equals\("chula.png#1"\) && system.getId\(\) == 1\)/,
                "if (system.getId() == 1)")
    }
    if (index($0, "stumpsTransactionLimit > 7") != 0) {
        sub(/stumpsTransactionLimit > 7/, "stumpsTransactionLimit > 400")
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        for (i = 6; i <= 79; i++) {
            print "            emitHeadPhaseContinuation(ordered, " i ", undefs);"
        }
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) in_continuation = 1
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 79) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 79) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
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
        print "            if (headOrder == 79) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) {
            if (index(line, "getCornerLinker(VerticalSide.BOTTOM)") != 0) {
                sub(/getCornerLinker\(VerticalSide.BOTTOM\)/,
                        "getCornerLinker(VerticalSide.TOP)", line)
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
            -I "$init" \
            :app:stemsHeadPhaseAllegrettoProbe
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
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 1 ] || \
        ! grep -q '^stemsheadclinkfrontier allegretto.png#1 system 1 headOrder 79 headX 82 headSig 89 headInterId 1433 cAlias h:82:LEFT:TOP .*lastIndex 1 maxIndex 2 relations 2 .*glyphs 1 .*existingGlyph glyph:297 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 79 allocatorBefore 2239 allocatorAfter 2240 .*addedVertices \[id2240:.*bounds=2299:692:3:47:.*addedEdges \[system1:sourceId1317:targetId2240:.*system1:sourceId1433:targetId2240:.*addedSystemStems \[g:2299:692:2:47:.*:stemId2240\] ' "$rows" || \
        ! grep -q '^stemsheadphasecontinue allegretto.png#1 system 1 headOrder 79 headX 82 headSig 89 headInterId 1433 .*decisions \[LEFT:top=true:bottom=false:branch=TopOnly,RIGHT:top=false:bottom=true:branch=BottomOnly\].*returned true .*closedValueChanges 0 .*sigVerticesBefore 637 sigVerticesAfter 638 sigEdgesBefore 562 sigEdgesAfter 564 .*nextHeadOrder 80 nextHeadX 81 nextHeadSig 48 nextHeadInterId 1351 ' "$rows"; then
    echo "Allegretto system-1 order-79 crossed-side C-link contract differs" >&2
    exit 1
fi

base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
base_runner="$script_dir/run-stems-head-phase-prefix-allegretto-system1-order65.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system1-order65.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$tmp_dir/semantic1" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ] || \
        [ "$fragment_sha" != "4f27146b667a76b23e38607b8669ae78edeb73af78cad818ce8a95cedf54300c" ] || \
        [ "$init_sha" != "782c68149e2962f81b0a006a4b01ae662f306efc31ae457f9cf7e36645d7693d" ] || \
        [ "$base_runner_sha" != "be1f28c0528721e23ba24e1b8107f5069310d47a1a537945052d2a536a260e74" ] || \
        [ "$base_fixture_sha" != "0bccd92c0a4305704c5903984ccf9734823bf4879b5aa6f2621595700fa6507d" ] || \
        [ "$probe_sha" != "b22c21f1b9410ec66aa5445f8aa2f9aa4e4149c02b733abe03617ec6be05c032" ] || \
        [ "$body_sha" != "e9802845ac23e54fb14617dc21a63ac1a5be0d5b64e998bf0b8cd0ff1a288d62" ]; then
    echo "Allegretto system-1 order-79 provenance drifted" >&2
    exit 1
fi
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-allegretto-system1-order79.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Allegretto post-STUMPS HEADS order 79.'
    echo '# Snapshot-minimized replay: orders 1-78 mutate without snapshots; selected LEFT/TOP order 79 emits.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-allegretto-system1-order79 page allegretto.png#1 system 1 rows $row_count baseProbeSourceSha256 $base_probe_sha fragmentSourceSha256 $fragment_sha allegrettoInitSha256 $init_sha baseV65RunnerSha256 $base_runner_sha baseV65FixtureSha256 $base_fixture_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedAllegrettoSystem1Order79CrossSideCreatedStem javaEvidence ReturnedBeforeEightyFirstHead"
} > "$out"
echo "wrote $out"
