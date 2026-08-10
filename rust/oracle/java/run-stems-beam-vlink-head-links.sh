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
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkHeadLinksProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlink-head-links.sh"

case "$page_key" in
    chula) page_file=chula.png; expected_systems=3 ;;
    allegretto) page_file=allegretto.png; expected_systems=3 ;;
    batuque) page_file=batuque.png; expected_systems=3 ;;
    carmen) page_file=carmen.png; expected_systems=5 ;;
    cucaracha) page_file=cucaracha.png; expected_systems=3 ;;
    hove) page_file=hove.png; expected_systems=5 ;;
    zizi) page_file=zizi.png; expected_systems=2 ;;
    BachInvention5) page_file=BachInvention5.jpg; expected_systems=6 ;;
    *) echo "unknown beam VLink head-links page key: $page_key" >&2; exit 2 ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
create_stem_fixture="$repo_root/rust/oracle/stems-beam-create-stem-$page_key.txt"
reuse_check_fixture="$repo_root/rust/oracle/stems-beam-vlink-reuse-check-$page_key.txt"
base_apply_fixture="$repo_root/rust/oracle/stems-beam-vlink-base-apply-$page_key.txt"
base_apply_manifest="$repo_root/rust/oracle/stems-beam-vlink-base-apply-manifest.txt"
base_apply_probe="$repo_root/rust/oracle/java/StemsBeamVLinkBaseApplyProbe.java"
base_apply_runner="$repo_root/rust/oracle/java/run-stems-beam-vlink-base-apply.sh"
b_linker_flag_fixture="$repo_root/rust/oracle/stems-beam-vlink-b-linker-flag-$page_key.txt"
b_linker_flag_manifest="$repo_root/rust/oracle/stems-beam-vlink-b-linker-flag-manifest.txt"
b_linker_flag_probe="$repo_root/rust/oracle/java/StemsBeamVLinkBLinkerFlagProbe.java"
b_linker_flag_runner="$repo_root/rust/oracle/java/run-stems-beam-vlink-b-linker-flag.sh"
sibling_links_fixture="$repo_root/rust/oracle/stems-beam-vlink-sibling-links-$page_key.txt"
sibling_links_manifest="$repo_root/rust/oracle/stems-beam-vlink-sibling-links-manifest.txt"
sibling_links_probe="$repo_root/rust/oracle/java/StemsBeamVLinkSiblingLinksProbe.java"
sibling_links_runner="$repo_root/rust/oracle/java/run-stems-beam-vlink-sibling-links.sh"
for required in "$probe_cp_file" "$scheduler_fixture" "$expand_fixture" \
        "$create_stem_fixture" "$reuse_check_fixture" "$base_apply_fixture" \
        "$base_apply_manifest" "$base_apply_probe" "$base_apply_runner" \
        "$b_linker_flag_fixture" "$b_linker_flag_manifest" \
        "$b_linker_flag_probe" "$b_linker_flag_runner" \
        "$sibling_links_fixture" "$sibling_links_manifest" \
        "$sibling_links_probe" "$sibling_links_runner"; do
    if [ ! -f "$required" ]; then
        echo "missing frozen B-linker-flag input: $required" >&2
        exit 2
    fi
done

sha256_file()
{
    shasum -a 256 "$1" | awk '{print $1}'
}

sha256_text()
{
    shasum -a 256 | awk '{print $1}'
}

# Bind both ordered entry names and every byte reachable through the effective production
# classpath. Key loaded classes are also emitted separately below for readable provenance.
effective_classpath_sha256()
{
    classpath=$1
    remaining=$classpath
    {
        while :; do
            case "$remaining" in
                *:*) entry=${remaining%%:*}; remaining=${remaining#*:} ;;
                *) entry=$remaining; remaining= ;;
            esac
            printf 'entry %s\n' "$entry"
            if [ -f "$entry" ]; then
                printf 'file %s\n' "$(sha256_file "$entry")"
            elif [ -d "$entry" ]; then
                # Batch hashing avoids spawning one shasum process per class while retaining a
                # deterministic, path-sensitive digest of every directory member.
                find "$entry" -type f -exec shasum -a 256 {} + | LC_ALL=C sort
            else
                echo "effective classpath entry disappeared: $entry" >&2
                return 1
            fi
            if [ -z "$remaining" ]; then break; fi
        done
    } | sha256_text
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
base_apply_sha256=$(sha256_file "$base_apply_fixture")
base_apply_manifest_sha256=$(sha256_file "$base_apply_manifest")
b_linker_flag_sha256=$(sha256_file "$b_linker_flag_fixture")
b_linker_flag_manifest_sha256=$(sha256_file "$b_linker_flag_manifest")
sibling_links_sha256=$(sha256_file "$sibling_links_fixture")
sibling_links_manifest_sha256=$(sha256_file "$sibling_links_manifest")

create_scheduler_pin=$(field_from_row stemsbeamcreatestemcorpussummary schedulerFixtureSha256 "$create_stem_fixture")
create_expand_pin=$(field_from_row stemsbeamcreatestemcorpussummary expandFixtureSha256 "$create_stem_fixture")
reuse_scheduler_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary schedulerFixtureSha256 "$reuse_check_fixture")
reuse_expand_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary expandFixtureSha256 "$reuse_check_fixture")
reuse_create_pin=$(field_from_row stemsbeamvlinkreusecheckcorpussummary createStemFixtureSha256 "$reuse_check_fixture")
base_scheduler_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary schedulerFixtureSha256 "$base_apply_fixture")
base_expand_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary expandFixtureSha256 "$base_apply_fixture")
base_create_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary createStemFixtureSha256 "$base_apply_fixture")
base_reuse_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary reuseCheckFixtureSha256 "$base_apply_fixture")
base_page=$(awk '/^stemsbeamvlinkbaseapplypage / { print $2 }' "$base_apply_fixture")
manifest_fixture_pin=$(awk -v page="$page_key" '
    $1 == "stemsbeamvlinkbaseapplymanifestentry" {
        found = 0
        for (i = 2; i < NF; i += 2) if ($i == "page" && $(i + 1) == page) found = 1
        if (found) for (i = 2; i < NF; i += 2) if ($i == "fixtureSha256") print $(i + 1)
    }
' "$base_apply_manifest")
b_flag_scheduler_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        schedulerFixtureSha256 "$b_linker_flag_fixture")
b_flag_expand_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        expandFixtureSha256 "$b_linker_flag_fixture")
b_flag_create_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        createStemFixtureSha256 "$b_linker_flag_fixture")
b_flag_reuse_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        reuseCheckFixtureSha256 "$b_linker_flag_fixture")
b_flag_base_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        baseApplyFixtureSha256 "$b_linker_flag_fixture")
b_flag_base_manifest_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        baseApplyManifestSha256 "$b_linker_flag_fixture")
b_flag_page=$(awk '/^stemsbeamvlinkblinkerflagpage / { print $2 }' \
        "$b_linker_flag_fixture")
