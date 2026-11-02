//! Spec 024 US1 (a filter in the chain): AS1 – AS5 at the measurement rates (research R-07).

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60::__test_hooks::InsertMutation;
use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{Error, Pipeline, Stream, VasEvent};
use rr_dr60_harness::configs;
use rr_dr60_harness::insert_checks::{self as ic, CONTROLS};
use rr_dr60_harness::record_contract_checks as rc;
use rr_dr60_harness::stimulus;

/// AS1: the response within 0.1 dB and 2° of the design script; the structural zeros at or
/// below −120 dB and the unity points within 1e-6 dB; the placement identity bit for bit.
#[test]
fn insert_as1_response_and_placement() {
    let rates = configs::measurement_rates();
    for row in ic::response_rows(rates) {
        let c = row.case;
        println!(
            "AS1 set {} type {} rate {:5}: rel err {:.2e}, phase {:.4}°, DC {:8.2} dB, Nyquist {:8.2} dB",
            c.set, c.ftype, c.rate, row.rel_err, row.phase_err_deg, row.dc_db, row.nyquist_db
        );
        assert!(
            row.rel_err <= 1.0,
            "0.1 dB with the 1e-6 floor (FR-004; R-07): {row:?}"
        );
        assert!(row.phase_err_deg <= 2.0, "2° (FR-004): {row:?}");
        let zero_dc = c.ftype == 1 || c.ftype == 2;
        let zero_nyq = c.ftype == 0 || c.ftype == 2;
        if zero_dc {
            assert!(row.dc_db <= -120.0, "the DC zero (SC-001): {row:?}");
        } else {
            assert!(
                (row.dc_db - f64::from(c.gain_db)).abs() <= 1e-6,
                "DC unity: {row:?}"
            );
        }
        if zero_nyq {
            assert!(
                row.nyquist_db <= -120.0,
                "the Nyquist zero (SC-001): {row:?}"
            );
        } else {
            assert!(
                (row.nyquist_db - f64::from(c.gain_db)).abs() <= 1e-6,
                "Nyquist unity: {row:?}"
            );
        }
    }
    for &rate in rates {
        for values in [[0.0, 1000.0, 0.7071, 0.0], [3.0, 1500.0, 4.0, 6.0]] {
            let (n, bad) = ic::placement_identity(rate, values);
            println!("AS1 placement {rate}: {n} samples, {bad} differ");
            assert_eq!(bad, 0, "the placement identity (US1 AS1) at {rate}");
        }
    }
}

/// AS2: the trajectory within 1e-9 of the smoothstep in log-frequency; the entry at P with its
/// tag; the click rows at least 30 dB below the step.
#[test]
fn insert_as2_transitions_and_clicks() {
    for &rate in configs::measurement_rates() {
        let traj = ic::trajectory(rate, [0.0, 1000.0, 0.7071, 0.0], (CONTROLS[1], 2000.0));
        let n = (0.02 * f64::from(rate)).round() as usize;
        let (l0, l1) = (rr_dr60_detmath::ln(1000.0), rr_dr60_detmath::ln(2000.0));
        for (i, &(f, _, _)) in traj.iter().enumerate().take(n + 4) {
            let m = (i + 1).min(n);
            let want = if m == n {
                2000.0
            } else {
                rr_dr60_detmath::exp(l0 + ic::smoothstep(m as f64 / n as f64) * (l1 - l0))
            };
            assert!(
                (f - want).abs() / want < 1e-9,
                "trajectory at {rate}, sample {i}: {f} vs {want}"
            );
        }
        let x = stimulus::tone(1000.0, 0.1, f64::from(rate), rate as usize / 2);
        let (_, entries) = ic::change_run(
            rate,
            [0.0, 1000.0, 0.7071, 0.0],
            &x,
            1234,
            (CONTROLS[3], 6.0),
            InsertMutation::default(),
            100,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            (entries[0].position, entries[0].control, entries[0].tag),
            (1234, 35, 9)
        );
    }
    for rate in [6000u32, 48_000] {
        let mut asserted = Vec::new();
        for t in 0..4u8 {
            let v = [f32::from(t), 1000.0, 0.7071, 0.0];
            asserted.push(ic::click_row(rate, v, (CONTROLS[1], 2000.0)));
            asserted.push(ic::click_row(
                rate,
                [f32::from(t), 2000.0, 0.7071, 0.0],
                (CONTROLS[1], 1000.0),
            ));
        }
        asserted.push(ic::click_row(
            rate,
            [0.0, 1000.0, 0.7071, 0.0],
            (CONTROLS[3], 12.0),
        ));
        asserted.push(ic::click_row(
            rate,
            [0.0, 1000.0, 0.7071, 0.0],
            (CONTROLS[3], -12.0),
        ));
        for r in &asserted {
            println!(
                "AS2 click {} {:?} -> {:?}: {:.1} dB below the step (floor {:.1} dB)",
                r.rate, r.values, r.change, r.reduction_db, r.floor_db
            );
            assert!(r.reduction_db >= 30.0, "US1 AS2's 30 dB: {r:?}");
        }
        for (values, change) in [
            ([0.0, 1000.0, 10.0, 0.0], (CONTROLS[1], 2000.0)),
            ([0.0, 20.0, 0.7071, 0.0], (CONTROLS[1], 20_000.0)),
            ([0.0, 1000.0, 0.7071, 0.0], (CONTROLS[0], 1.0)),
        ] {
            let r = ic::click_row(rate, values, change);
            println!(
                "AS2 click {} {:?} -> {:?}: {:.1} dB (reported)",
                r.rate, r.values, r.change, r.reduction_db
            );
        }
    }
    for t in [3u8, 2, 0] {
        let (svf, tdf) = ic::b1_row(t);
        println!(
            "AS2 B1 row type {t}: state-variable {svf:+.1} dB, transposed direct form II {tdf:+.1} dB"
        );
        assert!(svf < tdf, "the state-variable form overshoots less (D4)");
    }
}

