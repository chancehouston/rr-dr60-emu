//! The record and playback halves and paths shared by the one-pass engine and the split
//! engine (spec 006 FR-001, FR-002, FR-007, FR-009, FR-016; research R-01, R-02, R-06;
//! data-model.md › Paths).
//!
//! ```text
//! host in → [limiter] → host→device → [stage 4] → [clamp] → [VAS] → narrow to f32 ─┐
//!           RecordPath (RecordHalf = stage 4, clamp, VAS, narrowing)               │
//!                                                                                  ▼
//! host out ← device→host ← [stage 10] ← widen ←────────────────── PlaybackPath (PlaybackHalf)
//! ```
//!
//! The one-pass engine is a [`RecordPath`] feeding a [`PlaybackPath`] sample by sample; the
//! split engine puts the memory between its own instances of the two. Both run the same stage
//! objects' code in the same order on the same device samples, so record-then-play is
//! bit-identical to the one-pass processing by construction (R-01).

#[cfg(any(test, feature = "__test-hooks"))]
use alloc::vec::Vec;

use crate::codec;
use crate::pipeline::VasEvent;
use crate::rate::RatePlan;
use crate::resample::{self, PolyphaseDown, PolyphaseUp};
use crate::sanitize::{narrow_out, sanitize_in};
use crate::settings::{DEVICE_RATE_HZ, Settings, Tap, VasMode};
#[cfg(any(test, feature = "__test-hooks"))]
use crate::stages::biquad::Biquad;
use crate::stages::clamp::ClampStage;
use crate::stages::limiter::Limiter;
use crate::stages::vas::{VasDecision, VasStage};
use crate::stages::voiceband::{PlaybackStage, RecordStage};

/// Stage 10's settling length, in device samples: the first device sample after which its
/// impulse response stays below 10⁻⁵ of its own peak (−100 dB relative; engineering target,
/// spec 006 FR-008). Computed from the committed `PLAYBACK_SOS` by [`settling_len`] and
/// asserted equal to this constant by a unit test, so a coefficient change cannot silently
/// invalidate the playback drain (research R-06).
pub(crate) const N10: usize = 121; // 006 FR-008 (engineering target: −100 dB)

/// Impulse-response samples examined by [`settling_len`]: far longer than any committed tail
/// (engineering target, spec 006 research R-06).
#[cfg(any(test, feature = "__test-hooks"))]
const SETTLING_SCAN: usize = 10_000; // engineering target (006 R-06)

/// The settling length of a biquad cascade: the first index after which |h| stays below
/// `rel` × its peak. Test and design-time use only (allocates).
#[cfg(any(test, feature = "__test-hooks"))]
pub(crate) fn settling_len(sos: &[[f64; 5]], rel: f64) -> usize {
    let mut sections: Vec<Biquad> = sos.iter().map(|&s| Biquad::from_sos(s)).collect();
    let mut h = Vec::with_capacity(SETTLING_SCAN);
    for k in 0..SETTLING_SCAN {
        let mut y: f64 = if k == 0 { 1.0 } else { 0.0 };
        for s in &mut sections {
            y = s.process(y);
        }
        h.push(y.abs());
    }
    let peak = h.iter().copied().fold(0.0, f64::max);
    h.iter()
        .rposition(|&v| v >= rel * peak)
        .map_or(0, |i| i + 1)
}

/// Rounds a kept post-VAS device-rate sample to `f32`: the point where the recording is taken
/// (spec 006 FR-009; research R-02). The one-pass engine and the record path narrow here
/// alike, so a 32-bit store and the one-pass processing see the same samples, and the playback
/// half widens with `f64::from`, which is exact. The physical defence is one of inaudibility,
/// not fidelity: at this point the device holds an 8-bit µ-law code (A-003) whose finest step
/// is about 2⁻¹² of full scale (G.711 segment 0), while this rounding's error is at most 2⁻²⁵,
/// about 12 bits finer. `narrow_out` rounds to nearest, flushes `f32` subnormals to 0.0 and
/// saturates at ±`f32::MAX` (with the clamp on the sample is within ±1.0, so saturation never
/// acts).
#[inline]
pub(crate) fn narrow_device(d: f64) -> f32 {
    narrow_out(d)
}

/// A path latency in host samples: round(D + τ · host / 6000), rounded once (spec 006 FR-016;
/// hardware review B4).
fn host_latency(plan: &RatePlan, tau_device: f64, host_rate_hz: u32) -> u32 {
    (f64::from(plan.delay_host_samples())
        + tau_device * f64::from(host_rate_hz) / f64::from(DEVICE_RATE_HZ)
        + 0.5) as u32
}

/// The speech codec's delay in host samples at `host_rate_hz`: 180 · host / 6000, an integer at
/// every supported rate (spec 008 FR-004; research R-06).
pub(crate) fn codec_delay_host_samples(host_rate_hz: u32) -> u32 {
    let delay = u64::from(codec::DELAY_SAMPLES) * u64::from(host_rate_hz);
    debug_assert_eq!(
        delay % u64::from(DEVICE_RATE_HZ),
        0,
        "a whole number of host samples"
    );
    u32::try_from(delay / u64::from(DEVICE_RATE_HZ)).unwrap_or(u32::MAX)
}

/// `(l, m)` of a plan: `m / l` host samples per device sample (`1 / 1` at the identity rate).
pub(crate) fn l_m(plan: &RatePlan) -> (u64, u64) {
    match *plan {
        RatePlan::Identity => (1, 1),
        RatePlan::Convert { l, m, .. } => (u64::from(l), u64::from(m)),
    }
}