b_flag_manifest_fixture_pin=$(awk -v page="$page_key" '
    $1 == "stemsbeamvlinkblinkerflagmanifestentry" {
        found = 0
        for (i = 2; i < NF; i += 2) if ($i == "page" && $(i + 1) == page) found = 1
        if (found) for (i = 2; i < NF; i += 2) if ($i == "fixtureSha256") print $(i + 1)
    }
' "$b_linker_flag_manifest")
sibling_scheduler_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        schedulerFixtureSha256 "$sibling_links_fixture")
sibling_expand_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        expandFixtureSha256 "$sibling_links_fixture")
sibling_create_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        createStemFixtureSha256 "$sibling_links_fixture")
sibling_reuse_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        reuseCheckFixtureSha256 "$sibling_links_fixture")
sibling_base_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        baseApplyFixtureSha256 "$sibling_links_fixture")
sibling_b_flag_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary \
        bLinkerFlagFixtureSha256 "$sibling_links_fixture")
sibling_page=$(awk '/^stemsbeamvlinksiblinglinkspage / { print $2 }' \
        "$sibling_links_fixture")
sibling_manifest_fixture_pin=$(awk -v page="$page_key" '
    $1 == "stemsbeamvlinksiblinglinksmanifestentry" {
        found = 0
        for (i = 2; i < NF; i += 2) if ($i == "page" && $(i + 1) == page) found = 1
        if (found) for (i = 2; i < NF; i += 2) if ($i == "fixtureSha256") print $(i + 1)
    }
' "$sibling_links_manifest")
if [ "$create_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$create_expand_pin" != "$expand_sha256" ] || \
        [ "$reuse_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$reuse_expand_pin" != "$expand_sha256" ] || \
        [ "$reuse_create_pin" != "$create_stem_sha256" ] || \
        [ "$base_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$base_expand_pin" != "$expand_sha256" ] || \
        [ "$base_create_pin" != "$create_stem_sha256" ] || \
        [ "$base_reuse_pin" != "$reuse_check_sha256" ] || \
        [ "$manifest_fixture_pin" != "$base_apply_sha256" ] || \
        [ "$base_page" != "$page_file#1" ] || \
        [ "$b_flag_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$b_flag_expand_pin" != "$expand_sha256" ] || \
        [ "$b_flag_create_pin" != "$create_stem_sha256" ] || \
        [ "$b_flag_reuse_pin" != "$reuse_check_sha256" ] || \
        [ "$b_flag_base_pin" != "$base_apply_sha256" ] || \
        [ "$b_flag_base_manifest_pin" != "$base_apply_manifest_sha256" ] || \
        [ "$b_flag_manifest_fixture_pin" != "$b_linker_flag_sha256" ] || \
        [ "$b_flag_page" != "$page_file#1" ] || \
        [ "$sibling_scheduler_pin" != "$scheduler_sha256" ] || \
        [ "$sibling_expand_pin" != "$expand_sha256" ] || \
        [ "$sibling_create_pin" != "$create_stem_sha256" ] || \
        [ "$sibling_reuse_pin" != "$reuse_check_sha256" ] || \
        [ "$sibling_base_pin" != "$base_apply_sha256" ] || \
        [ "$sibling_b_flag_pin" != "$b_linker_flag_sha256" ] || \
        [ "$sibling_manifest_fixture_pin" != "$sibling_links_sha256" ] || \
        [ "$sibling_page" != "$page_file#1" ]; then
    echo "boundary-16 predecessor provenance mismatch for $page_key" >&2
    exit 1
fi

hash_set=$(printf '%s\n' "$scheduler_sha256" "$expand_sha256" "$create_stem_sha256" \
        "$reuse_check_sha256" "$base_apply_sha256" "$b_linker_flag_sha256" \
        "$sibling_links_sha256" | \
        sort -u | wc -l | tr -d ' ')
if [ "$hash_set" -ne 7 ]; then
    echo "predecessor hashes are not independently discriminating" >&2
    exit 1
fi

probe_cp_raw=$(sed -n '1p' "$probe_cp_file")
# The oracle uses only production classes. Exclude Gradle test outputs so an unrelated test helper
# cannot shadow a production Audiveris class on either the compiler or runtime class path. Omit
# nonexistent inherited entries as well, so javac and java see one identical warning-free path.
probe_cp=
remaining_cp=$probe_cp_raw
main_classes_count=0
main_resources_count=0
while :; do
    case "$remaining_cp" in
        *:*) cp_entry=${remaining_cp%%:*}; remaining_cp=${remaining_cp#*:} ;;
        *) cp_entry=$remaining_cp; remaining_cp= ;;
    esac
    case "$cp_entry" in
        */app/build/classes/java/test|*/app/build/resources/test) ;;
        *)
            if [ -e "$cp_entry" ]; then
                case ":$probe_cp:" in
                    *:"$cp_entry":*)
                        echo "duplicate production oracle classpath entry: $cp_entry" >&2
                        exit 1
                        ;;
                esac
                if [ -n "$probe_cp" ]; then probe_cp="$probe_cp:"; fi
                probe_cp="$probe_cp$cp_entry"
                if [ "$cp_entry" = "$repo_root/app/build/classes/java/main" ]; then
                    main_classes_count=$((main_classes_count + 1))
                elif [ "$cp_entry" = "$repo_root/app/build/resources/main" ]; then
                    main_resources_count=$((main_resources_count + 1))
                fi
            fi
            ;;
    esac
    if [ -z "$remaining_cp" ]; then break; fi
done
if [ "$main_classes_count" -ne 1 ] || [ "$main_resources_count" -ne 1 ]; then
    echo "production oracle classpath lacks unique Audiveris main outputs" >&2
    exit 1
fi
expected_classpath_prefix="$repo_root/app/build/classes/java/main:$repo_root/app/build/resources/main"
case "$probe_cp" in
    "$expected_classpath_prefix":*) ;;
    *)
        echo "production Audiveris classes/resources are not first on the effective classpath" >&2
        exit 1
        ;;
esac
jgrapht_core_jars=$(printf '%s\n' "$probe_cp" | tr ':' '\n' | \
        awk '/\/jgrapht-core\/.*\/jgrapht-core-[^\/]+\.jar$/ { print }')
jgrapht_count=$(printf '%s\n' "$jgrapht_core_jars" | awk 'NF { count++ } END { print count + 0 }')
if [ "$jgrapht_count" -ne 1 ]; then
    echo "classpath must contain exactly one JGraphT core jar" >&2
    exit 1
fi
jgrapht_jar=$jgrapht_core_jars
if ! printf '%s\n' "$jgrapht_jar" | \
        awk '/\/jgrapht-core\/1\.5\.2\/.+\/jgrapht-core-1\.5\.2\.jar$/ { found = 1 } END { exit !found }' || \
        [ ! -f "$jgrapht_jar" ]; then
    echo "the sole JGraphT core jar must be version 1.5.2" >&2
    exit 1
fi
jgrapht_sha256=$(sha256_file "$jgrapht_jar")
expected_jgrapht_sha256=dfa596e9f0d0838f1b5e81dd0cd60e3a76c2c290ac25a0a029ffde58cf5e4c14
if [ "$jgrapht_sha256" != "$expected_jgrapht_sha256" ]; then
    echo "JGraphT core 1.5.2 artifact drift" >&2
    exit 1
fi

verify_base_apply_source_pin()
{
    pin_name=$1
    source_path=$2
    current_pin=$(sha256_file "$source_path")
    split_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary "$pin_name" \
            "$base_apply_fixture")
    manifest_pin=$(field_from_row stemsbeamvlinkbaseapplymanifestsummary "$pin_name" \
            "$base_apply_manifest")
    if [ "$current_pin" != "$split_pin" ] || [ "$split_pin" != "$manifest_pin" ]; then
        echo "boundary-14 transitive source pin mismatch: $pin_name" >&2
        exit 1
    fi
}

