//! Spec 024 US2 (the engine describes the slot): AS1 – AS5 (research R-04, R-05, R-09, R-11).

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60::components::{self, ComponentId, InsertionPointId::BeforeLimiter, Scale};
use rr_dr60::record::{self, RecordReadout, flags};
use rr_dr60::{ControlKind, ControlTiming, ControlValue, Error, Pipeline, Stream, controls};
use rr_dr60_harness::configs;
use rr_dr60_harness::insert_checks as ic;
use rr_dr60_harness::record_contract_checks as rc;

/// AS1: one point and one component; the parameters agree with the engine's validation and
/// defaults and with the control manifest, both ways.
#[test]
fn insert_as1_manifests() {
    let points = components::insertion_points();
    assert_eq!(points.len(), 1);
    assert_eq!(
        (points[0].identifier, points[0].control_index),
        ("before_limiter", 31)
    );
    let comps = components::components();
    assert_eq!(comps.len(), 1);
    let c = &comps[0];
    assert_eq!(
        (c.id, c.identifier, c.latency_samples, c.parameter_offset),
        (ComponentId::BandFilter, "band_filter", 0, 0)
    );
    let p = Pipeline::new(rr_dr60::Settings::new(48_000)).unwrap();
    for (i, prm) in c.parameters.iter().enumerate() {
        let e = &controls()[prm.control_index as usize];
        assert_eq!(
            (e.identifier, e.value, e.unit),
            (prm.identifier, prm.value, prm.unit)
        );
        assert_eq!(e.kind, ControlKind::EmulatorOption);
        assert_eq!(e.timing, ControlTiming::Live);
        let engine = p
            .component_parameter_value(BeforeLimiter, i as u32)
            .unwrap();
        let (lo, hi, def) = match prm.value {
            ControlValue::Continuous { min, max, default } => (min, max, default),
            ControlValue::Choice { options, default } => (
                0.0,
                options.last().unwrap().value as f32,
                default.unwrap() as f32,
            ),
            _ => unreachable!(),
        };
        assert_eq!(engine, def, "{}: the engine's default", prm.identifier);
        let mut q = Pipeline::new(rr_dr60::Settings::new(48_000)).unwrap();
        assert!(
            q.set_component_parameter(BeforeLimiter, i as u32, f64::from(lo))
                .is_ok()
        );
        assert!(
            q.set_component_parameter(BeforeLimiter, i as u32, f64::from(hi))
                .is_ok()
        );
        assert!(
            q.set_component_parameter(BeforeLimiter, i as u32, f64::from(hi) + 1.0)
                .is_err()
        );
    }
    assert_eq!(
        c.parameters.iter().map(|p| p.scale).collect::<Vec<_>>(),
        [Scale::Linear, Scale::Log, Scale::Log, Scale::Decibel]
    );
    let slot = &controls()[31];
    assert_eq!(
        (slot.identifier, slot.timing),
        ("insert.before_limiter", ControlTiming::Idle)
    );
}

/// AS2: the five entries at 31 – 35; 158 by the derivation; a 142-entry buffer is filled (the
/// tight bound in `insert_matrix`); no smoke case exceeds 142 (the promise script's
/// byte-identical `--changes` and `--record-changes` diffs against 0.22, 0.90 and 0.91, whose
/// buffers hold 142, prove it).
#[test]
fn insert_as2_control_manifest_and_158() {
    let ids: Vec<&str> = controls()[31..].iter().map(|e| e.identifier).collect();
    assert_eq!(
        ids,
        [
            "insert.before_limiter",
            "band_filter.type",
            "band_filter.frequency_hz",
            "band_filter.q",
            "band_filter.gain_db"
        ]
    );
    assert_eq!(
        rr_dr60::changes::MAX_CHANGES_PER_CALL,
        2 * 64 + 3 + 8 + 16 + 3
    );
    assert_eq!(components::COMPONENT_PARAMETER_BUDGET, 16);
}

