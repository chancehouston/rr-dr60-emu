//! The adaptive codebook of the speech codec (spec 008 research R-02, R-04; contracts/
//! codec-design.md › Quantizers and › Searches): the past excitation with its periodic
//! extension, the half-sample interpolator, the lag codes and the delta, the open-loop estimate
//! on the weighted speech and the closed-loop search. Every figure is a design constant of the
//! model (A-041; `tools/celp-design`), none a device figure.
//!
//! Lags are counted in **half-samples** throughout (`half = 2·lag`): 30 … 240 for lags
//! 15 … 120 (`tables::MIN_LAG`, `MAX_LAG`).

use super::fixed::{mac, ratio_sq_greater, round_q12, round_q15, sq};
use super::tables::{
    FRAME_SAMPLES, HALF_SAMPLE_BELOW, INTERP_HALF, LAG_CODE_TO_HALF, MAX_LAG, MIN_LAG,
    PAST_WEIGHTED, SUBFRAME_SAMPLES,
};

/// Past excitation kept: the longest lag plus the interpolator's six-sample reach, with slack.
pub(crate) const PAST_EXC: usize = 132;
/// The interpolator's taps (positions −5.5 … +5.5 around the half-sample point).
const INTERP_TAPS: usize = 12;
/// The periodic extension's reach: the subframe plus the interpolator's upper half.
const EXTENSION: usize = SUBFRAME_SAMPLES + INTERP_TAPS / 2;
/// The shortest and longest lags in half-samples.
pub(crate) const HALF_MIN: u32 = 2 * MIN_LAG as u32;
pub(crate) const HALF_MAX: u32 = 2 * MAX_LAG as u32;
/// Half-sample lags below this (72 = lag 36) have their own subframe-1 codes.
const HALF_FINE_LIMIT: u32 = 2 * HALF_SAMPLE_BELOW as u32;
/// The number of half-sample codes: 42 (lags 15 … 35.5).
const FINE_CODES: u32 = HALF_FINE_LIMIT - HALF_MIN;
const _: () = assert!(FINE_CODES == 42 && HALF_FINE_LIMIT == 72); // contracts/codec-design.md
/// Subframe 1's unused lag code (FR-010: never produced; refused on import).
pub(crate) const UNUSED_LAG_CODE: u8 = 127;
/// The subframe-2 delta's range in half-samples (a 5-bit two's-complement field).
pub(crate) const DELTA_MIN: i32 = -16;
pub(crate) const DELTA_MAX: i32 = 15;
/// The open-loop search's three lag ranges, in samples (S-009 §3.4 adapted to 15 … 120).
const RANGES: [(usize, usize); 3] = [(15, 29), (30, 59), (60, 120)];
/// The 0.85 preference for a shorter lag, squared as 7225 / 10000 (S-009 §3.4).
const PREFER_NUM: i128 = 7225;
const PREFER_DEN: i128 = 10000;
/// The weighted-speech buffer: the past 240 and the current frame.
pub(crate) const WSP_LEN: usize = PAST_WEIGHTED + FRAME_SAMPLES;

/// Subframe 1's 7-bit lag code → the lag in half-samples; `None` for code 127 (unused).
pub(crate) fn lag_code_to_half(code: u8) -> Option<u32> {
    LAG_CODE_TO_HALF
        .get(usize::from(code))
        .map(|&h| u32::from(h))
}

/// A half-sample lag → subframe 1's code: half-sample resolution below `HALF_SAMPLE_BELOW`
/// (codes 0 … 41), integer above (an odd half above it rounds down to the even lag; the encoder
/// never asks for one, `representable_sf1`).
pub(crate) fn half_to_lag_code(half: u32) -> u8 {
    let half = half.clamp(HALF_MIN, HALF_MAX);
    if half < HALF_FINE_LIMIT {
        // range-proven: half − 30 < 42.
        half.wrapping_sub(HALF_MIN) as u8
    } else {
        // range-proven: (half − 72) / 2 ≤ 84, plus 42 ≤ 126.
        FINE_CODES.wrapping_add(half.wrapping_sub(HALF_FINE_LIMIT).wrapping_shr(1)) as u8
    }
}

