// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Path2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.EnumMap;
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
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.math.GeoOrder;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Part;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamGroupInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact, identity-free oracle for constructor-time {@link BeamLinker} B/V topology and geometry.
 *
 * <p>Every page reaches the real HEADS boundary. The probe then follows the beam-construction
 * prefix of {@code StemsRetriever.inspectStems}: it installs the exact parameters and purged
 * vertical-seed pool, keeps the stable-x system-beam list live while constructors run, and assigns
 * each surviving beam its real linker. It stops after every constructor and before HeadLinker
 * construction, {@code inspectVLinkers}, anchors, or StemBuilder.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamVLinkerProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;

    private static final Field RETRIEVER_SYSTEM_SEEDS;

    private static final Field RETRIEVER_SYSTEM_BEAMS;

    private static final java.lang.reflect.Method PURGE_NO_STEM_SEEDS;

    private static final Field PARAM_SLOPE_MARGIN;

    private static final Field PARAM_VICINITY_MARGIN;

    private static final Field PARAM_MAIN_STEM_THICKNESS;

    private static final Field PARAM_HALF_BEAM_LU_DX;

    private static final Field PARAM_MAX_BEAM_SIDE_DX;

    private static final Field PARAM_MAX_BEAM_GROUP_DY;

    private static final Field PARAM_MAX_BEAM_SEED_DY_RATIO;

    private static final Field LINKER_NEIGHBOR_SEEDS;

    private static final Field LINKER_STUMPS;

    private static final Field LINKER_SIDE_STUMPS;

    private static final Field LINKER_ALL_B;

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

    private static final Field V_LU_AREA;

    private static final Field V_THEO_LINE;

    private static final Field V_SEEDS;

    private static final Field V_STOPPING_HEAD_SIDE;

    private static final Field V_STEM_BUILDER;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            final Class<?> bLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            final Class<?> vLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker");

            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);
            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = declaredField(StemsRetriever.class, "systemBeams");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            PARAM_SLOPE_MARGIN = declaredField(parameters, "slopeMargin");
            PARAM_VICINITY_MARGIN = declaredField(parameters, "vicinityMargin");
            PARAM_MAIN_STEM_THICKNESS = declaredField(parameters, "mainStemThickness");
            PARAM_HALF_BEAM_LU_DX = declaredField(parameters, "halfBeamLuDx");
            PARAM_MAX_BEAM_SIDE_DX = declaredField(parameters, "maxBeamSideDx");
            PARAM_MAX_BEAM_GROUP_DY = declaredField(parameters, "maxBeamGroupDy");
            PARAM_MAX_BEAM_SEED_DY_RATIO = declaredField(parameters, "maxBeamSeedDyRatio");

            LINKER_NEIGHBOR_SEEDS = declaredField(BeamLinker.class, "neighborSeeds");
            LINKER_STUMPS = declaredField(BeamLinker.class, "stumps");
            LINKER_SIDE_STUMPS = declaredField(BeamLinker.class, "sideStumps");
            LINKER_ALL_B = declaredField(BeamLinker.class, "allBLinkers");
            LINKER_SIDE_B = declaredField(BeamLinker.class, "sideBLinkers");
            LINKER_STUMP_V = declaredField(BeamLinker.class, "stumpLinkers");

            B_ID = declaredField(bLinker, "id");
            B_H_SIDE = declaredField(bLinker, "hSide");
            B_REF_PT = declaredField(bLinker, "refPt");
            B_STUMP = declaredField(bLinker, "stump");
            B_IS_ANCHOR = declaredField(bLinker, "isAnchor");
            B_V_LINKERS = declaredField(bLinker, "vLinkers");
            B_LINKED = declaredField(bLinker, "linked");
            B_CLOSED = declaredField(bLinker, "closed");

            V_V_SIDE = declaredField(vLinker, "vSide");
            V_Y_DIR = declaredField(vLinker, "yDir");
            V_LU_AREA = declaredField(vLinker, "luArea");
            V_THEO_LINE = declaredField(vLinker, "theoLine");
            V_SEEDS = declaredField(vLinker, "seeds");
            V_STOPPING_HEAD_SIDE = declaredField(vLinker, "stoppingHeadSide");
            V_STEM_BUILDER = declaredField(vLinker, "sb");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamVLinkerProbe ()
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
                "stemsbeamvlinkerpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, totals, hash);
        }
        System.out.printf(
                "stemsbeamvlinkerpagesummary %s systems %d constructors %d survivors %d "
                        + "tremolos %d parts %d stumpBs %d orphanAttempts %d orphanCreated %d "
                        + "Bs %d Vs %d stumpVs %d initialGeometries %d rebuiltGeometries %d "
                        + "alienNeighborScans %d alienCandidates %d alienSurvivors %d "
                        + "alienLimiters %d seedChecks %d reachableSeeds %d orphanBs %d "
                        + "zeroDirectionBs %d orphanVs %d topVs %d bottomVs %d sideBLinks %d "
                        + "orphanChecks %d "
                        + "orphanExisting %d orphanEmpty %d orphanInterior %d "
                        + "partFoldRows %d finalGeometries %d geometryRows %d alienLookups %d "
                        + "alienCandidateRows %d "
                        + "alienGroupDrops %d alienBadDrops %d alienHookDrops %d alienMissDrops %d "
                        + "alienAlignedDrops %d alienAcceptedRows %d alienSortRows %d "
                        + "alienShrinkRows %d seedCandidateRows %d seedHitRows %d hash %016x%n",
                page,
                sheet.getSystems().size(),
                totals.constructors,
                totals.liveBeams,
                totals.tremolos,
                totals.parts,
                totals.stumpB,
                totals.orphanChecks - totals.orphanExisting,
                totals.orphanCreated,
                totals.bLinkers,
                totals.vLinkers,
                totals.stumpV,
                totals.vLinkers,
                totals.alienShrinks,
                totals.alienCandidates,
                totals.alienCandidates,
                totals.alienAccepted,
                totals.alienShrinks,
                totals.seedCandidates,
                totals.seedHits,
                totals.orphanB,
                totals.zeroDirectionB,
                totals.orphanV,
                totals.topV,
                totals.bottomV,
                totals.sideBLinks,
                totals.orphanChecks,
                totals.orphanExisting,
                totals.orphanEmpty,
                totals.orphanInterior,
                totals.limitStaffRows,
                totals.vLinkers,
                totals.geometries,
                totals.alienLookups,
                totals.alienCandidates,
                totals.alienGroupDrops,
                totals.alienBadDrops,
                totals.alienHookDrops,
                totals.alienMissDrops,
                totals.alienAlignedDrops,
                totals.alienAccepted,
                totals.alienSortRows,
                totals.alienShrinks,
                totals.seedCandidates,
                totals.seedHits,
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

        final double slopeMargin = PARAM_SLOPE_MARGIN.getDouble(params);
        final int vicinityMargin = PARAM_VICINITY_MARGIN.getInt(params);
        final int mainStemThickness = PARAM_MAIN_STEM_THICKNESS.getInt(params);
        final double halfBeamLuDx = PARAM_HALF_BEAM_LU_DX.getDouble(params);
        final int maxBeamSideDx = PARAM_MAX_BEAM_SIDE_DX.getInt(params);
        final int maxBeamGroupDy = PARAM_MAX_BEAM_GROUP_DY.getInt(params);
        final double maxBeamSeedDyRatio = PARAM_MAX_BEAM_SEED_DY_RATIO.getDouble(params);

        final List<Glyph> sourceSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        final List<Glyph> keptSeeds = new ArrayList<>(sourceSeeds);
        PURGE_NO_STEM_SEEDS.invoke(retriever, keptSeeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, keptSeeds);
        final IdentityHashMap<Glyph, Integer> keptOrdinals = identityOrdinals(keptSeeds);

        final List<Inter> sourceBeams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> sigOrdinals = interOrdinals(sourceBeams);
        final List<Inter> liveBeams = new ArrayList<>(sourceBeams);
        Collections.sort(liveBeams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, liveBeams);

        emit(String.format(
                "stemsbeamvlinkersystem %s system %d profile %d interline %d stemThickness %d "
                        + "bounds %s sourceSeeds %d keptSeeds %d sourceBeams %d skewSlope %s "
                        + "slopeMargin %s vicinityMargin %d mainStemThickness %d halfBeamLuDx %s "
                        + "maxBeamSideDx %d maxBeamGroupDy %d maxBeamSeedDyRatio %s",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                sheet.getScale().getStemThickness(),
                rectangle(system.getBounds()),
                sourceSeeds.size(),
                keptSeeds.size(),
                sourceBeams.size(),
                hexDouble(sheet.getSkew().getSlope()),
                hexDouble(slopeMargin),
                vicinityMargin,
                mainStemThickness,
                hexDouble(halfBeamLuDx),
                maxBeamSideDx,
                maxBeamGroupDy,
                hexDouble(maxBeamSeedDyRatio)), hash, pageHash);

        totals.parts = system.getParts().size();
        for (int partOrdinal = 0; partOrdinal < system.getParts().size(); partOrdinal++) {
            final Part part = system.getParts().get(partOrdinal);
            final StringBuilder staffOrdinals = new StringBuilder();
            final StringBuilder staffBounds = new StringBuilder();
            for (Staff staff : part.getStaves()) {
                append(staffOrdinals, Integer.toString(system.getStaves().indexOf(staff)));
                append(staffBounds, rectangle(staff.getAreaBounds().getBounds()));
            }
            emit(String.format(
                    "stemsbeamvlinkerpart %s system %d ordinal %d staffOrdinals %s "
                            + "staffAreaBounds %s bounds %s",
                    page,
                    system.getId(),
                    partOrdinal,
                    emptyToken(staffOrdinals),
                    emptyToken(staffBounds),
                    rectangle(part.getAreaBounds())), hash, pageHash);
        }

        final List<BeamRecord> records = new ArrayList<>();
        for (Iterator<Inter> iterator = liveBeams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (beam.getLinker() != null) {
                throw new IllegalStateException("HEADS beam already has linker");
            }
            final List<Inter> visible = new ArrayList<>(liveBeams);
            final BeamLinker linker = new BeamLinker(beam, retriever);
            final boolean tremolo = linker.looksLikeTremolo();
            records.add(new BeamRecord(
                    beam, linker, records.size(), sigOrdinals.get(beam), visible, tremolo));
            totals.constructors++;
            if (tremolo) {
                totals.tremolos++;
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }
        }
        totals.liveBeams = liveBeams.size();

        final IdentityHashMap<BeamGroupInter, Integer> groupAliases = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, Integer> beamGlyphAliases = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, Integer> builtStumpAliases = new IdentityHashMap<>();

        // Pre-assign source aliases in constructor order so later sibling evidence cannot make a
        // future beam or group appear to precede its first constructor row.
        for (BeamRecord record : records) {
            alias(groupAliases, record.beam.getGroup());
            alias(beamGlyphAliases, record.beam.getGlyph());
        }

        for (BeamRecord record : records) {
            runBeam(
                    page,
                    sheet,
                    system,
                    retriever,
                    record,
                    sigOrdinals,
                    keptOrdinals,
                    groupAliases,
                    beamGlyphAliases,
                    builtStumpAliases,
                    slopeMargin,
                    vicinityMargin,
                    mainStemThickness,
                    halfBeamLuDx,
                    maxBeamSideDx,
                    maxBeamGroupDy,
                    maxBeamSeedDyRatio,
                    totals,
                    hash,
                    pageHash);
        }

        emit(String.format(
                "stemsbeamvlinkersystemsummary %s system %d systems 1 constructors %d survivors %d "
                        + "tremolos %d parts %d stumpBs %d orphanAttempts %d orphanCreated %d "
                        + "Bs %d Vs %d stumpVs %d initialGeometries %d rebuiltGeometries %d "
                        + "alienNeighborScans %d alienCandidates %d alienSurvivors %d "
                        + "alienLimiters %d seedChecks %d reachableSeeds %d orphanBs %d "
                        + "zeroDirectionBs %d orphanVs %d topVs %d bottomVs %d sideBLinks %d "
                        + "orphanChecks %d "
                        + "orphanExisting %d orphanEmpty %d orphanInterior %d "
                        + "partFoldRows %d finalGeometries %d geometryRows %d alienLookups %d "
                        + "alienCandidateRows %d "
                        + "alienGroupDrops %d alienBadDrops %d alienHookDrops %d alienMissDrops %d "
                        + "alienAlignedDrops %d alienAcceptedRows %d alienSortRows %d "
                        + "alienShrinkRows %d seedCandidateRows %d seedHitRows %d hash %016x",
                page,
                system.getId(),
                totals.constructors,
                totals.liveBeams,
                totals.tremolos,
                totals.parts,
                totals.stumpB,
                totals.orphanChecks - totals.orphanExisting,
                totals.orphanCreated,
                totals.bLinkers,
                totals.vLinkers,
                totals.stumpV,
                totals.vLinkers,
                totals.alienShrinks,
                totals.alienCandidates,
                totals.alienCandidates,
                totals.alienAccepted,
                totals.alienShrinks,
                totals.seedCandidates,
                totals.seedHits,
                totals.orphanB,
                totals.zeroDirectionB,
                totals.orphanV,
                totals.topV,
                totals.bottomV,
                totals.sideBLinks,
                totals.orphanChecks,
                totals.orphanExisting,
                totals.orphanEmpty,
                totals.orphanInterior,
                totals.limitStaffRows,
                totals.vLinkers,
                totals.geometries,
                totals.alienLookups,
                totals.alienCandidates,
                totals.alienGroupDrops,
                totals.alienBadDrops,
                totals.alienHookDrops,
                totals.alienMissDrops,
                totals.alienAlignedDrops,
                totals.alienAccepted,
                totals.alienSortRows,
                totals.alienShrinks,
                totals.seedCandidates,
                totals.seedHits,
                hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void runBeam (String page,
                                 Sheet sheet,
                                 SystemInfo system,
                                 StemsRetriever retriever,
                                 BeamRecord record,
                                 IdentityHashMap<Inter, Integer> sigOrdinals,
                                 IdentityHashMap<Glyph, Integer> keptOrdinals,
                                 IdentityHashMap<BeamGroupInter, Integer> groupAliases,
                                 IdentityHashMap<Glyph, Integer> beamGlyphAliases,
                                 IdentityHashMap<Glyph, Integer> builtStumpAliases,
                                 double slopeMargin,
                                 int vicinityMargin,
                                 int mainStemThickness,
                                 double halfBeamLuDx,
                                 int maxBeamSideDx,
                                 int maxBeamGroupDy,
                                 double maxBeamSeedDyRatio,
                                 Totals totals,
                                 RowHasher... hashes)
        throws Exception
    {
        final AbstractBeamInter beam = record.beam;
        final BeamLinker linker = record.linker;
        final List<Object> allB = (List<Object>) LINKER_ALL_B.get(linker);
        final Map<HorizontalSide, Object> sideB =
                (Map<HorizontalSide, Object>) LINKER_SIDE_B.get(linker);
        final List<Object> stumpV = (List<Object>) LINKER_STUMP_V.get(linker);
        final List<Glyph> stumps = (List<Glyph>) LINKER_STUMPS.get(linker);
        final Map<HorizontalSide, Glyph> sideStumps =
                (Map<HorizontalSide, Glyph>) LINKER_SIDE_STUMPS.get(linker);
        final Set<Glyph> neighbors = (Set<Glyph>) LINKER_NEIGHBOR_SEEDS.get(linker);
        final BeamGroupInter group = beam.getGroup();
        final int groupAlias = alias(groupAliases, group);
        final int beamGlyphAlias = alias(beamGlyphAliases, beam.getGlyph());
        final IdentityHashMap<Object, Integer> bOrdinals = identityObjectOrdinals(allB);

        totals.bLinkers += allB.size();
        totals.sideBLinks += sideB.size();

        emit(String.format(
                "stemsbeamvlinkerconstructor %s system %d constructor %d sigOrdinal %d live %s "
                        + "visible %s shape %s bounds %s median %s height %s topBorder %s "
                        + "bottomBorder %s profile %d grade %s good %s hook %s group group:%d "
                        + "groupMembers %s beamGlyph beamglyph:%d beamGlyphBounds %s "
                        + "beamGlyphRuns %s stumps %d sideStumpLeft %s sideStumpRight %s "
                        + "bLinkers %d sideBLeft %s sideBRight %s stumpLinkers %s tremolo %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                record.sigOrdinal,
                !record.tremolo,
                interOrdinals(record.visibleBeams, sigOrdinals),
                beam.getShape(),
                rectangle(beam.getBounds()),
                line(beam.getMedian()),
                hexDouble(beam.getHeight()),
                line(beam.getBorder(VerticalSide.TOP)),
                line(beam.getBorder(VerticalSide.BOTTOM)),
                beam.getProfile(),
                hexDouble(beam.getGrade()),
                beam.isGood(),
                beam instanceof BeamHookInter,
                groupAlias,
                interOrdinals(group.getMembers(), sigOrdinals),
                beamGlyphAlias,
                optionalRectangle(beam.getGlyph() != null ? beam.getGlyph().getBounds() : null),
                glyphRunToken(beam.getGlyph()),
                stumps.size(),
                glyphToken(sideStumps.get(HorizontalSide.LEFT), keptOrdinals, builtStumpAliases),
                glyphToken(sideStumps.get(HorizontalSide.RIGHT), keptOrdinals, builtStumpAliases),
                allB.size(),
                bToken(sideB.get(HorizontalSide.LEFT), bOrdinals),
                bToken(sideB.get(HorizontalSide.RIGHT), bOrdinals),
                vTokens(stumpV, allB),
                record.tremolo), hashes);

        for (HorizontalSide side : HorizontalSide.values()) {
            emitOrphan(
                    page,
                    system,
                    record,
                    side,
                    sideB,
                    bOrdinals,
                    sigOrdinals,
                    beamGlyphAliases,
                    totals,
                    hashes);
        }

        int vCreationOrdinal = 0;
        for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
            final Object b = allB.get(bOrdinal);
            final int id = B_ID.getInt(b);
            if (id != bOrdinal + 1) {
                throw new IllegalStateException("BLinker id does not match registration order");
            }
            final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
            final Point2D refPt = (Point2D) B_REF_PT.get(b);
            final Glyph stump = (Glyph) B_STUMP.get(b);
            final boolean anchor = B_IS_ANCHOR.getBoolean(b);
            final boolean linked = B_LINKED.getBoolean(b);
            final boolean closed = B_CLOSED.getBoolean(b);
            final Map<VerticalSide, Object> vLinkers =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
            final boolean orphan = stump == null;

            if (anchor || linked || closed) {
                throw new IllegalStateException("constructor boundary has mutated BLinker state");
            }
            if (orphan) {
                totals.orphanB++;
            } else {
                totals.stumpB++;
                if (vLinkers.isEmpty()) {
                    totals.zeroDirectionB++;
                }
            }

            emit(String.format(
                    "stemsbeamvlinkerb %s system %d beam %d ordinal %d id %d mode %s "
                            + "hSide %s stump %s stumpBounds %s stumpWeight %d stumpCenterLine %s "
                            + "stumpRuns %s ref %s anchor %s linked %s closed %s vLinkers %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    id,
                    orphan ? "orphan" : "stump",
                    optionalSide(hSide),
                    glyphToken(stump, keptOrdinals, builtStumpAliases),
                    optionalRectangle(stump != null ? stump.getBounds() : null),
                    stump != null ? stump.getWeight() : 0,
                    stump != null ? line(stump.getCenterLine()) : "-",
                    glyphRunToken(stump),
                    point(refPt),
                    anchor,
                    linked,
                    closed,
                    vMapTokens(vLinkers, bOrdinal)), hashes);

            for (Map.Entry<VerticalSide, Object> entry : vLinkers.entrySet()) {
                runVLinker(
                        page,
                        sheet,
                        system,
                        retriever,
                        record,
                        neighbors,
                        b,
                        bOrdinal,
                        orphan,
                        vCreationOrdinal++,
                        entry.getKey(),
                        entry.getValue(),
                        sigOrdinals,
                        keptOrdinals,
                        builtStumpAliases,
                        slopeMargin,
                        vicinityMargin,
                        halfBeamLuDx,
                        maxBeamSideDx,
                        maxBeamGroupDy,
                        maxBeamSeedDyRatio,
                        totals,
                        hashes);
            }
        }

        if (mainStemThickness <= 0) {
            throw new IllegalStateException("invalid mainStemThickness");
        }
    }

    private static void emitOrphan (String page,
                                    SystemInfo system,
                                    BeamRecord record,
                                    HorizontalSide side,
                                    Map<HorizontalSide, Object> sideB,
                                    IdentityHashMap<Object, Integer> bOrdinals,
                                    IdentityHashMap<Inter, Integer> sigOrdinals,
                                    IdentityHashMap<Glyph, Integer> beamGlyphAliases,
                                    Totals totals,
                                    RowHasher... hashes)
        throws Exception
    {
        totals.orphanChecks++;
        final Object actual = sideB.get(side);
        final Glyph actualStump = actual != null ? (Glyph) B_STUMP.get(actual) : null;
        Point2D end = null;
        List<AbstractBeamInter> siblings = Collections.emptyList();
        AbstractBeamInter first = null;
        AbstractBeamInter last = null;
        boolean glyphFirst = false;
        boolean glyphLast = false;
        final String action;

        if (actualStump != null) {
            action = "existing";
            totals.orphanExisting++;
        } else {
            end = side == HorizontalSide.LEFT
                    ? record.beam.getMedian().getP1() : record.beam.getMedian().getP2();
            siblings = record.linker.getSiblingBeamsAt(end);
            first = siblings.isEmpty() ? null : siblings.get(0);
            last = siblings.isEmpty() ? null : siblings.get(siblings.size() - 1);
            glyphFirst = first != null && record.beam.getGlyph() == first.getGlyph();
            glyphLast = last != null && record.beam.getGlyph() == last.getGlyph();

            if (siblings.isEmpty()) {
                action = "empty";
                totals.orphanEmpty++;
                if (actual != null) {
                    throw new IllegalStateException("orphan exists despite empty siblings");
                }
            } else if (!glyphFirst && !glyphLast) {
                action = "interior";
                totals.orphanInterior++;
                if (actual != null) {
                    throw new IllegalStateException("orphan exists for interior beam");
                }
            } else {
                action = "created";
                totals.orphanCreated++;
                if (actual == null || actualStump != null) {
                    throw new IllegalStateException("missing expected orphan BLinker");
                }
            }
        }

        emit(String.format(
                "stemsbeamvlinkerorphan %s system %d beam %d side %s end %s siblings %s "
                        + "first %s last %s selfGlyph beamglyph:%d firstGlyph %s lastGlyph %s "
                        + "glyphFirst %s glyphLast %s action %s sideB %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                side,
                optionalPoint(end),
                abstractBeamOrdinals(siblings, sigOrdinals),
                optionalInter(first, sigOrdinals),
                optionalInter(last, sigOrdinals),
                alias(beamGlyphAliases, record.beam.getGlyph()),
                first != null ? "beamglyph:" + alias(beamGlyphAliases, first.getGlyph()) : "-",
                last != null ? "beamglyph:" + alias(beamGlyphAliases, last.getGlyph()) : "-",
                actualStump != null ? "-" : Boolean.toString(glyphFirst),
                actualStump != null ? "-" : Boolean.toString(glyphLast),
                action,
                bToken(actual, bOrdinals)), hashes);
    }

    private static void runVLinker (String page,
                                    Sheet sheet,
                                    SystemInfo system,
                                    StemsRetriever retriever,
                                    BeamRecord record,
                                    Set<Glyph> neighbors,
                                    Object b,
                                    int bOrdinal,
                                    boolean orphan,
                                    int creationOrdinal,
                                    VerticalSide mapSide,
                                    Object v,
                                    IdentityHashMap<Inter, Integer> sigOrdinals,
                                    IdentityHashMap<Glyph, Integer> keptOrdinals,
                                    IdentityHashMap<Glyph, Integer> builtStumpAliases,
                                    double slopeMargin,
                                    int vicinityMargin,
                                    double halfBeamLuDx,
                                    int maxBeamSideDx,
                                    int maxBeamGroupDy,
                                    double maxBeamSeedDyRatio,
                                    Totals totals,
                                    RowHasher... hashes)
        throws Exception
    {
        final AbstractBeamInter beam = record.beam;
        final int bId = B_ID.getInt(b);
        final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
        final Point2D refPt = (Point2D) B_REF_PT.get(b);
        final Glyph stump = (Glyph) B_STUMP.get(b);
        final VerticalSide vSide = (VerticalSide) V_V_SIDE.get(v);
        final int yDir = V_Y_DIR.getInt(v);
        final HorizontalSide stopping = (HorizontalSide) V_STOPPING_HEAD_SIDE.get(v);
        final Area actualArea = (Area) V_LU_AREA.get(v);
        final Line2D actualTheo = (Line2D) V_THEO_LINE.get(v);
        final Set<Glyph> actualSeeds = (Set<Glyph>) V_SEEDS.get(v);

        if (mapSide != vSide || yDir != vSide.direction()
                || stopping != (yDir < 0 ? HorizontalSide.LEFT : HorizontalSide.RIGHT)
                || V_STEM_BUILDER.get(v) != null || actualArea == null || actualTheo == null
                || actualSeeds == null) {
            throw new IllegalStateException("invalid constructor VLinker state");
        }

        totals.vLinkers++;
        if (orphan) {
            totals.orphanV++;
        } else {
            totals.stumpV++;
        }
        if (vSide == VerticalSide.TOP) {
            totals.topV++;
        } else {
            totals.bottomV++;
        }

        emit(String.format(
                "stemsbeamvlinkerv %s system %d beam %d bOrdinal %d bId %d creation %d "
                        + "mode %s hSide %s stump %s vSide %s yDir %d stoppingHeadSide %s "
                        + "ref %s seedCount %d",
                page,
                system.getId(),
                record.constructorOrdinal,
                bOrdinal,
                bId,
                creationOrdinal,
                orphan ? "orphan" : "stump",
                optionalSide(hSide),
                glyphToken(stump, keptOrdinals, builtStumpAliases),
                vSide,
                yDir,
                stopping,
                point(refPt),
                actualSeeds.size()), hashes);

        final double initialYLimit = emitInitialLimit(
                page, system, record, bOrdinal, vSide, refPt, totals, hashes);
        final Geometry initial = geometry(
                beam,
                system,
                refPt,
                vSide,
                initialYLimit,
                slopeMargin,
                halfBeamLuDx,
                maxBeamSeedDyRatio);

        final AlienReplay alienReplay = replayAliens(
                page,
                system,
                record,
                bOrdinal,
                hSide,
                vSide,
                refPt,
                initial.theo,
                vicinityMargin,
                maxBeamSideDx,
                maxBeamGroupDy,
                sigOrdinals,
                totals,
                hashes);
        final double finalYLimit = alienReplay.limit != null
                ? LineUtil.yAtX(alienReplay.limit, refPt.getX()) : initialYLimit;
        final Geometry fin = geometry(
                beam,
                system,
                refPt,
                vSide,
                finalYLimit,
                slopeMargin,
                halfBeamLuDx,
                maxBeamSeedDyRatio);

        emitGeometry(
                page, system, record, bOrdinal, vSide, "initial", initial, totals, hashes);
        emitGeometry(page, system, record, bOrdinal, vSide, "final", fin, totals, hashes);

        if (!sameLineBits(actualTheo, fin.theo) || !actualArea.equals(fin.area)
                || !actualArea.getBounds().equals(fin.area.getBounds())
                || !sameRectangle2DBits(actualArea.getBounds2D(), fin.area.getBounds2D())) {
            throw new IllegalStateException("replayed VLinker geometry differs from constructor");
        }

        final Set<Glyph> expectedSeeds = Glyphs.intersectedGlyphs(neighbors, actualArea);
        if (!sameIdentities(actualSeeds, expectedSeeds)) {
            throw new IllegalStateException("replayed VLinker seed lookup differs");
        }
        int candidateOrdinal = 0;
        for (Glyph glyph : neighbors) {
            final boolean hit = actualArea.intersects(glyph.getBounds());
            totals.seedCandidates++;
            emit(String.format(
                    "stemsbeamvlinkerseedcandidate %s system %d beam %d bOrdinal %d vSide %s "
                            + "ordinal %d glyph %s bounds %s hit %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    vSide,
                    candidateOrdinal++,
                    glyphToken(glyph, keptOrdinals, builtStumpAliases),
                    rectangle(glyph.getBounds()),
                    hit), hashes);
        }
        int hitOrdinal = 0;
        for (Glyph glyph : actualSeeds) {
            totals.seedHits++;
            emit(String.format(
                    "stemsbeamvlinkerseed %s system %d beam %d bOrdinal %d vSide %s "
                            + "ordinal %d glyph %s bounds %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    vSide,
                    hitOrdinal++,
                    glyphToken(glyph, keptOrdinals, builtStumpAliases),
                    rectangle(glyph.getBounds())), hashes);
        }
    }

    private static double emitInitialLimit (String page,
                                            SystemInfo system,
                                            BeamRecord record,
                                            int bOrdinal,
                                            VerticalSide vSide,
                                            Point2D refPt,
                                            Totals totals,
                                            RowHasher... hashes)
    {
        final int yDir = vSide.direction();
        final Rectangle systemBox = system.getBounds();
        double yLimit = yDir < 0 ? systemBox.getMaxY() : systemBox.getMinY();
        final Point center = record.beam.getCenter();
        final List<Staff> around = system.getStavesAround(center);

        emit(String.format(
                "stemsbeamvlinkerlimit %s system %d beam %d bOrdinal %d vSide %s "
                        + "beamCenter %d:%d ref %s systemBounds %s systemYLimit %s around %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                bOrdinal,
                vSide,
                center.x,
                center.y,
                point(refPt),
                rectangle(systemBox),
                hexDouble(yLimit),
                staffOrdinals(around, system)), hashes);

        for (int ordinal = 0; ordinal < around.size(); ordinal++) {
            final Staff staff = around.get(ordinal);
            final int staffOrdinal = system.getStaves().indexOf(staff);
            final int partOrdinal = system.getParts().indexOf(staff.getPart());
            final Rectangle partBox = staff.getPart().getAreaBounds();
            final double before = yLimit;
            final double candidate = yDir > 0
                    ? partBox.y + partBox.height - 1 : partBox.y;
            yLimit = yDir > 0 ? Math.max(yLimit, candidate) : Math.min(yLimit, candidate);
            totals.limitStaffRows++;
            emit(String.format(
                    "stemsbeamvlinkerlimitstaff %s system %d beam %d bOrdinal %d vSide %s "
                            + "ordinal %d staffOrdinal %d partOrdinal %d staffBounds %s "
                            + "partBounds %s before %s candidate %s after %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    vSide,
                    ordinal,
                    staffOrdinal,
                    partOrdinal,
                    rectangle(staff.getAreaBounds().getBounds()),
                    rectangle(partBox),
                    hexDouble(before),
                    hexDouble(candidate),
                    hexDouble(yLimit)), hashes);
        }
        return yLimit;
    }

    private static AlienReplay replayAliens (String page,
                                             SystemInfo system,
                                             BeamRecord record,
                                             int bOrdinal,
                                             HorizontalSide hSide,
                                             VerticalSide vSide,
                                             Point2D refPt,
                                             Line2D initialTheo,
                                             int vicinityMargin,
                                             int maxBeamSideDx,
                                             int maxBeamGroupDy,
                                             IdentityHashMap<Inter, Integer> sigOrdinals,
                                             Totals totals,
                                             RowHasher... hashes)
    {
        final Rectangle systemBox = system.getBounds();
        final Rectangle beamBox = record.beam.getBounds();
        final Rectangle fatBox = new Rectangle(
                beamBox.x, systemBox.y, beamBox.width, systemBox.height);
        fatBox.grow(vicinityMargin, 0);
        final List<Inter> neighbors = Inters.intersectedInters(
                record.visibleBeams, GeoOrder.BY_ABSCISSA, fatBox);
        totals.alienLookups++;

        emit(String.format(
                "stemsbeamvlinkeralienlookup %s system %d beam %d bOrdinal %d vSide %s "
                        + "visible %s beamBox %s systemBox %s fatBox %s neighbors %s "
                        + "groupMembers %s initialTheo %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                bOrdinal,
                vSide,
                interOrdinals(record.visibleBeams, sigOrdinals),
                rectangle(beamBox),
                rectangle(systemBox),
                rectangle(fatBox),
                interOrdinals(neighbors, sigOrdinals),
                interOrdinals(record.beam.getGroup().getMembers(), sigOrdinals),
                line(initialTheo)), hashes);

        final List<Inter> accepted = new ArrayList<>();
        for (int ordinal = 0; ordinal < neighbors.size(); ordinal++) {
            final AbstractBeamInter alien = (AbstractBeamInter) neighbors.get(ordinal);
            totals.alienCandidates++;
            final boolean sameGroup = record.beam.getGroup().getMembers().contains(alien);
            final boolean good = alien.isGood();
            final boolean hook = alien instanceof BeamHookInter;
            boolean intersects = false;
            Point2D cross = null;
            double dy = Double.NaN;
            double endpointX = Double.NaN;
            double dx = Double.NaN;
            boolean aligned = false;
            final String action;

            if (sameGroup) {
                totals.alienGroupDrops++;
                action = "group";
            } else if (!good) {
                totals.alienBadDrops++;
                action = "bad";
            } else if (hook) {
                totals.alienHookDrops++;
                action = "hook";
            } else {
                intersects = alien.getMedian().intersectsLine(initialTheo);
                if (!intersects) {
                    totals.alienMissDrops++;
                    action = "miss";
                } else {
                    cross = LineUtil.intersection(initialTheo, alien.getMedian());
                    dy = Math.abs(cross.getY() - refPt.getY());
                    endpointX = hSide == HorizontalSide.LEFT
                            ? alien.getMedian().getX1() : alien.getMedian().getX2();
                    dx = Math.abs(cross.getX() - endpointX);
                    aligned = dy <= maxBeamGroupDy && dx < maxBeamSideDx;
                    if (aligned) {
                        totals.alienAlignedDrops++;
                        action = "aligned";
                    } else {
                        totals.alienAccepted++;
                        accepted.add(alien);
                        action = "accept";
                    }
                }
            }

            emit(String.format(
                    "stemsbeamvlinkeralien %s system %d beam %d bOrdinal %d vSide %s "
                            + "ordinal %d sigOrdinal %d sameGroup %s grade %s good %s hook %s "
                            + "bounds %s median %s intersects %s cross %s dy %s endpoint %s "
                            + "dx %s maxGroupDy %d maxSideDx %d aligned %s action %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    vSide,
                    ordinal,
                    sigOrdinals.get(alien),
                    sameGroup,
                    hexDouble(alien.getGrade()),
                    good,
                    hook,
                    rectangle(alien.getBounds()),
                    line(alien.getMedian()),
                    intersects,
                    optionalPoint(cross),
                    finiteDouble(dy),
                    finiteDouble(endpointX),
                    finiteDouble(dx),
                    maxBeamGroupDy,
                    maxBeamSideDx,
                    aligned,
                    action), hashes);
        }

        StemsRetriever.sortBeamsFromRef(refPt, vSide.direction(), accepted);
        final double skewSlope = system.getSheet().getSkew().getSlope();
        for (int ordinal = 0; ordinal < accepted.size(); ordinal++) {
            final AbstractBeamInter alien = (AbstractBeamInter) accepted.get(ordinal);
            final Line2D outgoing = alien.getBorder(vSide);
            final Point2D target = StemsRetriever.getTargetPt(refPt, outgoing, skewSlope);
            final double key = vSide.direction() * (target.getY() - refPt.getY());
            totals.alienSortRows++;
            emit(String.format(
                    "stemsbeamvlinkeraliensort %s system %d beam %d bOrdinal %d vSide %s "
                            + "ordinal %d sigOrdinal %d outgoingBorder %s target %s key %s",
                    page,
                    system.getId(),
                    record.constructorOrdinal,
                    bOrdinal,
                    vSide,
                    ordinal,
                    sigOrdinals.get(alien),
                    line(outgoing),
                    point(target),
                    hexDouble(key)), hashes);
        }

        final AbstractBeamInter selected = accepted.isEmpty()
                ? null : (AbstractBeamInter) accepted.get(0);
        final Line2D limit = selected != null ? selected.getBorder(vSide.opposite()) : null;
        if (selected != null) {
            totals.alienShrinks++;
        }
        emit(String.format(
                "stemsbeamvlinkeralienselected %s system %d beam %d bOrdinal %d vSide %s "
                        + "selected %s limitSide %s limit %s refinedYLimit %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                bOrdinal,
                vSide,
                optionalInter(selected, sigOrdinals),
                selected != null ? vSide.opposite().toString() : "-",
                limit != null ? line(limit) : "-",
                limit != null ? hexDouble(LineUtil.yAtX(limit, refPt.getX())) : "-"), hashes);

        return new AlienReplay(limit);
    }

    private static Geometry geometry (AbstractBeamInter beam,
                                      SystemInfo system,
                                      Point2D refPt,
                                      VerticalSide vSide,
                                      double yLimit,
                                      double slopeMargin,
                                      double halfBeamLuDx,
                                      double maxBeamSeedDyRatio)
    {
        final int yDir = vSide.direction();
        final double skewSlope = system.getSheet().getSkew().getSlope();
        final double luSlope = -skewSlope;
        final double dSlope = yDir * slopeMargin;
        final double xRef = refPt.getX();
        final Line2D border = beam.getBorder(vSide);
        final Point2D pl = LineUtil.intersectionAtX(border, xRef - halfBeamLuDx);
        final Point2D pr = LineUtil.intersectionAtX(border, xRef + halfBeamLuDx);
        final int profile = Math.max(beam.getProfile(), system.getProfile());
        final int yGapPixels = system.getSheet().getScale().toPixels(
                BeamStemRelation.getYGapMaximum(profile));
        final double yOffset = yDir * maxBeamSeedDyRatio * yGapPixels;
        final double dy = yLimit - refPt.getY();
        final Point2D q0 = new Point2D.Double(pl.getX(), pl.getY() + yOffset);
        final Point2D q1 = new Point2D.Double(pr.getX(), pr.getY() + yOffset);
        final Point2D q2 = new Point2D.Double(
                pr.getX() + ((luSlope + dSlope) * dy), yLimit);
        final Point2D q3 = new Point2D.Double(
                pl.getX() + ((luSlope - dSlope) * dy), yLimit);
        final Path2D path = new Path2D.Double();
        path.moveTo(q0.getX(), q0.getY());
        path.lineTo(q1.getX(), q1.getY());
        path.lineTo(q2.getX(), q2.getY());
        path.lineTo(q3.getX(), q3.getY());
        path.closePath();
        final Area area = new Area(path);
        final Point2D target = StemsRetriever.getTargetPt(
                refPt, new Line2D.Double(0, yLimit, 100, yLimit), skewSlope);
        final Line2D theo = new Line2D.Double(refPt, target);
        return new Geometry(
                border,
                pl,
                pr,
                profile,
                yGapPixels,
                yOffset,
                skewSlope,
                luSlope,
                dSlope,
                yLimit,
                dy,
                q0,
                q1,
                q2,
                q3,
                area,
                theo);
    }

    private static void emitGeometry (String page,
                                      SystemInfo system,
                                      BeamRecord record,
                                      int bOrdinal,
                                      VerticalSide vSide,
                                      String phase,
                                      Geometry g,
                                      Totals totals,
                                      RowHasher... hashes)
    {
        totals.geometries++;
        emit(String.format(
                "stemsbeamvlinkergeometry %s system %d beam %d bOrdinal %d vSide %s phase %s "
                        + "border %s pl %s pr %s profile %d yGapPixels %d yOffset %s "
                        + "skewSlope %s luSlope %s dSlope %s yLimit %s dy %s quad %s "
                        + "bounds %s bounds2d %s theo %s",
                page,
                system.getId(),
                record.constructorOrdinal,
                bOrdinal,
                vSide,
                phase,
                line(g.border),
                point(g.pl),
                point(g.pr),
                g.profile,
                g.yGapPixels,
                hexDouble(g.yOffset),
                hexDouble(g.skewSlope),
                hexDouble(g.luSlope),
                hexDouble(g.dSlope),
                hexDouble(g.yLimit),
                hexDouble(g.dy),
                point(g.q0) + ":" + point(g.q1) + ":" + point(g.q2) + ":" + point(g.q3),
                rectangle(g.area.getBounds()),
                rectangle2D(g.area.getBounds2D()),
                line(g.theo)), hashes);
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
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS beam-VLinker oracle.");
        System.out.println("#");
        System.out.println("# Every page starts in a fresh Epsilon-GC JVM and reaches real HEADS.");
        System.out.println("# The probe executes the stable-x BeamLinker constructor loop, including");
        System.out.println("# equipStumps and equipOrphanSides, then stops before any HeadLinker or");
        System.out.println("# inspectVLinkers call. No anchor or StemBuilder exists at this boundary.");
        System.out.println("#");
        System.out.println("# Rows freeze exact B/V creation order and side aliases, orphan eligibility,");
        System.out.println("# system/part y limits, initial and final raw lookup quadrilaterals, theoretical");
        System.out.println("# lines, every closer-alien filter/sort/selection, and final seed reachability.");
        System.out.println("# Doubles include hex text/raw bits. Ordinals and aliases are deterministic");
        System.out.println("# per-system boundary handles, never Java IDs or identity hash codes.");
    }

    private static Field declaredField (Class<?> owner,
                                        String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static IdentityHashMap<Glyph, Integer> identityOrdinals (List<Glyph> glyphs)
    {
        final IdentityHashMap<Glyph, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < glyphs.size(); ordinal++) {
            if (ordinals.put(glyphs.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate glyph identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static IdentityHashMap<Inter, Integer> interOrdinals (List<? extends Inter> inters)
    {
        final IdentityHashMap<Inter, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < inters.size(); ordinal++) {
            if (ordinals.put(inters.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate inter identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static IdentityHashMap<Object, Integer> identityObjectOrdinals (List<Object> values)
    {
        final IdentityHashMap<Object, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (ordinals.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate object identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static <T> int alias (IdentityHashMap<T, Integer> aliases,
                                  T value)
    {
        if (value == null) {
            return -1;
        }
        final Integer old = aliases.get(value);
        if (old != null) {
            return old;
        }
        final int next = aliases.size();
        aliases.put(value, next);
        return next;
    }

    private static String glyphToken (Glyph glyph,
                                      IdentityHashMap<Glyph, Integer> keptOrdinals,
                                      IdentityHashMap<Glyph, Integer> builtAliases)
    {
        if (glyph == null) {
            return "-";
        }
        final Integer kept = keptOrdinals.get(glyph);
        return kept != null ? "kept:" + kept : "built:" + alias(builtAliases, glyph);
    }

    private static String bToken (Object b,
                                  IdentityHashMap<Object, Integer> ordinals)
    {
        return b != null ? "b:" + ordinals.get(b) : "-";
    }

    private static String vMapTokens (Map<VerticalSide, Object> values,
                                      int bOrdinal)
    {
        final StringBuilder builder = new StringBuilder();
        for (VerticalSide side : values.keySet()) {
            append(builder, "b:" + bOrdinal + ":" + side);
        }
        return emptyToken(builder);
    }

    private static String vTokens (List<Object> stumpV,
                                   List<Object> allB)
        throws IllegalAccessException
    {
        final IdentityHashMap<Object, String> tokens = new IdentityHashMap<>();
        for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
            final Map<VerticalSide, Object> map =
                    (Map<VerticalSide, Object>) B_V_LINKERS.get(allB.get(bOrdinal));
            for (Map.Entry<VerticalSide, Object> entry : map.entrySet()) {
                tokens.put(entry.getValue(), "b:" + bOrdinal + ":" + entry.getKey());
            }
        }
        final StringBuilder builder = new StringBuilder();
        for (Object v : stumpV) {
            final String token = tokens.get(v);
            if (token == null) {
                throw new IllegalStateException("stump VLinker absent from B maps");
            }
            append(builder, token);
        }
        return emptyToken(builder);
    }

    private static String interOrdinals (List<? extends Inter> inters,
                                         IdentityHashMap<Inter, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Inter inter : inters) {
            final Integer ordinal = ordinals.get(inter);
            if (ordinal == null) {
                throw new IllegalStateException("inter is absent from source pool");
            }
            append(builder, Integer.toString(ordinal));
        }
        return emptyToken(builder);
    }

    private static String abstractBeamOrdinals (List<AbstractBeamInter> inters,
                                                IdentityHashMap<Inter, Integer> ordinals)
    {
        return interOrdinals(inters, ordinals);
    }

    private static String optionalInter (Inter inter,
                                         IdentityHashMap<Inter, Integer> ordinals)
    {
        return inter != null ? Integer.toString(ordinals.get(inter)) : "-";
    }

    private static String staffOrdinals (List<Staff> staves,
                                         SystemInfo system)
    {
        final StringBuilder builder = new StringBuilder();
        for (Staff staff : staves) {
            append(builder, Integer.toString(system.getStaves().indexOf(staff)));
        }
        return emptyToken(builder);
    }

    private static void append (StringBuilder builder,
                                String token)
    {
        if (builder.length() != 0) {
            builder.append(',');
        }
        builder.append(token);
    }

    private static String emptyToken (StringBuilder builder)
    {
        return builder.length() == 0 ? "-" : builder.toString();
    }

    private static boolean sameIdentities (Set<Glyph> left,
                                           Set<Glyph> right)
    {
        if (left.size() != right.size()) {
            return false;
        }
        final Iterator<Glyph> leftIt = left.iterator();
        final Iterator<Glyph> rightIt = right.iterator();
        while (leftIt.hasNext()) {
            if (leftIt.next() != rightIt.next()) {
                return false;
            }
        }
        return true;
    }

    private static boolean sameLineBits (Line2D left,
                                         Line2D right)
    {
        return Double.doubleToLongBits(left.getX1()) == Double.doubleToLongBits(right.getX1())
                && Double.doubleToLongBits(left.getY1()) == Double.doubleToLongBits(right.getY1())
                && Double.doubleToLongBits(left.getX2()) == Double.doubleToLongBits(right.getX2())
                && Double.doubleToLongBits(left.getY2()) == Double.doubleToLongBits(right.getY2());
    }

    private static boolean sameRectangle2DBits (java.awt.geom.Rectangle2D left,
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
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String optionalRectangle (Rectangle box)
    {
        return box != null ? rectangle(box) : "-";
    }

    private static String rectangle2D (java.awt.geom.Rectangle2D box)
    {
        return hexDouble(box.getX()) + ":" + hexDouble(box.getY()) + ":"
                + hexDouble(box.getWidth()) + ":" + hexDouble(box.getHeight());
    }

    private static String line (Line2D line)
    {
        return line != null ? point(line.getP1()) + ":" + point(line.getP2()) : "-";
    }

    private static String point (Point2D point)
    {
        return hexDouble(point.getX()) + ":" + hexDouble(point.getY());
    }

    private static String optionalPoint (Point2D point)
    {
        return point != null ? point(point) : "-";
    }

    private static String optionalSide (HorizontalSide side)
    {
        return side != null ? side.toString() : "-";
    }

    private static String finiteDouble (double value)
    {
        return Double.isFinite(value) ? hexDouble(value) : "-";
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static String glyphRunToken (Glyph glyph)
    {
        if (glyph == null) {
            return "-";
        }
        int count = 0;
        final RowHasher hash = new RowHasher();
        final var table = glyph.getRunTable();
        hash.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<Run> iterator = table.iterator(sequence); iterator.hasNext();) {
                final Run run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
                count++;
            }
            hash.add(row.toString());
        }
        return count + ":" + String.format("%016x", hash.value());
    }

    private static void emit (String row,
                              RowHasher... hashes)
    {
        System.out.println(row);
        for (RowHasher hash : hashes) {
            hash.add(row);
        }
    }

    private record BeamRecord(
            AbstractBeamInter beam,
            BeamLinker linker,
            int constructorOrdinal,
            int sigOrdinal,
            List<Inter> visibleBeams,
            boolean tremolo)
    {
    }

    private record AlienReplay(Line2D limit)
    {
    }

    private record Geometry(
            Line2D border,
            Point2D pl,
            Point2D pr,
            int profile,
            int yGapPixels,
            double yOffset,
            double skewSlope,
            double luSlope,
            double dSlope,
            double yLimit,
            double dy,
            Point2D q0,
            Point2D q1,
            Point2D q2,
            Point2D q3,
            Area area,
            Line2D theo)
    {
    }

    private static final class Totals
    {
        long constructors;
        long tremolos;
        long parts;
        long liveBeams;
        long bLinkers;
        long stumpB;
        long orphanB;
        long zeroDirectionB;
        long vLinkers;
        long stumpV;
        long orphanV;
        long topV;
        long bottomV;
        long sideBLinks;
        long orphanChecks;
        long orphanExisting;
        long orphanCreated;
        long orphanEmpty;
        long orphanInterior;
        long limitStaffRows;
        long geometries;
        long alienLookups;
        long alienCandidates;
        long alienGroupDrops;
        long alienBadDrops;
        long alienHookDrops;
        long alienMissDrops;
        long alienAlignedDrops;
        long alienAccepted;
        long alienSortRows;
        long alienShrinks;
        long seedCandidates;
        long seedHits;

        void include (Totals that)
        {
            constructors += that.constructors;
            tremolos += that.tremolos;
            parts += that.parts;
            liveBeams += that.liveBeams;
            bLinkers += that.bLinkers;
            stumpB += that.stumpB;
            orphanB += that.orphanB;
            zeroDirectionB += that.zeroDirectionB;
            vLinkers += that.vLinkers;
            stumpV += that.stumpV;
            orphanV += that.orphanV;
            topV += that.topV;
            bottomV += that.bottomV;
            sideBLinks += that.sideBLinks;
            orphanChecks += that.orphanChecks;
            orphanExisting += that.orphanExisting;
            orphanCreated += that.orphanCreated;
            orphanEmpty += that.orphanEmpty;
            orphanInterior += that.orphanInterior;
            limitStaffRows += that.limitStaffRows;
            geometries += that.geometries;
            alienLookups += that.alienLookups;
            alienCandidates += that.alienCandidates;
            alienGroupDrops += that.alienGroupDrops;
            alienBadDrops += that.alienBadDrops;
            alienHookDrops += that.alienHookDrops;
            alienMissDrops += that.alienMissDrops;
            alienAlignedDrops += that.alienAlignedDrops;
            alienAccepted += that.alienAccepted;
            alienSortRows += that.alienSortRows;
            alienShrinks += that.alienShrinks;
            seedCandidates += that.seedCandidates;
            seedHits += that.seedHits;
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            final byte[] bytes = (row + "\n").getBytes(StandardCharsets.UTF_8);
            for (byte b : bytes) {
                value ^= b & 0xffL;
                value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }
}
