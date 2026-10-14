//! Spec 008 Edge Cases, one test each (task T029), for what the story tests and the core's
//! unit tests do not already cover at the harness level: plain assertions through the public
//! API. The release-only cases (the 60-minute recording and the 7 × 7 cross-rate grid) live in
//! `codec_matrix` (`--ignored`).

use rr_dr60::{
    __test_hooks, Error, Pipeline, RecordInfo, State, StopOutcome, StopResult, VasEvent, VasMode,
    speech_codec,
};
use rr_dr60_harness::{codec_checks, configs, split_checks, stimulus};

/// A −20 dBFS 1 kHz tone of `secs` seconds at `rate`.
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

/// Records `x` in one block and stops; the block's report and the stop's outcome.
fn record_stop(p: &mut Pipeline, x: &[f32]) -> (RecordInfo, StopOutcome) {
    p.start_recording().unwrap();
    let info = p.record(x, &mut []).unwrap();
    assert!(info.stopped.is_none());
    (info, p.stop_recording(&mut []).unwrap())
}

fn kept(out: &StopOutcome) -> (u32, u64) {
    match out.result {
        StopResult::Kept { number, length } => (number, length),
        other => panic!("{other:?}"),
    }
}

/// The exported bytes of recording `number`, then the bytes with their CRC recomputed after
/// `edit`.
fn edited_export(p: &Pipeline, number: u32, edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut b = split_checks::export_bytes(p, number);
    edit(&mut b);
    let n = b.len();
    let crc = split_checks::crc32(&b[..n - 4]);
    b[n - 4..].copy_from_slice(&crc.to_le_bytes());
    b
}

/// A recording shorter than one frame yields one frame completed on silence; a recording to
/// which no input was fed is still discarded (spec 006 FR-012).
#[test]
fn shorter_than_one_frame() {
    for rate in [6000, 48_000] {
        let mut p = small("codec_only", rate);
        let x = tone(rate, 50.0 / 6000.0);
        let (info, out) = record_stop(&mut p, &x);
        let (number, length) = kept(&out);
        assert_eq!((number, length), (1, 120), "{rate} Hz");
        let represented = info.stored as u64 + out.flush.stored as u64 - out.silence_fed_samples;
        assert!(
            (50..=120).contains(&represented),
            "{rate} Hz: n′ = {represented}"
        );
        assert_eq!(out.silence_fed_samples, 120 - represented + 60, "{rate} Hz");
        assert_eq!(info.frames + out.flush.frames, 1);
        // Nothing fed: discarded without a feed.
        p.start_recording().unwrap();
        let empty = p.stop_recording(&mut []).unwrap();
        assert_eq!(empty.result, StopResult::DiscardedEmpty);
        assert_eq!(empty.silence_fed_samples, 0);
        assert_eq!(empty.flush, RecordInfo::default());
        assert_eq!(p.memory_status().count, 1);
    }
}

/// A stop exactly on a frame boundary adds no padding frame: the look-ahead alone is fed and
/// the frame holding the last samples completes inside the stop.
#[test]
fn stop_exactly_on_a_frame_boundary() {
    let mut p = small("codec_only", 6000);
    let (info, out) = record_stop(&mut p, &tone(6000, 240.0 / 6000.0));
    assert_eq!(kept(&out).1, 240);
    assert_eq!(out.silence_fed_samples, 60, "the look-ahead alone");
    assert_eq!(info.frames, 1, "frame 0 on push 179");
    assert_eq!(
        out.flush.frames, 1,
        "frame 1 on push 299, inside the stop feed"
    );
    assert_eq!(out.flush.stored, 60);
}

/// Digital silence is coded like any other frame (A-041: no silence detection): one second
/// gives 50 frames, and two runs produce identical frames (deterministic).
#[test]
fn silence_codes_every_frame() {
    let frames_of = || {
        let mut p = small("codec_only", 6000);
        let (_, out) = record_stop(&mut p, &vec![0.0f32; 6000]);
        assert_eq!(kept(&out).1, 6000);
        assert_eq!(out.flush.frames_total, 50);
        __test_hooks::stored_frames(&p, 1).to_vec()
    };
    let a = frames_of();
    let b = frames_of();
    assert_eq!(a.len(), 500);
    assert_eq!(a, b, "deterministic on silence");
}

