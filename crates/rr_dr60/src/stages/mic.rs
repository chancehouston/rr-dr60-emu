//! Signal-chain stages 1 – 2, the microphone path (spec 012; research R-02 – R-05): the
//! capsule's hiss and the microphone amplifier, at the host rate ahead of the limiter.
//!
//! - [`MicCapsule`] (stage 1): the input plus a deterministic Gaussian hiss (A-048), a
//!   128-layer ziggurat over xoshiro256++ keyed from the engine seed and the recording counter
//!   (A-049), each rejection loop capped at [`RETRY_CAP`] attempts.
//! - [`MicAmp`] (stage 2): the input high-pass with the 1 kHz normalization in its numerator
//!   (A-046), the clipper, the output coupling and the roll-off (A-047, A-053): three
//!   first-order sections fitted per host rate (`mic_coeffs`).
//! - [`MicFront`]: the two, each `None` when its switch is off (FR-007), so a bypassed stage is
//!   a skipped branch and bit-identical to 0.11 by construction (FR-010).
//!
//! Every figure is a design constant modeled on the circuit (S-002 sheet 3.1), printed by
//! `tools/mic-chain/design_mic_chain.py`; nothing here is a measurement of a real unit. Only
//! basic IEEE operations run per sample; the ziggurat's tail and wedge use
//! `rr_dr60_detmath::{exp, ln}` (FR-003: no platform math).

use rr_dr60_detmath::{exp, ln};

use crate::settings::MicSettings;
use crate::stages::first_order::FirstOrder;
use crate::stages::mic_coeffs::{
    HISS_SIGMA, HP_IN, HP_OUT, LP, MIC_COUNTER_SALT, MIC_STREAM, RETRY_CAP, ZIG_RATIO, ZIG_TAIL_R,
    ZIG_X,
};
use crate::stages::output::rate_index;

/// SplitMix64's increment (Steele, Lea & Flood 2014), the keying's state expansion (A-049).
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15; // A-049

/// SplitMix64's finalizer (A-049): a bijection of `u64`.
#[inline]
const fn mix64(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9); // A-049 (SplitMix64)
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB); // A-049 (SplitMix64)
    z ^ (z >> 31)
}

/// The xoshiro256++ state for `(seed, counter)` (spec 012 FR-003, FR-009; A-049):
/// `z = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT)`, then four SplitMix64
/// outputs from `z`. Two independent bijections keep the seed and the counter from cancelling,
/// and SplitMix64 never yields an all-zero state, so seed 0 does not degenerate.
pub(crate) const fn key(seed: u64, counter: u64) -> [u64; 4] {
    let mut z = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT);
    let mut s = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        z = z.wrapping_add(GOLDEN_GAMMA);
        s[i] = mix64(z);
        i += 1;
    }
    s
}

/// 2⁻⁵³: a 53-bit integer to [0, 1).
const TWO_POW_M53: f64 = 1.0 / 9_007_199_254_740_992.0; // 2^-53 (A-049)

/// Stage 1, the capsule (spec 012 FR-003, FR-004): the input plus the hiss at
/// σ_rate · 10^(offset / 20). The capsule's own response is flat (A-052).
#[derive(Clone, Debug)]
pub(crate) struct MicCapsule {
    /// The xoshiro256++ state.
    state: [u64; 4],
    /// The engine seed (`Settings::seed`).
    seed: u64,
    /// The recording counter this stream is keyed to (FR-009).
    counter: u64,
    /// σ_rate · 10^(offset / 20), computed at construction.
    gain: f64,
    /// Test-only: every rejection loop runs to its cap (A-049's cap path, spec 012 T023).
    #[cfg(test)]
    force_cap: bool,
}

