//! Spec 020 (the processing record): the sixteen-cell re-render matrix, the named effective
//! changes and the playback twins, the recipe (research R-06), the dirty-engine counter
//! identity. Not `record_checks.rs`, which is spec 017's.

use rr_dr60::changes::{STREAM_RECORD, Stream};
use rr_dr60::input::{i24_from_sample, samples_from_i24};
use rr_dr60::record::{self, RecordReadout, RunData, StopFields, StopHow};
use rr_dr60::{ChangeEntry, Encoding, Pipeline, Settings, StopResult, VasEvent, VasMode};

use crate::checks::Make;
use crate::stimulus;

/// The control manifest's indices the matrix schedules.
pub const KNEE: u32 = 10;
/// `mic.hiss_offset_db`.
pub const HISS: u32 = 28;
/// `vas.threshold_dbfs`.
pub const THRESHOLD: u32 = 16;

/// One cell of the D6 matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The speech codec on.
    pub codec: bool,
    /// Two record-side changes scheduled.
    pub changes: bool,
    /// The voice activation in mute mode (else drop).
    pub mute: bool,
    /// The recording stops itself (the memory pre-filled by import).
    pub self_stop: bool,
}

impl Cell {
    /// The sixteen cells.
    pub fn all() -> Vec<Cell> {
        let mut v = Vec::new();
        for codec in [false, true] {
            for changes in [false, true] {
                for mute in [false, true] {
                    for self_stop in [false, true] {
                        v.push(Cell {
                            codec,
                            changes,
                            mute,
                            self_stop,
                        });
                    }
                }
            }
        }
        v
    }

    /// A short name.
    pub fn name(self) -> String {
        format!(
            "{}/{}/{}/{}",
            if self.codec { "codec" } else { "samples" },
            if self.changes { "changes" } else { "constant" },
            if self.mute { "mute" } else { "drop" },
            if self.self_stop {
                "self_stop"
            } else {
                "stop_call"
            }
        )
    }
}

/// The memory capacity of a self-stop cell, in seconds (whole frames with the codec on);
/// a stop-call cell has 20 s (the 2.3 s stimulus must fit in mute mode).
pub const CAPACITY_S: u32 = 2;
/// The remaining capacity at the start of a self-stop cell, in device samples (1.2 s: inside
/// voice in both gate modes, research R-06).
pub const SELF_STOP_REMAINING: u64 = 7200;

/// The cell's settings at `rate`: the default engine with the cell's codec and gate mode, a
/// 100 ms hang (the stimulus's gaps pause the gate), seed 0x20.
pub fn settings(cell: Cell, rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = cell.codec;
    s.vas.mode = if cell.mute {
        VasMode::Mute
    } else {
        VasMode::Drop
    };
    s.vas.hang_ms = 100.0;
    s.seed = 0x20;
    s.memory.capacity_s = if cell.self_stop { CAPACITY_S } else { 20 };
    s.memory.max_recordings = 4;
    s
}

/// The matrix stimulus: voice at −20 dBFS in three 0.5 s segments with two 0.4 s gaps,
/// ending in voice (research R-06), quantized to 24 bits at the feed point (D12).
pub fn raw_stimulus(rate: u32) -> Vec<i32> {
    let fs = f64::from(rate);
    let gap = stimulus::silence((0.4 * fs) as usize);
    let mut x = stimulus::voiced(fs, 0.5, -20.0, 0x20);
    x.extend(&gap);
    x.extend(stimulus::voiced(fs, 0.5, -20.0, 0x21));
    x.extend(&gap);
    x.extend(stimulus::voiced(fs, 0.5, -20.0, 0x22));
    x.iter().map(|&v| i24_from_sample(v)).collect()
}

/// The engine's input for raw samples (the published feed-point rule).
pub fn samples(raw: &[i32]) -> Vec<f32> {
    let mut v = vec![0.0f32; raw.len()];
    samples_from_i24(raw, &mut v);
    v
}

/// An exported filler recording of `units` stored units (zeros) in `encoding`: a 6 kHz
/// `bypass_all` engine with the codec setting records exactly that many units, exports
/// (research R-06: the fill is by import, never by recording).
pub fn filler(encoding: Encoding, units: u64) -> Vec<u8> {
    let mut s = crate::configs::settings("bypass_all", 6000);
    s.speech_codec_enabled = encoding == Encoding::Frames;
    s.memory.capacity_s = CAPACITY_S.max((units / 6000) as u32 + 1);
    let mut p = Pipeline::new(s).expect("the filler engine");
    p.start_recording().expect("start");
    let n = units as usize * encoding.unit_samples();
    let _ = p.record(&vec![0.0f32; n], &mut []).expect("record");
    let out = p.stop_recording(&mut []).expect("stop");
    let StopResult::Kept { number, length } = out.result else {
        panic!("empty filler")
    };
    assert_eq!(length, n as u64, "the filler's length");
    let mut bytes = vec![0u8; p.export_len(number).expect("len")];
    let _ = p.export(number, &mut bytes).expect("export");
    bytes
}

/// Fills `p`'s memory by import so that its remaining capacity is `remaining` device
/// samples.
pub fn fill_to_remaining(p: &mut Pipeline, remaining: u64) {
    let st = p.memory_status();
    assert!(
        st.remaining_samples >= remaining,
        "already fuller than asked"
    );
    let units = (st.remaining_samples - remaining) / p.encoding().unit_samples() as u64;
    if units > 0 {
        let _ = p
            .import(&filler(p.encoding(), units))
            .expect("the filler fits");
    }
    assert_eq!(p.memory_status().remaining_samples, remaining);
}

/// The two record-side changes a `changes` cell schedules (positions in host input samples).
pub fn cell_changes(rate: u32) -> [(u64, u32, f64, u64); 2] {
    let fs = u64::from(rate);
    // The knee lands below the −20 dBFS voice's peaks, so the entry is effective (the
    // limiter engages from it on); the hiss change rides on the second voiced segment.
    [(fs * 3 / 10, KNEE, -30.0, 7), (fs * 9 / 10, HISS, 3.0, 8)]
}

