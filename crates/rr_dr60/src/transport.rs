//! The transport: the engine's state (idle, recording, playing), the record and playback calls,
//! the route, and the reports (spec 006 FR-003 – FR-008, FR-013, FR-015, FR-016, FR-019;
//! research R-05, R-06; data-model.md › Transport).
//!
//! The device records into its memory or plays an item back, never both at once: its power
//! amplifier is muted while recording (S-002), so recording and playback are exclusive states
//! here and a conflicting start is refused with the activity in progress. Each recording and
//! each playback starts from its path's reset state (A-037, an engine decision required by
//! FR-007).

use crate::error::Error;
use crate::memory::{MemoryStatus, Span};
use crate::pipeline::{Pipeline, VasEvent};
use crate::settings::Route;

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
    /// recording is kept).
    MemoryFull,
}

/// The report of one `record` call, and of the stop flush (spec 006 FR-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[must_use = "read `stored` and `stopped`: the recording can stop inside a block"]
#[non_exhaustive]
pub struct RecordInfo {
    /// Device samples appended to the recording by this call.
    pub stored: usize,
    /// VAS events this call generated (splices in drop mode, muted regions in mute mode),
    /// with positions in the recording's device samples from its start.
    pub events: usize,
    /// The VAS is paused at the end of the call.
    pub paused: bool,
    /// The recording stopped by itself during this call; the engine is idle and the recording
    /// is kept (a following `stop_recording` is `NotRecording`).
    pub stopped: Option<StopReason>,
}

/// What `stop_recording` did with the recording (spec 006 FR-005, FR-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StopResult {
    /// The recording is in the memory.
    Kept {
        /// Its number (1 to the count).
        number: u32,
        /// Its length in device samples (÷ 6000 for seconds, A-026).
        length: u64,
    },
    /// No input was fed since the start: the flush was skipped and no recording was added
    /// (A-039, an engine decision).
    DiscardedEmpty,
}

/// The result of `stop_recording` (spec 006 FR-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "read `result`: the recording may have been discarded as empty"]
#[non_exhaustive]
pub struct StopOutcome {
    /// The stop flush's report (all zeros when the flush was skipped).
    pub flush: RecordInfo,
    /// Kept, with its number and length, or discarded.
    pub result: StopResult,
}

/// The report of one `play` call (spec 006 FR-006, FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[must_use = "read `produced` and `finished`"]
#[non_exhaustive]
pub struct PlayInfo {
    /// Host samples written to the start of the output buffer.
    pub produced: usize,
    /// The recording and its drain ended in this call; the engine is idle.
    pub finished: bool,
}

/// The player's cursor into the recording being played (data-model.md › Transport).
#[derive(Clone, Copy, Debug, Default)]
struct Cursor {
    span: Span,
    /// Stored samples pulled so far.
    offset: usize,
    /// Host samples produced so far.
    produced: u64,
    /// Host samples the playback produces in all: ⌈K·m/l⌉ + the drain (research R-06).
    total: u64,
    /// The recording's number.
    number: u32,
}

/// The engine's state machine (research R-05).
#[derive(Clone, Debug, Default)]
pub(crate) struct Transport {
    state: State,
    /// Any host sample was pushed since the recording started (an empty block does not count):
    /// decides the flush and the empty case (hardware review B5).
    fed_any: bool,
    cursor: Cursor,
}

impl Transport {
    /// Back to idle (spec 006 FR-019: `reset`).
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
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
    /// limiter discharged, the VAS unpaused, the filters and the conversion cleared; A-037)
    /// and the recording in progress is opened as number `count + 1`. Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::RecordingInProgress`], [`Error::PlaybackInProgress`], then
    /// [`Error::MemoryFull`] (no recorded time remains) before [`Error::NoRecordingSlots`]
    /// (S-001: "FULL" refuses a start).
    pub fn start_recording(&mut self) -> Result<(), Error> {
        self.refuse_if_active()?;
        self.memory.open()?;
        self.recorder.reset();
        self.transport.fed_any = false;
        self.transport.state = State::Recording;
        Ok(())
    }

