// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.StopWatch;
import org.audiveris.omr.util.VerticalSide;

/** Deterministic page-wide evidence for Java's second head-linking phase. */
public final class StemsHeadFinalizePageProbe {
    private static final Method INSPECT;
    private static final Field PARAMS;
    private static final Field WATCH;
    private static final Field STEM_CHECKER;
    private static final Field SYSTEM_BEAMS;
    private static final Field SYSTEM_HEADS;
    private static final Field SYSTEM_STEMS;
    private static final Field UNDEFS;
    private static final Method FINALIZE;
    private static final Constructor<?> PARAMETERS;

    static {
        try {
            INSPECT = method("inspectStems");
            PARAMS = field("params");
            WATCH = field("watch");
            STEM_CHECKER = field("stemChecker");
            SYSTEM_BEAMS = field("systemBeams");
            SYSTEM_HEADS = field("systemHeads");
            SYSTEM_STEMS = field("systemStems");
            UNDEFS = field("undefs");
            FINALIZE = method("finalizeStems");
            Class<?> type = Class.forName("org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS = type.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private StemsHeadFinalizePageProbe() {}

    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("expected one image path");
        CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADS");
        Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        Path path = Path.of(args[0]).toAbsolutePath();
        Book book = new Book(path);
        book.createStubs();
        SheetStub stub = book.getValidStubs().get(0);
        stub.reachStep(OmrStep.HEADS, false);
        Sheet sheet = stub.getSheet();
        String page = path.getFileName() + "#1";
        System.out.printf("stemsheadfinalizepage page %s systems %d mode ForegroundPageSerial%n",
                page, sheet.getSystems().size());
        for (SystemInfo system : sheet.getSystems()) runSystem(page, system);
        System.exit(0);
    }

    @SuppressWarnings("unchecked")
    private static void runSystem(String page, SystemInfo system) throws Exception {
        StemsRetriever retriever = new StemsRetriever(system);
        PARAMS.set(retriever, PARAMETERS.newInstance(system, system.getSheet().getScale()));
        WATCH.set(retriever, new StopWatch("Rust phase-2 page evidence"));
        INSPECT.invoke(retriever);
        STEM_CHECKER.set(retriever, new StemChecker(system.getSheet()));
        SIGraph sig = system.getSig();

        List<Inter> beams = sig.inters(AbstractBeamInter.class);
        Collections.sort(beams, Inters.byReverseWidth);
        SYSTEM_BEAMS.set(retriever, beams);
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
            AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (!beam.getLinker().linkSides(system.getProfile())) iterator.remove();
        }
        for (Inter inter : beams) {
            ((AbstractBeamInter) inter).getLinker().linkStumps(system.getProfile());
        }

        List<Inter> sigHeads = sig.inters(ShapeSet.getTemplateNotesStem(system.getSheet()));
        IdentityHashMap<HeadInter, Integer> sigOrdinals = new IdentityHashMap<>();
        for (int index = 0; index < sigHeads.size(); index++) {
            sigOrdinals.put((HeadInter) sigHeads.get(index), index);
        }
        List<Inter> xHeads = new ArrayList<>(sigHeads);
        Collections.sort(xHeads, Inters.byAbscissa);
        IdentityHashMap<HeadInter, Integer> xOrdinals = new IdentityHashMap<>();
        for (int index = 0; index < xHeads.size(); index++) {
            xOrdinals.put((HeadInter) xHeads.get(index), index);
        }
        List<Inter> heads = new ArrayList<>(sigHeads);
        Collections.sort(heads, Inters.byReverseGrade);
        SYSTEM_HEADS.set(retriever, heads);
        LinkedHashMap<Inter, Set<HorizontalSide>> undefs =
                (LinkedHashMap<Inter, Set<HorizontalSide>>) UNDEFS.get(retriever);
        List<HeadInter> unlinked = new ArrayList<>();
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            if (!head.getLinker().linkSides(
                    Profiles.STRICT, system.getProfile(), undefs, false)) {
                unlinked.add(head);
            }
        }
        System.out.printf(
                "stemsheadphase2baseline page %s system %d heads %d queueSize %d queue %s "
                        + "sigVertices %d sigEdges %d systemStems %d allocator %d%n",
                page, system.getId(), heads.size(), unlinked.size(),
                headList(unlinked, xOrdinals, sigOrdinals), sig.vertexSet().size(),
                sig.edgeSet().size(), ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(),
                system.getSheet().getPersistentIdGenerator().get());

        for (int queueIndex = 0; queueIndex < unlinked.size(); queueIndex++) {
            HeadInter head = unlinked.get(queueIndex);
            List<String> decisions = decisions(head);
            IdentityHashMap<HeadLinker.SLinker, String> beforeSides = sideSnapshot(xHeads);
            int verticesBefore = sig.vertexSet().size();
            int edgesBefore = sig.edgeSet().size();
            int stemsBefore = ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size();
            int allocatorBefore = system.getSheet().getPersistentIdGenerator().get();
            String sidesBefore = headSides(head);
            boolean returned = head.getLinker().linkSides(
                    Profiles.STRICT, system.getProfile(), undefs, true);
            List<String> sideChanges = sideChanges(
                    xHeads, xOrdinals, sigOrdinals, beforeSides);
            System.out.printf(
                    "stemsheadphase2retry page %s system %d queueIndex %d headX %d headSig %d "
                            + "headInterId %d grade %s append true sidesBefore %s decisions %s "
                            + "returned %s sidesAfter %s undefs %s sideChanges %s "
                            + "sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d "
                            + "sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d "
                            + "allocatorBefore %d allocatorAfter %d%n",
                    page, system.getId(), queueIndex, xOrdinals.get(head),
                    sigOrdinals.get(head), head.getId(), hex(head.getGrade()), sidesBefore,
                    compact(decisions), returned, headSides(head),
                    compact(undefs.get(head) == null ? List.of() : undefs.get(head)),
                    compact(sideChanges), verticesBefore, sig.vertexSet().size(), edgesBefore,
                    sig.edgeSet().size(), stemsBefore,
                    ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(), allocatorBefore,
                    system.getSheet().getPersistentIdGenerator().get());
        }
        if (system.getId() == 2) {
            Set<Relation> beforeHeadStem = Collections.newSetFromMap(new IdentityHashMap<>());
            for (Relation relation : sig.edgeSet()) {
                if (relation instanceof HeadStemRelation) beforeHeadStem.add(relation);
            }
            IdentityHashMap<HeadInter, Boolean> abnormalBefore = new IdentityHashMap<>();
            for (Inter inter : heads) abnormalBefore.put((HeadInter) inter, ((HeadInter) inter).isAbnormal());
            int multipleBefore = 0;
            for (Inter inter : heads) {
                if (sig.getRelations((HeadInter) inter, HeadStemRelation.class).size() > 1) multipleBefore++;
            }
            FINALIZE.invoke(retriever);
            int multipleAfter = 0;
            int noStem = 0;
            int abnormal = 0;
            int abnormalChanges = 0;
            for (Inter inter : heads) {
                HeadInter head = (HeadInter) inter;
                int links = sig.getRelations(head, HeadStemRelation.class).size();
                if (links > 1) multipleAfter++;
                if (links == 0) noStem++;
                if (head.isAbnormal()) abnormal++;
                if (abnormalBefore.get(head) != head.isAbnormal()) abnormalChanges++;
            }
            int removed = 0;
            for (Relation relation : beforeHeadStem) if (!sig.containsEdge(relation)) removed++;
            System.out.printf(
                    "stemsheadfinalize page %s system %d checked %d multipleBefore %d multipleAfter %d noStem %d abnormal %d removed %d abnormalChanges %d sigEdges %d systemStems %d allocator %d%n",
                    page, system.getId(), heads.size(), multipleBefore, multipleAfter, noStem, abnormal, removed,
                    abnormalChanges, sig.edgeSet().size(), ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(),
                    system.getSheet().getPersistentIdGenerator().get());
        }
    }

