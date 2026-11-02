//! The band filter, spec 024's first component (D4; research R-02, R-03): one second-order
//! topology-preserving state-variable section (Simper/Zavalishin) followed by a gain, in `f64`.
//!
//! ```text
//! g = sin(π f / r) / cos(π f / r)   (detmath; f the applied frequency, ≤ 0.45 r)
//! k = 1 / Q;  a1 = 1 / (1 + g (g + k));  a2 = g a1;  a3 = g a2
//! v3 = x − s2;  v1 = a1 s1 + a2 v3;  v2 = s2 + a2 s1 + a3 v3;  s1 = 2 v1 − s1;  s2 = 2 v2 − s2
//! y = gain · (m0 x + m1 (k v1) + m2 v2)
//! ```
//!
//! The four types are **constant mixes** `(m0, m1, m2)` of one state's signals `(x, k·v1, v2)`
//! (plan R5): low-pass `(0, 0, 1)`, band-pass `(0, 1, 0)` (a 0 dB peak), high-pass
//! `(1, −1, −1)`, notch `(1, −1, 0)`. The state update does not depend on the type, so a type
//! change blends two mixes of the one state, which equals two sections fed the same input bit
//! for bit. A parameter landing moves along a 20 ms smoothstep in the parameter's own scale
//! (log of the applied frequency, log-Q, dB), recomputing the coefficients per sample only while
//! a transition runs (D3). An addition of the emulator, not a part of the original recorder.

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60_detmath as dm;

/// The frequency the engine applies at most, as a fraction of the host rate (spec 024 D4).
pub(crate) const CEILING: f64 = 0.45; // engineering target (024 D4)
/// The state is flushed to zero below this magnitude, both values together (the rule of
/// `biquad.rs` and `first_order.rs`; hardware plan review B14).
const FLUSH: f64 = 1e-30; // engineering target (024 D4; 001 R-06's flush rule)

/// The four types' mixes on `(x, k·v1, v2)` (research R-03): low-pass, high-pass, band-pass,
/// notch, by the type's value (spec 024 D4: 0 low-pass, 1 high-pass, 2 band-pass, 3 notch).
pub(crate) const MIXES: [[f64; 3]; 4] = [
    [0.0, 0.0, 1.0],   // low-pass (024 D4)
    [1.0, -1.0, -1.0], // high-pass (024 D4)
    [0.0, 1.0, 0.0],   // band-pass, 0 dB peak (024 D4)
    [1.0, -1.0, 0.0],  // notch (024 D4)
];

/// The number of types.
pub(crate) const TYPE_COUNT: u8 = 4; // engineering target (024 D4): the four types

/// The smoothstep at `u` (the pinned expression of spec 019; spec 011's shape).
#[inline]
fn smoothstep(u: f64) -> f64 {
    u * u * (3.0 - 2.0 * u)
}

/// A parameter's transition in its own scale (research R-02): `n` counts the samples of the
/// transition (1 at P), the value at `n` is `from + w(n/N)·(to − from)`, the target itself at
/// `n = N`; inactive at rest (`cur == to`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ramp {
    from: f64,
    to: f64,
    cur: f64,
    n: u32,
    active: bool,
}

impl Ramp {
    fn at(v: f64) -> Self {
        Self {
            from: v,
            to: v,
            cur: v,
            n: 0,
            active: false,
        }
    }

    /// Starts a transition from the current value (spec 011's restart rule) or sets it at once.
    fn land(&mut self, to: f64, at_once: bool, from_target: bool) {
        if at_once {
            *self = Self::at(to);
            return;
        }
        if !self.active && to.to_bits() == self.cur.to_bits() {
            // A landing at the value in force changes nothing (hardware post-implementation B4:
            // a 20 ms ramp from x to x would read `exp(ln f)`, not `f`).
            self.to = to;
            return;
        }
        // The `restart_from_target` mutant restarts from the previous target.
        self.from = if from_target { self.to } else { self.cur };
        self.to = to;
        self.n = 0;
        self.active = true;
    }

    /// Advances one sample; the value for this sample.
    #[inline]
    fn step(&mut self, len: u32) -> f64 {
        self.n += 1;
        if self.n >= len {
            self.cur = self.to;
            self.active = false;
        } else {
            let w = smoothstep(f64::from(self.n) / f64::from(len));
            self.cur = self.from + w * (self.to - self.from);
        }
        self.cur
    }

