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
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkBLinkerFlagProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlink-b-linker-flag.sh"

case "$page_key" in
    chula) page_file=chula.png; expected_systems=3 ;;
    allegretto) page_file=allegretto.png; expected_systems=3 ;;
    batuque) page_file=batuque.png; expected_systems=3 ;;
    carmen) page_file=carmen.png; expected_systems=5 ;;
    cucaracha) page_file=cucaracha.png; expected_systems=3 ;;
    hove) page_file=hove.png; expected_systems=5 ;;
    zizi) page_file=zizi.png; expected_systems=2 ;;
    BachInvention5) page_file=BachInvention5.jpg; expected_systems=6 ;;
    *) echo "unknown beam VLink B-linker-flag page key: $page_key" >&2; exit 2 ;;
esac

scheduler_fixture="$repo_root/rust/oracle/stems-beam-scheduler-$page_key.txt"
expand_fixture="$repo_root/rust/oracle/stems-beam-expand-$page_key.txt"
create_stem_fixture="$repo_root/rust/oracle/stems-beam-create-stem-$page_key.txt"
reuse_check_fixture="$repo_root/rust/oracle/stems-beam-vlink-reuse-check-$page_key.txt"
base_apply_fixture="$repo_root/rust/oracle/stems-beam-vlink-base-apply-$page_key.txt"
base_apply_manifest="$repo_root/rust/oracle/stems-beam-vlink-base-apply-manifest.txt"
base_apply_probe="$repo_root/rust/oracle/java/StemsBeamVLinkBaseApplyProbe.java"
base_apply_runner="$repo_root/rust/oracle/java/run-stems-beam-vlink-base-apply.sh"
for required in "$probe_cp_file" "$scheduler_fixture" "$expand_fixture" \
        "$create_stem_fixture" "$reuse_check_fixture" "$base_apply_fixture" \
        "$base_apply_manifest" "$base_apply_probe" "$base_apply_runner"; do
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
        [ "$base_page" != "$page_file#1" ]; then
    echo "boundary-15 predecessor provenance mismatch for $page_key" >&2
    exit 1
fi

hash_set=$(printf '%s\n' "$scheduler_sha256" "$expand_sha256" "$create_stem_sha256" \
        "$reuse_check_sha256" "$base_apply_sha256" | sort -u | wc -l | tr -d ' ')
if [ "$hash_set" -ne 5 ]; then
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

lock_dir=/private/tmp/stems-beam-vlink-b-linker-flag.lock
if ! mkdir "$lock_dir"; then
    echo "another B-linker-flag oracle runner owns $lock_dir" >&2
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
probe_classes=$(mktemp -d /private/tmp/stems-beam-vlink-b-linker-flag-classes.XXXXXX)
pass_one=$(mktemp /private/tmp/stems-beam-vlink-b-linker-flag-pass1.XXXXXX)
pass_two=$(mktemp /private/tmp/stems-beam-vlink-b-linker-flag-pass2.XXXXXX)

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
        [ "$stem_linker_class_sha256_before" != "$expected_stem_linker_class_sha256" ]; then
    echo "frozen Temurin image artifact drift" >&2
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
                org.audiveris.omr.rustport.StemsBeamVLinkBLinkerFlagProbe \
                --system "$system_id" \
                "$repo_root/data/examples/$page_file:1" \
                "$scheduler_fixture" "$expand_fixture" "$create_stem_fixture" \
                "$reuse_check_fixture" "$base_apply_fixture"
        )
        system_id=$((system_id + 1))
    done
}

# javac and every runtime JVM are foreground, awaited, and reaped. The runner never starts '&'.
# Therefore the maximum concurrent Java process count is exactly one.
run_fresh_pass > "$pass_one"
run_fresh_pass > "$pass_two"
if ! cmp -s "$pass_one" "$pass_two"; then
    echo "two fresh $page_key B-linker-flag passes are not byte-identical" >&2
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
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker.class")" \
            != "$beam_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker.class")" \
            != "$b_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/BeamLinker\$BLinker\$VLinker.class")" \
            != "$v_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/app/build/classes/java/main/org/audiveris/omr/sheet/stem/StemLinker.class")" \
            != "$stem_linker_class_sha256_before" ] || \
        [ "$(sha256_file "$repo_root/data/examples/$page_file")" != "$page_input_sha256_before" ] || \
        [ "$(sha256_file "$scheduler_fixture")" != "$scheduler_sha256" ] || \
        [ "$(sha256_file "$expand_fixture")" != "$expand_sha256" ] || \
        [ "$(sha256_file "$create_stem_fixture")" != "$create_stem_sha256" ] || \
        [ "$(sha256_file "$reuse_check_fixture")" != "$reuse_check_sha256" ] || \
        [ "$(sha256_file "$base_apply_fixture")" != "$base_apply_sha256" ] || \
        [ "$(sha256_file "$base_apply_manifest")" != "$base_apply_manifest_sha256" ] || \
        [ "$(sha256_file "$jgrapht_jar")" != "$jgrapht_sha256" ]; then
    echo "B-linker-flag input changed during the two-pass run" >&2
    exit 1
