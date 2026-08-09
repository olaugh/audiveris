// SPDX-License-Identifier: AGPL-3.0-or-later

mod support;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = audiveris_mei::serialize_mei(&support::phase_one_document())?;
    print!("{}", String::from_utf8(output.into_bytes())?);
    Ok(())
}
