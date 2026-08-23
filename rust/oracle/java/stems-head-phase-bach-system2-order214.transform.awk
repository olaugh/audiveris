# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the terminal queue-214 existing-stem C-link probe from the frozen
# multi-beam source while preserving the full Java lifecycle before the target.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order214CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 214", line)
    gsub(/queue-182/, "queue-214", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 90 || sigOrdinals.get(head) != 134", line)
    gsub(/get\(HorizontalSide.LEFT\)/, "get(HorizontalSide.RIGHT)", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:90:RIGHT:BOTTOM", line)
    gsub(/glyphRows.add\("id" \+ glyph.getId\(\) \+ ":" \+ glyphToken\(glyph\)\);/,
            "glyphRows.add((glyphToken(glyph).equals(glyphToken(candidate)) ? \"candidateGlyph\" : \"supportGlyph\") + \":\" + glyphToken(glyph));", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q214", line)
    if (line ~ /HeadInter next = \(HeadInter\) heads.get\(order \+ 1\);/) {
        next
    }
    if (line ~ /nextHeadOrder %d nextHeadX %d nextHeadSig %d nextHeadInterId %d/) {
        line = "                            + \"queueExhausted true \""
    }
    if (line ~ /compact\(addedEdges\), compact\(addedStems\), compact\(linkerChanges\), order \+ 1,/) {
        sub(/, order \+ 1,$/, ");", line)
    }
    if (line ~ /xOrdinals.get\(next\), sigOrdinals.get\(next\), next.getId\(\)\);/) {
        next
    }
    print line
}
