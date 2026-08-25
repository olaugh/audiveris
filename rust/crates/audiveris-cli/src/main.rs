// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_cli::{Parameters, parse};
use audiveris_core::step::OmrStep;
use audiveris_image::ingest::Loader;
use audiveris_omr::cue_beams_step::{
    NativeCueBeamsOptions, recognize_native_cue_beams_with_options,
};
use audiveris_omr::native_headers::recognize_native_headers;
use audiveris_omr::native_heads::recognize_native_heads_with_small_heads;
use audiveris_omr::native_ledgers::recognize_native_ledgers;
use audiveris_omr::native_reduction::recognize_native_reduction;
use audiveris_omr::native_stem_seeds::recognize_native_stem_seeds;
use audiveris_omr::native_stems::recognize_native_stems;
use audiveris_omr::recognize::{
    grid_lines_report, recognize_grid_lines_raster, recognize_native_beams_with_stem_seeds,
    recognize_scale_raster, scale_report,
};
use audiveris_omr::report::{
    beams_json, cue_beams_json, grid_json, headers_json, heads_json, ledgers_json, reduction_json,
    stem_seeds_json, stems_json,
};
use std::{
    fmt::Write as _,
    io::Write as _,
    time::{Duration, Instant},
};

fn usage() {
    println!(
        "Audiveris Rust port (incomplete)\n\n\
         Usage: audiveris-cli [options] [inputs]\n\n\
         Native text recognition currently stops at -step GRID, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID page.png\n\n\
         Schema-1 JSON is published through CUE_BEAMS, e.g.:\n\
         \x20 audiveris-cli -batch -step CUE_BEAMS -json page.png\n\
         Active cue recognition requires Java's small-head switch:\n\
         \x20 -constant org.audiveris.omr.sheet.ProcessingSwitches.smallHeads=true\n\
         Ordinary cue recognition can be disabled independently with:\n\
         \x20 -constant org.audiveris.omr.sheet.beam.CueBeamsStep.enabled=false\n\
         Supplemental hook recovery is independently enabled with:\n\
         \x20 -constant org.audiveris.omr.sheet.beam.CueBeamsStep.supplementalHookRecovery=true\n\n\
         PNG, JPEG and PDF inputs are accepted. A PDF is a book of sheets and\n\
         every page is processed; -sheets selects a subset, e.g.:\n\
         \x20 audiveris-cli -batch -step GRID score.pdf -sheets 1 3-5\n\n\
         HEADERS through CUE_BEAMS currently require -json.\n\
         Small-beam pages are refused explicitly; later stages use the\n\
         compatibility handoff."
    );
}

fn is_native_step(step: OmrStep) -> bool {
    step <= OmrStep::Grid
        || matches!(
            step,
            OmrStep::Headers
                | OmrStep::StemSeeds
                | OmrStep::Beams
                | OmrStep::Ledgers
                | OmrStep::Heads
                | OmrStep::Stems
                | OmrStep::Reduction
                | OmrStep::CueBeams
        )
}

fn is_json_only_step(step: OmrStep) -> bool {
    matches!(
        step,
        OmrStep::Headers
            | OmrStep::StemSeeds
            | OmrStep::Beams
            | OmrStep::Ledgers
            | OmrStep::Heads
            | OmrStep::Stems
            | OmrStep::Reduction
            | OmrStep::CueBeams
    )
}

const SMALL_HEADS_CONSTANT: &str = "org.audiveris.omr.sheet.ProcessingSwitches.smallHeads";
const CUE_BEAMS_ENABLED_CONSTANT: &str = "org.audiveris.omr.sheet.beam.CueBeamsStep.enabled";
const CUE_BEAMS_RECOVERY_CONSTANT: &str =
    "org.audiveris.omr.sheet.beam.CueBeamsStep.supplementalHookRecovery";

