//! Spec 015 User Story 1: the same session at any buffer size (tasks.md T006; spec › US1
//! AS1 – AS9) at the measurement rates. Report:
//! `cargo test -p rr_dr60_harness --test us1_changes -- --nocapture`.

use rr_dr60::changes::speed_landing;
use rr_dr60::output::volume_transition_samples;
use rr_dr60::{Pipeline, Route, Settings, Stream, Tap};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::speed_checks::{self as sc, stored_stream};
use rr_dr60_harness::{configs, stimulus};

const PARTITIONS: [Partition; 4] = [
    Partition::Fixed(1),
    Partition::Fixed(64),
    Partition::Fixed(4096),
    Partition::Irregular,
];

/// The 0.9 default with the chain on and the codec bypassed (`default_output`), on `route`.
fn chain_settings(rate: u32) -> Settings {
    configs::settings("default_output", rate)
}

/// A 2 s voiced recording in an engine with `settings`.
fn recorded(settings: Settings, seconds: f64) -> (Pipeline, u32) {
    let x = stimulus::voiced(f64::from(settings.host_rate_hz), seconds, -20.0, 0x015);
    let mut p = Pipeline::new(settings).expect("settings");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    (p, 1)
}

fn at(check: impl Fn(u32)) {
    for &r in configs::measurement_rates() {
        check(r);
    }
}

/// AS1 (FR-008, SC-001): a volume, a route and (the player) a speed change at inside-block
/// positions, at the four partitions and on both routes: bit-identical outputs and logs, equal
/// to the block-size-1 plain-setter oracle.
#[test]
fn as1_partition_independence_and_setter_oracle() {
    at(|rate| {
        let p0 = u64::from(rate / 3) + 7; // inside a block at every partition
        for start_route in [Route::Speaker, Route::Earphone] {
            let other = if start_route == Route::Speaker {
                Route::Earphone
            } else {
                Route::Speaker
            };
            // The player.
            let changes = [
                Change::volume(Stream::Playback, p0, 0.9, 1),
                Change::route(Stream::Playback, p0 + 777, other, 2),
                Change::speed(p0 + 1234, 5, 3),
            ];
            let mut engines = Vec::new();
            for part in PARTITIONS {
                let (mut p, n) = recorded(chain_settings(rate), 1.0);
                p.set_route(start_route);
                engines.push(cc::run_scheduled(
                    &mut p,
                    Source::Playback(n),
                    &changes,
                    part,
                ));
            }
            let (mut p, n) = recorded(chain_settings(rate), 1.0);
            p.set_route(start_route);
            let oracle = cc::setter_oracle(&mut p, Source::Playback(n), &changes);
            for (i, r) in engines.iter().enumerate() {
                assert!(
                    same(&r.out, &oracle),
                    "{rate} Hz player {start_route:?} {:?}: differs from the setter oracle at {:?}",
                    PARTITIONS[i],
                    cc::first_diff(&r.out, &oracle)
                );
                assert_eq!(
                    r.log, engines[0].log,
                    "{rate} Hz player {:?}: the log",
                    PARTITIONS[i]
                );
            }
            assert_eq!(engines[0].log.len(), 3, "{rate} Hz player: three entries");
            for (e, c) in engines[0].log.iter().zip(&changes) {
                assert_eq!(
                    (e.position, e.control, e.after, e.tag),
                    (c.position, c.control, c.value, c.tag)
                );
            }
            // The one-pass.
            let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
            let changes = [
                Change::volume(Stream::OnePass, p0, 0.9, 1),
                Change::route(Stream::OnePass, p0 + 777, other, 2),
            ];
            let mut runs = Vec::new();
            for part in PARTITIONS {
                let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
                p.set_route(start_route);
                runs.push(cc::run_scheduled(
                    &mut p,
                    Source::OnePass(&x),
                    &changes,
                    part,
                ));
            }
            let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
            p.set_route(start_route);
            let oracle = cc::setter_oracle(&mut p, Source::OnePass(&x), &changes);
            for (i, r) in runs.iter().enumerate() {
                assert!(
                    same(&r.out, &oracle),
                    "{rate} Hz one-pass {start_route:?} {:?}: differs from the setter oracle at {:?}",
                    PARTITIONS[i],
                    cc::first_diff(&r.out, &oracle)
                );
                assert_eq!(
                    r.log, runs[0].log,
                    "{rate} Hz one-pass {:?}: the log",
                    PARTITIONS[i]
                );
            }
            // A `set_route` before the first call is a plain change the first call logs (tag
            // 0) unless the route was already the default (D3; the player's was consumed by
            // `start_playback`).
            let plain = usize::from(start_route != Route::default());
            assert_eq!(
                runs[0].log.len(),
                2 + plain,
                "{rate} Hz one-pass {start_route:?}: the entries"
            );
            if plain == 1 {
                assert_eq!((runs[0].log[0].tag, runs[0].log[0].position), (0, 0));
            }
        }
    });
}

