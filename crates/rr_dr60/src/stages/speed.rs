//! The playback speed control (spec 013; A-054's mechanism (b)): a resampling of the stored
//! 6 kHz device stream at the device rate, between the speech decoder (stage 9) and the codec's
//! reconstruction filter (stage 10), run only by the player.
//!
//! Level k/5 (k = 3 … 7, levels 1 … 5) plays at speed r = k/5. Device output n sits at the
//! **place** p_n = n·k/5 stored samples, kept as an exact count of fifths (`p5`; research R-02).
//! At a place p5 = 5i + ph:
//! - level 3 (k = 5): the output is the stored sample x[i], widened exactly; nothing is read
//!   ahead (spec 013 FR-006: bit-identical to version 0.12);
//! - levels 1, 2, 4, 5: the output is Σ_{m=0}^{56} c_k[ph][m] · x[i + 28 − m], with c_k[ph][m]
//!   = h_k[ph + 5m] phase ph of a 281-tap Kaiser prototype h_k at the 30 kHz prototype rate,
//!   centred on tap 140, so the response is centred on the place and adds no latency (FR-003,
//!   FR-008; research R-03). The sum is four partial sums in a fixed order (research R-03).
//!
//! After the recording's own part (places before K) the stage plays its own **tail** (the
//! outputs whose kernel still reaches a stored sample), then returns silence; the player plays
//! the unchanged drain after the tail (FR-005; research R-04). The level latches when the place
//! reaches K; a change into level 3 waits until the place is whole (FR-007; research R-02).
//! Every figure is pinned to `tools/playback-speed/speed_design.py`; modeled on the documents,
//! not measured on a unit.

use alloc::boxed::Box;
use alloc::vec;

use crate::resample::design::{kaiser_beta, kaiser_lowpass, polyphase_split};

/// Fifths of a stored sample: the place's resolution and the prototype's upsampling factor
/// (spec 013 FR-002: every ratio is k/5).
pub(crate) const PHASES: usize = 5;
/// The look-ahead D in stored samples: half the prototype's length (research R-03).
pub(crate) const D: usize = crate::speed::LOOKAHEAD_SAMPLES as usize;
/// The prototype's length, 10·D + 1 = 281 taps at the 30 kHz prototype rate (research R-03).
pub(crate) const TAPS: usize = 10 * D + 1; // engineering target (013 R-03)
/// Taps per phase, 2·D + 1 = 57 (the last tap of phases 1 – 4 is zero padding).
pub(crate) const PER_PHASE: usize = 2 * D + 1;
/// The prototype rate, Hz: 5 × the 6 kHz device rate (A-026).
const PROTO_RATE_HZ: f64 = 30_000.0;
/// The Kaiser design attenuation, dB: FR-004's 60 dB with margin (engineering target, 013 R-03).
const ATTEN_DB: f64 = 70.0;
/// The ring of stored samples: a power of two above the 57-tap window and the two samples an
/// output advances at most (research R-04).
const RING: usize = 128; // engineering target (013 R-04)
/// The level whose ratio is 1 (k = 5): the identity.
const K_IDENTITY: u32 = 5; // 013 FR-002: ratio 5/5

/// The prototype's band edges for numerator k (k ≠ 5), in the stored stream's frequencies, Hz
/// (research R-03): the slow levels pass to 2550 Hz (the record band's top) and stop from
/// 3100 Hz (the image 6000 − f of a stored 2900 Hz tone); the fast levels pass to 2250 ÷ r and
/// stop from 3000 ÷ r (the device Nyquist frequency after the move).
pub(crate) fn edges_hz(k: u32) -> (f64, f64) {
    if k < K_IDENTITY {
        (2550.0, 3100.0) // 013 R-03 (FR-004 (a), (b))
    } else {
        let r = f64::from(k) / 5.0;
        (2250.0 / r, 3000.0 / r) // 013 R-03 (FR-004 (a) – (c))
    }
}