/// Whether subframe 1's code can carry `half` exactly.
pub(crate) fn representable_sf1(half: u32) -> bool {
    (HALF_MIN..=HALF_MAX).contains(&half) && (half < HALF_FINE_LIMIT || half & 1 == 0)
}

/// Subframe 2's lag from subframe 1's and the 5-bit delta code (two's complement, −16 … +15
/// half-samples), clamped into 15 … 120 (every code decodes, FR-010).
pub(crate) fn delta_decode(lag1_half: u32, code: u8) -> u32 {
    let code = i32::from(code & 0x1f);
    let delta = if code >= 16 {
        code.wrapping_sub(32)
    } else {
        code
    };
    // range-proven: lag1 ≤ 240, |delta| ≤ 16.
    let half = (lag1_half as i32).saturating_add(delta);
    half.clamp(HALF_MIN as i32, HALF_MAX as i32) as u32
}

/// The delta code for subframe 2's lag (the encoder keeps it within the range, so the code is
/// exact).
pub(crate) fn delta_encode(lag1_half: u32, lag2_half: u32) -> u8 {
    let delta = (lag2_half as i32).saturating_sub(lag1_half as i32);
    debug_assert!((DELTA_MIN..=DELTA_MAX).contains(&delta), "delta {delta}");
    // range-proven: masked to five bits.
    (delta & 0x1f) as u8
}

/// The lag window subframe 2 may use: `[lag1 − 16, lag1 + 15] ∩ [30, 240]` in half-samples.
pub(crate) fn delta_window(lag1_half: u32) -> (u32, u32) {
    let lo = (lag1_half as i32)
        .saturating_add(DELTA_MIN)
        .max(HALF_MIN as i32) as u32;
    let hi = (lag1_half as i32)
        .saturating_add(DELTA_MAX)
        .min(HALF_MAX as i32) as u32;
    (lo, hi)
}

/// The past excitation (S-009 §3.7.1), Q15: the most recent `PAST_EXC` samples, latest last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Excitation {
    buf: [i32; PAST_EXC],
}

impl Default for Excitation {
    fn default() -> Self {
        Self { buf: [0; PAST_EXC] }
    }
}

impl Excitation {
    pub(crate) fn reset(&mut self) {
        self.buf = [0; PAST_EXC];
    }

    /// The adaptive-codebook vector for a lag of `half` half-samples: `v[n] = e[n − T]` for an
    /// integer lag, the 12-tap interpolation at `n − T − ½` for a half-sample one; lags shorter
    /// than the subframe (and the interpolator's reach) use the periodic extension of the past
    /// (S-009 §3.7.1), in the encoder and the decoder alike.
    pub(crate) fn adaptive_vector(&self, half: u32, out: &mut [i32; SUBFRAME_SAMPLES]) {
        let half = half.clamp(HALF_MIN, HALF_MAX);
        let t = (half.wrapping_shr(1)) as usize;
        let fractional = half & 1 == 1;
        // The periodic extension: ext[PAST_EXC + k] = ext[PAST_EXC + k − T].
        let mut ext = [0i32; PAST_EXC + EXTENSION];
        ext[..PAST_EXC].copy_from_slice(&self.buf);
        for k in 0..EXTENSION {
            let i = PAST_EXC.wrapping_add(k);
            ext[i] = ext[i.wrapping_sub(t)];
        }
        for (n, o) in out.iter_mut().enumerate() {
            // The sample at n − T sits at ext index PAST_EXC + n − T (≥ 12 since T ≤ 120).
            let at = PAST_EXC.wrapping_add(n).wrapping_sub(t);
            if fractional {
                // Σ_j h[j] · e[n − T − 6 + j], j = 0 … 11 (taps at −5.5 … +5.5 of n − T − ½).
                let start = at.wrapping_sub(INTERP_TAPS / 2);
                let mut acc = 0i64;
                for (&tap, &e) in INTERP_HALF.iter().zip(&ext[start..]) {
                    acc = mac(acc, i32::from(tap), e);
                }
                *o = round_q15(acc);
            } else {
                *o = ext[at];
            }
        }
    }

    /// Appends the subframe's excitation: the past moves down by 60 and `u` becomes its most
    /// recent 60 samples.
    pub(crate) fn push_subframe(&mut self, u: &[i32; SUBFRAME_SAMPLES]) {
        self.buf.copy_within(SUBFRAME_SAMPLES.., 0);
        self.buf[PAST_EXC.wrapping_sub(SUBFRAME_SAMPLES)..].copy_from_slice(u);
    }
}

