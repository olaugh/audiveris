# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order213", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 213", line)
    gsub(/queue 183/, "queue 213", line)
    gsub(/queue-183/, "queue-213", line)
    gsub(/q183/, "q213", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 29 || sigOrdinals.get(head) != 92", line)
    print line
}