/// What the record half did with one device sample (research R-01). The kept value is the
/// 64-bit post-VAS sample; [`RecordPath::push`] narrows it (FR-009), and the test-only
/// un-narrowed route (`push_wide`) passes it on as it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RecordOut {
    /// Kept (recording).
    Keep(f64),
    /// Kept, and the first sample after a pause (a splice, or the end of a muted region).
    Resume(f64),
    /// Muted by the VAS (paused, mute mode): stored and played as +0.0, so the stream keeps
    /// its length (spec 003 FR-004; research R-09). Its own variant, because the event
    /// counters need the first muted sample to open the region.
    Mute,
    /// Dropped by the VAS (paused, drop mode).
    Drop,
}

/// The record half of the device chain: stage 4, the ADC clamp, stage 5 and the FR-009
/// narrowing (spec 005 data-model.md › Pipeline order and taps; spec 006 research R-01).
#[derive(Clone, Debug)]
pub(crate) struct RecordHalf {
    /// Stage 4: the record band-pass (5 sections; spec 004 research R-06).
    pub(crate) record: RecordStage,
    /// Stage 4 runs: enabled (001 FR-007).
    run_record: bool,
    /// The ADC's full-scale saturation after stage 4 (spec 005 FR-008; A-030); bypassed when
    /// `Settings::adc_clamp_enabled` is false.
    clamp: ClampStage,
    pub(crate) vas: VasStage,
    /// Stage 5 runs: the VAS is enabled and, when the tap is honoured, the tap is after it
    /// (spec 003 FR-003; research R-01).
    run_vas: bool,
    /// Stage 5 mutes paused audio instead of dropping it (spec 003 FR-004; research R-09).
    mute: bool,
    /// The stop feed holds the gate open (spec 008 FR-013; research R-06): a VAS decision to
    /// drop keeps the sample instead, the first such sample after a dropped stretch as a
    /// resume, so its splice event is recorded. Drop mode only; the VAS state still updates.
    forced: bool,
    /// The previous device sample was dropped (so a forced keep is a resume).
    after_drop: bool,
}

impl RecordHalf {
    /// The half for `settings`. With `honour_tap` the one-pass tap rules apply (the VAS does
    /// not run at the taps before it); without it the half ignores the tap (the recorder,
    /// spec 006 FR-001).
    pub(crate) fn new(settings: &Settings, honour_tap: bool) -> Self {
        Self {
            record: RecordStage::record(),
            run_record: settings.record_stage_enabled,
            clamp: ClampStage::new(settings.adc_clamp_enabled),
            vas: VasStage::new(&settings.vas),
            run_vas: settings.vas.enabled
                && (!honour_tap
                    || matches!(settings.tap, Tap::AfterVas | Tap::AfterDecoder)
                    || settings.tap.is_after_stage_10()),
            mute: settings.vas.mode == VasMode::Mute,
            forced: false,
            after_drop: false,
        }
    }

    /// One device-rate sample. A bypassed stage passes the sample through bit-exactly, with no
    /// arithmetic (001 FR-007). The clamp sees stage 4's output, so the VAS and every tap from
    /// "after record" on see clamped samples (spec 005 FR-008); a dropped sample goes no
    /// further, and a muted sample becomes +0.0 (spec 003 Edge Cases). The kept value is the
    /// 64-bit post-VAS sample; the path narrows it to `f32` (spec 006 FR-009). With the gate
    /// held open (spec 008 FR-013) a drop becomes a keep, the first one after a dropped stretch
    /// a resume.
    #[inline]
    pub(crate) fn process(&mut self, mut d: f64) -> RecordOut {
        if self.run_record {
            d = self.record.process(d);
        }
        d = self.clamp.process(d);
        let decision = if self.run_vas {
            self.vas.process(d)
        } else {
            VasDecision::Keep
        };
        let out = match decision {
            VasDecision::Drop if self.mute => RecordOut::Mute,
            VasDecision::Drop if self.forced && self.after_drop => RecordOut::Resume(d),
            VasDecision::Drop if self.forced => RecordOut::Keep(d),
            VasDecision::Drop => RecordOut::Drop,
            VasDecision::Resume if self.forced && !self.after_drop => RecordOut::Keep(d),
            VasDecision::Resume => RecordOut::Resume(d),
            VasDecision::Keep => RecordOut::Keep(d),
        };
        self.after_drop = matches!(out, RecordOut::Drop);
        out
    }

    /// Holds the VAS gate open (`true`) or releases it (spec 008 FR-013: the stop feed in drop
    /// mode with the speech codec on). In mute mode nothing is dropped, so there is nothing to
    /// force.
    pub(crate) fn force_open(&mut self, on: bool) {
        self.forced = on && !self.mute;
    }

    /// The VAS runs and is paused.
    pub(crate) fn paused(&self) -> bool {
        self.run_vas && self.vas.is_paused()
    }

    /// Group delay at 1 kHz of stage 4 if it runs, in device samples (001 R-10).
    pub(crate) fn group_delay_1k_samples(&self) -> f64 {
        if self.run_record {
            self.record.group_delay_1k_samples()
        } else {
            0.0
        }
    }

    pub(crate) fn reset(&mut self) {
        self.record.reset();
        self.clamp.reset();
        self.vas.reset();
        self.forced = false;
        self.after_drop = false;
    }
}

/// The playback half of the device chain: stage 10 (spec 006 research R-01).
#[derive(Clone, Debug)]
pub(crate) struct PlaybackHalf {
    /// Stage 10: the playback low-pass (3 sections; spec 004 research R-06).
    pub(crate) playback: PlaybackStage,
    /// Stage 10 runs: enabled and, when the tap is honoured, the tap is after playback.
    run_playback: bool,
}

impl PlaybackHalf {
    /// The half for `settings`; without `honour_tap` stage 10 runs unless its own bypass is set
    /// (the player, spec 006 FR-002).
    pub(crate) fn new(settings: &Settings, honour_tap: bool) -> Self {
        Self {
            playback: PlaybackStage::playback(),
            // Spec 009 FR-011: the output-chain taps run stage 10 too (009 T013 adds the chain).
            run_playback: settings.playback_stage_enabled
                && (!honour_tap || settings.tap.is_after_stage_10()),
        }
    }

