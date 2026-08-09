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
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkReuseCheckProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlink-reuse-check.sh"

case "$page_key" in
    chula) page_file=chula.png; expected_systems=3 ;;
    allegretto) page_file=allegretto.png; expected_systems=3 ;;
    batuque) page_file=batuque.png; expected_systems=3 ;;
    carmen) page_file=carmen.png; expected_systems=5 ;;
    cucaracha) page_file=cucaracha.png; expected_systems=3 ;;
    hove) page_file=hove.png; expected_systems=5 ;;
    zizi) page_file=zizi.png; expected_systems=2 ;;
    BachInvention5) page_file=BachInvention5.jpg; expected_systems=6 ;;
    *)
        echo "unknown beam VLink reuse/check page key: $page_key" >&2
        exit 2
        ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
create_stem_fixture="$repo_root/rust/oracle/stems-beam-create-stem-$page_key.txt"
if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi
if [ ! -f "$scheduler_fixture" ] || [ ! -f "$expand_fixture" ] || \
        [ ! -f "$create_stem_fixture" ]; then
    echo "missing frozen scheduler/expand/createStem fixture for $page_key" >&2
    exit 2
fi

scheduler_sha256=$(shasum -a 256 "$scheduler_fixture" | awk '{print $1}')
expand_sha256=$(shasum -a 256 "$expand_fixture" | awk '{print $1}')
create_stem_sha256=$(shasum -a 256 "$create_stem_fixture" | awk '{print $1}')
create_scheduler_pin=$(awk '
    /^stemsbeamcreatestemcorpussummary / {
        for (i = 1; i < NF; i++) if ($i == "schedulerFixtureSha256") print $(i + 1)
    }
' "$create_stem_fixture")
create_expand_pin=$(awk '
    /^stemsbeamcreatestemcorpussummary / {
        for (i = 1; i < NF; i++) if ($i == "expandFixtureSha256") print $(i + 1)
    }
' "$create_stem_fixture")
create_page=$(awk '/^stemsbeamcreatestempage / { print $2 }' "$create_stem_fixture")
if [ "$create_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$create_expand_pin" != "$expand_sha256" ] || \
        [ "$create_page" != "$page_file#1" ]; then
    echo "createStem predecessor provenance mismatch for $page_key" >&2
    exit 1
fi
# Explicit negative certificate: swapping either distinct predecessor hash must be rejected.
if [ "$expand_sha256" = "$create_scheduler_pin" ] || \
        [ "$scheduler_sha256" = "$create_expand_pin" ]; then
    echo "predecessor fixture hashes are not independently discriminating" >&2
    exit 1
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-vlink-reuse-check-classes.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-beam-vlink-reuse-check-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-beam-vlink-reuse-check-pass2.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$pass_one" "$pass_two"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -Xlint:all \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

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
                org.audiveris.omr.rustport.StemsBeamVLinkReuseCheckProbe \
                --system "$system_id" \
                "$repo_root/data/examples/$page_file:1" \
                "$scheduler_fixture" \
                "$expand_fixture" \
                "$create_stem_fixture"
        )
        system_id=$((system_id + 1))
    done
}

# Two independent executions. Each pass launches exactly one foreground JVM at a time, waits for it
# to exit, and therefore reaps it before launching the next. No background Java process is started.
run_fresh_pass > "$pass_one"
run_fresh_pass > "$pass_two"
if ! cmp -s "$pass_one" "$pass_two"; then
    echo "two fresh $page_key VLink reuse/check passes are not byte-identical" >&2
    exit 1
fi
raw_pass_sha256=$(shasum -a 256 "$pass_one" | awk '{print $1}')

