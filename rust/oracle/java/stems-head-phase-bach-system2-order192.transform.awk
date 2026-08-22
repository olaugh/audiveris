# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-192 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order192", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 192", line)
    gsub(/queue 183/, "queue 192", line)
    gsub(/queue-183/, "queue-192", line)
    gsub(/q183/, "q192", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 173 || sigOrdinals.get(head) != 160", line)
    print line
}