    /// One stored device sample, widened exactly, through stage 10 if it runs.
    #[inline]
    pub(crate) fn process(&mut self, d: f64) -> f64 {
        if self.run_playback {
            self.playback.process(d)
        } else {
            d
        }
    }

    /// Stage 10 runs.
    pub(crate) fn runs(&self) -> bool {
        self.run_playback
    }

    /// Group delay at 1 kHz of stage 10 if it runs, in device samples (001 R-10).
    pub(crate) fn group_delay_1k_samples(&self) -> f64 {
        if self.run_playback {
            self.playback.group_delay_1k_samples()
        } else {
            0.0
        }
    }

    pub(crate) fn reset(&mut self) {
        self.playback.reset();
    }
}

/// Stream counters that place VAS events in the output (spec 003 research R-05, R-06), owned
/// by the record path (spec 006 plan review P10).
#[derive(Clone, Debug)]
pub(crate) struct EventCounters {
    /// Host samples per device sample is `m / l` (`l = m = 1` at the 6 kHz identity rate, and
    /// for the recorder, whose positions are device samples: spec 006 FR-006).
    pub(crate) l: u64,
    pub(crate) m: u64,
    /// Device samples kept (fed onward) since the stream started.
    kept: u64,
    /// Device samples dropped since the stream started.
    dropped: u64,
    /// ⌊dropped·m/l⌋ at the previous splice (the cumulative-floor rule, R-06).
    removed_reported: u64,
    /// Mute mode: the output position of the open muted region's first sample (R-09).
    mute_start: Option<u64>,
    /// An event generated at this step, waiting to be handed out by `take_event`.
    pending: Option<VasEvent>,
}

impl EventCounters {
    fn new(l: u64, m: u64) -> Self {
        Self {
            l,
            m,
            kept: 0,
            dropped: 0,
            removed_reported: 0,
            mute_start: None,
            pending: None,
        }
    }

    fn reset(&mut self) {
        self.kept = 0;
        self.dropped = 0;
        self.removed_reported = 0;
        self.mute_start = None;
        self.pending = None;
    }

    /// The output position of the kept sample with index `kept`: the first output n with
    /// ⌊n·l/m⌋ ≥ kept, i.e. ⌈kept·m/l⌉ (R-06).
    #[inline]
    fn position(&self) -> u64 {
        (u128::from(self.kept) * u128::from(self.m)).div_ceil(u128::from(self.l)) as u64
    }

    /// Records one device sample's outcome.
    #[inline]
    fn record(&mut self, out: RecordOut) {
        match out {
            RecordOut::Keep(_) => self.kept += 1,
            RecordOut::Drop => self.dropped += 1,
            RecordOut::Mute => {
                // R-09: a muted sample counts as kept, so the emission schedule stays
                // one-in-one-out. The region starts at this sample's output position.
                if self.mute_start.is_none() {
                    self.mute_start = Some(self.position());
                }
                self.kept += 1;
            }
            RecordOut::Resume(_) => {
                // R-06: the splice is at the first output whose newest device sample is this one
                // (kept index j), i.e. the first n with ⌊n·l/m⌋ ≥ j: n = ⌈j·m/l⌉.
                let position = self.position();
                self.pending = Some(match self.mute_start.take() {
                    // Mute mode: the region ends here; its length is in output samples (R-09).
                    Some(start) => VasEvent {
                        output_position: start,
                        input_length: position - start,
                    },
                    // Drop mode: the removed length uses the cumulative floor, so the integers
                    // add up at 44.1/88.2 kHz.
                    None => {
                        let removed = (u128::from(self.dropped) * u128::from(self.m)
                            / u128::from(self.l)) as u64;
                        let event = VasEvent {
                            output_position: position,
                            input_length: removed - self.removed_reported,
                        };
                        self.removed_reported = removed;
                        event
                    }
                });
                self.kept += 1;
            }
        }
    }
}

/// The record path: the limiter at the host rate (spec 005 research R-01), the host-to-device
/// conversion and a [`RecordHalf`], with the event counters (spec 006 FR-001; research R-01).
#[derive(Clone, Debug)]
pub(crate) struct RecordPath {
    /// Stage 3 at the host rate, before the conversion.
    pub(crate) limiter: Limiter,
    /// Stage 3 runs: the AGC is enabled (spec 002 FR-002; spec 005 FR-002).
    run_limiter: bool,
    /// `None` at the 6 kHz identity rate (spec 004 FR-002) and at the honoured "after AGC" tap.
    pub(crate) down: Option<PolyphaseDown>,
    pub(crate) half: RecordHalf,
    pub(crate) counters: EventCounters,
    /// The rate plan's `(l, m)`, for the event bound (whatever unit the counters use).
    plan_l: u64,
    plan_m: u64,
    #[cfg_attr(not(any(test, feature = "__test-hooks")), allow(dead_code))]
    host_rate_hz: u32,
    tap_after_agc: bool,
    /// The latency to the encoder's input (spec 006 FR-016): the conversion and stage 4.
    latency: u32,
    /// The speech codec runs after this path (spec 008 FR-015).
    codec_runs: bool,
    /// The speech codec's delay in host samples when it runs after this path (spec 008 FR-004:
    /// 180 device samples, a whole number of host samples at every rate); 0 when it does not.
    codec_delay: u32,
}

