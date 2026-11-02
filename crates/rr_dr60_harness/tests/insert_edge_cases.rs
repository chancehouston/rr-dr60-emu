//! Spec 024's edge cases, one test each (spec.md › Edge Cases; every name contains "insert").

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60::__test_hooks as hooks;
use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{ChangeEntry, Pipeline, Settings, Stream, VasEvent};
use rr_dr60_harness::insert_checks::{self as ic, CONTROLS};
use rr_dr60_harness::stimulus;

const DEF: [f32; 4] = [0.0, 1000.0, 0.7071, 0.0];

fn calls(p: &mut Pipeline, x: &[f32]) -> (Vec<f32>, Vec<ChangeEntry>) {
    let mut out = vec![0.0f32; x.len()];
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut ev = vec![VasEvent::default(); 64];
    let mut entries = Vec::new();
    for (k, block) in x.chunks(1024).enumerate() {
        let info = p
            .process_with_events_and_changes(
                block,
                &mut out[k * 1024..k * 1024 + block.len()],
                &mut ev,
                &mut log,
            )
            .unwrap();
        entries.extend_from_slice(&log[..info.changes]);
    }
    (out, entries)
}

/// A parameter scheduled while the slot is empty: logged and in the store; a component set
/// later starts from it.
#[test]
fn insert_edge_parameter_while_empty() {
    let mut p = Pipeline::new(ic::oracle_settings(48_000)).unwrap();
    p.schedule_change(Stream::OnePass, 10, CONTROLS[1], 3000.0, 4)
        .unwrap();
    let x = stimulus::impulse(2048);
    let (_, entries) = calls(&mut p, &x);
    assert_eq!(entries.len(), 1);
    assert_eq!(p.component_parameter_value(BeforeLimiter, 1), Ok(3000.0));
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    p.reset();
    let y = ic::run(&mut p, &x, 512);
    let want = ic::reference(
        48_000,
        [0.0, 3000.0, 0.7071, 0.0],
        &x.iter().map(|&v| f64::from(v)).collect::<Vec<_>>(),
    );
    assert!(
        y.iter()
            .zip(&want)
            .all(|(a, b)| a.to_bits() == (*b as f32).to_bits())
    );
}

/// Above the ceiling: accepted, the ceiling applied, the requested value stored; `reconfigure`
/// to another rate re-applies the ceiling.
#[test]
fn insert_edge_frequency_above_the_ceiling() {
    let mut p = ic::engine(ic::oracle_settings(6000), [0.0, 5000.0, 0.7071, 0.0]);
    assert_eq!(p.component_parameter_value(BeforeLimiter, 1), Ok(5000.0));
    let x = stimulus::impulse(2048);
    let y6 = ic::run(&mut p, &x, 512);
    let mut at = ic::engine(ic::oracle_settings(6000), [0.0, 2700.0, 0.7071, 0.0]);
    assert_eq!(y6, ic::run(&mut at, &x, 512));
    p.reconfigure(ic::oracle_settings(48_000)).unwrap();
    let y48 = ic::run(&mut p, &x, 512);
    let mut fresh = ic::engine(ic::oracle_settings(48_000), [0.0, 5000.0, 0.7071, 0.0]);
    assert_eq!(
        y48,
        ic::run(&mut fresh, &x, 512),
        "5000 Hz applied at 48 kHz"
    );
}

/// A second landing within 20 ms restarts from the current value; a type change during a
/// frequency transition blends while the trajectory continues.
#[test]
fn insert_edge_interrupted_transition() {
    let rate = 48_000u32;
    let mut p = ic::engine(ic::oracle_settings(rate), DEF);
    p.schedule_change(Stream::OnePass, 100, CONTROLS[1], 2000.0, 1)
        .unwrap();
    p.schedule_change(Stream::OnePass, 400, CONTROLS[1], 500.0, 2)
        .unwrap();
    p.schedule_change(Stream::OnePass, 500, CONTROLS[0], 1.0, 3)
        .unwrap();
    let x = stimulus::tone(1000.0, 0.1, f64::from(rate), 3000);
    let _ = ic::run(&mut p, &x[..100], 100);
    hooks::insert_tracing(&mut p, Stream::OnePass, true);
    let _ = ic::run(&mut p, &x[100..], 100);
    let t = hooks::insert_trace(&p, Stream::OnePass).to_vec();
    assert!(
        t[299].0 > 1000.0 && t[299].0 < 2000.0,
        "inside the first ramp"
    );
    assert!(
        (t[300].0 - t[299].0).abs() < 30.0,
        "the second ramp starts from the current value"
    );
    assert_eq!(
        t[300 + 959].0.to_bits(),
        500.0f64.to_bits(),
        "the target at P + N − 1"
    );
    assert!(
        t[450].0 < t[300].0,
        "the trajectory continues through the type blend"
    );
}

/// The component already in the slot: no change and no entry.
#[test]
fn insert_edge_same_component() {
    let mut p = ic::engine(ic::oracle_settings(48_000), DEF);
    let x = stimulus::tone(800.0, 0.2, 48_000.0, 4096);
    let a = ic::run(&mut p, &x[..2048], 512);
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    let (_, entries) = calls(&mut p, &x[2048..]);
    assert!(entries.is_empty());
    let mut q = ic::engine(ic::oracle_settings(48_000), DEF);
    let whole = ic::run(&mut q, &x, 512);
    let mut p2 = ic::engine(ic::oracle_settings(48_000), DEF);
    let _ = ic::run(&mut p2, &x[..2048], 512);
    p2.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    let b = ic::run(&mut p2, &x[2048..], 512);
    assert_eq!([a, b].concat(), whole, "the state was not cleared");
}

