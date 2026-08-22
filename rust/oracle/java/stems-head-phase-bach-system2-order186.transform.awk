# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-186 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order186", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 186", line)
    gsub(/queue 183/, "queue 186", line)
    gsub(/queue-183/, "queue-186", line)
    gsub(/q183/, "q186", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 190 || sigOrdinals.get(head) != 214", line)
    print line
}
