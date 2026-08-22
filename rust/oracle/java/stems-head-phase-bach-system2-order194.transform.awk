# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-194 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order194", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 194", line)
    gsub(/queue 183/, "queue 194", line)
    gsub(/queue-183/, "queue-194", line)
    gsub(/q183/, "q194", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 16 || sigOrdinals.get(head) != 184", line)
    print line
}
