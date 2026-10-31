//! The transport: the engine's state (idle, recording, playing), the record and playback calls,
//! the route, and the reports (spec 006 FR-003 – FR-008, FR-013, FR-015, FR-016, FR-019;
//! research R-05, R-06; data-model.md › Transport; spec 008 FR-008, FR-013: the speech
//! encoder after the VAS, frames in the memory, the stop rule, frame-wise playback).
//!
//! The device records into its memory or plays an item back, never both at once: its power
//! amplifier is muted while recording (S-002), so recording and playback are exclusive states
//! here and a conflicting start is refused with the activity in progress. Each recording and
//! each playback starts from its path's reset state (A-037, an engine decision required by
//! FR-007).

use crate::changes::ChangeEntry;
use crate::codec::{self, Frame};
use crate::error::Error;
use crate::form;
use crate::memory::{Encoding, MemoryStatus, Span};
use crate::pipeline::{Pipeline, VasEvent};
use crate::settings::{Route, VasMode};
use crate::stages::speed::SpeedOut;

/// The engine's state (spec 006 FR-003): exactly one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum State {
    /// Neither recording nor playing.
    #[default]
    Idle,
    /// A recording is in progress; the speaker is muted (S-002).
    Recording,
    /// A playback is in progress toward the selected route.
    Playing,
}

/// The engine's readable state (spec 006 FR-003; contracts/rust-api.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct EngineState {
    /// Idle, recording or playing.
    pub state: State,
    /// The selected output route (spec 006 FR-015).
    pub route: Route,
    /// The number of the recording in progress (the count plus one) or being played; `None`
    /// when idle.
    pub current: Option<u32>,
    /// The speaker is muted: while recording (S-002: the amplifier's mute pin is in standby
    /// during recording). A label in this version; the amplifier is not modeled.
    pub speaker_muted: bool,
}

/// Why a recording stopped by itself (spec 006 FR-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StopReason {
    /// The stored total reached the time capacity (S-001: "FULL"; A-039: the partial
    /// recording is kept; with the speech codec on, on a frame boundary, spec 008 FR-013).
    MemoryFull,
}

/// The report of one `record` call, and of the stop flush (spec 006 FR-006; spec 008 FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[must_use = "read `stored` and `stopped`: the recording can stop inside a block"]
#[non_exhaustive]
pub struct RecordInfo {
    /// Device samples the record path kept in this call: appended to the recording with the
    /// speech codec bypassed, fed to the speech encoder with it on (the recording then grows by
    /// whole frames, `frames`; at "memory full" the samples after the last stored frame, fewer
    /// than 180, are not kept).
    pub stored: usize,
    /// VAS events this call generated (splices in drop mode, muted regions in mute mode),
    /// with positions in the recording's device samples from its start.
    pub events: usize,
    /// The VAS is paused at the end of the call.
    pub paused: bool,
    /// The recording stopped by itself during this call; the engine is idle and the recording
    /// is kept (a following `stop_recording` is `NotRecording`).
    pub stopped: Option<StopReason>,
    /// Speech-codec frames appended to the recording in this call (spec 008 FR-008); 0 with
    /// the codec bypassed.
    pub frames: usize,
    /// Speech-codec frames in the recording so far, this call's included (spec 008 FR-008).
    pub frames_total: u64,
    /// Record-side changes this call applied (spec 017 FR-005): scheduled changes that landed
    /// (a value landed on the one-pass reaches the recorder through `start_recording`'s adopt,
    /// with no entry);
    /// [`record_with_changes`](crate::Pipeline::record_with_changes) writes the entries.
    pub changes: usize,
    /// The record stream's input position after the call (spec 017 FR-005): the host samples
    /// fed since `start_recording`; after a self-stop, the recording's final input position.
    pub input_samples_total: u64,
}

/// What `stop_recording` did with the recording (spec 006 FR-005, FR-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StopResult {
    /// The recording is in the memory.
    Kept {
        /// Its number (1 to the count).
        number: u32,
        /// Its length in device samples (÷ 6000 for seconds, A-026); a whole number of frames
        /// (120 samples) with the speech codec on (spec 008 FR-013).
        length: u64,
    },
    /// No input was fed since the start, or (with the speech codec on) no device sample was
    /// kept before the stop: the flush was skipped and no recording was added (A-039, an
    /// engine decision; spec 008 FR-013).
    DiscardedEmpty,
}

/// The result of `stop_recording` (spec 006 FR-005; spec 008 FR-013).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "read `result`: the recording may have been discarded as empty"]
#[non_exhaustive]
pub struct StopOutcome {
    /// The stop flush's report (all zeros when the flush was skipped): the record path's
    /// latency of host silence, and with the speech codec on the padding to the frame
    /// boundary and the look-ahead.
    pub flush: RecordInfo,
    /// Kept, with its number and length, or discarded.
    pub result: StopResult,
    /// Post-VAS device samples of silence produced after the last represented input sample
    /// (spec 008 FR-013): the padding to the frame boundary plus the 60-sample look-ahead,
    /// `120·F − n′ + 60` for F frames and n′ kept samples; 0 with the speech codec bypassed
    /// (spec 006's stop, byte for byte) and when the recording was discarded. "Silence" means
    /// **zero-valued input**: the stop feeds zeros through the whole record path, so with stage 1
    /// on the recording carries the capsule's hiss there (spec 012 FR-009).
    pub silence_fed_samples: u64,
    /// Record-side changes the stop's feed applied (spec 017 FR-005);
    /// [`stop_recording_with_changes`](crate::Pipeline::stop_recording_with_changes) writes
    /// the entries. 0 when the flush was skipped.
    pub changes: usize,
    /// The recording's final input position (spec 017 FR-005): every host sample fed since
    /// `start_recording`, the stop's silence included. A change scheduled at or past it was
    /// dropped without an entry.
    pub input_samples_total: u64,
    /// The record stream's input position when the stop was called: the host samples fed
    /// before it, the stop's silence excluded (spec 020 D9). A re-render feeds exactly this
    /// many samples and calls the same stop.
    pub stop_input_position: u64,
    /// The kept device sample at which the speech codec's padding begins (spec 020 D9): the
    /// kept count after the latency flush with the codec on; the recording's length with the
    /// codec bypassed, after a self-stop, or when the flush itself filled the memory (no
    /// padding is fed then).
    pub padding_start_sample: u64,
}

/// The report of one `play` call (spec 006 FR-006, FR-008; spec 008 FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[must_use = "read `produced` and `finished`"]
#[non_exhaustive]
pub struct PlayInfo {
    /// Host samples written to the start of the output buffer.
    pub produced: usize,
    /// The recording and its drain ended in this call; the engine is idle.
    pub finished: bool,
    /// Speech-codec frames decoded in this call (spec 008 FR-008); 0 with the codec bypassed.
    pub frames: usize,
    /// The index (0-based) of the frame whose decoded samples are entering the playback path
    /// at the end of the call (what is heard lags by `playback_latency_samples`), held at the
    /// last frame during the drain; 0 with the codec bypassed (spec 008 FR-008). Since 0.13 it
    /// is ⌊place ÷ 120⌋ with the codec on (the same values at level 3), at most the last frame.
    pub frame_index: u64,
    /// Since 0.13 (spec 013 FR-009): the place in the recording, in whole stored device samples,
    /// of the last device sample computed in this call (the playback speed control's place), and
    /// the recording's length K once the place has reached the recording's end; 0 before any.
    /// It leads the audible output by
    /// [`playback_latency_samples`](crate::Pipeline::playback_latency_samples), like
    /// `frame_index`.
    pub place: u64,
    /// Since 0.16 (spec 015 FR-005): the control changes the playback stream applied in this
    /// call (scheduled ones and plain setters that moved the stream); the entries themselves
    /// are written by [`Pipeline::play_with_changes`].
    pub changes: usize,
}

/// The player's cursor into the recording being played (data-model.md › Transport; spec 008
/// research R-06: in stored units, frames or samples).
#[derive(Clone, Copy, Debug, Default)]
struct Cursor {
    span: Span,
    /// Stored units pulled so far (samples, or frames decoded).
    offset: usize,
    /// Host samples produced so far.
    produced: u64,
    /// Host samples the playback produces in all: ⌈(K′ + T)·m/l⌉ + the drain, unknown
    /// (`u64::MAX`) until the speed control's place reaches K (spec 013 research R-05; ⌈K·m/l⌉ +
    /// the drain at level 3, spec 006 research R-06).
    total: u64,
    /// The recording's number.
    number: u32,
}

/// Samples per frame as `usize`.
const FRAME: usize = codec::FRAME_SAMPLES as usize;
/// The look-ahead as `u64`.
const LOOKAHEAD: u64 = codec::LOOKAHEAD_SAMPLES as u64; // A-040 (design constant, 008 R-02)

/// The engine's state machine (research R-05).
#[derive(Clone, Debug)]
pub(crate) struct Transport {
    pub(crate) state: State,
    /// Any host sample was pushed since the recording started (an empty block does not count):
    /// decides the flush and the empty case (hardware review B5).
    fed_any: bool,
    cursor: Cursor,
    /// Post-VAS device samples fed to the speech encoder in this recording (spec 008 FR-013).
    kept: u64,
    /// Frames appended to the recording in progress (spec 008 FR-008).
    frames_total: u64,
    /// The decoded frame being played and the read position in it (120 = empty).
    decoded: [f32; FRAME],
    decoded_read: usize,
    /// The index of the frame being played (spec 008 FR-008).
    frame_index: u64,
    /// The recording counter (spec 012 FR-009): successful recording starts since
    /// construction. It never rewinds: `reset` and `reconfigure` keep it.
    pub(crate) counter: u64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            state: State::Idle,
            fed_any: false,
            cursor: Cursor::default(),
            kept: 0,
            frames_total: 0,
            decoded: [0.0; FRAME],
            decoded_read: FRAME,
            frame_index: 0,
            counter: 0, // 012 FR-009: 0 at construction
        }
    }
}

impl Transport {
    /// Idle, with the recording counter carried in (spec 012 FR-009: `reconfigure`).
    pub(crate) fn with_counter(counter: u64) -> Self {
        Self {
            counter,
            ..Self::default()
        }
    }

    /// Back to idle (spec 006 FR-019: `reset`), keeping the recording counter (spec 012
    /// FR-009: it never rewinds).
    pub(crate) fn reset(&mut self) {
        *self = Self {
            counter: self.counter,
            ..Self::default()
        };
    }
}

/// The refusal for a path in progress, if any (spec 006 FR-003).
fn refuse_if_active(state: State) -> Result<(), Error> {
    match state {
        State::Recording => Err(Error::RecordingInProgress),
        State::Playing => Err(Error::PlaybackInProgress),
        _ => Ok(()),
    }
}

impl Pipeline {
    /// The refusal the one-pass calls and `reconfigure` give while a path is active.
    pub(crate) fn refuse_if_active(&self) -> Result<(), Error> {
        refuse_if_active(self.transport.state)
    }

    /// Starts a recording (spec 006 FR-003, FR-004, FR-012): the record path is reset (the
    /// limiter discharged, the VAS unpaused, the filters and the conversion cleared; A-037),
    /// the speech encoder with it (spec 008 FR-018), and the recording in progress is opened as
    /// number `count + 1`. Since 0.12 the capsule's hiss is keyed to the **recording counter**
    /// and the counter then advances (spec 012 FR-009; A-049): each recording carries its own
    /// stream from its beginning. The counter counts successful starts since construction and
    /// never rewinds: [`reset`](Self::reset) and [`reconfigure`](Self::reconfigure) keep it, a
    /// refused start does not advance it, and the one-pass processing uses its current value
    /// (the next recording's stream). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`], [`Error::PlaybackInProgress`], then
    /// [`Error::MemoryFull`] (no recorded time remains) before [`Error::NoRecordingSlots`]
    /// (S-001: "FULL" refuses a start).
    pub fn start_recording(&mut self) -> Result<(), Error> {
        self.refuse_if_active()?;
        // Spec 020 D5: the slot block, written as the recording opens: the counter the hiss
        // stream is keyed with, the configuration (every landed value is in `settings`), the
        // memory's remaining capacity before the open.
        let block = crate::memory::SlotBlock {
            counter: self.transport.counter,
            settings: self.settings,
            remaining_samples: self.memory.status().remaining_samples,
            events_total: 0,
            changes_total: 0,
        };
        self.memory.open(Some(block))?;
        self.recorder.reset();
        // 012 FR-009: the recorder's hiss is stream `counter`; the counter never rewinds.
        self.recorder.rekey(self.transport.counter);
        // 017 D4 (the adopt rule): the settings' record-side values, a one-pass landing's
        // included, are in the stages when the recording starts; no entry. The record
        // schedule is kept (D5).
        self.recorder.adopt(&self.settings);
        self.transport.counter = self.transport.counter.wrapping_add(1);
        if let Some(enc) = &mut self.encoder {
            enc.reset();
        }
        self.transport.fed_any = false;
        self.transport.kept = 0;
        self.transport.frames_total = 0;
        self.transport.state = State::Recording;
        Ok(())
    }

    /// Feeds one block of host-rate input to the record path and appends what it produces to
    /// the recording in progress; writes the block's VAS events to `events` as
    /// [`process_with_events`](Self::process_with_events) does, with positions in the
    /// recording's device samples from its start (spec 006 FR-001, FR-006). With the speech
    /// codec on, the kept samples go to the encoder and every completed frame to the memory
    /// (spec 008 FR-008). The recording stops by itself at the exact device sample (or frame)
    /// at which the memory fills (`stopped`, FR-012); the rest of the block is discarded and
    /// the engine is idle. With the speech codec on, a VAS event reported in the stopping block
    /// may name a position up to the look-ahead (60 device samples) past the stored length: the
    /// recording closes on the push that completes the frame filling the memory, and the
    /// look-ahead samples kept after that frame are not represented (spec 008 FR-014; research
    /// R-07). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NotRecording`] when no recording is in progress; nothing changes.
    pub fn record(&mut self, input: &[f32], events: &mut [VasEvent]) -> Result<RecordInfo, Error> {
        if self.transport.state != State::Recording {
            return Err(Error::NotRecording);
        }
        if !input.is_empty() {
            self.transport.fed_any = true;
        }
        // Spec 017 research R-02: values landed on the one-pass become pending here.
        self.recorder.begin_call(&self.settings, &self.record_tags);
        let mut info = RecordInfo::default();
        let _ = self.feed(input, events, &mut info);
        if !input.is_empty() {
            self.finalize_record(); // 016 FR-001: a non-empty call
        }
        self.take_landed_record(true); // 017 D4
        info.changes = self.recorder.ctl.log_len();
        info.input_samples_total = self.recorder.ctl.input;
        // Spec 020 D5: the totals (a self-stop in this call closed the block as kept; it is
        // still the last).
        if let Some(b) = self.memory.last_block_mut() {
            b.events_total += info.events as u64;
            b.changes_total += info.changes as u64;
        }
        Ok(info)
    }

    /// As [`record`](Self::record), and writes the record-side changes this call applied to
    /// `changes` (spec 017 FR-005), each a [`ChangeEntry`] at the **input** index it landed on;
    /// `info.changes` counts them all;
    /// [`changes::MAX_CHANGES_PER_CALL`](crate::changes::MAX_CHANGES_PER_CALL) is always
    /// enough. Real-time safe.
    ///
    /// # Errors
    ///
    /// As [`record`](Self::record).
    pub fn record_with_changes(
        &mut self,
        input: &[f32],
        events: &mut [VasEvent],
        changes: &mut [ChangeEntry],
    ) -> Result<RecordInfo, Error> {
        let info = self.record(input, events)?;
        self.recorder.ctl.drain_log(changes);
        Ok(info)
    }

    /// An `events` length for [`record`](Self::record) that is always enough for a block of
    /// `frames` host samples with the current settings (spec 003 R-07), every pending hang
    /// change included (spec 017 FR-002). Never allocates.
    pub fn max_record_events(&self, frames: usize) -> usize {
        self.recorder.max_events(frames, self.settings.vas.hang_ms)
    }

