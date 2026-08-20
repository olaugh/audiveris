#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Read-only finalizeStems census from v103; all linkStems work mutates before the census.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi
release_field()
{
    awk -F= -v name="$1" '$1 == name { value = $2; gsub(/^"|"$/, "", value); print value }' \
        "$JAVA_HOME/release"
}
if [ "$(release_field IMPLEMENTOR)" != "Eclipse Adoptium" ] || \
        [ "$(release_field IMPLEMENTOR_VERSION)" != "Temurin-25.0.3+9" ] || \
        [ "$(release_field JAVA_RUNTIME_VERSION)" != "25.0.3+9-LTS" ] || \
        [ "$(release_field OS_NAME)" != "Darwin" ] || \
        [ "$(release_field OS_ARCH)" != "aarch64" ] || \
        [ "$(release_field JVM_VARIANT)" != "Hotspot" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS aarch64 HotSpot" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_base=/private/tmp/stems-head-phase-prefix-v104
tmp_dir=$(mktemp -d "$tmp_base.XXXXXX")
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
semantic_one="$tmp_dir/semantic1"
semantic_two="$tmp_dir/semantic2"
rows="$tmp_dir/rows"
stumps_actual="$tmp_dir/stumps-actual"
stumps_frozen="$tmp_dir/stumps-frozen"
probe_source="$tmp_dir/StemsBeamSidesLoopProbe.java"
trap ':' EXIT
fragment18="$script_dir/stems-head-phase-v28-fragment.java"

awk -v fragment="$fragment18" '
{
    if (index($0, "Collections.sort(ordered, Inters.byReverseGrade);") != 0) {
        print
        print "            RETRIEVER_SYSTEM_HEADS.set(retriever, ordered);"
        next
    }
    if (index($0, "final LinkedHashMap<Inter, Set<HorizontalSide>> undefs = new LinkedHashMap<>();") != 0) {
        print "            final Field retrieverUndefs = StemsRetriever.class.getDeclaredField(\"undefs\");"
        print "            retrieverUndefs.setAccessible(true);"
        print "            final LinkedHashMap<Inter, Set<HorizontalSide>> undefs ="
        print "                    (LinkedHashMap<Inter, Set<HorizontalSide>>) retrieverUndefs.get(retriever);"
        next
    }
    if (index($0, "emitHeadPhaseContinuation(ordered, 5, undefs);") != 0) {
        print
        print "            emitHeadPhaseContinuation(ordered, 6, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 7, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 8, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 9, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 10, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 11, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 12, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 13, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 14, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 15, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 16, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 17, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 18, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 19, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 20, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 21, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 22, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 23, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 24, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 25, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 26, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 27, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 28, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 29, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 30, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 31, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 32, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 33, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 34, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 35, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 36, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 37, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 38, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 39, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 40, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 41, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 42, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 43, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 44, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 45, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 46, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 47, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 48, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 49, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 50, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 51, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 52, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 53, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 54, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 55, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 56, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 57, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 58, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 59, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 60, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 61, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 62, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 63, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 64, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 65, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 66, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 67, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 68, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 69, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 70, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 71, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 72, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 73, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 74, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 75, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 76, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 77, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 78, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 79, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 80, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 81, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 82, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 83, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 84, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 85, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 86, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 87, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 88, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 89, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 90, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 91, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 92, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 93, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 94, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 95, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 96, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 97, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 98, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 99, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 100, undefs);"
        print "            emitHeadPhaseContinuation(ordered, 101, undefs);"
        print "            emitHeadPhaseTwo(undefs);"
        print "            emitFinalizeStemsCensus(ordered, undefs);"
        next
    }
    if (index($0, "void emitHeadPhaseContinuation (") != 0) {
        print "        /** Census and execute the exact private finalizeStems call. */"
        print "        void emitFinalizeStemsCensus (List<Inter> ordered,"
        print "                                      LinkedHashMap<Inter, Set<HorizontalSide>> undefs)"
        print "            throws Exception"
        print "        {"
        print "            RETRIEVER_SYSTEM_HEADS.set(retriever, ordered);"
        print "            final Field retrieverUndefs = StemsRetriever.class.getDeclaredField(\"undefs\");"
        print "            retrieverUndefs.setAccessible(true);"
        print "            final LinkedHashMap<Inter, Set<HorizontalSide>> ownedUndefs ="
        print "                    (LinkedHashMap<Inter, Set<HorizontalSide>>) retrieverUndefs.get(retriever);"
        print "            if (ownedUndefs != undefs) {"
        print "                ownedUndefs.clear();"
        print "                ownedUndefs.putAll(undefs);"
        print "            }"
        print "            final List<String> undefRows = new ArrayList<>();"
        print "            for (Map.Entry<Inter, Set<HorizontalSide>> entry : ownedUndefs.entrySet()) {"
        print "                final HeadInter head = (HeadInter) entry.getKey();"
        print "                undefRows.add(\"x\" + headOrdinals.get(head) + \":sig\""
        print "                        + headSigOrdinals.get(head) + \":id\" + head.getId()"
        print "                        + \":sides\" + entry.getValue());"
        print "            }"
        print "            final List<String> candidates = new ArrayList<>();"
        print "            final IdentityHashMap<HeadInter, Boolean> abnormalBefore = new IdentityHashMap<>();"
        print "            final IdentityHashMap<Relation, String> headStemBefore = new IdentityHashMap<>();"
        print "            int multipleStemHeadsBefore = 0;"
        print "            int sameSideHeadsBefore = 0;"
        print "            int noStemHeadsBefore = 0;"
        print "            int abnormalHeadsBefore = 0;"
        print "            for (Inter hInter : ordered) {"
        print "                final HeadInter head = (HeadInter) hInter;"
        print "                abnormalBefore.put(head, head.isAbnormal());"
        print "                if (head.isAbnormal()) abnormalHeadsBefore++;"
        print "                int left = 0;"
        print "                int right = 0;"
        print "                final List<String> relations = new ArrayList<>();"
        print "                for (Relation relation : system.getSig().getRelations(head, HeadStemRelation.class)) {"
        print "                    final HeadStemRelation hs = (HeadStemRelation) relation;"
        print "                    final StemInter stem = (StemInter) system.getSig().getOppositeInter(head, relation);"
        print "                    if (hs.getHeadSide() == HorizontalSide.LEFT) left++; else right++;"
        print "                    final String token = \"stem\" + stem.getId() + \":side\" + hs.getHeadSide()"
        print "                            + \":stemGrade\" + hex(stem.getGrade()) + \":relation\""
        print "                            + relationState(relation);"
        print "                    relations.add(token);"
        print "                    headStemBefore.put(relation, \"x\" + headOrdinals.get(head) + \":sig\""
        print "                            + headSigOrdinals.get(head) + \":id\" + head.getId() + \":\" + token);"
        print "                }"
        print "                final int relationCount = relations.size();"
        print "                final boolean multiple = relationCount > 1;"
        print "                final boolean sameSide = left > 1 || right > 1;"
        print "                final boolean noStem = ShapeSet.StemHeads.contains(head.getShape())"
        print "                        && relationCount == 0;"
        print "                if (multiple) multipleStemHeadsBefore++;"
        print "                if (sameSide) sameSideHeadsBefore++;"
        print "                if (noStem) noStemHeadsBefore++;"
        print "                if (multiple || noStem) {"
        print "                    candidates.add(\"x\" + headOrdinals.get(head) + \":sig\""
        print "                            + headSigOrdinals.get(head) + \":id\" + head.getId()"
        print "                            + \":shape\" + head.getShape() + \":relations\" + relationCount"
        print "                            + \":left\" + left + \":right\" + right + \":abnormal\""
        print "                            + head.isAbnormal() + \":rows\" + list(relations));"
        print "                }"
        print "            }"
        print "            final PersistentSnapshot before = snapshot("
        print "                    sheet, retriever, inspectionBeams, heads, allLinkers);"
        print "            System.out.printf("
        print "                    \"stemsfinalizebaseline %s system %d heads %d multipleStemHeads %d \""
        print "                            + \"sameSideHeads %d noStemHeads %d abnormalHeads %d undefs %s \""
        print "                            + \"candidates %s sigVertices %d sigEdges %d systemStems %d \""
        print "                            + \"sigHash %s relationStateHash %s interHash %s linkerHash %s \""
        print "                            + \"terminal ReadyBeforeFinalizeStems%n\","
        print "                    page, system.getId(), ordered.size(), multipleStemHeadsBefore,"
        print "                    sameSideHeadsBefore, noStemHeadsBefore, abnormalHeadsBefore,"
        print "                    list(undefRows), list(candidates), before.sig.vertices.size(),"
        print "                    before.sig.edges.size(), before.systemStems.entries.size(),"
        print "                    before.sig.hash, before.sig.relationStateHash, before.inters.hash,"
        print "                    before.linkers.hash);"
        print "            final Field watchField = StemsRetriever.class.getDeclaredField(\"watch\");"
        print "            watchField.setAccessible(true);"
        print "            final Class<?> watchClass = Class.forName(\"org.audiveris.omr.util.StopWatch\");"
        print "            watchField.set(retriever, watchClass.getConstructor(String.class)"
        print "                    .newInstance(\"Rust finalizeStems census\"));"
        print "            final Method finalizeStems = StemsRetriever.class.getDeclaredMethod(\"finalizeStems\");"
        print "            finalizeStems.setAccessible(true);"
        print "            finalizeStems.invoke(retriever);"
        print "            final PersistentSnapshot after = snapshot("
        print "                    sheet, retriever, inspectionBeams, heads, allLinkers);"
        print "            final List<String> removedHeadStemRelations = new ArrayList<>();"
        print "            for (Map.Entry<Relation, String> entry : headStemBefore.entrySet()) {"
        print "                if (!after.sig.edges.containsKey(entry.getKey())) {"
        print "                    removedHeadStemRelations.add(entry.getValue());"
        print "                }"
        print "            }"
        print "            Collections.sort(removedHeadStemRelations);"
        print "            final List<String> addedRelations = new ArrayList<>();"
        print "            for (Relation relation : system.getSig().edgeSet()) {"
        print "                if (!before.sig.edges.containsKey(relation)) {"
        print "                    final Inter source = system.getSig().getEdgeSource(relation);"
        print "                    final Inter target = system.getSig().getEdgeTarget(relation);"
        print "                    addedRelations.add(relation.getClass().getSimpleName() + \":source\""
        print "                            + source.getId() + \":target\" + target.getId() + \":\""
        print "                            + relationState(relation));"
        print "                }"
        print "            }"
        print "            Collections.sort(addedRelations);"
        print "            final List<String> abnormalChanges = new ArrayList<>();"
        print "            int multipleStemHeadsAfter = 0;"
        print "            int sameSideHeadsAfter = 0;"
        print "            int noStemHeadsAfter = 0;"
        print "            int abnormalHeadsAfter = 0;"
        print "            for (Inter hInter : ordered) {"
        print "                final HeadInter head = (HeadInter) hInter;"
        print "                if (head.isAbnormal()) abnormalHeadsAfter++;"
        print "                if (abnormalBefore.get(head) != head.isAbnormal()) {"
        print "                    abnormalChanges.add(\"x\" + headOrdinals.get(head) + \":sig\""
        print "                            + headSigOrdinals.get(head) + \":id\" + head.getId() + \":\""
        print "                            + abnormalBefore.get(head) + \"->\" + head.isAbnormal());"
        print "                }"
        print "                int left = 0;"
        print "                int right = 0;"
        print "                int relationCount = 0;"
        print "                for (Relation relation : system.getSig().getRelations(head, HeadStemRelation.class)) {"
        print "                    relationCount++;"
        print "                    if (((HeadStemRelation) relation).getHeadSide() == HorizontalSide.LEFT) left++;"
        print "                    else right++;"
        print "                }"
        print "                if (relationCount > 1) multipleStemHeadsAfter++;"
        print "                if (left > 1 || right > 1) sameSideHeadsAfter++;"
        print "                if (ShapeSet.StemHeads.contains(head.getShape()) && relationCount == 0) {"
        print "                    noStemHeadsAfter++;"
        print "                }"
        print "            }"
        print "            Collections.sort(abnormalChanges);"
        print "            System.out.printf("
        print "                    \"stemsfinalizeresult %s system %d multipleStemHeadsBefore %d \""
        print "                            + \"multipleStemHeadsAfter %d sameSideHeadsBefore %d \""
        print "                            + \"sameSideHeadsAfter %d noStemHeadsBefore %d noStemHeadsAfter %d \""
        print "                            + \"abnormalHeadsBefore %d abnormalHeadsAfter %d \""
        print "                            + \"removedHeadStemRelations %s addedRelations %s \""
        print "                            + \"abnormalChanges %s sigVerticesBefore %d sigVerticesAfter %d \""
        print "                            + \"sigEdgesBefore %d sigEdgesAfter %d systemStemsBefore %d \""
        print "                            + \"systemStemsAfter %d sigHashBefore %s sigHashAfter %s \""
        print "                            + \"relationStateHashBefore %s relationStateHashAfter %s \""
        print "                            + \"interHashBefore %s interHashAfter %s linkerHashBefore %s \""
        print "                            + \"linkerHashAfter %s allocatorBefore %d allocatorAfter %d \""
        print "                            + \"terminal ReturnedAfterFinalizeStems%n\","
        print "                    page, system.getId(), multipleStemHeadsBefore, multipleStemHeadsAfter,"
        print "                    sameSideHeadsBefore, sameSideHeadsAfter, noStemHeadsBefore, noStemHeadsAfter,"
        print "                    abnormalHeadsBefore, abnormalHeadsAfter, list(removedHeadStemRelations),"
        print "                    list(addedRelations), list(abnormalChanges), before.sig.vertices.size(),"
        print "                    after.sig.vertices.size(), before.sig.edges.size(), after.sig.edges.size(),"
        print "                    before.systemStems.entries.size(), after.systemStems.entries.size(),"
        print "                    before.sig.hash, after.sig.hash, before.sig.relationStateHash,"
        print "                    after.sig.relationStateHash, before.inters.hash, after.inters.hash,"
        print "                    before.linkers.hash, after.linkers.hash, before.allocator, after.allocator);"
        print "        }"
        print ""
        print "        /** The Java unlinkedHeads list: every head whose phase-1 linkSides returned false. */"
        print "        private final List<HeadInter> phase2Queue = new ArrayList<>();"
        print ""
        print "        /** Heads linking phase 2: linkSides(..., append=true) over unlinkedHeads. */"
        print "        void emitHeadPhaseTwo (LinkedHashMap<Inter, Set<HorizontalSide>> undefs)"
        print "            throws Exception"
        print "        {"
        print "            final List<String> queue = new ArrayList<>();"
        print "            for (HeadInter q : phase2Queue) {"
        print "                queue.add(\"x\" + headOrdinals.get(q) + \":sig\" + headSigOrdinals.get(q)"
        print "                        + \":id\" + q.getId());"
        print "            }"
        print "            System.out.printf("
        print "                    \"stemsheadphasetwobaseline %s system %d queueSize %d queue %s \""
        print "                            + \"terminal ReadyBeforeFirstAppendRetry%n\","
        print "                    page, system.getId(), phase2Queue.size(), list(queue));"
        print "            for (int qi = 0; qi < phase2Queue.size(); qi++) {"
        print "                final HeadInter head = phase2Queue.get(qi);"
        print "                final HeadLinker linker = head.getLinker();"
        print "                final String sidesBefore = headSideState(linker);"
        print "                final List<String> decisions = new ArrayList<>();"
        print "                for (HorizontalSide hs : HorizontalSide.values()) {"
        print "                    final HeadLinker.SLinker s = linker.getSLinkers().get(hs);"
        print "                    if (s.isLinked()) {"
        print "                        decisions.add(hs + \":SkipAlreadyLinked\");"
        print "                        continue;"
        print "                    }"
        print "                    final boolean t = s.getCornerLinker(VerticalSide.TOP).canLink("
        print "                            Profiles.STRICT, true);"
        print "                    final boolean b = s.getCornerLinker(VerticalSide.BOTTOM).canLink("
        print "                            Profiles.STRICT, true);"
        print "                    final String branch;"
        print "                    if (t && !b) {"
        print "                        branch = \"TopOnly\";"
        print "                    } else if (!t && b) {"
        print "                        branch = \"BottomOnly\";"
        print "                    } else if (t) {"
        print "                        branch = \"Both\";"
        print "                    } else {"
        print "                        branch = \"Neither\";"
        print "                    }"
        print "                    decisions.add(hs + \":top=\" + t + \":bottom=\" + b + \":branch=\" + branch);"
        print "                }"
        print "                final PersistentSnapshot before = snapshot("
        print "                        sheet, retriever, inspectionBeams, heads, allLinkers);"
        print "                final boolean returned = linker.linkSides("
        print "                        Profiles.STRICT, system.getProfile(), undefs, true);"
        print "                final PersistentSnapshot after = snapshot("
        print "                        sheet, retriever, inspectionBeams, heads, allLinkers);"
        print "                final List<String> phase2Changes = new ArrayList<>();"
        print "                for (Map.Entry<StemLinker, String> pe : before.linkers.state.entrySet()) {"
        print "                    final String pnext = after.linkers.state.get(pe.getKey());"
        print "                    if (!pe.getValue().equals(pnext)) {"
        print "                        phase2Changes.add(linkerAlias(pe.getKey()) + \":\" + pe.getValue()"
        print "                                + \"->\" + pnext);"
        print "                    }"
        print "                }"
        print "                Collections.sort(phase2Changes);"
        print "                System.out.printf("
        print "                        \"stemsheadphasetwo %s system %d queueIndex %d headX %d headSig %d \""
        print "                                + \"headInterId %d grade %s append true sidesBefore %s decisions %s returned %s \""
        print "                                + \"sidesAfter %s undefs %s sigVerticesBefore %d sigVerticesAfter %d \""
        print "                                + \"sigEdgesBefore %d sigEdgesAfter %d systemStemsBefore %d \""
        print "                                + \"systemStemsAfter %d relationStateHashBefore %s \""
        print "                                + \"relationStateHashAfter %s linkerStateHashBefore %s \""
        print "                                + \"linkerStateHashAfter %s linkerChanges %s terminal ReturnedAfterAppendRetry%n\","
        print "                        page, system.getId(), qi, headOrdinals.get(head),"
        print "                        headSigOrdinals.get(head), head.getId(), hex(head.getGrade()),"
        print "                        sidesBefore, list(decisions), returned, headSideState(linker),"
        print "                        undefs.get(head) == null ? \"[]\" : undefs.get(head),"
        print "                        before.sig.vertices.size(), after.sig.vertices.size(),"
        print "                        before.sig.edges.size(), after.sig.edges.size(),"
        print "                        before.systemStems.entries.size(), after.systemStems.entries.size(),"
        print "                        before.sig.relationStateHash, after.sig.relationStateHash,"
        print "                        before.linkers.hash, after.linkers.hash, list(phase2Changes));"
        print "            }"
        print "        }"
        print ""
        print ""
        in_continuation = 1
    }
    if (in_continuation && index($0, "final HeadLinker linker = head.getLinker();") != 0) {
        print
        print "            if (headOrder < 101) {"
        print "                if (!linker.linkSides(Profiles.STRICT, system.getProfile(), undefs, false)) {"
        print "                    phase2Queue.add(head);"
        print "                }"
        print "                return;"
        print "            }"
        next
    }
    if (index($0, "final boolean returned = linker.linkSides(") != 0) {
        print "            if (headOrder == 7 || headOrder == 22 || headOrder == 27 || headOrder == 34 || headOrder == 36 || headOrder == 37 || headOrder == 38 || headOrder == 39 || headOrder == 40 || headOrder == 41 || headOrder == 42 || headOrder == 43 || headOrder == 44 || headOrder == 45 || headOrder == 46 || headOrder == 47 || headOrder == 48 || headOrder == 49 || headOrder == 50 || headOrder == 51 || headOrder == 52 || headOrder == 53 || headOrder == 54 || headOrder == 55 || headOrder == 56 || headOrder == 57 || headOrder == 58 || headOrder == 59 || headOrder == 60 || headOrder == 61 || headOrder == 62 || headOrder == 63 || headOrder == 64 || headOrder == 65 || headOrder == 66 || headOrder == 67 || headOrder == 68 || headOrder == 69 || headOrder == 70 || headOrder == 71 || headOrder == 72 || headOrder == 73 || headOrder == 74 || headOrder == 75 || headOrder == 76 || headOrder == 77 || headOrder == 78 || headOrder == 79 || headOrder == 80 || headOrder == 81 || headOrder == 82 || headOrder == 83 || headOrder == 84 || headOrder == 85 || headOrder == 86 || headOrder == 87 || headOrder == 88 || headOrder == 89 || headOrder == 90 || headOrder == 91 || headOrder == 92 || headOrder == 93 || headOrder == 94 || headOrder == 95 || headOrder == 96 || headOrder == 97 || headOrder == 98 || headOrder == 99 || headOrder == 100 || headOrder == 101) emitHeadCLinkEnvelope(head, linker, before, headOrder);"
        print
        in_linker = 1
        next
    }
    if (in_linker && index($0, "final PersistentSnapshot after = snapshot(") != 0) {
        print
        capture_after = 1
        next
    }
    if (capture_after && index($0, "heads, allLinkers);") != 0) {
        print
        print "            if (!returned) phase2Queue.add(head);"
        print "            if (headOrder == 7 || headOrder == 22 || headOrder == 27 || headOrder == 34 || headOrder == 36 || headOrder == 37 || headOrder == 38 || headOrder == 39 || headOrder == 40 || headOrder == 41 || headOrder == 42 || headOrder == 43 || headOrder == 44 || headOrder == 45 || headOrder == 46 || headOrder == 47 || headOrder == 48 || headOrder == 49 || headOrder == 50 || headOrder == 51 || headOrder == 52 || headOrder == 53 || headOrder == 54 || headOrder == 55 || headOrder == 56 || headOrder == 57 || headOrder == 58 || headOrder == 59 || headOrder == 60 || headOrder == 61 || headOrder == 62 || headOrder == 63 || headOrder == 64 || headOrder == 65 || headOrder == 66 || headOrder == 67 || headOrder == 68 || headOrder == 69 || headOrder == 70 || headOrder == 71 || headOrder == 72 || headOrder == 73 || headOrder == 74 || headOrder == 75 || headOrder == 76 || headOrder == 77 || headOrder == 78 || headOrder == 79 || headOrder == 80 || headOrder == 81 || headOrder == 82 || headOrder == 83 || headOrder == 84 || headOrder == 85 || headOrder == 86 || headOrder == 87 || headOrder == 88 || headOrder == 89 || headOrder == 90 || headOrder == 91 || headOrder == 92 || headOrder == 93 || headOrder == 94 || headOrder == 95 || headOrder == 96 || headOrder == 97 || headOrder == 98 || headOrder == 99 || headOrder == 100 || headOrder == 101) emitHeadCLinkResult(before, after, headOrder);"
        capture_after = 0
        in_linker = 0
        next
    }
    if (in_continuation && index($0, "final HeadInter next = (HeadInter) ordered.get(headOrder + 1);") != 0) {
        print "            final boolean hasNext = headOrder + 1 < ordered.size();"
        print "            final HeadInter next = hasNext ? (HeadInter) ordered.get(headOrder + 1) : null;"
        print "            System.out.printf("
        print "                    \"stemsheadphasecontinue %s system %d headOrder %d headX %d headSig %d \""
        print "                            + \"headInterId %d grade %s stemProfile %d linkProfile %d append false \""
        print "                            + \"sidesBefore %s decisions %s incident %s returned %s sidesAfter %s \""
        print "                            + \"undefs %s closureWrites %s closedValueChanges %d unlinkedCount 0 \""
        print "                            + \"sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d \""
        print "                            + \"sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d \""
        print "                            + \"relationStateHashBefore %s relationStateHashAfter %s \""
        print "                            + \"linkerStateHashBefore %s linkerStateHashAfter %s \""
        print "                            + \"nextHeadOrder %s nextHeadX %s nextHeadSig %s nextHeadInterId %s \""
        print "                            + \"nextGrade %s nextSides %s terminal %s%n\","
        print "                    page, system.getId(), headOrder, headOrdinals.get(head),"
        print "                    headSigOrdinals.get(head), head.getId(), hex(head.getGrade()),"
        print "                    Profiles.STRICT, system.getProfile(), sidesBefore, list(decisions),"
        print "                    list(incident), returned, headSideState(linker),"
        print "                    undefs.get(head) == null ? \"[]\" : undefs.get(head), list(closureWrites),"
        print "                    closedValueChanges, before.sig.vertices.size(), after.sig.vertices.size(),"
        print "                    before.sig.edges.size(), after.sig.edges.size(),"
        print "                    before.systemStems.entries.size(), after.systemStems.entries.size(),"
        print "                    before.sig.relationStateHash, after.sig.relationStateHash,"
        print "                    before.linkers.hash, after.linkers.hash,"
        print "                    hasNext ? Integer.toString(headOrder + 1) : \"-\","
        print "                    hasNext ? headOrdinals.get(next).toString() : \"-\","
        print "                    hasNext ? headSigOrdinals.get(next).toString() : \"-\","
        print "                    hasNext ? Integer.toString(next.getId()) : \"-\","
        print "                    hasNext ? hex(next.getGrade()) : \"-\","
        print "                    hasNext ? headSideState(next.getLinker()) : \"-\","
        print "                    hasNext ? \"ReturnedBeforeNextHead\" : \"ReturnedAfterFinalHead\");"
        skip_continuation_tail = 1
        next
    }
    if (skip_continuation_tail) {
        if (index($0, "headSideState(next.getLinker()));") != 0) {
            skip_continuation_tail = 0
        }
        next
    }
    if (index($0, "void emitHeadCLinkMutation (") != 0) {
        while ((getline line < fragment) > 0) print line
        close(fragment)
    }
    print
}' "$script_dir/StemsBeamSidesLoopProbe.java" > "$probe_source"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon \
            -PstumpsTransactionLimit=7 -PheadPhasePrefixProbe=true \
            -PheadPhaseV7ProbeSource="$probe_source" \
            -I "$script_dir/stems-head-phase-v7.init.gradle" \
            :app:stemsHeadPhaseV7Probe
    ) > "$target"
}
run_pass "$warmup"
run_pass "$pass_one"
run_pass "$pass_two"
grep -E '^(stemsbeam|stemshead|stemsfinalize)' "$pass_one" > "$semantic_one"
grep -E '^(stemsbeam|stemshead|stemsfinalize)' "$pass_two" > "$semantic_two"
if ! cmp -s "$semantic_one" "$semantic_two"; then
    echo "two fresh post-STUMPS v27 semantic passes are not byte-identical" >&2
    diff "$semantic_one" "$semantic_two" | head -8 >&2
    exit 1
