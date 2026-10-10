//! The per-sample processing engine (data-model.md; research.md R-09, R-10).

use crate::error::Error;
use crate::rate::RatePlan;
use crate::resample::{self, PolyphaseDown, PolyphaseUp};
use crate::sanitize::{narrow_out, sanitize_in};
use crate::settings::{DEVICE_RATE_HZ, Settings, Tap};
use crate::stages::agc::AgcStage;
use crate::stages::vas::{VasDecision, VasStage};
use crate::stages::voiceband::VoiceBandStage;
use crate::validate::validate;

/// One emulator instance: host-rate mono audio in, host-rate mono audio out.
///
/// Internally, each sample goes through the 8 kHz device-rate domain (A-001), the record-path
/// AGC (signal-chain stage 3, spec 002), the record band-limit stage (stage 4), the Voice
/// Activated System (VAS, stage 5, spec 003) and the playback band-limit stage (stage 10). Each
/// stage can be bypassed, and the output can be taken after the AGC, stage 4 or the VAS instead
/// ([`Settings`], FR-007). The device rate itself is always applied (FR-004). The band-limit
/// stages are modeled on the MSM7702 voice-band codec's assumed 300–3400 Hz band (A-002,
/// A-014); the AGC on an assumed AGC (A-017 – A-020); the VAS on the owner's manual and assumed
/// values (A-008, A-021 – A-025).
///
/// # Output length
///
/// With the VAS on in drop mode (the default), pauses are removed, so **a block can produce
/// fewer output samples than it consumed, or none**. Every processing call returns a
/// [`BlockInfo`]; only `output[..produced]` is output. [`process_with_events`]
/// (Self::process_with_events) also reports where the splices are.
///
/// # Output level
///
/// With the AGC on (the default), output can exceed ±1.0 by up to
/// [`AgcSettings::max_gain_db`](crate::AgcSettings::max_gain_db) (+40 dB by default) for
/// about one attack time after creation, a reset or a long silence. The output is never
/// clipped and is always finite. Limit or clip it before converting to integer PCM.
///
/// # Real-time use
///
/// [`process`](Self::process), [`process_in_place`](Self::process_in_place),
/// [`process_with_events`](Self::process_with_events), [`max_events`](Self::max_events) and
/// [`reset`](Self::reset) never allocate, lock or do I/O (FR-015, FR-017). Output, and the
/// VAS events, are bit-identical for any way of splitting the input into blocks (FR-014).
/// [`new`](Self::new) allocates and is not real-time safe.
///
/// # Threads
///
/// `Pipeline` is `Send + Sync`. It can move to the audio thread, and `&Pipeline` (for
/// [`latency_samples`](Self::latency_samples) or [`settings`](Self::settings)) can be shared.
/// Processing takes `&mut self`, so only one thread processes at a time.
///
/// # Example
///
/// ```
/// use rr_dr60::{Pipeline, Settings};
///
/// let mut p = Pipeline::new(Settings::new(48_000))?;
/// let mut buffer = vec![0.0f32; 512];
/// let info = p.process_in_place(&mut buffer); // in the audio callback
/// let output = &buffer[..info.produced]; // the VAS may have removed a pause
/// assert!(output.len() <= 512);
/// assert!(p.latency_samples() > 0);
/// # Ok::<(), rr_dr60::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Pipeline {
    settings: Settings,
    converters: Option<(PolyphaseDown, PolyphaseUp)>,
    chain: DeviceChain,
    latency: u32,
    /// Stream counters for VAS events (spec 003 research R-05, R-06).
    events: EventCounters,
}

// Contract (contracts/rust-api.md): `Pipeline` is Send + Sync. Fails to compile if a future
// field (e.g. an Rc or Cell) would silently break that.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Pipeline>();
};

/// The result of one processing call (spec 003 FR-004, FR-005; contracts/rust-api.md).
///
/// With the VAS in drop mode (the default), a block can produce fewer output samples than it
/// consumed, or none: only `output[..produced]` holds the block's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[must_use = "read `produced`: with the VAS in drop mode, only output[..produced] is output"]
#[non_exhaustive]
pub struct BlockInfo {
    /// Output samples written to the start of the output buffer. Never more than the input
    /// length; equal to it when the VAS is bypassed, not reached by the tap, or in mute mode.
    pub produced: usize,
    /// VAS events this block generated: splices in drop mode, muted regions in mute mode.
    pub events: usize,
    /// The VAS is paused at the end of the block.
    pub paused: bool,
}