    /// Stops the recording (spec 006 FR-005): passes the record path's own latency of silence
    /// through it, so every input sample fed is represented in the recording, closes an open
    /// muted region with a synthesized event, and closes the recording as `Kept`; or, when
    /// nothing was fed since the start, skips the flush and discards it (`DiscardedEmpty`,
    /// A-039). The flush's report is returned like a `record` call's (it can itself fill the
    /// memory). Real-time safe.
    ///
    /// With the speech codec on (spec 008 FR-013): when no device sample was kept before the
    /// stop the recording is discarded without any feed; otherwise, after the flush, host
    /// silence is fed until the encoder has emitted the frame holding the last represented
    /// sample (the padding to the frame boundary plus the 60-sample look-ahead,
    /// `silence_fed_samples`), **with the VAS gate held open in drop mode** so a paused gate
    /// cannot prevent the last frame (a forced resume records its splice event); in mute mode
    /// the padding is stored as muted zeros. The recording's length is a whole number of
    /// frames; the work is bounded by the flush plus 179 device samples' worth of input.
    ///
    /// # Errors
    ///
    /// [`Error::NotRecording`] when no recording is in progress, which includes a recording
    /// that already stopped at the memory's limit.
    pub fn stop_recording(&mut self, events: &mut [VasEvent]) -> Result<StopOutcome, Error> {
        if self.transport.state != State::Recording {
            return Err(Error::NotRecording);
        }
        // Spec 017 FR-002, FR-005: the record schedule applies during the stop's feed; values
        // landed on the one-pass become pending here; the end of the recording clears the
        // schedule (`close_recording`) and the report carries the final input position.
        self.recorder.begin_call(&self.settings, &self.record_tags);
        let stop_input_position = self.recorder.ctl.input; // 020 D9: before the flush
        let mut outcome = self.stop_recording_inner(events)?;
        self.take_landed_record(true);
        outcome.changes = self.recorder.ctl.log_len();
        outcome.input_samples_total = self.recorder.ctl.input;
        outcome.stop_input_position = stop_input_position;
        // Spec 020 D5: the totals, for a kept recording only (a discarded one has no block).
        if let StopResult::Kept { .. } = outcome.result {
            if let Some(b) = self.memory.last_block_mut() {
                b.events_total += outcome.flush.events as u64;
                b.changes_total += outcome.changes as u64;
            }
        }
        Ok(outcome)
    }

    /// As [`stop_recording`](Self::stop_recording), and writes the record-side changes the
    /// stop's feed applied to `changes` (spec 017 FR-005). Real-time safe.
    ///
    /// # Errors
    ///
    /// As [`stop_recording`](Self::stop_recording).
    pub fn stop_recording_with_changes(
        &mut self,
        events: &mut [VasEvent],
        changes: &mut [ChangeEntry],
    ) -> Result<StopOutcome, Error> {
        let outcome = self.stop_recording(events)?;
        self.recorder.ctl.drain_log(changes);
        Ok(outcome)
    }

    fn stop_recording_inner(&mut self, events: &mut [VasEvent]) -> Result<StopOutcome, Error> {
        let codec_on = self.encoder.is_some();
        // The second condition (spec 008 FR-013: no kept sample before the stop) cannot occur
        // through the API: the VAS starts open and the conversion yields a device sample on the
        // first host sample, so any input keeps at least one. Kept as the spec's rule.
        // coverage: unreachable by construction (Constitution III)
        if !self.transport.fed_any || (codec_on && self.transport.kept == 0) {
            self.transport.state = State::Idle;
            self.recorder.clear_schedule(); // 017 D5: the skipped flush is an end too
            let result = self.memory.close(false);
            return Ok(StopOutcome {
                flush: RecordInfo::default(),
                result,
                silence_fed_samples: 0,
                changes: 0,
                input_samples_total: 0,
                stop_input_position: 0,
                padding_start_sample: 0,
            });
        }
        let mut flush = RecordInfo::default();
        let pre_encoder = self.recorder.latency_host_samples_pre_encoder();
        if !codec_on {
            // Spec 006's stop, byte for byte (spec 008 FR-021).
            let stopped = self.feed_zeros(pre_encoder, events, &mut flush);
            let result = match stopped {
                Some(result) => result,
                None => self.close_recording(events, &mut flush),
            };
            if pre_encoder > 0 {
                self.finalize_record(); // 016 FR-001: the flush's figures
            }
            let length = match result {
                StopResult::Kept { length, .. } => length,
                StopResult::DiscardedEmpty => 0,
            };
            return Ok(StopOutcome {
                flush,
                result,
                silence_fed_samples: 0,
                changes: 0,
                input_samples_total: 0,
                stop_input_position: 0,
                padding_start_sample: length, // 020 D9: no padding without the codec
            });
        }
        let drop_mode = self.settings.vas.enabled && self.settings.vas.mode == VasMode::Drop;
        if drop_mode {
            self.recorder.force_open(true);
        }
        let mut stopped = self.feed_zeros(pre_encoder, events, &mut flush);
        // n′: the kept samples, the flush's included; the last frame F − 1 completes when the
        // kept count reaches 120·F + 60.
        let represented = self.transport.kept;
        let frames_needed = represented.div_ceil(FRAME as u64);
        let target = frames_needed
            .saturating_mul(FRAME as u64)
            .saturating_add(LOOKAHEAD);
        // Bounded: each host zero advances the conversion by at most one device sample, and
        // the padding is at most 179 device samples plus the conversion's own delay.
        let (l, m) = self.plan_l_m;
        let bound = (target.saturating_sub(represented))
            .saturating_mul(m.div_ceil(l))
            .saturating_add(u64::from(pre_encoder).saturating_mul(2))
            .saturating_add(16);
        let mut fed = 0u64;
        while stopped.is_none() && self.transport.kept < target && fed < bound {
            stopped = self.feed_zeros(1, events, &mut flush);
            fed = fed.saturating_add(1);
        }
        debug_assert!(
            stopped.is_some() || self.transport.kept >= target,
            "the stop feed reached its bound"
        );
        if drop_mode {
            self.recorder.force_open(false);
        }
        let silence_fed_samples = self.transport.kept.saturating_sub(represented);
        // 020 D9: the padding starts at `represented`; a flush that filled the memory fed no
        // padding, so the figure is the length then.
        let padding_start_sample = match stopped {
            Some(StopResult::Kept { length, .. }) => length,
            _ => represented,
        };
        let result = match stopped {
            Some(result) => result,
            None => self.close_recording(events, &mut flush),
        };
        if pre_encoder > 0 || fed > 0 {
            self.finalize_record(); // 016 FR-001: the flush's figures
        }
        Ok(StopOutcome {
            flush,
            result,
            silence_fed_samples,
            changes: 0,
            input_samples_total: 0,
            stop_input_position: 0,
            padding_start_sample,
        })
    }

    /// Starts playback of recording `number` through the playback path, reset first (spec 006
    /// FR-002, FR-004, FR-008), the speech decoder with it (spec 008 FR-018). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`], [`Error::PlaybackInProgress`], or
    /// [`Error::NoSuchRecording`] for 0 or a number above the count.
    pub fn start_playback(&mut self, number: u32) -> Result<(), Error> {
        self.refuse_if_active()?;
        let span = self
            .memory
            .span(number)
            .ok_or(Error::NoSuchRecording { number })?;
        self.player.reset();
        self.player.set_controls(self.route, self.volume); // spec 009 FR-009
        // Spec 015 D3: the stream starts with the readings, no entry; the schedule is kept.
        self.player.set_start_values(
            self.route,
            self.volume,
            self.speed_level,
            self.battery_volts,
        );
        if let Some(dec) = &mut self.decoder {
            dec.reset();
        }
        // Spec 013 FR-005, FR-007: the speed control from rest at the recording's start, at the
        // host's level; the total (⌈(K′ + T)·m/l⌉ + the drain) is fixed when its place reaches K
        // (research R-05), so it starts unknown.
        let k = span.len as u64 * self.memory.encoding().unit_samples() as u64;
        self.speed.reset(self.speed_level, k);
        self.transport.cursor = Cursor {
            span,
            offset: 0,
            produced: 0,
            total: u64::MAX, // 013 R-05: fixed at the speed control's latch
            number,
        };
        self.transport.decoded_read = FRAME;
        self.transport.frame_index = 0;
        self.transport.state = State::Playing;
        Ok(())
    }

    /// Produces the next block of host-rate output of the recording being played (spec 006
    /// FR-002, FR-006, FR-008): exactly ⌈(K′ + T)·m/l⌉ samples for K stored device samples
    /// played at the playback speed level(s) set (spec 013 FR-005: K′ device samples of the
    /// recording's own part, ⌈K ÷ r⌉ at a constant level, and the speed control's tail of T;
    /// ⌈K·m/l⌉ at level 3), then the drain (`drain_samples()` of settled silence), then
    /// `finished` and idle. With the speech codec on the frames are decoded one at a time as the
    /// output needs them (spec 008 FR-008: `frames`, `frame_index`). Only `output[..produced]`
    /// is written. Real-time safe: at most 2 stored samples per device output (and up to 29
    /// more, with at most one frame decoded, when the level leaves 3), each frame decoded once.
    ///
    /// # Errors
    ///
    /// [`Error::NotPlaying`] when no playback is in progress; nothing changes.
    pub fn play(&mut self, output: &mut [f32]) -> Result<PlayInfo, Error> {
        if self.transport.state != State::Playing {
            return Err(Error::NotPlaying);
        }
        // Spec 009 FR-009: the route, the volume and the level land at this call's first
        // produced sample, with the scheduled changes (spec 015 research R-02, R-07).
        self.player.begin_call(
            self.route,
            self.volume,
            Some(self.speed_level),
            self.battery_volts,
        );
        let mut produced = 0;
        let mut frames = 0;
        #[cfg(feature = "__test-hooks")]
        let mut trace = core::mem::take(&mut self.speed_trace); // its capacity kept; no allocation
        #[cfg(feature = "__test-hooks")]
        trace.clear();
        let Pipeline {
            memory,
            player,
            transport,
            decoder,
            speed,
            plan_l_m,
            ..
        } = self;
        let (l, m) = *plan_l_m;
        let drain = u64::from(player.drain_len_host_samples());
        let span = transport.cursor.span;
        let Transport {
            cursor,
            decoded,
            decoded_read,
            frame_index,
            ..
        } = transport;
        // The source (spec 013 research R-01): the next stored device sample, decoding a frame
        // when the codec is on, or `None` past the recording's end.
        let mut source = || -> Option<f32> {
            match decoder {
                None => {
                    let v = memory.samples(span).get(cursor.offset).copied();
                    cursor.offset += 1;
                    v
                }
                Some(dec) => {
                    if *decoded_read >= FRAME && cursor.offset < span.len {
                        let bytes = memory.frames(span);
                        let size = codec::FRAME_BYTES as usize;
                        let at = cursor.offset * size;
                        let mut frame: Frame = [0; codec::FRAME_BYTES as usize];
                        frame.copy_from_slice(&bytes[at..at + size]);
                        dec.decode(&frame, decoded);
                        *decoded_read = 0;
                        cursor.offset += 1;
                        frames += 1;
                    }
                    if *decoded_read < FRAME {
                        let v = decoded[*decoded_read];
                        *decoded_read += 1;
                        Some(v)
                    } else {
                        None
                    }
                }
            }
        };
        while produced < output.len() && player.ctl.produced < cursor.total {
            // Spec 015 research R-02: the apply point at the top of host sample `produced`,
            // before any pull for it (a speed request then reaches the pull d(P)).
            player.apply_if_due(&mut |level| speed.request(level));
            let y = loop {
                if let Some(y) = player.next_wide() {
                    break player.emit_applied(y);
                }
                // The next device sample: the speed control's own part and tail, then the
                // drain's silence (spec 013 FR-005; research R-04).
                let d = match speed.next(&mut source) {
                    SpeedOut::Own(v) | SpeedOut::Tail(v) => v,
                    SpeedOut::Silence => 0.0,
                };
                #[cfg(feature = "__test-hooks")]
                if trace.len() < trace.capacity() {
                    trace.push(d); // within the reserved capacity: no allocation
                }
                // 013 research R-05: the total is fixed once the place reaches K; the host
                // sample being produced needs this device sample, so it lies within the total.
                if cursor.total == u64::MAX {
                    if let Some(n) = speed.final_count() {
                        let pre = (u128::from(n) * u128::from(m)).div_ceil(u128::from(l)) as u64;
                        cursor.total = pre + drain;
                    }
                }
                player.push_device_f64(d);
            };
            output[produced] = y;
            produced += 1;
        }
        cursor.produced = player.ctl.produced;
        let done = cursor.produced >= cursor.total;
        // 013 FR-009: the place and the frame index from the speed control's place.
        let place = speed.place();
        if decoder.is_some() {
            *frame_index = (place / FRAME as u64).min((span.len as u64).saturating_sub(1));
        }
        let finished = done;
        if finished {
            transport.state = State::Idle;
        }
        let info = PlayInfo {
            produced,
            finished,
            frames,
            frame_index: transport.frame_index,
            place,
            changes: player.ctl.log_len(),
        };
        if produced > 0 {
            self.finalize_play(); // 016 FR-001: a non-empty call
        }
        self.take_landed_play();
        if finished {
            self.player.clear_schedule(); // 015 FR-002: the end of a playback
        }
        #[cfg(feature = "__test-hooks")]
        {
            self.speed_trace = trace; // 019 research R-06: the device-rate trace hook
        }
        Ok(info)
    }

    /// As [`play`](Self::play), and writes the control changes the playback stream applied in
    /// this call to `changes` (spec 015 FR-005), each a [`ChangeEntry`] at the output index it
    /// landed on; `info.changes` counts them all;
    /// [`changes::MAX_CHANGES_PER_CALL`](crate::changes::MAX_CHANGES_PER_CALL) is always
    /// enough. Real-time safe.
    ///
    /// # Errors
    ///
    /// As [`play`](Self::play).
    pub fn play_with_changes(
        &mut self,
        output: &mut [f32],
        changes: &mut [ChangeEntry],
    ) -> Result<PlayInfo, Error> {
        let info = self.play(output)?;
        self.player.ctl.drain_log(changes);
        Ok(info)
    }

    /// Spec 015 research R-07: the readings follow the changes the playback stream landed.
    fn take_landed_play(&mut self) {
        let ctl = &mut self.player.ctl;
        if let Some(r) = ctl.landed_route.take() {
            self.route = r;
        }
        if let Some(v) = ctl.landed_volume.take() {
            self.volume = v;
        }
        if let Some(l) = ctl.landed_level.take() {
            self.speed_level = l;
        }
        if let Some(b) = ctl.landed_battery.take() {
            self.battery_volts = b; // 018
        }
    }

    /// Stops the playback at once, without the drain (spec 006 FR-008). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NotPlaying`] when no playback is in progress.
    pub fn stop_playback(&mut self) -> Result<(), Error> {
        if self.transport.state != State::Playing {
            return Err(Error::NotPlaying);
        }
        self.transport.state = State::Idle;
        self.player.clear_schedule(); // 015 FR-002
        Ok(())
    }

