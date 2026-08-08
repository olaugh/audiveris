// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
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
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.image.DistanceTable;
import org.audiveris.omr.image.TemplateFactory;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.note.DistancesBuilder;
import org.audiveris.omr.sheet.note.HeadSeedScale;
import org.audiveris.omr.sheet.note.HeadSeedTally;
import org.audiveris.omr.sheet.note.HeadSpotsBuilder;
import org.audiveris.omr.sheet.note.NoteHeadsBuilder;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.AbstractNoteInter;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.InterPairPredicate;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.Exclusion;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;
import org.audiveris.omr.util.HorizontalSide;

import ij.process.ByteProcessor;

/** Exact oracle for the production HEADS operations after range lookup. */
@SuppressWarnings("unchecked")
public class HeadsPostRangeProbe
{
    private static final Class<?> BUILDER_CLASS = NoteHeadsBuilder.class;

    private static final double EPSILON = 1E-5;

    private HeadsPostRangeProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            System.exit(0);
        }
        final boolean fullTrace = (args.length == 2) && args[0].equals("--full-trace");
        if ((!fullTrace && (args.length != 1)) || (fullTrace && (args.length != 2))) {
            throw new IllegalArgumentException("expected [--full-trace] <path>:<sheet>");
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "LEDGERS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        MusicFont.checkMusicFont();

        final String[] parts = args[fullTrace ? 1 : 0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        runPage(Paths.get(parts[0]).toAbsolutePath(), Integer.parseInt(parts[1]), fullTrace);
        System.exit(0);
    }

    @SuppressWarnings("unchecked")
    private static void runPage (Path path,
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
        final Map<SystemInfo, HeadSeedTally> tallies = new LinkedHashMap<>();
        final RowHasher pageHash = new RowHasher();
        int pageStaffs = 0;
        int pageInputs = 0;
        int pageDuplicates = 0;
        int pageOverlaps = 0;
        int pageStaffHeads = 0;
        int pagePurgedBeams = 0;
        int pagePurgedHeads = 0;
        int pageFinalHeads = 0;

        System.out.printf(
                "headpostpage %s systems %d staves %d family %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                family);

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final HeadSeedTally tally = new HeadSeedTally();
            tallies.put(system, tally);
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
            int systemInputs = 0;
            int systemDuplicates = 0;
            int systemOverlaps = 0;
            int systemStaffHeads = 0;

            for (Staff staff : system.getStaves()) {
                if (staff.isTablature()) {
                    continue;
                }
                pageStaffs++;
                setField(
                        builder,
                        "catalog",
                        TemplateFactory.getInstance().getCatalog(
                                family, staff.getHeadPointSize()));

                final List<HeadInter> seedHeads = processStaff(builder, staff, true);
                systemCompetitors.addAll(seedHeads);
                Collections.sort(systemCompetitors, Inters.byOrdinate);
                final List<HeadInter> rangeHeads = processStaff(builder, staff, false);
                final List<HeadInter> heads = new ArrayList<>(seedHeads);
                heads.addAll(rangeHeads);
                Collections.sort(heads, Inters.byFullAbscissa);

                final IdentityHashMap<HeadInter, String> origins = new IdentityHashMap<>();
                for (int ordinal = 0; ordinal < seedHeads.size(); ordinal++) {
                    origins.put(seedHeads.get(ordinal), "seed:" + ordinal);
                }
                for (int ordinal = 0; ordinal < rangeHeads.size(); ordinal++) {
                    origins.put(rangeHeads.get(ordinal), "range:" + ordinal);
                }
                final IdentityHashMap<HeadInter, Integer> inputOrdinals = identityOrdinals(heads);
                final RowHasher staffHash = new RowHasher();
                final RowHasher inputHash = new RowHasher();
                final RowHasher duplicateCheckHash = new RowHasher();
                final RowHasher duplicateDecisionHash = new RowHasher();
                final RowHasher overlapCheckHash = new RowHasher();
                final RowHasher overlapDecisionHash = new RowHasher();
                final RowHasher tallyHash = new RowHasher();
                final RowHasher staffHeadHash = new RowHasher();

                for (HeadInter head : heads) {
                    final String row = inputRow(
                            page,
                            system,
                            staff,
                            inputOrdinals.get(head),
                            origins.get(head),
                            head,
                            tally);
                    addSemantic(row, pageHash, systemHash, staffHash, inputHash);
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }

                final PurgeTrace duplicateTrace = tracePurge(
                        system,
                        staff,
                        heads,
                        inputOrdinals,
                        origins,
                        tally,
                        (one, two) -> ((HeadInter) one).isSameAs((HeadInter) two),
                        true,
                        "duplicate");
                for (String row : duplicateTrace.checkRows) {
                    addSemantic(row, pageHash, systemHash, staffHash, duplicateCheckHash);
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }
                for (String row : duplicateTrace.decisionRows) {
                    addSemantic(row, pageHash, systemHash, staffHash, duplicateDecisionHash);
                    System.out.println(row);
                }
                final int duplicates = purge(
                        builder,
                        heads,
                        "duplicate",
                        (one, two) -> ((HeadInter) one).isSameAs((HeadInter) two),
                        true);
                assertPurge(page, system, staff, heads, duplicateTrace, duplicates, tally);

                final PurgeTrace overlapTrace = tracePurge(
                        system,
                        staff,
                        heads,
                        inputOrdinals,
                        origins,
                        tally,
                        (one, two) -> ((HeadInter) one).overlaps((HeadInter) two),
                        false,
                        "overlap");
                for (String row : overlapTrace.checkRows) {
                    addSemantic(row, pageHash, systemHash, staffHash, overlapCheckHash);
                    if (fullTrace) {
                        System.out.println(row);
                    }
                }
                for (String row : overlapTrace.decisionRows) {
                    addSemantic(row, pageHash, systemHash, staffHash, overlapDecisionHash);
                    System.out.println(row);
                }
                final int overlaps = purge(
                        builder,
                        heads,
                        "overlap",
                        (one, two) -> ((HeadInter) one).overlaps((HeadInter) two),
                        false);
                assertPurge(page, system, staff, heads, overlapTrace, overlaps, tally);
                for (PurgeDecision decision : overlapTrace.decisions) {
                    final Exclusion exclusion = exclusionBetween(
                            system.getSig(), decision.purged, decision.kept);
                    if ((exclusion == null)
                            || (exclusion.cause != Exclusion.ExclusionCause.OVERLAP)) {
                        throw new IllegalStateException("production overlap exclusion is absent");
                    }
                }

                final int tallyBefore = tallySize(tally);
                tally.purgeRemovedHeads();
                final int tallyAfter = tallySize(tally);
                for (HeadInter head : heads) {
                    for (HorizontalSide side : HorizontalSide.values()) {
                        final Double dx = tally.getDx(head, side);
                        if (dx == null) {
                            continue;
                        }
                        final String row = String.format(
                                "headposttally %s system %d staff %d input %d side %s dx %s",
                                page,
                                system.getId(),
                                staff.getId(),
                                inputOrdinals.get(head),
                                side,
                                hexDouble(dx));
                        addSemantic(row, pageHash, systemHash, staffHash, tallyHash);
                        if (fullTrace) {
                            System.out.println(row);
                        }
                    }
                }

                final int notesBefore = staffNoteCount(staff);
                for (HeadInter head : heads) {
                    final double gradeBefore = head.getGrade();
                    final boolean stemless = ShapeSet.StemLessHeads.contains(head.getShape());
                    if (stemless) {
                        head.increase(NoteHeadsBuilder.getStemLessBoost());
                    }
                    staff.addNote(head);
                    final String row = staffHeadRow(
                            page,
                            system,
                            staff,
                            inputOrdinals.get(head),
                            origins.get(head),
                            head,
                            gradeBefore,
                            stemless,
                            tally);
                    addSemantic(row, pageHash, systemHash, staffHash, staffHeadHash);
                    System.out.println(row);
                }
                final int notesAfter = staffNoteCount(staff);
                if ((notesAfter != notesBefore) || !staffContainsAll(staff, heads)) {
                    throw new IllegalStateException(String.format(
                            "staff note attachment state differs: %d -> %d for %d heads",
                            notesBefore,
                            notesAfter,
                            heads.size()));
                }

                final String summary = String.format(
                        "headpoststaffsummary %s system %d staff %d inputs %d seed %d range %d "
                                + "duplicates %d afterDuplicates %d overlaps %d tallyBefore %d "
                                + "tallyAfter %d notesBefore %d notesAfter %d retained %d "
                                + "inputHash %016x duplicateChecks %d:%016x duplicateHash %016x "
                                + "overlapChecks %d:%016x overlapHash %016x tallyHash %016x "
                                + "staffHeadHash %016x hash %016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        inputOrdinals.size(),
                        seedHeads.size(),
                        rangeHeads.size(),
                        duplicates,
                        heads.size(),
                        overlaps,
                        tallyBefore,
                        tallyAfter,
                        notesBefore,
                        notesAfter,
                        heads.size(),
                        inputHash.value(),
                        duplicateTrace.checkRows.size(),
                        duplicateCheckHash.value(),
                        duplicateDecisionHash.value(),
                        overlapTrace.checkRows.size(),
                        overlapCheckHash.value(),
                        overlapDecisionHash.value(),
                        tallyHash.value(),
                        staffHeadHash.value(),
                        staffHash.value());
                pageHash.add(summary);
                systemHash.add(summary);
                System.out.println(summary);

                pageInputs += inputOrdinals.size();
                pageDuplicates += duplicates;
                pageOverlaps += overlaps;
                pageStaffHeads += heads.size();
                systemInputs += inputOrdinals.size();
                systemDuplicates += duplicates;
                systemOverlaps += overlaps;
                systemStaffHeads += heads.size();
            }

            final SIGraph sig = system.getSig();
            final List<Inter> headsBeforeBeam = sig.inters(HeadInter.class);
            Collections.sort(headsBeforeBeam, Inters.byOrdinate);
            final IdentityHashMap<Inter, Integer> headOrdinals = identityOrdinalsInter(
                    headsBeforeBeam);
            final int minBeamWidth = intField(field(builder, "params"), "minBeamWidth");
            final List<Inter> beamsBefore = sig.inters(
                    inter -> ShapeSet.Beams.contains(inter.getShape())
                            && inter.getBounds().width < minBeamWidth);
            final IdentityHashMap<Inter, Integer> beamOrdinals = identityOrdinalsInter(beamsBefore);
            final RowHasher beamInputHash = new RowHasher();
            for (Inter beam : beamsBefore) {
                final String row = String.format(
                        "headpostbeaminput %s system %d ordinal %d %s",
                        page,
                        system.getId(),
                        beamOrdinals.get(beam),
                        interRecord(beam));
                pageHash.add(row);
                systemHash.add(row);
                beamInputHash.add(row);
                if (fullTrace) {
                    System.out.println(row);
                }
            }
            invoke(builder, "purgeSmallBeams");
            int purgedBeams = 0;
            int purgedHeads = 0;
            final RowHasher beamDecisionHash = new RowHasher();
            for (Inter beam : beamsBefore) {
                if (!beam.isRemoved()) {
                    continue;
                }
                purgedBeams++;
                final String row = String.format(
                        "headpostbeampurge %s system %d beam %d contextual %s %s",
                        page,
                        system.getId(),
                        beamOrdinals.get(beam),
                        hexDouble(beam.getContextualGrade()),
                        interRecord(beam));
                pageHash.add(row);
                systemHash.add(row);
                beamDecisionHash.add(row);
                System.out.println(row);
            }
            for (Inter head : headsBeforeBeam) {
                if (!head.isRemoved()) {
                    continue;
                }
                purgedHeads++;
                final String row = String.format(
                        "headpostheadpurge %s system %d head %d %s",
                        page,
                        system.getId(),
                        headOrdinals.get(head),
                        interRecord(head));
                pageHash.add(row);
                systemHash.add(row);
                beamDecisionHash.add(row);
                System.out.println(row);
            }

            final List<Inter> finalHeads = sig.inters(HeadInter.class);
            Collections.sort(finalHeads, Inters.byOrdinate);
            final RowHasher finalHeadHash = new RowHasher();
            for (int ordinal = 0; ordinal < finalHeads.size(); ordinal++) {
                final String row = String.format(
                        "headpostsystemhead %s system %d ordinal %d %s",
                        page,
                        system.getId(),
                        ordinal,
                        interRecord(finalHeads.get(ordinal)));
                pageHash.add(row);
                systemHash.add(row);
                finalHeadHash.add(row);
                System.out.println(row);
            }
            final String summary = String.format(
                    "headpostsystemsummary %s system %d inputs %d duplicates %d overlaps %d "
                            + "staffHeads %d beamInputs %d purgedBeams %d purgedHeads %d "
                            + "finalHeads %d beamInputHash %016x beamDecisionHash %016x "
                            + "finalHeadHash %016x hash %016x",
                    page,
                    system.getId(),
                    systemInputs,
                    systemDuplicates,
                    systemOverlaps,
                    systemStaffHeads,
                    beamsBefore.size(),
                    purgedBeams,
                    purgedHeads,
                    finalHeads.size(),
                    beamInputHash.value(),
                    beamDecisionHash.value(),
                    finalHeadHash.value(),
                    systemHash.value());
            pageHash.add(summary);
            System.out.println(summary);
            pagePurgedBeams += purgedBeams;
            pagePurgedHeads += purgedHeads;
            pageFinalHeads += finalHeads.size();
        }

        // Exact HeadsStep.doEpilog order. No linking is performed by HEADS.
        sheet.getPicture().discardImage(Picture.ImageKey.HEAD_SPOTS);
        HeadSeedTally.analyze(sheet, tallies.values());
        final HeadSeedScale headSeedScale = sheet.getScale().getHeadSeedScale();
        final RowHasher scaleHash = new RowHasher();
        for (Shape shape : Shape.values()) {
            for (HorizontalSide side : HorizontalSide.values()) {
                final Double dx = (headSeedScale == null) ? null : headSeedScale.getDx(shape, side);
                if (dx == null) {
                    continue;
                }
                final String row = String.format(
                        "headpostscale %s shape %s side %s dx %s",
                        page,
                        shape,
                        side,
                        hexDouble(dx));
                pageHash.add(row);
                scaleHash.add(row);
                System.out.println(row);
            }
        }
        System.out.printf(
                "headpostpagesummary %s staffs %d inputs %d duplicates %d overlaps %d "
                        + "staffHeads %d purgedBeams %d purgedHeads %d finalHeads %d "
                        + "scaleHash %016x hash %016x%n",
                page,
                pageStaffs,
                pageInputs,
                pageDuplicates,
                pageOverlaps,
                pageStaffHeads,
                pagePurgedBeams,
                pagePurgedHeads,
                pageFinalHeads,
                scaleHash.value(),
                pageHash.value());
    }

    private static PurgeTrace tracePurge (SystemInfo system,
                                          Staff staff,
                                          List<HeadInter> heads,
                                          IdentityHashMap<HeadInter, Integer> ordinals,
                                          IdentityHashMap<HeadInter, String> origins,
                                          HeadSeedTally tally,
                                          InterPairPredicate predicate,
                                          boolean doRemove,
                                          String operation)
    {
        final PurgeTrace trace = new PurgeTrace(heads);
        for (HeadInter head : heads) {
            trace.leftDx.put(head, tally.getDx(head, HorizontalSide.LEFT));
            trace.rightDx.put(head, tally.getDx(head, HorizontalSide.RIGHT));
        }

        LeftLoop:
        for (int leftIndex = 0, limit = heads.size() - 1; leftIndex < limit; leftIndex++) {
            final HeadInter left = heads.get(leftIndex);
            if (trace.removed.containsKey(left)) {
                continue;
            }
            final Rectangle leftBox = left.getBounds();
            final int xMax = leftBox.x + leftBox.width - 1;
            for (int rightIndex = leftIndex + 1; rightIndex < heads.size(); rightIndex++) {
                final HeadInter right = heads.get(rightIndex);
                if (trace.removed.containsKey(right)) {
                    continue;
                }
                final Rectangle rightBox = right.getBounds();
                final boolean intersects = leftBox.intersects(rightBox);
                final boolean matches = intersects && predicate.test(left, right);
                final boolean breaks = !intersects && (rightBox.x > xMax);
                trace.checkRows.add(String.format(
                        "headpostcheck system %d staff %d operation %s ordinal %d left %d:%s "
                                + "right %d:%s intersects %s matches %s breaks %s",
                        system.getId(),
                        staff.getId(),
                        operation,
                        trace.checkRows.size(),
                        ordinals.get(left),
                        origins.get(left),
                        ordinals.get(right),
                        origins.get(right),
                        intersects,
                        matches,
                        breaks));
                if (matches) {
                    final Choice choice = choosePurged(left, right, trace.leftDx, trace.rightDx);
                    final PurgeDecision decision = new PurgeDecision(
                            left, right, choice.purged, choice.purged == left ? right : left);
                    trace.decisions.add(decision);
                    trace.decisionRows.add(String.format(
                            "headpost%s system %d staff %d ordinal %d "
                                    + "left %d:%s right %d:%s diff %s choice %s purged %d kept %d "
                                    + "leftTally %s:%s rightTally %s:%s",
                            operation,
                            system.getId(),
                            staff.getId(),
                            trace.decisions.size() - 1,
                            ordinals.get(left),
                            origins.get(left),
                            ordinals.get(right),
                            origins.get(right),
                            hexDouble(Math.abs(left.getGrade() - right.getGrade())),
                            choice.reason,
                            ordinals.get(choice.purged),
                            ordinals.get(decision.kept),
                            nullableDouble(trace.leftDx.get(left)),
                            nullableDouble(trace.rightDx.get(left)),
                            nullableDouble(trace.leftDx.get(right)),
                            nullableDouble(trace.rightDx.get(right))));
                    if (doRemove) {
                        trace.removed.put(choice.purged, Boolean.TRUE);
                        if (choice.purged == left) {
                            continue LeftLoop;
                        }
                    }
                }
                if (breaks) {
                    break;
                }
            }
        }
        if (doRemove) {
            for (HeadInter head : heads) {
                if (!trace.removed.containsKey(head)) {
                    trace.survivors.add(head);
                }
            }
        } else {
            trace.survivors.addAll(heads);
        }
        return trace;
    }

    private static Choice choosePurged (HeadInter left,
                                        HeadInter right,
                                        IdentityHashMap<HeadInter, Double> leftDx,
                                        IdentityHashMap<HeadInter, Double> rightDx)
    {
        final double diff = Math.abs(left.getGrade() - right.getGrade());
        if (diff >= EPSILON) {
            return (left.getGrade() < right.getGrade())
                    ? new Choice(left, "lower-grade-left")
                    : new Choice(right, "lower-grade-right");
        }
        final Double ll = leftDx.get(left);
        final Double lr = rightDx.get(left);
        if ((ll == null) && (lr == null)) {
            return new Choice(left, "equal-left-no-seed");
        }
        final Double rl = leftDx.get(right);
        final Double rr = rightDx.get(right);
        if ((rl == null) && (rr == null)) {
            return new Choice(right, "equal-right-no-seed");
        }
        if (left.getShape() != right.getShape()) {
            return new Choice(right, "equal-shape");
        }
        if (!left.getBounds().equals(right.getBounds())) {
            return new Choice(right, "equal-bounds");
        }
        if (left.isGood() && right.isGood()) {
            if ((ll == null) && (rl != null)) {
                leftDx.put(left, rl);
            }
            if ((lr == null) && (rr != null)) {
                rightDx.put(left, rr);
            }
        }
        return new Choice(right, "equal-preserve-left");
    }

    private static void assertPurge (String page,
                                     SystemInfo system,
                                     Staff staff,
                                     List<HeadInter> actual,
                                     PurgeTrace trace,
                                     int count,
                                     HeadSeedTally tally)
    {
        if ((count != trace.decisions.size()) || (actual.size() != trace.survivors.size())) {
            throw new IllegalStateException(String.format(
                    "%s system %d staff %d purge count/list mismatch",
                    page,
                    system.getId(),
                    staff.getId()));
        }
        for (int index = 0; index < actual.size(); index++) {
            if (actual.get(index) != trace.survivors.get(index)) {
                throw new IllegalStateException("production purge survivor order differs");
            }
        }
        for (HeadInter head : trace.inputs) {
            if (!sameDouble(tally.getDx(head, HorizontalSide.LEFT), trace.leftDx.get(head))
                    || !sameDouble(tally.getDx(head, HorizontalSide.RIGHT), trace.rightDx.get(head))) {
                throw new IllegalStateException("production purge tally differs from trace");
            }
        }
    }

    private static boolean sameDouble (Double left,
                                       Double right)
    {
        return (left == null) ? right == null
                : (right != null)
                        && (Double.doubleToLongBits(left) == Double.doubleToLongBits(right));
    }

    private static Exclusion exclusionBetween (SIGraph sig,
                                               Inter one,
                                               Inter two)
    {
        Relation relation = sig.getRelation(one, two, Exclusion.class);
        if (relation == null) {
            relation = sig.getRelation(two, one, Exclusion.class);
        }
        return (Exclusion) relation;
    }

    private static String inputRow (String page,
                                    SystemInfo system,
                                    Staff staff,
                                    int ordinal,
                                    String origin,
                                    HeadInter head,
                                    HeadSeedTally tally)
    {
        return String.format(
                "headpostinput %s system %d staff %d ordinal %d origin %s %s tally %s:%s",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                origin,
                interRecord(head),
                nullableDouble(tally.getDx(head, HorizontalSide.LEFT)),
                nullableDouble(tally.getDx(head, HorizontalSide.RIGHT)));
    }

    private static String staffHeadRow (String page,
                                        SystemInfo system,
                                        Staff staff,
                                        int inputOrdinal,
                                        String origin,
                                        HeadInter head,
                                        double gradeBefore,
                                        boolean stemless,
                                        HeadSeedTally tally)
    {
        return String.format(
                "headpoststaffhead %s system %d staff %d input %d origin %s stemless %s "
                        + "gradeBefore %s gradeAfter %s %s tally %s:%s",
                page,
                system.getId(),
                staff.getId(),
                inputOrdinal,
                origin,
                stemless,
                hexDouble(gradeBefore),
                hexDouble(head.getGrade()),
                interRecord(head),
                nullableDouble(tally.getDx(head, HorizontalSide.LEFT)),
                nullableDouble(tally.getDx(head, HorizontalSide.RIGHT)));
    }

    private static String interRecord (Inter inter)
    {
        final Rectangle box = inter.getBounds();
        final StringBuilder record = new StringBuilder(String.format(
                "class %s shape %s bounds %s grade %s best %s contextual %s good %s "
                        + "removed %s impacts %s",
                inter.getClass().getSimpleName(),
                inter.getShape(),
                rectangle(box),
                hexDouble(inter.getGrade()),
                hexDouble(inter.getBestGrade()),
                nullableDouble(inter.getContextualGrade()),
                inter.isGood(),
                inter.isRemoved(),
                (inter instanceof HeadInter head) ? impacts(head.getImpacts()) : "-"));
        final Glyph glyph = inter.getGlyph();
        if (glyph == null) {
            record.append(" glyph -");
        } else {
            record.append(String.format(
                    " glyph bounds:%s:weight:%d:runs:%016x",
                    rectangle(glyph.getBounds()),
                    glyph.getWeight(),
                    runTableHash(glyph)));
        }
        if (inter instanceof AbstractBeamInter beam) {
            record.append(" beamWidth ").append(box.width)
                    .append(" meanHeight ").append(hexDouble(beam.getHeight()));
        }
        return record.toString();
    }

    private static String impacts (GradeImpacts impacts)
    {
        if (impacts == null) {
            return "-";
        }
        final StringBuilder record = new StringBuilder();
        for (int index = 0; index < impacts.getImpactCount(); index++) {
            if (!record.isEmpty()) {
                record.append(',');
            }
            record.append(impacts.getName(index)).append(':')
                    .append(hexDouble(impacts.getImpact(index))).append(':')
                    .append(hexDouble(impacts.getWeight(index)));
        }
        return record.toString();
    }

    private static long runTableHash (Glyph glyph)
    {
        final List<String> rows = new ArrayList<>();
        final var table = glyph.getRunTable();
        rows.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<org.audiveris.omr.run.Run> it = table.iterator(sequence); it.hasNext();) {
                final var run = it.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            rows.add(row.toString());
        }
        return hash(rows);
    }

    private static int tallySize (HeadSeedTally tally)
        throws Exception
    {
        int count = 0;
        final Map<?, Map<?, ?>> data = (Map<?, Map<?, ?>>) field(tally, "data");
        for (Map<?, ?> side : data.values()) {
            count += side.size();
        }
        return count;
    }

    private static int staffNoteCount (Staff staff)
        throws Exception
    {
        final Collection<?> notes = (Collection<?>) field(staff, "notes");
        return (notes == null) ? 0 : notes.size();
    }

    private static boolean staffContainsAll (Staff staff,
                                             List<HeadInter> heads)
        throws Exception
    {
        final Collection<?> notes = (Collection<?>) field(staff, "notes");
        if (notes == null) {
            return heads.isEmpty();
        }
        for (HeadInter head : heads) {
            boolean found = false;
            for (Object note : notes) {
                if (note == head) {
                    found = true;
                    break;
                }
            }
            if (!found) {
                return false;
            }
        }
        return true;
    }

    private static <T> IdentityHashMap<T, Integer> identityOrdinals (List<T> values)
    {
        final IdentityHashMap<T, Integer> ordinals = new IdentityHashMap<>();
        for (int index = 0; index < values.size(); index++) {
            ordinals.put(values.get(index), index);
        }
        return ordinals;
    }

    private static IdentityHashMap<Inter, Integer> identityOrdinalsInter (List<Inter> values)
    {
        return identityOrdinals(values);
    }

    @SuppressWarnings("unchecked")
    private static List<HeadInter> processStaff (NoteHeadsBuilder builder,
                                                 Staff staff,
                                                 boolean useSeeds)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(
                "processStaff", Staff.class, boolean.class);
        method.setAccessible(true);
        return (List<HeadInter>) method.invoke(builder, staff, useSeeds);
    }

    private static int purge (NoteHeadsBuilder builder,
                              List<HeadInter> heads,
                              String operation,
                              InterPairPredicate predicate,
                              boolean doRemove)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(
                "purge", List.class, String.class, InterPairPredicate.class, boolean.class);
        method.setAccessible(true);
        return ((Number) method.invoke(builder, heads, operation, predicate, doRemove)).intValue();
    }

    private static void prepareBuilder (NoteHeadsBuilder builder,
                                        SystemInfo system,
                                        List<Glyph> systemSpots)
        throws Exception
    {
        setField(builder, "systemBarAreas", invoke(builder, "getSystemBarAreas"));
        setField(builder, "systemCompetitors", invoke(builder, "getSystemCompetitors"));
        final List<Glyph> systemSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        Collections.sort(systemSeeds, Glyphs.byOrdinate);
        Collections.sort(systemSpots, Glyphs.byOrdinate);
        setField(builder, "systemSeeds", systemSeeds);
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

    private static Object invoke (Object target,
                                  String name)
        throws Exception
    {
        final Method method = target.getClass().getDeclaredMethod(name);
        method.setAccessible(true);
        return method.invoke(target);
    }

    private static Object field (Object target,
                                 String name)
        throws Exception
    {
        Class<?> type = target.getClass();
        while (type != null) {
            try {
                final Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                return field.get(target);
            } catch (NoSuchFieldException ex) {
                type = type.getSuperclass();
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static void setField (Object target,
                                  String name,
                                  Object value)
        throws Exception
    {
        final Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    private static int intField (Object target,
                                 String name)
        throws Exception
    {
        return ((Number) field(target, name)).intValue();
    }

    private static String rectangle (Rectangle box)
    {
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String nullableDouble (Double value)
    {
        return (value == null) ? "-" : hexDouble(value);
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static long hash (Collection<String> rows)
    {
        final RowHasher hash = new RowHasher();
        for (String row : rows) {
            hash.add(row);
        }
        return hash.value();
    }

    private static void addSemantic (String row,
                                     RowHasher... hashes)
    {
        for (RowHasher hash : hashes) {
            hash.add(row);
        }
    }

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) HEADS post-range oracle.");
        System.out.println("#");
        System.out.println("# Each target reaches LEDGERS, runs production seed and range lookup, then");
        System.out.println("# invokes the real private duplicate purge, overlap exclusion insertion,");
        System.out.println("# tally purge, stemless boost, staff attachment, and small-beam purge in");
        System.out.println("# NoteHeadsBuilder.buildHeads order. HeadsStep image discard and tally analysis");
        System.out.println("# also run. HEADS performs no linking operation.");
        System.out.println("#");
        System.out.println("# Default output retains purge decisions and boundary survivors while hashing");
        System.out.println("# every ordered input and pair check. Pass --full-trace for those verbose rows.");
        System.out.println("# All identities are per-boundary ordinals; doubles include hex text/raw bits.");
        System.out.println("# FNV-1a-64 hashes cover canonical UTF-8 rows with trailing newlines.");
    }

    private static final class Choice
    {
        final HeadInter purged;

        final String reason;

        Choice (HeadInter purged,
                String reason)
        {
            this.purged = purged;
            this.reason = reason;
        }
    }

    private static final class PurgeDecision
    {
        final HeadInter left;

        final HeadInter right;

        final HeadInter purged;

        final HeadInter kept;

        PurgeDecision (HeadInter left,
                       HeadInter right,
                       HeadInter purged,
                       HeadInter kept)
        {
            this.left = left;
            this.right = right;
            this.purged = purged;
            this.kept = kept;
        }
    }

    private static final class PurgeTrace
    {
        final List<HeadInter> inputs;

        final IdentityHashMap<HeadInter, Boolean> removed = new IdentityHashMap<>();

        final IdentityHashMap<HeadInter, Double> leftDx = new IdentityHashMap<>();

        final IdentityHashMap<HeadInter, Double> rightDx = new IdentityHashMap<>();

        final List<String> checkRows = new ArrayList<>();

        final List<String> decisionRows = new ArrayList<>();

        final List<PurgeDecision> decisions = new ArrayList<>();

        final List<HeadInter> survivors = new ArrayList<>();

        PurgeTrace (List<HeadInter> inputs)
        {
            this.inputs = new ArrayList<>(inputs);
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
