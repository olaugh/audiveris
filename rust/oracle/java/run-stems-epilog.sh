#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
audiveris_root=${AUDIVERIS_ROOT:-/Users/john/sources/aug04-rubigo/audiveris}
image=${1:-$audiveris_root/data/examples/chula.png}
tmp_dir=$(mktemp -d /private/tmp/stems-epilog.XXXXXX)
trap 'rm -rf "$tmp_dir"' EXIT

run_pass()
{
    "$audiveris_root/gradlew" --no-daemon \
        -I "$script_dir/stems-epilog.init.gradle" \
        -PrustPortRepo="$repo_root" \
        -PstemsEpilogImage="$image" \
        :app:stemsEpilogProbe | grep '^stemsepilog '
}

run_pass > "$tmp_dir/pass1"
run_pass > "$tmp_dir/pass2"
cmp "$tmp_dir/pass1" "$tmp_dir/pass2"
rows="$tmp_dir/pass1"
if [ "$(wc -l < "$rows" | tr -d ' ')" -ne 3 ] || \
        ! grep -Fq 'system 1 removedCount 4 removed [969, 971, 973, 975] beamHeadCount 123 beamHeadGradeSha256 bf3e6389a88830b4598dcd20c82225abb150ed7d9c9c6b9bec3e234d035f10fe contextualCount 259 contextualNull 0' "$rows" || \
        ! grep -Fq 'system 2 removedCount 3 removed [1018, 1075, 1077] beamHeadCount 109 beamHeadGradeSha256 556ae5783a9c9450f26da1086a06ba2c75786952f0c7ba42156d1358a2e463bf contextualCount 233 contextualNull 0' "$rows" || \
        ! grep -Fq 'system 3 removedCount 5 removed [1101, 1102, 1103, 1104, 1105] beamHeadCount 110 beamHeadGradeSha256 e4c0fd107782f3bd5bcab379aebca6793a2f9ed2fc881456be1bd0d941518b3e contextualCount 274 contextualNull 0' "$rows"; then
    echo "STEMS epilog contract differs" >&2
    cat "$rows" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/StemsEpilogProbe.java" | awk '{print $1}')
init_sha=$(shasum -a 256 "$script_dir/stems-epilog.init.gradle" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
out=/private/tmp/stems-epilog-audit.txt
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25) STEMS sheet-epilog evidence.'
    echo '# schema: stems-epilog-v1'
    cat "$rows"
    echo "stemsepilog summary schema stems-epilog-v1 rows 3 javaSourceCommit 07b5f9f33822960c3b70bbea85e26f24ada08963 probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha bodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope GenericFinalizeBeamsAndContextualization javaEvidence ReturnedAfterStemsStepEpilog"
} > "$out"
echo "wrote $out"
