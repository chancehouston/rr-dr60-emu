//! Spec 024's seven-rate matrix (research R-07, R-11): every case of the design script's table,
//! the placement identity, the partitions, the cross-rate table, the tight bounds.

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60::__test_hooks::InsertMutation;
use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{ChangeEntry, Pipeline, Route, Stream, VasEvent};
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::changes_checks as cc;
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::insert_checks::{self as ic, CONTROLS};
use rr_dr60_harness::stimulus;

/// Every case of the design script's table at the seven rates (FR-004, SC-001).
#[test]
fn insert_matrix_response() {
    let rows = ic::response_rows(&configs::all_rates());
    assert_eq!(
        rows.len(),
        rr_dr60_harness::band_filter_ref::REF_CASES.len()
    );
    let worst = rows.iter().map(|r| r.rel_err).fold(0.0, f64::max);
    let worst_phase = rows.iter().map(|r| r.phase_err_deg).fold(0.0, f64::max);
    println!(
        "matrix: {} cases, worst tolerance use {worst:.3}, worst phase {worst_phase:.2e}°",
        rows.len()
    );
    assert!(worst <= 1.0 && worst_phase <= 2.0);
    // The implementation bound (hardware post-implementation B9): about 0.01 dB with the 1e-6
    // floor, so a missing prewarp (0.01 dB near the corner at 48 kHz) fails here too.
    assert!(worst <= 0.1, "the implementation bound: {worst}");
}

/// The placement identity at the seven rates.
#[test]
fn insert_matrix_placement() {
    for rate in configs::all_rates() {
        let (n, bad) = ic::placement_identity(rate, [2.0, 1200.0, 2.5, -3.0]);
        assert_eq!(bad, 0, "{rate}: {bad} of {n}");
    }
}

/// US3 AS2: the partitions at block sizes 1, 64, 4096 and irregular give identical outputs and
/// logs with scheduled parameter changes (a frequency move, a type change, a gain change).
#[test]
fn insert_matrix_partitions() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::tone(900.0, 0.2, f64::from(rate), rate as usize / 4);
        let run = |block: &dyn Fn(usize) -> usize| {
            let mut p = ic::engine(ic::oracle_settings(rate), [0.0, 1000.0, 0.7071, 0.0]);
            let q = u64::from(rate) / 40;
            p.schedule_change(Stream::OnePass, q, CONTROLS[1], 3000.0, 1)
                .unwrap();
            p.schedule_change(Stream::OnePass, q + 7, CONTROLS[0], 2.0, 2)
                .unwrap();
            p.schedule_change(Stream::OnePass, 3 * q, CONTROLS[3], 9.0, 3)
                .unwrap();
            let mut out = vec![0.0f32; x.len()];
            let mut entries = Vec::new();
            let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
            let mut ev = vec![VasEvent::default(); 64];
            let (mut pos, mut k) = (0, 0);
            while pos < x.len() {
                let n = block(k).min(x.len() - pos);
                let info = p
                    .process_with_events_and_changes(
                        &x[pos..pos + n],
                        &mut out[pos..pos + n],
                        &mut ev,
                        &mut log,
                    )
                    .unwrap();
                entries.extend_from_slice(&log[..info.changes]);
                pos += n;
                k += 1;
            }
            (out, entries)
        };
        let base = run(&|_| 1);
        for (name, b) in [
            ("64", &(|_: usize| 64usize) as &dyn Fn(usize) -> usize),
            ("4096", &|_: usize| 4096usize),
            ("irregular", &|k: usize| [7usize, 333, 1, 64, 1000][k % 5]),
        ] {
            let got = run(b);
            assert_eq!(got.0, base.0, "{rate} {name}: outputs");
            assert_eq!(got.1, base.1, "{rate} {name}: logs");
        }
        assert_eq!(base.1.len(), 3);
    }
}

/// The cross-rate table (hardware plan review B16): the low-pass at Q 0.7071 read at 2500 Hz,
/// by corner and host rate (reported, as the design script prints it; the two cells the guide
/// quotes are asserted).
#[test]
fn insert_matrix_cross_rate() {
    for corner in [1000.0f32, 1500.0, 2000.0, 2500.0] {
        let mut line = format!("cross-rate {corner:6.0}");
        for rate in configs::all_rates() {
            let c = rr_dr60_harness::band_filter_ref::RefCase {
                set: 99,
                ftype: 0,
                rate,
                freq: corner,
                q: 0.7071,
                gain_db: 0.0,
                f_applied: f64::from(corner),
                length: rate as usize,
                dc: 1.0,
                nyquist: 0.0,
                first: 0,
                count: 0,
            };
            let ir = ic::impulse_response(&c);
            let h = rr_dr60_harness::analysis::dtft(&ir, 2500.0, f64::from(rate)).norm();
            let d = rr_dr60_harness::analysis::db(h);
            line.push_str(&format!(" {d:8.2}"));
            // The two cells the guide quotes, from the design script's print.
            if corner == 2000.0 && rate == 6000 {
                assert!((d - -13.53).abs() < 0.01, "{d}");
            }
            if corner == 2000.0 && rate == 48_000 {
                assert!((d - -5.41).abs() < 0.01, "{d}");
            }
        }
        println!("{line}");
    }
}

