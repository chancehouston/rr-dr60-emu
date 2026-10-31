//! Spec 021 seven-rate matrix (tasks.md T014): every cell's endpoint identity (exact where the
//! switched stage is last, the convergence bound elsewhere), the blend identity on the exact
//! chain cells, the partitions, stage 10's host onset table, the 48 kHz click rows reported, the
//! level table and the tight bounds of `MAX_CHANGES_PER_CALL`.
//! Report: `cargo test -p rr_dr60_harness --test bypass_matrix -- --nocapture`.

use rr_dr60::{ChangeEntry, Pipeline, Route, Stream, VasEvent};
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn rates() -> Vec<u32> {
    if cfg!(coverage) {
        configs::measurement_rates().to_vec()
    } else {
        configs::all_rates().to_vec()
    }
}

/// Every cell at every rate: the endpoint identities.
#[test]
fn cells() {
    let mut rows = Vec::new();
    for rate in rates() {
        for cell in bp::cells() {
            rows.extend(bp::endpoint_identity(&standard, &cell, rate));
        }
    }
    finish(rows);
}

/// The blend identity on the exact chain cells and stage 10's trace at every rate.
#[test]
fn blend_identity_table() {
    let mut rows = Vec::new();
    for rate in rates() {
        for cell in bp::cells().iter().filter(|c| c.exact && c.stage != 10) {
            rows.extend(bp::blend_identity(&standard, cell, rate));
        }
        rows.extend(bp::stage10_trace(&standard, rate));
    }
    finish(rows);
}

/// Stage 10's host onset per rate (the first differing and the first identical host sample
/// against the twin, the bound); the live tail where the twin is shorter.
#[test]
fn stage10_host_onset_table() {
    let mut rows = Vec::new();
    for rate in rates() {
        for cell in bp::cells().iter().filter(|c| {
            c.exact
                && c.stream == Stream::Playback
                && matches!(c.config, "playback_only" | "output_only_amp")
        }) {
            rows.extend(bp::live_tail(&standard, cell, rate));
        }
    }
    finish(rows);
}

/// The partitions on one cell per stage at every rate.
#[test]
fn partitions_table() {
    let mut rows = Vec::new();
    for rate in rates() {
        let cells = bp::cells();
        for stage in [10u8, 11, 12, 13] {
            if let Some(c) = cells.iter().find(|c| c.stage == stage && c.exact) {
                rows.extend(bp::partitions(&standard, c, rate));
            }
        }
    }
    finish(rows);
}

/// FR-007: the asserted rows at 6 kHz and reported at 48 kHz; the level table; the reversal
/// at every rate.
#[test]
fn click_and_level_tables() {
    let mut rows = bp::click_table(&standard, 6000);
    rows.extend(bp::click_table(&standard, 48_000));
    rows.extend(bp::level_table());
    for rate in rates() {
        rows.extend(bp::reversal(&standard, rate));
    }
    finish(rows);
}

/// Spec 021 research R-05 (the 017 pattern): the one-pass attains `MAX_CHANGES_PER_CALL` = 142
/// in one call (64 scheduled playback-side + 3 plain + 3 mirrored switches + 64 record-side +
/// 8 cross-stream record values), the player at most 68 (64 + 4 plain); the C bound names it.
#[test]
fn the_tight_bounds() {
    let rate = 48_000;
    let mut s = bp::settings(bp::DEFAULT_CHAIN, rate);
    s.vas.enabled = true;
    // The player: 64 scheduled switches at position 0 plus the four plain controls.
    let stored = bp::stored_tone(1000.0, 0.5);
    let mut p = bp::engine(&standard, s, &stored, Route::Speaker, 0.5);
    p.start_playback(1).unwrap();
    p.set_route(Route::Earphone);
    p.set_volume(0.7).unwrap();
    p.set_playback_speed(4).unwrap();
    p.set_battery_volts(2.8).unwrap();
    for i in 0..64u64 {
        let on = i % 2 == 1;
        p.schedule_change(Stream::Playback, 0, 23, if on { 1.0 } else { 0.0 }, i + 1).unwrap();
    }
    let mut y = vec![0.0f32; 256];
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let info = p.play_with_changes(&mut y, &mut log).unwrap();
    assert_eq!(info.changes, 68, "the player's tight bound");
    p.stop_playback().unwrap();
    // The one-pass: the player landed the three switches (mirrored, 3), 64 scheduled switches
    // on its playback-side schedule, the three plain controls (route, volume, battery: the
    // player's landings made them differ from the one-pass's last values), 64 record-side
    // entries and the 8 cross-stream record values landed on the record stream earlier.
    let mut s2 = s;
    s2.vas.enabled = true;
    let mut q = bp::engine(&standard, s2, &stored, Route::Speaker, 0.5);
    q.start_playback(1).unwrap();
    q.set_route(Route::Earphone);
    q.set_volume(0.7).unwrap();
    q.set_battery_volts(2.8).unwrap();
    for (i, c) in [22u32, 23, 24].iter().enumerate() {
        q.schedule_change(Stream::Playback, 0, *c, 0.0, 100 + i as u64).unwrap();
    }
    let _ = q.play_with_changes(&mut y, &mut log).unwrap();
    q.stop_playback().unwrap();
    // The eight record-side values landed on the recorder.
    q.start_recording().unwrap();
    for (i, id) in [
        "vas.sensitivity",
        "vas.threshold_dbfs",
        "vas.hang_ms",
        "vas.onset_ms",
        "agc.knee_dbfs",
        "agc.attack_ms",
        "agc.release_ms",
        "mic.hiss_offset_db",
    ]
    .iter()
    .enumerate()
    {
        let control = cc::control_index(id);
        let value = match *id {
            "vas.sensitivity" => 4.0,
            "vas.threshold_dbfs" => -45.0,
            "vas.hang_ms" => 150.0,
            "vas.onset_ms" => 30.0,
            "agc.knee_dbfs" => -3.0,
            "agc.attack_ms" => 4.0,
            "agc.release_ms" => 1000.0,
            _ => 2.0,
        };
        q.schedule_change(Stream::Record, 0, control, value, 200 + i as u64).unwrap();
    }
    let x = bp::host_tone(1000.0, 0.05, rate);
    let mut ev = vec![VasEvent::default(); 64];
    let _ = q.record(&x, &mut ev).unwrap();
    let _ = q.stop_recording(&mut ev).unwrap();
    // Now the one-pass call: 64 switch entries + 64 record-side entries scheduled at 0.
    for i in 0..64u64 {
        q.schedule_change(Stream::OnePass, 0, 23, if i % 2 == 1 { 1.0 } else { 0.0 }, 300 + i).unwrap();
    }
    let knee = cc::control_index("agc.knee_dbfs");
    for i in 0..64u64 {
        q.schedule_change(Stream::OnePass, 0, knee, -(1.0 + (i % 20) as f64), 400 + i).unwrap();
    }
    let n = 64;
    let mut yo = vec![0.0f32; n];
    let mut evo = vec![VasEvent::default(); q.max_events(n)];
    let info = q
        .process_with_events_and_changes(&x[..n], &mut yo, &mut evo, &mut log)
        .unwrap();
    assert_eq!(info.changes, rr_dr60::changes::MAX_CHANGES_PER_CALL, "the one-pass attains the bound");
    assert_eq!(rr_dr60::changes::MAX_CHANGES_PER_CALL, 142);
    let _ = Pipeline::new(s).unwrap();
    let _ = cc::run(&mut q, Source::OnePass(&x[..8]), Partition::Fixed(8));
    let _ = Change::speed;
}
