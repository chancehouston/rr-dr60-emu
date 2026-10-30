//! Spec 015: the seven-rate report (tasks.md T007): the partition identity per control and
//! stream, the place pin of d(P), the speed change's audible lag against the playback latency,
//! and the 64 + 3 call. Report:
//! `cargo test -p rr_dr60_harness --test changes_matrix -- --nocapture`.

use rr_dr60::changes::{MAX_CHANGES_PER_CALL, SCHEDULE_CAPACITY, speed_landing};
use rr_dr60::{ChangeEntry, Pipeline, Route, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::{configs, stimulus};

fn recorded(rate: u32, seconds: f64) -> (Pipeline, u32) {
    let s = configs::settings("default_output", rate);
    let x = stimulus::voiced(f64::from(rate), seconds, -20.0, 0x015);
    let mut p = Pipeline::new(s).expect("settings");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    (p, 1)
}

/// SC-001 at the seven rates: each control on its streams, both routes, the four partitions
/// against the setter oracle; 0 mismatches.
#[test]
fn partition_identity_per_control_and_stream() {
    let mut mismatches = 0usize;
    for rate in configs::all_rates() {
        let p0 = u64::from(rate / 4) + 3;
        let x = stimulus::voiced(f64::from(rate), 0.6, -20.0, 0x015);
        for route in [Route::Speaker, Route::Earphone] {
            let other = if route == Route::Speaker {
                Route::Earphone
            } else {
                Route::Speaker
            };
            let cases: Vec<(&str, Stream, &str, Change)> = vec![
                (
                    "volume",
                    Stream::OnePass,
                    "default_output",
                    Change::volume(Stream::OnePass, p0, 0.7, 1),
                ),
                (
                    "route",
                    Stream::OnePass,
                    "default_output",
                    Change::route(Stream::OnePass, p0, other, 1),
                ),
                (
                    "volume",
                    Stream::Playback,
                    "default_output",
                    Change::volume(Stream::Playback, p0, 0.7, 1),
                ),
                (
                    "route",
                    Stream::Playback,
                    "default_output",
                    Change::route(Stream::Playback, p0, other, 1),
                ),
                (
                    "speed",
                    Stream::Playback,
                    "default_output",
                    Change::speed(p0, 4, 1),
                ),
                (
                    "speed codec",
                    Stream::Playback,
                    "default_codec",
                    Change::speed(p0, 4, 1),
                ),
            ];
            for (name, stream, config, change) in cases {
                let recorded_in = |config: &str| {
                    let s = configs::settings(config, rate);
                    let mut p = Pipeline::new(s).expect("settings");
                    p.start_recording().expect("start");
                    let _ = p.record(&x, &mut []).expect("record");
                    let _ = p.stop_recording(&mut []).expect("stop");
                    (p, 1u32)
                };
                let oracle = match stream {
                    Stream::OnePass => {
                        let mut p =
                            Pipeline::new(configs::settings(config, rate)).expect("settings");
                        p.set_route(route);
                        cc::setter_oracle(&mut p, Source::OnePass(&x), &[change])
                    }
                    _ => {
                        let (mut p, n) = recorded_in(config);
                        p.set_route(route);
                        cc::setter_oracle(&mut p, Source::Playback(n), &[change])
                    }
                };
                for part in [
                    Partition::Fixed(1),
                    Partition::Fixed(64),
                    Partition::Fixed(4096),
                    Partition::Irregular,
                ] {
                    let out = match stream {
                        Stream::OnePass => {
                            let mut p =
                                Pipeline::new(configs::settings(config, rate)).expect("settings");
                            p.set_route(route);
                            cc::run_scheduled(&mut p, Source::OnePass(&x), &[change], part).out
                        }
                        _ => {
                            let (mut p, n) = recorded_in(config);
                            p.set_route(route);
                            cc::run_scheduled(&mut p, Source::Playback(n), &[change], part).out
                        }
                    };
                    let ok = same(&out, &oracle);
                    if !ok {
                        mismatches += 1;
                    }
                    println!(
                        "{rate:>5} Hz {route:?} {name:>11} {stream:?} {part:?}: {} ({} samples)",
                        if ok { "identical" } else { "DIFFERS" },
                        out.len()
                    );
                }
            }
        }
    }
    assert_eq!(mismatches, 0);
}

/// d(P) pinned independently of the resampler (research R-08; hardware review P8): at level 3
/// before the latch, `PlayInfo::place` is the index of the last device output computed, so
/// after P one-sample `play` calls `place + 1 == speed_landing(P)`; `place == 0` at P = 0.
#[test]
fn speed_landing_pinned_by_the_place() {
    for rate in configs::all_rates() {
        let s = configs::settings("speed_only", rate);
        let x = stimulus::stored_voiced(0.3);
        let (mut p, n) = configs::stored_recording(&standard, s, &x);
        p.start_playback(n).expect("start");
        let mut buf = [0.0f32; 1];
        let mut last_place = 0u64;
        for pos in 1..=240u64 {
            let info = p.play(&mut buf).expect("play");
            assert_eq!(info.produced, 1);
            last_place = info.place;
            let want = speed_landing(pos, rate).expect("rate");
            assert_eq!(
                last_place + 1,
                want,
                "{rate} Hz P={pos}: place {last_place}"
            );
        }
        assert!(last_place > 0);
        assert_eq!(speed_landing(0, rate).expect("rate"), 0);
    }
}

/// The onset of a speed change (product M1, coach 16; restated at implementation): for a
/// 3 → 5 change at P on a 1 kHz recording, the first host sample differing bit for bit from
/// the constant-level run lies within two device periods after P (the device sample d(P) sits
/// on a whole place, where every level's kernel is the identity, so the first *different*
/// device output is the next one, pulled up to ⌈rate ÷ 6000⌉ host samples later); the
/// perceptual lag is the playback path's latency, `playback_latency_samples()` (the place
/// leads the audible output by it, spec 013 FR-009), printed beside it and documented, not
/// asserted (a bit difference is not an audible one).
#[test]
fn speed_change_onset_and_the_playback_latency() {
    for rate in configs::all_rates() {
        let s = configs::settings("speed_only", rate);
        let x = stimulus::stored_tone(1000.0, 0.5, 6000);
        let (mut p, n) = configs::stored_recording(&standard, s, &x);
        let latency = u64::from(p.playback_latency_samples());
        let p0 = u64::from(rate / 2);
        let steady = cc::run(&mut p, Source::Playback(n), Partition::Fixed(64)).out;
        let changed = cc::run_scheduled(
            &mut p,
            Source::Playback(n),
            &[Change::speed(p0, 5, 1)],
            Partition::Fixed(64),
        )
        .out;
        let first = cc::first_diff(&changed, &steady).expect("the change is audible") as u64;
        let lag = first as i64 - p0 as i64;
        let bound = 2 * i64::from(rate.div_ceil(6000)) + 2;
        println!(
            "{rate:>5} Hz: a 3 -> 5 change at {p0} first differs bit for bit at {first} (+{lag} host samples; bound +{bound}); the playback latency {latency}"
        );
        assert!(
            (0..=bound).contains(&lag),
            "{rate} Hz: onset +{lag} outside 0 ..= {bound}"
        );
    }
}

/// SC-004: 64 scheduled changes inside one 4096-sample block plus a plain route, volume and
/// level set before it: `changes == 67`, `MAX_CHANGES_PER_CALL` holds exactly and the log
/// buffer of that length receives every entry.
#[test]
fn sixty_four_plus_three_in_one_call() {
    for rate in configs::all_rates() {
        let (mut p, n) = recorded(rate, 2.0);
        p.start_playback(n).expect("start");
        let mut buf = vec![0.0f32; 4096];
        let _ = p.play(&mut buf).expect("play");
        let pos = p.stream_position(Stream::Playback).expect("stream");
        for i in 0..SCHEDULE_CAPACITY as u64 {
            p.schedule_change(
                Stream::Playback,
                pos + 10 + i * 50,
                cc::volume_control(),
                0.2 + 0.01 * (i as f64 % 50.0),
                i + 1,
            )
            .expect("schedule");
        }
        assert_eq!(p.pending_changes(Stream::Playback).expect("stream"), 64);
        assert!(
            p.schedule_change(Stream::Playback, pos + 5, cc::volume_control(), 0.5, 99)
                .is_err()
        );
        p.set_route(Route::Earphone);
        p.set_volume(0.9).expect("valid");
        p.set_playback_speed(2).expect("valid");
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let info = p.play_with_changes(&mut buf, &mut log).expect("play");
        // The player's largest call: 64 + 3 = 67, below the bound (139 since spec 017: the
        // one-pass carries two schedules and the eight cross-stream landings).
        assert_eq!(info.changes, SCHEDULE_CAPACITY + 3, "{rate} Hz");
        assert!(info.changes < MAX_CHANGES_PER_CALL);
        assert_eq!(p.pending_changes(Stream::Playback).expect("stream"), 0);
        assert_eq!(log[0].tag, 0);
        assert_eq!(log[1].tag, 0);
        assert_eq!(log[2].tag, 0);
        assert_eq!(log[3].tag, 1);
        assert_eq!(log[66].tag, 64);
        cc::assert_chained(
            &log,
            cc::volume_control(),
            f64::from(rr_dr60::output::VOLUME_REFERENCE),
        );
        println!(
            "{rate:>5} Hz: {} entries in one call (the bound {MAX_CHANGES_PER_CALL})",
            info.changes
        );
    }
}
