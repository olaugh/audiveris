// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
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
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.GlyphIndex;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.glyph.dynamic.FilamentIndex;
import org.audiveris.omr.math.LineUtil;
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
import org.audiveris.omr.sheet.stem.StemItem;
import org.audiveris.omr.sheet.stem.StemLinker;
import org.audiveris.omr.sheet.stem.StemsRetriever;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.sig.relation.StemPortion;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact identity-free oracle for the pure beam-origin {@code VLinker.expand} boundary.
 *
 * <p>The page reaches real HEADS. This probe reproduces {@code inspectStems()} through every
 * beam-origin and head-origin {@link StemBuilder} assignment. It then evaluates every inspected
 * beam builder for {@code stemProfile=0..constructionMax} with the effective system link profile.
 * The real private {@code expand} is invoked reflectively and checked against a source-order replay.
 * The boundary stops at the last branch immediately before {@code StemBuilder.createStem}.</p>
 */
@SuppressWarnings("unchecked")
public final class StemsBeamExpandProbe
{
    private static final Constructor<?> PARAMETERS_CONSTRUCTOR;

    private static final Field RETRIEVER_PARAMS;

    private static final Field RETRIEVER_SYSTEM_SEEDS;

    private static final Field RETRIEVER_SYSTEM_BEAMS;

    private static final Field RETRIEVER_SYSTEM_HEADS;

    private static final Field RETRIEVER_STEM_CHECKER;

    private static final Field RETRIEVER_SYSTEM_STEMS;

    private static final Method PURGE_NO_STEM_SEEDS;

    private static final Field PARAM_MIN_LINKER_LENGTH;

    private static final Field LINKER_ALL_B;

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

    private static final Field C_V_SIDE;

    private static final Field C_STEM_BUILDER;

    private static final Method C_GET_HEAD;

    private static final Method C_GET_S_LINKER;

    private static final Method C_CHECK_STEM_RELATION;

    private static final Method C_HAS_CONCRETE_START;

    private static final Method S_GET_HORIZONTAL_SIDE;

    private static final Field STEM_BUILDER_ITEMS;

    private static final Field STEM_BUILDER_LENGTH_MAP;

    private static final Field STEM_BUILDER_THEO_LINE;

    private static final Field STEM_ITEM_LINE;

    private static final Field STEM_ITEM_GLYPH;

    private static final Field STEM_ITEM_CONTRIB;

    private static final Field LINKER_ITEM_LINKER;

    private static final Class<?> B_LINKER_CLASS;

    private static final Class<?> V_LINKER_CLASS;

    private static final Class<?> C_LINKER_CLASS;

    private static final Class<?> LINKER_ITEM_CLASS;

    static {
        try {
            final Class<?> parameters = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemsRetriever$Parameters");
            B_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker");
            V_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.BeamLinker$BLinker$VLinker");
            C_LINKER_CLASS = Class.forName(
                    "org.audiveris.omr.sheet.stem.HeadLinker$SLinker$CLinker");
            final Class<?> sLinker = Class.forName(
                    "org.audiveris.omr.sheet.stem.HeadLinker$SLinker");
            LINKER_ITEM_CLASS = Class.forName(
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
            PARAM_MIN_LINKER_LENGTH = declaredField(parameters, "minLinkerLength");

            LINKER_ALL_B = declaredField(BeamLinker.class, "allBLinkers");
            B_ID = declaredField(B_LINKER_CLASS, "id");
            B_H_SIDE = declaredField(B_LINKER_CLASS, "hSide");
            B_STUMP = declaredField(B_LINKER_CLASS, "stump");
            B_IS_ANCHOR = declaredField(B_LINKER_CLASS, "isAnchor");
            B_V_LINKERS = declaredField(B_LINKER_CLASS, "vLinkers");
            V_V_SIDE = declaredField(V_LINKER_CLASS, "vSide");
            V_Y_DIR = declaredField(V_LINKER_CLASS, "yDir");
            V_THEO_LINE = declaredField(V_LINKER_CLASS, "theoLine");
            V_STEM_BUILDER = declaredField(V_LINKER_CLASS, "sb");
            V_EXPAND = V_LINKER_CLASS.getDeclaredMethod(
                    "expand", int.class, int.class, Map.class, Set.class);
            V_EXPAND.setAccessible(true);

            C_V_SIDE = declaredField(C_LINKER_CLASS, "vSide");
            C_STEM_BUILDER = declaredField(C_LINKER_CLASS, "sb");
            C_GET_HEAD = C_LINKER_CLASS.getDeclaredMethod("getHead");
            C_GET_HEAD.setAccessible(true);
            C_GET_S_LINKER = C_LINKER_CLASS.getDeclaredMethod("getSLinker");
            C_GET_S_LINKER.setAccessible(true);
            C_CHECK_STEM_RELATION = C_LINKER_CLASS.getDeclaredMethod(
                    "checkStemRelation", Line2D.class, int.class);
            C_CHECK_STEM_RELATION.setAccessible(true);
            C_HAS_CONCRETE_START = C_LINKER_CLASS.getDeclaredMethod(
                    "hasConcreteStart", int.class);
            C_HAS_CONCRETE_START.setAccessible(true);
            S_GET_HORIZONTAL_SIDE = sLinker.getDeclaredMethod("getHorizontalSide");
            S_GET_HORIZONTAL_SIDE.setAccessible(true);

            STEM_BUILDER_ITEMS = declaredField(StemBuilder.class, "items");
            STEM_BUILDER_LENGTH_MAP = declaredField(StemBuilder.class, "lengthMap");
            STEM_BUILDER_THEO_LINE = declaredField(StemBuilder.class, "theoLine");
            STEM_ITEM_LINE = declaredField(StemItem.class, "line");
            STEM_ITEM_GLYPH = declaredField(StemItem.class, "glyph");
            STEM_ITEM_CONTRIB = declaredField(StemItem.class, "contrib");
            LINKER_ITEM_LINKER = declaredField(LINKER_ITEM_CLASS, "linker");
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemsBeamExpandProbe ()
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
                "stemsbeamexpandpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily());
        for (SystemInfo system : sheet.getSystems()) {
            runSystem(page, sheet, system, totals, hash);
        }
        System.out.printf(
                "stemsbeamexpandpagesummary %s systems %d %s hash %016x%n",
                page, sheet.getSystems().size(), totals.fields(), hash.value());
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

        final List<Glyph> seeds = new ArrayList<>(
                system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED));
        PURGE_NO_STEM_SEEDS.invoke(retriever, seeds);
        RETRIEVER_SYSTEM_SEEDS.set(retriever, seeds);

        final List<Inter> beamSigOrder = system.getSig().inters(AbstractBeamInter.class);
        final IdentityHashMap<Inter, Integer> beamSigOrdinals = identityOrdinals(beamSigOrder);
        final List<Inter> beams = new ArrayList<>(beamSigOrder);
        Collections.sort(beams, Inters.byAbscissa);
        RETRIEVER_SYSTEM_BEAMS.set(retriever, beams);
        for (Iterator<Inter> iterator = beams.iterator(); iterator.hasNext();) {
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

        final List<Inter> headSigOrder = system.getSig().inters(
                ShapeSet.getTemplateNotesStem(sheet));
        final List<Inter> heads = new ArrayList<>(headSigOrder);
        Collections.sort(heads, Inters.byAbscissa);
        RETRIEVER_SYSTEM_HEADS.set(retriever, heads);
        final IdentityHashMap<Object, String> cAliases = new IdentityHashMap<>();
        for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
            final HeadInter head = (HeadInter) heads.get(headOrdinal);
            if (head.getLinker() != null) {
                throw new IllegalStateException("HEADS head already has linker");
            }
            head.setLinker(new HeadLinker(head, retriever));
            for (HorizontalSide hSide : HorizontalSide.values()) {
                for (VerticalSide vSide : VerticalSide.values()) {
                    final Object c = head.getLinker().getCornerLinker(hSide, vSide);
                    if (cAliases.put(c, cToken(headOrdinal, hSide, vSide)) != null) {
                        throw new IllegalStateException("duplicate CLinker identity");
                    }
                }
            }
        }

        // Exact inspectStems chronology: all beam builders, then all head builders.
        for (Inter inter : beams) {
            ((AbstractBeamInter) inter).getLinker().inspectVLinkers();
        }
        for (Inter inter : heads) {
            ((HeadInter) inter).getLinker().inspectCLinkers();
        }

        final IdentityHashMap<Object, String> bAliases = new IdentityHashMap<>();
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                final Object b = allB.get(bOrdinal);
                bAliases.put(b, bToken(beamSigOrdinals.get(beam), bOrdinal));
            }
        }

        for (Object c : cAliases.keySet()) {
            if (C_STEM_BUILDER.get(c) == null) {
                throw new IllegalStateException("missing C builder at expand checkpoint");
            }
        }

        final int systemProfile = system.getProfile();
        final int stubProfile = sheet.getStub().getProfile();
        if (systemProfile != stubProfile) {
            throw new IllegalStateException("system/stub profile divergence");
        }
        final int minLinkerLength = PARAM_MIN_LINKER_LENGTH.getInt(params);
        emit(String.format(
                "stemsbeamexpandsystem %s system %d profile %d stubProfile %d "
                        + "interline %d minLinkerLength %d beams %d heads %d "
                        + "beamSigOrder %s",
                page, system.getId(), systemProfile, stubProfile,
                sheet.getScale().getInterline(), minLinkerLength, beams.size(), heads.size(),
                ordinalToken(beams, beamSigOrdinals)), hash, pageHash);

