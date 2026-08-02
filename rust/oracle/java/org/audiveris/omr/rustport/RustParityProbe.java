// SPDX-License-Identifier: AGPL-3.0-or-later

package org.audiveris.omr.rustport;

import java.awt.image.BufferedImage;
import java.awt.image.Raster;
import java.nio.file.Path;
import java.util.Arrays;

import ij.process.ByteProcessor;

import org.audiveris.omr.image.ChamferDistance;
import org.audiveris.omr.image.GlobalFilter;
import org.audiveris.omr.image.ImageLoading;
import org.audiveris.omr.image.MedianGrayFilter;
import org.audiveris.omr.image.VerticalFilter;
import org.audiveris.omr.math.BasicLine;
import org.audiveris.omr.math.Histogram;
import org.audiveris.omr.math.InjectionSolver;
import org.audiveris.omr.math.IntegerFunction;
import org.audiveris.omr.math.Rational;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sig.GradeUtil;
import org.audiveris.omr.sheet.Picture;
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

        RunTable runs = new RunTable(Orientation.HORIZONTAL, 10, 5);
        runs.addRun(0, new Run(1, 2));
        runs.addRun(0, new Run(5, 3));
        runs.addRun(1, new Run(0, 1));
        runs.addRun(1, new Run(4, 2));
        System.out.println("runs=" + runs.getTotalRunCount() + "/" + runs.getWeight() + "/" + runs.getRunAt(6, 0));

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
        RunTable extracted = new RunTableFactory(Orientation.HORIZONTAL).createTable(binary);
        System.out.println("image.runs=" + extracted.getTotalRunCount() + "/" + extracted.getWeight() + "/" + extracted.getRunAt(1, 0) + "/" + extracted.getRunAt(4, 2));
        System.out.println("image.adaptive=" + pixels(new VerticalFilter(raster, 0.7, 0.9).filteredImage()));

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
        } finally {
            loader.dispose();
        }

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

    private static String pixels (ByteProcessor image)
    {
        int[] values = new int[image.getWidth() * image.getHeight()];
        for (int index = 0; index < values.length; index++) {
            values[index] = image.get(index % image.getWidth(), index / image.getWidth());
        }
        return Arrays.toString(values);
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

    private static long hashInt (long hash,
                                 int value)
    {
        for (int shift = 24; shift >= 0; shift -= 8) {
            hash = (hash ^ ((value >>> shift) & 0xff)) * 0x100000001b3L;
        }
        return hash;
    }
}