/// AS2 (FR-003, SC-002): spec 011's splice identity at the tap after stage 11 with P in place
/// of the block boundary: samples before P equal the old position's run, sample P is transition
/// sample 1 and sample P + N − 1 carries the new position's run.
#[test]
fn as2_volume_splice_identity() {
    at(|rate| {
        let n = volume_transition_samples(rate).expect("a supported rate") as usize;
        for config in configs::STAGE11_CONFIGS {
            let s = configs::settings(config, rate);
            let route = configs::stage11_route(config);
            let level = if route == Route::Speaker { -30.0 } else { 0.0 };
            let c = rate as usize * 4 / 5 + 123; // inside a 64-sample block
            let x = stimulus::tone(
                1000.0,
                stimulus::amplitude(level),
                f64::from(rate),
                c + rate as usize,
            );
            let held = |pos: f32| {
                let mut p = Pipeline::new(s).expect("settings");
                p.set_route(route);
                p.set_volume(pos).expect("valid");
                cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(64)).out
            };
            for (from, to) in [(0.1f32, 1.0f32), (1.0, 0.1)] {
                let a = held(from);
                let b = held(to);
                let mut p = Pipeline::new(s).expect("settings");
                p.set_route(route);
                p.set_volume(from).expect("valid");
                let y = cc::run_scheduled(
                    &mut p,
                    Source::OnePass(&x),
                    &[Change::volume(Stream::OnePass, c as u64, to, 1)],
                    Partition::Fixed(64),
                )
                .out;
                assert!(
                    same(&y[..c], &a[..c]),
                    "{rate} {config} {from}->{to}: before P"
                );
                assert!(
                    same(&y[c + n - 1..], &b[c + n - 1..]),
                    "{rate} {config} {from}->{to}: from sample P + N - 1"
                );
                assert_eq!(
                    cc::first_diff(&y, &a),
                    Some(c),
                    "{rate} {config}: sample P is transition sample 1"
                );
                let last = (0..y.len())
                    .rev()
                    .find(|&i| y[i].to_bits() != b[i].to_bits());
                assert_eq!(
                    last,
                    Some(c + n - 2),
                    "{rate} {config}: the last differing sample is P + N - 2"
                );
            }
        }
    });
}

/// AS3 (FR-003, SC-002): a route change at P: the samples before P equal the old route's run,
/// the first samples from P differ from it, and (6 kHz, stage 11 off, as spec 009's check) the
/// samples from P equal a fresh engine's on the new route fed the input from P.
#[test]
fn as3_route_oracle() {
    at(|rate| {
        let mut s = configs::settings("output_only_speaker", rate);
        s.output = rr_dr60::OutputSettings::DEVICE;
        let x = stimulus::tone(1000.0, stimulus::amplitude(-20.0), f64::from(rate), 8192);
        let p0 = 3001u64;
        let old = {
            let mut p = Pipeline::new(s).expect("settings");
            p.set_volume(1.0).expect("valid");
            cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(64)).out
        };
        let mut p = Pipeline::new(s).expect("settings");
        p.set_volume(1.0).expect("valid");
        let y = cc::run_scheduled(
            &mut p,
            Source::OnePass(&x),
            &[Change::route(Stream::OnePass, p0, Route::Earphone, 1)],
            Partition::Fixed(64),
        )
        .out;
        let c = p0 as usize;
        assert!(same(&y[..c], &old[..c]), "{rate}: before P");
        assert!(!same(&y[c..c + 64], &old[c..c + 64]), "{rate}: from P");
        assert_eq!(
            p.route(),
            Route::Earphone,
            "{rate}: the reading moved at landing"
        );
        if rate == rr_dr60::DEVICE_RATE_HZ {
            let mut t = s;
            t.output.volume_amp_enabled = false;
            let mut p = Pipeline::new(t).expect("settings");
            let y = cc::run_scheduled(
                &mut p,
                Source::OnePass(&x),
                &[Change::route(Stream::OnePass, p0, Route::Earphone, 1)],
                Partition::Fixed(64),
            )
            .out;
            let mut fresh = Pipeline::new(t).expect("settings");
            fresh.set_route(Route::Earphone);
            let want = cc::run(&mut fresh, Source::OnePass(&x[c..]), Partition::Fixed(64)).out;
            assert!(same(&y[c..], &want), "6 kHz: the new route from rest at P");
        }
    });
}

