//! The output chain, signal-chain stages 11 – 13 (spec 009): the volume control and power
//! amplifier, the speaker template, the earphone output.
//!
//! Runs at the **host rate after the resampler** on the 64-bit sample, before the narrowing
//! (research R-01), inside both [`PlaybackPath`](crate::paths::PlaybackPath) instances (the
//! one-pass's and the player's), so record-then-play stays bit-identical to the one-pass. Every
//! figure is a **design constant modeled on the circuit** (S-002 sheet 3.1) **and the datasheet**
//! (S-006), emitted by `tools/output-chain/design_output_chain.py` into
//! [`output_coeffs`](super::output_coeffs); nothing here is a measurement of a real unit:
//!
//! - stage 11 (A-031 adopted; A-043, A-044): the volume control's 32 Hz input coupling corner
//!   (both routes), the position's **loaded law** (the wiper resistance in series with the
//!   amplifier's input resistor; A-044), and on the speaker route the amplifier's 159 Hz input
//!   corner, its gain over the clip ceiling folded into one per-block scalar, and a symmetric
//!   parabolic knee at the ceiling (research R-04);
//! - stage 12 (A-010 revised, **low confidence**): a **template of a small speaker**, a
//!   second-order resonance and a rising shelf, peak-normalized to 1.0 at each rate;
//! - stage 13 (A-032 adopted): the earphone follower's input and output coupling corners
//!   (120 and 280 Hz nominal); **no clipper is modeled** (the follower's class-A cut-off on
//!   loud material is registered, not modeled).
//!
//! The controls (the route and the volume position) are copied in once per processing call by
//! [`OutputChain::set_controls`] (FR-009: they take effect at the next block); nothing in the
//! per-sample path reads a control. Since spec 011 a position change starts a **volume
//! transition** (A-045): over N = 20 ms of host samples the knob moves from where it is to the
//! new position along a smoothstep and the active route's scalar follows the loaded law at the
//! knob's position, held within the two ends and against the previous sample, the last sample
//! carrying exactly the target's block scalar (research R-01 – R-04). Construction, `reset`, a
//! route change, an inactive chain and a bypassed stage 11 take the position at once. Plain
//! IEEE `f64` arithmetic, no platform math function (FR-012; 001 R-04).

use crate::output::volume_transition_samples;
use crate::settings::{OutputSettings, Route, SUPPORTED_HOST_RATES, Settings, Tap};
use crate::stages::biquad::Biquad;
use crate::stages::first_order::FirstOrder;
use crate::stages::output_coeffs as c;

/// The index of a supported host rate into the per-rate tables (`SUPPORTED_HOST_RATES` order).
pub(crate) fn rate_index(host_rate_hz: u32) -> Option<usize> {
    SUPPORTED_HOST_RATES.iter().position(|&r| r == host_rate_hz)
}

/// The loaded volume law on the speaker route (A-044): volts of amplifier output per volt of
/// codec output at wiper fraction `p`: `p · 2 Rf / (Ri + R_pot · p (1 − p))`. 13.6 at 1.
#[inline]
pub(crate) fn g_spk(p: f64) -> f64 {
    p * (2.0 * c::R_FEEDBACK_OHM) / (c::R_INPUT_OHM + c::R_POT_OHM * p * (1.0 - p))
}

/// The loaded volume law on the earphone route (A-044): the volume control's output per volt
/// of codec output at wiper fraction `p`, divided against the branch's input impedance:
/// `p · (Ri + R65 + Z) / (Ri + R65 + Z + R_pot · p (1 − p))`. Exactly 1 at 1.
#[inline]
pub(crate) fn g_ear(p: f64) -> f64 {
    let z = c::R_INPUT_OHM + c::R_EAR_SERIES_OHM + c::Z_EAR_OHM;
    p * z / (z + c::R_POT_OHM * p * (1.0 - p))
}

/// The speaker route's per-block scalar: the loaded gain over the clip ceiling, so that at the
/// reference position a full-scale codec output sits exactly at the ceiling (A-042, A-043).
#[inline]
pub(crate) fn k_spk(p: f64) -> f64 {
    g_spk(p) * c::CODEC_FULL_SCALE_V / c::V_CLIP_V
}

/// The earphone route's per-block scalar (A-042: unity at position 1).
#[inline]
pub(crate) fn k_ear(p: f64) -> f64 {
    g_ear(p)
}

// Spec 011 FR-005: the output chain's share of the drain (`N_OUT`) alone exceeds 2.87
// transition lengths at every rate (2.8786 – 2.8833), so a transition started in a playback's
// last block completes inside the drain.
const _: () = {
    let mut i = 0;
    while i < SUPPORTED_HOST_RATES.len() {
        let n = SUPPORTED_HOST_RATES[i] as u64 * crate::output::VOLUME_TRANSITION_MS as u64 / 1000; // A-045
        assert!(
            100 * c::N_OUT[i] as u64 > 287 * n,
            "the drain completes a transition"
        );
        i += 1;
    }
};

/// Holds a transition sample's gain `g` within the previous sample's gain and the target's
/// (spec 011 FR-003; research R-03: the range and running hold): upward within `[prev, to]`,
/// downward within `[to, prev]`. Compares and selects only; `g` is never NaN.
#[inline(always)]
fn hold(g: f64, prev: f64, to: f64, up: bool) -> f64 {
    let (lo, hi) = if up { (prev, to) } else { (to, prev) };
    let g = if g < lo { lo } else { g };
    if g > hi { hi } else { g }
}

/// The amplifier's clipper (research R-04; A-031, low): a symmetric parabolic knee from
/// `k = KNEE_START` of the ceiling to `2 − k`, C¹ at both joints, a hard ceiling of 1.0 above.
/// Plain IEEE arithmetic: compares, multiplies, one divide of constants. Since spec 018 the
/// chain applies [`knee_b`] with the battery's ceiling factor; this form is its b = 1 twin
/// (bit-identical, pinned).
#[inline(always)]
pub(crate) fn knee(x: f64, k: f64) -> f64 {
    let u = x.abs(); // exact (a sign-bit clear), so bit-identical everywhere
    let y = if u <= k {
        u
    } else if u >= 2.0 - k {
        1.0
    } else {
        let d = 2.0 - k - u;
        1.0 - d * d / (4.0 * (1.0 - k))
    };
    if x < 0.0 { -y } else { y }
}

/// The clipper at a ceiling of `b` (spec 018 FR-002, research R-03): the knee's start at
/// `lo = k·b`, the hard ceiling `b` from `hi = (2 − k)·b`, the parabola `b − d² ÷ den` with
/// `den = 4·(1 − k)·b` between them, the sample untouched below the start (the below-knee
/// identity). The three thresholds are the chain's stored fields, recomputed when the factor
/// moves; at b = 1.0 every branch is [`knee`]'s bit for bit (a multiply by 1.0 is exact).
#[inline(always)]
pub(crate) fn knee_b(x: f64, b: f64, lo: f64, hi: f64, den: f64) -> f64 {
    let u = x.abs();
    let y = if u <= lo {
        u
    } else if u >= hi {
        b
    } else {
        let d = hi - u;
        b - d * d / den
    };
    if x < 0.0 { -y } else { y }
}

/// The output clamp's bound (spec 010 FR-001; A-042: the route's full scale): `y` above `hi`
/// becomes `hi`, below `lo` becomes `lo`, a not-a-number becomes 0.0 (unreachable: the output
/// is finite, spec 002 R-07; defined so the behaviour is stated), and anything else, −0.0
/// included, passes unchanged. Not `f64::clamp`, whose `lo <= hi` assertion would be a panic
/// site in the per-sample path once the ends are fields (research R-01).
#[inline(always)]
pub(crate) fn bound(y: f64, lo: f64, hi: f64) -> f64 {
    // IEEE maxNum / minNum: branchless (one instruction each on aarch64, a short branchless
    // sequence on x86-64); they return one operand exactly, map ±∞ to the ends and would return
    // `lo` for a NaN, which the select below turns into 0.0 (a compare and a conditional move;
    // research R-01 as built: +0.47 ns per host sample on aarch64 against +3.6 ns for a
    // two-branch form; +4.25 ns, +2.46 %, on the x86-64 CI runner; SC-005's budget is 3 %).
    let r = y.max(lo).min(hi);
    if y.is_nan() { 0.0 } else { r }
}

