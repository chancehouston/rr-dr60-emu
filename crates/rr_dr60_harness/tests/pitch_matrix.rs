//! Spec 023: the seven-rate report (tasks.md T012): every cell of levels × modes on the tones,
//! the vowel and the glide; the lengths; the content-offset pin; the partitions; the shift
//! digests against the reference; the tight bounds 142 / 69.

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::speed::SpeedMode;
use rr_dr60::{ChangeEntry, Route, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::configs;
use rr_dr60_harness::pitch_checks::{self as pc, Req};

fn report(rows: &[rr_dr60_harness::checks::MeasurementResult]) {
    for r in rows {
        println!("{r}");
    }
    assert_all(rows);
}

/// Every cell at the seven rates: the tones in both modes at the four levels.
#[test]
fn tones_at_the_seven_rates() {
    for rate in configs::all_rates() {
        report(&pc::tone_rows(&standard, rate));
    }
}

/// The vowel and the glide at the seven rates.
#[test]
fn vowel_and_glide_at_the_seven_rates() {
    for rate in configs::all_rates() {
        report(&pc::vowel_and_glide(&standard, rate));
    }
}

/// The lengths, the place and the exact onset at the seven rates; the content-offset pin.
#[test]
fn lengths_and_offsets_at_the_seven_rates() {
    for rate in configs::all_rates() {
        report(&pc::lengths(&standard, rate));
        report(&pc::offset_rows(&standard, rate));
    }
}

/// The partition identity (spec 015 FR-008): the default engine with the vowel stored and a
/// schedule of level and mode changes at block sizes 1, 64, 4096 and irregular: the same output
/// and log.
#[test]
fn partitions() {
    let x = pc::vowel(1.0);
    for &rate in configs::measurement_rates() {
        let n = u64::from(rate) / 50;
        let p0 = u64::from(rate) / 4;
        let sched = [
            Change::speed(p0, 4, 1),
            Change::mode(p0 + n, SpeedMode::PitchKept, 2),
            Change::speed(p0 + 3 * n, 1, 3),
            Change::mode(p0 + 5 * n, SpeedMode::PitchWithSpeed, 4),
            Change::speed(p0 + 7 * n, 3, 5),
            Change::mode(p0 + 7 * n, SpeedMode::PitchKept, 6),
            Change::speed(p0 + 9 * n, 5, 7),
        ];
        let runs: Vec<cc::Run> = [
            Partition::Fixed(1),
            Partition::Fixed(64),
            Partition::Fixed(4096),
            Partition::Irregular,
        ]
        .into_iter()
        .map(|part| {
            let mut p = pc::engine(&standard, "speed_default", rate, &x);
            p.set_route(Route::Speaker);
            p.set_playback_speed(1).unwrap();
            cc::run_scheduled(&mut p, Source::Playback(1), &sched, part)
        })
        .collect();
        for r in &runs[1..] {
            assert!(cc::same(&runs[0].out, &r.out), "{rate}: the output");
            assert_eq!(runs[0].log, r.log, "{rate}: the log");
        }
        assert_eq!(
            cc::entries_of(&runs[0].log, 30).len(),
            3,
            "{rate}: the mode entries"
        );
        assert_eq!(
            cc::entries_of(&runs[0].log, 2).len(),
            4,
            "{rate}: the level entries"
        );
    }
}

/// The engine against the reference at 6 kHz on the off-grid tone, the vowel, the glide and
/// the square: constant levels in pitch-kept mode and three schedules (level changes, mode
/// changes, both): bit-exact outputs and equal shift sequences, codec off and on (the codec's
/// decoded stream is the stored stream the reference reads).
#[test]
fn shift_digests_against_the_reference() {
    let stims: [(&str, Vec<f32>); 4] = [
        ("1234.5 Hz", pc::tone(pc::OFFGRID_HZ, 1.0)),
        ("vowel", pc::vowel(1.0)),
        ("glide", pc::glide(1.0)),
        ("square17", pc::square17(6000)),
    ];
    let schedules: [pc::Case; 3] = [
        (
            "levels",
            1,
            SpeedMode::PitchKept,
            vec![
                (1200, Req::Level(5)),
                (2500, Req::Level(2)),
                (3700, Req::Level(3)),
                (4900, Req::Level(4)),
            ],
        ),
        (
            "modes",
            1,
            SpeedMode::PitchWithSpeed,
            vec![
                (1200, Req::Mode(SpeedMode::PitchKept)),
                (3000, Req::Mode(SpeedMode::PitchWithSpeed)),
                (4500, Req::Mode(SpeedMode::PitchKept)),
            ],
        ),
        (
            "both",
            1,
            SpeedMode::PitchKept,
            vec![
                (1200, Req::Level(4)),
                (1200, Req::Mode(SpeedMode::PitchWithSpeed)),
                (2700, Req::Mode(SpeedMode::PitchKept)),
                (3600, Req::Level(3)),
                (4800, Req::Level(2)),
            ],
        ),
    ];
    for config in ["speed_only", "speed_only_codec"] {
        for (name, x) in &stims {
            for level in pc::LEVELS {
                let (mism, eq, a, b) =
                    pc::against_reference(&standard, config, x, level, SpeedMode::PitchKept, &[]);
                assert_eq!(mism, 0, "{config} {name} L{level}: mismatches");
                assert!(eq, "{config} {name} L{level}: shifts {a} vs {b}");
                println!("{config} {name} L{level}: bit-exact, {a} frames");
            }
            for (sname, level, mode, sched) in &schedules {
                let (mism, eq, a, b) =
                    pc::against_reference(&standard, config, x, *level, *mode, sched);
                assert_eq!(mism, 0, "{config} {name} {sname}: mismatches");
                assert!(eq, "{config} {name} {sname}: shifts {a} vs {b}");
                println!("{config} {name} {sname}: bit-exact, {a} frames");
            }
        }
    }
}

/// The tight bounds (research R-07, coach 4): `MAX_CHANGES_PER_CALL` stays 142; the player
/// attains 69 in one call (64 scheduled at position 0 plus the five plain changes).
#[test]
fn the_tight_bounds() {
    assert_eq!(MAX_CHANGES_PER_CALL, 142);
    let x = pc::vowel(0.5);
    let mut p = pc::engine(&standard, "speed_default", 48_000, &x);
    for i in 0..64u64 {
        p.schedule_change(Stream::Playback, 0, 1, 0.1 + 0.001 * i as f64, i + 1)
            .unwrap();
    }
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, 30, 1.0, 99),
        Err(rr_dr60::Error::ScheduleFull { .. })
    ));
    p.start_playback(1).unwrap();
    p.set_route(Route::Earphone);
    p.set_volume(0.25).unwrap();
    p.set_playback_speed(5).unwrap();
    p.set_battery_volts(2.5).unwrap();
    p.set_playback_speed_mode(SpeedMode::PitchKept);
    let mut y = vec![0.0f32; 512];
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let info = p.play_with_changes(&mut y, &mut log).unwrap();
    assert_eq!(info.changes, 69, "the player's attained count");
    assert!(cc::entries_of(&log[..69], 30).len() == 1);
}

/// The click counts (reported).
#[test]
fn click_counts() {
    report(&pc::click_rows(&standard));
}
