// SPDX-License-Identifier: AGPL-3.0-or-later

package org.audiveris.omr.rustport;

import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.awt.image.BufferedImage;
import java.awt.image.Raster;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

import ij.process.ByteProcessor;

import org.audiveris.omr.image.ChamferDistance;
import org.audiveris.omr.image.GlobalFilter;
import org.audiveris.omr.image.ImageLoading;
import org.audiveris.omr.image.MedianGrayFilter;
import org.audiveris.omr.image.VerticalFilter;
import org.audiveris.omr.image.WatershedGrayLevel;
import org.audiveris.omr.glyph.dynamic.FilamentFactory;
import org.audiveris.omr.glyph.dynamic.FilamentIndex;
import org.audiveris.omr.lag.BasicLag;
import org.audiveris.omr.lag.JunctionRatioPolicy;
import org.audiveris.omr.lag.Lag;
import org.audiveris.omr.lag.Lags;
import org.audiveris.omr.lag.LagManager;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.lag.SectionFactory;
import org.audiveris.omr.math.AreaUtil;
import org.audiveris.omr.math.BasicLine;
import org.audiveris.omr.math.GeoPath;
import org.audiveris.omr.math.Histogram;
import org.audiveris.omr.math.InjectionSolver;
import org.audiveris.omr.math.IntegerFunction;
import org.audiveris.omr.math.NaturalSpline;
import org.audiveris.omr.math.Projection;
import org.audiveris.omr.math.Range;
import org.audiveris.omr.math.Rational;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.score.PageNumber;
import org.audiveris.omr.score.PageRef;
import org.audiveris.omr.score.Score;
import org.audiveris.omr.sig.GradeUtil;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.OneLineStaff;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.ScaleBuilder;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Skew;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.BarAlignment;
import org.audiveris.omr.sheet.grid.BarColumn;
import org.audiveris.omr.sheet.grid.BarConnection;
import org.audiveris.omr.sheet.grid.BarsRetriever;
import org.audiveris.omr.sheet.grid.ClustersRetriever;
import org.audiveris.omr.sheet.grid.FilamentComb;
import org.audiveris.omr.sheet.grid.GridBuilder;
import org.audiveris.omr.sheet.grid.LineCluster;
import org.audiveris.omr.sheet.grid.PeakGraph;
import org.audiveris.omr.sheet.grid.StaffFilament;
import org.audiveris.omr.sheet.grid.StaffPattern;
import org.audiveris.omr.sheet.grid.StaffPeak;
import org.audiveris.omr.sheet.grid.StaffProjector;
import org.audiveris.omr.sheet.grid.TargetLine;
import org.audiveris.omr.sheet.grid.TargetStaff;
import org.audiveris.omr.sheet.grid.TargetSystem;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.NaturalSpec;
import org.audiveris.omr.util.Table;
import org.audiveris.omr.util.WrappedInteger;

/** Emits stable vectors from the production Java classes for Rust parity checks. */
public final class RustParityProbe
{
    private RustParityProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
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

        IntegerFunction integer = new IntegerFunction(2, 9);
        int[][] functionValues = {{2, 1}, {3, 4}, {4, 4}, {5, 2}, {6, 5}, {7, 1}, {8, 3}, {9, 3}};
        for (int[] entry : functionValues) {
            integer.setValue(entry[0], entry[1]);
        }
        System.out.println("integer.function=" + integer.argMax(2, 9) + "/" + integer.getArea() + "/" + integer.getLocalMaxima(0, 20) + "/" + integer.getDerivative(3));

        Projection.Short shortProjection = new Projection.Short(-3, 1);
        shortProjection.increment(-3, Short.MAX_VALUE);
        shortProjection.increment(-3);
        shortProjection.increment(-2, 65_537);
        shortProjection.increment(-1, -65_537);
        shortProjection.increment(0, Integer.MAX_VALUE);
        shortProjection.increment(1, Short.MIN_VALUE);
        List<Integer> shortValues = new ArrayList<>();
        for (int pos = shortProjection.getStart(); pos <= shortProjection.getStop(); pos++) {
            shortValues.add(shortProjection.getValue(pos));
        }
        List<Integer> shortDerivatives = new ArrayList<>();
        for (int pos = shortProjection.getStart() - 1; pos <= shortProjection.getStop(); pos++) {
            shortDerivatives.add(shortProjection.getDerivative(pos));
        }
        System.out.println(
                "projection.short.synthetic=" + shortProjection.getStart() + ":"
                        + shortProjection.getStop() + ":" + shortProjection.getLength()
                        + "/" + shortValues + "/" + shortDerivatives);

        Scale projectorScale = new Scale(
                new Scale.InterlineScale(10, 10, 10),
                new Scale.LineScale(1, 1, 1),
                null,
                null,
                null);
        int roundUpThreshold = staffDerivativeThreshold(
                projectorScale,
                new int[]{0, 5, 10, 15, 20, 25});
        int roundDownThreshold = staffDerivativeThreshold(
                projectorScale,
                new int[]{0, 15, 30, 45, 60, 75});
        Object staffProjectorConstants = staticField(StaffProjector.class, "constants");
        org.audiveris.omr.constant.Constant.Integer topDerivativeNumber =
                (org.audiveris.omr.constant.Constant.Integer) field(
                        staffProjectorConstants,
                        "topDerivativeNumber");
        int savedTopDerivativeNumber = topDerivativeNumber.getValue();
        int zeroTopThreshold;
        try {
            topDerivativeNumber.setValue(0);
            zeroTopThreshold = staffDerivativeThreshold(
                    projectorScale,
                    new int[]{0, 5, 10, 15, 20, 25});
        } finally {
            topDerivativeNumber.setValue(savedTopDerivativeNumber);
        }
        System.out.println(
                "grid.staff-projector-threshold.synthetic=ties:"
                        + roundUpThreshold + "," + roundDownThreshold
                        + ";top0:" + zeroTopThreshold);

        StaffProjector blankProjector = staffProjector(
                projectorScale,
                new int[]{0, 0, 1, 0, 0, 0, 0, 1, 0, 0});
        Method findAllBlanks = StaffProjector.class.getDeclaredMethod("findAllBlanks");
        findAllBlanks.setAccessible(true);
        findAllBlanks.invoke(blankProjector);
        List<String> blankRegions = new ArrayList<>();
        for (Object blank : (List<?>) field(blankProjector, "allBlanks")) {
            blankRegions.add(blank(blank));
        }
        Method selectBlank = StaffProjector.class.getDeclaredMethod(
                "selectBlank",
                org.audiveris.omr.util.HorizontalSide.class,
                int.class,
                int.class);
        selectBlank.setAccessible(true);
        Object rightFromTwo = selectBlank.invoke(
                blankProjector,
                org.audiveris.omr.util.HorizontalSide.RIGHT,
                2,
                2);
        Object rightFromFour = selectBlank.invoke(
                blankProjector,
                org.audiveris.omr.util.HorizontalSide.RIGHT,
                4,
                2);
        Object leftFromSeven = selectBlank.invoke(
                blankProjector,
                org.audiveris.omr.util.HorizontalSide.LEFT,
                7,
                2);
        System.out.println(
                "grid.staff-projector-blanks.synthetic=all:"
                        + String.join(",", blankRegions)
                        + ";right2:" + blank(rightFromTwo)
                        + ";right4:" + blank(rightFromFour)
                        + ";left7:" + blank(leftFromSeven));

        StaffProjector tiedPeakProjector = staffProjector(
                projectorScale,
                new int[]{0, 0, 0, 0, 0, 10, 10, 10, 5, 0, 0});
        configurePeakThresholds(tiedPeakProjector, 2, 5);
        Object tiedPeakSide = refinePeakSide(tiedPeakProjector, 4, 7, 1, false, 4, 0);
        StaffProjector borderPeakProjector = staffProjector(
                projectorScale,
                new int[]{0, 0, 0, 0, 4, 4});
        configurePeakThresholds(borderPeakProjector, 2, 5);
        Object borderPeakSide = refinePeakSide(borderPeakProjector, 4, 5, 1, false, 3, 0);
        System.out.println(
                "grid.staff-projector-peak-side.synthetic=tie:"
                        + peakSide(tiedPeakSide)
                        + ";border:" + peakSide(borderPeakSide));

        int[] acceptedPeakCounts = {0, 0, 0, 10, 40, 40, 40, 40, 10, 0, 0, 0, 0};
        StaffProjector acceptedPeakProjector = staffProjector(
                projectorScale,
                acceptedPeakCounts,
                new int[]{0, 10, 20, 30, 40});
        configurePeakThresholds(acceptedPeakProjector, 2, 5);
        StaffPeak acceptedPeak = createPeak(acceptedPeakProjector, 4, 7, false, 10, 10, 0);
        int[] widePeakCounts = {
            0, 0, 0, 10, 40, 40, 40, 40, 40, 40, 40, 40, 40,
            40, 40, 40, 40, 40, 40, 40, 40, 10, 0, 0, 0, 0
        };
        StaffProjector widePeakProjector = staffProjector(
                projectorScale,
                widePeakCounts,
                new int[]{0, 10, 20, 30, 40});
        configurePeakThresholds(widePeakProjector, 2, 5);
        StaffPeak widePeak = createPeak(widePeakProjector, 4, 20, false, 10, 10, 0);
        StaffProjector missingPeakProjector = staffProjector(
                projectorScale,
                new int[10],
                new int[]{0, 10, 20, 30, 40});
        configurePeakThresholds(missingPeakProjector, 2, 5);
        StaffPeak missingPeak = createPeak(missingPeakProjector, 2, 3, false, 10, 10, 0);
        System.out.println(
                "grid.staff-projector-peak-candidate.synthetic=accepted:"
                        + staffPeakRange(acceptedPeak)
                        + ";overWidth:" + staffPeakRange(widePeak)
                        + ";missing:" + staffPeakRange(missingPeak));

