# Barline tuning layer: Python port status

The paired-staff barline hypothesis layer prototyped in
`stage-omr-data/scripts/tune_sonata_barlines.py` is being ported into this
workspace as an enhancement beyond Java parity.  The Python script is the
oracle for this layer (the Java executable remains the oracle for
parity-port code only).

## What is landed

| Piece | Where | Status |
|---|---|---|
| Engine: projection metrics, certain-connector predicate, flank noise | `audiveris-image/src/bar_tuning.rs` | done; 11 unit tests port the full Python suite |
| Engine: geometry count (edge margin, corroboration, height-scaled gap), DP boundary selection with honest-short fallback | same module | done |
| Parameters | `BarTuningParameters::python_reference()` (frozen pixel constants) and `from_scale(interline)` (identical at the Schenker calibration interline of 6 px) | done |
| Python-oracle parity | `tests/barline_tuning_parity.rs` + `oracle/py-barline-tuning/{schenker-sonata01,graceful-ghost-rag}.txt` | **bit-exact**: 104 + 20 systems, worst float delta 0.0 |
| Interline-mode decisions | same test, `from_scale(12)` on the warped GGR corpus | all 20 systems match the oracle's boundaries (volta-aware; the residual list is empty) |
| Engraver ground truth | `tests/barline_tuning_synthetic.rs` + `oracle/py-barline-tuning/verovio-synthetic.txt` | 15 synthetic systems at interlines 6/12/18; projection alone recovers every barline, fabricates none |
| Barline-form classification (types layer, first slice) | `classify_boundary` in `bar_tuning.rs`, `tests/barline_classification.rs` | 60/60 engraved forms + 4 hand-verified scan boundaries |

Fixture images live under the gitignored `stage-omr-data/data/` tree;
the tests take the workspace root from `AUDIVERIS_BARLINE_TUNING_FIXTURES`
and skip loudly when it is unset.  Regeneration recipes are in the fixture
file headers (`export_barline_tuning_fixtures.py`,
`generate_verovio_barline_corpus.py`).

## Design decisions worth knowing

- **Two parameter modes.** `python_reference()` exists for parity;
  `from_scale(interline)` is the forward path.  The Graceful Ghost Rag
  evaluation showed why: absolute pixel thresholds calibrated on 6 px
  interline scans mis-handle 12 px scans (warp tilt opens 8-11 px gaps on
  genuine bars; a 7 px thick+thin section double bar escapes a 6 px merge).
- **The oracle rounds candidate metrics to four decimals at construction**,
  so certainty/corroboration/evidence all see rounded values; the engine
  rounds at the same point (found by the parity harness, which is exactly
  what it is for).
- **Volta-backed counting** replaced the one pinned interline residual:
  GGR p2s5's first-ending barline is projection-only with gap 15 (the
  bracket junction interrupts it), and the pixel oracle recovered it only
  because a double-counted repeat bar inflated the count.  With
  `geometry_count_with_voltas`, its detected bracket counts the
  interrupted, stem-clean candidate on principle, and
  `KNOWN_INTERLINE_RESIDUALS` is empty.
- **DP determinism**: the selection DP replicates Python dict semantics
  (insertion-order scan, strict-less ties keep the first optimum); states
  live in a Vec on purpose.

## Remaining milestones

1. **PR3 - SIG adapter** (`audiveris-omr/src/tuned_barlines.rs`): build
   `SystemBarInput` from the GRID SIG.  Resolved semantics, from
   `stage-omr-data/scripts/eval_schenker_measure_checks.py`: cluster
   accepted barline inters by running-mean x within tolerance; `support` =
   distinct staves in the cluster; `grades` = plain `grade` values (not
   `contextual_grade`); synthesized `SYSTEM_LEFT/RIGHT` edge entries.
   **Wired byte-parity additionally requires porting that script's
   `regularize_bar_rows`** (page-level median-gap spacing model that demotes
   low-grade aligned strokes before the tuning layer ever runs) - the raw
   JSON the Python consumed is post-regularization.
2. **PR6 - wiring** (blocked until the working tree's uncommitted changes
   land): env gate `AUDIVERIS_TUNE_PIANO_BARLINES` (default OFF), call at
   the seam in `recognize_grid_lines_raster_with_scale_options` after
   `build_peak_graph` (the only point where the final SIG, system bounds,
   scale, and the grayscale raster coexist), results on a new
   `Option<TunedBarlinesReport>` field of `GridLinesRecognition`, plus a
   `tuned_barlines` section in `report.rs` after `candidates(...)`.
3. **Types layer, next slices**: volta-bracket detection landed (and now
   recovers the formerly pinned p2s5 ending barline on principle - the
   residual list is empty), but the
   `examples/classify_barlines.rs` survey over all 692 real-corpus tuned
   boundaries shows the repeat-dot probe over-fires on dense pages
   (101 repeat classifications and 159 volta flags, far beyond the true
   counts; line-end bars with adjacent noteheads are the main victims)
   even though all 60 engraved forms and the hand-verified spot cases
   classify correctly.  Next: precision work on the dot probe (dot size
   upper bound, per-staff both-spaces requirement, clear-column check
   between dots and following content), then wiring classification into
   the report.
