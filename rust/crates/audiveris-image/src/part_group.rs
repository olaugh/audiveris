// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cycle-free value form of Java GRID `PartGroup`.

use std::{cmp::Ordering, fmt};

/// Symbol displayed at the left of a grouped set of parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartGroupingSymbol {
    Bracket,
    Brace,
    Square,
}

impl fmt::Display for PartGroupingSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bracket => "bracket",
            Self::Brace => "brace",
            Self::Square => "square",
        })
    }
}

/// Persistent part grouping discovered from braces, brackets, or barlines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartGroup {
    number: i32,
    symbol: PartGroupingSymbol,
    barline: bool,
    first_staff_id: i32,
    last_staff_id: i32,
    name: Option<String>,
    abbreviation: Option<String>,
}

impl PartGroup {
    /// Java's public constructor leaves range validation to its caller and
    /// initially makes the group span only its first staff.
    #[must_use]
    pub const fn new(
        number: i32,
        symbol: PartGroupingSymbol,
        barline: bool,
        first_staff_id: i32,
    ) -> Self {
        Self {
            number,
            symbol,
            barline,
            first_staff_id,
            last_staff_id: first_staff_id,
            name: None,
            abbreviation: None,
        }
    }

    #[must_use]
    pub const fn number(&self) -> i32 {
        self.number
    }

    #[must_use]
    pub const fn symbol(&self) -> PartGroupingSymbol {
        self.symbol
    }

    #[must_use]
    pub const fn is_barline(&self) -> bool {
        self.barline
    }

    #[must_use]
    pub const fn is_brace(&self) -> bool {
        matches!(self.symbol, PartGroupingSymbol::Brace)
    }

    #[must_use]
    pub const fn first_staff_id(&self) -> i32 {
        self.first_staff_id
    }

    #[must_use]
    pub const fn last_staff_id(&self) -> i32 {
        self.last_staff_id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn abbreviation(&self) -> Option<&str> {
        self.abbreviation.as_deref()
    }

    pub fn set_last_staff_id(&mut self, last_staff_id: i32) {
        self.last_staff_id = last_staff_id;
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub fn set_abbreviation(&mut self, abbreviation: Option<String>) {
        self.abbreviation = abbreviation;
    }

    /// Java `byFirstId` comparator; equal first IDs remain equal even when
    /// other fields differ.
    #[must_use]
    pub fn compare_by_first_id(left: &Self, right: &Self) -> Ordering {
        left.first_staff_id.cmp(&right.first_staff_id)
    }
}

impl fmt::Display for PartGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "PartGroup{{{} number:{}",
            self.symbol, self.number
        )?;
        if let Some(name) = &self.name {
            write!(formatter, " name:{name}")?;
        }
        if let Some(abbreviation) = &self.abbreviation {
            write!(formatter, " abbr:{abbreviation}")?;
        }
        write!(
            formatter,
            " barline:{} staves:{}",
            self.barline, self.first_staff_id
        )?;
        if self.last_staff_id != self.first_staff_id {
            write!(formatter, "-{}", self.last_staff_id)?;
        }
        formatter.write_str("}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_mutators_match_java_value_state() {
        let mut group = PartGroup::new(2, PartGroupingSymbol::Brace, true, 3);
        assert_eq!(group.number(), 2);
        assert_eq!(group.symbol(), PartGroupingSymbol::Brace);
        assert!(group.is_barline());
        assert!(group.is_brace());
        assert_eq!((group.first_staff_id(), group.last_staff_id()), (3, 3));
        assert_eq!((group.name(), group.abbreviation()), (None, None));
        assert_eq!(
            group.to_string(),
            "PartGroup{brace number:2 barline:true staves:3}"
        );

        group.set_last_staff_id(6);
        group.set_name(Some("Piano".into()));
        group.set_abbreviation(Some("Pno.".into()));
        assert_eq!(group.name(), Some("Piano"));
        assert_eq!(group.abbreviation(), Some("Pno."));
        assert_eq!(
            group.to_string(),
            "PartGroup{brace number:2 name:Piano abbr:Pno. barline:true staves:3-6}"
        );
    }

    #[test]
    fn symbols_and_first_staff_comparator_preserve_java_ordering() {
        let later = PartGroup::new(1, PartGroupingSymbol::Bracket, false, 8);
        let earlier = PartGroup::new(3, PartGroupingSymbol::Square, true, -2);
        let equal = PartGroup::new(9, PartGroupingSymbol::Brace, false, 8);

        assert!(!later.is_brace());
        assert_eq!(PartGroupingSymbol::Bracket.to_string(), "bracket");
        assert_eq!(PartGroupingSymbol::Square.to_string(), "square");
        assert_eq!(
            PartGroup::compare_by_first_id(&earlier, &later),
            Ordering::Less
        );
        assert_eq!(
            PartGroup::compare_by_first_id(&later, &equal),
            Ordering::Equal
        );
    }
}
