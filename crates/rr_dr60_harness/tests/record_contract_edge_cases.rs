//! Spec 020: the edge cases (tasks.md T022; spec › Edge Cases; plan review coach 4, hardware
//! B3, B5, B8).

use rr_dr60::record::{self, RecordReadout, RunData, StopHow, flags};
use rr_dr60::timeline::{self, KeptOrRemoved};
use rr_dr60::{ChangeEntry, Error, Pipeline, Settings, Stream, VasEvent};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

const RATE: u32 = 48_000;

fn cell(codec: bool) -> Cell {
    Cell {
        codec,
        changes: true,
        mute: false,
        self_stop: false,
    }
}

/// An entry landing in the stop's flush (between the stop position and the input total) is a
/// consistent run and reproduces (hardware B3).
#[test]
fn flush_entry_is_consistent() {
    let raw = rc::raw_stimulus(RATE);
    let mut s = rc::settings(cell(true), RATE);
    s.speech_codec_enabled = true;
    let mut p = Pipeline::new(s).unwrap();
    let pre = u64::from(p.record_latency_pre_encoder_samples());
    assert!(pre > 100, "48 kHz: a flush of {pre} samples");
    p.schedule_change(
        Stream::Record,
        raw.len() as u64 + pre / 2,
        rc::KNEE,
        -30.0,
        9,
    )
    .unwrap();
    let (p, run) = rc::record_run_in(
        &mut p,
        Cell {
            changes: false,
            ..cell(true)
        },
        &raw,
    );
    assert_eq!(run.changes.len(), 1, "the flush entry");
    let e = run.changes[0];
    assert!(
        e.position >= run.stop.stop_input_position && e.position < run.stop.input_samples_total
    );
    assert_eq!(rc::readout(&run.record).change_count, 1);
    assert_eq!(
        rc::re_render(&run.record, &raw, 1024, Delta::None),
        run.digest
    );
    let _ = p;
}

/// With the codec on, an event up to the look-ahead past the stored length is consistent;
/// one beyond is refused (hardware B5).
#[test]
fn lookahead_event_is_consistent() {
    let raw = rc::raw_stimulus(RATE);
    let (p, run) = rc::record_run(&standard, RATE, cell(true), &raw);
    let length = p.recording_length(run.number).unwrap();
    // The last event (the stop's flush) moved to the look-ahead's edge keeps the count.
    let mut ev = run.events.clone();
    *ev.last_mut().unwrap() = VasEvent::new(length + 60, 1);
    let run_data = RunData {
        events: &ev,
        changes: &run.changes,
        stop: run.stop,
    };
    assert!(p.processing_record_len(run.number, &run_data).is_ok());
    *ev.last_mut().unwrap() = VasEvent::new(length + 61, 1);
    let run_data = RunData {
        events: &ev,
        changes: &run.changes,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &run_data).unwrap_err(),
        Error::InconsistentRun {
            reason: "an event past the recording"
        }
    );
}

/// More rows than the engine counted is refused.
#[test]
fn surplus_refused() {
    let raw = rc::raw_stimulus(RATE);
    let (p, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let mut ev = run.events.clone();
    ev.push(*ev.last().unwrap());
    let run_data = RunData {
        events: &ev,
        changes: &run.changes,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &run_data).unwrap_err(),
        Error::InconsistentRun {
            reason: "more events than counted"
        }
    );
    let mut ch = run.changes.clone();
    ch.push(ChangeEntry::new(
        ch[1].position + 1,
        rc::KNEE,
        -30.0,
        -29.0,
        3,
        rr_dr60::STREAM_RECORD,
    ));
    let run_data = RunData {
        events: &run.events,
        changes: &ch,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &run_data).unwrap_err(),
        Error::InconsistentRun {
            reason: "more entries than counted"
        }
    );
}

