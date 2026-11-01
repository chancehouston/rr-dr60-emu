//! Spec 023 US2: a live control like the level (AS1 – AS7).

use rr_dr60::speed::SpeedMode;
use rr_dr60::{
    ChangeEntry, ControlKind, ControlTiming, ControlValue, Error, Pipeline, Settings, Stream,
};
use rr_dr60_ffi::{
    RrDr60Status, rr_dr60_create, rr_dr60_destroy, rr_dr60_get_playback_speed_mode,
    rr_dr60_playback_length_samples_in_mode, rr_dr60_set_playback_speed_mode,
};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::pitch_checks::{self as pc, Req};
use rr_dr60_harness::{configs, controls_checks};

const MODE: u32 = 30;

fn report(rows: &[rr_dr60_harness::checks::MeasurementResult]) {
    for r in rows {
        println!("{r}");
    }
    assert_all(rows);
}

/// AS1 (FR-005): the mode blend against the instant switch; level 3 at once.
#[test]
fn as1_mode_blend() {
    report(&pc::mode_change_rows(&standard));
}

/// AS2 (FR-005): a level change in pitch-kept mode is frame-fed: the reading A/B.
#[test]
fn as2_level_change_reading_a_b() {
    report(&pc::level_change_rows(&standard));
}

/// AS3 (FR-001): kept by `reset` and `reconfigure`; a plain set of the mode in force is not
/// logged, a changed one is (tag 0); a scheduled set to the value in force is a logged no-op.
#[test]
fn as3_lifecycle() {
    let x = pc::vowel(0.5);
    for &rate in configs::measurement_rates() {
        let s = configs::settings("speed_default", rate);
        let mut p = pc::engine(&standard, "speed_default", rate, &x);
        assert_eq!(p.playback_speed_mode(), SpeedMode::PitchWithSpeed);
        p.set_playback_speed_mode(SpeedMode::PitchKept);
        p.reset();
        assert_eq!(
            p.playback_speed_mode(),
            SpeedMode::PitchKept,
            "{rate}: reset"
        );
        p.reconfigure(s).unwrap();
        assert_eq!(
            p.playback_speed_mode(),
            SpeedMode::PitchKept,
            "{rate}: reconfigure"
        );
        // A set before `start_playback` is adopted at the start, no entry (spec 015 D3); a plain
        // set of the value in force during a playback is not logged; a changed one lands at the
        // next call's first sample with tag 0 and the stream word.
        p.set_playback_speed(1).unwrap();
        p.set_playback_speed_mode(SpeedMode::PitchWithSpeed);
        let run = cc::run(&mut p, Source::Playback(1), Partition::Fixed(512));
        assert!(
            cc::entries_of(&run.log, MODE).is_empty(),
            "{rate}: adopted at the start"
        );
        p.start_playback(1).unwrap();
        let mut y = vec![0.0f32; 512];
        let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let info = p.play_with_changes(&mut y, &mut log).unwrap();
        assert!(cc::entries_of(&log[..info.changes], MODE).is_empty());
        p.set_playback_speed_mode(SpeedMode::PitchWithSpeed); // the value in force
        let info = p.play_with_changes(&mut y, &mut log).unwrap();
        assert!(
            cc::entries_of(&log[..info.changes], MODE).is_empty(),
            "{rate}: the value in force"
        );
        p.set_playback_speed_mode(SpeedMode::PitchKept);
        let info = p.play_with_changes(&mut y, &mut log).unwrap();
        let e = cc::entries_of(&log[..info.changes], MODE);
        assert_eq!(e.len(), 1, "{rate}: one entry");
        assert_eq!(
            (e[0].position, e[0].before, e[0].after, e[0].tag),
            (1024, 0.0, 1.0, 0)
        );
        assert_eq!(e[0].stream, Stream::Playback.word());
        p.stop_playback().unwrap();
        // A scheduled set to the value in force: a logged no-op (before == after).
        p.set_playback_speed_mode(SpeedMode::PitchWithSpeed); // adopted at the start
        let run = cc::run_scheduled(
            &mut p,
            Source::Playback(1),
            &[Change::mode(100, SpeedMode::PitchWithSpeed, 7)],
            Partition::Fixed(512),
        );
        let e = cc::entries_of(&run.log, MODE);
        assert_eq!(e.len(), 1);
        assert_eq!(
            (e[0].position, e[0].before, e[0].after, e[0].tag),
            (100, 0.0, 0.0, 7)
        );
        assert_eq!(p.playback_speed_mode(), SpeedMode::PitchWithSpeed);
    }
}

