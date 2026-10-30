//! Record-path limiter: signal-chain stage 3 (spec 005; A-028 adopted, A-034 – A-036).
//!
//! **The circuit (S-002, sheet 3.1).** The microphone amplifier feeds the codec's input amp
//! through R7 (22 kΩ) into a node that R10 (1 kΩ) connects to the amp's inverting input. A
//! shunt transistor Q4 sits on that node, driven by the emitter follower Q5 from the hold
//! capacitor C6 (1 µF). While C6 is discharged the shunt is off and the divider passes the
//! signal at unity; as C6 charges, Q4 conducts and attenuates the node exponentially in the
//! capacitor voltage. C6 is charged by Q6, a half-wave peak detector on the codec input amp's
//! output (GSX) through the R13/R17 divider and the 1.6 V reference, with R12 (390 Ω) limiting
//! the peak charge current, and discharged through R11 (10 MΩ): R11·C6 = 10 s. The result is a
//! **limiter**, not a compressor: unity gain below a knee near the codec's full scale, a flat
//! ceiling above it, a charge **dead time** of a few milliseconds on a loud onset (the capacitor
//! must climb through two base–emitter drops before the shunt acts), an **armed** state for
//! several seconds after a loud sound, and a fast-then-slow **release** of about 1.4 s.
//!
//! **The model (research R-02, R-03).** One state variable, the capacitor voltage `vc`, stepped
//! once per **host** sample before the rate conversion (R-01), with the shunt's gain through the
//! exponential base–emitter law, the detector's view clamped at the input amp's rail, the
//! current-gain- and R12-limited charge, the R11-plus-follower discharge, and an analytic cap
//! that stops the charge where the current sample would no longer exceed the threshold, which is
//! what the continuous loop does within a sample and what keeps a 6 kHz host from
//! over-attenuating. Below a floor the gain is exactly 1.0 and no multiply happens, so sub-knee
//! input passes bit for bit; far below it the capacitor flushes to 0.0, so a recovered stage is
//! bit-identical to a fresh one. Component values are the schematic's (S-002); the transistor
//! parameters, the rail and the polarity are estimates (A-028, A-034, A-035, A-036). The stage is
//! **modeled on the circuit, not measured against a unit**: the dead time is uncertain by about
//! 3× either way, the release by about 40 %, the knee by ±2 dB.
//!
//! **Conventions.** Full-scale units: 1.0 = `V_FS` = 0.7 V peak at GSX (A-030). The sensed
//! polarity is positive input samples (A-036). The reference model
//! `tools/agc-model/limiter_model.py` runs the same equations as a 192 kHz reference of the
//! continuous loop and as this discretization at the seven host rates; the unit tests below pin
//! this module to its figures (R-04). Only basic IEEE operations plus `rr_dr60_detmath::{exp,
//! ln}` per sample, and `sin` at construction, so it is bit-identical on every platform.

use core::f64::consts::LN_10;

use rr_dr60_detmath::{TAU, exp, ln, sin};

use crate::settings::AgcSettings;

// ---- S-002 component values ---------------------------------------------------------------------
/// Hold capacitor (S-002: C6, 1 µF).
const C6: f64 = 1e-6; // S-002
/// Discharge resistor (S-002: R11, 10 MΩ); R11 · C6 = 10 s is the arming memory (A-028).
const R11: f64 = 10e6; // S-002
/// Detector collector resistor (S-002: R12, 390 Ω): caps the peak charge current (A-028).
const R12: f64 = 390.0; // S-002
/// Q6 base divider from GSX (S-002: R13, 100 kΩ).
const R13: f64 = 100e3; // S-002
/// Q6 base divider to the 1.6 V reference (S-002: R17, 330 kΩ).
const R17: f64 = 330e3; // S-002
/// Thevenin resistance at Q6's base, 76.7 kΩ (S-002: R13 ∥ R17).
const RTH: f64 = R13 * R17 / (R13 + R17); // S-002
/// Fraction of the GSX swing reaching Q6's base, 0.767 (S-002: R17 / (R13 + R17)).
const KDIV: f64 = R17 / (R13 + R17); // S-002
/// Series resistor into the shunt node (S-002: R7, 22 kΩ).
const R7: f64 = 22e3; // S-002
/// Shunt node to the codec's inverting input (S-002: R10, 1 kΩ).
const R10: f64 = 1e3; // S-002
/// The codec input amp's input resistance, R7 + R10 (S-002), so the unloaded divider is unity.
const R_IN: f64 = 23e3; // S-002
/// AIN+ reference voltage (S-002).
const V_REF: f64 = 1.6; // S-002
// ---- physics and transistor estimates -----------------------------------------------------------
/// Thermal voltage at room temperature (physics; A-028 records the −0.3 dB / 10 °C knee drift).
const VT: f64 = 0.026; // physics (A-028)
/// Transistor saturation current, Vbe = VT · ln(I / IS) (A-028 estimate).
const IS: f64 = 1e-14; // A-028 estimate
/// Q4 reverse current gain: the saturated shunt's conductance (A-028 estimate).
const BETA_R: f64 = 5.0; // A-028 estimate
/// Q5 current gain: its base current adds to the discharge (A-028 estimate).
const BETA5: f64 = 300.0; // A-028 estimate
/// Codec input-amp swing limit at GSX, the detector's view only (A-028 estimate).
const V_RAIL: f64 = 1.2; // A-028 estimate
/// GSX full scale, peak: 0 dBFS = +3 dBm0 = 0.7 V (A-030).
const V_FS: f64 = 0.7; // A-030
/// Sensed polarity: +1, positive input samples drive the detector (A-036).
const POLARITY: f64 = 1.0; // A-036
// ---- mapping reference points (005 research R-04, R-05) -----------------------------------------
/// Q6 current gain the reference dead time was measured at (005 R-04).
const BETA6_REF: f64 = 300.0; // 005 R-04 (A-034)
/// Reference dead time at `BETA6_REF`, the default knee and 10 dB over it, ms (005 R-04).
const T_REF_MS: f64 = 2.167; // 005 R-04 (A-034)
/// The knee the attack mapping is referenced to, dBFS (005 R-05): where `T_REF_MS` was measured.
/// Not the default knee (−1 dBFS since the post-implementation review); the mapping scales by
/// `I(KNEE_REF) / I(knee)` so the configured dead time holds at any knee.
const KNEE_REF_DBFS: f64 = 1.0; // 005 R-05 (A-028)
/// The release setting that maps to the schematic's discharge (R11 exactly), ms (005 R-05).
const RELEASE_REF_MS: f64 = 1370.0; // 005 R-05 (A-035)
// ---- engineering targets (005 research R-03, R-05) ----------------------------------------------
/// `vc` below this is set to 0.0, so a recovered stage equals a fresh one (005 R-03).
const V_FLUSH: f64 = 1e-6; // engineering target (005 R-03)
/// Attenuation limit, dB: a clamp on `vc` (005 R-03).
const MAX_ATTEN_DB: f64 = 40.0; // engineering target (005 R-03)
/// Points of the detector overdrive sum in the attack mapping (005 R-05).
const OVERDRIVE_POINTS: usize = 360; // engineering target (005 R-05)
/// Guard range for the mapped detector current gain (005 R-05); no setting in range reaches it.
const BETA6_MIN: f64 = 10.0; // engineering target (005 R-05)
const BETA6_MAX: f64 = 10_000.0; // engineering target (005 R-05)
/// The shunt's deviation from unity below which the gain is exactly 1.0: 2⁻²⁴, half an f32 ulp
/// (005 R-03; plan review P4).
const UNITY_DEVIATION: f64 = 1.0 / 16_777_216.0; // engineering target (005 R-03)
// ---- derived -----------------------------------------------------------------------------------
/// Two stacked base–emitter drops (Q5 then Q4): `ib4 = IS · exp(vc / (2 · VT))` (R-02 step 1).
const INV_2VT: f64 = 1.0 / (2.0 * VT); // S-002 topology, A-028
const TWO_VT: f64 = 2.0 * VT; // A-028
/// The shunt's unity constant. Solving R-02 step 1 (`g = (BETA_R + 1) · ib4 / VT`, Q4 ∥ R10,
/// the R7 divider relative to its unloaded value) in closed form gives
/// `a(vc) = 1 / (1 + exp(vc / 2VT) / K1)`, with `K1 = VT · R_IN / ((BETA_R + 1) · R7 · R10 · IS)`.
/// Its inverse is `vc(a) = 2VT · (ln K1 + ln((1 − a) / a))`. At `vc = 0` the deviation from
/// unity is `1 / K1` ≈ 2.2e-9 (R-03).
const K1: f64 = VT * R_IN / ((BETA_R + 1.0) * R7 * R10 * IS); // S-002, A-028
const INV_K1: f64 = 1.0 / K1; // S-002, A-028
/// Detector gain from the GSX voltage to Q6's overdrive, with the sensed polarity (A-036).
const KDIV_POL: f64 = KDIV * POLARITY; // S-002, A-036
const INV_R12: f64 = 1.0 / R12; // S-002

