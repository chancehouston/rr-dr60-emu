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
//! per-sample path reads a control. Plain IEEE `f64` arithmetic, no platform math function
//! (FR-012; 001 R-04).

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

/// The amplifier's clipper (research R-04; A-031, low): a symmetric parabolic knee from
/// `k = KNEE_START` of the ceiling to `2 − k`, C¹ at both joints, a hard ceiling of 1.0 above.
/// Plain IEEE arithmetic: compares, multiplies, one divide of constants.
#[inline]
pub(crate) fn knee(x: f64, k: f64) -> f64 {
    let u = if x < 0.0 { -x } else { x };
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

/// The output chain's state for one playback path (data-model.md › `OutputChain` state).
#[derive(Clone, Debug)]
pub(crate) struct OutputChain {
    route: Route,
    k_spk: f64,
    k_ear: f64,
    run_amp: bool,
    run_speaker: bool,
    run_earphone: bool,
    /// The point `process` returns: `RouteOutput` for the player; the configured tap for the
    /// one-pass.
    tap: Tap,
    /// The knee's start: in production always `KNEE_START` (A-031); a field only so the SC-006
    /// mutation tests can move it through the `__test-hooks` feature.
    knee_start: f64,
    /// A scale on the speaker route's scalar: in production always 1.0; the mutation tests'
    /// lever on the level.
    gain_scale: f64,
    vol_hpf: FirstOrder,
    amp_hpf: FirstOrder,
    speaker_res: Biquad,
    speaker_shelf: FirstOrder,
    ear_in: FirstOrder,
    ear_out: FirstOrder,
    n_out: u32,
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
        } = settings.output;
        let mut chain = Self {
            route: Route::Speaker,
            k_spk: 1.0,
            k_ear: 1.0,
            run_amp: enabled && volume_amp_enabled, // A-031 (009 FR-002)
            run_speaker: enabled && speaker_enabled, // A-010 (009 FR-002)
            run_earphone: enabled && earphone_enabled, // A-032 (009 FR-002)
            tap: if honour_tap {
                settings.tap
            } else {
                Tap::RouteOutput
            },
            knee_start: c::KNEE_START, // A-031 (low): the knee's shape
            gain_scale: 1.0,
            vol_hpf: FirstOrder::new(c::VOL_HPF[i]), // A-031: 32 Hz, both routes
            amp_hpf: FirstOrder::new(c::AMP_HPF[i]), // A-031: 159 Hz, speaker route
            speaker_res: Biquad::from_sos(c::SPK_RES[i]), // A-010: the template's resonance
            speaker_shelf: FirstOrder::new(c::SPK_SHELF[i]), // A-010: the template's shelf
            ear_in: FirstOrder::new(c::EAR_IN[i]),   // A-032: 120 Hz nominal
            ear_out: FirstOrder::new(c::EAR_OUT[i]), // A-032: 280 Hz nominal
            n_out: c::N_OUT[i],                      // research R-08
        };
        chain.set_controls(Route::Speaker, c::VOLUME_REFERENCE as f32);
        chain
    }

    /// Copies the controls in at a block boundary (FR-009): a route change resets every
    /// section (the newly selected branch and the one left start from rest); the volume
    /// position only moves the two scalars (the loaded law, A-044; one divide each). At
    /// position 0 both scalars are exactly 0.0.
    pub(crate) fn set_controls(&mut self, route: Route, volume: f32) {
        if route != self.route {
            self.route = route;
            self.reset();
        }
        let p = f64::from(volume);
        self.k_spk = k_spk(p) * self.gain_scale;
        self.k_ear = k_ear(p);
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
        let route = self.route;
        self.set_controls(route, self.volume_for_rescale());
    }

    /// Test-only: the position the current scalars were computed from is not stored (the
    /// controls are re-copied at every block), so a mutation re-applies the reference position;
    /// the harness sets its own position afterwards.
    #[cfg(feature = "__test-hooks")]
    fn volume_for_rescale(&self) -> f32 {
        c::VOLUME_REFERENCE as f32
    }

    /// One host-rate sample through the chain, returning the sample at the configured tap
    /// (data-model.md › The chain in the playback path). With every stage off (or at a tap
    /// before the chain) the sample passes through with no arithmetic.
    #[inline]
    pub(crate) fn process(&mut self, y: f64) -> f64 {
        if !self.runs_any() {
            return y;
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
                    knee(self.k_spk * self.amp_hpf.process(u) + 0.0, self.knee_start)
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

    /// Zeroes every section's state; the controls are kept.
    pub(crate) fn reset(&mut self) {
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

    fn settings(rate: u32) -> Settings {
        Settings::new(rate)
    }

    fn settings_output(rate: u32, o: OutputSettings) -> Settings {
        let mut s = Settings::new(rate);
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

    /// FR-009: a route change resets both branches' sections; a volume change only the scalars.
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
        assert_ne!(ch.scalars(), before.scalars());
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
}