# Boundary 15 replays boundary 14, so bind the frozen predecessor implementation and every
# source/artifact that its split fixture and manifest claim before launching the first JVM.
verify_base_apply_source_pin probeSourceSha256 "$base_apply_probe"
verify_base_apply_source_pin runnerSourceSha256 "$base_apply_runner"
verify_base_apply_source_pin beamLinkerSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java"
verify_base_apply_source_pin linkSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Link.java"
verify_base_apply_source_pin sigraphSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/SIGraph.java"
verify_base_apply_source_pin sigListenerSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/SigListener.java"
verify_base_apply_source_pin systemInfoSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/SystemInfo.java"
verify_base_apply_source_pin sheetSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Sheet.java"
verify_base_apply_source_pin basicIndexSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/util/BasicIndex.java"
verify_base_apply_source_pin entityIndexSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/util/EntityIndex.java"
verify_base_apply_source_pin glyphIndexSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/glyph/GlyphIndex.java"
verify_base_apply_source_pin interIndexSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/InterIndex.java"
verify_base_apply_source_pin abstractEntitySourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/util/AbstractEntity.java"
verify_base_apply_source_pin abstractInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/AbstractInter.java"
verify_base_apply_source_pin interSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/Inter.java"
verify_base_apply_source_pin stemInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/StemInter.java"
verify_base_apply_source_pin abstractChordInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/AbstractChordInter.java"
verify_base_apply_source_pin headChordInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/HeadChordInter.java"
verify_base_apply_source_pin relationSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Relation.java"
verify_base_apply_source_pin supportSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Support.java"
verify_base_apply_source_pin beamStemRelationSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamStemRelation.java"
verify_base_apply_source_pin beamRestRelationSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamRestRelation.java"
verify_base_apply_source_pin chordStemRelationSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/ChordStemRelation.java"
verify_base_apply_source_pin abstractBeamInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/AbstractBeamInter.java"
verify_base_apply_source_pin beamInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/BeamInter.java"
verify_base_apply_source_pin beamHookInterSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/BeamHookInter.java"
verify_base_apply_source_pin sheetStubSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/SheetStub.java"
verify_base_apply_source_pin bookSourceSha256 "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Book.java"
verify_base_apply_source_pin gradleSourceSha256 "$repo_root/app/build.gradle"
base_jgrapht_pin=$(field_from_row stemsbeamvlinkbaseapplycorpussummary \
        jgraphtCoreJarSha256 "$base_apply_fixture")
manifest_jgrapht_pin=$(field_from_row stemsbeamvlinkbaseapplymanifestsummary \
        jgraphtCoreJarSha256 "$base_apply_manifest")
base_jgrapht_version=$(field_from_row stemsbeamvlinkbaseapplycorpussummary \
        jgraphtCoreVersion "$base_apply_fixture")
manifest_jgrapht_version=$(field_from_row stemsbeamvlinkbaseapplymanifestsummary \
        jgraphtCoreVersion "$base_apply_manifest")
if [ "$jgrapht_sha256" != "$base_jgrapht_pin" ] || \
        [ "$base_jgrapht_pin" != "$manifest_jgrapht_pin" ] || \
        [ "$base_jgrapht_version" != "1.5.2" ] || \
        [ "$manifest_jgrapht_version" != "1.5.2" ]; then
    echo "boundary-14 transitive JGraphT pin mismatch" >&2
    exit 1
fi

verify_b_linker_flag_source_pin()
{
    pin_name=$1
    source_path=$2
    current_pin=$(sha256_file "$source_path")
    split_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary "$pin_name" \
            "$b_linker_flag_fixture")
    manifest_pin=$(field_from_row stemsbeamvlinkblinkerflagmanifestsummary "$pin_name" \
            "$b_linker_flag_manifest")
    if [ "$current_pin" != "$split_pin" ] || [ "$split_pin" != "$manifest_pin" ]; then
        echo "boundary-15 transitive source pin mismatch: $pin_name" >&2
        exit 1
    fi
}

# Boundary 16 resumes the exact installed Boundary-15 transaction, so bind the probe, runner,
# active setter/seam sources, and shared ordering enums to both its split and terminal manifest.
verify_b_linker_flag_source_pin probeSourceSha256 "$b_linker_flag_probe"
verify_b_linker_flag_source_pin runnerSourceSha256 "$b_linker_flag_runner"
verify_b_linker_flag_source_pin beamLinkerSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java"
verify_b_linker_flag_source_pin stemLinkerSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java"
verify_b_linker_flag_source_pin horizontalSideSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/util/HorizontalSide.java"
verify_b_linker_flag_source_pin verticalSideSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/util/VerticalSide.java"
b_flag_manifest_base_pin=$(field_from_row stemsbeamvlinkblinkerflagmanifestsummary \
        baseApplyManifestSha256 "$b_linker_flag_manifest")
b_flag_jgrapht_pin=$(field_from_row stemsbeamvlinkblinkerflagcorpussummary \
        jgraphtCoreJarSha256 "$b_linker_flag_fixture")
b_flag_manifest_jgrapht_pin=$(field_from_row stemsbeamvlinkblinkerflagmanifestsummary \
        jgraphtCoreJarSha256 "$b_linker_flag_manifest")
if [ "$b_flag_manifest_base_pin" != "$base_apply_manifest_sha256" ] || \
        [ "$b_flag_jgrapht_pin" != "$jgrapht_sha256" ] || \
        [ "$b_flag_manifest_jgrapht_pin" != "$jgrapht_sha256" ]; then
    echo "boundary-15 transitive manifest/artifact pin mismatch" >&2
    exit 1
fi

verify_sibling_links_source_pin()
{
    pin_name=$1
    source_path=$2
    current_pin=$(sha256_file "$source_path")
    split_pin=$(field_from_row stemsbeamvlinksiblinglinkscorpussummary "$pin_name" \
            "$sibling_links_fixture")
    manifest_pin=$(field_from_row stemsbeamvlinksiblinglinksmanifestsummary "$pin_name" \
            "$sibling_links_manifest")
    if [ "$current_pin" != "$split_pin" ] || [ "$split_pin" != "$manifest_pin" ]; then
        echo "boundary-16 transitive source pin mismatch: $pin_name" >&2
        exit 1
    fi
}

# Boundary 17 reruns the complete typed Boundary-16 transaction. Bind the predecessor oracle and
# every Boundary-16-active source to both its split fixture and terminal manifest.
verify_sibling_links_source_pin probeSourceSha256 "$sibling_links_probe"
verify_sibling_links_source_pin runnerSourceSha256 "$sibling_links_runner"
verify_sibling_links_source_pin beamLinkerSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java"
verify_sibling_links_source_pin stemLinkerSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java"
verify_sibling_links_source_pin stemBuilderSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java"
verify_sibling_links_source_pin stemItemSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemItem.java"
verify_sibling_links_source_pin stemsRetrieverSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"
verify_sibling_links_source_pin beamGroupInterSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/BeamGroupInter.java"
verify_sibling_links_source_pin smallBeamInterSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/SmallBeamInter.java"
verify_sibling_links_source_pin ensembleHelperSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/EnsembleHelper.java"
verify_sibling_links_source_pin containmentSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Containment.java"
verify_sibling_links_source_pin beamBeamRelationSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamBeamRelation.java"
verify_sibling_links_source_pin abstractStemConnectionSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/AbstractStemConnection.java"
verify_sibling_links_source_pin beamPortionSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamPortion.java"
verify_sibling_links_source_pin scaleSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Scale.java"
verify_sibling_links_source_pin skewSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Skew.java"
verify_sibling_links_source_pin staffSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Staff.java"
verify_sibling_links_source_pin staffLineSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/StaffLine.java"
verify_sibling_links_source_pin lineInfoSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/grid/LineInfo.java"
verify_sibling_links_source_pin lineUtilSourceSha256 \
        "$repo_root/app/src/main/java/org/audiveris/omr/math/LineUtil.java"

