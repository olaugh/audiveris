# External Design Review: Next OMR Milestone and AMT-Derived MIDI as Weak Supervision

Date: 2026-08-22. Scope: response to the ten review questions on milestone
priority and audio/AMT weak supervision for notehead recognition.

## Summary of recommendations

- **Verdict on AMT: prepare infrastructure, defer inference.** Build the
  interchange schema and a measure-level pitch-set checksum as a
  *verification and review-prioritization* tool. Do not put AMT into the
  notehead detection loop until a frozen benchmark exists and a
  perfect-MIDI upper-bound experiment on synthetic pages justifies it.
- **Highest-leverage non-AMT task: the frozen, human-verified real-scan
  benchmark, which in practice is the same work item as head/ledger
  annotation infrastructure.** Everything in the status report that says
  "on our examples" is currently unfalsifiable, including the claim of
  beating stock Audiveris.
- The one genuinely early AMT payoff is not detection at all: bar-level
  pitch-class agreement is a cheap independent checksum on **clef, key,
  and octave hypotheses** (it would have caught the tilt-induced
  baritone-clef misclassification), exactly parallel to how printed
  measure numbers already checksum bar counts.

---

## 1. Next milestones, ranked

**1. Frozen real-scan benchmark + head/stem/ledger annotation and
evaluation infrastructure (one combined milestone).** This is the highest
expected value by a wide margin, and it is the precondition for every
other item on the list, including AMT. The project's stated principle —
preserve candidates, defer decisions — is only enforceable if there is a
fixed corpus on which deferring decisions measurably beats pruning.
Today barline superiority is "not yet a frozen benchmark," heads are
"appears materially more accurate," and the ledger threshold of 0.6
"looked promising." Those are three instances of the same missing
artifact. Concretely: freeze ~30–50 pages spanning Schenker Beethoven,
both Bolcom works, 2–3 IMSLP-typical scans, and the curved Graceful
Ghost pages; human-verify heads (position, filled/half/whole), ledgers,
barlines, and system/measure structure using the labeling tools you
already built for ledgers and measure-number review; version it; never
train on it. The stem/head annotation infrastructure you list as a
candidate milestone is this milestone — annotating heads and evaluating
them are the same tooling.

**2. Finish LEDGERS and feed ledger geometry back into the dewarp
field.** It is unblocked, it has tooling and synthetic ground truth
already, and it compounds: ledgers are the only evidence for the
deformation field outside the staff, and heads/beams/stems far from the
staff — exactly the crowded-chord and rolled-grace-chord cases you cite
as misses — are where the current dewarp is least constrained. This
feedback loop (ledgers → dewarp extension → better ledger/head
extraction outside staff) is your comparative advantage over stock
Audiveris, whose `LedgersBuilder` assumes much flatter geometry. The
known weakness — half/whole heads contributing little ink across the
ledger — argues for evaluating ledger recall stratified by head type on
the frozen benchmark (which you have started) and for letting head
candidates and ledger candidates support each other bidirectionally
rather than sequentially.

**3. Mid-system clef/key/time changes.** Ranked above AMT because pitch
assignment from staff position is undefined without the correct clef
state at each x, and because *any* future AMT alignment consumes printed
pitch — an unmodeled mid-measure clef change poisons every downstream
pitch comparison in the rest of the system. It is also a bounded,
well-understood visual task with strong context priors (changes cluster
at measure boundaries; courtesy signatures at system ends predict
next-system headers).

AMT infrastructure preparation is deliberately fourth: do the thin slice
described in §10, nothing more, until milestone 1 exists.

## 2. Is AMT valuable now? Mostly premature — with one exception

Challenge accepted, in both directions. Walking the boundaries:

- **Raw visual proposal recall: AMT is nearly useless here.** Template
  matching over staff-constrained positions already achieves very high
  proposal recall cheaply; the failure mode of head proposal stages is
  precision and classification, not recall. More fundamentally, AMT
  events carry no x/y information until an alignment exists, and the
  alignment needs measures — so AMT cannot help propose until the
  structure it would help with already exists. Circular.
- **Filled/open classification: weak and indirect.** Filled vs open is a
  duration statement; extracting duration from performance MIDI requires
  tempo/rhythm alignment, which requires more structure than
  classification does. You would be using the hardest signal to fix one
  of the easier problems.
