# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order204", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 204", line)
    gsub(/queue 183/, "queue 204", line)
    gsub(/queue-183/, "queue-204", line)
    gsub(/q183/, "q204", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 43 || sigOrdinals.get(head) != 193", line)
    print line
}
