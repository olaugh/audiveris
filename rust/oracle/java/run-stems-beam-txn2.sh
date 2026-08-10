#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Boundary-20 oracle runner (fast evidence per rust/PORTING.md): single fresh-JVM pass
# per system. The second frontier's transaction executes in fresh-evidence mode; its
# per-family rows are split into per-family txn2 fixture files. Predecessor pins:
# boundary-17/18 head/outer prefixes and the boundary-19 resume rows must match their
# frozen fixtures byte-for-byte, and the probe internally requires every fresh emitter
# to reproduce the frozen first-transaction rows (replay-on-frozen, rust/PORTING.md).
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
probe_source="$script_dir/StemsBeamSecondTransactionProbe.java"

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
outer_blinker_fixture="$repo_root/rust/oracle/stems-beam-vlink-outer-blinker-$page_key.txt"
scheduler_resume_fixture="$repo_root/rust/oracle/stems-beam-scheduler-resume-$page_key.txt"
for required in "$probe_cp_file" "$probe_source" "$scheduler_fixture" "$expand_fixture" \
        "$create_stem_fixture" "$reuse_check_fixture" "$base_apply_fixture" \
        "$b_linker_flag_fixture" "$sibling_links_fixture" "$head_links_fixture" \
        "$outer_blinker_fixture" "$scheduler_resume_fixture"; do
    if [ ! -f "$required" ]; then
        echo "missing input: $required" >&2
        exit 2
    fi
done

probe_cp=$(cat "$probe_cp_file")
probe_classes=$(mktemp -d /private/tmp/stems-beam-txn2-classes.XXXXXX)
trap 'rm -rf "$probe_classes"' EXIT
"$JAVA_HOME/bin/javac" -Xlint:all,-path -cp "$probe_cp" -d "$probe_classes" "$probe_source"

pass_out=$(mktemp /private/tmp/stems-beam-txn2-pass.XXXXXX)
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
            org.audiveris.omr.rustport.StemsBeamSecondTransactionProbe \
            --system "$system_id" \
            "$repo_root/data/examples/$page_file:1" \
            "$scheduler_fixture" "$expand_fixture" "$create_stem_fixture" \
            "$reuse_check_fixture" "$base_apply_fixture" "$b_linker_flag_fixture" \
            "$sibling_links_fixture" "$head_links_fixture" "$outer_blinker_fixture" \
            "$scheduler_resume_fixture"
    ) >> "$pass_out"
    system_id=$((system_id + 1))
done

# Predecessor pins. Per-system JVMs interleave txn2 rows between systems, so txn1
# rows are recovered as the order-preserving subsequence that matches the frozen
# fixture rows exactly; the remainder is the txn2 stream. The match must consume
# every frozen row.
split_family()
{
    family_grep=$1
    frozen_file=$2
    normalize=$3
    txn2_out=$4
    frozen_rows=$(mktemp /private/tmp/stems-beam-txn2-frozen.XXXXXX)
    mine_rows=$(mktemp /private/tmp/stems-beam-txn2-mine.XXXXXX)
    grep "$family_grep" "$frozen_file" \
        | grep -v '^stemsbeamvlinkheadlinkspagesummary ' \
        | grep -v '^stemsbeamvlinkheadlinkscorpussummary ' > "$frozen_rows" || true
    if [ "$normalize" = "b17" ]; then
        grep "$family_grep" "$pass_out" \
            | sed 's/StemsBeamSecondTransactionProbe\$/StemsBeamVLinkHeadLinksProbe$/g' \
            > "$mine_rows"
    else
        grep "$family_grep" "$pass_out" > "$mine_rows" || true
    fi
    awk -v frozen="$frozen_rows" -v txn2="$txn2_out" '
        BEGIN {
            n = 0
            while ((getline line < frozen) > 0) expected[n++] = line
            close(frozen)
            cursor = 0
        }
        {
            if (cursor < n && $0 == expected[cursor]) { cursor++ }
            else { print > txn2 }
        }
        END {
            if (cursor != n) {
                printf "matched only %d of %d frozen rows\n", cursor, n > "/dev/stderr"
                exit 1
            }
        }' "$mine_rows"
    status=$?
    rm -f "$frozen_rows" "$mine_rows"
    if [ "$status" -ne 0 ]; then
        echo "txn1 subsequence drifted from $frozen_file" >&2
        exit 1
    fi
}
head_txn2=$(mktemp /private/tmp/stems-beam-txn2-head.XXXXXX)
outer_txn2=$(mktemp /private/tmp/stems-beam-txn2-outer.XXXXXX)
resume_extra=$(mktemp /private/tmp/stems-beam-txn2-resume.XXXXXX)
split_family '^stemsbeamvlinkheadlinks' "$head_links_fixture" b17 "$head_txn2"
split_family '^stemsbeamvlinkouterblinker' "$outer_blinker_fixture" plain "$outer_txn2"
split_family '^stemsbeamschedulerresume' "$scheduler_resume_fixture" plain "$resume_extra"
if [ -s "$resume_extra" ]; then
    echo "unexpected extra resume rows beyond the frozen boundary-19 fixture" >&2
    head -4 "$resume_extra" >&2
    exit 1
