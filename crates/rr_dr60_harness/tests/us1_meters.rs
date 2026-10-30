//! Spec 016 User Story 1: every meter against its outside oracle (tasks.md T007; spec › US1
//! AS1 – AS6) at the measurement rates. Report:
//! `cargo test -p rr_dr60_harness --test us1_meters -- --nocapture`.

use rr_dr60::{__test_hooks, Pipeline, Tap};
use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::{configs, meters_checks as mc, report, stimulus};

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

/// AS1 (FR-005): the host-rate peaks equal the tap outputs; the record-filter peak equals the
/// stage 4 stream (and the `AfterRecord` output at 6 kHz).
#[test]
fn as1_peaks_equal_tap_outputs() {
    finish(at(mc::host_rate_peaks));
}

/// AS2 (FR-006): the gain reduction within 0.01 dB of the bypassed-twin ratio on every block
/// (onset blocks included), the steady figures within the model's print, the charge against
/// the hook. 5 s tones (the coverage lesson of spec 005) in 64-sample blocks.
#[test]
fn as2_gain_reduction_and_charge() {
    finish(at(|r| mc::limiter_figures(r, 5.0, 64)));
}

/// AS2, the flush (SC-002): after a burst and the spec 005 flush length of silence both limiter
/// figures read exactly 0 (release-only; 6 kHz, 200 s).
#[test]
#[ignore = "200 s of audio; run with --ignored in release"]
fn as2_charge_flush() {
    let rate = 6000;
    let f = f64::from(rate);
    let mut x = stimulus::tone_at_phase(
        1000.0,
        9.0,
        stimulus::peak_phase(1000.0, rate),
        f,
        (0.2 * f) as usize,
    );
    x.extend(std::iter::repeat_n(0.0f32, (200.0 * f) as usize));
    let mut s = configs::settings_011(rate);
    s.tap = Tap::AfterAgc;
    let mut p = Pipeline::new(s).unwrap();
    let blocks = mc::one_pass_blocks(&mut p, &x, 4096);
    let last = blocks.last().unwrap().meters;
    assert!(
        blocks[0].meters.gain_reduction_db > 5.0,
        "{}",
        blocks[0].meters.gain_reduction_db
    );
    assert_eq!(last.gain_reduction_db, 0.0);
    assert_eq!(last.hold_charge, 0.0);
    assert_eq!(__test_hooks::limiter_state(&p), 0.0);
}

/// AS3 (FR-007): the converter-clamp count against the twin and the stream; the record-filter
/// peak above 1.0 on every counting block.
#[test]
fn as3_adc_clamp_count() {
    finish(at(mc::adc_clamp_counts));
}

/// AS4 (FR-007): the output-clamp count against the clamp-off twin's playback; the route peak
/// equal between the twins.
#[test]
fn as4_output_clamp_count() {
    finish(at(mc::output_clamp_counts));
}

/// AS5 (FR-008): the state and the hang against the reference gate, the reports and the
/// events, in drop and mute mode, one-pass and recorder.
#[test]
fn as5_vas_state_and_hang() {
    finish(at(|r| {
        let mut rows = mc::vas_figures(r, false);
        rows.extend(mc::vas_figures(r, true));
        rows
    }));
}

/// AS6 (FR-001, FR-004): the groups per call, the flush, the drain.
#[test]
fn as6_transport_groups() {
    finish(at(mc::transport_groups));
}
