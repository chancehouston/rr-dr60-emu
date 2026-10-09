//! C API parity and error contract (FR-023, FR-024; contracts/c-api.md; tasks.md T054).

use rr_dr60::Pipeline;
use rr_dr60_ffi::*;
use rr_dr60_harness::{configs, golden, stimulus};

fn create(s: &RrDr60Settings) -> *mut RrDr60Pipeline {
    let mut p = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(s, &mut p) }, RrDr60Status::Ok);
    p
}

#[test]
fn c_api_matches_rust_api_everywhere() {
    for rate in configs::all_rates() {
        let x = golden::stimulus("sweep_log", rate);
        for config in configs::CONFIGS {
            let s = configs::settings(config, rate);
            let mut want = vec![0.0; x.len()];
            let mut rust = Pipeline::new(s).unwrap();
            rust.process(&x, &mut want).unwrap();
            let p = create(&configs::c_settings(&s));
            let mut got = vec![0.0f32; x.len()];
            // SAFETY: live handle; valid non-overlapping buffers.
            assert_eq!(
                unsafe { rr_dr60_process(p, x.as_ptr(), got.as_mut_ptr(), x.len()) },
                RrDr60Status::Ok
            );
            assert_eq!(got, want, "{rate} Hz {config}");
            let mut lat = 0;
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_latency_samples(p, &mut lat) },
                RrDr60Status::Ok
            );
            assert_eq!(lat, rust.latency_samples());
            // SAFETY: from rr_dr60_create.
            unsafe { rr_dr60_destroy(p) };
        }
    }
}

/// Spec 002 FR-014, US2 AS6: C and Rust output are bit-identical with the AGC on, for the
/// AGC configurations and for the minimum and maximum of every AGC setting.
#[test]
fn c_api_matches_rust_api_with_agc() {
    let rate = 48_000;
    let x = stimulus::step(1000.0, &[-40.0, -10.0, -40.0], &[0.1, 0.1, 0.2], 48_000.0);
    let mut cases: Vec<rr_dr60::Settings> = configs::AGC_CONFIGS
        .iter()
        .map(|c| configs::settings(c, rate))
        .collect();
    let extremes: [fn(&mut rr_dr60::AgcSettings, bool); 5] = [
        |a, hi| a.target_dbfs = if hi { 0.0 } else { -30.0 },
        |a, hi| a.max_gain_db = if hi { 60.0 } else { 0.0 },
        |a, hi| a.max_attenuation_db = if hi { 40.0 } else { 0.0 },
        |a, hi| a.attack_ms = if hi { 100.0 } else { 1.0 },
        |a, hi| a.release_ms = if hi { 10_000.0 } else { 50.0 },
    ];
    for set in extremes {
        for hi in [false, true] {
            let mut s = configs::settings("default_agc", rate);
            set(&mut s.agc, hi);
            cases.push(s);
        }
    }
    for s in cases {
        let mut want = vec![0.0; x.len()];
        Pipeline::new(s).unwrap().process(&x, &mut want).unwrap();
        let p = create(&configs::c_settings(&s));
        let mut got = vec![0.0f32; x.len()];
        // SAFETY: live handle; valid non-overlapping buffers of x.len() floats.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), got.as_mut_ptr(), x.len()) },
            RrDr60Status::Ok
        );
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
        assert_eq!(got, want, "{:?}", s.agc);
    }
}

/// Validates through the C API and returns (status, field).
fn validate(s: &RrDr60Settings) -> (RrDr60Status, RrDr60SettingField) {
    let mut field = RrDr60SettingField::AgcReleaseMs; // overwritten on every call
    // SAFETY: valid pointers.
    let status = unsafe { rr_dr60_settings_validate(s, &mut field) };
    (status, field)
}