    /// Ends the transition at its target.
    fn finish(&mut self) {
        *self = Self::at(self.to);
    }
}

/// The type blend (research R-03): from a constant mix to a type's mix over N samples; at both
/// ends the branch is selected, never computed (spec 019's lesson).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Blend {
    from: [f64; 3],
    /// The blend's origin type, while `from` is a type's own mix (a landing back to it reverses).
    origin: Option<u8>,
    k: u32,
    active: bool,
}

/// Deliberate defects for the spec 024 mutation tests (research › The six mutants); `Default`
/// is the committed behavior.
#[cfg(feature = "__test-hooks")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BandFilterMutation {
    /// The coefficients jump at P (every landing at once).
    pub param_step: bool,
    /// The mix switches at P (no type blend).
    pub type_instant: bool,
    /// The frequency ramp runs in Hz, not in log-frequency.
    pub freq_linear: bool,
    /// A second landing restarts from the previous target, not from the current value.
    pub restart_from_target: bool,
}

/// One band filter: the state, the cached coefficients, three parameter ramps and the type
/// blend. Fixed size; no allocation.
#[derive(Clone, Debug)]
pub(crate) struct BandFilter {
    rate: f64,
    /// N, the transition's length in host samples (20 ms; spec 011's count).
    len: u32,
    ceiling: f64,
    s1: f64,
    s2: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    k: f64,
    gain: f64,
    ftype: u8,
    /// log of the applied frequency (or Hz under the `freq_linear` mutant).
    freq: Ramp,
    /// The applied frequency's target in Hz, exact (at rest the section uses it, not
    /// `exp(ln f)`, which differs in the last bits).
    freq_hz_to: f64,
    /// log-Q.
    q: Ramp,
    /// Q's target, exact (as `freq_hz_to`).
    q_to: f64,
    /// The gain in dB.
    gain_db: Ramp,
    blend: Blend,
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: BandFilterMutation,
}

impl BandFilter {
    /// A filter at `rate` with the store's band-filter values `[type, frequency, Q, gain dB]`,
    /// the state at zero. `len` is the transition's length in samples.
    pub(crate) fn new(rate: u32, len: u32, values: [f32; 4]) -> Self {
        let rate = f64::from(rate);
        let mut f = Self {
            rate,
            len: len.max(2),
            ceiling: CEILING * rate,
            s1: 0.0,
            s2: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            k: 0.0,
            gain: 1.0,
            ftype: 0,
            freq: Ramp::at(0.0),
            freq_hz_to: 1000.0,
            q: Ramp::at(0.0),
            q_to: 1.0,
            gain_db: Ramp::at(0.0),
            blend: Blend {
                from: MIXES[0],
                origin: None,
                k: 0,
                active: false,
            },
            #[cfg(feature = "__test-hooks")]
            mutation: BandFilterMutation::default(),
        };
        f.set_all(values);
        f
    }

    /// Sets every parameter at once (construction, the adopt rule, the idle setter's snap).
    pub(crate) fn set_all(&mut self, values: [f32; 4]) {
        for (i, v) in values.into_iter().enumerate() {
            self.land(i, v, true);
        }
    }

    /// The applied frequency for the store's requested value (D4: min(f, 0.45 × rate)).
    #[inline]
    pub(crate) fn applied_frequency(&self, requested: f32) -> f64 {
        let f = f64::from(requested);
        if f > self.ceiling { self.ceiling } else { f }
    }

    fn freq_scale(&self, hz: f64) -> f64 {
        #[cfg(feature = "__test-hooks")]
        if self.mutation.freq_linear {
            return hz;
        }
        dm::ln(hz)
    }

    fn freq_hz(&self, v: f64) -> f64 {
        #[cfg(feature = "__test-hooks")]
        if self.mutation.freq_linear {
            return v;
        }
        dm::exp(v)
    }

    /// A parameter lands (research R-02, R-03): `index` 0 the type, 1 the frequency, 2 Q, 3 the
    /// gain in dB, with the store's value; `at_once` sets it without a transition (the slot
    /// empty, a stream's first input sample, construction).
    pub(crate) fn land(&mut self, index: usize, value: f32, at_once: bool) {
        #[cfg(feature = "__test-hooks")]
        let at_once = at_once || (self.mutation.param_step && index != 0);
        #[cfg(feature = "__test-hooks")]
        let from_target = self.mutation.restart_from_target;
        #[cfg(not(feature = "__test-hooks"))]
        let from_target = false;
        match index {
            0 => self.land_type(value as u8, at_once),
            1 => {
                let f = self.applied_frequency(value);
                let v = self.freq_scale(f);
                self.freq.land(v, at_once, from_target);
                self.freq_hz_to = f;
            }
            2 => {
                self.q.land(dm::ln(f64::from(value)), at_once, from_target);
                self.q_to = f64::from(value);
            }
            _ => self.gain_db.land(f64::from(value), at_once, from_target),
        }
        if at_once {
            self.recompute();
        }
    }