/// A recording made and its record written: everything the re-render needs and the digest
/// it must reproduce.
#[derive(Clone, Debug)]
pub struct Run {
    /// The record's bytes.
    pub record: Vec<u8>,
    /// The stored form's bytes.
    pub export: Vec<u8>,
    /// Its digest.
    pub digest: u64,
    /// The run as collected.
    pub events: Vec<VasEvent>,
    /// The entries as collected.
    pub changes: Vec<ChangeEntry>,
    /// The stop's fields.
    pub stop: StopFields,
    /// The recording's number in `engine`.
    pub number: u32,
    /// The counter the recording was keyed with.
    pub counter: u64,
    /// The gate was paused when the stop was called (stop-call cells assert `false`).
    pub flush_paused: bool,
}

/// Records `raw` in a `cell` engine from `make` at `rate` and writes the record (D10: the run
/// collected from every call and the stop). A `changes` cell schedules [`cell_changes`]; a
/// `self_stop` cell pre-fills the memory to [`SELF_STOP_REMAINING`] by import.
pub fn record_run(make: Make<'_>, rate: u32, cell: Cell, raw: &[i32]) -> (Pipeline, Run) {
    let mut p = make(settings(cell, rate));
    record_run_in(&mut p, cell, raw)
}

/// As [`record_run`] in an existing engine (its settings are the cell's).
pub fn record_run_in(p: &mut Pipeline, cell: Cell, raw: &[i32]) -> (Pipeline, Run) {
    let mut p = std::mem::replace(p, Pipeline::new(Settings::new(6000)).unwrap());
    let rate = p.settings().host_rate_hz;
    if cell.self_stop {
        fill_to_remaining(&mut p, SELF_STOP_REMAINING);
    }
    if cell.changes {
        for (pos, control, value, tag) in cell_changes(rate) {
            p.schedule_change(Stream::Record, pos, control, value, tag)
                .expect("schedule");
        }
    }
    let x = samples(raw);
    let counter = p.recording_counter();
    p.start_recording().expect("start");
    let (mut events, mut changes) = (Vec::new(), Vec::new());
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut stop: Option<StopFields> = None;
    let mut flush_paused = false;
    for block in x.chunks(1024) {
        let mut ev = vec![VasEvent::default(); p.max_record_events(block.len())];
        let info = p
            .record_with_changes(block, &mut ev, &mut log)
            .expect("record");
        events.extend_from_slice(&ev[..info.events]);
        changes.extend_from_slice(&log[..info.changes]);
        if info.stopped.is_some() {
            assert!(cell.self_stop, "{}: the memory filled", cell.name());
            stop = Some(StopFields::from(&info));
            break;
        }
    }
    let stop = match stop {
        Some(s) => s,
        None => {
            assert!(!cell.self_stop, "{}: no self-stop", cell.name());
            let mut ev = vec![VasEvent::default(); p.max_record_events(rate as usize)];
            let out = p
                .stop_recording_with_changes(&mut ev, &mut log)
                .expect("stop");
            events.extend_from_slice(&ev[..out.flush.events]);
            changes.extend_from_slice(&log[..out.changes]);
            flush_paused = out.flush.paused;
            assert!(StopFields::kept(&out));
            StopFields::from(&out)
        }
    };
    let number = p.memory_status().count;
    let run = RunData {
        events: &events,
        changes: &changes,
        stop,
    };
    let n = p.processing_record_len(number, &run).expect("len");
    let mut record = vec![0u8; n];
    assert_eq!(
        p.processing_record_write(number, &run, &mut record)
            .expect("write"),
        n
    );
    let mut export = vec![0u8; p.export_len(number).expect("len")];
    let _ = p.export(number, &mut export).expect("export");
    let digest = p.recording_digest(number).expect("digest");
    (
        p,
        Run {
            record,
            export,
            digest,
            events,
            changes,
            stop,
            number,
            counter,
            flush_paused,
        },
    )
}

/// What the re-render may vary (the named effective changes of FR-005 and the twins).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delta {
    /// Nothing: the contract.
    None,
    /// The seed + 1.
    Seed,
    /// The counter + 1.
    Counter,
    /// A record-side setting moved: `mic.hiss_offset_db` +1 dB.
    Hiss,
    /// `agc.knee_dbfs` to −30 dBFS (below the −20 dBFS voice's peaks: a 1 dB move of a
    /// −1 dBFS knee is never reached by the stimulus and leaves the digest unmoved).
    Knee,
    /// `vas.threshold_dbfs` −3 dB.
    Threshold,
    /// `speech_codec_enabled` toggled.
    Codec,
    /// The memory's capacity + 1 s (the self-stop cell: the fill differs).
    Capacity,
    /// A 10 ms burst at −20 dBFS inserted at a kept input position.
    Burst,
    /// The hiss entry moved later by one device sample's worth of host input: its 20 ms ramp
    /// moves with it, so every ramped sample changes; asserted with the codec bypassed, where
    /// every stored sample is visible. (A limiter entry moved by one sample is effective only
    /// when the landing sample exceeds the knee: at 8 kHz it did not.)
    EntryMoved,
    /// The knee entry moved later by 0.1 s; asserted with the codec on (a one-sample move need
    /// not flip an 80-bit frame, hardware review B3; 0.1 s of unlimited voice does).
    EntryMovedFar,
    /// The host's stop one frame's worth of input earlier.
    StopEarlier,
    /// Twins (the digest must not move): a playback-side setting or live control.
    TwinVolume,
    /// The route.
    TwinRoute,
    /// The playback speed.
    TwinSpeed,
    /// The battery value.
    TwinBattery,
    /// `output.clamp_enabled`.
    TwinClamp,
    /// The fill by recording instead of import (the counter moves: must fail).
    RecordedFiller,
    /// No fill at all (the self-stop cell's mutant: the recording cannot stop itself where
    /// the original did).
    NoFill,
}

