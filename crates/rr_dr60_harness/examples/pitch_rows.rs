//! Prints the pitch-kept rows of one run, as the C smoke program prints them (`smoke --pitch`;
//! spec 023 FR-008, tasks.md T015): a ±0.125 square (24 up, 24 down; exactly representable,
//! spec 006's lesson for a C-versus-Rust stimulus) for 1.5 s at 48 kHz recorded through the
//! default engine with the codec bypassed, then played at level 1 in pitch-kept mode set by the
//! setter (the entries per 1024-sample call and the output digest), then at level 1 in the
//! default mode with the mode scheduled to pitch kept at output 24000 (tag 5) and back at 48000
//! (tag 6) (the entries and the digest); then the mode-aware length helper at the five levels in
//! both modes; then map (c) in both modes at four outputs on the second run's log.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example pitch_rows`

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::speed::SpeedMode;
use rr_dr60::timeline::{place_at, place_at_in_mode};
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream};
use rr_dr60_harness::changes_checks::digest;

fn main() {
    let rate = 48_000u32;
    let n = rate as usize * 3 / 2;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    println!("pitch");
    let mut p = Pipeline::new(s).expect("settings");
    p.set_route(Route::Speaker);
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    let k = p.recording_length(1).expect("length");
    let mut play = |p: &mut Pipeline, name: &str| -> Vec<ChangeEntry> {
        println!("{name}");
        p.start_playback(1).expect("start_playback");
        let mut out = Vec::new();
        let mut entries = Vec::new();
        let mut y = vec![0.0f32; 1024];
        loop {
            let info = p.play_with_changes(&mut y, &mut log).expect("play");
            for e in &log[..info.changes] {
                println!(
                    "{} {} {:016x} {:016x} {}",
                    e.position,
                    e.control,
                    e.before.to_bits(),
                    e.after.to_bits(),
                    e.tag
                );
                entries.push(*e);
            }
            out.extend_from_slice(&y[..info.produced]);
            if info.finished {
                break;
            }
        }
        println!("digest {:016x}", digest(&out));
        entries
    };
    p.set_playback_speed(1).expect("level");
    p.set_playback_speed_mode(SpeedMode::PitchKept);
    let _ = play(&mut p, "kept");
    p.set_playback_speed_mode(SpeedMode::PitchWithSpeed);
    p.schedule_change(Stream::Playback, 24_000, 30, 1.0, 5)
        .expect("schedule");
    p.schedule_change(Stream::Playback, 48_000, 30, 0.0, 6)
        .expect("schedule");
    let entries = play(&mut p, "scheduled");
    println!("lengths");
    for level in 1..=5u8 {
        for mode in [SpeedMode::PitchWithSpeed, SpeedMode::PitchKept] {
            println!(
                "{level} {} {}",
                mode as u32,
                p.playback_length_samples_in_mode(1, level, mode)
                    .expect("length in mode")
            );
        }
    }
    println!("place");
    for index in [0u64, 3000, 6100, 12_000] {
        for mode in [0u32, 1] {
            let st = place_at_in_mode(index, rate, 1, mode, k, &entries).expect("place");
            println!(
                "{index} {mode} {} {} {} {} {}",
                st.level,
                u32::from(st.fading),
                u32::from(st.latched),
                st.place_fifths,
                st.old_place_fifths
            );
        }
        let st = place_at(index, rate, 1, k, &entries).expect("place");
        println!(
            "{index} default {} {} {} {} {}",
            st.level,
            u32::from(st.fading),
            u32::from(st.latched),
            st.place_fifths,
            st.old_place_fifths
        );
    }
}
