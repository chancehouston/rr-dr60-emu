//! The per-sample processing engine (data-model.md; research.md R-09, R-10; spec 005 research
//! R-01: the limiter runs at the host rate, before the rate conversion; spec 006 research R-01:
//! the one-pass is a record path feeding a playback path).

use crate::codec::{self, SpeechDecoder, SpeechEncoder};
use crate::error::Error;
use crate::memory::{Encoding, Memory};
use crate::paths::{self, PlaybackPath, RecordPath};
use crate::rate::RatePlan;
use crate::settings::{DEVICE_RATE_HZ, Route, Settings, Tap};
use crate::transport::Transport;
use crate::validate::validate;

/// One emulator instance: host-rate mono audio in, host-rate mono audio out.
///
/// The record path is the limiter (signal-chain stage 3, spec 005) at the host rate, the
/// host-to-device conversion into the 6 kHz device-rate domain (A-026), the record band-limit
/// stage (stage 4), the ADC's full-scale saturation (the clamp, spec 005 FR-008), and the Voice
/// Activated System (VAS, stage 5, spec 003); the playback path is the playback band-limit
/// stage (stage 10) and the device-to-host conversion. Each stage can be bypassed, and the
/// output can be taken after the limiter (at the host rate), after stage 4 or after the VAS
/// instead ([`Settings`], FR-007). The device rate itself is always applied (FR-004) except at
/// the "after AGC" tap, which is before the conversion. The band-limit stages are modeled on the
/// MSM7702 codec family's templates (A-027); the limiter on the schematic's circuit (A-028,
/// A-034 – A-036); the VAS on the owner's manual and assumed values (A-008, A-021 – A-025).
///
/// Since 0.6 the device-rate stream is rounded to 32-bit precision after the VAS, the point
/// where a recording is taken (spec 006 FR-009; in the device an 8-bit µ-law code sits there,
/// A-003); the one-pass processing and record-then-play therefore agree bit for bit.
///
/// # Output length
///
/// With the VAS on in drop mode (the default), pauses are removed, so **a block can produce
/// fewer output samples than it consumed, or none**. Every processing call returns a
/// [`BlockInfo`]; only `output[..produced]` is output. [`process_with_events`](Self::process_with_events)
/// also reports where the splices are.
///
/// # Output level
///
/// 0 dBFS is the codec's full scale (A-030). The limiter never amplifies: below its knee it is
/// the identity, and sustained input above the knee is held about 1 dB below full scale; only
/// the dead time of a loud onset reaches the ADC clamp, so the record path (the "after record"
/// and "after VAS" taps) never exceeds ±1.0. The playback band-limit stage reconstructs a
/// clipped onset with overshoot (the fundamental of a full-scale square wave is 4/π), so the
/// default output can exceed ±1.0 by a few dB there. With the clamp bypassed, or at the "after
/// AGC" tap, the output carries the input's excess during the limiter's dead time. The output
/// is always finite.
///
/// # Record and playback paths
///
/// Since 0.6 the engine also has the device's structure (spec 006): a **record path**
/// (stages 3–5) that appends device-rate samples to a recording in a modeled **memory**
/// (60 minutes, 99 recordings by default, S-001; [`MemorySettings`](crate::MemorySettings)),
/// and a **playback path** (stage 10) that plays a stored recording toward a
/// [`Route`](crate::Route), speaker or earphone. The engine is **idle**, **recording** or
/// **playing** ([`state`](Self::state)); the two never run at once (the device mutes its
/// amplifier while recording, S-002). The loop: [`start_recording`](Self::start_recording),
/// [`record`](Self::record) per block, [`stop_recording`](Self::stop_recording) (which
/// flushes the record path's latency and reports the recording's number), then
/// [`start_playback`](Self::start_playback), [`play`](Self::play) per block until
/// `finished` (the playback drains to settled). Record-then-play is bit-identical to the
/// one-pass processing of the same input (FR-007), and the one-pass calls stay available
/// while the engine is idle. The stored unit is the speech codec's 10-byte frame (spec 008;
/// the engine's device-rate 32-bit samples with the codec bypassed, [`encoding`](Self::encoding));
/// the host never parses the exported form, it exports and imports it as opaque bytes.
///
/// # Real-time use
///
/// [`process`](Self::process), [`process_in_place`](Self::process_in_place),
/// [`process_with_events`](Self::process_with_events), [`max_events`](Self::max_events),
/// [`reset`](Self::reset), and of the record and playback paths `start_recording`, `record`,
/// `max_record_events`, `stop_recording`, `start_playback`, `play`, `stop_playback`,
/// `set_route`, `route`, `state`, `memory_status`, `recording_length`, the latency and drain
/// reads and `footprint_bytes` never allocate, lock or do I/O (FR-015, FR-017; spec 006
/// FR-017). Output, and the VAS events, are bit-identical for any way of splitting the input
/// into blocks (FR-014). [`new`](Self::new) and [`reconfigure`](Self::reconfigure) allocate
/// (the recording store for the full configured capacity is reserved there, about 86 MB at
/// the defaults, virtual until written) and are not real-time safe.
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
///
/// // Or the way the device works (spec 006): record into the memory, then play back.
/// use rr_dr60::StopResult;
/// let mut p = Pipeline::new(Settings::new(48_000))?;
/// let input = vec![0.1f32; 4800];
/// p.start_recording()?;
/// let info = p.record(&input, &mut [])?; // any block length; events optional
/// assert!(info.stopped.is_none()); // the memory did not fill
/// let outcome = p.stop_recording(&mut [])?; // flushes the record latency
/// let StopResult::Kept { number, length } = outcome.result else { unreachable!() };
/// assert!(length >= 600); // device samples at 6 kHz
/// p.start_playback(number)?;
/// let mut out = vec![0.0f32; 512];
/// let mut played = 0;
/// loop {
///     let info = p.play(&mut out)?;
///     played += info.produced;
///     if info.finished {
///         break;
///     }
/// }
/// assert_eq!(played as u32, p.drain_samples() + 8 * length as u32); // 48 kHz: 8 host samples per device sample
/// # Ok::<(), rr_dr60::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Pipeline {
    pub(crate) settings: Settings,
    /// The one-pass stream: its own record and playback path instances (spec 006 research
    /// R-01; Complexity Tracking row 1).
    pub(crate) one_pass: OnePass,
    pub(crate) latency: u32,
    /// The split engine's record path (spec 006 FR-001): ignores the tap, device-sample event
    /// positions, reset at every `start_recording`.
    pub(crate) recorder: RecordPath,
    /// The split engine's playback path (spec 006 FR-002): ignores the tap, reset at every
    /// `start_playback`.
    pub(crate) player: PlaybackPath,
    pub(crate) memory: Memory,
    pub(crate) transport: Transport,
    /// The output route (spec 006 FR-015): not a setting; kept through reset and reconfigure.
    pub(crate) route: Route,
    /// The volume position (spec 009 FR-003): not a setting; kept through reset and
    /// reconfigure; copied into both playback paths' chains at every processing call.
    pub(crate) volume: f32,
    /// The rate plan's `(l, m)`: `m / l` host samples per device sample.
    pub(crate) plan_l_m: (u64, u64),
    /// The recorder's speech encoder (stage 7; spec 008 research R-06), `None` when the codec
    /// is bypassed; reset at every `start_recording`.
    pub(crate) encoder: Option<SpeechEncoder>,
    /// The player's speech decoder (stage 9), `None` when bypassed; reset at every
    /// `start_playback`.
    pub(crate) decoder: Option<SpeechDecoder>,
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
    /// Speech-codec frames encoded in this block (spec 008 FR-008); 0 with the codec bypassed
    /// or at a tap before it.
    pub frames: usize,
}

/// A VAS splice (drop mode) or muted region (mute mode) (spec 003 FR-005).
///
/// Positions count output samples from the start of the stream (creation, reset or
/// reconfigure), so they don't depend on how the input was split into blocks. The record path
/// of spec 006 reports them in the recording's device samples from its start instead.
///
/// `repr(C)`, so the C API can hand the core a caller-provided event buffer without copying.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct VasEvent {
    /// Drop mode: output index of the first sample after the splice. Mute mode: output index of
    /// the region's first muted sample.
    pub output_position: u64,
    /// Drop mode: host input samples removed at this splice. Mute mode: the region's length.
    pub input_length: u64,
}

/// The speech codec in the one-pass stream (spec 008 research R-06): the encoder, the decoder
/// and a delay line that makes the codec's delay exactly 180 kept samples (`DELAY_SAMPLES`), a
/// whole number of host samples at every rate, so record-then-play stays bit-identical to the
/// one-pass (FR-009). A frame's 120 decoded samples enter the line when the encoder emits it
/// (on kept sample 120(k+1) + 59); one sample leaves per kept sample, 0.0 for the first 180.
#[derive(Clone, Debug)]
struct OnePassCodec {
    enc: SpeechEncoder,
    dec: SpeechDecoder,
    /// The ring: at most 121 samples wait at any time (research R-06), so 180 slots suffice.
    ring: [f32; CODEC_RING],
    read: usize,
    write: usize,
    len: usize,
    /// Kept samples seen while the line fills (0 … 180).
    filling: u32,
    /// The decoder's output buffer.
    decoded: [f32; codec::FRAME_SAMPLES as usize],
}

/// The delay line's length: the codec's delay.
const CODEC_RING: usize = codec::DELAY_SAMPLES as usize;

