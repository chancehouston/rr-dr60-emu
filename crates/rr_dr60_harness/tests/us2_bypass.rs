//! Spec 021 User Story 2: the switch is bounded, described and safe (tasks.md T013; spec › US2
//! AS1 – AS9). Report: `cargo test -p rr_dr60_harness --test us2_bypass -- --nocapture`.

use rr_dr60::__test_hooks::player_bypass_targets;
use rr_dr60::{ChangeEntry, ControlTiming, Error, Pipeline, Route, Settings, Stream, Tap, VasEvent};
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::changes_checks::{self as cc, Partition, Source};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;

fn quiet(rate: u32) -> Settings {
    bp::settings(bp::DEFAULT_CHAIN, rate)
}

fn recorded(s: Settings) -> Pipeline {
    let stored = bp::stored_tone(1000.0, 1.0);
    bp::engine(&standard, s, &stored, Route::Speaker, 0.5)
}

fn one_pass_block(p: &mut Pipeline, n: usize) -> Vec<ChangeEntry> {
    let x = bp::host_tone(1000.0, 0.1, 48_000);
    let x = &x[..n.min(x.len())];
    let mut y = vec![0.0f32; x.len()];
    let mut ev = vec![VasEvent::default(); p.max_events(x.len())];
    let mut ch = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let info = p
        .process_with_events_and_changes(x, &mut y, &mut ev, &mut ch)
        .unwrap();
    ch[..info.changes].to_vec()
}

/// US2 AS1 (FR-001, FR-004): the refusals.
#[test]
fn as1_refusals() {
    for &rate in configs::measurement_rates() {
        // A stage configured off on the player: both values refused, nothing pending, the
        // output the no-schedule run's.
        let mut s = quiet(rate);
        s.output.speaker_enabled = false;
        let mut p = recorded(s);
        for v in [0.0, 1.0] {
            assert!(matches!(
                p.schedule_change(Stream::Playback, 0, bp::CONTROL_STAGE_12, v, 1),
                Err(Error::StageNotConfigured { control: 23 })
            ));
        }
        assert_eq!(p.pending_changes(Stream::Playback).unwrap(), 0);
        let a = cc::run(&mut p, Source::Playback(1), Partition::Fixed(512));
        let mut q = recorded(s);
        let b = cc::run(&mut q, Source::Playback(1), Partition::Fixed(512));
        assert!(cc::same(&a.out, &b.out) && a.log == b.log, "{rate} Hz: bit-identical to the no-schedule run");
        // The one-pass's taps.
        let at = |tap: Tap, control: u32| {
            let mut s = quiet(rate);
            s.tap = tap;
            let mut p = Pipeline::new(s).unwrap();
            p.schedule_change(Stream::OnePass, 0, control, 0.0, 1)
        };
        for (tap, refused, accepted) in [
            (Tap::AfterVolumeAmp, vec![23u32, 24], vec![22u32]),
            (Tap::AfterSpeaker, vec![24], vec![22, 23]),
            (Tap::AfterEarphone, vec![23], vec![22, 24]),
            (Tap::AfterPlayback, vec![22, 23, 24], vec![]),
            (Tap::RouteOutput, vec![], vec![22, 23, 24]),
        ] {
            for c in refused {
                assert!(matches!(at(tap, c), Err(Error::StageNotConfigured { control }) if control == c), "{rate} Hz {tap:?} {c}");
            }
            for c in accepted {
                assert!(at(tap, c).is_ok(), "{rate} Hz {tap:?} {c}");
            }
        }
        // The value: exactly 0 or 1.
        let mut p = Pipeline::new(quiet(rate)).unwrap();
        for v in [0.5, 2.0, -1.0, f64::NAN] {
            assert!(matches!(
                p.schedule_change(Stream::Playback, 0, bp::CONTROL_STAGE_13, v, 1),
                Err(Error::InvalidControlValue { control: 24, .. })
            ));
        }
        // The streams: the record stream refuses all four; the one-pass refuses stage 10.
        for c in [5, 22, 23, 24] {
            assert!(matches!(
                p.schedule_change(Stream::Record, 0, c, 0.0, 1),
                Err(Error::ControlNotOnStream { .. })
            ));
        }
        assert!(matches!(
            p.schedule_change(Stream::OnePass, 0, 5, 0.0, 1),
            Err(Error::ControlNotOnStream { control: 5, .. })
        ));
        assert_eq!(p.pending_changes(Stream::Playback).unwrap(), 0);
        assert_eq!(p.pending_changes(Stream::OnePass).unwrap(), 0);
    }
}

