// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_cli::{Parameters, parse};
use audiveris_core::step::OmrStep;
use audiveris_omr::recognize::{
    grid_lines_report, recognize_grid_lines, recognize_scale, scale_report,
};

fn usage() {
    println!(
        "Audiveris Rust port (incomplete)\n\n\
         Usage: audiveris-cli [options] [inputs]\n\n\
         Native recognition currently stops at -step SCALE, e.g.:\n\
         \x20 audiveris-cli -batch -step SCALE page.png\n\n\
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
        if step == OmrStep::Grid {
            let recognition = recognize_grid_lines(input)
                .map_err(|error| format!("{}: {error}", input.display()))?;
            print!(
                "input={}\n{}",
                input.display(),
                grid_lines_report(&recognition)
            );
        } else {
            let recognition =
                recognize_scale(input).map_err(|error| format!("{}: {error}", input.display()))?;
            print!("input={}\n{}", input.display(), scale_report(&recognition));
        }
    }
    Ok(true)
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