/// AS4 (FR-003, SC-002): a speed schedule at 6 kHz equals spec 013's reference with the
/// device-index schedule at d(P) computed by the formula, bit for bit, the codec off and on,
/// including two changes within 4 device samples where a deferred switch into level 3 is
/// overridden and a change past the latch; at the other rates it equals the setter oracle.
#[test]
fn as4_speed_reference_and_oracle() {
    let x = stimulus::stored_voiced(0.7);
    for config in ["speed_only", "speed_only_codec"] {
        let s = configs::settings(config, 6000);
        let (mut p, n) = configs::stored_recording(&standard, s, &x);
        let stream = stored_stream(&mut p, n);
        let k = stream.len() as u64;
        let mut schedule = vec![
            Change::speed(500, 1, 1),
            Change::speed(1500, 4, 2),
            Change::speed(2200, 3, 3), // waits for a whole place …
            Change::speed(2202, 5, 4), // … and is overridden within 4 device samples
        ];
        let dev: Vec<(u64, u8)> = schedule
            .iter()
            .map(|c| {
                (
                    speed_landing(c.position, 6000).expect("rate"),
                    c.value as u8,
                )
            })
            .collect();
        let (want, _) = sc::reference_output_at(&stream, 121, 3, &dev);
        // After the latch (inside the speed control's tail), before the end: no effect, logged.
        let after_latch = want.len() as u64 - 10;
        assert!(
            after_latch > 2202 && after_latch < k,
            "{config}: the tail position {after_latch} of K = {k}"
        );
        schedule.push(Change::speed(after_latch, 2, 5));
        let got = cc::run_scheduled(
            &mut p,
            Source::Playback(n),
            &schedule,
            Partition::Fixed(121),
        );
        assert_eq!(
            sc::mismatches(&got.out[..want.len()], &want),
            0,
            "{config}: the reference"
        );
        // The speed-only drain is 0 at 6 kHz, so the playback is exactly the reference's
        // length; `run` returns at the single `finished`.
        assert_eq!(got.out.len(), want.len(), "{config}: the length");
        assert_eq!(got.log.len(), 5, "{config}: every change logged");
        assert_eq!(got.log[4].position, after_latch);
        assert_eq!((got.log[4].before, got.log[4].after), (5.0, 2.0));
    }
    at(|rate| {
        if rate == 6000 {
            return;
        }
        let s = configs::settings("speed_only", rate);
        let (mut p, n) = configs::stored_recording(&standard, s, &x);
        let schedule = [
            Change::speed(1000, 1, 1),
            Change::speed(4000, 5, 2),
            Change::speed(9000, 3, 3),
        ];
        let got = cc::run_scheduled(&mut p, Source::Playback(n), &schedule, Partition::Fixed(64));
        let (mut q, n) = configs::stored_recording(&standard, s, &x);
        let want = cc::setter_oracle(&mut q, Source::Playback(n), &schedule);
        assert!(
            same(&got.out, &want),
            "{rate}: the setter oracle at {:?}",
            cc::first_diff(&got.out, &want)
        );
    });
}

