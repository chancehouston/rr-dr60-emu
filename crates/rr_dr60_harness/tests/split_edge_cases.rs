//! Spec 006 Edge Cases, one test each (task T038), for what the story tests and the unit tests
//! do not already cover at the harness level: plain assertions through the public API.

use rr_dr60::{
    Error, Pipeline, Route, Setting, Settings, State, StopReason, StopResult, VasEvent, VasMode,
};
use rr_dr60_harness::{configs, split_checks, stimulus};

/// A −20 dBFS 1 kHz tone of `secs` at `rate`.
fn tone(rate: u32, secs: f64) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone(1000.0, stimulus::amplitude(-20.0), fs, (secs * fs) as usize)
}

/// A 10 s / 3-recording engine of `config` at `rate`.
fn small(config: &str, rate: u32) -> Pipeline {
    Pipeline::new(configs::memory_capacity(
        configs::settings(config, rate),
        10,
        3,
    ))
    .unwrap()
}

/// Records `x` in one block and stops; the recording's number.
fn record_one(p: &mut Pipeline, x: &[f32]) -> u32 {
    p.start_recording().unwrap();
    assert!(p.record(x, &mut []).unwrap().stopped.is_none());
    match p.stop_recording(&mut []).unwrap().result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    }
}

/// Memory fills inside the stop flush: the flush is cut at the boundary like any block.
#[test]
fn memory_fills_inside_the_flush() {
    let mut p = Pipeline::new(configs::memory_capacity(
        configs::settings("default_agc", 48_000),
        1,
        3,
    ))
    .unwrap();
    let x = tone(48_000, 2.0);
    p.start_recording().unwrap();
    let mut fed = 0;
    while p.memory_status().remaining_samples > 5 {
        assert!(
            p.record(&x[fed..fed + 8], &mut [])
                .unwrap()
                .stopped
                .is_none()
        );
        fed += 8;
    }
    let room = p.memory_status().remaining_samples as usize;
    assert!(room > 0 && room <= 5);
    let out = p.stop_recording(&mut []).unwrap();
    assert_eq!(out.flush.stopped, Some(StopReason::MemoryFull));
    assert_eq!(out.flush.stored, room, "the flush is cut at the boundary");
    assert!(matches!(
        out.result,
        StopResult::Kept {
            number: 1,
            length: 6000
        }
    ));
    assert_eq!(p.state().state, State::Idle);
    assert_eq!(p.memory_status().remaining_samples, 0);
}

/// Stop while the VAS is paused: the recording ends at the last stored sample, the flush
/// stores nothing in drop mode, and in mute mode the open region is closed by one event.
#[test]
fn stop_while_paused() {
    for rate in [6000, 48_000] {
        let fs = f64::from(rate);
        let mut x = tone(rate, 1.0);
        x.resize((3.0 * fs) as usize, 0.0); // 2 s of silence outlasts the 1 s hang (A-023)
        // Drop mode.
        let mut p = small("default_vas", rate);
        p.start_recording().unwrap();
        let info = p.record(&x, &mut []).unwrap();
        assert!(
            info.paused,
            "{rate} Hz: the VAS should be paused at the end"
        );
        let stored = p.memory_status().used_samples;
        let mut events = vec![VasEvent::default(); 8];
        let out = p.stop_recording(&mut events).unwrap();
        assert_eq!(
            out.flush.stored, 0,
            "{rate} Hz: paused time is not recorded"
        );
        assert_eq!(out.flush.events, 0);
        assert!(matches!(out.result, StopResult::Kept { length, .. } if length == stored));
        // Mute mode: the muted region is stored as silence and closed at the stop.
        let mut s = configs::settings("default_vas", rate);
        s.vas.mode = VasMode::Mute;
        let mut p = Pipeline::new(configs::memory_capacity(s, 10, 3)).unwrap();
        p.start_recording().unwrap();
        let info = p.record(&x, &mut events).unwrap();
        assert!(
            info.paused && info.events == 0,
            "{rate} Hz: the region is still open"
        );
        let out = p.stop_recording(&mut events).unwrap();
        assert_eq!(
            out.flush.events, 1,
            "{rate} Hz: the open region gets its event"
        );
        let length = match out.result {
            StopResult::Kept { length, .. } => length,
            other => panic!("{other:?}"),
        };
        assert_eq!(events[0].output_position + events[0].input_length, length);
    }
}