/// US2 AS2 (FR-004): the latency, the drain and the length are unchanged by any schedule.
#[test]
fn as2_latency_drain_length_fixed() {
    for &rate in configs::measurement_rates() {
        let s = quiet(rate);
        let fresh = Pipeline::new(s).unwrap();
        let mut p = recorded(s);
        let n = bp::n_fade(11, rate) as u64;
        let sched = [
            Change::new(Stream::Playback, 100, 5, 0.0, 1),
            Change::new(Stream::Playback, 200, 22, 0.0, 2),
            Change::new(Stream::Playback, 300, 23, 0.0, 3),
            Change::new(Stream::Playback, 300 + n / 3, 24, 0.0, 4),
            Change::new(Stream::Playback, 900, 23, 1.0, 5),
        ];
        let live = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(1024));
        let mut q = recorded(s);
        let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(1024));
        assert_eq!(live.out.len(), plain.out.len(), "{rate} Hz: the length");
        assert_eq!(p.playback_latency_samples(), fresh.playback_latency_samples(), "{rate} Hz: the playback latency");
        assert_eq!(p.latency_samples(), fresh.latency_samples(), "{rate} Hz: the one-pass latency");
        // The one-pass: a schedule changes neither its latency nor the produced count.
        let x = bp::host_tone(1000.0, 0.5, rate);
        let mut o = Pipeline::new(s).unwrap();
        let osched = [Change::new(Stream::OnePass, 50, 23, 0.0, 1)];
        let r = cc::run_scheduled(&mut o, Source::OnePass(&x), &osched, Partition::Fixed(256));
        assert_eq!(r.out.len(), x.len());
        assert_eq!(o.latency_samples(), fresh.latency_samples());
    }
}

use rr_dr60_harness::changes_checks::Change;

trait ChangeExt {
    fn new(stream: Stream, position: u64, control: u32, value: f64, tag: u64) -> Change;
}
impl ChangeExt for Change {
    fn new(stream: Stream, position: u64, control: u32, value: f64, tag: u64) -> Change {
        Change {
            stream,
            position,
            control,
            value,
            tag,
        }
    }
}

/// US2 AS3 (FR-001): the log: chained entries with the stream word, `settings()` reflecting
/// the landing, the no-op entry, the same-position pair.
#[test]
fn as3_log_chained_and_settings_reflect() {
    for &rate in configs::measurement_rates() {
        let s = quiet(rate);
        let mut p = recorded(s);
        let sched = [
            Change::new(Stream::Playback, 100, 23, 0.0, 1),
            Change::new(Stream::Playback, 100, 23, 1.0, 2), // the same-position pair
            Change::new(Stream::Playback, 500, 23, 0.0, 3),
            Change::new(Stream::Playback, 700, 23, 0.0, 4), // the no-op
            Change::new(Stream::Playback, 900, 5, 0.0, 5),
        ];
        let r = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(333));
        let e = cc::entries_of(&r.log, 23);
        assert_eq!(e.len(), 4, "{rate} Hz");
        let shape: Vec<(u64, f64, f64, u64, u32)> =
            e.iter().map(|c| (c.position, c.before, c.after, c.tag, c.stream)).collect();
        assert_eq!(
            shape,
            [(100, 1.0, 0.0, 1, 1), (100, 0.0, 1.0, 2, 1), (500, 1.0, 0.0, 3, 1), (700, 0.0, 0.0, 4, 1)]
        );
        cc::assert_chained(&r.log, 23, 1.0);
        let e10 = cc::entries_of(&r.log, 5);
        assert_eq!((e10.len(), e10[0].position, e10[0].after), (1, 900, 0.0));
        assert!(!p.settings().output.speaker_enabled, "{rate} Hz: settings() reflects the landing");
        assert!(!p.settings().playback_stage_enabled);
        assert_eq!(player_bypass_targets(&p), ([false, true, false], true));
        // The same-position pair produced no blend: the output equals the run without it.
        let mut q = recorded(s);
        let without = [sched[2], sched[3], sched[4]];
        let r2 = cc::run_scheduled(&mut q, Source::Playback(1), &without, Partition::Fixed(333));
        assert!(cc::same(&r.out, &r2.out), "{rate} Hz: off-then-on at one position is no blend");
    }
}