/// The output chain's state for one playback path (data-model.md › `OutputChain` state).
#[derive(Clone, Debug)]
pub(crate) struct OutputChain {
    route: Route,
    k_spk: f64,
    k_ear: f64,
    run_amp: bool,
    run_speaker: bool,
    run_earphone: bool,
    /// `run_amp || run_speaker || run_earphone`, precomputed for the per-sample path.
    active: bool,
    /// The point `process` returns: `RouteOutput` for the player; the configured tap for the
    /// one-pass.
    tap: Tap,
    /// The knee's start: in production always `KNEE_START` (A-031); a field only so the SC-006
    /// mutation tests can move it through the `__test-hooks` feature.
    knee_start: f64,
    /// A scale on the speaker route's scalar: in production always 1.0; the mutation tests'
    /// lever on the level.
    gain_scale: f64,
    /// The output clamp's gate (spec 010 FR-001, FR-003): `clamp_enabled` **and** the return
    /// point is the route output. The player's return point is always the route output; the
    /// one-pass's is the configured tap, so the per-stage taps are never bounded.
    clamp: bool,
    /// Spec 016 (research R-03): the route peak and the output-clamp count are read only when
    /// the return point is the route output (the player always; the one-pass at that tap).
    meter_route: bool,
    /// Spec 016 FR-003 (j): the largest absolute sample of the current call at the route
    /// output before the bound; cleared at finalization.
    route_peak: f64,
    /// Spec 016 FR-003 (k): the host samples of the current call the bound changed.
    clamp_count: u32,
    /// The bound's two ends: in production always −1.0 and +1.0 (A-042: the route's full
    /// scale); fields only so the mutation tests can move them (`clamp_bound`,
    /// `clamp_positive_only`).
    bound_lo: f64,
    bound_hi: f64,
    vol_hpf: FirstOrder,
    amp_hpf: FirstOrder,
    speaker_res: Biquad,
    speaker_shelf: FirstOrder,
    ear_in: FirstOrder,
    ear_out: FirstOrder,
    n_out: u32,
    /// Spec 011 (A-045): the knob position the active route's scalar was last computed from;
    /// equal to `to` outside a transition (data-model.md › `OutputChain` state).
    knob: f64,
    /// The transition's start (the knob when it began) and target positions.
    from: f64,
    to: f64,
    /// The last position `set_controls` received: the compare that decides "a change".
    target: f32,
    /// The transition sample counter (0 … N) and N for this rate; `ramp_k == ramp_n` outside
    /// a transition (FR-002).
    ramp_k: u32,
    ramp_n: u32,
    /// 1 / N, computed once (research R-02: `u = k · inv_n` is non-decreasing in `k`).
    inv_n: f64,
    /// The transition's direction: the active route's target scalar above its start.
    up: bool,
    /// The target's block scalars as version 0.10 computes them, written at sample N (FR-001).
    g_to_spk: f64,
    g_to_ear: f64,
    /// True after `new()` and `reset()` until the first processed sample: `set_controls` then
    /// takes the position at once (FR-005; a zero-length call does not clear it).
    fresh: bool,
    /// Test hooks (spec 011 research R-07; `__test-hooks`): version 0.10's instantaneous step;
    /// the knob moved linearly in time. Both false in production.
    volume_step: bool,
    shape_linear: bool,
    /// Test hook (spec 015 research R-13): a route switch runs the full reset, losing the
    /// call's meter accumulators. False in production.
    pub(crate) full_reset_on_route: bool,
    /// Spec 018 (research R-02, R-03): the battery's ceiling factor in force, its transition
    /// (start, target, the counter and its length with 1 / N), the last value `set_battery`
    /// received, and the clipper's thresholds for the factor in force.
    batt: f64,
    batt_from: f64,
    batt_to: f64,
    batt_target: f32,
    batt_k: u32,
    batt_n: u32,
    batt_inv_n: f64,
    knee_lo: f64,
    knee_hi: f64,
    knee_den: f64,
    /// Spec 018 FR-010 (l): the host samples of the call the knee or the ceiling changed.
    amp_clip_count: u32,
    /// Spec 018 (research R-05): the amplifier meters are read at the taps that run the knee
    /// (`AfterVolumeAmp`, `AfterSpeaker`, `RouteOutput`), decided at construction.
    meter_amp: bool,
    /// Test hooks (spec 018 research R-07; `__test-hooks`), all false / 0 in production.
    batt_shape_linear: bool,
    batt_scale_sample: bool,
    batt_no_at_once_on_route: bool,
    batt_count_at_or_above: bool,
}

impl OutputChain {
    /// The chain for validated `settings` at the default controls (the speaker route, the
    /// reference position). With `honour_tap` the chain runs only for the taps after stage 10
    /// other than [`Tap::AfterPlayback`] (the one-pass, FR-011); without it every enabled stage
    /// runs and the output is the route's (the player). No allocation.
    pub(crate) fn new(settings: &Settings, honour_tap: bool) -> Self {
        // `Pipeline::new` validated the rate (the rate plan is built first), so the index exists;
        // 0 (the 6 kHz tables) only guards an impossible state.
        let i = rate_index(settings.host_rate_hz).unwrap_or(0);
        let enabled =
            !honour_tap || (settings.tap.is_after_stage_10() && settings.tap != Tap::AfterPlayback);
        let OutputSettings {
            volume_amp_enabled,
            speaker_enabled,
            earphone_enabled,
            clamp_enabled,
        } = settings.output;
        let tap = if honour_tap {
            settings.tap
        } else {
            Tap::RouteOutput
        };
        let mut chain = Self {
            route: Route::Speaker,
            k_spk: 1.0,
            k_ear: 1.0,
            run_amp: enabled && volume_amp_enabled, // A-031 (009 FR-002)
            run_speaker: enabled && speaker_enabled, // A-010 (009 FR-002)
            run_earphone: enabled && earphone_enabled, // A-032 (009 FR-002)
            active: enabled && (volume_amp_enabled || speaker_enabled || earphone_enabled),
            tap,
            knee_start: c::KNEE_START, // A-031 (low): the knee's shape
            gain_scale: 1.0,
            // 010 FR-003: only the route-output tap is bounded; the gate is decided here, never
            // read from the settings per sample.
            clamp: clamp_enabled && tap == Tap::RouteOutput,
            meter_route: tap == Tap::RouteOutput, // 016 research R-03
            route_peak: 0.0,
            clamp_count: 0,
            bound_lo: -1.0, // A-042: the route's full scale (spec 010 FR-001)
            bound_hi: 1.0,
            vol_hpf: FirstOrder::new(c::VOL_HPF[i]), // A-031: 32 Hz, both routes
            amp_hpf: FirstOrder::new(c::AMP_HPF[i]), // A-031: 159 Hz, speaker route
            speaker_res: Biquad::from_sos(c::SPK_RES[i]), // A-010: the template's resonance
            speaker_shelf: FirstOrder::new(c::SPK_SHELF[i]), // A-010: the template's shelf
            ear_in: FirstOrder::new(c::EAR_IN[i]),   // A-032: 120 Hz nominal
            ear_out: FirstOrder::new(c::EAR_OUT[i]), // A-032: 280 Hz nominal
            n_out: c::N_OUT[i],                      // research R-08
            knob: c::VOLUME_REFERENCE,
            from: c::VOLUME_REFERENCE,
            to: c::VOLUME_REFERENCE,
            target: c::VOLUME_REFERENCE as f32,
            ramp_k: 0,
            ramp_n: 0,
            inv_n: 0.0,
            up: false,
            g_to_spk: 1.0,
            g_to_ear: 1.0,
            fresh: true, // 011 FR-005: the first block takes the position at once
            volume_step: false,
            shape_linear: false,
            full_reset_on_route: false,
            batt: 1.0, // 018: the default's factor is exactly 1 (A-056 at the nominal mark)
            batt_from: 1.0,
            batt_to: 1.0,
            batt_target: crate::battery::DEFAULT_BATTERY_VOLTS,
            batt_k: 0,
            batt_n: 0,
            batt_inv_n: 0.0,
            knee_lo: 0.0,
            knee_hi: 0.0,
            knee_den: 1.0,
            amp_clip_count: 0,
            meter_amp: matches!(
                tap,
                Tap::AfterVolumeAmp | Tap::AfterSpeaker | Tap::RouteOutput
            ),
            batt_shape_linear: false,
            batt_scale_sample: false,
            batt_no_at_once_on_route: false,
            batt_count_at_or_above: false,
        };
        // A-045: N = 20 ms of host samples. Unreachable fallback: `Pipeline::new` validated the
        // rate before building the chain; 120 (the 6 kHz figure) only avoids a panic site.
        chain.set_ramp_len(volume_transition_samples(settings.host_rate_hz).unwrap_or(120));
        chain.set_batt_len(volume_transition_samples(settings.host_rate_hz).unwrap_or(120)); // 018: the same 20 ms
        chain.recompute_knee();
        chain.set_controls(Route::Speaker, c::VOLUME_REFERENCE as f32);
        chain
    }

    /// Copies the controls in at a block boundary (FR-009): a route change resets every
    /// section (the newly selected branch and the one left start from rest) and takes the
    /// position at once; a position change starts a **volume transition** (spec 011 FR-001:
    /// the knob moves over N samples from where it is, research R-01), unless the chain is
    /// fresh, inactive or has stage 11 off, which take it at once (FR-005); setting the
    /// position already targeted changes nothing. At position 0 both scalars are exactly 0.0
    /// once taken (at once or at transition sample N).
    pub(crate) fn set_controls(&mut self, route: Route, volume: f32) {
        if route != self.route {
            self.route = route;
            // Every section, the shared 32 Hz corner included (a simplification: physically that
            // section, C14 into VR1, is common to both routes and would keep its state; the
            // record band-pass leaves no low-frequency content for it to hold, hardware review
            // T043 N3). The sections-only reset marks the chain fresh: the new route starts at
            // the new position; the meter accumulators keep the samples before the switch
            // (spec 015 research R-05).
            if self.full_reset_on_route {
                self.reset(); // the mutant (research R-13)
            } else {
                self.reset_sections();
            }
        }
        if self.fresh || !self.active || !self.run_amp || self.volume_step {
            self.take_at_once(volume); // 011 FR-005 (the hook: 0.10's step)
        } else if volume != self.target {
            self.start_transition(volume); // 011 FR-001
        }
    }

    /// Takes `volume` at once (version 0.10's arithmetic): both scalars from the loaded law
    /// (A-044; one divide each), the knob at the position, no transition (spec 011 R-04).
    fn take_at_once(&mut self, volume: f32) {
        let p = f64::from(volume);
        self.k_spk = k_spk(p) * self.gain_scale;
        self.k_ear = k_ear(p);
        self.g_to_spk = self.k_spk;
        self.g_to_ear = self.k_ear;
        self.knob = p;
        self.from = p;
        self.to = p;
        self.target = volume;
        self.ramp_k = self.ramp_n;
    }

