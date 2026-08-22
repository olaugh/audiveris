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
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.StopWatch;
import org.audiveris.omr.util.VerticalSide;

/** Identity-free Bach system-2 evidence for phase-1 queue 183 reconciliation. */
public final class StemsHeadPhaseOneBachSystem2Order183Probe {
    private static final int TARGET_SYSTEM = 2;
    private static final int TARGET_ORDER = 183;
    private static final Method INSPECT;
    private static final Field PARAMS;
    private static final Field WATCH;
    private static final Field STEM_CHECKER;
    private static final Field SYSTEM_BEAMS;
    private static final Field SYSTEM_HEADS;
    private static final Field SYSTEM_STEMS;
    private static final Field UNDEFS;
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
            Class<?> type = Class.forName("org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS = type.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private StemsHeadPhaseOneBachSystem2Order183Probe() {}

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
        runSystem(path.getFileName() + "#1", sheet.getSystems().get(TARGET_SYSTEM - 1));
        System.exit(0);
    }

    @SuppressWarnings("unchecked")
    private static void runSystem(String page, SystemInfo system) throws Exception {
        StemsRetriever retriever = new StemsRetriever(system);
        PARAMS.set(retriever, PARAMETERS.newInstance(system, system.getSheet().getScale()));
        WATCH.set(retriever, new StopWatch("Rust Bach system-2 queue-183 evidence"));
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
        IdentityHashMap<HeadInter, Integer> sigOrdinals = ordinals(sigHeads);
        List<Inter> xHeads = new ArrayList<>(sigHeads);
        Collections.sort(xHeads, Inters.byAbscissa);
        IdentityHashMap<HeadInter, Integer> xOrdinals = ordinals(xHeads);
        List<Inter> heads = new ArrayList<>(sigHeads);
        Collections.sort(heads, Inters.byReverseGrade);
        SYSTEM_HEADS.set(retriever, heads);
        LinkedHashMap<Inter, Set<HorizontalSide>> undefs =
                (LinkedHashMap<Inter, Set<HorizontalSide>>) UNDEFS.get(retriever);

        for (int order = 0; order < heads.size(); order++) {
            HeadInter head = (HeadInter) heads.get(order);
            if (order != TARGET_ORDER) {
                head.getLinker().linkSides(Profiles.STRICT, system.getProfile(), undefs, false);
                continue;
            }
            if (xOrdinals.get(head) != 62 || sigOrdinals.get(head) != 99) {
                throw new IllegalStateException("Bach system-2 queue-183 identity drifted");
            }
            for (int profile = Profiles.STRICT;
                    profile <= Profiles.RATHER_GOOD_HEAD; profile++) {
                System.out.printf(
                        "stemsheadbachs2q183profile page %s system %d headOrder %d headX %d "
                                + "headSig %d grade %s stemProfile %d decisions %s%n",
                        page, system.getId(), order, xOrdinals.get(head), sigOrdinals.get(head),
                        hex(head.getGrade()), profile, compact(decisions(head, profile)));
            }

            IdentityHashMap<HeadLinker.SLinker, String> sidesBefore = sideSnapshot(xHeads);
            int verticesBefore = sig.vertexSet().size();
            int edgesBefore = sig.edgeSet().size();
            int stemsBefore = ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size();
            int allocatorBefore = system.getSheet().getPersistentIdGenerator().get();
            boolean returned = head.getLinker().linkSides(
                    Profiles.STRICT, system.getProfile(), undefs, false);

            List<String> incidents = new ArrayList<>();
            for (var relation : sig.getRelations(head, HeadStemRelation.class)) {
                StemInter stem = (StemInter) sig.getOppositeInter(head, relation);
                List<String> stemHeads = new ArrayList<>();
                for (var stemRelation : sig.getRelations(stem, HeadStemRelation.class)) {
                    HeadInter stemHead = (HeadInter) sig.getOppositeInter(stem, stemRelation);
                    stemHeads.add("x" + xOrdinals.get(stemHead)
                            + ":sig" + sigOrdinals.get(stemHead)
                            + ":side" + ((HeadStemRelation) stemRelation).getHeadSide());
                }
                Collections.sort(stemHeads);
                incidents.add("existingStem:headSide"
                        + ((HeadStemRelation) relation).getHeadSide()
                        + ":heads" + compact(stemHeads));
            }
            Collections.sort(incidents);

            HeadInter next = (HeadInter) heads.get(order + 1);
            System.out.printf(
                    "stemsheadbachs2q183result page %s system %d headOrder %d headX %d headSig %d "
                            + "returned %s undefs %s sideChanges %s incidents %s "
                            + "sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d "
                            + "sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d "
                            + "allocatorUnchanged %s nextHeadOrder %d nextHeadX %d nextHeadSig %d%n",
                    page, system.getId(), order, xOrdinals.get(head), sigOrdinals.get(head),
                    returned, compact(undefs.get(head) == null ? List.of() : undefs.get(head)),
                    compact(sideChanges(xHeads, xOrdinals, sigOrdinals, sidesBefore)),
                    compact(incidents), verticesBefore, sig.vertexSet().size(), edgesBefore,
                    sig.edgeSet().size(), stemsBefore,
                    ((Map<?, ?>) SYSTEM_STEMS.get(retriever)).size(),
                    allocatorBefore == system.getSheet().getPersistentIdGenerator().get(), order + 1,
                    xOrdinals.get(next), sigOrdinals.get(next));
            return;
        }
        throw new IllegalStateException("Bach system-2 queue 183 was not reached");
    }

    private static List<String> decisions(HeadInter head, int profile) {
        List<String> rows = new ArrayList<>();
        for (HorizontalSide side : HorizontalSide.values()) {
            HeadLinker.SLinker linker = head.getLinker().getSLinkers().get(side);
            if (linker.isLinked()) rows.add(side + ":SkipAlreadyLinked");
            else if (linker.isClosed()) rows.add(side + ":SkipClosed");
            else {
                boolean top = linker.getCornerLinker(VerticalSide.TOP).canLink(profile, false);
                boolean bottom = linker.getCornerLinker(VerticalSide.BOTTOM).canLink(profile, false);
                String branch = top ? (bottom ? "Both" : "TopOnly")
                        : (bottom ? "BottomOnly" : "Neither");
                rows.add(side + ":top=" + top + ":bottom=" + bottom + ":branch=" + branch);
            }
        }
        return rows;
    }

    private static IdentityHashMap<HeadInter, Integer> ordinals(List<Inter> heads) {
        IdentityHashMap<HeadInter, Integer> result = new IdentityHashMap<>();
        for (int index = 0; index < heads.size(); index++) {
            result.put((HeadInter) heads.get(index), index);
        }
        return result;
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

    private static String state(HeadLinker.SLinker linker) {
        return linker.isLinked() + ":" + linker.isClosed();
    }

    private static String compact(Object value) {
        return value.toString().replace(" ", "");
    }

    private static String hex(double value) {
        return String.format("%a/%016x", value, Double.doubleToRawLongBits(value));
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