        AreaUtil.CoreData acceptedCore = verticalCore(
                acceptedPeakProjector,
                4,
                7,
                0,
                40);
        RunTable gapCoreBinary = new RunTable(Orientation.VERTICAL, 13, 41);
        gapCoreBinary.addRun(3, new Run(0, 10));
        for (int x = 4; x <= 7; x++) {
            gapCoreBinary.addRun(x, new Run(0, 10));
            gapCoreBinary.addRun(x, new Run(20, 20));
        }
        gapCoreBinary.addRun(8, new Run(0, 10));
        StaffProjector gapCoreProjector = staffProjector(
                projectorScale,
                gapCoreBinary,
                new int[]{0, 10, 20, 30, 40});
        configurePeakThresholds(gapCoreProjector, 2, 5);
        StaffPeak gapCorePeak = createPeak(gapCoreProjector, 4, 7, false, 10, 10, 0);
        AreaUtil.CoreData gapCore = verticalCore(gapCoreProjector, 4, 7, 0, 40);
        StaffPeak serifCorePeak = createPeak(
                acceptedPeakProjector,
                4,
                7,
                false,
                10,
                10,
                4);
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.staff-projector-core.synthetic=accepted:%s:gap%d;gap:%s:gap%d;serif:%s:white%.12f%n",
                staffPeakRange(acceptedPeak),
                acceptedCore.gap,
                staffPeakRange(gapCorePeak),
                gapCore.gap,
                staffPeakRange(serifCorePeak),
                acceptedCore.whiteRatio);

        StaffProjector rejectedRangeProjector = staffProjector(
                projectorScale,
                new int[]{
                    0, 0, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
                    30, 30, 30, 30, 30, 30, 30, 0, 40, 0, 0
                },
                new int[]{0, 10, 20, 30, 40});
        configurePeakThresholds(rejectedRangeProjector, 2, 5);
        List<StaffPeak> rejectedBrowse = browseRange(
                rejectedRangeProjector,
                2,
                18,
                false,
                20,
                20,
                0);
        List<StaffPeak> afterRejected = findPeaksInRange(
                rejectedRangeProjector,
                0,
                21,
                "FULL",
                25,
                20,
                20,
                0);
        StaffProjector rightEdgeProjector;
        try {
            // A five-column sheet has only four derivative samples. The threshold is
            // irrelevant to this direct range test, so use Java's verified zero-top path.
            topDerivativeNumber.setValue(0);
            rightEdgeProjector = staffProjector(
                    projectorScale,
                    new int[]{0, 0, 20, 20, 20},
                    new int[]{0, 10, 20, 30, 40});
        } finally {
            topDerivativeNumber.setValue(savedTopDerivativeNumber);
        }
        configurePeakThresholds(rightEdgeProjector, 2, 5);
        List<StaffPeak> initialHalfEdge = findPeaksInRange(
                rightEdgeProjector,
                0,
                4,
                "INITIAL_HALF",
                12,
                10,
                10,
                0);
        List<StaffPeak> fullEdge = findPeaksInRange(
                rightEdgeProjector,
                0,
                4,
                "FULL",
                25,
                10,
                10,
                0);
        System.out.println(
                "grid.staff-projector-ranges.synthetic=rejectedBrowse:"
                        + staffPeakRanges(rejectedBrowse)
                        + ";afterRejected:" + staffPeakRanges(afterRejected)
                        + ";initialHalfEdge:" + staffPeakRanges(initialHalfEdge)
                        + ";fullEdge:" + staffPeakRanges(fullEdge));

        int[] braceCounts = new int[21];
        braceCounts[0] = braceCounts[1] = braceCounts[2] = 1;
        braceCounts[6] = 6;
        braceCounts[7] = 7;
        braceCounts[9] = 8;
        Arrays.fill(braceCounts, 13, braceCounts.length, 1);
        StaffProjector braceProjector = staffProjector(
                projectorScale,
                braceCounts,
                new int[]{0, 10, 20, 30, 40});
        Object braceParams = field(braceProjector, "params");
        setIntField(braceParams, "blankThreshold", 0);
        setIntField(braceParams, "minWideBlankWidth", 3);
        setIntField(braceParams, "braceThreshold", 5);
        Staff braceStaff = (Staff) field(braceProjector, "staff");
        braceStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.LEFT, 15);
        findAllBlanks.invoke(braceProjector);
        Method selectEndingBlanks = StaffProjector.class.getDeclaredMethod("selectEndingBlanks");
        selectEndingBlanks.setAccessible(true);
        selectEndingBlanks.invoke(braceProjector);
        @SuppressWarnings("unchecked")
        Map<org.audiveris.omr.util.HorizontalSide, Object> braceEndingBlanks =
                (Map<org.audiveris.omr.util.HorizontalSide, Object>) field(
                        braceProjector,
                        "endingBlanks");
        List<String> braceBlanks = new ArrayList<>();
        for (Object blank : (List<?>) field(braceProjector, "allBlanks")) {
            braceBlanks.add(blank(blank));
        }
        StaffPeak brace = braceProjector.findBracePeak(0, 14);
        System.out.println(
                "grid.staff-projector-brace.synthetic=blanks:"
                        + String.join(",", braceBlanks)
                        + ";leftEnding:"
                        + blank(braceEndingBlanks.get(org.audiveris.omr.util.HorizontalSide.LEFT))
                        + ";peak:" + staffPeakRange(brace)
                        + ":" + brace.getTop() + "-" + brace.getBottom()
                        + ":brace" + brace.isBrace());

        RunTable composedBinary = new RunTable(Orientation.VERTICAL, 20, 6);
        for (int x = 0; x < 20; x++) {
            if ((x == 5) || (x == 6)) {
                composedBinary.addRun(x, new Run(0, 6));
            } else {
                composedBinary.addRun(x, new Run(0, 1));
                composedBinary.addRun(x, new Run(5, 1));
            }
        }
        StaffProjector composedProjector = staffProjector(
                projectorScale,
                composedBinary,
                new int[]{0, 5});
        Object composedParams = field(composedProjector, "params");
        setIntField(composedParams, "staffAbscissaMargin", 20);
        setIntField(composedParams, "blankThreshold", 2);
        setIntField(composedParams, "minWideBlankWidth", 2);
        setIntField(composedParams, "barThreshold", 4);
        setIntField(composedParams, "linesThreshold", 2);
        setIntField(composedParams, "chunkThreshold", 4);
        setIntField(composedParams, "barRefineDx", 2);
        setIntField(composedParams, "chunkWidth", 1);
        setIntField(composedParams, "maxBarWidth", 4);
        setIntField(composedParams, "gapThreshold", 1);
        setIntField(composedParams, "braceThreshold", 4);
        Method computeProjection = StaffProjector.class.getDeclaredMethod("computeProjection");
        computeProjection.setAccessible(true);
        computeProjection.invoke(composedProjector);
        findAllBlanks.invoke(composedProjector);
        selectEndingBlanks.invoke(composedProjector);
        Method findPeaks = StaffProjector.class.getDeclaredMethod("findPeaks");
        findPeaks.setAccessible(true);
        findPeaks.invoke(composedProjector);
        Projection.Short composedProjection = (Projection.Short) field(
                composedProjector,
                "projection");
        long composedCountsDigest = 0xcbf29ce484222325L;
        for (int x = composedProjection.getStart(); x <= composedProjection.getStop(); x++) {
            composedCountsDigest = hashInt(composedCountsDigest, composedProjection.getValue(x));
        }
        List<String> composedBlanks = new ArrayList<>();
        for (Object blank : (List<?>) field(composedProjector, "allBlanks")) {
            composedBlanks.add(blank(blank));
        }
        List<String> composedPeaks = new ArrayList<>();
        for (StaffPeak peak : composedProjector.getPeaks()) {
            composedPeaks.add(String.format(
                    java.util.Locale.ROOT,
                    "%d-%d:%.12f",
                    peak.getStart(),
                    peak.getStop(),
                    peak.getImpacts().getGrade()));
        }
        StaffPeak composedBrace = composedProjector.findBracePeak(0, 7);
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.staff-projector-composed.synthetic=bounds:%d-%d;counts:%016x;derivative:%d;blanks:%s;search:4-7;peaks:%s;brace:%s%n",
                composedProjection.getStart(),
                composedProjection.getStop(),
                composedCountsDigest,
                field(composedProjector, "derivativeThreshold"),
                String.join(",", composedBlanks),
                composedPeaks.isEmpty() ? "none" : String.join(",", composedPeaks),
                staffPeakRange(composedBrace));

        int[] linesRootCounts = new int[30];
        Arrays.fill(linesRootCounts, 1);
        Arrays.fill(linesRootCounts, 0, 2, 0);
        Arrays.fill(linesRootCounts, 5, 11, 0);
        Arrays.fill(linesRootCounts, 12, 14, 0);
        StaffProjector linesRootProjector = staffProjector(
                projectorScale,
                linesRootCounts,
                new int[]{0, 4});
        Object linesRootParams = field(linesRootProjector, "params");
        setIntField(linesRootParams, "blankThreshold", 0);
        setIntField(linesRootParams, "minSmallBlankWidth", 4);
        setIntField(linesRootParams, "maxLeftExtremum", 8);
        findAllBlanks.invoke(linesRootProjector);
        Staff linesRootStaff = (Staff) field(linesRootProjector, "staff");
        linesRootStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.LEFT, 3);
        StaffPeak unmarkedFirstPeak = new StaffPeak(linesRootStaff, 0, 4, 20, 21, null);
        StaffPeak markedLaterPeak = new StaffPeak(linesRootStaff, 0, 4, 25, 26, null);
        markedLaterPeak.setStaffEnd(org.audiveris.omr.util.HorizontalSide.LEFT);
        @SuppressWarnings("unchecked")
        List<StaffPeak> linesRootPeaks = (List<StaffPeak>) field(linesRootProjector, "peaks");
        linesRootPeaks.add(unmarkedFirstPeak);
        linesRootPeaks.add(markedLaterPeak);
        Object selectedLinesRootBlank = selectBlank.invoke(
                linesRootProjector,
                org.audiveris.omr.util.HorizontalSide.LEFT,
                unmarkedFirstPeak.getStart(),
                4);
        linesRootProjector.checkLinesRoot();
        int changedStaffLeft = linesRootStaff.getAbscissa(
                org.audiveris.omr.util.HorizontalSide.LEFT);
        boolean changedStartMarked = markedLaterPeak.isStaffEnd(
                org.audiveris.omr.util.HorizontalSide.LEFT);

        linesRootStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.LEFT, 3);
        markedLaterPeak.setStaffEnd(org.audiveris.omr.util.HorizontalSide.LEFT);
        setIntField(linesRootParams, "maxLeftExtremum", 9);
        linesRootProjector.checkLinesRoot();
        int boundaryStaffLeft = linesRootStaff.getAbscissa(
                org.audiveris.omr.util.HorizontalSide.LEFT);
        boolean boundaryStartMarked = markedLaterPeak.isStaffEnd(
                org.audiveris.omr.util.HorizontalSide.LEFT);

        linesRootStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.LEFT, 3);
        markedLaterPeak.setStaffEnd(org.audiveris.omr.util.HorizontalSide.LEFT);
        setIntField(linesRootParams, "maxLeftExtremum", 8);
        linesRootProjector.setBracePeak(new StaffPeak(linesRootStaff, 0, 4, 2, 3, null));
        linesRootProjector.checkLinesRoot();
        System.out.println(
                "grid.staff-projector-lines-root.synthetic=selected:"
                        + blank(selectedLinesRootBlank)
                        + ";changed:left" + changedStaffLeft + ":start" + changedStartMarked
                        + ":first" + unmarkedFirstPeak.isStaffEnd(
                                org.audiveris.omr.util.HorizontalSide.LEFT)
                        + ";boundary:left" + boundaryStaffLeft + ":start"
                        + boundaryStartMarked
                        + ";brace:left" + linesRootStaff.getAbscissa(
                                org.audiveris.omr.util.HorizontalSide.LEFT)
                        + ":start" + markedLaterPeak.isStaffEnd(
                                org.audiveris.omr.util.HorizontalSide.LEFT));

        int[] resultOperationCounts = new int[100];
        Arrays.fill(resultOperationCounts, 1);
        Arrays.fill(resultOperationCounts, 40, 51, 0);
        StaffProjector resultOperationProjector = staffProjector(
                projectorScale, resultOperationCounts, new int[]{0, 4});
        Sheet resultOperationSheet = (Sheet) field(resultOperationProjector, "sheet");
        PeakGraph resultOperationGraph = new PeakGraph(resultOperationSheet, new ArrayList<>());
        Field resultOperationGraphField = StaffProjector.class.getDeclaredField("peakGraph");
        resultOperationGraphField.setAccessible(true);
        resultOperationGraphField.set(resultOperationProjector, resultOperationGraph);
        Staff resultOperationStaff = (Staff) field(resultOperationProjector, "staff");
        StaffPeak resultFirst = new StaffPeak(resultOperationStaff, 0, 4, 10, 11, null);
        StaffPeak resultLast = new StaffPeak(resultOperationStaff, 0, 4, 20, 21, null);
        resultFirst.setStaffEnd(org.audiveris.omr.util.HorizontalSide.RIGHT);
        resultLast.setStaffEnd(org.audiveris.omr.util.HorizontalSide.LEFT);
        @SuppressWarnings("unchecked")
        List<StaffPeak> resultOperationPeaks =
                (List<StaffPeak>) field(resultOperationProjector, "peaks");
        resultOperationPeaks.add(resultFirst);
        resultOperationPeaks.add(resultLast);
        resultOperationGraph.addVertex(resultFirst);
        resultOperationGraph.addVertex(resultLast);
        int initialStartIndex = resultOperationProjector.getStartPeakIndex();
        int initialLastStart = resultOperationProjector.getLastPeak().getStart();
        StaffPeak resultInserted = new StaffPeak(resultOperationStaff, 0, 4, 15, 16, null);
        StaffPeak equalAnchor = new StaffPeak(resultOperationStaff, 9, 12, 20, 21, null);
        resultOperationProjector.insertPeak(resultInserted, equalAnchor);
        List<String> insertedOrder = new ArrayList<>();
        for (StaffPeak peak : resultOperationProjector.getPeaks()) {
            insertedOrder.add(Integer.toString(peak.getStart()));
        }
        int insertedStartIndex = resultOperationProjector.getStartPeakIndex();
        resultOperationProjector.removePeaks(List.of(resultInserted, resultFirst));
        List<String> remainingOrder = new ArrayList<>();
        for (StaffPeak peak : resultOperationProjector.getPeaks()) {
            remainingOrder.add(Integer.toString(peak.getStart()));
        }

        StaffProjector rightEndProjector = staffProjector(
                projectorScale, resultOperationCounts, new int[]{0, 4});
        Object rightEndParams = field(rightEndProjector, "params");
        setIntField(rightEndParams, "blankThreshold", 0);
        setIntField(rightEndParams, "minSmallBlankWidth", 3);
        setIntField(rightEndParams, "maxRightExtremum", 6);
        findAllBlanks.invoke(rightEndProjector);
        Staff rightEndStaff = (Staff) field(rightEndProjector, "staff");
        rightEndStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.RIGHT, 25);
        StaffPeak rightEndPeak = new StaffPeak(rightEndStaff, 0, 4, 30, 32, null);
        @SuppressWarnings("unchecked")
        List<StaffPeak> rightEndPeaks = (List<StaffPeak>) field(rightEndProjector, "peaks");
        rightEndPeaks.add(rightEndPeak);
        rightEndProjector.refineRightEnd();
        int overLimitRight = rightEndStaff.getAbscissa(
                org.audiveris.omr.util.HorizontalSide.RIGHT);
        boolean overLimitMarked = rightEndPeak.isStaffEnd(
                org.audiveris.omr.util.HorizontalSide.RIGHT);
        rightEndStaff.setAbscissa(org.audiveris.omr.util.HorizontalSide.RIGHT, 25);
        rightEndPeak.unset(StaffPeak.Attribute.STAFF_RIGHT_END);
        setIntField(rightEndParams, "maxRightExtremum", 7);
        rightEndProjector.refineRightEnd();
        System.out.println(
                "grid.staff-projector-result-ops.synthetic=initial:start"
                        + initialStartIndex + ":last" + initialLastStart
                        + ";insert:" + String.join(",", insertedOrder) + ":start"
                        + insertedStartIndex
                        + ";remove:" + String.join(",", remainingOrder) + ":last"
                        + resultOperationProjector.getLastPeak().getStart()
                        + ";rightBlank:40-50;over:right" + overLimitRight + ":marked"
                        + overLimitMarked
                        + ";boundary:right" + rightEndStaff.getAbscissa(
                                org.audiveris.omr.util.HorizontalSide.RIGHT)
                        + ":marked" + rightEndPeak.isStaffEnd(
                                org.audiveris.omr.util.HorizontalSide.RIGHT));

        RunTable runs = new RunTable(Orientation.HORIZONTAL, 10, 5);
        runs.addRun(0, new Run(1, 2));
        runs.addRun(0, new Run(5, 3));
        runs.addRun(1, new Run(0, 1));
        runs.addRun(1, new Run(4, 2));
        System.out.println("runs=" + runs.getTotalRunCount() + "/" + runs.getWeight() + "/" + runs.getRunAt(6, 0));

        RunTable dispatchSource = new RunTable(Orientation.VERTICAL, 5, 8);
        dispatchSource.addRun(0, new Run(0, 2));
        dispatchSource.addRun(0, new Run(4, 3));
        dispatchSource.addRun(1, new Run(1, 4));
        dispatchSource.addRun(3, new Run(0, 1));
        dispatchSource.addRun(3, new Run(3, 2));
        dispatchSource.addRun(4, new Run(2, 5));
        Book dispatchBook = new Book(Path.of("grid-run-dispatch.synthetic"));
        SheetStub dispatchStub = new SheetStub(dispatchBook, 1);
        dispatchBook.addStub(dispatchStub);
        Sheet dispatchSheet = new Sheet(dispatchStub, dispatchSource);
        dispatchSheet.setScale(new Scale(
                new Scale.InterlineScale(10, 10, 10),
                new Scale.LineScale(1, 2, 2),
                null,
                null,
                null));
        LagManager dispatchManager = dispatchSheet.getLagManager();
        RunTable dispatchLong = new RunTable(Orientation.VERTICAL, 5, 8);
        RunTable dispatchHorizontal = dispatchManager.dispatchRuns(dispatchSource, dispatchLong);
        System.out.println(
                "grid.run-dispatch.synthetic=source:"
                        + dispatchSource.getTotalRunCount() + "/" + dispatchSource.getWeight()
                        + ";long:" + dispatchLong.getTotalRunCount() + "/" + dispatchLong.getWeight()
                        + ";horizontal:" + dispatchHorizontal.getTotalRunCount() + "/"
                        + dispatchHorizontal.getWeight());

        RunTable sectionRuns = new RunTable(Orientation.HORIZONTAL, 9, 6);
        sectionRuns.addRun(0, new Run(1, 3));
        sectionRuns.addRun(0, new Run(6, 2));
        sectionRuns.addRun(1, new Run(1, 3));
        sectionRuns.addRun(1, new Run(6, 2));
        sectionRuns.addRun(2, new Run(1, 7));
        sectionRuns.addRun(3, new Run(2, 5));
        sectionRuns.addRun(4, new Run(2, 2));
        sectionRuns.addRun(4, new Run(5, 2));
        sectionRuns.addRun(5, new Run(2, 2));
        sectionRuns.addRun(5, new Run(5, 2));
        Lag sectionLag = new BasicLag("rust-synthetic", Orientation.HORIZONTAL);
        List<Section> sections = new SectionFactory(
                sectionLag,
                JunctionRatioPolicy.DEFAULT).createSections(sectionRuns, null, true);
        List<String> sectionShapes = new ArrayList<>();
        for (Section section : sections) {
            StringBuilder runShape = new StringBuilder();
            for (Run run : section.getRuns()) {
                if (!runShape.isEmpty()) {
                    runShape.append(',');
                }
                runShape.append(run.getStart()).append('+').append(run.getLength());
            }
            java.awt.Rectangle bounds = section.getBounds();
            sectionShapes.add(String.format(
                    java.util.Locale.ROOT,
                    "%d-%d/%d/%d/%d/%d,%d,%d,%d/%s",
                    section.getFirstPos(),
                    section.getLastPos(),
                    section.getRunCount(),
                    section.getWeight(),
                    section.getMaxRunLength(),
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                    runShape));
        }
        Collections.sort(sectionShapes);
        System.out.println(
                "grid.sections.synthetic=" + sections.size() + "/"
                        + sectionLag.getEntities().size() + "/"
                        + sectionLag.getRunTable().getTotalRunCount() + "/"
                        + sectionLag.getRunTable().getWeight() + "/"
                        + String.join("|", sectionShapes));

        RunTable filamentRuns = new RunTable(Orientation.HORIZONTAL, 165, 15);
        int[][] filamentStrips = {{2, 0}, {5, 40}, {8, 80}, {11, 120}};
        for (int[] strip : filamentStrips) {
            filamentRuns.addRun(strip[0], new Run(strip[1], 45));
            filamentRuns.addRun(strip[0] + 1, new Run(strip[1], 45));
        }
        List<Section> filamentSections = new SectionFactory(
                Orientation.HORIZONTAL,
                JunctionRatioPolicy.DEFAULT).createSections(filamentRuns, null, false);
        StaffFilament filament = new StaffFilament(10);
        for (Section section : filamentSections) {
            filament.addSection(section);
        }
        java.awt.Rectangle filamentBounds = filament.getBounds();
        java.awt.geom.Point2D filamentStart = filament.getStartPoint();
        java.awt.geom.Point2D filamentStop = filament.getStopPoint();
        StringBuilder filamentSamples = new StringBuilder();
        int[] sampleCoords = {0, 40, 80, 120, 164};
        for (int coord : sampleCoords) {
            if (!filamentSamples.isEmpty()) {
                filamentSamples.append(',');
            }
            filamentSamples.append(String.format(
                    java.util.Locale.ROOT,
                    "%d:%.12f:%.12f",
                    coord,
                    filament.getPositionAt(coord, Orientation.HORIZONTAL),
                    filament.getSlopeAt(coord, Orientation.HORIZONTAL)));
        }
        StringBuilder within = new StringBuilder(4);
        for (int coord : new int[]{-1, 0, 164, 165}) {
            within.append(filament.isWithinRange(coord) ? '1' : '0');
        }
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.filament.synthetic=%d/%d,%d,%d,%d/%d/%d/%.12f,%.12f/%.12f,%.12f/%.12f/%s/%s%n",
                filament.getMembers().size(),
                filamentBounds.x,
                filamentBounds.y,
                filamentBounds.width,
                filamentBounds.height,
                filament.getWeight(),
                filament.getTrueLength(),
                filamentStart.getX(),
                filamentStart.getY(),
                filamentStop.getX(),
                filamentStop.getY(),
                filament.getThickness(),
                filamentSamples,
                within);

        RunTable factoryRuns = new RunTable(Orientation.HORIZONTAL, 85, 14);
        for (int row : new int[]{2, 3}) {
            factoryRuns.addRun(row, new Run(0, 40));
            factoryRuns.addRun(row, new Run(45, 40));
        }
        for (int row : new int[]{10, 11}) {
            factoryRuns.addRun(row, new Run(0, 40));
        }
        List<Section> factorySections = new SectionFactory(
                Orientation.HORIZONTAL,
                JunctionRatioPolicy.DEFAULT).createSections(factoryRuns, null, false);
        Scale factoryScale = new Scale(
                new Scale.InterlineScale(10, 10, 10),
                new Scale.LineScale(1, 1, 1),
                null,
                null,
                null);
        FilamentFactory<StaffFilament> filamentFactory = new FilamentFactory<>(
                factoryScale,
                new FilamentIndex(null),
                Orientation.HORIZONTAL,
                StaffFilament.class);
        List<StaffFilament> factoryFilaments = filamentFactory.retrieveFilaments(factorySections);
        List<String> factoryShapes = new ArrayList<>();
        for (StaffFilament factoryFilament : factoryFilaments) {
            List<String> memberShapes = new ArrayList<>();
            for (Section member : factoryFilament.getMembers()) {
                java.awt.Rectangle memberBounds = member.getBounds();
                memberShapes.add(String.format(
                        java.util.Locale.ROOT,
                        "%d,%d,%d,%d,%d",
                        memberBounds.x,
                        memberBounds.y,
                        memberBounds.width,
                        memberBounds.height,
                        member.getWeight()));
            }
            Collections.sort(memberShapes);
            java.awt.Rectangle factoryBounds = factoryFilament.getBounds();
            java.awt.geom.Point2D factoryStart = factoryFilament.getStartPoint();
            java.awt.geom.Point2D factoryStop = factoryFilament.getStopPoint();
            factoryShapes.add(String.format(
                    java.util.Locale.ROOT,
                    "%d/%d,%d,%d,%d/%d/%d/%s/%.12f,%.12f/%.12f,%.12f/%.12f",
                    factoryFilament.getMembers().size(),
                    factoryBounds.x,
                    factoryBounds.y,
                    factoryBounds.width,
                    factoryBounds.height,
                    factoryFilament.getWeight(),
                    factoryFilament.getTrueLength(),
                    String.join(";", memberShapes),
                    factoryStart.getX(),
                    factoryStart.getY(),
                    factoryStop.getX(),
                    factoryStop.getY(),
                    factoryFilament.getThickness()));
        }
        Collections.sort(factoryShapes);
        System.out.println(
                "grid.filament-factory.synthetic=" + factoryFilaments.size() + "/"
                        + String.join("|", factoryShapes));

        List<Section> overlapSections = new ArrayList<>();
        for (int[] spec : new int[][]{{0, 2, 40}, {10, 3, 40}, {5, 8, 40}}) {
            RunTable overlapRuns = new RunTable(Orientation.HORIZONTAL, 55, 12);
            overlapRuns.addRun(spec[1], new Run(spec[0], spec[2]));
            overlapSections.add(new SectionFactory(
                    Orientation.HORIZONTAL,
                    JunctionRatioPolicy.DEFAULT).createSections(
                            overlapRuns,
                            null,
                            false).get(0));
        }
        FilamentFactory<StaffFilament> overlapFactory = new FilamentFactory<>(
                factoryScale,
                new FilamentIndex(null),
                Orientation.HORIZONTAL,
                StaffFilament.class);
        List<StaffFilament> overlapFilaments = overlapFactory.retrieveFilaments(overlapSections);
        List<Integer> overlapMemberCounts = new ArrayList<>();
        for (StaffFilament overlapFilament : overlapFilaments) {
            overlapMemberCounts.add(overlapFilament.getMembers().size());
        }
        Collections.sort(overlapMemberCounts);
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.filament-factory.overlap=%d/%016x/%d/%s/%016x%n",
                overlapSections.size(),
                sectionDigest(overlapSections),
                overlapFilaments.size(),
                overlapMemberCounts,
                filamentDigest(overlapFilaments));

        StaffFilament clusterSeed = staffFilament(0, 12, 40, 10);
        LineCluster lineCluster = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                clusterSeed);
        lineCluster.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(45, 12, 40, 10)), 0);
        lineCluster.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(10, 2, 40, 10)), -1);
        // mergeWith is relative to the receiver's current first key (-1), hence +2 -> position +1.
        lineCluster.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(10, 22, 44, 10)), 2);
        List<String> clusterLines = new ArrayList<>();
        for (StaffFilament clusterLine : lineCluster.getLines()) {
            clusterLines.add(clusterLine.getClusterPos() + ":" + clusterLine.getMembers().size());
        }
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.line-cluster.synthetic=%d/%s/%s/%s/%s/%d/%s/%s%n",
                lineCluster.getSize(),
                String.join(",", clusterLines),
                rectangle(lineCluster.getFirstLine().getBounds()),
                rectangle(lineCluster.getLastLine().getBounds()),
                rectangle(lineCluster.getBounds()),
                lineCluster.getTrueLength(),
                points(lineCluster.getPointsAt(5.0, 3, 0.25)),
                points(lineCluster.getPointsAt(-3.0, 3, 0.25)));

        Scale indexedScale = new Scale(
                new Scale.InterlineScale(10, 10, 10),
                new Scale.LineScale(1, 1, 2),
                null,
                null,
                null);
        LineCluster indexedCluster = new LineCluster(
                indexedScale,
                indexedScale.getInterlineScale(),
                staffFilament(0, 12, 40, 10));
        indexedCluster.mergeWith(new LineCluster(
                indexedScale,
                indexedScale.getInterlineScale(),
                staffFilament(0, 2, 40, 10)), -1);
        indexedCluster.mergeWith(new LineCluster(
                indexedScale,
                indexedScale.getInterlineScale(),
                staffFilament(0, 22, 40, 10)), 2);
        boolean atLimitAccepted = indexedCluster.includeFilamentByIndex(
                staffFilament(10, 13, 19, 10),
                1);
        boolean aboveAccepted = indexedCluster.includeFilamentByIndex(
                staffFilament(10, 4, 19, 10),
                0);
        List<String> indexedLines = new ArrayList<>();
        for (StaffFilament indexedLine : indexedCluster.getLines()) {
            indexedLines.add(indexedLine.getClusterPos() + ":" + indexedLine.getMembers().size());
        }
        System.out.println(
                "grid.line-cluster-index.synthetic=max:" + indexedScale.getMaxFore()
                        + ";limitAccepted:" + atLimitAccepted
                        + ";aboveAccepted:" + aboveAccepted
                        + ";lines:" + String.join(",", indexedLines)
                        + ";starts:" + points(indexedCluster.getStarts())
                        + ";stops:" + points(indexedCluster.getStops()));

        LineCluster mergeDestination = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(0, 10, 20, 10));
        mergeDestination.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(0, 30, 20, 10)), 2);
        LineCluster mergeSource = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(25, 20, 20, 10));
        mergeSource.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(50, 20, 20, 10)), 0);
        mergeSource.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(25, 30, 20, 10)), 1);
        mergeDestination.mergeWith(mergeSource, 1);
        List<String> mergedLifecycleLines = new ArrayList<>();
        for (StaffFilament lifecycleLine : mergeDestination.getLines()) {
            mergedLifecycleLines.add(lifecycleLine.getClusterPos() + ":"
                    + lifecycleLine.getMembers().size()
                    + ":" + lifecycleLine.getTrueLength());
        }

        LineCluster renumberCluster = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(0, 20, 20, 10));
        renumberCluster.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(0, 0, 20, 10)), -2);
        renumberCluster.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                staffFilament(0, 50, 20, 10)), 5);
        renumberCluster.renumberLines();
        List<String> renumberedPositions = new ArrayList<>();
        for (StaffFilament lifecycleLine : renumberCluster.getLines()) {
            renumberedPositions.add(Integer.toString(lifecycleLine.getClusterPos()));
        }

        List<StaffFilament> trimLogical = List.of(
                staffFilament(0, 0, 10, 10),
                staffFilament(0, 10, 20, 10),
                staffFilament(0, 20, 30, 10),
                staffFilament(0, 30, 40, 10),
                staffFilament(0, 40, 20, 10),
                staffFilament(0, 50, 20, 10),
                staffFilament(0, 60, 20, 10));
        LineCluster trimCluster = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                trimLogical.get(2));
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(0)), -2);
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(1)), 1);
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(3)), 3);
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(4)), 4);
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(5)), 5);
        trimCluster.mergeWith(new LineCluster(
                factoryScale, factoryScale.getInterlineScale(), trimLogical.get(6)), 6);
        List<StaffFilament> trimmed = trimCluster.trim(new TreeSet<>(List.of(5)), 0.5);
        List<String> trimmedIds = new ArrayList<>();
        for (StaffFilament lifecycleLine : trimmed) {
            for (int i = 0; i < trimLogical.size(); i++) {
                if (lifecycleLine == trimLogical.get(i)) {
                    trimmedIds.add(Integer.toString(i + 1));
                    break;
                }
            }
        }
        List<String> keptTrimLines = new ArrayList<>();
        for (StaffFilament lifecycleLine : trimCluster.getLines()) {
            for (int i = 0; i < trimLogical.size(); i++) {
                if (lifecycleLine == trimLogical.get(i)) {
                    keptTrimLines.add(lifecycleLine.getClusterPos() + ":" + (i + 1));
                    break;
                }
            }
        }
        System.out.println(
                "grid.line-cluster-lifecycle.synthetic=merge:"
                        + String.join(",", mergedLifecycleLines)
                        + ";renumber:" + String.join(",", renumberedPositions)
                        + ";trimRemoved:" + String.join(",", trimmedIds)
                        + ";trimKept:" + String.join(",", keptTrimLines));

        List<StaffFilament> cycleFilaments = List.of(
                staffFilament(0, 1, 18, 10),
                staffFilament(20, 4, 18, 10),
                staffFilament(40, 7, 18, 10));
        FilamentComb cycleComb1 = new FilamentComb(1);
        cycleComb1.append(cycleFilaments.get(0), 0.0);
        cycleComb1.append(cycleFilaments.get(1), 10.0);
        FilamentComb cycleComb2 = new FilamentComb(2);
        cycleComb2.append(cycleFilaments.get(1), 0.0);
        cycleComb2.append(cycleFilaments.get(2), 10.0);
        FilamentComb cycleComb3 = new FilamentComb(3);
        cycleComb3.append(cycleFilaments.get(2), 0.0);
        cycleComb3.append(cycleFilaments.get(0), 10.0);
        LineCluster cycleCluster = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                cycleFilaments.get(0));
        List<String> cycleLines = new ArrayList<>();
        for (StaffFilament cycleLine : cycleCluster.getLines()) {
            cycleLines.add(cycleLine.getClusterPos() + "-"
                    + (cycleFilaments.indexOf(cycleLine) + 1));
        }

        List<StaffFilament> collisionFilaments = List.of(
                staffFilament(0, 1, 18, 10),
                staffFilament(20, 4, 18, 10),
                staffFilament(40, 7, 18, 10),
                staffFilament(60, 10, 18, 10));
        LineCluster collisionDestination = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                collisionFilaments.get(0));
        collisionDestination.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                collisionFilaments.get(3)), 1);
        LineCluster collisionSwallowed = new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                collisionFilaments.get(1));
        collisionSwallowed.mergeWith(new LineCluster(
                factoryScale,
                factoryScale.getInterlineScale(),
                collisionFilaments.get(2)), 1);
        FilamentComb collisionComb1 = new FilamentComb(1);
        collisionComb1.append(collisionFilaments.get(0), 0.0);
        collisionComb1.append(collisionFilaments.get(1), 10.0);
        FilamentComb collisionComb2 = new FilamentComb(1);
        collisionComb2.append(collisionFilaments.get(1), 0.0);
        collisionComb2.append(collisionFilaments.get(2), 10.0);
        Method includeCluster = LineCluster.class.getDeclaredMethod(
                "include",
                StaffFilament.class,
                int.class);
        includeCluster.setAccessible(true);
        includeCluster.invoke(collisionDestination, collisionFilaments.get(0), 0);
        List<String> collisionLines = new ArrayList<>();
        for (StaffFilament collisionLine : collisionDestination.getLines()) {
            collisionLines.add(collisionLine.getClusterPos() + "-"
                    + (collisionFilaments.indexOf(collisionLine) + 1));
        }
        System.out.println(
                "grid.line-cluster-recursive.synthetic=cycleProcessed:"
                        + cycleComb1.isProcessed() + "," + cycleComb2.isProcessed() + ","
                        + cycleComb3.isProcessed()
                        + ";cycleLines:" + String.join(",", cycleLines)
                        + ";clusterMerged:"
                        + (collisionSwallowed.getAncestor() == collisionDestination)
                        + ";filamentParent:"
                        + (collisionFilaments.indexOf(collisionFilaments.get(1).getAncestor()) + 1)
                        + ";collisionLines:" + String.join(",", collisionLines)
                        + ";collisionProcessed:" + collisionComb1.isProcessed() + ","
                        + collisionComb2.isProcessed());

        Book columnBook = new Book(Path.of("bar-column.synthetic"));
        SheetStub columnStub = new SheetStub(columnBook, 1);
        columnBook.addStub(columnStub);
        Sheet columnSheet = new Sheet(
                columnStub,
                new RunTable(Orientation.VERTICAL, 100, 100));
        columnSheet.setScale(indexedScale);
        Skew columnSkew = new Skew(0.0, columnSheet);
        List<Staff> columnStaves = List.of(
                new Staff(4, 0.0, 99.0, 10, new ArrayList<>()),
                new Staff(5, 0.0, 99.0, 10, new ArrayList<>()),
                new Staff(6, 0.0, 99.0, 10, new ArrayList<>()));
        SystemInfo columnSystem = new SystemInfo(12, columnSheet, columnStaves);
        PeakGraph columnGraph = new PeakGraph(columnSheet, new ArrayList<>());
        BarColumn barColumn = new BarColumn(columnSystem, columnGraph);
        StaffPeak topPeak = staffPeak(columnStaves.get(0), 2, 20, 10, 11, columnSkew);
        StaffPeak middlePeak = staffPeak(columnStaves.get(1), 30, 48, 12, 14, columnSkew);
        StaffPeak bottomPeak = staffPeak(columnStaves.get(2), 58, 76, 14, 17, columnSkew);
        topPeak.setStaffEnd(org.audiveris.omr.util.HorizontalSide.LEFT);
        for (StaffPeak peak : List.of(topPeak, middlePeak, bottomPeak)) {
            columnGraph.addVertex(peak);
        }
        BarAlignment topAlignment = new BarAlignment(
                topPeak,
                middlePeak,
                0.0,
                1.0,
                new BarAlignment.Impacts(1.0, 1.0));
        BarAlignment bottomAlignment = new BarAlignment(
                middlePeak,
                bottomPeak,
                0.0,
                1.0,
                new BarAlignment.Impacts(1.0, 1.0));
        columnGraph.addEdge(
                topPeak,
                middlePeak,
                new BarConnection(columnSheet, topAlignment, 1.0, 1.0));
        columnGraph.addEdge(
                middlePeak,
                bottomPeak,
                new BarConnection(columnSheet, bottomAlignment, 1.0, 1.0));
        // Deliberately add out of order: slots remain fixed in staff order.
        barColumn.addPeak(bottomPeak);
        barColumn.addPeak(topPeak);
        barColumn.addPeak(middlePeak);
        double initialWidth = barColumn.getWidth();
        double initialX = barColumn.getXDsk();
        boolean initialFull = barColumn.isFull();
        boolean initialConnected = barColumn.isFullyConnected();
        StaffPeak replacement = staffPeak(columnStaves.get(1), 30, 48, 20, 24, columnSkew);
        columnGraph.addVertex(replacement);
        barColumn.addPeak(replacement);
        double replacementWidth = barColumn.getWidth();
        double replacementX = barColumn.getXDsk();
        boolean replacementFull = barColumn.isFull();
        boolean replacementConnected = barColumn.isFullyConnected();
        StaffPeak braceReplacement = staffPeak(columnStaves.get(1), 30, 48, 20, 24, columnSkew);
        braceReplacement.set(StaffPeak.Attribute.BRACE);
        barColumn.addPeak(braceReplacement);
        List<String> columnSlots = new ArrayList<>();
        for (StaffPeak peak : barColumn.getPeaks()) {
            columnSlots.add(String.format(
                    java.util.Locale.ROOT,
                    "%d@%.1f",
                    peak.getStaff().getId(),
                    peak.getDeskewedAbscissa()));
        }
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.bar-column.synthetic=slots:%s;initial:%.12f,%.12f,%s,%s,%s;overwrite:%.12f,%.12f,%s,%s;brace:%s,%s%n",
                String.join(",", columnSlots),
                initialWidth,
                initialX,
                initialFull,
                initialConnected,
                barColumn.isStart(),
                replacementWidth,
                replacementX,
                replacementFull,
                replacementConnected,
                braceReplacement.isBrace(),
                barColumn.isFull());

        Book barsBook = new Book(Path.of("bars-columns-start.synthetic"));
        SheetStub barsStub = new SheetStub(barsBook, 1);
        barsBook.addStub(barsStub);
        Sheet barsSheet = new Sheet(
                barsStub,
                new RunTable(Orientation.VERTICAL, 100, 100));
        barsSheet.setScale(indexedScale);
        Skew barsSkew = new Skew(0.0, barsSheet);
        List<Staff> barsStaves = List.of(
                new OneLineStaff(10, 0.0, 99.0, 10, new ArrayList<>()),
                new OneLineStaff(11, 0.0, 99.0, 10, new ArrayList<>()));
        SystemInfo barsSystem = new SystemInfo(13, barsSheet, barsStaves);
        barsSheet.getSystemManager().setSystems(List.of(barsSystem));
        BarsRetriever barsRetriever = new BarsRetriever(barsSheet);
        PeakGraph barsGraph = (PeakGraph) field(barsRetriever, "peakGraph");

        StaffPeak c0Top = staffPeak(barsStaves.get(0), 2, 20, 10, 11, barsSkew);
        StaffPeak c0Bottom = staffPeak(barsStaves.get(1), 30, 48, 10, 11, barsSkew);
        StaffPeak c1Top = staffPeak(barsStaves.get(0), 2, 20, 30, 31, barsSkew);
        StaffPeak c1Bottom = staffPeak(barsStaves.get(1), 30, 48, 30, 31, barsSkew);
        StaffPeak c2Bottom = staffPeak(barsStaves.get(1), 30, 48, 35, 36, barsSkew);
        StaffPeak c2Top = staffPeak(barsStaves.get(0), 2, 20, 36, 37, barsSkew);
        StaffPeak c3Top = staffPeak(barsStaves.get(0), 2, 20, 50, 51, barsSkew);
        StaffPeak c3Bottom = staffPeak(barsStaves.get(1), 30, 48, 50, 51, barsSkew);
        for (StaffPeak peak : List.of(
                c2Bottom,
                c3Top,
                c0Bottom,
                c1Top,
                c2Top,
                c0Top,
                c3Bottom,
                c1Bottom)) {
            barsGraph.addVertex(peak);
        }
        connectBars(barsSheet, barsGraph, c0Top, c0Bottom);
        connectBars(barsSheet, barsGraph, c1Top, c1Bottom);
        connectBars(barsSheet, barsGraph, c3Top, c3Bottom);

        Method buildColumns = BarsRetriever.class.getDeclaredMethod("buildColumns");
        buildColumns.setAccessible(true);
        buildColumns.invoke(barsRetriever);
        @SuppressWarnings("unchecked")
        Map<SystemInfo, List<BarColumn>> barsColumnMap =
                (Map<SystemInfo, List<BarColumn>>) field(barsRetriever, "columnMap");
        List<BarColumn> barsColumns = barsColumnMap.get(barsSystem);
        Method detectStartColumns = BarsRetriever.class.getDeclaredMethod("detectStartColumns");
        detectStartColumns.setAccessible(true);
        detectStartColumns.invoke(barsRetriever);
        List<String> builtColumns = new ArrayList<>();
        int startColumnIndex = -1;
        for (int i = 0; i < barsColumns.size(); i++) {
            BarColumn column = barsColumns.get(i);
            List<String> slots = new ArrayList<>();
            for (StaffPeak peak : column.getPeaks()) {
                slots.add(peak != null
                        ? String.format(
                                java.util.Locale.ROOT,
                                "%d@%.1f",
                                peak.getStaff().getId(),
                                peak.getDeskewedAbscissa())
                        : "-");
            }
            if (column.isStart()) {
                startColumnIndex = i;
            }
            builtColumns.add(String.format(
                    java.util.Locale.ROOT,
                    "[%s|%.1f|%s|%s]",
                    String.join(",", slots),
                    column.getXDsk(),
                    column.isFull(),
                    column.isFullyConnected()));
        }
        Object barsParams = field(barsRetriever, "params");
        System.out.printf(
                java.util.Locale.ROOT,
                "grid.bars-columns-start.synthetic=max:%s,%s,%s;columns:%s;start:%d%n",
                field(barsParams, "maxColumnDx"),
                field(barsParams, "maxBraceBarGap"),
                field(barsParams, "maxDoubleBarGap"),
                String.join(",", builtColumns),
                startColumnIndex);

        Book combBook = new Book(Path.of("comb-discovery.synthetic"));
        SheetStub combStub = new SheetStub(combBook, 1);
        combBook.addStub(combStub);
        Sheet combSheet = new Sheet(
                combStub,
                new RunTable(Orientation.VERTICAL, 110, 100));
        combSheet.setScale(new Scale(
                new Scale.InterlineScale(10, 10, 10),
                new Scale.LineScale(1, 1, 1),
                null,
                null,
                null));
        combSheet.setSkew(new Skew(0.0, combSheet));
        List<StaffFilament> combFilaments = List.of(
                staffFilament(0, 2, 110, 10),
                staffFilament(0, 12, 110, 10),
                staffFilament(0, 22, 41, 10),
                staffFilament(0, 45, 110, 10));
        ClustersRetriever combRetriever = new ClustersRetriever(combSheet, 1, combFilaments);
        Method retrieveCombs = ClustersRetriever.class.getDeclaredMethod("retrieveCombs");
        retrieveCombs.setAccessible(true);
        retrieveCombs.invoke(combRetriever);
        @SuppressWarnings("unchecked")
        Map<Integer, List<FilamentComb>> combColumns =
                (Map<Integer, List<FilamentComb>>) field(combRetriever, "colCombs");
        int[] combXs = (int[]) field(combRetriever, "colX");
        List<String> combShapes = new ArrayList<>();
        for (Map.Entry<Integer, List<FilamentComb>> entry : combColumns.entrySet()) {
            List<String> columnShapes = new ArrayList<>();
            for (FilamentComb comb : entry.getValue()) {
                List<String> ids = new ArrayList<>();
                for (StaffFilament combFilament : comb.getFilaments()) {
                    ids.add(Integer.toString(combFilaments.indexOf(combFilament) + 1));
                }
                columnShapes.add("[" + String.join(",", ids) + "]");
            }
            combShapes.add(combXs[entry.getKey()] + String.join("+", columnShapes));
        }
        Method retrievePopularSize = ClustersRetriever.class.getDeclaredMethod(
                "retrievePopularSize");
        retrievePopularSize.setAccessible(true);
        int popularCombSize = (Integer) retrievePopularSize.invoke(combRetriever);
        System.out.println(
                "grid.combs.synthetic=" + String.join(";", combShapes)
                        + ";popular:" + popularCombSize);

        TargetSystem targetSystem = new TargetSystem(
                new SystemInfo(7, null, new ArrayList<>()),
                0.0,
                100.0,
                300.0);
        TargetStaff targetStaff = new TargetStaff(
                new Staff(3, 100.0, 300.0, 10, new ArrayList<>()),
                50.0,
                targetSystem);
        TargetLine targetLine = new TargetLine(filament, 75.0, targetStaff);
        System.out.println(
                "grid.target-line.synthetic=y:75.000000000000"
                        + ";left:" + point(targetLine.sourceOf(100.0))
                        + ";mid:" + point(targetLine.sourceOf(200.0))
                        + ";right:" + point(targetLine.sourceOf(300.0))
                        + ";above:" + point(targetLine.sourceOf(new Point2D.Double(200.0, 65.0)))
                        + ";below:" + point(targetLine.sourceOf(new Point2D.Double(200.0, 85.0)))
                        + ";extra:" + point(targetLine.sourceOf(350.0)));

        Book scoreBook = new Book(Path.of("score-update.synthetic"));
        SheetStub scoreStub1 = new SheetStub(scoreBook, 1);
        SheetStub scoreStub2 = new SheetStub(scoreBook, 2);
        SheetStub scoreStub3 = new SheetStub(scoreBook, 3);
        scoreBook.addStub(scoreStub1);
        scoreBook.addStub(scoreStub2);
        scoreBook.addStub(scoreStub3);
        scoreStub1.addPageRef(new PageRef(scoreStub1, 1, false));
        scoreStub2.addPageRef(new PageRef(scoreStub2, 1, false));
        scoreStub2.addPageRef(new PageRef(scoreStub2, 2, true));
        scoreStub3.addPageRef(new PageRef(scoreStub3, 1, false));
        scoreBook.updateScores(scoreStub1);
        String initialScoreTopology = scoreTopology(scoreBook.getScores());
        scoreStub2.getPageRefs().clear();
        scoreStub2.addPageRef(new PageRef(scoreStub2, 1, false));
        scoreBook.updateScores(scoreStub2);
        System.out.println(
                "grid.score-update.synthetic=initial:"
                        + initialScoreTopology
                        + ";updated:"
                        + scoreTopology(scoreBook.getScores()));

        NaturalSpline lineSpline = NaturalSpline.interpolate(
                new double[]{0, 10},
                new double[]{1, 6});
        NaturalSpline quadraticSpline = NaturalSpline.interpolate(
                new double[]{0, 20, 30},
                new double[]{0, 10, 10});
        NaturalSpline cubicSpline = NaturalSpline.interpolate(
                new double[]{0, 12, 19, 30},
                new double[]{0, 1, 2, 3});
        String upperException;
        try {
            lineSpline.yAtX(10.000001);
            upperException = "none";
        } catch (RuntimeException ex) {
            upperException = ex.getClass().getSimpleName();
        }
        System.out.printf(
                java.util.Locale.ROOT,
                "spline.synthetic=line:%.14f,%.14f;quadratic:%.14f,%.14f;cubic:%.14f,%.14f;lower:%.14f;upper:%s%n",
                lineSpline.yAtX(4.0),
                lineSpline.yDerivativeAtX(4.0),
                quadraticSpline.yAtX(20.0),
                quadraticSpline.yDerivativeAtX(20.0),
                cubicSpline.yAtX(24.5),
                cubicSpline.yDerivativeAtX(12.0),
                lineSpline.yAtX(-2.0),
                upperException);

        ByteProcessor raster = new ByteProcessor(5, 3);
        int[] pixels = {0, 126, 127, 128, 255, 255, 0, 255, 0, 255, 10, 20, 200, 210, 220};
        for (int index = 0; index < pixels.length; index++) {
            raster.set(index % 5, index / 5, pixels[index]);
        }
        ByteProcessor binary = new GlobalFilter(raster, 127).filteredImage();
        System.out.println("image.threshold=" + pixels(binary));
        System.out.println("image.median=" + pixels(new MedianGrayFilter(1).filter(raster)));
        Table distances = new ChamferDistance.Integer().computeToFore(binary);
        System.out.println("image.chamfer=" + values(distances));
        Table watershedDistances = new Table.Short(5, 1);
        int[] watershedProfile = {3, 2, 1, 2, 3};
        for (int x = 0; x < watershedProfile.length; x++) {
            watershedDistances.setValue(x, 0, watershedProfile[x]);
        }
        WatershedGrayLevel watershed = new WatershedGrayLevel(watershedDistances, true);
        boolean[][] watershedMap = watershed.process(1);
        StringBuilder watershedBits = new StringBuilder(watershedProfile.length);
        for (int x = 0; x < watershedProfile.length; x++) {
            watershedBits.append(watershedMap[x][0] ? '1' : '0');
        }
        System.out.println("watershed.synthetic=" + watershed.getRegionCount() + "/" + watershedBits);
        RunTable extracted = new RunTableFactory(Orientation.HORIZONTAL).createTable(binary);
        System.out.println("image.runs=" + extracted.getTotalRunCount() + "/" + extracted.getWeight() + "/" + extracted.getRunAt(1, 0) + "/" + extracted.getRunAt(4, 2));
        System.out.println("image.adaptive=" + pixels(new VerticalFilter(raster, 0.7, 0.9).filteredImage()));

        ByteProcessor staffPixels = new ByteProcessor(8, 10);
        staffPixels.setValue(255);
        staffPixels.fill();
        for (int[] point : new int[][]{{2, 1}, {3, 1}, {4, 1}, {2, 4}, {3, 4}, {2, 8}}) {
            staffPixels.set(point[0], point[1], 0);
        }
        StaffPattern fractionalPattern = new StaffPattern(3, 3, 1, 3.5);
        ByteProcessor tiePixels = new ByteProcessor(4, 1);
        tiePixels.setValue(255);
        tiePixels.fill();
        tiePixels.set(0, 0, 0);
        StaffPattern tiePattern = new StaffPattern(1, 2, 1, 4.0);
        ByteProcessor inclusivePixels = new ByteProcessor(3, 3);
        StaffPattern inclusivePattern = new StaffPattern(1, 1, 2, 4.0);
        ByteProcessor emptyPixels = new ByteProcessor(1, 1);
        emptyPixels.set(0, 0, 255);
        System.out.printf(
                java.util.Locale.ROOT,
                "staff-pattern.synthetic=%.12f/%.12f/%.12f/%.12f/%.12f%n",
                fractionalPattern.evaluate(new Point2D.Double(2.0, 1.0), staffPixels),
                tiePattern.evaluate(new Point2D.Double(0.5, 0.0), tiePixels),
                inclusivePattern.evaluate(new Point2D.Double(1.0, 1.0), inclusivePixels),
                inclusivePattern.evaluate(new Point2D.Double(0.0, 0.0), emptyPixels),
                tiePattern.evaluate(new Point2D.Double(-1.0, 0.0), tiePixels));

        ImageLoading.Loader loader = ImageLoading.getLoader(Path.of("data/examples/chula.png"));
        try {
            BufferedImage loaded = Picture.adjustImageFormat(loader.getImage(1));
            System.out.printf(
                    java.util.Locale.ROOT,
                    "load.chula=%dx%d/%016x%n",
                    loaded.getWidth(),
                    loaded.getHeight(),
                    fnv1a64(loaded.getRaster()));
            ByteProcessor adaptive = new VerticalFilter(new ByteProcessor(loaded), 0.7, 0.9).filteredImage();
            System.out.printf(
                    java.util.Locale.ROOT,
                    "binary.chula=%016x%n",
                    fnv1a64(adaptive));
            RunTable vertical = new RunTableFactory(Orientation.VERTICAL).createTable(adaptive);
            System.out.printf(
                    java.util.Locale.ROOT,
                    "scale.vertical-runs=%d/%d/%016x%n",
                    vertical.getTotalRunCount(),
                    vertical.getWeight(),
                    runTableDigest(vertical));

            Book book = new Book(Path.of("data/examples/chula.png"));
            SheetStub stub = new SheetStub(book, 1);
            book.addStub(stub);
            Sheet sheet = new Sheet(stub, vertical);
            ScaleBuilder scaleBuilder = new ScaleBuilder(sheet);
            Scale scale = scaleBuilder.retrieveScale();
            sheet.setScale(scale);
            System.out.printf(
                    java.util.Locale.ROOT,
                    "scale.chula=%s/%s/%s/%s/%s%n",
                    value(scale.getFore()),
                    value(scale.getInterline()),
                    value(scale.getSmallInterline()),
                    value(scale.getBeamThickness()),
                    value(scale.getSmallBeamScale() != null
                            ? scale.getSmallBeamScale().getMain() : null));

            Object histoKeeper = field(scaleBuilder, "histoKeeper");
            IntegerFunction blackFunction = (IntegerFunction) field(histoKeeper, "blackFunction");
            IntegerFunction comboFunction = (IntegerFunction) field(histoKeeper, "comboFunction");
            System.out.printf(
                    java.util.Locale.ROOT,
                    "scale.chula.detail=black:%s;combo:%s;combo2:%s;beam:%s;beam2:%s;guess:%s;areas:%d,%d%n",
                    range((Range) field(scaleBuilder, "blackPeak")),
                    range((Range) field(scaleBuilder, "comboPeak")),
                    range((Range) field(scaleBuilder, "comboPeak2")),
                    value((Integer) field(scaleBuilder, "beamKey")),
                    value((Integer) field(scaleBuilder, "beamKey2")),
                    value((Integer) field(scaleBuilder, "beamGuess")),
                    blackFunction.getArea(),
                    comboFunction.getArea());

            GridBuilder gridBuilder = new GridBuilder(sheet);
            gridBuilder.linesRetriever.createBothLags();
            Lag horizontalLag = sheet.getLagManager().getLag(Lags.HLAG);
            Lag verticalLag = sheet.getLagManager().getLag(Lags.VLAG);
            System.out.printf(
                    java.util.Locale.ROOT,
                    "grid.chula=%d/%d/%d/%016x/%d/%d/%d/%016x%n",
                    horizontalLag.getEntities().size(),
                    horizontalLag.getRunTable().getTotalRunCount(),
                    horizontalLag.getRunTable().getWeight(),
                    sectionDigest(horizontalLag.getEntities()),
                    verticalLag.getEntities().size(),
                    verticalLag.getRunTable().getTotalRunCount(),
                    verticalLag.getRunTable().getWeight(),
                    sectionDigest(verticalLag.getEntities()));

            // Exercise the dependency-light factory slice on a bounded sample of real page
            // sections. Keep thin cores whose expanded coordinate intervals are disjoint, so
            // this vector isolates core filtering and the real-gap branch. Overlap behavior has
            // its own synthetic boundary vector; leftover expansion remains outside this one.
            int pageMinCore = (int) Math.rint(scale.getInterline() * 0.5);
            int pageMaxLength = 4 * scale.getInterline();
            int pageMaxCoordGap = (int) Math.rint(scale.getInterline() * 1.7);
            List<Section> pageFactorySections = new ArrayList<>();
            for (Section section : horizontalLag.getEntities()) {
                java.awt.Rectangle bounds = section.getBounds();
                if ((bounds.width < pageMinCore)
                        || (bounds.width > pageMaxLength)
                        || (section.getMeanThickness(Orientation.HORIZONTAL) > 1.0)) {
                    continue;
                }

                boolean separated = true;
                for (Section accepted : pageFactorySections) {
                    java.awt.Rectangle other = accepted.getBounds();
                    if ((bounds.x <= (other.x + other.width - 1 + pageMaxCoordGap))
                            && (other.x <= (bounds.x + bounds.width - 1 + pageMaxCoordGap))) {
                        separated = false;
                        break;
                    }
                }
                if (separated) {
                    pageFactorySections.add(section);
                    if (pageFactorySections.size() == 8) {
                        break;
                    }
                }
            }
            FilamentFactory<StaffFilament> pageFilamentFactory = new FilamentFactory<>(
                    scale,
                    new FilamentIndex(null),
                    Orientation.HORIZONTAL,
                    StaffFilament.class);
            List<StaffFilament> pageFilaments = pageFilamentFactory.retrieveFilaments(
                    pageFactorySections);
            System.out.printf(
                    java.util.Locale.ROOT,
                    "grid.filament-factory.chula=%d/%016x/%d/%016x%n",
                    pageFactorySections.size(),
                    sectionDigest(pageFactorySections),
                    pageFilaments.size(),
                    filamentDigest(pageFilaments));
        } finally {
            loader.dispose();
        }

        emitScale(
                "k545",
                Path.of("../../data/synth/k545-movement1-exposition/page-001.png"));
        emitScale(
                "essen",
                Path.of("../../data/synth/essenfolksong-erk20/page-001.png"));
        emitScale(
                "josquin",
                Path.of("../../data/synth/josquin-4vperilludaveprolatum/page-001.png"));

        loader = ImageLoading.getLoader(Path.of("app/src/test/resources/org/audiveris/omr/image/Dichterliebe01-1.png"));
        try {
            BufferedImage loaded = Picture.adjustImageFormat(loader.getImage(1));
            System.out.printf(
                    java.util.Locale.ROOT,
                    "load.dichterliebe=%dx%d/%016x%n",
                    loaded.getWidth(),
                    loaded.getHeight(),
                    fnv1a64(loaded.getRaster()));
            ByteProcessor adaptive = new VerticalFilter(new ByteProcessor(loaded), 0.7, 0.9).filteredImage();
            System.out.printf(
                    java.util.Locale.ROOT,
                    "binary.dichterliebe=%016x%n",
                    fnv1a64(adaptive));
        } finally {
            loader.dispose();
        }

        System.out.println("pipeline=" + String.join(",", Arrays.stream(OmrStep.values()).map(Enum::name).toList()));

    }

    private static void emitScale (String slug,
                                   Path path)
        throws Exception
    {
        ImageLoading.Loader loader = ImageLoading.getLoader(path);
        try {
            BufferedImage loaded = Picture.adjustImageFormat(loader.getImage(1));
            ByteProcessor adaptive = new VerticalFilter(new ByteProcessor(loaded), 0.7, 0.9).filteredImage();
            RunTable vertical = new RunTableFactory(Orientation.VERTICAL).createTable(adaptive);

            Book book = new Book(path);
            SheetStub stub = new SheetStub(book, 1);
            book.addStub(stub);
            Sheet sheet = new Sheet(stub, vertical);
            ScaleBuilder scaleBuilder = new ScaleBuilder(sheet);
            Scale scale = scaleBuilder.retrieveScale();
            System.out.printf(
                    java.util.Locale.ROOT,
                    "scale.%s=%s/%s/%s/%s/%s%n",
                    slug,
                    value(scale.getFore()),
                    value(scale.getInterline()),
                    value(scale.getSmallInterline()),
                    value(scale.getBeamThickness()),
                    value(scale.getSmallBeamScale() != null
                            ? scale.getSmallBeamScale().getMain() : null));

            Object histoKeeper = field(scaleBuilder, "histoKeeper");
            IntegerFunction blackFunction = (IntegerFunction) field(histoKeeper, "blackFunction");
            IntegerFunction comboFunction = (IntegerFunction) field(histoKeeper, "comboFunction");
            System.out.printf(
                    java.util.Locale.ROOT,
                    "scale.%s.detail=black:%s;combo:%s;combo2:%s;beam:%s;beam2:%s;guess:%s;areas:%d,%d%n",
                    slug,
                    range((Range) field(scaleBuilder, "blackPeak")),
                    range((Range) field(scaleBuilder, "comboPeak")),
                    range((Range) field(scaleBuilder, "comboPeak2")),
                    value((Integer) field(scaleBuilder, "beamKey")),
                    value((Integer) field(scaleBuilder, "beamKey2")),
                    value((Integer) field(scaleBuilder, "beamGuess")),
                    blackFunction.getArea(),
                    comboFunction.getArea());
        } finally {
            loader.dispose();
        }
    }

    private static String pixels (ByteProcessor image)
    {
        int[] values = new int[image.getWidth() * image.getHeight()];
        for (int index = 0; index < values.length; index++) {
            values[index] = image.get(index % image.getWidth(), index / image.getWidth());
        }
        return Arrays.toString(values);
    }

    private static Object field (Object instance,
                                 String name)
        throws ReflectiveOperationException
    {
        Field field = instance.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    private static Object staticField (Class<?> owner,
                                       String name)
        throws ReflectiveOperationException
    {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(null);
    }

    private static String range (Range range)
    {
        return range != null ? range.min + "," + range.main + "," + range.max : "null";
    }

    private static String value (Integer value)
    {
        return value != null ? value.toString() : "null";
    }

    private static String values (Table table)
    {
        int[] values = new int[table.getWidth() * table.getHeight()];
        for (int index = 0; index < values.length; index++) {
            values[index] = table.getValue(index);
        }
        return Arrays.toString(values);
    }

    private static long fnv1a64 (Raster raster)
    {
        long hash = 0xcbf29ce484222325L;
        for (int y = 0; y < raster.getHeight(); y++) {
            for (int x = 0; x < raster.getWidth(); x++) {
                hash = (hash ^ raster.getSample(x, y, 0)) * 0x100000001b3L;
            }
        }
        return hash;
    }

    private static long fnv1a64 (ByteProcessor image)
    {
        long hash = 0xcbf29ce484222325L;
        for (int y = 0; y < image.getHeight(); y++) {
            for (int x = 0; x < image.getWidth(); x++) {
                hash = (hash ^ image.get(x, y)) * 0x100000001b3L;
            }
        }
        return hash;
    }

    private static long runTableDigest (RunTable table)
    {
        long hash = 0xcbf29ce484222325L;
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            for (java.util.Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                Run run = it.next();
                hash = hashInt(hash, sequence);
                hash = hashInt(hash, run.getStart());
                hash = hashInt(hash, run.getLength());
            }
        }
        return hash;
    }

    private static long sectionDigest (Iterable<Section> sections)
    {
        long hash = 0xcbf29ce484222325L;
        for (Section section : sections) {
            hash = hashSection(hash, section);
        }
        return hash;
    }

    private static long filamentDigest (Iterable<StaffFilament> filaments)
    {
        long hash = 0xcbf29ce484222325L;
        for (StaffFilament filament : filaments) {
            java.awt.Rectangle bounds = filament.getBounds();
            hash = hashInt(hash, filament.getMembers().size());
            hash = hashInt(hash, bounds.x);
            hash = hashInt(hash, bounds.y);
            hash = hashInt(hash, bounds.width);
            hash = hashInt(hash, bounds.height);
            hash = hashInt(hash, filament.getWeight());
            hash = hashInt(hash, filament.getTrueLength());
            for (Section section : filament.getMembers()) {
                hash = hashSection(hash, section);
            }
        }
        return hash;
    }

    private static StaffFilament staffFilament (int x,
                                                int y,
                                                int length,
                                                int interline)
        throws Exception
    {
        return staffFilament(x, y, length, 1, interline);
    }

    private static int staffDerivativeThreshold (Scale scale,
                                                 int[] counts)
        throws Exception
    {
        return (Integer) field(staffProjector(scale, counts), "derivativeThreshold");
    }

    private static StaffProjector staffProjector (Scale scale,
                                                  int[] counts)
        throws Exception
    {
        return staffProjector(scale, counts, new int[]{0, 100});
    }

    private static StaffProjector staffProjector (Scale scale,
                                                  int[] counts,
                                                  int[] lineOrdinates)
        throws Exception
    {
        int maximumCount = 0;
        for (int count : counts) {
            maximumCount = Math.max(maximumCount, count);
        }
        final int height = Math.max(
                maximumCount,
                lineOrdinates[lineOrdinates.length - 1]) + 1;
        RunTable binary = new RunTable(Orientation.VERTICAL, counts.length, height);
        for (int x = 0; x < counts.length; x++) {
            if (counts[x] > 0) {
                binary.addRun(x, new Run(0, counts[x]));
            }
        }
        return staffProjector(scale, binary, lineOrdinates);
    }

    private static StaffProjector staffProjector (Scale scale,
                                                  RunTable binary,
                                                  int[] lineOrdinates)
        throws Exception
    {
        Book book = new Book(Path.of("staff-projector-threshold.synthetic"));
        SheetStub stub = new SheetStub(book, 1);
        book.addStub(stub);
        Sheet sheet = new Sheet(stub, binary);
        sheet.setScale(scale);
        sheet.setSkew(new Skew(0.0, sheet));
        List<org.audiveris.omr.sheet.grid.LineInfo> staffLines = new ArrayList<>();
        for (int ordinate : lineOrdinates) {
            staffLines.add(staffFilament(
                    0,
                    ordinate,
                    binary.getWidth(),
                    scale.getInterline()));
        }
        Staff staff = new Staff(
                1,
                0.0,
                binary.getWidth() - 1.0,
                scale.getInterline(),
                staffLines);
        StaffProjector projector = new StaffProjector(sheet, staff, null);
        Method computeProjection = StaffProjector.class.getDeclaredMethod("computeProjection");
        computeProjection.setAccessible(true);
        computeProjection.invoke(projector);
        return projector;
    }

    private static AreaUtil.CoreData verticalCore (StaffProjector projector,
                                                   int start,
                                                   int stop,
                                                   int top,
                                                   int bottom)
        throws ReflectiveOperationException
    {
        ByteProcessor filter = (ByteProcessor) field(projector, "pixelFilter");
        return AreaUtil.verticalCore(
                filter,
                new GeoPath(new Line2D.Double(start, top, start, bottom)),
                new GeoPath(new Line2D.Double(stop, top, stop, bottom)));
    }

    private static StaffPeak createPeak (StaffProjector projector,
                                         int rawStart,
                                         int rawStop,
                                         boolean halfMode,
                                         int minimumDerivativeUp,
                                         int minimumDerivativeDown,
                                         int addedChunk)
        throws ReflectiveOperationException
    {
        Method method = StaffProjector.class.getDeclaredMethod(
                "createPeak",
                int.class,
                int.class,
                boolean.class,
                int.class,
                int.class,
                int.class);
        method.setAccessible(true);
        return (StaffPeak) method.invoke(
                projector,
                rawStart,
                rawStop,
                halfMode,
                minimumDerivativeUp,
                minimumDerivativeDown,
                addedChunk);
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static List<StaffPeak> findPeaksInRange (StaffProjector projector,
                                                    int xMin,
                                                    int xMax,
                                                    String modeName,
                                                    int minimumCount,
                                                    int minimumDerivativeUp,
                                                    int minimumDerivativeDown,
                                                    int addedChunk)
        throws ReflectiveOperationException
    {
        Class<?> modeClass = Class.forName(StaffProjector.class.getName() + "$PeakMode");
        Object mode = Enum.valueOf((Class<? extends Enum>) modeClass, modeName);
        Method method = StaffProjector.class.getDeclaredMethod(
                "findPeaksInRange",
                int.class,
                int.class,
                modeClass,
                int.class,
                int.class,
                int.class,
                int.class);
        method.setAccessible(true);
        return (List<StaffPeak>) method.invoke(
                projector,
                xMin,
                xMax,
                mode,
                minimumCount,
                minimumDerivativeUp,
                minimumDerivativeDown,
                addedChunk);
    }

    @SuppressWarnings("unchecked")
    private static List<StaffPeak> browseRange (StaffProjector projector,
                                               int rangeStart,
                                               int rangeStop,
                                               boolean halfMode,
                                               int minimumDerivativeUp,
                                               int minimumDerivativeDown,
                                               int addedChunk)
        throws ReflectiveOperationException
    {
        Method method = StaffProjector.class.getDeclaredMethod(
                "browseRange",
                int.class,
                int.class,
                boolean.class,
                int.class,
                int.class,
                int.class);
        method.setAccessible(true);
        return (List<StaffPeak>) method.invoke(
                projector,
                rangeStart,
                rangeStop,
                halfMode,
                minimumDerivativeUp,
                minimumDerivativeDown,
                addedChunk);
    }

    private static String staffPeakRange (StaffPeak peak)
    {
        return peak != null ? peak.getStart() + "-" + peak.getStop() : "null";
    }

    private static String staffPeakRanges (List<StaffPeak> peaks)
    {
        if (peaks.isEmpty()) {
            return "none";
        }
        List<String> ranges = new ArrayList<>();
        for (StaffPeak peak : peaks) {
            ranges.add(staffPeakRange(peak));
        }
        return String.join(",", ranges);
    }

    private static String blank (Object blank)
        throws ReflectiveOperationException
    {
        return blank != null ? field(blank, "start") + "-" + field(blank, "stop") : "null";
    }

    private static void configurePeakThresholds (StaffProjector projector,
                                                 int linesThreshold,
                                                 int chunkThreshold)
        throws ReflectiveOperationException
    {
        Object params = field(projector, "params");
        setIntField(params, "linesThreshold", linesThreshold);
        setIntField(params, "chunkThreshold", chunkThreshold);
    }

    private static Object refinePeakSide (StaffProjector projector,
                                          int xStart,
                                          int xStop,
                                          int direction,
                                          boolean halfMode,
                                          int minimumDerivative,
                                          int addedChunk)
        throws ReflectiveOperationException
    {
        Method method = StaffProjector.class.getDeclaredMethod(
                "refinePeakSide",
                int.class,
                int.class,
                int.class,
                boolean.class,
                int.class,
                int.class);
        method.setAccessible(true);
        return method.invoke(
                projector,
                xStart,
                xStop,
                direction,
                halfMode,
                minimumDerivative,
                addedChunk);
    }

    private static String peakSide (Object side)
        throws ReflectiveOperationException
    {
        if (side == null) {
            return "null";
        }
        return String.format(
                java.util.Locale.ROOT,
                "%d,%.12f,%.12f",
                field(side, "abscissa"),
                field(side, "derGrade"),
                field(side, "chunkGrade"));
    }

    private static void setIntField (Object instance,
                                     String name,
                                     int value)
        throws ReflectiveOperationException
    {
        Field field = instance.getClass().getDeclaredField(name);
        field.setAccessible(true);
        field.setInt(instance, value);
    }

    private static StaffFilament staffFilament (int x,
                                                int y,
                                                int length,
                                                int thickness,
                                                int interline)
        throws Exception
    {
        RunTable table = new RunTable(
                Orientation.HORIZONTAL,
                x + length + 1,
                y + thickness + 1);
        for (int row = y; row < (y + thickness); row++) {
            table.addRun(row, new Run(x, length));
        }
        Section section = new SectionFactory(
                Orientation.HORIZONTAL,
                JunctionRatioPolicy.DEFAULT).createSections(table, null, false).get(0);
        StaffFilament filament = new StaffFilament(interline);
        filament.addSection(section);
        return filament;
    }

    private static String rectangle (java.awt.Rectangle rectangle)
    {
        return rectangle.x + "," + rectangle.y + "," + rectangle.width + "," + rectangle.height;
    }

    private static String points (List<Point2D> points)
    {
        List<String> values = new ArrayList<>();
        for (Point2D point : points) {
            values.add(point != null
                    ? String.format(java.util.Locale.ROOT, "%.6f,%.6f", point.getX(), point.getY())
                    : "null");
        }
        return String.join(";", values);
    }

    private static String point (Point2D point)
    {
        return String.format(java.util.Locale.ROOT, "%.12f,%.12f", point.getX(), point.getY());
    }

    private static String scoreTopology (List<Score> scores)
    {
        List<String> topologies = new ArrayList<>();
        for (Score score : scores) {
            List<String> pages = new ArrayList<>();
            for (PageNumber page : score.getPageNumbers()) {
                pages.add(page.sheetNumber + "." + page.sheetPageId);
            }
            topologies.add(String.join(",", pages));
        }
        return scores.size() + ":" + String.join("|", topologies);
    }

    private static StaffPeak staffPeak (Staff staff,
                                        int top,
                                        int bottom,
                                        int start,
                                        int stop,
                                        Skew skew)
    {
        StaffPeak peak = new StaffPeak(staff, top, bottom, start, stop, null);
        peak.computeDeskewedCenter(skew);
        return peak;
    }

    private static void connectBars (Sheet sheet,
                                     PeakGraph graph,
                                     StaffPeak top,
                                     StaffPeak bottom)
    {
        BarAlignment alignment = new BarAlignment(
                top,
                bottom,
                0.0,
                1.0,
                new BarAlignment.Impacts(1.0, 1.0));
        graph.addEdge(top, bottom, new BarConnection(sheet, alignment, 1.0, 1.0));
    }

    private static long hashSection (long hash,
                                     Section section)
    {
        hash = hashInt(hash, section.getFirstPos());
        hash = hashInt(hash, section.getRunCount());
        hash = hashInt(hash, section.getWeight());
        hash = hashInt(hash, section.getMaxRunLength());
        for (Run run : section.getRuns()) {
            hash = hashInt(hash, run.getStart());
            hash = hashInt(hash, run.getLength());
        }
        return hash;
    }

    private static long hashInt (long hash,
                                 int value)
    {
        for (int shift = 24; shift >= 0; shift -= 8) {
            hash = (hash ^ ((value >>> shift) & 0xff)) * 0x100000001b3L;
        }
        return hash;
    }
}
