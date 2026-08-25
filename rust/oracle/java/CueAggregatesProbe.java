// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.lag.BasicLag;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.beam.BeamsBuilder;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.inter.StemInter;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.ui.symbol.MusicFont;

/** Exact identity-free oracle for {@code BeamsBuilder.getCueAggregates()}. */
public final class CueAggregatesProbe
{
    private CueAggregatesProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            System.out.println("# Temurin-25 BeamsBuilder.getCueAggregates oracle.");
            System.out.println("# Java reaches REDUCTION with the qualified smallHeads switch true.");
            System.exit(0);
        }
        final boolean stageAudit = (args.length == 3) && args[0].equals("--stage");
        if ((args.length != 1) && !stageAudit) {
            throw new IllegalArgumentException("expected [--stage STEP] <path>:<sheet>");
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters(
                "-batch",
                "-step",
                stageAudit ? args[1] : "REDUCTION",
                "-constant",
                "org.audiveris.omr.sheet.ProcessingSwitches.smallHeads=true");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        MusicFont.checkMusicFont();

        final String target = args[stageAudit ? 2 : 0];
        final String[] parts = target.split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        runPage(
                Paths.get(parts[0]).toAbsolutePath(),
                Integer.parseInt(parts[1]),
                stageAudit ? OmrStep.valueOf(args[1]) : OmrStep.REDUCTION,
                stageAudit);
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted,
                                 OmrStep step,
                                 boolean stageAudit)
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

        wantedStub.reachStep(step, false);
        final Sheet sheet = wantedStub.getSheet();
        final String page = path.getFileName() + "#" + wanted;
        if (stageAudit) {
            printStageSmallHeads(page, step, sheet);
            return;
        }
        final List<String> summaryRows = new ArrayList<>();

        for (SystemInfo system : sheet.getSystems()) {
            final SIGraph sig = system.getSig();
            final List<Inter> sigOrder = new ArrayList<>(sig.vertexSet());
            final Map<Inter, Integer> ordinals = new IdentityHashMap<>();
            for (int ordinal = 0; ordinal < sigOrder.size(); ordinal++) {
                ordinals.put(sigOrder.get(ordinal), ordinal);
            }

            final BeamsBuilder builder = new BeamsBuilder(
                    system,
                    new BasicLag("cue-aggregate-probe", Orientation.VERTICAL));
            final Object parameters = field(BeamsBuilder.class, "params").get(builder);
            final int cueXMargin = field(parameters.getClass(), "cueXMargin").getInt(parameters);
            final int cueYMargin = field(parameters.getClass(), "cueYMargin").getInt(parameters);

            final Method method = BeamsBuilder.class.getDeclaredMethod("getCueAggregates");
            method.setAccessible(true);
            @SuppressWarnings("unchecked")
            final List<Object> aggregates = (List<Object>) method.invoke(builder);

            final Map<Inter, Integer> retainedAggregate = new IdentityHashMap<>();
            for (int aggregateOrdinal = 0; aggregateOrdinal < aggregates.size(); aggregateOrdinal++) {
                final Object aggregate = aggregates.get(aggregateOrdinal);
                @SuppressWarnings("unchecked")
                final List<Inter> heads = (List<Inter>) field(aggregate.getClass(), "heads")
                        .get(aggregate);
                @SuppressWarnings("unchecked")
                final List<Inter> stems = (List<Inter>) field(aggregate.getClass(), "stems")
                        .get(aggregate);
                final Rectangle bounds = (Rectangle) field(aggregate.getClass(), "bounds")
                        .get(aggregate);
                final List<String> members = new ArrayList<>();
                for (int member = 0; member < heads.size(); member++) {
                    final Inter head = heads.get(member);
                    final Inter stem = stems.get(member);
                    retainedAggregate.put(head, aggregateOrdinal);
                    members.add(ordinals.get(head) + ":" + ordinals.get(stem));
                }
                final String row = String.format(
                        "cueaggregate %s system %d ordinal %d bounds %d %d %d %d members %s",
                        page,
                        system.getId(),
                        aggregateOrdinal,
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                        String.join(",", members));
                summaryRows.add(row);
                System.out.println(row);
            }

            int smallBlackCount = 0;
            final List<Inter> qualified = new ArrayList<>();
            for (Inter inter : sigOrder) {
                if (!inter.isRemoved() && inter.getShape() == Shape.NOTEHEAD_BLACK_SMALL) {
                    smallBlackCount++;
                    if (inter.getContextualGrade() != null && inter.getContextualGrade() >= 0.5) {
                        qualified.add(inter);
                    }
                }
            }
            Collections.sort(qualified, Inters.byAbscissa);
            final Method stemOf = BeamsBuilder.class.getDeclaredMethod("stemOf", Inter.class);
            stemOf.setAccessible(true);
            for (Inter inter : qualified) {
                final Inter stem = (Inter) stemOf.invoke(builder, inter);
                final Rectangle bounds = inter.getBounds();
                final Integer aggregate = retainedAggregate.get(inter);
                final String row = String.format(
                        "cueaggregatehead %s system %d sig %d stem %d bounds %d %d %d %d gradeBits %016x contextualBits %016x aggregate %d",
                        page,
                        system.getId(),
                        ordinals.get(inter),
                        ordinals.get(stem),
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                        Double.doubleToLongBits(inter.getGrade()),
                        Double.doubleToLongBits(inter.getContextualGrade()),
                        aggregate != null ? aggregate : -1);
                summaryRows.add(row);
                System.out.println(row);
            }
            final String systemRow = String.format(
                    "cueaggregatesystem %s system %d interline %d margins %d %d smallBlack %d qualified %d aggregates %d",
                    page,
                    system.getId(),
                    sheet.getScale().getInterline(),
                    cueXMargin,
                    cueYMargin,
                    smallBlackCount,
                    qualified.size(),
                    aggregates.size());
            summaryRows.add(systemRow);
            System.out.println(systemRow);
        }

        System.out.printf(
                "cueaggregatesummary %s systems %d rows %d %016x%n",
                page,
                sheet.getSystems().size(),
                summaryRows.size(),
                hash(summaryRows));
    }

    private static void printStageSmallHeads (String page,
                                              OmrStep step,
                                              Sheet sheet)
    {
        for (SystemInfo system : sheet.getSystems()) {
            int count = 0;
            for (Inter inter : system.getSig().vertexSet()) {
                if (!inter.isRemoved() && inter.getShape() == Shape.NOTEHEAD_BLACK_SMALL) {
                    final Rectangle bounds = inter.getBounds();
                    System.out.printf(
                            "cueaggregatestage %s step %s system %d id %d bounds %d %d %d %d gradeBits %016x contextual %s%n",
                            page,
                            step,
                            system.getId(),
                            inter.getId(),
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height,
                            Double.doubleToLongBits(inter.getGrade()),
                            inter.getContextualGrade() == null
                                    ? "-"
                                    : String.format("%016x", Double.doubleToLongBits(inter.getContextualGrade())));
                    for (Relation relation : system.getSig().getRelations(
                            inter,
                            HeadStemRelation.class)) {
                        final HeadStemRelation headStem = (HeadStemRelation) relation;
                        final StemInter stem = (StemInter) system.getSig().getEdgeTarget(relation);
                        final Rectangle stemBounds = stem.getBounds();
                        System.out.printf(
                                "cueaggregatestagerelation %s step %s system %d head %d stem %d stemBounds %d %d %d %d gradeBits %016x dxBits %016x dyBits %016x side %s extension %s consistencyBits %016x%n",
                                page,
                                step,
                                system.getId(),
                                inter.getId(),
                                stem.getId(),
                                stemBounds.x,
                                stemBounds.y,
                                stemBounds.width,
                                stemBounds.height,
                                Double.doubleToLongBits(headStem.getGrade()),
                                Double.doubleToLongBits(headStem.getDx()),
                                Double.doubleToLongBits(headStem.getDy()),
                                headStem.getHeadSide(),
                                headStem.getExtensionPoint(),
                                Double.doubleToLongBits(headStem.getConsistency()));
                    }
                    count++;
                }
            }
            System.out.printf(
                    "cueaggregatestagesummary %s step %s system %d smallBlack %d%n",
                    page,
                    step,
                    system.getId(),
                    count);
        }
    }

    private static Field field (Class<?> type,
                                String name)
        throws Exception
    {
        final Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static long hash (List<String> records)
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
}
