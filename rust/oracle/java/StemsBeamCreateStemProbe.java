// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
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
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.WeakGlyph;
import org.audiveris.omr.math.AreaUtil;
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
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact first stateful beam boundary: one real expand followed by createStem.
 *
 * <p>Each target system is reconstructed from a fresh HEADS sheet. The selected scheduler prefix
 * must join both frozen beam-scheduler and beam-expand fixtures. The real shared theoretical-line
 * delta is retained, then {@link StemBuilder#createStem} is called exactly once. Execution stops
 * before VLinker's stem-reuse loop and before BeamStemRelation.checkLink, SIG insertion, relation
 * insertion, or linker-flag mutation.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamCreateStemProbe
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
    private static final Class<?> B_LINKER_CLASS;
    private static final Class<?> V_LINKER_CLASS;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            B_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            V_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker");
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
            STEM_BUILDER_THEO_LINE = field(StemBuilder.class, "theoLine");
            GLYPH_ORIGINALS = field(GlyphIndex.class, "originals");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamCreateStemProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            return;
        }
        if (args.length != 5 || !args[0].equals("--system")) {
            throw new IllegalArgumentException(
                    "expected --system <id> <path>:<sheet> <scheduler-fixture> <expand-fixture>");
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
        final Map<Integer, Expected> expected = expectedFrontiers(schedulerFixture, expandFixture);

        final Sheet sheet = loadPage(pagePath, sheetNumber);
        final int systemCount = sheet.getSystems().size();
        if (expected.size() != systemCount) {
            throw new IllegalStateException("fixture/system count mismatch");
        }
        final String page = pagePath.getFileName() + "#" + sheetNumber;
        if (wantedSystem == 1) {
            printHeader();
            System.out.printf(
                    "stemsbeamcreatestempage %s systems %d schedulerFixtureSha256 %s "
                            + "expandFixtureSha256 %s executionMode foregroundJvmPerSystem "
                            + "registryHashMode StructuralGlyphMultisetMembershipOnly%n",
                    page, systemCount, sha256File(schedulerFixture), sha256File(expandFixture));
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
        for (Inter inter : heads) {
            final HeadInter head = (HeadInter) inter;
            if (head.getLinker() != null) throw new IllegalStateException("head linker already set");
            head.setLinker(new HeadLinker(head, retriever));
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
                bAliases, planRefs, inspectionBeams, heads, allLinkers, expected,
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
            final int allocatorBefore = before.allocator;
            final int candidateObjectIdBefore = candidate.getId();
            final String noStaff = noStaffDigest(sheet);

            System.out.printf(
                    "stemsbeamcreatestembaseline %s system %d executionMode %s "
                            + "allocator %d glyphActive %d glyphOriginals %d interIndex %d "
                            + "sigVertices %d sigEdges %d systemStems %d noStaff %s "
                            + "glyphActiveHash %s glyphOriginalsHash %s interIndexHash %s "
                            + "sigHash %s systemStemsHash %s%n",
                    page, system.getId(), executionMode, before.allocator,
                    before.glyphs.active.size(), before.glyphs.originals.size(),
                    before.inters.identities.size(), before.sig.vertices.size(), before.sig.edges.size(),
                    before.systemStems.entries.size(), noStaff, before.glyphs.activeHash,
                    before.glyphs.originalsHash, before.inters.hash, before.sig.hash,
                    before.systemStems.hash);
            System.out.printf(
                    "stemsbeamcreatestemfrontier %s system %d beamOrder %d beamSig %d "
                            + "hSide %s bAlias %s vSide %s builder %d plan %d stemProfile %d "
                            + "linkProfile %d lineBefore %s selectedGlyphRefs %s%n",
                    page, system.getId(), beamIndex, beamSigOrdinals.get(beam), hSide,
                    bAliases.get(b), V_V_SIDE.get(v), ref.builder, ref.plan, stemProfile,
                    linkProfile, line(lineBefore), list(selected));
            System.out.printf(
                    "stemsbeamcreatestemexpand %s system %d plan %d lastIndex %d relations %d "
                            + "glyphs %d lineAfter %s lineChanged %s builderAliases %s "
                            + "attachmentAliases %s%n",
                    page, system.getId(), ref.plan, lastIndex, relations.size(), glyphs.size(),
                    line(lineAfterExpand), !sameLine(lineBefore, lineAfterExpand), builderAlias,
                    attachmentAlias);

            System.out.printf(
                    "stemsbeamcreatestemlookup %s system %d certificate ExhaustiveGlyphEqualsScan "
                            + "candidate %s candidateBounds %s candidateWeight %d "
                            + "candidateRunTable %s "
                            + "aliasOrder JavaGlyphId baselineUnionSize %d "
                            + "scannedActive %d scannedOriginals %d activeEqualMatches %d "
                            + "originalEqualMatches %d lookup %s presentAlias %s presentId %s "
                            + "presentActive %s presentGlyph %s "
                            + "systemStemCertificate ExhaustiveSystemStemEqualsScan "
                            + "scannedSystemStems %d systemStemEqualMatches %d "
                            + "systemStemLookup %s "
                            + "systemStemInterId %s systemStemGrade %s activeHash %s originalsHash %s "
                            + "systemStemsHash %s%n",
                    page, system.getId(), glyphToken(candidate), rectangle(candidate.getBounds()),
                    candidate.getWeight(), runTableToken(candidate), glyphsBefore.aliases.size(),
                    glyphsBefore.active.size(),
                    glyphsBefore.originals.size(), glyphsBefore.equalActiveCount(candidate),
                    existing != null ? 1 : 0, existing != null ? "Present" : "Absent",
                    existing != null ? glyphsBefore.alias(existing) : "-",
                    existing != null ? Integer.toString(existing.getId()) : "-",
                    existing != null ? Boolean.toString(existingActive) : "-",
                    existing != null ? glyphToken(existing) : "-",
                    before.systemStems.entries.size(), existingStem != null ? 1 : 0,
                    existingStem != null ? "Present" : "Absent",
                    existingStem != null ? Integer.toString(existingStem.getId()) : "-",
                    existingStem != null ? hex(existingStem.getGrade()) : "-",
                    glyphsBefore.activeHash, glyphsBefore.originalsHash,
                    before.systemStems.hash);

            final StemInter stem = builder.createStem(glyphs, stemProfile);
            final PersistentSnapshot after = snapshot(
                    sheet, retriever, inspectionBeams, heads, allLinkers);
            final Glyph registered = after.glyphs.uniqueEqualOriginal(candidate);
            final String registration;
            if (existing == null) registration = "New";
            else if (existingActive) registration = "ReuseActive";
            else registration = "ReinsertOriginal";
            final String disposition;
            final GradeImpacts impacts;
            if (existingStem != null) {
                if (stem != existingStem) throw new IllegalStateException("system stem reuse mismatch");
                disposition = "Reused";
                impacts = stem.getImpacts();
            } else if (stem == null) {
                disposition = "Rejected";
                impacts = checker.checkStem(registered, stemProfile);
            } else if (stem.getImpacts() == null) {
                disposition = "CreatedArtificial";
                impacts = checker.checkStem(registered, stemProfile);
            } else {
                disposition = "CreatedChecked";
                impacts = stem.getImpacts();
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

            final int allocatorDelta = after.allocator - allocatorBefore;
            System.out.printf(
                    "stemsbeamcreatestemresult %s system %d plan %d candidate %s "
                            + "candidateComponents %s registration %s "
                            + "candidateObjectIdBefore %d canonicalGlyphIdBefore %s "
                            + "registeredAlias %s postAliasOrder JavaGlyphId "
                            + "postUnionSize %d registeredGlyphId %d disposition %s "
                            + "returnedStemInterId %s stemGrade %s "
                            + "stemMedian %s stemMeanThickness %s stemBounds %s "
                            + "stemAbnormal %s stemSigAttached %s "
                            + "stemMinGrade %s checkerMinThreshold %s artificialGrade %s "
                            + "impacts %s%n",
                    page, system.getId(), ref.plan, glyphToken(candidate), list(selected),
                    registration, candidateObjectIdBefore,
                    existing != null ? Integer.toString(existing.getId()) : "-",
                    after.glyphs.alias(registered),
                    after.glyphs.aliases.size(), registered.getId(), disposition,
                    stem != null ? Integer.toString(stem.getId()) : "-",
                    stem != null ? hex(stem.getGrade()) : "-",
                    stem != null ? line(stem.getMedian()) : "-",
                    stem != null && stem.getWidth() != null ? hex(stem.getWidth()) : "-",
                    stem != null && stem.getBounds() != null ? rectangle(stem.getBounds()) : "-",
                    stem != null ? Boolean.toString(stem.isAbnormal()) : "-",
                    stem != null ? Boolean.toString(stem.getSig() != null) : "-",
                    hex(StemInter.getMinGrade()),
                    hex(checker.getMinThreshold(stemProfile)),
                    hex(PARAMETERS_ARTIFICIAL_GRADE.getDouble(params)), impactsToken(impacts));
            System.out.printf(
                    "stemsbeamcreatestemdelta %s system %d allocatorBefore %d allocatorAfter %d "
                            + "allocatorDelta %d glyphActiveBefore %d glyphActiveAfter %d "
                            + "glyphOriginalsBefore %d glyphOriginalsAfter %d "
                            + "systemStemsBefore %d systemStemsAfter %d registeredAlias %s "
                            + "registeredGlyph %s "
                            + "glyphActiveHashAfter %s glyphOriginalsHashAfter %s "
                            + "systemStemsHashAfter %s%n",
                    page, system.getId(), allocatorBefore, after.allocator, allocatorDelta,
                    before.glyphs.active.size(), after.glyphs.active.size(),
                    before.glyphs.originals.size(), after.glyphs.originals.size(),
                    before.systemStems.entries.size(), after.systemStems.entries.size(),
                    after.glyphs.alias(registered), glyphToken(registered),
                    after.glyphs.activeHash, after.glyphs.originalsHash,
                    after.systemStems.hash);
            System.out.printf(
                    "stemsbeamcreatestemguard %s system %d lineDeltaRetained true "
                            + "interIndexUnchanged true sigUnchanged true relationsUnchanged true "
                            + "linkerFlagsUnchanged true stopBeforeVReuse true "
                            + "stopBeforeBeamStemCheck true%n",
                    page, system.getId());
            System.out.printf(
                    "stemsbeamcreatestemsummary %s system %d transaction CreateStemOnly "
                            + "registration %s disposition %s allocatorDelta %d%n",
                    page, system.getId(), registration, disposition, allocatorDelta);

            totals.transactions++;
            totals.allocatorDelta += allocatorDelta;
            switch (registration) {
                case "New" -> totals.newGlyphs++;
                case "ReuseActive" -> totals.reusedGlyphs++;
                case "ReinsertOriginal" -> totals.reinsertedGlyphs++;
                default -> throw new IllegalStateException(registration);
            }
            switch (disposition) {
                case "CreatedChecked" -> totals.checkedStems++;
                case "Reused" -> totals.reusedStems++;
                case "CreatedArtificial" -> totals.artificialStems++;
                case "Rejected" -> totals.rejectedStems++;
                default -> throw new IllegalStateException(disposition);
            }
            stopped = true;
            return stem != null;
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
                identities.put(inter, true);
                rows.add(inter.getId() + "=" + inter.getClass().getSimpleName()
                        + ":removed=" + inter.isRemoved());
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (InterRegistrySnapshot that)
        {
            return identitySetEquals(identities, that.identities) && hash.equals(that.hash);
        }
    }

    private static final class SigSnapshot
    {
        final IdentityHashMap<Inter, Boolean> vertices = new IdentityHashMap<>();
        final IdentityHashMap<Relation, Boolean> edges = new IdentityHashMap<>();
        final String hash;

        SigSnapshot (Sheet sheet)
        {
            final List<String> rows = new ArrayList<>();
            for (SystemInfo system : sheet.getSystems()) {
                final SIGraph sig = system.getSig();
                for (Inter inter : sig.vertexSet()) {
                    vertices.put(inter, true);
                    rows.add("v:" + system.getId() + ":" + inter.getId()
                            + ":" + inter.getClass().getSimpleName());
                }
                for (Relation relation : sig.edgeSet()) {
                    edges.put(relation, true);
                    rows.add("e:" + system.getId() + ":" + sig.getEdgeSource(relation).getId()
                            + ":" + sig.getEdgeTarget(relation).getId()
                            + ":" + relation.getClass().getSimpleName());
                }
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (SigSnapshot that)
        {
            return identitySetEquals(vertices, that.vertices)
                    && identitySetEquals(edges, that.edges) && hash.equals(that.hash);
        }
    }

    private static final class LinkerSnapshot
    {
        final IdentityHashMap<StemLinker, String> state = new IdentityHashMap<>();

        LinkerSnapshot (List<StemLinker> linkers)
        {
            for (StemLinker linker : linkers) {
                if (state.put(linker, linker.isLinked() + ":" + linker.isClosed()) != null) {
                    throw new IllegalStateException("duplicate linker snapshot identity");
                }
            }
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
                rows.add(glyphToken(entry.getKey()) + "=stem:" + entry.getValue().getId()
                        + ":grade=" + hex(entry.getValue().getGrade()));
            }
            rows.sort(Comparator.naturalOrder());
            hash = sha256Rows(rows);
        }

        boolean same (SystemStemsSnapshot that)
        {
            if (entries.size() != that.entries.size() || !hash.equals(that.hash)) return false;
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
            staff = stem.getStaff();
            value = valueOf(stem);
        }

        boolean same (StemInter stem)
        {
            return stem == identity && stem.getSig() == sig && stem.getGlyph() == glyph
                    && stem.getImpacts() == impacts && stem.getStaff() == staff
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
                    + ":bounds=" + (bounds != null ? rectangle(bounds) : "-")
                    + ":median=" + (median != null ? line(median) : "-");
        }
    }

    private static final class LineSnapshot
    {
        final IdentityHashMap<Object, Line2D> lines = new IdentityHashMap<>();

        LineSnapshot (List<Inter> beams)
            throws Exception
        {
            for (Inter inter : beams) {
                final List<Object> allB =
                        (List<Object>) LINKER_ALL_B.get(((AbstractBeamInter) inter).getLinker());
                for (Object b : allB) {
                    final Map<VerticalSide, Object> vMap =
                            (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                    for (Object v : vMap.values()) lines.put(v, copy((Line2D) V_THEO_LINE.get(v)));
                }
            }
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
                                                             Path expand)
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
        final Map<Integer, Expected> result = new HashMap<>();
        for (Map.Entry<Integer, Map<String, String>> entry : frontiers.entrySet()) {
            final int system = entry.getKey();
            final Map<String, String> frontier = entry.getValue();
            final String key = system + ":" + required(frontier, "plan");
            final Map<String, String> plan = plans.get(key);
            final Map<String, String> end = ends.get(key);
            if (plan == null || end == null || !"ReadyForCreateStem".equals(end.get("outcome"))) {
                throw new IllegalStateException("frontier lacks ready expand fixture row");
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
            result.put(system, new Expected(List.of(
                    required(frontier, "beamSig"), required(frontier, "hSide"),
                    required(frontier, "bAlias"), required(frontier, "vSide"),
                    required(frontier, "builder"), required(frontier, "plan"),
                    required(frontier, "stemProfile"), required(frontier, "linkProfile"),
                    required(end, "lastIndex"), required(end, "relationCount"),
                    required(end, "glyphCount"))));
        }
        return result;
    }

    private static Map<String, String> fields (String line)
    {
        final String[] tokens = line.split(" ");
        final Map<String, String> result = new HashMap<>();
        for (int i = 2; i + 1 < tokens.length; i += 2) result.put(tokens[i], tokens[i + 1]);
        return result;
    }

    private static String required (Map<String, String> fields,
                                    String name)
    {
        final String value = fields.get(name);
        if (value == null) throw new IllegalStateException("missing field " + name);
        return value;
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

    private static String hex (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
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
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam createStem oracle.");
        System.out.println("# schema: stems-beam-create-stem-v1");
        System.out.println("# One frozen first-ready expand delta is retained, then createStem is called once.");
        System.out.println("# Each system uses a fresh HEADS sheet; only system 1 is labeled sheet-first.");
        System.out.println("# Stop is before VLinker reuse, BeamStemRelation, SIG, relations, and linker flags; frozen fixtures require systemStem lookup Absent.");
    }

    private record PlanRef(int plan, int builder, String bAlias, VerticalSide vSide)
    {
    }

    private record Expected(List<String> values)
    {
    }

    private static final class Totals
    {
        long transactions;
        long newGlyphs;
        long reusedGlyphs;
        long reinsertedGlyphs;
        long checkedStems;
        long reusedStems;
        long artificialStems;
        long rejectedStems;
        long allocatorDelta;
    }
}
