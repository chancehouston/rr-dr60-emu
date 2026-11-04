//! Spec 028 US3 [TR: D2, FR-002, SC-003]: the new map agrees with the existing one wherever
//! nothing is removed, and outside the heard lead where the coder is off, at the measurement
//! rates.

use rr_dr60::{Tap, VasMode};
use rr_dr60_harness::configs;
use rr_dr60_harness::onepass_map_checks::{self as om, Cell, Render};
use rr_dr60_harness::stimulus;

/// The method against the existing map with `latency_samples()` outside `[pos, pos + lead)`.
fn equal_outside(r: &Render, lead: u64) -> u64 {
    let mut differ = 0;
    for o in 0..r.y.len() as u64 {
        let inside = r
            .events
            .iter()
            .any(|e| e.output_position <= o && o < e.output_position + lead);
        let m = r.p.one_pass_input_at_output(o, &r.events).centre;
        if inside {
            differ += u64::from(m != r.existing(o));
        } else {
            assert_eq!(m, r.existing(o), "o {o}");
        }
    }
    differ
}

/// AS1: the coder off in drop mode: equal outside `[pos, pos + L_post)` per pause, bit for bit
/// everywhere at 6 kHz; also at `Tap::AfterVas` with the codec setting on.
#[test]
fn as1_onepass_map_no_codec() {
    for &rate in configs::measurement_rates() {
        let r = om::render(Cell::all()[0], rate);
        let d = equal_outside(&r, r.l_post);
        assert_eq!(d, r.l_post * r.events.len() as u64, "{rate} Hz");
        if rate == 6000 {
            assert_eq!(d, 0);
        }
        let mut s = Cell::all()[1].settings(rate);
        s.tap = Tap::AfterVas;
        let r = om::render_with(s);
        let d = equal_outside(&r, r.l_post);
        assert_eq!(d, r.l_post * r.events.len() as u64, "{rate} Hz AfterVas");
    }
}

/// AS2: mute mode, the coder on or off: nothing is removed and the two maps are equal.
#[test]
fn as2_onepass_map_mute() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all().into_iter().filter(|c| c.mode == VasMode::Mute) {
            let r = om::render(cell, rate);
            assert_eq!(equal_outside(&r, 0), 0, "{rate} Hz {}", cell.name());
        }
    }
}

/// AS3: a render with no pause: equal in every cell. Definitional (with no report both maps are
/// `o − L`; no mutant can fail it): it pins that no pause leaves nothing to correct.
#[test]
fn as3_onepass_map_no_pause() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::tone(om::F_BURST, 0.1, f64::from(rate), rate as usize);
        for cell in Cell::all() {
            let (p, y, events) = om::render_input(cell.settings(rate), &x);
            assert!(events.is_empty(), "{rate} Hz {}: no pause", cell.name());
            let drop = cell.mode == VasMode::Drop;
            for o in 0..y.len() as u64 {
                let want =
                    rr_dr60::timeline::onepass_output_to_input(o, p.latency_samples(), &[], drop);
                assert_eq!(p.one_pass_input_at_output(o, &[]).centre, want.0);
            }
        }
    }
}
