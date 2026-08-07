// SPDX-License-Identifier: AGPL-3.0-or-later

//! Java `KeyBuilder`'s projection-peak pipeline: the part of the key stage that decides *how many*
//! alterations a signature has, and *where each one starts*, before any glyph is classified.
//!
//! This is the machinery whose absence left the native key stage at 45 of 65 staves. The native
//! first pass enumerated glyph subsets over the whole browse area and let the classifier vote;
//! Java instead reads the staff-free **projection** (black pixel count per abscissa), finds
//! stem-like peaks, infers the signature from peak count and spacing — sharps have two stems per
//! item, flats one — and only then allocates one slice per expected alteration. Classification
//! happens *inside* that structure, one winner per slice.
//!
//! Everything here is a pure function over an [`IntegerFunction`] and a [`StaffHeaderRange`], so
//! each Java method has a same-shaped Rust twin that unit tests can pin without a sheet.

use audiveris_core::integer_function::IntegerFunction;
use audiveris_core::range::Range;

use crate::key_parameters::KeyPipelineParameters;
use crate::staff_header::StaffHeaderRange;

/// Which alteration shape a `ShapeBuilder` is hunting. Java runs one builder per shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyShapeKind {
    /// One projection peak (stem) per item.
    Flat,
    /// Two projection peaks per item.
    Sharp,
}

/// Java `KeyPeak`: one stem-like projection peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPeak {
    /// First abscissa of the peak.
    pub min: i32,
    /// Abscissa of the maximum value.
    pub main: i32,
    /// Last abscissa of the peak.
    pub max: i32,
    /// Projection value at `main`.
    pub height: i32,
    /// Area above `minPeakCumul` between `min` and `max`.
    pub area: i32,
}

impl KeyPeak {
    /// Java `KeyPeak.getCenter`.
    #[must_use]
    pub fn center(&self) -> f64 {
        0.5 * f64::from(self.min + self.max)
    }

    /// Java `KeyPeak.getWidth`.
    #[must_use]
    pub const fn width(&self) -> i32 {
        self.max - self.min + 1
    }
}

/// Java `ShapeBuilder.browseArea`: walk the projection, pairing spaces and stem-like peaks.
///
/// `has_stem` is Java's `isStemLike`, which reads the staff-free raster — the one input this pure
/// function cannot own. It receives the peak and must answer whether a stem-length run of black
/// rows exists under it.
pub fn browse_area(
    hilos: &[Range],
    projection: &IntegerFunction,
    parameters: &KeyPipelineParameters,
    range: &mut StaffHeaderRange,
    peaks: &mut Vec<KeyPeak>,
    has_stem: &mut dyn FnMut(&KeyPeak) -> bool,
) {
    if hilos.is_empty() {
        return;
    }
    let mut hilo_iter = hilos.iter();
    let mut hilo = hilo_iter.next();

    let mut space_start = -1;
    let mut space_stop = -1;

    let mut x = range.browse_start;
    while x <= range.browse_stop {
        let cumul = projection.value(x);
        if cumul > parameters.max_space_cumul {
            if space_start != -1 {
                if !check_space(space_start, space_stop, parameters, range, peaks) {
                    return;
                }
                space_start = -1;
            }
            if let Some(current) = hilo {
                if x >= current.min {
                    if !create_peak(
                        current.min,
                        current.main,
                        current.max,
                        projection.value(current.main),
                        projection.area_above(current.min, current.max, parameters.min_peak_cumul),
                        parameters,
                        range,
                        peaks,
                        has_stem,
                    ) {
                        return;
                    }
                    x = current.max;
                    // Determine the next suitable hilo, if any.
                    hilo = match hilo_iter.next() {
                        Some(next) => {
                            let gap = next.min - current.max - 1;
                            if gap <= parameters.max_inner_peak_gap {
                                Some(next)
                            } else {
                                None
                            }
                        }
                        None => None,
                    };
                }
            }
        } else {
            if space_start == -1 {
                space_start = x;
            }
            space_stop = x;
        }
        x += 1;
    }

    if space_start != -1 {
        check_space(space_start, space_stop, parameters, range, peaks);
    }
}