    fn land_type(&mut self, t: u8, at_once: bool) {
        let t = t.min(TYPE_COUNT - 1);
        #[cfg(feature = "__test-hooks")]
        let at_once = at_once || self.mutation.type_instant;
        if at_once {
            self.ftype = t;
            self.blend.active = false;
            return;
        }
        if !self.blend.active {
            if t == self.ftype {
                return;
            }
            self.blend = Blend {
                from: MIXES[self.ftype as usize],
                origin: Some(self.ftype),
                k: 0,
                active: true,
            };
            self.ftype = t;
            return;
        }
        if t == self.ftype {
            return; // the type already targeted
        }
        if self.blend.origin == Some(t) {
            // Back to the origin: reverse from the current phase (spec 021).
            self.blend.from = MIXES[self.ftype as usize];
            self.blend.origin = Some(self.ftype);
            self.blend.k = self.len - self.blend.k;
            self.ftype = t;
            return;
        }
        // Freeze the current mix and restart toward the newest type (hardware plan review B4).
        let w = smoothstep(f64::from(self.blend.k) / f64::from(self.len));
        let to = MIXES[self.ftype as usize];
        let from = self.blend.from;
        self.blend.from = [
            from[0] + w * (to[0] - from[0]),
            from[1] + w * (to[1] - from[1]),
            from[2] + w * (to[2] - from[2]),
        ];
        self.blend.origin = None;
        self.blend.k = 0;
        self.ftype = t;
    }

    /// Ends every transition at its target and clears the state (`reset`, `reconfigure`,
    /// `start_recording`, `set_component`; plan R2).
    pub(crate) fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.freq.finish();
        self.q.finish();
        self.gain_db.finish();
        self.blend.active = false;
        self.recompute();
    }

    /// Whether a parameter transition or a type blend runs.
    #[cfg(test)]
    pub(crate) fn transitioning(&self) -> bool {
        self.freq.active || self.q.active || self.gain_db.active || self.blend.active
    }

    /// The applied (frequency Hz, Q, gain dB) now (the trace's reading).
    pub(crate) fn applied(&self) -> (f64, f64, f64) {
        let f = if self.freq.active {
            self.freq_hz(self.freq.cur)
        } else {
            self.freq_hz_to
        };
        let q = if self.q.active {
            dm::exp(self.q.cur)
        } else {
            self.q_to
        };
        (f, q, self.gain_db.cur)
    }

    fn recompute(&mut self) {
        let (f, q, _) = self.applied();
        let x = core::f64::consts::PI * f / self.rate;
        let g = dm::sin(x) / dm::cos(x);
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        self.a1 = a1;
        self.a2 = a2;
        self.a3 = g * a2;
        self.k = k;
        self.gain = dm::exp(self.gain_db.cur * (core::f64::consts::LN_10 / 20.0));
    }

    /// One sample.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        if self.freq.active || self.q.active || self.gain_db.active {
            let len = self.len;
            if self.freq.active {
                self.freq.step(len);
            }
            if self.q.active {
                self.q.step(len);
            }
            if self.gain_db.active {
                self.gain_db.step(len);
            }
            self.recompute();
        }
        let v3 = x - self.s2;
        let v1 = self.a1 * self.s1 + self.a2 * v3;
        let v2 = self.s2 + self.a2 * self.s1 + self.a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        if self.s1.abs() < FLUSH && self.s2.abs() < FLUSH {
            self.s1 = 0.0;
            self.s2 = 0.0;
        }
        let b = self.k * v1;
        let to = MIXES[self.ftype as usize];
        let y = if self.blend.active {
            self.blend.k += 1;
            if self.blend.k >= self.len {
                self.blend.active = false;
                mix(to, x, b, v2)
            } else {
                let w = smoothstep(f64::from(self.blend.k) / f64::from(self.len));
                let ya = mix(self.blend.from, x, b, v2);
                let yb = mix(to, x, b, v2);
                ya + w * (yb - ya)
            }
        } else {
            mix(to, x, b, v2)
        };
        self.gain * y
    }
}

