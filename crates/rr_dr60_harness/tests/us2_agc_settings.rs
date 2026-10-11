//! Spec 005 User Story 3: tune, bypass or tap the limiter, from either interface (tasks.md
//! T030; spec › US3 AS1–AS5). Replaces the spec 002 US2 tests (005 FR-013).
//!
//! AS1 and AS2 run the FR-011 settings-matrix checks for one case each; AS3 proves the "after
//! AGC" tap is the limiter's own output at the host rate; AS4 is C parity for the matrix at
//! seven rates; AS5 is the spec 001 golden file with the limiter bypassed.

use rr_dr60::{Error, Pipeline, SUPPORTED_HOST_RATES, Setting, Settings, Tap};
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60SettingField, RrDr60Settings, RrDr60Status, rr_dr60_create,
    rr_dr60_destroy, rr_dr60_latency_samples, rr_dr60_process, rr_dr60_settings_validate,
};
use rr_dr60_harness::agc_checks;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::{configs, golden, report, stimulus};
use sha2::Digest;

/// An AGC field and a way to put it out of range.
type BreakCase = (Setting, fn(&mut Settings));

/// The reference model's figures at 48 kHz for the `agc_onset` stimulus at the default knee
/// (005 research R-04, the script's "Golden `agc_onset` stimulus" section, run 2026-10-11 at
/// knee −1 dBFS): the dead time of the +11 dBFS step (12 dB over the knee; shorter than the
/// 10 dB-over figure because the overdrive is larger) and the −1 dB release after its 0.4 s
/// (12 dB of attenuation, so longer than the 10 dB figure).
const DEAD_MS: f64 = 2.417; // 005 R-04
const REL_S: f64 = 1.475; // 005 R-04

fn process(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = Pipeline::new(settings)
        .unwrap()
        .process(x, &mut y)
        .unwrap()
        .produced;
    y.truncate(produced);
    y
}

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn case(label: &str) -> agc_checks::Case {
    agc_checks::matrix()
        .into_iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no matrix case {label:?}"))
}

/// US3 AS1 (005 FR-011): with the knee at −10 dBFS the measured knee lies at −10 + 1 ± 2 dBFS
/// and the ceiling follows it, at 6 and 48 kHz.
#[test]
fn as1_knee_m10_moves_the_knee_and_ceiling() {
    let c = case("knee -10 dBFS");
    let mut r = Vec::new();
    for rate in [6000, 48_000] {
        r.extend(
            agc_checks::check_fr011_settings(rate, &c, &standard)
                .into_iter()
                .filter(|row| row.requirement == "005/FR-011" && row.property.contains("knee")),
        );
    }
    assert!(r.iter().any(|row| row.property.contains("bisection")));
    assert!(r.iter().any(|row| row.property.contains("ceiling")));
    finish(r);
}

/// US3 AS2 (005 FR-011): with a 4 s release the gain is back within 1 dB of unity 4 s ± 40 %
/// after the burst, and the −6 / −3 dB points scale with it.
#[test]
fn as2_release_4s() {
    let c = case("release 4000 ms");
    let mut r = Vec::new();
    for rate in [6000, 48_000] {
        r.extend(
            agc_checks::check_fr011_settings(rate, &c, &standard)
                .into_iter()
                .filter(|row| row.property.contains("release") || row.property.contains("gain ")),
        );
    }
    assert!(r.iter().any(|row| row.property == "release time to -1 dB"));
    finish(r);
}

