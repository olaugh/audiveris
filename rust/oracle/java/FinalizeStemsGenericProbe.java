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
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.StopWatch;

/** Focused executable evidence for generic StemsRetriever.finalizeStems mutations. */
public final class FinalizeStemsGenericProbe {
    private static final Method INSPECT;
    private static final Method LINK;
    private static final Method FINALIZE;
    private static final Field PARAMS;
    private static final Field HEADS;
    private static final Field UNDEFS;
    private static final Field WATCH;
    private static final Constructor<?> PARAMETERS;

    static {
        try {
            INSPECT = method("inspectStems");
            LINK = method("linkStems");
            FINALIZE = method("finalizeStems");
            PARAMS = field("params");
            HEADS = field("systemHeads");
            UNDEFS = field("undefs");
            WATCH = field("watch");
            Class<?> type = Class.forName("org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS = type.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private FinalizeStemsGenericProbe() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0 || args.length % 3 != 0) {
            throw new IllegalArgumentException("expected image/system/synthetic triples");
        }
        CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADS");
        Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        for (int index = 0; index < args.length; index += 3) {
            runCase(args[index], args[index + 1], args[index + 2]);
        }
        System.exit(0);
    }

    private static void runCase(String image, String systemValue, String syntheticValue)
            throws Exception {
        Path path = Path.of(image).toAbsolutePath();
        Book book = new Book(path);
        book.createStubs();
        SheetStub stub = book.getValidStubs().get(0);
        stub.reachStep(OmrStep.HEADS, false);
        Sheet sheet = stub.getSheet();
        int systemId = Integer.parseInt(systemValue);
        SystemInfo system = sheet.getSystems().stream()
                .filter(candidate -> candidate.getId() == systemId)
                .findFirst().orElseThrow();
        runSystem(path.getFileName() + "#1", system, Boolean.parseBoolean(syntheticValue));
    }

    @SuppressWarnings("unchecked")
    private static void runSystem(String page, SystemInfo system, boolean synthesizeAbnormal)
            throws Exception {
        StemsRetriever retriever = new StemsRetriever(system);
        PARAMS.set(retriever, PARAMETERS.newInstance(system, system.getSheet().getScale()));
        WATCH.set(retriever, new StopWatch("generic finalizeStems fixture"));
        INSPECT.invoke(retriever);
        LINK.invoke(retriever);

        List<Inter> heads = (List<Inter>) HEADS.get(retriever);
        LinkedHashMap<Inter, Set<HorizontalSide>> undefs =
                (LinkedHashMap<Inter, Set<HorizontalSide>>) UNDEFS.get(retriever);
        SIGraph sig = system.getSig();
        IdentityHashMap<Relation, String> relationBefore = new IdentityHashMap<>();
        HeadInter synthetic = null;

        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            List<Inter> stems = new ArrayList<>();
            List<HeadStemRelation> relations = new ArrayList<>();
            for (Relation relation : sig.getRelations(head, HeadStemRelation.class)) {
                HeadStemRelation headStem = (HeadStemRelation) relation;
                StemInter stem = (StemInter) sig.getOppositeInter(head, relation);
                stems.add(stem);
                relations.add(headStem);
                relationBefore.put(relation, relationToken(head, stem, headStem));
            }
            if (stems.size() > 1) {
                List<List<Inter>> partitions = sig.getPartitions(null, stems);
                System.out.printf(
                        "finalizegeneric before page %s system %d head %d shape %s center %d,%d "
                                + "grade %s abnormal %s relations %s partitions %s canonical %s%n",
                        page, system.getId(), head.getId(), head.getShape(), head.getCenter().x,
                        head.getCenter().y, hex(head.getGrade()), head.isAbnormal(),
                        relationRows(sig, head, relations), partitionRows(partitions),
                        canonical(sig, head, relations));
            } else if (synthesizeAbnormal && synthetic == null && stems.isEmpty()
                    && org.audiveris.omr.glyph.ShapeSet.StemHeads.contains(head.getShape())) {
                synthetic = head;
                head.setAbnormal(false);
                System.out.printf(
                        "finalizegeneric synthetic page %s system %d head %d shape %s "
                                + "abnormal true->false relations 0 undefs %s%n",
                        page, system.getId(), head.getId(), head.getShape(),
                        compact(undefs.get(head) == null ? List.of() : undefs.get(head)));
            }
        }

        IdentityHashMap<Inter, Boolean> abnormalBefore = new IdentityHashMap<>();
        for (Inter inter : sig.vertexSet()) abnormalBefore.put(inter, inter.isAbnormal());

        FINALIZE.invoke(retriever);
        List<String> removed = new ArrayList<>();
        for (Map.Entry<Relation, String> entry : relationBefore.entrySet()) {
            if (!sig.edgeSet().contains(entry.getKey())) removed.add(entry.getValue());
        }
        Collections.sort(removed);
        List<String> abnormal = new ArrayList<>();
        for (Inter inter : sig.vertexSet()) {
            if (abnormalBefore.get(inter) != null
                    && abnormalBefore.get(inter) != inter.isAbnormal()) {
                abnormal.add(inter.getClass().getSimpleName() + inter.getId() + ":"
                        + abnormalBefore.get(inter)
                        + "->" + inter.isAbnormal());
            }
        }
        Collections.sort(abnormal);
        System.out.printf(
                "finalizegeneric after page %s system %d removed %s abnormal %s syntheticHead %s%n",
                page, system.getId(), compact(removed), compact(abnormal),
                synthetic == null ? "-" : Integer.toString(synthetic.getId()));
    }