/// A VAS splice (drop mode) or muted region (mute mode) (spec 003 FR-005).
///
/// Positions count output samples from the start of the stream (creation, reset or
/// reconfigure), so they don't depend on how the input was split into blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct VasEvent {
    /// Drop mode: output index of the first sample after the splice. Mute mode: output index of
    /// the region's first muted sample.
    pub output_position: u64,
    /// Drop mode: host input samples removed at this splice. Mute mode: the region's length.
    pub input_length: u64,
}

/// What the device chain did with one device sample.
#[derive(Clone, Copy, Debug)]
enum DeviceOut {
    /// Kept (recording).
    Keep(f64),
    /// Kept, and the first sample after a pause (a splice).
    Resume(f64),
    /// Dropped by the VAS (paused).
    Drop,
}

/// Stream counters that place VAS events in the output (spec 003 research R-05, R-06).
#[derive(Clone, Debug)]
struct EventCounters {
    /// Host samples per device sample is `m / l` (`l = m = 1` at the 8 kHz identity rate).
    l: u64,
    m: u64,
    /// Device samples kept (fed to the interpolator) since the stream started.
    kept: u64,
    /// Device samples dropped since the stream started.
    dropped: u64,
    /// ⌊dropped·m/l⌋ at the previous splice (the cumulative-floor rule, R-06).
    removed_reported: u64,
    /// An event generated at this step, waiting to be reported by the block loop.
    pending: Option<VasEvent>,
}

impl EventCounters {
    fn new(plan: &RatePlan) -> Self {
        let (l, m) = match *plan {
            RatePlan::Identity => (1, 1),
            RatePlan::Convert { l, m, .. } => (u64::from(l), u64::from(m)),
        };
        Self {
            l,
            m,
            kept: 0,
            dropped: 0,
            removed_reported: 0,
            pending: None,
        }
    }

    fn reset(&mut self) {
        self.kept = 0;
        self.dropped = 0;
        self.removed_reported = 0;
        self.pending = None;
    }

    /// Records one device sample's outcome; returns the sample to feed onward, if any.
    #[inline]
    fn record(&mut self, out: DeviceOut) -> Option<f64> {
        match out {
            DeviceOut::Keep(d) => {
                self.kept += 1;
                Some(d)
            }
            DeviceOut::Drop => {
                self.dropped += 1;
                None
            }
            DeviceOut::Resume(d) => {
                // R-06: the splice is at the first output whose newest device sample is this one
                // (kept index j), i.e. the first n with ⌊n·l/m⌋ ≥ j: n = ⌈j·m/l⌉. The removed
                // length uses the cumulative floor, so the integers add up at 44.1/88.2 kHz.
                let (l, m) = (u128::from(self.l), u128::from(self.m));
                let position = (u128::from(self.kept) * m).div_ceil(l) as u64;
                let removed = (u128::from(self.dropped) * m / l) as u64;
                self.pending = Some(VasEvent {
                    output_position: position,
                    input_length: removed - self.removed_reported,
                });
                self.removed_reported = removed;
                self.kept += 1;
                Some(d)
            }
        }
    }
}

/// The device-rate part of the chain (stages 3, 4, 5 and 10) and which parts of it run.
#[derive(Clone, Debug)]
struct DeviceChain {
    agc: AgcStage,
    /// Stage 3 runs: the AGC is enabled (spec 002 FR-002).
    run_agc: bool,
    vas: VasStage,
    /// Stage 5 runs: the VAS is enabled and the tap is after it (spec 003 FR-003; research R-01).
    run_vas: bool,
    record: VoiceBandStage,
    playback: VoiceBandStage,
    /// Stage 4 runs: enabled and the tap is not after the AGC (FR-007; spec 002 FR-003).
    run_record: bool,
    /// Stage 10 runs: enabled and the tap is after playback (data-model.md › Tap).
    run_playback: bool,
}