impl MicCapsule {
    /// The capsule for `mic`, keyed to `(seed, 0)` (FR-009). `None` for an unsupported rate
    /// (validated before construction).
    pub(crate) fn new(mic: &MicSettings, seed: u64, host_rate_hz: u32) -> Option<Self> {
        let sigma = HISS_SIGMA[rate_index(host_rate_hz)?]; // A-048
        // exp(0.0) is exactly 1.0, so offset 0 multiplies by σ exactly (research R-04).
        let gain = sigma * exp(f64::from(mic.hiss_offset_db) * core::f64::consts::LN_10 / 20.0);
        Some(Self {
            state: key(seed, 0),
            seed,
            counter: 0,
            gain,
            #[cfg(test)]
            force_cap: false,
        })
    }

    /// Keys the stream to `(seed, counter)` from its beginning (FR-009).
    pub(crate) fn rekey(&mut self, counter: u64) {
        self.counter = counter;
        self.state = key(self.seed, counter);
    }

    /// Back to the beginning of the current counter's stream.
    pub(crate) fn reset(&mut self) {
        self.rekey(self.counter);
    }

    /// One host sample: `x` plus the hiss.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        x + self.gain * self.gauss()
    }

    /// xoshiro256++ (Blackman & Vigna 2019; A-049).
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// A uniform in (0, 1] from bits 11 – 63, so `ln` never sees 0 (research R-04).
    #[inline]
    fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 * TWO_POW_M53
    }

    /// One standard normal draw: Doornik's ZIGNOR ziggurat, 128 layers (A-049). The layer
    /// comes from bits 0 – 6, the sign from bit 7, the magnitude from bits 11 – 63 (no
    /// overlap). Each rejection loop is capped at [`RETRY_CAP`] attempts and then returns its
    /// last candidate, deterministically (constitution IV; P(cap) ≈ 8e-16 per sample).
    #[inline]
    fn gauss(&mut self) -> f64 {
        let mut x = 0.0;
        for _ in 0..RETRY_CAP {
            let r = self.next_u64();
            let i = (r & 0x7F) as usize;
            let neg = r & 0x80 != 0;
            let m = ((r >> 11) as f64 + 0.5) * TWO_POW_M53; // |u| in (0, 1]
            if m < ZIG_RATIO[i] && !self.forced() {
                let v = m * ZIG_X[i];
                return if neg { -v } else { v };
            }
            if i == 0 {
                return self.tail(neg);
            }
            let v = m * ZIG_X[i];
            x = if neg { -v } else { v };
            let f0 = exp(-0.5 * (ZIG_X[i] * ZIG_X[i] - v * v));
            let f1 = exp(-0.5 * (ZIG_X[i + 1] * ZIG_X[i + 1] - v * v));
            if f1 + self.uniform() * (f0 - f1) < 1.0 && !self.forced() {
                return x;
            }
        }
        x
    }

    /// The tail beyond R (Marsaglia 1964, as in ZIGNOR), capped like the main loop.
    fn tail(&mut self, neg: bool) -> f64 {
        let mut x = 0.0;
        for _ in 0..RETRY_CAP {
            x = ln(self.uniform()) / ZIG_TAIL_R;
            let y = ln(self.uniform());
            if -2.0 * y >= x * x && !self.forced() {
                break;
            }
        }
        let v = ZIG_TAIL_R - x;
        if neg { -v } else { v }
    }

    /// Whether the test hook forces every loop to its cap.
    #[inline(always)]
    fn forced(&self) -> bool {
        #[cfg(test)]
        {
            self.force_cap
        }
        #[cfg(not(test))]
        {
            false
        }
    }
}

/// Stage 2, the microphone amplifier (spec 012 FR-005, FR-006): the input high-pass (the 1 kHz
/// normalization folded in; A-046), the clipper (A-051), the output coupling and the roll-off
/// (A-047, lumped ahead of the limiter, A-053), in the device's order.
#[derive(Clone, Debug)]
pub(crate) struct MicAmp {
    hp_in: FirstOrder,
    hp_out: FirstOrder,
    lp: FirstOrder,
}