fi

schema_count=$(awk '$0 == "# schema: stems-beam-vlink-b-linker-flag-v1" { count++ } END { print count + 0 }' "$pass_one")
stale_count=$(awk '/^stemsbeamvlinkbaseapply/ { count++ } END { print count + 0 }' "$pass_one")
if [ "$schema_count" -ne 1 ] || [ "$stale_count" -ne 0 ]; then
    echo "B-linker-flag schema header/output family drift" >&2
    exit 1
fi

census=$(awk -v systems="$expected_systems" -v pageRef="$page_file#1" '
    function clear_fields( key) { for (key in f) delete f[key] }
    function fields( start, i) {
        clear_fields()
        for (i = start; i <= NF; i += 2) f[$i] = $(i + 1)
    }
    function key() { return f["system"] SUBSEP f["plan"] SUBSEP f["scope"] SUBSEP f["case"] }
    function rank(label) {
        if (label == "stemsbeamvlinkblinkerflagpredecessor") return 1
        if (label == "stemsbeamvlinkblinkerflagbaseline") return 2
        if (label == "stemsbeamvlinkblinkerflagbentry") return 3
        if (label == "stemsbeamvlinkblinkerflagtarget") return 4
        if (label == "stemsbeamvlinkblinkerflagobservation") return 5
        if (label == "stemsbeamvlinkblinkerflagwritetrace") return 6
        if (label == "stemsbeamvlinkblinkerflagresult") return 7
        if (label == "stemsbeamvlinkblinkerflagdeltaguard") return 8
        if (label == "stemsbeamvlinkblinkerflagsummary") return 9
        return 0
    }
    /^stemsbeamvlinkblinkerflagpage / {
        page++
        if ($2 != pageRef || $3 != "systems" || $4 != systems) bad++
        next
    }
    /^stemsbeamvlinkblinkerflag(predecessor|baseline|bentry|target|observation|writetrace|result|deltaguard|summary) / {
        fields(3)
        k = key()
        r = rank($1)
        seenKey[k] = 1
        scopeByKey[k] = f["scope"]
        caseByKey[k] = f["case"]
        if ($2 != pageRef || f["system"] !~ /^[1-9][0-9]*$/) bad++
        if (f["scope"] != "real" && f["scope"] != "synthetic") bad++
        if (f["scope"] == "real" && (f["case"] != "-" || f["system"] > systems)) bad++
        if (f["scope"] == "synthetic" && f["system"] != 1) bad++
        if (lastRank[k] > r) bad++
        lastRank[k] = r
        family[k SUBSEP $1]++
        if ($1 == "stemsbeamvlinkblinkerflagbaseline") {
            arenaExpected[k] = f["arenaEntries"] + 0
            if (f["targetMatches"] != 1 ||
                    f["noChild"] + f["child1"] + f["child2"] != f["arenaEntries"]) bad++
            if (f["scope"] == "real") {
                realArena += f["arenaEntries"]
                frozen += f["frozenEntries"]
                anchors += f["dynamicAnchors"]
                if (f["frozenEntries"] + f["dynamicAnchors"] != f["arenaEntries"] ||
                        f["linkedCountBefore"] != 0 || f["targetFrozenMatch"] != "true") bad++
            } else if (f["arenaEntries"] != 1 || f["frozenEntries"] != 0 ||
                    f["dynamicAnchors"] != 0 || f["targetFrozenMatch"] != "false") {
                bad++
            }
        } else if ($1 == "stemsbeamvlinkblinkerflagbentry") {
            if (f["arenaOrdinal"] != bentries[k]) bad++
            if (f["scope"] == "synthetic" && f["origin"] != "IsolatedSyntheticCell") bad++
            bentries[k]++
        } else if ($1 == "stemsbeamvlinkblinkerflagobservation") {
            if (f["observationOrdinal"] != observations[k]) bad++
            observations[k]++
        } else if ($1 == "stemsbeamvlinkblinkerflagtarget") {
            children[k] = (f["orderedChildAliases"] == "-") ? 0 : split(f["orderedChildAliases"], tmp, ",")
            if (f["receiverRuntimeClass"] !~ /BeamLinker\$BLinker$/ ||
                    f["setterDeclaringClass"] !~ /BeamLinker\$BLinker$/ ||
                    f["sameIdentity"] != "true" || f["sLinkerRead"] != "NotRead") bad++
        } else if ($1 == "stemsbeamvlinkblinkerflagwritetrace") {
            if (f["requested"] != "true" || f["assignmentAttempted"] != "true" ||
                    f["assignmentCompleted"] != "true" || f["writeCount"] != 1 ||
                    f["setterBodyCallbacks"] != 0 || f["setterBodyListeners"] != 0 ||
                    f["setterBodyAllocations"] != 0 ||
                    f["setterBodyValidationReads"] != 0 || f["throwStage"] != "-") bad++
        } else if ($1 == "stemsbeamvlinkblinkerflagresult") {
            if (f["terminal"] != "ReadyBeforeSiblingBeamLinks" ||
                    f["linkSiblingsCalled"] != "false" || f["supportGradeRead"] != "false") bad++
        } else if ($1 == "stemsbeamvlinkblinkerflagdeltaguard") {
            if (f["stopBeforeSiblingBeamLinks"] != "true" ||
                    f["stopBeforeHeadRelationLoop"] != "true" ||
                    f["noCellOtherThanSelectedChanged"] != "true") bad++
        } else if ($1 == "stemsbeamvlinkblinkerflagsummary") {
            summaries++
            if (f["scope"] == "real") {
                real++
                realSystem[f["system"]]++
                if (f["case"] != "-" || f["transition"] != "FalseToTrue" ||
                        f["valueChangeCount"] != 1 || f["applyReturn"] != "true") bad++
            } else {
                synthetic++
                order = order (order == "" ? "" : ",") f["case"]
                if (f["case"] == "IdempotentTrue") {
                    if (f["transition"] != "TrueToTrueIdempotent" || f["valueChangeCount"] != 0) bad++
                    idempotent++
                } else {
                    if (f["transition"] != "FalseToTrue" || f["valueChangeCount"] != 1) bad++
                }
                if ((f["case"] == "ApplyReturnFalse") != (f["applyReturn"] == "false")) bad++
            }
        }
        next
    }
    /^stemsbeamvlinkblinkerflag/ { bad++; next }
    END {
        expectedOrder = "InitialFalse,IdempotentTrue,ApplyReturnFalse,TwoChildrenShared"
        if (page != 1 || real != systems || synthetic != 4 || idempotent != 1 ||
                summaries != systems + 4 || order != expectedOrder) bad++
        for (i = 1; i <= systems; i++) if (realSystem[i] != 1) bad++
        for (k in seenKey) if (!(k in arenaExpected)) bad++
        for (k in arenaExpected) {
            if (bentries[k] != arenaExpected[k] || observations[k] != children[k] + 1) bad++
            if (scopeByKey[k] == "synthetic") {
                expectedChildren = caseByKey[k] == "TwoChildrenShared" ? 2 : 1
                if (children[k] != expectedChildren || observations[k] != expectedChildren + 1) bad++
            }
            if (family[k SUBSEP "stemsbeamvlinkblinkerflagpredecessor"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagbaseline"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagtarget"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagwritetrace"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagresult"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagdeltaguard"] != 1 ||
                    family[k SUBSEP "stemsbeamvlinkblinkerflagsummary"] != 1) bad++
        }
        printf "%d:%d:%d:%d:%d:%d:%d\n", bad, real, synthetic, summaries,
                realArena, frozen, anchors
    }
' "$pass_one")
IFS=: read -r census_bad real_transactions synthetic_cases transaction_rows \
    real_arena_entries frozen_entries dynamic_anchors <<EOF
$census
EOF
if [ "$census_bad" -ne 0 ]; then
    echo "invalid $page_key B-linker-flag census" >&2
    exit 1
fi

raw_pass_sha256=$(sha256_file "$pass_one")
raw_pass_lines=$(wc -l < "$pass_one" | tr -d ' ')
raw_pass_bytes=$(wc -c < "$pass_one" | tr -d ' ')
row_counts=""
for family in page predecessor baseline bentry target observation writetrace result deltaguard summary; do
    label=stemsbeamvlinkblinkerflag$family
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
append_source_hash sheetStubSourceSha256 app/src/main/java/org/audiveris/omr/sheet/SheetStub.java
append_source_hash bookSourceSha256 app/src/main/java/org/audiveris/omr/sheet/Book.java
append_source_hash horizontalSideSourceSha256 app/src/main/java/org/audiveris/omr/util/HorizontalSide.java
append_source_hash verticalSideSourceSha256 app/src/main/java/org/audiveris/omr/util/VerticalSide.java
append_source_hash gradleSourceSha256 app/build.gradle

cat "$pass_one"
printf 'stemsbeamvlinkblinkerflagpagesummary %s systems %s realTransactions %s syntheticCases %s totalTransactions %s realFalseToTrue %s realIdempotent 0 syntheticFalseToTrue 3 syntheticIdempotent 1 realArenaEntries %s frozenEntries %s dynamicAnchors %s isolatedExactClassCells 4 applyReturnFalseCases 1 stopBeforeSiblingBeamLinks true\n' \
    "$page_file#1" "$expected_systems" "$real_transactions" "$synthetic_cases" \
    "$transaction_rows" "$expected_systems" "$real_arena_entries" "$frozen_entries" \
    "$dynamic_anchors"
runtime_processes=$((2 * expected_systems))
total_processes=$((runtime_processes + 1))
printf 'stemsbeamvlinkblinkerflagcorpussummary schema stems-beam-vlink-b-linker-flag-v1 mode %s pages 1 pageRefs %s rowCounts %s pageInputSha256 %s probeSourceSha256 %s runnerSourceSha256 %s effectiveClasspathSha256 %s jdkReleaseSha256 %s javaExecutableSha256 %s javaJpegLibrarySha256 %s javaModulesSha256 %s javaVmLibrarySha256 %s javaAwtLibrarySha256 %s javaAwtLwawtLibrarySha256 %s javaArchitecture %s javaRuntimeVersion %s javaVmVariant %s javaImageType %s beamLinkerClassSha256 %s bLinkerClassSha256 %s vLinkerClassSha256 %s stemLinkerClassSha256 %s%s jgraphtCoreVersion 1.5.2 jgraphtCoreJarSha256 %s schedulerFixtureSha256 %s expandFixtureSha256 %s createStemFixtureSha256 %s reuseCheckFixtureSha256 %s baseApplyFixtureSha256 %s baseApplyManifestSha256 %s predecessorSwapNegative true baseEvidenceMeaning OrderedFourRowShaBundleNotTypedState emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s freshRunsPerPage 2 freshRunsByteIdentical true rawPassSha256 %s freshJvmPerSystem true compilerJavaProcesses 1 runtimeJavaProcessesPerPass %s runtimeJavaProcesses %s totalJavaProcesses %s maximumConcurrentJavaProcesses 1 concurrencyScope Boundary15RunnerLockedInvocation compilerJavaProcessReaped true runtimeJavaProcessesReaped true foregroundJavaProcessesOnly true backgroundJavaProcessesStarted 0 dynamicAllBArenaGuardOnly true stopBeforeSiblingBeamLinks true\n' \
    "$page_key" "$page_file#1" "$row_counts" "$page_input_sha256_before" \
    "$probe_source_sha256" "$runner_source_sha256" "$effective_classpath_sha256_before" \
    "$jdk_release_sha256_before" "$java_executable_sha256_before" \
    "$java_jpeg_library_sha256_before" "$java_modules_sha256_before" \
    "$java_vm_library_sha256_before" "$java_awt_library_sha256_before" \
    "$java_awt_lwawt_library_sha256_before" "$java_architecture" "$java_runtime_version" \
    "$java_vm_variant" "$java_image_type" "$beam_linker_class_sha256_before" \
    "$b_linker_class_sha256_before" "$v_linker_class_sha256_before" \
    "$stem_linker_class_sha256_before" "$source_hashes" "$jgrapht_sha256" \
    "$scheduler_sha256" "$expand_sha256" "$create_stem_sha256" \
    "$reuse_check_sha256" "$base_apply_sha256" "$base_apply_manifest_sha256" \
    "$raw_pass_sha256" "$raw_pass_lines" "$raw_pass_bytes" "$raw_pass_sha256" \
    "$expected_systems" "$runtime_processes" "$total_processes"
