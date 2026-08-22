# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget system-2 queue-16 instrumentation to system-3 queue 19 / Java Inter 1555.
{
    gsub("1394", "1555")
    gsub("cucarachas2q16", "cucarachas3q19")
    print
}
