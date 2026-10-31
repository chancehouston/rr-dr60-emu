//! Spec 020 at the seven rates: the re-render identity in every cell, the effective changes
//! and the twins, with a printed table.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

fn rates() -> Vec<u32> {
    if cfg!(coverage) {
        configs::measurement_rates().to_vec()
    } else {
        configs::all_rates().to_vec()
    }
}

#[test]
fn re_render_every_cell_at_every_rate() {
    println!("rate     cell                         digest           re-render");
    for rate in rates() {
        let raw = rc::raw_stimulus(rate);
        for cell in Cell::all() {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let got = rc::re_render(&run.record, &raw, 4096, Delta::None);
            println!(
                "{rate:6}   {:28} {:016x} {}",
                cell.name(),
                run.digest,
                if got == run.digest {
                    "equal"
                } else {
                    "DIFFERS"
                }
            );
            assert_eq!(got, run.digest, "{rate} Hz {}", cell.name());
        }
    }
}

#[test]
fn deltas_at_every_rate() {
    for rate in rates() {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        for d in Delta::effective(cell.codec) {
            assert_ne!(
                rc::re_render(&run.record, &raw, 4096, d),
                run.digest,
                "{rate} Hz {d:?}"
            );
        }
        for d in Delta::TWINS {
            assert_eq!(
                rc::re_render(&run.record, &raw, 4096, d),
                run.digest,
                "{rate} Hz {d:?}"
            );
        }
    }
}

#[test]
fn maps_at_every_rate() {
    println!("rate     map(a)  map(b)  impulse(b) impulse(a) codec  one-pass");
    for rate in rates() {
        let (a, b, _) = rc::markers_check(rate);
        let (ib, ia) = rc::impulse_check(rate);
        let (c, miss) = rc::codec_markers_check(rate);
        let o = rc::onepass_check(rate);
        println!("{rate:6}   {a:5.2}   {b:5.2}   {ib:5.2}      {ia:5.2}      {c:5.2}  {o:5.2}");
        assert!(
            a <= 1.0 && b <= 1.0 && ib <= 1.0 && ia <= 1.0 && c <= 6.0 && o <= 2.0 && miss >= 100.0,
            "{rate} Hz"
        );
        for (name, start, requests) in rc::place_cases() {
            let (m, _) = rc::place_case_mismatches(rate, start, &requests);
            assert_eq!(m, 0, "{rate} Hz: {name}");
        }
    }
}