/// Spec 002 US2 AS5, FR-014 (contracts/c-api.md): invalid AGC settings are rejected with
/// INVALID_SETTING, and `rr_dr60_settings_validate` names the field.
#[test]
fn c_api_names_invalid_agc_settings() {
    let cases: [(RrDr60SettingField, fn(&mut RrDr60Settings)); 5] = [
        (RrDr60SettingField::AgcTargetDbfs, |s| s.agc_target_dbfs = 1.0),
        (RrDr60SettingField::AgcMaxGainDb, |s| s.agc_max_gain_db = f32::NAN),
        (RrDr60SettingField::AgcMaxAttenuationDb, |s| s.agc_max_attenuation_db = 41.0),
        (RrDr60SettingField::AgcAttackMs, |s| s.agc_attack_ms = 0.0),
        (RrDr60SettingField::AgcReleaseMs, |s| s.agc_release_ms = f32::INFINITY),
    ];
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    for (field, break_it) in cases {
        let mut s = rr_dr60_settings_default(48_000);
        break_it(&mut s);
        assert_eq!(validate(&s), (RrDr60Status::InvalidSetting, field));
        let mut out = sentinel;
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, RrDr60Status::InvalidSetting);
        assert_eq!(out, sentinel, "*out touched on error");
    }
    assert_eq!(
        validate(&rr_dr60_settings_default(48_000)),
        (RrDr60Status::Ok, RrDr60SettingField::None)
    );
}

/// Spec 002 contracts/c-api.md: `validate` returns exactly what `create` returns, in the
/// documented order (struct size → tap → host rate → AGC fields), and handles NULL.
#[test]
fn c_api_validate_agrees_with_create() {
    let base = rr_dr60_settings_default(48_000);
    let old_layout = RrDr60Settings { struct_size: 24, ..base };
    let bad_size_and_tap = RrDr60Settings { struct_size: 24, tap: 9, ..base };
    let bad_tap_and_rate = RrDr60Settings { tap: 9, host_rate_hz: 22_050, ..base };
    let bad_rate_and_attack = RrDr60Settings { host_rate_hz: 22_050, agc_attack_ms: 0.0, ..base };
    let bad_target_and_release =
        RrDr60Settings { agc_target_dbfs: 5.0, agc_release_ms: 1.0, ..base };
    let expected = [
        (old_layout, RrDr60Status::InvalidArgument, RrDr60SettingField::StructSize),
        (bad_size_and_tap, RrDr60Status::InvalidArgument, RrDr60SettingField::StructSize),
        (bad_tap_and_rate, RrDr60Status::InvalidArgument, RrDr60SettingField::Tap),
        (bad_rate_and_attack, RrDr60Status::UnsupportedHostRate, RrDr60SettingField::HostRate),
        (bad_target_and_release, RrDr60Status::InvalidSetting, RrDr60SettingField::AgcTargetDbfs),
    ];
    for (s, status, field) in expected {
        assert_eq!(validate(&s), (status, field), "{s:?}");
        let mut out = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, status, "{s:?}");
    }
    // SAFETY: NULL settings is reported, never dereferenced; a NULL out_field is allowed.
    unsafe {
        let mut field = RrDr60SettingField::Tap;
        assert_eq!(
            rr_dr60_settings_validate(std::ptr::null(), &mut field),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_settings_validate(&base, std::ptr::null_mut()),
            RrDr60Status::Ok
        );
    }
}

/// Spec 002 FR-014: an invalid AGC setting in `rr_dr60_reconfigure` leaves the handle unchanged.
#[test]
fn c_api_reconfigure_rejects_invalid_agc_and_keeps_state() {
    let x = stimulus::step(1000.0, &[-30.0], &[0.2], 48_000.0);
    let good = rr_dr60_settings_default(48_000);
    let (p, q) = (create(&good), create(&good));
    let mut scratch = vec![0.0f32; x.len()];
    let mut bad = good;
    bad.agc_attack_ms = 0.0;
    // SAFETY: live handles; valid buffers and settings.
    unsafe {
        assert_eq!(rr_dr60_process(p, x.as_ptr(), scratch.as_mut_ptr(), x.len()), RrDr60Status::Ok);
        assert_eq!(rr_dr60_process(q, x.as_ptr(), scratch.as_mut_ptr(), x.len()), RrDr60Status::Ok);
        assert_eq!(rr_dr60_reconfigure(p, &bad), RrDr60Status::InvalidSetting);
        let (mut a, mut b) = (vec![0.0f32; x.len()], vec![0.0f32; x.len()]);
        assert_eq!(rr_dr60_process(p, x.as_ptr(), a.as_mut_ptr(), x.len()), RrDr60Status::Ok);
        assert_eq!(rr_dr60_process(q, x.as_ptr(), b.as_mut_ptr(), x.len()), RrDr60Status::Ok);
        assert_eq!(a, b, "failed reconfigure changed the handle");
        rr_dr60_destroy(p);
        rr_dr60_destroy(q);
    }
}

