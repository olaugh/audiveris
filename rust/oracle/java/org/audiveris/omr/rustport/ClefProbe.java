// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The HEADERS clef oracle: what Java decides, per staff, on the eight-sheet corpus.
//
// Unlike `MusicFontScout`, which measures a pure function of the font, this has to run the real
// pipeline -- LOAD, BINARY, SCALE, GRID, HEADERS -- because the question is not "what box does
// this shape have" but "which shape did Java choose for the ink actually on this page". So it
// drives a `SheetStub` to HEADERS exactly as the batch CLI does, and reads the resulting
// `StaffHeader` rather than reconstructing anything.
//
// Three fields are here specifically because they are where the Rust port is most likely to
// diverge, and none of them is visible to a unit test:
//
//   - `specific` is `staff.getSpecificInterline()`, which `ClefBuilder` hands the classifier,
//     while `MusicFont.getPointSize` reads the *sheet* interline. On a sheet carrying two staff
//     sizes those differ, and this corpus has such sheets. Emitting both makes it checkable.
//   - `clefStop` is the value `getHeaderStop` is built from, and so the value BEAMS and
//     `StemScaler` have been waiting on.
//   - the glyph box and weight identify *which* ink was classified, so a shape mismatch can be
//     told apart from a part-assembly mismatch. Without it a disagreement is unattributable.
package org.audiveris.omr.rustport;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.header.StaffHeader;
import org.audiveris.omr.sig.inter.ClefInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.step.OmrStep;

import java.awt.Rectangle;
import java.nio.file.Path;
import java.util.Locale;

public class ClefProbe
{
    /** The corpus, in the order `grid-sig.txt` lists it, so the two oracles read side by side. */
    private static final String[] SHEETS = {
        "D0392410-1.256.png",
        "allegretto.png",
        "batuque.png",
        "carmen.png",
        "chula.png",
        "cucaracha.png",
        "hove.png",
        "zizi.png",
        "BachInvention5.jpg"
    };

    public static void main (String[] args)
            throws Exception
    {
        installBatchCli();
        System.out.println("# Java Audiveris 5.11 HEADERS clef decisions, one line per staff.");
        System.out.println("# Produced by ClefProbe, which drives each sheet to HEADERS in-process.");
        System.out.println("#");
        System.out.println("# page <file> <sheet-interline>");
        System.out.println("# staff <id> <specific-interline> <header-start> <header-stop> <clef-stop>"
                + " <shape> <grade> <symbol-x> <symbol-y> <symbol-w> <symbol-h>"
                + " <glyph-x> <glyph-y> <glyph-w> <glyph-h> <glyph-weight>");
        System.out.println("# candidate <staff-id> <shape> <grade> <ctx-grade> <removed>"
                + " <symbol-x> <symbol-y> <symbol-w> <symbol-h>"
                + " <glyph-x> <glyph-y> <glyph-w> <glyph-h>");
        System.out.println("# key <staff-id> <fifths|NONE> <grade> <x> <y> <w> <h> <key-stop>");
        System.out.println("# alter <staff-id> <index> <x> <y> <w> <h> <grade>");
        System.out.println("# time <staff-id> <shape|NONE> <rational> <grade> <x> <y> <w> <h>"
                + " <time-stop>");

        for (String name : SHEETS) {
            // The working directory is app/, because `WellKnowns.RES_URI` resolves the fonts
            // and the classifier relative to it. The corpus lives at the repository root.
            final Path path = Path.of("..", "data", "examples", name);
            // Deliberately not closed: `Book.close` goes through `OMR.engine`, which only exists
            // under the GUI or the CLI. The process is short-lived and exits after the sweep.
            // `new Book(inputPath)` is the image-input constructor, the one `BookManager.loadInput`
            // uses. `Book.createBook` is for a *target* .omr path and makes `createStubs` try to
            // browse the image as a project archive.
            final Book book = new Book(path);
            {
                // Not `book.createStubs()`: it reaches for `Main.getCli()` to decide output
                // folders and saving, and there is no CLI in this process. A single stub is
                // enough -- every corpus entry is one page.
                final SheetStub stub = new SheetStub(book, 1);
                book.addStub(stub);
                stub.reachStep(OmrStep.HEADERS, false);
                final Sheet sheet = stub.getSheet();
                System.out.printf(
                        Locale.ROOT,
                        "page %s %d%n",
                        name,
                        sheet.getInterline());

                for (SystemInfo system : sheet.getSystems()) {
                    for (Staff staff : system.getStaves()) {
                        emitStaff(staff);
                        emitCandidates(sheet, staff);
                        emitKeyAndTime(staff);
                    }
                }
            }
        }
    }