impl MicAmp {
    /// The amplifier at `host_rate_hz`, at rest. `None` for an unsupported rate.
    pub(crate) fn new(host_rate_hz: u32) -> Option<Self> {
        let i = rate_index(host_rate_hz)?;
        Some(Self {
            hp_in: FirstOrder::new(HP_IN[i]),   // A-046
            hp_out: FirstOrder::new(HP_OUT[i]), // A-047, A-053
            lp: FirstOrder::new(LP[i]),         // A-047, A-053
        })
    }

    /// One host sample. The clipper is linear until spec 012 US2 adds it (tasks.md T026).
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let v = self.hp_in.process(x);
        self.lp.process(self.hp_out.process(v))
    }

    /// Every section at rest (FR-009).
    pub(crate) fn reset(&mut self) {
        self.hp_in.reset();
        self.hp_out.reset();
        self.lp.reset();
    }
}

/// Where the microphone path is read (spec 012 FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MicStop {
    /// After stage 1.
    Capsule,
    /// After stage 2.
    Amp,
}

/// Stages 1 – 2 as the record path's front runs them (spec 012 FR-001, FR-007): each `None`
/// when its switch is off, decided at construction.
#[derive(Clone, Debug)]
pub(crate) struct MicFront {
    capsule: Option<MicCapsule>,
    amp: Option<MicAmp>,
}

impl MicFront {
    /// The front for validated `settings`-derived values, keyed to `(seed, 0)`.
    pub(crate) fn new(mic: &MicSettings, seed: u64, host_rate_hz: u32) -> Self {
        Self {
            capsule: if mic.capsule_enabled {
                MicCapsule::new(mic, seed, host_rate_hz)
            } else {
                None
            },
            amp: if mic.amp_enabled {
                MicAmp::new(host_rate_hz)
            } else {
                None
            },
        }
    }

