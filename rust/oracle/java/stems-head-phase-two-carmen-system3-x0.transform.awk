# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated Carmen system-3 x1 C-link instrumentation to
# queue 3's x0 / Java Inter 2405 phase-two retry.
{
    gsub("head.getId[(][)] == 2505", "head.getId() == 2405")
    gsub("stemsheadphase2carmens3x1", "stemsheadphase2carmens3x0")
    print
}