/// Java `ShapeBuilder.checkSpace`.
fn check_space(
    space_start: i32,
    space_stop: i32,
    parameters: &KeyPipelineParameters,
    range: &mut StaffHeaderRange,
    peaks: &[KeyPeak],
) -> bool {
    let space_width = space_stop - space_start + 1;
    if !range.has_start() {
        if space_width > parameters.max_first_space_width {
            // No key signature at all.
            return false;
        }
        range.set_start(space_stop + 1);
        true
    } else if peaks.is_empty() {
        range.set_start(space_stop + 1);
        true
    } else if space_width > parameters.max_inner_space {
        range.shrink_stop(space_start - 1);
        false
    } else {
        true
    }
}

/// Java `ShapeBuilder.createPeak`. Returns false when browsing must stop.
#[expect(clippy::too_many_arguments, reason = "mirrors the Java method")]
fn create_peak(
    start: i32,
    main: i32,
    stop: i32,
    height: i32,
    area: i32,
    parameters: &KeyPipelineParameters,
    range: &mut StaffHeaderRange,
    peaks: &mut Vec<KeyPeak>,
    has_stem: &mut dyn FnMut(&KeyPeak) -> bool,
) -> bool {
    let peak = KeyPeak {
        min: start,
        main,
        max: stop,
        height,
        area,
    };
    let mut invalid = false;
    if height > parameters.max_peak_cumul || peak.width() > parameters.max_peak_width {
        invalid = true;
        range.shrink_stop(peak.min - 1);
    } else {
        // A non-stem-like peak is simply ignored, not fatal.
        if !has_stem(&peak) {
            return true;
        }
        if let Some(last) = peaks.last() {
            let x = f64::from(start + stop) / 2.0;
            let dx = x - f64::from(last.min + last.max) / 2.0;
            if dx > f64::from(parameters.max_peak_dx) {
                invalid = true;
                range.shrink_stop(peak.min - 1);
            }
        } else {
            let offset = start - range.browse_start;
            if offset > parameters.max_first_peak_offset {
                invalid = true;
                range.shrink_stop(peak.min - 1);
            } else if !range.has_start() {
                range.set_start(range.browse_start);
            }
        }
    }
    if invalid {
        false
    } else {
        peaks.push(peak);
        true
    }
}

/// Java `ShapeBuilder.mergePeaks`: adjacent peaks (gap <= 1) merge into one.
pub fn merge_peaks(
    peaks: &mut Vec<KeyPeak>,
    projection: &IntegerFunction,
    parameters: &KeyPipelineParameters,
) {
    let mut index = 1;
    while index < peaks.len() {
        let previous = peaks[index - 1];
        let peak = peaks[index];
        if peak.min - previous.max <= 1 {
            let merged = KeyPeak {
                min: previous.min,
                main: projection.arg_max(previous.min, peak.max),
                max: peak.max,
                height: previous.height.max(peak.height),
                area: projection.area_above(previous.min, peak.max, parameters.min_peak_cumul),
            };
            peaks[index - 1] = merged;
            peaks.remove(index);
        } else {
            index += 1;
        }
    }
}

