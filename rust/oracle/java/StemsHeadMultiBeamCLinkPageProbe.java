// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemBuilder;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemHalfLinker;
import org.audiveris.omr.sheet.stem.StemItem;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.AbstractConnection;
import org.audiveris.omr.sig.relation.AbstractStemConnection;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.sig.relation.Support;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.StopWatch;
import org.audiveris.omr.util.VerticalSide;

/** Deterministic full-lifecycle evidence for Bach system-2 queue-182 multi-beam C-link. */
@SuppressWarnings("unchecked")
public final class StemsHeadMultiBeamCLinkPageProbe {
    private static final int TARGET_SYSTEM = 2;
    private static final int TARGET_ORDER = 182;
    private static final Method INSPECT;
    private static final Field PARAMS;
    private static final Field WATCH;
    private static final Field STEM_CHECKER;
    private static final Field SYSTEM_BEAMS;
    private static final Field SYSTEM_HEADS;
    private static final Field SYSTEM_STEMS;
    private static final Field UNDEFS;
    private static final Constructor<?> PARAMETERS;
    private static final Class<?> C_LINKER_CLASS;
    private static final Class<?> B_LINKER_CLASS;
    private static final Method C_EXPAND;
    private static final Field C_REF_PT;
    private static final Field C_Y_DIR;
    private static final Field C_STEM_BUILDER;
    private static final Field PARAMETERS_MIN_STEM_TAIL_LG;
    private static final Field PARAMETERS_BEST_STEM_TAIL_LG;
    private static final Field STEM_BUILDER_THEO_LINE;
    private static final Field STEM_ITEM_GLYPH;
    private static final Field LINKER_ITEM_LINKER;
    private static final Field LINKER_ALL_B;
    private static final Field B_ID;
    private static final Method UPDATE_STEM_LINE;

