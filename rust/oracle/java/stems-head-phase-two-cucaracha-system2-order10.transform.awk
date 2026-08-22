# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget system-2 queue-9 instrumentation to queue 10 / Java Inter 1392.
{
    gsub("1400", "1392")
    gsub("cucarachas2q9", "cucarachas2q10")
    print
}