/// US3 AS3 (005 FR-001): at 48 kHz through `agc_only` the output is the limiter's own output
/// at the host rate, sample for sample: latency 0; every block produces as many samples as it
/// consumed with no events; a −20 dBFS tone is returned bit-exact; the `agc_onset` stimulus's
/// gain trace at the tap gives the pinned dead time (3.17 ms ± 1 sample, research R-04) and
/// the −1 dB release at 1.362 s ± 1 % (the T003 figures); and the output is bit-identical for
/// block sizes 1, 64 and 4096.
#[test]
fn as3_tap_after_agc_is_the_limiter_output_latency_0() {
    let rate = 48_000;
    let fs = f64::from(rate);
    let s = configs::settings("agc_only", rate);
    assert_eq!(s.tap, Tap::AfterAgc);
    let mut p = Pipeline::new(s).unwrap();
    assert_eq!(p.latency_samples(), 0); // 005 FR-001

    // Every block: produced == n, events == 0, through the loud onset included.
    let x = golden::stimulus("agc_onset", rate);
    for block in [1usize, 64, 4096] {
        let mut p = Pipeline::new(s).unwrap();
        assert_eq!(p.max_events(block), 0);
        for chunk in x.chunks(block) {
            let mut y = vec![0.0; chunk.len()];
            let info = p.process(chunk, &mut y).unwrap();
            assert_eq!(
                (info.produced, info.events, info.paused),
                (chunk.len(), 0, false)
            );
        }
    }

    // A −20 dBFS tone returned bit-exact (FR-003 at the tap).
    let quiet = stimulus::tone_at_phase(
        1000.0,
        -20.0,
        stimulus::peak_phase(1000.0, rate),
        fs,
        rate as usize,
    );
    let mut y = vec![0.0; quiet.len()];
    let info = p.process(&quiet, &mut y).unwrap();
    assert_eq!(info.produced, quiet.len());
    assert_eq!(y, quiet, "the tap must return sub-knee input bit for bit");

    // The onset stimulus's exact gain trace at the tap: dead time and release pinned to the
    // reference model's figures (005 research R-04 at the −1 dBFS knee: 3.19 ms at 48 kHz for
    // the +11 dBFS step, 12 dB over; 1.361 s to −1 dB).
    let g = agc_checks::gain_trace(&standard, rate, "agc_only", &x, stimulus::amplitude(-20.0));
    let steady = agc_checks::steady_db(&g, fs, 0.45, 0.5);
    let dead_ms = agc_checks::dead_time_s(&g, fs, 0.1, steady).unwrap() * 1e3;
    let release = agc_checks::release_s(&g, fs, 0.5, -1.0).unwrap();
    eprintln!(
        "US3 AS3: dead time {dead_ms:.3} ms (want DEAD_MS ± 1 sample), release {release:.4} s (want REL_S ± 1 %)"
    );
    let one_sample_ms = 1e3 / fs;
    assert!(
        (dead_ms - DEAD_MS).abs() <= one_sample_ms + 1e-9,
        "dead time {dead_ms} ms"
    ); // R-04
    assert!(
        (release - REL_S).abs() <= 0.01 * REL_S,
        "release {release} s"
    ); // R-04

    // Bit-identical for block sizes 1, 64 and 4096 (FR-014 at the tap).
    let whole = process(s, &x);
    for block in [1usize, 64, 4096] {
        let mut p = Pipeline::new(s).unwrap();
        let mut out = Vec::with_capacity(x.len());
        for chunk in x.chunks(block) {
            let mut y = vec![0.0; chunk.len()];
            let produced = p.process(chunk, &mut y).unwrap().produced;
            out.extend_from_slice(&y[..produced]);
        }
        assert!(
            out.iter()
                .zip(&whole)
                .all(|(a, b)| a.to_bits() == b.to_bits())
                && out.len() == whole.len(),
            "block size {block} changed the tap output"
        );
    }
}

fn c_process(cs: &RrDr60Settings, x: &[f32]) -> (Vec<f32>, u32) {
    let mut p: *mut RrDr60Pipeline = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(cs, &mut p) }, RrDr60Status::Ok);
    let mut y = vec![0.0f32; x.len()];
    let mut lat = u32::MAX;
    // SAFETY: live handle; valid non-overlapping buffers of x.len() floats; valid out-pointer.
    unsafe {
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(rr_dr60_latency_samples(p, &mut lat), RrDr60Status::Ok);
        rr_dr60_destroy(p);
    }
    (y, lat)
}