    /// Erases recording `number` (spec 006 FR-013; S-001): its time returns to "remaining" and
    /// the later recordings move down by one. Allowed while idle or playing: erasing the
    /// recording being played stops the playback first (S-001 allows erasing the item being
    /// played); erasing a lower number leaves the playback undisturbed and renumbers the played
    /// item; erasing a higher number changes nothing about it. **Not real-time safe**: the
    /// stored samples after the erased recording move down (a memmove bounded by the store's
    /// size; no allocation).
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`] while recording; [`Error::NoSuchRecording`] for 0 or a
    /// number above the count. Nothing changes on an error.
    pub fn erase(&mut self, number: u32) -> Result<(), Error> {
        if self.transport.state == State::Recording {
            return Err(Error::RecordingInProgress);
        }
        if self.memory.span(number).is_none() {
            return Err(Error::NoSuchRecording { number });
        }
        if self.transport.state == State::Playing {
            let cursor = &mut self.transport.cursor;
            if cursor.number == number {
                self.transport.state = State::Idle;
                self.player.clear_schedule(); // 015 FR-002: the playback ended
            } else if number < cursor.number {
                cursor.number -= 1;
            }
        }
        let erased = self.memory.erase(number).expect("checked above");
        // `cursor.number` is the played item's new number, so this holds exactly when the
        // erased one was below it: its units moved down with the store.
        if self.transport.state == State::Playing && number <= self.transport.cursor.number {
            self.transport.cursor.span.start -= erased.len;
        }
        Ok(())
    }

    /// Erases every recording (spec 006 FR-013; S-001 "erase all"): the memory reads as new
    /// with its capacity kept. Allowed while idle or playing (the playback stops first).
    /// Not real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`] while recording; nothing changes.
    pub fn erase_all(&mut self) -> Result<(), Error> {
        if self.transport.state == State::Recording {
            return Err(Error::RecordingInProgress);
        }
        if self.transport.state == State::Playing {
            self.transport.state = State::Idle;
            self.player.clear_schedule(); // 015 FR-002: the playback ended
        }
        self.memory.erase_all();
        Ok(())
    }

    /// The recording counter recording `number` was keyed with (spec 020 D5, FR-004), or
    /// `None` for an imported recording (no slot block). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchRecording`].
    pub fn recording_counter_of(&self, number: u32) -> Result<Option<u64>, Error> {
        self.memory
            .block(number)
            .map(|b| b.map(|b| b.counter))
            .ok_or(Error::NoSuchRecording { number })
    }

    /// The exported length of recording `number`, in bytes, so the host can size its buffer
    /// (spec 006 FR-014; spec 008 FR-010): 52 + 10 × the stored frames with the speech codec
    /// on, 52 + 4 × the stored samples when it is bypassed (form version 2). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchRecording`].
    pub fn export_len(&self, number: u32) -> Result<usize, Error> {
        let span = self
            .memory
            .span(number)
            .ok_or(Error::NoSuchRecording { number })?;
        Ok(form::export_len(self.memory.encoding(), span.len))
    }

    /// Writes recording `number` as an opaque byte sequence into `out` and returns the bytes
    /// written, always [`export_len`](Self::export_len) (spec 006 FR-014; spec 008 FR-010). The
    /// host stores or sends the bytes unchanged and never parses them; the same recording
    /// exports to the same bytes every time. The form (contracts/recording-form.md, version 2)
    /// declares the recording's encoding, speech-codec frames or device-rate samples when the
    /// codec was bypassed, and for frames the codec's model version, so an importer knows which
    /// decoder they need (FR-011). Allowed in any state. Allocates nothing, but computes a CRC
    /// over the recording: not real-time safe (FR-017).
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchRecording`]; [`Error::BufferTooSmall`] with the needed length, `out`
    /// untouched.
    pub fn export(&self, number: u32, out: &mut [u8]) -> Result<usize, Error> {
        let span = self
            .memory
            .span(number)
            .ok_or(Error::NoSuchRecording { number })?;
        let payload = match self.memory.encoding() {
            Encoding::Frames => form::Payload::Frames(self.memory.frames(span)),
            Encoding::Samples => form::Payload::Samples(self.memory.samples(span)),
        };
        form::export(payload, out)
    }

    /// Adds a recording from the bytes of an export, in this or another engine instance of
    /// the same version configured with the same encoding, at any host rate (spec 006 FR-014;
    /// spec 008 FR-010, FR-012); returns its number. The recording has the length and the
    /// playback output of the original. Allowed while idle or playing (it only appends);
    /// refused while recording. Allocates nothing (the store is reserved), but validates the
    /// whole sequence: not real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`]; then, in the contract's order, [`Error::UnknownForm`],
    /// [`Error::UnsupportedFormVersion`] (version 1, spec 006's never-shipped samples form, a
    /// later version, or non-zero flags), [`Error::Damaged`] (truncated, altered, another device
    /// rate, an unknown encoding, no units, a sample that is not finite or lies outside the
    /// record path's output range, or a frame with a code point the encoder never produces),
    /// [`Error::UnsupportedCodecModelVersion`] (frames this release cannot decode; FR-011: from
    /// the 1.0 release every released codec model version keeps its decoder, before it a dropped
    /// version is refused),
    /// [`Error::EncodingDiffers`] (the other encoding than the engine's; checked before the fit
    /// so the host learns the real reason) and [`Error::DoesNotFit`] (the remaining time or
    /// slots). The memory is unchanged on every error.
    pub fn import(&mut self, bytes: &[u8]) -> Result<u32, Error> {
        if self.transport.state == State::Recording {
            return Err(Error::RecordingInProgress);
        }
        let v = form::validate(bytes)?;
        // Both imports check the encoding (`EncodingDiffers`) before the fit (`DoesNotFit`).
        match v.encoding {
            Encoding::Frames => self.memory.import_frames(v.units),
            Encoding::Samples => self.memory.import_samples(form::decode(v.units)),
        }
    }

    /// Selects the output route (spec 006 FR-015): speaker or earphone, at any time, during
    /// playback too; it takes effect at the next `play` block or processing call and survives
    /// `reset` and `reconfigure`. Since 0.9 the route selects which branch of the output chain
    /// runs and which output scale applies (spec 009 FR-001, FR-004): the volume control and
    /// power amplifier into the speaker template, or the earphone output; the newly selected
    /// branch starts from rest (FR-009); a volume transition in progress ends at once and the new
    /// route starts at the position last set (spec 011 FR-005). Real-time safe.
    pub fn set_route(&mut self, route: Route) {
        self.route = route;
    }

    /// The selected output route (spec 006 FR-015).
    pub fn route(&self) -> Route {
        self.route
    }

    /// The engine's state: idle, recording or playing, the route, the current recording's
    /// number and whether the speaker is muted (spec 006 FR-003). Real-time safe.
    pub fn state(&self) -> EngineState {
        let state = self.transport.state;
        EngineState {
            state,
            route: self.route,
            current: match state {
                State::Recording => Some(self.memory.count() as u32 + 1),
                State::Playing => Some(self.transport.cursor.number),
                _ => None,
            },
            speaker_muted: state == State::Recording, // S-002: the amp is muted while recording
        }
    }

    /// The memory's figures (spec 006 FR-011). Real-time safe.
    pub fn memory_status(&self) -> MemoryStatus {
        self.memory.status()
    }

    /// The length of recording `number` in device samples (÷ 6000 for seconds, A-026); a
    /// whole number of frames with the speech codec on. Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchRecording`].
    pub fn recording_length(&self, number: u32) -> Result<u64, Error> {
        self.memory
            .span(number)
            .map(|s| s.len as u64 * self.memory.encoding().unit_samples() as u64)
            .ok_or(Error::NoSuchRecording { number })
    }

    /// The host samples [`play`](Self::play) produces for recording `number` played at a
    /// constant playback speed `level` (spec 013 FR-009): ⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ host samples
    /// for K stored device samples at the level's numerator k (the recording's own part and the
    /// speed control's tail of T ≤ 46 device samples, 0 at level 3), plus
    /// [`drain_samples`](Self::drain_samples). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSpeedLevel`] (checked first), then [`Error::NoSuchRecording`].
    pub fn playback_length_samples(&self, number: u32, level: u8) -> Result<u64, Error> {
        if !(1..=crate::speed::LEVEL_COUNT).contains(&level) {
            return Err(Error::InvalidSpeedLevel { level });
        }
        let k = self.recording_length(number)?;
        let (l, m) = self.plan_l_m;
        let numerator = crate::stages::speed::numerator(level);
        let own = (5 * k).div_ceil(u64::from(numerator)); // 013 FR-005: ⌈5K ÷ k⌉
        let tail = crate::stages::speed::tail_len(k, numerator); // 013 R-04
        let pre_drain = (u128::from(own + tail) * u128::from(m)).div_ceil(u128::from(l)) as u64;
        Ok(pre_drain + u64::from(self.player.drain_len_host_samples()))
    }

    /// The record path's latency in host samples **to the speech encoder's input** (spec 020
    /// D8, research R-01): the conversion's delay plus the record filter's 1 kHz group delay,
    /// rounded to a host sample (spec 006 FR-016's figure without the codec's 180); the stop's
    /// flush feeds exactly this many zeros. Real-time safe.
    pub fn record_latency_pre_encoder_samples(&self) -> u32 {
        self.recorder.latency_host_samples_pre_encoder()
    }

    /// The record path's latency in host samples, the speech codec's delay included (see
    /// below). Spec 020's timeline maps use
    /// [`record_latency_pre_encoder_samples`](Self::record_latency_pre_encoder_samples): a
    /// stored recording's frames carry no kept-domain shift (spec 008 FR-004), so the 180 are
    /// the one-pass's alone.
    pub fn record_latency_samples(&self) -> u32 {
        self.recorder.latency_host_samples()
    }

    /// The playback path's latency in host samples: output samples from the start of playback
    /// until the first stored sample is represented, `round(D + τ_play · host / 6000)`, and
    /// measured as the path's 1 kHz group delay (spec 006 FR-016); the onset peak of a
    /// one-sample recording lies 0.2–0.3 device samples later and is reported for information
    /// only. The speech decoder adds nothing (spec 008 FR-004). The two latencies sum to
    /// [`latency_samples`](Self::latency_samples) within one sample. Real-time safe.
    pub fn playback_latency_samples(&self) -> u32 {
        self.player.latency_host_samples()
    }

    /// The host samples `play` produces after the recording's own part and the speed control's
    /// tail (⌈K·m/l⌉ at level 3; spec 013 FR-005): stage 10's
    /// settling length of device-rate silence (121 device samples at the committed
    /// coefficients, −100 dB; 0 with the stage bypassed), the rate conversion's full
    /// remaining response (spec 006 FR-008), and the output chain's settle
    /// ([`output::settle_samples`](crate::output::settle_samples), about 58 ms) when any
    /// output stage runs (spec 009 research R-08). Real-time safe.
    pub fn drain_samples(&self) -> u32 {
        self.player.drain_len_host_samples()
    }

    /// The bytes reserved for the recording store and its span table (spec 006 FR-017,
    /// SC-007; spec 008 SC-007): about 1.8 MB at the default capacity with the speech codec on,
    /// about 86 MB with it bypassed, virtual until recordings are written (commit charge on
    /// Windows). It never changes after construction or reconfiguration. Real-time safe.
    pub fn footprint_bytes(&self) -> usize {
        self.memory.footprint_bytes()
    }

    /// Feeds `input` to the recorder and the memory, accumulating into `info`. Returns the
    /// closed recording's result when the memory filled.
    fn feed(
        &mut self,
        input: &[f32],
        events: &mut [VasEvent],
        info: &mut RecordInfo,
    ) -> Option<StopResult> {
        for &x in input {
            if let Some(stopped) = self.feed_one(x, events, info) {
                return Some(stopped);
            }
        }
        info.paused = self.recorder.paused();
        info.frames_total = self.transport.frames_total;
        None
    }

    /// As [`feed`](Self::feed), with `n` zeros (the stop flush, spec 006 FR-005).
    fn feed_zeros(
        &mut self,
        n: u32,
        events: &mut [VasEvent],
        info: &mut RecordInfo,
    ) -> Option<StopResult> {
        for _ in 0..n {
            if let Some(stopped) = self.feed_one(0.0, events, info) {
                return Some(stopped);
            }
        }
        info.paused = self.recorder.paused();
        info.frames_total = self.transport.frames_total;
        None
    }

    /// One host sample: pushes it, appends a produced device sample (or feeds it to the speech
    /// encoder and appends the frame it completes), hands out the event, and closes the
    /// recording when the memory fills (spec 006 FR-012; spec 008 FR-008, FR-013).
    #[inline]
    fn feed_one(
        &mut self,
        x: f32,
        events: &mut [VasEvent],
        info: &mut RecordInfo,
    ) -> Option<StopResult> {
        let room = match self.recorder.push(x) {
            Some(kept) => {
                info.stored += 1;
                match &mut self.encoder {
                    None => self.memory.append_sample(kept),
                    Some(enc) => {
                        self.transport.kept += 1;
                        match enc.push(kept) {
                            None => true,
                            Some(frame) => {
                                info.frames += 1;
                                self.transport.frames_total += 1;
                                self.memory.append_frame(&frame)
                            }
                        }
                    }
                }
            }
            None => true,
        };
        if let Some(event) = self.recorder.take_event() {
            write_event(events, info, event);
        }
        if room {
            return None;
        }
        info.paused = self.recorder.paused();
        info.frames_total = self.transport.frames_total;
        info.stopped = Some(StopReason::MemoryFull);
        Some(self.close_recording(events, info))
    }

    /// Closes the recording in progress as kept: an open muted region gets its synthesized
    /// closing event (position = its start, length = stored − start; plan review P5b), and the
    /// engine returns to idle.
    fn close_recording(&mut self, events: &mut [VasEvent], info: &mut RecordInfo) -> StopResult {
        if let Some(event) = self.recorder.close_open_region() {
            write_event(events, info, event);
        }
        info.frames_total = self.transport.frames_total;
        self.transport.state = State::Idle;
        self.recorder.clear_schedule(); // 017 D5: every end of a recording
        self.memory.close(true)
    }
}

/// Counts `event` in `info` and writes it to `events` when a slot is free (spec 003 FR-005).
#[inline]
fn write_event(events: &mut [VasEvent], info: &mut RecordInfo, event: VasEvent) {
    if let Some(slot) = events.get_mut(info.events) {
        *slot = event;
    }
    info.events += 1;
}

