//! Spec 018 User Story 2: the condition is described, kept and visible (tasks.md T011; spec ›
//! US2 AS1 – AS5; AS3, the C parity, is `ffi_parity battery_parity`). Report:
//! `cargo test -p rr_dr60_harness --test us2_battery -- --nocapture`.

use rr_dr60::__test_hooks;
use rr_dr60::battery::{self, ALKALINE_TRAJECTORY, ceiling_factor};
use rr_dr60::output::{VOLUME_6DB_OVER, VOLUME_REFERENCE};
use rr_dr60::{ChangeEntry, Error, Pipeline, Route, Stream};
use rr_dr60_harness::battery_checks as bat;
use rr_dr60_harness::changes_checks as cc;
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;

/// The factor the chain holds after `p` processes `x` one sample at a time, per sample.
fn trace(p: &mut Pipeline, x: &[f32]) -> Vec<f64> {
    x.iter()
        .map(|&v| {
            let mut y = [0.0f32; 1];
            let _ = p.process(&[v], &mut y).expect("ok");
            __test_hooks::battery_factor(p, Stream::OnePass).expect("the one-pass")
        })
        .collect()
}

/// US2 AS1 (FR-001): the value survives `reset` and `reconfigure` and is in force at once on
/// the first sample after each, with no transition and no change entry.
#[test]
fn as1_kept_through_reset_and_reconfigure() {
    for &rate in configs::measurement_rates() {
        let settings = bat::stage11_only(rate);
        let x = bat::clipped_tone(rate);
        let b = f64::from(ceiling_factor(2.4));
        let mut p = standard(settings);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_6DB_OVER).unwrap();
        p.set_battery_volts(2.4).unwrap();
        let mut y = vec![0.0f32; 2048];
        let _ = p.process(&x[..2048], &mut y).unwrap();
        for (what, act) in [
            (
                "reset",
                Box::new(|p: &mut Pipeline| p.reset()) as Box<dyn Fn(&mut Pipeline)>,
            ),
            (
                "reconfigure",
                Box::new(move |p: &mut Pipeline| p.reconfigure(settings).unwrap()),
            ),
        ] {
            act(&mut p);
            assert_eq!(p.battery_volts(), 2.4, "{rate}: kept by {what}");
            assert_eq!(
                p.meters().battery_ceiling,
                ceiling_factor(2.4),
                "{rate}: the rest read-out after {what}"
            );
            // The first produced sample carries the value at once (spec 011's rule): no
            // transition, and no change entry for it.
            let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
            let mut y = [0.0f32; 1];
            let info = p
                .process_with_events_and_changes(&x[..1], &mut y, &mut [], &mut log)
                .unwrap();
            assert!(
                bat::battery_entries(&log[..info.changes]).is_empty(),
                "{rate}: no entry after {what}"
            );
            let t = trace(&mut p, &x[1..64]);
            assert!(
                t.iter().all(|&f| f.to_bits() == b.to_bits()),
                "{rate}: at once after {what}: {t:?}"
            );
        }
    }
}

/// US2 AS2 (FR-001): a value outside 2.2 … 3.0 V or not finite is refused by the setter and by
/// the schedule, the value is unchanged and nothing is pending.
#[test]
fn as2_refusals_store_nothing() {
    for &rate in configs::measurement_rates() {
        let mut p = standard(configs::settings("default_output", rate));
        p.set_battery_volts(2.6).unwrap();
        let control = cc::control_index("battery_volts");
        for bad in [
            2.1f32,
            3.1,
            2.199_999,
            3.000_001,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert!(
                matches!(
                    p.set_battery_volts(bad),
                    Err(Error::InvalidBatteryVolts { volts }) if volts.to_bits() == bad.to_bits()
                ),
                "{rate}: the setter at {bad}"
            );
            for stream in [Stream::OnePass, Stream::Playback] {
                assert!(
                    matches!(
                        p.schedule_change(stream, 10, control, f64::from(bad), 1),
                        Err(Error::InvalidBatteryVolts { .. })
                    ),
                    "{rate}: the schedule at {bad} on {stream:?}"
                );
                assert_eq!(
                    p.pending_changes(stream).unwrap(),
                    0,
                    "{rate}: nothing pending"
                );
            }
            assert_eq!(p.battery_volts(), 2.6, "{rate}: unchanged after {bad}");
        }
        // The record stream has no battery control (spec 018 FR-001: the output clock only).
        assert!(matches!(
            p.schedule_change(Stream::Record, 10, control, 2.4, 1),
            Err(Error::ControlNotOnStream { .. })
        ));
        // The ends of the range are accepted.
        p.set_battery_volts(2.2).unwrap();
        p.set_battery_volts(3.0).unwrap();
        assert_eq!(p.battery_volts(), 3.0);
    }
}

