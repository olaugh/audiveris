// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NaturalSpecError {
    InvalidInteger(String),
    InvalidRange,
    MissingMaximum,
    NonIncreasing,
}

impl fmt::Display for NaturalSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInteger(value) => write!(f, "invalid integer: {value}"),
            Self::InvalidRange => f.write_str("illegal range provided"),
            Self::MissingMaximum => f.write_str("no maximum value provided"),
            Self::NonIncreasing => f.write_str("non increasing values"),
        }
    }
}

impl Error for NaturalSpecError {}

fn integer(raw: &str) -> Result<i32, NaturalSpecError> {
    raw.trim()
        .parse()
        .map_err(|_| NaturalSpecError::InvalidInteger(raw.trim().to_owned()))
}

pub fn decode(
    spec: Option<&str>,
    check_increasing: bool,
    max_value: Option<i32>,
) -> Result<Vec<i32>, NaturalSpecError> {
    let Some(spec) = spec else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();

    for raw_token in spec.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some(minus_pos) = token.find('-') {
            let first = integer(&token[..minus_pos])?;
            let tail = token[minus_pos + 1..].trim();
            let last = if tail.is_empty() {
                max_value.ok_or(NaturalSpecError::MissingMaximum)?
            } else {
                integer(tail)?
            };
            if last < first {
                return Err(NaturalSpecError::InvalidRange);
            }
            values.extend(first..=last);
        } else {
            for part in token.split_whitespace() {
                values.push(integer(part)?);
            }
        }
    }

    if check_increasing && !is_increasing(&values) {
        return Err(NaturalSpecError::NonIncreasing);
    }
    if let Some(maximum) = max_value {
        values.retain(|value| (0..=maximum).contains(value));
    }
    Ok(values)
}

#[must_use]
pub fn encode(values: &[i32]) -> String {
    let mut output = String::new();
    let mut holding = false;
    let mut previous: Option<i32> = None;
    for &value in values {
        match previous {
            None => output.push_str(&value.to_string()),
            Some(prev) if prev == value - 1 => {
                if !holding {
                    output.push('-');
                    holding = true;
                }
            }
            Some(prev) => {
                if holding {
                    output.push_str(&prev.to_string());
                }
                output.push(',');
                output.push_str(&value.to_string());
                holding = false;
            }
        }
        previous = Some(value);
    }
    if holding {
        output.push_str(
            &previous
                .expect("a held range has a previous value")
                .to_string(),
        );
    }
    output
}

#[must_use]
pub fn is_increasing(values: &[i32]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

pub fn normalized(spec: Option<&str>, max_value: i32) -> Result<String, NaturalSpecError> {
    let normalized = encode(&decode(spec.or(Some("")), true, Some(max_value))?);
    if normalized.is_empty() {
        Ok(if max_value > 1 {
            format!("1-{max_value}")
        } else {
            "1".to_owned()
        })
    } else {
        Ok(normalized)
    }
}

pub fn counts(spec: Option<&str>, max_value: i32) -> Result<String, NaturalSpecError> {
    let selected = match spec {
        Some(value) => decode(Some(value), false, Some(max_value))?.len(),
        None => usize::try_from(max_value).unwrap_or_default(),
    };
    Ok(format!("{selected} of {max_value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_java_decode_cases() {
        assert_eq!(decode(None, true, None).unwrap(), []);
        assert_eq!(decode(Some("1 - 3 , 6"), true, None).unwrap(), [1, 2, 3, 6]);
        assert_eq!(
            decode(Some("3 6 8, 10-12"), true, None).unwrap(),
            [3, 6, 8, 10, 11, 12]
        );
        assert_eq!(decode(Some(" "), true, None).unwrap(), []);
        assert_eq!(decode(Some("2-3,1"), false, None).unwrap(), [2, 3, 1]);
        assert_eq!(decode(Some("3-3"), false, None).unwrap(), [3]);
        assert_eq!(
            decode(Some("2-3,1"), true, None),
            Err(NaturalSpecError::NonIncreasing)
        );
        assert_eq!(
            decode(Some("3-2"), false, None),
            Err(NaturalSpecError::InvalidRange)
        );
    }

    #[test]
    fn ports_java_encode_cases() {
        assert_eq!(encode(&[1, 2, 3, 4, 5]), "1-5");
        assert_eq!(encode(&[5, 2, 4, 6, 7, 8, 10, 12]), "5,2,4,6-8,10,12");
        assert_eq!(encode(&[5, 4, 3, 2]), "5,4,3,2");
        assert_eq!(encode(&[15]), "15");
        assert_eq!(encode(&[]), "");
    }

    #[test]
    fn handles_bounded_and_open_ranges() {
        assert_eq!(
            decode(Some("0,1,3-"), false, Some(4)).unwrap(),
            [0, 1, 3, 4]
        );
        assert_eq!(
            decode(Some("3-"), false, None),
            Err(NaturalSpecError::MissingMaximum)
        );
        assert_eq!(normalized(None, 4).unwrap(), "1-4");
        assert_eq!(counts(Some("1,3-4"), 4).unwrap(), "3 of 4");
    }
}