impl Delta {
    /// The effective changes of a stop-call changes cell with the codec `codec` (the
    /// capacity delta is the self-stop cell's).
    pub fn effective(codec: bool) -> [Delta; 9] {
        [
            Delta::Seed,
            Delta::Counter,
            Delta::Hiss,
            Delta::Knee,
            Delta::Threshold,
            Delta::Codec,
            Delta::Burst,
            if codec {
                Delta::EntryMovedFar
            } else {
                Delta::EntryMoved
            },
            Delta::StopEarlier,
        ]
    }
    /// The twins.
    pub const TWINS: [Delta; 5] = [
        Delta::TwinVolume,
        Delta::TwinRoute,
        Delta::TwinSpeed,
        Delta::TwinBattery,
        Delta::TwinClamp,
    ];
}

/// Reads a record.
pub fn readout(record: &[u8]) -> RecordReadout {
    let mut r = RecordReadout::default();
    record::read(record, &mut r).expect("a valid record");
    r
}

/// The recipe (research R-06): an engine built at the record's identity and configuration,
/// its memory filled by import to the record's remaining capacity, the record's entries
/// scheduled on the record stream (refilled past 64 pending), the raw audio fed through the
/// feed-point rule up to the stop position in `block` samples, the same stop, the digest.
/// `delta` varies one thing.
pub fn re_render(record: &[u8], raw: &[i32], block: usize, delta: Delta) -> u64 {
    let r = readout(record);
    let mut s = r.settings;
    let mut counter = r.recording_counter;
    let mut raw = raw.to_vec();
    let mut stop = r.stop;
    let mut changes = vec![ChangeEntry::default(); r.change_count as usize];
    assert_eq!(
        record::record_changes(record, 0, &mut changes),
        changes.len()
    );
    match delta {
        Delta::None => {}
        Delta::Seed => s.seed = s.seed.wrapping_add(1),
        Delta::Counter => counter = counter.wrapping_add(1),
        Delta::Hiss => s.mic.hiss_offset_db += 1.0,
        // A knee 1 dB lower is not reached by −20 dBFS voice (the digest would hold, hardware
        // review B3): the knee moves below the voice's peaks, so the limiter engages.
        Delta::Knee => s.agc.knee_dbfs = -30.0,
        Delta::Threshold => s.vas.threshold_dbfs -= 3.0,
        Delta::Codec => s.speech_codec_enabled = !s.speech_codec_enabled,
        Delta::Capacity => s.memory.capacity_s += 1,
        Delta::Burst => {
            let rate = s.host_rate_hz as usize;
            let at = rate / 4; // inside the first voiced segment
            for (i, v) in stimulus::tone(1000.0, 0.1, f64::from(s.host_rate_hz), rate / 100)
                .iter()
                .enumerate()
            {
                raw[at + i] = i24_from_sample(samples(&[raw[at + i]])[0] + v);
            }
        }
        Delta::EntryMoved => {
            assert!(changes.len() >= 2, "a changes cell");
            // At least one device sample later: ⌈rate ÷ 6000⌉ host samples (at 8 kHz one
            // host sample can land on the same device sample, spec 017's k(P) = ⌈P · 6000 ÷ rate⌉).
            changes[1].position += u64::from(s.host_rate_hz).div_ceil(6000);
        }
        Delta::EntryMovedFar => {
            assert!(!changes.is_empty(), "a changes cell");
            changes[0].position += u64::from(s.host_rate_hz) / 10;
        }
        Delta::StopEarlier => {
            assert_eq!(stop.how, StopHow::StopCall, "a stop-call cell");
            stop.stop_input_position -= 120 * u64::from(s.host_rate_hz) / 6000;
        }
        Delta::TwinClamp => s.output.clamp_enabled = !s.output.clamp_enabled,
        _ => {}
    }
    let mut p = Pipeline::new_at_counter(s, counter).expect("the record's configuration");
    match delta {
        Delta::TwinVolume => p.set_volume(0.5).unwrap(),
        Delta::TwinRoute => p.set_route(rr_dr60::Route::Earphone),
        Delta::TwinSpeed => p.set_playback_speed(5).unwrap(),
        Delta::TwinBattery => p.set_battery_volts(2.4).unwrap(),
        _ => {}
    }
    // The fill (D6): by import, so that the remaining equals the record's.
    let st = p.memory_status();
    if delta == Delta::RecordedFiller {
        let units = (st.remaining_samples - r.remaining_samples_at_start)
            / p.encoding().unit_samples() as u64;
        p.start_recording().unwrap();
        let _ = p
            .record(
                &vec![0.0f32; units as usize * p.encoding().unit_samples()],
                &mut [],
            )
            .unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
    } else if delta == Delta::NoFill {
        // The mutant: no fill.
    } else if st.remaining_samples > r.remaining_samples_at_start && delta != Delta::Capacity {
        fill_to_remaining(&mut p, r.remaining_samples_at_start);
    } else if delta == Delta::Capacity {
        fill_to_remaining(&mut p, r.remaining_samples_at_start + 6000);
    }
    let x = samples(&raw);
    let mut next = 0usize; // the next entry to schedule
    let schedule = |p: &mut Pipeline, next: &mut usize| {
        while *next < changes.len() && p.pending_changes(Stream::Record).unwrap() < 64 {
            let c = changes[*next];
            p.schedule_change(Stream::Record, c.position, c.control, c.after, c.tag)
                .expect("schedule");
            *next += 1;
        }
    };
    schedule(&mut p, &mut next);
    p.start_recording().expect("start");
    let fed = (stop.stop_input_position as usize).min(x.len());
    let mut pos = 0usize;
    let mut stopped = false;
    let mut ev = vec![VasEvent::default(); p.max_record_events(block)];
    while pos < fed {
        let n = block.min(fed - pos);
        let info = p.record(&x[pos..pos + n], &mut ev).expect("record");
        pos += n;
        schedule(&mut p, &mut next);
        if info.stopped.is_some() {
            stopped = true;
            break;
        }
    }
    match stop.how {
        StopHow::StopCall => {
            if !stopped {
                let _ = p.stop_recording(&mut ev).expect("stop");
            }
        }
        StopHow::MemoryFull => {
            if !stopped {
                // The fill was wrong (a delta): stop so the digest differs honestly.
                let _ = p.stop_recording(&mut ev).expect("stop");
            }
        }
    }
    let number = p.memory_status().count;
    p.recording_digest(number).expect("digest")
}

