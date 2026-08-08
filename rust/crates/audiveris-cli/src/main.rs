// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_cli::{Parameters, parse};
use audiveris_core::step::OmrStep;
use audiveris_image::ingest::Loader;
use audiveris_omr::native_headers::recognize_native_headers;
use audiveris_omr::native_ledgers::recognize_native_ledgers;
use audiveris_omr::native_stem_seeds::recognize_native_stem_seeds;
use audiveris_omr::recognize::{
    grid_lines_report, recognize_grid_lines_raster, recognize_native_beams_with_stem_seeds,
    recognize_scale_raster, scale_report,
};
use audiveris_omr::report::{beams_json, grid_json, headers_json, ledgers_json, stem_seeds_json};

fn usage() {
    println!(
        "Audiveris Rust port (incomplete)\n\n\
         Usage: audiveris-cli [options] [inputs]\n\n\
         Native text recognition currently stops at -step GRID, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID page.png\n\n\
         Schema-1 JSON is published through LEDGERS, e.g.:\n\
         \x20 audiveris-cli -batch -step LEDGERS -json page.png\n\n\
         PNG, JPEG and PDF inputs are accepted. A PDF is a book of sheets and\n\
         every page is processed; -sheets selects a subset, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID score.pdf -sheets 1 3-5\n\n\
         HEADERS, STEM_SEEDS, BEAMS, and LEDGERS currently require -json.\n\
         Small-beam pages are refused explicitly; later stages use the\n\
         compatibility handoff."
    );
}

fn is_native_step(step: OmrStep) -> bool {
    step <= OmrStep::Grid
        || matches!(
            step,
            OmrStep::Headers | OmrStep::StemSeeds | OmrStep::Beams | OmrStep::Ledgers
        )
}

fn is_json_only_step(step: OmrStep) -> bool {
    matches!(
        step,
        OmrStep::Headers | OmrStep::StemSeeds | OmrStep::Beams | OmrStep::Ledgers
    )
}

/// Native batch recognition for the stages the port supports so far.
///
/// Returns `Ok(true)` when the requested step was handled natively; `Ok(false)`
/// hands off to the parameter dump for still-unported requests.
fn run_native(parameters: &Parameters, json: bool) -> Result<bool, String> {
    let Some(step) = parameters.step else {
        return Ok(false);
    };
    if !is_native_step(step) {
        return Ok(false);
    }
    if is_json_only_step(step) && !json {
        return Err(format!(
            "native -step {step} output currently requires -json"
        ));
    }
    if parameters.arguments.is_empty() {
        return Err(format!("-step {step:?} requires at least one input image"));
    }
    for input in &parameters.arguments {
        // An input is a book of sheets, not an image. Only a PDF supplies more
        // than one, and it is opened once here so a multi-page file is parsed
        // once rather than per sheet.
        let loader =
            Loader::open(input).map_err(|error| format!("{}: {error}", input.display()))?;
        let count = loader.image_count();
        for sheet in sheets_to_process(&parameters.sheets, count) {
            let raster = loader
                .image(sheet)
                .map_err(|error| format!("{}: {error}", input.display()))?;
            // Single-sheet inputs keep their original one-line header, so the
            // existing reports and their fixtures are unchanged.
            let header = if count > 1 {
                format!("input={} sheet={sheet}\n", input.display())
            } else {
                format!("input={}\n", input.display())
            };
            let input_name = input.display().to_string();
            let report = if step >= OmrStep::Grid {
                let recognition = recognize_grid_lines_raster(&raster)
                    .map_err(|error| format!("{} sheet {sheet}: {error}", input.display()))?;
                if step == OmrStep::Grid && json {
                    // One JSON document per sheet, one per line: a consensus
                    // front end reading several producers wants a stream it can
                    // consume incrementally, not one array it must buffer.
                    print!("{}", grid_json(&recognition, &input_name, sheet));
                    continue;
                }
                if step == OmrStep::Grid {
                    grid_lines_report(&recognition)
                } else {
                    // Each published stage consumes its real native upstream
                    // products, in Java stage order.
                    let headers = recognize_native_headers(&recognition)
                        .map_err(|error| format!("{} sheet {sheet}: {error}", input.display()))?;
                    if step == OmrStep::Headers {
                        print!(
                            "{}",
                            headers_json(&recognition, &headers, &input_name, sheet)
                        );
                        continue;
                    }
                    let stem_seeds =
                        recognize_native_stem_seeds(&recognition, &headers).map_err(|error| {
                            format!(
                                "{} sheet {sheet}: STEM_SEEDS failed: {error}",
                                input.display()
                            )
                        })?;
                    if step == OmrStep::StemSeeds {
                        print!(
                            "{}",
                            stem_seeds_json(
                                &recognition,
                                &headers,
                                &stem_seeds,
                                &input_name,
                                sheet,
                            )
                        );
                        continue;
                    }
                    let beams = recognize_native_beams_with_stem_seeds(
                        &recognition,
                        headers.beam_erases(),
                        &stem_seeds,
                    )
                    .map_err(|error| {
                        format!("{} sheet {sheet}: BEAMS failed: {error}", input.display())
                    })?;
                    if step == OmrStep::Beams {
                        print!(
                            "{}",
                            beams_json(
                                &recognition,
                                &headers,
                                &stem_seeds,
                                &beams,
                                &input_name,
                                sheet,
                            )
                        );
                        continue;
                    }
                    let ledgers =
                        recognize_native_ledgers(&recognition, &beams).map_err(|error| {
                            format!("{} sheet {sheet}: LEDGERS failed: {error}", input.display())
                        })?;
                    print!(
                        "{}",
                        ledgers_json(
                            &recognition,
                            &headers,
                            &stem_seeds,
                            &beams,
                            &ledgers,
                            &input_name,
                            sheet,
                        )
                    );
                    continue;
                }
            } else {
                let recognition = recognize_scale_raster(&raster)
                    .map_err(|error| format!("{} sheet {sheet}: {error}", input.display()))?;
                scale_report(&recognition)
            };
            print!("{header}{report}");
        }
    }
    Ok(true)
}