    /**
     * Emits the header's key and time, or their absence.
     *
     * <p>Absence is the point as much as presence: before porting `KeyBuilder` and `TimeBuilder`
     * it is worth knowing how much of this corpus exercises them at all. A stage with two examples
     * on nine pages needs a different plan from one with sixty.
     */
    private static void emitKeyAndTime (Staff staff)
    {
        final StaffHeader header = staff.getHeader();

        if (header == null) {
            return;
        }

        if (header.key != null) {
            System.out.printf(
                    Locale.ROOT,
                    "key %d %s %s %s %s%n",
                    staff.getId(),
                    header.key.getFifths(),
                    Double.toString(header.key.getGrade()),
                    box(header.key.getBounds()),
                    value(staff.getKeyStop()));

            // Per-alteration boxes, so a one-member divergence in the union box can be
            // attributed to the member rather than argued about.
            int index = 0;
            for (org.audiveris.omr.sig.inter.Inter member : header.key.getMembers()) {
                System.out.printf(
                        Locale.ROOT,
                        "alter %d %d %s %s%n",
                        staff.getId(),
                        index++,
                        box(member.getBounds()),
                        Double.toString(member.getGrade()));
            }
        } else {
            System.out.printf("key %d NONE - - - - - %s%n", staff.getId(), value(staff.getKeyStop()));
        }

        if (header.time != null) {
            // A null shape is not an error: `TimePairInter` and `TimeCustomInter` carry no single
            // Shape and are described only by their rational. Emitting both columns keeps those
            // distinguishable from a shaped COMMON_TIME.
            System.out.printf(
                    Locale.ROOT,
                    "time %d %s %s %s %s %s%n",
                    staff.getId(),
                    header.time.getShape(),
                    header.time.getTimeRational(),
                    Double.toString(header.time.getGrade()),
                    box(header.time.getBounds()),
                    value(staff.getTimeStop()));
        } else {
            System.out.printf(
                    "time %d NONE - - - - - - %s%n",
                    staff.getId(),
                    value(staff.getTimeStop()));
        }
    }

    /**
     * Emits every clef candidate registered for a staff, survivor or not.
     *
     * <p>This exists because `clefStop` is <em>not</em> a property of the clef the header ends up
     * with. `registerClefs` sets it from the candidate at index 0 of a list sorted by *intrinsic*
     * grade, whereas `selectClef` later picks the header clef by *contextual* grade and deletes
     * the rest. When those two orders disagree, the staff's `clefStop` describes an inter that no
     * longer exists, and no amount of comparing against the survivor's box will reproduce it.
     *
     * <p>The deleted candidates are still reachable: `Inter.remove()` unlinks the vertex from the
     * SIG but the sheet's `InterIndex` retains the object, so it can be read back with its grade,
     * its contextual grade and its bounds.
     */
    private static void emitCandidates (Sheet sheet,
                                        Staff staff)
    {
        for (Inter inter : sheet.getInterIndex().getEntities()) {
            if (!(inter instanceof ClefInter clef) || (clef.getStaff() != staff)) {
                continue;
            }

            final Rectangle box = clef.getBounds();
            final Rectangle glyph = (clef.getGlyph() != null) ? clef.getGlyph().getBounds() : null;
            System.out.printf(
                    Locale.ROOT,
                    "candidate %d %s %s %s %b %s %s%n",
                    staff.getId(),
                    clef.getShape(),
                    Double.toString(clef.getGrade()),
                    Double.toString(clef.getContextualGrade()),
                    clef.isRemoved(),
                    box(box),
                    (glyph != null) ? box(glyph) : "- - - -");
        }
    }

    /**
     * Installs a batch CLI into {@code Main}, reflectively.
     *
     * <p>`reachStep` and `createStubs` both consult {@code Main.getCli()} -- for the output folder
     * and for whether to save -- and it is a private static with no setter, populated only by
     * {@code Main.main}. Running the pipeline in-process is what makes this oracle able to read
     * live {@code StaffHeader} objects instead of re-parsing a saved .omr, so the reflection buys
     * something real.
     */
    private static void installBatchCli ()
            throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADERS");
        final java.lang.reflect.Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);
    }

    private static void emitStaff (Staff staff)
    {
        final StaffHeader header = staff.getHeader();
        final ClefInter clef = (header != null) ? header.clef : null;

        // A staff with no clef is a real outcome, not a probe failure, and the Rust side has to
        // reproduce the absence too -- so it gets a line rather than being skipped.
        if (clef == null) {
            System.out.printf(
                    Locale.ROOT,
                    "staff %d %d %s %s %s NONE%n",
                    staff.getId(),
                    staff.getSpecificInterline(),
                    (header != null) ? Integer.toString(header.start) : "-",
                    (header != null) ? Integer.toString(header.stop) : "-",
                    value(staff.getClefStop()));
            return;
        }

        final Rectangle symbol = clef.getBounds();
        final Rectangle glyph = (clef.getGlyph() != null) ? clef.getGlyph().getBounds() : null;
        System.out.printf(
                Locale.ROOT,
                "staff %d %d %d %d %s %s %s %s %s%n",
                staff.getId(),
                staff.getSpecificInterline(),
                staff.getHeader().start,
                staff.getHeader().stop,
                value(staff.getClefStop()),
                clef.getShape(),
                // Grades are compared as text, so print the raw double rather than a rounded one:
                // this oracle exists to catch a *different* clef, but a drifting grade on the same
                // clef is worth seeing before it becomes a different one.
                Double.toString(clef.getGrade()),
                box(symbol),
                (glyph != null)
                        ? box(glyph) + " " + clef.getGlyph().getWeight()
                        : "- - - - -");
    }

    private static String box (Rectangle rectangle)
    {
        return rectangle == null ? "- - - -"
                : String.format(
                        Locale.ROOT,
                        "%d %d %d %d",
                        rectangle.x,
                        rectangle.y,
                        rectangle.width,
                        rectangle.height);
    }

    private static String value (Integer value)
    {
        return value == null ? "-" : Integer.toString(value);
    }
}
