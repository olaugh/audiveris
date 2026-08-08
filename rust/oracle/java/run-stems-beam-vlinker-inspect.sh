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
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkerInspectProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlinker-inspect.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-vlinker-inspect-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-vlinker-inspect-output.XXXXXX)
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
            -Xmx4g \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsBeamVLinkerInspectProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx4g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamVLinkerInspectProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

summary_counts=$(awk '
    /^stemsbeamvinspectpagesummary / {
        pages++
        for (key in fields) delete fields[key]
        for (key in seen) delete seen[key]
        for (i = 3; i <= NF; i += 2) {
            key = $i
            if (seen[key]++) duplicates++
            fields[key] = $(i + 1)
        }
        required = "systems beams smallBeams heads smallHeads corners initialBs initialVs " \
                "beamStarts bVisits anchorSkips inspectedVs siblingScans siblingHits " \
                "beamCandidates eligibleBeams findCalls findCandidates bReuses anchorReuses " \
                "anchorsCreated headLookups headScans headAreaHits headCompetingPasses " \
                "headCompetingRemovals headCompetitorDrops smallHeadCandidates sizeDrops " \
                "headNearDrops cornerChecks cornerInside voidCornerDrops cAccepted resultBs " \
                "resultCs finalBs seedSnapshots builderChecks hash"
        count = split(required, names, " ")
        for (i = 1; i <= count; i++) {
            if (!(names[i] in fields)) missing++
        }
        systems += fields["systems"]
        beams += fields["beams"]
        smallBeams += fields["smallBeams"]
        heads += fields["heads"]
        smallHeads += fields["smallHeads"]
        corners += fields["corners"]
        initialBs += fields["initialBs"]
        initialVs += fields["initialVs"]
        beamStarts += fields["beamStarts"]
        bVisits += fields["bVisits"]
        anchorSkips += fields["anchorSkips"]
        inspectedVs += fields["inspectedVs"]
        siblingScans += fields["siblingScans"]
        siblingHits += fields["siblingHits"]
        beamCandidates += fields["beamCandidates"]
        eligibleBeams += fields["eligibleBeams"]
        findCalls += fields["findCalls"]
        findCandidates += fields["findCandidates"]
        bReuses += fields["bReuses"]
        anchorReuses += fields["anchorReuses"]
        anchorsCreated += fields["anchorsCreated"]
        headLookups += fields["headLookups"]
        headScans += fields["headScans"]
        headAreaHits += fields["headAreaHits"]
        headCompetingPasses += fields["headCompetingPasses"]
        headCompetingRemovals += fields["headCompetingRemovals"]
        headCompetitorDrops += fields["headCompetitorDrops"]
        smallHeadCandidates += fields["smallHeadCandidates"]
        sizeDrops += fields["sizeDrops"]
        headNearDrops += fields["headNearDrops"]
        cornerChecks += fields["cornerChecks"]
        cornerInside += fields["cornerInside"]
        voidCornerDrops += fields["voidCornerDrops"]
        cAccepted += fields["cAccepted"]
        resultBs += fields["resultBs"]
        resultCs += fields["resultCs"]
        finalBs += fields["finalBs"]
        seedSnapshots += fields["seedSnapshots"]
        builderChecks += fields["builderChecks"]
    }
    END {
        printf "%d", pages
        printf ":%d:%d:%d:%d:%d:%d", systems, beams, smallBeams, heads, smallHeads, corners
        printf ":%d:%d:%d:%d:%d:%d", initialBs, initialVs, beamStarts, bVisits, anchorSkips, \
                inspectedVs
        printf ":%d:%d:%d:%d:%d:%d", siblingScans, siblingHits, beamCandidates, \
                eligibleBeams, findCalls, findCandidates
        printf ":%d:%d:%d:%d", bReuses, anchorReuses, anchorsCreated, headLookups
        printf ":%d:%d:%d:%d:%d:%d", headScans, headAreaHits, headCompetingPasses, \
                headCompetingRemovals, headCompetitorDrops, smallHeadCandidates
        printf ":%d:%d:%d:%d:%d:%d", sizeDrops, headNearDrops, cornerChecks, cornerInside, \
                voidCornerDrops, cAccepted
        printf ":%d:%d:%d:%d:%d:%d:%d\n", resultBs, resultCs, finalBs, seedSnapshots, \
                builderChecks, missing, duplicates
    }
' "$probe_output")

IFS=: read -r pages systems beams small_beams heads small_heads corners initial_bs initial_vs \
    beam_starts b_visits anchor_skips inspected_vs sibling_scans sibling_hits beam_candidates \
    eligible_beams find_calls find_candidates b_reuses anchor_reuses anchors_created \
    head_lookups head_scans head_area_hits head_competing_passes head_competing_removals \
    head_competitor_drops small_head_candidates size_drops head_near_drops corner_checks \
    corner_inside void_corner_drops c_accepted result_bs result_cs final_bs seed_snapshots \
    builder_checks missing_fields duplicate_fields <<EOF
$summary_counts
EOF

if [ "$missing_fields:$duplicate_fields" != "0:0" ]; then
    echo "STEMS BeamVLinker-inspect page summaries have $missing_fields missing and $duplicate_fields duplicate fields" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeamvinspectpage / { pages++ }
    /^stemsbeamvinspectsystem / { systems++ }
    /^stemsbeamvinspectbeamorder / { beams++ }
    /^stemsbeamvinspecthead / { heads++ }
    /^stemsbeamvinspectcorner / { corners++ }
    /^stemsbeamvinspectinitialb / { initialBs++ }
    /^stemsbeamvinspectbeamstart / { beamStarts++ }
    /^stemsbeamvinspectbvisit / { bVisits++ }
    /^stemsbeamvinspectv / { Vs++ }
    /^stemsbeamvinspectsibling / { siblingScans++ }
    /^stemsbeamvinspectbeamcandidate / { beamCandidates++ }
    /^stemsbeamvinspectfind / { finds++ }
    /^stemsbeamvinspectfindcandidate / { findCandidates++ }
    /^stemsbeamvinspectfindresult / { findResults++ }
    /^stemsbeamvinspectheadlookup / { headLookups++ }
    /^stemsbeamvinspectheadscan / { headScans++ }
    /^stemsbeamvinspectheadcompeting / { headCompeting++ }
    /^stemsbeamvinspectheadcandidate / { headCandidates++ }
    /^stemsbeamvinspectcornercheck / { cornerChecks++ }
    /^stemsbeamvinspectresult / { results++ }
    /^stemsbeamvinspectbeamend / { beamEnds++ }
    /^stemsbeamvinspectfinalb / { finalBs++ }
    /^stemsbeamvinspectsystemsummary / { systemSummaries++ }
    /^stemsbeamvinspectpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", pages, systems, beams, heads, \
                corners, initialBs, beamStarts, bVisits, Vs, siblingScans, beamCandidates, finds
        printf ":%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", findCandidates, findResults, \
                headLookups, headScans, headCompeting, headCandidates, cornerChecks, results, \
                beamEnds, finalBs, systemSummaries, pageSummaries
    }