impl DeviceChain {
    fn new(settings: &Settings) -> Self {
        Self {
            agc: AgcStage::new(&settings.agc),
            run_agc: settings.agc.enabled,
            vas: VasStage::new(&settings.vas),
            run_vas: settings.vas.enabled
                && matches!(settings.tap, Tap::AfterVas | Tap::AfterPlayback),
            record: VoiceBandStage::new(),
            playback: VoiceBandStage::new(),
            run_record: settings.record_stage_enabled && settings.tap != Tap::AfterAgc,
            run_playback: settings.playback_stage_enabled && settings.tap == Tap::AfterPlayback,
        }
    }

    /// One device-rate sample. A bypassed stage passes the sample through bit-exactly,
    /// with no arithmetic (FR-007). The AGC runs on every sample, also while the VAS is paused
    /// (spec 003 FR-016, A-025); a dropped sample never reaches stage 10.
    #[inline]
    fn process(&mut self, mut d: f64) -> DeviceOut {
        if self.run_agc {
            d = self.agc.process(d);
        }
        if self.run_record {
            d = self.record.process(d);
        }
        let decision = if self.run_vas {
            self.vas.process(d)
        } else {
            VasDecision::Keep
        };
        if decision == VasDecision::Drop {
            return DeviceOut::Drop;
        }
        if self.run_playback {
            d = self.playback.process(d);
        }
        if decision == VasDecision::Resume {
            DeviceOut::Resume(d)
        } else {
            DeviceOut::Keep(d)
        }
    }

    /// The VAS runs and is paused.
    fn paused(&self) -> bool {
        self.run_vas && self.vas.is_paused()
    }

    /// Group delay at 1 kHz of the stages that run, in device samples (R-10).
    fn group_delay_1k_samples(&self) -> f64 {
        let mut tau = 0.0;
        if self.run_record {
            tau += self.record.group_delay_1k_samples();
        }
        if self.run_playback {
            tau += self.playback.group_delay_1k_samples();
        }
        tau
    }

    fn reset(&mut self) {
        self.agc.reset();
        self.vas.reset();
        self.record.reset();
        self.playback.reset();
    }
}

impl Pipeline {
    /// Validates `settings` and allocates every buffer.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not in
    /// [`SUPPORTED_HOST_RATES`](crate::SUPPORTED_HOST_RATES), checked first.
    /// [`Error::InvalidSetting`] if an AGC or VAS setting is out of range or not finite
    /// (spec 002 FR-011, spec 003 FR-012).
    pub fn new(settings: Settings) -> Result<Self, Error> {
        validate(&settings)?;
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        let chain = DeviceChain::new(&settings);
        let latency = latency_samples(&plan, &settings, &chain);
        Ok(Self {
            settings,
            converters: resample::converters(&plan),
            chain,
            latency,
            events: EventCounters::new(&plan),
        })
    }