fi

complete_fixture="$repo_root/rust/oracle/stems-beam-stumps-complete-chula-system1.txt"
grep '^stemsbeamstumpstxn' "$pass_one" | awk '
    /^stemsbeamstumpstxnresult / && / transaction 4 plan 508 / { emit = 1 }
    emit { print }
    /^stemsbeamstumpstxnresumeterminal / && / transactions 7 terminal Completed / { exit }
' > "$stumps_actual"
grep '^stemsbeamstumpstxn' "$complete_fixture" > "$stumps_frozen"
if ! cmp -s "$stumps_frozen" "$stumps_actual"; then
    echo "post-STUMPS probe changed the frozen complete-STUMPS predecessor" >&2
    diff "$stumps_frozen" "$stumps_actual" | head -8 >&2
    exit 1
fi

grep -E '^(stemshead|stemsfinalize)' "$pass_one" > "$rows"
if [ "$(grep -c '^stemsheadphasetwobaseline ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsheadphasetwo ' "$rows")" -ne 5 ] || \
        ! grep -q '^stemsheadphasetwobaseline .*queueSize 5 queue \[x32:sig50:id1389,x71:sig49:id1387,x70:sig46:id1377,x0:sig51:id1390,x31:sig47:id1381\] ' "$rows" || \
        ! grep -q '^stemsheadphasetwo .*queueIndex 0 headX 32 headSig 50 headInterId 1389 .*append true sidesBefore \[LEFT:false:false,RIGHT:false:false\] decisions \[LEFT:top=true:bottom=true:branch=Both,RIGHT:top=true:bottom=false:branch=TopOnly\] returned false sidesAfter \[LEFT:false:false,RIGHT:false:false\] undefs \[LEFT\] sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 .*linkerChanges - terminal ReturnedAfterAppendRetry' "$rows" || \
        ! grep -q '^stemsheadphasetwo .*queueIndex 1 headX 71 headSig 49 headInterId 1387 .*append true sidesBefore \[LEFT:true:true,RIGHT:false:true\] decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=true:bottom=false:branch=TopOnly\] returned true sidesAfter \[LEFT:true:true,RIGHT:false:true\] undefs \[LEFT\] sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 .*linkerChanges \[h:73:LEFT:BOTTOM:true:false->true:true,h:73:LEFT:TOP:true:false->true:true,h:73:RIGHT:BOTTOM:false:false->false:true,h:73:RIGHT:TOP:false:false->false:true,linker:SLinker:head:73:false:false->false:true,linker:SLinker:head:73:true:false->true:true\] terminal ReturnedAfterAppendRetry' "$rows" || \
        ! grep -q '^stemsheadphasetwo .*queueIndex 2 headX 70 headSig 46 headInterId 1377 .*append true sidesBefore \[LEFT:true:true,RIGHT:false:true\] decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=true:bottom=false:branch=TopOnly\] returned true sidesAfter \[LEFT:true:true,RIGHT:false:true\] undefs \[LEFT\] sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 .*linkerChanges - terminal ReturnedAfterAppendRetry' "$rows" || \
        ! grep -q '^stemsheadphasetwo .*queueIndex 3 headX 0 headSig 51 headInterId 1390 .*append true sidesBefore \[LEFT:true:true,RIGHT:false:true\] decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\] returned true sidesAfter \[LEFT:true:true,RIGHT:false:true\] undefs \[LEFT\] sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 .*linkerChanges \[h:1:LEFT:BOTTOM:true:false->true:true,h:1:LEFT:TOP:true:false->true:true,h:1:RIGHT:BOTTOM:false:false->false:true,h:1:RIGHT:TOP:false:false->false:true,linker:SLinker:head:1:false:false->false:true,linker:SLinker:head:1:true:false->true:true\] terminal ReturnedAfterAppendRetry' "$rows" || \
        ! grep -q '^stemsheadphasetwo .*queueIndex 4 headX 31 headSig 47 headInterId 1381 .*append true sidesBefore \[LEFT:false:false,RIGHT:false:false\] decisions \[LEFT:top=true:bottom=true:branch=Both,RIGHT:top=true:bottom=false:branch=TopOnly\] returned false sidesAfter \[LEFT:false:false,RIGHT:false:false\] undefs \[LEFT\] sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 .*linkerChanges - terminal ReturnedAfterAppendRetry' "$rows"; then
    echo "v103 phase-2 append-retry sweep contract differs" >&2
    exit 1