b17_active_source_rows()
{
    printf 'beamLinkerSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java")"
    printf 'stemLinkerSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java")"
    printf 'stemBuilderSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java")"
    printf 'stemItemSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemItem.java")"
    printf 'stemsRetrieverSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java")"
    printf 'beamGroupInterSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/BeamGroupInter.java")"
    printf 'smallBeamInterSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/SmallBeamInter.java")"
    printf 'ensembleHelperSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/EnsembleHelper.java")"
    printf 'containmentSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/Containment.java")"
    printf 'beamBeamRelationSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamBeamRelation.java")"
    printf 'abstractStemConnectionSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/AbstractStemConnection.java")"
    printf 'beamPortionSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/BeamPortion.java")"
    printf 'scaleSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Scale.java")"
    printf 'skewSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Skew.java")"
    printf 'staffSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Staff.java")"
    printf 'staffLineSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/StaffLine.java")"
    printf 'lineInfoSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/grid/LineInfo.java")"
    printf 'lineUtilSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/math/LineUtil.java")"
    printf 'headLinkerSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java")"
    printf 'headInterSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/inter/HeadInter.java")"
    printf 'headStemRelationSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sig/relation/HeadStemRelation.java")"
    printf 'partSourceSha256 %s\n' "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/Part.java")"
}
b17_active_source_bundle_sha256_before=$(b17_active_source_rows | sha256_text)

lock_dir=/private/tmp/stems-beam-vlink-head-links.lock
if ! mkdir "$lock_dir"; then
    echo "another head-links oracle runner owns $lock_dir" >&2
    exit 1