/// Route change during playback: reported at once, the sound unchanged in this version.
#[test]
fn route_change_during_playback() {
    let mut p = small("default_agc", 48_000);
    let n = record_one(&mut p, &tone(48_000, 0.5));
    let speaker = split_checks::play_to_end(&mut p, n, 512);
    p.start_playback(n).unwrap();
    let mut out = Vec::new();
    let mut buf = [0.0f32; 512];
    let mut switched = false;
    loop {
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        if !switched {
            p.set_route(Route::Earphone);
            assert_eq!(p.state().route, Route::Earphone);
            assert_eq!(p.state().state, State::Playing);
            switched = true;
        }
        if info.finished {
            break;
        }
    }
    assert_eq!(out, speaker);
    assert_eq!(p.route(), Route::Earphone);
}

/// Reconfigure is refused while recording or playing (settings unchanged); a capacity below
/// one second or a count below one is refused at construction and at reconfiguration.
#[test]
fn reconfigure_refusals_and_capacity_too_small() {
    let mut p = small("default_agc", 48_000);
    let settings = *p.settings();
    let other = configs::settings("default_agc", 96_000);
    p.start_recording().unwrap();
    assert_eq!(p.reconfigure(other), Err(Error::RecordingInProgress));
    assert_eq!(*p.settings(), settings);
    p.reset();
    let n = record_one(&mut p, &tone(48_000, 0.2));
    p.start_playback(n).unwrap();
    assert_eq!(p.reconfigure(other), Err(Error::PlaybackInProgress));
    assert_eq!(*p.settings(), settings);
    p.stop_playback().unwrap();
    for (bad, field) in [
        (
            configs::memory_capacity(settings, 0, 3),
            Setting::MemoryCapacityS,
        ),
        (
            configs::memory_capacity(settings, 10, 0),
            Setting::MemoryMaxRecordings,
        ),
    ] {
        assert_eq!(
            Pipeline::new(bad).err(),
            Some(Error::InvalidSetting { setting: field })
        );
        assert_eq!(
            p.reconfigure(bad),
            Err(Error::InvalidSetting { setting: field })
        );
        assert_eq!(*p.settings(), settings);
    }
}

/// Reset returns to idle, clears the path state and leaves the memory's recordings; a
/// recording in progress is discarded without flush; a playback stops without drain.
#[test]
fn reset_keeps_the_memory() {
    let mut p = small("default_vas", 48_000);
    let x = tone(48_000, 0.3);
    let a = record_one(&mut p, &x);
    let b = record_one(&mut p, &tone(48_000, 0.2));
    let want = split_checks::play_to_end(&mut p, a, 512);
    p.start_playback(b).unwrap();
    let _ = p.play(&mut [0.0f32; 100]).unwrap();
    p.reset();
    assert_eq!(p.state().state, State::Idle);
    assert_eq!(p.memory_status().count, 2);
    assert_eq!(split_checks::play_to_end(&mut p, a, 512), want);
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    assert_eq!(p.state().current, Some(3));
    p.reset();
    assert_eq!(
        p.memory_status().count,
        2,
        "the recording in progress is discarded"
    );
    assert_eq!(
        p.memory_status().used_samples,
        p.recording_length(1).unwrap() + p.recording_length(2).unwrap()
    );
    assert_eq!(p.stop_recording(&mut []), Err(Error::NotRecording));
}

/// Bad numbers: 0 and above the count are "no such recording" for every call that takes one.
#[test]
fn bad_numbers() {
    let mut p = small("default_agc", 48_000);
    for n in [0, 1, 2] {
        assert_eq!(
            p.start_playback(n),
            Err(Error::NoSuchRecording { number: n })
        );
        assert_eq!(p.erase(n), Err(Error::NoSuchRecording { number: n }));
        assert_eq!(p.export_len(n), Err(Error::NoSuchRecording { number: n }));
        assert_eq!(
            p.export(n, &mut [0u8; 64]),
            Err(Error::NoSuchRecording { number: n })
        );
        assert_eq!(
            p.recording_length(n),
            Err(Error::NoSuchRecording { number: n })
        );
    }
    assert_eq!(record_one(&mut p, &tone(48_000, 0.1)), 1);
    for n in [0, 2, u32::MAX] {
        assert_eq!(
            p.start_playback(n),
            Err(Error::NoSuchRecording { number: n })
        );
        assert_eq!(p.erase(n), Err(Error::NoSuchRecording { number: n }));
    }
    assert_eq!(p.state().state, State::Idle);
    assert_eq!(p.memory_status().count, 1);
}

/// An empty byte sequence is not a form; the memory is unchanged.
#[test]
fn empty_import() {
    let mut p = small("default_agc", 48_000);
    let before = p.memory_status();
    assert_eq!(p.import(&[]), Err(Error::UnknownForm));
    assert_eq!(p.memory_status(), before);
}