/// AS5 (FR-003): a change scheduled at a position below the stream's position lands on the
/// next sample produced and is logged at that index; one at exactly the position lands there.
#[test]
fn as5_passed_position() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
        let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
        let head = cc::run(&mut p, Source::OnePass(&x[..3000]), Partition::Fixed(64));
        let pos = p.stream_position(Stream::OnePass).expect("stream");
        assert_eq!(pos as usize, head.out.len());
        let changes = [
            Change::volume(Stream::OnePass, 5, 0.9, 1),
            Change::route(Stream::OnePass, pos, Route::Earphone, 2),
        ];
        let r = cc::run_scheduled(
            &mut p,
            Source::OnePass(&x[3000..]),
            &changes,
            Partition::Fixed(64),
        );
        assert_eq!(r.log.len(), 2);
        assert_eq!(
            r.log[0].position, pos,
            "{rate}: the passed position landed at the stream position"
        );
        assert_ne!(r.log[0].position, 5);
        assert_eq!(
            r.log[1].position, pos,
            "{rate}: a change at the position lands there"
        );
        // The output equals the setter oracle with both changes at `pos`.
        let mut q = Pipeline::new(chain_settings(rate)).expect("settings");
        let _ = cc::run(&mut q, Source::OnePass(&x[..3000]), Partition::Fixed(64));
        let want = cc::setter_oracle(
            &mut q,
            Source::OnePass(&x[3000..]),
            &[
                Change::volume(Stream::OnePass, pos, 0.9, 1),
                Change::route(Stream::OnePass, pos, Route::Earphone, 2),
            ],
        );
        assert!(same(&r.out, &want), "{rate}: the setter oracle");
        // The speed on the player at 48 kHz: a passed position lands at the stream position.
        if rate == 48_000 {
            let (mut p, n) = recorded(chain_settings(rate), 0.5);
            p.start_playback(n).expect("start");
            let mut buf = vec![0.0f32; 3000];
            let _ = p.play(&mut buf).expect("play");
            let pos = p.stream_position(Stream::Playback).expect("stream");
            assert_eq!(pos, 3000);
            p.schedule_change(Stream::Playback, 10, cc::speed_control(), 5.0, 9)
                .expect("schedule");
            let mut log = vec![rr_dr60::ChangeEntry::default(); 4];
            let info = p.play_with_changes(&mut buf, &mut log).expect("play");
            assert_eq!(info.changes, 1);
            assert_eq!(log[0].position, 3000);
            assert_eq!(log[0].after, 5.0);
        }
    });
}

/// AS6 (FR-007): a change at an all-partition boundary of the output clock equals the plain
/// setter called between the calls, bit for bit, at every partition and for every control; the
/// voice activation bypassed (the output clock is the input's) and in mute mode, plus a
/// drop-mode case at a 4096-input boundary.
#[test]
fn as6_boundary_equals_plain_setter() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
        let at = 4096u64;
        let mut variants: Vec<(&str, Settings)> = Vec::new();
        let mut off = chain_settings(rate);
        off.vas.enabled = false;
        variants.push(("vas off", off));
        let mut mute = chain_settings(rate);
        mute.vas.mode = rr_dr60::VasMode::Mute;
        variants.push(("mute", mute));
        for (name, s) in variants {
            for (control, change) in [
                ("volume", Change::volume(Stream::OnePass, at, 0.9, 1)),
                (
                    "route",
                    Change::route(Stream::OnePass, at, Route::Earphone, 2),
                ),
            ] {
                let mut p = Pipeline::new(s).expect("settings");
                let plain = {
                    let a = cc::run(
                        &mut p,
                        Source::OnePass(&x[..at as usize]),
                        Partition::Fixed(4096),
                    )
                    .out;
                    change.apply_plain(&mut p);
                    let b = cc::run(
                        &mut p,
                        Source::OnePass(&x[at as usize..]),
                        Partition::Fixed(4096),
                    )
                    .out;
                    [a, b].concat()
                };
                for part in [
                    Partition::Fixed(1),
                    Partition::Fixed(64),
                    Partition::Fixed(4096),
                    Partition::IrregularWithBoundary(at),
                ] {
                    let mut p = Pipeline::new(s).expect("settings");
                    let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &[change], part);
                    assert!(
                        same(&r.out, &plain),
                        "{rate} {name} {control} {part:?}: {:?}",
                        cc::first_diff(&r.out, &plain)
                    );
                    assert_eq!(r.log.len(), 1);
                    assert_eq!(r.log[0].position, at);
                }
            }
        }
        // Drop mode at a 4096-input boundary: the plain setter between the two input blocks
        // against the schedule at the output position that boundary reaches.
        let s = chain_settings(rate);
        let mut p = Pipeline::new(s).expect("settings");
        let a = cc::run(&mut p, Source::OnePass(&x[..4096]), Partition::Fixed(4096));
        let out_at = p.stream_position(Stream::OnePass).expect("stream");
        p.set_volume(0.9).expect("valid");
        let b = cc::run(&mut p, Source::OnePass(&x[4096..]), Partition::Fixed(4096));
        let plain = [a.out, b.out].concat();
        for part in [
            Partition::Fixed(1),
            Partition::Fixed(64),
            Partition::Fixed(4096),
        ] {
            let mut p = Pipeline::new(s).expect("settings");
            let r = cc::run_scheduled(
                &mut p,
                Source::OnePass(&x),
                &[Change::volume(Stream::OnePass, out_at, 0.9, 1)],
                part,
            );
            assert!(
                same(&r.out, &plain),
                "{rate} drop {part:?}: {:?}",
                cc::first_diff(&r.out, &plain)
            );
        }
    });
}

