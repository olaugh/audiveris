# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-198 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order198", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 198", line)
    gsub(/queue 183/, "queue 198", line)
    gsub(/queue-183/, "queue-198", line)
    gsub(/q183/, "q198", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 50 || sigOrdinals.get(head) != 194", line)
    print line
}