- **Pitch from staff position: genuinely useful, early, and cheap — as a
  checksum, not a detector.** A bar-level pitch-class comparison between
  AMT and (clef hypothesis + staff positions of confident heads) detects
  wrong clef, wrong key signature, and systematic octave errors with
  high reliability, because those errors shift *every* note in the
  measure and the signal aggregates. This is the first boundary where
  AMT pays for itself, and it requires no per-note alignment.
- **Chord assembly / cardinality: useful.** Piano AMT chord cardinality
  is decent, and "this vertical slice has 4 simultaneous notes, you
  found 3 heads" is a well-posed, aggregate, hard-to-hallucinate signal
  for crowded chords — one of your real miss categories.
- **Voice assignment: not useful.** AMT voice separation for piano is
  poor; do not import it.
- **Rhythm/duration: useful only as ordering,** never as durations
  (rubato, arpeggiation, pedal smearing destroy duration evidence).
  Onset *order* within a measure, with a simultaneity tolerance window,
  is robust; durations are not.
- **Missing-note recovery: the flagship use, but late.** It needs
  measure-level alignment plus per-note matching, and it is the use
  most exposed to hallucination. Gate it behind the §7 upper-bound
  experiment.
- **End-to-end semantic decoding: where AMT is strongest, eventually.**
  A performed pitch sequence is a powerful global constraint on the
  final semantic decode (accidental propagation, octava recovery, tie
  vs repeated-note disambiguation). But that stage doesn't exist yet.

So the honest answer: AMT first becomes genuinely useful at the
**measure-level pitch-set verification boundary** — which, because your
barlines and measure-number OCR are already strong, is closer than the
pessimistic framing suggests. It does *not* become useful for the
notehead stages you are currently building, and the visual pipeline's
actual bottleneck (STEMS completion, evaluation) is untouched by it.
The distracting-second-research-project risk you name is real; the
mitigation is the strict scope of §10.

## 3. What aligns to what: the smallest viable system

**Do not build a score follower, an HMM, or note-level DTW.** You have
an asset most alignment literature assumes away: a trusted measure
grid, from barlines cross-checked by printed measure numbers. Exploit
it.

Representation:

- **Score side:** a *measure DAG* — nodes are printed measures (id,
  system, page, x-span, staff set), edges are sequential flow plus
  repeat/ending/segno/coda jumps. A performance is a path through this
  DAG (possibly with cuts/omitted repeats: allow skip edges with
  penalties). You need this graph anyway for semantic output.
- **Per printed measure:** the *pitch multiset with partial order* —
  for each confident head candidate: staff position → (pitch under
  current clef/key hypothesis), x-position, and the derived 12-dim
  pitch-class histogram + head count.
- **Performance side:** AMT note events (onset, pitch, confidence)
  chunked only by the alignment itself; plus per-window chroma and
  onset-density curves.

Two stages:

- **Stage A — global: DTW of bar-level chroma over the measure DAG.**
  Standard DTW between the AMT chroma/density sequence (windowed at
  ~0.5–1 s) and the concatenated per-measure histograms of each
  candidate path through the DAG. With ≤ a handful of repeat structures
  per movement, enumerating path variants (repeat taken / not taken /
  ending choice) and picking the best DTW score is entirely adequate —
  that *is* your sequence-graph model, without HMM machinery. Output:
  per-measure time spans with an alignment cost profile, and explicit
  detection of "no good path" (cadenzas, cuts, wrong edition) as a
  first-class outcome, not an error.
- **Stage B — local: bipartite matching per measure,** only where Stage
  A confidence is high. Match AMT events ↔ candidate heads via min-cost
  flow: cost = pitch distance (octave-tolerant: full-cost octave match
  ≪ wrong pitch class) + order-violation penalty (monotone x vs onset,
  with a ~100 ms simultaneity/arpeggiation window) + (1 −
  visual score) shaping. Unmatched-on-either-side arcs with calibrated
  penalties give you the NO_PRINT / NO_PERF states natively.

That is the whole system. It produces useful evidence with zero
rhythmic recognition: Stage A alone yields the clef/key/octave checksum
and review prioritization; Stage B yields cardinality deficits and
rescue candidates.