/// US3 AS4 (005 FR-015): at all seven host rates, every settings-matrix case through
/// `agc_only` and `default_agc` gives `memcmp`-equal output and the same latency through the C
/// API; an out-of-range knee is rejected naming `AgcKneeDbfs` in both interfaces.
#[test]
fn as4_c_parity_and_rejection() {
    for rate in SUPPORTED_HOST_RATES {
        let x = golden::stimulus("agc_staircase", rate);
        for config in configs::AGC_CONFIGS {
            for c in agc_checks::matrix() {
                let mut s = configs::settings(config, rate);
                s.agc = c.agc;
                let mut rust = Pipeline::new(s).unwrap();
                let mut want = vec![0.0; x.len()];
                let produced = rust.process(&x, &mut want).unwrap().produced;
                want.truncate(produced);
                let (got, lat) = c_process(&configs::c_settings(&s), &x);
                let got = &got[..produced];
                assert!(
                    got.len() == want.len()
                        && got
                            .iter()
                            .zip(&want)
                            .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "{rate} Hz {config} {}: C output differs",
                    c.label
                );
                assert_eq!(
                    lat,
                    rust.latency_samples(),
                    "{rate} Hz {config} {}",
                    c.label
                );
            }
        }
    }
    // Rejection, named, in both interfaces (005 FR-010, FR-015).
    let mut bad = Settings::new(48_000);
    bad.agc.knee_dbfs = 4.0; // above the +3 dBFS cap (005 FR-010)
    assert_eq!(
        Pipeline::new(bad).err(),
        Some(Error::InvalidSetting {
            setting: Setting::AgcKneeDbfs
        })
    );
    let cbad = configs::c_settings(&bad);
    assert_eq!(cbad.agc_knee_dbfs, 4.0);
    let mut field = RrDr60SettingField::None;
    // SAFETY: valid pointers.
    let status = unsafe { rr_dr60_settings_validate(&cbad, &mut field) };
    assert_eq!(
        (status, field),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::AgcKneeDbfs
        )
    );
}

/// US3 AS5 (005 FR-016): with the limiter bypassed every spec 001 golden entry is reproduced
/// bit for bit, and the committed file's hash equals the SHA guard that `golden_agc.rs` pins.
#[test]
fn as5_golden_v1_unchanged_with_agc_bypassed() {
    let actual = golden::generate(&|s| {
        assert!(
            !s.agc.enabled,
            "spec 001 configurations have the limiter bypassed"
        );
        Pipeline::new(s).unwrap()
    });
    assert_eq!(actual.entries.len(), 112);
    golden::compare(&golden::committed(), &actual).unwrap();
    // The SHA guard constant in tests/golden_agc.rs is the committed file's hash (004 re-bless).
    let bytes = std::fs::read(golden::path()).unwrap();
    let hex: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let guard = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_agc.rs"),
    )
    .unwrap();
    assert!(
        guard.contains(&hex),
        "GOLDEN_V1_SHA256 in tests/golden_agc.rs is not the committed golden-v1.json's hash {hex}"
    );
}

/// Spec 002 US2 AS5, kept by 005 FR-010: an out-of-range value of any limiter field is
/// rejected by `new` and by `reconfigure`, naming the field; a failed reconfigure leaves the
/// pipeline unchanged.
#[test]
fn invalid_settings_are_named_and_rejected() {
    let cases: [BreakCase; 3] = [
        (Setting::AgcKneeDbfs, |s| s.agc.knee_dbfs = 4.0),
        (Setting::AgcAttackMs, |s| s.agc.attack_ms = 0.0),
        (Setting::AgcReleaseMs, |s| s.agc.release_ms = 10_001.0),
    ];
    let x = stimulus::step(1000.0, &[-30.0], &[0.2], 48_000.0);
    for (setting, break_it) in cases {
        let mut bad = Settings::new(48_000);
        break_it(&mut bad);
        let want = Error::InvalidSetting { setting };
        assert_eq!(Pipeline::new(bad).err(), Some(want));
        assert!(want.to_string().contains("agc."), "{want}");

        let mut p = Pipeline::new(configs::settings("default_agc", 48_000)).unwrap();
        let mut warm = x.clone();
        let produced = p.process_in_place(&mut warm).produced;
        warm.truncate(produced);
        let mut control = p.clone();
        assert_eq!(p.reconfigure(bad), Err(want));
        assert_eq!(p.settings(), control.settings());
        let (mut a, mut b) = (x.clone(), x.clone());
        let produced = p.process_in_place(&mut a).produced;
        a.truncate(produced);
        let produced = control.process_in_place(&mut b).produced;
        b.truncate(produced);
        assert_eq!(a, b, "{setting:?}: failed reconfigure changed the pipeline");
    }
}
