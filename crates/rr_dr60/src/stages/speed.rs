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
//! **A level change crossfades** (spec 019; research R-01): from the output the change lands on,
//! for N = `speed::CROSSFADE_SAMPLES` outputs the **old branch** (the old level's computation
//! at its own place, advancing at the old ratio) fades out while the **new branch** (the new
//! level's at the common place) fades in: outputs n = 1 … N − 1 are y_old + w_n·(y_new − y_old)
//! with the smoothstep weights, output N is y_new itself (a + 1.0·(b − a) is not b in floating
//! point); the place that governs the length, the latch and the tail is the new branch's. A
//! request during a fade waits for the fade's end (the latest wins). An engine convention, not
//! a device behavior (A-054).
//! **The pitch-kept mode** (spec 023; A-054's open question, an engine convention): a second
//! mechanism behind the same place, a waveform-similarity overlap-add on the stored stream at the
//! device rate (research R-02 – R-06): synthesis hops of Ss = 60 outputs on a grid anchored at
//! the first output the time-scaler computes; at each hop boundary the frame's nominal analysis
//! position is the place, a search over shifts 0, −1, +1, … ±Δ (Δ = 40) picks the shift whose
//! 60-sample segment best continues the previous frame (a plain cross-correlation, an outward
//! strict scan), and output m of the hop is x[prev + 60 + m]·w[60 + m] + x[cur + m]·w[m] with
//! the window of `speed_window.rs` (w[60 + m] = 1 − w[m] exactly; an exact continuation outputs
//! the stored sample itself). A level change between levels 1, 2, 4 and 5 is felt by the next hop
//! (no fade); a mode change, and a change into level 3 in this mode, crossfade over N outputs
//! from the old mechanism's frozen branch; level 3 is the identity in both modes. The tail after
//! the own part is 120 − ((own − origin) mod 60) outputs, or 60 (a grid bound, since Δ < Ss).
//! Every figure is pinned to `tools/playback-speed/speed_design.py`; modeled on the documents,
//! not measured on a unit.

use alloc::boxed::Box;
use alloc::vec;

use super::speed_window::WINDOW;
use crate::speed::SpeedMode;

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
/// The ring of stored samples: a power of two above the two branches' span during a crossfade
/// (96 stored samples of divergence over 120 outputs at 1 ↔ 5, plus the 57-tap centred kernel:
/// 153; spec 019 research R-03; 128 before spec 019).
const RING: usize = 256; // engineering target (019 R-03)
/// The crossfade's length in outputs (spec 019 FR-002).
const FADE_N: u32 = crate::speed::CROSSFADE_SAMPLES;
/// The time-scaler's frame, hop, search window and correlation length (spec 023 D2).
const FRAME: usize = crate::speed::FRAME_SAMPLES as usize; // 023 R-02
const HOP: usize = crate::speed::HOP_SAMPLES as usize; // 023 R-02
const SEARCH: i64 = crate::speed::SEARCH_SAMPLES as i64; // 023 R-03
const CORR: usize = crate::speed::CORRELATION_SAMPLES as usize; // 023 R-03
/// The search buffer: the candidates' span, 2Δ + Lc = 140 stored samples (research R-03).
const SEARCH_SPAN: usize = 2 * SEARCH as usize + CORR;
const _: () = assert!(FRAME == 2 * HOP && (SEARCH as usize) < HOP && CORR == HOP); // 023 R-06's proof needs Δ < Ss
/// The shift trace reserved at construction (the golden's digest; a test hook).
#[cfg(feature = "__test-hooks")]
const SHIFT_TRACE_CAPACITY: usize = 8192; // engineering target (023 R-12)

/// The pitch-kept time-scaler's state (data-model.md › TimeScaler): fixed size, copied whole
/// when a fade freezes the old branch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimeScaler {
    /// Anchored: the grid runs (false at level 3, in the default mode, or before the first output).
    active: bool,
    /// Outputs since the anchor; `n % HOP` the index inside the current hop.
    n: u32,
    /// The current frame's nominal analysis position (a whole stored sample) and its shift.
    cur_a: i64,
    cur_s: i32,
    /// The previous frame's.
    prev_a: i64,
    prev_s: i32,
}

impl TimeScaler {
    const IDLE: Self = Self {
        active: false,
        n: 0,
        cur_a: 0,
        cur_s: 0,
        prev_a: 0,
        prev_s: 0,
    };
}

/// T_c(own, origin): the pitch-kept tail (research R-06): the hop in progress at the latch ends,
/// and the previous frame's tail fades through the next hop: 120 − ((own − origin) mod 60), or
/// 60 when the remainder is 0. At most `speed::MAX_PITCH_KEPT_TAIL_SAMPLES`.
pub(crate) fn pitch_kept_tail(own: u64, origin: u64) -> u64 {
    let d = own.wrapping_sub(origin) % HOP as u64;
    if d == 0 { HOP as u64 } else { FRAME as u64 - d }
}

/// The weights of a crossfade of `n` outputs: entry i − 1 (i = 1 … n) is the smoothstep
/// `u * u * (3.0 - 2.0 * u)` at u = i ÷ n, the pinned expression (spec 019 research R-02:
/// exactly 0.15625 at i = n ÷ 4 and 1.0 at i = n).
fn fade_weights(n: u32) -> Box<[f64]> {
    (1..=n)
        .map(|i| {
            let u = f64::from(i) / f64::from(n);
            u * u * (3.0 - 2.0 * u)
        })
        .collect()
}