/// The prototype's cutoff for numerator k, in cycles per prototype sample: halfway between the
/// band edges (research R-03; the script prints it for the pins).
pub(crate) fn cutoff(k: u32) -> f64 {
    let (pass, stop) = edges_hz(k);
    (pass + stop) / 2.0 / PROTO_RATE_HZ
}

/// The polyphase bank for numerator k (k ≠ 5): 5 phases × 57 taps, phase-major, each phase
/// summing to 1 (`design::polyphase_split`). Allocates; construction time only (FR-010).
pub(crate) fn design_bank(k: u32) -> Box<[f64]> {
    let h = kaiser_lowpass(TAPS, cutoff(k), kaiser_beta(ATTEN_DB));
    let (bank, per_phase) = polyphase_split(&h, PHASES);
    debug_assert_eq!(per_phase, PER_PHASE);
    bank
}

/// T(K, k): the device outputs after a constant-level recording's own part whose kernel still
/// reaches the last stored sample K − 1 (research R-04): the place p5 = 5i + ph reads down to
/// x[i − 28] for ph = 0 and x[i − 27] for ph > 0 (phase ph's last tap is zero padding). 0 at
/// level 3. At most `speed::MAX_TAIL_SAMPLES` (46, level 1).
pub(crate) fn tail_len(stored: u64, k: u32) -> u64 {
    if k == K_IDENTITY || stored == 0 {
        return 0;
    }
    let k64 = u64::from(k);
    let own = (5 * stored).div_ceil(k64);
    let mut n = own;
    loop {
        let p5 = n * k64;
        let (i, ph) = (p5 / 5, p5 % 5);
        let lowest = i as i64 - D as i64 + i64::from(ph != 0);
        if lowest > stored as i64 - 1 {
            return n - own;
        }
        n += 1;
    }
}

/// The numerator k of `level` (1 … 5 → 3 … 7).
#[inline]
pub(crate) fn numerator(level: u8) -> u32 {
    crate::speed::RATIO_NUMERATORS[usize::from(level - 1)]
}

/// What the speed control produced for one device output (data-model.md › SpeedStage).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SpeedOut {
    /// A sample of the recording's own part (place before K).
    Own(f64),
    /// A sample of the tail (place at or past K, the kernel still reaching a stored sample).
    Tail(f64),
    /// Past the tail: silence (+0.0).
    Silence,
}

/// The player's speed control (data-model.md › SpeedStage). Built at construction; `reset` at
/// every playback start; no allocation afterwards (FR-010).
#[derive(Clone, Debug)]
pub(crate) struct SpeedStage {
    /// Banks for k = 3, 4, 6, 7 (levels 1 and 2 share a prototype; research R-03).
    banks: [Box<[f64]>; 4],
    /// The stored samples pulled, by index mod `RING`.
    ring: Box<[f32]>,
    /// Stored samples pulled from the source so far (at most K).
    pulled: u64,
    /// K: the recording's stored device samples, known at the playback's start.
    stored: u64,
    /// The place of the next output, in fifths of a stored sample.
    p5: u64,
    /// The numerator computing the outputs.
    active: u32,
    /// The host's numerator as of the current play call.
    requested: u32,
    /// The place reached K: no level change applies (FR-005, FR-007).
    latched: bool,
    /// The place of the last output computed, in fifths.
    last_p5: Option<u64>,
    /// `Own` outputs so far (K′).
    own: u64,
    /// `Tail` outputs so far (T).
    tail: u64,
    /// K′ + T, fixed when the place reaches K (the transport's total, research R-05).
    final_count: Option<u64>,
    /// The test hooks' mutation (research R-12).
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: crate::__test_hooks::SpeedMutation,
}

impl SpeedStage {
    /// A stage at `level`, its banks designed (allocates; construction time only).
    pub(crate) fn new(level: u8) -> Self {
        let slow = design_bank(3); // levels 1 and 2 share the prototype (013 R-03)
        let k = numerator(level);
        Self {
            banks: [slow.clone(), slow, design_bank(6), design_bank(7)],
            ring: vec![0.0f32; RING].into_boxed_slice(),
            pulled: 0,
            stored: 0,
            p5: 0,
            active: k,
            requested: k,
            latched: false,
            last_p5: None,
            own: 0,
            tail: 0,
            final_count: None,
            #[cfg(feature = "__test-hooks")]
            mutation: crate::__test_hooks::SpeedMutation::default(),
        }
    }