/// AS7 (FR-003, FR-005): two changes of one control at one position: both applied before
/// sample P, logged in submission order with chained values; the output equals the second
/// change's alone.
#[test]
fn as7_two_at_one_position() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x015);
        let p0 = u64::from(rate / 4) + 1; // 250 ms in: the chain's sections hold signal
        for (pair, alone) in [
            (
                [
                    Change::volume(Stream::OnePass, p0, 0.3, 1),
                    Change::volume(Stream::OnePass, p0, 0.8, 2),
                ],
                Change::volume(Stream::OnePass, p0, 0.8, 2),
            ),
            (
                [
                    Change::route(Stream::OnePass, p0, Route::Earphone, 1),
                    Change::route(Stream::OnePass, p0, Route::Earphone, 2),
                ],
                Change::route(Stream::OnePass, p0, Route::Earphone, 2),
            ),
        ] {
            let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
            let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &pair, Partition::Fixed(64));
            let mut q = Pipeline::new(chain_settings(rate)).expect("settings");
            let w = cc::run_scheduled(&mut q, Source::OnePass(&x), &[alone], Partition::Fixed(64));
            assert!(same(&r.out, &w.out), "{rate}: equals the second alone");
            assert_eq!(r.log.len(), 2);
            assert_eq!((r.log[0].tag, r.log[1].tag), (1, 2));
            assert_eq!(r.log[1].before, r.log[0].after, "chained");
            assert_eq!(r.log[0].position, p0);
            assert_eq!(r.log[1].position, p0);
        }
        // A route pair that returns to the starting route is not a no-op: each switch resets
        // the chain's sections (spec 011 FR-005), so the output from P differs from a run with
        // no change (restated at implementation, post-implementation Clarifications).
        let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
        let r = cc::run_scheduled(
            &mut p,
            Source::OnePass(&x),
            &[
                Change::route(Stream::OnePass, p0, Route::Earphone, 1),
                Change::route(Stream::OnePass, p0, Route::Speaker, 2),
            ],
            Partition::Fixed(64),
        );
        let mut q = Pipeline::new(chain_settings(rate)).expect("settings");
        let w = cc::run(&mut q, Source::OnePass(&x), Partition::Fixed(64));
        assert!(same(&r.out[..p0 as usize], &w.out[..p0 as usize]));
        assert!(
            !same(&r.out, &w.out),
            "{rate}: a round trip resets the sections"
        );
        assert_eq!(r.log.len(), 2);
        // The speed on the player.
        let (mut p, n) = recorded(chain_settings(rate), 0.5);
        let pair = [Change::speed(p0, 1, 1), Change::speed(p0, 5, 2)];
        let r = cc::run_scheduled(&mut p, Source::Playback(n), &pair, Partition::Fixed(64));
        let (mut q, n) = recorded(chain_settings(rate), 0.5);
        let w = cc::run_scheduled(
            &mut q,
            Source::Playback(n),
            &[Change::speed(p0, 5, 2)],
            Partition::Fixed(64),
        );
        assert!(
            same(&r.out, &w.out),
            "{rate}: the speed, equals the second alone"
        );
        assert_eq!(r.log.len(), 2);
        assert_eq!(
            (
                r.log[0].before,
                r.log[0].after,
                r.log[1].before,
                r.log[1].after
            ),
            (3.0, 1.0, 1.0, 5.0)
        );
    });
}

