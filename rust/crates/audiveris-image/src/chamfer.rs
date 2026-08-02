// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::run_table::FOREGROUND;

pub const VALUE_TARGET: i32 = 0;
pub const VALUE_UNKNOWN: i32 = -1;
pub const CHESSBOARD: &[(isize, isize, i32)] = &[(1, 0, 1), (1, 1, 1)];
pub const CHAMFER_3: &[(isize, isize, i32)] = &[(1, 0, 3), (1, 1, 4)];
pub const CHAMFER_5: &[(isize, isize, i32)] = &[(1, 0, 5), (1, 1, 7), (2, 1, 11)];
pub const CHAMFER_7: &[(isize, isize, i32)] = &[(1, 0, 14), (1, 1, 20), (2, 1, 31), (3, 1, 44)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceTable {
    width: usize,
    height: usize,
    normalizer: i32,
    values: Vec<i32>,
}

impl DistanceTable {
    fn new(width: usize, height: usize, normalizer: i32) -> Self {
        Self {
            width,
            height,
            normalizer,
            values: vec![VALUE_UNKNOWN; width * height],
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub const fn normalizer(&self) -> i32 {
        self.normalizer
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> i32 {
        self.values[y * self.width + x]
    }

    #[must_use]
    pub fn normalized(&self, x: usize, y: usize) -> f64 {
        f64::from(self.get(x, y)) / f64::from(self.normalizer)
    }

    fn set(&mut self, x: usize, y: usize, value: i32) {
        self.values[y * self.width + x] = value;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ChamferDistance<'a> {
    mask: &'a [(isize, isize, i32)],
}

impl Default for ChamferDistance<'static> {
    fn default() -> Self {
        Self { mask: CHAMFER_3 }
    }
}

impl<'a> ChamferDistance<'a> {
    #[must_use]
    pub const fn new(mask: &'a [(isize, isize, i32)]) -> Self {
        Self { mask }
    }

    #[must_use]
    pub fn compute(&self, width: usize, height: usize, targets: &[bool]) -> DistanceTable {
        assert_eq!(targets.len(), width * height);
        let mut output = DistanceTable::new(width, height, self.mask[0].2);
        for (index, &target) in targets.iter().enumerate() {
            output.values[index] = if target { VALUE_TARGET } else { VALUE_UNKNOWN };
        }
        self.process(&mut output);
        output
    }

    #[must_use]
    pub fn compute_to_fore(&self, width: usize, height: usize, pixels: &[u8]) -> DistanceTable {
        let targets: Vec<bool> = pixels.iter().map(|&pixel| pixel == FOREGROUND).collect();
        self.compute(width, height, &targets)
    }

    #[must_use]
    pub fn compute_to_back(&self, width: usize, height: usize, pixels: &[u8]) -> DistanceTable {
        let targets: Vec<bool> = pixels.iter().map(|&pixel| pixel != FOREGROUND).collect();
        self.compute(width, height, &targets)
    }

    fn test_and_set(output: &mut DistanceTable, x: isize, y: isize, new_value: i32) {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return;
        };
        if x >= output.width || y >= output.height {
            return;
        }
        let current = output.get(x, y);
        if current >= 0 && current < new_value {
            return;
        }
        output.set(x, y, new_value);
    }

    fn update_forward(&self, output: &mut DistanceTable, x: usize, y: usize, value: i32) {
        let (x, y) = (x as isize, y as isize);
        for &(dx, dy, distance) in self.mask {
            Self::test_and_set(output, x + dx, y + dy, value + distance);
            if dy != 0 {
                Self::test_and_set(output, x - dx, y + dy, value + distance);
            }
            if dx != dy {
                Self::test_and_set(output, x + dy, y + dx, value + distance);
                if dy != 0 {
                    Self::test_and_set(output, x - dy, y + dx, value + distance);
                }
            }
        }
    }

    fn update_backward(&self, output: &mut DistanceTable, x: usize, y: usize, value: i32) {
        let (x, y) = (x as isize, y as isize);
        for &(dx, dy, distance) in self.mask {
            Self::test_and_set(output, x - dx, y - dy, value + distance);
            if dy != 0 {
                Self::test_and_set(output, x + dx, y - dy, value + distance);
            }
            if dx != dy {
                Self::test_and_set(output, x - dy, y - dx, value + distance);
                if dy != 0 {
                    Self::test_and_set(output, x + dy, y - dx, value + distance);
                }
            }
        }
    }

    fn process(&self, output: &mut DistanceTable) {
        for y in 0..output.height {
            for x in 0..output.width {
                let value = output.get(x, y);
                if value != VALUE_UNKNOWN {
                    self.update_forward(output, x, y, value);
                }
            }
        }
        for y in (0..output.height).rev() {
            for x in (0..output.width).rev() {
                let value = output.get(x, y);
                if value != VALUE_UNKNOWN {
                    self.update_backward(output, x, y, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::BACKGROUND;

    #[test]
    fn computes_java_default_chamfer_distances() {
        let mut targets = vec![false; 25];
        targets[2 * 5 + 2] = true;
        let distances = ChamferDistance::default().compute(5, 5, &targets);
        assert_eq!(distances.normalizer(), 3);
        assert_eq!(distances.get(2, 2), 0);
        assert_eq!(distances.get(3, 2), 3);
        assert_eq!(distances.get(3, 3), 4);
        assert_eq!(distances.get(4, 2), 6);
        assert_eq!(distances.get(4, 4), 8);
        assert_eq!(distances.get(0, 0), 8);
    }

    #[test]
    fn computes_foreground_and_background_transforms() {
        let pixels = [BACKGROUND, BACKGROUND, BACKGROUND, FOREGROUND, BACKGROUND];
        let transform = ChamferDistance::default();
        let foreground = transform.compute_to_fore(5, 1, &pixels);
        assert_eq!((foreground.get(0, 0), foreground.get(3, 0)), (9, 0));
        let background = transform.compute_to_back(5, 1, &pixels);
        assert_eq!((background.get(0, 0), background.get(3, 0)), (0, 3));
    }

    #[test]
    fn supports_other_java_masks() {
        let mut targets = vec![false; 9];
        targets[4] = true;
        let chessboard = ChamferDistance::new(CHESSBOARD).compute(3, 3, &targets);
        assert_eq!(chessboard.get(0, 0), 1);
        let five = ChamferDistance::new(CHAMFER_5).compute(3, 3, &targets);
        assert_eq!(five.get(0, 0), 7);
    }
}
