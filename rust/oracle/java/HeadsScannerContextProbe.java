// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
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
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.image.ChamferDistance;
import org.audiveris.omr.image.DistanceTable;
import org.audiveris.omr.image.Anchored.Anchor;
import static org.audiveris.omr.image.Anchored.Anchor.LEFT_STEM;
import static org.audiveris.omr.image.Anchored.Anchor.MIDDLE_LEFT;
import static org.audiveris.omr.image.Anchored.Anchor.RIGHT_STEM;
import org.audiveris.omr.image.PixelDistance;
import org.audiveris.omr.image.Template;
import org.audiveris.omr.image.TemplateFactory;
import org.audiveris.omr.image.TemplateFactory.Catalog;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.math.PointUtil;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Part;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.LineInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sheet.note.DistancesBuilder;
import org.audiveris.omr.sheet.note.HeadSeedTally;
import org.audiveris.omr.sheet.note.HeadSpotsBuilder;
import org.audiveris.omr.sheet.note.NoteHeadsBuilder;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.AbstractHorizontalInter;
import org.audiveris.omr.sig.inter.AbstractVerticalInter;
import org.audiveris.omr.sig.inter.BarConnectorInter;
import org.audiveris.omr.sig.inter.BarlineInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.LedgerInter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;
import org.audiveris.omr.util.ByteUtil;
import org.audiveris.omr.util.HorizontalSide;

import ij.process.ByteProcessor;

/**
 * Exact oracle for the immutable construction context of NoteHeadsBuilder.Scanner.
 *
 * <p>Each target reaches the real LEDGERS step, then runs the real HEADS prolog. The probe prepares
 * a NoteHeadsBuilder exactly through the immutable system fields at the start of buildHeads, but it
 * never invokes Scanner.lookup. It reflectively constructs seed-mode and range-mode scanners in
 * processStaff order and proves their static geometry agrees before recording it once. No head is
 * created, no SIG vertex is added, and no head/seed tally is mutated.
 *
 * <p>The default mode deliberately stops before competitor and frozen-bar slicing. It freezes the
 * schedule, scale parameters, catalog selection, offsets, staff/ledger source identities, farther
 * ledger axes, and complete theoretical-ordinate/range vectors. The optional {@code --slices}
 * mode freezes the pre-lookup competitor/bar candidate decisions, semantic bands, and exact
 * seed/spot/competitor/bar rectangle slices without mutating the SIG. The vectors use run-length
 * encoding so every integer ordinate remains recoverable without a full-width record per pixel.
 */
public class HeadsScannerContextProbe
{
    private static final Class<?> BUILDER_CLASS = NoteHeadsBuilder.class;

    private static final Class<?> STAFF_LINE_ADAPTER_CLASS = nested("StaffLineAdapter");

    private static final Class<?> LEDGER_ADAPTER_CLASS = nested("LedgerAdapter");

    private static final Class<?> SCANNER_CLASS = nested("Scanner");