' "$probe_output")
IFS=: read -r row_pages row_systems row_beams row_heads row_corners row_initial_bs \
    row_beam_starts row_b_visits row_vs row_sibling_scans row_beam_candidates row_finds \
    row_find_candidates row_find_results row_head_lookups row_head_scans row_head_competing \
    row_head_candidates row_corner_checks row_results row_beam_ends row_final_bs \
    row_system_summaries row_page_summaries <<EOF
$row_counts
EOF

if [ "$pages:$systems:$beams:$heads:$corners:$initial_bs:$initial_vs" != \
     "8:30:803:3521:14084:2116:2417" ]; then
    echo "unexpected STEMS BeamVLinker-inspect corpus shape" >&2
    echo "observed: $pages:$systems:$beams:$heads:$corners:$initial_bs:$initial_vs" >&2
    exit 1
fi
if [ "$row_pages:$row_systems:$row_beams:$row_heads:$row_corners:$row_initial_bs:$row_beam_starts:$row_b_visits:$row_vs:$row_sibling_scans:$row_beam_candidates:$row_finds:$row_find_candidates:$row_find_results:$row_head_lookups:$row_head_scans:$row_head_competing:$row_head_candidates:$row_corner_checks:$row_results:$row_beam_ends:$row_final_bs:$row_system_summaries:$row_page_summaries" != \
     "$pages:$systems:$beams:$heads:$corners:$initial_bs:$beam_starts:$b_visits:$inspected_vs:$sibling_scans:$beam_candidates:$find_calls:$find_candidates:$find_calls:$head_lookups:$head_scans:$head_competing_passes:$head_area_hits:$corner_checks:$inspected_vs:$beams:$final_bs:$systems:$pages" ]; then
    echo "STEMS BeamVLinker-inspect detail row counts disagree with page summaries" >&2
    exit 1
