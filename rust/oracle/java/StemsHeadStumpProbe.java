// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.awt.geom.Rectangle2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.dynamic.SectionCompound;
import org.audiveris.omr.lag.DynamicSection;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.lag.Sections;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact, identity-free oracle for {@code HeadLinker.CLinker.buildStump()} and the registration
 * order which surrounds it.
 *
 * <p>The probe stops at the real HEADS boundary, then reproduces the constructor-only prefix of
 * {@code StemsRetriever.inspectStems}: parameters and purged seeds, sorted BeamLinkers (including
 * tremolo removal), and sorted HeadLinkers. It never calls either inspection pass. Every fallback
 * compound is independently rebuilt without registration and checked against the glyph which the
 * real constructor registered, including candidates rejected by {@code standsOut()}.
 */
@SuppressWarnings("unchecked")
public class StemsHeadStumpProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;

    private static final Field RETRIEVER_SYSTEM_SEEDS;

    private static final Field RETRIEVER_SYSTEM_BEAMS;

    private static final Field RETRIEVER_SYSTEM_HEADS;

    private static final Method PURGE_NO_STEM_SEEDS;

    private static final Method GET_NEIGHBORING_SEEDS;

    private static final Field PARAM_MAX_HEAD_SEED_DY;

    private static final Field PARAM_MAX_HEAD_OUT_DX;

    private static final Field PARAM_MAX_HEAD_IN_DX;

    private static final Field PARAM_MIN_HEAD_STUMP_DY;

    private static final Field PARAM_STUMP_AREA_DX_IN;

    private static final Field PARAM_STUMP_AREA_DX_OUT;

    private static final Field PARAM_STUMP_AREA_DY;

    private static final Field PARAM_MAIN_STEM_THICKNESS;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);

            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = declaredField(StemsRetriever.class, "systemBeams");
            RETRIEVER_SYSTEM_HEADS = declaredField(StemsRetriever.class, "systemHeads");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            GET_NEIGHBORING_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "getNeighboringSeeds", Rectangle.class);
            GET_NEIGHBORING_SEEDS.setAccessible(true);

            PARAM_MAX_HEAD_SEED_DY = declaredField(parameters, "maxHeadSeedDy");
            PARAM_MAX_HEAD_OUT_DX = declaredField(parameters, "maxHeadOutDx");
            PARAM_MAX_HEAD_IN_DX = declaredField(parameters, "maxHeadInDx");
            PARAM_MIN_HEAD_STUMP_DY = declaredField(parameters, "minHeadStumpDy");
            PARAM_STUMP_AREA_DX_IN = declaredField(parameters, "stumpAreaDxIn");
            PARAM_STUMP_AREA_DX_OUT = declaredField(parameters, "stumpAreaDxOut");
            PARAM_STUMP_AREA_DY = declaredField(parameters, "stumpAreaDy");
            PARAM_MAIN_STEM_THICKNESS = declaredField(parameters, "mainStemThickness");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsHeadStumpProbe ()
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
        final Totals pageTotals = new Totals();
        final RowHasher pageHash = new RowHasher();

        System.out.printf(
                "stemsheadstumppage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());

        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, pageTotals, pageHash);
        }

        System.out.printf(
                "stemsheadstumppagesummary %s systems %d beams %d tremolos %d beamRegs %d "
                        + "heads %d corners %d seeds %d fallbacks %d emptyBuilds %d "
                        + "buildCandidates %d acceptedBuilds %d rejectedBuilds %d headRegs %d "
                        + "newBuilds %d reusedBuilds %d sections %d steps %d subsections %d "
                        + "hash %016x%n",
                page,
                sheet.getSystems().size(),
                pageTotals.beams,
                pageTotals.tremolos,
                pageTotals.beamRegs,
                pageTotals.heads,
                pageTotals.corners,
                pageTotals.seeds,
                pageTotals.fallbacks,
                pageTotals.emptyBuilds,
                pageTotals.buildCandidates,
                pageTotals.acceptedBuilds,
                pageTotals.rejectedBuilds,
                pageTotals.headRegs,
                pageTotals.newBuilds,
                pageTotals.reusedBuilds,
                pageTotals.sections,
                pageTotals.steps,
                pageTotals.subsections,
                pageHash.value());
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
                                   Totals pageTotals,
                                   RowHasher pageHash)
        throws Exception
    {
        final Totals totals = new Totals();
        final RowHasher systemHash = new RowHasher();
        final StemsRetriever retriever = new StemsRetriever(system);
        final Object params = PARAMETERS_CONSTRUCTOR.newInstance(system, sheet.getScale());
        RETRIEVER_PARAMS.set(retriever, params);

        final int maxHeadSeedDy = PARAM_MAX_HEAD_SEED_DY.getInt(params);
        final int maxHeadOutDx = PARAM_MAX_HEAD_OUT_DX.getInt(params);
        final int maxHeadInDx = PARAM_MAX_HEAD_IN_DX.getInt(params);
        final int minHeadStumpDy = PARAM_MIN_HEAD_STUMP_DY.getInt(params);
        final double stumpAreaDxIn = PARAM_STUMP_AREA_DX_IN.getDouble(params);
        final double stumpAreaDxOut = PARAM_STUMP_AREA_DX_OUT.getDouble(params);
        final double stumpAreaDy = PARAM_STUMP_AREA_DY.getDouble(params);
        final int mainStemThickness = PARAM_MAIN_STEM_THICKNESS.getInt(params);

        final List<Glyph> sourceSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        final List<Glyph> keptSeeds = new ArrayList<>(sourceSeeds);
        PURGE_NO_STEM_SEEDS.invoke(retriever, keptSeeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, keptSeeds);
        final IdentityHashMap<Glyph, Integer> keptOrdinals = identityOrdinals(keptSeeds);

        final List<Section> verticalSections = new ArrayList<>(system.getVerticalSections());
        final IdentityHashMap<Section, Integer> sectionOrdinals = sectionOrdinals(verticalSections);
        final RegistryTracker registry = new RegistryTracker(sheet.getGlyphIndex());

        final List<Inter> beams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = identityInterOrdinals(beams);
        Collections.sort(beams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, beams);

        emit(String.format(
                "stemsheadstumpsystem %s system %d profile %d interline %d stemThickness %d "
                        + "bounds %s sourceSeeds %d keptSeeds %d verticalSections %d "
                        + "preGlyphs %d beams %d maxHeadSeedDy %d maxHeadInDx %d "
                        + "maxHeadOutDx %d minHeadStumpDy %d stumpAreaDxIn %s "
                        + "stumpAreaDxOut %s stumpAreaDy %s mainStemThickness %d",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                sheet.getScale().getStemThickness(),
                rectangle(system.getBounds()),
                sourceSeeds.size(),
                keptSeeds.size(),
                verticalSections.size(),
                registry.preCount(),
                beams.size(),
                maxHeadSeedDy,
                maxHeadInDx,
                maxHeadOutDx,
                minHeadStumpDy,
                hexDouble(stumpAreaDxIn),
                hexDouble(stumpAreaDxOut),
                hexDouble(stumpAreaDy),
                mainStemThickness), systemHash, pageHash);

        int beamOrdinal = 0;
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (beam.getLinker() != null) {
                throw new IllegalStateException("fresh HEADS beam already has linker");
            }
            final BeamLinker linker = new BeamLinker(beam, retriever);
            final List<Registration> registrations = registry.capture("beam:" + beamOrdinal);
            final boolean tremolo = linker.looksLikeTremolo();
            totals.beams++;
            totals.beamRegs += registrations.size();
            if (tremolo) {
                totals.tremolos++;
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }

            emit(String.format(
                    "stemsheadstumpbeam %s system %d ordinal %d sigOrdinal %d shape %s "
                            + "bounds %s median %s tremolo %s registrations %d",
                    page,
                    system.getId(),
                    beamOrdinal,
                    beamSigOrdinals.get(beam),
                    beam.getShape(),
                    rectangle(beam.getBounds()),
                    line(beam.getMedian()),
                    tremolo,
                    registrations.size()), systemHash, pageHash);
            for (Registration registration : registrations) {
                emitRegistration(
                        "stemsheadstumpbeamreg", page, system, registration, systemHash, pageHash);
            }
            beamOrdinal++;
        }

        final List<Inter> heads = system.getSig().inters(ShapeSet.getTemplateNotesStem(sheet));
        final IdentityHashMap<Inter, Integer> headSigOrdinals = identityInterOrdinals(heads);
        Collections.sort(heads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, heads);
        totals.heads = heads.size();

        final IdentityHashMap<Glyph, Integer> stumpAliases = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, Integer> buildAliases = new IdentityHashMap<>();
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            final Set<Glyph> neighborSet = (Set<Glyph>) GET_NEIGHBORING_SEEDS.invoke(
                    retriever, head.getBounds());
            final List<Glyph> neighbors = new ArrayList<>(neighborSet);
            final int registrationFloor = registry.nextRegistrationOrdinal();

            final HeadLinker linker = new HeadLinker(head, retriever);
            head.setLinker(linker);
            final List<Registration> registrations = registry.capture("head:" + headOrdinal);
            totals.headRegs += registrations.size();

            final List<CornerResult> corners = new ArrayList<>(4);
            int constructorOrdinal = 0;
            for (HorizontalSide horizontal : HorizontalSide.values()) {
                for (VerticalSide vertical : VerticalSide.values()) {
                    final Point2D reference = head.getStemReferencePoint(horizontal, vertical);
                    final Glyph selectedSeed = selectSeed(
                            neighbors,
                            reference,
                            horizontal.direction(),
                            vertical.direction(),
                            maxHeadSeedDy,
                            maxHeadOutDx,
                            maxHeadInDx,
                            minHeadStumpDy);
                    final Glyph returned = linker.getCornerLinker(horizontal, vertical).getStump();
                    final BuildReplay build;
                    final Glyph actualBuilt;
                    if (selectedSeed != null) {
                        if (returned != selectedSeed) {
                            throw new IllegalStateException("production selected a different seed");
                        }
                        build = null;
                        actualBuilt = null;
                    } else {
                        build = replayBuild(
                                system,
                                verticalSections,
                                sectionOrdinals,
                                reference,
                                horizontal.direction(),
                                vertical.direction(),
                                stumpAreaDxIn,
                                stumpAreaDxOut,
                                stumpAreaDy,
                                mainStemThickness,
                                minHeadStumpDy);
                        actualBuilt = (build.candidate != null)
                                ? registry.findEqual(build.candidate) : null;
                        if ((build.candidate != null) && (actualBuilt == null)) {
                            throw new IllegalStateException(
                                    "registered build candidate is missing at " + page + " system "
                                            + system.getId() + " head " + headOrdinal + " corner "
                                            + corner(horizontal, vertical) + " candidate "
                                            + rectangle(build.candidate.getBounds()) + "/"
                                            + build.candidate.getWeight());
                        }
                        final Glyph expected = (build.candidate != null && build.standsOut)
                                ? actualBuilt : null;
                        if (returned != expected) {
                            throw new IllegalStateException("production build result differs from replay");
                        }
                    }
                    corners.add(new CornerResult(
                            constructorOrdinal,
                            horizontal,
                            vertical,
                            reference,
                            selectedSeed,
                            returned,
                            build,
                            actualBuilt));
                    constructorOrdinal++;
                }
            }

            validateHeadRegistrations(corners, registrations, registrationFloor, registry);
            emit(String.format(
                    "stemsheadstumphead %s system %d ordinal %d sigOrdinal %d staff %d "
                            + "shape %s bounds %s neighbors %d registrations %d",
                    page,
                    system.getId(),
                    headOrdinal,
                    headSigOrdinals.get(head),
                    head.getStaff().getId(),
                    head.getShape(),
                    rectangle(head.getBounds()),
                    neighbors.size(),
                    registrations.size()), systemHash, pageHash);

            final IdentityHashMap<Glyph, Boolean> currentNew = new IdentityHashMap<>();
            for (Registration registration : registrations) {
                currentNew.put(registration.glyph, Boolean.TRUE);
            }
            final IdentityHashMap<Glyph, Boolean> firstCurrentUse = new IdentityHashMap<>();
            for (CornerResult corner : corners) {
                emitCorner(
                        page,
                        system,
                        headOrdinal,
                        corner,
                        keptOrdinals,
                        registry,
                        registrationFloor,
                        currentNew,
                        firstCurrentUse,
                        stumpAliases,
                        buildAliases,
                        totals,
                        systemHash,
                        pageHash);
            }
            for (Registration registration : registrations) {
                emitRegistration(
                        "stemsheadstumpheadreg", page, system, registration, systemHash, pageHash);
            }
        }

        if (totals.corners != 4 * totals.heads) {
            throw new IllegalStateException("system corner count mismatch");
        }
        emitSummary("stemsheadstumpsystemsummary", page, system, totals, systemHash.value());
        pageTotals.include(totals);
    }

    private static void emitCorner (String page,
                                    SystemInfo system,
                                    int headOrdinal,
                                    CornerResult corner,
                                    IdentityHashMap<Glyph, Integer> keptOrdinals,
                                    RegistryTracker registry,
                                    int registrationFloor,
                                    IdentityHashMap<Glyph, Boolean> currentNew,
                                    IdentityHashMap<Glyph, Boolean> firstCurrentUse,
                                    IdentityHashMap<Glyph, Integer> stumpAliases,
                                    IdentityHashMap<Glyph, Integer> buildAliases,
                                    Totals totals,
                                    RowHasher... hashes)
    {
        totals.corners++;
        final String kind;
        final String returnedOrigin;
        final String stumpAlias;
        if (corner.selectedSeed != null) {
            totals.seeds++;
            kind = "seed:" + keptOrdinals.get(corner.selectedSeed);
            returnedOrigin = "kept:" + keptOrdinals.get(corner.returned);
            stumpAlias = Integer.toString(alias(stumpAliases, corner.returned));
        } else {
            totals.fallbacks++;
            if (corner.returned == null) {
                kind = "none";
                returnedOrigin = "-";
                stumpAlias = "-";
            } else {
                totals.acceptedBuilds++;
                kind = "built:" + registry.origin(corner.returned).token();
                returnedOrigin = registry.origin(corner.returned).token();
                stumpAlias = Integer.toString(alias(stumpAliases, corner.returned));
            }
        }

        emit(String.format(
                "stemsheadstumpcorner %s system %d head %d ctorOrdinal %d corner %s "
                        + "xDir %d yDir %d ref %s kind %s returnedOrigin %s stumpAlias %s",
                page,
                system.getId(),
                headOrdinal,
                corner.constructorOrdinal,
                corner(corner.horizontal, corner.vertical),
                corner.horizontal.direction(),
                corner.vertical.direction(),
                point(corner.reference),
                kind,
                returnedOrigin,
                stumpAlias), hashes);

        if (corner.build == null) {
            return;
        }
        final BuildReplay build = corner.build;
        totals.sections += build.sections.size();
        totals.steps += build.steps.size();
        if (build.subsection != null) {
            totals.subsections++;
        }
        for (SectionEvidence section : build.sections) {
            emit(String.format(
                    "stemsheadstumpsection %s system %d head %d ctorOrdinal %d "
                            + "sortOrdinal %d inputOrdinal %d sourceOrdinal %d bounds %s "
                            + "weight %d firstPos %d runs %d:%016x areaCenter %d:%d "
                            + "distance %s containsRef %s",
                    page,
                    system.getId(),
                    headOrdinal,
                    corner.constructorOrdinal,
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
                    hexDouble(section.distance),
                    section.containsReference), hashes);
        }
        for (BuildStep step : build.steps) {
            emit(String.format(
                    "stemsheadstumpstep %s system %d head %d ctorOrdinal %d "
                            + "sortOrdinal %d sourceOrdinal %d preMember %s afterAddWidth %d "
                            + "tooWide %s removed %s finalWeight %d finalBounds %s "
                            + "memberSources %s",
                    page,
                    system.getId(),
                    headOrdinal,
                    corner.constructorOrdinal,
                    step.sortOrdinal,
                    step.sourceOrdinal,
                    step.preMember,
                    step.afterAddWidth,
                    step.tooWide,
                    step.removed,
                    step.finalWeight,
                    optionalRectangle(step.finalBounds),
                    step.memberSources), hashes);
        }
        if (build.subsection != null) {
            final SubsectionEvidence subsection = build.subsection;
            emit(String.format(
                    "stemsheadstumpsubsection %s system %d head %d ctorOrdinal %d "
                            + "sourceOrdinal %d stemWidth %d x0 %d i0 %d x1 %d i1 %d "
                            + "bounds %s weight %d firstPos %d runs %d:%016x",
                    page,
                    system.getId(),
                    headOrdinal,
                    corner.constructorOrdinal,
                    subsection.sourceOrdinal,
                    subsection.stemWidth,
                    subsection.x0,
                    subsection.i0,
                    subsection.x1,
                    subsection.i1,
                    optionalRectangle(subsection.section.getWeight() == 0
                            ? null : subsection.section.getBounds()),
                    subsection.section.getWeight(),
                    subsection.section.getFirstPos(),
                    subsection.section.getRunCount(),
                    sectionRunHash(subsection.section)), hashes);
        }

        if (build.candidate == null) {
            totals.emptyBuilds++;
            emit(String.format(
                    "stemsheadstumpbuild %s system %d head %d ctorOrdinal %d area %s "
                            + "sections %d preAdded %s compoundWeight %d compoundBounds %s "
                            + "candidate none registration none returned none",
                    page,
                    system.getId(),
                    headOrdinal,
                    corner.constructorOrdinal,
                    rectangle2D(build.area),
                    build.sections.size(),
                    optionalInteger(build.preAddedSourceOrdinal),
                    build.compoundWeight,
                    optionalRectangle(build.compoundBounds)), hashes);
            return;
        }

        totals.buildCandidates++;
        if (!build.standsOut) {
            totals.rejectedBuilds++;
        }
        final Glyph actual = corner.actualBuilt;
        final Origin origin = registry.origin(actual);
        final boolean newInHead = currentNew.containsKey(actual);
        final boolean first = firstCurrentUse.put(actual, Boolean.TRUE) == null;
        final String event;
        if (newInHead && first && (origin.registrationOrdinal >= registrationFloor)) {
            event = "new:" + origin.registrationOrdinal;
            totals.newBuilds++;
        } else {
            event = "reuse:" + origin.token();
            totals.reusedBuilds++;
        }
        emit(String.format(
                "stemsheadstumpbuild %s system %d head %d ctorOrdinal %d area %s "
                        + "sections %d preAdded %s compoundWeight %d compoundBounds %s "
                        + "candidate bounds:%s,weight:%d,runs:%d:%016x refY %d extDy %d "
                        + "standsOut %s buildAlias %d origin %s registration %s returned %s",
                page,
                system.getId(),
                headOrdinal,
                corner.constructorOrdinal,
                rectangle2D(build.area),
                build.sections.size(),
                optionalInteger(build.preAddedSourceOrdinal),
                build.compoundWeight,
                optionalRectangle(build.compoundBounds),
                rectangle(build.candidate.getBounds()),
                build.candidate.getWeight(),
                runCount(build.candidate),
                runTableHash(build.candidate),
                build.refY,
                build.extDy,
                build.standsOut,
                alias(buildAliases, actual),
                origin.token(),
                event,
                corner.returned != null ? "built" : "none"), hashes);
    }

    private static BuildReplay replayBuild (SystemInfo system,
                                            List<Section> verticalSections,
                                            IdentityHashMap<Section, Integer> sectionOrdinals,
                                            Point2D reference,
                                            int xDir,
                                            int yDir,
                                            double stumpAreaDxIn,
                                            double stumpAreaDxOut,
                                            double stumpAreaDy,
                                            int mainStemThickness,
                                            int minHeadStumpDy)
    {
        final double left = (xDir > 0) ? reference.getX() - stumpAreaDxIn
                : reference.getX() - stumpAreaDxOut;
        final double right = (xDir > 0) ? reference.getX() + stumpAreaDxOut
                : reference.getX() + stumpAreaDxIn;
        final double top = (yDir > 0) ? reference.getY() : reference.getY() - stumpAreaDy;
        final Rectangle2D areaBox = new Rectangle2D.Double(
                left, top, right - left, stumpAreaDy);
        final List<Section> intersected = new ArrayList<>(Sections.intersectedSections(
                new Area(areaBox), verticalSections));
        final IdentityHashMap<Section, Integer> inputOrdinals = sectionOrdinals(intersected);
        Collections.sort(intersected, Comparator.comparingDouble(
                section -> Math.abs(section.getAreaCenter().getX() - reference.getX())));

        final Point pixelReference = new Point((int) reference.getX(), (int) reference.getY());
        final List<SectionEvidence> evidence = new ArrayList<>(intersected.size());
        for (int sortOrdinal = 0; sortOrdinal < intersected.size(); sortOrdinal++) {
            final Section section = intersected.get(sortOrdinal);
            evidence.add(new SectionEvidence(
                    section,
                    sortOrdinal,
                    inputOrdinals.get(section),
                    requiredOrdinal(sectionOrdinals, section),
                    Math.abs(section.getAreaCenter().getX() - reference.getX()),
                    section.contains(pixelReference)));
        }

        if (intersected.isEmpty()) {
            return new BuildReplay(
                    areaBox, evidence, List.of(), -1, null, 0, null, null, 0, 0, false);
        }

        final SectionCompound compound = new SectionCompound();
        int preAdded = -1;
        for (Section section : intersected) {
            if (section.contains(pixelReference)) {
                compound.addSection(section);
                preAdded = requiredOrdinal(sectionOrdinals, section);
                break;
            }
        }

        final List<BuildStep> steps = new ArrayList<>(intersected.size());
        for (int sortOrdinal = 0; sortOrdinal < intersected.size(); sortOrdinal++) {
            final Section section = intersected.get(sortOrdinal);
            final boolean preMember = compound.getMembers().contains(section);
            compound.addSection(section);
            final int afterAddWidth = compound.getWidth();
            final boolean tooWide = afterAddWidth > mainStemThickness;
            final boolean removed = tooWide && compound.removeSection(section);
            final int finalWeight = compound.getWeight();
            final Rectangle finalBounds = finalWeight == 0 ? null : compound.getBounds();
            steps.add(new BuildStep(
                    sortOrdinal,
                    requiredOrdinal(sectionOrdinals, section),
                    preMember,
                    afterAddWidth,
                    tooWide,
                    removed,
                    finalWeight,
                    finalBounds,
                    memberSources(compound, sectionOrdinals)));
        }

        SubsectionEvidence subsection = null;
        if (compound.getWeight() == 0) {
            final Section wide = intersected.get(0);
            final int stemWidth = system.getSheet().getScale().getStemThickness();
            final int x0 = (int) Math.rint(reference.getX() - stemWidth / 2.0);
            final int i0 = Math.max(0, x0 - wide.getFirstPos());
            final int x1 = x0 + stemWidth;
            final int i1 = Math.min(x1 - wide.getFirstPos(), wide.getRunCount());
            final DynamicSection thin = new DynamicSection(Orientation.VERTICAL);
            if (i1 > i0) {
                thin.setFirstPos(x0);
                for (Run run : wide.getRuns().subList(i0, i1)) {
                    thin.append(new Run(run));
                }
            }
            subsection = new SubsectionEvidence(
                    requiredOrdinal(sectionOrdinals, wide), stemWidth, x0, i0, x1, i1, thin);
            if (thin.getWeight() != 0) {
                compound.addSection(thin);
            }
        }

        final int compoundWeight = compound.getWeight();
        final Rectangle compoundBounds = compoundWeight == 0 ? null : compound.getBounds();
        if (compoundWeight == 0) {
            return new BuildReplay(
                    areaBox,
                    evidence,
                    steps,
                    preAdded,
                    subsection,
                    compoundWeight,
                    compoundBounds,
                    null,
                    0,
                    0,
                    false);
        }

        final Glyph candidate = compound.toGlyph(GlyphGroup.STUMP);
        final int refY = (int) Math.rint(reference.getY());
        final Rectangle box = candidate.getBounds();
        final int extDy = (yDir > 0)
                ? box.y + box.height - 1 - refY : refY - box.y;
        return new BuildReplay(
                areaBox,
                evidence,
                steps,
                preAdded,
                subsection,
                compoundWeight,
                compoundBounds,
                candidate,
                refY,
                extDy,
                extDy >= minHeadStumpDy);
    }

    private static Glyph selectSeed (List<Glyph> neighbors,
                                     Point2D reference,
                                     int xDir,
                                     int yDir,
                                     int maxHeadSeedDy,
                                     int maxHeadOutDx,
                                     int maxHeadInDx,
                                     int minHeadStumpDy)
    {
        final Point2D out = new Point2D.Double(
                reference.getX() + xDir * maxHeadOutDx, reference.getY());
        final Point2D in = new Point2D.Double(
                reference.getX() - xDir * maxHeadInDx, reference.getY());
        final Point2D left = xDir > 0 ? in : out;
        final Point2D right = xDir > 0 ? out : in;
        final Rectangle2D seedRect = new Rectangle2D.Double(
                left.getX(),
                left.getY() - maxHeadSeedDy,
                right.getX() - left.getX(),
                2.0 * maxHeadSeedDy);
        final List<Glyph> seeds = new ArrayList<>(Glyphs.intersectedGlyphs(
                neighbors, new Area(seedRect)));
        if (seeds.size() > 1) {
            Collections.sort(seeds, Comparator.comparingDouble(
                    glyph -> requireCenterLine(glyph).ptSegDistSq(reference)));
        }
        for (Glyph seed : seeds) {
            final double seedX = LineUtil.xAtY(requireCenterLine(seed), reference.getY());
            final int dx = (int) Math.round(xDir * (seedX - reference.getX()));
            if (((dx >= 0) && (dx <= maxHeadOutDx))
                    || ((dx <= 0) && (-dx <= maxHeadInDx))) {
                final Rectangle box = seed.getBounds();
                final int refY = (int) Math.rint(reference.getY());
                final int extDy = (yDir > 0)
                        ? box.y + box.height - 1 - refY : refY - box.y;
                if (extDy >= minHeadStumpDy) {
                    return seed;
                }
            }
        }
        return null;
    }

    private static void validateHeadRegistrations (List<CornerResult> corners,
                                                   List<Registration> registrations,
                                                   int registrationFloor,
                                                   RegistryTracker registry)
    {
        final List<Glyph> expected = new ArrayList<>();
        final IdentityHashMap<Glyph, Boolean> seen = new IdentityHashMap<>();
        for (CornerResult corner : corners) {
            if (corner.actualBuilt == null) {
                continue;
            }
            final Origin origin = registry.origin(corner.actualBuilt);
            if ((origin.registrationOrdinal >= registrationFloor)
                    && (seen.put(corner.actualBuilt, Boolean.TRUE) == null)) {
                expected.add(corner.actualBuilt);
            }
        }
        if (expected.size() != registrations.size()) {
            throw new IllegalStateException("head registration count differs from replay");
        }
        for (int index = 0; index < expected.size(); index++) {
            if (expected.get(index) != registrations.get(index).glyph) {
                throw new IllegalStateException("head registration order differs from replay");
            }
        }
    }

    private static void emitRegistration (String prefix,
                                          String page,
                                          SystemInfo system,
                                          Registration registration,
                                          RowHasher... hashes)
    {
        final Glyph glyph = registration.glyph;
        emit(String.format(
                "%s %s system %d ordinal %d phase %s phaseOrdinal %d bounds %s "
                        + "weight %d runs %d:%016x",
                prefix,
                page,
                system.getId(),
                registration.origin.registrationOrdinal,
                registration.origin.phase,
                registration.origin.phaseOrdinal,
                rectangle(glyph.getBounds()),
                glyph.getWeight(),
                runCount(glyph),
                runTableHash(glyph)), hashes);
    }

    private static void emitSummary (String prefix,
                                     String page,
                                     SystemInfo system,
                                     Totals totals,
                                     long hash)
    {
        System.out.printf(
                "%s %s system %d beams %d tremolos %d beamRegs %d heads %d corners %d "
                        + "seeds %d fallbacks %d emptyBuilds %d buildCandidates %d "
                        + "acceptedBuilds %d rejectedBuilds %d headRegs %d newBuilds %d "
                        + "reusedBuilds %d sections %d steps %d subsections %d hash %016x%n",
                prefix,
                page,
                system.getId(),
                totals.beams,
                totals.tremolos,
                totals.beamRegs,
                totals.heads,
                totals.corners,
                totals.seeds,
                totals.fallbacks,
                totals.emptyBuilds,
                totals.buildCandidates,
                totals.acceptedBuilds,
                totals.rejectedBuilds,
                totals.headRegs,
                totals.newBuilds,
                totals.reusedBuilds,
                totals.sections,
                totals.steps,
                totals.subsections,
                hash);
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

    private static String memberSources (SectionCompound compound,
                                         IdentityHashMap<Section, Integer> sectionOrdinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Section member : compound.getMembers()) {
            if (builder.length() != 0) {
                builder.append(',');
            }
            final Integer ordinal = sectionOrdinals.get(member);
            builder.append(ordinal != null ? ordinal : "sub");
        }
        return builder.length() == 0 ? "-" : builder.toString();
    }

    private static int requiredOrdinal (IdentityHashMap<Section, Integer> ordinals,
                                        Section section)
    {
        final Integer ordinal = ordinals.get(section);
        if (ordinal == null) {
            throw new IllegalStateException("section is absent from vertical source pool");
        }
        return ordinal;
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

    private static IdentityHashMap<Inter, Integer> identityInterOrdinals (List<Inter> inters)
    {
        final IdentityHashMap<Inter, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < inters.size(); ordinal++) {
            if (ordinals.put(inters.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate inter identity in ordered pool");
            }
        }
        return ordinals;
    }

    private static int alias (IdentityHashMap<Glyph, Integer> aliases,
                              Glyph glyph)
    {
        final Integer previous = aliases.get(glyph);
        if (previous != null) {
            return previous;
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

    private static Line2D requireCenterLine (Glyph glyph)
    {
        final Line2D line = glyph.getCenterLine();
        if (line == null) {
            throw new IllegalStateException("vertical seed has no center line");
        }
        return line;
    }

    private static String corner (HorizontalSide horizontal,
                                  VerticalSide vertical)
    {
        return "" + vertical.name().charAt(0) + horizontal.name().charAt(0);
    }

    private static String optionalInteger (int value)
    {
        return value >= 0 ? Integer.toString(value) : "-";
    }

    private static String rectangle (Rectangle box)
    {
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String optionalRectangle (Rectangle box)
    {
        return box != null ? rectangle(box) : "-";
    }

    private static String rectangle2D (Rectangle2D box)
    {
        return hexDouble(box.getX()) + ":" + hexDouble(box.getY()) + ":"
                + hexDouble(box.getWidth()) + ":" + hexDouble(box.getHeight());
    }

    private static String point (Point2D point)
    {
        return hexDouble(point.getX()) + ":" + hexDouble(point.getY());
    }

    private static String line (Line2D line)
    {
        return point(line.getP1()) + ":" + point(line.getP2());
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
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS head-stump oracle.");
        System.out.println("#");
        System.out.println("# Every target starts a fresh JVM and reaches real HEADS. The probe then");
        System.out.println("# executes the exact constructor prefix of StemsRetriever.inspectStems:");
        System.out.println("# purged seeds, stable-x BeamLinkers with tremolo removal, then stable-x");
        System.out.println("# HeadLinkers. No VLinker or CLinker inspection is performed.");
        System.out.println("#");
        System.out.println("# Head corners use constructor order TL, BL, TR, BR. seed/built/none is");
        System.out.println("# validated by object identity. Independent build rows freeze the stump area,");
        System.out.println("# lag input and stable distance order, pre-add, every width-gated compound step,");
        System.out.println("# subsection extraction, fixed glyph pixels, stands-out gate, registration/reuse");
        System.out.println("# ordinal, and returned/rejected result. Rejected glyph registrations remain visible.");
        System.out.println("#");
        System.out.println("# pre:N and reg:N are page-system-local boundary ordinals, never Java IDs.");
        System.out.println("# stumpAlias and buildAlias expose identity aliasing in first-corner order.");
        System.out.println("# The runner uses Epsilon GC so rejected weak-only originals remain observable");
        System.out.println("# and cannot introduce collector-timing-dependent registration/reuse rows.");
        System.out.println("# Doubles include hex text/raw bits. FNV-1a-64 hashes cover canonical UTF-8");
        System.out.println("# semantic rows with trailing newlines; the runner adds SHA-256 commitments.");
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
                            phase,
                            registrations.size(),
                            -1,
                            nextRegistrationOrdinal++);
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
                    if ((match != null) && (match != glyph)) {
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

    private record CornerResult(
            int constructorOrdinal,
            HorizontalSide horizontal,
            VerticalSide vertical,
            Point2D reference,
            Glyph selectedSeed,
            Glyph returned,
            BuildReplay build,
            Glyph actualBuilt)
    {
    }

    private record SectionEvidence(
            Section section,
            int sortOrdinal,
            int inputOrdinal,
            int sourceOrdinal,
            double distance,
            boolean containsReference)
    {
    }

    private record BuildStep(
            int sortOrdinal,
            int sourceOrdinal,
            boolean preMember,
            int afterAddWidth,
            boolean tooWide,
            boolean removed,
            int finalWeight,
            Rectangle finalBounds,
            String memberSources)
    {
    }

    private record SubsectionEvidence(
            int sourceOrdinal,
            int stemWidth,
            int x0,
            int i0,
            int x1,
            int i1,
            Section section)
    {
    }

    private record BuildReplay(
            Rectangle2D area,
            List<SectionEvidence> sections,
            List<BuildStep> steps,
            int preAddedSourceOrdinal,
            SubsectionEvidence subsection,
            int compoundWeight,
            Rectangle compoundBounds,
            Glyph candidate,
            int refY,
            int extDy,
            boolean standsOut)
    {
    }

    private static final class Totals
    {
        int beams;

        int tremolos;

        int beamRegs;

        int heads;

        int corners;

        int seeds;

        int fallbacks;

        int emptyBuilds;

        int buildCandidates;

        int acceptedBuilds;

        int rejectedBuilds;

        int headRegs;

        int newBuilds;

        int reusedBuilds;

        int sections;

        int steps;

        int subsections;

        void include (Totals other)
        {
            beams += other.beams;
            tremolos += other.tremolos;
            beamRegs += other.beamRegs;
            heads += other.heads;
            corners += other.corners;
            seeds += other.seeds;
            fallbacks += other.fallbacks;
            emptyBuilds += other.emptyBuilds;
            buildCandidates += other.buildCandidates;
            acceptedBuilds += other.acceptedBuilds;
            rejectedBuilds += other.rejectedBuilds;
            headRegs += other.headRegs;
            newBuilds += other.newBuilds;
            reusedBuilds += other.reusedBuilds;
            sections += other.sections;
            steps += other.steps;
            subsections += other.subsections;
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