## 4. Audio helping noteheads without circularity

Recommended machinery: **min-cost-flow matching (Stage B) feeding a
log-linear candidate re-ranker.** A factor graph is the right mental
model but the wrong first implementation; differentiable matching is a
research project; Viterbi/semi-Markov belongs to the later rhythm
stage. The re-ranker:

    score(head) = w_v · visual_score
                + w_a · align_conf · match_quality      // AMT support
                − w_m · align_conf · miss_penalty        // AMT silence
    with hard invariants:
      (1) AMT can never create a candidate. It can only re-rank
          candidates the visual stage proposed — including the
          below-threshold candidates you already retain. Your
          defer-decisions principle is precisely what makes AMT rescue
          possible without inventing image objects.
      (2) w_m is capped so that AMT silence alone can never push a
          visually strong head below acceptance. A strong head with no
          performed counterpart resolves to NO_PERF (edition variance,
          AMT miss), never deletion.
      (3) Every AMT-touched decision records provenance:
          {amt_note_ids, model_id, match_cost, align_conf, rule}.
      (4) All AMT terms are gated by Stage-A alignment confidence, so
          misalignment degrades continuously to vision-only behavior.

For NO_PRINT events (performed note, no matching candidate at any
threshold): emit a **search ticket** — measure × staff × pitch-band →
x-range × y-band — that re-runs the visual detector locally at a lower
threshold. The detection must still fire on image evidence; AMT only
directed attention. Tickets that fire nothing are recorded as
edition/AMT discrepancies and become human-review items. This is the
entire anti-hallucination story: AMT chooses where to look and how to
break ties, never what exists.

If/when you outgrow this: the factor-graph variables are head-candidate
activation and pitch, per-measure alignment span, and match edges; the
factors are visual likelihood, staff-position/clef consistency, AMT
match, chord-cardinality, mutual-exclusion (overlapping candidates),
and alignment quality gating all AMT factors. But the re-ranker version
will tell you whether that investment is warranted.

## 5. Uncertainty representation and interchange schema

Do not reduce AMT to a MIDI file. Keep, in descending priority:

1. Per-note confidence (decoded events).
2. Onset and frame posteriorgrams (88 × T, downsampled to ~20–50 ms) —
   this subsumes top-k pitch alternatives and lets you re-query "was
   there any energy at pitch p near time t?" when a search ticket needs
   corroboration, without re-running the model.
3. Pedal posterior curve (explains merged/missing repeated notes).
4. Ensemble members kept separate (model id per event), merged only at
   match time — inter-model agreement is a better-calibrated per-note
   confidence than any single model's logits.
5. Alignment output: per-measure spans with costs; per-note match edges
   with costs; explicit `NO_PRINT` and `NO_PERF` states; per-movement
   path choice through the measure DAG with margin over the runner-up
   path.

Concrete interchange (one JSON/Arrow bundle per recording):

    amt_bundle/
      meta.json          # recording id, model ids+versions, sample rate
      notes.parquet      # model_id, onset_s, offset_s, pitch, velocity, conf
      posteriors.npz     # onset[88,T], frame[88,T], pedal[T], hop_s
    alignment_bundle/    # per (recording, score) pair
      path.json          # measure DAG path taken, per-measure spans+costs
      matches.parquet    # measure_id, amt_note_id|null, head_cand_id|null,
                         # cost, state ∈ {MATCH, NO_PRINT, NO_PERF}, provenance

Chord-size distributions and octave alternatives are *derived* from the
posteriorgrams at query time; don't schema-freeze them.

## 6. AMT capability requirements and systems

Requirements, in order: (a) piano-specific onset/frame posterior access
— not token-only decoding; (b) explicit pedal and repeated-note (re-onset)
modeling; (c) local, cheap inference (a movement in ≪ real time on one
GPU or CPU-tolerable); (d) permissive license, pinnable weights;
(e) known behavior on old/noisy recordings — all mainstream models are
MAESTRO-trained (modern Disklavier audio) and degrade on historical
recordings, so plan light denoising/bandwidth checks and treat pre-1950
recordings as out of scope initially.

