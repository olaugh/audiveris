#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze a bounded order-21 continuation replay with existing-stem C-link
# retry envelope/result instrumentation. Orders below 18 execute without snapshots.
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
tmp_base=/private/tmp/stems-head-phase-prefix-v21
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
trap 'rm -rf "$tmp_dir"' EXIT
fragment18="$script_dir/stems-head-phase-v21-fragment.java"

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
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) {
        in_continuation = 1
    }
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 18) {"
        print "                linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false);"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 7 || headOrder == 21) emitHeadCLinkEnvelope(head, linker, before);"
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
        print "            if (headOrder == 7 || headOrder == 21) emitHeadCLinkResult(before, after);"
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
    echo "two fresh post-STUMPS v21 semantic passes are not byte-identical" >&2
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
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 4 ] || \
        ! grep -q 'headOrder 21 headX 28 headSig 55 headInterId 1399 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\]' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .* headOrder 21 .* relations 2 .*glyphs 1 .*candidateIdBefore 300 existingGlyph glyph:300 existingActive true existingStem 2378 ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 21 allocatorBefore 2382 allocatorAfter 2382 registeredGlyphs - addedVertices - addedEdges - addedSystemStems - linkerChanges \[h:27:LEFT:BOTTOM:true:false->true:true,h:27:LEFT:TOP:true:false->true:true,h:27:RIGHT:BOTTOM:false:false->false:true,h:27:RIGHT:TOP:false:false->false:true,linker:SLinker:head:27:false:false->false:true,linker:SLinker:head:27:true:false->true:true\] ' "$rows"; then
    echo "v21 bounded order-21 retry contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ]; then
    echo "base probe drifted from the frozen v6 source" >&2
    exit 1
fi
v20_runner_sha=$(shasum -a 256 "$script_dir/run-stems-head-phase-prefix-v20.sh" | awk '{print $1}')
if [ "$v20_runner_sha" != "54468f53de6c0d1d931e391640642f55ce6c4733721df569ef6f10ef93704497" ]; then
    echo "v21 base v20 runner drifted" >&2
    exit 1
fi
v20_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1-v20.txt"
v20_fixture_sha=$(shasum -a 256 "$v20_fixture" | awk '{print $1}')
if [ "$v20_fixture_sha" != "be6a820b3740105e4fdddeb0e9ec475d1dd3ebc8611fd7be555cf55957dfe4a4" ]; then
    echo "v21 base v20 fixture drifted" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
stumps_sha=$(shasum -a 256 "$complete_fixture" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment18" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1-v21.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) post-STUMPS head phase v21.'
    echo '# schema: stems-head-phase-prefix-v21'
    echo '# Bounded order-21 derivative: orders 1-20 execute linkSides without persistent snapshots; order 21 emits existing-stem retry envelope/result plus continuation.'
    echo '# This minimized scope is intentional for deterministic replay under the full-snapshot heap limit.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v21 page chula.png#1 system 1 rows $row_count baseProbeSourceSha256 $base_probe_sha baseV20RunnerSourceSha256 $v20_runner_sha baseV20FixtureSha256 $v20_fixture_sha fragmentSourceSha256 $fragment_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedOrder21ExistingStemRetry javaEvidence ReturnedBeforeTwentySecondHead"
} > "$out"
echo "wrote $out"
