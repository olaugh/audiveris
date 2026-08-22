# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-193 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order193", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 193", line)
    gsub(/queue 183/, "queue 193", line)
    gsub(/queue-183/, "queue-193", line)
    gsub(/q183/, "q193", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 27 || sigOrdinals.get(head) != 178", line)
    print line
}
