//! Spec 008 User Story 1: record through the speech codec and hear it back (FR-002 – FR-009,
//! FR-014 – FR-016; SC-001 – SC-005; tasks T016, T021). Engines and the codec-only
//! configuration only: no form bytes, no registers, no C (Principle VII).
//!
//! `cargo test -p rr_dr60_harness --test us1_codec -- --nocapture` prints every figure.

use rr_dr60_harness::checks::assert_all;
use rr_dr60_harness::{codec_checks, configs, report};

/// SC-001 (FR-003): exactly 4000 bit/s and whole frames at the seven rates.
#[test]
fn bit_rate_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::bit_rate(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-004 (SC-002): the delay is 180 ± 2 device samples at the seven rates.
#[test]
fn delay_lag_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::delay_lag(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-007 (US1 scenario 5): frame causality and the look-ahead probe at 6 kHz.
#[test]
fn frame_causality_and_lookahead_probe() {
    let results = codec_checks::frame_causality();
    report::print(&results);
    assert_all(&results);
}

/// FR-005 (SC-003): RMS, third-octave levels and the log-spectral distance at −10, −20 and
/// −40 dBFS at the seven rates.
#[test]
fn transparency_at_all_rates_and_levels() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        for level in codec_checks::LEVELS_DBFS {
            results.extend(codec_checks::transparency(rate, level));
        }
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-006 (SC-004): segmental SNR, the silence floor and the level tracking at the seven rates
/// and three levels.
#[test]
fn noise_floor_at_all_rates_and_levels() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        for level in codec_checks::LEVELS_DBFS {
            results.extend(codec_checks::noise_floor(rate, level));
        }
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-009 (SC-002): record-then-play equals the one-pass with the stop's silence appended and
/// the output skipped by the delay, at the seven rates, in two partitions.
#[test]
fn one_pass_equivalence_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::one_pass_equivalence(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-014: the after-VAS tap and the VAS events are unchanged by the codec.
#[test]
fn vas_unchanged_by_the_codec_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::after_vas_tap_unchanged(rate));
        results.extend(codec_checks::vas_events_unchanged(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// SC-002, FR-004: the latency bound and the per-rate figures.
#[test]
fn latency_bound_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::latency_bound(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-008: the frame read-outs of the record and playback calls.
#[test]
fn frame_reports_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::frame_reports(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-021 (US1 scenario 4): the codec bypassed is spec 006, with the read-outs at 0.
#[test]
fn bypassed_equals_006_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(codec_checks::bypassed_equals_006(rate));
    }
    report::print(&results);
    assert_all(&results);
}
