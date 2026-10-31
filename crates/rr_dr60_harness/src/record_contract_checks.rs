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
            if self.self_stop { "self_stop" } else { "stop_call" }
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
    s.vas.mode = if cell.mute { VasMode::Mute } else { VasMode::Drop };
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
    assert!(st.remaining_samples >= remaining, "already fuller than asked");
    let units = (st.remaining_samples - remaining) / p.encoding().unit_samples() as u64;
    if units > 0 {
        let _ = p.import(&filler(p.encoding(), units)).expect("the filler fits");
    }
    assert_eq!(p.memory_status().remaining_samples, remaining);
}

/// The two record-side changes a `changes` cell schedules (positions in host input samples).
pub fn cell_changes(rate: u32) -> [(u64, u32, f64, u64); 2] {
    let fs = u64::from(rate);
    // The knee lands below the −20 dBFS voice's peaks, so the entry is effective (the
    // limiter engages from it on); the hiss change rides on the second voiced segment.
    [
        (fs * 3 / 10, KNEE, -30.0, 7),
        (fs * 9 / 10, HISS, 3.0, 8),
    ]
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
        let info = p.record_with_changes(block, &mut ev, &mut log).expect("record");
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
    assert_eq!(p.processing_record_write(number, &run, &mut record).expect("write"), n);
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
            if codec { Delta::EntryMovedFar } else { Delta::EntryMoved },
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
    assert_eq!(record::record_changes(record, 0, &mut changes), changes.len());
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
            .record(&vec![0.0f32; units as usize * p.encoding().unit_samples()], &mut [])
            .unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
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
    mismatches += yd.iter().zip(&yf).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
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
