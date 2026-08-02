// SPDX-License-Identifier: AGPL-3.0-or-later

#[must_use]
pub fn clamp(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[must_use]
pub fn contextual(inter: f64, contribution: f64) -> f64 {
    ((1.0 + contribution) * inter) / (1.0 + contribution * inter)
}

#[must_use]
pub fn contribution_of(partner: f64, ratio: f64) -> f64 {
    partner * (ratio - 1.0)
}

pub fn contextual_from_partners(
    inter: f64,
    partners: &[f64],
    ratios: &[f64],
) -> Result<f64, &'static str> {
    if partners.len() != ratios.len() {
        return Err("arrays of different lengths");
    }
    let contribution = partners
        .iter()
        .zip(ratios)
        .map(|(&partner, &ratio)| contribution_of(partner, ratio))
        .sum();
    Ok(contextual(inter, contribution))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_java_contextual_grade_case() {
        let result = contextual_from_partners(0.2, &[0.5, 0.8], &[5.0, 2.0]).unwrap();
        assert!((result - 0.49).abs() < 0.01);
    }

    #[test]
    fn validates_and_clamps() {
        assert_eq!(clamp(-1.0), 0.0);
        assert_eq!(clamp(2.0), 1.0);
        assert!(contextual_from_partners(0.2, &[0.5], &[1.0, 2.0]).is_err());
    }
}
