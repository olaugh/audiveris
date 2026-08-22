# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-188 multi-head existing-stem C-link probe from the frozen source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order188CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 188", line)
    gsub(/queue-182/, "queue-188", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 47 || sigOrdinals.get(head) != 57", line)
    gsub(/VerticalSide.BOTTOM/, "VerticalSide.TOP", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:47:LEFT:TOP", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q188", line)
    print line
}