/// AS8 (FR-002, FR-003): changes submitted at 1000 then 500 land at 500 and 1000, against the
/// setter oracle.
#[test]
fn as8_position_order() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x015);
        let changes = [
            Change::volume(Stream::OnePass, 1000, 0.9, 1),
            Change::volume(Stream::OnePass, 500, 0.4, 2),
        ];
        let mut p = Pipeline::new(chain_settings(rate)).expect("settings");
        let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &changes, Partition::Fixed(64));
        assert_eq!(r.log.len(), 2);
        assert_eq!((r.log[0].position, r.log[0].tag), (500, 2));
        assert_eq!((r.log[1].position, r.log[1].tag), (1000, 1));
        assert_eq!(r.log[1].before, r.log[0].after);
        let mut q = Pipeline::new(chain_settings(rate)).expect("settings");
        let want = cc::setter_oracle(&mut q, Source::OnePass(&x), &changes);
        assert!(same(&r.out, &want), "{rate}: the setter oracle");
    });
}

/// AS9 (FR-003): a route switch inside a 1024-sample block leaves spec 016's accumulators
/// intact: the call's route peak equals the largest absolute route-output sample of the whole
/// call (both routes' samples), the clamp count the whole call's.
#[test]
fn as9_meters_across_a_route_switch() {
    at(|rate| {
        let mut s = configs::settings("output_only_speaker", rate);
        s.output = rr_dr60::OutputSettings::DEVICE;
        s.tap = Tap::RouteOutput;
        let x = stimulus::square(1000.0, 1.0, f64::from(rate), 2048);
        let mut p = Pipeline::new(s).expect("settings");
        p.set_volume(1.0).expect("valid");
        // Warm the speaker route for a block, then switch inside the second.
        let _ = cc::run(&mut p, Source::OnePass(&x[..1024]), Partition::Fixed(1024));
        let r = cc::run_scheduled(
            &mut p,
            Source::OnePass(&x[1024..]),
            &[Change::route(
                Stream::OnePass,
                1024 + 300,
                Route::Earphone,
                1,
            )],
            Partition::Fixed(1024),
        );
        let peak = r.out.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        let m = p.meters();
        assert_eq!(
            m.route_peak, peak,
            "{rate}: the route peak covers both routes' samples"
        );
        // The output-clamp count covers the whole call too (the clamp on, a full-scale square
        // at position 1 drives both routes over the bound).
        let mut c = s;
        c.output.clamp_enabled = true;
        let mut q = Pipeline::new(c).expect("settings");
        q.set_volume(1.0).expect("valid");
        let _ = cc::run(&mut q, Source::OnePass(&x[..1024]), Partition::Fixed(1024));
        let rc = cc::run_scheduled(
            &mut q,
            Source::OnePass(&x[1024..]),
            &[Change::route(
                Stream::OnePass,
                1024 + 300,
                Route::Earphone,
                1,
            )],
            Partition::Fixed(1024),
        );
        let bounded = r.out.iter().filter(|v| v.abs() > 1.0).count() as u32;
        assert!(
            bounded > 0,
            "{rate}: the square drives the route output over the bound"
        );
        assert_eq!(
            q.meters().output_clamp_count,
            bounded,
            "{rate}: the clamp count covers both routes' samples"
        );
        assert!(rc.out.iter().all(|v| v.abs() <= 1.0));
        assert!(
            r.out[..300].iter().any(|&v| v.abs() == peak)
                || r.out[300..].iter().any(|&v| v.abs() == peak)
        );
        let before = r.out[..300].iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!(
            before > 0.0,
            "{rate}: the speaker route produced output before the switch"
        );
    });
}