fi
probe_classes=
pass_one=
pass_two=
cleanup()
{
    if [ -n "$probe_classes" ]; then rm -rf "$probe_classes"; fi
    if [ -n "$pass_one" ]; then rm -f "$pass_one"; fi
    if [ -n "$pass_two" ]; then rm -f "$pass_two"; fi
    rmdir "$lock_dir" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
probe_classes=$(mktemp -d /private/tmp/stems-beam-vlink-head-links-classes.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-beam-vlink-head-links-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-beam-vlink-head-links-pass2.XXXXXX)

probe_source_sha256_before=$(sha256_file "$probe_source")
runner_source_sha256_before=$(sha256_file "$runner_source")
probe_cp_file_sha256_before=$(sha256_file "$probe_cp_file")
effective_classpath_sha256_before=$(effective_classpath_sha256 "$probe_cp")
jdk_release_sha256_before=$(sha256_file "$JAVA_HOME/release")
java_executable_sha256_before=$(sha256_file "$JAVA_HOME/bin/java")
java_jpeg_library_sha256_before=$(sha256_file "$JAVA_HOME/lib/libjavajpeg.dylib")
java_modules_sha256_before=$(sha256_file "$JAVA_HOME/lib/modules")
java_vm_library_sha256_before=$(sha256_file "$JAVA_HOME/lib/server/libjvm.dylib")
java_awt_library_sha256_before=$(sha256_file "$JAVA_HOME/lib/libawt.dylib")
java_awt_lwawt_library_sha256_before=$(sha256_file "$JAVA_HOME/lib/libawt_lwawt.dylib")
java_architecture=$(release_field OS_ARCH)
java_runtime_version=$(release_field JAVA_RUNTIME_VERSION)
java_vm_variant=$(release_field JVM_VARIANT)
java_image_type=$(release_field IMAGE_TYPE)
beam_linker_source_sha256_before=$(sha256_file \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java")
stem_linker_source_sha256_before=$(sha256_file \
        "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java")
beam_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker.class")
b_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker.class")
v_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker\$VLinker.class")
stem_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/StemLinker.class")
head_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker.class")
s_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker\$SLinker.class")
c_linker_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker\$SLinker\$CLinker.class")
head_inter_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/HeadInter.class")
stem_inter_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/StemInter.class")
head_stem_relation_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/relation/HeadStemRelation.class")
head_chord_inter_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/HeadChordInter.class")
chord_stem_relation_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/relation/ChordStemRelation.class")
part_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/Part.class")
sigraph_class_sha256_before=$(sha256_file \
        "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/SIGraph.class")
page_input_sha256_before=$(sha256_file "$repo_root/data/examples/$page_file")

expected_jdk_release_sha256=0cac9d5b21cd5a251ecb5064526a1d2b38d80ddfffc0531d0d5b765ac0117e08
expected_java_executable_sha256=0a1eea36b7899323b32caab6f1d0e416ad7208792b076391278062efab4b15d8
expected_java_jpeg_library_sha256=ab2e9b8a49e053ff9c7c2ca11fbbf7fdf82e42ece1f2f768fbef0f02130dc958
expected_java_modules_sha256=0c2bf8eb97ddd398588f2c7038e0cc4e3708ec1e5c683d0554c8a49549966a5e
expected_java_vm_library_sha256=260f5d38172bd53ee206baddd9b43c06e040ea6a6203440824e658f94556203b
expected_java_awt_library_sha256=701305c6c4d3967a46831e1ee59780105e644973ad904497e220d12413848312
expected_java_awt_lwawt_library_sha256=5302653069a3157d4f21dea234b1ff17757930647736d78958fcb999572c3e46
expected_beam_linker_class_sha256=cce4ad51e766cc6a7ed0e06234c41abedbdad3c83ab1123a17da9601c73640b7
expected_b_linker_class_sha256=f934e6d4f96258ec19ac5ae59ec4425a502a620653895239b4789f3e3f7798a0
expected_v_linker_class_sha256=ad68fa1774655ba9daeea85575dd9cd19da1323029e1261c0b54a9b92f8562d0
expected_stem_linker_class_sha256=74244246274f00ad582c122b3154a60d66527a3db36897abe435ec28f31c1438
expected_head_linker_class_sha256=53c2dfc0c193727ceba4b4f2bc8b8fe2337193799c5f0fae09e94c3a6973b190
expected_s_linker_class_sha256=96e5b0c785d5b3254b2ae3ac7b58c82fb86edcd168d02b3f0f23842d915f68a8
expected_c_linker_class_sha256=d4dbf011fd1cf4a8d71a9fd633f57f9aef2a37e343e2e3fae3c32c2bd7a74eed
expected_head_inter_class_sha256=376fdb8a54bde5563bff0a5c2e9266f3ae10cae39b6417ecf07a5cd1fd2b3025
expected_stem_inter_class_sha256=54d44cff96e202a8d5c051fb0d2eed5967395e1e1f6b610266eae647ae9fc8f7
expected_head_stem_relation_class_sha256=ce7857f515641dfb02ffd417a25ade027c540588d37611568243f85b1d18cb24
expected_head_chord_inter_class_sha256=2510e00512308242868f765bd1f8255ab66f0557406b640834b39e099240d4cf
expected_chord_stem_relation_class_sha256=f0db7e1b6f069da8c2e93ba53e6680fd1eda8ee615def92f91605bcc168b155e
expected_part_class_sha256=f72bb09e0880c8b925b88665ad30bdb8bb6a8ec67a6ed4add44fd969eb3efcf7
expected_sigraph_class_sha256=37951822aacc1abab352edfa4a083514f3beded38a26a857894578a330be77e0
if [ "$jdk_release_sha256_before" != "$expected_jdk_release_sha256" ] || \
        [ "$java_executable_sha256_before" != "$expected_java_executable_sha256" ] || \
        [ "$java_jpeg_library_sha256_before" != "$expected_java_jpeg_library_sha256" ] || \
        [ "$java_modules_sha256_before" != "$expected_java_modules_sha256" ] || \
        [ "$java_vm_library_sha256_before" != "$expected_java_vm_library_sha256" ] || \
        [ "$java_awt_library_sha256_before" != "$expected_java_awt_library_sha256" ] || \
        [ "$java_awt_lwawt_library_sha256_before" != "$expected_java_awt_lwawt_library_sha256" ] || \
        [ "$beam_linker_class_sha256_before" != "$expected_beam_linker_class_sha256" ] || \
        [ "$b_linker_class_sha256_before" != "$expected_b_linker_class_sha256" ] || \
        [ "$v_linker_class_sha256_before" != "$expected_v_linker_class_sha256" ] || \
        [ "$stem_linker_class_sha256_before" != "$expected_stem_linker_class_sha256" ] || \
        [ "$head_linker_class_sha256_before" != "$expected_head_linker_class_sha256" ] || \
        [ "$s_linker_class_sha256_before" != "$expected_s_linker_class_sha256" ] || \
        [ "$c_linker_class_sha256_before" != "$expected_c_linker_class_sha256" ] || \
        [ "$head_inter_class_sha256_before" != "$expected_head_inter_class_sha256" ] || \
        [ "$stem_inter_class_sha256_before" != "$expected_stem_inter_class_sha256" ] || \
        [ "$head_stem_relation_class_sha256_before" != "$expected_head_stem_relation_class_sha256" ] || \
        [ "$head_chord_inter_class_sha256_before" != "$expected_head_chord_inter_class_sha256" ] || \
        [ "$chord_stem_relation_class_sha256_before" != "$expected_chord_stem_relation_class_sha256" ] || \
        [ "$part_class_sha256_before" != "$expected_part_class_sha256" ] || \
        [ "$sigraph_class_sha256_before" != "$expected_sigraph_class_sha256" ]; then
    echo "frozen Temurin/application bytecode artifact drift" >&2
    exit 1
fi

env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
    "$JAVA_HOME/bin/javac" -Xlint:all,-path -cp "$probe_cp" -d "$probe_classes" "$probe_source"

run_fresh_pass()
{
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
                org.audiveris.omr.rustport.StemsBeamVLinkHeadLinksProbe \
                --system "$system_id" \
                "$repo_root/data/examples/$page_file:1" \
                "$scheduler_fixture" "$expand_fixture" "$create_stem_fixture" \
                "$reuse_check_fixture" "$base_apply_fixture" "$b_linker_flag_fixture" \
                "$sibling_links_fixture"
        )
        system_id=$((system_id + 1))
    done
}

# javac and every runtime JVM are foreground, awaited, and reaped. The runner never starts '&'.
# Therefore the maximum concurrent Java process count within this locked runner invocation is one.
run_fresh_pass > "$pass_one"
run_fresh_pass > "$pass_two"
if ! cmp -s "$pass_one" "$pass_two"; then
    echo "two fresh $page_key head-links passes are not byte-identical" >&2
    exit 1
fi
if [ "$(sha256_file "$probe_source")" != "$probe_source_sha256_before" ] || \
        [ "$(sha256_file "$runner_source")" != "$runner_source_sha256_before" ] || \
        [ "$(sha256_file "$probe_cp_file")" != "$probe_cp_file_sha256_before" ] || \
        [ "$(effective_classpath_sha256 "$probe_cp")" != "$effective_classpath_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/release")" != "$jdk_release_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/bin/java")" != "$java_executable_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/lib/libjavajpeg.dylib")" != "$java_jpeg_library_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/lib/modules")" != "$java_modules_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/lib/server/libjvm.dylib")" != "$java_vm_library_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/lib/libawt.dylib")" != "$java_awt_library_sha256_before" ] || \
        [ "$(sha256_file "$JAVA_HOME/lib/libawt_lwawt.dylib")" != "$java_awt_lwawt_library_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java")" \
            != "$beam_linker_source_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java")" \
            != "$stem_linker_source_sha256_before" ] || \
        [ "$(b17_active_source_rows | sha256_text)" \
            != "$b17_active_source_bundle_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker.class")" \
            != "$beam_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker.class")" \
            != "$b_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker\$VLinker.class")" \
            != "$v_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/StemLinker.class")" \
            != "$stem_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker.class")" \
            != "$head_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker\$SLinker.class")" \
            != "$s_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/HeadLinker\$SLinker\$CLinker.class")" \
            != "$c_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/HeadInter.class")" \
            != "$head_inter_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/StemInter.class")" \
            != "$stem_inter_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/relation/HeadStemRelation.class")" \
            != "$head_stem_relation_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/inter/HeadChordInter.class")" \
            != "$head_chord_inter_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/relation/ChordStemRelation.class")" \
            != "$chord_stem_relation_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/Part.class")" \
            != "$part_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sig/SIGraph.class")" \
            != "$sigraph_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/data/examples/$page_file")" != "$page_input_sha256_before" ] || \
        [ "$(sha256_file "$scheduler_fixture")" != "$scheduler_sha256" ] || \
        [ "$(sha256_file "$expand_fixture")" != "$expand_sha256" ] || \
        [ "$(sha256_file "$create_stem_fixture")" != "$create_stem_sha256" ] || \
        [ "$(sha256_file "$reuse_check_fixture")" != "$reuse_check_sha256" ] || \
        [ "$(sha256_file "$base_apply_fixture")" != "$base_apply_sha256" ] || \
        [ "$(sha256_file "$base_apply_manifest")" != "$base_apply_manifest_sha256" ] || \
        [ "$(sha256_file "$b_linker_flag_fixture")" != "$b_linker_flag_sha256" ] || \
        [ "$(sha256_file "$b_linker_flag_manifest")" != "$b_linker_flag_manifest_sha256" ] || \
        [ "$(sha256_file "$sibling_links_fixture")" != "$sibling_links_sha256" ] || \
        [ "$(sha256_file "$sibling_links_manifest")" != "$sibling_links_manifest_sha256" ] || \
        [ "$(sha256_file "$jgrapht_jar")" != "$jgrapht_sha256" ]; then
    echo "head-links input changed during the two-pass run" >&2
    exit 1
fi

schema_count=$(awk '$0 == "# schema: stems-beam-vlink-head-links-v1" { count++ } END { print count + 0 }' "$pass_one")
stale_count=$(awk '/^stemsbeamvlink(blinkerflag|baseapply|siblinglinks)/ { count++ } END { print count + 0 }' "$pass_one")
if [ "$schema_count" -ne 1 ] || [ "$stale_count" -ne 0 ]; then
    echo "head-links schema header/output family drift" >&2
    exit 1
