// SPDX-License-Identifier: AGPL-3.0-or-later

//! Credible-beam veto gate (enhancement pass, default OFF).
//!
//! Java consumes BEAMS output as hard vetoes at three points that run before
//! stems exist: `LedgersFilter` drops any horizontal section intersecting any
//! beam, the ledger candidate factory purges sticks centered in a good full
//! beam, and `NoteHeadsBuilder.purgeSmallBeams` deletes heads that lose a
//! local arbitration. A hallucinated beam is therefore irreversible: on the
//! Schenker corpus, 38% of beam inters are thinner than the sheet's measured
//! beam thickness or shorter than two interlines, and beam grade does not
//! separate them from real beams (means 0.458 vs 0.422).
//!
//! Under `AUDIVERIS_CREDIBLE_BEAM_VETOES` a beam may veto only when its
//! geometry is consistent with the sheet's own beam scale: at least the main
//! beam thickness tall and at least two interlines wide. Sub-scale beams stay
//! in the SIG as hypotheses for later joint resolution; nothing they touched
//! is deleted. With the flag unset every call site behaves byte-exactly as
//! Java does.

/// `AUDIVERIS_CREDIBLE_BEAM_VETOES` gate, mirroring the
/// `AUDIVERIS_TUNE_PIANO_BARLINES` convention.
#[must_use]
pub fn credible_beam_vetoes_enabled() -> bool {
    std::env::var("AUDIVERIS_CREDIBLE_BEAM_VETOES")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

/// `AUDIVERIS_SOFT_HEAD_BLOCKS` gate (enhancement, default OFF).
///
/// Java's head lookups abandon an entire search when the nominal location is
/// blocked by a competitor inter or scores hopelessly: a head whose ideal spot
/// is grazed by its own beam is never evaluated anywhere else. Under this
/// gate the nominal block only skips that one offset and exploration
/// continues; every downstream grade gate still applies. Cached: the flag is
/// consulted once per process.
#[must_use]
pub fn soft_head_blocks_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("AUDIVERIS_SOFT_HEAD_BLOCKS")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
    })
}

/// `AUDIVERIS_STACKED_BEAM_SPLIT` gate (enhancement, default OFF).
///
/// Java splits a fused beam stack into exactly two beams, so a three- or
/// four-level stack whose gutters have closed up in a scan is unrecoverable.
/// Under this gate the split honors the count Java already derives from the
/// stack's own thickness.
#[must_use]
pub fn stacked_beam_split_limit() -> usize {
    static LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        if std::env::var("AUDIVERIS_STACKED_BEAM_SPLIT")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        {
            8
        } else {
            2
        }
    })
}

/// `AUDIVERIS_FUSED_BEAM_HEADROOM` gate (enhancement, default OFF).
///
/// A beam item's height is the distance between least-squares border fits,
/// and morphological closing fuses noteheads and stem stubs onto real beams,
/// dragging one border outward: on the Schenker corpus fused single beams
/// measure 5.0-5.7px against a declared 4. Java rejects any item taller than
/// 1.4x the typical height, so exactly the beams with notes attached die as
/// "too thick" -- re-centring the scale does not help because the floor and
/// the split count move with it (the measured-thickness experiment showed
/// this). Fusion only ever adds height, so under this gate the *ceiling
/// alone* is lifted to 2.0x. The population this admits is provably single
/// beams: any blob whose ink weight implies two beams (weight/width >= 1.5x
/// typical) is split into lines of exactly typical height before grading
/// ever sees it. A normal-height beam grades identically either way -- the
/// max-height impact at typical height is 1.0 under both ceilings.
///
/// The same fused ink also destroys the jitter (distance) impact: Java fits
/// one line through every border run endpoint and rejects an RMS residual
/// above 2% of the glyph width -- half a pixel on a short beam -- so the
/// endpoints of fused stems and heads zero the impact for exactly the beams
/// with notes attached (137 of 138 below-floor items on Schenker page 17
/// died there once the ceiling was lifted). Under this gate the jitter fit
/// is trimmed: endpoints farther than the border-grouping tolerance
/// (0.8x typical height) from the first fit are dropped and the line is
/// refit, so a beam with note bumps measures its actual border while a blob
/// that is ragged everywhere keeps its raggedness and still fails.
#[must_use]
pub fn fused_beam_headroom_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("AUDIVERIS_FUSED_BEAM_HEADROOM")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
    })
}

/// The height ceiling the gate grants: 2.0x typical instead of Java's 1.4x.
#[must_use]
pub fn beam_height_ceiling_ratio() -> f64 {
    if fused_beam_headroom_enabled() {
        2.0
    } else {
        JAVA_BEAM_HEIGHT_CEILING_RATIO
    }
}

/// Java's `maxHeightHigh` ratio in `BeamsBuilder.ItemParameters`.
pub const JAVA_BEAM_HEIGHT_CEILING_RATIO: f64 = 1.4;

/// The sheet-scale context a veto site needs to judge one beam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamVetoScale {
    pub interline: f64,
    pub beam_thickness: f64,
}

impl BeamVetoScale {
    /// `Some` when the enhancement gate is set, ready to pass down a
    /// recognition pipeline; `None` reproduces Java behavior exactly.
    #[must_use]
    pub fn from_env(interline: f64, beam_thickness: f64) -> Option<Self> {
        credible_beam_vetoes_enabled().then_some(Self {
            interline,
            beam_thickness,
        })
    }

    /// Whether a beam of this bounding size has earned veto power.
    #[must_use]
    pub fn credible(self, width: f64, height: f64) -> bool {
        height >= self.beam_thickness && width >= 2.0 * self.interline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALE: BeamVetoScale = BeamVetoScale {
        interline: 6.0,
        beam_thickness: 4.0,
    };

    #[test]
    fn sub_scale_beams_are_not_credible() {
        // The two Schenker page-1 killers: a 10x3 hook and a 10x4 beam.
        assert!(!SCALE.credible(10.0, 3.0));
        assert!(!SCALE.credible(10.0, 4.0));
        // A real beam at this scale.
        assert!(SCALE.credible(32.0, 4.0));
        // Exact thresholds are inclusive.
        assert!(SCALE.credible(12.0, 4.0));
        assert!(!SCALE.credible(11.9, 4.0));
        assert!(!SCALE.credible(12.0, 3.9));
    }
}
