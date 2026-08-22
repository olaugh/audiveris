# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-196 existing-stem C-link probe from the frozen multi-beam source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order196CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 196", line)
    gsub(/queue-182/, "queue-196", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 111 || sigOrdinals.get(head) != 50", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:111:LEFT:BOTTOM", line)
    gsub(/glyphRows.add\("id" \+ glyph.getId\(\) \+ ":" \+ glyphToken\(glyph\)\);/,
            "glyphRows.add((glyphToken(glyph).equals(glyphToken(candidate)) ? \"candidateGlyph\" : \"supportGlyph\") + \":\" + glyphToken(glyph));", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q196", line)
    print line
}