fn small_heads_enabled(parameters: &Parameters) -> bool {
    parameters
        .constants
        .get(SMALL_HEADS_CONSTANT)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn cue_beams_options(parameters: &Parameters) -> NativeCueBeamsOptions {
    NativeCueBeamsOptions {
        enabled: parameters
            .constants
            .get(CUE_BEAMS_ENABLED_CONSTANT)
            .is_none_or(|value| !value.eq_ignore_ascii_case("false")),
        supplemental_hook_recovery: parameters
            .constants
            .get(CUE_BEAMS_RECOVERY_CONSTANT)
            .is_some_and(|value| value.eq_ignore_ascii_case("true")),
    }
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
                    if step == OmrStep::Ledgers {
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
                    let heads = recognize_native_heads_with_small_heads(
                        &recognition,
                        &headers,
                        &stem_seeds,
                        &beams,
                        &ledgers,
                        small_heads_enabled(parameters),
                    )
                    .map_err(|error| {
                        format!("{} sheet {sheet}: HEADS failed: {error}", input.display())
                    })?;
                    if step == OmrStep::Heads {
                        print!(
                            "{}",
                            heads_json(
                                &recognition,
                                &headers,
                                &stem_seeds,
                                &beams,
                                &ledgers,
                                &heads.epilog,
                                &input_name,
                                sheet,
                            )
                        );
                        continue;
                    }
                    let inspect_profile = stem_seeds
                        .systems
                        .first()
                        .map_or(1, |system| system.raw.profile);
                    let stems = recognize_native_stems(
                        &recognition,
                        &headers,
                        &stem_seeds,
                        &beams,
                        &ledgers,
                        &heads,
                        inspect_profile,
                    )
                    .map_err(|error| {
                        format!("{} sheet {sheet}: STEMS failed: {error}", input.display())
                    })?;
                    if step == OmrStep::Stems {
                        print!(
                            "{}",
                            stems_json(
                                &recognition,
                                &headers,
                                &stem_seeds,
                                &beams,
                                &ledgers,
                                &heads.epilog,
                                &stems,
                                &input_name,
                                sheet,
                            )
                        );
                        continue;
                    }
                    let reduction =
                        recognize_native_reduction(&recognition, stems).map_err(|error| {
                            format!(
                                "{} sheet {sheet}: REDUCTION failed: {error}",
                                input.display()
                            )
                        })?;
                    if step == OmrStep::Reduction {
                        print!(
                            "{}",
                            reduction_json(
                                &recognition,
                                &headers,
                                &stem_seeds,
                                &beams,
                                &ledgers,
                                &heads.epilog,
                                &reduction,
                                &input_name,
                                sheet,
                            )
                        );
                        continue;
                    }
                    let cue_beams = recognize_native_cue_beams_with_options(
                        &recognition,
                        reduction,
                        small_heads_enabled(parameters),
                        cue_beams_options(parameters),
                    )
                    .map_err(|error| {
                        format!(
                            "{} sheet {sheet}: CUE_BEAMS failed: {error}",
                            input.display()
                        )
                    })?;
                    print!(
                        "{}",
                        cue_beams_json(
                            &recognition,
                            &headers,
                            &stem_seeds,
                            &beams,
                            &ledgers,
                            &heads.epilog,
                            &cue_beams,
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

/// The additive, line-oriented control channel used by `omrscope`.
///
/// The schema-1 recognition documents deliberately remain their own lines
/// between `stage_started` and `stage_completed` markers. Keeping the payload
/// byte-for-byte identical to ordinary `-json` output means existing consumers
/// can keep using their normal parser, while a live viewer can use the markers
/// for ordering and timing. Stdout is flushed after every line because it is a
/// pipe to a GUI, not an interactive terminal.
struct OmrscopeStream {
    output: std::io::Stdout,
    sequence: u64,
}

impl OmrscopeStream {
    fn new() -> Self {
        Self {
            output: std::io::stdout(),
            sequence: 0,
        }
    }

    fn marker(&mut self, event: &str, fields: &str) -> Result<(), String> {
        self.sequence = self.sequence.saturating_add(1);
        let line = omrscope_marker_line("rust", event, self.sequence, fields);
        self.output
            .write_all(line.as_bytes())
            .and_then(|()| self.output.flush())
            .map_err(|error| format!("cannot write omrscope stream marker: {error}"))
    }

    fn run_started(&mut self, target: OmrStep) -> Result<(), String> {
        self.marker(
            "run_started",
            &format!("\"target\":{}", json_string(&target.to_string())),
        )
    }

    fn stage_started(&mut self, stage: OmrStep, input: &str, sheet: usize) -> Result<(), String> {
        self.marker(
            "stage_started",
            &format!(
                "\"stage\":{},\"input\":{},\"sheet\":{sheet}",
                json_string(&stage.to_string()),
                json_string(input),
            ),
        )
    }

    fn snapshot(&mut self, payload: &str) -> Result<(), String> {
        self.output
            .write_all(payload.as_bytes())
            .and_then(|()| self.output.flush())
            .map_err(|error| format!("cannot write omrscope stage payload: {error}"))
    }

    fn stage_completed(
        &mut self,
        stage: OmrStep,
        input: &str,
        sheet: usize,
        elapsed: Duration,
    ) -> Result<(), String> {
        self.marker(
            "stage_completed",
            &format!(
                "\"stage\":{},\"input\":{},\"sheet\":{sheet},\"elapsed_ms\":{}",
                json_string(&stage.to_string()),
                json_string(input),
                elapsed_millis(elapsed),
            ),
        )
    }

    fn stage_failed(
        &mut self,
        stage: OmrStep,
        input: &str,
        sheet: usize,
        elapsed: Duration,
        message: &str,
    ) -> Result<(), String> {
        self.marker(
            "stage_failed",
            &format!(
                "\"stage\":{},\"input\":{},\"sheet\":{sheet},\"elapsed_ms\":{},\"message\":{}",
                json_string(&stage.to_string()),
                json_string(input),
                elapsed_millis(elapsed),
                json_string(message),
            ),
        )
    }

    fn run_finished(&mut self, success: bool, elapsed: Duration) -> Result<(), String> {
        self.marker(
            "run_finished",
            &format!(
                "\"success\":{success},\"elapsed_ms\":{}",
                elapsed_millis(elapsed),
            ),
        )
    }
}

fn omrscope_marker_line(engine: &str, event: &str, sequence: u64, fields: &str) -> String {
    let suffix = if fields.is_empty() {
        String::new()
    } else {
        format!(",{fields}")
    };
    format!(
        "@omrscope {{\"stream_schema\":1,\"engine\":{},\"event\":{},\"sequence\":{sequence}{suffix}}}\n",
        json_string(engine),
        json_string(event),
    )
}

fn elapsed_millis(elapsed: Duration) -> String {
    // A duration is diagnostic rather than a parity value, but decimal rather
    // than Debug formatting keeps the line valid JSON on every platform.
    format!("{:.3}", elapsed.as_secs_f64() * 1_000.0)
}

fn json_string(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                let _ = write!(quoted, "\\u{:04x}", control as u32);
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn stream_stages_through(target: OmrStep) -> Result<&'static [OmrStep], String> {
    const STAGES: [OmrStep; 9] = [
        OmrStep::Grid,
        OmrStep::Headers,
        OmrStep::StemSeeds,
        OmrStep::Beams,
        OmrStep::Ledgers,
        OmrStep::Heads,
        OmrStep::Stems,
        OmrStep::Reduction,
        OmrStep::CueBeams,
    ];
    let Some(index) = STAGES.iter().position(|stage| *stage == target) else {
        return Err(format!(
            "native omrscope stream begins at -step GRID and currently ends at -step CUE_BEAMS, not -step {target}"
        ));
    };
    Ok(&STAGES[..=index])
}

/// Streams every currently published native stage through `parameters.step`.
///
/// This intentionally has a separate implementation from [`run_native`]. The
/// ordinary path is an established public text/JSON interface, so refactoring
/// it just to add marker lines would create an unnecessary compatibility risk.
fn run_native_stream(parameters: &Parameters, json: bool) -> Result<bool, String> {
    let Some(target) = parameters.step else {
        return Ok(false);
    };
    if !is_native_step(target) {
        return Ok(false);
    }
    if !json {
        return Err("native omrscope stream output requires -json".to_owned());
    }
    let _stages = stream_stages_through(target)?;
    if parameters.arguments.is_empty() {
        return Err(format!(
            "-step {target:?} requires at least one input image"
        ));
    }

    let mut stream = OmrscopeStream::new();
    let run_started = Instant::now();
    stream.run_started(target)?;

    let result = (|| {
        let mut processed_sheets = 0_usize;
        for input in &parameters.arguments {
            // As in the legacy path, open a book only once and then process its
            // selected sheets in source order.
            let loader =
                Loader::open(input).map_err(|error| format!("{}: {error}", input.display()))?;
            let count = loader.image_count();
            for sheet in sheets_to_process(&parameters.sheets, count) {
                processed_sheets = processed_sheets.saturating_add(1);
                let raster = loader
                    .image(sheet)
                    .map_err(|error| format!("{}: {error}", input.display()))?;
                let input_name = input.display().to_string();

                // GRID -------------------------------------------------------
                stream.stage_started(OmrStep::Grid, &input_name, sheet)?;
                let started = Instant::now();
                let recognition = match recognize_grid_lines_raster(&raster) {
                    Ok(recognition) => recognition,
                    Err(error) => {
                        let message = format!("{} sheet {sheet}: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Grid,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&grid_json(&recognition, &input_name, sheet))?;
                stream.stage_completed(OmrStep::Grid, &input_name, sheet, recognition_elapsed)?;
                if target == OmrStep::Grid {
                    continue;
                }

                // HEADERS ----------------------------------------------------
                stream.stage_started(OmrStep::Headers, &input_name, sheet)?;
                let started = Instant::now();
                let headers = match recognize_native_headers(&recognition) {
                    Ok(headers) => headers,
                    Err(error) => {
                        let message = format!("{} sheet {sheet}: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Headers,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&headers_json(&recognition, &headers, &input_name, sheet))?;
                stream.stage_completed(
                    OmrStep::Headers,
                    &input_name,
                    sheet,
                    recognition_elapsed,
                )?;
                if target == OmrStep::Headers {
                    continue;
                }

                // STEM_SEEDS -------------------------------------------------
                stream.stage_started(OmrStep::StemSeeds, &input_name, sheet)?;
                let started = Instant::now();
                let stem_seeds = match recognize_native_stem_seeds(&recognition, &headers) {
                    Ok(stem_seeds) => stem_seeds,
                    Err(error) => {
                        let message = format!(
                            "{} sheet {sheet}: STEM_SEEDS failed: {error}",
                            input.display()
                        );
                        stream.stage_failed(
                            OmrStep::StemSeeds,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&stem_seeds_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(
                    OmrStep::StemSeeds,
                    &input_name,
                    sheet,
                    recognition_elapsed,
                )?;
                if target == OmrStep::StemSeeds {
                    continue;
                }

                // BEAMS ------------------------------------------------------
                stream.stage_started(OmrStep::Beams, &input_name, sheet)?;
                let started = Instant::now();
                let beams = match recognize_native_beams_with_stem_seeds(
                    &recognition,
                    headers.beam_erases(),
                    &stem_seeds,
                ) {
                    Ok(beams) => beams,
                    Err(error) => {
                        let message =
                            format!("{} sheet {sheet}: BEAMS failed: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Beams,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&beams_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(OmrStep::Beams, &input_name, sheet, recognition_elapsed)?;
                if target == OmrStep::Beams {
                    continue;
                }

                // LEDGERS ----------------------------------------------------
                stream.stage_started(OmrStep::Ledgers, &input_name, sheet)?;
                let started = Instant::now();
                let ledgers = match recognize_native_ledgers(&recognition, &beams) {
                    Ok(ledgers) => ledgers,
                    Err(error) => {
                        let message =
                            format!("{} sheet {sheet}: LEDGERS failed: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Ledgers,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&ledgers_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(
                    OmrStep::Ledgers,
                    &input_name,
                    sheet,
                    recognition_elapsed,
                )?;
                if target == OmrStep::Ledgers {
                    continue;
                }

                // HEADS ------------------------------------------------------
                stream.stage_started(OmrStep::Heads, &input_name, sheet)?;
                let started = Instant::now();
                let heads = match recognize_native_heads_with_small_heads(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    small_heads_enabled(parameters),
                ) {
                    Ok(heads) => heads,
                    Err(error) => {
                        let message =
                            format!("{} sheet {sheet}: HEADS failed: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Heads,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&heads_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &heads.epilog,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(OmrStep::Heads, &input_name, sheet, recognition_elapsed)?;
                if target == OmrStep::Heads {
                    continue;
                }

                // STEMS ------------------------------------------------------
                stream.stage_started(OmrStep::Stems, &input_name, sheet)?;
                let started = Instant::now();
                let inspect_profile = stem_seeds
                    .systems
                    .first()
                    .map_or(1, |system| system.raw.profile);
                let stems = match recognize_native_stems(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &heads,
                    inspect_profile,
                ) {
                    Ok(stems) => stems,
                    Err(error) => {
                        let message =
                            format!("{} sheet {sheet}: STEMS failed: {error}", input.display());
                        stream.stage_failed(
                            OmrStep::Stems,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&stems_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &heads.epilog,
                    &stems,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(OmrStep::Stems, &input_name, sheet, recognition_elapsed)?;
                if target == OmrStep::Stems {
                    continue;
                }

                // REDUCTION -------------------------------------------------
                stream.stage_started(OmrStep::Reduction, &input_name, sheet)?;
                let started = Instant::now();
                let reduction = match recognize_native_reduction(&recognition, stems) {
                    Ok(reduction) => reduction,
                    Err(error) => {
                        let message = format!(
                            "{} sheet {sheet}: REDUCTION failed: {error}",
                            input.display()
                        );
                        stream.stage_failed(
                            OmrStep::Reduction,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&reduction_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &heads.epilog,
                    &reduction,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(
                    OmrStep::Reduction,
                    &input_name,
                    sheet,
                    recognition_elapsed,
                )?;
                if target == OmrStep::Reduction {
                    continue;
                }

                // CUE_BEAMS -------------------------------------------------
                stream.stage_started(OmrStep::CueBeams, &input_name, sheet)?;
                let started = Instant::now();
                let cue_beams = match recognize_native_cue_beams_with_options(
                    &recognition,
                    reduction,
                    small_heads_enabled(parameters),
                    cue_beams_options(parameters),
                ) {
                    Ok(cue_beams) => cue_beams,
                    Err(error) => {
                        let message = format!(
                            "{} sheet {sheet}: CUE_BEAMS failed: {error}",
                            input.display()
                        );
                        stream.stage_failed(
                            OmrStep::CueBeams,
                            &input_name,
                            sheet,
                            started.elapsed(),
                            &message,
                        )?;
                        return Err(message);
                    }
                };
                let recognition_elapsed = started.elapsed();
                stream.snapshot(&cue_beams_json(
                    &recognition,
                    &headers,
                    &stem_seeds,
                    &beams,
                    &ledgers,
                    &heads.epilog,
                    &cue_beams,
                    &input_name,
                    sheet,
                ))?;
                stream.stage_completed(
                    OmrStep::CueBeams,
                    &input_name,
                    sheet,
                    recognition_elapsed,
                )?;
            }
        }
        if processed_sheets == 0 {
            return Err("native omrscope stream selected no sheets".to_owned());
        }
        Ok(true)
    })();

    stream.run_finished(result.is_ok(), run_started.elapsed())?;
    result
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

/// `-stream-json` is likewise a port extension, kept outside the Java-mirroring
/// parser. It changes stdout framing only when explicitly present.
fn take_stream_json_flag(args: &mut Vec<String>) -> bool {
    let before = args.len();
    args.retain(|argument| argument != "-stream-json");
    args.len() != before
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = take_json_flag(&mut args);
    let stream_json = take_stream_json_flag(&mut args);
    match parse(&args) {
        Ok(parameters) if parameters.help => usage(),
        Ok(parameters) => match if stream_json {
            run_native_stream(&parameters, json)
        } else {
            run_native(&parameters, json)
        } {
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
    use super::{
        CUE_BEAMS_ENABLED_CONSTANT, CUE_BEAMS_RECOVERY_CONSTANT, SMALL_HEADS_CONSTANT,
        cue_beams_options, is_native_step, json_string, omrscope_marker_line, run_native,
        run_native_stream, sheets_to_process, small_heads_enabled, stream_stages_through,
        take_stream_json_flag,
    };
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
            OmrStep::Heads,
            OmrStep::Stems,
            OmrStep::Reduction,
            OmrStep::CueBeams,
        ] {
            assert!(is_native_step(step), "{step} should be native");
        }
        assert!(!is_native_step(OmrStep::Symbols));
    }

    #[test]
    fn small_heads_constant_uses_java_boolean_value_of_semantics() {
        let mut parameters = Parameters::default();
        assert!(!small_heads_enabled(&parameters));
        parameters
            .constants
            .insert(SMALL_HEADS_CONSTANT.to_owned(), "TRUE".to_owned());
        assert!(small_heads_enabled(&parameters));
        parameters
            .constants
            .insert(SMALL_HEADS_CONSTANT.to_owned(), "yes".to_owned());
        assert!(!small_heads_enabled(&parameters));
    }

    #[test]
    fn cue_beams_controls_are_independent_and_have_production_defaults() {
        let mut parameters = Parameters::default();
        assert!(cue_beams_options(&parameters).enabled);
        assert!(!cue_beams_options(&parameters).supplemental_hook_recovery);

        parameters
            .constants
            .insert(CUE_BEAMS_ENABLED_CONSTANT.to_owned(), "false".to_owned());
        parameters
            .constants
            .insert(CUE_BEAMS_RECOVERY_CONSTANT.to_owned(), "TRUE".to_owned());
        assert!(!cue_beams_options(&parameters).enabled);
        assert!(cue_beams_options(&parameters).supplemental_hook_recovery);
    }

    #[test]
    fn downstream_text_requests_fail_instead_of_dumping_parameters() {
        for step in [
            OmrStep::Headers,
            OmrStep::StemSeeds,
            OmrStep::Beams,
            OmrStep::Ledgers,
            OmrStep::Heads,
            OmrStep::Stems,
            OmrStep::Reduction,
            OmrStep::CueBeams,
        ] {
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
    }

    #[test]
    fn stream_flag_is_additive_and_never_reaches_the_java_compatible_parser() {
        let legacy = vec![
            "-batch".to_owned(),
            "-step".to_owned(),
            "GRID".to_owned(),
            "chula.png".to_owned(),
        ];
        let mut unchanged = legacy.clone();
        assert!(!take_stream_json_flag(&mut unchanged));
        assert_eq!(unchanged, legacy);

        let mut extended = legacy.clone();
        extended.insert(3, "-stream-json".to_owned());
        assert!(take_stream_json_flag(&mut extended));
        assert_eq!(extended, legacy);
    }

    #[test]
    fn stream_markers_escape_json_and_carry_a_monotonic_sequence_field() {
        assert_eq!(
            json_string("path\nwith \"quotes\""),
            "\"path\\nwith \\\"quotes\\\"\""
        );
        assert_eq!(
            omrscope_marker_line("rust", "stage_started", 17, r#""stage":"GRID""#),
            "@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"stage_started\",\"sequence\":17,\"stage\":\"GRID\"}\n"
        );
    }

    #[test]
    fn stream_stage_plan_is_java_pipeline_order_and_refuses_unpublished_stages() {
        assert_eq!(
            stream_stages_through(OmrStep::CueBeams).expect("CUE_BEAMS stream plan"),
            [
                OmrStep::Grid,
                OmrStep::Headers,
                OmrStep::StemSeeds,
                OmrStep::Beams,
                OmrStep::Ledgers,
                OmrStep::Heads,
                OmrStep::Stems,
                OmrStep::Reduction,
                OmrStep::CueBeams,
            ]
        );
        assert!(
            stream_stages_through(OmrStep::Scale)
                .expect_err("SCALE has no schema-1 stage snapshot")
                .contains("GRID")
        );

        let no_json = Parameters {
            step: Some(OmrStep::Grid),
            ..Parameters::default()
        };
        assert_eq!(
            run_native_stream(&no_json, false).expect_err("stream needs JSON"),
            "native omrscope stream output requires -json"
        );

        let no_input = Parameters {
            step: Some(OmrStep::Heads),
            ..Parameters::default()
        };
        assert!(
            run_native_stream(&no_input, true)
                .expect_err("stream needs an input")
                .contains("requires at least one input image")
        );
    }
}
