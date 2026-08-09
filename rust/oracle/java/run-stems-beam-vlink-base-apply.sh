#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

page_key=chula
if [ "$#" -ne 0 ]; then
    if [ "$#" -ne 2 ] || [ "$1" != "--page" ]; then
        echo "usage: $0 [--page chula|allegretto|batuque|carmen|cucaracha|hove|zizi|BachInvention5]" >&2
        exit 2
    fi
    page_key=$2
fi
if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkBaseApplyProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlink-base-apply.sh"

case "$page_key" in
    chula) page_file=chula.png; expected_systems=3 ;;
    allegretto) page_file=allegretto.png; expected_systems=3 ;;
    batuque) page_file=batuque.png; expected_systems=3 ;;
    carmen) page_file=carmen.png; expected_systems=5 ;;
    cucaracha) page_file=cucaracha.png; expected_systems=3 ;;
    hove) page_file=hove.png; expected_systems=5 ;;
    zizi) page_file=zizi.png; expected_systems=2 ;;
    BachInvention5) page_file=BachInvention5.jpg; expected_systems=6 ;;
    *) echo "unknown beam VLink base-apply page key: $page_key" >&2; exit 2 ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
create_stem_fixture="$repo_root/rust/oracle/stems-beam-create-stem-$page_key.txt"
reuse_check_fixture="$repo_root/rust/oracle/stems-beam-vlink-reuse-check-$page_key.txt"
for required in "$probe_cp_file" "$scheduler_fixture" "$expand_fixture" \
        "$create_stem_fixture" "$reuse_check_fixture"; do
    if [ ! -f "$required" ]; then
        echo "missing frozen base-apply input: $required" >&2
        exit 2
    fi
done

sha256_file()
{
    shasum -a 256 "$1" | awk '{print $1}'
}

field_from_row()
{
    row_prefix=$1
    field_name=$2
    fixture=$3
    awk -v prefix="$row_prefix" -v field="$field_name" '
        index($0, prefix " ") == 1 {
            for (i = 1; i < NF; i++) if ($i == field) print $(i + 1)
        }
    ' "$fixture"
}

scheduler_sha256=$(sha256_file "$scheduler_fixture")
expand_sha256=$(sha256_file "$expand_fixture")
create_stem_sha256=$(sha256_file "$create_stem_fixture")
reuse_check_sha256=$(sha256_file "$reuse_check_fixture")
create_scheduler_pin=$(field_from_row stemsbeamcreatestemcorpussummary schedulerFixtureSha256 "$create_stem_fixture")
create_expand_pin=$(field_from_row stemsbeamcreatestemcorpussummary expandFixtureSha256 "$create_stem_fixture")
reuse_scheduler_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary schedulerFixtureSha256 "$reuse_check_fixture")
reuse_expand_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary expandFixtureSha256 "$reuse_check_fixture")
reuse_create_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary createStemFixtureSha256 "$reuse_check_fixture")
create_page=$(awk '/^stemsbeamcreatestempage / { print $2 }' "$create_stem_fixture")
reuse_page=$(awk '/^stemsbeamvlinkreusecheckpage / { print $2 }' "$reuse_check_fixture")
if [ "$create_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$create_expand_pin" != "$expand_sha256" ] || \
        [ "$reuse_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$reuse_expand_pin" != "$expand_sha256" ] || \
        [ "$reuse_create_pin" != "$create_stem_sha256" ] || \
        [ "$create_page" != "$page_file#1" ] || [ "$reuse_page" != "$page_file#1" ]; then
    echo "boundary-14 predecessor provenance mismatch for $page_key" >&2
    exit 1
fi
if [ "$scheduler_sha256" = "$expand_sha256" ] || \
        [ "$scheduler_sha256" = "$create_stem_sha256" ] || \
        [ "$scheduler_sha256" = "$reuse_check_sha256" ] || \
        [ "$expand_sha256" = "$create_stem_sha256" ] || \
        [ "$expand_sha256" = "$reuse_check_sha256" ] || \
        [ "$create_stem_sha256" = "$reuse_check_sha256" ]; then
    echo "predecessor hashes are not independently discriminating" >&2
    exit 1
fi

probe_cp=$(sed -n '1p' "$probe_cp_file")
jgrapht_jar=$(printf '%s\n' "$probe_cp" | tr ':' '\n' | awk '/\/jgrapht-core\/1\.5\.2\/.+\/jgrapht-core-1\.5\.2\.jar$/ { print }')
jgrapht_count=$(printf '%s\n' "$jgrapht_jar" | awk 'NF { count++ } END { print count + 0 }')
if [ "$jgrapht_count" -ne 1 ] || [ ! -f "$jgrapht_jar" ]; then
    echo "classpath must contain exactly one JGraphT core 1.5.2 jar" >&2
    exit 1
