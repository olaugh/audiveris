// SPDX-License-Identifier: AGPL-3.0-or-later

package org.audiveris.omr.rustport;

import java.awt.geom.Point2D;
import java.awt.image.BufferedImage;
import java.awt.image.Raster;
import java.lang.reflect.Field;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

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
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.lag.SectionFactory;
import org.audiveris.omr.math.BasicLine;
import org.audiveris.omr.math.Histogram;
import org.audiveris.omr.math.InjectionSolver;
import org.audiveris.omr.math.IntegerFunction;
import org.audiveris.omr.math.NaturalSpline;
import org.audiveris.omr.math.Range;
import org.audiveris.omr.math.Rational;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sig.GradeUtil;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.ScaleBuilder;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.GridBuilder;
import org.audiveris.omr.sheet.grid.LineCluster;
import org.audiveris.omr.sheet.grid.StaffFilament;
import org.audiveris.omr.sheet.grid.StaffPattern;
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

        RunTable runs = new RunTable(Orientation.HORIZONTAL, 10, 5);
        runs.addRun(0, new Run(1, 2));
        runs.addRun(0, new Run(5, 3));
        runs.addRun(1, new Run(0, 1));
        runs.addRun(1, new Run(4, 2));
        System.out.println("runs=" + runs.getTotalRunCount() + "/" + runs.getWeight() + "/" + runs.getRunAt(6, 0));

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