/// `reconfigure`: the component and the store kept, the section rebuilt, a running transition
/// ended at its target.
#[test]
fn insert_edge_reconfigure_mid_transition() {
    let mut p = ic::engine(ic::oracle_settings(48_000), DEF);
    p.schedule_change(Stream::OnePass, 10, CONTROLS[3], 12.0, 1)
        .unwrap();
    let x = stimulus::tone(800.0, 0.2, 48_000.0, 200);
    let _ = calls(&mut p, &x);
    p.reconfigure(ic::oracle_settings(16_000)).unwrap();
    assert_eq!(p.component(BeforeLimiter), ComponentId::BandFilter);
    assert_eq!(p.component_parameter_value(BeforeLimiter, 3), Ok(12.0));
    let imp = stimulus::impulse(1024);
    let mut want = ic::engine(ic::oracle_settings(16_000), [0.0, 1000.0, 0.7071, 12.0]);
    assert_eq!(ic::run(&mut p, &imp, 256), ic::run(&mut want, &imp, 256));
}

/// The one-pass and the recorder share the store: a one-pass landing reaches the recorder at
/// `start_recording` with no entry.
#[test]
fn insert_edge_shared_store() {
    let mut s = Settings::new(48_000);
    s.speech_codec_enabled = false;
    let mut p = ic::engine(s, DEF);
    p.schedule_change(Stream::OnePass, 0, CONTROLS[2], 4.0, 9)
        .unwrap();
    let _ = calls(&mut p, &stimulus::tone(500.0, 0.1, 48_000.0, 256));
    assert_eq!(p.component_parameter_value(BeforeLimiter, 2), Ok(4.0));
    p.start_recording().unwrap();
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut ev = vec![VasEvent::default(); 1024];
    let info = p
        .record_with_changes(
            &stimulus::tone(500.0, 0.1, 48_000.0, 480),
            &mut ev,
            &mut log,
        )
        .unwrap();
    assert_eq!(info.changes, 0, "adopted at start_recording with no entry");
    let _ = p.stop_recording(&mut ev).unwrap();
}

/// The notch on a tone at its frequency: the tone attenuated by the design's depth (reported).
#[test]
fn insert_edge_notch_on_a_tone() {
    for rate in [6000u32, 48_000] {
        let fs = f64::from(rate);
        let x = stimulus::tone(1000.0, 0.5, fs, rate as usize);
        let mut p = ic::engine(ic::oracle_settings(rate), [3.0, 1000.0, 0.7071, 0.0]);
        let y = ic::run(&mut p, &x, 1024);
        let tail = &y[rate as usize / 2..];
        let rms = (tail
            .iter()
            .map(|v| f64::from(*v) * f64::from(*v))
            .sum::<f64>()
            / tail.len() as f64)
            .max(1e-300);
        let depth = rr_dr60_harness::analysis::db(
            rr_dr60_detmath::sqrt(rms) / (0.5 / rr_dr60_detmath::sqrt(2.0)),
        );
        println!("notch on its tone at {rate}: {depth:.1} dB");
        assert!(depth < -60.0, "the notch attenuates its tone");
    }
}

/// A component in the slot during the stop's flush: the flush runs through it.
#[test]
fn insert_edge_flush_through_the_slot() {
    let mut s = Settings::new(48_000);
    s.speech_codec_enabled = false;
    s.mic.capsule_enabled = false;
    let rec = |values: Option<[f32; 4]>| {
        let mut p = match values {
            Some(v) => ic::engine(s, v),
            None => Pipeline::new(s).unwrap(),
        };
        p.start_recording().unwrap();
        let mut ev = vec![VasEvent::default(); 1024];
        let _ = p
            .record(&stimulus::tone(500.0, 0.5, 48_000.0, 4800), &mut ev)
            .unwrap();
        let out = p.stop_recording(&mut ev).unwrap();
        (out.silence_fed_samples, p.recording_digest(1).unwrap())
    };
    let (_, with) = rec(Some([0.0, 300.0, 4.0, 6.0]));
    let (_, without) = rec(None);
    assert_ne!(
        with, without,
        "the flush and the input both run through the slot"
    );
}

/// The filter's state is cleared at each recording's start (plan R2; restated edge case).
#[test]
fn insert_edge_state_cleared_per_recording() {
    let mut s = Settings::new(48_000);
    s.speech_codec_enabled = false;
    s.mic.capsule_enabled = false;
    let mut p = ic::engine(s, [0.0, 200.0, 8.0, 0.0]);
    let x = stimulus::tone(200.0, 0.5, 48_000.0, 4800);
    let mut digests = Vec::new();
    for _ in 0..2 {
        p.start_recording().unwrap();
        let mut ev = vec![VasEvent::default(); 1024];
        let _ = p.record(&x, &mut ev).unwrap();
        let _ = p.stop_recording(&mut ev).unwrap();
        digests.push(p.recording_digest(p.memory_status().count).unwrap());
    }
    assert_eq!(digests[0], digests[1]);
}