/// 006 T019, T020 (spec 006 FR-003 – FR-008, FR-012, FR-015, FR-016, FR-019; research R-05,
/// R-06; data-model › Transitions).
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::settings::{Settings, VasMode};
    use alloc::vec::Vec;
    use rr_dr60_detmath::{TAU, exp, ln, sin};

    /// The spec 006 configuration: the defaults with the speech codec bypassed (spec 008
    /// FR-021: these tests state the 006 rules, which the codec's own tests restate for frames).
    fn settings_006(rate: u32) -> Settings {
        let mut s = crate::test_util::settings_011(rate); // 012 FR-010
        s.output = crate::settings::OutputSettings::BYPASSED; // 009 FR-016: the earlier checks run with stages 11–13 bypassed
        s.tap = crate::settings::Tap::AfterPlayback; // 009 FR-011: 0.8's output
        s.speech_codec_enabled = false;
        s
    }

    /// A 1 kHz tone at `dbfs` for `secs` at `fs`, phase 0.
    fn tone(dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / fs;
        (0..(secs * fs) as usize)
            .map(|n| (amp * sin(w * n as f64)) as f32)
            .collect()
    }

    /// A pipeline with a `secs` second memory of `max` recordings (never the default 60 min in
    /// a debug test, research R-11).
    fn small(rate: u32, secs: u32, max: u32) -> Pipeline {
        let mut s = settings_006(rate);
        s.memory.capacity_s = secs;
        s.memory.max_recordings = max;
        Pipeline::new(s).unwrap()
    }

    fn small_with(s: Settings, secs: u32, max: u32) -> Pipeline {
        let mut s = s;
        s.memory.capacity_s = secs;
        s.memory.max_recordings = max;
        Pipeline::new(s).unwrap()
    }

    /// Records `x` in one block and stops; returns the stop outcome.
    fn record_all(p: &mut Pipeline, x: &[f32]) -> StopOutcome {
        p.start_recording().unwrap();
        let info = p.record(x, &mut []).unwrap();
        assert_eq!(info.stopped, None);
        p.stop_recording(&mut []).unwrap()
    }

    /// Plays `number` to the end in blocks of `block`; returns the output and the drain.
    fn play_all(p: &mut Pipeline, number: u32, block: usize) -> (Vec<f32>, Vec<f32>) {
        p.start_playback(number).unwrap();
        let mut out = Vec::new();
        let mut buf = std::vec![0.0f32; block];
        loop {
            let info = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                break;
            }
            assert_eq!(info.produced, block, "a block short of the end");
        }
        let drain_at = out.len() - p.drain_samples() as usize;
        let drain = out.split_off(drain_at);
        (out, drain)
    }

    fn one_pass(s: Settings, x: &[f32]) -> Vec<f32> {
        let mut p = Pipeline::new(s).unwrap();
        let mut y = std::vec![0.0f32; x.len()];
        let n = p.process(x, &mut y).unwrap().produced;
        y.truncate(n);
        y
    }

    /// FR-003 (data-model › Transitions): the refusals in their check order, the states after
    /// each call, `speaker_muted` only while recording, `current` while recording and playing.
    #[test]
    fn transitions_and_refusals() {
        let mut p = small(6000, 2, 2);
        let x = tone(-20.0, 0.2, 6000.0);
        // Idle.
        assert_eq!(p.state().state, State::Idle);
        assert!(!p.state().speaker_muted);
        assert_eq!(p.state().current, None);
        assert_eq!(p.record(&x, &mut []), Err(Error::NotRecording));
        assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
        assert_eq!(p.play(&mut [0.0; 4]), Err(Error::NotPlaying));
        assert_eq!(p.stop_playback(), Err(Error::NotPlaying));
        assert_eq!(
            p.start_playback(0),
            Err(Error::NoSuchRecording { number: 0 })
        );
        assert_eq!(
            p.start_playback(1),
            Err(Error::NoSuchRecording { number: 1 })
        );
        assert_eq!(
            p.recording_length(1),
            Err(Error::NoSuchRecording { number: 1 })
        );
        // Recording.
        p.start_recording().unwrap();
        let st = p.state();
        assert_eq!(
            (st.state, st.current, st.speaker_muted),
            (State::Recording, Some(1), true)
        );
        assert_eq!(p.start_recording(), Err(Error::RecordingInProgress));
        assert_eq!(p.start_playback(1), Err(Error::RecordingInProgress));
        assert_eq!(
            p.reconfigure(*p.settings()),
            Err(Error::RecordingInProgress)
        );
        assert_eq!(
            p.process(&x, &mut x.clone()),
            Err(Error::RecordingInProgress)
        );
        assert_eq!(p.play(&mut [0.0; 4]), Err(Error::NotPlaying));
        assert_eq!(p.stop_playback(), Err(Error::NotPlaying));
        assert_eq!(
            p.state().state,
            State::Recording,
            "unchanged after refusals"
        );
        let info = p.record(&x, &mut []).unwrap();
        assert_eq!(info.stored, x.len());
        let out = p.stop_recording(&mut []).unwrap();
        assert!(
            matches!(out.result, StopResult::Kept { number: 1, .. }),
            "{out:?}"
        );
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.memory_status().count, 1);
        // Playing.
        p.start_playback(1).unwrap();
        let st = p.state();
        assert_eq!(
            (st.state, st.current, st.speaker_muted),
            (State::Playing, Some(1), false)
        );
        assert_eq!(p.start_recording(), Err(Error::PlaybackInProgress));
        assert_eq!(p.start_playback(1), Err(Error::PlaybackInProgress));
        assert_eq!(p.reconfigure(*p.settings()), Err(Error::PlaybackInProgress));
        assert_eq!(
            p.process(&x, &mut x.clone()),
            Err(Error::PlaybackInProgress)
        );
        assert_eq!(p.record(&x, &mut []), Err(Error::NotRecording));
        assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
        assert_eq!(p.state().state, State::Playing);
        p.stop_playback().unwrap();
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.state().current, None);
    }

    /// FR-015: the route is its own control, changeable during playback, reported, and kept
    /// through `reset` and `reconfigure`; both routes give the same samples.
    #[test]
    fn route_is_set_at_any_time_and_survives_reset_and_reconfigure() {
        let mut p = small(8000, 2, 2);
        // Spec 009 FR-016: with the output chain bypassed the route is the label of spec 006
        // (the same sound on both routes); with the chain on it selects the branch (009 US1).
        let mut s = *p.settings();
        s.output = crate::settings::OutputSettings::BYPASSED;
        p.reconfigure(s).unwrap();
        assert_eq!(p.route(), Route::Speaker);
        let x = tone(-20.0, 0.3, 8000.0);
        let _ = record_all(&mut p, &x);
        let (speaker, _) = play_all(&mut p, 1, 97);
        p.start_playback(1).unwrap();
        let mut buf = [0.0f32; 50];
        let _ = p.play(&mut buf).unwrap();
        p.set_route(Route::Earphone);
        assert_eq!(p.state().route, Route::Earphone);
        let mut rest = Vec::from(buf);
        loop {
            let info = p.play(&mut buf).unwrap();
            rest.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                break;
            }
        }
        let pre = rest.len() - p.drain_samples() as usize;
        assert_eq!(
            rest[..pre],
            speaker[..],
            "with the chain bypassed the route does not change the sound"
        );
        p.reset();
        assert_eq!(p.route(), Route::Earphone);
        p.reconfigure(*p.settings()).unwrap();
        assert_eq!(p.route(), Route::Earphone);
        assert_eq!(p.memory_status().count, 1, "reconfigure keeps the memory");
    }

    /// FR-019: `reset` in any state: a recording in progress is discarded without flush and the
    /// store truncated; a playback stops without drain; the memory's contents are kept.
    #[test]
    fn reset_in_any_state() {
        let mut p = small(6000, 2, 3);
        let x = tone(-20.0, 0.2, 6000.0);
        let _ = record_all(&mut p, &x);
        let used = p.memory_status().used_samples;
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        assert!(p.memory_status().used_samples > used);
        p.reset();
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.memory_status().count, 1);
        assert_eq!(
            p.memory_status().used_samples,
            used,
            "the in-progress samples are gone"
        );
        p.start_playback(1).unwrap();
        let _ = p.play(&mut [0.0; 10]).unwrap();
        p.reset();
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.play(&mut [0.0; 10]), Err(Error::NotPlaying));
        assert_eq!(p.memory_status().count, 1);
        // Reset also resets the one-pass stream, as before.
        p.reset();
    }

    /// FR-009: the one-pass calls are refused while active and the pipeline is unchanged
    /// (the in-place calls, which cannot report a reason, produce nothing).
    #[test]
    fn one_pass_refused_while_active_leaves_the_pipeline_unchanged() {
        let mut p = small(6000, 2, 2);
        let x = tone(-20.0, 0.2, 6000.0);
        p.start_recording().unwrap();
        let _ = p.record(&x[..600], &mut []).unwrap();
        let before = p.clone();
        let mut y = x.clone();
        assert_eq!(p.process(&x, &mut y), Err(Error::RecordingInProgress));
        assert_eq!(
            p.process_with_events(&x, &mut y, &mut []),
            Err(Error::RecordingInProgress)
        );
        let info = p.process_in_place(&mut y);
        assert_eq!((info.produced, info.events), (0, 0));
        let info = p.process_in_place_with_events(&mut y, &mut []);
        assert_eq!((info.produced, info.events), (0, 0));
        assert_eq!(y, x, "nothing written in place");
        // Clone-compare: the one-pass stream and the recorder are untouched.
        let mut a = p.clone();
        let mut b = before;
        let _ = a.stop_recording(&mut []).unwrap();
        let _ = b.stop_recording(&mut []).unwrap();
        let (ya, _) = play_all(&mut a, 1, 50);
        let (yb, _) = play_all(&mut b, 1, 50);
        assert_eq!(ya, yb);
        let (mut oa, mut ob) = (x.clone(), x.clone());
        let _ = a.process_in_place(&mut oa);
        let _ = b.process_in_place(&mut ob);
        assert_eq!(oa, ob);
    }

    /// FR-004 (A-037): each recording starts from the record path's reset state: after a
    /// +9 dBFS burst, a −20 dBFS tone recorded as item 2 equals a fresh one-pass of the tone.
    #[test]
    fn start_recording_resets_the_recorder() {
        for rate in [6000, 48_000] {
            let fs = f64::from(rate);
            let mut s = settings_006(rate);
            s.vas.enabled = false;
            let mut p = small_with(s, 5, 3);
            let _ = record_all(&mut p, &tone(9.0, 1.0, fs));
            let quiet = tone(-20.0, 0.5, fs);
            let _ = record_all(&mut p, &quiet);
            assert_eq!(p.memory_status().count, 2);
            let (item2, _) = play_all(&mut p, 2, 101);
            // The one-pass of the tone followed by the record latency + one device sample of
            // zeros (FR-007's prefix rule).
            let tail = p.record_latency_samples() as usize + (rate as usize).div_ceil(6000);
            let mut x = quiet.clone();
            x.extend(core::iter::repeat_n(0.0f32, tail));
            let want = one_pass(s, &x);
            assert!(
                want.len() >= item2.len(),
                "{rate} Hz: {} < {}",
                want.len(),
                item2.len()
            );
            assert_eq!(item2[..], want[..item2.len()], "{rate} Hz");
            // And a fresh playback starts from the player's reset state: the same twice.
            let (again, _) = play_all(&mut p, 2, 7);
            assert_eq!(again, item2);
        }
    }

    /// FR-005, FR-012: an empty `record(&[])` does not count as fed; stop then discards; and
    /// after a memory-full stop a `stop_recording` is `NotRecording`.
    #[test]
    fn empty_block_and_memory_full_stop() {
        let mut p = small(6000, 1, 2);
        p.start_recording().unwrap();
        let info = p.record(&[], &mut []).unwrap();
        assert_eq!(info, RecordInfo::default());
        let out = p.stop_recording(&mut []).unwrap();
        assert_eq!(out.result, StopResult::DiscardedEmpty);
        assert_eq!(out.flush, RecordInfo::default());
        assert_eq!(p.memory_status().count, 0);
        // 1 s capacity at 6 kHz: 6000 samples; feed 1.5 s with the VAS off.
        let mut s = settings_006(6000);
        s.vas.enabled = false;
        s.agc.enabled = false;
        let mut p = small_with(s, 1, 2);
        p.start_recording().unwrap();
        let x = tone(-20.0, 1.5, 6000.0);
        let info = p.record(&x, &mut []).unwrap();
        assert_eq!(info.stored, 6000);
        assert_eq!(info.stopped, Some(StopReason::MemoryFull));
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
        assert_eq!(p.recording_length(1), Ok(6000));
        assert_eq!(p.memory_status().remaining_samples, 0);
        assert_eq!(p.start_recording(), Err(Error::MemoryFull));
    }

    /// FR-005: `stop_recording` flushes the record latency of zeros: the stored count grows by
    /// the flush's device samples; with nothing fed there is no flush.
    #[test]
    fn stop_flushes_record_latency_of_zeros() {
        for rate in [6000, 8000, 44_100, 48_000] {
            let fs = f64::from(rate);
            let mut s = settings_006(rate);
            s.vas.enabled = false;
            let mut p = small_with(s, 5, 2);
            p.start_recording().unwrap();
            let x = tone(-20.0, 0.25, fs);
            let info = p.record(&x, &mut []).unwrap();
            let out = p.stop_recording(&mut []).unwrap();
            let latency = p.record_latency_samples() as u64;
            let StopResult::Kept { length, .. } = out.result else {
                panic!("{rate} Hz: {out:?}")
            };
            assert_eq!(length, (info.stored + out.flush.stored) as u64, "{rate} Hz");
            // The flush's device samples: ⌊(N + L)·l/m⌋-ish; bounded by the latency in device
            // samples plus one.
            let device_latency = (latency * 6000).div_ceil(u64::from(rate));
            assert!(
                out.flush.stored as u64 <= device_latency + 1 && out.flush.stored > 0,
                "{rate} Hz: flush stored {} for latency {latency}",
                out.flush.stored
            );
            assert_eq!(out.flush.stopped, None);
        }
    }

    /// FR-007, FR-008 (research R-06): playback produces ⌈K·m/l⌉ samples then exactly the
    /// drain, `finished` only then, in blocks of 1, 7 and 4096 at the seven rates.
    #[test]
    fn playback_produces_ceil_k_m_over_l_then_drain() {
        let table: [(u32, u64, u64); 7] = [
            (6000, 1, 1),
            (8000, 3, 4),
            (16_000, 3, 8),
            (44_100, 20, 147),
            (48_000, 1, 8),
            (88_200, 10, 147),
            (96_000, 1, 16),
        ]; // spec 004 FR-002
        for (rate, l, m) in table {
            let mut s = settings_006(rate);
            s.vas.enabled = false;
            let mut p = small_with(s, 5, 2);
            let out = record_all(&mut p, &tone(-20.0, 0.1, f64::from(rate)));
            let StopResult::Kept { length: k, .. } = out.result else {
                panic!()
            };
            let want = (k * m).div_ceil(l) + u64::from(p.drain_samples());
            for block in [1usize, 7, 4096] {
                p.start_playback(1).unwrap();
                let mut buf = std::vec![0.0f32; block];
                let mut total = 0u64;
                let mut finished_at = None;
                for call in 0.. {
                    let info = p.play(&mut buf).unwrap();
                    total += info.produced as u64;
                    if info.finished {
                        finished_at = Some(call);
                        break;
                    }
                    assert_eq!(info.produced, block, "{rate} Hz block {block}: short block");
                }
                assert_eq!(total, want, "{rate} Hz, block {block}");
                assert!(finished_at.is_some());
                assert_eq!(p.state().state, State::Idle);
                assert_eq!(p.play(&mut buf), Err(Error::NotPlaying));
            }
        }
    }

    /// FR-008: with stage 10 off at 6 kHz the drain is zero: playback is exactly K samples.
    #[test]
    fn drain_zero_when_stage10_off_at_6k() {
        let mut s = settings_006(6000);
        s.vas.enabled = false;
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        let mut p = small_with(s, 1, 1);
        assert_eq!(p.drain_samples(), 0);
        assert_eq!(p.record_latency_samples(), 0);
        let x: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let out = record_all(&mut p, &x);
        assert_eq!(
            out.result,
            StopResult::Kept {
                number: 1,
                length: 100
            }
        );
        let (y, drain) = play_all(&mut p, 1, 33);
        assert_eq!(y, x, "bypass_all at 6 kHz stores and plays the input");
        assert!(drain.is_empty());
    }

    /// FR-005, FR-006 (plan review P5b): a mute-mode region still open at stop gets a
    /// synthesized event, position = its start, length = stored − start; events are device
    /// positions from the recording's start.
    #[test]
    fn mute_region_open_at_stop_gets_a_synthesized_event() {
        let mut s = settings_006(6000);
        s.vas.mode = VasMode::Mute;
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.adc_clamp_enabled = false;
        let mut p = small_with(s, 10, 2);
        // 1 s of tone, then 3 s of silence: the hang (1 s) then a 2 s open region at stop.
        let x = [tone(-20.0, 1.0, 6000.0), std::vec![0.0; 18_000]].concat();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 4];
        let info = p.record(&x, &mut ev).unwrap();
        assert_eq!(info.events, 0, "the region is still open");
        assert!(info.paused);
        assert_eq!(info.stored, x.len(), "mute mode stores the silence");
        let out = p.stop_recording(&mut ev).unwrap();
        assert_eq!(out.flush.events, 1, "{out:?}");
        let StopResult::Kept { length, .. } = out.result else {
            panic!()
        };
        assert_eq!(
            ev[0].output_position, 12_000,
            "device samples from the start"
        );
        assert_eq!(
            ev[0].input_length,
            length - 12_000,
            "to the end of the recording"
        );
        assert_eq!(out.flush.stored as u64, length - x.len() as u64);
    }

    /// FR-006: in drop mode the events are device positions from the recording's start
    /// (position = the kept count at the resume; length = the dropped device samples).
    #[test]
    fn events_are_device_positions_from_the_recording_start() {
        let mut s = settings_006(48_000);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.vas.hang_ms = 500.0;
        let mut p = small_with(s, 10, 2);
        let x = [
            tone(-30.0, 1.0, 48_000.0),
            std::vec![0.0; 2 * 48_000],
            tone(-30.0, 1.0, 48_000.0),
        ]
        .concat();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 4];
        let info = p.record(&x, &mut ev).unwrap();
        assert_eq!(info.events, 1);
        // The decimator delays the stream by D = 238 host samples, 29.75 device samples, so the
        // burst occupies device samples ≈ 30 … 6030. Kept before the splice: the burst, the
        // 0.5 s hang and that delay: 6000 + 3000 + 30 = 9030 device samples (± one device sample
        // of edge smear, 003 R-11).
        let delay = (238.0f64 * 6000.0 / 48_000.0).round() as i64; // D from research R-06
        let position = ev[0].output_position as i64;
        assert!((position - (9000 + delay)).abs() <= 1, "{:?}", ev[0]);
        // Dropped: the 2 s gap minus the hang, plus the 20 ms onset of the second burst:
        // 12 000 − 3000 + 120 = 9120 device samples (± 1).
        assert!((ev[0].input_length as i64 - 9120).abs() <= 1, "{:?}", ev[0]);
        // Kept after the splice: the second burst minus the onset, less what is still inside
        // the conversion when the input ends (about 30 device samples).
        let after = info.stored as i64 - position;
        assert!(
            (5880 - delay - 2..=5881).contains(&after),
            "kept after the splice: {after}"
        );
    }

    /// 006 T027 (FR-012; A-039): with a 1 s capacity at 6 kHz, the block that crosses the limit
    /// reports `stored` = the remainder and `stopped = MemoryFull`, the engine is idle and the
    /// recording is kept with length 6 000.
    #[test]
    fn recording_stops_at_the_exact_capacity_sample_inside_a_block() {
        let mut s = settings_006(6000);
        s.vas.enabled = false;
        s.agc.enabled = false;
        let mut p = small_with(s, 1, 2);
        p.start_recording().unwrap();
        let x = tone(-20.0, 0.7, 6000.0);
        let first = p.record(&x, &mut []).unwrap();
        assert_eq!((first.stored, first.stopped), (4200, None));
        assert_eq!(p.memory_status().remaining_samples, 1800);
        let second = p.record(&x, &mut []).unwrap();
        assert_eq!(second.stored, 1800, "the remainder, not the block");
        assert_eq!(second.stopped, Some(StopReason::MemoryFull));
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.state().current, None);
        assert_eq!(p.recording_length(1), Ok(6000));
        let st = p.memory_status();
        assert_eq!(
            (st.used_samples, st.remaining_samples, st.count),
            (6000, 0, 1)
        );
        assert_eq!(p.record(&x, &mut []), Err(Error::NotRecording));
        assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
    }

    /// 006 T027 (FR-005, FR-012): the stop flush itself can fill the memory: the flush report
    /// carries `stopped`, the recording is kept at exactly the capacity, the engine is idle.
    #[test]
    fn flush_cut_by_capacity() {
        let mut s = settings_006(48_000);
        s.vas.enabled = false;
        s.agc.enabled = false;
        let mut p = small_with(s, 1, 2);
        p.start_recording().unwrap();
        // Feed one host sample at a time until 10 device samples of room remain.
        let x = tone(-20.0, 2.0, 48_000.0);
        let mut fed = 0;
        while p.memory_status().remaining_samples > 10 {
            let info = p.record(&x[fed..fed + 1], &mut []).unwrap();
            assert_eq!(info.stopped, None);
            fed += 1;
        }
        assert_eq!(p.memory_status().remaining_samples, 10);
        let flush_device = p.record_latency_samples() / 8; // 48 kHz → 6 kHz
        assert!(
            flush_device > 10,
            "the flush ({flush_device}) must be the longer"
        );
        let out = p.stop_recording(&mut []).unwrap();
        assert_eq!(out.flush.stored, 10);
        assert_eq!(out.flush.stopped, Some(StopReason::MemoryFull));
        assert_eq!(
            out.result,
            StopResult::Kept {
                number: 1,
                length: 6000
            }
        );
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.memory_status().remaining_samples, 0);
        assert_eq!(p.start_recording(), Err(Error::MemoryFull));
    }

    /// 006 T027 (FR-012; S-001): the count limit refuses only a start, with time left.
    #[test]
    fn count_limit_only_prevents_start() {
        let mut p = small(6000, 10, 2);
        let x = tone(-20.0, 0.5, 6000.0);
        for n in 1..=2 {
            assert!(matches!(
                record_all(&mut p, &x).result,
                StopResult::Kept { number, .. } if number == n
            ));
        }
        let st = p.memory_status();
        assert_eq!((st.count, st.slots_remaining), (2, 0));
        assert!(st.remaining_samples > 0);
        assert_eq!(p.start_recording(), Err(Error::NoRecordingSlots));
        assert_eq!(p.state().state, State::Idle);
        // Erase one and the start proceeds (US2 scenario 6 for the count).
        p.erase(1).unwrap();
        p.start_recording().unwrap();
        assert_eq!(p.state().current, Some(2));
        p.reset();
    }

    /// US2 scenario 4 at the device's own figure (FR-012; S-001: 99 items; final coach review):
    /// 99 short recordings fill the slots with time to spare; the 100th start is refused.
    #[test]
    fn ninety_nine_recordings_fill_the_device_slots() {
        let mut p = small(6000, 60, 99);
        let x = tone(-20.0, 0.05, 6000.0);
        for n in 1..=99u32 {
            assert!(matches!(
                record_all(&mut p, &x).result,
                StopResult::Kept { number, .. } if number == n
            ));
        }
        let st = p.memory_status();
        assert_eq!((st.count, st.slots_remaining), (99, 0));
        assert!(
            st.remaining_samples > 300_000,
            "time left: {}",
            st.remaining_samples
        );
        assert_eq!(p.start_recording(), Err(Error::NoRecordingSlots));
        p.erase(50).unwrap();
        assert_eq!(p.memory_status().count, 98);
        assert!(p.start_recording().is_ok());
        assert_eq!(p.state().current, Some(99));
        p.reset();
    }

    /// 006 T027 (FR-013; S-001: the played item may be erased): erasing the recording being
    /// played, or all, stops playback first; erase is refused while recording.
    #[test]
    fn erase_during_playback_of_the_played_item_stops_it() {
        let mut p = small(6000, 10, 3);
        let x = tone(-20.0, 0.2, 6000.0);
        for _ in 0..3 {
            let _ = record_all(&mut p, &x);
        }
        p.start_playback(2).unwrap();
        let mut buf = [0.0f32; 100];
        let _ = p.play(&mut buf).unwrap();
        p.erase(2).unwrap();
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.state().current, None);
        assert_eq!(p.memory_status().count, 2);
        assert_eq!(p.play(&mut buf), Err(Error::NotPlaying));
        // Erase all while playing.
        p.start_playback(1).unwrap();
        let _ = p.play(&mut buf).unwrap();
        p.erase_all().unwrap();
        assert_eq!(p.state().state, State::Idle);
        let st = p.memory_status();
        assert_eq!(
            (st.count, st.used_samples, st.remaining_samples),
            (0, 0, 60_000)
        );
        // Refused while recording; a bad number.
        assert_eq!(p.erase(1), Err(Error::NoSuchRecording { number: 1 }));
        p.start_recording().unwrap();
        assert_eq!(p.erase(1), Err(Error::RecordingInProgress));
        assert_eq!(p.erase_all(), Err(Error::RecordingInProgress));
        assert_eq!(p.state().state, State::Recording);
        p.reset();
    }

    /// 006 T027 (FR-013): erasing a lower number while playing leaves the playback undisturbed
    /// (its output bit-identical to an uninterrupted playback) and renumbers the played item;
    /// erasing a higher number changes nothing about it.
    #[test]
    fn erase_of_a_lower_number_renumbers_the_played_item() {
        let mut p = small(6000, 10, 4);
        for n in 1..=4 {
            let x = tone(-20.0 - f64::from(n), 0.3, 6000.0);
            let _ = record_all(&mut p, &x);
        }
        let mut reference = p.clone();
        let (want, _) = play_all(&mut reference, 3, 100);
        // Play 3, erase 1 after one block, then 4 (above), then play on.
        p.start_playback(3).unwrap();
        let mut out = Vec::new();
        let mut buf = [0.0f32; 100];
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        p.erase(1).unwrap();
        assert_eq!(p.state().state, State::Playing);
        assert_eq!(p.state().current, Some(2));
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        p.erase(3).unwrap(); // the former 4, above the played item
        assert_eq!(p.state().current, Some(2));
        assert_eq!(p.memory_status().count, 2);
        loop {
            let info = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                break;
            }
        }
        out.truncate(out.len() - p.drain_samples() as usize);
        assert_eq!(out.len(), want.len());
        assert!(
            out.iter()
                .zip(&want)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "playback disturbed by the erase"
        );
        assert_eq!(p.state().state, State::Idle);
        // The renumbered item plays the same again.
        let (again, _) = play_all(&mut p, 2, 100);
        assert_eq!(again, want);
    }

    /// 006 T027 (FR-019): reconfigure keeps the recordings (playback bit-identical before and
    /// after), copies into a larger memory (the capacities reflect the new size) and is refused
    /// with `StoredExceedsCapacity` when they would not fit.
    #[test]
    fn reconfigure_keeps_recordings_and_refuses_when_they_do_not_fit() {
        let mut p = small(6000, 2, 3);
        let x = tone(-20.0, 1.0, 6000.0);
        let _ = record_all(&mut p, &x);
        let _ = record_all(&mut p, &tone(-30.0, 0.5, 6000.0));
        let (want, _) = play_all(&mut p, 2, 100);
        assert_eq!(p.memory_capacities(), (12_000, 4));
        // Same capacity: kept.
        let mut s = *p.settings();
        s.vas.hang_ms = 500.0;
        p.reconfigure(s).unwrap();
        assert_eq!(p.memory_capacities(), (12_000, 4));
        assert_eq!(p.memory_status().count, 2);
        assert_eq!(play_all(&mut p, 2, 100).0, want);
        // Larger: re-reserved and copied.
        s.memory.capacity_s = 5;
        s.memory.max_recordings = 10;
        p.reconfigure(s).unwrap();
        assert_eq!(p.memory_capacities(), (30_000, 11));
        let st = p.memory_status();
        assert_eq!(
            (st.count, st.capacity_samples, st.slots_remaining),
            (2, 30_000, 8)
        );
        assert_eq!(play_all(&mut p, 2, 100).0, want);
        // Too small for the stored time, then for the count.
        s.memory.capacity_s = 1;
        assert_eq!(p.reconfigure(s), Err(Error::StoredExceedsCapacity));
        s.memory.capacity_s = 5;
        s.memory.max_recordings = 1;
        assert_eq!(p.reconfigure(s), Err(Error::StoredExceedsCapacity));
        assert_eq!(
            p.memory_capacities(),
            (30_000, 11),
            "a refusal changes nothing"
        );
        assert_eq!(play_all(&mut p, 2, 100).0, want);
    }

    /// 006 T033 (FR-014; US3 scenario 4): an import longer than the remaining time, or with no
    /// slot left, is `DoesNotFit`; the memory is unchanged either way.
    #[test]
    fn import_counts_against_capacity() {
        // Time: a 2 s recording into a 2 s memory, then a 1 s one does not fit.
        let mut src = small(6000, 10, 3);
        let _ = record_all(&mut src, &tone(-20.0, 2.0, 6000.0));
        let _ = record_all(&mut src, &tone(-20.0, 1.0, 6000.0));
        let mut two = std::vec![0u8; src.export_len(1).unwrap()];
        assert_eq!(src.export(1, &mut two).unwrap(), two.len());
        let mut one = std::vec![0u8; src.export_len(2).unwrap()];
        assert_eq!(src.export(2, &mut one).unwrap(), one.len());
        // 2 s + the one-sample flush = 12 001 stored; a 3 s memory keeps 5 999 after it.
        let mut p = small(48_000, 3, 3);
        assert_eq!(p.import(&two), Ok(1));
        let before = p.memory_status();
        assert_eq!(before.remaining_samples, 18_000 - 12_001);
        assert_eq!(p.import(&one), Err(Error::DoesNotFit));
        assert_eq!(p.memory_status(), before);
        assert_eq!(p.recording_length(1), Ok(12_001));
        // Slots: a 1-slot memory with one recording refuses a second import with time left.
        let mut p = small(48_000, 10, 1);
        assert_eq!(p.import(&one), Ok(1));
        let before = p.memory_status();
        assert!(before.remaining_samples > 12_001);
        assert_eq!(p.import(&one), Err(Error::DoesNotFit));
        assert_eq!(p.memory_status(), before);
        assert_eq!(p.state().state, State::Idle);
    }

    /// 006 T033 (FR-013, FR-014): an import while playing appends after the played recording
    /// and leaves the playback bit-identical to an uninterrupted one.
    #[test]
    fn import_while_playing_appends_and_does_not_disturb_playback() {
        let mut p = small(6000, 10, 4);
        let _ = record_all(&mut p, &tone(-20.0, 0.3, 6000.0));
        let _ = record_all(&mut p, &tone(-30.0, 0.2, 6000.0));
        let mut bytes = std::vec![0u8; p.export_len(2).unwrap()];
        p.export(2, &mut bytes).unwrap();
        let mut reference = p.clone();
        let (want, _) = play_all(&mut reference, 1, 100);
        p.start_playback(1).unwrap();
        let mut out = Vec::new();
        let mut buf = [0.0f32; 100];
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        assert_eq!(p.import(&bytes), Ok(3));
        assert_eq!(p.state().state, State::Playing);
        assert_eq!(p.state().current, Some(1));
        assert_eq!(p.memory_status().count, 3);
        loop {
            let info = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                break;
            }
        }
        out.truncate(out.len() - p.drain_samples() as usize);
        assert_eq!(out, want);
        // The imported copy plays like the original.
        assert_eq!(play_all(&mut p, 3, 100), play_all(&mut p, 2, 100));
    }

    /// 006 T033 (FR-013, Edge Cases): import is refused while recording; nothing changes.
    #[test]
    fn import_while_recording_refused() {
        let mut p = small(6000, 10, 3);
        let _ = record_all(&mut p, &tone(-20.0, 0.2, 6000.0));
        let mut bytes = std::vec![0u8; p.export_len(1).unwrap()];
        p.export(1, &mut bytes).unwrap();
        p.start_recording().unwrap();
        let before = p.memory_status();
        assert_eq!(p.import(&bytes), Err(Error::RecordingInProgress));
        assert_eq!(p.memory_status(), before);
        assert_eq!(p.state().state, State::Recording);
        // Export is allowed while recording (it reads a closed recording).
        let mut again = std::vec![0u8; p.export_len(1).unwrap()];
        assert_eq!(p.export(1, &mut again), Ok(bytes.len()));
        assert_eq!(again, bytes);
        p.reset();
    }

    /// 006 T033 (FR-014): export of 0, of a number above the count, and into a short buffer.
    #[test]
    fn export_of_a_bad_number() {
        let mut p = small(6000, 10, 3);
        assert_eq!(p.export_len(1), Err(Error::NoSuchRecording { number: 1 }));
        let _ = record_all(&mut p, &tone(-20.0, 0.2, 6000.0));
        assert_eq!(p.export_len(0), Err(Error::NoSuchRecording { number: 0 }));
        assert_eq!(p.export_len(2), Err(Error::NoSuchRecording { number: 2 }));
        let needed = p.export_len(1).unwrap();
        assert_eq!(needed, 52 + 4 * 1201); // form version 2, the samples encoding (008 FR-010)
        let mut buf = std::vec![0xEEu8; 10];
        assert_eq!(
            p.export(2, &mut buf),
            Err(Error::NoSuchRecording { number: 2 })
        );
        assert_eq!(p.export(1, &mut buf), Err(Error::BufferTooSmall { needed }));
        assert!(buf.iter().all(|&b| b == 0xEE));
    }
}

