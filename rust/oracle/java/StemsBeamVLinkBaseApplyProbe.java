// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.awt.geom.Path2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
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
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemBuilder;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.AbstractInter;
import org.audiveris.omr.sig.inter.BeamInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.AbstractConnection;
import org.audiveris.omr.sig.relation.AbstractStemConnection;
import org.audiveris.omr.sig.relation.BeamPortion;
import org.audiveris.omr.sig.relation.BeamRestRelation;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.sig.relation.ChordStemRelation;
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
 * Stateful beam V-link base-application boundary.
 *
 * <p>Each target system is reconstructed from a fresh HEADS sheet. The selected scheduler prefix
 * joins the frozen scheduler, expand, createStem, and V-link reuse/check fixtures. The probe
 * reconstructs the accepted boundary-13 result, executes BeamLinker.java 1651--1656 exactly:
 * conditional {@link SIGraph#addVertex(Inter)} followed by {@link Link#applyTo(Inter)}, and stops
 * before base B-linker flag, sibling-beam, or head-relation mutation. System 1 also emits isolated
 * synthetic Sheet/System/SIG certificates for existing-stem, suppression, and exception-prefix
 * behavior.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamVLinkBaseApplyProbe
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
    private static final Field LINKER_SIDE_B;
    private static final Field B_ID;
    private static final Field B_H_SIDE;
    private static final Field B_IS_ANCHOR;
    private static final Field B_V_LINKERS;
    private static final Field V_V_SIDE;
    private static final Field V_Y_DIR;
    private static final Field V_THEO_LINE;
    private static final Field V_STEM_BUILDER;
    private static final Method V_EXPAND;
    private static final Field STEM_BUILDER_THEO_LINE;
    private static final Field GLYPH_ORIGINALS;
    private static final Field BEAM_OUT_WEIGHTS;
    private static final Field INTER_STAFF;
    private static final Field BOOK_MODIFIED;
    private static final Field GRAPH_LISTENERS;
    private static final Field VERTEX_SET_LISTENERS;
    private static final Field INTER_AREA;
    private static final Class<?> B_LINKER_CLASS;
    private static final Class<?> V_LINKER_CLASS;
    private static final Class<?> C_LINKER_CLASS;
    private static final Field C_V_SIDE;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
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
            LINKER_SIDE_B = field(BeamLinker.class, "sideBLinkers");
            B_ID = field(B_LINKER_CLASS, "id");
            B_H_SIDE = field(B_LINKER_CLASS, "hSide");
            B_IS_ANCHOR = field(B_LINKER_CLASS, "isAnchor");
            B_V_LINKERS = field(B_LINKER_CLASS, "vLinkers");
            V_V_SIDE = field(V_LINKER_CLASS, "vSide");
            V_Y_DIR = field(V_LINKER_CLASS, "yDir");
            V_THEO_LINE = field(V_LINKER_CLASS, "theoLine");
            V_STEM_BUILDER = field(V_LINKER_CLASS, "sb");
            V_EXPAND = V_LINKER_CLASS.getDeclaredMethod(
                    "expand", int.class, int.class, Map.class, Set.class);
            V_EXPAND.setAccessible(true);
            C_V_SIDE = field(C_LINKER_CLASS, "vSide");
            STEM_BUILDER_THEO_LINE = field(StemBuilder.class, "theoLine");
            GLYPH_ORIGINALS = field(GlyphIndex.class, "originals");
            BEAM_OUT_WEIGHTS = field(BeamStemRelation.class, "OUT_WEIGHTS");
            INTER_STAFF = field(AbstractInter.class, "staff");
            BOOK_MODIFIED = field(Book.class, "modified");
            GRAPH_LISTENERS = field(DefaultListenableGraph.class, "graphListeners");
            VERTEX_SET_LISTENERS = field(DefaultListenableGraph.class, "vertexSetListeners");
            INTER_AREA = field(AbstractInter.class, "area");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamVLinkBaseApplyProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            return;
        }
        if (args.length != 7 || !args[0].equals("--system")) {
            throw new IllegalArgumentException(
                    "expected --system <id> <path>:<sheet> <scheduler-fixture> "
                            + "<expand-fixture> <createStem-fixture> <reuse-check-fixture>");
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
        final Map<Integer, Expected> expected = expectedFrontiers(
                schedulerFixture, expandFixture, createStemFixture, reuseCheckFixture);

        final Sheet sheet = loadPage(pagePath, sheetNumber);
        final int systemCount = sheet.getSystems().size();
        if (expected.size() != systemCount) {
            throw new IllegalStateException("fixture/system count mismatch");
        }
        final String page = pagePath.getFileName() + "#" + sheetNumber;
        if (wantedSystem == 1) {
            printHeader();
            System.out.printf(
                    "stemsbeamvlinkbaseapplypage %s systems %d schedulerFixtureSha256 %s "
                            + "expandFixtureSha256 %s createStemFixtureSha256 %s "
                            + "reuseCheckFixtureSha256 %s executionMode foregroundJvmPerSystem "
                            + "vertexOrder JGraphTLinkedHashMap edgeOrder JGraphTLinkedHashMap "
                            + "incidentOrder IncomingThenOutgoing duplicateOrder SourceOutgoing "
                            + "listenerMode ExactStandardSingleSigListener "
                            + "headless %s "
                            + "registryHashMode StructuralPayloadWithoutOpaqueUnrelatedIds "
                            + "graphHashMode SourceOrderStructuralEndpoints%n",
                    page, systemCount, sha256File(schedulerFixture), sha256File(expandFixture),
                    sha256File(createStemFixture), sha256File(reuseCheckFixture), OMR.gui == null);
        }

        final Totals totals = new Totals();
        final SystemInfo system = systemById(sheet, wantedSystem);
        runSystem(
                page, sheet, system, expected.get(wantedSystem),
                wantedSystem == 1 ? "sheet-first" : "isolated-system-frontier", totals);
        System.exit(0);
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
                bAliases.put(b, bAlias);
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
                bAliases, cAliases, headOrdinals, planRefs, inspectionBeams, heads, allLinkers, expected,
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
            emitBaseApplyRows(
                    run, ref.plan, executionMode, boundaryBefore, predecessorCompat, noStaff,
                    initialAlias, finalAlias, finalStem, reuse, beamIndex, hSide, b, vSide,
                    headToBeam, ref, stemProfile, linkProfile, lastIndex, relations,
                    glyphs, selectedGlyphs, candidateObjectIdBefore, registered,
                    registration, disposition, relationInputHashBefore);

            final PersistentSnapshot boundaryAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            assertRealBaseApplyDelta(boundaryBefore, boundaryAfter, beam, finalStem, link);
            liveStems.assertUnchanged();
            if (!relationInputHashBefore.equals(relationInputHash(relations, cAliases))) {
                throw new IllegalStateException("base apply mutated head relation input map");
            }

            totals.transactions++;
            totals.newVertexBranches += run.newVertex ? 1 : 0;
            totals.existingVertexBranches += run.newVertex ? 0 : 1;
            totals.edgesAdded += Boolean.TRUE.equals(run.applyReturned) ? 1 : 0;
            totals.chordMatches += run.stemIncidents.chordMatches;
            if (run.thrown != null) totals.throwsSeen++;

            if (system.getId() == 1) {
                final PersistentSnapshot realBeforeSynthetic = snapshot(
                        sheet, retriever, inspectionBeams, heads, allLinkers);
                emitSyntheticBaseApplyCoverage(
                        beam, initialStem, headToBeam, stemProfile, ref.plan);
                final PersistentSnapshot realAfterSynthetic = snapshot(
                        sheet, retriever, inspectionBeams, heads, allLinkers);
                realBeforeSynthetic.assertSame(realAfterSynthetic);
            }
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

        void emitBaseApplyRows (ApplyRun run,
                                int plan,
                                String executionMode,
                                PersistentSnapshot persistentBefore,
                                PredecessorCompatSnapshot predecessorCompat,
                                String noStaff,
                                String initialAlias,
                                String finalAlias,
                                StemInter finalStem,
                                ReuseRun reuse,
                                int beamIndex,
                                HorizontalSide hSide,
                                Object b,
                                VerticalSide vSide,
                                VerticalSide headToBeam,
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
                                String relationInputHashBefore)
        {
            final ApplyMoment before = run.before;
            final ApplyMoment afterVertex = run.afterVertex;
            final ApplyMoment after = run.after;
            final int candidateId = before.allocator + 1;
            final Inter candidateInter = interAtId(before.inters, candidateId);
            final Glyph candidateGlyph = glyphActiveAtId(before.glyphs, candidateId);
            final Glyph candidateOriginal = glyphOriginalAtId(before.glyphs, candidateId);
            final boolean vipConfigured = sheet.getInterIndex().isVipId(candidateId);
            final String scope = run.scope;
            final String caseName = run.caseName;
            final BeamStemRelation draft = (BeamStemRelation) run.link.relation;
            System.out.printf(
                    "stemsbeamvlinkbaseapplybaseline %s system %d plan %d scope %s case %s "
                            + "executionMode %s allocator %d nextSharedId %d "
                            + "nextIdInter %s nextIdGlyphActive %s nextIdGlyphOriginal %s "
                            + "nextIdVipConfigured %s interIndex %d interIndexHash %s "
                            + "interIndexScanned %d interIndexBeamIndexOrdinal %s "
                            + "interIndexBeamObjectMatches %d interIndexBeamIdMatches %d "
                            + "glyphActiveBeamIdMatches %d glyphOriginalBeamIdMatches %d "
                            + "interIndexStemIndexOrdinal %s interIndexStemObjectMatches %d "
                            + "interIndexStemIdMatches %d glyphActiveStemIdMatches %d "
                            + "glyphOriginalStemIdMatches %d interIndexGeneratedNextIdMatches %d "
                            + "glyphActive %d glyphActiveHash %s glyphOriginals %d "
                            + "glyphOriginalsHash %s sigVertices %d sigVertexHash %s sigEdges %d "
                            + "sigEdgeHash %s sigVertexScanned %d sigBeamObjectMatches %d "
                            + "sigBeamVertexOrdinal %s sigStemObjectMatches %d "
                            + "sigStemVertexOrdinal %s beamAlias %s beamInterId %d beamClass %s "
                            + "beamAbnormal %s beamVip %s beamRemoved %s beamGroupVertexOrdinal %s "
                            + "beamGroupStateHash %s beamStateHash %s stemAlias %s stemInterId %d "
                            + "stemSigAttached %s stemVip %s stemRemoved %s stemAbnormal %s "
                            + "stemStateHash %s stubModified %s bookModifiedRaw %s bookDirty %s "
                            + "listenerHash %s noStaff %s systemStemsHash %s linkerStateHash %s "
                            + "lineStateHash %s relationInputHash %s%n",
                    page, system.getId(), plan, scope, caseName, executionMode,
                    before.allocator, candidateId,
                    candidateInter != null ? "inter:" + candidateInter.getId() : "-",
                    candidateGlyph != null ? "glyph:" + candidateGlyph.getId() : "-",
                    candidateOriginal != null ? "glyph:" + candidateOriginal.getId() : "-",
                    vipConfigured, before.inters.identities.size(), before.inters.hash,
                    before.inters.identities.size(), before.inters.indexOrdinal(run.beam),
                    before.inters.objectMatches(run.beam),
                    before.inters.idMatches(run.beam.getId()),
                    before.glyphs.activeIdMatches(run.beam.getId()),
                    before.glyphs.originalIdMatches(run.beam.getId()),
                    before.inters.indexOrdinal(run.stem), before.inters.objectMatches(run.stem),
                    before.inters.idMatches(before.stem.id),
                    before.glyphs.activeIdMatches(before.stem.id),
                    before.glyphs.originalIdMatches(before.stem.id),
                    before.inters.idMatches(candidateId),
                    before.glyphs.active.size(), before.glyphs.activeHash,
                    before.glyphs.originals.size(), before.glyphs.originalsHash,
                    before.graph.vertices.size(), before.graph.vertexHash,
                    before.graph.edges.size(), before.graph.edgeHash,
                    before.graph.vertices.size(), before.graph.vertexObjectMatches(run.beam),
                    before.graph.vertexOrdinal(run.beam),
                    before.graph.vertexObjectMatches(run.stem),
                    before.graph.vertexOrdinal(run.stem), run.beamAlias,
                    run.beam.getId(), run.beam.getClass().getSimpleName(),
                    before.beam.abnormal, run.beam.isVip(), run.beam.isRemoved(),
                    beamGroupOrdinal(run.beam, before.graph),
                    beamGroupStateHash(run.beam, before.graph),
                    sha256Rows(List.of(before.beam.stableValue)),
                    run.stemAlias, before.stem.id, before.stem.sig != null,
                    before.stem.vip, before.stem.removed, before.stem.abnormal,
                    sha256Rows(List.of(before.stem.value)), before.dirty.stubModified,
                    before.dirty.bookModified, before.dirty.bookDirty, before.listeners.hash(),
                    noStaff, persistentBefore.systemStems.hash, persistentBefore.linkers.hash,
                    persistentBefore.lines.hash, relationInputHashBefore);
            System.out.printf(
                    "stemsbeamvlinkbaseapplypredecessorcompat %s system %d plan %d "
                            + "scope real case - phase Before algorithm Boundary13FrozenV1 "
                            + "legacyInterIndexHash %s legacySigVertices %d legacySigEdges %d "
                            + "legacySigHash %s legacySigRelationStateHash %s "
                            + "legacySystemStems %d legacySystemStemsHash %s "
                            + "legacyLinkerStateHash %s%n",
                    page, system.getId(), plan, predecessorCompat.interIndexHash,
                    predecessorCompat.sigVertices, predecessorCompat.sigEdges,
                    predecessorCompat.sigHash, predecessorCompat.sigRelationStateHash,
                    predecessorCompat.systemStems, predecessorCompat.systemStemsHash,
                    predecessorCompat.linkerStateHash);
            System.out.printf(
                    "stemsbeamvlinkbaseapplyfrontier %s system %d plan %d scope %s case %s "
                            + "beamOrder %d beamSig %d hSide %s bAlias %s vSide %s "
                            + "headToBeam %s builder %d stemProfile %d linkProfile %d "
                            + "lastIndex %d relationCount %d glyphCount %d selectedGlyphRefs %s "
                            + "createCandidateObjectIdBefore %d createRegistration %s "
                            + "createDisposition %s createRegisteredAlias glyph:%d "
                            + "initialStemAlias %s finalStemAlias %s reuseOutcome %s "
                            + "realReuse %s freshLinkAlias %s partnerAlias %s "
                            + "partnerInterId %d outgoing %s relationClass %s portion %s dx %s "
                            + "dy %s grade %s impacts %s extension %s predecessorJoin Exact "
                            + "predecessorTerminal ReadyBeforeSigMutation%n",
                    page, system.getId(), plan, scope, caseName, beamIndex,
                    beamSigOrdinals.get(run.beam), hSide, bAliases.get(b), vSide, headToBeam,
                    ref.builder, stemProfile, linkProfile, lastIndex, relations.size(),
                    glyphs.size(), list(selectedGlyphs), candidateObjectIdBefore, registration,
                    disposition, registered.getId(), initialAlias, finalAlias, reuse.outcome,
                    finalStem != null && !initialAlias.equals(finalAlias), draftAlias(run, plan), finalAlias,
                    run.before.stem.id, run.link.outgoing, draft.getClass().getSimpleName(),
                    draft.getBeamPortion(), hex(draft.getDx()), hex(draft.getDy()),
                    hex(draft.getGrade()), impactsToken(draft.getImpacts()),
                    point(draft.getExtensionPoint()));
            emitApplyCoreRows(run, plan);
            System.out.printf(
                    "stemsbeamvlinkbaseapplydeltaguard %s system %d plan %d scope %s case %s "
                            + "allocatorBefore %d allocatorAfter %d interIndexHashBefore %s "
                            + "interIndexHashAfter %s glyphActiveHashBefore %s glyphActiveHashAfter %s "
                            + "glyphOriginalsHashBefore %s glyphOriginalsHashAfter %s "
                            + "sigVertexHashBefore %s sigVertexHashAfter %s sigEdgeHashBefore %s "
                            + "sigEdgeHashAfter %s stemGeometryUnchanged true beamGeometryUnchanged true "
                            + "beamGroupUnchanged true beamGroupVertexOrdinalBefore %s "
                            + "beamGroupVertexOrdinalAfter %s beamGroupStateHashBefore %s "
                            + "beamGroupStateHashAfter %s glyphRegistriesUnchanged true "
                            + "systemStemsMapIdentityUnchanged true linesUnchanged true "
                            + "linkerFlagsUnchanged true "
                            + "headRelationsUnchanged true siblingBeamRelationsUnchanged true "
                            + "allowedMutation VertexAndBaseBeamStemOnly "
                            + "stopBeforeBLinkerFlagMutation true stopBeforeSiblingBeamLinks true "
                            + "stopBeforeHeadRelationLoop true%n",
                    page, system.getId(), plan, scope, caseName, before.allocator,
                    after.allocator, before.inters.hash, after.inters.hash,
                    before.glyphs.activeHash, after.glyphs.activeHash,
                    before.glyphs.originalsHash, after.glyphs.originalsHash,
                    before.graph.vertexHash, after.graph.vertexHash,
                    before.graph.edgeHash, after.graph.edgeHash,
                    beamGroupOrdinal(run.beam, before.graph),
                    beamGroupOrdinal(run.beam, after.graph),
                    beamGroupStateHash(run.beam, before.graph),
                    beamGroupStateHash(run.beam, after.graph));
            System.out.printf(
                    "stemsbeamvlinkbaseapplysummary %s system %d plan %d scope %s case %s "
                            + "branch %s vertexAdded %s applyReturned %s edgeAdded %s "
                            + "chordMatches %d beamRule %s terminal %s throwStage %s%n",
                    page, system.getId(), plan, scope, caseName,
                    run.newVertex ? "NewIdZero" : "ExistingPositive",
                    !before.graph.vertexOrdinals.containsKey(run.stem)
                            && after.graph.vertexOrdinals.containsKey(run.stem),
                    boolToken(run.applyReturned),
                    run.after.graph.edgeOrdinals.containsKey(run.link.relation),
                    run.stemIncidents.chordMatches, run.beamIncidents.rule,
                    terminal(run), run.throwStage);
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

        void emitSyntheticBaseApplyCoverage (AbstractBeamInter realBeam,
                                             StemInter realStem,
                                             VerticalSide headToBeam,
                                             int profile,
                                             int plan)
            throws Exception
        {
            runSyntheticBaseApplyCase(
                    newBaseApplySyntheticFixture(
                            "ExistingFullSuccess", realBeam, realStem, headToBeam,
                            profile, false, "ExistingAttached"),
                    "synthetic", "ExistingFullSuccess", plan, FailureMode.NONE);
            runSyntheticBaseApplyCase(
                    newBaseApplySyntheticFixture(
                            "ExistingHookSuccess", realBeam, realStem, headToBeam,
                            profile, true, "ExistingAttached"),
                    "synthetic", "ExistingHookSuccess", plan, FailureMode.NONE);

            BaseApplySyntheticFixture fixture = newBaseApplySyntheticFixture(
                    "ExistingDuplicateSuppress", realBeam, realStem, headToBeam,
                    profile, false, "ExistingAttached");
            if (!fixture.link().applyTo(fixture.beam())) {
                throw new IllegalStateException("synthetic duplicate setup relation was not added");
            }
            clearSyntheticDirty(fixture);
            fixture = withFreshSyntheticLink(fixture);
            runSyntheticBaseApplyCase(
                    fixture, "synthetic", "ExistingDuplicateSuppress", plan,
                    FailureMode.NONE);

            fixture = newBaseApplySyntheticFixture(
                    "SourceRemovedSuppress", realBeam, realStem, headToBeam,
                    profile, false, "ExistingAttached");
            fixture.beam().remove(false);
            clearSyntheticDirty(fixture);
            runSyntheticBaseApplyCase(
                    fixture, "synthetic", "SourceRemovedSuppress", plan,
                    FailureMode.NONE);

            fixture = newBaseApplySyntheticFixture(
                    "TargetRemovedSuppress", realBeam, realStem, headToBeam,
                    profile, false, "ExistingAttached");
            fixture.stem().remove(false);
            clearSyntheticDirty(fixture);
            runSyntheticBaseApplyCase(
                    fixture, "synthetic", "TargetRemovedSuppress", plan,
                    FailureMode.NONE);

            fixture = newBaseApplySyntheticFixture(
                    "MissingPositiveTargetThrows", realBeam, realStem, headToBeam,
                    profile, false, "MissingPositive");
            runSyntheticBaseApplyCase(
                    fixture, "envelope", "MissingPositiveTargetThrows", plan,
                    FailureMode.NONE);

            fixture = newBaseApplySyntheticFixture(
                    "NewVertexListenerThrows", realBeam, realStem, headToBeam,
                    profile, false, "NewIdZero");
            installSyntheticListener(
                    fixture.sig(), new ThrowingGraphListener(ThrowEvent.VERTEX_ADDED), true);
            runSyntheticBaseApplyCase(
                    fixture, "envelope", "NewVertexListenerThrows", plan,
                    FailureMode.VERTEX_LISTENER);

            fixture = newBaseApplySyntheticFixture(
                    "EarlyEdgeListenerThrows", realBeam, realStem, headToBeam,
                    profile, false, "ExistingAttached");
            installSyntheticListener(
                    fixture.sig(), new ThrowingGraphListener(ThrowEvent.EDGE_ADDED), true);
            runSyntheticBaseApplyCase(
                    fixture, "envelope", "EarlyEdgeListenerThrows", plan,
                    FailureMode.EARLY_EDGE_LISTENER);

            fixture = newBaseApplySyntheticFixture(
                    "LaterEdgeListenerThrows", realBeam, realStem, headToBeam,
                    profile, false, "ExistingAttached");
            installSyntheticListener(
                    fixture.sig(), new ThrowingGraphListener(ThrowEvent.EDGE_ADDED), false);
            runSyntheticBaseApplyCase(
                    fixture, "envelope", "LaterEdgeListenerThrows", plan,
                    FailureMode.LATER_EDGE_LISTENER);
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

        void installSyntheticListener (SIGraph sig,
                                       GraphListener<Inter, Relation> listener,
                                       boolean beforeSigListener)
        {
            final List<GraphListener<Inter, Relation>> listeners =
                    (List<GraphListener<Inter, Relation>>) (List<?>) get(GRAPH_LISTENERS, sig);
            if (!ListenerTopology.capture(sig).isStandard()) {
                throw new IllegalStateException("synthetic listener installed on nonstandard SIG");
            }
            if (beforeSigListener) listeners.add(0, listener);
            else listeners.add(listener);
        }

        void runSyntheticBaseApplyCase (BaseApplySyntheticFixture fixture,
                                        String scope,
                                        String caseName,
                                        int plan,
                                        FailureMode failure)
            throws Exception
        {
            final PersistentSnapshot realBefore = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final ApplyRun run = executeBaseApply(
                    scope, caseName, fixture.beam(), "synthetic-stem:" + caseName,
                    fixture.stem(), fixture.link(), failure);
            final PersistentSnapshot realAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            realBefore.assertSame(realAfter);
            assertSyntheticBaseApply(run);
            emitSyntheticBaseApplyRows(
                    run, plan, fixture, realBefore.provenanceHash(), realAfter.provenanceHash());
            totals.transactions++;
            totals.newVertexBranches += run.newVertex ? 1 : 0;
            totals.existingVertexBranches += run.newVertex ? 0 : 1;
            totals.edgesAdded += run.after.graph.edgeOrdinals.containsKey(run.link.relation) ? 1 : 0;
            totals.chordMatches += run.stemIncidents.chordMatches;
            if (run.thrown != null) totals.throwsSeen++;
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

        void emitSyntheticBaseApplyRows (ApplyRun run,
                                         int plan,
                                         BaseApplySyntheticFixture fixture,
                                         String enclosingHashBefore,
                                         String enclosingHashAfter)
        {
            final ApplyMoment before = run.before;
            final ApplyMoment after = run.after;
            final int candidateId = before.allocator + 1;
            final Inter candidateInter = interAtId(before.inters, candidateId);
            final Glyph candidateGlyph = glyphActiveAtId(before.glyphs, candidateId);
            final Glyph candidateOriginal = glyphOriginalAtId(before.glyphs, candidateId);
            final boolean vipConfigured = fixture.sheet().getInterIndex().isVipId(candidateId);
            final BeamStemRelation draft = (BeamStemRelation) run.link.relation;
            System.out.printf(
                    "stemsbeamvlinkbaseapplybaseline %s system %d plan %d scope %s case %s "
                            + "executionMode IsolatedSyntheticSheet allocator %d nextSharedId %d "
                            + "nextIdInter %s nextIdGlyphActive %s nextIdGlyphOriginal %s "
                            + "nextIdVipConfigured %s interIndex %d interIndexHash %s "
                            + "interIndexScanned %d interIndexBeamIndexOrdinal %s "
                            + "interIndexBeamObjectMatches %d interIndexBeamIdMatches %d "
                            + "glyphActiveBeamIdMatches %d glyphOriginalBeamIdMatches %d "
                            + "interIndexStemIndexOrdinal %s interIndexStemObjectMatches %d "
                            + "interIndexStemIdMatches %d glyphActiveStemIdMatches %d "
                            + "glyphOriginalStemIdMatches %d interIndexGeneratedNextIdMatches %d "
                            + "glyphActive %d glyphActiveHash %s glyphOriginals %d "
                            + "glyphOriginalsHash %s sigVertices %d sigVertexHash %s sigEdges %d "
                            + "sigEdgeHash %s sigVertexScanned %d sigBeamObjectMatches %d "
                            + "sigBeamVertexOrdinal %s sigStemObjectMatches %d "
                            + "sigStemVertexOrdinal %s beamAlias %s beamInterId %d beamClass %s "
                            + "beamAbnormal %s beamVip %s beamRemoved %s beamGroupVertexOrdinal %s "
                            + "beamGroupStateHash %s beamStateHash %s stemAlias %s stemInterId %d "
                            + "stemSigAttached %s stemVip %s stemRemoved %s stemAbnormal %s "
                            + "stemStateHash %s stubModified %s bookModifiedRaw %s bookDirty %s "
                            + "listenerHash %s noStaff NotApplicable systemStemsHash NotApplicable "
                            + "linkerStateHash NotApplicable lineStateHash NotApplicable "
                            + "relationInputHash %s%n",
                    page, system.getId(), plan, run.scope, run.caseName, before.allocator,
                    candidateId, candidateInter != null ? "inter:" + candidateInter.getId() : "-",
                    candidateGlyph != null ? "glyph:" + candidateGlyph.getId() : "-",
                    candidateOriginal != null ? "glyph:" + candidateOriginal.getId() : "-",
                    vipConfigured, before.inters.identities.size(), before.inters.hash,
                    before.inters.identities.size(), before.inters.indexOrdinal(run.beam),
                    before.inters.objectMatches(run.beam),
                    before.inters.idMatches(run.beam.getId()),
                    before.glyphs.activeIdMatches(run.beam.getId()),
                    before.glyphs.originalIdMatches(run.beam.getId()),
                    before.inters.indexOrdinal(run.stem), before.inters.objectMatches(run.stem),
                    before.inters.idMatches(before.stem.id),
                    before.glyphs.activeIdMatches(before.stem.id),
                    before.glyphs.originalIdMatches(before.stem.id),
                    before.inters.idMatches(candidateId),
                    before.glyphs.active.size(), before.glyphs.activeHash,
                    before.glyphs.originals.size(), before.glyphs.originalsHash,
                    before.graph.vertices.size(), before.graph.vertexHash,
                    before.graph.edges.size(), before.graph.edgeHash,
                    before.graph.vertices.size(), before.graph.vertexObjectMatches(run.beam),
                    before.graph.vertexOrdinal(run.beam),
                    before.graph.vertexObjectMatches(run.stem),
                    before.graph.vertexOrdinal(run.stem), run.beamAlias,
                    run.beam.getId(), run.beam.getClass().getSimpleName(), before.beam.abnormal,
                    run.beam.isVip(), run.beam.isRemoved(),
                    beamGroupOrdinal(run.beam, before.graph),
                    beamGroupStateHash(run.beam, before.graph),
                    sha256Rows(List.of(before.beam.stableValue)), run.stemAlias, before.stem.id,
                    before.stem.sig == fixture.sig(), before.stem.vip, before.stem.removed,
                    before.stem.abnormal, sha256Rows(List.of(before.stem.value)),
                    before.dirty.stubModified, before.dirty.bookModified,
                    before.dirty.bookDirty, before.listeners.hash(),
                    sha256Rows(List.of(run.relationBefore)));
            System.out.printf(
                    "stemsbeamvlinkbaseapplyfrontier %s system %d plan %d scope %s case %s "
                            + "beamOrder %s beamSig %s hSide NotApplicable bAlias NotApplicable "
                            + "vSide %s headToBeam %s builder NotApplicable stemProfile %d "
                            + "linkProfile NotApplicable lastIndex NotApplicable relationCount 0 "
                            + "glyphCount 0 selectedGlyphRefs [] createCandidateObjectIdBefore 0 "
                            + "createRegistration NotApplicable createDisposition NotApplicable "
                            + "createRegisteredAlias - initialStemAlias %s finalStemAlias %s "
                            + "reuseOutcome IsolatedSynthetic realReuse false freshLinkAlias %s "
                            + "partnerAlias %s partnerInterId %d outgoing %s relationClass %s "
                            + "portion %s dx %s dy %s grade %s impacts %s extension %s "
                            + "predecessorJoin IsolatedSyntheticCheckLink "
                            + "predecessorTerminal ReadyBeforeSigMutation%n",
                    page, system.getId(), plan, run.scope, run.caseName,
                    before.graph.vertexOrdinal(run.beam), before.graph.vertexOrdinal(run.beam),
                    fixture.headToBeam().opposite(), fixture.headToBeam(), fixture.profile(),
                    run.stemAlias, run.stemAlias, draftAlias(run, plan), run.stemAlias,
                    before.stem.id, run.link.outgoing, draft.getClass().getSimpleName(),
                    draft.getBeamPortion(), hex(draft.getDx()), hex(draft.getDy()),
                    hex(draft.getGrade()), impactsToken(draft.getImpacts()),
                    point(draft.getExtensionPoint()));
            emitApplyCoreRows(run, plan);
            System.out.printf(
                    "stemsbeamvlinkbaseapplydeltaguard %s system %d plan %d scope %s case %s "
                            + "observedSheet IsolatedSyntheticSheet enclosingRealSheetHashBefore %s "
                            + "enclosingRealSheetHashAfter %s enclosingRealSheetUnchanged %s "
                            + "allocatorBefore %d allocatorAfter %d interIndexHashBefore %s "
                            + "interIndexHashAfter %s glyphActiveHashBefore %s glyphActiveHashAfter %s "
                            + "glyphOriginalsHashBefore %s glyphOriginalsHashAfter %s "
                            + "sigVertexHashBefore %s sigVertexHashAfter %s sigEdgeHashBefore %s "
                            + "sigEdgeHashAfter %s stemGeometryUnchanged true beamGeometryUnchanged true "
                            + "beamGroupUnchanged true beamGroupVertexOrdinalBefore %s "
                            + "beamGroupVertexOrdinalAfter %s beamGroupStateHashBefore %s "
                            + "beamGroupStateHashAfter %s glyphRegistriesUnchanged true "
                            + "systemStemsMapIdentityUnchanged NotApplicable linesUnchanged NotApplicable "
                            + "linkerFlagsUnchanged NotApplicable headRelationsUnchanged true "
                            + "siblingBeamRelationsUnchanged true allowedMutation %s "
                            + "stopBeforeBLinkerFlagMutation true stopBeforeSiblingBeamLinks true "
                            + "stopBeforeHeadRelationLoop true%n",
                    page, system.getId(), plan, run.scope, run.caseName, enclosingHashBefore,
                    enclosingHashAfter, enclosingHashBefore.equals(enclosingHashAfter),
                    before.allocator, after.allocator, before.inters.hash, after.inters.hash,
                    before.glyphs.activeHash, after.glyphs.activeHash,
                    before.glyphs.originalsHash, after.glyphs.originalsHash,
                    before.graph.vertexHash, after.graph.vertexHash,
                    before.graph.edgeHash, after.graph.edgeHash,
                    beamGroupOrdinal(run.beam, before.graph),
                    beamGroupOrdinal(run.beam, after.graph),
                    beamGroupStateHash(run.beam, before.graph),
                    beamGroupStateHash(run.beam, after.graph), syntheticAllowedMutation(run));
            System.out.printf(
                    "stemsbeamvlinkbaseapplysummary %s system %d plan %d scope %s case %s "
                            + "branch %s vertexAdded %s applyReturned %s edgeAdded %s "
                            + "chordMatches %d beamRule %s terminal %s throwStage %s%n",
                    page, system.getId(), plan, run.scope, run.caseName,
                    run.newVertex ? "NewIdZero" : "ExistingPositive",
                    !before.graph.vertexOrdinals.containsKey(run.stem)
                            && after.graph.vertexOrdinals.containsKey(run.stem),
                    boolToken(run.applyReturned), after.graph.edgeOrdinals.containsKey(run.link.relation),
                    run.stemIncidents.chordMatches, run.beamIncidents.rule,
                    terminal(run), run.throwStage);
        }

        String syntheticAllowedMutation (ApplyRun run)
        {
            if (run.failureMode == FailureMode.VERTEX_LISTENER) {
                return "AllocatorIndexVertexPrefixBeforeSigListener";
            }
            if (run.failureMode == FailureMode.EARLY_EDGE_LISTENER) {
                return "EdgePrefixBeforeSigRelationCallback";
            }
            if (run.failureMode == FailureMode.LATER_EDGE_LISTENER) {
                return "EdgeAndCallbackPrefixBeforeLaterListener";
            }
            if (run.thrown != null) return "NoneBeforeMissingEndpointFailure";
            if (Boolean.TRUE.equals(run.applyReturned)) return "BaseBeamStemOnly";
            return "None";
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

        void emitApplyCoreRows (ApplyRun run,
                                int plan)
        {
            final ApplyMoment before = run.before;
            final ApplyMoment afterVertex = run.afterVertex;
            final ApplyMoment after = run.after;
            final int assignedId = run.newVertex ? run.stem.getId() : 0;
            final boolean vipRead = run.newVertex;
            final boolean vipConfigured = vipRead
                    && run.beam.getSig().getSystem().getSheet().getInterIndex().isVipId(assignedId);
            System.out.printf(
                    "stemsbeamvlinkbaseapplylisteners %s system %d plan %d scope %s case %s "
                            + "vertexSetCount %d vertexSetClasses %s graphCount %d graphClasses %s "
                            + "standardSingleSigListener %s callbackDispatch SourceSigIdentity%n",
                    page, system.getId(), plan, run.scope, run.caseName,
                    before.listeners.vertexSet.size(), list(before.listeners.vertexSet),
                    before.listeners.graph.size(), list(before.listeners.graph),
                    before.listeners.isStandard());
            System.out.printf(
                    "stemsbeamvlinkbaseapplyvertextrace %s system %d plan %d scope %s case %s "
                            + "branch %s idRead %d callAttempted %s registerAttempted %s "
                            + "allocatorBefore %d allocatorAfter %d assignedId %s "
                            + "vipMembershipRead %s vipConfigured %s stemVipBefore %s stemVipAfter %s "
                            + "interIndexBefore %d interIndexAfter %d vertexCountBefore %d "
                            + "vertexCountAfter %d interIndexScannedAfterVertex %d "
                            + "interIndexStemObjectMatchesAfterVertex %d "
                            + "interIndexAssignedIdMatchesAfterVertex %s "
                            + "sigVertexScannedAfterVertex %d sigBeamObjectMatchesAfterVertex %d "
                            + "sigStemObjectMatchesAfterVertex %d sigBeamVertexOrdinalAfterVertex %s "
                            + "sigStemVertexOrdinalAfterVertex %s vertexOrdinal %s vertexCallReturn %s "
                            + "vertexEventCompleted %s stemSigAttachedAfterVertex %s "
                            + "stemAddedCompleted %s "
                            + "stemRemovedBefore %s stemRemovedAfter %s abnormalRequest %s "
                            + "abnormalChanged %s stemAbnormalBefore %s stemAbnormalAfter %s "
                            + "stubBefore %s stubAfter %s bookModifiedBefore %s bookModifiedAfter %s "
                            + "bookDirtyBefore %s bookDirtyAfter %s throwStage %s throwClass %s%n",
                    page, system.getId(), plan, run.scope, run.caseName,
                    run.newVertex ? "NewIdZero" : "ExistingPositive", before.stem.id,
                    run.newVertex, run.newVertex, before.allocator, afterVertex.allocator,
                    run.newVertex ? Integer.toString(assignedId) : "-", vipRead, vipConfigured,
                    before.stem.vip, afterVertex.stem.vip,
                    before.inters.identities.size(), afterVertex.inters.identities.size(),
                    before.graph.vertices.size(), afterVertex.graph.vertices.size(),
                    afterVertex.inters.identities.size(),
                    afterVertex.inters.objectMatches(run.stem),
                    run.newVertex ? Integer.toString(afterVertex.inters.idMatches(assignedId))
                            : "NotRead",
                    afterVertex.graph.vertices.size(),
                    afterVertex.graph.vertexObjectMatches(run.beam),
                    afterVertex.graph.vertexObjectMatches(run.stem),
                    afterVertex.graph.vertexOrdinal(run.beam),
                    afterVertex.graph.vertexOrdinal(run.stem),
                    afterVertex.graph.vertexOrdinal(run.stem), boolToken(run.vertexReturned),
                    run.newVertex
                            ? Boolean.toString(!"VertexAdd".equals(run.throwStage)) : "NotRead",
                    run.stem.getSig() == run.beam.getSig(),
                    run.newVertex
                            ? Boolean.toString(!"VertexAdd".equals(run.throwStage)) : "NotRead",
                    before.stem.removed, afterVertex.stem.removed,
                    !run.newVertex ? "NotRead"
                            : "VertexAdd".equals(run.throwStage) ? "NotReached" : "true",
                    !run.newVertex ? "NotRead"
                            : "VertexAdd".equals(run.throwStage) ? "NotReached"
                                    : Boolean.toString(before.stem.abnormal
                                            != afterVertex.stem.abnormal),
                    before.stem.abnormal, afterVertex.stem.abnormal,
                    before.dirty.stubModified, afterVertex.dirty.stubModified,
                    before.dirty.bookModified, afterVertex.dirty.bookModified,
                    before.dirty.bookDirty, afterVertex.dirty.bookDirty,
                    "VertexAdd".equals(run.throwStage) ? run.throwStage : "-",
                    "VertexAdd".equals(run.throwStage) ? run.thrown.getClass().getName() : "-");
            final int unread = (int) run.duplicate.rows.stream()
                    .filter(row -> !row.classCheckRead).count();
            System.out.printf(
                    "stemsbeamvlinkbaseapplyapplydecision %s system %d plan %d scope %s case %s "
                            + "sourceAlias %s sourceInterId %d targetAlias %s targetInterId %d "
                            + "sourceRemovedRead %s sourceRemoved %s targetRemovedRead %s "
                            + "targetRemoved %s duplicateLookup %s sourceOutgoingScanned %d "
                            + "sourceOutgoingProvenanceSha256 %s "
                            + "pairEdges %d classChecksRead %d unreadSuffix %d firstMatchIdentity %s "
                            + "pairProvenanceSha256 %s "
                            + "firstMatchObjectIdentity %s "
                            + "action %s relationRuntimeClass %s%n",
                    page, system.getId(), plan, run.scope, run.caseName, run.beamAlias,
                    run.beam.getId(), run.stemAlias, run.stem.getId(),
                    run.duplicate.sourceRemovedRead, run.duplicate.sourceRemoved,
                    run.duplicate.targetRemovedRead, run.duplicate.targetRemoved,
                    run.duplicate.state, run.duplicate.sourceOutgoingScanned,
                    run.duplicate.sourceOutgoingHash,
                    run.duplicate.rows.size(), run.duplicate.rows.size() - unread, unread,
                    run.duplicate.firstMatch != null
                            ? before.graph.edgeAlias(run.duplicate.firstMatch) : "-",
                    run.duplicate.pairHash,
                    run.duplicate.firstMatch != null
                            ? relationObjectAlias(run, run.duplicate.firstMatch, plan) : "-",
                    run.duplicate.action, run.link.relation.getClass().getName());
            for (DuplicateRow row : run.duplicate.rows) {
                System.out.printf(
                        "stemsbeamvlinkbaseapplyduplicatescan %s system %d plan %d scope %s case %s "
                                + "pairOrdinal %d sourceOutgoingOrdinal %d globalRelationIdentity %s "
                                + "relationObjectIdentity %s relationClass %s classCheckRead %s "
                                + "matchesRuntimeClass %s "
                                + "action %s%n",
                        page, system.getId(), plan, run.scope, run.caseName, row.pairOrdinal,
                        row.sourceOutgoingOrdinal, row.globalIdentity,
                        relationObjectAlias(run, row.relation, plan),
                        row.relation.getClass().getName(), row.classCheckRead, row.matches,
                        row.action);
            }
            final boolean edgePresent = after.graph.edgeOrdinals.containsKey(run.link.relation);
            final String callbackOutcome = Boolean.TRUE.equals(run.applyReturned)
                    ? "Completed" : run.failureMode == FailureMode.LATER_EDGE_LISTENER && edgePresent
                            ? "CompletedBeforeLaterListenerThrow" : "NotRun";
            System.out.printf(
                    "stemsbeamvlinkbaseapplyedgestruct %s system %d plan %d scope %s case %s "
                            + "applyCalled %s applyReturn %s freshDraftIdentity %s "
                            + "draftInGraphBefore %s draftInGraphAfter %s graphRelationIdentity %s "
                            + "globalEdgeOrdinal %s sourceOutgoingOrdinal %s targetIncomingOrdinal %s "
                            + "existingRelationIdentity %s existingRelationObjectIdentity %s "
                            + "globalInserted %s sourceOutgoingInserted %s "
                            + "targetIncomingInserted %s edgeEventCompleted %s callbackOutcome %s "
                            + "throwStage %s throwClass %s throwMessageHash %s%n",
                    page, system.getId(), plan, run.scope, run.caseName,
                    !"NotReached".equals(run.duplicate.action), boolToken(run.applyReturned),
                    draftAlias(run, plan),
                    before.graph.edgeOrdinals.containsKey(run.link.relation), edgePresent,
                    after.graph.edgeAlias(run.link.relation),
                    edgePresent ? Integer.toString(after.graph.edgeOrdinals.get(run.link.relation)) : "-",
                    edgePresent ? Integer.toString(new ArrayList<>(
                            run.beam.getSig().outgoingEdgesOf(run.beam)).indexOf(run.link.relation)) : "-",
                    edgePresent ? Integer.toString(new ArrayList<>(
                            run.beam.getSig().incomingEdgesOf(run.stem)).indexOf(run.link.relation)) : "-",
                    run.duplicate.firstMatch != null
                            ? before.graph.edgeAlias(run.duplicate.firstMatch) : "-",
                    run.duplicate.firstMatch != null
                            ? relationObjectAlias(run, run.duplicate.firstMatch, plan) : "-",
                    edgePresent, edgePresent, edgePresent,
                    run.thrown == null && Boolean.TRUE.equals(run.applyReturned), callbackOutcome,
                    run.throwStage,
                    run.thrown != null ? run.thrown.getClass().getName() : "-",
                    run.thrown != null ? sha256Rows(List.of(String.valueOf(run.thrown.getMessage()))) : "-");
            emitStemIncidentRows(run, plan, "Before", run.stemIncidentsBefore);
            emitStemIncidentRows(run, plan, "AfterCallback", run.stemIncidents);
            System.out.printf(
                    "stemsbeamvlinkbaseapplyrelationcallback %s system %d plan %d scope %s case %s "
                            + "outcome %s relationBeforeHash %s relationAfterHash %s "
                            + "extensionBefore %s extensionAfter %s portionBefore %s portionAfter %s "
                            + "preStemScanState %s preStemIncidentCount %d "
                            + "preStemIncidentHash %s "
                            + "stemScanState %s stemIncidentCount %d stemIncidentHash %s "
                            + "chordMatches %d chordInvalidations %d beamCheckCalled %s "
                            + "preBeamScanState %s preBeamIncidentCount %d "
                            + "preBeamIncidentHash %s "
                            + "beamScanState %s beamRule %s beamIncidentCount %d beamIncidentHash %s "
                            + "left %s right %s requestedAbnormal %s beamAbnormalBefore %s "
                            + "beamAbnormalAfter %s beamAbnormalChanged %s stubBefore %s stubAfter %s "
                            + "bookModifiedBefore %s bookModifiedAfter %s bookDirtyBefore %s "
                            + "bookDirtyAfter %s%n",
                    page, system.getId(), plan, run.scope, run.caseName, callbackOutcome,
                    sha256Rows(List.of(run.relationBefore)), sha256Rows(List.of(run.relationAfter)),
                    run.extensionBefore,
                    point(((BeamStemRelation) run.link.relation).getExtensionPoint()),
                    run.portionBefore,
                    ((BeamStemRelation) run.link.relation).getBeamPortion(),
                    run.stemIncidentsBefore.state, run.stemIncidentsBefore.rows.size(),
                    run.stemIncidentsBefore.hash,
                    run.stemIncidents.state, run.stemIncidents.rows.size(),
                    run.stemIncidents.hash, run.stemIncidents.chordMatches,
                    run.stemIncidents.chordMatches,
                    callbackOutcome.startsWith("Completed"),
                    run.beamIncidentsBefore.state, run.beamIncidentsBefore.rows.size(),
                    run.beamIncidentsBefore.hash, run.beamIncidents.state,
                    run.beamIncidents.rule, run.beamIncidents.rows.size(),
                    run.beamIncidents.hash, run.beamIncidents.left, run.beamIncidents.right,
                    callbackOutcome.startsWith("Completed")
                            ? Boolean.toString(run.beamIncidents.requestedAbnormal) : "NotRead",
                    before.beam.abnormal, after.beam.abnormal,
                    before.beam.abnormal != after.beam.abnormal,
                    afterVertex.dirty.stubModified, after.dirty.stubModified,
                    afterVertex.dirty.bookModified, after.dirty.bookModified,
                    afterVertex.dirty.bookDirty, after.dirty.bookDirty);
            emitBeamIncidentRows(run, plan, "Before", run.beamIncidentsBefore);
            emitBeamIncidentRows(run, plan, "AfterCallback", run.beamIncidents);
            System.out.printf(
                    "stemsbeamvlinkbaseapplyresult %s system %d plan %d scope %s case %s "
                            + "terminal %s branch %s retainedDraftIdentity %s "
                            + "retainedDraftGrade %s applyReturn %s graphRelationIdentity %s "
                            + "existingRelationIdentity %s existingRelationObjectIdentity %s "
                            + "finalStemAlias %s finalStemInterId %d "
                            + "finalStemSigAttached %s finalStemVip %s finalStemAbnormal %s "
                            + "finalBeamAbnormal %s finalBeamVip %s finalBeamRemoved %s "
                            + "finalBeamGroupVertexOrdinal %s finalBeamGroupStateHash %s "
                            + "allocator %d interIndexHash %s sigVertexHash %s "
                            + "sigEdgeHash %s stubModified %s bookModifiedRaw %s bookDirty %s%n",
                    page, system.getId(), plan, run.scope, run.caseName, terminal(run),
                    run.newVertex ? "NewIdZero" : "ExistingPositive", draftAlias(run, plan),
                    hex(((Support) run.link.relation).getGrade()), boolToken(run.applyReturned),
                    after.graph.edgeAlias(run.link.relation),
                    run.duplicate.firstMatch != null
                            ? before.graph.edgeAlias(run.duplicate.firstMatch) : "-",
                    run.duplicate.firstMatch != null
                            ? relationObjectAlias(run, run.duplicate.firstMatch, plan) : "-",
                    run.stemAlias, run.stem.getId(), run.stem.getSig() == run.beam.getSig(),
                    run.stem.isVip(), run.stem.isAbnormal(), run.beam.isAbnormal(),
                    run.beam.isVip(), run.beam.isRemoved(),
                    beamGroupOrdinal(run.beam, after.graph),
                    beamGroupStateHash(run.beam, after.graph),
                    after.allocator, after.inters.hash, after.graph.vertexHash,
                    after.graph.edgeHash, after.dirty.stubModified,
                    after.dirty.bookModified, after.dirty.bookDirty);
        }

        void emitStemIncidentRows (ApplyRun run,
                                   int plan,
                                   String phase,
                                   StemIncidentScan scan)
        {
            for (IncidentRow row : scan.rows) {
                System.out.printf(
                        "stemsbeamvlinkbaseapplystemincident %s system %d plan %d scope %s case %s "
                                + "phase %s incidentOrdinal %d direction %s directionOrdinal %d "
                                + "globalRelationIdentity %s relationObjectIdentity %s "
                                + "relationClass %s oppositeAlias %s oppositeInterId %d "
                                + "oppositeVertexOrdinal %s chordStemMatch %s action %s%n",
                        page, system.getId(), plan, run.scope, run.caseName, phase,
                        row.incidentOrdinal, row.direction, row.directionOrdinal,
                        row.globalIdentity, relationObjectAlias(run, row.relation, plan),
                        row.relation.getClass().getName(), oppositeAlias(run, row.opposite),
                        row.opposite.getId(), row.oppositeVertexOrdinal, row.chordStemMatch,
                        phase.equals("AfterCallback") && row.chordStemMatch
                                ? "InvalidateChord" : "ObserveOnly");
            }
        }

        void emitBeamIncidentRows (ApplyRun run,
                                   int plan,
                                   String phase,
                                   BeamIncidentTrace scan)
        {
            for (BeamIncidentRow row : scan.rows) {
                System.out.printf(
                        "stemsbeamvlinkbaseapplybeamincident %s system %d plan %d scope %s case %s "
                                + "phase %s incidentOrdinal %d direction %s directionOrdinal %d "
                                + "globalRelationIdentity %s relationObjectIdentity %s "
                                + "relationClass %s oppositeAlias %s oppositeInterId %d "
                                + "oppositeVertexOrdinal %s readState %s relevant %s portion %s "
                                + "contribution %s%n",
                        page, system.getId(), plan, run.scope, run.caseName, phase,
                        row.incidentOrdinal, row.direction, row.directionOrdinal,
                        row.globalIdentity, relationObjectAlias(run, row.relation, plan),
                        row.relation.getClass().getName(), oppositeAlias(run, row.opposite),
                        row.opposite.getId(), row.oppositeVertexOrdinal, row.readState,
                        row.relevant, row.portion, row.contribution);
            }
        }

        String oppositeAlias (ApplyRun run,
                              Inter opposite)
        {
            if (opposite == run.beam) return run.beamAlias;
            if (opposite == run.stem) return run.stemAlias;
            return "inter:" + opposite.getId();
        }

        String draftAlias (ApplyRun run,
                           int plan)
        {
            return run.scope.equals("real")
                    ? "draft:" + plan
                    : run.scope + "-draft:" + run.caseName;
        }

        String relationObjectAlias (ApplyRun run,
                                    Relation relation,
                                    int plan)
        {
            if (relation == run.link.relation) return draftAlias(run, plan);
            String graphAlias = run.before.graph.edgeAlias(relation);
            if (graphAlias.equals("-")) graphAlias = run.after.graph.edgeAlias(relation);
            if (graphAlias.equals("-")) {
                throw new IllegalStateException("relation object lacks graph identity");
            }
            return "graph-object:" + graphAlias.substring("sig-edge:".length());
        }

        void emitCheckRows (String scope,
                            String syntheticCase,
                            int plan,
                            AbstractBeamInter beam,
                            String stemAlias,
                            StemInter stem,
                            CheckTrace t,
                            Link link)
        {
            System.out.printf(
                    "stemsbeamvlinkbaseapplycheckcontext %s system %d plan %d scope %s case %s "
                            + "beamMedian %s beamHeight %s vSide %s headToBeam %s borderSide %s "
                            + "beamBorder %s stemAlias %s stemInterId %d stemMedian %s "
                            + "stemActualWidth %s interline %d scaleStemThickness %d "
                            + "halfScaleStemWidth %s profile %d portionXInP0 %s "
                            + "xOutMax %s yMax %s maxDxInput %s maxDxRint %s maxDxInt %d "
                            + "xWeight %s yWeight %s intrinsicRatio %s minGrade %s "
                            + "portionComparison StrictLtGt gradeComparison InclusiveGe%n",
                    page, system.getId(), plan, scope, syntheticCase, line(beam.getMedian()),
                    hex(beam.getHeight()), t.vSide, t.headToBeam, t.borderSide, line(t.border),
                    stemAlias, stem.getId(), line(stem.getMedian()), hex(stem.getWidth()),
                    sheet.getScale().getInterline(), sheet.getScale().getStemThickness(),
                    hex(t.halfStemWidth), t.profile, hex(t.portionXInP0), hex(t.xMax), hex(t.yMax),
                    hex(t.maxDxInput), hex(t.maxDxRint), t.maxDx, hex(t.xWeight), hex(t.yWeight),
                    hex(t.intrinsicRatio), hex(t.minGrade));
            System.out.printf(
                    "stemsbeamvlinkbaseapplychecktrace %s system %d plan %d scope %s case %s "
                            + "intersectionDen %s v12 %s v34 %s xNumerator %s yNumerator %s "
                            + "crossX %s crossY %s leftThreshold %s rightThreshold %s "
                            + "portion %s signedXGapPixels %s signedDxFrac %s storedDx %s "
                            + "upperGapRaw %s upperGap %s lowerGapRaw %s lowerGap %s "
                            + "yGapPixels %s storedDy %s xImpactRaw %s yImpactRaw %s "
                            + "xImpact %s yImpact %s xPow %s yPow %s "
                            + "global %s totalWeight %s reciprocalWeight %s root %s "
                            + "intrinsicRatio %s grade %s candidateExtension %s accepted %s%n",
                    page, system.getId(), plan, scope, syntheticCase, hex(t.den), hex(t.v12),
                    hex(t.v34), hex(t.xNumerator), hex(t.yNumerator), hex(t.crossX), hex(t.crossY),
                    hex(t.leftThreshold), hex(t.rightThreshold), t.portion, hex(t.signedXGapPixels),
                    hex(t.signedDxFrac), hex(t.storedDx), hex(t.upperGapRaw), hex(t.upperGap),
                    hex(t.lowerGapRaw), hex(t.lowerGap), hex(t.yGapPixels), hex(t.storedDy),
                    hex(t.xImpactRaw), hex(t.yImpactRaw), hex(t.xImpact), hex(t.yImpact),
                    hex(t.xPow), hex(t.yPow), hex(t.global),
                    hex(t.totalWeight), hex(t.reciprocalWeight), hex(t.root),
                    hex(t.intrinsicRatio), hex(t.grade), point(t.candidateExtension), t.accepted);
            final BeamStemRelation relation = link != null
                    ? (BeamStemRelation) link.relation : null;
            System.out.printf(
                    "stemsbeamvlinkbaseapplycheckresult %s system %d plan %d scope %s case %s "
                            + "checkOutcome %s linkNull %s partnerAlias %s partnerInterId %s "
                            + "outgoing %s relationClass %s portion %s dx %s dy %s grade %s "
                            + "impacts %s extension %s terminal %s%n",
                    page, system.getId(), plan, scope, syntheticCase,
                    link != null ? "Accepted" : "Rejected", link == null,
                    link != null ? stemAlias : "-",
                    link != null ? Integer.toString(link.partner.getId()) : "-",
                    link != null ? Boolean.toString(link.outgoing) : "-",
                    relation != null ? relation.getClass().getSimpleName() : "-",
                    relation != null ? relation.getBeamPortion() : "-",
                    relation != null ? hex(relation.getDx()) : "-",
                    relation != null ? hex(relation.getDy()) : "-",
                    relation != null ? hex(relation.getGrade()) : "-",
                    relation != null ? impactsToken(relation.getImpacts()) : "-",
                    relation != null ? point(relation.getExtensionPoint()) : "-",
                    link != null ? "ReadyBeforeSigMutation" : "BeamStemRejected");
        }

        void emitSyntheticCoverage (AbstractBeamInter beam,
                                    StemInter initialStem,
                                    VerticalSide headToBeam,
                                    int profile,
                                    int plan)
            throws Exception
        {
            final SyntheticSigFixture fixture = isolatedSyntheticSig(initialStem);
            final LiveStemCatalogue liveStems = fixture.liveStems;
            final PersistentSnapshot realSyntheticBefore = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final String isolatedSigHashBefore = isolatedSigHash(fixture.sig);
            final String liveStemHashBefore = liveStems.hash();
            for (LiveStemRow row : liveStems.rows) {
                System.out.printf(
                        "stemsbeamvlinkbaseapplysyntheticlivestem %s system %d plan %d "
                                + "case SyntheticReuseTargets origin IsolatedSyntheticSig "
                                + "idNamespace isolated-1500000000 catalogOrdinal %d "
                                + "sigVertexOrdinal %d stemAlias %s javaInterId %d "
                                + "firstHeadStemEdgeOrdinal %s glyphId %d glyph %s grade %s "
                                + "median %s width %s bounds %s abnormal %s sigAttached %s "
                                + "stateHash %s%n",
                        page, system.getId(), plan, row.catalogOrdinal, row.sigVertexOrdinal,
                        row.alias, row.stem.getId(), row.firstHeadStemEdgeOrdinal,
                        row.stem.getGlyph().getId(), glyphToken(row.stem.getGlyph()),
                        hex(row.stem.getGrade()), line(row.stem.getMedian()),
                        hex(row.stem.getWidth()), rectangle(row.stem.getBounds()),
                        row.stem.isAbnormal(), row.stem.getSig() == fixture.sig,
                        stemStateHash(row.stem));
            }
            final StemInter reuseA = liveStems.ordered.get(0);
            final StemInter reuseB = liveStems.ordered.get(1);
            final LinkedHashMap<String, SyntheticReuseEntry> zero = new LinkedHashMap<>();
            zero.put("synthetic:c:0", new SyntheticReuseEntry(
                    true, HorizontalSide.LEFT, fixture.zeroHead));
            emitSyntheticReuseCase(
                    "ZeroMissingJavaNull", zero, initialStem, plan, beam,
                    headToBeam, profile, liveStems, fixture.edgeOrdinals);

            final LinkedHashMap<String, SyntheticReuseEntry> unique = new LinkedHashMap<>();
            unique.put("synthetic:c:0", new SyntheticReuseEntry(
                    true, HorizontalSide.LEFT, fixture.uniqueHead));
            unique.put("synthetic:c:1", new SyntheticReuseEntry(
                    true, HorizontalSide.RIGHT, fixture.unreadHead));
            emitSyntheticReuseCase(
                    "UniqueBreakUnreadSuffix", unique, initialStem, plan, beam,
                    headToBeam, profile, liveStems, fixture.edgeOrdinals);

            final LinkedHashMap<String, SyntheticReuseEntry> multiple = new LinkedHashMap<>();
            multiple.put("synthetic:c:0", new SyntheticReuseEntry(
                    true, HorizontalSide.LEFT, fixture.multipleHead));
            emitSyntheticReuseCase(
                    "MultipleContinue", multiple, initialStem, plan, beam,
                    headToBeam, profile, liveStems, fixture.edgeOrdinals);

            final Scale scale = sheet.getScale();
            final int maxDx = scale.toPixels(BeamStemRelation.getXInGapMaximum(0));
            final double leftThreshold = beam.getMedian().getX1() + maxDx;
            final double rightThreshold = beam.getMedian().getX2() - maxDx;
            final double[] xs = new double[]
            { Math.nextDown(leftThreshold), leftThreshold, rightThreshold, Math.nextUp(rightThreshold) };
            final String[] names = new String[]
            { "LeftBelow", "LeftExact", "RightExact", "RightAbove" };
            for (int i = 0; i < xs.length; i++) {
                final BeamPortion portion = BeamStemRelation.computeBeamPortion(beam, xs[i], scale);
                System.out.printf(
                        "stemsbeamvlinkbaseapplysyntheticportion %s system %d plan %d case %s "
                                + "xStem %s leftThreshold %s rightThreshold %s maxDx %d "
                                + "comparison StrictLtGt portion %s%n",
                        page, system.getId(), plan, names[i], hex(xs[i]), hex(leftThreshold),
                        hex(rightThreshold), maxDx, portion);
            }

            final BeamStemRelation threshold = new BeamStemRelation();
            threshold.setGrade(threshold.getMinGrade());
            System.out.printf(
                    "stemsbeamvlinkbaseapplysyntheticthreshold %s system %d plan %d "
                            + "grade %s minGrade %s comparison InclusiveGe accepted %s%n",
                    page, system.getId(), plan, hex(threshold.getGrade()),
                    hex(threshold.getMinGrade()), threshold.getGrade() >= threshold.getMinGrade());

            final Line2D border = beam.getBorder(headToBeam.opposite());
            Line2D parallel = null;
            for (double shift : new double[] { 1, -1, 2, -2, 4, -4, 8, -8, 16, -16 }) {
                final Line2D candidate = new Line2D.Double(
                        border.getX1(), border.getY1() + shift,
                        border.getX2(), border.getY2() + shift);
                final Point2D ignored = LineUtil.intersection(candidate, border);
                final double den = intersectionDen(candidate, border);
                if (den == 0.0 && !sameLine(candidate, border)
                        && (!Double.isFinite(ignored.getX()) || !Double.isFinite(ignored.getY()))) {
                    parallel = candidate;
                    break;
                }
            }
            if (parallel == null) {
                throw new IllegalStateException("could not construct exact distinct parallel line");
            }
            final StemInter parallelStem = new StemInter(null, 1.0);
            parallelStem.setWidth(initialStem.getWidth());
            parallelStem.setMedian(parallel.getP1(), parallel.getP2());
            final StemInterState parallelStateBefore = new StemInterState(parallelStem);
            final CheckTrace parallelTrace = CheckTrace.compute(
                    beam, parallelStem, headToBeam, scale, profile);
            final Link parallelLink = BeamStemRelation.checkLink(
                    beam, parallelStem, headToBeam, scale, profile);
            parallelTrace.assertActual(parallelLink, parallelStem);
            if (parallelLink != null || Double.isFinite(parallelTrace.crossX)
                    && Double.isFinite(parallelTrace.crossY)) {
                throw new IllegalStateException("parallel synthetic did not reject non-finite check");
            }
            if (!parallelStateBefore.same(parallelStem)) {
                throw new IllegalStateException("parallel synthetic stem mutated by checkLink");
            }
            emitCheckRows(
                    "synthetic", "DistinctParallelNonFinite", plan, beam,
                    "synthetic:parallel", parallelStem, parallelTrace, parallelLink);
            System.out.printf(
                    "stemsbeamvlinkbaseapplysyntheticparallel %s system %d plan %d "
                            + "stemMedian %s border %s den %s crossX %s crossY %s "
                            + "grade %s checkOutcome Rejected zeroMutation true%n",
                    page, system.getId(), plan, line(parallel), line(border),
                    hex(parallelTrace.den), hex(parallelTrace.crossX),
                    hex(parallelTrace.crossY), hex(parallelTrace.grade));

            // A fixed horizontal pair makes Java's exact determinant grouping observable:
            // the x quotient is infinite while the independently grouped 0/0 y quotient is NaN.
            final Line2D horizontalA = new Line2D.Double(0, 0, 2, 0);
            final Line2D horizontalB = new Line2D.Double(0, 1, 2, 1);
            final double x1 = horizontalA.getX1();
            final double y1 = horizontalA.getY1();
            final double x2 = horizontalA.getX2();
            final double y2 = horizontalA.getY2();
            final double x3 = horizontalB.getX1();
            final double y3 = horizontalB.getY1();
            final double x4 = horizontalB.getX2();
            final double y4 = horizontalB.getY2();
            final double horizontalDen = ((x1 - x2) * (y3 - y4))
                    - ((y1 - y2) * (x3 - x4));
            final double horizontalV12 = (x1 * y2) - (y1 * x2);
            final double horizontalV34 = (x3 * y4) - (y3 * x4);
            final double horizontalXNumerator = (horizontalV12 * (x3 - x4))
                    - ((x1 - x2) * horizontalV34);
            final double horizontalYNumerator = (horizontalV12 * (y3 - y4))
                    - ((y1 - y2) * horizontalV34);
            final Point2D horizontalCross = LineUtil.intersection(horizontalA, horizontalB);
            requireBits(
                    horizontalXNumerator / horizontalDen, horizontalCross.getX(),
                    "horizontal intersection x");
            requireBits(
                    horizontalYNumerator / horizontalDen, horizontalCross.getY(),
                    "horizontal intersection y");
            if (!Double.isInfinite(horizontalCross.getX())
                    || !Double.isNaN(horizontalCross.getY())) {
                throw new IllegalStateException("fixed horizontal intersection lost Inf/NaN split");
            }
            System.out.printf(
                    "stemsbeamvlinkbaseapplysyntheticintersection %s system %d plan %d "
                            + "case DistinctHorizontalInfNaN first %s second %s den %s "
                            + "v12 %s v34 %s xNumerator %s yNumerator %s crossX %s crossY %s "
                            + "finiteInputs true exactLineUtil true%n",
                    page, system.getId(), plan, line(horizontalA), line(horizontalB),
                    hex(horizontalDen), hex(horizontalV12), hex(horizontalV34),
                    hex(horizontalXNumerator), hex(horizontalYNumerator),
                    hex(horizontalCross.getX()), hex(horizontalCross.getY()));
            liveStems.assertUnchanged();
            final String isolatedSigHashAfter = isolatedSigHash(fixture.sig);
            final String liveStemHashAfter = liveStems.hash();
            final PersistentSnapshot realSyntheticAfter = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            realSyntheticBefore.assertSame(realSyntheticAfter);
            if (!isolatedSigHashBefore.equals(isolatedSigHashAfter)
                    || !liveStemHashBefore.equals(liveStemHashAfter)) {
                throw new IllegalStateException("isolated synthetic SIG mutated");
            }
            System.out.printf(
                    "stemsbeamvlinkbaseapplysyntheticguard %s system %d plan %d "
                            + "origin IsolatedSyntheticSig isolatedSigHashBefore %s "
                            + "isolatedSigHashAfter %s liveStemHashBefore %s liveStemHashAfter %s "
                            + "realAllocatorBefore %d realAllocatorAfter %d "
                            + "realInterIndexHashBefore %s realInterIndexHashAfter %s "
                            + "realSigHashBefore %s realSigHashAfter %s "
                            + "realSigRelationStateHashBefore %s realSigRelationStateHashAfter %s "
                            + "realSystemStemsHashBefore %s realSystemStemsHashAfter %s "
                            + "realLinkerStateHashBefore %s realLinkerStateHashAfter %s "
                            + "realLineStateHashBefore %s realLineStateHashAfter %s "
                            + "isolatedGraphUnchanged true realSheetUnchanged true "
                            + "allocatorUnchanged true interIndexUnchanged true sigUnchanged true "
                            + "relationsUnchanged true linkerFlagsUnchanged true zeroMutation true%n",
                    page, system.getId(), plan, isolatedSigHashBefore, isolatedSigHashAfter,
                    liveStemHashBefore, liveStemHashAfter, realSyntheticBefore.allocator,
                    realSyntheticAfter.allocator, realSyntheticBefore.inters.hash,
                    realSyntheticAfter.inters.hash, realSyntheticBefore.sig.hash,
                    realSyntheticAfter.sig.hash, realSyntheticBefore.sig.relationStateHash,
                    realSyntheticAfter.sig.relationStateHash,
                    realSyntheticBefore.systemStems.hash, realSyntheticAfter.systemStems.hash,
                    realSyntheticBefore.linkers.hash, realSyntheticAfter.linkers.hash,
                    realSyntheticBefore.lines.hash, realSyntheticAfter.lines.hash);
        }

        String isolatedSigHash (SIGraph sig)
        {
            final List<String> rows = new ArrayList<>();
            int vertexOrdinal = 0;
            for (Inter inter : sig.vertexSet()) {
                final Rectangle bounds = inter.getBounds();
                final String state = inter instanceof StemInter stem
                        ? stemStateHash(stem)
                        : inter.getClass().getSimpleName() + ":" + hex(inter.getGrade()) + ":"
                                + (bounds != null ? rectangle(bounds) : "-") + ":"
                                + inter.isAbnormal();
                rows.add("v:" + vertexOrdinal++ + ":" + inter.getId() + ":"
                        + (inter.getSig() == sig) + ":" + state);
            }
            int edgeOrdinal = 0;
            for (Relation relation : sig.edgeSet()) {
                rows.add("e:" + edgeOrdinal++ + ":" + sig.getEdgeSource(relation).getId()
                        + ":" + sig.getEdgeTarget(relation).getId() + ":"
                        + relation.getClass().getSimpleName() + ":" + relationState(relation));
            }
            return sha256Rows(rows);
        }

        void emitSyntheticReuseCase (String caseName,
                                     LinkedHashMap<String, SyntheticReuseEntry> entries,
                                     StemInter initialStem,
                                     int plan,
                                     AbstractBeamInter beam,
                                     VerticalSide headToBeam,
                                     int profile,
                                     LiveStemCatalogue liveStems,
                                     IdentityHashMap<Relation, Integer> sigEdgeOrdinals)
        {
            StemInter stem = initialStem;
            final StemInterState initialStateBefore = new StemInterState(initialStem);
            boolean stopped = false;
            boolean javaNull = false;
            int read = 0;
            int ordinal = 0;
            String selectedOrdinal = "-";
            for (Map.Entry<String, SyntheticReuseEntry> mapEntry : entries.entrySet()) {
                if (stopped || javaNull) {
                    System.out.printf(
                            "stemsbeamvlinkbaseapplysyntheticreuseentry %s system %d plan %d "
                                    + "case %s mapOrdinal %d cAlias %s conditionRead false "
                                    + "evidenceValidated false sLinked - parentHSide - "
                                    + "lookupState NotRead sideStemCount - sideStemAliases - "
                                    + "selectedStemAlias - action UnreadAfterBreak%n",
                            page, system.getId(), plan, caseName, ordinal, mapEntry.getKey());
                    ordinal++;
                    continue;
                }
                read++;
                final SyntheticReuseEntry input = mapEntry.getValue();
                if (!input.linked) {
                    System.out.printf(
                            "stemsbeamvlinkbaseapplysyntheticreuseentry %s system %d plan %d "
                                    + "case %s mapOrdinal %d cAlias %s conditionRead true "
                                    + "evidenceValidated true sLinked false parentHSide - "
                                    + "lookupState NotRead sideStemCount - sideStemAliases - "
                                    + "selectedStemAlias - action SkipUnlinked%n",
                            page, system.getId(), plan, caseName, ordinal, mapEntry.getKey());
                    ordinal++;
                    continue;
                }
                final HeadScan scan = HeadScan.capture(
                        input.head, input.parentSide, liveStems, sigEdgeOrdinals);
                final Map<HorizontalSide, Set<StemInter>> sideMap = input.head.getSideStems();
                scan.assertMap(sideMap);
                final Set<StemInter> stems = sideMap.get(input.parentSide);
                if (stems == null) {
                    // Exact Java failure point: stems.size() on the missing EnumMap key.
                    try {
                        stems.size();
                        throw new IllegalStateException("missing set unexpectedly readable");
                    } catch (NullPointerException expectedNull) {
                        javaNull = true;
                    }
                    System.out.printf(
                            "stemsbeamvlinkbaseapplysyntheticreuseentry %s system %d plan %d "
                                    + "case %s mapOrdinal %d cAlias %s conditionRead true "
                                    + "evidenceValidated true sLinked true parentHSide %s "
                                    + "headInterId %d isolatedSig true scanState Exhaustive "
                                    + "incidentEdges %d headSnapshotHash %s projectionHash %s "
                                    + "lookupState MissingJavaNull sideStemCount 0 sideStemAliases - "
                                    + "selectedStemAlias - action JavaNullPointer%n",
                            page, system.getId(), plan, caseName, ordinal, mapEntry.getKey(),
                            input.parentSide, input.head.getId(), scan.rows.size(),
                            scan.snapshotHash, scan.projectionHash);
                    emitSyntheticScanRows(caseName, plan, ordinal, scan);
                    ordinal++;
                    continue;
                }
                final List<String> aliases = new ArrayList<>();
                for (StemInter candidate : stems) {
                    aliases.add(liveStems.alias(candidate));
                }
                if (stems.size() == 1) {
                    stem = stems.iterator().next();
                    selectedOrdinal = Integer.toString(ordinal);
                    stopped = true;
                }
                System.out.printf(
                        "stemsbeamvlinkbaseapplysyntheticreuseentry %s system %d plan %d "
                                + "case %s mapOrdinal %d cAlias %s conditionRead true "
                                + "evidenceValidated true sLinked true parentHSide %s "
                                + "headInterId %d isolatedSig true scanState Exhaustive "
                                + "incidentEdges %d headSnapshotHash %s projectionHash %s "
                                + "lookupState Present sideStemCount %d sideStemAliases %s "
                                + "selectedStemAlias %s action %s%n",
                        page, system.getId(), plan, caseName, ordinal, mapEntry.getKey(),
                        input.parentSide, input.head.getId(), scan.rows.size(), scan.snapshotHash,
                        scan.projectionHash, stems.size(), list(aliases),
                        stems.size() == 1 ? aliases.get(0) : "-",
                        stems.size() == 1 ? "SelectBreak" : "ContinueMultiple");
                emitSyntheticScanRows(caseName, plan, ordinal, scan);
                ordinal++;
            }
            final String finalAlias = stemAlias(
                    stem, initialStem, plan, liveStems);
            String checkOutcome = "NotReachedJavaNull";
            if (!javaNull) {
                final StemInterState syntheticStateBefore = new StemInterState(stem);
                final CheckTrace trace = CheckTrace.compute(
                        beam, stem, headToBeam, sheet.getScale(), profile);
                final Link link = BeamStemRelation.checkLink(
                        beam, stem, headToBeam, sheet.getScale(), profile);
                trace.assertActual(link, stem);
                if (!syntheticStateBefore.same(stem)) {
                    throw new IllegalStateException("synthetic reuse target mutated by checkLink");
                }
                emitCheckRows(
                        "synthetic", caseName, plan, beam, finalAlias, stem, trace, link);
                checkOutcome = link != null ? "Accepted" : "Rejected";
            }
            if (!initialStateBefore.same(initialStem)) {
                throw new IllegalStateException("synthetic cardinality loop mutated initial stem");
            }
            System.out.printf(
                    "stemsbeamvlinkbaseapplysyntheticselection %s system %d plan %d case %s "
                            + "origin IsolatedSyntheticSig "
                            + "cardinalityModel ActualHeadInterGetSideStems cardinality %s "
                            + "entriesTotal %d entriesConditionRead %d unreadSuffix %d "
                            + "selectedMapOrdinal %s outcome %s finalStemAlias %s "
                            + "finalStemInterId %d finalGlyphId %s finalGlyph %s finalGrade %s "
                            + "finalMedian %s finalWidth %s finalBounds %s finalAbnormal %s "
                            + "finalSigAttached %s checkOutcome %s zeroMutation true%n",
                    page, system.getId(), plan, caseName,
                    caseName.startsWith("Zero") ? "0"
                            : caseName.startsWith("Unique") ? "1" : "multiple",
                    entries.size(), read, entries.size() - read, selectedOrdinal,
                    javaNull ? "JavaNullPointer" : stopped ? "Selected" : "NoUnique",
                    finalAlias, stem.getId(), stem.getGlyph() != null
                            ? Integer.toString(stem.getGlyph().getId()) : "-",
                    stem.getGlyph() != null ? glyphToken(stem.getGlyph()) : "-",
                    hex(stem.getGrade()), line(stem.getMedian()), hex(stem.getWidth()),
                    rectangle(stem.getBounds()), stem.isAbnormal(), stem.getSig() != null,
                    checkOutcome);
        }

        void emitSyntheticScanRows (String caseName,
                                    int plan,
                                    int mapOrdinal,
                                    HeadScan scan)
        {
            for (HeadStemScanRow row : scan.rows) {
                System.out.printf(
                        "stemsbeamvlinkbaseapplysyntheticheadstemscan %s system %d plan %d "
                                + "case %s mapOrdinal %d scanOrdinal %d sigEdgeOrdinal %d "
                                + "scanOrderDomain HeadInterRelationsSourceOrder "
                                + "sigEdgeIdentityDomain IsolatedSigEdgeSetSourceOrder "
                                + "edgeHeadSide %s targetStemAlias %s targetStemInterId %d "
                                + "targetSigAttached %s targetGlyphId %d matchesParentSide %s "
                                + "distinctInsertion %s headSnapshotHash %s action %s%n",
                        page, system.getId(), plan, caseName, mapOrdinal, row.scanOrdinal,
                        row.sigEdgeOrdinal, row.edgeHeadSide, row.targetAlias,
                        row.target.getId(), row.target.getSig() != null,
                        row.target.getGlyph().getId(), row.matchesParentSide,
                        row.distinctInsertion, scan.snapshotHash, row.action);
            }
        }

        SyntheticSigFixture isolatedSyntheticSig (StemInter initialStem)
        {
            final IsolatedSyntheticSig sig = new IsolatedSyntheticSig(system);
            final Glyph sourceGlyph = initialStem.getGlyph();
            if (sourceGlyph == null || sourceGlyph.getRunTable() == null) {
                throw new IllegalStateException("createStem lacks glyph for isolated synthetic SIG");
            }
            final Glyph sharedGlyph = new Glyph(
                    sourceGlyph.getLeft(), sourceGlyph.getTop(), sourceGlyph.getRunTable());
            sharedGlyph.setId(1_500_000_001);

            final StemInter stemA = new StemInter(sharedGlyph, initialStem.getGrade());
            stemA.setId(1_500_000_101);
            stemA.setWidth(initialStem.getWidth());
            stemA.setMedian(initialStem.getMedian().getP1(), initialStem.getMedian().getP2());
            final StemInter stemB = new StemInter(sharedGlyph, initialStem.getGrade());
            stemB.setId(1_500_000_102);
            stemB.setWidth(initialStem.getWidth());
            stemB.setMedian(initialStem.getMedian().getP1(), initialStem.getMedian().getP2());

            final HeadInter zeroHead = syntheticHead(1_500_000_201, 20);
            final HeadInter uniqueHead = syntheticHead(1_500_000_202, 40);
            final HeadInter unreadHead = syntheticHead(1_500_000_203, 60);
            final HeadInter multipleHead = syntheticHead(1_500_000_204, 80);
            for (Inter inter : List.of(
                    zeroHead, uniqueHead, unreadHead, multipleHead, stemA, stemB)) {
                if (!sig.addVertex(inter)) {
                    throw new IllegalStateException("duplicate isolated synthetic SIG vertex");
                }
            }
            addSyntheticHeadStem(sig, uniqueHead, stemA, HorizontalSide.LEFT);
            addSyntheticHeadStem(sig, unreadHead, stemB, HorizontalSide.RIGHT);
            addSyntheticHeadStem(sig, multipleHead, stemA, HorizontalSide.LEFT);
            addSyntheticHeadStem(sig, multipleHead, stemB, HorizontalSide.LEFT);

            final IdentityHashMap<Relation, Integer> edgeOrdinals = new IdentityHashMap<>();
            int edgeOrdinal = 0;
            for (Relation relation : sig.edgeSet()) edgeOrdinals.put(relation, edgeOrdinal++);
            final LiveStemCatalogue liveStems = LiveStemCatalogue.forExaminedScans(
                    sig, edgeOrdinals, "synthetic-live");
            liveStems.intern(stemA);
            liveStems.intern(stemB);
            return new SyntheticSigFixture(
                    sig, zeroHead, uniqueHead, unreadHead, multipleHead,
                    stemA, stemB, edgeOrdinals, liveStems);
        }

        HeadInter syntheticHead (int id,
                                 int x)
        {
            final HeadInter head = new HeadInter(
                    new Rectangle(x, 20, 10, 8), Shape.NOTEHEAD_BLACK, 1.0, null, 0.0);
            head.setId(id);
            return head;
        }

        void addSyntheticHeadStem (SIGraph sig,
                                   HeadInter head,
                                   StemInter stem,
                                   HorizontalSide side)
        {
            final HeadStemRelation relation = new HeadStemRelation();
            relation.setHeadSide(side);
            relation.setExtensionPoint(new Point2D.Double(head.getCenter().x, head.getCenter().y));
            if (!sig.addEdge(head, stem, relation)) {
                throw new IllegalStateException("duplicate isolated HeadStemRelation");
            }
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
                throw new IllegalStateException("pure base-apply boundary mutated live state");
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
                                                             Path reuseCheck)
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
                plans.put(required(values, "system") + ":" + required(values, "plan"), values);
            } else if (line.startsWith("stemsbeamexpandend ")) {
                values = fields(line);
                ends.put(required(values, "system") + ":" + required(values, "plan"), values);
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
                    reuseBaseline, reuseSelection, reuseCheckResult, reuseGuard));
        }
        if (result.size() != createFrontiers.size() || result.size() != createResults.size()
                || result.size() != createDeltas.size() || result.size() != createGuards.size()
                || result.size() != reuseBaselines.size()
                || result.size() != reuseSelections.size()
                || result.size() != reuseCheckResults.size()
                || result.size() != reuseGuards.size()) {
            throw new IllegalStateException("predecessor fixture system coverage mismatch");
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
        for (int i = 2; i + 1 < tokens.length; i += 2) result.put(tokens[i], tokens[i + 1]);
        return result;
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
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLink base-apply oracle.");
        System.out.println("# schema: stems-beam-vlink-base-apply-v1");
        System.out.println("# Frozen scheduler/expand/createStem/reuse-check predecessors are reconstructed and joined exactly.");
        System.out.println("# Real rows include a computed boundary-13 legacy-hash compatibility certificate at Before.");
        System.out.println("# Real rows execute conditional SIG.addVertex then base Link.applyTo/BeamStem callback.");
        System.out.println("# Stop is before B-linker flag, sibling-beam, or head-relation mutation.");
        System.out.println("# Real Chula first frontiers are ID-zero; system 1 adds independent isolated-sheet branch certificates.");
        System.out.println("# Throwing-listener rows, if present, are envelope-only and not production-equivalence claims.");
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
                            Map<String, String> reuseGuard)
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
    }
}
