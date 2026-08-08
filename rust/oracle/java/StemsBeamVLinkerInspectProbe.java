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
import java.util.ArrayList;
import java.util.Collections;
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
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.math.GeoOrder;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.BeamStemRelation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact identity-free oracle for the read-only prefix of {@code BeamLinker.inspectVLinkers()}.
 *
 * <p>The page reaches real HEADS. This probe installs the same retriever inputs, constructs every
 * live beam linker and then every head linker in the exact stable-x order, and visits beam B/V
 * linkers in production order. For each VLinker it invokes the real private {@code filterBeams}
 * completely before the real private {@code filterHeads}. It deliberately stops before the next
 * statement, {@code new StemBuilder(...)}, and verifies that every V/C builder and every seed pool
 * remains untouched. Cross-beam {@code findLinker} anchor append/reuse is therefore the only
 * mutation at this boundary.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamVLinkerInspectProbe
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

    private static final Field PARAM_VICINITY_MARGIN;

    private static final Field PARAM_MAX_BEAM_GROUP_DY;

    private static final Field PARAM_SLOPE_MARGIN;

    private static final Field PARAM_HALF_BEAM_LU_DX;

    private static final Field PARAM_MAX_BEAM_SEED_DY_RATIO;

    private static final Field LINKER_ALL_B;

    private static final Field B_ID;

    private static final Field B_H_SIDE;

    private static final Field B_REF_PT;

    private static final Field B_STUMP;

    private static final Field B_IS_ANCHOR;

    private static final Field B_V_LINKERS;

    private static final Field V_V_SIDE;

    private static final Field V_Y_DIR;

    private static final Field V_LU_AREA;

    private static final Field V_THEO_LINE;

    private static final Field V_SEEDS;

    private static final Field V_STEM_BUILDER;

    private static final Method V_FILTER_BEAMS;

    private static final Method V_FILTER_HEADS;

    private static final Field C_STEM_BUILDER;

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
            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = declaredField(StemsRetriever.class, "systemBeams");
            RETRIEVER_SYSTEM_HEADS = declaredField(StemsRetriever.class, "systemHeads");
            RETRIEVER_STEM_CHECKER = declaredField(StemsRetriever.class, "stemChecker");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            PARAM_MAX_BEAM_LINKER_DX = declaredField(parameters, "maxBeamLinkerDx");
            PARAM_MAX_BEAM_SIDE_DX = declaredField(parameters, "maxBeamSideDx");
            PARAM_MIN_BEAM_HEAD_DY = declaredField(parameters, "minBeamHeadDy");
            PARAM_VICINITY_MARGIN = declaredField(parameters, "vicinityMargin");
            PARAM_MAX_BEAM_GROUP_DY = declaredField(parameters, "maxBeamGroupDy");
            PARAM_SLOPE_MARGIN = declaredField(parameters, "slopeMargin");
            PARAM_HALF_BEAM_LU_DX = declaredField(parameters, "halfBeamLuDx");
            PARAM_MAX_BEAM_SEED_DY_RATIO = declaredField(parameters, "maxBeamSeedDyRatio");

            LINKER_ALL_B = declaredField(BeamLinker.class, "allBLinkers");
            B_ID = declaredField(bLinker, "id");
            B_H_SIDE = declaredField(bLinker, "hSide");
            B_REF_PT = declaredField(bLinker, "refPt");
            B_STUMP = declaredField(bLinker, "stump");
            B_IS_ANCHOR = declaredField(bLinker, "isAnchor");
            B_V_LINKERS = declaredField(bLinker, "vLinkers");

            V_V_SIDE = declaredField(vLinker, "vSide");
            V_Y_DIR = declaredField(vLinker, "yDir");
            V_LU_AREA = declaredField(vLinker, "luArea");
            V_THEO_LINE = declaredField(vLinker, "theoLine");
            V_SEEDS = declaredField(vLinker, "seeds");
            V_STEM_BUILDER = declaredField(vLinker, "sb");
            V_FILTER_BEAMS = vLinker.getDeclaredMethod("filterBeams", List.class);
            V_FILTER_BEAMS.setAccessible(true);
            V_FILTER_HEADS = vLinker.getDeclaredMethod("filterHeads", List.class);
            V_FILTER_HEADS.setAccessible(true);

            C_STEM_BUILDER = declaredField(cLinker, "sb");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamVLinkerInspectProbe ()
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
                "stemsbeamvinspectpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, totals, hash);
        }
        System.out.printf(
                "stemsbeamvinspectpagesummary %s systems %d beams %d smallBeams %d heads %d "
                        + "smallHeads %d corners %d "
                        + "initialBs %d initialVs %d beamStarts %d bVisits %d anchorSkips %d "
                        + "inspectedVs %d "
                        + "siblingScans %d siblingHits %d beamCandidates %d eligibleBeams %d "
                        + "findCalls %d findCandidates %d bReuses %d anchorReuses %d "
                        + "anchorsCreated %d headLookups %d headScans %d headAreaHits %d "
                        + "headCompetingPasses %d headCompetingRemovals %d "
                        + "headCompetitorDrops %d smallHeadCandidates %d sizeDrops %d "
                        + "headNearDrops %d cornerChecks %d cornerInside %d voidCornerDrops %d "
                        + "cAccepted %d resultBs %d resultCs %d finalBs %d seedSnapshots %d "
                        + "builderChecks %d hash %016x%n",
                page,
                sheet.getSystems().size(),
                totals.beams,
                totals.smallBeams,
                totals.heads,
                totals.smallHeads,
                totals.corners,
                totals.initialBs,
                totals.initialVs,
                totals.beamStarts,
                totals.bVisits,
                totals.anchorSkips,
                totals.inspectedVs,
                totals.siblingScans,
                totals.siblingHits,
                totals.beamCandidates,
                totals.eligibleBeams,
                totals.findCalls,
                totals.findCandidates,
                totals.bReuses,
                totals.anchorReuses,
                totals.anchorsCreated,
                totals.headLookups,
                totals.headScans,
                totals.headAreaHits,
                totals.headCompetingPasses,
                totals.headCompetingRemovals,
                totals.headCompetitorDrops,
                totals.smallHeadCandidates,
                totals.smallHeadDrops,
                totals.headNearDrops,
                totals.cornerChecks,
                totals.cornerInside,
                totals.voidCornerDrops,
                totals.cAccepted,
                totals.resultBs,
                totals.resultCs,
                totals.finalBs,
                totals.seedSnapshots,
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
        final int vicinityMargin = PARAM_VICINITY_MARGIN.getInt(params);
        final int maxBeamGroupDy = PARAM_MAX_BEAM_GROUP_DY.getInt(params);
        final double slopeMargin = PARAM_SLOPE_MARGIN.getDouble(params);
        final double halfBeamLuDx = PARAM_HALF_BEAM_LU_DX.getDouble(params);
        final double maxBeamSeedDyRatio = PARAM_MAX_BEAM_SEED_DY_RATIO.getDouble(params);
        final boolean allowSmall = BeamLinker.allowSmallHeadOnStandardBeam();

        final List<Glyph> keptSeeds = new ArrayList<>(
                system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED));
        PURGE_NO_STEM_SEEDS.invoke(retriever, keptSeeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, keptSeeds);
        final List<Glyph> keptSeedSnapshot = new ArrayList<>(keptSeeds);
        final IdentityHashMap<Glyph, Integer> keptOrdinals = identityOrdinals(keptSeeds);

        final List<Inter> sourceBeams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = interOrdinals(sourceBeams);
        final List<Inter> systemBeams = new ArrayList<>(sourceBeams);
        Collections.sort(systemBeams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, systemBeams);

        for (Iterator<Inter> iterator = systemBeams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (beam.getLinker() != null) {
                throw new IllegalStateException("HEADS beam already has linker");
            }
            final BeamLinker linker = new BeamLinker(beam, retriever);
            if (linker.looksLikeTremolo()) {
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }
        }

        final IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals = new IdentityHashMap<>();
        final IdentityHashMap<Glyph, Integer> beamGlyphAliases = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < systemBeams.size(); ordinal++) {
            final AbstractBeamInter beam = (AbstractBeamInter) systemBeams.get(ordinal);
            beamXOrdinals.put(beam, ordinal);
            alias(beamGlyphAliases, beam.getGlyph());
        }

        final List<Inter> preSortHeads = system.getSig().inters(
                ShapeSet.getTemplateNotesStem(sheet));
        final IdentityHashMap<Inter, Integer> headPreOrdinals = interOrdinals(preSortHeads);
        final List<Inter> systemHeads = new ArrayList<>(preSortHeads);
        Collections.sort(systemHeads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, systemHeads);
        final IdentityHashMap<HeadInter, Integer> headXOrdinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < systemHeads.size(); ordinal++) {
            headXOrdinals.put((HeadInter) systemHeads.get(ordinal), ordinal);
        }

        emit(String.format(
                "stemsbeamvinspectsystem %s system %d profile %d interline %d bounds %s "
                        + "beamSigOrder %s beamXOrder %s headPreSort %s headXOrder %s "
                        + "keptSeeds %d maxBeamLinkerDx %d maxBeamSideDx %d maxBeamGroupDy %d "
                        + "minBeamHeadDy %d vicinityMargin %d "
                        + "slopeMargin %s halfBeamLuDx %s maxBeamSeedDyRatio %s "
                        + "allowSmallHeadOnStandardBeam %s",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                rectangle(system.getBounds()),
                ordinalRange(sourceBeams.size()),
                interOrdinals(systemBeams, beamSigOrdinals),
                ordinalRange(preSortHeads.size()),
                interOrdinals(systemHeads, headPreOrdinals),
                keptSeeds.size(),
                maxBeamLinkerDx,
                maxBeamSideDx,
                maxBeamGroupDy,
                minBeamHeadDy,
                vicinityMargin,
                hexDouble(slopeMargin),
                hexDouble(halfBeamLuDx),
                hexDouble(maxBeamSeedDyRatio),
                allowSmall), hash, pageHash);

        for (int xOrdinal = 0; xOrdinal < systemBeams.size(); xOrdinal++) {
            final AbstractBeamInter beam = (AbstractBeamInter) systemBeams.get(xOrdinal);
            emit(String.format(
                    "stemsbeamvinspectbeamorder %s system %d xOrdinal %d sigOrdinal %d "
                            + "shape %s small %s bounds %s median %s glyph %s groupMembers %s",
                    page,
                    system.getId(),
                    xOrdinal,
                    beamSigOrdinals.get(beam),
                    beam.getShape(),
                    beam.isSmall(),
                    rectangle(beam.getBounds()),
                    line(beam.getMedian()),
                    glyphToken(beam.getGlyph(), beamGlyphAliases),
                    beamTokens(beam.getGroup().getMembers(), beamSigOrdinals)), hash, pageHash);
        }

        final IdentityHashMap<Object, String> cAliases = new IdentityHashMap<>();
        final List<Object> allCLinkers = new ArrayList<>();
        for (int xOrdinal = 0; xOrdinal < systemHeads.size(); xOrdinal++) {
            final HeadInter head = (HeadInter) systemHeads.get(xOrdinal);
            if (head.getLinker() != null) {
                throw new IllegalStateException("HEADS head already has linker");
            }
            head.setLinker(new HeadLinker(head, retriever));
            final int staffOrdinal = system.getStaves().indexOf(head.getStaff());
            final Shape shape = head.getShape();
            if (shape.isSmallHead()) totals.smallHeads++;
            emit(String.format(
                    "stemsbeamvinspecthead %s system %d xOrdinal %d preSortOrdinal %d shape %s "
                            + "bounds %s center %d:%d staffOrdinal %d grade %s small %s half %s",
                    page,
                    system.getId(),
                    xOrdinal,
                    headPreOrdinals.get(head),
                    shape,
                    rectangle(head.getBounds()),
                    head.getCenter().x,
                    head.getCenter().y,
                    staffOrdinal,
                    hexDouble(head.getGrade()),
                    shape.isSmallHead(),
                    ShapeSet.HalfHeads.contains(shape)), hash, pageHash);

            int constructorOrdinal = 0;
            for (HorizontalSide hSide : HorizontalSide.values()) {
                for (VerticalSide vSide : VerticalSide.values()) {
                    final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                    final String token = cToken(xOrdinal, hSide, vSide);
                    if (cAliases.put(c, token) != null || C_STEM_BUILDER.get(c) != null) {
                        throw new IllegalStateException("invalid initial CLinker identity/state");
                    }
                    allCLinkers.add(c);
                    totals.builderChecks++;
                    emit(String.format(
                            "stemsbeamvinspectcorner %s system %d head %d constructorOrdinal %d "
                                    + "hSide %s vSide %s alias %s ref %s builder null",
                            page,
                            system.getId(),
                            xOrdinal,
                            constructorOrdinal++,
                            hSide,
                            vSide,
                            token,
                            point(head.getLinker().getCornerLinker(hSide, vSide)
                                    .getReferencePoint())), hash, pageHash);
                }
            }
        }

        totals.beams = systemBeams.size();
        for (Inter inter : systemBeams) {
            if (((AbstractBeamInter) inter).isSmall()) totals.smallBeams++;
        }
        totals.heads = systemHeads.size();
        totals.corners = allCLinkers.size();

        final IdentityHashMap<Object, String> bAliases = new IdentityHashMap<>();
        final IdentityHashMap<Object, List<Glyph>> vSeedSnapshots = new IdentityHashMap<>();
        for (Inter inter : systemBeams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            totals.initialBs += allB.size();
            for (int ordinal = 0; ordinal < allB.size(); ordinal++) {
                final Object b = allB.get(ordinal);
                registerB(beam, b, ordinal, beamSigOrdinals, bAliases);
                emitBState(
                        "stemsbeamvinspectinitialb", page, system, beam, b, ordinal,
                        beamSigOrdinals, hash, pageHash);
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                totals.initialVs += vMap.size();
                for (Object v : vMap.values()) {
                    if (V_STEM_BUILDER.get(v) != null) {
                        throw new IllegalStateException("VLinker already has StemBuilder");
                    }
                    final Set<Glyph> seeds = (Set<Glyph>) V_SEEDS.get(v);
                    vSeedSnapshots.put(v, new ArrayList<>(seeds));
                    totals.builderChecks++;
                    totals.seedSnapshots++;
                }
            }
        }

        final int[] inspectOrdinal = { 0 };
        final int[] findOrdinal = { 0 };
        for (int beamInspection = 0; beamInspection < systemBeams.size(); beamInspection++) {
            final AbstractBeamInter beam = (AbstractBeamInter) systemBeams.get(beamInspection);
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            totals.beamStarts++;
            emit(String.format(
                    "stemsbeamvinspectbeamstart %s system %d inspection %d xOrdinal %d "
                            + "sigOrdinal %d allB %s",
                    page,
                    system.getId(),
                    beamInspection,
                    beamXOrdinals.get(beam),
                    beamSigOrdinals.get(beam),
                    bListTokens(beam, allB, beamSigOrdinals, bAliases)), hash, pageHash);

            for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                final Object b = allB.get(bOrdinal);
                final boolean anchor = B_IS_ANCHOR.getBoolean(b);
                final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
                final int maxProfile = hSide != null ? Profiles.BEAM_SIDE : Profiles.BEAM_SEED;
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                totals.bVisits++;
                if (anchor) totals.anchorSkips++;
                emit(String.format(
                        "stemsbeamvinspectbvisit %s system %d inspection %d beamSig %d "
                                + "ordinal %d alias %s isAnchor %s action %s hSide %s "
                                + "maxProfile %s vSides %s",
                        page,
                        system.getId(),
                        beamInspection,
                        beamSigOrdinals.get(beam),
                        bOrdinal,
                        bAliases.get(b),
                        anchor,
                        anchor ? "skipAnchor" : "visit",
                        anchor ? "-" : optionalSide(hSide),
                        anchor ? "-" : Integer.toString(maxProfile),
                        sideTokens(vMap.keySet())), hash, pageHash);
                if (anchor) continue;
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    inspectV(
                            page,
                            sheet,
                            system,
                            retriever,
                            beam,
                            b,
                            bOrdinal,
                            entry.getKey(),
                            entry.getValue(),
                            maxProfile,
                            maxBeamLinkerDx,
                            maxBeamSideDx,
                            minBeamHeadDy,
                            allowSmall,
                            vicinityMargin,
                            maxBeamGroupDy,
                            slopeMargin,
                            halfBeamLuDx,
                            maxBeamSeedDyRatio,
                            systemBeams,
                            systemHeads,
                            beamSigOrdinals,
                            beamXOrdinals,
                            beamGlyphAliases,
                            headXOrdinals,
                            cAliases,
                            bAliases,
                            keptOrdinals,
                            inspectOrdinal,
                            findOrdinal,
                            totals,
                            hash,
                            pageHash);
                }
            }
            emit(String.format(
                    "stemsbeamvinspectbeamend %s system %d inspection %d sigOrdinal %d allB %s",
                    page,
                    system.getId(),
                    beamInspection,
                    beamSigOrdinals.get(beam),
                    bListTokens(beam, allB, beamSigOrdinals, bAliases)), hash, pageHash);
        }

        for (Inter inter : systemBeams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            totals.finalBs += allB.size();
            for (int ordinal = 0; ordinal < allB.size(); ordinal++) {
                final Object b = allB.get(ordinal);
                registerB(beam, b, ordinal, beamSigOrdinals, bAliases);
                emitBState(
                        "stemsbeamvinspectfinalb", page, system, beam, b, ordinal,
                        beamSigOrdinals, hash, pageHash);
            }
        }

        if (!sameIdentityList(keptSeeds, keptSeedSnapshot)) {
            throw new IllegalStateException("system seed pool mutated during inspect replay");
        }
        for (Map.Entry<Object, List<Glyph>> entry : vSeedSnapshots.entrySet()) {
            if (V_STEM_BUILDER.get(entry.getKey()) != null
                    || !sameIdentityIteration(
                            (Set<Glyph>) V_SEEDS.get(entry.getKey()), entry.getValue())) {
                throw new IllegalStateException("VLinker state mutated past filter boundary");
            }
            totals.builderChecks++;
        }
        for (Object c : allCLinkers) {
            if (C_STEM_BUILDER.get(c) != null) {
                throw new IllegalStateException("CLinker unexpectedly has StemBuilder");
            }
            totals.builderChecks++;
        }
        if (totals.finalBs != totals.initialBs + totals.anchorsCreated) {
            throw new IllegalStateException("anchor accounting disagrees with final B topology");
        }

        emit(String.format(
                "stemsbeamvinspectsystemsummary %s system %d systems 1 beams %d smallBeams %d "
                        + "heads %d smallHeads %d corners %d initialBs %d initialVs %d "
                        + "beamStarts %d bVisits %d anchorSkips %d inspectedVs %d "
                        + "siblingScans %d siblingHits %d beamCandidates %d eligibleBeams %d "
                        + "findCalls %d findCandidates %d bReuses %d anchorReuses %d "
                        + "anchorsCreated %d headLookups %d headScans %d headAreaHits %d "
                        + "headCompetingPasses %d headCompetingRemovals %d "
                        + "headCompetitorDrops %d smallHeadCandidates %d sizeDrops %d "
                        + "headNearDrops %d cornerChecks %d cornerInside %d voidCornerDrops %d "
                        + "cAccepted %d resultBs %d resultCs %d finalBs %d seedSnapshots %d "
                        + "builderChecks %d hash %016x",
                page,
                system.getId(),
                totals.beams,
                totals.smallBeams,
                totals.heads,
                totals.smallHeads,
                totals.corners,
                totals.initialBs,
                totals.initialVs,
                totals.beamStarts,
                totals.bVisits,
                totals.anchorSkips,
                totals.inspectedVs,
                totals.siblingScans,
                totals.siblingHits,
                totals.beamCandidates,
                totals.eligibleBeams,
                totals.findCalls,
                totals.findCandidates,
                totals.bReuses,
                totals.anchorReuses,
                totals.anchorsCreated,
                totals.headLookups,
                totals.headScans,
                totals.headAreaHits,
                totals.headCompetingPasses,
                totals.headCompetingRemovals,
                totals.headCompetitorDrops,
                totals.smallHeadCandidates,
                totals.smallHeadDrops,
                totals.headNearDrops,
                totals.cornerChecks,
                totals.cornerInside,
                totals.voidCornerDrops,
                totals.cAccepted,
                totals.resultBs,
                totals.resultCs,
                totals.finalBs,
                totals.seedSnapshots,
                totals.builderChecks,
                hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void inspectV (String page,
                                  Sheet sheet,
                                  SystemInfo system,
                                  StemsRetriever retriever,
                                  AbstractBeamInter beam,
                                  Object b,
                                  int bOrdinal,
                                  VerticalSide mapSide,
                                  Object v,
                                  int maxProfile,
                                  int maxBeamLinkerDx,
                                  int maxBeamSideDx,
                                  int minBeamHeadDy,
                                  boolean allowSmall,
                                  int vicinityMargin,
                                  int maxBeamGroupDy,
                                  double slopeMargin,
                                  double halfBeamLuDx,
                                  double maxBeamSeedDyRatio,
                                  List<Inter> systemBeams,
                                  List<Inter> systemHeads,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                  IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals,
                                  IdentityHashMap<Glyph, Integer> beamGlyphAliases,
                                  IdentityHashMap<HeadInter, Integer> headXOrdinals,
                                  IdentityHashMap<Object, String> cAliases,
                                  IdentityHashMap<Object, String> bAliases,
                                  IdentityHashMap<Glyph, Integer> keptOrdinals,
                                  int[] inspectOrdinal,
                                  int[] findOrdinal,
                                  Totals totals,
                                  RowHasher... hashes)
        throws Exception
    {
        if (V_STEM_BUILDER.get(v) != null) {
            throw new IllegalStateException("VLinker already inspected");
        }
        final VerticalSide vSide = (VerticalSide) V_V_SIDE.get(v);
        final int yDir = V_Y_DIR.getInt(v);
        final Point2D refPt = (Point2D) B_REF_PT.get(b);
        final Line2D theoLine = (Line2D) V_THEO_LINE.get(v);
        final Area luArea = (Area) V_LU_AREA.get(v);
        final Set<Glyph> seeds = (Set<Glyph>) V_SEEDS.get(v);
        if (vSide != mapSide || yDir != vSide.direction()) {
            throw new IllegalStateException("VLinker side mismatch");
        }
        final LookupGeometry lookup = lookupGeometry(
                beam,
                sheet,
                system,
                systemBeams,
                refPt,
                (HorizontalSide) B_H_SIDE.get(b),
                vSide,
                yDir,
                vicinityMargin,
                maxBeamSideDx,
                maxBeamGroupDy,
                slopeMargin,
                halfBeamLuDx,
                maxBeamSeedDyRatio);
        if (!sameLineBits(theoLine, lookup.theoLine)
                || !luArea.equals(lookup.area)
                || !luArea.getBounds().equals(lookup.area.getBounds())
                || !sameRectangle2DBits(luArea.getBounds2D(), lookup.area.getBounds2D())) {
            throw new IllegalStateException("raw lookup-geometry replay differs from VLinker");
        }
        final int ordinal = inspectOrdinal[0]++;
        totals.inspectedVs++;
        emit(String.format(
                "stemsbeamvinspectv %s system %d inspectOrdinal %d beamSig %d beamX %d "
                        + "bOrdinal %d bAlias %s beamShape %s beamSmall %s hSide %s ref %s "
                        + "vSide %s yDir %d "
                        + "maxProfile %d theo %s luYLimit %s luQuad %s luBounds %s "
                        + "luBounds2d %s seeds %s",
                page,
                system.getId(),
                ordinal,
                beamSigOrdinals.get(beam),
                beamXOrdinals.get(beam),
                bOrdinal,
                bAliases.get(b),
                beam.getShape(),
                beam.isSmall(),
                optionalSide((HorizontalSide) B_H_SIDE.get(b)),
                point(refPt),
                vSide,
                yDir,
                maxProfile,
                line(theoLine),
                hexDouble(lookup.yLimit),
                lookup.quadToken,
                rectangle(luArea.getBounds()),
                rectangle2D(luArea.getBounds2D()),
                seedTokens(seeds, keptOrdinals)), hashes);

        final List<AbstractBeamInter> actualSiblings = beam.getLinker().getSiblingBeamsAt(refPt);
        final List<AbstractBeamInter> replaySiblings = replaySiblings(
                page,
                sheet,
                system,
                beam,
                bOrdinal,
                vSide,
                refPt,
                maxBeamSideDx,
                beamSigOrdinals,
                beamXOrdinals,
                beamGlyphAliases,
                totals,
                hashes);
        if (!sameIdentityList(actualSiblings, replaySiblings)) {
            throw new IllegalStateException("sibling replay differs from BeamLinker");
        }

        final List<FindDecision> decisions = replayFilterBeams(
                page,
                system,
                beam,
                bOrdinal,
                vSide,
                theoLine,
                actualSiblings,
                maxBeamLinkerDx,
                beamSigOrdinals,
                beamXOrdinals,
                beamGlyphAliases,
                bAliases,
                findOrdinal,
                totals,
                hashes);
        final List<Object> actualBs = (List<Object>) V_FILTER_BEAMS.invoke(v, actualSiblings);
        final List<Object> expectedBs = finishFindDecisions(
                page,
                system,
                beam,
                bOrdinal,
                vSide,
                decisions,
                actualBs,
                beamSigOrdinals,
                bAliases,
                totals,
                hashes);
        if (!sameIdentityList(actualBs, expectedBs)) {
            throw new IllegalStateException("filterBeams result differs from replay");
        }

        final HeadReplay headReplay = replayFilterHeads(
                page,
                system,
                beam,
                bOrdinal,
                vSide,
                yDir,
                refPt,
                luArea,
                actualSiblings,
                systemHeads,
                minBeamHeadDy,
                allowSmall,
                beamSigOrdinals,
                headXOrdinals,
                cAliases,
                totals,
                hashes);
        final List<Object> actualCs = (List<Object>) V_FILTER_HEADS.invoke(v, actualSiblings);
        if (!sameIdentityList(actualCs, headReplay.cLinkers)) {
            throw new IllegalStateException("filterHeads result differs from replay");
        }

        final String bTargets = objectTokens(actualBs, bAliases);
        final String cTargets = objectTokens(actualCs, cAliases);
        final StringBuilder targets = new StringBuilder();
        for (Object target : actualBs) append(targets, bAliases.get(target));
        for (Object target : actualCs) append(targets, cAliases.get(target));
        totals.resultBs += actualBs.size();
        totals.resultCs += actualCs.size();
        emit(String.format(
                "stemsbeamvinspectresult %s system %d inspectOrdinal %d beamSig %d bOrdinal %d "
                        + "vSide %s bTargets %s cTargets %s targets %s seeds %s maxProfile %d "
                        + "builder null",
                page,
                system.getId(),
                ordinal,
                beamSigOrdinals.get(beam),
                bOrdinal,
                vSide,
                bTargets,
                cTargets,
                emptyToken(targets),
                seedTokens(seeds, keptOrdinals),
                maxProfile), hashes);

        if (V_STEM_BUILDER.get(v) != null) {
            throw new IllegalStateException("filter replay created StemBuilder");
        }
        totals.builderChecks++;
    }

    private static List<AbstractBeamInter> replaySiblings (
            String page,
            Sheet sheet,
            SystemInfo system,
            AbstractBeamInter beam,
            int bOrdinal,
            VerticalSide vSide,
            Point2D refPt,
            int margin,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals,
            IdentityHashMap<Glyph, Integer> beamGlyphAliases,
            Totals totals,
            RowHasher... hashes)
    {
        final Line2D vertical = sheet.getSkew().skewedVertical(refPt);
        final List<Inter> members = beam.getGroup().getMembers();
        final List<AbstractBeamInter> accepted = new ArrayList<>();
        final IdentityHashMap<AbstractBeamInter, Point2D> crosses = new IdentityHashMap<>();
        for (Inter inter : members) {
            final AbstractBeamInter sibling = (AbstractBeamInter) inter;
            final Point2D cross = LineUtil.intersection(vertical, sibling.getMedian());
            crosses.put(sibling, cross);
            final boolean within = sibling.getMedian().getX1() - margin <= cross.getX()
                    && cross.getX() <= sibling.getMedian().getX2() + margin;
            if (within) accepted.add(sibling);
        }
        Collections.sort(
                accepted,
                (left, right) -> Double.compare(
                        crosses.get(left).getY(), crosses.get(right).getY()));
        for (int groupOrdinal = 0; groupOrdinal < members.size(); groupOrdinal++) {
            final AbstractBeamInter sibling = (AbstractBeamInter) members.get(groupOrdinal);
            final Point2D cross = crosses.get(sibling);
            final int sortedOrdinal = identityIndex(accepted, sibling);
            final boolean within = sortedOrdinal >= 0;
            totals.siblingScans++;
            if (within) totals.siblingHits++;
            emit(String.format(
                    "stemsbeamvinspectsibling %s system %d beamSig %d bOrdinal %d vSide %s "
                            + "groupOrdinal %d siblingSig %d siblingX %d glyph %s median %s "
                            + "vertical %s cross %s margin %d within %s sortedOrdinal %d",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    groupOrdinal,
                    beamSigOrdinals.get(sibling),
                    beamXOrdinals.get(sibling),
                    glyphToken(sibling.getGlyph(), beamGlyphAliases),
                    line(sibling.getMedian()),
                    line(vertical),
                    point(cross),
                    margin,
                    within,
                    sortedOrdinal), hashes);
        }
        return accepted;
    }

    private static List<FindDecision> replayFilterBeams (
            String page,
            SystemInfo system,
            AbstractBeamInter beam,
            int bOrdinal,
            VerticalSide vSide,
            Line2D theoLine,
            List<AbstractBeamInter> siblings,
            int maxBeamLinkerDx,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals,
            IdentityHashMap<Glyph, Integer> beamGlyphAliases,
            IdentityHashMap<Object, String> bAliases,
            int[] findOrdinal,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final List<FindDecision> decisions = new ArrayList<>();
        for (int siblingOrdinal = 0; siblingOrdinal < siblings.size(); siblingOrdinal++) {
            final AbstractBeamInter target = siblings.get(siblingOrdinal);
            totals.beamCandidates++;
            final String action;
            if (target == beam) {
                action = "self";
            } else if (target.getGlyph() == null) {
                action = "nullGlyph";
            } else if (target.getGlyph() == beam.getGlyph()) {
                action = "sameGlyph";
            } else {
                action = "find";
            }
            emit(String.format(
                    "stemsbeamvinspectbeamcandidate %s system %d sourceSig %d bOrdinal %d "
                            + "vSide %s siblingOrdinal %d targetSig %d targetX %d sourceGlyph %s "
                            + "targetGlyph %s action %s",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    siblingOrdinal,
                    beamSigOrdinals.get(target),
                    beamXOrdinals.get(target),
                    glyphToken(beam.getGlyph(), beamGlyphAliases),
                    glyphToken(target.getGlyph(), beamGlyphAliases),
                    action), hashes);
            if (!action.equals("find")) continue;

            totals.eligibleBeams++;
            totals.findCalls++;
            final int callOrdinal = findOrdinal[0]++;
            final Point2D cross = LineUtil.intersection(theoLine, target.getMedian());
            final List<Object> targetB = (List<Object>) LINKER_ALL_B.get(target.getLinker());
            final int beforeCount = targetB.size();
            Object best = null;
            double bestDx = Double.MAX_VALUE;
            emit(String.format(
                    "stemsbeamvinspectfind %s system %d callOrdinal %d sourceSig %d bOrdinal %d "
                            + "vSide %s targetSig %d targetX %d theo %s targetMedian %s cross %s "
                            + "x0 %s maxDx %d beforeCount %d beforeBs %s",
                    page,
                    system.getId(),
                    callOrdinal,
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    beamSigOrdinals.get(target),
                    beamXOrdinals.get(target),
                    line(theoLine),
                    line(target.getMedian()),
                    point(cross),
                    hexDouble(cross.getX()),
                    maxBeamLinkerDx,
                    beforeCount,
                    bListTokens(target, targetB, beamSigOrdinals, bAliases)), hashes);
            for (int candidateOrdinal = 0; candidateOrdinal < targetB.size(); candidateOrdinal++) {
                final Object candidate = targetB.get(candidateOrdinal);
                final Point2D candidateRef = (Point2D) B_REF_PT.get(candidate);
                final double dx = Math.abs(candidateRef.getX() - cross.getX());
                final double before = bestDx;
                final boolean replace = bestDx > dx;
                if (replace) {
                    bestDx = dx;
                    best = candidate;
                }
                totals.findCandidates++;
                emit(String.format(
                        "stemsbeamvinspectfindcandidate %s system %d callOrdinal %d ordinal %d "
                                + "alias %s ref %s dx %s bestBefore %s strictReplace %s "
                                + "bestAfter %s bestAliasAfter %s",
                        page,
                        system.getId(),
                        callOrdinal,
                        candidateOrdinal,
                        bAliases.get(candidate),
                        point(candidateRef),
                        hexDouble(dx),
                        hexDouble(before),
                        replace,
                        hexDouble(bestDx),
                        best != null ? bAliases.get(best) : "-"), hashes);
            }
            final boolean reuse = bestDx <= maxBeamLinkerDx;
            decisions.add(new FindDecision(
                    callOrdinal, target, beforeCount, cross, best, bestDx, maxBeamLinkerDx, reuse));
        }
        return decisions;
    }

    private static List<Object> finishFindDecisions (
            String page,
            SystemInfo system,
            AbstractBeamInter source,
            int sourceBOrdinal,
            VerticalSide vSide,
            List<FindDecision> decisions,
            List<Object> actual,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<Object, String> bAliases,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        if (actual.size() != decisions.size()) {
            throw new IllegalStateException("filterBeams cardinality differs from eligible siblings");
        }
        final List<Object> expected = new ArrayList<>();
        for (int ordinal = 0; ordinal < decisions.size(); ordinal++) {
            final FindDecision decision = decisions.get(ordinal);
            final List<Object> targetB =
                    (List<Object>) LINKER_ALL_B.get(decision.target.getLinker());
            final Object result;
            final String action;
            if (decision.reuse) {
                if (targetB.size() != decision.beforeCount) {
                    throw new IllegalStateException("findLinker reuse changed B topology");
                }
                result = decision.best;
                action = B_IS_ANCHOR.getBoolean(result) ? "reuseAnchor" : "reuseInitial";
                totals.bReuses++;
                if (B_IS_ANCHOR.getBoolean(result)) totals.anchorReuses++;
            } else {
                if (targetB.size() != decision.beforeCount + 1) {
                    throw new IllegalStateException("findLinker create did not append exactly one B");
                }
                result = targetB.get(decision.beforeCount);
                registerB(
                        decision.target,
                        result,
                        decision.beforeCount,
                        beamSigOrdinals,
                        bAliases);
                if (!B_IS_ANCHOR.getBoolean(result)
                        || !samePointBits((Point2D) B_REF_PT.get(result), decision.cross)
                        || !((Map<?, ?>) B_V_LINKERS.get(result)).isEmpty()) {
                    throw new IllegalStateException("appended B is not the predicted anchor");
                }
                action = "createAnchor";
                totals.anchorsCreated++;
            }
            if (actual.get(ordinal) != result) {
                throw new IllegalStateException("filterBeams result identity differs from replay");
            }
            expected.add(result);
            emit(String.format(
                    "stemsbeamvinspectfindresult %s system %d callOrdinal %d sourceSig %d "
                            + "sourceB %d vSide %s targetSig %d bestDx %s thresholdInclusive %d "
                            + "action %s result %s id0 %d ref %s isAnchor %s beforeCount %d "
                            + "afterCount %d",
                    page,
                    system.getId(),
                    decision.callOrdinal,
                    beamSigOrdinals.get(source),
                    sourceBOrdinal,
                    vSide,
                    beamSigOrdinals.get(decision.target),
                    hexDouble(decision.bestDx),
                    decision.maxBeamLinkerDx,
                    action,
                    bAliases.get(result),
                    B_ID.getInt(result) - 1,
                    point((Point2D) B_REF_PT.get(result)),
                    B_IS_ANCHOR.getBoolean(result),
                    decision.beforeCount,
                    targetB.size()), hashes);
        }
        return expected;
    }

    private static HeadReplay replayFilterHeads (
            String page,
            SystemInfo system,
            AbstractBeamInter beam,
            int bOrdinal,
            VerticalSide vSide,
            int yDir,
            Point2D refPt,
            Area luArea,
            List<AbstractBeamInter> siblings,
            List<Inter> systemHeads,
            int minBeamHeadDy,
            boolean allowSmall,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<HeadInter, Integer> headXOrdinals,
            IdentityHashMap<Object, String> cAliases,
            Totals totals,
            RowHasher... hashes)
    {
        totals.headLookups++;
        if (siblings.isEmpty()) {
            emit(String.format(
                    "stemsbeamvinspectheadlookup %s system %d beamSig %d bOrdinal %d "
                            + "vSide %s siblings - action empty luBounds %s",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    rectangle(luArea.getBounds())), hashes);
            return new HeadReplay(new ArrayList<>());
        }

        final AbstractBeamInter borderBeam = yDir > 0
                ? siblings.get(siblings.size() - 1) : siblings.get(0);
        final VerticalSide borderSide = yDir > 0 ? VerticalSide.BOTTOM : VerticalSide.TOP;
        final Line2D lastBorder = borderBeam.getBorder(borderSide);
        final double yLastBorder = LineUtil.yAtX(lastBorder, refPt.getX());
        emit(String.format(
                "stemsbeamvinspectheadlookup %s system %d beamSig %d bOrdinal %d vSide %s "
                        + "siblings %s lastBeamSig %d lastBorderSide %s lastBorder %s "
                        + "refX %s yLastBorder %s luBounds %s luBounds2d %s "
                        + "systemHeads %s",
                page,
                system.getId(),
                beamSigOrdinals.get(beam),
                bOrdinal,
                vSide,
                abstractBeamTokens(siblings, beamSigOrdinals),
                beamSigOrdinals.get(borderBeam),
                borderSide,
                line(lastBorder),
                hexDouble(refPt.getX()),
                hexDouble(yLastBorder),
                rectangle(luArea.getBounds()),
                rectangle2D(luArea.getBounds2D()),
                headTokens(systemHeads, headXOrdinals)), hashes);

        final List<Inter> replayAreaHits = new ArrayList<>();
        final Rectangle areaBounds = luArea.getBounds();
        final double xMax = areaBounds.getMaxX();
        int hitOrdinal = 0;
        for (int inputOrdinal = 0; inputOrdinal < systemHeads.size(); inputOrdinal++) {
            final HeadInter head = (HeadInter) systemHeads.get(inputOrdinal);
            final boolean removed = head.isRemoved();
            final boolean intersects = !removed && luArea.intersects(head.getBounds());
            final boolean breakAfter = !removed && !intersects && head.getBounds().x > xMax;
            final int candidateOrdinal = intersects ? hitOrdinal++ : -1;
            if (intersects) replayAreaHits.add(head);
            totals.headScans++;
            if (intersects) totals.headAreaHits++;
            emit(String.format(
                    "stemsbeamvinspectheadscan %s system %d beamSig %d bOrdinal %d vSide %s "
                            + "inputOrdinal %d headX %d bounds %s removed %s intersects %s "
                            + "areaMaxX %s candidateOrdinal %d breakAfter %s",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    inputOrdinal,
                    headXOrdinals.get(head),
                    rectangle(head.getBounds()),
                    removed,
                    intersects,
                    hexDouble(xMax),
                    candidateOrdinal,
                    breakAfter), hashes);
            if (breakAfter) break;
        }
        final List<Inter> actualAreaHits = Inters.intersectedInters(
                systemHeads, GeoOrder.BY_ABSCISSA, luArea);
        if (!sameIdentityList(actualAreaHits, replayAreaHits)) {
            throw new IllegalStateException("head area scan differs from Inters");
        }

        final List<Inter> survivors = new ArrayList<>(actualAreaHits);
        final IdentityHashMap<Inter, StringBuilder> competitors = new IdentityHashMap<>();
        for (int siblingOrdinal = 0; siblingOrdinal < siblings.size(); siblingOrdinal++) {
            final AbstractBeamInter sibling = siblings.get(siblingOrdinal);
            final Set<Inter> competing = beam.getSig().getCompetingInters(sibling);
            final List<Inter> before = new ArrayList<>(survivors);
            final List<Inter> removed = new ArrayList<>();
            for (Inter candidate : actualAreaHits) {
                if (competing.contains(candidate)) {
                    competitors.computeIfAbsent(candidate, ignored -> new StringBuilder());
                    append(competitors.get(candidate), Integer.toString(beamSigOrdinals.get(sibling)));
                }
            }
            for (Inter candidate : before) {
                if (competing.contains(candidate)) removed.add(candidate);
            }
            survivors.removeAll(competing);
            totals.headCompetingPasses++;
            totals.headCompetingRemovals += removed.size();
            emit(String.format(
                    "stemsbeamvinspectheadcompeting %s system %d beamSig %d bOrdinal %d "
                            + "vSide %s siblingOrdinal %d siblingSig %d before %s removals %s "
                            + "after %s",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    siblingOrdinal,
                    beamSigOrdinals.get(sibling),
                    headTokens(before, headXOrdinals),
                    headTokens(removed, headXOrdinals),
                    headTokens(survivors, headXOrdinals)), hashes);
        }

        final List<Object> expected = new ArrayList<>();
        final HorizontalSide imposedVoidSide = yDir < 0
                ? HorizontalSide.LEFT : HorizontalSide.RIGHT;
        for (int areaOrdinal = 0; areaOrdinal < actualAreaHits.size(); areaOrdinal++) {
            final HeadInter head = (HeadInter) actualAreaHits.get(areaOrdinal);
            final StringBuilder competing = competitors.get(head);
            if (competing != null) {
                totals.headCompetitorDrops++;
                emit(String.format(
                        "stemsbeamvinspectheadcandidate %s system %d beamSig %d bOrdinal %d "
                                + "vSide %s areaOrdinal %d survivorOrdinal - headX %d shape %s "
                                + "headSmall %s beamSmall %s allowSmall %s sizeMismatch - "
                                + "centerY - yLastBorder - dy - minBeamHeadDy - "
                                + "distanceAccepted - "
                                + "competingSiblings %s action competitor",
                        page,
                        system.getId(),
                        beamSigOrdinals.get(beam),
                        bOrdinal,
                        vSide,
                        areaOrdinal,
                        headXOrdinals.get(head),
                        head.getShape(),
                        head.getShape().isSmallHead(),
                        beam.isSmall(),
                        allowSmall,
                        emptyToken(competing)), hashes);
            }
        }

        for (int survivorOrdinal = 0; survivorOrdinal < survivors.size(); survivorOrdinal++) {
            final HeadInter head = (HeadInter) survivors.get(survivorOrdinal);
            final Shape shape = head.getShape();
            final boolean small = shape.isSmallHead();
            final boolean sizeMismatch = !allowSmall && small && !beam.isSmall();
            if (small) totals.smallHeadCandidates++;
            Double dy = null;
            final String action;
            if (sizeMismatch) {
                action = "size";
                totals.smallHeadDrops++;
            } else {
                dy = yDir * (head.getCenter().y - yLastBorder);
                if (dy < minBeamHeadDy) {
                    action = "near";
                    totals.headNearDrops++;
                } else {
                    action = "corners";
                }
            }
            emit(String.format(
                    "stemsbeamvinspectheadcandidate %s system %d beamSig %d bOrdinal %d "
                            + "vSide %s areaOrdinal %d survivorOrdinal %d headX %d shape %s "
                            + "headSmall %s beamSmall %s allowSmall %s sizeMismatch %s "
                            + "centerY %s yLastBorder %s dy %s minBeamHeadDy %s "
                            + "distanceAccepted %s action %s",
                    page,
                    system.getId(),
                    beamSigOrdinals.get(beam),
                    bOrdinal,
                    vSide,
                    identityIndex(actualAreaHits, head),
                    survivorOrdinal,
                    headXOrdinals.get(head),
                    shape,
                    small,
                    beam.isSmall(),
                    allowSmall,
                    sizeMismatch,
                    sizeMismatch ? "-" : Integer.toString(head.getCenter().y),
                    sizeMismatch ? "-" : hexDouble(yLastBorder),
                    dy != null ? hexDouble(dy) : "-",
                    sizeMismatch ? "-" : Integer.toString(minBeamHeadDy),
                    sizeMismatch ? "-" : Boolean.toString(dy >= minBeamHeadDy),
                    action), hashes);
            if (!action.equals("corners")) continue;

            int cornerOrdinal = 0;
            for (var sLinker : head.getLinker().getSLinkers().values()) {
                final HorizontalSide hSide = sLinker.getHorizontalSide();
                final Object c = sLinker.getCornerLinker(vSide.opposite());
                final Point2D cRef = sLinker.getCornerLinker(vSide.opposite())
                        .getReferencePoint();
                final boolean inside = luArea.contains(cRef);
                final boolean half = ShapeSet.HalfHeads.contains(shape);
                final Boolean voidSideOk = inside ? !half || hSide == imposedVoidSide : null;
                final String cornerAction = !inside ? "outside" : !voidSideOk ? "void" : "accept";
                totals.cornerChecks++;
                if (inside) totals.cornerInside++;
                if (inside && !voidSideOk) totals.voidCornerDrops++;
                if (cornerAction.equals("accept")) {
                    expected.add(c);
                    totals.cAccepted++;
                }
                emit(String.format(
                        "stemsbeamvinspectcornercheck %s system %d beamSig %d bOrdinal %d "
                                + "vSide %s headX %d cornerOrdinal %d hSide %s targetVSide %s "
                                + "cAlias %s ref %s contains %s half %s imposedVoidSide %s "
                                + "voidSideOk %s action %s",
                        page,
                        system.getId(),
                        beamSigOrdinals.get(beam),
                        bOrdinal,
                        vSide,
                        headXOrdinals.get(head),
                        cornerOrdinal++,
                        hSide,
                        vSide.opposite(),
                        cAliases.get(c),
                        point(cRef),
                        inside,
                        half,
                        imposedVoidSide,
                        voidSideOk != null ? voidSideOk.toString() : "-",
                        cornerAction), hashes);
            }
        }
        return new HeadReplay(expected);
    }

    private static void emitBState (String prefix,
                                    String page,
                                    SystemInfo system,
                                    AbstractBeamInter beam,
                                    Object b,
                                    int ordinal,
                                    IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                    RowHasher... hashes)
        throws IllegalAccessException
    {
        final Map<VerticalSide, Object> vMap =
                (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
        emit(String.format(
                "%s %s system %d beamSig %d ordinal %d id0 %d alias %s mode %s hSide %s "
                        + "ref %s isAnchor %s vSides %s",
                prefix,
                page,
                system.getId(),
                beamSigOrdinals.get(beam),
                ordinal,
                B_ID.getInt(b) - 1,
                bToken(beam, ordinal, beamSigOrdinals),
                B_STUMP.get(b) != null ? "stump" : B_IS_ANCHOR.getBoolean(b) ? "anchor" : "orphan",
                optionalSide((HorizontalSide) B_H_SIDE.get(b)),
                point((Point2D) B_REF_PT.get(b)),
                B_IS_ANCHOR.getBoolean(b),
                sideTokens(vMap.keySet())), hashes);
    }

    private static void registerB (AbstractBeamInter beam,
                                   Object b,
                                   int ordinal,
                                   IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                   IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        if (B_ID.getInt(b) - 1 != ordinal) {
            throw new IllegalStateException("B id does not match insertion ordinal");
        }
        final String token = bToken(beam, ordinal, beamSigOrdinals);
        final String old = aliases.putIfAbsent(b, token);
        if (old != null && !old.equals(token)) {
            throw new IllegalStateException("B identity changed alias");
        }
    }

    private static String bListTokens (AbstractBeamInter beam,
                                       List<Object> allB,
                                       IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                       IdentityHashMap<Object, String> aliases)
        throws IllegalAccessException
    {
        final StringBuilder builder = new StringBuilder();
        for (int ordinal = 0; ordinal < allB.size(); ordinal++) {
            final Object b = allB.get(ordinal);
            registerB(beam, b, ordinal, beamSigOrdinals, aliases);
            append(builder, aliases.get(b) + (B_IS_ANCHOR.getBoolean(b) ? ":A" : ":I"));
        }
        return emptyToken(builder);
    }

    private static String bToken (AbstractBeamInter beam,
                                  int ordinal,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals)
    {
        return "beam:" + beamSigOrdinals.get(beam) + ":b:" + ordinal;
    }

    private static String cToken (int headOrdinal,
                                  HorizontalSide hSide,
                                  VerticalSide vSide)
    {
        return "h:" + headOrdinal + ":" + hSide + ":" + vSide;
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
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS BeamVLinker inspect oracle.");
        System.out.println("#");
        System.out.println("# Each page runs in a fresh Epsilon-GC JVM and reaches real HEADS.");
        System.out.println("# Beam and head linkers are constructed in the production stable-x order.");
        System.out.println("# Each V executes real filterBeams fully before real filterHeads; the probe");
        System.out.println("# stops immediately before new StemBuilder and proves all seeds/builders unchanged.");
        System.out.println("# Cross-beam B append/reuse, full sibling/head scans, and B-before-C target");
        System.out.println("# ordering use dense identity-free aliases and raw double bits.");
        System.out.println("# Lookup rows retain the independent buildLuArea y-limit and raw quadrilateral.");
    }

    private static Field declaredField (Class<?> owner,
                                        String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static IdentityHashMap<Glyph, Integer> identityOrdinals (List<Glyph> values)
    {
        final IdentityHashMap<Glyph, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (ordinals.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate glyph identity");
            }
        }
        return ordinals;
    }

    private static IdentityHashMap<Inter, Integer> interOrdinals (List<? extends Inter> values)
    {
        final IdentityHashMap<Inter, Integer> ordinals = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (ordinals.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate inter identity");
            }
        }
        return ordinals;
    }

    private static <T> int alias (IdentityHashMap<T, Integer> aliases,
                                  T value)
    {
        if (value == null) return -1;
        final Integer old = aliases.get(value);
        if (old != null) return old;
        final int next = aliases.size();
        aliases.put(value, next);
        return next;
    }

    private static String glyphToken (Glyph glyph,
                                      IdentityHashMap<Glyph, Integer> aliases)
    {
        return glyph != null ? "beamglyph:" + alias(aliases, glyph) : "-";
    }

    private static String seedTokens (Set<Glyph> seeds,
                                      IdentityHashMap<Glyph, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Glyph glyph : seeds) {
            final Integer ordinal = ordinals.get(glyph);
            if (ordinal == null) throw new IllegalStateException("V seed outside kept seed pool");
            append(builder, Integer.toString(ordinal));
        }
        return emptyToken(builder);
    }

    private static String objectTokens (List<Object> values,
                                        IdentityHashMap<Object, String> aliases)
    {
        final StringBuilder builder = new StringBuilder();
        for (Object value : values) {
            final String token = aliases.get(value);
            if (token == null) throw new IllegalStateException("missing object alias");
            append(builder, token);
        }
        return emptyToken(builder);
    }

    private static String interOrdinals (List<? extends Inter> values,
                                         IdentityHashMap<Inter, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Inter value : values) append(builder, Integer.toString(ordinals.get(value)));
        return emptyToken(builder);
    }

    private static String beamTokens (List<? extends Inter> values,
                                      IdentityHashMap<Inter, Integer> ordinals)
    {
        return interOrdinals(values, ordinals);
    }

    private static String abstractBeamTokens (List<AbstractBeamInter> values,
                                              IdentityHashMap<Inter, Integer> ordinals)
    {
        return interOrdinals(values, ordinals);
    }

    private static String headTokens (List<Inter> values,
                                      IdentityHashMap<HeadInter, Integer> ordinals)
    {
        final StringBuilder builder = new StringBuilder();
        for (Inter value : values) {
            append(builder, Integer.toString(ordinals.get((HeadInter) value)));
        }
        return emptyToken(builder);
    }

    private static String ordinalRange (int size)
    {
        final StringBuilder builder = new StringBuilder();
        for (int ordinal = 0; ordinal < size; ordinal++) append(builder, Integer.toString(ordinal));
        return emptyToken(builder);
    }

    private static String sideTokens (Set<VerticalSide> sides)
    {
        final StringBuilder builder = new StringBuilder();
        for (VerticalSide side : sides) append(builder, side.toString());
        return emptyToken(builder);
    }

    private static String optionalSide (HorizontalSide side)
    {
        return side != null ? side.toString() : "-";
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
        return value != null ? point(value.getP1()) + ":" + point(value.getP2()) : "-";
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

    private static LookupGeometry lookupGeometry (AbstractBeamInter beam,
                                                  Sheet sheet,
                                                  SystemInfo system,
                                                  List<Inter> systemBeams,
                                                  Point2D refPt,
                                                  HorizontalSide hSide,
                                                  VerticalSide vSide,
                                                  int yDir,
                                                  int vicinityMargin,
                                                  int maxBeamSideDx,
                                                  int maxBeamGroupDy,
                                                  double slopeMargin,
                                                  double halfBeamLuDx,
                                                  double maxBeamSeedDyRatio)
    {
        final Rectangle systemBox = system.getBounds();
        double yLimit = yDir < 0 ? systemBox.getMaxY() : systemBox.getMinY();
        final List<Staff> around = system.getStavesAround(beam.getCenter());
        for (Staff staff : around) {
            final Rectangle partBox = staff.getPart().getAreaBounds();
            final double candidate = yDir > 0
                    ? partBox.y + partBox.height - 1 : partBox.y;
            yLimit = yDir > 0 ? Math.max(yLimit, candidate) : Math.min(yLimit, candidate);
        }

        final double skewSlope = sheet.getSkew().getSlope();
        final Point2D initialTarget = StemsRetriever.getTargetPt(
                refPt, new Line2D.Double(0, yLimit, 100, yLimit), skewSlope);
        final Line2D initialTheo = new Line2D.Double(refPt, initialTarget);
        final Rectangle beamBox = beam.getBounds();
        final Rectangle fatBox = new Rectangle(
                beamBox.x, systemBox.y, beamBox.width, systemBox.height);
        fatBox.grow(vicinityMargin, 0);
        final List<Inter> aliens = Inters.intersectedInters(
                systemBeams, GeoOrder.BY_ABSCISSA, fatBox);
        aliens.removeAll(beam.getGroup().getMembers());
        for (Iterator<Inter> iterator = aliens.iterator(); iterator.hasNext();) {
            final AbstractBeamInter alien = (AbstractBeamInter) iterator.next();
            final Line2D median = alien.getMedian();
            if (!alien.isGood()
                    || alien instanceof BeamHookInter
                    || !median.intersectsLine(initialTheo)) {
                iterator.remove();
            } else {
                final Point2D cross = LineUtil.intersection(initialTheo, median);
                final double dy = Math.abs(cross.getY() - refPt.getY());
                if (dy <= maxBeamGroupDy) {
                    final double endpointX = hSide == HorizontalSide.LEFT
                            ? median.getX1() : median.getX2();
                    final double dx = Math.abs(cross.getX() - endpointX);
                    if (dx < maxBeamSideDx) iterator.remove();
                }
            }
        }
        if (!aliens.isEmpty()) {
            StemsRetriever.sortBeamsFromRef(refPt, yDir, aliens);
            final AbstractBeamInter firstAlien = (AbstractBeamInter) aliens.get(0);
            final Line2D limit = firstAlien.getBorder(vSide.opposite());
            yLimit = LineUtil.yAtX(limit, refPt.getX());
        }

        final double luSlope = -skewSlope;
        final double dSlope = yDir * slopeMargin;
        final double xRef = refPt.getX();
        final Line2D border = beam.getBorder(vSide);
        final Point2D pl = LineUtil.intersectionAtX(border, xRef - halfBeamLuDx);
        final Point2D pr = LineUtil.intersectionAtX(border, xRef + halfBeamLuDx);
        final int profile = Math.max(beam.getProfile(), system.getProfile());
        final int yGapPixels = sheet.getScale().toPixels(
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
        final Line2D finalTheo = new Line2D.Double(refPt, target);
        return new LookupGeometry(
                yLimit,
                point(q0) + ":" + point(q1) + ":" + point(q2) + ":" + point(q3),
                area,
                finalTheo);
    }

    private static void append (StringBuilder builder,
                                String token)
    {
        if (builder.length() != 0) builder.append(',');
        builder.append(token);
    }

    private static String emptyToken (StringBuilder builder)
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

    private static boolean sameIdentityIteration (Set<Glyph> left,
                                                  List<Glyph> right)
    {
        if (left.size() != right.size()) return false;
        final Iterator<Glyph> iterator = left.iterator();
        for (Glyph glyph : right) {
            if (!iterator.hasNext() || iterator.next() != glyph) return false;
        }
        return !iterator.hasNext();
    }

    private static boolean samePointBits (Point2D left,
                                          Point2D right)
    {
        return Double.doubleToLongBits(left.getX()) == Double.doubleToLongBits(right.getX())
                && Double.doubleToLongBits(left.getY()) == Double.doubleToLongBits(right.getY());
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

    private static void emit (String row,
                              RowHasher... hashes)
    {
        System.out.println(row);
        for (RowHasher hash : hashes) hash.add(row);
    }

    private record FindDecision(
            int callOrdinal,
            AbstractBeamInter target,
            int beforeCount,
            Point2D cross,
            Object best,
            double bestDx,
            int maxBeamLinkerDx,
            boolean reuse)
    {
    }

    private record LookupGeometry(
            double yLimit,
            String quadToken,
            Area area,
            Line2D theoLine)
    {
    }

    private record HeadReplay(List<Object> cLinkers)
    {
    }

    private static final class Totals
    {
        long beams;
        long smallBeams;
        long heads;
        long smallHeads;
        long corners;
        long initialBs;
        long initialVs;
        long beamStarts;
        long bVisits;
        long anchorSkips;
        long inspectedVs;
        long siblingScans;
        long siblingHits;
        long beamCandidates;
        long eligibleBeams;
        long findCalls;
        long findCandidates;
        long bReuses;
        long anchorReuses;
        long anchorsCreated;
        long headLookups;
        long headScans;
        long headAreaHits;
        long headCompetingPasses;
        long headCompetingRemovals;
        long headCompetitorDrops;
        long smallHeadCandidates;
        long smallHeadDrops;
        long headNearDrops;
        long cornerChecks;
        long cornerInside;
        long voidCornerDrops;
        long cAccepted;
        long resultBs;
        long resultCs;
        long finalBs;
        long seedSnapshots;
        long builderChecks;

        void include (Totals that)
        {
            beams += that.beams;
            smallBeams += that.smallBeams;
            heads += that.heads;
            smallHeads += that.smallHeads;
            corners += that.corners;
            initialBs += that.initialBs;
            initialVs += that.initialVs;
            beamStarts += that.beamStarts;
            bVisits += that.bVisits;
            anchorSkips += that.anchorSkips;
            inspectedVs += that.inspectedVs;
            siblingScans += that.siblingScans;
            siblingHits += that.siblingHits;
            beamCandidates += that.beamCandidates;
            eligibleBeams += that.eligibleBeams;
            findCalls += that.findCalls;
            findCandidates += that.findCandidates;
            bReuses += that.bReuses;
            anchorReuses += that.anchorReuses;
            anchorsCreated += that.anchorsCreated;
            headLookups += that.headLookups;
            headScans += that.headScans;
            headAreaHits += that.headAreaHits;
            headCompetingPasses += that.headCompetingPasses;
            headCompetingRemovals += that.headCompetingRemovals;
            headCompetitorDrops += that.headCompetitorDrops;
            smallHeadCandidates += that.smallHeadCandidates;
            smallHeadDrops += that.smallHeadDrops;
            headNearDrops += that.headNearDrops;
            cornerChecks += that.cornerChecks;
            cornerInside += that.cornerInside;
            voidCornerDrops += that.voidCornerDrops;
            cAccepted += that.cAccepted;
            resultBs += that.resultBs;
            resultCs += that.resultCs;
            finalBs += that.finalBs;
            seedSnapshots += that.seedSnapshots;
            builderChecks += that.builderChecks;
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
