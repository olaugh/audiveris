# SPDX-License-Identifier: AGPL-3.0-or-later

function emit_loop() {
    print "        for (Inter inter : heads) {"
    print "            HeadInter head = (HeadInter) inter;"
    print "            boolean audit = system.getId() == 1 && xOrdinals.get(head) == 26;"
    print "            String sidesBefore = headSides(head);"
    print "            List<String> headDecisions = audit ? decisions(head) : List.of();"
    print "            List<String> incident = new ArrayList<>();"
    print "            List<String> closureWrites = new ArrayList<>();"
    print "            IdentityHashMap<HeadLinker.SLinker, Boolean> modeledClosed ="
    print "                    new IdentityHashMap<>();"
    print "            if (audit) {"
    print "                for (Inter candidate : xHeads) {"
    print "                    HeadInter candidateHead = (HeadInter) candidate;"
    print "                    for (HeadLinker.SLinker s : candidateHead.getLinker().getSLinkers().values()) {"
    print "                        modeledClosed.put(s, s.isClosed());"
    print "                    }"
    print "                }"
    print "            }"
    print "            IdentityHashMap<HeadLinker.SLinker, String> beforeSides = sideSnapshot(xHeads);"
    print "            int verticesBefore = sig.vertexSet().size();"
    print "            int edgesBefore = sig.edgeSet().size();"
    print "            int stemsBefore = ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size();"
    print "            int allocatorBefore = system.getSheet().getPersistentIdGenerator().get();"
    print "            boolean returned = head.getLinker().linkSides("
    print "                    Profiles.STRICT, system.getProfile(), undefs, false);"
    print "            if (!returned) unlinked.add(head);"
    print "            if (audit) {"
    print "                for (Relation relation : sig.getRelations(head, HeadStemRelation.class)) {"
    print "                    HeadStemRelation headStem = (HeadStemRelation) relation;"
    print "                    StemInter stem = (StemInter) sig.getOppositeInter(head, relation);"
    print "                    List<String> stemHeads = new ArrayList<>();"
    print "                    for (Relation stemRelation : sig.getRelations(stem, HeadStemRelation.class)) {"
    print "                        HeadInter stemHead = (HeadInter) sig.getOppositeInter(stem, stemRelation);"
    print "                        HeadStemRelation stemHeadRelation = (HeadStemRelation) stemRelation;"
    print "                        stemHeads.add(\"x\" + xOrdinals.get(stemHead) + \":sig\""
    print "                                + sigOrdinals.get(stemHead) + \":id\" + stemHead.getId()"
    print "                                + \":side\" + stemHeadRelation.getHeadSide());"
    print "                        if (stemHead != head) {"
    print "                            for (HorizontalSide side : HorizontalSide.values()) {"
    print "                                HeadLinker.SLinker s = stemHead.getLinker().getSLinkers().get(side);"
    print "                                boolean prior = modeledClosed.get(s);"
    print "                                closureWrites.add(\"x\" + xOrdinals.get(stemHead) + \":sig\""
    print "                                        + sigOrdinals.get(stemHead) + \":\" + side + \":\""
    print "                                        + prior + \"->true\");"
    print "                                modeledClosed.put(s, true);"
    print "                            }"
    print "                        }"
    print "                    }"
    print "                    incident.add(\"stem\" + stem.getId() + \":headSide\""
    print "                            + headStem.getHeadSide() + \":heads\" + compact(stemHeads));"
    print "                }"
    print "                long closedValueChanges = closureWrites.stream()"
    print "                        .filter(value -> value.endsWith(\"false->true\")).count();"
    print "                int order = heads.indexOf(head);"
    print "                HeadInter next = (HeadInter) heads.get(order + 1);"
    print "                System.out.printf("
    print "                        \"stemsheadziziorder34 page %s system %d headOrder %d headX %d \""
    print "                                + \"headSig %d headInterId %d grade %s sidesBefore %s \""
    print "                                + \"decisions %s incident %s returned %s sidesAfter %s \""
    print "                                + \"undefs %s closureWrites %s closedValueChanges %d \""
    print "                                + \"sideChanges %s sigVerticesBefore %d sigVerticesAfter %d \""
    print "                                + \"sigEdgesBefore %d sigEdgesAfter %d systemStemsBefore %d \""
    print "                                + \"systemStemsAfter %d allocatorBefore %d allocatorAfter %d \""
    print "                                + \"nextHeadOrder %d nextHeadX %d nextHeadSig %d \""
    print "                                + \"nextHeadInterId %d terminal ReturnedBeforeThirtySixthHead%n\","
    print "                        page, system.getId(), order, xOrdinals.get(head), sigOrdinals.get(head),"
    print "                        head.getId(), hex(head.getGrade()), sidesBefore, compact(headDecisions),"
    print "                        compact(incident), returned, headSides(head),"
    print "                        compact(undefs.get(head) == null ? List.of() : undefs.get(head)),"
    print "                        compact(closureWrites), closedValueChanges,"
    print "                        compact(sideChanges(xHeads, xOrdinals, sigOrdinals, beforeSides)),"
    print "                        verticesBefore, sig.vertexSet().size(), edgesBefore, sig.edgeSet().size(),"
    print "                        stemsBefore, ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(),"
    print "                        allocatorBefore, system.getSheet().getPersistentIdGenerator().get(),"
    print "                        order + 1, xOrdinals.get(next), sigOrdinals.get(next), next.getId());"
    print "            }"
    print "        }"
}

/import org\.audiveris\.omr\.sig\.inter\.Inters;/ {
    print
    print "import org.audiveris.omr.sig.inter.StemInter;"
    print "import org.audiveris.omr.sig.relation.HeadStemRelation;"
    print "import org.audiveris.omr.sig.relation.Relation;"
    next
}

$0 == "        List<HeadInter> unlinked = new ArrayList<>();" {
    in_phase_one = 1
    print
    next
}

in_phase_one && !skipping && $0 == "        for (Inter inter : heads) {" {
    emit_loop()
    skipping = 1
    in_phase_one = 0
    depth = 1
    next
}

skipping {
    opens = gsub(/\{/, "{")
    closes = gsub(/\}/, "}")
    depth += opens - closes
    if (depth == 0) skipping = 0
    next
}

{ print }