fi

if [ "$corners" -ne $((4 * heads)) ] \
        || [ "$beam_starts" -ne "$beams" ] \
        || [ "$b_visits" -ne $((initial_bs + anchor_skips)) ] \
        || [ "$anchor_skips" -gt "$anchors_created" ] \
        || [ "$inspected_vs" -ne "$initial_vs" ] \
        || [ "$head_lookups" -ne "$inspected_vs" ] \
        || [ "$seed_snapshots" -ne "$inspected_vs" ] \
        || [ "$sibling_hits" -ne "$beam_candidates" ] \
        || [ "$sibling_hits" -ne "$head_competing_passes" ] \
        || [ "$eligible_beams" -ne "$find_calls" ] \
        || [ "$eligible_beams" -ne "$result_bs" ] \
        || [ "$find_calls" -ne $((b_reuses + anchors_created)) ] \
        || [ "$anchor_reuses" -gt "$b_reuses" ] \
        || [ "$final_bs" -ne $((initial_bs + anchors_created)) ] \
        || [ "$head_competing_removals" -ne "$head_competitor_drops" ] \
        || [ "$small_beams:$small_heads:$small_head_candidates:$size_drops" != "0:0:0:0" ] \
        || [ "$corner_checks" -ne $((2 * (head_area_hits - head_competitor_drops - size_drops - head_near_drops))) ] \
        || [ "$corner_inside" -ne $((void_corner_drops + c_accepted)) ] \
        || [ "$c_accepted" -ne "$result_cs" ] \
        || [ "$builder_checks" -ne $((3 * initial_vs + 2 * corners)) ]; then
    echo "STEMS BeamVLinker-inspect totals violate coverage invariants" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsbeamvinspectsystemsummary / {
        for (key in fields) delete fields[key]
        for (key in seen) delete seen[key]
        duplicate = 0
        for (i = 5; i <= NF; i += 2) {
            key = $i
            if (seen[key]++) duplicate = 1
            fields[key] = $(i + 1)
        }
        required = "systems beams smallBeams heads smallHeads corners initialBs initialVs " \
                "beamStarts bVisits anchorSkips inspectedVs siblingScans siblingHits " \
                "beamCandidates eligibleBeams findCalls findCandidates bReuses anchorReuses " \
                "anchorsCreated headLookups headScans headAreaHits headCompetingPasses " \
                "headCompetingRemovals headCompetitorDrops smallHeadCandidates sizeDrops " \
                "headNearDrops cornerChecks cornerInside voidCornerDrops cAccepted resultBs " \
                "resultCs finalBs seedSnapshots builderChecks hash"
        count = split(required, names, " ")
        missing = 0
        for (i = 1; i <= count; i++) if (!(names[i] in fields)) missing = 1
        bad = duplicate || missing || fields["systems"] != 1
        if (fields["corners"] != 4 * fields["heads"]) bad = 1
        if (fields["beamStarts"] != fields["beams"]) bad = 1
        if (fields["bVisits"] != fields["initialBs"] + fields["anchorSkips"]) bad = 1
        if (fields["inspectedVs"] != fields["initialVs"]) bad = 1
        if (fields["headLookups"] != fields["inspectedVs"]) bad = 1
        if (fields["seedSnapshots"] != fields["inspectedVs"]) bad = 1
        if (fields["siblingHits"] != fields["beamCandidates"]) bad = 1
        if (fields["siblingHits"] != fields["headCompetingPasses"]) bad = 1
        if (fields["eligibleBeams"] != fields["findCalls"]) bad = 1
        if (fields["eligibleBeams"] != fields["resultBs"]) bad = 1
        if (fields["findCalls"] != fields["bReuses"] + fields["anchorsCreated"]) bad = 1
        if (fields["finalBs"] != fields["initialBs"] + fields["anchorsCreated"]) bad = 1
        if (fields["headCompetingRemovals"] != fields["headCompetitorDrops"]) bad = 1
        if (fields["smallBeams"] + fields["smallHeads"] + fields["smallHeadCandidates"] + fields["sizeDrops"] != 0) bad = 1
        if (fields["cornerChecks"] != 2 * (fields["headAreaHits"] - fields["headCompetitorDrops"] - fields["sizeDrops"] - fields["headNearDrops"])) bad = 1
        if (fields["cornerInside"] != fields["voidCornerDrops"] + fields["cAccepted"]) bad = 1
        if (fields["cAccepted"] != fields["resultCs"]) bad = 1
        if (fields["builderChecks"] != 3 * fields["initialVs"] + 2 * fields["corners"]) bad = 1
        if (bad) badSystems++
    }
    END { print badSystems + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries violate BeamVLinker-inspect invariants" >&2
    exit 1
fi

observed_totals="$beams:$small_beams:$heads:$small_heads:$corners:$initial_bs:$initial_vs:$beam_starts:$b_visits:$anchor_skips:$inspected_vs:$sibling_scans:$sibling_hits:$beam_candidates:$eligible_beams:$find_calls:$find_candidates:$b_reuses:$anchor_reuses:$anchors_created:$head_lookups:$head_scans:$head_area_hits:$head_competing_passes:$head_competing_removals:$head_competitor_drops:$small_head_candidates:$size_drops:$head_near_drops:$corner_checks:$corner_inside:$void_corner_drops:$c_accepted:$result_bs:$result_cs:$final_bs:$seed_snapshots:$builder_checks"
expected_totals="803:0:3521:0:14084:2116:2417:803:2145:29:2417:4960:4514:4514:1617:1617:5354:1472:215:145:2417:158886:5739:4514:0:0:0:0:46:11386:5590:531:5059:1617:5059:2261:2417:35419"
if [ "$observed_totals" != "$expected_totals" ]; then
    echo "unexpected STEMS BeamVLinker-inspect corpus totals" >&2
    echo "expected: $expected_totals" >&2
    echo "observed: $observed_totals" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
expected_probe_source_sha256=39ed0694f7c31593f157b5f250f8bfa4f006984e3b491a877903d64d810edd7b
expected_body_sha256=470827ebc19065890c41c10016511e77eeefc851823bb8587f7537c7e7db23cf
if [ "$probe_source_sha256:$body_sha256" != \
     "$expected_probe_source_sha256:$expected_body_sha256" ]; then
    echo "unexpected STEMS BeamVLinker-inspect source/body hash" >&2
    echo "observed: $probe_source_sha256:$body_sha256" >&2
    exit 1
fi
cat "$probe_output"
printf 'stemsbeamvinspectcorpussummary pages %d systems %d beams %d smallBeams %d heads %d smallHeads %d corners %d initialBs %d initialVs %d beamStarts %d bVisits %d anchorSkips %d inspectedVs %d siblingScans %d siblingHits %d beamCandidates %d eligibleBeams %d findCalls %d findCandidates %d bReuses %d anchorReuses %d anchorsCreated %d headLookups %d headScans %d headAreaHits %d headCompetingPasses %d headCompetingRemovals %d headCompetitorDrops %d smallHeadCandidates %d sizeDrops %d headNearDrops %d cornerChecks %d cornerInside %d voidCornerDrops %d cAccepted %d resultBs %d resultCs %d finalBs %d seedSnapshots %d builderChecks %d probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$beams" "$small_beams" "$heads" "$small_heads" "$corners" \
    "$initial_bs" "$initial_vs" "$beam_starts" "$b_visits" "$anchor_skips" \
    "$inspected_vs" "$sibling_scans" "$sibling_hits" "$beam_candidates" "$eligible_beams" \
    "$find_calls" "$find_candidates" "$b_reuses" "$anchor_reuses" "$anchors_created" \
    "$head_lookups" "$head_scans" "$head_area_hits" "$head_competing_passes" \
    "$head_competing_removals" "$head_competitor_drops" "$small_head_candidates" \
    "$size_drops" "$head_near_drops" "$corner_checks" "$corner_inside" \
    "$void_corner_drops" "$c_accepted" "$result_bs" "$result_cs" "$final_bs" \
    "$seed_snapshots" "$builder_checks" "$probe_source_sha256" "$runner_source_sha256" \
    "$body_sha256"
