//! Spec 005 User Story 2: the record chain still records speech (tasks.md T025; spec › US2
//! AS1–AS3; FR-012 (i)–(iii)).
//!
//! The three chain scenarios through the default pipeline (limiter, record stage, clamp, VAS
//! and playback stage all on) at the 6 kHz identity rate and at 48 kHz, measured by output
//! length and events only (spec 003 research R-11). Report:
//! `cargo test -p rr_dr60_harness --test chain -- --nocapture`.

use rr_dr60_harness::chain_checks;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::report;

const RATES: [u32; 2] = [6000, 48_000];

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// US2 AS1 (FR-012 (i)): the speech-like pattern at −30 dBFS over a −60 dBFS floor at level 3:
/// no splice inside a burst group, each 5 s gap cut to the hang time.
#[test]
fn as1_speech_like_pattern_over_m60_floor_level_3() {
    finish(
        RATES
            .iter()
            .flat_map(|&r| chain_checks::check_fr012_i(r, &standard))
            .collect(),
    );
}

/// US2 AS2 (FR-012 (ii)): the same pattern over a −45 dBFS floor: no pause at level 3; at
/// level 1 every gap is paused.
#[test]
fn as2_m45_floor_no_pause_at_level_3_pauses_at_level_1() {
    finish(
        RATES
            .iter()
            .flat_map(|&r| chain_checks::check_fr012_ii(r, &standard))
            .collect(),
    );
}

/// US2 AS3 (FR-012 (iii)): 30 s of a −60 dBFS floor, then a +6 dBFS burst: exactly one
/// splice, recording resumes after the onset time, the burst is limited and clamped.
#[test]
fn as3_loud_burst_after_30_s_floor_one_splice() {
    finish(
        RATES
            .iter()
            .flat_map(|&r| chain_checks::check_fr012_iii(r, &standard))
            .collect(),
    );
}