/// The open-loop lag estimate on the weighted speech (S-009 §3.4 adapted): the best
/// correlation `R(k) = Σ sw[n] sw[n − k]` over the frame in each of the three ranges (ties to
/// the shorter lag), then the shorter range's winner preferred when its normalized correlation
/// is at least 0.85 of the longer's (so pitch doubling loses). Negative correlations rank as
/// zero; all-zero input gives lag 15. The result is in samples.
pub(crate) fn open_loop_lag(wsp: &[i32; WSP_LEN]) -> usize {
    let frame = &wsp[PAST_WEIGHTED..];
    let mut winners = [(0usize, 0i64, 0i64); 3]; // (lag, R, E)
    for (w, &(lo, hi)) in winners.iter_mut().zip(RANGES.iter()) {
        let mut best: Option<(usize, i64)> = None;
        for k in lo..=hi {
            let start = PAST_WEIGHTED.wrapping_sub(k);
            let mut r = 0i64;
            for (&a, &b) in frame.iter().zip(&wsp[start..]) {
                r = mac(r, a, b);
            }
            if best.is_none_or(|(_, br)| r > br) {
                best = Some((k, r));
            }
        }
        let (k, r) = best.unwrap_or((lo, 0));
        let start = PAST_WEIGHTED.wrapping_sub(k);
        let mut e = 0i64;
        for &b in &wsp[start..start.wrapping_add(FRAME_SAMPLES)] {
            e = mac(e, b, b);
        }
        *w = (k, r, e);
    }
    let (mut t3_lag, mut t3_r, mut t3_e) = winners[2];
    for &(lag, r, e) in winners[..2].iter().rev() {
        if prefer_shorter(r, e, t3_r, t3_e) {
            t3_lag = lag;
            t3_r = r;
            t3_e = e;
        }
    }
    t3_lag
}

/// `R_s / √E_s ≥ 0.85 · R_l / √E_l` with negative correlations as zero: in integers,
/// `R_s² · E_l · 10000 ≥ 7225 · R_l² · E_s`.
fn prefer_shorter(r_short: i64, e_short: i64, r_long: i64, e_long: i64) -> bool {
    let (rs, rl) = (r_short.max(0), r_long.max(0));
    let (es, el) = (i128::from(e_short.max(1)), i128::from(e_long.max(1)));
    sq(rs).saturating_mul(el).saturating_mul(PREFER_DEN)
        >= sq(rl).saturating_mul(es).saturating_mul(PREFER_NUM)
}

/// `out[n] = Σ_{k ≤ n} x[k] h[n − k]`, rounded from Q12 (h's format) to Q15 and saturated: the
/// zero-state response of the weighted synthesis filter to a codebook vector.
pub(crate) fn convolve(
    x: &[i32; SUBFRAME_SAMPLES],
    h: &[i32; SUBFRAME_SAMPLES],
    out: &mut [i32; SUBFRAME_SAMPLES],
) {
    for (n, o) in out.iter_mut().enumerate() {
        let mut acc = 0i64;
        for (&xk, &hk) in x[..=n].iter().zip(h[..=n].iter().rev()) {
            acc = mac(acc, xk, hk);
        }
        *o = round_q12(acc);
    }
}

/// The winner of a closed-loop search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClosedLoop {
    /// The lag in half-samples.
    pub(crate) half: u32,
    /// The adaptive-codebook vector.
    pub(crate) v: [i32; SUBFRAME_SAMPLES],
    /// The vector through the weighted synthesis filter (Q15).
    pub(crate) y: [i32; SUBFRAME_SAMPLES],
    /// `Σ x·y` (the correlation with the target).
    pub(crate) corr: i64,
    /// `Σ y·y`.
    pub(crate) energy: i64,
}

