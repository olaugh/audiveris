// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::VecDeque;

const GRAY_LEVELS: usize = 256;
const WATERSHED: i32 = -1;
const NEIGHBORS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatershedResult {
    pub lines: Vec<bool>,
    pub region_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct Pixel {
    x: usize,
    y: usize,
}

/// Segment a row-major 8-bit gray image using the Audiveris flooding order.
///
/// `step` controls how many adjacent gray-level queues can be explored together. A value of one
/// reproduces the Java test and the usual strict level-by-level traversal.
#[must_use]
pub fn watershed_gray_level(
    width: usize,
    height: usize,
    image: &[u8],
    bright_on_dark: bool,
    step: usize,
) -> WatershedResult {
    assert_eq!(image.len(), width * height);
    assert!(step > 0);
    let image: Vec<u8> = image
        .iter()
        .map(|&value| if bright_on_dark { 255 - value } else { value })
        .collect();
    let mut regions = vec![0i32; image.len()];
    let mut queues: Vec<VecDeque<Pixel>> = (0..GRAY_LEVELS).map(|_| VecDeque::new()).collect();
    let mut max_region = 0i32;
    let mut level = 0usize;
    let mut y_offset = 0usize;

    while level < GRAY_LEVELS {
        while let Some(pixel) = next_pixel(&mut queues, level, step) {
            extend(pixel, width, height, &image, &mut regions, &mut queues);
        }

        if let Some(seed) = find_seed(level as u8, y_offset, width, height, &image, &regions) {
            max_region += 1;
            regions[seed.y * width + seed.x] = max_region;
            y_offset = seed.y;
            queues[level].push_back(seed);
        } else {
            level += 1;
            y_offset = 0;
        }
    }

    WatershedResult {
        lines: regions.iter().map(|&region| region == WATERSHED).collect(),
        region_count: usize::try_from(max_region).expect("region IDs remain positive"),
    }
}

fn next_pixel(queues: &mut [VecDeque<Pixel>], level: usize, step: usize) -> Option<Pixel> {
    for queue in queues
        .iter_mut()
        .take((level + step).min(GRAY_LEVELS))
        .skip(level)
    {
        if let Some(pixel) = queue.pop_front() {
            return Some(pixel);
        }
    }
    None
}

fn find_seed(
    level: u8,
    y_offset: usize,
    width: usize,
    height: usize,
    image: &[u8],
    regions: &[i32],
) -> Option<Pixel> {
    for y in y_offset..height {
        for x in 0..width {
            let index = y * width + x;
            if image[index] == level && regions[index] == 0 {
                return Some(Pixel { x, y });
            }
        }
    }
    None
}

fn extend(
    pixel: Pixel,
    width: usize,
    height: usize,
    image: &[u8],
    regions: &mut [i32],
    queues: &mut [VecDeque<Pixel>],
) {
    let region = regions[pixel.y * width + pixel.x];
    if region == WATERSHED {
        return;
    }
    for (dx, dy) in NEIGHBORS {
        let Some(x) = pixel.x.checked_add_signed(dx) else {
            continue;
        };
        let Some(y) = pixel.y.checked_add_signed(dy) else {
            continue;
        };
        if x >= width || y >= height {
            continue;
        }
        let index = y * width + x;
        let neighbor_region = regions[index];
        if neighbor_region == WATERSHED {
            continue;
        }
        if neighbor_region == 0 {
            regions[index] = region;
            queues[usize::from(image[index])].push_back(Pixel { x, y });
        } else if neighbor_region != region {
            regions[index] = WATERSHED;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_flat_basin_is_one_region() {
        let result = watershed_gray_level(3, 3, &[10; 9], false, 1);
        assert_eq!(result.region_count, 1);
        assert_eq!(result.lines, [false; 9]);
    }

    #[test]
    fn two_minima_create_a_watershed() {
        let image = [0, 10, 20, 10, 0, 0, 10, 20, 10, 0, 0, 10, 20, 10, 0];
        let result = watershed_gray_level(5, 3, &image, false, 1);
        assert_eq!(result.region_count, 2);
        assert!(result.lines.iter().any(|&line| line));
        assert!(result.lines[2]);
        assert!(result.lines[7]);
        assert!(result.lines[12]);
    }

    #[test]
    fn bright_on_dark_matches_explicit_inversion() {
        let image = [0, 64, 255, 64, 0];
        let inverted: Vec<_> = image.iter().map(|&value| 255 - value).collect();
        assert_eq!(
            watershed_gray_level(5, 1, &image, true, 1),
            watershed_gray_level(5, 1, &inverted, false, 1)
        );
    }
}
