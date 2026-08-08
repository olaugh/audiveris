// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.PathIterator;
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
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact, identity-free oracle for the seed-selection half of
 * {@code HeadLinker.CLinker.retrieveStump()}.
 *
 * <p>Each invocation reaches the real HEADS boundary in a fresh JVM. The probe constructs a real
 * {@link StemsRetriever}, reads its production no-stem areas, validates an independently recorded
 * purge against the private production {@code purgeNoStemSeeds} method, and validates each
 * independently filtered head-neighbor pool against private production
 * {@code getNeighboringSeeds}. It then replays the short, read-only seed loop with production
 * geometry primitives. It deliberately reports {@code fallback} without invoking
 * {@code buildStump()}, because that method registers a glyph even when its final stands-out gate
 * rejects it.
 */
@SuppressWarnings("unchecked")
public class StemsHeadSeedProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;

    private static final Field RETRIEVER_SYSTEM_SEEDS;

    private static final Field RETRIEVER_NO_STEM_AREAS;

    private static final Method PURGE_NO_STEM_SEEDS;

    private static final Method GET_NEIGHBORING_SEEDS;

    private static final Field PARAM_MAX_BAR_OVERLAP;

    private static final Field PARAM_VICINITY_MARGIN;

    private static final Field PARAM_MAX_HEAD_SEED_DY;

    private static final Field PARAM_MAX_HEAD_OUT_DX;

    private static final Field PARAM_MAX_HEAD_IN_DX;

    private static final Field PARAM_MIN_HEAD_STUMP_DY;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);

            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_NO_STEM_AREAS = declaredField(StemsRetriever.class, "noStemAreas");

            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            GET_NEIGHBORING_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "getNeighboringSeeds", Rectangle.class);
            GET_NEIGHBORING_SEEDS.setAccessible(true);

            PARAM_MAX_BAR_OVERLAP = declaredField(parameters, "maxBarOverlap");
            PARAM_VICINITY_MARGIN = declaredField(parameters, "vicinityMargin");
            PARAM_MAX_HEAD_SEED_DY = declaredField(parameters, "maxHeadSeedDy");
            PARAM_MAX_HEAD_OUT_DX = declaredField(parameters, "maxHeadOutDx");
            PARAM_MAX_HEAD_IN_DX = declaredField(parameters, "maxHeadInDx");
            PARAM_MIN_HEAD_STUMP_DY = declaredField(parameters, "minHeadStumpDy");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsHeadSeedProbe ()
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
                "stemsheadseedpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());

        for (SystemInfo system : sheet.getSystems()) {
            final StemsRetriever retriever = new StemsRetriever(system);
            final Object parameters = PARAMETERS_CONSTRUCTOR.newInstance(system, sheet.getScale());
            RETRIEVER_PARAMS.set(retriever, parameters);

            final double maxBarOverlap = PARAM_MAX_BAR_OVERLAP.getDouble(parameters);
            final int vicinityMargin = PARAM_VICINITY_MARGIN.getInt(parameters);
            final int maxHeadSeedDy = PARAM_MAX_HEAD_SEED_DY.getInt(parameters);
            final int maxHeadOutDx = PARAM_MAX_HEAD_OUT_DX.getInt(parameters);
            final int maxHeadInDx = PARAM_MAX_HEAD_IN_DX.getInt(parameters);
            final int minHeadStumpDy = PARAM_MIN_HEAD_STUMP_DY.getInt(parameters);

            final List<Area> noStemAreas = new ArrayList<>(
                    (List<Area>) RETRIEVER_NO_STEM_AREAS.get(retriever));
            final List<Glyph> sourceSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
            final PurgeResult purge = replayPurge(sourceSeeds, noStemAreas, maxBarOverlap);

            // Validate the recorded independent purge against the exact private production method.
            final List<Glyph> productionPurged = new ArrayList<>(sourceSeeds);
            PURGE_NO_STEM_SEEDS.invoke(retriever, productionPurged);
            requireSameIdentityOrder(
                    page + " system " + system.getId() + " purged seed pool",
                    purge.kept,
                    productionPurged);
            RETRIEVER_SYSTEM_SEEDS.set(retriever, productionPurged);

            final List<Inter> inSigOrder = system.getSig().inters(
                    ShapeSet.getTemplateNotesStem(sheet));
            final IdentityHashMap<HeadInter, Integer> sigOrdinals = new IdentityHashMap<>();
            final List<HeadInter> heads = new ArrayList<>(inSigOrder.size());
            for (int ordinal = 0; ordinal < inSigOrder.size(); ordinal++) {
                final HeadInter head = (HeadInter) inSigOrder.get(ordinal);
                if (head.isRemoved()) {
                    throw new IllegalStateException("STEMS selected a removed head: " + head);
                }
                sigOrdinals.put(head, ordinal);
                heads.add(head);
            }
            Collections.sort(heads, (one, two) -> Inters.byAbscissa.compare(one, two));

            final IdentityHashMap<Glyph, Integer> sourceOrdinals = identityOrdinals(sourceSeeds);
            final IdentityHashMap<Glyph, Integer> keptOrdinals = identityOrdinals(purge.kept);
            final RowHasher systemHash = new RowHasher();
            final Totals totals = new Totals();
            totals.sourceSeeds = sourceSeeds.size();
            totals.keptSeeds = purge.kept.size();
            totals.purgedSeeds = sourceSeeds.size() - purge.kept.size();
            totals.noStemAreas = noStemAreas.size();
            totals.heads = heads.size();

            emit(String.format(
                    "stemsheadseedsystem %s system %d profile %d interline %d bounds %s "
                            + "maxBarOverlap %s vicinityMargin %d maxHeadSeedDy %d "
                            + "maxHeadInDx %d maxHeadOutDx %d minHeadStumpDy %d "
                            + "sourceSeeds %d keptSeeds %d noStemAreas %d heads %d",
                    page,
                    system.getId(),
                    system.getProfile(),
                    sheet.getScale().getInterline(),
                    rectangle(system.getBounds()),
                    hexDouble(maxBarOverlap),
                    vicinityMargin,
                    maxHeadSeedDy,
                    maxHeadInDx,
                    maxHeadOutDx,
                    minHeadStumpDy,
                    sourceSeeds.size(),
                    purge.kept.size(),
                    noStemAreas.size(),
                    heads.size()), systemHash, pageHash);

            for (int areaOrdinal = 0; areaOrdinal < noStemAreas.size(); areaOrdinal++) {
                final Area area = noStemAreas.get(areaOrdinal);
                final PathEvidence evidence = pathEvidence(area);
                final Rectangle2D box = area.getBounds2D();
                emit(String.format(
                        "stemsheadseednostem %s system %d ordinal %d bounds %s winding %d "
                                + "segments %d pathHash %016x",
                        page,
                        system.getId(),
                        areaOrdinal,
                        rectangle2D(box),
                        evidence.windingRule,
                        evidence.segmentCount,
                        evidence.hash), systemHash, pageHash);
            }

            for (PurgeDecision decision : purge.decisions) {
                final Glyph seed = sourceSeeds.get(decision.sourceOrdinal);
                final Line2D centerLine = requireCenterLine(seed);
                emit(String.format(
                        "stemsheadseedfree %s system %d sourceOrdinal %d kept %s keptOrdinal %s "
                                + "bounds %s weight %d runs %d:%016x centerLine %s",
                        page,
                        system.getId(),
                        decision.sourceOrdinal,
                        decision.kept,
                        decision.kept ? Integer.toString(decision.keptOrdinal) : "-",
                        rectangle(seed.getBounds()),
                        seed.getWeight(),
                        runCount(seed),
                        runTableHash(seed),
                        line(centerLine)), systemHash, pageHash);

                for (PurgeCheck check : decision.checks) {
                    totals.purgeChecks++;
                    final String row;
                    if (check.intersects) {
                        row = String.format(
                                "stemsheadseedpurgecheck %s system %d sourceOrdinal %d "
                                        + "checkOrdinal %d areaOrdinal %d intersects true "
                                        + "seedSize %s interBounds %s interSize %s barBounds %s "
                                        + "barSize %s denominator %s ratio %s threshold %s purge %s",
                                page,
                                system.getId(),
                                decision.sourceOrdinal,
                                check.checkOrdinal,
                                check.areaOrdinal,
                                hexDouble(check.seedSize),
                                rectangle2D(check.intersectionBounds),
                                hexDouble(check.intersectionSize),
                                rectangle2D(check.barBounds),
                                hexDouble(check.barSize),
                                hexDouble(check.denominator),
                                hexDouble(check.ratio),
                                hexDouble(maxBarOverlap),
                                check.purge);
                    } else {
                        row = String.format(
                                "stemsheadseedpurgecheck %s system %d sourceOrdinal %d "
                                        + "checkOrdinal %d areaOrdinal %d intersects false "
                                        + "seedSize %s purge false",
                                page,
                                system.getId(),
                                decision.sourceOrdinal,
                                check.checkOrdinal,
                                check.areaOrdinal,
                                hexDouble(check.seedSize));
                    }
                    emit(row, systemHash, pageHash);
                }

                emit(String.format(
                        "stemsheadseedpurgedecision %s system %d sourceOrdinal %d checkedAreas %d "
                                + "outcome %s",
                        page,
                        system.getId(),
                        decision.sourceOrdinal,
                        decision.checks.size(),
                        decision.kept ? "kept:" + decision.keptOrdinal
                                : "purged:" + decision.purgeAreaOrdinal), systemHash, pageHash);
            }

            for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
                final HeadInter head = heads.get(headOrdinal);
                final Rectangle headBox = head.getBounds();
                final Rectangle systemBox = system.getBounds();
                final Rectangle fatBox = new Rectangle(
                        headBox.x, systemBox.y, headBox.width, systemBox.height);
                fatBox.grow(vicinityMargin, 0);
                final List<Glyph> neighbors = new ArrayList<>(
                        Glyphs.intersectedGlyphs(purge.kept, fatBox));

                // Validate the per-head pool against production with the production fields set.
                final Set<Glyph> productionNeighbors = (Set<Glyph>) GET_NEIGHBORING_SEEDS.invoke(
                        retriever, headBox);
                requireSameIdentityOrder(
                        page + " system " + system.getId() + " head " + headOrdinal
                                + " neighbor pool",
                        neighbors,
                        new ArrayList<>(productionNeighbors));

                emit(String.format(
                        "stemsheadseedhead %s system %d ordinal %d sigOrdinal %d staff %d "
                                + "shape %s bounds %s grade %s fatBox %s neighbors %d",
                        page,
                        system.getId(),
                        headOrdinal,
                        sigOrdinals.get(head),
                        head.getStaff().getId(),
                        head.getShape(),
                        rectangle(headBox),
                        hexDouble(head.getGrade()),
                        rectangle(fatBox),
                        neighbors.size()), systemHash, pageHash);

                int constructorOrdinal = 0;
                for (HorizontalSide horizontal : HorizontalSide.values()) {
                    final int xDir = horizontal.direction();
                    for (VerticalSide vertical : VerticalSide.values()) {
                        final int yDir = vertical.direction();
                        final Point2D reference = head.getStemReferencePoint(horizontal, vertical);
                        final Point2D outPoint = new Point2D.Double(
                                reference.getX() + (xDir * maxHeadOutDx), reference.getY());
                        final Point2D inPoint = new Point2D.Double(
                                reference.getX() - (xDir * maxHeadInDx), reference.getY());
                        final Point2D left = (xDir > 0) ? inPoint : outPoint;
                        final Point2D right = (xDir > 0) ? outPoint : inPoint;
                        final Rectangle2D seedRect = new Rectangle2D.Double(
                                left.getX(),
                                left.getY() - maxHeadSeedDy,
                                right.getX() - left.getX(),
                                2.0 * maxHeadSeedDy);
                        final Area seedArea = new Area(seedRect);
                        final List<Glyph> intersected = new ArrayList<>(
                                Glyphs.intersectedGlyphs(neighbors, seedArea));
                        final IdentityHashMap<Glyph, Integer> intersectionOrdinals =
                                identityOrdinals(intersected);

                        final List<SeedCandidate> candidates = new ArrayList<>(intersected.size());
                        for (int inputOrdinal = 0; inputOrdinal < intersected.size(); inputOrdinal++) {
                            final Glyph seed = intersected.get(inputOrdinal);
                            final Line2D centerLine = requireCenterLine(seed);
                            candidates.add(new SeedCandidate(
                                    seed,
                                    inputOrdinal,
                                    centerLine,
                                    centerLine.ptSegDistSq(reference)));
                        }
                        if (candidates.size() > 1) {
                            Collections.sort(candidates, Comparator.comparingDouble(
                                    candidate -> candidate.distanceSq));
                        }

                        int selectedSortOrdinal = -1;
                        int selectedPoolOrdinal = -1;
                        int visited = 0;
                        for (int sortOrdinal = 0; sortOrdinal < candidates.size(); sortOrdinal++) {
                            final SeedCandidate candidate = candidates.get(sortOrdinal);
                            candidate.evaluate(
                                    reference,
                                    xDir,
                                    yDir,
                                    maxHeadOutDx,
                                    maxHeadInDx,
                                    minHeadStumpDy);
                            candidate.visited = selectedSortOrdinal < 0;
                            if (candidate.visited) {
                                visited++;
                                if (candidate.wouldAccept) {
                                    selectedSortOrdinal = sortOrdinal;
                                    selectedPoolOrdinal = sourceOrdinals.get(candidate.seed);
                                    candidate.selected = true;
                                }
                            }
                        }

                        totals.corners++;
                        totals.neighborRows += neighbors.size();
                        totals.candidates += candidates.size();
                        totals.visited += visited;
                        if (selectedSortOrdinal >= 0) {
                            totals.selected++;
                        } else {
                            totals.fallback++;
                        }

                        emit(String.format(
                                "stemsheadseedcorner %s system %d head %d ctorOrdinal %d "
                                        + "inspectOrdinal %d corner %s xDir %d yDir %d ref %s "
                                        + "out %s in %s seedRect %s neighbors %d intersected %d "
                                        + "candidates %d",
                                page,
                                system.getId(),
                                headOrdinal,
                                constructorOrdinal,
                                inspectionOrdinal(horizontal, vertical),
                                corner(horizontal, vertical),
                                xDir,
                                yDir,
                                point(reference),
                                point(outPoint),
                                point(inPoint),
                                rectangle2D(seedRect),
                                neighbors.size(),
                                intersected.size(),
                                candidates.size()), systemHash, pageHash);

                        for (int neighborOrdinal = 0; neighborOrdinal < neighbors.size();
                                neighborOrdinal++) {
                            final Glyph seed = neighbors.get(neighborOrdinal);
                            final Integer intersectionOrdinal = intersectionOrdinals.get(seed);
                            emit(String.format(
                                    "stemsheadseedneighbor %s system %d head %d ctorOrdinal %d "
                                            + "neighborOrdinal %d sourceOrdinal %d keptOrdinal %d "
                                            + "bounds %s intersects %s intersectionOrdinal %s",
                                    page,
                                    system.getId(),
                                    headOrdinal,
                                    constructorOrdinal,
                                    neighborOrdinal,
                                    sourceOrdinals.get(seed),
                                    keptOrdinals.get(seed),
                                    rectangle(seed.getBounds()),
                                    intersectionOrdinal != null,
                                    intersectionOrdinal != null
                                            ? intersectionOrdinal.toString() : "-"),
                                    systemHash,
                                    pageHash);
                        }

                        for (int sortOrdinal = 0; sortOrdinal < candidates.size(); sortOrdinal++) {
                            final SeedCandidate candidate = candidates.get(sortOrdinal);
                            final Rectangle seedBox = candidate.seed.getBounds();
                            emit(String.format(
                                    "stemsheadseedcandidate %s system %d head %d ctorOrdinal %d "
                                            + "sortOrdinal %d inputOrdinal %d sourceOrdinal %d "
                                            + "keptOrdinal %d bounds %s centerLine %s distanceSq %s "
                                            + "seedX %s signedDelta %s dx %d refY %d extDy %d "
                                            + "horizontalGate %s standsOut %s "
                                            + "standsOutEvaluated %s wouldAccept %s visited %s "
                                            + "selected %s",
                                    page,
                                    system.getId(),
                                    headOrdinal,
                                    constructorOrdinal,
                                    sortOrdinal,
                                    candidate.inputOrdinal,
                                    sourceOrdinals.get(candidate.seed),
                                    keptOrdinals.get(candidate.seed),
                                    rectangle(seedBox),
                                    line(candidate.centerLine),
                                    hexDouble(candidate.distanceSq),
                                    hexDouble(candidate.seedX),
                                    hexDouble(candidate.signedDelta),
                                    candidate.dx,
                                    candidate.refY,
                                    candidate.extDy,
                                    candidate.horizontalGate,
                                    candidate.standsOut,
                                    candidate.visited && candidate.horizontalGate,
                                    candidate.wouldAccept,
                                    candidate.visited,
                                    candidate.selected), systemHash, pageHash);
                        }

                        emit(String.format(
                                "stemsheadseeddecision %s system %d head %d ctorOrdinal %d "
                                        + "corner %s visited %d outcome %s selectedSortOrdinal %s",
                                page,
                                system.getId(),
                                headOrdinal,
                                constructorOrdinal,
                                corner(horizontal, vertical),
                                visited,
                                selectedSortOrdinal >= 0
                                        ? "selected:" + selectedPoolOrdinal : "fallback",
                                selectedSortOrdinal >= 0
                                        ? Integer.toString(selectedSortOrdinal) : "-"),
                                systemHash,
                                pageHash);
                        constructorOrdinal++;
                    }
                }
                if (constructorOrdinal != 4) {
                    throw new IllegalStateException("head did not produce four constructor corners");
                }
            }

            if (totals.corners != 4 * totals.heads) {
                throw new IllegalStateException("system corner count mismatch");
            }
            System.out.printf(
                    "stemsheadseedsystemsummary %s system %d sourceSeeds %d keptSeeds %d "
                            + "purgedSeeds %d noStemAreas %d purgeChecks %d heads %d corners %d "
                            + "neighborRows %d candidates %d visited %d selected %d fallback %d "
                            + "hash %016x%n",
                    page,
                    system.getId(),
                    totals.sourceSeeds,
                    totals.keptSeeds,
                    totals.purgedSeeds,
                    totals.noStemAreas,
                    totals.purgeChecks,
                    totals.heads,
                    totals.corners,
                    totals.neighborRows,
                    totals.candidates,
                    totals.visited,
                    totals.selected,
                    totals.fallback,
                    systemHash.value());
            pageTotals.include(totals);
        }

        System.out.printf(
                "stemsheadseedpagesummary %s systems %d sourceSeeds %d keptSeeds %d purgedSeeds %d "
                        + "noStemAreas %d purgeChecks %d heads %d corners %d neighborRows %d "
                        + "candidates %d visited %d selected %d fallback %d hash %016x%n",
                page,
                sheet.getSystems().size(),
                pageTotals.sourceSeeds,
                pageTotals.keptSeeds,
                pageTotals.purgedSeeds,
                pageTotals.noStemAreas,
                pageTotals.purgeChecks,
                pageTotals.heads,
                pageTotals.corners,
                pageTotals.neighborRows,
                pageTotals.candidates,
                pageTotals.visited,
                pageTotals.selected,
                pageTotals.fallback,
                pageHash.value());
    }

    private static PurgeResult replayPurge (List<Glyph> sourceSeeds,
                                            List<Area> noStemAreas,
                                            double maxBarOverlap)
    {
        final List<Glyph> kept = new ArrayList<>();
        final List<PurgeDecision> decisions = new ArrayList<>(sourceSeeds.size());

        for (int sourceOrdinal = 0; sourceOrdinal < sourceSeeds.size(); sourceOrdinal++) {
            final Glyph seed = sourceSeeds.get(sourceOrdinal);
            final Rectangle seedBox = seed.getBounds();
            final double seedSize = seedBox.width * seedBox.height;
            final List<PurgeCheck> checks = new ArrayList<>();
            int purgeAreaOrdinal = -1;

            for (int areaOrdinal = 0; areaOrdinal < noStemAreas.size(); areaOrdinal++) {
                final Area noStem = noStemAreas.get(areaOrdinal);
                final boolean intersects = noStem.intersects(seedBox);
                if (!intersects) {
                    checks.add(PurgeCheck.noIntersection(
                            checks.size(), areaOrdinal, seedSize));
                    continue;
                }

                final Area intersection = new Area(seedBox);
                intersection.intersect(noStem);
                final Rectangle2D intersectionBounds = intersection.getBounds2D();
                final double intersectionSize = intersectionBounds.getWidth()
                        * intersectionBounds.getHeight();
                final Rectangle2D barBounds = noStem.getBounds2D();
                final double barSize = barBounds.getWidth() * barBounds.getHeight();
                final double denominator = Math.min(barSize, seedSize);
                final double ratio = intersectionSize / denominator;
                final boolean purge = ratio > maxBarOverlap;
                checks.add(PurgeCheck.intersection(
                        checks.size(),
                        areaOrdinal,
                        seedSize,
                        intersectionBounds,
                        intersectionSize,
                        barBounds,
                        barSize,
                        denominator,
                        ratio,
                        purge));
                if (purge) {
                    purgeAreaOrdinal = areaOrdinal;
                    break;
                }
            }

            final boolean isKept = purgeAreaOrdinal < 0;
            final int keptOrdinal = isKept ? kept.size() : -1;
            if (isKept) {
                kept.add(seed);
            }
            decisions.add(new PurgeDecision(
                    sourceOrdinal, checks, isKept, keptOrdinal, purgeAreaOrdinal));
        }
        return new PurgeResult(kept, decisions);
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

    private static void requireSameIdentityOrder (String label,
                                                  List<?> expected,
                                                  List<?> actual)
    {
        if (expected.size() != actual.size()) {
            throw new IllegalStateException(
                    label + " sizes differ: " + expected.size() + " != " + actual.size());
        }
        for (int index = 0; index < expected.size(); index++) {
            if (expected.get(index) != actual.get(index)) {
                throw new IllegalStateException(label + " differs at ordinal " + index);
            }
        }
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

    private static Line2D requireCenterLine (Glyph glyph)
    {
        final Line2D line = glyph.getCenterLine();
        if (line == null) {
            throw new IllegalStateException("vertical seed has no center line");
        }
        return line;
    }

    private static Field declaredField (Class<?> owner,
                                        String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static PathEvidence pathEvidence (Area area)
    {
        final PathIterator iterator = area.getPathIterator(null);
        final int windingRule = iterator.getWindingRule();
        final RowHasher hash = new RowHasher();
        final double[] coordinates = new double[6];
        int segments = 0;

        while (!iterator.isDone()) {
            final int kind = iterator.currentSegment(coordinates);
            final int count = switch (kind) {
                case PathIterator.SEG_MOVETO, PathIterator.SEG_LINETO -> 2;
                case PathIterator.SEG_QUADTO -> 4;
                case PathIterator.SEG_CUBICTO -> 6;
                case PathIterator.SEG_CLOSE -> 0;
                default -> throw new IllegalStateException("unknown path segment " + kind);
            };
            final StringBuilder row = new StringBuilder().append(kind);
            for (int index = 0; index < count; index++) {
                row.append(':').append(hexDouble(coordinates[index]));
            }
            hash.add(row.toString());
            segments++;
            iterator.next();
        }
        return new PathEvidence(windingRule, segments, hash.value());
    }

    private static int inspectionOrdinal (HorizontalSide horizontal,
                                          VerticalSide vertical)
    {
        if ((horizontal == HorizontalSide.RIGHT) && (vertical == VerticalSide.TOP)) {
            return 0;
        }
        if ((horizontal == HorizontalSide.LEFT) && (vertical == VerticalSide.BOTTOM)) {
            return 1;
        }
        if ((horizontal == HorizontalSide.LEFT) && (vertical == VerticalSide.TOP)) {
            return 2;
        }
        return 3;
    }

    private static String corner (HorizontalSide horizontal,
                                  VerticalSide vertical)
    {
        return "" + vertical.name().charAt(0) + horizontal.name().charAt(0);
    }

    private static String rectangle (Rectangle box)
    {
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
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
            for (Iterator<org.audiveris.omr.run.Run> it = table.iterator(sequence); it.hasNext();) {
                it.next();
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
            for (Iterator<org.audiveris.omr.run.Run> it = table.iterator(sequence); it.hasNext();) {
                final var run = it.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            hash.add(row.toString());
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
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS head-seed oracle.");
        System.out.println("#");
        System.out.println("# Each target reaches real HEADS in a fresh JVM and constructs a real");
        System.out.println("# StemsRetriever. The recorded no-stem purge is checked against its private");
        System.out.println("# production method; every head neighbor pool is likewise checked against");
        System.out.println("# production getNeighboringSeeds.");
        System.out.println("#");
        System.out.println("# Free rows are the VERTICAL_SEED-filtered view of SystemInfo free glyphs in");
        System.out.println("# LinkedHashSet insertion order. Purge checks stop at the first rejecting area.");
        System.out.println("# Head and corner order is the exact HeadLinker constructor order: stable head-x,");
        System.out.println("# then TL, BL, TR, BR. inspectOrdinal records later TR, BL, TL, BR order.");
        System.out.println("#");
        System.out.println("# Candidate rows retain stable seed-area input order, distance-sort order, exact");
        System.out.println("# center lines/doubles, Java rounding, both gates, and whether production visits");
        System.out.println("# the row before its first success. Fallback is reported without buildStump, so");
        System.out.println("# this probe never registers a STEMS glyph or mutates the SIG.");
        System.out.println("#");
        System.out.println("# All identities are boundary-local ordinals. Doubles include hex text/raw bits.");
        System.out.println("# FNV-1a-64 hashes cover canonical UTF-8 semantic rows with trailing newlines.");
        System.out.println("# The runner appends SHA-256 commitments over this source and emitted body.");
    }

    private static final class SeedCandidate
    {
        final Glyph seed;

        final int inputOrdinal;

        final Line2D centerLine;

        final double distanceSq;

        double seedX;

        double signedDelta;

        int dx;

        int refY;

        int extDy;

        boolean horizontalGate;

        boolean standsOut;

        boolean wouldAccept;

        boolean visited;

        boolean selected;

        SeedCandidate (Glyph seed,
                       int inputOrdinal,
                       Line2D centerLine,
                       double distanceSq)
        {
            this.seed = seed;
            this.inputOrdinal = inputOrdinal;
            this.centerLine = centerLine;
            this.distanceSq = distanceSq;
        }

        void evaluate (Point2D reference,
                       int xDir,
                       int yDir,
                       int maxHeadOutDx,
                       int maxHeadInDx,
                       int minHeadStumpDy)
        {
            seedX = LineUtil.xAtY(centerLine, reference.getY());
            signedDelta = xDir * (seedX - reference.getX());
            dx = (int) Math.round(signedDelta);
            horizontalGate = ((dx >= 0) && (dx <= maxHeadOutDx))
                    || ((dx <= 0) && (-dx <= maxHeadInDx));
            final Rectangle glyphBox = seed.getBounds();
            refY = (int) Math.rint(reference.getY());
            extDy = (yDir > 0)
                    ? glyphBox.y + glyphBox.height - 1 - refY
                    : refY - glyphBox.y;
            standsOut = extDy >= minHeadStumpDy;
            wouldAccept = horizontalGate && standsOut;
        }
    }

    private record PurgeResult(List<Glyph> kept, List<PurgeDecision> decisions)
    {
    }

    private record PurgeDecision(
            int sourceOrdinal,
            List<PurgeCheck> checks,
            boolean kept,
            int keptOrdinal,
            int purgeAreaOrdinal)
    {
    }

    private static final class PurgeCheck
    {
        final int checkOrdinal;

        final int areaOrdinal;

        final boolean intersects;

        final double seedSize;

        final Rectangle2D intersectionBounds;

        final double intersectionSize;

        final Rectangle2D barBounds;

        final double barSize;

        final double denominator;

        final double ratio;

        final boolean purge;

        private PurgeCheck (int checkOrdinal,
                            int areaOrdinal,
                            boolean intersects,
                            double seedSize,
                            Rectangle2D intersectionBounds,
                            double intersectionSize,
                            Rectangle2D barBounds,
                            double barSize,
                            double denominator,
                            double ratio,
                            boolean purge)
        {
            this.checkOrdinal = checkOrdinal;
            this.areaOrdinal = areaOrdinal;
            this.intersects = intersects;
            this.seedSize = seedSize;
            this.intersectionBounds = intersectionBounds;
            this.intersectionSize = intersectionSize;
            this.barBounds = barBounds;
            this.barSize = barSize;
            this.denominator = denominator;
            this.ratio = ratio;
            this.purge = purge;
        }

        static PurgeCheck noIntersection (int checkOrdinal,
                                          int areaOrdinal,
                                          double seedSize)
        {
            return new PurgeCheck(
                    checkOrdinal, areaOrdinal, false, seedSize, null, 0, null, 0, 0, 0, false);
        }

        static PurgeCheck intersection (int checkOrdinal,
                                        int areaOrdinal,
                                        double seedSize,
                                        Rectangle2D intersectionBounds,
                                        double intersectionSize,
                                        Rectangle2D barBounds,
                                        double barSize,
                                        double denominator,
                                        double ratio,
                                        boolean purge)
        {
            return new PurgeCheck(
                    checkOrdinal,
                    areaOrdinal,
                    true,
                    seedSize,
                    intersectionBounds,
                    intersectionSize,
                    barBounds,
                    barSize,
                    denominator,
                    ratio,
                    purge);
        }
    }

    private record PathEvidence(int windingRule, int segmentCount, long hash)
    {
    }

    private static final class Totals
    {
        int sourceSeeds;

        int keptSeeds;

        int purgedSeeds;

        int noStemAreas;

        int purgeChecks;

        int heads;

        int corners;

        int neighborRows;

        int candidates;

        int visited;

        int selected;

        int fallback;

        void include (Totals other)
        {
            sourceSeeds += other.sourceSeeds;
            keptSeeds += other.keptSeeds;
            purgedSeeds += other.purgedSeeds;
            noStemAreas += other.noStemAreas;
            purgeChecks += other.purgeChecks;
            heads += other.heads;
            corners += other.corners;
            neighborRows += other.neighborRows;
            candidates += other.candidates;
            visited += other.visited;
            selected += other.selected;
            fallback += other.fallback;
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