    /// Back to rest at the start of a recording of `stored` device samples, at `level` (FR-007:
    /// a playback start). No allocation.
    pub(crate) fn reset(&mut self, level: u8, stored: u64) {
        let k = numerator(level);
        self.ring.fill(0.0);
        self.pulled = 0;
        self.stored = stored;
        self.p5 = 0;
        self.active = k;
        self.requested = k;
        self.latched = false;
        self.last_p5 = None;
        self.own = 0;
        self.tail = 0;
        self.final_count = None;
        if stored == 0 {
            self.latch(); // an empty recording has no own part (not reachable through the API)
        }
    }

    /// The host's level for the outputs computed from now on (spec 013 FR-007; applied by
    /// `next` under the latch and the deferred-switch rules).
    pub(crate) fn request(&mut self, level: u8) {
        self.requested = numerator(level);
    }

    /// The place reported by a play call (FR-009): ⌊place⌋ of the last output computed; K once
    /// the place has reached the recording's end; 0 before any output.
    pub(crate) fn place(&self) -> u64 {
        if self.latched {
            self.stored
        } else {
            self.last_p5.map_or(0, |p5| p5 / 5)
        }
    }

    /// K′ + T, the device outputs before the drain, once the place has reached K (FR-005).
    pub(crate) fn final_count(&self) -> Option<u64> {
        self.final_count
    }

    /// The place of the next output, in fifths (a test hook's reading).
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn place_fifths(&self) -> u64 {
        self.p5
    }

    /// The look-ahead in effect (a mutant moves it; research R-12).
    #[inline]
    fn lookahead(&self) -> i64 {
        #[cfg(feature = "__test-hooks")]
        {
            D as i64 + i64::from(self.mutation.lookahead_delta)
        }
        #[cfg(not(feature = "__test-hooks"))]
        {
            D as i64
        }
    }

    /// Pulls stored samples until index `j` is available (or all K are).
    #[inline]
    fn pull_through(&mut self, j: i64, src: &mut impl FnMut() -> Option<f32>) {
        while self.pulled < self.stored && (self.pulled as i64) <= j {
            let v = src().unwrap_or(0.0); // the source holds K samples (always Some here)
            self.ring[self.pulled as usize & (RING - 1)] = v;
            self.pulled += 1;
        }
    }

    /// The stored sample at index `j`: 0 outside 0 … K − 1 (it must have been pulled).
    #[inline]
    fn x(&self, j: i64) -> f64 {
        if j < 0 || (j as u64) >= self.pulled {
            0.0
        } else {
            f64::from(self.ring[j as usize & (RING - 1)])
        }
    }

