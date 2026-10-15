//! Spec 010 edge cases (tasks.md T010; spec › Edge Cases): samples exactly at the bound pass
//! unchanged; volume 0 is bit-identical with the option on or off; the bypassed chain is
//! bounded at the route output and not at `AfterPlayback`; the per-stage taps are never
//! bounded; a route change mid-playback keeps the output bounded on both sides; a reconfigure
//! from off to on and back applies from the next call; the play calls are bounded with the
//! settings naming a per-stage tap while the one-pass at that tap is not; the three low rates
//! are reported, not predicted.

use rr_dr60::{Pipeline, Route, Tap};
use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::output_clamp_checks::{self as clamp, Expect};
use rr_dr60_harness::{configs, output_checks, split_checks};

fn peak(y: &[f32]) -> f32 {
    y.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

fn bounded(y: &[f32]) -> bool {
    y.iter().all(|v| v.abs() <= 1.0)
}

/// "Samples exactly at the bound": a ±1.0 square with every stage bypassed is exactly at the
/// bound after the resampler only at the identity rate; there the option changes nothing and
/// every sample is still exactly ±1.0 (or 0.0 at the edges).
#[test]
fn samples_exactly_at_the_bound_pass_unchanged() {
    let rate = 6000;
    let x = clamp::stimulus_by_name("square_1k_fs", rate, 1.0);
    let on = output_checks::run(
        &standard,
        clamp::bypassed_with_clamp(rate, true),
        Route::Speaker,
        1.0,
        &x,
    );
    let off = output_checks::run(
        &standard,
        clamp::bypassed_with_clamp(rate, false),
        Route::Speaker,
        1.0,
        &x,
    );
    assert_eq!(
        peak(&off),
        1.0,
        "the identity rate keeps the square exactly at ±1.0"
    );
    assert!(on.iter().zip(&off).all(|(a, b)| a.to_bits() == b.to_bits()));
}

/// "Volume position 0": exact zeros from the next block, bit-identical with the option on or
/// off.
#[test]
fn volume_zero_is_bit_identical() {
    for &rate in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("square_1k_fs", rate, 0.5);
        for route in [Route::Speaker, Route::Earphone] {
            let config = match route {
                Route::Speaker => "default_output_clamp",
                Route::Earphone => "default_earphone_clamp",
                _ => unreachable!("the two routes of spec 006"),
            };
            let on = output_checks::run(&standard, configs::settings(config, rate), route, 0.0, &x);
            let off = output_checks::run(
                &standard,
                configs::settings(configs::output_clamp_twin(config), rate),
                route,
                0.0,
                &x,
            );
            assert!(
                on.iter().zip(&off).all(|(a, b)| a.to_bits() == b.to_bits()),
                "{rate} {route:?}"
            );
        }
    }
}

/// "Output chain bypassed" and "The per-stage taps": the check functions at the measurement
/// rates (the matrix repeats them at the seven).
#[test]
fn bypassed_chain_and_taps() {
    let mut rows = Vec::new();
    for &rate in configs::measurement_rates() {
        rows.extend(clamp::bypassed_chain_bounded(&standard, rate));
        rows.extend(clamp::per_stage_taps_unclamped(
            &standard,
            rate,
            Route::Speaker,
        ));
    }
    assert_all(&rows);
}

