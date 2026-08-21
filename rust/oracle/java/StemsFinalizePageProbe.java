// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.StopWatch;

/** Deterministic page-wide evidence for Java's generic {@code finalizeStems}. */
public final class StemsFinalizePageProbe {
    private static final Method INSPECT;
    private static final Method LINK;
    private static final Method FINALIZE;
    private static final Field PARAMS;
    private static final Field WATCH;
    private static final Field SYSTEM_STEMS;
    private static final Field UNDEFS;
    private static final Constructor<?> PARAMETERS;

    static {
        try {
            INSPECT = method("inspectStems");
            LINK = method("linkStems");
            FINALIZE = method("finalizeStems");
            PARAMS = field("params");
            WATCH = field("watch");
            SYSTEM_STEMS = field("systemStems");
            UNDEFS = field("undefs");
            Class<?> type = Class.forName("org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS = type.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private StemsFinalizePageProbe() {}

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
        System.out.printf("stemsfinalizepage page %s systems %d mode ForegroundPageSerial%n",
                page, sheet.getSystems().size());
        for (SystemInfo system : sheet.getSystems()) runSystem(page, system);
        System.exit(0);
    }

    @SuppressWarnings("unchecked")
    private static void runSystem(String page, SystemInfo system) throws Exception {
        StemsRetriever retriever = new StemsRetriever(system);
        PARAMS.set(retriever, PARAMETERS.newInstance(system, system.getSheet().getScale()));
        WATCH.set(retriever, new StopWatch("Rust page finalizeStems evidence"));
        INSPECT.invoke(retriever);

        SIGraph sig = system.getSig();
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

        LINK.invoke(retriever);
        LinkedHashMap<Inter, Set<HorizontalSide>> undefs =
                (LinkedHashMap<Inter, Set<HorizontalSide>>) UNDEFS.get(retriever);
        IdentityHashMap<Relation, String> relationsBefore = relationSnapshot(
                sig, xHeads, xOrdinals, sigOrdinals);
        List<String> multipleBefore = headsWithStemCount(
                sig, xHeads, xOrdinals, sigOrdinals, 2, true);
        List<String> noStemBefore = headsWithStemCount(
                sig, xHeads, xOrdinals, sigOrdinals, 0, false);
        List<String> abnormalBefore = abnormalHeads(xHeads, xOrdinals, sigOrdinals);
        int verticesBefore = sig.vertexSet().size();
        int edgesBefore = sig.edgeSet().size();
        int stemsBefore = ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size();
        int allocatorBefore = system.getSheet().getPersistentIdGenerator().get();

        FINALIZE.invoke(retriever);

        List<String> removed = new ArrayList<>();
        for (Map.Entry<Relation, String> entry : relationsBefore.entrySet()) {
            if (!sig.edgeSet().contains(entry.getKey())) removed.add(entry.getValue());
        }
        Collections.sort(removed);
        List<String> abnormalAfter = abnormalHeads(xHeads, xOrdinals, sigOrdinals);
        List<String> abnormalChanges = new ArrayList<>();
        for (Inter inter : xHeads) {
            HeadInter head = (HeadInter) inter;
            String alias = headAlias(head, xOrdinals, sigOrdinals);
            boolean before = abnormalBefore.contains(alias);
            if (before != head.isAbnormal()) {
                abnormalChanges.add(alias + ":" + before + "->" + head.isAbnormal());
            }
        }
        System.out.printf(
                "stemsfinalizesystem page %s system %d heads %d undefs %s "
                        + "multipleBefore %s noStemBefore %s abnormalBefore %s "
                        + "removedHeadStem %s abnormalAfter %s abnormalChanges %s "
                        + "sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d "
                        + "sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d "
                        + "allocatorBefore %d allocatorAfter %d%n",
                page, system.getId(), xHeads.size(), undefRows(undefs, xOrdinals, sigOrdinals),
                compact(multipleBefore), compact(noStemBefore), compact(abnormalBefore),
                compact(removed), compact(abnormalAfter), compact(abnormalChanges),
                verticesBefore, sig.vertexSet().size(), edgesBefore, sig.edgeSet().size(),
                stemsBefore, ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(), allocatorBefore,
                system.getSheet().getPersistentIdGenerator().get());
    }

    private static IdentityHashMap<Relation, String> relationSnapshot(
            SIGraph sig,
            List<Inter> heads,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals) {
        IdentityHashMap<Relation, String> rows = new IdentityHashMap<>();
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            for (Relation relation : sig.getRelations(head, HeadStemRelation.class)) {
                HeadStemRelation headStem = (HeadStemRelation) relation;
                StemInter stem = (StemInter) sig.getOppositeInter(head, relation);
                rows.put(relation, headAlias(head, xOrdinals, sigOrdinals) + ":stem"
                        + stem.getId() + ":side" + headStem.getHeadSide());
            }
        }
        return rows;
    }

    private static List<String> headsWithStemCount(
            SIGraph sig,
            List<Inter> heads,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals,
            int count,
            boolean atLeast) {
        List<String> rows = new ArrayList<>();
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            int actual = sig.getRelations(head, HeadStemRelation.class).size();
            if ((atLeast && actual >= count) || (!atLeast && actual == count)) {
                rows.add(headAlias(head, xOrdinals, sigOrdinals));
            }
        }
        return rows;
    }

    private static List<String> abnormalHeads(
            List<Inter> heads,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals) {
        List<String> rows = new ArrayList<>();
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            if (head.isAbnormal()) rows.add(headAlias(head, xOrdinals, sigOrdinals));
        }
        return rows;
    }

    private static String undefRows(
            LinkedHashMap<Inter, Set<HorizontalSide>> undefs,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals) {
        List<String> rows = new ArrayList<>();
        for (Map.Entry<Inter, Set<HorizontalSide>> entry : undefs.entrySet()) {
            rows.add(headAlias((HeadInter) entry.getKey(), xOrdinals, sigOrdinals)
                    + ":" + compact(entry.getValue()));
        }
        return compact(rows);
    }

    private static String headAlias(
            HeadInter head,
            IdentityHashMap<HeadInter, Integer> xOrdinals,
            IdentityHashMap<HeadInter, Integer> sigOrdinals) {
        return "x" + xOrdinals.get(head) + ":sig" + sigOrdinals.get(head) + ":id" + head.getId();
    }

    private static String compact(Object value) {
        return value.toString().replace(" ", "");
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
