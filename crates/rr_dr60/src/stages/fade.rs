//! The live stage bypass's crossfade (spec 021 D3, D5; research R-01, R-02): one `Fade` per
//! playback-side stage (10 – 13). The stage keeps running while bypassed; only its
//! contribution to the signal is blended between its processed output `y` and its input `x`
//! as `y + w·(x − y)`, `w` the **bypass weight**: the smoothstep `u²(3 − 2u)` at `u = k / N`
//! over N = 20 ms of samples (A-045's length, an engine convention; the device has no stage
//! switches). A bypass runs `k` from 0 to N, a return from N to 0; a landing during a fade
//! reverses the direction from the current phase; at both ends the branch is **selected**, not
//! computed (spec 019's lesson: `a + 1.0·(b − a)` is not `b`, and `y + 0·(x − y)` turns a
//! −0.0 into +0.0).

/// What a fade says about the sample being produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FadeOut {
    /// At rest on: the stage's processed output itself.
    On,
    /// At rest bypassed: the stage's input itself.
    Bypassed,
    /// Fading: `y + w·(x − y)` with this bypass weight, strictly inside (0, 1).
    Blend(f64),
}

/// One stage's bypass state (data-model.md › Fade): the phase index `k` (0 = on, N =
/// bypassed), the direction (+1 toward bypassed, −1 toward on, 0 at rest), N and 1/N.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fade {
    k: u32,
    dir: i8,
    n: u32,
    inv_n: f64,
}

impl Fade {
    /// A fade of `n` samples at rest on (`n` ≥ 2; the callers pass 20 ms of samples).
    pub(crate) fn new(n: u32) -> Self {
        let n = n.max(2);
        Self {
            k: 0,
            dir: 0,
            n,
            inv_n: 1.0 / f64::from(n),
        }
    }

    /// The fade's length N.
    #[cfg(test)]
    pub(crate) fn len(&self) -> u32 {
        self.n
    }

    /// A fade is running (the direction is not 0).
    #[inline]
    pub(crate) fn running(&self) -> bool {
        self.dir != 0
    }

    /// At rest with the stage bypassed (`k == N` and not running).
    #[inline]
    pub(crate) fn at_rest_bypassed(&self) -> bool {
        self.dir == 0 && self.k == self.n
    }

    /// The value in force or being approached: `true` while heading to or resting at bypassed.
    pub(crate) fn target_bypassed(&self) -> bool {
        match self.dir {
            1 => true,
            -1 => false,
            _ => self.k == self.n,
        }
    }

    /// A landing (spec 021 D2, D5; research R-02): `at_once` (a stream's first produced
    /// sample, spec 011's rule) snaps to the value; otherwise from rest the fade starts toward
    /// the value, and during a fade the direction flips from the current phase (`k` kept). A
    /// landing of the value already targeted changes nothing (the stream logs the no-op).
    pub(crate) fn land(&mut self, bypassed: bool, at_once: bool) {
        if at_once {
            self.k = if bypassed { self.n } else { 0 };
            self.dir = 0;
            return;
        }
        if self.target_bypassed() == bypassed {
            return;
        }
        self.dir = if bypassed { 1 } else { -1 };
    }

    /// Snaps to `bypassed` at rest (the engine's `reset`, `start_playback`'s adopt).
    pub(crate) fn snap(&mut self, bypassed: bool) {
        self.land(bypassed, true);
    }

    /// The step for the sample being produced (research R-02): clamp the phase into [0, N]
    /// first, then test the ends (so a direction flipped at an end never leaves the range), and
    /// blend strictly inside. The landing sample computes `k = 1` (or N − 1 for a return).
    #[inline]
    pub(crate) fn step(&mut self) -> FadeOut {
        if self.dir == 0 {
            return if self.k == self.n {
                FadeOut::Bypassed
            } else {
                FadeOut::On
            };
        }
        let k = (i64::from(self.k) + i64::from(self.dir)).clamp(0, i64::from(self.n));
        // The clamp keeps `k` within [0, N] ≤ u32::MAX.
        self.k = k as u32;
        if self.k == 0 {
            self.dir = 0;
            FadeOut::On
        } else if self.k == self.n {
            self.dir = 0;
            FadeOut::Bypassed
        } else {
            FadeOut::Blend(self.weight(self.k))
        }
    }

    /// The bypass weight at phase `k`: the smoothstep `u²(3 − 2u)` at `u = k · (1/N)` (spec
    /// 011's form; `(N/2) · fl(1/N)` is exactly 0.5 at every supported rate, research R-01).
    #[inline]
    pub(crate) fn weight(&self, k: u32) -> f64 {
        let u = f64::from(k) * self.inv_n;
        u * u * (3.0 - 2.0 * u)
    }

    /// Test hook (spec 021 research R-09): the linear weight `u` (the `fade_linear` mutant).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn weight_linear(&self, k: u32) -> f64 {
        f64::from(k) * self.inv_n
    }

    /// The phase index (tests and the trace).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn phase(&self) -> u32 {
        self.k
    }
}

/// A deliberate alteration of the live bypass for the spec 021 mutation tests (research R-09;
/// `Pipeline::__with_bypass_mutation`). `Default` is the committed behavior.
#[cfg(any(test, feature = "__test-hooks"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BypassMutation {
    /// The blend written as `x + w·(y − x)` (the first draft's inverted labels).
    pub weight_inverted: bool,
    /// No fade: every landing switches at once (the click reader's step reference).
    pub fade_step: bool,
    /// The linear weight `u` in place of the smoothstep.
    pub fade_linear: bool,
    /// A bypassed stage's filter not run (the return then differs from the all-on run).
    pub stage_cold: bool,
    /// Stage 11's clip count counts while bypassed.
    pub count_ungated: bool,
    /// A stage not on the active route holds its fade's phase.
    pub phase_frozen_off_route: bool,
}

