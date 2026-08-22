# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order203", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 203", line)
    gsub(/queue 183/, "queue 203", line)
    gsub(/queue-183/, "queue-203", line)
    gsub(/q183/, "q203", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 125 || sigOrdinals.get(head) != 25", line)
    print line
}