    /// Stages 1 then 2, each skipped when off.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let x = match &mut self.capsule {
            Some(c) => c.process(x),
            None => x,
        };
        match &mut self.amp {
            Some(a) => a.process(x),
            None => x,
        }
    }

    /// Stages 1 – 2 up to `stop` (the taps, FR-008).
    #[inline]
    pub(crate) fn process_upto(&mut self, x: f64, stop: MicStop) -> f64 {
        match stop {
            MicStop::Amp => self.process(x),
            MicStop::Capsule => match &mut self.capsule {
                Some(c) => c.process(x),
                None => x,
            },
        }
    }

    /// Keys the hiss to `counter` from its beginning (FR-009); no effect with stage 1 off.
    pub(crate) fn rekey(&mut self, counter: u64) {
        if let Some(c) = &mut self.capsule {
            c.rekey(counter);
        }
    }

    /// The sections at rest and the hiss at the beginning of its current stream (FR-009).
    pub(crate) fn reset(&mut self) {
        if let Some(c) = &mut self.capsule {
            c.reset();
        }
        if let Some(a) = &mut self.amp {
            a.reset();
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // analysis in tests, not the DSP path (001 R-04)
mod tests {
    extern crate std;
    use super::*;
    use crate::settings::SUPPORTED_HOST_RATES;
    use std::vec::Vec;

    /// `tools/mic-chain/design_mic_chain.py`'s key vectors, printed 2026-10-16 (A-049):
    /// `(seed, counter, state, first three outputs)`.
    const KEY_VECTORS: [(u64, u64, [u64; 4], [u64; 3]); 3] = [
        (
            0,
            0,
            [
                0xeadb92fa864bd2b2,
                0x16abbcad2a042a56,
                0xb617bc4468079182,
                0x85eab382a43562f5,
            ],
            [0x2970d3955a0435d5, 0x982fce1af44bf199, 0x299e70eea13e0176],
        ),
        (
            1,
            0,
            [
                0x0bb2c342bed22952,
                0xb4212b9b02b833e9,
                0xb1a8b04b31479a8f,
                0x8c4bba6a2b7ac29a,
            ],
            [0xe227e9b8b51e2890, 0x144cfa3243d9db7d, 0xfb4568e8cfefb4a8],
        ),
        (
            0,
            1,
            [
                0xd508e2bfcd9b0eb3,
                0x891ca582008fd65c,
                0xfa001ad46ec3e23a,
                0xdd30b43c037a0dce,
            ],
            [0x52f16d4e0e742b7e, 0x44cd464c9e0d20bf, 0xa99c2247d2ebfbae],
        ),
    ];

    /// The script's section −3 dB points per rate (`SUPPORTED_HOST_RATES` order) as
    /// `[HP_in, HP_out, LP]` in Hz, NaN where the roll-off's point is above Nyquist; printed
    /// 2026-10-16 (A-046, A-047).
    const SECTION_M3DB_HZ: [[f64; 3]; 7] = [
        [733.6, 76.7, f64::NAN],
        [770.3, 76.6, f64::NAN],
        [813.3, 76.1, f64::NAN],
        [813.0, 77.2, 12927.0],
        [807.9, 77.7, 13046.0],
        [817.7, 76.8, 12945.0],
        [818.0, 76.8, 12946.0],
    ];

    /// The script reads the −3 dB points on a 20 000-point log grid (one step ≈ 0.05 %), so
    /// the pin is 0.1 % (engineering target, spec 012 T023).
    const M3DB_REL: f64 = 1e-3;

    fn capsule(seed: u64, rate: u32) -> MicCapsule {
        MicCapsule::new(&MicSettings::DEVICE, seed, rate).unwrap()
    }

    /// The −3 dB point of section `c` relative to `ref_sq` (its squared gain at the reference:
    /// Nyquist for a high-pass, DC for the roll-off): solves |H|² = ref_sq / 2 in cos ω.
    fn m3db(c: [f64; 3], ref_sq: f64, fs: f64) -> f64 {
        let (b0, b1, a1) = (c[0], c[1], c[2]);
        let k = ref_sq / 2.0;
        let cw = (k * (1.0 + a1 * a1) - b0 * b0 - b1 * b1) / (2.0 * b0 * b1 - 2.0 * k * a1);
        if cw < -1.0 {
            return f64::NAN; // above Nyquist
        }
        cw.acos() * fs / core::f64::consts::TAU
    }

    /// 012 T023 (A-049): `key` matches the script's two-bijection formula, and the first three
    /// xoshiro256++ outputs match; seed 0 gives a non-zero state.
    #[test]
    fn keying_matches_the_script_vectors() {
        for (seed, counter, state, outs) in KEY_VECTORS {
            let k = key(seed, counter);
            assert_eq!(k, state, "key({seed}, {counter})");
            assert!(k.iter().all(|&w| w != 0), "seed 0 does not degenerate");
            let mut c = capsule(seed, 48_000);
            c.rekey(counter);
            let got = [c.next_u64(), c.next_u64(), c.next_u64()];
            assert_eq!(got, outs, "outputs of key({seed}, {counter})");
        }
    }

    /// 012 T023 (A-049): a capsule is keyed `(seed, 0)` at construction; `rekey` and `reset`
    /// restart the stream from its beginning; distinct counters give distinct streams.
    #[test]
    fn rekey_and_reset_restart_the_stream() {
        let draw = |c: &mut MicCapsule| -> Vec<f64> { (0..64).map(|_| c.gauss()).collect() };
        let mut a = capsule(7, 44_100);
        let first = draw(&mut a);
        a.reset();
        assert_eq!(draw(&mut a), first, "reset restarts counter 0's stream");
        a.rekey(1);
        let second = draw(&mut a);
        assert_ne!(second, first, "counter 1 is another stream");
        a.reset();
        assert_eq!(draw(&mut a), second, "reset keeps the counter");
        let mut b = capsule(8, 44_100);
        assert_ne!(draw(&mut b), first, "seed 8 is another stream");
    }

    /// 012 T023 (A-049): forcing every loop to its cap returns a deterministic, finite value
    /// (the last candidate), the same from two capsules with the same key.
    #[test]
    fn retry_cap_returns_the_last_candidate_deterministically() {
        let mut a = capsule(3, 48_000);
        let mut b = capsule(3, 48_000);
        a.force_cap = true;
        b.force_cap = true;
        for _ in 0..2000 {
            let (x, y) = (a.gauss(), b.gauss());
            assert_eq!(x.to_bits(), y.to_bits());
            assert!(x.is_finite() && x != 0.0, "{x}");
        }
        // The cap path differs from the free one (it was taken).
        let mut free = capsule(3, 48_000);
        let mut forced = capsule(3, 48_000);
        forced.force_cap = true;
        let f: Vec<f64> = (0..16).map(|_| free.gauss()).collect();
        let g: Vec<f64> = (0..16).map(|_| forced.gauss()).collect();
        assert_ne!(f, g);
    }

    /// 012 T023: the draws have zero mean and unit variance within the sampling error of
    /// 200 000 draws (5 standard errors; engineering target), and stay finite.
    #[test]
    fn draws_are_standard_normal_in_the_moments() {
        let n = 200_000;
        let mut c = capsule(0, 48_000);
        let (mut s1, mut s2) = (0.0, 0.0);
        for _ in 0..n {
            let x = c.gauss();
            assert!(x.is_finite());
            s1 += x;
            s2 += x * x;
        }
        let mean = s1 / n as f64;
        let var = s2 / n as f64 - mean * mean;
        let se = 1.0 / (n as f64).sqrt();
        assert!(mean.abs() < 5.0 * se, "mean {mean}");
        assert!((var - 1.0).abs() < 5.0 * se * 2f64.sqrt(), "variance {var}");
    }

    /// 012 T023, US1 AS5 (release-only, 10⁸ draws): the count beyond ±4σ within ±5 % of the
    /// Gaussian expectation (6 334) and beyond ±5σ within ±40 % (57).
    #[test]
    #[ignore = "release-only: 10^8 draws (cargo test -p rr_dr60 --release stages::mic -- --ignored)"]
    fn tail_counts_on_1e8_draws() {
        let n = 100_000_000u64;
        let mut c = capsule(0, 48_000);
        let (mut c4, mut c5) = (0u64, 0u64);
        for _ in 0..n {
            let x = c.gauss().abs();
            c4 += u64::from(x > 4.0);
            c5 += u64::from(x > 5.0);
        }
        // P(|x| > 4) = 6.334e-5, P(|x| > 5) = 5.733e-7 (the normal distribution's tails).
        let (e4, e5) = (6.334e-5 * n as f64, 5.733e-7 * n as f64);
        std::println!("beyond 4σ: {c4} (expected {e4:.0}); beyond 5σ: {c5} (expected {e5:.1})");
        assert!((c4 as f64 - e4).abs() <= 0.05 * e4, "4σ count {c4}"); // US1 AS5: ±5 %
        assert!((c5 as f64 - e5).abs() <= 0.40 * e5, "5σ count {c5}"); // US1 AS5: ±40 %
    }

    /// 012 T023 (A-048): the gain is σ_rate at offset 0 exactly and moves by the offset.
    #[test]
    fn gain_is_sigma_times_the_offset() {
        for (i, &rate) in SUPPORTED_HOST_RATES.iter().enumerate() {
            assert_eq!(
                capsule(0, rate).gain,
                HISS_SIGMA[i],
                "{rate} Hz at offset 0"
            );
            let mut m = MicSettings::DEVICE;
            m.hiss_offset_db = -12.0;
            let g = MicCapsule::new(&m, 0, rate).unwrap().gain;
            let want = HISS_SIGMA[i] * 10f64.powf(-12.0 / 20.0);
            assert!(
                (g - want).abs() <= 1e-12 * want,
                "{rate} Hz at −12 dB: {g} vs {want}"
            );
        }
    }

    /// 012 T023 (A-046, A-047): each realized section's −3 dB point equals the script's print,
    /// and the cascade has unity gain at 1 kHz.
    #[test]
    fn section_corners_match_the_script() {
        for (i, &rate) in SUPPORTED_HOST_RATES.iter().enumerate() {
            let fs = f64::from(rate);
            let nyq = |c: [f64; 3]| ((c[0] - c[1]) / (1.0 - c[2])).powi(2);
            let got = [
                m3db(HP_IN[i], nyq(HP_IN[i]), fs),
                m3db(HP_OUT[i], nyq(HP_OUT[i]), fs),
                m3db(LP[i], 1.0, fs),
            ];
            for (j, (&g, &w)) in got.iter().zip(&SECTION_M3DB_HZ[i]).enumerate() {
                if w.is_nan() {
                    assert!(
                        g.is_nan(),
                        "{rate} Hz section {j}: {g} Hz, the script: above Nyquist"
                    );
                } else {
                    assert!(
                        (g - w).abs() <= M3DB_REL * w,
                        "{rate} Hz section {j}: {g} Hz vs the script's {w}"
                    );
                }
            }
            // |H(1 kHz)| of the cascade is 1 (FR-002, FR-005: the normalization in HP_in).
            let z = |c: [f64; 3]| {
                let w = core::f64::consts::TAU * 1000.0 / fs;
                let (re, im) = (w.cos(), -w.sin());
                let num = ((c[0] + c[1] * re).powi(2) + (c[1] * im).powi(2)).sqrt();
                let den = ((1.0 + c[2] * re).powi(2) + (c[2] * im).powi(2)).sqrt();
                num / den
            };
            let g1k = z(HP_IN[i]) * z(HP_OUT[i]) * z(LP[i]);
            assert!((g1k - 1.0).abs() < 1e-9, "{rate} Hz: |H(1 kHz)| = {g1k}");
        }
    }

    /// 012 T023 (FR-007, FR-010): both stages off pass the input bit for bit; the amplifier
    /// alone is a filter at rest after `reset`.
    #[test]
    fn bypass_is_the_identity_and_reset_is_rest() {
        let x: Vec<f64> = (0..512)
            .map(|n| ((n * 37 % 101) as f64 - 50.0) / 61.0)
            .collect();
        let mut off = MicFront::new(&MicSettings::BYPASSED, 0, 48_000);
        for &v in &x {
            assert_eq!(off.process(v).to_bits(), v.to_bits());
            assert_eq!(off.process_upto(v, MicStop::Capsule).to_bits(), v.to_bits());
        }
        let mut amp_only = MicSettings::BYPASSED;
        amp_only.amp_enabled = true;
        let mut f = MicFront::new(&amp_only, 0, 48_000);
        let a: Vec<f64> = x.iter().map(|&v| f.process(v)).collect();
        assert_ne!(a, x, "the amplifier filters");
        f.reset();
        let b: Vec<f64> = x.iter().map(|&v| f.process(v)).collect();
        assert_eq!(a, b, "reset puts the sections at rest");
        // Stage 1 off at the capsule tap: the input unchanged even with the amplifier on.
        for &v in &x {
            assert_eq!(f.process_upto(v, MicStop::Capsule).to_bits(), v.to_bits());
        }
    }

    /// 012 T023 (FR-003, FR-009): with the capsule on, the front is the input plus the hiss
    /// stream, identical across two fronts and restarted by `reset`.
    #[test]
    fn capsule_adds_the_stream() {
        let mut m = MicSettings::DEVICE;
        m.amp_enabled = false;
        let mut f = MicFront::new(&m, 5, 16_000);
        let mut c = capsule(5, 16_000);
        for n in 0..1000 {
            let v = f64::from(n as u16) * 1e-3;
            let want = v + c.gain * c.gauss();
            assert_eq!(f.process(v).to_bits(), want.to_bits());
        }
        f.reset();
        c.reset();
        assert_eq!(f.process(0.0).to_bits(), (c.gain * c.gauss()).to_bits());
        f.rekey(9);
        c.rekey(9);
        assert_eq!(f.process(0.0).to_bits(), (c.gain * c.gauss()).to_bits());
    }
}