fi

census=$(awk -v systems="$expected_systems" -v pageRef="$page_file#1" '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        if (((NF - start + 1) % 2) != 0) bad++
        for (i = start; i <= NF; i += 2) {
            if ($i in f) bad++
            f[$i] = $(i + 1)
        }
    }
    function key() { return f["system"] SUBSEP f["plan"] SUBSEP f["scope"] SUBSEP f["case"] }
    function entry_key(k) { return k SUBSEP f["mapOrdinal"] }
    function known(label) {
        return label ~ /^stemsbeamvlinkheadlinks(predecessor|baseline|headentry|slinkerwrite|sourceoutgoing|pairrelation|pairscan|consistency|edge|headincident|headincidentscan|stemincident|stemincidentscan|callback|entryresult|remainder|result|deltaguard|summary)$/
    }
    /^stemsbeamvlinkheadlinkspage / {
        page++
        if ($2 != pageRef || $3 != "systems" || $4 != systems) bad++
        next
    }
    /^stemsbeamvlinkheadlinkssyntheticcase / {
        fields(3)
        k = key()
        if ($2 != pageRef || f["system"] != 1 ||
                (f["scope"] != "synthetic" && f["scope"] != "envelope") ||
                f["case"] == "-" || f["join"] != "IsolatedBoundary16Replay" ||
                f["construction"] != "RealBookSheetSystemSIGHeadLinker" ||
                real != 1 || syntheticSeen[k]++) bad++
        syntheticOrder = syntheticOrder (syntheticOrder == "" ? "" : ",") f["case"]
        syntheticScope[k] = f["scope"]
        syntheticTerminal[k] = f["terminal"]
        syntheticExpectedEvents[k] = f["eventCount"] + 0
        if (f["relationManual"] == "true") manualCases++
        if ((f["newChordStemCount"] + 0) != 0) chordRewires += f["newChordStemCount"] + 0
        if (f["throwClass"] != "-") throws++
        next
    }
    /^stemsbeamvlinkheadlinkssyntheticevent / {
        fields(3)
        k = key()
        if (!(k in syntheticSeen) || syntheticGuard[k] ||
                f["eventOrdinal"] != syntheticEvents[k]) bad++
        syntheticEvents[k]++
        next
    }
    /^stemsbeamvlinkheadlinkssyntheticguard / {
        fields(3)
        k = key()
        if (!(k in syntheticSeen) || syntheticGuard[k]++ ||
                syntheticEvents[k] != syntheticExpectedEvents[k] ||
                f["terminal"] != syntheticTerminal[k] || f["isolatedOnly"] != "true" ||
                f["productionEquivalent"] != "false" ||
                f["enclosingRealSheetUnchanged"] != "true" ||
                f["outerBLinkerAssignmentRead"] != "false") bad++
        if (syntheticScope[k] == "synthetic") supported++
        else envelope++
        transactions++
        supplementalEvents += syntheticEvents[k]
        isolatedGraphDelta += f["graphDelta"] + 0
        next
    }
    /^stemsbeamvlinkheadlinks/ {
        if (!known($1)) { bad++; next }
        fields(3)
        k = key()
        label = $1
        seenKey[k] = 1
        family[k SUBSEP label]++
        if ($2 != pageRef || f["system"] !~ /^[1-9][0-9]*$/ ||
                f["plan"] !~ /^[0-9]+$/ || f["scope"] != "real" ||
                f["case"] != "-" || f["system"] > systems) bad++

        if (label == "stemsbeamvlinkheadlinkspredecessor") {
            if (phase[k] != 0 || f["predecessorTerminal"] != "ReadyBeforeHeadRelationLoop" ||
                    f["join"] != "FullBoundary16Replay") bad++
            phase[k] = 1
        } else if (label == "stemsbeamvlinkheadlinksbaseline") {
            if (phase[k] != 1 || f["soleSigListener"] != "true" ||
                    f["headless"] != "true" || f["eventStart"] != 0) bad++
            phase[k] = 2
            expectedEntries[k] = f["relationEntries"] + 0
            nextEvent[k] = f["eventStart"] + 0
        } else if (label == "stemsbeamvlinkheadlinksheadentry") {
            if ((phase[k] != 2 && phase[k] != 15) || f["mapOrdinal"] != entryCount[k] ||
                    (f["branch"] != "ExistingHeadStem" && f["branch"] != "Linked")) bad++
            ek = entry_key(k)
            branch[ek] = f["branch"]
            entryCount[k]++
            headEntries++
            if (f["branch"] == "ExistingHeadStem") duplicateEntries++
            phase[k] = 3
        } else if (label == "stemsbeamvlinkheadlinksslinkerwrite") {
            if (phase[k] != 3 || f["eventOrdinal"] != nextEvent[k] ||
                    f["writeCount"] != 1 || f["completed"] != "true") bad++
            nextEvent[k]++
            sWrites++
            sValueChanges += f["valueChangeCount"] + 0
            phase[k] = 4
        } else if (label == "stemsbeamvlinkheadlinkssourceoutgoing") {
            ek = entry_key(k)
            if ((phase[k] != 4 && phase[k] != 5) ||
                    f["sourceOutgoingOrdinal"] != sourceRows[ek]) bad++
            sourceRows[ek]++
            phase[k] = 5
        } else if (label == "stemsbeamvlinkheadlinkspairrelation") {
            ek = entry_key(k)
            if ((phase[k] < 4 || phase[k] > 6) || f["pairOrdinal"] != pairRows[ek]) bad++
            pairRows[ek]++
            phase[k] = 6
        } else if (label == "stemsbeamvlinkheadlinkspairscan") {
            ek = entry_key(k)
            if (phase[k] < 4 || phase[k] > 6 || f["sourceOutgoingCount"] != sourceRows[ek] ||
                    f["pairCount"] != pairRows[ek]) bad++
            if ((branch[ek] == "ExistingHeadStem" && f["state"] != "FirstHeadStemMatch") ||
                    (branch[ek] == "Linked" && f["state"] != "ExhaustiveNoMatch")) bad++
            phase[k] = 7
        } else if (label == "stemsbeamvlinkheadlinksconsistency") {
            if (phase[k] != 7 || branch[entry_key(k)] != "Linked" ||
                    f["eventOrdinal"] != nextEvent[k] || f["completed"] != "true") bad++
            nextEvent[k]++
            consistencyWrites++
            phase[k] = 8
        } else if (label == "stemsbeamvlinkheadlinksedge") {
            if (phase[k] != 8 || f["eventOrdinal"] != nextEvent[k] ||
                    f["insertionReturned"] != "true" || f["callbackSynchronous"] != "true") bad++
            nextEvent[k]++
            insertedEntries++
            phase[k] = 9
        } else if (label == "stemsbeamvlinkheadlinksheadincident") {
            ek = entry_key(k)
            if ((phase[k] != 9 && phase[k] != 10) ||
                    f["incidentOrdinal"] != headIncidentRows[ek]) bad++
            headIncidentRows[ek]++
            phase[k] = 10
        } else if (label == "stemsbeamvlinkheadlinksheadincidentscan") {
            ek = entry_key(k)
            if ((phase[k] != 9 && phase[k] != 10) ||
                    f["incidentCount"] != headIncidentRows[ek]) bad++
            phase[k] = 11
        } else if (label == "stemsbeamvlinkheadlinksstemincident") {
            ek = entry_key(k)
            if ((phase[k] != 11 && phase[k] != 12) ||
                    f["incidentOrdinal"] != stemIncidentRows[ek]) bad++
            stemIncidentRows[ek]++
            phase[k] = 12
        } else if (label == "stemsbeamvlinkheadlinksstemincidentscan") {
            ek = entry_key(k)
            if ((phase[k] != 11 && phase[k] != 12) ||
                    f["incidentCount"] != stemIncidentRows[ek]) bad++
            phase[k] = 13
        } else if (label == "stemsbeamvlinkheadlinkscallback") {
            if (phase[k] != 13 || f["eventOrdinal"] != nextEvent[k] ||
                    f["completed"] != "true") bad++
            nextEvent[k]++
            if (f["headAbnormalBefore"] != f["headAbnormalAfter"]) headChanges++
            if (f["stemAbnormalBefore"] != f["stemAbnormalAfter"]) stemChanges++
            phase[k] = 14
        } else if (label == "stemsbeamvlinkheadlinksentryresult") {
            ek = entry_key(k)
            if ((branch[ek] == "ExistingHeadStem" && phase[k] != 7) ||
                    (branch[ek] == "Linked" && phase[k] != 14) ||
                    f["branch"] != branch[ek]) bad++
            if (branch[ek] == "ExistingHeadStem" &&
                    (f["insertionReturned"] != "NotRead" || f["callbackCompleted"] != "false")) bad++
            if (branch[ek] == "Linked" &&
                    (f["insertionReturned"] != "true" || f["callbackCompleted"] != "true")) bad++
            phase[k] = 15
        } else if (label == "stemsbeamvlinkheadlinksremainder") {
            if ((phase[k] != 2 && phase[k] != 15) || f["comparisonEvaluated"] != "true" ||
                    f["splitBody"] != "CommentOnly" || f["splitCalls"] != 0 ||
                    f["returnedTrue"] != "true") bad++
            phase[k] = 16
        } else if (label == "stemsbeamvlinkheadlinksresult") {
            if (phase[k] != 16 || f["headEntries"] != entryCount[k] ||
                    f["relationsInserted"] + f["duplicateEntries"] != entryCount[k] ||
                    f["sWriteCount"] != entryCount[k] || f["eventCount"] != nextEvent[k] ||
                    f["dirtyCascadeCount"] != f["headAbnormalChangeCount"] + f["stemAbnormalChangeCount"] ||
                    f["returnedTrue"] != "true" ||
                    f["terminal"] != "ReturnedTrueBeforeOuterBLinkerAssignment") bad++
            dirtyCascades += f["dirtyCascadeCount"] + 0
            sheetEdits += f["sheetEditMutationCount"] + 0
            phase[k] = 17
        } else if (label == "stemsbeamvlinkheadlinksdeltaguard") {
            if (phase[k] != 17 || f["splitCalls"] != 0 ||
                    f["outerBLinkerAssignmentRead"] != "false" ||
                    f["stopBeforeOuterBLinkerAssignment"] != "true") bad++
            phase[k] = 18
        } else if (label == "stemsbeamvlinkheadlinkssummary") {
            if (phase[k] != 18 || f["headEntries"] != entryCount[k] ||
                    f["sWrites"] != entryCount[k] || f["returnedTrue"] != "true" ||
                    f["terminal"] != "ReturnedTrueBeforeOuterBLinkerAssignment") bad++
            phase[k] = 19
            transactions++
            realEvents += nextEvent[k]
            real++
            realSystem[f["system"]]++
        }
        next
    }
    /^#/ { next }
    { if (NF != 0) bad++ }
    END {
        expectedSyntheticOrder = "Duplicate,SmallHeadConsistency,NullSideExtension," \
                "ManualNoChord,ManualChordRewire,PreInsertThrow,CallbackThrow"
        if (page != 1 || real != systems || supported != 2 || envelope != 5 ||
                transactions != real + supported + envelope ||
                syntheticOrder != expectedSyntheticOrder) bad++
        for (i = 1; i <= systems; i++) if (realSystem[i] != 1) bad++
        for (k in seenKey) {
            if (phase[k] != 19 || entryCount[k] != expectedEntries[k] ||
                    family[k SUBSEP "stemsbeamvlinkheadlinkspredecessor"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkheadlinksbaseline"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkheadlinksremainder"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkheadlinksresult"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkheadlinksdeltaguard"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkheadlinkssummary"] != 1) bad++
        }
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n",
                bad, real, supported, envelope, transactions, headEntries,
                duplicateEntries, insertedEntries, sWrites, sValueChanges,
                consistencyWrites, headChanges, stemChanges, dirtyCascades, sheetEdits,
                realEvents, supplementalEvents, isolatedGraphDelta, manualCases, chordRewires,
                throws
    }
' "$pass_one")
IFS=: read -r census_bad real_transactions supported_cases envelope_cases transaction_rows \
    head_entries duplicate_entries relations_inserted s_writes s_value_changes \
    consistency_writes head_abnormal_changes stem_abnormal_changes dirty_cascades \
    sheet_edit_mutations real_events isolated_events isolated_graph_delta manual_cases \
    chord_rewires throw_cases <<EOF
$census
EOF
if [ "$census_bad" -ne 0 ]; then
    echo "invalid $page_key head-links census" >&2
    exit 1
fi

raw_pass_sha256=$(sha256_file "$pass_one")
raw_pass_lines=$(wc -l < "$pass_one" | tr -d ' ')
raw_pass_bytes=$(wc -c < "$pass_one" | tr -d ' ')
row_counts=""
for family in page predecessor baseline headentry slinkerwrite sourceoutgoing pairrelation \
        pairscan consistency edge headincident headincidentscan stemincident stemincidentscan \
        callback entryresult remainder result deltaguard summary syntheticcase syntheticevent \
        syntheticguard; do
    label=stemsbeamvlinkheadlinks$family
    count=$(awk -v label="$label" '$1 == label { count++ } END { print count + 0 }' "$pass_one")
    if [ -n "$row_counts" ]; then row_counts="$row_counts,"; fi
    row_counts="$row_counts$label:$count"
done

probe_source_sha256=$probe_source_sha256_before
runner_source_sha256=$runner_source_sha256_before
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
append_source_hash stemLinkerSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/StemLinker.java
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
append_source_hash smallBeamInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/SmallBeamInter.java
append_source_hash beamBeamRelationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/BeamBeamRelation.java
append_source_hash sheetStubSourceSha256 app/src/main/java/org/audiveris/omr/sheet/SheetStub.java
append_source_hash bookSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Book.java
append_source_hash horizontalSideSourceSha256 app/src/main/java/org/audiveris/omr/util/HorizontalSide.java
append_source_hash verticalSideSourceSha256 app/src/main/java/org/audiveris/omr/util/VerticalSide.java
append_source_hash stemBuilderSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java
append_source_hash stemItemSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/StemItem.java
append_source_hash stemsRetrieverSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java
append_source_hash beamGroupInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/BeamGroupInter.java
append_source_hash ensembleHelperSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/EnsembleHelper.java
append_source_hash containmentSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/Containment.java
append_source_hash abstractStemConnectionSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/AbstractStemConnection.java
append_source_hash beamPortionSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/BeamPortion.java
append_source_hash scaleSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Scale.java
append_source_hash skewSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Skew.java
append_source_hash staffSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Staff.java
append_source_hash staffLineSourceSha256 app/src/main/java/org/audiveris/omr/sheet/StaffLine.java
append_source_hash lineInfoSourceSha256 app/src/main/java/org/audiveris/omr/sheet/grid/LineInfo.java
append_source_hash lineUtilSourceSha256 app/src/main/java/org/audiveris/omr/math/LineUtil.java
append_source_hash headLinkerSourceSha256 app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java
append_source_hash headInterSourceSha256 app/src/main/java/org/audiveris/omr/sig/inter/HeadInter.java
append_source_hash headStemRelationSourceSha256 app/src/main/java/org/audiveris/omr/sig/relation/HeadStemRelation.java
append_source_hash partSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Part.java
append_source_hash gradleSourceSha256 app/build.gradle

cat "$pass_one"
isolated_cases=$((supported_cases + envelope_cases))
printf 'stemsbeamvlinkheadlinkspagesummary %s systems %s realTransactions %s supportedSyntheticCases %s envelopeCases %s isolatedCases %s totalTransactions %s headEntries %s duplicateEntries %s relationsInserted %s sWrites %s sValueChanges %s consistencyWrites %s headAbnormalChanges %s stemAbnormalChanges %s dirtyCascades %s sheetEditMutations %s realEvents %s isolatedEvents %s isolatedGraphDelta %s isolatedThrows %s isolatedManualCases %s chordRewires %s stopBeforeOuterBLinkerAssignment true\n' \
    "$page_file#1" "$expected_systems" "$real_transactions" "$supported_cases" \
    "$envelope_cases" "$isolated_cases" "$transaction_rows" "$head_entries" "$duplicate_entries" \
    "$relations_inserted" "$s_writes" "$s_value_changes" "$consistency_writes" \
    "$head_abnormal_changes" "$stem_abnormal_changes" "$dirty_cascades" \
    "$sheet_edit_mutations" "$real_events" "$isolated_events" "$isolated_graph_delta" \
    "$throw_cases" "$manual_cases" "$chord_rewires"
runtime_processes=$((2 * expected_systems))
total_processes=$((runtime_processes + 1))
printf 'stemsbeamvlinkheadlinkscorpussummary schema stems-beam-vlink-head-links-v1 mode %s pages 1 pageRefs %s rowCounts %s pageInputSha256 %s probeSourceSha256 %s runnerSourceSha256 %s effectiveClasspathSha256 %s jdkReleaseSha256 %s javaExecutableSha256 %s javaJpegLibrarySha256 %s javaModulesSha256 %s javaVmLibrarySha256 %s javaAwtLibrarySha256 %s javaAwtLwawtLibrarySha256 %s javaArchitecture %s javaRuntimeVersion %s javaVmVariant %s javaImageType %s beamLinkerClassSha256 %s bLinkerClassSha256 %s vLinkerClassSha256 %s stemLinkerClassSha256 %s headLinkerClassSha256 %s sLinkerClassSha256 %s cLinkerClassSha256 %s headInterClassSha256 %s stemInterClassSha256 %s headStemRelationClassSha256 %s headChordInterClassSha256 %s chordStemRelationClassSha256 %s partClassSha256 %s sigraphClassSha256 %s%s jgraphtCoreVersion 1.5.2 jgraphtCoreJarSha256 %s schedulerFixtureSha256 %s expandFixtureSha256 %s createStemFixtureSha256 %s reuseCheckFixtureSha256 %s baseApplyFixtureSha256 %s baseApplyManifestSha256 %s bLinkerFlagFixtureSha256 %s bLinkerFlagManifestSha256 %s siblingLinksFixtureSha256 %s siblingLinksManifestSha256 %s predecessorReplay FullBoundary16TypedReplayAndExactJavaRowJoin querySerialization UTF8ColonTokensLF-LazyNotReadLiteral emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshRunsPerPage 2 freshRunsByteIdentical true rawPassSha256 %s freshJvmPerSystem true compilerJavaProcesses 1 runtimeJavaProcessesPerPass %s runtimeJavaProcesses %s totalJavaProcesses %s maximumConcurrentJavaProcesses 1 concurrencyScope Boundary17RunnerLockedInvocation compilerJavaProcessReaped true runtimeJavaProcessesReaped true foregroundJavaProcessesOnly true backgroundJavaProcessesStarted 0 realTransactions %s supportedSyntheticCases %s envelopeCases %s isolatedCases %s totalTransactions %s headEntries %s duplicateEntries %s relationsInserted %s sWrites %s sValueChanges %s consistencyWrites %s headAbnormalChanges %s stemAbnormalChanges %s dirtyCascades %s sheetEditMutations %s realEvents %s isolatedEvents %s isolatedGraphDelta %s isolatedThrows %s isolatedManualCases %s chordRewires %s system1IsolatedBlock true supportedSyntheticEvidenceScope IsolatedJavaGateOnlyNotProductionEquivalent envelopeEvidenceScope IsolatedJavaGateOnlyNotProductionEquivalent stopBeforeOuterBLinkerAssignment true\n' \
    "$page_key" "$page_file#1" "$row_counts" "$page_input_sha256_before" \
    "$probe_source_sha256" "$runner_source_sha256" "$effective_classpath_sha256_before" \
    "$jdk_release_sha256_before" "$java_executable_sha256_before" \
    "$java_jpeg_library_sha256_before" "$java_modules_sha256_before" \
    "$java_vm_library_sha256_before" "$java_awt_library_sha256_before" \
    "$java_awt_lwawt_library_sha256_before" "$java_architecture" "$java_runtime_version" \
    "$java_vm_variant" "$java_image_type" "$beam_linker_class_sha256_before" \
    "$b_linker_class_sha256_before" "$v_linker_class_sha256_before" \
    "$stem_linker_class_sha256_before" "$head_linker_class_sha256_before" \
    "$s_linker_class_sha256_before" "$c_linker_class_sha256_before" \
    "$head_inter_class_sha256_before" "$stem_inter_class_sha256_before" \
    "$head_stem_relation_class_sha256_before" "$head_chord_inter_class_sha256_before" \
    "$chord_stem_relation_class_sha256_before" "$part_class_sha256_before" \
    "$sigraph_class_sha256_before" "$source_hashes" "$jgrapht_sha256" \
    "$scheduler_sha256" "$expand_sha256" "$create_stem_sha256" \
    "$reuse_check_sha256" "$base_apply_sha256" "$base_apply_manifest_sha256" \
    "$b_linker_flag_sha256" "$b_linker_flag_manifest_sha256" \
    "$sibling_links_sha256" "$sibling_links_manifest_sha256" \
    "$raw_pass_sha256" "$raw_pass_lines" "$raw_pass_bytes" "$raw_pass_sha256" \
    "$expected_systems" "$runtime_processes" "$total_processes" \
    "$real_transactions" "$supported_cases" "$envelope_cases" "$isolated_cases" \
    "$transaction_rows" \
    "$head_entries" "$duplicate_entries" "$relations_inserted" "$s_writes" \
    "$s_value_changes" "$consistency_writes" "$head_abnormal_changes" \
    "$stem_abnormal_changes" "$dirty_cascades" "$sheet_edit_mutations" \
    "$real_events" "$isolated_events" "$isolated_graph_delta" "$throw_cases" \
    "$manual_cases" "$chord_rewires"