schema_count=$(awk '
    $0 == "# schema: stems-beam-vlink-reuse-check-v1" { count++ }
    END { print count + 0 }
' "$pass_one")
if [ "$schema_count" -ne 1 ]; then
    echo "VLink reuse/check schema header is missing or duplicated" >&2
    exit 1
fi

aggregate=$(awk '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    /^stemsbeamvlinkreusechecksummary / {
        fields(3)
        transactions++
        relationEntries += f["relationEntries"]
        linkedEntries += f["linkedEntries"]
        sideScans += f["sideScans"]
        reuses += f["realReuseCensus"]
        if (f["checkOutcome"] == "Accepted") accepted++
        else if (f["checkOutcome"] == "Rejected") rejected++
        else bad++
        if (f["terminal"] != "ReadyBeforeSigMutation" &&
                f["terminal"] != "BeamStemRejected") bad++
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d\n", bad, transactions, relationEntries,
                linkedEntries, sideScans, reuses, accepted, rejected
    }
' "$pass_one")
IFS=: read -r aggregate_bad transactions relation_entries linked_entries side_scans \
    real_reuses accepted_checks rejected_checks <<EOF
$aggregate
EOF
if [ "$aggregate_bad" -ne 0 ] || [ "$transactions" -ne "$expected_systems" ] || \
        [ "$relation_entries" -le 0 ] || [ "$linked_entries" -ne 0 ] || \
        [ "$side_scans" -ne 0 ] || [ "$real_reuses" -ne 0 ] || \
        [ $((accepted_checks + rejected_checks)) -ne "$expected_systems" ]; then
    echo "invalid real VLink reuse/check census" >&2
    exit 1
fi

certificate_errors=$(awk -v systems="$expected_systems" '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    /^stemsbeamvlinkreusecheckbaseline / {
        fields(3); s = f["system"] + 0; baseline[s]++; plan[s] = f["plan"]
        mode[s] = f["executionMode"]
    }
    /^stemsbeamvlinkreusecheckfrontier / {
        fields(3); s = f["system"] + 0; frontier[s]++
        relationCount[s] = f["relationCount"] + 0
        liveStemCount[s] = f["liveStemCount"] + 0
        if (f["plan"] != plan[s] || f["predecessorJoin"] != "Exact" ||
                f["createReturnedStemInterId"] != 0 ||
                f["createStemSigAttached"] != "false" ||
                f["createStemAbnormal"] != "false" ||
                f["createStemMedian"] == "-" || f["createStemWidth"] == "-" ||
                f["createStemBounds"] == "-" || liveStemCount[s] != 0 ||
                length(f["liveStemCatalogueHash"]) != 64 ||
                f["liveStemCatalogueHash"] !~ /^[0-9a-f]+$/) bad++
    }
    /^stemsbeamvlinkreusechecklivestem / {
        fields(3); s = f["system"] + 0; liveStemRows[s]++
        expectedOrdinal = liveStemRows[s] - 1
        idKey = s ":" f["javaInterId"]
        aliasKey = s ":" f["stemAlias"]
        if (f["plan"] != plan[s] || f["catalogOrdinal"] != expectedOrdinal ||
                f["sigVertexOrdinal"] !~ /^[0-9]+$/ ||
                f["stemAlias"] != "live:" expectedOrdinal ||
                f["javaInterId"] !~ /^[1-9][0-9]*$/ || seenInter[idKey]++ ||
                f["glyphId"] !~ /^[1-9][0-9]*$/ ||
                seenAlias[aliasKey]++ || f["glyph"] == "-" || f["grade"] == "-" ||
                f["median"] == "-" || f["width"] == "-" || f["bounds"] == "-" ||
                f["abnormal"] !~ /^(true|false)$/ || f["sigAttached"] != "true" ||
                length(f["stateHash"]) != 64 || f["stateHash"] !~ /^[0-9a-f]+$/) bad++
    }
    /^stemsbeamvlinkreusecheckreuseentry / {
        fields(3); s = f["system"] + 0; entries[s]++
        expectedOrdinal = entries[s] - 1
        if (f["plan"] != plan[s] || f["mapOrdinal"] != expectedOrdinal ||
                f["conditionRead"] != "true" || f["evidenceValidated"] != "true" ||
                f["sLinked"] != "false" || f["lookupState"] != "NotRead" ||
                f["scanState"] != "NotRead" || f["action"] != "SkipUnlinked" ||
                f["cAlias"] !~ /^h:[0-9]+:(LEFT|RIGHT):(TOP|BOTTOM)$/) bad++
    }
    /^stemsbeamvlinkreusecheckheadstemscan / { realScanRows++ }
    /^stemsbeamvlinkreusecheckselection / {
        fields(3); s = f["system"] + 0; selection[s]++
        if (f["plan"] != plan[s] || f["initialStemAlias"] != f["finalStemAlias"] ||
                f["initialStemInterId"] != 0 || f["finalStemInterId"] != 0 ||
                f["reused"] != "false" || f["selectedMapOrdinal"] != "-" ||
                f["selectedC"] != "-" || f["breakIndex"] != "-" ||
                f["entriesConditionRead"] != f["entriesTotal"] ||
                f["unreadSuffix"] != 0 || f["linkedEntries"] != 0 ||
                f["allLinked"] != "false" || f["reuseOutcome"] != "AllUnlinked" ||
                f["terminal"] != "ContinueToCheck" || f["finalGlyphId"] !~ /^[1-9][0-9]*$/ ||
                f["finalGrade"] == "-" || f["finalMedian"] == "-" ||
                f["finalWidth"] == "-" || f["finalBounds"] == "-" ||
                f["finalAbnormal"] != "false" || f["finalSigAttached"] != "false" ||
                length(f["finalStateHash"]) != 64 ||
                f["finalStateHash"] !~ /^[0-9a-f]+$/) bad++
    }
    /^stemsbeamvlinkreusecheckcheckcontext / {
        fields(3)
        if (f["scope"] == "real") {
            s = f["system"] + 0; context[s]++
            if (f["plan"] != plan[s] || f["case"] != "-" ||
                    f["xWeight"] !~ /\/3ff0000000000000$/ ||
                    f["yWeight"] !~ /\/4010000000000000$/ ||
                    f["intrinsicRatio"] !~ /\/3ff0000000000000$/ ||
                    f["portionComparison"] != "StrictLtGt" ||
                    f["gradeComparison"] != "InclusiveGe" ||
                    f["scaleStemThickness"] !~ /^[1-9][0-9]*$/) bad++
        }
    }
    /^stemsbeamvlinkreusecheckchecktrace / {
        fields(3)
        if (f["scope"] == "real") {
            s = f["system"] + 0; trace[s]++
            if (f["plan"] != plan[s] || f["crossX"] == "-" || f["crossY"] == "-" ||
                    f["portion"] !~ /^(LEFT|CENTER|RIGHT)$/ ||
                    f["grade"] == "-" || f["accepted"] !~ /^(true|false)$/) bad++
        }
    }
    /^stemsbeamvlinkreusecheckcheckresult / {
        fields(3)
        if (f["scope"] == "real") {
            s = f["system"] + 0; result[s]++
            if (f["plan"] != plan[s]) bad++
            if (f["checkOutcome"] == "Accepted") {
                if (f["linkNull"] != "false" || f["outgoing"] != "true" ||
                        f["relationClass"] != "BeamStemRelation" ||
                        f["portion"] !~ /^(LEFT|CENTER|RIGHT)$/ ||
                        f["dx"] == "-" || f["dy"] == "-" || f["grade"] == "-" ||
                        f["impacts"] == "-" || f["extension"] == "-" ||
                        f["terminal"] != "ReadyBeforeSigMutation") bad++
            } else if (f["checkOutcome"] == "Rejected") {
                if (f["linkNull"] != "true" || f["partnerAlias"] != "-" ||
                        f["outgoing"] != "-" || f["relationClass"] != "-" ||
                        f["terminal"] != "BeamStemRejected") bad++
            } else bad++
        }
    }
    /^stemsbeamvlinkreusecheckguard / {
        fields(3); s = f["system"] + 0; guard[s]++
        if (f["plan"] != plan[s] || f["allocatorBefore"] != f["allocatorAfter"] ||
                f["glyphActiveHashBefore"] != f["glyphActiveHashAfter"] ||
                f["glyphOriginalsHashBefore"] != f["glyphOriginalsHashAfter"] ||
                f["interIndexHashBefore"] != f["interIndexHashAfter"] ||
                f["sigHashBefore"] != f["sigHashAfter"] ||
                f["sigRelationStateHashBefore"] != f["sigRelationStateHashAfter"] ||
                f["systemStemsHashBefore"] != f["systemStemsHashAfter"] ||
                f["linkerStateHashBefore"] != f["linkerStateHashAfter"] ||
                f["lineStateHashBefore"] != f["lineStateHashAfter"] ||
                f["liveStemCatalogueHashBefore"] != f["liveStemCatalogueHashAfter"] ||
                f["relationInputHashBefore"] != f["relationInputHashAfter"] ||
                f["createStemStateUnchanged"] != "true" ||
                f["registriesUnchanged"] != "true" ||
                f["allocatorUnchanged"] != "true" || f["linesUnchanged"] != "true" ||
                f["systemStemsUnchanged"] != "true" ||
                f["liveStemsUnchanged"] != "true" ||
                f["interIndexUnchanged"] != "true" || f["sigUnchanged"] != "true" ||
                f["relationsUnchanged"] != "true" ||
                f["linkerFlagsUnchanged"] != "true" ||
                f["stopBeforeSigAddVertex"] != "true" ||
                f["stopBeforeRelationApply"] != "true" ||
                f["stopBeforeLinkerFlagMutation"] != "true") bad++
    }
    /^stemsbeamvlinkreusechecksummary / {
        fields(3); s = f["system"] + 0; summary[s]++
        if (f["relationEntries"] != relationCount[s] || f["linkedEntries"] != 0 ||
                f["sideScans"] != 0 || f["reused"] != "false" ||
                f["reuseOutcome"] != "AllUnlinked" || f["realReuseCensus"] != 0) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticreuseentry / {
        fields(3); syntheticEntry[f["case"]]++
        if (f["case"] == "ZeroMissingJavaNull" &&
                f["action"] != "JavaNullPointer") bad++
        if (f["case"] == "UniqueBreakUnreadSuffix" &&
                f["mapOrdinal"] == 1 &&
                (f["conditionRead"] != "false" || f["evidenceValidated"] != "false" ||
                 f["action"] != "UnreadAfterBreak")) bad++
        if (f["case"] == "MultipleContinue" &&
                (f["sideStemCount"] != 2 || f["action"] != "ContinueMultiple")) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticheadstemscan / {
        fields(3); syntheticScan++
        scanIdKey = f["targetStemInterId"]
        scanKey = f["case"] ":" f["mapOrdinal"]
        expectedScanOrdinal = syntheticScanPerEntry[scanKey]++
        if (f["plan"] != plan[1] ||
                f["case"] !~ /^(UniqueBreakUnreadSuffix|MultipleContinue)$/ ||
                f["scanOrdinal"] != expectedScanOrdinal ||
                f["scanOrderDomain"] != "HeadInterRelationsSourceOrder" ||
                f["sigEdgeIdentityDomain"] != "IsolatedSigEdgeSetSourceOrder" ||
                f["targetStemAlias"] !~ /^synthetic-live:[0-9]+$/ ||
                f["targetStemInterId"] !~ /^15000001(01|02)$/ ||
                f["targetSigAttached"] != "true" ||
                f["targetGlyphId"] != 1500000001 ||
                f["edgeHeadSide"] !~ /^(LEFT|RIGHT)$/ ||
                f["matchesParentSide"] != "true" ||
                length(f["headSnapshotHash"]) != 64 ||
                f["headSnapshotHash"] !~ /^[0-9a-f]+$/) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticselection / {
        fields(3); syntheticSelection[f["case"]]++
        if (f["origin"] != "IsolatedSyntheticSig" ||
                f["cardinalityModel"] != "ActualHeadInterGetSideStems" ||
                f["zeroMutation"] != "true" || f["finalMedian"] == "-" ||
                f["finalWidth"] == "-" || f["finalBounds"] == "-") bad++
        if (f["case"] == "ZeroMissingJavaNull" &&
                (f["cardinality"] != 0 || f["outcome"] != "JavaNullPointer" ||
                 f["checkOutcome"] != "NotReachedJavaNull")) bad++
        if (f["case"] == "UniqueBreakUnreadSuffix" &&
                (f["cardinality"] != 1 || f["outcome"] != "Selected" ||
                 f["unreadSuffix"] != 1 ||
                 f["finalStemAlias"] !~ /^synthetic-live:[0-9]+$/ ||
                 f["finalStemInterId"] !~ /^[1-9][0-9]*$/ ||
                 f["finalSigAttached"] != "true")) bad++
        if (f["case"] == "MultipleContinue" &&
                (f["cardinality"] != "multiple" || f["outcome"] != "NoUnique")) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticportion / {
        fields(3); portions[f["case"]]++
        if (f["case"] == "LeftBelow") expected = "LEFT"
        else if (f["case"] == "RightAbove") expected = "RIGHT"
        else expected = "CENTER"
        if (f["comparison"] != "StrictLtGt" || f["portion"] != expected) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticlivestem / {
        fields(3); syntheticLive++
        expectedOrdinal = syntheticLive - 1
        if (f["case"] != "SyntheticReuseTargets" ||
                f["origin"] != "IsolatedSyntheticSig" ||
                f["idNamespace"] != "isolated-1500000000" ||
                f["catalogOrdinal"] != expectedOrdinal ||
                f["stemAlias"] != ("synthetic-live:" expectedOrdinal) ||
                f["javaInterId"] !~ /^[1-9][0-9]*$/ ||
                f["firstHeadStemEdgeOrdinal"] !~ /^[0-9]+$/ ||
                f["glyphId"] !~ /^[1-9][0-9]*$/ || f["glyph"] == "-" ||
                f["grade"] == "-" || f["median"] == "-" || f["width"] == "-" ||
                f["bounds"] == "-" || f["sigAttached"] != "true" ||
                length(f["stateHash"]) != 64 || f["stateHash"] !~ /^[0-9a-f]+$/) bad++
        if (syntheticLive == 1) syntheticSharedGlyph = f["glyphId"]
        else if (f["glyphId"] != syntheticSharedGlyph) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticthreshold / {
        fields(3); threshold++
        if (f["grade"] != f["minGrade"] || f["comparison"] != "InclusiveGe" ||
                f["accepted"] != "true") bad++
    }
    /^stemsbeamvlinkreusechecksyntheticparallel / {
        fields(3); parallel++
        if (f["checkOutcome"] != "Rejected" || f["zeroMutation"] != "true" ||
                f["den"] !~ /\/(0000000000000000|8000000000000000)$/ ||
                (f["crossX"] !~ /Infinity|NaN/ && f["crossY"] !~ /Infinity|NaN/)) bad++
    }
    /^stemsbeamvlinkreusechecksyntheticintersection / {
        fields(3); intersection++
        if (f["case"] != "DistinctHorizontalInfNaN" ||
                f["den"] !~ /\/(0000000000000000|8000000000000000)$/ ||
                f["crossX"] !~ /Infinity/ || f["crossY"] !~ /NaN/ ||
                f["finiteInputs"] != "true" || f["exactLineUtil"] != "true") bad++
    }
    /^stemsbeamvlinkreusechecksyntheticguard / {
        fields(3); syntheticGuard++
        if (f["plan"] != plan[1] || f["origin"] != "IsolatedSyntheticSig" ||
                f["isolatedSigHashBefore"] != f["isolatedSigHashAfter"] ||
                f["liveStemHashBefore"] != f["liveStemHashAfter"] ||
                f["realAllocatorBefore"] != f["realAllocatorAfter"] ||
                f["realInterIndexHashBefore"] != f["realInterIndexHashAfter"] ||
                f["realSigHashBefore"] != f["realSigHashAfter"] ||
                f["realSigRelationStateHashBefore"] != f["realSigRelationStateHashAfter"] ||
                f["realSystemStemsHashBefore"] != f["realSystemStemsHashAfter"] ||
                f["realLinkerStateHashBefore"] != f["realLinkerStateHashAfter"] ||
                f["realLineStateHashBefore"] != f["realLineStateHashAfter"] ||
                f["isolatedGraphUnchanged"] != "true" ||
                f["realSheetUnchanged"] != "true" ||
                f["allocatorUnchanged"] != "true" ||
                f["interIndexUnchanged"] != "true" || f["sigUnchanged"] != "true" ||
                f["relationsUnchanged"] != "true" ||
                f["linkerFlagsUnchanged"] != "true" || f["zeroMutation"] != "true") bad++
    }
    END {
        if (realScanRows != 0 || syntheticLive != 2 || syntheticScan != 3 ||
                syntheticGuard != 1 || threshold != 1 || parallel != 1 || intersection != 1) bad++
        for (name in syntheticEntry) syntheticEntryTotal += syntheticEntry[name]
        if (syntheticEntryTotal != 4) bad++
        for (name in syntheticSelection) syntheticSelectionTotal += syntheticSelection[name]
        if (syntheticSelectionTotal != 3) bad++
        for (name in portions) portionTotal += portions[name]
        if (portionTotal != 4) bad++
        for (s = 1; s <= systems; s++) {
            expectedMode = (s == 1) ? "sheet-first" : "isolated-system-frontier"
            if (baseline[s] != 1 || frontier[s] != 1 || selection[s] != 1 ||
                    context[s] != 1 || trace[s] != 1 || result[s] != 1 ||
                    guard[s] != 1 || summary[s] != 1 ||
                    entries[s] != relationCount[s] || liveStemRows[s] != liveStemCount[s] ||
                    mode[s] != expectedMode) bad++
        }
        print bad + 0
    }
' "$pass_one")
if [ "$certificate_errors" -ne 0 ]; then
    echo "$certificate_errors VLink reuse/check certificate errors" >&2
    exit 1
fi

live_stem_rows=$(awk '/^stemsbeamvlinkreusechecklivestem / { count++ } END { print count + 0 }' "$pass_one")
printf 'stemsbeamvlinkreusecheckpagesummary %s#1 systems %s transactions %s relationEntries %s linkedEntries %s sideScans %s liveStemRows %s realReuses %s acceptedChecks %s rejectedChecks %s syntheticLiveStemRows 2 syntheticHeadScanRows 3 syntheticGuardRows 1 syntheticZeroCases 1 syntheticUniqueCases 1 syntheticMultipleCases 1 syntheticIntersectionCases 1 sigMutations 0 relationMutations 0 linkerFlagMutations 0\n' \
    "$page_file" "$expected_systems" "$transactions" "$relation_entries" \
    "$linked_entries" "$side_scans" "$live_stem_rows" "$real_reuses" "$accepted_checks" \
    "$rejected_checks" >> "$pass_one"

row_counts=$(awk '
    /^stemsbeamvlinkreusecheckpage / { page++ }
    /^stemsbeamvlinkreusecheckbaseline / { baseline++ }
    /^stemsbeamvlinkreusecheckfrontier / { frontier++ }
    /^stemsbeamvlinkreusechecklivestem / { liveStem++ }
    /^stemsbeamvlinkreusecheckreuseentry / { entry++ }
    /^stemsbeamvlinkreusecheckheadstemscan / { scan++ }
    /^stemsbeamvlinkreusecheckselection / { selection++ }
    /^stemsbeamvlinkreusecheckcheckcontext / { context++ }
    /^stemsbeamvlinkreusecheckchecktrace / { trace++ }
    /^stemsbeamvlinkreusecheckcheckresult / { result++ }
    /^stemsbeamvlinkreusecheckguard / { guard++ }
    /^stemsbeamvlinkreusechecksummary / { summary++ }
    /^stemsbeamvlinkreusechecksyntheticreuseentry / { syntheticEntry++ }
    /^stemsbeamvlinkreusechecksyntheticlivestem / { syntheticLive++ }
    /^stemsbeamvlinkreusechecksyntheticheadstemscan / { syntheticScan++ }
    /^stemsbeamvlinkreusechecksyntheticselection / { syntheticSelection++ }
    /^stemsbeamvlinkreusechecksyntheticportion / { syntheticPortion++ }
    /^stemsbeamvlinkreusechecksyntheticthreshold / { syntheticThreshold++ }
    /^stemsbeamvlinkreusechecksyntheticparallel / { syntheticParallel++ }
    /^stemsbeamvlinkreusechecksyntheticintersection / { syntheticIntersection++ }
    /^stemsbeamvlinkreusechecksyntheticguard / { syntheticGuard++ }
    /^stemsbeamvlinkreusecheckpagesummary / { pageSummary++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n",
                page, baseline, frontier, liveStem, entry, scan, selection, context, trace,
                result, guard, summary, syntheticLive, syntheticEntry, syntheticScan,
                syntheticSelection,
                syntheticPortion, syntheticThreshold, syntheticParallel,
                syntheticIntersection, syntheticGuard, pageSummary
    }
' "$pass_one")

body_sha256=$(shasum -a 256 "$pass_one" | awk '{print $1}')
body_lines=$(wc -l < "$pass_one" | tr -d ' ')
body_bytes=$(wc -c < "$pass_one" | tr -d ' ')
probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
beam_linker_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java" | awk '{print $1}')
beam_stem_relation_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamStemRelation.java" | awk '{print $1}')
head_inter_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/HeadInter.java" | awk '{print $1}')
head_stem_relation_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/HeadStemRelation.java" | awk '{print $1}')
line_util_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/math/LineUtil.java" | awk '{print $1}')
scale_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Scale.java" | awk '{print $1}')
abstract_connection_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/AbstractConnection.java" | awk '{print $1}')
support_impacts_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/SupportImpacts.java" | awk '{print $1}')
support_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Support.java" | awk '{print $1}')
grade_impacts_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/GradeImpacts.java" | awk '{print $1}')
grade_util_source_sha256=$(shasum -a 256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/GradeUtil.java" | awk '{print $1}')

cat "$pass_one"
printf 'stemsbeamvlinkreusecheckcorpussummary schema stems-beam-vlink-reuse-check-v1 mode %s pages 1 pageRefs %s#1 rowCounts %s probeSourceSha256 %s runnerSourceSha256 %s beamLinkerSourceSha256 %s beamStemRelationSourceSha256 %s headInterSourceSha256 %s headStemRelationSourceSha256 %s lineUtilSourceSha256 %s scaleSourceSha256 %s abstractConnectionSourceSha256 %s supportImpactsSourceSha256 %s supportSourceSha256 %s gradeImpactsSourceSha256 %s gradeUtilSourceSha256 %s schedulerFixtureSha256 %s expandFixtureSha256 %s createStemFixtureSha256 %s pairedFixtureSwapNegative true emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshRunsPerPage 2 freshRunsByteIdentical true rawPassSha256 %s freshJvmPerPage false freshJvmPerSystem true compilerJavaProcesses 1 runtimeJavaProcessesPerPass %s runtimeJavaProcesses %s totalJavaProcesses %s maximumConcurrentJavaProcesses 1 freshSheetPerSystem true compilerJavaProcessReaped true runnerJavaProcessesReaped true foregroundJavaProcessesOnly true backgroundJavaProcessesStarted 0 realReuseCensus %s\n' \
    "$page_key" "$page_file" "$row_counts" "$probe_source_sha256" \
    "$runner_source_sha256" "$beam_linker_source_sha256" \
    "$beam_stem_relation_source_sha256" "$head_inter_source_sha256" \
    "$head_stem_relation_source_sha256" "$line_util_source_sha256" \
    "$scale_source_sha256" "$abstract_connection_source_sha256" \
    "$support_impacts_source_sha256" "$support_source_sha256" \
    "$grade_impacts_source_sha256" "$grade_util_source_sha256" \
    "$scheduler_sha256" "$expand_sha256" \
    "$create_stem_sha256" "$body_sha256" "$body_lines" "$body_bytes" \
    "$raw_pass_sha256" "$expected_systems" $((2 * expected_systems)) \
    $((2 * expected_systems + 1)) "$real_reuses"
