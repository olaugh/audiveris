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
probe_source="$repo_root/rust/oracle/java/StemsBeamCreateStemProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-create-stem.sh"

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
        echo "unknown beam-createStem page key: $page_key" >&2
        exit 2
        ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi
if [ ! -f "$scheduler_fixture" ] || [ ! -f "$expand_fixture" ]; then
    echo "missing frozen scheduler/expand fixture for $page_key" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-create-stem-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-create-stem-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -Xlint:all \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

# Each target system owns one fresh foreground Epsilon JVM and one fresh HEADS sheet. Processes are
# launched serially, awaited, and reaped before the next system. This bounds Epsilon retention while
# keeping weak GlyphIndex identities deterministic. Systems after the first are explicitly isolated
# allocator experiments, not a fabricated cross-system execution chronology.
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
            org.audiveris.omr.rustport.StemsBeamCreateStemProbe \
            --system "$system_id" \
            "$repo_root/data/examples/$page_file:1" \
            "$scheduler_fixture" \
            "$expand_fixture"
    ) >> "$probe_output"
    system_id=$((system_id + 1))
done

aggregate=$(awk '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    /^stemsbeamcreatestemsummary / {
        fields(3)
        transactions++
        if (f["registration"] == "New") newGlyphs++
        else if (f["registration"] == "ReuseActive") reusedGlyphs++
        else if (f["registration"] == "ReinsertOriginal") reinsertedGlyphs++
        else bad++
        if (f["disposition"] == "CreatedChecked") checkedStems++
        else if (f["disposition"] == "Reused") reusedStems++
        else if (f["disposition"] == "CreatedArtificial") artificialStems++
        else if (f["disposition"] == "Rejected") rejectedStems++
        else bad++
        allocatorDelta += f["allocatorDelta"]
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", bad, transactions, newGlyphs,
                reusedGlyphs, reinsertedGlyphs, checkedStems, reusedStems,
                artificialStems, rejectedStems, allocatorDelta
    }
' "$probe_output")
IFS=: read -r aggregate_bad transactions new_glyphs reused_glyphs reinserted_glyphs \
    checked_stems reused_stems artificial_stems rejected_stems allocator_delta <<EOF
$aggregate
EOF
if [ "$aggregate_bad" -ne 0 ] || [ "$transactions" -ne "$expected_systems" ] || \
        [ "$reused_stems" -ne 0 ]; then
    echo "invalid per-system beam-createStem summaries" >&2
    exit 1
fi
printf 'stemsbeamcreatestempagesummary %s#1 systems %s transactions %s newGlyphs %s reusedGlyphs %s reinsertedGlyphs %s checkedStems %s reusedStems %s artificialStems %s rejectedStems %s allocatorDelta %s sigMutations 0 relationMutations 0 linkerFlagMutations 0\n' \
    "$page_file" "$expected_systems" "$transactions" "$new_glyphs" "$reused_glyphs" \
    "$reinserted_glyphs" "$checked_stems" "$reused_stems" "$artificial_stems" \
    "$rejected_stems" "$allocator_delta" >> "$probe_output"

schema_count=$(awk '$0 == "# schema: stems-beam-create-stem-v1" { count++ } END { print count + 0 }' \
    "$probe_output")
if [ "$schema_count" -ne 1 ]; then
    echo "beam-createStem schema header is missing or duplicated" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeamcreatestempage / { pages++ }
    /^stemsbeamcreatestembaseline / { baselines++ }
    /^stemsbeamcreatestemfrontier / { frontiers++ }
    /^stemsbeamcreatestemexpand / { expands++ }
    /^stemsbeamcreatestemlookup / { lookups++ }
    /^stemsbeamcreatestemresult / { results++ }
    /^stemsbeamcreatestemdelta / { deltas++ }
    /^stemsbeamcreatestemguard / { guards++ }
    /^stemsbeamcreatestemsummary / { summaries++ }
    /^stemsbeamcreatestempagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n",
                pages, baselines, frontiers, expands, lookups, results, deltas,
                guards, summaries, pageSummaries
    }
' "$probe_output")
expected_row_counts="1:$expected_systems:$expected_systems:$expected_systems:$expected_systems:$expected_systems:$expected_systems:$expected_systems:$expected_systems:1"
if [ "$row_counts" != "$expected_row_counts" ]; then
    echo "unexpected $page_key beam-createStem row counts" >&2
    echo "observed: $row_counts" >&2
    exit 1