    /// Starts a transition from the knob's current position to `volume` (spec 011 FR-001,
    /// FR-003: a change during a transition starts a new full-length one from the knob).
    fn start_transition(&mut self, volume: f32) {
        let p = f64::from(volume);
        self.from = self.knob;
        self.to = p;
        self.target = volume;
        self.g_to_spk = k_spk(p) * self.gain_scale;
        self.g_to_ear = k_ear(p);
        self.up = match self.route {
            Route::Speaker => self.g_to_spk > self.k_spk,
            Route::Earphone => self.g_to_ear > self.k_ear,
        };
        self.ramp_k = 0;
    }

    /// Sets N (and 1 / N) and ends any transition (construction and the length hook).
    fn set_ramp_len(&mut self, n: u32) {
        self.ramp_n = n.max(1);
        self.inv_n = 1.0 / f64::from(self.ramp_n);
        self.ramp_k = self.ramp_n;
    }

    /// Spec 018: the battery transition's N (the volume's 20 ms) and 1 / N; ends any transition.
    fn set_batt_len(&mut self, n: u32) {
        self.batt_n = n.max(1);
        self.batt_inv_n = 1.0 / f64::from(self.batt_n);
        self.batt_k = self.batt_n;
        self.batt = self.batt_to;
        self.recompute_knee(); // hardware review B4: the thresholds follow the factor here too
    }

    /// Spec 018 (research R-03): the clipper's thresholds for the factor in force: `k·b`,
    /// `(2 − k)·b` and `4·(1 − k)·b`; at b = 1.0 exactly [`knee`]'s constants.
    fn recompute_knee(&mut self) {
        let k = self.knee_start;
        self.knee_lo = k * self.batt;
        self.knee_hi = (2.0 - k) * self.batt;
        self.knee_den = 4.0 * (1.0 - k) * self.batt;
    }

    /// Spec 018 FR-001, FR-003 (research R-01, R-02): the battery value in volts. Idempotent
    /// against the last request. The factor is taken at once (no transition) while the chain
    /// is fresh, inactive or has stage 11 off and on the earphone route; otherwise a **20 ms
    /// transition** on the factor starts from the factor in force (a change during a
    /// transition restarts from where it is).
    pub(crate) fn set_battery(&mut self, volts: f32) {
        if volts == self.batt_target {
            return;
        }
        self.batt_target = volts;
        let b = f64::from(crate::battery::ceiling_factor(volts));
        self.batt_to = b;
        if self.fresh || !self.active || !self.run_amp || self.route == Route::Earphone {
            self.batt = b;
            self.batt_from = b;
            self.batt_k = self.batt_n;
            self.recompute_knee();
        } else {
            self.batt_from = self.batt;
            self.batt_k = 0;
        }
    }

    /// One battery transition sample (spec 018 FR-003; spec 011's form on the factor): the
    /// smoothstep from the start to the target, monotonic by construction; at sample N the
    /// target exactly. The thresholds follow.
    #[inline]
    fn batt_step(&mut self) {
        self.batt_k += 1;
        if self.batt_k >= self.batt_n {
            self.batt_k = self.batt_n;
            self.batt = self.batt_to;
        } else {
            let u = f64::from(self.batt_k) * self.batt_inv_n;
            let s = if self.batt_shape_linear {
                u
            } else {
                u * u * (3.0 - 2.0 * u)
            };
            self.batt = self.batt_from + (self.batt_to - self.batt_from) * s;
        }
        self.recompute_knee();
    }

    /// The battery's ceiling factor in force (the unit tests and the test hook; the meters read
    /// the field directly). Gated like its only callers, or the feature-less builds see dead
    /// code under `-D warnings` (spec 016's lesson).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn battery_factor(&self) -> f64 {
        self.batt
    }

    /// One transition sample (spec 011 FR-001 – FR-003; data-model.md › The per-sample step):
    /// the knob along the smoothstep `u²(3 − 2u)`, the active route's scalar the loaded law at
    /// the knob held within the previous scalar and the target's; at sample N the target's
    /// block scalars exactly and the knob at the target.
    #[inline]
    fn ramp_step(&mut self) {
        self.ramp_k += 1;
        if self.ramp_k >= self.ramp_n {
            self.ramp_k = self.ramp_n;
            self.k_spk = self.g_to_spk; // 011 FR-001: exactly 0.10's scalar at sample N
            self.k_ear = self.g_to_ear;
            self.knob = self.to;
            return;
        }
        let u = f64::from(self.ramp_k) * self.inv_n;
        // A-045: the smoothstep (zero slope at both ends: the knob starts and ends at rest).
        let s = if self.shape_linear {
            u
        } else {
            u * u * (3.0 - 2.0 * u)
        };
        self.knob = self.from + (self.to - self.from) * s;
        match self.route {
            Route::Speaker => {
                let g = k_spk(self.knob) * self.gain_scale;
                self.k_spk = hold(g, self.k_spk, self.g_to_spk, self.up);
            }
            Route::Earphone => {
                let g = k_ear(self.knob);
                self.k_ear = hold(g, self.k_ear, self.g_to_ear, self.up);
            }
        }
    }

    /// Test-only (the `__test-hooks` feature): alters the chain for the SC-006 mutation tests
    /// (spec 009 FR-012; contracts/rust-api.md › Test hooks). The controls are re-applied so
    /// the scalars carry the new scale.
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn __mutate(&mut self, m: &crate::__test_hooks::OutputMutation) {
        let s = m.speaker_numerator_scale;
        let b = self.speaker_res.coefficients();
        self.speaker_res = Biquad::from_sos([b[0] * s, b[1] * s, b[2] * s, b[3], b[4]]);
        self.knee_start = m.knee_start;
        self.gain_scale = m.gain_scale;
        self.n_out = u32::try_from(i64::from(self.n_out) + m.n_out_delta).unwrap_or(0);
        // Spec 010 (research R-06): the bound moved, or applied to positive samples only.
        self.bound_hi = m.clamp_bound;
        self.bound_lo = if m.clamp_positive_only {
            f64::NEG_INFINITY
        } else {
            -m.clamp_bound
        };
        // Spec 011 (research R-07): 0.10's step, the linear shape, the length mutants; the
        // position is re-applied at once (R-04: no transition is left half-set).
        self.volume_step = m.volume_step;
        self.shape_linear = m.volume_shape_linear;
        let n = i64::from(self.ramp_n) + m.volume_ramp_delta;
        self.set_ramp_len(u32::try_from(n).unwrap_or(1));
        self.take_at_once(self.volume_for_rescale());
        // Spec 018 (research R-07): the battery mutants; the thresholds follow the moved knee.
        self.batt_shape_linear = m.battery_shape_linear;
        self.batt_scale_sample = m.battery_scale_sample;
        self.batt_no_at_once_on_route = m.battery_no_at_once_on_route;
        self.batt_count_at_or_above = m.battery_count_at_or_above;
        let bn = i64::from(self.batt_n) + m.battery_ramp_delta;
        self.set_batt_len(u32::try_from(bn).unwrap_or(1));
        self.recompute_knee();
    }

    /// Test-only: the position the current scalars were computed from is not stored (the
    /// controls are re-copied at every block), so a mutation re-applies the reference position;
    /// the harness sets its own position afterwards.
    #[cfg(feature = "__test-hooks")]
    fn volume_for_rescale(&self) -> f32 {
        c::VOLUME_REFERENCE as f32
    }

    /// One host-rate sample through the chain, returning the sample at the configured tap
    /// (data-model.md › The chain in the playback path), **bounded** when the output clamp is
    /// on and the tap is the route output (spec 010 FR-001, FR-003; research R-01). The bound
    /// applies to the stages' result even when every stage is off (the chain inactive), so a
    /// bypassed chain at the route-output tap is still bounded.
    #[inline(always)]
    pub(crate) fn process(&mut self, y: f64) -> f64 {
        let r = self.inner(y);
        // Spec 016 FR-003 (j), (k): the peak before the bound; the count of bounded samples
        // (`bound` returns `r` bit-exact inside the bound, so the test is exact).
        if self.meter_route {
            let a = r.abs();
            if a > self.route_peak {
                self.route_peak = a;
            }
        }
        if self.clamp {
            let b = bound(r, self.bound_lo, self.bound_hi);
            if b != r {
                self.clamp_count += 1;
            }
            b
        } else {
            r
        }
    }

    /// Spec 016 (research R-01): writes the route figures into `m` when the return point is
    /// the route output (otherwise `m` keeps its rest values) and clears the accumulators.
    pub(crate) fn finalize_into(&mut self, m: &mut crate::pipeline::Meters) {
        let peak = core::mem::replace(&mut self.route_peak, 0.0);
        let count = core::mem::replace(&mut self.clamp_count, 0);
        if self.meter_route {
            m.route_peak = crate::sanitize::narrow_out(peak);
            m.output_clamp_count = count;
        }
        // Spec 018 FR-010 (research R-05): the factor is reported at the taps that run the
        // knee; the count only where the knee runs (the speaker route with stage 11 on).
        let clips = core::mem::replace(&mut self.amp_clip_count, 0);
        if self.meter_amp {
            m.battery_ceiling = self.batt as f32;
            if self.route == Route::Speaker && self.run_amp {
                m.amp_clip_count = clips;
            }
        }
    }

    /// The stages themselves: the sample at the configured tap before the output clamp. With
    /// every stage off (or at a tap before the chain) the sample passes through with no
    /// arithmetic.
    #[inline(always)]
    fn inner(&mut self, y: f64) -> f64 {
        if !self.active {
            return y;
        }
        self.fresh = false; // 011 FR-005: the first processed sample ends "fresh"
        if self.ramp_k < self.ramp_n {
            self.ramp_step(); // 011 FR-001: one transition sample
        }
        // Stage 11, both routes: the volume control's input coupling corner.
        let u = if self.run_amp {
            self.vol_hpf.process(y)
        } else {
            y
        };
        match self.route {
            Route::Speaker => {
                // Stage 11's amplifier path: the input corner, the loaded gain over the ceiling,
                // the knee. `+ 0.0` turns a −0.0 at position 0 into +0.0.
                let a = if self.run_amp {
                    if self.batt_k < self.batt_n {
                        self.batt_step(); // 018 FR-003: one battery transition sample
                    }
                    let xin = self.k_spk * self.amp_hpf.process(u) + 0.0;
                    // 018 FR-010: the samples the knee or the ceiling changes.
                    let clipped = if self.batt_count_at_or_above {
                        xin.abs() >= self.knee_lo
                    } else {
                        xin.abs() > self.knee_lo
                    };
                    if clipped {
                        self.amp_clip_count += 1;
                    }
                    if self.batt_scale_sample {
                        // The mutant (research R-07, restated at implementation): the knee's
                        // output scaled by the factor with the thresholds fixed, D3's rejected
                        // "output scale follows the ceiling" (scaling the input and the output
                        // both is bit-identical after the f32 narrowing, so it is no mutant).
                        knee(xin, self.knee_start) * self.batt
                    } else {
                        knee_b(xin, self.batt, self.knee_lo, self.knee_hi, self.knee_den)
                    }
                } else {
                    u
                };
                if self.tap == Tap::AfterVolumeAmp {
                    return a;
                }
                // Stage 12: the speaker template.
                if self.run_speaker {
                    self.speaker_shelf.process(self.speaker_res.process(a))
                } else {
                    a
                }
            }
            Route::Earphone => {
                // Stage 11 on the earphone route: the volume control's output.
                let v = if self.run_amp {
                    self.k_ear * u + 0.0
                } else {
                    u
                };
                if self.tap == Tap::AfterVolumeAmp {
                    return v;
                }
                // Stage 13: the follower's two coupling corners; no clipper (A-032).
                if self.run_earphone {
                    self.ear_out.process(self.ear_in.process(v))
                } else {
                    v
                }
            }
        }
    }

    /// Zeroes every section's state and the meter accumulators; the controls are kept. Spec
    /// 011 FR-005: a transition in progress completes at once (the scalars and the knob at its
    /// target) and the chain is fresh, so the next `set_controls` takes its position at once.
    pub(crate) fn reset(&mut self) {
        self.reset_sections();
        self.route_peak = 0.0;
        self.clamp_count = 0;
        self.amp_clip_count = 0;
    }

    /// The sections' part of [`reset`](Self::reset), without the meter accumulators: what a
    /// route switch needs (spec 015 FR-003, research R-05), so that a switch positioned inside
    /// a call keeps the call's route peak and output-clamp count (spec 016). At a block boundary
    /// the two are the same, the accumulators having been zeroed at the previous finalization.
    pub(crate) fn reset_sections(&mut self) {
        self.fresh = true;
        self.ramp_k = self.ramp_n;
        self.k_spk = self.g_to_spk;
        self.k_ear = self.g_to_ear;
        self.knob = self.to;
        // Spec 018 FR-003: a battery transition in progress ends at the switch (the mutant
        // leaves it running).
        if !self.batt_no_at_once_on_route {
            self.batt_k = self.batt_n;
            self.batt = self.batt_to;
            self.recompute_knee();
        }
        self.vol_hpf.reset();
        self.amp_hpf.reset();
        self.speaker_res.reset();
        self.speaker_shelf.reset();
        self.ear_in.reset();
        self.ear_out.reset();
    }

    /// Whether any output stage runs (the drain grows by [`OutputChain::n_out`] if so).
    pub(crate) fn runs_any(&self) -> bool {
        self.run_amp || self.run_speaker || self.run_earphone
    }

    /// The chain's settle in host samples at this rate: the 32 Hz section's state decaying by
    /// 100 dB (research R-08; spec 006 FR-008's criterion).
    pub(crate) fn n_out(&self) -> u32 {
        self.n_out
    }

    /// Test-only: every section's state is exactly 0.0.
    #[cfg(test)]
    pub(crate) fn at_rest(&self) -> bool {
        self.vol_hpf.state() == 0.0
            && self.amp_hpf.state() == 0.0
            && self.speaker_res_at_rest()
            && self.speaker_shelf.state() == 0.0
            && self.ear_in.state() == 0.0
            && self.ear_out.state() == 0.0
    }

    #[cfg(test)]
    fn speaker_res_at_rest(&self) -> bool {
        // Two zero inputs give two exactly-zero outputs only when both states are zero
        // (y₁ = s₁, then y₂ = s₂); no struct equality, which the `op-count` feature breaks.
        let mut b = self.speaker_res;
        b.process(0.0) == 0.0 && b.process(0.0) == 0.0
    }

    /// Test-only: the scalars `(k_spk, k_ear)`.
    #[cfg(test)]
    pub(crate) fn scalars(&self) -> (f64, f64) {
        (self.k_spk, self.k_ear)
    }
}

