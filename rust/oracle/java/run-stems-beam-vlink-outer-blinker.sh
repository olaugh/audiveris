#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Boundary-18 oracle runner (fast evidence per rust/PORTING.md): single fresh-JVM pass
# per system. Before the fixture is written, the boundary-17 rows the probe re-emits are
# required to match the frozen head-links fixture byte-for-byte, which pins every
# predecessor without re-freezing them.
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
if [ ! -x "$JAVA_HOME/bin/java" ] || [ ! -x "$JAVA_HOME/bin/javac" ] || \
        [ ! -f "$JAVA_HOME/release" ]; then
    echo "JAVA_HOME does not contain the frozen Temurin JDK" >&2
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
        [ "$(release_field JAVA_VERSION)" != "25.0.3" ] || \
        [ "$(release_field OS_NAME)" != "Darwin" ] || \
        [ "$(release_field OS_ARCH)" != "aarch64" ] || \
        [ "$(release_field JVM_VARIANT)" != "Hotspot" ] || \
        [ "$(release_field IMAGE_TYPE)" != "JDK" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$script_dir/StemsBeamVLinkOuterBLinkerProbe.java"

case "$page_key" in
    chula) page_file=chula.png; expected_systems=3 ;;
    allegretto) page_file=allegretto.png; expected_systems=3 ;;
    batuque) page_file=batuque.png; expected_systems=3 ;;
    carmen) page_file=carmen.png; expected_systems=5 ;;
    cucaracha) page_file=cucaracha.png; expected_systems=3 ;;
    hove) page_file=hove.png; expected_systems=5 ;;
    zizi) page_file=zizi.png; expected_systems=2 ;;
    BachInvention5) page_file=BachInvention5.jpg; expected_systems=6 ;;
    *) echo "unknown outer-blinker page key: $page_key" >&2; exit 2 ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
create_stem_fixture="$repo_root/rust/oracle/stems-beam-create-stem-$page_key.txt"
reuse_check_fixture="$repo_root/rust/oracle/stems-beam-vlink-reuse-check-$page_key.txt"
base_apply_fixture="$repo_root/rust/oracle/stems-beam-vlink-base-apply-$page_key.txt"
b_linker_flag_fixture="$repo_root/rust/oracle/stems-beam-vlink-b-linker-flag-$page_key.txt"
sibling_links_fixture="$repo_root/rust/oracle/stems-beam-vlink-sibling-links-$page_key.txt"
head_links_fixture="$repo_root/rust/oracle/stems-beam-vlink-head-links-$page_key.txt"
for required in "$probe_cp_file" "$probe_source" "$scheduler_fixture" "$expand_fixture" \
        "$create_stem_fixture" "$reuse_check_fixture" "$base_apply_fixture" \
        "$b_linker_flag_fixture" "$sibling_links_fixture" "$head_links_fixture"; do
    if [ ! -f "$required" ]; then
        echo "missing input: $required" >&2
        exit 2
    fi
done

probe_cp=$(cat "$probe_cp_file")
probe_classes=$(mktemp -d /private/tmp/stems-beam-vlink-outer-blinker-classes.XXXXXX)
trap 'rm -rf "$probe_classes"' EXIT
"$JAVA_HOME/bin/javac" -Xlint:all,-path -cp "$probe_cp" -d "$probe_classes" "$probe_source"

pass_out=$(mktemp /private/tmp/stems-beam-vlink-outer-blinker-pass.XXXXXX)
system_id=1
while [ "$system_id" -le "$expected_systems" ]; do
    (
        cd "$repo_root/app"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            "$JAVA_HOME/bin/java" \
            -XX:+UnlockExperimentalVMOptions \
            -XX:+UseEpsilonGC \
            -Xmx48g \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsBeamVLinkOuterBLinkerProbe \
            --system "$system_id" \
            "$repo_root/data/examples/$page_file:1" \
            "$scheduler_fixture" "$expand_fixture" "$create_stem_fixture" \
            "$reuse_check_fixture" "$base_apply_fixture" "$b_linker_flag_fixture" \
            "$sibling_links_fixture" "$head_links_fixture"
    ) >> "$pass_out"
    system_id=$((system_id + 1))
done

# Predecessor pin: the re-emitted boundary-17 rows must equal the frozen fixture exactly.
# Two sanctioned differences only: the pagesummary/corpussummary rows are composed by the
# boundary-17 runner (process counts, runner hashes), never printed by the probe; and one
# isolated-evidence row prints the probe's own synthetic exception class name, which this
# probe's rename changes, so that single token is normalized before comparison.
b17_new=$(mktemp /private/tmp/stems-beam-vlink-outer-blinker-b17.XXXXXX)
b17_frozen=$(mktemp /private/tmp/stems-beam-vlink-outer-blinker-b17f.XXXXXX)
grep '^stemsbeamvlinkheadlinks' "$pass_out" \
    | sed 's/StemsBeamVLinkOuterBLinkerProbe\$/StemsBeamVLinkHeadLinksProbe$/g' > "$b17_new"
grep '^stemsbeamvlinkheadlinks' "$head_links_fixture" \
    | grep -v '^stemsbeamvlinkheadlinkspagesummary ' \
    | grep -v '^stemsbeamvlinkheadlinkscorpussummary ' > "$b17_frozen"
if ! cmp -s "$b17_new" "$b17_frozen"; then
    echo "re-emitted boundary-17 rows drifted from the frozen head-links fixture" >&2
    diff "$b17_frozen" "$b17_new" | head -10 >&2
    exit 1
fi
rm -f "$b17_new" "$b17_frozen"

fixture_out="$repo_root/rust/oracle/stems-beam-vlink-outer-blinker-$page_key.txt"
{
    echo "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink outer B-linker oracle."
    echo "# schema: stems-beam-vlink-outer-blinker-v1"
    echo "# Fast evidence per rust/PORTING.md: one fresh-JVM pass per system; before this file"
    echo "# was written, the boundary-17 rows the probe re-emits were required to match the"
    echo "# frozen head-links fixture byte-for-byte, pinning every predecessor."
    echo "# Each system executes one real transaction: after VLinker.link returns true, the"
    echo "# caller BLinker.link performs one idempotent setLinked(true) on the outer BLinker."
    echo "# Stop is AssignedOuterBLinkerBeforeNextVIteration."
    grep '^stemsbeamvlinkouterblinker' "$pass_out"
} > "$fixture_out"
rm -f "$pass_out"
row_count=$(grep -c '^stemsbeamvlinkouterblinker' "$fixture_out")
expected_rows=$((1 + 2 * expected_systems))
if [ "$row_count" -ne "$expected_rows" ]; then
    echo "expected $expected_rows outer-blinker rows, got $row_count" >&2
    exit 1
fi
echo "wrote $fixture_out ($row_count rows)"