fi

certificate_errors=$(awk -v systems="$expected_systems" '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    /^stemsbeamcreatestembaseline / {
        fields(3)
        s = f["system"] + 0
        baseline[s]++
        active[s] = f["glyphActive"]
        originals[s] = f["glyphOriginals"]
        activeHash[s] = f["glyphActiveHash"]
        originalsHash[s] = f["glyphOriginalsHash"]
        stemsHash[s] = f["systemStemsHash"]
        stemCount[s] = f["systemStems"]
        mode[s] = f["executionMode"]
    }
    /^stemsbeamcreatestemlookup / {
        fields(3)
        s = f["system"] + 0
        lookup[s]++
        if (f["certificate"] != "ExhaustiveGlyphEqualsScan") bad++
        candidate[s] = f["candidate"]
        baselineUnion[s] = f["baselineUnionSize"]
        lookupKind[s] = f["lookup"]
        lookupAlias[s] = f["presentAlias"]
        lookupId[s] = f["presentId"]
        lookupActive[s] = f["presentActive"]
        systemStemLookup[s] = f["systemStemLookup"]
        systemStemInterId[s] = f["systemStemInterId"]
        if (f["aliasOrder"] != "JavaGlyphId" ||
                f["baselineUnionSize"] !~ /^[1-9][0-9]*$/) bad++
        if (f["candidateBounds"] !~ /^-?[0-9]+:-?[0-9]+:[1-9][0-9]*:[1-9][0-9]*$/ ||
                f["candidateWeight"] !~ /^[1-9][0-9]*$/ ||
                f["candidateRunTable"] == "-") bad++
        if (f["scannedActive"] != active[s] || f["scannedOriginals"] != originals[s] ||
                f["activeHash"] != activeHash[s] || f["originalsHash"] != originalsHash[s] ||
                f["systemStemsHash"] != stemsHash[s]) bad++
        if (f["activeEqualMatches"] !~ /^[01]$/ ||
                f["originalEqualMatches"] !~ /^[01]$/) bad++
        if (f["lookup"] == "Absent") {
            if (f["originalEqualMatches"] != 0 || f["activeEqualMatches"] != 0 ||
                    f["presentAlias"] != "-" ||
                    f["presentId"] != "-" || f["presentActive"] != "-" ||
                    f["presentGlyph"] != "-") bad++
        } else if (f["lookup"] == "Present") {
            if (f["originalEqualMatches"] != 1 || f["presentAlias"] !~ /^glyph:[0-9]+$/ ||
                    f["presentId"] !~ /^[1-9][0-9]*$/ || f["presentActive"] !~ /^(true|false)$/ ||
                    f["presentGlyph"] == "-") bad++
            if (f["presentAlias"] != "glyph:" f["presentId"] ||
                    f["presentGlyph"] != f["candidate"]) bad++
            if ((f["presentActive"] == "true" && f["activeEqualMatches"] != 1) ||
                    (f["presentActive"] == "false" && f["activeEqualMatches"] != 0)) bad++
        } else bad++
        if (f["systemStemCertificate"] != "ExhaustiveSystemStemEqualsScan" ||
                f["scannedSystemStems"] != stemCount[s] ||
                f["systemStemEqualMatches"] !~ /^[01]$/) bad++
        if (f["systemStemLookup"] != "Absent" || f["systemStemEqualMatches"] != 0 ||
                f["systemStemInterId"] != "-" || f["systemStemGrade"] != "-") bad++
    }
    /^stemsbeamcreatestemresult / {
        fields(3)
        s = f["system"] + 0
        if (f["candidate"] != candidate[s]) bad++
        if (index(f["candidateComponents"], ",") > 0) {
            if (f["candidateObjectIdBefore"] != 0) bad++
        } else {
            split(f["candidateComponents"], componentParts, ":")
            if (f["candidateObjectIdBefore"] != componentParts[2]) bad++
        }
        if (lookupKind[s] == "Present") {
            if (f["canonicalGlyphIdBefore"] != lookupId[s]) bad++
        } else if (f["canonicalGlyphIdBefore"] != "-") bad++
        if (f["returnedStemInterId"] !~ /^(-|[0-9]+)$/) bad++
        if (f["disposition"] == "Rejected") {
            if (f["returnedStemInterId"] != "-" || f["stemGrade"] != "-" ||
                    f["stemMedian"] != "-" || f["stemMeanThickness"] != "-" ||
                    f["stemBounds"] != "-" || f["stemAbnormal"] != "-" ||
                    f["stemSigAttached"] != "-") bad++
        } else if (f["disposition"] == "Reused") {
            if (systemStemLookup[s] != "Present" ||
                    f["returnedStemInterId"] != systemStemInterId[s] ||
                    f["stemGrade"] == "-") bad++
        } else if (f["returnedStemInterId"] != 0 || f["stemGrade"] == "-") bad++
        if (f["disposition"] != "Rejected") {
            if (split(f["stemMedian"], medianParts, ":") != 4 ||
                    f["stemMeanThickness"] == "-" ||
                    f["stemBounds"] !~ /^-?[0-9]+:-?[0-9]+:[1-9][0-9]*:[1-9][0-9]*$/ ||
                    f["stemAbnormal"] !~ /^(true|false)$/ ||
                    f["stemSigAttached"] !~ /^(true|false)$/) bad++
            for (medianPart = 1; medianPart <= 4; medianPart++) {
                if (index(medianParts[medianPart], "/") == 0) bad++
            }
            if (index(f["stemMeanThickness"], "/") == 0) bad++
        }
        if (f["disposition"] == "CreatedChecked" ||
                f["disposition"] == "CreatedArtificial") {
            if (f["returnedStemInterId"] != 0 || f["stemAbnormal"] != "false" ||
                    f["stemSigAttached"] != "false") bad++
        }
        if (f["registeredAlias"] != "glyph:" f["registeredGlyphId"] ||
                f["postAliasOrder"] != "JavaGlyphId") bad++
        expectedUnion = baselineUnion[s] + ((f["registration"] == "New") ? 1 : 0)
        if (f["postUnionSize"] != expectedUnion) bad++
        if (lookupKind[s] == "Present" &&
                (f["registeredAlias"] != lookupAlias[s] ||
                 f["registeredGlyphId"] != lookupId[s] ||
                 (lookupActive[s] == "true" && f["registration"] != "ReuseActive") ||
                 (lookupActive[s] == "false" && f["registration"] != "ReinsertOriginal"))) bad++
        if (lookupKind[s] == "Absent" && f["registration"] != "New") bad++
        resultAlias[s] = f["registeredAlias"]
    }
    /^stemsbeamcreatestemdelta / {
        fields(3)
        s = f["system"] + 0
        if (f["registeredAlias"] != resultAlias[s]) bad++
    }
    /^stemsbeamcreatestemguard / {
        fields(3)
        guard[f["system"] + 0]++
        if (f["lineDeltaRetained"] != "true" || f["interIndexUnchanged"] != "true" ||
                f["sigUnchanged"] != "true" || f["relationsUnchanged"] != "true" ||
                f["linkerFlagsUnchanged"] != "true" ||
                f["stopBeforeVReuse"] != "true" ||
                f["stopBeforeBeamStemCheck"] != "true") bad++
    }
    END {
        for (s = 1; s <= systems; s++) {
            if (baseline[s] != 1 || lookup[s] != 1 || guard[s] != 1) bad++
            expectedMode = (s == 1) ? "sheet-first" : "isolated-system-frontier"
            if (mode[s] != expectedMode) bad++
        }
        print bad + 0
    }
' "$probe_output")
if [ "$certificate_errors" -ne 0 ]; then
    echo "$certificate_errors beam-createStem certificate/guard errors" >&2
    exit 1
fi

body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
body_lines=$(wc -l < "$probe_output" | tr -d ' ')
body_bytes=$(wc -c < "$probe_output" | tr -d ' ')
probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
cat "$probe_output"
printf 'stemsbeamcreatestemcorpussummary schema stems-beam-create-stem-v1 mode %s pages 1 pageRefs %s#1 rowCounts %s probeSourceSha256 %s runnerSourceSha256 %s schedulerFixtureSha256 %s expandFixtureSha256 %s emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshJvmPerPage false freshJvmPerSystem true javaProcessesPerPage %s freshSheetPerSystem true runnerJavaProcessesReaped true backgroundJavaProcessesStarted 0\n' \
    "$page_key" "$page_file" "$row_counts" "$probe_source_sha256" "$runner_source_sha256" \
    "$(shasum -a 256 "$scheduler_fixture" | awk '{print $1}')" \
    "$(shasum -a 256 "$expand_fixture" | awk '{print $1}')" \
    "$body_sha256" "$body_lines" "$body_bytes" "$expected_systems"
