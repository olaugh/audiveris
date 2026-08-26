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
import java.io.BufferedReader;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.dynamic.FilamentIndex;
import org.audiveris.omr.glyph.dynamic.LinkedSection;
import org.audiveris.omr.glyph.dynamic.StraightFilament;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.math.GeoUtil;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.math.PointsCollector;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Profiles;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.StaffLine;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.LineInfo;
import org.audiveris.omr.sheet.stem.BeamLinker;
import org.audiveris.omr.sheet.stem.HeadCorner;
import org.audiveris.omr.sheet.stem.HeadLinker;
import org.audiveris.omr.sheet.stem.StemBuilder;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemHalfLinker;
import org.audiveris.omr.sheet.stem.StemItem;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemScaler;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sheet.stem.VerticalsBuilder;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact identity-free oracle for head-origin {@link StemBuilder} construction.
 *
 * <p>It executes the complete production beam-builder registry prefix, then every real private
 * CLinker {@code inspect(profile)} exactly once in stable head-x /
 * {@link HeadCorner#values()} order. A tracking GlyphIndex records each real registration attempt
 * while a bounded structural registry independently replays its action. The oracle snapshots the
 * exact constructor inputs, occurrences, sorts, items, lengths, and current VIP-only
 * {@code filterHeadParts} behavior.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsHeadStemBuilderProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field SHEET_GLYPH_INDEX;

    private static final Field RETRIEVER_PARAMS;
    private static final Field RETRIEVER_SYSTEM_SEEDS;
    private static final Field RETRIEVER_SYSTEM_BEAMS;
    private static final Field RETRIEVER_SYSTEM_HEADS;
    private static final Field RETRIEVER_STEM_CHECKER;
    private static final Field RETRIEVER_SYSTEM_STEMS;
    private static final Method PURGE_NO_STEM_SEEDS;
    private static final Method VERTICALS_RETRIEVE_CANDIDATES;

    private static final Field PARAM_MAX_STEM_THICKNESS;
    private static final Field PARAM_MAX_BEAM_LINKER_DX;
    private static final Field PARAM_MIN_SEED_CONTRIB;
    private static final Field PARAM_MAX_LINE_SEED_DX;
    private static final Field PARAM_MAX_LINE_SECTION_DX;
    private static final Field PARAM_MAX_STEM_ALIGNMENT_DX;
    private static final Field PARAM_MAX_STEM_ALIGNMENT_DY;

    private static final Field BEAM_ALL_B;
    private static final Field B_ID;
    private static final Field B_IS_ANCHOR;
    private static final Field B_H_SIDE;
    private static final Field B_REF_PT;
    private static final Field B_STUMP;
    private static final Field B_V_LINKERS;
    private static final Field B_LINKED;
    private static final Field B_CLOSED;
    private static final Field V_STEM_BUILDER;
    private static final Method V_INSPECT;
    private static final Method V_IS_LINKED;
    private static final Method V_IS_CLOSED;

    private static final Field C_V_SIDE;
    private static final Field C_Y_DIR;
    private static final Field C_THEO_LINE;
    private static final Field C_LU_AREA;
    private static final Field C_TARGET_BEAM;
    private static final Field C_Y_RANGE;
    private static final Field C_SEEDS;
    private static final Field C_STEM_BUILDER;
    private static final Method C_INSPECT;
    private static final Method C_RETRIEVE_SEEDS;
    private static final Method C_LOOKUP_OTHER_HEADS;
    private static final Method C_IS_LINKED;
    private static final Method C_IS_CLOSED;
    private static final Field HEAD_NEIGHBOR_SEEDS;

    private static final Field STEM_BUILDER_ITEMS;
    private static final Field STEM_BUILDER_LENGTH_MAP;
    private static final Field STEM_BUILDER_LAST_HEAD_Y;
    private static final Field STEM_BUILDER_Y_RANGE;
    private static final Field STEM_BUILDER_Y_DIR;
    private static final Method STEM_FILTER_HEAD_PARTS;
    private static final Field STEM_ITEM_LINE;
    private static final Field STEM_ITEM_GLYPH;
    private static final Field STEM_ITEM_CONTRIB;
    private static final Field LINKER_ITEM_LINKER;
    private static final Field LINKED_SECTION_SOURCE;
    private static final Method RETRIEVER_GET_GAP_MAP;

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
            final Class<?> linkerItem = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemItem$LinkerItem");

            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);
            SHEET_GLYPH_INDEX = field(Sheet.class, "glyphIndex");
            RETRIEVER_PARAMS = field(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = field(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = field(StemsRetriever.class, "systemBeams");
            RETRIEVER_SYSTEM_HEADS = field(StemsRetriever.class, "systemHeads");
            RETRIEVER_STEM_CHECKER = field(StemsRetriever.class, "stemChecker");
            RETRIEVER_SYSTEM_STEMS = field(StemsRetriever.class, "systemStems");
            PURGE_NO_STEM_SEEDS = StemsRetriever.class.getDeclaredMethod(
                    "purgeNoStemSeeds", List.class);
            PURGE_NO_STEM_SEEDS.setAccessible(true);
            VERTICALS_RETRIEVE_CANDIDATES = VerticalsBuilder.class.getDeclaredMethod(
                    "retrieveCandidates");
            VERTICALS_RETRIEVE_CANDIDATES.setAccessible(true);

            PARAM_MAX_STEM_THICKNESS = field(parameters, "maxStemThickness");
            PARAM_MAX_BEAM_LINKER_DX = field(parameters, "maxBeamLinkerDx");
            PARAM_MIN_SEED_CONTRIB = field(parameters, "minSeedContrib");
            PARAM_MAX_LINE_SEED_DX = field(parameters, "maxLineSeedDx");
            PARAM_MAX_LINE_SECTION_DX = field(parameters, "maxLineSectionDx");
            PARAM_MAX_STEM_ALIGNMENT_DX = field(parameters, "maxStemAlignmentDx");
            PARAM_MAX_STEM_ALIGNMENT_DY = field(parameters, "maxStemAlignmentDy");

            BEAM_ALL_B = field(BeamLinker.class, "allBLinkers");
            B_ID = field(bLinker, "id");
            B_IS_ANCHOR = field(bLinker, "isAnchor");
            B_H_SIDE = field(bLinker, "hSide");
            B_REF_PT = field(bLinker, "refPt");
            B_STUMP = field(bLinker, "stump");
            B_V_LINKERS = field(bLinker, "vLinkers");
            B_LINKED = field(bLinker, "linked");
            B_CLOSED = field(bLinker, "closed");
            V_STEM_BUILDER = field(vLinker, "sb");
            V_INSPECT = vLinker.getDeclaredMethod("inspect", int.class);
            V_INSPECT.setAccessible(true);
            V_IS_LINKED = vLinker.getMethod("isLinked");
            V_IS_LINKED.setAccessible(true);
            V_IS_CLOSED = vLinker.getMethod("isClosed");
            V_IS_CLOSED.setAccessible(true);

            C_V_SIDE = field(cLinker, "vSide");
            C_Y_DIR = field(cLinker, "yDir");
            C_THEO_LINE = field(cLinker, "theoLine");
            C_LU_AREA = field(cLinker, "luArea");
            C_TARGET_BEAM = field(cLinker, "targetBeam");
            C_Y_RANGE = field(cLinker, "yRange");
            C_SEEDS = field(cLinker, "seeds");
            C_STEM_BUILDER = field(cLinker, "sb");
            C_INSPECT = cLinker.getDeclaredMethod("inspect", int.class);
            C_INSPECT.setAccessible(true);
            C_RETRIEVE_SEEDS = cLinker.getDeclaredMethod("retrieveSeeds");
            C_RETRIEVE_SEEDS.setAccessible(true);
            C_LOOKUP_OTHER_HEADS = cLinker.getDeclaredMethod("lookupOtherHeads");
            C_LOOKUP_OTHER_HEADS.setAccessible(true);
            C_IS_LINKED = cLinker.getMethod("isLinked");
            C_IS_LINKED.setAccessible(true);
            C_IS_CLOSED = cLinker.getMethod("isClosed");
            C_IS_CLOSED.setAccessible(true);
            HEAD_NEIGHBOR_SEEDS = field(HeadLinker.class, "neighborSeeds");

            STEM_BUILDER_ITEMS = field(StemBuilder.class, "items");
            STEM_BUILDER_LENGTH_MAP = field(StemBuilder.class, "lengthMap");
            STEM_BUILDER_LAST_HEAD_Y = field(StemBuilder.class, "lastHeadY");
            STEM_BUILDER_Y_RANGE = field(StemBuilder.class, "yRange");
            STEM_BUILDER_Y_DIR = field(StemBuilder.class, "yDir");
            STEM_FILTER_HEAD_PARTS = StemBuilder.class.getDeclaredMethod(
                    "filterHeadParts", Collection.class);
            STEM_FILTER_HEAD_PARTS.setAccessible(true);
            STEM_ITEM_LINE = field(StemItem.class, "line");
            STEM_ITEM_GLYPH = field(StemItem.class, "glyph");
            STEM_ITEM_CONTRIB = field(StemItem.class, "contrib");
            LINKER_ITEM_LINKER = field(linkerItem, "linker");
            LINKED_SECTION_SOURCE = field(LinkedSection.class, "section");
            RETRIEVER_GET_GAP_MAP = StemsRetriever.class.getDeclaredMethod("getGapMap");
            RETRIEVER_GET_GAP_MAP.setAccessible(true);
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsHeadStemBuilderProbe ()
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
            throw new IllegalArgumentException("expected one <path>:<sheet> target");
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
    }

    private static void runPage (Path path,
                                 int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        // STEM_SEEDS registers every staff/header-gated raw candidate, including candidates that
        // fail StemChecker and therefore have no group or graph owner at HEADS.  Reconstruct this
        // concrete upstream product on a separate sheet so the live page's sections, filament
        // registry, and exact STEMS chronology remain untouched.
        final SeedBaseline seedBaseline = loadSeedBaseline(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final Totals totals = new Totals();
        final RowHasher hash = new RowHasher();
        final OriginRegistry pageOrigins = buildPageOrigins(
                sheet, seedBaseline.checkedGlyphs, page);
        final OriginRegistry beamOnlyOrigins = buildPageOrigins(
                sheet, seedBaseline.checkedGlyphs, page);
        final StumpPlans stumpPlans = StumpPlans.load(page);
        final TrackingGlyphIndex glyphIndex = installTrackingGlyphIndex(
                sheet, pageOrigins.baselineGlyphs());
        System.out.printf(
                "stemsheadbuilderpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        emit(String.format(
                "stemsheadbuilderoriginbaseline %s rawSeedCandidates %d "
                        + "checkedSeedCandidates %d checkedSeedStructuralKeys %d "
                        + "modeledRegistry %d registrySha256 %s",
                page,
                seedBaseline.rawCandidates,
                seedBaseline.checkedGlyphs.size(),
                seedBaseline.checkedStructuralKeys,
                pageOrigins.count(),
                pageOrigins.historySha()), hash);
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(
                    page, sheet, system, pageOrigins, beamOnlyOrigins, stumpPlans, glyphIndex,
                    totals, hash);
        }
        stumpPlans.assertConsumed();
        System.out.printf(
                "stemsheadbuilderpagesummary %s systems %d %s hash %016x%n",
                page, sheet.getSystems().size(), totals.fields(), hash.value());
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
                                   OriginRegistry pageOrigins,
                                   OriginRegistry beamOnlyOrigins,
                                   StumpPlans stumpPlans,
                                   TrackingGlyphIndex glyphIndex,
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
        final List<Glyph> seedSnapshot = new ArrayList<>(seeds);
        emitSeedDuplicateCensus(page, system, seeds, totals, hash, pageHash);
        for (Glyph seed : seeds) {
            pageOrigins.label(seed, "stemSeed");
            beamOnlyOrigins.label(seed, "stemSeed");
        }

        final List<Inter> sourceBeams = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = identityOrdinals(sourceBeams);
        final List<Inter> beams = new ArrayList<>(sourceBeams);
        Collections.sort(beams, Inters.byAbscissa);
        final IdentityHashMap<Inter, Integer> beamXOrdinals = identityOrdinals(beams);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, beams);
        final List<StumpEvent> stumpEvents = new ArrayList<>();
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
            final AbstractBeamInter beam = (AbstractBeamInter) iterator.next();
            if (beam.getLinker() != null) {
                throw new IllegalStateException("HEADS beam already has linker");
            }
            final int registrationMark = glyphIndex.mark();
            final BeamLinker linker = new BeamLinker(beam, retriever);
            stumpEvents.addAll(stumpPlans.captureBeam(
                    system.getId(), beamXOrdinals.get(beam), beamSigOrdinals.get(beam),
                    glyphIndex.since(registrationMark)));
            if (linker.looksLikeTremolo()) {
                iterator.remove();
                beam.remove();
            } else {
                beam.setLinker(linker);
            }
        }

        final List<Inter> sourceHeads = system.getSig().inters(
                ShapeSet.getTemplateNotesStem(sheet));
        final IdentityHashMap<Inter, Integer> headSigOrdinals = identityOrdinals(sourceHeads);
        final List<Inter> heads = new ArrayList<>(sourceHeads);
        Collections.sort(heads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, heads);

        final IdentityHashMap<Object, String> cAliases = new IdentityHashMap<>();
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            if (head.getLinker() != null) {
                throw new IllegalStateException("HEADS head already has linker");
            }
            final int registrationMark = glyphIndex.mark();
            head.setLinker(new HeadLinker(head, retriever));
            stumpEvents.addAll(stumpPlans.captureHead(
                    system.getId(), headOrdinal, headSigOrdinals.get(head),
                    glyphIndex.since(registrationMark)));
            for (HorizontalSide hSide : HorizontalSide.values()) {
                for (VerticalSide vSide : VerticalSide.values()) {
                    final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                    cAliases.put(c, cAlias(headOrdinal, hSide, vSide));
                    if (C_STEM_BUILDER.get(c) != null || C_SEEDS.get(c) != null) {
                        throw new IllegalStateException("CLinker crossed constructor seam early");
                    }
                }
            }
        }

        final IdentityHashMap<Object, String> bAliases = new IdentityHashMap<>();
        registerAllBs(beams, beamSigOrdinals, bAliases);
        final OriginRegistry origins = pageOrigins;
        final int modeledRegistryAtSystemStart = origins.count();
        final String registryShaAtSystemStart = origins.historySha();
        emitStumpRegistrations(
                page, system, stumpEvents, origins, beamOnlyOrigins, totals, hash, pageHash);
        emit(String.format(
                "stemsheadbuilderregistryboundary %s system %d phase afterStumps "
                        + "modeledBefore %d modeledAfter %d shaBefore %s shaAfter %s",
                page, system.getId(), modeledRegistryAtSystemStart, origins.count(),
                registryShaAtSystemStart, origins.historySha()), hash, pageHash);

        emit(String.format(
                "stemsheadbuildersystem %s system %d profile %d inspectProfile %d "
                        + "interline %d bounds %s beamSigOrder %s beamXOrder %s "
                        + "headSigOrder %s headXOrder %s seeds %d modeledRegistry %d "
                        + "maxStemThickness %d maxLineSectionDx %s maxStemAlignmentDx %s "
                        + "maxStemAlignmentDy %s minCoreSectionLength 0 "
                        + "minSideRatio %s gap0 %d gap1 %d gap2 %d gap3 %d gap4 %d",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getStub().getProfile(),
                sheet.getScale().getInterline(),
                rectangle(system.getBounds()),
                ordinalRange(sourceBeams.size()),
                interTokens(beams, beamSigOrdinals),
                ordinalRange(sourceHeads.size()),
                interTokens(heads, headSigOrdinals),
                seeds.size(),
                origins.count(),
                PARAM_MAX_STEM_THICKNESS.getInt(params),
                hexDouble(PARAM_MAX_LINE_SECTION_DX.getDouble(params)),
                hexDouble(PARAM_MAX_STEM_ALIGNMENT_DX.getDouble(params)),
                hexDouble(PARAM_MAX_STEM_ALIGNMENT_DY.getDouble(params)),
                hexDouble(VerticalsBuilder.getMinSideRatio().getValue()),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(0)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(1)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(2)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(3)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(4))), hash, pageHash);
        if (system.getProfile() != sheet.getStub().getProfile()) {
            totals.profileDivergences++;
        }

        // Real beam-origin builders establish the exact page filament/glyph registration history.
        int beamBuilderOrdinal = 0;
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
            for (Object b : allB) {
                if (B_IS_ANCHOR.getBoolean(b)) continue;
                final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
                final int maxProfile = hSide != null ? Profiles.BEAM_SIDE : Profiles.BEAM_SEED;
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    final Object v = entry.getValue();
                    if (V_STEM_BUILDER.get(v) != null) {
                        throw new IllegalStateException("beam V already inspected");
                    }
                    final List<StraightFilament> filamentsBefore = filaments(sheet);
                    final int beforeBs = registerAllBs(beams, beamSigOrdinals, bAliases);
                    final int registrationMark = glyphIndex.mark();
                    V_INSPECT.invoke(v, maxProfile);
                    if (V_STEM_BUILDER.get(v) == null) {
                        throw new IllegalStateException("beam V did not assign builder");
                    }
                    final int afterBs = registerAllBs(beams, beamSigOrdinals, bAliases);
                    final RegistrationBatch batch = registrations(
                            sheet, filamentsBefore, glyphIndex.since(registrationMark), origins,
                            "beamBuilderChunk",
                            "beam:" + system.getId() + ":" + beamBuilderOrdinal);
                    for (int registrationOrdinal = 0;
                            registrationOrdinal < batch.events.size(); registrationOrdinal++) {
                        final Registration event = batch.events.get(registrationOrdinal);
                        final OriginRegistration beamOnlyRegistration = beamOnlyOrigins.register(
                                event.canonical,
                                "beamBuilderChunk",
                                "beam:" + system.getId() + ":" + beamBuilderOrdinal + ":"
                                        + registrationOrdinal);
                        final String beamOnlyPreOrigins = beamOnlyRegistration.priorCategories;
                        final boolean beamOnlyReuse = beamOnlyRegistration.reuse;
                        final boolean actionDiff = beamOnlyReuse != event.modeledReuse;
                        emit(String.format(
                                "stemsheadbuilderbeamreg %s system %d builder %d ordinal %d "
                                        + "result %s canonicalAlias %s canonical %s "
                                        + "preOriginCategories %s "
                                        + "beamOnlyResult %s beamOnlyPreOriginCategories %s "
                                        + "actionDiff %s headToLaterBeamReuse %s "
                                        + "bounds %s orientation %s weight %d runs %d sha256 %s",
                                page, system.getId(), beamBuilderOrdinal, registrationOrdinal,
                                event.modeledReuse ? "Reuse" : "New", event.canonicalAlias,
                                glyphAlias(event.canonical),
                                event.origins, beamOnlyReuse ? "Reuse" : "New",
                                beamOnlyPreOrigins, actionDiff,
                                actionDiff && event.origins.contains("headBuilderChunk"),
                                rectangle(event.canonical.getBounds()),
                                event.canonical.getRunTable().getOrientation(),
                                event.canonical.getWeight(), glyphRunCount(event.canonical),
                                glyphRunSha(event.canonical)), hash, pageHash);
                        if (actionDiff) totals.beamActionDiffs++;
                        if (actionDiff && event.origins.contains("headBuilderChunk")) {
                            totals.headToLaterBeamReuses++;
                        }
                    }
                    emit(String.format(
                            "stemsheadbuilderbeamchron %s system %d ordinal %d beamSig %d "
                                    + "bAlias %s vSide %s profile %d filaments %d glyphNew %d "
                                    + "glyphReuse %d bArenaBefore %d bArenaAfter %d "
                                    + "modeledRegistryBefore %d modeledRegistryAfter %d "
                                    + "registryShaBefore %s registryShaAfter %s "
                                    + "registrationActionSha256 %s",
                            page,
                            system.getId(),
                            beamBuilderOrdinal++,
                            beamSigOrdinals.get(beam),
                            bAliases.get(b),
                            entry.getKey(),
                            maxProfile,
                            batch.events.size(),
                            batch.news,
                            batch.reuses,
                            beforeBs,
                            afterBs,
                            batch.modeledBefore,
                            batch.modeledAfter,
                            batch.registryShaBefore,
                            batch.registryShaAfter,
                            batch.actionSha), hash, pageHash);
                    totals.beamBuilders++;
                    totals.beamFilaments += batch.events.size();
                    totals.beamGlyphNews += batch.news;
                    totals.beamGlyphReuses += batch.reuses;
                }
            }
        }

        final int expectedBeamBuilders = countVBuilders(beams);
        if (beamBuilderOrdinal != expectedBeamBuilders) {
            throw new IllegalStateException("beam builder chronology count mismatch");
        }

        final int sigVertices = system.getSig().vertexSet().size();
        final int sigEdges = system.getSig().edgeSet().size();
        final int stemInters = system.getSig().inters(StemInter.class).size();
        final int systemStems = ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size();
        final IdentityHashMap<Object, String> initialLinkStates = linkStates(beams, cAliases);

        int builderOrdinal = 0;
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            emit(String.format(
                    "stemsheadbuilderhead %s system %d xOrdinal %d sigOrdinal %d staff %d "
                            + "shape %s bounds %s center %d:%d grade %s vip %s "
                            + "glyph %s glyphWeight %d glyphRuns %d",
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
                    head.isVip(),
                    glyphAlias(head.getGlyph()),
                    head.getGlyph().getWeight(),
                    glyphRunCount(head.getGlyph())), hash, pageHash);
            if (head.isVip()) totals.vipHeads++;
            if (head.getShape().isSmallHead()) totals.smallHeads++;

            int cornerOrdinal = 0;
            for (HeadCorner corner : HeadCorner.values()) {
                final Object c = head.getLinker().getCornerLinker(corner.hSide, corner.vSide);
                inspectC(
                        page,
                        sheet,
                        system,
                        retriever,
                        head,
                        headOrdinal,
                        corner,
                        cornerOrdinal++,
                        c,
                        builderOrdinal++,
                        beams,
                        beamSigOrdinals,
                        bAliases,
                        cAliases,
                        origins,
                        glyphIndex,
                        params,
                        totals,
                        hash,
                        pageHash);
            }
        }

        if (builderOrdinal != 4 * heads.size()) {
            throw new IllegalStateException("head builder count mismatch");
        }
        if (!sameIdentityList(seeds, seedSnapshot)
                || system.getSig().vertexSet().size() != sigVertices
                || system.getSig().edgeSet().size() != sigEdges
                || system.getSig().inters(StemInter.class).size() != stemInters
                || ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size() != systemStems) {
            throw new IllegalStateException("builder chronology mutated forbidden system state");
        }
        assertLinkStates(initialLinkStates, beams, cAliases);
        for (Object c : cAliases.keySet()) {
            if (C_STEM_BUILDER.get(c) == null) {
                throw new IllegalStateException("C builder assignment missing");
            }
        }
        emit(String.format(
                "stemsheadbuildersystemsummary %s system %d %s hash %016x",
                page, system.getId(), totals.fields(), hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void inspectC (String page,
                                  Sheet sheet,
                                  SystemInfo system,
                                  StemsRetriever retriever,
                                  HeadInter head,
                                  int headOrdinal,
                                  HeadCorner corner,
                                  int cornerOrdinal,
                                  Object c,
                                  int builderOrdinal,
                                  List<Inter> beams,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                  IdentityHashMap<Object, String> bAliases,
                                  IdentityHashMap<Object, String> cAliases,
                                  OriginRegistry origins,
                                  TrackingGlyphIndex glyphIndex,
                                  Object params,
                                  Totals totals,
                                  RowHasher... hashes)
        throws Exception
    {
        if (C_STEM_BUILDER.get(c) != null || C_SEEDS.get(c) != null) {
            throw new IllegalStateException("C inspected more than once");
        }
        final VerticalSide vSide = (VerticalSide) C_V_SIDE.get(c);
        final int cYDir = C_Y_DIR.getInt(c);
        final Line2D theo = (Line2D) C_THEO_LINE.get(c);
        final int builderYDir = theo.getY2() > theo.getY1() ? 1 : -1;
        final Area area = (Area) C_LU_AREA.get(c);
        final StemLinker linker = (StemLinker) c;
        if (vSide != corner.vSide || cYDir != vSide.direction()) {
            throw new IllegalStateException("HeadCorner/CLinker direction mismatch");
        }
        final List<StraightFilament> filamentsBefore = filaments(sheet);
        final int modeledRegistryBefore = origins.count();
        final int bArenaBefore = registerAllBs(beams, beamSigOrdinals, bAliases);
        final int sigVerticesBefore = system.getSig().vertexSet().size();
        final int sigEdgesBefore = system.getSig().edgeSet().size();
        final int stemIntersBefore = system.getSig().inters(StemInter.class).size();
        final int systemStemsBefore = ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size();
        final IdentityHashMap<Object, Object> buildersBefore = builderStates(beams, cAliases);
        final IdentityHashMap<Object, String> linksBefore = linkStates(beams, cAliases);
        final int profile = sheet.getStub().getProfile();
        final List<Glyph> inputSeeds = new ArrayList<>((List<Glyph>) C_RETRIEVE_SEEDS.invoke(c));
        replayRetrieveSeeds(
                page, system, retriever, head, c, builderOrdinal, inputSeeds, params, totals,
                hashes);
        final List<Object> inputCs = new ArrayList<>((List<Object>) C_LOOKUP_OTHER_HEADS.invoke(c));
        final List<PredictedB> predictedBs = predictBeamTargets(
                head, corner, c, theo, beams, beamSigOrdinals, bAliases, params);
        emitInputRows(
                page,
                system,
                builderOrdinal,
                inputSeeds,
                inputCs,
                predictedBs,
                bAliases,
                cAliases,
                hashes);

        emit(String.format(
                "stemsheadbuilder %s system %d builder %d head %d cornerOrdinal %d corner %s "
                        + "alias %s hSide %s vSide %s profile %d cYDir %d builderYDir %d "
                        + "directionDiverges %s ref %s theo %s luBounds %s startStump %s "
                        + "modeledRegistryBefore %d bArenaBefore %d",
                page,
                system.getId(),
                builderOrdinal,
                headOrdinal,
                cornerOrdinal,
                corner.getId(),
                cAliases.get(c),
                corner.hSide,
                corner.vSide,
                profile,
                cYDir,
                builderYDir,
                cYDir != builderYDir,
                point(linker.getReferencePoint()),
                line(theo),
                rectangle(area.getBounds()),
                glyphAlias(linker.getStump()),
                modeledRegistryBefore,
                bArenaBefore), hashes);

        final int registrationMark = glyphIndex.mark();
        C_INSPECT.invoke(c, profile);
        final StemBuilder builder = (StemBuilder) C_STEM_BUILDER.get(c);
        if (builder == null || STEM_BUILDER_Y_DIR.getInt(builder) != builderYDir) {
            throw new IllegalStateException("C inspect failed builder assignment/direction");
        }
        final int bArenaAfter = registerAllBs(beams, beamSigOrdinals, bAliases);
        final List<Object> inputBs = resolvePredictedBs(predictedBs, bAliases);
        final List<Object> inputTargets = new ArrayList<>(inputCs);
        inputTargets.addAll(inputBs);
        final RegistrationBatch batch = registrations(
                sheet, filamentsBefore, glyphIndex.since(registrationMark), origins,
                "headBuilderChunk", "head:" + system.getId() + ":" + builderOrdinal);

        emitSectionScan(
                page,
                system,
                builderOrdinal,
                builder,
                c,
                params,
                totals,
                hashes);
        emitRegistrations(
                page,
                system,
                builderOrdinal,
                batch,
                totals,
                hashes);
        replayConstructor(
                page,
                sheet,
                system,
                retriever,
                builderOrdinal,
                head,
                c,
                builder,
                profile,
                inputSeeds,
                inputTargets,
                batch,
                params,
                bAliases,
                cAliases,
                totals,
                hashes);

        if (system.getSig().vertexSet().size() != sigVerticesBefore
                || system.getSig().edgeSet().size() != sigEdgesBefore
                || system.getSig().inters(StemInter.class).size() != stemIntersBefore
                || ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size()
                        != systemStemsBefore) {
            throw new IllegalStateException("C builder mutated SIG/systemStems");
        }
        assertOnlyBuilderAssigned(buildersBefore, beams, cAliases, c, builder);
        assertLinkStates(linksBefore, beams, cAliases);
        emit(String.format(
                "stemsheadbuilderend %s system %d builder %d seeds %s items %d lengths 5 "
                        + "filaments %d glyphNew %d glyphReuse %d bArenaBefore %d "
                        + "bArenaAfter %d anchorsCreated %d "
                        + "sigVertexDelta 0 sigEdgeDelta 0 stemInterDelta 0 "
                        + "systemStemDelta 0 linkMutations 0 sbAssigned true "
                        + "modeledRegistryBefore %d modeledRegistryAfter %d "
                        + "registryShaBefore %s registryShaAfter %s "
                        + "registrationActionSha256 %s",
                page,
                system.getId(),
                builderOrdinal,
                glyphAliases((List<Glyph>) C_SEEDS.get(c)),
                ((List<StemItem>) STEM_BUILDER_ITEMS.get(builder)).size(),
                batch.events.size(),
                batch.news,
                batch.reuses,
                bArenaBefore,
                bArenaAfter,
                bArenaAfter - bArenaBefore,
                batch.modeledBefore,
                batch.modeledAfter,
                batch.registryShaBefore,
                batch.registryShaAfter,
                batch.actionSha), hashes);

        totals.builders++;
        if (head.isVip()) totals.vipBuilders++;
        if (builderYDir < 0) totals.topBuilders++; else totals.bottomBuilders++;
        if (cYDir != builderYDir) totals.directionDivergences++;
        if (linker.getStump() == null) totals.stumplessStarts++;
        totals.anchorsCreated += bArenaAfter - bArenaBefore;
        totals.filaments += batch.events.size();
        totals.glyphNews += batch.news;
        totals.glyphReuses += batch.reuses;
    }

    private static void emitSectionScan (String page,
                                         SystemInfo system,
                                         int builderOrdinal,
                                         StemBuilder builder,
                                         Object c,
                                         Object params,
                                         Totals totals,
                                         RowHasher... hashes)
        throws Exception
    {
        final Area area = (Area) C_LU_AREA.get(c);
        final Line2D theo = (Line2D) C_THEO_LINE.get(c);
        final StemLinker linker = (StemLinker) c;
        final Glyph stump = linker.getStump();
        final Rectangle stumpBox = stump != null ? stump.getBounds() : null;
        final Double lastHeadY = (Double) STEM_BUILDER_LAST_HEAD_Y.get(builder);
        final int yDir = STEM_BUILDER_Y_DIR.getInt(builder);
        final int maxThickness = PARAM_MAX_STEM_THICKNESS.getInt(params);
        final double maxDistance = PARAM_MAX_LINE_SECTION_DX.getDouble(params);

        final List<Section> verticals = new ArrayList<>(system.getVerticalSections());
        final List<Section> acceptedV = new ArrayList<>();
        final TreeMap<String, Integer> vReasons = new TreeMap<>();
        final MessageDigest vRejects = sha256();
        for (int ordinal = 0; ordinal < verticals.size(); ordinal++) {
            final Section section = verticals.get(ordinal);
            final Rectangle box = section.getBounds();
            final String action;
            if (!area.intersects(box)) action = "outside";
            else if (box.width > maxThickness) action = "wide";
            else if (stumpBox != null && GeoUtil.yOverlap(box, stumpBox) > 0
                    && box.height < stumpBox.height) action = "stumpOverlap";
            else {
                final Point2D center = section.getCentroid2D();
                if (lastHeadY != null && yDir * Double.compare(center.getY(), lastHeadY) >= 0) {
                    action = "pastHead";
                } else if (theo.ptLineDist(center) > maxDistance) {
                    action = "distance";
                } else {
                    action = "accept";
                    acceptedV.add(section);
                }
            }
            vReasons.merge(action, 1, Integer::sum);
            if (!action.equals("accept")) {
                update(vRejects, ordinal + ":" + rectangle(box) + ":" + action + "\n");
            }
        }
        Collections.sort(acceptedV, Section.byFullPosition);
        emit(String.format(
                "stemsheadbuildervsection %s system %d builder %d sourceCount %d "
                        + "acceptedCount %d acceptedSourceOrdinals %s reasons %s "
                        + "rejectSha256 %s",
                page,
                system.getId(),
                builderOrdinal,
                verticals.size(),
                acceptedV.size(),
                selectedOrdinals(acceptedV, verticals),
                reasonToken(vReasons),
                hex(vRejects.digest())), hashes);
        totals.vScans += verticals.size();
        totals.vAccepts += acceptedV.size();

        final List<Section> horizontals = new ArrayList<>(system.getHorizontalSections());
        final List<Section> acceptedH = new ArrayList<>();
        final TreeMap<String, Integer> hReasons = new TreeMap<>();
        final MessageDigest hRejects = sha256();
        for (int ordinal = 0; ordinal < horizontals.size(); ordinal++) {
            final Section section = horizontals.get(ordinal);
            final Rectangle box = section.getBounds();
            final String action;
            if (!area.intersects(box)) action = "outside";
            else if (box.width > 1) action = "wide";
            else if (lastHeadY != null
                    && yDir * Double.compare(section.getCentroid2D().getY(), lastHeadY) >= 0) {
                action = "pastHead";
            } else {
                action = "accept";
                acceptedH.add(section);
            }
            hReasons.merge(action, 1, Integer::sum);
            if (!action.equals("accept")) {
                update(hRejects, ordinal + ":" + rectangle(box) + ":" + action + "\n");
            }
        }
        Collections.sort(acceptedH, Section.byFullPosition);
        emit(String.format(
                "stemsheadbuilderhsection %s system %d builder %d sourceCount %d "
                        + "acceptedCount %d acceptedSourceOrdinals %s reasons %s "
                        + "rejectSha256 %s",
                page,
                system.getId(),
                builderOrdinal,
                horizontals.size(),
                acceptedH.size(),
                selectedOrdinals(acceptedH, horizontals),
                reasonToken(hReasons),
                hex(hRejects.digest())), hashes);
        totals.hScans += horizontals.size();
        totals.hAccepts += acceptedH.size();
    }

    private static void emitStumpRegistrations (String page,
                                                SystemInfo system,
                                                List<StumpEvent> events,
                                                OriginRegistry origins,
                                                OriginRegistry beamOnlyOrigins,
                                                Totals totals,
                                                RowHasher... hashes)
    {
        for (int ordinal = 0; ordinal < events.size(); ordinal++) {
            final StumpEvent event = events.get(ordinal);
            final String category = event.kind.equals("Beam")
                    ? "beamStumpCandidate" : "headStumpCandidate";
            final String source = "stump:" + system.getId() + ":" + ordinal + ":"
                    + event.source;
            final OriginRegistration modeled = origins.register(
                    event.attempt.canonical, category, source);
            final OriginRegistration isolated = beamOnlyOrigins.register(
                    event.attempt.canonical, category, source);
            if (event.attempt.reuse != modeled.reuse) {
                throw new IllegalStateException(
                        "external GlyphIndex stump action differs from bounded registry at "
                                + source);
            }
            final boolean actionDiff = isolated.reuse != modeled.reuse;
            final String attachmentCategory = event.attached
                    ? (event.kind.equals("Beam") ? "beamStump" : "headStump")
                    : (event.kind.equals("Beam") ? "rejectedBeamStump" : "rejectedHeadStump");
            origins.label(event.attempt.canonical, attachmentCategory);
            beamOnlyOrigins.label(event.attempt.canonical, attachmentCategory);
            emit(String.format(
                    "stemsheadbuilderstumpreg %s system %d event %d kind %s source %s "
                            + "result %s beamOnlyResult %s actionDiff %s "
                            + "canonicalAlias %s canonical %s "
                            + "preOriginCategories %s attachment %s bounds %s orientation %s "
                            + "weight %d runs %d sha256 %s",
                    page,
                    system.getId(),
                    ordinal,
                    event.kind,
                    event.source,
                    modeled.reuse ? "Reuse" : "New",
                    isolated.reuse ? "Reuse" : "New",
                    actionDiff,
                    modeled.canonicalAlias,
                    glyphAlias(event.attempt.canonical),
                    modeled.priorCategories,
                    event.attached ? "Attached" : "RejectedAfterRegistration",
                    rectangle(event.attempt.canonical.getBounds()),
                    event.attempt.canonical.getRunTable().getOrientation(),
                    event.attempt.canonical.getWeight(),
                    glyphRunCount(event.attempt.canonical),
                    glyphRunSha(event.attempt.canonical)), hashes);
            totals.stumpRegistrations++;
            if (modeled.reuse) totals.stumpGlyphReuses++; else totals.stumpGlyphNews++;
            if (actionDiff) totals.stumpActionDiffs++;
        }
    }

    private static void emitRegistrations (String page,
                                           SystemInfo system,
                                           int builderOrdinal,
                                           RegistrationBatch batch,
                                           Totals totals,
                                           RowHasher... hashes)
        throws Exception
    {
        for (int ordinal = 0; ordinal < batch.events.size(); ordinal++) {
            final Registration event = batch.events.get(ordinal);
            final StraightFilament filament = event.filament;
            final StringBuilder members = new StringBuilder();
            int memberOrdinal = 0;
            for (Section member : filament.getMembers()) {
                final String alias = sectionAlias(member, system);
                append(members, alias);
                emit(String.format(
                        "stemsheadbuilderfilamentmember %s system %d builder %d filament %d "
                                + "ordinal %d alias %s orientation %s bounds %s weight %d "
                                + "runs %d",
                        page,
                        system.getId(),
                        builderOrdinal,
                        ordinal,
                        memberOrdinal++,
                        alias,
                        member.getOrientation(),
                        rectangle(member.getBounds()),
                        member.getWeight(),
                        member.getRunCount()), hashes);
                totals.filamentMembers++;
            }
            emit(String.format(
                    "stemsheadbuilderfilament %s system %d builder %d ordinal %d members %s "
                            + "bounds %s weight %d centerLine %s meanThickness %s "
                            + "meanDistance %s length %d",
                    page,
                    system.getId(),
                    builderOrdinal,
                    ordinal,
                    empty(members),
                    rectangle(filament.getBounds()),
                    filament.getWeight(),
                    line(filament.getCenterLine()),
                    hexDouble(filament.getMeanThickness(Orientation.VERTICAL)),
                    hexDouble(filament.getMeanDistance()),
                    filament.getLength(Orientation.VERTICAL)), hashes);
            emit(String.format(
                            "stemsheadbuilderglyphreg %s system %d builder %d ordinal %d "
                            + "result %s canonicalAlias %s canonical %s "
                            + "originCategories %s bounds %s "
                            + "orientation %s weight %d runs %d sha256 %s",
                    page,
                    system.getId(),
                    builderOrdinal,
                    ordinal,
                    event.modeledReuse ? "Reuse" : "New",
                    event.canonicalAlias,
                    glyphAlias(event.canonical),
                    event.origins,
                    rectangle(event.canonical.getBounds()),
                    event.canonical.getRunTable().getOrientation(),
                    event.canonical.getWeight(),
                    glyphRunCount(event.canonical),
                    glyphRunSha(event.canonical)), hashes);
        }
    }

    private static void emitSeedDuplicateCensus (String page,
                                                 SystemInfo system,
                                                 List<Glyph> seeds,
                                                 Totals totals,
                                                 RowHasher... hashes)
    {
        final TreeMap<String, Integer> counts = new TreeMap<>();
        for (Glyph seed : seeds) counts.merge(glyphAlias(seed), 1, Integer::sum);
        int keys = 0;
        int extraOccurrences = 0;
        final MessageDigest digest = sha256();
        for (Map.Entry<String, Integer> entry : counts.entrySet()) {
            if (entry.getValue() <= 1) continue;
            keys++;
            extraOccurrences += entry.getValue() - 1;
            update(digest, entry.getKey() + ":" + entry.getValue() + "\n");
        }
        emit(String.format(
                "stemsheadbuilderseedduplicates %s system %d keptSeeds %d "
                        + "duplicateStructuralKeys %d extraOccurrences %d duplicateSha256 %s",
                page, system.getId(), seeds.size(), keys, extraOccurrences,
                hex(digest.digest())), hashes);
        totals.seedDuplicateKeys += keys;
        totals.seedDuplicateExtraOccurrences += extraOccurrences;
    }

    private static List<PredictedB> predictBeamTargets (
            HeadInter head,
            HeadCorner corner,
            Object c,
            Line2D theo,
            List<Inter> beams,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<Object, String> bAliases,
            Object params)
        throws Exception
    {
        final List<PredictedB> result = new ArrayList<>();
        final AbstractBeamInter targetBeam = (AbstractBeamInter) C_TARGET_BEAM.get(c);
        if (targetBeam == null) return result;
        if (head.getShape() == org.audiveris.omr.glyph.Shape.NOTEHEAD_VOID
                && corner.vSide.direction() == corner.hSide.direction()) {
            return result;
        }
        final Point2D cross = LineUtil.intersection(targetBeam.getMedian(), theo);
        final List<AbstractBeamInter> siblings = targetBeam.getLinker().getSiblingBeamsAt(cross);
        final double maxDx = PARAM_MAX_BEAM_LINKER_DX.getDouble(params);
        for (AbstractBeamInter sibling : siblings) {
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(sibling.getLinker());
            Object best = null;
            double bestDx = Double.MAX_VALUE;
            final Point2D beamCross = LineUtil.intersection(theo, sibling.getMedian());
            for (Object b : allB) {
                final double dx = Math.abs(((Point2D) B_REF_PT.get(b)).getX() - beamCross.getX());
                if (bestDx > dx) {
                    bestDx = dx;
                    best = b;
                }
            }
            final boolean reuse = bestDx <= maxDx;
            final String predictedAlias = reuse ? bAliases.get(best)
                    : "beam:" + beamSigOrdinals.get(sibling) + ":b:" + allB.size();
            result.add(new PredictedB(
                    sibling, allB.size(), best, bestDx, beamCross, reuse, predictedAlias));
        }
        return result;
    }

    private static void replayRetrieveSeeds (String page,
                                             SystemInfo system,
                                             StemsRetriever retriever,
                                             HeadInter head,
                                             Object c,
                                             int builderOrdinal,
                                             List<Glyph> actual,
                                             Object params,
                                             Totals totals,
                                             RowHasher... hashes)
        throws Exception
    {
        final List<Glyph> systemSeeds = (List<Glyph>) RETRIEVER_SYSTEM_SEEDS.get(retriever);
        final IdentityHashMap<Glyph, Integer> seedOrdinals = identityOrdinals(systemSeeds);
        final Set<Glyph> neighbors = (Set<Glyph>) HEAD_NEIGHBOR_SEEDS.get(head.getLinker());
        final Area area = (Area) C_LU_AREA.get(c);
        final Glyph stump = ((StemLinker) c).getStump();
        final Rectangle stumpBox = stump != null ? stump.getBounds() : null;
        final Rectangle yRange = (Rectangle) C_Y_RANGE.get(c);
        final Line2D theo = (Line2D) C_THEO_LINE.get(c);
        final int minContrib = PARAM_MIN_SEED_CONTRIB.getInt(params);
        final double maxDistance = PARAM_MAX_LINE_SEED_DX.getDouble(params);
        final List<SeedOccurrence> prelim = new ArrayList<>();
        int sourceOrdinal = 0;
        for (Glyph seed : neighbors) {
            final Integer systemOrdinal = seedOrdinals.get(seed);
            if (systemOrdinal == null) {
                throw new IllegalStateException("neighbor seed outside current system pool");
            }
            final Rectangle box = seed.getBounds();
            final int contribution = Math.max(0, GeoUtil.yOverlap(yRange, box));
            final double distance = theo.ptLineDist(seed.getCentroid());
            final String action;
            if (!area.intersects(box)) action = "outside";
            else if (stumpBox != null && GeoUtil.yOverlap(box, stumpBox) > 0) action = "stump";
            else if (contribution < minContrib) action = "contrib";
            else if (distance > maxDistance) action = "distance";
            else {
                action = "prelim";
                prelim.add(new SeedOccurrence(
                        seed, sourceOrdinal, systemOrdinal, contribution, distance));
            }
            emit(String.format(
                    "stemsheadbuilderseedsource %s system %d builder %d sourceOrdinal %d "
                            + "systemSeedOrdinal %d glyph %s bounds %s contrib %d "
                            + "minContrib %d distance %s maxDistance %s action %s",
                    page, system.getId(), builderOrdinal, sourceOrdinal++, systemOrdinal,
                    glyphAlias(seed), rectangle(box), contribution, minContrib,
                    hexDouble(distance), hexDouble(maxDistance), action), hashes);
            totals.seedSourceScans++;
        }
        final List<SeedOccurrence> before = new ArrayList<>(prelim);
        Collections.sort(
                prelim,
                (left, right) -> Integer.compare(right.contrib, left.contrib));
        totals.maxRetrieveSeedSortItems = Math.max(
                totals.maxRetrieveSeedSortItems, before.size());
        if (before.size() >= 32) totals.retrieveSeedSortAtLeast32++;
        emit(String.format(
                "stemsheadbuildersortaudit %s system %d builder %d phase retrieveSeeds "
                        + "items %d strictCycles 0 equivalenceInconsistencies 0 "
                        + "offenderSha256 %s jdk25MiniTimSort %s",
                page, system.getId(), builderOrdinal, before.size(),
                hex(sha256().digest()), before.size() < 32), hashes);
        totals.sortAudits++;
        for (int input = 0; input < before.size(); input++) {
            final SeedOccurrence seed = before.get(input);
            emit(String.format(
                    "stemsheadbuilderseedsort %s system %d builder %d input %d output %d "
                            + "sourceOrdinal %d systemSeedOrdinal %d glyph %s contrib %d",
                    page, system.getId(), builderOrdinal, input, identityIndex(prelim, seed),
                    seed.sourceOrdinal, seed.systemSeedOrdinal, glyphAlias(seed.glyph),
                    seed.contrib), hashes);
            totals.retrieveSeedSortRows++;
        }
        final List<SeedOccurrence> kept = new ArrayList<>();
        for (SeedOccurrence seed : prelim) {
            SeedOccurrence conflict = null;
            for (SeedOccurrence previous : kept) {
                if (GeoUtil.yOverlap(seed.glyph.getBounds(), previous.glyph.getBounds()) > 0) {
                    conflict = previous;
                    break;
                }
            }
            emit(String.format(
                    "stemsheadbuilderseedoverlap %s system %d builder %d sortedOrdinal %d "
                            + "glyph %s conflict %s action %s",
                    page, system.getId(), builderOrdinal, identityIndex(prelim, seed),
                    glyphAlias(seed.glyph), conflict != null ? glyphAlias(conflict.glyph) : "-",
                    conflict != null ? "remove" : "keep"), hashes);
            if (conflict == null) kept.add(seed);
        }
        final List<Glyph> replay = new ArrayList<>();
        for (SeedOccurrence seed : kept) replay.add(seed.glyph);
        if (!sameIdentityList(replay, actual)) {
            throw new IllegalStateException("retrieveSeeds differs from independent replay");
        }
    }


    private static List<Object> resolvePredictedBs (List<PredictedB> predictions,
                                                    IdentityHashMap<Object, String> bAliases)
        throws Exception
    {
        final List<Object> values = new ArrayList<>();
        for (PredictedB prediction : predictions) {
            final List<Object> allB = (List<Object>) BEAM_ALL_B.get(
                    prediction.beam.getLinker());
            final Object actual = prediction.reuse
                    ? prediction.best : allB.get(prediction.beforeCount);
            if (!prediction.alias.equals(bAliases.get(actual))) {
                throw new IllegalStateException("findLinker prediction differs from actual arena");
            }
            values.add(actual);
        }
        return values;
    }

    private static void emitInputRows (String page,
                                       SystemInfo system,
                                       int builderOrdinal,
                                       List<Glyph> seeds,
                                       List<Object> cs,
                                       List<PredictedB> bs,
                                       IdentityHashMap<Object, String> bAliases,
                                       IdentityHashMap<Object, String> cAliases,
                                       RowHasher... hashes)
    {
        for (int ordinal = 0; ordinal < seeds.size(); ordinal++) {
            emit(String.format(
                    "stemsheadbuilderinputseed %s system %d builder %d ordinal %d glyph %s "
                            + "bounds %s",
                    page, system.getId(), builderOrdinal, ordinal,
                    glyphAlias(seeds.get(ordinal)), rectangle(seeds.get(ordinal).getBounds())),
                    hashes);
        }
        int targetOrdinal = 0;
        for (Object c : cs) {
            emit(String.format(
                    "stemsheadbuilderinputtarget %s system %d builder %d ordinal %d "
                            + "kind C alias %s action existing",
                    page, system.getId(), builderOrdinal, targetOrdinal++, cAliases.get(c)), hashes);
        }
        for (PredictedB b : bs) {
            emit(String.format(
                    "stemsheadbuilderinputtarget %s system %d builder %d ordinal %d "
                            + "kind B alias %s action %s beforeArena %d best %s bestDx %s "
                            + "cross %s",
                    page, system.getId(), builderOrdinal, targetOrdinal++, b.alias,
                    b.reuse ? "reuse" : "createAnchor", b.beforeCount,
                    b.best != null ? bAliases.get(b.best) : "-", hexDouble(b.bestDx),
                    point(b.cross)), hashes);
        }
    }

    private static void replayConstructor (String page,
                                           Sheet sheet,
                                           SystemInfo system,
                                           StemsRetriever retriever,
                                           int builderOrdinal,
                                           HeadInter head,
                                           Object c,
                                           StemBuilder builder,
                                           int maxProfile,
                                           List<Glyph> inputSeeds,
                                           List<Object> inputTargets,
                                           RegistrationBatch batch,
                                           Object params,
                                           IdentityHashMap<Object, String> bAliases,
                                           IdentityHashMap<Object, String> cAliases,
                                           Totals totals,
                                           RowHasher... hashes)
        throws Exception
    {
        final StemLinker start = (StemLinker) c;
        final Glyph startStump = start.getStump();
        final Line2D theo = (Line2D) C_THEO_LINE.get(c);
        final Rectangle yRange = (Rectangle) STEM_BUILDER_Y_RANGE.get(builder);
        final int yDir = STEM_BUILDER_Y_DIR.getInt(builder);
        final double maxDx = PARAM_MAX_STEM_ALIGNMENT_DX.getDouble(params);
        final double maxDy = PARAM_MAX_STEM_ALIGNMENT_DY.getDouble(params);

        final List<Glyph> seeds = new ArrayList<>(inputSeeds);
        final List<String> seedOccurrenceAliases = new ArrayList<>();
        for (int ordinal = 0; ordinal < inputSeeds.size(); ordinal++) {
            seedOccurrenceAliases.add("seedInput:" + ordinal);
        }
        final List<Glyph> removedSeeds = replayUnaligned(
                page, system, builderOrdinal, "seed", seeds, seedOccurrenceAliases,
                startStump, yDir,
                maxDx, maxDy, totals, hashes);

        final List<ReplayItem> targets = new ArrayList<>();
        for (int ordinal = 0; ordinal < inputTargets.size(); ordinal++) {
            final StemLinker target = (StemLinker) inputTargets.get(ordinal);
            final Glyph stump = target.getStump();
            final boolean removed = removedSeeds.contains(stump);
            final String kind = target instanceof StemHalfLinker ? "C" : "B";
            final String alias = linkerAlias(target, bAliases, cAliases);
            emit(String.format(
                    "stemsheadbuildertargetfilter %s system %d builder %d ordinal %d "
                            + "kind %s alias %s stump %s removedByStructuralSeed %s action %s",
                    page, system.getId(), builderOrdinal, ordinal, kind, alias,
                    glyphAlias(stump), removed, removed ? "remove" : "keep"), hashes);
            totals.inputTargets++;
            if (removed) {
                totals.removedTargets++;
                continue;
            }
            final int contribution = target instanceof StemHalfLinker
                    ? (stump != null ? contrib(yRange, stump.getBounds()) : 0)
                    : (stump != null ? stump.getBounds().height : 0);
            targets.add(new ReplayItem(
                    kind, alias, target, stump, linkerLine(target), contribution,
                    targets.size()));
        }
        sortReplayItems(
                page, system, builderOrdinal, "targets", targets, yDir, totals, hashes);

        final Double lastHeadY = (Double) STEM_BUILDER_LAST_HEAD_Y.get(builder);
        if (lastHeadY != null) {
            throw new IllegalStateException("C StemBuilder lastHeadY must remain null");
        }
        emit(String.format(
                "stemsheadbuilderlasthead %s system %d builder %d lastHeadY - "
                        + "pastSeedDrops 0 pastVSectionDrops 0 pastHSectionDrops 0",
                page, system.getId(), builderOrdinal), hashes);
        if (!sameIdentityList(seeds, (List<Glyph>) C_SEEDS.get(c))) {
            throw new IllegalStateException("C seed mutation differs from replay");
        }
        for (int ordinal = 0; ordinal < inputSeeds.size(); ordinal++) {
            final Glyph seed = inputSeeds.get(ordinal);
            emit(String.format(
                    "stemsheadbuilderseedfilter %s system %d builder %d ordinal %d glyph %s "
                            + "removedStructural %s retainedIdentity %s pastLastHead false",
                    page, system.getId(), builderOrdinal, ordinal, glyphAlias(seed),
                    removedSeeds.contains(seed), containsIdentity(seeds, seed)), hashes);
            totals.inputSeeds++;
        }

        final List<ChunkOccurrence> chunks = new ArrayList<>();
        for (int ordinal = 0; ordinal < batch.events.size(); ordinal++) {
            final Registration event = batch.events.get(ordinal);
            chunks.add(new ChunkOccurrence(
                    "chunkEvent:s" + system.getId() + ":c" + builderOrdinal + ":" + ordinal,
                    event.canonical,
                    ordinal));
        }
        emitChunkDuplicateCensus(page, system, builderOrdinal, chunks, totals, hashes);

        // chunks.removeAll(seeds): all structurally equal occurrences disappear.
        for (ChunkOccurrence chunk : chunks) {
            if (seeds.contains(chunk.glyph)) chunk.action = "seedStructural";
        }
        final List<ChunkOccurrence> survivors = activeChunks(chunks);

        // C-specific source bug: low-remain chunks are removed only under head.isVip().
        final Glyph headGlyph = head.getGlyph();
        final Rectangle headBox = headGlyph.getBounds();
        final List<Glyph> headPartInput = chunkGlyphs(survivors);
        final List<Glyph> realHeadParts = new ArrayList<>(headPartInput);
        STEM_FILTER_HEAD_PARTS.invoke(builder, realHeadParts);
        for (ChunkOccurrence chunk : survivors) {
            final int removed = removedHeadPixels(chunk.glyph, headGlyph, headBox);
            final int remain = chunk.glyph.getWeight() - removed;
            final boolean remove = remain < 15 && head.isVip();
            emit(String.format(
                    "stemsheadbuilderheadparts %s system %d builder %d event %s glyph %s "
                            + "head %s yOverlap %d weight %d removed %d remain %d threshold 15 "
                            + "vip %s lowRemain %s action %s",
                    page, system.getId(), builderOrdinal, chunk.alias, glyphAlias(chunk.glyph),
                    glyphAlias(headGlyph), GeoUtil.yOverlap(chunk.glyph.getBounds(), headBox),
                    chunk.glyph.getWeight(), removed, remain, head.isVip(), remain < 15,
                    remove ? "removeVipOnly" : remain < 15 ? "keepNonVipBug" : "keep"), hashes);
            if (remain < 15) totals.lowRemainChunks++;
            if (remove) {
                chunk.action = "headParts";
                totals.headPartDrops++;
                totals.removeVipOnly++;
            } else if (remain < 15) {
                totals.keepNonVipBug++;
            }
        }
        final List<Glyph> replayHeadParts = chunkGlyphs(activeChunks(chunks));
        if (!sameIdentityList(realHeadParts, replayHeadParts)) {
            throw new IllegalStateException("filterHeadParts occurrence replay differs");
        }

        // filterUnaligned removes all structural equals of every selected alien.
        final List<ChunkOccurrence> beforeAlign = activeChunks(chunks);
        final List<Glyph> alignGlyphs = chunkGlyphs(beforeAlign);
        final List<String> alignAliases = new ArrayList<>();
        for (ChunkOccurrence chunk : beforeAlign) alignAliases.add(chunk.alias);
        final List<Glyph> unaligned = replayUnaligned(
                page, system, builderOrdinal, "chunk", alignGlyphs, alignAliases,
                startStump, yDir,
                maxDx, maxDy, totals, hashes);
        for (ChunkOccurrence chunk : beforeAlign) {
            if (unaligned.contains(chunk.glyph)) chunk.action = "unalignedStructural";
        }

        // chunks.remove(stump): only the first structural-equal occurrence.
        if (startStump != null) {
            for (ChunkOccurrence chunk : activeChunks(chunks)) {
                if (chunk.glyph.equals(startStump)) {
                    chunk.action = "startFirstStructural";
                    break;
                }
            }
        }
        final List<ChunkOccurrence> keptChunks = activeChunks(chunks);
        Collections.sort(
                keptChunks,
                (left, right) -> (yDir > 0 ? org.audiveris.omr.glyph.Glyphs.byOrdinate
                        : org.audiveris.omr.glyph.Glyphs.byReverseBottom)
                        .compare(left.glyph, right.glyph));
        for (int ordinal = 0; ordinal < keptChunks.size(); ordinal++) {
            keptChunks.get(ordinal).finalOrdinal = ordinal;
        }
        for (ChunkOccurrence chunk : chunks) {
            emit(String.format(
                    "stemsheadbuilderchunkfilter %s system %d builder %d inputOrdinal %d "
                            + "event %s glyph %s finalOrdinal %d action %s",
                    page, system.getId(), builderOrdinal, chunk.inputOrdinal, chunk.alias,
                    glyphAlias(chunk.glyph), chunk.finalOrdinal, chunk.action), hashes);
            if (!chunk.action.equals("keep")) totals.chunkDrops++;
        }

        final List<ReplayItem> preItems = new ArrayList<>();
        preItems.add(new ReplayItem(
                "startC", cAliases.get(c), start, startStump, linkerLine(start),
                startStump != null ? contrib(yRange, startStump.getBounds()) : 0, 0));
        for (ReplayItem target : targets) {
            preItems.add(new ReplayItem(
                    target.kind, target.alias, target.linker, target.glyph, target.line,
                    target.contrib, preItems.size()));
        }
        for (int ordinal = 0; ordinal < seeds.size(); ordinal++) {
            final Glyph seed = seeds.get(ordinal);
            StemLinker duplicate = null;
            for (ReplayItem target : targets) {
                if (seed == target.linker.getStump()) {
                    duplicate = target.linker;
                    break;
                }
            }
            final boolean startOverlap = startStump != null
                    && GeoUtil.yOverlap(startStump.getBounds(), seed.getBounds()) > 0;
            final int contribution = contrib(yRange, seed.getBounds());
            final String action = duplicate != null ? "duplicateTargetIdentity"
                    : startOverlap ? "startYOverlap" : contribution <= 0 ? "zeroContrib" : "keep";
            emit(String.format(
                    "stemsheadbuilderseeditemfilter %s system %d builder %d ordinal %d "
                            + "glyph %s duplicateTargetIdentity %s duplicateAlias %s "
                            + "startYOverlap %s contrib %d action %s",
                    page, system.getId(), builderOrdinal, ordinal, glyphAlias(seed),
                    duplicate != null,
                    duplicate != null ? linkerAlias(duplicate, bAliases, cAliases) : "-",
                    duplicate == null ? startOverlap : false,
                    duplicate == null && !startOverlap ? contribution : -1,
                    action), hashes);
            if (!action.equals("keep")) {
                totals.seedItemDrops++;
                continue;
            }
            preItems.add(new ReplayItem(
                    "seed", "seedInput:" + identityIndex(inputSeeds, seed), null, seed,
                    seed.getCenterLine(), contribution,
                    preItems.size()));
        }
        for (ChunkOccurrence chunk : keptChunks) {
            preItems.add(new ReplayItem(
                    "chunk", chunk.alias, null, chunk.glyph, chunk.glyph.getCenterLine(),
                    contrib(yRange, chunk.glyph.getBounds()), preItems.size()));
        }
        sortReplayItems(
                page, system, builderOrdinal, "items",
                preItems.subList(1, preItems.size()), yDir, totals, hashes);
        for (int ordinal = 0; ordinal < preItems.size(); ordinal++) {
            final ReplayItem item = preItems.get(ordinal);
            emit(String.format(
                    "stemsheadbuilderitempre %s system %d builder %d creationOrdinal %d "
                            + "sortedOrdinal %d kind %s alias %s line %s glyph %s contrib %d",
                    page, system.getId(), builderOrdinal, item.creationOrdinal, ordinal,
                    item.kind, item.alias, line(item.line), glyphAlias(item.glyph), item.contrib),
                    hashes);
            totals.preItems++;
        }

        final List<FinalReplayItem> finalItems = replayGaps(
                page, sheet, system, builderOrdinal, preItems, yDir, maxProfile, totals, hashes);
        compareActualItems(
                page, system, builderOrdinal, builder, finalItems, totals, hashes);
        replayLengths(
                page, sheet, system, retriever, builderOrdinal, builder, finalItems, theo,
                yDir, maxProfile, totals, hashes);
    }

    private static List<Glyph> replayUnaligned (String page,
                                               SystemInfo system,
                                               int builderOrdinal,
                                               String phase,
                                               List<Glyph> glyphs,
                                               List<String> aliases,
                                               Glyph startStump,
                                               int yDir,
                                               double maxDx,
                                               double maxDy,
                                               Totals totals,
                                               RowHasher... hashes)
    {
        if (glyphs.size() != aliases.size()) {
            throw new IllegalStateException("glyph occurrence alias count mismatch");
        }
        final List<Glyph> removed = new ArrayList<>();
        final List<GlyphOccurrence> inputs = new ArrayList<>();
        for (int ordinal = 0; ordinal < glyphs.size(); ordinal++) {
            inputs.add(new GlyphOccurrence(aliases.get(ordinal), glyphs.get(ordinal)));
        }
        final List<GlyphOccurrence> sorted = new ArrayList<>(inputs);
        Collections.sort(
                sorted,
                (left, right) -> (yDir > 0 ? org.audiveris.omr.glyph.Glyphs.byOrdinate
                        : org.audiveris.omr.glyph.Glyphs.byReverseBottom)
                        .compare(left.glyph, right.glyph));
        GlyphOccurrence promoted = null;
        if (startStump != null) {
            promoted = removeFirstEqualOccurrence(sorted, startStump);
            if (promoted == null) promoted = new GlyphOccurrence("startStumpSynthetic", startStump);
            sorted.add(0, promoted);
        }
        int ordinal = 0;
        for (int index = 0; index < sorted.size() - 1; index++) {
            final GlyphOccurrence first = sorted.get(index);
            final GlyphOccurrence second = sorted.get(index + 1);
            final Point2D firstDeskew = system.getSkew().deskewed(first.glyph.getCentroidDouble());
            final Point2D secondDeskew = system.getSkew().deskewed(second.glyph.getCentroidDouble());
            final double dy = Math.abs(secondDeskew.getY() - firstDeskew.getY());
            final boolean dyBypass = dy > maxDy;
            final double dx = dyBypass ? Double.NaN
                    : Math.abs(secondDeskew.getX() - firstDeskew.getX());
            final boolean aligned = dyBypass || dx <= maxDx;
            GlyphOccurrence selectedAlien = null;
            GlyphOccurrence actualRemoved = null;
            if (!aligned) {
                selectedAlien = first.glyph.getBounds().height < second.glyph.getBounds().height
                        ? first : second;
                removed.add(selectedAlien.glyph);
                actualRemoved = removeFirstEqualOccurrence(sorted, selectedAlien.glyph);
                index--;
                totals.alignRemovals++;
            }
            emit(String.format(
                    "stemsheadbuilderalign %s system %d builder %d phase %s ordinal %d "
                            + "startStump %s promotedOccurrence %s firstOccurrence %s "
                            + "secondOccurrence %s first %s second %s "
                            + "firstDeskew %s secondDeskew %s dy %s maxDy %s dyBypass %s "
                            + "dx %s maxDx %s aligned %s selectedAlienOccurrence %s "
                            + "actualRemovedOccurrence %s alien %s tieRemoveSecond %s",
                    page, system.getId(), builderOrdinal, phase, ordinal++, glyphAlias(startStump),
                    promoted != null ? promoted.alias : "-", first.alias, second.alias,
                    glyphAlias(first.glyph), glyphAlias(second.glyph),
                    point(firstDeskew), point(secondDeskew), hexDouble(dy), hexDouble(maxDy),
                    dyBypass, dyBypass ? "-" : hexDouble(dx), hexDouble(maxDx), aligned,
                    selectedAlien != null ? selectedAlien.alias : "-",
                    actualRemoved != null ? actualRemoved.alias : "-",
                    selectedAlien != null ? glyphAlias(selectedAlien.glyph) : "-", !aligned
                            && first.glyph.getBounds().height == second.glyph.getBounds().height), hashes);
            totals.alignComparisons++;
        }
        glyphs.removeAll(removed);
        final StringBuilder removedKeys = new StringBuilder();
        for (Glyph glyph : removed) append(removedKeys, glyphAlias(glyph));
        final StringBuilder retained = new StringBuilder();
        for (GlyphOccurrence input : inputs) {
            if (!removed.contains(input.glyph)) append(retained, input.alias);
        }
        emit(String.format(
                "stemsheadbuilderalignresult %s system %d builder %d phase %s "
                        + "removedStructuralKeys %s retainedOccurrences %s",
                page, system.getId(), builderOrdinal, phase, empty(removedKeys), empty(retained)),
                hashes);
        return removed;
    }

    private static void sortReplayItems (String page,
                                         SystemInfo system,
                                         int builderOrdinal,
                                         String phase,
                                         List<ReplayItem> items,
                                         int yDir,
                                         Totals totals,
                                         RowHasher... hashes)
    {
        final List<ReplayItem> before = new ArrayList<>(items);
        final Comparator<ReplayItem> comparator = replayComparator(yDir);
        if (phase.equals("targets")) {
            totals.maxTargetSortItems = Math.max(totals.maxTargetSortItems, before.size());
            if (before.size() >= 32) totals.targetSortAtLeast32++;
        } else {
            totals.maxFinalSortItems = Math.max(totals.maxFinalSortItems, before.size());
            if (before.size() >= 32) totals.finalSortAtLeast32++;
        }
        final SortAnomalies anomalies = auditComparator(before, comparator);
        emit(String.format(
                "stemsheadbuildersortaudit %s system %d builder %d phase %s items %d "
                        + "strictCycles %d equivalenceInconsistencies %d offenderSha256 %s "
                        + "jdk25MiniTimSort %s",
                page, system.getId(), builderOrdinal, phase, before.size(), anomalies.cycles,
                anomalies.equivalence, anomalies.digest, before.size() < 32), hashes);
        totals.sortAudits++;
        totals.sortCycles += anomalies.cycles;
        totals.sortEquivalence += anomalies.equivalence;
        Collections.sort(items, comparator);
        for (int input = 0; input < before.size(); input++) {
            final ReplayItem item = before.get(input);
            final int output = identityIndex(items, item);
            final StringBuilder equals = new StringBuilder();
            final StringBuilder predecessors = new StringBuilder();
            for (int other = 0; other < before.size(); other++) {
                if (other != input && comparator.compare(before.get(other), item) == 0) {
                    append(equals, Integer.toString(other));
                    if (other < input) append(predecessors, Integer.toString(other));
                }
            }
            emit(String.format(
                    "stemsheadbuildersort %s system %d builder %d phase %s input %d "
                            + "output %d alias %s kind %s line %s ref %s key1 %s key2 %s "
                            + "equalInputs %s stableEqualPredecessors %s",
                    page, system.getId(), builderOrdinal, phase, input, output, item.alias,
                    item.kind, line(item.line),
                    item.linker != null ? point(item.linker.getReferencePoint()) : "-",
                    hexDouble(item.line.getY1()), hexDouble(item.line.getY2()),
                    empty(equals), empty(predecessors)), hashes);
            totals.sortRows++;
        }
    }

    private static Comparator<ReplayItem> replayComparator (int yDir)
    {
        return (left,
                right) -> yDir * Double.compare(
                        ordinateKeyOf(left, yDir),
                        ordinateKeyOf(right, yDir));
    }

    private static double ordinateKeyOf (ReplayItem item,
                                         int yDir)
    {
        if (item.linker instanceof StemHalfLinker) {
            return item.linker.getReferencePoint().getY();
        }

        return yDir > 0 ? item.line.getY1() : item.line.getY2();
    }

    private static <T> SortAnomalies auditComparator (List<T> values,
                                                      Comparator<T> comparator)
    {
        long cycles = 0;
        long equivalence = 0;
        final MessageDigest digest = sha256();
        for (int i = 0; i < values.size(); i++) {
            for (int j = i + 1; j < values.size(); j++) {
                for (int k = j + 1; k < values.size(); k++) {
                    final int ij = Integer.signum(comparator.compare(values.get(i), values.get(j)));
                    final int jk = Integer.signum(comparator.compare(values.get(j), values.get(k)));
                    final int ki = Integer.signum(comparator.compare(values.get(k), values.get(i)));
                    final int ik = Integer.signum(comparator.compare(values.get(i), values.get(k)));
                    final int ji = -ij;
                    final int kj = -jk;
                    final boolean cycle = (ij < 0 && jk < 0 && ki < 0)
                            || (ij > 0 && jk > 0 && ki > 0);
                    boolean inconsistent = false;
                    if (ij == 0 && (ik != jk || -ik != ki)) inconsistent = true;
                    if (jk == 0 && (ji != ki || ij != ik)) inconsistent = true;
                    if (ik == 0 && (ij != kj || ji != jk)) inconsistent = true;
                    if (cycle || inconsistent) {
                        update(digest, i + ":" + j + ":" + k + ":" + ij + ":" + jk + ":"
                                + ki + ":" + ik + ":" + cycle + ":" + inconsistent + "\n");
                    }
                    if (cycle) cycles++;
                    if (inconsistent) equivalence++;
                }
            }
        }
        return new SortAnomalies(cycles, equivalence, hex(digest.digest()));
    }

    private static List<FinalReplayItem> replayGaps (String page,
                                                    Sheet sheet,
                                                    SystemInfo system,
                                                    int builderOrdinal,
                                                    List<ReplayItem> items,
                                                    int yDir,
                                                    int maxProfile,
                                                    Totals totals,
                                                    RowHasher... hashes)
    {
        final List<FinalReplayItem> result = new ArrayList<>();
        final int maxGap = sheet.getScale().toPixels(StemChecker.getMaxYGap(maxProfile));
        Point2D last = null;
        for (int index = 0; index < items.size(); index++) {
            final ReplayItem item = items.get(index);
            final Point2D start = yDir > 0 ? item.line.getP1() : item.line.getP2();
            final Point2D stop = yDir > 0 ? item.line.getP2() : item.line.getP1();
            final double gap = last != null ? yDir * (start.getY() - last.getY()) : Double.NaN;
            final String action;
            Line2D inserted = null;
            if (last == null) action = "initial";
            else if (gap > maxGap) action = "truncate";
            else if (gap > 0.01) {
                action = "insert";
                inserted = yDir > 0 ? new Line2D.Double(last, start)
                        : new Line2D.Double(start, last);
            } else action = "contiguous";
            emit(String.format(
                    "stemsheadbuildergap %s system %d builder %d ordinal %d itemIndex %d "
                            + "itemAlias %s start %s stop %s lastBefore %s gap %s maxGap %d "
                            + "epsilon %s action %s insertedLine %s insertedContrib %s",
                    page, system.getId(), builderOrdinal, index, index, item.alias, point(start),
                    point(stop), last != null ? point(last) : "-",
                    last != null ? hexDouble(gap) : "-", maxGap, hexDouble(0.01), action,
                    inserted != null ? line(inserted) : "-",
                    inserted != null ? Integer.toString(inserted.getBounds().height) : "-"),
                    hashes);
            totals.gapChecks++;
            if (action.equals("truncate")) {
                totals.gapTruncations++;
                break;
            }
            if (inserted != null) {
                result.add(new FinalReplayItem(
                        "gap", "gap:" + result.size(), null, null, inserted,
                        inserted.getBounds().height));
                totals.gapInserts++;
            }
            result.add(new FinalReplayItem(
                    item.kind, item.alias, item.linker, item.glyph, item.line, item.contrib));
            if (last == null || yDir * (stop.getY() - last.getY()) > 0.01) last = stop;
        }
        return result;
    }

    private static void compareActualItems (String page,
                                            SystemInfo system,
                                            int builderOrdinal,
                                            StemBuilder builder,
                                            List<FinalReplayItem> expected,
                                            Totals totals,
                                            RowHasher... hashes)
        throws Exception
    {
        final List<StemItem> actual = (List<StemItem>) STEM_BUILDER_ITEMS.get(builder);
        if (actual.size() != expected.size()) {
            throw new IllegalStateException("final item count differs from replay at builder "
                    + builderOrdinal + " expected=" + expected.size() + " actual=" + actual.size());
        }
        for (int ordinal = 0; ordinal < actual.size(); ordinal++) {
            final StemItem item = actual.get(ordinal);
            final FinalReplayItem replay = expected.get(ordinal);
            final Line2D actualLine = (Line2D) STEM_ITEM_LINE.get(item);
            final Glyph actualGlyph = (Glyph) STEM_ITEM_GLYPH.get(item);
            final int actualContrib = STEM_ITEM_CONTRIB.getInt(item);
            Object actualLinker = null;
            if (LINKER_ITEM_LINKER.getDeclaringClass().isAssignableFrom(item.getClass())) {
                actualLinker = LINKER_ITEM_LINKER.get(item);
            }
            if (actualLinker != replay.linker || actualGlyph != replay.glyph
                    || actualContrib != replay.contrib || !sameLine(actualLine, replay.line)) {
                throw new IllegalStateException("final item differs from replay at builder "
                        + builderOrdinal + " item " + ordinal);
            }
            emit(String.format(
                    "stemsheadbuilderitem %s system %d builder %d ordinal %d kind %s "
                            + "alias %s line %s glyph %s contrib %d",
                    page, system.getId(), builderOrdinal, ordinal, replay.kind, replay.alias,
                    line(actualLine), glyphAlias(actualGlyph), actualContrib), hashes);
            totals.items++;
            if (replay.kind.equals("gap")) totals.gaps++;
        }
    }

    private static void replayLengths (String page,
                                       Sheet sheet,
                                       SystemInfo system,
                                       StemsRetriever retriever,
                                       int builderOrdinal,
                                       StemBuilder builder,
                                       List<FinalReplayItem> items,
                                       Line2D theo,
                                       int yDir,
                                       int maxProfile,
                                       Totals totals,
                                       RowHasher... hashes)
        throws Exception
    {
        final TreeMap<Integer, Integer> actual =
                (TreeMap<Integer, Integer>) STEM_BUILDER_LENGTH_MAP.get(builder);
        final TreeMap<Integer, Integer> gaps =
                (TreeMap<Integer, Integer>) RETRIEVER_GET_GAP_MAP.invoke(retriever);
        final TreeMap<Integer, Integer> replay = new TreeMap<>();
        final int maxGap = gaps.get(maxProfile);
        for (int index = 0; index < items.size(); index++) {
            final FinalReplayItem item = items.get(index);
            if (!item.kind.equals("gap")) continue;
            for (Map.Entry<Integer, Integer> entry : gaps.entrySet()) {
                if (item.contrib > entry.getValue()) {
                    replay.putIfAbsent(
                            entry.getKey(), lengthAt(items, index - 1, theo, yDir));
                } else break;
            }
            if (item.contrib > maxGap) {
                replay.putIfAbsent(maxProfile, lengthAt(items, index - 1, theo, yDir));
                break;
            }
        }
        for (Map.Entry<Integer, Integer> entry : gaps.entrySet()) {
            replay.putIfAbsent(
                    entry.getKey(), lengthAt(items, items.size() - 1, theo, yDir));
        }
        if (!actual.equals(replay)) {
            throw new IllegalStateException("length map differs from replay at builder "
                    + builderOrdinal + " actual=" + actual + " replay=" + replay);
        }
        for (int profile = 0; profile <= Profiles.MAX_VALUE; profile++) {
            emit(String.format(
                    "stemsheadbuilderlength %s system %d builder %d profile %d threshold %d "
                            + "length %d replayLength %d",
                    page, system.getId(), builderOrdinal, profile,
                    sheet.getScale().toPixels(StemChecker.getMaxYGap(profile)),
                    actual.get(profile), replay.get(profile)), hashes);
            totals.lengths++;
        }
    }

    private static int lengthAt (List<FinalReplayItem> items,
                                 int lastIndex,
                                 Line2D theo,
                                 int yDir)
    {
        Rectangle rectangle = null;
        for (int index = 0; index <= lastIndex; index++) {
            final FinalReplayItem item = items.get(index);
            if (item.kind.equals("gap") || item.line == null) continue;
            if (rectangle == null) rectangle = item.line.getBounds();
            else rectangle.add(item.line.getBounds());
            if (item.linker instanceof StemHalfLinker
                    && item.linker.getSource() instanceof HeadInter head) {
                rectangle.add(head.getBounds());
            }
        }
        if (rectangle == null) return 0;
        return yDir > 0
                ? rectangle.y + rectangle.height - (int) theo.getY1()
                : (int) theo.getY1() - rectangle.y;
    }

    private static void emitChunkDuplicateCensus (String page,
                                                  SystemInfo system,
                                                  int builderOrdinal,
                                                  List<ChunkOccurrence> chunks,
                                                  Totals totals,
                                                  RowHasher... hashes)
    {
        final TreeMap<String, Integer> counts = new TreeMap<>();
        for (ChunkOccurrence chunk : chunks) counts.merge(glyphAlias(chunk.glyph), 1, Integer::sum);
        int keys = 0;
        int extra = 0;
        final MessageDigest digest = sha256();
        for (Map.Entry<String, Integer> entry : counts.entrySet()) {
            if (entry.getValue() <= 1) continue;
            keys++;
            extra += entry.getValue() - 1;
            update(digest, entry.getKey() + ":" + entry.getValue() + "\n");
        }
        emit(String.format(
                "stemsheadbuilderchunkduplicates %s system %d builder %d attempts %d "
                        + "duplicateStructuralKeys %d extraOccurrences %d duplicateSha256 %s",
                page, system.getId(), builderOrdinal, chunks.size(), keys, extra,
                hex(digest.digest())), hashes);
        totals.chunkDuplicateKeys += keys;
        totals.chunkDuplicateExtraOccurrences += extra;
    }

    private static List<ChunkOccurrence> activeChunks (List<ChunkOccurrence> chunks)
    {
        final List<ChunkOccurrence> active = new ArrayList<>();
        for (ChunkOccurrence chunk : chunks) {
            if (chunk.action.equals("keep")) active.add(chunk);
        }
        return active;
    }

    private static GlyphOccurrence removeFirstEqualOccurrence (
            List<GlyphOccurrence> occurrences,
            Glyph glyph)
    {
        for (int ordinal = 0; ordinal < occurrences.size(); ordinal++) {
            if (occurrences.get(ordinal).glyph.equals(glyph)) {
                return occurrences.remove(ordinal);
            }
        }
        return null;
    }

    private static List<Glyph> chunkGlyphs (List<ChunkOccurrence> chunks)
    {
        final List<Glyph> glyphs = new ArrayList<>();
        for (ChunkOccurrence chunk : chunks) glyphs.add(chunk.glyph);
        return glyphs;
    }

    private static int removedHeadPixels (Glyph chunk,
                                         Glyph head,
                                         Rectangle headBox)
    {
        if (GeoUtil.yOverlap(chunk.getBounds(), headBox) <= 0) return 0;
        int removed = 0;
        final PointsCollector points = chunk.getPointsCollector();
        final int[] xx = points.getXValues();
        final int[] yy = points.getYValues();
        final int yMin = headBox.y;
        final int yMax = headBox.y + headBox.height - 1;
        for (int index = 0; index < yy.length; index++) {
            if (yy[index] >= yMin && yy[index] <= yMax
                    && head.contains(new Point(xx[index], yy[index]))) {
                removed++;
            }
        }
        return removed;
    }

    private static int contrib (Rectangle yRange,
                                Rectangle box)
    {
        return Math.max(0, GeoUtil.yOverlap(yRange, box));
    }

    private static Line2D linkerLine (StemLinker linker)
    {
        final Glyph stump = linker.getStump();
        if (stump != null) return stump.getCenterLine();
        final Point2D ref = linker.getReferencePoint();
        if (linker.getClass().getSimpleName().equals("BLinker")
                && linker.getSource() instanceof AbstractBeamInter beam) {
            final double halfHeight = beam.getHeight() / 2.0;
            return new Line2D.Double(
                    ref.getX(), ref.getY() - halfHeight,
                    ref.getX(), ref.getY() + halfHeight);
        }
        return new Line2D.Double(ref, ref);
    }

    private static boolean sameLine (Line2D left,
                                     Line2D right)
    {
        return Double.doubleToLongBits(left.getX1()) == Double.doubleToLongBits(right.getX1())
                && Double.doubleToLongBits(left.getY1()) == Double.doubleToLongBits(right.getY1())
                && Double.doubleToLongBits(left.getX2()) == Double.doubleToLongBits(right.getX2())
                && Double.doubleToLongBits(left.getY2()) == Double.doubleToLongBits(right.getY2());
    }

    private static int identityIndex (List<?> values,
                                      Object target)
    {
        for (int index = 0; index < values.size(); index++) {
            if (values.get(index) == target) return index;
        }
        return -1;
    }

    private static RegistrationBatch registrations (Sheet sheet,
                                                    List<StraightFilament> filamentsBefore,
                                                    List<ActualRegistrationAttempt> attempts,
                                                    OriginRegistry origins,
                                                    String newCategory,
                                                    String sourcePrefix)
    {
        final int modeledBefore = origins.count();
        final String registryShaBefore = origins.historySha();
        final MessageDigest actionDigest = sha256();
        final Set<StraightFilament> before = identitySet(filamentsBefore);
        final List<StraightFilament> added = new ArrayList<>();
        for (StraightFilament filament : filaments(sheet)) {
            if (!before.contains(filament)) added.add(filament);
        }
        if (added.size() != attempts.size()) {
            throw new IllegalStateException(
                    "filament/registration attempt count mismatch: " + added.size() + "/"
                            + attempts.size());
        }
        final List<Registration> events = new ArrayList<>();
        int news = 0;
        int reuses = 0;
        for (int ordinal = 0; ordinal < added.size(); ordinal++) {
            final StraightFilament filament = added.get(ordinal);
            final Glyph candidate = filament.toGlyph(null);
            final ActualRegistrationAttempt attempt = attempts.get(ordinal);
            if (!candidate.equals(attempt.candidate)
                    || !candidate.equals(attempt.canonical)) {
                throw new IllegalStateException(
                        "filament/registration structural order mismatch at " + ordinal);
            }
            final OriginRegistration modeled = origins.register(
                    attempt.canonical, newCategory, sourcePrefix + ":" + ordinal);
            if (attempt.reuse != modeled.reuse) {
                throw new IllegalStateException(
                        "external GlyphIndex builder action differs from bounded registry at "
                                + sourcePrefix + ":" + ordinal);
            }
            if (modeled.reuse) {
                reuses++;
            } else {
                news++;
            }
            events.add(new Registration(
                    filament,
                    attempt.canonical,
                    modeled.reuse,
                    modeled.priorCategories,
                    modeled.canonicalAlias));
            update(actionDigest,
                    (modeled.reuse ? "Reuse" : "New") + ":"
                            + glyphAlias(attempt.canonical) + ":"
                            + modeled.priorCategories + "\n");
        }
        return new RegistrationBatch(
                events, news, reuses, modeledBefore, origins.count(), registryShaBefore,
                origins.historySha(), hex(actionDigest.digest()));
    }

    private static OriginRegistry buildPageOrigins (Sheet sheet,
                                                     List<Glyph> checkedSeedCandidates,
                                                     String page)
        throws IOException
    {
        final OriginRegistry origins = new OriginRegistry();
        // Mirror the bounded, projectable source order in native GlyphRegistry::seeded:
        // persistent staff lines, checked STEM_SEEDS registrations, raw beams, hooks, ledgers,
        // then concrete heads.  Unrelated live GlyphIndex contents are deliberately excluded;
        // registrations still use the real Java index to fail closed on any such collision.
        for (Staff staff : sheet.getStaffManager().getStaves()) {
            for (LineInfo line : staff.getLines()) {
                if (line instanceof StaffLine staffLine && staffLine.getGlyph() != null) {
                    origins.addBaseline(staffLine.getGlyph(), "grid");
                }
            }
        }
        for (Glyph glyph : checkedSeedCandidates) {
            origins.addBaseline(glyph, "checkedStemSeedCandidate");
        }

        final List<Glyph> rawBeams = new ArrayList<>();
        final List<Glyph> hooks = new ArrayList<>();
        for (SystemInfo system : sheet.getSystems()) {
            for (Inter inter : system.getSig().inters(AbstractBeamInter.class)) {
                if (inter.getGlyph() == null) continue;
                if (String.valueOf(inter.getShape()).contains("BEAM_HOOK")) {
                    hooks.add(inter.getGlyph());
                } else {
                    rawBeams.add(inter.getGlyph());
                }
            }
        }
        for (Glyph glyph : rawBeams) origins.addBaseline(glyph, "rawBeam");
        for (Glyph glyph : hooks) origins.addBaseline(glyph, "rawBeam");

        for (SystemInfo system : sheet.getSystems()) {
            for (Inter inter : system.getSig().vertexSet()) {
                final Glyph glyph = inter.getGlyph();
                if (glyph == null) continue;
                if (String.valueOf(inter.getShape()).contains("LEDGER")) {
                    origins.addBaseline(glyph, "ledger");
                }
            }
        }
        for (Glyph glyph : orderedHeadBaselineGlyphs(sheet, page)) {
            origins.addBaseline(glyph, "head");
        }
        origins.sealBaseline();
        return origins;
    }

    private static List<Glyph> orderedHeadBaselineGlyphs (Sheet sheet,
                                                          String page)
        throws IOException
    {
        final String seedFixture = System.getProperty("audiveris.rustport.headSeedFixture");
        final String rangeFixture = System.getProperty("audiveris.rustport.headRangeFixture");
        if (seedFixture == null || rangeFixture == null) {
            throw new IllegalStateException("missing bounded head baseline fixture properties");
        }
        final List<Glyph> liveGlyphs = glyphs(sheet);
        final List<Glyph> ordered = new ArrayList<>();
        appendHeadFixtureGlyphs(
                Paths.get(seedFixture), "headseedhead", page, liveGlyphs, ordered);
        appendHeadFixtureGlyphs(
                Paths.get(rangeFixture), "headrangehead", page, liveGlyphs, ordered);
        return ordered;
    }

    private static void appendHeadFixtureGlyphs (Path fixture,
                                                 String family,
                                                 String page,
                                                 List<Glyph> liveGlyphs,
                                                 List<Glyph> ordered)
        throws IOException
    {
        final String prefix = family + " " + page + " ";
        try (BufferedReader reader = Files.newBufferedReader(fixture, StandardCharsets.UTF_8)) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (!line.startsWith(prefix)) continue;
                final String[] fields = line.split(" ");
                String bounds = null;
                int weight = -1;
                String runs = null;
                for (int index = 2; index + 1 < fields.length; index += 2) {
                    if (fields[index].equals("glyphBounds")) bounds = fields[index + 1];
                    if (fields[index].equals("glyphWeight")) {
                        weight = Integer.parseInt(fields[index + 1]);
                    }
                    if (fields[index].equals("glyphRuns")) runs = fields[index + 1];
                }
                Glyph match = null;
                for (Glyph glyph : liveGlyphs) {
                    if (rectangle(glyph.getBounds()).equals(bounds)
                            && glyph.getWeight() == weight
                            && String.format("%016x", runTableHash(glyph)).equals(runs)) {
                        if (match != null && match != glyph) {
                            throw new IllegalStateException(
                                    "ambiguous bounded head structural key: " + family + " "
                                            + page + " " + bounds + "/" + runs);
                        }
                        match = glyph;
                    }
                }
                if (match == null) {
                    throw new IllegalStateException(
                            "bounded head baseline glyph absent: " + family + " " + page
                                    + " key " + bounds + "/" + weight + "/" + runs);
                }
                ordered.add(match);
            }
        }
    }

    private static SeedBaseline loadSeedBaseline (Path path,
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
        if (selected == null) {
            throw new IllegalArgumentException("missing seed-baseline sheet " + wanted + " in " + path);
        }
        selected.reachStep(OmrStep.HEADERS, false);
        final Sheet sheet = selected.getSheet();
        sheet.getScale().setStemScale(new StemScaler(sheet).retrieveStemWidth());

        int rawCandidates = 0;
        final List<Glyph> checkedGlyphs = new ArrayList<>();
        final Set<String> checkedKeys = new TreeSet<>();
        for (SystemInfo system : sheet.getSystems()) {
            final VerticalsBuilder builder = new VerticalsBuilder(system);
            final List<StraightFilament> candidates =
                    (List<StraightFilament>) VERTICALS_RETRIEVE_CANDIDATES.invoke(builder);
            rawCandidates += candidates.size();
            for (StraightFilament candidate : candidates) {
                final Point2D center = candidate.getCenter2D();
                final Staff staff = system.getClosestStaff(center);
                if (staff == null || staff.isTablature() || center.getX() < staff.getHeaderStop()) {
                    continue;
                }
                final Glyph glyph = candidate.toGlyph(null);
                checkedGlyphs.add(glyph);
                checkedKeys.add(glyphAlias(glyph));
            }
        }
        return new SeedBaseline(rawCandidates, checkedGlyphs, checkedKeys.size());
    }

    private static int registerAllBs (List<Inter> beams,
                                      IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                      IdentityHashMap<Object, String> bAliases)
        throws Exception
    {
        int count = 0;
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> values = (List<Object>) BEAM_ALL_B.get(beam.getLinker());
            for (int ordinal = 0; ordinal < values.size(); ordinal++) {
                final Object b = values.get(ordinal);
                if (B_ID.getInt(b) - 1 != ordinal) {
                    throw new IllegalStateException("B id/insertion order mismatch");
                }
                final String alias = "beam:" + beamSigOrdinals.get(beam) + ":b:" + ordinal;
                final String prior = bAliases.putIfAbsent(b, alias);
                if (prior != null && !prior.equals(alias)) {
                    throw new IllegalStateException("B alias changed");
                }
                count++;
            }
        }
        return count;
    }

    private static int countVBuilders (List<Inter> beams)
        throws Exception
    {
        int count = 0;
        for (Inter inter : beams) {
            for (Object b : (List<Object>) BEAM_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker())) {
                if (!B_IS_ANCHOR.getBoolean(b)) {
                    count += ((Map<?, ?>) B_V_LINKERS.get(b)).size();
                }
            }
        }
        return count;
    }

    private static IdentityHashMap<Object, String> linkStates (
            List<Inter> beams,
            IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final IdentityHashMap<Object, String> states = new IdentityHashMap<>();
        for (Inter inter : beams) {
            for (Object b : (List<Object>) BEAM_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker())) {
                states.put(b, B_LINKED.getBoolean(b) + ":" + B_CLOSED.getBoolean(b));
                for (Object v : ((Map<?, ?>) B_V_LINKERS.get(b)).values()) {
                    states.put(v, V_IS_LINKED.invoke(v) + ":" + V_IS_CLOSED.invoke(v));
                }
            }
        }
        for (Object c : cAliases.keySet()) {
            states.put(c, C_IS_LINKED.invoke(c) + ":" + C_IS_CLOSED.invoke(c));
        }
        return states;
    }

    private static void assertLinkStates (IdentityHashMap<Object, String> before,
                                          List<Inter> beams,
                                          IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final IdentityHashMap<Object, String> after = linkStates(beams, cAliases);
        for (Map.Entry<Object, String> entry : before.entrySet()) {
            if (!entry.getValue().equals(after.get(entry.getKey()))) {
                throw new IllegalStateException("builder mutated linker state");
            }
        }
        for (Map.Entry<Object, String> entry : after.entrySet()) {
            if (!before.containsKey(entry.getKey()) && !entry.getValue().equals("false:false")) {
                throw new IllegalStateException("new anchor has mutated linker state");
            }
        }
    }

    private static IdentityHashMap<Object, Object> builderStates (
            List<Inter> beams,
            IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final IdentityHashMap<Object, Object> states = new IdentityHashMap<>();
        for (Inter inter : beams) {
            for (Object b : (List<Object>) BEAM_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker())) {
                for (Object v : ((Map<?, ?>) B_V_LINKERS.get(b)).values()) {
                    states.put(v, V_STEM_BUILDER.get(v));
                }
            }
        }
        for (Object c : cAliases.keySet()) states.put(c, C_STEM_BUILDER.get(c));
        return states;
    }

    private static void assertOnlyBuilderAssigned (IdentityHashMap<Object, Object> before,
                                                   List<Inter> beams,
                                                   IdentityHashMap<Object, String> cAliases,
                                                   Object current,
                                                   StemBuilder builder)
        throws Exception
    {
        final IdentityHashMap<Object, Object> after = builderStates(beams, cAliases);
        for (Map.Entry<Object, Object> entry : before.entrySet()) {
            final Object expected = entry.getKey() == current ? builder : entry.getValue();
            if (after.get(entry.getKey()) != expected) {
                throw new IllegalStateException("unexpected builder assignment");
            }
        }
        for (Map.Entry<Object, Object> entry : after.entrySet()) {
            if (!before.containsKey(entry.getKey()) && entry.getValue() != null) {
                throw new IllegalStateException("new anchor unexpectedly has builder");
            }
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
        if (selected == null) {
            throw new IllegalArgumentException("missing sheet " + wanted + " in " + path);
        }
        selected.reachStep(OmrStep.HEADS, false);
        return selected.getSheet();
    }

    private static TrackingGlyphIndex installTrackingGlyphIndex (Sheet sheet,
                                                                 Collection<Glyph> baselineGlyphs)
        throws IllegalAccessException
    {
        final TrackingGlyphIndex tracking = new TrackingGlyphIndex();
        tracking.initTransients(sheet);
        tracking.seedBaseline(baselineGlyphs);
        SHEET_GLYPH_INDEX.set(sheet, tracking);
        return tracking;
    }

    private static Field field (Class<?> owner,
                                String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static List<Glyph> glyphs (Sheet sheet)
    {
        final List<Glyph> values = new ArrayList<>();
        final Iterator<Glyph> iterator = sheet.getGlyphIndex().iterator();
        while (iterator.hasNext()) values.add(iterator.next());
        return values;
    }

    private static List<StraightFilament> filaments (Sheet sheet)
    {
        final List<StraightFilament> values = new ArrayList<>();
        final FilamentIndex index = sheet.getFilamentIndex();
        for (var filament : index.getEntities()) {
            if (filament instanceof StraightFilament straight) values.add(straight);
        }
        return values;
    }

    private static Glyph equalGlyph (List<Glyph> values,
                                     Glyph candidate)
    {
        for (Glyph value : values) {
            if (value.equals(candidate)) return value;
        }
        return null;
    }

    private static String sectionAlias (Section member,
                                        SystemInfo system)
        throws Exception
    {
        final Section source = member instanceof LinkedSection
                ? (Section) LINKED_SECTION_SOURCE.get(member) : member;
        final List<Section> sections = new ArrayList<>(member.getOrientation().isVertical()
                ? system.getVerticalSections() : system.getHorizontalSections());
        for (int ordinal = 0; ordinal < sections.size(); ordinal++) {
            if (sections.get(ordinal) == source) {
                return (member.getOrientation().isVertical() ? "v:" : "h:") + ordinal;
            }
        }
        throw new IllegalStateException("filament member outside SystemInfo inputs");
    }

    private static String selectedOrdinals (List<Section> selected,
                                            List<Section> source)
    {
        final StringBuilder value = new StringBuilder();
        for (Section section : selected) {
            int found = -1;
            for (int ordinal = 0; ordinal < source.size(); ordinal++) {
                if (source.get(ordinal) == section) {
                    found = ordinal;
                    break;
                }
            }
            if (found < 0) throw new IllegalStateException("accepted section missing from source");
            append(value, Integer.toString(found));
        }
        return empty(value);
    }

    private static String linkerAlias (Object linker,
                                       IdentityHashMap<Object, String> bAliases,
                                       IdentityHashMap<Object, String> cAliases)
    {
        final String b = bAliases.get(linker);
        final String c = cAliases.get(linker);
        if (b == null && c == null) throw new IllegalStateException("unaliased linker item");
        return b != null ? b : c;
    }

    private static String cAlias (int headOrdinal,
                                  HorizontalSide hSide,
                                  VerticalSide vSide)
    {
        return "head:" + headOrdinal + ":" + hSide + ":" + vSide;
    }

    private static <T> IdentityHashMap<T, Integer> identityOrdinals (List<? extends T> values)
    {
        final IdentityHashMap<T, Integer> result = new IdentityHashMap<>();
        for (int ordinal = 0; ordinal < values.size(); ordinal++) {
            if (result.put(values.get(ordinal), ordinal) != null) {
                throw new IllegalStateException("duplicate input identity");
            }
        }
        return result;
    }

    private static <T> Set<T> identitySet (Collection<T> values)
    {
        final Set<T> result = Collections.newSetFromMap(new IdentityHashMap<>());
        result.addAll(values);
        return result;
    }

    private static boolean containsIdentity (Collection<?> values,
                                             Object target)
    {
        for (Object value : values) if (value == target) return true;
        return false;
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

    private static String interTokens (List<? extends Inter> values,
                                      IdentityHashMap<Inter, Integer> ordinals)
    {
        final StringBuilder result = new StringBuilder();
        for (Inter value : values) append(result, Integer.toString(ordinals.get(value)));
        return empty(result);
    }

    private static String ordinalRange (int size)
    {
        final StringBuilder result = new StringBuilder();
        for (int ordinal = 0; ordinal < size; ordinal++) {
            append(result, Integer.toString(ordinal));
        }
        return empty(result);
    }

    private static String glyphAliases (Collection<Glyph> values)
    {
        final StringBuilder result = new StringBuilder();
        for (Glyph glyph : values) append(result, glyphAlias(glyph));
        return empty(result);
    }

    private static String glyphAlias (Glyph glyph)
    {
        if (glyph == null) return "-";
        return "g:" + rectangle(glyph.getBounds()) + ":" + glyphRunSha(glyph);
    }

    private static String glyphRunSha (Glyph glyph)
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
        return hex(digest.digest());
    }

    private static int glyphRunCount (Glyph glyph)
    {
        int count = 0;
        final var table = glyph.getRunTable();
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
            while (iterator.hasNext()) {
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
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                final var run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            hash.add(row.toString());
        }
        return hash.value();
    }

    private static String reasonToken (TreeMap<String, Integer> values)
    {
        final StringBuilder result = new StringBuilder();
        for (Map.Entry<String, Integer> entry : values.entrySet()) {
            append(result, entry.getKey() + ":" + entry.getValue());
        }
        return empty(result);
    }

    private static String rectangle (Rectangle value)
    {
        return value.x + ":" + value.y + ":" + value.width + ":" + value.height;
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

    private static void append (StringBuilder builder,
                                String value)
    {
        if (builder.length() > 0) builder.append(',');
        builder.append(value);
    }

    private static String empty (StringBuilder builder)
    {
        return builder.length() == 0 ? "-" : builder.toString();
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
                                String value)
    {
        digest.update(value.getBytes(StandardCharsets.UTF_8));
    }

    private static String hex (byte[] values)
    {
        final StringBuilder result = new StringBuilder();
        for (byte value : values) result.append(String.format("%02x", value & 0xff));
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
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) head StemBuilder oracle.");
        System.out.println("# stemsheadbuilder schema 1");
        System.out.println("# A bounded structural registry and real Java registration actions are paired.");
        System.out.println("# Stumps and beam builders establish the prefix, then every real private");
        System.out.println("# CLinker.inspect(profile) runs exactly once in head-x / TR,BL,TL,BR order.");
        System.out.println("# Dense rejects are hashed; accepted sections, registrations, items, lengths,");
        System.out.println("# allowed anchors, and forbidden mutation deltas are explicit.");
    }

    private static final class TrackingGlyphIndex
            extends GlyphIndex
    {
        private final List<ActualRegistrationAttempt> attempts = new ArrayList<>();

        void seedBaseline (Collection<Glyph> glyphs)
        {
            for (Glyph source : glyphs) {
                final Rectangle bounds = source.getBounds();
                final Glyph clone = new Glyph(bounds.x, bounds.y, source.getRunTable());
                for (GlyphGroup group : source.getGroups()) clone.addGroup(group);
                clone.setVip(source.isVip());
                super.registerOriginal(clone);
            }
        }

        @Override
        public synchronized Glyph registerOriginal (Glyph candidate)
        {
            final Glyph canonical = super.registerOriginal(candidate);
            attempts.add(new ActualRegistrationAttempt(candidate, canonical, canonical != candidate));
            return canonical;
        }

        int mark ()
        {
            return attempts.size();
        }

        List<ActualRegistrationAttempt> since (int mark)
        {
            if (mark < 0 || mark > attempts.size()) {
                throw new IllegalArgumentException("invalid registration mark");
            }
            return new ArrayList<>(attempts.subList(mark, attempts.size()));
        }
    }

    private static final class ActualRegistrationAttempt
    {
        final Glyph candidate;
        final Glyph canonical;
        final boolean reuse;

        ActualRegistrationAttempt (Glyph candidate,
                                   Glyph canonical,
                                   boolean reuse)
        {
            this.candidate = candidate;
            this.canonical = canonical;
            this.reuse = reuse;
        }
    }

    private static final class StumpPlans
    {
        private final Map<String, List<StumpPlan>> bySource = new TreeMap<>();

        static StumpPlans load (String page)
            throws IOException
        {
            final String beamFixture = System.getProperty(
                    "audiveris.rustport.beamStumpFixture");
            final String headFixture = System.getProperty(
                    "audiveris.rustport.headStumpFixture");
            if (beamFixture == null || headFixture == null) {
                throw new IllegalStateException("missing bounded stump fixture properties");
            }
            final StumpPlans plans = new StumpPlans();
            plans.loadBeam(Paths.get(beamFixture), page);
            plans.loadHead(Paths.get(headFixture), page);
            return plans;
        }

        private void loadBeam (Path fixture,
                               String page)
            throws IOException
        {
            final String prefix = "stemsbeamstumpbuild " + page + " ";
            try (BufferedReader reader = Files.newBufferedReader(fixture, StandardCharsets.UTF_8)) {
                String line;
                while ((line = reader.readLine()) != null) {
                    if (!line.startsWith(prefix)) continue;
                    final Map<String, String> fields = rowFields(line);
                    if (fields.get("registration").equals("none")) continue;
                    final int system = Integer.parseInt(fields.get("system"));
                    final int beam = Integer.parseInt(fields.get("beam"));
                    final String key = "B:" + system + ":" + beam;
                    bySource.computeIfAbsent(key, ignored -> new ArrayList<>()).add(
                            new StumpPlan(
                                    "Beam",
                                    fields.get("side"),
                                    normalizeBeamCandidate(fields.get("candidate")),
                                    true));
                }
            }
        }

        private void loadHead (Path fixture,
                               String page)
            throws IOException
        {
            final String prefix = "stemsheadstumpbuild " + page + " ";
            try (BufferedReader reader = Files.newBufferedReader(fixture, StandardCharsets.UTF_8)) {
                String line;
                while ((line = reader.readLine()) != null) {
                    if (!line.startsWith(prefix)) continue;
                    final Map<String, String> fields = rowFields(line);
                    if (fields.get("registration").equals("none")) continue;
                    final int system = Integer.parseInt(fields.get("system"));
                    final int head = Integer.parseInt(fields.get("head"));
                    final String key = "H:" + system + ":" + head;
                    bySource.computeIfAbsent(key, ignored -> new ArrayList<>()).add(
                            new StumpPlan(
                                    "Head",
                                    fields.get("ctorOrdinal"),
                                    normalizeHeadCandidate(fields.get("candidate")),
                                    fields.get("returned").equals("built")));
                }
            }
        }

        List<StumpEvent> captureBeam (int system,
                                      int beamXOrdinal,
                                      int beamSigOrdinal,
                                      List<ActualRegistrationAttempt> attempts)
        {
            return capture(
                    "B:" + system + ":" + beamXOrdinal,
                    "beam:" + beamXOrdinal + ":sig:" + beamSigOrdinal,
                    attempts);
        }

        List<StumpEvent> captureHead (int system,
                                      int headXOrdinal,
                                      int headSigOrdinal,
                                      List<ActualRegistrationAttempt> attempts)
        {
            return capture(
                    "H:" + system + ":" + headXOrdinal,
                    "head:" + headXOrdinal + ":sig:" + headSigOrdinal,
                    attempts);
        }

        private List<StumpEvent> capture (String key,
                                          String sourcePrefix,
                                          List<ActualRegistrationAttempt> attempts)
        {
            final List<StumpPlan> expected = bySource.remove(key);
            final List<StumpPlan> plans = expected != null ? expected : Collections.emptyList();
            if (plans.size() != attempts.size()) {
                throw new IllegalStateException(
                        "stump registration count mismatch at " + key + ": " + plans.size()
                                + "/" + attempts.size());
            }
            final List<StumpEvent> result = new ArrayList<>();
            for (int ordinal = 0; ordinal < plans.size(); ordinal++) {
                final StumpPlan plan = plans.get(ordinal);
                final ActualRegistrationAttempt attempt = attempts.get(ordinal);
                if (!plan.fingerprint.equals(stumpFingerprint(attempt.candidate))
                        || !attempt.candidate.equals(attempt.canonical)) {
                    throw new IllegalStateException(
                            "stump structural mismatch at " + key + ":" + ordinal
                                    + " expected " + plan.fingerprint + " actual "
                                    + stumpFingerprint(attempt.candidate));
                }
                result.add(new StumpEvent(
                        plan.kind,
                        sourcePrefix + ":" + (plan.kind.equals("Beam") ? "side:" : "ctor:")
                                + plan.detail,
                        plan.attached,
                        attempt));
            }
            return result;
        }

        void assertConsumed ()
        {
            if (!bySource.isEmpty()) {
                throw new IllegalStateException("unconsumed stump fixture sources " + bySource.keySet());
            }
        }

        private static String normalizeBeamCandidate (String token)
        {
            if (token == null || token.equals("none")) {
                throw new IllegalStateException("registered beam stump has no candidate");
            }
            return token;
        }

        private static String normalizeHeadCandidate (String token)
        {
            if (token == null || token.equals("none")) {
                throw new IllegalStateException("registered head stump has no candidate");
            }
            final String[] members = token.split(",");
            if (members.length != 3
                    || !members[0].startsWith("bounds:")
                    || !members[1].startsWith("weight:")
                    || !members[2].startsWith("runs:")) {
                throw new IllegalStateException("invalid head stump candidate token " + token);
            }
            return members[0].substring("bounds:".length()) + ":"
                    + members[1].substring("weight:".length()) + ":"
                    + members[2].substring("runs:".length());
        }

        private static Map<String, String> rowFields (String line)
        {
            final String[] tokens = line.split(" ");
            final Map<String, String> fields = new TreeMap<>();
            for (int index = 2; index + 1 < tokens.length; index += 2) {
                fields.put(tokens[index], tokens[index + 1]);
            }
            return fields;
        }
    }

    private static String stumpFingerprint (Glyph glyph)
    {
        return rectangle(glyph.getBounds()) + ":" + glyph.getWeight() + ":"
                + glyphRunCount(glyph) + ":" + String.format("%016x", runTableHash(glyph));
    }

    private static final class StumpPlan
    {
        final String kind;
        final String detail;
        final String fingerprint;
        final boolean attached;

        StumpPlan (String kind,
                   String detail,
                   String fingerprint,
                   boolean attached)
        {
            this.kind = kind;
            this.detail = detail;
            this.fingerprint = fingerprint;
            this.attached = attached;
        }
    }

    private static final class StumpEvent
    {
        final String kind;
        final String source;
        final boolean attached;
        final ActualRegistrationAttempt attempt;

        StumpEvent (String kind,
                    String source,
                    boolean attached,
                    ActualRegistrationAttempt attempt)
        {
            this.kind = kind;
            this.source = source;
            this.attached = attached;
            this.attempt = attempt;
        }
    }

    private static final class OriginRegistry
    {
        private final Map<String, TreeSet<String>> byKey = new TreeMap<>();
        private final Map<String, Glyph> baselineGlyphsByKey = new TreeMap<>();
        private final Map<String, String> aliases = new TreeMap<>();
        private MessageDigest history = sha256();
        private int nextRegistrationAlias;
        private boolean sealed;

        void addBaseline (Glyph glyph,
                          String category)
        {
            if (glyph == null) return;
            if (sealed) throw new IllegalStateException("baseline already sealed");
            final String key = glyphAlias(glyph);
            byKey.computeIfAbsent(key, ignored -> new TreeSet<>()).add(category);
            baselineGlyphsByKey.putIfAbsent(key, glyph);
        }

        void label (Glyph glyph,
                    String category)
        {
            if (glyph == null) return;
            final TreeSet<String> categories = byKey.get(glyphAlias(glyph));
            if (categories == null) {
                throw new IllegalStateException(
                        "provenance cannot invent a registration for " + glyphAlias(glyph));
            }
            categories.add(category);
        }

        OriginRegistration register (Glyph glyph,
                                     String category,
                                     String source)
        {
            if (!sealed) throw new IllegalStateException("unsealed modeled registry");
            final String key = glyphAlias(glyph);
            final TreeSet<String> categories = byKey.get(key);
            final boolean reuse = categories != null;
            final String prior = categories == null || categories.isEmpty()
                    ? "-" : String.join(",", categories);
            if (!reuse) {
                byKey.put(key, new TreeSet<>());
                aliases.put(key, "registered:" + nextRegistrationAlias++);
            }
            byKey.get(key).add(category);
            update(history, source + ":" + (reuse ? "Reuse" : "New") + ":" + key + "\n");
            return new OriginRegistration(reuse, prior, aliases.get(key));
        }

        String categories (Glyph glyph)
        {
            final Set<String> values = byKey.get(glyphAlias(glyph));
            return values == null || values.isEmpty() ? "-" : String.join(",", values);
        }

        String canonicalAlias (Glyph glyph)
        {
            final String alias = aliases.get(glyphAlias(glyph));
            if (alias == null) throw new IllegalStateException("unmodeled canonical alias");
            return alias;
        }

        int count ()
        {
            return byKey.size();
        }

        List<Glyph> baselineGlyphs ()
        {
            if (!sealed) throw new IllegalStateException("unsealed modeled registry");
            return new ArrayList<>(baselineGlyphsByKey.values());
        }

        void sealBaseline ()
        {
            if (sealed) throw new IllegalStateException("baseline sealed twice");
            history = sha256();
            aliases.clear();
            int ordinal = 0;
            for (String key : byKey.keySet()) {
                aliases.put(key, "baseline:" + ordinal++);
                update(history, key + "\n");
            }
            sealed = true;
        }

        String historySha ()
        {
            try {
                return hex(((MessageDigest) history.clone()).digest());
            } catch (CloneNotSupportedException ex) {
                throw new IllegalStateException("SHA-256 digest is not cloneable", ex);
            }
        }
    }

    private static final class SeedBaseline
    {
        final int rawCandidates;
        final List<Glyph> checkedGlyphs;
        final int checkedStructuralKeys;

        SeedBaseline (int rawCandidates,
                      List<Glyph> checkedGlyphs,
                      int checkedStructuralKeys)
        {
            this.rawCandidates = rawCandidates;
            this.checkedGlyphs = checkedGlyphs;
            this.checkedStructuralKeys = checkedStructuralKeys;
        }
    }

    private static final class OriginRegistration
    {
        final boolean reuse;
        final String priorCategories;
        final String canonicalAlias;

        OriginRegistration (boolean reuse,
                            String priorCategories,
                            String canonicalAlias)
        {
            this.reuse = reuse;
            this.priorCategories = priorCategories;
            this.canonicalAlias = canonicalAlias;
        }
    }

    private static final class Registration
    {
        final StraightFilament filament;
        final Glyph canonical;
        final boolean modeledReuse;
        final String origins;
        final String canonicalAlias;

        Registration (StraightFilament filament,
                      Glyph canonical,
                      boolean modeledReuse,
                      String origins,
                      String canonicalAlias)
        {
            this.filament = filament;
            this.canonical = canonical;
            this.modeledReuse = modeledReuse;
            this.origins = origins;
            this.canonicalAlias = canonicalAlias;
        }
    }

    private static final class RegistrationBatch
    {
        final List<Registration> events;
        final int news;
        final int reuses;
        final int modeledBefore;
        final int modeledAfter;
        final String registryShaBefore;
        final String registryShaAfter;
        final String actionSha;

        RegistrationBatch (List<Registration> events,
                           int news,
                           int reuses,
                           int modeledBefore,
                           int modeledAfter,
                           String registryShaBefore,
                           String registryShaAfter,
                           String actionSha)
        {
            this.events = events;
            this.news = news;
            this.reuses = reuses;
            this.modeledBefore = modeledBefore;
            this.modeledAfter = modeledAfter;
            this.registryShaBefore = registryShaBefore;
            this.registryShaAfter = registryShaAfter;
            this.actionSha = actionSha;
        }
    }

    private static final class PredictedB
    {
        final AbstractBeamInter beam;
        final int beforeCount;
        final Object best;
        final double bestDx;
        final Point2D cross;
        final boolean reuse;
        final String alias;

        PredictedB (AbstractBeamInter beam,
                    int beforeCount,
                    Object best,
                    double bestDx,
                    Point2D cross,
                    boolean reuse,
                    String alias)
        {
            this.beam = beam;
            this.beforeCount = beforeCount;
            this.best = best;
            this.bestDx = bestDx;
            this.cross = cross;
            this.reuse = reuse;
            this.alias = alias;
        }
    }

    private static final class SeedOccurrence
    {
        final Glyph glyph;
        final int sourceOrdinal;
        final int systemSeedOrdinal;
        final int contrib;
        final double distance;

        SeedOccurrence (Glyph glyph,
                        int sourceOrdinal,
                        int systemSeedOrdinal,
                        int contrib,
                        double distance)
        {
            this.glyph = glyph;
            this.sourceOrdinal = sourceOrdinal;
            this.systemSeedOrdinal = systemSeedOrdinal;
            this.contrib = contrib;
            this.distance = distance;
        }
    }

    private static final class ChunkOccurrence
    {
        final String alias;
        final Glyph glyph;
        final int inputOrdinal;
        String action = "keep";
        int finalOrdinal = -1;

        ChunkOccurrence (String alias,
                         Glyph glyph,
                         int inputOrdinal)
        {
            this.alias = alias;
            this.glyph = glyph;
            this.inputOrdinal = inputOrdinal;
        }
    }

    private static final class GlyphOccurrence
    {
        final String alias;
        final Glyph glyph;

        GlyphOccurrence (String alias,
                         Glyph glyph)
        {
            this.alias = alias;
            this.glyph = glyph;
        }
    }

    private static final class ReplayItem
    {
        final String kind;
        final String alias;
        final StemLinker linker;
        final Glyph glyph;
        final Line2D line;
        final int contrib;
        final int creationOrdinal;

        ReplayItem (String kind,
                    String alias,
                    StemLinker linker,
                    Glyph glyph,
                    Line2D line,
                    int contrib,
                    int creationOrdinal)
        {
            this.kind = kind;
            this.alias = alias;
            this.linker = linker;
            this.glyph = glyph;
            this.line = line;
            this.contrib = contrib;
            this.creationOrdinal = creationOrdinal;
        }
    }

    private static final class FinalReplayItem
    {
        final String kind;
        final String alias;
        final StemLinker linker;
        final Glyph glyph;
        final Line2D line;
        final int contrib;

        FinalReplayItem (String kind,
                         String alias,
                         StemLinker linker,
                         Glyph glyph,
                         Line2D line,
                         int contrib)
        {
            this.kind = kind;
            this.alias = alias;
            this.linker = linker;
            this.glyph = glyph;
            this.line = line;
            this.contrib = contrib;
        }
    }

    private static final class SortAnomalies
    {
        final long cycles;
        final long equivalence;
        final String digest;

        SortAnomalies (long cycles,
                       long equivalence,
                       String digest)
        {
            this.cycles = cycles;
            this.equivalence = equivalence;
            this.digest = digest;
        }
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            for (byte octet : (row + "\n").getBytes(StandardCharsets.UTF_8)) {
                value ^= octet & 0xffL;
                value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }

    private static final class Totals
    {
        long stumpRegistrations;
        long stumpGlyphNews;
        long stumpGlyphReuses;
        long stumpActionDiffs;
        long beamBuilders;
        long beamFilaments;
        long beamGlyphNews;
        long beamGlyphReuses;
        long builders;
        long topBuilders;
        long bottomBuilders;
        long directionDivergences;
        long profileDivergences;
        long stumplessStarts;
        long anchorsCreated;
        long vScans;
        long vAccepts;
        long hScans;
        long hAccepts;
        long filaments;
        long filamentMembers;
        long glyphNews;
        long glyphReuses;
        long lowRemainChunks;
        long headPartDrops;
        long items;
        long gaps;
        long lengths;
        long sortCycles;
        long sortEquivalence;
        long inputTargets;
        long removedTargets;
        long inputSeeds;
        long alignComparisons;
        long alignRemovals;
        long chunkDrops;
        long seedItemDrops;
        long preItems;
        long sortRows;
        long maxTargetSortItems;
        long maxFinalSortItems;
        long targetSortAtLeast32;
        long finalSortAtLeast32;
        long gapChecks;
        long gapInserts;
        long gapTruncations;
        long seedSourceScans;
        long retrieveSeedSortRows;
        long maxRetrieveSeedSortItems;
        long retrieveSeedSortAtLeast32;
        long vipHeads;
        long vipBuilders;
        long smallHeads;
        long removeVipOnly;
        long keepNonVipBug;
        long beamActionDiffs;
        long headToLaterBeamReuses;
        long seedDuplicateKeys;
        long seedDuplicateExtraOccurrences;
        long chunkDuplicateKeys;
        long chunkDuplicateExtraOccurrences;
        long sortAudits;

        void include (Totals other)
        {
            stumpRegistrations += other.stumpRegistrations;
            stumpGlyphNews += other.stumpGlyphNews;
            stumpGlyphReuses += other.stumpGlyphReuses;
            stumpActionDiffs += other.stumpActionDiffs;
            beamBuilders += other.beamBuilders;
            beamFilaments += other.beamFilaments;
            beamGlyphNews += other.beamGlyphNews;
            beamGlyphReuses += other.beamGlyphReuses;
            builders += other.builders;
            topBuilders += other.topBuilders;
            bottomBuilders += other.bottomBuilders;
            directionDivergences += other.directionDivergences;
            profileDivergences += other.profileDivergences;
            stumplessStarts += other.stumplessStarts;
            anchorsCreated += other.anchorsCreated;
            vScans += other.vScans;
            vAccepts += other.vAccepts;
            hScans += other.hScans;
            hAccepts += other.hAccepts;
            filaments += other.filaments;
            filamentMembers += other.filamentMembers;
            glyphNews += other.glyphNews;
            glyphReuses += other.glyphReuses;
            lowRemainChunks += other.lowRemainChunks;
            headPartDrops += other.headPartDrops;
            items += other.items;
            gaps += other.gaps;
            lengths += other.lengths;
            sortCycles += other.sortCycles;
            sortEquivalence += other.sortEquivalence;
            inputTargets += other.inputTargets;
            removedTargets += other.removedTargets;
            inputSeeds += other.inputSeeds;
            alignComparisons += other.alignComparisons;
            alignRemovals += other.alignRemovals;
            chunkDrops += other.chunkDrops;
            seedItemDrops += other.seedItemDrops;
            preItems += other.preItems;
            sortRows += other.sortRows;
            maxTargetSortItems = Math.max(maxTargetSortItems, other.maxTargetSortItems);
            maxFinalSortItems = Math.max(maxFinalSortItems, other.maxFinalSortItems);
            targetSortAtLeast32 += other.targetSortAtLeast32;
            finalSortAtLeast32 += other.finalSortAtLeast32;
            gapChecks += other.gapChecks;
            gapInserts += other.gapInserts;
            gapTruncations += other.gapTruncations;
            seedSourceScans += other.seedSourceScans;
            retrieveSeedSortRows += other.retrieveSeedSortRows;
            maxRetrieveSeedSortItems = Math.max(
                    maxRetrieveSeedSortItems, other.maxRetrieveSeedSortItems);
            retrieveSeedSortAtLeast32 += other.retrieveSeedSortAtLeast32;
            vipHeads += other.vipHeads;
            vipBuilders += other.vipBuilders;
            smallHeads += other.smallHeads;
            removeVipOnly += other.removeVipOnly;
            keepNonVipBug += other.keepNonVipBug;
            beamActionDiffs += other.beamActionDiffs;
            headToLaterBeamReuses += other.headToLaterBeamReuses;
            seedDuplicateKeys += other.seedDuplicateKeys;
            seedDuplicateExtraOccurrences += other.seedDuplicateExtraOccurrences;
            chunkDuplicateKeys += other.chunkDuplicateKeys;
            chunkDuplicateExtraOccurrences += other.chunkDuplicateExtraOccurrences;
            sortAudits += other.sortAudits;
        }

        String fields ()
        {
            return String.format(
                    "stumpRegistrations %d stumpGlyphNew %d stumpGlyphReuse %d "
                            + "stumpActionDiffs %d "
                            + "beamBuilders %d beamFilaments %d beamGlyphNew %d beamGlyphReuse %d "
                            + "builders %d topBuilders %d bottomBuilders %d "
                            + "directionDivergences %d profileDivergences %d "
                            + "stumplessStarts %d anchorsCreated %d "
                            + "vScans %d vAccepts %d hScans %d hAccepts %d "
                            + "filaments %d filamentMembers %d glyphNew %d glyphReuse %d "
                            + "lowRemainChunks %d headPartDrops %d items %d gaps %d "
                            + "lengths %d sortCycles %d sortEquivalence %d "
                            + "inputTargets %d removedTargets %d "
                            + "inputSeeds %d alignComparisons %d alignRemovals %d "
                            + "chunkDrops %d seedItemDrops %d preItems %d sortRows %d "
                            + "maxTargetSortItems %d maxFinalSortItems %d "
                            + "targetSortListsAtLeast32 %d finalSortListsAtLeast32 %d "
                            + "gapChecks %d gapInserts %d gapTruncations %d "
                            + "seedSourceScans %d retrieveSeedSortRows %d "
                            + "maxRetrieveSeedSortItems %d retrieveSeedSortListsAtLeast32 %d "
                            + "vipHeads %d vipBuilders %d smallHeads %d "
                            + "removeVipOnly %d keepNonVipBug %d beamActionDiffs %d "
                            + "headToLaterBeamReuses %d "
                            + "seedDuplicateKeys %d seedDuplicateExtraOccurrences %d "
                            + "chunkDuplicateKeys %d chunkDuplicateExtraOccurrences %d "
                            + "sortAudits %d "
                            + "sigMutations 0 systemStemMutations 0 "
                            + "linkMutations 0 unexpectedBuilderMutations 0",
                    stumpRegistrations,
                    stumpGlyphNews,
                    stumpGlyphReuses,
                    stumpActionDiffs,
                    beamBuilders,
                    beamFilaments,
                    beamGlyphNews,
                    beamGlyphReuses,
                    builders,
                    topBuilders,
                    bottomBuilders,
                    directionDivergences,
                    profileDivergences,
                    stumplessStarts,
                    anchorsCreated,
                    vScans,
                    vAccepts,
                    hScans,
                    hAccepts,
                    filaments,
                    filamentMembers,
                    glyphNews,
                    glyphReuses,
                    lowRemainChunks,
                    headPartDrops,
                    items,
                    gaps,
                    lengths,
                    sortCycles,
                    sortEquivalence,
                    inputTargets,
                    removedTargets,
                    inputSeeds,
                    alignComparisons,
                    alignRemovals,
                    chunkDrops,
                    seedItemDrops,
                    preItems,
                    sortRows,
                    maxTargetSortItems,
                    maxFinalSortItems,
                    targetSortAtLeast32,
                    finalSortAtLeast32,
                    gapChecks,
                    gapInserts,
                    gapTruncations,
                    seedSourceScans,
                    retrieveSeedSortRows,
                    maxRetrieveSeedSortItems,
                    retrieveSeedSortAtLeast32,
                    vipHeads,
                    vipBuilders,
                    smallHeads,
                    removeVipOnly,
                    keepNonVipBug,
                    beamActionDiffs,
                    headToLaterBeamReuses,
                    seedDuplicateKeys,
                    seedDuplicateExtraOccurrences,
                    chunkDuplicateKeys,
                    chunkDuplicateExtraOccurrences,
                    sortAudits);
        }
    }
}