impl OnePassCodec {
    fn new() -> Self {
        Self {
            enc: SpeechEncoder::new(),
            dec: SpeechDecoder::new(),
            ring: [0.0; CODEC_RING],
            read: 0,
            write: 0,
            len: 0,
            filling: 0,
            decoded: [0.0; codec::FRAME_SAMPLES as usize],
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// One kept post-VAS sample in, the delayed decoded sample out; counts the frame it
    /// completes in `frames`.
    #[inline]
    fn step(&mut self, kept: f32, frames: &mut usize) -> f32 {
        if let Some(frame) = self.enc.push(kept) {
            *frames += 1;
            self.dec.decode(&frame, &mut self.decoded);
            for &v in &self.decoded {
                debug_assert!(self.len < CODEC_RING, "the delay line holds at most 121");
                self.ring[self.write] = v;
                self.write = (self.write + 1) % CODEC_RING;
                self.len += 1;
            }
        }
        if self.filling < codec::DELAY_SAMPLES {
            self.filling += 1;
            return 0.0;
        }
        debug_assert!(
            self.len > 0,
            "a sample is due at every kept sample after the delay"
        );
        let v = self.ring[self.read];
        self.read = (self.read + 1) % CODEC_RING;
        self.len = self.len.saturating_sub(1);
        v
    }
}

/// The one-pass stream: a record path feeding a playback path sample by sample (spec 006
/// research R-01). Both honour the tap; the counters live in the record path.
#[derive(Clone, Debug)]
pub(crate) struct OnePass {
    pub(crate) rec: RecordPath,
    pub(crate) play: PlaybackPath,
    /// The speech codec between the two paths (spec 008 FR-002, FR-015): present when it is on
    /// and the tap is at the decoder or after it.
    codec: Option<OnePassCodec>,
    /// The output is the limiter's, at the host rate: nothing after it runs (spec 005 FR-001).
    tap_after_agc: bool,
    /// Test-only: `false` runs the pre-0.6 un-narrowed route, so `narrowing_delta` can measure
    /// FR-009's effect (spec 006 research R-02). The shipped build has no flag and no branch.
    #[cfg(any(test, feature = "__test-hooks"))]
    narrow: bool,
}

impl OnePass {
    fn new(settings: &Settings, plan: &RatePlan) -> Self {
        let rec = RecordPath::new(settings, plan, true, false);
        Self {
            codec: rec.codec_runs().then(OnePassCodec::new),
            rec,
            play: PlaybackPath::new(settings, plan, true),
            tap_after_agc: settings.tap == Tap::AfterAgc,
            #[cfg(any(test, feature = "__test-hooks"))]
            narrow: true,
        }
    }

    /// One host sample through the engine (data-model.md steps 1–3; spec 005 data-model.md ›
    /// Pipeline order and taps; spec 008 research R-06: the codec between the two paths).
    /// Returns the host sample emitted at this step, if any (spec 003 research R-05), and
    /// counts a completed speech frame in `frames`.
    #[inline]
    fn tick(&mut self, x: f32, frames: &mut usize) -> Option<f32> {
        if self.tap_after_agc {
            // The limiter's output sample for sample: no conversion, no chain, no events
            // (spec 005 FR-001).
            return Some(self.rec.limiter_only(x));
        }
        #[cfg(any(test, feature = "__test-hooks"))]
        if !self.narrow {
            if let Some(d) = self.rec.push_wide(x) {
                self.play.push_device_wide(d);
            }
            return self.play.try_next_host();
        }
        // No let-chains: MSRV 1.85 (001 R-01).
        if let Some(kept) = self.rec.push(x) {
            let d = match &mut self.codec {
                Some(c) => c.step(kept, frames),
                None => kept,
            };
            self.play.push_device(d);
        }
        self.play.try_next_host()
    }

    fn reset(&mut self) {
        self.rec.reset();
        self.play.reset();
        if let Some(c) = &mut self.codec {
            c.reset();
        }
    }
}

impl Pipeline {
    /// Validates `settings` and allocates every buffer.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not in
    /// [`SUPPORTED_HOST_RATES`](crate::SUPPORTED_HOST_RATES), checked first.
    /// [`Error::InvalidSetting`] if an AGC, VAS or memory setting is out of range or not
    /// finite (spec 002 FR-011, spec 003 FR-012, spec 006 FR-010).
    /// [`Error::AllocationFailed`] if the recording store for the configured capacity cannot
    /// be reserved (spec 006 FR-017).
    pub fn new(settings: Settings) -> Result<Self, Error> {
        validate(&settings)?;
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        // Spec 008 FR-012: the codec setting fixes the memory's encoding.
        let memory = Memory::new(&settings.memory, Encoding::for_settings(&settings))?;
        Ok(Self::with_memory(
            settings,
            &plan,
            memory,
            Route::default(),
            crate::output::VOLUME_REFERENCE, // A-043
        ))
    }

    /// Builds every path around an existing memory (`new` and `reconfigure`). Infallible once
    /// the settings are validated and the plan exists, so `reconfigure` can move the memory in
    /// without a window in which the pipeline has none.
    fn with_memory(
        settings: Settings,
        plan: &RatePlan,
        memory: Memory,
        route: Route,
        volume: f32,
    ) -> Self {
        let one_pass = OnePass::new(&settings, plan);
        let latency = latency_samples(plan, &settings, &one_pass);
        Self {
            settings,
            one_pass,
            latency,
            recorder: RecordPath::new(&settings, plan, false, true),
            player: PlaybackPath::new(&settings, plan, false),
            memory,
            transport: Transport::default(),
            route,
            volume,
            plan_l_m: paths::l_m(plan),
            encoder: settings.speech_codec_enabled.then(SpeechEncoder::new),
            decoder: settings.speech_codec_enabled.then(SpeechDecoder::new),
        }
    }

    /// Processes one block. `input` and `output` must have the same length, which may be
    /// any length including 0. Real-time safe.
    ///
    /// Only `output[..produced]` is written (see [`BlockInfo`]); the rest is left unchanged.
    ///
    /// The route and the volume position are read at entry (spec 009 FR-009: a change takes
    /// effect at the next call), and the output is at the configured tap: by default the
    /// selected route's output (spec 009 FR-010).
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if the lengths differ, [`Error::RecordingInProgress`] or
    /// [`Error::PlaybackInProgress`] while a path is active (spec 006 FR-009),
    /// [`Error::TapNotOnRoute`] when the tap is [`Tap::AfterSpeaker`] or
    /// [`Tap::AfterEarphone`] and the other route is selected (spec 009 FR-011). The pipeline
    /// state is not changed.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<BlockInfo, Error> {
        if input.len() != output.len() {
            return Err(Error::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        self.refuse_if_active()?;
        self.tap_on_route()?;
        Ok(self.run_block(Some(input), output, &mut []))
    }

    /// Processes one block in place. Real-time safe.
    ///
    /// The block's output is `buffer[..produced]` (see [`BlockInfo`]); samples past `produced`
    /// are unspecified. Output index never exceeds input index, so in-place processing is safe.
    /// While a recording or playback is in progress the call is refused: nothing is processed
    /// and `produced` is 0 (this signature has no error channel; [`process`](Self::process)
    /// and [`state`](Self::state) report the reason; spec 006 FR-009). So is a call with a
    /// named route tap of the other route (spec 009 FR-011): a host that uses
    /// [`Tap::AfterSpeaker`] or [`Tap::AfterEarphone`] checks [`route`](Self::route) first,
    /// because a refused in-place call is indistinguishable from a drop-mode pause.
    pub fn process_in_place(&mut self, buffer: &mut [f32]) -> BlockInfo {
        if self.refuse_if_active().is_err() || self.tap_on_route().is_err() {
            return self.refused_block();
        }
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
    /// [`Error::LengthMismatch`] if `input` and `output` lengths differ,
    /// [`Error::RecordingInProgress`] or [`Error::PlaybackInProgress`] while a path is active
    /// (spec 006 FR-009), [`Error::TapNotOnRoute`] for a named route tap of the other route
    /// (spec 009 FR-011). The pipeline state is not changed.
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
        self.refuse_if_active()?;
        self.tap_on_route()?;
        Ok(self.run_block(Some(input), output, events))
    }

    /// As [`process_in_place`](Self::process_in_place), and writes the block's VAS events to
    /// `events` as [`process_with_events`](Self::process_with_events) does. Real-time safe.
    /// Refused (nothing processed, `produced` 0) while a path is active or with a named route
    /// tap of the other route (spec 009 FR-011; see [`process_in_place`](Self::process_in_place)).
    pub fn process_in_place_with_events(
        &mut self,
        buffer: &mut [f32],
        events: &mut [VasEvent],
    ) -> BlockInfo {
        if self.refuse_if_active().is_err() || self.tap_on_route().is_err() {
            return self.refused_block();
        }
        self.run_block(None, buffer, events)
    }

    /// Spec 009 FR-011: a named route tap with the other route selected is refused at the
    /// processing call; the route output tap follows the route.
    fn tap_on_route(&self) -> Result<(), Error> {
        let tap = self.settings.tap;
        let wrong = matches!(
            (tap, self.route),
            (Tap::AfterSpeaker, Route::Earphone) | (Tap::AfterEarphone, Route::Speaker)
        );
        if wrong {
            Err(Error::TapNotOnRoute {
                tap,
                route: self.route,
            })
        } else {
            Ok(())
        }
    }

    /// Sets the volume position (spec 009 FR-003; A-043, A-044): the control's wiper fraction,
    /// 0 (silence) to 1 (maximum), at any time including during playback; takes effect at the
    /// next processing call or `play` block; survives [`reset`](Self::reset) and
    /// [`reconfigure`](Self::reconfigure). Since 0.11 a change is a **20 ms volume transition**
    /// (spec 011; A-045; [`output::VOLUME_TRANSITION_MS`](crate::output::VOLUME_TRANSITION_MS)):
    /// from the next block the knob moves from where it is to `position`, starting and ending
    /// at rest, the gain following the loaded law at the knob's position and never passing
    /// either end, the new position's gain reached exactly at the transition's last sample; a
    /// change during a transition starts a new 20 ms movement from the knob's current position,
    /// from rest, so the knob trails a stream of updates (about 19 ms at 60 Hz, 23 ms at 120 Hz,
    /// about 105 ms if the position changes every 64-sample block at 48 kHz): send a position
    /// when the control moves, at most once per UI frame, and do not fade by changing the
    /// volume every block. The first block that produces output after `new`, `reset`,
    /// `reconfigure` or [`start_playback`](Self::start_playback), and the first after a route
    /// change, take the position at once. The one-pass processing and the playback each keep
    /// their own knob position. With stage 11 bypassed the position has no effect. The routes'
    /// gains follow the circuit's loaded law
    /// ([`output::speaker_gain_db`](crate::output::speaker_gain_db),
    /// [`output::earphone_gain_db`](crate::output::earphone_gain_db)). Default
    /// [`output::VOLUME_REFERENCE`](crate::output::VOLUME_REFERENCE), where full-scale material
    /// just reaches the clip ceiling on the speaker route; positions above it clip. Real-time
    /// safe.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidVolume`] for a non-finite position or one outside 0 … 1; the position
    /// is unchanged.
    pub fn set_volume(&mut self, position: f32) -> Result<(), Error> {
        if !(position.is_finite() && (0.0..=1.0).contains(&position)) {
            return Err(Error::InvalidVolume { position });
        }
        self.volume = position;
        Ok(())
    }

    /// The volume position (spec 009 FR-003): the last valid setting, not the knob's position
    /// during a volume transition (spec 011 FR-011).
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// The report of a refused in-place call: nothing produced, the stream's paused state.
    fn refused_block(&self) -> BlockInfo {
        BlockInfo {
            produced: 0,
            events: 0,
            paused: self.one_pass.rec.paused(),
            frames: 0,
        }
    }

    /// An `events` length for [`process_with_events`](Self::process_with_events) that is always
    /// enough for a block of `frames` input samples with the current settings (spec 003 R-07).
    /// Splices are at least H + 2 device samples apart, and `frames` host samples hold at most
    /// ⌊frames·l/m⌋ + 1 device samples (+ 2 at the non-integer rates (8, 16, 44.1 and 88.2 kHz)); the "+ 2" also covers a
    /// splice on the block's first device sample (engineering target, 003 R-15). 0 at the
    /// "after AGC" tap, where the VAS never runs (spec 005 FR-001). Never allocates.
    pub fn max_events(&self, frames: usize) -> usize {
        self.one_pass.rec.max_events(frames)
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
        // Spec 009 FR-009: the controls are read once per call, at the block boundary.
        self.one_pass.play.set_controls(self.route, self.volume);
        let (mut produced, mut count, mut frames) = (0, 0, 0);
        for i in 0..out.len() {
            let x = match input {
                Some(input) => input[i],
                None => out[i],
            };
            if let Some(y) = self.one_pass.tick(x, &mut frames) {
                out[produced] = y;
                produced += 1;
            }
            if let Some(event) = self.one_pass.rec.take_event() {
                if let Some(slot) = events.get_mut(count) {
                    *slot = event;
                }
                count += 1;
            }
        }
        BlockInfo {
            produced,
            events: count,
            paused: self.one_pass.rec.paused(),
            frames,
        }
    }

    /// Fixed latency of the current configuration, in host-rate samples (FR-012).
    ///
    /// This is the rate conversion's delay plus the voice-band stages' group delay at 1 kHz,
    /// rounded to the nearest sample (research.md R-10), plus the speech codec's delay of 180
    /// device samples when it runs (spec 008 FR-004: 1440 host samples at 48 kHz, so 1923 in
    /// all). The limiter adds none, and at the "after AGC" tap the latency is 0 (spec 005
    /// FR-001).
    pub fn latency_samples(&self) -> u32 {
        self.latency
    }

    /// The memory's encoding (spec 008 FR-012): speech-codec frames with the codec on,
    /// device-rate samples with it bypassed. Real-time safe.
    pub fn encoding(&self) -> Encoding {
        self.memory.encoding()
    }

    /// The speech codec's constants (spec 008 FR-003): the same as
    /// [`speech_codec::info`](crate::speech_codec::info). Real-time safe.
    pub fn speech_codec_info(&self) -> codec::SpeechCodecInfo {
        codec::info()
    }

    /// Returns the pipeline to idle and to its freshly created path state, without allocating
    /// (FR-017; spec 006 FR-019): the limiter's capacitor is discharged, every filter, the VAS
    /// and the conversions reset, in the one-pass stream and in both split paths. A recording
    /// in progress is discarded without flush (its samples leave the store); a playback stops
    /// without drain. The memory's recordings and the route are kept.
    pub fn reset(&mut self) {
        self.one_pass.reset();
        self.recorder.reset();
        self.player.reset();
        self.memory.discard_in_progress();
        self.transport.reset();
    }

    /// Applies new settings and resets all path state, as if the pipeline had been created
    /// anew with `settings` (FR-008), **keeping the memory's recordings and the route** (spec
    /// 006 FR-019): with the same capacity the store is kept as it is; with a different one
    /// that still holds the recordings a new store is reserved and they are copied. The new
    /// configuration is built completely before it replaces the old one, so on error the
    /// pipeline is unchanged: same settings, latency, state and memory.
    ///
    /// May allocate; **not** real-time safe. Call it between streams, not from the audio
    /// callback.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`] or [`Error::PlaybackInProgress`] unless the engine is
    /// idle; [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not supported;
    /// [`Error::InvalidSetting`] if an AGC, VAS or memory setting is invalid (spec 002 FR-011,
    /// spec 003 FR-012, spec 006 FR-010); [`Error::StoredExceedsCapacity`] when the stored
    /// recordings do not fit the new configuration: their time or count exceeds the new
    /// capacity, or the speech-codec setting changes while recordings are present (one memory
    /// holds one encoding, spec 008 FR-012; erase first); [`Error::AllocationFailed`] when a new
    /// store cannot be reserved.
    pub fn reconfigure(&mut self, settings: Settings) -> Result<(), Error> {
        self.refuse_if_active()?;
        validate(&settings)?;
        // Spec 008 FR-012: a change of the codec setting with recordings present does not fit
        // (one memory holds one encoding); with an empty memory the store is re-reserved.
        let encoding = Encoding::for_settings(&settings);
        if !self.memory.fits(&settings.memory, encoding) {
            return Err(Error::StoredExceedsCapacity);
        }
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        let memory = if self.memory.same_capacity(&settings.memory, encoding) {
            // Kept by move (research R-03): no second reservation and no copy. Nothing below
            // can fail, so the pipeline is never left without its store.
            core::mem::replace(&mut self.memory, Memory::empty())
        } else {
            self.memory.copied_to(&settings.memory, encoding)?
        };
        *self = Self::with_memory(settings, &plan, memory, self.route, self.volume);
        Ok(())
    }

    /// The current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

#[cfg(feature = "__test-hooks")]
impl Pipeline {
    /// Test-only (hidden): a pipeline whose band-limit stages use `record` and `playback`
    /// instead of the committed coefficients. Used by the SC-007 mutation tests (spec 004
    /// tasks.md T024; spec 004 research R-06).
    ///
    /// # Errors
    ///
    /// As [`Pipeline::new`].
    #[doc(hidden)]
    pub fn __with_stage_sos(
        settings: Settings,
        record: [[f64; 5]; 5],
        playback: [[f64; 5]; 3],
    ) -> Result<Self, Error> {
        use crate::stages::voiceband::{PlaybackStage, RecordStage};
        let mut p = Self::new(settings)?;
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        p.one_pass
            .rec
            .replace_record_stage(RecordStage::with_sos(record), &plan);
        p.one_pass
            .play
            .replace_playback_stage(PlaybackStage::with_sos(playback), &plan);
        p.latency = latency_samples(&plan, &settings, &p.one_pass);
        Ok(p)
    }

    /// Test-only (hidden): a pipeline whose output chain is altered in both playback paths
    /// (the one-pass's and the player's) by `m`: the speaker template's numerator scaled, the
    /// knee's start moved, the drain's settle shifted, the speaker route's gain scaled. Used
    /// by the spec 009 SC-006 mutation tests (tasks.md T017); the latencies are unchanged.
    ///
    /// # Errors
    ///
    /// As [`Pipeline::new`].
    #[doc(hidden)]
    pub fn __with_output_mutation(
        settings: Settings,
        m: &crate::__test_hooks::OutputMutation,
    ) -> Result<Self, Error> {
        let mut p = Self::new(settings)?;
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        p.one_pass.play.mutate_chain(m, &plan);
        p.player.mutate_chain(m, &plan);
        Ok(p)
    }

    /// Test-only (hidden): a one-pass pipeline with the FR-009 `f32` narrowing of the post-VAS
    /// device-rate stream enabled (the shipped behaviour) or disabled (the pre-0.6 engine), so
    /// the harness can measure the narrowing's effect (spec 006 research R-02;
    /// contracts/rust-api.md › Test hooks).
    ///
    /// # Errors
    ///
    /// As [`Pipeline::new`].
    #[doc(hidden)]
    pub fn __with_device_narrowing(settings: Settings, enabled: bool) -> Result<Self, Error> {
        Self::with_device_narrowing(settings, enabled)
    }
}

#[cfg(any(test, feature = "__test-hooks"))]
impl Pipeline {
    /// The limiter's hold-capacitor voltage (spec 005 R-03): 0.0 when discharged.
    pub(crate) fn limiter_state(&self) -> f64 {
        self.one_pass.rec.limiter.state()
    }

    /// The actual `Vec` capacities of the store and the span table (spec 006 SC-007; plan
    /// review P6).
    pub(crate) fn memory_capacities(&self) -> (usize, usize) {
        self.memory.capacities()
    }

    /// A pipeline with the FR-009 narrowing `enabled` or not (spec 006 research R-02).
    pub(crate) fn with_device_narrowing(settings: Settings, enabled: bool) -> Result<Self, Error> {
        let mut p = Self::new(settings)?;
        p.one_pass.narrow = enabled;
        Ok(p)
    }

    /// The one-pass codec delay line's fill, or `None` with the codec bypassed (spec 008
    /// research R-06).
    pub(crate) fn codec_fifo_fill(&self) -> Option<usize> {
        self.one_pass.codec.as_ref().map(|c| c.len)
    }

    /// The stored frame bytes of recording `number` (spec 008 SC-008; an empty slice for a
    /// samples memory or a bad number).
    pub(crate) fn stored_frames(&self, number: u32) -> &[u8] {
        match (self.memory.encoding(), self.memory.span(number)) {
            (Encoding::Frames, Some(span)) => self.memory.frames(span),
            _ => &[],
        }
    }

    /// Flips one bit of a stored frame of recording `number` (byte `byte` of the recording's
    /// frame bytes, bit `bit`); returns whether it did.
    pub(crate) fn flip_stored_frame_bit(&mut self, number: u32, byte: usize, bit: u8) -> bool {
        let Some(span) = self.memory.span(number) else {
            return false;
        };
        match self.memory.frames_mut(span).get_mut(byte) {
            Some(b) => {
                *b ^= 1u8 << (bit & 7);
                true
            }
            None => false,
        }
    }
}

/// 001 R-10: round(D_down + D_up + (τ_rec + τ_play) · host / DEVICE_RATE_HZ), rounded once. A
/// bypassed stage contributes 0, and with the tap after the record stage τ_play is not
/// included. The limiter adds nothing; at the "after AGC" tap nothing after it runs, so the
/// latency is 0 (spec 005 FR-001). Spec 006 FR-016 reports the two paths' latencies rounded
/// separately; this single rounding is kept for the one-pass figure.
fn latency_samples(plan: &RatePlan, settings: &Settings, one_pass: &OnePass) -> u32 {
    if settings.tap == Tap::AfterAgc {
        return 0;
    }
    let stages =
        one_pass.rec.half.group_delay_1k_samples() + one_pass.play.half.group_delay_1k_samples();
    let total = 2.0 * f64::from(plan.delay_host_samples())
        + stages * f64::from(settings.host_rate_hz) / f64::from(DEVICE_RATE_HZ);
    // Spec 008 FR-004: the codec's delay is a whole number of host samples, added after the
    // single rounding of the rest.
    let codec_delay = if one_pass.rec.codec_runs() {
        paths::codec_delay_host_samples(settings.host_rate_hz)
    } else {
        0
    };
    ((total + 0.5) as u32).saturating_add(codec_delay)
}

#[cfg(test)]
mod validation_tests {
    extern crate std;
    use super::*;
    use crate::error::Setting;
    use std::vec::Vec;

    /// The spec 006 configuration: the defaults with the speech codec bypassed (spec 008 FR-021:
    /// the earlier specs' rules are checked with the codec off; the codec has its own tests).
    fn settings_006(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.output = crate::settings::OutputSettings::BYPASSED; // 009 FR-016: the earlier checks run with stages 11–13 bypassed
        s.tap = Tap::AfterPlayback; // 009 FR-011: 0.8's output
        s.speech_codec_enabled = false;
        s
    }

    /// 002 T014, restated by 005 T007 (spec 005 FR-001): with the tap after the AGC, the
    /// band-limit stages, the clamp and the VAS do not run, whatever their settings, and the
    /// output is the limiter's own at the host rate with latency 0: the stage settings make no
    /// difference, and with the limiter bypassed the output is the input.
    #[test]
    fn tap_after_agc_skips_both_band_limit_stages() {
        let x: Vec<f32> = (0..9600)
            .map(|n| if n % 97 == 0 { 1.5 } else { -0.01 })
            .collect();
        let run = |s: Settings| {
            let mut y = x.clone();
            let _ = Pipeline::new(s).unwrap().process_in_place(&mut y);
            y
        };
        for rate in crate::SUPPORTED_HOST_RATES {
            let mut stages_on = Settings::new(rate);
            stages_on.tap = Tap::AfterAgc;
            let mut stages_off = stages_on;
            stages_off.record_stage_enabled = false;
            stages_off.playback_stage_enabled = false;
            stages_off.adc_clamp_enabled = false;
            stages_off.vas.enabled = false;
            assert_eq!(run(stages_on), run(stages_off), "{rate} Hz");
            assert_eq!(Pipeline::new(stages_on).unwrap().latency_samples(), 0);
            let mut bypassed = stages_on;
            bypassed.agc.enabled = false;
            assert_eq!(
                run(bypassed),
                x,
                "{rate} Hz: limiter bypassed is the identity"
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
            let mut tap_vas = settings_006(rate); // the codec runs at the default tap only (008 FR-015)
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

/// 005 T007 (spec 005 FR-001, FR-002, FR-008, FR-014; research R-01; contracts/rust-api.md):
/// the limiter at the host rate, the "after AGC" tap, the clamp's place in the chain.
#[cfg(test)]
mod limiter_tests {
    extern crate std;
    use super::*;
    use crate::sanitize::{narrow_out, sanitize_in};
    use crate::stages::limiter::Limiter;
    use rr_dr60_detmath::{PI, TAU, exp, ln, sin};
    use std::vec::Vec;

    /// The spec 006 configuration: the defaults with the speech codec bypassed (spec 008 FR-021:
    /// the earlier specs' rules are checked with the codec off; the codec has its own tests).
    fn settings_006(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.output = crate::settings::OutputSettings::BYPASSED; // 009 FR-016: the earlier checks run with stages 11–13 bypassed
        s.tap = Tap::AfterPlayback; // 009 FR-011: 0.8's output
        s.speech_codec_enabled = false;
        s
    }

    /// A 1 kHz tone at `dbfs` for `secs`, starting at the host-grid peak phase (005 R-08).
    fn tone(dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / fs;
        let phase = (PI / 2.0) % w;
        (0..(secs * fs) as usize)
            .map(|n| (amp * sin(w * n as f64 + phase)) as f32)
            .collect()
    }

    /// The onset stimulus: −20 dBFS 0.1 s, then +11 dBFS 0.4 s (data-model.md › Attack step).
    fn onset(fs: f64) -> Vec<f32> {
        let mut x = tone(-20.0, 0.1, fs);
        let w = TAU * 1000.0 / fs;
        let phase = (PI / 2.0) % w;
        let amp = exp(11.0 * ln(10.0) / 20.0);
        let n0 = x.len();
        x.extend((0..(0.4 * fs) as usize).map(|n| (amp * sin(w * (n0 + n) as f64 + phase)) as f32));
        x
    }

    fn run(s: Settings, x: &[f32]) -> (Vec<f32>, BlockInfo) {
        let mut y = std::vec![0.0f32; x.len()];
        let info = Pipeline::new(s).unwrap().process(x, &mut y).unwrap();
        y.truncate(info.produced);
        (y, info)
    }

    fn peak(y: &[f32]) -> f32 {
        y.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    /// FR-001: at the "after AGC" tap the output is a standalone limiter's output, bit for bit,
    /// at the host rate: latency 0, no events, one sample out per sample in.
    #[test]
    fn tap_after_agc_is_the_limiter_output_at_the_host_rate() {
        for rate in [6000, 48_000] {
            let fs = f64::from(rate);
            let mut s = Settings::new(rate);
            s.tap = Tap::AfterAgc;
            let p = Pipeline::new(s).unwrap();
            assert_eq!(p.latency_samples(), 0, "{rate} Hz");
            for n in [0, 1, 4800, 1 << 20] {
                assert_eq!(p.max_events(n), 0, "{rate} Hz, {n} frames");
            }
            let x = onset(fs);
            let (y, info) = run(s, &x);
            assert_eq!(
                info,
                BlockInfo {
                    produced: x.len(),
                    events: 0,
                    paused: false,
                    frames: 0,
                },
                "{rate} Hz"
            );
            let mut limiter = Limiter::new(&s.agc, rate);
            let mut attenuated = 0;
            for (n, (&xi, &yi)) in x.iter().zip(&y).enumerate() {
                let want = narrow_out(limiter.process(sanitize_in(xi)));
                assert_eq!(yi.to_bits(), want.to_bits(), "{rate} Hz: sample {n}");
                if yi != xi {
                    attenuated += 1;
                }
            }
            assert!(attenuated > 0, "{rate} Hz: the limiter never acted");
            // With events, the same: no event is ever reported.
            let mut p = Pipeline::new(s).unwrap();
            let mut y2 = std::vec![0.0f32; x.len()];
            let mut ev = [VasEvent::default(); 4];
            let info = p.process_with_events(&x, &mut y2, &mut ev).unwrap();
            assert_eq!((info.produced, info.events), (x.len(), 0));
            assert_eq!(y2, y);
        }
    }

    /// SC-006 (FR-001): the limiter adds no latency at any tap; the "after AGC" tap is 0.
    #[test]
    fn latency_unchanged_with_limiter_on_or_off() {
        for rate in crate::SUPPORTED_HOST_RATES {
            for tap in [Tap::AfterRecord, Tap::AfterVas, Tap::AfterPlayback] {
                let mut on = Settings::new(rate);
                on.tap = tap;
                let mut off = on;
                off.agc.enabled = false;
                let (lat_on, lat_off) = (
                    Pipeline::new(on).unwrap().latency_samples(),
                    Pipeline::new(off).unwrap().latency_samples(),
                );
                assert_eq!(lat_on, lat_off, "{rate} Hz {tap:?}");
                assert!(lat_on > 0 || rate == DEVICE_RATE_HZ, "{rate} Hz {tap:?}");
            }
            let mut s = Settings::new(rate);
            s.tap = Tap::AfterAgc;
            assert_eq!(Pipeline::new(s).unwrap().latency_samples(), 0);
            s.agc.enabled = false;
            assert_eq!(Pipeline::new(s).unwrap().latency_samples(), 0);
        }
    }

    /// FR-008 (A-030): the clamp applies after the record stage, so with the limiter off a
    /// +11 dBFS tone through stage 4 peaks at exactly 1.0; with the clamp off it exceeds 1.0;
    /// the "after AGC" tap never clamps.
    #[test]
    fn clamp_applies_after_the_record_stage() {
        let x = tone(11.0, 0.5, 6000.0);
        let mut s = settings_006(6000); // the chain without the codec (008 FR-021)
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.playback_stage_enabled = false;
        let (y, _) = run(s, &x);
        assert_eq!(peak(&y), 1.0, "clamped peak");
        assert!(
            y.iter().filter(|v| v.abs() == 1.0).count() > 100,
            "no flat top"
        );
        let mut off = s;
        off.adc_clamp_enabled = false;
        let (y, _) = run(off, &x);
        assert!(peak(&y) > 1.0, "unclamped peak {}", peak(&y));
        let mut tap = s;
        tap.tap = Tap::AfterAgc;
        let (y, _) = run(tap, &x);
        assert_eq!(y, x, "the after-AGC tap must not clamp");
    }

    /// FR-008: the spec 001 `bypass_all` configuration (everything off, the clamp included)
    /// is the identity at 6 kHz beyond full scale; the clamp alone saturates.
    #[test]
    fn bypass_all_disables_the_clamp() {
        let x: Vec<f32> = [1.5f32, -2.0, 0.25, 1.0, -1.0, 0.999]
            .iter()
            .copied()
            .cycle()
            .take(600)
            .collect();
        let mut s = settings_006(6000); // the chain without the codec (008 FR-021)
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        let (y, info) = run(s, &x);
        assert_eq!(info.produced, x.len());
        assert_eq!(y, x);
        s.adc_clamp_enabled = true;
        let (y, _) = run(s, &x);
        let want: Vec<f32> = x.iter().map(|v| v.clamp(-1.0, 1.0)).collect();
        assert_eq!(y, want);
    }

    /// data-model.md › Pipeline order: record → clamp → VAS → playback. The clamp sees stage
    /// 4's output (its peak is exactly 1.0 only if the clamp comes after the band-pass), and the
    /// "after record" and "after VAS" taps both see clamped samples, so the clamp precedes the
    /// VAS and stage 10 (whose order spec 003 established).
    #[test]
    fn device_chain_order_is_record_clamp_vas_playback() {
        let x = tone(11.0, 0.5, 6000.0);
        let mut s = settings_006(6000); // the chain without the codec (008 FR-021)
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.tap = Tap::AfterRecord;
        let (y, _) = run(s, &x);
        assert_eq!(peak(&y), 1.0);
        s.record_stage_enabled = false;
        for tap in [Tap::AfterRecord, Tap::AfterVas, Tap::AfterPlayback] {
            s.tap = tap;
            s.playback_stage_enabled = false;
            let (y, _) = run(s, &x);
            let want: Vec<f32> = x.iter().map(|v| v.clamp(-1.0, 1.0)).collect();
            assert_eq!(y, want, "{tap:?}");
        }
    }

    /// FR-014: `reset` and `reconfigure` discharge the limiter.
    #[test]
    fn reset_discharges_the_limiter() {
        let s = Settings::new(48_000);
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.limiter_state(), 0.0);
        let x = onset(48_000.0);
        let _ = p.process_in_place(&mut x.clone());
        assert!(p.limiter_state() > 0.9, "state {}", p.limiter_state());
        p.reset();
        assert_eq!(p.limiter_state(), 0.0);
        let _ = p.process_in_place(&mut x.clone());
        assert!(p.limiter_state() > 0.9);
        p.reconfigure(s).unwrap();
        assert_eq!(p.limiter_state(), 0.0);
    }
}

#[cfg(test)]
mod vas_event_tests {
    extern crate std;
    use super::*;
    use rr_dr60_detmath::{TAU, exp, ln, sin};
    use std::vec::Vec;

    /// The VAS-isolated configuration (spec 003 definitions): the limiter, stages 4 and 10 and
    /// the clamp off.
    fn vas_only(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.tap = Tap::AfterVas;
        s
    }

    /// 1 kHz at −30 dBFS (threshold + 10 dB; A-022 revised by spec 005) with a half-sample
    /// phase offset at the 6 kHz device rate (spec 004 research R-08), so every device sample
    /// is a sound sample and burst lengths are exact (see `stages::vas` tests).
    fn loud(n: usize, fs: f64) -> Vec<f32> {
        let amp = exp(-30.0 * ln(10.0) / 20.0);
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

    /// 003 T018 (FR-004, FR-005; research R-06), restated at 6 kHz by 004 FR-012: at the
    /// identity rate, burst 1 s, gap 5 s, burst 1 s keeps 6000 + 6000 (hang) + 6000 − 120 (onset)
    /// samples, with one splice at output 12000 that removed 30000 − 6000 + 120 input samples.
    #[test]
    fn splice_at_6k_is_exact() {
        let x = burst_gap_burst(6000.0);
        let mut p = Pipeline::new(vas_only(6000)).unwrap();
        let (y, ev, info) = run_all(&mut p, &x);
        assert_eq!(y.len(), 6000 + 6000 + 6000 - 120);
        assert_eq!(
            ev,
            [VasEvent {
                output_position: 12_000,
                input_length: 24_120
            }]
        );
        assert_eq!(info.events, 1);
        assert!(!info.paused);
        assert_eq!(ev[0].input_length as usize + y.len(), x.len());
        // The kept samples are the input, bit for bit: the first burst and the hang time, then
        // the second burst minus the onset (6 kHz is the device rate; nothing else is in the path).
        assert_eq!(&y[..12_000], &x[..12_000]);
        assert_eq!(&y[12_000..], &x[12_000 + 24_120..]);
    }

    /// 003 T018: at 48 kHz (m/l = 8) the removed lengths and the output add up to the input
    /// exactly, the removed length is a whole number of device samples and within one device
    /// sample of 8 × the 6 kHz value (the decimator's edge smear can move the pause by one device
    /// sample; 003 research R-11 › Measured). The position also carries the decimator's delay,
    /// so only its alignment is checked.
    #[test]
    fn splice_at_48k_adds_up() {
        let x = burst_gap_burst(48_000.0);
        let mut p = Pipeline::new(vas_only(48_000)).unwrap();
        let (y, ev, _) = run_all(&mut p, &x);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].input_length % 8, 0);
        let removed: u64 = ev.iter().map(|e| e.input_length).sum();
        assert_eq!(removed as usize + y.len(), x.len());
        assert_eq!(ev[0].output_position % 8, 0);
        let device = 8; // ± one device sample, engineering target (003 FR-017)
        let length = i64::try_from(ev[0].input_length).unwrap();
        assert!(
            (length - 8 * 24_120).abs() <= device,
            "{}",
            ev[0].input_length
        );
    }

    /// 003 T018: a too-short event slice still counts the event, and writes nothing.
    #[test]
    fn short_event_slice_counts_but_does_not_write() {
        let x = burst_gap_burst(6000.0);
        let mut p = Pipeline::new(vas_only(6000)).unwrap();
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
        let mut p = Pipeline::new(vas_only(6000)).unwrap();
        let mut y = std::vec![0.0f32; 20_000];
        let info = p.process(&loud(20_000, 6000.0), &mut y).unwrap();
        assert!(!info.paused);
        let info = p.process(&[0.0; 10_000], &mut y[..10_000]).unwrap();
        assert!(info.paused);
        assert_eq!(info.produced, 6000); // one hang time (A-023) at 6 kHz
        p.reset();
        let info = p.process(&[0.0; 10], &mut y[..10]).unwrap();
        assert!(!info.paused && info.produced == 10);
    }

    /// 003 T052 (FR-005; research R-07): the densest possible splice train. With `onset_ms = 0`
    /// a single sound sample resumes recording, and a gap of exactly H + 1 silent device samples
    /// pauses it again, so splices are H + 2 device samples apart: the spacing `max_events`
    /// assumes. At the 6 kHz identity rate the bursts are one device sample; at the other rates
    /// one millisecond (6 device samples), so they survive the decimator. Every block's event
    /// count stays within
    /// `max_events(n)`, for block sizes including 1, and the total is one splice per burst.
    #[test]
    fn max_events_holds_for_the_densest_splice_train() {
        for rate in crate::SUPPORTED_HOST_RATES {
            let fs = f64::from(rate);
            let mut s = vas_only(rate);
            s.vas.hang_ms = 50.0; // H = 300 device samples at 6 kHz
            s.vas.onset_ms = 0.0;
            let mut p = Pipeline::new(s).unwrap();
            let h = p.one_pass.rec.half.vas.hang_samples() as usize;
            // Device samples → host samples, rounded up (m/l host samples per device sample).
            let (l, m) = (
                u128::from(p.one_pass.rec.counters.l),
                u128::from(p.one_pass.rec.counters.m),
            );
            let host = |n: usize| (n as u128 * m).div_ceil(l) as usize;
            let identity = l == m;
            let burst = if identity { 1 } else { host(6) }; // 1 device sample, or 1 ms
            // H + 1 device samples (one more than the hang, FR-008); H + 2 where m/l is not an
            // integer, so rounding can't shorten the gap.
            let gap = host(if m % l == 0 { h + 1 } else { h + 2 });
            let bursts = 300;
            let cycle = [std::vec![0.0f32; gap], loud(burst, fs)].concat();
            // The last burst must clear the conversion before the input ends (R-11 › Measurement
            // trap): one pipeline latency plus one device sample of trailing silence.
            let tail = p.latency_samples() as usize + host(1);
            let x: Vec<f32> = cycle
                .iter()
                .copied()
                .cycle()
                .take(bursts * cycle.len())
                .chain(core::iter::repeat_n(0.0, tail))
                .collect();
            let (mut pos, mut total) = (0, 0);
            let sizes = [1usize, 2, 7, 64, 1000, 4 * cycle.len()];
            let mut y = std::vec![0.0f32; 4 * cycle.len()];
            let mut ev = std::vec![VasEvent::default(); 64];
            let mut k = 0;
            while pos < x.len() {
                let n = sizes[k % sizes.len()].min(x.len() - pos);
                k += 1;
                let cap = p.max_events(n);
                assert!(cap <= ev.len(), "{rate} Hz: cap {cap}");
                let info = p
                    .process_with_events(&x[pos..pos + n], &mut y[..n], &mut ev[..cap])
                    .unwrap();
                assert!(
                    info.events <= cap,
                    "{rate} Hz: {} > {cap} for n = {n}",
                    info.events
                );
                total += info.events;
                pos += n;
            }
            // Every burst follows a gap that paused recording, so every burst is a resume.
            assert_eq!(total, bursts, "{rate} Hz");
        }
    }

    /// 003 T052 (FR-005; research R-07): a block holding two splices, with event slices of
    /// length 0, 1 and 2: `events` counts 2 every time, the first `len` are written in order, and
    /// the rest are counted but not written.
    #[test]
    fn short_event_slices_count_every_splice() {
        let fs = 6000.0;
        let s = fs as usize;
        // burst, gap, burst, gap, burst: two splices (the 6 kHz T018 geometry, twice).
        let x = [
            loud(s, fs),
            std::vec![0.0; 5 * s],
            loud(s, fs),
            std::vec![0.0; 5 * s],
            loud(s, fs),
        ]
        .concat();
        let mut p = Pipeline::new(vas_only(6000)).unwrap();
        let (_, want, _) = run_all(&mut p, &x);
        assert_eq!(want.len(), 2);
        for len in 0..=2 {
            p.reset();
            let mut y = std::vec![0.0f32; x.len()];
            let sentinel = VasEvent {
                output_position: u64::MAX,
                input_length: u64::MAX,
            };
            let mut ev = std::vec![sentinel; len];
            let info = p.process_with_events(&x, &mut y, &mut ev).unwrap();
            assert_eq!(info.events, 2, "len {len}");
            assert_eq!(ev, want[..len], "len {len}");
        }
    }
}

/// 006 T004 (spec 006 FR-009; research R-02): the one `f32` narrowing of the device-rate stream
/// after the VAS, the point where the recording is taken. Observed through the test-only
/// un-narrowed path (`with_device_narrowing(settings, false)`).
#[cfg(test)]
mod narrowing_tests {
    extern crate std;
    use super::*;
    use crate::sanitize::{narrow_out, sanitize_in};
    use crate::settings::VasMode;
    use crate::stages::limiter::Limiter;
    use crate::stages::voiceband::PlaybackStage;
    use rr_dr60_detmath::{TAU, exp, ln, sin};
    use std::vec::Vec;

    /// The spec 006 configuration: the defaults with the speech codec bypassed (spec 008 FR-021:
    /// the earlier specs' rules are checked with the codec off; the codec has its own tests).
    fn settings_006(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.output = crate::settings::OutputSettings::BYPASSED; // 009 FR-016: the earlier checks run with stages 11–13 bypassed
        s.tap = Tap::AfterPlayback; // 009 FR-011: 0.8's output
        s.speech_codec_enabled = false;
        s
    }

    /// A 1 kHz tone at `dbfs` for `secs` at the 6 kHz device rate, phase 0 (no sample-peak
    /// detector is read here, so no aligned phase is needed).
    fn tone(dbfs: f64, secs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / 6000.0;
        (0..(secs * 6000.0) as usize)
            .map(|n| (amp * sin(w * n as f64)) as f32)
            .collect()
    }

    fn run(p: &mut Pipeline, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, BlockInfo) {
        let mut y = std::vec![0.0f32; x.len()];
        let mut ev = std::vec![VasEvent::default(); p.max_events(x.len())];
        let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
        y.truncate(info.produced);
        ev.truncate(info.events);
        (y, ev, info)
    }

    /// FR-009: with every stage on at 6 kHz, the un-narrowed and the narrowed one-pass differ
    /// (a −20 dBFS tone is not `f32`-exact after stage 4), and the narrowed output equals a
    /// manual re-run: the stream taken at the "after VAS" tap (which narrows the kept sample
    /// exactly as `narrow_device` does) fed through a fresh stage 10 and narrowed again. So
    /// the rounding sits after the VAS and before stage 10.
    #[test]
    fn narrowing_point_is_after_the_vas() {
        let s = settings_006(6000); // the limiter, stage 4, the clamp, the VAS and stage 10 on; the codec off (008 FR-021)
        let x = tone(-20.0, 0.5);
        let (wide, _, _) = run(&mut Pipeline::with_device_narrowing(s, false).unwrap(), &x);
        let (narrow, _, _) = run(&mut Pipeline::with_device_narrowing(s, true).unwrap(), &x);
        assert_eq!(wide.len(), narrow.len());
        let differing = wide
            .iter()
            .zip(&narrow)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert!(differing > 0, "the narrowing changed nothing at 6 kHz");
        let (default, _, _) = run(&mut Pipeline::new(s).unwrap(), &x);
        assert_eq!(default, narrow, "the default pipeline narrows");

        // The manual re-run: the post-VAS stream, narrowed, then stage 10.
        let mut tap = s;
        tap.tap = Tap::AfterVas;
        let (post_vas, _, _) = run(
            &mut Pipeline::with_device_narrowing(tap, false).unwrap(),
            &x,
        );
        assert_eq!(post_vas.len(), narrow.len());
        let mut stage10 = PlaybackStage::playback();
        for (n, (&v, &want)) in post_vas.iter().zip(&narrow).enumerate() {
            let got = narrow_out(stage10.process(f64::from(v)));
            assert_eq!(got.to_bits(), want.to_bits(), "sample {n}");
        }
    }

    /// FR-009 (research R-02): with every stage bypassed at the 6 kHz identity rate there is no
    /// arithmetic before the narrowing, so the output is the input bit for bit, beyond ±1.0 too.
    #[test]
    fn bypass_all_at_6k_is_bit_identical_to_input() {
        let x: Vec<f32> = [1.5f32, -2.0, 0.25, 1.0, -1.0, 0.999, 3.0e-39, 1.0e-7]
            .iter()
            .copied()
            .cycle()
            .take(600)
            .collect();
        let mut s = settings_006(6000);
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        for narrow in [false, true] {
            let (y, _, info) = run(&mut Pipeline::with_device_narrowing(s, narrow).unwrap(), &x);
            assert_eq!(info.produced, x.len());
            let want: Vec<f32> = x.iter().map(|&v| narrow_out(sanitize_in(v))).collect();
            assert_eq!(
                y.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                want.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "narrowing {narrow}"
            );
        }
    }

    /// FR-009 (spec 005 FR-001): the "after AGC" tap never reaches the device chain, so the
    /// narrowing cannot touch it: the output is the limiter's `f32` output as today, with the
    /// narrowing on or off, at the identity and at a converting rate.
    #[test]
    fn after_agc_tap_never_narrows() {
        for rate in [6000, 48_000] {
            let fs = f64::from(rate);
            let w = TAU * 1000.0 / fs;
            let amp = exp(11.0 * ln(10.0) / 20.0);
            let x: Vec<f32> = (0..(0.5 * fs) as usize)
                .map(|n| (amp * sin(w * n as f64)) as f32)
                .collect();
            let mut s = Settings::new(rate);
            s.tap = Tap::AfterAgc;
            let (wide, _, _) = run(&mut Pipeline::with_device_narrowing(s, false).unwrap(), &x);
            let (narrow, _, _) = run(&mut Pipeline::with_device_narrowing(s, true).unwrap(), &x);
            assert_eq!(wide, narrow, "{rate} Hz");
            let mut limiter = Limiter::new(&s.agc, rate);
            for (n, (&xi, &yi)) in x.iter().zip(&narrow).enumerate() {
                let want = narrow_out(limiter.process(sanitize_in(xi)));
                assert_eq!(yi.to_bits(), want.to_bits(), "{rate} Hz: sample {n}");
            }
        }
    }

    /// FR-009 (spec 003 research R-09): in mute mode the muted samples are +0.0 exactly after
    /// the narrowing (the recording stores them as such), and the region's events are the same
    /// with the narrowing on or off (the VAS decides before the narrowing).
    #[test]
    fn mute_mode_muted_samples_narrow_to_positive_zero() {
        let mut s = settings_006(6000);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.vas.mode = VasMode::Mute;
        s.tap = Tap::AfterVas;
        let x = [tone(-30.0, 1.0), std::vec![0.0; 30_000], tone(-30.0, 1.0)].concat();
        let (wide, ev_wide, _) = run(&mut Pipeline::with_device_narrowing(s, false).unwrap(), &x);
        let (narrow, ev, info) = run(&mut Pipeline::with_device_narrowing(s, true).unwrap(), &x);
        assert_eq!(narrow.len(), x.len(), "mute mode keeps the length");
        assert_eq!(ev, ev_wide);
        assert_eq!(ev.len(), 1, "{ev:?}");
        assert!(!info.paused);
        let (start, len) = (ev[0].output_position as usize, ev[0].input_length as usize);
        assert_eq!(start, 6000 + 6000, "the region starts after the hang time");
        assert!(len > 20_000, "region length {len}");
        for (n, v) in narrow[start..start + len].iter().enumerate() {
            assert_eq!(
                v.to_bits(),
                0.0f32.to_bits(),
                "muted sample {n} is not +0.0"
            );
        }
        // Outside the region the kept samples are the input (no arithmetic at 6 kHz).
        assert_eq!(&narrow[..start], &x[..start]);
        assert_eq!(wide, narrow, "no arithmetic before the narrowing here");
    }
}

#[cfg(all(test, feature = "op-count"))]
mod op_count_tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    impl Pipeline {
        fn ops(&self) -> u64 {
            let rec = &self.one_pass.rec;
            let play = &self.one_pass.play;
            rec.down.as_ref().map_or(0, |d| d.ops)
                + play.up.as_ref().map_or(0, |u| u.ops)
                + rec.limiter.ops
                + rec.limiter.transcendentals
                + rec.half.vas.ops
                + rec.half.record.ops()
                + play.half.playback.ops()
        }

        fn taps(&self) -> (u64, u64) {
            (
                self.one_pass
                    .rec
                    .down
                    .as_ref()
                    .map_or(0, |d| d.taps() as u64),
                self.one_pass
                    .play
                    .up
                    .as_ref()
                    .map_or(0, |u| u.taps() as u64),
            )
        }
    }

    /// FR-015 bounded work: no host sample costs more than one limiter step (spec 005 R-12: at
    /// most 1 `exp` + 1 `ln` + 30 basic operations), one full decimator branch, one
    /// interpolator branch, both band-limit stages (5 + 3 sections × 5 multiply-adds, spec 004
    /// research R-06) and one VAS decision (spec 003), and the total is independent of how the
    /// input is split into blocks. The default settings have the limiter and the VAS on, and
    /// the input has a loud click train (over the knee, so the limiter charges) with a long gap,
    /// so the VAS both keeps and drops samples.
    #[test]
    fn per_sample_work_is_bounded_and_block_independent() {
        for rate in crate::SUPPORTED_HOST_RATES {
            // A quarter second of clicks, then 1.5 s of near-silence (longer than the 1 s hang
            // time, so the VAS pauses), then clicks again (it resumes).
            let r = rate as usize;
            let x: Vec<f32> = (0..r / 4 + 3 * r / 2 + r / 4)
                .map(|n| {
                    let quiet = n >= r / 4 && n < r / 4 + 3 * r / 2;
                    if !quiet && n % 97 == 0 { 1.5 } else { -0.0001 }
                })
                .collect();
            let mut one = Pipeline::new(Settings::new(rate)).unwrap();
            let (taps_down, taps_up) = one.taps();
            // + the limiter's per-host-sample work (005 R-12: 30 basic + 2 transcendental) +
            // the VAS's (003 T023: one compare and a few counter updates).
            let bound = taps_down + taps_up + 40 + 32 + 4;
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

/// 008 T015 (spec 008 FR-004, FR-008, FR-009, FR-015; research R-06): the one-pass codec's
/// delay line, the frame counts per block, the decoder tap and reset.
#[cfg(test)]
mod codec_tests {
    extern crate std;
    use super::*;
    use rr_dr60_detmath::{TAU, exp, ln, sin};
    use std::vec::Vec;

    fn tone(dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / fs;
        (0..(secs * fs) as usize)
            .map(|n| (amp * sin(w * n as f64)) as f32)
            .collect()
    }

    /// The codec alone at the decoder tap (the harness's `codec_only`).
    fn codec_only(rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.vas.enabled = false;
        s.speech_codec_enabled = true;
        s.tap = Tap::AfterDecoder;
        s
    }

    fn run(s: Settings, x: &[f32], block: usize) -> (Vec<f32>, Vec<usize>) {
        let mut p = Pipeline::new(s).unwrap();
        let mut out = Vec::new();
        let mut frames = Vec::new();
        for chunk in x.chunks(block.max(1)) {
            let mut y = std::vec![0.0f32; chunk.len()];
            let info = p.process(chunk, &mut y).unwrap();
            assert_eq!(
                info.produced,
                chunk.len(),
                "one in, one out without the VAS"
            );
            out.extend_from_slice(&y);
            frames.push(info.frames);
        }
        (out, frames)
    }

    /// R-06: at 6 kHz the first 180 outputs of the codec-only one-pass are exactly 0.0 (the
    /// delay line filling), the 181st is the decoded first sample, and the line never holds
    /// more than 121 samples.
    #[test]
    fn one_pass_fifo_first_180_outputs_are_zero() {
        let mut x = tone(-12.0, 0.1, 6000.0);
        x[0] = 0.9; // an impulse on top
        let mut p = Pipeline::new(codec_only(6000)).unwrap();
        assert_eq!(p.codec_fifo_fill(), Some(0));
        let mut y = std::vec![0.0f32; x.len()];
        let mut worst = 0;
        for (i, &v) in x.iter().enumerate() {
            let info = p.process(&[v], &mut y[i..=i]).unwrap();
            assert_eq!(info.produced, 1);
            worst = worst.max(p.codec_fifo_fill().unwrap());
        }
        assert!(y[..180].iter().all(|&v| v == 0.0), "the first 180 outputs");
        assert!(
            y[180..].iter().any(|&v| v != 0.0),
            "then the decoded stream"
        );
        assert!(worst <= 121, "the line held {worst}");
        assert_eq!(
            p.latency_samples(),
            180,
            "the delay alone: nothing else runs at 6 kHz"
        );
        // With the codec bypassed there is no line.
        let mut off = codec_only(6000);
        off.speech_codec_enabled = false;
        assert_eq!(Pipeline::new(off).unwrap().codec_fifo_fill(), None);
    }

    /// FR-008: `BlockInfo.frames` counts the frames completed in the block (pushes 179, 299,
    /// 419 … at 6 kHz), 0 with the codec bypassed and at a tap before it, and the count does not
    /// depend on the block size.
    #[test]
    fn one_pass_frames_per_block() {
        let x = tone(-20.0, 0.2, 6000.0); // 1200 samples: frames at 179, 299, …, 1139 → 9 frames
        let (_, frames) = run(codec_only(6000), &x, 100);
        assert_eq!(frames, [0, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1]);
        let (_, frames) = run(codec_only(6000), &x, 1);
        assert_eq!(frames.iter().sum::<usize>(), 9);
        let (_, frames) = run(codec_only(6000), &x, 1200);
        assert_eq!(frames, [9]);
        let mut off = codec_only(6000);
        off.speech_codec_enabled = false;
        assert_eq!(run(off, &x, 100).1.iter().sum::<usize>(), 0);
        let mut before = codec_only(6000);
        before.tap = Tap::AfterVas;
        let (y, frames) = run(before, &x, 100);
        assert_eq!(frames.iter().sum::<usize>(), 0);
        assert_eq!(
            y, x,
            "the after-VAS tap is before the codec: the identity here"
        );
        let p = Pipeline::new(before).unwrap();
        assert_eq!(p.latency_samples(), 0);
        assert_eq!(p.codec_fifo_fill(), None);
    }

    /// FR-015: the decoder tap equals the default tap with stage 10 bypassed, bit for bit, and
    /// the codec's output is the same whatever the block partition.
    #[test]
    fn decoder_tap_equals_playback_tap_without_stage_10() {
        for rate in [6000u32, 48_000] {
            let x = tone(-20.0, 0.3, f64::from(rate));
            let dec = codec_only(rate);
            let mut play = dec;
            play.tap = Tap::AfterPlayback;
            let (a, _) = run(dec, &x, 997);
            let (b, _) = run(play, &x, 64);
            assert_eq!(a, b, "{rate} Hz");
            assert!(a.iter().any(|&v| v != 0.0));
            let mut on = play;
            on.playback_stage_enabled = true;
            let (c, _) = run(on, &x, 1000);
            assert_ne!(c, a, "{rate} Hz: stage 10 changes the output");
        }
    }

    /// FR-018: `reset` resets the one-pass codec pair: a burst, a reset and silence give the
    /// same output as a fresh pipeline; without the reset they differ.
    #[test]
    fn reset_resets_the_one_pass_codec() {
        let burst = tone(-12.0, 0.2, 6000.0);
        let silence = std::vec![0.0f32; 1200];
        let s = codec_only(6000);
        let mut p = Pipeline::new(s).unwrap();
        let mut y = burst.clone();
        let _ = p.process_in_place(&mut y);
        p.reset();
        let mut after = silence.clone();
        let _ = p.process_in_place(&mut after);
        let (fresh, _) = run(s, &silence, 1200);
        assert_eq!(after, fresh);
        let mut q = Pipeline::new(s).unwrap();
        let mut y = burst;
        let _ = q.process_in_place(&mut y);
        let mut without = silence;
        let _ = q.process_in_place(&mut without);
        assert_ne!(without, fresh);
    }
}

/// 009 T009 (spec 009 FR-003, FR-008 – FR-011; contracts/rust-api.md; research R-01, R-05,
/// R-06, R-08): the controls, the route-tap refusal, the drain, the latencies and the
/// equivalence precondition.
#[cfg(test)]
mod output_chain_tests {
    extern crate std;
    use super::*;
    use crate::output::{self, VOLUME_REFERENCE};
    use crate::settings::OutputSettings;
    use std::vec;
    use std::vec::Vec;

    fn tone(rate: u32, n: usize) -> Vec<f32> {
        let w = core::f64::consts::TAU * 1000.0 / f64::from(rate);
        (0..n)
            .map(|i| (0.25 * rr_dr60_detmath::sin(w * i as f64)) as f32)
            .collect()
    }

    #[test]
    fn set_volume_refuses_and_keeps_position() {
        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        assert_eq!(p.volume(), VOLUME_REFERENCE); // A-043
        for bad in [1.5, -0.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let err = p.set_volume(bad).unwrap_err();
            assert!(
                matches!(err, Error::InvalidVolume { position } if position.to_bits() == bad.to_bits())
            );
            assert_eq!(p.volume(), VOLUME_REFERENCE, "{bad}");
        }
        assert_eq!(p.set_volume(0.0), Ok(()));
        assert_eq!(p.volume(), 0.0);
        assert_eq!(p.set_volume(1.0), Ok(()));
        assert_eq!(p.volume(), 1.0);
    }

    #[test]
    fn volume_and_route_survive_reset_and_reconfigure() {
        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        p.set_volume(0.42).unwrap();
        p.set_route(Route::Earphone);
        p.reset();
        assert_eq!((p.volume(), p.route()), (0.42, Route::Earphone));
        let mut s = Settings::new(44_100);
        s.output.speaker_enabled = false;
        p.reconfigure(s).unwrap();
        assert_eq!((p.volume(), p.route()), (0.42, Route::Earphone));
    }

    #[test]
    fn route_tap_refused_on_the_other_route() {
        let x = tone(48_000, 256);
        let mut y = vec![0.0f32; 256];
        for (tap, wrong, right) in [
            (Tap::AfterSpeaker, Route::Earphone, Route::Speaker),
            (Tap::AfterEarphone, Route::Speaker, Route::Earphone),
        ] {
            let mut s = Settings::new(48_000);
            s.tap = tap;
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(wrong);
            assert_eq!(
                p.process(&x, &mut y),
                Err(Error::TapNotOnRoute { tap, route: wrong })
            );
            assert_eq!(
                p.process_with_events(&x, &mut y, &mut []),
                Err(Error::TapNotOnRoute { tap, route: wrong })
            );
            let mut buf = x.clone();
            assert_eq!(p.process_in_place(&mut buf).produced, 0);
            assert_eq!(
                p.process_in_place_with_events(&mut buf, &mut []).produced,
                0
            );
            assert_eq!(buf, x, "nothing processed");
            p.set_route(right);
            assert!(p.process(&x, &mut y).is_ok());
        }
        for tap in [Tap::RouteOutput, Tap::AfterVolumeAmp, Tap::AfterPlayback] {
            let mut s = Settings::new(48_000);
            s.tap = tap;
            let mut p = Pipeline::new(s).unwrap();
            for route in [Route::Speaker, Route::Earphone] {
                p.set_route(route);
                assert!(p.process(&x, &mut y).is_ok(), "{tap:?} {route:?}");
            }
        }
    }

    /// FR-009: a `set_volume` between two calls changes the second call's output and none of
    /// the first's (the one-pass at 6 kHz, the chain alone, so the output is the input's
    /// length and alignment).
    #[test]
    fn controls_land_at_the_next_call() {
        let mut s = Settings::new(6000);
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.speech_codec_enabled = false;
        s.output.speaker_enabled = false;
        let x = tone(6000, 128);
        let run = |volume_second: f32| {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(Route::Earphone);
            p.set_volume(1.0).unwrap();
            let mut y = vec![0.0f32; 128];
            let _ = p.process(&x[..64], &mut y[..64]).unwrap();
            p.set_volume(volume_second).unwrap();
            let _ = p.process(&x[64..], &mut y[64..]).unwrap();
            y
        };
        let (same, changed) = (run(1.0), run(0.5));
        assert_eq!(same[..64], changed[..64], "the first call is unchanged");
        assert_ne!(
            same[64], changed[64],
            "the first sample of the next call carries the new gain"
        );
    }

    /// Research R-08: `drain_samples` grows by `N_OUT` when any output stage runs, and is spec
    /// 006's figure with the chain bypassed; the reported latencies are unchanged (FR-008).
    #[test]
    fn drain_grows_by_n_out_and_latencies_are_unchanged() {
        for (rate, n_out, one_pass_008, play_006) in
            [(6000, 346, 181, 0), (48_000, 2764, 1923, 241)]
        {
            let on = Pipeline::new(Settings::new(rate)).unwrap();
            let mut s = Settings::new(rate);
            s.output = OutputSettings::BYPASSED;
            let off = Pipeline::new(s).unwrap();
            assert_eq!(on.drain_samples(), off.drain_samples() + n_out, "{rate}");
            assert_eq!(output::settle_samples(rate), Some(n_out));
            for only in 0..3 {
                let mut s = Settings::new(rate);
                s.output = OutputSettings {
                    volume_amp_enabled: only == 0,
                    speaker_enabled: only == 1,
                    earphone_enabled: only == 2,
                    clamp_enabled: false,
                };
                let p = Pipeline::new(s).unwrap();
                assert_eq!(
                    p.drain_samples(),
                    off.drain_samples() + n_out,
                    "{rate} stage {only}"
                );
            }
            assert_eq!(on.latency_samples(), one_pass_008, "{rate}");
            assert_eq!(on.latency_samples(), off.latency_samples());
            assert_eq!(on.playback_latency_samples(), play_006);
            assert_eq!(on.record_latency_samples(), off.record_latency_samples());
        }
    }

    /// FR-010 (coach C8): the one-pass's first 180 outputs of an impulse stream are exactly 0.0
    /// with the chain on at 6 kHz, so the chain's state at the equivalence skip point is rest.
    #[test]
    fn one_pass_prefix_is_exact_zero() {
        let mut x = vec![0.0f32; 400];
        x[0] = 0.5;
        for route in [Route::Speaker, Route::Earphone] {
            let mut p = Pipeline::new(Settings::new(6000)).unwrap();
            p.set_route(route);
            p.set_volume(1.0).unwrap();
            let mut y = vec![0.0f32; 400];
            let n = p.process(&x, &mut y).unwrap().produced;
            assert!(n >= 181, "{n}");
            assert!(y[..180].iter().all(|v| v.to_bits() == 0), "{route:?}");
        }
    }

    /// Edge case "Recording": record, stop, play twice; both playbacks bit-identical (the chain
    /// holds no state across a recording and a later playback starts from rest).
    #[test]
    fn playback_starts_from_rest_after_a_recording() {
        let mut p = Pipeline::new(Settings::new(6000)).unwrap();
        p.set_volume(1.0).unwrap();
        let x = tone(6000, 6000);
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        let play = |p: &mut Pipeline| {
            p.start_playback(1).unwrap();
            let mut out = Vec::new();
            let mut buf = vec![0.0f32; 512];
            loop {
                let info = p.play(&mut buf).unwrap();
                out.extend_from_slice(&buf[..info.produced]);
                if info.finished {
                    break;
                }
            }
            out
        };
        let a = play(&mut p);
        let b = play(&mut p);
        assert_eq!(a, b);
        // 6000 kept samples are 51 whole frames (spec 008 FR-013: ⌈(6000 + 60) / 120⌉), then
        // the drain with the chain's settle.
        assert_eq!(a.len() as u32, 51 * 120 + p.drain_samples());
    }

    /// Research R-01: the one-pass and the player run the same `OutputChain` code: at 6 kHz
    /// with every earlier stage bypassed, `play` of a stored tone equals the one-pass of the
    /// same tone sample for sample (both through the same chain from rest).
    #[test]
    fn both_paths_run_the_same_chain() {
        let mut s = Settings::new(6000);
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.speech_codec_enabled = false;
        let x = tone(6000, 3000);
        for route in [Route::Speaker, Route::Earphone] {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(route);
            p.set_volume(0.6).unwrap();
            let mut y = vec![0.0f32; x.len()];
            let _ = p.process(&x, &mut y).unwrap();
            let mut q = Pipeline::new(s).unwrap();
            q.set_route(route);
            q.set_volume(0.6).unwrap();
            q.start_recording().unwrap();
            let _ = q.record(&x, &mut []).unwrap();
            let _ = q.stop_recording(&mut []).unwrap();
            q.start_playback(1).unwrap();
            let mut played = vec![0.0f32; x.len() + q.drain_samples() as usize];
            let info = q.play(&mut played).unwrap();
            assert!(info.finished);
            assert_eq!(played[..x.len()], y[..], "{route:?}");
        }
    }
}
