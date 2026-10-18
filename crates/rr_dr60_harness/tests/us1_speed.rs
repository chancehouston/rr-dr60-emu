//! Spec 013 US1: slow and fast playback at a chosen level (tasks.md T011; research R-08).
//!
//! The speed control's own response at 6 kHz in `speed_only` (FR-004 (a) – (c), FR-008, the
//! tail against the independent reference, level 3's identity), the lengths and the length
//! helper at the measurement rates (FR-005, FR-009), the other-rate level comparison (FR-004)
//! and the default path (US1 AS5).

use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::configs::measurement_rates;
use rr_dr60_harness::report;
use rr_dr60_harness::speed_checks as sc;

/// US1 AS1 (FR-005, FR-009): the lengths, the length helper and one `finished`, in the speed-only
/// configuration and the default engine, at the measurement rates.
#[test]
fn us1_as1_lengths() {
    let mut results = Vec::new();
    for &rate in measurement_rates() {
        results.extend(sc::lengths(&standard, rate, "speed_only"));
        results.extend(sc::lengths(&standard, rate, "speed_default"));
    }
    report::print(&results);
    assert_all(&results);
}

/// US1 AS2, AS3 (FR-004 (a) – (c)) at 6 kHz.
#[test]
fn us1_as2_as3_band_at_6k() {
    let results = sc::band_6k(&standard);
    report::print(&results);
    assert_all(&results);
}

/// FR-004's other-rate clause at the measurement rates.
#[test]
fn us1_as2_band_other_rates() {
    let mut results = Vec::new();
    for &rate in measurement_rates() {
        if rate != 6000 {
            results.extend(sc::band_other_rate(&standard, rate));
        }
    }
    report::print(&results);
    assert_all(&results);
}

/// US1 AS4 (FR-006, FR-009): level 3's identity and reports, codec off and on.
#[test]
fn us1_as4_level3_identity() {
    let mut results = sc::level3_identity(&standard, "speed_only");
    results.extend(sc::level3_identity(&standard, "speed_only_codec"));
    report::print(&results);
    assert_all(&results);
}

/// US1 AS5 (FR-003): the default path at the measurement rates.
#[test]
fn us1_as5_default_path() {
    let mut results = Vec::new();
    for &rate in measurement_rates() {
        results.extend(sc::default_path(&standard, rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// US1 AS6 (FR-008): the impulse at 6 kHz.
#[test]
fn us1_as6_impulse() {
    let results = sc::impulse(&standard);
    report::print(&results);
    assert_all(&results);
}

/// US1 AS6 (FR-005; research R-08 (c)): the tail and the whole output against the independent
/// reference, codec off and on.
#[test]
fn us1_as6_tail_and_reference() {
    let mut results = sc::tail_and_reference(&standard, "speed_only");
    results.extend(sc::tail_and_reference(&standard, "speed_only_codec"));
    report::print(&results);
    assert_all(&results);
}