/// 009 T008 (spec 009 FR-003 – FR-009; research R-03 – R-05, R-08; contracts/output-chain-
/// design.md): the clipper, the loaded law, the chain's semantics and the pinned table.
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // analysis in tests, not the DSP path (001 R-04)
mod tests {
    extern crate std;
    use super::*;
    use crate::stages::output_coeffs::{
        GAIN_LAW_SAMPLES, GROUP_DELAY_1K_EAR_MS, GROUP_DELAY_1K_SPK_MS, KNEE_10PCT_DB, KNEE_TABLE,
        N_OUT, RATES, SPK_1K_DB, SPK_PEAK_HZ, VOLUME_REFERENCE,
    };
    use std::vec::Vec;

    /// The 0.11 default (spec 012 FR-010): the output chain's checks run with stages 1 – 2
    /// bypassed.
    fn settings(rate: u32) -> Settings {
        crate::test_util::settings_011(rate)
    }

    /// Spec 015 research R-05: a route switch resets the sections only, so a switch inside a
    /// call keeps the call's meter accumulators; the full reset zeroes them.
    #[test]
    fn reset_sections_keeps_the_accumulators() {
        let mut ch = OutputChain::new(&settings(48_000), false);
        ch.route_peak = 1.5;
        ch.clamp_count = 7;
        ch.reset_sections();
        assert_eq!((ch.route_peak, ch.clamp_count), (1.5, 7));
        assert!(ch.fresh);
        ch.reset();
        assert_eq!((ch.route_peak, ch.clamp_count), (0.0, 0));
        // A route change inside a call goes through reset_sections (the accumulators kept).
        ch.route_peak = 2.0;
        ch.clamp_count = 3;
        ch.set_controls(Route::Earphone, 0.5);
        assert_eq!((ch.route_peak, ch.clamp_count), (2.0, 3));
        assert_eq!(ch.route, Route::Earphone);
    }

    fn settings_output(rate: u32, o: OutputSettings) -> Settings {
        let mut s = crate::test_util::settings_011(rate); // 012 FR-010
        s.output = o;
        s
    }

    /// Peak and THD (fraction) of a 1 kHz sine `level_db` re the ceiling through the knee at
    /// 48 kHz over whole cycles (the script's method, `knee_figures`).
    fn knee_figures(level_db: f64) -> (f64, f64) {
        let (fs, f0, cycles) = (48_000usize, 1000usize, 100usize);
        let n = fs / f0 * cycles;
        let amp = 10f64.powf(level_db / 20.0);
        let y: Vec<f64> = (0..n)
            .map(|i| {
                knee(
                    amp * (core::f64::consts::TAU * i as f64 / 48.0).sin(),
                    c::KNEE_START,
                )
            })
            .collect();
        let peak = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        // The harmonic amplitudes by direct DFT at the exact bins.
        let mag = |h: usize| -> f64 {
            let w = core::f64::consts::TAU * (h as f64) / 48.0;
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in y.iter().enumerate() {
                re += v * (w * i as f64).cos();
                im -= v * (w * i as f64).sin();
            }
            (re * re + im * im).sqrt() / n as f64
        };
        let fund = mag(1);
        let harm: f64 = (2..=24).map(|h| mag(h).powi(2)).sum::<f64>().sqrt();
        (peak, harm / fund)
    }

    /// FR-006 (A-031, low): the six rows of the script's knee table within 1e-6 (peak) and
    /// 0.05 percentage points (THD), and the 10 % crossing within 0.05 dB.
    #[test]
    fn knee_reproduces_the_pinned_table() {
        for row in KNEE_TABLE {
            let (peak, thd) = knee_figures(row[0]);
            assert!(
                (peak - row[1]).abs() < 1e-6,
                "{} dB: peak {peak} vs {}",
                row[0],
                row[1]
            );
            assert!(
                (thd * 100.0 - row[2]).abs() < 0.05,
                "{} dB: THD {} % vs {} %",
                row[0],
                thd * 100.0,
                row[2]
            );
        }
        // The 10 % crossing by bisection between +1 and +3 dB.
        let (mut lo, mut hi) = (1.0f64, 3.0f64);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if knee_figures(mid).1 < 0.10 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        assert!(
            (0.5 * (lo + hi) - KNEE_10PCT_DB).abs() < 0.05,
            "{} vs {KNEE_10PCT_DB}",
            0.5 * (lo + hi)
        );
        // Pinned figures the script printed 2026-10-15 (never the Rust's own output).
        assert!((KNEE_10PCT_DB - 2.266).abs() < 0.01);
        assert_eq!(KNEE_TABLE.len(), 6);
    }

