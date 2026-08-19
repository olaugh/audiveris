        void emitHeadCLinkEnvelope (HeadInter head,
                                    HeadLinker linker,
                                    PersistentSnapshot before)
            throws Exception
        {
            final HeadLinker.SLinker side = linker.getSLinkers().get(HorizontalSide.LEFT);
            final HeadLinker.SLinker.CLinker corner = side.getCornerLinker(VerticalSide.BOTTOM);
            final Point2D refPt = (Point2D) C_REF_PT.get(corner);
            final int yDir = C_Y_DIR.getInt(corner);
            final int minTail = PARAMETERS_MIN_STEM_TAIL_LG.getInt(params);
            final int bestTail = PARAMETERS_BEST_STEM_TAIL_LG.getInt(params);
            final double yHard = refPt.getY() + (yDir * minTail);
            final double ySoft = refPt.getY() + (yDir * bestTail);
            final LinkedHashMap<StemLinker, Relation> planRelations = new LinkedHashMap<>();
            final LinkedHashSet<Glyph> planGlyphs = new LinkedHashSet<>();
            final int lastIndex = (Integer) C_EXPAND.invoke(
                    corner, yHard, ySoft, Profiles.STRICT, system.getProfile(),
                    planRelations, planGlyphs);
            final PersistentSnapshot expanded = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            before.assertOnlyLineChanged(expanded);
            final Glyph candidate = planGlyphs.size() == 1
                    ? planGlyphs.iterator().next() : GlyphFactory.buildGlyph(planGlyphs);
            final Glyph existing = before.glyphs.equalOriginal(candidate);
            final boolean existingActive = existing != null
                    && before.glyphs.active.containsKey(existing);
            final StemInter existingStem = before.systemStems.equalStem(candidate);
            final List<String> relationInputs = new ArrayList<>();
            for (Map.Entry<StemLinker, Relation> entry : planRelations.entrySet()) {
                relationInputs.add(linkerAlias(entry.getKey()) + ":" + relationState(entry.getValue()));
            }
            System.out.printf(
                    "stemsheadclinkfrontier %s system %d headOrder 21 headX %d headSig %d headInterId %d "
                            + "cAlias %s refPt %s yDir %d minTail %d bestTail %d yHard %s ySoft %s "
                            + "lastIndex %d maxIndex %d relations %d relationRows %s glyphs %d selected %s "
                            + "candidate %s candidateIdBefore %d existingGlyph %s existingActive %s existingStem %s "
                            + "lineChanged %s terminal ReadyForHeadCreateStem%n",
                    page, system.getId(), headOrdinals.get(head), headSigOrdinals.get(head), head.getId(),
                    cAliases.get(corner), point(refPt), yDir, minTail, bestTail, hex(yHard), hex(ySoft),
                    lastIndex, ((StemBuilder) C_STEM_BUILDER.get(corner)).maxIndex(), planRelations.size(),
                    list(relationInputs), planGlyphs.size(), list(selectedGlyphTokens(planGlyphs, before.glyphs)),
                    glyphToken(candidate), candidate.getId(), existing != null ? before.glyphs.alias(existing) : "-",
                    existingActive, existingStem != null ? Integer.toString(existingStem.getId()) : "-",
                    !before.lines.same(expanded.lines));
        }

        void emitHeadCLinkResult (PersistentSnapshot before,
                                  PersistentSnapshot after)
            throws Exception
        {
            final List<String> addedVertices = new ArrayList<>();
            for (Inter inter : after.sig.vertices.keySet()) {
                if (!before.sig.vertices.containsKey(inter)) {
                    addedVertices.add("id" + inter.getId() + ":" + interStructuralToken(inter));
                }
            }
            Collections.sort(addedVertices);
            final List<String> addedEdges = new ArrayList<>();
            for (SystemInfo info : sheet.getSystems()) {
                final SIGraph sig = info.getSig();
                for (Relation relation : sig.edgeSet()) {
                    if (!before.sig.edges.containsKey(relation)) {
                        addedEdges.add("system" + info.getId() + ":sourceId"
                                + sig.getEdgeSource(relation).getId() + ":targetId"
                                + sig.getEdgeTarget(relation).getId() + ":" + relationState(relation));
                    }
                }
            }
            Collections.sort(addedEdges);
            final List<String> addedStems = new ArrayList<>();
            for (Map.Entry<Glyph, StemInter> entry : after.systemStems.entries.entrySet()) {
                if (!before.systemStems.entries.containsKey(entry.getKey())) {
                    addedStems.add(glyphToken(entry.getKey()) + ":stemId" + entry.getValue().getId());
                }
            }
            Collections.sort(addedStems);
            final List<String> registeredGlyphs = new ArrayList<>();
            for (Glyph glyph : after.glyphs.originals.keySet()) {
                if (!before.glyphs.originals.containsKey(glyph)) {
                    registeredGlyphs.add(after.glyphs.alias(glyph) + ":" + glyphToken(glyph)
                            + ":active=" + after.glyphs.active.containsKey(glyph));
                }
            }
            Collections.sort(registeredGlyphs);
            final List<String> linkerChanges = new ArrayList<>();
            for (Map.Entry<StemLinker, String> entry : before.linkers.state.entrySet()) {
                final String next = after.linkers.state.get(entry.getKey());
                if (!entry.getValue().equals(next)) {
                    linkerChanges.add(linkerAlias(entry.getKey()) + ":"
                            + entry.getValue() + "->" + next);
                }
            }
            Collections.sort(linkerChanges);
            System.out.printf(
                    "stemsheadclinkresult headOrder 21 allocatorBefore %d allocatorAfter %d "
                            + "registeredGlyphs %s addedVertices %s addedEdges %s addedSystemStems %s "
                            + "linkerChanges %s sigHashBefore %s sigHashAfter %s "
                            + "systemStemsHashBefore %s systemStemsHashAfter %s "
                            + "glyphActiveHashBefore %s glyphActiveHashAfter %s "
                            + "glyphOriginalsHashBefore %s glyphOriginalsHashAfter %s "
                            + "linkerStateHashBefore %s linkerStateHashAfter %s "
                            + "relationStateHashBefore %s relationStateHashAfter %s "
                            + "terminal ReturnedHeadCLinkTransaction%n",
                    before.allocator, after.allocator, list(registeredGlyphs), list(addedVertices),
                    list(addedEdges), list(addedStems), list(linkerChanges), before.sig.hash,
                    after.sig.hash, before.systemStems.hash, after.systemStems.hash,
                    before.glyphs.activeHash, after.glyphs.activeHash,
                    before.glyphs.originalsHash, after.glyphs.originalsHash,
                    before.linkers.hash, after.linkers.hash, before.sig.relationStateHash,
                    after.sig.relationStateHash);
        }