    private HeadsScannerContextProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            System.exit(0);
        }

        if ((args.length == 1) && args[0].equals("--slices-header")) {
            printSlicesHeader();
            System.exit(0);
        }

        if ((args.length == 1) && args[0].equals("--seed-heads-header")) {
            printSeedHeadsHeader();
            System.exit(0);
        }

        if ((args.length == 1) && args[0].equals("--range-heads-header")) {
            printRangeHeadsHeader();
            System.exit(0);
        }

        final boolean slices = (args.length == 2) && args[0].equals("--slices");
        final boolean seedHeads = (args.length == 2) && args[0].equals("--seed-heads");
        final boolean rangeHeads = (args.length == 2) && args[0].equals("--range-heads");
        final boolean rangeHeadsFull = (args.length == 2)
                && args[0].equals("--range-heads-full");
        if ((!slices && !seedHeads && !rangeHeads && !rangeHeadsFull && (args.length != 1))
                || ((slices || seedHeads || rangeHeads || rangeHeadsFull)
                        && (args.length != 2))) {
            throw new IllegalArgumentException(
                    "expected [--slices|--seed-heads|--range-heads|--range-heads-full] "
                            + "<path>:<sheet> target");
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "LEDGERS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        MusicFont.checkMusicFont();

        final String[] parts = args[
                (slices || seedHeads || rangeHeads || rangeHeadsFull) ? 1 : 0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        final Path path = Paths.get(parts[0]).toAbsolutePath();
        final int wanted = Integer.parseInt(parts[1]);
        if (rangeHeads || rangeHeadsFull) {
            runRangeHeadsPage(path, wanted, rangeHeadsFull);
        } else if (seedHeads) {
            runSeedHeadsPage(path, wanted);
        } else if (slices) {
            runSlicesPage(path, wanted);
        } else {
            runPage(path, wanted);
        }
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final Scale scale = sheet.getScale();
        final String page = path.getFileName() + "#" + wanted;
        final MusicFamily family = sheet.getStub().getMusicFamily();

        // Exact HeadsStep.doProlog products, in production order.
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        final Map<SystemInfo, List<Glyph>> sheetSpots = new HeadSpotsBuilder(sheet).getSpots();

        System.out.printf(
                "headscannerpage %s width %d height %d systems %d staves %d family %s%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                family);

        final List<String> pageRows = new ArrayList<>();
        int pageStandardStaves = 0;
        int pageGeometries = 0;
        int pageSchedules = 0;

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final NoteHeadsBuilder builder = new NoteHeadsBuilder(
                    system,
                    distances,
                    systemSpots,
                    new HeadSeedTally(),
                    false,
                    new java.util.TreeMap<>());
            prepareBuilder(builder, system, systemSpots);

            final String paramsRow = parameterRow(page, system, builder, scale);
            pageRows.add(paramsRow);
            System.out.println(paramsRow);

            int systemGeometries = 0;
            int systemSchedules = 0;
            final List<String> systemRows = new ArrayList<>();
            for (Staff staff : system.getStaves()) {
                final String staffRow = staffRow(page, system, staff, family);
                pageRows.add(staffRow);
                systemRows.add(staffRow);
                System.out.println(staffRow);
                if (staff.isTablature()) {
                    continue;
                }
                pageStandardStaves++;

                // Force the real family/point-size catalog lookup made by buildHeads.
                final int pointSize = staff.getHeadPointSize();
                if (TemplateFactory.getInstance().getCatalog(family, pointSize) == null) {
                    throw new IllegalStateException("missing catalog " + family + "/" + pointSize);
                }

                final List<Geometry> geometries = buildGeometries(builder, staff);
                final List<String> staffRows = new ArrayList<>();
                for (int ordinal = 0; ordinal < geometries.size(); ordinal++) {
                    final String row = geometryRow(page, system, staff, ordinal, geometries.get(ordinal));
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    System.out.println(row);
                }
                pageGeometries += geometries.size();
                systemGeometries += geometries.size();

                for (String phase : new String[] { "seed", "range" }) {
                    for (int ordinal = 0; ordinal < geometries.size(); ordinal++) {
                        final String row = String.format(
                                "headscannerschedule %s system %d staff %d phase %s ordinal %d "
                                        + "geometry %d",
                                page,
                                system.getId(),
                                staff.getId(),
                                phase,
                                ordinal,
                                ordinal);
                        pageRows.add(row);
                        systemRows.add(row);
                        staffRows.add(row);
                        System.out.println(row);
                        pageSchedules++;
                        systemSchedules++;
                    }
                }

                final String summary = String.format(
                        "headscannerstaffsummary %s system %d staff %d geometries %d schedules %d "
                                + "%016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        geometries.size(),
                        2 * geometries.size(),
                        hash(staffRows));
                pageRows.add(summary);
                systemRows.add(summary);
                System.out.println(summary);
            }

            final String summary = String.format(
                    "headscannersystemsummary %s system %d staves %d geometries %d schedules %d "
                            + "%016x",
                    page,
                    system.getId(),
                    system.getStaves().size(),
                    systemGeometries,
                    systemSchedules,
                    hash(systemRows));
            pageRows.add(summary);
            System.out.println(summary);
        }

        System.out.printf(
                "headscannerpagesummary %s standardStaves %d geometries %d schedules %d %016x%n",
                page,
                pageStandardStaves,
                pageGeometries,
                pageSchedules,
                hash(pageRows));
    }

    /**
     * Run only the real seed-based half of {@code NoteHeadsBuilder.buildHeads}.
     *
     * <p>Each staff invokes the private production {@code processStaff(staff, true)} method.
     * Returned heads are then appended to the live competitor pool and stably sorted by ordinate,
     * exactly as in {@code buildHeads}. Range lookup and every later purge are intentionally absent.
     */
    @SuppressWarnings("unchecked")
    private static void runSeedHeadsPage (Path path,
                                          int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final MusicFamily family = sheet.getStub().getMusicFamily();
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        final Map<SystemInfo, List<Glyph>> sheetSpots = new HeadSpotsBuilder(sheet).getSpots();
        final ByteProcessor image = sheet.getPicture().getSource(Picture.SourceKey.BINARY);
        final List<String> pageRows = new ArrayList<>();
        int pageStaffs = 0;
        int pageAttempts = 0;
        int pageHeads = 0;

        System.out.printf(
                "headseedpage %s width %d height %d systems %d staves %d family %s%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                family);

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final HeadSeedTally tally = new HeadSeedTally();
            final NoteHeadsBuilder builder = new NoteHeadsBuilder(
                    system,
                    distances,
                    systemSpots,
                    tally,
                    false,
                    new java.util.TreeMap<>());
            prepareBuilder(builder, system, systemSpots);
            setField(builder, "image", image);

            final List<Glyph> systemSeeds = (List<Glyph>) field(builder, "systemSeeds");
            final List<Inter> systemCompetitors = (List<Inter>) field(builder, "systemCompetitors");
            final List<String> systemRows = new ArrayList<>();
            final int initialSigCount = system.getSig().vertexSet().size();
            final int initialCompetitorCount = systemCompetitors.size();
            int systemAttempts = 0;
            int systemHeads = 0;

            for (Staff staff : system.getStaves()) {
                if (staff.isTablature()) {
                    continue;
                }
                pageStaffs++;
                final Catalog catalog = TemplateFactory.getInstance().getCatalog(
                        family,
                        staff.getHeadPointSize());
                if (catalog == null) {
                    throw new IllegalStateException(
                            "missing catalog " + family + "/" + staff.getHeadPointSize());
                }
                setField(builder, "catalog", catalog);

                // Trace the same private eval/createInter operations against the live pre-staff
                // state, but restore the debug counters before invoking production processStaff.
                final List<Geometry> geometries = buildGeometries(builder, staff);
                final PerfCounts perfBeforeTrace = readSeedPerf(builder);
                final List<SeedAttempt> attempts = traceSeedAttempts(
                        builder,
                        systemSeeds,
                        geometries,
                        catalog,
                        image);
                final PerfCounts perfAfterTrace = readSeedPerf(builder);
                final PerfCounts tracedCalls = perfAfterTrace.minus(perfBeforeTrace);
                restoreSeedPerf(builder, perfBeforeTrace);

                final List<Inter> sigBefore = new ArrayList<>(system.getSig().vertexSet());
                final int competitorsBefore = systemCompetitors.size();
                final PerfCounts perfBefore = readSeedPerf(builder);
                final List<HeadInter> heads = processSeedStaff(builder, staff);
                final PerfCounts perfAfter = readSeedPerf(builder);
                final PerfCounts perfDelta = perfAfter.minus(perfBefore);
                final List<Inter> sigAfter = new ArrayList<>(system.getSig().vertexSet());

                final int predictedAbandons = (int) attempts.stream()
                        .filter(attempt -> attempt.outcome.startsWith("abandoned-"))
                        .count();
                if ((tracedCalls.bars != perfDelta.bars)
                        || (tracedCalls.overlaps != perfDelta.overlaps)
                        || (tracedCalls.evals != perfDelta.evals)
                        || (predictedAbandons != perfDelta.abandons)) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d traced perf %s/%d abandons != production %s",
                            page,
                            system.getId(),
                            staff.getId(),
                            tracedCalls,
                            predictedAbandons,
                            perfDelta));
                }

                final List<SeedAttempt> finals = attempts.stream()
                        .filter(attempt -> attempt.outcome.equals("final"))
                        .toList();
                if (finals.size() != heads.size()) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d predicted %d seed heads but production made %d",
                            page,
                            system.getId(),
                            staff.getId(),
                            finals.size(),
                            heads.size()));
                }
                if (sigAfter.size() != sigBefore.size() + heads.size()) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d SIG grew %d, expected %d",
                            page,
                            system.getId(),
                            staff.getId(),
                            sigAfter.size() - sigBefore.size(),
                            heads.size()));
                }

                // This mutation is deliberately before recording final competitor ordinals and
                // before moving to the next staff, exactly as in buildHeads.
                systemCompetitors.addAll(heads);
                Collections.sort(systemCompetitors, Inters.byOrdinate);

                final List<String> staffRows = new ArrayList<>();
                final List<String> attemptRows = new ArrayList<>();
                final List<String> kernelRows = new ArrayList<>();
                final List<String> candidateRows = new ArrayList<>();
                final List<String> headRows = new ArrayList<>();
                for (int ordinal = 0; ordinal < attempts.size(); ordinal++) {
                    final SeedAttempt attempt = attempts.get(ordinal);
                    attempt.ordinal = ordinal;
                    final String row = seedAttemptRow(page, system, staff, attempt);
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    attemptRows.add(row);
                    kernelRows.add(seedKernelRecord(system, staff, attempt));
                    System.out.println(row);
                }

                int candidateOrdinal = 0;
                for (SeedAttempt attempt : attempts) {
                    if (!attempt.minGradePass) {
                        continue;
                    }
                    attempt.candidateOrdinal = candidateOrdinal;
                    final String row = seedCandidateRow(
                            page,
                            system,
                            staff,
                            candidateOrdinal++,
                            attempt);
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    candidateRows.add(row);
                    System.out.println(row);
                }

                for (int ordinal = 0; ordinal < heads.size(); ordinal++) {
                    final HeadInter head = heads.get(ordinal);
                    final SeedAttempt attempt = finals.get(ordinal);
                    assertFinalHeadMatches(page, system, staff, head, attempt);
                    final String row = seedHeadRow(
                            page,
                            system,
                            staff,
                            ordinal,
                            head,
                            attempt,
                            sigAfter,
                            systemCompetitors,
                            tally);
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    headRows.add(row);
                    System.out.println(row);
                }

                final String summary = String.format(
                        "headseedstaffsummary %s system %d staff %d scanners %d seeds %d "
                                + "attempts %d candidates %d heads %d sigBefore %d sigAfter %d competitorsBefore "
                                + "%d competitorsAfter %d outcomes %d:%d:%d:%d:%d:%d:%d "
                                + "perf %d:%d:%d:%d attemptHash %016x kernelHash %016x candidateHash %016x "
                                + "headHash %016x hash %016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        geometries.size(),
                        systemSeeds.size(),
                        attempts.size(),
                        candidateRows.size(),
                        heads.size(),
                        sigBefore.size(),
                        sigAfter.size(),
                        competitorsBefore,
                        systemCompetitors.size(),
                        outcomeCount(attempts, "abandoned-bar"),
                        outcomeCount(attempts, "abandoned-competitor"),
                        outcomeCount(attempts, "abandoned-nominal"),
                        outcomeCount(attempts, "no-best"),
                        outcomeCount(attempts, "min-grade"),
                        outcomeCount(attempts, "glyph"),
                        outcomeCount(attempts, "final"),
                        perfDelta.bars,
                        perfDelta.overlaps,
                        perfDelta.evals,
                        perfDelta.abandons,
                        hash(attemptRows),
                        hash(kernelRows),
                        hash(candidateRows),
                        hash(headRows),
                        hash(staffRows));
                pageRows.add(summary);
                systemRows.add(summary);
                System.out.println(summary);

                pageAttempts += attempts.size();
                pageHeads += heads.size();
                systemAttempts += attempts.size();
                systemHeads += heads.size();
            }

            final PerfCounts systemPerf = readSeedPerf(builder);
            final String summary = String.format(
                    "headseedsystemsummary %s system %d staffs %d attempts %d heads %d "
                            + "sigBefore %d sigAfter %d competitorsBefore %d competitorsAfter %d "
                            + "perf %d:%d:%d:%d %016x",
                    page,
                    system.getId(),
                    system.getStaves().stream().filter(staff -> !staff.isTablature()).count(),
                    systemAttempts,
                    systemHeads,
                    initialSigCount,
                    system.getSig().vertexSet().size(),
                    initialCompetitorCount,
                    systemCompetitors.size(),
                    systemPerf.bars,
                    systemPerf.overlaps,
                    systemPerf.evals,
                    systemPerf.abandons,
                    hash(systemRows));
            pageRows.add(summary);
            System.out.println(summary);
        }

        System.out.printf(
                "headseedpagesummary %s staffs %d attempts %d heads %d %016x%n",
                page,
                pageStaffs,
                pageAttempts,
                pageHeads,
                hash(pageRows));
    }

    /**
     * Run the real seed and range halves of {@code NoteHeadsBuilder.buildHeads}, stopping before
     * the staff duplicate and overlap purges.
     *
     * <p>The range algorithm is replayed against identical pre-range state solely to expose its
     * otherwise-private decisions. Its performance counters are restored before production
     * {@code processStaff(staff, false)} runs, and the replayed results are checked against the
     * production heads, glyphs, counter deltas, and SIG insertion order.
     */
    @SuppressWarnings("unchecked")
    private static void runRangeHeadsPage (Path path,
                                           int wanted,
                                           boolean fullTrace)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final MusicFamily family = sheet.getStub().getMusicFamily();
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        final Map<SystemInfo, List<Glyph>> sheetSpots = new HeadSpotsBuilder(sheet).getSpots();
        final ByteProcessor image = sheet.getPicture().getSource(Picture.SourceKey.BINARY);
        final RowHasher pageHash = new RowHasher();
        int pageStaffs = 0;
        int pageSpots = 0;
        int pageScans = 0;
        int pageAttempts = 0;
        int pageRawCandidates = 0;
        int pageCandidates = 0;
        int pageSeedHeads = 0;
        int pageRangeHeads = 0;

        System.out.printf(
                "headrangepage %s width %d height %d systems %d staves %d family %s%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                family);

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final HeadSeedTally tally = new HeadSeedTally();
            final NoteHeadsBuilder builder = new NoteHeadsBuilder(
                    system,
                    distances,
                    systemSpots,
                    tally,
                    false,
                    new java.util.TreeMap<>());
            prepareBuilder(builder, system, systemSpots);
            setField(builder, "image", image);

            final List<Inter> systemCompetitors = (List<Inter>) field(
                    builder, "systemCompetitors");
            final RowHasher systemHash = new RowHasher();
            final int initialSigCount = system.getSig().vertexSet().size();
            final int initialCompetitorCount = systemCompetitors.size();
            int systemSpotSlices = 0;
            int systemScans = 0;
            int systemAttempts = 0;
            int systemRawCandidates = 0;
            int systemCandidates = 0;
            int systemSeedHeads = 0;
            int systemRangeHeads = 0;

            for (Staff staff : system.getStaves()) {
                if (staff.isTablature()) {
                    continue;
                }
                pageStaffs++;
                final Catalog catalog = TemplateFactory.getInstance().getCatalog(
                        family,
                        staff.getHeadPointSize());
                if (catalog == null) {
                    throw new IllegalStateException(
                            "missing catalog " + family + "/" + staff.getHeadPointSize());
                }
                setField(builder, "catalog", catalog);

                final RowHasher staffHash = new RowHasher();
                final RowHasher seedHash = new RowHasher();
                final RowHasher spotHash = new RowHasher();
                final RowHasher spotKernelHash = new RowHasher();
                final RowHasher scanHash = new RowHasher();
                final RowHasher scanKernelHash = new RowHasher();
                final RowHasher attemptHash = new RowHasher();
                final RowHasher attemptKernelHash = new RowHasher();
                final RowHasher rawCandidateHash = new RowHasher();
                final RowHasher rawKernelHash = new RowHasher();
                final RowHasher candidateHash = new RowHasher();
                final RowHasher headHash = new RowHasher();
                final List<Inter> sigBefore = new ArrayList<>(system.getSig().vertexSet());
                final int competitorsBefore = systemCompetitors.size();

                // Exact first half of buildHeads for this staff.
                final List<HeadInter> seedHeads = processSeedStaff(builder, staff);
                systemCompetitors.addAll(seedHeads);
                Collections.sort(systemCompetitors, Inters.byOrdinate);
                final List<Inter> sigAfterSeeds = new ArrayList<>(system.getSig().vertexSet());
                if (sigAfterSeeds.size() != sigBefore.size() + seedHeads.size()) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d seed SIG grew %d, expected %d",
                            page,
                            system.getId(),
                            staff.getId(),
                            sigAfterSeeds.size() - sigBefore.size(),
                            seedHeads.size()));
                }
                for (int ordinal = 0; ordinal < seedHeads.size(); ordinal++) {
                    final String row = rangeSeedHeadRow(
                            page,
                            system,
                            staff,
                            ordinal,
                            seedHeads.get(ordinal),
                            sigAfterSeeds,
                            systemCompetitors);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    seedHash.add(row);
                    System.out.println(row);
                }

                // Construct range scanners after seed heads have joined the live competitor pool.
                final List<Geometry> geometries = buildGeometries(builder, staff, false);
                final PerfCounts perfBeforeTrace = readRangePerf(builder);
                final RangeStaffTrace trace = traceRangeStaff(
                        builder,
                        geometries,
                        catalog,
                        image,
                        distances,
                        systemSpots,
                        systemCompetitors);
                final PerfCounts perfAfterTrace = readRangePerf(builder);
                final PerfCounts tracedCalls = perfAfterTrace.minus(perfBeforeTrace);
                restoreRangePerf(builder, perfBeforeTrace);

                final PerfCounts perfBefore = readRangePerf(builder);
                final List<HeadInter> rangeHeads = processRangeStaff(builder, staff);
                final PerfCounts perfAfter = readRangePerf(builder);
                final PerfCounts perfDelta = perfAfter.minus(perfBefore);
                final List<Inter> sigAfterRange = new ArrayList<>(system.getSig().vertexSet());
                final int predictedAbandons = (int) trace.attempts.stream()
                        .filter(attempt -> attempt.outcome.startsWith("abandoned-"))
                        .count();
                if ((tracedCalls.bars != perfDelta.bars)
                        || (tracedCalls.overlaps != perfDelta.overlaps)
                        || (tracedCalls.evals != perfDelta.evals)
                        || (predictedAbandons != perfDelta.abandons)) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d traced range perf %s/%d abandons != production %s",
                            page,
                            system.getId(),
                            staff.getId(),
                            tracedCalls,
                            predictedAbandons,
                            perfDelta));
                }
                if (rangeHeads.size() != trace.finalCandidates.size()) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d predicted %d range heads but production made %d",
                            page,
                            system.getId(),
                            staff.getId(),
                            trace.finalCandidates.size(),
                            rangeHeads.size()));
                }
                if (sigAfterRange.size() != sigAfterSeeds.size() + rangeHeads.size()) {
                    throw new IllegalStateException(String.format(
                            "%s system %d staff %d range SIG grew %d, expected %d",
                            page,
                            system.getId(),
                            staff.getId(),
                            sigAfterRange.size() - sigAfterSeeds.size(),
                            rangeHeads.size()));
                }

                for (RangeSpot spot : trace.spots) {
                    final String row = rangeSpotRow(page, system, staff, spot);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    spotHash.add(row);
                    spotKernelHash.add(rangeSpotKernelRecord(system, staff, spot));
                    System.out.println(row);
                }
                for (int ordinal = 0; ordinal < trace.scans.size(); ordinal++) {
                    final RangeScan scan = trace.scans.get(ordinal);
                    scan.ordinal = ordinal;
                    final String row = rangeScanRow(page, system, staff, scan);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    scanHash.add(row);
                    scanKernelHash.add(rangeScanKernelRecord(system, staff, scan));
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }
                for (int ordinal = 0; ordinal < trace.attempts.size(); ordinal++) {
                    final RangeAttempt attempt = trace.attempts.get(ordinal);
                    attempt.ordinal = ordinal;
                    final String row = rangeAttemptRow(page, system, staff, attempt);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    attemptHash.add(row);
                    attemptKernelHash.add(rangeAttemptKernelRecord(system, staff, attempt));
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }
                for (RangeCandidate candidate : trace.rawCandidates) {
                    final String row = rangeRawCandidateRow(page, system, staff, candidate);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    rawCandidateHash.add(row);
                    rawKernelHash.add(rangeRawKernelRecord(system, staff, candidate));
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }
                for (RangeCandidate candidate : trace.candidates) {
                    final String row = rangeCandidateRow(page, system, staff, candidate);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    candidateHash.add(row);
                    System.out.println(row);
                }
                for (int ordinal = 0; ordinal < rangeHeads.size(); ordinal++) {
                    final HeadInter head = rangeHeads.get(ordinal);
                    final RangeCandidate candidate = trace.finalCandidates.get(ordinal);
                    candidate.finalOrdinal = ordinal;
                    assertRangeHeadMatches(page, system, staff, head, candidate);
                    final String row = rangeHeadRow(
                            page,
                            system,
                            staff,
                            ordinal,
                            head,
                            candidate,
                            sigAfterRange);
                    pageHash.add(row);
                    systemHash.add(row);
                    staffHash.add(row);
                    headHash.add(row);
                    System.out.println(row);
                }

                final String summary = String.format(
                        "headrangestaffsummary %s system %d staff %d scanners %d seedHeads %d "
                                + "spotSlices %d scans %d safetySkips %d attempts %d rawCandidates %d candidates %d "
                                + "seedConflicts %d glyphEmpty %d heads %d sigBefore %d sigAfterSeed %d "
                                + "sigAfterRange %d competitorsBefore %d competitorsAfterSeed %d "
                                + "scanKinds %d:%d:%d:%d:%d:%d decisions %d:%d:%d:%d:%d "
                                + "perf %d:%d:%d:%d outcomes %d:%d:%d:%d:%d:%d:%d "
                                + "seedHash %016x spotHash %016x spotKernelHash %016x "
                                + "scanHash %016x scanKernelHash %016x attemptHash %016x attemptKernelHash %016x "
                                + "rawCandidateHash %016x rawKernelHash %016x "
                                + "candidateHash %016x headHash %016x hash %016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        geometries.size(),
                        seedHeads.size(),
                        trace.spots.size(),
                        trace.scans.size(),
                        trace.scans.stream().filter(scan -> !scan.safety.equals("safe")).count(),
                        trace.attempts.size(),
                        trace.rawCandidates.size(),
                        trace.candidates.size(),
                        trace.candidates.stream()
                                .filter(candidate -> candidate.outcome.equals("seed-conflict"))
                                .count(),
                        trace.candidates.stream()
                                .filter(candidate -> candidate.outcome.equals("glyph-empty"))
                                .count(),
                        rangeHeads.size(),
                        sigBefore.size(),
                        sigAfterSeeds.size(),
                        sigAfterRange.size(),
                        competitorsBefore,
                        systemCompetitors.size(),
                        trace.scans.stream().filter(scan -> scan.shapeChoice.equals("all")).count(),
                        trace.scans.stream().filter(scan -> scan.shapeChoice.equals("hollow")).count(),
                        trace.scans.stream().filter(scan -> scan.safety.equals("right-limit")).count(),
                        trace.scans.stream().filter(scan -> scan.safety.equals("above-image")).count(),
                        trace.scans.stream().filter(scan -> scan.safety.equals("below-image")).count(),
                        trace.scans.stream().filter(scan -> scan.safety.equals("foreground-gap")).count(),
                        trace.attempts.stream()
                                .filter(attempt -> Boolean.TRUE.equals(attempt.blackToVoid))
                                .count(),
                        trace.rawCandidates.size() - trace.candidates.size(),
                        trace.candidates.stream()
                                .filter(candidate -> candidate.outcome.equals("seed-conflict"))
                                .count(),
                        trace.candidates.stream()
                                .filter(candidate -> candidate.outcome.equals("glyph-empty"))
                                .count(),
                        rangeHeads.size(),
                        perfDelta.bars,
                        perfDelta.overlaps,
                        perfDelta.evals,
                        perfDelta.abandons,
                        rangeOutcomeCount(trace.attempts, "abandoned-bar"),
                        rangeOutcomeCount(trace.attempts, "abandoned-competitor"),
                        rangeOutcomeCount(trace.attempts, "abandoned-nominal"),
                        rangeOutcomeCount(trace.attempts, "no-best"),
                        rangeOutcomeCount(trace.attempts, "weak-stemless"),
                        rangeOutcomeCount(trace.attempts, "min-grade"),
                        rangeOutcomeCount(trace.attempts, "provisional"),
                        seedHash.value(),
                        spotHash.value(),
                        spotKernelHash.value(),
                        scanHash.value(),
                        scanKernelHash.value(),
                        attemptHash.value(),
                        attemptKernelHash.value(),
                        rawCandidateHash.value(),
                        rawKernelHash.value(),
                        candidateHash.value(),
                        headHash.value(),
                        staffHash.value());
                pageHash.add(summary);
                systemHash.add(summary);
                System.out.println(summary);

                pageSpots += trace.spots.size();
                pageScans += trace.scans.size();
                pageAttempts += trace.attempts.size();
                pageRawCandidates += trace.rawCandidates.size();
                pageCandidates += trace.candidates.size();
                pageSeedHeads += seedHeads.size();
                pageRangeHeads += rangeHeads.size();
                systemSpotSlices += trace.spots.size();
                systemScans += trace.scans.size();
                systemAttempts += trace.attempts.size();
                systemRawCandidates += trace.rawCandidates.size();
                systemCandidates += trace.candidates.size();
                systemSeedHeads += seedHeads.size();
                systemRangeHeads += rangeHeads.size();
            }

            final PerfCounts seedPerf = readSeedPerf(builder);
            final PerfCounts rangePerf = readRangePerf(builder);
            final String summary = String.format(
                    "headrangesystemsummary %s system %d staffs %d spotSlices %d scans %d attempts %d "
                            + "rawCandidates %d candidates %d seedHeads %d rangeHeads %d "
                            + "sigBefore %d sigAfter %d competitorsBefore %d competitorsAfter %d "
                            + "seedPerf %d:%d:%d:%d rangePerf %d:%d:%d:%d %016x",
                    page,
                    system.getId(),
                    system.getStaves().stream().filter(staff -> !staff.isTablature()).count(),
                    systemSpotSlices,
                    systemScans,
                    systemAttempts,
                    systemRawCandidates,
                    systemCandidates,
                    systemSeedHeads,
                    systemRangeHeads,
                    initialSigCount,
                    system.getSig().vertexSet().size(),
                    initialCompetitorCount,
                    systemCompetitors.size(),
                    seedPerf.bars,
                    seedPerf.overlaps,
                    seedPerf.evals,
                    seedPerf.abandons,
                    rangePerf.bars,
                    rangePerf.overlaps,
                    rangePerf.evals,
                    rangePerf.abandons,
                    systemHash.value());
            pageHash.add(summary);
            System.out.println(summary);
        }

        System.out.printf(
                "headrangepagesummary %s staffs %d spotSlices %d scans %d attempts %d rawCandidates %d "
                        + "candidates %d seedHeads %d rangeHeads %d %016x%n",
                page,
                pageStaffs,
                pageSpots,
                pageScans,
                pageAttempts,
                pageRawCandidates,
                pageCandidates,
                pageSeedHeads,
                pageRangeHeads,
                pageHash.value());
    }

    @SuppressWarnings("unchecked")
    private static List<HeadInter> processRangeStaff (NoteHeadsBuilder builder,
                                                      Staff staff)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(
                "processStaff", Staff.class, boolean.class);
        method.setAccessible(true);
        return (List<HeadInter>) method.invoke(builder, staff, false);
    }

    @SuppressWarnings("unchecked")
    private static RangeStaffTrace traceRangeStaff (NoteHeadsBuilder builder,
                                                    List<Geometry> geometries,
                                                    Catalog catalog,
                                                    ByteProcessor image,
                                                    DistanceTable distances,
                                                    List<Glyph> systemSpots,
                                                    List<Inter> systemCompetitors)
        throws Exception
    {
        final RangeStaffTrace trace = new RangeStaffTrace();
        final Object params = field(builder, "params");
        final int templateHalf = intField(builder, "templateHalf");
        final double maxDistanceLow = doubleField(params, "maxDistanceLow");
        final double reallyBadDistance = doubleField(params, "reallyBadDistance");
        final int maxTemplateDx = intField(params, "maxTemplateDx");
        final Method relevantBlack = SCANNER_CLASS.getDeclaredMethod(
                "getRelevantBlackAbscissae", int.class, int.class);
        relevantBlack.setAccessible(true);
        final Method theoretical = SCANNER_CLASS.getDeclaredMethod(
                "getTheoreticalOrdinate", int.class);
        theoretical.setAccessible(true);
        final Method eval = SCANNER_CLASS.getDeclaredMethod(
                "eval", Shape.class, int.class, int.class, Anchor.class);
        eval.setAccessible(true);
        final Method evalBlackAsVoid = SCANNER_CLASS.getDeclaredMethod(
                "evalBlackAsVoid", int.class, int.class, Anchor.class);
        evalBlackAsVoid.setAccessible(true);
        final Method weakStemless = SCANNER_CLASS.getDeclaredMethod(
                "isWeakStemLessHead", Shape.class, PixelDistance.class);
        weakStemless.setAccessible(true);
        final Method createInter = BUILDER_CLASS.getDeclaredMethod(
                "createInter",
                PixelDistance.class,
                Anchor.class,
                Shape.class,
                Staff.class,
                double.class);
        createInter.setAccessible(true);
        final Method aggregateMatches = BUILDER_CLASS.getDeclaredMethod(
                "aggregateMatches", List.class);
        aggregateMatches.setAccessible(true);
        final Method filterSeedConflicts = BUILDER_CLASS.getDeclaredMethod(
                "filterSeedConflicts", List.class, List.class);
        filterSeedConflicts.setAccessible(true);

        for (int scannerOrdinal = 0; scannerOrdinal < geometries.size(); scannerOrdinal++) {
            final Geometry geometry = geometries.get(scannerOrdinal);
            final Object scanner = geometry.scanner;
            final int scanLeft = geometry.rangeLeft;
            final int scanRight = geometry.rangeRight;
            if (scanRight < scanLeft) {
                continue;
            }
            final boolean[] blackRelevants = (boolean[]) relevantBlack.invoke(
                    scanner, scanLeft, scanRight);
            final int[] yOffsets = (int[]) field(scanner, "yOffsets");
            final Collection<Shape> allShapes = (Collection<Shape>) field(
                    scanner, "scannerTemplateNotesAll");
            final Collection<Shape> hollowShapes = (Collection<Shape>) field(
                    scanner, "scannerTemplateNotesHollow");
            final List<Inter> competitors = (List<Inter>) field(scanner, "competitors");
            final Area competitorsArea = (Area) field(scanner, "competitorsArea");
            final List<Glyph> spots = glyphsSlice(builder, systemSpots, competitorsArea);
            final List<RangeCandidate> scannerRaw = new ArrayList<>();

            for (int spotOrdinal = 0; spotOrdinal < spots.size(); spotOrdinal++) {
                final Glyph spot = spots.get(spotOrdinal);
                final Rectangle expanded = spot.getBounds();
                expanded.grow(geometry.interline, 0);
                final int effectiveLeft = Math.max(scanLeft, expanded.x);
                final int effectiveRight = Math.min(
                        scanRight, expanded.x + expanded.width - 1);
                trace.spots.add(new RangeSpot(
                        scannerOrdinal,
                        geometry.source,
                        spotOrdinal,
                        identityOrdinal(systemSpots, spot),
                        spot,
                        expanded,
                        (effectiveRight < effectiveLeft) ? null : effectiveLeft,
                        (effectiveRight < effectiveLeft) ? null : effectiveRight));
            }

            for (int x0 = scanLeft; x0 <= scanRight; x0++) {
                final int y0 = (int) theoretical.invoke(scanner, x0);
                final int probeX = x0 + templateHalf;
                String safety = "safe";
                Integer safetyValue = null;
                if (probeX >= distances.getWidth()) {
                    safety = "right-limit";
                } else if (y0 < 0) {
                    safety = "above-image";
                } else if (y0 >= distances.getHeight()) {
                    safety = "below-image";
                } else {
                    safetyValue = distances.getValue(probeX, y0);
                    if ((safetyValue / ChamferDistance.DEFAULT_NORMALIZER) > templateHalf) {
                        safety = "foreground-gap";
                    }
                }
                if (!safety.equals("safe")) {
                    final int skippedTo = x0 + (2 * templateHalf);
                    trace.scans.add(new RangeScan(
                            scannerOrdinal,
                            geometry.source,
                            x0,
                            y0,
                            probeX,
                            safetyValue,
                            safety,
                            skippedTo,
                            false,
                            "-",
                            "-"));
                    x0 += (2 * templateHalf) - 1;
                    continue;
                }

                final boolean blackRelevant = blackRelevants[x0 - scanLeft];
                final Collection<Shape> shapeSet = blackRelevant ? allShapes : hollowShapes;
                final String shapeChoice = blackRelevant ? "all" : "hollow";
                trace.scans.add(new RangeScan(
                        scannerOrdinal,
                        geometry.source,
                        x0,
                        y0,
                        probeX,
                        safetyValue,
                        safety,
                        x0 + 1,
                        blackRelevant,
                        shapeChoice,
                        shapeNames(shapeSet)));

                int shapeOrdinal = 0;
                for (Shape originalShape : shapeSet) {
                    Shape finalShape = originalShape;
                    PixelDistance best = null;
                    int bestYOrdinal = -1;
                    int bestYValue = 0;
                    int bestUpdates = 0;
                    String outcome = null;
                    final StringBuilder evaluations = new StringBuilder();
                    final PerfCounts perfBefore = readRangePerf(builder);

                    for (int yOrdinal = 0; yOrdinal < yOffsets.length; yOrdinal++) {
                        final int yOffset = yOffsets[yOrdinal];
                        final int y = y0 + yOffset;
                        final PerfCounts beforeEval = readRangePerf(builder);
                        final PixelDistance location = (PixelDistance) eval.invoke(
                                scanner,
                                originalShape,
                                x0,
                                y,
                                MIDDLE_LEFT);
                        final PerfCounts afterEval = readRangePerf(builder);
                        final String gate;
                        if (location != null) {
                            gate = "evaluated:" + hexDouble(location.d);
                        } else if (afterEval.bars > beforeEval.bars) {
                            gate = "bar";
                        } else if (afterEval.overlaps > beforeEval.overlaps) {
                            gate = "competitor";
                        } else {
                            throw new IllegalStateException(
                                    "null range eval without a rejecting gate");
                        }
                        if (!evaluations.isEmpty()) {
                            evaluations.append(',');
                        }
                        evaluations.append(yOrdinal).append(':').append(yOffset).append(':')
                                .append(y).append(':').append(gate);

                        if ((location != null) && (location.d <= maxDistanceLow)) {
                            if ((best == null) || (best.d > location.d)) {
                                best = location;
                                bestYOrdinal = yOrdinal;
                                bestYValue = yOffset;
                                bestUpdates++;
                            }
                        } else if (y == y0
                                && ((location == null)
                                        || (location.d >= reallyBadDistance))) {
                            outcome = switch (gate) {
                                case "bar" -> "abandoned-bar";
                                case "competitor" -> "abandoned-competitor";
                                default -> "abandoned-nominal";
                            };
                            break;
                        }
                    }

                    Boolean blackToVoid = null;
                    boolean weak = false;
                    HeadInter tracedHead = null;
                    if (outcome == null) {
                        if (best == null) {
                            outcome = "no-best";
                        } else {
                            if (originalShape == Shape.NOTEHEAD_BLACK) {
                                final Shape converted = (Shape) evalBlackAsVoid.invoke(
                                        scanner,
                                        best.x,
                                        best.y,
                                        MIDDLE_LEFT);
                                blackToVoid = converted != null;
                                if (converted != null) {
                                    finalShape = converted;
                                }
                            }
                            weak = (boolean) weakStemless.invoke(scanner, finalShape, best);
                            if (weak) {
                                outcome = "weak-stemless";
                            } else {
                                tracedHead = (HeadInter) createInter.invoke(
                                        builder,
                                        best,
                                        MIDDLE_LEFT,
                                        finalShape,
                                        staffOfScanner(scanner),
                                        (double) geometry.pitch);
                                if (tracedHead == null) {
                                    outcome = "min-grade";
                                } else {
                                    outcome = "provisional";
                                }
                            }
                        }
                    }

                    final PerfCounts perfDelta = readRangePerf(builder).minus(perfBefore);
                    final RangeAttempt attempt = new RangeAttempt(
                            scannerOrdinal,
                            geometry.source,
                            x0,
                            y0,
                            blackRelevant,
                            shapeChoice,
                            shapeOrdinal++,
                            originalShape,
                            finalShape,
                            evaluations.toString(),
                            perfDelta,
                            bestUpdates,
                            best,
                            bestYOrdinal,
                            bestYValue,
                            blackToVoid,
                            weak,
                            tracedHead,
                            outcome);
                    trace.attempts.add(attempt);
                    if (tracedHead != null) {
                        final RangeCandidate candidate = new RangeCandidate(
                                trace.rawCandidates.size(), attempt, tracedHead);
                        attempt.rawCandidateOrdinal = candidate.rawOrdinal;
                        trace.rawCandidates.add(candidate);
                        scannerRaw.add(candidate);
                    }
                }
            }

            final List<HeadInter> rawHeads = new ArrayList<>();
            for (RangeCandidate candidate : scannerRaw) {
                rawHeads.add(candidate.head);
            }
            final List<HeadInter> aggregated = (List<HeadInter>) aggregateMatches.invoke(
                    builder, rawHeads);
            final List<RangeCandidate> scannerCandidates = new ArrayList<>();
            for (int aggregateOrdinal = 0; aggregateOrdinal < aggregated.size(); aggregateOrdinal++) {
                final RangeCandidate winner = rangeCandidateByHead(
                        scannerRaw, aggregated.get(aggregateOrdinal));
                winner.aggregateOrdinal = aggregateOrdinal;
                winner.aggregateWinnerRawOrdinal = winner.rawOrdinal;
                winner.candidateOrdinal = trace.candidates.size();
                scannerCandidates.add(winner);
                trace.candidates.add(winner);
            }
            for (RangeCandidate candidate : scannerRaw) {
                for (RangeCandidate winner : scannerCandidates) {
                    final double dx = candidate.head.getCenter2D().getX()
                            - winner.head.getCenter2D().getX();
                    if (Math.abs(dx) <= maxTemplateDx) {
                        candidate.aggregateOrdinal = winner.aggregateOrdinal;
                        candidate.aggregateWinnerRawOrdinal = winner.rawOrdinal;
                        break;
                    }
                }
                if (candidate.aggregateOrdinal < 0) {
                    throw new IllegalStateException("range candidate was not aggregated");
                }
                if (candidate.candidateOrdinal < 0) {
                    candidate.outcome = "aggregate";
                }
            }
            for (RangeCandidate winner : scannerCandidates) {
                final StringBuilder members = new StringBuilder();
                for (RangeCandidate candidate : scannerRaw) {
                    if (candidate.aggregateOrdinal == winner.aggregateOrdinal) {
                        if (!members.isEmpty()) {
                            members.append(',');
                        }
                        members.append(candidate.rawOrdinal);
                    }
                }
                winner.aggregateMembers = members.toString();
            }

            final List<HeadInter> filtered = (List<HeadInter>) filterSeedConflicts.invoke(
                    builder, new ArrayList<>(aggregated), competitors);
            for (RangeCandidate candidate : scannerCandidates) {
                if (!identityContains(filtered, candidate.head)) {
                    candidate.seedConflicts = seedConflictEvidence(
                            builder,
                            candidate.head,
                            competitors,
                            systemCompetitors);
                    if (candidate.seedConflicts.equals("-")) {
                        throw new IllegalStateException(
                                "seed conflict filter removed a candidate without evidence");
                    }
                    candidate.outcome = "seed-conflict";
                    continue;
                }
                candidate.glyph = predictRangeGlyph(candidate.head, catalog, image);
                if (candidate.glyph == null) {
                    candidate.outcome = "glyph-empty";
                } else {
                    candidate.outcome = "final";
                    candidate.finalOrdinal = trace.finalCandidates.size();
                    trace.finalCandidates.add(candidate);
                }
            }
        }
        return trace;
    }

    private static RangeCandidate rangeCandidateByHead (List<RangeCandidate> candidates,
                                                        HeadInter selected)
    {
        for (RangeCandidate candidate : candidates) {
            if (candidate.head == selected) {
                return candidate;
            }
        }
        throw new IllegalStateException("aggregated head is absent from raw candidates");
    }

    private static boolean identityContains (List<?> values,
                                             Object selected)
    {
        for (Object value : values) {
            if (value == selected) {
                return true;
            }
        }
        return false;
    }

    private static String seedConflictEvidence (NoteHeadsBuilder builder,
                                                HeadInter head,
                                                List<Inter> competitors,
                                                List<Inter> systemCompetitors)
        throws Exception
    {
        final Method overlapSeed = BUILDER_CLASS.getDeclaredMethod(
                "overlapSeed", Inter.class, List.class);
        overlapSeed.setAccessible(true);
        final StringBuilder evidence = new StringBuilder();
        for (int localOrdinal = 0; localOrdinal < competitors.size(); localOrdinal++) {
            final Inter competitor = competitors.get(localOrdinal);
            if (!(competitor instanceof HeadInter)
                    || !(boolean) overlapSeed.invoke(
                            builder, head, Collections.singletonList(competitor))) {
                continue;
            }
            if (!evidence.isEmpty()) {
                evidence.append(',');
            }
            evidence.append(localOrdinal).append(':')
                    .append(identityOrdinal(systemCompetitors, competitor)).append(':')
                    .append(competitor.getId()).append(':')
                    .append(rectangle(competitor.getBounds())).append(':')
                    .append(hexDouble(competitor.getGrade()));
        }
        return evidence.isEmpty() ? "-" : evidence.toString();
    }

    private static RangeGlyph predictRangeGlyph (HeadInter head,
                                                 Catalog catalog,
                                                 ByteProcessor image)
    {
        final Template template = catalog.getTemplate(head.getShape());
        final Rectangle templateBounds = template.getBounds(head.getBounds());
        final List<Point> foreground = template.getForegroundPixels(templateBounds, image);
        if (foreground.isEmpty()) {
            return null;
        }
        final Rectangle foregroundBounds = PointUtil.boundsOf(foreground);
        final ByteProcessor buffer = new ByteProcessor(
                foregroundBounds.width, foregroundBounds.height);
        ByteUtil.fill(buffer, 255);
        for (Point point : foreground) {
            buffer.set(
                    point.x - foregroundBounds.x,
                    point.y - foregroundBounds.y,
                    0);
        }
        final RunTable table = new RunTableFactory(Orientation.VERTICAL).createTable(buffer);
        final Rectangle glyphBounds = new Rectangle(
                templateBounds.x + foregroundBounds.x,
                templateBounds.y + foregroundBounds.y,
                foregroundBounds.width,
                foregroundBounds.height);
        return new RangeGlyph(
                templateBounds,
                foregroundBounds,
                glyphBounds,
                table.getWeight(),
                runTableHash(table));
    }

    private static PerfCounts readRangePerf (NoteHeadsBuilder builder)
        throws Exception
    {
        final Object perf = field(builder, "rangePerf");
        return new PerfCounts(
                intField(perf, "bars"),
                intField(perf, "overlaps"),
                intField(perf, "evals"),
                intField(perf, "abandons"));
    }

    private static void restoreRangePerf (NoteHeadsBuilder builder,
                                          PerfCounts values)
        throws Exception
    {
        final Object perf = field(builder, "rangePerf");
        writeIntField(perf, "bars", values.bars);
        writeIntField(perf, "overlaps", values.overlaps);
        writeIntField(perf, "evals", values.evals);
        writeIntField(perf, "abandons", values.abandons);
    }

    private static int rangeOutcomeCount (List<RangeAttempt> attempts,
                                          String outcome)
    {
        return (int) attempts.stream().filter(attempt -> attempt.outcome.equals(outcome)).count();
    }

    private static String rangeScanRow (String page,
                                        SystemInfo system,
                                        Staff staff,
                                        RangeScan scan)
    {
        return String.format(
                "headrangescan %s system %d staff %d ordinal %d scanner %d source %s "
                        + "x %d y %d probeX %d probeValue %s safety %s nextX %d "
                        + "blackRelevant %s shapeChoice %s shapes %s",
                page,
                system.getId(),
                staff.getId(),
                scan.ordinal,
                scan.scannerOrdinal,
                scan.source.text,
                scan.x,
                scan.y,
                scan.probeX,
                (scan.probeValue == null) ? "-" : scan.probeValue,
                scan.safety,
                scan.nextX,
                scan.blackRelevant,
                scan.shapeChoice,
                scan.shapes);
    }

    private static String rangeSpotRow (String page,
                                        SystemInfo system,
                                        Staff staff,
                                        RangeSpot spot)
    {
        return "headrangespot " + page + " " + rangeSpotKernelFields(system, staff, spot);
    }

    private static String rangeSpotKernelRecord (SystemInfo system,
                                                Staff staff,
                                                RangeSpot spot)
    {
        return "headrangespotkernel " + rangeSpotKernelFields(system, staff, spot);
    }

    private static String rangeSpotKernelFields (SystemInfo system,
                                                 Staff staff,
                                                 RangeSpot spot)
    {
        final Rectangle bounds = spot.glyph.getBounds();
        final String effective = (spot.effectiveLeft == null) ? "-"
                : spot.effectiveLeft + ":" + spot.effectiveRight;
        return String.format(
                "system %d staff %d scanner %d source %s spot %d pool %d bounds %s "
                        + "weight %d runs %016x expanded %s effective %s",
                system.getId(),
                staff.getId(),
                spot.scannerOrdinal,
                spot.source.text,
                spot.spotOrdinal,
                spot.poolOrdinal,
                rectangle(bounds),
                spot.glyph.getWeight(),
                runTableHash(spot.glyph.getRunTable()),
                rectangle(spot.expandedBounds),
                effective);
    }

    private static String rangeScanKernelRecord (SystemInfo system,
                                                 Staff staff,
                                                 RangeScan scan)
    {
        return String.format(
                "headrangescankernel system %d staff %d scanner %d source %s x %d y %d "
                        + "probeX %d probeValue %s safety %s nextX %d blackRelevant %s "
                        + "shapeChoice %s shapes %s",
                system.getId(),
                staff.getId(),
                scan.scannerOrdinal,
                scan.source.text,
                scan.x,
                scan.y,
                scan.probeX,
                (scan.probeValue == null) ? "-" : scan.probeValue,
                scan.safety,
                scan.nextX,
                scan.blackRelevant,
                scan.shapeChoice,
                scan.shapes);
    }

    private static String rangeAttemptRow (String page,
                                           SystemInfo system,
                                           Staff staff,
                                           RangeAttempt attempt)
    {
        final String selected = (attempt.best == null) ? "-"
                : String.format(
                        "%d:%d:%d:%d:%s",
                        attempt.bestYOrdinal,
                        attempt.bestYValue,
                        attempt.best.x,
                        attempt.best.y,
                        hexDouble(attempt.best.d));
        return String.format(
                "headrangeattempt %s system %d staff %d ordinal %d scanner %d source %s "
                        + "x %d y %d blackRelevant %s shapeChoice %s shapeOrdinal %d "
                        + "original %s finalShape %s evaluations %s calls %d:%d:%d "
                        + "bestUpdates %d selected %s blackToVoid %s weakStemless %s "
                        + "rawCandidate %s outcome %s",
                page,
                system.getId(),
                staff.getId(),
                attempt.ordinal,
                attempt.scannerOrdinal,
                attempt.source.text,
                attempt.x,
                attempt.y,
                attempt.blackRelevant,
                attempt.shapeChoice,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.evaluations.isEmpty() ? "-" : attempt.evaluations,
                attempt.perf.bars,
                attempt.perf.overlaps,
                attempt.perf.evals,
                attempt.bestUpdates,
                selected,
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                attempt.weakStemless,
                (attempt.rawCandidateOrdinal < 0) ? "-" : attempt.rawCandidateOrdinal,
                attempt.outcome);
    }

    private static String rangeAttemptKernelRecord (SystemInfo system,
                                                    Staff staff,
                                                    RangeAttempt attempt)
    {
        final String selected = (attempt.best == null) ? "-"
                : String.format(
                        "%d:%d:%d:%d:%s",
                        attempt.bestYOrdinal,
                        attempt.bestYValue,
                        attempt.best.x,
                        attempt.best.y,
                        hexDouble(attempt.best.d));
        return String.format(
                "headrangeattemptkernel system %d staff %d scanner %d source %s x %d y %d "
                        + "blackRelevant %s shapeChoice %s shapeOrdinal %d original %s "
                        + "finalShape %s evaluations %s calls %d:%d:%d bestUpdates %d "
                        + "selected %s blackToVoid %s weakStemless %s rawCandidate %s outcome %s",
                system.getId(),
                staff.getId(),
                attempt.scannerOrdinal,
                attempt.source.text,
                attempt.x,
                attempt.y,
                attempt.blackRelevant,
                attempt.shapeChoice,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.evaluations.isEmpty() ? "-" : attempt.evaluations,
                attempt.perf.bars,
                attempt.perf.overlaps,
                attempt.perf.evals,
                attempt.bestUpdates,
                selected,
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                attempt.weakStemless,
                (attempt.rawCandidateOrdinal < 0) ? "-" : attempt.rawCandidateOrdinal,
                attempt.outcome);
    }

    private static String rangeRawCandidateRow (String page,
                                                SystemInfo system,
                                                Staff staff,
                                                RangeCandidate candidate)
    {
        final HeadInter head = candidate.head;
        return String.format(
                "headrangerawcandidate %s system %d staff %d ordinal %d attempt %d scanner %d "
                        + "source %s x %d y %d original %s finalShape %s selected %d:%d:%d:%d:%s "
                        + "blackToVoid %s slim %s pitch %s grade %s impacts %s "
                        + "aggregate %d aggregateWinner %d candidate %s outcome %s",
                page,
                system.getId(),
                staff.getId(),
                candidate.rawOrdinal,
                candidate.attempt.ordinal,
                candidate.attempt.scannerOrdinal,
                candidate.attempt.source.text,
                candidate.attempt.x,
                candidate.attempt.y,
                candidate.attempt.originalShape,
                candidate.attempt.finalShape,
                candidate.attempt.bestYOrdinal,
                candidate.attempt.bestYValue,
                candidate.attempt.best.x,
                candidate.attempt.best.y,
                hexDouble(candidate.attempt.best.d),
                (candidate.attempt.blackToVoid == null) ? "-" : candidate.attempt.blackToVoid,
                rectangle(head.getBounds()),
                hexDouble(head.getPitch()),
                hexDouble(head.getGrade()),
                impactRecord(head.getImpacts()),
                candidate.aggregateOrdinal,
                candidate.aggregateWinnerRawOrdinal,
                (candidate.candidateOrdinal < 0) ? "-" : candidate.candidateOrdinal,
                candidate.outcome);
    }

    private static String rangeRawKernelRecord (SystemInfo system,
                                               Staff staff,
                                               RangeCandidate candidate)
    {
        final RangeAttempt attempt = candidate.attempt;
        return String.format(
                "headrangerawkernel system %d staff %d scanner %d source %s x %d y %d "
                        + "shapeOrdinal %d original %s finalShape %s selected %d:%d:%d:%d:%s "
                        + "blackToVoid %s slim %s pitch %s grade %s impacts %s outcome provisional",
                system.getId(),
                staff.getId(),
                attempt.scannerOrdinal,
                attempt.source.text,
                attempt.x,
                attempt.y,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.bestYOrdinal,
                attempt.bestYValue,
                attempt.best.x,
                attempt.best.y,
                hexDouble(attempt.best.d),
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                rectangle(candidate.head.getBounds()),
                hexDouble(candidate.head.getPitch()),
                hexDouble(candidate.head.getGrade()),
                impactRecord(candidate.head.getImpacts()));
    }

    private static String rangeCandidateRow (String page,
                                             SystemInfo system,
                                             Staff staff,
                                             RangeCandidate candidate)
    {
        final String glyph = (candidate.glyph == null) ? "-" : candidate.glyph.record();
        return String.format(
                "headrangecandidate %s system %d staff %d ordinal %d raw %d attempt %d "
                        + "scanner %d source %s x %d y %d original %s finalShape %s "
                        + "selected %d:%d:%d:%d:%s blackToVoid %s slim %s pitch %s grade %s "
                        + "impacts %s aggregate %d members %s seedConflicts %s glyph %s "
                        + "finalOrdinal %s outcome %s",
                page,
                system.getId(),
                staff.getId(),
                candidate.candidateOrdinal,
                candidate.rawOrdinal,
                candidate.attempt.ordinal,
                candidate.attempt.scannerOrdinal,
                candidate.attempt.source.text,
                candidate.attempt.x,
                candidate.attempt.y,
                candidate.attempt.originalShape,
                candidate.attempt.finalShape,
                candidate.attempt.bestYOrdinal,
                candidate.attempt.bestYValue,
                candidate.attempt.best.x,
                candidate.attempt.best.y,
                hexDouble(candidate.attempt.best.d),
                (candidate.attempt.blackToVoid == null) ? "-" : candidate.attempt.blackToVoid,
                rectangle(candidate.head.getBounds()),
                hexDouble(candidate.head.getPitch()),
                hexDouble(candidate.head.getGrade()),
                impactRecord(candidate.head.getImpacts()),
                candidate.aggregateOrdinal,
                candidate.aggregateMembers,
                candidate.seedConflicts,
                glyph,
                (candidate.finalOrdinal < 0) ? "-" : candidate.finalOrdinal,
                candidate.outcome);
    }

    private static String rangeSeedHeadRow (String page,
                                            SystemInfo system,
                                            Staff staff,
                                            int ordinal,
                                            HeadInter head,
                                            List<Inter> sigAfterSeeds,
                                            List<Inter> systemCompetitors)
    {
        final Glyph glyph = head.getGlyph();
        if (glyph == null) {
            throw new IllegalStateException("range-boundary seed head has no glyph");
        }
        return String.format(
                "headrangeseedhead %s system %d staff %d ordinal %d sigId %d sigOrdinal %d "
                        + "competitorOrdinal %d shape %s pitch %s bounds %s grade %s impacts %s "
                        + "good %s glyphId %d glyphBounds %s glyphWeight %d glyphRuns %016x",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                head.getId(),
                identityOrdinal(sigAfterSeeds, head),
                identityOrdinal(systemCompetitors, head),
                head.getShape(),
                hexDouble(head.getPitch()),
                rectangle(head.getBounds()),
                hexDouble(head.getGrade()),
                impactRecord(head.getImpacts()),
                head.isGood(),
                glyph.getId(),
                rectangle(glyph.getBounds()),
                glyph.getWeight(),
                runTableHash(glyph.getRunTable()));
    }

    private static String rangeHeadRow (String page,
                                        SystemInfo system,
                                        Staff staff,
                                        int ordinal,
                                        HeadInter head,
                                        RangeCandidate candidate,
                                        List<Inter> sigAfterRange)
    {
        final Glyph glyph = head.getGlyph();
        if (glyph == null) {
            throw new IllegalStateException("range-created head has no glyph");
        }
        return String.format(
                "headrangehead %s system %d staff %d ordinal %d candidate %d raw %d attempt %d "
                        + "scanner %d source %s x %d y %d original %s finalShape %s "
                        + "selected %d:%d:%d:%d:%s blackToVoid %s slim %s sigId %d sigOrdinal %d "
                        + "shape %s pitch %s bounds %s grade %s impacts %s good %s "
                        + "glyphId %d glyphBounds %s glyphWeight %d glyphRuns %016x",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                candidate.candidateOrdinal,
                candidate.rawOrdinal,
                candidate.attempt.ordinal,
                candidate.attempt.scannerOrdinal,
                candidate.attempt.source.text,
                candidate.attempt.x,
                candidate.attempt.y,
                candidate.attempt.originalShape,
                candidate.attempt.finalShape,
                candidate.attempt.bestYOrdinal,
                candidate.attempt.bestYValue,
                candidate.attempt.best.x,
                candidate.attempt.best.y,
                hexDouble(candidate.attempt.best.d),
                (candidate.attempt.blackToVoid == null) ? "-" : candidate.attempt.blackToVoid,
                rectangle(candidate.head.getBounds()),
                head.getId(),
                identityOrdinal(sigAfterRange, head),
                head.getShape(),
                hexDouble(head.getPitch()),
                rectangle(head.getBounds()),
                hexDouble(head.getGrade()),
                impactRecord(head.getImpacts()),
                head.isGood(),
                glyph.getId(),
                rectangle(glyph.getBounds()),
                glyph.getWeight(),
                runTableHash(glyph.getRunTable()));
    }

    private static String impactRecord (GradeImpacts impacts)
    {
        final StringBuilder values = new StringBuilder();
        for (int index = 0; index < impacts.getImpactCount(); index++) {
            if (!values.isEmpty()) {
                values.append(',');
            }
            values.append(impacts.getName(index)).append(':')
                    .append(hexDouble(impacts.getImpact(index))).append(':')
                    .append(hexDouble(impacts.getWeight(index)));
        }
        return values.toString();
    }

    private static void assertRangeHeadMatches (String page,
                                                SystemInfo system,
                                                Staff staff,
                                                HeadInter actual,
                                                RangeCandidate candidate)
    {
        final HeadInter traced = candidate.head;
        final RangeGlyph glyph = candidate.glyph;
        if ((glyph == null)
                || (actual.getShape() != traced.getShape())
                || (Double.doubleToLongBits(actual.getPitch())
                        != Double.doubleToLongBits(traced.getPitch()))
                || (Double.doubleToLongBits(actual.getGrade())
                        != Double.doubleToLongBits(traced.getGrade()))
                || (actual.getImpacts().getImpactCount() != traced.getImpacts().getImpactCount())
                || !actual.getBounds().equals(glyph.glyphBounds)
                || (actual.getGlyph() == null)
                || !actual.getGlyph().getBounds().equals(glyph.glyphBounds)
                || (actual.getGlyph().getWeight() != glyph.weight)
                || (runTableHash(actual.getGlyph().getRunTable()) != glyph.runDigest)) {
            throw new IllegalStateException(String.format(
                    "%s system %d staff %d final range head does not match candidate %d",
                    page,
                    system.getId(),
                    staff.getId(),
                    candidate.candidateOrdinal));
        }
        for (int index = 0; index < actual.getImpacts().getImpactCount(); index++) {
            if (Double.doubleToLongBits(actual.getImpacts().getImpact(index))
                    != Double.doubleToLongBits(traced.getImpacts().getImpact(index))) {
                throw new IllegalStateException("final range head impact differs from trace");
            }
        }
    }

    @SuppressWarnings("unchecked")
    private static List<HeadInter> processSeedStaff (NoteHeadsBuilder builder,
                                                     Staff staff)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(
                "processStaff", Staff.class, boolean.class);
        method.setAccessible(true);
        return (List<HeadInter>) method.invoke(builder, staff, true);
    }

    @SuppressWarnings("unchecked")
    private static List<SeedAttempt> traceSeedAttempts (NoteHeadsBuilder builder,
                                                        List<Glyph> systemSeeds,
                                                        List<Geometry> geometries,
                                                        Catalog catalog,
                                                        ByteProcessor image)
        throws Exception
    {
        final List<SeedAttempt> attempts = new ArrayList<>();
        final int[] xOffsets = (int[]) field(builder, "xOffsets");
        final Object params = field(builder, "params");
        final double maxDistanceLow = doubleField(params, "maxDistanceLow");
        final double reallyBadDistance = doubleField(params, "reallyBadDistance");
        final Method theoretical = SCANNER_CLASS.getDeclaredMethod(
                "getTheoreticalOrdinate", int.class);
        theoretical.setAccessible(true);
        final Method eval = SCANNER_CLASS.getDeclaredMethod(
                "eval", Shape.class, int.class, int.class, Anchor.class);
        eval.setAccessible(true);
        final Method evalBlackAsVoid = SCANNER_CLASS.getDeclaredMethod(
                "evalBlackAsVoid", int.class, int.class, Anchor.class);
        evalBlackAsVoid.setAccessible(true);
        final Method createInter = BUILDER_CLASS.getDeclaredMethod(
                "createInter",
                PixelDistance.class,
                Anchor.class,
                Shape.class,
                Staff.class,
                double.class);
        createInter.setAccessible(true);

        for (int scannerOrdinal = 0; scannerOrdinal < geometries.size(); scannerOrdinal++) {
            final Geometry geometry = geometries.get(scannerOrdinal);
            final Object scanner = geometry.scanner;
            final Area seedsArea = (Area) field(scanner, "seedsArea");
            final List<Glyph> seeds = glyphsSlice(builder, systemSeeds, seedsArea);
            final List<Shape> shapes = new ArrayList<>(
                    (Collection<Shape>) field(scanner, "scannerTemplateNotesStem"));
            final int[] yOffsets = (int[]) field(scanner, "yOffsets");
            final Object line = field(scanner, "line");
            final Method lineYAt = line.getClass().getDeclaredMethod("yAt", double.class);
            lineYAt.setAccessible(true);

            for (Glyph seed : seeds) {
                final int seedPoolOrdinal = identityOrdinal(systemSeeds, seed);
                int nominalX = (int) Math.rint(seed.getCenter2D().getX());
                final double yLine = ((Number) lineYAt.invoke(line, (double) nominalX)).doubleValue();
                final Point2D top = seed.getStartPoint(Orientation.VERTICAL);
                final Point2D bottom = seed.getStopPoint(Orientation.VERTICAL);
                nominalX = (int) Math.rint(LineUtil.xAtY(top, bottom, yLine));
                final int nominalY = (int) theoretical.invoke(scanner, nominalX);

                for (Anchor side : new Anchor[] { LEFT_STEM, RIGHT_STEM }) {
                    for (int shapeOrdinal = 0; shapeOrdinal < shapes.size(); shapeOrdinal++) {
                        final Shape originalShape = shapes.get(shapeOrdinal);
                        Shape finalShape = originalShape;
                        PixelDistance best = null;
                        int bestYOrdinal = -1;
                        int bestXOrdinal = -1;
                        int bestUpdates = 0;
                        String nominalGate = "-";
                        Double nominalDistance = null;
                        String outcome = null;
                        final PerfCounts perfBefore = readSeedPerf(builder);

                        Search:
                        for (int yOrdinal = 0; yOrdinal < yOffsets.length; yOrdinal++) {
                            final int y = nominalY + yOffsets[yOrdinal];
                            for (int xOrdinal = 0; xOrdinal < xOffsets.length; xOrdinal++) {
                                final int x = nominalX + xOffsets[xOrdinal];
                                final PerfCounts beforeEval = readSeedPerf(builder);
                                final PixelDistance location = (PixelDistance) eval.invoke(
                                        scanner,
                                        originalShape,
                                        x,
                                        y,
                                        side);
                                final PerfCounts afterEval = readSeedPerf(builder);

                                if ((x == nominalX) && (y == nominalY)) {
                                    if (location != null) {
                                        nominalGate = "evaluated";
                                        nominalDistance = location.d;
                                    } else if (afterEval.bars > beforeEval.bars) {
                                        nominalGate = "bar";
                                    } else if (afterEval.overlaps > beforeEval.overlaps) {
                                        nominalGate = "competitor";
                                    } else {
                                        throw new IllegalStateException(
                                                "null nominal eval without a rejecting gate");
                                    }
                                }

                                if ((location != null) && (location.d <= maxDistanceLow)) {
                                    if ((best == null) || (best.d > location.d)) {
                                        best = location;
                                        bestYOrdinal = yOrdinal;
                                        bestXOrdinal = xOrdinal;
                                        bestUpdates++;
                                    }
                                } else if ((x == nominalX) && (y == nominalY)
                                        && ((location == null)
                                                || (location.d >= reallyBadDistance))) {
                                    outcome = switch (nominalGate) {
                                        case "bar" -> "abandoned-bar";
                                        case "competitor" -> "abandoned-competitor";
                                        default -> "abandoned-nominal";
                                    };
                                    break Search;
                                }
                            }
                        }

                        Boolean blackToVoid = null;
                        HeadInter tracedHead = null;
                        boolean minGradePass = false;
                        if (outcome == null) {
                            if (best == null) {
                                outcome = "no-best";
                            } else {
                                if (originalShape == Shape.NOTEHEAD_BLACK) {
                                    final Shape converted = (Shape) evalBlackAsVoid.invoke(
                                            scanner,
                                            best.x,
                                            best.y,
                                            side);
                                    blackToVoid = converted != null;
                                    if (converted != null) {
                                        finalShape = converted;
                                    }
                                }
                                tracedHead = (HeadInter) createInter.invoke(
                                        builder,
                                        best,
                                        side,
                                        finalShape,
                                        field(scanner, "line") == null ? null
                                                : staffOfScanner(scanner),
                                        (double) geometry.pitch);
                                minGradePass = tracedHead != null;
                                if (!minGradePass) {
                                    outcome = "min-grade";
                                } else {
                                    final Template template = catalog.getTemplate(finalShape);
                                    final Rectangle templateBounds = template.getBounds(
                                            tracedHead.getBounds());
                                    final List<Point> foreground = template.getForegroundPixels(
                                            templateBounds,
                                            image);
                                    outcome = foreground.isEmpty() ? "glyph" : "final";
                                }
                            }
                        }

                        final PerfCounts perfDelta = readSeedPerf(builder).minus(perfBefore);
                        attempts.add(new SeedAttempt(
                                scannerOrdinal,
                                geometry.source,
                                seedPoolOrdinal,
                                seed,
                                side,
                                shapeOrdinal,
                                originalShape,
                                finalShape,
                                nominalX,
                                nominalY,
                                nominalGate,
                                nominalDistance,
                                perfDelta,
                                bestUpdates,
                                best,
                                bestYOrdinal,
                                (bestYOrdinal < 0) ? 0 : yOffsets[bestYOrdinal],
                                bestXOrdinal,
                                (bestXOrdinal < 0) ? 0 : xOffsets[bestXOrdinal],
                                blackToVoid,
                                minGradePass,
                                tracedHead,
                                outcome));
                    }
                }
            }
        }
        return attempts;
    }

    private static Staff staffOfScanner (Object scanner)
        throws Exception
    {
        final Object line = field(scanner, "line");
        final Method method = line.getClass().getMethod("getStaff");
        method.setAccessible(true);
        return (Staff) method.invoke(line);
    }

    private static String seedAttemptRow (String page,
                                          SystemInfo system,
                                          Staff staff,
                                          SeedAttempt attempt)
    {
        final Rectangle seedBounds = attempt.seed.getBounds();
        final String selected = (attempt.best == null) ? "-"
                : String.format(
                        "%d:%d:%d:%d:%d:%d:%s",
                        attempt.bestYOrdinal,
                        attempt.bestYValue,
                        attempt.bestXOrdinal,
                        attempt.bestXValue,
                        attempt.best.x,
                        attempt.best.y,
                        hexDouble(attempt.best.d));
        final String slim = (attempt.tracedHead == null) ? "-"
                : rectangle(attempt.tracedHead.getBounds());
        return String.format(
                "headseedattempt %s system %d staff %d ordinal %d scanner %d source %s "
                        + "seed %d seedId %d seedBounds %s seedWeight %d seedRuns %016x side %s "
                        + "shapeOrdinal %d original %s finalShape %s nominal %d:%d nominalGate %s "
                        + "nominalDistance %s calls %d:%d:%d bestUpdates %d selected %s "
                        + "blackToVoid %s minGrade %s slim %s outcome %s",
                page,
                system.getId(),
                staff.getId(),
                attempt.ordinal,
                attempt.scannerOrdinal,
                attempt.source.text,
                attempt.seedPoolOrdinal,
                attempt.seed.getId(),
                rectangle(seedBounds),
                attempt.seed.getWeight(),
                runTableHash(attempt.seed.getRunTable()),
                attempt.side,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.nominalX,
                attempt.nominalY,
                attempt.nominalGate,
                nullableDouble(attempt.nominalDistance),
                attempt.perf.bars,
                attempt.perf.overlaps,
                attempt.perf.evals,
                attempt.bestUpdates,
                selected,
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                attempt.minGradePass,
                slim,
                attempt.outcome);
    }

    /** Identity-free canonical search row shared with the Rust pre-glyph kernel. */
    private static String seedKernelRecord (SystemInfo system,
                                            Staff staff,
                                            SeedAttempt attempt)
    {
        final Rectangle seedBounds = attempt.seed.getBounds();
        final String selected = (attempt.best == null) ? "-"
                : String.format(
                        "%d:%d:%d:%d:%d:%d:%s",
                        attempt.bestYOrdinal,
                        attempt.bestYValue,
                        attempt.bestXOrdinal,
                        attempt.bestXValue,
                        attempt.best.x,
                        attempt.best.y,
                        hexDouble(attempt.best.d));
        final String slim = (attempt.tracedHead == null) ? "-"
                : rectangle(attempt.tracedHead.getBounds());
        return String.format(
                "headseedkernel system %d staff %d scanner %d source %s seed %d "
                        + "seedBounds %s seedWeight %d seedRuns %016x side %s shapeOrdinal %d "
                        + "original %s finalShape %s nominal %d:%d nominalGate %s "
                        + "nominalDistance %s calls %d:%d:%d bestUpdates %d selected %s "
                        + "blackToVoid %s minGrade %s slim %s outcome %s",
                system.getId(),
                staff.getId(),
                attempt.scannerOrdinal,
                attempt.source.text,
                attempt.seedPoolOrdinal,
                rectangle(seedBounds),
                attempt.seed.getWeight(),
                runTableHash(attempt.seed.getRunTable()),
                attempt.side,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.nominalX,
                attempt.nominalY,
                attempt.nominalGate,
                nullableDouble(attempt.nominalDistance),
                attempt.perf.bars,
                attempt.perf.overlaps,
                attempt.perf.evals,
                attempt.bestUpdates,
                selected,
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                attempt.minGradePass,
                slim,
                attempt.minGradePass ? "provisional" : attempt.outcome);
    }

    private static String seedHeadRow (String page,
                                       SystemInfo system,
                                       Staff staff,
                                       int ordinal,
                                       HeadInter head,
                                       SeedAttempt attempt,
                                       List<Inter> sigAfter,
                                       List<Inter> systemCompetitors,
                                       HeadSeedTally tally)
    {
        final Glyph glyph = head.getGlyph();
        if (glyph == null) {
            throw new IllegalStateException("seed-created head has no glyph");
        }
        final GradeImpacts impacts = head.getImpacts();
        final StringBuilder impactValues = new StringBuilder();
        for (int index = 0; index < impacts.getImpactCount(); index++) {
            if (!impactValues.isEmpty()) {
                impactValues.append(',');
            }
            impactValues.append(impacts.getName(index)).append(':')
                    .append(hexDouble(impacts.getImpact(index))).append(':')
                    .append(hexDouble(impacts.getWeight(index)));
        }
        final Rectangle box = head.getBounds();
        final Rectangle glyphBounds = glyph.getBounds();
        return String.format(
                "headseedhead %s system %d staff %d ordinal %d candidate %d attempt %d scanner %d "
                        + "seed %d side %s "
                        + "shapeOrdinal %d original %s finalShape %s selected %d:%d:%d:%d:%d:%d "
                        + "distance %s blackToVoid %s slim %s sigId %d sigOrdinal %d "
                        + "competitorOrdinal %d shape %s pitch %s bounds %s grade %s impacts %s "
                        + "good %s glyphId %d glyphBounds %s glyphWeight %d glyphRuns %016x "
                        + "tallyLeft %s tallyRight %s",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                attempt.candidateOrdinal,
                attempt.ordinal,
                attempt.scannerOrdinal,
                attempt.seedPoolOrdinal,
                attempt.side,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.bestYOrdinal,
                attempt.bestYValue,
                attempt.bestXOrdinal,
                attempt.bestXValue,
                attempt.best.x,
                attempt.best.y,
                hexDouble(attempt.best.d),
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                rectangle(attempt.tracedHead.getBounds()),
                head.getId(),
                identityOrdinal(sigAfter, head),
                identityOrdinal(systemCompetitors, head),
                head.getShape(),
                hexDouble(head.getPitch()),
                rectangle(box),
                hexDouble(head.getGrade()),
                impactValues,
                head.isGood(),
                glyph.getId(),
                rectangle(glyphBounds),
                glyph.getWeight(),
                runTableHash(glyph.getRunTable()),
                nullableDouble(tally.getDx(head, HorizontalSide.LEFT)),
                nullableDouble(tally.getDx(head, HorizontalSide.RIGHT)));
    }

    private static String seedCandidateRow (String page,
                                            SystemInfo system,
                                            Staff staff,
                                            int ordinal,
                                            SeedAttempt attempt)
    {
        final HeadInter head = attempt.tracedHead;
        if (head == null) {
            throw new IllegalStateException("minimum-grade candidate has no provisional head");
        }
        final GradeImpacts impacts = head.getImpacts();
        final StringBuilder impactValues = new StringBuilder();
        for (int index = 0; index < impacts.getImpactCount(); index++) {
            if (!impactValues.isEmpty()) {
                impactValues.append(',');
            }
            impactValues.append(impacts.getName(index)).append(':')
                    .append(hexDouble(impacts.getImpact(index))).append(':')
                    .append(hexDouble(impacts.getWeight(index)));
        }
        return String.format(
                "headseedcandidate %s system %d staff %d ordinal %d attempt %d scanner %d "
                        + "seed %d side %s shapeOrdinal %d original %s finalShape %s "
                        + "selected %d:%d:%d:%d:%d:%d distance %s blackToVoid %s pitch %s "
                        + "slim %s grade %s impacts %s outcome %s",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                attempt.ordinal,
                attempt.scannerOrdinal,
                attempt.seedPoolOrdinal,
                attempt.side,
                attempt.shapeOrdinal,
                attempt.originalShape,
                attempt.finalShape,
                attempt.bestYOrdinal,
                attempt.bestYValue,
                attempt.bestXOrdinal,
                attempt.bestXValue,
                attempt.best.x,
                attempt.best.y,
                hexDouble(attempt.best.d),
                (attempt.blackToVoid == null) ? "-" : attempt.blackToVoid,
                hexDouble(head.getPitch()),
                rectangle(head.getBounds()),
                hexDouble(head.getGrade()),
                impactValues,
                attempt.outcome);
    }

    private static void assertFinalHeadMatches (String page,
                                                SystemInfo system,
                                                Staff staff,
                                                HeadInter actual,
                                                SeedAttempt attempt)
    {
        final HeadInter traced = attempt.tracedHead;
        if ((traced == null)
                || (actual.getShape() != traced.getShape())
                || (Double.doubleToLongBits(actual.getPitch())
                        != Double.doubleToLongBits(traced.getPitch()))
                || (Double.doubleToLongBits(actual.getGrade())
                        != Double.doubleToLongBits(traced.getGrade()))
                || (actual.getImpacts().getImpactCount() != traced.getImpacts().getImpactCount())) {
            throw new IllegalStateException(String.format(
                    "%s system %d staff %d final head does not match traced attempt %d",
                    page,
                    system.getId(),
                    staff.getId(),
                    attempt.ordinal));
        }
        for (int index = 0; index < actual.getImpacts().getImpactCount(); index++) {
            if (Double.doubleToLongBits(actual.getImpacts().getImpact(index))
                    != Double.doubleToLongBits(traced.getImpacts().getImpact(index))) {
                throw new IllegalStateException("final head impact differs from trace");
            }
        }
    }

    private static PerfCounts readSeedPerf (NoteHeadsBuilder builder)
        throws Exception
    {
        final Object perf = field(builder, "seedsPerf");
        return new PerfCounts(
                intField(perf, "bars"),
                intField(perf, "overlaps"),
                intField(perf, "evals"),
                intField(perf, "abandons"));
    }

    private static void restoreSeedPerf (NoteHeadsBuilder builder,
                                         PerfCounts values)
        throws Exception
    {
        final Object perf = field(builder, "seedsPerf");
        writeIntField(perf, "bars", values.bars);
        writeIntField(perf, "overlaps", values.overlaps);
        writeIntField(perf, "evals", values.evals);
        writeIntField(perf, "abandons", values.abandons);
    }

    private static int identityOrdinal (List<?> universe,
                                        Object selected)
    {
        for (int index = 0; index < universe.size(); index++) {
            if (universe.get(index) == selected) {
                return index;
            }
        }
        throw new IllegalStateException("item is absent from its identity pool");
    }

    private static int outcomeCount (List<SeedAttempt> attempts,
                                     String outcome)
    {
        return (int) attempts.stream().filter(attempt -> attempt.outcome.equals(outcome)).count();
    }

    private static String rectangle (Rectangle box)
    {
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String nullableDouble (Double value)
    {
        return (value == null) ? "-" : hexDouble(value);
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

        wantedStub.reachStep(OmrStep.LEDGERS, false);
        return wantedStub.getSheet();
    }

    @SuppressWarnings("unchecked")
    private static void runSlicesPage (Path path,
                                       int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        final Map<SystemInfo, List<Glyph>> sheetSpots = new HeadSpotsBuilder(sheet).getSpots();
        final List<String> pageRows = new ArrayList<>();
        int pageGeometries = 0;
        int pageCompetitorCandidates = 0;
        int pageCompetitors = 0;
        int pageBarCandidates = 0;
        int pageBars = 0;

        System.out.printf(
                "headslicepage %s width %d height %d systems %d staves %d%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount());

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final NoteHeadsBuilder builder = new NoteHeadsBuilder(
                    system,
                    distances,
                    systemSpots,
                    new HeadSeedTally(),
                    false,
                    new java.util.TreeMap<>());
            prepareBuilder(builder, system, systemSpots);

            final List<Glyph> systemSeeds = (List<Glyph>) field(builder, "systemSeeds");
            final List<Inter> systemCompetitors = (List<Inter>) field(builder, "systemCompetitors");
            final List<Area> systemBarAreas = (List<Area>) field(builder, "systemBarAreas");
            final Set<Shape> competingShapes = (Set<Shape>) staticField("COMPETING_SHAPES");
            final List<Inter> competitorCandidates = system.getSig().inters(
                    inter -> competingShapes.contains(inter.getShape()));
            final int maxStem = sheet.getScale().getMaxStem();
            final int minBeamWidth = intField(field(builder, "params"), "minBeamWidth");
            final List<Inter> barCandidates = system.getSig().inters(
                    inter -> inter instanceof BarlineInter || inter instanceof BarConnectorInter);
            final List<Inter> frozenBars = system.getSig().inters(inter -> inter.isFrozen()
                    && (inter instanceof BarlineInter || inter instanceof BarConnectorInter));
            Collections.sort(frozenBars, Inters.byOrdinate);
            if (frozenBars.size() != systemBarAreas.size()) {
                throw new IllegalStateException("bar owner/area count mismatch");
            }

            final List<String> systemRows = new ArrayList<>();
            for (int ordinal = 0; ordinal < competitorCandidates.size(); ordinal++) {
                final Inter candidate = competitorCandidates.get(ordinal);
                final String decision = competitorDecision(candidate, maxStem, minBeamWidth);
                final String accepted = identityIndex(systemCompetitors, candidate);
                if ((decision.equals("accept")) != !accepted.equals("-")) {
                    throw new IllegalStateException("competitor decision/list mismatch");
                }
                final String row = String.format(
                        "headslicecompetitorcandidate %s system %d inputOrdinal %d decision %s "
                                + "acceptedOrdinal %s maxStem %d minBeamWidth %d evidence %s %s",
                        page,
                        system.getId(),
                        ordinal,
                        decision,
                        accepted,
                        maxStem,
                        minBeamWidth,
                        competitorEvidence(candidate, minBeamWidth),
                        interRecord(candidate));
                pageRows.add(row);
                systemRows.add(row);
                System.out.println(row);
                pageCompetitorCandidates++;
            }
            for (int ordinal = 0; ordinal < systemCompetitors.size(); ordinal++) {
                final String row = String.format(
                        "headslicecompetitor %s system %d ordinal %d %s",
                        page,
                        system.getId(),
                        ordinal,
                        interRecord(systemCompetitors.get(ordinal)));
                pageRows.add(row);
                systemRows.add(row);
                System.out.println(row);
                pageCompetitors++;
            }
            for (int ordinal = 0; ordinal < barCandidates.size(); ordinal++) {
                final Inter candidate = barCandidates.get(ordinal);
                final String row = String.format(
                        "headslicebarcandidate %s system %d inputOrdinal %d decision %s "
                                + "frozenOrdinal %s %s",
                        page,
                        system.getId(),
                        ordinal,
                        candidate.isFrozen() ? "frozen" : "not-frozen",
                        identityIndex(frozenBars, candidate),
                        interRecord(candidate));
                pageRows.add(row);
                systemRows.add(row);
                System.out.println(row);
                pageBarCandidates++;
            }
            for (int ordinal = 0; ordinal < frozenBars.size(); ordinal++) {
                final Inter owner = frozenBars.get(ordinal);
                final Area area = systemBarAreas.get(ordinal);
                if (!areaSummary(owner.getArea()).equals(areaSummary(area))) {
                    throw new IllegalStateException("frozen bar owner/area mismatch");
                }
                final String row = String.format(
                        "headslicebar %s system %d ordinal %d %s",
                        page,
                        system.getId(),
                        ordinal,
                        interRecord(owner));
                pageRows.add(row);
                systemRows.add(row);
                System.out.println(row);
                pageBars++;
            }

            int systemGeometries = 0;
            for (Staff staff : system.getStaves()) {
                if (staff.isTablature()) {
                    continue;
                }
                final List<String> staffRows = new ArrayList<>();
                final List<Geometry> geometries = buildGeometries(builder, staff);
                for (int ordinal = 0; ordinal < geometries.size(); ordinal++) {
                    final Geometry geometry = geometries.get(ordinal);
                    final Object scanner = geometry.scanner;
                    final Area seedsArea = (Area) field(scanner, "seedsArea");
                    final Area competitorsArea = (Area) field(scanner, "competitorsArea");
                    final List<Area> bars = (List<Area>) field(scanner, "barAreas");
                    final List<Inter> competitors = (List<Inter>) field(scanner, "competitors");
                    final List<Glyph> seeds = glyphsSlice(builder, systemSeeds, seedsArea);
                    final List<Glyph> spots = glyphsSlice(builder, systemSpots, competitorsArea);
                    final String row = String.format(
                            "headslicescan %s system %d staff %d ordinal %d source %s "
                                    + "seedBand %s competitorBand %s barBand %s "
                                    + "seedsArea %s competitorsArea %s seeds %s spots %s "
                                    + "competitors %s bars %s",
                            page,
                            system.getId(),
                            staff.getId(),
                            ordinal,
                            geometry.source.text,
                            seedBand(builder, geometry),
                            competitorBand(geometry),
                            barBand(builder, geometry),
                            areaSummary(seedsArea),
                            areaSummary(competitorsArea),
                            identityIndexes(systemSeeds, seeds),
                            identityIndexes(systemSpots, spots),
                            identityIndexes(systemCompetitors, competitors),
                            identityIndexes(systemBarAreas, bars));
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    System.out.println(row);
                    pageGeometries++;
                    systemGeometries++;
                }
                final String summary = String.format(
                        "headslicestaffsummary %s system %d staff %d geometries %d %016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        geometries.size(),
                        hash(staffRows));
                pageRows.add(summary);
                systemRows.add(summary);
                System.out.println(summary);
            }

            final String summary = String.format(
                    "headslicesystemsummary %s system %d seeds %d spots %d "
                            + "competitorCandidates %d competitors %d barCandidates %d bars %d "
                            + "geometries %d %016x",
                    page,
                    system.getId(),
                    systemSeeds.size(),
                    systemSpots.size(),
                    competitorCandidates.size(),
                    systemCompetitors.size(),
                    barCandidates.size(),
                    systemBarAreas.size(),
                    systemGeometries,
                    hash(systemRows));
            pageRows.add(summary);
            System.out.println(summary);
        }

        System.out.printf(
                "headslicepagesummary %s competitorCandidates %d competitors %d "
                        + "barCandidates %d bars %d geometries %d %016x%n",
                page,
                pageCompetitorCandidates,
                pageCompetitors,
                pageBarCandidates,
                pageBars,
                pageGeometries,
                hash(pageRows));
    }

    private static void prepareBuilder (NoteHeadsBuilder builder,
                                        SystemInfo system,
                                        List<Glyph> systemSpots)
        throws Exception
    {
        setField(builder, "systemBarAreas", call(builder, "getSystemBarAreas"));
        setField(builder, "systemCompetitors", call(builder, "getSystemCompetitors"));

        final List<Glyph> systemSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        Collections.sort(systemSeeds, Glyphs.byOrdinate);
        Collections.sort(systemSpots, Glyphs.byOrdinate);
        setField(builder, "systemSeeds", systemSeeds);
    }

    private static String parameterRow (String page,
                                        SystemInfo system,
                                        NoteHeadsBuilder builder,
                                        Scale scale)
        throws Exception
    {
        final Object params = field(builder, "params");
        return String.format(
                "headscannerparams %s system %d mainInterline %d maxStem %d maxDistanceLow %s "
                        + "reallyBadDistance %s maxTemplateDx %d maxClosedDy %d maxOpenDy %d "
                        + "minBeamWidth %d vBarMargin %s minTemplateWidth %d templateHalf %d "
                        + "xOffsets %s",
                page,
                system.getId(),
                scale.getInterline(),
                scale.getMaxStem(),
                hexDouble(doubleField(params, "maxDistanceLow")),
                hexDouble(doubleField(params, "reallyBadDistance")),
                intField(params, "maxTemplateDx"),
                intField(params, "maxClosedDy"),
                intField(params, "maxOpenDy"),
                intField(params, "minBeamWidth"),
                hexDouble(doubleField(params, "vBarMargin")),
                intField(builder, "minTemplateWidth"),
                intField(builder, "templateHalf"),
                joinInts((int[]) field(builder, "xOffsets")));
    }

    private static String staffRow (String page,
                                    SystemInfo system,
                                    Staff staff,
                                    MusicFamily family)
    {
        final Part part = staff.getPart();
        final String partFields;
        if (part == null) {
            partFields = "part - merged false partFirst - partLast -";
        } else {
            partFields = String.format(
                    "part %d merged %s partFirst %d partLast %d",
                    part.getId(),
                    part.isMerged(),
                    part.getFirstStaff().getId(),
                    part.getLastStaff().getId());
        }
        return String.format(
                "headscannerstaff %s system %d staff %d tablature %s drum %s lines %d "
                        + "interline %d headerStop %d %s pointSize %d catalog %s/%d",
                page,
                system.getId(),
                staff.getId(),
                staff.isTablature(),
                staff.isDrum(),
                staff.getLineCount(),
                staff.getSpecificInterline(),
                staff.getHeaderStop(),
                partFields,
                staff.getHeadPointSize(),
                family,
                staff.getHeadPointSize());
    }

    private static List<Geometry> buildGeometries (NoteHeadsBuilder builder,
                                                   Staff staff)
        throws Exception
    {
        return buildGeometries(builder, staff, true);
    }

    private static List<Geometry> buildGeometries (NoteHeadsBuilder builder,
                                                   Staff staff,
                                                   boolean useSeeds)
        throws Exception
    {
        final List<Geometry> geometries = new ArrayList<>();
        final int lineCount = staff.getLineCount();
        final int maxPitch = lineCount;
        int pitch = -lineCount;
        Object previous = null;

        for (int lineIndex = 0; lineIndex < staff.getLines().size(); lineIndex++) {
            final LineInfo line = staff.getLines().get(lineIndex);
            final Object adapter = newStaffLineAdapter(builder, staff, line);
            geometries.add(newGeometry(
                    builder,
                    staff,
                    Source.staffLine(lineIndex, line),
                    adapter,
                    previous,
                    -1,
                    pitch++,
                    useSeeds));
            geometries.add(newGeometry(
                    builder,
                    staff,
                    Source.staffLine(lineIndex, line),
                    adapter,
                    null,
                    0,
                    pitch++,
                    useSeeds));
            if (pitch == maxPitch) {
                geometries.add(newGeometry(
                        builder,
                        staff,
                        Source.staffLine(lineIndex, line),
                        adapter,
                        null,
                        1,
                        pitch++,
                        useSeeds));
            }
            previous = adapter;
        }

        if (lineCount == 1) {
            return geometries;
        }

        final Part part = staff.getPart();
        for (int dir : new int[] { -1, 1 }) {
            boolean lookFurther = true;
            if ((part != null) && part.isMerged()) {
                if ((dir > 0) && (staff == part.getFirstStaff())) {
                    lookFurther = false;
                } else if ((dir < 0) && (staff == part.getLastStaff())) {
                    lookFurther = false;
                }
            }

            pitch = dir * 4;
            for (int ledgerIndex = dir;; ledgerIndex += dir) {
                final List<LedgerInter> ledgers = staff.getLedgers(ledgerIndex);
                if ((ledgers == null) || ledgers.isEmpty()) {
                    break;
                }
                pitch += 2 * dir;
                char prefix = 'a';
                for (int ordinal = 0; ordinal < ledgers.size(); ordinal++) {
                    final Glyph glyph = ledgers.get(ordinal).getGlyph();
                    final Object adapter = newLedgerAdapter(
                            builder,
                            staff,
                            String.valueOf(prefix++),
                            glyph);
                    final Source source = Source.ledger(ledgerIndex, ordinal, glyph);
                    geometries.add(newGeometry(
                            builder,
                            staff,
                            source,
                            adapter,
                            null,
                            0,
                            pitch,
                            useSeeds));
                    if (lookFurther) {
                        geometries.add(newGeometry(
                                builder,
                                staff,
                                source,
                                adapter,
                            null,
                            dir,
                            pitch + dir,
                            useSeeds));
                    }
                }
            }
        }

        return geometries;
    }

    private static Geometry newGeometry (NoteHeadsBuilder builder,
                                         Staff staff,
                                         Source source,
                                         Object line,
                                         Object line2,
                                         int dir,
                                         int pitch,
                                         boolean useSeeds)
        throws Exception
    {
        final Object seed = newScanner(builder, line, line2, dir, pitch, true);
        final Object range = newScanner(builder, line, line2, dir, pitch, false);
        final Geometry a = readGeometry(builder, staff, source, seed, line, line2);
        final Geometry b = readGeometry(builder, staff, source, range, line, line2);
        if (!a.equals(b)) {
            throw new IllegalStateException(
                    "seed/range scanner geometry differs for staff " + staff.getId() + " pitch "
                            + pitch);
        }
        return useSeeds ? a : b;
    }

    @SuppressWarnings("unchecked")
    private static Geometry readGeometry (NoteHeadsBuilder builder,
                                          Staff staff,
                                          Source source,
                                          Object scanner,
                                          Object line,
                                          Object line2)
        throws Exception
    {
        final int left = (int) callDeclared(line.getClass(), line, "getLeftAbscissa");
        final int right = (int) callDeclared(line.getClass(), line, "getRightAbscissa");
        final int line2Left = (line2 == null) ? Integer.MIN_VALUE
                : (int) callDeclared(line2.getClass(), line2, "getLeftAbscissa");
        final int line2Right = (line2 == null) ? Integer.MIN_VALUE
                : (int) callDeclared(line2.getClass(), line2, "getRightAbscissa");
        final int rangeLeft = Math.max(left, staff.getHeaderStop());
        final int rangeRight = right - intField(builder, "minTemplateWidth");

        final List<Object> farther = (List<Object>) field(scanner, "ledgers");
        final List<String> fartherAxes = new ArrayList<>();
        for (Object adapter : farther) {
            final Point2D p1 = (Point2D) field(adapter, "left");
            final Point2D p2 = (Point2D) field(adapter, "right");
            fartherAxes.add(axis(p1, p2));
        }

        return new Geometry(
                scanner,
                source,
                intField(scanner, "interline"),
                intField(scanner, "dir"),
                intField(scanner, "pitch"),
                booleanField(scanner, "isOpen"),
                (int[]) field(scanner, "yOffsets"),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesAll")),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesStem")),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesHollow")),
                left,
                right,
                line2Left,
                line2Right,
                fartherAxes,
                ordinateRle(scanner, left, right),
                rangeLeft,
                rangeRight,
                ordinateRle(scanner, rangeLeft, rangeRight));
    }

    private static String geometryRow (String page,
                                       SystemInfo system,
                                       Staff staff,
                                       int ordinal,
                                       Geometry geometry)
    {
        return String.format(
                "headscannergeometry %s system %d staff %d ordinal %d source %s dir %d pitch %d "
                        + "open %s interline %d line %d %d line2 %s yOffsets %s all %s stem %s "
                        + "hollow %s farther %s ordinate %s range %d %d rangeOrdinate %s",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                geometry.source.text,
                geometry.dir,
                geometry.pitch,
                geometry.open,
                geometry.interline,
                geometry.left,
                geometry.right,
                (geometry.line2Left == Integer.MIN_VALUE) ? "-"
                        : geometry.line2Left + ":" + geometry.line2Right,
                joinInts(geometry.yOffsets),
                geometry.allShapes,
                geometry.stemShapes,
                geometry.hollowShapes,
                geometry.fartherAxes.isEmpty() ? "-" : String.join(",", geometry.fartherAxes),
                geometry.ordinateRle,
                geometry.rangeLeft,
                geometry.rangeRight,
                geometry.rangeOrdinateRle);
    }

    private static String ordinateRle (Object scanner,
                                       int left,
                                       int right)
        throws Exception
    {
        if (right < left) {
            return "-";
        }
        final Method method = SCANNER_CLASS.getDeclaredMethod("getTheoreticalOrdinate", int.class);
        method.setAccessible(true);
        final StringBuilder sb = new StringBuilder();
        int value = (int) method.invoke(scanner, left);
        int count = 1;
        for (int x = left + 1; x <= right; x++) {
            final int next = (int) method.invoke(scanner, x);
            if (next == value) {
                count++;
            } else {
                appendRun(sb, value, count);
                value = next;
                count = 1;
            }
        }
        appendRun(sb, value, count);
        return sb.toString();
    }

    private static void appendRun (StringBuilder sb,
                                   int value,
                                   int count)
    {
        if (!sb.isEmpty()) {
            sb.append(',');
        }
        sb.append(value).append(':').append(count);
    }

    private static Object newStaffLineAdapter (NoteHeadsBuilder builder,
                                               Staff staff,
                                               LineInfo line)
        throws Exception
    {
        return constructor(STAFF_LINE_ADAPTER_CLASS).newInstance(builder, staff, line);
    }

    private static Object newLedgerAdapter (NoteHeadsBuilder builder,
                                            Staff staff,
                                            String prefix,
                                            Glyph glyph)
        throws Exception
    {
        return constructor(LEDGER_ADAPTER_CLASS).newInstance(builder, staff, prefix, glyph);
    }

    private static Object newScanner (NoteHeadsBuilder builder,
                                      Object line,
                                      Object line2,
                                      int dir,
                                      int pitch,
                                      boolean useSeeds)
        throws Exception
    {
        return constructor(SCANNER_CLASS).newInstance(
                builder, line, line2, dir, pitch, useSeeds);
    }

    private static Constructor<?> constructor (Class<?> type)
    {
        final Constructor<?>[] constructors = type.getDeclaredConstructors();
        if (constructors.length != 1) {
            throw new IllegalStateException("expected one constructor for " + type.getName());
        }
        constructors[0].setAccessible(true);
        return constructors[0];
    }

    private static Class<?> nested (String simpleName)
    {
        try {
            return Class.forName(NoteHeadsBuilder.class.getName() + "$" + simpleName);
        } catch (ClassNotFoundException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private static Object call (Object target,
                                String methodName)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(methodName);
        method.setAccessible(true);
        return method.invoke(target);
    }

    private static Object callDeclared (Class<?> type,
                                        Object target,
                                        String methodName)
        throws Exception
    {
        final Method method = type.getDeclaredMethod(methodName);
        method.setAccessible(true);
        return method.invoke(target);
    }

    @SuppressWarnings("unchecked")
    private static List<Glyph> glyphsSlice (NoteHeadsBuilder builder,
                                            List<Glyph> glyphs,
                                            Area area)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(
                "getGlyphsSlice", List.class, Area.class);
        method.setAccessible(true);
        return (List<Glyph>) method.invoke(builder, glyphs, area);
    }

    private static Object field (Object target,
                                 String name)
        throws Exception
    {
        final Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static Object staticField (String name)
        throws Exception
    {
        final Field field = BUILDER_CLASS.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(null);
    }

    private static void setField (Object target,
                                  String name,
                                  Object value)
        throws Exception
    {
        final Field field = BUILDER_CLASS.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    private static void writeIntField (Object target,
                                       String name,
                                       int value)
        throws Exception
    {
        final Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.setInt(target, value);
    }

    private static int intField (Object target,
                                 String name)
        throws Exception
    {
        return ((Number) field(target, name)).intValue();
    }

    private static double doubleField (Object target,
                                       String name)
        throws Exception
    {
        return ((Number) field(target, name)).doubleValue();
    }

    private static boolean booleanField (Object target,
                                         String name)
        throws Exception
    {
        return (boolean) field(target, name);
    }

    private static String shapeNames (Collection<Shape> shapes)
    {
        final StringBuilder sb = new StringBuilder();
        for (Shape shape : shapes) {
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(shape);
        }
        return sb.isEmpty() ? "-" : sb.toString();
    }

    private static String joinInts (int[] values)
    {
        if (values.length == 0) {
            return "-";
        }
        final StringBuilder sb = new StringBuilder();
        for (int value : values) {
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(value);
        }
        return sb.toString();
    }

    private static <T> String identityIndexes (List<T> universe,
                                               List<T> selected)
    {
        if (selected.isEmpty()) {
            return "-";
        }
        final StringBuilder sb = new StringBuilder();
        for (T item : selected) {
            int found = -1;
            for (int index = 0; index < universe.size(); index++) {
                if (universe.get(index) == item) {
                    found = index;
                    break;
                }
            }
            if (found < 0) {
                throw new IllegalStateException("slice item is absent from its source pool");
            }
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(found);
        }
        return sb.toString();
    }

    private static <T> String identityIndex (List<T> universe,
                                             T selected)
    {
        for (int index = 0; index < universe.size(); index++) {
            if (universe.get(index) == selected) {
                return Integer.toString(index);
            }
        }
        return "-";
    }

    private static String areaSummary (Area area)
    {
        if (area == null) {
            return "-";
        }
        final Rectangle box = area.getBounds();
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String interRecord (Inter inter)
    {
        final Rectangle box = inter.getBounds();
        final int staff = (inter.getStaff() == null) ? -1 : inter.getStaff().getId();
        final StringBuilder sb = new StringBuilder(String.format(
                "class %s shape %s staff %s bounds %d:%d:%d:%d grade %s best %s "
                        + "good %s frozen %s removed %s area %s",
                inter.getClass().getSimpleName(),
                inter.getShape(),
                (staff < 0) ? "-" : Integer.toString(staff),
                box.x,
                box.y,
                box.width,
                box.height,
                hexDouble(inter.getGrade()),
                hexDouble(inter.getBestGrade()),
                inter.isGood(),
                inter.isFrozen(),
                inter.isRemoved(),
                areaSummary(inter.getArea())));
        if (inter instanceof AbstractVerticalInter vertical) {
            final Line2D median = vertical.getMedian();
            sb.append(" vertical ").append(axis(median.getP1(), median.getP2()));
            sb.append(" width ").append(hexDouble(vertical.getWidth()));
        } else if (inter instanceof AbstractHorizontalInter horizontal) {
            final Line2D median = horizontal.getMedian();
            sb.append(" horizontal ").append(axis(median.getP1(), median.getP2()));
            sb.append(" height ").append(hexDouble(horizontal.getHeight()));
        } else {
            sb.append(" geometry -");
        }
        return sb.toString();
    }

    private static String competitorDecision (Inter inter,
                                              int maxStem,
                                              int minBeamWidth)
    {
        if (!inter.isGood()) {
            return "not-good";
        }
        if (inter instanceof AbstractVerticalInter vertical
                && ((int) Math.floor(vertical.getWidth()) <= maxStem)) {
            return "thin-vertical";
        }
        if (inter instanceof AbstractBeamInter beam
                && !beam.getGroup().hasLongBeam(minBeamWidth)) {
            return "short-beam-group";
        }
        return "accept";
    }

    private static String competitorEvidence (Inter inter,
                                              int minBeamWidth)
    {
        if (inter instanceof AbstractVerticalInter vertical) {
            return "vertical-floor:" + (int) Math.floor(vertical.getWidth());
        }
        if (inter instanceof AbstractBeamInter beam) {
            final StringBuilder widths = new StringBuilder();
            for (AbstractBeamInter member : beam.getGroup().getBeams()) {
                if (!widths.isEmpty()) {
                    widths.append(',');
                }
                widths.append(member.getBounds().width);
            }
            return "beam-group:" + beam.getGroup().hasLongBeam(minBeamWidth) + ":" + widths;
        }
        return "-";
    }

    private static String seedBand (NoteHeadsBuilder builder,
                                    Geometry geometry)
        throws Exception
    {
        final Object constants = staticField("constants");
        final double ratio = constantValue(field(constants, "pitchMargin"));
        final double above = (geometry.interline * (geometry.dir - ratio)) / 2;
        final double below = (geometry.interline * (geometry.dir + ratio)) / 2;
        return band(above, below);
    }

    private static String competitorBand (Geometry geometry)
    {
        final double ratio = HeadInter.getShrinkVertRatio();
        final double above = (geometry.interline * (geometry.dir - ratio)) / 2;
        final double below = (geometry.interline * (geometry.dir + ratio)) / 2;
        return band(above, below);
    }

    private static String barBand (NoteHeadsBuilder builder,
                                   Geometry geometry)
        throws Exception
    {
        final double margin = doubleField(field(builder, "params"), "vBarMargin");
        final double above = (geometry.interline * geometry.dir / 2.0) - margin;
        final double below = (geometry.interline * geometry.dir / 2.0) + margin;
        return band(above, below);
    }

    private static String band (double above,
                                double below)
    {
        return hexDouble(above) + ":" + hexDouble(below) + ":" + hexDouble(below + 1);
    }

    private static double constantValue (Object constant)
        throws Exception
    {
        final Method method = constant.getClass().getMethod("getValue");
        return ((Number) method.invoke(constant)).doubleValue();
    }

    private static String axis (Point2D left,
                                Point2D right)
    {
        return hexDouble(left.getX()) + ":" + hexDouble(left.getY()) + ":"
                + hexDouble(right.getX()) + ":" + hexDouble(right.getY());
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/" + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static long runTableHash (RunTable table)
    {
        final List<String> records = new ArrayList<>();
        records.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder record = new StringBuilder().append(sequence);
            for (Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                final Run run = it.next();
                record.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            records.add(record.toString());
        }
        return hash(records);
    }

    private static long hash (Collection<String> records)
    {
        long hash = 0xcbf29ce484222325L;
        for (String record : records) {
            for (byte value : (record + "\n").getBytes(StandardCharsets.UTF_8)) {
                hash ^= Byte.toUnsignedLong(value);
                hash *= 0x100000001b3L;
            }
        }
        return hash;
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) HEADS scanner-context oracle.");
        System.out.println("#");
        System.out.println("# Every target reaches real LEDGERS and runs real HEADS prolog in a fresh JVM.");
        System.out.println("# Private Scanner instances are constructed in processStaff order, but lookup");
        System.out.println("# is never called. Seed and range construction must have identical static geometry.");
        System.out.println("# Competitor/bar Area slicing is intentionally deferred to a later sub-gate.");
        System.out.println("#");
        System.out.println("# Doubles are Double.toHexString/raw-bits. Ordinate vectors are value:length RLE");
        System.out.println("# over every inclusive x in the declared interval. Row summaries are FNV-1a-64");
        System.out.println("# over UTF-8 canonical records with a trailing newline.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM: chula, allegretto, batuque, carmen,");
        System.out.println("# cucaracha, hove, zizi, BachInvention5.");
    }

    private static void printSlicesHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) HEADS base-slice oracle.");
        System.out.println("#");
        System.out.println("# Every target reaches real LEDGERS and runs real HEADS prolog in a fresh JVM.");
        System.out.println("# Scanner constructors run in processStaff order, before lookup mutates the SIG.");
        System.out.println("# The fixture freezes semantic Area bounds and the exact finite rectangle slices");
        System.out.println("# consumed for vertical seeds, head spots, existing competitors, and frozen bars.");
        System.out.println("# Dynamic seed-created HeadInter competitors and template-box overlap are deferred.");
        System.out.println("#");
        System.out.println("# Doubles are Double.toHexString/raw-bits. Source-pool ordinals preserve stable");
        System.out.println("# ordinate/abscissa ordering. Summaries are FNV-1a-64 over canonical UTF-8 rows.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM: chula, allegretto, batuque, carmen,");
        System.out.println("# cucaracha, hove, zizi, BachInvention5.");
    }

    private static void printSeedHeadsHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) HEADS seed-pass oracle.");
        System.out.println("#");
        System.out.println("# Every target reaches real LEDGERS and runs real HEADS prolog in a fresh JVM.");
        System.out.println("# Each non-tablature staff invokes actual processStaff(staff,true) in production");
        System.out.println("# order, then appends returned heads to systemCompetitors and stably ordinate-sorts.");
        System.out.println("# Range lookup, aggregation, duplicate/overlap purge, and linking never run.");
        System.out.println("#");
        System.out.println("# Internally, one headseedattempt row records every seed/side/original-shape loop");
        System.out.println("# iteration. The default runner omits these diagnostic rows after hashing them;");
        System.out.println("# pass --full-trace to retain them in generated output.");
        System.out.println("# Private eval/createInter are replayed against identical live state solely to expose");
        System.out.println("# provenance; their debug counters are restored before the production invocation and");
        System.out.println("# asserted equal afterward. Glyph outcome is predicted by the exact foreground gate.");
        System.out.println("# Compact headseedcandidate rows retain every minimum-grade provisional head, while");
        System.out.println("# Final rows freeze SIG/competitor identity order, inter impacts, registered glyph runs,");
        System.out.println("# and both identity-keyed HeadSeedTally dx values.");
        System.out.println("#");
        System.out.println("# Doubles are Double.toHexString/raw-bits. Run and row digests are FNV-1a-64 over");
        System.out.println("# canonical UTF-8 records with trailing newlines.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM: chula, allegretto, batuque, carmen,");
        System.out.println("# cucaracha, hove, zizi, BachInvention5.");
    }

    private static void printRangeHeadsHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) HEADS range-pass oracle.");
        System.out.println("#");
        System.out.println("# Every target reaches real LEDGERS and runs real HEADS prolog in a fresh JVM.");
        System.out.println("# Each non-tablature staff invokes actual processStaff(staff,true), appends and");
        System.out.println("# ordinate-sorts those seed heads into systemCompetitors, then invokes actual");
        System.out.println("# processStaff(staff,false), exactly as buildHeads. Duplicate and overlap purge,");
        System.out.println("# tally purge, staff-note attachment, beam purge, and linking never run.");
        System.out.println("#");
        System.out.println("# Private range lookup is replayed against identical pre-range state to expose");
        System.out.println("# safety skips, black relevance and shape selection, every evaluation and best");
        System.out.println("# update, abandonment, black-to-void conversion, weak-stemless rejection,");
        System.out.println("# aggregation, dynamic seed-head conflicts, and predicted glyph construction.");
        System.out.println("# Replay counters are restored before production and asserted equal afterward.");
        System.out.println("#");
        System.out.println("# The default runner hashes and omits headrangescan, headrangeattempt, and");
        System.out.println("# headrangerawcandidate diagnostics. Pass --full-trace to retain them.");
        System.out.println("# Compact candidate rows are post-aggregation; final rows freeze production SIG");
        System.out.println("# order, impacts, glyph bounds, weights, and complete vertical-run digests.");
        System.out.println("# Doubles are Double.toHexString/raw-bits. Row and run digests are FNV-1a-64");
        System.out.println("# over canonical UTF-8 records with trailing newlines.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM: chula, allegretto, batuque, carmen,");
        System.out.println("# cucaracha, hove, zizi, BachInvention5.");
    }

    private static final class PerfCounts
    {
        final int bars;

        final int overlaps;

        final int evals;

        final int abandons;

        PerfCounts (int bars,
                    int overlaps,
                    int evals,
                    int abandons)
        {
            this.bars = bars;
            this.overlaps = overlaps;
            this.evals = evals;
            this.abandons = abandons;
        }

        PerfCounts minus (PerfCounts other)
        {
            return new PerfCounts(
                    bars - other.bars,
                    overlaps - other.overlaps,
                    evals - other.evals,
                    abandons - other.abandons);
        }

        @Override
        public String toString ()
        {
            return bars + ":" + overlaps + ":" + evals + ":" + abandons;
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String record)
        {
            for (byte item : (record + "\n").getBytes(StandardCharsets.UTF_8)) {
                value ^= Byte.toUnsignedLong(item);
                value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }

    private static final class RangeStaffTrace
    {
        final List<RangeSpot> spots = new ArrayList<>();

        final List<RangeScan> scans = new ArrayList<>();

        final List<RangeAttempt> attempts = new ArrayList<>();

        final List<RangeCandidate> rawCandidates = new ArrayList<>();

        final List<RangeCandidate> candidates = new ArrayList<>();

        final List<RangeCandidate> finalCandidates = new ArrayList<>();
    }

    private static final class RangeSpot
    {
        final int scannerOrdinal;

        final Source source;

        final int spotOrdinal;

        final int poolOrdinal;

        final Glyph glyph;

        final Rectangle expandedBounds;

        final Integer effectiveLeft;

        final Integer effectiveRight;

        RangeSpot (int scannerOrdinal,
                   Source source,
                   int spotOrdinal,
                   int poolOrdinal,
                   Glyph glyph,
                   Rectangle expandedBounds,
                   Integer effectiveLeft,
                   Integer effectiveRight)
        {
            this.scannerOrdinal = scannerOrdinal;
            this.source = source;
            this.spotOrdinal = spotOrdinal;
            this.poolOrdinal = poolOrdinal;
            this.glyph = glyph;
            this.expandedBounds = expandedBounds;
            this.effectiveLeft = effectiveLeft;
            this.effectiveRight = effectiveRight;
        }
    }

    private static final class RangeScan
    {
        int ordinal;

        final int scannerOrdinal;

        final Source source;

        final int x;

        final int y;

        final int probeX;

        final Integer probeValue;

        final String safety;

        final int nextX;

        final boolean blackRelevant;

        final String shapeChoice;

        final String shapes;

        RangeScan (int scannerOrdinal,
                   Source source,
                   int x,
                   int y,
                   int probeX,
                   Integer probeValue,
                   String safety,
                   int nextX,
                   boolean blackRelevant,
                   String shapeChoice,
                   String shapes)
        {
            this.scannerOrdinal = scannerOrdinal;
            this.source = source;
            this.x = x;
            this.y = y;
            this.probeX = probeX;
            this.probeValue = probeValue;
            this.safety = safety;
            this.nextX = nextX;
            this.blackRelevant = blackRelevant;
            this.shapeChoice = shapeChoice;
            this.shapes = shapes;
        }
    }

    private static final class RangeAttempt
    {
        int ordinal;

        int rawCandidateOrdinal = -1;

        final int scannerOrdinal;

        final Source source;

        final int x;

        final int y;

        final boolean blackRelevant;

        final String shapeChoice;

        final int shapeOrdinal;

        final Shape originalShape;

        final Shape finalShape;

        final String evaluations;

        final PerfCounts perf;

        final int bestUpdates;

        final PixelDistance best;

        final int bestYOrdinal;

        final int bestYValue;

        final Boolean blackToVoid;

        final boolean weakStemless;

        final HeadInter tracedHead;

        final String outcome;

        RangeAttempt (int scannerOrdinal,
                      Source source,
                      int x,
                      int y,
                      boolean blackRelevant,
                      String shapeChoice,
                      int shapeOrdinal,
                      Shape originalShape,
                      Shape finalShape,
                      String evaluations,
                      PerfCounts perf,
                      int bestUpdates,
                      PixelDistance best,
                      int bestYOrdinal,
                      int bestYValue,
                      Boolean blackToVoid,
                      boolean weakStemless,
                      HeadInter tracedHead,
                      String outcome)
        {
            this.scannerOrdinal = scannerOrdinal;
            this.source = source;
            this.x = x;
            this.y = y;
            this.blackRelevant = blackRelevant;
            this.shapeChoice = shapeChoice;
            this.shapeOrdinal = shapeOrdinal;
            this.originalShape = originalShape;
            this.finalShape = finalShape;
            this.evaluations = evaluations;
            this.perf = perf;
            this.bestUpdates = bestUpdates;
            this.best = best;
            this.bestYOrdinal = bestYOrdinal;
            this.bestYValue = bestYValue;
            this.blackToVoid = blackToVoid;
            this.weakStemless = weakStemless;
            this.tracedHead = tracedHead;
            this.outcome = outcome;
        }
    }

    private static final class RangeCandidate
    {
        final int rawOrdinal;

        final RangeAttempt attempt;

        final HeadInter head;

        int aggregateOrdinal = -1;

        int aggregateWinnerRawOrdinal = -1;

        int candidateOrdinal = -1;

        String aggregateMembers = "-";

        String seedConflicts = "-";

        RangeGlyph glyph;

        int finalOrdinal = -1;

        String outcome = "provisional";

        RangeCandidate (int rawOrdinal,
                        RangeAttempt attempt,
                        HeadInter head)
        {
            this.rawOrdinal = rawOrdinal;
            this.attempt = attempt;
            this.head = head;
        }
    }

    private static final class RangeGlyph
    {
        final Rectangle templateBounds;

        final Rectangle foregroundBounds;

        final Rectangle glyphBounds;

        final int weight;

        final long runDigest;

        RangeGlyph (Rectangle templateBounds,
                    Rectangle foregroundBounds,
                    Rectangle glyphBounds,
                    int weight,
                    long runDigest)
        {
            this.templateBounds = templateBounds;
            this.foregroundBounds = foregroundBounds;
            this.glyphBounds = glyphBounds;
            this.weight = weight;
            this.runDigest = runDigest;
        }

        String record ()
        {
            return String.format(
                    "template:%s:foreground:%s:bounds:%s:weight:%d:runs:%016x",
                    rectangle(templateBounds),
                    rectangle(foregroundBounds),
                    rectangle(glyphBounds),
                    weight,
                    runDigest);
        }
    }

    private static final class SeedAttempt
    {
        int ordinal;

        int candidateOrdinal = -1;

        final int scannerOrdinal;

        final Source source;

        final int seedPoolOrdinal;

        final Glyph seed;

        final Anchor side;

        final int shapeOrdinal;

        final Shape originalShape;

        final Shape finalShape;

        final int nominalX;

        final int nominalY;

        final String nominalGate;

        final Double nominalDistance;

        final PerfCounts perf;

        final int bestUpdates;

        final PixelDistance best;

        final int bestYOrdinal;

        final int bestYValue;

        final int bestXOrdinal;

        final int bestXValue;

        final Boolean blackToVoid;

        final boolean minGradePass;

        final HeadInter tracedHead;

        final String outcome;

        SeedAttempt (int scannerOrdinal,
                     Source source,
                     int seedPoolOrdinal,
                     Glyph seed,
                     Anchor side,
                     int shapeOrdinal,
                     Shape originalShape,
                     Shape finalShape,
                     int nominalX,
                     int nominalY,
                     String nominalGate,
                     Double nominalDistance,
                     PerfCounts perf,
                     int bestUpdates,
                     PixelDistance best,
                     int bestYOrdinal,
                     int bestYValue,
                     int bestXOrdinal,
                     int bestXValue,
                     Boolean blackToVoid,
                     boolean minGradePass,
                     HeadInter tracedHead,
                     String outcome)
        {
            this.scannerOrdinal = scannerOrdinal;
            this.source = source;
            this.seedPoolOrdinal = seedPoolOrdinal;
            this.seed = seed;
            this.side = side;
            this.shapeOrdinal = shapeOrdinal;
            this.originalShape = originalShape;
            this.finalShape = finalShape;
            this.nominalX = nominalX;
            this.nominalY = nominalY;
            this.nominalGate = nominalGate;
            this.nominalDistance = nominalDistance;
            this.perf = perf;
            this.bestUpdates = bestUpdates;
            this.best = best;
            this.bestYOrdinal = bestYOrdinal;
            this.bestYValue = bestYValue;
            this.bestXOrdinal = bestXOrdinal;
            this.bestXValue = bestXValue;
            this.blackToVoid = blackToVoid;
            this.minGradePass = minGradePass;
            this.tracedHead = tracedHead;
            this.outcome = outcome;
        }
    }

    private static final class Source
    {
        final String text;

        private Source (String text)
        {
            this.text = text;
        }

        static Source staffLine (int index,
                                 LineInfo line)
        {
            final Point2D left = line.getEndPoint(HorizontalSide.LEFT);
            final Point2D right = line.getEndPoint(HorizontalSide.RIGHT);
            return new Source("staff-line:" + index + ":axis:" + axis(left, right));
        }

        static Source ledger (int index,
                              int ordinal,
                              Glyph glyph)
        {
            final Point2D left = glyph.getStartPoint(Orientation.HORIZONTAL);
            final Point2D right = glyph.getStopPoint(Orientation.HORIZONTAL);
            final Rectangle box = glyph.getBounds();
            return new Source(String.format(
                    "ledger:%d:%d:bounds:%d:%d:%d:%d:weight:%d:runs:%016x:axis:%s",
                    index,
                    ordinal,
                    box.x,
                    box.y,
                    box.width,
                    box.height,
                    glyph.getWeight(),
                    runTableHash(glyph.getRunTable()),
                    axis(left, right)));
        }

        @Override
        public boolean equals (Object obj)
        {
            return (obj instanceof Source other) && text.equals(other.text);
        }

        @Override
        public int hashCode ()
        {
            return text.hashCode();
        }
    }

    private static final class Geometry
    {
        final Object scanner;

        final Source source;

        final int interline;

        final int dir;

        final int pitch;

        final boolean open;

        final int[] yOffsets;

        final String allShapes;

        final String stemShapes;

        final String hollowShapes;

        final int left;

        final int right;

        final int line2Left;

        final int line2Right;

        final List<String> fartherAxes;

        final String ordinateRle;

        final int rangeLeft;

        final int rangeRight;

        final String rangeOrdinateRle;

        Geometry (Object scanner,
                  Source source,
                  int interline,
                  int dir,
                  int pitch,
                  boolean open,
                  int[] yOffsets,
                  String allShapes,
                  String stemShapes,
                  String hollowShapes,
                  int left,
                  int right,
                  int line2Left,
                  int line2Right,
                  List<String> fartherAxes,
                  String ordinateRle,
                  int rangeLeft,
                  int rangeRight,
                  String rangeOrdinateRle)
        {
            this.scanner = scanner;
            this.source = source;
            this.interline = interline;
            this.dir = dir;
            this.pitch = pitch;
            this.open = open;
            this.yOffsets = yOffsets;
            this.allShapes = allShapes;
            this.stemShapes = stemShapes;
            this.hollowShapes = hollowShapes;
            this.left = left;
            this.right = right;
            this.line2Left = line2Left;
            this.line2Right = line2Right;
            this.fartherAxes = fartherAxes;
            this.ordinateRle = ordinateRle;
            this.rangeLeft = rangeLeft;
            this.rangeRight = rangeRight;
            this.rangeOrdinateRle = rangeOrdinateRle;
        }

        @Override
        public boolean equals (Object obj)
        {
            if (!(obj instanceof Geometry other)) {
                return false;
            }
            return source.equals(other.source) && interline == other.interline && dir == other.dir
                    && pitch == other.pitch && open == other.open
                    && Arrays.equals(yOffsets, other.yOffsets) && allShapes.equals(other.allShapes)
                    && stemShapes.equals(other.stemShapes)
                    && hollowShapes.equals(other.hollowShapes) && left == other.left
                    && right == other.right && line2Left == other.line2Left
                    && line2Right == other.line2Right && fartherAxes.equals(other.fartherAxes)
                    && ordinateRle.equals(other.ordinateRle) && rangeLeft == other.rangeLeft
                    && rangeRight == other.rangeRight
                    && rangeOrdinateRle.equals(other.rangeOrdinateRle);
        }

        @Override
        public int hashCode ()
        {
            return source.hashCode();
        }
    }
}
