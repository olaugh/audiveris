import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.nio.file.Paths;
import org.audiveris.omr.OMR;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.KeyAlterInter;
import org.audiveris.omr.step.OmrStep;

/**
 * Print every KeyAlterInter's grade and measured pitch as raw f64 bits, plus the
 * staff-line ordinates and glyph reference points the pitch is built from.
 *
 * The port matches Java on six of chula system 1's ten header grades; the four
 * that differ do so by 2-3 ulp, and the two staff-1 alters differ by *different*
 * ulp counts, which points at per-glyph input rather than a shared scale. This
 * prints the inputs so the divergence can be located instead of reasoned about.
 */
public final class KeyAlterPitchBits {
    private static String bits (double value) {
        return Long.toHexString(Double.doubleToRawLongBits(value));
    }

    public static void main (String[] args) throws Exception {
        final org.audiveris.omr.CLI cli =
                new org.audiveris.omr.CLI(org.audiveris.omr.WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADERS");
        final java.lang.reflect.Field cliField =
                org.audiveris.omr.Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        OMR.engine = org.audiveris.omr.sheet.BookManager.getInstance();
        final Book book = OMR.engine.loadInput(Paths.get(args[0]));
        book.createStubs();
        final var stub = book.getStubs().get(0);
        book.reachBookStep(OmrStep.HEADERS, false, java.util.List.of(stub), false);
        final Sheet sheet = stub.getSheet();

        for (SystemInfo system : sheet.getSystems()) {
            for (Inter inter : system.getSig().vertexSet()) {
                if (!(inter instanceof KeyAlterInter alter)) continue;
                final Staff staff = alter.getStaff();
                final Rectangle box = alter.getBounds();
                final var glyph = alter.getGlyph();
                final StringBuilder line = new StringBuilder("keyalterpitch system ")
                        .append(system.getId())
                        .append(" staff ").append(staff.getId())
                        .append(" shape ").append(alter.getShape())
                        .append(" bounds ").append(box.x).append(':').append(box.y)
                        .append(':').append(box.width).append(':').append(box.height)
                        .append(" grade ").append(bits(alter.getGrade()))
                        .append(" measuredPitch ").append(bits(alter.getMeasuredPitch()));

                if (glyph != null) {
                    final java.awt.Point centroid = glyph.getCentroid();
                    final Point2D center = glyph.getCenter2D();
                    line.append(" centroid ").append(centroid.x).append(':').append(centroid.y)
                        .append(" center ").append(bits(center.getX()))
                        .append('/').append(bits(center.getY()))
                        .append(" massPitch ")
                        .append(bits(staff.pitchPositionOf(centroid)))
                        .append(" geoPitch ")
                        .append(bits(staff.pitchPositionOf(center)))
                        .append(" topAtCentroid ")
                        .append(bits(staff.getFirstLine().yAt((double) centroid.x)))
                        .append(" bottomAtCentroid ")
                        .append(bits(staff.getLastLine().yAt((double) centroid.x)))
                        .append(" topAtCenter ")
                        .append(bits(staff.getFirstLine().yAt(center.getX())))
                        .append(" bottomAtCenter ")
                        .append(bits(staff.getLastLine().yAt(center.getX())));
                } else {
                    line.append(" glyph -");
                }
                System.out.println(line);
            }
        }
        System.exit(0);
    }
}