/// AS3: the empty slot equals a never-set engine; filter → none → filter equals a run that
/// first set the filter; the re-render identity with the filter, the named deltas fail it.
#[test]
fn insert_as3_empty_slot_and_toggle() {
    for &rate in configs::measurement_rates() {
        let s = ic::placement_settings(rate);
        let x = stimulus::tone(800.0, 0.3, f64::from(rate), 8192);
        let plain = ic::run(&mut Pipeline::new(s).unwrap(), &x, 1024);
        let mut emptied = ic::engine(s, [0.0, 1000.0, 0.7071, 0.0]);
        emptied
            .set_component(BeforeLimiter, ComponentId::None)
            .unwrap();
        assert_eq!(
            ic::run(&mut emptied, &x, 1024),
            plain,
            "the empty slot at {rate}"
        );
        // In the oracle configuration (nothing but the slot holds state), with no reset: the
        // component set clears the state (coach post-implementation 10).
        let o = ic::oracle_settings(rate);
        let mut toggled = ic::engine(o, [1.0, 500.0, 2.0, 3.0]);
        let _ = ic::run(&mut toggled, &x, 1024);
        toggled
            .set_component(BeforeLimiter, ComponentId::None)
            .unwrap();
        toggled
            .set_component(BeforeLimiter, ComponentId::BandFilter)
            .unwrap();
        let mut fresh = ic::engine(o, [1.0, 500.0, 2.0, 3.0]);
        assert_eq!(
            ic::run(&mut toggled, &x, 1024),
            ic::run(&mut fresh, &x, 1024),
            "filter → none → filter at {rate}"
        );
    }
    // The re-render cells with the filter in the slot (SC-004) in all sixteen of spec 020's
    // cells (coach post-implementation 11), the second recording included; the named deltas at
    // the measurement rates' first cell.
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        for (k, cell) in rc::Cell::all().into_iter().enumerate() {
            let mut p = Pipeline::new(rc::settings(cell, rate)).unwrap();
            p.set_component_parameter(BeforeLimiter, 1, 700.0).unwrap();
            p.set_component_parameter(BeforeLimiter, 0, 1.0).unwrap();
            p.set_component(BeforeLimiter, ComponentId::BandFilter)
                .unwrap();
            let (mut p, run1) = rc::record_run_in(&mut p, cell, &raw);
            let runs = if cell.self_stop {
                vec![run1]
            } else {
                let (_, run2) = rc::record_run_in(&mut p, cell, &raw);
                vec![run1, run2]
            };
            for (n, run) in runs.iter().enumerate() {
                let none = rc::re_render(&run.record, &raw, 1024, rc::Delta::None);
                assert_eq!(none, run.digest, "{cell:?} recording {} at {rate}", n + 1);
                if k == 0 {
                    for d in [rc::Delta::ComponentNone, rc::Delta::ParameterMoved] {
                        assert_ne!(
                            rc::re_render(&run.record, &raw, 1024, d),
                            run.digest,
                            "{d:?} at {rate}"
                        );
                    }
                }
            }
        }
    }
}