/// The limiter (data-model.md › Limiter): one state variable and the parameters derived from
/// the settings at construction. Creating one allocates nothing; processing never allocates.
#[derive(Clone, Debug)]
pub(crate) struct Limiter {
    /// Hold-capacitor voltage, V; 0.0 when discharged (the fresh state).
    vc: f64,
    /// Detector threshold at Q6's base, V: `KDIV · V_FS · 10^(knee / 20)` (R-05).
    vbe6: f64,
    /// `vc` below which the gain is exactly 1.0 (R-03): 0.171 V.
    v_floor: f64,
    /// `vc` giving `MAX_ATTEN_DB` of attenuation: 1.275 V.
    v_limit: f64,
    /// Integration step over the capacitance: `1 / (host_rate · C6)`.
    dt_over_c6: f64,
    /// `beta6 / RTH`: charge current per volt of overdrive, with `beta6` the detector current
    /// gain from the attack setting (knee-aware, R-05; [`Limiter::beta6_for`]).
    charge_gain: f64,
    /// The two discharge terms, `k_dis / R11` and `k_dis / BETA5`, with `k_dis` the discharge
    /// scale `RELEASE_REF_MS / release_ms` (R-05).
    dis_r11: f64,
    dis_b5: f64,
    /// The detector onset in full-scale units, `vbe6 / (KDIV · V_FS)`: the ceiling (1.122 at
    /// the default knee, +1.00 dBFS).
    ceiling_fs: f64,
    /// `ln(K1)`, for the closed-form inverse of the gain law.
    ln_k1: f64,
    /// Spec 016 (the per-block meters, research R-02): the largest entry voltage of the
    /// current call among the samples that went through the gain law (above the floor), so the
    /// call's deepest applied gain is `a(vc_max)`; 0.0 when none did. Taken and cleared at
    /// each call's finalization.
    vc_max: f64,
    /// Basic operations and transcendental calls so far (bounded-work tests).
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
    #[cfg(feature = "op-count")]
    pub(crate) transcendentals: u64,
}

/// `10^(db / 20)` with detmath.
#[inline]
fn db_to_amplitude(db: f64) -> f64 {
    exp(db * (LN_10 / 20.0))
}

/// Detector threshold voltage for a knee in dBFS: the conduction onset `KDIV · V_FS · 10^(knee/20)`
/// (R-05; 0.479 V at the default −1 dBFS knee, 0.602 V at the +1 dBFS mapping reference; A-028:
/// a hard threshold, the Vbe at the attack's mA charge peaks, re-pointed to the detector's
/// steady-state current after the post-implementation review).
fn vbe6_of_knee(knee_dbfs: f64) -> f64 {
    KDIV * V_FS * db_to_amplitude(knee_dbfs)
}

/// `I(knee)`: the detector's mean overdrive over one cycle of a 1 kHz sine at knee + 10 dB, a
/// fixed `OVERDRIVE_POINTS`-point sum at the bin centres (R-05). 0.1531 V at the default knee
/// (−1 dBFS), 0.1174 V at the +1 dBFS mapping reference.
fn overdrive_index(knee_dbfs: f64) -> f64 {
    let vbe6 = vbe6_of_knee(knee_dbfs);
    let amp = V_FS * db_to_amplitude(knee_dbfs + 10.0);
    let n = OVERDRIVE_POINTS as f64;
    let mut sum = 0.0;
    for k in 0..OVERDRIVE_POINTS {
        let gsx = (amp * sin(TAU * (k as f64 + 0.5) / n)).clamp(-V_RAIL, V_RAIL);
        sum += (KDIV * gsx - vbe6).max(0.0);
    }
    sum / n
}