/// US2 AS4 (FR-010): on a hard-clipped tone at 2.4 V the amplifier clip count equals the
/// twin's count of output samples above the knee's start (the knee is monotonic and the
/// identity below it) and the ceiling meter equals the factor; both rest on below-knee
/// material at the default; on the earphone route the count is 0 while the factor is
/// reported; the rest read-out follows the value.
#[test]
fn as4_meters_clip_count_and_factor() {
    for &rate in configs::measurement_rates() {
        let settings = bat::stage11_only(rate);
        let x = bat::clipped_tone(rate);
        let b = ceiling_factor(2.4);
        let knee_lo = 0.85 * f64::from(b);
        let mut p = standard(settings);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_6DB_OVER).unwrap();
        p.set_battery_volts(2.4).unwrap();
        assert_eq!(
            p.rest_meters().battery_ceiling,
            ceiling_factor(p.battery_volts()),
            "{rate}: the rest follows the value"
        );
        let mut total = 0u32;
        let mut twin = 0usize;
        for chunk in x.chunks(1024) {
            let mut y = vec![0.0f32; chunk.len()];
            let _ = p.process(chunk, &mut y).unwrap();
            let m = p.meters();
            // The first block carries the 3.0 → 2.4 V transition at once (fresh) so every block
            // reads the factor of 2.4 V.
            assert_eq!(
                m.battery_ceiling.to_bits(),
                b.to_bits(),
                "{rate}: the factor"
            );
            // The twin counts the f32-narrowed output above the start: valid because the knee is
            // the identity below its start and strictly increasing above it; a parabola sample
            // within half an f32 ULP above the start could disagree after narrowing (hardware
            // review B6), negligible on a clipped tone.
            let above = y.iter().filter(|&&v| f64::from(v).abs() > knee_lo).count();
            assert_eq!(
                m.amp_clip_count as usize, above,
                "{rate}: a block's clip count"
            );
            total += m.amp_clip_count;
            twin += above;
        }
        assert!(
            total > 0 && total as usize == twin,
            "{rate}: clipped at all"
        );
        // Below the knee at the default: both rest.
        let mut q = standard(settings);
        q.set_route(Route::Speaker);
        q.set_volume(VOLUME_REFERENCE).unwrap();
        let (_, quiet) = &bat::below_knee_stimuli(rate)[0];
        let mut y = vec![0.0f32; quiet.len()];
        let _ = q.process(quiet, &mut y).unwrap();
        assert_eq!(
            q.meters().amp_clip_count,
            0,
            "{rate}: no clipping below the knee"
        );
        assert_eq!(
            q.meters().battery_ceiling,
            1.0,
            "{rate}: the default's factor"
        );
        // The earphone route: the count rests at 0, the factor is reported.
        let mut e = standard(settings);
        e.set_route(Route::Earphone);
        e.set_volume(1.0).unwrap();
        e.set_battery_volts(2.2).unwrap();
        let mut y = vec![0.0f32; x.len()];
        let _ = e.process(&x, &mut y).unwrap();
        assert_eq!(e.meters().amp_clip_count, 0, "{rate}: the earphone route");
        assert_eq!(
            e.meters().battery_ceiling.to_bits(),
            ceiling_factor(2.2).to_bits(),
            "{rate}: the factor is reported on the earphone route"
        );
    }
}

