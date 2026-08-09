// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.EnumMap;
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
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.Shape;
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
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.Exclusion;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact scheduler prefix oracle between inspected beam builders and the first stateful link.
 *
 * <p>Known-false {@code VLinker.link} prefixes are executed in real source order, including their
 * persistent downward theoretical-line shifts. A ready prefix is named but rolled back and exposed
 * as {@code AwaitingVLinkTransaction}; the probe never calls {@code StemBuilder.createStem}. Each
 * system stops at its first mutation frontier. The probe also stops before the rare competing-hook
 * removal frontier reached without a V transaction.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamSchedulerProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;
    private static final Field RETRIEVER_PARAMS;
    private static final Field RETRIEVER_SYSTEM_SEEDS;
    private static final Field RETRIEVER_SYSTEM_BEAMS;
    private static final Field RETRIEVER_SYSTEM_HEADS;
    private static final Field RETRIEVER_STEM_CHECKER;
    private static final Method PURGE_NO_STEM_SEEDS;
    private static final Field LINKER_ALL_B;
    private static final Field LINKER_SIDE_B;
    private static final Field LINKER_STUMP_V;
    private static final Field LINKER_SIDE_STUMPS;
    private static final Field B_ID;
    private static final Field B_H_SIDE;
    private static final Field B_STUMP;
    private static final Field B_IS_ANCHOR;
    private static final Field B_V_LINKERS;
    private static final Field V_V_SIDE;
    private static final Field V_Y_DIR;
    private static final Field V_THEO_LINE;
    private static final Field V_STEM_BUILDER;
    private static final Method V_EXPAND;
    private static final Field STEM_BUILDER_THEO_LINE;
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
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            LINKER_ALL_B = field(BeamLinker.class, "allBLinkers");
            LINKER_SIDE_B = field(BeamLinker.class, "sideBLinkers");
            LINKER_STUMP_V = field(BeamLinker.class, "stumpLinkers");
            LINKER_SIDE_STUMPS = field(BeamLinker.class, "sideStumps");
            B_ID = field(B_LINKER_CLASS, "id");
            B_H_SIDE = field(B_LINKER_CLASS, "hSide");
            B_STUMP = field(B_LINKER_CLASS, "stump");
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
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamSchedulerProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            return;
        }
        if (args.length != 1) {
            throw new IllegalArgumentException("expected exactly one <path>:<sheet> target");
        }
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();
        final String[] target = args[0].split(":");
        if (target.length != 2) throw new IllegalArgumentException("target must be <path>:<sheet>");
        printHeader();
        runPage(Paths.get(target[0]).toAbsolutePath(), Integer.parseInt(target[1]));
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final Totals totals = new Totals();
        final RowHasher hash = new RowHasher();
        final IdentityHashMap<Relation, Integer> pairCreationOrdinals = new IdentityHashMap<>();
        int pairCreationOrdinal = 0;
        for (SystemInfo system : sheet.getSystems()) {
            for (Relation relation : system.getSig().edgeSet()) {
                if (relation instanceof Exclusion) {
                    final Inter source = system.getSig().getEdgeSource(relation);
                    final Inter target = system.getSig().getEdgeTarget(relation);
                    if (isSameItemBeamHookPair(source, target)) {
                        pairCreationOrdinals.put(relation, pairCreationOrdinal++);
                    }
                }
            }
        }
        final IdentityHashMap<Glyph, Integer> liveBeamGlyphAliases = new IdentityHashMap<>();
        System.out.printf(
                "stemsbeamschedulerpage %s systems %d staves %d family %s "
                        + "rawBeamHookPairs %d%n",
                page, sheet.getSystems().size(), sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily(), pairCreationOrdinals.size());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(
                    page, sheet, system, liveBeamGlyphAliases,
                    pairCreationOrdinals, totals, hash);
        }
        System.out.printf(
                "stemsbeamschedulerpagesummary %s systems %d liveBeamGlyphAliases %d "
                        + "%s hash %016x%n",
                page, sheet.getSystems().size(), liveBeamGlyphAliases.size(),
                totals.fields(), hash.value());
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
                                   IdentityHashMap<Glyph, Integer> liveBeamGlyphAliases,
                                   IdentityHashMap<Relation, Integer> pairCreationOrdinals,
                                   Totals pageTotals,
                                   RowHasher pageHash)
        throws Exception
    {
        final Totals totals = new Totals();
        final RowHasher hash = new RowHasher();
        final StemsRetriever retriever = new StemsRetriever(system);
        final Object params = PARAMETERS_CONSTRUCTOR.newInstance(system, sheet.getScale());
        RETRIEVER_PARAMS.set(retriever, params);
        RETRIEVER_STEM_CHECKER.set(retriever, new StemChecker(sheet));

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
        final IdentityHashMap<Object, Object> vParents = new IdentityHashMap<>();
        final IdentityHashMap<Object, PlanRef[]> planRefs = new IdentityHashMap<>();
        final IdentityHashMap<Object, Line2D> initialVLines = new IdentityHashMap<>();
        int planOrdinal = 0;
        int builderOrdinal = 0;
        for (Inter inter : inspectionBeams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                final Object b = allB.get(bOrdinal);
                bAliases.put(b, "beam:" + beamSigOrdinals.get(beam) + ":b:" + bOrdinal);
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Object v : vMap.values()) {
                    vParents.put(v, b);
                    final Line2D line = (Line2D) V_THEO_LINE.get(v);
                    initialVLines.put(v, copy(line));
                }
                if (B_IS_ANCHOR.getBoolean(b)) continue;
                final int constructionMax = B_H_SIDE.get(b) != null
                        ? Profiles.BEAM_SIDE : Profiles.BEAM_SEED;
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    final Object v = entry.getValue();
                    final PlanRef[] refs = new PlanRef[constructionMax + 1];
                    for (int profile = 0; profile <= constructionMax; profile++) {
                        refs[profile] = new PlanRef(
                                planOrdinal++, builderOrdinal, constructionMax,
                                bAliases.get(b), entry.getKey());
                    }
                    planRefs.put(v, refs);
                    builderOrdinal++;
                }
            }
        }

        final List<Inter> liveBeamSigOrder = system.getSig().inters(AbstractBeamInter.class);
        for (Inter inter : liveBeamSigOrder) {
            final Glyph glyph = inter.getGlyph();
            if (glyph != null && !liveBeamGlyphAliases.containsKey(glyph)) {
                liveBeamGlyphAliases.put(glyph, liveBeamGlyphAliases.size());
            }
        }
        final List<Inter> work = new ArrayList<>(liveBeamSigOrder);
        Collections.sort(work, Inters.byReverseWidth);
        final IdentityHashMap<Relation, Integer> pairLiveOrdinals = new IdentityHashMap<>();
        int pairLiveOrdinal = 0;
        for (Relation relation : system.getSig().edgeSet()) {
            if (relation instanceof Exclusion) {
                final Inter source = system.getSig().getEdgeSource(relation);
                final Inter target = system.getSig().getEdgeTarget(relation);
                if (isSameItemBeamHookPair(source, target)) {
                    pairLiveOrdinals.put(relation, pairLiveOrdinal++);
                }
            }
        }
        totals.liveBeamHookPairs += pairLiveOrdinals.size();
        final int widthTies = adjacentWidthTies(work);
        totals.widthTies += widthTies;
        emit(String.format(
                "stemsbeamschedulersystem %s system %d profile %d stubProfile %d "
                        + "originalBeamSigOrder %s liveBeamSigOrder %s inspectionXOrder %s "
                        + "reverseWidthOrder %s widthTies %d liveBeamHookPairs %d "
                        + "builders %d isolatedPlans %d",
                page, system.getId(), system.getProfile(), sheet.getStub().getProfile(),
                beamTokens(originalBeamSigOrder, beamSigOrdinals),
                beamTokens(liveBeamSigOrder, beamSigOrdinals),
                beamTokens(inspectionBeams, beamSigOrdinals),
                beamTokens(work, beamSigOrdinals), widthTies, pairLiveOrdinals.size(),
                builderOrdinal, planOrdinal),
                hash, pageHash);

        for (int reverseOrdinal = 0; reverseOrdinal < work.size(); reverseOrdinal++) {
            emitBeam(
                    page, system, (AbstractBeamInter) work.get(reverseOrdinal), reverseOrdinal,
                    originalBeamSigOrder, beamSigOrdinals, liveBeamGlyphAliases,
                    pairCreationOrdinals, pairLiveOrdinals, totals, hash, pageHash);
        }

        final PersistentSnapshot before = snapshot(system, inspectionBeams, initialVLines);
        final Scheduler scheduler = new Scheduler(
                page, system, work, beamSigOrdinals, bAliases, vParents, planRefs,
                initialVLines, totals, hash, pageHash);
        scheduler.run();

        // The emitted trace keeps known-false Java shifts in order. Restore only after capture so
        // this oracle itself leaves the independently inspected checkpoint unchanged.
        restoreLines(initialVLines);
        before.assertSame(snapshot(system, inspectionBeams, initialVLines));
        totals.systems++;
        emit(String.format(
                "stemsbeamschedulersystemsummary %s system %d %s hash %016x",
                page, system.getId(), totals.fields(), hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void emitBeam (String page,
                                  SystemInfo system,
                                  AbstractBeamInter beam,
                                  int reverseOrdinal,
                                  List<Inter> originalBeamSigOrder,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                  IdentityHashMap<Glyph, Integer> liveBeamGlyphAliases,
                                  IdentityHashMap<Relation, Integer> pairCreationOrdinals,
                                  IdentityHashMap<Relation, Integer> pairLiveOrdinals,
                                  Totals totals,
                                  RowHasher... hashes)
    {
        final Glyph glyph = beam.getGlyph();
        final List<String> sameGlyphMembers = new ArrayList<>();
        for (Inter inter : originalBeamSigOrder) {
            final AbstractBeamInter other = (AbstractBeamInter) inter;
            if (other.getGlyph() == glyph) sameGlyphMembers.add(beamToken(other, beamSigOrdinals));
        }
        final List<String> pairExclusions = new ArrayList<>();
        int qualifyingHookCompetitors = 0;
        for (Relation relation : system.getSig().getExclusions(beam)) {
            final Inter opposite = system.getSig().getOppositeInter(beam, relation);
            if (pairCreationOrdinals.containsKey(relation)
                    && pairLiveOrdinals.containsKey(relation)) {
                if (!(opposite instanceof AbstractBeamInter oppositeBeam)) {
                    throw new IllegalStateException("beam-hook pair endpoint is not a beam");
                }
                pairExclusions.add(
                        "pairCreation:" + pairCreationOrdinals.get(relation)
                                + ":pairLive:" + pairLiveOrdinals.get(relation)
                                + ":" + beamToken(oppositeBeam, beamSigOrdinals)
                                + "@" + ((Exclusion) relation).cause);
                totals.livePairEndpointViews++;
                if (oppositeBeam.isHook() && oppositeBeam.getGlyph() == glyph) {
                    qualifyingHookCompetitors++;
                }
            }
        }
        if (qualifyingHookCompetitors > 1) {
            throw new IllegalStateException("multiple same-item hook competitors");
        }
        final BeamHookInter competing = beam.getCompetingHook();
        final Relation competingExclusion = competing != null
                ? system.getSig().getExclusion(beam, competing) : null;
        if (competing != null) {
            totals.selectedCompetitors++;
            if (competing.getGlyph() != glyph
                    || system.getSig().getExclusion(beam, competing) == null
                    || competing.getShape() != Shape.BEAM_HOOK) {
                throw new IllegalStateException("invalid competing-hook topology");
            }
        }
        final Rectangle bounds = beam.getBounds();
        emit(String.format(
                "stemsbeamschedulerbeam %s system %d reverseOrdinal %d beamSig %d "
                        + "shape %s isHook %s width %d bounds %s glyph %s "
                        + "liveBeamGlyphAlias %s sameGlyphMembers %s pairExclusions %s "
                        + "competingHook %s "
                        + "competingPairCreation %s competingPairLive %s "
                        + "competingHookGlyphSameIdentity %s",
                page, system.getId(), reverseOrdinal, beamSigOrdinals.get(beam),
                beam.getShape(), beam.isHook(), bounds.width, rectangle(bounds), glyphToken(glyph),
                glyph != null ? "beamGlyph:" + liveBeamGlyphAliases.get(glyph) : "-",
                list(sameGlyphMembers), list(pairExclusions),
                competing != null ? beamToken(competing, beamSigOrdinals) : "-",
                competingExclusion != null ? token(pairCreationOrdinals.get(competingExclusion)) : "-",
                competingExclusion != null ? token(pairLiveOrdinals.get(competingExclusion)) : "-",
                competing != null && competing.getGlyph() == glyph), hashes);
        totals.beams++;
    }

    private static final class Scheduler
    {
        final String page;
        final SystemInfo system;
        final List<Inter> work;
        final IdentityHashMap<Inter, Integer> beamSigOrdinals;
        final IdentityHashMap<Object, String> bAliases;
        final IdentityHashMap<Object, Object> vParents;
        final IdentityHashMap<Object, PlanRef[]> planRefs;
        final IdentityHashMap<Object, Line2D> initialVLines;
        final IdentityHashMap<Object, Integer> vAttempts = new IdentityHashMap<>();
        final Set<Inter> locallyRemoved = Collections.newSetFromMap(new IdentityHashMap<>());
        final Totals totals;
        final RowHasher[] hashes;
        int event;
        boolean stopped;

        Scheduler (String page,
                   SystemInfo system,
                   List<Inter> work,
                   IdentityHashMap<Inter, Integer> beamSigOrdinals,
                   IdentityHashMap<Object, String> bAliases,
                   IdentityHashMap<Object, Object> vParents,
                   IdentityHashMap<Object, PlanRef[]> planRefs,
                   IdentityHashMap<Object, Line2D> initialVLines,
                   Totals totals,
                   RowHasher... hashes)
        {
            this.page = page;
            this.system = system;
            this.work = work;
            this.beamSigOrdinals = beamSigOrdinals;
            this.bAliases = bAliases;
            this.vParents = vParents;
            this.planRefs = planRefs;
            this.initialVLines = initialVLines;
            this.totals = totals;
            this.hashes = hashes;
        }

        void run ()
            throws Exception
        {
            int index = 0;
            while (index < work.size() && !stopped) {
                final AbstractBeamInter beam = (AbstractBeamInter) work.get(index);
                final BeamHookInter competitor = beam.getCompetingHook();
                final boolean competitorLocallyRemoved = competitor != null
                        && locallyRemoved.contains(competitor);
                if (competitorLocallyRemoved) totals.locallyRemovedStillSelected++;
                final EnumMap<HorizontalSide, Boolean> linkedSides =
                        new EnumMap<>(HorizontalSide.class);
                boolean beamOk = true;
                final Map<HorizontalSide, Object> sides =
                        (Map<HorizontalSide, Object>) LINKER_SIDE_B.get(beam.getLinker());
                for (HorizontalSide hSide : HorizontalSide.values()) {
                    final Object b = sides.get(hSide);
                    if (b == null) {
                        sideRow(beam, index, hSide, "-", "MissingBLinker", false,
                                competitor, competitorLocallyRemoved);
                        continue;
                    }
                    final int stemProfile = (beam.isHook() || competitor != null)
                            ? system.getProfile() : Profiles.BEAM_SIDE;
                    final BResult result = runB(
                            "SIDES", beam, index, hSide, b, stemProfile, system.getProfile());
                    if (stopped) return;
                    sideRow(beam, index, hSide, bAliases.get(b), result.action, result.ok,
                            competitor, competitorLocallyRemoved);
                    if (result.ok) {
                        linkedSides.put(hSide, true);
                    } else if (!beam.isHook()) {
                        beamOk = false;
                        break;
                    }
                }
                if (beam.isHook() && linkedSides.isEmpty()) beamOk = false;
                if (!beam.isHook() && linkedSides.size() == 2 && competitor != null) {
                    frontier(
                            "AwaitingHookRemovalTransaction", "SIDES", beam, index, "-", "-",
                            "-", "-", "-", "-", "-", "-", "-", "-", "-",
                            "competingHook=" + beamToken(competitor, beamSigOrdinals));
                    totals.awaitingHookRemoval++;
                    stopped = true;
                    return;
                }
                if (!beamOk) {
                    decisionRow(beam, index, "RemoveKnownFalse", linkedSides);
                    locallyRemoved.add(beam);
                    work.remove(index);
                    totals.locallyRemovedBeams++;
                } else {
                    decisionRow(beam, index, "RetainKnownTrue", linkedSides);
                    index++;
                    totals.retainedBeams++;
                }
            }

            // The Java iterator now traverses only beams retained by the side phase.
            for (int beamIndex = 0; beamIndex < work.size() && !stopped; beamIndex++) {
                final AbstractBeamInter beam = (AbstractBeamInter) work.get(beamIndex);
                final List<Object> stumpVs = (List<Object>) LINKER_STUMP_V.get(beam.getLinker());
                final Map<HorizontalSide, Glyph> sideStumps =
                        (Map<HorizontalSide, Glyph>) LINKER_SIDE_STUMPS.get(beam.getLinker());
                for (Object v : stumpVs) {
                    final Glyph stump = (Glyph) B_STUMP.get(vParents.get(v));
                    final List<String> structuralSides = new ArrayList<>();
                    for (Map.Entry<HorizontalSide, Glyph> entry : sideStumps.entrySet()) {
                        if (entry.getValue().equals(stump)) structuralSides.add(entry.getKey().toString());
                    }
                    if (!structuralSides.isEmpty()) {
                        stumpRow(beam, beamIndex, v, stump, structuralSides, "SkipStructuralSideStump");
                        totals.structuralSideStumpSkips++;
                        continue;
                    }
                    final Object b = vParents.get(v);
                    if (((StemLinker) v).isLinked()) {
                        stumpRow(beam, beamIndex, v, stump, structuralSides, "SkipAlreadyLinked");
                        totals.linkedStumpSkips++;
                        continue;
                    }
                    stumpRow(beam, beamIndex, v, stump, structuralSides, "InvokeVLink");
                    runV("STUMPS", beam, beamIndex, null, b, v,
                            Profiles.BEAM_SEED, system.getProfile(), false);
                    if (stopped) return;
                }
            }
            emit(String.format(
                    "stemsbeamschedulercomplete %s system %d event %d type NoReadyPrefix "
                            + "work %s",
                    page, system.getId(), event++, workToken()), hashes);
            totals.completedSystems++;
        }

        BResult runB (String phase,
                      AbstractBeamInter beam,
                      int beamIndex,
                      HorizontalSide hSide,
                      Object b,
                      int stemProfile,
                      int linkProfile)
            throws Exception
        {
            final Map<VerticalSide, Object> vMap =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            if (vMap.isEmpty()) {
                totals.emptyVTrue++;
                return new BResult(true, "EmptyVMapTrue");
            }
            if (((StemLinker) b).isLinked()) {
                totals.initiallyLinkedB++;
                return new BResult(true, "AlreadyLinkedTrue");
            }
            for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                final Object v = entry.getValue();
                final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
                final int targets = builder.getTargetLinkers().size();
                if (targets == 0) {
                    attemptRow(
                            phase, beam, beamIndex, hSide, b, v, stemProfile, linkProfile,
                            "allTargets", 0, builder.getCLinkers(null).size(), "-",
                            "SkipNoTargetLinker", null, null, false, false, 0.0);
                    totals.targetPrecheckSkips++;
                    continue;
                }
                runV(phase, beam, beamIndex, hSide, b, v, stemProfile, linkProfile, true);
                if (stopped) return new BResult(false, "AwaitingFrontier");
            }
            return new BResult(false, "AllInvokedVKnownFalse");
        }

        void runV (String phase,
                   AbstractBeamInter beam,
                   int beamIndex,
                   HorizontalSide hSide,
                   Object b,
                   Object v,
                   int stemProfile,
                   int linkProfile,
                   boolean targetPrechecked)
            throws Exception
        {
            final PlanRef[] refs = planRefs.get(v);
            if (refs == null || stemProfile < 0 || stemProfile >= refs.length) {
                throw new IllegalStateException("scheduler profile has no isolated plan");
            }
            final PlanRef ref = refs[stemProfile];
            final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
            final Line2D stored = (Line2D) V_THEO_LINE.get(v);
            final Line2D before = copy(stored);
            final int attemptOrdinal = vAttempts.merge(v, 1, Integer::sum);
            final boolean shiftedFromIsolated = !sameLine(before, initialVLines.get(v));
            if (attemptOrdinal > 1 && shiftedFromIsolated) {
                frontier(
                        "AwaitingShiftedVRetry", phase, beam, beamIndex,
                        hSide != null ? hSide.toString() : "-", bAliases.get(b),
                        V_V_SIDE.get(v).toString(), vAlias(b, v), Integer.toString(ref.builder()),
                        Integer.toString(stemProfile), Integer.toString(linkProfile),
                        Integer.toString(ref.plan()), "-", line(before), line(before),
                        "sameVAttempt=" + attemptOrdinal);
                totals.shiftedVRetryFrontiers++;
                stopped = true;
                return;
            }
            final int headTargets = builder.getCLinkers(null).size();
            final int allTargets = builder.getTargetLinkers().size();
            final LinkedHashMap<StemLinker, Relation> relations = new LinkedHashMap<>();
            final LinkedHashSet<Glyph> glyphs = new LinkedHashSet<>();
            final int lastIndex;
            final String outcome;
            if (headTargets == 0) {
                lastIndex = -2;
                outcome = "NoHeadTarget";
            } else {
                lastIndex = (Integer) V_EXPAND.invoke(
                        v, stemProfile, linkProfile, relations, glyphs);
                if (lastIndex == -1) outcome = "ExpandFailed";
                else if (relations.isEmpty()) outcome = "NoRelations";
                else if (glyphs.isEmpty()) outcome = "NoGlyphs";
                else outcome = "ReadyForCreateStem";
            }
            final Line2D after = copy(stored);
            final Object attachment = beam.getAttachments().get("theo-" + B_ID.getInt(b));
            final boolean builderAlias = STEM_BUILDER_THEO_LINE.get(builder) == stored;
            final boolean attachmentAlias = attachment == stored;
            final boolean changed = !sameLine(before, after);
            final double dx = after.getX1() - before.getX1();
            if (!builderAlias) throw new IllegalStateException("builder/theoretical-line alias lost");
            if ((Integer) V_Y_DIR.get(v) > 0 && !attachmentAlias) {
                throw new IllegalStateException("downward/current-attachment alias lost");
            }
            final String action = outcome.equals("ReadyForCreateStem")
                    ? "AwaitingVLinkTransaction" : "KnownFalseReturn";
            attemptRow(
                    phase, beam, beamIndex, hSide, b, v, stemProfile, linkProfile,
                    targetPrechecked ? "allTargets" : "none", allTargets, headTargets,
                    outcome, action, before, after, builderAlias, attachmentAlias, dx);
            totals.invokedV++;
            if (changed) totals.knownOrPendingLineDeltas++;
            if (outcome.equals("ReadyForCreateStem")) {
                // The selected isolated plan is pending as one VLink transaction. Do not carry its
                // line delta into scheduler state; createStem and BeamStemRelation can still fail.
                stored.setLine(before);
                frontier(
                        "AwaitingVLinkTransaction", phase, beam, beamIndex,
                        hSide != null ? hSide.toString() : "-", bAliases.get(b),
                        V_V_SIDE.get(v).toString(), vAlias(b, v), Integer.toString(ref.builder()),
                        Integer.toString(stemProfile), Integer.toString(linkProfile),
                        Integer.toString(ref.plan()), outcome, line(before), line(after),
                        "lastIndex=" + lastIndex + ",relations=" + relations.size()
                                + ",glyphs=" + glyphs.size());
                totals.awaitingV++;
                stopped = true;
            } else {
                totals.knownFalseV++;
                if (changed) totals.knownFalseLineDeltas++;
            }
        }

        void attemptRow (String phase,
                         AbstractBeamInter beam,
                         int beamIndex,
                         HorizontalSide hSide,
                         Object b,
                         Object v,
                         int stemProfile,
                         int linkProfile,
                         String targetGate,
                         int allTargets,
                         int headTargets,
                         String outcome,
                         String action,
                         Line2D before,
                         Line2D after,
                         boolean builderAlias,
                         boolean attachmentAlias,
                         double dx)
        {
            final PlanRef ref = planRefs.get(v)[stemProfile];
            emit(String.format(
                    "stemsbeamschedulerattempt %s system %d event %d phase %s beamOrder %d "
                            + "beamSig %d width %d hSide %s bAlias %s vSide %s vAlias %s "
                            + "builder %d stemProfile %d linkProfile %d targetGate %s "
                            + "allTargets %d headTargets %d plan %d outcome %s action %s "
                            + "lineBefore %s lineAfter %s lineChanged %s dx %s "
                            + "builderAliases %s attachmentAliases %s sameVAttempt %d work %s",
                    page, system.getId(), event++, phase, beamIndex,
                    beamSigOrdinals.get(beam), beam.getBounds().width,
                    hSide != null ? hSide : "-", bAliases.get(b), get(V_V_SIDE, v), vAlias(b, v),
                    ref.builder(), stemProfile, linkProfile, targetGate, allTargets, headTargets,
                    ref.plan(), outcome, action, before != null ? line(before) : "-",
                    after != null ? line(after) : "-",
                    before != null && after != null && !sameLine(before, after), hex(dx),
                    builderAlias, attachmentAlias, vAttempts.getOrDefault(v, 0), workToken()),
                    hashes);
            totals.attemptRows++;
        }

        void sideRow (AbstractBeamInter beam,
                      int beamIndex,
                      HorizontalSide hSide,
                      String bAlias,
                      String action,
                      boolean logicalResult,
                      BeamHookInter competitor,
                      boolean competitorLocallyRemoved)
        {
            final int profile = (beam.isHook() || competitor != null)
                    ? system.getProfile() : Profiles.BEAM_SIDE;
            emit(String.format(
                    "stemsbeamschedulerside %s system %d event %d beamOrder %d beamSig %d "
                            + "width %d hSide %s bAlias %s stemProfile %d linkProfile %d "
                            + "profileReason %s action %s logicalResult %s competingHook %s "
                            + "competingHookLocallyRemoved %s work %s",
                    page, system.getId(), event++, beamIndex, beamSigOrdinals.get(beam),
                    beam.getBounds().width, hSide, bAlias, profile, system.getProfile(),
                    beam.isHook() ? "Hook" : competitor != null ? "CompetingHook" : "BeamSide",
                    action, logicalResult,
                    competitor != null ? beamToken(competitor, beamSigOrdinals) : "-",
                    competitorLocallyRemoved, workToken()), hashes);
            totals.sideRows++;
        }

        void decisionRow (AbstractBeamInter beam,
                          int beamIndex,
                          String action,
                          EnumMap<HorizontalSide, Boolean> linkedSides)
        {
            emit(String.format(
                    "stemsbeamschedulerbeamdecision %s system %d event %d beamOrder %d "
                            + "beamSig %d isHook %s linkedSides %s action %s workBefore %s",
                    page, system.getId(), event++, beamIndex, beamSigOrdinals.get(beam),
                    beam.isHook(), list(linkedSides.keySet()), action, workToken()), hashes);
            totals.beamDecisions++;
        }

        void stumpRow (AbstractBeamInter beam,
                       int beamIndex,
                       Object v,
                       Glyph stump,
                       List<String> structuralSides,
                       String action)
        {
            final Object b = vParents.get(v);
            emit(String.format(
                    "stemsbeamschedulerstump %s system %d event %d beamOrder %d beamSig %d "
                            + "width %d bAlias %s vSide %s vAlias %s stump %s "
                            + "structuralSideMatches %s stemProfile %d linkProfile %d "
                            + "linkedGuard %s action %s work %s",
                    page, system.getId(), event++, beamIndex, beamSigOrdinals.get(beam),
                    beam.getBounds().width, bAliases.get(b), get(V_V_SIDE, v), vAlias(b, v),
                    glyphToken(stump), list(structuralSides), Profiles.BEAM_SEED,
                    system.getProfile(), ((StemLinker) v).isLinked(), action, workToken()), hashes);
            totals.stumpRows++;
        }

        void frontier (String type,
                       String phase,
                       AbstractBeamInter beam,
                       int beamIndex,
                       String hSide,
                       String bAlias,
                       String vSide,
                       String vAlias,
                       String builder,
                       String stemProfile,
                       String linkProfile,
                       String plan,
                       String outcome,
                       String lineBefore,
                       String lineAfter,
                       String evidence)
        {
            emit(String.format(
                    "stemsbeamschedulerfrontier %s system %d event %d type %s phase %s "
                            + "beamOrder %d beamSig %d hSide %s bAlias %s vSide %s vAlias %s "
                            + "builder %s stemProfile %s linkProfile %s plan %s outcome %s "
                            + "lineBefore %s lineAfter %s evidence %s "
                            + "before %s current %s remaining %s",
                    page, system.getId(), event++, type, phase, beamIndex,
                    beamSigOrdinals.get(beam), hSide, bAlias, vSide, vAlias, builder,
                    stemProfile, linkProfile, plan, outcome, lineBefore, lineAfter, evidence,
                    beamTokens(work.subList(0, beamIndex), beamSigOrdinals),
                    beamToken(beam, beamSigOrdinals),
                    beamTokens(work.subList(beamIndex + 1, work.size()), beamSigOrdinals)), hashes);
        }

        String vAlias (Object b,
                       Object v)
        {
            return bAliases.get(b) + ":v:" + ((VerticalSide) get(V_V_SIDE, v)).name();
        }

        String workToken ()
        {
            return beamTokens(work, beamSigOrdinals);
        }
    }

    private static PersistentSnapshot snapshot (SystemInfo system,
                                                List<Inter> beams,
                                                IdentityHashMap<Object, Line2D> initialVLines)
        throws Exception
    {
        final List<Object> identities = new ArrayList<>();
        identities.addAll(system.getSig().vertexSet());
        identities.addAll(system.getSig().edgeSet());
        final StringBuilder state = new StringBuilder();
        for (Inter inter : beams) {
            final BeamLinker linker = ((AbstractBeamInter) inter).getLinker();
            identities.add(linker);
            for (Object b : (List<Object>) LINKER_ALL_B.get(linker)) {
                identities.add(b);
                state.append(((StemLinker) b).isLinked()).append(':')
                        .append(((StemLinker) b).isClosed()).append(';');
                for (Object v : ((Map<VerticalSide, Object>) B_V_LINKERS.get(b)).values()) {
                    identities.add(v);
                    identities.add(V_STEM_BUILDER.get(v));
                    state.append(line((Line2D) V_THEO_LINE.get(v))).append(';');
                    if (!sameLine((Line2D) V_THEO_LINE.get(v), initialVLines.get(v))) {
                        throw new IllegalStateException("line not restored at snapshot");
                    }
                }
            }
        }
        return new PersistentSnapshot(identities, state.toString());
    }

    private static void restoreLines (IdentityHashMap<Object, Line2D> lines)
        throws IllegalAccessException
    {
        for (Map.Entry<Object, Line2D> entry : lines.entrySet()) {
            ((Line2D) V_THEO_LINE.get(entry.getKey())).setLine(entry.getValue());
        }
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

    private static IdentityHashMap<Inter, Integer> ordinals (List<Inter> values)
    {
        final IdentityHashMap<Inter, Integer> result = new IdentityHashMap<>();
        for (int i = 0; i < values.size(); i++) result.put(values.get(i), i);
        return result;
    }

    private static IdentityHashMap<Glyph, Integer> glyphOrdinals (List<Inter> beams)
    {
        final IdentityHashMap<Glyph, Integer> result = new IdentityHashMap<>();
        for (Inter inter : beams) {
            final Glyph glyph = inter.getGlyph();
            if (glyph != null && !result.containsKey(glyph)) result.put(glyph, result.size());
        }
        return result;
    }

    private static int adjacentWidthTies (List<Inter> beams)
    {
        int ties = 0;
        for (int i = 1; i < beams.size(); i++) {
            if (beams.get(i - 1).getBounds().width == beams.get(i).getBounds().width) ties++;
        }
        return ties;
    }

    private static boolean isSameItemBeamHookPair (Inter left,
                                                   Inter right)
    {
        if (!(left instanceof AbstractBeamInter leftBeam)
                || !(right instanceof AbstractBeamInter rightBeam)) return false;
        return leftBeam.getGlyph() != null
                && leftBeam.getGlyph() == rightBeam.getGlyph()
                && leftBeam.isHook() != rightBeam.isHook();
    }

    private static String token (Integer value)
    {
        return value != null ? value.toString() : "-";
    }

    private static String beamToken (AbstractBeamInter beam,
                                     IdentityHashMap<Inter, Integer> ordinals)
    {
        return "beam:" + ordinals.get(beam);
    }

    private static String beamTokens (List<? extends Inter> beams,
                                      IdentityHashMap<Inter, Integer> ordinals)
    {
        final List<String> tokens = new ArrayList<>();
        for (Inter inter : beams) tokens.add(beamToken((AbstractBeamInter) inter, ordinals));
        return list(tokens);
    }

    private static String glyphToken (Glyph glyph)
    {
        if (glyph == null) return "-";
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
        return Double.doubleToLongBits(left.getX1()) == Double.doubleToLongBits(right.getX1())
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

    private static void emit (String row,
                              RowHasher... hashes)
    {
        System.out.println(row);
        for (RowHasher hash : hashes) hash.add(row);
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam scheduler oracle.");
        System.out.println("# schema: stems-beam-scheduler-v1");
        System.out.println("# Known-false V prefixes execute in source order; their downward line shifts persist.");
        System.out.println("# Each system stops at its first V-link or competing-hook-removal mutation frontier.");
        System.out.println("# ReadyForCreateStem is necessary, not success; createStem is never invoked.");
    }

    private record PlanRef(
            int plan,
            int builder,
            int constructionMax,
            String bAlias,
            VerticalSide vSide)
    {
    }

    private record BResult(boolean ok, String action)
    {
    }

    private record PersistentSnapshot(List<Object> identities, String state)
    {
        void assertSame (PersistentSnapshot other)
        {
            if (identities.size() != other.identities.size() || !state.equals(other.state)) {
                throw new IllegalStateException("scheduler oracle mutated persistent state");
            }
            for (int i = 0; i < identities.size(); i++) {
                if (identities.get(i) != other.identities.get(i)) {
                    throw new IllegalStateException("scheduler oracle changed persistent identity order");
                }
            }
        }
    }

    private static final class Totals
    {
        long systems;
        long beams;
        long widthTies;
        long liveBeamHookPairs;
        long livePairEndpointViews;
        long selectedCompetitors;
        long locallyRemovedStillSelected;
        long sideRows;
        long attemptRows;
        long beamDecisions;
        long stumpRows;
        long targetPrecheckSkips;
        long invokedV;
        long knownFalseV;
        long knownFalseLineDeltas;
        long knownOrPendingLineDeltas;
        long awaitingV;
        long awaitingHookRemoval;
        long shiftedVRetryFrontiers;
        long emptyVTrue;
        long initiallyLinkedB;
        long locallyRemovedBeams;
        long retainedBeams;
        long structuralSideStumpSkips;
        long linkedStumpSkips;
        long completedSystems;

        void include (Totals that)
        {
            systems += that.systems;
            beams += that.beams;
            widthTies += that.widthTies;
            liveBeamHookPairs += that.liveBeamHookPairs;
            livePairEndpointViews += that.livePairEndpointViews;
            selectedCompetitors += that.selectedCompetitors;
            locallyRemovedStillSelected += that.locallyRemovedStillSelected;
            sideRows += that.sideRows;
            attemptRows += that.attemptRows;
            beamDecisions += that.beamDecisions;
            stumpRows += that.stumpRows;
            targetPrecheckSkips += that.targetPrecheckSkips;
            invokedV += that.invokedV;
            knownFalseV += that.knownFalseV;
            knownFalseLineDeltas += that.knownFalseLineDeltas;
            knownOrPendingLineDeltas += that.knownOrPendingLineDeltas;
            awaitingV += that.awaitingV;
            awaitingHookRemoval += that.awaitingHookRemoval;
            shiftedVRetryFrontiers += that.shiftedVRetryFrontiers;
            emptyVTrue += that.emptyVTrue;
            initiallyLinkedB += that.initiallyLinkedB;
            locallyRemovedBeams += that.locallyRemovedBeams;
            retainedBeams += that.retainedBeams;
            structuralSideStumpSkips += that.structuralSideStumpSkips;
            linkedStumpSkips += that.linkedStumpSkips;
            completedSystems += that.completedSystems;
        }

        String fields ()
        {
            return String.format(
                    "systems %d beams %d widthTies %d liveBeamHookPairs %d "
                            + "livePairEndpointViews %d "
                            + "selectedCompetitors %d locallyRemovedStillSelected %d "
                            + "sideRows %d attemptRows %d beamDecisions %d stumpRows %d "
                            + "targetPrecheckSkips %d invokedV %d knownFalseV %d "
                            + "knownFalseLineDeltas %d knownOrPendingLineDeltas %d "
                            + "awaitingV %d awaitingHookRemoval %d shiftedVRetryFrontiers %d "
                            + "emptyVTrue %d initiallyLinkedB %d locallyRemovedBeams %d "
                            + "retainedBeams %d structuralSideStumpSkips %d linkedStumpSkips %d "
                            + "completedSystems %d forbiddenPersistentMutations 0",
                    systems, beams, widthTies, liveBeamHookPairs, livePairEndpointViews,
                    selectedCompetitors,
                    locallyRemovedStillSelected, sideRows, attemptRows, beamDecisions, stumpRows,
                    targetPrecheckSkips, invokedV, knownFalseV, knownFalseLineDeltas,
                    knownOrPendingLineDeltas, awaitingV, awaitingHookRemoval,
                    shiftedVRetryFrontiers, emptyVTrue, initiallyLinkedB, locallyRemovedBeams,
                    retainedBeams, structuralSideStumpSkips, linkedStumpSkips, completedSystems);
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            for (byte byteValue : (row + "\n").getBytes(StandardCharsets.UTF_8)) {
                value ^= byteValue & 0xffL;
                value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }
}