/// AS4 (FR-006): the control manifest's entry at index 30, and entry 2's reworded sentence.
#[test]
fn as4_control_manifest() {
    let c = rr_dr60::controls();
    assert_eq!(c.len(), 31);
    let e = &c[30];
    assert_eq!(e.identifier, "playback_speed_mode");
    assert_eq!(e.name, "Playback speed mode");
    assert_eq!(e.kind, ControlKind::ModelInternal);
    assert_eq!(e.timing, ControlTiming::Live);
    assert_eq!(e.paths, rr_dr60::ControlPaths::PLAYBACK);
    assert_eq!(e.stage, 9);
    assert_eq!(e.unit, "");
    match e.value {
        ControlValue::Choice { options, default } => {
            assert_eq!(options.len(), 2);
            assert_eq!(
                (options[0].identifier, options[0].value),
                ("pitch_with_speed", 0)
            );
            assert_eq!((options[1].identifier, options[1].value), ("pitch_kept", 1));
            assert_eq!(default, Some(0));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(e.sources, ["S-001", "S-012"]);
    assert_eq!(e.assumptions, ["A-054"]);
    assert_eq!(
        e.description,
        "Chooses whether slow and fast playback moves the pitch with the speed (the modeled default) or keeps it by time-scaling the decoded stream."
    );
    assert_eq!(
        c[2].description,
        "Plays recordings slower or faster than normal; whether the pitch follows the speed is the speed mode's choice."
    );
    assert_eq!(controls_checks::LIVE[4], "playback_speed_mode");
    assert_eq!(
        rr_dr60::control("playback_speed_mode").map(|e| e.stage),
        Some(9)
    );
    assert_eq!(
        rr_dr60::changes::clock_of(30).unwrap(),
        rr_dr60::Clock::Output
    );
}

/// AS5 (FR-001): the refusals: 0.5 an invalid control value, 2 an invalid mode; the one-pass
/// and record streams; through C status 38 with the mode unchanged.
#[test]
fn as5_refusals() {
    let s = Settings::new(48_000);
    let mut p = Pipeline::new(s).unwrap();
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, MODE, 0.5, 1),
        Err(Error::InvalidControlValue { control: 30, .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, MODE, 2.0, 1),
        Err(Error::InvalidSpeedMode { mode: 2 })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, MODE, -1.0, 1),
        Err(Error::InvalidControlValue { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, MODE, f64::NAN, 1),
        Err(Error::InvalidControlValue { .. })
    ));
    for stream in [Stream::OnePass, Stream::Record] {
        assert!(matches!(
            p.schedule_change(stream, 0, MODE, 1.0, 1),
            Err(Error::ControlNotOnStream { control: 30, .. })
        ));
    }
    assert_eq!(p.pending_changes(Stream::Playback), Ok(0));
    assert!(p.schedule_change(Stream::Playback, 0, MODE, 1.0, 1).is_ok());
    assert_eq!(p.pending_changes(Stream::Playback), Ok(1));
    assert_eq!(
        Error::InvalidSpeedMode { mode: 2 }.to_string(),
        "invalid playback speed mode 2: expected 0 or 1"
    );
    assert_eq!(rr_dr60::speed::SpeedMode::from_u32(2), None);
    // C.
    let cs = configs::c_settings(&s);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&cs, &mut h) }, RrDr60Status::Ok);
    let mut mode = 9u32;
    // SAFETY: a live handle and a writable out-pointer.
    unsafe {
        assert_eq!(
            rr_dr60_get_playback_speed_mode(h, &mut mode),
            RrDr60Status::Ok
        );
        assert_eq!(mode, 0);
        assert_eq!(
            rr_dr60_set_playback_speed_mode(h, 2),
            RrDr60Status::InvalidSpeedMode
        );
        assert_eq!(RrDr60Status::InvalidSpeedMode as u32, 38);
        assert_eq!(
            rr_dr60_get_playback_speed_mode(h, &mut mode),
            RrDr60Status::Ok
        );
        assert_eq!(mode, 0);
        assert_eq!(rr_dr60_set_playback_speed_mode(h, 1), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_get_playback_speed_mode(h, &mut mode),
            RrDr60Status::Ok
        );
        assert_eq!(mode, 1);
        let mut n = 0u64;
        assert_eq!(
            rr_dr60_playback_length_samples_in_mode(h, 1, 1, 3, &mut n),
            RrDr60Status::InvalidSpeedMode
        );
        assert_eq!(
            rr_dr60_playback_length_samples_in_mode(h, 1, 0, 1, &mut n),
            RrDr60Status::InvalidSpeedLevel
        );
        assert_eq!(
            rr_dr60_playback_length_samples_in_mode(h, 1, 1, 1, &mut n),
            RrDr60Status::NoSuchRecording
        );
        rr_dr60_destroy(h);
    }
}

/// AS6 (FR-008): the promise's script exists, names 0.22's merge and runs in CI (the run itself
/// is tasks.md T016's step).
#[test]
fn as6_promise_pointer() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let script = std::fs::read_to_string(format!("{root}/scripts/abi-promise.sh"))
        .expect("scripts/abi-promise.sh");
    assert!(script.contains("12a3c45"), "the default revision");
    assert!(
        script.contains("--abi") && script.contains("--bypass"),
        "every 0.22 mode"
    );
    let ci = std::fs::read_to_string(format!("{root}/.github/workflows/ci.yml")).unwrap();
    assert!(
        ci.contains("scripts/abi-promise.sh 12a3c45")
            || ci.contains(
                "scripts/abi-promise.sh 12a3c45"
                    .replace("12a3c45", "12a3c4560855a87990d0d47a48a49d99355cdcfd")
                    .as_str()
            ),
        "the CI step"
    );
}