impl RecordPath {
    /// A record path for validated `settings` and its `plan`. `honour_tap`: the one-pass tap
    /// rules apply (no converter at the "after AGC" tap; the VAS off at the taps before it);
    /// `device_positions`: VAS events are placed in device samples from the start (the
    /// recorder, spec 006 FR-006) instead of host output samples (the one-pass, spec 003).
    /// Allocates (the decimator); construction time only.
    pub(crate) fn new(
        settings: &Settings,
        plan: &RatePlan,
        honour_tap: bool,
        device_positions: bool,
    ) -> Self {
        let tap_after_agc = honour_tap && settings.tap == Tap::AfterAgc;
        let (l, m) = l_m(plan);
        let half = RecordHalf::new(settings, honour_tap);
        let latency = if tap_after_agc {
            0
        } else {
            host_latency(plan, half.group_delay_1k_samples(), settings.host_rate_hz)
        };
        // Spec 008 FR-004, FR-015: the codec runs after the recorder whenever it is on, and
        // after the one-pass record path at the taps from the decoder's on.
        let codec_runs = settings.speech_codec_enabled
            && (!honour_tap
                || settings.tap == Tap::AfterDecoder
                || settings.tap.is_after_stage_10());
        let codec_delay = if codec_runs {
            codec_delay_host_samples(settings.host_rate_hz)
        } else {
            0
        };
        Self {
            limiter: Limiter::new(&settings.agc, settings.host_rate_hz),
            run_limiter: settings.agc.enabled,
            // The "after AGC" tap never converts, so its decimator is not built (001 R-01).
            down: if tap_after_agc {
                None
            } else {
                resample::decimator(plan)
            },
            half,
            counters: if device_positions {
                EventCounters::new(1, 1)
            } else {
                EventCounters::new(l, m)
            },
            plan_l: l,
            plan_m: m,
            host_rate_hz: settings.host_rate_hz,
            tap_after_agc,
            latency,
            codec_runs,
            codec_delay,
        }
    }

    /// One host sample in. Returns the kept device sample, **narrowed to `f32`** (spec 006
    /// FR-009: the point where the recording is taken, `narrow_device`), when this step
    /// produced one, after the counters recorded the outcome; `None` when no device sample
    /// was due or the VAS dropped it. A muted sample comes out as +0.0.
    #[inline]
    pub(crate) fn push(&mut self, x: f32) -> Option<f32> {
        self.step(x).map(narrow_device)
    }

    /// Test-only: the pre-0.6 un-narrowed route, so `narrowing_delta` can measure FR-009's
    /// effect (spec 006 research R-02). The shipped build has no caller.
    #[cfg(any(test, feature = "__test-hooks"))]
    #[inline]
    pub(crate) fn push_wide(&mut self, x: f32) -> Option<f64> {
        self.step(x)
    }

    /// The 64-bit post-VAS sample this host sample produced, if any (+0.0 for a muted one).
    #[inline]
    fn step(&mut self, x: f32) -> Option<f64> {
        let x = self.limited(x);
        let d = match &mut self.down {
            None => x,
            Some(down) => down.push(x)?,
        };
        let out = self.half.process(d);
        self.counters.record(out);
        match out {
            RecordOut::Keep(v) | RecordOut::Resume(v) => Some(v),
            RecordOut::Mute => Some(0.0),
            RecordOut::Drop => None,
        }
    }

    /// The limiter alone, at the host rate: the one-pass output at the "after AGC" tap
    /// (spec 005 FR-001). No conversion, no chain, no events.
    #[inline]
    pub(crate) fn limiter_only(&mut self, x: f32) -> f32 {
        narrow_out(self.limited(x))
    }

    /// Stage 3 at the host rate (spec 005 R-01). Bypassed: no arithmetic (spec 002 FR-002).
    #[inline]
    fn limited(&mut self, x: f32) -> f64 {
        let x = sanitize_in(x);
        if self.run_limiter {
            self.limiter.process(x)
        } else {
            x
        }
    }

    /// The event the last `push` generated, if any, handed out once (plan review P10).
    #[inline]
    pub(crate) fn take_event(&mut self) -> Option<VasEvent> {
        self.counters.pending.take()
    }

    /// The VAS runs and is paused.
    pub(crate) fn paused(&self) -> bool {
        self.half.paused()
    }

    /// Closes a mute-mode region still open at the end of a recording with a synthesized
    /// event: position = its start, length = the kept count − start (spec 006 FR-005; plan
    /// review P5b). `None` when no region is open.
    pub(crate) fn close_open_region(&mut self) -> Option<VasEvent> {
        let start = self.counters.mute_start.take()?;
        let end = self.counters.position();
        Some(VasEvent {
            output_position: start,
            input_length: end - start,
        })
    }

    /// An event count that is always enough for a block of `frames` host samples (spec 003
    /// R-07): splices are at least H + 2 device samples apart, and `frames` host samples hold
    /// at most ⌊frames·l/m⌋ + 1 device samples (+ 2 at the non-integer rates); the "+ 2" also
    /// covers a splice on the block's first device sample (engineering target, 003 R-15).
    pub(crate) fn max_events(&self, frames: usize) -> usize {
        if self.tap_after_agc {
            return 0;
        }
        let device = frames as u128 * u128::from(self.plan_l) / u128::from(self.plan_m);
        let spacing = u128::from(self.half.vas.hang_samples()) + 2;
        usize::try_from(device / spacing + 2).unwrap_or(usize::MAX)
    }

    /// The path's latency in host samples: round(D + τ_rec · host / 6000) plus the speech
    /// codec's delay when it runs (spec 008 FR-004), or 0 at the honoured "after AGC" tap
    /// (spec 006 FR-016).
    pub(crate) fn latency_host_samples(&self) -> u32 {
        self.latency.saturating_add(self.codec_delay)
    }

    /// The latency to the speech encoder's input: the spec 006 figure without the codec's delay
    /// (spec 008 FR-013: the stop flush that brings every input sample to the encoder).
    pub(crate) fn latency_host_samples_pre_encoder(&self) -> u32 {
        self.latency
    }

