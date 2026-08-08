// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Path2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.Iterator;
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
import org.audiveris.omr.math.GeoOrder;
import org.audiveris.omr.math.GeoUtil;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.math.Rational;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadCorner;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamGroupInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact identity-free oracle for the prefix of {@code HeadLinker.CLinker.inspect()} ending
 * immediately before {@code new StemBuilder(...)}.
 *
 * <p>The probe first replays the preceding BeamLinker reachability prefix, because its
 * cross-beam anchors are visible to head-origin {@code findLinker} calls. It then visits heads in
 * the stable bounds-x order and corners in {@link HeadCorner#values()} order. Real private
 * {@code retrieveSeeds} and {@code lookupOtherHeads} calls are independently replayed and checked;
 * the optional sibling-beam path invokes the real {@code findLinker}. No C StemBuilder is
 * constructed.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsHeadCornerReachabilityProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;
    private static final Field RETRIEVER_SYSTEM_SEEDS;
    private static final Field RETRIEVER_SYSTEM_BEAMS;
    private static final Field RETRIEVER_SYSTEM_HEADS;
    private static final Field RETRIEVER_STEM_CHECKER;
    private static final Method PURGE_NO_STEM_SEEDS;

    private static final Field PARAM_MAX_BEAM_LINKER_DX;
    private static final Field PARAM_MAX_BEAM_SIDE_DX;
    private static final Field PARAM_MIN_BEAM_HEAD_DY;
    private static final Field PARAM_MIN_HEAD_HEAD_DY;
    private static final Field PARAM_MIN_SEED_CONTRIB;
    private static final Field PARAM_MAX_LINE_SEED_DX;
    private static final Field PARAM_MAX_HEAD_IN_DX;
    private static final Field PARAM_MAX_HEAD_OUT_DX;
    private static final Field PARAM_SLOPE_MARGIN;

    private static final Field BEAM_ALL_B;
    private static final Field B_REF_PT;
    private static final Field B_IS_ANCHOR;
    private static final Field B_V_LINKERS;
    private static final Method B_IS_LINKED;
    private static final Method B_IS_CLOSED;
    private static final Field V_STEM_BUILDER;
    private static final Method V_IS_LINKED;
    private static final Method V_IS_CLOSED;
    private static final Method V_FILTER_BEAMS;
    private static final Method V_FILTER_HEADS;

    private static final Field HEAD_NEIGHBOR_BEAMS;
    private static final Field HEAD_NEIGHBOR_SEEDS;
    private static final Field C_V_SIDE;
    private static final Field C_Y_DIR;
    private static final Field C_REF_PT;
    private static final Field C_OUT_PT;
    private static final Field C_IN_PT;
    private static final Field C_STUMP;
    private static final Field C_TARGET_PT;
    private static final Field C_LU_AREA;
    private static final Field C_SEEDS;
    private static final Field C_THEO_LINE;
    private static final Field C_Y_RANGE;
    private static final Field C_BEAM_GROUPS;
    private static final Field C_TARGET_BEAM;
    private static final Field C_STEM_BUILDER;
    private static final Method C_RETRIEVE_SEEDS;
    private static final Method C_LOOKUP_OTHER_HEADS;
    private static final Method C_IS_LINKED;
    private static final Method C_IS_CLOSED;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            final Class<?> bLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            final Class<?> vLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker");
            final Class<?> cLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker");

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

            PARAM_MAX_BEAM_LINKER_DX = field(parameters, "maxBeamLinkerDx");
            PARAM_MAX_BEAM_SIDE_DX = field(parameters, "maxBeamSideDx");
            PARAM_MIN_BEAM_HEAD_DY = field(parameters, "minBeamHeadDy");
            PARAM_MIN_HEAD_HEAD_DY = field(parameters, "minHeadHeadDy");
            PARAM_MIN_SEED_CONTRIB = field(parameters, "minSeedContrib");
            PARAM_MAX_LINE_SEED_DX = field(parameters, "maxLineSeedDx");
            PARAM_MAX_HEAD_IN_DX = field(parameters, "maxHeadInDx");
            PARAM_MAX_HEAD_OUT_DX = field(parameters, "maxHeadOutDx");
            PARAM_SLOPE_MARGIN = field(parameters, "slopeMargin");

            BEAM_ALL_B = field(BeamLinker.class, "allBLinkers");
            B_REF_PT = field(bLinker, "refPt");
            B_IS_ANCHOR = field(bLinker, "isAnchor");
            B_V_LINKERS = field(bLinker, "vLinkers");
            B_IS_LINKED = bLinker.getMethod("isLinked");
            B_IS_CLOSED = bLinker.getMethod("isClosed");
            B_IS_LINKED.setAccessible(true);
            B_IS_CLOSED.setAccessible(true);
            V_STEM_BUILDER = field(vLinker, "sb");
            V_IS_LINKED = vLinker.getMethod("isLinked");
            V_IS_CLOSED = vLinker.getMethod("isClosed");
            V_IS_LINKED.setAccessible(true);
            V_IS_CLOSED.setAccessible(true);
            V_FILTER_BEAMS = vLinker.getDeclaredMethod("filterBeams", List.class);
            V_FILTER_BEAMS.setAccessible(true);
            V_FILTER_HEADS = vLinker.getDeclaredMethod("filterHeads", List.class);
            V_FILTER_HEADS.setAccessible(true);

            HEAD_NEIGHBOR_BEAMS = field(HeadLinker.class, "neighborBeams");
            HEAD_NEIGHBOR_SEEDS = field(HeadLinker.class, "neighborSeeds");
            C_V_SIDE = field(cLinker, "vSide");
            C_Y_DIR = field(cLinker, "yDir");
            C_REF_PT = field(cLinker, "refPt");
            C_OUT_PT = field(cLinker, "outPt");
            C_IN_PT = field(cLinker, "inPt");
            C_STUMP = field(cLinker, "stump");
            C_TARGET_PT = field(cLinker, "targetPt");
            C_LU_AREA = field(cLinker, "luArea");
            C_SEEDS = field(cLinker, "seeds");
            C_THEO_LINE = field(cLinker, "theoLine");
            C_Y_RANGE = field(cLinker, "yRange");
            C_BEAM_GROUPS = field(cLinker, "beamGroups");
            C_TARGET_BEAM = field(cLinker, "targetBeam");
            C_STEM_BUILDER = field(cLinker, "sb");
            C_RETRIEVE_SEEDS = cLinker.getDeclaredMethod("retrieveSeeds");
            C_RETRIEVE_SEEDS.setAccessible(true);
            C_LOOKUP_OTHER_HEADS = cLinker.getDeclaredMethod("lookupOtherHeads");
            C_LOOKUP_OTHER_HEADS.setAccessible(true);
            C_IS_LINKED = cLinker.getMethod("isLinked");
            C_IS_CLOSED = cLinker.getMethod("isClosed");
            C_IS_LINKED.setAccessible(true);
            C_IS_CLOSED.setAccessible(true);
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsHeadCornerReachabilityProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            System.exit(0);
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
        if (target.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
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
        System.out.printf(
                "stemsheadreachpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, totals, hash);
        }
        System.out.printf(
                "stemsheadreachpagesummary %s systems %d heads %d corners %d seedsScanned %d "
                        + "seedsKept %d headScans %d headTargets %d siblingScans %d "
                        + "beamTargets %d anchorsCreated %d cSeedWrites %d builderChecks %d "
                        + "hash %016x%n",
                page,
                sheet.getSystems().size(),
                totals.heads,
                totals.corners,
                totals.seedsScanned,
                totals.seedsKept,
                totals.headScans,
                totals.headTargets,
                totals.siblingScans,
                totals.beamTargets,
                totals.anchorsCreated,
                totals.cSeedWrites,
                totals.builderChecks,
                hash.value());
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
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

        final int maxBeamLinkerDx = PARAM_MAX_BEAM_LINKER_DX.getInt(params);
        final int maxBeamSideDx = PARAM_MAX_BEAM_SIDE_DX.getInt(params);
        final int minBeamHeadDy = PARAM_MIN_BEAM_HEAD_DY.getInt(params);
        final int minHeadHeadDy = PARAM_MIN_HEAD_HEAD_DY.getInt(params);
        final int minSeedContrib = PARAM_MIN_SEED_CONTRIB.getInt(params);
        final double maxLineSeedDx = PARAM_MAX_LINE_SEED_DX.getDouble(params);
        final int maxHeadInDx = PARAM_MAX_HEAD_IN_DX.getInt(params);
        final int maxHeadOutDx = PARAM_MAX_HEAD_OUT_DX.getInt(params);
        final double slopeMargin = PARAM_SLOPE_MARGIN.getDouble(params);

        final List<Glyph> seeds = new ArrayList<>(
                system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED));
        PURGE_NO_STEM_SEEDS.invoke(retriever, seeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, seeds);
        final List<Glyph> seedSnapshot = new ArrayList<>(seeds);
        final IdentityHashMap<Glyph, Integer> seedOrdinals = ordinals(seeds);

        final List<Inter> sourceBeams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = interOrdinals(sourceBeams);
        final List<Inter> beams = new ArrayList<>(sourceBeams);
        Collections.sort(beams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, beams);
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            final BeamLinker linker = new BeamLinker(beam, retriever);
            if (linker.looksLikeTremolo()) {
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }
        }

        final List<Inter> sourceHeads = system.getSig().inters(
                ShapeSet.getTemplateNotesStem(sheet));
        final IdentityHashMap<Inter, Integer> headSigOrdinals = interOrdinals(sourceHeads);
        final List<Inter> heads = new ArrayList<>(sourceHeads);
        Collections.sort(heads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, heads);
        final IdentityHashMap<HeadInter, Integer> headXOrdinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < heads.size(); ordinal++) {
            final HeadInter head = (HeadInter) heads.get(ordinal);
            headXOrdinals.put(head, ordinal);
            head.setLinker(new HeadLinker(head, retriever));
        }

        final IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < beams.size(); ordinal++) {
            beamXOrdinals.put((AbstractBeamInter) beams.get(ordinal), ordinal);
        }
        final IdentityHashMap<Object, String> bAliases = new IdentityHashMap<>();
        final IdentityHashMap<Object, String> cAliases = new IdentityHashMap<>();
        registerCLinkers(heads, cAliases);
        final int initialBs = registerAllBs(beams, beamSigOrdinals, bAliases);

        final SIGraph sig = system.getSig();
        final int glyphCount = sheet.getGlyphIndex().getEntities().size();
        final int vertexCount = sig.vertexSet().size();
        final int edgeCount = sig.edgeSet().size();
        final int stemCount = sig.inters(StemInter.class).size();

        emit(String.format(
                "stemsheadreachsystem %s system %d profile %d interline %d bounds %s "
                        + "beamSigOrder %s beamXOrder %s headSigOrder %s headXOrder %s "
                        + "keptSeeds %d initialBs %d maxBeamLinkerDx %d maxBeamSideDx %d "
                        + "minBeamHeadDy %d minHeadHeadDy %d minSeedContrib %d "
                        + "maxLineSeedDx %s maxHeadInDx %d maxHeadOutDx %d slopeMargin %s",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                rectangle(system.getBounds()),
                ordinalRange(sourceBeams.size()),
                interTokens(beams, beamSigOrdinals),
                ordinalRange(sourceHeads.size()),
                interTokens(heads, headSigOrdinals),
                seeds.size(),
                initialBs,
                maxBeamLinkerDx,
                maxBeamSideDx,
                minBeamHeadDy,
                minHeadHeadDy,
                minSeedContrib,
                hexDouble(maxLineSeedDx),
                maxHeadInDx,
                maxHeadOutDx,
                hexDouble(slopeMargin)), hash, pageHash);

        // Production-order beam reachability is a prerequisite state transition. Invoke both real
        // private filters in their source order, but deliberately never construct the V builder.
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
            for (Object b : allB) {
                if (B_IS_ANCHOR.getBoolean(b)) continue;
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Object v : vMap.values()) {
                    final Point2D ref = (Point2D) B_REF_PT.get(b);
                    final List<AbstractBeamInter> siblings = beam.getLinker().getSiblingBeamsAt(ref);
                    V_FILTER_BEAMS.invoke(v, siblings);
                    V_FILTER_HEADS.invoke(v, siblings);
                    if (V_STEM_BUILDER.get(v) != null) {
                        throw new IllegalStateException("beam reachability crossed builder seam");
                    }
                    totals.builderChecks++;
                }
            }
        }
        final int afterBeamBs = registerAllBs(beams, beamSigOrdinals, bAliases);
        emit(String.format(
                "stemsheadreachbarena %s system %d phase afterBeamPrefix total %d added %d arenas %s",
                page,
                system.getId(),
                afterBeamBs,
                afterBeamBs - initialBs,
                bArena(beams, beamSigOrdinals, bAliases)), hash, pageHash);

        totals.heads = heads.size();
        int inspectOrdinal = 0;
        int findOrdinal = 0;
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            emit(String.format(
                    "stemsheadreachhead %s system %d xOrdinal %d sigOrdinal %d staff %d "
                            + "shape %s bounds %s center %d:%d grade %s duration %s",
                    page,
                    system.getId(),
                    headOrdinal,
                    headSigOrdinals.get(head),
                    head.getStaff().getId(),
                    head.getShape(),
                    rectangle(head.getBounds()),
                    head.getCenter().x,
                    head.getCenter().y,
                    hexDouble(head.getGrade()),
                    head.getShape().getNoteDuration()), hash, pageHash);

            for (HeadCorner corner : HeadCorner.values()) {
                final HorizontalSide hSide = corner.hSide;
                final VerticalSide vSide = corner.vSide;
                final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                if (C_STEM_BUILDER.get(c) != null || C_SEEDS.get(c) != null) {
                    throw new IllegalStateException("CLinker already crossed reachability seam");
                }
                final GeometryReplay geometry = replayGeometry(
                        sheet,
                        system,
                        head,
                        c,
                        hSide,
                        vSide,
                        beams,
                        minBeamHeadDy,
                        maxHeadInDx,
                        maxHeadOutDx,
                        slopeMargin);
                emitGeometry(
                        page,
                        system,
                        headOrdinal,
                        inspectOrdinal,
                        cAliases.get(c),
                        geometry,
                        beamSigOrdinals,
                        hash,
                        pageHash);

                final SeedReplay seedReplay = replaySeeds(
                        head,
                        c,
                        seedOrdinals,
                        minSeedContrib,
                        maxLineSeedDx);
                final List<Glyph> actualSeeds = (List<Glyph>) C_RETRIEVE_SEEDS.invoke(c);
                if (!sameIdentityList(actualSeeds, seedReplay.kept)) {
                    throw new IllegalStateException("retrieveSeeds replay differs from production");
                }
                C_SEEDS.set(c, actualSeeds);
                totals.cSeedWrites++;
                totals.seedsScanned += seedReplay.scans;
                totals.seedsKept += actualSeeds.size();
                emit(String.format(
                        "stemsheadreachseedscan %s system %d head %d inspectOrdinal %d "
                                + "neighborSeeds %d scans %d prelim %d kept %d rejects %d "
                                + "rejectSha %s",
                        page,
                        system.getId(),
                        headOrdinal,
                        inspectOrdinal,
                        ((Set<Glyph>) HEAD_NEIGHBOR_SEEDS.get(head.getLinker())).size(),
                        seedReplay.scans,
                        seedReplay.prelim,
                        actualSeeds.size(),
                        seedReplay.rejects,
                        seedReplay.rejectSha), hash, pageHash);
                for (int ordinal = 0; ordinal < actualSeeds.size(); ordinal++) {
                    final Glyph seed = actualSeeds.get(ordinal);
                    emit(String.format(
                            "stemsheadreachseed %s system %d head %d inspectOrdinal %d "
                                    + "ordinal %d keptSeed %d bounds %s centroid %d:%d "
                                    + "contrib %d distance %s",
                            page,
                            system.getId(),
                            headOrdinal,
                            inspectOrdinal,
                            ordinal,
                            seedOrdinals.get(seed),
                            rectangle(seed.getBounds()),
                            seed.getCentroid().x,
                            seed.getCentroid().y,
                            contribution((Rectangle) C_Y_RANGE.get(c), seed.getBounds()),
                            hexDouble(((Line2D) C_THEO_LINE.get(c)).ptLineDist(seed.getCentroid()))),
                            hash,
                            pageHash);
                }

                final HeadReplay headReplay = replayHeads(
                        system,
                        head,
                        c,
                        vSide,
                        minHeadHeadDy,
                        heads,
                        headXOrdinals,
                        cAliases);
                final List<Object> actualHeads = (List<Object>) C_LOOKUP_OTHER_HEADS.invoke(c);
                if (!sameIdentityList(actualHeads, headReplay.targets)) {
                    throw new IllegalStateException("lookupOtherHeads replay differs from production");
                }
                totals.headScans += headReplay.scans;
                totals.headTargets += actualHeads.size();
                emit(String.format(
                        "stemsheadreachheadscan %s system %d head %d inspectOrdinal %d "
                                + "scans %d candidates %d targets %d rejects %d rejectSha %s",
                        page,
                        system.getId(),
                        headOrdinal,
                        inspectOrdinal,
                        headReplay.scans,
                        headReplay.candidates,
                        actualHeads.size(),
                        headReplay.rejects,
                        headReplay.rejectSha), hash, pageHash);
                for (int ordinal = 0; ordinal < actualHeads.size(); ordinal++) {
                    final Object target = actualHeads.get(ordinal);
                    emit(String.format(
                            "stemsheadreachheadtarget %s system %d head %d inspectOrdinal %d "
                                    + "ordinal %d target %s",
                            page,
                            system.getId(),
                            headOrdinal,
                            inspectOrdinal,
                            ordinal,
                            cAliases.get(target)), hash, pageHash);
                }

                final List<Object> beamTargets = new ArrayList<>();
                final AbstractBeamInter targetBeam = (AbstractBeamInter) C_TARGET_BEAM.get(c);
                final String beamAction;
                if (targetBeam == null) {
                    beamAction = "none";
                } else if ((head.getShape() == Shape.NOTEHEAD_VOID)
                        && (vSide.direction() == hSide.direction())) {
                    beamAction = "voidSideSkip";
                } else {
                    beamAction = "inspect";
                    final Line2D theo = (Line2D) C_THEO_LINE.get(c);
                    final Point2D xp = LineUtil.intersection(targetBeam.getMedian(), theo);
                    final List<AbstractBeamInter> siblings = targetBeam.getLinker()
                            .getSiblingBeamsAt(xp);
                    final SiblingReplay siblingReplay = replaySiblings(
                            sheet,
                            targetBeam,
                            xp,
                            maxBeamSideDx,
                            siblings,
                            beamSigOrdinals);
                    totals.siblingScans += siblingReplay.scans;
                    emit(String.format(
                            "stemsheadreachsiblings %s system %d head %d inspectOrdinal %d "
                                    + "targetBeam %d cross %s scans %d accepted %d rejects %d "
                                    + "rejectSha %s",
                            page,
                            system.getId(),
                            headOrdinal,
                            inspectOrdinal,
                            beamSigOrdinals.get(targetBeam),
                            point(xp),
                            siblingReplay.scans,
                            siblings.size(),
                            siblingReplay.rejects,
                            siblingReplay.rejectSha), hash, pageHash);
                    for (int siblingOrdinal = 0; siblingOrdinal < siblings.size(); siblingOrdinal++) {
                        final AbstractBeamInter sibling = siblings.get(siblingOrdinal);
                        final FindReplay find = replayFind(
                                sibling,
                                theo,
                                maxBeamLinkerDx,
                                beamSigOrdinals,
                                bAliases);
                        final Object result = sibling.getLinker().findLinker(theo);
                        finishFind(
                                sibling,
                                find,
                                result,
                                beamSigOrdinals,
                                bAliases,
                                totals);
                        beamTargets.add(result);
                        emit(String.format(
                                "stemsheadreachbeamtarget %s system %d head %d "
                                        + "inspectOrdinal %d ordinal %d findOrdinal %d "
                                        + "beamSig %d before %d cross %s candidateScans %d "
                                        + "candidateSha %s best %s bestDx %s action %s "
                                        + "result %s after %d",
                                page,
                                system.getId(),
                                headOrdinal,
                                inspectOrdinal,
                                siblingOrdinal,
                                findOrdinal++,
                                beamSigOrdinals.get(sibling),
                                find.beforeCount,
                                point(find.cross),
                                find.beforeCount,
                                find.candidateSha,
                                find.best == null ? "-" : bAliases.get(find.best),
                                hexDouble(find.bestDx),
                                find.reuse ? "reuse" : "createAnchor",
                                bAliases.get(result),
                                ((List<Object>) BEAM_ALL_B.get(sibling.getLinker())).size()),
                                hash,
                                pageHash);
                    }
                }
                totals.beamTargets += beamTargets.size();

                final StringBuilder targetTokens = new StringBuilder();
                for (Object target : actualHeads) append(targetTokens, cAliases.get(target));
                for (Object target : beamTargets) append(targetTokens, bAliases.get(target));
                emit(String.format(
                        "stemsheadreachresult %s system %d head %d inspectOrdinal %d corner %s "
                                + "alias %s seeds %s headTargets %s beamAction %s "
                                + "beamTargets %s targets %s builder null",
                        page,
                        system.getId(),
                        headOrdinal,
                        inspectOrdinal,
                        corner.getId(),
                        cAliases.get(c),
                        seedTokens(actualSeeds, seedOrdinals),
                        objectTokens(actualHeads, cAliases),
                        beamAction,
                        objectTokens(beamTargets, bAliases),
                        empty(targetTokens)), hash, pageHash);
                if (C_STEM_BUILDER.get(c) != null || C_SEEDS.get(c) != actualSeeds) {
                    throw new IllegalStateException("C reachability crossed builder seam");
                }
                totals.builderChecks++;
                totals.corners++;
                inspectOrdinal++;
            }
        }

        final int finalBs = registerAllBs(beams, beamSigOrdinals, bAliases);
        if (!sameIdentityList(seeds, seedSnapshot)
                || sheet.getGlyphIndex().getEntities().size() != glyphCount
                || sig.vertexSet().size() != vertexCount
                || sig.edgeSet().size() != edgeCount
                || sig.inters(StemInter.class).size() != stemCount) {
            throw new IllegalStateException("reachability mutated forbidden production state");
        }
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            for (Object b : (List<Object>) BEAM_ALL_B.get(beam.getLinker())) {
                if ((boolean) B_IS_LINKED.invoke(b) || (boolean) B_IS_CLOSED.invoke(b)) {
                    throw new IllegalStateException("B linker state mutated");
                }
                for (Object v : ((Map<VerticalSide, Object>) B_V_LINKERS.get(b)).values()) {
                    if ((boolean) V_IS_LINKED.invoke(v) || (boolean) V_IS_CLOSED.invoke(v)
                            || V_STEM_BUILDER.get(v) != null) {
                        throw new IllegalStateException("V linker state mutated");
                    }
                }
            }
        }
        for (Object c : cAliases.keySet()) {
            if ((boolean) C_IS_LINKED.invoke(c) || (boolean) C_IS_CLOSED.invoke(c)
                    || C_STEM_BUILDER.get(c) != null) {
                throw new IllegalStateException("C linker state mutated");
            }
        }
        if (totals.corners != 4L * totals.heads) {
            throw new IllegalStateException("corner count mismatch");
        }
        if (finalBs != afterBeamBs + totals.anchorsCreated) {
            throw new IllegalStateException("head-origin anchor accounting mismatch");
        }
        emit(String.format(
                "stemsheadreachbarena %s system %d phase final total %d addedFromHeads %d "
                        + "arenas %s",
                page,
                system.getId(),
                finalBs,
                finalBs - afterBeamBs,
                bArena(beams, beamSigOrdinals, bAliases)), hash, pageHash);
        emit(String.format(
                "stemsheadreachsystemsummary %s system %d heads %d corners %d "
                        + "seedsScanned %d seedsKept %d headScans %d headTargets %d "
                        + "siblingScans %d beamTargets %d anchorsCreated %d cSeedWrites %d "
                        + "builderChecks %d hash %016x",
                page,
                system.getId(),
                totals.heads,
                totals.corners,
                totals.seedsScanned,
                totals.seedsKept,
                totals.headScans,
                totals.headTargets,
                totals.siblingScans,
                totals.beamTargets,
                totals.anchorsCreated,
                totals.cSeedWrites,
                totals.builderChecks,
                hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static GeometryReplay replayGeometry (Sheet sheet,
                                                   SystemInfo system,
                                                   HeadInter head,
                                                   Object c,
                                                   HorizontalSide hSide,
                                                   VerticalSide vSide,
                                                   List<Inter> systemBeams,
                                                   int minBeamHeadDy,
                                                   int maxHeadInDx,
                                                   int maxHeadOutDx,
                                                   double slopeMargin)
        throws IllegalAccessException
    {
        final int xDir = hSide.direction();
        final int yDir = vSide.direction();
        final Point2D ref = head.getStemReferencePoint(hSide, vSide);
        final Point2D out = new Point2D.Double(ref.getX() + xDir * maxHeadOutDx, ref.getY());
        final Point2D in = new Point2D.Double(ref.getX() - xDir * maxHeadInDx, ref.getY());
        if (C_V_SIDE.get(c) != vSide || C_Y_DIR.getInt(c) != yDir
                || !samePoint(ref, (Point2D) C_REF_PT.get(c))
                || !samePoint(out, (Point2D) C_OUT_PT.get(c))
                || !samePoint(in, (Point2D) C_IN_PT.get(c))) {
            throw new IllegalStateException("CLinker reference geometry replay differs");
        }

        final double partLimit;
        final Rectangle partBox = head.getStaff().getPart().getAreaBounds();
        if (yDir > 0) {
            partLimit = partBox.y + partBox.height - 1;
        } else {
            partLimit = partBox.y;
        }
        final Lookup initial = lookup(
                sheet, ref, out, in, xDir, yDir, slopeMargin, partLimit);

        final List<Inter> neighborBeams = (List<Inter>) HEAD_NEIGHBOR_BEAMS.get(head.getLinker());
        // HeadLinker constructor uses StemsRetriever.Parameters.vicinityMargin. Validate the actual
        // neighbor list against the public ordered intersection once its resulting bounds are known
        // from the smallest rectangle containing the live neighbors; the full margin is separately
        // frozen by the system row and prior reachability oracle.
        final List<Inter> broad = Inters.intersectedInters(
                systemBeams, GeoOrder.BY_ABSCISSA, initial.area);
        final List<Inter> beamCandidates = Inters.intersectedInters(
                neighborBeams, GeoOrder.BY_ABSCISSA, initial.area);
        if (!sameIdentityList(broad, beamCandidates)) {
            // A broad full-system scan and the production vicinity-prefilter must be equivalent
            // once intersected with this corner's lookup area.
            throw new IllegalStateException("head neighbor-beam prefilter lost a lookup hit");
        }

        final BeamGroupReplay groupReplay = replayBeamGroups(
                neighborBeams, initial.area, beamCandidates, ref, yDir, minBeamHeadDy);
        final List<BeamGroupInter> actualGroups = (List<BeamGroupInter>) C_BEAM_GROUPS.get(c);
        if (!sameIdentityList(actualGroups, groupReplay.groups)) {
            throw new IllegalStateException("beam-group lookup replay differs");
        }

        AbstractBeamInter expectedTargetBeam = null;
        Point2D target = initial.theo.getP2();
        Lookup finalLookup = initial;
        final SemanticDigest targetRejects = new SemanticDigest();
        int targetScans = 0;
        outer:
        for (int groupOrdinal = 0; groupOrdinal < groupReplay.groups.size(); groupOrdinal++) {
            final BeamGroupInter group = groupReplay.groups.get(groupOrdinal);
            final List<Inter> members = group.getMembers();
            StemsRetriever.sortBeamsFromRef(ref, yDir, members);
            for (int memberOrdinal = 0; memberOrdinal < members.size(); memberOrdinal++) {
                final Inter member = members.get(memberOrdinal);
                final AbstractBeamInter beam = (AbstractBeamInter) member;
                targetScans++;
                if (!beam.getMedian().intersectsLine(initial.theo)) {
                    targetRejects.add("noMedianIntersection:" + groupOrdinal + ":"
                            + memberOrdinal + ":" + line(beam.getMedian()));
                    continue;
                }
                if (head.getShape().isSmallHead()) {
                    final AbstractBeamInter nearest = (AbstractBeamInter) members.get(0);
                    final Line2D border = nearest.getBorder(vSide.opposite());
                    final double yLimit = LineUtil.yAtX(border, ref.getX());
                    finalLookup = lookup(
                            sheet, ref, out, in, xDir, yDir, slopeMargin, yLimit);
                    target = StemsRetriever.getTargetPt(
                            ref, border, sheet.getSkew().getSlope());
                } else {
                    expectedTargetBeam = (AbstractBeamInter) members.get(members.size() - 1);
                    final Line2D border = expectedTargetBeam.getBorder(vSide);
                    final double margin = expectedTargetBeam.getHeight();
                    final Line2D limit = new Line2D.Double(
                            border.getX1(),
                            border.getY1() + yDir * margin,
                            border.getX2(),
                            border.getY2() + yDir * margin);
                    final double yLimit = LineUtil.yAtX(limit, ref.getX());
                    finalLookup = lookup(
                            sheet, ref, out, in, xDir, yDir, slopeMargin, yLimit);
                    target = StemsRetriever.getTargetPt(
                            ref, border, sheet.getSkew().getSlope());
                }
                break outer;
            }
        }
        final Line2D expectedTheo = new Line2D.Double(ref, target);
        final Rectangle expectedRange = new Rectangle(
                0,
                (int) Math.rint(yDir > 0 ? ref.getY() : target.getY()),
                0,
                (int) Math.rint(Math.abs(target.getY() - ref.getY())));
        final Area actualArea = (Area) C_LU_AREA.get(c);
        if (C_TARGET_BEAM.get(c) != expectedTargetBeam
                || !samePoint(target, (Point2D) C_TARGET_PT.get(c))
                || !sameLine(expectedTheo, (Line2D) C_THEO_LINE.get(c))
                || !expectedRange.equals(C_Y_RANGE.get(c))
                || !actualArea.equals(finalLookup.area)
                || !sameRectangle2D(actualArea.getBounds2D(), finalLookup.area.getBounds2D())) {
            throw new IllegalStateException("CLinker target/lookup geometry replay differs");
        }
        return new GeometryReplay(
                hSide,
                vSide,
                ref,
                out,
                in,
                (Glyph) C_STUMP.get(c),
                partLimit,
                initial,
                groupReplay,
                targetScans,
                targetRejects.count(),
                targetRejects.hex(),
                expectedTargetBeam,
                target,
                finalLookup,
                expectedTheo,
                expectedRange);
    }

    private static Lookup lookup (Sheet sheet,
                                  Point2D ref,
                                  Point2D out,
                                  Point2D in,
                                  int xDir,
                                  int yDir,
                                  double slopeMargin,
                                  double yLimit)
    {
        final double slope = -sheet.getSkew().getSlope();
        final double dSlope = xDir * yDir * slopeMargin;
        final double dy = yLimit - out.getY();
        final Point2D q2 = new Point2D.Double(
                in.getX() + (slope - dSlope) * dy, yLimit);
        final Point2D q3 = new Point2D.Double(
                out.getX() + (slope + dSlope) * dy, yLimit);
        final Path2D path = new Path2D.Double();
        path.moveTo(out.getX(), out.getY());
        path.lineTo(in.getX(), in.getY());
        path.lineTo(q2.getX(), q2.getY());
        path.lineTo(q3.getX(), q3.getY());
        path.closePath();
        final Point2D target = StemsRetriever.getTargetPt(
                ref,
                new Line2D.Double(0, yLimit, 100, yLimit),
                sheet.getSkew().getSlope());
        return new Lookup(
                yLimit,
                new Area(path),
                new Line2D.Double(ref, target),
                point(out) + ":" + point(in) + ":" + point(q2) + ":" + point(q3));
    }

    private static BeamGroupReplay replayBeamGroups (List<Inter> neighbors,
                                                     Area area,
                                                     List<Inter> actualCandidates,
                                                     Point2D ref,
                                                     int yDir,
                                                     int minBeamHeadDy)
    {
        final SemanticDigest rejects = new SemanticDigest();
        final List<Inter> candidates = new ArrayList<>();
        final double xMax = area.getBounds().getMaxX();
        int scans = 0;
        for (int ordinal = 0; ordinal < neighbors.size(); ordinal++) {
            final Inter inter = neighbors.get(ordinal);
            scans++;
            if (inter.isRemoved()) {
                rejects.add("removed:" + ordinal);
            } else if (area.intersects(inter.getBounds())) {
                candidates.add(inter);
            } else if (inter.getBounds().x > xMax) {
                rejects.add("breakX:" + ordinal + ":" + rectangle(inter.getBounds()));
                break;
            } else {
                rejects.add("outside:" + ordinal + ":" + rectangle(inter.getBounds()));
            }
        }
        if (!sameIdentityList(candidates, actualCandidates)) {
            throw new IllegalStateException("beam-area scan replay differs");
        }
        final List<Inter> kept = new ArrayList<>();
        final double slope = candidates.isEmpty()
                ? 0 : candidates.get(0).getSig().getSystem().getSheet().getSkew().getSlope();
        for (int ordinal = 0; ordinal < candidates.size(); ordinal++) {
            final AbstractBeamInter beam = (AbstractBeamInter) candidates.get(ordinal);
            final Line2D limit = beam.getBorder(VerticalSide.of(-yDir));
            final Point2D target = StemsRetriever.getTargetPt(ref, limit, slope);
            final double direction = yDir * (target.getY() - ref.getY());
            if (direction <= 0) {
                rejects.add("direction:" + ordinal + ":" + point(target) + ":" + hexDouble(direction));
            } else {
                kept.add(beam);
            }
        }
        StemsRetriever.sortBeamsFromRef(ref, yDir, kept);
        final Set<BeamGroupInter> groupSet = new LinkedHashSet<>();
        for (int ordinal = 0; ordinal < kept.size(); ordinal++) {
            final AbstractBeamInter beam = (AbstractBeamInter) kept.get(ordinal);
            if (groupSet.isEmpty()) {
                final Line2D limit = beam.getBorder(VerticalSide.of(-yDir));
                final Point2D target = StemsRetriever.getTargetPt(ref, limit, slope);
                final double distance = yDir * (target.getY() - ref.getY());
                if (distance < minBeamHeadDy) {
                    rejects.add("near:" + ordinal + ":" + point(target) + ":" + hexDouble(distance));
                    continue;
                }
            }
            groupSet.add(beam.getGroup());
        }
        return new BeamGroupReplay(
                new ArrayList<>(groupSet), scans, rejects.count(), rejects.hex());
    }

    private static SeedReplay replaySeeds (HeadInter head,
                                           Object c,
                                           IdentityHashMap<Glyph, Integer> seedOrdinals,
                                           int minSeedContrib,
                                           double maxLineSeedDx)
        throws IllegalAccessException
    {
        final Set<Glyph> neighbors = (Set<Glyph>) HEAD_NEIGHBOR_SEEDS.get(head.getLinker());
        final Area area = (Area) C_LU_AREA.get(c);
        final Glyph stump = (Glyph) C_STUMP.get(c);
        final Rectangle stumpBox = stump != null ? stump.getBounds() : null;
        final Rectangle yRange = (Rectangle) C_Y_RANGE.get(c);
        final Line2D theo = (Line2D) C_THEO_LINE.get(c);
        final SemanticDigest rejects = new SemanticDigest();
        final List<Glyph> prelim = new ArrayList<>();
        int scans = 0;
        for (Glyph seed : neighbors) {
            scans++;
            final int ordinal = required(seedOrdinals, seed);
            final Rectangle box = seed.getBounds();
            if (!area.intersects(box)) {
                rejects.add("outside:" + ordinal + ":" + rectangle(box));
                continue;
            }
            if (stumpBox != null && GeoUtil.yOverlap(box, stumpBox) > 0) {
                rejects.add("stump:" + ordinal + ":" + rectangle(box));
                continue;
            }
            final int contrib = contribution(yRange, box);
            if (contrib < minSeedContrib) {
                rejects.add("contrib:" + ordinal + ":" + contrib + ":" + rectangle(box));
                continue;
            }
            final double distance = theo.ptLineDist(seed.getCentroid());
            if (distance > maxLineSeedDx) {
                rejects.add("distance:" + ordinal + ":" + hexDouble(distance));
                continue;
            }
            prelim.add(seed);
        }
        Collections.sort(
                prelim,
                (left, right) -> Integer.compare(
                        contribution(yRange, right.getBounds()),
                        contribution(yRange, left.getBounds())));
        final List<Glyph> kept = new ArrayList<>();
        outer:
        for (Glyph seed : prelim) {
            for (Glyph previous : kept) {
                if (GeoUtil.yOverlap(seed.getBounds(), previous.getBounds()) > 0) {
                    rejects.add("overlap:" + required(seedOrdinals, seed) + ":"
                            + required(seedOrdinals, previous));
                    continue outer;
                }
            }
            kept.add(seed);
        }
        return new SeedReplay(
                kept, scans, prelim.size(), rejects.count(), rejects.hex());
    }

    private static HeadReplay replayHeads (SystemInfo system,
                                           HeadInter head,
                                           Object c,
                                           VerticalSide vSide,
                                           int minHeadHeadDy,
                                           List<Inter> systemHeads,
                                           IdentityHashMap<HeadInter, Integer> headXOrdinals,
                                           IdentityHashMap<Object, String> cAliases)
        throws IllegalAccessException
    {
        final int yDir = vSide.direction();
        final Point2D ref = (Point2D) C_REF_PT.get(c);
        final Area area = (Area) C_LU_AREA.get(c);
        final Set<Inter> competing = system.getSig().getCompetingInters(head);
        final Rational duration = head.getShape().getNoteDuration();
        final double yLast = ref.getY() + yDir * minHeadHeadDy;
        final SemanticDigest rejects = new SemanticDigest();
        final List<Object> targets = new ArrayList<>();
        int scans = 0;
        int candidates = 0;
        final double xMax = area.getBounds().getMaxX();
        for (Inter inter : systemHeads) {
            final HeadInter candidate = (HeadInter) inter;
            scans++;
            if (candidate.isRemoved()) {
                rejects.add("removed:" + headXOrdinals.get(candidate));
                continue;
            }
            if (!area.intersects(candidate.getBounds())) {
                if (candidate.getBounds().x > xMax) {
                    rejects.add("breakX:" + headXOrdinals.get(candidate));
                    break;
                }
                rejects.add("outside:" + headXOrdinals.get(candidate));
                continue;
            }
            candidates++;
            if (candidate == head) {
                rejects.add("self:" + headXOrdinals.get(candidate));
                continue;
            }
            if (competing.contains(candidate)) {
                rejects.add("competing:" + headXOrdinals.get(candidate));
                continue;
            }
            if (!candidate.getShape().getNoteDuration().equals(duration)) {
                rejects.add("duration:" + headXOrdinals.get(candidate) + ":"
                        + candidate.getShape().getNoteDuration());
                continue;
            }
            final double dy = yDir * (candidate.getCenter().y - yLast);
            if (dy < 0) {
                rejects.add("near:" + headXOrdinals.get(candidate) + ":" + hexDouble(dy));
                continue;
            }
            for (HorizontalSide side : HorizontalSide.values()) {
                final Object target = candidate.getLinker().getCornerLinker(side, vSide);
                final Point2D targetRef = (Point2D) C_REF_PT.get(target);
                if (area.contains(targetRef)) {
                    targets.add(target);
                } else {
                    rejects.add("cornerOutside:" + headXOrdinals.get(candidate) + ":" + side
                            + ":" + point(targetRef));
                }
            }
        }
        for (Object target : targets) {
            if (!cAliases.containsKey(target)) {
                throw new IllegalStateException("unaliased C target");
            }
        }
        return new HeadReplay(
                targets, scans, candidates, rejects.count(), rejects.hex());
    }

    private static SiblingReplay replaySiblings (Sheet sheet,
                                                 AbstractBeamInter beam,
                                                 Point2D point,
                                                 int margin,
                                                 List<AbstractBeamInter> actual,
                                                 IdentityHashMap<Inter, Integer> beamSigOrdinals)
    {
        final Line2D vertical = sheet.getSkew().skewedVertical(point);
        final List<AbstractBeamInter> expected = new ArrayList<>();
        final IdentityHashMap<AbstractBeamInter, Point2D> crosses = new IdentityHashMap<>();
        final SemanticDigest rejects = new SemanticDigest();
        final List<Inter> members = beam.getGroup().getMembers();
        for (Inter inter : members) {
            final AbstractBeamInter sibling = (AbstractBeamInter) inter;
            final Point2D cross = LineUtil.intersection(vertical, sibling.getMedian());
            crosses.put(sibling, cross);
            if (sibling.getMedian().getX1() - margin <= cross.getX()
                    && cross.getX() <= sibling.getMedian().getX2() + margin) {
                expected.add(sibling);
            } else {
                rejects.add("outside:" + beamSigOrdinals.get(sibling) + ":" + point(cross));
            }
        }
        Collections.sort(
                expected,
                Comparator.comparingDouble(sibling -> crosses.get(sibling).getY()));
        if (!sameIdentityList(expected, actual)) {
            throw new IllegalStateException("sibling replay differs from production");
        }
        return new SiblingReplay(
                members.size(), rejects.count(), rejects.hex());
    }

    private static FindReplay replayFind (AbstractBeamInter beam,
                                          Line2D stemLine,
                                          int maxBeamLinkerDx,
                                          IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                          IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        final Point2D cross = LineUtil.intersection(stemLine, beam.getMedian());
        final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
        Object best = null;
        double bestDx = Double.MAX_VALUE;
        final SemanticDigest candidates = new SemanticDigest();
        for (int ordinal = 0; ordinal < allB.size(); ordinal++) {
            final Object candidate = allB.get(ordinal);
            final double dx = Math.abs(((Point2D) B_REF_PT.get(candidate)).getX() - cross.getX());
            final boolean replace = bestDx > dx;
            candidates.add(ordinal + ":" + point((Point2D) B_REF_PT.get(candidate)) + ":"
                    + hexDouble(dx) + ":" + (replace ? "replace" : "keep"));
            if (replace) {
                bestDx = dx;
                best = candidate;
            }
        }
        return new FindReplay(
                allB.size(), cross, candidates.hex(), best, bestDx,
                bestDx <= maxBeamLinkerDx);
    }

    private static void finishFind (AbstractBeamInter beam,
                                    FindReplay replay,
                                    Object actual,
                                    IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                    IdentityHashMap<Object, String> aliases,
                                    Totals totals)
        throws IllegalAccessException
    {
        final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
        if (replay.reuse) {
            if (actual != replay.best || allB.size() != replay.beforeCount) {
                throw new IllegalStateException("findLinker reuse replay differs");
            }
        } else {
            if (allB.size() != replay.beforeCount + 1
                    || allB.get(replay.beforeCount) != actual
                    || !B_IS_ANCHOR.getBoolean(actual)
                    || !samePoint((Point2D) B_REF_PT.get(actual), replay.cross)) {
                throw new IllegalStateException("findLinker anchor replay differs");
            }
            totals.anchorsCreated++;
        }
        registerB(beam, actual, beamSigOrdinals, aliases);
    }

    private static void emitGeometry (String page,
                                      SystemInfo system,
                                      int headOrdinal,
                                      int inspectOrdinal,
                                      String alias,
                                      GeometryReplay replay,
                                      IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                      RowHasher... hashes)
    {
        emit(String.format(
                "stemsheadreachcorner %s system %d head %d inspectOrdinal %d corner %s "
                        + "alias %s hSide %s vSide %s xDir %d yDir %d ref %s out %s in %s "
                        + "stump %s partLimit %s initialYLimit %s initialQuad %s "
                        + "beamScans %d groups %s beamRejects %d beamRejectSha %s "
                        + "targetScans %d targetRejects %d targetRejectSha %s "
                        + "targetBeam %s target %s finalYLimit %s finalQuad %s "
                        + "lookupBounds %s lookupBounds2d %s theo %s yRange %s",
                page,
                system.getId(),
                headOrdinal,
                inspectOrdinal,
                replay.vSide.name().charAt(0) + "" + replay.hSide.name().charAt(0),
                alias,
                replay.hSide,
                replay.vSide,
                replay.hSide.direction(),
                replay.vSide.direction(),
                point(replay.ref),
                point(replay.out),
                point(replay.in),
                replay.stump == null ? "-" : rectangle(replay.stump.getBounds()) + ":"
                        + replay.stump.getWeight(),
                hexDouble(replay.partLimit),
                hexDouble(replay.initial.yLimit),
                replay.initial.quad,
                replay.groupReplay.scans,
                groupTokens(replay.groupReplay.groups, beamSigOrdinals),
                replay.groupReplay.rejects,
                replay.groupReplay.rejectSha,
                replay.targetScans,
                replay.targetRejects,
                replay.targetRejectSha,
                replay.targetBeam == null ? "-" : beamSigOrdinals.get(replay.targetBeam),
                point(replay.target),
                hexDouble(replay.finalLookup.yLimit),
                replay.finalLookup.quad,
                rectangle(replay.finalLookup.area.getBounds()),
                rectangle2D(replay.finalLookup.area.getBounds2D()),
                line(replay.theo),
                rectangle(replay.yRange)), hashes);
    }

    private static void registerCLinkers (List<Inter> heads,
                                          IdentityHashMap<Object, String> aliases)
    {
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            for (HorizontalSide hSide : HorizontalSide.values()) {
                for (VerticalSide vSide : VerticalSide.values()) {
                    final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                    aliases.put(c, "h:" + headOrdinal + ":" + vSide.name().charAt(0)
                            + hSide.name().charAt(0));
                }
            }
        }
    }

    private static int registerAllBs (List<Inter> beams,
                                      IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                      IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        int total = 0;
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
            total += allB.size();
            for (Object b : allB) registerB(beam, b, beamSigOrdinals, aliases);
        }
        return total;
    }

    private static void registerB (AbstractBeamInter beam,
                                   Object b,
                                   IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                   IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
        final int ordinal = identityIndex(allB, b);
        if (ordinal < 0) throw new IllegalStateException("B not in owner arena");
        final String token = "b:" + beamSigOrdinals.get(beam) + ":" + ordinal;
        final String old = aliases.putIfAbsent(b, token);
        if (old != null && !old.equals(token)) {
            throw new IllegalStateException("B alias changed");
        }
    }

    private static String bArena (List<Inter> beams,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                  IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        final StringBuilder result = new StringBuilder();
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
            final StringBuilder members = new StringBuilder();
            for (Object b : allB) {
                registerB(beam, b, beamSigOrdinals, aliases);
                append(members, aliases.get(b) + (B_IS_ANCHOR.getBoolean(b) ? ":A" : ":I"));
            }
            append(result, beamSigOrdinals.get(beam) + "=[" + empty(members) + "]");
        }
        return empty(result);
    }

    private static Sheet loadPage (Path path,
                                   int wanted)
        throws Exception
    {
        final Book book = new Book(path);
        book.createStubs();
        SheetStub wantedStub = null;
        for (SheetStub stub : book.getValidStubs()) {
            if (stub.getNumber() == wanted) {
                wantedStub = stub;
                break;
            }
        }
        if (wantedStub == null) {
            throw new IllegalArgumentException("missing sheet " + wanted + " in " + path);
        }
        wantedStub.reachStep(OmrStep.HEADS, false);
        return wantedStub.getSheet();
    }

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS head reachability oracle.");
        System.out.println("#");
        System.out.println("# Fresh Epsilon-GC JVMs reach real HEADS, construct all beam/head linkers in");
        System.out.println("# production stable-x order, replay beam reachability, then visit heads by x and");
        System.out.println("# HeadCorner.values order TR,BL,TL,BR. The seam is immediately before each");
        System.out.println("# CLinker new StemBuilder call; no C builders are constructed.");
        System.out.println("# Geometry, lookup groups, seeds, other heads, sibling beams, findLinker");
        System.out.println("# reuse/append, C-before-B targets, and immediate/final B arenas are graded.");
        System.out.println("# Dense rejected scans are count + ordered semantic SHA-256; every accepted");
        System.out.println("# target and every B mutation is explicit. Doubles include raw bits.");
        System.out.println("# The only permitted reachability mutations are C.seeds assignments and");
        System.out.println("# cross-beam/head B anchor appends; SIG, glyph index, stems, links, and builders");
        System.out.println("# remain unchanged from the post-linker baseline.");
    }

    private static Field field (Class<?> owner,
                                String name)
        throws NoSuchFieldException
    {
        final Field result = owner.getDeclaredField(name);
        result.setAccessible(true);
        return result;
    }

    private static <T> IdentityHashMap<T, Integer> ordinals (List<T> values)
    {
        final IdentityHashMap<T, Integer> result = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (result.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate identity");
            }
        }
        return result;
    }

    private static IdentityHashMap<Inter, Integer> interOrdinals (List<? extends Inter> values)
    {
        final IdentityHashMap<Inter, Integer> result = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (result.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate inter identity");
            }
        }
        return result;
    }

    private static int required (IdentityHashMap<Glyph, Integer> ordinals,
                                 Glyph value)
    {
        final Integer ordinal = ordinals.get(value);
        if (ordinal == null) throw new IllegalStateException("glyph outside kept-seed pool");
        return ordinal;
    }

    private static int contribution (Rectangle range,
                                     Rectangle box)
    {
        return Math.max(0, GeoUtil.yOverlap(range, box));
    }

    private static String seedTokens (List<Glyph> values,
                                      IdentityHashMap<Glyph, Integer> ordinals)
    {
        final StringBuilder result = new StringBuilder();
        for (Glyph value : values) append(result, Integer.toString(required(ordinals, value)));
        return empty(result);
    }

    private static String objectTokens (List<Object> values,
                                        IdentityHashMap<Object, String> aliases)
    {
        final StringBuilder result = new StringBuilder();
        for (Object value : values) append(result, aliases.get(value));
        return empty(result);
    }

    private static String interTokens (List<? extends Inter> values,
                                       IdentityHashMap<Inter, Integer> ordinals)
    {
        final StringBuilder result = new StringBuilder();
        for (Inter value : values) append(result, Integer.toString(ordinals.get(value)));
        return empty(result);
    }

    private static String groupTokens (List<BeamGroupInter> groups,
                                       IdentityHashMap<Inter, Integer> beamOrdinals)
    {
        final StringBuilder result = new StringBuilder();
        for (BeamGroupInter group : groups) {
            final StringBuilder members = new StringBuilder();
            for (Inter member : group.getMembers()) {
                append(members, Integer.toString(beamOrdinals.get(member)));
            }
            append(result, "[" + empty(members) + "]");
        }
        return empty(result);
    }

    private static String ordinalRange (int size)
    {
        final StringBuilder result = new StringBuilder();
        for (int ordinal = 0; ordinal < size; ordinal++) append(result, Integer.toString(ordinal));
        return empty(result);
    }

    private static void append (StringBuilder builder,
                                String value)
    {
        if (builder.length() != 0) builder.append(',');
        builder.append(value);
    }

    private static String empty (StringBuilder builder)
    {
        return builder.length() == 0 ? "-" : builder.toString();
    }

    private static int identityIndex (List<?> values,
                                      Object target)
    {
        for (int index = 0; index < values.size(); index++) {
            if (values.get(index) == target) return index;
        }
        return -1;
    }

    private static boolean sameIdentityList (List<?> left,
                                             List<?> right)
    {
        if (left.size() != right.size()) return false;
        for (int index = 0; index < left.size(); index++) {
            if (left.get(index) != right.get(index)) return false;
        }
        return true;
    }

    private static boolean samePoint (Point2D left,
                                      Point2D right)
    {
        return Double.doubleToLongBits(left.getX()) == Double.doubleToLongBits(right.getX())
                && Double.doubleToLongBits(left.getY()) == Double.doubleToLongBits(right.getY());
    }

    private static boolean sameLine (Line2D left,
                                     Line2D right)
    {
        return Double.doubleToLongBits(left.getX1()) == Double.doubleToLongBits(right.getX1())
                && Double.doubleToLongBits(left.getY1()) == Double.doubleToLongBits(right.getY1())
                && Double.doubleToLongBits(left.getX2()) == Double.doubleToLongBits(right.getX2())
                && Double.doubleToLongBits(left.getY2()) == Double.doubleToLongBits(right.getY2());
    }

    private static boolean sameRectangle2D (java.awt.geom.Rectangle2D left,
                                            java.awt.geom.Rectangle2D right)
    {
        return Double.doubleToLongBits(left.getX()) == Double.doubleToLongBits(right.getX())
                && Double.doubleToLongBits(left.getY()) == Double.doubleToLongBits(right.getY())
                && Double.doubleToLongBits(left.getWidth())
                        == Double.doubleToLongBits(right.getWidth())
                && Double.doubleToLongBits(left.getHeight())
                        == Double.doubleToLongBits(right.getHeight());
    }

    private static String rectangle (Rectangle box)
    {
        return box.x + ":" + box.y + ":" + box.width + ":" + box.height;
    }

    private static String rectangle2D (java.awt.geom.Rectangle2D box)
    {
        return hexDouble(box.getX()) + ":" + hexDouble(box.getY()) + ":"
                + hexDouble(box.getWidth()) + ":" + hexDouble(box.getHeight());
    }

    private static String line (Line2D value)
    {
        return point(value.getP1()) + ":" + point(value.getP2());
    }

    private static String point (Point2D value)
    {
        return hexDouble(value.getX()) + ":" + hexDouble(value.getY());
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static void emit (String row,
                              RowHasher... hashes)
    {
        System.out.println(row);
        for (RowHasher hash : hashes) hash.add(row);
    }

    private record Lookup(double yLimit, Area area, Line2D theo, String quad)
    {
    }

    private record BeamGroupReplay(
            List<BeamGroupInter> groups, int scans, int rejects, String rejectSha)
    {
    }

    private record GeometryReplay(
            HorizontalSide hSide,
            VerticalSide vSide,
            Point2D ref,
            Point2D out,
            Point2D in,
            Glyph stump,
            double partLimit,
            Lookup initial,
            BeamGroupReplay groupReplay,
            int targetScans,
            int targetRejects,
            String targetRejectSha,
            AbstractBeamInter targetBeam,
            Point2D target,
            Lookup finalLookup,
            Line2D theo,
            Rectangle yRange)
    {
    }

    private record SeedReplay(
            List<Glyph> kept, int scans, int prelim, int rejects, String rejectSha)
    {
    }

    private record HeadReplay(
            List<Object> targets, int scans, int candidates, int rejects, String rejectSha)
    {
    }

    private record SiblingReplay(int scans, int rejects, String rejectSha)
    {
    }

    private record FindReplay(
            int beforeCount,
            Point2D cross,
            String candidateSha,
            Object best,
            double bestDx,
            boolean reuse)
    {
    }

    private static final class Totals
    {
        long heads;
        long corners;
        long seedsScanned;
        long seedsKept;
        long headScans;
        long headTargets;
        long siblingScans;
        long beamTargets;
        long anchorsCreated;
        long cSeedWrites;
        long builderChecks;

        void include (Totals that)
        {
            heads += that.heads;
            corners += that.corners;
            seedsScanned += that.seedsScanned;
            seedsKept += that.seedsKept;
            headScans += that.headScans;
            headTargets += that.headTargets;
            siblingScans += that.siblingScans;
            beamTargets += that.beamTargets;
            anchorsCreated += that.anchorsCreated;
            cSeedWrites += that.cSeedWrites;
            builderChecks += that.builderChecks;
        }
    }

    private static final class SemanticDigest
    {
        private final MessageDigest digest;
        private int count;

        SemanticDigest ()
        {
            try {
                digest = MessageDigest.getInstance("SHA-256");
            } catch (java.security.NoSuchAlgorithmException ex) {
                throw new ExceptionInInitializerError(ex);
            }
        }

        void add (String value)
        {
            digest.update((value + "\n").getBytes(StandardCharsets.UTF_8));
            count++;
        }

        int count ()
        {
            return count;
        }

        String hex ()
        {
            final byte[] bytes;
            try {
                bytes = ((MessageDigest) digest.clone()).digest();
            } catch (CloneNotSupportedException ex) {
                throw new IllegalStateException(ex);
            }
            final StringBuilder result = new StringBuilder(2 * bytes.length);
            for (byte value : bytes) result.append(String.format("%02x", value & 0xff));
            return result.toString();
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            for (byte value : (row + "\n").getBytes(StandardCharsets.UTF_8)) {
                this.value ^= value & 0xffL;
                this.value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }
}