/// US3 AS3: the slot block's growth inside the memory (reported) and the footprint.
#[test]
fn insert_matrix_footprint() {
    let p = Pipeline::new(rr_dr60::Settings::new(48_000)).unwrap();
    println!(
        "footprint {} bytes; the slot block's fields grew by {} bytes per recording slot (computed from the fields)",
        p.footprint_bytes(),
        4 + 4 * rr_dr60::components::COMPONENT_PARAMETER_BUDGET
    );
}

/// Research R-11: the one-pass attains 146 (64 + 64 + 3 + 8 + 4 + 3) of the bound 158; a
/// 142-entry buffer receives the first 142 with the count reported.
#[test]
fn insert_matrix_the_tight_bounds() {
    assert_eq!(rr_dr60::changes::MAX_CHANGES_PER_CALL, 158);
    let rate = 48_000;
    let mut s = bp::settings(bp::DEFAULT_CHAIN, rate);
    s.vas.enabled = true;
    let stored = bp::stored_tone(1000.0, 0.5);
    let mut q = bp::engine(&standard, s, &stored, Route::Speaker, 0.5);
    q.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    q.start_playback(1).unwrap();
    q.set_route(Route::Earphone);
    q.set_volume(0.7).unwrap();
    q.set_battery_volts(2.8).unwrap();
    for (i, c) in [22u32, 23, 24].iter().enumerate() {
        q.schedule_change(Stream::Playback, 0, *c, 0.0, 100 + i as u64)
            .unwrap();
    }
    let mut y = vec![0.0f32; 256];
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let _ = q.play_with_changes(&mut y, &mut log).unwrap();
    q.stop_playback().unwrap();
    q.start_recording().unwrap();
    let values = [
        ("vas.sensitivity", 4.0),
        ("vas.threshold_dbfs", -45.0),
        ("vas.hang_ms", 150.0),
        ("vas.onset_ms", 30.0),
        ("agc.knee_dbfs", -3.0),
        ("agc.attack_ms", 4.0),
        ("agc.release_ms", 1000.0),
        ("mic.hiss_offset_db", 2.0),
        ("band_filter.type", 1.0),
        ("band_filter.frequency_hz", 1500.0),
        ("band_filter.q", 2.0),
        ("band_filter.gain_db", -3.0),
    ];
    for (i, (id, v)) in values.iter().enumerate() {
        q.schedule_change(Stream::Record, 0, cc::control_index(id), *v, 200 + i as u64)
            .unwrap();
    }
    let x = bp::host_tone(1000.0, 0.05, rate);
    let mut ev = vec![VasEvent::default(); 64];
    let rec = q.record_with_changes(&x, &mut ev, &mut log).unwrap();
    println!("the recorder's call: {} entries", rec.changes);
    let _ = q.stop_recording(&mut ev).unwrap();
    for i in 0..64u64 {
        q.schedule_change(
            Stream::OnePass,
            0,
            23,
            if i % 2 == 1 { 1.0 } else { 0.0 },
            300 + i,
        )
        .unwrap();
    }
    let knee = cc::control_index("agc.knee_dbfs");
    for i in 0..64u64 {
        q.schedule_change(Stream::OnePass, 0, knee, -(1.0 + (i % 20) as f64), 400 + i)
            .unwrap();
    }
    let n = 64;
    let mut yo = vec![0.0f32; n];
    let mut evo = vec![VasEvent::default(); q.max_events(n)];
    let mut twin = q.clone();
    let mut short = vec![ChangeEntry::default(); 142];
    let info = q
        .process_with_events_and_changes(&x[..n], &mut yo, &mut evo, &mut short)
        .unwrap();
    println!(
        "the one-pass's call: {} entries of the bound {}",
        info.changes,
        rr_dr60::changes::MAX_CHANGES_PER_CALL
    );
    assert_eq!(info.changes, 146, "the one-pass attains 146 (R-11)");
    // The short buffer receives exactly the first 142 of the full log (coach 16); the
    // recorder's 64 and the player's 69 are pinned by record_matrix and pitch_matrix.
    let mut full = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut yt = vec![0.0f32; n];
    let mut evt = vec![VasEvent::default(); twin.max_events(n)];
    let _ = twin
        .process_with_events_and_changes(&x[..n], &mut yt, &mut evt, &mut full)
        .unwrap();
    assert_eq!(short[..], full[..142], "the first 142");
    let _ = InsertMutation::default();
}

/// US1 AS5 (research R-07; hardware plan review B12): the gain at which a quiet recording stops
/// pausing, per type at levels 3 and 5, and a Q 10 row at 0 dB; 10 s runs, a lower estimate
/// (spec 012's lesson). Release-only; the guide quotes these figures.
#[test]
#[ignore = "release-mode bisection; run with --release --ignored"]
fn insert_matrix_gate_offset_bisection() {
    if cfg!(debug_assertions) {
        eprintln!("needs --release; skipped");
        return;
    }
    let rate = 48_000;
    for level in [3u8, 5] {
        for t in 0..4u8 {
            let g = ic::gate_stop_gain(rate, level, t, 0.7071, 10.0);
            println!(
                "gate at level {level}, type {t} at 1 kHz, Q 0.7071: stops pausing from {g:?} dB of gain"
            );
        }
        let q10 = ic::quiet_pauses(rate, level, [0.0, 1000.0, 10.0, 0.0], 10.0);
        println!(
            "gate at level {level}, low-pass 1 kHz Q 10 at 0 dB: {}",
            if q10 { "pauses" } else { "does not pause" }
        );
    }
}