fi
jgrapht_sha256=$(sha256_file "$jgrapht_jar")
expected_jgrapht_sha256=dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14
if [ "$jgrapht_sha256" != "$expected_jgrapht_sha256" ]; then
    echo "JGraphT core 1.5.2 artifact drift" >&2
    exit 1
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-vlink-base-apply-classes.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-beam-vlink-base-apply-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-beam-vlink-base-apply-pass2.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$pass_one" "$pass_two"' EXIT HUP INT TERM

"$JAVA_HOME/bin/javac" -Xlint:all -cp "$probe_cp" -d "$probe_classes" "$probe_source"

run_fresh_pass()
{
    system_id=1
    while [ "$system_id" -le "$expected_systems" ]; do
        (
            cd "$repo_root/app"
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -XX:+UnlockExperimentalVMOptions \
                -XX:+UseEpsilonGC \
                -Xmx48g \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.StemsBeamVLinkBaseApplyProbe \
                --system "$system_id" \
                "$repo_root/data/examples/$page_file:1" \
                "$scheduler_fixture" "$expand_fixture" "$create_stem_fixture" \
                "$reuse_check_fixture"
        )
        system_id=$((system_id + 1))
    done
}

# javac above and every runtime JVM below are foreground processes. Each is awaited and reaped
# before the next starts. The runner never uses '&'; maximum Java concurrency is exactly one.
run_fresh_pass > "$pass_one"
run_fresh_pass > "$pass_two"
if ! cmp -s "$pass_one" "$pass_two"; then
    echo "two fresh $page_key base-apply passes are not byte-identical" >&2
    exit 1
fi

schema_count=$(awk '$0 == "# schema: stems-beam-vlink-base-apply-v1" { count++ } END { print count + 0 }' "$pass_one")
if [ "$schema_count" -ne 1 ]; then
    echo "base-apply schema header is missing or duplicated" >&2
    exit 1
fi

