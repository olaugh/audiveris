// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_cli::parse;

fn usage() {
    println!(
        "Audiveris Rust port (incomplete)\n\nUsage: audiveris-cli [options] [inputs]\n\nUse the Java Audiveris executable for recognition until PORTING.md marks the required stages compatible."
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(parameters) if parameters.help => usage(),
        Ok(parameters) => println!("{parameters:#?}"),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }
}