    private static String relationRows(SIGraph sig, HeadInter head,
                                       List<HeadStemRelation> relations) {
        List<String> rows = new ArrayList<>();
        for (HeadStemRelation relation : relations) {
            StemInter stem = (StemInter) sig.getOppositeInter(head, relation);
            rows.add(relationToken(head, stem, relation));
        }
        return compact(rows);
    }

    private static String relationToken(HeadInter head, StemInter stem,
                                        HeadStemRelation relation) {
        double contribution = stem.getGrade() * (relation.getTargetRatio() - 1.0);
        return "head" + head.getId() + ":stem" + stem.getId()
                + ":side" + relation.getHeadSide()
                + ":stemGrade" + hex(stem.getGrade())
                + ":relationGrade" + hex(relation.getGrade())
                + ":targetRatio" + hex(relation.getTargetRatio())
                + ":contribution" + hex(contribution)
                + ":dy" + hex(relation.getDy())
                + ":extensionY" + hex(relation.getExtensionPoint().getY());
    }

    private static String partitionRows(List<List<Inter>> partitions) {
        List<String> rows = new ArrayList<>();
        for (List<Inter> partition : partitions) {
            List<Integer> ids = new ArrayList<>();
            for (Inter inter : partition) ids.add(inter.getId());
            rows.add(compact(ids));
        }
        return compact(rows);
    }

    private static String canonical(SIGraph sig, HeadInter head,
                                    List<HeadStemRelation> relations) {
        if (relations.size() != 2 || relations.stream().anyMatch(relation -> relation.getDy() > 0.2)) {
            return "false";
        }
        HeadStemRelation left = relations.stream()
                .filter(relation -> relation.getHeadSide() == HorizontalSide.LEFT)
                .findFirst().orElse(null);
        HeadStemRelation right = relations.stream()
                .filter(relation -> relation.getHeadSide() == HorizontalSide.RIGHT)
                .findFirst().orElse(null);
        return Boolean.toString(left != null && right != null
                && HeadStemRelation.isCanonicalShare(left, head, right));
    }

    private static String compact(Object value) {
        return String.valueOf(value).replace(" ", "");
    }

    private static String hex(double value) {
        return Double.toHexString(value) + "/" + Long.toHexString(Double.doubleToRawLongBits(value));
    }

    private static Field field(String name) throws ReflectiveOperationException {
        Field field = StemsRetriever.class.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Method method(String name) throws ReflectiveOperationException {
        Method method = StemsRetriever.class.getDeclaredMethod(name);
        method.setAccessible(true);
        return method;
    }
}