/// "Route change mid-playback": the bound applies to whichever route is selected at each
/// block; the output stays bounded on both sides of the change and equals the bounded
/// option-off output with the same schedule.
#[test]
fn route_change_mid_playback_keeps_the_bound() {
    for &rate in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("square_1k_fs", rate, 1.0);
        let block = 1000usize;
        let run = |on: bool| {
            let mut p =
                Pipeline::new(clamp::with_clamp("output_only_speaker_clamp", rate, on)).unwrap();
            // Both branches on, so the earphone route has its stage too.
            let mut s = *p.settings();
            s.output.earphone_enabled = true;
            p.reconfigure(s).unwrap();
            p.set_route(Route::Speaker);
            p.set_volume(1.0).unwrap();
            let mut y = vec![0.0f32; x.len()];
            let mut produced = 0;
            for (i, chunk) in x.chunks(block).enumerate() {
                if i == 10 {
                    p.set_route(Route::Earphone);
                }
                if i == 20 {
                    p.set_route(Route::Speaker);
                }
                let n = p
                    .process(chunk, &mut y[produced..produced + chunk.len()])
                    .unwrap()
                    .produced;
                produced += n;
            }
            y.truncate(produced);
            y
        };
        let on = run(true);
        let off = run(false);
        assert!(bounded(&on), "{rate}: bounded through the route changes");
        assert!(
            peak(&off) > 1.0,
            "{rate}: the option-off run exceeds the bound"
        );
        assert!(
            on.iter()
                .zip(&off)
                .all(|(a, b)| a.to_bits() == b.clamp(-1.0, 1.0).to_bits()),
            "{rate}: the identity through route changes"
        );
    }
}

/// "Reconfigure": the option is read at configuration; a change applies from the next
/// processing call (the paths reset).
#[test]
fn reconfigure_applies_from_the_next_call() {
    for &rate in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("square_1k_fs", rate, 1.0);
        let mut p = Pipeline::new(configs::settings("output_only_speaker", rate)).unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let mut y = vec![0.0f32; x.len()];
        let n = p.process(&x, &mut y).unwrap().produced;
        assert!(peak(&y[..n]) > 1.0, "{rate}: off");
        p.reconfigure(configs::settings("output_only_speaker_clamp", rate))
            .unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let n = p.process(&x, &mut y).unwrap().produced;
        assert!(bounded(&y[..n]), "{rate}: on after the reconfigure");
        assert!(
            y[..n].iter().any(|v| v.abs() == 1.0),
            "{rate}: the bound acts"
        );
        p.reconfigure(configs::settings("output_only_speaker", rate))
            .unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let n = p.process(&x, &mut y).unwrap().produced;
        assert!(peak(&y[..n]) > 1.0, "{rate}: off again");
        assert!(!p.settings().output.clamp_enabled);
    }
}

/// FR-003: the play calls always return the route output, so with the option on they are
/// bounded whatever tap the settings name, while the one-pass at that tap is not.
#[test]
fn play_calls_are_bounded_whatever_the_tap() {
    for &rate in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("square_1k_fs", rate, 1.0);
        let mut s = configs::settings("default_output_clamp", rate);
        s.tap = Tap::AfterSpeaker;
        let mut p = Pipeline::new(s).unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let run = split_checks::record_then_play(
            &mut p,
            &x,
            split_checks::RECORD_BLOCK,
            split_checks::PLAY_BLOCK,
        );
        assert!(
            bounded(&run.pre_drain),
            "{rate}: the play calls are bounded"
        );
        assert!(
            run.pre_drain.iter().any(|v| v.abs() == 1.0),
            "{rate}: the bound acts in play"
        );
        let one_pass = output_checks::run(&standard, s, Route::Speaker, 1.0, &x);
        assert!(
            peak(&one_pass) > 1.0,
            "{rate}: the one-pass at AfterSpeaker is unbounded"
        );
    }
}

/// "Low host rates": the figures at 6, 8 and 16 kHz are reported, not predicted; the identity
/// holds.
#[test]
fn low_host_rates_are_reported() {
    let mut rows = Vec::new();
    for rate in [6000, 8000, 16_000] {
        for (config, stim) in [
            ("output_only_speaker_clamp", "square_1k_fs"),
            ("output_only_earphone_clamp", "square_1k_fs"),
        ] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                config,
                stim,
                1.0,
                Expect::Report,
            ));
        }
    }
    for r in &rows {
        println!("{r}");
    }
    assert_all(&rows);
}