/// AS7 (FR-005): the one wait rule for the speed pair in the four orderings, against the
/// reference (bit-exact) with the entries logged; a request past the latch is dropped and
/// logged.
#[test]
fn as7_wait_rule() {
    let x = pc::vowel(1.0);
    let n = 120usize;
    let s = 1200usize;
    let cases: [pc::Case; 4] = [
        (
            "level during a mode blend",
            1,
            SpeedMode::PitchWithSpeed,
            vec![
                (s, Req::Mode(SpeedMode::PitchKept)),
                (s + 10, Req::Level(5)),
            ],
        ),
        (
            "mode during a level crossfade (default mode)",
            1,
            SpeedMode::PitchWithSpeed,
            vec![
                (s, Req::Level(5)),
                (s + 10, Req::Mode(SpeedMode::PitchKept)),
            ],
        ),
        (
            "level landing while a mode blend runs",
            2,
            SpeedMode::PitchKept,
            vec![
                (s, Req::Mode(SpeedMode::PitchWithSpeed)),
                (s + n / 2, Req::Level(4)),
            ],
        ),
        (
            "mode request while an into-3 blend runs",
            5,
            SpeedMode::PitchKept,
            vec![
                (s, Req::Level(3)),
                (s + 20, Req::Mode(SpeedMode::PitchWithSpeed)),
            ],
        ),
    ];
    for (name, level, mode, sched) in &cases {
        let (mism, shifts_equal, a, b) =
            pc::against_reference(&standard, "speed_only", &x, *level, *mode, sched);
        assert_eq!(mism, 0, "{name}: mismatches");
        assert!(shifts_equal, "{name}: shifts ({a} vs {b})");
        // The entries: both requests logged in position order with their tags.
        let mut p = pc::engine(&standard, "speed_only", 6000, &x);
        p.set_playback_speed(*level).unwrap();
        p.set_playback_speed_mode(*mode);
        let changes: Vec<Change> = sched
            .iter()
            .enumerate()
            .map(|(i, &(at, r))| match r {
                Req::Level(l) => Change::speed(at as u64, l, i as u64 + 1),
                Req::Mode(m) => Change::mode(at as u64, m, i as u64 + 1),
            })
            .collect();
        let run = cc::run_scheduled(&mut p, Source::Playback(1), &changes, Partition::Fixed(1));
        let log: Vec<&ChangeEntry> = run.log.iter().filter(|e| e.tag > 0).collect();
        assert_eq!(log.len(), 2, "{name}: the log");
        assert_eq!((log[0].tag, log[1].tag), (1, 2));
        assert_eq!(
            (log[0].position, log[1].position),
            (changes[0].position, changes[1].position)
        );
    }
    // A request past the latch: dropped by the stage (the output equals the run without it)
    // and logged.
    let k = 6000u64;
    let own = (5 * k).div_ceil(3);
    let mut p = pc::engine(&standard, "speed_only", 6000, &x);
    p.set_playback_speed(1).unwrap();
    p.set_playback_speed_mode(SpeedMode::PitchKept);
    let with = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::mode(own + 30, SpeedMode::PitchWithSpeed, 9)],
        Partition::Fixed(64),
    );
    let mut q = pc::engine(&standard, "speed_only", 6000, &x);
    q.set_playback_speed(1).unwrap();
    q.set_playback_speed_mode(SpeedMode::PitchKept);
    let without = cc::run(&mut q, Source::Playback(1), Partition::Fixed(64));
    assert!(
        cc::same(&with.out, &without.out),
        "past the latch: the output"
    );
    let e = cc::entries_of(&with.log, MODE);
    assert_eq!(e.len(), 1);
    assert_eq!(
        (e[0].position, e[0].tag, e[0].before, e[0].after),
        (own + 30, 9, 1.0, 0.0)
    );
    assert_eq!(
        p.playback_speed_mode(),
        SpeedMode::PitchWithSpeed,
        "the reading follows the landing"
    );
}

/// Research R-02 (hardware B1): the published window's halves sum to one exactly at every n
/// and each entry is within 4 ULP of the formula in deterministic math.
#[test]
fn window_bits_published() {
    let w = pc::window();
    let f = pc::window_formula();
    assert_eq!(w.len(), 120);
    for n in 0..60 {
        assert_eq!((w[n] + w[60 + n]).to_bits(), 1.0f64.to_bits(), "n={n}");
        let ulps = (w[n].to_bits() as i64 - f[n].to_bits() as i64).abs();
        assert!(ulps <= 4, "n={n}: {ulps} ULP from the formula");
    }
    assert_eq!(w[0], 0.0);
    assert_eq!(w[60], 1.0);
}