    private static List<String> decisions(HeadInter head) {
        List<String> rows = new ArrayList<>();
        for (HorizontalSide side : HorizontalSide.values()) {
            HeadLinker.SLinker linker = head.getLinker().getSLinkers().get(side);
            if (linker.isLinked()) {
                rows.add(side + ":SkipAlreadyLinked");
                continue;
            }
            var top = linker.getCornerLinker(VerticalSide.TOP);
            var bottom = linker.getCornerLinker(VerticalSide.BOTTOM);
            boolean topOk = top.canLink(Profiles.STRICT, true);
            boolean bottomOk = bottom.canLink(Profiles.STRICT, true);
            String branch = topOk ? (bottomOk ? "Both" : "TopOnly")
                    : (bottomOk ? "BottomOnly" : "Neither");
            rows.add(side + ":top=" + topOk + ":bottom=" + bottomOk + ":branch=" + branch);
        }
        return rows;
    }

    private static IdentityHashMap<HeadLinker.SLinker, String> sideSnapshot(List<Inter> heads) {
        IdentityHashMap<HeadLinker.SLinker, String> result = new IdentityHashMap<>();
        for (Inter inter : heads) {
            for (HeadLinker.SLinker side : ((HeadInter) inter).getLinker().getSLinkers().values()) {
                result.put(side, state(side));
            }
        }
        return result;
    }

    private static List<String> sideChanges(
            List<Inter> heads,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals,
            IdentityHashMap<HeadLinker.SLinker, String> before) {
        List<String> rows = new ArrayList<>();
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            for (Map.Entry<HorizontalSide, HeadLinker.SLinker> entry
                    : head.getLinker().getSLinkers().entrySet()) {
                String prior = before.get(entry.getValue());
                String after = state(entry.getValue());
                if (!prior.equals(after)) {
                    rows.add("x" + xOrdinals.get(head) + ":sig" + sigOrdinals.get(head)
                            + ":" + entry.getKey() + ":" + prior + "->" + after);
                }
            }
        }
        return rows;
    }

    private static String headList(
            List<HeadInter> heads,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals) {
        List<String> rows = new ArrayList<>();
        for (HeadInter head : heads) {
            rows.add("x" + xOrdinals.get(head) + ":sig" + sigOrdinals.get(head)
                    + ":id" + head.getId());
        }
        return compact(rows);
    }

    private static String headSides(HeadInter head) {
        List<String> rows = new ArrayList<>();
        for (Map.Entry<HorizontalSide, HeadLinker.SLinker> entry
                : head.getLinker().getSLinkers().entrySet()) {
            rows.add(entry.getKey() + ":" + state(entry.getValue()));
        }
        return compact(rows);
    }

    private static String state(HeadLinker.SLinker linker) {
        return linker.isLinked() + ":" + linker.isClosed();
    }

    private static String compact(Object value) {
        return value.toString().replace(" ", "");
    }

    private static String hex(double value) {
        return Long.toHexString(Double.doubleToRawLongBits(value));
    }

    private static Method method(String name) throws ReflectiveOperationException {
        Method method = StemsRetriever.class.getDeclaredMethod(name);
        method.setAccessible(true);
        return method;
    }

    private static Field field(String name) throws ReflectiveOperationException {
        Field field = StemsRetriever.class.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }
}
