//! The one-pass drain's settle lengths (spec 026 D2; plan R-02): the last sample of a section
//! cascade's impulse response at or above 10⁻⁵ of its peak (the −100 dB rule of `N_OUT` and
//! `N10`), measured through the engine's own sections at construction.

use super::biquad::Biquad;
use super::first_order::FirstOrder;

/// The settle of an impulse response read sample by sample from `next` over `n` samples.
fn settle(n: usize, mut next: impl FnMut(f64) -> f64) -> u32 {
    let (mut peak, mut last) = (0.0f64, 0usize);
    // No storage: the peak of these responses lies in their first samples (an impulse into a
    // high-pass or a band-pass), so a first short pass fixes the threshold.
    let mut h0 = [0.0f64; 64];
    for (i, h) in h0.iter_mut().enumerate() {
        *h = next(if i == 0 { 1.0 } else { 0.0 }).abs();
        peak = peak.max(*h);
    }
    let thr = 1e-5 * peak;
    for (i, h) in h0.iter().enumerate() {
        if *h >= thr {
            last = i + 1;
        }
    }
    for i in 64..n {
        if next(0.0).abs() >= thr {
            last = i + 1;
        }
    }
    last as u32
}

/// The settle of a cascade of first-order sections (`[b0, b1, a1]`), over at most `n` samples.
pub(crate) fn first_order_settle(secs: &[[f64; 3]], n: usize) -> u32 {
    let mut s: [FirstOrder; 3] = [
        FirstOrder::new(secs[0]),
        FirstOrder::new(*secs.get(1).unwrap_or(&[1.0, 0.0, 0.0])),
        FirstOrder::new(*secs.get(2).unwrap_or(&[1.0, 0.0, 0.0])),
    ];
    settle(n, |x| s.iter_mut().fold(x, |y, f| f.process(y)))
}

/// The settle of a biquad cascade (`[b0, b1, b2, a1, a2]` per section), over at most `n`.
pub(crate) fn sos_settle(sos: &[[f64; 5]], n: usize) -> u32 {
    let mut b: [Biquad; 5] = [
        Biquad::from_sos(sos[0]),
        Biquad::from_sos(*sos.get(1).unwrap_or(&[1.0, 0.0, 0.0, 0.0, 0.0])),
        Biquad::from_sos(*sos.get(2).unwrap_or(&[1.0, 0.0, 0.0, 0.0, 0.0])),
        Biquad::from_sos(*sos.get(3).unwrap_or(&[1.0, 0.0, 0.0, 0.0, 0.0])),
        Biquad::from_sos(*sos.get(4).unwrap_or(&[1.0, 0.0, 0.0, 0.0, 0.0])),
    ];
    settle(n, |x| b.iter_mut().fold(x, |y, f| f.process(y)))
}

/// Stage 2's settle at the host rate's index (the input high-pass, the output coupling and the
/// roll-off, research R-02 of spec 012).
pub(crate) fn mic_amp_settle(rate_index: usize) -> u32 {
    use super::mic_coeffs::{HP_IN, HP_OUT, LP};
    first_order_settle(
        &[HP_IN[rate_index], HP_OUT[rate_index], LP[rate_index]],
        400_000,
    )
}

/// Stage 4's record band-pass settle at the 6 kHz device rate (N₄).
pub(crate) fn record_settle() -> u32 {
    sos_settle(&super::voiceband_coeffs::RECORD_SOS, 40_000)
}

/// Stage 10's reconstruction settle at 6 kHz, by the same rule (N₁₀'s check, in the tests).
#[cfg(test)]
pub(crate) fn playback_settle() -> u32 {
    sos_settle(&super::voiceband_coeffs::PLAYBACK_SOS, 40_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N₄ and N₁₀ as `uv run tools/filter-design/design_voiceband.py --drain` prints them (the
    /// hardware review's figures; N₁₀ the pinned constant).
    #[test]
    fn device_settles() {
        assert_eq!(record_settle(), 160);
        assert_eq!(playback_settle(), 121);
        assert_eq!(playback_settle() as usize, crate::paths::N10);
    }

    /// Stage 2's settle by rate (`SUPPORTED_HOST_RATES` order), pinned to the print of
    /// `uv run tools/mic-chain/design_mic_chain.py --drain` (the 77 Hz coupling's decay).
    #[test]
    fn mic_settle_by_rate() {
        let v: [u32; 7] = core::array::from_fn(mic_amp_settle);
        assert_eq!(v, [90, 113, 200, 463, 498, 857, 926]);
    }
}