/// US2 AS5 (FR-006): the published law equals the engine: at each trajectory row `ceiling_db`
/// is within 0.05 dB of the register and the engine's clipped peak equals `ceiling_factor` to
/// the bit; the NiMH figure sits in A-056's 2.4 – 2.5 V.
#[test]
fn as5_published_law_equals_the_engine() {
    assert!((2.4..=2.5).contains(&battery::NIMH_PAIR_VOLTS));
    assert_eq!(battery::BATTERY_VOLTS_RANGE, (2.2, 3.0));
    assert_eq!(battery::DEFAULT_BATTERY_VOLTS, 3.0);
    for (i, &(hours, volts)) in ALKALINE_TRAJECTORY.iter().enumerate() {
        assert!(
            (f64::from(battery::ceiling_db(volts)) - bat::REGISTER_DB[i]).abs() <= 0.05,
            "{hours} h: {} vs {}",
            battery::ceiling_db(volts),
            bat::REGISTER_DB[i]
        );
    }
    for &rate in configs::measurement_rates() {
        let settings = bat::stage11_only(rate);
        let x = bat::clipped_tone(rate);
        for &(_, volts) in &ALKALINE_TRAJECTORY {
            let y = bat::run_at(
                &standard,
                settings,
                Route::Speaker,
                VOLUME_6DB_OVER,
                volts,
                &x,
            );
            let peak = bat::peak_after_settle(&y, rate) as f32;
            assert_eq!(
                peak.to_bits(),
                ceiling_factor(volts).to_bits(),
                "{rate}: {volts} V"
            );
        }
    }
}

/// US2 AS1 / FR-001 (coach review 2): a value landed on one stream becomes pending for the
/// other: a change scheduled and landed on the playback stream is in force on the one-pass's
/// first sample afterwards (a tag-0 entry in its log), and the reverse.
#[test]
fn as1_cross_stream_landing() {
    for &rate in configs::measurement_rates() {
        let settings = bat::stage11_only(rate);
        let x = bat::clipped_tone(rate);
        let b = |v: f32| f64::from(ceiling_factor(v));
        // Playback → one-pass.
        let mut p = standard(settings);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_6DB_OVER).unwrap();
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        let r = cc::run_scheduled(
            &mut p,
            cc::Source::Playback(1),
            &[cc::Change::battery(Stream::Playback, 1000, 2.4, 3)],
            cc::Partition::Fixed(1024),
        );
        assert_eq!(
            bat::battery_entries(&r.log).len(),
            1,
            "{rate}: landed on the playback"
        );
        assert_eq!(
            p.battery_volts(),
            2.4,
            "{rate}: the reading follows the landing"
        );
        let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let mut y = [0.0f32; 1];
        let info = p
            .process_with_events_and_changes(&x[..1], &mut y, &mut [], &mut log)
            .unwrap();
        let e = bat::battery_entries(&log[..info.changes]);
        assert_eq!(e.len(), 1, "{rate}: the one-pass logs the mirrored landing");
        assert_eq!(
            (e[0].tag, e[0].after),
            (0, f64::from(2.4f32)),
            "{rate}: tag 0"
        );
        assert_eq!(
            __test_hooks::battery_factor(&p, Stream::OnePass).unwrap(),
            b(2.4),
            "{rate}: in force on the one-pass's first sample (at once: a fresh chain)"
        );
        // One-pass → playback.
        let mut q = standard(settings);
        q.set_route(Route::Speaker);
        q.set_volume(VOLUME_6DB_OVER).unwrap();
        q.start_recording().unwrap();
        let _ = q.record(&x, &mut []).unwrap();
        let _ = q.stop_recording(&mut []).unwrap();
        let r = cc::run_scheduled(
            &mut q,
            cc::Source::OnePass(&x[..4096]),
            &[cc::Change::battery(Stream::OnePass, 500, 2.2, 5)],
            cc::Partition::Fixed(1024),
        );
        assert_eq!(
            bat::battery_entries(&r.log).len(),
            1,
            "{rate}: landed on the one-pass"
        );
        q.start_playback(1).unwrap();
        let mut y = vec![0.0f32; 64];
        let info = q.play_with_changes(&mut y, &mut log).unwrap();
        let e = bat::battery_entries(&log[..info.changes]);
        // `start_playback` takes the value as a start value (spec 015 D3: no entry).
        assert!(e.is_empty(), "{rate}: a start value, no entry: {e:?}");
        assert_eq!(
            __test_hooks::battery_factor(&q, Stream::Playback).unwrap(),
            b(2.2),
            "{rate}: in force on the playback's first sample"
        );
    }
}