    /// Feeds one block of host-rate input to the record path and appends what it produces to
    /// the recording in progress; writes the block's VAS events to `events` as
    /// [`process_with_events`](Self::process_with_events) does, with positions in the
    /// recording's device samples from its start (spec 006 FR-001, FR-006). The recording
    /// stops by itself at the exact device sample at which the memory fills (`stopped`,
    /// FR-012); the rest of the block is discarded and the engine is idle. Real-time safe.
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
        let (info, _) = self.feed(input, events);
        Ok(info)
    }

    /// An `events` length for [`record`](Self::record) that is always enough for a block of
    /// `frames` host samples with the current settings (spec 003 R-07). Never allocates.
    pub fn max_record_events(&self, frames: usize) -> usize {
        self.recorder.max_events(frames)
    }

    /// Stops the recording (spec 006 FR-005): passes the record path's own latency of silence
    /// through it, so every input sample fed is represented in the recording, closes an open
    /// muted region with a synthesized event, and closes the recording as `Kept`; or, when
    /// nothing was fed since the start, skips the flush and discards it (`DiscardedEmpty`,
    /// A-039). The flush's report is returned like a `record` call's (it can itself fill the
    /// memory). Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NotRecording`] when no recording is in progress, which includes a recording
    /// that already stopped at the memory's limit.
    pub fn stop_recording(&mut self, events: &mut [VasEvent]) -> Result<StopOutcome, Error> {
        if self.transport.state != State::Recording {
            return Err(Error::NotRecording);
        }
        if !self.transport.fed_any {
            self.transport.state = State::Idle;
            let result = self.memory.close(false);
            return Ok(StopOutcome {
                flush: RecordInfo::default(),
                result,
            });
        }
        let (mut flush, stopped) = self.feed_zeros(self.recorder.latency_host_samples(), events);
        let result = match stopped {
            // The flush filled the memory: the recording is already closed (FR-012).
            Some(result) => result,
            None => self.close_recording(events, &mut flush),
        };
        Ok(StopOutcome { flush, result })
    }

    /// Starts playback of recording `number` through the playback path, reset first (spec 006
    /// FR-002, FR-004, FR-008). Real-time safe.
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
        let (l, m) = self.plan_l_m;
        // ⌈K·m/l⌉ host samples for K stored, then the drain (research R-06).
        let pre_drain = (span.len as u128 * u128::from(m)).div_ceil(u128::from(l)) as u64;
        self.transport.cursor = Cursor {
            span,
            offset: 0,
            produced: 0,
            total: pre_drain + u64::from(self.player.drain_len_host_samples()),
            number,
        };
        self.transport.state = State::Playing;
        Ok(())
    }

    /// Produces the next block of host-rate output of the recording being played (spec 006
    /// FR-002, FR-006, FR-008): exactly ⌈K·m/l⌉ samples for K stored samples, then the drain
    /// (`drain_samples()` of settled silence), then `finished` and idle. Only
    /// `output[..produced]` is written. Real-time safe: at most one stored sample is pulled
    /// per output sample.
    ///
    /// # Errors
    ///
    /// [`Error::NotPlaying`] when no playback is in progress; nothing changes.
    pub fn play(&mut self, output: &mut [f32]) -> Result<PlayInfo, Error> {
        if self.transport.state != State::Playing {
            return Err(Error::NotPlaying);
        }
        let mut produced = 0;
        let cursor = &mut self.transport.cursor;
        let stored = self.memory.samples(cursor.span);
        while produced < output.len() && cursor.produced < cursor.total {
            let y = loop {
                if let Some(y) = self.player.try_next_host() {
                    break y;
                }
                // Pull the next stored device sample, or a zero of the drain.
                let d = match stored.get(cursor.offset) {
                    Some(&v) => v,
                    None => 0.0,
                };
                cursor.offset += 1;
                self.player.push_device(d);
            };
            output[produced] = y;
            produced += 1;
            cursor.produced += 1;
        }
        let finished = cursor.produced >= cursor.total;
        if finished {
            self.transport.state = State::Idle;
        }
        Ok(PlayInfo { produced, finished })
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
            } else if number < cursor.number {
                cursor.number -= 1;
            }
        }
        let erased = self.memory.erase(number).expect("checked above");
        if self.transport.state == State::Playing && number < self.transport.cursor.number + 1 {
            // The played recording's samples moved down with the store (its number already did).
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
        }
        self.memory.erase_all();
        Ok(())
    }

    /// Selects the output route (spec 006 FR-015): speaker or earphone, at any time, during
    /// playback too; it takes effect at the next `play` block and survives `reset` and
    /// `reconfigure`. The two routes produce identical output in this version. Real-time safe.
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

    /// The length of recording `number` in device samples (÷ 6000 for seconds, A-026).
    /// Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchRecording`].
    pub fn recording_length(&self, number: u32) -> Result<u64, Error> {
        self.memory
            .span(number)
            .map(|s| s.len as u64)
            .ok_or(Error::NoSuchRecording { number })
    }

    /// The record path's latency in host samples: input samples until the corresponding device
    /// sample is stored (spec 006 FR-016); the stop flush is this long. Real-time safe.
    pub fn record_latency_samples(&self) -> u32 {
        self.recorder.latency_host_samples()
    }

    /// The playback path's latency in host samples: output samples from the start of playback
    /// until the first stored sample is represented, `round(D + τ_play · host / 6000)`, and
    /// measured as the path's 1 kHz group delay (spec 006 FR-016); the onset peak of a
    /// one-sample recording lies 0.2–0.3 device samples later and is reported for information
    /// only. The two latencies sum to [`latency_samples`](Self::latency_samples) within one
    /// sample. Real-time safe.
    pub fn playback_latency_samples(&self) -> u32 {
        self.player.latency_host_samples()
    }

    /// The host samples `play` produces after the recording's own ⌈K·m/l⌉: stage 10's
    /// settling length of device-rate silence (121 device samples at the committed
    /// coefficients, −100 dB; 0 with the stage bypassed) and the rate conversion's full
    /// remaining response (spec 006 FR-008). Real-time safe.
    pub fn drain_samples(&self) -> u32 {
        self.player.drain_len_host_samples()
    }

    /// The bytes reserved for the recording store and its span table (spec 006 FR-017,
    /// SC-007): about 86 MB at the default capacity, virtual until recordings are written
    /// (commit charge on Windows). It never changes after construction or reconfiguration.
    /// Real-time safe.
    pub fn footprint_bytes(&self) -> usize {
        self.memory.footprint_bytes()
    }

    /// Feeds `input` to the recorder and the memory. Returns the report and, when the memory
    /// filled, the closed recording's result.
    fn feed(&mut self, input: &[f32], events: &mut [VasEvent]) -> (RecordInfo, Option<StopResult>) {
        let mut info = RecordInfo::default();
        for &x in input {
            if let Some(stopped) = self.feed_one(x, events, &mut info) {
                return (info, Some(stopped));
            }
        }
        info.paused = self.recorder.paused();
        (info, None)
    }

    /// As [`feed`](Self::feed), with `n` zeros (the stop flush, spec 006 FR-005).
    fn feed_zeros(&mut self, n: u32, events: &mut [VasEvent]) -> (RecordInfo, Option<StopResult>) {
        let mut info = RecordInfo::default();
        for _ in 0..n {
            if let Some(stopped) = self.feed_one(0.0, events, &mut info) {
                return (info, Some(stopped));
            }
        }
        info.paused = self.recorder.paused();
        (info, None)
    }

    /// One host sample: pushes it, appends a produced device sample, hands out the event, and
    /// closes the recording when the memory fills (spec 006 FR-012).
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
                self.memory.append(kept)
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
        self.transport.state = State::Idle;
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
        let mut s = Settings::new(rate);
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
            "the route does not change the sound"
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
            let mut s = Settings::new(rate);
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
        let mut s = Settings::new(6000);
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
            let mut s = Settings::new(rate);
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
            let mut s = Settings::new(rate);
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
        let mut s = Settings::new(6000);
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
        let mut s = Settings::new(6000);
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
        let mut s = Settings::new(48_000);
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
        let mut s = Settings::new(6000);
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
        let mut s = Settings::new(48_000);
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
}
