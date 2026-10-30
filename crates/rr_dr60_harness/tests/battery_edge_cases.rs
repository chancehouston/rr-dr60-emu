//! Spec 018 edge cases (tasks.md T012; spec › Edge cases): stage 11 bypassed, the earphone
//! route and the switch back, the taps before stage 11, two changes at one position, a volume
//! and a battery transition at once, a change past a playback's end, the route peak beside the
//! unmoved gain reduction, the lower bound.

use rr_dr60::__test_hooks;
use rr_dr60::battery::ceiling_factor;
use rr_dr60::output::VOLUME_6DB_OVER;
use rr_dr60::{OutputSettings, Pipeline, Route, Settings, Stream, Tap};
use rr_dr60_harness::battery_checks as bat;
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::{configs, stimulus};

const RATE: u32 = 48_000;

fn factor(p: &Pipeline) -> f64 {
    __test_hooks::battery_factor(p, Stream::OnePass).expect("the one-pass")
}

/// A scheduled run on the one-pass at `settings`, `route` and `volume`.
fn scheduled(
    settings: Settings,
    route: Route,
    volume: f32,
    x: &[f32],
    changes: &[Change],
) -> cc::Run {
    let mut p = standard(settings);
    p.set_route(route);
    p.set_volume(volume).unwrap();
    cc::run_scheduled(&mut p, Source::OnePass(x), changes, Partition::Fixed(1024))
}

/// Stage 11 bypassed: the change is accepted, logged, in force at once, and has no effect; the
/// clip meter rests while the factor is reported.
#[test]
fn stage11_bypassed_logs_without_effect() {
    let mut s = configs::settings("default_output", RATE);
    s.output = OutputSettings::BYPASSED;
    let x = bat::clipped_tone(RATE);
    let c = bat::change_at(RATE);
    let r = scheduled(
        s,
        Route::Speaker,
        1.0,
        &x,
        &[Change::battery(Stream::OnePass, c, 2.2, 1)],
    );
    let reference = bat::run_at(&standard, s, Route::Speaker, 1.0, 3.0, &x);
    assert!(
        cc::same(&r.out, &reference),
        "no effect with stage 11 bypassed"
    );
    assert_eq!(bat::battery_entries(&r.log).len(), 1, "logged");
    let mut p = standard(s);
    p.set_battery_volts(2.2).unwrap();
    let mut y = vec![0.0f32; 2048];
    let _ = p.process(&x[..2048], &mut y).unwrap();
    assert_eq!(factor(&p), f64::from(ceiling_factor(2.2)), "at once");
    let t: Vec<f64> = (0..4)
        .map(|i| {
            let mut y = [0.0f32; 1];
            p.set_battery_volts(if i % 2 == 0 { 2.6 } else { 2.2 })
                .unwrap();
            let _ = p.process(&x[..1], &mut y).unwrap();
            factor(&p)
        })
        .collect();
    assert_eq!(
        t,
        [2.6f32, 2.2, 2.6, 2.2].map(|v| f64::from(ceiling_factor(v))),
        "every change at once while stage 11 is bypassed"
    );
    assert_eq!(p.meters().amp_clip_count, 0, "the clip meter rests");
    assert_eq!(
        p.meters().battery_ceiling,
        ceiling_factor(2.2),
        "the factor is reported"
    );
}

/// The earphone route: no effect, logged; a value set on the earphone route is in force at
/// once when the speaker returns (the chain state), with no transition.
#[test]
fn earphone_route_then_speaker_takes_the_value_at_once() {
    let s = bat::stage11_only(RATE);
    let x = bat::clipped_tone(RATE);
    let c = bat::change_at(RATE);
    let r = scheduled(
        s,
        Route::Earphone,
        1.0,
        &x,
        &[Change::battery(Stream::OnePass, c, 2.4, 1)],
    );
    let reference = bat::run_at(&standard, s, Route::Earphone, 1.0, 3.0, &x);
    assert!(
        cc::same(&r.out, &reference),
        "no effect on the earphone route"
    );
    assert_eq!(bat::battery_entries(&r.log).len(), 1, "logged");
    let mut p = standard(s);
    p.set_route(Route::Earphone);
    p.set_volume(VOLUME_6DB_OVER).unwrap();
    let mut y = vec![0.0f32; 2048];
    let _ = p.process(&x[..2048], &mut y).unwrap();
    p.set_battery_volts(2.4).unwrap();
    let _ = p.process(&x[..2048], &mut y).unwrap();
    assert_eq!(
        factor(&p),
        f64::from(ceiling_factor(2.4)),
        "at once on the earphone route"
    );
    p.set_route(Route::Speaker);
    let mut y = [0.0f32; 1];
    let _ = p.process(&x[..1], &mut y).unwrap();
    assert_eq!(
        factor(&p),
        f64::from(ceiling_factor(2.4)),
        "the first speaker sample's ceiling is b(2.4) = 0.5, no transition"
    );
    let t: Vec<f64> = (1..64)
        .map(|i| {
            let mut y = [0.0f32; 1];
            let _ = p.process(&x[i..i + 1], &mut y).unwrap();
            factor(&p)
        })
        .collect();
    assert!(
        t.iter().all(|&f| f == f64::from(ceiling_factor(2.4))),
        "no transition"
    );
    let mut a = vec![0.0f32; x.len() - 64];
    let _ = p.process(&x[64..], &mut a).unwrap();
    assert_eq!(
        (bat::peak_after_settle(&a, RATE) as f32).to_bits(),
        ceiling_factor(2.4).to_bits(),
        "the speaker route plays at the 2.4 V ceiling"
    );
}