        final StateSnapshot systemBaseline = snapshot(
                sheet, system, retriever, beams, bAliases, cAliases);
        int builderOrdinal = 0;
        int planOrdinal = 0;
        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            for (int bOrdinal = 0; bOrdinal < allB.size(); bOrdinal++) {
                final Object b = allB.get(bOrdinal);
                if (B_IS_ANCHOR.getBoolean(b)) continue;
                final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
                final int constructionMax = hSide != null
                        ? Profiles.BEAM_SIDE : Profiles.BEAM_SEED;
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    final Object v = entry.getValue();
                    final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
                    if (builder == null) {
                        throw new IllegalStateException("missing beam builder at expand checkpoint");
                    }
                    for (int stemProfile = 0; stemProfile <= constructionMax; stemProfile++) {
                        runPlan(
                                page, sheet, system, retriever, beam, b, bOrdinal, v,
                                builder, builderOrdinal, planOrdinal++, constructionMax,
                                stemProfile, systemProfile,
                                minLinkerLength, bAliases, cAliases, totals, hash, pageHash);
                    }
                    builderOrdinal++;
                }
            }
        }

        final StateSnapshot systemAfter = snapshot(
                sheet, system, retriever, beams, bAliases, cAliases);
        systemBaseline.assertSame(systemAfter, "system expand matrix");
        totals.systems++;
        totals.builders += builderOrdinal;
        emit(String.format(
                "stemsbeamexpandsystemsummary %s system %d %s hash %016x",
                page, system.getId(), totals.fields(), hash.value()), pageHash);
        pageTotals.include(totals);
    }

    private static void runPlan (String page,
                                 Sheet sheet,
                                 SystemInfo system,
                                 StemsRetriever retriever,
                                 AbstractBeamInter beam,
                                 Object b,
                                 int bOrdinal,
                                 Object v,
                                 StemBuilder builder,
                                 int builderOrdinal,
                                 int planOrdinal,
                                 int constructionMax,
                                 int stemProfile,
                                 int linkProfile,
                                 int minLinkerLength,
                                 IdentityHashMap<Object, String> bAliases,
                                 IdentityHashMap<Object, String> cAliases,
                                 Totals totals,
                                 RowHasher... hashes)
        throws Exception
    {
        final List<StemItem> items = (List<StemItem>) STEM_BUILDER_ITEMS.get(builder);
        final List<Object> headTargets = cLinkers(items);
        final VerticalSide vSide = (VerticalSide) V_V_SIDE.get(v);
        final int yDir = V_Y_DIR.getInt(v);
        final HorizontalSide hSide = (HorizontalSide) B_H_SIDE.get(b);
        final Glyph stump = (Glyph) B_STUMP.get(b);
        final Line2D storedTheo = (Line2D) V_THEO_LINE.get(v);
        final Line2D theo = new Line2D.Double(storedTheo.getP1(), storedTheo.getP2());
        final Object theoAttachment = beam.getAttachments().get("theo-" + B_ID.getInt(b));
        final boolean attachmentAliasesTheo = theoAttachment == storedTheo;
        final boolean builderAliasesTheo = STEM_BUILDER_THEO_LINE.get(builder) == storedTheo;
        if (!builderAliasesTheo) {
            throw new IllegalStateException("VLinker/StemBuilder theoretical line alias missing");
        }
        if (yDir > 0 && !attachmentAliasesTheo) {
            throw new IllegalStateException("downward VLinker/beam attachment line alias missing");
        }
        final int initialGap = sheet.getScale().toPixels(StemChecker.getMaxYGap(stemProfile));
        final int standardGap = sheet.getScale().toPixels(
                StemChecker.getMaxYGap(Profiles.STANDARD));
        final StateSnapshot before = snapshotOnePlan(
                sheet, system, retriever, beam, bAliases, cAliases, builder);

        emit(String.format(
                "stemsbeamexpandplan %s system %d plan %d builder %d beamSig %s "
                        + "bOrdinal %d bAlias %s bId %d hSide %s "
                        + "vSide %s yDir %d constructionMax %d "
                        + "stemProfile %d linkProfile %d "
                        + "items %d maxIndex %d headTargets %s targetCount %d "
                        + "startStump %s theo %s initialMaxYGap %d standardMaxYGap %d "
                        + "minLinkerLength %d",
                page, system.getId(), planOrdinal, builderOrdinal,
                beamSigToken(beam, bAliases, b), bOrdinal, bAliases.get(b), B_ID.getInt(b),
                hSide != null ? hSide : "-", vSide, yDir,
                constructionMax, stemProfile, linkProfile, items.size(), items.size() - 1,
                linkerTokens(headTargets, cAliases), headTargets.size(), glyphAlias(stump),
                line(theo), initialGap, standardGap, minLinkerLength), hashes);

        final LinkedHashMap<Object, Relation> actualRelations = new LinkedHashMap<>();
        final LinkedHashSet<Glyph> actualGlyphs = new LinkedHashSet<>();
        final ReplayResult replay;
        final int actualLastIndex;
        Line2D actualStoredTheoAfter = new Line2D.Double(theo.getP1(), theo.getP2());
        if (headTargets.isEmpty()) {
            actualLastIndex = -2; // VLinker.link returns before invoking expand.
            replay = ReplayResult.noHeadTarget(orientedLine(theo, yDir));
            totals.noHeadTarget++;
        } else {
            try {
                actualLastIndex = (Integer) V_EXPAND.invoke(
                        v, stemProfile, linkProfile, actualRelations, actualGlyphs);
                actualStoredTheoAfter = new Line2D.Double(
                        storedTheo.getP1(), storedTheo.getP2());
                final boolean storedTheoMutated = !sameLineBits(theo, actualStoredTheoAfter);
                if (storedTheoMutated) totals.storedTheoMutations++;
                if (storedTheoMutated && attachmentAliasesTheo) totals.attachmentLineMutations++;
            } finally {
                // Matrix entries are independent variants from the same live inspect checkpoint.
                storedTheo.setLine(theo);
            }
            replay = replayExpand(
                    page, sheet, system, builder, planOrdinal, vSide, yDir, theo,
                    stemProfile, linkProfile, minLinkerLength, bAliases, cAliases, totals,
                    hashes);
            if (yDir > 0 && !sameLineBits(actualStoredTheoAfter, replay.finalStemLine)) {
                throw new IllegalStateException("downward stored theoLine differs from replay");
            }
            if (yDir < 0 && !sameLineBits(actualStoredTheoAfter, theo)) {
                throw new IllegalStateException("upward expand mutated stored theoLine");
            }
            assertExpandResult(
                    actualLastIndex, actualRelations, actualGlyphs, replay, cAliases);
        }

        final boolean storedTheoMutated = !headTargets.isEmpty() && yDir > 0
                && !sameLineBits(theo, actualStoredTheoAfter);
        if (!headTargets.isEmpty() && yDir < 0 && !sameLineBits(storedTheo, theo)) {
            throw new IllegalStateException("upward expand unexpectedly mutated stored theoLine");
        }
        final double storedTheoShiftDx = actualStoredTheoAfter.getX1() - theo.getX1();
        if (storedTheoMutated) {
            if (!sameDouble(actualStoredTheoAfter.getY1(), theo.getY1())
                    || !sameDouble(actualStoredTheoAfter.getY2(), theo.getY2())
                    || !sameDouble(
                            actualStoredTheoAfter.getX2() - theo.getX2(), storedTheoShiftDx)) {
                throw new IllegalStateException("stored theoLine mutation is not a pure x shift");
            }
            totals.maxAbsStoredTheoShift = Math.max(
                    totals.maxAbsStoredTheoShift, Math.abs(storedTheoShiftDx));
        }

        final String outcome;
        if (headTargets.isEmpty()) {
            outcome = "NoHeadTarget";
        } else if (actualLastIndex == -1) {
            outcome = "ExpandFailed";
            totals.expandFailed++;
        } else if (actualRelations.isEmpty()) {
            outcome = "NoRelations";
            totals.noRelations++;
        } else if (actualGlyphs.isEmpty()) {
            outcome = "NoGlyphs";
            totals.noGlyphs++;
        } else {
            outcome = "ReadyForCreateStem";
            totals.ready++;
        }
        final HorizontalSide stoppingSide = yDir < 0
                ? HorizontalSide.LEFT : HorizontalSide.RIGHT;
        String terminalKind = "-";
        String terminalC = "-";
        String terminalRelation = "-";
        String terminalPortion = "-";
        boolean terminalCorrectSideEnd = false;
        if (actualLastIndex >= 0 && actualLastIndex < items.size()) {
            final StemItem terminal = items.get(actualLastIndex);
            terminalKind = itemKind(terminal);
            if (LINKER_ITEM_CLASS.isInstance(terminal)) {
                final Object linker = LINKER_ITEM_LINKER.get(terminal);
                if (C_LINKER_CLASS.isInstance(linker)) {
                    terminalC = cAliases.get(linker);
                    final HeadStemRelation relation =
                            (HeadStemRelation) actualRelations.get(linker);
                    if (relation != null) {
                        terminalRelation = relation.getHeadSide().toString();
                        terminalCorrectSideEnd = replay.stoppingIndex == actualLastIndex;
                        if (terminalCorrectSideEnd) {
                            terminalPortion = (yDir > 0
                                    ? StemPortion.STEM_BOTTOM : StemPortion.STEM_TOP).toString();
                        }
                    }
                }
            }
        }
        if (outcome.equals("ReadyForCreateStem") && stemProfile == Profiles.BEAM_SIDE) {
            totals.beamSideReady++;
            if (replay.stoppingUpdates == 0) {
                totals.beamSideReadyWithoutStoppingHead++;
            } else if (replay.stoppingIndex != actualLastIndex) {
                totals.beamSideReadyBeyondStoppingHead++;
            } else {
                totals.beamSideReadyAtStoppingHead++;
            }
        }

        int glyphOrdinal = 0;
        for (Glyph glyph : actualGlyphs) {
            final Integer sourceItem = replay.insertionItems.get(glyph);
            if (sourceItem == null) {
                throw new IllegalStateException("expanded glyph has no structural item source");
            }
            emit(String.format(
                    "stemsbeamexpandglyph %s system %d plan %d ordinal %d "
                            + "sourceItem %d sourceRef %s content %s bounds %s weight %d "
                            + "centroid %s centerLine %s",
                    page, system.getId(), planOrdinal, glyphOrdinal++, sourceItem,
                    itemRef(items.get(sourceItem), sourceItem, bAliases, cAliases),
                    glyphAlias(glyph), rectangle(glyph.getBounds()), glyph.getWeight(),
                    point(glyph.getCentroidDouble()), line(glyph.getCenterLine())), hashes);
        }

        int relationOrdinal = 0;
        for (Map.Entry<Object, Relation> entry : actualRelations.entrySet()) {
            final HeadStemRelation relation = (HeadStemRelation) entry.getValue();
            final Integer firstItem = replay.relationFirstItemIndices.get(entry.getKey());
            final Integer latestItem = replay.relationItemIndices.get(entry.getKey());
            emit(String.format(
                    "stemsbeamexpandfinalrelation %s system %d plan %d ordinal %d "
                            + "cAlias %s firstItem %s latestItem %s pastReturn %s headSide %s dx %s "
                            + "dy %s grade %s extension %s impacts %s",
                    page, system.getId(), planOrdinal, relationOrdinal++,
                    cAliases.get(entry.getKey()), firstItem != null ? firstItem : "-",
                    latestItem != null ? latestItem : "-",
                    latestItem != null && actualLastIndex >= 0 && latestItem > actualLastIndex,
                    relation.getHeadSide(), hexDouble(relation.getDx()),
                    hexDouble(relation.getDy()), hexDouble(relation.getGrade()),
                    point(relation.getExtensionPoint()), impactNames(relation.getImpacts())
                            + ":" + impactValues(relation.getImpacts())
                            + ":" + impactWeights(relation.getImpacts())), hashes);
        }

        final List<String> postReturnRelations = new ArrayList<>();
        for (Object linker : actualRelations.keySet()) {
            final Integer latest = replay.relationItemIndices.get(linker);
            final int itemIndex = latest != null ? latest : itemIndexOfLinker(items, linker);
            if (actualLastIndex >= 0 && itemIndex > actualLastIndex) {
                postReturnRelations.add(cAliases.get(linker) + "@" + itemIndex);
            }
        }
        totals.postReturnRelations += postReturnRelations.size();
        if (!postReturnRelations.isEmpty()) totals.plansWithPostReturnRelations++;

        final Line2D restoredGlyphLine = orientedLine(theo, yDir);
        final LinkedHashSet<Glyph> restoredReplay = new LinkedHashSet<>();
        for (Glyph glyph : actualGlyphs) {
            updateStemLine(glyph, restoredReplay, restoredGlyphLine);
        }
        final boolean rollbackLineDiverges = !sameLineBits(replay.finalStemLine, restoredGlyphLine);
        if (actualLastIndex >= 0 && actualLastIndex < items.size() - 1 && rollbackLineDiverges) {
            totals.rollbackLineDivergences++;
        }

        final StateSnapshot after = snapshotOnePlan(
                sheet, system, retriever, beam, bAliases, cAliases, builder);
        before.assertSame(after, "expand plan " + planOrdinal);
        emit(String.format(
                "stemsbeamexpandend %s system %d plan %d builder %d outcome %s "
                        + "expandInvoked %s lastIndex %s relationCount %d relations %s "
                        + "glyphCount %d glyphs %s stoppingIndex %s stopCause %s "
                        + "relationsPastReturn %s storedTheoBefore %s storedTheoAfter %s "
                        + "storedTheoMutated %s attachmentAliasesTheo %s builderAliasesTheo %s "
                        + "attachmentLineMutated %s storedTheoShiftDx %s "
                        + "terminalKind %s terminalC %s terminalRelationSide %s "
                        + "terminalPortion %s terminalCorrectSideEnd %s "
                        + "beamSideReadyWithoutStoppingHead %s "
                        + "finalStemLine %s restoredGlyphLine %s "
                        + "rollbackLineDiverges %s traceSha256 %s visits %d gaps %d "
                        + "showStoppingGaps %d separationChecks %d separationStops %d "
                        + "relationAttempts %d relationAccepts %d relationRejects %d "
                        + "stoppingUpdates %d glyphUpdateCalls %d glyphInsertions %d "
                        + "glyphEqualSkips %d sigVertexDelta 0 sigEdgeDelta 0 "
                        + "stemInterDelta 0 systemStemDelta 0 glyphIndexDelta 0 "
                        + "filamentIndexDelta 0 linkMutations 0 builderMutations 0",
                page, system.getId(), planOrdinal, builderOrdinal, outcome,
                !headTargets.isEmpty(), headTargets.isEmpty() ? "-" : actualLastIndex,
                actualRelations.size(), relationTokens(actualRelations.keySet(), cAliases),
                actualGlyphs.size(), glyphAliases(actualGlyphs),
                replay.stoppingIndex >= 0 ? replay.stoppingIndex : "-", replay.stopCause,
                listToken(postReturnRelations), line(theo), line(actualStoredTheoAfter),
                storedTheoMutated, attachmentAliasesTheo, builderAliasesTheo,
                storedTheoMutated && attachmentAliasesTheo, hexDouble(storedTheoShiftDx),
                terminalKind, terminalC, terminalRelation, terminalPortion,
                terminalCorrectSideEnd,
                outcome.equals("ReadyForCreateStem")
                        && stemProfile == Profiles.BEAM_SIDE
                        && replay.stoppingUpdates == 0,
                line(replay.finalStemLine),
                line(restoredGlyphLine), rollbackLineDiverges, replay.traceSha256,
                replay.visits, replay.gaps, replay.showStoppingGaps,
                replay.separationChecks, replay.separationStops,
                replay.relationAttempts, replay.relationAccepts, replay.relationRejects,
                replay.stoppingUpdates, replay.glyphUpdateCalls, replay.glyphInsertions,
                replay.glyphEqualSkips), hashes);
        totals.plans++;
        totals.relations += actualRelations.size();
        totals.glyphs += actualGlyphs.size();
    }

    private static ReplayResult replayExpand (String page,
                                              Sheet sheet,
                                              SystemInfo system,
                                              StemBuilder builder,
                                              int planOrdinal,
                                              VerticalSide beamVSide,
                                              int yDir,
                                              Line2D theo,
                                              int stemProfile,
                                              int linkProfile,
                                              int minLinkerLength,
                                              IdentityHashMap<Object, String> bAliases,
                                              IdentityHashMap<Object, String> cAliases,
                                              Totals totals,
                                              RowHasher... hashes)
        throws Exception
    {
        final List<StemItem> items = (List<StemItem>) STEM_BUILDER_ITEMS.get(builder);
        final int maxIndex = items.size() - 1;
        int maxYGap = sheet.getScale().toPixels(StemChecker.getMaxYGap(stemProfile));
        final Line2D stemLine = orientedLine(theo, yDir);
        StemItem stoppingHeadItem = null;
        int stoppingIndex = -1;
        LinkedHashSet<Glyph> stoppingGlyphs = null;
        IdentityHashMap<Glyph, Integer> stoppingInsertionItems = null;
        final LinkedHashMap<Object, HeadStemRelation> relations = new LinkedHashMap<>();
        final IdentityHashMap<Object, Integer> relationItemIndices = new IdentityHashMap<>();
        final IdentityHashMap<Object, Integer> relationFirstItemIndices = new IdentityHashMap<>();
        final LinkedHashSet<Glyph> glyphs = new LinkedHashSet<>();
        final IdentityHashMap<Glyph, Integer> insertionItems = new IdentityHashMap<>();
        final ReplayResult result = new ReplayResult(
                stemLine, relations, relationItemIndices, relationFirstItemIndices, glyphs,
                insertionItems);
        final MessageDigest trace = sha256Digest();

        for (int i = 0; i <= maxIndex; i++) {
            final StemItem item = items.get(i);
            result.visits++;
            trace(trace, "visit " + i + " " + itemKind(item) + "\n");
            if (item instanceof StemItem.GapItem) {
                result.gaps++;
            }

            final int contrib = STEM_ITEM_CONTRIB.getInt(item);
            if (item instanceof StemItem.GapItem) {
                final boolean showStopping = contrib > maxYGap;
                final String gapAction = !showStopping ? "continue"
                        : stoppingHeadItem == null ? "fail" : "rewind";
                emit(String.format(
                        "stemsbeamexpandgap %s system %d plan %d item %d contrib %d "
                                + "maxYGap %d stoppingIndex %s action %s glyphsBefore %s "
                                + "restoredGlyphs %s relationsRetained %s stemLineRetained %s",
                        page, system.getId(), planOrdinal, i, contrib, maxYGap,
                        stoppingIndex >= 0 ? stoppingIndex : "-", gapAction,
                        glyphAliases(glyphs), stoppingGlyphs != null
                                ? glyphAliases(stoppingGlyphs) : "-",
                        relationTokens(relations.keySet(), cAliases), line(stemLine)), hashes);
            }
            if ((item instanceof StemItem.GapItem) && (contrib > maxYGap)) {
                result.showStoppingGaps++;
                final String action = stoppingHeadItem == null ? "fail" : "rollback";
                trace(trace, "gapstop " + i + " " + contrib + " " + maxYGap + " " + action + "\n");
                if (stoppingHeadItem == null) {
                    return result.finish(-1, -1, "ShowStoppingGapBeforeStoppingHead", trace);
                }
                glyphs.clear();
                glyphs.addAll(stoppingGlyphs);
                insertionItems.clear();
                insertionItems.putAll(stoppingInsertionItems);
                return result.finish(
                        stoppingIndex, stoppingIndex, "ShowStoppingGapRollback", trace);
            } else if (LINKER_ITEM_CLASS.isInstance(item)) {
                final Object linker = LINKER_ITEM_LINKER.get(item);
                if (C_LINKER_CLASS.isInstance(linker)) {
                    final Object c = linker;
                    final HeadInter head = (HeadInter) C_GET_HEAD.invoke(c);

                    if (stoppingHeadItem != null) {
                        result.separationChecks++;
                        final StemItem gap = lastGapBefore(items, i);
                        final int gapIndex = identityIndex(items, gap);
                        double closeDy = Double.NaN;
                        boolean underMin = false;
                        Object opposite = null;
                        boolean oppositeConcrete = false;
                        int oppositeLength = -1;
                        if (gap != null) {
                            final double y = ((StemLinker) c).getReferencePoint().getY();
                            final Line2D gapLine = (Line2D) STEM_ITEM_LINE.get(gap);
                            closeDy = yDir > 0 ? y - gapLine.getY2() : gapLine.getY1() - y;
                            underMin = closeDy < minLinkerLength;
                            if (underMin) {
                                final HorizontalSide storedH = storedHorizontalSide(c);
                                opposite = head.getLinker().getCornerLinker(
                                        storedH.opposite(), beamVSide);
                                oppositeConcrete = (Boolean) C_HAS_CONCRETE_START.invoke(
                                        opposite, linkProfile);
                                oppositeLength = ((StemBuilder) C_STEM_BUILDER.get(opposite))
                                        .getLength(linkProfile);
                            }
                        }
                        emit(String.format(
                                "stemsbeamexpandseparation %s system %d plan %d item %d "
                                        + "candidate %s stoppingIndex %d gapIndex %s closeDy %s "
                                        + "minLinkerLength %d underMin %s opposite %s "
                                        + "oppositeLength %s oppositeConcrete %s action %s",
                                page, system.getId(), planOrdinal, i, cAliases.get(c),
                                stoppingIndex, gapIndex >= 0 ? gapIndex : "-",
                                gap != null ? hexDouble(closeDy) : "-", minLinkerLength,
                                gap != null ? underMin : "-",
                                opposite != null ? cAliases.get(opposite) : "-",
                                opposite != null ? oppositeLength : "-",
                                opposite != null ? oppositeConcrete : "-",
                                oppositeConcrete ? "rollback" : "continue"), hashes);
                        trace(trace, "separation " + i + " " + gapIndex + " "
                                + (gap != null ? Double.doubleToLongBits(closeDy) : "-") + " "
                                + oppositeConcrete + "\n");
                        if (oppositeConcrete) {
                            result.separationStops++;
                            glyphs.clear();
                            glyphs.addAll(stoppingGlyphs);
                            insertionItems.clear();
                            insertionItems.putAll(stoppingInsertionItems);
                            return result.finish(
                                    stoppingIndex, stoppingIndex, "SeparatedHeadRollback", trace);
                        }
                    }

                    result.relationAttempts++;
                    final RelationEvidence evidence = relationEvidence(
                            sheet.getScale(), c, stemLine, linkProfile);
                    if (evidence.storedH != evidence.derivedH) {
                        totals.relationSideMismatches++;
                    }
                    final HeadStemRelation relation = (HeadStemRelation) C_CHECK_STEM_RELATION.invoke(
                            c, stemLine, linkProfile);
                    evidence.assertRelation(relation);
                    if (relation == null) {
                        result.relationRejects++;
                        emitRelation(
                                page, system, planOrdinal, i, c, evidence, null,
                                HorizontalSide.valueOf(yDir < 0 ? "LEFT" : "RIGHT"),
                                glyphs, null, null, false, false, "none", -1, -1, -1,
                                cAliases, hashes);
                        trace(trace, "relation " + i + " reject " + evidence.digestToken() + "\n");
                        continue;
                    }

                    result.relationAccepts++;
                    final boolean replaces = relations.containsKey(c);
                    final int mapOrdinal = replaces
                            ? identityIndex(new ArrayList<>(relations.keySet()), c)
                            : relations.size();
                    relations.put(c, relation);
                    relationFirstItemIndices.putIfAbsent(c, i);
                    relationItemIndices.put(c, i);
                    final HorizontalSide stoppingSide = yDir < 0
                            ? HorizontalSide.LEFT : HorizontalSide.RIGHT;
                    StemPortion portion = null;
                    Line2D compositeLine = null;
                    boolean isEnd = false;
                    boolean stoppingUpdate = false;
                    if ((relation.getHeadSide() == stoppingSide) && !glyphs.isEmpty()) {
                        final Glyph composite = glyphs.size() > 1
                                ? GlyphFactory.buildGlyph(glyphs) : glyphs.iterator().next();
                        compositeLine = composite.getCenterLine();
                        portion = relation.getStemPortion(head, compositeLine, sheet.getScale());
                        isEnd = portion == (yDir > 0
                                ? StemPortion.STEM_BOTTOM : StemPortion.STEM_TOP);
                        if (isEnd) {
                            stoppingHeadItem = item;
                            stoppingIndex = i;
                            // Source-faithful: snapshot happens before this C item's stump update.
                            stoppingGlyphs = new LinkedHashSet<>(glyphs);
                            stoppingInsertionItems = new IdentityHashMap<>(insertionItems);
                            maxYGap = sheet.getScale().toPixels(
                                    StemChecker.getMaxYGap(Profiles.STANDARD));
                            result.stoppingUpdates++;
                            stoppingUpdate = true;
                        }
                    }

                    emitRelation(
                            page, system, planOrdinal, i, c, evidence, relation, stoppingSide,
                            glyphs, compositeLine, portion, isEnd, stoppingUpdate,
                            replaces ? "replace" : "new", mapOrdinal,
                            relationFirstItemIndices.get(c), relationItemIndices.get(c), cAliases,
                            hashes);
                    trace(trace, "relation " + i + " accept " + evidence.digestToken()
                            + " stop=" + stoppingUpdate + "\n");
                }
            }

            final UpdateEvidence update = updateStemLineEvidence(
                    items, i, glyphs, insertionItems, stemLine, bAliases, cAliases);
            result.glyphUpdateCalls++;
            if (update.inserted) result.glyphInsertions++;
            if (update.equalSkip) result.glyphEqualSkips++;
            emit(String.format(
                    "stemsbeamexpandupdate %s system %d plan %d item %d itemRef %s "
                            + "attempted %s canonicalBefore %s action %s glyphsBefore %s "
                            + "glyphsAfter %s lineBefore %s lineAfter %s compositeBounds %s "
                            + "compositeWeight %s compositeRuns %s compositeCentroid %s "
                            + "intersection %s lineShift %s",
                    page, system.getId(), planOrdinal, i,
                    itemRef(items.get(i), i, bAliases, cAliases), update.attempted,
                    update.canonicalBefore, update.action, update.glyphsBefore,
                    update.glyphsAfter, line(update.lineBefore), line(update.lineAfter),
                    update.composite != null ? rectangle(update.composite.getBounds()) : "-",
                    update.composite != null ? update.composite.getWeight() : "-",
                    update.composite != null ? glyphRunSha256(update.composite) : "-",
                    update.composite != null ? point(update.composite.getCentroidDouble()) : "-",
                    update.intersection != null ? point(update.intersection) : "-",
                    update.inserted ? hexDouble(update.dx) : "-"), hashes);
            trace(trace, update.traceToken() + "\n");
        }

        return result.finish(maxIndex, stoppingIndex, "ExhaustedItems", trace);
    }

    private static RelationEvidence relationEvidence (Scale scale,
                                                      Object c,
                                                      Line2D stemLine,
                                                      int profile)
        throws Exception
    {
        final HeadInter head = (HeadInter) C_GET_HEAD.invoke(c);
        final VerticalSide cVSide = (VerticalSide) C_V_SIDE.get(c);
        final HorizontalSide storedH = storedHorizontalSide(c);
        final int yDir = cVSide.direction();
        final int xDir = -stemLine.relativeCCW(head.getCenter());
        final HorizontalSide derivedH = xDir < 0
                ? HorizontalSide.LEFT : HorizontalSide.RIGHT;
        final Point2D ref = head.getStemReferencePoint(derivedH, cVSide);
        final double xStem = LineUtil.xAtY(stemLine, ref.getY());
        final double xGapPixels = xDir * (xStem - ref.getX());
        final double yGapPixels;
        final Glyph stump = ((StemLinker) c).getStump();
        if (stump != null) {
            final Rectangle stumpBox = stump.getBounds();
            final double overlap = yDir > 0
                    ? stumpBox.y + stumpBox.height - stemLine.getY1()
                    : stemLine.getY2() - stumpBox.y;
            yGapPixels = Math.abs(Math.min(overlap, 0));
        } else if (ref.getY() < stemLine.getY1()) {
            yGapPixels = stemLine.getY1() - ref.getY();
        } else if (ref.getY() > stemLine.getY2()) {
            yGapPixels = ref.getY() - stemLine.getY2();
        } else {
            yGapPixels = 0;
        }

        final double dx = scale.pixelsToFrac(xGapPixels);
        final double dy = scale.pixelsToFrac(yGapPixels);
        final boolean out = dx >= 0;
        final double xMax = out
                ? HeadStemRelation.getXOutGapMaximum(profile).getValue()
                : HeadStemRelation.getXInGapMaximum(profile).getValue();
        final double yMax = HeadStemRelation.getYGapMaximum(profile).getValue();
        final double xImpactRaw = out ? (xMax - dx) / xMax : (xMax + dx) / xMax;
        final double yImpactRaw = (yMax - dy) / yMax;
        final double xImpact = clamp(xImpactRaw);
        final double yImpact = clamp(yImpactRaw);
        final double xWeight = out ? 2.0 : 1.0;
        final double yWeight = 1.0;
        double global = 1.0;
        double totalWeight = 0.0;
        final double[] impactValues = { xImpact, yImpact };
        final double[] weights = { xWeight, yWeight };
        for (int index = 0; index < impactValues.length; index++) {
            final double weight = weights[index];
            final double impact = impactValues[index];
            totalWeight += weight;
            if (impact == 0) {
                global = 0;
            } else if (weight != 0) {
                global *= Math.pow(impact, weight);
            }
        }
        final double grade = Math.pow(global, 1.0 / totalWeight);
        final double minGrade = new HeadStemRelation().getMinGrade();
        final boolean accepted = grade >= minGrade;
        final Point2D extension = accepted
                ? new Point2D.Double(
                        xStem,
                        yDir > 0 ? head.getBounds().y
                                : head.getBounds().y + head.getBounds().height - 1)
                : null;
        return new RelationEvidence(
                head, cVSide, storedH, xDir, derivedH, new Line2D.Double(stemLine.getP1(),
                        stemLine.getP2()), ref, xStem, xGapPixels, yGapPixels, dx, dy, out,
                xMax, yMax, xImpactRaw, yImpactRaw, xImpact, yImpact, xWeight, yWeight,
                grade, minGrade, accepted, extension);
    }

    private static void emitRelation (String page,
                                      SystemInfo system,
                                      int planOrdinal,
                                      int itemIndex,
                                      Object c,
                                      RelationEvidence evidence,
                                      HeadStemRelation relation,
                                      HorizontalSide stoppingSide,
                                      Collection<Glyph> glyphsBefore,
                                      Line2D compositeLine,
                                      StemPortion portion,
                                      boolean isEnd,
                                      boolean stoppingUpdate,
                                      String mapAction,
                                      int mapOrdinal,
                                      int firstItem,
                                      int latestItem,
                                      IdentityHashMap<Object, String> cAliases,
                                      RowHasher... hashes)
    {
        final GradeImpacts impacts = relation != null ? relation.getImpacts() : null;
        emit(String.format(
                "stemsbeamexpandrelation %s system %d plan %d item %d cAlias %s "
                        + "headShape %s headBounds %s headCenter %d:%d cHSide %s cVSide %s "
                        + "stemLine %s relativeCCW %d xDir %d relationHeadSide %s "
                        + "sideMismatch %s ref %s xStem %s xGapPixels %s yGapPixels %s "
                        + "dx %s dy %s xKind %s xMax %s yMax %s xImpactRaw %s "
                        + "yImpactRaw %s xImpact %s yImpact %s xWeight %s yWeight %s "
                        + "grade %s minGrade %s accepted %s actualHeadSide %s "
                        + "actualDx %s actualDy %s actualGrade %s impactNames %s "
                        + "impactValues %s impactWeights %s extension %s stoppingSide %s "
                        + "glyphsBefore %s stoppingEligible %s compositeLine %s "
                        + "stemPortion %s isEnd %s stoppingUpdate %s stoppingSnapshot %s "
                        + "mapAction %s mapOrdinal %s firstItem %s latestItem %s",
                page, system.getId(), planOrdinal, itemIndex, cAliases.get(c),
                evidence.head.getShape(), rectangle(evidence.head.getBounds()),
                evidence.head.getCenter().x, evidence.head.getCenter().y,
                evidence.storedH, evidence.cVSide, line(evidence.stemLine),
                -evidence.xDir, evidence.xDir, evidence.derivedH,
                evidence.storedH != evidence.derivedH, point(evidence.ref),
                hexDouble(evidence.xStem), hexDouble(evidence.xGapPixels),
                hexDouble(evidence.yGapPixels), hexDouble(evidence.dx),
                hexDouble(evidence.dy), evidence.out ? "OUT" : "IN",
                hexDouble(evidence.xMax), hexDouble(evidence.yMax),
                hexDouble(evidence.xImpactRaw), hexDouble(evidence.yImpactRaw),
                hexDouble(evidence.xImpact), hexDouble(evidence.yImpact),
                hexDouble(evidence.xWeight), hexDouble(evidence.yWeight),
                hexDouble(evidence.grade), hexDouble(evidence.minGrade), evidence.accepted,
                relation != null ? relation.getHeadSide() : "-",
                relation != null ? hexDouble(relation.getDx()) : "-",
                relation != null ? hexDouble(relation.getDy()) : "-",
                relation != null ? hexDouble(relation.getGrade()) : "-",
                impacts != null ? impactNames(impacts) : "-",
                impacts != null ? impactValues(impacts) : "-",
                impacts != null ? impactWeights(impacts) : "-",
                relation != null ? point(relation.getExtensionPoint()) : "-",
                stoppingSide, glyphAliases(glyphsBefore),
                relation != null && relation.getHeadSide() == stoppingSide
                        && !glyphsBefore.isEmpty(),
                compositeLine != null ? line(compositeLine) : "-",
                portion != null ? portion : "-",
                relation != null ? isEnd : "-",
                relation != null ? stoppingUpdate : "-",
                stoppingUpdate ? glyphAliases(glyphsBefore) : "-", mapAction,
                mapOrdinal >= 0 ? mapOrdinal : "-", firstItem >= 0 ? firstItem : "-",
                latestItem >= 0 ? latestItem : "-"), hashes);
    }

    private static UpdateEvidence updateStemLineEvidence (List<StemItem> items,
                                                          int itemIndex,
                                                          LinkedHashSet<Glyph> glyphs,
                                                          IdentityHashMap<Glyph, Integer> insertionItems,
                                                          Line2D stemLine,
                                                          IdentityHashMap<Object, String> bAliases,
                                                          IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final Glyph attemptedGlyph = (Glyph) STEM_ITEM_GLYPH.get(items.get(itemIndex));
        final String attempted = attemptedGlyph != null
                ? itemRef(items.get(itemIndex), itemIndex, bAliases, cAliases) : "-";
        final Glyph canonical = equalGlyph(glyphs, attemptedGlyph);
        final String canonicalBefore = canonical != null
                ? itemRef(
                        items.get(insertionItems.get(canonical)), insertionItems.get(canonical),
                        bAliases, cAliases) : "-";
        final String beforeGlyphs = structuralGlyphRefs(
                items, glyphs, insertionItems, bAliases, cAliases);
        final Line2D beforeLine = new Line2D.Double(stemLine.getP1(), stemLine.getP2());
        if (attemptedGlyph == null) {
            return new UpdateEvidence(
                    attempted, canonicalBefore, "null", beforeGlyphs, beforeGlyphs,
                    beforeLine, new Line2D.Double(stemLine.getP1(), stemLine.getP2()),
                    null, null, 0, false, false);
        }
        if (canonical != null) {
            return new UpdateEvidence(
                    attempted, canonicalBefore, "equalSkip", beforeGlyphs, beforeGlyphs,
                    beforeLine, new Line2D.Double(stemLine.getP1(), stemLine.getP2()),
                    null, null, 0, false, true);
        }

        glyphs.add(attemptedGlyph);
        insertionItems.put(attemptedGlyph, itemIndex);
        final Glyph composite = glyphs.size() > 1
                ? GlyphFactory.buildGlyph(glyphs) : attemptedGlyph;
        final Point2D centroid = composite.getCentroidDouble();
        final Point2D intersection = LineUtil.intersectionAtY(stemLine, centroid.getY());
        final double dx = centroid.getX() - intersection.getX();
        stemLine.setLine(
                stemLine.getX1() + dx, stemLine.getY1(),
                stemLine.getX2() + dx, stemLine.getY2());
        return new UpdateEvidence(
                attempted, canonicalBefore, "insert", beforeGlyphs,
                structuralGlyphRefs(items, glyphs, insertionItems, bAliases, cAliases), beforeLine,
                new Line2D.Double(stemLine.getP1(), stemLine.getP2()), composite,
                intersection, dx, true, false);
    }

    private static void updateStemLine (Glyph glyph,
                                        Set<Glyph> glyphs,
                                        Line2D stemLine)
    {
        if (glyph == null || glyphs.contains(glyph)) return;
        glyphs.add(glyph);
        final Glyph composite = glyphs.size() > 1 ? GlyphFactory.buildGlyph(glyphs) : glyph;
        final Point2D centroid = composite.getCentroidDouble();
        final Point2D intersection = LineUtil.intersectionAtY(stemLine, centroid.getY());
        final double dx = centroid.getX() - intersection.getX();
        stemLine.setLine(
                stemLine.getX1() + dx, stemLine.getY1(),
                stemLine.getX2() + dx, stemLine.getY2());
    }

    private static void assertExpandResult (int actualLastIndex,
                                            LinkedHashMap<Object, Relation> actualRelations,
                                            LinkedHashSet<Glyph> actualGlyphs,
                                            ReplayResult replay,
                                            IdentityHashMap<Object, String> cAliases)
    {
        if (actualLastIndex != replay.lastIndex) {
            throw new IllegalStateException(
                    "expand lastIndex differs: " + actualLastIndex + " != " + replay.lastIndex);
        }
        if (actualRelations.size() != replay.relations.size()) {
            throw new IllegalStateException("expand relation count differs");
        }
        final Iterator<Map.Entry<Object, Relation>> actualRelationIterator =
                actualRelations.entrySet().iterator();
        final Iterator<Map.Entry<Object, HeadStemRelation>> replayRelationIterator =
                replay.relations.entrySet().iterator();
        while (actualRelationIterator.hasNext()) {
            final Map.Entry<Object, Relation> actual = actualRelationIterator.next();
            final Map.Entry<Object, HeadStemRelation> expected = replayRelationIterator.next();
            if (actual.getKey() != expected.getKey()
                    || !(actual.getValue() instanceof HeadStemRelation actualHead)
                    || !sameRelation(actualHead, expected.getValue())) {
                throw new IllegalStateException(
                        "expand relation differs at " + cAliases.get(actual.getKey()));
            }
        }
        if (!sameIdentityIteration(actualGlyphs, replay.glyphs)) {
            throw new IllegalStateException("expand glyph order/identity differs");
        }
    }

    private static boolean sameRelation (HeadStemRelation left,
                                         HeadStemRelation right)
    {
        if (left.getHeadSide() != right.getHeadSide()
                || !sameDouble(left.getDx(), right.getDx())
                || !sameDouble(left.getDy(), right.getDy())
                || !sameDouble(left.getGrade(), right.getGrade())
                || !samePointBits(left.getExtensionPoint(), right.getExtensionPoint())) {
            return false;
        }
        final GradeImpacts li = left.getImpacts();
        final GradeImpacts ri = right.getImpacts();
        if (li.getImpactCount() != ri.getImpactCount()) return false;
        for (int i = 0; i < li.getImpactCount(); i++) {
            if (!li.getName(i).equals(ri.getName(i))
                    || !sameDouble(li.getImpact(i), ri.getImpact(i))
                    || !sameDouble(li.getWeight(i), ri.getWeight(i))) return false;
        }
        return true;
    }

    private static StateSnapshot snapshotOnePlan (Sheet sheet,
                                                  SystemInfo system,
                                                  StemsRetriever retriever,
                                                  AbstractBeamInter ignoredBeam,
                                                  IdentityHashMap<Object, String> bAliases,
                                                  IdentityHashMap<Object, String> cAliases,
                                                  StemBuilder ignoredBuilder)
        throws Exception
    {
        return snapshot(
                sheet, system, retriever,
                (List<Inter>) RETRIEVER_SYSTEM_BEAMS.get(retriever), bAliases, cAliases);
    }

    private static StateSnapshot snapshot (Sheet sheet,
                                           SystemInfo system,
                                           StemsRetriever retriever,
                                           List<Inter> beams,
                                           IdentityHashMap<Object, String> bAliases,
                                           IdentityHashMap<Object, String> cAliases)
        throws Exception
    {
        final List<Object> identities = new ArrayList<>();
        final StringBuilder state = new StringBuilder();
        appendIdentityCollection(identities, system.getSig().vertexSet());
        appendIdentityCollection(identities, system.getSig().edgeSet());
        appendIdentityCollection(identities, system.getSig().inters(StemInter.class));

        final Map<?, ?> systemStems = (Map<?, ?>) RETRIEVER_SYSTEM_STEMS.get(retriever);
        state.append("systemStems=").append(systemStems.size()).append(';');
        for (Map.Entry<?, ?> entry : systemStems.entrySet()) {
            identities.add(entry.getKey());
            identities.add(entry.getValue());
        }

        final List<Glyph> glyphs = glyphIndexGlyphs(sheet.getGlyphIndex());
        state.append("glyphs=").append(glyphs.size()).append(';');
        identities.addAll(glyphs);
        final Collection<?> filaments = sheet.getFilamentIndex().getEntities();
        state.append("filaments=").append(filaments.size()).append(';');
        identities.addAll(filaments);

        for (Inter inter : beams) {
            final AbstractBeamInter beam = (AbstractBeamInter) inter;
            identities.add(beam);
            identities.add(beam.getLinker());
            final List<Object> allB = (List<Object>) LINKER_ALL_B.get(beam.getLinker());
            state.append("beamBs=").append(allB.size()).append(';');
            for (Object b : allB) {
                identities.add(b);
                state.append(bAliases.get(b)).append(':')
                        .append(B_IS_ANCHOR.getBoolean(b)).append(':')
                        .append(((StemLinker) b).isLinked()).append(':')
                        .append(((StemLinker) b).isClosed()).append(';');
                final Map<VerticalSide, Object> vMap =
                        (Map<VerticalSide, Object>) B_V_LINKERS.get(b);
                for (Map.Entry<VerticalSide, Object> entry : vMap.entrySet()) {
                    final Object v = entry.getValue();
                    identities.add(v);
                    final StemBuilder builder = (StemBuilder) V_STEM_BUILDER.get(v);
                    identities.add(builder);
                    state.append("V:").append(entry.getKey()).append(':')
                            .append(((StemLinker) v).isLinked()).append(':')
                            .append(((StemLinker) v).isClosed()).append(':')
                            .append(lineBits((Line2D) V_THEO_LINE.get(v))).append(':')
                            .append(beam.getAttachments().get("theo-" + B_ID.getInt(b))
                                    == V_THEO_LINE.get(v)).append(';');
                    appendBuilderSnapshot(identities, state, builder);
                }
            }
        }

        final List<Map.Entry<String, Object>> orderedCs = new ArrayList<>();
        for (Map.Entry<Object, String> entry : cAliases.entrySet()) {
            orderedCs.add(Map.entry(entry.getValue(), entry.getKey()));
        }
        orderedCs.sort(Map.Entry.comparingByKey());
        for (Map.Entry<String, Object> entry : orderedCs) {
            final Object c = entry.getValue();
            identities.add(c);
            final StemBuilder builder = (StemBuilder) C_STEM_BUILDER.get(c);
            identities.add(builder);
            state.append(entry.getKey()).append(':')
                    .append(((StemLinker) c).isLinked()).append(':')
                    .append(((StemLinker) c).isClosed()).append(';');
            appendBuilderSnapshot(identities, state, builder);
        }
        return new StateSnapshot(identities, state.toString());
    }

    private static void appendBuilderSnapshot (List<Object> identities,
                                               StringBuilder state,
                                               StemBuilder builder)
        throws Exception
    {
        if (builder == null) {
            state.append("builder=null;");
            return;
        }
        final List<StemItem> items = (List<StemItem>) STEM_BUILDER_ITEMS.get(builder);
        state.append("builderTheo=")
                .append(lineBits((Line2D) STEM_BUILDER_THEO_LINE.get(builder)))
                .append(';').append("items=").append(items.size()).append(':');
        for (StemItem item : items) {
            identities.add(item);
            final Glyph glyph = (Glyph) STEM_ITEM_GLYPH.get(item);
            identities.add(glyph);
            final Line2D line = (Line2D) STEM_ITEM_LINE.get(item);
            state.append(item.getClass().getSimpleName()).append('@')
                    .append(lineBits(line)).append('@')
                    .append(STEM_ITEM_CONTRIB.getInt(item)).append(',');
            if (LINKER_ITEM_CLASS.isInstance(item)) {
                identities.add(LINKER_ITEM_LINKER.get(item));
            }
        }
        state.append("lengths=").append(STEM_BUILDER_LENGTH_MAP.get(builder)).append(';');
    }

    private static void appendIdentityCollection (List<Object> target,
                                                  Collection<?> source)
    {
        target.addAll(source);
    }

    private static List<Object> cLinkers (List<StemItem> items)
        throws IllegalAccessException
    {
        final List<Object> result = new ArrayList<>();
        for (StemItem item : items) {
            if (LINKER_ITEM_CLASS.isInstance(item)) {
                final Object linker = LINKER_ITEM_LINKER.get(item);
                if (C_LINKER_CLASS.isInstance(linker)) result.add(linker);
            }
        }
        return result;
    }

    private static StemItem lastGapBefore (List<StemItem> items,
                                          int index)
    {
        for (int i = index - 1; i >= 0; i--) {
            if (items.get(i) instanceof StemItem.GapItem) return items.get(i);
        }
        return null;
    }

    private static int itemIndexOfLinker (List<StemItem> items,
                                         Object linker)
        throws IllegalAccessException
    {
        for (int index = 0; index < items.size(); index++) {
            final StemItem item = items.get(index);
            if (LINKER_ITEM_CLASS.isInstance(item)
                    && LINKER_ITEM_LINKER.get(item) == linker) return index;
        }
        return -1;
    }

    private static int firstIdentityGlyphItem (List<StemItem> items,
                                               Glyph glyph)
        throws IllegalAccessException
    {
        for (int index = 0; index < items.size(); index++) {
            if (STEM_ITEM_GLYPH.get(items.get(index)) == glyph) return index;
        }
        return -1;
    }

    private static String structuralRef (List<StemItem> items,
                                         Glyph glyph,
                                         IdentityHashMap<Object, String> bAliases,
                                         IdentityHashMap<Object, String> cAliases)
        throws IllegalAccessException
    {
        final int index = firstIdentityGlyphItem(items, glyph);
        return index >= 0 ? itemRef(items.get(index), index, bAliases, cAliases) : "-";
    }

    private static String structuralGlyphRefs (List<StemItem> items,
                                               Collection<Glyph> glyphs,
                                               IdentityHashMap<Glyph, Integer> insertionItems,
                                               IdentityHashMap<Object, String> bAliases,
                                               IdentityHashMap<Object, String> cAliases)
        throws IllegalAccessException
    {
        final List<String> refs = new ArrayList<>();
        for (Glyph glyph : glyphs) {
            final Integer item = insertionItems.get(glyph);
            if (item == null) throw new IllegalStateException("missing insertion item");
            refs.add(itemRef(items.get(item), item, bAliases, cAliases));
        }
        return listToken(refs);
    }

    private static String itemRef (StemItem item,
                                   int index,
                                   IdentityHashMap<Object, String> bAliases,
                                   IdentityHashMap<Object, String> cAliases)
        throws IllegalAccessException
    {
        if (item instanceof StemItem.GapItem) return "gap:" + index;
        if (LINKER_ITEM_CLASS.isInstance(item)) {
            final Object linker = LINKER_ITEM_LINKER.get(item);
            if (V_LINKER_CLASS.isInstance(linker)) return "start:" + index;
            if (B_LINKER_CLASS.isInstance(linker)) return "B:" + bAliases.get(linker);
            if (C_LINKER_CLASS.isInstance(linker)) return "C:" + cAliases.get(linker);
        }
        return "glyphItem:" + index;
    }

    private static String itemKind (StemItem item)
        throws IllegalAccessException
    {
        if (item instanceof StemItem.GapItem) return "gap";
        if (LINKER_ITEM_CLASS.isInstance(item)) {
            final Object linker = LINKER_ITEM_LINKER.get(item);
            if (V_LINKER_CLASS.isInstance(linker)) return "start";
            if (B_LINKER_CLASS.isInstance(linker)) return "B";
            if (C_LINKER_CLASS.isInstance(linker)) return "C";
        }
        return "glyph";
    }

    private static HorizontalSide storedHorizontalSide (Object c)
        throws Exception
    {
        return (HorizontalSide) S_GET_HORIZONTAL_SIDE.invoke(C_GET_S_LINKER.invoke(c));
    }

    private static Glyph equalGlyph (Collection<Glyph> glyphs,
                                     Glyph candidate)
    {
        if (candidate == null) return null;
        for (Glyph glyph : glyphs) if (glyph.equals(candidate)) return glyph;
        return null;
    }

    private static Line2D orientedLine (Line2D theo,
                                       int yDir)
    {
        return yDir > 0
                ? new Line2D.Double(theo.getP1(), theo.getP2())
                : new Line2D.Double(theo.getP2(), theo.getP1());
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

    private static Field declaredField (Class<?> owner,
                                        String name)
        throws NoSuchFieldException
    {
        final Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static IdentityHashMap<Inter, Integer> identityOrdinals (
            List<? extends Inter> values)
    {
        final IdentityHashMap<Inter, Integer> result = new IdentityHashMap<>();
        for (int index = 0; index < values.size(); index++) {
            if (result.put(values.get(index), index) != null) {
                throw new IllegalStateException("duplicate inter identity");
            }
        }
        return result;
    }

    private static List<Glyph> glyphIndexGlyphs (GlyphIndex index)
    {
        final List<Glyph> result = new ArrayList<>();
        final Iterator<Glyph> iterator = index.iterator();
        while (iterator.hasNext()) result.add(iterator.next());
        return result;
    }

    private static String bToken (int beamSigOrdinal,
                                  int bOrdinal)
    {
        return "beam:" + beamSigOrdinal + ":b:" + bOrdinal;
    }

    private static String cToken (int headOrdinal,
                                  HorizontalSide hSide,
                                  VerticalSide vSide)
    {
        return "h:" + headOrdinal + ":" + hSide + ":" + vSide;
    }

    private static String beamSigToken (AbstractBeamInter beam,
                                        IdentityHashMap<Object, String> bAliases,
                                        Object b)
    {
        final String alias = bAliases.get(b);
        final int start = "beam:".length();
        return alias.substring(start, alias.indexOf(":b:"));
    }

    private static String ordinalToken (List<Inter> values,
                                        IdentityHashMap<Inter, Integer> ordinals)
    {
        final List<String> result = new ArrayList<>();
        for (Inter value : values) result.add(Integer.toString(ordinals.get(value)));
        return listToken(result);
    }

    private static String linkerTokens (Collection<Object> linkers,
                                        IdentityHashMap<Object, String> aliases)
    {
        final List<String> result = new ArrayList<>();
        for (Object linker : linkers) result.add(aliases.get(linker));
        return listToken(result);
    }

    private static String relationTokens (Collection<?> linkers,
                                          IdentityHashMap<Object, String> aliases)
    {
        final List<String> result = new ArrayList<>();
        for (Object linker : linkers) result.add(aliases.get(linker));
        return listToken(result);
    }

    private static String glyphAliases (Collection<Glyph> glyphs)
    {
        final List<String> result = new ArrayList<>();
        for (Glyph glyph : glyphs) result.add(glyphAlias(glyph));
        return listToken(result);
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
        final MessageDigest digest = sha256Digest();
        final var table = glyph.getRunTable();
        trace(digest, table.getOrientation() + " " + table.getWidth() + " "
                + table.getHeight() + "\n");
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<org.audiveris.omr.run.Run> iterator = table.iterator(sequence);
                    iterator.hasNext();) {
                final var run = iterator.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            trace(digest, row.append('\n').toString());
        }
        return digestHex(digest.digest());
    }

    private static String impactNames (GradeImpacts impacts)
    {
        final List<String> result = new ArrayList<>();
        for (int i = 0; i < impacts.getImpactCount(); i++) result.add(impacts.getName(i));
        return listToken(result);
    }

    private static String impactValues (GradeImpacts impacts)
    {
        final List<String> result = new ArrayList<>();
        for (int i = 0; i < impacts.getImpactCount(); i++) {
            result.add(hexDouble(impacts.getImpact(i)));
        }
        return listToken(result);
    }

    private static String impactWeights (GradeImpacts impacts)
    {
        final List<String> result = new ArrayList<>();
        for (int i = 0; i < impacts.getImpactCount(); i++) {
            result.add(hexDouble(impacts.getWeight(i)));
        }
        return listToken(result);
    }

    private static String listToken (Collection<String> values)
    {
        return values.isEmpty() ? "-" : String.join(",", values);
    }

    private static String rectangle (Rectangle box)
    {
        return box.x + ":" + box.y + ":" + box.width + ":" + box.height;
    }

    private static String line (Line2D value)
    {
        return value != null ? point(value.getP1()) + ":" + point(value.getP2()) : "-";
    }

    private static String lineBits (Line2D value)
    {
        return String.format(
                "%016x:%016x:%016x:%016x",
                Double.doubleToLongBits(value.getX1()),
                Double.doubleToLongBits(value.getY1()),
                Double.doubleToLongBits(value.getX2()),
                Double.doubleToLongBits(value.getY2()));
    }

    private static String point (Point2D value)
    {
        return value != null ? hexDouble(value.getX()) + ":" + hexDouble(value.getY()) : "-";
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static boolean sameDouble (double left,
                                       double right)
    {
        return Double.doubleToLongBits(left) == Double.doubleToLongBits(right);
    }

    private static boolean samePointBits (Point2D left,
                                          Point2D right)
    {
        if (left == null || right == null) return left == right;
        return sameDouble(left.getX(), right.getX()) && sameDouble(left.getY(), right.getY());
    }

    private static boolean sameLineBits (Line2D left,
                                         Line2D right)
    {
        return sameDouble(left.getX1(), right.getX1())
                && sameDouble(left.getY1(), right.getY1())
                && sameDouble(left.getX2(), right.getX2())
                && sameDouble(left.getY2(), right.getY2());
    }

    private static boolean sameIdentityIteration (Collection<?> left,
                                                  Collection<?> right)
    {
        if (left.size() != right.size()) return false;
        final Iterator<?> li = left.iterator();
        final Iterator<?> ri = right.iterator();
        while (li.hasNext()) if (li.next() != ri.next()) return false;
        return true;
    }

    private static int identityIndex (List<?> values,
                                      Object target)
    {
        if (target == null) return -1;
        for (int index = 0; index < values.size(); index++) {
            if (values.get(index) == target) return index;
        }
        return -1;
    }

    private static double clamp (double value)
    {
        if (value < 0) value = 0;
        if (value > 1) value = 1;
        return value;
    }

    private static MessageDigest sha256Digest ()
    {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (java.security.NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static void trace (MessageDigest digest,
                               String value)
    {
        digest.update(value.getBytes(StandardCharsets.UTF_8));
    }

    private static String digestHex (byte[] values)
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
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam VLinker.expand oracle.");
        System.out.println("# schema: stems-beam-expand-v1");
        System.out.println("#");
        System.out.println("# Each page runs in a fresh independent JVM and reaches real HEADS.");
        System.out.println("# The real inspectStems chronology assigns all V and C StemBuilders first.");
        System.out.println("# Every V builder is graded at stemProfile 0..constructionMax with the");
        System.out.println("# effective system linkProfile; scheduler selection is deliberately out of scope.");
        System.out.println("# Real private expand output is checked against an exact independent replay.");
        System.out.println("# The boundary stops before createStem. It asserts zero graph/index/link mutation,");
        System.out.println("# while pinning Java's downward V/builder/attachment theoLine alias mutation.");
    }

    private static final class ReplayResult
    {
        int lastIndex = -2;
        int stoppingIndex = -1;
        String stopCause = "NoHeadTarget";
        String traceSha256 = digestHex(sha256Digest().digest());
        final Line2D finalStemLine;
        final LinkedHashMap<Object, HeadStemRelation> relations;
        final IdentityHashMap<Object, Integer> relationItemIndices;
        final IdentityHashMap<Object, Integer> relationFirstItemIndices;
        final LinkedHashSet<Glyph> glyphs;
        final IdentityHashMap<Glyph, Integer> insertionItems;
        int visits;
        int gaps;
        int showStoppingGaps;
        int separationChecks;
        int separationStops;
        int relationAttempts;
        int relationAccepts;
        int relationRejects;
        int stoppingUpdates;
        int glyphUpdateCalls;
        int glyphInsertions;
        int glyphEqualSkips;

        ReplayResult (Line2D finalStemLine,
                      LinkedHashMap<Object, HeadStemRelation> relations,
                      IdentityHashMap<Object, Integer> relationItemIndices,
                      IdentityHashMap<Object, Integer> relationFirstItemIndices,
                      LinkedHashSet<Glyph> glyphs,
                      IdentityHashMap<Glyph, Integer> insertionItems)
        {
            this.finalStemLine = finalStemLine;
            this.relations = relations;
            this.relationItemIndices = relationItemIndices;
            this.relationFirstItemIndices = relationFirstItemIndices;
            this.glyphs = glyphs;
            this.insertionItems = insertionItems;
        }

        static ReplayResult noHeadTarget (Line2D line)
        {
            return new ReplayResult(
                    line, new LinkedHashMap<>(), new IdentityHashMap<>(), new IdentityHashMap<>(),
                    new LinkedHashSet<>(), new IdentityHashMap<>());
        }

        ReplayResult finish (int lastIndex,
                             int stoppingIndex,
                             String stopCause,
                             MessageDigest trace)
        {
            this.lastIndex = lastIndex;
            this.stoppingIndex = stoppingIndex;
            this.stopCause = stopCause;
            traceSha256 = digestHex(trace.digest());
            return this;
        }
    }

    private record RelationEvidence(
            HeadInter head,
            VerticalSide cVSide,
            HorizontalSide storedH,
            int xDir,
            HorizontalSide derivedH,
            Line2D stemLine,
            Point2D ref,
            double xStem,
            double xGapPixels,
            double yGapPixels,
            double dx,
            double dy,
            boolean out,
            double xMax,
            double yMax,
            double xImpactRaw,
            double yImpactRaw,
            double xImpact,
            double yImpact,
            double xWeight,
            double yWeight,
            double grade,
            double minGrade,
            boolean accepted,
            Point2D extension)
    {
        void assertRelation (HeadStemRelation relation)
        {
            if ((relation != null) != accepted) {
                throw new IllegalStateException("computed HeadStemRelation acceptance differs");
            }
            if (relation == null) return;
            if (relation.getHeadSide() != derivedH
                    || !sameDouble(relation.getDx(), dx)
                    || !sameDouble(relation.getDy(), dy)
                    || !sameDouble(relation.getGrade(), grade)
                    || !sameDouble(relation.getMinGrade(), minGrade)
                    || !samePointBits(relation.getExtensionPoint(), extension)) {
                throw new IllegalStateException("computed HeadStemRelation payload differs");
            }
            final GradeImpacts impacts = relation.getImpacts();
            if (impacts.getImpactCount() != 2
                    || !impacts.getName(0).equals(out ? "xOutGap" : "xInGap")
                    || !impacts.getName(1).equals("yGap")
                    || !sameDouble(impacts.getImpact(0), xImpact)
                    || !sameDouble(impacts.getImpact(1), yImpact)
                    || !sameDouble(impacts.getWeight(0), xWeight)
                    || !sameDouble(impacts.getWeight(1), yWeight)) {
                throw new IllegalStateException("computed HeadStemRelation impacts differ");
            }
        }

        String digestToken ()
        {
            return cVSide + ":" + storedH + ":" + derivedH + ":"
                    + Long.toHexString(Double.doubleToLongBits(dx)) + ":"
                    + Long.toHexString(Double.doubleToLongBits(dy)) + ":"
                    + Long.toHexString(Double.doubleToLongBits(grade));
        }
    }

    private record UpdateEvidence(
            String attempted,
            String canonicalBefore,
            String action,
            String glyphsBefore,
            String glyphsAfter,
            Line2D lineBefore,
            Line2D lineAfter,
            Glyph composite,
            Point2D intersection,
            double dx,
            boolean inserted,
            boolean equalSkip)
    {
        String traceToken ()
        {
            return "update " + attempted + " " + canonicalBefore + " " + action + " "
                    + lineBits(lineBefore) + " " + lineBits(lineAfter);
        }
    }

    private record StateSnapshot(List<Object> identities, String state)
    {
        void assertSame (StateSnapshot other,
                         String context)
        {
            if (!sameIdentityIteration(identities, other.identities) || !state.equals(other.state)) {
                throw new IllegalStateException(context + " mutated forbidden persistent state");
            }
        }
    }

    private static final class Totals
    {
        long systems;
        long builders;
        long plans;
        long noHeadTarget;
        long expandFailed;
        long noRelations;
        long noGlyphs;
        long ready;
        long relations;
        long glyphs;
        long postReturnRelations;
        long plansWithPostReturnRelations;
        long rollbackLineDivergences;
        long relationSideMismatches;
        long storedTheoMutations;
        long attachmentLineMutations;
        long beamSideReadyWithoutStoppingHead;
        long beamSideReady;
        long beamSideReadyBeyondStoppingHead;
        long beamSideReadyAtStoppingHead;
        double maxAbsStoredTheoShift;

        void include (Totals that)
        {
            systems += that.systems;
            builders += that.builders;
            plans += that.plans;
            noHeadTarget += that.noHeadTarget;
            expandFailed += that.expandFailed;
            noRelations += that.noRelations;
            noGlyphs += that.noGlyphs;
            ready += that.ready;
            relations += that.relations;
            glyphs += that.glyphs;
            postReturnRelations += that.postReturnRelations;
            plansWithPostReturnRelations += that.plansWithPostReturnRelations;
            rollbackLineDivergences += that.rollbackLineDivergences;
            relationSideMismatches += that.relationSideMismatches;
            storedTheoMutations += that.storedTheoMutations;
            attachmentLineMutations += that.attachmentLineMutations;
            beamSideReadyWithoutStoppingHead += that.beamSideReadyWithoutStoppingHead;
            beamSideReady += that.beamSideReady;
            beamSideReadyBeyondStoppingHead += that.beamSideReadyBeyondStoppingHead;
            beamSideReadyAtStoppingHead += that.beamSideReadyAtStoppingHead;
            maxAbsStoredTheoShift = Math.max(
                    maxAbsStoredTheoShift, that.maxAbsStoredTheoShift);
        }

        String fields ()
        {
            return String.format(
                    "systems %d builders %d plans %d noHeadTarget %d expandFailed %d "
                            + "noRelations %d noGlyphs %d ready %d relations %d glyphs %d "
                            + "postReturnRelations %d plansWithPostReturnRelations %d "
                            + "rollbackLineDivergences %d relationSideMismatches %d "
                            + "storedTheoMutations %d attachmentLineMutations %d "
                            + "beamSideReady %d beamSideReadyWithoutStoppingHead %d "
                            + "beamSideReadyBeyondStoppingHead %d "
                            + "beamSideReadyAtStoppingHead %d "
                            + "maxAbsStoredTheoShift %s "
                            + "sigMutations 0 systemStemMutations 0 glyphIndexMutations 0 "
                            + "filamentIndexMutations 0 linkMutations 0 builderMutations 0",
                    systems, builders, plans, noHeadTarget, expandFailed, noRelations,
                    noGlyphs, ready, relations, glyphs, postReturnRelations,
                    plansWithPostReturnRelations, rollbackLineDivergences,
                    relationSideMismatches, storedTheoMutations, attachmentLineMutations,
                    beamSideReady, beamSideReadyWithoutStoppingHead,
                    beamSideReadyBeyondStoppingHead, beamSideReadyAtStoppingHead,
                    hexDouble(maxAbsStoredTheoShift));
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