/// Non-speech input (a tone, noise) goes through the same model: the bit rate, whole frames,
/// determinism and the latency hold; no transparency claim is made.
#[test]
fn non_speech_inputs_keep_the_frame_rules() {
    let rate = 48_000;
    let fs = f64::from(rate);
    let noise = stimulus::bandlimited_noise(0x0D65, 2 * rate as usize, fs, -20.0);
    for (name, x) in [("tone", tone(rate, 2.0)), ("noise", noise)] {
        let run = || {
            let mut p = small("default_codec", rate);
            let (info, out) = record_stop(&mut p, &x);
            let (_, length) = kept(&out);
            let frames = (info.frames + out.flush.frames) as u64;
            assert_eq!(length % 120, 0, "{name}");
            assert_eq!(frames * 120, length, "{name}");
            assert_eq!(
                (frames * 80) as f64 / (length as f64 / 6000.0),
                4000.0,
                "{name}: bit rate"
            );
            assert_eq!(
                p.latency_samples(),
                1923,
                "{name}: the one-pass latency at 48 kHz"
            );
            __test_hooks::stored_frames(&p, 1).to_vec()
        };
        assert_eq!(run(), run(), "{name}: deterministic");
    }
}

/// Clipped input (a full-scale square wave, and a tone above full scale) is coded like any
/// other: the clamp sits before the encoder, the output is finite, nothing panics.
#[test]
fn clipped_input_is_coded() {
    let rate = 6000;
    let square: Vec<f32> = (0..6000)
        .map(|n| if (n / 3) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let loud = stimulus::tone(1000.0, stimulus::amplitude(6.0), 6000.0, 6000);
    for (name, x) in [("square", square), ("+6 dBFS", loud)] {
        let mut p = small("default_codec", rate);
        let (_, out) = record_stop(&mut p, &x);
        let (number, length) = kept(&out);
        assert!(length >= 6000 && length % 120 == 0, "{name}");
        let y = split_checks::play_to_end(&mut p, number, 997);
        assert!(y.iter().all(|v| v.is_finite()), "{name}");
        let mut one = Pipeline::new(configs::settings("default_codec", rate)).unwrap();
        let mut z = x.clone();
        let _ = one.process_in_place(&mut z);
        assert!(z.iter().all(|v| v.is_finite()), "{name}: one-pass");
    }
}

/// A VAS pause inside a frame: in drop mode the splice falls where it falls and its position
/// equals the codec-off run's; in mute mode the muted samples are zeros the encoder codes, so
/// the length is the input's device length padded to a frame.
#[test]
fn vas_pause_inside_a_frame() {
    let rate = 6000;
    let mut x = tone(rate, 3018.0 / 6000.0);
    x.resize(3018 + 2 * 6000, 0.0); // 2 s of silence outlasts the 1 s hang (A-023)
    x.extend(tone(rate, 0.5));
    let drop = |codec: bool| {
        let mut s = configs::memory_capacity(configs::settings("codec_only_vas", rate), 10, 3);
        s.speech_codec_enabled = codec;
        let mut p = Pipeline::new(s).unwrap();
        p.start_recording().unwrap();
        let mut ev = [VasEvent::default(); 8];
        let info = p.record(&x, &mut ev).unwrap();
        let out = p.stop_recording(&mut []).unwrap();
        (ev[..info.events].to_vec(), kept(&out).1)
    };
    let (on, length_on) = drop(true);
    let (off, length_off) = drop(false);
    assert_eq!(on, off, "the splice is where the gate put it");
    assert_eq!(on.len(), 1);
    assert_ne!(
        on[0].output_position % 120,
        0,
        "the splice falls inside a frame"
    );
    assert!(length_on >= length_off && length_on - length_off < 120);
    assert_eq!(length_on % 120, 0);
    // Mute mode: nothing is dropped.
    let mut s = configs::memory_capacity(configs::settings("codec_only_vas", rate), 10, 3);
    s.vas.mode = VasMode::Mute;
    let mut p = Pipeline::new(s).unwrap();
    let (_, out) = record_stop(&mut p, &x);
    let length = kept(&out).1;
    assert_eq!(length, (x.len() as u64).div_ceil(120) * 120);
}

/// A configured capacity is whole seconds, and every whole second is 50 frames, so the public
/// settings never round; the default is a whole number of frames. (The rounding rule itself is
/// the core's unit test with a hypothetical 7-sample unit.)
#[test]
fn capacity_is_whole_frames_for_every_setting() {
    assert_eq!(21_600_000 % speech_codec::FRAME_SAMPLES, 0);
    for secs in [1u32, 7, 59, 3600, 7200] {
        let p = Pipeline::new(configs::memory_capacity(
            configs::settings("default_codec", 6000),
            secs,
            1,
        ))
        .unwrap();
        let st = p.memory_status();
        assert_eq!(st.capacity_samples, u64::from(secs) * 6000);
        assert_eq!(st.capacity_samples % 120, 0);
        assert_eq!(p.footprint_bytes(), secs as usize * 50 * 10 + 2 * 16);
    }
}

/// The codec bypassed then enabled again (or the reverse) with recordings present is refused;
/// the host erases first.
#[test]
fn codec_setting_change_with_recordings_refused() {
    let on = configs::memory_capacity(configs::settings("default_codec", 48_000), 10, 3);
    let mut off = on;
    off.speech_codec_enabled = false;
    let mut p = Pipeline::new(on).unwrap();
    let _ = record_stop(&mut p, &tone(48_000, 0.5));
    let before = p.memory_status();
    let refused = p.reconfigure(off);
    assert_eq!(refused, Err(Error::StoredExceedsCapacity));
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("do not fit the new configuration"),
        "the broadened reason"
    );
    assert_eq!(p.memory_status(), before);
    assert!(p.settings().speech_codec_enabled);
    p.erase_all().unwrap();
    p.reconfigure(off).unwrap();
    let _ = record_stop(&mut p, &tone(48_000, 0.5));
    assert_eq!(p.reconfigure(on), Err(Error::StoredExceedsCapacity));
    p.erase(1).unwrap();
    p.reconfigure(on).unwrap();
    assert_eq!(p.state().state, State::Idle);
}

