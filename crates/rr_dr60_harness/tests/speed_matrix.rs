//! Spec 013: the seven-rate report (tasks.md T015; data-model.md › Report rows).
//!
//! Per host rate and level: the playback's length against ⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ + the drain
//! and the length helper (FR-005, FR-009), in the speed-only configuration and the default
//! engine; and FR-004's other-rate level comparison. Asserted where the spec asserts.

use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::configs::all_rates;
use rr_dr60_harness::report;
use rr_dr60_harness::speed_checks as sc;

#[test]
fn speed_matrix_lengths() {
    let mut results = Vec::new();
    for rate in all_rates() {
        results.extend(sc::lengths(&standard, rate, "speed_only"));
        results.extend(sc::lengths(&standard, rate, "speed_default"));
        results.extend(sc::lengths(&standard, rate, "speed_default_no_codec"));
    }
    report::print(&results);
    assert_all(&results);
}

#[test]
fn speed_matrix_other_rate_levels() {
    let mut results = Vec::new();
    for rate in all_rates() {
        if rate != 6000 {
            results.extend(sc::band_other_rate(&standard, rate));
        }
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-007 (reported; post-implementation review B2): the step a level change makes, pinned to
/// the script within 0.5 dB.
#[test]
fn speed_matrix_change_step() {
    let results = sc::change_step();
    report::print(&results);
    assert_all(&results);
}

/// FR-005 (reported): the speed control's tail per level, the worst case over K mod k.
#[test]
fn speed_matrix_tails() {
    for level in sc::LEVELS {
        let k = sc::numerator(level);
        let worst = (1000..1000 + 5 * u64::from(k))
            .map(|kk| rr_dr60::__test_hooks::speed_tail_len(k, kk))
            .max()
            .unwrap();
        println!("speed control, level {level}: tail at most {worst} device samples");
        assert!(worst <= u64::from(rr_dr60::speed::MAX_TAIL_SAMPLES));
    }
}
