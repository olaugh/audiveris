// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub min: i32,
    pub main: i32,
    pub max: i32,
}

impl Range {
    #[must_use]
    pub const fn new(min: i32, main: i32, max: i32) -> Self {
        Self { min, main, max }
    }

    #[must_use]
    pub const fn width(self) -> i32 {
        self.max - self.min + 1
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{},{})", self.min, self.main, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_width_and_display() {
        let range = Range::new(3, 5, 8);
        assert_eq!(range.width(), 6);
        assert_eq!(range.to_string(), "(3,5,8)");
    }
}