    /// The speech codec runs after this path (spec 008 FR-015).
    pub(crate) fn codec_runs(&self) -> bool {
        self.codec_runs
    }

    /// Holds the VAS gate open or releases it (spec 008 FR-013; `RecordHalf::force_open`).
    pub(crate) fn force_open(&mut self, on: bool) {
        self.half.force_open(on);
    }

    /// Back to the freshly created state (the limiter discharged, the decimator and the half
    /// cleared, the counters at zero). No allocation (spec 006 FR-004).
    pub(crate) fn reset(&mut self) {
        self.limiter.reset();
        if let Some(down) = &mut self.down {
            down.reset();
        }
        self.half.reset();
        self.counters.reset();
    }

    /// Test-only: stage 4 with other coefficients (the SC-007 mutation tests, spec 004 T024);
    /// the latency follows.
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn replace_record_stage(&mut self, stage: RecordStage, plan: &RatePlan) {
        self.half.record = stage;
        if !self.tap_after_agc {
            self.latency =
                host_latency(plan, self.half.group_delay_1k_samples(), self.host_rate_hz);
        }
    }
}

/// The playback path: a [`PlaybackHalf`] and the device-to-host conversion (spec 006 FR-002;
/// research R-01, R-06).
#[derive(Clone, Debug)]
pub(crate) struct PlaybackPath {
    pub(crate) half: PlaybackHalf,
    /// `None` at the 6 kHz identity rate and at the honoured "after AGC" tap.
    pub(crate) up: Option<PolyphaseUp>,
    /// The identity rate's one pending host sample (stage 10's output, 64-bit).
    pending: Option<f64>,
    plan_l: u64,
    plan_m: u64,
    host_rate_hz: u32,
    delay_host: u32,
    tap_after_agc: bool,
    latency: u32,
    drain: u32,
}

impl PlaybackPath {
    /// A playback path for validated `settings` and its `plan`. Allocates (the interpolator);
    /// construction time only.
    pub(crate) fn new(settings: &Settings, plan: &RatePlan, honour_tap: bool) -> Self {
        let tap_after_agc = honour_tap && settings.tap == Tap::AfterAgc;
        let (l, m) = l_m(plan);
        let half = PlaybackHalf::new(settings, honour_tap);
        let mut path = Self {
            half,
            up: if tap_after_agc {
                None
            } else {
                resample::interpolator(plan)
            },
            pending: None,
            plan_l: l,
            plan_m: m,
            host_rate_hz: settings.host_rate_hz,
            delay_host: plan.delay_host_samples(),
            tap_after_agc,
            latency: 0,
            drain: 0,
        };
        path.derive_figures(plan);
        path
    }

    /// The latency and the drain length from the plan and the half (spec 006 FR-008, FR-016).
    fn derive_figures(&mut self, plan: &RatePlan) {
        if self.tap_after_agc {
            self.latency = 0;
            self.drain = 0;
            return;
        }
        self.latency = host_latency(plan, self.half.group_delay_1k_samples(), self.host_rate_hz);
        // ⌈N₁₀·m/l⌉ + 2D: stage 10's settling in host samples, then the interpolator's full
        // remaining response (research R-06; hardware review B2). N₁₀ = 0 with stage 10 off.
        let n10 = if self.half.runs() { N10 as u64 } else { 0 };
        let settle = (u128::from(n10) * u128::from(self.plan_m)).div_ceil(u128::from(self.plan_l));
        self.drain = u32::try_from(settle).unwrap_or(u32::MAX) + 2 * self.delay_host;
    }

    /// One stored device sample in: widened exactly, through stage 10 if it runs, into the
    /// interpolator (or the identity rate's pending slot).
    #[inline]
    pub(crate) fn push_device(&mut self, d: f32) {
        self.push_wide(f64::from(d));
    }

    /// Test-only: the pre-0.6 un-narrowed route (spec 006 research R-02).
    #[cfg(any(test, feature = "__test-hooks"))]
    #[inline]
    pub(crate) fn push_device_wide(&mut self, d: f64) {
        self.push_wide(d);
    }

    #[inline]
    fn push_wide(&mut self, d: f64) {
        let y = self.half.process(d);
        match &mut self.up {
            None => self.pending = Some(y),
            Some(up) => up.push_device(y),
        }
    }

    /// The next host sample, if the device samples it reads have arrived (spec 003 research
    /// R-05): at the identity rate the pending sample, once. Narrowed for output.
    #[inline]
    pub(crate) fn try_next_host(&mut self) -> Option<f32> {
        let y = match &mut self.up {
            None => self.pending.take()?,
            Some(up) => up.try_next_host()?,
        };
        Some(narrow_out(y))
    }

    /// The path's latency in host samples: round(D + τ_play · host / 6000) (spec 006 FR-016).
    #[allow(dead_code)] // read by the transport (006 US1, T023)
    pub(crate) fn latency_host_samples(&self) -> u32 {
        self.latency
    }

    /// The drain the player runs after the last stored sample, in host samples: ⌈N₁₀·m/l⌉ + 2D
    /// (spec 006 FR-008); 0 at the identity rate with stage 10 off.
    #[allow(dead_code)] // read by the transport (006 US1, T023)
    pub(crate) fn drain_len_host_samples(&self) -> u32 {
        self.drain
    }

    /// Back to the freshly created state. No allocation (spec 006 FR-004).
    pub(crate) fn reset(&mut self) {
        self.half.reset();
        if let Some(up) = &mut self.up {
            up.reset();
        }
        self.pending = None;
    }

    /// Test-only: stage 10 with other coefficients (the SC-007 mutation tests, spec 004 T024);
    /// the latency and the drain follow.
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn replace_playback_stage(&mut self, stage: PlaybackStage, plan: &RatePlan) {
        self.half.playback = stage;
        self.derive_figures(plan);
    }
}