Current sensible picks: the Kong et al. high-resolution piano
transcription model (strong onsets, pedal model, permissive, posteriors
accessible) as primary; Magenta Onsets & Frames (Apache-2.0, simple,
posteriorgrams trivially accessible) as the architectural second;
hFT-Transformer or Transkun as alternates. Avoid MT3-style token
decoders for this use — no usable posteriors. **Ensemble exactly two**
architecturally different models: the purpose is not accuracy but
calibration — agreement/disagreement is your per-note confidence, and a
note only one model hears should never drive a rescue.

## 7. Evaluation: ablations and the kill/keep criteria

Run in this order, cheapest kill first:

1. **Upper bound (synthetic, kill test):** vision-only vs vision +
   *perfect* score MIDI on degraded/warped synthetic pages, via the §3
   pipeline. If perfect MIDI adds **< 2 points notehead F1 on degraded
   synthetic**, abandon AMT-for-detection outright — no real AMT can
   beat its own ceiling. This costs a day and can kill the whole idea.
2. **Degraded oracle:** perfect MIDI corrupted by an explicit AMT error
   model (octave substitutions, merged repeated notes under pedal,
   spurious harmonic false positives, onset jitter, deletions —
   parameters measured, not guessed, from a real AMT run on rendered
   audio of the same synthetic scores). Maps the degradation curve.
3. **Real AMT on real scans** (Beethoven; Bolcom as stress case).
4. **Signal ablations:** order-only, pitch-class-only, cardinality-only
   — tells you which factor earns its weight in §4.
5. **Controls for hallucination:** deliberately mismatched
   recording/edition, and shuffled-measure alignment. Required result:
   gains vanish *gracefully* (alignment confidence gates to
   vision-only) and **AMT-attributable false inventions stay near zero**
   — every accepted head traceable to an AMT promotion is audited by a
   human against the image.
6. **Stage ablation:** AMT after candidate generation only, vs AMT
   allowed to issue search tickets — measures whether tickets add
   recall or only risk.

