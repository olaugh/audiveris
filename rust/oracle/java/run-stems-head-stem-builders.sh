#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

if [ "$#" -gt 1 ]; then
    echo "usage: $0 [<path>:<sheet>]" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
head_builder_xmx=${AUDIVERIS_HEAD_BUILDER_XMX:-48g}
probe_source="$repo_root/rust/oracle/java/StemsHeadStemBuilderProbe.java"
head_seed_fixture="$repo_root/rust/oracle/heads-seed-pass.txt"
head_range_fixture="$repo_root/rust/oracle/heads-range-pass.txt"
beam_stump_fixture="$repo_root/rust/oracle/stems-beam-stumps.txt"
head_stump_fixture="$repo_root/rust/oracle/stems-head-stumps.txt"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi
if [ "$(shasum -a 256 "$head_seed_fixture" | awk '{print $1}')" != \
        aca3cd20941846ae0eab9b4c1e56b3c9959afb6ed649519888b854e2b68f0414 ] || \
        [ "$(shasum -a 256 "$head_range_fixture" | awk '{print $1}')" != \
        35a8d063d557979b9d5e948c279a6228c42ffd3fb5a7784d236779b490740770 ] || \
        [ "$(shasum -a 256 "$beam_stump_fixture" | awk '{print $1}')" != \
        902478763d2897eb0d3f031a0895bee7d91a5a7bf8acf8188bf752273e149f14 ] || \
        [ "$(shasum -a 256 "$head_stump_fixture" | awk '{print $1}')" != \
        dd0247fbd992c7ec40351040efd336f98c8efa88bab0eef10c744430252e966e ]; then
    echo "bounded HEADS baseline fixture hash mismatch" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-head-builders-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-head-builders-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" -cp "$probe_cp" -d "$probe_classes" "$probe_source"

run_probe ()
{
    target=$1
    (
        cd "$repo_root/app"
        env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
            -XX:+UnlockExperimentalVMOptions \
            -XX:+UseEpsilonGC \
            -Xmx"$head_builder_xmx" \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -Daudiveris.rustport.headSeedFixture="$head_seed_fixture" \
            -Daudiveris.rustport.headRangeFixture="$head_range_fixture" \
            -Daudiveris.rustport.beamStumpFixture="$beam_stump_fixture" \
            -Daudiveris.rustport.headStumpFixture="$head_stump_fixture" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsHeadStemBuilderProbe "$target"
    )
}

if [ "$#" -eq 0 ]; then
    set -- "$repo_root/data/examples/chula.png:1"
fi

set +e
{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx"$head_builder_xmx" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsHeadStemBuilderProbe --header
    for target in "$@"; do
        run_probe "$target"
    done
} > "$probe_output"
probe_status=$?
set -e
if [ "$probe_status" -ne 0 ]; then
    tail -20 "$probe_output" >&2
    exit "$probe_status"
fi

body_sha=$(shasum -a 256 "$probe_output" | awk '{print $1}')
body_lines=$(wc -l < "$probe_output" | tr -d ' ')
body_bytes=$(wc -c < "$probe_output" | tr -d ' ')
if [ "$body_bytes" -ge 100000000 ]; then
    echo "head StemBuilder page body exceeds 100,000,000-byte fixture guard" >&2
    exit 1
fi

page_label=$(awk '/^stemsheadbuilderpagesummary / { print $2 }' "$probe_output")
case "$page_label" in
    allegretto.png#1)
        expected_body_sha=59379d10d0fabaab8d694fcd12400c1535a7fc8758ffa427a78a260f6c9a6cb5
        expected_body_lines=57371
        expected_body_bytes=16869864
        expected_page_hash=11ed50600e3262b7
        ;;
    BachInvention5.jpg#1)
        expected_body_sha=a387886a910a67f01737e2157ab4a79b0d1ce6760bfa87c767fa3e84b6d13b2f
        expected_body_lines=182405
        expected_body_bytes=53577681
        expected_page_hash=5a73c2dc2daeb4d4
        ;;
    batuque.png#1)
        expected_body_sha=2011ae02307311a6b6b9deded093154f9d730e82bf14473fdc6c0ce61e83dc16
        expected_body_lines=57573
        expected_body_bytes=16608267
        expected_page_hash=6772265079b4a6cf
        ;;
    carmen.png#1)
        expected_body_sha=3e8c6d9290e4b7aa330c5575afa83a1fec11faf029d09cd525a0f2449a356ad0
        expected_body_lines=75555
        expected_body_bytes=22025596
        expected_page_hash=1b55e98b9ce8308f
        ;;
    chula.png#1)
        expected_body_sha=0192b264d8ee80dcf7dde435451dfb18a44b498c70f0b741e903120251bc23c6
        expected_body_lines=53833
        expected_body_bytes=15336003
        expected_page_hash=029f3c74392f96b4
        ;;
    cucaracha.png#1)
        expected_body_sha=5fe7b525028ca8cc0fe28b971f8d6e81d16f265e561f81abc6638a1fb9fb8bc0
        expected_body_lines=75110
        expected_body_bytes=21838286
        expected_page_hash=6d3b4087df9742b5
        ;;
    hove.png#1)
        expected_body_sha=0ea595037937a96b4e474b0fb2d2bd572e4b7e4e5f9efe7c16cf8d170fca201e
        expected_body_lines=56391
        expected_body_bytes=15701515
        expected_page_hash=407e4094185259c3
        ;;
    zizi.png#1)
        expected_body_sha=5602434b8e6756695648d5e83f7b6980176cb1e415c8f394bf7e6cf9adbfbe47
        expected_body_lines=35503
        expected_body_bytes=9972353
        expected_page_hash=e9d648ab45c2568e
        ;;
    *)
        echo "head StemBuilder target is outside the frozen eight-page corpus: $page_label" >&2
        exit 1
        ;;
