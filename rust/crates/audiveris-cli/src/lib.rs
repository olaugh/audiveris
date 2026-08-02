// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_core::{natural_spec, step::OmrStep};
use std::{collections::BTreeMap, error::Error, fmt, path::PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameters {
    pub batch: bool,
    pub export: bool,
    pub force: bool,
    pub help: bool,
    pub print: bool,
    pub save: bool,
    pub swap: bool,
    pub transcribe: bool,
    pub upgrade: bool,
    pub version: bool,
    pub sample: bool,
    pub annotate: bool,
    pub output: Option<PathBuf>,
    pub playlist: Option<PathBuf>,
    pub sheets: Vec<i32>,
    pub step: Option<OmrStep>,
    pub run_class: Option<String>,
    pub constants: BTreeMap<String, String>,
    pub arguments: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for CliError {}

fn property(value: &str) -> (String, String) {
    let split = value.find([':', '=']);
    match split {
        Some(index) => (
            value[..index].trim().to_owned(),
            value[index + 1..].trim().to_owned(),
        ),
        None => (value.trim().to_owned(), String::new()),
    }
}

fn value(args: &[String], index: &mut usize, option: &str) -> Result<String, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| CliError(format!("option {option} requires a value")))
}

pub fn parse(args: &[String]) -> Result<Parameters, CliError> {
    let mut params = Parameters::default();
    let mut index = 0;
    let mut options = true;
    while index < args.len() {
        let argument = args[index].trim();
        if options && argument == "--" {
            options = false;
        } else if options && argument.starts_with('-') {
            match argument {
                "-batch" => params.batch = true,
                "-export" => params.export = true,
                "-force" => params.force = true,
                "-help" => params.help = true,
                "-print" => params.print = true,
                "-save" => params.save = true,
                "-swap" => params.swap = true,
                "-transcribe" => params.transcribe = true,
                "-upgrade" => params.upgrade = true,
                "-version" => params.version = true,
                "-sample" => params.sample = true,
                "-annotate" => params.annotate = true,
                "-output" => params.output = Some(value(args, &mut index, argument)?.into()),
                "-playlist" => params.playlist = Some(value(args, &mut index, argument)?.into()),
                "-run" => params.run_class = Some(value(args, &mut index, argument)?),
                "-constant" | "-option" => {
                    let (key, value) = property(&value(args, &mut index, argument)?);
                    params.constants.insert(key, value);
                }
                "-step" => {
                    let raw = value(args, &mut index, argument)?;
                    params.step = Some(raw.parse().map_err(CliError)?);
                }
                "-sheets" => {
                    let mut specs = Vec::new();
                    while index + 1 < args.len() && !args[index + 1].trim().starts_with('-') {
                        index += 1;
                        specs.push(args[index].trim().to_owned());
                    }
                    if specs.is_empty() {
                        return Err(CliError("option -sheets requires a value".to_owned()));
                    }
                    for spec in &specs {
                        params.sheets.extend(
                            natural_spec::decode(Some(spec), false, None)
                                .map_err(|error| CliError(error.to_string()))?,
                        );
                    }
                    params.sheets.sort_unstable();
                    params.sheets.dedup();
                }
                unknown => return Err(CliError(format!("unknown option: {unknown}"))),
            }
        } else {
            params.arguments.push(argument.into());
        }
        index += 1;
    }
    if params.transcribe {
        if params.step.is_some_and(|step| step != OmrStep::Page) {
            return Err(CliError(
                "-transcribe is only compatible with -step PAGE".to_owned(),
            ));
        }
        params.step = Some(OmrStep::Page);
    }
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn ports_java_sheet_cases() {
        assert_eq!(
            parse(&strings(&["-sheets", "3", "4", "6", "11 14"]))
                .unwrap()
                .sheets,
            [3, 4, 6, 11, 14]
        );
        assert_eq!(
            parse(&strings(&["-sheets", "1", "3-6", "10"]))
                .unwrap()
                .sheets,
            [1, 3, 4, 5, 6, 10]
        );
        assert_eq!(
            parse(&strings(&["-sheets", "1", "4 - 6", "20"]))
                .unwrap()
                .sheets,
            [1, 4, 5, 6, 20]
        );
    }

    #[test]
    fn ports_java_combined_case() {
        let params = parse(&strings(&[
            "-help",
            "-batch",
            "-sheets",
            "2 5",
            " 3",
            "-step",
            "PAGE",
            "myScript.xml",
            "my Input.pdf",
        ]))
        .unwrap();
        assert!(params.batch && params.help);
        assert_eq!(params.sheets, [2, 3, 5]);
        assert_eq!(params.step, Some(OmrStep::Page));
        assert_eq!(
            params.arguments,
            [PathBuf::from("myScript.xml"), PathBuf::from("my Input.pdf")]
        );
    }

    #[test]
    fn ports_property_and_error_cases() {
        let params = parse(&strings(&[
            "-constant",
            "omr.toto : totoValue",
            "-option",
            "keyWithNoValue",
        ]))
        .unwrap();
        assert_eq!(params.constants["omr.toto"], "totoValue");
        assert_eq!(params.constants["keyWithNoValue"], "");
        assert!(parse(&strings(&["-step"])).unwrap_err().0.contains("-step"));
    }

    #[test]
    fn stops_option_parsing() {
        let params = parse(&strings(&["--", "-not-an-option.pdf"])).unwrap();
        assert_eq!(params.arguments, [PathBuf::from("-not-an-option.pdf")]);
    }
}
