//! Codec band-limit stages: signal-chain stages 4 and 10 (spec 004; A-026, A-027, A-015, A-016;
//! S-005).
//!
//! Stage 4 models the record path (the MSM7702's transmit filter and ADC): a band-pass that
//! passes about 225 to 2550 Hz. Stage 10 models the playback path (DAC and receive filter): a
//! low-pass that passes DC. Both follow the OKI codec family's datasheet templates scaled by
//! 0.75 to the device's measured 6 kHz clock (A-027, medium confidence), have unity gain at
//! 765 Hz (A-015) and are minimum-phase (A-016). They are modeled on S-005, not measured from a
//! real unit. The coefficients live in `voiceband_coeffs.rs`, generated and verified by
//! `tools/filter-design/design_voiceband.py` (spec 004 research R-04, R-05, R-06).

use rr_dr60_detmath::{TAU, cos, sin};

use super::biquad::Biquad;
use super::voiceband_coeffs::{PLAYBACK_SOS, RECORD_SOS};
use crate::settings::DEVICE_RATE_HZ;

/// A band-limit stage: a cascade of `N` biquad sections at the 6 kHz device rate (A-026).
///
/// The record stage has 5 sections (3rd-order high-pass + 5th-order low-pass) and the playback
/// stage 3 (the low-pass alone), so each is its own type and the per-sample work stays a
/// fixed-size array with no allocation (spec 004 research R-06).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VoiceBandStage<const N: usize> {
    sections: [Biquad; N],
}

/// Signal-chain stage 4 (record path, transmit template; A-027).
pub(crate) type RecordStage = VoiceBandStage<5>;
/// Signal-chain stage 10 (playback path, receive template; A-027).
pub(crate) type PlaybackStage = VoiceBandStage<3>;

impl RecordStage {
    /// The record stage with the committed coefficients (`RECORD_SOS`), zero state.
    pub(crate) fn record() -> Self {
        Self::with_sos(RECORD_SOS)
    }
}

impl PlaybackStage {
    /// The playback stage with the committed coefficients (`PLAYBACK_SOS`), zero state.
    pub(crate) fn playback() -> Self {
        Self::with_sos(PLAYBACK_SOS)
    }
}

impl<const N: usize> VoiceBandStage<N> {
    /// A stage with explicit coefficients. Used by `record`/`playback` and, through the hidden
    /// `__test-hooks` feature, by the SC-007 mutation tests (spec 004 tasks.md T024).
    pub(crate) fn with_sos(sos: [[f64; 5]; N]) -> Self {
        Self {
            sections: sos.map(Biquad::from_sos),
        }
    }

    /// Processes one device-rate sample.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let mut y = x;
        for s in &mut self.sections {
            y = s.process(y);
        }
        y
    }

    /// Multiply-adds performed by all sections (test-only; 001 tasks.md T059).
    #[cfg(all(test, feature = "op-count"))]
    pub(crate) fn ops(&self) -> u64 {
        self.sections.iter().map(|s| s.ops).sum()
    }

    /// Clears all state. No allocation (001 FR-017).
    pub(crate) fn reset(&mut self) {
        for s in &mut self.sections {
            s.reset();
        }
    }

    /// Group delay at 1 kHz, in device-rate samples, computed analytically from the
    /// coefficients (001 research R-10). Uses `rr_dr60_detmath`, so it is bit-identical
    /// everywhere and the reported latency is too.
    pub(crate) fn group_delay_1k_samples(&self) -> f64 {
        let w = TAU * 1000.0 / f64::from(DEVICE_RATE_HZ);
        self.sections
            .iter()
            .map(|s| {
                let [b0, b1, b2, a1, a2] = s.coefficients();
                poly_group_delay([b0, b1, b2], w) - poly_group_delay([1.0, a1, a2], w)
            })
            .sum()
    }
}