    static {
        try {
            INSPECT = method(StemsRetriever.class, "inspectStems");
            PARAMS = field(StemsRetriever.class, "params");
            WATCH = field(StemsRetriever.class, "watch");
            STEM_CHECKER = field(StemsRetriever.class, "stemChecker");
            SYSTEM_BEAMS = field(StemsRetriever.class, "systemBeams");
            SYSTEM_HEADS = field(StemsRetriever.class, "systemHeads");
            SYSTEM_STEMS = field(StemsRetriever.class, "systemStems");
            UNDEFS = field(StemsRetriever.class, "undefs");
            Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS.setAccessible(true);
            PARAMETERS_MIN_STEM_TAIL_LG = field(parameters, "minStemTailLg");
            PARAMETERS_BEST_STEM_TAIL_LG = field(parameters, "bestStemTailLg");
            C_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker");
            B_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            C_EXPAND = C_LINKER_CLASS.getDeclaredMethod(
                    "expand", double.class, double.class, int.class, int.class,
                    Map.class, Set.class);
            C_EXPAND.setAccessible(true);
            C_REF_PT = field(C_LINKER_CLASS, "refPt");
            C_Y_DIR = field(C_LINKER_CLASS, "yDir");
            C_STEM_BUILDER = field(C_LINKER_CLASS, "sb");
            STEM_BUILDER_THEO_LINE = field(StemBuilder.class, "theoLine");
            STEM_ITEM_GLYPH = field(StemItem.class, "glyph");
            LINKER_ITEM_LINKER = field(StemItem.LinkerItem.class, "linker");
            LINKER_ALL_B = field(BeamLinker.class, "allBLinkers");
            B_ID = field(B_LINKER_CLASS, "id");
            UPDATE_STEM_LINE = StemHalfLinker.class.getDeclaredMethod(
                    "updateStemLine", Glyph.class, Set.class, Line2D.class, Double.class);
            UPDATE_STEM_LINE.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private StemsHeadMultiBeamCLinkPageProbe() {}

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
        runSystem(path.getFileName() + "#1", stub.getSheet(),
                stub.getSheet().getSystems().get(TARGET_SYSTEM - 1));
        System.exit(0);
    }

    private static void runSystem(String page, Sheet sheet, SystemInfo system) throws Exception {
        StemsRetriever retriever = new StemsRetriever(system);
        Object params = PARAMETERS.newInstance(system, sheet.getScale());
        PARAMS.set(retriever, params);
        WATCH.set(retriever, new StopWatch("Rust multi-beam C-link evidence"));
        INSPECT.invoke(retriever);
        STEM_CHECKER.set(retriever, new StemChecker(sheet));
        SIGraph sig = system.getSig();

        List<Inter> beams = sig.inters(AbstractBeamInter.class);
        IdentityHashMap<Inter, Integer> beamSigOrdinals = ordinals(beams);
        Collections.sort(beams, Inters.byReverseWidth);
        SYSTEM_BEAMS.set(retriever, beams);
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
            AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (!beam.getLinker().linkSides(system.getProfile())) iterator.remove();
        }
        for (Inter inter : beams) {
            ((AbstractBeamInter) inter).getLinker().linkStumps(system.getProfile());
        }

        List<Inter> sigHeads = sig.inters(ShapeSet.getTemplateNotesStem(sheet));
        IdentityHashMap<Inter, Integer> sigOrdinals = ordinals(sigHeads);
        List<Inter> xHeads = new ArrayList<>(sigHeads);
        Collections.sort(xHeads, Inters.byAbscissa);
        IdentityHashMap<Inter, Integer> xOrdinals = ordinals(xHeads);
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
            if (xOrdinals.get(head) != 138 || sigOrdinals.get(head) != 149) {
                throw new IllegalStateException("Bach multi-beam frontier identity drifted");
            }
            HeadLinker.SLinker side = head.getLinker().getSLinkers().get(HorizontalSide.LEFT);
            Object corner = side.getCornerLinker(VerticalSide.BOTTOM);
            if (!side.getCornerLinker(VerticalSide.BOTTOM).canLink(Profiles.STRICT, false)) {
                throw new IllegalStateException("Bach multi-beam corner no longer links at STRICT profile");
            }
            Point2D refPt = (Point2D) C_REF_PT.get(corner);
            int yDir = C_Y_DIR.getInt(corner);
            int minTail = PARAMETERS_MIN_STEM_TAIL_LG.getInt(params);
            int bestTail = PARAMETERS_BEST_STEM_TAIL_LG.getInt(params);
            double yHard = refPt.getY() + yDir * minTail;
            double ySoft = refPt.getY() + yDir * bestTail;
            StemBuilder builder = (StemBuilder) C_STEM_BUILDER.get(corner);
            Line2D storedLine = (Line2D) STEM_BUILDER_THEO_LINE.get(builder);
            Line2D originalLine = copy(storedLine);

            LinkedHashSet<Glyph> replayGlyphs = new LinkedHashSet<>();
            Line2D replayLine = yDir > 0 ? copy(originalLine)
                    : new Line2D.Double(originalLine.getP2(), originalLine.getP1());
            List<String> itemRows = new ArrayList<>();
            for (int index = 0; index <= builder.maxIndex(); index++) {
                StemItem item = builder.get(index);
                Glyph glyph = (Glyph) STEM_ITEM_GLYPH.get(item);
                String linker = "-";
                if (item instanceof StemItem.LinkerItem) {
                    linker = linkerAlias((StemLinker) LINKER_ITEM_LINKER.get(item),
                            beamSigOrdinals, xOrdinals);
                }
                String beforeLine = line(replayLine);
                if (glyph != null) {
                    UPDATE_STEM_LINE.invoke(null, glyph, replayGlyphs, replayLine, null);
                }
                itemRows.add(index + ":" + item.getClass().getSimpleName()
                        + ":linker=" + linker
                        + ":glyph=" + (glyph != null ? glyphToken(glyph) : "-")
                        + ":before=" + beforeLine + ":after=" + line(replayLine));
            }

            LinkedHashMap<StemLinker, Relation> planRelations = new LinkedHashMap<>();
            LinkedHashSet<Glyph> planGlyphs = new LinkedHashSet<>();
            int lastIndex = (Integer) C_EXPAND.invoke(
                    corner, yHard, ySoft, Profiles.STRICT, system.getProfile(),
                    planRelations, planGlyphs);
            Glyph candidate = planGlyphs.size() == 1
                    ? planGlyphs.iterator().next() : GlyphFactory.buildGlyph(planGlyphs);
            List<String> relationRows = new ArrayList<>();
            for (Map.Entry<StemLinker, Relation> entry : planRelations.entrySet()) {
                relationRows.add(linkerAlias(entry.getKey(), beamSigOrdinals, xOrdinals)
                        + ":state=" + entry.getKey().isLinked() + ":"
                        + entry.getKey().isClosed() + ":" + relationState(entry.getValue()));
            }
            List<String> glyphRows = new ArrayList<>();
            for (Glyph glyph : planGlyphs) {
                glyphRows.add("id" + glyph.getId() + ":" + glyphToken(glyph));
            }
            Map<Glyph, StemInter> stems = (Map<Glyph, StemInter>) SYSTEM_STEMS.get(retriever);
            StemInter existingStem = stems.get(candidate);
            List<String> existingRelationRows = new ArrayList<>();
            if (existingStem != null) {
                for (Map.Entry<StemLinker, Relation> entry : planRelations.entrySet()) {
                    if (!B_LINKER_CLASS.isInstance(entry.getKey())) continue;
                    Inter beam = entry.getKey().getSource();
                    BeamStemRelation existing = (BeamStemRelation) sig.getRelation(
                            beam, existingStem, BeamStemRelation.class);
                    existingRelationRows.add(
                            linkerAlias(entry.getKey(), beamSigOrdinals, xOrdinals)
                                    + ":" + (existing != null
                                            ? relationState(existing) : "missing"));
                }
            }
            System.out.printf(
                    "stemsheadmultibeamfrontier page %s system %d headOrder %d headX %d "
                            + "headSig %d headInterId %d grade %s stemProfile %d "
                            + "cAlias h:138:LEFT:BOTTOM "
                            + "refPt %s yDir %d minTail %d bestTail %d yHard %s ySoft %s "
                            + "lastIndex %d maxIndex %d itemRows %s relations %d relationRows %s "
                            + "glyphs %d selected %s candidateIdBefore %d candidate %s "
                            + "existingCandidateStem %s existingStem %s existingBeamRelations %s "
                            + "initialLine %s finalLine %s terminal ReadyForMultiBeamCLink%n",
                    page, system.getId(), order, xOrdinals.get(head), sigOrdinals.get(head),
                    head.getId(), hex(head.getGrade()), Profiles.STRICT, point(refPt), yDir,
                    minTail, bestTail,
                    hex(yHard), hex(ySoft), lastIndex, builder.maxIndex(), compact(itemRows),
                    planRelations.size(), compact(relationRows), planGlyphs.size(),
                    compact(glyphRows), candidate.getId(), glyphToken(candidate),
                    existingStem != null,
                    existingStem != null ? interToken(existingStem) : "-",
                    compact(existingRelationRows),
                    line(originalLine), line(storedLine));

            storedLine.setLine(originalLine);
            IdentityHashMap<Inter, Boolean> verticesBefore = identities(sig.vertexSet());
            IdentityHashMap<Relation, Boolean> edgesBefore = identities(sig.edgeSet());
            IdentityHashMap<Glyph, Boolean> stemsBefore = identities(stems.keySet());
            IdentityHashMap<StemLinker, String> linkerBefore = linkerStates(beams, xHeads);
            int allocatorBefore = sheet.getPersistentIdGenerator().get();
            boolean returned = head.getLinker().linkSides(
                    Profiles.STRICT, system.getProfile(), undefs, false);

            List<String> addedVertices = new ArrayList<>();
            for (Inter inter : sig.vertexSet()) {
                if (!verticesBefore.containsKey(inter)) {
                    addedVertices.add("id" + inter.getId() + ":" + interToken(inter));
                }
            }
            List<String> addedEdges = new ArrayList<>();
            for (Relation relation : sig.edgeSet()) {
                if (!edgesBefore.containsKey(relation)) {
                    addedEdges.add("source=" + interAlias(
                                    sig.getEdgeSource(relation), xOrdinals,
                                    existingStem)
                            + ":target=" + interAlias(
                                    sig.getEdgeTarget(relation), xOrdinals,
                                    existingStem)
                            + ":" + relationState(relation));
                }
            }
            List<String> addedStems = new ArrayList<>();
            for (Map.Entry<Glyph, StemInter> entry : stems.entrySet()) {
                if (!stemsBefore.containsKey(entry.getKey())) {
                    addedStems.add("glyphId" + entry.getKey().getId() + ":"
                            + glyphToken(entry.getKey()) + ":stemId" + entry.getValue().getId());
                }
            }
            List<String> linkerChanges = new ArrayList<>();
            IdentityHashMap<StemLinker, String> linkerAfter = linkerStates(beams, xHeads);
            for (Map.Entry<StemLinker, String> entry : linkerBefore.entrySet()) {
                String after = linkerAfter.get(entry.getKey());
                if (!entry.getValue().equals(after)) {
                    linkerChanges.add(linkerAlias(entry.getKey(), beamSigOrdinals, xOrdinals)
                            + ":" + entry.getValue() + "->" + after);
                }
            }
            Collections.sort(addedVertices);
            Collections.sort(addedEdges);
            Collections.sort(addedStems);
            Collections.sort(linkerChanges);
            HeadInter next = (HeadInter) heads.get(order + 1);
            System.out.printf(
                    "stemsheadmultibeamresult page %s system %d headOrder %d returned %s "
                            + "undefs %s allocatorDelta %d "
                            + "sigVerticesBefore %d sigVerticesAfter %d sigEdgesBefore %d "
                            + "sigEdgesAfter %d systemStemsBefore %d systemStemsAfter %d "
                            + "addedVertices %s addedEdges %s addedSystemStems %s linkerChanges %s "
                            + "nextHeadOrder %d nextHeadX %d nextHeadSig %d nextHeadInterId %d "
                            + "terminal ReturnedMultiBeamCLinkTransaction%n",
                    page, system.getId(), order, returned,
                    compact(undefs.get(head) == null ? List.of() : undefs.get(head)),
                    sheet.getPersistentIdGenerator().get() - allocatorBefore,
                    verticesBefore.size(), sig.vertexSet().size(), edgesBefore.size(),
                    sig.edgeSet().size(), stemsBefore.size(), stems.size(), compact(addedVertices),
                    compact(addedEdges), compact(addedStems), compact(linkerChanges), order + 1,
                    xOrdinals.get(next), sigOrdinals.get(next), next.getId());
            return;
        }
        throw new IllegalStateException("Bach multi-beam frontier was not reached");
    }

    private static IdentityHashMap<StemLinker, String> linkerStates(
            List<Inter> beams, List<Inter> heads) throws Exception {
        IdentityHashMap<StemLinker, String> states = new IdentityHashMap<>();
        for (Inter inter : beams) {
            for (Object value : (List<Object>) LINKER_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker())) {
                StemLinker linker = (StemLinker) value;
                states.put(linker, linker.isLinked() + ":" + linker.isClosed());
            }
        }
        for (Inter inter : heads) {
            HeadInter head = (HeadInter) inter;
            for (HeadLinker.SLinker s : head.getLinker().getSLinkers().values()) {
                states.put(s, s.isLinked() + ":" + s.isClosed());
                for (VerticalSide vertical : VerticalSide.values()) {
                    StemLinker c = s.getCornerLinker(vertical);
                    states.put(c, c.isLinked() + ":" + c.isClosed());
                }
            }
        }
        return states;
    }

    private static String linkerAlias(
            StemLinker linker,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<Inter, Integer> headXOrdinals) throws Exception {
        if (B_LINKER_CLASS.isInstance(linker)) {
            Inter beam = linker.getSource();
            return "beam:sig" + beamSigOrdinals.get(beam) + ":inter" + beam.getId()
                    + ":b" + B_ID.getInt(linker);
        }
        if (C_LINKER_CLASS.isInstance(linker)) {
            HeadInter head = (HeadInter) linker.getSource();
            for (Map.Entry<HorizontalSide, HeadLinker.SLinker> side
                    : head.getLinker().getSLinkers().entrySet()) {
                for (VerticalSide vertical : VerticalSide.values()) {
                    if (side.getValue().getCornerLinker(vertical) == linker) {
                        return "head:x" + headXOrdinals.get(head) + ":inter" + head.getId()
                                + ":" + side.getKey() + ":" + vertical;
                    }
                }
            }
            return "head:x" + headXOrdinals.get(head) + ":inter" + head.getId();
        }
        if (linker instanceof HeadLinker.SLinker s) {
            HeadInter head = (HeadInter) linker.getSource();
            for (Map.Entry<HorizontalSide, HeadLinker.SLinker> side
                    : head.getLinker().getSLinkers().entrySet()) {
                if (side.getValue() == s) {
                    return "head:x" + headXOrdinals.get(head) + ":inter" + head.getId()
                            + ":S:" + side.getKey();
                }
            }
        }
        return linker.getClass().getSimpleName() + ":inter" + linker.getSource().getId();
    }

    private static String relationState(Relation relation) {
        StringBuilder value = new StringBuilder(relation.getClass().getSimpleName())
                .append(":manual=").append(relation.isManual());
        if (relation instanceof Support support) {
            value.append(":grade=").append(hex(support.getGrade()))
                    .append(":impacts=").append(impacts(support.getImpacts()));
        }
        if (relation instanceof AbstractConnection connection) {
            value.append(":dx=").append(hex(connection.getDx()))
                    .append(":dy=").append(hex(connection.getDy()));
        }
        if (relation instanceof AbstractStemConnection stemConnection) {
            value.append(":extension=").append(point(stemConnection.getExtensionPoint()));
        }
        if (relation instanceof HeadStemRelation headStem) {
            value.append(":headSide=").append(headStem.getHeadSide())
                    .append(":consistency=").append(hex(headStem.getConsistency()));
        }
        if (relation instanceof BeamStemRelation beamStem) {
            value.append(":beamPortion=").append(beamStem.getBeamPortion());
        }
        return value.toString();
    }

    private static String impacts(GradeImpacts impacts) {
        if (impacts == null) return "-";
        List<String> values = new ArrayList<>();
        for (int index = 0; index < impacts.getImpactCount(); index++) {
            values.add(impacts.getName(index) + ":" + hex(impacts.getImpact(index))
                    + ":w=" + hex(impacts.getWeight(index)));
        }
        return compact(values);
    }

    private static String interToken(Inter inter) {
        Rectangle box = inter.getBounds();
        StringBuilder value = new StringBuilder(inter.getClass().getSimpleName())
                .append(":grade=").append(hex(inter.getGrade()))
                .append(":bounds=").append(rectangle(box));
        if (inter instanceof StemInter stem) {
            value.append(":median=").append(line(stem.getMedian()))
                    .append(":width=").append(hex(stem.getWidth()));
        }
        return value.toString();
    }

    private static String interAlias(
            Inter inter,
            IdentityHashMap<Inter, Integer> headXOrdinals,
            StemInter existingStem) {
        if (inter == existingStem) return "existingCandidateStem";
        if (inter instanceof HeadInter) return "headX" + headXOrdinals.get(inter);
        return inter.getClass().getSimpleName() + ":id" + inter.getId();
    }

    private static String glyphToken(Glyph glyph) throws Exception {
        Rectangle box = glyph.getBounds();
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        var table = glyph.getRunTable();
        update(digest, table.getOrientation() + " " + table.getWidth() + " "
                + table.getHeight() + "\n");
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                var run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            update(digest, row.append('\n').toString());
        }
        return "g:" + box.x + ":" + box.y + ":" + box.width + ":" + box.height
                + ":" + hexBytes(digest.digest());
    }

    private static Line2D copy(Line2D line) {
        return new Line2D.Double(line.getP1(), line.getP2());
    }

    private static String line(Line2D value) {
        return point(value.getP1()) + "->" + point(value.getP2());
    }

    private static String point(Point2D value) {
        return value == null ? "-" : hex(value.getX()) + ":" + hex(value.getY());
    }

    private static String rectangle(Rectangle value) {
        return value == null ? "-" : value.x + ":" + value.y + ":"
                + value.width + ":" + value.height;
    }

    private static String compact(Object value) {
        return value.toString().replace(" ", "");
    }

    private static String hex(double value) {
        return String.format("%a/%016x", value, Double.doubleToRawLongBits(value));
    }

    private static void update(MessageDigest digest, String value) {
        digest.update(value.getBytes(StandardCharsets.UTF_8));
    }

    private static String hexBytes(byte[] bytes) {
        StringBuilder value = new StringBuilder(bytes.length * 2);
        for (byte item : bytes) value.append(String.format("%02x", item & 0xff));
        return value.toString();
    }

    private static <T> IdentityHashMap<T, Boolean> identities(Iterable<T> values) {
        IdentityHashMap<T, Boolean> result = new IdentityHashMap<>();
        for (T value : values) result.put(value, Boolean.TRUE);
        return result;
    }

    private static IdentityHashMap<Inter, Integer> ordinals(List<Inter> values) {
        IdentityHashMap<Inter, Integer> result = new IdentityHashMap<>();
        for (int index = 0; index < values.size(); index++) result.put(values.get(index), index);
        return result;
    }

    private static Method method(Class<?> owner, String name) throws ReflectiveOperationException {
        Method method = owner.getDeclaredMethod(name);
        method.setAccessible(true);
        return method;
    }

    private static Field field(Class<?> owner, String name) throws ReflectiveOperationException {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }
}
