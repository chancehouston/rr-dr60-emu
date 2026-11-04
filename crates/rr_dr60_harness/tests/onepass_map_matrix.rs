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

/// The seven-rate table: per cell L_post, C, the record-then-play comparison (asserted: no
/// output off by more than ⌈m/l⌉), the spans' f_a − f_b margin (asserted ≥ 12 dB) and the
/// existing map's and the workbench rule's differences (reported).
#[test]
fn onepass_map_seven_rates() {
    println!("rate cell | L_post C | twin outputs off | min f_a − f_b | existing | workbench");
    for rate in configs::all_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let t = om::twin(&r);
            let (n, d, h, e) = om::lead_counts(&r, &t);
            assert_eq!(h, 0, "{rate} Hz {}", cell.name());
            let mut margin = f64::INFINITY;
            for ev in &r.events {
                let (a, s) = (ev.output_position as usize, r.heard(ev) as usize);
                if s - a >= rate as usize / 100 {
                    let m = om::level_db(&r.y, rate, om::F_A, a, s)
                        - om::level_db(&r.y, rate, om::F_B, a, s);
                    margin = margin.min(m);
                }
            }
            if margin.is_finite() {
                assert!(margin >= 12.0, "{rate} Hz {}", cell.name());
            }
            println!(
                "{rate} {} | {} {} | {n} {h} | {margin:.1} | {e} | {d}",
                cell.name(),
                r.l_post,
                r.c
            );
        }
    }
}

/// The read's cost at 48 kHz (release; reported, not asserted): ns per read at 0, 100 and 1000
/// pause reports.
#[test]
#[ignore = "release timing; run with --release -- --ignored"]
fn onepass_map_read_cost() {
    use rr_dr60::{Pipeline, VasEvent};
    let p = Pipeline::new(Cell::all()[1].settings(48_000)).unwrap();
    for n in [0usize, 100, 1000] {
        let ev: Vec<VasEvent> = (0..n as u64)
            .map(|i| VasEvent::new(10_000 + i * 20_000, 5_000))
            .collect();
        let reads = 20_000u64;
        let end = 10_000 + n as u64 * 20_000 + 10_000;
        let t = std::time::Instant::now();
        let mut acc = 0u64;
        for i in 0..reads {
            acc = acc.wrapping_add(p.one_pass_input_at_output(end * i / reads, &ev).centre);
        }
        let ns = t.elapsed().as_nanos() as f64 / reads as f64;
        println!(
            "{n} reports: {ns:.0} ns per read ({:.2} ns per report) [{acc}]",
            ns / n.max(1) as f64
        );
    }
}
