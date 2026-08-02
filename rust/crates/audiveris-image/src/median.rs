// SPDX-License-Identifier: AGPL-3.0-or-later

#[must_use]
pub fn median_gray(width: usize, height: usize, input: &[u8], radius: usize) -> Vec<u8> {
    assert_eq!(input.len(), width * height);
    let mut output = vec![0; input.len()];
    let mut histogram = [0usize; 256];
    for y in 0..height {
        for x in 0..width {
            // This deliberately matches Audiveris's symmetric boundary behavior: the radius is
            // reduced to the nearest image edge rather than padding or clipping each side.
            let local_radius = radius.min(x).min(y).min(width - 1 - x).min(height - 1 - y);
            histogram.fill(0);
            for sample_y in y - local_radius..=y + local_radius {
                for sample_x in x - local_radius..=x + local_radius {
                    histogram[usize::from(input[sample_y * width + sample_x])] += 1;
                }
            }
            let side = 2 * local_radius + 1;
            let median_count = (side * side).div_ceil(2);
            // The Java implementation walks down from white (255), selecting the upper median.
            let mut sum = 0;
            let mut median = 255;
            while sum < median_count {
                sum += histogram[median];
                if sum < median_count {
                    median -= 1;
                }
            }
            output[y * width + x] = u8::try_from(median).expect("histogram index is a byte");
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_java_black_and_white_invariants() {
        assert_eq!(median_gray(10, 4, &[0; 40], 1), [0; 40]);
        assert_eq!(median_gray(10, 4, &[255; 40], 1), [255; 40]);
    }

    #[test]
    fn ports_java_eight_by_eight_example() {
        let mut input = vec![255; 64];
        for (x, y) in [(2, 5), (3, 2), (3, 3), (3, 4), (4, 2), (4, 3)] {
            input[y * 8 + x] = 0;
        }
        let result = median_gray(8, 8, &input, 1);
        let black: Vec<_> = result
            .iter()
            .enumerate()
            .filter_map(|(index, &pixel)| (pixel == 0).then_some((index % 8, index / 8)))
            .collect();
        assert_eq!(black, [(3, 3), (4, 3)]);
    }

    #[test]
    fn preserves_boundary_pixels_by_reducing_radius() {
        let input = [7, 255, 255, 255, 255, 255, 255, 255, 9];
        let result = median_gray(3, 3, &input, 1);
        assert_eq!(result[0], 7);
        assert_eq!(result[8], 9);
    }
}