/// The plan's derived figures (006 research R-06; data-model.md › Per-rate figures), pinned
/// here so the T013 tests can recompute each from the committed code and fail if one moved.
#[cfg(test)]
pub(crate) mod figures {
    // derived 2026-10-11 from rate.rs and voiceband_coeffs.rs (006 research R-06); the tests
    // below recompute each

    /// The seven host rates, in `SUPPORTED_HOST_RATES` order (spec 004 FR-002).
    pub(crate) const RATES: [u32; 7] = [6000, 8000, 16_000, 44_100, 48_000, 88_200, 96_000];
    /// One converter's linear-phase delay D, in host samples (`RatePlan::delay_host_samples`).
    pub(crate) const D: [u32; 7] = [0, 40, 80, 219, 238, 437, 476];
    /// Record-path latency, host samples: round(D + τ_rec · host / 6000) (006 FR-016).
    pub(crate) const RECORD_LATENCY: [u32; 7] = [1, 41, 81, 223, 242, 445, 485];
    /// Playback-path latency, host samples: round(D + τ_play · host / 6000) (006 FR-016).
    pub(crate) const PLAYBACK_LATENCY: [u32; 7] = [0, 40, 81, 221, 241, 442, 481];
    /// The one-pass latency table (001 R-10; the committed `latency` test prints it).
    pub(crate) const ONE_PASS_LATENCY: [u32; 7] = [1, 81, 162, 444, 483, 887, 966];
    /// The speech codec's delay in host samples: 180 · rate / 6000 (spec 008 FR-004; research
    /// R-06; data-model.md › Per-rate figures).
    pub(crate) const CODEC_DELAY: [u32; 7] = [180, 240, 480, 1323, 1440, 2646, 2880];
    /// The one-pass latency with the codec on (spec 008 data-model.md): 006's plus the delay.
    pub(crate) const ONE_PASS_LATENCY_008: [u32; 7] = [181, 321, 642, 1767, 1923, 3533, 3846];
    /// Playback drain, host samples: ⌈N₁₀ · m / l⌉ + 2D (006 FR-008).
    pub(crate) const DRAIN: [u32; 7] = [121, 242, 483, 1328, 1444, 2653, 2888];
    /// Stage 10's settling length at −100 dB relative to its own impulse-response peak, in
    /// device samples (006 FR-008; engineering target: the −100 dB criterion).
    pub(crate) const N10_PINNED: usize = 121;
    /// Group delay at 1 kHz of stage 4, in device samples (`voiceband_coeffs.rs`).
    pub(crate) const TAU_REC: f64 = 0.5387;
    /// Group delay at 1 kHz of stage 10, in device samples (`voiceband_coeffs.rs`).
    pub(crate) const TAU_PLAY: f64 = 0.3133;
    /// ‖h₁₀‖₁: the L1 norm of stage 10's impulse response (006 research R-02, review P2).
    pub(crate) const H10_L1: f64 = 2.65;
    /// The largest interpolator branch's L1 norm at 48 kHz (006 research R-02, review P2).
    pub(crate) const MAX_INTERPOLATOR_BRANCH_L1: f64 = 2.33;
}

/// 006 T013 (spec 006 FR-001, FR-002, FR-007, FR-016; research R-01, R-06; plan review P10):
/// the halves and paths behind both engines.
#[cfg(test)]
mod tests {
    extern crate std;
    use super::figures::*;
    use super::*;
    use crate::pipeline::Pipeline;
    use crate::rate::RatePlan;
    use crate::settings::{Settings, VasMode};
    use crate::stages::voiceband::{PlaybackStage, RecordStage};
    use crate::stages::voiceband_coeffs::PLAYBACK_SOS;
    use rr_dr60_detmath::{TAU, exp, ln, sin};
    use std::vec::Vec;

    fn plan(rate: u32) -> RatePlan {
        RatePlan::for_host(rate).unwrap()
    }

    /// (l, m) of the plan: `m / l` host samples per device sample.
    fn l_m(rate: u32) -> (u64, u64) {
        match plan(rate) {
            RatePlan::Identity => (1, 1),
            RatePlan::Convert { l, m, .. } => (u64::from(l), u64::from(m)),
        }
    }

    /// A 1 kHz tone at `dbfs` for `secs` at `fs`, phase 0.
    fn tone(dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / fs;
        (0..(secs * fs) as usize)
            .map(|n| (amp * sin(w * n as f64)) as f32)
            .collect()
    }

    /// Loud burst over the knee, a VAS pause, a quiet burst: the limiter charges, the VAS
    /// drops (or mutes) and resumes, the clamp acts on the onset.
    fn stream(fs: f64) -> Vec<f32> {
        [
            tone(9.0, 0.3, fs),
            std::vec![0.0; (2.5 * fs) as usize],
            tone(-30.0, 0.4, fs),
            std::vec![0.0; (0.2 * fs) as usize],
        ]
        .concat()
    }

    /// The one-pass output and events (`process_with_events`, one block).
    fn one_pass(s: Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, bool) {
        let mut p = Pipeline::new(s).unwrap();
        let mut y = std::vec![0.0f32; x.len()];
        let mut ev = std::vec![VasEvent::default(); p.max_events(x.len())];
        let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
        y.truncate(info.produced);
        ev.truncate(info.events);
        (y, ev, info.paused)
    }

    /// The same through a `RecordPath` feeding a `PlaybackPath` sample by sample (R-01's
    /// `tick`), both honouring the tap and with host-rate event positions.
    fn paths(s: Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, bool) {
        let plan = plan(s.host_rate_hz);
        let mut rec = RecordPath::new(&s, &plan, true, false);
        let mut play = PlaybackPath::new(&s, &plan, true);
        let (mut y, mut ev) = (Vec::new(), Vec::new());
        for &v in x {
            if let Some(kept) = rec.push(v) {
                play.push_device(kept);
            }
            if let Some(out) = play.try_next_host() {
                y.push(out);
            }
            if let Some(e) = rec.take_event() {
                ev.push(e);
            }
        }
        (y, ev, rec.paused())
    }

