// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-neutral mutation boundary for Java `Staff.simplifyLines`.

/// Adapter over the staff line collection and sheet glyph index used while
/// replacing transient staff filaments with persistent staff lines.
///
/// `OriginalLine` is intentionally distinct from `SimplifiedLine`: Java's
/// input collection contains `StaffFilament` instances while the successful
/// output collection contains `StaffLine` instances.
pub trait StaffLinesSimplifier {
    type OriginalLine: Clone;
    type SimplifiedLine;
    type GlyphIndex;
    type Error;

    /// Capture `sheet.getGlyphIndex()` before inspecting or mutating lines.
    fn glyph_index(&mut self) -> Self::GlyphIndex;

    /// Shallow-copy the current staff lines in collection iteration order.
    fn snapshot_original_lines(&mut self) -> Vec<Self::OriginalLine>;

    /// Clear the staff's line collection before the first conversion.
    fn clear_lines(&mut self);

    /// Perform `StaffFilament.toStaffLine(glyphIndex)` for one original.
    ///
    /// A Java type-cast or conversion failure is represented by `Error`.
    fn simplify_line(
        &mut self,
        original: &Self::OriginalLine,
        glyph_index: &Self::GlyphIndex,
    ) -> Result<Self::SimplifiedLine, Self::Error>;

    /// Append one successfully converted line to the now-persistent list.
    fn append_simplified_line(&mut self, line: Self::SimplifiedLine);
}

/// Replace transient staff filaments with persistent staff lines in exact
/// Java `Staff.simplifyLines` order and return the shallow original snapshot.
///
/// Conversion errors propagate immediately. Since Java clears `lines` before
/// entering the conversion loop, already-appended persistent lines remain and
/// all unconverted originals stay detached when an error occurs.
pub fn simplify_staff_lines<Simplifier>(
    simplifier: &mut Simplifier,
) -> Result<Vec<Simplifier::OriginalLine>, Simplifier::Error>
where
    Simplifier: StaffLinesSimplifier,
{
    let glyph_index = simplifier.glyph_index();
    let originals = simplifier.snapshot_original_lines();
    simplifier.clear_lines();

    for original in &originals {
        let staff_line = simplifier.simplify_line(original, &glyph_index)?;
        simplifier.append_simplified_line(staff_line);
    }

    Ok(originals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Filament(usize);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct PersistentLine {
        source: usize,
        glyph_index: usize,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Call {
        GlyphIndex,
        Snapshot(Vec<usize>),
        Clear,
        Convert { filament: usize, glyph_index: usize },
        Append(usize),
    }

    #[derive(Debug)]
    struct RecordingStaff {
        originals: Vec<Filament>,
        persistent: Vec<PersistentLine>,
        calls: Vec<Call>,
        fail_on: Option<usize>,
    }

    impl RecordingStaff {
        fn new(originals: &[usize]) -> Self {
            Self {
                originals: originals.iter().copied().map(Filament).collect(),
                persistent: Vec::new(),
                calls: Vec::new(),
                fail_on: None,
            }
        }
    }

    impl StaffLinesSimplifier for RecordingStaff {
        type OriginalLine = Filament;
        type SimplifiedLine = PersistentLine;
        type GlyphIndex = usize;
        type Error = &'static str;

        fn glyph_index(&mut self) -> Self::GlyphIndex {
            self.calls.push(Call::GlyphIndex);
            41
        }

        fn snapshot_original_lines(&mut self) -> Vec<Self::OriginalLine> {
            self.calls.push(Call::Snapshot(
                self.originals.iter().map(|line| line.0).collect(),
            ));
            self.originals.clone()
        }

        fn clear_lines(&mut self) {
            self.calls.push(Call::Clear);
            self.originals.clear();
            self.persistent.clear();
        }

        fn simplify_line(
            &mut self,
            original: &Self::OriginalLine,
            glyph_index: &Self::GlyphIndex,
        ) -> Result<Self::SimplifiedLine, Self::Error> {
            self.calls.push(Call::Convert {
                filament: original.0,
                glyph_index: *glyph_index,
            });
            if self.fail_on == Some(original.0) {
                return Err("filament conversion failed");
            }
            Ok(PersistentLine {
                source: original.0,
                glyph_index: *glyph_index,
            })
        }

        fn append_simplified_line(&mut self, line: Self::SimplifiedLine) {
            self.calls.push(Call::Append(line.source));
            self.persistent.push(line);
        }
    }

    #[test]
    fn captures_index_snapshots_clears_then_converts_and_appends_in_order() {
        let mut staff = RecordingStaff::new(&[7, 3, 11]);

        assert_eq!(
            simplify_staff_lines(&mut staff),
            Ok(vec![Filament(7), Filament(3), Filament(11)])
        );
        assert!(staff.originals.is_empty());
        assert_eq!(
            staff.persistent,
            [
                PersistentLine {
                    source: 7,
                    glyph_index: 41,
                },
                PersistentLine {
                    source: 3,
                    glyph_index: 41,
                },
                PersistentLine {
                    source: 11,
                    glyph_index: 41,
                },
            ]
        );
        assert_eq!(
            staff.calls,
            [
                Call::GlyphIndex,
                Call::Snapshot(vec![7, 3, 11]),
                Call::Clear,
                Call::Convert {
                    filament: 7,
                    glyph_index: 41,
                },
                Call::Append(7),
                Call::Convert {
                    filament: 3,
                    glyph_index: 41,
                },
                Call::Append(3),
                Call::Convert {
                    filament: 11,
                    glyph_index: 41,
                },
                Call::Append(11),
            ]
        );
    }

    #[test]
    fn conversion_failure_retains_prefix_and_leaves_remaining_originals_detached() {
        let mut staff = RecordingStaff::new(&[7, 3, 11]);
        staff.fail_on = Some(3);

        assert_eq!(
            simplify_staff_lines(&mut staff),
            Err("filament conversion failed")
        );
        assert!(staff.originals.is_empty());
        assert_eq!(
            staff.persistent,
            [PersistentLine {
                source: 7,
                glyph_index: 41,
            }]
        );
        assert_eq!(
            staff.calls,
            [
                Call::GlyphIndex,
                Call::Snapshot(vec![7, 3, 11]),
                Call::Clear,
                Call::Convert {
                    filament: 7,
                    glyph_index: 41,
                },
                Call::Append(7),
                Call::Convert {
                    filament: 3,
                    glyph_index: 41,
                },
            ]
        );
    }

    #[test]
    fn empty_staff_still_captures_index_and_clears_collection() {
        let mut staff = RecordingStaff::new(&[]);

        assert_eq!(simplify_staff_lines(&mut staff), Ok(Vec::new()));
        assert_eq!(
            staff.calls,
            [Call::GlyphIndex, Call::Snapshot(Vec::new()), Call::Clear]
        );
    }
}