/// Frames into a bypassed engine, samples into a codec engine: "encoding differs", distinct
/// from damaged and does not fit.
#[test]
fn cross_encoding_import_refused() {
    let mut codec = small("default_codec", 44_100);
    let mut bypassed = small("default_vas", 44_100);
    let (_, out) = record_stop(&mut codec, &tone(44_100, 1.0));
    let frames = split_checks::export_bytes(&codec, kept(&out).0);
    let (_, out) = record_stop(&mut bypassed, &tone(44_100, 1.0));
    let samples = split_checks::export_bytes(&bypassed, kept(&out).0);
    let e = bypassed.import(&frames).unwrap_err();
    assert_eq!(e, Error::EncodingDiffers);
    assert!(e.to_string().contains("encoding differs"));
    assert_eq!(codec.import(&samples), Err(Error::EncodingDiffers));
    assert_eq!(codec.memory_status().count, 1);
    assert_eq!(bypassed.memory_status().count, 1);
}

/// A version 2 sequence from a later release with a higher form version is refused as
/// unsupported, whatever follows the header; this release's form imports unchanged.
#[test]
fn later_form_version_refused() {
    let mut p = small("default_codec", 6000);
    let (_, out) = record_stop(&mut p, &tone(6000, 0.5));
    let n = kept(&out).0;
    let later = edited_export(&p, n, |b| b[8..10].copy_from_slice(&3u16.to_le_bytes()));
    assert_eq!(
        p.import(&later),
        Err(Error::UnsupportedFormVersion { version: 3 })
    );
    let far = edited_export(&p, n, |b| {
        b[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes())
    });
    assert_eq!(
        p.import(&far),
        Err(Error::UnsupportedFormVersion { version: 0xFFFF })
    );
    assert_eq!(p.import(&split_checks::export_bytes(&p, n)), Ok(2));
}