    /// The dot product of phase `ph` of the active bank with x[base − m], m = 0 … 56, as four
    /// partial sums in a fixed order (research R-03): s_q over m ≡ q (mod 4), m ascending, then
    /// (s₀ + s₁) + (s₂ + s₃). Inside the stored samples it reads the ring without bounds tests;
    /// at the edges, through `x` (zeros outside 0 … K − 1). Both paths add the same terms in the
    /// same order, so they are bit-identical.
    #[inline]
    fn dot(&self, ph: usize, base: i64) -> f64 {
        let c = &self.bank(self.active)[ph * PER_PHASE..(ph + 1) * PER_PHASE];
        let (mut s0, mut s1, mut s2, mut s3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        if base >= (PER_PHASE - 1) as i64 && (base as u64) < self.pulled {
            let b = base as usize;
            let r = &self.ring;
            let at = |m: usize| f64::from(r[(b - m) & (RING - 1)]);
            let mut m = 0;
            while m + 4 < PER_PHASE {
                s0 += c[m] * at(m);
                s1 += c[m + 1] * at(m + 1);
                s2 += c[m + 2] * at(m + 2);
                s3 += c[m + 3] * at(m + 3);
                m += 4;
            }
            s0 += c[PER_PHASE - 1] * at(PER_PHASE - 1); // m = 56 ≡ 0 (mod 4)
        } else {
            let mut s = [0.0f64; 4];
            for (m, &cm) in c.iter().enumerate() {
                s[m & 3] += cm * self.x(base - m as i64);
            }
            (s0, s1, s2, s3) = (s[0], s[1], s[2], s[3]);
        }
        (s0 + s1) + (s2 + s3)
    }

    /// The bank for numerator k (k ≠ 5).
    #[inline]
    fn bank(&self, k: u32) -> &[f64] {
        match k {
            3 => &self.banks[0],
            4 => &self.banks[1],
            6 => &self.banks[2],
            _ => &self.banks[3],
        }
    }

    /// The step in fifths for numerator k (a mutant changes level 5's; research R-12).
    #[inline]
    fn step(&self, k: u32) -> u64 {
        #[cfg(feature = "__test-hooks")]
        if k == 7 {
            if let Some(s) = self.mutation.ratio_k7 {
                return u64::from(s);
            }
        }
        u64::from(k)
    }

    /// The latch (FR-005: as soon as the place reaches K): the level is fixed and K′ + T is
    /// known (the tail still to play counted from the current place and level).
    #[inline]
    fn latch(&mut self) {
        if self.latched || self.p5 < 5 * self.stored {
            return;
        }
        self.latched = true;
        #[cfg(feature = "__test-hooks")]
        let no_tail = self.mutation.no_tail;
        #[cfg(not(feature = "__test-hooks"))]
        let no_tail = false;
        let mut remaining = 0u64;
        if self.active != K_IDENTITY && !no_tail {
            let mut p5 = self.p5;
            loop {
                let (i, ph) = ((p5 / 5) as i64, p5 % 5);
                if i - D as i64 + i64::from(ph != 0) > self.stored as i64 - 1 {
                    break;
                }
                remaining += 1;
                p5 += self.step(self.active);
            }
        }
        self.final_count = Some(self.own + self.tail + remaining);
    }

    /// Applies the requested level under the deferred-switch rule (FR-007).
    #[inline]
    fn apply_request(&mut self) {
        if self.latched || self.requested == self.active {
            return;
        }
        if self.requested == K_IDENTITY {
            #[cfg(feature = "__test-hooks")]
            if self.mutation.snap_into_level_3 && self.p5 % 5 != 0 {
                self.p5 = self.p5.div_ceil(5) * 5; // the mutant: a forward snap
            }
            if self.p5 % 5 == 0 {
                self.active = K_IDENTITY;
            }
        } else {
            self.active = self.requested;
        }
    }

    /// The next device output (data-model.md › the state machine). `src` returns the next
    /// stored device sample (it is called at most K times per playback). Bounded work: at most
    /// 57 multiply-adds and at most 2 + 29 pulls (FR-010).
    #[inline]
    pub(crate) fn next(&mut self, src: &mut impl FnMut() -> Option<f32>) -> SpeedOut {
        // The steady identity (level 3, no change pending, nothing read ahead): one stored
        // sample per output, exactly what the general path computes (FR-006), without its tests.
        #[cfg(feature = "__test-hooks")]
        let plain = !self.mutation.lookahead_at_level_3;
        #[cfg(not(feature = "__test-hooks"))]
        let plain = true;
        let i = self.p5 / 5;
        if plain
            && self.active == K_IDENTITY
            && self.requested == K_IDENTITY
            && !self.latched
            && self.pulled == i
            && i < self.stored
        {
            let v = src().unwrap_or(0.0); // the source holds K samples (always Some here)
            self.ring[i as usize & (RING - 1)] = v;
            self.pulled = i + 1;
            self.last_p5 = Some(self.p5);
            self.p5 += 5;
            self.own += 1;
            self.latch();
            return SpeedOut::Own(f64::from(v));
        }
        self.next_general(src)
    }

    /// The general step: the latch, a pending change, the look-ahead, the tail.
    fn next_general(&mut self, src: &mut impl FnMut() -> Option<f32>) -> SpeedOut {
        let i = (self.p5 / 5) as i64;
        self.pull_through(i, src);
        #[cfg(feature = "__test-hooks")]
        let latch_after = self.mutation.latch_after_switch;
        #[cfg(not(feature = "__test-hooks"))]
        let latch_after = false;
        if !latch_after {
            self.latch();
        }
        self.apply_request();
        if latch_after {
            self.latch();
        }
        let ph = (self.p5 % 5) as usize;
        let own = self.p5 < 5 * self.stored;
        if !own && self.final_count.is_some_and(|f| self.own + self.tail >= f) {
            return SpeedOut::Silence;
        }
        let y = if self.active == K_IDENTITY {
            #[cfg(feature = "__test-hooks")]
            if self.mutation.lookahead_at_level_3 {
                self.pull_through(i + D as i64, src); // the mutant: look-ahead at level 3
            }
            self.x(i)
        } else {
            let base = i + self.lookahead();
            self.pull_through(base, src);
            self.dot(ph, base)
        };
        self.last_p5 = Some(self.p5);
        self.p5 += self.step(self.active);
        let out = if own {
            self.own += 1;
            SpeedOut::Own(y)
        } else {
            self.tail += 1;
            SpeedOut::Tail(y)
        };
        if !latch_after {
            self.latch(); // the total is known as soon as the place reaches K (research R-05)
        }
        out
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// A stage fed from `x`, run to its first `Silence`; the outputs (own and tail).
    fn run(level: u8, x: &[f32]) -> (Vec<f64>, u64, u64) {
        let mut st = SpeedStage::new(level);
        st.reset(level, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        let mut out = Vec::new();
        while let SpeedOut::Own(v) | SpeedOut::Tail(v) = st.next(&mut src) {
            out.push(v);
        }
        (out, st.own, st.tail)
    }

    /// 013 T012 (research R-03): the banks' cutoff, energy and second moment against the
    /// script's print (2026-10-17), within 1e-12; each phase sums to 1 within 1e-15.
    #[test]
    fn banks_match_the_script() {
        // speed_design.py, printed 2026-10-17: (k, cutoff, energy, moment)
        let pins = [
            (
                3,
                0.094166666666667,
                4.627700863476776,
                -1.029165517199091e-04,
            ),
            (
                4,
                0.094166666666667,
                4.627700863476776,
                -1.029165517199091e-04,
            ),
            (
                6,
                0.072916666666667,
                3.565198698361072,
                -1.304399895794038e-04,
            ),
            (7, 0.0625, 3.044048562407605, 8.723903669859059e-05),
        ];
        for (k, cut, energy, moment) in pins {
            assert!(
                (cutoff(k) - cut).abs() < 1e-12,
                "k={k} cutoff {}",
                cutoff(k)
            );
            let b = design_bank(k);
            assert_eq!(b.len(), PHASES * PER_PHASE);
            let (mut e, mut mo) = (0.0, 0.0);
            for ph in 0..PHASES {
                let row = &b[ph * PER_PHASE..(ph + 1) * PER_PHASE];
                let sum: f64 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-15, "k={k} phase {ph} sums to {sum}"); // 013 FR-003
                for (m, &c) in row.iter().enumerate() {
                    let t = (ph as f64 + 5.0 * m as f64 - 140.0) / 140.0;
                    e += c * c;
                    mo += c * t * t;
                }
            }
            assert!((e - energy).abs() < 1e-12, "k={k} energy {e}"); // 013 R-03 pin
            assert!((mo - moment).abs() < 1e-12, "k={k} moment {mo}"); // 013 R-03 pin
        }
    }

    /// 013 T012 (research R-03): a regression guard on the designed banks' bit patterns (pinned
    /// from this Rust; a change of the designer's code must show here, not only in the goldens).
    #[test]
    fn bank_bits_digest() {
        let digest = |k: u32| {
            design_bank(k)
                .iter()
                .fold(0xcbf2_9ce4_8422_2325u64, |h, c| {
                    (h ^ c.to_bits()).wrapping_mul(0x0100_0000_01b3) // FNV-1a over the words
                })
        };
        let got = [digest(3), digest(6), digest(7)];
        std::eprintln!("bank digests: {got:#018x?}");
        assert_eq!(got, BANK_DIGESTS);
    }

    /// The banks' FNV-1a digests, pinned 2026-10-17 (013 T012).
    const BANK_DIGESTS: [u64; 3] = [
        0xaf05_1118_b444_11ac,
        0xc662_be6e_98d7_be62,
        0x5339_708e_ee8b_b40d,
    ];

    /// 013 T012 (FR-005; research R-02, R-04): ⌈5K ÷ k⌉ own outputs (the script's table) and
    /// the tail's worst case over K mod k (46 / 34 / 23 / 20, the script's print).
    #[test]
    fn lengths_and_tails() {
        for (k, own_120, worst) in [(3u32, 200, 46u64), (4, 150, 34), (6, 100, 23), (7, 86, 20)] {
            let level = (k - 2) as u8;
            let (_, own, _) = run(level, &[0.5f32; 120]);
            assert_eq!(own, own_120, "k={k}"); // the script: K = 120
            let w = (1000..1000 + 5 * u64::from(k))
                .map(|kk| tail_len(kk, k))
                .max()
                .unwrap();
            assert_eq!(w, worst, "k={k} tail");
            assert!(worst <= u64::from(crate::speed::MAX_TAIL_SAMPLES));
            // The stage's own tail equals the rule.
            for kk in [1u64, 2, 27, 28, 29, 57, 1001, 1003] {
                let x = std::vec![0.25f32; kk as usize];
                let (_, own, tail) = run(level, &x);
                assert_eq!(own, (5 * kk).div_ceil(u64::from(k)), "k={k} K={kk} own");
                assert_eq!(tail, tail_len(kk, k), "k={k} K={kk} tail");
            }
        }
        // Level 3: K outputs, no tail, the samples unaltered.
        let x: Vec<f32> = (0..300).map(|n| (n as f32 * 0.01).fract() - 0.5).collect();
        let (y, own, tail) = run(3, &x);
        assert_eq!((own, tail), (300, 0));
        assert!(y.iter().zip(&x).all(|(&a, &b)| a == f64::from(b)));
    }

    /// 013 T012 (FR-006): at level 3 nothing is read ahead: one pull per output.
    #[test]
    fn no_lookahead_at_level_3() {
        let mut st = SpeedStage::new(3);
        st.reset(3, 1000);
        let mut pulls = 0u64;
        let mut src = || {
            pulls += 1;
            Some(0.1f32)
        };
        for _ in 0..50 {
            let _ = st.next(&mut src);
        }
        assert_eq!(pulls, 50);
    }

    /// 013 T012 (FR-007; research R-02): the switch into level 3 waits at most 4 outputs.
    #[test]
    fn switch_into_level_3_waits_for_a_whole_place() {
        for level in [1u8, 2, 4, 5] {
            for start in 0..5 {
                let mut st = SpeedStage::new(level);
                st.reset(level, 1000);
                let mut src = || Some(0.0f32);
                for _ in 0..start {
                    let _ = st.next(&mut src);
                }
                st.request(3);
                let mut waited = 0;
                while st.active != K_IDENTITY {
                    let _ = st.next(&mut src);
                    waited += 1;
                    assert!(waited <= 5, "level {level}");
                }
                assert!(waited <= 4 + 1, "level {level}: {waited}"); // the switching output itself
                assert_eq!(st.p5 % 5, 0);
            }
        }
    }

    /// 013 T012 (FR-003): DC passes every level within 1e-15 once the kernel is filled.
    #[test]
    fn dc_passes() {
        for level in [1u8, 2, 4, 5] {
            let x = std::vec![0.5f32; 400];
            let (y, _, _) = run(level, &x);
            for &v in &y[100..250] {
                assert!((v - 0.5).abs() < 1e-15, "level {level}: {v}");
            }
        }
    }
}