    /// FR-006: symmetric, never above 1.0, C¹ at both joints, `+0.0` at zero.
    #[test]
    fn knee_is_symmetric_bounded_and_smooth() {
        let k = c::KNEE_START;
        for i in 0..1000 {
            let x = i as f64 * 0.003;
            assert_eq!(knee(-x, k), -knee(x, k), "{x}");
            assert!(knee(x, k) <= 1.0, "{x}");
        }
        for x in [1e3, 1e6, f64::MAX] {
            assert_eq!(knee(x, k), 1.0);
            assert_eq!(knee(-x, k), -1.0);
        }
        let e = 1e-6;
        let slope = |at: f64| (knee(at + e, k) - knee(at - e, k)) / (2.0 * e);
        assert!((slope(k) - 1.0).abs() < 1e-4, "{}", slope(k));
        assert!(slope(2.0 - k).abs() < 1e-4, "{}", slope(2.0 - k));
        assert_eq!(knee(k, k), k, "continuous at the knee");
        assert_eq!(knee(0.0, k).to_bits(), 0.0f64.to_bits(), "+0.0");
        // A −0.0 input gives −0.0 (the sign is restored as is); the chain adds +0.0 before the
        // knee so position 0 writes +0.0 (`bypass_semantics_and_taps`).
    }

    /// FR-003 (A-044, A-043): the loaded law at the six pinned positions, unity at the
    /// reference position within 1e-7 after the `f32` rounding (coach Q10), exactly 0.0 at 0,
    /// exactly 1.0 for the earphone at 1, monotonic.
    #[test]
    fn loaded_law_matches_the_pinned_samples() {
        for row in GAIN_LAW_SAMPLES {
            let (p, gs, ge) = (row[0], row[1], row[2]);
            assert!(
                (g_spk(p) - gs).abs() <= 1e-12 * gs,
                "{p}: {} vs {gs}",
                g_spk(p)
            );
            assert!(
                (g_ear(p) - ge).abs() <= 1e-12 * ge,
                "{p}: {} vs {ge}",
                g_ear(p)
            );
        }
        let p_ref = f64::from(VOLUME_REFERENCE as f32);
        assert!((k_spk(p_ref) - 1.0).abs() <= 1e-7, "{}", k_spk(p_ref));
        assert_eq!(VOLUME_REFERENCE as f32, 0.133_451_74_f32); // printed 2026-10-15
        assert_eq!(k_spk(0.0), 0.0);
        assert_eq!(k_ear(0.0), 0.0);
        assert_eq!(k_ear(1.0), 1.0);
        assert!((20.0 * g_spk(1.0).log10() - 22.67).abs() < 0.01); // 2·68/10 (S-002, S-006)
        let mut last = (-1.0, -1.0);
        for i in 0..=100 {
            let p = i as f64 / 100.0;
            let now = (k_spk(p), k_ear(p));
            assert!(now.0 > last.0 && now.1 > last.1, "monotonic at {p}");
            last = now;
        }
    }

    fn run(chain: &mut OutputChain, x: &[f64]) -> Vec<f64> {
        x.iter().map(|&v| chain.process(v)).collect()
    }

    /// FR-009: a route change resets both branches' sections; a volume change only the scalars,
    /// which since spec 011 move from the first processed sample and reach the target at
    /// transition sample N (011 FR-001; tasks.md T019).
    #[test]
    fn route_change_resets_sections_and_volume_change_does_not() {
        let mut ch = OutputChain::new(&settings(48_000), false);
        let burst: Vec<f64> = (0..480).map(|i| if i < 48 { 0.5 } else { 0.0 }).collect();
        run(&mut ch, &burst);
        assert!(
            !ch.at_rest(),
            "the speaker branch and the 32 Hz corner hold state"
        );
        let before = ch.clone();
        ch.set_controls(Route::Speaker, 0.5);
        assert_eq!(
            ch.vol_hpf, before.vol_hpf,
            "a volume change leaves the state"
        );
        assert_eq!(ch.amp_hpf, before.amp_hpf);
        assert_eq!(
            ch.scalars(),
            before.scalars(),
            "011: nothing moves before a sample"
        );
        ch.process(0.0);
        assert_ne!(
            ch.scalars(),
            before.scalars(),
            "011: the first transition sample"
        );
        for _ in 1..n_at(48_000) {
            ch.process(0.0);
        }
        assert_eq!(
            ch.scalars().0,
            k_spk(0.5),
            "011: the target exactly at sample N"
        );
        ch.set_controls(Route::Earphone, 0.5);
        assert!(ch.at_rest(), "a route change resets every section");
        run(&mut ch, &burst);
        assert!(!ch.at_rest());
        ch.set_controls(Route::Speaker, 0.5);
        assert!(ch.at_rest(), "and so does the return");
    }

    /// Edge case "volume position 0" (research R-08): after a 1 ms burst at 48 kHz, 100 ms of
    /// zeros leave the route output below −120 dBFS and 400 ms leave every section's state
    /// exactly 0.0 (the flush), on both routes.
    #[test]
    fn zeros_flush_every_section_to_exact_zero() {
        for route in [Route::Speaker, Route::Earphone] {
            let mut ch = OutputChain::new(&settings(48_000), false);
            ch.set_controls(route, 1.0);
            let burst: Vec<f64> = (0..48).map(|_| 0.5).collect();
            run(&mut ch, &burst);
            let tail = run(&mut ch, &std::vec![0.0; 4800]);
            assert!(
                tail[4799].abs() < 1e-6,
                "{route:?}: {} at 100 ms",
                tail[4799]
            );
            run(&mut ch, &std::vec![0.0; 48_000 * 4 / 10 - 4800]);
            assert!(ch.at_rest(), "{route:?}: flushed by 400 ms");
            assert_eq!(ch.process(0.0).to_bits(), 0.0f64.to_bits());
        }
    }

