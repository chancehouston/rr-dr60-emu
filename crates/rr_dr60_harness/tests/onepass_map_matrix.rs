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
    println!(
        "rate cell | L_post C | twin outputs off excluded | min f_a − f_b | existing | workbench"
    );
    for rate in configs::all_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let t = om::twin(&r);
            let (n, d, h, e, x) = om::lead_counts_excluded(&r, &t);
            assert_eq!(h, 0, "{rate} Hz {}", cell.name());
            // D7 (iii): the outputs the twin clamps or the lead precedes, counted (the start, the
            // stop's flush and padding).
            let bound = r.c + r.l_post + 2 * u64::from(r.p.one_pass_drain_samples());
            assert!(x <= bound, "{rate} Hz {}: {x} excluded", cell.name());
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
                "{rate} {} | {} {} | {n} {h} {x} | {margin:.1} | {e} | {d}",
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
    let mut t0 = 0.0;
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
        if n == 0 {
            t0 = ns;
        }
        let slope = if n == 0 { 0.0 } else { (ns - t0) / n as f64 };
        println!("{n} reports: {ns:.1} ns per read, {slope:.2} ns per added report [{acc}]");
    }
}

/// The taps the cells do not cover (hardware post-implementation Q4): with the coder on in drop
/// mode at `AfterVas`, `AfterDecoder`, `AfterVolumeAmp`, `AfterSpeaker`, `RouteOutput` on the
/// earphone route and `AfterEarphone`, the method is off the record-then-play twin on no output
/// (asserted) and L_post is printed.
#[test]
fn onepass_map_taps() {
    use rr_dr60::{Route, Tap};
    let taps = [
        (Tap::AfterVas, Route::Speaker),
        (Tap::AfterDecoder, Route::Speaker),
        (Tap::AfterVolumeAmp, Route::Speaker),
        (Tap::AfterSpeaker, Route::Speaker),
        (Tap::RouteOutput, Route::Earphone),
        (Tap::AfterEarphone, Route::Earphone),
    ];
    for rate in configs::all_rates() {
        for (tap, route) in taps {
            let mut s = Cell::all()[4].settings(rate);
            s.tap = tap;
            let r = om::render_routed(s, route);
            let t = om::twin(&r);
            let (n, _, h, e) = om::lead_counts(&r, &t);
            println!(
                "{rate} {tap:?} {route:?}: L_post {} C {} | {n} outputs, {h} off | existing {e}",
                r.l_post, r.c
            );
            assert!(n > 1000);
            assert_eq!(h, 0, "{rate} Hz {tap:?} {route:?}");
        }
    }
}