/// AS4: refused while active, kept by reset and reconfigure, the store in force.
#[test]
fn insert_as4_lifecycle() {
    for &rate in configs::measurement_rates() {
        let mut p = Pipeline::new(ic::oracle_settings(rate)).unwrap();
        p.set_component_parameter(BeforeLimiter, 1, 300.0).unwrap();
        p.start_recording().unwrap();
        assert_eq!(
            p.set_component(BeforeLimiter, ComponentId::BandFilter),
            Err(Error::ComponentWhileActive)
        );
        let _ = p.stop_recording(&mut [VasEvent::default(); 16]).unwrap();
        p.set_component(BeforeLimiter, ComponentId::BandFilter)
            .unwrap();
        p.reset();
        let s = *p.settings();
        p.reconfigure(s).unwrap();
        assert_eq!(p.component(BeforeLimiter), ComponentId::BandFilter);
        let x = stimulus::impulse(4096);
        let y = ic::run(&mut p, &x, 512);
        let want = ic::reference(
            rate,
            [0.0, 300.0, 0.7071, 0.0],
            &x.iter().map(|&v| f64::from(v)).collect::<Vec<_>>(),
        );
        for (a, b) in y.iter().zip(&want) {
            assert_eq!(
                a.to_bits(),
                (*b as f32).to_bits(),
                "the store in force at {rate}"
            );
        }
    }
}

/// AS5: the loud row against the empty twin; the limiter-input peak reads the slot's output.
#[test]
fn insert_as5_loud_and_gate() {
    for &rate in configs::measurement_rates() {
        let ((gr, clamp), (gr0, clamp0), bare, bare0) = ic::loud_row(rate);
        println!(
            "AS5 loud {rate}: reduction {gr:.2} dB, clamps {clamp}; twin {gr0:.2} dB, {clamp0}; limiter bypassed: {bare} clamps"
        );
        assert!(gr > 0.0, "the limiter responds (US1 AS5)");
        assert!(
            bare > 0,
            "the converter clamp responds with the limiter bypassed (US1 AS5)"
        );
        assert!(
            gr0 == 0.0 && clamp0 == 0,
            "the twin at −6 dBFS stays below both"
        );
        assert_eq!(
            bare0, 0,
            "its empty-slot twin with the limiter bypassed does not clamp (B7)"
        );
        let d = ic::limiter_input_peak_row(rate);
        assert_eq!(
            d, 0.0,
            "D6: the limiter-input peak reads the slot's output at {rate}"
        );
    }
}

/// FR-013: the latencies unchanged with the filter in the slot.
#[test]
fn insert_latency_unchanged() {
    for rate in configs::all_rates() {
        let mut s = rr_dr60::Settings::new(rate);
        s.speech_codec_enabled = false;
        let a = Pipeline::new(s).unwrap();
        let b = ic::engine(s, [2.0, 1000.0, 5.0, 0.0]);
        assert_eq!(
            a.latency_samples(),
            b.latency_samples(),
            "the one-pass at {rate}"
        );
        assert_eq!(
            a.record_latency_samples(),
            b.record_latency_samples(),
            "the record path at {rate}"
        );
    }
    let _ = Stream::Record;
}

/// FR-007: with the slot empty the tap after the slot equals `AfterMicAmp`, bit for bit, at
/// the seven rates, a −0.0 input sample included (the empty slot does no arithmetic).
#[test]
fn insert_empty_after_insert_equals_after_mic_amp() {
    for rate in configs::all_rates() {
        let mut x = stimulus::tone(900.0, 0.3, f64::from(rate), 4096);
        x[17] = -0.0;
        let a = ic::run(
            &mut Pipeline::new(ic::placement_settings(rate)).unwrap(),
            &x,
            1000,
        );
        let mut s = ic::placement_settings(rate);
        s.tap = rr_dr60::Tap::AfterMicAmp;
        let b = ic::run(&mut Pipeline::new(s).unwrap(), &x, 1000);
        assert!(
            a.iter().zip(&b).all(|(u, v)| u.to_bits() == v.to_bits()),
            "{rate}"
        );
    }
}