Metrics: head proposal P/R and F1 stratified (clean synthetic /
degraded synthetic / hard real), filled-open accuracy, staff-position
error, chord-cardinality error, AMT-attributable FP rate, and
alignment-failure detection rate (did the system *know* the Bolcom
edition didn't match?).

**Keep** if real AMT yields ≥ 2–3 points head recall on hard real scans
at ≤ 0.5 point precision cost, with AMT-attributable FP < ~0.2% of
accepted heads and mismatch controls clean. **Abandon (for detection)**
if experiment 1 fails, or real-AMT gain is < 1 point, or mismatch
controls show priors leaking into output. Note the verification/review
use of §2 survives an abandon verdict — its bar is far lower (does the
review queue surface real errors first? yes/no).

## 8. Paired data curriculum

The centerpiece should be **ASAP-style existing aligned data** (score
MusicXML + real performance MIDI, beat-aligned; heavily Beethoven/
Chopin/Liszt): render the score side through your Verovio/MuseScore
exact-label pipeline (clean + degraded variants), and you get real
human performances with exact printed-note ground truth and exact
alignment — no AMT needed to bootstrap, and audio-synthesis of the
performance MIDI gives you AMT-model input for measuring the §7 error
model. Tiers:

1. **Synthetic + exact MIDI** (have it): alignment correctness, upper
   bounds, unit tests.
2. **Aligned score–performance corpora (ASAP etc.), scores re-rendered
   by your pipeline:** train/fit alignment costs, AMT error model,
   re-ranker weights.
3. **Schenker Beethoven scans + commercial/public recordings**, with
   human-confirmed measure anchors (your measure-number OCR gives these
   nearly free): evaluation.
4. **Bolcom + available performances:** adversarial evaluation only —
   edition mismatch, unusual notation.

Anti-leakage rule: split **by work (opus), across all tiers at once** —
a work that appears in any training tier, in any rendering,
degradation, or edition, is banned from evaluation. Your own history
(one symbolic source reflowed many ways masquerading as diversity) is
exactly the failure this prevents. Beethoven Op. 2 No. 1 should be
declared evaluation-only *now*, before any AMT fitting happens, since
so much tooling already touched it.

## 9. Training vs inference: inference-time only, for now

- **Test-time re-ranking + verification + review prioritization: yes.**
  This is the entire recommended scope.
- **Joint multimodal training: no.** It welds AMT priors into the
  visual weights invisibly — the exact circularity you fear, made
  undebuggable — and multiplies the renderer-monoculture risk (the
  model can learn alignment artifacts of your synthesis chain).
- **Offline pseudo-labeling: only through a blind-review gate.** AMT
  may *nominate* regions; a human confirms **from the image alone,
  seeing no AMT overlay** before anything becomes a training label; the
  label carries `origin: amt_nominated_human_confirmed` forever, so you
  can ablate the cohort later. AMT-derived promotions never enter
  training unconfirmed — enforce this in the data loader (refuse
  unconfirmed provenance), not in a convention document.
- **Human-review prioritization: yes, immediately** — it is the
  cheapest and least risky consumer of the alignment, and it
  accelerates milestone 1 (annotators triage disagreement-flagged
  measures first).
- **Detect-but-don't-promote for missing objects: yes** — that is the
  search-ticket + NO_PRINT ledger of §4.

## 10. Bounded two-day proof of concept

**Inputs:** (a) one clean Verovio-rendered movement with exact MIDI and
exact head labels, not from an evaluation work; (b) its curl+speckle
degraded variant; (c) Beethoven Op. 2 No. 1, mvt with best
measure-number OCR, one public-domain/owned recording; (d) Bolcom pair
only if hours remain — expected outcome there is "alignment correctly
reports low confidence," which is itself a pass. Existing geometry,
measure grid, and native HEADS candidates (including sub-threshold
retained candidates) are assumed.

**Day 1 — synthetic, oracle:** implement the measure DAG (trivial here:
no repeats or use one with a repeat), bar-chroma DTW (Stage A),
min-cost-flow matcher (Stage B), and the capped log-linear re-ranker.
Run vision-only vs +perfect-MIDI vs +error-model-MIDI on the degraded
page. This is experiment 7.1–7.2. **Stop condition 1:** perfect-MIDI
gain < 2 F1 → declare AMT-for-detection dead, ship the byproducts, skip
Day 2's re-ranking (do only the checksum).

**Day 2 — real:** run one AMT model (Kong et al.) on the recording;
Stage A against the OCR-anchored measure grid; emit (a) per-measure
pitch-class agreement + review queue, (b) rescue list (sub-threshold
head candidates with strong AMT matches), (c) NO_PRINT ticket list.
Human work: with the existing labeling tool, adjudicate the top ~30
review-queue measures and every rescue/ticket (~1–2 h). **Metrics:**
Stage-A measure spans vs ~10 hand-tapped anchor measures; rescue
precision (target: > 80% of rescued candidates are real heads);
AMT-attributable inventions (target: 0); review-queue enrichment (are
flagged measures actually error-denser than unflagged?). **Stop
condition 2:** Stage A cannot align a professionally recorded Beethoven
movement to an OCR-numbered measure grid → the foundation is unsound;
halt and diagnose rather than adding machinery.

**Reusable even if AMT fails:** the measure DAG with repeat/ending
paths (needed for semantic output regardless); the interchange schema;
the min-cost-flow pitch-multiset matcher — which, pointed at *clean*
reference MIDI/MusicXML instead of AMT, is a general OMR-output-vs-
reference symbolic differ, i.e. your future end-to-end QA harness; the
review-prioritization queue; bar-chroma DTW (reusable for
playback-synchronized review tooling). That is most of the code, which
is why the two days are a good bet even under the pessimistic branch.

---

## Bottom line

**Do not proceed with AMT-in-the-detection-loop now.** Prepare the thin
infrastructure (schema, measure DAG, DTW + flow matcher) via the
bounded PoC, adopt the measure-level pitch-set checksum for clef/key/
octave verification and human-review triage — and gate any detection
role on the perfect-MIDI upper bound and the frozen benchmark. AMT's
real leverage arrives at semantic decoding, which you have not reached.

**The single highest-leverage non-AMT task while STEMS is blocked: the
frozen, human-verified real-scan benchmark (heads, ledgers, barlines,
measures), with LEDGERS-completion-plus-dewarp-extension as the first
milestone measured against it.** Every claim the project currently
makes — and every future decision about AMT — depends on it existing.
