// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_cli::{Parameters, parse};
use audiveris_core::step::OmrStep;
use audiveris_image::ingest::Loader;
use audiveris_omr::recognize::{
    grid_lines_report, recognize_grid_lines_raster, recognize_scale_raster, scale_report,
};

fn usage() {
    println!(
        "Audiveris Rust port (incomplete)\n\n\
         Usage: audiveris-cli [options] [inputs]\n\n\
         Native recognition currently stops at -step GRID, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID page.png\n\n\
         PNG, JPEG and PDF inputs are accepted. A PDF is a book of sheets and\n\
         every page is processed; -sheets selects a subset, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID score.pdf -sheets 1 3-5\n\n\
         Use the Java Audiveris executable for later stages until PORTING.md\n\
         marks them compatible."
    );
}

/// Native batch recognition for the stages the port supports so far.
///
/// Returns `Ok(true)` when the requested step was handled natively; `Ok(false)`
/// hands off to the parameter dump for still-unported requests.
fn run_native(parameters: &Parameters) -> Result<bool, String> {
    let Some(step) = parameters.step else {
        return Ok(false);
    };
    if step > OmrStep::Grid {
        return Ok(false);
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
            let report = if step == OmrStep::Grid {
                let recognition = recognize_grid_lines_raster(&raster)
                    .map_err(|error| format!("{} sheet {sheet}: {error}", input.display()))?;
                grid_lines_report(&recognition)
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(parameters) if parameters.help => usage(),
        Ok(parameters) => match run_native(&parameters) {
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
    use super::sheets_to_process;

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
}