/// The closed-loop search over `candidates` (half-sample lags, in ascending order): the lag
/// maximizing `max(Σ x·y, 0)² / Σ y·y`, ties to the first candidate (FR-016). `None` for no
/// candidates.
pub(crate) fn closed_loop(
    exc: &Excitation,
    target: &[i32; SUBFRAME_SAMPLES],
    h: &[i32; SUBFRAME_SAMPLES],
    candidates: &[u32],
) -> Option<ClosedLoop> {
    let mut best: Option<ClosedLoop> = None;
    for &half in candidates {
        let mut v = [0i32; SUBFRAME_SAMPLES];
        exc.adaptive_vector(half, &mut v);
        let mut y = [0i32; SUBFRAME_SAMPLES];
        convolve(&v, h, &mut y);
        let (mut corr, mut energy) = (0i64, 0i64);
        for (&xi, &yi) in target.iter().zip(&y) {
            corr = mac(corr, xi, yi);
            energy = mac(energy, yi, yi);
        }
        let better = match &best {
            None => true,
            Some(b) => ratio_sq_greater(corr.max(0), energy, b.corr.max(0), b.energy),
        };
        if better {
            best = Some(ClosedLoop {
                half,
                v,
                y,
                corr,
                energy,
            });
        }
    }
    best
}

/// The integer lags within ±3 of `center` (samples) as half-sample lags inside `[lo, hi]`,
/// ascending; returns the count written to `out`.
pub(crate) fn integer_candidates(center: usize, lo: u32, hi: u32, out: &mut [u32; 7]) -> usize {
    let mut n = 0;
    for t in center.saturating_sub(3)..=center.saturating_add(3) {
        let half = (t as u32).wrapping_mul(2);
        if (lo..=hi).contains(&half) && (HALF_MIN..=HALF_MAX).contains(&half) {
            out[n] = half;
            n = n.wrapping_add(1);
        }
    }
    n
}

/// The half-sample neighbours of the integer lag `half_int` (`half − 1, half, half + 1`) inside
/// `[lo, hi]`, with `sf1` restricting them to what subframe 1's code can carry; ascending.
pub(crate) fn fraction_candidates(
    half_int: u32,
    lo: u32,
    hi: u32,
    sf1: bool,
    out: &mut [u32; 3],
) -> usize {
    let mut n = 0;
    for half in half_int.saturating_sub(1)..=half_int.saturating_add(1) {
        let ok = (lo..=hi).contains(&half)
            && (HALF_MIN..=HALF_MAX).contains(&half)
            && (!sf1 || representable_sf1(half));
        if ok {
            out[n] = half;
            n = n.wrapping_add(1);
        }
    }
    n
}