/// 008 T015 (spec 008 FR-004, FR-008, FR-009, FR-013, FR-014, FR-017; SC-002; research R-06):
/// the frames in the record path and the memory, the stop rule in its cases, the latencies and
/// the playback read-outs.
#[cfg(test)]
mod codec_tests {
    extern crate std;
    use super::*;
    use crate::codec::{DELAY_SAMPLES, FRAME_SAMPLES, LOOKAHEAD_SAMPLES};
    use crate::settings::{Settings, Tap, VasMode};
    use alloc::vec::Vec;
    use rr_dr60_detmath::{TAU, exp, ln, sin};

    /// A 1 kHz tone at `dbfs` for `secs` at `fs`, phase 0.
    fn tone(dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
        let amp = exp(dbfs * ln(10.0) / 20.0);
        let w = TAU * 1000.0 / fs;
        (0..(secs * fs) as usize)
            .map(|n| (amp * sin(w * n as f64)) as f32)
            .collect()
    }

    /// The codec with everything else bypassed (the harness's `codec_only`, the tap ignored by
    /// the paths), a `secs` second memory of `max` recordings.
    fn codec_only(rate: u32, secs: u32, max: u32) -> Settings {
        let mut s = crate::test_util::settings_011(rate); // 012 FR-010
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.vas.enabled = false;
        s.speech_codec_enabled = true;
        s.memory.capacity_s = secs;
        s.memory.max_recordings = max;
        s
    }

    fn kept(out: &StopOutcome) -> (u32, u64) {
        match out.result {
            StopResult::Kept { number, length } => (number, length),
            other => panic!("{other:?}"),
        }
    }

    /// FR-008: `RecordInfo.frames` counts the frames completed in the block, `frames_total`
    /// the recording's; 1234 samples at 6 kHz in blocks of 100 give 9 frames (frame k on push
    /// 120(k+1) + 59), the first in the block holding push 179.
    #[test]
    fn record_frames_total_and_block() {
        let mut p = Pipeline::new(codec_only(6000, 10, 2)).unwrap();
        let x = tone(-20.0, 1234.0 / 6000.0, 6000.0);
        assert_eq!(x.len(), 1234);
        p.start_recording().unwrap();
        let mut per_block = Vec::new();
        let mut total = 0;
        for chunk in x.chunks(100) {
            let info = p.record(chunk, &mut []).unwrap();
            per_block.push(info.frames);
            total += info.frames;
            assert_eq!(info.frames_total as usize, total);
            assert_eq!(info.stored, chunk.len());
        }
        assert_eq!(total, 9);
        assert_eq!(per_block[0], 0);
        assert_eq!(per_block[1], 1, "push 179 falls in block 100 … 199");
        assert_eq!(per_block[2], 1, "push 299");
        assert_eq!(per_block[3], 0);
        assert_eq!(per_block[4], 1, "push 419");
        assert_eq!(p.memory_status().used_samples, 9 * 120);
        p.reset();
    }