#[test]
fn error_table() {
    let ok = rr_dr60_settings_default(48_000);
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    let mut out = sentinel;
    let status = |s: RrDr60Settings, out: &mut *mut RrDr60Pipeline| {
        // SAFETY: valid pointers.
        unsafe { rr_dr60_create(&s, out) }
    };
    // SAFETY: NULL settings / NULL out-pointer must be rejected without dereferencing.
    assert_eq!(
        unsafe { rr_dr60_create(std::ptr::null(), &mut out) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        unsafe { rr_dr60_create(&ok, std::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        status(rr_dr60_settings_default(22_050), &mut out),
        RrDr60Status::UnsupportedHostRate
    );
    assert_eq!(
        status(RrDr60Settings { tap: 7, ..ok }, &mut out),
        RrDr60Status::InvalidArgument
    );
    assert_eq!(
        status(
            RrDr60Settings {
                struct_size: 4,
                ..ok
            },
            &mut out
        ),
        RrDr60Status::InvalidArgument
    );
    assert_eq!(out, sentinel, "*out touched on error");

    let p = create(&ok);
    let x = vec![0.25f32; 256];
    let mut y = vec![0.0f32; 256];
    // SAFETY (all below): p is live; buffers are valid for their stated lengths.
    unsafe {
        assert_eq!(
            rr_dr60_process(std::ptr::null_mut(), x.as_ptr(), y.as_mut_ptr(), 64),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(p, std::ptr::null(), y.as_mut_ptr(), 64),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), std::ptr::null_mut(), 64),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(p, std::ptr::null(), std::ptr::null_mut(), 0),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(p, y.as_ptr(), y.as_mut_ptr().add(1), 64),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_process(p, y.as_ptr().add(1), y.as_mut_ptr(), 64),
            RrDr60Status::InvalidArgument
        );
        let mut lat = 7u32;
        assert_eq!(
            rr_dr60_latency_samples(std::ptr::null(), &mut lat),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_latency_samples(p, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(lat, 7);
        assert_eq!(
            rr_dr60_reset(std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        let unsupported = rr_dr60_settings_default(22_050);
        assert_eq!(
            rr_dr60_reconfigure(p, &unsupported),
            RrDr60Status::UnsupportedHostRate
        );
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 256),
            RrDr60Status::Ok,
            "old config must still work"
        );
        rr_dr60_destroy(std::ptr::null_mut());
        rr_dr60_destroy(p);
    }
}

#[test]
fn version_string_matches_crate() {
    // SAFETY: static NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(rr_dr60_version_string()) };
    assert_eq!(v.to_str().unwrap(), rr_dr60::VERSION);
}

#[cfg(feature = "ffi-test-panic")]
mod panics {
    use super::*;

    fn poison(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        unsafe {
            rr_dr60__test_force_panic(p);
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32),
                RrDr60Status::InternalError
            );
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32),
                RrDr60Status::InternalError,
                "not poisoned"
            );
            let mut lat = 0;
            assert_eq!(
                rr_dr60_latency_samples(p, &mut lat),
                RrDr60Status::InternalError
            );
        }
    }

    fn works(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32) },
            RrDr60Status::Ok
        );
    }

    #[test]
    fn panic_poisons_and_reset_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        // SAFETY: live handle.
        assert_eq!(unsafe { rr_dr60_reset(p) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }

    #[test]
    fn panic_poisons_and_reconfigure_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        let s = rr_dr60_settings_default(16_000);
        // SAFETY: live handle; valid settings.
        assert_eq!(unsafe { rr_dr60_reconfigure(p, &s) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }
}