/// Host rate change between record and playback: the playback runs at the engine's rate at
/// the time playback starts, with the length rule at that rate, and equals a fresh engine's
/// playback of the same bytes.
#[test]
fn host_rate_change_between_record_and_playback() {
    let mut p = small("default_agc", 48_000);
    let n = record_one(&mut p, &tone(48_000, 0.5));
    let bytes = split_checks::export_bytes(&p, n);
    let k = p.recording_length(n).unwrap();
    p.reconfigure(configs::memory_capacity(
        configs::settings("default_agc", 96_000),
        10,
        3,
    ))
    .unwrap();
    let out = split_checks::play_to_end(&mut p, n, 512);
    assert_eq!(
        out.len() as u64,
        split_checks::playback_len(k, 96_000) + u64::from(p.drain_samples())
    );
    let mut fresh = small("default_agc", 96_000);
    assert_eq!(fresh.import(&bytes), Ok(1));
    assert_eq!(split_checks::play_to_end(&mut fresh, 1, 512), out);
}

/// The one-pass processing while recording or playing: refused with the activity, the
/// in-place forms produce nothing and leave the buffer unchanged (FR-009).
#[test]
fn one_pass_refused_while_active() {
    let mut p = small("default_vas", 48_000);
    let x = tone(48_000, 0.1);
    let mut y = vec![7.0f32; x.len()];
    let mut events = vec![VasEvent::default(); p.max_events(x.len())];
    p.start_recording().unwrap();
    assert_eq!(
        p.process(&x, &mut y).err(),
        Some(Error::RecordingInProgress)
    );
    assert_eq!(
        p.process_with_events(&x, &mut y, &mut events).err(),
        Some(Error::RecordingInProgress)
    );
    let mut in_place = x.clone();
    assert_eq!(p.process_in_place(&mut in_place).produced, 0);
    assert_eq!(
        p.process_in_place_with_events(&mut in_place, &mut events)
            .produced,
        0
    );
    assert_eq!(in_place, x);
    assert!(y.iter().all(|&v| v == 7.0));
    assert!(p.record(&x, &mut []).unwrap().stopped.is_none());
    let n = match p.stop_recording(&mut []).unwrap().result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    p.start_playback(n).unwrap();
    assert_eq!(p.process(&x, &mut y).err(), Some(Error::PlaybackInProgress));
    assert_eq!(p.process_in_place(&mut in_place).produced, 0);
    assert_eq!(in_place, x);
    assert_eq!(p.state().state, State::Playing);
}

/// The count limit is reached by a recording still running: it runs to the stop; only a
/// further start is refused.
#[test]
fn count_limit_reached_by_a_running_recording() {
    let mut p = Pipeline::new(configs::memory_capacity(
        configs::settings("default_agc", 48_000),
        10,
        1,
    ))
    .unwrap();
    p.start_recording().unwrap();
    assert_eq!(p.state().current, Some(1));
    for _ in 0..3 {
        assert!(
            p.record(&tone(48_000, 0.2), &mut [])
                .unwrap()
                .stopped
                .is_none()
        );
    }
    assert!(matches!(
        p.stop_recording(&mut []).unwrap().result,
        StopResult::Kept { number: 1, .. }
    ));
    assert_eq!(p.memory_status().slots_remaining, 0);
    assert_eq!(p.start_recording(), Err(Error::NoRecordingSlots));
}

/// The recording in progress has the number count + 1; after a discarded (empty) stop the
/// number is free again.
#[test]
fn recording_in_progress_number() {
    let mut p = small("default_agc", 48_000);
    assert_eq!(record_one(&mut p, &tone(48_000, 0.1)), 1);
    p.start_recording().unwrap();
    assert_eq!(p.state().current, Some(2));
    assert_eq!(
        p.stop_recording(&mut []).unwrap().result,
        StopResult::DiscardedEmpty
    );
    assert_eq!(p.state().current, None);
    assert_eq!(p.memory_status().count, 1);
    assert_eq!(record_one(&mut p, &tone(48_000, 0.1)), 2);
}

/// Device rate in the stored form: lengths and figures are exact device samples; seconds are
/// the sample count over 6000, and the recordings' sum never exceeds the capacity.
#[test]
fn device_rate_figures() {
    let mut p = small("bypass_all", 6000);
    let x = vec![0.1f32; 6000];
    let n = record_one(&mut p, &x);
    let length = p.recording_length(n).unwrap();
    assert_eq!(length, 6000 + u64::from(p.record_latency_samples()));
    let st = p.memory_status();
    assert_eq!(st.used_samples, length);
    assert_eq!(st.used_seconds(), length as f64 / 6000.0);
    assert_eq!(st.capacity_samples, 60_000);
    assert_eq!(st.used_samples + st.remaining_samples, st.capacity_samples);
    let _ = Settings::new(6000); // the device rate is not a setting (A-026)
}