/// The dirty-engine counter identity (spec US1 AS5): an engine that recorded `c` times with a
/// landed knee change, a pause at the end of the last recording, a burst in its last 100 ms
/// and a one-pass run between; a fresh engine at `c` records the same bytes, and its one-pass
/// equals the original's after the original's `reset`. Returns the mismatch count (0).
pub fn dirty_engine_identity(make: Make<'_>, rate: u32, c: u64) -> usize {
    let cell = Cell {
        codec: false,
        changes: false,
        mute: false,
        self_stop: false,
    };
    let mut s = settings(cell, rate);
    s.memory.capacity_s = 20;
    let raw = raw_stimulus(rate);
    let x = samples(&raw);
    let mut dirty = make(s);
    let fs = f64::from(rate);
    let mut tail = x.clone();
    tail.extend(stimulus::tone(1000.0, 0.5, fs, (0.1 * fs) as usize));
    tail.extend(stimulus::silence((0.5 * fs) as usize));
    let mut y = vec![0.0f32; 2048];
    for i in 0..c {
        if i == 1 {
            dirty
                .schedule_change(Stream::Record, 100, KNEE, -4.0, 5)
                .unwrap();
        }
        dirty.start_recording().unwrap();
        let _ = dirty.record(&tail, &mut []).unwrap();
        let _ = dirty.stop_recording(&mut []).unwrap();
        let _ = dirty.process(&x[..2048], &mut y).unwrap();
    }
    assert_eq!(dirty.recording_counter(), c);
    let mut fs2 = *dirty.settings();
    fs2.memory = s.memory;
    let mut fresh = Pipeline::new_at_counter(fs2, c).expect("fresh");
    let mut mismatches = 0;
    for p in [&mut dirty, &mut fresh] {
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
    }
    let (nd, nf) = (dirty.memory_status().count, fresh.memory_status().count);
    if dirty.recording_digest(nd).unwrap() != fresh.recording_digest(nf).unwrap() {
        mismatches += 1;
    }
    if fresh.recording_counter() != c + 1 {
        mismatches += 1;
    }
    dirty.reset();
    fresh.reset();
    let mut yd = vec![0.0f32; 4096];
    let mut yf = vec![0.0f32; 4096];
    let _ = dirty.process(&x[..4096], &mut yd).unwrap();
    let _ = fresh.process(&x[..4096], &mut yf).unwrap();
    mismatches += yd
        .iter()
        .zip(&yf)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    mismatches
}

/// The conversion's `(l, m)` for a rate (device = host · l ÷ m): the harness's own table.
pub fn l_m(rate: u32) -> (u64, u64) {
    match rate {
        6000 => (1, 1),
        8000 => (3, 4),
        16_000 => (3, 8),
        44_100 => (20, 147),
        48_000 => (1, 8),
        88_200 => (10, 147),
        96_000 => (1, 16),
        other => panic!("{other} Hz"),
    }
}

/// The harness's own event-row rule (research R-01): the first removed input sample's host
/// centre, ⌊(k + R_before) · m ÷ l⌋ − L_pre, clamped at 0.
pub fn event_host_positions(events: &[VasEvent], rate: u32, latency_pre: u32) -> Vec<u64> {
    let (l, m) = l_m(rate);
    let mut removed = 0u64;
    events
        .iter()
        .map(|e| {
            let host = (e.output_position + removed) * m / l;
            removed += e.input_length;
            host.saturating_sub(u64::from(latency_pre))
        })
        .collect()
}

/// `STREAM_RECORD` for the tests' assertions.
pub const RECORD_STREAM: u32 = STREAM_RECORD;

// ---- US3: the timeline maps (research R-01, R-04) ---------------------------------------------

use rr_dr60::timeline::{self, KeptOrRemoved, PlaceState};

/// The marker stimulus: four 50 ms bursts at −20 dBFS (longer than the gate's onset time, so
/// each burst opens the gate), 1 kHz tones for the latency convention (a sharp onset) or
/// voiced segments for the codec-on case (the coder reproduces speech, not a pure tone) at the host positions
/// [`marker_positions`], silence between them (the 100 ms hang pauses the gate after each burst,
/// so every burst after the first opens a gap), quantized at the feed point.
pub fn marker_stimulus(rate: u32) -> Vec<f32> {
    marker_stimulus_of(rate, false)
}

/// [`marker_stimulus`] with tone (`voiced` false) or voiced bursts.
pub fn marker_stimulus_of(rate: u32, voiced: bool) -> Vec<f32> {
    let fs = f64::from(rate);
    let n = (2.1 * fs) as usize;
    let mut x = vec![0.0f32; n];
    for (j, b) in marker_positions(rate).iter().enumerate() {
        let burst = if voiced {
            stimulus::voiced(fs, 0.05, -20.0, 0x30 + j as u64)
        } else {
            stimulus::tone(1000.0, 0.1, fs, (0.05 * fs) as usize)
        };
        for (i, v) in burst.iter().enumerate() {
            x[*b as usize + i] = *v;
        }
    }
    x.iter()
        .map(|&v| samples(&[i24_from_sample(v)])[0])
        .collect()
}

/// The bursts' host positions: 0.3, 0.8, 1.3 and 1.8 s.
pub fn marker_positions(rate: u32) -> [u64; 4] {
    let fs = u64::from(rate);
    [fs * 3 / 10, fs * 8 / 10, fs * 13 / 10, fs * 18 / 10]
}