/// The taps before stage 11 (the front, after playback): the change is logged and has no
/// effect on the output.
#[test]
fn taps_before_stage_11_log_without_effect() {
    let x = bat::clipped_tone(RATE);
    let c = bat::change_at(RATE);
    for tap in [Tap::AfterPlayback, Tap::AfterVas, Tap::AfterMicAmp] {
        let mut s = configs::settings("default_output", RATE);
        s.tap = tap;
        let r = scheduled(
            s,
            Route::Speaker,
            VOLUME_6DB_OVER,
            &x,
            &[Change::battery(Stream::OnePass, c, 2.2, 1)],
        );
        let reference = bat::run_at(&standard, s, Route::Speaker, VOLUME_6DB_OVER, 3.0, &x);
        assert!(
            cc::same(&r.out, &reference),
            "{tap:?}: no effect before stage 11"
        );
        assert_eq!(bat::battery_entries(&r.log).len(), 1, "{tap:?}: logged");
    }
}

/// Two changes at one position: the later wins and there is one transition; both are logged,
/// chained.
#[test]
fn two_changes_at_one_position_later_wins() {
    let s = bat::stage11_only(RATE);
    let x = bat::clipped_tone(RATE);
    let c = bat::change_at(RATE);
    let n = bat::n_of(RATE);
    let r = scheduled(
        s,
        Route::Speaker,
        VOLUME_6DB_OVER,
        &x,
        &[
            Change::battery(Stream::OnePass, c, 2.6, 1),
            Change::battery(Stream::OnePass, c, 2.2, 2),
        ],
    );
    let entries = bat::battery_entries(&r.log);
    assert_eq!(entries.len(), 2);
    // The entries carry the f32 values widened (spec 015).
    let w = |v: f32| f64::from(v);
    assert_eq!((entries[0].before, entries[0].after), (w(3.0), w(2.6)));
    assert_eq!((entries[1].before, entries[1].after), (w(2.6), w(2.2)));
    let b = bat::run_at(&standard, s, Route::Speaker, VOLUME_6DB_OVER, 2.2, &x);
    let c = c as usize;
    assert!(
        cc::same(&r.out[c + n - 1..], &b[c + n - 1..]),
        "one transition to 2.2 V"
    );
    // The trace through a plain pair: one monotonic movement to b(2.2).
    let mut p = standard(s);
    p.set_route(Route::Speaker);
    p.set_volume(VOLUME_6DB_OVER).unwrap();
    let mut y = vec![0.0f32; 2048];
    let _ = p.process(&x[..2048], &mut y).unwrap();
    p.set_battery_volts(2.6).unwrap();
    p.set_battery_volts(2.2).unwrap();
    let t: Vec<f64> = (0..n)
        .map(|i| {
            let mut y = [0.0f32; 1];
            let _ = p.process(&x[2048 + i..2049 + i], &mut y).unwrap();
            factor(&p)
        })
        .collect();
    assert!(t.windows(2).all(|w| w[1] <= w[0]), "monotonic");
    assert_eq!(t[n - 1], f64::from(ceiling_factor(2.2)));
}