fi
rm -f "$resume_extra"

# Per-family txn2 fixture files. Head/outer txn2 rows are the tail past the frozen
# prefix; the other families print txn2 rows only.
emit_family()
{
    family_grep=$1
    schema=$2
    out_file=$3
    skip=$4
    {
        echo "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) boundary-20 second-transaction rows."
        echo "$schema"
        echo "# Fast evidence per rust/PORTING.md: fresh-evidence second transaction; every fresh"
        echo "# emitter reproduced the frozen first-transaction rows byte-for-byte in this same"
        echo "# run (replay-on-frozen), and the txn1 head/outer/resume prefixes matched their"
        echo "# frozen fixtures before this file was written."
        grep "$family_grep" "$pass_out" | tail -n "+$((skip + 1))"
    } > "$out_file"
}
emit_family '^stemsbeamcreatestem' '# schema: stems-beam-create-stem-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-create-stem-$page_key.txt" 0
emit_family '^stemsbeamvlinkreusecheck' '# schema: stems-beam-vlink-reuse-check-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-reuse-check-$page_key.txt" 0
emit_family '^stemsbeamvlinkbaseapply' '# schema: stems-beam-vlink-base-apply-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-base-apply-$page_key.txt" 0
# The strict boundary-14 parser requires a page row; the frozen fixture's page row is
# valid provenance for the second transaction (identical predecessor inputs).
frozen_base_page=$(grep -m1 '^stemsbeamvlinkbaseapplypage ' "$base_apply_fixture")
printf '%s\n' "$frozen_base_page" >> "$repo_root/rust/oracle/stems-beam-txn2-base-apply-$page_key.txt"
emit_family '^stemsbeamvlinkblinkerflag' '# schema: stems-beam-vlink-b-linker-flag-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-b-linker-flag-$page_key.txt" 0
emit_family '^stemsbeamvlinksiblinglinks' '# schema: stems-beam-vlink-sibling-links-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-sibling-links-$page_key.txt" 0
emit_family_file()
{
    source_file=$1
    schema=$2
    out_file=$3
    {
        echo "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) boundary-20 second-transaction rows."
        echo "$schema"
        echo "# Fast evidence per rust/PORTING.md: fresh-evidence second transaction; every fresh"
        echo "# emitter reproduced the frozen first-transaction rows byte-for-byte in this same"
        echo "# run (replay-on-frozen), and the txn1 head/outer/resume subsequences matched their"
        echo "# frozen fixtures before this file was written."
        cat "$source_file"
    } > "$out_file"
}
emit_family_file "$head_txn2" '# schema: stems-beam-vlink-head-links-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-head-links-$page_key.txt"
emit_family_file "$outer_txn2" '# schema: stems-beam-vlink-outer-blinker-v1' \
    "$repo_root/rust/oracle/stems-beam-txn2-outer-blinker-$page_key.txt"
rm -f "$head_txn2" "$outer_txn2"
{
    echo "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) boundary-20 manifest."
    echo "# schema: stems-beam-txn2-manifest-v1"
    grep '^stemsbeamtxn2page' "$pass_out"
    grep '^stemsbeamschedulerresume' "$pass_out"
} > "$repo_root/rust/oracle/stems-beam-txn2-manifest-$page_key.txt"
rm -f "$pass_out"
for family in create-stem reuse-check base-apply b-linker-flag sibling-links head-links outer-blinker; do
    file="$repo_root/rust/oracle/stems-beam-txn2-$family-$page_key.txt"
    rows=$(grep -c '^stemsbeam' "$file" || true)
    if [ "$rows" -lt "$expected_systems" ]; then
        echo "txn2 $family has only $rows rows" >&2
        exit 1
    fi
done
echo "wrote per-family txn2 fixtures for $page_key"
