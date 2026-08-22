# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated Allegretto x14 C-link instrumentation to Carmen
# system 3's x1 / Java Inter 2505 phase-two retry.
{
    gsub("head.getId[(][)] == 1777", "head.getId() == 2505")
    gsub("stemsheadphase2x14", "stemsheadphase2carmens3x1")
    print
}