/// Java `ShapeBuilder.purgeLightPeaks`: discard peaks well under the area quorum that also sit
/// close to a stronger peak.
///
/// The removal is iterative in **ascending area order**, and each distance test runs against the
/// *remaining* members of that ordering — a peak removed earlier no longer shields a later one.
pub fn purge_light_peaks(
    peaks: &mut Vec<KeyPeak>,
    shape: KeyShapeKind,
    parameters: &KeyPipelineParameters,
) {
    let count = peaks.len();
    if count <= 1 {
        return;
    }
    let mut sorted: Vec<KeyPeak> = peaks.clone();
    sorted.sort_by_key(|peak| peak.area);

    // Quorum over all but the smallest peak.
    let total_area: i32 = sorted[1..].iter().map(|peak| peak.area).sum();
    let quorum = (parameters.peak_area_quorum * f64::from(total_area)) / (count - 1) as f64;

    let min_dx = match shape {
        KeyShapeKind::Sharp => parameters.min_sharp_light_peak_dx,
        KeyShapeKind::Flat => parameters.min_flat_light_peak_dx,
    };

    let mut index = 0;
    while index < sorted.len() {
        let peak = sorted[index];
        if f64::from(peak.area) >= quorum {
            break;
        }
        let smallest_dx = sorted
            .iter()
            .enumerate()
            .filter(|&(other_index, _)| other_index != index)
            .map(|(_, other)| (other.center() - peak.center()).abs())
            .fold(f64::MAX, f64::min);
        if smallest_dx < min_dx {
            sorted.remove(index);
        } else {
            index += 1;
        }
    }
    peaks.retain(|peak| sorted.contains(peak));
}

/// Java `ShapeBuilder.inferSignature`: the signed item count read off the peaks alone.
///
/// Negative for flats, positive for sharps, zero for "no key of this shape here". May discard
/// trailing peaks (odd sharp count, unacceptable lone peak) exactly as Java does.
pub fn infer_signature(
    peaks: &mut Vec<KeyPeak>,
    range: &mut StaffHeaderRange,
    shape: KeyShapeKind,
    parameters: &KeyPipelineParameters,
) -> i32 {
    let mut peak_count = peaks.len();
    if peak_count == 0 {
        return 0;
    }
    let first = peaks[0];
    let last = *peaks.last().expect("non-empty");
    let heading = first.min - range.start().unwrap_or(range.browse_start);

    if peak_count > 1 {
        let mean_dx = (last.center() - first.center()) / (peak_count - 1) as f64;
        let mut shorts = 0;
        let mut sum = 0.0;
        for window in peaks.windows(2) {
            let interval = window[1].center() - window[0].center();
            if interval <= mean_dx {
                shorts += 1;
                sum += interval;
            }
        }
        let mean_short = sum / f64::from(shorts);

        if mean_short < parameters.min_flat_delta {
            if shape != KeyShapeKind::Sharp {
                return 0;
            }
        } else if mean_short > parameters.max_sharp_delta {
            if shape != KeyShapeKind::Flat {
                return 0;
            }
        } else if heading > parameters.max_flat_heading && shape != KeyShapeKind::Sharp {
            return 0;
        }

        if shape == KeyShapeKind::Sharp {
            if peak_count % 2 != 0 {
                // Discard the last peak to restore an even count.
                range.shrink_stop(last.min - 1);
                peaks.truncate(peak_count - 1);
                peak_count -= 1;
            }
            if peak_count / 2 > 7 {
                0
            } else {
                i32::try_from(peak_count / 2).unwrap_or(0)
            }
        } else if peak_count > 7 {
            0
        } else {
            -i32::try_from(peak_count).unwrap_or(0)
        }
    } else if heading <= parameters.max_flat_heading {
        if shape == KeyShapeKind::Flat { -1 } else { 0 }
    } else {
        // A lone late peak is not acceptable as a key.
        range.shrink_stop(last.min - 1);
        peaks.clear();
        0
    }
}

/// Java `ShapeBuilder.checkPeakDeltas`: cut the signature where inter-item spacing jumps.
fn check_peak_deltas(
    signature: i32,
    peaks: &mut Vec<KeyPeak>,
    range: &mut StaffHeaderRange,
    parameters: &KeyPipelineParameters,
) -> i32 {
    let count = signature.unsigned_abs() as usize;
    let items_per_peak = if signature < 0 { 1 } else { 2 };
    let mut total_dx = 0.0;
    let mut max_dx = f64::MIN;
    let mut dxs = vec![0.0; count - 1];

    let mut ip = items_per_peak;
    while ip < peaks.len() {
        let dx = peaks[ip].center() - peaks[ip - 1].center();
        total_dx += dx;
        max_dx = max_dx.max(dx);
        dxs[(ip / items_per_peak) - 1] = dx;
        ip += items_per_peak;
    }

    let w_mean = (count - 2) as f64;
    let mean_dx = (total_dx - max_dx) / w_mean;
    let w_max = 3.5;
    let max = ((w_mean * mean_dx) + (w_max * f64::from(parameters.max_peak_dx))) / (w_mean + w_max);

    for (is, &dx) in dxs.iter().enumerate() {
        if dx > max {
            let ip = items_per_peak * (is + 1);
            let peak = peaks[ip];
            range.shrink_stop(peak.min - 1);
            peaks.truncate(ip);
            return if signature < 0 {
                -i32::try_from(is + 1).unwrap_or(0)
            } else {
                i32::try_from(is + 1).unwrap_or(0)
            };
        }
    }
    signature
}