    /// FR-002, FR-011 (data-model.md › bypasses): the bypass semantics and the taps, sample
    /// exact, on a deterministic noise input.
    #[test]
    fn bypass_semantics_and_taps() {
        let mut seed = 0x0D60_u64;
        let noise: Vec<f64> = (0..1000)
            .map(|_| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((seed >> 11) as f64 / (1u64 << 53) as f64) * 0.5 - 0.25
            })
            .collect();
        // Everything off: the identity, and `runs_any` false.
        let mut off = OutputChain::new(&settings_output(48_000, OutputSettings::BYPASSED), false);
        assert!(!off.runs_any());
        assert_eq!(run(&mut off, &noise), noise);
        // The one-pass at a tap before the chain: the identity too.
        let mut s = settings(48_000);
        s.tap = Tap::AfterPlayback;
        let mut before = OutputChain::new(&s, true);
        assert!(!before.runs_any());
        assert_eq!(run(&mut before, &noise), noise);
        for tap in [
            Tap::AfterVas,
            Tap::AfterDecoder,
            Tap::AfterRecord,
            Tap::AfterAgc,
        ] {
            s.tap = tap;
            assert!(!OutputChain::new(&s, true).runs_any(), "{tap:?}");
        }
        // Stage 11 off, speaker on: `a = y`, then the template.
        let o = OutputSettings {
            volume_amp_enabled: false,
            speaker_enabled: true,
            earphone_enabled: true,
            clamp_enabled: false,
        };
        let mut ch = OutputChain::new(&settings_output(48_000, o), false);
        ch.set_controls(Route::Speaker, 0.0); // the position is ignored
        let mut tmpl = (
            Biquad::from_sos(c::SPK_RES[4]),
            FirstOrder::new(c::SPK_SHELF[4]),
        );
        let want: Vec<f64> = noise
            .iter()
            .map(|&v| tmpl.1.process(tmpl.0.process(v)))
            .collect();
        assert_eq!(run(&mut ch, &noise), want);
        // Stage 12 off: `s = a` (the amplifier path alone), `AfterVolumeAmp` = `RouteOutput`.
        let o = OutputSettings {
            volume_amp_enabled: true,
            speaker_enabled: false,
            earphone_enabled: true,
            clamp_enabled: false,
        };
        let mut a = OutputChain::new(&settings_output(48_000, o), false);
        let mut s = settings_output(48_000, o);
        s.tap = Tap::AfterVolumeAmp;
        let mut a_tap = OutputChain::new(&s, true);
        let ya = run(&mut a, &noise);
        assert_eq!(ya, run(&mut a_tap, &noise));
        let (mut vol, mut amp) = (
            FirstOrder::new(c::VOL_HPF[4]),
            FirstOrder::new(c::AMP_HPF[4]),
        );
        let k = k_spk(f64::from(VOLUME_REFERENCE as f32));
        let want: Vec<f64> = noise
            .iter()
            .map(|&v| knee(k * amp.process(vol.process(v)) + 0.0, c::KNEE_START))
            .collect();
        assert_eq!(ya, want);
        // Stage 13 off on the earphone route: `e = K_EAR · u`; and `AfterVolumeAmp` there is the
        // volume control's output.
        let o = OutputSettings {
            volume_amp_enabled: true,
            speaker_enabled: true,
            earphone_enabled: false,
            clamp_enabled: false,
        };
        let mut e = OutputChain::new(&settings_output(48_000, o), false);
        e.set_controls(Route::Earphone, 0.5);
        let mut s = settings(48_000);
        s.tap = Tap::AfterVolumeAmp;
        let mut e_tap = OutputChain::new(&s, true);
        e_tap.set_controls(Route::Earphone, 0.5);
        let ye = run(&mut e, &noise);
        assert_eq!(ye, run(&mut e_tap, &noise));
        let mut vol = FirstOrder::new(c::VOL_HPF[4]);
        let want: Vec<f64> = noise
            .iter()
            .map(|&v| k_ear(0.5) * vol.process(v) + 0.0)
            .collect();
        assert_eq!(ye, want);
        // The named taps equal the route output on their route.
        for (tap, route) in [
            (Tap::AfterSpeaker, Route::Speaker),
            (Tap::AfterEarphone, Route::Earphone),
        ] {
            let mut s = settings(48_000);
            s.tap = tap;
            let mut named = OutputChain::new(&s, true);
            let mut routed = OutputChain::new(&settings(48_000), false);
            named.set_controls(route, 0.7);
            routed.set_controls(route, 0.7);
            assert_eq!(run(&mut named, &noise), run(&mut routed, &noise), "{tap:?}");
        }
        // Position 0 writes +0.0 on both routes.
        for route in [Route::Speaker, Route::Earphone] {
            let mut z = OutputChain::new(&settings(48_000), false);
            z.set_controls(route, 0.0);
            for v in [-0.3, 0.3, 0.0] {
                let y = z.process(v);
                assert_eq!(y.to_bits(), 0.0f64.to_bits(), "{route:?} {v}");
            }
        }
    }

    /// FR-005, FR-008; research R-08: the pinned per-rate figures the script printed
    /// 2026-10-15 (the Rust never pins its own output), the template's peak gain exactly 1.0
    /// and never above at the seven rates, and `N_OUT` recomputed as the 32 Hz pole's −100 dB
    /// decay.
    #[test]
    fn pinned_table_and_template_peak_at_seven_rates() {
        assert_eq!(RATES, SUPPORTED_HOST_RATES);
        assert_eq!(N_OUT, [346, 461, 922, 2539, 2764, 5078, 5527]); // printed 2026-10-15
        let peak_hz = [828.0, 832.0, 836.0, 837.0, 837.0, 838.0, 838.0];
        let one_k = [-0.90, -0.79, -0.69, -0.67, -0.67, -0.66, -0.66];
        let gd_spk = [0.332, 0.324, 0.316, 0.314, 0.314, 0.313, 0.313];
        let gd_ear = [0.072, 0.069, 0.066, 0.065, 0.065, 0.065, 0.065];
        for (i, &fs) in RATES.iter().enumerate() {
            assert!((SPK_PEAK_HZ[i] - peak_hz[i]).abs() < 0.5, "{fs}");
            assert!((SPK_1K_DB[i] - one_k[i]).abs() < 0.01, "{fs}");
            assert!((GROUP_DELAY_1K_SPK_MS[i] - gd_spk[i]).abs() < 0.001, "{fs}");
            assert!((GROUP_DELAY_1K_EAR_MS[i] - gd_ear[i]).abs() < 0.001, "{fs}");
            assert!(GROUP_DELAY_1K_SPK_MS[i] <= 0.5 && GROUP_DELAY_1K_EAR_MS[i] <= 0.5);
            // The template's gain on a 1 Hz grid from the impulse response (65 536 samples
            // hold the whole tail at every rate).
            let mut res = Biquad::from_sos(c::SPK_RES[i]);
            let mut shelf = FirstOrder::new(c::SPK_SHELF[i]);
            let h: Vec<f64> = (0..65_536)
                .map(|n| shelf.process(res.process(if n == 0 { 1.0 } else { 0.0 })))
                .collect();
            // Spec 010 FR-011 (research R-04; T038 N1): the impulse-response L1 norm of the
            // committed sections is the speaker route's provable bound, pinned here against the
            // figures the README and A-042 quote.
            let l1: f64 = h.iter().map(|v| v.abs()).sum();
            let want_l1 = [1.75, 1.88, 2.09, 2.24, 2.25, 2.28, 2.29][i];
            assert!(
                (l1 - want_l1).abs() <= 0.02,
                "{fs}: L1 {l1:.4}, want {want_l1}"
            );
            let fsf = f64::from(fs);
            let mut peak = 0.0f64;
            let mut f = 20.0;
            while f <= fsf / 2.0 {
                let w = core::f64::consts::TAU * f / fsf;
                let (mut re, mut im) = (0.0, 0.0);
                for (n, v) in h.iter().enumerate() {
                    re += v * (w * n as f64).cos();
                    im -= v * (w * n as f64).sin();
                }
                peak = peak.max((re * re + im * im).sqrt());
                f += if f < 2000.0 { 1.0 } else { 10.0 };
            }
            assert!((peak - 1.0).abs() < 1e-6, "{fs}: peak {peak}");
            // N_OUT: the 32 Hz section's pole decays by 100 dB (research R-08).
            let a = -c::VOL_HPF[i][2];
            let n = (1e5f64.ln() / -a.ln()).ceil() as u32;
            assert_eq!(n, N_OUT[i], "{fs}");
            let ch = OutputChain::new(&settings(fs), false);
            assert_eq!(ch.n_out(), N_OUT[i]);
        }
    }

    // ---- Spec 010 (tasks.md T011): the output clamp ----

    /// 010 FR-001, Edge Cases: the bound on a vector that crosses it, ±∞, NaN and −0.0.
    #[test]
    fn bound_replaces_beyond_and_passes_within() {
        // The true neighbours of ±1.0 (T039 R11; `next_up` needs Rust 1.86, the MSRV is 1.85).
        let above = f64::from_bits(1.0f64.to_bits() + 1);
        let below = f64::from_bits(1.0f64.to_bits() - 1);
        let cases: [(f64, f64); 14] = [
            (-2.0, -1.0),
            (-above, -1.0),
            (-1.0, -1.0),
            (-below, -below),
            (-0.0, -0.0),
            (0.0, 0.0),
            (0.5, 0.5),
            (below, below),
            (1.0, 1.0),
            (above, 1.0),
            (2.0, 1.0),
            (f64::INFINITY, 1.0),
            (f64::NEG_INFINITY, -1.0),
            (f64::NAN, 0.0),
        ];
        for (x, want) in cases {
            let got = bound(x, -1.0, 1.0);
            assert_eq!(got.to_bits(), want.to_bits(), "bound({x})");
        }
        assert_eq!(bound(0.75, -0.5, 0.5), 0.5);
        assert_eq!(bound(-0.75, f64::NEG_INFINITY, 0.5), -0.75);
    }

    /// 010 FR-005 (research R-01): bounding in f64 before the narrowing equals bounding the
    /// narrowed f32 afterwards, bit for bit, over a sweep spanning the f64 range and the
    /// neighbourhood of ±1.
    #[test]
    fn bound_commutes_with_the_narrowing() {
        use crate::sanitize::narrow_out;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..100_000u32 {
            let t = f64::from(i) / 100_000.0;
            xs.push(-1e300 + 2e300 * t);
            xs.push(-2.0 + 4.0 * t);
            xs.push(1.0 + (t - 0.5) * 1e-6);
            xs.push(-1.0 + (t - 0.5) * 1e-6);
        }
        xs.extend([f64::MAX, f64::MIN, f64::from(f32::MAX) * 2.0, 1e-40, -1e-40]);
        for x in xs {
            let a = narrow_out(bound(x, -1.0, 1.0));
            let b = narrow_out(x).clamp(-1.0, 1.0);
            assert_eq!(a.to_bits(), b.to_bits(), "{x}");
        }
    }

    /// 010 FR-003: the gate is true for the player (every tap) and for the one-pass at the
    /// route output only; the per-stage and earlier taps are never bounded.
    #[test]
    fn clamp_gate_follows_the_tap() {
        let mut o = OutputSettings::DEVICE;
        o.clamp_enabled = true;
        for tap in [
            Tap::RouteOutput,
            Tap::AfterVolumeAmp,
            Tap::AfterSpeaker,
            Tap::AfterEarphone,
            Tap::AfterPlayback,
            Tap::AfterDecoder,
            Tap::AfterVas,
        ] {
            let mut s = settings_output(48_000, o);
            s.tap = tap;
            assert!(OutputChain::new(&s, false).clamp, "player, {tap:?}");
            assert_eq!(
                OutputChain::new(&s, true).clamp,
                tap == Tap::RouteOutput,
                "one-pass, {tap:?}"
            );
        }
        let s = settings_output(48_000, OutputSettings::DEVICE);
        assert!(!OutputChain::new(&s, false).clamp);
        assert!(!OutputChain::new(&s, true).clamp);
    }

    /// 010 Edge Cases "Output chain bypassed": a chain with every stage off at the route
    /// output and the option on returns the bounded sample; off, the sample itself. The rest
    /// state, the settle and `runs_any` are unchanged by the option.
    #[test]
    fn bypassed_chain_is_bounded_at_the_route_output() {
        let mut o = OutputSettings::BYPASSED;
        o.clamp_enabled = true;
        let s = settings_output(48_000, o);
        let mut on = OutputChain::new(&s, true);
        assert!(!on.active && on.clamp);
        assert_eq!(on.process(1.5), 1.0);
        assert_eq!(on.process(-1.5), -1.0);
        assert_eq!(on.process(0.25), 0.25);
        let mut off = OutputChain::new(&settings_output(48_000, OutputSettings::BYPASSED), true);
        assert_eq!(off.process(1.5), 1.5);
        assert!(on.at_rest() && off.at_rest());
        assert_eq!(on.n_out(), off.n_out());
        assert_eq!(on.runs_any(), off.runs_any());
        // The device chain on the speaker route at position 1 on a full-scale square: the
        // template rings above 1.0 without the option and is bounded with it (the identity).
        let mut d = OutputSettings::DEVICE;
        d.clamp_enabled = true;
        let mut chain_on = OutputChain::new(&settings_output(48_000, d), false);
        let mut chain_off =
            OutputChain::new(&settings_output(48_000, OutputSettings::DEVICE), false);
        chain_on.set_controls(Route::Speaker, 1.0);
        chain_off.set_controls(Route::Speaker, 1.0);
        let mut exceeded = false;
        for i in 0..48_000usize {
            let x = if (i / 24) % 2 == 0 { 1.0 } else { -1.0 };
            let a = chain_on.process(x);
            let b = chain_off.process(x);
            exceeded |= b.abs() > 1.0;
            assert_eq!(a.to_bits(), bound(b, -1.0, 1.0).to_bits(), "{i}");
        }
        assert!(
            exceeded,
            "the template rings above the bound on a full-scale square"
        );
    }

    // ---- Spec 011: the volume transition (tasks.md T016; research R-05 (b)) ----

    fn smoothstep(u: f64) -> f64 {
        u * u * (3.0 - 2.0 * u)
    }

    fn law(route: Route, p: f64) -> f64 {
        match route {
            Route::Speaker => k_spk(p),
            Route::Earphone => k_ear(p),
        }
    }

    fn active_scalar(ch: &OutputChain, route: Route) -> f64 {
        let (spk, ear) = ch.scalars();
        if route == Route::Speaker { spk } else { ear }
    }

    fn n_at(rate: u32) -> usize {
        crate::output::volume_transition_samples(rate).unwrap() as usize
    }

    /// A chain on `route` at `from`, one sample processed (no longer fresh), then `to` set:
    /// the active scalar after each of the next N + 2 samples.
    fn transition(rate: u32, route: Route, from: f32, to: f32) -> Vec<f64> {
        let mut ch = OutputChain::new(&settings(rate), false);
        ch.set_controls(route, from);
        ch.process(0.0);
        ch.set_controls(route, to);
        (0..n_at(rate) + 2)
            .map(|_| {
                ch.process(0.0);
                active_scalar(&ch, route)
            })
            .collect()
    }

    /// The formula of research R-02 / R-03 recomputed: the law at the smoothstep's knob, held
    /// within the previous gain and the target's; sample N the target exactly.
    fn expected(rate: u32, route: Route, from: f32, to: f32) -> Vec<f64> {
        let n = n_at(rate);
        let (pf, pt) = (f64::from(from), f64::from(to));
        let (g0, g1) = (law(route, pf), law(route, pt));
        let up = g1 > g0;
        let inv_n = 1.0 / n as f64;
        let mut prev = g0;
        let mut out = Vec::new();
        for k in 1..=n + 2 {
            let g = if k >= n {
                g1
            } else {
                hold(
                    law(route, pf + (pt - pf) * smoothstep(k as f64 * inv_n)),
                    prev,
                    g1,
                    up,
                )
            };
            out.push(g);
            prev = g;
        }
        out
    }

    fn monotonic_within(seq: &[f64], start: f64, end: f64) -> bool {
        let up = end > start;
        let (lo, hi) = if up { (start, end) } else { (end, start) };
        let mut prev = start;
        seq.iter().all(|&g| {
            let ok = (lo..=hi).contains(&g) && if up { g >= prev } else { g <= prev };
            prev = g;
            ok
        })
    }

    /// FR-001 – FR-003 (US1 AS1 (b)): the gain sequence is the formula exactly, monotonic,
    /// within the ends and exactly 0.10's scalar from sample N, on R-03's 21 × 21 grid at
    /// 48 kHz and four pairs at every other rate, on both routes.
    #[test]
    fn transition_gain_sequence_is_the_formula() {
        let grid: Vec<f32> = (0..=20).map(|i| (f64::from(i) * 0.05) as f32).collect();
        for &rate in &SUPPORTED_HOST_RATES {
            let pairs: Vec<(f32, f32)> = if rate == 48_000 {
                grid.iter()
                    .flat_map(|&a| grid.iter().map(move |&b| (a, b)))
                    .filter(|(a, b)| a != b)
                    .collect()
            } else {
                std::vec![(0.1, 1.0), (1.0, 0.1), (0.0, 0.5), (0.7, 0.0)]
            };
            let n = n_at(rate);
            for route in [Route::Speaker, Route::Earphone] {
                for &(a, b) in &pairs {
                    let got = transition(rate, route, a, b);
                    assert_eq!(
                        got,
                        expected(rate, route, a, b),
                        "{rate} {route:?} {a} -> {b}"
                    );
                    let (g0, g1) = (law(route, f64::from(a)), law(route, f64::from(b)));
                    assert!(
                        monotonic_within(&got, g0, g1),
                        "{rate} {route:?} {a} -> {b}"
                    );
                    assert_eq!(got[n - 1].to_bits(), g1.to_bits(), "exact at N");
                    assert_ne!(
                        got[n - 2].to_bits(),
                        g1.to_bits(),
                        "N - 1 differs ({rate} {a} -> {b})"
                    );
                }
            }
        }
    }

    /// FR-003 (research R-03, plan review Q2): a retarget to a position within rounding of the
    /// moving knob stays exactly monotonic (the running hold).
    #[test]
    fn retarget_within_rounding_of_the_knob_is_monotonic() {
        for route in [Route::Speaker, Route::Earphone] {
            for half in [3usize, 480, 959, 1500] {
                for nudge in [-2i32, -1, 0, 1, 2] {
                    let mut ch = OutputChain::new(&settings(96_000), false);
                    ch.set_controls(route, 0.3);
                    ch.process(0.0);
                    ch.set_controls(route, 0.7);
                    for _ in 0..half {
                        ch.process(0.0);
                    }
                    let knob = ch.knob as f32;
                    let to = f32::from_bits((knob.to_bits() as i32 + nudge) as u32);
                    let start = active_scalar(&ch, route);
                    ch.set_controls(route, to);
                    let seq: Vec<f64> = (0..n_at(96_000) + 1)
                        .map(|_| {
                            ch.process(0.0);
                            active_scalar(&ch, route)
                        })
                        .collect();
                    let end = law(route, f64::from(to));
                    let up = end > start;
                    let mut prev = start;
                    for &g in &seq {
                        assert!(
                            if up { g >= prev } else { g <= prev },
                            "{route:?} {half} {nudge}"
                        );
                        prev = g;
                    }
                    assert_eq!(seq.last().unwrap().to_bits(), end.to_bits());
                }
            }
        }
    }

    /// FR-003: a retarget mid-transition starts from the knob's current position: the first
    /// step after it is no larger than the first transition's largest step.
    #[test]
    fn retarget_starts_from_the_current_knob() {
        for &rate in &SUPPORTED_HOST_RATES {
            for route in [Route::Speaker, Route::Earphone] {
                let mut ch = OutputChain::new(&settings(rate), false);
                ch.set_controls(route, 0.1);
                ch.process(0.0);
                ch.set_controls(route, 1.0);
                let mut prev = active_scalar(&ch, route);
                let mut largest = 0.0f64;
                for _ in 0..n_at(rate) / 2 {
                    ch.process(0.0);
                    let g = active_scalar(&ch, route);
                    largest = largest.max((g - prev).abs());
                    prev = g;
                }
                ch.set_controls(route, 0.4);
                ch.process(0.0);
                let step = (active_scalar(&ch, route) - prev).abs();
                assert!(step <= largest, "{rate} {route:?}: {step} > {largest}");
            }
        }
    }

    /// FR-005 (research R-04): a fresh chain (after `new()`, after `reset()`, after a
    /// zero-length block), an inactive chain and stage 11 off take the position at once; a
    /// route change mid-transition completes it at the new position.
    #[test]
    fn start_at_once_rules() {
        let route = Route::Earphone;
        let at_once = |ch: &mut OutputChain, p: f32| {
            ch.set_controls(route, p);
            ch.process(0.0);
            active_scalar(ch, route) == law(route, f64::from(p))
        };
        let mut ch = OutputChain::new(&settings(48_000), false);
        assert!(at_once(&mut ch, 0.5), "after new()");
        ch.set_controls(route, 0.9); // a transition, not yet run
        ch.reset();
        assert!(at_once(&mut ch, 0.2), "after reset()");
        // A zero-length block: two set_controls with no sample between keep the chain fresh.
        let mut z = OutputChain::new(&settings(48_000), false);
        z.set_controls(route, 0.2);
        assert!(
            at_once(&mut z, 0.8),
            "a zero-length block does not consume the first block"
        );
        // Stage 11 off: the position has no effect and is taken at once.
        let mut s = settings(48_000);
        s.output.volume_amp_enabled = false;
        let mut off = OutputChain::new(&s, false);
        off.set_controls(route, 0.3);
        off.process(0.0);
        off.set_controls(route, 0.6);
        assert_eq!(off.scalars().1, k_ear(f64::from(0.6f32)));
        // An inactive chain (the one-pass at a tap before the chain).
        let mut t = settings(48_000);
        t.tap = Tap::AfterPlayback;
        let mut before = OutputChain::new(&t, true);
        before.set_controls(route, 0.3);
        before.process(0.0);
        before.set_controls(route, 0.6);
        assert_eq!(before.scalars().1, k_ear(f64::from(0.6f32)));
        // A route change mid-transition completes it at the new position.
        let mut r = OutputChain::new(&settings(48_000), false);
        r.set_controls(Route::Speaker, 0.1);
        r.process(0.0);
        r.set_controls(Route::Speaker, 1.0);
        for _ in 0..100 {
            r.process(0.0);
        }
        r.set_controls(Route::Earphone, 1.0);
        r.process(0.0);
        assert_eq!(r.scalars(), (k_spk(1.0), k_ear(1.0)));
        assert_eq!(r.ramp_k, r.ramp_n);
    }

    /// FR-008: a transition to 0 ends with both scalars exactly +0.0 at sample N.
    #[test]
    fn transition_to_zero_is_exact_at_n() {
        for &rate in &SUPPORTED_HOST_RATES {
            for route in [Route::Speaker, Route::Earphone] {
                let seq = transition(rate, route, 1.0, 0.0);
                let n = n_at(rate);
                assert!(seq[n - 2] > 0.0, "{rate} {route:?}");
                assert_eq!(seq[n - 1].to_bits(), 0.0f64.to_bits(), "{rate} {route:?}");
            }
        }
    }

    /// FR-001, FR-003 (post-implementation review N1): a retarget restarts the formula from the
    /// knob's current position and the active scalar at the retarget, in both directions: the
    /// sequence after the retarget equals the formula with `from` = the knob and the running
    /// hold starting at the scalar, so a wrong start (e.g. `from = to`) fails.
    #[test]
    fn retarget_restarts_the_formula_from_the_knob() {
        let rate = 48_000;
        let n = n_at(rate);
        for route in [Route::Speaker, Route::Earphone] {
            for (a, b, c) in [(0.1f32, 1.0f32, 0.4f32), (1.0, 0.1, 0.6), (0.2, 0.9, 0.95)] {
                let mut ch = OutputChain::new(&settings(rate), false);
                ch.set_controls(route, a);
                ch.process(0.0);
                ch.set_controls(route, b);
                for _ in 0..n / 2 {
                    ch.process(0.0);
                }
                let (knob, g0) = (ch.knob, active_scalar(&ch, route));
                ch.set_controls(route, c);
                let pt = f64::from(c);
                let g1 = law(route, pt);
                let up = g1 > g0;
                let inv_n = 1.0 / n as f64;
                let mut prev = g0;
                for k in 1..=n {
                    ch.process(0.0);
                    let want = if k == n {
                        g1
                    } else {
                        hold(
                            law(route, knob + (pt - knob) * smoothstep(k as f64 * inv_n)),
                            prev,
                            g1,
                            up,
                        )
                    };
                    assert_eq!(
                        active_scalar(&ch, route).to_bits(),
                        want.to_bits(),
                        "{route:?} {a}->{b}->{c} k={k}"
                    );
                    prev = want;
                }
            }
        }
    }
}

