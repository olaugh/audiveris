#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

if [ "$#" -ne 0 ]; then
    echo "usage: $0" >&2
    exit 2
fi

if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$repo_root/rust/oracle/java/StemsBeamStemBuilderProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-stem-builders.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-stem-builders-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-stem-builders-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

run_probe ()
{
    target=$1
    (
        cd "$repo_root/app"
        env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
            -XX:+UnlockExperimentalVMOptions \
            -XX:+UseEpsilonGC \
            -Xmx48g \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsBeamStemBuilderProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx48g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamStemBuilderProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

summary_fields='systems beams smallBeams heads smallHeads initialBs initialVs bVisits anchorSkips anchorsCreated finalBs builders topBuilders bottomBuilders directionDivergences profile3Builders profile4Builders stumplessStarts inputSeeds removedSeeds retainedSeeds inputTargets removedTargets retainedTargets inputBTargets removedBTargets retainedBTargets inputCTargets removedCTargets retainedCTargets alignComparisons alignRemovals alignTieRemoveSecond vSectionScans vSectionAccepts vSectionOutside vSectionWide vSectionStumpOverlap vSectionPastHead vSectionDistance hSectionScans hSectionAccepts hSectionOutside hSectionWide hSectionPastHead selectedSectionRows filaments filamentMembers filamentRegistrations externalMembers glyphAttempts glyphNew glyphReuse reuseSeedOrigins reuseBeamStumpOrigins reuseHeadStumpOrigins reuseRejectedHeadStumpOrigins reusePriorChunkOrigins reuseRawBeamOrigins reuseHeadOrigins reuseLedgerOrigins reuseGridOrigins reuseSigOtherOrigins reuseUnmodeledOrigins chunkRemovals chunkSeedDrops chunkUnalignedDrops chunkStartDrops chunkHeadPartsDrops seedItemDrops preItems preStart preB preC preSeed preChunk sortAudits sortRows sortCycles sortEquivalence maxTargetSortItems maxFinalSortItems sortListsAtLeast32 gapChecks gapInserts gapTruncations finalItems finalStart finalB finalC finalSeed finalChunk finalGap lengthRows builderChecks sigMutations systemStemMutations linkMutations cBuilderMutations unexpectedBuilderMutations'

summary_signature=$(awk -v field_list="$summary_fields" '
    BEGIN {
        count = split(field_list, names, " ")
        for (i = 1; i <= count; i++) required[names[i]] = 1
        required["hash"] = 1
    }
    /^stemsbeambuilderpagesummary / {
        pages++
        for (key in fields) delete fields[key]
        for (key in seen) delete seen[key]
        for (i = 3; i <= NF; i += 2) {
            key = $i
            if (seen[key]++) duplicates++
            fields[key] = $(i + 1)
        }
        for (key in required) if (!(key in fields)) missing++
        for (i = 1; i <= count; i++) {
            key = names[i]
            if (key == "maxTargetSortItems" || key == "maxFinalSortItems") {
                if (fields[key] > totals[key]) totals[key] = fields[key]
            } else {
                totals[key] += fields[key]
            }
        }
    }
    END {
        printf "%d:%d:%d", pages, missing, duplicates
        for (i = 1; i <= count; i++) printf ":%d", totals[names[i]]
        printf "\n"
    }
' "$probe_output")

expected_signature='8:0:0:30:803:0:3521:0:2116:2417:2145:29:145:2261:2417:1390:1027:1:512:1905:590:2169:215:1954:6676:6:6670:1617:0:1617:5059:6:5053:1475:114:3:2851879:2174:2834360:4655:7267:2973:450:2615734:4524:2602329:6507:2374:6698:1442:3998:1442:0:1442:799:643:103:78:93:142:464:0:0:0:0:0:0:175:103:69:3:0:1930:10378:2417:1617:5053:24:1267:4657:14631:0:0:11:14:0:9660:320:563:9417:2417:1617:4063:2:998:320:12085:35419:0:0:0:0:0'
if [ "$summary_signature" != "$expected_signature" ]; then
    echo "unexpected beam StemBuilder corpus totals" >&2
    echo "observed: $summary_signature" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeambuilderpage / { pages++ }
    /^stemsbeambuildersystem / { systems++ }
    /^stemsdiagnosticbeambuilderglyphinventory / { inventories++ }
    /^stemsbeambuilder / { builders++ }
    /^stemsbeambuildertargetfilter / { targets++ }
    /^stemsbeambuilderalign / { aligns++ }
    /^stemsbeambuilderseedfilter / { seeds++ }
    /^stemsbeambuildervsection / { vSections++ }
    /^stemsbeambuilderhsection / { hSections++ }
    /^stemsbeambuilderfilamentmember / { members++ }
    /^stemsbeambuilderfilament / { filaments++ }
    /^stemsbeambuilderglyphreg / { glyphs++ }
    /^stemsbeambuilderchunkfilter / { chunks++ }
    /^stemsbeambuilderseeditemfilter / { seedItems++ }
    /^stemsbeambuildersortaudit / { sortAudits++ }
    /^stemsbeambuildersort / { sortRows++ }
    /^stemsbeambuilderitempre / { preItems++ }
    /^stemsbeambuildergap / { gaps++ }
    /^stemsbeambuilderitem / { finalItems++ }
    /^stemsbeambuilderlength / { lengths++ }
    /^stemsbeambuilderend / { ends++ }
    /^stemsbeambuildersystemsummary / { systemSummaries++ }
    /^stemsbeambuilderpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", pages, systems, inventories,
                builders, targets, aligns, seeds, vSections, hSections, members, filaments, glyphs
        printf ":%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", chunks, seedItems, sortAudits,
                sortRows, preItems, gaps, finalItems, lengths, ends, systemSummaries, pageSummaries
    }
' "$probe_output")
expected_rows='8:30:30:2417:6676:1475:2169:2417:2417:3998:1442:1442:1442:1954:4657:14631:10378:9660:9417:12085:2417:30:8'
if [ "$row_counts" != "$expected_rows" ]; then
    echo "beam StemBuilder detail row counts disagree with frozen totals" >&2
    echo "observed: $row_counts" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsbeambuildersystemsummary / {
        for (key in f) delete f[key]
        for (key in seen) delete seen[key]
        duplicate = 0
        for (i = 5; i <= NF; i += 2) {
            if (seen[$i]++) duplicate = 1
            f[$i] = $(i + 1)
        }
        bad = duplicate || f["systems"] != 1
        if (f["smallBeams"] + f["smallHeads"] != 0) bad = 1
        if (f["builders"] != f["initialVs"]) bad = 1
        if (f["builders"] != f["topBuilders"] + f["bottomBuilders"]) bad = 1
        if (f["directionDivergences"] > f["builders"]) bad = 1
        if (f["builders"] != f["profile3Builders"] + f["profile4Builders"]) bad = 1
        if (f["bVisits"] != f["initialBs"] + f["anchorSkips"]) bad = 1
        if (f["finalBs"] != f["initialBs"] + f["anchorsCreated"]) bad = 1
        if (f["inputSeeds"] != f["removedSeeds"] + f["retainedSeeds"]) bad = 1
        if (f["inputTargets"] != f["removedTargets"] + f["retainedTargets"]) bad = 1
        if (f["inputTargets"] != f["inputBTargets"] + f["inputCTargets"]) bad = 1
        if (f["removedTargets"] != f["removedBTargets"] + f["removedCTargets"]) bad = 1
        if (f["retainedTargets"] != f["retainedBTargets"] + f["retainedCTargets"]) bad = 1
        if (f["alignTieRemoveSecond"] > f["alignRemovals"] || f["alignRemovals"] > f["alignComparisons"]) bad = 1
        if (f["vSectionScans"] != f["vSectionAccepts"] + f["vSectionOutside"] + f["vSectionWide"] + f["vSectionStumpOverlap"] + f["vSectionPastHead"] + f["vSectionDistance"]) bad = 1
        if (f["hSectionScans"] != f["hSectionAccepts"] + f["hSectionOutside"] + f["hSectionWide"] + f["hSectionPastHead"]) bad = 1
        if (f["selectedSectionRows"] != f["vSectionAccepts"] + f["hSectionAccepts"]) bad = 1
        if (f["filaments"] != f["filamentRegistrations"] || f["filaments"] != f["glyphAttempts"]) bad = 1
        if (f["glyphAttempts"] != f["glyphNew"] + f["glyphReuse"]) bad = 1
        if (f["reuseUnmodeledOrigins"] != 0 || f["externalMembers"] != 0) bad = 1
        if (f["chunkRemovals"] != f["chunkSeedDrops"] + f["chunkUnalignedDrops"] + f["chunkStartDrops"] + f["chunkHeadPartsDrops"]) bad = 1
        if (f["preItems"] != f["preStart"] + f["preB"] + f["preC"] + f["preSeed"] + f["preChunk"]) bad = 1
        if (f["preStart"] != f["builders"]) bad = 1
        if (f["finalItems"] != f["finalStart"] + f["finalB"] + f["finalC"] + f["finalSeed"] + f["finalChunk"] + f["finalGap"]) bad = 1
        if (f["finalStart"] != f["builders"] || f["finalGap"] != f["gapInserts"]) bad = 1
        if (f["lengthRows"] != 5 * f["builders"]) bad = 1
        if (f["builderChecks"] != 3 * f["initialVs"] + 8 * f["heads"]) bad = 1
        if (f["sortListsAtLeast32"] != 0) bad = 1
        if (f["sigMutations"] + f["systemStemMutations"] + f["linkMutations"] + f["cBuilderMutations"] + f["unexpectedBuilderMutations"] != 0) bad = 1
        if (bad) badSystems++
    }
    END { print badSystems + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems beam StemBuilder system summaries violate invariants" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
body_lines=$(wc -l < "$probe_output" | tr -d ' ')
body_bytes=$(wc -c < "$probe_output" | tr -d ' ')
expected_probe_source_sha256=acf21ca496ec309138cb530c05827a4e5b639763d7f4cc5cc0bff4d2b8657646
expected_body_sha256=be00a4a2c5ee05b92b3fe70b157cbb246ab066ddd9fe89168db63657438672e4
if [ "$probe_source_sha256:$body_sha256:$body_lines:$body_bytes" != \
     "$expected_probe_source_sha256:$expected_body_sha256:91209:29164943" ]; then
    echo "unexpected beam StemBuilder source/body fingerprint" >&2
    echo "observed: $probe_source_sha256:$body_sha256:$body_lines:$body_bytes" >&2
    exit 1
fi

corpus_fields=$(awk -v field_list="$summary_fields" '
    BEGIN { count = split(field_list, names, " ") }
    /^stemsbeambuilderpagesummary / {
        for (i = 3; i <= NF; i += 2) fields[$i] = $(i + 1)
        for (i = 1; i <= count; i++) {
            key = names[i]
            if (key == "maxTargetSortItems" || key == "maxFinalSortItems") {
                if (fields[key] > totals[key]) totals[key] = fields[key]
            } else totals[key] += fields[key]
        }
    }
    END {
        for (i = 1; i <= count; i++) {
            if (i > 1) printf " "
            printf "%s %d", names[i], totals[names[i]]
        }
        printf "\n"
    }
' "$probe_output")

cat "$probe_output"
printf 'stemsbeambuildercorpussummary pages 8 %s probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s\n' \
    "$corpus_fields" "$probe_source_sha256" "$runner_source_sha256" "$body_sha256" \
    "$body_lines" "$body_bytes"
