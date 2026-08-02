// SPDX-License-Identifier: AGPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolygonMask {
    polygon: Vec<Point>,
    points: Vec<(i32, i32)>,
}

impl PolygonMask {
    #[must_use]
    pub fn new(polygon: Vec<Point>) -> Self {
        assert!(
            polygon.len() >= 3,
            "a mask polygon needs at least three vertices"
        );
        let min_x = polygon
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min)
            .floor() as i32;
        let max_x = polygon
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil() as i32;
        let min_y = polygon
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min)
            .floor() as i32;
        let max_y = polygon
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil() as i32;

        let mut points = Vec::new();
        for y in min_y..max_y {
            for x in min_x..max_x {
                if contains(&polygon, f64::from(x), f64::from(y)) {
                    points.push((x, y));
                }
            }
        }
        Self { polygon, points }
    }

    #[must_use]
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    pub fn points(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.points.iter().copied()
    }

    pub fn apply(&self, mut adapter: impl FnMut(i32, i32)) {
        for &(x, y) in &self.points {
            adapter(x, y);
        }
    }

    #[must_use]
    pub fn polygon(&self) -> &[Point] {
        &self.polygon
    }
}

fn contains(polygon: &[Point], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current.y > y) != (previous.y > y) {
            let crossing =
                (previous.x - current.x) * (y - current.y) / (previous.y - current.y) + current.x;
            if x < crossing {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_java_rectangle_mask_count_and_apply_order() {
        let mask = PolygonMask::new(vec![
            Point::new(10.0, 20.0),
            Point::new(13.0, 20.0),
            Point::new(13.0, 22.0),
            Point::new(10.0, 22.0),
        ]);
        assert_eq!(mask.point_count(), 6);
        assert_eq!(
            mask.points().collect::<Vec<_>>(),
            [(10, 20), (11, 20), (12, 20), (10, 21), (11, 21), (12, 21)]
        );
        let mut applied = Vec::new();
        mask.apply(|x, y| applied.push((x, y)));
        assert_eq!(applied, mask.points().collect::<Vec<_>>());
    }

    #[test]
    fn handles_non_rectangular_area() {
        let mask = PolygonMask::new(vec![
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(0.0, 3.0),
        ]);
        assert!(mask.points().any(|point| point == (0, 0)));
        assert!(!mask.points().any(|point| point == (2, 2)));
    }
}