/// Record at 48 kHz, play at 44.1 kHz through frames: the frames depend on the record rate only,
/// the import at 44.1 kHz holds the same frames, and the playback obeys the length rule.
#[test]
fn record_at_48k_play_at_44k_through_frames() {
    let mut src = small("default_codec", 48_000);
    let (_, out) = record_stop(&mut src, &tone(48_000, 1.0));
    let (n, length) = kept(&out);
    let bytes = split_checks::export_bytes(&src, n);
    let mut dst = small("default_codec", 44_100);
    assert_eq!(dst.import(&bytes), Ok(1));
    assert_eq!(
        __test_hooks::stored_frames(&dst, 1),
        __test_hooks::stored_frames(&src, n)
    );
    assert_eq!(dst.recording_length(1), Ok(length));
    let y = split_checks::play_to_end(&mut dst, 1, 997);
    assert_eq!(
        y.len() as u64,
        split_checks::playback_len(length, 44_100) + u64::from(dst.drain_samples())
    );
    assert!(y.iter().all(|v| v.is_finite()));
}

/// The one-pass processing's trailing partial frame: 1234 samples at 6 kHz yield 9 frames
/// (frame k on push 120(k+1) + 59); feeding the silence the stop would (146 samples) completes
/// the 11 frames that hold every input sample.
#[test]
fn one_pass_trailing_partial_frame() {
    let mut p = Pipeline::new(configs::settings("codec_only", 6000)).unwrap();
    let x = tone(6000, 1234.0 / 6000.0);
    let mut y = x.clone();
    let info = p.process_in_place(&mut y);
    assert_eq!(info.frames, 9);
    let mut zeros = vec![0.0f32; 146]; // 1320 − 1234 + 60 (FR-013)
    let more = p.process_in_place(&mut zeros);
    assert_eq!(info.frames + more.frames, 11);
    assert_eq!(11, 1234u64.div_ceil(120));
    // One zero fewer leaves the last frame unproduced.
    let mut q = Pipeline::new(configs::settings("codec_only", 6000)).unwrap();
    let mut y = x.clone();
    let a = q.process_in_place(&mut y).frames;
    let mut zeros = vec![0.0f32; 145];
    let b = q.process_in_place(&mut zeros).frames;
    assert_eq!(a + b, 10);
}

/// A frame with the one unused code point (lag code 127) is refused on import as damaged; a
/// frame at the legal maximum (lag code 126) imports and decodes to finite output.
#[test]
fn unproducible_bit_pattern_refused() {
    let mut p = small("default_codec", 6000);
    let (_, out) = record_stop(&mut p, &tone(6000, 50.0 / 6000.0));
    let (n, length) = kept(&out);
    assert_eq!(length, 120, "one frame");
    let bad = edited_export(&p, n, |b| {
        b[48..58].copy_from_slice(&codec_checks::LAG_CODE_127_FRAME)
    });
    let before = p.memory_status();
    assert_eq!(p.import(&bad), Err(Error::Damaged));
    assert_eq!(p.memory_status(), before);
    let legal = [0u8, 0, 0x03, 0xF0, 0, 0, 0, 0, 0, 0]; // lag code 126
    let ok = edited_export(&p, n, |b| b[48..58].copy_from_slice(&legal));
    assert_eq!(p.import(&ok), Ok(2));
    let y = split_checks::play_to_end(&mut p, 2, 997);
    assert!(y.iter().all(|v| v.is_finite()));
    assert_eq!(__test_hooks::stored_frames(&p, 2), &legal);
}
