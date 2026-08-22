# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget Cucaracha system-2 queue-16 instrumentation to Hove system-5 queue 1 / Java Inter 1721.
{
    gsub("1394", "1721")
    gsub("cucarachas2q16", "hoves5q1")
    print
}
