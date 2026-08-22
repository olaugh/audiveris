# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-190 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order190", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 190", line)
    gsub(/queue 183/, "queue 190", line)
    gsub(/queue-183/, "queue-190", line)
    gsub(/q183/, "q190", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 65 || sigOrdinals.get(head) != 196", line)
    print line
}
