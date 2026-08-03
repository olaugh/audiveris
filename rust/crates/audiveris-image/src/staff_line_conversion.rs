// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete headless `StaffFilament.toStaffLine` conversion.

use audiveris_core::natural_spline::{NaturalSpline, SplineError};

use crate::{
    filament::{FilamentError, StaffFilament},
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

/// Pixel glyph built from the union of a staff filament's sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaffGlyph {
    pub x: usize,
    pub y: usize,
    pub runs: RunTable,
}

/// Persistent, simplified staff-line data produced by GRID cleanup.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistentStaffLine {
    pub points: Vec<(f64, f64)>,
    pub thickness: f64,
    pub glyph: StaffGlyph,
}

/// Production adapter for `GlyphIndex.registerOriginal`.
pub trait OriginalGlyphRegistrar {
    type Error;

    fn register_original(&mut self, glyph: StaffGlyph) -> Result<StaffGlyph, Self::Error>;
}

#[derive(Debug, PartialEq)]
pub enum StaffLineConversionError<RegistrationError> {
    Filament(FilamentError),
    RunTable(RunTableError),
    Spline(SplineError),
    Registration(RegistrationError),
}

impl<RegistrationError> From<FilamentError> for StaffLineConversionError<RegistrationError> {
    fn from(error: FilamentError) -> Self {
        Self::Filament(error)
    }
}

impl<RegistrationError> From<RunTableError> for StaffLineConversionError<RegistrationError> {
    fn from(error: RunTableError) -> Self {
        Self::RunTable(error)
    }
}

impl<RegistrationError> From<SplineError> for StaffLineConversionError<RegistrationError> {
    fn from(error: SplineError) -> Self {
        Self::Spline(error)
    }
}

/// Convert and register one filament in Java source order: build compound
/// glyph, register it, construct `StaffLine` (+0.5 y), simplify, assign glyph.
pub fn to_registered_staff_line<Registrar: OriginalGlyphRegistrar>(
    filament: &StaffFilament,
    registrar: &mut Registrar,
) -> Result<PersistentStaffLine, StaffLineConversionError<Registrar::Error>> {
    let glyph = build_staff_glyph(filament)?;
    let glyph = registrar
        .register_original(glyph)
        .map_err(StaffLineConversionError::Registration)?;
    finish_staff_line(filament, glyph)
}

fn build_staff_glyph<RegistrationError>(
    filament: &StaffFilament,
) -> Result<StaffGlyph, StaffLineConversionError<RegistrationError>> {
    let bounds = filament.bounds()?;
    let mut pixels = vec![BACKGROUND; bounds.width * bounds.height];
    for section in filament.sections() {
        for (offset, run) in section.runs().iter().enumerate() {
            let y = section.first_pos() + offset;
            for x in run.start..=run.stop() {
                pixels[(y - bounds.y) * bounds.width + (x - bounds.x)] = FOREGROUND;
            }
        }
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    Ok(StaffGlyph {
        x: bounds.x,
        y: bounds.y,
        runs: RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?,
    })
}

fn finish_staff_line<RegistrationError>(
    filament: &StaffFilament,
    glyph: StaffGlyph,
) -> Result<PersistentStaffLine, StaffLineConversionError<RegistrationError>> {
    let geometry = filament.geometry()?;
    let mut points = geometry
        .points()
        .iter()
        .map(|&(x, y)| (x, y + 0.5))
        .collect::<Vec<_>>();
    let segment_length = ((filament.interline() as f64) * 4.0).round_ties_even() as usize;
    simplify_points(&mut points, 1.0, segment_length)?;
    Ok(PersistentStaffLine {
        points,
        thickness: filament.thickness()?,
        glyph,
    })
}

fn simplify_points(
    points: &mut Vec<(f64, f64)>,
    max_dy: f64,
    segment_length: usize,
) -> Result<(), SplineError> {
    let left = points[0];
    let right = points[points.len() - 1];
    let width = right.0 - left.0;
    let original_spline = NaturalSpline::interpolate(points)?;
    let mut definitions = vec![left, right];

    loop {
        let segment = width / (definitions.len() - 1) as f64;
        if segment < segment_length as f64 {
            return Ok(());
        }
        let candidate = NaturalSpline::interpolate(&definitions)?;
        let acceptable = points.iter().all(|&(x, y)| {
            candidate
                .y_at_x(x)
                .is_ok_and(|candidate_y| (candidate_y - y).abs() <= max_dy)
        });
        if acceptable {
            *points = definitions;
            return Ok(());
        }
        let mut index = 0;
        while index < definitions.len() - 1 {
            let left = definitions[index];
            let right = definitions[index + 1];
            let x = 0.5 * (left.0 + right.0);
            let y = original_spline.y_at_x(x)?;
            definitions.insert(index + 1, (x, y));
            index += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run_table::Run,
        section::{JunctionPolicy, build_sections},
    };

    #[derive(Default)]
    struct Registrar {
        calls: usize,
        fail: bool,
    }

    impl OriginalGlyphRegistrar for Registrar {
        type Error = &'static str;

        fn register_original(&mut self, mut glyph: StaffGlyph) -> Result<StaffGlyph, Self::Error> {
            self.calls += 1;
            if self.fail {
                return Err("registration failed");
            }
            glyph.x += 100;
            Ok(glyph)
        }
    }

    fn filament() -> StaffFilament {
        let mut filament = StaffFilament::new(10).unwrap();
        let mut table = RunTable::new(Orientation::Horizontal, 100, 8).unwrap();
        assert!(table.add_run(3, Run::new(10, 81)).unwrap());
        filament
            .add_section(
                build_sections(&table, JunctionPolicy::DEFAULT_RATIO)
                    .into_iter()
                    .next()
                    .unwrap(),
            )
            .unwrap();
        filament
    }

    #[test]
    fn builds_registers_offsets_simplifies_and_assigns_glyph() {
        let mut registrar = Registrar::default();
        let line = to_registered_staff_line(&filament(), &mut registrar).unwrap();
        assert_eq!(registrar.calls, 1);
        assert_eq!(line.glyph.x, 110);
        assert_eq!(line.glyph.y, 3);
        assert_eq!(line.glyph.runs.orientation(), Orientation::Horizontal);
        assert_eq!(line.glyph.runs.total_run_count(), 1);
        assert_eq!(line.points, [(10.0, 3.5), (90.0, 3.5)]);
        assert_eq!(line.thickness, 1.0);
    }

    #[test]
    fn registration_failure_happens_before_staff_line_geometry() {
        let mut registrar = Registrar {
            fail: true,
            ..Registrar::default()
        };
        assert_eq!(
            to_registered_staff_line(&filament(), &mut registrar),
            Err(StaffLineConversionError::Registration(
                "registration failed"
            ))
        );
        assert_eq!(registrar.calls, 1);
    }

    #[test]
    fn simplifier_keeps_originals_once_subdivision_is_shorter_than_source_segments() {
        let mut points = vec![(0.0, 0.0), (4.0, 5.0), (8.0, 0.0)];
        simplify_points(&mut points, 1.0, 5).unwrap();
        assert_eq!(points, [(0.0, 0.0), (4.0, 5.0), (8.0, 0.0)]);
    }
}