fi
if [ "$(grep -c '^stemsfinalizebaseline ' "$rows")" -ne 1 ] || \
        [ "$(grep -c '^stemsfinalizeresult ' "$rows")" -ne 1 ] || \
        ! grep -Fqx 'stemsfinalizebaseline chula.png#1 system 1 heads 102 multipleStemHeads 0 sameSideHeads 0 noStemHeads 2 abnormalHeads 2 undefs [x32:sig50:id1389:sides[LEFT],x71:sig49:id1387:sides[LEFT],x70:sig46:id1377:sides[LEFT],x0:sig51:id1390:sides[LEFT],x31:sig47:id1381:sides[LEFT]] candidates [x32:sig50:id1389:shapeNOTEHEAD_VOID:relations0:left0:right0:abnormaltrue:rows-,x31:sig47:id1381:shapeNOTEHEAD_VOID:relations0:left0:right0:abnormaltrue:rows-] sigVertices 685 sigEdges 706 systemStems 46 sigHash 9f76a890d36347327670bf410c735c01f8a77ef032827cf05e2943bbebc2c014 relationStateHash 40575f27cf08f5cb84a12202810df1683b4907b0dc703866c02df9b6f73fee70 interHash 4d29ef1d2f596a9ec92f37ab46ec3d8eff0a8cda96bf194c099a7a5cafe13f22 linkerHash 02f3e3adabf96ef1bda37002902263f55fc553a2676e58fbf9b78a54724ada9b terminal ReadyBeforeFinalizeStems' "$rows" || \
        ! grep -Fqx 'stemsfinalizeresult chula.png#1 system 1 multipleStemHeadsBefore 0 multipleStemHeadsAfter 0 sameSideHeadsBefore 0 sameSideHeadsAfter 0 noStemHeadsBefore 2 noStemHeadsAfter 2 abnormalHeadsBefore 2 abnormalHeadsAfter 2 removedHeadStemRelations - addedRelations - abnormalChanges - sigVerticesBefore 685 sigVerticesAfter 685 sigEdgesBefore 706 sigEdgesAfter 706 systemStemsBefore 46 systemStemsAfter 46 sigHashBefore 9f76a890d36347327670bf410c735c01f8a77ef032827cf05e2943bbebc2c014 sigHashAfter 9f76a890d36347327670bf410c735c01f8a77ef032827cf05e2943bbebc2c014 relationStateHashBefore 40575f27cf08f5cb84a12202810df1683b4907b0dc703866c02df9b6f73fee70 relationStateHashAfter 40575f27cf08f5cb84a12202810df1683b4907b0dc703866c02df9b6f73fee70 interHashBefore 4d29ef1d2f596a9ec92f37ab46ec3d8eff0a8cda96bf194c099a7a5cafe13f22 interHashAfter 4d29ef1d2f596a9ec92f37ab46ec3d8eff0a8cda96bf194c099a7a5cafe13f22 linkerHashBefore 02f3e3adabf96ef1bda37002902263f55fc553a2676e58fbf9b78a54724ada9b linkerHashAfter 02f3e3adabf96ef1bda37002902263f55fc553a2676e58fbf9b78a54724ada9b allocatorBefore 2385 allocatorAfter 2385 terminal ReturnedAfterFinalizeStems' "$rows"; then
    echo "v104 finalizeStems census contract differs" >&2
    exit 1
