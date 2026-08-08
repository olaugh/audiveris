// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.Point;
import java.awt.geom.Area;
import java.awt.geom.Line2D;
import java.awt.geom.Path2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.dynamic.FilamentIndex;
import org.audiveris.omr.glyph.dynamic.LinkedSection;
import org.audiveris.omr.glyph.dynamic.StraightFilament;
import org.audiveris.omr.lag.Lag;
import org.audiveris.omr.lag.Lags;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.math.GeoOrder;
import org.audiveris.omr.math.GeoUtil;
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
import org.audiveris.omr.sheet.stem.StemBuilder;
import org.audiveris.omr.sheet.stem.StemHalfLinker;
import org.audiveris.omr.sheet.stem.StemItem;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sheet.stem.VerticalsBuilder;
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
 * Exact identity-free oracle for beam-origin {@link StemBuilder} constructor completion.
 *
 * <p>The page reaches real HEADS. This probe installs the same retriever inputs, constructs every
 * live beam linker and then every head linker in the exact stable-x order, and visits beam B/V
 * linkers in production order. For each VLinker it invokes the real private {@code inspect}
 * exactly once, including filters, complete {@code new StemBuilder(...)}, and V builder assignment.
 * It independently replays every constructor-owned decision and proves that SIG, system stems,
 * linker state, other V builders, and every C builder remain untouched.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamStemBuilderProbe
{
    private static final IdentityHashMap<Glyph, String> PAGE_REJECTED_HEAD_STUMPS =
            new IdentityHashMap<>();

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

    private static final Field PARAM_MAX_STEM_THICKNESS;

    private static final Field PARAM_MAX_LINE_SECTION_DX;

    private static final Field PARAM_MAX_STEM_ALIGNMENT_DX;

    private static final Field PARAM_MAX_STEM_ALIGNMENT_DY;

    private static final Field PARAM_MIN_LINKER_LENGTH;

    private static final Field LINKER_ALL_B;

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

    private static final Field V_STEM_BUILDER;

    private static final Method V_FILTER_BEAMS;

    private static final Method V_FILTER_HEADS;

    private static final Method V_INSPECT;

    private static final Method V_IS_LINKED;

    private static final Method V_IS_CLOSED;

    private static final Method C_IS_LINKED;

    private static final Method C_IS_CLOSED;

    private static final Field STEM_BUILDER_ITEMS;

    private static final Field STEM_BUILDER_LENGTH_MAP;

    private static final Field STEM_BUILDER_LAST_HEAD_Y;

    private static final Field STEM_BUILDER_Y_RANGE;

    private static final Field STEM_BUILDER_Y_DIR;

    private static final Field STEM_ITEM_LINE;

    private static final Field STEM_ITEM_GLYPH;

    private static final Field STEM_ITEM_CONTRIB;

    private static final Field LINKER_ITEM_LINKER;

    private static final Field LINKED_SECTION_SOURCE;

    private static final Field RETRIEVER_SYSTEM_STEMS;

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
            final Class<?> linkerItem = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemItem$LinkerItem");

            PARAMETERS_CONSTRUCTOR = parameters.getDeclaredConstructor(SystemInfo.class, Scale.class);
            PARAMETERS_CONSTRUCTOR.setAccessible(true);
            RETRIEVER_PARAMS = declaredField(StemsRetriever.class, "params");
            RETRIEVER_SYSTEM_SEEDS = declaredField(StemsRetriever.class, "systemSeeds");
            RETRIEVER_SYSTEM_BEAMS = declaredField(StemsRetriever.class, "systemBeams");
            RETRIEVER_SYSTEM_HEADS = declaredField(StemsRetriever.class, "systemHeads");
            RETRIEVER_STEM_CHECKER = declaredField(StemsRetriever.class, "stemChecker");
            RETRIEVER_SYSTEM_STEMS = declaredField(StemsRetriever.class, "systemStems");
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
            PARAM_MAX_STEM_THICKNESS = declaredField(parameters, "maxStemThickness");
            PARAM_MAX_LINE_SECTION_DX = declaredField(parameters, "maxLineSectionDx");
            PARAM_MAX_STEM_ALIGNMENT_DX = declaredField(parameters, "maxStemAlignmentDx");
            PARAM_MAX_STEM_ALIGNMENT_DY = declaredField(parameters, "maxStemAlignmentDy");
            PARAM_MIN_LINKER_LENGTH = declaredField(parameters, "minLinkerLength");

            LINKER_ALL_B = declaredField(BeamLinker.class, "allBLinkers");
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
            V_STEM_BUILDER = declaredField(vLinker, "sb");
            V_FILTER_BEAMS = vLinker.getDeclaredMethod("filterBeams", List.class);
            V_FILTER_BEAMS.setAccessible(true);
            V_FILTER_HEADS = vLinker.getDeclaredMethod("filterHeads", List.class);
            V_FILTER_HEADS.setAccessible(true);
            V_INSPECT = vLinker.getDeclaredMethod("inspect", int.class);
            V_INSPECT.setAccessible(true);
            V_IS_LINKED = vLinker.getDeclaredMethod("isLinked");
            V_IS_LINKED.setAccessible(true);
            V_IS_CLOSED = vLinker.getDeclaredMethod("isClosed");
            V_IS_CLOSED.setAccessible(true);
            C_IS_LINKED = cLinker.getDeclaredMethod("isLinked");
            C_IS_LINKED.setAccessible(true);
            C_IS_CLOSED = cLinker.getDeclaredMethod("isClosed");
            C_IS_CLOSED.setAccessible(true);

            STEM_BUILDER_ITEMS = declaredField(StemBuilder.class, "items");
            STEM_BUILDER_LENGTH_MAP = declaredField(StemBuilder.class, "lengthMap");
            STEM_BUILDER_LAST_HEAD_Y = declaredField(StemBuilder.class, "lastHeadY");
            STEM_BUILDER_Y_RANGE = declaredField(StemBuilder.class, "yRange");
            STEM_BUILDER_Y_DIR = declaredField(StemBuilder.class, "yDir");
            STEM_ITEM_LINE = declaredField(StemItem.class, "line");
            STEM_ITEM_GLYPH = declaredField(StemItem.class, "glyph");
            STEM_ITEM_CONTRIB = declaredField(StemItem.class, "contrib");
            LINKER_ITEM_LINKER = declaredField(linkerItem, "linker");
            LINKED_SECTION_SOURCE = declaredField(LinkedSection.class, "section");

            C_STEM_BUILDER = declaredField(cLinker, "sb");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamStemBuilderProbe ()
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
        final IdentityHashMap<Glyph, String> pageChunkAliases = new IdentityHashMap<>();
        PAGE_REJECTED_HEAD_STUMPS.clear();

        System.out.printf(
                "stemsbeambuilderpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, pageChunkAliases, totals, hash);
        }
        System.out.printf(
                "stemsbeambuilderpagesummary %s systems %d %s hash %016x%n",
                page, sheet.getSystems().size(), builderSummaryFields(totals), hash.value());
    }

    private static void runSystem (String page,
                                   Sheet sheet,
                                   SystemInfo system,
                                   IdentityHashMap<Glyph, String> chunkAliases,
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
        final int maxStemThickness = PARAM_MAX_STEM_THICKNESS.getInt(params);
        final double maxLineSectionDx = PARAM_MAX_LINE_SECTION_DX.getDouble(params);
        final double maxStemAlignmentDx = PARAM_MAX_STEM_ALIGNMENT_DX.getDouble(params);
        final double maxStemAlignmentDy = PARAM_MAX_STEM_ALIGNMENT_DY.getDouble(params);
        final int minLinkerLength = PARAM_MIN_LINKER_LENGTH.getInt(params);
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

        emit(String.format(
                "stemsbeambuildersystem %s system %d profile %d interline %d "
                        + "maxStemThickness %d maxLineSectionDx %s maxStemAlignmentDx %s "
                        + "maxStemAlignmentDy %s minLinkerLength %d minCoreSectionLength 0 "
                        + "minSideRatio %s gap0 %d gap1 %d gap2 %d gap3 %d gap4 %d "
                        + "saveConnections %s verticalSections %d horizontalSections %d "
                        + "verticalGlobalOrdinals %s horizontalGlobalOrdinals %s",
                page,
                system.getId(),
                system.getProfile(),
                sheet.getScale().getInterline(),
                maxStemThickness,
                hexDouble(maxLineSectionDx),
                hexDouble(maxStemAlignmentDx),
                hexDouble(maxStemAlignmentDy),
                minLinkerLength,
                hexDouble(VerticalsBuilder.getMinSideRatio().getValue()),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(0)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(1)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(2)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(3)),
                sheet.getScale().toPixels(StemChecker.getMaxYGap(4)),
                StemBuilder.saveConnections(),
                system.getVerticalSections().size(),
                system.getHorizontalSections().size(),
                globalSectionOrdinals(sheet, system.getVerticalSections(), Lags.VLAG),
                globalSectionOrdinals(sheet, system.getHorizontalSections(), Lags.HLAG)),
                hash, pageHash);
        if (StemBuilder.saveConnections()) {
            throw new IllegalStateException("StemBuilder saveConnections must remain disabled");
        }

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

        final Set<Glyph> glyphsBeforeHeads = identitySet(
                glyphIndexGlyphs(sheet.getGlyphIndex()));
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

        final Set<Glyph> attachedHeadStumps = Collections.newSetFromMap(new IdentityHashMap<>());
        for (Object c : allCLinkers) {
            final Glyph stump = ((StemLinker) c).getStump();
            if (stump != null) attachedHeadStumps.add(stump);
        }
        final List<Glyph> rejectedHeadStumps = new ArrayList<>();
        for (Glyph glyph : glyphIndexGlyphs(sheet.getGlyphIndex())) {
            if (!glyphsBeforeHeads.contains(glyph) && glyph.getGroups().contains(GlyphGroup.STUMP)
                    && !attachedHeadStumps.contains(glyph)) {
                rejectedHeadStumps.add(glyph);
            }
        }
        Collections.sort(rejectedHeadStumps, Comparator.comparing(StemsBeamStemBuilderProbe::glyphAlias));
        for (int ordinal = 0; ordinal < rejectedHeadStumps.size(); ordinal++) {
            final Glyph glyph = rejectedHeadStumps.get(ordinal);
            PAGE_REJECTED_HEAD_STUMPS.put(
                    glyph, "rejectedHeadStump:s" + system.getId() + ":" + ordinal);
        }

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

        emitGlyphInventory(
                page, sheet, system, retriever, bAliases, cAliases, chunkAliases,
                totals, hash, pageHash);

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
                            maxStemThickness,
                            maxLineSectionDx,
                            maxStemAlignmentDx,
                            maxStemAlignmentDy,
                            systemBeams,
                            systemHeads,
                            beamSigOrdinals,
                            beamXOrdinals,
                            beamGlyphAliases,
                            headXOrdinals,
                            cAliases,
                            bAliases,
                            chunkAliases,
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
            throw new IllegalStateException("system seed pool mutated during builder construction");
        }
        for (Map.Entry<Object, List<Glyph>> entry : vSeedSnapshots.entrySet()) {
            if (V_STEM_BUILDER.get(entry.getKey()) == null) {
                throw new IllegalStateException("VLinker missing StemBuilder after construction");
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
                "stemsbeambuildersystemsummary %s system %d systems 1 %s hash %016x",
                page, system.getId(), builderSummaryFields(totals), hash.value()), pageHash);
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
                                  int maxStemThickness,
                                  double maxLineSectionDx,
                                  double maxStemAlignmentDx,
                                  double maxStemAlignmentDy,
                                  List<Inter> systemBeams,
                                  List<Inter> systemHeads,
                                  IdentityHashMap<Inter, Integer> beamSigOrdinals,
                                  IdentityHashMap<AbstractBeamInter, Integer> beamXOrdinals,
                                  IdentityHashMap<Glyph, Integer> beamGlyphAliases,
                                  IdentityHashMap<HeadInter, Integer> headXOrdinals,
                                  IdentityHashMap<Object, String> cAliases,
                                  IdentityHashMap<Object, String> bAliases,
                                  IdentityHashMap<Glyph, String> chunkAliases,
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
        final int builderYDir = theoLine.getY2() > theoLine.getY1() ? 1 : -1;
        final boolean directionDiverges = builderYDir != yDir;
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
        final List<Glyph> seedBefore = new ArrayList<>(seeds);
        final GlyphIndex glyphIndex = sheet.getGlyphIndex();
        final FilamentIndex filamentIndex = sheet.getFilamentIndex();
        final List<Glyph> glyphsBefore = glyphIndexGlyphs(glyphIndex);
        final List<StraightFilament> filamentsBefore = filamentIndexFilaments(filamentIndex);
        final int sigVerticesBefore = system.getSig().vertexSet().size();
        final int sigEdgesBefore = system.getSig().edgeSet().size();
        final int systemStemsBefore = ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size();
        final IdentityHashMap<Object, String> linkStatesBefore = linkStateSnapshot(
                systemBeams, cAliases);
        final IdentityHashMap<Object, Object> builderStatesBefore = builderStateSnapshot(
                systemBeams, cAliases);

        final String predictedBTargets = predictedBTargets(
                decisions, beamSigOrdinals, bAliases);
        final String cTargets = objectTokens(headReplay.cLinkers, cAliases);
        emit(String.format(
                "stemsbeambuilder %s system %d inspectOrdinal %d beamSig %d bOrdinal %d "
                        + "vSide %s profile %d vYDir %d builderYDir %d "
                        + "directionDiverges %s theo %s luBounds %s yRange %s "
                        + "startStump %s inputSeeds %s inputBTargets %s inputCTargets %s",
                page,
                system.getId(),
                ordinal,
                beamSigOrdinals.get(beam),
                bOrdinal,
                vSide,
                maxProfile,
                yDir,
                builderYDir,
                directionDiverges,
                line(theoLine),
                rectangle(luArea.getBounds()),
                rectangle(theoLine.getBounds()),
                glyphAlias((Glyph) B_STUMP.get(b)),
                glyphAliases(seedBefore),
                predictedBTargets,
                cTargets), hashes);

        V_INSPECT.invoke(v, maxProfile);
        final int linkMutationCount = linkStateMutationCount(
                linkStatesBefore, systemBeams, cAliases);
        if (linkMutationCount != 0) {
            throw new IllegalStateException("StemBuilder path mutated linker flags");
        }
        final List<Object> actualBs = finishFindDecisions(
                page,
                system,
                beam,
                bOrdinal,
                vSide,
                decisions,
                beamSigOrdinals,
                bAliases,
                totals,
                hashes);
        final List<Object> actualCs = headReplay.cLinkers;
        final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
        if (builder == null) {
            throw new IllegalStateException("VLinker.inspect did not assign StemBuilder");
        }
        if (STEM_BUILDER_Y_DIR.getInt(builder) != builderYDir) {
            throw new IllegalStateException("StemBuilder direction differs from theoretical line");
        }
        final int unexpectedBuilderMutations = builderStateMutationCount(
                builderStatesBefore, systemBeams, cAliases, v, builder);
        if (unexpectedBuilderMutations != 0) {
            throw new IllegalStateException("unexpected builder-field mutation");
        }

        emitBuilderState(
                page,
                sheet,
                system,
                retriever,
                beam,
                b,
                v,
                builder,
                ordinal,
                maxProfile,
                builderYDir,
                directionDiverges,
                seedBefore,
                actualBs,
                actualCs,
                glyphsBefore,
                filamentsBefore,
                maxStemThickness,
                maxLineSectionDx,
                maxStemAlignmentDx,
                maxStemAlignmentDy,
                bAliases,
                cAliases,
                chunkAliases,
                sigVerticesBefore,
                sigEdgesBefore,
                systemStemsBefore,
                linkMutationCount,
                unexpectedBuilderMutations,
                totals,
                hashes);

        final String bTargets = objectTokens(actualBs, bAliases);
        final StringBuilder targets = new StringBuilder();
        for (Object target : actualBs) append(targets, bAliases.get(target));
        for (Object target : actualCs) append(targets, cAliases.get(target));
        totals.resultBs += actualBs.size();
        totals.resultCs += actualCs.size();
        emit(String.format(
                "stemsbeamvinspectresult %s system %d inspectOrdinal %d beamSig %d bOrdinal %d "
                        + "vSide %s bTargets %s cTargets %s targets %s seeds %s maxProfile %d "
                        + "builder set",
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
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<Object, String> bAliases,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
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

    private static String builderSummaryFields (Totals totals)
    {
        return String.format(
                "beams %d smallBeams %d heads %d smallHeads %d initialBs %d initialVs %d "
                        + "bVisits %d anchorSkips %d anchorsCreated %d finalBs %d "
                        + "builders %d topBuilders %d bottomBuilders %d "
                        + "directionDivergences %d profile3Builders %d "
                        + "profile4Builders %d stumplessStarts %d "
                        + "inputSeeds %d removedSeeds %d retainedSeeds %d "
                        + "inputTargets %d removedTargets %d retainedTargets %d "
                        + "inputBTargets %d removedBTargets %d retainedBTargets %d "
                        + "inputCTargets %d removedCTargets %d retainedCTargets %d "
                        + "alignComparisons %d alignRemovals %d alignTieRemoveSecond %d "
                        + "vSectionScans %d vSectionAccepts %d vSectionOutside %d "
                        + "vSectionWide %d vSectionStumpOverlap %d vSectionPastHead %d "
                        + "vSectionDistance %d hSectionScans %d hSectionAccepts %d "
                        + "hSectionOutside %d hSectionWide %d hSectionPastHead %d "
                        + "selectedSectionRows %d filaments %d filamentMembers %d "
                        + "filamentRegistrations %d externalMembers 0 "
                        + "glyphAttempts %d glyphNew %d glyphReuse %d "
                        + "reuseSeedOrigins %d reuseBeamStumpOrigins %d "
                        + "reuseHeadStumpOrigins %d reuseRejectedHeadStumpOrigins %d "
                        + "reusePriorChunkOrigins %d reuseRawBeamOrigins %d "
                        + "reuseHeadOrigins %d reuseLedgerOrigins %d reuseGridOrigins %d "
                        + "reuseSigOtherOrigins %d reuseUnmodeledOrigins %d "
                        + "chunkRemovals %d chunkSeedDrops %d chunkUnalignedDrops %d "
                        + "chunkStartDrops %d chunkHeadPartsDrops 0 seedItemDrops %d "
                        + "preItems %d preStart %d preB %d preC %d preSeed %d preChunk %d "
                        + "sortAudits %d sortRows %d sortCycles %d sortEquivalence %d "
                        + "maxTargetSortItems %d maxFinalSortItems %d sortListsAtLeast32 %d "
                        + "gapChecks %d gapInserts %d gapTruncations %d "
                        + "finalItems %d finalStart %d finalB %d finalC %d finalSeed %d "
                        + "finalChunk %d finalGap %d lengthRows %d builderChecks %d "
                        + "sigMutations 0 systemStemMutations 0 linkMutations 0 "
                        + "cBuilderMutations 0 unexpectedBuilderMutations 0",
                totals.beams, totals.smallBeams, totals.heads, totals.smallHeads,
                totals.initialBs, totals.initialVs, totals.bVisits, totals.anchorSkips,
                totals.anchorsCreated, totals.finalBs, totals.builders, totals.topBuilders,
                totals.bottomBuilders, totals.builderDirectionDivergences,
                totals.profile3Builders, totals.profile4Builders,
                totals.builderStumplessStarts, totals.builderInputSeeds,
                totals.builderRemovedSeeds, totals.builderRetainedSeeds,
                totals.builderInputTargets, totals.builderRemovedTargets,
                totals.builderRetainedTargets, totals.builderInputBTargets,
                totals.builderRemovedBTargets, totals.builderRetainedBTargets,
                totals.builderInputCTargets, totals.builderRemovedCTargets,
                totals.builderRetainedCTargets, totals.builderAlignComparisons,
                totals.builderAlignRemovals, totals.builderAlignTieRemovals,
                totals.builderVSectionScans, totals.builderVSectionAccepts,
                totals.builderVSectionOutside, totals.builderVSectionWide,
                totals.builderVSectionStumpOverlap, totals.builderVSectionPastHead,
                totals.builderVSectionDistance, totals.builderHSectionScans,
                totals.builderHSectionAccepts, totals.builderHSectionOutside,
                totals.builderHSectionWide, totals.builderHSectionPastHead,
                totals.builderSelectedSectionRows, totals.builderFilaments,
                totals.builderFilamentMembers, totals.builderFilaments,
                totals.builderGlyphAttempts, totals.builderGlyphNews, totals.builderGlyphReuses,
                totals.builderReuseSeedOrigins, totals.builderReuseBeamStumpOrigins,
                totals.builderReuseHeadStumpOrigins,
                totals.builderReuseRejectedHeadStumpOrigins,
                totals.builderReusePriorChunkOrigins, totals.builderReuseRawBeamOrigins,
                totals.builderReuseHeadOrigins, totals.builderReuseLedgerOrigins,
                totals.builderReuseGridOrigins, totals.builderReuseSigOtherOrigins,
                totals.builderReuseUnmodeledOrigins, totals.builderChunkRemovals,
                totals.builderChunkSeedDrops, totals.builderChunkUnalignedDrops,
                totals.builderChunkStartDrops, totals.builderSeedItemDrops,
                totals.builderPreItems, totals.builderPreStartItems, totals.builderPreBItems,
                totals.builderPreCItems, totals.builderPreSeedItems, totals.builderPreChunkItems,
                totals.builderSortAudits, totals.builderSortRows, totals.builderSortCycles,
                totals.builderSortEquivalenceInconsistencies,
                totals.builderMaxTargetSortItems, totals.builderMaxFinalSortItems,
                totals.builderSortListsAtLeast32, totals.builderGapChecks,
                totals.builderGapInserts, totals.builderGapTruncations,
                totals.builderFinalItems, totals.builderFinalStartItems,
                totals.builderFinalBItems, totals.builderFinalCItems,
                totals.builderFinalSeedItems, totals.builderFinalChunkItems,
                totals.builderFinalGapItems, totals.builderLengthRows, totals.builderChecks);
    }

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam StemBuilder constructor oracle.");
        System.out.println("#");
        System.out.println("# Each page runs in a fresh Epsilon-GC JVM and reaches real HEADS.");
        System.out.println("# Beam and head linkers are constructed in the production stable-x order.");
        System.out.println("# Each beam-origin V invokes the real private inspect(maxProfile) exactly once in");
        System.out.println("# production order, through complete StemBuilder constructor return and sb assignment.");
        System.out.println("# Rows freeze constructor inputs, mutations, section decisions, filament and glyph");
        System.out.println("# registrations, chunk occurrences, stable sorts, gaps, final items, and lengths.");
        System.out.println("# Reflection asserts exact replay plus no SIG, stem-map, linker, or C-builder mutation.");
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

    private static List<Glyph> glyphIndexGlyphs (GlyphIndex index)
    {
        final List<Glyph> glyphs = new ArrayList<>();
        final Iterator<Glyph> iterator = index.iterator();
        while (iterator.hasNext()) glyphs.add(iterator.next());
        return glyphs;
    }

    private static void emitBuilderState (
            String page,
            Sheet sheet,
            SystemInfo system,
            StemsRetriever retriever,
            AbstractBeamInter beam,
            Object b,
            Object v,
            StemBuilder builder,
            int builderOrdinal,
            int maxProfile,
            int builderYDir,
            boolean directionDiverges,
            List<Glyph> seedBefore,
            List<Object> inputBs,
            List<Object> inputCs,
            List<Glyph> glyphsBefore,
            List<StraightFilament> filamentsBefore,
            int maxStemThickness,
            double maxLineSectionDx,
            double maxStemAlignmentDx,
            double maxStemAlignmentDy,
            IdentityHashMap<Object, String> bAliases,
            IdentityHashMap<Object, String> cAliases,
            IdentityHashMap<Glyph, String> chunkAliases,
            int sigVerticesBefore,
            int sigEdgesBefore,
            int systemStemsBefore,
            int linkMutationCount,
            int unexpectedBuilderMutations,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final Line2D theoLine = (Line2D) V_THEO_LINE.get(v);
        final Area luArea = (Area) V_LU_AREA.get(v);
        final Rectangle yRange = (Rectangle) STEM_BUILDER_Y_RANGE.get(builder);
        final Glyph startStump = (Glyph) B_STUMP.get(b);
        if (startStump == null) totals.builderStumplessStarts++;
        final Double actualLastHeadY = (Double) STEM_BUILDER_LAST_HEAD_Y.get(builder);
        final Set<Glyph> actualSeedSet = (Set<Glyph>) V_SEEDS.get(v);

        final List<Glyph> replaySeeds = new ArrayList<>(seedBefore);
        final List<Glyph> seedRemoved = replayUnaligned(
                page, system, builderOrdinal, "seed", replaySeeds, startStump,
                builderYDir, maxStemAlignmentDx, maxStemAlignmentDy, totals, hashes);

        final List<StemLinker> inputTargets = new ArrayList<>();
        for (Object target : inputBs) inputTargets.add((StemLinker) target);
        for (Object target : inputCs) inputTargets.add((StemLinker) target);
        final List<ReplayItem> targetItems = new ArrayList<>();
        int targetOrdinal = 0;
        for (StemLinker target : inputTargets) {
            final Glyph stump = target.getStump();
            final boolean removed = seedRemoved.contains(stump);
            final String alias = linkerAlias(target, bAliases, cAliases);
            emit(String.format(
                    "stemsbeambuildertargetfilter %s system %d builder %d ordinal %d "
                            + "alias %s kind %s stump %s removedByContent %s action %s",
                    page, system.getId(), builderOrdinal, targetOrdinal++, alias,
                    target instanceof StemHalfLinker ? "C" : "B", glyphAlias(stump), removed,
                    removed ? "removeUnalignedStump" : "keep"), hashes);
            totals.builderInputTargets++;
            final boolean cTarget = target instanceof StemHalfLinker;
            if (cTarget) totals.builderInputCTargets++; else totals.builderInputBTargets++;
            if (removed) {
                totals.builderRemovedTargets++;
                if (cTarget) totals.builderRemovedCTargets++; else totals.builderRemovedBTargets++;
                continue;
            }
            final int contrib = target instanceof StemHalfLinker
                    ? (stump != null ? contrib(yRange, stump.getBounds()) : 0)
                    : (stump != null ? stump.getBounds().height : 0);
            targetItems.add(new ReplayItem(
                    target instanceof StemHalfLinker ? "C" : "B",
                    alias,
                    target,
                    stump,
                    linkerLine(target),
                    contrib,
                    targetItems.size()));
        }
        totals.builderRetainedTargets += targetItems.size();
        for (ReplayItem item : targetItems) {
            if (item.kind.equals("C")) totals.builderRetainedCTargets++;
            else totals.builderRetainedBTargets++;
        }
        emitStableSort(
                page, system, builderOrdinal, "targets", targetItems, builderYDir, totals, hashes);

        Double replayLastHeadY = null;
        for (int index = targetItems.size() - 1; index >= 0; index--) {
            final ReplayItem item = targetItems.get(index);
            if (item.kind.equals("C")) {
                replayLastHeadY = item.linker.getReferencePoint().getY();
                break;
            }
        }
        if (!sameOptionalDoubleBits(actualLastHeadY, replayLastHeadY)) {
            throw new IllegalStateException("StemBuilder lastHeadY differs from replay");
        }

        final List<Glyph> pastHead = new ArrayList<>();
        if (replayLastHeadY != null) {
            for (Glyph seed : replaySeeds) {
                final Point center = GeoUtil.center(seed.getBounds());
                if (builderYDir * Double.compare(center.getY(), replayLastHeadY) >= 0) {
                    seedRemoved.add(seed);
                    pastHead.add(seed);
                }
            }
            replaySeeds.removeAll(seedRemoved);
        }
        if (!sameIdentityIteration(actualSeedSet, replaySeeds)) {
            throw new IllegalStateException("StemBuilder seed mutation differs from replay");
        }
        totals.builderInputSeeds += seedBefore.size();
        totals.builderRetainedSeeds += replaySeeds.size();
        totals.builderRemovedSeeds += seedBefore.size() - replaySeeds.size();
        for (int ordinal = 0; ordinal < seedBefore.size(); ordinal++) {
            final Glyph seed = seedBefore.get(ordinal);
            final boolean unaligned = seedRemoved.contains(seed) && !pastHead.contains(seed);
            final boolean lastHead = pastHead.contains(seed);
            emit(String.format(
                    "stemsbeambuilderseedfilter %s system %d builder %d ordinal %d glyph %s "
                            + "unalignedContent %s pastLastHeadContent %s retainedIdentity %s",
                    page, system.getId(), builderOrdinal, ordinal, glyphAlias(seed), unaligned,
                    lastHead, containsIdentity(replaySeeds, seed)), hashes);
        }

        emitSectionScans(
                page, system, builderOrdinal, theoLine, luArea, startStump, replayLastHeadY,
                builderYDir, maxStemThickness, maxLineSectionDx, totals, hashes);

        final List<StraightFilament> filamentsAfter = filamentIndexFilaments(
                sheet.getFilamentIndex());
        final Set<StraightFilament> beforeFilamentSet = identitySet(filamentsBefore);
        final List<StraightFilament> newFilaments = new ArrayList<>();
        for (StraightFilament filament : filamentsAfter) {
            if (!beforeFilamentSet.contains(filament)) newFilaments.add(filament);
        }
        final List<Glyph> glyphsAfter = glyphIndexGlyphs(sheet.getGlyphIndex());
        final List<Glyph> chunkAttempts = new ArrayList<>();
        final List<ChunkEvent> chunkEvents = new ArrayList<>();
        final List<Glyph> canonicalAtAttempt = new ArrayList<>(glyphsBefore);
        for (int filamentOrdinal = 0; filamentOrdinal < newFilaments.size(); filamentOrdinal++) {
            final int capturedFilamentOrdinal = filamentOrdinal;
            final StraightFilament filament = newFilaments.get(filamentOrdinal);
            final Glyph candidate = filament.toGlyph(null);
            final Glyph priorCanonical = equalGlyph(canonicalAtAttempt, candidate);
            final boolean reuse = priorCanonical != null;
            final Glyph canonical = reuse ? priorCanonical : equalGlyph(glyphsAfter, candidate);
            if (canonical == null) {
                throw new IllegalStateException("registered filament glyph missing from GlyphIndex");
            }
            if (!reuse) canonicalAtAttempt.add(canonical);
            chunkAttempts.add(canonical);
            final String eventAlias = "chunkEvent:s" + system.getId() + ":" + builderOrdinal
                    + ":" + filamentOrdinal;
            chunkEvents.add(new ChunkEvent(eventAlias, canonical, "pending", -1));
            final String origin = reuse
                    ? glyphOrigin(canonical, system, retriever, bAliases, cAliases, chunkAliases)
                    : "newStemBuilderChunk";
            final String alias = chunkAliases.computeIfAbsent(
                    canonical,
                    ignored -> "chunk:s" + system.getId() + ":" + builderOrdinal + ":"
                            + capturedFilamentOrdinal);
            emitFilament(
                    page, system, builderOrdinal, filamentOrdinal, filament, alias, totals, hashes);
            emit(String.format(
                    "stemsbeambuilderglyphreg %s system %d builder %d ordinal %d "
                            + "event %s result %s canonical %s originCategories %s "
                            + "bounds %s orientation %s weight %d runs %d sha256 %s",
                    page, system.getId(), builderOrdinal, filamentOrdinal,
                    eventAlias, reuse ? "Reuse" : "New", glyphAlias(canonical),
                    glyphOriginCategories(origin),
                    rectangle(canonical.getBounds()),
                    canonical.getRunTable().getOrientation(), canonical.getWeight(),
                    glyphRunCount(canonical), glyphRunSha256(canonical)), hashes);
            totals.builderGlyphAttempts++;
            if (reuse) {
                totals.builderGlyphReuses++;
                totals.countReuseOrigin(origin);
            } else totals.builderGlyphNews++;
        }
        totals.builderFilaments += newFilaments.size();

        final List<ChunkEvent> survivingEvents = new ArrayList<>();
        for (ChunkEvent event : chunkEvents) {
            if (replaySeeds.contains(event.glyph)) {
                event.action = "seedEqual";
            } else {
                survivingEvents.add(event);
            }
        }
        final List<Glyph> survivingGlyphs = chunkEventGlyphs(survivingEvents);
        final List<Glyph> unalignedChunks = replayUnaligned(
                page, system, builderOrdinal, "chunk", survivingGlyphs, startStump,
                builderYDir, maxStemAlignmentDx, maxStemAlignmentDy, totals, hashes);
        for (Iterator<ChunkEvent> iterator = survivingEvents.iterator(); iterator.hasNext();) {
            final ChunkEvent event = iterator.next();
            if (unalignedChunks.contains(event.glyph)) {
                event.action = "unaligned";
                iterator.remove();
            }
        }
        if (startStump != null) {
            for (Iterator<ChunkEvent> iterator = survivingEvents.iterator(); iterator.hasNext();) {
                final ChunkEvent event = iterator.next();
                if (startStump.equals(event.glyph)) {
                    event.action = "startEqual";
                    iterator.remove();
                    break;
                }
            }
        }
        Collections.sort(
                survivingEvents,
                (left, right) -> (builderYDir > 0 ? Glyphs.byOrdinate : Glyphs.byReverseBottom)
                        .compare(left.glyph, right.glyph));
        for (int ordinal = 0; ordinal < survivingEvents.size(); ordinal++) {
            final ChunkEvent event = survivingEvents.get(ordinal);
            event.action = "keep";
            event.finalOrdinal = ordinal;
        }
        for (int ordinal = 0; ordinal < chunkEvents.size(); ordinal++) {
            final ChunkEvent event = chunkEvents.get(ordinal);
            emit(String.format(
                    "stemsbeambuilderchunkfilter %s system %d builder %d ordinal %d event %s glyph %s "
                            + "seedContentEqual %s unalignedContent %s startContentEqual %s "
                            + "finalOccurrenceOrdinal %d action %s headParts NA",
                    page, system.getId(), builderOrdinal, ordinal, event.alias,
                    glyphAlias(event.glyph), replaySeeds.contains(event.glyph),
                    unalignedChunks.contains(event.glyph),
                    startStump != null && startStump.equals(event.glyph), event.finalOrdinal,
                    event.action), hashes);
            if (!event.action.equals("keep")) totals.builderChunkRemovals++;
            if (event.action.equals("seedEqual")) totals.builderChunkSeedDrops++;
            if (event.action.equals("unaligned")) totals.builderChunkUnalignedDrops++;
            if (event.action.equals("startEqual")) totals.builderChunkStartDrops++;
        }

        final List<ReplayItem> preItems = buildPreItems(
                page, system, builderOrdinal, v, startStump, yRange, replaySeeds, targetItems,
                survivingEvents, builderYDir, bAliases, cAliases, totals, hashes);
        final List<FinalReplayItem> finalReplay = emitGapReplay(
                page, system, builderOrdinal, preItems, builderYDir, maxProfile, totals, hashes);
        emitActualItems(
                page, system, builderOrdinal, builder, finalReplay, totals, hashes);

        final TreeMap<Integer, Integer> lengthMap =
                (TreeMap<Integer, Integer>) STEM_BUILDER_LENGTH_MAP.get(builder);
        for (int profile = 0; profile <= Profiles.MAX_VALUE; profile++) {
            final int threshold = sheet.getScale().toPixels(StemChecker.getMaxYGap(profile));
            emit(String.format(
                    "stemsbeambuilderlength %s system %d builder %d profile %d threshold %d "
                            + "length %d",
                    page, system.getId(), builderOrdinal, profile, threshold,
                    lengthMap.get(profile)), hashes);
            totals.builderLengthRows++;
        }

        if (system.getSig().vertexSet().size() != sigVerticesBefore
                || system.getSig().edgeSet().size() != sigEdgesBefore
                || ((Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever)).size() != systemStemsBefore) {
            throw new IllegalStateException("StemBuilder constructor mutated SIG or systemStems");
        }
        for (Object c : cAliases.keySet()) {
            if (C_STEM_BUILDER.get(c) != null) {
                throw new IllegalStateException("beam StemBuilder assigned a C builder");
            }
        }
        emit(String.format(
                "stemsbeambuilderend %s system %d builder %d outputSeeds %s lastHeadY %s "
                        + "inputTargets %d retainedTargetItems %d filaments %d glyphAttempts %d "
                        + "glyphDelta %d sigVertexDelta 0 sigEdgeDelta 0 systemStemDelta 0 "
                        + "linkMutations %d "
                        + "unexpectedBuilderMutations %d noLinkMutation true "
                        + "noCBuilderMutation true",
                page, system.getId(), builderOrdinal, glyphAliases(replaySeeds),
                replayLastHeadY != null ? hexDouble(replayLastHeadY) : "-", inputTargets.size(),
                targetItems.size(), newFilaments.size(), chunkAttempts.size(),
                glyphsAfter.size() - glyphsBefore.size(), linkMutationCount,
                unexpectedBuilderMutations), hashes);
        totals.builders++;
        if (maxProfile == Profiles.BEAM_SEED) totals.profile3Builders++;
        if (maxProfile == Profiles.BEAM_SIDE) totals.profile4Builders++;
        if (builderYDir < 0) totals.topBuilders++; else totals.bottomBuilders++;
        if (directionDiverges) totals.builderDirectionDivergences++;
    }

    private static List<Glyph> replayUnaligned (
            String page,
            SystemInfo system,
            int builderOrdinal,
            String phase,
            List<Glyph> glyphs,
            Glyph startStump,
            int yDir,
            double maxDx,
            double maxDy,
            Totals totals,
            RowHasher... hashes)
    {
        final List<Glyph> removed = new ArrayList<>();
        final List<Glyph> sorted = new ArrayList<>(glyphs);
        Collections.sort(sorted, yDir > 0 ? Glyphs.byOrdinate : Glyphs.byReverseBottom);
        final boolean startOriginallyPresent = startStump != null && sorted.contains(startStump);
        if (startStump != null) {
            sorted.remove(startStump);
            sorted.add(0, startStump);
        }
        int comparisonOrdinal = 0;
        for (int index = 0; index < sorted.size() - 1; index++) {
            final Glyph first = sorted.get(index);
            final Glyph second = sorted.get(index + 1);
            final Point2D dsk1 = system.getSkew().deskewed(first.getCentroidDouble());
            final Point2D dsk2 = system.getSkew().deskewed(second.getCentroidDouble());
            final double dy = Math.abs(dsk2.getY() - dsk1.getY());
            final boolean dyBypass = dy > maxDy;
            final double dx = dyBypass ? Double.NaN : Math.abs(dsk2.getX() - dsk1.getX());
            final boolean aligned = dyBypass || dx <= maxDx;
            Glyph alien = null;
            boolean tieRemoveSecond = false;
            if (!aligned) {
                alien = first.getBounds().height < second.getBounds().height ? first : second;
                tieRemoveSecond = first.getBounds().height == second.getBounds().height;
                removed.add(alien);
                sorted.remove(alien);
                index--;
                totals.builderAlignRemovals++;
                if (tieRemoveSecond) totals.builderAlignTieRemovals++;
            }
            emit(String.format(
                    "stemsbeambuilderalign %s system %d builder %d phase %s ordinal %d "
                            + "startStump %s startOriginallyPresent %s first %s second %s "
                            + "firstBounds %s secondBounds %s firstCentroid %s secondCentroid %s "
                            + "firstDeskew %s secondDeskew %s dy %s maxDy %s dyBypass %s "
                            + "dx %s maxDx %s aligned %s alien %s tieHeightRemoveSecond %s",
                    page, system.getId(), builderOrdinal, phase, comparisonOrdinal++,
                    glyphAlias(startStump), startOriginallyPresent, glyphAlias(first),
                    glyphAlias(second), rectangle(first.getBounds()), rectangle(second.getBounds()),
                    point(first.getCentroidDouble()), point(second.getCentroidDouble()), point(dsk1),
                    point(dsk2), hexDouble(dy), hexDouble(maxDy), dyBypass,
                    dyBypass ? "-" : hexDouble(dx), hexDouble(maxDx), aligned,
                    glyphAlias(alien), tieRemoveSecond), hashes);
            totals.builderAlignComparisons++;
        }
        glyphs.removeAll(removed);
        return removed;
    }

    private static void emitStableSort (
            String page,
            SystemInfo system,
            int builderOrdinal,
            String phase,
            List<ReplayItem> items,
            int yDir,
            Totals totals,
            RowHasher... hashes)
    {
        final List<ReplayItem> before = new ArrayList<>(items);
        if (phase.equals("targets")) {
            totals.builderMaxTargetSortItems = Math.max(
                    totals.builderMaxTargetSortItems, before.size());
        } else {
            totals.builderMaxFinalSortItems = Math.max(
                    totals.builderMaxFinalSortItems, before.size());
        }
        if (before.size() >= 32) totals.builderSortListsAtLeast32++;
        final Comparator<ReplayItem> comparator = (left, right) -> {
            if (left.linker instanceof StemHalfLinker
                    && right.linker instanceof StemHalfLinker) {
                return yDir * Double.compare(
                        left.linker.getReferencePoint().getY(),
                        right.linker.getReferencePoint().getY());
            }
            return yDir > 0
                    ? Double.compare(left.line.getY1(), right.line.getY1())
                    : Double.compare(right.line.getY2(), left.line.getY2());
        };
        long cycles = 0;
        long equivalenceInconsistencies = 0;
        final MessageDigest offenderDigest = sha256Digest();
        for (int i = 0; i < before.size(); i++) {
            for (int j = i + 1; j < before.size(); j++) {
                for (int k = j + 1; k < before.size(); k++) {
                    final int ij = Integer.signum(comparator.compare(before.get(i), before.get(j)));
                    final int jk = Integer.signum(comparator.compare(before.get(j), before.get(k)));
                    final int ki = Integer.signum(comparator.compare(before.get(k), before.get(i)));
                    final boolean cycle = (ij < 0 && jk < 0 && ki < 0)
                            || (ij > 0 && jk > 0 && ki > 0);
                    boolean inconsistent = false;
                    final int ik = Integer.signum(comparator.compare(before.get(i), before.get(k)));
                    final int ji = -ij;
                    final int kj = -jk;
                    if (ij == 0 && (ik != jk || -ik != ki)) inconsistent = true;
                    if (jk == 0 && (ji != ki || ij != ik)) inconsistent = true;
                    if (ik == 0 && (ij != kj || ji != jk)) inconsistent = true;
                    if (cycle || inconsistent) {
                        updateDigest(
                                offenderDigest,
                                i + ":" + j + ":" + k + ":" + ij + ":" + jk + ":"
                                        + ki + ":" + ik + ":" + cycle + ":" + inconsistent
                                        + "\n");
                    }
                    if (cycle) cycles++;
                    if (inconsistent) equivalenceInconsistencies++;
                }
            }
        }
        emit(String.format(
                "stemsbeambuildersortaudit %s system %d builder %d phase %s items %d "
                        + "strictCycles %d equivalenceInconsistencies %d offenderSha256 %s",
                page, system.getId(), builderOrdinal, phase, before.size(), cycles,
                equivalenceInconsistencies, digestHex(offenderDigest.digest())), hashes);
        totals.builderSortAudits++;
        totals.builderSortCycles += cycles;
        totals.builderSortEquivalenceInconsistencies += equivalenceInconsistencies;
        Collections.sort(items, comparator);
        for (int input = 0; input < before.size(); input++) {
            final ReplayItem item = before.get(input);
            final int output = identityIndex(items, item);
            final StringBuilder equalInputs = new StringBuilder();
            final StringBuilder stableEqualPredecessors = new StringBuilder();
            for (int other = 0; other < before.size(); other++) {
                if (other != input && comparator.compare(before.get(other), item) == 0) {
                    append(equalInputs, Integer.toString(other));
                    if (other < input) append(stableEqualPredecessors, Integer.toString(other));
                }
            }
            emit(String.format(
                    "stemsbeambuildersort %s system %d builder %d phase %s input %d "
                            + "output %d alias %s kind %s line %s ref %s key1 %s key2 %s "
                            + "equalInputs %s stableEqualPredecessors %s",
                    page, system.getId(), builderOrdinal, phase, input, output, item.alias,
                    item.kind, line(item.line),
                    item.linker != null ? point(item.linker.getReferencePoint()) : "-",
                    hexDouble(item.line.getY1()), hexDouble(item.line.getY2()),
                    emptyToken(equalInputs), emptyToken(stableEqualPredecessors)), hashes);
            totals.builderSortRows++;
        }
    }

    private static boolean sameOptionalDoubleBits (Double left,
                                                   Double right)
    {
        if (left == null || right == null) return left == right;
        return Double.doubleToLongBits(left) == Double.doubleToLongBits(right);
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

    private static String linkerAlias (
            StemLinker linker,
            IdentityHashMap<Object, String> bAliases,
            IdentityHashMap<Object, String> cAliases)
    {
        final String bAlias = bAliases.get(linker);
        return bAlias != null ? bAlias : cAliases.get(linker);
    }

    private static boolean containsIdentity (Collection<?> values,
                                             Object target)
    {
        for (Object value : values) if (value == target) return true;
        return false;
    }

    private static List<Glyph> chunkEventGlyphs (List<ChunkEvent> events)
    {
        final List<Glyph> glyphs = new ArrayList<>();
        for (ChunkEvent event : events) glyphs.add(event.glyph);
        return glyphs;
    }

    private static IdentityHashMap<Object, String> linkStateSnapshot (
            List<Inter> systemBeams,
            IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final IdentityHashMap<Object, String> states = new IdentityHashMap<>();
        for (Inter inter : systemBeams) {
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker());
            for (Object b : allB) {
                states.put(b, B_LINKED.getBoolean(b) + ":" + B_CLOSED.getBoolean(b));
                final Map<?, ?> vMap = (Map<?, ?>) B_V_LINKERS.get(b);
                for (Object v : vMap.values()) {
                    states.put(v, V_IS_LINKED.invoke(v) + ":" + V_IS_CLOSED.invoke(v));
                }
            }
        }
        for (Object c : cAliases.keySet()) {
            states.put(c, C_IS_LINKED.invoke(c) + ":" + C_IS_CLOSED.invoke(c));
        }
        return states;
    }

    private static int linkStateMutationCount (
            IdentityHashMap<Object, String> before,
            List<Inter> systemBeams,
            IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        int mutations = 0;
        final IdentityHashMap<Object, String> after = linkStateSnapshot(systemBeams, cAliases);
        for (Map.Entry<Object, String> entry : before.entrySet()) {
            if (!entry.getValue().equals(after.get(entry.getKey()))) mutations++;
        }
        for (Map.Entry<Object, String> entry : after.entrySet()) {
            if (!before.containsKey(entry.getKey()) && !entry.getValue().equals("false:false")) {
                mutations++;
            }
        }
        return mutations;
    }

    private static IdentityHashMap<Object, Object> builderStateSnapshot (
            List<Inter> systemBeams,
            IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final IdentityHashMap<Object, Object> states = new IdentityHashMap<>();
        for (Inter inter : systemBeams) {
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(
                    ((AbstractBeamInter) inter).getLinker());
            for (Object b : allB) {
                for (Object v : ((Map<?, ?>) B_V_LINKERS.get(b)).values()) {
                    states.put(v, V_STEM_BUILDER.get(v));
                }
            }
        }
        for (Object c : cAliases.keySet()) states.put(c, C_STEM_BUILDER.get(c));
        return states;
    }

    private static int builderStateMutationCount (
            IdentityHashMap<Object, Object> before,
            List<Inter> systemBeams,
            IdentityHashMap<Object, String> cAliases,
            Object currentV,
            StemBuilder expectedBuilder)
        throws Exception
    {
        int unexpected = 0;
        final IdentityHashMap<Object, Object> after = builderStateSnapshot(systemBeams, cAliases);
        for (Map.Entry<Object, Object> entry : before.entrySet()) {
            final Object expected = entry.getKey() == currentV ? expectedBuilder : entry.getValue();
            if (after.get(entry.getKey()) != expected) unexpected++;
        }
        for (Object object : after.keySet()) {
            if (!before.containsKey(object) && after.get(object) != null) unexpected++;
        }
        return unexpected;
    }

    private static void emitSectionScans (
            String page,
            SystemInfo system,
            int builderOrdinal,
            Line2D theoLine,
            Area luArea,
            Glyph startStump,
            Double lastHeadY,
            int yDir,
            int maxStemThickness,
            double maxLineSectionDx,
            Totals totals,
            RowHasher... hashes)
    {
        final Rectangle stumpBox = startStump != null ? startStump.getBounds() : null;
        final List<Section> systemVerticals = new ArrayList<>(system.getVerticalSections());
        final List<Section> systemHorizontals = new ArrayList<>(system.getHorizontalSections());
        final MessageDigest vDigest = sha256Digest();
        final TreeMap<String, Integer> vReasons = new TreeMap<>();
        final List<Section> acceptedV = new ArrayList<>();
        for (int ordinal = 0; ordinal < systemVerticals.size(); ordinal++) {
            final Section section = systemVerticals.get(ordinal);
            final Rectangle box = section.getBounds();
            final boolean intersects = luArea.intersects(box);
            final boolean widthAccepted = intersects && box.width <= maxStemThickness;
            final int stumpOverlap = widthAccepted && stumpBox != null
                    ? GeoUtil.yOverlap(box, stumpBox) : Integer.MIN_VALUE;
            final boolean stumpRejected = widthAccepted && stumpBox != null && stumpOverlap > 0
                    && box.height < stumpBox.height;
            final Point2D center = !intersects || !widthAccepted || stumpRejected
                    ? null : section.getCentroid2D();
            final boolean lastHeadRejected = center != null && lastHeadY != null
                    && yDir * Double.compare(center.getY(), lastHeadY) >= 0;
            final double distance = center != null && !lastHeadRejected
                    ? theoLine.ptLineDist(center) : Double.NaN;
            final boolean distanceRejected = Double.isFinite(distance)
                    && distance > maxLineSectionDx;
            final String action;
            if (!intersects) action = "outsideArea";
            else if (!widthAccepted) action = "wide";
            else if (stumpRejected) action = "shorterStumpOverlap";
            else if (lastHeadRejected) action = "pastLastHead";
            else if (distanceRejected) action = "lineDistance";
            else {
                action = "accept";
                acceptedV.add(section);
            }
            final String decision = String.format(
                    "sourceOrdinal %d alias v:%d bounds %s areaIntersects %s "
                            + "width %d maxWidth %d widthAccepted %s stumpOverlap %s "
                            + "stumpHeight %s shorterStumpReject %s center %s lastHeadY %s "
                            + "lastHeadReject %s theoDistance %s maxLineSectionDx %s action %s",
                    ordinal, ordinal,
                    rectangle(box), intersects, box.width, maxStemThickness,
                    intersects ? Boolean.toString(widthAccepted) : "-",
                    widthAccepted && stumpBox != null ? Integer.toString(stumpOverlap) : "-",
                    stumpBox != null ? Integer.toString(stumpBox.height) : "-",
                    widthAccepted && stumpBox != null ? Boolean.toString(stumpRejected) : "-",
                    center != null ? point(center) : "-",
                    lastHeadY != null ? hexDouble(lastHeadY) : "-",
                    center != null && lastHeadY != null ? Boolean.toString(lastHeadRejected) : "-",
                    Double.isFinite(distance) ? hexDouble(distance) : "-",
                    hexDouble(maxLineSectionDx), action);
            updateDigest(vDigest, decision + "\n");
            vReasons.merge(action, 1, Integer::sum);
            totals.builderVSectionScans++;
            if (action.equals("accept")) totals.builderVSectionAccepts++;
            else if (action.equals("outsideArea")) totals.builderVSectionOutside++;
            else if (action.equals("wide")) totals.builderVSectionWide++;
            else if (action.equals("shorterStumpOverlap")) totals.builderVSectionStumpOverlap++;
            else if (action.equals("pastLastHead")) totals.builderVSectionPastHead++;
            else if (action.equals("lineDistance")) totals.builderVSectionDistance++;
        }
        Collections.sort(acceptedV, Section.byFullPosition);
        final String vAccepted = selectedOrdinals(acceptedV, systemVerticals);
        emit(String.format(
                "stemsbeambuildervsection %s system %d builder %d sourceCount %d "
                        + "acceptedCount %d acceptedSourceOrdinals %s reasons %s "
                        + "decisionSha256 %s",
                page, system.getId(), builderOrdinal, systemVerticals.size(), acceptedV.size(),
                vAccepted, reasonToken(vReasons), digestHex(vDigest.digest())), hashes);
        totals.builderSelectedSectionRows += acceptedV.size();

        final MessageDigest hDigest = sha256Digest();
        final TreeMap<String, Integer> hReasons = new TreeMap<>();
        final List<Section> acceptedH = new ArrayList<>();
        for (int ordinal = 0; ordinal < systemHorizontals.size(); ordinal++) {
            final Section section = systemHorizontals.get(ordinal);
            final Rectangle box = section.getBounds();
            final boolean intersects = luArea.intersects(box);
            final boolean widthAccepted = intersects && box.width <= 1;
            final Point2D center = widthAccepted ? section.getCentroid2D() : null;
            final boolean lastHeadRejected = center != null && lastHeadY != null
                    && yDir * Double.compare(center.getY(), lastHeadY) >= 0;
            final String action = !intersects ? "outsideArea" : !widthAccepted ? "wide"
                    : lastHeadRejected ? "pastLastHead" : "accept";
            if (action.equals("accept")) acceptedH.add(section);
            final String decision = String.format(
                    "sourceOrdinal %d alias h:%d bounds %s areaIntersects %s "
                            + "width %d maxWidth 1 widthAccepted %s center %s lastHeadY %s "
                            + "lastHeadReject %s action %s",
                    ordinal, ordinal,
                    rectangle(box), intersects, box.width,
                    intersects ? Boolean.toString(widthAccepted) : "-",
                    center != null ? point(center) : "-",
                    lastHeadY != null ? hexDouble(lastHeadY) : "-",
                    center != null && lastHeadY != null ? Boolean.toString(lastHeadRejected) : "-",
                    action);
            updateDigest(hDigest, decision + "\n");
            hReasons.merge(action, 1, Integer::sum);
            totals.builderHSectionScans++;
            if (action.equals("accept")) totals.builderHSectionAccepts++;
            else if (action.equals("outsideArea")) totals.builderHSectionOutside++;
            else if (action.equals("wide")) totals.builderHSectionWide++;
            else if (action.equals("pastLastHead")) totals.builderHSectionPastHead++;
        }
        Collections.sort(acceptedH, Section.byFullPosition);
        final String hAccepted = selectedOrdinals(acceptedH, systemHorizontals);
        emit(String.format(
                "stemsbeambuilderhsection %s system %d builder %d sourceCount %d "
                        + "acceptedCount %d acceptedSourceOrdinals %s reasons %s "
                        + "decisionSha256 %s",
                page, system.getId(), builderOrdinal, systemHorizontals.size(), acceptedH.size(),
                hAccepted, reasonToken(hReasons), digestHex(hDigest.digest())), hashes);
        totals.builderSelectedSectionRows += acceptedH.size();
    }

    private static Glyph equalGlyph (List<Glyph> glyphs,
                                     Glyph candidate)
    {
        for (Glyph glyph : glyphs) if (glyph.equals(candidate)) return glyph;
        return null;
    }

    private static void emitFilament (
            String page,
            SystemInfo system,
            int builderOrdinal,
            int filamentOrdinal,
            StraightFilament filament,
            String chunkAlias,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final StringBuilder members = new StringBuilder();
        int memberOrdinal = 0;
        for (Section member : filament.getMembers()) {
            final String alias = sectionAlias(member, system);
            append(members, alias);
            emit(String.format(
                    "stemsbeambuilderfilamentmember %s system %d builder %d filament %d "
                            + "ordinal %d alias %s orientation %s bounds %s weight %d runs %d",
                    page, system.getId(), builderOrdinal, filamentOrdinal, memberOrdinal++, alias,
                    member.getOrientation(), rectangle(member.getBounds()), member.getWeight(),
                    member.getRunCount()), hashes);
            totals.builderFilamentMembers++;
        }
        emit(String.format(
                "stemsbeambuilderfilament %s system %d builder %d ordinal %d "
                        + "registrationDelta %d chunkAlias %s "
                        + "members %s bounds %s weight %d centerLine %s start %s stop %s "
                        + "meanThickness %s meanDistance %s length %d",
                page, system.getId(), builderOrdinal, filamentOrdinal, filamentOrdinal + 1,
                chunkAlias, emptyToken(members), rectangle(filament.getBounds()),
                filament.getWeight(), line(filament.getCenterLine()),
                point(filament.getStartPoint(org.audiveris.omr.run.Orientation.VERTICAL)),
                point(filament.getStopPoint(org.audiveris.omr.run.Orientation.VERTICAL)),
                hexDouble(filament.getMeanThickness(org.audiveris.omr.run.Orientation.VERTICAL)),
                hexDouble(filament.getMeanDistance()),
                filament.getLength(org.audiveris.omr.run.Orientation.VERTICAL)), hashes);
    }

    private static String sectionAlias (Section member,
                                        SystemInfo system)
        throws Exception
    {
        final Section underlying = member instanceof LinkedSection
                ? (Section) LINKED_SECTION_SOURCE.get(member) : member;
        final List<Section> source = new ArrayList<>(member.getOrientation().isVertical()
                ? system.getVerticalSections() : system.getHorizontalSections());
        int match = -1;
        for (int ordinal = 0; ordinal < source.size(); ordinal++) {
            final Section candidate = source.get(ordinal);
            if (candidate == underlying) {
                if (match >= 0) {
                    throw new IllegalStateException("duplicate SystemInfo section identity");
                }
                match = ordinal;
            }
        }
        if (match >= 0) {
            return (member.getOrientation().isVertical() ? "v:" : "h:") + match;
        }
        throw new IllegalStateException("filament member is outside SystemInfo section inputs");
    }

    private static String globalSectionOrdinals (
            Sheet sheet,
            Collection<Section> systemSections,
            String lagName)
    {
        final Lag lag = sheet.getLagManager().getLag(lagName);
        final IdentityHashMap<Section, Integer> globalOrdinals = new IdentityHashMap<>();
        int ordinal = 0;
        for (Section section : lag.getEntities()) globalOrdinals.put(section, ordinal++);
        final StringBuilder token = new StringBuilder();
        for (Section section : systemSections) {
            final Integer global = globalOrdinals.get(section);
            if (global == null) {
                throw new IllegalStateException("SystemInfo section absent from sheet lag " + lagName);
            }
            append(token, Integer.toString(global));
        }
        return emptyToken(token);
    }

    private static String glyphOrigin (
            Glyph glyph,
            SystemInfo system,
            StemsRetriever retriever,
            IdentityHashMap<Object, String> bAliases,
            IdentityHashMap<Object, String> cAliases,
            IdentityHashMap<Glyph, String> chunkAliases)
        throws Exception
    {
        final java.util.TreeSet<String> origins = new java.util.TreeSet<>();
        final List<Glyph> seeds = (List<Glyph>) RETRIEVER_SYSTEM_SEEDS.get(retriever);
        for (int ordinal = 0; ordinal < seeds.size(); ordinal++) {
            final Glyph seed = seeds.get(ordinal);
            if (seed == glyph) origins.add("stemSeed:" + ordinal + ":identity");
            else if (seed.equals(glyph)) origins.add("stemSeed:" + ordinal + ":content");
        }
        for (Object b : bAliases.keySet()) {
            final Glyph stump = (Glyph) B_STUMP.get(b);
            if (stump == glyph) origins.add("beamStump:" + bAliases.get(b) + ":identity");
            else if (stump != null && stump.equals(glyph)) {
                origins.add("beamStump:" + bAliases.get(b) + ":content");
            }
        }
        for (Object c : cAliases.keySet()) {
            final Glyph stump = ((StemLinker) c).getStump();
            if (stump == glyph) origins.add("headStump:" + cAliases.get(c) + ":identity");
            else if (stump != null && stump.equals(glyph)) {
                origins.add("headStump:" + cAliases.get(c) + ":content");
            }
        }
        for (Map.Entry<Glyph, String> entry : chunkAliases.entrySet()) {
            if (entry.getKey() == glyph) {
                origins.add("priorStemBuilderChunk:" + entry.getValue() + ":identity");
            } else if (entry.getKey().equals(glyph)) {
                origins.add("priorStemBuilderChunk:" + entry.getValue() + ":content");
            }
        }
        for (Map.Entry<Glyph, String> entry : PAGE_REJECTED_HEAD_STUMPS.entrySet()) {
            if (entry.getKey() == glyph) {
                origins.add(entry.getValue() + ":identity");
            } else if (entry.getKey().equals(glyph)) {
                origins.add(entry.getValue() + ":content");
            }
        }
        for (Inter inter : system.getSig().vertexSet()) {
            final Glyph interGlyph = inter.getGlyph();
            if (interGlyph == null || (interGlyph != glyph && !interGlyph.equals(glyph))) continue;
            final String relation = interGlyph == glyph ? "identity" : "content";
            if (inter instanceof AbstractBeamInter) {
                origins.add("rawBeamOrHook:" + inter.getShape() + ":" + relation);
                continue;
            }
            if (inter instanceof HeadInter) {
                origins.add("head:" + inter.getShape() + ":" + relation);
                continue;
            }
            final String shape = String.valueOf(inter.getShape());
            if (shape.contains("LEDGER")) {
                origins.add("ledger:" + shape + ":" + relation);
                continue;
            }
            if (shape.contains("BAR") || shape.contains("BRACE") || shape.contains("BRACKET")) {
                origins.add("grid:" + shape + ":" + relation);
                continue;
            }
            origins.add("sigOther:" + shape + ":" + relation);
        }
        if (origins.isEmpty()) origins.add("otherPreStems:groups=" + glyphGroupsToken(glyph));
        return String.join("+", origins);
    }

    private static void emitGlyphInventory (
            String page,
            Sheet sheet,
            SystemInfo system,
            StemsRetriever retriever,
            IdentityHashMap<Object, String> bAliases,
            IdentityHashMap<Object, String> cAliases,
            IdentityHashMap<Glyph, String> chunkAliases,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final List<Glyph> glyphs = glyphIndexGlyphs(sheet.getGlyphIndex());
        Collections.sort(glyphs, Comparator.comparing(StemsBeamStemBuilderProbe::glyphAlias));
        final MessageDigest digest = sha256Digest();
        final TreeMap<String, Integer> categories = new TreeMap<>();
        for (Glyph glyph : glyphs) {
            final String origin = glyphOrigin(
                    glyph, system, retriever, bAliases, cAliases, chunkAliases);
            for (String member : origin.split("\\+")) {
                final String category = member.contains(":")
                        ? member.substring(0, member.indexOf(':')) : member;
                categories.merge(category, 1, Integer::sum);
            }
            updateDigest(digest, glyphAlias(glyph) + " origin " + origin + " groups "
                    + glyphGroupsToken(glyph) + "\n");
        }
        System.out.println(String.format(
                "stemsdiagnosticbeambuilderglyphinventory %s system %d count %d categories %s "
                        + "sortedStructuralSha256 %s",
                page, system.getId(), glyphs.size(), reasonToken(categories),
                digestHex(digest.digest())));
        totals.builderGlyphInventory += glyphs.size();
    }

    private static List<ReplayItem> buildPreItems (
            String page,
            SystemInfo system,
            int builderOrdinal,
            Object v,
            Glyph startStump,
            Rectangle yRange,
            List<Glyph> seeds,
            List<ReplayItem> sortedTargets,
            List<ChunkEvent> chunks,
            int yDir,
            IdentityHashMap<Object, String> bAliases,
            IdentityHashMap<Object, String> cAliases,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final List<ReplayItem> items = new ArrayList<>();
        final StemLinker start = (StemLinker) v;
        items.add(new ReplayItem(
                "startHalf", "start", start, startStump, linkerLine(start),
                startStump != null ? contrib(yRange, startStump.getBounds()) : 0, 0));
        for (ReplayItem target : sortedTargets) {
            items.add(new ReplayItem(
                    target.kind, target.alias, target.linker, target.glyph, target.line,
                    target.contrib, items.size()));
        }
        for (int ordinal = 0; ordinal < seeds.size(); ordinal++) {
            final Glyph seed = seeds.get(ordinal);
            StemLinker duplicate = null;
            for (ReplayItem target : sortedTargets) {
                if (seed == target.linker.getStump()) {
                    duplicate = target.linker;
                    break;
                }
            }
            final boolean startOverlap = startStump != null
                    && GeoUtil.yOverlap(startStump.getBounds(), seed.getBounds()) > 0;
            final int seedContrib = contrib(yRange, seed.getBounds());
            final String action = duplicate != null ? "duplicateTargetIdentity"
                    : startOverlap ? "startYOverlap" : seedContrib <= 0 ? "zeroContrib" : "keep";
            emit(String.format(
                    "stemsbeambuilderseeditemfilter %s system %d builder %d ordinal %d "
                            + "glyph %s duplicateTargetIdentity %s duplicateAlias %s "
                            + "startYOverlap %s contrib %d action %s",
                    page, system.getId(), builderOrdinal, ordinal, glyphAlias(seed),
                    duplicate != null,
                    duplicate != null ? linkerAlias(duplicate, bAliases, cAliases) : "-",
                    duplicate == null ? Boolean.toString(startOverlap) : "-",
                    duplicate == null && !startOverlap ? seedContrib : -1, action), hashes);
            if (!action.equals("keep")) {
                totals.builderSeedItemDrops++;
                continue;
            }
            items.add(new ReplayItem(
                    "seed", glyphAlias(seed), null, seed, seed.getCenterLine(), seedContrib,
                    items.size()));
        }
        for (ChunkEvent chunk : chunks) {
            items.add(new ReplayItem(
                    "chunk", chunk.alias, null, chunk.glyph, chunk.glyph.getCenterLine(),
                    contrib(yRange, chunk.glyph.getBounds()), items.size()));
        }
        if (items.size() > 1) {
            emitStableSort(
                    page, system, builderOrdinal, "items", items.subList(1, items.size()),
                    yDir, totals, hashes);
        }
        for (int ordinal = 0; ordinal < items.size(); ordinal++) {
            final ReplayItem item = items.get(ordinal);
            emit(String.format(
                    "stemsbeambuilderitempre %s system %d builder %d creationOrdinal %d "
                            + "sortedOrdinal %d kind %s alias %s line %s glyph %s contrib %d",
                    page, system.getId(), builderOrdinal, item.creationOrdinal, ordinal, item.kind,
                    item.alias, line(item.line), glyphAlias(item.glyph), item.contrib), hashes);
            totals.builderPreItems++;
            totals.countPreKind(item.kind);
        }
        return items;
    }

    private static List<FinalReplayItem> emitGapReplay (
            String page,
            SystemInfo system,
            int builderOrdinal,
            List<ReplayItem> items,
            int yDir,
            int maxProfile,
            Totals totals,
            RowHasher... hashes)
    {
        final List<FinalReplayItem> result = new ArrayList<>();
        final int maxGap = system.getSheet().getScale().toPixels(
                StemChecker.getMaxYGap(maxProfile));
        Point2D lastPt = null;
        int checkOrdinal = 0;
        for (int index = 0; index < items.size(); index++) {
            final ReplayItem item = items.get(index);
            final Point2D start = yDir > 0 ? item.line.getP1() : item.line.getP2();
            final Point2D stop = yDir > 0 ? item.line.getP2() : item.line.getP1();
            final double gap = lastPt != null
                    ? yDir * (start.getY() - lastPt.getY()) : Double.NaN;
            final String action;
            Line2D inserted = null;
            if (lastPt == null) action = "initial";
            else if (gap > maxGap) action = "truncate";
            else if (gap > 0.01) {
                action = "insert";
                inserted = yDir > 0 ? new Line2D.Double(lastPt, start)
                        : new Line2D.Double(start, lastPt);
            } else action = "contiguous";
            emit(String.format(
                    "stemsbeambuildergap %s system %d builder %d ordinal %d itemIndex %d "
                            + "itemAlias %s start %s stop %s lastBefore %s gap %s maxGap %d "
                            + "epsilon %s action %s insertedLine %s insertedContrib %s",
                    page, system.getId(), builderOrdinal, checkOrdinal++, index, item.alias,
                    point(start), point(stop), lastPt != null ? point(lastPt) : "-",
                    lastPt != null ? hexDouble(gap) : "-", maxGap, hexDouble(0.01), action,
                    inserted != null ? line(inserted) : "-",
                    inserted != null ? Integer.toString(inserted.getBounds().height) : "-"), hashes);
            totals.builderGapChecks++;
            if (action.equals("insert")) totals.builderGapInserts++;
            if (action.equals("truncate")) {
                totals.builderGapTruncations++;
                break;
            }
            if (inserted != null) {
                result.add(new FinalReplayItem(
                        "gap", "gap:" + result.size(), null, null, inserted,
                        inserted.getBounds().height));
            }
            result.add(new FinalReplayItem(
                    item.kind, item.alias, item.linker, item.glyph, item.line, item.contrib));
            if (lastPt == null || yDir * (stop.getY() - lastPt.getY()) > 0.01) lastPt = stop;
        }
        return result;
    }

    private static void emitActualItems (
            String page,
            SystemInfo system,
            int builderOrdinal,
            StemBuilder builder,
            List<FinalReplayItem> expected,
            Totals totals,
            RowHasher... hashes)
        throws Exception
    {
        final List<StemItem> items = (List<StemItem>) STEM_BUILDER_ITEMS.get(builder);
        if (items.size() != expected.size()) {
            throw new IllegalStateException("StemBuilder final item count differs from replay");
        }
        for (int ordinal = 0; ordinal < items.size(); ordinal++) {
            final StemItem item = items.get(ordinal);
            final FinalReplayItem replay = expected.get(ordinal);
            final Line2D itemLine = (Line2D) STEM_ITEM_LINE.get(item);
            final Glyph glyph = (Glyph) STEM_ITEM_GLYPH.get(item);
            final int itemContrib = STEM_ITEM_CONTRIB.getInt(item);
            Object linker = null;
            if (LINKER_ITEM_LINKER.getDeclaringClass().isAssignableFrom(item.getClass())) {
                linker = LINKER_ITEM_LINKER.get(item);
            }
            if (linker != replay.linker || glyph != replay.glyph
                    || itemContrib != replay.contrib || !sameLineBits(itemLine, replay.line)) {
                throw new IllegalStateException(
                        "StemBuilder final item differs from replay at " + ordinal);
            }
            emit(String.format(
                    "stemsbeambuilderitem %s system %d builder %d ordinal %d kind %s "
                            + "alias %s line %s glyph %s contrib %d",
                    page, system.getId(), builderOrdinal, ordinal, replay.kind, replay.alias,
                    line(itemLine), glyphAlias(glyph), itemContrib), hashes);
            totals.builderFinalItems++;
            totals.countFinalKind(replay.kind);
        }
    }

    private static List<StraightFilament> filamentIndexFilaments (FilamentIndex index)
    {
        final List<StraightFilament> filaments = new ArrayList<>();
        for (var filament : index.getEntities()) {
            if (filament instanceof StraightFilament straight) filaments.add(straight);
        }
        return filaments;
    }

    private static <T> Set<T> identitySet (Collection<T> values)
    {
        final Set<T> result = Collections.newSetFromMap(
                new IdentityHashMap<>(identityMapCapacity(values.size())));
        result.addAll(values);
        return result;
    }

    private static int identityMapCapacity (int size)
    {
        return Math.max(1, (2 * size) + 1);
    }

    private static String predictedBTargets (
            List<FindDecision> decisions,
            IdentityHashMap<Inter, Integer> beamSigOrdinals,
            IdentityHashMap<Object, String> bAliases)
    {
        final StringBuilder builder = new StringBuilder();
        for (FindDecision decision : decisions) {
            if (decision.reuse) {
                append(builder, bAliases.get(decision.best));
            } else {
                append(builder, "beam:" + beamSigOrdinals.get(decision.target)
                        + ":b:" + decision.beforeCount);
            }
        }
        return emptyToken(builder);
    }

    private static String glyphAliases (Collection<Glyph> glyphs)
    {
        final StringBuilder builder = new StringBuilder();
        for (Glyph glyph : glyphs) append(builder, glyphAlias(glyph));
        return emptyToken(builder);
    }

    private static String glyphAlias (Glyph glyph)
    {
        if (glyph == null) return "-";
        final Rectangle box = glyph.getBounds();
        return "g:" + box.x + ":" + box.y + ":" + box.width + ":" + box.height
                + ":" + glyphRunSha256(glyph);
    }

    private static String glyphRunSha256 (Glyph glyph)
    {
        try {
            final MessageDigest digest = MessageDigest.getInstance("SHA-256");
            final var table = glyph.getRunTable();
            updateDigest(digest, table.getOrientation() + " " + table.getWidth() + " "
                    + table.getHeight() + "\n");
            for (int sequence = 0; sequence < table.getSize(); sequence++) {
                final StringBuilder row = new StringBuilder().append(sequence);
                for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                        iterator.hasNext();) {
                    final var run = iterator.next();
                    row.append(' ').append(run.getStart()).append(':').append(run.getLength());
                }
                updateDigest(digest, row.append('\n').toString());
            }
            final StringBuilder result = new StringBuilder();
            for (byte value : digest.digest()) result.append(String.format("%02x", value & 0xff));
            return result.toString();
        } catch (java.security.NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static String glyphGroupsToken (Glyph glyph)
    {
        final List<String> groups = new ArrayList<>();
        for (GlyphGroup group : glyph.getGroups()) groups.add(group.name());
        Collections.sort(groups);
        return groups.isEmpty() ? "-" : String.join(",", groups);
    }

    private static String glyphOriginCategories (String origin)
    {
        final java.util.TreeSet<String> categories = new java.util.TreeSet<>();
        for (String member : origin.split("\\+")) {
            categories.add(member.contains(":")
                    ? member.substring(0, member.indexOf(':')) : member);
        }
        return String.join(",", categories);
    }

    private static void updateDigest (MessageDigest digest,
                                      String value)
    {
        digest.update(value.getBytes(StandardCharsets.UTF_8));
    }

    private static MessageDigest sha256Digest ()
    {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (java.security.NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static String digestHex (byte[] bytes)
    {
        final StringBuilder result = new StringBuilder();
        for (byte value : bytes) result.append(String.format("%02x", value & 0xff));
        return result.toString();
    }

    private static String selectedOrdinals (List<Section> selected,
                                            List<Section> source)
    {
        final StringBuilder builder = new StringBuilder();
        for (Section section : selected) append(builder, Integer.toString(identityIndex(source, section)));
        return emptyToken(builder);
    }

    private static String reasonToken (TreeMap<String, Integer> reasons)
    {
        final StringBuilder builder = new StringBuilder();
        for (Map.Entry<String, Integer> entry : reasons.entrySet()) {
            append(builder, entry.getKey() + ":" + entry.getValue());
        }
        return emptyToken(builder);
    }

    private static int glyphRunCount (Glyph glyph)
    {
        int count = 0;
        final var table = glyph.getRunTable();
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                iterator.next();
                count++;
            }
        }
        return count;
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
        // Reachability is already frozen by StemsBeamVLinkerInspectProbe. This constructor-boundary
        // fixture keeps those source-faithful assertions active without duplicating its 61 MB row
        // stream in a second committed oracle.
        if (row.startsWith("stemsbeamvinspect")) return;
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

    private record ReplayItem(
            String kind,
            String alias,
            StemLinker linker,
            Glyph glyph,
            Line2D line,
            int contrib,
            int creationOrdinal)
    {
    }

    private static final class ChunkEvent
    {
        final String alias;
        final Glyph glyph;
        String action;
        int finalOrdinal;

        ChunkEvent (String alias,
                    Glyph glyph,
                    String action,
                    int finalOrdinal)
        {
            this.alias = alias;
            this.glyph = glyph;
            this.action = action;
            this.finalOrdinal = finalOrdinal;
        }
    }

    private record FinalReplayItem(
            String kind,
            String alias,
            StemLinker linker,
            Glyph glyph,
            Line2D line,
            int contrib)
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
        long builders;
        long topBuilders;
        long bottomBuilders;
        long builderDirectionDivergences;
        long profile3Builders;
        long profile4Builders;
        long builderStumplessStarts;
        long builderInputSeeds;
        long builderRemovedSeeds;
        long builderRetainedSeeds;
        long builderInputTargets;
        long builderRemovedTargets;
        long builderRetainedTargets;
        long builderInputBTargets;
        long builderRemovedBTargets;
        long builderRetainedBTargets;
        long builderInputCTargets;
        long builderRemovedCTargets;
        long builderRetainedCTargets;
        long builderAlignComparisons;
        long builderAlignRemovals;
        long builderAlignTieRemovals;
        long builderVSectionScans;
        long builderVSectionAccepts;
        long builderVSectionOutside;
        long builderVSectionWide;
        long builderVSectionStumpOverlap;
        long builderVSectionPastHead;
        long builderVSectionDistance;
        long builderHSectionScans;
        long builderHSectionAccepts;
        long builderHSectionOutside;
        long builderHSectionWide;
        long builderHSectionPastHead;
        long builderSelectedSectionRows;
        long builderFilaments;
        long builderFilamentMembers;
        long builderGlyphAttempts;
        long builderGlyphInventory;
        long builderGlyphNews;
        long builderGlyphReuses;
        long builderReuseSeedOrigins;
        long builderReuseBeamStumpOrigins;
        long builderReuseHeadStumpOrigins;
        long builderReuseRejectedHeadStumpOrigins;
        long builderReusePriorChunkOrigins;
        long builderReuseRawBeamOrigins;
        long builderReuseHeadOrigins;
        long builderReuseLedgerOrigins;
        long builderReuseGridOrigins;
        long builderReuseSigOtherOrigins;
        long builderReuseUnmodeledOrigins;
        long builderChunkRemovals;
        long builderChunkSeedDrops;
        long builderChunkUnalignedDrops;
        long builderChunkStartDrops;
        long builderSeedItemDrops;
        long builderSortRows;
        long builderSortAudits;
        long builderSortCycles;
        long builderSortEquivalenceInconsistencies;
        long builderMaxTargetSortItems;
        long builderMaxFinalSortItems;
        long builderSortListsAtLeast32;
        long builderPreItems;
        long builderPreStartItems;
        long builderPreBItems;
        long builderPreCItems;
        long builderPreSeedItems;
        long builderPreChunkItems;
        long builderGapChecks;
        long builderGapInserts;
        long builderGapTruncations;
        long builderFinalItems;
        long builderFinalStartItems;
        long builderFinalBItems;
        long builderFinalCItems;
        long builderFinalSeedItems;
        long builderFinalChunkItems;
        long builderFinalGapItems;
        long builderLengthRows;

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
            builders += that.builders;
            topBuilders += that.topBuilders;
            bottomBuilders += that.bottomBuilders;
            builderDirectionDivergences += that.builderDirectionDivergences;
            profile3Builders += that.profile3Builders;
            profile4Builders += that.profile4Builders;
            builderStumplessStarts += that.builderStumplessStarts;
            builderInputSeeds += that.builderInputSeeds;
            builderRemovedSeeds += that.builderRemovedSeeds;
            builderRetainedSeeds += that.builderRetainedSeeds;
            builderInputTargets += that.builderInputTargets;
            builderRemovedTargets += that.builderRemovedTargets;
            builderRetainedTargets += that.builderRetainedTargets;
            builderInputBTargets += that.builderInputBTargets;
            builderRemovedBTargets += that.builderRemovedBTargets;
            builderRetainedBTargets += that.builderRetainedBTargets;
            builderInputCTargets += that.builderInputCTargets;
            builderRemovedCTargets += that.builderRemovedCTargets;
            builderRetainedCTargets += that.builderRetainedCTargets;
            builderAlignComparisons += that.builderAlignComparisons;
            builderAlignRemovals += that.builderAlignRemovals;
            builderAlignTieRemovals += that.builderAlignTieRemovals;
            builderVSectionScans += that.builderVSectionScans;
            builderVSectionAccepts += that.builderVSectionAccepts;
            builderVSectionOutside += that.builderVSectionOutside;
            builderVSectionWide += that.builderVSectionWide;
            builderVSectionStumpOverlap += that.builderVSectionStumpOverlap;
            builderVSectionPastHead += that.builderVSectionPastHead;
            builderVSectionDistance += that.builderVSectionDistance;
            builderHSectionScans += that.builderHSectionScans;
            builderHSectionAccepts += that.builderHSectionAccepts;
            builderHSectionOutside += that.builderHSectionOutside;
            builderHSectionWide += that.builderHSectionWide;
            builderHSectionPastHead += that.builderHSectionPastHead;
            builderSelectedSectionRows += that.builderSelectedSectionRows;
            builderFilaments += that.builderFilaments;
            builderFilamentMembers += that.builderFilamentMembers;
            builderGlyphAttempts += that.builderGlyphAttempts;
            builderGlyphInventory += that.builderGlyphInventory;
            builderGlyphNews += that.builderGlyphNews;
            builderGlyphReuses += that.builderGlyphReuses;
            builderReuseSeedOrigins += that.builderReuseSeedOrigins;
            builderReuseBeamStumpOrigins += that.builderReuseBeamStumpOrigins;
            builderReuseHeadStumpOrigins += that.builderReuseHeadStumpOrigins;
            builderReuseRejectedHeadStumpOrigins += that.builderReuseRejectedHeadStumpOrigins;
            builderReusePriorChunkOrigins += that.builderReusePriorChunkOrigins;
            builderReuseRawBeamOrigins += that.builderReuseRawBeamOrigins;
            builderReuseHeadOrigins += that.builderReuseHeadOrigins;
            builderReuseLedgerOrigins += that.builderReuseLedgerOrigins;
            builderReuseGridOrigins += that.builderReuseGridOrigins;
            builderReuseSigOtherOrigins += that.builderReuseSigOtherOrigins;
            builderReuseUnmodeledOrigins += that.builderReuseUnmodeledOrigins;
            builderChunkRemovals += that.builderChunkRemovals;
            builderChunkSeedDrops += that.builderChunkSeedDrops;
            builderChunkUnalignedDrops += that.builderChunkUnalignedDrops;
            builderChunkStartDrops += that.builderChunkStartDrops;
            builderSeedItemDrops += that.builderSeedItemDrops;
            builderSortRows += that.builderSortRows;
            builderSortAudits += that.builderSortAudits;
            builderSortCycles += that.builderSortCycles;
            builderSortEquivalenceInconsistencies += that.builderSortEquivalenceInconsistencies;
            builderMaxTargetSortItems = Math.max(
                    builderMaxTargetSortItems, that.builderMaxTargetSortItems);
            builderMaxFinalSortItems = Math.max(
                    builderMaxFinalSortItems, that.builderMaxFinalSortItems);
            builderSortListsAtLeast32 += that.builderSortListsAtLeast32;
            builderPreItems += that.builderPreItems;
            builderPreStartItems += that.builderPreStartItems;
            builderPreBItems += that.builderPreBItems;
            builderPreCItems += that.builderPreCItems;
            builderPreSeedItems += that.builderPreSeedItems;
            builderPreChunkItems += that.builderPreChunkItems;
            builderGapChecks += that.builderGapChecks;
            builderGapInserts += that.builderGapInserts;
            builderGapTruncations += that.builderGapTruncations;
            builderFinalItems += that.builderFinalItems;
            builderFinalStartItems += that.builderFinalStartItems;
            builderFinalBItems += that.builderFinalBItems;
            builderFinalCItems += that.builderFinalCItems;
            builderFinalSeedItems += that.builderFinalSeedItems;
            builderFinalChunkItems += that.builderFinalChunkItems;
            builderFinalGapItems += that.builderFinalGapItems;
            builderLengthRows += that.builderLengthRows;
        }

        void countPreKind (String kind)
        {
            if (kind.equals("startHalf")) builderPreStartItems++;
            else if (kind.equals("B")) builderPreBItems++;
            else if (kind.equals("C")) builderPreCItems++;
            else if (kind.equals("seed")) builderPreSeedItems++;
            else if (kind.equals("chunk")) builderPreChunkItems++;
            else throw new IllegalStateException("unknown pre-item kind " + kind);
        }

        void countReuseOrigin (String origin)
        {
            if (origin.contains("stemSeed:")) builderReuseSeedOrigins++;
            if (origin.contains("beamStump:")) builderReuseBeamStumpOrigins++;
            if (origin.contains("headStump:")) builderReuseHeadStumpOrigins++;
            if (origin.contains("rejectedHeadStump:")) builderReuseRejectedHeadStumpOrigins++;
            if (origin.contains("priorStemBuilderChunk:")) builderReusePriorChunkOrigins++;
            if (origin.contains("rawBeamOrHook:")) builderReuseRawBeamOrigins++;
            if (origin.contains("head:")) builderReuseHeadOrigins++;
            if (origin.contains("ledger:")) builderReuseLedgerOrigins++;
            if (origin.contains("grid:")) builderReuseGridOrigins++;
            if (origin.contains("sigOther:")) builderReuseSigOtherOrigins++;
            if (origin.contains("otherPreStems:")) builderReuseUnmodeledOrigins++;
        }

        void countFinalKind (String kind)
        {
            if (kind.equals("startHalf")) builderFinalStartItems++;
            else if (kind.equals("B")) builderFinalBItems++;
            else if (kind.equals("C")) builderFinalCItems++;
            else if (kind.equals("seed")) builderFinalSeedItems++;
            else if (kind.equals("chunk")) builderFinalChunkItems++;
            else if (kind.equals("gap")) builderFinalGapItems++;
            else throw new IllegalStateException("unknown final-item kind " + kind);
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