/// Spec 018 (research R-02, R-03): the scaled clipper and the battery transition.
#[cfg(test)]
mod battery_tests {
    extern crate std;
    use super::*;
    use crate::battery::ceiling_factor;
    use crate::settings::Settings;
    use std::vec::Vec;

    fn chain(rate: u32) -> OutputChain {
        let mut s = Settings::new(rate);
        s.tap = Tap::AfterVolumeAmp;
        let mut c = OutputChain::new(&s, false);
        c.set_controls(Route::Speaker, crate::output::VOLUME_6DB_OVER);
        c
    }

    fn knee_b_at(x: f64, b: f64) -> f64 {
        let k = c::KNEE_START;
        knee_b(x, b, k * b, (2.0 - k) * b, 4.0 * (1.0 - k) * b)
    }

    /// FR-002, FR-005: at b = 1.0 the scaled clipper is `knee` bit for bit over a sweep.
    #[test]
    fn knee_b_equals_knee_at_unity() {
        let k = c::KNEE_START;
        for i in 0..=20_000 {
            let x = -2.5 + 5.0 * f64::from(i) / 20_000.0;
            assert_eq!(knee_b_at(x, 1.0).to_bits(), knee(x, k).to_bits(), "{x}");
        }
        assert_eq!(knee_b_at(-0.0, 1.0).to_bits(), knee(-0.0, k).to_bits());
    }

