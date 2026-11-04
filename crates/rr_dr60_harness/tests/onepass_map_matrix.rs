//! Spec 028 [TR: D2, D7, FR-002, FR-007]: the one-pass output map at seven rates.

use rr_dr60_harness::configs;
use rr_dr60_harness::onepass_map_checks::{self as om, Cell};

/// Research R-00: the signed draft's rule, the heard-point rule and the existing map against
/// the record-then-play twin, around every splice, in every cell (reported; the heard-point
/// rule is asserted in `us1_onepass_map`).
#[test]
fn onepass_map_lead_reproduction() {
    println!("rate cell: L_post C | outputs | draft wrong | heard wrong | existing wrong");
    for rate in configs::all_rates() {
        for cell in Cell::all() {
            if cell.default_engine {
                continue;
            }
            let r = om::render(cell, rate);
            let t = om::twin(&r);
            let (n, d, h, e) = om::lead_counts(&r, &t);
            println!(
                "{rate} {}: {} {} | {n} | {d} | {h} | {e}",
                cell.name(),
                r.l_post,
                r.c
            );
        }
    }
}