esac

actual_page_hash=$(awk '/^stemsheadbuilderpagesummary / {
    for (i = 3; i <= NF; i += 2) f[$i] = $(i + 1)
    print f["hash"]
}' "$probe_output")

if [ "$body_sha" != "$expected_body_sha" ] || \
        [ "$body_lines" -ne "$expected_body_lines" ] || \
        [ "$body_bytes" -ne "$expected_body_bytes" ]; then
    echo "head StemBuilder byte pin mismatch for $page_label: $body_sha/$body_lines/$body_bytes pageHash $actual_page_hash" >&2
    exit 1
fi

if [ "$(grep -Fxc '# stemsheadbuilder schema 1' "$probe_output")" -ne 1 ]; then
    echo "missing or duplicate head StemBuilder schema header" >&2
    exit 1
fi

if ! awk -v expected_page_hash="$expected_page_hash" '
    function need(name, fields) { schema[name] = fields; required[name] = 1 }
    function fail(message) { print "head StemBuilder semantic pin: " message > "/dev/stderr"; bad++ }
    BEGIN {
        need("stemsheadbuilderpage", "systems,staves,family")
        need("stemsheadbuilderoriginbaseline", "rawSeedCandidates,checkedSeedCandidates,checkedSeedStructuralKeys,modeledRegistry,registrySha256")
        need("stemsheadbuilderseedduplicates", "system,keptSeeds,duplicateStructuralKeys,extraOccurrences,duplicateSha256")
        need("stemsheadbuilderregistryboundary", "system,phase,modeledBefore,modeledAfter,shaBefore,shaAfter")
        need("stemsheadbuilderstumpreg", "system,event,kind,source,result,beamOnlyResult,actionDiff,canonicalAlias,canonical,preOriginCategories,attachment,bounds,orientation,weight,runs,sha256")
        need("stemsheadbuildersystem", "system,profile,inspectProfile,interline,bounds,beamSigOrder,beamXOrder,headSigOrder,headXOrder,seeds,modeledRegistry,maxStemThickness,maxLineSectionDx,maxStemAlignmentDx,maxStemAlignmentDy,minCoreSectionLength,minSideRatio,gap0,gap1,gap2,gap3,gap4")
        need("stemsheadbuilderbeamreg", "system,builder,ordinal,result,canonicalAlias,canonical,preOriginCategories,beamOnlyResult,beamOnlyPreOriginCategories,actionDiff,headToLaterBeamReuse,bounds,orientation,weight,runs,sha256")
        need("stemsheadbuilderbeamchron", "system,ordinal,beamSig,bAlias,vSide,profile,filaments,glyphNew,glyphReuse,bArenaBefore,bArenaAfter,modeledRegistryBefore,modeledRegistryAfter,registryShaBefore,registryShaAfter,registrationActionSha256")
        need("stemsheadbuilderhead", "system,xOrdinal,sigOrdinal,staff,shape,bounds,center,grade,vip,glyph,glyphWeight,glyphRuns")
        need("stemsheadbuilder", "system,builder,head,cornerOrdinal,corner,alias,hSide,vSide,profile,cYDir,builderYDir,directionDiverges,ref,theo,luBounds,startStump,modeledRegistryBefore,bArenaBefore")
        need("stemsheadbuilderinputseed", "system,builder,ordinal,glyph,bounds")
        need("stemsheadbuilderseedsource", "system,builder,sourceOrdinal,systemSeedOrdinal,glyph,bounds,contrib,minContrib,distance,maxDistance,action")
        need("stemsheadbuilderseedsort", "system,builder,input,output,sourceOrdinal,systemSeedOrdinal,glyph,contrib")
        need("stemsheadbuilderseedoverlap", "system,builder,sortedOrdinal,glyph,conflict,action")
        need("stemsheadbuilderinputtarget", "system,builder,ordinal,kind,alias,action|system,builder,ordinal,kind,alias,action,beforeArena,best,bestDx,cross")
        need("stemsheadbuildervsection", "system,builder,sourceCount,acceptedCount,acceptedSourceOrdinals,reasons,rejectSha256")
        need("stemsheadbuilderhsection", "system,builder,sourceCount,acceptedCount,acceptedSourceOrdinals,reasons,rejectSha256")
        need("stemsheadbuilderfilamentmember", "system,builder,filament,ordinal,alias,orientation,bounds,weight,runs")
        need("stemsheadbuilderfilament", "system,builder,ordinal,members,bounds,weight,centerLine,meanThickness,meanDistance,length")
        need("stemsheadbuilderglyphreg", "system,builder,ordinal,result,canonicalAlias,canonical,originCategories,bounds,orientation,weight,runs,sha256")
        need("stemsheadbuilderalign", "system,builder,phase,ordinal,startStump,promotedOccurrence,firstOccurrence,secondOccurrence,first,second,firstDeskew,secondDeskew,dy,maxDy,dyBypass,dx,maxDx,aligned,selectedAlienOccurrence,actualRemovedOccurrence,alien,tieRemoveSecond")
        need("stemsheadbuilderalignresult", "system,builder,phase,removedStructuralKeys,retainedOccurrences")
        need("stemsheadbuildertargetfilter", "system,builder,ordinal,kind,alias,stump,removedByStructuralSeed,action")
        need("stemsheadbuilderlasthead", "system,builder,lastHeadY,pastSeedDrops,pastVSectionDrops,pastHSectionDrops")
        need("stemsheadbuilderseedfilter", "system,builder,ordinal,glyph,removedStructural,retainedIdentity,pastLastHead")
        need("stemsheadbuilderheadparts", "system,builder,event,glyph,head,yOverlap,weight,removed,remain,threshold,vip,lowRemain,action")
        need("stemsheadbuilderchunkduplicates", "system,builder,attempts,duplicateStructuralKeys,extraOccurrences,duplicateSha256")
        need("stemsheadbuilderchunkfilter", "system,builder,inputOrdinal,event,glyph,finalOrdinal,action")
        need("stemsheadbuilderseeditemfilter", "system,builder,ordinal,glyph,duplicateTargetIdentity,duplicateAlias,startYOverlap,contrib,action")
        need("stemsheadbuildersortaudit", "system,builder,phase,items,strictCycles,equivalenceInconsistencies,offenderSha256,jdk25MiniTimSort")
        need("stemsheadbuildersort", "system,builder,phase,input,output,alias,kind,line,ref,key1,key2,equalInputs,stableEqualPredecessors")
        need("stemsheadbuilderitempre", "system,builder,creationOrdinal,sortedOrdinal,kind,alias,line,glyph,contrib")
        need("stemsheadbuildergap", "system,builder,ordinal,itemIndex,itemAlias,start,stop,lastBefore,gap,maxGap,epsilon,action,insertedLine,insertedContrib")
        need("stemsheadbuilderitem", "system,builder,ordinal,kind,alias,line,glyph,contrib")
        need("stemsheadbuilderlength", "system,builder,profile,threshold,length,replayLength")
        need("stemsheadbuilderend", "system,builder,seeds,items,lengths,filaments,glyphNew,glyphReuse,bArenaBefore,bArenaAfter,anchorsCreated,sigVertexDelta,sigEdgeDelta,stemInterDelta,systemStemDelta,linkMutations,sbAssigned,modeledRegistryBefore,modeledRegistryAfter,registryShaBefore,registryShaAfter,registrationActionSha256")
        summary_schema = "system,stumpRegistrations,stumpGlyphNew,stumpGlyphReuse,stumpActionDiffs,beamBuilders,beamFilaments,beamGlyphNew,beamGlyphReuse,builders,topBuilders,bottomBuilders,directionDivergences,profileDivergences,stumplessStarts,anchorsCreated,vScans,vAccepts,hScans,hAccepts,filaments,filamentMembers,glyphNew,glyphReuse,lowRemainChunks,headPartDrops,items,gaps,lengths,sortCycles,sortEquivalence,inputTargets,removedTargets,inputSeeds,alignComparisons,alignRemovals,chunkDrops,seedItemDrops,preItems,sortRows,maxTargetSortItems,maxFinalSortItems,targetSortListsAtLeast32,finalSortListsAtLeast32,gapChecks,gapInserts,gapTruncations,seedSourceScans,retrieveSeedSortRows,maxRetrieveSeedSortItems,retrieveSeedSortListsAtLeast32,vipHeads,vipBuilders,smallHeads,removeVipOnly,keepNonVipBug,beamActionDiffs,headToLaterBeamReuses,seedDuplicateKeys,seedDuplicateExtraOccurrences,chunkDuplicateKeys,chunkDuplicateExtraOccurrences,sortAudits,sigMutations,systemStemMutations,linkMutations,unexpectedBuilderMutations,hash"
        need("stemsheadbuildersystemsummary", summary_schema)
        sub(/^system,/, "systems,", summary_schema)
        need("stemsheadbuilderpagesummary", summary_schema)
    }
    /^#/ { next }
    {
        family = $1
        if (!(family in schema)) { fail("unknown row family " family); next }
        if ((NF % 2) != 0) { fail("odd field count for " family); next }
        actual = ""
        delete value
        for (i = 3; i <= NF; i += 2) {
            actual = actual (actual == "" ? "" : ",") $i
            value[$i] = $(i + 1)
        }
        if (index("|" schema[family] "|", "|" actual "|") == 0) {
            fail("token schema mismatch for " family ": " actual)
        }
        seen[family]++
        rows[family]++
        if (index($0, "otherPreStems") != 0) fail("forbidden otherPreStems origin")
        if (family == "stemsheadbuildersystem") {
            if (value["profile"] != value["inspectProfile"]) profile_divergences++
            if (value["profile"] != 1 || value["inspectProfile"] != 1) fail("unfrozen profile")
        }
        if (family == "stemsheadbuildersortaudit") phase_audits[value["phase"]]++
        if (family == "stemsheadbuilderglyphreg" && value["result"] == "Reuse" \
                && value["originCategories"] == "-") fail("unmodeled C reuse")
        if (family == "stemsheadbuilderstumpreg" && value["result"] == "Reuse" \
                && value["preOriginCategories"] == "-") fail("external-only stump reuse")
        if (family == "stemsheadbuilderbeamreg") {
            if (value["result"] == "Reuse" && value["preOriginCategories"] == "-") {
                fail("unmodeled beam reuse")
            }
            if (value["beamOnlyResult"] == "Reuse" \
                    && value["beamOnlyPreOriginCategories"] == "-") {
                fail("unmodeled beam-only reuse")
            }
        }
        if (family == "stemsheadbuilderoriginbaseline") {
            for (key in value) origin[key] = value[key]
        }
        if (family == "stemsheadbuilderpagesummary") {
            for (key in value) page[key] = value[key]
        }
    }
    END {
        for (family in required) if (!seen[family]) fail("missing row family " family)
        if (rows["stemsheadbuilderpage"] != 1 || rows["stemsheadbuilderoriginbaseline"] != 1 \
                || rows["stemsheadbuilderpagesummary"] != 1) fail("page singleton algebra")
        if (rows["stemsheadbuilderhead"] * 4 != page["builders"]) fail("head/builder algebra")
        if (rows["stemsheadbuilder"] != page["builders"] \
                || rows["stemsheadbuilderend"] != page["builders"] \
                || rows["stemsheadbuilderlasthead"] != page["builders"] \
                || rows["stemsheadbuildervsection"] != page["builders"] \
                || rows["stemsheadbuilderhsection"] != page["builders"] \
                || rows["stemsheadbuilderchunkduplicates"] != page["builders"] \
                || rows["stemsheadbuilderalignresult"] != 2 * page["builders"]) {
            fail("per-builder row algebra")
        }
        if (rows["stemsheadbuilderlength"] != 5 * page["builders"] \
                || page["lengths"] != 5 * page["builders"]) fail("length algebra")
        if (rows["stemsheadbuildersortaudit"] != 3 * page["builders"] \
                || phase_audits["retrieveSeeds"] != page["builders"] \
                || phase_audits["targets"] != page["builders"] \
                || phase_audits["items"] != page["builders"] \
                || page["sortAudits"] != 3 * page["builders"]) fail("three-sort algebra")
        if (rows["stemsheadbuilderbeamchron"] != page["beamBuilders"] \
                || rows["stemsheadbuilderbeamreg"] != page["beamFilaments"] \
                || rows["stemsheadbuilderfilament"] != page["filaments"] \
                || rows["stemsheadbuilderglyphreg"] != page["filaments"] \
                || rows["stemsheadbuilderchunkfilter"] != page["filaments"] \
                || rows["stemsheadbuilderfilamentmember"] != page["filamentMembers"]) {
            fail("registration/filament algebra")
        }
        if (rows["stemsheadbuilderstumpreg"] != page["stumpRegistrations"] \
                || page["stumpRegistrations"] \
                        != page["stumpGlyphNew"] + page["stumpGlyphReuse"]) {
            fail("stump registration algebra")
        }
        if (rows["stemsheadbuilderinputtarget"] != page["inputTargets"] \
                || rows["stemsheadbuildertargetfilter"] != page["inputTargets"] \
                || rows["stemsheadbuilderinputseed"] != page["inputSeeds"] \
                || rows["stemsheadbuilderseedfilter"] != page["inputSeeds"] \
                || rows["stemsheadbuilderseeditemfilter"] != page["inputSeeds"] \
                || rows["stemsheadbuilderseedsource"] != page["seedSourceScans"] \
                || rows["stemsheadbuilderseedsort"] != page["retrieveSeedSortRows"] \
                || rows["stemsheadbuilderseedoverlap"] != page["retrieveSeedSortRows"]) {
            fail("seed/target row algebra")
        }
        if (rows["stemsheadbuilderalign"] != page["alignComparisons"] \
                || rows["stemsheadbuildergap"] != page["gapChecks"] \
                || rows["stemsheadbuilderitempre"] != page["preItems"] \
                || rows["stemsheadbuildersort"] != page["sortRows"] \
                || rows["stemsheadbuilderitem"] != page["items"]) fail("item/gap row algebra")
        if (rows["stemsheadbuildersystem"] != page["systems"] \
                || rows["stemsheadbuildersystemsummary"] != page["systems"] \
                || rows["stemsheadbuilderregistryboundary"] != page["systems"] \
                || rows["stemsheadbuilderseedduplicates"] != page["systems"]) {
            fail("system row algebra")
        }
        if (page["builders"] != page["topBuilders"] + page["bottomBuilders"] \
                || page["directionDivergences"] != 0 || page["profileDivergences"] != 0 \
                || profile_divergences != 0) fail("direction/profile algebra")
        if (page["targetSortListsAtLeast32"] != 0 \
                || page["finalSortListsAtLeast32"] != 0 \
                || page["retrieveSeedSortListsAtLeast32"] != 0) fail("unmodeled >=32 sort")
        if (page["vipHeads"] != 0 || page["vipBuilders"] != 0 || page["smallHeads"] != 0 \
                || page["removeVipOnly"] != 0 || page["headPartDrops"] != 0 \
                || page["keepNonVipBug"] != page["lowRemainChunks"]) fail("VIP headparts pin")
        if (page["seedDuplicateKeys"] != 0 || page["seedDuplicateExtraOccurrences"] != 0 \
                || page["chunkDuplicateKeys"] != 0 \
                || page["chunkDuplicateExtraOccurrences"] != 0) fail("duplicate pin")
        if (page["sigMutations"] + page["systemStemMutations"] + page["linkMutations"] \
                + page["unexpectedBuilderMutations"] != 0) fail("forbidden mutation pin")
        if (page["hash"] != expected_page_hash) {
            fail("page semantic hash " page["hash"] " expected " expected_page_hash)
        }
        if (origin["rawSeedCandidates"] < origin["checkedSeedCandidates"] \
                || origin["checkedSeedCandidates"] < origin["checkedSeedStructuralKeys"] \
                || origin["modeledRegistry"] <= 0) fail("origin baseline algebra")
        exit bad != 0
    }
' "$probe_output"; then
    exit 1
fi

cat "$probe_output"
printf 'stemsheadbuildercheckpoint schema 1 page %s pages %d builders %d lengths %d bodySha256 %s bodyLines %s bodyBytes %s probeSourceSha256 %s runnerSourceSha256 %s\n' \
    "$page_label" 1 \
    "$(awk '/^stemsheadbuilderpagesummary / {for(i=3;i<=NF;i+=2)f[$i]=$(i+1); print f["builders"]}' "$probe_output")" \
    "$(awk '/^stemsheadbuilderpagesummary / {for(i=3;i<=NF;i+=2)f[$i]=$(i+1); print f["lengths"]}' "$probe_output")" \
    "$body_sha" "$body_lines" "$body_bytes" \
    "$(shasum -a 256 "$probe_source" | awk '{print $1}')" \
    "$(shasum -a 256 "$0" | awk '{print $1}')"
