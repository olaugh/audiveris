// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light state contract for Java `StaffHeader`.

use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl HeaderBounds {
    /// Java `Rectangle`'s inclusive right edge (`x + width - 1`).
    #[must_use]
    pub const fn right(self) -> i32 {
        self.x.wrapping_add(self.width).wrapping_sub(1)
    }
}

/// Minimal common state of a clef, key, or time `Inter` needed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderComponent {
    pub id: usize,
    pub bounds: HeaderBounds,
    frozen: bool,
}

impl HeaderComponent {
    #[must_use]
    pub const fn new(id: usize, bounds: HeaderBounds) -> Self {
        Self {
            id,
            bounds,
            frozen: false,
        }
    }

    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaffHeaderRangeError {
    MissingStart,
}

impl fmt::Display for StaffHeaderRangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStart => formatter.write_str("staff header range has no precise start"),
        }
    }
}

impl Error for StaffHeaderRangeError {}

/// Java `StaffHeader.Range`, including its JAXB/default field values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaffHeaderRange {
    pub valid: bool,
    pub browse_start: i32,
    pub browse_stop: i32,
    start: Option<i32>,
    stop: Option<i32>,
}

impl StaffHeaderRange {
    pub fn start(&self) -> Result<i32, StaffHeaderRangeError> {
        self.start.ok_or(StaffHeaderRangeError::MissingStart)
    }

    #[must_use]
    pub fn stop(&self) -> i32 {
        self.stop.unwrap_or(self.browse_stop)
    }

    pub fn width(&self) -> Result<i32, StaffHeaderRangeError> {
        Ok(self.stop().wrapping_sub(self.start()?).wrapping_add(1))
    }

    #[must_use]
    pub const fn has_start(&self) -> bool {
        self.start.is_some()
    }

    pub fn set_start(&mut self, start: i32) {
        self.start = Some(start);
    }

    pub fn set_stop(&mut self, stop: i32) {
        self.stop = Some(stop);
    }

    /// Java `shrinkStop`: equal values are accepted; larger values are ignored.
    pub fn shrink_stop(&mut self, stop: i32) {
        if self.stop.is_none_or(|current| stop <= current) {
            self.stop = Some(stop);
        }
    }

    #[must_use]
    pub const fn precise_stop(&self) -> Option<i32> {
        self.stop
    }
}

impl fmt::Display for StaffHeaderRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(if self.valid { "SUCCESS" } else { "FAILURE" })?;
        if self.browse_start != 0 {
            write!(formatter, " bStart={}", self.browse_start)?;
        }
        if self.browse_stop != 0 {
            write!(formatter, " bStop={}", self.browse_stop)?;
        }
        if let Some(start) = self.start {
            write!(formatter, " start={start}")?;
        }
        if let Some(stop) = self.stop {
            write!(formatter, " stop={stop}")?;
        }
        Ok(())
    }
}

/// Clef + optional key + optional time state at the beginning of one staff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaffHeader {
    pub start: i32,
    pub stop: i32,
    pub clef: Option<HeaderComponent>,
    pub key: Option<HeaderComponent>,
    pub time: Option<HeaderComponent>,
    pub clef_range: Option<StaffHeaderRange>,
    pub key_range: Option<StaffHeaderRange>,
    pub alter_starts: Option<Vec<i32>>,
    pub time_range: Option<StaffHeaderRange>,
}

impl Default for StaffHeader {
    /// Java's private JAXB constructor: `start == 0`; every other field keeps
    /// its JVM default, including `stop == 0`.
    fn default() -> Self {
        Self {
            start: 0,
            stop: 0,
            clef: None,
            key: None,
            time: None,
            clef_range: None,
            key_range: None,
            alter_starts: None,
            time_range: None,
        }
    }
}

impl StaffHeader {
    /// Java's public constructor initializes `stop` to `start`.
    #[must_use]
    pub fn new(start: i32) -> Self {
        Self {
            start,
            stop: start,
            ..Self::default()
        }
    }

    /// Freeze non-null components in Java's clef, key, time order.
    pub fn freeze(&mut self) {
        if let Some(clef) = &mut self.clef {
            clef.freeze();
        }
        if let Some(key) = &mut self.key {
            key.freeze();
        }
        if let Some(time) = &mut self.time {
            time.freeze();
        }
    }

    /// Right edge of time, else key, else clef, matching Java precedence.
    #[must_use]
    pub fn actual_stop(&self) -> Option<i32> {
        self.time
            .as_ref()
            .or(self.key.as_ref())
            .or(self.clef.as_ref())
            .map(|component| component.bounds.right())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(id: usize, x: i32, width: i32) -> HeaderComponent {
        HeaderComponent::new(
            id,
            HeaderBounds {
                x,
                y: 2,
                width,
                height: 8,
            },
        )
    }

    #[test]
    fn constructors_preserve_distinct_java_defaults() {
        assert_eq!(StaffHeader::default().start, 0);
        assert_eq!(StaffHeader::default().stop, 0);
        let header = StaffHeader::new(37);
        assert_eq!((header.start, header.stop), (37, 37));
        assert!(header.clef_range.is_none());
        assert!(header.alter_starts.is_none());
    }

    #[test]
    fn actual_stop_uses_time_then_key_then_clef() {
        let mut header = StaffHeader::new(10);
        assert_eq!(header.actual_stop(), None);
        header.clef = Some(component(1, 12, 5));
        assert_eq!(header.actual_stop(), Some(16));
        header.key = Some(component(2, 20, 3));
        assert_eq!(header.actual_stop(), Some(22));
        header.time = Some(component(3, 30, 4));
        assert_eq!(header.actual_stop(), Some(33));
    }

    #[test]
    fn freeze_touches_each_present_component_without_creating_absent_ones() {
        let mut header = StaffHeader::new(10);
        header.clef = Some(component(1, 12, 5));
        header.time = Some(component(3, 30, 4));
        header.freeze();
        assert!(header.clef.as_ref().unwrap().is_frozen());
        assert!(header.time.as_ref().unwrap().is_frozen());
        assert!(header.key.is_none());
    }

    #[test]
    fn range_defaults_fallbacks_and_shrink_rules_match_java() {
        let mut range = StaffHeaderRange::default();
        assert_eq!(range.start(), Err(StaffHeaderRangeError::MissingStart));
        assert_eq!(range.stop(), 0);
        assert_eq!(range.to_string(), "FAILURE");

        range.valid = true;
        range.browse_start = 10;
        range.browse_stop = 40;
        range.set_start(14);
        assert_eq!(range.stop(), 40);
        assert_eq!(range.width().unwrap(), 27);
        range.shrink_stop(31);
        range.shrink_stop(35);
        assert_eq!(range.precise_stop(), Some(31));
        range.shrink_stop(31);
        assert_eq!(
            range.to_string(),
            "SUCCESS bStart=10 bStop=40 start=14 stop=31"
        );
    }
}