    /// FR-013: n′ = 1234 kept samples stop as 11 whole frames (1320 samples) with
    /// `silence_fed_samples` = 1320 − 1234 + 60 = 146; n′ = 1200 adds no frame
    /// (`silence_fed_samples` = 60, the look-ahead alone).
    #[test]
    fn stop_pads_to_whole_frames_and_a_boundary_adds_no_frame() {
        for (n, want_frames, want_fed) in [
            (1234usize, 11u64, 146u64),
            (1200, 10, 60),
            (1, 1, 179),
            (120, 1, 60),
            (121, 2, 179),
        ] {
            let mut p = Pipeline::new(codec_only(6000, 10, 2)).unwrap();
            p.start_recording().unwrap();
            let x = tone(-20.0, n as f64 / 6000.0, 6000.0);
            assert_eq!(x.len(), n);
            let info = p.record(&x, &mut []).unwrap();
            assert_eq!(info.stored, n);
            let out = p.stop_recording(&mut []).unwrap();
            let (number, length) = kept(&out);
            assert_eq!(number, 1);
            assert_eq!(length, want_frames * u64::from(FRAME_SAMPLES), "n′ = {n}");
            assert_eq!(out.silence_fed_samples, want_fed, "n′ = {n}");
            assert_eq!(
                out.silence_fed_samples,
                want_frames * 120 - n as u64 + u64::from(LOOKAHEAD_SAMPLES)
            );
            assert_eq!(out.flush.frames_total, want_frames);
            assert_eq!(info.frames as u64 + out.flush.frames as u64, want_frames);
            assert_eq!(
                out.flush.stored as u64, want_fed,
                "at 6 kHz every fed zero is kept"
            );
            assert_eq!(p.recording_length(1), Ok(length));
            assert_eq!(p.memory_status().used_samples, length);
        }
    }

    /// FR-013 (plan review Q3a): the shortest recording. One host sample at 48 kHz yields one
    /// device sample at once (the conversion emits on its first input) and the VAS starts open,
    /// so "fed but nothing kept" cannot arise: the stop pads that single sample to one frame
    /// (`silence_fed_samples` 179); an empty feed is discarded as in 006.
    #[test]
    fn one_host_sample_becomes_one_frame_and_an_empty_feed_is_discarded() {
        let s = codec_only(48_000, 10, 2);
        let mut p = Pipeline::new(s).unwrap();
        p.start_recording().unwrap();
        let info = p.record(&[0.25f32], &mut []).unwrap();
        assert_eq!((info.stored, info.frames), (1, 0));
        let out = p.stop_recording(&mut []).unwrap();
        let (_, length) = kept(&out);
        assert_eq!(length, 120);
        // The kept total is one frame plus the look-ahead: 180. n′ is 30: the sample itself
        // (a device sample at once) plus the 29 more the 006 flush (242 host samples at 8 per
        // device sample) yields, so the silence fed is 120·1 − 30 + 60 = 150.
        assert_eq!(
            info.stored as u64 + out.flush.stored as u64,
            120 + u64::from(LOOKAHEAD_SAMPLES)
        );
        assert_eq!(out.silence_fed_samples, 150);
        // The same at 44.1 kHz (a non-integer ratio): one device sample from one host sample,
        // and the kept total 180 again.
        let mut q = Pipeline::new(codec_only(44_100, 10, 2)).unwrap();
        q.start_recording().unwrap();
        let info44 = q.record(&[0.25f32], &mut []).unwrap();
        assert_eq!(
            (info44.stored, info44.frames),
            (1, 0),
            "44.1 kHz: a device sample at once"
        );
        let out44 = q.stop_recording(&mut []).unwrap();
        assert_eq!(kept(&out44).1, 120);
        assert_eq!(info44.stored as u64 + out44.flush.stored as u64, 180);
        assert_eq!(out.flush.frames_total, 1);
        assert_eq!(p.memory_status().count, 1);
        p.start_recording().unwrap();
        let info = p.record(&[], &mut []).unwrap();
        assert_eq!(info, RecordInfo::default());
        let out = p.stop_recording(&mut []).unwrap();
        assert_eq!(out.result, StopResult::DiscardedEmpty);
        assert_eq!(out.silence_fed_samples, 0);
        assert_eq!(p.memory_status().count, 1);
        assert_eq!(p.state().state, State::Idle);
    }

    /// FR-013 (plan review Q3b): a VAS paused at the stop in drop mode: the gate is held open
    /// for the padding, the recording ends on a whole frame, and the forced resume's splice
    /// event is in the flush report with the dropped length.
    #[test]
    fn stop_in_drop_mode_with_vas_paused_completes_the_frame_and_records_the_splice() {
        let mut s = codec_only(6000, 10, 2);
        s.vas.enabled = true;
        s.vas.hang_ms = 50.0; // 300 device samples
        let mut p = Pipeline::new(s).unwrap();
        let x = [tone(-30.0, 1.0, 6000.0), std::vec![0.0f32; 6000]].concat();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 4];
        let info = p.record(&x, &mut ev).unwrap();
        assert!(info.paused);
        assert_eq!(info.events, 0);
        assert_eq!(info.stored, 6300, "the burst and the hang time");
        let out = p.stop_recording(&mut ev).unwrap();
        let (_, length) = kept(&out);
        // n′ = 6300 → 53 frames = 6360; silence fed 6360 − 6300 + 60.
        assert_eq!(length, 53 * 120);
        assert_eq!(out.silence_fed_samples, 120);
        assert_eq!(out.flush.events, 1, "{out:?}");
        assert_eq!(ev[0].output_position, 6300, "the forced resume's position");
        assert_eq!(ev[0].input_length, 5700, "the dropped stretch");
        assert_eq!(out.flush.stored, 120);
        assert_eq!(out.flush.frames, 1);
        assert_eq!(p.recording_length(1), Ok(6360));
        // A second recording starts from the reset state: the gate is not held open, so
        // silence is kept for the hang time (300 samples) and then dropped.
        p.start_recording().unwrap();
        let info = p.record(&[0.0f32; 1000], &mut ev).unwrap();
        assert_eq!(info.stored, 300, "fresh: unpaused for the hang time only");
        assert!(info.paused);
        p.reset();
        // At 48 kHz with stage 4 on the pre-encoder flush is 242 host samples: the gate is
        // held open through it too, so its device samples are kept (and counted in n′) where
        // 006 would have dropped them, the forced resume falls on the flush's first device
        // sample (position = the kept count at the stop), and the length is a whole frame.
        let mut s = codec_only(48_000, 10, 2);
        s.vas.enabled = true;
        s.vas.hang_ms = 50.0;
        s.record_stage_enabled = true;
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.record_latency_samples(), 242 + 1440);
        let x = [tone(-30.0, 1.0, 48_000.0), std::vec![0.0f32; 48_000]].concat();
        p.start_recording().unwrap();
        let info = p.record(&x, &mut ev).unwrap();
        assert!(info.paused);
        assert_eq!(info.events, 0);
        let out = p.stop_recording(&mut ev).unwrap();
        let (_, length) = kept(&out);
        let kept_total = info.stored as u64 + out.flush.stored as u64;
        let n_prime = kept_total - out.silence_fed_samples;
        assert!(
            n_prime > info.stored as u64,
            "the flush's samples are kept: n′ {n_prime}"
        );
        assert_eq!(length, n_prime.div_ceil(120) * 120, "a whole frame");
        assert_eq!(out.silence_fed_samples, length - n_prime + 60);
        assert_eq!(kept_total, length + 60);
        assert_eq!(out.flush.events, 1, "{out:?}");
        assert_eq!(
            ev[0].output_position, info.stored as u64,
            "the forced resume at the stop"
        );
        assert!(ev[0].input_length > 5000, "{:?}", ev[0]);
        assert_eq!(p.recording_length(1), Ok(length));
    }

    /// FR-013 (plan review Q3c): in mute mode nothing is dropped, so nothing is forced: the
    /// padding is stored as muted zeros and the open region closes with 006's synthesized
    /// event at the recording's end.
    #[test]
    fn stop_in_mute_mode_needs_no_forcing() {
        let mut s = codec_only(6000, 10, 2);
        s.vas.enabled = true;
        s.vas.mode = VasMode::Mute;
        s.vas.hang_ms = 50.0;
        let mut p = Pipeline::new(s).unwrap();
        let x = [tone(-30.0, 1.0, 6000.0), std::vec![0.0f32; 6000]].concat();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 4];
        let info = p.record(&x, &mut ev).unwrap();
        assert_eq!(info.stored, 12_000, "mute mode keeps every sample");
        assert_eq!(info.events, 0, "the region is open");
        assert_eq!(info.frames, 99, "frame 99 waits for its look-ahead");
        let out = p.stop_recording(&mut ev).unwrap();
        let (_, length) = kept(&out);
        assert_eq!(
            length, 12_000,
            "100 whole frames: the look-ahead completes the last"
        );
        assert_eq!(out.silence_fed_samples, 60);
        assert_eq!(out.flush.frames, 1);
        assert_eq!(out.flush.frames_total, 100);
        assert_eq!(out.flush.events, 1);
        assert_eq!(ev[0].output_position, 6300);
        assert_eq!(ev[0].input_length, 12_060 - 6300, "through the padding");
    }

    /// FR-021 (plan review P9): with the codec bypassed the stop is spec 006's: the flush
    /// alone, no padding, `silence_fed_samples` 0 and no frames.
    #[test]
    fn stop_bypassed_follows_006_with_no_padding() {
        let mut s = codec_only(48_000, 10, 2);
        s.speech_codec_enabled = false;
        s.vas.enabled = true;
        let mut p = Pipeline::new(s).unwrap();
        let x = [tone(-30.0, 0.5, 48_000.0), std::vec![0.0f32; 96_000]].concat();
        p.start_recording().unwrap();
        let info = p.record(&x, &mut []).unwrap();
        assert_eq!(info.frames, 0);
        let out = p.stop_recording(&mut []).unwrap();
        assert_eq!(out.silence_fed_samples, 0);
        assert_eq!((out.flush.frames, out.flush.frames_total), (0, 0));
        let (_, length) = kept(&out);
        assert_eq!(
            length,
            u64::from(info.stored as u32) + out.flush.stored as u64
        );
        assert_eq!(p.encoding(), Encoding::Samples);
    }

    /// FR-013 (A-039): a 2 s capacity holds exactly 100 frames; the recording stops on the
    /// frame boundary with the remaining time exactly 0, the samples after the last stored
    /// frame are not kept, and a stop afterwards is `NotRecording`.
    #[test]
    fn memory_full_stops_on_a_frame_boundary() {
        let mut p = Pipeline::new(codec_only(6000, 2, 2)).unwrap();
        assert_eq!(p.memory_status().capacity_samples, 12_000);
        p.start_recording().unwrap();
        let x = tone(-20.0, 2.5, 6000.0);
        let info = p.record(&x, &mut []).unwrap();
        assert_eq!(info.stopped, Some(StopReason::MemoryFull));
        assert_eq!(info.frames, 100);
        assert_eq!(info.frames_total, 100);
        assert_eq!(
            info.stored,
            100 * 120 + 60,
            "the kept samples up to the 100th frame"
        );
        assert_eq!(p.state().state, State::Idle);
        assert_eq!(p.recording_length(1), Ok(12_000));
        let st = p.memory_status();
        assert_eq!(
            (st.used_samples, st.remaining_samples, st.count),
            (12_000, 0, 1)
        );
        assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
        assert_eq!(p.start_recording(), Err(Error::MemoryFull));
    }

    /// FR-014: the VAS events of the same stimulus are identical with the codec on and off
    /// (positions are post-VAS device samples; the codec sits after the gate).
    #[test]
    fn vas_event_positions_unchanged_by_the_codec() {
        let mut on = codec_only(48_000, 10, 2);
        on.vas.enabled = true;
        on.vas.hang_ms = 500.0;
        let mut off = on;
        off.speech_codec_enabled = false;
        let x = [
            tone(-30.0, 1.0, 48_000.0),
            std::vec![0.0f32; 2 * 48_000],
            tone(-30.0, 1.0, 48_000.0),
        ]
        .concat();
        let run = |s: Settings| {
            let mut p = Pipeline::new(s).unwrap();
            p.start_recording().unwrap();
            let mut ev = [VasEvent::default(); 4];
            let info = p.record(&x, &mut ev).unwrap();
            (info.events, ev, info.paused)
        };
        let a = run(on);
        let b = run(off);
        assert_eq!(a, b);
        assert_eq!(a.0, 1);
    }

    /// FR-004, SC-002 (data-model.md › Per-rate figures): the record path's latency grows by
    /// 180 device samples' worth of host samples, the playback path's not at all, the one-pass
    /// figures are 181 / 321 / 642 / 1767 / 1923 / 3533 / 3846 and the sum rule holds.
    #[test]
    fn latencies_include_the_codec_delay() {
        let rates = [6000u32, 8000, 16_000, 44_100, 48_000, 88_200, 96_000];
        let one_pass_008 = [181u32, 321, 642, 1767, 1923, 3533, 3846];
        for (i, &rate) in rates.iter().enumerate() {
            let on = Pipeline::new(Settings::new(rate)).unwrap();
            let mut s = Settings::new(rate);
            s.speech_codec_enabled = false;
            let off = Pipeline::new(s).unwrap();
            let delay = DELAY_SAMPLES * rate / 6000;
            assert_eq!(DELAY_SAMPLES * rate % 6000, 0, "{rate} Hz: a whole number");
            assert_eq!(
                on.record_latency_samples(),
                off.record_latency_samples() + delay,
                "{rate} Hz record"
            );
            assert_eq!(
                on.playback_latency_samples(),
                off.playback_latency_samples(),
                "{rate} Hz playback"
            );
            assert_eq!(on.latency_samples(), one_pass_008[i], "{rate} Hz one-pass");
            assert_eq!(on.latency_samples(), off.latency_samples() + delay);
            let sum = on.record_latency_samples() + on.playback_latency_samples();
            assert!(
                sum.abs_diff(on.latency_samples()) <= 1,
                "{rate} Hz: the sum rule"
            );
            assert_eq!(on.drain_samples(), off.drain_samples());
            assert_eq!(on.speech_codec_info().delay_samples, DELAY_SAMPLES);
        }
    }

    /// FR-008: `PlayInfo.frames` counts the frames decoded in the call and `frame_index` the
    /// frame being played at its end, held at the last frame during the drain; the playback
    /// length is ⌈120·F·m/l⌉ plus the drain.
    #[test]
    fn play_reports_frames_and_index() {
        for rate in [6000u32, 48_000] {
            let fs = f64::from(rate);
            let mut p = Pipeline::new(codec_only(rate, 10, 2)).unwrap();
            p.start_recording().unwrap();
            let _ = p.record(&tone(-20.0, 0.21, fs), &mut []).unwrap();
            let out = p.stop_recording(&mut []).unwrap();
            let (number, length) = kept(&out);
            let frames = length / 120;
            assert!(frames >= 11, "{rate} Hz: {frames} frames");
            let (l, m) = match rate {
                6000 => (1u64, 1u64),
                _ => (1, 8),
            };
            p.start_playback(number).unwrap();
            let mut buf = std::vec![0.0f32; 100];
            let (mut total_frames, mut produced) = (0usize, 0u64);
            let last_index = loop {
                let info = p.play(&mut buf).unwrap();
                total_frames += info.frames;
                produced += info.produced as u64;
                // The frame holding the last device sample read so far, or the last frame.
                let device_read = (produced * l).div_ceil(m);
                let expect = (device_read.saturating_sub(1) / 120).min(frames - 1);
                assert_eq!(
                    info.frame_index, expect,
                    "{rate} Hz after {produced} host samples"
                );
                if info.finished {
                    break info.frame_index;
                }
            };
            assert_eq!(total_frames as u64, frames, "{rate} Hz");
            assert_eq!(last_index, frames - 1, "{rate} Hz: held at the last frame");
            assert_eq!(
                produced,
                (length * m).div_ceil(l) + u64::from(p.drain_samples()),
                "{rate} Hz"
            );
            // A second playback decodes the same frames again (the decoder reset, FR-018).
            p.start_playback(number).unwrap();
            let mut again = std::vec![0.0f32; 1 << 16];
            let info = p.play(&mut again).unwrap();
            assert!(info.finished);
            assert_eq!(info.frames as u64, frames);
        }
    }

    /// FR-012, FR-019 (coach Q11): a reconfigure that keeps the codec setting follows 006's
    /// rules (a tap change with recordings present succeeds); one that changes it is refused
    /// with recordings present and re-reserves the store when the memory is empty.
    #[test]
    fn reconfigure_other_than_the_codec_follows_006() {
        let s = codec_only(6000, 2, 3);
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.encoding(), Encoding::Frames);
        assert_eq!(p.footprint_bytes(), 100 * 10 + 4 * 16);
        p.start_recording().unwrap();
        let _ = p.record(&tone(-20.0, 0.5, 6000.0), &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        let before = p.memory_status();
        let mut tap = s;
        tap.tap = Tap::AfterVas;
        p.reconfigure(tap).unwrap();
        assert_eq!(p.memory_status(), before);
        assert_eq!(p.encoding(), Encoding::Frames);
        let mut off = s;
        off.speech_codec_enabled = false;
        assert_eq!(p.reconfigure(off), Err(Error::StoredExceedsCapacity));
        assert_eq!(p.memory_status(), before, "a refusal changes nothing");
        assert_eq!(p.settings().tap, Tap::AfterVas);
        p.erase_all().unwrap();
        p.reconfigure(off).unwrap();
        assert_eq!(p.encoding(), Encoding::Samples);
        assert_eq!(p.footprint_bytes(), 12_000 * 4 + 4 * 16);
        p.reconfigure(s).unwrap();
        assert_eq!(p.encoding(), Encoding::Frames);
        assert_eq!(p.footprint_bytes(), 100 * 10 + 4 * 16);
    }

    /// 008 T025 (FR-012; SC-007): at the default capacity a codec change on an empty memory
    /// re-reserves the store for the other encoding: 1 801 600 bytes of frames (180 000 × 10 +
    /// 100 spans × 16) against 86 401 600 of samples, and back.
    #[test]
    fn reconfigure_codec_with_empty_memory_rereserves() {
        let on = Settings::new(6000);
        let mut off = on;
        off.speech_codec_enabled = false;
        let mut p = Pipeline::new(on).unwrap();
        assert_eq!(p.footprint_bytes(), 1_801_600);
        assert_eq!(p.memory_status().capacity_samples, 21_600_000);
        p.reconfigure(off).unwrap();
        assert_eq!(p.encoding(), Encoding::Samples);
        assert_eq!(p.footprint_bytes(), 86_401_600);
        assert_eq!(p.memory_status().capacity_samples, 21_600_000);
        p.reconfigure(on).unwrap();
        assert_eq!(p.encoding(), Encoding::Frames);
        assert_eq!(p.footprint_bytes(), 1_801_600);
    }

    /// Plays recording `number` to the end in blocks of `block`; the whole output.
    fn play_to_end(p: &mut Pipeline, number: u32, block: usize) -> Vec<f32> {
        p.start_playback(number).unwrap();
        let mut out = Vec::new();
        let mut buf = std::vec![0.0f32; block];
        loop {
            let info = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                return out;
            }
        }
    }

    /// Records `x` in one block, stops, and returns the recording's exported bytes.
    fn record_export(p: &mut Pipeline, x: &[f32]) -> Vec<u8> {
        p.start_recording().unwrap();
        assert!(p.record(x, &mut []).unwrap().stopped.is_none());
        let (number, _) = kept(&p.stop_recording(&mut []).unwrap());
        let mut bytes = std::vec![0u8; p.export_len(number).unwrap()];
        assert_eq!(p.export(number, &mut bytes).unwrap(), bytes.len());
        bytes
    }

    /// 008 T025 (FR-012): a samples form into a memory of frames and a frames form into a
    /// memory of samples are both `EncodingDiffers`, checked before the fit (a form far larger
    /// than the memory still reports the encoding), with the memory unchanged.
    #[test]
    fn import_other_encoding_refused_encoding_differs() {
        let on = codec_only(6000, 10, 3);
        let mut off = on;
        off.speech_codec_enabled = false;
        let mut frames_engine = Pipeline::new(on).unwrap();
        let mut samples_engine = Pipeline::new(off).unwrap();
        let x = tone(-20.0, 2.0, 6000.0);
        let frames = record_export(&mut frames_engine, &x);
        let samples = record_export(&mut samples_engine, &x);
        assert_eq!(u16::from_le_bytes([frames[22], frames[23]]), 1);
        assert_eq!(u16::from_le_bytes([samples[22], samples[23]]), 2);
        let before = frames_engine.memory_status();
        assert_eq!(frames_engine.import(&samples), Err(Error::EncodingDiffers));
        assert_eq!(frames_engine.memory_status(), before);
        let before = samples_engine.memory_status();
        assert_eq!(samples_engine.import(&frames), Err(Error::EncodingDiffers));
        assert_eq!(samples_engine.memory_status(), before);
        // Encoding before fit: a 2 s samples form into a 1 s frames memory.
        let mut tiny = Pipeline::new(codec_only(6000, 1, 1)).unwrap();
        assert_eq!(tiny.import(&samples), Err(Error::EncodingDiffers));
        // The same bytes import into an engine of their own encoding.
        assert_eq!(frames_engine.import(&frames), Ok(2));
        assert_eq!(samples_engine.import(&samples), Ok(2));
    }

    /// 008 T025 (FR-013): an import of frames counts against the capacity in frames: 100
    /// frames into a 1 s memory (50 frames) is `DoesNotFit`, as is a second recording into one
    /// slot; the memory is unchanged either way.
    #[test]
    fn import_frames_counts_against_capacity_in_frames() {
        let mut src = Pipeline::new(codec_only(6000, 10, 3)).unwrap();
        let two_s = record_export(&mut src, &tone(-20.0, 2.0, 6000.0));
        assert_eq!(src.recording_length(1), Ok(100 * 120));
        assert_eq!(two_s.len(), 52 + 10 * 100);
        let mut one_s = Pipeline::new(codec_only(48_000, 1, 3)).unwrap();
        let before = one_s.memory_status();
        assert_eq!(before.remaining_samples, 6000);
        assert_eq!(one_s.import(&two_s), Err(Error::DoesNotFit));
        assert_eq!(one_s.memory_status(), before);
        // 100 frames into a 3 s memory with 50 frames taken: 100 > 150 − 50 is false, so it
        // fits exactly; a third does not.
        let mut three_s = Pipeline::new(codec_only(48_000, 3, 3)).unwrap();
        let one = record_export(&mut src, &tone(-20.0, 1.0, 6000.0));
        assert_eq!(three_s.import(&one), Ok(1));
        assert_eq!(three_s.import(&two_s), Ok(2));
        assert_eq!(three_s.memory_status().remaining_samples, 0);
        let before = three_s.memory_status();
        assert_eq!(three_s.import(&one), Err(Error::DoesNotFit));
        assert_eq!(three_s.memory_status(), before);
        // Slots.
        let mut slot = Pipeline::new(codec_only(44_100, 10, 1)).unwrap();
        assert_eq!(slot.import(&one), Ok(1));
        let before = slot.memory_status();
        assert_eq!(slot.import(&one), Err(Error::DoesNotFit));
        assert_eq!(slot.memory_status(), before);
        assert_eq!(slot.recording_length(1), Ok(50 * 120));
    }

    /// 008 T025 (FR-010, SC-009): a frames export imports into an engine at another host rate
    /// with the same length and bit-identical playback, and re-exports to the same bytes.
    #[test]
    fn frames_round_trip_across_rates() {
        let mut src = Pipeline::new(codec_only(48_000, 10, 3)).unwrap();
        let bytes = record_export(&mut src, &tone(-20.0, 1.0, 48_000.0));
        let again = {
            let mut b = std::vec![0u8; src.export_len(1).unwrap()];
            src.export(1, &mut b).unwrap();
            b
        };
        assert_eq!(again, bytes, "export is identical every time");
        let length = src.recording_length(1).unwrap();
        assert_eq!(length % 120, 0);
        assert_eq!(bytes.len(), 52 + 10 * (length / 120) as usize);
        let mut dst = Pipeline::new(codec_only(44_100, 10, 3)).unwrap();
        assert_eq!(dst.import(&bytes), Ok(1));
        assert_eq!(dst.recording_length(1), Ok(length));
        src.reconfigure(codec_only(44_100, 10, 3)).unwrap();
        let want = play_to_end(&mut src, 1, 997);
        let got = play_to_end(&mut dst, 1, 997);
        assert_eq!(want.len(), got.len());
        assert!(
            want.iter()
                .zip(&got)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        let mut re = std::vec![0u8; dst.export_len(1).unwrap()];
        dst.export(1, &mut re).unwrap();
        assert_eq!(re, bytes);
    }

    /// 008 T025 (research R-07; plan review P11): at "memory full" the last frame is appended
    /// when the kept count reaches its boundary plus the look-ahead, so a VAS event in the
    /// stopping block may name a position past the stored length, by at most the look-ahead
    /// (60 device samples); documented, not clamped. A 1 s memory in drop mode with the tone
    /// ending so that the gate pauses between kept 6000 and 6060.
    #[test]
    fn events_may_exceed_the_length_at_memory_full() {
        let mut s = codec_only(6000, 1, 2);
        s.vas.enabled = true;
        s.vas.hang_ms = 50.0; // the shortest hang: 300 device samples
        let mut past_the_length = 0;
        for tone_samples in (5600..5800).step_by(4) {
            let mut p = Pipeline::new(s).unwrap();
            p.start_recording().unwrap();
            let mut x = tone(-20.0, tone_samples as f64 / 6000.0, 6000.0);
            x.resize(7000, 0.0);
            let mut ev = [VasEvent::default(); 8];
            let info = p.record(&x, &mut ev).unwrap();
            let mut events: Vec<VasEvent> = ev[..info.events].to_vec();
            if info.stopped.is_none() {
                // The gate paused before the 50th frame completed; the stop feed (gate held
                // open) completes it and fills the memory.
                let mut sev = [VasEvent::default(); 8];
                let out = p.stop_recording(&mut sev).unwrap();
                assert_eq!(out.flush.stopped, Some(StopReason::MemoryFull));
                events.extend_from_slice(&sev[..out.flush.events]);
            } else {
                assert_eq!(info.stopped, Some(StopReason::MemoryFull));
            }
            let length = p.recording_length(1).unwrap();
            assert_eq!(length, 6000, "{tone_samples}");
            for e in &events {
                assert!(
                    e.output_position <= length + u64::from(LOOKAHEAD_SAMPLES),
                    "{tone_samples}: event at {} past 6000 + 60",
                    e.output_position
                );
                if e.output_position > length {
                    past_the_length += 1;
                }
            }
        }
        assert!(
            past_the_length > 0,
            "no tone length produced an event past the length"
        );
    }
}

