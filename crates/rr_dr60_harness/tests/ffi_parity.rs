//! C API parity and error contract (FR-023, FR-024; contracts/c-api.md; tasks.md T054).

use rr_dr60::{Pipeline, Tap};
use rr_dr60_ffi::*;
use rr_dr60_harness::{configs, golden};

fn c_settings(s: rr_dr60::Settings) -> RrDr60Settings {
    RrDr60Settings {
        record_stage_enabled: s.record_stage_enabled,
        playback_stage_enabled: s.playback_stage_enabled,
        tap: if s.tap == Tap::AfterRecord {
            RrDr60Tap::AfterRecord as u32
        } else {
            RrDr60Tap::AfterPlayback as u32
        },
        seed: s.seed,
        ..rr_dr60_settings_default(s.host_rate_hz)
    }
}

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
            let p = create(&c_settings(s));
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
