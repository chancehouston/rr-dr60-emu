//! Spec 009 User Story 1: hear the speaker and the earphone differ (tasks.md T010; spec › US1
//! AS1–AS6). One test per acceptance scenario, each running the output-chain checks at the
//! measurement rates on the output-chain-only configurations and asserting every row passes.
//! Report: `cargo test -p rr_dr60_harness --test us1_output -- --nocapture`.
//!
//! User Story 2's chain cases (`chain_*`, tasks.md T016) live here too, after US1's.

use rr_dr60::Route;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
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

/// US1 AS1 (FR-006, FR-007): the earphone composite within ±0.5 dB of the design from 20 Hz to
/// 2.5 kHz, and the amplifier path's two corners and flat passband on the speaker route with
/// the speaker bypassed.
#[test]
fn as1_earphone_and_amplifier_responses() {
    finish(at(|r| {
        let mut rows = output_checks::earphone_response(r, &standard);
        rows.extend(output_checks::amplifier_response(r, &standard));
        rows.extend(output_checks::amplifier_gain(r, &standard));
        rows
    }));
}

/// US1 AS2 (FR-004, FR-006): a full-scale sine at the reference position reads within
/// −1.0 … 0 dB with THD ≤ 1.5 %; at the 6 dB-over position the peak never exceeds full scale,
/// reads ≥ −0.1 dB, and THD is 15 – 30 % (44.1 kHz and above; the peak ceiling alone below).
#[test]
fn as2_reference_position_and_clipping() {
    finish(at(|r| {
        let mut rows = output_checks::default_position_level(r, &standard);
        rows.extend(if r >= 44_100 {
            output_checks::clip(r, &standard)
        } else {
            output_checks::peak_ceiling(r, &standard)
        });
        rows
    }));
}

/// US1 AS3 (FR-005): the speaker template within ±0.5 dB of the design's discrete realization
/// from 100 Hz to 2.5 kHz, a gain never above 1.0, and its 1 kHz reading reported.
#[test]
fn as3_speaker_template() {
    finish(at(|r| output_checks::speaker_response(r, &standard)));
}

/// US1 AS4 (FR-003, FR-009): the loaded law within 0.1 dB at six positions on both routes; at
/// position 0 exact zeros at the after-stage-11 tap and the route output below −120 dBFS
/// within 100 ms.
#[test]
fn as4_volume_law_and_position_zero() {
    finish(at(|r| {
        let mut rows = Vec::new();
        for route in [Route::Speaker, Route::Earphone] {
            rows.extend(output_checks::volume_law(r, route, &standard));
            rows.extend(output_checks::volume_zero(r, route, &standard));
        }
        rows
    }));
}

/// US1 AS5 (FR-007): a full-scale sine at the maximum position on the earphone route peaks
/// within −0.5 … 0 dB, never above full scale, with THD+N below 0.1 % at 44.1 kHz and above.
#[test]
fn as5_earphone_at_maximum_volume() {
    finish(at(|r| output_checks::earphone_max_volume(r, &standard)));
}

/// US1 AS6 (FR-004): the earphone route at the reference position reads the loaded law's
/// figure (about −18.6 dB) within ±1.5 dB; and FR-008: the chain's 1 kHz group delay per
/// route at most 0.5 ms.
#[test]
fn as6_earphone_default_level_and_group_delay() {
    finish(at(|r| {
        let mut rows: Vec<MeasurementResult> = output_checks::default_position_level(r, &standard)
            .into_iter()
            .filter(|row| row.config == "output_only_earphone")
            .collect();
        for route in [Route::Speaker, Route::Earphone] {
            rows.extend(output_checks::group_delay_1k(r, route, &standard));
        }
        rows
    }));
}