/// 012 T024 (spec 012 FR-009; research R-05's table): the recording counter and the hiss
/// streams it keys. The configuration is stage 1 alone at 6 kHz (the identity rate: no
/// conversion, every other stage bypassed), so a recording stores the capsule's output sample
/// for sample and the oracle is the stage itself keyed to `(seed, counter)`.
#[cfg(test)]
mod counter_tests {
    extern crate std;
    use super::*;
    use crate::settings::{MicSettings, OutputSettings, Settings, Tap};
    use crate::stages::mic::MicCapsule;
    use alloc::vec::Vec;

    fn capsule_settings(seed: u64) -> Settings {
        let mut s = Settings::new(6000);
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
        s.adc_clamp_enabled = false;
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.speech_codec_enabled = false;
        s.output = OutputSettings::BYPASSED;
        s.mic = MicSettings::DEVICE;
        s.mic.amp_enabled = false;
        s.tap = Tap::AfterMicCapsule;
        s.seed = seed;
        s.memory.capacity_s = 1;
        s.memory.max_recordings = 4;
        s
    }

    /// The capsule's stream for `(seed, counter)`: `n` samples on zero input.
    fn oracle(s: &Settings, counter: u64, n: usize) -> Vec<f32> {
        let mut c = MicCapsule::new(&s.mic, s.seed, s.host_rate_hz).unwrap();
        c.rekey(counter);
        (0..n).map(|_| c.process(0.0) as f32).collect()
    }

    fn one_pass_n(p: &mut Pipeline, n: usize) -> Vec<f32> {
        let x = std::vec![0.0f32; n];
        let mut y = std::vec![0.0f32; n];
        assert_eq!(p.process(&x, &mut y).unwrap().produced, n);
        y
    }

    /// Records `n` zeros and stops.
    fn record_n(p: &mut Pipeline, n: usize) -> StopOutcome {
        p.start_recording().unwrap();
        let _ = p.record(&std::vec![0.0f32; n], &mut []).unwrap();
        p.stop_recording(&mut []).unwrap()
    }

    fn stored(p: &Pipeline, number: u32) -> Vec<f32> {
        p.memory.samples(p.memory.span(number).unwrap()).to_vec()
    }