/// The timeline tests' recorder: the default engine with the microphone chain bypassed (its
/// stages 1 – 2 add their own group delay, outside the latency convention), the codec per
/// `codec`, the gate in drop mode with a 100 ms hang (`vas` false: the VAS-bypassed twin).
pub fn marker_settings(rate: u32, codec: bool, vas: bool) -> Settings {
    let mut s = Settings::new(rate);
    s.mic = rr_dr60::MicSettings::BYPASSED;
    s.speech_codec_enabled = codec;
    s.vas.enabled = vas;
    s.vas.hang_ms = 100.0;
    s.memory.capacity_s = 20;
    s
}

/// The shared onset detector: every index whose |x| first exceeds −40 dBFS after at least
/// `quiet` samples below it.
pub fn onsets(x: &[f32], quiet: usize) -> Vec<usize> {
    let thr = 0.01f32;
    let mut out = Vec::new();
    let mut below = quiet; // the stream starts quiet
    for (i, &v) in x.iter().enumerate() {
        if v.abs() > thr {
            if below >= quiet {
                out.push(i);
            }
            below = 0;
        } else {
            below += 1;
        }
    }
    out
}

/// Records `x` (host samples) and returns the engine, the stored samples, the events and the
/// stop.
pub fn record_samples(s: Settings, x: &[f32]) -> (Pipeline, Vec<f32>, Vec<VasEvent>, StopFields) {
    let mut p = Pipeline::new(s).expect("settings");
    p.start_recording().expect("start");
    let mut events = Vec::new();
    for block in x.chunks(1024) {
        let mut ev = vec![VasEvent::default(); p.max_record_events(block.len())];
        let info = p.record(block, &mut ev).expect("record");
        assert!(info.stopped.is_none());
        events.extend_from_slice(&ev[..info.events]);
    }
    let mut ev = vec![VasEvent::default(); p.max_record_events(p.settings().host_rate_hz as usize)];
    let out = p.stop_recording(&mut ev).expect("stop");
    events.extend_from_slice(&ev[..out.flush.events]);
    let n = p.memory_status().count;
    let stored = rr_dr60::__test_hooks::stored_samples(&p, n).to_vec();
    (p, stored, events, StopFields::from(&out))
}

/// The gate's onset time in device samples (`vas.onset_ms` 20 ms, A-024): a burst's first
/// 120 device samples are consumed before the gate opens, so a kept onset sits 20 ms after
/// the burst's start.
pub const GATE_ONSET_DEVICE: u64 = 120;

/// The markers check (US3 AS1, research R-01): (i) the splice arithmetic bit-exact against the
/// VAS-bypassed twin (every kept sample); (ii) the latency convention on the twin's detected
/// onsets (no gate): map (a)'s centre within one device sample of the burst's host position
/// and map (b) within one device sample of the detected onset; (iii) through the gaps: a host
/// position inside each burst's kept part maps to a kept sample whose pre-gate index equals
/// the twin's (exact) and whose centre returns within one device sample, while a position
/// inside the gate's onset reads as removed. Returns `(max_a, max_b, bursts)` in device samples.
pub fn markers_check(rate: u32) -> (f64, f64, usize) {
    let x = marker_stimulus(rate);
    let drop_s = marker_settings(rate, false, true);
    let twin_s = marker_settings(rate, false, false);
    let (p, drop, events, stop) = record_samples(drop_s, &x);
    let (_, twin, twin_events, twin_stop) = record_samples(twin_s, &x);
    assert!(twin_events.is_empty());
    // Four bursts open four gaps (the codec-off stop finds the gate paused and adds none).
    assert_eq!(events.len(), 4, "{rate} Hz: four gaps");
    // (i) the splice identity: drop[k] == twin[k + R(k)] bit for bit.
    let mut removed = 0u64;
    let mut ei = 0usize;
    for (k, v) in drop.iter().enumerate() {
        while ei < events.len() && events[ei].output_position <= k as u64 {
            removed += events[ei].input_length;
            ei += 1;
        }
        let t = k as u64 + removed;
        assert_eq!(
            v.to_bits(),
            twin[t as usize].to_bits(),
            "{rate} Hz: drop sample {k} == twin sample {t}"
        );
    }
    let per_device = f64::from(rate) / 6000.0;
    let latency = u64::from(p.record_latency_pre_encoder_samples());
    let quiet = (0.03 * 6000.0) as usize;
    let twin_onsets = onsets(&twin, quiet);
    assert_eq!(twin_onsets.len(), 4, "{rate} Hz: four bursts in the twin");
    let (mut max_a, mut max_b) = (0.0f64, 0.0f64);
    for (j, b) in marker_positions(rate).iter().enumerate() {
        // (ii) the twin: the latency convention on a detected onset.
        let t = twin_onsets[j] as u64;
        let a = timeline::kept_to_input(t, &twin_s, &[], &twin_stop).expect("rate");
        max_a = max_a.max((a.centre as f64 - *b as f64).abs() / per_device);
        let KeptOrRemoved::Kept(tb) = timeline::input_to_kept(*b, &twin_s, &[]).expect("rate")
        else {
            panic!("{rate} Hz: the twin keeps everything")
        };
        max_b = max_b.max((tb as f64 - t as f64).abs());
        // (iii) through the gaps: a position 40 ms into the burst (inside its kept part).
        let h = b + u64::from(rate) * 4 / 100;
        let KeptOrRemoved::Kept(k) = timeline::input_to_kept(h, &drop_s, &events).expect("rate")
        else {
            panic!("{rate} Hz burst {j}: 40 ms in is kept")
        };
        let pre_gate = rr_dr60::changes::record_landing(h + latency, rate).unwrap();
        let r: u64 = events
            .iter()
            .take_while(|e| e.output_position <= k)
            .map(|e| e.input_length)
            .sum();
        assert_eq!(
            k + r,
            pre_gate,
            "{rate} Hz burst {j}: the kept sample's pre-gate index"
        );
        assert_eq!(
            drop[k as usize].to_bits(),
            twin[pre_gate as usize].to_bits(),
            "{rate} Hz burst {j}"
        );
        let a = timeline::kept_to_input(k, &drop_s, &events, &stop).expect("rate");
        assert!(!a.clamped);
        max_a = max_a.max((a.centre as f64 - h as f64).abs() / per_device);
        // Inside the gate's onset (5 ms into the burst): removed.
        let inside = b + u64::from(rate) * 5 / 1000;
        assert!(
            matches!(
                timeline::input_to_kept(inside, &drop_s, &events).expect("rate"),
                KeptOrRemoved::Removed { .. }
            ),
            "{rate} Hz burst {j}: the onset's samples are removed"
        );
        // The kept onset sits one gate onset after the burst (the detector reads the event).
        let kept_onset = events[j].output_position;
        let a = timeline::kept_to_input(kept_onset, &drop_s, &events, &stop).expect("rate");
        let want = b + GATE_ONSET_DEVICE * u64::from(rate) / 6000;
        max_a = max_a.max((a.centre as f64 - want as f64).abs() / per_device);
    }
    (max_a, max_b, 4)
}