    /// Processes one block. `input` and `output` must have the same length, which may be
    /// any length including 0. Real-time safe.
    ///
    /// Only `output[..produced]` is written (see [`BlockInfo`]); the rest is left unchanged.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if the lengths differ. The pipeline state is not changed.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<BlockInfo, Error> {
        if input.len() != output.len() {
            return Err(Error::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        Ok(self.run_block(Some(input), output, &mut []))
    }

    /// Processes one block in place. Real-time safe.
    ///
    /// The block's output is `buffer[..produced]` (see [`BlockInfo`]); samples past `produced`
    /// are unspecified. Output index never exceeds input index, so in-place processing is safe.
    pub fn process_in_place(&mut self, buffer: &mut [f32]) -> BlockInfo {
        self.run_block(None, buffer, &mut [])
    }

    /// As [`process`](Self::process), and writes the block's VAS events (splices in drop mode,
    /// muted regions in mute mode) to `events`: the first `events.len()` of them, in order.
    /// [`BlockInfo::events`] counts all of them, so `info.events > events.len()` means some
    /// were not written; [`max_events`](Self::max_events) gives a length that is always enough.
    /// Real-time safe (spec 003 FR-005).
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if `input` and `output` lengths differ. The pipeline state is
    /// not changed.
    pub fn process_with_events(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        events: &mut [VasEvent],
    ) -> Result<BlockInfo, Error> {
        if input.len() != output.len() {
            return Err(Error::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        Ok(self.run_block(Some(input), output, events))
    }

    /// An `events` length for [`process_with_events`](Self::process_with_events) that is always
    /// enough for a block of `frames` input samples with the current settings (spec 003 R-07).
    /// Splices are at least H + 2 device samples apart, and `frames` host samples hold at most
    /// ⌊frames·l/m⌋ + 1 device samples (engineering target, 003 R-15). Never allocates.
    pub fn max_events(&self, frames: usize) -> usize {
        let (l, m) = (u128::from(self.events.l), u128::from(self.events.m));
        let device = frames as u128 * l / m;
        let spacing = u128::from(self.chain.vas.hang_samples()) + 2;
        usize::try_from(device / spacing + 2).unwrap_or(usize::MAX)
    }

    /// The one block loop behind every processing call. Reads `input[i]` (or `out[i]` when
    /// processing in place) and writes emitted samples to `out[..produced]`; `produced ≤ i + 1`
    /// at every step, so in-place processing never overwrites unread input.
    fn run_block(
        &mut self,
        input: Option<&[f32]>,
        out: &mut [f32],
        events: &mut [VasEvent],
    ) -> BlockInfo {
        let (mut produced, mut count) = (0, 0);
        for i in 0..out.len() {
            let x = match input {
                Some(input) => input[i],
                None => out[i],
            };
            if let Some(y) = self.tick(x) {
                out[produced] = y;
                produced += 1;
            }
            if let Some(event) = self.events.pending.take() {
                if let Some(slot) = events.get_mut(count) {
                    *slot = event;
                }
                count += 1;
            }
        }
        BlockInfo {
            produced,
            events: count,
            paused: self.chain.paused(),
        }
    }

    /// Fixed latency of the current configuration, in host-rate samples (FR-012).
    ///
    /// This is the rate conversion's delay plus the voice-band stages' group delay at 1 kHz,
    /// rounded to the nearest sample (research.md R-10).
    pub fn latency_samples(&self) -> u32 {
        self.latency
    }

    /// Returns the pipeline to its freshly created state, without allocating (FR-017).
    pub fn reset(&mut self) {
        if let Some((down, up)) = &mut self.converters {
            down.reset();
            up.reset();
        }
        self.chain.reset();
        self.events.reset();
    }

    /// Applies new settings and resets all state, as if the pipeline had been created anew
    /// with `settings` (FR-008). The new configuration is built completely before it replaces
    /// the old one, so on error the pipeline is unchanged: same settings, latency and state.
    ///
    /// May allocate; **not** real-time safe. Call it between streams, not from the audio
    /// callback.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not supported, or
    /// [`Error::InvalidSetting`] if an AGC or VAS setting is invalid (spec 002 FR-011,
    /// spec 003 FR-012).
    pub fn reconfigure(&mut self, settings: Settings) -> Result<(), Error> {
        *self = Self::new(settings)?;
        Ok(())
    }

    /// The current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// One host sample through the engine (data-model.md steps 1–3). Returns the host sample
    /// emitted at this step, if any (spec 003 research R-05).
    #[inline]
    fn tick(&mut self, x: f32) -> Option<f32> {
        let x = sanitize_in(x);
        let y = match &mut self.converters {
            None => self.events.record(self.chain.process(x))?,
            Some((down, up)) => {
                if let Some(d) = down.push(x) {
                    // No let-chains: MSRV 1.85 (001 R-01).
                    if let Some(kept) = self.events.record(self.chain.process(d)) {
                        up.push_device(kept);
                    }
                }
                up.try_next_host()?
            }
        };
        Some(narrow_out(y))
    }
}

#[cfg(feature = "__test-hooks")]
impl Pipeline {
    /// Test-only (hidden): a pipeline whose voice-band stages use `sos` instead of the
    /// committed coefficients. Used by the SC-008 mutation test (tasks.md T057).
    ///
    /// # Errors
    ///
    /// As [`Pipeline::new`].
    #[doc(hidden)]
    pub fn __with_stage_sos(settings: Settings, sos: [[f64; 5]; 6]) -> Result<Self, Error> {
        let mut p = Self::new(settings)?;
        p.chain.record = VoiceBandStage::with_sos(sos);
        p.chain.playback = VoiceBandStage::with_sos(sos);
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        p.latency = latency_samples(&plan, &settings, &p.chain);
        Ok(p)
    }
}

/// R-10: round(D_down + D_up + (τ_rec + τ_play) · host / 8000). A bypassed stage contributes
/// 0, and with the tap after the record stage τ_play is not included.
fn latency_samples(plan: &RatePlan, settings: &Settings, chain: &DeviceChain) -> u32 {
    let stages = chain.group_delay_1k_samples();
    let total = 2.0 * f64::from(plan.delay_host_samples())
        + stages * f64::from(settings.host_rate_hz) / f64::from(DEVICE_RATE_HZ);
    (total + 0.5) as u32
}

#[cfg(test)]
mod validation_tests {
    extern crate std;
    use super::*;
    use crate::error::Setting;
    use std::vec::Vec;

    /// 002 T014: with the tap after the AGC, stages 4 and 10 do not run, whatever their
    /// settings (spec 002 FR-003, data-model.md › Tap).
    #[test]
    fn tap_after_agc_skips_both_band_limit_stages() {
        let x: Vec<f32> = (0..9600)
            .map(|n| if n % 97 == 0 { 0.5 } else { -0.01 })
            .collect();
        let run = |s: Settings| {
            let mut y = x.clone();
            let _ = Pipeline::new(s).unwrap().process_in_place(&mut y);
            y
        };
        for rate in crate::SUPPORTED_HOST_RATES {
            let mut tap_agc = Settings::new(rate);
            tap_agc.agc.enabled = false; // isolate the tap logic from the AGC stage
            tap_agc.tap = Tap::AfterAgc;
            let mut bypass = tap_agc;
            bypass.tap = Tap::AfterPlayback;
            bypass.record_stage_enabled = false;
            bypass.playback_stage_enabled = false;
            assert_eq!(run(tap_agc), run(bypass), "{rate} Hz");
            assert_eq!(
                Pipeline::new(tap_agc).unwrap().latency_samples(),
                Pipeline::new(bypass).unwrap().latency_samples()
            );
        }
    }

    /// 002 T006: AGC settings are validated on creation (FR-011).
    #[test]
    fn new_rejects_invalid_agc_setting() {
        let mut s = Settings::new(48_000);
        s.agc.attack_ms = 0.0;
        assert_eq!(
            Pipeline::new(s).err(),
            Some(Error::InvalidSetting {
                setting: Setting::AgcAttackMs
            })
        );
    }

    /// 002 T006: a failed reconfigure leaves settings, latency and state unchanged (FR-011,
    /// 001 FR-008).
    #[test]
    fn failed_reconfigure_leaves_pipeline_unchanged() {
        let x: Vec<f32> = (0..4800)
            .map(|n| if n % 50 == 0 { 0.5 } else { 0.0 })
            .collect();
        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        let mut warm = x.clone();
        let _ = p.process_in_place(&mut warm);
        let mut untouched = p.clone();

        let mut bad = Settings::new(48_000);
        bad.agc.release_ms = f32::NAN;
        assert_eq!(
            p.reconfigure(bad),
            Err(Error::InvalidSetting {
                setting: Setting::AgcReleaseMs
            })
        );
        assert_eq!(p.settings(), untouched.settings());
        assert_eq!(p.latency_samples(), untouched.latency_samples());
        let (mut a, mut b) = (x.clone(), x);
        let _ = p.process_in_place(&mut a);
        let _ = untouched.process_in_place(&mut b);
        assert_eq!(a, b);
    }

    /// 003 T005: VAS settings are validated on creation (spec 003 FR-012).
    #[test]
    fn new_rejects_invalid_vas_setting() {
        let mut s = Settings::new(48_000);
        s.vas.sensitivity = 6;
        assert_eq!(
            Pipeline::new(s).err(),
            Some(Error::InvalidSetting {
                setting: Setting::VasSensitivity
            })
        );
    }

    /// 003 T005: a failed reconfigure with a bad VAS setting changes nothing (FR-012).
    #[test]
    fn failed_vas_reconfigure_leaves_pipeline_unchanged() {
        let x: Vec<f32> = (0..4800)
            .map(|n| if n % 50 == 0 { 0.5 } else { 0.0 })
            .collect();
        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        let mut warm = x.clone();
        let _ = p.process_in_place(&mut warm);
        let mut untouched = p.clone();
        let mut bad = Settings::new(48_000);
        bad.vas.onset_ms = f32::INFINITY;
        assert_eq!(
            p.reconfigure(bad),
            Err(Error::InvalidSetting {
                setting: Setting::VasOnsetMs
            })
        );
        assert_eq!(p.settings(), untouched.settings());
        assert_eq!(p.latency_samples(), untouched.latency_samples());
        let (mut a, mut b) = (x.clone(), x);
        let ia = p.process_in_place(&mut a);
        let ib = untouched.process_in_place(&mut b);
        assert_eq!((a, ia), (b, ib));
    }

    /// 003 T005: VAS bypassed (and before T020, absent), every block produces exactly as many
    /// samples as it consumes, with no events and never paused, for every tap and rate
    /// (spec 003 FR-002, FR-004).
    #[test]
    fn block_info_is_one_in_one_out_without_vas() {
        let x: Vec<f32> = (0..2400)
            .map(|n| if n % 31 == 0 { 0.4 } else { 0.0 })
            .collect();
        for rate in crate::SUPPORTED_HOST_RATES {
            for tap in [
                Tap::AfterAgc,
                Tap::AfterRecord,
                Tap::AfterVas,
                Tap::AfterPlayback,
            ] {
                let mut s = Settings::new(rate);
                s.vas.enabled = false;
                s.tap = tap;
                let mut p = Pipeline::new(s).unwrap();
                let mut y = std::vec![0.0f32; 333];
                let info = p.process(&x[..333], &mut y).unwrap();
                assert_eq!(
                    (info.produced, info.events, info.paused),
                    (333, 0, false),
                    "{rate} Hz {tap:?}"
                );
                let mut buf = x.clone();
                let info = p.process_in_place(&mut buf);
                assert_eq!((info.produced, info.events, info.paused), (2400, 0, false));
                let info = p.process(&[], &mut []).unwrap();
                assert_eq!(info.produced, 0);
            }
        }
    }

    /// 003 T005: with the tap after VAS, stage 10 does not run (spec 003 FR-003; research R-01).
    #[test]
    fn tap_after_vas_skips_the_playback_stage() {
        let x: Vec<f32> = (0..9600)
            .map(|n| if n % 97 == 0 { 0.5 } else { -0.01 })
            .collect();
        let run = |s: Settings| {
            let mut y = x.clone();
            let _ = Pipeline::new(s).unwrap().process_in_place(&mut y);
            y
        };
        for rate in crate::SUPPORTED_HOST_RATES {
            let mut tap_vas = Settings::new(rate);
            tap_vas.vas.enabled = false; // isolate the tap logic from the VAS stage
            tap_vas.tap = Tap::AfterVas;
            let mut no_playback = tap_vas;
            no_playback.tap = Tap::AfterPlayback;
            no_playback.playback_stage_enabled = false;
            assert_eq!(run(tap_vas), run(no_playback), "{rate} Hz");
            assert_eq!(
                Pipeline::new(tap_vas).unwrap().latency_samples(),
                Pipeline::new(no_playback).unwrap().latency_samples()
            );
        }
    }
}

#[cfg(test)]
mod vas_event_tests {
    extern crate std;
    use super::*;
    use rr_dr60_detmath::{TAU, exp, ln, sin};
    use std::vec::Vec;

    /// The VAS-isolated configuration (spec 003 definitions): AGC and stages 4 and 10 off.
    fn vas_only(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.tap = Tap::AfterVas;
        s
    }

    /// 1 kHz at −8 dBFS (threshold + 10 dB) with a half-sample phase offset at 8 kHz, so every
    /// device sample is a sound sample and burst lengths are exact (see `stages::vas` tests).
    fn loud(n: usize, fs: f64) -> Vec<f32> {
        let amp = exp(-8.0 * ln(10.0) / 20.0);
        (0..n)
            .map(|k| (amp * sin(TAU * ((1000.0 * (k as f64 + 0.5) / fs) % 1.0))) as f32)
            .collect()
    }

    fn burst_gap_burst(fs: f64) -> Vec<f32> {
        let s = fs as usize;
        [loud(s, fs), std::vec![0.0; 5 * s], loud(s, fs)].concat()
    }

    fn run_all(p: &mut Pipeline, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, BlockInfo) {
        let mut y = std::vec![0.0f32; x.len()];
        let mut ev = std::vec![VasEvent::default(); p.max_events(x.len())];
        let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
        y.truncate(info.produced);
        ev.truncate(info.events);
        (y, ev, info)
    }

    /// 003 T018 (FR-004, FR-005; research R-06): at 8 kHz, burst 1 s, gap 5 s, burst 1 s keeps
    /// 8000 + 8000 (hang) + 8000 − 160 (onset) samples, with one splice at output 16000 that
    /// removed 40000 − 8000 + 160 input samples.
    #[test]
    fn splice_at_8k_is_exact() {
        let x = burst_gap_burst(8000.0);
        let mut p = Pipeline::new(vas_only(8000)).unwrap();
        let (y, ev, info) = run_all(&mut p, &x);
        assert_eq!(y.len(), 8000 + 8000 + 8000 - 160);
        assert_eq!(
            ev,
            [VasEvent {
                output_position: 16_000,
                input_length: 32_160
            }]
        );
        assert_eq!(info.events, 1);
        assert!(!info.paused);
        assert_eq!(ev[0].input_length as usize + y.len(), x.len());
        // The kept samples are the input, bit for bit: the first burst and the hang time, then
        // the second burst minus the onset (8 kHz is the device rate; nothing else is in the path).
        assert_eq!(&y[..16_000], &x[..16_000]);
        assert_eq!(&y[16_000..], &x[16_000 + 32_160..]);
    }

    /// 003 T018: at 48 kHz (m/l = 6) the removed lengths and the output add up to the input
    /// exactly, the removed length is a whole number of device samples and within one device
    /// sample of 6 × the 8 kHz value (the decimator's edge smear can move the pause by one device
    /// sample; research R-11 › Measured). The position also carries the decimator's delay, so
    /// only its alignment is checked.
    #[test]
    fn splice_at_48k_adds_up() {
        let x = burst_gap_burst(48_000.0);
        let mut p = Pipeline::new(vas_only(48_000)).unwrap();
        let (y, ev, _) = run_all(&mut p, &x);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].input_length % 6, 0);
        let removed: u64 = ev.iter().map(|e| e.input_length).sum();
        assert_eq!(removed as usize + y.len(), x.len());
        assert_eq!(ev[0].output_position % 6, 0);
        let device = 6; // ± one device sample, engineering target (003 FR-017)
        let length = i64::try_from(ev[0].input_length).unwrap();
        assert!(
            (length - 6 * 32_160).abs() <= device,
            "{}",
            ev[0].input_length
        );
    }

    /// 003 T018: a too-short event slice still counts the event, and writes nothing.
    #[test]
    fn short_event_slice_counts_but_does_not_write() {
        let x = burst_gap_burst(8000.0);
        let mut p = Pipeline::new(vas_only(8000)).unwrap();
        let mut y = std::vec![0.0f32; x.len()];
        let info = p.process_with_events(&x, &mut y, &mut []).unwrap();
        assert_eq!(info.events, 1);
        assert!(
            p.process_with_events(&x[..3], &mut y[..2], &mut [])
                .is_err()
        );
    }

    /// 003 T018 (R-07): `max_events` is enough for every block of a 60 s run with a 50 ms hang
    /// time (a splice every 130 ms), at every rate, for varied block sizes.
    #[test]
    fn max_events_is_always_enough() {
        for rate in crate::SUPPORTED_HOST_RATES {
            let fs = f64::from(rate);
            let mut s = vas_only(rate);
            s.vas.hang_ms = 50.0;
            let mut p = Pipeline::new(s).unwrap();
            let cycle = [
                loud((0.03 * fs) as usize, fs),
                std::vec![0.0; (0.1 * fs) as usize],
            ]
            .concat();
            let x: Vec<f32> = cycle
                .iter()
                .copied()
                .cycle()
                .take(60 * rate as usize)
                .collect();
            let (mut pos, mut size, mut total) = (0, 1usize, 0);
            let mut y = std::vec![0.0f32; 8192];
            let mut ev = std::vec![VasEvent::default(); 4096];
            while pos < x.len() {
                let n = size.min(x.len() - pos);
                let cap = p.max_events(n);
                let info = p
                    .process_with_events(&x[pos..pos + n], &mut y[..n], &mut ev[..cap])
                    .unwrap();
                assert!(info.events <= cap, "{rate} Hz: {} > {cap}", info.events);
                total += info.events;
                pos += n;
                size = size * 7 % 8191 + 1;
            }
            assert!(total > 400, "{rate} Hz: only {total} splices");
        }
    }

    /// 003 T018: `paused` is true at the end of a block that ends in a dropped stretch.
    #[test]
    fn paused_flag_reports_the_end_of_the_block() {
        let mut p = Pipeline::new(vas_only(8000)).unwrap();
        let mut y = std::vec![0.0f32; 20_000];
        let info = p.process(&loud(20_000, 8000.0), &mut y).unwrap();
        assert!(!info.paused);
        let info = p.process(&[0.0; 10_000], &mut y[..10_000]).unwrap();
        assert!(info.paused);
        assert_eq!(info.produced, 8000);
        p.reset();
        let info = p.process(&[0.0; 10], &mut y[..10]).unwrap();
        assert!(!info.paused && info.produced == 10);
    }
}

#[cfg(all(test, feature = "op-count"))]
mod op_count_tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    impl Pipeline {
        fn ops(&self) -> u64 {
            let conv = self.converters.as_ref().map_or(0, |(d, u)| d.ops + u.ops);
            conv + self.chain.agc.ops
                + self.chain.vas.ops
                + self.chain.record.ops()
                + self.chain.playback.ops()
        }

