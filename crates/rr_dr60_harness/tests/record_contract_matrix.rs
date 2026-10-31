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
            println!("{rate:6}   {:28} {:016x} {}", cell.name(), run.digest, if got == run.digest { "equal" } else { "DIFFERS" });
            assert_eq!(got, run.digest, "{rate} Hz {}", cell.name());
        }
    }
}

#[test]
fn deltas_at_every_rate() {
    for rate in rates() {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell { codec: true, changes: true, mute: false, self_stop: false };
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        for d in Delta::effective(cell.codec) {
            assert_ne!(rc::re_render(&run.record, &raw, 4096, d), run.digest, "{rate} Hz {d:?}");
        }
        for d in Delta::TWINS {
            assert_eq!(rc::re_render(&run.record, &raw, 4096, d), run.digest, "{rate} Hz {d:?}");
        }
    }
}