/// Java `ShapeBuilder.refineSignature`: trailing-length checks, then spacing regularity.
pub fn refine_signature(
    mut signature: i32,
    peaks: &mut Vec<KeyPeak>,
    range: &mut StaffHeaderRange,
    parameters: &KeyPipelineParameters,
) -> i32 {
    if signature == 0 {
        return 0;
    }
    let last = *peaks.last().expect("non-zero signature implies peaks");
    let trail = range.stop() - last.min + 1;

    if signature < 0 {
        if trail < parameters.min_flat_trail {
            let keep = peaks.len() - 1;
            peaks.truncate(keep);
            range.shrink_stop(last.min - 1);
            signature += 1;
        }
    } else if trail < parameters.min_sharp_trail {
        // Remove the last two peaks.
        peaks.truncate(peaks.len() - 1);
        let last = *peaks.last().expect("a sharp item has two peaks");
        peaks.truncate(peaks.len() - 1);
        range.shrink_stop(last.min - 1);
        signature -= 1;
    }

    if signature.abs() > 2 {
        signature = check_peak_deltas(signature, peaks, range, parameters);
    }
    signature
}

/// Java `ShapeBuilder.refineShapeStop`: place the range stop at the projection minimum inside the
/// trailing window after the last peak.
fn refine_shape_stop(
    last_peak: &KeyPeak,
    typical_trail: i32,
    max_trail: i32,
    projection: &IntegerFunction,
    range: &mut StaffHeaderRange,
) {
    let x_min = (last_peak.min + typical_trail) - 1;
    let x_max = projection.x_max().min(last_peak.min + max_trail);

    let mut min_count = i32::MAX;
    let mut new_stop = None;
    for x in x_min..=x_max {
        let count = projection.value(x);
        if count < min_count {
            new_stop = Some(x - 1);
            min_count = count;
        }
    }
    if let Some(stop) = new_stop {
        range.shrink_stop(stop);
    }
}

/// Java `ShapeBuilder.computeStarts`: one start abscissa per expected alteration.
pub fn compute_starts(
    signature: i32,
    peaks: &[KeyPeak],
    projection: &IntegerFunction,
    parameters: &KeyPipelineParameters,
    range: &mut StaffHeaderRange,
) -> Vec<i32> {
    let mut starts = Vec::new();
    if signature > 0 {
        starts.push(range.start().unwrap_or(range.browse_start));
        let mut index = 2;
        while index < peaks.len() {
            let peak = peaks[index];
            let previous = peaks[index - 1];
            starts.push((0.5 * f64::from(peak.min + previous.max)).ceil() as i32);
            index += 2;
        }
        refine_shape_stop(
            peaks.last().expect("positive signature implies peaks"),
            parameters.std_sharp_trail,
            parameters.max_sharp_trail,
            projection,
            range,
        );
    } else if signature < 0 {
        let first = peaks[0];
        let flat_heading = first.min - range.start().unwrap_or(range.browse_start);
        if flat_heading <= parameters.max_flat_heading {
            starts.push(range.start().unwrap_or(range.browse_start));
            for peak in &peaks[1..] {
                starts.push(peak.min);
            }
            refine_shape_stop(
                peaks.last().expect("negative signature implies peaks"),
                parameters.std_flat_trail,
                parameters.max_flat_trail,
                projection,
                range,
            );
        }
    }
    starts
}

