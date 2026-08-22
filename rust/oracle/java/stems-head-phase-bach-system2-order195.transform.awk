# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-195 rejected existing-stem C-link probe from the frozen source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order195CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 195", line)
    gsub(/queue-182/, "queue-195", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 98 || sigOrdinals.get(head) != 136", line)
    gsub(/VerticalSide.BOTTOM/, "VerticalSide.TOP", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:98:LEFT:TOP", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q195", line)
    print line
}
