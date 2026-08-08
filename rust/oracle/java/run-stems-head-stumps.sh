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
probe_source="$repo_root/rust/oracle/java/StemsHeadStumpProbe.java"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-head-stumps-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-head-stumps-output.XXXXXX)
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
            org.audiveris.omr.rustport.StemsHeadStumpProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx4g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsHeadStumpProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

row_counts=$(awk '
    /^stemsheadstumppagesummary / { pages++ }
    /^stemsheadstumpsystemsummary / { systems++ }
    /^stemsheadstumpbeam / { beams++ }
    /^stemsheadstumpbeamreg / { beamRegs++ }
    /^stemsheadstumphead / { heads++ }
    /^stemsheadstumpheadreg / { headRegs++ }
    /^stemsheadstumpcorner / { corners++ }
    /^stemsheadstumpsection / { sections++ }
    /^stemsheadstumpstep / { steps++ }
    /^stemsheadstumpsubsection / { subsections++ }
    /^stemsheadstumpbuild / {
        builds++
        for (i = 1; i <= NF; i++) {
            if (($i == "candidate") && ($(i + 1) == "none")) emptyBuilds++
            else if ($i == "standsOut") {
                candidates++
                if ($(i + 1) == "true") accepted++
                else rejected++
            } else if ($i == "registration") {
                if ($(i + 1) ~ /^new:/) newBuilds++
                else if ($(i + 1) ~ /^reuse:/) reusedBuilds++
            }
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", pages,
            systems, beams, beamRegs, heads, headRegs, corners, sections, steps,
            subsections, builds, emptyBuilds, candidates, accepted, rejected,
            newBuilds + reusedBuilds
    }
' "$probe_output")

summary_counts=$(awk '
    /^stemsheadstumppagesummary / {
        for (i = 1; i <= NF; i++) {
            if ($i == "beams") beams += $(i + 1)
            else if ($i == "tremolos") tremolos += $(i + 1)
            else if ($i == "beamRegs") beamRegs += $(i + 1)
            else if ($i == "heads") heads += $(i + 1)
            else if ($i == "corners") corners += $(i + 1)
            else if ($i == "seeds") seeds += $(i + 1)
            else if ($i == "fallbacks") fallbacks += $(i + 1)
            else if ($i == "emptyBuilds") emptyBuilds += $(i + 1)
            else if ($i == "buildCandidates") candidates += $(i + 1)
            else if ($i == "acceptedBuilds") accepted += $(i + 1)
            else if ($i == "rejectedBuilds") rejected += $(i + 1)
            else if ($i == "headRegs") headRegs += $(i + 1)
            else if ($i == "newBuilds") newBuilds += $(i + 1)
            else if ($i == "reusedBuilds") reusedBuilds += $(i + 1)
            else if ($i == "sections") sections += $(i + 1)
            else if ($i == "steps") steps += $(i + 1)
            else if ($i == "subsections") subsections += $(i + 1)
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", beams,
            tremolos, beamRegs, heads, corners, seeds, fallbacks, emptyBuilds,
            candidates, accepted, rejected, headRegs, newBuilds, reusedBuilds,
            sections, steps, subsections
    }
' "$probe_output")

IFS=: read -r pages systems row_beams row_beam_regs row_heads row_head_regs row_corners \
    row_sections row_steps row_subsections row_builds row_empty row_candidates row_accepted \
    row_rejected row_registration_events <<EOF
$row_counts
EOF
IFS=: read -r beams tremolos beam_regs heads corners seeds fallbacks empty_builds candidates \
    accepted rejected head_regs new_builds reused_builds sections steps subsections <<EOF
$summary_counts
EOF

if [ "$pages:$systems:$heads:$corners" != "8:30:3521:14084" ]; then
    echo "unexpected STEMS head-stump corpus shape $pages:$systems:$heads:$corners" >&2
    exit 1
fi

if [ "$row_beams:$row_beam_regs:$row_heads:$row_head_regs:$row_corners:$row_sections:$row_steps:$row_subsections:$row_empty:$row_candidates:$row_accepted:$row_rejected:$row_registration_events" != \
     "$beams:$beam_regs:$heads:$head_regs:$corners:$sections:$steps:$subsections:$empty_builds:$candidates:$accepted:$rejected:$((new_builds + reused_builds))" ]; then
    echo "STEMS head-stump row counts disagree with page summaries" >&2
    exit 1
fi

if [ "$row_builds" -ne "$fallbacks" ] \
        || [ $((seeds + fallbacks)) -ne "$corners" ] \
        || [ $((empty_builds + candidates)) -ne "$fallbacks" ] \
        || [ $((accepted + rejected)) -ne "$candidates" ] \
        || [ $((new_builds + reused_builds)) -ne "$candidates" ]; then
    echo "STEMS head-stump outcome totals violate coverage invariants" >&2
    exit 1
fi

if [ "$beams:$tremolos:$beam_regs:$heads:$corners:$seeds:$fallbacks:$empty_builds:$candidates:$accepted:$rejected:$head_regs:$new_builds:$reused_builds:$sections:$steps:$subsections" != \
     "803:0:5:3521:14084:4182:9902:969:8933:758:8175:5591:5591:3342:18398:18398:3660" ]; then
    echo "unexpected STEMS head-stump corpus totals" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsheadstumpsystemsummary / {
        heads = corners = seeds = fallbacks = emptyBuilds = candidates = -1
        accepted = rejected = newBuilds = reusedBuilds = -1
        for (i = 1; i <= NF; i++) {
            if ($i == "heads") heads = $(i + 1)
            else if ($i == "corners") corners = $(i + 1)
            else if ($i == "seeds") seeds = $(i + 1)
            else if ($i == "fallbacks") fallbacks = $(i + 1)
            else if ($i == "emptyBuilds") emptyBuilds = $(i + 1)
            else if ($i == "buildCandidates") candidates = $(i + 1)
            else if ($i == "acceptedBuilds") accepted = $(i + 1)
            else if ($i == "rejectedBuilds") rejected = $(i + 1)
            else if ($i == "newBuilds") newBuilds = $(i + 1)
            else if ($i == "reusedBuilds") reusedBuilds = $(i + 1)
        }
        if ((corners != 4 * heads) || (corners != seeds + fallbacks) ||
                (fallbacks != emptyBuilds + candidates) ||
                (candidates != accepted + rejected) ||
                (candidates != newBuilds + reusedBuilds)) bad++
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries violate stump invariants" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'stemsheadstumpcorpussummary pages %d systems %d beams %d tremolos %d beamRegs %d heads %d corners %d seeds %d fallbacks %d emptyBuilds %d buildCandidates %d acceptedBuilds %d rejectedBuilds %d headRegs %d newBuilds %d reusedBuilds %d sections %d steps %d subsections %d probeSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$beams" "$tremolos" "$beam_regs" "$heads" "$corners" \
    "$seeds" "$fallbacks" "$empty_builds" "$candidates" "$accepted" "$rejected" \
    "$head_regs" "$new_builds" "$reused_builds" "$sections" "$steps" \
    "$subsections" "$probe_source_sha256" "$body_sha256"