    fn bits(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    /// R-01 (FR-007): record path then playback path equals the one-pass bit for bit, with the
    /// VAS in drop and mute mode and with everything bypassed, at 6 and 48 kHz.
    #[test]
    fn record_path_then_playback_path_equals_one_pass_at_6k_and_48k() {
        for rate in [6000, 48_000] {
            let x = stream(f64::from(rate));
            let mut drop = Settings::new(rate);
            drop.speech_codec_enabled = false; // the paths alone, no codec (008 FR-021)
            drop.vas.hang_ms = 500.0; // a pause inside the 2.5 s gap
            let mut mute = drop;
            mute.vas.mode = VasMode::Mute;
            let mut bypass = Settings::new(rate);
            bypass.speech_codec_enabled = false; // the paths alone, no codec (008 FR-021)
            bypass.agc.enabled = false;
            bypass.vas.enabled = false;
            bypass.record_stage_enabled = false;
            bypass.playback_stage_enabled = false;
            bypass.adc_clamp_enabled = false;
            for (name, s) in [("drop", drop), ("mute", mute), ("bypass_all", bypass)] {
                let (want, want_ev, want_paused) = one_pass(s, &x);
                let (got, got_ev, got_paused) = paths(s, &x);
                assert_eq!(bits(&got), bits(&want), "{rate} Hz {name}: output");
                assert_eq!(got_ev, want_ev, "{rate} Hz {name}: events");
                assert_eq!(got_paused, want_paused, "{rate} Hz {name}: paused");
                if name != "bypass_all" {
                    assert!(!want_ev.is_empty(), "{rate} Hz {name}: no VAS event");
                }
                if name == "drop" {
                    assert!(got.len() < x.len(), "{rate} Hz: nothing dropped");
                }
            }
        }
    }

    /// R-01 (FR-006; plan review P10): the record path owns the counters: `take_event` hands
    /// each event out once; with `device_positions` the positions and lengths are device
    /// samples from the start (at 6 kHz every push is a device sample: `Some` kept, `None`
    /// dropped, so the figures are exact; at 48 kHz the counts add up to the device total).
    #[test]
    fn record_path_owns_counters() {
        let mut s = Settings::new(6000);
        s.vas.hang_ms = 500.0;
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.adc_clamp_enabled = false;
        let x = stream(6000.0);
        let mut rec = RecordPath::new(&s, &plan(6000), false, true);
        let (mut kept, mut dropped_since, mut events) = (0u64, 0u64, Vec::new());
        for &v in &x {
            let before = kept;
            match rec.push(v) {
                Some(_) => kept += 1,
                None => dropped_since += 1,
            }
            if let Some(e) = rec.take_event() {
                assert_eq!(e.output_position, before, "position is the kept count");
                assert_eq!(e.input_length, dropped_since, "length is the dropped count");
                dropped_since = 0;
                events.push(e);
                assert_eq!(rec.take_event(), None, "handed out once");
            }
        }
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(rec.take_event(), None);

        let s48 = Settings {
            host_rate_hz: 48_000,
            ..s
        };
        let x = stream(48_000.0);
        let mut rec = RecordPath::new(&s48, &plan(48_000), false, true);
        let (mut kept, mut events) = (0u64, Vec::new());
        for &v in &x {
            if rec.push(v).is_some() {
                kept += 1;
            }
            if let Some(e) = rec.take_event() {
                events.push(e);
                assert_eq!(rec.take_event(), None);
            }
        }
        assert_eq!(events.len(), 1);
        let removed: u64 = events.iter().map(|e| e.input_length).sum();
        let device_total = (x.len() as u64).div_ceil(8);
        assert!(
            (kept + removed).abs_diff(device_total) <= 1,
            "kept {kept} + removed {removed} vs {device_total} device samples"
        );
        assert!(events[0].output_position < kept);
    }

    /// R-01: at the 6 kHz identity rate the playback path holds one pending sample: pushed,
    /// it comes out once.
    #[test]
    fn identity_rate_pending_sample() {
        let mut s = Settings::new(6000);
        s.playback_stage_enabled = false;
        let mut play = PlaybackPath::new(&s, &plan(6000), true);
        assert_eq!(play.try_next_host(), None);
        play.push_device(0.5);
        assert_eq!(play.try_next_host(), Some(0.5));
        assert_eq!(play.try_next_host(), None);
        play.push_device(-0.25);
        play.reset();
        assert_eq!(
            play.try_next_host(),
            None,
            "reset clears the pending sample"
        );
    }

    /// R-06 (FR-007; review B3): K stored samples play back as exactly ⌈K·m/l⌉ host samples
    /// at every rate (the interpolator emits host sample n while ⌊n·l/m⌋ < K).
    #[test]
    fn playback_length_is_ceil_k_m_over_l() {
        for (i, rate) in RATES.iter().copied().enumerate() {
            assert_eq!(plan(rate).delay_host_samples(), D[i], "{rate} Hz: D");
            let (l, m) = l_m(rate);
            for stage10 in [true, false] {
                let mut s = Settings::new(rate);
                s.playback_stage_enabled = stage10;
                for k in 1..=50u64 {
                    let mut play = PlaybackPath::new(&s, &plan(rate), true);
                    let mut n = 0u64;
                    for _ in 0..k {
                        play.push_device(1.0);
                        while play.try_next_host().is_some() {
                            n += 1;
                        }
                    }
                    assert_eq!(
                        n,
                        (k * m).div_ceil(l),
                        "{rate} Hz, K = {k}, stage 10 {stage10}"
                    );
                }
            }
        }
    }

    /// R-06 (FR-008): the drain length table, ⌈N₁₀·m/l⌉ + 2D, and 2D alone with stage 10 off.
    #[test]
    fn drain_len_table() {
        for (i, rate) in RATES.iter().copied().enumerate() {
            let s = Settings::new(rate);
            let play = PlaybackPath::new(&s, &plan(rate), true);
            assert_eq!(play.drain_len_host_samples(), DRAIN[i], "{rate} Hz");
            let (l, m) = l_m(rate);
            assert_eq!(
                u64::from(DRAIN[i]),
                (N10_PINNED as u64 * m).div_ceil(l) + 2 * u64::from(D[i]),
                "{rate} Hz: the table matches the formula"
            );
            let mut off = s;
            off.playback_stage_enabled = false;
            let play = PlaybackPath::new(&off, &plan(rate), true);
            assert_eq!(
                play.drain_len_host_samples(),
                2 * D[i],
                "{rate} Hz, stage 10 off"
            );
        }
    }

    /// R-06 (FR-016; review B4; SC-008): the two latencies, each rounded once, and their sum
    /// equals the one-pass figure at every rate; a bypassed stage contributes 0.
    #[test]
    fn record_and_playback_latency_table() {
        for (i, rate) in RATES.iter().copied().enumerate() {
            let mut s = Settings::new(rate);
            s.speech_codec_enabled = false; // the 006 figures (008 FR-021); the codec's row below
            let rec = RecordPath::new(&s, &plan(rate), false, true);
            let play = PlaybackPath::new(&s, &plan(rate), false);
            assert_eq!(
                rec.latency_host_samples(),
                RECORD_LATENCY[i],
                "{rate} Hz record"
            );
            assert_eq!(
                play.latency_host_samples(),
                PLAYBACK_LATENCY[i],
                "{rate} Hz playback"
            );
            let one_pass = Pipeline::new(s).unwrap().latency_samples();
            assert_eq!(one_pass, ONE_PASS_LATENCY[i], "{rate} Hz one-pass");
            assert_eq!(
                rec.latency_host_samples() + play.latency_host_samples(),
                one_pass,
                "{rate} Hz: the sum"
            );
            let mut off = s;
            off.record_stage_enabled = false;
            off.playback_stage_enabled = false;
            let rec = RecordPath::new(&off, &plan(rate), false, true);
            let play = PlaybackPath::new(&off, &plan(rate), false);
            assert_eq!(rec.latency_host_samples(), D[i], "{rate} Hz: stage 4 off");
            assert_eq!(play.latency_host_samples(), D[i], "{rate} Hz: stage 10 off");
            // Spec 008 FR-004 (research R-06): with the codec on the record path and the
            // one-pass grow by exactly the codec's delay, the playback path not at all, and the
            // sum rule keeps 006's margin.
            let on = Settings::new(rate);
            assert!(on.speech_codec_enabled);
            assert_eq!(codec_delay_host_samples(rate), CODEC_DELAY[i], "{rate} Hz");
            let rec = RecordPath::new(&on, &plan(rate), false, true);
            let play = PlaybackPath::new(&on, &plan(rate), false);
            assert!(rec.codec_runs());
            assert_eq!(
                rec.latency_host_samples(),
                RECORD_LATENCY[i] + CODEC_DELAY[i],
                "{rate} Hz record with the codec"
            );
            assert_eq!(rec.latency_host_samples_pre_encoder(), RECORD_LATENCY[i]);
            assert_eq!(play.latency_host_samples(), PLAYBACK_LATENCY[i]);
            let one_pass = Pipeline::new(on).unwrap().latency_samples();
            assert_eq!(
                one_pass, ONE_PASS_LATENCY_008[i],
                "{rate} Hz one-pass with the codec"
            );
            assert_eq!(
                rec.latency_host_samples() + play.latency_host_samples(),
                one_pass
            );
            // At the taps before the decoder the one-pass codec does not run.
            for tap in [Tap::AfterRecord, Tap::AfterVas] {
                let mut before = on;
                before.tap = tap;
                let rec = RecordPath::new(&before, &plan(rate), true, false);
                assert!(!rec.codec_runs(), "{rate} Hz {tap:?}");
                assert_eq!(rec.latency_host_samples(), RECORD_LATENCY[i]);
            }
            let mut dec = on;
            dec.tap = Tap::AfterDecoder;
            assert_eq!(
                Pipeline::new(dec).unwrap().latency_samples(),
                RECORD_LATENCY[i] + CODEC_DELAY[i] + D[i],
                "{rate} Hz at the decoder tap: the record path, the delay, the conversion"
            );
        }
    }

    /// FR-008 (research R-06): N₁₀, the first device sample after which stage 10's impulse
    /// response stays below 10⁻⁵ of its own peak (−100 dB, engineering target), computed from
    /// the committed coefficients, equals the constant the drain uses.
    #[test]
    fn n10_from_committed_coefficients() {
        assert_eq!(settling_len(&PLAYBACK_SOS, 1e-5), N10_PINNED); // 006 FR-008
        assert_eq!(N10, N10_PINNED, "the drain's constant is the pinned figure");
        // The neighbouring criteria the research quotes, for orientation (−80 and −120 dB).
        assert_eq!(settling_len(&PLAYBACK_SOS, 1e-4), 95);
        assert_eq!(settling_len(&PLAYBACK_SOS, 1e-6), 146);
    }

    /// R-06: the group delays at 1 kHz the latency tables are built from.
    #[test]
    fn tau_rec_tau_play() {
        let rec = RecordStage::record().group_delay_1k_samples();
        let play = PlaybackStage::playback().group_delay_1k_samples();
        assert!((rec - TAU_REC).abs() <= 1e-3, "τ_rec {rec}");
        assert!((play - TAU_PLAY).abs() <= 1e-3, "τ_play {play}");
    }
}