/// The impulse latency convention (US3 AS1, research R-01 (ii)): an impulse through
/// `record_only` lands at `input_to_kept` of its host position within one device sample, and
/// maps back within one device sample's worth of host samples. Returns the two deviations.
pub fn impulse_check(rate: u32) -> (f64, f64) {
    let n0 = (rate / 2) as usize;
    let mut x = stimulus::silence(rate as usize);
    x[n0] = 0.5;
    let s = crate::configs::settings("record_only", rate);
    let (_, stored, events, stop) = record_samples(s, &x);
    assert!(events.is_empty());
    let (peak, _) = stored
        .iter()
        .enumerate()
        .fold((0usize, 0.0f32), |(bi, bv), (i, &v)| {
            if v.abs() > bv { (i, v.abs()) } else { (bi, bv) }
        });
    let got = match timeline::input_to_kept(n0 as u64, &s, &events).expect("rate") {
        KeptOrRemoved::Kept(k) => k,
        other => panic!("{other:?}"),
    };
    let a = timeline::kept_to_input(peak as u64, &s, &events, &stop).expect("rate");
    let per_device = f64::from(rate) / 6000.0;
    (
        (got as f64 - peak as f64).abs(),
        (a.centre as f64 - n0 as f64).abs() / per_device,
    )
}

/// The rectified 12-sample moving-average envelope of `x` (a periodic tone's waveform can
/// lock a correlation one period off; its envelope cannot).
fn envelope(x: &[f32]) -> Vec<f32> {
    let w = 12usize;
    let mut out = vec![0.0f32; x.len()];
    let mut acc = 0.0f32;
    for i in 0..x.len() {
        acc += x[i].abs();
        if i >= w {
            acc -= x[i - w].abs();
        }
        out[i] = acc / w as f32;
    }
    out
}

/// The lag (in samples) at which `probe` best matches `reference` when placed at
/// `at + lag`, lag in −`span` … `span` (a normalized cross-correlation peak).
fn best_lag(reference: &[f32], probe: &[f32], at: usize, span: usize) -> i64 {
    let mut best = (f64::MIN, 0i64);
    for lag in -(span as i64)..=(span as i64) {
        let start = at as i64 + lag;
        if start < 0 || (start as usize) + probe.len() > reference.len() {
            continue;
        }
        let seg = &reference[start as usize..start as usize + probe.len()];
        let (mut dot, mut e1, mut e2) = (0.0f64, 0.0f64, 0.0f64);
        for (a, b) in seg.iter().zip(probe) {
            dot += f64::from(*a) * f64::from(*b);
            e1 += f64::from(*a) * f64::from(*a);
            e2 += f64::from(*b) * f64::from(*b);
        }
        let c = if e1 > 0.0 && e2 > 0.0 {
            dot / rr_dr60_detmath::sqrt(e1 * e2)
        } else {
            0.0
        };
        if c > best.0 {
            best = (c, lag);
        }
    }
    best.1
}

/// The codec-on markers (US3 AS1 (iii)): the drop-mode recording with the codec on, decoded by
/// a 6 kHz player with every playback stage bypassed (the stored form imported); each burst's
/// kept part in the twin (the 30 ms after the gate's onset) cross-correlated against the
/// decoded stream around the kept index the events predict: the lag within two device samples
/// (spec 008's figure). And the alignment read with the full `record_latency_samples()` (the
/// codec's 180 included), which misses by about 30 ms. Returns `(max_lag, miss)`.
pub fn codec_markers_check(rate: u32) -> (f64, f64) {
    let x = marker_stimulus_of(rate, true);
    let drop_s = marker_settings(rate, true, true);
    let (p, _, events, stop) = record_samples(drop_s, &x);
    let (_, twin, _, _) = record_samples(marker_settings(rate, false, false), &x);
    let n = p.memory_status().count;
    let mut bytes = vec![0u8; p.export_len(n).unwrap()];
    p.export(n, &mut bytes).unwrap();
    let mut player_settings = crate::configs::settings("bypass_all", 6000);
    player_settings.speech_codec_enabled = true;
    let mut q = Pipeline::new(player_settings).unwrap();
    let m = q.import(&bytes).unwrap();
    // The envelopes (a periodic waveform can lock a correlation one period off).
    let decoded = envelope(&crate::split_checks::play_to_end(&mut q, m, 1024));
    let twin = envelope(&twin);
    // Four gaps; the codec-on stop holds the gate open for its flush and adds a fifth event.
    assert_eq!(events.len(), 5, "{rate} Hz: four gaps and the stop's");
    let latency = u64::from(p.record_latency_pre_encoder_samples());
    let per_device = f64::from(rate) / 6000.0;
    let (mut max_lag, mut miss) = (0.0f64, 0.0f64);
    for (j, b) in marker_positions(rate).iter().enumerate() {
        // The kept onset the events predict, and its pre-gate twin index.
        let k = events[j].output_position;
        let r: u64 = events
            .iter()
            .take_while(|e| e.output_position <= k)
            .map(|e| e.input_length)
            .sum();
        let t = (k + r) as usize;
        // The envelopes' probe spans the burst's kept part from well after the onset (the
        // twin's envelope is already high before `t`, the decoded one rises at `k`) through
        // its end (tone → silence): the kept part is 300 − 120 = 180 samples, so [60, 300).
        let probe = &twin[t + 30..t + 300];
        let lag = best_lag(&decoded, probe, k as usize + 30, 30);
        max_lag = max_lag.max(lag.unsigned_abs() as f64);
        // The map's alignment of the kept onset, against the same reading with the wrong
        // latency (the codec's 180 kept samples included): the miss is about 30 ms.
        let a = timeline::kept_to_input(k, &drop_s, &events, &stop).unwrap();
        let full = u64::from(p.record_latency_samples());
        let wrong = ((k + r) * l_m(rate).1 / l_m(rate).0).saturating_sub(full) as f64;
        miss = miss.max((wrong - a.centre as f64).abs() / per_device);
        let _ = (b, latency);
    }
    (max_lag, miss)
}

