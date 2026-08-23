# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order209", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 209", line)
    gsub(/queue 183/, "queue 209", line)
    gsub(/queue-183/, "queue-209", line)
    gsub(/q183/, "q209", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 54 || sigOrdinals.get(head) != 59", line)
    print line
}