census=$(awk -v systems="$expected_systems" '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    /^stemsbeamvlinkbaseapplypage / {
        fields(3); pageRows++
        if (f["headless"] != "true" ||
                f["graphHashMode"] != "SourceOrderStructuralEndpoints") bad++
    }
    /^stemsbeamvlinkbaseapplybaseline / {
        fields(3); baseline[f["scope"] ":" f["case"]]++
        if (f["interIndexScanned"] != f["interIndex"] ||
                f["sigVertexScanned"] != f["sigVertices"] ||
                f["interIndexStemObjectMatches"] !~ /^[01]$/ ||
                f["glyphActiveBeamIdMatches"] !~ /^[0-9]+$/ ||
                f["glyphOriginalBeamIdMatches"] !~ /^[0-9]+$/ ||
                f["glyphActiveStemIdMatches"] !~ /^[0-9]+$/ ||
                f["glyphOriginalStemIdMatches"] !~ /^[0-9]+$/ ||
                f["beamVip"] !~ /^(true|false)$/ ||
                f["beamRemoved"] !~ /^(true|false)$/) bad++
        if (f["case"] == "SourceRemovedSuppress") {
            if (f["interIndexBeamIndexOrdinal"] != "-" ||
                    f["interIndexBeamObjectMatches"] != 0 ||
                    f["interIndexBeamIdMatches"] != 0 ||
                    f["sigBeamObjectMatches"] != 0 || f["sigBeamVertexOrdinal"] != "-") bad++
        } else if (f["interIndexBeamIndexOrdinal"] !~ /^[0-9]+$/ ||
                f["interIndexBeamObjectMatches"] != 1 ||
                f["interIndexBeamIdMatches"] != 1 ||
                f["sigBeamObjectMatches"] != 1 ||
                f["sigBeamVertexOrdinal"] !~ /^[0-9]+$/) bad++
        if ((f["beamGroupVertexOrdinal"] == "-") != (f["beamGroupStateHash"] == "-")) bad++
        if (f["beamGroupStateHash"] != "-" &&
                (length(f["beamGroupStateHash"]) != 64 ||
                 f["beamGroupStateHash"] !~ /^[0-9a-f]+$/)) bad++
        if (f["stemInterId"] == 0 &&
                (f["interIndexStemIndexOrdinal"] != "-" ||
                 f["interIndexStemObjectMatches"] != 0 ||
                 f["interIndexStemIdMatches"] != 0 ||
                 f["glyphActiveStemIdMatches"] != 0 ||
                 f["glyphOriginalStemIdMatches"] != 0)) bad++
    }
    /^stemsbeamvlinkbaseapplypredecessorcompat / {
        fields(3); predecessorCompat++
        if (f["scope"] != "real" || f["case"] != "-" || f["phase"] != "Before" ||
                f["algorithm"] != "Boundary13FrozenV1" ||
                f["legacySigVertices"] !~ /^[0-9]+$/ ||
                f["legacySigEdges"] !~ /^[0-9]+$/ ||
                f["legacySystemStems"] !~ /^[0-9]+$/) bad++
        for (key in f) {
            if (key ~ /^(legacyInterIndexHash|legacySigHash|legacySigRelationStateHash|legacySystemStemsHash|legacyLinkerStateHash)$/ &&
                    (length(f[key]) != 64 || f[key] !~ /^[0-9a-f]+$/)) bad++
        }
    }
    /^stemsbeamvlinkbaseapplyrelationcallback / {
        fields(3); callbacks++
        if (f["chordMatches"] != 0 || f["chordInvalidations"] != 0 ||
                length(f["preStemIncidentHash"]) != 64 ||
                f["preStemIncidentHash"] !~ /^[0-9a-f]+$/ ||
                length(f["stemIncidentHash"]) != 64 ||
                f["stemIncidentHash"] !~ /^[0-9a-f]+$/ ||
                length(f["preBeamIncidentHash"]) != 64 ||
                f["preBeamIncidentHash"] !~ /^[0-9a-f]+$/ ||
                length(f["beamIncidentHash"]) != 64 ||
                f["beamIncidentHash"] !~ /^[0-9a-f]+$/) bad++
    }
    /^stemsbeamvlinkbaseapplydeltaguard / {
        fields(3); guards++
        if (f["stemGeometryUnchanged"] != "true" ||
                f["beamGeometryUnchanged"] != "true" ||
                f["beamGroupUnchanged"] != "true" ||
                f["beamGroupVertexOrdinalBefore"] != f["beamGroupVertexOrdinalAfter"] ||
                f["beamGroupStateHashBefore"] != f["beamGroupStateHashAfter"] ||
                f["glyphRegistriesUnchanged"] != "true" ||
                f["stopBeforeBLinkerFlagMutation"] != "true" ||
                f["stopBeforeSiblingBeamLinks"] != "true" ||
                f["stopBeforeHeadRelationLoop"] != "true") bad++
        if (f["scope"] != "real" && f["enclosingRealSheetUnchanged"] != "true") bad++
    }
    /^stemsbeamvlinkbaseapplysummary / {
        fields(3); summaries++
        if (f["chordMatches"] != 0) bad++
        if (f["scope"] == "real") {
            real++
            if (f["case"] != "-" || f["branch"] != "NewIdZero" ||
                    f["vertexAdded"] != "true" || f["applyReturned"] != "true" ||
                    f["edgeAdded"] != "true" ||
                    f["terminal"] != "ReadyBeforeBLinkerFlagMutation" ||
                    f["throwStage"] != "-") bad++
        } else if (f["scope"] == "synthetic") {
            supported++
            supportedOrder = supportedOrder (supportedOrder == "" ? "" : ",") f["case"]
            if (f["terminal"] != "ReadyBeforeBLinkerFlagMutation" ||
                    f["throwStage"] != "-") bad++
        } else if (f["scope"] == "envelope") {
            envelope++
            envelopeOrder = envelopeOrder (envelopeOrder == "" ? "" : ",") f["case"]
            if (f["applyReturned"] != "-" || f["throwStage"] == "-" ||
                    f["terminal"] !~ /^EnvelopeThrownAt/) bad++
        } else bad++
    }
    END {
        expectedSupported = "ExistingFullSuccess,ExistingHookSuccess,ExistingDuplicateSuppress,SourceRemovedSuppress,TargetRemovedSuppress"
        expectedEnvelope = "MissingPositiveTargetThrows,NewVertexListenerThrows,EarlyEdgeListenerThrows,LaterEdgeListenerThrows"
        if (pageRows != 1 || predecessorCompat != systems || real != systems ||
                supported != 5 || envelope != 4 ||
                summaries != systems + 9 || callbacks != summaries || guards != summaries ||
                supportedOrder != expectedSupported || envelopeOrder != expectedEnvelope) bad++
        printf "%d:%d:%d:%d:%d\n", bad, real, supported, envelope, summaries
    }
' "$pass_one")
IFS=: read -r census_bad real_transactions supported_cases envelope_cases transaction_rows <<EOF
$census
EOF
if [ "$census_bad" -ne 0 ]; then
    echo "invalid $page_key base-apply census" >&2
    exit 1
fi

raw_pass_sha256=$(sha256_file "$pass_one")
raw_pass_lines=$(wc -l < "$pass_one" | tr -d ' ')
raw_pass_bytes=$(wc -c < "$pass_one" | tr -d ' ')
row_counts=""
for family in page baseline predecessorcompat frontier listeners vertextrace applydecision duplicatescan \
        edgestruct stemincident relationcallback beamincident result deltaguard summary; do
    label=stemsbeamvlinkbaseapply$family
    count=$(awk -v label="$label" '$1 == label { count++ } END { print count + 0 }' "$pass_one")
    if [ -n "$row_counts" ]; then row_counts="$row_counts,"; fi
    row_counts="$row_counts$label:$count"