/// AS3: the record form 2 round trip with the filter and a scheduled change; a version-1
/// record reads as an empty slot; the named deltas fail the re-render; an unknown component
/// is flagged.
#[test]
fn insert_as3_record_v2() {
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        let cell = rc::Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let mut p = Pipeline::new(rc::settings(cell, rate)).unwrap();
        p.set_component_parameter(BeforeLimiter, 0, 3.0).unwrap();
        p.set_component_parameter(BeforeLimiter, 1, 1200.0).unwrap();
        p.set_component(BeforeLimiter, ComponentId::BandFilter)
            .unwrap();
        p.schedule_change(Stream::Record, u64::from(rate) / 3, 34, 4.0, 77)
            .unwrap();
        let (_, run) = rc::record_run_in(&mut p, cell, &raw);
        let r = rc::readout(&run.record);
        assert_eq!(
            (r.form_version, r.component, r.store[0], r.store[1]),
            (2, ComponentId::BandFilter, 3.0, 1200.0)
        );
        assert!(
            run.changes.iter().any(|e| e.control == 34 && e.tag == 77),
            "the landing in the run"
        );
        assert_eq!(
            rc::re_render(&run.record, &raw, 1024, rc::Delta::None),
            run.digest,
            "{rate}"
        );
        assert_ne!(
            rc::re_render(&run.record, &raw, 1024, rc::Delta::ComponentNone),
            run.digest
        );
        assert_ne!(
            rc::re_render(&run.record, &raw, 1024, rc::Delta::ParameterMoved),
            run.digest
        );
    }
    // An unknown component is flagged (the re-render refuses a record with the flag).
    let raw = rc::raw_stimulus(6000);
    let cell = rc::Cell {
        codec: false,
        changes: false,
        mute: false,
        self_stop: false,
    };
    let (_, run) = rc::record_run(&rr_dr60_harness::checks::standard, 6000, cell, &raw);
    let text = String::from_utf8(run.record.clone()).unwrap();
    let doctored = text.replacen(
        "insert.before_limiter = none",
        "insert.before_limiter = reverb",
        1,
    );
    let body_end = doctored.trim_end_matches('\n').rfind('\n').unwrap() + 1;
    let body = &doctored[..body_end];
    let signed = format!(
        "{body}record_checksum = fnv1a64:{:016x}\n",
        rr_dr60::digest::fnv1a64(body.as_bytes())
    );
    let mut got = RecordReadout::default();
    record::read(signed.as_bytes(), &mut got).unwrap();
    assert_ne!(got.flags & flags::UNKNOWN_COMPONENT, 0);
}

/// AS4: the C path is `ffi_parity insert_parity` / `insert_record_parity` and the smoke's
/// `--insert`; the promise is `scripts/abi-promise.sh` with 12a3c45, f5a0245 and 97bbdc2.
#[test]
fn insert_as4_c_parity_pointer() {
    assert_eq!(
        rr_dr60_ffi::RR_DR60_COMPONENT_BAND_FILTER,
        ComponentId::BandFilter as u32
    );
    assert_eq!(rr_dr60_ffi::RR_DR60_MAX_CHANGES_PER_CALL, 158);
}

/// AS5: the playback stream refuses the parameters; out of range is refused; above the
/// ceiling but in range is accepted and the ceiling applied.
#[test]
fn insert_as5_refusals_and_ceiling() {
    for &rate in configs::measurement_rates() {
        let mut p = ic::engine(ic::oracle_settings(rate), [0.0, 1000.0, 0.7071, 0.0]);
        assert!(matches!(
            p.schedule_change(Stream::Playback, 0, 33, 2000.0, 1),
            Err(Error::ControlNotOnStream { control: 33, .. })
        ));
        assert!(matches!(
            p.schedule_change(Stream::OnePass, 0, 35, 25.0, 1),
            Err(Error::InvalidControlValue { control: 35, .. })
        ));
        let ceiling = components::band_filter_frequency_ceiling_hz(rate);
        let mut q = ic::engine(ic::oracle_settings(rate), [0.0, 20_000.0, 0.7071, 0.0]);
        let mut at_ceiling = ic::engine(
            ic::oracle_settings(rate),
            [0.0, ceiling.min(20_000.0) as f32, 0.7071, 0.0],
        );
        let x = rr_dr60_harness::stimulus::impulse(2048);
        assert_eq!(
            ic::run(&mut q, &x, 512),
            ic::run(&mut at_ceiling, &x, 512),
            "{rate}: the ceiling applied"
        );
        assert_eq!(
            q.component_parameter_value(BeforeLimiter, 1),
            Ok(20_000.0),
            "the requested value stored"
        );
    }
}
