import java.nio.file.Paths;
import org.audiveris.omr.OMR;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.LedgerInter;
import org.audiveris.omr.step.OmrStep;

/** Print every final ledger's grade and per-impact grades as raw f64 bits. */
public final class LedgerImpactBits {
    public static void main (String[] args) throws Exception {
        final org.audiveris.omr.CLI cli =
                new org.audiveris.omr.CLI(org.audiveris.omr.WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "LEDGERS");
        final java.lang.reflect.Field cliField =
                org.audiveris.omr.Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        OMR.engine = org.audiveris.omr.sheet.BookManager.getInstance();
        final Book book = OMR.engine.loadInput(Paths.get(args[0]));
        book.createStubs();
        final var stub = book.getStubs().get(0);
        book.reachBookStep(OmrStep.LEDGERS, false, java.util.List.of(stub), false);
        final Sheet sheet = stub.getSheet();
        for (SystemInfo system : sheet.getSystems()) {
            for (Inter inter : system.getSig().vertexSet()) {
                if (!(inter instanceof LedgerInter ledger)) continue;
                final GradeImpacts impacts = ledger.getImpacts();
                final StringBuilder line = new StringBuilder("ledgerbits system ")
                        .append(system.getId())
                        .append(" bounds ").append(ledger.getBounds().x)
                        .append(':').append(ledger.getBounds().y)
                        .append(" grade ")
                        .append(Long.toHexString(Double.doubleToRawLongBits(ledger.getGrade())));
                if (impacts != null) {
                    for (int i = 0; i < impacts.getImpactCount(); i++) {
                        line.append(" i").append(i).append('=')
                            .append(Long.toHexString(
                                Double.doubleToRawLongBits(impacts.getImpact(i))))
                            .append('w')
                            .append(Long.toHexString(
                                Double.doubleToRawLongBits(impacts.getWeight(i))));
                    }
                } else {
                    line.append(" impacts -");
                }
                System.out.println(line);
            }
        }
        System.exit(0);
    }
}
