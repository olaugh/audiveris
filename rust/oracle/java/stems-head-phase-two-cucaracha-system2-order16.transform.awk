# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget system-2 queue-10 instrumentation to queue 16 / Java Inter 1394.
{
    gsub("1392", "1394")
    gsub("cucarachas2q10", "cucarachas2q16")
    print
}