/// A self-stopped recording's run is built from the `RecordInfo` (no stop outcome exists), and
/// the record reads `memory_full` with the padding start filled with the length (hardware B8).
#[test]
fn self_stop_run_from_record_info() {
    let raw = rc::raw_stimulus(RATE);
    let (p, run) = rc::record_run(
        &standard,
        RATE,
        Cell {
            self_stop: true,
            ..cell(true)
        },
        &raw,
    );
    assert_eq!(run.stop.how, StopHow::MemoryFull);
    assert_eq!(run.stop.stop_input_position, run.stop.input_samples_total);
    let r = rc::readout(&run.record);
    assert_eq!(r.stop.how, StopHow::MemoryFull);
    assert_eq!(
        r.stop.padding_start_sample,
        p.recording_length(run.number).unwrap()
    );
    assert_eq!(r.flags, 0);
    assert_eq!(
        rc::re_render(&run.record, &raw, 1024, Delta::None),
        run.digest
    );
}

/// An empty slot, a record requested while recording, and the counter of an imported one.
#[test]
fn empty_slot_in_progress_and_imported() {
    let raw = rc::raw_stimulus(RATE);
    let (mut p, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let data = RunData {
        events: &run.events,
        changes: &run.changes,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(7, &data).unwrap_err(),
        Error::NoSuchRecording { number: 7 }
    );
    assert_eq!(p.recording_counter_of(1).unwrap(), Some(0));
    p.start_recording().unwrap();
    assert_eq!(
        p.processing_record_len(1, &data).unwrap_err(),
        Error::RecordingInProgress
    );
    assert!(
        p.recording_digest(1).is_ok(),
        "the digest is allowed in any state"
    );
    let _ = p.stop_recording(&mut []).unwrap();
}

/// An empty collection for a run with non-zero totals: a valid record flagged incomplete, whose
/// re-render fails the digest (the entries are not replayed).
#[test]
fn empty_collection_flagged_and_fails() {
    let raw = rc::raw_stimulus(RATE);
    let (p, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let data = RunData {
        events: &[],
        changes: &[],
        stop: run.stop,
    };
    let mut rec = vec![0u8; p.processing_record_len(run.number, &data).unwrap()];
    p.processing_record_write(run.number, &data, &mut rec)
        .unwrap();
    let r = rc::readout(&rec);
    assert_eq!(r.flags, flags::INCOMPLETE_RUN);
    assert_ne!(rc::re_render(&rec, &raw, 1024, Delta::None), run.digest);
}

/// Entries from another stream, out of order, or past the total: refused with their reason.
#[test]
fn other_stream_and_order() {
    let raw = rc::raw_stimulus(RATE);
    let (p, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let mut ch = run.changes.clone();
    ch[0].stream = rr_dr60::STREAM_ONE_PASS;
    let d = RunData {
        events: &run.events,
        changes: &ch,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &d).unwrap_err(),
        Error::InconsistentRun {
            reason: "not the record stream"
        }
    );
    ch[0].stream = 0;
    let d = RunData {
        events: &run.events,
        changes: &ch,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &d).unwrap_err(),
        Error::InconsistentRun {
            reason: "no stream"
        }
    );
    let mut ch = run.changes.clone();
    ch[1].position = run.stop.input_samples_total;
    let d = RunData {
        events: &run.events,
        changes: &ch,
        stop: run.stop,
    };
    assert_eq!(
        p.processing_record_len(run.number, &d).unwrap_err(),
        Error::InconsistentRun {
            reason: "an entry at or past the final input position"
        }
    );
}

/// A stop moved by less than one frame's input, or an input change inside a removed gap, need
/// not move the digest; the record's own fields move and two records differ.
#[test]
fn small_moves_change_the_record_not_necessarily_the_digest() {
    let raw = rc::raw_stimulus(RATE);
    let (_, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let r = rc::readout(&run.record);
    let mut moved = raw.clone();
    // Inside the first gap (0.5 … 0.9 s): removed by the gate (a code of 1000 is −78 dBFS, far
    // below the −40 dBFS threshold: the quiet-sample case; a loud sample would reopen the gate).
    moved[RATE as usize * 7 / 10] = 1000;
    let got = rc::re_render(&run.record, &moved, 1024, Delta::None);
    assert_eq!(
        got, run.digest,
        "a sample inside a removed gap cannot move a stored byte"
    );
    // A second recording stopped one input sample earlier: its record differs in the stop
    // position (the reader reports both), whether or not the digest moved.
    let (_, run2) = rc::record_run(&standard, RATE, cell(false), &raw[..raw.len() - 1]);
    let r2 = rc::readout(&run2.record);
    assert_eq!(r2.stop.stop_input_position + 1, r.stop.stop_input_position);
    assert_ne!(run2.record, run.record);
}

/// The counter near its maximum wraps, as the engine's own counter does.
#[test]
fn counter_near_max_wraps() {
    let mut p = Pipeline::new_at_counter(Settings::new(6000), u64::MAX).unwrap();
    assert_eq!(p.recording_counter(), u64::MAX);
    p.start_recording().unwrap();
    let _ = p.record(&[0.5f32; 600], &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    assert_eq!(p.recording_counter(), 0);
    assert_eq!(p.recording_counter_of(1).unwrap(), Some(u64::MAX));
}

/// The maps at the ends: clamped and reported; a removed position names its event.
#[test]
fn map_ends() {
    let raw = rc::raw_stimulus(RATE);
    let (_, run) = rc::record_run(&standard, RATE, cell(false), &raw);
    let s = rc::settings(cell(false), RATE);
    let a = timeline::kept_to_input(0, &s, &run.events, &run.stop).unwrap();
    assert!(a.clamped && a.centre == 0);
    let a = timeline::kept_to_input(u64::MAX / 16, &s, &run.events, &run.stop).unwrap();
    assert!(a.clamped && a.last == run.stop.stop_input_position - 1 && a.centre <= a.last);
    let inside = RATE as u64 * 7 / 10;
    assert!(matches!(
        timeline::input_to_kept(inside, &s, &run.events).unwrap(),
        KeptOrRemoved::Removed { .. }
    ));
    assert_eq!(
        timeline::onepass_output_to_input(0, 483, &[], true),
        (0, true)
    );
}

/// Erase moves the blocks; a memory-keeping reconfigure keeps them; a reset keeps them.
#[test]
fn erase_reconfigure_reset_and_the_blocks() {
    let raw = rc::raw_stimulus(RATE);
    let (mut p, run1) = rc::record_run(&standard, RATE, cell(false), &raw);
    let (mut p, run2) = rc::record_run_in(&mut p, cell(false), &raw[..raw.len() / 2]);
    assert_eq!(p.recording_counter_of(2).unwrap(), Some(1));
    p.erase(1).unwrap();
    assert_eq!(p.recording_counter_of(1).unwrap(), Some(1));
    let data = RunData {
        events: &run2.events,
        changes: &run2.changes,
        stop: run2.stop,
    };
    let mut rec = vec![0u8; p.processing_record_len(1, &data).unwrap()];
    p.processing_record_write(1, &data, &mut rec).unwrap();
    assert_eq!(rc::readout(&rec).recording_counter, 1);
    p.reset();
    let mut s = *p.settings();
    s.memory.max_recordings += 1;
    p.reconfigure(s).unwrap();
    assert_eq!(p.recording_counter_of(1).unwrap(), Some(1));
    let _ = run1;
    // A record naming a setting this engine does not know is reported; a missing required
    // line refused (the pure reader's rules through the engine).
    let text = String::from_utf8(rec.clone())
        .unwrap()
        .replace("setting.tap = ", "setting.future = 1\nsetting.tap = ");
    let mut doctored = text.into_bytes();
    let body_end = doctored[..doctored.len() - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap()
        + 1;
    let h = rr_dr60::digest::fnv1a64(&doctored[..body_end]);
    doctored.truncate(body_end);
    doctored.extend_from_slice(format!("record_checksum = fnv1a64:{h:016x}\n").as_bytes());
    let mut r = RecordReadout::default();
    record::read(&doctored, &mut r).unwrap();
    assert_eq!(r.flags, flags::UNKNOWN_SETTINGS);
}