/// A volume transition and a battery transition in flight at once are independent: the output
/// equals the old constant run before P and the new constant run (both values) from P + N − 1,
/// and the factor trace is the battery's own smoothstep.
#[test]
fn volume_and_battery_transitions_at_once() {
    let s = bat::stage11_only(RATE);
    let x = bat::clipped_tone(RATE);
    let c = bat::change_at(RATE);
    let n = bat::n_of(RATE);
    let r = scheduled(
        s,
        Route::Speaker,
        VOLUME_6DB_OVER,
        &x,
        &[
            Change::volume(Stream::OnePass, c, 1.0, 1),
            Change::battery(Stream::OnePass, c, 2.4, 2),
        ],
    );
    let a = bat::run_at(&standard, s, Route::Speaker, VOLUME_6DB_OVER, 3.0, &x);
    let b = bat::run_at(&standard, s, Route::Speaker, 1.0, 2.4, &x);
    let c = c as usize;
    assert!(cc::same(&r.out[..c], &a[..c]));
    assert!(cc::same(&r.out[c + n - 1..], &b[c + n - 1..]));
    let t = bat::factor_trace(&standard, RATE, 3.0, 2.4, n);
    let mut p = standard(s);
    p.set_route(Route::Speaker);
    p.set_volume(VOLUME_6DB_OVER).unwrap();
    let mut y = vec![0.0f32; 2048];
    let _ = p.process(&x[..2048], &mut y).unwrap();
    p.set_volume(1.0).unwrap();
    p.set_battery_volts(2.4).unwrap();
    let both: Vec<f64> = (0..n)
        .map(|i| {
            let mut y = [0.0f32; 1];
            let _ = p.process(&x[2048 + i..2049 + i], &mut y).unwrap();
            factor(&p)
        })
        .collect();
    assert_eq!(
        both, t,
        "the battery's trace does not depend on the volume's transition"
    );
}

/// A change scheduled past a playback's end is dropped with the schedule at the playback's end
/// and never logged.
#[test]
fn change_past_the_playback_end_is_dropped() {
    let mut s = configs::settings("default_output", RATE);
    s.speech_codec_enabled = false;
    let x = stimulus::voiced(f64::from(RATE), 0.5, -20.0, 0x018);
    let mut p = standard(s);
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let len = p.playback_length_samples(1, 3).unwrap();
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::battery(Stream::Playback, len + 100_000, 2.2, 9)],
        Partition::Fixed(1024),
    );
    assert!(bat::battery_entries(&r.log).is_empty(), "never landed");
    assert_eq!(
        p.pending_changes(Stream::Playback).unwrap(),
        0,
        "cleared at the end"
    );
    assert_eq!(p.battery_volts(), 3.0, "the value never changed");
}

/// The meters' route peak reads the lowered ceiling on the default engine while the limiter's
/// gain-reduction figure (the record side) is unmoved.
#[test]
fn route_peak_follows_the_ceiling_gain_reduction_unmoved() {
    let mut s = configs::settings("default_output", RATE);
    s.speech_codec_enabled = false;
    s.mic = rr_dr60::MicSettings::DEVICE;
    let x = bat::clipped_tone(RATE);
    let read = |volts: f32| {
        let mut p = standard(s);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_6DB_OVER).unwrap();
        p.set_battery_volts(volts).unwrap();
        let mut y = vec![0.0f32; x.len()];
        let _ = p.process(&x, &mut y).unwrap();
        p.meters()
    };
    let (full, low) = (read(3.0), read(2.2));
    assert!(
        low.route_peak < 0.5 * full.route_peak,
        "{} vs {}",
        low.route_peak,
        full.route_peak
    );
    assert_eq!(
        low.gain_reduction_db.to_bits(),
        full.gain_reduction_db.to_bits()
    );
    assert_eq!(low.hold_charge.to_bits(), full.hold_charge.to_bits());
    assert_eq!(
        low.reconstruction_peak.to_bits(),
        full.reconstruction_peak.to_bits()
    );
    assert_eq!(low.battery_ceiling, ceiling_factor(2.2));
}

/// The lower bound 2.2 V is accepted and played: the clipped peak equals its factor.
#[test]
fn lower_bound_is_played() {
    let s = bat::stage11_only(RATE);
    let x = bat::clipped_tone(RATE);
    let y = bat::run_at(&standard, s, Route::Speaker, VOLUME_6DB_OVER, 2.2, &x);
    assert_eq!(
        (bat::peak_after_settle(&y, RATE) as f32).to_bits(),
        ceiling_factor(2.2).to_bits()
    );
    assert!((f64::from(ceiling_factor(2.2)) - 1.0 / 3.0).abs() < 1e-6);
}

/// A scheduled change to the current value is a no-op that is still logged (before == after).
#[test]
fn no_op_change_is_logged() {
    let s = bat::stage11_only(RATE);
    let x = bat::clipped_tone(RATE);
    let r = scheduled(
        s,
        Route::Speaker,
        VOLUME_6DB_OVER,
        &x,
        &[Change::battery(Stream::OnePass, 700, 3.0, 5)],
    );
    let reference = bat::run_at(&standard, s, Route::Speaker, VOLUME_6DB_OVER, 3.0, &x);
    assert!(cc::same(&r.out, &reference));
    let e = bat::battery_entries(&r.log);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].before, e[0].after, e[0].tag), (3.0, 3.0, 5));
}