    /// FR-002: at b = 0.5 the sample passes below 0.425, the ceiling is 0.5 at and above
    /// 0.575, the parabola is C¹ at both joints.
    #[test]
    fn knee_b_scales_the_thresholds() {
        // Hardware review B7 (post-implementation): the joints at every law point, not at 0.5
        // alone (finite differences; T004's wording restated).
        for volts in [3.0f32, 2.8, 2.6, 2.4, 2.2] {
            let b = f64::from(ceiling_factor(volts));
            let (lo, hi) = (c::KNEE_START * b, (2.0 - c::KNEE_START) * b); // the chain's own products
            for x in [0.0, 0.1 * b, 0.5 * b, 0.999 * lo, -0.8 * b] {
                assert_eq!(knee_b_at(x, b).to_bits(), x.to_bits(), "{volts} V {x}");
            }
            for x in [hi, 1.2 * b, 1.5 * b, 2.0] {
                assert_eq!(knee_b_at(x, b), b, "{volts} V {x}");
                assert_eq!(knee_b_at(-x, b), -b, "{volts} V {x}");
            }
            let h = 1e-6;
            // Continuity and matching slopes at the joints (1 at the start, 0 at the ceiling).
            assert!(
                (knee_b_at(lo + h, b) - knee_b_at(lo - h, b)).abs() < 1e-5,
                "{volts} V"
            );
            assert!(
                ((knee_b_at(lo + h, b) - knee_b_at(lo, b)) / h - 1.0).abs() < 1e-3,
                "{volts} V"
            );
            assert!((knee_b_at(hi - h, b) - b).abs() < 1e-9, "{volts} V");
            assert!(
                ((knee_b_at(hi, b) - knee_b_at(hi - h, b)) / h).abs() < 1e-3,
                "{volts} V"
            );
        }
    }

    /// FR-003: the factor follows spec 011's smoothstep from the factor in force to the target,
    /// exactly the target at sample N; at 44.1 kHz sample ⌊N/4⌋ carries s(220 / 882).
    #[test]
    fn battery_transition_shape() {
        for rate in [48_000u32, 44_100] {
            let mut ch = chain(rate);
            let n = crate::output::volume_transition_samples(rate).unwrap() as usize;
            let _ = ch.process(0.0); // the first sample ends "fresh"
            let b0 = ch.battery_factor();
            assert_eq!(b0, 1.0);
            ch.set_battery(2.4);
            let b1 = f64::from(ceiling_factor(2.4));
            let mut trace = Vec::new();
            for _ in 0..n + 10 {
                let _ = ch.process(0.0);
                trace.push(ch.battery_factor());
            }
            let inv = 1.0 / n as f64;
            for (j, &b) in trace.iter().enumerate().take(n) {
                let k = (j + 1) as f64;
                let want = if j + 1 == n {
                    b1
                } else {
                    let u = k * inv;
                    b0 + (b1 - b0) * (u * u * (3.0 - 2.0 * u))
                };
                assert_eq!(b.to_bits(), want.to_bits(), "{rate} Hz sample {}", j + 1);
            }
            assert!(trace.windows(2).all(|w| w[1] <= w[0]), "monotonic");
            assert_eq!(trace[n - 1], b1);
            assert_eq!(trace[n + 5], b1);
            let q = n / 4;
            let u = q as f64 * inv;
            let s = u * u * (3.0 - 2.0 * u);
            assert!(
                (trace[q - 1] - (b0 + (b1 - b0) * s)).abs() < 1e-12,
                "{rate} Hz ⌊N/4⌋"
            );
            if rate == 44_100 {
                assert_eq!(q, 220);
                assert!((s - 0.155_613).abs() < 1e-5);
            } else {
                assert!((s - 0.156_25).abs() < 1e-12);
            }
        }
    }

    /// Research R-01: a request equal to the target does not restart a transition.
    #[test]
    fn set_battery_is_idempotent() {
        let mut ch = chain(48_000);
        let _ = ch.process(0.0);
        ch.set_battery(2.4);
        for _ in 0..100 {
            let _ = ch.process(0.0);
        }
        let mid = ch.battery_factor();
        ch.set_battery(2.4); // the per-call plain apply
        let _ = ch.process(0.0);
        assert!(ch.battery_factor() < mid, "the transition continued");
        assert_ne!(ch.battery_factor(), mid);
        // A different request restarts from the factor in force.
        let here = ch.battery_factor();
        ch.set_battery(3.0);
        let _ = ch.process(0.0);
        assert!(ch.battery_factor() > here && ch.battery_factor() < 1.0);
    }

    /// FR-003: at once on a fresh chain, with stage 11 off, on the earphone route, and at a
    /// route change (which ends a transition in progress).
    #[test]
    fn at_once_cases() {
        let mut ch = chain(48_000);
        ch.set_battery(2.4); // fresh
        assert_eq!(ch.battery_factor(), f64::from(ceiling_factor(2.4)));
        let _ = ch.process(0.0);
        assert_eq!(ch.battery_factor(), f64::from(ceiling_factor(2.4)));
        // Stage 11 off.
        let mut s = Settings::new(48_000);
        s.output.volume_amp_enabled = false;
        let mut off = OutputChain::new(&s, false);
        let _ = off.process(0.0);
        off.set_battery(2.2);
        assert_eq!(off.battery_factor(), f64::from(ceiling_factor(2.2)));
        // The earphone route: at once, and in force when the speaker returns.
        let mut ear = chain(48_000);
        ear.set_controls(Route::Earphone, 1.0);
        let _ = ear.process(0.0);
        ear.set_battery(2.4);
        assert_eq!(ear.battery_factor(), f64::from(ceiling_factor(2.4)));
        ear.set_controls(Route::Speaker, 1.0);
        let _ = ear.process(0.0);
        assert_eq!(ear.battery_factor(), f64::from(ceiling_factor(2.4)));
        // A route change ends a transition in progress.
        let mut sw = chain(48_000);
        let _ = sw.process(0.0);
        sw.set_battery(2.4);
        for _ in 0..50 {
            let _ = sw.process(0.0);
        }
        assert!(sw.battery_factor() > f64::from(ceiling_factor(2.4)));
        sw.set_controls(Route::Earphone, 1.0);
        assert_eq!(sw.battery_factor(), f64::from(ceiling_factor(2.4)));
    }
}