/// Group delay in samples of P(z) = p0 + p1·z⁻¹ + p2·z⁻², at angular frequency `w`:
/// Re(Σ k·p_k·e^(−jωk) / Σ p_k·e^(−jωk)).
fn poly_group_delay(p: [f64; 3], w: f64) -> f64 {
    let (c1, s1, c2, s2) = (cos(w), sin(w), cos(2.0 * w), sin(2.0 * w));
    // Denominator D = p0 + p1·e^(−jω) + p2·e^(−j2ω); numerator N = p1·e^(−jω) + 2·p2·e^(−j2ω).
    let (dr, di) = (p[0] + p[1] * c1 + p[2] * c2, -(p[1] * s1 + p[2] * s2));
    let (nr, ni) = (p[1] * c1 + 2.0 * p[2] * c2, -(p[1] * s1 + 2.0 * p[2] * s2));
    (nr * dr + ni * di) / (dr * dr + di * di)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test-only analysis; not bit-critical.
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    const FS: f64 = 6000.0; // A-026: the device rate
    const REF_HZ: f64 = 765.0; // 0.75 × 1020 Hz, the S-005 reference frequency (A-027)
    const EDGE_HZ: f64 = 2550.0; // 0.75 × 3400 Hz (A-027)

    /// Complex response of a cascade at `f` Hz: (re, im).
    fn response(sos: &[[f64; 5]], f: f64) -> (f64, f64) {
        let w = std::f64::consts::TAU * f / FS;
        let (z1r, z1i) = (w.cos(), -w.sin());
        let (z2r, z2i) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let mut h = (1.0, 0.0);
        for &[b0, b1, b2, a1, a2] in sos {
            let br = b0 + b1 * z1r + b2 * z2r;
            let bi = b1 * z1i + b2 * z2i;
            let ar = 1.0 + a1 * z1r + a2 * z2r;
            let ai = a1 * z1i + a2 * z2i;
            let d = ar * ar + ai * ai;
            let (qr, qi) = ((br * ar + bi * ai) / d, (bi * ar - br * ai) / d);
            h = (h.0 * qr - h.1 * qi, h.0 * qi + h.1 * qr);
        }
        h
    }

    fn mag_db(sos: &[[f64; 5]], f: f64) -> f64 {
        let (r, i) = response(sos, f);
        10.0 * (r * r + i * i).log10()
    }

    /// Loss relative to the reference frequency, in dB (positive = quieter than at 765 Hz).
    fn loss_db(sos: &[[f64; 5]], f: f64) -> f64 {
        mag_db(sos, REF_HZ) - mag_db(sos, f)
    }

    /// Numerical group delay in ms (central difference of the unwrapped phase).
    fn group_delay_ms(sos: &[[f64; 5]], f: f64) -> f64 {
        let h = 0.01;
        let (r1, i1) = response(sos, f - h);
        let (r2, i2) = response(sos, f + h);
        let dphi = (i2 * r1 - r2 * i1).atan2(r2 * r1 + i2 * i1); // arg(H2 / H1)
        -dphi / (std::f64::consts::TAU * 2.0 * h / FS) / FS * 1000.0
    }

    /// Group-delay distortion at `f`: group delay minus its minimum over 225–2550 Hz (S-005's
    /// definition, A-027).
    fn gdd_ms(sos: &[[f64; 5]], f: f64) -> f64 {
        let mut gmin = f64::MAX;
        let mut x = 225.0;
        while x <= EDGE_HZ {
            gmin = gmin.min(group_delay_ms(sos, x));
            x += 2.5;
        }
        group_delay_ms(sos, f) - gmin
    }

    fn assert_stable_and_minimum_phase(sos: &[[f64; 5]]) {
        for &[b0, b1, b2, a1, a2] in sos {
            // Jury test: poles strictly inside the unit circle...
            assert!(
                a2.abs() < 1.0 && a1.abs() < 1.0 + a2,
                "unstable section {a1} {a2}"
            );
            // ...and zeros on or inside it (minimum phase, A-016), with root-finding slack.
            let (c0, c1, c2) = if b0 < 0.0 {
                (-b0, -b1, -b2)
            } else {
                (b0, b1, b2)
            };
            assert!(
                c2.abs() <= c0 * (1.0 + 1e-9) && c1.abs() <= (c0 + c2) * (1.0 + 1e-9),
                "zeros outside"
            );
        }
    }

    /// 004 FR-005 (A-027 transmit template, S-005 scaled by 0.75): the record stage's loss.
    #[test]
    fn record_template_loss() {
        let sos = &RECORD_SOS[..];
        assert!(mag_db(sos, REF_HZ).abs() <= 0.1, "{}", mag_db(sos, REF_HZ)); // A-015
        let mut f = 225.0;
        while f <= 2250.0 {
            let l = loss_db(sos, f);
            assert!((-0.15..=0.2).contains(&l), "loss {l:+.3} dB at {f} Hz"); // S-005, A-027
            f += 1.0;
        }
        let edge = loss_db(sos, EDGE_HZ);
        assert!((0.0..=0.8).contains(&edge), "loss at 2550 Hz {edge:.3} dB"); // S-005, A-027
        assert!(loss_db(sos, 45.0) >= 20.0, "45 Hz: {}", loss_db(sos, 45.0)); // S-005 60 Hz row
        assert!(loss_db(sos, 20.0) >= 20.0, "20 Hz: {}", loss_db(sos, 20.0));
        // ≥ 40 dB at DC (engineering target, 004 FR-015; G.712 states no DC requirement). The
        // high-pass has zeros at z = 1, so the attenuation is infinite.
        assert!(loss_db(sos, 0.0) >= 40.0, "DC: {}", loss_db(sos, 0.0));
        std::eprintln!(
            "record stage: loss at 2550 Hz {edge:.3} dB; report-only 2700 Hz {:.1} dB, 2900 Hz {:.1} dB",
            loss_db(sos, 2700.0),
            loss_db(sos, 2900.0)
        );
    }

    /// 004 FR-006 (S-005 transmit group-delay rows ÷ 0.75, A-027) and A-016.
    #[test]
    fn record_template_group_delay() {
        let sos = &RECORD_SOS[..];
        for (f, limit) in [
            (375.0, 1.0),
            (450.0, 0.47),
            (750.0, 0.17),
            (1000.0, 0.17),
            (1500.0, 0.17),
            (1950.0, 0.17),
            (2100.0, 1.0),
        ] {
            let g = gdd_ms(sos, f);
            assert!(g <= limit, "GDD {g:.3} ms at {f} Hz (limit {limit})");
        }
        let gd1k = group_delay_ms(sos, 1000.0);
        assert!(gd1k <= 2.0, "group delay at 1 kHz = {gd1k} ms"); // A-016
        assert!(group_delay_ms(sos, 225.0) > gd1k); // A-016
        let gd_edge = group_delay_ms(sos, EDGE_HZ);
        assert!(gd_edge > gd1k); // A-016
        assert!(gd_edge <= 2.0, "group delay at 2550 Hz = {gd_edge} ms"); // engineering target
        assert_stable_and_minimum_phase(sos);
    }

    /// 004 FR-008 (A-027 receive template, corrected: a low-pass that passes DC; S-005 scaled).
    #[test]
    fn playback_template_loss() {
        let sos = &PLAYBACK_SOS[..];
        assert!(mag_db(sos, REF_HZ).abs() <= 0.1, "{}", mag_db(sos, REF_HZ)); // A-015
        for f in [0.0, 20.0, 45.0] {
            let l = loss_db(sos, f);
            assert!(
                (-0.15..=0.25).contains(&l),
                "loss {l:+.3} dB at {f} Hz (passes DC)"
            );
        }
        let mut f = 1.0;
        while f <= 2250.0 {
            let l = loss_db(sos, f);
            assert!((-0.15..=0.25).contains(&l), "loss {l:+.3} dB at {f} Hz"); // S-005, A-027
            f += 1.0;
        }
        let edge = loss_db(sos, EDGE_HZ);
        assert!((0.0..=0.8).contains(&edge), "loss at 2550 Hz {edge:.3} dB"); // S-005, A-027
        std::eprintln!(
            "playback stage: loss at 2550 Hz {edge:.3} dB; report-only 2700 Hz {:.1} dB, 2900 Hz {:.1} dB",
            loss_db(sos, 2700.0),
            loss_db(sos, 2900.0)
        );
    }

    /// 004 FR-008 (S-005 receive group-delay rows ÷ 0.75, A-027) and A-016.
    #[test]
    fn playback_template_group_delay() {
        let sos = &PLAYBACK_SOS[..];
        for (f, limit) in [
            (375.0, 0.17),
            (450.0, 0.17),
            (750.0, 0.17),
            (1000.0, 0.17),
            (1500.0, 0.17),
            (1950.0, 0.17),
            (2100.0, 1.0),
        ] {
            let g = gdd_ms(sos, f);
            assert!(g <= limit, "GDD {g:.3} ms at {f} Hz (limit {limit})");
        }
        let gd1k = group_delay_ms(sos, 1000.0);
        assert!(gd1k <= 2.0, "group delay at 1 kHz = {gd1k} ms"); // A-016
        let gd_edge = group_delay_ms(sos, EDGE_HZ);
        assert!(gd_edge > gd1k); // A-016
        assert!(gd_edge <= 2.0, "group delay at 2550 Hz = {gd_edge} ms"); // engineering target
        assert_stable_and_minimum_phase(sos);
    }

    #[test]
    fn analytic_group_delay_matches_numeric() {
        let r = RecordStage::record().group_delay_1k_samples();
        let rn = group_delay_ms(&RECORD_SOS, 1000.0) / 1000.0 * FS;
        assert!(
            (r - rn).abs() <= 1e-8,
            "record: analytic {r} vs numeric {rn}"
        );
        let p = PlaybackStage::playback().group_delay_1k_samples();
        let pn = group_delay_ms(&PLAYBACK_SOS, 1000.0) / 1000.0 * FS;
        assert!(
            (p - pn).abs() <= 1e-8,
            "playback: analytic {p} vs numeric {pn}"
        );
    }

    fn impulse_response<const N: usize>(stage: &mut VoiceBandStage<N>, n: usize) -> Vec<f64> {
        (0..n)
            .map(|k| stage.process(if k == 0 { 1.0 } else { 0.0 }))
            .collect()
    }

    #[test]
    fn stage_process_matches_cascade_and_resets() {
        let mut r = RecordStage::record();
        let first = impulse_response(&mut r, 64);
        r.reset();
        assert_eq!(first, impulse_response(&mut r, 64));
        assert!(first.iter().any(|&v| v != 0.0));

        let mut p = PlaybackStage::playback();
        let first = impulse_response(&mut p, 64);
        p.reset();
        assert_eq!(first, impulse_response(&mut p, 64));
        // A low-pass passes DC: its impulse response sums toward the DC gain (≈ 1).
        let sum: f64 = impulse_response(&mut PlaybackStage::playback(), 4000)
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 0.02, "playback DC gain {sum}");
    }
}