/// `m0 x + m1 b + m2 v2` in this order (the harness reference repeats it).
#[inline]
pub(crate) fn mix(m: [f64; 3], x: f64, b: f64, v2: f64) -> f64 {
    m[0] * x + m[1] * b + m[2] * v2
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const DEFAULTS: [f32; 4] = [0.0, 1000.0, 0.7071, 0.0];

    fn filt(rate: u32, values: [f32; 4]) -> BandFilter {
        BandFilter::new(rate, rate / 50, values)
    }

    fn impulse(f: &mut BandFilter, n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| f.process(if i == 0 { 1.0 } else { 0.0 }))
            .collect()
    }

    fn dc_gain(f: &mut BandFilter) -> f64 {
        let mut y = 0.0;
        for _ in 0..200_000 {
            y = f.process(1.0);
        }
        y
    }

    /// The coefficient form's structural zeros (D4): the high-pass and band-pass at DC.
    #[test]
    fn structural_zeros_exact() {
        for &rate in &[6000, 48_000] {
            for t in [1.0f32, 2.0] {
                let mut f = filt(rate, [t, 1000.0, 0.7071, 0.0]);
                let y = dc_gain(&mut f);
                assert!(y.abs() < 1e-12, "type {t} at {rate}: DC {y}");
            }
            // Nyquist: an alternating input through the low-pass and the band-pass.
            for t in [0.0f32, 2.0] {
                let mut f = filt(rate, [t, 1000.0, 0.7071, 0.0]);
                let mut y = 0.0;
                for i in 0..200_000 {
                    y = f.process(if i % 2 == 0 { 1.0 } else { -1.0 });
                }
                assert!(y.abs() < 1e-12, "type {t} at {rate}: Nyquist {y}");
            }
        }
    }

    /// The unity points (D4): the low-pass and notch at DC.
    #[test]
    fn unity_points() {
        for t in [0.0f32, 3.0] {
            let mut f = filt(48_000, [t, 1000.0, 0.7071, 0.0]);
            let y = dc_gain(&mut f);
            assert!((y - 1.0).abs() < 1e-10, "type {t}: {y}");
        }
    }

    /// Research R-03: the triples are in {−1, 0, 1}.
    #[test]
    fn mixes_are_constant_triples() {
        for m in MIXES {
            for c in m {
                assert!(c == 0.0 || c == 1.0 || c == -1.0);
            }
        }
    }

    /// Plan R5: the blend over one state equals two sections fed the same input, bit for bit.
    #[test]
    fn blend_equals_two_sections() {
        let rate = 48_000;
        let mut one = filt(rate, DEFAULTS);
        let mut a = filt(rate, DEFAULTS);
        let mut b = filt(rate, [1.0, 1000.0, 0.7071, 0.0]);
        let x: Vec<f64> = (0..4000).map(|i| dm::sin(0.07 * i as f64) * 0.5).collect();
        for &v in &x[..1000] {
            one.process(v);
            a.process(v);
            b.process(v);
        }
        one.land(0, 1.0, false);
        let len = one.len;
        for (i, &v) in x[1000..].iter().enumerate() {
            let y = one.process(v);
            let ya = a.process(v);
            let yb = b.process(v);
            let n = i as u32 + 1;
            let want = if n >= len {
                yb
            } else {
                let w = smoothstep(f64::from(n) / f64::from(len));
                ya + w * (yb - ya)
            };
            assert_eq!(y.to_bits(), want.to_bits(), "sample {i}");
        }
    }

    /// Research R-02: n = 1 at P, the target exact at P + N − 1, log of the applied frequency.
    #[test]
    fn ramp_trajectory() {
        let rate = 48_000;
        let mut f = filt(rate, DEFAULTS);
        f.land(1, 2000.0, false);
        let len = f.len;
        let (l0, l1) = (dm::ln(1000.0), dm::ln(2000.0));
        for n in 1..=len {
            f.process(0.0);
            let want = if n == len {
                l1
            } else {
                l0 + smoothstep(f64::from(n) / f64::from(len)) * (l1 - l0)
            };
            assert!((f.freq.cur - want).abs() < 1e-9, "n {n}");
        }
        assert_eq!(f.freq.cur, l1);
        assert!(!f.transitioning());
    }

    /// Hardware plan review B11: both endpoints clamped; a move wholly above the ceiling is flat.
    #[test]
    fn ramp_clamped_endpoints() {
        let mut f = filt(6000, [0.0, 20_000.0, 0.7071, 0.0]);
        assert_eq!(f.applied().0.to_bits(), (0.45f64 * 6000.0).to_bits());
        f.land(1, 10_000.0, false);
        for _ in 0..f.len {
            f.process(0.0);
            assert!((f.applied().0 - 2700.0).abs() < 1e-9);
        }
    }

    /// Spec 011's rule: a second landing restarts from the current value.
    #[test]
    fn second_landing_restarts_from_current() {
        let mut f = filt(48_000, DEFAULTS);
        f.land(3, 12.0, false);
        for _ in 0..(f.len / 2) {
            f.process(0.0);
        }
        let mid = f.gain_db.cur;
        f.land(3, -12.0, false);
        assert_eq!(f.gain_db.from, mid);
        f.process(0.0);
        assert!(f.gain_db.cur < mid);
    }

    /// Hardware plan review B4: a type landing during a blend freezes the current mix and
    /// restarts; back to the origin reverses; the same type is a no-op.
    #[test]
    fn type_landing_during_blend_restarts() {
        let mut f = filt(48_000, DEFAULTS);
        f.land(0, 1.0, false);
        for _ in 0..(f.len / 2) {
            f.process(0.0);
        }
        let k = f.blend.k;
        f.land(0, 1.0, false); // the same type
        assert_eq!(f.blend.k, k);
        f.land(0, 0.0, false); // back to the origin
        assert_eq!(f.blend.k, f.len - k);
        assert_eq!(f.blend.origin, Some(1));
        f.land(0, 2.0, false); // a third type
        assert_eq!(f.blend.k, 0);
        assert_eq!(f.blend.origin, None);
        assert!(f.blend.active);
        // The output is continuous: the frozen mix is the one in force.
        let w = smoothstep(f64::from(f.len - k) / f64::from(f.len));
        let want = [w * 0.0 + (1.0 - w) * 1.0, 0.0, 0.0];
        assert!((f.blend.from[0] - want[0]).abs() < 1e-12);
    }

    /// Hardware post-implementation B4: a landing at the value in force, and a move wholly above
    /// the ceiling, are bit-identical to no landing.
    #[test]
    fn same_value_landing_is_a_no_op() {
        let x: Vec<f64> = (0..2000).map(|i| dm::sin(0.05 * i as f64) * 0.4).collect();
        for (rate, values, index, to) in [
            (48_000, DEFAULTS, 1usize, 1000.0f32),
            (48_000, DEFAULTS, 2, 0.7071),
            (6000, [0.0, 5000.0, 3.0, 0.0], 1, 10_000.0),
        ] {
            let mut a = filt(rate, values);
            let mut b = filt(rate, values);
            for &v in &x[..500] {
                a.process(v);
                b.process(v);
            }
            b.land(index, to, false);
            for &v in &x[500..] {
                assert_eq!(
                    a.process(v).to_bits(),
                    b.process(v).to_bits(),
                    "{rate} {index}"
                );
            }
        }
    }

    /// Hardware plan review B14: the state flushes below 1e-30.
    #[test]
    fn flush_below_1e_30() {
        let mut f = filt(96_000, [0.0, 20.0, 10.0, 0.0]);
        f.process(1e-29);
        for _ in 0..2_000_000 {
            f.process(0.0);
        }
        assert_eq!((f.s1, f.s2), (0.0, 0.0));
    }

    /// Research R-02: the coefficients change only while a transition runs.
    #[test]
    fn recompute_only_while_ramping() {
        let mut f = filt(48_000, DEFAULTS);
        let a1 = f.a1;
        for _ in 0..100 {
            f.process(0.3);
        }
        assert_eq!(f.a1, a1);
        f.land(2, 3.0, false);
        f.process(0.3);
        assert_ne!(f.a1, a1);
    }

    /// The impulse response decays (a stable section at the extremes).
    #[test]
    fn stable_at_extremes() {
        for (freq, q) in [(20.0f32, 10.0f32), (2700.0, 10.0), (20_000.0, 0.5)] {
            let mut f = filt(6000, [0.0, freq, q, 24.0]);
            let h = impulse(&mut f, 400_000);
            assert!(h.last().unwrap().abs() < 1e-6, "{freq} {q}");
        }
    }
}