impl Limiter {
    /// A discharged limiter for validated `settings` at `host_rate_hz` (R-05 mapping).
    pub(crate) fn new(settings: &AgcSettings, host_rate_hz: u32) -> Self {
        let knee = f64::from(settings.knee_dbfs);
        let vbe6 = vbe6_of_knee(knee);
        let beta6 = Self::beta6_for(knee, f64::from(settings.attack_ms));
        let k_dis = RELEASE_REF_MS / f64::from(settings.release_ms);
        let ln_k1 = ln(K1);
        let vc_of_gain = |a: f64| TWO_VT * (ln_k1 + ln((1.0 - a) / a));
        Self {
            vc: 0.0,
            vbe6,
            // R-03: the floor is where the shunt's deviation from unity reaches 2⁻²⁴; the limit
            // where the attenuation reaches MAX_ATTEN_DB. Both from the closed-form inverse.
            v_floor: vc_of_gain(1.0 - UNITY_DEVIATION),
            v_limit: vc_of_gain(db_to_amplitude(-MAX_ATTEN_DB)),
            dt_over_c6: 1.0 / (f64::from(host_rate_hz) * C6),
            charge_gain: beta6 / RTH,
            dis_r11: k_dis / R11,
            dis_b5: k_dis / BETA5,
            ceiling_fs: vbe6 / (KDIV * V_FS),
            ln_k1,
            vc_max: 0.0,
            #[cfg(feature = "op-count")]
            ops: 0,
            #[cfg(feature = "op-count")]
            transcendentals: 0,
        }
    }

    /// The knee-aware attack mapping (R-05): the detector current gain that realizes
    /// `attack_ms` of dead time at 10 dB over `knee_dbfs`,
    /// `BETA6_REF · (T_REF_MS / attack_ms) · I(KNEE_REF_DBFS) / I(knee_dbfs)`, guarded to
    /// `[BETA6_MIN, BETA6_MAX]`. About 217 at the defaults.
    pub(crate) fn beta6_for(knee_dbfs: f64, attack_ms: f64) -> f64 {
        let raw = BETA6_REF * (T_REF_MS / attack_ms) * overdrive_index(KNEE_REF_DBFS)
            / overdrive_index(knee_dbfs);
        raw.clamp(BETA6_MIN, BETA6_MAX)
    }

    /// The capacitor voltage at which a sample of amplitude `ax` (full-scale units) no longer
    /// exceeds the threshold (R-03): `vc(a_t)` with `a_t = ceiling / ax`; +∞ when `ax` is at or
    /// below the ceiling (no cap).
    #[inline]
    fn vc_target(&self, ax: f64) -> f64 {
        let a_t = self.ceiling_fs / ax;
        if a_t >= 1.0 {
            f64::INFINITY
        } else {
            TWO_VT * (self.ln_k1 + ln((1.0 - a_t) / a_t))
        }
    }

