# SPDX-License-Identifier: AGPL-3.0-or-later
# Add a bounded phase-two x113 C-link envelope to Java's production HeadLinker.

function emit_helpers() {
    print "    private static String rustHex (double value)"
    print "    {"
    print "        return Long.toHexString(Double.doubleToRawLongBits(value));"
    print "    }"
    print ""
    print "    private static String rustPoint (Point2D point)"
    print "    {"
    print "        return point == null ? \"-\" : rustHex(point.getX()) + \":\" + rustHex(point.getY());"
    print "    }"
    print ""
    print "    private static String rustRectangle (Rectangle box)"
    print "    {"
    print "        return box == null ? \"-\" : box.x + \":\" + box.y + \":\" + box.width + \":\" + box.height;"
    print "    }"
    print ""
    print "    private static String rustGlyph (Glyph glyph)"
    print "    {"
    print "        return glyph == null ? \"-\" : \"id\" + glyph.getId() + \":\""
    print "                + rustRectangle(glyph.getBounds()) + \":weight\" + glyph.getWeight();"
    print "    }"
    print ""
    print "    private static String rustHeadStem (HeadStemRelation relation)"
    print "    {"
    print "        return \"grade\" + rustHex(relation.getGrade())"
    print "                + \":dx\" + rustHex(relation.getDx())"
    print "                + \":dy\" + rustHex(relation.getDy())"
    print "                + \":extension\" + rustPoint(relation.getExtensionPoint())"
    print "                + \":headSide\" + relation.getHeadSide()"
    print "                + \":consistency\" + rustHex(relation.getConsistency());"
    print "    }"
    print ""
    print "    private static String rustRelations (Map<StemLinker, Relation> relations)"
    print "    {"
    print "        final List<String> rows = new ArrayList<>();"
    print "        for (Entry<StemLinker, Relation> entry : relations.entrySet()) {"
    print "            final Relation relation = entry.getValue();"
    print "            rows.add(entry.getKey().getId() + \":\""
    print "                    + (relation instanceof HeadStemRelation hs ? rustHeadStem(hs)"
    print "                            : relation.getClass().getSimpleName()));"
    print "        }"
    print "        return rows.toString().replace(\" \", \"\");"
    print "    }"
    print ""
    print "    private static String rustGlyphs (Set<Glyph> glyphs)"
    print "    {"
    print "        final List<String> rows = new ArrayList<>();"
    print "        for (Glyph glyph : glyphs) rows.add(rustGlyph(glyph));"
    print "        return rows.toString().replace(\" \", \"\");"
    print "    }"
    print ""
    print "    private static String rustStem (StemInter stem)"
    print "    {"
    print "        if (stem == null) return \"-\";"
    print "        final Line2D median = stem.getMedian();"
    print "        return \"id\" + stem.getId() + \":glyph\" + rustGlyph(stem.getGlyph())"
    print "                + \":grade\" + rustHex(stem.getGrade())"
    print "                + \":bounds\" + rustRectangle(stem.getBounds())"
    print "                + \":median\" + rustPoint(median.getP1()) + \":\" + rustPoint(median.getP2())"
    print "                + \":width\" + rustHex(stem.getWidth());"
    print "    }"
    print ""
}

function emit_frontier() {
    print "                final boolean rustAudit = append && head.getId() == 1826"
    print "                        && hSide == HorizontalSide.RIGHT && vSide == VerticalSide.TOP;"
    print "                final int rustVerticesBefore = system.getSig().vertexSet().size();"
    print "                final int rustEdgesBefore = system.getSig().edgeSet().size();"
    print "                final int rustAllocatorBefore = system.getSheet().getPersistentIdGenerator().get();"
    print "                final Glyph rustCandidate = !rustAudit ? null : glyphs.size() == 1"
    print "                        ? glyphs.iterator().next() : GlyphFactory.buildGlyph(glyphs);"
    print "                final StemInter rustExistingStem = rustAudit"
    print "                        ? retriever.getStemInter(rustCandidate) : null;"
    print "                if (rustAudit) {"
    print "                    System.out.printf("
    print "                            \"stemsheadphase2x113frontier headInterId %d corner %s \""
    print "                                    + \"hSide %s vSide %s refPt %s yDir %d \""
    print "                                    + \"minTail %d bestTail %d yHard %s ySoft %s \""
    print "                                    + \"lastIndex %d maxIndex %d relations %d relationRows %s \""
    print "                                    + \"glyphs %d selected %s candidate %s candidateIdBefore %d \""
    print "                                    + \"existingStem %s verticesBefore %d edgesBefore %d \""
    print "                                    + \"allocatorBefore %d terminal ReadyForHeadCreateStem%n\","
    print "                            head.getId(), cName(), hSide, vSide, rustPoint(refPt), yDir,"
    print "                            params.minStemTailLg, params.bestStemTailLg, rustHex(yHard),"
    print "                            rustHex(ySoft), lastIndex, sb.maxIndex(), relations.size(),"
    print "                            rustRelations(relations), glyphs.size(), rustGlyphs(glyphs),"
    print "                            rustGlyph(rustCandidate), rustCandidate.getId(),"
    print "                            rustStem(rustExistingStem), rustVerticesBefore, rustEdgesBefore,"
    print "                            rustAllocatorBefore);"
    print "                }"
}

function emit_result() {
    print "                if (rustAudit) {"
    print "                    final HeadStemRelation applied = (HeadStemRelation) sig.getRelation("
    print "                            head, stem, HeadStemRelation.class);"
    print "                    System.out.printf("
    print "                            \"stemsheadphase2x113result headInterId %d linkedStem %s \""
    print "                                    + \"reusedExisting %s applied %s relationRows %s \""
    print "                                    + \"verticesBefore %d verticesAfter %d edgesBefore %d \""
    print "                                    + \"edgesAfter %d allocatorBefore %d allocatorAfter %d \""
    print "                                    + \"terminal ReturnedHeadCLinkTransaction%n\","
    print "                            head.getId(), rustStem(stem), stem == rustExistingStem,"
    print "                            rustHeadStem(applied), rustRelations(relations),"
    print "                            rustVerticesBefore, sig.vertexSet().size(), rustEdgesBefore,"
    print "                            sig.edgeSet().size(), rustAllocatorBefore,"
    print "                            system.getSheet().getPersistentIdGenerator().get());"
    print "                }"
}

{
    if (index($0, "import org.audiveris.omr.glyph.Glyph;") != 0) {
        print
        print "import org.audiveris.omr.glyph.GlyphFactory;"
        next
    }
    if (index($0, "//~ Inner Classes") != 0) {
        emit_helpers()
        print
        next
    }
    if (index($0, "public boolean link (int stemProfile,") != 0) {
        in_c_link = 1
    }
    print
    if (in_c_link && index($0, "final int lastIndex = expand(") != 0) {
        in_expand_call = 1
    } else if (in_c_link && in_expand_call && index($0, "glyphs);") != 0) {
        emit_frontier()
        in_expand_call = 0
    }
    if (in_c_link && index($0, "// At this point, we have successfully linked") != 0) {
        emit_result()
        in_c_link = 0
    }
}
