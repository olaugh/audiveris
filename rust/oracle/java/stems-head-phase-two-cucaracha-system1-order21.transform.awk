# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget queue-19 instrumentation to queue 21 / Java Inter 1077.
{
    gsub("1069", "1077")
    gsub("cucarachas1q19", "cucarachas1q21")
    print
}