fi
: <<'DISABLED_V27_ACTIVE'
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 10 ] || \
        ! grep -q 'headOrder 27 headX 33 headSig 26 headInterId 1337 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\].*nextHeadOrder 28 nextHeadX 85 nextHeadSig 87 nextHeadInterId 1463 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .*headOrder 27 headX 33 headSig 26 .*cAlias h:33:LEFT:BOTTOM .*lastIndex 1 maxIndex 1 relations 1 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:314 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 27 allocatorBefore 2382 allocatorAfter 2383 .*addedVertices \[id2383:.*addedEdges \[system1:sourceId1337:targetId2383:.*addedSystemStems \[g:1019:388:3:89:.*:stemId2383\]' "$rows"; then
    echo "v27 both-open C-link contract differs" >&2
    exit 1
fi
DISABLED_V27_ACTIVE
: <<'DISABLED_V26_CONTRACT'
if [ "$(grep -c '^stemsheadphasecontinue ' "$rows")" -ne 10 ] || \
        ! grep -q 'headOrder 25 headX 59 headSig 74 headInterId 1437 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2363:headSideLEFT:heads\[x58:sig73:id1435:sideLEFT,x59:sig74:id1437:sideLEFT\]\].*closureWrites \[x58:sig73:LEFT:false->true,x58:sig73:RIGHT:false->true\].*closedValueChanges 2 .*nextHeadOrder 26 nextHeadX 61 nextHeadSig 31 nextHeadInterId 1347 ' "$rows" || \
        ! grep -q 'headOrder 26 headX 61 headSig 31 headInterId 1347 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2345:headSideLEFT:heads\[x60:sig30:id1345:sideLEFT,x61:sig31:id1347:sideLEFT\]\].*closureWrites \[x60:sig30:LEFT:false->true,x60:sig30:RIGHT:false->true\].*closedValueChanges 2 .*nextHeadOrder 27 nextHeadX 33 nextHeadSig 26 nextHeadInterId 1337 ' "$rows" || \
        ! grep -q 'headOrder 27 headX 33 headSig 26 headInterId 1337 .*decisions \[LEFT:top=false:bottom=true:branch=BottomOnly,RIGHT:top=false:bottom=false:branch=Neither\].*nextHeadOrder 28 nextHeadX 85 nextHeadSig 87 nextHeadInterId 1463 ' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .* headOrder 27 headX 33 headSig 26 .*cAlias h:33:LEFT:BOTTOM .*lastIndex 1 maxIndex 1 relations 1 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:314 existingActive true existingStem - ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 27 allocatorBefore 2382 allocatorAfter 2383 .*addedVertices \[id2383:.*addedEdges \[system1:sourceId1337:targetId2383:.*addedSystemStems \[g:1019:388:3:89:.*:stemId2383\].*linkerChanges \[h:33:LEFT:BOTTOM:false:false->true:false,h:33:LEFT:TOP:false:false->true:false,linker:SLinker:head:33:false:false->true:false\]' "$rows"; then
    echo "v27 both-open C-link contract differs" >&2
    exit 1
fi
DISABLED_V26_CONTRACT
if false; then
    :
fi
: <<'DISABLED_V23_CONTRACT'
        ! grep -q 'headOrder 22 headX 4 headSig 7 headInterId 1299 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\]' "$rows" || \
        ! grep -q '^stemsheadclinkfrontier .* headOrder 22 .* relations 2 .*glyphs 2 .*candidateIdBefore 0 existingGlyph glyph:315 existingActive true existingStem 2354 ' "$rows" || \
        ! grep -q '^stemsheadclinkresult headOrder 22 allocatorBefore 2382 allocatorAfter 2382 registeredGlyphs - addedVertices - addedEdges - addedSystemStems - linkerChanges \[h:3:LEFT:BOTTOM:true:false->true:true,h:3:LEFT:TOP:true:false->true:true,h:3:RIGHT:BOTTOM:false:false->false:true,h:3:RIGHT:TOP:false:false->false:true,linker:SLinker:head:3:false:false->false:true,linker:SLinker:head:3:true:false->true:true\] ' "$rows" || \
        ! grep -q 'headOrder 23 headX 78 headSig 39 headInterId 1363 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2370:headSideLEFT:heads\[x77:sig38:id1361:sideLEFT,x78:sig39:id1363:sideLEFT\]\].*closureWrites \[x77:sig38:LEFT:false->true,x77:sig38:RIGHT:false->true\].*closedValueChanges 2 unlinkedCount 0 sigVerticesBefore 682 sigVerticesAfter 682 sigEdgesBefore 693 sigEdgesAfter 693 relationStateHashBefore 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d relationStateHashAfter 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d .*nextHeadOrder 24 nextHeadX 93 nextHeadSig 25 nextHeadInterId 1335 .*nextSides \[LEFT:true:false,RIGHT:false:false\]' "$rows" || \
        ! grep -q 'headOrder 24 headX 93 headSig 25 headInterId 1335 .*decisions \[LEFT:SkipAlreadyLinked,RIGHT:top=false:bottom=false:branch=Neither\].*incident \[stem2342:headSideLEFT:heads\[x92:sig24:id1333:sideLEFT,x93:sig25:id1335:sideLEFT\]\].*closureWrites \[x92:sig24:LEFT:false->true,x92:sig24:RIGHT:false->true\].*closedValueChanges 2 unlinkedCount 0 sigVerticesBefore 682 sigVerticesAfter 682 sigEdgesBefore 693 sigEdgesAfter 693 relationStateHashBefore 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d relationStateHashAfter 3c50fb8ad029b80132262f4607e856ffcfadd6aa777d03bd3795288018faac9d .*nextHeadOrder 25 nextHeadX 59 nextHeadSig 74 nextHeadInterId 1437 .*nextSides \[LEFT:true:false,RIGHT:false:false\]' "$rows"; then
    echo "v24 bounded prelinked-closure contract differs" >&2
    exit 1
fi
DISABLED_V23_CONTRACT

probe_sha=$(shasum -a 256 "$probe_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$script_dir/StemsBeamSidesLoopProbe.java" | awk '{print $1}')
if [ "$base_probe_sha" != "d5d46115fb4358918648d35e24cd043753b62ce709f767f8958d34ba25c9c4cf" ]; then
    echo "base probe drifted from the frozen v6 source" >&2
    exit 1
fi
v103_runner_sha=$(shasum -a 256 "$script_dir/run-stems-head-phase-prefix-v103.sh" | awk '{print $1}')
if [ "$v103_runner_sha" != "9714b52a8ce9941ef0832a3f17689fcbdeb5b14246546770cd001167be618b26" ]; then
    echo "v104 base v103 runner drifted" >&2
    exit 1
fi
v103_fixture="$repo_root/rust/oracle/stems-head-phase-prefix-chula-system1-v103.txt"
v103_fixture_sha=$(shasum -a 256 "$v103_fixture" | awk '{print $1}')
if [ "$v103_fixture_sha" != "2c192a356f2aa9447c6b0bea5a5979086646390043b2b1333983038d5d3d03c4" ]; then
    echo "v104 base v103 fixture drifted" >&2
    exit 1
fi
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
semantic_sha=$(shasum -a 256 "$semantic_one" | awk '{print $1}')
stumps_sha=$(shasum -a 256 "$complete_fixture" | awk '{print $1}')
fragment_sha=$(shasum -a 256 "$fragment18" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out="/private/tmp/stems-head-phase-prefix-chula-system1-v104-audit.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) finalizeStems census v104.'
    echo '# schema: stems-head-phase-prefix-v104'
    echo '# Strict v103 predecessor followed by exact finalizeStems baseline and result census.'
    echo '# systemHeads and undefs are rebound to the production-owned reverse-grade inputs before finalization.'
    cat "$rows"
    printf '%s\n' \
        "stemsheadphaseprefix summary schema stems-head-phase-prefix-v104 page chula.png#1 system 1 rows $row_count baseProbeSourceSha256 $base_probe_sha baseV103RunnerSourceSha256 $v103_runner_sha baseV103FixtureSha256 $v103_fixture_sha fragmentSourceSha256 $fragment_sha probeSourceSha256 $probe_sha runnerSourceSha256 $runner_sha emittedBodySha256 $body_sha semanticPassSha256 $semantic_sha completeStumpsFixtureSha256 $stumps_sha freshRuns 2 freshRunsByteIdentical true nativeScope BoundedSnapshotMinimizedFinalizeStemsCensus javaEvidence ReturnedAfterFinalizeStems"
} > "$out"
echo "wrote $out"