/// The sheet ids to process, honouring `-sheets` as Java's `Book` does.
///
/// An empty selection means every sheet. Ids outside the input's range are
/// dropped rather than raised, because `-sheets 1-10` against a three-page
/// book is a normal way to ask for "up to ten".
fn sheets_to_process(selected: &[i32], count: usize) -> Vec<usize> {
    if selected.is_empty() {
        return (1..=count).collect();
    }
    selected
        .iter()
        .filter_map(|id| usize::try_from(*id).ok())
        .filter(|id| (1..=count).contains(id))
        .collect()
}

/// `-json` is a port extension, not one of Audiveris's options.
///
/// It is stripped here rather than added to `Parameters`, because that parser
/// mirrors Java's CLI exactly and is pinned by tests against it. A flag Java
/// does not have does not belong in it.
fn take_json_flag(args: &mut Vec<String>) -> bool {
    let before = args.len();
    args.retain(|argument| argument != "-json");
    args.len() != before
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = take_json_flag(&mut args);
    match parse(&args) {
        Ok(parameters) if parameters.help => usage(),
        Ok(parameters) => match run_native(&parameters, json) {
            Ok(true) => {}
            Ok(false) => println!("{parameters:#?}"),
            Err(error) => {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_native_step, run_native, sheets_to_process};
    use audiveris_cli::Parameters;
    use audiveris_core::step::OmrStep;

    #[test]
    fn no_selection_means_every_sheet() {
        assert_eq!(sheets_to_process(&[], 3), vec![1, 2, 3]);
        assert_eq!(sheets_to_process(&[], 1), vec![1]);
    }

    #[test]
    fn a_selection_keeps_only_the_sheets_the_input_has() {
        assert_eq!(sheets_to_process(&[2, 4], 5), vec![2, 4]);
        // `-sheets 1-10` against a three-sheet book asks for "up to ten", and
        // Java's Book skips the ids it does not have rather than failing.
        assert_eq!(sheets_to_process(&[1, 2, 3, 4, 5], 3), vec![1, 2, 3]);
        assert_eq!(sheets_to_process(&[0, -1, 7], 3), Vec::<usize>::new());
    }

    #[test]
    fn native_stage_routing_skips_the_uncomposed_gap() {
        for step in [
            OmrStep::Load,
            OmrStep::Binary,
            OmrStep::Scale,
            OmrStep::Grid,
            OmrStep::Headers,
            OmrStep::StemSeeds,
            OmrStep::Beams,
            OmrStep::Ledgers,
        ] {
            assert!(is_native_step(step), "{step} should be native");
        }
        assert!(!is_native_step(OmrStep::Heads));
    }

    #[test]
    fn downstream_text_requests_fail_instead_of_dumping_parameters() {
        for step in [OmrStep::Headers, OmrStep::StemSeeds] {
            let parameters = Parameters {
                step: Some(step),
                ..Parameters::default()
            };
            let error =
                run_native(&parameters, false).expect_err("downstream text should fail explicitly");
            assert_eq!(
                error,
                format!("native -step {step} output currently requires -json")
            );
        }

        let parameters = Parameters {
            step: Some(OmrStep::Heads),
            ..Parameters::default()
        };
        assert!(!run_native(&parameters, true).expect("unsupported handoff"));
    }
}