/// Java `ShapeBuilder.allocateSlices`: tile `[start, next_start - 1]`, the last one to the stop.
#[must_use]
pub fn allocate_slices(starts: &[i32], range: &StaffHeaderRange) -> Vec<(i32, i32)> {
    let count = starts.len();
    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            let stop = if index < count - 1 {
                starts[index + 1] - 1
            } else {
                range.stop()
            };
            (start, stop)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_parameters::KeyPipelineParameters;

    fn parameters() -> KeyPipelineParameters {
        // Interline 21 both sheet and staff, the corpus' most common.
        KeyPipelineParameters::new(21, 21)
    }

    fn function(values: &[(i32, i32)]) -> IntegerFunction {
        let x_min = values.first().unwrap().0;
        let x_max = values.last().unwrap().0;
        let mut function = IntegerFunction::new(x_min, x_max);
        for &(x, value) in values {
            function.set_value(x, value);
        }
        function
    }

    #[test]
    fn merge_joins_only_adjacent_peaks() {
        let projection = function(&[(0, 0), (1, 9), (2, 9), (3, 4), (4, 9), (5, 0), (9, 0)]);
        let parameters = parameters();
        let mut peaks = vec![
            KeyPeak {
                min: 1,
                main: 1,
                max: 2,
                height: 9,
                area: 5,
            },
            KeyPeak {
                min: 3,
                main: 4,
                max: 4,
                height: 9,
                area: 3,
            },
        ];
        // Java merges when `peak.min - prevPeak.max <= 1`, i.e. truly adjacent columns with no
        // blank between them. min=3 against max=2 qualifies; a one-column gap would not — the
        // first version of this fixture had exactly that and the test caught the misreading.
        merge_peaks(&mut peaks, &projection, &parameters);
        assert_eq!(peaks.len(), 1);
        assert_eq!((peaks[0].min, peaks[0].max), (1, 4));
        assert_eq!(peaks[0].main, 1, "argMax keeps the first of tied maxima");
    }

    #[test]
    fn a_lone_late_peak_clears_everything() {
        let mut range = StaffHeaderRange::default();
        range.browse_start = 100;
        range.browse_stop = 200;
        range.set_start(100);
        let mut peaks = vec![KeyPeak {
            min: 150,
            main: 151,
            max: 153,
            height: 30,
            area: 40,
        }];
        // Heading 50 is far beyond maxFlatHeading (4 at interline 21).
        let signature = infer_signature(&mut peaks, &mut range, KeyShapeKind::Flat, &parameters());
        assert_eq!(signature, 0);
        assert!(peaks.is_empty(), "the lone peak is discarded");
        assert_eq!(range.stop(), 149, "and the stop shrinks before it");
    }

    #[test]
    fn sharps_require_an_even_peak_count() {
        let mut range = StaffHeaderRange::default();
        range.browse_stop = 100;
        range.set_start(0);
        // Five evenly spaced stems ~10 px apart: sharp-like spacing (maxSharpDelta = 14.7).
        let mut peaks = (0..5)
            .map(|index| KeyPeak {
                min: index * 10,
                main: index * 10 + 1,
                max: index * 10 + 2,
                height: 40,
                area: 50,
            })
            .collect::<Vec<_>>();
        let signature = infer_signature(&mut peaks, &mut range, KeyShapeKind::Sharp, &parameters());
        assert_eq!(signature, 2, "five peaks lose one, two sharps remain");
        assert_eq!(peaks.len(), 4);
    }

    #[test]
    fn slices_tile_the_range_without_gaps() {
        let mut range = StaffHeaderRange::default();
        range.browse_start = 10;
        range.browse_stop = 90;
        range.set_start(10);
        range.set_stop(80);
        let slices = allocate_slices(&[10, 30, 55], &range);
        assert_eq!(slices, vec![(10, 29), (30, 54), (55, 80)]);
    }
}