        fn taps(&self) -> (u64, u64) {
            self.converters
                .as_ref()
                .map_or((0, 0), |(d, u)| (d.taps() as u64, u.taps() as u64))
        }
    }

    /// FR-015 bounded work: no host sample costs more than one full decimator branch, one
    /// interpolator branch, both band-limit stages (2 × 6 sections × 5 multiply-adds) and one
    /// AGC update (spec 002) and one VAS decision (spec 003), and the total is independent of
    /// how the input is split into blocks. The default settings have the AGC and the VAS on, and
    /// the input has long gaps, so the VAS both keeps and drops samples.
    #[test]
    fn per_sample_work_is_bounded_and_block_independent() {
        for rate in crate::SUPPORTED_HOST_RATES {
            // A quarter second of clicks, then 1.5 s of near-silence (longer than the 1 s hang
            // time, so the VAS pauses), then clicks again (it resumes).
            let r = rate as usize;
            let x: Vec<f32> = (0..r / 4 + 3 * r / 2 + r / 4)
                .map(|n| {
                    let quiet = n >= r / 4 && n < r / 4 + 3 * r / 2;
                    if !quiet && n % 97 == 0 { 0.5 } else { -0.0001 }
                })
                .collect();
            let mut one = Pipeline::new(Settings::new(rate)).unwrap();
            let (taps_down, taps_up) = one.taps();
            // + the AGC's fixed per-device-sample work (002 T024: 32 Hilbert multiply-adds,
            // 33 compares and 12 scalar operations) + the VAS's (003 T023: one compare and a few
            // counter updates).
            let bound = taps_down + taps_up + 60 + 77 + 4;
            let mut worst = 0;
            for &v in &x {
                let before = one.ops();
                let _ = one.process_in_place(&mut [v]);
                worst = worst.max(one.ops() - before);
            }
            assert!(
                worst <= bound,
                "{rate} Hz: {worst} multiply-adds in one sample > bound {bound}"
            );
            assert!(worst > 0);
            let mut block = Pipeline::new(Settings::new(rate)).unwrap();
            let mut buf = x.clone();
            let produced = block.process_in_place(&mut buf).produced;
            assert!(produced < buf.len(), "{rate} Hz: the VAS dropped nothing");
            buf.truncate(produced);
            assert_eq!(
                block.ops(),
                one.ops(),
                "{rate} Hz: work depends on block size"
            );
        }
    }
}