    /// One host-rate sample (data-model.md › Per-sample transitions, steps 1–5).
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let vc = self.vc;
        // Steps 1–2: the shunt's gain and the output. Below the floor the gain is exactly 1.0
        // and the multiply is skipped (R-03: bit-exact passthrough); Q4's base current `ib4`
        // is needed only above it.
        let (y, ib4) = if vc < self.v_floor {
            (x, 0.0)
        } else {
            let e = exp(vc * INV_2VT);
            let a = 1.0 / (1.0 + e * INV_K1);
            // Spec 016 (research R-02): one compare, only above the floor.
            if vc > self.vc_max {
                self.vc_max = vc;
            }
            #[cfg(feature = "op-count")]
            {
                self.ops += 6;
                self.transcendentals += 1;
            }
            (a * x, IS * e)
        };
        // Step 3: the detector's view. The codec input amp's rail bounds what Q6 sees; the
        // overdrive is the sensed polarity's GSX swing at Q6's base minus its Vbe.
        let gsx = (y * V_FS).clamp(-V_RAIL, V_RAIL);
        let over = KDIV_POL * gsx - self.vbe6;
        #[cfg(feature = "op-count")]
        {
            self.ops += 4;
        }
        if over <= 0.0 && vc == 0.0 {
            // Discharged and nothing to charge: the fresh state, unchanged. The whole sub-knee
            // path (FR-003) ends here, bit-exact.
            return y;
        }
        // Step 4: charge, current-gain limited then collector-resistor limited (`vc` never
        // reaches V_REF: it is clamped at `v_limit` = 1.275 V below).
        let charging = over > 0.0;
        let ic = if charging {
            (self.charge_gain * over).min((V_REF - vc) * INV_R12)
        } else {
            0.0
        };
        // Step 5: discharge through R11 plus Q5's base current. Below the floor the base-current
        // term is dropped with `ib4 = 0`: at the floor it is 9e-16 A against R11's 1.7e-8 A
        // (seven orders smaller), and this keeps the sub-floor path free of transcendentals
        // (R-03, R-12).
        let idis = self.dis_r11 * vc + self.dis_b5 * ib4;
        // Step 6: integrate, then the analytic cap (only when this sample charged), the limit
        // and the flush (R-03).
        let mut vc = (vc + (ic - idis) * self.dt_over_c6).max(0.0);
        if charging {
            vc = vc.min(self.vc_target(x.abs()));
            #[cfg(feature = "op-count")]
            {
                self.ops += 7;
                self.transcendentals += 1;
            }
        }
        vc = vc.min(self.v_limit);
        if vc < V_FLUSH {
            vc = 0.0;
        }
        #[cfg(feature = "op-count")]
        {
            self.ops += 12;
        }
        self.vc = vc;
        y
    }

    /// Returns to the freshly created state: capacitor discharged (FR-014).
    pub(crate) fn reset(&mut self) {
        self.vc = 0.0;
        self.vc_max = 0.0;
    }

    /// Spec 016 (research R-02, R-07): the largest entry voltage of the call so far, cleared for
    /// the next call.
    pub(crate) fn take_vc_max(&mut self) -> f64 {
        core::mem::replace(&mut self.vc_max, 0.0)
    }

    /// Spec 016 FR-002 (c): the attenuation the gain law applies at entry voltage `vc`, in
    /// non-negative dB; exactly 0 below the floor, where the gain is exactly 1.0 (R-03).
    pub(crate) fn reduction_db_at(&self, vc: f64) -> f64 {
        if vc < self.v_floor {
            0.0
        } else {
            let a = 1.0 / (1.0 + exp(vc * INV_2VT) * INV_K1);
            -20.0 * ln(a) / LN_10
        }
    }

    /// Spec 016 FR-002 (d): the hold as a fraction of the hold at the model's deepest
    /// attenuation (`MAX_ATTEN_DB`, an engineering target, 005 R-03): `vc / v_limit`, at most
    /// 1 (`process` caps `vc` at `v_limit`); settings-independent, since `v_limit` depends on
    /// `K1` alone.
    pub(crate) fn charge(&self) -> f64 {
        (self.vc / self.v_limit).min(1.0)
    }

    /// `v_limit` in volts (the charge's denominator; the `limiter_v_limit` test hook).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn v_limit(&self) -> f64 {
        self.v_limit
    }

    /// The capacitor voltage, V (tests and the harness's `limiter_state` hook).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn state(&self) -> f64 {
        self.vc
    }

    /// The gain the next sample will get, in dB (unit tests).
    #[cfg(test)]
    pub(crate) fn gain_db(&self) -> f64 {
        if self.vc < self.v_floor {
            0.0
        } else {
            let a = 1.0 / (1.0 + exp(self.vc * INV_2VT) * INV_K1);
            20.0 * ln(a) / LN_10
        }
    }

    /// The derived parameters (unit tests: a flushed stage equals a fresh one).
    #[cfg(test)]
    fn params(&self) -> [f64; 8] {
        [
            self.vbe6,
            self.charge_gain,
            self.dis_r11,
            self.dis_b5,
            self.v_floor,
            self.v_limit,
            self.dt_over_c6,
            self.ceiling_fs,
        ]
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use rr_dr60_detmath::{PI, cos};
    use std::vec::Vec;

    // Pinned to tools/agc-model/limiter_model.py, run 2026-10-11 at the default knee −1 dBFS
    // (005 R-04; T003; re-pinned after the post-implementation review moved the default knee
    // from +1 to −1 dBFS). The script runs the same equations as this module: a 192 kHz
    // reference of the continuous loop and the emulator's discretized loop (R-03) at the seven
    // host rates, with the emulator's conventions (positive polarity sensed, A-036; tones at
    // the host-grid peak phase, R-08). Dead time = the first sample whose attenuation exceeds
    // half the steady attenuation (FR-005). Levels are relative to the knee: "10 dB over" is
    // +9 dBFS.
    //
    //   knee (steady gain = -1 dB), 192 kHz reference    +0.03 dBFS      (FR-004: 0 +-2)
    //   ceiling (detector onset)                        -1.00 dBFS = 0.891 FS; no steady clipping
    //   self-consistent ceiling, Is 1e-15 / 1e-14 / 1e-13  +0.22 / -0.80 / -1.97 dBFS (review B1)
    //   static rows, emulator 6 kHz / 48 kHz (out peak, dBFS):
    //     +2 dBFS -0.995 / -0.958; +3 -0.994 / -0.956; +6 -0.990 / -0.951; +12 -0.982 / -0.935;
    //     +20 -0.966 / -0.912   (the excess over -1.00 is the one-sample gain lag, no look-ahead)
    //   T_REF (beta6 300, knee +1 reference, +10 dB over) 2.167 ms       (R-05 mapping reference)
    //   default attack 3.0 ms at knee -1 -> beta6        166.1
    //   dead time, -22 -> +9 dBFS, 192 kHz reference     3.18 ms, ramp 0.13 ms, over-atten. -0.01 dB
    //   dead time, emulator  6 kHz                        3.33 ms (20 samples), ramp 0.00 ms
    //   dead time, emulator  8 kHz                        2.50 ms (20 samples), ramp 0.88 ms
    //   dead time, emulator 16 kHz                        3.25 ms (52 samples)
    //   dead time, emulator 44.1 kHz                      3.20 ms (141 samples)
    //   dead time, emulator 48 kHz                        3.19 ms (153 samples), ramp 0.10 ms
    //   dead time, emulator 88.2 / 96 kHz                 3.19 / 3.18 ms
    //   +1 dBFS step (2 dB over), 6 kHz / 48 kHz         21.33 / 26.33 ms (steady -2.00 / -1.96 dB)
    //   armed (2 s after a 0.2 s burst), 6 / 8 / 48 kHz    0.333 / 0.375 / 0.271 ms
    //   armed vs gap at 48 kHz: 1 s 0.19, 2 s 0.27, 3 s 0.35, 5 s 1.19, 10 s 2.15, 20 s 2.31 ms (fresh 3.19)
    //   release from +9 dBFS 1 s burst (9.9 dB), 48 kHz   1.361 s to -1 dB;
    //     -9.18 dB @0.05 s, -7.20 @0.2, -6.12 @0.3, -4.39 @0.5, -2.87 @0.75, -1.86 @1.0
    //   release from +16 dBFS (16.9 dB)                    1.69 s
    //   release setting 200 / 1370 / 4000 / 10000 ms  -> 0.197 / 1.361 / 3.980 / 9.956 s
    //   knee-aware mapping, attack 3 ms at 48 kHz:
    //     knee -1 / +1 / -10 / -25 / -40 / +3 dBFS      -> 3.19 / 3.17 / 3.23 / 3.23 / 3.23 / 3.15 ms
    //     I(knee) overdrive index                        0.1531 / 0.1174 / 0.0947 / 0.0168 / 0.0030 / 0.0626 V
    //   attack 1 / 10 / 50 ms at 48 kHz (6 kHz), knee -1 1.17 (1.33) / 10.27 (10.33) / 52.31 (50.33) ms
    //   V_FLOOR (1 - gain = 2^-24)                       0.1714 V
    //   limit 40 dB at Vc                                1.275 V
    //   gain exactly 1.0 after a 1 s burst (6 kHz)       18.2 s; flush (Vc < 1e-6 V) ~121 s later
    //   polarity: asymmetric stimulus, larger peak knee + 3 dB (+2 dBFS): -2.95 dB (positive side) vs 0.00 dB
    //   bandwidth: 6 kHz +4 dBFS + 1 kHz -20 dBFS @48 k  sum peak +4.51 dBFS -> expected drop 5.51 dB; simulated -5.47 dB
    //   clipped onset, +9 dBFS, after the limiter         79 % in the first 2 ms; 0 % after 20 ms

    /// The host-grid peak phase (005 R-08): a host sample lands on the positive peak.
    fn peak_phase(freq_hz: f64, fs: f64) -> f64 {
        (PI / 2.0) % (TAU * freq_hz / fs)
    }

    fn amp(dbfs: f64) -> f64 {
        exp(dbfs * ln(10.0) / 20.0)
    }

    /// A 1 kHz tone stepping through `(dBFS, seconds)` segments with continuous phase, starting
    /// at the host-grid peak phase.
    fn tone(segments: &[(f64, f64)], fs: f64) -> Vec<f64> {
        let w = TAU * 1000.0 / fs;
        let phase = peak_phase(1000.0, fs);
        let mut out = Vec::new();
        let mut n = 0usize;
        for &(dbfs, secs) in segments {
            let a = amp(dbfs);
            for _ in 0..(secs * fs + 0.5) as usize {
                out.push(a * sin(w * n as f64 + phase));
                n += 1;
            }
        }
        out
    }

    fn device(settings: impl Fn(&mut AgcSettings), fs: f64) -> Limiter {
        let mut s = AgcSettings::DEVICE;
        settings(&mut s);
        Limiter::new(&s, fs as u32)
    }

    /// Runs `x` through `stage`: (output, the gain in dB applied to each sample).
    fn run(stage: &mut Limiter, x: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let mut y = Vec::with_capacity(x.len());
        let mut g = Vec::with_capacity(x.len());
        for &v in x {
            g.push(stage.gain_db());
            y.push(stage.process(v));
        }
        (y, g)
    }

    fn mean(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() / v.len() as f64
    }

    /// Mean gain over `[from_s, to_s)`.
    fn steady_db(g: &[f64], fs: f64, from_s: f64, to_s: f64) -> f64 {
        mean(&g[(from_s * fs) as usize..(to_s * fs) as usize])
    }

    /// FR-005: the index of the first sample at or after `t0` whose attenuation exceeds half
    /// the steady attenuation.
    fn dead_index(g: &[f64], fs: f64, t0: f64, steady: f64) -> usize {
        let half = 0.5 * steady;
        ((t0 * fs) as usize..g.len())
            .find(|&n| g[n] < half)
            .expect("the step never reached half the steady attenuation")
    }

    fn dead_samples(g: &[f64], fs: f64, t0: f64, steady: f64) -> usize {
        dead_index(g, fs, t0, steady) - (t0 * fs) as usize
    }

    /// From the dead-time point to within 1 dB of the steady value, ms.
    fn ramp_ms(g: &[f64], fs: f64, t0: f64, steady: f64) -> f64 {
        let n_dead = dead_index(g, fs, t0, steady);
        let n_done = (n_dead..g.len())
            .find(|&n| g[n] < steady + 1.0)
            .expect("never within 1 dB of steady");
        (n_done - n_dead) as f64 / fs * 1e3
    }

    /// Seconds after `t0` until the gain first exceeds `to_db`.
    fn release_s(g: &[f64], fs: f64, t0: f64, to_db: f64) -> f64 {
        let n = ((t0 * fs) as usize..g.len())
            .find(|&n| g[n] > to_db)
            .expect("never released");
        n as f64 / fs - t0
    }

    fn assert_bit_exact(y: &[f64], x: &[f64], what: &str) {
        for (n, (a, b)) in y.iter().zip(x).enumerate() {
            assert_eq!(a.to_bits(), b.to_bits(), "{what}: sample {n} differs");
        }
    }

    /// 005 FR-003 (A-028): below the knee the limiter is the identity, bit for bit, and the
    /// capacitor never charges: −20 dBFS, then −2.5 dBFS (knee − 1.5 dB), 5 s each.
    #[test]
    fn passthrough_below_the_knee_is_bit_exact() {
        for fs in [6000.0, 48_000.0] {
            let x = tone(&[(-20.0, 5.0), (-2.5, 5.0)], fs);
            let mut stage = device(|_| {}, fs);
            let mut y = Vec::with_capacity(x.len());
            for &v in &x {
                y.push(stage.process(v));
                assert_eq!(stage.state(), 0.0, "{fs} Hz: the capacitor charged");
            }
            assert_bit_exact(&y, &x, "sub-knee passthrough");
        }
    }

    /// 005 FR-004 (A-028): above the knee the output peak sits at the ceiling, −1.00 dBFS,
    /// from +2 to +20 dBFS (the harness tolerance is ±0.5 dB; this pins the model to the
    /// script's discretized static rows, ±0.02 dB), and the −1 dB point is +0.03 ± 0.1 dBFS
    /// by bisection to 0.05 dB. The peak reads a little above the ceiling, more at 48 kHz and
    /// at higher levels (+0.09 dB at +20 dBFS): `process` applies the gain of the previous
    /// sample's state, so the cap set on the sample before the peak (0.991 × the peak at
    /// 48 kHz) lets the peak sample through at up to +0.08 dB, plus about 0.02 dB of discharge
    /// over the cycle; a property of "no look-ahead" (review B5, 2026-10-11).
    #[test]
    fn ceiling_and_knee_at_defaults() {
        for (fs, rows) in [
            (
                6000.0,
                [
                    (2.0, -0.995),
                    (3.0, -0.994),
                    (6.0, -0.990),
                    (12.0, -0.982),
                    (20.0, -0.966),
                ],
            ),
            (
                48_000.0,
                [
                    (2.0, -0.958),
                    (3.0, -0.956),
                    (6.0, -0.951),
                    (12.0, -0.935),
                    (20.0, -0.912),
                ],
            ),
        ] {
            for (level, want) in rows {
                let x = tone(&[(level, 1.0)], fs);
                let (y, _) = run(&mut device(|_| {}, fs), &x);
                let tail = &y[y.len() - (0.1 * fs) as usize..];
                let peak = tail.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                let peak_dbfs = 20.0 * ln(peak) / LN_10;
                std::eprintln!(
                    "{fs} Hz: {level:+} dBFS in -> peak {peak_dbfs:+.3} dBFS (script {want:+.3})"
                );
                assert!(
                    (peak_dbfs - want).abs() <= 0.02, // 005 R-04: the script's discretized static rows
                    "{fs} Hz, {level} dBFS: peak {peak_dbfs:+.3} dBFS vs {want:+.3}"
                );
            }
            let steady = |level: f64| {
                let (_, g) = run(&mut device(|_| {}, fs), &tone(&[(level, 1.0)], fs));
                steady_db(&g, fs, 0.9, 1.0)
            };
            let (mut lo, mut hi) = (-4.0, 4.0);
            while hi - lo > 0.05 {
                let mid = 0.5 * (lo + hi);
                if steady(mid) < -1.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let knee = 0.5 * (lo + hi);
            std::eprintln!("{fs} Hz: knee (−1 dB point) {knee:+.3} dBFS");
            assert!((knee - 0.03).abs() <= 0.1, "{fs} Hz: knee {knee:+.3} dBFS"); // R-04
        }
    }

    /// 005 FR-005 (A-034): from the discharged state a −22 → +9 dBFS step (10 dB over the
    /// knee) has a dead time of 20 samples at 6 kHz (3.33 ms) and 153 at 48 kHz (3.19 ms), ±1;
    /// the ramp to within 1 dB of steady is ≤ 0.15 ms and the attenuation never overshoots
    /// its steady value by more than 0.05 dB (the charge cap, R-03). The +1 dBFS step (2 dB
    /// over) takes 128 / 1264 samples (21.33 / 26.33 ms), ±2.
    #[test]
    fn dead_time_from_discharged_state() {
        for (fs, want, want_2db) in [(6000.0, 20, 128), (48_000.0, 153, 1264)] {
            let x = tone(&[(-22.0, 0.05), (9.0, 1.0), (-22.0, 0.1)], fs);
            let (_, g) = run(&mut device(|_| {}, fs), &x);
            let steady = steady_db(&g, fs, 1.0, 1.05);
            let dead = dead_samples(&g, fs, 0.05, steady);
            let ramp = ramp_ms(&g, fs, 0.05, steady);
            let gmin = g[(0.05 * fs) as usize..(1.05 * fs) as usize]
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            std::eprintln!(
                "{fs} Hz: +9 dBFS step dead {dead} samples ({:.2} ms), ramp {ramp:.3} ms, steady {steady:.2} dB, over-attenuation {:+.3} dB",
                dead as f64 / fs * 1e3,
                gmin - steady
            );
            assert!(
                (dead as i64 - want).abs() <= 1,
                "{fs} Hz: dead {dead} vs {want}"
            );
            assert!(ramp <= 0.15, "{fs} Hz: ramp {ramp} ms");
            assert!(
                gmin - steady >= -0.05,
                "{fs} Hz: over-attenuation {}",
                gmin - steady
            );
            assert!((steady + 10.0).abs() <= 0.1, "{fs} Hz: steady {steady} dB");

            let x = tone(&[(-22.0, 0.05), (1.0, 1.5)], fs);
            let (_, g) = run(&mut device(|_| {}, fs), &x);
            let steady = steady_db(&g, fs, 1.5, 1.55);
            let dead = dead_samples(&g, fs, 0.05, steady);
            std::eprintln!(
                "{fs} Hz: +1 dBFS step dead {dead} samples ({:.2} ms), steady {steady:.2} dB",
                dead as f64 / fs * 1e3
            );
            assert!(
                (dead as i64 - want_2db).abs() <= 2,
                "{fs} Hz: dead {dead} vs {want_2db}"
            );
        }
    }

    /// 005 FR-005 (A-028): armed, 2 s after a 0.2 s burst 10 dB over the knee, the dead time is
    /// 2 samples at 6 kHz (0.333 ms) and 13 at 48 kHz (0.271 ms), ±1. The arming decays with
    /// R11·C6 = 10 s: at 48 kHz the dead time after gaps of 1 / 3 / 5 / 10 / 20 s is 0.19 /
    /// 0.35 / 1.19 / 2.15 / 2.31 ms against 3.19 ms fresh (review B2); the 5 and 10 s points
    /// are pinned ±2 samples.
    #[test]
    fn dead_time_when_armed() {
        for (fs, want) in [(6000.0, 2), (48_000.0, 13)] {
            let x = tone(&[(9.0, 0.2), (-22.0, 2.0), (9.0, 0.2)], fs);
            let (_, g) = run(&mut device(|_| {}, fs), &x);
            let steady = steady_db(&g, fs, 2.35, 2.4);
            let dead = dead_samples(&g, fs, 2.2, steady);
            std::eprintln!(
                "{fs} Hz: armed dead time {dead} samples ({:.3} ms)",
                dead as f64 / fs * 1e3
            );
            assert!(
                (dead as i64 - want).abs() <= 1,
                "{fs} Hz: dead {dead} vs {want}"
            );
        }
        let fs = 48_000.0;
        for (gap, want) in [(5.0, 57), (10.0, 103)] {
            let x = tone(&[(9.0, 0.2), (-22.0, gap), (9.0, 0.2)], fs);
            let (_, g) = run(&mut device(|_| {}, fs), &x);
            let t_on = 0.2 + gap;
            let steady = steady_db(&g, fs, t_on + 0.15, t_on + 0.2);
            let dead = dead_samples(&g, fs, t_on, steady);
            std::eprintln!(
                "48 kHz: dead time {dead} samples ({:.2} ms) after a {gap} s gap",
                dead as f64 / fs * 1e3
            );
            assert!(
                (dead as i64 - want).abs() <= 2,
                "gap {gap} s: dead {dead} vs {want}"
            ); // R-04 (review B2)
        }
    }

    /// 005 FR-006 (A-035): after +9 dBFS for 1 s (10 dB of attenuation) the gain reaches −1 dB
    /// 1.361 s ± 1 % later at 48 kHz, passing −6.12 ± 0.05 dB at 0.3 s and −2.87 ± 0.05 dB at
    /// 0.75 s; from +16 dBFS (17 dB) it takes 1.69 s ± 2 %.
    #[test]
    fn release_at_defaults() {
        let fs = 48_000.0;
        let x = tone(&[(9.0, 1.0), (-22.0, 3.0)], fs);
        let (_, g) = run(&mut device(|_| {}, fs), &x);
        let rel = release_s(&g, fs, 1.0, -1.0);
        let at = |t: f64| g[((1.0 + t) * fs) as usize];
        std::eprintln!(
            "48 kHz release: −1 dB at {rel:.4} s; {:.3} dB @0.3 s, {:.3} dB @0.75 s",
            at(0.3),
            at(0.75)
        );
        assert!((rel / 1.361 - 1.0).abs() <= 0.01, "release {rel} s"); // R-04
        assert!((at(0.3) + 6.12).abs() <= 0.05, "{} dB at 0.3 s", at(0.3));
        assert!((at(0.75) + 2.87).abs() <= 0.05, "{} dB at 0.75 s", at(0.75));

        let x = tone(&[(16.0, 1.0), (-22.0, 3.0)], fs);
        let (_, g) = run(&mut device(|_| {}, fs), &x);
        let rel = release_s(&g, fs, 1.0, -1.0);
        std::eprintln!("48 kHz release from +16 dBFS: −1 dB at {rel:.3} s");
        assert!(
            (rel / 1.69 - 1.0).abs() <= 0.02,
            "release {rel} s from 17 dB"
        );
    }

    /// 005 FR-011 (R-05): the release setting maps one to one: 200 / 4000 / 10 000 ms realize
    /// 0.197 / 3.980 / 9.956 s ± 1 % at 48 kHz.
    #[test]
    fn release_setting_scales_one_to_one() {
        let fs = 48_000.0;
        for (setting, want) in [(200.0, 0.197), (4000.0, 3.980), (10_000.0, 9.956)] {
            let x = tone(&[(9.0, 1.0), (-22.0, 12.0)], fs);
            let (_, g) = run(&mut device(|a| a.release_ms = setting, fs), &x);
            let rel = release_s(&g, fs, 1.0, -1.0);
            std::eprintln!("release setting {setting} ms -> {rel:.4} s");
            assert!(
                (rel / want - 1.0).abs() <= 0.01,
                "{setting} ms: {rel} s vs {want}"
            );
        }
    }

    /// 005 FR-011 (R-05): the knee-aware mapping holds the dead time at 3.0 ± 0.3 ms for a
    /// 3 ms attack at knees +1, −10, −25, −40 and +3 dBFS (script 3.17 / 3.23 / 3.23 / 3.23 /
    /// 3.15), and attacks of 1 / 10 / 50 ms at the default knee realize 1.17 / 10.27 /
    /// 52.31 ms ± 2 %; `beta6_for(1, 3)` is 216.7 ± 0.1 (the mapping reference) and
    /// `beta6_for(-1, 3)` 166.1 ± 0.1 (the default).
    #[test]
    fn knee_aware_attack_mapping() {
        let fs = 48_000.0;
        let b = Limiter::beta6_for(1.0, 3.0);
        assert!((b - 216.7).abs() <= 0.1, "beta6 {b}"); // R-04
        let b = Limiter::beta6_for(-1.0, 3.0);
        assert!((b - 166.1).abs() <= 0.1, "beta6 {b}"); // R-04
        let dead_ms = |knee: f64, attack: f64| {
            let x = tone(&[(knee - 21.0, 0.05), (knee + 10.0, 1.0)], fs);
            let (_, g) = run(
                &mut device(
                    |a| {
                        a.knee_dbfs = knee as f32;
                        a.attack_ms = attack as f32;
                    },
                    fs,
                ),
                &x,
            );
            let steady = steady_db(&g, fs, 1.0, 1.05);
            dead_samples(&g, fs, 0.05, steady) as f64 / fs * 1e3
        };
        for knee in [1.0, -10.0, -25.0, -40.0, 3.0] {
            let d = dead_ms(knee, 3.0);
            std::eprintln!("knee {knee:+} dBFS, attack 3 ms -> dead {d:.3} ms");
            assert!((d - 3.0).abs() <= 0.3, "knee {knee}: dead {d} ms");
        }
        for (attack, want) in [(1.0, 1.17), (10.0, 10.27), (50.0, 52.31)] {
            let d = dead_ms(-1.0, attack);
            std::eprintln!("attack {attack} ms -> dead {d:.3} ms");
            assert!(
                (d / want - 1.0).abs() <= 0.02,
                "attack {attack}: {d} ms vs {want}"
            );
        }
    }

    /// 005 FR-007 (A-036): the asymmetric stimulus `sin θ + 0.5 cos 2θ` (peaks +0.75 / −1.5)
    /// scaled so the larger peak is 3 dB over the knee (+2 dBFS): with that peak on the
    /// positive, sensed side the steady gain is −2.95 ± 0.05 dB (the ceiling margin); with it
    /// on the negative side the smaller peak (−4 dBFS) is below the knee and the output is the
    /// input bit for bit.
    #[test]
    fn polarity_positive_samples_sensed() {
        let fs = 48_000.0;
        let a = amp(2.0) / 1.5; // 005 FR-007: the larger peak 3 dB over the −1 dBFS knee
        let w = TAU * 1000.0 / fs;
        let stimulus = |sign: f64| -> Vec<f64> {
            (0..(2.0 * fs) as usize)
                .map(|n| sign * a * (sin(w * n as f64) + 0.5 * cos(2.0 * w * n as f64)))
                .collect()
        };
        let x = stimulus(-1.0); // the +1.5 peak is positive: sensed
        let (_, g) = run(&mut device(|_| {}, fs), &x);
        let steady = steady_db(&g, fs, 1.9, 2.0);
        std::eprintln!("polarity: +1.5 peak positive -> steady gain {steady:.3} dB");
        assert!((steady + 2.95).abs() <= 0.05, "steady gain {steady} dB"); // R-04
        let x = stimulus(1.0); // the +1.5 peak is negative: not sensed
        let mut stage = device(|_| {}, fs);
        let (y, _) = run(&mut stage, &x);
        assert_bit_exact(&y, &x, "larger peak on the un-sensed polarity");
        assert_eq!(stage.state(), 0.0);
    }

    /// 005 R-03: the unity floor is 0.1714 ± 0.0005 V; after a 1 s +9 dBFS burst at 6 kHz the
    /// gain is exactly 1.0 again (output bit-identical to input) from 18.2 ± 0.5 s after the
    /// burst, the capacitor flushes to 0.0 within 140 s, and the flushed stage equals a fresh one.
    #[test]
    fn floor_and_flush() {
        let fs = 6000.0;
        let mut stage = device(|_| {}, fs);
        assert!(
            (stage.v_floor - 0.1714).abs() <= 0.0005,
            "V_FLOOR {}",
            stage.v_floor
        );
        assert!(
            (stage.v_limit - 1.275).abs() <= 0.002,
            "V_LIMIT {}",
            stage.v_limit
        );
        let x = tone(&[(9.0, 1.0), (-22.0, 140.0)], fs);
        let (y, _) = run(&mut stage, &x);
        let last_diff = y
            .iter()
            .zip(&x)
            .rposition(|(a, b)| a.to_bits() != b.to_bits())
            .expect("the burst must be attenuated");
        let unity_s = (last_diff + 1) as f64 / fs - 1.0;
        std::eprintln!(
            "6 kHz: gain exactly 1.0 from {unity_s:.2} s after the burst; state at 140 s {:e} V",
            stage.state()
        );
        assert!((unity_s - 18.2).abs() <= 0.5, "unity after {unity_s} s"); // R-04
        assert_eq!(stage.state(), 0.0, "not flushed within 140 s");
        let fresh = device(|_| {}, fs);
        assert_eq!(stage.params(), fresh.params());
        assert_eq!(stage.state(), fresh.state());
    }

    /// 005 FR-014: the stage has no block state, so any chunking of the onset stimulus gives
    /// bit-identical output and state; `reset` discharges it and the next onset has the full
    /// dead time again.
    #[test]
    fn partition_invariance_and_reset() {
        let fs = 48_000.0;
        let x = tone(&[(-20.0, 0.1), (11.0, 0.4), (-20.0, 0.5)], fs);
        let mut whole = device(|_| {}, fs);
        let (y_whole, g_whole) = run(&mut whole, &x);
        let mut chunked = device(|_| {}, fs);
        let (mut y, mut g) = (Vec::new(), Vec::new());
        let mut pos = 0;
        let mut size = 1usize;
        while pos < x.len() {
            let n = size.min(x.len() - pos);
            let (yy, gg) = run(&mut chunked, &x[pos..pos + n]);
            y.extend(yy);
            g.extend(gg);
            pos += n;
            size = size * 7 % 8191 + 1;
        }
        assert_bit_exact(&y, &y_whole, "chunked vs whole");
        assert_eq!(g, g_whole);
        assert_eq!(chunked.state().to_bits(), whole.state().to_bits());

        assert!(whole.state() > 0.0);
        whole.reset();
        assert_eq!(whole.state(), 0.0);
        let steady = steady_db(&g_whole, fs, 0.45, 0.5);
        let dead_fresh = dead_samples(&g_whole, fs, 0.1, steady);
        let (_, g_again) = run(&mut whole, &x);
        assert_eq!(dead_samples(&g_again, fs, 0.1, steady), dead_fresh);
        assert_eq!(g_again, g_whole, "after reset the trace repeats exactly");
    }

    /// 005 FR-004 (engineering target, R-03): a +60 dBFS tone, far outside the device's range,
    /// is held at the 40 dB attenuation limit; the loop stays finite and `vc` at `v_limit`.
    #[test]
    fn limit_at_40_db() {
        let fs = 48_000.0;
        let mut stage = device(|_| {}, fs);
        let x = tone(&[(60.0, 0.5)], fs);
        let (y, g) = run(&mut stage, &x);
        let gmin = g.iter().copied().fold(f64::INFINITY, f64::min);
        std::eprintln!(
            "+60 dBFS: minimum gain {gmin:.4} dB, state {:.4} V",
            stage.state()
        );
        assert!(gmin >= -40.0 - 1e-6, "gain {gmin} dB below the 40 dB limit");
        assert!(stage.state() <= stage.v_limit);
        assert!(y.iter().all(|v| v.is_finite()));
        assert!(
            (gmin + 40.0).abs() <= 0.01,
            "the limit was not reached: {gmin} dB"
        );
    }

    /// 005 R-12: per host sample at most 1 `exp` + 1 `ln` + 30 basic operations while
    /// charging, and no transcendental call while the capacitor is below the floor and not
    /// charging (the sub-knee path and the tail of the release; a charging sample below the
    /// floor, the start of a dead time, calls `ln` once for the cap).
    #[cfg(feature = "op-count")]
    #[test]
    fn op_count_per_sample() {
        let fs = 48_000.0;
        let x = tone(&[(-20.0, 0.05), (11.0, 0.05), (-20.0, 0.1)], fs);
        let mut stage = device(|_| {}, fs);
        let (mut worst_ops, mut worst_trans) = (0, 0);
        for &v in &x {
            let before = stage.state();
            let below = before < stage.v_floor;
            let (o, t) = (stage.ops, stage.transcendentals);
            stage.process(v);
            let (d_ops, d_trans) = (stage.ops - o, stage.transcendentals - t);
            if below && stage.state() <= before {
                assert_eq!(d_trans, 0, "transcendental call below the floor");
            }
            worst_ops = worst_ops.max(d_ops);
            worst_trans = worst_trans.max(d_trans);
        }
        assert!(
            worst_ops <= 30,
            "{worst_ops} basic operations in one sample"
        ); // R-12
        assert!(
            worst_trans <= 2,
            "{worst_trans} transcendental calls in one sample"
        );
        assert!(worst_trans == 2, "the charging path was not exercised");
    }
}

/// Spec 016 (the per-block meters): the charge scale and the gain-from-voltage helper, pinned
/// to `tools/agc-model/limiter_model.py`'s "## Charge (spec 016)" print (T003, 2026-10-30).
#[cfg(test)]
mod meters_tests {
    use super::*;
    use crate::settings::AgcSettings;

    fn default_limiter() -> Limiter {
        Limiter::new(&AgcSettings::DEVICE, 48_000)
    }

    /// `v_limit` equals the script's 1.27538 V (research R-07: the harness divides by the
    /// script's literal, so the engine's value must agree with it closely).
    #[test]
    fn v_limit_matches_the_script() {
        let l = default_limiter();
        assert!((l.v_limit() - 1.27538).abs() < 1e-4, "{}", l.v_limit());
    }

    /// The landmarks of the hold law (FR-006), from the closed-form inverse: floor 0.1344,
    /// 1 dB 0.7269, 10 dB 0.8441, 20 dB 0.9022, 40 dB 1.0000.
    #[test]
    fn charge_landmarks() {
        let l = default_limiter();
        let ln_k1 = ln(K1);
        let vc_of_gain = |a: f64| TWO_VT * (ln_k1 + ln((1.0 - a) / a));
        let ratio = |a: f64| vc_of_gain(a) / l.v_limit();
        for (label, a, want) in [
            ("floor", 1.0 - UNITY_DEVIATION, 0.1344),
            ("1 dB", db_to_amplitude(-1.0), 0.7269),
            ("10 dB", db_to_amplitude(-10.0), 0.8441),
            ("20 dB", db_to_amplitude(-20.0), 0.9022),
            ("40 dB", db_to_amplitude(-40.0), 1.0),
        ] {
            let r = ratio(a);
            assert!((r - want).abs() < 1e-3, "{label}: {r} vs {want}");
        }
        // The engine's charge at those voltages (clamped to 1 at the cap).
        let mut l2 = default_limiter();
        l2.vc = vc_of_gain(db_to_amplitude(-10.0));
        assert!((l2.charge() - 0.8441).abs() < 1e-3);
        l2.vc = l2.v_limit;
        assert_eq!(l2.charge(), 1.0);
    }

    /// The reduction at an entry voltage: exactly 0 below the floor, 1 dB at the 1 dB landmark,
    /// 40 dB at the cap; monotone in `vc` (research R-02).
    #[test]
    fn reduction_db_at_voltage() {
        let l = default_limiter();
        let ln_k1 = ln(K1);
        let vc_of_gain = |a: f64| TWO_VT * (ln_k1 + ln((1.0 - a) / a));
        assert_eq!(l.reduction_db_at(0.0), 0.0);
        assert_eq!(l.reduction_db_at(l.v_floor * 0.999), 0.0);
        assert!((l.reduction_db_at(vc_of_gain(db_to_amplitude(-1.0))) - 1.0).abs() < 1e-9);
        assert!((l.reduction_db_at(vc_of_gain(db_to_amplitude(-10.0))) - 10.0).abs() < 1e-9);
        assert!((l.reduction_db_at(l.v_limit) - 40.0).abs() < 1e-6);
        let mut last = 0.0;
        for k in 0..100 {
            let r = l.reduction_db_at(l.v_limit * f64::from(k) / 99.0);
            assert!(r >= last);
            last = r;
        }
    }

    /// `vc_max` is the largest entry voltage among the samples that went through the gain law,
    /// taken once; a sub-floor run leaves it 0 (the gain is exactly 1.0 there).
    #[test]
    fn vc_max_is_the_largest_entry_voltage_and_is_taken_once() {
        let mut l = default_limiter();
        for _ in 0..200 {
            l.process(0.01);
        }
        assert_eq!(l.take_vc_max(), 0.0, "a sub-knee run never charges");
        // A loud run: the entry voltage rises monotonically, so vc_max is the entry voltage of
        // the last sample that went through the law.
        let mut entry_last = 0.0;
        for i in 0..2000 {
            let x = if i % 2 == 0 { 2.5 } else { -2.5 };
            if l.vc >= l.v_floor {
                entry_last = l.vc;
            }
            l.process(x);
        }
        let taken = l.take_vc_max();
        assert_eq!(taken, entry_last);
        assert_eq!(l.take_vc_max(), 0.0, "cleared by the take");
        assert!(l.charge() > 0.7, "{}", l.charge());
    }
}