/// 008 T013 (spec 008 FR-010, FR-016; research R-02; contracts/codec-design.md › Quantizers,
/// › Searches): the lag codes, the delta, the interpolator, the periodic extension and the
/// closed-loop search with its tie rule.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn lag_codes_map_127_distinct_lags_and_back() {
        let mut seen = BTreeSet::new();
        for code in 0..127u8 {
            let half = lag_code_to_half(code).expect("codes 0 … 126 decode");
            assert!((HALF_MIN..=HALF_MAX).contains(&half), "code {code}: {half}");
            assert!(seen.insert(half), "code {code} repeats lag {half}");
            assert_eq!(half_to_lag_code(half), code, "round trip of code {code}");
            assert!(representable_sf1(half));
        }
        assert_eq!(seen.len(), 127);
        assert_eq!(lag_code_to_half(UNUSED_LAG_CODE), None);
        assert_eq!(lag_code_to_half(200), None);
        // Half-sample below 36, integer above.
        assert_eq!(lag_code_to_half(0), Some(30));
        assert_eq!(lag_code_to_half(41), Some(71));
        assert_eq!(lag_code_to_half(42), Some(72));
        assert_eq!(lag_code_to_half(126), Some(240));
        assert!(!representable_sf1(73));
        assert!(!representable_sf1(29));
        assert!(!representable_sf1(241));
        assert_eq!(half_to_lag_code(73), 42, "an odd half above 36 rounds down");
        assert_eq!(half_to_lag_code(10), 0, "clamped");
        assert_eq!(half_to_lag_code(900), 126);
    }

    #[test]
    fn delta_decodes_two_s_complement_and_clamps() {
        assert_eq!(delta_decode(100, 0), 100);
        assert_eq!(delta_decode(100, 15), 115);
        assert_eq!(delta_decode(100, 16), 84); // −16
        assert_eq!(delta_decode(100, 31), 99); // −1
        assert_eq!(delta_decode(30, 16), 30, "clamped into 15 – 120");
        assert_eq!(delta_decode(240, 15), 240);
        assert_eq!(delta_decode(100, 0xff), 99, "masked to five bits");
        for lag1 in [30u32, 31, 100, 239, 240] {
            for delta in DELTA_MIN..=DELTA_MAX {
                let want = (lag1 as i32 + delta).clamp(30, 240) as u32;
                let code = delta_encode(lag1, (lag1 as i32 + delta).clamp(30, 240) as u32);
                assert_eq!(delta_decode(lag1, code), want, "{lag1} {delta}");
            }
            let (lo, hi) = delta_window(lag1);
            assert_eq!(lo, (lag1 as i32 - 16).max(30) as u32);
            assert_eq!(hi, (lag1 as i32 + 15).min(240) as u32);
        }
    }

    /// The interpolator is the tool's windowed sinc: 12 taps, symmetric, unit sum (within one
    /// unit), the two centre taps largest.
    #[test]
    fn interpolator_taps_equal_the_tools() {
        assert_eq!(INTERP_HALF.len(), 12);
        let sum: i32 = INTERP_HALF.iter().map(|&t| i32::from(t)).sum();
        assert!(
            (sum - 32768).abs() <= 8,
            "sum {sum} (twelve roundings of half a unit)"
        );
        for j in 0..6 {
            assert_eq!(INTERP_HALF[j], INTERP_HALF[11 - j], "symmetric");
        }
        assert_eq!(INTERP_HALF[5], 20603);
        assert!(INTERP_HALF.iter().all(|&t| t.abs() <= 20603));
        // A constant past interpolates to the constant (unit sum).
        let e = Excitation {
            buf: [1000; PAST_EXC],
        };
        let mut v = [0i32; 60];
        e.adaptive_vector(2 * 100 + 1, &mut v);
        assert!(v.iter().all(|&x| (x - 1000).abs() <= 1), "{v:?}");
        e.adaptive_vector(2 * 100, &mut v);
        assert_eq!(v, [1000; 60]);
    }

    /// Lags shorter than the subframe use the periodic extension: with a period-20 past, lag
    /// 20 reproduces the pattern over the whole subframe, in integer and half-sample form.
    #[test]
    fn periodic_extension_for_short_lags() {
        let mut e = Excitation::default();
        for (i, v) in e.buf.iter_mut().enumerate() {
            *v = if i % 20 == 7 { 8000 } else { 0 };
        }
        let mut v = [0i32; 60];
        e.adaptive_vector(40, &mut v);
        for (n, &x) in v.iter().enumerate() {
            let past = e.buf[PAST_EXC + n - 20 - 20 * ((n) / 20 + 1)];
            assert_eq!(x, past, "n = {n}");
            assert_eq!(x, if (PAST_EXC + n) % 20 == 7 { 8000 } else { 0 });
        }
        // The half-sample form of the extension is finite and periodic too.
        e.adaptive_vector(41, &mut v);
        for n in 0..40 {
            assert_eq!(v[n], v[n + 20], "n = {n}");
        }
        // push_subframe shifts the past by 60 and appends the subframe as the latest past.
        let before = e.buf;
        let u = [123i32; 60];
        e.push_subframe(&u);
        assert_eq!(&e.buf[..PAST_EXC - 60], &before[60..]);
        assert_eq!(&e.buf[PAST_EXC - 60..], &u);
        let mut v = [0i32; 60];
        e.adaptive_vector(2 * 60, &mut v);
        assert_eq!(v, u, "lag 60 reads the subframe just pushed");
        e.reset();
        assert_eq!(e, Excitation::default());
    }

    /// A periodic excitation at lag 40 is found by the closed-loop search over all lags; the
    /// equal candidates 80 and 120 (the same vector) lose to 40 by the lowest-index rule, and
    /// the rule is asserted directly with two equal candidates in both orders.
    #[test]
    fn closed_loop_finds_lag_40_and_ties_go_to_the_first_candidate() {
        let mut e = Excitation::default();
        for (i, v) in e.buf[..PAST_EXC].iter_mut().enumerate() {
            let phase = i % 40;
            *v = if phase == 3 {
                12000
            } else if phase == 11 {
                -7000
            } else if phase == 25 {
                4000
            } else {
                0
            };
        }
        let mut h = [0i32; 60];
        h[0] = 4096; // the identity: y = v
        h[1] = 1000;
        let mut target = [0i32; 60];
        e.adaptive_vector(80, &mut target); // the continuation of the pattern
        convolve(&target.clone(), &h, &mut target);
        let all: std::vec::Vec<u32> = (30..=240).collect();
        let best = closed_loop(&e, &target, &h, &all).unwrap();
        assert_eq!(best.half, 80, "lag 40 in half-samples");
        assert_eq!(best.y, target);
        let other = closed_loop(&e, &target, &h, &[240, 160, 80]).unwrap();
        assert_eq!(
            other.half, 240,
            "an equal criterion keeps the first candidate"
        );
        let other = closed_loop(&e, &target, &h, &[80, 160, 240]).unwrap();
        assert_eq!(other.half, 80);
        assert_eq!(other.corr, best.corr);
        assert_eq!(other.energy, best.energy);
        assert!(closed_loop(&e, &target, &h, &[]).is_none());
        // Candidate generators.
        let mut ints = [0u32; 7];
        assert_eq!(integer_candidates(40, 30, 240, &mut ints), 7);
        assert_eq!(ints, [74, 76, 78, 80, 82, 84, 86]);
        assert_eq!(integer_candidates(15, 30, 240, &mut ints), 4);
        assert_eq!(&ints[..4], &[30, 32, 34, 36]);
        assert_eq!(integer_candidates(120, 30, 240, &mut ints), 4);
        assert_eq!(integer_candidates(100, 196, 203, &mut ints), 4); // 196 … 202
        let mut fr = [0u32; 3];
        assert_eq!(fraction_candidates(40, 30, 240, true, &mut fr), 3);
        assert_eq!(fr, [39, 40, 41]);
        assert_eq!(
            fraction_candidates(72, 30, 240, true, &mut fr),
            2,
            "35.5 is a code, 36.5 is not"
        );
        assert_eq!(&fr[..2], &[71, 72]);
        assert_eq!(
            fraction_candidates(74, 30, 240, true, &mut fr),
            1,
            "integer only from 36"
        );
        assert_eq!(fr[0], 74);
        assert_eq!(fraction_candidates(72, 30, 240, false, &mut fr), 3);
        assert_eq!(fraction_candidates(30, 30, 240, true, &mut fr), 2);
        assert_eq!(&fr[..2], &[30, 31]);
        assert_eq!(fraction_candidates(240, 30, 240, false, &mut fr), 2);
        assert_eq!(&fr[..2], &[239, 240]);
    }

    /// The open-loop estimate: a period-40 weighted speech gives lag 40 (not 80 or 120: the
    /// shorter lag is preferred at equal correlation), silence gives 15, and a long period
    /// outside the short ranges wins when the shorter candidates correlate poorly.
    #[test]
    fn open_loop_prefers_the_shorter_of_equal_lags_and_silence_gives_15() {
        let mut w = [0i32; WSP_LEN];
        for (i, v) in w.iter_mut().enumerate() {
            *v = match i % 40 {
                0 => 9000,
                1 => 6000,
                2 => 2000,
                20 => -5000,
                _ => 0,
            };
        }
        assert_eq!(open_loop_lag(&w), 40);
        assert_eq!(open_loop_lag(&[0i32; WSP_LEN]), 15);
        // Period 90 with no sub-harmonic structure: lag 90.
        let mut w = [0i32; WSP_LEN];
        for (i, v) in w.iter_mut().enumerate() {
            *v = match i % 90 {
                0 => 9000,
                1 => 5000,
                _ => 0,
            };
        }
        assert_eq!(open_loop_lag(&w), 90);
        // The preference rule itself: equal normalized correlations prefer the shorter;
        // a shorter candidate at 0.84 of the longer does not.
        assert!(prefer_shorter(100, 100, 100, 100));
        assert!(prefer_shorter(85, 100, 100, 100));
        assert!(!prefer_shorter(84, 100, 100, 100));
        assert!(prefer_shorter(0, 100, -5, 100), "negatives rank as zero");
        assert!(prefer_shorter(0, 0, 0, 0));
    }
}