/// A crossfade in progress (spec 019 research R-01; data-model.md): the old branch's numerator
/// and place, and the blend outputs produced so far.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    old_k: u32,
    old_p5: u64,
    n: u32,
    /// Spec 023: the old branch's mode and, when it is pitch kept, its frozen time-scaler
    /// (continuing its own hops at its own place; research R-05).
    old_mode: SpeedMode,
    old_scaler: TimeScaler,
}
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
    /// The crossfade in progress (spec 019): `None` at a constant level.
    fade: Option<Fade>,
    /// The crossfade's weights (research R-02; `fade_len_delta` changes the length).
    weights: Box<[f64]>,
    /// The place of the last output computed, in fifths.
    last_p5: Option<u64>,
    /// `Own` outputs so far (K′).
    own: u64,
    /// `Tail` outputs so far (T).
    tail: u64,
    /// K′ + T, fixed when the place reaches K (the transport's total, research R-05).
    final_count: Option<u64>,
    /// Spec 023: the mode in force and the host's request.
    mode: SpeedMode,
    requested_mode: SpeedMode,
    /// Spec 023: the time-scaler (research R-01).
    scaler: TimeScaler,
    /// Spec 023 research R-03: the search's contiguous buffers (the candidates' span and the
    /// continuation), reserved at construction.
    search_a: Box<[f64]>,
    search_c: Box<[f64]>,
    /// The test hooks' mutation (research R-12).
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: crate::__test_hooks::SpeedMutation,
    /// Spec 023: the pitch-kept mutants (research R-12).
    #[cfg(feature = "__test-hooks")]
    pub(crate) pitch_mutation: crate::__test_hooks::PitchMutation,
    /// Spec 023: the per-frame shifts since the playback's start (the golden's digest).
    #[cfg(feature = "__test-hooks")]
    pub(crate) shift_trace: alloc::vec::Vec<i32>,
    /// The `into3_swap` mutant's remaining outputs in the hop and its own copy of the frame
    /// pair (the stage's scaler is idle at level 3).
    #[cfg(feature = "__test-hooks")]
    scaler_swap_remaining: u32,
    #[cfg(feature = "__test-hooks")]
    scaler_swap: TimeScaler,
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
            fade: None,
            weights: fade_weights(FADE_N),
            last_p5: None,
            own: 0,
            tail: 0,
            final_count: None,
            mode: SpeedMode::PitchWithSpeed,
            requested_mode: SpeedMode::PitchWithSpeed,
            scaler: TimeScaler::IDLE,
            search_a: vec![0.0f64; SEARCH_SPAN].into_boxed_slice(),
            search_c: vec![0.0f64; CORR].into_boxed_slice(),
            #[cfg(feature = "__test-hooks")]
            mutation: crate::__test_hooks::SpeedMutation::default(),
            #[cfg(feature = "__test-hooks")]
            pitch_mutation: crate::__test_hooks::PitchMutation::default(),
            #[cfg(feature = "__test-hooks")]
            shift_trace: alloc::vec::Vec::with_capacity(SHIFT_TRACE_CAPACITY),
            #[cfg(feature = "__test-hooks")]
            scaler_swap_remaining: 0,
            #[cfg(feature = "__test-hooks")]
            scaler_swap: TimeScaler::IDLE,
        }
    }

    /// The host's mode for the outputs computed from now on (spec 023 D1, D5; applied by `next`
    /// under the wait rule and the latch).
    pub(crate) fn request_mode(&mut self, mode: SpeedMode) {
        self.requested_mode = mode;
        // At level 3 with nothing pending the mode applies at once (D5: neither mechanism
        // sounds there), here rather than in the general step, so the steady identity's fast
        // path stays untouched (research R-01).
        if mode != self.mode
            && self.active == K_IDENTITY
            && self.requested == K_IDENTITY
            && self.fade.is_none()
            && !self.latched
        {
            self.mode = mode;
            self.scaler = TimeScaler::IDLE;
        }
    }

    /// The window the time-scaler uses (a mutant swaps a symmetric Hann in; research R-12).
    #[inline]
    fn window(&self, m: usize) -> f64 {
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.window_sum_off {
            // The symmetric Hann, period N − 1: the overlapping pair no longer sums to 1.
            let u = m as f64 / (FRAME - 1) as f64;
            return 0.5 - 0.5 * rr_dr60_detmath::cos(2.0 * core::f64::consts::PI * u);
        }
        WINDOW[m]
    }

    /// The search window in effect (a mutant narrows it).
    #[inline]
    fn search_delta(&self) -> i64 {
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.delta_narrow {
            return SEARCH / 2;
        }
        SEARCH
    }

    /// Anchors the time-scaler at the current place: the grid's origin is this output, the
    /// previous frame the natural continuation one hop back at shift 0, the current frame at
    /// the place with shift 0 (research R-02: the first frame has no continuation to match).
    fn anchor(&mut self, sc: &mut TimeScaler, record: bool) {
        let a0 = (self.p5 / 5) as i64;
        *sc = TimeScaler {
            active: true,
            n: 0,
            cur_a: a0,
            cur_s: 0,
            prev_a: a0 - HOP as i64,
            prev_s: 0,
        };
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.onset_fade {
            sc.prev_a = i64::MIN / 4; // the previous half reads zeros: a fade-in
        }
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.grid_unanchored {
            sc.prev_a = a0; // the previous frame at the place itself: a doubled onset
        }
        #[cfg(feature = "__test-hooks")]
        if record && self.shift_trace.len() < self.shift_trace.capacity() {
            self.shift_trace.push(0);
        }
        #[cfg(not(feature = "__test-hooks"))]
        let _ = record;
    }

    /// The similarity search at a hop boundary (research R-03): the candidates' span and the
    /// continuation copied into the contiguous buffers, then the plain cross-correlation over
    /// `CORR` samples for each shift in the outward order 0, −1, +1, … with a strict comparison
    /// (a tie keeps the smaller displacement; silence gives 0), the sums in `dot`'s order.
    fn search(&mut self, a: i64, cont: i64) -> i32 {
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.no_search {
            return 0;
        }
        let delta = self.search_delta();
        let lo = a - SEARCH;
        for m in 0..SEARCH_SPAN {
            let v = self.x(lo + m as i64);
            self.search_a[m] = v;
        }
        #[cfg(feature = "__test-hooks")]
        let cont = if self.pitch_mutation.match_nominal {
            a
        } else {
            cont
        };
        for m in 0..CORR {
            let v = self.x(cont + m as i64);
            self.search_c[m] = v;
        }
        let mut best = f64::NEG_INFINITY;
        let mut best_s = 0i32;
        let mut consider = |this: &Self, s: i64| {
            let off = (s + SEARCH) as usize;
            let seg = &this.search_a[off..off + CORR];
            let (mut s0, mut s1, mut s2, mut s3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut m = 0;
            while m + 3 < CORR {
                s0 += seg[m] * this.search_c[m];
                s1 += seg[m + 1] * this.search_c[m + 1];
                s2 += seg[m + 2] * this.search_c[m + 2];
                s3 += seg[m + 3] * this.search_c[m + 3];
                m += 4;
            }
            let c = (s0 + s1) + (s2 + s3); // CORR is a multiple of 4
            if c > best {
                best = c;
                best_s = s as i32;
            }
        };
        #[cfg(feature = "__test-hooks")]
        if self.pitch_mutation.tie_break_outward_off {
            for s in -delta..=delta {
                consider(self, s);
            }
            return best_s;
        }
        consider(self, 0);
        for d in 1..=delta {
            consider(self, -d);
            consider(self, d);
        }
        best_s
    }

    /// One pitch-kept output of time-scaler `sc` whose place is `p5` (research R-02): the hop
    /// boundary's frame advance and search, the lazy pulls, the exact-continuation shortcut,
    /// else the overlap-add.
    fn scaler_output(
        &mut self,
        sc: &mut TimeScaler,
        p5: u64,
        src: &mut impl FnMut() -> Option<f32>,
        record: bool,
    ) -> f64 {
        if !sc.active {
            self.anchor(sc, record);
        }
        if sc.n > 0 && sc.n as usize % HOP == 0 {
            sc.prev_a = sc.cur_a;
            sc.prev_s = sc.cur_s;
            #[cfg(feature = "__test-hooks")]
            if self.pitch_mutation.restart_overlap {
                sc.prev_a = (p5 / 5) as i64 - HOP as i64; // the previous frame dropped
                sc.prev_s = 0;
            }
            sc.cur_a = (p5 / 5) as i64;
            let cont = sc.prev_a + i64::from(sc.prev_s) + HOP as i64;
            // The search reads the candidates' span (through cur_a + Δ + Lc − 1) and the
            // continuation (through cont + Lc − 1; on the slow levels past the span: restated
            // at implementation, research R-01).
            self.pull_through(
                (sc.cur_a + SEARCH + CORR as i64 - 1).max(cont + CORR as i64 - 1),
                src,
            );
            sc.cur_s = self.search(sc.cur_a, cont);
            #[cfg(feature = "__test-hooks")]
            if record && self.shift_trace.len() < self.shift_trace.capacity() {
                self.shift_trace.push(sc.cur_s);
            }
        }
        let m = (sc.n as usize) % HOP;
        let j1 = sc.prev_a + i64::from(sc.prev_s) + HOP as i64 + m as i64;
        let j2 = sc.cur_a + i64::from(sc.cur_s) + m as i64;
        self.pull_through(j1.max(j2), src);
        sc.n += 1;
        if sc.prev_a + i64::from(sc.prev_s) + HOP as i64 == sc.cur_a + i64::from(sc.cur_s) {
            return self.x(j2); // the exact continuation: the stored sample itself
        }
        self.x(j1) * self.window(HOP + m) + self.x(j2) * self.window(m)
    }

    /// The crossfade's weights (the test hook `crossfade_weights`).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Whether a crossfade is running (the tests).
    #[cfg(test)]
    fn fading(&self) -> bool {
        self.fade.is_some()
    }

    /// Back to rest at the start of a recording of `stored` device samples, at `level` (FR-007:
    /// a playback start). No allocation.
    pub(crate) fn reset(&mut self, level: u8, mode: SpeedMode, stored: u64) {
        let k = numerator(level);
        self.mode = mode;
        self.ring.fill(0.0);
        self.pulled = 0;
        self.stored = stored;
        self.p5 = 0;
        self.active = k;
        self.requested = k;
        self.latched = false;
        self.fade = None;
        self.scaler = TimeScaler::IDLE;
        self.requested_mode = self.mode;
        #[cfg(feature = "__test-hooks")]
        self.shift_trace.clear();
        #[cfg(feature = "__test-hooks")]
        {
            // The length mutant (research R-05): the table rebuilt at a playback start when the
            // mutant moved the length (never in production: the delta is 0).
            let n = (i64::from(FADE_N) + i64::from(self.mutation.fade_len_delta)).max(1) as u32;
            if self.weights.len() != n as usize {
                self.weights = fade_weights(n);
            }
        }
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

    /// The state the last output was computed under (spec 020 map (c)'s pin, research R-04):
    /// its place, the level, whether the old branch contributed and at which place, and
    /// whether the place has reached K after it.
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn last_state(&self) -> crate::timeline::PlaceState {
        let (fading, old_place_fifths) = match self.fade {
            Some(f) => (true, f.old_p5 - self.step(f.old_k)),
            None => (false, 0),
        };
        // Spec 023: the old branch's place is its own in both mechanisms (the frozen scaler
        // reads the same place arithmetic).
        crate::timeline::PlaceState {
            place_fifths: self.last_p5.unwrap_or(0),
            level: match self.active {
                3 => 1,
                4 => 2,
                5 => 3,
                6 => 4,
                _ => 5,
            },
            fading,
            old_place_fifths,
            latched: self.latched,
        }
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
            debug_assert!(
                (j as u64) + RING as u64 > self.pulled,
                "the ring no longer holds stored sample {j} (pulled {})",
                self.pulled
            ); // 019 R-03
            f64::from(self.ring[j as usize & (RING - 1)])
        }
    }

    /// The dot product of phase `ph` of `bank` with x[base − m], m = 0 … 56, as four
    /// partial sums in a fixed order (research R-03): s_q over m ≡ q (mod 4), m ascending, then
    /// (s₀ + s₁) + (s₂ + s₃). Inside the stored samples it reads the ring without bounds tests;
    /// at the edges, through `x` (zeros outside 0 … K − 1). Both paths add the same terms in the
    /// same order, so they are bit-identical.
    #[inline]
    fn dot(&self, bank: &[f64], ph: usize, base: i64) -> f64 {
        let c = &bank[ph * PER_PHASE..(ph + 1) * PER_PHASE];
        let (mut s0, mut s1, mut s2, mut s3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        if base >= (PER_PHASE - 1) as i64 && (base as u64) < self.pulled {
            debug_assert!(
                (base as u64) + RING as u64 > self.pulled + PER_PHASE as u64,
                "the ring no longer holds the kernel at base {base} (pulled {})",
                self.pulled
            ); // 019 R-03
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
        if self.active != K_IDENTITY && !no_tail && self.mode == SpeedMode::PitchKept {
            // Spec 023 research R-06: the grid bound, from the hop in progress.
            remaining = pitch_kept_tail(u64::from(self.scaler.n), 0);
        } else if self.active != K_IDENTITY && !no_tail {
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

    /// Applies the requested level under the deferred-switch rule (FR-007) and the wait (spec
    /// 019 FR-005: not while a crossfade runs); a change starts a crossfade (research R-01).
    #[inline]
    fn apply_request(&mut self) {
        if self.latched || self.requested == self.active {
            return;
        }
        #[cfg(feature = "__test-hooks")]
        let (step, no_wait) = (self.mutation.fade_step, self.mutation.fade_no_wait);
        #[cfg(not(feature = "__test-hooks"))]
        let (step, no_wait) = (false, false);
        if self.fade.is_some() && !no_wait {
            return; // the wait: the latest request applies when the fade ends
        }
        if self.requested == K_IDENTITY {
            #[cfg(feature = "__test-hooks")]
            if self.mutation.snap_into_level_3 && self.p5 % 5 != 0 {
                self.p5 = self.p5.div_ceil(5) * 5; // the mutant: a forward snap
            }
            if self.p5 % 5 != 0 {
                return; // the deferred switch into level 3 (013 FR-007)
            }
        }
        let old = self.active;
        self.active = self.requested;
        if self.mode == SpeedMode::PitchKept {
            // Spec 023 D5: between levels 1, 2, 4 and 5 the time-scaler's next hop reads the
            // new place (no fade); out of level 3 the grid anchors at this output (no fade);
            // into level 3 (D12) a fade from the frozen scaler to the identity, after the
            // deferred switch's wait above.
            #[cfg(feature = "__test-hooks")]
            let swap = self.pitch_mutation.into3_swap;
            #[cfg(not(feature = "__test-hooks"))]
            let swap = false;
            if self.active == K_IDENTITY && !step && !swap && self.last_p5.is_some() {
                self.fade = Some(Fade {
                    old_k: old,
                    old_p5: self.p5,
                    n: 0,
                    old_mode: SpeedMode::PitchKept,
                    old_scaler: self.scaler,
                });
            }
            #[cfg(feature = "__test-hooks")]
            if self.active == K_IDENTITY && swap {
                // The mutant (the plan's first rule): the current hop finishes with the
                // identity in place of the current frame (a swap at a weight near one).
                self.scaler_swap = self.scaler;
                self.scaler_swap_remaining = (HOP - self.scaler.n as usize % HOP) as u32;
            }
            if self.active == K_IDENTITY {
                self.scaler = TimeScaler::IDLE;
            }
            return;
        }
        // Spec 019 (spec 011's first-sample rule): a change landing on the playback's first
        // output switches at once, no fade (nothing has sounded yet).
        if !step && self.last_p5.is_some() {
            // The old branch keeps its place and ratio from here; `fade_no_wait` restarts with
            // the running fade's new branch as the old one (research R-05).
            self.fade = Some(Fade {
                old_k: old,
                old_p5: self.p5,
                n: 0,
                old_mode: SpeedMode::PitchWithSpeed,
                old_scaler: TimeScaler::IDLE,
            });
        }
    }

    /// Applies the requested mode (spec 023 D5): not past the latch, not while a fade runs (the
    /// wait rule shared with the level), at once at level 3 or on the first output, else a
    /// crossfade whose old branch is the old mechanism frozen at its level (the time-scaler's
    /// copy continuing its own hops) and whose new branch starts at the common place (the
    /// time-scaler's first frame at shift 0).
    #[inline]
    fn apply_mode_request(&mut self) {
        if self.latched || self.requested_mode == self.mode {
            return;
        }
        #[cfg(feature = "__test-hooks")]
        let (step, no_wait) = (self.pitch_mutation.mode_step, self.mutation.fade_no_wait);
        #[cfg(not(feature = "__test-hooks"))]
        let (step, no_wait) = (false, false);
        if self.fade.is_some() && !no_wait {
            return;
        }
        let old_mode = self.mode;
        self.mode = self.requested_mode;
        if self.active != K_IDENTITY && !step && self.last_p5.is_some() {
            self.fade = Some(Fade {
                old_k: self.active,
                old_p5: self.p5,
                n: 0,
                old_mode,
                old_scaler: self.scaler,
            });
        }
        self.scaler = TimeScaler::IDLE; // the new branch anchors at the common place
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
            && self.fade.is_none() // 019 R-01: never inside a crossfade
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
        self.apply_mode_request(); // spec 023
        if latch_after {
            self.latch();
        }
        let ph = (self.p5 % 5) as usize;
        let own = self.p5 < 5 * self.stored;
        if !own && self.final_count.is_some_and(|f| self.own + self.tail >= f) {
            return SpeedOut::Silence;
        }
        // The pulls follow the leading branch (019 R-03): the new branch's base, or the old
        // branch's when the old level is faster.
        let base_new = if self.active == K_IDENTITY {
            i
        } else {
            i + self.lookahead()
        };
        #[cfg(feature = "__test-hooks")]
        if self.active == K_IDENTITY && self.mutation.lookahead_at_level_3 {
            self.pull_through(i + D as i64, src); // the mutant: look-ahead at level 3
        }
        let had_fade = self.fade.is_some();
        let old = self.fade.map(|f| {
            let io = (f.old_p5 / 5) as i64;
            let base_old = if f.old_k == K_IDENTITY {
                io
            } else {
                io + self.lookahead()
            };
            (f, io, base_old)
        });
        let base_pull = old.map_or(base_new, |(_, _, b)| b.max(base_new));
        let pitch_kept = self.mode == SpeedMode::PitchKept && self.active != K_IDENTITY;
        if !pitch_kept {
            self.pull_through(base_pull, src);
        }
        let y_new = if self.active == K_IDENTITY {
            #[cfg(feature = "__test-hooks")]
            if self.scaler_swap_remaining > 0 {
                // The `into3_swap` mutant: the hop finishes with the identity as the current
                // frame (the plan's first rule).
                let sw = self.scaler_swap;
                let m = HOP - self.scaler_swap_remaining as usize;
                let j1 = sw.prev_a + i64::from(sw.prev_s) + HOP as i64 + m as i64;
                self.scaler_swap_remaining -= 1;
                self.x(j1) * self.window(HOP + m) + self.x(i) * self.window(m)
            } else {
                self.x(i)
            }
            #[cfg(not(feature = "__test-hooks"))]
            self.x(i)
        } else if pitch_kept {
            // Spec 023: the time-scaler reads the place (the frame's nominal position) and the
            // ring; its pulls are lazy (research R-01).
            let p5 = self.p5;
            let mut sc = self.scaler;
            let y = self.scaler_output(&mut sc, p5, src, true);
            self.scaler = sc;
            y
        } else {
            self.dot(self.bank(self.active), ph, base_new)
        };
        let y = match old {
            None => y_new,
            Some((mut f, io, base_old)) => {
                // Spec 019 FR-001: blend outputs 1 … N − 1, the new branch itself at N.
                f.n += 1;
                let len = self.weights.len() as u32;
                let y = if f.n >= len {
                    y_new
                } else {
                    let y_old = if f.old_k == K_IDENTITY {
                        self.x(io)
                    } else if f.old_mode == SpeedMode::PitchKept {
                        // Spec 023 research R-05: the frozen scaler continues its own hops at
                        // its own place.
                        let mut sc = f.old_scaler;
                        let y = self.scaler_output(&mut sc, f.old_p5, src, false);
                        f.old_scaler = sc;
                        y
                    } else {
                        self.pull_through(base_old, src);
                        self.dot(self.bank(f.old_k), (f.old_p5 % 5) as usize, base_old)
                    };
                    #[cfg(feature = "__test-hooks")]
                    let w = if self.mutation.fade_linear {
                        f64::from(f.n) / f64::from(len) // the mutant: a linear weight
                    } else {
                        self.weights[(f.n - 1) as usize]
                    };
                    #[cfg(not(feature = "__test-hooks"))]
                    let w = self.weights[(f.n - 1) as usize];
                    y_old + w * (y_new - y_old)
                };
                f.old_p5 += self.step(f.old_k);
                self.fade = (f.n < len).then_some(f);
                y
            }
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
        // Spec 023 D5 (restated at implementation): a mode request that waited for a blend
        // applies when the blend ends, here, before the next output, since at level 3 the next
        // output may take the steady identity's fast path, which tests nothing (the plain
        // re-request of a block-size-1 run masked it; the 4096-block partition caught it).
        if had_fade && self.fade.is_none() && self.requested_mode != self.mode {
            self.apply_mode_request();
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
        st.reset(level, SpeedMode::PitchWithSpeed, x.len() as u64);
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
        st.reset(3, SpeedMode::PitchWithSpeed, 1000);
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
                st.reset(level, SpeedMode::PitchWithSpeed, 1000);
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

    // ---- Spec 019 (T004): the crossfade ----------------------------------------------------

    /// A stage at `level` over `x` with the requests `(output index, level)`; every output
    /// (own and tail) and the final count.
    fn run_with(level: u8, x: &[f32], requests: &[(usize, u8)]) -> (Vec<f64>, Option<u64>) {
        run_mutated(
            level,
            x,
            requests,
            crate::__test_hooks::SpeedMutation::default(),
        )
    }

    fn run_mutated(
        level: u8,
        x: &[f32],
        requests: &[(usize, u8)],
        m: crate::__test_hooks::SpeedMutation,
    ) -> (Vec<f64>, Option<u64>) {
        let mut st = SpeedStage::new(level);
        st.mutation = m;
        st.reset(level, SpeedMode::PitchWithSpeed, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        let mut out = Vec::new();
        loop {
            for &(at, l) in requests {
                if at == out.len() {
                    st.request(l);
                }
            }
            match st.next(&mut src) {
                SpeedOut::Own(v) | SpeedOut::Tail(v) => out.push(v),
                SpeedOut::Silence => break,
            }
        }
        (out, st.final_count())
    }

    fn step_mutation() -> crate::__test_hooks::SpeedMutation {
        crate::__test_hooks::SpeedMutation {
            fade_step: true,
            ..Default::default()
        }
    }

    fn tone(len: usize) -> Vec<f32> {
        (0..len)
            .map(|n| {
                0.5 * rr_dr60_detmath::sin(2.0 * core::f64::consts::PI * 2000.0 * n as f64 / 6000.0)
                    as f32
            })
            .collect()
    }

    /// Spec 019 research R-02 (SC-002): the weight table's pins.
    #[test]
    fn weights_are_the_smoothstep() {
        let st = SpeedStage::new(3);
        let w = st.weights();
        assert_eq!(w.len(), 120);
        assert_eq!(w[29].to_bits(), 0.15625f64.to_bits()); // u = 1/4
        assert_eq!(w[119].to_bits(), 1.0f64.to_bits());
        assert!(w[0] > 0.0);
        assert!(w.windows(2).all(|p| p[1] > p[0]), "strictly increasing");
        for (i, &v) in w.iter().enumerate() {
            let u = (i + 1) as f64 / 120.0;
            assert_eq!(v.to_bits(), (u * u * (3.0 - 2.0 * u)).to_bits());
        }
    }

    /// Spec 019 FR-001: a change a → b landing at output s: outputs s … s + N − 2 equal
    /// y_old + w·(y_new − y_old) of two constant-level stages at the two places, output s + N − 1
    /// equals the level-b stage's value, and the rest the level-b computation from the common
    /// place. s is chosen so that the common place is a multiple of k_b (the level-b stage's
    /// outputs pass through it).
    #[test]
    fn fade_starts_at_the_request_and_ends_at_n() {
        let x = tone(4000);
        for (a, b) in [
            (3u8, 5u8),
            (5, 3),
            (1, 5),
            (5, 1),
            (2, 4),
            (4, 2),
            (3, 1),
            (1, 3),
        ] {
            let (ka, kb) = (u64::from(numerator(a)), u64::from(numerator(b)));
            // The landing output: s outputs at level a reach place s·k_a; make it a multiple
            // of k_b (and whole for a change into 3).
            let mut s = 300usize;
            while (s as u64 * ka) % kb != 0 {
                s += 1;
            }
            let (y, _) = run_with(a, &x, &[(s, b)]);
            let (ya, _) = run_with(a, &x, &[]);
            let (yb, _) = run_with(b, &x, &[]);
            let st = SpeedStage::new(a);
            let w = st.weights();
            let ps = s as u64 * ka;
            let jb = (ps / kb) as usize; // the level-b output at the common place
            let n_fade = w.len();
            for n in 1..n_fade {
                let want = ya[s + n - 1] + w[n - 1] * (yb[jb + n - 1] - ya[s + n - 1]);
                assert_eq!(y[s + n - 1].to_bits(), want.to_bits(), "{a}->{b} blend {n}");
            }
            assert_eq!(
                y[s + n_fade - 1].to_bits(),
                yb[jb + n_fade - 1].to_bits(),
                "{a}->{b} at N"
            );
            for n in n_fade..n_fade + 300 {
                assert_eq!(
                    y[s + n].to_bits(),
                    yb[jb + n].to_bits(),
                    "{a}->{b} after {n}"
                );
            }
            assert!(
                y[..s]
                    .iter()
                    .zip(&ya)
                    .all(|(p, q)| p.to_bits() == q.to_bits()),
                "{a}->{b} before"
            );
        }
    }

    /// Spec 019 FR-004: a change into level 3 waits for a whole place, then blends; level 3's
    /// branch reads whole places (its ratio 1).
    #[test]
    fn fade_into_level_3_waits_for_a_whole_place_then_blends() {
        let x = tone(3000);
        for level in [1u8, 2, 4, 5] {
            for s in 301..306usize {
                let mut st = SpeedStage::new(level);
                st.reset(level, SpeedMode::PitchWithSpeed, x.len() as u64);
                let mut it = x.iter().copied();
                let mut src = || it.next();
                for _ in 0..s {
                    let _ = st.next(&mut src);
                }
                st.request(3);
                let mut waited = 0;
                while !st.fading() {
                    let _ = st.next(&mut src);
                    waited += 1;
                    assert!(waited <= 5);
                }
                assert_eq!(st.active, K_IDENTITY);
                // The first blend output was computed at the (whole) place of the switch; the
                // old branch has since advanced by one step of its own ratio.
                assert_eq!(
                    st.last_p5.unwrap() % 5,
                    0,
                    "the blend started at a whole place"
                );
                assert_eq!(
                    st.fade.unwrap().old_p5,
                    st.last_p5.unwrap() + u64::from(numerator(level))
                );
            }
        }
    }

    /// Spec 019 FR-005: a request during a fade waits for its end; the latest wins; an equal
    /// request starts nothing; a request at the latch is dropped.
    #[test]
    fn request_during_a_fade_waits() {
        let x = tone(4000);
        // 1 → 5 at 300, then 2 at 310 and 4 at 320: the fade to 5 runs to 300 + 120, then a fade
        // to 4 (the latest) starts; the 2 never sounds.
        let (y, _) = run_with(1, &x, &[(300, 5), (310, 2), (320, 4)]);
        let (y_wait, _) = run_with(1, &x, &[(300, 5), (420, 4)]);
        assert!(
            y.iter()
                .zip(&y_wait)
                .all(|(p, q)| p.to_bits() == q.to_bits()),
            "the latest waits to the end"
        );
        // A request equal to the running target: nothing.
        let (y_eq, _) = run_with(1, &x, &[(300, 5), (310, 5)]);
        let (y_one, _) = run_with(1, &x, &[(300, 5)]);
        assert!(
            y_eq.iter()
                .zip(&y_one)
                .all(|(p, q)| p.to_bits() == q.to_bits())
        );
        // A request at the latch is dropped: the output equals the run without it.
        let k = u64::from(numerator(5));
        let latch_at = (5 * x.len() as u64).div_ceil(k) as usize; // the first output at or past K
        let (y_late, f_late) = run_with(5, &x, &[(latch_at, 1)]);
        let (y_none, f_none) = run_with(5, &x, &[]);
        assert_eq!(y_late.len(), y_none.len());
        assert!(
            y_late
                .iter()
                .zip(&y_none)
                .all(|(p, q)| p.to_bits() == q.to_bits())
        );
        assert_eq!(f_late, f_none);
    }

    /// Spec 019 FR-006: a fade across the recording's end continues through the tail with
    /// zeros past K; the final count equals the stepped run's (the new branch's trajectory).
    #[test]
    fn fade_continues_through_the_tail() {
        let x = tone(500);
        for (a, b) in [(3u8, 5u8), (5, 1), (1, 5), (2, 4)] {
            let k = u64::from(numerator(a));
            let s = ((5 * x.len() as u64) / k) as usize - 10; // ten outputs before K
            let (y, f) = run_with(a, &x, &[(s, b)]);
            let (ys, fs) = run_mutated(a, &x, &[(s, b)], step_mutation());
            assert_eq!(f, fs, "{a}->{b} final count");
            assert_eq!(y.len(), ys.len(), "{a}->{b} outputs");
            assert!(y.iter().all(|v| v.is_finite()));
        }
    }

    /// Spec 019 FR-003: at a constant level the outputs equal the stepped build's (no fade
    /// state is entered): every level, own part and tail.
    #[test]
    fn constant_level_is_untouched() {
        let x = tone(1000);
        for level in [1u8, 2, 3, 4, 5] {
            let (y, f) = run_with(level, &x, &[]);
            let (ys, fs) = run_mutated(level, &x, &[], step_mutation());
            assert_eq!(f, fs);
            assert_eq!(y.len(), ys.len());
            assert!(
                y.iter().zip(&ys).all(|(p, q)| p.to_bits() == q.to_bits()),
                "level {level}"
            );
        }
        // Level 3 is the stored samples exactly.
        let (y, _, _) = run(3, &x);
        assert!(y.iter().zip(&x).all(|(&a, &b)| a == f64::from(b)));
    }

    /// Spec 019 research R-03: 1 ↔ 5 fades back and forth with the ring's debug assertions live.
    #[test]
    fn ring_span_is_within_256() {
        let x = tone(6000);
        let requests: Vec<(usize, u8)> = (1..30)
            .map(|i| (i * 150, if i % 2 == 1 { 5 } else { 1 }))
            .collect();
        let (y, _) = run_with(1, &x, &requests);
        assert!(y.iter().all(|v| v.is_finite()));
        let (y2, _) = run_with(
            5,
            &x,
            &requests
                .iter()
                .map(|&(s, l)| (s, 6 - l))
                .collect::<Vec<_>>(),
        );
        assert!(y2.iter().all(|v| v.is_finite()));
    }

    /// Spec 019 (spec 011's first-sample rule): a request landing on the first output switches
    /// at once without a fade: the outputs equal the run started at that level.
    #[test]
    fn first_output_switches_without_a_fade() {
        let x = tone(1000);
        for (a, b) in [(3u8, 4u8), (5, 1), (1, 3)] {
            let (y, _) = run_with(a, &x, &[(0, b)]);
            let (yb, _) = run_with(b, &x, &[]);
            assert_eq!(y.len(), yb.len(), "{a}->{b}");
            assert!(
                y.iter().zip(&yb).all(|(p, q)| p.to_bits() == q.to_bits()),
                "{a}->{b}"
            );
        }
    }

    /// Spec 019 research R-05: `fade_step` creates no fade and applies a request at once (0.18),
    /// also inside 20 ms of a previous one.
    #[test]
    fn fade_step_is_0_18() {
        let x = tone(2000);
        let mut st = SpeedStage::new(1);
        st.mutation = step_mutation();
        st.reset(1, SpeedMode::PitchWithSpeed, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        for _ in 0..300 {
            let _ = st.next(&mut src);
        }
        st.request(5);
        let _ = st.next(&mut src);
        assert_eq!(st.active, 7);
        assert!(!st.fading());
        st.request(2);
        let _ = st.next(&mut src);
        assert_eq!(st.active, 4, "a request inside 20 ms applies at once");
    }

    /// Spec 019 research R-05: `fade_no_wait` applies a request during a fade at once, the
    /// running fade's new branch becoming the old branch.
    #[test]
    fn fade_no_wait_restarts() {
        let x = tone(2000);
        let m = crate::__test_hooks::SpeedMutation {
            fade_no_wait: true,
            ..Default::default()
        };
        let mut st = SpeedStage::new(1);
        st.mutation = m;
        st.reset(1, SpeedMode::PitchWithSpeed, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        for _ in 0..300 {
            let _ = st.next(&mut src);
        }
        st.request(5);
        let _ = st.next(&mut src);
        let f1 = st.fade.unwrap();
        assert_eq!((f1.old_k, f1.n), (3, 1));
        st.request(2);
        let _ = st.next(&mut src);
        let f2 = st.fade.unwrap();
        assert_eq!(
            (f2.old_k, f2.n),
            (7, 1),
            "restarted from the level-5 branch"
        );
        assert_eq!(st.active, 4);
    }

    /// Spec 019 research R-05: `fade_len_delta` moves the fade's end; `fade_linear` reads 0.25
    /// at output 30 (the committed 0.15625).
    #[test]
    fn fade_len_delta_and_linear_mutants() {
        let x = tone(3000);
        let longer = crate::__test_hooks::SpeedMutation {
            fade_len_delta: 1,
            ..Default::default()
        };
        let mut st = SpeedStage::new(3);
        st.mutation = longer;
        st.reset(3, SpeedMode::PitchWithSpeed, x.len() as u64);
        assert_eq!(st.weights().len(), 121);
        let (y, _) = run_mutated(3, &x, &[(300, 5)], longer);
        let (yd, _) = run_with(3, &x, &[(300, 5)]);
        // The 120th blend output differs (still a blend under the mutant), the 121st equal.
        assert_ne!(y[300 + 119].to_bits(), yd[300 + 119].to_bits());
        let linear = crate::__test_hooks::SpeedMutation {
            fade_linear: true,
            ..Default::default()
        };
        let (ya, _) = run_with(3, &x, &[]);
        let (yb, _) = run_with(5, &x, &[]);
        // At output 30 of the fade: s = 300 at level 3 is place 1500 = 7 · 214 + 2: pick s = 301
        // → place 1505 = 7 · 215 (a multiple of 7).
        let s = 301usize;
        let (yl, _) = run_mutated(3, &x, &[(s, 5)], linear);
        let jb = 215usize;
        let n = 30usize;
        let want = ya[s + n - 1] + 0.25 * (yb[jb + n - 1] - ya[s + n - 1]);
        assert_eq!(yl[s + n - 1].to_bits(), want.to_bits());
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

    // ---- spec 023: the pitch-kept mode (tasks.md T005) ----

    #[derive(Clone, Copy)]
    enum Req {
        Level(u8),
        Mode(SpeedMode),
    }

    /// A stage in `mode` fed from `x`, the requests applied before the output at their index,
    /// run to its first `Silence`; the outputs and the stage.
    fn run_pitch(
        level: u8,
        mode: SpeedMode,
        x: &[f32],
        reqs: &[(usize, Req)],
        pm: crate::__test_hooks::PitchMutation,
    ) -> (Vec<f64>, SpeedStage) {
        let mut st = SpeedStage::new(level);
        st.pitch_mutation = pm;
        st.reset(level, mode, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        let mut out = Vec::new();
        loop {
            for &(at, r) in reqs {
                if at == out.len() {
                    match r {
                        Req::Level(l) => st.request(l),
                        Req::Mode(m) => st.request_mode(m),
                    }
                }
            }
            match st.next(&mut src) {
                SpeedOut::Own(v) | SpeedOut::Tail(v) => out.push(v),
                SpeedOut::Silence => break,
            }
            assert!(out.len() < 200_000, "no end");
        }
        (out, st)
    }

    fn pm() -> crate::__test_hooks::PitchMutation {
        crate::__test_hooks::PitchMutation::default()
    }

    /// ±0.125, 8 up and 9 down (period 17: off the hop grid; exactly representable): the
    /// script's `square17`.
    fn square17(len: usize) -> Vec<f32> {
        (0..len)
            .map(|n| if n % 17 < 8 { 0.125 } else { -0.125 })
            .collect()
    }

    /// Deterministic noise in ±0.5 (an LCG), for the searches whose best is the continuation.
    fn noise(len: usize) -> Vec<f32> {
        let mut s = 0x0D60_u32;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 8) as f32 / 16_777_216.0 - 0.5
            })
            .collect()
    }

    fn own_len(k: u64, numerator: u32) -> u64 {
        (5 * k).div_ceil(u64::from(numerator))
    }

    /// 023 R-02 (hardware B1): the generated window's halves sum to 1.0 exactly at every n;
    /// w[30]'s bits are the script's print (2026-10-31).
    #[test]
    fn window_sums_to_one_exactly() {
        for n in 0..HOP {
            assert_eq!(
                (WINDOW[n] + WINDOW[HOP + n]).to_bits(),
                1.0f64.to_bits(),
                "n={n}"
            );
        }
        assert_eq!(WINDOW[30].to_bits(), 0x3fdf_ffff_ffff_fffd);
        assert_eq!(WINDOW[0].to_bits(), 0.0f64.to_bits());
        assert_eq!(WINDOW[HOP].to_bits(), 1.0f64.to_bits());
    }

    /// 023 FR-004 (hardware B1 b): the first 60 outputs after leaving level 3 are the stored
    /// samples themselves, bit for bit (the anchor's exact continuation), from the start and
    /// mid-stream, on the tone and on noise.
    #[test]
    fn onset_is_bit_exact() {
        for x in [tone(3000), noise(3000)] {
            for level in [1u8, 2, 4, 5] {
                let (y, _) = run_pitch(level, SpeedMode::PitchKept, &x, &[], pm());
                for m in 0..HOP {
                    assert_eq!(
                        y[m].to_bits(),
                        f64::from(x[m]).to_bits(),
                        "L{level} start {m}"
                    );
                }
                let s = 300;
                let (y, st) =
                    run_pitch(3, SpeedMode::PitchKept, &x, &[(s, Req::Level(level))], pm());
                assert!(!st.fading());
                for m in 0..HOP {
                    assert_eq!(
                        y[s + m].to_bits(),
                        f64::from(x[s + m]).to_bits(),
                        "L{level} at {s} + {m}"
                    );
                }
            }
        }
    }

    /// 023 R-02 (hardware B1 b): a hop whose search picked the exact continuation outputs the
    /// stored samples themselves: at level 1 on noise the first hop's continuation lies 24
    /// samples past the nominal position (within Δ), so its shift is 24 and outputs 60 … 119
    /// are x[60 … 119] bit for bit; the second hop's continuation is out of reach (48 > Δ).
    #[test]
    fn exact_continuation_shortcut() {
        let x = noise(3000);
        let (y, st) = run_pitch(1, SpeedMode::PitchKept, &x, &[], pm());
        assert_eq!(&st.shift_trace[..2], &[0, 24]);
        for m in 0..HOP {
            assert_eq!(
                y[HOP + m].to_bits(),
                f64::from(x[HOP + m]).to_bits(),
                "m={m}"
            );
        }
        assert_ne!(st.shift_trace[2], 48);
        let mixed =
            (0..HOP).any(|m| y[2 * HOP + m].to_bits() != f64::from(x[2 * HOP + m]).to_bits());
        assert!(mixed, "the second hop is an overlap-add");
    }

    /// 023 R-06: T_c(K, k) against the script's table (2026-10-31), and the stage's own + tail
    /// counts equal ⌈5K ÷ k⌉ + T_c at the four K and four levels.
    #[test]
    fn tail_table() {
        let table: [(u64, [u64; 4]); 4] = [
            (1, [118, 118, 119, 119]),
            (59, [81, 106, 70, 77]),
            (119, [101, 91, 80, 95]),
            (600, [80, 90, 100, 111]),
        ];
        for (k, tails) in table {
            for (li, level) in [1u8, 2, 4, 5].iter().enumerate() {
                let own = own_len(k, numerator(*level));
                let t = pitch_kept_tail(own, 0);
                assert_eq!(t, tails[li], "K={k} L{level}");
                assert!((HOP as u64..=FRAME as u64 - 1).contains(&t));
                let x = noise(k as usize);
                let (y, st) = run_pitch(*level, SpeedMode::PitchKept, &x, &[], pm());
                assert_eq!((st.own, st.tail), (own, t), "K={k} L{level} counts");
                assert_eq!(y.len() as u64, own + t);
                assert_eq!(st.final_count, Some(own + t));
            }
        }
        assert_eq!(pitch_kept_tail(60, 0), 60);
        assert_eq!(pitch_kept_tail(61, 0), 119);
        assert_eq!(pitch_kept_tail(119, 0), 61);
    }

    /// 023 R-06 (013's `no_tail` pattern): the tail carries content, and every output the
    /// scaler would compute from final_count on is exactly 0 (the grid bound: both frames past
    /// the stored samples), at the four levels and four K.
    #[test]
    fn tail_rule_removed_gives_zeros() {
        for k in [1usize, 59, 119, 600] {
            for level in [1u8, 2, 4, 5] {
                let x = noise(k);
                let (y, mut st) = run_pitch(level, SpeedMode::PitchKept, &x, &[], pm());
                let own = st.own as usize;
                // At the fast levels the frame lags the place, so content spills into the
                // tail; at the slow levels the frame reads ahead of the place and the last
                // content is heard before the own part ends (the bound is loose there).
                if k > 100 && level >= 4 {
                    assert!(
                        y[own..].iter().any(|v| *v != 0.0),
                        "K={k} L{level}: a silent tail"
                    );
                }
                // Past the end: the scaler continued by hand reads zeros only.
                let mut sc = st.scaler;
                let mut p5 = st.p5;
                let mut none = || None::<f32>;
                for n in 0..4 * FRAME {
                    let v = st.scaler_output(&mut sc, p5, &mut none, true);
                    assert_eq!(v.to_bits(), 0.0f64.to_bits(), "K={k} L{level} past {n}");
                    p5 += u64::from(numerator(level));
                }
            }
        }
    }

    /// 023 R-03: on exact silence every shift is 0 (the outward scan with a strict comparison);
    /// the ascending-order mutant picks −40.
    #[test]
    fn search_tie_break_outward() {
        let x = vec![0.0f32; 2000];
        let (y, st) = run_pitch(1, SpeedMode::PitchKept, &x, &[], pm());
        assert!(st.shift_trace.iter().all(|&s| s == 0));
        assert!(st.shift_trace.len() > 10);
        assert!(y.iter().all(|v| v.to_bits() == 0.0f64.to_bits()));
        let m = crate::__test_hooks::PitchMutation {
            tie_break_outward_off: true,
            ..pm()
        };
        let (_, st) = run_pitch(1, SpeedMode::PitchKept, &x, &[], m);
        assert_eq!(st.shift_trace[0], 0, "the anchor");
        assert!(st.shift_trace[1..].iter().all(|&s| s == -40));
    }

    /// 023 R-09, R-12: the per-frame shift sequence on the exactly representable square
    /// (period 17) at the four levels equals the script's print (2026-10-31; FNV-1a 64 over
    /// the shifts as i32 LE, the anchor's 0 first).
    #[test]
    fn search_on_offgrid_tone_matches_script() {
        let x = square17(12_000);
        let pins = [
            (
                1u8,
                335usize,
                0x9830_1963_5b7d_89c8u64,
                [0i32, 7, -3, 4, -6, 1, 8, -2, 5, -5, 2, -8],
            ),
            (
                2,
                251,
                0x038f_54dc_718c_3ef4,
                [0, -5, 7, 2, -3, -8, 4, -1, -6, 6, 1, -4],
            ),
            (
                4,
                168,
                0x2cfd_02e4_c251_0bf2,
                [0, 5, -7, -2, 3, 8, -4, 1, 6, -6, -1, 4],
            ),
            (
                5,
                144,
                0xbd38_77cf_4521_ed81,
                [0, -7, 3, -4, 6, -1, -8, 2, -5, 5, -2, 8],
            ),
        ];
        for (level, frames, digest, first) in pins {
            let (_, st) = run_pitch(level, SpeedMode::PitchKept, &x, &[], pm());
            assert_eq!(st.shift_trace.len(), frames, "L{level} frames");
            assert_eq!(&st.shift_trace[..12], &first, "L{level} first 12");
            let mut h = crate::digest::Fnv1a64::new();
            for s in &st.shift_trace {
                h.update(&s.to_le_bytes());
            }
            assert_eq!(h.finish(), digest, "L{level} digest");
        }
    }

    /// 023 D5: a change between levels 1, 2, 4 and 5 in pitch-kept mode creates no fade; the
    /// next hop's nominal position is the place at the new ratio.
    #[test]
    fn level_change_is_frame_fed() {
        let x = noise(4000);
        for (a, b) in [(1u8, 5u8), (5, 1), (2, 4), (4, 2), (1, 2), (5, 4)] {
            let s = 301;
            let (y, st) = run_pitch(a, SpeedMode::PitchKept, &x, &[(s, Req::Level(b))], pm());
            assert!(!st.fading(), "{a}->{b}");
            // Replay the place to the next hop boundary after s (the grid's origin is output 0).
            let next_hop = s.div_ceil(HOP) * HOP;
            let p5 = s as u64 * u64::from(numerator(a))
                + (next_hop - s) as u64 * u64::from(numerator(b));
            let (ya, _) = run_pitch(a, SpeedMode::PitchKept, &x, &[], pm());
            assert!(
                y[..s]
                    .iter()
                    .zip(&ya)
                    .all(|(p, q)| p.to_bits() == q.to_bits()),
                "{a}->{b} before the change"
            );
            // The hop in progress is finished at the old frames (outputs s … next_hop − 1).
            assert!(
                y[s..next_hop]
                    .iter()
                    .zip(&ya[s..next_hop])
                    .all(|(p, q)| p.to_bits() == q.to_bits()),
                "{a}->{b} the hop in progress"
            );
            // A stage stopped right after the boundary shows the new nominal position.
            let mut st2 = SpeedStage::new(a);
            st2.reset(a, SpeedMode::PitchKept, x.len() as u64);
            let mut it = x.iter().copied();
            let mut src = || it.next();
            for n in 0..=next_hop {
                if n == s {
                    st2.request(b);
                }
                let _ = st2.next(&mut src);
            }
            assert_eq!(st2.scaler.cur_a, (p5 / 5) as i64, "{a}->{b} the next frame");
        }
    }

    /// 023 D12: a change into level 3 in pitch-kept mode waits for a whole place, then fades
    /// over N outputs from the frozen scaler (the old branch) to the identity; after the fade
    /// the identity bit for bit.
    #[test]
    fn into_level_3_fades() {
        let x = noise(4000);
        for a in [1u8, 2, 4, 5] {
            let ka = u64::from(numerator(a));
            let mut s = 301usize;
            while (s as u64 * ka) % 5 == 0 {
                s += 1; // not a whole place: the wait is exercised
            }
            let (y, _) = run_pitch(a, SpeedMode::PitchKept, &x, &[(s, Req::Level(3))], pm());
            let (ya, _) = run_pitch(a, SpeedMode::PitchKept, &x, &[], pm());
            // The switch at the first whole place at or after s.
            let mut sw = s;
            while (sw as u64 * ka) % 5 != 0 {
                sw += 1;
            }
            assert!(
                y[..sw]
                    .iter()
                    .zip(&ya)
                    .all(|(p, q)| p.to_bits() == q.to_bits()),
                "L{a}: before the switch"
            );
            let i0 = (sw as u64 * ka / 5) as usize; // the identity's index at the switch
            let st = SpeedStage::new(a);
            let w = st.weights();
            // Blend outputs 1 … N − 1: y = y_old + w·(x[i] − y_old) with the old branch the
            // frozen scaler, which continues the constant run's own hops: y_old == ya.
            for n in 1..FADE_N as usize {
                let want = ya[sw + n - 1] + w[n - 1] * (f64::from(x[i0 + n - 1]) - ya[sw + n - 1]);
                assert_eq!(y[sw + n - 1].to_bits(), want.to_bits(), "L{a} blend {n}");
            }
            for n in FADE_N as usize..FADE_N as usize + 200 {
                assert_eq!(
                    y[sw + n - 1].to_bits(),
                    f64::from(x[i0 + n - 1]).to_bits(),
                    "L{a} identity {n}"
                );
            }
        }
    }

    /// 023 D5: out of level 3 the grid anchors at the landing output (its place a₀), the previous
    /// frame one hop back at shift 0, no wait and no fade; the mutant `grid_unanchored` and
    /// `onset_fade` move the onset.
    #[test]
    fn out_of_level_3_anchors() {
        let x = noise(3000);
        let s = 301;
        for b in [1u8, 2, 4, 5] {
            let mut st = SpeedStage::new(3);
            st.reset(3, SpeedMode::PitchKept, x.len() as u64);
            let mut it = x.iter().copied();
            let mut src = || it.next();
            for _ in 0..s {
                let _ = st.next(&mut src);
            }
            st.request(b);
            let y0 = match st.next(&mut src) {
                SpeedOut::Own(v) => v,
                _ => panic!(),
            };
            assert!(!st.fading(), "L{b}");
            assert_eq!(st.active, numerator(b));
            assert_eq!(st.scaler.cur_a, s as i64);
            assert_eq!(st.scaler.prev_a, s as i64 - HOP as i64);
            assert_eq!((st.scaler.cur_s, st.scaler.prev_s), (0, 0));
            assert_eq!(st.shift_trace, vec![0]);
            assert_eq!(y0.to_bits(), f64::from(x[s]).to_bits());
        }
        let (y, _) = run_pitch(3, SpeedMode::PitchKept, &x, &[(s, Req::Level(1))], pm());
        for (name, m) in [
            (
                "grid_unanchored",
                crate::__test_hooks::PitchMutation {
                    grid_unanchored: true,
                    ..pm()
                },
            ),
            (
                "onset_fade",
                crate::__test_hooks::PitchMutation {
                    onset_fade: true,
                    ..pm()
                },
            ),
        ] {
            let (ym, _) = run_pitch(3, SpeedMode::PitchKept, &x, &[(s, Req::Level(1))], m);
            assert!(
                (1..HOP).any(|n| y[s + n].to_bits() != ym[s + n].to_bits()),
                "{name} leaves the onset"
            );
        }
    }

    /// 023 D5: a mode change at a level other than 3 is a crossfade over N outputs from the
    /// common place in both directions (the first 59 blend samples of the into-pitch-kept
    /// direction against the bit-exact onset; the into-default direction's last blend sample
    /// equal to the default run's); at level 3 it applies at once with no fade; the `mode_step`
    /// mutant applies at once.
    #[test]
    fn mode_change_fades() {
        let x = noise(4000);
        let s = 301;
        let n_fade = FADE_N as usize;
        for level in [1u8, 2, 4, 5] {
            let k = u64::from(numerator(level));
            // Default → pitch kept.
            let (y, st) = run_pitch(
                level,
                SpeedMode::PitchWithSpeed,
                &x,
                &[(s, Req::Mode(SpeedMode::PitchKept))],
                pm(),
            );
            assert_eq!(st.mode, SpeedMode::PitchKept);
            let (ya, _) = run_pitch(level, SpeedMode::PitchWithSpeed, &x, &[], pm());
            let w = SpeedStage::new(level).weights().to_vec();
            let a0 = (s as u64 * k / 5) as usize;
            assert!(
                y[..s]
                    .iter()
                    .zip(&ya)
                    .all(|(p, q)| p.to_bits() == q.to_bits())
            );
            for n in 1..HOP {
                let want = ya[s + n - 1] + w[n - 1] * (f64::from(x[a0 + n - 1]) - ya[s + n - 1]);
                assert_eq!(
                    y[s + n - 1].to_bits(),
                    want.to_bits(),
                    "L{level} into pitch-kept blend {n}"
                );
            }
            assert!((HOP..n_fade - 1).any(|n| y[s + n].to_bits() != ya[s + n].to_bits()));
            // Pitch kept → default: the last blend sample is the default branch itself.
            let (y, st) = run_pitch(
                level,
                SpeedMode::PitchKept,
                &x,
                &[(s, Req::Mode(SpeedMode::PitchWithSpeed))],
                pm(),
            );
            assert_eq!(st.mode, SpeedMode::PitchWithSpeed);
            let (yp, _) = run_pitch(level, SpeedMode::PitchKept, &x, &[], pm());
            assert!(
                y[..s]
                    .iter()
                    .zip(&yp)
                    .all(|(p, q)| p.to_bits() == q.to_bits())
            );
            for n in n_fade..n_fade + 300 {
                assert_eq!(
                    y[s + n - 1].to_bits(),
                    ya[s + n - 1].to_bits(),
                    "L{level} into default {n}"
                );
            }
            assert!((1..n_fade - 1).any(|n| y[s + n - 1].to_bits() != ya[s + n - 1].to_bits()));
            let m = crate::__test_hooks::PitchMutation {
                mode_step: true,
                ..pm()
            };
            let (ym, _) = run_pitch(
                level,
                SpeedMode::PitchKept,
                &x,
                &[(s, Req::Mode(SpeedMode::PitchWithSpeed))],
                m,
            );
            assert_eq!(
                ym[s].to_bits(),
                ya[s].to_bits(),
                "L{level} mode_step: at once"
            );
            assert_ne!(ym[s].to_bits(), y[s].to_bits());
        }
        // Level 3: at once, no fade, bit-identical to the identity.
        let (y, st) = run_pitch(
            3,
            SpeedMode::PitchWithSpeed,
            &x,
            &[(s, Req::Mode(SpeedMode::PitchKept))],
            pm(),
        );
        assert_eq!(st.mode, SpeedMode::PitchKept);
        assert!(
            y.iter()
                .zip(&x)
                .all(|(p, q)| p.to_bits() == f64::from(*q).to_bits())
        );
        assert_eq!(y.len(), x.len());
    }

    /// 023 US2 AS7: one wait rule for the speed pair: a request of either control during a
    /// blend (a mode blend, or an into-3 blend in pitch-kept mode) waits for the blend's end
    /// and the latest wins, in the four orderings.
    #[test]
    fn wait_rule_speed_pair() {
        let x = noise(4000);
        let s = 300;
        let n_fade = FADE_N as usize;
        let probe = |reqs: &[(usize, Req)], start_mode: SpeedMode| {
            let mut st = SpeedStage::new(1);
            st.reset(1, start_mode, x.len() as u64);
            let mut it = x.iter().copied();
            let mut src = || it.next();
            let mut states = Vec::new();
            for n in 0..s + n_fade + 5 {
                for &(at, r) in reqs {
                    if at == n {
                        match r {
                            Req::Level(l) => st.request(l),
                            Req::Mode(m) => st.request_mode(m),
                        }
                    }
                }
                let _ = st.next(&mut src);
                states.push((st.active, st.mode, st.fading()));
            }
            states
        };
        // (1) a mode blend, then a level request inside it: the level waits.
        let st = probe(
            &[
                (s, Req::Mode(SpeedMode::PitchKept)),
                (s + 10, Req::Level(5)),
            ],
            SpeedMode::PitchWithSpeed,
        );
        // `st[n]` is the state after output n; a blend started at output s runs through output
        // s + N − 1 and a waiting request lands on output s + N: a level request through the
        // general step's apply (visible after output s + N), a mode request at the blend's end
        // (visible after output s + N − 1, applied before output s + N: the fast path at level 3
        // tests nothing, so the stage applies it when the blend ends).
        assert!(st[s].2 && st[s + n_fade - 2].2 && !st[s + n_fade - 1].2);
        assert!(st[s..s + n_fade].iter().all(|t| t.0 == 3));
        assert_eq!(st[s + n_fade].0, 7, "the level lands at the blend's end");
        assert!(!st[s + n_fade].2, "no fade on a pitch-kept level change");
        // (2) an into-3 blend in pitch-kept mode, then a mode request inside it: waits.
        let st = probe(
            &[
                (s, Req::Level(3)),
                (s + 10, Req::Mode(SpeedMode::PitchWithSpeed)),
            ],
            SpeedMode::PitchKept,
        );
        let sw = (s..).find(|&n| st[n].0 == 5).unwrap();
        assert!(
            st[sw..sw + n_fade - 1]
                .iter()
                .all(|t| t.1 == SpeedMode::PitchKept && t.2)
        );
        assert_eq!(st[sw + n_fade - 2].1, SpeedMode::PitchKept);
        assert_eq!(
            st[sw + n_fade - 1].1,
            SpeedMode::PitchWithSpeed,
            "applied at the blend's end"
        );
        assert!(
            !st[sw + n_fade - 1].2 && !st[sw + n_fade].2,
            "a mode change at level 3 is at once"
        );
        // (3) a mode blend, then a second mode request inside it: the latest wins at the end.
        let st = probe(
            &[
                (s, Req::Mode(SpeedMode::PitchKept)),
                (s + 10, Req::Mode(SpeedMode::PitchWithSpeed)),
            ],
            SpeedMode::PitchWithSpeed,
        );
        assert!(
            st[s..s + n_fade - 1]
                .iter()
                .all(|t| t.1 == SpeedMode::PitchKept)
        );
        assert_eq!(st[s + n_fade - 1].1, SpeedMode::PitchWithSpeed);
        assert!(
            st[s + n_fade - 1].2 && st[s + n_fade].2,
            "the second change blends in turn"
        );
        // (4) a level request in the default mode (019's blend), then a mode request inside it.
        let st = probe(
            &[
                (s, Req::Level(5)),
                (s + 10, Req::Mode(SpeedMode::PitchKept)),
            ],
            SpeedMode::PitchWithSpeed,
        );
        assert!(
            st[s..s + n_fade - 1]
                .iter()
                .all(|t| t.1 == SpeedMode::PitchWithSpeed && t.2)
        );
        assert_eq!(st[s + n_fade - 2].1, SpeedMode::PitchWithSpeed);
        assert_eq!(
            st[s + n_fade - 1].1,
            SpeedMode::PitchKept,
            "applied at the level blend's end"
        );
        assert!(
            st[s + n_fade - 1].2 && st[s + n_fade].2,
            "the mode change blends after the level's blend"
        );
    }

    /// 023 R-01: the ring holds every sample both branches read (its debug assertion) over the
    /// 20 level pairs × 60 grid phases with a simultaneous level and mode change, in both mode
    /// directions, the searches running.
    #[test]
    fn ring_span_pitch_within_256() {
        let x = noise(1500);
        for a in 1..=5u8 {
            for b in 1..=5u8 {
                if a == b {
                    continue;
                }
                for ph in 0..HOP {
                    let s = 240 + ph;
                    for (ma, mb) in [
                        (SpeedMode::PitchKept, SpeedMode::PitchWithSpeed),
                        (SpeedMode::PitchWithSpeed, SpeedMode::PitchKept),
                    ] {
                        let (y, _) =
                            run_pitch(a, ma, &x, &[(s, Req::Level(b)), (s, Req::Mode(mb))], pm());
                        assert!(y.len() > s + FADE_N as usize, "{a}->{b} ph {ph}");
                    }
                }
            }
        }
    }

    /// 023 R-01 (restated at implementation): the pulls are lazy: the anchor's hop reads one
    /// sample per output (pulled = m + 1), a hop boundary pulls the candidates' span through
    /// cur_a + 99 and the continuation through cont + 59 (whichever is further), and a
    /// synthesis output pulls through max(j_prev, j_cur).
    #[test]
    fn lazy_pulls() {
        let x = noise(3000);
        let mut st = SpeedStage::new(1);
        st.reset(1, SpeedMode::PitchKept, x.len() as u64);
        let mut it = x.iter().copied();
        let mut src = || it.next();
        for m in 0..HOP {
            let _ = st.next(&mut src);
            assert_eq!(st.pulled, m as u64 + 1, "m={m}");
        }
        let _ = st.next(&mut src); // output 60: the first hop boundary
        assert_eq!(st.scaler.cur_a, 36);
        assert_eq!(st.pulled, 36 + 99 + 1); // cont = 60: cont + 59 = 119 < 135
        for m in 1..HOP {
            let _ = st.next(&mut src);
            let sc = st.scaler;
            let j1 = sc.prev_a + i64::from(sc.prev_s) + HOP as i64 + m as i64;
            let j2 = sc.cur_a + i64::from(sc.cur_s) + m as i64;
            assert_eq!(st.pulled, (136).max(j1.max(j2) + 1) as u64, "m={m}");
        }
        // The second hop (output 120): cur_a = 72, cont = 36 + 24 + 60 = 120 (the first hop's
        // shift is 24 on noise): the continuation reaches 179, past the span's 171.
        let _ = st.next(&mut src);
        let sc = st.scaler;
        assert_eq!((sc.cur_a, sc.prev_a, sc.prev_s), (72, 36, 24));
        assert_eq!(st.pulled, 120 + 59 + 1);
    }

    /// 023 R-12: each of the ten mutants changes what it names (the committed output or shift
    /// sequence differs on a stimulus and schedule chosen for it); `tie_break_outward_off` is
    /// read on the shifts (exact silence stretches to silence either way).
    #[test]
    fn mutants_change_what_they_name() {
        type M = crate::__test_hooks::PitchMutation;
        let sq = square17(6000);
        let nz = noise(4000);
        let differs = |x: &[f32], level: u8, mode: SpeedMode, reqs: &[(usize, Req)], m: M| {
            let (y, _) = run_pitch(level, mode, x, reqs, pm());
            let (ym, _) = run_pitch(level, mode, x, reqs, m);
            y.len() != ym.len() || y.iter().zip(&ym).any(|(p, q)| p.to_bits() != q.to_bits())
        };
        assert!(differs(
            &sq,
            1,
            SpeedMode::PitchKept,
            &[],
            M {
                no_search: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchKept,
            &[],
            M {
                restart_overlap: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchKept,
            &[(301, Req::Level(3))],
            M {
                into3_swap: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchKept,
            &[],
            M {
                onset_fade: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchKept,
            &[],
            M {
                window_sum_off: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            5,
            SpeedMode::PitchKept,
            &[],
            M {
                delta_narrow: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchKept,
            &[],
            M {
                match_nominal: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            3,
            SpeedMode::PitchKept,
            &[(301, Req::Level(1))],
            M {
                grid_unanchored: true,
                ..pm()
            }
        ));
        assert!(differs(
            &nz,
            1,
            SpeedMode::PitchWithSpeed,
            &[(301, Req::Mode(SpeedMode::PitchKept))],
            M {
                mode_step: true,
                ..pm()
            }
        ));
        let z = vec![0.0f32; 1500];
        let (_, a) = run_pitch(1, SpeedMode::PitchKept, &z, &[], pm());
        let (_, b) = run_pitch(
            1,
            SpeedMode::PitchKept,
            &z,
            &[],
            M {
                tie_break_outward_off: true,
                ..pm()
            },
        );
        assert_ne!(a.shift_trace, b.shift_trace);
        // The default mode and level 3 are untouched by every mutant.
        for m in [
            M {
                no_search: true,
                ..pm()
            },
            M {
                window_sum_off: true,
                ..pm()
            },
            M {
                onset_fade: true,
                ..pm()
            },
        ] {
            assert!(!differs(&nz, 1, SpeedMode::PitchWithSpeed, &[], m));
            assert!(!differs(&nz, 3, SpeedMode::PitchKept, &[], m));
        }
    }
}
