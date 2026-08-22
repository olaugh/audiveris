# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-191 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order191", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 191", line)
    gsub(/queue 183/, "queue 191", line)
    gsub(/queue-183/, "queue-191", line)
    gsub(/q183/, "q191", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 150 || sigOrdinals.get(head) != 29", line)
    print line
}
