//! Spec 009: the seven-rate report of every output-chain check (tasks.md T011; SC-001 – SC-004
//! at all seven host rates, the clip rows at the four rates ≥ 44.1 kHz).
//! Report: `cargo test -p rr_dr60_harness --test output_matrix -- --nocapture`.

use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::{configs, output_checks, report};

/// Every US1 row at every supported host rate.
#[test]
fn output_chain_matrix() {
    let results: Vec<_> = configs::all_rates()
        .iter()
        .flat_map(|&r| output_checks::all_us1(r, &standard))
        .collect();
    report::print(&results);
    let (passed, total) = (results.iter().filter(|r| r.passed).count(), results.len());
    println!("{total} checks, {passed} passed, {} failed", total - passed);
    assert_all(&results);
}
