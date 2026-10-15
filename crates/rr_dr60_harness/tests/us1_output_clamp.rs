//! Spec 010 User Story 1: bounded output for integer-buffer hosts (tasks.md T009; spec › US1
//! AS1–AS6). One test per acceptance scenario at the measurement rates, each asserting every
//! row passes. Report: `cargo test -p rr_dr60_harness --test us1_output_clamp -- --nocapture`.
//!
//! User Story 2's whole-engine cases (`equivalence_*`, `change_timing_*`, `stored_form_*`,
//! `latencies_*`; tasks.md T014) live here too, after US1's.

use rr_dr60::{Route, Tap};
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::output_clamp_checks::{self as clamp, Expect};
use rr_dr60_harness::{configs, output_checks, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn at(check: impl Fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    configs::measurement_rates()
        .iter()
        .flat_map(|&r| check(r))
        .collect()
}

/// US1 AS1 (FR-001, FR-003, FR-005): the primary stimulus, a 1 kHz sine of amplitude 1.5 with
/// every stage bypassed, exceeds the bound at every rate and the identity holds on both routes.
#[test]
fn identity_primary_stimulus_every_stage_bypassed() {
    finish(at(|r| {
        let mut rows = Vec::new();
        for route in [Route::Speaker, Route::Earphone] {
            rows.extend(clamp::identity_at(
                &standard,
                r,
                "output_clamp_only",
                route,
                1.0,
                "sine_1k_1p5",
                2.0,
                Expect::Exceeds,
            ));
        }
        rows
    }));
}

/// US1 AS2 – AS3 (FR-005, FR-011): the secondary stimuli in the chain configurations at
/// position 1: the identity holds and the option-off peaks are reported per rate (the 48 kHz
/// speaker square beside spec 009's "about 1.9").
#[test]
fn identity_secondary_stimuli_chain_configurations() {
    finish(at(|r| {
        let mut rows = Vec::new();
        for (config, stim) in [
            ("output_only_speaker_clamp", "square_1k_fs"),
            ("output_only_speaker_clamp", "tone_1k_0"),
            ("output_only_earphone_clamp", "square_1k_fs"),
            ("default_output_clamp", "square_1k_fs"),
            ("default_output_clamp", "tone_1k_0"),
            ("default_output_clamp", "sine_1k_1p5"),
            ("default_earphone_clamp", "square_1k_fs"),
        ] {
            rows.extend(clamp::identity(
                &standard,
                r,
                config,
                stim,
                2.0,
                Expect::Exceeds,
            ));
        }
        // Reported, not expected to exceed: the earphone configurations on the tones.
        for (config, stim) in [
            ("output_only_earphone_clamp", "tone_1k_0"),
            ("output_only_earphone_clamp", "sine_1k_1p5"),
            ("default_earphone_clamp", "tone_1k_0"),
            ("default_earphone_clamp", "sine_1k_1p5"),
        ] {
            rows.extend(clamp::identity(
                &standard,
                r,
                config,
                stim,
                2.0,
                Expect::Report,
            ));
        }
        if r == 48_000 {
            let peak = clamp::off_peak(
                &standard,
                r,
                "output_only_speaker_clamp",
                "square_1k_fs",
                2.0,
            );
            println!(
                "48 kHz, chain-only speaker route, full-scale square at position 1: option-off peak {peak:.3} (spec 009 reported about 1.9)"
            );
        }
        rows
    }));
}

/// US1 AS4 (FR-010, SC-002): the 0.9 default on the speaker route at the reference position
/// does not exceed the bound on a full-scale tone at any rate (the count is zero), and the
/// identity holds (so the output is bit-identical with the option on or off).
#[test]
fn identity_reference_position_does_not_overshoot() {
    finish(at(|r| {
        let mut rows = clamp::identity(
            &standard,
            r,
            "default_output_ref_clamp",
            "tone_1k_0",
            2.0,
            Expect::Within,
        );
        rows.extend(clamp::identity(
            &standard,
            r,
            "default_output_ref_clamp",
            "sine_1k_1p5",
            2.0,
            Expect::Within,
        ));
        // The square at 96 kHz reads 3 samples over (T006); reported.
        rows.extend(clamp::identity(
            &standard,
            r,
            "default_output_ref_clamp",
            "square_1k_fs",
            2.0,
            Expect::Report,
        ));
        rows
    }));
}

/// US1 AS5 (FR-003, SC-002): the per-stage taps and every earlier tap are never bounded.
#[test]
fn taps_unclamped() {
    finish(at(|r| {
        let mut rows = clamp::per_stage_taps_unclamped(&standard, r, Route::Speaker);
        rows.extend(clamp::per_stage_taps_unclamped(
            &standard,
            r,
            Route::Earphone,
        ));
        rows
    }));
}

/// US1 AS5 (spec Edge Cases "Output chain bypassed"): the bypassed chain is still bounded at
/// the route output and not at `AfterPlayback`.
#[test]
fn bypassed_chain_bounded() {
    finish(at(|r| clamp::bypassed_chain_bounded(&standard, r)));
}

/// US1 AS6 (FR-001, FR-005): every bounded sample is exactly ±1.0, no sample inside the bound
/// moves, and the reported latencies are unchanged by the option.
#[test]
fn bounded_samples_are_exactly_the_bound() {
    for &r in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("square_1k_fs", r, 1.0);
        let on = output_checks::run(
            &standard,
            configs::settings("output_only_speaker_clamp", r),
            Route::Speaker,
            1.0,
            &x,
        );
        let off = output_checks::run(
            &standard,
            configs::settings("output_only_speaker", r),
            Route::Speaker,
            1.0,
            &x,
        );
        assert_eq!(on.len(), off.len(), "{r}");
        let mut bounded = 0usize;
        for (a, b) in on.iter().zip(&off) {
            if b.abs() > 1.0 {
                assert_eq!(a.abs(), 1.0, "{r}: a bounded sample is exactly the bound");
                assert_eq!(a.signum(), b.signum(), "{r}");
                bounded += 1;
            } else {
                assert_eq!(
                    a.to_bits(),
                    b.to_bits(),
                    "{r}: a sample inside the bound moved"
                );
            }
        }
        assert!(bounded > 0, "{r}: the stimulus must exceed the bound");
        let p_on = rr_dr60::Pipeline::new(configs::settings("default_output_clamp", r)).unwrap();
        let p_off = rr_dr60::Pipeline::new(configs::settings("default_output", r)).unwrap();
        assert_eq!(p_on.latency_samples(), p_off.latency_samples(), "{r}");
        assert_eq!(
            p_on.record_latency_samples(),
            p_off.record_latency_samples(),
            "{r}"
        );
        assert_eq!(
            p_on.playback_latency_samples(),
            p_off.playback_latency_samples(),
            "{r}"
        );
        assert_eq!(p_on.drain_samples(), p_off.drain_samples(), "{r}");
        println!(
            "{r} Hz: {bounded} of {} samples bounded, each exactly ±1.0; latencies unchanged",
            on.len()
        );
    }
}

/// A sanity check the matrix relies on: the primary stimulus's option-off output at the
/// route output equals `AfterPlayback`'s when every stage is bypassed (the chain is inactive),
/// so the option alone makes them differ.
#[test]
fn bypassed_route_output_equals_after_playback_without_the_option() {
    for &r in configs::measurement_rates() {
        let x = clamp::stimulus_by_name("sine_1k_1p5", r, 1.0);
        let a = output_checks::run(
            &standard,
            configs::settings("bypass_route_output", r),
            Route::Speaker,
            1.0,
            &x,
        );
        let mut s = configs::settings("bypass_route_output", r);
        s.tap = Tap::AfterPlayback;
        let b = output_checks::run(&standard, s, Route::Speaker, 1.0, &x);
        assert!(
            a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits()),
            "{r}"
        );
    }
}