/// Applies a fade's verdict to a stage's output `y` and input `x`: the ends selected, the
/// inside blended (spec 021 FR-002).
#[inline(always)]
pub(crate) fn apply(out: FadeOut, y: f64, x: f64) -> f64 {
    match out {
        FadeOut::On => y,
        FadeOut::Bypassed => x,
        FadeOut::Blend(w) => y + w * (x - y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SUPPORTED_HOST_RATES;

    fn n_of(rate: u32) -> u32 {
        crate::output::volume_transition_samples(rate).unwrap()
    }

    /// Spec 021 D9(c): w(N/2) = 0.5 exactly at the seven rates (research R-01).
    #[test]
    fn midpoint_is_exactly_half() {
        for rate in SUPPORTED_HOST_RATES {
            let f = Fade::new(n_of(rate));
            assert_eq!(f.weight(f.len() / 2), 0.5, "{rate} Hz, N = {}", f.len());
        }
        let f = Fade::new(crate::speed::CROSSFADE_SAMPLES);
        assert_eq!(f.weight(60), 0.5);
    }

    /// FR-005: w monotone in k within a direction, strictly inside (0, 1) for 0 < k < N.
    #[test]
    fn monotone_within_a_direction() {
        for rate in SUPPORTED_HOST_RATES {
            let f = Fade::new(n_of(rate));
            let mut last = 0.0;
            for k in 1..f.len() {
                let w = f.weight(k);
                assert!(w > last && w < 1.0, "{rate} Hz k = {k}: {w}");
                last = w;
            }
        }
    }

    /// FR-002: the landing sample computes k = 1; a return from rest computes N − 1 first.
    #[test]
    fn landing_sample_computes_k_1() {
        let mut f = Fade::new(120);
        f.land(true, false);
        assert_eq!(f.step(), FadeOut::Blend(f.weight(1)));
        assert_eq!(f.phase(), 1);
        let mut g = Fade::new(120);
        g.snap(true);
        g.land(false, false);
        assert_eq!(g.step(), FadeOut::Blend(g.weight(119)));
    }

    /// Research R-02 (hardware B2): the same-position pairs: the clamp holds the end, no blend.
    #[test]
    fn step_clamps_first() {
        let mut f = Fade::new(120);
        f.land(true, false);
        f.land(false, false); // flipped at k = 0 before any sample
        assert_eq!(f.step(), FadeOut::On);
        assert!(!f.running());
        assert_eq!(f.phase(), 0);
        let mut g = Fade::new(120);
        g.snap(true);
        g.land(false, false);
        g.land(true, false);
        assert_eq!(g.step(), FadeOut::Bypassed);
        assert!(g.at_rest_bypassed());
    }

    /// D5 / US1 AS7: a reversal at k = N/2 reaches k = 0 in exactly N/2 samples.
    #[test]
    fn reversal_at_half_returns_in_half() {
        for rate in SUPPORTED_HOST_RATES {
            let n = n_of(rate);
            let mut f = Fade::new(n);
            f.land(true, false);
            for _ in 0..n / 2 {
                f.step();
            }
            assert_eq!(f.phase(), n / 2);
            f.land(false, false);
            let mut steps = 0;
            loop {
                steps += 1;
                if f.step() == FadeOut::On {
                    break;
                }
                assert!(steps <= n, "{rate} Hz: no return");
            }
            assert_eq!(steps, n / 2, "{rate} Hz");
            assert!(!f.running());
        }
    }

    /// The edge case restated at the plan: a landing at the sample that would compute k = N
    /// reverses from N − 1 and never reaches w = 1; a landing after a selected end is a return
    /// from rest.
    #[test]
    fn toggle_at_last_blend_sample() {
        let mut f = Fade::new(120);
        f.land(true, false);
        for _ in 0..119 {
            f.step();
        }
        assert_eq!(f.phase(), 119);
        f.land(false, false); // before the sample that would select x
        assert_eq!(f.step(), FadeOut::Blend(f.weight(118)));
        let mut g = Fade::new(120);
        g.land(true, false);
        for _ in 0..120 {
            g.step();
        }
        assert!(g.at_rest_bypassed());
        g.land(false, false);
        assert_eq!(g.step(), FadeOut::Blend(g.weight(119)));
    }

    /// Spec 011's rule: the stream's first produced sample switches at once.
    #[test]
    fn at_once_snaps() {
        let mut f = Fade::new(960);
        f.land(true, true);
        assert!(f.at_rest_bypassed());
        assert_eq!(f.step(), FadeOut::Bypassed);
        f.land(false, true);
        assert_eq!(f.step(), FadeOut::On);
        // During a fade too.
        f.land(true, false);
        f.step();
        f.land(false, true);
        assert_eq!(f.step(), FadeOut::On);
    }

    /// FR-002: the ends are selected, a −0.0 preserved; a same-target landing is ignored.
    #[test]
    fn ends_are_selected() {
        assert_eq!(apply(FadeOut::On, -0.0, 1.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(
            apply(FadeOut::Bypassed, 1.0, -0.0).to_bits(),
            (-0.0f64).to_bits()
        );
        assert_eq!(apply(FadeOut::Blend(0.5), 1.0, 3.0), 2.0);
        let mut f = Fade::new(120);
        f.land(false, false);
        assert!(!f.running());
        f.land(true, false);
        f.land(true, false);
        assert_eq!(f.step(), FadeOut::Blend(f.weight(1)));
    }
}