/// US2 AS4 (FR-001): the mirror both ways: a one-pass landing adopted by the player at
/// `start_playback` without an entry; a player landing landed by the one-pass's next call with
/// the same tag; a stage the one-pass's tap does not reach skipped.
#[test]
fn as4_mirror_both_ways() {
    for &rate in configs::measurement_rates() {
        let s = quiet(rate);
        let mut p = recorded(s);
        let x = bp::host_tone(1000.0, 0.2, rate);
        let osched = [Change::new(Stream::OnePass, 10, 23, 0.0, 9)];
        let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &osched, Partition::Fixed(64));
        assert_eq!(cc::entries_of(&r.log, 23).len(), 1);
        assert!(!p.settings().output.speaker_enabled);
        let play = cc::run(&mut p, Source::Playback(1), Partition::Fixed(256));
        assert!(cc::entries_of(&play.log, 23).is_empty(), "{rate} Hz: the adopt logs nothing");
        assert_eq!(player_bypass_targets(&p).0, [false, true, false]);
        // The player → the one-pass: at the first sample of its next call, the same tag.
        let mut p = recorded(s);
        let sched = [Change::new(Stream::Playback, 20, 24, 0.0, 77), Change::new(Stream::Playback, 30, 22, 0.0, 78)];
        cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
        let log = one_pass_block(&mut p, 64);
        let m: Vec<(u64, u32, f64, f64, u64, u32)> = log
            .iter()
            .filter(|e| e.control == 22 || e.control == 24)
            .map(|e| (e.position, e.control, e.before, e.after, e.tag, e.stream))
            .collect();
        assert_eq!(m, [(0, 22, 1.0, 0.0, 78, 2), (0, 24, 1.0, 0.0, 77, 2)], "{rate} Hz: stage order, the tags, the one-pass word");
        // Not reached: a one-pass at AfterVolumeAmp never lands stage 12.
        let mut s2 = quiet(rate);
        s2.tap = Tap::AfterVolumeAmp;
        let mut p = recorded(s2);
        cc::run_scheduled(&mut p, Source::Playback(1), &[Change::new(Stream::Playback, 20, 23, 0.0, 1)], Partition::Fixed(256));
        let log = one_pass_block(&mut p, 64);
        assert!(log.iter().all(|e| e.control != 23), "{rate} Hz: skipped");
        assert!(!p.settings().output.speaker_enabled);
    }
}