/// The one-pass alignment (US3 AS2): the marker stimulus through the one-pass with every
/// stage after the gate bypassed (the output is the kept stream at the host rate); each output
/// onset maps to one gate onset after its burst's input position, within two device samples.
/// Returns the largest deviation in device samples.
pub fn onepass_check(rate: u32) -> f64 {
    let x = marker_stimulus(rate);
    let mut s = marker_settings(rate, false, true);
    s.playback_stage_enabled = false;
    s.output = rr_dr60::OutputSettings::BYPASSED;
    let mut p = Pipeline::new(s).unwrap();
    let mut y = Vec::with_capacity(x.len());
    let mut events = Vec::new();
    let mut out = vec![0.0f32; 1024];
    for block in x.chunks(1024) {
        let mut ev = vec![VasEvent::default(); p.max_events(block.len())];
        let info = p
            .process_with_events(block, &mut out[..block.len()], &mut ev)
            .unwrap();
        events.extend_from_slice(&ev[..info.events]);
        y.extend_from_slice(&out[..info.produced]);
    }
    let quiet = (0.03 * f64::from(rate)) as usize;
    let out_onsets = onsets(&y, quiet);
    assert_eq!(
        out_onsets.len(),
        4,
        "{rate} Hz: four bursts out ({})",
        out_onsets.len()
    );
    let per_device = f64::from(rate) / 6000.0;
    let mut max_dev = 0.0f64;
    for (j, b) in marker_positions(rate).iter().enumerate() {
        let (input, clamped) =
            timeline::onepass_output_to_input(out_onsets[j] as u64, p.latency_samples(), &events);
        assert!(!clamped);
        let want = b + GATE_ONSET_DEVICE * u64::from(rate) / 6000;
        max_dev = max_dev.max((input as f64 - want as f64).abs() / per_device);
    }
    max_dev
}

/// The ten corner cases of map (c) (research R-04): `(name, start level, requests as
/// (device output index the request should land on, level))`; the host positions are the
/// smallest P with `speed_landing(P) = d`.
pub fn place_cases() -> Vec<(&'static str, u8, Vec<(u64, u8)>)> {
    let n = u64::from(rr_dr60::speed::CROSSFADE_SAMPLES);
    vec![
        ("first output", 3, vec![(0, 5)]),
        (
            "landing on the fade's last output",
            3,
            vec![(100, 5), (100 + n - 1, 1)],
        ),
        (
            "waiting request equal to the active level",
            3,
            vec![(100, 5), (120, 5)],
        ),
        ("level 3 during a wait", 5, vec![(100, 1), (120, 3)]),
        ("request after the latch", 3, vec![(2990, 5)]),
        (
            "two requests inside one fade",
            3,
            vec![(100, 5), (130, 1), (160, 4)],
        ),
        (
            "deferred level 3 overtaken by the latch",
            5,
            vec![(2990, 3)],
        ),
        (
            "waited request equal to the old level",
            3,
            vec![(100, 5), (130, 3)],
        ),
        (
            "two entries on one device output",
            3,
            vec![(100, 5), (100, 1)],
        ),
        ("request in the tail", 1, vec![(2995, 5)]),
    ]
}

/// The smallest host position landing on device output `d` at `rate`.
pub fn host_position_for(d: u64, rate: u32) -> u64 {
    if d == 0 {
        return 0;
    }
    // d(P) = ⌊(P − 1) · 6000 ÷ rate⌋ + 1 = d  ⇔  (P − 1) · 6000 ÷ rate ∈ [d − 1, d)
    let p = ((d - 1) * u64::from(rate)).div_ceil(6000) + 1;
    assert_eq!(rr_dr60::changes::speed_landing(p, rate).unwrap(), d);
    p
}

