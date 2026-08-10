// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.awt.geom.Path2D;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Comparator;
import java.util.EnumMap;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.OMR;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.WeakGlyph;
import org.audiveris.omr.math.AreaUtil;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Part;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Skew;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.StaffLine;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.LineInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemBuilder;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemItem;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.AbstractInter;
import org.audiveris.omr.sig.inter.BeamGroupInter;
import org.audiveris.omr.sig.inter.BeamInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.SmallBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.HeadChordInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.AbstractConnection;
import org.audiveris.omr.sig.relation.AbstractStemConnection;
import org.audiveris.omr.sig.relation.BeamPortion;
import org.audiveris.omr.sig.relation.BeamRestRelation;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.sig.relation.ChordStemRelation;
import org.audiveris.omr.sig.relation.Containment;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Link;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.sig.relation.Support;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

import org.jgrapht.event.GraphEdgeChangeEvent;
import org.jgrapht.event.GraphListener;
import org.jgrapht.event.GraphVertexChangeEvent;
import org.jgrapht.graph.DefaultListenableGraph;

/**
 * Beam V-link HeadStem-link and method-return boundary.
 *
 * <p>Each target system is reconstructed from a fresh HEADS sheet. The selected scheduler prefix
 * joins and replays the frozen scheduler, expand, createStem, V-link reuse/check, and base-apply
 * B-linker-flag, and sibling-link fixtures. The probe resumes the exact
 * {@code ReadyBeforeHeadRelationLoop} frontier, executes the source-ordered
 * {@code relations.entrySet()} head loop, evaluates the commented-out remainder condition, and
 * stops after {@code VLinker.link} has returned true but before the caller's outer B-linker
 * assignment.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamVLinkOuterBLinkerProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;
    private static final Field RETRIEVER_PARAMS;
    private static final Field RETRIEVER_SYSTEM_SEEDS;
    private static final Field RETRIEVER_SYSTEM_BEAMS;
    private static final Field RETRIEVER_SYSTEM_HEADS;
    private static final Field RETRIEVER_STEM_CHECKER;
    private static final Field RETRIEVER_SYSTEM_STEMS;
    private static final Method PURGE_NO_STEM_SEEDS;
    private static final Field PARAMETERS_ARTIFICIAL_GRADE;
    private static final Field LINKER_ALL_B;
    private static final Field LINKER_BEAM;
    private static final Field LINKER_BEAM_GROUP;
    private static final Field LINKER_MEDIAN;
    private static final Field LINKER_RETRIEVER;
    private static final Field LINKER_SYSTEM;
    private static final Field LINKER_SCALE;
    private static final Field LINKER_PARAMS;
    private static final Field LINKER_SIDE_B;
    private static final Field LINKER_STUMP_V;
    private static final Field B_ID;
    private static final Field B_H_SIDE;
    private static final Field B_REF_PT;
    private static final Field B_STUMP;
    private static final Field B_IS_ANCHOR;
    private static final Field B_V_LINKERS;
    private static final Field B_LINKED;
    private static final Field B_CLOSED;
    private static final Field V_V_SIDE;
    private static final Field V_Y_DIR;
    private static final Field V_OUTER_B;
    private static final Field V_THEO_LINE;
    private static final Field V_STEM_BUILDER;
    private static final Method V_GET_B_LINKER;
    private static final Method V_LINK_SIBLINGS;
    private static final Object UNSAFE;
    private static final Method UNSAFE_ALLOCATE_INSTANCE;
    private static final Method V_EXPAND;
    private static final Field STEM_BUILDER_THEO_LINE;
    private static final Field STEM_BUILDER_ITEMS;
    private static final Field LINKER_ITEM_LINKER;
    private static final Field PARAMETERS_MAX_BEAM_SIDE_DX;
    private static final Field BEAM_LINKER_CONSTANTS;
    private static final Field CONSTANTS_MAX_SHORTER_RATIO;
    private static final Field GLYPH_ORIGINALS;
    private static final Field BEAM_OUT_WEIGHTS;
    private static final Field INTER_STAFF;
    private static final Field BOOK_MODIFIED;
    private static final Field GRAPH_LISTENERS;
    private static final Field VERTEX_SET_LISTENERS;
    private static final Field INTER_AREA;
    private static final Field HEAD_STEM_CONSISTENCY;
    private static final Field HEAD_STEM_CONSTANTS;
    private static final Field HEAD_STEM_NEUTRAL;
    private static final Field HEAD_LINKER_FIELD;
    private static final Field HEAD_TEMPLATE_FIELD;
    private static final Field HEAD_OLD_MIRROR_FIELD;
    private static final Class<?> B_LINKER_CLASS;
    private static final Class<?> V_LINKER_CLASS;
    private static final Class<?> C_LINKER_CLASS;
    private static final Field C_V_SIDE;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            final Class<?> beamConstants = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$Constants");
            B_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            V_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker");
            C_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker");
            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);
            RETRIEVER_PARAMS = field(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = field(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = field(StemsRetriever.class, "systemBeams");
            RETRIEVER_SYSTEM_HEADS = field(StemsRetriever.class, "systemHeads");
            RETRIEVER_STEM_CHECKER = field(StemsRetriever.class, "stemChecker");
            RETRIEVER_SYSTEM_STEMS = field(StemsRetriever.class, "systemStems");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            PARAMETERS_ARTIFICIAL_GRADE = field(parameters, "artificialStemGrade");
            LINKER_ALL_B = field(BeamLinker.class, "allBLinkers");
            LINKER_BEAM = field(BeamLinker.class, "beam");
            LINKER_BEAM_GROUP = field(BeamLinker.class, "beamGroup");
            LINKER_MEDIAN = field(BeamLinker.class, "median");
            LINKER_RETRIEVER = field(BeamLinker.class, "retriever");
            LINKER_SYSTEM = field(BeamLinker.class, "system");
            LINKER_SCALE = field(BeamLinker.class, "scale");
            LINKER_PARAMS = field(BeamLinker.class, "params");
            LINKER_SIDE_B = field(BeamLinker.class, "sideBLinkers");
            LINKER_STUMP_V = field(BeamLinker.class, "stumpLinkers");
            B_ID = field(B_LINKER_CLASS, "id");
            B_H_SIDE = field(B_LINKER_CLASS, "hSide");
            B_REF_PT = field(B_LINKER_CLASS, "refPt");
            B_STUMP = field(B_LINKER_CLASS, "stump");
            B_IS_ANCHOR = field(B_LINKER_CLASS, "isAnchor");
            B_V_LINKERS = field(B_LINKER_CLASS, "vLinkers");
            B_LINKED = field(B_LINKER_CLASS, "linked");
            B_CLOSED = field(B_LINKER_CLASS, "closed");
            V_V_SIDE = field(V_LINKER_CLASS, "vSide");
            V_Y_DIR = field(V_LINKER_CLASS, "yDir");
            V_OUTER_B = field(V_LINKER_CLASS, "this$1");
            V_THEO_LINE = field(V_LINKER_CLASS, "theoLine");
            V_STEM_BUILDER = field(V_LINKER_CLASS, "sb");
            V_GET_B_LINKER = V_LINKER_CLASS.getDeclaredMethod("getBLinker");
            V_GET_B_LINKER.setAccessible(true);
            V_LINK_SIBLINGS = V_LINKER_CLASS.getDeclaredMethod(
                    "linkSiblings", StemInter.class, double.class);
            V_LINK_SIBLINGS.setAccessible(true);
            V_EXPAND = V_LINKER_CLASS.getDeclaredMethod(
                    "expand", int.class, int.class, Map.class, Set.class);
            V_EXPAND.setAccessible(true);
            C_V_SIDE = field(C_LINKER_CLASS, "vSide");
            STEM_BUILDER_THEO_LINE = field(StemBuilder.class, "theoLine");
            STEM_BUILDER_ITEMS = field(StemBuilder.class, "items");
            LINKER_ITEM_LINKER = field(StemItem.LinkerItem.class, "linker");
            PARAMETERS_MAX_BEAM_SIDE_DX = field(parameters, "maxBeamSideDx");
            BEAM_LINKER_CONSTANTS = field(BeamLinker.class, "constants");
            CONSTANTS_MAX_SHORTER_RATIO = field(beamConstants, "maxShorterRatio");
            GLYPH_ORIGINALS = field(GlyphIndex.class, "originals");
            BEAM_OUT_WEIGHTS = field(BeamStemRelation.class, "OUT_WEIGHTS");
            INTER_STAFF = field(AbstractInter.class, "staff");
            BOOK_MODIFIED = field(Book.class, "modified");
            GRAPH_LISTENERS = field(DefaultListenableGraph.class, "graphListeners");
            VERTEX_SET_LISTENERS = field(DefaultListenableGraph.class, "vertexSetListeners");
            INTER_AREA = field(AbstractInter.class, "area");
            HEAD_STEM_CONSISTENCY = field(HeadStemRelation.class, "consistency");
            HEAD_STEM_CONSTANTS = field(HeadStemRelation.class, "constants");
            final Class<?> headStemConstants = Class.forName(
                    "org.audiveris.omr.sig.relation.HeadStemRelation$Constants");
            HEAD_STEM_NEUTRAL = field(headStemConstants, "neutralStemLength");
            HEAD_LINKER_FIELD = field(HeadInter.class, "linker");
            HEAD_TEMPLATE_FIELD = field(HeadInter.class, "template");
            HEAD_OLD_MIRROR_FIELD = field(HeadInter.class, "oldMirror");
            final Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
            final Field theUnsafe = unsafeClass.getDeclaredField("theUnsafe");
            theUnsafe.setAccessible(true);
            UNSAFE = theUnsafe.get(null);
            UNSAFE_ALLOCATE_INSTANCE = unsafeClass.getMethod("allocateInstance", Class.class);
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamVLinkOuterBLinkerProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        assertLoadedFromProductionClasses();
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            return;
        }
        if (args.length != 11 || !args[0].equals("--system")) {
            throw new IllegalArgumentException(
                    "expected --system <id> <path>:<sheet> <scheduler-fixture> "
                            + "<expand-fixture> <createStem-fixture> <reuse-check-fixture> "
                            + "<base-apply-fixture> <b-linker-flag-fixture> "
                            + "<sibling-links-fixture> <head-links-fixture>");
        }
        final int wantedSystem = Integer.parseInt(args[1]);
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        final String[] target = args[2].split(":");
        if (target.length != 2) throw new IllegalArgumentException("target must be <path>:<sheet>");
        final Path pagePath = Paths.get(target[0]).toAbsolutePath();
        final int sheetNumber = Integer.parseInt(target[1]);
        final Path schedulerFixture = Paths.get(args[3]).toAbsolutePath();
        final Path expandFixture = Paths.get(args[4]).toAbsolutePath();
        final Path createStemFixture = Paths.get(args[5]).toAbsolutePath();
        final Path reuseCheckFixture = Paths.get(args[6]).toAbsolutePath();
        final Path baseApplyFixture = Paths.get(args[7]).toAbsolutePath();
        final Path bLinkerFlagFixture = Paths.get(args[8]).toAbsolutePath();
        final Path siblingLinksFixture = Paths.get(args[9]).toAbsolutePath();
        final Path headLinksFixture = Paths.get(args[10]).toAbsolutePath();
        final Map<Integer, Expected> expected = expectedFrontiers(
                schedulerFixture, expandFixture, createStemFixture, reuseCheckFixture,
                baseApplyFixture, bLinkerFlagFixture, siblingLinksFixture);

        final Sheet sheet = loadPage(pagePath, sheetNumber);
        final int systemCount = sheet.getSystems().size();
        if (expected.size() != systemCount) {
            throw new IllegalStateException("fixture/system count mismatch");
        }
        final String page = pagePath.getFileName() + "#" + sheetNumber;
        if (wantedSystem == 1) {
            printHeader();
            System.out.printf(
                    "stemsbeamvlinkheadlinkspage %s systems %d schedulerFixtureSha256 %s "
                            + "expandFixtureSha256 %s createStemFixtureSha256 %s "
                            + "reuseCheckFixtureSha256 %s baseApplyFixtureSha256 %s "
                            + "bLinkerFlagFixtureSha256 %s siblingLinksFixtureSha256 %s "
                            + "executionMode foregroundJvmPerSystem "
                            + "relationOrder LinkedHashMapFirstInsertionLatestPayload "
                            + "incidentOrder IncomingThenOutgoing headless %s "
                            + "methodDispatch ResumedExactBeamLinkerVLinkerHeadLoop "
                            + "stop ReturnedTrueBeforeOuterBLinkerAssignment%n",
                    page, systemCount, sha256File(schedulerFixture), sha256File(expandFixture),
                    sha256File(createStemFixture), sha256File(reuseCheckFixture),
                    sha256File(baseApplyFixture), sha256File(bLinkerFlagFixture),
                    sha256File(siblingLinksFixture), OMR.gui == null);
            System.out.printf(
                    "stemsbeamvlinkouterblinkerpage %s systems %d headLinksFixtureSha256 %s "
                            + "executionMode foregroundJvmPerSystem "
                            + "methodDispatch ExactConcreteBLinkerSetLinked "
                            + "stop AssignedOuterBLinkerBeforeNextVIteration%n",
                    page, systemCount, sha256File(headLinksFixture));
        }

        final Totals totals = new Totals();
        final SystemInfo system = systemById(sheet, wantedSystem);
        runSystem(
                page, sheet, system, expected.get(wantedSystem),
                wantedSystem == 1 ? "sheet-first" : "isolated-system-frontier", totals);
        System.exit(0);
    }

    private static void assertLoadedFromProductionClasses ()
        throws Exception
    {
        final Class<?>[] classes = {
                BeamLinker.class, B_LINKER_CLASS, V_LINKER_CLASS, StemLinker.class,
                HeadLinker.class, HeadLinker.SLinker.class, C_LINKER_CLASS,
                HeadStemRelation.class, ChordStemRelation.class, HeadInter.class,
                HeadChordInter.class, StemInter.class, Part.class, SIGraph.class
        };
        final java.net.URL expected = BeamLinker.class.getProtectionDomain()
                .getCodeSource().getLocation();
        final Path expectedPath = Path.of(expected.toURI()).toRealPath();
        if (!expectedPath.endsWith(Path.of("app", "build", "classes", "java", "main"))) {
            throw new IllegalStateException("BeamLinker was not loaded from production classes");
        }
        for (Class<?> type : classes) {
            final java.security.CodeSource source = type.getProtectionDomain().getCodeSource();
            if (source == null || !Path.of(source.getLocation().toURI()).toRealPath()
                    .equals(expectedPath)) {
                throw new IllegalStateException(
                        "mixed production bytecode provider for " + type.getName());
            }
        }
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
                                   Expected expected,
                                   String executionMode,
                                   Totals totals)
        throws Exception
    {
        if (expected == null) throw new IllegalStateException("missing expected frontier");
        final StemsRetriever retriever = new StemsRetriever(system);
        final Object params = PARAMETERS_CONSTRUCTOR.newInstance(system, sheet.getScale());
        final StemChecker checker = new StemChecker(sheet);
        RETRIEVER_PARAMS.set(retriever, params);
        RETRIEVER_STEM_CHECKER.set(retriever, checker);

        final List<Glyph> seeds = new ArrayList<>(
                system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED));
        PURGE_NO_STEM_SEEDS.invoke(retriever, seeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, seeds);

        final List<Inter> originalBeamSigOrder = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = ordinals(originalBeamSigOrder);
        final List<Inter> inspectionBeams = new ArrayList<>(originalBeamSigOrder);
        Collections.sort(inspectionBeams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, inspectionBeams);
        for (Iterator<Inter> iterator = inspectionBeams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (beam.getLinker() != null) throw new IllegalStateException("beam linker already set");
            final BeamLinker linker = new BeamLinker(beam, retriever);
            if (linker.looksLikeTremolo()) {
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }
        }

        final List<Inter> heads = system.getSig().inters(ShapeSet.getTemplateNotesStem(sheet));
        final IdentityHashMap<HeadInter, Integer> headSigOrdinals = new IdentityHashMap<>();
        for (int sigOrdinal = 0; sigOrdinal < heads.size(); sigOrdinal++) {
            headSigOrdinals.put((HeadInter) heads.get(sigOrdinal), sigOrdinal);
        }
        Collections.sort(heads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, heads);
        final IdentityHashMap<Object, String> cAliases = new IdentityHashMap<>();
        final IdentityHashMap<HeadInter, Integer> headOrdinals = new IdentityHashMap<>();
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            headOrdinals.put(head, headOrdinal);
            if (head.getLinker() != null) throw new IllegalStateException("head linker already set");
            head.setLinker(new HeadLinker(head, retriever));
            for (HorizontalSide hSide : HorizontalSide.values()) {
                for (VerticalSide vSide : VerticalSide.values()) {
                    final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                    if (cAliases.put(c, cToken(headOrdinal, hSide, vSide)) != null) {
                        throw new IllegalStateException("duplicate CLinker identity");
                    }
                }
            }
        }
        for (Inter inter : inspectionBeams) {
            ((AbstractBeamInter) inter).getLinker().inspectVLinkers();
        }
        for (Inter inter : heads) ((HeadInter) inter).getLinker().inspectCLinkers();

        final IdentityHashMap<Object, String> bAliases = new IdentityHashMap<>();
        final IdentityHashMap<Object, PlanRef[]> planRefs = new IdentityHashMap<>();
        final List<StemLinker> allLinkers = new ArrayList<>();
        int planOrdinal = 0;
        int builderOrdinal = 0;
        for (Inter inter : inspectionBeams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                final Object b = allB.get(bOrdinal);
                final String bAlias = "beam:" + beamSigOrdinals.get(beam) + ":b:" + bOrdinal;
                if (bAliases.put(b, bAlias) != null) {
                    throw new IllegalStateException("duplicate BLinker identity");
                }
                allLinkers.add((StemLinker) b);
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Object v : vMap.values()) allLinkers.add((StemLinker) v);
                if (B_IS_ANCHOR.getBoolean(b)) continue;
                final int constructionMax = B_H_SIDE.get(b) != null
                        ? Profiles.BEAM_SIDE : Profiles.BEAM_SEED;
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    final PlanRef[] refs = new PlanRef[constructionMax + 1];
                    for (int profile = 0; profile <= constructionMax; profile++) {
                        refs[profile] = new PlanRef(
                                planOrdinal++, builderOrdinal, bAlias, entry.getKey());
                    }
                    planRefs.put(entry.getValue(), refs);
                    builderOrdinal++;
                }
            }
        }
        for (Inter inter : heads) {
            final HeadLinker headLinker = ((HeadInter) inter).getLinker();
            for (HeadLinker.SLinker side : headLinker.getSLinkers().values()) {
                allLinkers.add(side);
                allLinkers.addAll(side.getHalfLinkers());
            }
        }

        final List<Inter> work = system.getSig().inters(AbstractBeamInter.class);
        Collections.sort(work, Inters.byReverseWidth);
        final Scheduler scheduler = new Scheduler(
                page, sheet, system, retriever, checker, params, work, beamSigOrdinals,
                bAliases, cAliases, headOrdinals, headSigOrdinals, planRefs,
                inspectionBeams, heads, allLinkers, expected,
                executionMode, totals);
        scheduler.run();
        if (!scheduler.stopped) throw new IllegalStateException("no createStem frontier reached");
    }

    private static final class Scheduler
    {
        final String page;
        final Sheet sheet;
        final SystemInfo system;
        final StemsRetriever retriever;
        final StemChecker checker;
        final Object params;
        final List<Inter> work;
        final IdentityHashMap<Inter, Integer> beamSigOrdinals;
        final IdentityHashMap<Object, String> bAliases;
        final IdentityHashMap<Object, String> cAliases;
        final IdentityHashMap<HeadInter, Integer> headOrdinals;
        final IdentityHashMap<HeadInter, Integer> headSigOrdinals;
        final IdentityHashMap<Object, PlanRef[]> planRefs;
        final List<Inter> inspectionBeams;
        final List<Inter> heads;
        final List<StemLinker> allLinkers;
        final Expected expected;
        final String executionMode;
        final Totals totals;
        boolean stopped;

        Scheduler (String page,
                   Sheet sheet,
                   SystemInfo system,
                   StemsRetriever retriever,
                   StemChecker checker,
                   Object params,
                   List<Inter> work,
                   IdentityHashMap<Inter, Integer> beamSigOrdinals,
                   IdentityHashMap<Object, String> bAliases,
                   IdentityHashMap<Object, String> cAliases,
                   IdentityHashMap<HeadInter, Integer> headOrdinals,
                   IdentityHashMap<HeadInter, Integer> headSigOrdinals,
                   IdentityHashMap<Object, PlanRef[]> planRefs,
                   List<Inter> inspectionBeams,
                   List<Inter> heads,
                   List<StemLinker> allLinkers,
                   Expected expected,
                   String executionMode,
                   Totals totals)
        {
            this.page = page;
            this.sheet = sheet;
            this.system = system;
            this.retriever = retriever;
            this.checker = checker;
            this.params = params;
            this.work = work;
            this.beamSigOrdinals = beamSigOrdinals;
            this.bAliases = bAliases;
            this.cAliases = cAliases;
            this.headOrdinals = headOrdinals;
            this.headSigOrdinals = headSigOrdinals;
            this.planRefs = planRefs;
            this.inspectionBeams = inspectionBeams;
            this.heads = heads;
            this.allLinkers = allLinkers;
            this.expected = expected;
            this.executionMode = executionMode;
            this.totals = totals;
        }

        void run ()
            throws Exception
        {
            int index = 0;
            while (index < work.size() && !stopped) {
                final AbstractBeamInter beam = (AbstractBeamInter) work.get(index);
                final BeamHookInter competitor = beam.getCompetingHook();
                final EnumMap<HorizontalSide, Boolean> linkedSides =
                        new EnumMap<>(HorizontalSide.class);
                boolean beamOk = true;
                final Map<HorizontalSide, Object> sides =
                        (Map<HorizontalSide, Object>) LINKER_SIDE_B.get(beam.getLinker());
                for (HorizontalSide hSide : HorizontalSide.values()) {
                    final Object b = sides.get(hSide);
                    if (b == null) continue;
                    final int stemProfile = (beam.isHook() || competitor != null)
                            ? system.getProfile() : Profiles.BEAM_SIDE;
                    final boolean ok = runB(
                            beam, index, hSide, b, stemProfile, system.getProfile());
                    if (stopped) return;
                    if (ok) linkedSides.put(hSide, true);
                    else if (!beam.isHook()) {
                        beamOk = false;
                        break;
                    }
                }
                if (beam.isHook() && linkedSides.isEmpty()) beamOk = false;
                if (!beam.isHook() && linkedSides.size() == 2 && competitor != null) {
                    throw new IllegalStateException("hook-removal frontier precedes createStem");
                }
                if (!beamOk) work.remove(index);
                else index++;
            }
            throw new IllegalStateException("stump phase precedes createStem frontier");
        }

        boolean runB (AbstractBeamInter beam,
                      int beamIndex,
                      HorizontalSide hSide,
                      Object b,
                      int stemProfile,
                      int linkProfile)
            throws Exception
        {
            final Map<VerticalSide, Object> vMap =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            if (vMap.isEmpty() || ((StemLinker) b).isLinked()) return true;
            for (Object v : vMap.values()) {
                final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
                if (builder.getTargetLinkers().isEmpty()) continue;
                if (runV(beam, beamIndex, hSide, b, v, stemProfile, linkProfile)) return true;
                if (stopped) return false;
            }
            return false;
        }

        boolean runV (AbstractBeamInter beam,
                      int beamIndex,
                      HorizontalSide hSide,
                      Object b,
                      Object v,
                      int stemProfile,
                      int linkProfile)
            throws Exception
        {
            final PlanRef[] refs = planRefs.get(v);
            if (refs == null || stemProfile < 0 || stemProfile >= refs.length) {
                throw new IllegalStateException("scheduler profile has no isolated plan");
            }
            final PlanRef ref = refs[stemProfile];
            final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
            final int headTargets = builder.getCLinkers(null).size();
            if (headTargets == 0) return false;

            final Line2D stored = (Line2D) V_THEO_LINE.get(v);
            final Line2D lineBefore = copy(stored);
            final PersistentSnapshot before = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final LinkedHashMap<StemLinker, Relation> relations = new LinkedHashMap<>();
            final LinkedHashSet<Glyph> glyphs = new LinkedHashSet<>();
            final int lastIndex = (Integer) V_EXPAND.invoke(
                    v, stemProfile, linkProfile, relations, glyphs);
            final Line2D lineAfterExpand = copy(stored);

            final String outcome;
            if (lastIndex == -1) outcome = "ExpandFailed";
            else if (relations.isEmpty()) outcome = "NoRelations";
            else if (glyphs.isEmpty()) outcome = "NoGlyphs";
            else outcome = "ReadyForCreateStem";
            if (!outcome.equals("ReadyForCreateStem")) return false;

            assertExpected(
                    beam, hSide, b, v, ref, stemProfile, linkProfile,
                    lastIndex, relations.size(), glyphs.size());
            final PersistentSnapshot expanded = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            before.assertOnlyLineChanged(expanded);

            final Object attachment = beam.getAttachments().get("theo-" + B_ID.getInt(b));
            final boolean builderAlias = STEM_BUILDER_THEO_LINE.get(builder) == stored;
            final boolean attachmentAlias = attachment == stored;
            if (!builderAlias) throw new IllegalStateException("builder/theoretical-line alias lost");
            if ((Integer) V_Y_DIR.get(v) > 0 && !attachmentAlias) {
                throw new IllegalStateException("downward/current-attachment alias lost");
            }

            final Glyph candidate = glyphs.size() == 1
                    ? glyphs.iterator().next() : GlyphFactory.buildGlyph(glyphs);
            final GlyphRegistrySnapshot glyphsBefore = before.glyphs;
            final Glyph existing = glyphsBefore.equalOriginal(candidate);
            final boolean existingActive = existing != null && glyphsBefore.active.containsKey(existing);
            final StemInter existingStem = before.systemStems.equalStem(candidate);
            if (existingStem != null && existing == null) {
                throw new IllegalStateException("systemStems candidate lacks GlyphIndex original");
            }
            final List<String> selected = selectedGlyphTokens(glyphs, glyphsBefore);
            final StemInterState existingStemState = existingStem != null
                    ? new StemInterState(existingStem) : null;
            final int candidateObjectIdBefore = candidate.getId();
            final String noStaff = noStaffDigest(sheet);

            final StemInter stem = builder.createStem(glyphs, stemProfile);
            final PersistentSnapshot after = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final Glyph registered = after.glyphs.uniqueEqualOriginal(candidate);
            final String registration;
            if (existing == null) registration = "New";
            else if (existingActive) registration = "ReuseActive";
            else registration = "ReinsertOriginal";
            final String disposition;
            final GradeImpacts createImpacts;
            if (existingStem != null) {
                if (stem != existingStem) throw new IllegalStateException("system stem reuse mismatch");
                disposition = "Reused";
                createImpacts = stem.getImpacts();
            } else if (stem == null) {
                disposition = "Rejected";
                createImpacts = checker.checkStem(registered, stemProfile);
            } else if (stem.getImpacts() == null) {
                disposition = "CreatedArtificial";
                createImpacts = checker.checkStem(registered, stemProfile);
            } else {
                disposition = "CreatedChecked";
                createImpacts = stem.getImpacts();
            }
            after.assertCreateStemDelta(
                    before, expanded, registered, registration, existingStem, stem, retriever);
            if (!sameLine(stored, lineAfterExpand)) {
                throw new IllegalStateException("createStem changed theoretical line");
            }
            if (existingStem != null) {
                if (!existingStemState.same(stem)) {
                    throw new IllegalStateException("reused StemInter state changed");
                }
            } else if (stem != null) {
                if (stem.getId() != 0 || stem.getSig() != null || stem.getGlyph() != registered) {
                    throw new IllegalStateException("createStem crossed SIG boundary");
                }
                final Line2D stemMedian = stem.getMedian();
                final Double stemWidth = stem.getWidth();
                final Rectangle stemBounds = stem.getBounds();
                if (stemMedian == null || stemWidth == null || stemBounds == null
                        || stem.isAbnormal()) {
                    throw new IllegalStateException("incomplete created StemInter state");
                }
                if (!sameLine(stemMedian, registered.getCenterLine())
                        || Double.doubleToLongBits(stemWidth)
                                != Double.doubleToLongBits(registered.getMeanThickness(
                                        org.audiveris.omr.run.Orientation.VERTICAL))) {
                    throw new IllegalStateException("created StemInter glyph geometry mismatch");
                }
                final Rectangle ribbonBounds = AreaUtil.verticalRibbon(
                        new Path2D.Double(stemMedian), stemWidth).getBounds();
                if (!stemBounds.equals(ribbonBounds)) {
                    throw new IllegalStateException("created StemInter ribbon bounds mismatch");
                }
            }

            assertCreateStemExpected(
                    ref, candidate, selected, candidateObjectIdBefore, existing, registration,
                    registered, disposition, stem, createImpacts, stemProfile, before, after);
            executeBoundary(
                    beam, beamIndex, hSide, b, v, ref, stemProfile, linkProfile, lastIndex,
                    relations, glyphs, selected, candidateObjectIdBefore, registered,
                    registration, disposition, stem, after, noStaff);
            stopped = true;
            return stem != null;
        }

        void assertCreateStemExpected (PlanRef ref,
                                       Glyph candidate,
                                       List<String> selected,
                                       int candidateObjectIdBefore,
                                       Glyph existing,
                                       String registration,
                                       Glyph registered,
                                       String disposition,
                                       StemInter stem,
                                       GradeImpacts createImpacts,
                                       int stemProfile,
                                       PersistentSnapshot before,
                                       PersistentSnapshot after)
        {
            final Map<String, String> result = expected.createResult;
            final Map<String, String> delta = expected.createDelta;
            final Map<String, String> guard = expected.createGuard;
            assertField(result, "plan", Integer.toString(ref.plan));
            assertField(result, "candidate", glyphToken(candidate));
            assertField(result, "candidateComponents", list(selected));
            assertField(result, "registration", registration);
            assertField(result, "candidateObjectIdBefore", Integer.toString(candidateObjectIdBefore));
            assertField(result, "canonicalGlyphIdBefore",
                    existing != null ? Integer.toString(existing.getId()) : "-");
            assertField(result, "registeredAlias", "glyph:" + registered.getId());
            assertField(result, "postAliasOrder", "JavaGlyphId");
            assertField(result, "postUnionSize", Integer.toString(after.glyphs.aliases.size()));
            assertField(result, "registeredGlyphId", Integer.toString(registered.getId()));
            assertField(result, "disposition", disposition);
            assertField(result, "returnedStemInterId", stem != null ? Integer.toString(stem.getId()) : "-");
            assertField(result, "stemGrade", stem != null ? hex(stem.getGrade()) : "-");
            assertField(result, "stemMedian", stem != null ? line(stem.getMedian()) : "-");
            assertField(result, "stemMeanThickness",
                    stem != null && stem.getWidth() != null ? hex(stem.getWidth()) : "-");
            assertField(result, "stemBounds",
                    stem != null && stem.getBounds() != null ? rectangle(stem.getBounds()) : "-");
            assertField(result, "stemAbnormal",
                    stem != null ? Boolean.toString(stem.isAbnormal()) : "-");
            assertField(result, "stemSigAttached",
                    stem != null ? Boolean.toString(stem.getSig() != null) : "-");
            assertField(result, "stemMinGrade", hex(StemInter.getMinGrade()));
            assertField(result, "checkerMinThreshold", hex(checker.getMinThreshold(stemProfile)));
            assertField(result, "artificialGrade",
                    hex((Double) get(PARAMETERS_ARTIFICIAL_GRADE, params)));
            assertField(result, "impacts", impactsToken(createImpacts));
            assertField(delta, "allocatorBefore", Integer.toString(before.allocator));
            assertField(delta, "allocatorAfter", Integer.toString(after.allocator));
            assertField(delta, "allocatorDelta", Integer.toString(after.allocator - before.allocator));
            assertField(delta, "glyphActiveBefore", Integer.toString(before.glyphs.active.size()));
            assertField(delta, "glyphActiveAfter", Integer.toString(after.glyphs.active.size()));
            assertField(delta, "glyphOriginalsBefore",
                    Integer.toString(before.glyphs.originals.size()));
            assertField(delta, "glyphOriginalsAfter",
                    Integer.toString(after.glyphs.originals.size()));
            assertField(delta, "systemStemsBefore",
                    Integer.toString(before.systemStems.entries.size()));
            assertField(delta, "systemStemsAfter",
                    Integer.toString(after.systemStems.entries.size()));
            assertField(delta, "registeredAlias", "glyph:" + registered.getId());
            assertField(delta, "registeredGlyph", glyphToken(registered));
            assertField(delta, "glyphActiveHashAfter", after.glyphs.activeHash);
            assertField(delta, "glyphOriginalsHashAfter", after.glyphs.originalsHash);
            assertField(
                    delta, "systemStemsHashAfter",
                    PredecessorCompatSnapshot.captureLegacySystemStems(retriever).hash());
            for (String name : List.of(
                    "lineDeltaRetained", "interIndexUnchanged", "sigUnchanged",
                    "relationsUnchanged", "linkerFlagsUnchanged", "stopBeforeVReuse",
                    "stopBeforeBeamStemCheck")) {
                assertField(guard, name, "true");
            }
        }

        void executeBoundary (AbstractBeamInter beam,
                              int beamIndex,
                              HorizontalSide hSide,
                              Object b,
                              Object v,
                              PlanRef ref,
                              int stemProfile,
                              int linkProfile,
                              int lastIndex,
                              LinkedHashMap<StemLinker, Relation> relations,
                              LinkedHashSet<Glyph> glyphs,
                              List<String> selectedGlyphs,
                              int candidateObjectIdBefore,
                              Glyph registered,
                              String registration,
                              String disposition,
                              StemInter initialStem,
                              PersistentSnapshot boundaryBefore,
                              String noStaff)
            throws Exception
        {
            if (initialStem == null) {
                throw new IllegalStateException("frozen createStem frontier unexpectedly rejected");
            }
            final VerticalSide vSide = (VerticalSide) V_V_SIDE.get(v);
            final VerticalSide headToBeam = vSide.opposite();
            final List<String> actualRelationAliases = new ArrayList<>();
            for (StemLinker linker : relations.keySet()) {
                final String alias = cAliases.get(linker);
                if (alias == null) throw new IllegalStateException("relation key is not a known CLinker");
                actualRelationAliases.add(alias);
            }
            if (!actualRelationAliases.equals(expected.relationAliases)) {
                throw new IllegalStateException(
                        "create frontier relation order drift: actual=" + actualRelationAliases
                                + " expected=" + expected.relationAliases);
            }
            final String relationInputHashBefore = relationInputHash(relations, cAliases);
            final IdentityHashMap<Relation, Integer> sigEdgeOrdinals = new IdentityHashMap<>();
            int sigEdgeOrdinal = 0;
            for (Relation relation : system.getSig().edgeSet()) {
                sigEdgeOrdinals.put(relation, sigEdgeOrdinal++);
            }
            final LiveStemCatalogue liveStems = LiveStemCatalogue.forExaminedScans(
                    system.getSig(), sigEdgeOrdinals);
            final String initialAlias = stemAlias(initialStem, initialStem, ref.plan, liveStems);
            final ReuseRun reuse = runRealReuse(
                    ref.plan, relations, initialStem, liveStems, sigEdgeOrdinals);
            final StemInter finalStem = reuse.finalStem;
            final String finalAlias = stemAlias(finalStem, initialStem, ref.plan, liveStems);
            if (!"ContinueToCheck".equals(reuse.terminal)) {
                throw new IllegalStateException("real corpus reached Java null side-map failure");
            }
            final CheckTrace trace = CheckTrace.compute(
                    beam, finalStem, headToBeam, sheet.getScale(), stemProfile);
            final Link link = BeamStemRelation.checkLink(
                    beam, finalStem, headToBeam, sheet.getScale(), stemProfile);
            trace.assertActual(link, finalStem);
            if (link == null) throw new IllegalStateException("frozen boundary-13 link rejected");
            assertReuseCheckExpected(ref.plan, finalAlias, finalStem, reuse, link);

            // Capture the predecessor certificate with the frozen boundary-13 algorithms before
            // either addVertex or Link.applyTo is allowed to mutate the shared sheet/SIG state.
            final PredecessorCompatSnapshot predecessorCompat =
                    new PredecessorCompatSnapshot(sheet, retriever, allLinkers);
            predecessorCompat.assertMatches(
                    expected.reuseBaseline, boundaryBefore, noStaff, relationInputHashBefore,
                    executionMode);

            final ApplyRun run = executeBaseApply(
                    "real", "-", beam, finalAlias, finalStem, link, null);
            final PersistentSnapshot boundary14After = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            assertRealBaseApplyDelta(boundaryBefore, boundary14After, beam, finalStem, link);
            assertBaseApplyExpected(run, ref.plan, finalAlias, finalStem);
            liveStems.assertUnchanged();
            if (!relationInputHashBefore.equals(relationInputHash(relations, cAliases))) {
                throw new IllegalStateException("base apply mutated head relation input map");
            }

            final BLinkerFlagRun flagRun = executeBLinkerFlag(
                    "real", "-", b, v, ref, run, finalAlias, finalStem,
                    boundary14After);
            assertBLinkerFlagExpected(flagRun);

            final PersistentSnapshot boundary15After = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            assertRealBLinkerFlagDelta(boundary14After, boundary15After, flagRun);
            liveStems.assertUnchanged();
            if (!relationInputHashBefore.equals(relationInputHash(relations, cAliases))) {
                throw new IllegalStateException("B-linker flag write mutated relation input map");
            }

            final SiblingLinksRun siblingRun = executeSiblingLinks(
                    flagRun, boundary15After, relationInputHashBefore, relations);
            assertSiblingLinksPredecessor(siblingRun);

            final HeadLinksRun headRun = executeHeadLinks(
                    siblingRun, lastIndex, relations);
            emitHeadLinksRows(headRun);
            if (system.getId() == 1) emitIsolatedHeadLinksCoverage(headRun);
            executeOuterBLinker(flagRun, headRun, b, v);

            totals.transactions++;
            totals.newVertexBranches += run.newVertex ? 1 : 0;
            totals.existingVertexBranches += run.newVertex ? 0 : 1;
            totals.edgesAdded += Boolean.TRUE.equals(run.applyReturned) ? 1 : 0;
            totals.chordMatches += run.stemIncidents.chordMatches;
            if (run.thrown != null) totals.throwsSeen++;

            totals.siblingCandidates += siblingRun.siblings.size();
            totals.siblingEdgesAdded += siblingRun.accepted.size();
            totals.headEntries += headRun.entries.size();
            totals.headEdgesAdded += headRun.inserted.size();
        }

        @SuppressWarnings("unchecked")
        void executeOuterBLinker (BLinkerFlagRun flagRun,
                                  HeadLinksRun headRun,
                                  Object b,
                                  Object v)
            throws Exception
        {
            // Boundary 18: the caller seam in BLinker.link. VLinker.link has just returned
            // true; the caller executes one plain this.setLinked(true) on the outer BLinker
            // and moves on to its next VLinker. Boundary 15 already wrote the same shared
            // cell from inside VLinker.link, so the outer write is idempotent on every real
            // transaction; the boundary content is the exact concrete dispatch, the no-change
            // proof, and the loop-resumption facts.
            if (b.getClass() != B_LINKER_CLASS
                    || B_LINKED.getDeclaringClass() != B_LINKER_CLASS
                    || B_LINKED.getType() != boolean.class
                    || java.lang.reflect.Modifier.isVolatile(B_LINKED.getModifiers())
                    || B_LINKER_CLASS.getMethod("setLinked", boolean.class)
                            .getDeclaringClass() != B_LINKER_CLASS) {
                throw new IllegalStateException(
                        "outer B-linker setter is not exact concrete dispatch");
            }
            if (V_GET_B_LINKER.invoke(v) != b || flagRun.b != b) {
                throw new IllegalStateException("outer BLinker identity drift");
            }
            final Map<VerticalSide, Object> vMap =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            int ordinal = 0;
            int currentOrdinal = -1;
            int remainingWithTargets = 0;
            int remainingSkipped = 0;
            for (Object other : vMap.values()) {
                if (other == v) {
                    currentOrdinal = ordinal;
                } else if (currentOrdinal >= 0) {
                    final StemBuilder sb = (StemBuilder) V_STEM_BUILDER.get(other);
                    if (sb.getTargetLinkers().isEmpty()) remainingSkipped++;
                    else remainingWithTargets++;
                }
                ordinal++;
            }
            if (currentOrdinal < 0) {
                throw new IllegalStateException("current VLinker missing from its BLinker map");
            }
            final BLinkerArenaSnapshot arenaBefore = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, b);
            final boolean linkedBefore = ((StemLinker) b).isLinked();
            final boolean closedBefore = ((StemLinker) b).isClosed();
            if (!linkedBefore) {
                throw new IllegalStateException("outer BLinker not linked by boundary 15");
            }
            // The exact source seam: BLinker.link's setLinked(true) after a successful
            // vLinker.link, concrete dispatch certified above.
            ((BeamLinker.BLinker) b).setLinked(true);
            final boolean linkedAfter = ((StemLinker) b).isLinked();
            final boolean closedAfter = ((StemLinker) b).isClosed();
            final BLinkerArenaSnapshot arenaAfter = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, b);
            arenaBefore.assertOnlyTargetCellChanged(arenaAfter, b);
            if (!linkedAfter || closedBefore != closedAfter) {
                throw new IllegalStateException("outer write changed unexpected linker state");
            }
            final PersistentSnapshot outerAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            if (!outerAfter.provenanceHash().equals(headRun.after.provenanceHash())) {
                throw new IllegalStateException("outer idempotent write mutated persistent state");
            }
            final String prefix = page + " system " + system.getId() + " plan "
                    + flagRun.ref.plan + " scope real case -";
            System.out.printf(
                    "stemsbeamvlinkouterblinker %s bAlias %s vSide %s entryGuardReplayed true "
                            + "linkedBefore %s writeExecuted true linkedAfter %s valueChanged %s "
                            + "closedBefore %s closedAfter %s arenaOtherCellsUnchanged true "
                            + "persistentStateUnchanged true vMapSize %d currentVOrdinal %d "
                            + "remainingWithTargets %d remainingSkipped %d impliedLinkReturn true "
                            + "terminal AssignedOuterBLinkerBeforeNextVIteration%n",
                    prefix, flagRun.bAlias, V_V_SIDE.get(v), linkedBefore, linkedAfter,
                    linkedBefore != linkedAfter, closedBefore, closedAfter, vMap.size(),
                    currentOrdinal, remainingWithTargets, remainingSkipped);
            System.out.printf(
                    "stemsbeamvlinkouterblinkersummary %s transactions 1 idempotentWrites %d "
                            + "valueChanges %d impliedLinkReturn true terminal "
                            + "AssignedOuterBLinkerBeforeNextVIteration%n",
                    prefix, (linkedBefore == linkedAfter && linkedAfter) ? 1 : 0,
                    (linkedBefore != linkedAfter) ? 1 : 0);
        }

        void assertSiblingLinksPredecessor (SiblingLinksRun run)
            throws Exception
        {
            final PrintStream original = System.out;
            final ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            try (PrintStream capture = new PrintStream(bytes, true, StandardCharsets.UTF_8)) {
                System.setOut(capture);
                emitSiblingLinksRows(run);
            } finally {
                System.setOut(original);
            }
            final List<String> rows = bytes.toString(StandardCharsets.UTF_8).lines().toList();
            final SiblingLinksRows expectedRows = expected.siblingLinks;
            if (rows.size() != expectedRows.orderedRowCount
                    || !sha256Rows(rows).equals(expectedRows.orderedRowsSha256)) {
                throw new IllegalStateException(
                        "Boundary-16 exact ordered row bundle drift: rows=" + rows.size()
                                + " sha=" + sha256Rows(rows));
            }
        }

        HeadLinksRun executeHeadLinks (SiblingLinksRun predecessor,
                                       int lastIndex,
                                       LinkedHashMap<StemLinker, Relation> relations)
            throws Exception
        {
            final BLinkerFlagRun base = predecessor.predecessor;
            final StemInter stem = base.stem;
            final SIGraph sig = stem.getSig();
            if (sig == null || !sig.containsVertex(stem)
                    || base.currentV.getClass() != V_LINKER_CLASS) {
                throw new IllegalStateException("invalid Boundary-17 predecessor frontier");
            }
            if (!ListenerTopology.capture(sig).isStandard() || OMR.gui != null) {
                throw new IllegalStateException("compact head-links listener/headless envelope drift");
            }
            final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(base.currentV);
            final List<?> builderItemsBefore = new ArrayList<>(
                    (List<?>) STEM_BUILDER_ITEMS.get(builder));
            final String builderItemsHash = builderItemsHash(builderItemsBefore);
            final PersistentSnapshot before = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            predecessor.persistentAfter.assertSame(before);
            final GraphOrder graphBefore = new GraphOrder(sig);
            final IdentityHashMap<Inter, String> vertexStateBefore = new IdentityHashMap<>();
            for (Inter inter : graphBefore.vertices) {
                vertexStateBefore.put(inter, interStructuralToken(inter));
            }
            final DirtyFlags dirtyBefore = DirtyFlags.capture(sheet);
            final String relationInputHashBefore = relationInputHash(relations, cAliases);
            final String headPlanStateHashBefore = headRelationMapStateHash(relations, cAliases);
            if (!relationInputHashBefore.equals(predecessor.relationInputHashAfter)) {
                throw new IllegalStateException("Boundary-16 relation-map continuity drift");
            }
            final double neutral = neutralStemLength();
            if (Double.doubleToRawLongBits(neutral)
                    != Double.doubleToRawLongBits(2.8d)) {
                throw new IllegalStateException("HeadStem neutral length constant drift");
            }
            final BeamLinker sourceBeamLinker = base.baseRun.beam.getLinker();
            final Scale sourceScale = (Scale) LINKER_SCALE.get(sourceBeamLinker);
            if (sourceScale != sheet.getScale()) {
                throw new IllegalStateException("VLinker enclosing scale identity drift");
            }

            // Compact v1 validates every live target/draft before the first source mutation. These
            // are probe validation reads, not claims about lazy reads performed by the Java loop.
            if (stem.isManual()) {
                throw new IllegalStateException("manual stem is unsupported in compact head links");
            }
            int preflightOrdinal = 0;
            for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
                if (entry.getKey().getClass() != C_LINKER_CLASS
                        || !(entry.getKey() instanceof HeadLinker.SLinker.CLinker corner)
                        || entry.getValue().getClass() != HeadStemRelation.class) {
                    throw new IllegalStateException("non-exact head relation-map entry");
                }
                final HeadInter head = corner.getSource();
                final HeadStemRelation draft = (HeadStemRelation) entry.getValue();
                final Integer headOrdinal = headOrdinals.get(head);
                if (headOrdinal == null || cAliases.get(corner) == null
                        || !sig.containsVertex(head) || head.getSig() != sig
                        || head.isRemoved() || head.isManual() || draft.isManual()
                        || HEAD_STEM_CONSISTENCY.get(draft) != null
                        || draft.getHeadSide() == null || draft.getExtensionPoint() == null
                        || before.inters.objectMatches(head) != 1
                        || before.inters.idMatches(head.getId()) != 1
                        || graphBefore.vertexObjectMatches(head) != 1) {
                    throw new IllegalStateException(
                            "unsupported head/draft preflight at map ordinal "
                                    + preflightOrdinal);
                }
                preflightOrdinal++;
            }

            final IdentityHashMap<Relation, String> relationObjectAliases =
                    new IdentityHashMap<>();
            for (int ordinal = 0; ordinal < graphBefore.edges.size(); ordinal++) {
                final Relation relation = graphBefore.edges.get(ordinal);
                final String objectAlias = relationObjectIdentity(predecessor, relation);
                if (objectAlias.equals("-")) {
                    throw new IllegalStateException(
                            "Boundary-16 graph relation lacks typed object identity");
                }
                relationObjectAliases.put(relation, objectAlias);
            }
            int mapOrdinal = 0;
            for (Relation relation : relations.values()) {
                if (relationObjectAliases.put(
                        relation, "head-draft:" + base.ref.plan + ":" + mapOrdinal) != null) {
                    throw new IllegalStateException("plan draft is already attached to SIG");
                }
                mapOrdinal++;
            }

            final List<HeadEntryTrace> traces = new ArrayList<>();
            final List<InsertedHeadRelation> inserted = new ArrayList<>();
            final BaseStemState stemAtEntry = new BaseStemState(stem);
            int eventOrdinal = 0;
            mapOrdinal = 0;
            for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
                final HeadLinker.SLinker.CLinker corner =
                        (HeadLinker.SLinker.CLinker) entry.getKey();
                final HeadInter head = corner.getSource();
                final HeadLinker.SLinker side = corner.getSLinker();
                final HeadStemRelation draft = (HeadStemRelation) entry.getValue();
                final String cAlias = cAliases.get(corner);
                final String headAlias = interAlias(head);
                final String sAlias = "s:" + headOrdinals.get(head) + ":"
                        + side.getHorizontalSide();
                final String draftIdentity = relationObjectAliases.get(draft);
                final HeadObjectState headBefore = new HeadObjectState(head);
                final boolean stemAbnormalBeforeEntry = stem.isAbnormal();
                final SLinkerCellState sideBefore = SLinkerCellState.capture(side);
                final String relationStateBefore = headDraftState(draft);

                final int sWriteEvent = eventOrdinal++;
                side.setLinked(true);
                final SLinkerCellState sideAfterWrite = SLinkerCellState.capture(side);
                if (!sideBefore.sameExceptLinked(sideAfterWrite) || !sideAfterWrite.linked()) {
                    throw new IllegalStateException("SLinker assignment drift");
                }

                final GraphOrder pairGraph = new GraphOrder(sig);
                final HeadPairScan pair = HeadPairScan.capture(sig, head, stem, pairGraph);
                final Relation selected = sig.getRelation(
                        head, stem, HeadStemRelation.class);
                if (selected != pair.selected) {
                    throw new IllegalStateException("directed HeadStem lookup drift");
                }

                HeadConsistencyTrace consistency = null;
                HeadCallbackTrace callback = null;
                int consistencyEvent = -1;
                int edgeEvent = -1;
                int callbackEvent = -1;
                boolean insertionReturned = false;
                String graphRelationIdentity = "-";
                final String branch;
                if (selected != null) {
                    branch = "ExistingHeadStem";
                } else {
                    branch = "Linked";
                    final boolean small = head.getShape().isSmallHead();
                    final Line2D median = stem.getMedian();
                    final int interline = sourceScale.getInterline();
                    final double scaledStemLength =
                            (median.getY2() - median.getY1()) / interline;
                    final double ratio = scaledStemLength / neutral;
                    final double expectedConsistency = small ? 1.0 / ratio : ratio;
                    final Double consistencyBefore = (Double) HEAD_STEM_CONSISTENCY.get(draft);
                    final boolean debugEnabled = org.slf4j.LoggerFactory
                            .getLogger(HeadStemRelation.class).isDebugEnabled();
                    consistencyEvent = eventOrdinal++;
                    draft.setConsistency(small, scaledStemLength);
                    final Double consistencyAfter = (Double) HEAD_STEM_CONSISTENCY.get(draft);
                    if (consistencyBefore != null || consistencyAfter == null) {
                        throw new IllegalStateException("HeadStem consistency null-state drift");
                    }
                    requireBits(
                            expectedConsistency, consistencyAfter,
                            "HeadStem final consistency");
                    consistency = new HeadConsistencyTrace(
                            true, head.getShape(), small, copy(median), interline,
                            scaledStemLength, neutral, ratio, consistencyBefore,
                            consistencyAfter, debugEnabled);

                    final HorizontalSide headSideBefore = draft.getHeadSide();
                    final Point2D extensionBefore = draft.getExtensionPoint();
                    final boolean relationManual = draft.isManual();
                    final boolean headManual = head.isManual();
                    final boolean stemManual = stem.isManual();
                    final boolean headAbnormalBefore = head.isAbnormal();
                    final boolean stemAbnormalBefore = stem.isAbnormal();
                    final DirtyFlags callbackDirtyBefore = DirtyFlags.capture(sheet);
                    final GraphOrder beforeInsertion = new GraphOrder(sig);
                    edgeEvent = eventOrdinal++;
                    insertionReturned = sig.addEdge(head, stem, draft);
                    final GraphOrder afterInsertion = new GraphOrder(sig);
                    callbackEvent = eventOrdinal++;
                    if (!insertionReturned
                            || afterInsertion.edges.size() != beforeInsertion.edges.size() + 1
                            || afterInsertion.edgeOrdinals.get(draft)
                                    != beforeInsertion.edges.size()) {
                        throw new IllegalStateException("real HeadStem edge insertion drift");
                    }
                    graphRelationIdentity = afterInsertion.edgeAlias(draft);
                    final boolean stemHead = ShapeSet.StemHeads.contains(head.getShape());
                    final HeadStemIncidentTrace headIncidents = stemHead
                            ? HeadStemIncidentTrace.capture(sig, head, afterInsertion)
                            : HeadStemIncidentTrace.notRead("NotReadNonStemHead");
                    final HeadStemIncidentTrace stemIncidents =
                            HeadStemIncidentTrace.capture(sig, stem, afterInsertion);
                    final DirtyFlags callbackDirtyAfter = DirtyFlags.capture(sheet);
                    if (headSideBefore == null || extensionBefore == null
                            || draft.getHeadSide() != headSideBefore
                            || draft.getExtensionPoint() != extensionBefore
                            || relationManual || headManual || stemManual
                            || (stemHead && headIncidents.requestedAbnormal)
                            || stemIncidents.requestedAbnormal
                            || (stemHead && head.isAbnormal()) || stem.isAbnormal()) {
                        throw new IllegalStateException("real HeadStem callback envelope drift");
                    }
                    final boolean anyAbnormalChange =
                            headAbnormalBefore != head.isAbnormal()
                                    || stemAbnormalBefore != stem.isAbnormal();
                    if (anyAbnormalChange) {
                        if (!callbackDirtyAfter.stubModified()
                                || !callbackDirtyAfter.bookModified()
                                || !callbackDirtyAfter.bookDirty()) {
                            throw new IllegalStateException("abnormal dirty cascade did not complete");
                        }
                    } else if (!callbackDirtyBefore.equals(callbackDirtyAfter)) {
                        throw new IllegalStateException("callback dirtied sheet without abnormal change");
                    }
                    callback = new HeadCallbackTrace(
                            false, false, relationManual, true, headManual, true, stemManual,
                            "NotReadAuto", headIncidents, stemIncidents,
                            headAbnormalBefore, head.isAbnormal(), stemAbnormalBefore,
                            stem.isAbnormal(), callbackDirtyBefore, callbackDirtyAfter);
                    inserted.add(new InsertedHeadRelation(
                            mapOrdinal, head, stem, draft, draftIdentity,
                            graphRelationIdentity));
                }
                final HeadObjectState headAfter = new HeadObjectState(head);
                final boolean stemAbnormalAfterEntry = stem.isAbnormal();
                final SLinkerCellState sideAfter = SLinkerCellState.capture(side);
                if (!headBefore.sameExceptAbnormal(headAfter)
                        || !sideBefore.sameExceptLinked(sideAfter)
                        || !sideAfter.linked()) {
                    throw new IllegalStateException("head/SLinker payload drift");
                }
                if (selected != null
                        && (headBefore.abnormal != headAfter.abnormal
                                || stemAbnormalBeforeEntry != stemAbnormalAfterEntry
                                || !relationStateBefore.equals(headDraftState(draft))
                                || new GraphOrder(sig).edgeOrdinals.containsKey(draft))) {
                    throw new IllegalStateException(
                            "duplicate HeadStem branch crossed its lazy no-op envelope");
                }
                traces.add(new HeadEntryTrace(
                        mapOrdinal, entry.getKey(), corner, head, side, cAlias, sAlias,
                        headAlias, draft, draftIdentity, headBefore, headAfter,
                        stemAbnormalBeforeEntry, stemAbnormalAfterEntry,
                        sideBefore, sideAfter, sWriteEvent, pair, branch, consistency,
                        consistencyEvent, graphRelationIdentity, edgeEvent, insertionReturned,
                        callback, callbackEvent, relationStateBefore, headDraftState(draft)));
                mapOrdinal++;
            }

            final int builderItemCount = ((List<?>) STEM_BUILDER_ITEMS.get(builder)).size();
            final int maxIndex = builderItemCount - 1;
            final boolean remainderPresent = lastIndex < maxIndex;
            // The Java body is a comment (`stemDraft.splitAfter(lastIndex)`), so this comparison
            // is the final read before the unconditional `return true`.
            final PersistentSnapshot after = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final GraphOrder graphAfter = new GraphOrder(sig);
            final DirtyFlags dirtyAfter = DirtyFlags.capture(sheet);
            final String relationInputHashAfter = relationInputHash(relations, cAliases);
            final String headPlanStateHashAfter = headRelationMapStateHash(relations, cAliases);
            final BaseStemState stemAfter = new BaseStemState(stem);
            if (!stemAtEntry.sameExceptAbnormal(stemAfter)
                    || builderItemCount != builderItemsBefore.size()
                    || !builderItemsHash.equals(builderItemsHash(
                            (List<?>) STEM_BUILDER_ITEMS.get(builder)))) {
                throw new IllegalStateException("head loop mutated stem geometry/builder payload");
            }
            assertHeadLinksDelta(
                    before, after, graphBefore, graphAfter, traces, inserted,
                    relations, relationObjectAliases, vertexStateBefore, stem);
            return new HeadLinksRun(
                    predecessor, before, after, graphBefore, graphAfter, dirtyBefore, dirtyAfter,
                    traces, inserted, relationObjectAliases, lastIndex, maxIndex,
                    builderItemCount, remainderPresent, relationInputHashBefore,
                    relationInputHashAfter, headPlanStateHashBefore, headPlanStateHashAfter,
                    builderItemsHash, stemAtEntry, stemAfter,
                    eventOrdinal);
        }

        void assertHeadLinksDelta (PersistentSnapshot before,
                                   PersistentSnapshot after,
                                   GraphOrder graphBefore,
                                   GraphOrder graphAfter,
                                   List<HeadEntryTrace> traces,
                                   List<InsertedHeadRelation> inserted,
                                   LinkedHashMap<StemLinker, Relation> relations,
                                   IdentityHashMap<Relation, String> relationObjectAliases,
                                   IdentityHashMap<Inter, String> vertexStateBefore,
                                   StemInter stem)
        {
            if (before.allocator != after.allocator
                    || !before.glyphs.sameIdentityState(after.glyphs)
                    || !identitySetEquals(before.inters.identities, after.inters.identities)
                    || !before.systemStems.sameMapIdentity(after.systemStems)
                    || !before.lines.same(after.lines)
                    || graphBefore.vertices.size() != graphAfter.vertices.size()
                    || graphAfter.edges.size() != graphBefore.edges.size() + inserted.size()) {
                throw new IllegalStateException("head loop crossed a forbidden registry/topology boundary");
            }
            for (int index = 0; index < graphBefore.vertices.size(); index++) {
                if (graphBefore.vertices.get(index) != graphAfter.vertices.get(index)) {
                    throw new IllegalStateException("head loop reordered SIG vertices");
                }
            }
            for (int index = 0; index < graphBefore.edges.size(); index++) {
                if (graphBefore.edges.get(index) != graphAfter.edges.get(index)) {
                    throw new IllegalStateException("head loop reordered baseline SIG edges");
                }
            }
            for (Map.Entry<Relation, String> entry : before.sig.relationStates.entrySet()) {
                if (!entry.getValue().equals(after.sig.relationStates.get(entry.getKey()))) {
                    throw new IllegalStateException("head loop changed an existing SIG relation");
                }
            }
            for (int index = 0; index < inserted.size(); index++) {
                final InsertedHeadRelation appended = inserted.get(index);
                if (graphAfter.edges.get(graphBefore.edges.size() + index)
                        != appended.relation()
                        || appended.head().getSig().getEdgeSource(appended.relation())
                                != appended.head()
                        || appended.head().getSig().getEdgeTarget(appended.relation())
                                != appended.stem()) {
                    throw new IllegalStateException("HeadStem insertion order drift");
                }
            }
            final IdentityHashMap<StemLinker, Boolean> allowedSides = new IdentityHashMap<>();
            for (HeadEntryTrace trace : traces) {
                allowedSides.put(trace.side(), true);
                for (VerticalSide vSide : VerticalSide.values()) {
                    allowedSides.put(
                            trace.head().getLinker().getCornerLinker(
                                    trace.side().getHorizontalSide(), vSide),
                            true);
                }
            }
            final IdentityHashMap<Inter, Boolean> allowedInters = new IdentityHashMap<>();
            allowedInters.put(stem, true);
            for (HeadEntryTrace trace : traces) allowedInters.put(trace.head(), true);
            for (Map.Entry<Inter, String> entry : vertexStateBefore.entrySet()) {
                if (!allowedInters.containsKey(entry.getKey())
                        && !entry.getValue().equals(interStructuralToken(entry.getKey()))) {
                    throw new IllegalStateException("unrelated SIG vertex payload changed");
                }
            }
            if (before.linkers.state.size() != after.linkers.state.size()) {
                throw new IllegalStateException("head loop changed linker topology");
            }
            for (Map.Entry<StemLinker, String> entry : before.linkers.state.entrySet()) {
                final String afterValue = after.linkers.state.get(entry.getKey());
                if (allowedSides.containsKey(entry.getKey())) {
                    final String[] beforeParts = entry.getValue().split(":", 2);
                    if (beforeParts.length != 2
                            || !afterValue.equals("true:" + beforeParts[1])) {
                        throw new IllegalStateException("selected SLinker transition drift");
                    }
                } else if (!entry.getValue().equals(afterValue)) {
                    throw new IllegalStateException("unrelated linker flag changed");
                }
            }
            int mapOrdinal = 0;
            for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
                final HeadEntryTrace trace = traces.get(mapOrdinal);
                if (trace.mapKey() != entry.getKey() || trace.planRelation() != entry.getValue()
                        || !trace.planDraftIdentity().equals(
                                relationObjectAliases.get(entry.getValue()))) {
                    throw new IllegalStateException("head relation map identity/order drift");
                }
                mapOrdinal++;
            }
        }

        void emitHeadLinksRows (HeadLinksRun run)
            throws Exception
        {
            final BLinkerFlagRun base = run.predecessor.predecessor;
            final SiblingLinksRows b16 = expected.siblingLinks;
            final String prefix = page + " system " + system.getId() + " plan "
                    + base.ref.plan + " scope real case -";
            final StemInter stem = base.stem;
            final ListenerTopology listeners = ListenerTopology.capture(stem.getSig());
            System.out.printf(
                    "stemsbeamvlinkheadlinkspredecessor %s join FullBoundary16Replay "
                            + "b16TransactionRows %d b16TransactionEvidenceSha256 %s "
                            + "b16ResultRowSha256 %s b16GuardRowSha256 %s "
                            + "b16SummaryRowSha256 %s predecessorTerminal %s "
                            + "stemAlias %s stemInterId %d relationInputHash %s "
                            + "javaPredecessorStateSha256 %s proofDomain "
                            + "JavaOpaqueGuardRustFullTypedReplay%n",
                    prefix, b16.orderedRowCount, b16.orderedRowsSha256,
                    required(b16.result, "_rowSha256"), required(b16.guard, "_rowSha256"),
                    required(b16.summary, "_rowSha256"), required(b16.result, "terminal"),
                    base.stemAlias, stem.getId(), run.relationInputHashBefore,
                    run.before.provenanceHash());
            System.out.printf(
                    "stemsbeamvlinkheadlinksbaseline %s interline %d neutralStemLength %s "
                            + "relationEntries %d relationInputHash %s "
                            + "headPlanStateSha256 %s stemAlias %s "
                            + "stemInterId %d stemManual %s stemAbnormal %s "
                            + "graphVertices %d graphVertexSha256 %s graphEdges %d "
                            + "graphEdgeSha256 %s interIndexEntries %d interIndexSha256 %s "
                            + "builderItems %d builderItemsSha256 %s lastIndex %d "
                            + "maxIndex %d listenerTopologySha256 %s soleSigListener %s "
                            + "headless %s stubModified %s bookModified %s bookDirty %s "
                            + "eventStart 0%n",
                    prefix, sheet.getScale().getInterline(), hex(neutralStemLength()),
                    run.entries.size(), run.relationInputHashBefore,
                    run.headPlanStateHashBefore, base.stemAlias,
                    stem.getId(), stem.isManual(), run.stemBefore.abnormal,
                    run.graphBefore.vertices.size(), run.graphBefore.vertexHash,
                    run.graphBefore.edges.size(), run.graphBefore.edgeHash,
                    run.before.inters.identities.size(), run.before.inters.hash,
                    run.builderItemCount, run.builderItemsHash, run.lastIndex, run.maxIndex,
                    listeners.hash(), listeners.isStandard(), OMR.gui == null,
                    run.dirtyBefore.stubModified(), run.dirtyBefore.bookModified(),
                    run.dirtyBefore.bookDirty());

            int duplicateCount = 0;
            int consistencyWrites = 0;
            int headAbnormalChanges = 0;
            int stemAbnormalChanges = 0;
            int dirtyCascades = 0;
            int sValueChanges = 0;
            for (HeadEntryTrace trace : run.entries) {
                final HeadStemRelation draft = (HeadStemRelation) trace.planRelation();
                final String entryPrefix = prefix + " mapOrdinal " + trace.mapOrdinal();
                final List<String> observers = sObserverAliases(trace);
                final String staff = trace.head.getStaff() != null
                        ? "staff:" + trace.head.getStaff().getId() : "-";
                final String glyph = trace.head.getGlyph() != null
                        ? glyphToken(trace.head.getGlyph()) : "null";
                System.out.printf(
                        "stemsbeamvlinkheadlinksheadentry %s cAlias %s cRuntimeClass %s "
                                + "evidenceTiming BeforeEntryMutationSnapshot "
                                + "sAlias %s sRuntimeClass %s horizontalSide %s "
                                + "verticalSide %s observerAliases %s headAlias %s "
                                + "headSigOrdinal %d headXOrdinal %d "
                                + "headRuntimeClass %s headInterId %d headIndexOrdinal %s "
                                + "headIndexObjectMatches %d headIndexIdMatches %d "
                                + "headVertexOrdinal %s headVertexObjectMatches %d "
                                + "headSigSystemId %d shape %s "
                                + "small %s stemHead %s glyph %s staff %s center %s "
                                + "manual %s removed %s vip %s abnormalBefore %s "
                                + "sLinkedBefore %s sClosedBefore %s planDraftIdentity %s "
                                + "draftRuntimeClass %s draftManual %s draftHeadSide %s "
                                + "draftExtension %s draftConsistencyBeforeState Unset "
                                + "draftConsistencyBeforeValue - draftDx %s draftDy %s "
                                + "draftGrade %s draftImpacts %s draftGraphMatches 0 branch %s%n",
                        entryPrefix, trace.cAlias(), trace.corner().getClass().getName(),
                        trace.sAlias(), trace.side().getClass().getName(),
                        trace.sideBefore().horizontal(), C_V_SIDE.get(trace.corner()),
                        list(observers), trace.headAlias(), headSigOrdinals.get(trace.head),
                        headOrdinals.get(trace.head), trace.head.getClass().getName(),
                        trace.head.getId(), run.before.inters.indexOrdinal(trace.head),
                        run.before.inters.objectMatches(trace.head),
                        run.before.inters.idMatches(trace.head.getId()),
                        run.graphBefore.vertexOrdinal(trace.head),
                        run.graphBefore.vertexObjectMatches(trace.head),
                        trace.head.getSig().getSystem().getId(), trace.head.getShape(),
                        trace.head.getShape().isSmallHead(),
                        ShapeSet.StemHeads.contains(trace.head.getShape()), glyph, staff,
                        trace.head.getCenter().x + ":" + trace.head.getCenter().y,
                        trace.head.isManual(),
                        trace.head.isRemoved(), trace.head.isVip(), trace.headBefore().abnormal,
                        trace.sideBefore().linked(), trace.sideBefore().closed(),
                        trace.planDraftIdentity(), draft.getClass().getName(), draft.isManual(),
                        draft.getHeadSide(), point(draft.getExtensionPoint()),
                        hex(draft.getDx()), hex(draft.getDy()), hex(draft.getGrade()),
                        impactsToken(draft.getImpacts()), trace.branch());

                final boolean sChanged = trace.sideBefore().linked() != trace.sideAfter().linked();
                if (sChanged) sValueChanges++;
                System.out.printf(
                        "stemsbeamvlinkheadlinksslinkerwrite %s eventOrdinal %d "
                                + "receiverAlias %s receiverRuntimeClass %s declaringClass %s "
                                + "requested true before %s after %s writeCount 1 "
                                + "valueChangeCount %d closedBefore %s closedAfter %s "
                                + "observerAliases %s completed true%n",
                        entryPrefix, trace.sWriteEventOrdinal(), trace.sAlias(),
                        trace.side().getClass().getName(),
                        "org.audiveris.omr.sheet.stem.HeadLinker$SLinker",
                        trace.sideBefore().linked(), trace.sideAfter().linked(), sChanged ? 1 : 0,
                        trace.sideBefore().closed(), trace.sideAfter().closed(), list(observers));

                for (HeadSourceOutgoingRow row : trace.pair().outgoing) {
                    System.out.printf(
                            "stemsbeamvlinkheadlinkssourceoutgoing %s sourceOutgoingOrdinal %d "
                                    + "graphRelationIdentity %s relationObjectIdentity %s "
                                    + "runtimeClass %s targetVertexOrdinal %s%n",
                            entryPrefix, row.sourceOutgoingOrdinal(), row.graphRelationIdentity(),
                            headRelationObjectAlias(run, row.relation()), row.runtimeClass(),
                            row.targetVertexOrdinal());
                }
                for (HeadPairRow row : trace.pair().rows) {
                    System.out.printf(
                            "stemsbeamvlinkheadlinkspairrelation %s pairOrdinal %d "
                                    + "sourceOutgoingOrdinal %d graphRelationIdentity %s "
                                    + "relationObjectIdentity %s runtimeClass %s classRead %s "
                                    + "matches %s action %s%n",
                            entryPrefix, row.pairOrdinal(), row.sourceOutgoingOrdinal(),
                            row.graphRelationIdentity(),
                            headRelationObjectAlias(run, row.relation()), row.runtimeClass(),
                            row.classRead(), row.matches(), row.action());
                }
                final Relation selected = trace.pair().selected;
                System.out.printf(
                        "stemsbeamvlinkheadlinkspairscan %s state %s sourceOutgoingCount %d "
                                + "sourceOutgoingSha256 %s pairCount %d pairSha256 %s "
                                + "selectedGraphRelationIdentity %s "
                                + "selectedRelationObjectIdentity %s selectedRuntimeClass %s%n",
                        entryPrefix, trace.pair().state, trace.pair().outgoing.size(),
                        headOutgoingHash(run, trace.pair()), trace.pair().rows.size(),
                        headPairHash(run, trace.pair()),
                        selected != null ? trace.pair().rows.stream()
                                .filter(row -> row.relation() == selected)
                                .findFirst().orElseThrow().graphRelationIdentity() : "-",
                        selected != null ? headRelationObjectAlias(run, selected) : "-",
                        selected != null ? selected.getClass().getName() : "-");

                if (trace.consistency() != null) {
                    final HeadConsistencyTrace consistency = trace.consistency();
                    consistencyWrites++;
                    System.out.printf(
                            "stemsbeamvlinkheadlinksconsistency %s eventOrdinal %d "
                                    + "shapeRead %s shape %s small %s stemMedianRead true "
                                    + "stemMedian %s interlineRead true interline %d "
                                    + "scaledStemLength %s neutralStemLength %s ratio %s "
                                    + "consistencyBeforeState Unset consistencyBeforeValue - "
                                    + "consistencyAfterState Set consistencyAfterValue %s "
                                    + "debugEnabled %s completed true%n",
                            entryPrefix, trace.consistencyEventOrdinal(),
                            consistency.shapeRead(), consistency.shape(), consistency.small(),
                            line(consistency.medianCopy()), consistency.interline(),
                            hex(consistency.scaledStemLength()),
                            hex(consistency.neutralStemLength()), hex(consistency.ratio()),
                            hex(consistency.after()), consistency.debugEnabled());
                    System.out.printf(
                            "stemsbeamvlinkheadlinksedge %s eventOrdinal %d "
                                    + "graphRelationIdentity %s relationObjectIdentity %s "
                                    + "runtimeClass %s sourceAlias %s sourceInterId %d "
                                    + "sourceVertexOrdinal %s targetAlias %s targetInterId %d "
                                    + "targetVertexOrdinal %s graphInsertionOrdinal %d "
                                    + "insertionReturned %s callbackSynchronous true%n",
                            entryPrefix, trace.edgeEventOrdinal(),
                            trace.graphRelationIdentity(), trace.planDraftIdentity(),
                            draft.getClass().getName(), trace.headAlias(), trace.head.getId(),
                            run.graphAfter.vertexOrdinal(trace.head), base.stemAlias, stem.getId(),
                            run.graphAfter.vertexOrdinal(stem),
                            run.graphAfter.edgeOrdinals.get(draft), trace.insertionReturned());
                    emitHeadIncidentRows(
                            entryPrefix, "head", trace.callback().headIncidents(), run);
                    emitHeadIncidentRows(
                            entryPrefix, "stem", trace.callback().stemIncidents(), run);
                    final HeadCallbackTrace callback = trace.callback();
                    final boolean headChanged = callback.headAbnormalBefore()
                            != callback.headAbnormalAfter();
                    final boolean stemChanged = callback.stemAbnormalBefore()
                            != callback.stemAbnormalAfter();
                    if (headChanged) headAbnormalChanges++;
                    if (stemChanged) stemAbnormalChanges++;
                    dirtyCascades += (headChanged ? 1 : 0) + (stemChanged ? 1 : 0);
                    System.out.printf(
                            "stemsbeamvlinkheadlinkscallback %s eventOrdinal %d "
                                    + "relationObjectIdentity %s headSideWasNull %s "
                                    + "headSideAfter %s extensionWasNull %s extensionAfter %s "
                                    + "relationManual %s headManualRead %s headManual %s "
                                    + "stemManualRead %s stemManual %s chordBranch %s "
                                    + "headScanState %s headAbnormalRequested %s "
                                    + "stemScanState %s stemAbnormalRequested %s "
                                    + "headAbnormalBefore %s headAbnormalAfter %s "
                                    + "stemAbnormalBefore %s stemAbnormalAfter %s "
                                    + "stubModifiedBefore %s stubModifiedAfter %s "
                                    + "bookModifiedBefore %s bookModifiedAfter %s "
                                    + "bookDirtyBefore %s bookDirtyAfter %s completed true%n",
                            entryPrefix, trace.callbackEventOrdinal(), trace.planDraftIdentity(),
                            callback.headSideNull(), draft.getHeadSide(),
                            callback.extensionNull(), point(draft.getExtensionPoint()),
                            callback.relationManual(), callback.headManualRead(),
                            callback.headManual(), callback.stemManualRead(),
                            callback.stemManual(), callback.chordBranch(),
                            callback.headIncidents().state,
                            callback.headIncidents().requestedAbnormal,
                            callback.stemIncidents().state,
                            callback.stemIncidents().requestedAbnormal,
                            callback.headAbnormalBefore(), callback.headAbnormalAfter(),
                            callback.stemAbnormalBefore(), callback.stemAbnormalAfter(),
                            callback.dirtyBefore().stubModified(),
                            callback.dirtyAfter().stubModified(),
                            callback.dirtyBefore().bookModified(),
                            callback.dirtyAfter().bookModified(),
                            callback.dirtyBefore().bookDirty(),
                            callback.dirtyAfter().bookDirty());
                } else {
                    duplicateCount++;
                }
                System.out.printf(
                        "stemsbeamvlinkheadlinksentryresult %s branch %s "
                                + "sLinkedAfter %s sClosedAfter %s draftConsistencyAfterState %s "
                                + "draftConsistencyAfterValue %s graphRelationIdentity %s "
                                + "relationObjectIdentity %s insertionReturned %s "
                                + "callbackCompleted %s headAbnormalAfter %s "
                                + "stemAbnormalAfter %s relationStateBeforeSha256 %s "
                                + "relationStateAfterSha256 %s%n",
                        entryPrefix, trace.branch(), trace.sideAfter().linked(),
                        trace.sideAfter().closed(), trace.consistency() != null ? "Set" : "Unset",
                        trace.consistency() != null ? hex(trace.consistency().after()) : "-",
                        trace.graphRelationIdentity(), trace.planDraftIdentity(),
                        trace.consistency() != null
                                ? Boolean.toString(trace.insertionReturned()) : "NotRead",
                        trace.callback() != null, trace.headAfter().abnormal,
                        trace.stemAbnormalAfter(),
                        sha256Rows(List.of(trace.relationStateBefore())),
                        sha256Rows(List.of(trace.relationStateAfter())));
            }

            System.out.printf(
                    "stemsbeamvlinkheadlinksremainder %s lastIndex %d maxIndex %d "
                            + "builderItemCount %d comparisonEvaluated true remainderPresent %s "
                            + "splitBody CommentOnly splitCalls 0 returnedTrue true%n",
                    prefix, run.lastIndex, run.maxIndex, run.builderItemCount,
                    run.remainderPresent);
            System.out.printf(
                    "stemsbeamvlinkheadlinksresult %s headEntries %d duplicateEntries %d "
                            + "relationsInserted %d sWriteCount %d sValueChangeCount %d "
                            + "consistencyWriteCount %d headAbnormalChangeCount %d "
                            + "stemAbnormalChangeCount %d dirtyCascadeCount %d "
                            + "sheetEditMutationCount %d "
                            + "eventCount %d returnedTrue true terminal "
                            + "ReturnedTrueBeforeOuterBLinkerAssignment relationInputHash %s "
                            + "headPlanStateSha256 %s stateAfterSha256 %s%n",
                    prefix, run.entries.size(), duplicateCount, run.inserted.size(),
                    run.entries.size(), sValueChanges, consistencyWrites,
                    headAbnormalChanges, stemAbnormalChanges, dirtyCascades,
                    dirtyFlagMutationCount(run.dirtyBefore, run.dirtyAfter),
                    run.eventCount, run.relationInputHashAfter, run.headPlanStateHashAfter,
                    run.after.provenanceHash());
            System.out.printf(
                    "stemsbeamvlinkheadlinksdeltaguard %s allocatorUnchanged %s "
                            + "glyphRegistryUnchanged %s interIndexIdentityUnchanged %s "
                            + "sigVertexIdentityOrderUnchanged %s baselineEdgeIdentityOrderUnchanged %s "
                            + "appendedEdgesExactly %d systemStemsMapIdentityUnchanged %s "
                            + "beamLinesUnchanged %s builderItemsUnchanged %s "
                            + "relationMapIdentityOrderUnchanged %s "
                            + "legacyRelationInputPayloadHashChanged %s "
                            + "onlySelectedSCellsMayChange true onlyHeadStemAbnormalMayChange true "
                            + "manualChordBranchRead false splitCalls 0 "
                            + "outerBLinkerAssignmentRead false stopBeforeOuterBLinkerAssignment true "
                            + "headPlanStateChangedByConsistency %s%n",
                    prefix, run.before.allocator == run.after.allocator,
                    run.before.glyphs.sameIdentityState(run.after.glyphs),
                    identitySetEquals(run.before.inters.identities, run.after.inters.identities),
                    sameIdentityOrder(run.graphBefore.vertices, run.graphAfter.vertices),
                    sameIdentityPrefix(run.graphBefore.edges, run.graphAfter.edges),
                    run.inserted.size(), run.before.systemStems.sameMapIdentity(run.after.systemStems),
                    run.before.lines.same(run.after.lines), true, true,
                    !run.relationInputHashBefore.equals(run.relationInputHashAfter),
                    !run.headPlanStateHashBefore.equals(run.headPlanStateHashAfter));
            System.out.printf(
                    "stemsbeamvlinkheadlinkssummary %s headEntries %d duplicateEntries %d "
                            + "relationsInserted %d sWrites %d returnedTrue true terminal "
                            + "ReturnedTrueBeforeOuterBLinkerAssignment%n",
                    prefix, run.entries.size(), duplicateCount, run.inserted.size(),
                    run.entries.size());
        }

        private List<String> sObserverAliases (HeadEntryTrace trace)
        {
            final List<String> aliases = new ArrayList<>();
            for (VerticalSide vSide : VerticalSide.values()) {
                final Object observer = trace.head().getLinker().getCornerLinker(
                        trace.side().getHorizontalSide(), vSide);
                final String alias = cAliases.get(observer);
                if (alias == null) throw new IllegalStateException("SLinker observer lacks C alias");
                aliases.add(alias);
            }
            return aliases;
        }

        private String headRelationObjectAlias (HeadLinksRun run,
                                                Relation relation)
        {
            final String alias = run.relationObjectAliases.get(relation);
            if (alias == null) throw new IllegalStateException("head query relation lacks object alias");
            return alias;
        }

        private String headOutgoingHash (HeadLinksRun run,
                                         HeadPairScan scan)
        {
            final List<String> rows = new ArrayList<>();
            for (HeadSourceOutgoingRow row : scan.outgoing) {
                rows.add(row.sourceOutgoingOrdinal() + ":" + row.graphRelationIdentity()
                        + ":" + headRelationObjectAlias(run, row.relation()) + ":"
                        + row.runtimeClass() + ":" + row.targetVertexOrdinal());
            }
            return sha256Rows(rows);
        }

        private String headPairHash (HeadLinksRun run,
                                     HeadPairScan scan)
        {
            final List<String> rows = new ArrayList<>();
            for (HeadPairRow row : scan.rows) {
                rows.add(row.pairOrdinal() + ":" + row.sourceOutgoingOrdinal() + ":"
                        + row.graphRelationIdentity() + ":"
                        + headRelationObjectAlias(run, row.relation()) + ":"
                        + row.runtimeClass());
            }
            return sha256Rows(rows);
        }

        private String headIncidentHash (HeadLinksRun run,
                                         HeadStemIncidentTrace trace)
        {
            final List<String> rows = new ArrayList<>();
            for (HeadIncidentRow row : trace.rows) {
                rows.add(row.incidentOrdinal() + ":" + row.direction() + ":"
                        + row.directionOrdinal() + ":" + row.graphRelationIdentity() + ":"
                        + headRelationObjectAlias(run, row.relation()) + ":"
                        + row.relation().getClass().getName() + ":"
                        + headInterAlias(run, row.opposite())
                        + ":" + row.opposite().getId() + ":" + row.oppositeVertexOrdinal());
            }
            return sha256Rows(rows);
        }

        private void emitHeadIncidentRows (String entryPrefix,
                                           String endpoint,
                                           HeadStemIncidentTrace trace,
                                           HeadLinksRun run)
        {
            for (HeadIncidentRow row : trace.rows) {
                System.out.printf(
                        "stemsbeamvlinkheadlinks%sincident %s incidentOrdinal %d "
                                + "direction %s directionOrdinal %d graphRelationIdentity %s "
                                + "relationObjectIdentity %s runtimeClass %s oppositeAlias %s "
                                + "oppositeInterId %d oppositeVertexOrdinal %s classRead %s "
                                + "matches %s action %s%n",
                        endpoint, entryPrefix, row.incidentOrdinal(), row.direction(),
                        row.directionOrdinal(), row.graphRelationIdentity(),
                        headRelationObjectAlias(run, row.relation()),
                        row.relation().getClass().getName(), headInterAlias(run, row.opposite()),
                        row.opposite().getId(), row.oppositeVertexOrdinal(), row.classRead(),
                        row.matches(), row.action());
            }
            System.out.printf(
                    "stemsbeamvlinkheadlinks%sincidentscan %s state %s incidentCount %d "
                            + "incidentSha256 %s requestedAbnormal %s%n",
                    endpoint, entryPrefix, trace.state, trace.rows.size(),
                    headIncidentHash(run, trace), trace.requestedAbnormal);
        }

        private String headInterAlias (HeadLinksRun run,
                                       Inter inter)
        {
            final BLinkerFlagRun base = run.predecessor.predecessor;
            return inter == base.stem ? base.stemAlias : interAlias(inter);
        }

        void emitIsolatedHeadLinksCoverage (HeadLinksRun real)
            throws Exception
        {
            final PersistentSnapshot enclosingBefore = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final List<SyntheticHeadSpec> specs = List.of(
                    new SyntheticHeadSpec(
                            "Duplicate", Shape.NOTEHEAD_BLACK, true, false, false,
                            false, false, false),
                    new SyntheticHeadSpec(
                            "SmallHeadConsistency", Shape.NOTEHEAD_BLACK_SMALL, false, false,
                            false, false, false, false),
                    new SyntheticHeadSpec(
                            "NullSideExtension", Shape.NOTEHEAD_BLACK, false, true, false,
                            false, false, false),
                    new SyntheticHeadSpec(
                            "ManualNoChord", Shape.NOTEHEAD_BLACK, false, false, true,
                            false, false, false),
                    new SyntheticHeadSpec(
                            "ManualChordRewire", Shape.NOTEHEAD_BLACK, false, false, true,
                            true, false, false),
                    new SyntheticHeadSpec(
                            "PreInsertThrow", Shape.NOTEHEAD_BLACK, false, false, false,
                            false, true, false),
                    new SyntheticHeadSpec(
                            "CallbackThrow", Shape.NOTEHEAD_BLACK, false, false, false,
                            false, false, true));
            for (SyntheticHeadSpec spec : specs) {
                emitOneIsolatedHeadLinksCase(real, spec);
            }
            final PersistentSnapshot enclosingAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            enclosingBefore.assertSame(enclosingAfter);
        }

        void emitOneIsolatedHeadLinksCase (HeadLinksRun real,
                                           SyntheticHeadSpec spec)
            throws Exception
        {
            final IsolatedHeadFixture fixture = newIsolatedHeadFixture(spec);
            final SIGraph sig = fixture.sig();
            final GraphOrder graphBefore = new GraphOrder(sig);
            final DirtyFlags dirtyBefore = DirtyFlags.capture(fixture.sheet());
            final HeadObjectState headBefore = new HeadObjectState(fixture.head());
            final BaseStemState stemBefore = new BaseStemState(fixture.stem());
            final boolean sLinkedBefore = fixture.side().isLinked();
            final boolean sClosedBefore = fixture.side().isClosed();
            final List<Boolean> observerLinkedBefore = new ArrayList<>();
            final List<Boolean> observerClosedBefore = new ArrayList<>();
            for (VerticalSide vertical : VerticalSide.values()) {
                final HeadLinker.SLinker.CLinker observer = fixture.linker()
                        .getCornerLinker(HorizontalSide.RIGHT, vertical);
                observerLinkedBefore.add(observer.isLinked());
                observerClosedBefore.add(observer.isClosed());
            }
            final Double consistencyBefore = (Double) HEAD_STEM_CONSISTENCY.get(fixture.draft());
            final HorizontalSide headSideBefore = fixture.draft().getHeadSide();
            final Point2D extensionBefore = fixture.draft().getExtensionPoint();
            final Relation selectedBefore = sig.getRelation(
                    fixture.head(), fixture.stem(), HeadStemRelation.class);
            final int headStemBefore = countRelations(graphBefore, HeadStemRelation.class);
            final int chordStemBefore = countRelations(graphBefore, ChordStemRelation.class);

            boolean shapeRead = false;
            boolean medianRead = false;
            boolean interlineRead = false;
            boolean consistencyWritten = false;
            boolean insertionReturned = false;
            boolean addEdgeReturned = false;
            double scaledStemLength = Double.NaN;
            Throwable thrown = null;
            try {
                // Exact BeamLinker.VLinker.link entry body, without a fabricated VLinker/B16 state.
                final HeadInter head = fixture.corner().getSource();
                fixture.corner().getSLinker().setLinked(true);
                if (sig.getRelation(head, fixture.stem(), HeadStemRelation.class) == null) {
                    shapeRead = true;
                    final boolean small = head.getShape().isSmallHead();
                    medianRead = true;
                    final Line2D line = fixture.stem().getMedian();
                    interlineRead = true;
                    final int interline = fixture.scale().getInterline();
                    scaledStemLength = (line.getY2() - line.getY1()) / interline;
                    final HeadStemRelation relation = (HeadStemRelation) fixture.planRelation();
                    relation.setConsistency(small, scaledStemLength);
                    consistencyWritten = true;
                    addEdgeReturned = sig.addEdge(head, fixture.stem(), relation);
                    insertionReturned = true;
                }
            } catch (Throwable ex) {
                thrown = ex;
            }

            final GraphOrder graphAfter = new GraphOrder(sig);
            final DirtyFlags dirtyAfter = DirtyFlags.capture(fixture.sheet());
            final HeadObjectState headAfter = new HeadObjectState(fixture.head());
            final BaseStemState stemAfter = new BaseStemState(fixture.stem());
            final boolean sLinkedAfter = fixture.side().isLinked();
            final boolean sClosedAfter = fixture.side().isClosed();
            final Double consistencyAfter = (Double) HEAD_STEM_CONSISTENCY.get(fixture.draft());
            final HorizontalSide headSideAfter = fixture.draft().getHeadSide();
            final Point2D extensionAfter = fixture.draft().getExtensionPoint();
            final boolean draftAttachedBefore = graphBefore.edgeOrdinals.containsKey(fixture.draft());
            final boolean draftAttachedAfter = graphAfter.edgeOrdinals.containsKey(fixture.draft());
            final int headStemAfter = countRelations(graphAfter, HeadStemRelation.class);
            final int chordStemAfter = countRelations(graphAfter, ChordStemRelation.class);
            final boolean callbackCompleted = insertionReturned && addEdgeReturned && thrown == null;

            if (!sLinkedAfter || sClosedBefore != sClosedAfter
                    || !headBefore.sameExceptAbnormal(headAfter)
                    || !stemBefore.sameExceptAbnormal(stemAfter)
                    || consistencyBefore != null
                    || draftAttachedBefore
                    || fixture.draft().isManual() != spec.manual()
                    || !ListenerTopology.capture(sig).isStandard()) {
                throw new IllegalStateException(
                        "isolated head-links baseline/payload drift: " + spec.name());
            }
            int observerOrdinal = 0;
            for (VerticalSide vertical : VerticalSide.values()) {
                final HeadLinker.SLinker.CLinker observer = fixture.linker()
                        .getCornerLinker(HorizontalSide.RIGHT, vertical);
                if (observerLinkedBefore.get(observerOrdinal)
                        || !observer.isLinked()
                        || observerClosedBefore.get(observerOrdinal) != observer.isClosed()) {
                    throw new IllegalStateException(
                            "isolated S/C shared-cell observation drift: " + spec.name());
                }
                observerOrdinal++;
            }

            final boolean duplicate = spec.duplicate();
            final boolean missingBranch = !duplicate;
            final boolean preInsertThrow = spec.preInsertThrow();
            final boolean callbackThrow = spec.callbackThrow();
            final int expectedGraphDelta = duplicate || preInsertThrow ? 0 : 1;
            if ((selectedBefore != null) != duplicate
                    || shapeRead != missingBranch
                    || medianRead != missingBranch
                    || interlineRead != missingBranch
                    || consistencyWritten != missingBranch
                    || graphAfter.edges.size() - graphBefore.edges.size() != expectedGraphDelta
                    || draftAttachedAfter != (missingBranch && !preInsertThrow)
                    || headStemAfter - headStemBefore != (missingBranch && !preInsertThrow ? 1 : 0)) {
                throw new IllegalStateException(
                        "isolated head-links branch prefix drift: " + spec.name());
            }
            if (duplicate) {
                if (thrown != null || consistencyAfter != null || insertionReturned
                        || headSideAfter != headSideBefore
                        || extensionAfter != extensionBefore
                        || headBefore.abnormal != headAfter.abnormal
                        || stemBefore.abnormal != stemAfter.abnormal
                        || !dirtyBefore.equals(dirtyAfter)) {
                    throw new IllegalStateException("duplicate branch crossed lazy envelope");
                }
            } else {
                final double ratio = scaledStemLength / neutralStemLength();
                final double expectedConsistency = fixture.head().getShape().isSmallHead()
                        ? 1.0 / ratio : ratio;
                requireBits(
                        expectedConsistency, consistencyAfter,
                        "isolated HeadStem consistency " + spec.name());
                if (preInsertThrow) {
                    if (!(thrown instanceof IllegalArgumentException) || insertionReturned
                            || callbackCompleted || draftAttachedAfter
                            || headSideAfter != headSideBefore
                            || extensionAfter != extensionBefore
                            || headBefore.abnormal != headAfter.abnormal
                            || stemBefore.abnormal != stemAfter.abnormal
                            || !dirtyBefore.equals(dirtyAfter)) {
                        throw new IllegalStateException("pre-insert throw prefix drift");
                    }
                } else if (callbackThrow) {
                    if (!(thrown instanceof SyntheticHeadCallbackException)
                            || insertionReturned || callbackCompleted || !draftAttachedAfter
                            || headBefore.abnormal != headAfter.abnormal
                            || stemBefore.abnormal != stemAfter.abnormal
                            || !dirtyBefore.equals(dirtyAfter)) {
                        throw new IllegalStateException("callback throw prefix drift");
                    }
                } else if (!insertionReturned || !addEdgeReturned || !callbackCompleted
                        || thrown != null || headAfter.abnormal || stemAfter.abnormal
                        || !dirtyAfter.stubModified() || !dirtyAfter.bookModified()
                        || !dirtyAfter.bookDirty()) {
                    throw new IllegalStateException("supported isolated HeadStem did not complete");
                }
            }

            if (spec.nullFallback()) {
                if (headSideBefore != null || extensionBefore != null
                        || headSideAfter != HorizontalSide.RIGHT || extensionAfter == null) {
                    throw new IllegalStateException("null side/extension fallback drift");
                }
            } else if (!duplicate && !preInsertThrow
                    && (headSideAfter != headSideBefore || extensionAfter != extensionBefore)) {
                throw new IllegalStateException("prepopulated HeadStem payload was replaced");
            }

            Relation freshChordStem = null;
            for (Relation relation : graphAfter.edges) {
                if (relation instanceof ChordStemRelation
                        && !graphBefore.edgeOrdinals.containsKey(relation)) {
                    if (freshChordStem != null) {
                        throw new IllegalStateException("multiple fresh isolated ChordStem edges");
                    }
                    freshChordStem = relation;
                }
            }
            final boolean oldChordRetained = fixture.oldChordStem() != null
                    && graphAfter.edgeOrdinals.containsKey(fixture.oldChordStem());
            if (spec.chordRewire()) {
                if (oldChordRetained || freshChordStem == null
                        || fixture.chord().getStem() != fixture.stem()
                        || chordStemAfter != chordStemBefore) {
                    throw new IllegalStateException("manual chord rewire drift");
                }
            } else if (chordStemAfter != chordStemBefore || freshChordStem != null) {
                throw new IllegalStateException("unexpected isolated ChordStem mutation");
            }

            final String scope = duplicate || spec.shape().isSmallHead()
                    ? "synthetic" : "envelope";
            final String prefix = page + " system 1 plan "
                    + real.predecessor.predecessor.ref.plan
                    + " scope " + scope + " case " + spec.name();
            final String graphRelationIdentity = draftAttachedAfter
                    ? graphAfter.edgeAlias(fixture.draft())
                    : duplicate ? graphBefore.edgeAlias(selectedBefore) : "-";
            final String selectedIdentity = selectedBefore != null
                    ? graphBefore.edgeAlias(selectedBefore) : "-";
            final String throwStage = preInsertThrow ? "AddEdgeBeforeInsertion"
                    : callbackThrow ? "HeadCheckAbnormalDuringRelationCallback" : "-";
            final String terminal = thrown == null ? "IsolatedEntryCompleted"
                    : "Threw:" + throwStage;
            final boolean sideFallbackTaken = callbackCompleted && headSideBefore == null;
            final boolean extensionFallbackTaken = callbackCompleted && extensionBefore == null;
            final boolean manualBranchRead = callbackCompleted || callbackThrow;
            final boolean chordBranchRead = callbackCompleted && spec.manual();
            final boolean headAbnormalRead = callbackCompleted || callbackThrow;
            final boolean stemAbnormalRead = callbackCompleted;
            final List<SyntheticHeadEvent> events = new ArrayList<>();
            events.add(new SyntheticHeadEvent("SLinkerLinkedAssigned", "-"));
            events.add(new SyntheticHeadEvent("DirectedPairLookupCompleted", selectedIdentity));
            if (missingBranch) {
                events.add(new SyntheticHeadEvent("ConsistencyAssigned", "-"));
            }
            if (draftAttachedAfter) {
                events.add(new SyntheticHeadEvent("SigEdgeInserted", graphRelationIdentity));
                events.add(new SyntheticHeadEvent("HeadStemCallbackStarted", graphRelationIdentity));
            }
            if (spec.chordRewire()) {
                events.add(new SyntheticHeadEvent(
                        "OldChordStemRemoved", graphBefore.edgeAlias(fixture.oldChordStem())));
                events.add(new SyntheticHeadEvent(
                        "NewChordStemInserted", graphAfter.edgeAlias(freshChordStem)));
            }
            if (callbackCompleted) {
                events.add(new SyntheticHeadEvent(
                        "HeadStemCallbackCompleted", graphRelationIdentity));
            }
            if (thrown != null) {
                events.add(new SyntheticHeadEvent("Throw", graphRelationIdentity));
            }

            final double expectedConsistency = consistencyAfter != null
                    ? (fixture.head().getShape().isSmallHead()
                            ? 1.0 / (scaledStemLength / neutralStemLength())
                            : scaledStemLength / neutralStemLength())
                    : Double.NaN;
            System.out.printf(
                    "stemsbeamvlinkheadlinkssyntheticcase %s join IsolatedBoundary16Replay "
                            + "sourceRealB16EvidenceSha256 %s construction RealBookSheetSystemSIGHeadLinker "
                            + "shape %s small %s stemAttachedBefore %s soleSigListener true "
                            + "sLinkedBefore %s sLinkedAfter %s sClosedBefore %s sClosedAfter %s "
                            + "pairState %s selectedGraphRelationIdentity %s "
                            + "selectedRuntimeClass %s draftConsistencyBefore %s "
                            + "draftConsistencyAfter %s scaledStemLength %s neutralStemLength %s "
                            + "expectedConsistency %s headSideBefore %s headSideAfter %s "
                            + "extensionBefore %s extensionAfter %s draftAttachedBefore %s "
                            + "draftAttachedAfter %s relationManual %s headManual %s stemManual %s "
                            + "sideFallbackTaken %s extensionFallbackTaken %s manualBranchRead %s "
                            + "chordBranchRead %s headAbnormalRead %s stemAbnormalRead %s "
                            + "graphEdgesBefore %d graphEdgesAfter %d headStemBefore %d "
                            + "headStemAfter %d chordStemBefore %d chordStemAfter %d "
                            + "oldChordStemRetained %s newChordStemCount %d chordTargetsNewStem %s "
                            + "addEdgeReturned %s callbackState %s headAbnormalBefore %s "
                            + "headAbnormalAfter %s stemAbnormalBefore %s stemAbnormalAfter %s "
                            + "dirtyBefore %s dirtyAfter %s throwClass %s throwStage %s "
                            + "eventCount %d terminal %s%n",
                    prefix, expected.siblingLinks.orderedRowsSha256, fixture.head().getShape(),
                    fixture.head().getShape().isSmallHead(),
                    graphBefore.vertexOrdinals.containsKey(fixture.stem()), sLinkedBefore,
                    sLinkedAfter, sClosedBefore, sClosedAfter,
                    duplicate ? "FirstHeadStemMatch" : "ExhaustiveNoMatch", selectedIdentity,
                    selectedBefore != null ? selectedBefore.getClass().getName() : "-",
                    consistencyBefore != null ? hex(consistencyBefore) : "Unset",
                    consistencyAfter != null ? hex(consistencyAfter) : "Unset",
                    shapeRead ? hex(scaledStemLength) : "NotRead", hex(neutralStemLength()),
                    consistencyAfter != null ? hex(expectedConsistency) : "NotRead",
                    headSideBefore != null ? headSideBefore : "-",
                    headSideAfter != null ? headSideAfter : "-",
                    extensionBefore != null ? point(extensionBefore) : "-",
                    extensionAfter != null ? point(extensionAfter) : "-", draftAttachedBefore,
                    draftAttachedAfter, fixture.draft().isManual(), fixture.head().isManual(),
                    fixture.stem().isManual(), sideFallbackTaken, extensionFallbackTaken,
                    manualBranchRead, chordBranchRead, headAbnormalRead, stemAbnormalRead,
                    graphBefore.edges.size(), graphAfter.edges.size(), headStemBefore,
                    headStemAfter, chordStemBefore, chordStemAfter, oldChordRetained,
                    freshChordStem != null ? 1 : 0,
                    fixture.chord() != null && fixture.chord().getStem() == fixture.stem(),
                    insertionReturned ? Boolean.toString(addEdgeReturned) : "NotRead",
                    duplicate ? "NotReadDuplicate"
                            : preInsertThrow ? "NotReadPreInsertionThrow"
                                    : callbackThrow ? "StartedThrewInHeadAbnormal"
                                            : "Completed",
                    headBefore.abnormal, headAfter.abnormal, stemBefore.abnormal,
                    stemAfter.abnormal, dirtyBefore.token(), dirtyAfter.token(),
                    thrown != null ? thrown.getClass().getName() : "-", throwStage,
                    events.size(), terminal);
            for (int eventOrdinal = 0; eventOrdinal < events.size(); eventOrdinal++) {
                final SyntheticHeadEvent event = events.get(eventOrdinal);
                System.out.printf(
                        "stemsbeamvlinkheadlinkssyntheticevent %s eventOrdinal %d kind %s "
                                + "relationIdentity %s%n",
                        prefix, eventOrdinal, event.kind(), event.relationIdentity());
            }
            System.out.printf(
                    "stemsbeamvlinkheadlinkssyntheticguard %s graphDelta %d "
                            + "allowedMutations SelectedSSharedCellDraftConsistencyHeadStemCallback"
                            + "ManualChordRewireAbnormalDirty "
                            + "headPayloadUnchanged true stemPayloadUnchanged true "
                            + "closedFlagsUnchanged true unrelatedGraphPreserved true "
                            + "isolatedOnly true productionEquivalent false "
                            + "enclosingRealSheetUnchanged true outerBLinkerAssignmentRead false "
                            + "terminal %s%n",
                    prefix, graphAfter.edges.size() - graphBefore.edges.size(), terminal);
        }

        IsolatedHeadFixture newIsolatedHeadFixture (SyntheticHeadSpec spec)
            throws Exception
        {
            final Book book = new Book(Path.of(
                    "/private/tmp", "stems-beam-vlink-head-links-" + spec.name() + ".synthetic"));
            final SheetStub stub = new SheetStub(book, 1);
            book.addStub(stub);
            final Sheet isolatedSheet = new Sheet(stub, (RunTable) null);
            isolatedSheet.setScale(sheet.getScale());
            isolatedSheet.setSkew(new Skew());
            final Scale scale = isolatedSheet.getScale();
            final int interline = scale.getInterline();
            final List<LineInfo> staffLines = new ArrayList<>();
            for (int index = 0; index < 5; index++) {
                final double y = 60.0 + (index * interline);
                staffLines.add(new StaffLine(
                        List.of(new Point2D.Double(0.0, y), new Point2D.Double(160.0, y)),
                        1.0));
            }
            final Staff staff = new Staff(1, 0.0, 160.0, interline, staffLines);
            final SystemInfo system = new SystemInfo(1, isolatedSheet, List.of(staff));
            final Area area = new Area(new Rectangle(0, 0, 160, 200));
            system.setArea(new Area(area));
            staff.setArea(new Area(area));
            final Part part = new Part(system);
            part.addStaff(staff);
            system.addPart(part);
            isolatedSheet.getSystemManager().setSystems(List.of(system));
            isolatedSheet.getStaffManager().addStaff(staff);
            final SIGraph sig = system.getSig();
            if (!ListenerTopology.capture(sig).isStandard()) {
                throw new IllegalStateException("isolated head SIG lacks sole SigListener");
            }

            final StemsRetriever isolatedRetriever = new StemsRetriever(system);
            final Object params = PARAMETERS_CONSTRUCTOR.newInstance(system, scale);
            RETRIEVER_PARAMS.set(isolatedRetriever, params);
            RETRIEVER_SYSTEM_SEEDS.set(isolatedRetriever, new ArrayList<Glyph>());
            RETRIEVER_SYSTEM_BEAMS.set(isolatedRetriever, new ArrayList<Inter>());

            final Rectangle headBounds = new Rectangle(48, 78, 12, 10);
            final HeadInter head = spec.callbackThrow()
                    ? new ThrowingCheckHeadInter(headBounds, spec.shape(), 0.8, staff, 0.0)
                    : new HeadInter(headBounds, spec.shape(), 0.8, staff, 0.0);
            if (!sig.addVertex(head)) {
                throw new IllegalStateException("isolated head vertex setup drift");
            }
            RETRIEVER_SYSTEM_HEADS.set(isolatedRetriever, new ArrayList<Inter>(List.of(head)));
            final HeadLinker linker = new HeadLinker(head, isolatedRetriever);
            head.setLinker(linker);
            final HeadLinker.SLinker.CLinker corner = linker.getCornerLinker(
                    HorizontalSide.RIGHT, VerticalSide.BOTTOM);
            final HeadLinker.SLinker side = corner.getSLinker();

            final StemInter stem = new StemInter(null, 0.8);
            stem.setStaff(staff);
            stem.setWidth((double) scale.getStemThickness());
            stem.setMedian(
                    new Point2D.Double(61.0, 83.0),
                    new Point2D.Double(61.0, 83.0 + (2.0 * interline)));
            if (!sig.addVertex(stem)) {
                throw new IllegalStateException("isolated stem vertex setup drift");
            }

            final HeadStemRelation draft = new HeadStemRelation();
            if (!spec.nullFallback()) {
                draft.setHeadSide(HorizontalSide.RIGHT);
                draft.setExtensionPoint(corner.getReferencePoint());
            }
            draft.setManual(spec.manual());
            HeadStemRelation existing = null;
            if (spec.duplicate()) {
                existing = new HeadStemRelation();
                existing.setHeadSide(HorizontalSide.RIGHT);
                existing.setExtensionPoint(corner.getReferencePoint());
                if (!sig.addEdge(head, stem, existing)) {
                    throw new IllegalStateException("isolated duplicate setup drift");
                }
            }

            HeadChordInter chord = null;
            StemInter oldStem = null;
            Relation oldChordStem = null;
            if (spec.chordRewire()) {
                chord = new HeadChordInter(0.8);
                chord.setStaff(staff);
                if (!sig.addVertex(chord)) {
                    throw new IllegalStateException("isolated chord vertex setup drift");
                }
                chord.addMember(head);
                oldStem = new StemInter(null, 0.8);
                oldStem.setStaff(staff);
                oldStem.setWidth((double) scale.getStemThickness());
                oldStem.setMedian(
                        new Point2D.Double(42.0, 83.0),
                        new Point2D.Double(42.0, 83.0 + (2.0 * interline)));
                if (!sig.addVertex(oldStem)) {
                    throw new IllegalStateException("isolated old stem setup drift");
                }
                chord.setStem(oldStem);
                oldChordStem = sig.getRelation(chord, oldStem, ChordStemRelation.class);
                if (oldChordStem == null) {
                    throw new IllegalStateException("isolated old ChordStem setup drift");
                }
            }
            if (spec.preInsertThrow()) {
                stem.remove(false);
                if (sig.containsVertex(stem)) {
                    throw new IllegalStateException("isolated pre-insert endpoint remains live");
                }
            }
            clearSyntheticDirty(book, stub);
            return new IsolatedHeadFixture(
                    book, stub, isolatedSheet, system, sig, scale, staff, head, linker, corner,
                    side, stem, draft, draft, existing, chord, oldStem, oldChordStem);
        }

        int countRelations (GraphOrder graph,
                            Class<? extends Relation> relationClass)
        {
            int count = 0;
            for (Relation relation : graph.edges) {
                if (relationClass.isInstance(relation)) count++;
            }
            return count;
        }

        void assertBLinkerFlagExpected (BLinkerFlagRun run)
        {
            final BLinkerFlagRows rows = expected.bLinkerFlag;
            assertField(rows.predecessor, "plan", Integer.toString(run.ref.plan));
            assertField(rows.predecessor, "applyReturn", boolToken(run.baseRun.applyReturned));
            assertField(rows.predecessor, "supportGrade", run.baseEvidence.supportGrade);
            assertField(rows.predecessor, "stemAlias", run.stemAlias);
            assertField(rows.predecessor, "stemInterId", Integer.toString(run.stem.getId()));
            assertField(rows.predecessor, "bAlias", run.bAlias);
            assertField(rows.predecessor, "vAlias", run.vAlias);
            assertField(rows.target, "bAlias", run.bAlias);
            assertField(rows.target, "vAlias", run.vAlias);
            assertField(rows.target, "linkedBefore", Boolean.toString(run.linkedBefore));
            assertField(rows.target, "linkedAfter", Boolean.toString(run.linkedAfter));
            assertField(rows.write, "before", Boolean.toString(run.linkedBefore));
            assertField(rows.write, "after", Boolean.toString(run.linkedAfter));
            assertField(rows.write, "writeCount", "1");
            assertField(rows.write, "valueChangeCount", run.valueChanged ? "1" : "0");
            assertField(rows.result, "terminal", "ReadyBeforeSiblingBeamLinks");
            assertField(rows.result, "applyReturn", boolToken(run.baseRun.applyReturned));
            assertField(rows.result, "supportGrade", run.baseEvidence.supportGrade);
            assertField(rows.result, "stemAlias", run.stemAlias);
            assertField(rows.result, "stemInterId", Integer.toString(run.stem.getId()));
            assertField(rows.result, "bAlias", run.bAlias);
            assertField(rows.result, "linked", "true");
            assertField(rows.result, "linkSiblingsCalled", "false");
            assertField(rows.guard, "siblingDiscoveryRead", "false");
            assertField(rows.guard, "stopBeforeSiblingBeamLinks", "true");
            assertField(rows.guard, "stopBeforeHeadRelationLoop", "true");
            assertField(rows.summary, "terminal", "ReadyBeforeSiblingBeamLinks");
        }

        SiblingLinksRun executeSiblingLinks (BLinkerFlagRun predecessor,
                                              PersistentSnapshot persistentBefore,
                                              String relationInputHashBefore,
                                              LinkedHashMap<StemLinker, Relation> relations)
            throws Exception
        {
            final AbstractBeamInter baseBeam = predecessor.baseRun.beam;
            final StemInter stem = predecessor.stem;
            final BeamLinker beamLinker = baseBeam.getLinker();
            final SIGraph sig = baseBeam.getSig();
            if (beamLinker == null || sig == null || !sig.containsVertex(baseBeam)
                    || !sig.containsVertex(stem) || predecessor.currentV.getClass() != V_LINKER_CLASS) {
                throw new IllegalStateException("invalid compact sibling-links frontier");
            }
            final BeamGroupInter group = (BeamGroupInter) LINKER_BEAM_GROUP.get(beamLinker);
            if (group == null || group != baseBeam.getGroup() || !sig.containsVertex(group)) {
                throw new IllegalStateException("cached/live beam group identity drift");
            }
            final String groupObjectStateHashBefore = beamGroupObjectStateHash(baseBeam);
            final Line2D cachedMedian = (Line2D) LINKER_MEDIAN.get(beamLinker);
            if (cachedMedian != baseBeam.getMedian()) {
                throw new IllegalStateException("cached/base median identity drift");
            }
            final Point2D refPt = (Point2D) B_REF_PT.get(predecessor.b);
            final int yDir = (Integer) V_Y_DIR.get(predecessor.currentV);
            if (refPt == null || (yDir != -1 && yDir != 1)) {
                throw new IllegalStateException("invalid sibling reference geometry");
            }
            final double supportGrade = ((Support) predecessor.baseRun.link.relation).getGrade();
            if (!hex(supportGrade).equals(predecessor.baseEvidence.supportGrade)) {
                throw new IllegalStateException("Boundary-15 continuation grade drift");
            }
            final int maxBeamSideDx = PARAMETERS_MAX_BEAM_SIDE_DX.getInt(params);
            final double maxShorterRatio = maxShorterRatio();
            if (Double.doubleToRawLongBits(maxShorterRatio)
                    != Double.doubleToRawLongBits(0.8d)) {
                throw new IllegalStateException("maxShorterRatio runtime drift");
            }
            final double xInGapMaximum = BeamStemRelation.getXInGapMaximum(0).getValue();
            final int maxDx = sheet.getScale().toPixels(
                    BeamStemRelation.getXInGapMaximum(0));
            final Line2D skewedVertical = sheet.getSkew().skewedVertical(refPt);
            final Line2D stemMedian = stem.getMedian();
            final Point2D baseCross = LineUtil.intersection(stemMedian, cachedMedian);
            final double baseLength = cachedMedian.getX2() - cachedMedian.getX1();
            final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(predecessor.currentV);
            final List<?> builderItemsBefore = new ArrayList<>((List<?>) STEM_BUILDER_ITEMS.get(
                    builder));
            final String builderItemsHash = builderItemsHash(builderItemsBefore);
            final GraphOrder graphBefore = new GraphOrder(sig);
            final String groupStateHashBefore = beamGroupStateHash(baseBeam, graphBefore);
            final DirtyFlags dirtyBefore = DirtyFlags.capture(sheet);
            final BLinkerArenaSnapshot arenaBefore = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, predecessor.b);
            final StemIncidentScan stemIncidentsBefore = StemIncidentScan.capture(
                    sig, stem, graphBefore);
            if (stemIncidentsBefore.chordMatches != 0) {
                throw new IllegalStateException("compact sibling frontier has ChordStem match");
            }

            final List<GroupWork> groupWork = new ArrayList<>();
            final List<AbstractBeamInter> selected = new ArrayList<>();
            final IdentityHashMap<AbstractBeamInter, Integer> memberOrdinals =
                    new IdentityHashMap<>();
            final IdentityHashMap<Glyph, String> glyphAliases = new IdentityHashMap<>();
            int nextGlyphAlias = 0;
            int memberOrdinal = 0;
            int outgoingOrdinal = 0;
            for (Relation relation : sig.outgoingEdgesOf(group)) {
                final Inter target = sig.getEdgeTarget(relation);
                if (relation instanceof Containment) {
                    if (!(target instanceof AbstractBeamInter sibling)) {
                        throw new IllegalStateException("group Containment target is not beam");
                    }
                    if (memberOrdinals.put(sibling, memberOrdinal) != null) {
                        throw new IllegalStateException("duplicate beam identity in group members");
                    }
                    final Glyph glyph = sibling.getGlyph();
                    if (glyph != null && !glyphAliases.containsKey(glyph)) {
                        glyphAliases.put(glyph, "group-glyph:" + nextGlyphAlias++);
                    }
                    final Line2D median = sibling.getMedian();
                    final Point2D cross = LineUtil.intersection(skewedVertical, median);
                    final double leftLimit = median.getX1() - maxBeamSideDx;
                    final double rightLimit = median.getX2() + maxBeamSideDx;
                    final boolean inclusiveLeft = leftLimit <= cross.getX();
                    final boolean inclusiveRight = cross.getX() <= rightLimit;
                    final boolean include = inclusiveLeft && inclusiveRight;
                    if (include) selected.add(sibling);
                    groupWork.add(new GroupWork(
                            outgoingOrdinal, relation, target, sibling, memberOrdinal++, cross,
                            leftLimit, rightLimit, inclusiveLeft, inclusiveRight, include));
                } else {
                    groupWork.add(new GroupWork(
                            outgoingOrdinal, relation, target, null, -1, null,
                            Double.NaN, Double.NaN, false, false, false));
                }
                outgoingOrdinal++;
            }
            Collections.sort(
                    selected,
                    (left, right) -> Double.compare(
                            LineUtil.intersection(skewedVertical, left.getMedian()).getY(),
                            LineUtil.intersection(skewedVertical, right.getMedian()).getY()));
            final List<AbstractBeamInter> sourceSelected = beamLinker.getSiblingBeamsAt(refPt);
            assertIdentityList("getSiblingBeamsAt", selected, sourceSelected);
            final IdentityHashMap<AbstractBeamInter, Integer> sortedOrdinals =
                    new IdentityHashMap<>();
            int baseMatches = 0;
            for (int i = 0; i < sourceSelected.size(); i++) {
                final AbstractBeamInter candidate = sourceSelected.get(i);
                if (sortedOrdinals.put(candidate, i) != null) {
                    throw new IllegalStateException("duplicate selected sibling identity");
                }
                if (candidate == baseBeam) baseMatches++;
            }
            if (baseMatches != 1) {
                throw new IllegalStateException("base beam is not a unique selected group member");
            }
            final List<AbstractBeamInter> siblings = new ArrayList<>(sourceSelected);
            final boolean baseRemoved = siblings.remove(baseBeam);
            if (!baseRemoved || siblings.contains(baseBeam)) {
                throw new IllegalStateException("base beam removal drift");
            }
            final List<GroupScanRow> groupRows = new ArrayList<>();
            for (GroupWork work : groupWork) {
                final AbstractBeamInter sibling = work.beam;
                final String targetAlias = interAlias(work.target);
                final String sorted = sibling != null && sortedOrdinals.containsKey(sibling)
                        ? Integer.toString(sortedOrdinals.get(sibling)) : "-";
                final boolean isBase = sibling == baseBeam;
                groupRows.add(new GroupScanRow(
                        work.outgoingOrdinal, work.relation,
                        graphBefore.edgeAlias(work.relation),
                        "sig-relation-object:" + graphBefore.edgeOrdinals.get(work.relation),
                        work.relation.getClass().getName(), work.beam != null, work.target,
                        work.target.getClass().getName(),
                        work.beam != null ? "true" : "false",
                        work.beam != null ? "GetMembersRead" : "GraphReconstruction",
                        targetAlias, graphBefore.vertexOrdinal(work.target),
                        work.beam != null ? Integer.toString(work.memberOrdinal) : "-",
                        sibling != null ? sibling.getClass().getName() : "-",
                        sibling != null ? Integer.toString(sibling.getId()) : "-",
                        sibling != null ? persistentBefore.inters.indexOrdinal(sibling) : "-",
                        sibling != null ? Integer.toString(
                                persistentBefore.inters.objectMatches(sibling)) : "-",
                        sibling != null ? Integer.toString(
                                persistentBefore.inters.idMatches(sibling.getId())) : "-",
                        sibling != null ? Boolean.toString(sig.containsVertex(sibling)) : "-",
                        sibling != null ? Integer.toString(sig.getSystem().getId()) : "-",
                        sibling != null ? Boolean.toString(sibling.isRemoved()) : "-",
                        sibling != null ? Boolean.toString(sibling.isVip()) : "-",
                        sibling != null ? Boolean.toString(sibling.isAbnormal()) : "-",
                        sibling != null ? beamGroupOrdinal(sibling, graphBefore) : "-",
                        sibling != null ? beamGroupStateHash(sibling, graphBefore) : "-",
                        sibling != null ? beamGroupObjectStateHash(sibling) : "-",
                        sibling != null ? line(sibling.getMedian()) : "-",
                        sibling != null ? hex(sibling.getHeight()) : "-",
                        sibling != null ? glyphIdentity(sibling.getGlyph(), glyphAliases) : "-",
                        sibling != null ? nullableGlyphToken(sibling.getGlyph()) : "-",
                        work.cross != null ? point(work.cross) : "-",
                        work.beam != null ? hex(work.leftLimit) : "-",
                        work.beam != null ? hex(work.rightLimit) : "-",
                        work.beam != null ? Boolean.toString(work.inclusiveLeft) : "-",
                        work.beam != null ? Boolean.toString(work.inclusiveRight) : "-",
                        work.beam != null ? Boolean.toString(work.selected) : "-", sorted,
                        work.beam != null ? Boolean.toString(isBase) : "-",
                        isBase && work.selected ? "RemoveFirstBase" : "Retain"));
            }

            final List<SiblingPlan> plans = new ArrayList<>();
            for (int siblingOrdinal = 0; siblingOrdinal < siblings.size(); siblingOrdinal++) {
                final AbstractBeamInter sibling = siblings.get(siblingOrdinal);
                final Integer beamOrdinal = beamSigOrdinals.get(sibling);
                if (beamOrdinal == null) {
                    throw new IllegalStateException("group sibling lacks stable beam alias");
                }
                final String siblingAlias = "beam:" + beamOrdinal;
                final boolean sameGlyph = sibling.getGlyph() == baseBeam.getGlyph();
                SiblingPairScan pair = SiblingPairScan.notRead();
                Point2D cross = null;
                double siblingLength = Double.NaN;
                double ratio = Double.NaN;
                boolean shorter = false;
                double dy = Double.NaN;
                double product = Double.NaN;
                boolean wrongSide = false;
                Point2D extension = null;
                BeamPortion portion = null;
                String branch;
                BuilderLookup lookup = BuilderLookup.notRead();
                if (sameGlyph) {
                    branch = "SameGlyph";
                } else {
                    pair = SiblingPairScan.capture(sig, sibling, stem, graphBefore);
                    if (pair.firstMatch != null) {
                        branch = "ExistingBeamStem";
                    } else {
                        final Line2D siblingMedian = sibling.getMedian();
                        cross = LineUtil.intersection(stemMedian, siblingMedian);
                        siblingLength = siblingMedian.getX2() - siblingMedian.getX1();
                        ratio = siblingLength / baseLength;
                        shorter = ratio <= maxShorterRatio;
                        if (shorter) {
                            dy = cross.getY() - baseCross.getY();
                            product = dy * yDir;
                            wrongSide = product < 0;
                        }
                        if (wrongSide) {
                            branch = "ShorterWrongSide";
                        } else {
                            extension = new Point2D.Double(
                                    cross.getX(),
                                    cross.getY() - (yDir * (sibling.getHeight() / 2.0)));
                            portion = BeamStemRelation.computeBeamPortion(
                                    sibling, cross.getX(), sheet.getScale());
                            branch = "Linked";
                            lookup = captureBuilderLookup(builder, sibling);
                            if (lookup.selected != null
                                    && lookup.selected.getClass() != B_LINKER_CLASS) {
                                throw new IllegalStateException(
                                        "compact sibling lookup selected non-BLinker runtime");
                            }
                        }
                    }
                }
                plans.add(new SiblingPlan(
                        siblingOrdinal, sortedOrdinals.get(sibling), sibling, siblingAlias,
                        glyphIdentity(sibling.getGlyph(), glyphAliases), sameGlyph, pair, cross,
                        siblingLength, ratio, shorter, dy, product, wrongSide, extension, portion,
                        branch, lookup));
            }

            try {
                V_LINK_SIBLINGS.invoke(predecessor.currentV, stem, supportGrade);
            } catch (InvocationTargetException ex) {
                throw new IllegalStateException(
                        "real sibling-links seam threw", ex.getCause());
            }

            final GraphOrder graphAfter = new GraphOrder(sig);
            final DirtyFlags dirtyAfter = DirtyFlags.capture(sheet);
            final BLinkerArenaSnapshot arenaAfter = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, predecessor.b);
            if (!groupObjectStateHashBefore.equals(beamGroupObjectStateHash(baseBeam))) {
                throw new IllegalStateException("beam group object state changed at sibling seam");
            }
            final PersistentSnapshot persistentAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final List<?> builderItemsAfter = (List<?>) STEM_BUILDER_ITEMS.get(builder);
            assertIdentityList("StemBuilder.items", builderItemsBefore, builderItemsAfter);
            if (!builderItemsHash.equals(builderItemsHash(builderItemsAfter))) {
                throw new IllegalStateException("StemBuilder.items payload changed during callbacks");
            }
            final List<AcceptedSibling> accepted = new ArrayList<>();
            final IdentityHashMap<Relation, Boolean> acceptedRelations = new IdentityHashMap<>();
            for (SiblingPlan plan : plans) {
                if (!plan.branch.equals("Linked")) continue;
                BeamStemRelation fresh = null;
                final Set<Relation> pairAfter = sig.getAllEdges(plan.beam, stem);
                if (pairAfter != null) {
                    for (Relation relation : pairAfter) {
                        if (relation instanceof BeamStemRelation beamStem
                                && !graphBefore.edgeOrdinals.containsKey(relation)) {
                            if (fresh != null) {
                                throw new IllegalStateException(
                                        "multiple fresh sibling BeamStem relations");
                            }
                            fresh = beamStem;
                        }
                    }
                }
                if (fresh == null || acceptedRelations.put(fresh, true) != null) {
                    throw new IllegalStateException("missing/duplicate fresh sibling relation");
                }
                requirePointBits(plan.extension, fresh.getExtensionPoint(), "sibling extension");
                if (fresh.getBeamPortion() != plan.portion) {
                    throw new IllegalStateException("sibling beam portion drift");
                }
                requireBits(supportGrade, fresh.getGrade(), "sibling relation grade");
                final StemIncidentScan stemIncidents = StemIncidentScan.capturePrefix(
                        sig, stem, graphAfter, graphBefore, acceptedRelations);
                final BeamIncidentTrace beamIncidents = BeamIncidentTrace.capture(
                        sig, plan.beam, graphAfter);
                if (stemIncidents.chordMatches != 0) {
                    throw new IllegalStateException("compact sibling callback has ChordStem match");
                }
                final boolean linkerLinkedAfter = plan.lookup.selected == null
                        ? false : plan.lookup.selected.isLinked();
                if (plan.lookup.selected != null && !linkerLinkedAfter) {
                    throw new IllegalStateException("selected sibling BLinker was not linked");
                }
                accepted.add(new AcceptedSibling(
                        plan, fresh, graphAfter.edgeAlias(fresh), stemIncidents,
                        beamIncidents, new BeamObjectState(plan.beam), linkerLinkedAfter));
            }
            assertSiblingLinksDelta(
                    predecessor, plans, accepted, persistentBefore, persistentAfter,
                    graphBefore, graphAfter, arenaBefore, arenaAfter);
            final String relationInputHashAfter = relationInputHash(relations, cAliases);
            if (!relationInputHashBefore.equals(relationInputHashAfter)) {
                throw new IllegalStateException("sibling links mutated head-relation input map");
            }
            return new SiblingLinksRun(
                    predecessor, persistentBefore, persistentAfter, graphBefore, graphAfter,
                    dirtyBefore, dirtyAfter, arenaBefore, arenaAfter, stemIncidentsBefore,
                    groupRows, plans, accepted,
                    "beam-group:" + graphBefore.vertexOrdinal(group),
                    graphBefore.vertexOrdinal(group), groupStateHashBefore,
                    groupObjectStateHashBefore,
                    skewedVertical, baseCross, baseLength,
                    supportGrade, maxBeamSideDx, maxShorterRatio, xInGapMaximum, maxDx,
                    baseRemoved, relationInputHashBefore, relationInputHashAfter,
                    builderItemsBefore.size(), builderItemsHash);
        }

        String builderItemsHash (List<?> items)
            throws Exception
        {
            final List<String> rows = new ArrayList<>();
            for (int ordinal = 0; ordinal < items.size(); ordinal++) {
                final Object item = items.get(ordinal);
                final StringBuilder row = new StringBuilder()
                        .append(ordinal).append(':').append(item.getClass().getName());
                if (item instanceof StemItem.LinkerItem) {
                    final StemLinker linker = (StemLinker) LINKER_ITEM_LINKER.get(item);
                    row.append(":linker=").append(linkerAlias(linker))
                            .append(":source=").append(interAlias(linker.getSource()));
                }
                rows.add(row.toString());
            }
            return sha256Rows(rows);
        }

        BuilderLookup captureBuilderLookup (StemBuilder builder,
                                            AbstractBeamInter target)
            throws Exception
        {
            final List<?> items = (List<?>) STEM_BUILDER_ITEMS.get(builder);
            final List<BuilderItemRow> rows = new ArrayList<>();
            StemLinker selected = null;
            String selectedAlias = "-";
            boolean selectedLinked = false;
            boolean selectedClosed = false;
            boolean done = false;
            for (int ordinal = 0; ordinal < items.size(); ordinal++) {
                final Object item = items.get(ordinal);
                if (done) {
                    rows.add(new BuilderItemRow(
                            ordinal, "-", "NotRead", "NotRead", "-", "-", "-", "-",
                            "-", "UnreadAfterBreak"));
                    continue;
                }
                if (!(item instanceof StemItem.LinkerItem)) {
                    rows.add(new BuilderItemRow(
                            ordinal, item.getClass().getName(), "NotLinkerItem", "NotRead",
                            "-", "-", "-", "-", "false", "Continue"));
                    continue;
                }
                final StemLinker linker = (StemLinker) LINKER_ITEM_LINKER.get(item);
                final Inter source = linker.getSource();
                final boolean matches = source == target;
                final String alias = linkerAlias(linker);
                rows.add(new BuilderItemRow(
                        ordinal, item.getClass().getName(), "ReadLinker", "ReadSource",
                        alias, linker.getClass().getName(), interAlias(source),
                        Integer.toString(source.getId()), Boolean.toString(matches),
                        matches ? "SelectBreak" : "Continue"));
                if (matches) {
                    selected = linker;
                    selectedAlias = alias;
                    selectedLinked = linker.isLinked();
                    selectedClosed = linker.isClosed();
                    done = true;
                }
            }
            return new BuilderLookup(
                    selected != null ? "FirstSourceIdentityMatch" : "ExhaustiveNoMatch",
                    "ReconstructedFromImmutableItems", rows, selected, selectedAlias,
                    selectedLinked, selectedClosed);
        }

        String linkerAlias (StemLinker linker)
            throws Exception
        {
            final String bAlias = bAliases.get(linker);
            if (bAlias != null) return bAlias;
            final String cAlias = cAliases.get(linker);
            if (cAlias != null) return cAlias;
            if (linker.getClass() == V_LINKER_CLASS) {
                final Object b = V_OUTER_B.get(linker);
                final String parent = bAliases.get(b);
                if (parent == null) throw new IllegalStateException("VLinker lacks B alias");
                return parent + ":v:" + V_V_SIDE.get(linker);
            }
            return "linker:" + linker.getClass().getSimpleName() + ":" + interAlias(
                    linker.getSource());
        }

        String interAlias (Inter inter)
        {
            final Integer beamOrdinal = beamSigOrdinals.get(inter);
            if (beamOrdinal != null) return "beam:" + beamOrdinal;
            final Integer headOrdinal = inter instanceof HeadInter
                    ? headOrdinals.get((HeadInter) inter) : null;
            if (headOrdinal != null) return "head:" + headOrdinal;
            return "inter:" + inter.getId();
        }

        void assertSiblingLinksDelta (BLinkerFlagRun predecessor,
                                      List<SiblingPlan> plans,
                                      List<AcceptedSibling> accepted,
                                      PersistentSnapshot before,
                                      PersistentSnapshot after,
                                      GraphOrder graphBefore,
                                      GraphOrder graphAfter,
                                      BLinkerArenaSnapshot arenaBefore,
                                      BLinkerArenaSnapshot arenaAfter)
            throws Exception
        {
            if (before.allocator != after.allocator
                    || !before.glyphs.sameIdentityState(after.glyphs)
                    || !identitySetEquals(before.inters.identities, after.inters.identities)
                    || !identitySetEquals(before.sig.vertices, after.sig.vertices)
                    || !before.systemStems.same(after.systemStems)
                    || !before.lines.same(after.lines)) {
                throw new IllegalStateException("sibling links crossed an unrelated registry seam");
            }
            if (graphBefore.vertices.size() != graphAfter.vertices.size()) {
                throw new IllegalStateException("sibling links changed SIG vertex count");
            }
            for (int i = 0; i < graphBefore.vertices.size(); i++) {
                if (graphBefore.vertices.get(i) != graphAfter.vertices.get(i)) {
                    throw new IllegalStateException("sibling links changed SIG vertex order");
                }
            }
            if (graphAfter.edges.size() != graphBefore.edges.size() + accepted.size()) {
                throw new IllegalStateException("sibling edge count delta drift");
            }
            for (int i = 0; i < graphBefore.edges.size(); i++) {
                final Relation relation = graphBefore.edges.get(i);
                if (graphAfter.edges.get(i) != relation
                        || !before.sig.relationStates.get(relation).equals(
                                after.sig.relationStates.get(relation))) {
                    throw new IllegalStateException("existing SIG relation changed/reordered");
                }
            }
            for (int i = 0; i < accepted.size(); i++) {
                if (graphAfter.edges.get(graphBefore.edges.size() + i)
                        != accepted.get(i).relation) {
                    throw new IllegalStateException("fresh sibling relation insertion order drift");
                }
            }
            if (!predecessor.baseRun.after.stem.same(new BaseStemState(predecessor.stem))
                    || !predecessor.baseRun.after.beam.same(
                            new BeamObjectState(predecessor.baseRun.beam))) {
                throw new IllegalStateException("base beam/stem changed at sibling seam");
            }
            final IdentityHashMap<AbstractBeamInter, AcceptedSibling> acceptedByBeam =
                    new IdentityHashMap<>();
            final IdentityHashMap<Object, Boolean> selectedCells = new IdentityHashMap<>();
            for (AcceptedSibling row : accepted) {
                acceptedByBeam.put(row.plan.beam, row);
                if (row.plan.lookup.selected != null) {
                    final Object parentB = row.plan.lookup.selected;
                    if (bAliases.get(parentB) == null) {
                        throw new IllegalStateException("selected B lacks stable alias");
                    }
                    selectedCells.put(parentB, true);
                }
            }
            for (SiblingPlan plan : plans) {
                final BeamObjectState post = new BeamObjectState(plan.beam);
                if (!plan.beamBefore.sameExceptAbnormal(post)) {
                    throw new IllegalStateException("sibling beam payload changed beyond abnormal");
                }
                final AcceptedSibling acceptedRow = acceptedByBeam.get(plan.beam);
                if (acceptedRow == null) {
                    if (plan.beamBefore.abnormal != post.abnormal) {
                        throw new IllegalStateException("skipped sibling abnormal flag changed");
                    }
                } else if (post.abnormal != acceptedRow.beamIncidents.requestedAbnormal) {
                    throw new IllegalStateException("sibling callback abnormal request drift");
                }
            }
            if (arenaBefore.entries.size() != arenaAfter.entries.size()
                    || !arenaBefore.topologyHash.equals(arenaAfter.topologyHash)) {
                throw new IllegalStateException("B-linker arena topology changed");
            }
            for (int i = 0; i < arenaBefore.entries.size(); i++) {
                final BLinkerArenaEntry left = arenaBefore.entries.get(i);
                final BLinkerArenaEntry right = arenaAfter.entries.get(i);
                if (left.identity != right.identity || !left.sameTopology(right)
                        || left.closed != right.closed) {
                    throw new IllegalStateException("B-linker arena identity/closed drift");
                }
                if (selectedCells.containsKey(left.identity)) {
                    if (!right.linked) {
                        throw new IllegalStateException("selected sibling B cell was not linked");
                    }
                } else if (left.linked != right.linked) {
                    throw new IllegalStateException("unrelated B cell changed");
                }
            }
            final IdentityHashMap<StemLinker, Boolean> allowedLinkers = new IdentityHashMap<>();
            for (Object parentB : selectedCells.keySet()) {
                allowedLinkers.put((StemLinker) parentB, true);
                for (Object child : ((Map<VerticalSide, Object>) B_V_LINKERS.get(parentB)).values()) {
                    allowedLinkers.put((StemLinker) child, true);
                }
            }
            before.linkers.assertSelectedBCellsAfterTrue(after.linkers, allowedLinkers);
        }

        void emitSiblingLinksRows (SiblingLinksRun run)
            throws Exception
        {
            final BLinkerFlagRun pre = run.predecessor;
            final BLinkerFlagRows b15 = expected.bLinkerFlag;
            final String prefix = page + " system " + system.getId() + " plan " + pre.ref.plan
                    + " scope " + pre.scope + " case " + pre.caseName;
            final String baseBeamAlias = "beam:" + pre.before.targetEntry().beamSig;
            System.out.printf(
                    "stemsbeamvlinksiblinglinkspredecessor %s join FullBoundary15Replay "
                            + "b15TransactionRows %d b15TransactionEvidenceSha256 %s "
                            + "b15ResultRowSha256 %s b15GuardRowSha256 %s "
                            + "b15SummaryRowSha256 %s predecessorTerminal %s applyReturn %s "
                            + "supportGrade %s stemAlias %s stemInterId %d "
                            + "baseBeamAlias %s targetBAlias %s triggeringVAlias %s "
                            + "targetBLinked %s%n",
                    prefix, b15.orderedRowCount, b15.orderedRowsSha256,
                    required(b15.result, "_rowSha256"), required(b15.guard, "_rowSha256"),
                    required(b15.summary, "_rowSha256"), required(b15.result, "terminal"),
                    boolToken(pre.baseRun.applyReturned), hex(run.supportGrade), pre.stemAlias,
                    pre.stem.getId(), baseBeamAlias, pre.bAlias, pre.vAlias,
                    pre.linkedAfter);

            int memberCount = 0;
            int selectedCount = 0;
            for (GroupScanRow row : run.groupRows) {
                if (row.containment) memberCount++;
                if ("true".equals(row.selected)) selectedCount++;
            }
            final BeamGroupInter group = pre.baseRun.beam.getGroup();
            System.out.printf(
                    "stemsbeamvlinksiblinglinksbaseline %s baseBeamAlias %s "
                            + "baseBeamClass %s baseBeamInterId %d baseBeamVertexOrdinal %s "
                            + "baseBeamGlyphIdentity %s baseBeamGlyph %s baseBeamMedian %s "
                            + "baseBeamHeight %s baseBeamAbnormal %s stemAlias %s stemInterId %d "
                            + "stemMedian %s stemAbnormal %s cachedMedianSameIdentity true "
                            + "refPt %s yDir %d skewedVertical %s groupAlias %s "
                            + "groupClass %s groupInterId %d groupVertexOrdinal %s "
                            + "groupRemoved %s groupVip %s groupAbnormal %s "
                            + "groupStateHashBefore %s groupObjectStateHash %s "
                            + "groupOutgoingScanned %d "
                            + "groupQueryProvenanceSha256 %s groupMembers %d "
                            + "selectedBeforeBaseRemoval %d baseRemoved %s siblings %d "
                            + "maxBeamSideDx %d maxShorterRatio %s "
                            + "interline %d xInGapMaximum0 %s maxDxRint %d baseCross %s "
                            + "baseLength %s supportGradeSource FreshBaseBeamStemDraft "
                            + "supportGradeRead true supportGrade %s graphVertices %d "
                            + "graphEdgesBefore %d graphVertexHashBefore %s graphEdgeHashBefore %s "
                            + "listenerTopologyHash %s soleSigListener %s stemIncidentState %s "
                            + "stemIncidentRows %d chordStemMatches %d stemIncidentHash %s "
                            + "builderItems %d builderItemsHash %s arenaStateHashBefore %s "
                            + "relationInputHashBefore %s%n",
                    prefix, baseBeamAlias, pre.baseRun.beam.getClass().getName(),
                    pre.baseRun.beam.getId(), run.graphBefore.vertexOrdinal(pre.baseRun.beam),
                    glyphIdentityFromRows(pre.baseRun.beam.getGlyph(), run.groupRows),
                    nullableGlyphToken(pre.baseRun.beam.getGlyph()),
                    line(pre.baseRun.beam.getMedian()), hex(pre.baseRun.beam.getHeight()),
                    pre.baseRun.beam.isAbnormal(), pre.stemAlias, pre.stem.getId(),
                    line(pre.stem.getMedian()), pre.stem.isAbnormal(),
                    point((Point2D) B_REF_PT.get(pre.b)), V_Y_DIR.getInt(pre.currentV),
                    line(run.skewedVertical), run.groupAlias, group.getClass().getName(),
                    group.getId(), run.groupVertexOrdinal, group.isRemoved(), group.isVip(),
                    group.isAbnormal(), run.groupStateHashBefore, run.groupObjectStateHash,
                    run.groupRows.size(),
                    groupQueryHash(run.groupRows), memberCount, selectedCount, run.baseRemoved,
                    run.siblings.size(), run.maxBeamSideDx,
                    hex(run.maxShorterRatio), sheet.getScale().getInterline(),
                    hex(run.xInGapMaximum), run.maxDx, point(run.baseCross),
                    hex(run.baseLength), hex(run.supportGrade), run.graphBefore.vertices.size(),
                    run.graphBefore.edges.size(), run.graphBefore.vertexHash,
                    run.graphBefore.edgeHash, pre.baseRun.after.listeners.hash(),
                    pre.baseRun.after.listeners.isStandard(), run.stemIncidentsBefore.state,
                    run.stemIncidentsBefore.rows.size(), run.stemIncidentsBefore.chordMatches,
                    siblingStemIncidentHash(run, run.stemIncidentsBefore),
                    run.builderItemsCount, run.builderItemsHash,
                    run.arenaBefore.stateHash, run.relationInputHashBefore);

            for (GroupScanRow row : run.groupRows) {
                System.out.printf(
                        "stemsbeamvlinksiblinglinksgroupmember %s groupOutgoingOrdinal %d "
                                + "graphRelationIdentity %s relationObjectIdentity %s "
                                + "relationClass %s containmentMatch %s targetAlias %s "
                                + "targetRuntimeClass %s targetReadByGetMembers %s "
                                + "targetEvidence %s "
                                + "targetInterId %d targetVertexOrdinal %s memberOrdinal %s "
                                + "beamRuntimeClass %s beamInterId %s interIndexOrdinal %s "
                                + "interIndexObjectMatches %s interIndexIdMatches %s "
                                + "sigMembership %s sigSystemId %s beamRemoved %s beamVip %s "
                                + "beamAbnormal %s beamGroupVertexOrdinal %s "
                                + "beamGroupStateHash %s beamGroupObjectStateHash %s "
                                + "median %s height %s "
                                + "glyphIdentity %s glyph %s verticalCross %s "
                                + "leftLimit %s rightLimit %s inclusiveLeft %s "
                                + "inclusiveRight %s selected %s "
                                + "sortedOrdinal %s baseIdentity %s removeAction %s%n",
                        prefix, row.outgoingOrdinal, row.graphRelationIdentity,
                        row.relationObjectIdentity, row.runtimeClass, row.containment,
                        row.targetAlias, row.targetRuntimeClass,
                        row.targetReadByGetMembers, row.targetEvidence, row.target.getId(),
                        row.targetVertexOrdinal, row.memberOrdinal, row.beamRuntimeClass,
                        row.beamInterId, row.interIndexOrdinal, row.interIndexObjectMatches,
                        row.interIndexIdMatches, row.sigMembership, row.sigSystemId,
                        row.beamRemoved, row.beamVip, row.beamAbnormal,
                        row.beamGroupVertexOrdinal, row.beamGroupStateHash,
                        row.beamGroupObjectStateHash, row.median,
                        row.height, row.glyphIdentity, row.glyph, row.cross, row.leftLimit,
                        row.rightLimit, row.inclusiveLeft, row.inclusiveRight, row.selected,
                        row.sortedOrdinal,
                        row.baseIdentity, row.removeAction);
            }

            final IdentityHashMap<SiblingPlan, AcceptedSibling> acceptedByPlan =
                    new IdentityHashMap<>();
            for (AcceptedSibling accepted : run.accepted) {
                acceptedByPlan.put(accepted.plan, accepted);
            }
            int eventOrdinal = 0;
            int committedEdges = 0;
            int committedFlags = 0;
            DirtyFlags dirtyCursor = run.dirtyBefore;
            final List<String> committedEdgeAliases = new ArrayList<>();
            final List<String> committedBCells = new ArrayList<>();
            final IdentityHashMap<Object, Boolean> linkedCursor = new IdentityHashMap<>();
            for (BLinkerArenaEntry entry : run.arenaBefore.entries) {
                linkedCursor.put(entry.identity, entry.linked);
            }
            for (SiblingPlan plan : run.siblings) {
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssibling %s siblingOrdinal %d "
                                + "sortedOrdinal %d beamAlias %s beamClass %s beamInterId %d "
                                + "beamVertexOrdinal %s glyphIdentity %s glyph %s "
                                + "baseGlyphSameIdentity %s pairState %s "
                                + "sourceOutgoingScanned %d sourceOutgoingProvenanceSha256 %s "
                                + "pairRows %d pairProvenanceSha256 %s "
                                + "firstBeamStemIdentity %s branch %s%n",
                        prefix, plan.siblingOrdinal, plan.sortedOrdinal, plan.beamAlias,
                        plan.beam.getClass().getName(), plan.beam.getId(),
                        run.graphBefore.vertexOrdinal(plan.beam), plan.glyphIdentity,
                        nullableGlyphToken(plan.beam.getGlyph()), plan.sameGlyph,
                        plan.pair.state, plan.pair.sourceOutgoingScanned,
                        plan.pair.sourceOutgoingHash, plan.pair.rows.size(), plan.pair.hash,
                        plan.pair.firstMatch != null
                                ? run.graphBefore.edgeAlias(plan.pair.firstMatch) : "-",
                        plan.branch);
                for (SiblingSourceOutgoingRow row : plan.pair.sourceOutgoingRows) {
                    System.out.printf(
                            "stemsbeamvlinksiblinglinkssourceoutgoing %s siblingOrdinal %d "
                                    + "sourceOutgoingOrdinal %d graphRelationIdentity %s "
                                    + "relationObjectIdentity %s runtimeClass %s%n",
                            prefix, plan.siblingOrdinal, row.sourceOutgoingOrdinal,
                            row.graphRelationIdentity, row.relationObjectIdentity,
                            row.runtimeClass);
                }
                for (SiblingPairRow row : plan.pair.rows) {
                    System.out.printf(
                            "stemsbeamvlinksiblinglinkspairrelation %s siblingOrdinal %d "
                                    + "pairOrdinal %d sourceOutgoingOrdinal %s "
                                    + "graphRelationIdentity %s relationObjectIdentity %s "
                                    + "runtimeClass %s classRead %s matches %s action %s%n",
                            prefix, plan.siblingOrdinal, row.pairOrdinal,
                            row.sourceOutgoingOrdinal, row.graphRelationIdentity,
                            row.relationObjectIdentity, row.runtimeClass, row.classRead,
                            row.matches, row.action);
                }
                if (!plan.sameGlyph && plan.pair.firstMatch == null) {
                    final Line2D siblingMedian = plan.beam.getMedian();
                    final String dyRead = plan.shorter ? hex(plan.dy) : "-";
                    final String productRead = plan.shorter ? hex(plan.product) : "-";
                    final String wrongRead = plan.shorter
                            ? Boolean.toString(plan.wrongSide) : "-";
                    final String extension = plan.branch.equals("Linked")
                            ? point(plan.extension) : "-";
                    final String beamHalfHeight = plan.branch.equals("Linked")
                            ? hex(plan.beam.getHeight() / 2.0) : "-";
                    final String leftThreshold = plan.branch.equals("Linked")
                            ? hex(siblingMedian.getX1() + run.maxDx) : "-";
                    final String rightThreshold = plan.branch.equals("Linked")
                            ? hex(siblingMedian.getX2() - run.maxDx) : "-";
                    final String strictLeft = plan.branch.equals("Linked")
                            ? Boolean.toString(plan.cross.getX()
                                    < siblingMedian.getX1() + run.maxDx) : "-";
                    final String strictRight = plan.branch.equals("Linked")
                            ? Boolean.toString(plan.cross.getX()
                                    > siblingMedian.getX2() - run.maxDx) : "-";
                    System.out.printf(
                            "stemsbeamvlinksiblinglinksgeometry %s siblingOrdinal %d "
                                    + "stemMedian %s baseMedian %s siblingMedian %s "
                                    + "baseCross %s siblingCross %s baseLength %s "
                                    + "siblingLength %s ratio %s maxShorterRatio %s "
                                    + "shorterInclusive %s dyRead %s dy %s yDir %d "
                                    + "product %s wrongSideStrict %s extension %s "
                                    + "beamHalfHeight %s xInGapMaximum0 %s maxDxRint %d "
                                    + "leftThreshold %s rightThreshold %s strictLeft %s "
                                    + "strictRight %s portion %s gradeSource %s "
                                    + "gradeRead %s grade %s branch %s%n",
                            prefix, plan.siblingOrdinal, line(pre.stem.getMedian()),
                            line(pre.baseRun.beam.getMedian()), line(siblingMedian),
                            point(run.baseCross), point(plan.cross), hex(run.baseLength),
                            hex(plan.siblingLength), hex(plan.ratio),
                            hex(run.maxShorterRatio), plan.shorter,
                            plan.shorter ? "true" : "false", dyRead,
                            (Integer) V_Y_DIR.get(pre.currentV), productRead, wrongRead,
                            extension, beamHalfHeight, hex(run.xInGapMaximum), run.maxDx,
                            leftThreshold, rightThreshold, strictLeft, strictRight,
                            plan.branch.equals("Linked") ? plan.portion.toString() : "-",
                            plan.branch.equals("Linked") ? "FreshBaseBeamStemDraft" : "NotRead",
                            plan.branch.equals("Linked") ? "true" : "false",
                            plan.branch.equals("Linked") ? hex(run.supportGrade) : "-",
                            plan.branch);
                }
                final AcceptedSibling accepted = acceptedByPlan.get(plan);
                if (accepted != null) {
                    final int edgeEvent = eventOrdinal++;
                    final int callbackEvent = eventOrdinal++;
                    committedEdges++;
                    committedEdgeAliases.add(accepted.graphRelationIdentity);
                    final int sourceIncidence = identityOrdinal(
                            pre.baseRun.beam.getSig().outgoingEdgesOf(plan.beam),
                            accepted.relation);
                    final int targetIncidence = identityOrdinal(
                            pre.baseRun.beam.getSig().incomingEdgesOf(pre.stem),
                            accepted.relation);
                    System.out.printf(
                            "stemsbeamvlinksiblinglinksedge %s siblingOrdinal %d "
                                    + "eventOrdinal %d freshRelationIdentity sibling-draft:%d:%d "
                                    + "relationClass %s sourceAlias %s sourceInterId %d "
                                    + "targetAlias %s targetInterId %d insertionReturn true "
                                    + "returnObservedByCaller false graphRelationIdentity %s "
                                    + "graphInsertionOrdinal %d sourceOutgoingOrdinal %d "
                                    + "targetIncomingOrdinal %d extension %s portion %s grade %s%n",
                            prefix, plan.siblingOrdinal, edgeEvent, pre.ref.plan,
                            plan.siblingOrdinal, accepted.relation.getClass().getName(),
                            plan.beamAlias, plan.beam.getId(), pre.stemAlias, pre.stem.getId(),
                            accepted.graphRelationIdentity,
                            run.graphAfter.edgeOrdinals.get(accepted.relation), sourceIncidence,
                            targetIncidence, point(accepted.relation.getExtensionPoint()),
                            accepted.relation.getBeamPortion(), hex(accepted.relation.getGrade()));
                    for (IncidentRow row : accepted.stemIncidents.rows) {
                        System.out.printf(
                                "stemsbeamvlinksiblinglinksstemincident %s siblingOrdinal %d "
                                        + "eventOrdinal %d incidentOrdinal %d direction %s "
                                        + "directionOrdinal %d graphRelationIdentity %s "
                                        + "relationObjectIdentity %s runtimeClass %s "
                                        + "classRead true oppositeReadByGetChords %s "
                                        + "oppositeEvidence %s "
                                        + "oppositeAlias %s oppositeInterId %d "
                                        + "oppositeVertexOrdinal %s chordStemMatch %s%n",
                                prefix, plan.siblingOrdinal, callbackEvent,
                                row.incidentOrdinal, row.direction, row.directionOrdinal,
                                row.globalIdentity, relationObjectIdentity(run, row.relation),
                                row.relation.getClass().getName(), row.chordStemMatch,
                                row.chordStemMatch ? "GetChordsRead" : "GraphReconstruction",
                                siblingInterAlias(run, row.opposite),
                                row.opposite.getId(), row.oppositeVertexOrdinal,
                                row.chordStemMatch);
                    }
                    for (BeamIncidentRow row : accepted.beamIncidents.rows) {
                        System.out.printf(
                                "stemsbeamvlinksiblinglinksbeamincident %s siblingOrdinal %d "
                                        + "eventOrdinal %d incidentOrdinal %d direction %s "
                                        + "directionOrdinal %d graphRelationIdentity %s "
                                        + "relationObjectIdentity %s runtimeClass %s "
                                        + "classRead %s oppositeReadByCheckAbnormal false "
                                        + "oppositeEvidence GraphReconstruction "
                                        + "oppositeAlias %s oppositeInterId %d "
                                        + "oppositeVertexOrdinal %s readState %s relevant %s "
                                        + "portion %s contribution %s%n",
                                prefix, plan.siblingOrdinal, callbackEvent,
                                row.incidentOrdinal, row.direction, row.directionOrdinal,
                                row.globalIdentity, relationObjectIdentity(run, row.relation),
                                row.relation.getClass().getName(),
                                !row.readState.equals("UnreadAfterBreak"),
                                siblingInterAlias(run, row.opposite),
                                row.opposite.getId(), row.oppositeVertexOrdinal, row.readState,
                                row.relevant, row.portion, row.contribution);
                    }
                    final boolean abnormalChanged = plan.beamBefore.abnormal
                            != accepted.beamAfter.abnormal;
                    final DirtyFlags callbackBefore = dirtyCursor;
                    if (abnormalChanged) dirtyCursor = new DirtyFlags(true, true, true);
                    System.out.printf(
                            "stemsbeamvlinksiblinglinkscallback %s siblingOrdinal %d "
                                    + "eventOrdinal %d relationAdded true extensionPrepopulated true "
                                    + "portionPrepopulated true defaultExtensionBranchRead false "
                                    + "defaultPortionBranchRead false stemIncidentState %s "
                                    + "stemIncidentRows %d chordStemMatches %d stemIncidentHash %s "
                                    + "beamIncidentState %s beamRule %s beamIncidentRows %d "
                                    + "beamIncidentHash %s requestedAbnormal %s "
                                    + "beamAbnormalBefore %s beamAbnormalAfter %s "
                                    + "abnormalChanged %s dirtyBefore %s dirtyAfter %s%n",
                            prefix, plan.siblingOrdinal, callbackEvent,
                            accepted.stemIncidents.state, accepted.stemIncidents.rows.size(),
                            accepted.stemIncidents.chordMatches,
                            siblingStemIncidentHash(run, accepted.stemIncidents),
                            accepted.beamIncidents.state, accepted.beamIncidents.rule,
                            accepted.beamIncidents.rows.size(),
                            siblingBeamIncidentHash(run, accepted.beamIncidents),
                            accepted.beamIncidents.requestedAbnormal,
                            plan.beamBefore.abnormal, accepted.beamAfter.abnormal,
                            abnormalChanged, callbackBefore.token(), dirtyCursor.token());
                    for (BuilderItemRow row : plan.lookup.rows) {
                        System.out.printf(
                                "stemsbeamvlinksiblinglinkslinkerlookup %s siblingOrdinal %d "
                                        + "itemOrdinal %d runtimeClass %s linkerRead %s "
                                        + "sourceRead %s linkerAlias %s linkerRuntimeClass %s "
                                        + "sourceAlias %s sourceInterId %s identityMatch %s "
                                        + "action %s%n",
                                prefix, plan.siblingOrdinal, row.itemOrdinal, row.runtimeClass,
                                row.linkerRead, row.sourceRead, row.linkerAlias,
                                row.linkerRuntimeClass, row.sourceAlias, row.sourceInterId,
                                row.identityMatch, row.action);
                    }
                    if (plan.lookup.selected != null) {
                        final int flagEvent = eventOrdinal++;
                        final BLinkerArenaEntry beforeEntry = arenaEntry(
                                run.arenaBefore, plan.lookup.selected);
                        final BLinkerArenaEntry afterEntry = arenaEntry(
                                run.arenaAfter, plan.lookup.selected);
                        final Boolean linkedBeforeEvent = linkedCursor.get(plan.lookup.selected);
                        if (linkedBeforeEvent == null || !afterEntry.linked) {
                            throw new IllegalStateException(
                                    "selected B cell missing from serial linked cursor");
                        }
                        linkedCursor.put(plan.lookup.selected, true);
                        committedFlags++;
                        committedBCells.add(plan.lookup.selectedAlias);
                        System.out.printf(
                                "stemsbeamvlinksiblinglinkslinkerflag %s siblingOrdinal %d "
                                        + "eventOrdinal %d lookupState FirstSourceIdentityMatch "
                                        + "selectedAlias %s selectedRuntimeClass %s "
                                        + "selectedSourceAlias %s sharedCell bcell:%s "
                                        + "observerAliases %s linkedBefore %s linkedAfter %s "
                                        + "closedBefore %s closedAfter %s requested true "
                                        + "writeCount 1 valueChangeCount %d transition %s%n",
                                prefix, plan.siblingOrdinal, flagEvent,
                                plan.lookup.selectedAlias,
                                plan.lookup.selected.getClass().getName(), plan.beamAlias,
                                plan.lookup.selectedAlias,
                                list(observerAliases(beforeEntry)), linkedBeforeEvent,
                                true, beforeEntry.closed, afterEntry.closed,
                                linkedBeforeEvent ? 0 : 1,
                                linkedBeforeEvent ? "TrueToTrueIdempotent" : "FalseToTrue");
                    } else {
                        System.out.printf(
                                "stemsbeamvlinksiblinglinkslinkerflag %s siblingOrdinal %d "
                                        + "eventOrdinal - lookupState ExhaustiveNoMatch "
                                        + "selectedAlias - selectedRuntimeClass - "
                                        + "selectedSourceAlias - sharedCell - observerAliases - "
                                        + "linkedBefore - linkedAfter - closedBefore - closedAfter - "
                                        + "requested NotRead writeCount 0 valueChangeCount 0 "
                                        + "transition NoLinkerNoWrite%n",
                                prefix, plan.siblingOrdinal);
                    }
                }
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssiblingresult %s siblingOrdinal %d "
                                + "branch %s edgeCommitted %s linkerLookupRead %s "
                                + "linkerLookupState %s linkerLookupTiming %s "
                                + "linkerLookupRows %d linkerLookupHash %s "
                                + "linkerSelectedAlias %s edgePrefixCount %d flagPrefixCount %d "
                                + "terminal Continue%n",
                        prefix, plan.siblingOrdinal, plan.branch,
                        Boolean.toString(accepted != null),
                        accepted != null ? "true" : "false",
                        accepted != null ? plan.lookup.state : "NotRead",
                        accepted != null ? plan.lookup.timing : "NotRead",
                        accepted != null ? plan.lookup.rows.size() : 0,
                        accepted != null ? plan.lookup.hash : "NotRead",
                        accepted != null ? plan.lookup.selectedAlias : "-",
                        committedEdges, committedFlags);
            }
            if (!dirtyCursor.equals(run.dirtyAfter)) {
                throw new IllegalStateException("per-callback dirty prefix did not reach final flags");
            }
            for (BLinkerArenaEntry entry : run.arenaAfter.entries) {
                final Boolean linked = linkedCursor.get(entry.identity);
                if (linked == null || linked != entry.linked) {
                    throw new IllegalStateException("serial B-linker cursor did not reach final state");
                }
            }
            final String groupStateHashAfter = beamGroupStateHash(
                    pre.baseRun.beam, run.graphAfter);
            int abnormalMutationCount = 0;
            for (AcceptedSibling accepted : run.accepted) {
                if (accepted.plan.beamBefore.abnormal != accepted.beamAfter.abnormal) {
                    abnormalMutationCount++;
                }
            }
            if ((abnormalMutationCount == 0)
                    != run.groupStateHashBefore.equals(groupStateHashAfter)) {
                throw new IllegalStateException(
                        "group member-state hash does not track abnormal mutations");
            }
            System.out.printf(
                    "stemsbeamvlinksiblinglinksresult %s terminal ReadyBeforeHeadRelationLoop "
                            + "applyReturn %s supportGrade %s stemAlias %s stemInterId %d "
                            + "siblings %d committedEdges %d committedEdgeAliases %s "
                            + "committedFlags %d committedBCells %s eventCount %d "
                            + "groupStateHashAfter %s headRelationLoopRead false%n",
                    prefix, boolToken(pre.baseRun.applyReturned), hex(run.supportGrade),
                    pre.stemAlias, pre.stem.getId(), run.siblings.size(), committedEdges,
                    list(committedEdgeAliases), committedFlags, list(committedBCells),
                    eventOrdinal, groupStateHashAfter);
            System.out.printf(
                    "stemsbeamvlinksiblinglinksdeltaguard %s allocatorBefore %d allocatorAfter %d "
                            + "glyphActiveHashBefore %s glyphActiveHashAfter %s "
                            + "glyphOriginalsHashBefore %s glyphOriginalsHashAfter %s "
                            + "interIndexCountBefore %d interIndexCountAfter %d "
                            + "sigVertexCountBefore %d sigVertexCountAfter %d "
                            + "sigEdgeCountBefore %d sigEdgeCountAfter %d "
                            + "systemStemsHashBefore %s systemStemsHashAfter %s "
                            + "lineHashBefore %s lineHashAfter %s "
                            + "arenaTopologyHashBefore %s arenaTopologyHashAfter %s "
                            + "builderItemsHashBefore %s builderItemsHashAfter %s "
                            + "relationInputHashBefore %s relationInputHashAfter %s "
                            + "groupStateHashBefore %s groupStateHashAfter %s "
                            + "allowedMutations FreshSiblingBeamStemEdgesBeamAbnormalDirtySelectedBCells "
                            + "zeroChordStem true baseBeamStemUnchanged true "
                            + "unrelatedRelationsUnchanged true unrelatedLinkerFlagsUnchanged true "
                            + "headRelationLoopRead false stopBeforeHeadRelationLoop true%n",
                    prefix, run.persistentBefore.allocator, run.persistentAfter.allocator,
                    run.persistentBefore.glyphs.activeHash, run.persistentAfter.glyphs.activeHash,
                    run.persistentBefore.glyphs.originalsHash,
                    run.persistentAfter.glyphs.originalsHash,
                    run.persistentBefore.inters.identities.size(),
                    run.persistentAfter.inters.identities.size(),
                    run.graphBefore.vertices.size(), run.graphAfter.vertices.size(),
                    run.graphBefore.edges.size(), run.graphAfter.edges.size(),
                    run.persistentBefore.systemStems.hash,
                    run.persistentAfter.systemStems.hash,
                    run.persistentBefore.lines.hash, run.persistentAfter.lines.hash,
                    run.arenaBefore.topologyHash, run.arenaAfter.topologyHash,
                    run.builderItemsHash, run.builderItemsHash,
                    run.relationInputHashBefore, run.relationInputHashAfter,
                    run.groupStateHashBefore, groupStateHashAfter);
            System.out.printf(
                    "stemsbeamvlinksiblinglinkssummary %s groupRows %d siblings %d "
                            + "sameGlyph %d existingBeamStem %d shorterWrongSide %d linked %d "
                            + "edgesAdded %d linkerWrites %d events %d chordStemMatches 0 "
                            + "terminal ReadyBeforeHeadRelationLoop%n",
                    prefix, run.groupRows.size(), run.siblings.size(),
                    branchCount(run.siblings, "SameGlyph"),
                    branchCount(run.siblings, "ExistingBeamStem"),
                    branchCount(run.siblings, "ShorterWrongSide"),
                    branchCount(run.siblings, "Linked"), run.accepted.size(), committedFlags,
                    eventOrdinal);
        }

        /**
         * Emit source-executed branch evidence without pretending that an isolated graph is the
         * typed Boundary-15 production state. Every case owns a fresh Book/Sheet/System/SIG and the
         * enclosing real sheet is proved byte-for-byte unchanged by the existing snapshots.
         */
        void emitIsolatedSiblingLinksCoverage (SiblingLinksRun realRun)
            throws Exception
        {
            final PersistentSnapshot enclosingBefore = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final List<SyntheticSiblingSpec> specs = List.of(
                    new SyntheticSiblingSpec("SameGlyph", "Beam", true, false, false,
                            true, false, false, false),
                    new SyntheticSiblingSpec("ExistingBeamStem", "Beam", false, true, false,
                            true, false, false, false),
                    new SyntheticSiblingSpec("ShorterWrongSide", "Beam", false, false, true,
                            true, false, false, false),
                    new SyntheticSiblingSpec("LinkedBeam", "Beam", false, false, false,
                            true, false, false, false),
                    new SyntheticSiblingSpec("LinkedSmallBeam", "SmallBeam", false, false, false,
                            true, false, false, false),
                    new SyntheticSiblingSpec("LinkedHook", "Hook", false, false, false,
                            true, false, false, false),
                    new SyntheticSiblingSpec("LinkedNoBLinker", "Beam", false, false, false,
                            false, false, false, false),
                    new SyntheticSiblingSpec("LinkedIdempotentBCell", "Beam", false, false,
                            false, true, true, false, false),
                    new SyntheticSiblingSpec("ThrowBeforeInsertion", "Beam", false, false,
                            false, true, false, true, false),
                    new SyntheticSiblingSpec("ThrowDuringCallback", "Beam", false, false,
                            false, true, false, false, true));
            for (SyntheticSiblingSpec spec : specs) {
                emitOneIsolatedSiblingLinksCase(realRun, spec);
            }
            final PersistentSnapshot enclosingAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            enclosingBefore.assertSame(enclosingAfter);
        }

        void emitOneIsolatedSiblingLinksCase (SiblingLinksRun realRun,
                                              SyntheticSiblingSpec spec)
            throws Exception
        {
            final IsolatedSiblingFixture fixture = newIsolatedSiblingFixture(
                    spec, realRun.supportGrade);
            final SIGraph sig = fixture.sig;
            if (spec.throwBeforeInsertion) {
                fixture.stem.remove(false);
                if (sig.containsVertex(fixture.stem)) {
                    throw new IllegalStateException("isolated missing-endpoint fault setup drift");
                }
                clearSyntheticDirty(fixture.book(), fixture.stub());
            }
            final GraphOrder graphBefore = new GraphOrder(sig);
            final DirtyFlags dirtyBefore = DirtyFlags.capture(fixture.sheet);
            final BaseStemState stemBefore = new BaseStemState(fixture.stem);
            final BeamObjectState baseBefore = new BeamObjectState(fixture.base);
            final BeamObjectState siblingBefore = new BeamObjectState(fixture.sibling);
            final String groupObjectBefore = beamGroupObjectStateHash(fixture.base);
            final boolean siblingLinkedBefore = fixture.siblingB.isLinked();
            final boolean siblingClosedBefore = fixture.siblingB.isClosed();
            final List<?> builderItemsBefore = new ArrayList<>(
                    (List<?>) STEM_BUILDER_ITEMS.get(fixture.builder));
            final List<AbstractBeamInter> selectedBefore = fixture.baseOuter.getSiblingBeamsAt(
                    (Point2D) B_REF_PT.get(fixture.baseB));
            final boolean expectedSourceOrder = selectedBefore.size() == 2
                    && (spec.shorterWrongSide
                            ? selectedBefore.get(0) == fixture.sibling
                                    && selectedBefore.get(1) == fixture.base
                            : selectedBefore.get(0) == fixture.base
                                    && selectedBefore.get(1) == fixture.sibling);
            if (!expectedSourceOrder) {
                throw new IllegalStateException(
                        "isolated sibling was not source-reachable: " + spec.name
                                + " selected=" + selectedBefore);
            }
            final Line2D stemMedian = fixture.stem.getMedian();
            final Line2D baseMedian = fixture.base.getMedian();
            final Line2D siblingMedian = fixture.sibling.getMedian();
            final Point2D baseCross = LineUtil.intersection(stemMedian, baseMedian);
            final Point2D siblingCross = LineUtil.intersection(stemMedian, siblingMedian);
            final double baseLength = baseMedian.getX2() - baseMedian.getX1();
            final double siblingLength = siblingMedian.getX2() - siblingMedian.getX1();
            final double ratio = siblingLength / baseLength;
            final boolean shorterRead = !spec.sameGlyph && !spec.existingBeamStem;
            final boolean shorter = shorterRead && ratio <= maxShorterRatio();
            final double dy = shorter ? siblingCross.getY() - baseCross.getY() : Double.NaN;
            final double product = shorter ? dy : Double.NaN; // yDir is exactly +1.
            final boolean wrongSide = shorter && product < 0;
            final String expectedBranch = spec.sameGlyph ? "SameGlyph"
                    : spec.existingBeamStem ? "ExistingBeamStem"
                            : wrongSide ? "ShorterWrongSide" : "Linked";
            final Point2D extension = expectedBranch.equals("Linked")
                    ? new Point2D.Double(
                            siblingCross.getX(), siblingCross.getY() - fixture.sibling.getHeight() / 2.0)
                    : null;
            final BeamPortion portion = expectedBranch.equals("Linked")
                    ? BeamStemRelation.computeBeamPortion(
                            fixture.sibling, siblingCross.getX(), fixture.sheet.getScale())
                    : null;

            if (spec.throwDuringCallback) {
                if (!ListenerTopology.capture(sig).isStandard()) {
                    throw new IllegalStateException("isolated throw graph lacks standard listener");
                }
                sig.addGraphListener(new ThrowingGraphListener(ThrowEvent.EDGE_ADDED));
                final List<?> listeners = (List<?>) get(GRAPH_LISTENERS, sig);
                if (listeners.size() != 2
                        || !(listeners.get(1) instanceof ThrowingGraphListener)) {
                    throw new IllegalStateException("later-listener envelope order drift");
                }
            }

            Throwable thrown = null;
            try {
                V_LINK_SIBLINGS.invoke(fixture.currentV, fixture.stem, realRun.supportGrade);
            } catch (InvocationTargetException ex) {
                thrown = ex.getCause();
            }
            final GraphOrder graphAfter = new GraphOrder(sig);
            final long chordStemBefore = graphBefore.edges.stream()
                    .filter(ChordStemRelation.class::isInstance).count();
            final long chordStemAfter = graphAfter.edges.stream()
                    .filter(ChordStemRelation.class::isInstance).count();
            BeamStemRelation fresh = null;
            for (Relation relation : graphAfter.edges) {
                if (!graphBefore.edgeOrdinals.containsKey(relation)
                        && relation instanceof BeamStemRelation beamStem
                        && sig.getEdgeSource(relation) == fixture.sibling
                        && sig.getEdgeTarget(relation) == fixture.stem) {
                    if (fresh != null) {
                        throw new IllegalStateException("multiple isolated fresh sibling relations");
                    }
                    fresh = beamStem;
                }
            }
            final boolean callbackCompleted = fresh != null
                    && (!spec.throwDuringCallback || thrown instanceof SyntheticListenerException);
            final StemIncidentScan stemIncidents = callbackCompleted
                    ? StemIncidentScan.capture(sig, fixture.stem, graphAfter)
                    : StemIncidentScan.notRead();
            final BeamIncidentTrace beamIncidents = callbackCompleted
                    ? BeamIncidentTrace.capture(sig, fixture.sibling, graphAfter)
                    : BeamIncidentTrace.notRead();
            final DirtyFlags dirtyAfter = DirtyFlags.capture(fixture.sheet);
            final boolean siblingLinkedAfter = fixture.siblingB.isLinked();
            final boolean siblingClosedAfter = fixture.siblingB.isClosed();
            final List<?> builderItemsAfter = (List<?>) STEM_BUILDER_ITEMS.get(fixture.builder);
            final BeamObjectState baseAfter = new BeamObjectState(fixture.base);
            final BeamObjectState siblingAfter = new BeamObjectState(fixture.sibling);
            final BaseStemState stemAfter = new BaseStemState(fixture.stem);
            assertIdentityList("isolated StemBuilder.items", builderItemsBefore, builderItemsAfter);

            final boolean supported = !spec.throwBeforeInsertion && !spec.throwDuringCallback;
            final boolean shouldAdd = expectedBranch.equals("Linked") && !spec.throwBeforeInsertion;
            final boolean shouldWrite = shouldAdd && !spec.throwDuringCallback && spec.includeLinker;
            if ((fresh != null) != shouldAdd || (thrown == null) != supported
                    || siblingLinkedAfter != (siblingLinkedBefore || shouldWrite)
                    || siblingClosedBefore != siblingClosedAfter
                    || !baseBefore.same(baseAfter)
                    || !stemBefore.same(stemAfter)
                    || !siblingBefore.sameExceptAbnormal(siblingAfter)
                    || chordStemBefore != 0 || chordStemAfter != 0
                    || !groupObjectBefore.equals(beamGroupObjectStateHash(fixture.base))) {
                throw new IllegalStateException("isolated sibling-links branch/state drift: " + spec.name);
            }
            if (fresh != null) {
                requirePointBits(extension, fresh.getExtensionPoint(), "isolated sibling extension");
                if (fresh.getBeamPortion() != portion) {
                    throw new IllegalStateException("isolated sibling portion drift");
                }
                requireBits(realRun.supportGrade, fresh.getGrade(), "isolated sibling grade");
                if (stemIncidents.chordMatches != 0) {
                    throw new IllegalStateException("isolated supported callback has ChordStem match");
                }
            }
            if (spec.throwBeforeInsertion && !(thrown instanceof IllegalArgumentException)) {
                throw new IllegalStateException("missing endpoint did not throw before insertion");
            }
            if (spec.throwDuringCallback && !(thrown instanceof SyntheticListenerException)) {
                throw new IllegalStateException("later listener envelope did not throw");
            }

            final String scope = supported ? "synthetic" : "envelope";
            final String prefix = page + " system 1 plan " + realRun.predecessor.ref.plan
                    + " scope " + scope + " case " + spec.name;
            final String pairState = spec.sameGlyph ? "NotReadSameGlyph"
                    : spec.existingBeamStem ? "ExistingBeamStem" : "Absent";
            final String lookupState = !expectedBranch.equals("Linked")
                    || spec.throwBeforeInsertion || spec.throwDuringCallback ? "NotRead"
                            : spec.includeLinker ? "FirstSourceIdentityMatch" : "ExhaustiveNoMatch";
            final String throwStage = spec.throwBeforeInsertion ? "AddEdgeBeforeInsertion"
                    : spec.throwDuringCallback ? "LaterListenerAfterRelationCallback" : "-";
            final String terminal = supported ? "ReadyBeforeHeadRelationLoop"
                    : "Threw:" + throwStage;
            final String freshIdentity = fresh != null ? graphAfter.edgeAlias(fresh) : "-";
            final int eventCount = (fresh != null ? 2 : 0)
                    + (shouldWrite ? 1 : 0) + (thrown != null ? 1 : 0);
            System.out.printf(
                    "stemsbeamvlinksiblinglinkssyntheticcase %s join IsolatedBoundary15Replay "
                            + "sourceRealB15EvidenceSha256 %s construction RealBookSheetSystemSIG "
                            + "baseRuntimeClass %s siblingRuntimeClass %s stemAttachedBefore %s "
                            + "baseMedian %s siblingMedian %s stemMedian %s yDir 1 "
                            + "sameGlyphIdentity %s pairState %s baseCross %s siblingCross %s "
                            + "baseLength %s siblingLength %s ratioRead %s ratio %s "
                            + "maxShorterRatio %s shorterInclusive %s dyRead %s dy %s "
                            + "product %s wrongSideStrict %s extension %s portion %s "
                            + "supportGrade %s branch %s builderItems %d lookupState %s "
                            + "siblingBLinkedBefore %s siblingBLinkedAfter %s writeCount %d "
                            + "valueChangeCount %d graphEdgesBefore %d graphEdgesAfter %d "
                            + "freshRelationIdentity %s callbackCompleted %s "
                            + "stemIncidentState %s stemIncidentRows %d chordStemMatches %d "
                            + "beamIncidentState %s beamRule %s beamAbnormalBefore %s "
                            + "beamAbnormalAfter %s dirtyBefore %s dirtyAfter %s "
                            + "throwClass %s throwStage %s eventCount %d terminal %s%n",
                    prefix, expected.bLinkerFlag.orderedRowsSha256,
                    fixture.base.getClass().getName(), fixture.sibling.getClass().getName(),
                    graphBefore.vertexOrdinals.containsKey(fixture.stem), line(baseMedian),
                    line(siblingMedian), line(stemMedian), spec.sameGlyph, pairState,
                    point(baseCross), point(siblingCross), hex(baseLength), hex(siblingLength),
                    shorterRead, shorterRead ? hex(ratio) : "-", hex(maxShorterRatio()),
                    shorterRead ? Boolean.toString(shorter) : "-", shorter ? "true" : "false",
                    shorter ? hex(dy) : "-", shorter ? hex(product) : "-",
                    shorter ? Boolean.toString(wrongSide) : "-",
                    extension != null ? point(extension) : "-",
                    portion != null ? portion.toString() : "-", hex(realRun.supportGrade),
                    expectedBranch, fixture.builderItems, lookupState, siblingLinkedBefore,
                    siblingLinkedAfter, shouldWrite ? 1 : 0,
                    shouldWrite && !siblingLinkedBefore ? 1 : 0,
                    graphBefore.edges.size(), graphAfter.edges.size(), freshIdentity,
                    callbackCompleted, stemIncidents.state, stemIncidents.rows.size(),
                    stemIncidents.chordMatches, beamIncidents.state, beamIncidents.rule,
                    siblingBefore.abnormal, siblingAfter.abnormal, dirtyBefore.token(),
                    dirtyAfter.token(), thrown != null ? thrown.getClass().getName() : "-",
                    throwStage, eventCount, terminal);

            int eventOrdinal = 0;
            if (fresh != null) {
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssyntheticevent %s eventOrdinal %d "
                                + "kind SigEdgeInserted relationIdentity %s%n",
                        prefix, eventOrdinal++, freshIdentity);
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssyntheticevent %s eventOrdinal %d "
                                + "kind RelationCallbackCompleted relationIdentity %s%n",
                        prefix, eventOrdinal++, freshIdentity);
            }
            if (shouldWrite) {
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssyntheticevent %s eventOrdinal %d "
                                + "kind BLinkerLinkedAssigned relationIdentity -%n",
                        prefix, eventOrdinal++);
            }
            if (thrown != null) {
                System.out.printf(
                        "stemsbeamvlinksiblinglinkssyntheticevent %s eventOrdinal %d "
                                + "kind Throw relationIdentity %s%n",
                        prefix, eventOrdinal++, freshIdentity);
            }
            if (eventOrdinal != eventCount) {
                throw new IllegalStateException("isolated event accounting drift");
            }
            System.out.printf(
                    "stemsbeamvlinksiblinglinkssyntheticguard %s graphDelta %d "
                            + "allowedMutations FreshSiblingBeamStemBeamAbnormalDirtySelectedBCell "
                            + "baseBeamUnchanged true stemGeometryUnchanged true "
                            + "groupObjectUnchanged true zeroChordStem %s isolatedOnly true "
                            + "productionEquivalent false enclosingRealSheetUnchanged true "
                            + "headRelationLoopRead false terminal %s%n",
                    prefix, graphAfter.edges.size() - graphBefore.edges.size(),
                    chordStemBefore == 0 && chordStemAfter == 0, terminal);
        }

        IsolatedSiblingFixture newIsolatedSiblingFixture (SyntheticSiblingSpec spec,
                                                          double supportGrade)
            throws Exception
        {
            final Book book = new Book(Path.of(
                    "/private/tmp",
                    "stems-beam-vlink-sibling-links-" + spec.name + ".synthetic"));
            final SheetStub stub = new SheetStub(book, 1);
            book.addStub(stub);
            final Sheet isolatedSheet = new Sheet(stub, (RunTable) null);
            isolatedSheet.setScale(sheet.getScale());
            // The source path reads Sheet.getSkew().skewedVertical(). A null-picture synthetic
            // sheet cannot initialize the image-sized Skew transform, but the no-arg zero-slope
            // value is complete for skewedVertical(), which reads only the persisted slope.
            isolatedSheet.setSkew(new Skew());
            final int interline = isolatedSheet.getScale().getInterline();
            final List<LineInfo> staffLines = new ArrayList<>();
            for (int index = 0; index < 5; index++) {
                final double y = 100.0 + (index * interline);
                staffLines.add(new StaffLine(
                        List.of(new Point2D.Double(0.0, y), new Point2D.Double(100.0, y)),
                        1.0));
            }
            final Staff syntheticStaff = new Staff(1, 0.0, 100.0, interline, staffLines);
            final SystemInfo isolatedSystem = new SystemInfo(
                    1, isolatedSheet, List.of(syntheticStaff));
            isolatedSheet.getSystemManager().setSystems(List.of(isolatedSystem));
            isolatedSheet.getStaffManager().addStaff(syntheticStaff);
            final SIGraph sig = isolatedSystem.getSig();
            if (!ListenerTopology.capture(sig).isStandard()) {
                throw new IllegalStateException("isolated sibling SIG lacks sole SigListener");
            }
            final BeamGroupInter group = new BeamGroupInter();
            final AbstractBeamInter base = new BeamInter(
                    null, new Line2D.Double(0.0, 10.0, 10.0, 10.0), 4.0);
            final double siblingY = spec.shorterWrongSide ? 5.0 : 15.0;
            final double siblingX1 = spec.shorterWrongSide ? 1.0 : 0.0;
            final double siblingX2 = spec.shorterWrongSide ? 9.0 : 10.0;
            final Line2D siblingMedian = new Line2D.Double(
                    siblingX1, siblingY, siblingX2, siblingY);
            final AbstractBeamInter sibling = switch (spec.runtime) {
            case "Beam" -> new BeamInter(null, siblingMedian, 4.0);
            case "SmallBeam" -> new SmallBeamInter(null, siblingMedian, 4.0);
            case "Hook" -> new BeamHookInter(null, siblingMedian, 2.0);
            default -> throw new IllegalArgumentException("unknown isolated beam runtime");
            };
            final Glyph baseGlyph = new Glyph(0, 0, (RunTable) null);
            final Glyph siblingGlyph = spec.sameGlyph
                    ? baseGlyph : new Glyph(1, 0, (RunTable) null);
            base.setGlyph(baseGlyph);
            sibling.setGlyph(siblingGlyph);
            final StemInter stem = new StemInter(null, 0.8);
            stem.setWidth((double) isolatedSheet.getScale().getStemThickness());
            stem.setMedian(new Point2D.Double(5.0, -100.0), new Point2D.Double(5.0, 100.0));
            if (!sig.addVertex(group) || !sig.addVertex(base) || !sig.addVertex(sibling)
                    || !sig.addVertex(stem)) {
                throw new IllegalStateException("isolated sibling vertex setup drift");
            }
            group.addMember(base);
            group.addMember(sibling);
            final BeamStemRelation baseRelation = new BeamStemRelation();
            baseRelation.setExtensionPoint(new Point2D.Double(5.0, 8.0));
            baseRelation.setBeamPortion(BeamPortion.CENTER);
            baseRelation.setGrade(supportGrade);
            if (!sig.addEdge(base, stem, baseRelation)) {
                throw new IllegalStateException("isolated B15 base relation setup drift");
            }
            if (spec.existingBeamStem) {
                final BeamStemRelation existing = new BeamStemRelation();
                existing.setExtensionPoint(new Point2D.Double(5.0, siblingY - 2.0));
                existing.setBeamPortion(BeamPortion.CENTER);
                existing.setGrade(supportGrade);
                if (!sig.addEdge(sibling, stem, existing)) {
                    throw new IllegalStateException("isolated existing sibling relation setup drift");
                }
            }

            final StemsRetriever isolatedRetriever = new StemsRetriever(isolatedSystem);
            final Object isolatedParams = PARAMETERS_CONSTRUCTOR.newInstance(
                    isolatedSystem, isolatedSheet.getScale());
            RETRIEVER_PARAMS.set(isolatedRetriever, isolatedParams);
            final BeamLinker baseOuter = newIsolatedBeamLinker(
                    base, group, isolatedRetriever, isolatedParams);
            final BeamLinker.BLinker baseB = baseOuter.new BLinker(
                    null, null, new Point2D.Double(5.0, 10.0), false);
            final Object currentV = UNSAFE_ALLOCATE_INSTANCE.invoke(UNSAFE, V_LINKER_CLASS);
            V_OUTER_B.set(currentV, baseB);
            V_V_SIDE.set(currentV, VerticalSide.BOTTOM);
            V_Y_DIR.setInt(currentV, 1);
            V_THEO_LINE.set(currentV, stem.getMedian());
            ((Map<VerticalSide, Object>) B_V_LINKERS.get(baseB)).put(
                    VerticalSide.BOTTOM, currentV);
            baseB.setLinked(true); // Exact Boundary-15 post-state for the isolated cell.

            final BeamLinker siblingOuter = newIsolatedBeamLinker(
                    sibling, group, isolatedRetriever, isolatedParams);
            final BeamLinker.BLinker siblingB = siblingOuter.new BLinker(
                    null, null, new Point2D.Double(5.0, siblingY), false);
            siblingB.setLinked(spec.initiallyLinked);
            final List<StemItem> items = new ArrayList<>();
            if (spec.includeLinker) items.add(new StemItem.LinkerItem(siblingB));
            final StemBuilder builder = (StemBuilder) UNSAFE_ALLOCATE_INSTANCE.invoke(
                    UNSAFE, StemBuilder.class);
            STEM_BUILDER_ITEMS.set(builder, items);
            V_STEM_BUILDER.set(currentV, builder);
            clearSyntheticDirty(book, stub);
            return new IsolatedSiblingFixture(
                    book, stub, isolatedSheet, isolatedSystem, sig, group, base, sibling, stem,
                    baseOuter, baseB, currentV, siblingOuter, siblingB, builder, items.size());
        }

        BeamLinker newIsolatedBeamLinker (AbstractBeamInter beam,
                                          BeamGroupInter group,
                                          StemsRetriever isolatedRetriever,
                                          Object isolatedParams)
            throws Exception
        {
            final BeamLinker outer = (BeamLinker) UNSAFE_ALLOCATE_INSTANCE.invoke(
                    UNSAFE, BeamLinker.class);
            LINKER_BEAM.set(outer, beam);
            LINKER_BEAM_GROUP.set(outer, group);
            LINKER_MEDIAN.set(outer, beam.getMedian());
            LINKER_RETRIEVER.set(outer, isolatedRetriever);
            LINKER_SYSTEM.set(outer, beam.getSig().getSystem());
            LINKER_SCALE.set(outer, beam.getSig().getSystem().getSheet().getScale());
            LINKER_PARAMS.set(outer, isolatedParams);
            LINKER_ALL_B.set(outer, new ArrayList<>());
            LINKER_SIDE_B.set(outer, new EnumMap<HorizontalSide, Object>(HorizontalSide.class));
            LINKER_STUMP_V.set(outer, new ArrayList<>());
            beam.setLinker(outer);
            return outer;
        }

        String relationObjectIdentity (SiblingLinksRun run,
                                       Relation relation)
        {
            for (AcceptedSibling accepted : run.accepted) {
                if (accepted.relation == relation) {
                    return "sibling-draft:" + run.predecessor.ref.plan + ":"
                            + accepted.plan.siblingOrdinal;
                }
            }
            if (run.predecessor.baseRun.link.relation == relation) {
                return "base-draft:" + run.predecessor.ref.plan;
            }
            final Integer ordinal = run.graphBefore.edgeOrdinals.get(relation);
            return ordinal != null ? "sig-relation-object:" + ordinal : "-";
        }

        String groupQueryHash (List<GroupScanRow> rows)
        {
            final List<String> tokens = new ArrayList<>();
            for (GroupScanRow row : rows) {
                tokens.add(row.outgoingOrdinal + ":" + row.graphRelationIdentity + ":"
                        + row.relationObjectIdentity + ":" + row.runtimeClass + ":"
                        + row.targetAlias + ":" + row.targetRuntimeClass + ":"
                        + row.target.getId() + ":" + row.targetVertexOrdinal + ":"
                        + row.containment + ":" + row.memberOrdinal);
            }
            return sha256Rows(tokens);
        }

        String siblingStemIncidentHash (SiblingLinksRun run,
                                         StemIncidentScan scan)
        {
            final List<String> tokens = new ArrayList<>();
            for (IncidentRow row : scan.rows) {
                tokens.add(row.incidentOrdinal + ":" + row.direction + ":"
                        + row.directionOrdinal + ":" + row.globalIdentity + ":"
                        + relationObjectIdentity(run, row.relation) + ":"
                        + row.relation.getClass().getName() + ":"
                        + siblingInterAlias(run, row.opposite)
                        + ":" + row.opposite.getId() + ":" + row.oppositeVertexOrdinal + ":"
                        + row.chordStemMatch);
            }
            return sha256Rows(tokens);
        }

        String siblingBeamIncidentHash (SiblingLinksRun run,
                                         BeamIncidentTrace scan)
        {
            final List<String> tokens = new ArrayList<>();
            for (BeamIncidentRow row : scan.rows) {
                tokens.add(row.incidentOrdinal + ":" + row.direction + ":"
                        + row.directionOrdinal + ":" + row.globalIdentity + ":"
                        + relationObjectIdentity(run, row.relation) + ":"
                        + row.relation.getClass().getName() + ":"
                        + siblingInterAlias(run, row.opposite)
                        + ":" + row.opposite.getId() + ":" + row.oppositeVertexOrdinal + ":"
                        + row.readState + ":" + row.relevant + ":" + row.portion + ":"
                        + row.contribution);
            }
            return sha256Rows(tokens);
        }

        List<String> observerAliases (BLinkerArenaEntry entry)
        {
            final List<String> aliases = new ArrayList<>();
            aliases.add(entry.alias);
            aliases.addAll(entry.childAliases);
            return aliases;
        }

        String siblingInterAlias (SiblingLinksRun run,
                                  Inter inter)
        {
            return inter == run.predecessor.stem
                    ? run.predecessor.stemAlias
                    : inter instanceof BeamGroupInter ? run.groupAlias : interAlias(inter);
        }

        BLinkerArenaEntry arenaEntry (BLinkerArenaSnapshot arena,
                                      Object identity)
        {
            for (BLinkerArenaEntry entry : arena.entries) {
                if (entry.identity == identity) return entry;
            }
            throw new IllegalStateException("B arena entry missing");
        }

        String glyphIdentityFromRows (Glyph glyph,
                                      List<GroupScanRow> rows)
        {
            final String token = nullableGlyphToken(glyph);
            for (GroupScanRow row : rows) {
                if ("true".equals(row.baseIdentity)) {
                    if (!row.glyph.equals(token)) {
                        throw new IllegalStateException("base glyph payload drift");
                    }
                    return row.glyphIdentity;
                }
            }
            throw new IllegalStateException("base glyph missing from group scan");
        }

        static int branchCount (List<SiblingPlan> plans,
                                String branch)
        {
            int count = 0;
            for (SiblingPlan plan : plans) if (plan.branch.equals(branch)) count++;
            return count;
        }

        static int identityOrdinal (Collection<Relation> relations,
                                    Relation target)
        {
            int ordinal = 0;
            for (Relation relation : relations) {
                if (relation == target) return ordinal;
                ordinal++;
            }
            throw new IllegalStateException("relation absent from incidence collection");
        }

        void assertBaseApplyExpected (ApplyRun run,
                                      int plan,
                                      String finalAlias,
                                      StemInter finalStem)
        {
            final Map<String, String> result = expected.baseResult;
            final Map<String, String> guard = expected.baseGuard;
            final Map<String, String> summary = expected.baseSummary;
            assertField(result, "plan", Integer.toString(plan));
            assertField(result, "terminal", "ReadyBeforeBLinkerFlagMutation");
            assertField(result, "applyReturn", boolToken(run.applyReturned));
            assertField(result, "retainedDraftGrade", hex(((Support) run.link.relation).getGrade()));
            assertField(result, "graphRelationIdentity", run.after.graph.edgeAlias(run.link.relation));
            assertField(result, "finalStemAlias", finalAlias);
            assertField(result, "finalStemInterId", Integer.toString(finalStem.getId()));
            assertField(result, "allocator", Integer.toString(run.after.allocator));
            assertField(result, "interIndexHash", run.after.inters.hash);
            assertField(result, "sigVertexHash", run.after.graph.vertexHash);
            assertField(result, "sigEdgeHash", run.after.graph.edgeHash);
            assertField(result, "stubModified", Boolean.toString(run.after.dirty.stubModified));
            assertField(result, "bookModifiedRaw", Boolean.toString(run.after.dirty.bookModified));
            assertField(result, "bookDirty", Boolean.toString(run.after.dirty.bookDirty));
            assertField(guard, "linkerFlagsUnchanged", "true");
            assertField(guard, "stopBeforeBLinkerFlagMutation", "true");
            assertField(guard, "stopBeforeSiblingBeamLinks", "true");
            assertField(guard, "stopBeforeHeadRelationLoop", "true");
            assertField(summary, "applyReturned", boolToken(run.applyReturned));
            assertField(summary, "edgeAdded", Boolean.toString(
                    run.after.graph.edgeOrdinals.containsKey(run.link.relation)));
            assertField(summary, "chordMatches", Integer.toString(run.stemIncidents.chordMatches));
            assertField(summary, "terminal", "ReadyBeforeBLinkerFlagMutation");
        }

        BLinkerFlagRun executeBLinkerFlag (String scope,
                                          String caseName,
                                          Object expectedB,
                                          Object currentV,
                                          PlanRef ref,
                                          ApplyRun baseRun,
                                          String stemAlias,
                                          StemInter stem,
                                          PersistentSnapshot persistentBefore)
            throws Exception
        {
            if (expectedB.getClass() != B_LINKER_CLASS
                    || currentV.getClass() != V_LINKER_CLASS
                    || B_LINKED.getDeclaringClass() != B_LINKER_CLASS
                    || B_LINKED.getType() != boolean.class
                    || java.lang.reflect.Modifier.isVolatile(B_LINKED.getModifiers())
                    || B_LINKER_CLASS.getMethod("setLinked", boolean.class)
                            .getDeclaringClass() != B_LINKER_CLASS) {
                throw new IllegalStateException("B-linker setter is not exact concrete dispatch");
            }
            if (V_GET_B_LINKER.invoke(currentV) != expectedB) {
                throw new IllegalStateException("VLinker.getBLinker lexical parent drift");
            }
            final BLinkerArenaSnapshot before = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, expectedB);
            final String bAlias = bAliases.get(expectedB);
            if (bAlias == null || !bAlias.equals(ref.bAlias)
                    || V_V_SIDE.get(currentV) != ref.vSide) {
                throw new IllegalStateException("target B/V reference drift");
            }
            final String vAlias = bAlias + ":v:" + V_V_SIDE.get(currentV);
            final List<BLinkerObservation> observationsBefore = observations(
                    expectedB, currentV, bAlias);
            final boolean linkedBefore = ((StemLinker) expectedB).isLinked();
            final boolean closedBefore = ((StemLinker) expectedB).isClosed();
            boolean assignmentAttempted = false;
            boolean assignmentCompleted = false;
            assignmentAttempted = true;
            // This one expression is the exact source seam. Lexical-parent identity was certified
            // above, before the pre-state observations, so nothing is inserted between its getter
            // and concrete setter invocation.
            ((BeamLinker.BLinker) V_GET_B_LINKER.invoke(currentV)).setLinked(true);
            assignmentCompleted = true;
            final boolean linkedAfter = ((StemLinker) expectedB).isLinked();
            final boolean closedAfter = ((StemLinker) expectedB).isClosed();
            final BLinkerArenaSnapshot after = BLinkerArenaSnapshot.capture(
                    inspectionBeams, beamSigOrdinals, bAliases, expectedB);
            final List<BLinkerObservation> observationsAfter = observations(
                    expectedB, currentV, bAlias);
            final ApplyMoment baseStateAfterFlag = new ApplyMoment(
                    baseRun.beam.getSig().getSystem().getSheet(), baseRun.beam.getSig(),
                    baseRun.beam, baseRun.stem);
            before.assertOnlyTargetCellChanged(after, expectedB);
            assertObservationTransition(observationsBefore, observationsAfter, linkedBefore);
            if (!baseRun.after.same(baseStateAfterFlag)) {
                throw new IllegalStateException("B-linker flag mutated boundary-14 state");
            }
            if (!assignmentAttempted || !assignmentCompleted || !linkedAfter
                    || closedBefore != closedAfter) {
                throw new IllegalStateException("B-linker assignment trace drift");
            }
            final BaseEvidence evidence = BaseEvidence.of(
                    "-", new BaseApplyRows(
                            expected.baseFrontier, expected.baseResult,
                            expected.baseGuard, expected.baseSummary));
            return new BLinkerFlagRun(
                    scope, caseName, expectedB, currentV, bAlias, vAlias, ref,
                    baseRun, evidence, stemAlias, stem, persistentBefore, before, after,
                    observationsBefore, observationsAfter, linkedBefore, linkedAfter,
                    closedBefore, closedAfter, assignmentAttempted, assignmentCompleted,
                    !linkedBefore);
        }

        List<BLinkerObservation> observations (Object b,
                                              Object currentV,
                                              String bAlias)
            throws Exception
        {
            final List<BLinkerObservation> rows = new ArrayList<>();
            rows.add(new BLinkerObservation(
                    "ParentB", 0, bAlias, "-", false, true,
                    ((StemLinker) b).isLinked(), ((StemLinker) b).isClosed(), true));
            final Map<VerticalSide, Object> children =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            int ordinal = 0;
            int currentMatches = 0;
            for (Map.Entry<VerticalSide, Object> entry : children.entrySet()) {
                final Object child = entry.getValue();
                if (V_GET_B_LINKER.invoke(child) != b) {
                    throw new IllegalStateException("child V lexical parent drift");
                }
                rows.add(new BLinkerObservation(
                        child == currentV ? "CurrentV" : "OtherV", ordinal,
                        bAlias + ":v:" + entry.getKey(), entry.getKey().toString(),
                        child == currentV, true, ((StemLinker) child).isLinked(),
                        ((StemLinker) child).isClosed(), false));
                if (child == currentV) currentMatches++;
                ordinal++;
            }
            if (currentMatches != 1) {
                throw new IllegalStateException("current V is not a unique child of target B");
            }
            return rows;
        }

        void assertObservationTransition (List<BLinkerObservation> before,
                                          List<BLinkerObservation> after,
                                          boolean linkedBefore)
        {
            if (before.size() != after.size() || before.size() < 2) {
                throw new IllegalStateException("incomplete B/V shared-cell observations");
            }
            for (int i = 0; i < before.size(); i++) {
                final BLinkerObservation left = before.get(i);
                final BLinkerObservation right = after.get(i);
                if (!left.sameIdentity(right) || left.linked != linkedBefore
                        || !right.linked || left.closed != right.closed) {
                    throw new IllegalStateException("B/V shared-cell observation drift");
                }
            }
        }

        void assertRealBLinkerFlagDelta (PersistentSnapshot before,
                                         PersistentSnapshot after,
                                         BLinkerFlagRun run)
            throws Exception
        {
            if (before.allocator != after.allocator
                    || !before.glyphs.sameIdentityState(after.glyphs)
                    || !before.inters.same(after.inters) || !before.sig.same(after.sig)
                    || !before.systemStems.same(after.systemStems)
                    || !before.lines.same(after.lines)) {
                throw new IllegalStateException("B-linker flag crossed persistent mutation seam");
            }
            final IdentityHashMap<StemLinker, Boolean> allowed = new IdentityHashMap<>();
            allowed.put((StemLinker) run.b, true);
            final Map<VerticalSide, Object> children =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(run.b);
            for (Object child : children.values()) allowed.put((StemLinker) child, true);
            before.linkers.assertOnlySharedBCellChanged(after.linkers, allowed, run.linkedBefore);
        }

        void emitBLinkerFlagRows (BLinkerFlagRun run)
            throws Exception
        {
            final String prefix = page + " system " + system.getId() + " plan " + run.ref.plan
                    + " scope " + run.scope + " case " + run.caseName;
            System.out.printf(
                    "stemsbeamvlinkblinkerflagpredecessor %s "
                            + "join %s baseCase %s predecessorTerminal %s "
                            + "baseFrontierRowSha256 %s baseResultRowSha256 %s "
                            + "baseGuardRowSha256 %s baseSummaryRowSha256 %s "
                            + "baseEvidenceSha256 %s applyReturn %s supportGrade %s "
                            + "stemAlias %s stemInterId %d bAlias %s vAlias %s%n",
                    prefix, run.scope.equals("real")
                            ? "FullBoundary14Replay" : "IsolatedBoundary14Replay",
                    run.baseEvidence.caseName, run.baseEvidence.terminal,
                    run.baseEvidence.frontierRowSha256, run.baseEvidence.resultRowSha256,
                    run.baseEvidence.guardRowSha256, run.baseEvidence.summaryRowSha256,
                    run.baseEvidence.evidenceSha256,
                    boolToken(run.baseRun.applyReturned),
                    run.baseEvidence.supportGrade,
                    run.stemAlias, run.stem.getId(), run.bAlias, run.vAlias);

            System.out.printf(
                    "stemsbeamvlinkblinkerflagbaseline %s "
                            + "arenaScope %s arenaEntries %d "
                            + "frozenEntries %d dynamicAnchors %d noChild %d child1 %d child2 %d "
                            + "topologyHash %s stateHashBefore %s linkedCountBefore %d "
                            + "linkedAliasesBefore %s linkedSetHashBefore %s closedCountBefore %d "
                            + "closedAliasesBefore %s targetMatches %d targetFrozenMatch %s%n",
                    prefix, run.scope.equals("real")
                            ? "ExhaustiveLiveAllBLinkers" : "ExhaustiveIsolatedAllBLinkers",
                    run.before.entries.size(), run.before.frozenEntries,
                    run.before.dynamicAnchors, run.before.noChild, run.before.child1,
                    run.before.child2, run.before.topologyHash, run.before.stateHash,
                    run.before.linkedAliases.size(), list(run.before.linkedAliases),
                    run.before.linkedSetHash, run.before.closedAliases.size(),
                    list(run.before.closedAliases), run.before.targetMatches,
                    run.before.targetFrozenMatch);

            for (int i = 0; i < run.before.entries.size(); i++) {
                final BLinkerArenaEntry before = run.before.entries.get(i);
                final BLinkerArenaEntry after = run.after.entries.get(i);
                System.out.printf(
                        "stemsbeamvlinkblinkerflagbentry %s arenaOrdinal %d "
                                + "constructorOrdinal %d beamSig %s bOrdinal %d bJavaId %d "
                                + "bAlias %s origin %s runtimeClass %s hSide %s anchor %s "
                                + "stump %s ref %s childCount %d childAliases %s "
                                + "linkedBefore %s linkedAfter %s closedBefore %s closedAfter %s "
                                + "target %s action %s%n",
                        prefix, before.arenaOrdinal, before.constructorOrdinal, before.beamSig,
                        before.bOrdinal, before.javaId, before.alias, before.origin,
                        before.runtimeClass, before.hSide, before.anchor, before.stump,
                        before.ref, before.childAliases.size(), list(before.childAliases),
                        before.linked, after.linked, before.closed, after.closed,
                        before.target, before.target ? "TargetSharedFlagWrite" : "Unchanged");
            }

            final List<String> childAliases = new ArrayList<>();
            for (BLinkerObservation observation : run.observationsBefore) {
                if (!observation.role.equals("ParentB")) childAliases.add(observation.alias);
            }
            System.out.printf(
                    "stemsbeamvlinkblinkerflagtarget %s bAlias %s vAlias %s "
                            + "bRefBeamSig %s bRefJavaId %d vSide %s "
                            + "receiverRuntimeClass %s setterDeclaringClass %s "
                            + "getterDeclaringClass %s getterResolvedAlias %s sameIdentity true "
                            + "sharedCell EnclosingBLinkerLinked sLinkerRead NotRead "
                            + "orderedChildAliases %s linkedBefore %s linkedAfter %s "
                            + "closedBefore %s closedAfter %s%n",
                    prefix, run.bAlias, run.vAlias,
                    run.before.targetEntry().beamSig, run.before.targetEntry().javaId,
                    V_V_SIDE.get(run.currentV), B_LINKER_CLASS.getName(),
                    B_LINKER_CLASS.getMethod("setLinked", boolean.class).getDeclaringClass().getName(),
                    V_GET_B_LINKER.getDeclaringClass().getName(), run.bAlias,
                    list(childAliases), run.linkedBefore, run.linkedAfter,
                    run.closedBefore, run.closedAfter);

            for (int i = 0; i < run.observationsBefore.size(); i++) {
                final BLinkerObservation before = run.observationsBefore.get(i);
                final BLinkerObservation after = run.observationsAfter.get(i);
                System.out.printf(
                        "stemsbeamvlinkblinkerflagobservation %s observationOrdinal %d "
                                + "role %s alias %s vSide %s current %s sharedCell %s "
                                + "sameCell %s ownsLinkedField %s linkedBefore %s linkedAfter %s "
                                + "closedBefore %s closedAfter %s readMethod %s%n",
                        prefix, i, before.role, before.alias, before.vSide, before.current,
                        "bcell:" + run.bAlias, before.sameCell, before.ownsCell,
                        before.linked, after.linked, before.closed, after.closed,
                        before.role.equals("ParentB") ? "BLinker.isLinked" : "VLinker.isLinked");
            }

            System.out.printf(
                    "stemsbeamvlinkblinkerflagwritetrace %s getter getBLinker "
                            + "getterCompleted true receiverAlias %s setter setLinked requested true "
                            + "before %s after %s assignmentAttempted %s assignmentCompleted %s "
                            + "writeCount 1 valueChangeCount %d transition %s return void "
                            + "setterBodyValidationReads 0 setterBodyCallbacks 0 "
                            + "setterBodyListeners 0 setterBodyAllocations 0 "
                            + "plainNonVolatile true throwStage - exception -%n",
                    prefix, run.bAlias, run.linkedBefore, run.linkedAfter,
                    run.assignmentAttempted, run.assignmentCompleted, run.valueChanged ? 1 : 0,
                    run.valueChanged ? "FalseToTrue" : "TrueToTrueIdempotent");

            System.out.printf(
                    "stemsbeamvlinkblinkerflagresult %s terminal ReadyBeforeSiblingBeamLinks "
                            + "applyReturn %s supportGrade %s supportGradeRead false "
                            + "stemAlias %s stemInterId %d bAlias %s linked true "
                            + "writeCount 1 valueChangeCount %d stateHashAfter %s "
                            + "linkedCountAfter %d linkedAliasesAfter %s linkedSetHashAfter %s "
                            + "linkSiblingsCalled false%n",
                    prefix, boolToken(run.baseRun.applyReturned),
                    run.baseEvidence.supportGrade,
                    run.stemAlias, run.stem.getId(), run.bAlias, run.valueChanged ? 1 : 0,
                    run.after.stateHash, run.after.linkedAliases.size(),
                    list(run.after.linkedAliases), run.after.linkedSetHash);

            final PersistentSnapshot persistentAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            System.out.printf(
                    "stemsbeamvlinkblinkerflagdeltaguard %s guardDomain %s "
                            + "allocatorBefore %d allocatorAfter %d "
                            + "glyphActiveHashBefore %s glyphActiveHashAfter %s "
                            + "glyphOriginalsHashBefore %s glyphOriginalsHashAfter %s "
                            + "interIndexHashBefore %s interIndexHashAfter %s "
                            + "sigHashBefore %s sigHashAfter %s sigRelationHashBefore %s "
                            + "sigRelationHashAfter %s systemStemsHashBefore %s "
                            + "systemStemsHashAfter %s lineHashBefore %s lineHashAfter %s "
                            + "arenaTopologyHashBefore %s arenaTopologyHashAfter %s "
                            + "allBIdentityOrderUnchanged true allBClosedUnchanged true "
                            + "unrelatedBLinkedUnchanged true headLinkerFlagsUnchanged true "
                            + "noCellOtherThanSelectedChanged true allowedMutation %s "
                            + "supportGradeRead false siblingDiscoveryRead false "
                            + "stopBeforeSiblingBeamLinks true stopBeforeHeadRelationLoop true%n",
                    prefix, run.scope.equals("real")
                            ? "RealCommittedSheet" : "IsolatedCellPlusEnclosingRealUnchanged",
                    run.persistentBefore.allocator, persistentAfter.allocator,
                    run.persistentBefore.glyphs.activeHash, persistentAfter.glyphs.activeHash,
                    run.persistentBefore.glyphs.originalsHash, persistentAfter.glyphs.originalsHash,
                    run.persistentBefore.inters.hash, persistentAfter.inters.hash,
                    run.persistentBefore.sig.hash, persistentAfter.sig.hash,
                    run.persistentBefore.sig.relationStateHash, persistentAfter.sig.relationStateHash,
                    run.persistentBefore.systemStems.hash, persistentAfter.systemStems.hash,
                    run.persistentBefore.lines.hash, persistentAfter.lines.hash,
                    run.before.topologyHash, run.after.topologyHash,
                    run.valueChanged ? "SelectedBLinkedFalseToTrue" : "IdempotentWriteNoValueDelta");

            System.out.printf(
                    "stemsbeamvlinkblinkerflagsummary %s transition %s writeCount 1 "
                            + "valueChangeCount %d observations %d arenaEntries %d "
                            + "dynamicAnchors %d applyReturn %s terminal ReadyBeforeSiblingBeamLinks%n",
                    prefix, run.valueChanged ? "FalseToTrue" : "TrueToTrueIdempotent",
                    run.valueChanged ? 1 : 0, run.observationsBefore.size(),
                    run.before.entries.size(), run.before.dynamicAnchors,
                    boolToken(run.baseRun.applyReturned));
        }

        void emitSyntheticBLinkerFlagCoverage (BLinkerFlagRun realRun)
            throws Exception
        {
            final PersistentSnapshot enclosingBefore = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final VerticalSide vSide = (VerticalSide) V_V_SIDE.get(realRun.currentV);
            final VerticalSide headToBeam = vSide.opposite();
            final int profile = Integer.parseInt(expected.values.get(6));

            final BaseApplySyntheticFixture successFixture = newBaseApplySyntheticFixture(
                    "ExistingFullSuccess", realRun.baseRun.beam, realRun.baseRun.stem,
                    headToBeam, profile, false, "ExistingAttached");
            final ApplyRun successRun = executeBaseApply(
                    "synthetic", "ExistingFullSuccess", successFixture.beam(),
                    "synthetic-stem:ExistingFullSuccess", successFixture.stem(),
                    successFixture.link(), FailureMode.NONE);
            assertSyntheticBaseApply(successRun);
            assertSyntheticBaseExpected(successRun, "ExistingFullSuccess");

            BaseApplySyntheticFixture suppressFixture = newBaseApplySyntheticFixture(
                    "ExistingDuplicateSuppress", realRun.baseRun.beam, realRun.baseRun.stem,
                    headToBeam, profile, false, "ExistingAttached");
            if (!suppressFixture.link().applyTo(suppressFixture.beam())) {
                throw new IllegalStateException("B15 synthetic duplicate setup relation missing");
            }
            clearSyntheticDirty(suppressFixture);
            suppressFixture = withFreshSyntheticLink(suppressFixture);
            final ApplyRun suppressRun = executeBaseApply(
                    "synthetic", "ExistingDuplicateSuppress", suppressFixture.beam(),
                    "synthetic-stem:ExistingDuplicateSuppress", suppressFixture.stem(),
                    suppressFixture.link(), FailureMode.NONE);
            assertSyntheticBaseApply(suppressRun);
            assertSyntheticBaseExpected(suppressRun, "ExistingDuplicateSuppress");

            emitOneSyntheticBLinkerFlagCase(
                    "InitialFalse", 1, false, successRun, "ExistingFullSuccess",
                    realRun.ref, enclosingBefore);
            emitOneSyntheticBLinkerFlagCase(
                    "IdempotentTrue", 1, true, successRun, "ExistingFullSuccess",
                    realRun.ref, enclosingBefore);
            emitOneSyntheticBLinkerFlagCase(
                    "ApplyReturnFalse", 1, false, suppressRun,
                    "ExistingDuplicateSuppress", realRun.ref, enclosingBefore);
            emitOneSyntheticBLinkerFlagCase(
                    "TwoChildrenShared", 2, false, successRun, "ExistingFullSuccess",
                    realRun.ref, enclosingBefore);

            final PersistentSnapshot enclosingAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            enclosingBefore.assertSame(enclosingAfter);
        }

        void assertSyntheticBaseExpected (ApplyRun run,
                                          String baseCase)
        {
            final BaseApplyRows rows = expected.syntheticBaseRows.get(baseCase);
            if (rows == null) throw new IllegalStateException("missing synthetic base rows");
            final Map<String, String> result = rows.result;
            final Map<String, String> guard = rows.guard;
            final Map<String, String> summary = rows.summary;
            assertField(result, "terminal", "ReadyBeforeBLinkerFlagMutation");
            assertField(result, "applyReturn", boolToken(run.applyReturned));
            assertField(result, "retainedDraftGrade", hex(((Support) run.link.relation).getGrade()));
            assertField(result, "finalStemAlias", "synthetic-stem:" + baseCase);
            assertField(result, "finalStemInterId", Integer.toString(run.stem.getId()));
            assertField(result, "allocator", Integer.toString(run.after.allocator));
            assertField(result, "interIndexHash", run.after.inters.hash);
            assertField(result, "sigVertexHash", run.after.graph.vertexHash);
            assertField(result, "sigEdgeHash", run.after.graph.edgeHash);
            assertField(result, "stubModified", Boolean.toString(run.after.dirty.stubModified));
            assertField(result, "bookModifiedRaw", Boolean.toString(run.after.dirty.bookModified));
            assertField(result, "bookDirty", Boolean.toString(run.after.dirty.bookDirty));
            assertField(guard, "stopBeforeBLinkerFlagMutation", "true");
            assertField(guard, "stopBeforeSiblingBeamLinks", "true");
            assertField(guard, "stopBeforeHeadRelationLoop", "true");
            assertField(summary, "applyReturned", boolToken(run.applyReturned));
            assertField(summary, "edgeAdded", Boolean.toString(
                    run.after.graph.edgeOrdinals.containsKey(run.link.relation)));
            assertField(summary, "terminal", "ReadyBeforeBLinkerFlagMutation");
        }

        void emitOneSyntheticBLinkerFlagCase (String caseName,
                                              int childCount,
                                              boolean initiallyLinked,
                                              ApplyRun baseRun,
                                              String baseCase,
                                              PlanRef realRef,
                                              PersistentSnapshot enclosingBefore)
            throws Exception
        {
            final IsolatedBCell cell = newIsolatedBCell(caseName, childCount, initiallyLinked);
            final BaseApplyRows rows = expected.syntheticBaseRows.get(baseCase);
            final BaseEvidence evidence = BaseEvidence.of(baseCase, rows);
            final PlanRef ref = new PlanRef(
                    realRef.plan, realRef.builder, cell.alias, VerticalSide.TOP);
            final BLinkerArenaSnapshot before = BLinkerArenaSnapshot.captureIsolated(
                    cell.b, cell.alias);
            final List<BLinkerObservation> observationsBefore = observations(
                    cell.b, cell.currentV, cell.alias);
            if (cell.b.getClass() != B_LINKER_CLASS
                    || B_LINKED.getDeclaringClass() != B_LINKER_CLASS
                    || B_LINKED.getType() != boolean.class
                    || java.lang.reflect.Modifier.isVolatile(B_LINKED.getModifiers())
                    || B_LINKER_CLASS.getMethod("setLinked", boolean.class)
                            .getDeclaringClass() != B_LINKER_CLASS) {
                throw new IllegalStateException("isolated exact dispatch drift");
            }
            if (V_GET_B_LINKER.invoke(cell.currentV) != cell.b) {
                throw new IllegalStateException("isolated V lexical parent drift");
            }
            final boolean linkedBefore = ((StemLinker) cell.b).isLinked();
            final boolean closedBefore = ((StemLinker) cell.b).isClosed();
            ((BeamLinker.BLinker) V_GET_B_LINKER.invoke(cell.currentV)).setLinked(true);
            final boolean linkedAfter = ((StemLinker) cell.b).isLinked();
            final boolean closedAfter = ((StemLinker) cell.b).isClosed();
            final BLinkerArenaSnapshot after = BLinkerArenaSnapshot.captureIsolated(
                    cell.b, cell.alias);
            final List<BLinkerObservation> observationsAfter = observations(
                    cell.b, cell.currentV, cell.alias);
            final ApplyMoment baseStateAfterFlag = new ApplyMoment(
                    baseRun.beam.getSig().getSystem().getSheet(), baseRun.beam.getSig(),
                    baseRun.beam, baseRun.stem);
            before.assertOnlyTargetCellChanged(after, cell.b);
            assertObservationTransition(observationsBefore, observationsAfter, linkedBefore);
            if (!baseRun.after.same(baseStateAfterFlag)) {
                throw new IllegalStateException("isolated B flag mutated boundary-14 state");
            }
            if (!linkedAfter || closedBefore != closedAfter) {
                throw new IllegalStateException("isolated B write transition drift");
            }
            final BLinkerFlagRun run = new BLinkerFlagRun(
                    "synthetic", caseName, cell.b, cell.currentV, cell.alias,
                    cell.alias + ":v:TOP", ref, baseRun, evidence,
                    "synthetic-stem:" + baseCase, baseRun.stem, enclosingBefore,
                    before, after, observationsBefore, observationsAfter, linkedBefore,
                    linkedAfter, closedBefore, closedAfter, true, true, !linkedBefore);
            emitBLinkerFlagRows(run);
            final PersistentSnapshot enclosingAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            enclosingBefore.assertSame(enclosingAfter);
            totals.transactions++;
        }

        IsolatedBCell newIsolatedBCell (String caseName,
                                       int childCount,
                                       boolean initiallyLinked)
            throws Exception
        {
            final BeamLinker outer = (BeamLinker) UNSAFE_ALLOCATE_INSTANCE.invoke(
                    UNSAFE, BeamLinker.class);
            final BeamInter syntheticBeam = new BeamInter(
                    null, new Line2D.Double(0.0, 0.0, 10.0, 0.0), 2.0);
            LINKER_BEAM.set(outer, syntheticBeam);
            LINKER_ALL_B.set(outer, new ArrayList<>());
            LINKER_SIDE_B.set(outer, new EnumMap<HorizontalSide, Object>(HorizontalSide.class));
            LINKER_STUMP_V.set(outer, new ArrayList<>());
            final BeamLinker.BLinker b = outer.new BLinker(
                    null, null, new Point2D.Double(5.0, 0.0), false);
            final Map<VerticalSide, Object> children =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            final List<VerticalSide> sides = childCount == 1
                    ? List.of(VerticalSide.TOP)
                    : List.of(VerticalSide.TOP, VerticalSide.BOTTOM);
            for (VerticalSide side : sides) {
                // This certificate exercises only the exact shared B/V linked cell. Constructing a
                // live non-anchor VLinker would execute geometry discovery against a real System;
                // instead allocate the exact runtime class and initialize only the lexical-parent
                // and side fields that the setter seam observes. It is emitted and graded solely as
                // IsolatedSyntheticCell, never as a live/frozen topology entry.
                final Object v = UNSAFE_ALLOCATE_INSTANCE.invoke(UNSAFE, V_LINKER_CLASS);
                V_OUTER_B.set(v, b);
                V_Y_DIR.setInt(v, side.direction());
                V_V_SIDE.set(v, side);
                children.put(side, v);
            }
            b.setLinked(initiallyLinked);
            final List<Object> arena = (List<Object>) LINKER_ALL_B.get(outer);
            if (arena.size() != 1 || arena.get(0) != b || B_ID.getInt(b) != 1
                    || B_CLOSED.getBoolean(b) || children.size() != childCount) {
                throw new IllegalStateException("isolated B arena setup drift");
            }
            return new IsolatedBCell(
                    outer, b, children.get(VerticalSide.TOP),
                    "synthetic-b:" + caseName);
        }

        ReuseRun runRealReuse (int plan,
                               LinkedHashMap<StemLinker, Relation> relations,
                               StemInter initialStem,
                               LiveStemCatalogue liveStems,
                               IdentityHashMap<Relation, Integer> sigEdgeOrdinals)
        {
            StemInter stem = initialStem;
            final List<ReuseEntryTrace> rows = new ArrayList<>();
            boolean broken = false;
            boolean javaNull = false;
            int selectedOrdinal = -1;
            String selectedC = "-";
            int entriesRead = 0;
            int linkedEntries = 0;
            int scanCount = 0;
            int mapOrdinal = 0;
            final IdentityHashMap<HeadInter, String> headSnapshotHashes = new IdentityHashMap<>();
            for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
                final String alias = cAliases.get(entry.getKey());
                if (broken || javaNull) {
                    rows.add(ReuseEntryTrace.unread(mapOrdinal, alias));
                    mapOrdinal++;
                    continue;
                }
                entriesRead++;
                if (!C_LINKER_CLASS.isInstance(entry.getKey())
                        || !(entry.getValue() instanceof HeadStemRelation)) {
                    throw new IllegalStateException("VLink reuse relation is not C/HeadStem");
                }
                final HeadLinker.SLinker.CLinker cl =
                        (HeadLinker.SLinker.CLinker) entry.getKey();
                final HeadStemRelation hsRel = (HeadStemRelation) entry.getValue();
                final boolean linked = cl.isLinked();
                final HorizontalSide parentSide = cl.getSLinker().getHorizontalSide();
                final VerticalSide parentVSide = (VerticalSide) get(C_V_SIDE, cl);
                final HeadInter head = cl.getSource();
                final String refPoint = point(cl.getReferencePoint());
                if (!linked) {
                    rows.add(ReuseEntryTrace.unlinked(
                            mapOrdinal, alias, parentSide, parentVSide, refPoint,
                            head.getId(), hsRel.getHeadSide()));
                    mapOrdinal++;
                    continue;
                }
                linkedEntries++;
                final HeadScan scan = HeadScan.capture(
                        head, parentSide, liveStems, sigEdgeOrdinals);
                final String priorHeadSnapshot = headSnapshotHashes.put(head, scan.snapshotHash);
                if (priorHeadSnapshot != null && !priorHeadSnapshot.equals(scan.snapshotHash)) {
                    throw new IllegalStateException("same HeadInter scan snapshot diverged");
                }
                scanCount++;
                final Map<HorizontalSide, Set<StemInter>> sideMap = head.getSideStems();
                scan.assertMap(sideMap);
                final Set<StemInter> stems = sideMap.get(parentSide);
                if (stems == null) {
                    rows.add(ReuseEntryTrace.linked(
                            mapOrdinal, alias, parentSide, parentVSide, refPoint, head.getId(),
                            hsRel.getHeadSide(), scan, "MissingJavaNull", "-", "JavaNullPointer"));
                    javaNull = true;
                    mapOrdinal++;
                    continue;
                }
                // Exact Java source expression: a first unique side set selects iterator().next().
                if (stems.size() == 1) {
                    stem = stems.iterator().next();
                    if (stem.getId() <= 0 || stem.getSig() != system.getSig()) {
                        throw new IllegalStateException("head-side reuse target is not persistent SIG stem");
                    }
                    selectedOrdinal = mapOrdinal;
                    selectedC = alias;
                    rows.add(ReuseEntryTrace.linked(
                            mapOrdinal, alias, parentSide, parentVSide, refPoint, head.getId(),
                            hsRel.getHeadSide(), scan, "Present",
                            stemAlias(stem, initialStem, plan, liveStems),
                            "SelectBreak"));
                    broken = true;
                } else {
                    rows.add(ReuseEntryTrace.linked(
                            mapOrdinal, alias, parentSide, parentVSide, refPoint, head.getId(),
                            hsRel.getHeadSide(), scan, "Present", "-", "ContinueMultiple"));
                }
                mapOrdinal++;
            }
            final String outcome = javaNull ? "JavaNullPointer"
                    : selectedOrdinal >= 0 ? "Selected"
                            : linkedEntries == 0 ? "AllUnlinked" : "NoUnique";
            return new ReuseRun(
                    stem, rows, selectedOrdinal >= 0 ? Integer.toString(selectedOrdinal) : "-",
                    selectedC, selectedOrdinal >= 0 ? Integer.toString(selectedOrdinal) : "-",
                    entriesRead, linkedEntries, scanCount, outcome,
                    javaNull ? "JavaNullPointer" : "ContinueToCheck");
        }

        void assertReuseCheckExpected (int plan,
                                       String finalAlias,
                                       StemInter finalStem,
                                       ReuseRun reuse,
                                       Link link)
        {
            final Map<String, String> selection = expected.reuseSelection;
            final Map<String, String> check = expected.reuseCheckResult;
            final Map<String, String> guard = expected.reuseGuard;
            assertField(selection, "plan", Integer.toString(plan));
            assertField(selection, "finalStemAlias", finalAlias);
            assertField(selection, "finalStemInterId", Integer.toString(finalStem.getId()));
            assertField(selection, "reused", Boolean.toString(!"created:".concat(
                    Integer.toString(plan)).equals(finalAlias)));
            assertField(selection, "reuseOutcome", reuse.outcome);
            assertField(selection, "terminal", "ContinueToCheck");
            assertField(selection, "finalGlyphId", Integer.toString(finalStem.getGlyph().getId()));
            assertField(selection, "finalGlyph", glyphToken(finalStem.getGlyph()));
            assertField(selection, "finalGrade", hex(finalStem.getGrade()));
            assertField(selection, "finalMedian", line(finalStem.getMedian()));
            assertField(selection, "finalWidth", hex(finalStem.getWidth()));
            assertField(selection, "finalBounds", rectangle(finalStem.getBounds()));
            assertField(selection, "finalAbnormal", Boolean.toString(finalStem.isAbnormal()));
            assertField(selection, "finalSigAttached", Boolean.toString(finalStem.getSig() != null));
            assertField(selection, "finalStateHash", stemStateHash(finalStem));

            final BeamStemRelation relation = (BeamStemRelation) link.relation;
            assertField(check, "plan", Integer.toString(plan));
            assertField(check, "scope", "real");
            assertField(check, "case", "-");
            assertField(check, "checkOutcome", "Accepted");
            assertField(check, "linkNull", "false");
            assertField(check, "partnerAlias", finalAlias);
            assertField(check, "partnerInterId", Integer.toString(finalStem.getId()));
            assertField(check, "outgoing", Boolean.toString(link.outgoing));
            assertField(check, "relationClass", relation.getClass().getSimpleName());
            assertField(check, "portion", relation.getBeamPortion().toString());
            assertField(check, "dx", hex(relation.getDx()));
            assertField(check, "dy", hex(relation.getDy()));
            assertField(check, "grade", hex(relation.getGrade()));
            assertField(check, "impacts", impactsToken(relation.getImpacts()));
            assertField(check, "extension", point(relation.getExtensionPoint()));
            assertField(check, "terminal", "ReadyBeforeSigMutation");
            for (String name : List.of(
                    "stopBeforeSigAddVertex", "stopBeforeRelationApply",
                    "stopBeforeLinkerFlagMutation")) {
                assertField(guard, name, "true");
            }
        }

        ApplyRun executeBaseApply (String scope,
                                   String caseName,
                                   AbstractBeamInter beam,
                                   String stemAlias,
                                   StemInter stem,
                                   Link link,
                                   FailureMode requestedFailure)
            throws Exception
        {
            final SIGraph sig = beam.getSig();
            final Sheet targetSheet = sig.getSystem().getSheet();
            final FailureMode failure = requestedFailure != null
                    ? requestedFailure : FailureMode.NONE;
            final String beamAlias = scope.equals("real")
                    ? "beam:" + beam.getId() : "synthetic-beam:" + caseName;
            final ApplyMoment before = new ApplyMoment(targetSheet, sig, beam, stem);
            final StemIncidentScan stemIncidentsBefore =
                    StemIncidentScan.captureIfPresent(sig, stem, before.graph);
            final BeamIncidentTrace beamIncidentsBefore =
                    BeamIncidentTrace.captureIfPresent(sig, beam, before.graph);
            final boolean newVertex = stem.getId() == 0;
            if (scope.equals("real")) {
                final int candidateId = before.allocator + 1;
                if (OMR.gui != null || !before.listeners.isStandard()
                        || !newVertex || interAtId(before.inters, candidateId) != null
                        || glyphActiveAtId(before.glyphs, candidateId) != null
                        || glyphOriginalAtId(before.glyphs, candidateId) != null
                        || targetSheet.getInterIndex().isVipId(candidateId)) {
                    throw new IllegalStateException("real compact base-apply envelope violated");
                }
            }
            final String relationBefore = relationState(link.relation);
            final BeamStemRelation draftBefore = (BeamStemRelation) link.relation;
            final String extensionBefore = point(draftBefore.getExtensionPoint());
            final String portionBefore = String.valueOf(draftBefore.getBeamPortion());
            Boolean vertexReturned = null;
            Boolean applyReturned = null;
            Throwable thrown = null;
            String throwStage = "-";

            if (newVertex) {
                try {
                    vertexReturned = sig.addVertex(stem);
                } catch (Throwable ex) {
                    thrown = ex;
                    throwStage = "VertexAdd";
                }
            }
            final ApplyMoment afterVertex = new ApplyMoment(targetSheet, sig, beam, stem);
            final DuplicateLookup duplicate;
            if (thrown == null) {
                duplicate = DuplicateLookup.capture(
                        sig, beam, stem, link.relation.getClass(), afterVertex.graph);
                try {
                    applyReturned = link.applyTo(beam);
                } catch (Throwable ex) {
                    thrown = ex;
                    throwStage = "LinkApply";
                }
            } else {
                duplicate = new DuplicateLookup(
                        "NotReadAfterVertexThrow", false, false, false, "-", 0,
                        "NotRead", "NotRead", List.of(), null, "NotReached");
            }
            final ApplyMoment after = new ApplyMoment(targetSheet, sig, beam, stem);
            final boolean draftInGraph = after.graph.edgeOrdinals.containsKey(link.relation);
            final boolean callbackCompleted = Boolean.TRUE.equals(applyReturned)
                    || (draftInGraph && failure == FailureMode.LATER_EDGE_LISTENER);
            final StemIncidentScan stemIncidents = callbackCompleted
                    ? StemIncidentScan.capture(sig, stem, after.graph)
                    : StemIncidentScan.notRead();
            final BeamIncidentTrace beamIncidents = callbackCompleted
                    ? BeamIncidentTrace.capture(sig, beam, after.graph)
                    : BeamIncidentTrace.notRead();
            final ApplyRun run = new ApplyRun(
                    scope, caseName, beam, beamAlias, stem, stemAlias, link, failure,
                    newVertex, before, afterVertex, after, vertexReturned, applyReturned,
                    thrown, throwStage, duplicate, relationBefore, relationState(link.relation),
                    extensionBefore, portionBefore, stemIncidentsBefore, stemIncidents,
                    beamIncidentsBefore, beamIncidents);
            assertApplyRun(run);
            return run;
        }

        void assertApplyRun (ApplyRun run)
        {
            if (!run.before.beam.sameExceptAbnormal(run.after.beam)) {
                throw new IllegalStateException("base apply mutated beam geometry/group state");
            }
            if (!beamGroupOrdinal(run.beam, run.before.graph)
                    .equals(beamGroupOrdinal(run.beam, run.after.graph))
                    || !beamGroupStateHash(run.beam, run.before.graph)
                            .equals(beamGroupStateHash(run.beam, run.after.graph))) {
                throw new IllegalStateException("base apply mutated beam group state");
            }
            if (!run.before.stem.sameExceptIdSigAbnormalVip(run.after.stem)) {
                throw new IllegalStateException("base apply mutated stem geometry/grade state");
            }
            if (!run.before.glyphs.sameIdentityState(run.after.glyphs)) {
                throw new IllegalStateException("base apply mutated glyph registries");
            }
            if (run.scope.equals("real") && !run.before.listeners.isStandard()) {
                throw new IllegalStateException("real SIG listener topology is not standard");
            }
            if (run.newVertex && run.failureMode == FailureMode.NONE) {
                if (!Boolean.TRUE.equals(run.vertexReturned)
                        || run.after.stem.sig != run.beam.getSig()
                        || run.after.stem.id != run.before.allocator + 1) {
                    throw new IllegalStateException("new-stem vertex transaction mismatch");
                }
            }
            if (Boolean.TRUE.equals(run.applyReturned)) {
                if (!run.after.graph.edgeOrdinals.containsKey(run.link.relation)
                        || run.stemIncidents.chordMatches != 0) {
                    throw new IllegalStateException("base relation/chord certificate mismatch");
                }
                if (!run.relationBefore.equals(run.relationAfter)) {
                    throw new IllegalStateException("accepted checkLink draft changed in callback");
                }
                if (run.after.beam.abnormal != run.beamIncidents.requestedAbnormal) {
                    throw new IllegalStateException("virtual beam abnormal callback mismatch");
                }
            }
            assertIncidentVertexDomains(run);
        }

        void assertIncidentVertexDomains (ApplyRun run)
        {
            final int baselineVertices = run.before.graph.vertices.size();
            for (IncidentRow row : run.stemIncidentsBefore.rows) {
                assertPreEdgeVertexOrdinal(row.oppositeVertexOrdinal, baselineVertices);
            }
            for (BeamIncidentRow row : run.beamIncidentsBefore.rows) {
                assertPreEdgeVertexOrdinal(row.oppositeVertexOrdinal, baselineVertices);
            }
            for (IncidentRow row : run.stemIncidents.rows) {
                final int ordinal = Integer.parseInt(row.oppositeVertexOrdinal);
                if (ordinal < 0 || ordinal >= run.after.graph.vertices.size()
                        || (row.relation != run.link.relation && ordinal >= baselineVertices)) {
                    throw new IllegalStateException("post-stem incident endpoint domain drift");
                }
            }
            for (BeamIncidentRow row : run.beamIncidents.rows) {
                final int ordinal = Integer.parseInt(row.oppositeVertexOrdinal);
                if (ordinal < 0 || ordinal >= run.after.graph.vertices.size()
                        || (row.relation != run.link.relation && ordinal >= baselineVertices)
                        || (row.relation == run.link.relation && run.newVertex
                                && ordinal != baselineVertices)) {
                    throw new IllegalStateException("post-beam incident endpoint domain drift");
                }
            }
        }

        void assertPreEdgeVertexOrdinal (String token,
                                         int baselineVertices)
        {
            final int ordinal = Integer.parseInt(token);
            if (ordinal < 0 || ordinal >= baselineVertices) {
                throw new IllegalStateException("pre-edge incident endpoint domain drift");
            }
        }


        void assertRealBaseApplyDelta (PersistentSnapshot before,
                                       PersistentSnapshot after,
                                       AbstractBeamInter beam,
                                       StemInter stem,
                                       Link link)
        {
            if (after.allocator != before.allocator + 1
                    || stem.getId() != after.allocator
                    || stem.getSig() != system.getSig()) {
                throw new IllegalStateException("real shared allocator/StemInter registration delta");
            }
            if (!before.glyphs.sameIdentityState(after.glyphs)
                    || !before.linkers.same(after.linkers)
                    || !before.systemStems.sameMapIdentity(after.systemStems)
                    || !before.lines.same(after.lines)) {
                throw new IllegalStateException("base apply crossed non-SIG state boundary");
            }
            final IdentityHashMap<Inter, Boolean> expectedInters = copySet(before.inters.identities);
            expectedInters.put(stem, true);
            if (!identitySetEquals(expectedInters, after.inters.identities)) {
                throw new IllegalStateException("unexpected InterIndex identity delta");
            }
            final IdentityHashMap<Inter, Boolean> expectedVertices = copySet(before.sig.vertices);
            expectedVertices.put(stem, true);
            final IdentityHashMap<Relation, Boolean> expectedEdges = copySet(before.sig.edges);
            expectedEdges.put(link.relation, true);
            if (!identitySetEquals(expectedVertices, after.sig.vertices)
                    || !identitySetEquals(expectedEdges, after.sig.edges)
                    || !system.getSig().containsEdge(link.relation)
                    || system.getSig().getEdgeSource(link.relation) != beam
                    || system.getSig().getEdgeTarget(link.relation) != stem) {
                throw new IllegalStateException("unexpected SIG vertex/relation identity delta");
            }
            for (Map.Entry<Relation, String> entry : before.sig.relationStates.entrySet()) {
                if (!entry.getValue().equals(after.sig.relationStates.get(entry.getKey()))) {
                    throw new IllegalStateException("existing SIG relation payload changed");
                }
            }
        }


        BaseApplySyntheticFixture newBaseApplySyntheticFixture (String caseName,
                                                                AbstractBeamInter realBeam,
                                                                StemInter realStem,
                                                                VerticalSide headToBeam,
                                                                int profile,
                                                                boolean hook,
                                                                String stemSetup)
            throws Exception
        {
            final Book book = new Book(Path.of(
                    "stems-beam-vlink-base-apply-" + caseName + ".synthetic"));
            final SheetStub stub = new SheetStub(book, 1);
            book.addStub(stub);
            final Sheet syntheticSheet = new Sheet(stub, (RunTable) null);
            syntheticSheet.setScale(sheet.getScale());
            final SystemInfo syntheticSystem = new SystemInfo(1, syntheticSheet, List.of());
            syntheticSheet.getSystemManager().setSystems(List.of(syntheticSystem));
            final SIGraph sig = syntheticSystem.getSig();
            if (!ListenerTopology.capture(sig).isStandard()) {
                throw new IllegalStateException("isolated synthetic SIG lacks sole SigListener");
            }
            final AbstractBeamInter beam = hook
                    ? new BeamHookInter(
                            realBeam.getImpacts(), copy(realBeam.getMedian()), realBeam.getHeight())
                    : new BeamInter(
                            realBeam.getImpacts(), copy(realBeam.getMedian()), realBeam.getHeight());
            if (!sig.addVertex(beam)) {
                throw new IllegalStateException("isolated synthetic beam was not added");
            }
            final StemInter stem = new StemInter(null, realStem.getGrade());
            stem.setWidth(realStem.getWidth());
            stem.setMedian(realStem.getMedian().getP1(), realStem.getMedian().getP2());
            switch (stemSetup) {
            case "ExistingAttached" -> {
                if (!sig.addVertex(stem)) {
                    throw new IllegalStateException("isolated synthetic stem was not added");
                }
            }
            case "NewIdZero" -> {
                if (stem.getId() != 0 || stem.getSig() != null) {
                    throw new IllegalStateException("synthetic new stem is not detached ID-zero");
                }
            }
            case "MissingPositive" -> stem.setId(1_500_000_001);
            default -> throw new IllegalArgumentException("unknown synthetic stem setup " + stemSetup);
            }
            clearSyntheticDirty(book, stub);
            final Link link = BeamStemRelation.checkLink(
                    beam, stem, headToBeam, syntheticSheet.getScale(), profile);
            if (link == null || link.partner != stem || !link.outgoing) {
                throw new IllegalStateException("isolated synthetic checkLink was not accepted");
            }
            return new BaseApplySyntheticFixture(
                    book, stub, syntheticSheet, syntheticSystem, sig, beam, stem,
                    headToBeam, profile, link);
        }

        BaseApplySyntheticFixture withFreshSyntheticLink (BaseApplySyntheticFixture fixture)
        {
            final Link fresh = BeamStemRelation.checkLink(
                    fixture.beam(), fixture.stem(), fixture.headToBeam(),
                    fixture.sheet().getScale(), fixture.profile());
            if (fresh == null || fresh.relation == fixture.link().relation) {
                throw new IllegalStateException("synthetic fresh draft relation identity drift");
            }
            return new BaseApplySyntheticFixture(
                    fixture.book(), fixture.stub(), fixture.sheet(), fixture.system(), fixture.sig(),
                    fixture.beam(), fixture.stem(), fixture.headToBeam(), fixture.profile(), fresh);
        }

        void clearSyntheticDirty (BaseApplySyntheticFixture fixture)
        {
            clearSyntheticDirty(fixture.book(), fixture.stub());
        }

        void clearSyntheticDirty (Book book,
                                  SheetStub stub)
        {
            stub.setModified(false);
            book.setModified(false);
            book.setDirty(false);
        }


        void assertSyntheticBaseApply (ApplyRun run)
        {
            final IdentityHashMap<Inter, Boolean> expectedInters = copySet(run.before.inters.identities);
            final IdentityHashMap<Inter, Boolean> expectedVertices = new IdentityHashMap<>();
            for (Inter inter : run.before.graph.vertexOrdinals.keySet()) {
                expectedVertices.put(inter, true);
            }
            final IdentityHashMap<Relation, Boolean> expectedEdges = new IdentityHashMap<>();
            for (Relation relation : run.before.graph.edgeOrdinals.keySet()) {
                expectedEdges.put(relation, true);
            }
            if (run.newVertex) {
                expectedInters.put(run.stem, true);
                expectedVertices.put(run.stem, true);
            }
            final boolean draftAdded = run.after.graph.edgeOrdinals.containsKey(run.link.relation);
            if (draftAdded) expectedEdges.put(run.link.relation, true);
            if (!identitySetEquals(expectedInters, run.after.inters.identities)
                    || !identityKeysEqual(expectedVertices, run.after.graph.vertexOrdinals)
                    || !identityKeysEqual(expectedEdges, run.after.graph.edgeOrdinals)) {
                throw new IllegalStateException("isolated synthetic mutation prefix drift");
            }
            if (run.caseName.endsWith("Success")) {
                if (!Boolean.TRUE.equals(run.applyReturned) || run.thrown != null || !draftAdded) {
                    throw new IllegalStateException("synthetic success did not add the draft edge");
                }
            } else if (run.caseName.endsWith("Suppress")) {
                if (!Boolean.FALSE.equals(run.applyReturned) || run.thrown != null || draftAdded) {
                    throw new IllegalStateException("synthetic suppression mutated the graph");
                }
            } else if (run.scope.equals("envelope")) {
                if (run.thrown == null || run.applyReturned != null) {
                    throw new IllegalStateException("synthetic envelope failure did not throw");
                }
            }
            if (run.failureMode == FailureMode.VERTEX_LISTENER) {
                if (run.stem.getSig() != null || run.stem.isAbnormal()) {
                    throw new IllegalStateException("early vertex listener ran after SigListener");
                }
            } else if (run.failureMode == FailureMode.EARLY_EDGE_LISTENER) {
                if (!draftAdded || !run.relationBefore.equals(run.relationAfter)
                        || !run.stemIncidents.state.equals("NotRead")) {
                    throw new IllegalStateException("early edge listener prefix drift");
                }
            } else if (run.failureMode == FailureMode.LATER_EDGE_LISTENER) {
                if (!draftAdded || !run.stemIncidents.state.startsWith("Exhaustive")) {
                    throw new IllegalStateException("later edge listener did not follow callback");
                }
            }
        }


        String beamGroupOrdinal (AbstractBeamInter beam,
                                 GraphOrder graph)
        {
            return beam.getGroup() != null
                    ? graph.vertexOrdinal((Inter) beam.getGroup()) : "-";
        }

        String beamGroupStateHash (AbstractBeamInter beam,
                                   GraphOrder graph)
        {
            if (beam.getGroup() == null) return "-";
            final List<String> rows = new ArrayList<>();
            rows.add("group=" + interStructuralToken((Inter) beam.getGroup()));
            int memberOrdinal = 0;
            for (Inter member : beam.getGroup().getMembers()) {
                rows.add("member=" + memberOrdinal++ + ":vertex="
                        + graph.vertexOrdinal(member) + ":" + interStructuralToken(member));
            }
            return sha256Rows(rows);
        }

        String beamGroupObjectStateHash (AbstractBeamInter beam)
        {
            final BeamGroupInter group = beam.getGroup();
            return group != null
                    ? sha256Rows(List.of("group=" + interStructuralToken(group))) : "-";
        }


        void assertExpected (AbstractBeamInter beam,
                             HorizontalSide hSide,
                             Object b,
                             Object v,
                             PlanRef ref,
                             int stemProfile,
                             int linkProfile,
                             int lastIndex,
                             int relations,
                             int glyphs)
        {
            final List<String> actual = List.of(
                    Integer.toString(beamSigOrdinals.get(beam)), hSide.toString(), bAliases.get(b),
                    get(V_V_SIDE, v).toString(), Integer.toString(ref.builder),
                    Integer.toString(ref.plan), Integer.toString(stemProfile),
                    Integer.toString(linkProfile), Integer.toString(lastIndex),
                    Integer.toString(relations), Integer.toString(glyphs));
            if (!actual.equals(expected.values)) {
                throw new IllegalStateException(
                        "frontier does not join frozen fixtures: actual=" + actual
                                + " expected=" + expected.values);
            }
        }
    }

    private record GroupWork(int outgoingOrdinal,
                             Relation relation,
                             Inter target,
                             AbstractBeamInter beam,
                             int memberOrdinal,
                             Point2D cross,
                             double leftLimit,
                             double rightLimit,
                             boolean inclusiveLeft,
                             boolean inclusiveRight,
                             boolean selected)
    {
    }

    private record GroupScanRow(int outgoingOrdinal,
                                Relation containmentRelation,
                                String graphRelationIdentity,
                                String relationObjectIdentity,
                                String runtimeClass,
                                boolean containment,
                                Inter target,
                                String targetRuntimeClass,
                                String targetReadByGetMembers,
                                String targetEvidence,
                                String targetAlias,
                                String targetVertexOrdinal,
                                String memberOrdinal,
                                String beamRuntimeClass,
                                String beamInterId,
                                String interIndexOrdinal,
                                String interIndexObjectMatches,
                                String interIndexIdMatches,
                                String sigMembership,
                                String sigSystemId,
                                String beamRemoved,
                                String beamVip,
                                String beamAbnormal,
                                String beamGroupVertexOrdinal,
                                String beamGroupStateHash,
                                String beamGroupObjectStateHash,
                                String median,
                                String height,
                                String glyphIdentity,
                                String glyph,
                                String cross,
                                String leftLimit,
                                String rightLimit,
                                String inclusiveLeft,
                                String inclusiveRight,
                                String selected,
                                String sortedOrdinal,
                                String baseIdentity,
                                String removeAction)
    {
    }

    private record SiblingPairRow(int pairOrdinal,
                                  String sourceOutgoingOrdinal,
                                  Relation relation,
                                  String graphRelationIdentity,
                                  String relationObjectIdentity,
                                  String runtimeClass,
                                  boolean classRead,
                                  boolean matches,
                                  String action)
    {
    }

    private record SiblingSourceOutgoingRow(int sourceOutgoingOrdinal,
                                            Relation relation,
                                            String graphRelationIdentity,
                                            String relationObjectIdentity,
                                            String runtimeClass)
    {
    }

    private static final class SiblingPairScan
    {
        final String state;
        final int sourceOutgoingScanned;
        final String sourceOutgoingHash;
        final List<SiblingSourceOutgoingRow> sourceOutgoingRows;
        final List<SiblingPairRow> rows;
        final String hash;
        final Relation firstMatch;

        SiblingPairScan (String state,
                         int sourceOutgoingScanned,
                         String sourceOutgoingHash,
                         List<SiblingSourceOutgoingRow> sourceOutgoingRows,
                         List<SiblingPairRow> rows,
                         Relation firstMatch)
        {
            this.state = state;
            this.sourceOutgoingScanned = sourceOutgoingScanned;
            this.sourceOutgoingHash = sourceOutgoingHash;
            this.sourceOutgoingRows = List.copyOf(sourceOutgoingRows);
            this.rows = List.copyOf(rows);
            this.firstMatch = firstMatch;
            final List<String> tokens = new ArrayList<>();
            for (SiblingPairRow row : rows) {
                tokens.add(row.pairOrdinal + ":" + row.sourceOutgoingOrdinal + ":"
                        + row.graphRelationIdentity + ":" + row.relationObjectIdentity + ":"
                        + row.runtimeClass);
            }
            hash = state.equals("LazyDirectedPairClass")
                    ? sha256Rows(tokens) : "NotRead";
        }

        static SiblingPairScan notRead ()
        {
            return new SiblingPairScan(
                    "NotRead", 0, "NotRead", List.of(), List.of(), null);
        }

        static SiblingPairScan capture (SIGraph sig,
                                        Inter source,
                                        Inter target,
                                        GraphOrder graph)
        {
            final Set<Relation> rawPair = sig.getAllEdges(source, target);
            if (rawPair == null) {
                return new SiblingPairScan(
                        "MissingEndpoint", 0, "NotRead", List.of(), List.of(), null);
            }
            final List<Relation> outgoing = new ArrayList<>(sig.outgoingEdgesOf(source));
            final IdentityHashMap<Relation, Integer> outgoingOrdinals = new IdentityHashMap<>();
            final List<String> outgoingTokens = new ArrayList<>();
            final List<SiblingSourceOutgoingRow> outgoingRows = new ArrayList<>();
            for (int i = 0; i < outgoing.size(); i++) {
                final Relation relation = outgoing.get(i);
                final String graphIdentity = graph.edgeAlias(relation);
                final String objectIdentity =
                        "sig-relation-object:" + graph.edgeOrdinals.get(relation);
                final String runtimeClass = relation.getClass().getName();
                outgoingOrdinals.put(relation, i);
                outgoingRows.add(new SiblingSourceOutgoingRow(
                        i, relation, graphIdentity, objectIdentity, runtimeClass));
                outgoingTokens.add(i + ":" + graphIdentity + ":" + objectIdentity + ":"
                        + runtimeClass);
            }
            Relation first = null;
            int breakAt = -1;
            int ordinal = 0;
            for (Relation relation : rawPair) {
                if (BeamStemRelation.class.isInstance(relation)) {
                    first = relation;
                    breakAt = ordinal;
                    break;
                }
                ordinal++;
            }
            final List<SiblingPairRow> rows = new ArrayList<>();
            ordinal = 0;
            for (Relation relation : rawPair) {
                final boolean read = breakAt < 0 || ordinal <= breakAt;
                final boolean matches = read && BeamStemRelation.class.isInstance(relation);
                final Integer sourceOrdinal = outgoingOrdinals.get(relation);
                if (sourceOrdinal == null) {
                    throw new IllegalStateException("directed pair edge absent from source outgoing");
                }
                rows.add(new SiblingPairRow(
                        ordinal, Integer.toString(sourceOrdinal), relation,
                        graph.edgeAlias(relation),
                        "sig-relation-object:" + graph.edgeOrdinals.get(relation),
                        relation.getClass().getName(), read, matches,
                        !read ? "UnreadAfterBreak" : matches ? "SelectBreak" : "Continue"));
                ordinal++;
            }
            return new SiblingPairScan(
                    "LazyDirectedPairClass", outgoing.size(), sha256Rows(outgoingTokens),
                    outgoingRows, rows, first);
        }
    }

    private record BuilderItemRow(int itemOrdinal,
                                  String runtimeClass,
                                  String linkerRead,
                                  String sourceRead,
                                  String linkerAlias,
                                  String linkerRuntimeClass,
                                  String sourceAlias,
                                  String sourceInterId,
                                  String identityMatch,
                                  String action)
    {
    }

    private static final class BuilderLookup
    {
        final String state;
        final String timing;
        final List<BuilderItemRow> rows;
        final String hash;
        final StemLinker selected;
        final String selectedAlias;
        final boolean linkedBefore;
        final boolean closedBefore;

        BuilderLookup (String state,
                       String timing,
                       List<BuilderItemRow> rows,
                       StemLinker selected,
                       String selectedAlias,
                       boolean linkedBefore,
                       boolean closedBefore)
        {
            this.state = state;
            this.timing = timing;
            this.rows = List.copyOf(rows);
            this.selected = selected;
            this.selectedAlias = selectedAlias;
            this.linkedBefore = linkedBefore;
            this.closedBefore = closedBefore;
            final List<String> tokens = new ArrayList<>();
            for (BuilderItemRow row : rows) {
                tokens.add(row.itemOrdinal + ":" + row.runtimeClass + ":" + row.linkerRead
                        + ":" + row.sourceRead + ":" + row.linkerAlias + ":"
                        + row.linkerRuntimeClass + ":" + row.sourceAlias + ":"
                        + row.sourceInterId + ":" + row.identityMatch + ":" + row.action);
            }
            hash = sha256Rows(tokens);
        }

        static BuilderLookup notRead ()
        {
            return new BuilderLookup(
                    "NotRead", "NotRead", List.of(), null, "-", false, false);
        }
    }

    private static final class SiblingPlan
    {
        final int siblingOrdinal;
        final int sortedOrdinal;
        final AbstractBeamInter beam;
        final String beamAlias;
        final String glyphIdentity;
        final boolean sameGlyph;
        final SiblingPairScan pair;
        final Point2D cross;
        final double siblingLength;
        final double ratio;
        final boolean shorter;
        final double dy;
        final double product;
        final boolean wrongSide;
        final Point2D extension;
        final BeamPortion portion;
        final String branch;
        final BuilderLookup lookup;
        final BeamObjectState beamBefore;

        SiblingPlan (int siblingOrdinal,
                     int sortedOrdinal,
                     AbstractBeamInter beam,
                     String beamAlias,
                     String glyphIdentity,
                     boolean sameGlyph,
                     SiblingPairScan pair,
                     Point2D cross,
                     double siblingLength,
                     double ratio,
                     boolean shorter,
                     double dy,
                     double product,
                     boolean wrongSide,
                     Point2D extension,
                     BeamPortion portion,
                     String branch,
                     BuilderLookup lookup)
        {
            this.siblingOrdinal = siblingOrdinal;
            this.sortedOrdinal = sortedOrdinal;
            this.beam = beam;
            this.beamAlias = beamAlias;
            this.glyphIdentity = glyphIdentity;
            this.sameGlyph = sameGlyph;
            this.pair = pair;
            this.cross = cross;
            this.siblingLength = siblingLength;
            this.ratio = ratio;
            this.shorter = shorter;
            this.dy = dy;
            this.product = product;
            this.wrongSide = wrongSide;
            this.extension = extension;
            this.portion = portion;
            this.branch = branch;
            this.lookup = lookup;
            beamBefore = new BeamObjectState(beam);
        }
    }

    private record AcceptedSibling(SiblingPlan plan,
                                   BeamStemRelation relation,
                                   String graphRelationIdentity,
                                   StemIncidentScan stemIncidents,
                                   BeamIncidentTrace beamIncidents,
                                   BeamObjectState beamAfter,
                                   boolean linkerLinkedAfter)
    {
    }

    private static final class SiblingLinksRun
    {
        final BLinkerFlagRun predecessor;
        final PersistentSnapshot persistentBefore;
        final PersistentSnapshot persistentAfter;
        final GraphOrder graphBefore;
        final GraphOrder graphAfter;
        final DirtyFlags dirtyBefore;
        final DirtyFlags dirtyAfter;
        final BLinkerArenaSnapshot arenaBefore;
        final BLinkerArenaSnapshot arenaAfter;
        final StemIncidentScan stemIncidentsBefore;
        final List<GroupScanRow> groupRows;
        final List<SiblingPlan> siblings;
        final List<AcceptedSibling> accepted;
        final String groupAlias;
        final String groupVertexOrdinal;
        final String groupStateHashBefore;
        final String groupObjectStateHash;
        final Line2D skewedVertical;
        final Point2D baseCross;
        final double baseLength;
        final double supportGrade;
        final int maxBeamSideDx;
        final double maxShorterRatio;
        final double xInGapMaximum;
        final int maxDx;
        final boolean baseRemoved;
        final String relationInputHashBefore;
        final String relationInputHashAfter;
        final int builderItemsCount;
        final String builderItemsHash;

        SiblingLinksRun (BLinkerFlagRun predecessor,
                         PersistentSnapshot persistentBefore,
                         PersistentSnapshot persistentAfter,
                         GraphOrder graphBefore,
                         GraphOrder graphAfter,
                         DirtyFlags dirtyBefore,
                         DirtyFlags dirtyAfter,
                         BLinkerArenaSnapshot arenaBefore,
                         BLinkerArenaSnapshot arenaAfter,
                         StemIncidentScan stemIncidentsBefore,
                         List<GroupScanRow> groupRows,
                         List<SiblingPlan> siblings,
                         List<AcceptedSibling> accepted,
                         String groupAlias,
                         String groupVertexOrdinal,
                         String groupStateHashBefore,
                         String groupObjectStateHash,
                         Line2D skewedVertical,
                         Point2D baseCross,
                         double baseLength,
                         double supportGrade,
                         int maxBeamSideDx,
                         double maxShorterRatio,
                         double xInGapMaximum,
                         int maxDx,
                         boolean baseRemoved,
                         String relationInputHashBefore,
                         String relationInputHashAfter,
                         int builderItemsCount,
                         String builderItemsHash)
        {
            this.predecessor = predecessor;
            this.persistentBefore = persistentBefore;
            this.persistentAfter = persistentAfter;
            this.graphBefore = graphBefore;
            this.graphAfter = graphAfter;
            this.dirtyBefore = dirtyBefore;
            this.dirtyAfter = dirtyAfter;
            this.arenaBefore = arenaBefore;
            this.arenaAfter = arenaAfter;
            this.stemIncidentsBefore = stemIncidentsBefore;
            this.groupRows = List.copyOf(groupRows);
            this.siblings = List.copyOf(siblings);
            this.accepted = List.copyOf(accepted);
            this.groupAlias = groupAlias;
            this.groupVertexOrdinal = groupVertexOrdinal;
            this.groupStateHashBefore = groupStateHashBefore;
            this.groupObjectStateHash = groupObjectStateHash;
            this.skewedVertical = copy(skewedVertical);
            this.baseCross = copyPoint(baseCross);
            this.baseLength = baseLength;
            this.supportGrade = supportGrade;
            this.maxBeamSideDx = maxBeamSideDx;
            this.maxShorterRatio = maxShorterRatio;
            this.xInGapMaximum = xInGapMaximum;
            this.maxDx = maxDx;
            this.baseRemoved = baseRemoved;
            this.relationInputHashBefore = relationInputHashBefore;
            this.relationInputHashAfter = relationInputHashAfter;
            this.builderItemsCount = builderItemsCount;
            this.builderItemsHash = builderItemsHash;
        }
    }

    private static final class HeadObjectState
    {
        final HeadInter head;
        final SIGraph sig;
        final Object area;
        final Glyph glyph;
        final GradeImpacts impacts;
        final Object staff;
        final Shape shape;
        final Object linker;
        final Object template;
        final Object oldMirror;
        final Double pitch;
        final String stableValue;
        final boolean abnormal;

        HeadObjectState (HeadInter head)
        {
            this.head = head;
            sig = head.getSig();
            area = get(INTER_AREA, head);
            glyph = head.getGlyph();
            impacts = head.getImpacts();
            staff = get(INTER_STAFF, head);
            shape = head.getShape();
            linker = get(HEAD_LINKER_FIELD, head);
            template = get(HEAD_TEMPLATE_FIELD, head);
            oldMirror = get(HEAD_OLD_MIRROR_FIELD, head);
            pitch = head.getPitch();
            final Rectangle bounds = head.getBounds();
            stableValue = head.getClass().getName() + ":id=" + head.getId()
                    + ":shape=" + shape + ":grade=" + hex(head.getGrade())
                    + ":impacts=" + impactsToken(impacts)
                    + ":bounds=" + (bounds != null ? rectangle(bounds) : "-")
                    + ":center=" + point(head.getCenter())
                    + ":pitch=" + (pitch != null ? hex(pitch) : "-")
                    + ":removed=" + head.isRemoved() + ":vip=" + head.isVip()
                    + ":manual=" + head.isManual() + ":implicit=" + head.isImplicit()
                    + ":profile=" + head.getProfile();
            abnormal = head.isAbnormal();
        }

        boolean sameExceptAbnormal (HeadObjectState that)
        {
            return head == that.head && sig == that.sig && area == that.area
                    && glyph == that.glyph && impacts == that.impacts && staff == that.staff
                    && shape == that.shape && linker == that.linker && template == that.template
                    && oldMirror == that.oldMirror
                    && java.util.Objects.equals(pitch, that.pitch)
                    && stableValue.equals(that.stableValue);
        }
    }

    private record SLinkerCellState(HeadLinker.SLinker identity,
                                    HeadInter head,
                                    HorizontalSide horizontal,
                                    boolean linked,
                                    boolean closed)
    {
        static SLinkerCellState capture (HeadLinker.SLinker linker)
        {
            return new SLinkerCellState(
                    linker, linker.getHead(), linker.getHorizontalSide(), linker.isLinked(),
                    linker.isClosed());
        }

        boolean sameExceptLinked (SLinkerCellState that)
        {
            return identity == that.identity && head == that.head
                    && horizontal == that.horizontal && closed == that.closed;
        }
    }

    private record HeadSourceOutgoingRow(int sourceOutgoingOrdinal,
                                         Relation relation,
                                         String graphRelationIdentity,
                                         String runtimeClass,
                                         String targetVertexOrdinal)
    {
    }

    private record HeadPairRow(int pairOrdinal,
                               int sourceOutgoingOrdinal,
                               Relation relation,
                               String graphRelationIdentity,
                               String runtimeClass,
                               boolean classRead,
                               boolean matches,
                               String action)
    {
    }

    private static final class HeadPairScan
    {
        final String state;
        final List<HeadSourceOutgoingRow> outgoing;
        final List<HeadPairRow> rows;
        final Relation selected;
        final String outgoingHash;
        final String pairHash;

        HeadPairScan (String state,
                      List<HeadSourceOutgoingRow> outgoing,
                      List<HeadPairRow> rows,
                      Relation selected)
        {
            this.state = state;
            this.outgoing = List.copyOf(outgoing);
            this.rows = List.copyOf(rows);
            this.selected = selected;
            final List<String> outgoingTokens = new ArrayList<>();
            for (HeadSourceOutgoingRow row : outgoing) {
                outgoingTokens.add(row.sourceOutgoingOrdinal + ":"
                        + row.graphRelationIdentity + ":" + row.runtimeClass);
            }
            final List<String> pairTokens = new ArrayList<>();
            for (HeadPairRow row : rows) {
                pairTokens.add(row.pairOrdinal + ":" + row.sourceOutgoingOrdinal + ":"
                        + row.graphRelationIdentity + ":" + row.runtimeClass);
            }
            outgoingHash = sha256Rows(outgoingTokens);
            pairHash = sha256Rows(pairTokens);
        }

        static HeadPairScan capture (SIGraph sig,
                                     HeadInter head,
                                     StemInter stem,
                                     GraphOrder graph)
        {
            if (!sig.containsVertex(head) || !sig.containsVertex(stem)) {
                return new HeadPairScan("MissingEndpoint", List.of(), List.of(), null);
            }
            final List<Relation> outgoingRelations = new ArrayList<>(sig.outgoingEdgesOf(head));
            final List<HeadSourceOutgoingRow> outgoing = new ArrayList<>();
            final IdentityHashMap<Relation, Integer> outgoingOrdinals = new IdentityHashMap<>();
            for (int ordinal = 0; ordinal < outgoingRelations.size(); ordinal++) {
                final Relation relation = outgoingRelations.get(ordinal);
                outgoingOrdinals.put(relation, ordinal);
                outgoing.add(new HeadSourceOutgoingRow(
                        ordinal, relation, graph.edgeAlias(relation),
                        relation.getClass().getName(),
                        graph.vertexOrdinal(sig.getEdgeTarget(relation))));
            }
            final Set<Relation> pairSet = sig.getAllEdges(head, stem);
            final List<Relation> pair = pairSet != null
                    ? new ArrayList<>(pairSet) : List.of();
            Relation selected = null;
            int selectedOrdinal = -1;
            for (int ordinal = 0; ordinal < pair.size(); ordinal++) {
                final Relation relation = pair.get(ordinal);
                if (!outgoingOrdinals.containsKey(relation)) {
                    throw new IllegalStateException(
                            "directed pair relation is absent from source outgoing order");
                }
                if (selected == null && HeadStemRelation.class.isInstance(relation)) {
                    selected = relation;
                    selectedOrdinal = ordinal;
                }
            }
            final List<HeadPairRow> rows = new ArrayList<>();
            for (int ordinal = 0; ordinal < pair.size(); ordinal++) {
                final Relation relation = pair.get(ordinal);
                final boolean read = selectedOrdinal < 0 || ordinal <= selectedOrdinal;
                final boolean matches = read && HeadStemRelation.class.isInstance(relation);
                rows.add(new HeadPairRow(
                        ordinal, outgoingOrdinals.get(relation), relation,
                        graph.edgeAlias(relation), relation.getClass().getName(), read, matches,
                        !read ? "UnreadAfterBreak" : matches ? "SelectBreak" : "Continue"));
            }
            return new HeadPairScan(
                    selected != null ? "FirstHeadStemMatch" : "ExhaustiveNoMatch",
                    outgoing, rows, selected);
        }
    }

    private record HeadIncidentRow(int incidentOrdinal,
                                   String direction,
                                   int directionOrdinal,
                                   Relation relation,
                                   String graphRelationIdentity,
                                   Inter opposite,
                                   String oppositeVertexOrdinal,
                                   boolean classRead,
                                   boolean matches,
                                   String action)
    {
    }

    private static final class HeadStemIncidentTrace
    {
        final String state;
        final List<HeadIncidentRow> rows;
        final boolean requestedAbnormal;
        final String hash;

        HeadStemIncidentTrace (String state,
                               List<HeadIncidentRow> rows,
                               boolean requestedAbnormal)
        {
            this.state = state;
            this.rows = List.copyOf(rows);
            this.requestedAbnormal = requestedAbnormal;
            final List<String> tokens = new ArrayList<>();
            for (HeadIncidentRow row : rows) {
                tokens.add(row.incidentOrdinal + ":" + row.direction + ":"
                        + row.directionOrdinal + ":" + row.graphRelationIdentity + ":"
                        + row.relation.getClass().getName() + ":" + row.opposite.getId()
                        + ":" + row.oppositeVertexOrdinal + ":" + row.classRead + ":"
                        + row.matches + ":" + row.action);
            }
            hash = sha256Rows(tokens);
        }

        static HeadStemIncidentTrace notRead (String state)
        {
            return new HeadStemIncidentTrace(state, List.of(), false);
        }

        static HeadStemIncidentTrace capture (SIGraph sig,
                                               Inter inter,
                                               GraphOrder graph)
        {
            final List<Relation> incident = new ArrayList<>();
            final List<String> directions = new ArrayList<>();
            final List<Integer> directionOrdinals = new ArrayList<>();
            int ordinal = 0;
            for (Relation relation : sig.incomingEdgesOf(inter)) {
                incident.add(relation);
                directions.add("Incoming");
                directionOrdinals.add(ordinal++);
            }
            ordinal = 0;
            for (Relation relation : sig.outgoingEdgesOf(inter)) {
                if (sig.getEdgeSource(relation) == inter && sig.getEdgeTarget(relation) == inter) {
                    continue;
                }
                incident.add(relation);
                directions.add("Outgoing");
                directionOrdinals.add(ordinal++);
            }
            final List<HeadIncidentRow> rows = new ArrayList<>();
            boolean found = false;
            for (int index = 0; index < incident.size(); index++) {
                final Relation relation = incident.get(index);
                final boolean read = !found;
                final boolean matches = read && HeadStemRelation.class.isInstance(relation);
                if (matches) found = true;
                final Inter opposite = sig.getOppositeInter(inter, relation);
                rows.add(new HeadIncidentRow(
                        index, directions.get(index), directionOrdinals.get(index), relation,
                        graph.edgeAlias(relation), opposite, graph.vertexOrdinal(opposite), read,
                        matches,
                        !read ? "UnreadAfterBreak" : matches ? "SelectBreak" : "Continue"));
            }
            return new HeadStemIncidentTrace(
                    "LazyIncomingThenOutgoing", rows, !found);
        }
    }

    private record HeadConsistencyTrace(boolean shapeRead,
                                        Shape shape,
                                        boolean small,
                                        Line2D medianCopy,
                                        int interline,
                                        double scaledStemLength,
                                        double neutralStemLength,
                                        double ratio,
                                        Double before,
                                        double after,
                                        boolean debugEnabled)
    {
    }

    private record HeadCallbackTrace(boolean headSideNull,
                                     boolean extensionNull,
                                     boolean relationManual,
                                     boolean headManualRead,
                                     boolean headManual,
                                     boolean stemManualRead,
                                     boolean stemManual,
                                     String chordBranch,
                                     HeadStemIncidentTrace headIncidents,
                                     HeadStemIncidentTrace stemIncidents,
                                     boolean headAbnormalBefore,
                                     boolean headAbnormalAfter,
                                     boolean stemAbnormalBefore,
                                     boolean stemAbnormalAfter,
                                     DirtyFlags dirtyBefore,
                                     DirtyFlags dirtyAfter)
    {
    }

    private record HeadEntryTrace(int mapOrdinal,
                                  StemLinker mapKey,
                                  HeadLinker.SLinker.CLinker corner,
                                  HeadInter head,
                                  HeadLinker.SLinker side,
                                  String cAlias,
                                  String sAlias,
                                  String headAlias,
                                  Relation planRelation,
                                  String planDraftIdentity,
                                  HeadObjectState headBefore,
                                  HeadObjectState headAfter,
                                  boolean stemAbnormalBefore,
                                  boolean stemAbnormalAfter,
                                  SLinkerCellState sideBefore,
                                  SLinkerCellState sideAfter,
                                  int sWriteEventOrdinal,
                                  HeadPairScan pair,
                                  String branch,
                                  HeadConsistencyTrace consistency,
                                  int consistencyEventOrdinal,
                                  String graphRelationIdentity,
                                  int edgeEventOrdinal,
                                  boolean insertionReturned,
                                  HeadCallbackTrace callback,
                                  int callbackEventOrdinal,
                                  String relationStateBefore,
                                  String relationStateAfter)
    {
    }

    private record InsertedHeadRelation(int mapOrdinal,
                                        HeadInter head,
                                        StemInter stem,
                                        HeadStemRelation relation,
                                        String draftIdentity,
                                        String graphIdentity)
    {
    }

    private static final class HeadLinksRun
    {
        final SiblingLinksRun predecessor;
        final PersistentSnapshot before;
        final PersistentSnapshot after;
        final GraphOrder graphBefore;
        final GraphOrder graphAfter;
        final DirtyFlags dirtyBefore;
        final DirtyFlags dirtyAfter;
        final List<HeadEntryTrace> entries;
        final List<InsertedHeadRelation> inserted;
        final IdentityHashMap<Relation, String> relationObjectAliases;
        final int lastIndex;
        final int maxIndex;
        final int builderItemCount;
        final boolean remainderPresent;
        final String relationInputHashBefore;
        final String relationInputHashAfter;
        final String headPlanStateHashBefore;
        final String headPlanStateHashAfter;
        final String builderItemsHash;
        final BaseStemState stemBefore;
        final BaseStemState stemAfter;
        final int eventCount;

        HeadLinksRun (SiblingLinksRun predecessor,
                      PersistentSnapshot before,
                      PersistentSnapshot after,
                      GraphOrder graphBefore,
                      GraphOrder graphAfter,
                      DirtyFlags dirtyBefore,
                      DirtyFlags dirtyAfter,
                      List<HeadEntryTrace> entries,
                      List<InsertedHeadRelation> inserted,
                      IdentityHashMap<Relation, String> relationObjectAliases,
                      int lastIndex,
                      int maxIndex,
                      int builderItemCount,
                      boolean remainderPresent,
                      String relationInputHashBefore,
                      String relationInputHashAfter,
                      String headPlanStateHashBefore,
                      String headPlanStateHashAfter,
                      String builderItemsHash,
                      BaseStemState stemBefore,
                      BaseStemState stemAfter,
                      int eventCount)
        {
            this.predecessor = predecessor;
            this.before = before;
            this.after = after;
            this.graphBefore = graphBefore;
            this.graphAfter = graphAfter;
            this.dirtyBefore = dirtyBefore;
            this.dirtyAfter = dirtyAfter;
            this.entries = List.copyOf(entries);
            this.inserted = List.copyOf(inserted);
            this.relationObjectAliases = relationObjectAliases;
            this.lastIndex = lastIndex;
            this.maxIndex = maxIndex;
            this.builderItemCount = builderItemCount;
            this.remainderPresent = remainderPresent;
            this.relationInputHashBefore = relationInputHashBefore;
            this.relationInputHashAfter = relationInputHashAfter;
            this.headPlanStateHashBefore = headPlanStateHashBefore;
            this.headPlanStateHashAfter = headPlanStateHashAfter;
            this.builderItemsHash = builderItemsHash;
            this.stemBefore = stemBefore;
            this.stemAfter = stemAfter;
            this.eventCount = eventCount;
        }
    }

    private static final class BLinkerFlagRun
    {
        final String scope;
        final String caseName;
        final Object b;
        final Object currentV;
        final String bAlias;
        final String vAlias;
        final PlanRef ref;
        final ApplyRun baseRun;
        final BaseEvidence baseEvidence;
        final String stemAlias;
        final StemInter stem;
        final PersistentSnapshot persistentBefore;
        final BLinkerArenaSnapshot before;
        final BLinkerArenaSnapshot after;
        final List<BLinkerObservation> observationsBefore;
        final List<BLinkerObservation> observationsAfter;
        final boolean linkedBefore;
        final boolean linkedAfter;
        final boolean closedBefore;
        final boolean closedAfter;
        final boolean assignmentAttempted;
        final boolean assignmentCompleted;
        final boolean valueChanged;

        BLinkerFlagRun (String scope,
                        String caseName,
                        Object b,
                        Object currentV,
                        String bAlias,
                        String vAlias,
                        PlanRef ref,
                        ApplyRun baseRun,
                        BaseEvidence baseEvidence,
                        String stemAlias,
                        StemInter stem,
                        PersistentSnapshot persistentBefore,
                        BLinkerArenaSnapshot before,
                        BLinkerArenaSnapshot after,
                        List<BLinkerObservation> observationsBefore,
                        List<BLinkerObservation> observationsAfter,
                        boolean linkedBefore,
                        boolean linkedAfter,
                        boolean closedBefore,
                        boolean closedAfter,
                        boolean assignmentAttempted,
                        boolean assignmentCompleted,
                        boolean valueChanged)
        {
            this.scope = scope;
            this.caseName = caseName;
            this.b = b;
            this.currentV = currentV;
            this.bAlias = bAlias;
            this.vAlias = vAlias;
            this.ref = ref;
            this.baseRun = baseRun;
            this.baseEvidence = baseEvidence;
            this.stemAlias = stemAlias;
            this.stem = stem;
            this.persistentBefore = persistentBefore;
            this.before = before;
            this.after = after;
            this.observationsBefore = observationsBefore;
            this.observationsAfter = observationsAfter;
            this.linkedBefore = linkedBefore;
            this.linkedAfter = linkedAfter;
            this.closedBefore = closedBefore;
            this.closedAfter = closedAfter;
            this.assignmentAttempted = assignmentAttempted;
            this.assignmentCompleted = assignmentCompleted;
            this.valueChanged = valueChanged;
        }
    }

    private record BaseEvidence(String caseName,
                                String terminal,
                                String supportGrade,
                                String frontierRowSha256,
                                String resultRowSha256,
                                String guardRowSha256,
                                String summaryRowSha256,
                                String evidenceSha256)
    {
        static BaseEvidence of (String caseName,
                                BaseApplyRows rows)
        {
            final String frontier = required(rows.frontier, "_rowSha256");
            final String result = required(rows.result, "_rowSha256");
            final String guard = required(rows.guard, "_rowSha256");
            final String summary = required(rows.summary, "_rowSha256");
            return new BaseEvidence(
                    caseName, required(rows.result, "terminal"),
                    required(rows.result, "retainedDraftGrade"), frontier, result, guard,
                    summary, sha256Rows(List.of(frontier, result, guard, summary)));
        }
    }

    private static final class BLinkerArenaSnapshot
    {
        final List<BLinkerArenaEntry> entries;
        final String topologyHash;
        final String stateHash;
        final List<String> linkedAliases;
        final String linkedSetHash;
        final List<String> closedAliases;
        final int frozenEntries;
        final int dynamicAnchors;
        final int noChild;
        final int child1;
        final int child2;
        final int targetMatches;
        final boolean targetFrozenMatch;

        BLinkerArenaSnapshot (List<BLinkerArenaEntry> entries,
                              String topologyHash,
                              String stateHash,
                              List<String> linkedAliases,
                              String linkedSetHash,
                              List<String> closedAliases,
                              int frozenEntries,
                              int dynamicAnchors,
                              int noChild,
                              int child1,
                              int child2,
                              int targetMatches,
                              boolean targetFrozenMatch)
        {
            this.entries = entries;
            this.topologyHash = topologyHash;
            this.stateHash = stateHash;
            this.linkedAliases = linkedAliases;
            this.linkedSetHash = linkedSetHash;
            this.closedAliases = closedAliases;
            this.frozenEntries = frozenEntries;
            this.dynamicAnchors = dynamicAnchors;
            this.noChild = noChild;
            this.child1 = child1;
            this.child2 = child2;
            this.targetMatches = targetMatches;
            this.targetFrozenMatch = targetFrozenMatch;
        }

        static BLinkerArenaSnapshot capture (List<Inter> beams,
                                             IdentityHashMap<Inter, Integer> beamOrdinals,
                                             IdentityHashMap<Object, String> aliases,
                                             Object target)
            throws Exception
        {
            final List<BLinkerArenaEntry> entries = new ArrayList<>();
            final List<String> topologyRows = new ArrayList<>();
            final List<String> stateRows = new ArrayList<>();
            final List<String> linked = new ArrayList<>();
            final List<String> closed = new ArrayList<>();
            final IdentityHashMap<Object, Boolean> visited = new IdentityHashMap<>();
            int frozen = 0;
            int anchors = 0;
            int noChild = 0;
            int child1 = 0;
            int child2 = 0;
            int targetMatches = 0;
            boolean targetFrozen = false;
            int arenaOrdinal = 0;
            for (int constructorOrdinal = 0; constructorOrdinal < beams.size();
                    constructorOrdinal++) {
                final Inter inter = beams.get(constructorOrdinal);
                final AbstractBeamInter beam = (AbstractBeamInter) inter;
                final Integer beamSig = beamOrdinals.get(inter);
                if (beamSig == null || beam.getLinker() == null) {
                    throw new IllegalStateException("live B arena beam identity drift");
                }
                final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
                for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                    final Object b = allB.get(bOrdinal);
                    if (visited.put(b, true) != null) {
                        throw new IllegalStateException("duplicate B identity in live allB arena");
                    }
                    final String alias = aliases.get(b);
                    if (alias == null || b.getClass() != B_LINKER_CLASS) {
                        throw new IllegalStateException("live B arena alias/class drift");
                    }
                    final int javaId = B_ID.getInt(b);
                    if (javaId != bOrdinal + 1) {
                        throw new IllegalStateException("allB insertion ID drift");
                    }
                    final boolean anchor = B_IS_ANCHOR.getBoolean(b);
                    final String origin = anchor ? "DynamicAnchor" : "FrozenVLinker";
                    if (anchor) anchors++;
                    else frozen++;
                    final Map<VerticalSide, Object> children =
                            (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                    final List<String> childAliases = new ArrayList<>();
                    final List<Object> childIdentities = new ArrayList<>();
                    for (Map.Entry<VerticalSide, Object> child : children.entrySet()) {
                        if (child.getValue().getClass() != V_LINKER_CLASS
                                || V_GET_B_LINKER.invoke(child.getValue()) != b
                                || V_V_SIDE.get(child.getValue()) != child.getKey()) {
                            throw new IllegalStateException("live B child topology drift");
                        }
                        childAliases.add(alias + ":v:" + child.getKey());
                        childIdentities.add(child.getValue());
                    }
                    switch (children.size()) {
                    case 0 -> noChild++;
                    case 1 -> child1++;
                    case 2 -> child2++;
                    default -> throw new IllegalStateException("B has more than two V children");
                    }
                    if (anchor && !children.isEmpty()) {
                        throw new IllegalStateException("dynamic anchor unexpectedly owns V child");
                    }
                    final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
                    final Glyph stump = (Glyph) B_STUMP.get(b);
                    final Point2D ref = (Point2D) B_REF_PT.get(b);
                    final boolean isTarget = b == target;
                    if (isTarget) {
                        targetMatches++;
                        targetFrozen = !anchor;
                    }
                    final BLinkerArenaEntry entry = new BLinkerArenaEntry(
                            b, arenaOrdinal, constructorOrdinal, Integer.toString(beamSig),
                            bOrdinal, javaId,
                            alias, origin, b.getClass().getName(),
                            hSide != null ? hSide.toString() : "-", anchor,
                            stump != null ? glyphToken(stump) : "-", point(ref), childAliases,
                            childIdentities,
                            B_LINKED.getBoolean(b), B_CLOSED.getBoolean(b), isTarget);
                    entries.add(entry);
                    topologyRows.add(entry.topologyToken());
                    stateRows.add(entry.stateToken());
                    if (entry.linked) linked.add(alias);
                    if (entry.closed) closed.add(alias);
                    arenaOrdinal++;
                }
            }
            if (entries.size() != aliases.size() || !identityKeysEqual(visited, aliases)) {
                throw new IllegalStateException("live allB arena differs from frozen alias census");
            }
            if (targetMatches != 1 || !targetFrozen) {
                throw new IllegalStateException("target is not unique frozen B entry");
            }
            return new BLinkerArenaSnapshot(
                    entries, sha256Rows(topologyRows), sha256Rows(stateRows), linked,
                    sha256Rows(linked), closed, frozen, anchors, noChild, child1, child2,
                    targetMatches, targetFrozen);
        }

        static BLinkerArenaSnapshot captureIsolated (Object b,
                                                     String alias)
            throws Exception
        {
            if (b.getClass() != B_LINKER_CLASS) {
                throw new IllegalStateException("isolated B runtime class drift");
            }
            final Map<VerticalSide, Object> children =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            final List<String> childAliases = new ArrayList<>();
            final List<Object> childIdentities = new ArrayList<>();
            for (Map.Entry<VerticalSide, Object> child : children.entrySet()) {
                if (child.getValue().getClass() != V_LINKER_CLASS
                        || V_GET_B_LINKER.invoke(child.getValue()) != b
                        || V_V_SIDE.get(child.getValue()) != child.getKey()) {
                    throw new IllegalStateException("isolated V topology drift");
                }
                childAliases.add(alias + ":v:" + child.getKey());
                childIdentities.add(child.getValue());
            }
            if (children.isEmpty() || children.size() > 2) {
                throw new IllegalStateException("isolated B child cardinality drift");
            }
            final BLinkerArenaEntry entry = new BLinkerArenaEntry(
                    b, 0, 0, "-", 0, B_ID.getInt(b), alias, "IsolatedSyntheticCell",
                    b.getClass().getName(), "-", B_IS_ANCHOR.getBoolean(b), "-",
                    point((Point2D) B_REF_PT.get(b)), childAliases, childIdentities,
                    B_LINKED.getBoolean(b), B_CLOSED.getBoolean(b), true);
            final List<String> linked = entry.linked ? List.of(alias) : List.of();
            final List<String> closed = entry.closed ? List.of(alias) : List.of();
            return new BLinkerArenaSnapshot(
                    List.of(entry), sha256Rows(List.of(entry.topologyToken())),
                    sha256Rows(List.of(entry.stateToken())), linked, sha256Rows(linked), closed,
                    0, 0, 0, children.size() == 1 ? 1 : 0,
                    children.size() == 2 ? 1 : 0, 1, false);
        }

        BLinkerArenaEntry targetEntry ()
        {
            for (BLinkerArenaEntry entry : entries) if (entry.target) return entry;
            throw new IllegalStateException("missing target B arena entry");
        }

        void assertOnlyTargetCellChanged (BLinkerArenaSnapshot after,
                                          Object target)
        {
            if (entries.size() != after.entries.size()
                    || !topologyHash.equals(after.topologyHash)
                    || frozenEntries != after.frozenEntries
                    || dynamicAnchors != after.dynamicAnchors
                    || noChild != after.noChild || child1 != after.child1
                    || child2 != after.child2 || targetMatches != after.targetMatches) {
                throw new IllegalStateException("B arena topology changed at flag seam");
            }
            for (int i = 0; i < entries.size(); i++) {
                final BLinkerArenaEntry left = entries.get(i);
                final BLinkerArenaEntry right = after.entries.get(i);
                if (left.identity != right.identity || !left.sameTopology(right)
                        || left.closed != right.closed) {
                    throw new IllegalStateException("B arena identity/closed state changed");
                }
                if (left.identity == target) {
                    if (!right.linked) throw new IllegalStateException("target B was not linked");
                } else if (left.linked != right.linked) {
                    throw new IllegalStateException("unrelated B linked flag changed");
                }
            }
        }
    }

    private static final class BLinkerArenaEntry
    {
        final Object identity;
        final int arenaOrdinal;
        final int constructorOrdinal;
        final String beamSig;
        final int bOrdinal;
        final int javaId;
        final String alias;
        final String origin;
        final String runtimeClass;
        final String hSide;
        final boolean anchor;
        final String stump;
        final String ref;
        final List<String> childAliases;
        final List<Object> childIdentities;
        final boolean linked;
        final boolean closed;
        final boolean target;

        BLinkerArenaEntry (Object identity,
                           int arenaOrdinal,
                           int constructorOrdinal,
                           String beamSig,
                           int bOrdinal,
                           int javaId,
                           String alias,
                           String origin,
                           String runtimeClass,
                           String hSide,
                           boolean anchor,
                           String stump,
                           String ref,
                           List<String> childAliases,
                           List<Object> childIdentities,
                           boolean linked,
                           boolean closed,
                           boolean target)
        {
            this.identity = identity;
            this.arenaOrdinal = arenaOrdinal;
            this.constructorOrdinal = constructorOrdinal;
            this.beamSig = beamSig;
            this.bOrdinal = bOrdinal;
            this.javaId = javaId;
            this.alias = alias;
            this.origin = origin;
            this.runtimeClass = runtimeClass;
            this.hSide = hSide;
            this.anchor = anchor;
            this.stump = stump;
            this.ref = ref;
            this.childAliases = List.copyOf(childAliases);
            this.childIdentities = List.copyOf(childIdentities);
            this.linked = linked;
            this.closed = closed;
            this.target = target;
        }

        String topologyToken ()
        {
            return arenaOrdinal + ":" + constructorOrdinal + ":" + beamSig + ":" + bOrdinal
                    + ":" + javaId + ":" + alias + ":" + origin + ":" + runtimeClass
                    + ":h=" + hSide + ":anchor=" + anchor + ":stump=" + stump
                    + ":ref=" + ref + ":children=" + list(childAliases) + ":target=" + target;
        }

        String stateToken ()
        {
            return topologyToken() + ":linked=" + linked + ":closed=" + closed;
        }

        boolean sameTopology (BLinkerArenaEntry that)
        {
            if (!topologyToken().equals(that.topologyToken())
                    || childIdentities.size() != that.childIdentities.size()) {
                return false;
            }
            for (int i = 0; i < childIdentities.size(); i++) {
                if (childIdentities.get(i) != that.childIdentities.get(i)) return false;
            }
            return true;
        }
    }

    private static final class BLinkerObservation
    {
        final String role;
        final int ordinal;
        final String alias;
        final String vSide;
        final boolean current;
        final boolean sameCell;
        final boolean linked;
        final boolean closed;
        final boolean ownsCell;

        BLinkerObservation (String role,
                            int ordinal,
                            String alias,
                            String vSide,
                            boolean current,
                            boolean sameCell,
                            boolean linked,
                            boolean closed,
                            boolean ownsCell)
        {
            this.role = role;
            this.ordinal = ordinal;
            this.alias = alias;
            this.vSide = vSide;
            this.current = current;
            this.sameCell = sameCell;
            this.linked = linked;
            this.closed = closed;
            this.ownsCell = ownsCell;
        }

        boolean sameIdentity (BLinkerObservation that)
        {
            return role.equals(that.role) && ordinal == that.ordinal
                    && alias.equals(that.alias) && vSide.equals(that.vSide)
                    && current == that.current && sameCell == that.sameCell
                    && ownsCell == that.ownsCell;
        }
    }

    private enum FailureMode
    {
        NONE,
        VERTEX_LISTENER,
        EARLY_EDGE_LISTENER,
        LATER_EDGE_LISTENER
    }

    private record BaseApplySyntheticFixture(Book book,
                                             SheetStub stub,
                                             Sheet sheet,
                                             SystemInfo system,
                                             SIGraph sig,
                                             AbstractBeamInter beam,
                                             StemInter stem,
                                             VerticalSide headToBeam,
                                             int profile,
                                             Link link)
    {
    }

    private record IsolatedBCell(BeamLinker outer,
                                 BeamLinker.BLinker b,
                                 Object currentV,
                                 String alias)
    {
    }

    private record SyntheticHeadSpec(String name,
                                     Shape shape,
                                     boolean duplicate,
                                     boolean nullFallback,
                                     boolean manual,
                                     boolean chordRewire,
                                     boolean preInsertThrow,
                                     boolean callbackThrow)
    {
    }

    private record SyntheticHeadEvent(String kind,
                                      String relationIdentity)
    {
    }

    private record IsolatedHeadFixture(Book book,
                                       SheetStub stub,
                                       Sheet sheet,
                                       SystemInfo system,
                                       SIGraph sig,
                                       Scale scale,
                                       Staff staff,
                                       HeadInter head,
                                       HeadLinker linker,
                                       HeadLinker.SLinker.CLinker corner,
                                       HeadLinker.SLinker side,
                                       StemInter stem,
                                       HeadStemRelation draft,
                                       Relation planRelation,
                                       HeadStemRelation existing,
                                       HeadChordInter chord,
                                       StemInter oldStem,
                                       Relation oldChordStem)
    {
    }

    private static final class ThrowingCheckHeadInter
            extends HeadInter
    {
        private static final long serialVersionUID = 1L;

        ThrowingCheckHeadInter (Rectangle bounds,
                                Shape shape,
                                Double grade,
                                Staff staff,
                                Double pitch)
        {
            super(bounds, shape, grade, staff, pitch);
        }

        @Override
        public boolean checkAbnormal ()
        {
            throw new SyntheticHeadCallbackException(
                    "synthetic HeadStem callback checkAbnormal failure");
        }
    }

    private static final class SyntheticHeadCallbackException
            extends RuntimeException
    {
        private static final long serialVersionUID = 1L;

        SyntheticHeadCallbackException (String message)
        {
            super(message);
        }
    }

    private record SyntheticSiblingSpec(String name,
                                        String runtime,
                                        boolean sameGlyph,
                                        boolean existingBeamStem,
                                        boolean shorterWrongSide,
                                        boolean includeLinker,
                                        boolean initiallyLinked,
                                        boolean throwBeforeInsertion,
                                        boolean throwDuringCallback)
    {
    }

    private record IsolatedSiblingFixture(Book book,
                                          SheetStub stub,
                                          Sheet sheet,
                                          SystemInfo system,
                                          SIGraph sig,
                                          BeamGroupInter group,
                                          AbstractBeamInter base,
                                          AbstractBeamInter sibling,
                                          StemInter stem,
                                          BeamLinker baseOuter,
                                          BeamLinker.BLinker baseB,
                                          Object currentV,
                                          BeamLinker siblingOuter,
                                          BeamLinker.BLinker siblingB,
                                          StemBuilder builder,
                                          int builderItems)
    {
    }

    private enum ThrowEvent
    {
        VERTEX_ADDED,
        EDGE_ADDED
    }

    /** Named listener so topology rows never depend on an anonymous-class ordinal. */
    private static final class ThrowingGraphListener
            implements GraphListener<Inter, Relation>
    {
        final ThrowEvent event;

        ThrowingGraphListener (ThrowEvent event)
        {
            this.event = event;
        }

        @Override
        public void edgeAdded (GraphEdgeChangeEvent<Inter, Relation> event)
        {
            if (this.event == ThrowEvent.EDGE_ADDED) {
                throw new SyntheticListenerException("synthetic edgeAdded listener failure");
            }
        }

        @Override
        public void edgeRemoved (GraphEdgeChangeEvent<Inter, Relation> event)
        {
        }

        @Override
        public void vertexAdded (GraphVertexChangeEvent<Inter> event)
        {
            if (this.event == ThrowEvent.VERTEX_ADDED) {
                throw new SyntheticListenerException("synthetic vertexAdded listener failure");
            }
        }

        @Override
        public void vertexRemoved (GraphVertexChangeEvent<Inter> event)
        {
        }
    }

    private static final class SyntheticListenerException
            extends RuntimeException
    {
        private static final long serialVersionUID = 1L;

        SyntheticListenerException (String message)
        {
            super(message);
        }
    }

    private record DirtyFlags(boolean stubModified,
                              boolean bookModified,
                              boolean bookDirty)
    {
        static DirtyFlags capture (Sheet sheet)
        {
            final SheetStub stub = sheet.getStub();
            final Book book = stub.getBook();
            return new DirtyFlags(
                    stub.isModified(), (Boolean) get(BOOK_MODIFIED, book), book.isDirty());
        }

        String token ()
        {
            return stubModified + ":" + bookModified + ":" + bookDirty;
        }
    }

    private record ListenerTopology(List<String> vertexSet,
                                    List<String> graph)
    {
        static ListenerTopology capture (SIGraph sig)
        {
            final List<String> vertex = new ArrayList<>();
            for (Object listener : (List<?>) get(VERTEX_SET_LISTENERS, sig)) {
                vertex.add(listener.getClass().getName());
            }
            final List<String> graph = new ArrayList<>();
            for (Object listener : (List<?>) get(GRAPH_LISTENERS, sig)) {
                graph.add(listener.getClass().getName());
            }
            return new ListenerTopology(List.copyOf(vertex), List.copyOf(graph));
        }

        boolean isStandard ()
        {
            return vertexSet.isEmpty()
                    && graph.equals(List.of("org.audiveris.omr.sig.SigListener"));
        }

        String hash ()
        {
            return sha256Rows(List.of("vertex=" + list(vertexSet), "graph=" + list(graph)));
        }
    }

    private static final class GraphOrder
    {
        final List<Inter> vertices;
        final List<Relation> edges;
        final IdentityHashMap<Inter, Integer> vertexOrdinals = new IdentityHashMap<>();
        final IdentityHashMap<Relation, Integer> edgeOrdinals = new IdentityHashMap<>();
        final String vertexHash;
        final String edgeHash;

        GraphOrder (SIGraph sig)
        {
            vertices = new ArrayList<>(sig.vertexSet());
            edges = new ArrayList<>(sig.edgeSet());
            final List<String> vertexRows = new ArrayList<>();
            for (int ordinal = 0; ordinal < vertices.size(); ordinal++) {
                final Inter inter = vertices.get(ordinal);
                vertexOrdinals.put(inter, ordinal);
                vertexRows.add(ordinal + ":" + interStructuralToken(inter));
            }
            final List<String> edgeRows = new ArrayList<>();
            for (int ordinal = 0; ordinal < edges.size(); ordinal++) {
                final Relation relation = edges.get(ordinal);
                edgeOrdinals.put(relation, ordinal);
                edgeRows.add(ordinal + ":source="
                        + vertexOrdinals.get(sig.getEdgeSource(relation)) + ":target="
                        + vertexOrdinals.get(sig.getEdgeTarget(relation)) + ":"
                        + relationState(relation));
            }
            vertexHash = sha256Rows(vertexRows);
            edgeHash = sha256Rows(edgeRows);
        }

        String edgeAlias (Relation relation)
        {
            final Integer ordinal = edgeOrdinals.get(relation);
            return ordinal != null ? "sig-edge:" + ordinal : "-";
        }

        String vertexOrdinal (Inter inter)
        {
            final Integer ordinal = vertexOrdinals.get(inter);
            return ordinal != null ? Integer.toString(ordinal) : "-";
        }


        int vertexObjectMatches (Inter inter)
        {
            return vertexOrdinals.containsKey(inter) ? 1 : 0;
        }

        boolean same (GraphOrder that)
        {
            if (vertices.size() != that.vertices.size() || edges.size() != that.edges.size()
                    || !vertexHash.equals(that.vertexHash) || !edgeHash.equals(that.edgeHash)) {
                return false;
            }
            for (int i = 0; i < vertices.size(); i++) {
                if (vertices.get(i) != that.vertices.get(i)) return false;
            }
            for (int i = 0; i < edges.size(); i++) {
                if (edges.get(i) != that.edges.get(i)) return false;
            }
            return true;
        }
    }

    private static final class BaseStemState
    {
        final StemInter stem;
        final SIGraph sig;
        final Object area;
        final Glyph glyph;
        final GradeImpacts impacts;
        final Object staff;
        final Shape shape;
        final int id;
        final boolean vip;
        final boolean removed;
        final boolean abnormal;
        final String geometryValue;
        final String value;

        BaseStemState (StemInter stem)
        {
            this.stem = stem;
            sig = stem.getSig();
            area = get(INTER_AREA, stem);
            glyph = stem.getGlyph();
            impacts = stem.getImpacts();
            staff = get(INTER_STAFF, stem);
            shape = stem.getShape();
            id = stem.getId();
            vip = stem.isVip();
            removed = stem.isRemoved();
            abnormal = stem.isAbnormal();
            geometryValue = geometryToken(stem);
            value = StemInterState.valueOf(stem) + ":vip=" + stem.isVip()
                    + ":sig=" + (sig != null ? sig.getSystem().getId() : "-");
        }

        boolean sameExceptIdSigAbnormalVip (BaseStemState that)
        {
            if (stem != that.stem || area != that.area || glyph != that.glyph
                    || impacts != that.impacts || staff != that.staff || shape != that.shape) {
                return false;
            }
            return geometryValue.equals(that.geometryValue);
        }

        boolean same (BaseStemState that)
        {
            return stem == that.stem && sig == that.sig && area == that.area
                    && glyph == that.glyph && impacts == that.impacts && staff == that.staff
                    && shape == that.shape && id == that.id && vip == that.vip
                    && removed == that.removed && abnormal == that.abnormal
                    && geometryValue.equals(that.geometryValue) && value.equals(that.value);
        }

        boolean sameExceptAbnormal (BaseStemState that)
        {
            return stem == that.stem && sig == that.sig && area == that.area
                    && glyph == that.glyph && impacts == that.impacts && staff == that.staff
                    && shape == that.shape && id == that.id && vip == that.vip
                    && removed == that.removed && geometryValue.equals(that.geometryValue);
        }

        static String geometryToken (StemInter stem)
        {
            return stem.getShape() + ":" + hex(stem.getGrade()) + ":"
                    + impactsToken(stem.getImpacts()) + ":"
                    + line(stem.getMedian()) + ":" + hex(stem.getWidth()) + ":"
                    + rectangle(stem.getBounds()) + ":" + stem.isManual() + ":"
                    + stem.isImplicit() + ":" + stem.getProfile();
        }
    }

    private static final class BeamObjectState
    {
        final AbstractBeamInter beam;
        final SIGraph sig;
        final Object area;
        final Object group;
        final Line2D medianIdentity;
        final Glyph glyph;
        final GradeImpacts impacts;
        final Object staff;
        final Shape shape;
        final String stableValue;
        final boolean abnormal;

        BeamObjectState (AbstractBeamInter beam)
        {
            this.beam = beam;
            sig = beam.getSig();
            area = get(INTER_AREA, beam);
            group = beam.getGroup();
            medianIdentity = beam.getMedian();
            glyph = beam.getGlyph();
            impacts = beam.getImpacts();
            staff = get(INTER_STAFF, beam);
            shape = beam.getShape();
            stableValue = beam.getClass().getName() + ":id=" + beam.getId()
                    + ":shape=" + beam.getShape() + ":grade="
                    + hex(beam.getGrade()) + ":impacts=" + impactsToken(beam.getImpacts())
                    + ":median=" + line(beam.getMedian()) + ":height=" + hex(beam.getHeight())
                    + ":bounds=" + rectangle(beam.getBounds()) + ":removed="
                    + beam.isRemoved() + ":vip=" + beam.isVip() + ":manual="
                    + beam.isManual() + ":implicit=" + beam.isImplicit() + ":profile="
                    + beam.getProfile() + ":group="
                    + (group != null ? ((Inter) group).getId() : "-");
            abnormal = beam.isAbnormal();
        }

        boolean sameExceptAbnormal (BeamObjectState that)
        {
            return beam == that.beam && sig == that.sig && area == that.area
                    && group == that.group && medianIdentity == that.medianIdentity
                    && glyph == that.glyph && impacts == that.impacts && staff == that.staff
                    && shape == that.shape
                    && stableValue.equals(that.stableValue);
        }

        boolean same (BeamObjectState that)
        {
            return sameExceptAbnormal(that) && abnormal == that.abnormal;
        }
    }

    private static final class ApplyMoment
    {
        final int allocator;
        final InterRegistrySnapshot inters;
        final GlyphRegistrySnapshot glyphs;
        final GraphOrder graph;
        final DirtyFlags dirty;
        final ListenerTopology listeners;
        final BaseStemState stem;
        final BeamObjectState beam;

        ApplyMoment (Sheet sheet,
                     SIGraph sig,
                     AbstractBeamInter beam,
                     StemInter stem)
            throws Exception
        {
            allocator = sheet.getPersistentIdGenerator().get();
            inters = new InterRegistrySnapshot(sheet);
            glyphs = new GlyphRegistrySnapshot(sheet.getGlyphIndex());
            graph = new GraphOrder(sig);
            dirty = DirtyFlags.capture(sheet);
            listeners = ListenerTopology.capture(sig);
            this.stem = new BaseStemState(stem);
            this.beam = new BeamObjectState(beam);
        }

        boolean same (ApplyMoment that)
        {
            return allocator == that.allocator && inters.same(that.inters)
                    && glyphs.sameIdentityState(that.glyphs) && graph.same(that.graph)
                    && dirty.equals(that.dirty) && listeners.equals(that.listeners)
                    && stem.same(that.stem) && beam.same(that.beam);
        }
    }

    private record DuplicateRow(int pairOrdinal,
                                int sourceOutgoingOrdinal,
                                Relation relation,
                                String globalIdentity,
                                boolean classCheckRead,
                                boolean matches,
                                String action)
    {
    }

    private static final class DuplicateLookup
    {
        final String state;
        final boolean sourceRemovedRead;
        final boolean sourceRemoved;
        final boolean targetRemovedRead;
        final String targetRemoved;
        final int sourceOutgoingScanned;
        final String sourceOutgoingHash;
        final String pairHash;
        final List<DuplicateRow> rows;
        final Relation firstMatch;
        final String action;

        DuplicateLookup (String state,
                         boolean sourceRemovedRead,
                         boolean sourceRemoved,
                         boolean targetRemovedRead,
                         String targetRemoved,
                         int sourceOutgoingScanned,
                         String sourceOutgoingHash,
                         String pairHash,
                         List<DuplicateRow> rows,
                         Relation firstMatch,
                         String action)
        {
            this.state = state;
            this.sourceRemovedRead = sourceRemovedRead;
            this.sourceRemoved = sourceRemoved;
            this.targetRemovedRead = targetRemovedRead;
            this.targetRemoved = targetRemoved;
            this.sourceOutgoingScanned = sourceOutgoingScanned;
            this.sourceOutgoingHash = sourceOutgoingHash;
            this.pairHash = pairHash;
            this.rows = rows;
            this.firstMatch = firstMatch;
            this.action = action;
        }

        static DuplicateLookup capture (SIGraph sig,
                                        Inter source,
                                        Inter target,
                                        Class<?> relationClass,
                                        GraphOrder before)
        {
            final boolean sourceRemoved = source.isRemoved();
            if (sourceRemoved) {
                return new DuplicateLookup(
                        "NotRead", true, true, false, "-", 0, "NotRead", "NotRead",
                        List.of(), null,
                        "SuppressSourceRemoved");
            }
            final boolean targetRemoved = target.isRemoved();
            if (targetRemoved) {
                return new DuplicateLookup(
                        "NotRead", true, false, true, "true", 0, "NotRead", "NotRead",
                        List.of(), null,
                        "SuppressTargetRemoved");
            }
            if (sig == null || !sig.containsVertex(source) || !sig.containsVertex(target)) {
                return new DuplicateLookup(
                        "MissingEndpoint", true, false, true, "false", 0,
                        "NotRead", "NotRead", List.of(), null,
                        "Add");
            }
            final List<Relation> outgoing = new ArrayList<>(sig.outgoingEdgesOf(source));
            final List<String> outgoingTokens = new ArrayList<>();
            for (Relation relation : outgoing) {
                outgoingTokens.add(before.edgeAlias(relation) + ":"
                        + relation.getClass().getName());
            }
            final List<Relation> pair = new ArrayList<>();
            final List<Integer> outgoingOrdinals = new ArrayList<>();
            for (int ordinal = 0; ordinal < outgoing.size(); ordinal++) {
                final Relation relation = outgoing.get(ordinal);
                if (sig.getEdgeTarget(relation) == target) {
                    pair.add(relation);
                    outgoingOrdinals.add(ordinal);
                }
            }
            Relation first = null;
            int breakAt = -1;
            for (int ordinal = 0; ordinal < pair.size(); ordinal++) {
                if (relationClass.isInstance(pair.get(ordinal))) {
                    first = pair.get(ordinal);
                    breakAt = ordinal;
                    break;
                }
            }
            final List<DuplicateRow> rows = new ArrayList<>();
            final List<String> pairTokens = new ArrayList<>();
            for (int ordinal = 0; ordinal < pair.size(); ordinal++) {
                final Relation relation = pair.get(ordinal);
                final boolean read = breakAt < 0 || ordinal <= breakAt;
                final boolean matches = read && relationClass.isInstance(relation);
                rows.add(new DuplicateRow(
                        ordinal, outgoingOrdinals.get(ordinal), relation,
                        before.edgeAlias(relation), read, matches,
                        !read ? "UnreadAfterBreak" : matches ? "SelectBreak" : "Continue"));
                pairTokens.add(ordinal + ":" + outgoingOrdinals.get(ordinal) + ":"
                        + before.edgeAlias(relation) + ":" + relation.getClass().getName());
            }
            return new DuplicateLookup(
                    "ExhaustiveOutgoingThenLazyPairClass", true, false, true, "false",
                    outgoing.size(), sha256Rows(outgoingTokens), sha256Rows(pairTokens),
                    List.copyOf(rows), first,
                    first != null ? "SuppressExistingRelation" : "Add");
        }
    }

    private record IncidentRow(int incidentOrdinal,
                               String direction,
                               int directionOrdinal,
                               Relation relation,
                               String globalIdentity,
                               Inter opposite,
                               String oppositeVertexOrdinal,
                               boolean chordStemMatch)
    {
    }

    private static final class StemIncidentScan
    {
        final String state;
        final List<IncidentRow> rows;
        final int chordMatches;
        final String hash;

        StemIncidentScan (String state,
                          List<IncidentRow> rows)
        {
            this.state = state;
            this.rows = List.copyOf(rows);
            int matches = 0;
            final List<String> tokens = new ArrayList<>();
            for (IncidentRow row : rows) {
                if (row.chordStemMatch) matches++;
                tokens.add(row.incidentOrdinal + ":" + row.direction + ":"
                        + row.directionOrdinal + ":" + row.globalIdentity + ":"
                        + row.relation.getClass().getName() + ":" + row.opposite.getId()
                        + ":" + row.oppositeVertexOrdinal + ":" + row.chordStemMatch);
            }
            chordMatches = matches;
            hash = sha256Rows(tokens);
        }

        static StemIncidentScan notRead ()
        {
            return new StemIncidentScan("NotRead", List.of());
        }

        static StemIncidentScan captureIfPresent (SIGraph sig,
                                                   StemInter stem,
                                                   GraphOrder graph)
        {
            return sig.containsVertex(stem)
                    ? capture(sig, stem, graph)
                    : new StemIncidentScan("MissingVertex", List.of());
        }

        static StemIncidentScan capture (SIGraph sig,
                                         StemInter stem,
                                         GraphOrder graph)
        {
            final List<IncidentRow> rows = new ArrayList<>();
            int incident = 0;
            int direction = 0;
            for (Relation relation : sig.incomingEdgesOf(stem)) {
                final Inter opposite = sig.getOppositeInter(stem, relation);
                rows.add(new IncidentRow(
                        incident++, "Incoming", direction++, relation,
                        graph.edgeAlias(relation), opposite, graph.vertexOrdinal(opposite),
                        relation instanceof ChordStemRelation));
            }
            direction = 0;
            for (Relation relation : sig.outgoingEdgesOf(stem)) {
                if (sig.getEdgeSource(relation) == stem && sig.getEdgeTarget(relation) == stem) {
                    continue;
                }
                rows.add(new IncidentRow(
                        incident++, "Outgoing", direction++, relation,
                        graph.edgeAlias(relation), sig.getOppositeInter(stem, relation),
                        graph.vertexOrdinal(sig.getOppositeInter(stem, relation)),
                        relation instanceof ChordStemRelation));
            }
            return new StemIncidentScan("ExhaustiveIncomingThenOutgoing", rows);
        }

        static StemIncidentScan capturePrefix (SIGraph sig,
                                               StemInter stem,
                                               GraphOrder graphAfter,
                                               GraphOrder graphBefore,
                                               IdentityHashMap<Relation, Boolean> acceptedPrefix)
        {
            final List<IncidentRow> rows = new ArrayList<>();
            int incident = 0;
            int directionOrdinal = 0;
            for (Relation relation : sig.incomingEdgesOf(stem)) {
                if (!graphBefore.edgeOrdinals.containsKey(relation)
                        && !acceptedPrefix.containsKey(relation)) {
                    continue;
                }
                final Inter opposite = sig.getOppositeInter(stem, relation);
                rows.add(new IncidentRow(
                        incident++, "Incoming", directionOrdinal++, relation,
                        graphAfter.edgeAlias(relation), opposite,
                        graphAfter.vertexOrdinal(opposite),
                        relation instanceof ChordStemRelation));
            }
            directionOrdinal = 0;
            for (Relation relation : sig.outgoingEdgesOf(stem)) {
                if (!graphBefore.edgeOrdinals.containsKey(relation)
                        && !acceptedPrefix.containsKey(relation)) {
                    continue;
                }
                if (sig.getEdgeSource(relation) == stem && sig.getEdgeTarget(relation) == stem) {
                    continue;
                }
                final Inter opposite = sig.getOppositeInter(stem, relation);
                rows.add(new IncidentRow(
                        incident++, "Outgoing", directionOrdinal++, relation,
                        graphAfter.edgeAlias(relation), opposite,
                        graphAfter.vertexOrdinal(opposite),
                        relation instanceof ChordStemRelation));
            }
            return new StemIncidentScan("ExhaustiveIncomingThenOutgoingAtCallback", rows);
        }
    }

    private record BeamIncidentRow(int incidentOrdinal,
                                   String direction,
                                   int directionOrdinal,
                                   Relation relation,
                                   String globalIdentity,
                                   Inter opposite,
                                   String oppositeVertexOrdinal,
                                   String readState,
                                   boolean relevant,
                                   String portion,
                                   String contribution)
    {
    }

    private static final class BeamIncidentTrace
    {
        final String state;
        final String rule;
        final List<BeamIncidentRow> rows;
        final String left;
        final String right;
        final boolean requestedAbnormal;
        final String hash;

        BeamIncidentTrace (String state,
                           String rule,
                           List<BeamIncidentRow> rows,
                           String left,
                           String right,
                           boolean requestedAbnormal)
        {
            this.state = state;
            this.rule = rule;
            this.rows = List.copyOf(rows);
            this.left = left;
            this.right = right;
            this.requestedAbnormal = requestedAbnormal;
            final List<String> tokens = new ArrayList<>();
            for (BeamIncidentRow row : rows) {
                tokens.add(row.incidentOrdinal + ":" + row.direction + ":"
                        + row.directionOrdinal + ":" + row.globalIdentity + ":"
                        + row.relation.getClass().getName() + ":" + row.readState + ":"
                        + row.opposite.getId() + ":" + row.oppositeVertexOrdinal + ":"
                        + row.relevant + ":" + row.portion + ":" + row.contribution);
            }
            hash = sha256Rows(tokens);
        }

        static BeamIncidentTrace notRead ()
        {
            return new BeamIncidentTrace("NotRead", "NotRead", List.of(), "-", "-", false);
        }

        static BeamIncidentTrace captureIfPresent (SIGraph sig,
                                                   AbstractBeamInter beam,
                                                   GraphOrder graph)
        {
            return sig.containsVertex(beam)
                    ? capture(sig, beam, graph)
                    : new BeamIncidentTrace(
                            "MissingVertex", "NotRead", List.of(), "-", "-", false);
        }

        static BeamIncidentTrace capture (SIGraph sig,
                                          AbstractBeamInter beam,
                                          GraphOrder graph)
        {
            final List<Relation> incident = new ArrayList<>();
            final List<String> directions = new ArrayList<>();
            final List<Integer> directionOrdinals = new ArrayList<>();
            int ordinal = 0;
            for (Relation relation : sig.incomingEdgesOf(beam)) {
                incident.add(relation);
                directions.add("Incoming");
                directionOrdinals.add(ordinal++);
            }
            ordinal = 0;
            for (Relation relation : sig.outgoingEdgesOf(beam)) {
                if (sig.getEdgeSource(relation) == beam && sig.getEdgeTarget(relation) == beam) {
                    continue;
                }
                incident.add(relation);
                directions.add("Outgoing");
                directionOrdinals.add(ordinal++);
            }
            final List<BeamIncidentRow> rows = new ArrayList<>();
            if (beam instanceof BeamHookInter) {
                boolean found = false;
                for (int i = 0; i < incident.size(); i++) {
                    final Relation relation = incident.get(i);
                    final Inter opposite = sig.getOppositeInter(beam, relation);
                    final boolean read = !found;
                    final boolean relevant = read && relation instanceof BeamStemRelation;
                    if (relevant) found = true;
                    rows.add(new BeamIncidentRow(
                            i, directions.get(i), directionOrdinals.get(i), relation,
                            graph.edgeAlias(relation), opposite, graph.vertexOrdinal(opposite),
                            read ? "ExaminedClassOnly" : "UnreadAfterBreak",
                            relevant, "-",
                            relevant ? "FirstBeamStem" : "None"));
                }
                return new BeamIncidentTrace(
                        "LazyIncomingThenOutgoing", "HookHasAnyBeamStem", rows, "-", "-", !found);
            }
            boolean left = false;
            boolean right = false;
            for (int i = 0; i < incident.size(); i++) {
                final Relation relation = incident.get(i);
                final Inter opposite = sig.getOppositeInter(beam, relation);
                BeamPortion portion = null;
                if (relation instanceof BeamStemRelation beamStem) {
                    portion = beamStem.getBeamPortion();
                } else if (relation instanceof BeamRestRelation beamRest) {
                    portion = beamRest.getBeamPortion();
                }
                final String contribution;
                if (portion == BeamPortion.LEFT) {
                    left = true;
                    contribution = "Left";
                } else if (portion == BeamPortion.RIGHT) {
                    right = true;
                    contribution = "Right";
                } else {
                    contribution = "None";
                }
                rows.add(new BeamIncidentRow(
                        i, directions.get(i), directionOrdinals.get(i), relation,
                        graph.edgeAlias(relation), opposite, graph.vertexOrdinal(opposite),
                        portion != null ? "ExaminedClassAndPortion" : "ExaminedClassOnly",
                        portion != null,
                        portion != null ? portion.toString() : "-", contribution));
            }
            return new BeamIncidentTrace(
                    "ExhaustiveIncomingThenOutgoing", "FullBeamNeedsLeftAndRight", rows,
                    Boolean.toString(left), Boolean.toString(right), !left || !right);
        }
    }

    private static final class ApplyRun
    {
        final String scope;
        final String caseName;
        final AbstractBeamInter beam;
        final String beamAlias;
        final StemInter stem;
        final String stemAlias;
        final Link link;
        final FailureMode failureMode;
        final boolean newVertex;
        final ApplyMoment before;
        final ApplyMoment afterVertex;
        final ApplyMoment after;
        final Boolean vertexReturned;
        final Boolean applyReturned;
        final Throwable thrown;
        final String throwStage;
        final DuplicateLookup duplicate;
        final String relationBefore;
        final String relationAfter;
        final String extensionBefore;
        final String portionBefore;
        final StemIncidentScan stemIncidentsBefore;
        final StemIncidentScan stemIncidents;
        final BeamIncidentTrace beamIncidentsBefore;
        final BeamIncidentTrace beamIncidents;

        ApplyRun (String scope,
                  String caseName,
                  AbstractBeamInter beam,
                  String beamAlias,
                  StemInter stem,
                  String stemAlias,
                  Link link,
                  FailureMode failureMode,
                  boolean newVertex,
                  ApplyMoment before,
                  ApplyMoment afterVertex,
                  ApplyMoment after,
                  Boolean vertexReturned,
                  Boolean applyReturned,
                  Throwable thrown,
                  String throwStage,
                  DuplicateLookup duplicate,
                  String relationBefore,
                  String relationAfter,
                  String extensionBefore,
                  String portionBefore,
                  StemIncidentScan stemIncidentsBefore,
                  StemIncidentScan stemIncidents,
                  BeamIncidentTrace beamIncidentsBefore,
                  BeamIncidentTrace beamIncidents)
        {
            this.scope = scope;
            this.caseName = caseName;
            this.beam = beam;
            this.beamAlias = beamAlias;
            this.stem = stem;
            this.stemAlias = stemAlias;
            this.link = link;
            this.failureMode = failureMode;
            this.newVertex = newVertex;
            this.before = before;
            this.afterVertex = afterVertex;
            this.after = after;
            this.vertexReturned = vertexReturned;
            this.applyReturned = applyReturned;
            this.thrown = thrown;
            this.throwStage = throwStage;
            this.duplicate = duplicate;
            this.relationBefore = relationBefore;
            this.relationAfter = relationAfter;
            this.extensionBefore = extensionBefore;
            this.portionBefore = portionBefore;
            this.stemIncidentsBefore = stemIncidentsBefore;
            this.stemIncidents = stemIncidents;
            this.beamIncidentsBefore = beamIncidentsBefore;
            this.beamIncidents = beamIncidents;
        }
    }

    private static final class CheckTrace
    {
        final VerticalSide vSide;
        final VerticalSide headToBeam;
        final VerticalSide borderSide;
        final Line2D border;
        final int profile;
        final double portionXInP0;
        final double maxDxInput;
        final double maxDxRint;
        final int maxDx;
        final double halfStemWidth;
        final double xMax;
        final double yMax;
        final double xWeight;
        final double yWeight;
        final double intrinsicRatio;
        final double minGrade;
        final double den;
        final double v12;
        final double v34;
        final double xNumerator;
        final double yNumerator;
        final double crossX;
        final double crossY;
        final double leftThreshold;
        final double rightThreshold;
        final BeamPortion portion;
        final double signedXGapPixels;
        final double signedDxFrac;
        final double storedDx;
        final double upperGapRaw;
        final double upperGap;
        final double lowerGapRaw;
        final double lowerGap;
        final double yGapPixels;
        final double storedDy;
        final double xImpactRaw;
        final double yImpactRaw;
        final double xImpact;
        final double yImpact;
        final double xPow;
        final double yPow;
        final double global;
        final double totalWeight;
        final double reciprocalWeight;
        final double root;
        final double grade;
        final Point2D candidateExtension;
        final boolean accepted;
        final BeamStemRelation draft;

        private CheckTrace (VerticalSide vSide,
                            VerticalSide headToBeam,
                            VerticalSide borderSide,
                            Line2D border,
                            int profile,
                            double portionXInP0,
                            double maxDxInput,
                            double maxDxRint,
                            int maxDx,
                            double halfStemWidth,
                            double xMax,
                            double yMax,
                            double xWeight,
                            double yWeight,
                            double intrinsicRatio,
                            double minGrade,
                            double den,
                            double v12,
                            double v34,
                            double xNumerator,
                            double yNumerator,
                            double crossX,
                            double crossY,
                            double leftThreshold,
                            double rightThreshold,
                            BeamPortion portion,
                            double signedXGapPixels,
                            double signedDxFrac,
                            double storedDx,
                            double upperGapRaw,
                            double upperGap,
                            double lowerGapRaw,
                            double lowerGap,
                            double yGapPixels,
                            double storedDy,
                            double xImpactRaw,
                            double yImpactRaw,
                            double xImpact,
                            double yImpact,
                            double xPow,
                            double yPow,
                            double global,
                            double totalWeight,
                            double reciprocalWeight,
                            double root,
                            double grade,
                            Point2D candidateExtension,
                            boolean accepted,
                            BeamStemRelation draft)
        {
            this.vSide = vSide;
            this.headToBeam = headToBeam;
            this.borderSide = borderSide;
            this.border = border;
            this.profile = profile;
            this.portionXInP0 = portionXInP0;
            this.maxDxInput = maxDxInput;
            this.maxDxRint = maxDxRint;
            this.maxDx = maxDx;
            this.halfStemWidth = halfStemWidth;
            this.xMax = xMax;
            this.yMax = yMax;
            this.xWeight = xWeight;
            this.yWeight = yWeight;
            this.intrinsicRatio = intrinsicRatio;
            this.minGrade = minGrade;
            this.den = den;
            this.v12 = v12;
            this.v34 = v34;
            this.xNumerator = xNumerator;
            this.yNumerator = yNumerator;
            this.crossX = crossX;
            this.crossY = crossY;
            this.leftThreshold = leftThreshold;
            this.rightThreshold = rightThreshold;
            this.portion = portion;
            this.signedXGapPixels = signedXGapPixels;
            this.signedDxFrac = signedDxFrac;
            this.storedDx = storedDx;
            this.upperGapRaw = upperGapRaw;
            this.upperGap = upperGap;
            this.lowerGapRaw = lowerGapRaw;
            this.lowerGap = lowerGap;
            this.yGapPixels = yGapPixels;
            this.storedDy = storedDy;
            this.xImpactRaw = xImpactRaw;
            this.yImpactRaw = yImpactRaw;
            this.xImpact = xImpact;
            this.yImpact = yImpact;
            this.xPow = xPow;
            this.yPow = yPow;
            this.global = global;
            this.totalWeight = totalWeight;
            this.reciprocalWeight = reciprocalWeight;
            this.root = root;
            this.grade = grade;
            this.candidateExtension = candidateExtension;
            this.accepted = accepted;
            this.draft = draft;
        }

        static CheckTrace compute (AbstractBeamInter beam,
                                   StemInter stem,
                                   VerticalSide headToBeam,
                                   Scale scale,
                                   int profile)
        {
            final VerticalSide vSide = headToBeam.opposite();
            final VerticalSide borderSide = headToBeam.opposite();
            final Line2D border = beam.getBorder(borderSide);
            final Line2D stemLine = stem.getMedian();
            final double x1 = stemLine.getX1();
            final double y1 = stemLine.getY1();
            final double x2 = stemLine.getX2();
            final double y2 = stemLine.getY2();
            final double x3 = border.getX1();
            final double y3 = border.getY1();
            final double x4 = border.getX2();
            final double y4 = border.getY2();
            final double den = ((x1 - x2) * (y3 - y4))
                    - ((y1 - y2) * (x3 - x4));
            final double v12 = (x1 * y2) - (y1 * x2);
            final double v34 = (x3 * y4) - (y3 * x4);
            final double xNumerator = (v12 * (x3 - x4)) - ((x1 - x2) * v34);
            final double yNumerator = (v12 * (y3 - y4)) - ((y1 - y2) * v34);
            final Point2D cross = LineUtil.intersection(stemLine, border);
            final double crossX = cross.getX();
            final double crossY = cross.getY();
            requireBits(xNumerator / den, crossX, "intersection x");
            requireBits(yNumerator / den, crossY, "intersection y");

            final double portionXInP0 = BeamStemRelation.getXInGapMaximum(0).getValue();
            final double maxDxInput = scale.toPixelsDouble(
                    BeamStemRelation.getXInGapMaximum(0));
            final double maxDxRint = Math.rint(maxDxInput);
            final int maxDx = scale.toPixels(BeamStemRelation.getXInGapMaximum(0));
            if (maxDx != (int) maxDxRint) throw new IllegalStateException("Scale.toPixels drift");
            final double leftThreshold = beam.getMedian().getX1() + maxDx;
            final double rightThreshold = beam.getMedian().getX2() - maxDx;
            final BeamPortion portion = BeamStemRelation.computeBeamPortion(beam, crossX, scale);
            final BeamPortion derivedPortion = crossX < leftThreshold ? BeamPortion.LEFT
                    : crossX > rightThreshold ? BeamPortion.RIGHT : BeamPortion.CENTER;
            if (portion != derivedPortion) throw new IllegalStateException("strict portion drift");

            final Integer stemThickness = scale.getStemThickness();
            if (stemThickness == null) throw new IllegalStateException("missing scale stem thickness");
            final double halfStemWidth = stemThickness / 2.0;
            final double signedXGapPixels = portion == BeamPortion.CENTER ? 0
                    : portion == BeamPortion.LEFT
                            ? border.getX1() + halfStemWidth - crossX
                            : crossX - border.getX2() + halfStemWidth;
            final double upperGapRaw = stemLine.getY1() - crossY;
            final double upperGap = Math.max(0, upperGapRaw);
            final double lowerGapRaw = crossY - stemLine.getY2();
            final double lowerGap = Math.max(0, lowerGapRaw);
            final double yGapPixels = Math.max(upperGap, lowerGap);
            final double signedDxFrac = scale.pixelsToFrac(signedXGapPixels);
            final double storedDx = Math.abs(signedDxFrac);
            final double storedDy = scale.pixelsToFrac(yGapPixels);

            final BeamStemRelation draft = new BeamStemRelation();
            draft.setBeamPortion(portion);
            draft.setOutGaps(signedDxFrac, storedDy, profile);
            requireBits(storedDx, draft.getDx(), "draft dx");
            requireBits(storedDy, draft.getDy(), "draft dy");
            final GradeImpacts impacts = draft.getImpacts();
            if (impacts == null || impacts.getImpactCount() != 2
                    || !"xOutGap".equals(impacts.getName(0))
                    || !"yGap".equals(impacts.getName(1))) {
                throw new IllegalStateException("unexpected BeamStemRelation impacts");
            }
            final double[] outWeights = (double[]) get(BEAM_OUT_WEIGHTS, null);
            final double xWeight = outWeights[0];
            final double yWeight = outWeights[1];
            requireBits(xWeight, impacts.getWeight(0), "x weight");
            requireBits(yWeight, impacts.getWeight(1), "y weight");
            final double xMax = BeamStemRelation.getXOutGapMaximum(profile).getValue();
            final double yMax = BeamStemRelation.getYGapMaximum(profile).getValue();
            final double xImpactRaw = (xMax - storedDx) / xMax;
            final double yImpactRaw = (yMax - storedDy) / yMax;
            final double xImpact = clampImpact(xImpactRaw);
            final double yImpact = clampImpact(yImpactRaw);
            requireBits(xImpact, impacts.getImpact(0), "x impact");
            requireBits(yImpact, impacts.getImpact(1), "y impact");
            final double xPow = xImpact == 0 ? 0 : Math.pow(xImpact, xWeight);
            final double yPow = yImpact == 0 ? 0 : Math.pow(yImpact, yWeight);
            double global = 1d;
            if (xImpact == 0) global = 0;
            else if (xWeight != 0) global *= xPow;
            if (yImpact == 0) global = 0;
            else if (yWeight != 0) global *= yPow;
            final double totalWeight = xWeight + yWeight;
            final double reciprocalWeight = 1 / totalWeight;
            final double root = Math.pow(global, reciprocalWeight);
            final double intrinsicRatio = impacts.getIntrinsicRatio();
            final double grade = intrinsicRatio * root;
            requireBits(grade, draft.getGrade(), "draft grade");
            final double minGrade = draft.getMinGrade();
            final boolean accepted = grade >= minGrade;
            final int yDir = headToBeam == VerticalSide.TOP ? -1 : 1;
            final Point2D extension = new Point2D.Double(
                    crossX, crossY + (yDir * (beam.getHeight() - 1)));
            return new CheckTrace(
                    vSide, headToBeam, borderSide, copy(border), profile, portionXInP0,
                    maxDxInput, maxDxRint, maxDx, halfStemWidth, xMax, yMax, xWeight,
                    yWeight, intrinsicRatio, minGrade, den, v12, v34, xNumerator,
                    yNumerator, crossX, crossY, leftThreshold, rightThreshold, portion,
                    signedXGapPixels, signedDxFrac, storedDx, upperGapRaw, upperGap,
                    lowerGapRaw, lowerGap, yGapPixels, storedDy, xImpactRaw, yImpactRaw,
                    xImpact, yImpact, xPow, yPow, global, totalWeight, reciprocalWeight,
                    root, grade, extension, accepted, draft);
        }

        void assertActual (Link link,
                           StemInter stem)
        {
            if (accepted != (link != null)) {
                throw new IllegalStateException("checkLink acceptance differs from trace");
            }
            if (link == null) return;
            if (link.partner != stem || !link.outgoing
                    || !(link.relation instanceof BeamStemRelation)) {
                throw new IllegalStateException("unexpected checkLink wrapper");
            }
            final BeamStemRelation relation = (BeamStemRelation) link.relation;
            if (relation.getBeamPortion() != portion) {
                throw new IllegalStateException("checkLink portion differs");
            }
            requireBits(storedDx, relation.getDx(), "actual dx");
            requireBits(storedDy, relation.getDy(), "actual dy");
            requireBits(grade, relation.getGrade(), "actual grade");
            final GradeImpacts actual = relation.getImpacts();
            final GradeImpacts expected = draft.getImpacts();
            for (int i = 0; i < expected.getImpactCount(); i++) {
                if (!expected.getName(i).equals(actual.getName(i))) {
                    throw new IllegalStateException("impact name differs");
                }
                requireBits(expected.getImpact(i), actual.getImpact(i), "actual impact");
                requireBits(expected.getWeight(i), actual.getWeight(i), "actual weight");
            }
            requirePointBits(candidateExtension, relation.getExtensionPoint(), "actual extension");
        }
    }

    /** Dense identities for the exact union of live targets touched by examined scans. */
    private static final class LiveStemCatalogue
    {
        final SIGraph sig;
        final String aliasPrefix;
        final List<StemInter> ordered = new ArrayList<>();
        final List<LiveStemRow> rows = new ArrayList<>();
        final IdentityHashMap<StemInter, String> aliases = new IdentityHashMap<>();
        final IdentityHashMap<StemInter, StemInterState> states = new IdentityHashMap<>();
        final IdentityHashMap<StemInter, Integer> sigVertexOrdinals = new IdentityHashMap<>();
        final IdentityHashMap<StemInter, Integer> firstHeadStemEdges = new IdentityHashMap<>();
        final List<StemInter> sigStemOrder = new ArrayList<>();

        private LiveStemCatalogue (SIGraph sig,
                                   String aliasPrefix,
                                   IdentityHashMap<Relation, Integer> sigEdgeOrdinals)
        {
            this.sig = sig;
            this.aliasPrefix = aliasPrefix;
            for (Relation relation : sig.edgeSet()) {
                if (relation instanceof HeadStemRelation) {
                    final StemInter target = (StemInter) sig.getEdgeTarget(relation);
                    final Integer edgeOrdinal = sigEdgeOrdinals.get(relation);
                    if (edgeOrdinal == null) {
                        throw new IllegalStateException("HeadStemRelation lacks SIG edge ordinal");
                    }
                    firstHeadStemEdges.putIfAbsent(target, edgeOrdinal);
                }
            }
            final Set<Integer> interIds = new HashSet<>();
            int sigVertexOrdinal = 0;
            for (Inter inter : sig.vertexSet()) {
                if (inter instanceof StemInter stem) {
                    if (stem.getId() <= 0 || stem.getSig() != sig
                            || !interIds.add(stem.getId())) {
                        throw new IllegalStateException("ambiguous live SIG StemInter identity");
                    }
                    sigStemOrder.add(stem);
                    sigVertexOrdinals.put(stem, sigVertexOrdinal);
                }
                sigVertexOrdinal++;
            }
            for (StemInter target : firstHeadStemEdges.keySet()) {
                if (!sigVertexOrdinals.containsKey(target)) {
                    throw new IllegalStateException("HeadStemRelation target absent from SIG stems");
                }
            }
        }

        static LiveStemCatalogue forExaminedScans (
                SIGraph sig,
                IdentityHashMap<Relation, Integer> sigEdgeOrdinals)
        {
            return new LiveStemCatalogue(sig, "live", sigEdgeOrdinals);
        }

        static LiveStemCatalogue forExaminedScans (
                SIGraph sig,
                IdentityHashMap<Relation, Integer> sigEdgeOrdinals,
                String aliasPrefix)
        {
            return new LiveStemCatalogue(sig, aliasPrefix, sigEdgeOrdinals);
        }

        String intern (StemInter stem)
        {
            final String prior = aliases.get(stem);
            if (prior != null) return prior;
            final Integer vertexOrdinal = sigVertexOrdinals.get(stem);
            if (vertexOrdinal == null || !validPayload(stem)) {
                throw new IllegalStateException("invalid examined live SIG StemInter target");
            }
            final int catalogOrdinal = ordered.size();
            final String alias = aliasPrefix + ":" + catalogOrdinal;
            final Integer firstEdge = firstHeadStemEdges.get(stem);
            final LiveStemRow row = new LiveStemRow(
                    catalogOrdinal, vertexOrdinal, alias, stem,
                    firstEdge != null ? Integer.toString(firstEdge) : "-");
            ordered.add(stem);
            rows.add(row);
            aliases.put(stem, alias);
            states.put(stem, new StemInterState(stem));
            return alias;
        }

        String alias (StemInter stem)
        {
            final String alias = aliases.get(stem);
            if (alias == null) throw new IllegalStateException("stem absent from live SIG catalogue");
            return alias;
        }

        String hash ()
        {
            final List<String> values = new ArrayList<>();
            for (LiveStemRow row : rows) values.add(row.value());
            return sha256Rows(values);
        }

        void assertUnchanged ()
        {
            for (Map.Entry<StemInter, StemInterState> entry : states.entrySet()) {
                if (!entry.getValue().same(entry.getKey())) {
                    throw new IllegalStateException("live SIG stem payload mutated");
                }
            }
        }

        private static boolean validPayload (StemInter stem)
        {
            final Glyph glyph = stem.getGlyph();
            final Line2D median = stem.getMedian();
            final Double width = stem.getWidth();
            final Rectangle bounds = stem.getBounds();
            return stem.getId() > 0 && stem.getSig() != null && glyph != null
                    && glyph.getId() > 0 && median != null && width != null && bounds != null
                    && Double.isFinite(stem.getGrade()) && Double.isFinite(width);
        }
    }

    private record LiveStemRow(int catalogOrdinal,
                               int sigVertexOrdinal,
                               String alias,
                               StemInter stem,
                               String firstHeadStemEdgeOrdinal)
    {
        String value ()
        {
            return catalogOrdinal + ":" + sigVertexOrdinal + ":" + alias + ":"
                    + stem.getId() + ":" + firstHeadStemEdgeOrdinal + ":"
                    + glyphToken(stem.getGlyph()) + ":" + hex(stem.getGrade()) + ":"
                    + line(stem.getMedian()) + ":" + hex(stem.getWidth()) + ":"
                    + rectangle(stem.getBounds()) + ":" + stem.isAbnormal() + ":"
                    + stemStateHash(stem);
        }
    }

    private static final class HeadScan
    {
        final HorizontalSide parentSide;
        final List<HeadStemScanRow> rows;
        final EnumMap<HorizontalSide, LinkedHashSet<StemInter>> expected =
                new EnumMap<>(HorizontalSide.class);
        final String snapshotHash;
        final String projectionHash;

        private HeadScan (HorizontalSide parentSide,
                          List<HeadStemScanRow> rows,
                          String snapshotHash,
                          String projectionHash)
        {
            this.parentSide = parentSide;
            this.rows = rows;
            this.snapshotHash = snapshotHash;
            this.projectionHash = projectionHash;
            for (HeadStemScanRow row : rows) {
                expected.computeIfAbsent(row.edgeHeadSide, ignored -> new LinkedHashSet<>())
                        .add(row.target);
            }
        }

        static HeadScan capture (HeadInter head,
                                 HorizontalSide parentSide,
                                 LiveStemCatalogue liveStems,
                                 IdentityHashMap<Relation, Integer> sigEdgeOrdinals)
        {
            final SIGraph sig = head.getSig();
            if (sig == null) throw new IllegalStateException("scan head is not SIG-attached");
            final List<HeadStemScanRow> rows = new ArrayList<>();
            final List<String> snapshotRows = new ArrayList<>();
            final List<String> projectionRows = new ArrayList<>();
            int scanOrdinal = 0;
            final EnumMap<HorizontalSide, IdentityHashMap<StemInter, Boolean>> distinct =
                    new EnumMap<>(HorizontalSide.class);
            for (Relation relation : sig.getRelations(head, HeadStemRelation.class)) {
                final Integer edgeOrdinal = sigEdgeOrdinals.get(relation);
                if (edgeOrdinal == null) throw new IllegalStateException("scan edge lacks SIG ordinal");
                final HeadStemRelation hsRel = (HeadStemRelation) relation;
                final StemInter target = (StemInter) sig.getEdgeTarget(relation);
                if (target.getId() <= 0 || target.getSig() != sig) {
                    throw new IllegalStateException("live HeadStemRelation target is not persistent");
                }
                final IdentityHashMap<StemInter, Boolean> seen = distinct.computeIfAbsent(
                        hsRel.getHeadSide(), ignored -> new IdentityHashMap<>());
                final boolean inserted = seen.put(target, true) == null;
                final String alias = liveStems.intern(target);
                final boolean matches = hsRel.getHeadSide() == parentSide;
                final String action = matches
                        ? inserted ? "IncludeMatching" : "DuplicateMatching"
                        : "IgnoreOtherSide";
                final HeadStemScanRow row = new HeadStemScanRow(
                        scanOrdinal, edgeOrdinal, hsRel.getHeadSide(), target, alias,
                        matches, inserted, action);
                rows.add(row);
                final String snapshot = edgeOrdinal + ":" + hsRel.getHeadSide() + ":" + alias
                        + ":" + target.getId() + ":sig=true:glyph="
                        + (target.getGlyph() != null ? target.getGlyph().getId() : "-")
                        + ":distinct=" + inserted;
                snapshotRows.add(snapshot);
                projectionRows.add(snapshot + ":parent=" + parentSide + ":matches=" + matches);
                scanOrdinal++;
            }
            return new HeadScan(
                    parentSide, rows, sha256Rows(snapshotRows), sha256Rows(projectionRows));
        }

        void assertMap (Map<HorizontalSide, Set<StemInter>> actual)
        {
            if (!actual.keySet().equals(expected.keySet())) {
                throw new IllegalStateException("HeadInter side-stem keys differ from exhaustive scan");
            }
            for (HorizontalSide side : actual.keySet()) {
                final Set<StemInter> actualSet = actual.get(side);
                final LinkedHashSet<StemInter> expectedSet = expected.get(side);
                if (actualSet == null || actualSet.isEmpty() || expectedSet == null
                        || actualSet.size() != expectedSet.size()) {
                    throw new IllegalStateException("empty/invalid HeadInter side-stem key");
                }
                final Iterator<StemInter> ai = actualSet.iterator();
                final Iterator<StemInter> ei = expectedSet.iterator();
                while (ai.hasNext()) if (ai.next() != ei.next()) {
                    throw new IllegalStateException("HeadInter side-stem insertion order drift");
                }
            }
        }

        int matchingCount ()
        {
            int count = 0;
            for (HeadStemScanRow row : rows) if (row.matchesParentSide) count++;
            return count;
        }

        List<String> sideAliases ()
        {
            final LinkedHashSet<String> aliases = new LinkedHashSet<>();
            for (HeadStemScanRow row : rows) {
                if (row.matchesParentSide) aliases.add(row.targetAlias);
            }
            return new ArrayList<>(aliases);
        }
    }

    private record HeadStemScanRow(int scanOrdinal,
                                   int sigEdgeOrdinal,
                                   HorizontalSide edgeHeadSide,
                                   StemInter target,
                                   String targetAlias,
                                   boolean matchesParentSide,
                                   boolean distinctInsertion,
                                   String action)
    {
    }

    private static final class ReuseEntryTrace
    {
        final int mapOrdinal;
        final String cAlias;
        final String conditionRead;
        final String evidenceValidated;
        final String sLinked;
        final String parentHSide;
        final String parentVSide;
        final String headRef;
        final String headInterId;
        final String relationHeadSide;
        final String sideMismatch;
        final String lookupState;
        final String scanState;
        final String incidentEdges;
        final String matchingEdges;
        final String distinctSideStems;
        final String headSnapshotHash;
        final String projectionHash;
        final String sideStemAliases;
        final String selectedStemAlias;
        final String action;
        final List<HeadStemScanRow> scans;

        private ReuseEntryTrace (int mapOrdinal,
                                 String cAlias,
                                 String conditionRead,
                                 String evidenceValidated,
                                 String sLinked,
                                 String parentHSide,
                                 String parentVSide,
                                 String headRef,
                                 String headInterId,
                                 String relationHeadSide,
                                 String sideMismatch,
                                 String lookupState,
                                 String scanState,
                                 String incidentEdges,
                                 String matchingEdges,
                                 String distinctSideStems,
                                 String headSnapshotHash,
                                 String projectionHash,
                                 String sideStemAliases,
                                 String selectedStemAlias,
                                 String action,
                                 List<HeadStemScanRow> scans)
        {
            this.mapOrdinal = mapOrdinal;
            this.cAlias = cAlias;
            this.conditionRead = conditionRead;
            this.evidenceValidated = evidenceValidated;
            this.sLinked = sLinked;
            this.parentHSide = parentHSide;
            this.parentVSide = parentVSide;
            this.headRef = headRef;
            this.headInterId = headInterId;
            this.relationHeadSide = relationHeadSide;
            this.sideMismatch = sideMismatch;
            this.lookupState = lookupState;
            this.scanState = scanState;
            this.incidentEdges = incidentEdges;
            this.matchingEdges = matchingEdges;
            this.distinctSideStems = distinctSideStems;
            this.headSnapshotHash = headSnapshotHash;
            this.projectionHash = projectionHash;
            this.sideStemAliases = sideStemAliases;
            this.selectedStemAlias = selectedStemAlias;
            this.action = action;
            this.scans = scans;
        }

        static ReuseEntryTrace unread (int ordinal,
                                       String alias)
        {
            return new ReuseEntryTrace(
                    ordinal, alias, "false", "false", "-", "-", "-", "-", "-", "-",
                    "-", "NotRead", "NotRead", "-", "-", "-", "-", "-", "-", "-",
                    "UnreadAfterBreak", List.of());
        }

        static ReuseEntryTrace unlinked (int ordinal,
                                         String alias,
                                         HorizontalSide parentSide,
                                         VerticalSide parentVSide,
                                         String ref,
                                         int headId,
                                         HorizontalSide relationSide)
        {
            return new ReuseEntryTrace(
                    ordinal, alias, "true", "true", "false", parentSide.toString(),
                    parentVSide.toString(), ref, Integer.toString(headId), relationSide.toString(),
                    Boolean.toString(parentSide != relationSide), "NotRead", "NotRead", "-", "-",
                    "-", "-", "-", "-", "-", "SkipUnlinked", List.of());
        }

        static ReuseEntryTrace linked (int ordinal,
                                       String alias,
                                       HorizontalSide parentSide,
                                       VerticalSide parentVSide,
                                       String ref,
                                       int headId,
                                       HorizontalSide relationSide,
                                       HeadScan scan,
                                       String lookupState,
                                       String selected,
                                       String action)
        {
            return new ReuseEntryTrace(
                    ordinal, alias, "true", "true", "true", parentSide.toString(),
                    parentVSide.toString(), ref, Integer.toString(headId), relationSide.toString(),
                    Boolean.toString(parentSide != relationSide), lookupState, "Exhaustive",
                    Integer.toString(scan.rows.size()), Integer.toString(scan.matchingCount()),
                    Integer.toString(scan.sideAliases().size()), scan.snapshotHash,
                    scan.projectionHash, list(scan.sideAliases()), selected, action, scan.rows);
        }
    }

    private record ReuseRun(StemInter finalStem,
                            List<ReuseEntryTrace> entries,
                            String selectedMapOrdinal,
                            String selectedC,
                            String breakIndex,
                            int entriesRead,
                            int linkedEntries,
                            int scanCount,
                            String outcome,
                            String terminal)
    {
    }

    private record SyntheticReuseEntry(boolean linked,
                                       HorizontalSide parentSide,
                                       HeadInter head)
    {
    }

    private record SyntheticSigFixture(IsolatedSyntheticSig sig,
                                       HeadInter zeroHead,
                                       HeadInter uniqueHead,
                                       HeadInter unreadHead,
                                       HeadInter multipleHead,
                                       StemInter stemA,
                                       StemInter stemB,
                                       IdentityHashMap<Relation, Integer> edgeOrdinals,
                                       LiveStemCatalogue liveStems)
    {
    }

    /**
     * Before-state certificate computed with the frozen boundary-13 hash algorithms.
     *
     * <p>The boundary-14 structural hashes deliberately omit unrelated Java IDs and use
     * per-SIG source order. They therefore cannot be equated to boundary 13's legacy ID-bearing,
     * sheet-wide hashes. This compatibility snapshot is kept separate from the compact runtime
     * state and exists solely to prove exact predecessor continuity.
     */
    private static final class PredecessorCompatSnapshot
    {
        final String interIndexHash;
        final int sigVertices;
        final int sigEdges;
        final String sigHash;
        final String sigRelationStateHash;
        final int systemStems;
        final String systemStemsHash;
        final String linkerStateHash;

        PredecessorCompatSnapshot (Sheet sheet,
                                   StemsRetriever retriever,
                                   List<StemLinker> linkers)
            throws Exception
        {
            final List<String> interRows = new ArrayList<>();
            final Set<Integer> interIds = new HashSet<>();
            for (Inter inter : sheet.getInterIndex().getEntities()) {
                if (inter == null || inter.getId() <= 0 || !interIds.add(inter.getId())
                        || sheet.getInterIndex().getEntity(inter.getId()) != inter) {
                    throw new IllegalStateException(
                            "incomplete/ambiguous boundary-13 InterIndex compatibility scan");
                }
                interRows.add(inter.getId() + "=" + inter.getClass().getSimpleName()
                        + ":removed=" + inter.isRemoved());
            }
            interRows.sort(Comparator.naturalOrder());
            interIndexHash = sha256Rows(interRows);

            final IdentityHashMap<Inter, Boolean> vertices = new IdentityHashMap<>();
            final IdentityHashMap<Relation, Boolean> edges = new IdentityHashMap<>();
            final List<String> sigRows = new ArrayList<>();
            final List<String> relationRows = new ArrayList<>();
            for (SystemInfo system : sheet.getSystems()) {
                final SIGraph sig = system.getSig();
                for (Inter inter : sig.vertexSet()) {
                    vertices.put(inter, true);
                    sigRows.add("v:" + system.getId() + ":" + inter.getId()
                            + ":" + inter.getClass().getSimpleName());
                }
                for (Relation relation : sig.edgeSet()) {
                    edges.put(relation, true);
                    final String identity = "e:" + system.getId() + ":"
                            + sig.getEdgeSource(relation).getId() + ":"
                            + sig.getEdgeTarget(relation).getId() + ":"
                            + relation.getClass().getSimpleName();
                    sigRows.add(identity);
                    relationRows.add(identity + ":" + relationState(relation));
                }
            }
            sigRows.sort(Comparator.naturalOrder());
            relationRows.sort(Comparator.naturalOrder());
            sigVertices = vertices.size();
            sigEdges = edges.size();
            sigHash = sha256Rows(sigRows);
            sigRelationStateHash = sha256Rows(relationRows);

            final LegacySystemStems legacySystemStems = captureLegacySystemStems(retriever);
            systemStems = legacySystemStems.size();
            systemStemsHash = legacySystemStems.hash();

            final IdentityHashMap<StemLinker, Boolean> linkerIdentities =
                    new IdentityHashMap<>();
            final List<String> linkerRows = new ArrayList<>();
            for (StemLinker linker : linkers) {
                if (linkerIdentities.put(linker, true) != null) {
                    throw new IllegalStateException(
                            "duplicate boundary-13 linker compatibility identity");
                }
                linkerRows.add(linker.getId() + ":" + linker.isLinked()
                        + ":" + linker.isClosed());
            }
            linkerRows.sort(Comparator.naturalOrder());
            linkerStateHash = sha256Rows(linkerRows);
        }

        static LegacySystemStems captureLegacySystemStems (StemsRetriever retriever)
        {
            final Map<Glyph, StemInter> systemStemMap =
                    (Map<Glyph, StemInter>) get(RETRIEVER_SYSTEM_STEMS, retriever);
            final List<String> systemStemRows = new ArrayList<>();
            for (Map.Entry<Glyph, StemInter> entry : systemStemMap.entrySet()) {
                if (entry.getKey() == null || entry.getValue() == null
                        || entry.getValue().getGlyph() != entry.getKey()) {
                    throw new IllegalStateException(
                            "invalid boundary-13 systemStems compatibility entry");
                }
                systemStemRows.add(glyphToken(entry.getKey()) + "=stem:"
                        + entry.getValue().getId() + ":grade="
                        + hex(entry.getValue().getGrade()));
            }
            systemStemRows.sort(Comparator.naturalOrder());
            return new LegacySystemStems(systemStemMap.size(), sha256Rows(systemStemRows));
        }

        private record LegacySystemStems(int size, String hash)
        {
        }

        void assertMatches (Map<String, String> expected,
                            PersistentSnapshot before,
                            String noStaff,
                            String relationInputHash,
                            String executionMode)
        {
            assertCompatField(expected, "executionMode", executionMode);
            assertCompatField(expected, "allocator", Integer.toString(before.allocator));
            assertCompatField(
                    expected, "glyphActive", Integer.toString(before.glyphs.active.size()));
            assertCompatField(
                    expected, "glyphOriginals", Integer.toString(before.glyphs.originals.size()));
            assertCompatField(
                    expected, "interIndex", Integer.toString(before.inters.identities.size()));
            assertCompatField(expected, "glyphActiveHash", before.glyphs.activeHash);
            assertCompatField(expected, "glyphOriginalsHash", before.glyphs.originalsHash);
            assertCompatField(expected, "interIndexHash", interIndexHash);
            assertCompatField(expected, "sigVertices", Integer.toString(sigVertices));
            assertCompatField(expected, "sigEdges", Integer.toString(sigEdges));
            assertCompatField(expected, "sigHash", sigHash);
            assertCompatField(expected, "sigRelationStateHash", sigRelationStateHash);
            assertCompatField(expected, "systemStems", Integer.toString(systemStems));
            assertCompatField(expected, "systemStemsHash", systemStemsHash);
            assertCompatField(expected, "linkerStateHash", linkerStateHash);
            assertCompatField(expected, "lineStateHash", before.lines.hash);
            assertCompatField(expected, "relationInputHash", relationInputHash);
            assertCompatField(expected, "noStaff", noStaff);
        }

        private static void assertCompatField (Map<String, String> expected,
                                               String name,
                                               String actual)
        {
            final String wanted = required(expected, name);
            if (!wanted.equals(actual)) {
                throw new IllegalStateException(
                        "boundary-13 compatibility drift at " + name + ": actual=" + actual
                                + " expected=" + wanted);
            }
        }
    }

    /** SIG whose vertex insertion deliberately bypasses the real sheet InterIndex and allocator. */
    private static final class IsolatedSyntheticSig
            extends SIGraph
    {
        private static final long serialVersionUID = 1L;

        IsolatedSyntheticSig (SystemInfo system)
        {
            super(system);
        }

        @Override
        public boolean addVertex (Inter inter)
        {
            final boolean added = getDelegate().addVertex(inter);
            if (added) inter.setSig(this);
            return added;
        }
    }

    private static PersistentSnapshot snapshot (Sheet sheet,
                                                StemsRetriever retriever,
                                                List<Inter> beams,
                                                List<Inter> heads,
                                                List<StemLinker> linkers)
        throws Exception
    {
        final int allocator = sheet.getPersistentIdGenerator().get();
        if (sheet.getGlyphIndex().getLastId() != allocator
                || sheet.getInterIndex().getLastId() != allocator) {
            throw new IllegalStateException("sheet indexes do not share persistent allocator");
        }
        final GlyphRegistrySnapshot glyphs = new GlyphRegistrySnapshot(sheet.getGlyphIndex());
        final InterRegistrySnapshot inters = new InterRegistrySnapshot(sheet);
        final SigSnapshot sig = new SigSnapshot(sheet);
        final LinkerSnapshot linker = new LinkerSnapshot(linkers);
        final SystemStemsSnapshot systemStems = new SystemStemsSnapshot(retriever);
        final LineSnapshot lines = new LineSnapshot(beams);
        return new PersistentSnapshot(
                allocator, glyphs, inters, sig, linker, systemStems, lines);
    }

    private static final class PersistentSnapshot
    {
        final int allocator;
        final GlyphRegistrySnapshot glyphs;
        final InterRegistrySnapshot inters;
        final SigSnapshot sig;
        final LinkerSnapshot linkers;
        final SystemStemsSnapshot systemStems;
        final LineSnapshot lines;

        PersistentSnapshot (int allocator,
                            GlyphRegistrySnapshot glyphs,
                            InterRegistrySnapshot inters,
                            SigSnapshot sig,
                            LinkerSnapshot linkers,
                            SystemStemsSnapshot systemStems,
                            LineSnapshot lines)
        {
            this.allocator = allocator;
            this.glyphs = glyphs;
            this.inters = inters;
            this.sig = sig;
            this.linkers = linkers;
            this.systemStems = systemStems;
            this.lines = lines;
        }

        void assertOnlyLineChanged (PersistentSnapshot after)
        {
            if (allocator != after.allocator || !glyphs.sameIdentityState(after.glyphs)
                    || !inters.same(after.inters) || !sig.same(after.sig)
                    || !linkers.same(after.linkers) || !systemStems.same(after.systemStems)) {
                throw new IllegalStateException("expand crossed createStem mutation boundary");
            }
            lines.assertAtMostOneChanged(after.lines);
        }

        void assertCreateStemDelta (PersistentSnapshot before,
                                    PersistentSnapshot expanded,
                                    Glyph registered,
                                    String registration,
                                    StemInter existingStem,
                                    StemInter stem,
                                    StemsRetriever retriever)
        {
            if (!inters.same(before.inters) || !sig.same(before.sig)
                    || !linkers.same(before.linkers) || !lines.same(expanded.lines)) {
                throw new IllegalStateException("createStem crossed post-create boundary");
            }
            glyphs.assertAllowedDelta(before.glyphs, registered, registration);
            final int expectedAllocator = before.allocator + (registration.equals("New") ? 1 : 0);
            if (allocator != expectedAllocator) {
                throw new IllegalStateException("unexpected shared allocator delta");
            }
            systemStems.assertAllowedDelta(
                    before.systemStems, registered, existingStem, stem, retriever);
        }

        void assertSame (PersistentSnapshot after)
        {
            if (allocator != after.allocator || !glyphs.sameIdentityState(after.glyphs)
                    || !inters.same(after.inters) || !sig.same(after.sig)
                    || !linkers.same(after.linkers) || !systemStems.same(after.systemStems)
                    || !lines.same(after.lines)) {
                throw new IllegalStateException(
                        "isolated B-linker flag coverage mutated enclosing live state");
            }
        }

        String provenanceHash ()
        {
            return sha256Rows(List.of(
                    "allocator=" + allocator,
                    "glyphActive=" + glyphs.activeHash,
                    "glyphOriginals=" + glyphs.originalsHash,
                    "inters=" + inters.hash,
                    "sig=" + sig.hash,
                    "sigRelations=" + sig.relationStateHash,
                    "linkers=" + linkers.hash,
                    "systemStems=" + systemStems.hash,
                    "lines=" + lines.hash));
        }
    }

    private static final class GlyphRegistrySnapshot
    {
        final IdentityHashMap<Glyph, Boolean> active = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, Boolean> originals = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, String> aliases = new IdentityHashMap<>();
        final String activeHash;
        final String originalsHash;

        GlyphRegistrySnapshot (GlyphIndex index)
            throws Exception
        {
            final List<String> activeRows = new ArrayList<>();
            final Set<Integer> ids = new HashSet<>();
            for (Glyph glyph : index.getEntities()) {
                if (glyph == null || glyph.getId() <= 0 || !ids.add(glyph.getId())
                        || index.getEntity(glyph.getId()) != glyph) {
                    throw new IllegalStateException("incomplete/ambiguous active GlyphIndex");
                }
                active.put(glyph, true);
                activeRows.add(glyphToken(glyph));
            }
            activeRows.sort(Comparator.naturalOrder());
            final Map<WeakGlyph, WeakGlyph> map =
                    (Map<WeakGlyph, WeakGlyph>) GLYPH_ORIGINALS.get(index);
            final List<String> originalRows = new ArrayList<>();
            for (Map.Entry<WeakGlyph, WeakGlyph> entry : map.entrySet()) {
                if (entry.getKey() != entry.getValue()) {
                    throw new IllegalStateException("GlyphIndex originals key/value identity drift");
                }
                final Glyph glyph = entry.getValue().get();
                if (glyph == null || glyph.getId() <= 0 || originals.put(glyph, true) != null) {
                    throw new IllegalStateException("dead/duplicate GlyphIndex original");
                }
                originalRows.add(glyphToken(glyph) + ":active=" + active.containsKey(glyph));
            }
            originalRows.sort(Comparator.naturalOrder());
            final List<Glyph> union = new ArrayList<>(originals.keySet());
            for (Glyph glyph : active.keySet()) {
                if (!originals.containsKey(glyph)) union.add(glyph);
            }
            final Set<Integer> unionIds = new HashSet<>();
            for (Glyph glyph : union) {
                if (!unionIds.add(glyph.getId())) {
                    throw new IllegalStateException("duplicate Java glyph ID in registry union");
                }
                aliases.put(glyph, "glyph:" + glyph.getId());
            }
            activeHash = sha256Rows(activeRows);
            originalsHash = sha256Rows(originalRows);
        }

        String alias (Glyph glyph)
        {
            final String alias = aliases.get(glyph);
            if (alias == null) throw new IllegalStateException("glyph lacks registry alias");
            return alias;
        }

        int equalActiveCount (Glyph candidate)
        {
            int count = 0;
            for (Glyph glyph : active.keySet()) if (glyph.equals(candidate)) count++;
            if (count > 1) throw new IllegalStateException("duplicate equal active glyphs");
            return count;
        }

        int activeIdMatches (int id)
        {
            int count = 0;
            for (Glyph glyph : active.keySet()) if (glyph.getId() == id) count++;
            return count;
        }

        int originalIdMatches (int id)
        {
            int count = 0;
            for (Glyph glyph : originals.keySet()) if (glyph.getId() == id) count++;
            return count;
        }

        Glyph equalOriginal (Glyph candidate)
        {
            Glyph found = null;
            for (Glyph glyph : originals.keySet()) {
                if (glyph.equals(candidate)) {
                    if (found != null) throw new IllegalStateException("duplicate equal originals");
                    found = glyph;
                }
            }
            return found;
        }

        Glyph uniqueEqualOriginal (Glyph candidate)
        {
            final Glyph found = equalOriginal(candidate);
            if (found == null) throw new IllegalStateException("createStem did not register glyph");
            return found;
        }

        boolean sameIdentityState (GlyphRegistrySnapshot that)
        {
            return identitySetEquals(active, that.active)
                    && identitySetEquals(originals, that.originals)
                    && activeHash.equals(that.activeHash) && originalsHash.equals(that.originalsHash);
        }

        void assertAllowedDelta (GlyphRegistrySnapshot before,
                                 Glyph registered,
                                 String registration)
        {
            final IdentityHashMap<Glyph, Boolean> expectedActive = copySet(before.active);
            final IdentityHashMap<Glyph, Boolean> expectedOriginals = copySet(before.originals);
            expectedActive.put(registered, true);
            expectedOriginals.put(registered, true);
            if (!identitySetEquals(active, expectedActive)
                    || !identitySetEquals(originals, expectedOriginals)) {
                throw new IllegalStateException("unmodeled GlyphIndex delta");
            }
            if (registration.equals("New") && before.originals.containsKey(registered)) {
                throw new IllegalStateException("new registration reused old identity");
            }
            if (!registration.equals("New") && !before.originals.containsKey(registered)) {
                throw new IllegalStateException("reuse registration lacks baseline original");
            }
        }
    }

    private static final class InterRegistrySnapshot
    {
        final List<Inter> ordered = new ArrayList<>();
        final IdentityHashMap<Inter, Boolean> identities = new IdentityHashMap<>();
        final String hash;

        InterRegistrySnapshot (Sheet sheet)
        {
            final List<String> rows = new ArrayList<>();
            final Set<Integer> ids = new HashSet<>();
            for (Inter inter : sheet.getInterIndex().getEntities()) {
                if (inter == null || inter.getId() <= 0 || !ids.add(inter.getId())
                        || sheet.getInterIndex().getEntity(inter.getId()) != inter) {
                    throw new IllegalStateException("incomplete/ambiguous InterIndex");
                }
                ordered.add(inter);
                identities.put(inter, true);
                rows.add(interStructuralToken(inter));
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (InterRegistrySnapshot that)
        {
            return identitySetEquals(identities, that.identities) && hash.equals(that.hash);
        }

        int objectMatches (Inter target)
        {
            return identities.containsKey(target) ? 1 : 0;
        }

        String indexOrdinal (Inter target)
        {
            for (int ordinal = 0; ordinal < ordered.size(); ordinal++) {
                if (ordered.get(ordinal) == target) return Integer.toString(ordinal);
            }
            return "-";
        }

        int idMatches (int id)
        {
            int count = 0;
            for (Inter inter : identities.keySet()) if (inter.getId() == id) count++;
            return count;
        }
    }

    private static final class SigSnapshot
    {
        final IdentityHashMap<Inter, Boolean> vertices = new IdentityHashMap<>();
        final IdentityHashMap<Relation, Boolean> edges = new IdentityHashMap<>();
        final IdentityHashMap<Relation, String> relationStates = new IdentityHashMap<>();
        final String hash;
        final String relationStateHash;

        SigSnapshot (Sheet sheet)
        {
            final List<String> rows = new ArrayList<>();
            final List<String> relationRows = new ArrayList<>();
            for (SystemInfo system : sheet.getSystems()) {
                final SIGraph sig = system.getSig();
                final List<Inter> orderedVertices = new ArrayList<>(sig.vertexSet());
                final IdentityHashMap<Inter, Integer> vertexOrdinals = new IdentityHashMap<>();
                for (int ordinal = 0; ordinal < orderedVertices.size(); ordinal++) {
                    final Inter inter = orderedVertices.get(ordinal);
                    vertexOrdinals.put(inter, ordinal);
                    vertices.put(inter, true);
                    rows.add("v:" + system.getId() + ":" + ordinal + ":"
                            + interStructuralToken(inter));
                }
                int edgeOrdinal = 0;
                for (Relation relation : sig.edgeSet()) {
                    edges.put(relation, true);
                    final String identity = "e:" + system.getId() + ":" + edgeOrdinal++
                            + ":source=" + vertexOrdinals.get(sig.getEdgeSource(relation))
                            + ":target=" + vertexOrdinals.get(sig.getEdgeTarget(relation))
                            + ":" + relation.getClass().getSimpleName();
                    rows.add(identity);
                    final String state = relationState(relation);
                    relationStates.put(relation, state);
                    relationRows.add(identity + ":" + state);
                }
            }
            rows.sort(Comparator.naturalOrder());
            relationRows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
            relationStateHash = sha256Rows(relationRows);
        }

        boolean same (SigSnapshot that)
        {
            return identitySetEquals(vertices, that.vertices)
                    && identitySetEquals(edges, that.edges) && hash.equals(that.hash)
                    && relationStateHash.equals(that.relationStateHash);
        }
    }

    private static final class LinkerSnapshot
    {
        final IdentityHashMap<StemLinker, String> state = new IdentityHashMap<>();
        final String hash;

        LinkerSnapshot (List<StemLinker> linkers)
        {
            final List<String> rows = new ArrayList<>();
            for (StemLinker linker : linkers) {
                if (state.put(linker, linker.isLinked() + ":" + linker.isClosed()) != null) {
                    throw new IllegalStateException("duplicate linker snapshot identity");
                }
                rows.add(linker.getClass().getName() + ":linked=" + linker.isLinked()
                        + ":closed=" + linker.isClosed());
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (LinkerSnapshot that)
        {
            if (state.size() != that.state.size()) return false;
            for (Map.Entry<StemLinker, String> entry : state.entrySet()) {
                if (!entry.getValue().equals(that.state.get(entry.getKey()))) return false;
            }
            return true;
        }

        void assertOnlySharedBCellChanged (LinkerSnapshot that,
                                           IdentityHashMap<StemLinker, Boolean> allowed,
                                           boolean linkedBefore)
        {
            if (state.size() != that.state.size()) {
                throw new IllegalStateException("linker identity topology changed");
            }
            for (Map.Entry<StemLinker, String> entry : state.entrySet()) {
                final StemLinker linker = entry.getKey();
                final String before = entry.getValue();
                final String after = that.state.get(linker);
                if (after == null) throw new IllegalStateException("linker identity disappeared");
                if (allowed.containsKey(linker)) {
                    final String expectedBefore = linkedBefore + ":" + linker.isClosed();
                    final String expectedAfter = "true:" + linker.isClosed();
                    if (!before.equals(expectedBefore) || !after.equals(expectedAfter)) {
                        throw new IllegalStateException("shared B/V flag transition drift");
                    }
                } else if (!before.equals(after)) {
                    throw new IllegalStateException("unrelated/head linker flag changed");
                }
            }
        }

        void assertSelectedBCellsAfterTrue (LinkerSnapshot that,
                                            IdentityHashMap<StemLinker, Boolean> allowed)
        {
            if (state.size() != that.state.size()) {
                throw new IllegalStateException("linker identity topology changed");
            }
            for (Map.Entry<StemLinker, String> entry : state.entrySet()) {
                final StemLinker linker = entry.getKey();
                final String before = entry.getValue();
                final String after = that.state.get(linker);
                if (after == null) throw new IllegalStateException("linker identity disappeared");
                if (allowed.containsKey(linker)) {
                    final String[] beforeParts = before.split(":", 2);
                    final String[] afterParts = after.split(":", 2);
                    if (beforeParts.length != 2 || afterParts.length != 2
                            || !"true".equals(afterParts[0])
                            || !beforeParts[1].equals(afterParts[1])) {
                        throw new IllegalStateException("selected B/V shared-cell write drift");
                    }
                } else if (!before.equals(after)) {
                    throw new IllegalStateException("unrelated/head linker flag changed");
                }
            }
        }
    }

    private static final class SystemStemsSnapshot
    {
        final IdentityHashMap<Glyph, StemInter> entries = new IdentityHashMap<>();
        final String hash;

        SystemStemsSnapshot (StemsRetriever retriever)
            throws Exception
        {
            final List<String> rows = new ArrayList<>();
            final Map<Glyph, StemInter> map =
                    (Map<Glyph, StemInter>) RETRIEVER_SYSTEM_STEMS.get(retriever);
            for (Map.Entry<Glyph, StemInter> entry : map.entrySet()) {
                if (entry.getKey() == null || entry.getValue() == null
                        || entry.getValue().getGlyph() != entry.getKey()) {
                    throw new IllegalStateException("invalid systemStems entry");
                }
                entries.put(entry.getKey(), entry.getValue());
                rows.add(glyphToken(entry.getKey()) + "=stem:"
                        + BaseStemState.geometryToken(entry.getValue()));
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (SystemStemsSnapshot that)
        {
            if (entries.size() != that.entries.size() || !hash.equals(that.hash)) return false;
            return sameMapIdentity(that);
        }

        boolean sameMapIdentity (SystemStemsSnapshot that)
        {
            if (entries.size() != that.entries.size()) return false;
            for (Map.Entry<Glyph, StemInter> entry : entries.entrySet()) {
                if (that.entries.get(entry.getKey()) != entry.getValue()) return false;
            }
            return true;
        }

        StemInter equalStem (Glyph candidate)
        {
            StemInter found = null;
            for (Map.Entry<Glyph, StemInter> entry : entries.entrySet()) {
                if (entry.getKey().equals(candidate)) {
                    if (found != null) throw new IllegalStateException("duplicate equal system stems");
                    found = entry.getValue();
                }
            }
            return found;
        }

        void assertAllowedDelta (SystemStemsSnapshot before,
                                 Glyph registered,
                                 StemInter existing,
                                 StemInter stem,
                                 StemsRetriever retriever)
        {
            final IdentityHashMap<Glyph, StemInter> expected = new IdentityHashMap<>();
            expected.putAll(before.entries);
            if (existing != null) {
                if (stem != existing) throw new IllegalStateException("reused system stem mismatch");
            } else if (stem != null) {
                expected.put(registered, stem);
            }
            if (entries.size() != expected.size()) {
                throw new IllegalStateException("unexpected systemStems size delta");
            }
            for (Map.Entry<Glyph, StemInter> entry : expected.entrySet()) {
                if (entries.get(entry.getKey()) != entry.getValue()) {
                    throw new IllegalStateException("unexpected systemStems identity delta");
                }
            }
            final Map<Glyph, StemInter> live =
                    (Map<Glyph, StemInter>) get(RETRIEVER_SYSTEM_STEMS, retriever);
            if (live.size() != entries.size()) throw new IllegalStateException("snapshot incomplete");
        }
    }

    private static final class StemInterState
    {
        final StemInter identity;
        final SIGraph sig;
        final Glyph glyph;
        final GradeImpacts impacts;
        final Object staff;
        final String value;

        StemInterState (StemInter stem)
        {
            identity = stem;
            sig = stem.getSig();
            glyph = stem.getGlyph();
            impacts = stem.getImpacts();
            staff = get(INTER_STAFF, stem);
            value = valueOf(stem);
        }

        boolean same (StemInter stem)
        {
            return stem == identity && stem.getSig() == sig && stem.getGlyph() == glyph
                    && stem.getImpacts() == impacts && get(INTER_STAFF, stem) == staff
                    && value.equals(valueOf(stem));
        }

        private static String valueOf (StemInter stem)
        {
            final Rectangle bounds = stem.getBounds();
            final Line2D median = stem.getMedian();
            return "id=" + stem.getId() + ":shape=" + stem.getShape()
                    + ":grade=" + hex(stem.getGrade()) + ":impacts="
                    + impactsToken(stem.getImpacts()) + ":profile=" + stem.getProfile()
                    + ":abnormal=" + stem.isAbnormal() + ":manual=" + stem.isManual()
                    + ":implicit=" + stem.isImplicit() + ":removed=" + stem.isRemoved()
                    + ":width=" + (stem.getWidth() != null ? hex(stem.getWidth()) : "-")
                    + ":bounds=" + (bounds != null ? rectangle(bounds) : "-")
                    + ":median=" + (median != null ? line(median) : "-");
        }
    }

    private static final class LineSnapshot
    {
        final IdentityHashMap<Object, Line2D> lines = new IdentityHashMap<>();
        final String hash;

        LineSnapshot (List<Inter> beams)
            throws Exception
        {
            final List<String> rows = new ArrayList<>();
            for (Inter inter : beams) {
                final List<Object> allB =
                        (List<Object>) LINKER_ALL_B.get(((AbstractBeamInter) inter).getLinker());
                for (Object b : allB) {
                    final Map<VerticalSide, Object> vMap =
                            (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                    for (Object v : vMap.values()) {
                        final Line2D value = copy((Line2D) V_THEO_LINE.get(v));
                        lines.put(v, value);
                        rows.add(line(value));
                    }
                }
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (LineSnapshot that)
        {
            if (lines.size() != that.lines.size()) return false;
            for (Map.Entry<Object, Line2D> entry : lines.entrySet()) {
                if (!sameLine(entry.getValue(), that.lines.get(entry.getKey()))) return false;
            }
            return true;
        }

        void assertAtMostOneChanged (LineSnapshot that)
        {
            if (lines.size() != that.lines.size()) throw new IllegalStateException("line topology changed");
            int changed = 0;
            for (Map.Entry<Object, Line2D> entry : lines.entrySet()) {
                final Line2D after = that.lines.get(entry.getKey());
                if (after == null) throw new IllegalStateException("line identity disappeared");
                if (!sameLine(entry.getValue(), after)) changed++;
            }
            if (changed > 1) throw new IllegalStateException("expand changed multiple V lines");
        }
    }

    private static Map<Integer, Expected> expectedFrontiers (Path scheduler,
                                                             Path expand,
                                                             Path createStem,
                                                             Path reuseCheck,
                                                             Path baseApply,
                                                             Path bLinkerFlag,
                                                             Path siblingLinks)
        throws Exception
    {
        final Map<Integer, Map<String, String>> frontiers = new HashMap<>();
        for (String line : Files.readAllLines(scheduler, StandardCharsets.UTF_8)) {
            if (!line.startsWith("stemsbeamschedulerfrontier ")) continue;
            final Map<String, String> fields = fields(line);
            if (!"AwaitingVLinkTransaction".equals(fields.get("type"))) continue;
            final int system = Integer.parseInt(required(fields, "system"));
            if (frontiers.put(system, fields) != null) {
                throw new IllegalStateException("duplicate scheduler frontier");
            }
        }
        final Map<String, Map<String, String>> plans = new HashMap<>();
        final Map<String, Map<String, String>> ends = new HashMap<>();
        for (String line : Files.readAllLines(expand, StandardCharsets.UTF_8)) {
            final Map<String, String> values;
            if (line.startsWith("stemsbeamexpandplan ")) {
                values = fields(line);
                final String key = required(values, "system") + ":" + required(values, "plan");
                if (plans.put(key, values) != null) {
                    throw new IllegalStateException("duplicate expand plan row");
                }
            } else if (line.startsWith("stemsbeamexpandend ")) {
                values = fields(line);
                final String key = required(values, "system") + ":" + required(values, "plan");
                if (ends.put(key, values) != null) {
                    throw new IllegalStateException("duplicate expand end row");
                }
            }
        }
        final Map<Integer, Map<String, String>> createFrontiers = new HashMap<>();
        final Map<Integer, Map<String, String>> createResults = new HashMap<>();
        final Map<Integer, Map<String, String>> createDeltas = new HashMap<>();
        final Map<Integer, Map<String, String>> createGuards = new HashMap<>();
        for (String line : Files.readAllLines(createStem, StandardCharsets.UTF_8)) {
            final Map<Integer, Map<String, String>> destination;
            if (line.startsWith("stemsbeamcreatestemfrontier ")) destination = createFrontiers;
            else if (line.startsWith("stemsbeamcreatestemresult ")) destination = createResults;
            else if (line.startsWith("stemsbeamcreatestemdelta ")) destination = createDeltas;
            else if (line.startsWith("stemsbeamcreatestemguard ")) destination = createGuards;
            else continue;
            final Map<String, String> values = fields(line);
            final int system = Integer.parseInt(required(values, "system"));
            if (destination.put(system, values) != null) {
                throw new IllegalStateException("duplicate createStem predecessor row");
            }
        }
        final Map<Integer, Map<String, String>> reuseBaselines = new HashMap<>();
        final Map<Integer, Map<String, String>> reuseSelections = new HashMap<>();
        final Map<Integer, Map<String, String>> reuseCheckResults = new HashMap<>();
        final Map<Integer, Map<String, String>> reuseGuards = new HashMap<>();
        for (String line : Files.readAllLines(reuseCheck, StandardCharsets.UTF_8)) {
            final Map<Integer, Map<String, String>> destination;
            if (line.startsWith("stemsbeamvlinkreusecheckbaseline ")) {
                destination = reuseBaselines;
            } else if (line.startsWith("stemsbeamvlinkreusecheckselection ")) {
                destination = reuseSelections;
            } else if (line.startsWith("stemsbeamvlinkreusecheckcheckresult ")) {
                final Map<String, String> values = fields(line);
                if (!"real".equals(required(values, "scope"))) continue;
                destination = reuseCheckResults;
            } else if (line.startsWith("stemsbeamvlinkreusecheckguard ")) {
                destination = reuseGuards;
            } else {
                continue;
            }
            final Map<String, String> values = fields(line);
            final int system = Integer.parseInt(required(values, "system"));
            if (destination.put(system, values) != null) {
                throw new IllegalStateException("duplicate reuse/check predecessor row");
            }
        }
        final Map<Integer, Map<String, String>> baseFrontiers = new HashMap<>();
        final Map<Integer, Map<String, String>> baseResults = new HashMap<>();
        final Map<Integer, Map<String, String>> baseGuards = new HashMap<>();
        final Map<Integer, Map<String, String>> baseSummaries = new HashMap<>();
        final Map<String, Map<String, String>> syntheticBaseFrontiers = new HashMap<>();
        final Map<String, Map<String, String>> syntheticBaseResults = new HashMap<>();
        final Map<String, Map<String, String>> syntheticBaseGuards = new HashMap<>();
        final Map<String, Map<String, String>> syntheticBaseSummaries = new HashMap<>();
        for (String line : Files.readAllLines(baseApply, StandardCharsets.UTF_8)) {
            final String family;
            if (line.startsWith("stemsbeamvlinkbaseapplyfrontier ")) {
                family = "frontier";
            } else if (line.startsWith("stemsbeamvlinkbaseapplyresult ")) {
                family = "result";
            } else if (line.startsWith("stemsbeamvlinkbaseapplydeltaguard ")) {
                family = "guard";
            } else if (line.startsWith("stemsbeamvlinkbaseapplysummary ")) {
                family = "summary";
            } else {
                continue;
            }
            final Map<String, String> values = fields(line);
            values.put("_rowSha256", sha256Rows(List.of(line)));
            final String scope = required(values, "scope");
            if ("real".equals(scope)) {
                final Map<Integer, Map<String, String>> destination = switch (family) {
                case "frontier" -> baseFrontiers;
                case "result" -> baseResults;
                case "guard" -> baseGuards;
                case "summary" -> baseSummaries;
                default -> throw new IllegalStateException("unknown base family");
                };
                final int system = Integer.parseInt(required(values, "system"));
                if (destination.put(system, values) != null) {
                    throw new IllegalStateException("duplicate base-apply predecessor row");
                }
            } else if ("synthetic".equals(scope)) {
                final String caseName = required(values, "case");
                if (!caseName.equals("ExistingFullSuccess")
                        && !caseName.equals("ExistingDuplicateSuppress")) continue;
                final Map<String, Map<String, String>> destination = switch (family) {
                case "frontier" -> syntheticBaseFrontiers;
                case "result" -> syntheticBaseResults;
                case "guard" -> syntheticBaseGuards;
                case "summary" -> syntheticBaseSummaries;
                default -> throw new IllegalStateException("unknown synthetic base family");
                };
                if (destination.put(caseName, values) != null) {
                    throw new IllegalStateException("duplicate synthetic base predecessor row");
                }
            }
        }
        final Map<String, BaseApplyRows> syntheticBaseRows = new HashMap<>();
        for (String caseName : List.of("ExistingFullSuccess", "ExistingDuplicateSuppress")) {
            final Map<String, String> frontier = syntheticBaseFrontiers.get(caseName);
            final Map<String, String> resultRow = syntheticBaseResults.get(caseName);
            final Map<String, String> guard = syntheticBaseGuards.get(caseName);
            final Map<String, String> summary = syntheticBaseSummaries.get(caseName);
            if (frontier == null || resultRow == null || guard == null || summary == null) {
                throw new IllegalStateException("missing synthetic base predecessor " + caseName);
            }
            assertSameRowKey(
                    "synthetic base-apply " + caseName, frontier, resultRow, guard, summary);
            if (!"1".equals(required(frontier, "system"))) {
                throw new IllegalStateException("synthetic base-apply row is not in system 1");
            }
            syntheticBaseRows.put(
                    caseName, new BaseApplyRows(frontier, resultRow, guard, summary));
        }
        final Map<Integer, BLinkerFlagRows> bLinkerFlagRows = bLinkerFlagRows(bLinkerFlag);
        final Map<Integer, SiblingLinksRows> siblingLinksRows = siblingLinksRows(siblingLinks);
        final Map<Integer, Expected> result = new HashMap<>();
        for (Map.Entry<Integer, Map<String, String>> entry : frontiers.entrySet()) {
            final int system = entry.getKey();
            final Map<String, String> frontier = entry.getValue();
            final String key = system + ":" + required(frontier, "plan");
            final Map<String, String> plan = plans.get(key);
            final Map<String, String> end = ends.get(key);
            final Map<String, String> createFrontier = createFrontiers.get(system);
            final Map<String, String> createResult = createResults.get(system);
            final Map<String, String> createDelta = createDeltas.get(system);
            final Map<String, String> createGuard = createGuards.get(system);
            final Map<String, String> reuseBaseline = reuseBaselines.get(system);
            final Map<String, String> reuseSelection = reuseSelections.get(system);
            final Map<String, String> reuseCheckResult = reuseCheckResults.get(system);
            final Map<String, String> reuseGuard = reuseGuards.get(system);
            final Map<String, String> baseFrontier = baseFrontiers.get(system);
            final Map<String, String> baseResult = baseResults.get(system);
            final Map<String, String> baseGuard = baseGuards.get(system);
            final Map<String, String> baseSummary = baseSummaries.get(system);
            final BLinkerFlagRows bFlagRows = bLinkerFlagRows.get(system);
            final SiblingLinksRows siblingRows = siblingLinksRows.get(system);
            if (plan == null || end == null || !"ReadyForCreateStem".equals(end.get("outcome"))) {
                throw new IllegalStateException("frontier lacks ready expand fixture row");
            }
            if (createFrontier == null || createResult == null || createDelta == null
                    || createGuard == null) {
                throw new IllegalStateException("frontier lacks complete createStem predecessor rows");
            }
            if (reuseBaseline == null || reuseSelection == null || reuseCheckResult == null
                    || reuseGuard == null) {
                throw new IllegalStateException("frontier lacks complete reuse/check predecessor rows");
            }
            if (baseFrontier == null || baseResult == null || baseGuard == null
                    || baseSummary == null) {
                throw new IllegalStateException("frontier lacks complete base-apply predecessor rows");
            }
            if (bFlagRows == null) {
                throw new IllegalStateException(
                        "frontier lacks complete B-linker-flag predecessor rows");
            }
            if (siblingRows == null) {
                throw new IllegalStateException(
                        "frontier lacks complete sibling-links predecessor rows");
            }
            assertSameRowKey(
                    "real base-apply system " + system,
                    baseFrontier, baseResult, baseGuard, baseSummary);
            for (String name : List.of(
                    "beamSig", "bAlias", "vSide", "builder", "plan",
                    "stemProfile", "linkProfile")) {
                if (!required(frontier, name).equals(required(plan, name))) {
                    throw new IllegalStateException("scheduler/expand key mismatch: " + name);
                }
            }
            final String[] evidence = required(frontier, "evidence").split(",");
            final Map<String, String> evidenceFields = new HashMap<>();
            for (String token : evidence) {
                final String[] pair = token.split("=", 2);
                evidenceFields.put(pair[0], pair[1]);
            }
            if (!required(evidenceFields, "lastIndex").equals(required(end, "lastIndex"))
                    || !required(evidenceFields, "relations").equals(required(end, "relationCount"))
                    || !required(evidenceFields, "glyphs").equals(required(end, "glyphCount"))) {
                throw new IllegalStateException("scheduler/expand evidence mismatch");
            }
            for (String name : List.of(
                    "beamSig", "hSide", "bAlias", "vSide", "builder", "plan",
                    "stemProfile", "linkProfile")) {
                if (!required(frontier, name).equals(required(createFrontier, name))) {
                    throw new IllegalStateException("scheduler/createStem key mismatch: " + name);
                }
                if (!required(frontier, name).equals(required(baseFrontier, name))) {
                    throw new IllegalStateException("scheduler/base-apply key mismatch: " + name);
                }
            }
            if (!"ReadyBeforeBLinkerFlagMutation".equals(required(baseResult, "terminal"))
                    || !"ReadyBeforeBLinkerFlagMutation".equals(
                            required(baseSummary, "terminal"))
                    || !"true".equals(required(baseGuard, "stopBeforeBLinkerFlagMutation"))) {
                throw new IllegalStateException("base-apply predecessor is not at boundary 15");
            }
            if (!"ReadyBeforeSiblingBeamLinks".equals(required(bFlagRows.result, "terminal"))
                    || !"ReadyBeforeSiblingBeamLinks".equals(
                            required(bFlagRows.summary, "terminal"))
                    || !"true".equals(required(
                            bFlagRows.guard, "stopBeforeSiblingBeamLinks"))) {
                throw new IllegalStateException(
                        "B-linker-flag predecessor is not at boundary 16");
            }
            for (String name : List.of("system", "plan")) {
                if (!required(frontier, name).equals(required(bFlagRows.result, name))) {
                    throw new IllegalStateException(
                            "scheduler/B-linker-flag key mismatch: " + name);
                }
            }
            if (!required(end, "lastIndex").equals(required(
                    fieldsForCreateExpand(createStem, system), "lastIndex"))) {
                throw new IllegalStateException("expand/createStem lastIndex mismatch");
            }
            if (!required(end, "relationCount").equals(required(
                    fieldsForCreateExpand(createStem, system), "relations"))) {
                throw new IllegalStateException("expand/createStem relation count mismatch");
            }
            if (!required(end, "glyphCount").equals(required(
                    fieldsForCreateExpand(createStem, system), "glyphs"))) {
                throw new IllegalStateException("expand/createStem glyph count mismatch");
            }
            final List<String> relationAliases = "-".equals(required(end, "relations"))
                    ? List.of() : List.of(required(end, "relations").split(","));
            result.put(system, new Expected(List.of(
                    required(frontier, "beamSig"), required(frontier, "hSide"),
                    required(frontier, "bAlias"), required(frontier, "vSide"),
                    required(frontier, "builder"), required(frontier, "plan"),
                    required(frontier, "stemProfile"), required(frontier, "linkProfile"),
                    required(end, "lastIndex"), required(end, "relationCount"),
                    required(end, "glyphCount")), relationAliases,
                    required(end, "relationsPastReturn"), createResult, createDelta, createGuard,
                    reuseBaseline, reuseSelection, reuseCheckResult, reuseGuard,
                    baseFrontier, baseResult, baseGuard, baseSummary,
                    system == 1 ? syntheticBaseRows : Map.of(), bFlagRows, siblingRows));
        }
        if (result.size() != createFrontiers.size() || result.size() != createResults.size()
                || result.size() != createDeltas.size() || result.size() != createGuards.size()
                || result.size() != reuseBaselines.size()
                || result.size() != reuseSelections.size()
                || result.size() != reuseCheckResults.size()
                || result.size() != reuseGuards.size()
                || result.size() != baseFrontiers.size()
                || result.size() != baseResults.size()
                || result.size() != baseGuards.size()
                || result.size() != baseSummaries.size()
                || result.size() != bLinkerFlagRows.size()
                || result.size() != siblingLinksRows.size()) {
            throw new IllegalStateException("predecessor fixture system coverage mismatch");
        }
        return result;
    }

    /**
     * Read every real Boundary-16 transaction row in file order. The ordered bundle binds all
     * variable group, sibling, query, callback, and builder rows without pretending that a digest
     * replaces the Rust gate's full typed Boundary-16 replay.
     */
    private static Map<Integer, SiblingLinksRows> siblingLinksRows (Path fixture)
        throws Exception
    {
        final Map<Integer, List<String>> ordered = new LinkedHashMap<>();
        final Map<Integer, Map<String, String>> predecessors = new HashMap<>();
        final Map<Integer, Map<String, String>> baselines = new HashMap<>();
        final Map<Integer, Map<String, String>> results = new HashMap<>();
        final Map<Integer, Map<String, String>> guards = new HashMap<>();
        final Map<Integer, Map<String, String>> summaries = new HashMap<>();
        for (String line : Files.readAllLines(fixture, StandardCharsets.UTF_8)) {
            if (!line.startsWith("stemsbeamvlinksiblinglinks")) continue;
            final String family = line.substring(0, line.indexOf(' '));
            if (family.equals("stemsbeamvlinksiblinglinkspage")
                    || family.equals("stemsbeamvlinksiblinglinkspagesummary")
                    || family.equals("stemsbeamvlinksiblinglinkscorpussummary")
                    || family.contains("synthetic")) {
                continue;
            }
            final Map<String, String> values = fields(line);
            if (!"real".equals(values.get("scope"))) continue;
            values.put("_rowSha256", sha256Rows(List.of(line)));
            final int system = Integer.parseInt(required(values, "system"));
            ordered.computeIfAbsent(system, ignored -> new ArrayList<>()).add(line);
            final Map<Integer, Map<String, String>> destination = switch (family) {
            case "stemsbeamvlinksiblinglinkspredecessor" -> predecessors;
            case "stemsbeamvlinksiblinglinksbaseline" -> baselines;
            case "stemsbeamvlinksiblinglinksresult" -> results;
            case "stemsbeamvlinksiblinglinksdeltaguard" -> guards;
            case "stemsbeamvlinksiblinglinkssummary" -> summaries;
            default -> null;
            };
            if (destination != null && destination.put(system, values) != null) {
                throw new IllegalStateException("duplicate sibling-links predecessor " + family);
            }
        }
        final Map<Integer, SiblingLinksRows> result = new HashMap<>();
        for (Map.Entry<Integer, List<String>> entry : ordered.entrySet()) {
            final int system = entry.getKey();
            final Map<String, String> predecessor = predecessors.get(system);
            final Map<String, String> baseline = baselines.get(system);
            final Map<String, String> resultRow = results.get(system);
            final Map<String, String> guard = guards.get(system);
            final Map<String, String> summary = summaries.get(system);
            if (predecessor == null || baseline == null || resultRow == null || guard == null
                    || summary == null) {
                throw new IllegalStateException(
                        "incomplete sibling-links predecessor for system " + system);
            }
            assertSameRowKey(
                    "real sibling-links system " + system,
                    predecessor, baseline, resultRow, guard, summary);
            if (!"ReadyBeforeHeadRelationLoop".equals(required(resultRow, "terminal"))
                    || !"ReadyBeforeHeadRelationLoop".equals(required(summary, "terminal"))
                    || !"true".equals(required(guard, "stopBeforeHeadRelationLoop"))) {
                throw new IllegalStateException(
                        "sibling-links predecessor is not at boundary 17");
            }
            result.put(system, new SiblingLinksRows(
                    predecessor, baseline, resultRow, guard, summary,
                    entry.getValue().size(), sha256Rows(entry.getValue())));
        }
        return result;
    }

    /**
     * Read every real Boundary-15 transaction row in file order. The ordered digest includes the
     * variable B-arena and observation rows. It authenticates the Java evidence bundle; the Rust
     * integration gate independently replays and compares the complete typed Boundary-15 product.
     */
    private static Map<Integer, BLinkerFlagRows> bLinkerFlagRows (Path fixture)
        throws Exception
    {
        final Map<Integer, List<String>> ordered = new LinkedHashMap<>();
        final Map<Integer, Map<String, String>> predecessors = new HashMap<>();
        final Map<Integer, Map<String, String>> baselines = new HashMap<>();
        final Map<Integer, Map<String, String>> targets = new HashMap<>();
        final Map<Integer, Map<String, String>> writes = new HashMap<>();
        final Map<Integer, Map<String, String>> results = new HashMap<>();
        final Map<Integer, Map<String, String>> guards = new HashMap<>();
        final Map<Integer, Map<String, String>> summaries = new HashMap<>();
        for (String line : Files.readAllLines(fixture, StandardCharsets.UTF_8)) {
            if (!line.startsWith("stemsbeamvlinkblinkerflag")) continue;
            if (line.startsWith("stemsbeamvlinkblinkerflagpage ")
                    || line.startsWith("stemsbeamvlinkblinkerflagpagesummary ")
                    || line.startsWith("stemsbeamvlinkblinkerflagcorpussummary ")) {
                continue;
            }
            final Map<String, String> values = fields(line);
            if (!"real".equals(required(values, "scope"))) continue;
            final int system = Integer.parseInt(required(values, "system"));
            ordered.computeIfAbsent(system, unused -> new ArrayList<>()).add(line);
            final Map<Integer, Map<String, String>> destination;
            if (line.startsWith("stemsbeamvlinkblinkerflagpredecessor ")) {
                destination = predecessors;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagbaseline ")) {
                destination = baselines;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagtarget ")) {
                destination = targets;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagwritetrace ")) {
                destination = writes;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagresult ")) {
                destination = results;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagdeltaguard ")) {
                destination = guards;
            } else if (line.startsWith("stemsbeamvlinkblinkerflagsummary ")) {
                destination = summaries;
            } else {
                continue;
            }
            values.put("_rowSha256", sha256Rows(List.of(line)));
            if (destination.put(system, values) != null) {
                throw new IllegalStateException("duplicate B-linker-flag predecessor family");
            }
        }
        final Map<Integer, BLinkerFlagRows> result = new HashMap<>();
        for (Map.Entry<Integer, List<String>> entry : ordered.entrySet()) {
            final int system = entry.getKey();
            final Map<String, String> predecessor = predecessors.get(system);
            final Map<String, String> baseline = baselines.get(system);
            final Map<String, String> target = targets.get(system);
            final Map<String, String> write = writes.get(system);
            final Map<String, String> resultRow = results.get(system);
            final Map<String, String> guard = guards.get(system);
            final Map<String, String> summary = summaries.get(system);
            if (predecessor == null || baseline == null || target == null || write == null
                    || resultRow == null || guard == null || summary == null) {
                throw new IllegalStateException(
                        "incomplete real B-linker-flag predecessor system " + system);
            }
            assertSameRowKey(
                    "real B-linker-flag system " + system,
                    predecessor, baseline, target, write, resultRow, guard, summary);
            result.put(system, new BLinkerFlagRows(
                    predecessor, baseline, target, write, resultRow, guard, summary,
                    entry.getValue().size(), sha256Rows(entry.getValue())));
        }
        return result;
    }

    private static Map<String, String> fieldsForCreateExpand (Path createStem,
                                                              int wantedSystem)
        throws Exception
    {
        Map<String, String> found = null;
        for (String line : Files.readAllLines(createStem, StandardCharsets.UTF_8)) {
            if (!line.startsWith("stemsbeamcreatestemexpand ")) continue;
            final Map<String, String> values = fields(line);
            if (Integer.parseInt(required(values, "system")) == wantedSystem) {
                if (found != null) throw new IllegalStateException("duplicate createStem expand row");
                found = values;
            }
        }
        if (found == null) throw new IllegalStateException("missing createStem expand row");
        return found;
    }

    private static Map<String, String> fields (String line)
    {
        final String[] tokens = line.split(" ");
        final Map<String, String> result = new HashMap<>();
        if (tokens.length > 1) result.put("_page", tokens[1]);
        for (int i = 2; i + 1 < tokens.length; i += 2) result.put(tokens[i], tokens[i + 1]);
        return result;
    }

    @SafeVarargs
    private static void assertSameRowKey (String label,
                                          Map<String, String>... rows)
    {
        if (rows.length == 0) throw new IllegalArgumentException("no rows for " + label);
        final Map<String, String> reference = rows[0];
        for (Map<String, String> row : rows) {
            for (String name : List.of("_page", "system", "plan", "scope", "case")) {
                if (!required(reference, name).equals(required(row, name))) {
                    throw new IllegalStateException(label + " key mismatch at " + name);
                }
            }
        }
    }

    private static void assertField (Map<String, String> values,
                                     String name,
                                     String actual)
    {
        final String expected = required(values, name);
        if (!expected.equals(actual)) {
            throw new IllegalStateException(
                    "predecessor createStem drift at " + name + ": actual=" + actual
                            + " expected=" + expected);
        }
    }

    private static String cToken (int headOrdinal,
                                  HorizontalSide hSide,
                                  VerticalSide vSide)
    {
        return "h:" + headOrdinal + ":" + hSide + ":" + vSide;
    }

    private static String relationInputHash (LinkedHashMap<StemLinker, Relation> relations,
                                             IdentityHashMap<Object, String> cAliases)
    {
        final List<String> rows = new ArrayList<>();
        int ordinal = 0;
        for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
            final String alias = cAliases.get(entry.getKey());
            if (alias == null) throw new IllegalStateException("relation input lacks C alias");
            rows.add(ordinal + ":" + alias + ":" + relationState(entry.getValue()));
            ordinal++;
        }
        return sha256Rows(rows);
    }

    private static String headRelationMapStateHash (
            LinkedHashMap<StemLinker, Relation> relations,
            IdentityHashMap<Object, String> cAliases)
    {
        final List<String> rows = new ArrayList<>();
        int ordinal = 0;
        for (Map.Entry<StemLinker, Relation> entry : relations.entrySet()) {
            final String alias = cAliases.get(entry.getKey());
            if (alias == null || !(entry.getValue() instanceof HeadStemRelation relation)) {
                throw new IllegalStateException("head relation input lacks exact plan draft");
            }
            rows.add(ordinal++ + ":" + alias + ":" + headDraftState(relation));
        }
        return sha256Rows(rows);
    }

    private static String headDraftState (HeadStemRelation relation)
    {
        final Double nullable = (Double) get(HEAD_STEM_CONSISTENCY, relation);
        return relationState(relation) + ":nullableConsistency="
                + (nullable != null ? "Set:" + hex(nullable) : "Unset");
    }

    private static String relationState (Relation relation)
    {
        final StringBuilder value = new StringBuilder(relation.getClass().getName())
                .append(":manual=").append(relation.isManual());
        if (relation instanceof Support support) {
            value.append(":grade=").append(hex(support.getGrade()))
                    .append(":impacts=").append(impactsToken(support.getImpacts()));
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

    private static String interStructuralToken (Inter inter)
    {
        final Rectangle bounds = inter.getBounds();
        final StringBuilder value = new StringBuilder(inter.getClass().getName())
                .append(":shape=").append(inter.getShape())
                .append(":grade=").append(hex(inter.getGrade()))
                .append(":bounds=").append(bounds != null ? rectangle(bounds) : "-")
                .append(":removed=").append(inter.isRemoved())
                .append(":abnormal=").append(inter.isAbnormal())
                .append(":manual=").append(inter.isManual())
                .append(":implicit=").append(inter.isImplicit())
                .append(":profile=").append(inter.getProfile());
        if (inter instanceof StemInter stem) {
            value.append(":median=").append(
                    stem.getMedian() != null ? line(stem.getMedian()) : "-")
                    .append(":width=").append(
                            stem.getWidth() != null ? hex(stem.getWidth()) : "-");
        } else if (inter instanceof AbstractBeamInter beam) {
            value.append(":median=").append(
                    beam.getMedian() != null ? line(beam.getMedian()) : "-")
                    .append(":height=").append(hex(beam.getHeight()));
        }
        return value.toString();
    }

    private static String stemAlias (StemInter stem,
                                     StemInter initialStem,
                                     int plan,
                                     LiveStemCatalogue liveStems)
    {
        if (stem == initialStem) return "created:" + plan;
        return liveStems.alias(stem);
    }

    private static String stemStateHash (StemInter stem)
    {
        return sha256Rows(List.of(StemInterState.valueOf(stem)));
    }

    private static double intersectionDen (Line2D first,
                                           Line2D second)
    {
        return ((first.getX1() - first.getX2()) * (second.getY1() - second.getY2()))
                - ((first.getY1() - first.getY2()) * (second.getX1() - second.getX2()));
    }

    private static double clampImpact (double value)
    {
        if (value < 0) value = 0;
        if (value > 1) value = 1;
        return value;
    }

    private static void requireBits (double expected,
                                     double actual,
                                     String label)
    {
        if (Double.doubleToRawLongBits(expected) != Double.doubleToRawLongBits(actual)) {
            throw new IllegalStateException(
                    label + " raw-bit drift: expected=" + hex(expected) + " actual=" + hex(actual));
        }
    }

    private static void requirePointBits (Point2D expected,
                                          Point2D actual,
                                          String label)
    {
        if (expected == null || actual == null) {
            if (expected != actual) throw new IllegalStateException(label + " null drift");
            return;
        }
        requireBits(expected.getX(), actual.getX(), label + " x");
        requireBits(expected.getY(), actual.getY(), label + " y");
    }

    private static String required (Map<String, String> fields,
                                    String name)
    {
        final String value = fields.get(name);
        if (value == null) throw new IllegalStateException("missing field " + name);
        return value;
    }

    private static String boolToken (Boolean value)
    {
        return value != null ? value.toString() : "-";
    }

    private static String terminal (ApplyRun run)
    {
        if (run.thrown == null) return "ReadyBeforeBLinkerFlagMutation";
        if ("VertexAdd".equals(run.throwStage)) return "EnvelopeThrownAtVertexAdd";
        if (run.failureMode == FailureMode.LATER_EDGE_LISTENER) {
            return "EnvelopeThrownAtLaterEdgeListener";
        }
        if (run.failureMode == FailureMode.EARLY_EDGE_LISTENER) {
            return "EnvelopeThrownAtEarlyEdgeListener";
        }
        return "EnvelopeThrownAtEdgeInsert";
    }

    private static Inter interAtId (InterRegistrySnapshot snapshot,
                                    int id)
    {
        for (Inter inter : snapshot.identities.keySet()) {
            if (inter.getId() == id) return inter;
        }
        return null;
    }

    private static Glyph glyphActiveAtId (GlyphRegistrySnapshot snapshot,
                                          int id)
    {
        for (Glyph glyph : snapshot.active.keySet()) {
            if (glyph.getId() == id) return glyph;
        }
        return null;
    }

    private static Glyph glyphOriginalAtId (GlyphRegistrySnapshot snapshot,
                                            int id)
    {
        for (Glyph glyph : snapshot.originals.keySet()) {
            if (glyph.getId() == id) return glyph;
        }
        return null;
    }

    private static SystemInfo systemById (Sheet sheet,
                                          int id)
    {
        for (SystemInfo system : sheet.getSystems()) if (system.getId() == id) return system;
        throw new IllegalArgumentException("missing system " + id);
    }

    private static Sheet loadPage (Path path,
                                   int wanted)
        throws Exception
    {
        final Book book = new Book(path);
        book.createStubs();
        SheetStub selected = null;
        for (SheetStub stub : book.getValidStubs()) {
            if (stub.getNumber() == wanted) {
                selected = stub;
                break;
            }
        }
        if (selected == null) throw new IllegalArgumentException("missing sheet " + wanted);
        selected.reachStep(OmrStep.HEADS, false);
        return selected.getSheet();
    }

    private static String noStaffDigest (Sheet sheet)
    {
        final ByteProcessor buffer = sheet.getPicture().getSource(Picture.SourceKey.NO_STAFF);
        final MessageDigest digest = sha256();
        update(digest, buffer.getWidth() + "x" + buffer.getHeight() + "\n");
        for (int i = 0; i < buffer.getWidth() * buffer.getHeight(); i++) {
            digest.update((byte) buffer.get(i));
        }
        return buffer.getWidth() + "x" + buffer.getHeight() + ":" + digestHex(digest.digest());
    }

    private static List<String> selectedGlyphTokens (
            Collection<Glyph> selected,
            GlyphRegistrySnapshot registry)
    {
        final List<String> result = new ArrayList<>();
        for (Glyph glyph : selected) {
            final String membership = registry.active.containsKey(glyph) ? "active"
                    : registry.originals.containsKey(glyph) ? "original-only" : "transient";
            final String alias = registry.aliases.containsKey(glyph)
                    ? registry.alias(glyph) : "transient";
            result.add(alias + ":" + membership + ":id=" + glyph.getId()
                    + ":" + glyphToken(glyph));
        }
        return result;
    }

    private static String impactsToken (GradeImpacts impacts)
    {
        if (impacts == null) return "-";
        final List<String> values = new ArrayList<>();
        for (int i = 0; i < impacts.getImpactCount(); i++) {
            values.add(impacts.getName(i) + ":" + hex(impacts.getImpact(i))
                    + ":w=" + hex(impacts.getWeight(i)));
        }
        return list(values);
    }

    private static IdentityHashMap<Inter, Integer> ordinals (List<Inter> values)
    {
        final IdentityHashMap<Inter, Integer> result = new IdentityHashMap<>();
        for (int i = 0; i < values.size(); i++) result.put(values.get(i), i);
        return result;
    }

    private static String glyphToken (Glyph glyph)
    {
        final Rectangle box = glyph.getBounds();
        return "g:" + box.x + ":" + box.y + ":" + box.width + ":" + box.height
                + ":" + glyphRunSha256(glyph);
    }

    private static String glyphRunSha256 (Glyph glyph)
    {
        final MessageDigest digest = sha256();
        final var table = glyph.getRunTable();
        update(digest, table.getOrientation() + " " + table.getWidth() + " "
                + table.getHeight() + "\n");
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                final var run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            update(digest, row.append('\n').toString());
        }
        return digestHex(digest.digest());
    }

    private static String runTableToken (Glyph glyph)
    {
        final var table = glyph.getRunTable();
        final StringBuilder result = new StringBuilder()
                .append(table.getOrientation()).append(':')
                .append(table.getWidth()).append('x').append(table.getHeight()).append(':')
                .append('[');
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            if (sequence > 0) result.append(';');
            result.append(sequence).append('=');
            boolean first = true;
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                final var run = iterator.next();
                if (!first) result.append(',');
                result.append(run.getStart()).append(':').append(run.getLength());
                first = false;
            }
            if (first) result.append('-');
        }
        return result.append(']').toString();
    }

    private static String rectangle (Rectangle box)
    {
        return box.x + ":" + box.y + ":" + box.width + ":" + box.height;
    }

    private static Line2D copy (Line2D line)
    {
        return new Line2D.Double(line.getP1(), line.getP2());
    }

    private static Point2D copyPoint (Point2D point)
    {
        return new Point2D.Double(point.getX(), point.getY());
    }

    private static void assertIdentityList (String label,
                                            List<?> expected,
                                            List<?> actual)
    {
        if (expected.size() != actual.size()) {
            throw new IllegalStateException(label + " size drift");
        }
        for (int i = 0; i < expected.size(); i++) {
            if (expected.get(i) != actual.get(i)) {
                throw new IllegalStateException(label + " identity/order drift at " + i);
            }
        }
    }

    private static double maxShorterRatio ()
        throws Exception
    {
        final Object constants = BEAM_LINKER_CONSTANTS.get(null);
        final Object ratio = CONSTANTS_MAX_SHORTER_RATIO.get(constants);
        final Object value = ratio.getClass().getMethod("getValue").invoke(ratio);
        return ((Number) value).doubleValue();
    }

    private static double neutralStemLength ()
        throws Exception
    {
        final Object constants = HEAD_STEM_CONSTANTS.get(null);
        final Object fraction = HEAD_STEM_NEUTRAL.get(constants);
        final Object value = fraction.getClass().getMethod("getValue").invoke(fraction);
        return ((Number) value).doubleValue();
    }

    private static String glyphIdentity (Glyph glyph,
                                         IdentityHashMap<Glyph, String> aliases)
    {
        if (glyph == null) return "null-glyph";
        final String alias = aliases.get(glyph);
        if (alias == null) throw new IllegalStateException("group glyph lacks identity alias");
        return alias;
    }

    private static String nullableGlyphToken (Glyph glyph)
    {
        return glyph != null ? glyphToken(glyph) : "null";
    }

    private static boolean sameLine (Line2D left,
                                     Line2D right)
    {
        return left != null && right != null
                && Double.doubleToLongBits(left.getX1()) == Double.doubleToLongBits(right.getX1())
                && Double.doubleToLongBits(left.getY1()) == Double.doubleToLongBits(right.getY1())
                && Double.doubleToLongBits(left.getX2()) == Double.doubleToLongBits(right.getX2())
                && Double.doubleToLongBits(left.getY2()) == Double.doubleToLongBits(right.getY2());
    }

    private static String line (Line2D line)
    {
        return hex(line.getX1()) + ":" + hex(line.getY1()) + ":"
                + hex(line.getX2()) + ":" + hex(line.getY2());
    }

    private static String point (Point2D point)
    {
        return point != null ? hex(point.getX()) + ":" + hex(point.getY()) : "-";
    }

    private static String hex (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToRawLongBits(value));
    }

    private static String list (Collection<?> values)
    {
        if (values.isEmpty()) return "-";
        final StringBuilder result = new StringBuilder("[");
        boolean first = true;
        for (Object value : values) {
            if (!first) result.append(',');
            result.append(value);
            first = false;
        }
        return result.append(']').toString();
    }

    private static <T> IdentityHashMap<T, Boolean> copySet (IdentityHashMap<T, Boolean> source)
    {
        final IdentityHashMap<T, Boolean> result = new IdentityHashMap<>();
        result.putAll(source);
        return result;
    }

    private static <T> boolean identitySetEquals (IdentityHashMap<T, Boolean> left,
                                                  IdentityHashMap<T, Boolean> right)
    {
        if (left.size() != right.size()) return false;
        for (T value : left.keySet()) if (!right.containsKey(value)) return false;
        return true;
    }

    private static <T> boolean sameIdentityOrder (List<T> left,
                                                  List<T> right)
    {
        if (left.size() != right.size()) return false;
        for (int ordinal = 0; ordinal < left.size(); ordinal++) {
            if (left.get(ordinal) != right.get(ordinal)) return false;
        }
        return true;
    }

    private static <T> boolean sameIdentityPrefix (List<T> prefix,
                                                   List<T> values)
    {
        if (prefix.size() > values.size()) return false;
        for (int ordinal = 0; ordinal < prefix.size(); ordinal++) {
            if (prefix.get(ordinal) != values.get(ordinal)) return false;
        }
        return true;
    }

    private static int dirtyFlagMutationCount (DirtyFlags before,
                                               DirtyFlags after)
    {
        return (before.stubModified() != after.stubModified() ? 1 : 0)
                + (before.bookModified() != after.bookModified() ? 1 : 0)
                + (before.bookDirty() != after.bookDirty() ? 1 : 0);
    }

    private static <T> boolean identityKeysEqual (IdentityHashMap<T, Boolean> left,
                                                  IdentityHashMap<T, ?> right)
    {
        if (left.size() != right.size()) return false;
        for (T value : left.keySet()) if (!right.containsKey(value)) return false;
        return true;
    }

    private static String sha256Rows (List<String> rows)
    {
        final MessageDigest digest = sha256();
        for (String row : rows) update(digest, row + "\n");
        return digestHex(digest.digest());
    }

    private static String sha256File (Path path)
        throws Exception
    {
        return digestHex(sha256().digest(Files.readAllBytes(path)));
    }

    private static MessageDigest sha256 ()
    {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (java.security.NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static void update (MessageDigest digest,
                                String text)
    {
        digest.update(text.getBytes(StandardCharsets.UTF_8));
    }

    private static String digestHex (byte[] bytes)
    {
        final StringBuilder result = new StringBuilder();
        for (byte value : bytes) result.append(String.format("%02x", value & 0xff));
        return result.toString();
    }

    private static Field field (Class<?> owner,
                                String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Object get (Field field,
                               Object target)
    {
        try {
            return field.get(target);
        } catch (IllegalAccessException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink head-links oracle.");
        System.out.println("# schema: stems-beam-vlink-head-links-v1");
        System.out.println("# Frozen scheduler through sibling-links predecessors are replayed and joined exactly.");
        System.out.println("# Head plans follow the original LinkedHashMap first-insertion order with latest payload values.");
        System.out.println("# Each entry writes its shared SLinker before the directed HeadStem duplicate lookup.");
        System.out.println("# Missing pairs mutate the existing plan draft, insert it directly, and run the synchronous callback.");
        System.out.println("# Isolated manual, chord, duplicate, and exception evidence is gate-only, not production-equivalent.");
        System.out.println("# Stop is after return true and immediately before the caller outer BLinker assignment.");
    }

    private record PlanRef(int plan, int builder, String bAlias, VerticalSide vSide)
    {
    }

    private record Expected(List<String> values,
                            List<String> relationAliases,
                            String relationsPastReturn,
                            Map<String, String> createResult,
                            Map<String, String> createDelta,
                            Map<String, String> createGuard,
                            Map<String, String> reuseBaseline,
                            Map<String, String> reuseSelection,
                            Map<String, String> reuseCheckResult,
                            Map<String, String> reuseGuard,
                            Map<String, String> baseFrontier,
                            Map<String, String> baseResult,
                            Map<String, String> baseGuard,
                            Map<String, String> baseSummary,
                            Map<String, BaseApplyRows> syntheticBaseRows,
                            BLinkerFlagRows bLinkerFlag,
                            SiblingLinksRows siblingLinks)
    {
    }

    private record BaseApplyRows(Map<String, String> frontier,
                                 Map<String, String> result,
                                 Map<String, String> guard,
                                 Map<String, String> summary)
    {
    }

    private record BLinkerFlagRows(Map<String, String> predecessor,
                                   Map<String, String> baseline,
                                   Map<String, String> target,
                                   Map<String, String> write,
                                   Map<String, String> result,
                                   Map<String, String> guard,
                                   Map<String, String> summary,
                                   int orderedRowCount,
                                   String orderedRowsSha256)
    {
    }

    private record SiblingLinksRows(Map<String, String> predecessor,
                                    Map<String, String> baseline,
                                    Map<String, String> result,
                                    Map<String, String> guard,
                                    Map<String, String> summary,
                                    int orderedRowCount,
                                    String orderedRowsSha256)
    {
    }

    private static final class Totals
    {
        long transactions;
        long newVertexBranches;
        long existingVertexBranches;
        long edgesAdded;
        long chordMatches;
        long throwsSeen;
        long siblingCandidates;
        long siblingEdgesAdded;
        long headEntries;
        long headEdgesAdded;
    }
}
