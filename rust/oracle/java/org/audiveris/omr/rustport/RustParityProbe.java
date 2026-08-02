// SPDX-License-Identifier: AGPL-3.0-or-later

package org.audiveris.omr.rustport;

import java.util.Arrays;

import org.audiveris.omr.math.BasicLine;
import org.audiveris.omr.math.Histogram;
import org.audiveris.omr.math.InjectionSolver;
import org.audiveris.omr.math.Rational;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.sig.GradeUtil;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.NaturalSpec;
import org.audiveris.omr.util.WrappedInteger;

/** Emits stable vectors from the production Java classes for Rust parity checks. */
public final class RustParityProbe
{
    private RustParityProbe ()
    {
    }

    public static void main (String[] args)
    {
        System.out.println("natural.decode=" + NaturalSpec.decode("1 - 3 , 6", true));
        System.out.println("natural.encode=" + NaturalSpec.encode(Arrays.asList(5, 2, 4, 6, 7, 8, 10, 12)));

        Rational twoThirds = new Rational(2, 3);
        Rational oneHalf = new Rational(1, 2);
        System.out.println("rational.sum=" + twoThirds.plus(oneHalf));
        System.out.println("rational.gcd=" + Rational.gcd(twoThirds, new Rational(5, 4)));

        Histogram<Integer> histogram = new Histogram<>();
        int[][] buckets = {{3, 2}, {4, 10}, {5, 12}, {8, 3}, {10, 6}, {11, 0}};
        for (int[] bucket : buckets) {
            histogram.increaseCount(bucket[0], bucket[1]);
        }
        System.out.println("histogram.data=" + histogram.dataString());
        System.out.println("histogram.summary=" + histogram.getTotalCount() + "/" + histogram.getMaxBucket() + "/" + histogram.getMaxCount());

        BasicLine line = new BasicLine(
                new double[]{1, 2, 3, 4, 5},
                new double[]{4, 9, 14, 19, 24});
        // libm hypot can differ by one ULP across Java and Rust, so geometry vectors
        // use a declared 1e-15 decimal canonicalization boundary.
        System.out.printf(java.util.Locale.ROOT, "line.origin=%.15f%n", line.distanceOf(0, 0));
        System.out.printf(java.util.Locale.ROOT, "line.one-ten=%.15f%n", line.distanceOf(1, 10));

        double grade = GradeUtil.contextual(0.2, new double[]{0.5, 0.8}, new double[]{5.0, 2.0});
        System.out.printf(java.util.Locale.ROOT, "grade.contextual=%.17f%n", grade);

        InjectionSolver solver = new InjectionSolver(
                3,
                3,
                (domain, range, details) -> Math.abs((1 + domain) - range));
        WrappedInteger bestCost = new WrappedInteger(null);
        int[] mapping = solver.solve(bestCost);
        System.out.println("injection=" + Arrays.toString(mapping) + "/" + bestCost.value);

        RunTable runs = new RunTable(Orientation.HORIZONTAL, 10, 5);
        runs.addRun(0, new Run(1, 2));
        runs.addRun(0, new Run(5, 3));
        runs.addRun(1, new Run(0, 1));
        runs.addRun(1, new Run(4, 2));
        System.out.println("runs=" + runs.getTotalRunCount() + "/" + runs.getWeight() + "/" + runs.getRunAt(6, 0));

        System.out.println("pipeline=" + String.join(",", Arrays.stream(OmrStep.values()).map(Enum::name).toList()));
    }
}