    /// R-05: construction — the counter is 0 and the one-pass is keyed `(seed, 0)`.
    #[test]
    fn construction_keys_counter_zero() {
        let s = capsule_settings(11);
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.transport.counter, 0);
        assert_eq!(one_pass_n(&mut p, 300), oracle(&s, 0, 300));
    }

    /// R-05: a successful start keys the recorder to `(seed, counter)`, then the counter
    /// advances; recording k carries stream k from its beginning.
    #[test]
    fn start_rekeys_then_increments() {
        let s = capsule_settings(3);
        let mut p = Pipeline::new(s).unwrap();
        let _ = record_n(&mut p, 200);
        assert_eq!(p.transport.counter, 1);
        assert_eq!(stored(&p, 1), oracle(&s, 0, 200));
        let _ = record_n(&mut p, 150);
        assert_eq!(p.transport.counter, 2);
        assert_eq!(stored(&p, 2), oracle(&s, 1, 150));
    }

    /// R-05: `reset`, idle or mid-recording, keeps the counter and keys the one-pass to its
    /// current value (the next recording's stream).
    #[test]
    fn reset_keeps_the_counter_and_rekeys_the_one_pass() {
        let s = capsule_settings(5);
        let mut p = Pipeline::new(s).unwrap();
        let _ = record_n(&mut p, 100);
        let _ = record_n(&mut p, 100);
        let _ = one_pass_n(&mut p, 77);
        p.reset();
        assert_eq!(p.transport.counter, 2);
        assert_eq!(one_pass_n(&mut p, 100), oracle(&s, 2, 100));
        p.start_recording().unwrap();
        let _ = p.record(&[0.0; 10], &mut []).unwrap();
        p.reset(); // the recording in progress is discarded; its stream was consumed
        assert_eq!(p.transport.counter, 3);
        assert_eq!(p.memory_status().count, 2);
        assert_eq!(one_pass_n(&mut p, 100), oracle(&s, 3, 100));
        let _ = record_n(&mut p, 100);
        assert_eq!(stored(&p, 3), oracle(&s, 3, 100));
    }

    /// R-05: an accepted `reconfigure` keeps the counter and keys the one-pass to it (with the
    /// new seed); a refused one changes nothing, so the one-pass stream continues.
    #[test]
    fn reconfigure_keeps_the_counter() {
        let s = capsule_settings(1);
        let mut p = Pipeline::new(s).unwrap();
        let _ = record_n(&mut p, 100);
        let t = capsule_settings(9);
        p.reconfigure(t).unwrap();
        assert_eq!(p.transport.counter, 1);
        assert_eq!(one_pass_n(&mut p, 100), oracle(&t, 1, 100));
        // Refused: an invalid offset. The pipeline is unchanged, mid-stream.
        let mut bad = t;
        bad.mic.hiss_offset_db = 13.0;
        assert!(p.reconfigure(bad).is_err());
        assert_eq!(p.transport.counter, 1);
        assert_eq!(one_pass_n(&mut p, 100), oracle(&t, 1, 200)[100..]);
        let _ = record_n(&mut p, 50);
        assert_eq!(stored(&p, 2), oracle(&t, 1, 50));
        assert_eq!(p.transport.counter, 2);
    }

    /// R-05: a refused start (recording or playback in progress, memory full, no slots) does
    /// not advance the counter; a memory-full self-stop does not either.
    #[test]
    fn refused_starts_and_the_self_stop_do_not_advance() {
        let s = capsule_settings(2);
        let mut p = Pipeline::new(s).unwrap();
        p.start_recording().unwrap();
        assert_eq!(p.start_recording(), Err(Error::RecordingInProgress));
        assert_eq!(p.transport.counter, 1);
        let _ = p.stop_recording(&mut []).unwrap(); // empty: discarded
        let _ = record_n(&mut p, 100);
        assert_eq!(p.transport.counter, 2);
        p.start_playback(1).unwrap();
        assert_eq!(p.start_recording(), Err(Error::PlaybackInProgress));
        assert_eq!(p.transport.counter, 2);
        p.stop_playback().unwrap();
        // Memory full: 1 s = 6000 samples at 6 kHz; this recording stops by itself.
        p.start_recording().unwrap();
        let info = p.record(&std::vec![0.0f32; 7000], &mut []).unwrap();
        assert_eq!(info.stopped, Some(StopReason::MemoryFull));
        assert_eq!(p.transport.counter, 3);
        assert_eq!(p.start_recording(), Err(Error::MemoryFull));
        assert_eq!(p.transport.counter, 3);
        // No slots: one slot, used.
        let mut one = capsule_settings(2);
        one.memory.max_recordings = 1;
        let mut q = Pipeline::new(one).unwrap();
        let _ = record_n(&mut q, 10);
        assert_eq!(q.start_recording(), Err(Error::NoRecordingSlots));
        assert_eq!(q.transport.counter, 1);
    }

    /// R-05: an empty recording discarded at the stop has consumed its stream.
    #[test]
    fn an_empty_recording_consumes_its_stream() {
        let s = capsule_settings(4);
        let mut p = Pipeline::new(s).unwrap();
        let out = record_n(&mut p, 0);
        assert_eq!(out.result, StopResult::DiscardedEmpty);
        assert_eq!(p.transport.counter, 1);
        let _ = record_n(&mut p, 120);
        assert_eq!(stored(&p, 1), oracle(&s, 1, 120));
    }

    /// R-05: erase, export, import, playback and one-pass calls leave the counter unchanged,
    /// and the one-pass stream continues through them (and through recordings).
    #[test]
    fn other_calls_leave_the_counter_and_the_one_pass_stream_continues() {
        let s = capsule_settings(6);
        let mut p = Pipeline::new(s).unwrap();
        let want = oracle(&s, 0, 150);
        assert_eq!(one_pass_n(&mut p, 50), want[..50]);
        let _ = record_n(&mut p, 100);
        assert_eq!(one_pass_n(&mut p, 50), want[50..100]);
        let mut bytes = std::vec![0u8; p.export_len(1).unwrap()];
        p.export(1, &mut bytes).unwrap();
        assert_eq!(p.import(&bytes), Ok(2));
        p.start_playback(1).unwrap();
        let mut buf = [0.0f32; 64];
        while !p.play(&mut buf).unwrap().finished {}
        p.erase(2).unwrap();
        assert_eq!(p.transport.counter, 1);
        assert_eq!(one_pass_n(&mut p, 50), want[100..150]);
    }

    /// R-05: a zero-length call advances nothing.
    #[test]
    fn a_zero_length_call_advances_nothing() {
        let s = capsule_settings(8);
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.process(&[], &mut []).unwrap().produced, 0);
        assert_eq!(one_pass_n(&mut p, 40), oracle(&s, 0, 40));
        p.start_recording().unwrap();
        let _ = p.record(&[], &mut []).unwrap();
        let _ = p.record(&[0.0; 40], &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        assert_eq!(stored(&p, 1), oracle(&s, 0, 40));
        assert_eq!(p.transport.counter, 1);
    }
}

/// 020 T004, T006, T007 (spec 020 D5, D9, FR-004, FR-007; research R-05, R-07): the slot
/// block, the counter constructor and the stop fields.
#[cfg(test)]
mod record_block_tests {
    extern crate std;
    use super::*;
    use crate::changes::{STREAM_ONE_PASS, STREAM_PLAYBACK, STREAM_RECORD, Stream};
    use crate::record::{StopFields, StopHow};
    use crate::settings::Settings;
    use alloc::vec::Vec;

    const KNEE: u32 = 10; // the control manifest's `agc.knee_dbfs`

    /// The default record path at 6 kHz with the codec off and a small memory.
    fn settings(seed: u64) -> Settings {
        let mut s = Settings::new(6000);
        s.speech_codec_enabled = false;
        s.seed = seed;
        s.vas.hang_ms = 100.0; // a short hang: the stimulus's silences pause the gate
        s.memory.capacity_s = 20;
        s.memory.max_recordings = 8;
        s
    }

    /// A burst that opens the gate, then silence long enough to pause it.
    fn stimulus(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                if i < 2400 || (i % 6000) < 1200 {
                    0.5 * (i as f32 * 0.7).sin()
                } else {
                    0.0
                }
            })
            .collect()
    }

    fn export(p: &Pipeline, number: u32) -> Vec<u8> {
        let mut v = std::vec![0u8; p.export_len(number).unwrap()];
        p.export(number, &mut v).unwrap();
        v
    }

    fn record_one(p: &mut Pipeline, x: &[f32]) -> (RecordInfo, StopOutcome) {
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 64];
        let info = p.record(x, &mut ev).unwrap();
        let out = p.stop_recording(&mut ev).unwrap();
        (info, out)
    }

    fn one_pass(p: &mut Pipeline, n: usize) -> Vec<f32> {
        let x = stimulus(n);
        let mut y = std::vec![0.0f32; n];
        let _ = p.process(&x, &mut y).unwrap();
        y
    }

    /// T004: a fresh engine at counter c equals a dirty engine that recorded c times (a landed
    /// knee change, a pause at the end of the last recording, a burst in its last 100 ms, a
    /// one-pass run between); the one-pass after the original's `reset`.
    #[test]
    fn fresh_at_counter_equals_dirty_engine() {
        let x = stimulus(9000);
        let mut dirty = Pipeline::new(settings(77)).unwrap();
        for i in 0..3u64 {
            if i == 1 {
                dirty.schedule_change(Stream::Record, 100, KNEE, -4.0, 5).unwrap();
            }
            let mut tail = x.clone();
            tail.extend((0..600).map(|k| 0.9 * (k as f32 * 1.3).sin())); // a burst at the end
            tail.extend(core::iter::repeat_n(0.0, 3000)); // then a pause
            let _ = record_one(&mut dirty, &tail);
            let _ = one_pass(&mut dirty, 1500);
        }
        assert_eq!(dirty.recording_counter(), 3);
        // The landed knee stays in the settings: the fresh engine must start from it.
        let mut s = *dirty.settings();
        s.memory = settings(77).memory;
        let mut fresh = Pipeline::new_at_counter(s, 3).unwrap();
        assert_eq!(fresh.recording_counter(), 3);
        let _ = record_one(&mut dirty, &x);
        let _ = record_one(&mut fresh, &x);
        assert_eq!(fresh.recording_counter(), 4);
        assert_eq!(export(&dirty, 4), export(&fresh, 1), "the recording at counter 3");
        // The one-pass: equal after the original's reset (rekeyed at construction, reset and
        // reconfigure only; hardware review B6).
        dirty.reset();
        fresh.reset();
        assert_eq!(one_pass(&mut dirty, 2000), one_pass(&mut fresh, 2000));
        // A counter-0 engine differs (the stream is keyed).
        let mut zero = Pipeline::new(s).unwrap();
        let _ = record_one(&mut zero, &x);
        assert_ne!(export(&zero, 1), export(&fresh, 1));
        assert_eq!(dirty.recording_counter_of(4).unwrap(), Some(3));
        assert_eq!(fresh.recording_counter_of(1).unwrap(), Some(3));
    }

    /// T004: the pre-encoder latency against the published one.
    #[test]
    fn pre_encoder_latency() {
        for rate in crate::SUPPORTED_HOST_RATES {
            let mut s = Settings::new(rate);
            s.speech_codec_enabled = false;
            let p = Pipeline::new(s).unwrap();
            assert_eq!(p.record_latency_pre_encoder_samples(), p.record_latency_samples());
            s.speech_codec_enabled = true;
            let p = Pipeline::new(s).unwrap();
            assert_eq!(
                p.record_latency_pre_encoder_samples() + 180 * rate / 6000,
                p.record_latency_samples(),
                "{rate} Hz"
            );
        }
    }

    /// T006: the block holds the start configuration (a landed change and a reconfigure
    /// later do not move it); the totals; an imported recording has none.
    #[test]
    fn block_holds_the_start_configuration_and_totals() {
        let x = stimulus(9000);
        let mut p = Pipeline::new(settings(1)).unwrap();
        p.schedule_change(Stream::Record, 100, KNEE, -4.0, 5).unwrap();
        p.schedule_change(Stream::Record, 8000, KNEE, -6.0, 6).unwrap();
        let (info, out) = record_one(&mut p, &x);
        assert_eq!(info.changes, 2);
        let b = p.memory.block(1).unwrap().unwrap();
        assert_eq!(b.counter, 0);
        assert_eq!(b.settings.agc.knee_dbfs, settings(1).agc.knee_dbfs, "the start knee");
        assert_eq!(b.remaining_samples, 120_000);
        assert_eq!(b.changes_total, 2);
        assert_eq!(b.events_total, (info.events + out.flush.events) as u64);
        assert!(b.events_total >= 1, "the pause made an event");
        // The settings moved; a reconfigure to another rate keeps the block as it was.
        assert_eq!(p.settings().agc.knee_dbfs, -6.0);
        let mut s = *p.settings();
        s.host_rate_hz = 48_000;
        p.reconfigure(s).unwrap();
        let b = p.memory.block(1).unwrap().unwrap();
        assert_eq!(b.settings.host_rate_hz, 6000);
        assert_eq!(b.settings.agc.knee_dbfs, settings(1).agc.knee_dbfs);
        // Grow the slot count with the memory kept: the blocks survive.
        s.memory.max_recordings = 12;
        p.reconfigure(s).unwrap();
        assert_eq!(p.recording_counter_of(1).unwrap(), Some(0));
        assert_eq!(p.memory.capacities().1, 13); // the span table; the blocks match it
        // An imported recording has no block.
        let bytes = export(&p, 1);
        let n = p.import(&bytes).unwrap();
        assert_eq!(p.recording_counter_of(n).unwrap(), None);
        assert_eq!(p.recording_counter_of(9).unwrap_err(), Error::NoSuchRecording { number: 9 });
        // Erase moves the blocks with their recordings.
        p.erase(1).unwrap();
        assert_eq!(p.recording_counter_of(1).unwrap(), None);
    }

    /// T006: a landing in the stop's flush counts; a flush entry's position lies between the
    /// stop position and the input total (coach 4, hardware B3).
    #[test]
    fn flush_landing_counts_and_lies_in_the_flush() {
        let x = stimulus(3000);
        let mut p = Pipeline::new(settings(2)).unwrap();
        // The flush feeds the pre-encoder latency of zeros (1 at 6 kHz): position 3000 is the
        // flush's first sample.
        assert!(p.record_latency_pre_encoder_samples() >= 1);
        p.schedule_change(Stream::Record, 3000, KNEE, -4.0, 5).unwrap();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 64];
        let info = p.record(&x, &mut ev).unwrap();
        assert_eq!(info.changes, 0);
        let mut ch = [ChangeEntry::default(); 8];
        let out = p.stop_recording_with_changes(&mut ev, &mut ch).unwrap();
        assert_eq!(out.changes, 1);
        assert_eq!(out.stop_input_position, 3000);
        assert!(ch[0].position >= out.stop_input_position);
        assert!(ch[0].position < out.input_samples_total);
        assert_eq!(ch[0].stream, STREAM_RECORD);
        assert_eq!(p.memory.block(1).unwrap().unwrap().changes_total, 1);
    }

    /// T007: the stop fields for a host stop (codec off and on), a self-stop and the hybrid.
    #[test]
    fn stop_fields() {
        // Codec off: the padding start is the length.
        let x = stimulus(3000);
        let mut p = Pipeline::new(settings(3)).unwrap();
        let (_, out) = record_one(&mut p, &x);
        let StopResult::Kept { length, .. } = out.result else {
            panic!()
        };
        assert_eq!(out.padding_start_sample, length);
        assert_eq!(out.stop_input_position, 3000);
        let f = StopFields::from(&out);
        assert_eq!(f.how, StopHow::StopCall);
        assert_eq!(f.input_samples_total, out.input_samples_total);
        assert!(StopFields::kept(&out));
        // Codec on: the padding start is the represented count, below the length.
        let mut s = settings(3);
        s.speech_codec_enabled = true;
        let mut p = Pipeline::new(s).unwrap();
        let (_, out) = record_one(&mut p, &x);
        let StopResult::Kept { length, .. } = out.result else {
            panic!()
        };
        assert!(out.padding_start_sample < length);
        // The padding runs to the frame boundary plus the 60-sample look-ahead, which is not
        // stored (spec 008 FR-013): fed = length + 60 − represented.
        assert_eq!(out.padding_start_sample + out.silence_fed_samples, length + 60);
        assert_eq!(length, out.padding_start_sample.div_ceil(120) * 120);
        // A self-stop: the fields from the `RecordInfo`.
        let mut s = settings(3);
        s.memory.capacity_s = 1;
        s.vas.enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 64];
        let info = p.record(&stimulus(9000), &mut ev).unwrap();
        assert_eq!(p.memory.block(1).unwrap().unwrap().remaining_samples, 6000);
        assert_eq!(info.stopped, Some(StopReason::MemoryFull));
        let f = StopFields::from(&info);
        assert_eq!(f.how, StopHow::MemoryFull);
        assert_eq!(f.stop_input_position, info.input_samples_total);
        assert_eq!(f.silence_fed_samples, 0);
        assert!(p.stop_recording(&mut ev).is_err());
        // The hybrid: a host stop whose flush fills the memory keeps `StopCall`, no padding.
        let mut s = settings(3);
        s.memory.capacity_s = 1;
        s.vas.enabled = false;
        s.speech_codec_enabled = true;
        let mut p = Pipeline::new(s).unwrap();
        let pre = p.record_latency_pre_encoder_samples() as usize;
        p.start_recording().unwrap();
        let fill = stimulus(6000 - pre / 2);
        let info = p.record(&fill, &mut ev).unwrap();
        assert!(info.stopped.is_none());
        let out = p.stop_recording(&mut ev).unwrap();
        let StopResult::Kept { length, .. } = out.result else {
            panic!()
        };
        assert_eq!(out.flush.stopped, Some(StopReason::MemoryFull), "the flush filled it");
        assert_eq!(StopFields::from(&out).how, StopHow::StopCall);
        assert_eq!(out.padding_start_sample, length);
        assert_eq!(length, 6000);
    }

    /// T007: the stream word per stream, and 0 on a default entry.
    #[test]
    fn stream_words() {
        assert_eq!(ChangeEntry::default().stream, 0);
        assert_eq!(Stream::Playback.word(), STREAM_PLAYBACK);
        assert_eq!(Stream::OnePass.word(), STREAM_ONE_PASS);
        assert_eq!(Stream::Record.word(), STREAM_RECORD);
        let mut p = Pipeline::new(settings(4)).unwrap();
        p.schedule_change(Stream::OnePass, 10, KNEE, -4.0, 1).unwrap();
        p.schedule_change(Stream::OnePass, 10, 1, 0.5, 2).unwrap(); // the volume
        let x = stimulus(600);
        let mut y = std::vec![0.0f32; 600];
        let mut ev = [VasEvent::default(); 16];
        let mut ch = [ChangeEntry::default(); 8];
        let info = p
            .process_with_events_and_changes(&x, &mut y, &mut ev, &mut ch)
            .unwrap();
        assert_eq!(info.changes, 2);
        assert!(ch[..2].iter().all(|e| e.stream == STREAM_ONE_PASS));
        let _ = record_one(&mut p, &x);
        p.schedule_change(Stream::Playback, 5, 1, 0.3, 3).unwrap();
        p.start_playback(1).unwrap();
        let mut pi = p.play_with_changes(&mut y, &mut ch).unwrap();
        while pi.changes == 0 && !pi.finished {
            pi = p.play_with_changes(&mut y, &mut ch).unwrap();
        }
        assert_eq!(ch[0].stream, STREAM_PLAYBACK);
    }
}