/// Plays a 3000-sample recording through `speed_only` at `rate` from `start` with the case's
/// requests scheduled, collecting the per-output place trace and the log; returns the number
/// of outputs at which [`timeline::place_at`] disagrees with the trace, and the outputs.
pub fn place_case_mismatches(rate: u32, start: u8, requests: &[(u64, u8)]) -> (usize, usize) {
    let x = stimulus::stored_tone(1000.0, 0.5, 3000);
    let (mut p, number) = crate::configs::stored_recording(
        &crate::checks::standard,
        crate::configs::settings("speed_only", rate),
        &x,
    );
    p.set_playback_speed(start).unwrap();
    for (i, (d, level)) in requests.iter().enumerate() {
        let pos = host_position_for(*d, rate)
            + if i > 0 && requests[i - 1].0 == *d {
                1
            } else {
                0
            };
        p.schedule_change(Stream::Playback, pos, 2, f64::from(*level), i as u64 + 1)
            .unwrap();
    }
    p.start_playback(number).unwrap();
    let mut trace: Vec<PlaceState> = Vec::new();
    let mut log: Vec<ChangeEntry> = Vec::new();
    let mut out = vec![0.0f32; 1024];
    let mut ch = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    loop {
        let info = p.play_with_changes(&mut out, &mut ch).unwrap();
        log.extend_from_slice(&ch[..info.changes]);
        let t = rr_dr60::__test_hooks::speed_place_trace(&p);
        trace.extend_from_slice(&t);
        if info.finished {
            break;
        }
    }
    // The stage emits silence after its final count without moving: compare up to the first
    // repeated state (the walker has no final count and keeps advancing).
    let own_and_tail = 1 + trace.windows(2).take_while(|w| w[0] != w[1]).count();
    let mut mismatches = 0;
    for (i, want) in trace.iter().enumerate().take(own_and_tail) {
        let got = timeline::place_at(i as u64, rate, start, 3000, &log).unwrap();
        if got != *want {
            if mismatches < 3 {
                eprintln!("{rate} Hz output {i}: trace {want:?} walker {got:?}");
            }
            mismatches += 1;
        }
    }
    (mismatches, own_and_tail)
}

// ---- The six mutants (spec 020 SC-003, SC-005; tasks.md T023), each an alternative
// implementation that its named oracle must reject.

/// The mutants of the record contract.
pub mod mutants {
    use super::*;

    /// The digest over every exported byte but the last: `digest_equals_export_hash` rejects
    /// it.
    pub fn digest_without_last_byte(p: &Pipeline, number: u32) -> u64 {
        let mut bytes = vec![0u8; p.export_len(number).unwrap()];
        p.export(number, &mut bytes).unwrap();
        rr_dr60::digest::fnv1a64(&bytes[..bytes.len() - 1])
    }

    /// The record's checksum over the bytes without the line end before the checksum line:
    /// the reader rejects it (the round trip's read).
    pub fn checksum_wrong_span(record: &[u8]) -> Vec<u8> {
        let body_end = record[..record.len() - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .unwrap()
            + 1;
        let h = rr_dr60::digest::fnv1a64(&record[..body_end - 1]);
        let mut v = record[..body_end].to_vec();
        v.extend_from_slice(format!("record_checksum = fnv1a64:{h:016x}\n").as_bytes());
        v
    }

    /// The record written with the engine's current settings instead of the slot block's:
    /// after a landed change the bytes differ from the true record (US1 AS1 rejects it).
    pub fn settings_from_engine(p: &Pipeline, record: &[u8]) -> Vec<u8> {
        let mut r = readout(record);
        r.settings = *p.settings();
        let rows: Vec<_> = (0..r.event_count)
            .map(|i| record::record_event(record, i).unwrap())
            .collect();
        let mut changes = vec![ChangeEntry::default(); r.change_count as usize];
        let n = record::record_changes(record, 0, &mut changes);
        changes.truncate(n);
        let mut out = vec![0u8; record::len(&r, &rows, &changes)];
        record::write(&r, &rows, &changes, &mut out).unwrap();
        out
    }

    /// Map (a) with the codec's 180 kept samples added to the latency (the draft's error):
    /// the markers reject it (the centre moves by 180 device samples).
    pub fn alignment_with_codec_delay(
        k: u64,
        s: &Settings,
        events: &[VasEvent],
        stop: &StopFields,
    ) -> u64 {
        let a = timeline::kept_to_input(k, s, events, stop).unwrap();
        let (l, m) = l_m(s.host_rate_hz);
        a.centre.saturating_sub(180 * m / l)
    }

    /// Map (c) without the wait: a request during a crossfade applies at once (spec 019's
    /// rejected alternative); the corner case "two requests inside one fade" rejects it.
    pub fn place_at_without_wait(
        index: u64,
        rate: u32,
        start: u8,
        unit_count: u64,
        entries: &[ChangeEntry],
    ) -> PlaceState {
        let numerator = |level: u8| -> u64 { [3u64, 4, 5, 6, 7][(level - 1) as usize] };
        let level_of = |k: u64| -> u8 { [1u8, 2, 3, 4, 5][(k - 3) as usize] };
        let mut p5 = 0u64;
        let mut active = numerator(start);
        let mut requested = active;
        let mut latched = unit_count == 0;
        let mut fade: Option<(u64, u64, u32)> = None;
        let mut started = false;
        let n = u64::from(rr_dr60::speed::CROSSFADE_SAMPLES);
        let mut pending: Vec<(u64, u8)> = entries
            .iter()
            .filter(|e| e.control == 2)
            .map(|e| {
                (
                    rr_dr60::changes::speed_landing(e.position, rate).unwrap(),
                    e.after as u8,
                )
            })
            .collect();
        pending.reverse();
        let mut st = PlaceState::default();
        for i in 0..=index {
            while let Some(&(d, level)) = pending.last() {
                if d > i {
                    break;
                }
                requested = numerator(level);
                pending.pop();
            }
            if p5 >= 5 * unit_count {
                latched = true;
            }
            // The mutant: no `fade.is_none()` test.
            if !latched && requested != active && !(requested == 5 && p5 % 5 != 0) {
                let old = active;
                active = requested;
                if started {
                    fade = Some((old, p5, 0));
                }
            }
            st = PlaceState {
                place_fifths: p5,
                level: level_of(active),
                fading: false,
                old_place_fifths: 0,
                latched,
            };
            if let Some((ok, op5, k)) = fade {
                let k = k + 1;
                if u64::from(k) < n {
                    st.fading = true;
                    st.old_place_fifths = op5;
                    fade = Some((ok, op5 + ok, k));
                } else {
                    fade = None;
                }
            }
            started = true;
            p5 += active;
            if p5 >= 5 * unit_count {
                latched = true;
            }
            st.latched = latched;
        }
        st
    }
}
