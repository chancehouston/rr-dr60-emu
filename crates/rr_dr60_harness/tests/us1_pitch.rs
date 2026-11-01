//! Spec 023 US1: slower, same pitch (AS1 – AS6) at the measurement rates.

use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::configs;
use rr_dr60_harness::pitch_checks as pc;

fn report(rows: &[rr_dr60_harness::checks::MeasurementResult]) {
    for r in rows {
        println!("{r}");
    }
    assert_all(rows);
}

/// AS1, AS2 (FR-002, FR-003): the tone rows in both modes at the four levels.
#[test]
fn as1_as2_tones() {
    for &rate in configs::measurement_rates() {
        report(&pc::tone_rows(&standard, rate));
    }
}

/// AS3 (FR-002): the steady vowel and the glide at levels 1 and 5.
#[test]
fn as3_vowel_and_glide() {
    for &rate in configs::measurement_rates() {
        report(&pc::vowel_and_glide(&standard, rate));
    }
}

/// AS5 (FR-004, FR-005): the lengths, the place and the exact onset.
#[test]
fn as5_place_and_exact_length() {
    for &rate in configs::measurement_rates() {
        report(&pc::lengths(&standard, rate));
    }
}

/// AS6 (reported): the click counts per level.
#[test]
fn as6_click_counts() {
    report(&pc::click_rows(&standard));
}

/// Research R-08: the content offset bound from the shift trace.
#[test]
fn content_offset() {
    for &rate in configs::measurement_rates() {
        report(&pc::offset_rows(&standard, rate));
    }
}