/// US2 AS5 (FR-009): the meters: the clip count stops while stage 11 is bypassed and counts
/// again from the first sample at w < 1; the route peak reads the blend.
#[test]
fn as5_meters_gate() {
    for &rate in configs::measurement_rates() {
        let s = quiet(rate);
        let stored = rr_dr60_harness::stimulus::stored_tone(1000.0, 0.9, 6000);
        let mut p = bp::engine(&standard, s, &stored, Route::Speaker, 1.0); // position 1: clipping
        let n = bp::n_fade(11, rate) as usize;
        let block = n; // one block per fade length
        p.schedule_change(Stream::Playback, (3 * block) as u64, 22, 0.0, 1).unwrap();
        p.schedule_change(Stream::Playback, (7 * block) as u64, 22, 1.0, 2).unwrap();
        p.start_playback(1).unwrap();
        let mut counts = Vec::new();
        let mut peaks = Vec::new();
        let mut y = vec![0.0f32; block];
        for _ in 0..10 {
            let info = p.play(&mut y).unwrap();
            counts.push(p.meters().amp_clip_count);
            peaks.push((p.meters().route_peak, y[..info.produced].iter().fold(0.0f32, |m, v| m.max(v.abs()))));
            if info.finished {
                break;
            }
        }
        assert!(counts[0] > 0 && counts[1] > 0, "{rate} Hz: clipping before: {counts:?}");
        // Block 3 holds the blend (counts until w = 1 at its last sample); blocks 4 – 6 are
        // wholly at w = 1: no count; block 7 (the return) counts from its first sample.
        assert_eq!(&counts[4..7], &[0, 0, 0], "{rate} Hz: {counts:?}");
        assert!(counts[7] > 0, "{rate} Hz: the return counts: {counts:?}");
        for (i, (meter, out)) in peaks.iter().enumerate() {
            assert!((meter - out).abs() <= f32::EPSILON * 4.0 * meter.max(1.0), "{rate} Hz block {i}: route peak {meter} vs output {out}");
        }
    }
}

/// US2 AS6 (FR-011): the control manifest's four entries read live, nothing else moved.
#[test]
fn as6_control_manifest_live() {
    for id in [
        "playback_stage_enabled",
        "output.volume_amp_enabled",
        "output.speaker_enabled",
        "output.earphone_enabled",
    ] {
        let e = rr_dr60::control(id).unwrap();
        assert_eq!(e.timing, ControlTiming::Live, "{id}");
        assert_eq!(e.kind, rr_dr60::ControlKind::EmulatorOption, "{id}");
    }
    let live = rr_dr60::controls().iter().filter(|e| e.timing == ControlTiming::Live).count();
    assert_eq!(live, 16);
}

/// US2 AS7 (FR-010): the cost is asserted release-only by `scripts/ab-timing.sh f303c99 bypass`
/// (`bypass_timing`); here the all-on path's byte gate is 0 in every default engine.
#[test]
fn as7_cost_pointer() {
    let p = Pipeline::new(quiet(48_000)).unwrap();
    assert_eq!(player_bypass_targets(&p), ([false; 3], false));
}

/// US2 AS8: the C path is covered by `ffi_parity bypass_parity` and the smoke's `--bypass`.
#[test]
fn as8_c_parity_pointer() {
    assert_eq!(rr_dr60::changes::MAX_CHANGES_PER_CALL, 142);
}

/// US2 AS9 (FR-004): the trap and its recipe.
#[test]
fn as9_trap_and_recipe() {
    for &rate in configs::measurement_rates() {
        let s = quiet(rate);
        let mut p = recorded(s);
        let sched = [Change::new(Stream::Playback, 100, 23, 0.0, 1)];
        let live = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
        let snapshot = *p.settings();
        assert!(!snapshot.output.speaker_enabled);
        p.reconfigure(snapshot).unwrap();
        assert!(matches!(
            p.schedule_change(Stream::Playback, 0, 23, 1.0, 2),
            Err(Error::StageNotConfigured { control: 23 })
        ), "{rate} Hz: the trap");
        // The recipe: the switches back on, reconfigure, the bypass at position 0 (at once).
        let mut on = snapshot;
        on.output.speaker_enabled = true;
        p.reconfigure(on).unwrap();
        let r = cc::run_scheduled(&mut p, Source::Playback(1), &[Change::new(Stream::Playback, 0, 23, 0.0, 3)], Partition::Fixed(256));
        // From the live run's blend end on, the steady bypassed output equals the recipe's.
        let n = bp::n_fade(12, rate) as usize;
        let from = 100 + n - 1;
        let tail_live = &live.out[from..];
        let tail_recipe = &r.out[from..];
        assert!(cc::same(tail_live, tail_recipe), "{rate} Hz: the recipe reproduces the bypass's steady output");
    }
}