done

probe_source_sha256=$(sha256_file "$probe_source")
runner_source_sha256=$(sha256_file "$runner_source")
source_hashes=""
append_source_hash()
{
    label=$1
    path=$2
    if [ ! -f "$repo_root/$path" ]; then
        echo "missing pinned source: $path" >&2
        exit 1
    fi
    value=$(sha256_file "$repo_root/$path")
    source_hashes="$source_hashes $label $value"
}
append_source_hash beamLinkerSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java
append_source_hash linkSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/Link.java
append_source_hash sigraphSourceSha256 app/src/main/java/org/audiveris/omr/sig/SIGraph.java
append_source_hash sigListenerSourceSha256 app/src/main/java/org/audiveris/omr/sig/SigListener.java
append_source_hash systemInfoSourceSha256 app/src/main/java/org/audiveris/omr/sheet/SystemInfo.java
append_source_hash sheetSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Sheet.java
append_source_hash basicIndexSourceSha256 app/src/main/java/org/audiveris/omr/util/BasicIndex.java
append_source_hash entityIndexSourceSha256 app/src/main/java/org/audiveris/omr/util/EntityIndex.java
append_source_hash glyphIndexSourceSha256 app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java
append_source_hash interIndexSourceSha256 app/src/main/java/org/audiveris/omr/sig/InterIndex.java
append_source_hash abstractEntitySourceSha256 app/src/main/java/org/audiveris/omr/util/AbstractEntity.java
append_source_hash abstractInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/AbstractInter.java
append_source_hash interSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/Inter.java
append_source_hash stemInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/StemInter.java
append_source_hash abstractChordInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/AbstractChordInter.java
append_source_hash headChordInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/HeadChordInter.java
append_source_hash relationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/Relation.java
append_source_hash supportSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/Support.java
append_source_hash beamStemRelationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/BeamStemRelation.java
append_source_hash beamRestRelationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/BeamRestRelation.java
append_source_hash chordStemRelationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/ChordStemRelation.java
append_source_hash abstractBeamInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/AbstractBeamInter.java
append_source_hash beamInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/BeamInter.java
append_source_hash beamHookInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/BeamHookInter.java
append_source_hash sheetStubSourceSha256 app/src/main/java/org/audiveris/omr/sheet/SheetStub.java
append_source_hash bookSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Book.java
append_source_hash gradleSourceSha256 app/build.gradle

cat "$pass_one"
printf 'stemsbeamvlinkbaseapplypagesummary %s systems %s realTransactions %s syntheticSupportedCases %s envelopeCases %s totalTransactions %s realNewVertexBranches %s realExistingVertexBranches 0 realEdgesAdded %s realReuseCensus 0 chordStemMatches 0 isolatedSheetsPerSyntheticCase true stopBeforeBLinkerFlagMutation true\n' \
    "$page_file#1" "$expected_systems" "$real_transactions" "$supported_cases" \
    "$envelope_cases" "$transaction_rows" "$expected_systems" "$expected_systems"
runtime_processes=$((2 * expected_systems))
total_processes=$((runtime_processes + 1))
printf 'stemsbeamvlinkbaseapplycorpussummary schema stems-beam-vlink-base-apply-v1 mode %s pages 1 pageRefs %s rowCounts %s probeSourceSha256 %s runnerSourceSha256 %s%s jgraphtCoreVersion 1.5.2 jgraphtCoreJarSha256 %s schedulerFixtureSha256 %s expandFixtureSha256 %s createStemFixtureSha256 %s reuseCheckFixtureSha256 %s predecessorSwapNegative true emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshRunsPerPage 2 freshRunsByteIdentical true rawPassSha256 %s freshJvmPerSystem true compilerJavaProcesses 1 runtimeJavaProcessesPerPass %s runtimeJavaProcesses %s totalJavaProcesses %s maximumConcurrentJavaProcesses 1 compilerJavaProcessReaped true runtimeJavaProcessesReaped true foregroundJavaProcessesOnly true backgroundJavaProcessesStarted 0 opaqueUnrelatedIdsExcludedFromStructuralHashes true\n' \
    "$page_key" "$page_file#1" "$row_counts" "$probe_source_sha256" \
    "$runner_source_sha256" "$source_hashes" "$jgrapht_sha256" \
    "$scheduler_sha256" "$expand_sha256" "$create_stem_sha256" \
    "$reuse_check_sha256" "$raw_pass_sha256" "$raw_pass_lines" "$raw_pass_bytes" \
    "$raw_pass_sha256" "$expected_systems" "$runtime_processes" "$total_processes"
