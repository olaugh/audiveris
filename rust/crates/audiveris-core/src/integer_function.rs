// SPDX-License-Identifier: AGPL-3.0-or-later

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerFunction {
    x_min: i32,
    x_max: i32,
    values: Vec<i32>,
}

impl IntegerFunction {
    #[must_use]
    pub fn new(x_min: i32, x_max: i32) -> Self {
        assert!(x_max >= x_min);
        let len = usize::try_from(i64::from(x_max) - i64::from(x_min) + 1)
            .expect("integer function domain fits memory");
        Self {
            x_min,
            x_max,
            values: vec![0; len],
        }
    }

    fn index(&self, x: i32) -> usize {
        usize::try_from(x - self.x_min).expect("x is below function domain")
    }

    pub fn add_value(&mut self, x: i32, delta: i32) {
        if (self.x_min..=self.x_max).contains(&x) {
            let index = self.index(x);
            self.values[index] = self.values[index].wrapping_add(delta);
        }
    }

    pub fn set_value(&mut self, x: i32, value: i32) {
        if (self.x_min..=self.x_max).contains(&x) {
            let index = self.index(x);
            self.values[index] = value;
        }
    }

    #[must_use]
    pub fn value(&self, x: i32) -> i32 {
        self.values[self.index(x)]
    }

    #[must_use]
    pub const fn x_min(&self) -> i32 {
        self.x_min
    }

    #[must_use]
    pub const fn x_max(&self) -> i32 {
        self.x_max
    }

    #[must_use]
    pub fn derivative(&self, x: i32) -> i32 {
        self.value(x).wrapping_sub(self.value(x - 1))
    }

    #[must_use]
    pub fn arg_max(&self, x1: i32, x2: i32) -> i32 {
        let mut main = x1;
        let mut max_value = self.value(main);
        for x in x1..=x2 {
            let value = self.value(x);
            if max_value < value {
                max_value = value;
                main = x;
            }
        }
        main
    }

    #[must_use]
    pub fn area(&self) -> i32 {
        self.area_above(self.x_min, self.x_max, 0)
    }

    #[must_use]
    pub fn area_above(&self, x1: i32, x2: i32, baseline: i32) -> i32 {
        (x1..=x2).fold(0i32, |area, x| {
            area.wrapping_add(0.max(self.value(x).wrapping_sub(baseline)))
        })
    }

    #[must_use]
    pub fn local_maxima(&self, mut x1: i32, mut x2: i32) -> Vec<i32> {
        x1 = x1.max(self.x_min);
        x2 = x2.min(self.x_max);
        let mut maxima = Vec::new();
        let mut previous: Option<(i32, i32)> = None;
        let mut growing = false;

        for x in x1..=x2 {
            let value = self.value(x);
            if let Some((previous_x, previous_value)) = previous {
                if value >= previous_value {
                    growing = true;
                } else {
                    if growing {
                        maxima.push(previous_x);
                    }
                    growing = false;
                }
            }
            previous = Some((x, value));
        }

        maxima.sort_by_key(|x| std::cmp::Reverse(self.value(*x)));
        maxima
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> IntegerFunction {
        let mut function = IntegerFunction::new(2, 9);
        for (x, value) in [
            (2, 1),
            (3, 4),
            (4, 4),
            (5, 2),
            (6, 5),
            (7, 1),
            (8, 3),
            (9, 3),
        ] {
            function.set_value(x, value);
        }
        function
    }

    #[test]
    fn matches_java_values_derivatives_area_and_first_argmax() {
        let mut function = fixture();
        function.add_value(3, 2);
        function.add_value(20, 99);
        assert_eq!(function.value(3), 6);
        assert_eq!(function.derivative(3), 5);
        assert_eq!(function.arg_max(2, 9), 3);
        assert_eq!(function.area(), 25);
        assert_eq!(function.area_above(2, 5, 2), 6);
    }

    #[test]
    fn matches_java_plateau_and_terminal_local_maxima_behavior() {
        let function = fixture();
        // Java selects the last point of a plateau, keeps stable order for tied
        // heights, and does not emit a maximum without a following descent.
        assert_eq!(function.local_maxima(0, 20), [6, 4]);
        assert_eq!(function.local_maxima(2, 5), [4]);
        assert_eq!(function.local_maxima(8, 9), []);
    }
}
