// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.EnumMap;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.dynamic.SectionCompound;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.lag.Sections;
import org.audiveris.omr.math.GeoUtil;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.TremoloInter;
import org.audiveris.omr.sig.relation.BeamPortion;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact, identity-free oracle for the mutating {@link BeamLinker} constructor prefix.
 *
 * <p>The probe reaches the real HEADS boundary, installs the same retriever parameters and purged
 * system seed list as {@code StemsRetriever.inspectStems}, and then constructs beam linkers in the
 * real stable-x order. It independently replays seed lookup, seed purging, side classification,
 * side-section compounds, stump-direction tests, registration, and tremolo classification.
 */
@SuppressWarnings("unchecked")
public class StemsBeamStumpProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;

    private static final Field RETRIEVER_SYSTEM_SEEDS;

    private static final Field RETRIEVER_SYSTEM_BEAMS;

    private static final Method PURGE_NO_STEM_SEEDS;

    private static final Field PARAM_MAX_STEM_THICKNESS;

    private static final Field PARAM_MAX_BEAM_SEED_DX;

    private static final Field PARAM_MAX_BEAM_SEED_DY_RATIO;

    private static final Field PARAM_MIN_BEAM_STEMS_DX;

    private static final Field PARAM_MIN_BEAM_STUMP_DY;

    private static final Field LINKER_NEIGHBOR_SEEDS;

    private static final Field LINKER_STUMPS;

    private static final Field LINKER_SIDE_STUMPS;

    private static final Method GET_SEED_AREA;

    private static final Method GET_STUMP_AREA;

    private static final Method GET_STUMP_DIRECTIONS;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);
            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = declaredField(StemsRetriever.class, "systemBeams");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            PARAM_MAX_STEM_THICKNESS = declaredField(parameters, "maxStemThickness");
            PARAM_MAX_BEAM_SEED_DX = declaredField(parameters, "maxBeamSeedDx");
            PARAM_MAX_BEAM_SEED_DY_RATIO = declaredField(parameters, "maxBeamSeedDyRatio");
            PARAM_MIN_BEAM_STEMS_DX = declaredField(parameters, "minBeamStemsDx");
            PARAM_MIN_BEAM_STUMP_DY = declaredField(parameters, "minBeamStumpDy");
            LINKER_NEIGHBOR_SEEDS = declaredField(BeamLinker.class, "neighborSeeds");
            LINKER_STUMPS = declaredField(BeamLinker.class, "stumps");
            LINKER_SIDE_STUMPS = declaredField(BeamLinker.class, "sideStumps");
            GET_SEED_AREA = BeamLinker.class.getDeclaredMethod("getSeedArea");
            GET_SEED_AREA.setAccessible(true);
            GET_STUMP_AREA = BeamLinker.class.getDeclaredMethod(
                    "getStumpArea", HorizontalSide.class);
            GET_STUMP_AREA.setAccessible(true);
            GET_STUMP_DIRECTIONS = BeamLinker.class.getDeclaredMethod(
                    "getStumpDirections", Glyph.class);
            GET_STUMP_DIRECTIONS.setAccessible(true);
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamStumpProbe ()
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

        final String[] parts = args[0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        runPage(Paths.get(parts[0]).toAbsolutePath(), Integer.parseInt(parts[1]));
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
                "stemsbeamstumppage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, totals, hash);
        }
        System.out.printf(
                "stemsbeamstumppagesummary %s systems %d constructors %d sides %d neighbors %d "
                        + "seedInputs %d purgeComparisons %d purgeRemovals %d purgeBreaks %d "
                        + "sideSeeds %d buildAttempts %d emptySections %d zeroCompounds %d "
                        + "candidates %d directionAccepted %d directionRejected %d registrations %d "
                        + "newBuilds %d reusedBuilds %d sectionRows %d steps %d finalStumps %d "
                        + "finalSideStumps %d tremolos %d hash %016x%n",
                page,
                sheet.getSystems().size(),
                totals.constructors,
                totals.sides,
                totals.neighbors,
                totals.seedInputs,
                totals.purgeComparisons,
                totals.purgeRemovals,
                totals.purgeBreaks,
                totals.sideSeeds,
                totals.buildAttempts,
                totals.emptySections,
                totals.zeroCompounds,
                totals.candidates,
                totals.directionAccepted,
                totals.directionRejected,
                totals.registrations,
                totals.newBuilds,
                totals.reusedBuilds,
                totals.sectionRows,
                totals.steps,
                totals.finalStumps,
                totals.finalSideStumps,
                totals.tremolos,
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
        final int maxStemThickness = PARAM_MAX_STEM_THICKNESS.getInt(params);
        final double maxBeamSeedDx = PARAM_MAX_BEAM_SEED_DX.getDouble(params);
        final double maxBeamSeedDyRatio = PARAM_MAX_BEAM_SEED_DY_RATIO.getDouble(params);
        final int minBeamStemsDx = PARAM_MIN_BEAM_STEMS_DX.getInt(params);
        final int minBeamStumpDy = PARAM_MIN_BEAM_STUMP_DY.getInt(params);

        final List<Glyph> sourceSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        final List<Glyph> keptSeeds = new ArrayList<>(sourceSeeds);
        PURGE_NO_STEM_SEEDS.invoke(retriever, keptSeeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, keptSeeds);
        final IdentityHashMap<Glyph, Integer> keptOrdinals = identityOrdinals(keptSeeds);
        final List<Section> verticalSections = new ArrayList<>(system.getVerticalSections());
        final IdentityHashMap<Section, Integer> sectionOrdinals = sectionOrdinals(verticalSections);
        final RegistryTracker registry = new RegistryTracker(sheet.getGlyphIndex());
        final IdentityHashMap<Glyph, Integer> aliases = new IdentityHashMap<>();

        final List<Inter> beams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> sigOrdinals = interOrdinals(beams);
        Collections.sort(beams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, beams);

        emit(String.format(
                "stemsbeamstumpsystem %s system %d profile %d interline %d stemThickness %d "
                        + "maxStemThickness %d bounds %s sourceSeeds %d keptSeeds %d "
                        + "verticalSections %d preGlyphs %d beams %d maxBeamSeedDx %s "
                        + "maxBeamSeedDyRatio %s minBeamStemsDx %d minBeamStumpDy %d",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                sheet.getScale().getStemThickness(),
                maxStemThickness,
                rectangle(system.getBounds()),
                sourceSeeds.size(),
                keptSeeds.size(),
                verticalSections.size(),
                registry.preCount(),
                beams.size(),
                hexDouble(maxBeamSeedDx),
                hexDouble(maxBeamSeedDyRatio),
                minBeamStemsDx,
                minBeamStumpDy), hash, pageHash);

        for (int beamOrdinal = 0; beamOrdinal < beams.size(); beamOrdinal++) {
            final AbstractBeamInter beam = (AbstractBeamInter) beams.get(beamOrdinal);
            final int registrationFloor = registry.nextRegistrationOrdinal();
            final BeamLinker linker = new BeamLinker(beam, retriever);
            final List<Registration> registrations = registry.capture("beam:" + beamOrdinal);
            final boolean tremolo = linker.looksLikeTremolo();
            final boolean directTremolo = ((List<Glyph>) LINKER_STUMPS.get(linker)).size() == 1
                    && ((Map<HorizontalSide, Glyph>) LINKER_SIDE_STUMPS.get(linker)).isEmpty()
                    && TremoloInter.isTremoloWidth(
                            beam.getMedian().getX2() - beam.getMedian().getX1(), sheet.getScale());
            if (tremolo != directTremolo) {
                throw new IllegalStateException("tremolo replay differs");
            }
            runBeam(
                    page,
                    sheet,
                    system,
                    beam,
                    beamOrdinal,
                    sigOrdinals,
                    linker,
                    keptOrdinals,
                    verticalSections,
                    sectionOrdinals,
                    registry,
                    registrations,
                    registrationFloor,
                    aliases,
                    maxStemThickness,
                    maxBeamSeedDx,
                    maxBeamSeedDyRatio,
                    minBeamStemsDx,
                    minBeamStumpDy,
                    tremolo,
                    totals,
                    hash,
                    pageHash);
        }

        emit(String.format(
                "stemsbeamstumpsystemsummary %s system %d constructors %d sides %d "
                        + "neighbors %d seedInputs %d purgeComparisons %d purgeRemovals %d "
                        + "purgeBreaks %d sideSeeds %d buildAttempts %d emptySections %d "
                        + "zeroCompounds %d candidates %d directionAccepted %d "
                        + "directionRejected %d registrations %d newBuilds %d reusedBuilds %d "
                        + "sectionRows %d steps %d finalStumps %d finalSideStumps %d tremolos %d "
                        + "hash %016x",
                page,
                system.getId(),
                totals.constructors,
                totals.sides,
                totals.neighbors,
                totals.seedInputs,
                totals.purgeComparisons,
                totals.purgeRemovals,
                totals.purgeBreaks,
                totals.sideSeeds,
                totals.buildAttempts,
                totals.emptySections,
                totals.zeroCompounds,
                totals.candidates,
                totals.directionAccepted,
                totals.directionRejected,
                totals.registrations,
                totals.newBuilds,
                totals.reusedBuilds,
                totals.sectionRows,
                totals.steps,
                totals.finalStumps,
                totals.finalSideStumps,
                totals.tremolos,
                hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void runBeam (String page,
                                 Sheet sheet,
                                 SystemInfo system,
                                 AbstractBeamInter beam,
                                 int beamOrdinal,
                                 IdentityHashMap<Inter, Integer> sigOrdinals,
                                 BeamLinker linker,
                                 IdentityHashMap<Glyph, Integer> keptOrdinals,
                                 List<Section> verticalSections,
                                 IdentityHashMap<Section, Integer> sectionOrdinals,
                                 RegistryTracker registry,
                                 List<Registration> registrations,
                                 int registrationFloor,
                                 IdentityHashMap<Glyph, Integer> aliases,
                                 int maxStemThickness,
                                 double maxBeamSeedDx,
                                 double maxBeamSeedDyRatio,
                                 int minBeamStemsDx,
                                 int minBeamStumpDy,
                                 boolean tremolo,
                                 Totals totals,
                                 RowHasher... hashes)
        throws Exception
    {
        totals.constructors++;
        totals.registrations += registrations.size();
        if (tremolo) {
            totals.tremolos++;
        }
        final Set<Glyph> neighborSet = (Set<Glyph>) LINKER_NEIGHBOR_SEEDS.get(linker);
        final List<Glyph> neighbors = new ArrayList<>(neighborSet);
        totals.neighbors += neighbors.size();
        final Area seedArea = (Area) GET_SEED_AREA.invoke(linker);
        final Line2D median = beam.getMedian();
        final double slope = (median.getY2() - median.getY1())
                / (median.getX2() - median.getX1());
        final int effectiveProfile = Math.max(beam.getProfile(), system.getProfile());
        final int yGapPixels = sheet.getScale().toPixels(
                BeamStemRelation.getYGapMaximum(effectiveProfile));
        final double seedDy = maxBeamSeedDyRatio * yGapPixels;
        final double seedDx = maxBeamSeedDx;
        final Point2D seedStart = new Point2D.Double(
                median.getX1() - seedDx, median.getY1() - slope * seedDx);
        final Point2D seedStop = new Point2D.Double(
                median.getX2() + seedDx, median.getY2() + slope * seedDx);
        final double seedHeight = beam.getHeight() + 2 * seedDy;

        emit(String.format(
                "stemsbeamstumpbeam %s system %d ordinal %d sigOrdinal %d shape %s bounds %s "
                        + "median %s height %s beamProfile %d effectiveProfile %d yGapPixels %d "
                        + "seedDy %s seedMedian %s seedHeight %s groupMembers %d neighbors %d "
                        + "seedAreaBounds %s "
                        + "registrations %d width %s tremoloWidth %s stumps %d sideStumps %d "
                        + "looksLikeTremolo %s",
                page,
                system.getId(),
                beamOrdinal,
                sigOrdinals.get(beam),
                beam.getShape(),
                rectangle(beam.getBounds()),
                line(beam.getMedian()),
                hexDouble(beam.getHeight()),
                beam.getProfile(),
                effectiveProfile,
                yGapPixels,
                hexDouble(seedDy),
                point(seedStart) + ":" + point(seedStop),
                hexDouble(seedHeight),
                beam.getGroup().getMembers().size(),
                neighbors.size(),
                rectangle2D(seedArea.getBounds2D()),
                registrations.size(),
                hexDouble(beam.getMedian().getX2() - beam.getMedian().getX1()),
                TremoloInter.isTremoloWidth(
                        beam.getMedian().getX2() - beam.getMedian().getX1(), sheet.getScale()),
                ((List<Glyph>) LINKER_STUMPS.get(linker)).size(),
                ((Map<HorizontalSide, Glyph>) LINKER_SIDE_STUMPS.get(linker)).size(),
                tremolo), hashes);

        for (int ordinal = 0; ordinal < neighbors.size(); ordinal++) {
            final Glyph seed = neighbors.get(ordinal);
            emit(String.format(
                    "stemsbeamstumpneighbor %s system %d beam %d ordinal %d keptOrdinal %d "
                            + "bounds %s weight %d centerLine %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    ordinal,
                    requiredOrdinal(keptOrdinals, seed),
                    rectangle(seed.getBounds()),
                    seed.getWeight(),
                    line(requireCenterLine(seed))), hashes);
        }

        final SeedReplay seedReplay = replaySeeds(
                neighbors, keptOrdinals, seedArea, beam.getMedian(), minBeamStemsDx);
        totals.seedInputs += seedReplay.inputs.size();
        totals.purgeComparisons += seedReplay.comparisons.size();
        for (SeedInput input : seedReplay.inputs) {
            emit(String.format(
                    "stemsbeamstumpseed %s system %d beam %d sortOrdinal %d inputOrdinal %d "
                            + "keptOrdinal %d bounds %s crossX %s centerDistanceSq %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    input.sortOrdinal,
                    input.inputOrdinal,
                    input.keptOrdinal,
                    rectangle(input.glyph.getBounds()),
                    hexDouble(input.crossX),
                    hexDouble(input.centerDistanceSq)), hashes);
        }
        for (int ordinal = 0; ordinal < seedReplay.comparisons.size(); ordinal++) {
            final PurgeComparison comparison = seedReplay.comparisons.get(ordinal);
            if (comparison.action.startsWith("remove")) {
                totals.purgeRemovals++;
            } else if (comparison.action.equals("break")) {
                totals.purgeBreaks++;
            }
            emit(String.format(
                    "stemsbeamstumppurge %s system %d beam %d ordinal %d i %d j %d "
                            + "leftKept %d rightKept %d x1 %s x2 %s dx %s minDx %d "
                            + "yOverlap %d leftHeight %d rightHeight %d leftDistanceSq %s "
                            + "rightDistanceSq %s action %s survivors %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    ordinal,
                    comparison.i,
                    comparison.j,
                    comparison.leftKept,
                    comparison.rightKept,
                    hexDouble(comparison.x1),
                    hexDouble(comparison.x2),
                    hexDouble(comparison.x2 - comparison.x1),
                    minBeamStemsDx,
                    comparison.yOverlap,
                    comparison.leftHeight,
                    comparison.rightHeight,
                    hexDouble(comparison.leftDistanceSq),
                    hexDouble(comparison.rightDistanceSq),
                    comparison.action,
                    comparison.survivors), hashes);
        }

        final EnumMap<HorizontalSide, Glyph> replaySideSeeds = new EnumMap<>(HorizontalSide.class);
        if (!seedReplay.survivors.isEmpty()) {
            for (HorizontalSide side : HorizontalSide.values()) {
                final Glyph seed = seedReplay.survivors.get(
                        side == HorizontalSide.LEFT ? 0 : seedReplay.survivors.size() - 1);
                final double x = LineUtil.intersection(requireCenterLine(seed), beam.getMedian()).getX();
                final BeamPortion portion = BeamStemRelation.computeBeamPortion(
                        beam, x, sheet.getScale());
                emit(String.format(
                        "stemsbeamstumpsideclass %s system %d beam %d side %s keptOrdinal %d "
                                + "crossX %s portion %s selected %s",
                        page,
                        system.getId(),
                        beamOrdinal,
                        side,
                        requiredOrdinal(keptOrdinals, seed),
                        hexDouble(x),
                        portion,
                        portion != null && portion.side() == side), hashes);
                if ((portion != null) && (portion.side() == side)) {
                    replaySideSeeds.put(side, seed);
                }
            }
        }

        final Map<HorizontalSide, Glyph> productionSides =
                (Map<HorizontalSide, Glyph>) LINKER_SIDE_STUMPS.get(linker);
        final List<Glyph> expectedStumps = new ArrayList<>(seedReplay.survivors);
        final IdentityHashMap<Glyph, Boolean> firstNewUse = new IdentityHashMap<>();
        final List<Glyph> expectedRegistrations = new ArrayList<>();
        for (HorizontalSide side : HorizontalSide.values()) {
            totals.sides++;
            final Glyph sideSeed = replaySideSeeds.get(side);
            if (sideSeed != null) {
                totals.sideSeeds++;
                if (productionSides.get(side) != sideSeed) {
                    throw new IllegalStateException("side seed identity differs");
                }
                emit(String.format(
                        "stemsbeamstumpside %s system %d beam %d side %s mode seed "
                                + "keptOrdinal %d canonicalAlias %d",
                        page,
                        system.getId(),
                        beamOrdinal,
                        side,
                        requiredOrdinal(keptOrdinals, sideSeed),
                        alias(aliases, sideSeed)), hashes);
                continue;
            }

            totals.buildAttempts++;
            final BuildReplay build = replayBuild(
                    linker,
                    beam,
                    side,
                    verticalSections,
                    sectionOrdinals,
                    sigOrdinals,
                    maxStemThickness,
                    minBeamStumpDy);
            totals.sectionRows += build.sections.size();
            totals.steps += build.steps.size();
            if (build.sections.isEmpty()) {
                totals.emptySections++;
            } else if (build.compoundWeight == 0) {
                totals.zeroCompounds++;
            }
            final Glyph actual = productionSides.get(side);
            if (build.candidate == null) {
                if (actual != null) {
                    throw new IllegalStateException("production built a missing replay candidate");
                }
            } else {
                totals.candidates++;
                if (build.directions.accepted()) {
                    totals.directionAccepted++;
                    final Glyph canonical = registry.findEqual(build.candidate);
                    if (canonical == null || canonical != actual) {
                        throw new IllegalStateException("side build canonical identity differs");
                    }
                    final Origin origin = registry.origin(actual);
                    final boolean isNew = origin.registrationOrdinal >= registrationFloor
                            && firstNewUse.put(actual, Boolean.TRUE) == null;
                    if (isNew) {
                        totals.newBuilds++;
                        expectedRegistrations.add(actual);
                    } else {
                        totals.reusedBuilds++;
                    }
                    emitBuild(
                            page,
                            system,
                            beamOrdinal,
                            side,
                            build,
                            (isNew ? "new:" : "reuse:") + origin.token(),
                            Integer.toString(alias(aliases, actual)),
                            hashes);
                    if (side == HorizontalSide.LEFT) {
                        expectedStumps.add(0, actual);
                    } else {
                        expectedStumps.add(actual);
                    }
                } else {
                    totals.directionRejected++;
                    if (actual != null) {
                        throw new IllegalStateException("production accepted rejected directions");
                    }
                    emitBuild(page, system, beamOrdinal, side, build, "none", "-", hashes);
                }
            }
            if (build.candidate == null) {
                emitBuild(page, system, beamOrdinal, side, build, "none", "-", hashes);
            }
        }

        if (expectedRegistrations.size() != registrations.size()) {
            throw new IllegalStateException("beam registration count differs from replay");
        }
        for (int ordinal = 0; ordinal < registrations.size(); ordinal++) {
            if (expectedRegistrations.get(ordinal) != registrations.get(ordinal).glyph) {
                throw new IllegalStateException("beam registration order differs from replay");
            }
            emitRegistration(
                    page, system, beamOrdinal, registrations.get(ordinal), hashes);
        }

        final List<Glyph> productionStumps = (List<Glyph>) LINKER_STUMPS.get(linker);
        if (!sameIdentities(expectedStumps, productionStumps)) {
            throw new IllegalStateException("final stump list differs from replay");
        }
        totals.finalStumps += productionStumps.size();
        totals.finalSideStumps += productionSides.size();
        for (int ordinal = 0; ordinal < productionStumps.size(); ordinal++) {
            final Glyph stump = productionStumps.get(ordinal);
            emit(String.format(
                    "stemsbeamstumpfinal %s system %d beam %d ordinal %d origin %s "
                            + "canonicalAlias %d bounds %s weight %d runs %d:%016x",
                    page,
                    system.getId(),
                    beamOrdinal,
                    ordinal,
                    glyphOrigin(stump, keptOrdinals, registry),
                    alias(aliases, stump),
                    rectangle(stump.getBounds()),
                    stump.getWeight(),
                    runCount(stump),
                    runTableHash(stump)), hashes);
        }
        for (HorizontalSide side : HorizontalSide.values()) {
            final Glyph stump = productionSides.get(side);
            emit(String.format(
                    "stemsbeamstumpfinalside %s system %d beam %d side %s origin %s "
                            + "canonicalAlias %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    side,
                    stump != null ? glyphOrigin(stump, keptOrdinals, registry) : "-",
                    stump != null ? Integer.toString(alias(aliases, stump)) : "-"), hashes);
        }
    }

    private static SeedReplay replaySeeds (List<Glyph> neighbors,
                                           IdentityHashMap<Glyph, Integer> keptOrdinals,
                                           Area seedArea,
                                           Line2D median,
                                           int minBeamStemsDx)
    {
        final List<Glyph> intersected = new ArrayList<>(
                Glyphs.intersectedGlyphs(neighbors, seedArea));
        final IdentityHashMap<Glyph, Integer> inputOrdinals = identityOrdinals(intersected);
        Collections.sort(intersected, Comparator.comparingDouble(
                glyph -> LineUtil.intersection(requireCenterLine(glyph), median).getX()));
        final List<SeedInput> inputs = new ArrayList<>();
        for (int ordinal = 0; ordinal < intersected.size(); ordinal++) {
            final Glyph glyph = intersected.get(ordinal);
            final Line2D line = requireCenterLine(glyph);
            final Point2D cross = LineUtil.intersection(line, median);
            inputs.add(new SeedInput(
                    glyph,
                    ordinal,
                    inputOrdinals.get(glyph),
                    requiredOrdinal(keptOrdinals, glyph),
                    cross.getX(),
                    line.ptSegDistSq(cross)));
        }

        final List<Glyph> survivors = new ArrayList<>(intersected);
        final List<PurgeComparison> comparisons = new ArrayList<>();
        NextSeed:
        for (int i = 0; i < survivors.size(); i++) {
            final Glyph left = survivors.get(i);
            final Line2D leftLine = requireCenterLine(left);
            final Point2D leftCross = LineUtil.intersection(leftLine, median);
            final double x1 = leftCross.getX();
            for (int j = i + 1; j < survivors.size(); j++) {
                final Glyph right = survivors.get(j);
                final Line2D rightLine = requireCenterLine(right);
                final Point2D rightCross = LineUtil.intersection(rightLine, median);
                final double x2 = rightCross.getX();
                final int overlap = GeoUtil.yOverlap(left.getBounds(), right.getBounds());
                final double leftDistance = leftLine.ptSegDistSq(leftCross);
                final double rightDistance = rightLine.ptSegDistSq(rightCross);
                final String action;
                if ((x2 - x1) >= minBeamStemsDx) {
                    action = "break";
                    comparisons.add(comparison(
                            i, j, left, right, keptOrdinals, x1, x2, overlap,
                            leftDistance, rightDistance, action, survivors));
                    break;
                }
                if (overlap > 0) {
                    if (left.getHeight() >= right.getHeight()) {
                        survivors.remove(j);
                        action = "removeRightOverlap";
                        j--;
                    } else {
                        survivors.remove(i--);
                        action = "removeLeftOverlap";
                        comparisons.add(comparison(
                                i + 1, j, left, right, keptOrdinals, x1, x2, overlap,
                                leftDistance, rightDistance, action, survivors));
                        continue NextSeed;
                    }
                } else if (leftDistance <= rightDistance) {
                    survivors.remove(j);
                    action = "removeRightDistance";
                    j--;
                } else {
                    survivors.remove(i--);
                    action = "removeLeftDistance";
                    comparisons.add(comparison(
                            i + 1, j, left, right, keptOrdinals, x1, x2, overlap,
                            leftDistance, rightDistance, action, survivors));
                    continue NextSeed;
                }
                comparisons.add(comparison(
                        i, j + 1, left, right, keptOrdinals, x1, x2, overlap,
                        leftDistance, rightDistance, action, survivors));
            }
        }
        return new SeedReplay(inputs, comparisons, survivors);
    }

    private static PurgeComparison comparison (int i,
                                               int j,
                                               Glyph left,
                                               Glyph right,
                                               IdentityHashMap<Glyph, Integer> keptOrdinals,
                                               double x1,
                                               double x2,
                                               int overlap,
                                               double leftDistance,
                                               double rightDistance,
                                               String action,
                                               List<Glyph> survivors)
    {
        return new PurgeComparison(
                i,
                j,
                requiredOrdinal(keptOrdinals, left),
                requiredOrdinal(keptOrdinals, right),
                x1,
                x2,
                overlap,
                left.getHeight(),
                right.getHeight(),
                leftDistance,
                rightDistance,
                action,
                glyphOrdinals(survivors, keptOrdinals));
    }

    private static BuildReplay replayBuild (BeamLinker linker,
                                            AbstractBeamInter beam,
                                            HorizontalSide side,
                                            List<Section> verticalSections,
                                            IdentityHashMap<Section, Integer> sectionOrdinals,
                                            IdentityHashMap<Inter, Integer> sigOrdinals,
                                            int maxStemThickness,
                                            int minBeamStumpDy)
        throws Exception
    {
        final Area area = (Area) GET_STUMP_AREA.invoke(linker, side);
        final List<Section> sections = new ArrayList<>(
                Sections.intersectedSections(area, verticalSections));
        final IdentityHashMap<Section, Integer> inputOrdinals = sectionOrdinals(sections);
        final int xDir = side.direction();
        final double sideX = xDir < 0 ? beam.getMedian().getX1() : beam.getMedian().getX2();
        final double refX = sideX - xDir * maxStemThickness / 2.0;
        Collections.sort(sections, Comparator.comparingDouble(
                section -> Math.abs(section.getAreaCenter().getX() - refX)));
        final List<SectionEvidence> evidence = new ArrayList<>();
        for (int ordinal = 0; ordinal < sections.size(); ordinal++) {
            final Section section = sections.get(ordinal);
            evidence.add(new SectionEvidence(
                    section,
                    ordinal,
                    inputOrdinals.get(section),
                    requiredOrdinal(sectionOrdinals, section),
                    Math.abs(section.getAreaCenter().getX() - refX)));
        }
        if (sections.isEmpty()) {
            return new BuildReplay(
                    area,
                    refX,
                    stumpMedianStart(beam, side, maxStemThickness),
                    stumpMedianStop(beam, side, maxStemThickness),
                    beam.getHeight(),
                    evidence,
                    List.of(),
                    0,
                    null,
                    null,
                    null);
        }

        final SectionCompound compound = new SectionCompound();
        final List<BuildStep> steps = new ArrayList<>();
        for (int ordinal = 0; ordinal < sections.size(); ordinal++) {
            final Section section = sections.get(ordinal);
            compound.addSection(section);
            final int width = compound.getWidth();
            final boolean tooWide = width > maxStemThickness;
            if (tooWide) {
                compound.removeSection(section);
            }
            steps.add(new BuildStep(
                    ordinal,
                    requiredOrdinal(sectionOrdinals, section),
                    width,
                    tooWide,
                    compound.getWeight(),
                    compound.getWeight() != 0 ? compound.getBounds() : null,
                    memberSources(compound, sectionOrdinals)));
            if (tooWide) {
                break;
            }
        }
        if (compound.getWeight() == 0) {
            return new BuildReplay(
                    area,
                    refX,
                    stumpMedianStart(beam, side, maxStemThickness),
                    stumpMedianStop(beam, side, maxStemThickness),
                    beam.getHeight(),
                    evidence,
                    steps,
                    0,
                    null,
                    null,
                    null);
        }

        final Glyph candidate = compound.toGlyph(GlyphGroup.STUMP);
        final Set<VerticalSide> productionDirections =
                (Set<VerticalSide>) GET_STUMP_DIRECTIONS.invoke(linker, candidate);
        final DirectionEvidence directions = directionEvidence(
                linker, beam, candidate, sigOrdinals, minBeamStumpDy);
        if (!sameDirections(productionDirections, directions.directions)) {
            throw new IllegalStateException("stump direction replay differs");
        }
        return new BuildReplay(
                area,
                refX,
                stumpMedianStart(beam, side, maxStemThickness),
                stumpMedianStop(beam, side, maxStemThickness),
                beam.getHeight(),
                evidence,
                steps,
                compound.getWeight(),
                compound.getBounds(),
                candidate,
                directions);
    }

    private static DirectionEvidence directionEvidence (BeamLinker linker,
                                                         AbstractBeamInter beam,
                                                         Glyph candidate,
                                                         IdentityHashMap<Inter, Integer> sigOrdinals,
                                                         int minBeamStumpDy)
    {
        final Point2D center = candidate.getCenter2D();
        final List<AbstractBeamInter> siblings = linker.getSiblingBeamsAt(center);
        if (siblings.isEmpty()) {
            return new DirectionEvidence(
                    "-", -1, -1, false, false, false, center.getX(),
                    requireCenterLine(candidate), Double.NaN, Double.NaN, Double.NaN, Double.NaN,
                    minBeamStumpDy, null);
        }
        final AbstractBeamInter first = siblings.get(0);
        final AbstractBeamInter last = siblings.get(siblings.size() - 1);
        final boolean selfIsFirst = beam == first;
        final boolean selfIsLast = beam == last;
        final boolean selfGlyphFirst = beam.getGlyph() == first.getGlyph();
        final boolean selfGlyphLast = beam.getGlyph() == last.getGlyph();
        final boolean internal = !selfIsFirst && !selfIsLast && !selfGlyphFirst && !selfGlyphLast;
        if (internal) {
            return new DirectionEvidence(
                    interOrdinals(siblings, sigOrdinals),
                    requiredInterOrdinal(sigOrdinals, first),
                    requiredInterOrdinal(sigOrdinals, last),
                    selfGlyphFirst,
                    selfGlyphLast,
                    true,
                    center.getX(),
                    requireCenterLine(candidate),
                    Double.NaN,
                    Double.NaN,
                    Double.NaN,
                    Double.NaN,
                    minBeamStumpDy,
                    null);
        }
        final double x = center.getX();
        final Line2D line = requireCenterLine(candidate);
        final double topY = LineUtil.yAtX(first.getBorder(VerticalSide.TOP), x);
        final double bottomY = LineUtil.yAtX(last.getBorder(VerticalSide.BOTTOM), x);
        final double dyTop = Math.max(0, topY - line.getY1());
        final double dyBottom = Math.max(0, line.getY2() - bottomY);
        final List<VerticalSide> directions = new ArrayList<>();
        if (dyTop >= minBeamStumpDy) {
            directions.add(VerticalSide.TOP);
        }
        if (dyBottom >= minBeamStumpDy) {
            directions.add(VerticalSide.BOTTOM);
        }
        return new DirectionEvidence(
                interOrdinals(siblings, sigOrdinals),
                requiredInterOrdinal(sigOrdinals, first),
                requiredInterOrdinal(sigOrdinals, last),
                selfGlyphFirst,
                selfGlyphLast,
                false,
                x,
                line,
                topY,
                bottomY,
                dyTop,
                dyBottom,
                minBeamStumpDy,
                directions);
    }

    private static void emitBuild (String page,
                                   SystemInfo system,
                                   int beamOrdinal,
                                   HorizontalSide side,
                                   BuildReplay build,
                                   String registration,
                                   String canonicalAlias,
                                   RowHasher... hashes)
    {
        emit(String.format(
                "stemsbeamstumpbuild %s system %d beam %d side %s areaBounds %s refX %s "
                        + "stumpMedian %s stumpHeight %s sections %d steps %d compoundWeight %d "
                        + "compoundBounds %s "
                        + "candidate %s directions %s registration %s canonicalAlias %s",
                page,
                system.getId(),
                beamOrdinal,
                side,
                rectangle2D(build.area.getBounds2D()),
                hexDouble(build.refX),
                point(build.stumpStart) + ":" + point(build.stumpStop),
                hexDouble(build.stumpHeight),
                build.sections.size(),
                build.steps.size(),
                build.compoundWeight,
                optionalRectangle(build.compoundBounds),
                build.candidate != null
                        ? rectangle(build.candidate.getBounds()) + ":" + build.candidate.getWeight()
                                + ":" + runCount(build.candidate) + ":"
                                + String.format("%016x", runTableHash(build.candidate))
                        : "none",
                build.directions != null ? build.directions.directionToken() : "-",
                registration,
                canonicalAlias), hashes);
        for (SectionEvidence section : build.sections) {
            emit(String.format(
                    "stemsbeamstumpsection %s system %d beam %d side %s sortOrdinal %d "
                            + "inputOrdinal %d sourceOrdinal %d bounds %s weight %d firstPos %d "
                            + "runs %d:%016x areaCenter %d:%d distance %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    side,
                    section.sortOrdinal,
                    section.inputOrdinal,
                    section.sourceOrdinal,
                    rectangle(section.section.getBounds()),
                    section.section.getWeight(),
                    section.section.getFirstPos(),
                    section.section.getRunCount(),
                    sectionRunHash(section.section),
                    section.section.getAreaCenter().x,
                    section.section.getAreaCenter().y,
                    hexDouble(section.distance)), hashes);
        }
        for (BuildStep step : build.steps) {
            emit(String.format(
                    "stemsbeamstumpstep %s system %d beam %d side %s sortOrdinal %d "
                            + "sourceOrdinal %d afterAddWidth %d tooWide %s finalWeight %d "
                            + "finalBounds %s members %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    side,
                    step.sortOrdinal,
                    step.sourceOrdinal,
                    step.afterAddWidth,
                    step.tooWide,
                    step.finalWeight,
                    optionalRectangle(step.finalBounds),
                    step.memberSources), hashes);
        }
        if (build.directions != null) {
            final DirectionEvidence d = build.directions;
            emit(String.format(
                    "stemsbeamstumpdirections %s system %d beam %d side %s siblings %s "
                            + "firstSig %s lastSig %s selfGlyphFirst %s selfGlyphLast %s "
                            + "internal %s x %s centerLine %s topBorderY %s bottomBorderY %s "
                            + "dyTop %s dyBottom %s minDy %d directions %s",
                    page,
                    system.getId(),
                    beamOrdinal,
                    side,
                    d.siblings,
                    optionalInteger(d.firstSig),
                    optionalInteger(d.lastSig),
                    d.selfGlyphFirst,
                    d.selfGlyphLast,
                    d.internal,
                    hexDouble(d.x),
                    line(d.line),
                    finiteDouble(d.topBorderY),
                    finiteDouble(d.bottomBorderY),
                    finiteDouble(d.dyTop),
                    finiteDouble(d.dyBottom),
                    d.minDy,
                    d.directionToken()), hashes);
        }
    }

    private static void emitRegistration (String page,
                                          SystemInfo system,
                                          int beamOrdinal,
                                          Registration registration,
                                          RowHasher... hashes)
    {
        final Glyph glyph = registration.glyph;
        emit(String.format(
                "stemsbeamstumpreg %s system %d beam %d ordinal %d phase %s phaseOrdinal %d "
                        + "bounds %s weight %d runs %d:%016x",
                page,
                system.getId(),
                beamOrdinal,
                registration.origin.registrationOrdinal,
                registration.origin.phase,
                registration.origin.phaseOrdinal,
                rectangle(glyph.getBounds()),
                glyph.getWeight(),
                runCount(glyph),
                runTableHash(glyph)), hashes);
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

    private static boolean sameDirections (Set<VerticalSide> actual,
                                           List<VerticalSide> expected)
    {
        if (actual == null || expected == null) {
            return actual == null && expected == null;
        }
        return actual.size() == expected.size() && actual.containsAll(expected);
    }

    private static boolean sameIdentities (List<Glyph> left,
                                           List<Glyph> right)
    {
        if (left.size() != right.size()) {
            return false;
        }
        for (int index = 0; index < left.size(); index++) {
            if (left.get(index) != right.get(index)) {
                return false;
            }
        }
        return true;
    }

    private static String glyphOrigin (Glyph glyph,
                                       IdentityHashMap<Glyph, Integer> keptOrdinals,
                                       RegistryTracker registry)
    {
        final Integer kept = keptOrdinals.get(glyph);
        return kept != null ? "kept:" + kept : registry.origin(glyph).token();
    }

    private static String glyphOrdinals (List<Glyph> glyphs,
                                         IdentityHashMap<Glyph, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Glyph glyph : glyphs) {
            if (builder.length() != 0) {
                builder.append(',');
            }
            builder.append(requiredOrdinal(ordinals, glyph));
        }
        return builder.length() == 0 ? "-" : builder.toString();
    }

    private static String interOrdinals (List<AbstractBeamInter> inters,
                                         IdentityHashMap<Inter, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (AbstractBeamInter inter : inters) {
            if (builder.length() != 0) {
                builder.append(',');
            }
            builder.append(requiredInterOrdinal(ordinals, inter));
        }
        return builder.length() == 0 ? "-" : builder.toString();
    }

    private static String memberSources (SectionCompound compound,
                                         IdentityHashMap<Section, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Section section : compound.getMembers()) {
            if (builder.length() != 0) {
                builder.append(',');
            }
            builder.append(requiredOrdinal(ordinals, section));
        }
        return builder.length() == 0 ? "-" : builder.toString();
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

    private static IdentityHashMap<Section, Integer> sectionOrdinals (List<Section> sections)
    {
        final IdentityHashMap<Section, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < sections.size(); ordinal++) {
            if (ordinals.put(sections.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate section identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static IdentityHashMap<Inter, Integer> interOrdinals (List<Inter> inters)
    {
        final IdentityHashMap<Inter, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < inters.size(); ordinal++) {
            if (ordinals.put(inters.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate inter identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static int requiredOrdinal (IdentityHashMap<Glyph, Integer> ordinals,
                                        Glyph glyph)
    {
        final Integer ordinal = ordinals.get(glyph);
        if (ordinal == null) {
            throw new IllegalStateException("glyph is absent from ordered pool");
        }
        return ordinal;
    }

    private static int requiredOrdinal (IdentityHashMap<Section, Integer> ordinals,
                                        Section section)
    {
        final Integer ordinal = ordinals.get(section);
        if (ordinal == null) {
            throw new IllegalStateException("section is absent from ordered pool");
        }
        return ordinal;
    }

    private static int requiredInterOrdinal (IdentityHashMap<Inter, Integer> ordinals,
                                             Inter inter)
    {
        final Integer ordinal = ordinals.get(inter);
        if (ordinal == null) {
            throw new IllegalStateException("inter is absent from ordered pool");
        }
        return ordinal;
    }

    private static int alias (IdentityHashMap<Glyph, Integer> aliases,
                              Glyph glyph)
    {
        final Integer old = aliases.get(glyph);
        if (old != null) {
            return old;
        }
        final int ordinal = aliases.size();
        aliases.put(glyph, ordinal);
        return ordinal;
    }

    private static Field declaredField (Class<?> owner,
                                        String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Point2D stumpMedianStart (AbstractBeamInter beam,
                                             HorizontalSide side,
                                             int maxStemThickness)
    {
        if (side == HorizontalSide.LEFT) {
            return beam.getMedian().getP1();
        }
        return LineUtil.intersectionAtX(
                beam.getMedian(), beam.getMedian().getX2() - maxStemThickness);
    }

    private static Point2D stumpMedianStop (AbstractBeamInter beam,
                                            HorizontalSide side,
                                            int maxStemThickness)
    {
        if (side == HorizontalSide.RIGHT) {
            return beam.getMedian().getP2();
        }
        return LineUtil.intersectionAtX(
                beam.getMedian(), beam.getMedian().getX1() + maxStemThickness);
    }

    private static Line2D requireCenterLine (Glyph glyph)
    {
        final Line2D line = glyph.getCenterLine();
        if (line == null) {
            throw new IllegalStateException("glyph has no center line");
        }
        return line;
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
        return point(line.getP1()) + ":" + point(line.getP2());
    }

    private static String point (Point2D point)
    {
        return hexDouble(point.getX()) + ":" + hexDouble(point.getY());
    }

    private static String optionalInteger (int value)
    {
        return value >= 0 ? Integer.toString(value) : "-";
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

    private static int runCount (Glyph glyph)
    {
        int count = 0;
        final var table = glyph.getRunTable();
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            for (Iterator<Run> iterator = table.iterator(sequence); iterator.hasNext();) {
                iterator.next();
                count++;
            }
        }
        return count;
    }

    private static long runTableHash (Glyph glyph)
    {
        final RowHasher hash = new RowHasher();
        final var table = glyph.getRunTable();
        hash.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<Run> iterator = table.iterator(sequence); iterator.hasNext();) {
                final Run run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            hash.add(row.toString());
        }
        return hash.value();
    }

    private static long sectionRunHash (Section section)
    {
        final RowHasher hash = new RowHasher();
        hash.add(section.getOrientation() + " " + section.getFirstPos());
        for (Run run : section.getRuns()) {
            hash.add(run.getStart() + ":" + run.getLength());
        }
        return hash.value();
    }

    private static void emit (String row,
                              RowHasher... hashes)
    {
        System.out.println(row);
        for (RowHasher hash : hashes) {
            hash.add(row);
        }
    }

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS beam-stump oracle.");
        System.out.println("#");
        System.out.println("# Every page starts in a fresh Epsilon-GC JVM and reaches real HEADS.");
        System.out.println("# The probe then constructs every BeamLinker in stable x order, exactly as");
        System.out.println("# StemsRetriever.inspectStems does before tremolo removal or head linking.");
        System.out.println("#");
        System.out.println("# Rows freeze neighboring seed order, parallelogram intersection, stable cross-x");
        System.out.println("# sort, every duplicate-purge comparison, beam-side classification, and both");
        System.out.println("# horizontal sides. Each side build freezes full VLAG intersection/source order,");
        System.out.println("# stable area-center distance order, every width/break compound step, fixed glyph");
        System.out.println("# raster, sibling extrema and exact dy direction evidence, registration/reuse,");
        System.out.println("# canonical aliases, final stump/side lists, and looksLikeTremolo.");
        System.out.println("#");
        System.out.println("# pre:N, reg:N, kept:N, source section ordinals, and sig ordinals are local");
        System.out.println("# deterministic boundary ordinals, never Java IDs or identity hash codes.");
        System.out.println("# Doubles include hex text/raw bits. FNV-1a-64 covers semantic rows; the runner");
        System.out.println("# appends SHA-256 commitments for the source and emitted body.");
    }

    private static final class RegistryTracker
    {
        private final GlyphIndex index;

        private final IdentityHashMap<Glyph, Origin> origins = new IdentityHashMap<>();

        private final List<Glyph> strong = new ArrayList<>();

        private final int preCount;

        private int nextRegistrationOrdinal;

        RegistryTracker (GlyphIndex index)
        {
            this.index = index;
            final List<Glyph> initial = index.getEntities();
            preCount = initial.size();
            for (int ordinal = 0; ordinal < initial.size(); ordinal++) {
                final Glyph glyph = initial.get(ordinal);
                origins.put(glyph, new Origin("pre", ordinal, ordinal, -1));
                strong.add(glyph);
            }
        }

        int preCount ()
        {
            return preCount;
        }

        int nextRegistrationOrdinal ()
        {
            return nextRegistrationOrdinal;
        }

        List<Registration> capture (String phase)
        {
            final List<Registration> registrations = new ArrayList<>();
            for (Glyph glyph : index.getEntities()) {
                if (!origins.containsKey(glyph)) {
                    final Origin origin = new Origin(
                            phase, registrations.size(), -1, nextRegistrationOrdinal++);
                    origins.put(glyph, origin);
                    strong.add(glyph);
                    registrations.add(new Registration(glyph, origin));
                }
            }
            return registrations;
        }

        Glyph findEqual (Glyph candidate)
        {
            Glyph match = null;
            for (Glyph glyph : strong) {
                if (glyph.equals(candidate)) {
                    if (match != null && match != glyph) {
                        throw new IllegalStateException("multiple registered originals are equal");
                    }
                    match = glyph;
                }
            }
            return match;
        }

        Origin origin (Glyph glyph)
        {
            final Origin origin = origins.get(glyph);
            if (origin == null) {
                throw new IllegalStateException("untracked glyph identity");
            }
            return origin;
        }
    }

    private record Origin(String phase, int phaseOrdinal, int preOrdinal, int registrationOrdinal)
    {
        String token ()
        {
            return preOrdinal >= 0 ? "pre:" + preOrdinal : "reg:" + registrationOrdinal;
        }
    }

    private record Registration(Glyph glyph, Origin origin)
    {
    }

    private record SeedInput(
            Glyph glyph,
            int sortOrdinal,
            int inputOrdinal,
            int keptOrdinal,
            double crossX,
            double centerDistanceSq)
    {
    }

    private record PurgeComparison(
            int i,
            int j,
            int leftKept,
            int rightKept,
            double x1,
            double x2,
            int yOverlap,
            int leftHeight,
            int rightHeight,
            double leftDistanceSq,
            double rightDistanceSq,
            String action,
            String survivors)
    {
    }

    private record SeedReplay(
            List<SeedInput> inputs,
            List<PurgeComparison> comparisons,
            List<Glyph> survivors)
    {
    }

    private record SectionEvidence(
            Section section,
            int sortOrdinal,
            int inputOrdinal,
            int sourceOrdinal,
            double distance)
    {
    }

    private record BuildStep(
            int sortOrdinal,
            int sourceOrdinal,
            int afterAddWidth,
            boolean tooWide,
            int finalWeight,
            Rectangle finalBounds,
            String memberSources)
    {
    }

    private record DirectionEvidence(
            String siblings,
            int firstSig,
            int lastSig,
            boolean selfGlyphFirst,
            boolean selfGlyphLast,
            boolean internal,
            double x,
            Line2D line,
            double topBorderY,
            double bottomBorderY,
            double dyTop,
            double dyBottom,
            int minDy,
            List<VerticalSide> directions)
    {
        boolean accepted ()
        {
            return directions != null && !directions.isEmpty();
        }

        String directionToken ()
        {
            if (directions == null) {
                return "null";
            }
            if (directions.isEmpty()) {
                return "none";
            }
            final StringBuilder builder = new StringBuilder();
            for (VerticalSide side : directions) {
                if (builder.length() != 0) {
                    builder.append(',');
                }
                builder.append(side);
            }
            return builder.toString();
        }
    }

    private record BuildReplay(
            Area area,
            double refX,
            Point2D stumpStart,
            Point2D stumpStop,
            double stumpHeight,
            List<SectionEvidence> sections,
            List<BuildStep> steps,
            int compoundWeight,
            Rectangle compoundBounds,
            Glyph candidate,
            DirectionEvidence directions)
    {
    }

    private static final class Totals
    {
        int constructors;
        int sides;
        int neighbors;
        int seedInputs;
        int purgeComparisons;
        int purgeRemovals;
        int purgeBreaks;
        int sideSeeds;
        int buildAttempts;
        int emptySections;
        int zeroCompounds;
        int candidates;
        int directionAccepted;
        int directionRejected;
        int registrations;
        int newBuilds;
        int reusedBuilds;
        int sectionRows;
        int steps;
        int finalStumps;
        int finalSideStumps;
        int tremolos;

        void include (Totals other)
        {
            constructors += other.constructors;
            sides += other.sides;
            neighbors += other.neighbors;
            seedInputs += other.seedInputs;
            purgeComparisons += other.purgeComparisons;
            purgeRemovals += other.purgeRemovals;
            purgeBreaks += other.purgeBreaks;
            sideSeeds += other.sideSeeds;
            buildAttempts += other.buildAttempts;
            emptySections += other.emptySections;
            zeroCompounds += other.zeroCompounds;
            candidates += other.candidates;
            directionAccepted += other.directionAccepted;
            directionRejected += other.directionRejected;
            registrations += other.registrations;
            newBuilds += other.newBuilds;
            reusedBuilds += other.reusedBuilds;
            sectionRows += other.sectionRows;
            steps += other.steps;
            finalStumps += other.finalStumps;
            finalSideStumps += other.finalSideStumps;
            tremolos += other.tremolos;
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            for (byte value : (row + "\n").getBytes(StandardCharsets.UTF_8)) {
                this.value ^= Byte.toUnsignedLong(value);
                this.value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }
}
