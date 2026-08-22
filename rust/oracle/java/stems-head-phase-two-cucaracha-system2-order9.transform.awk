# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget system-2 queue-8 instrumentation to queue 9 / Java Inter 1400.
{
    gsub("1388", "1400")
    gsub("cucarachas2q8", "cucarachas2q9")
    print
}
