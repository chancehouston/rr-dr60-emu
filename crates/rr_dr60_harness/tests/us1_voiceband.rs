//! User Story 1 acceptance tests (spec.md US1 AS1–AS5; tasks.md T027, T028).
//!
//! These use only the Foundational harness pieces (stimulus, analysis), so US1 is testable
//! without the US3 measurement matrix (Constitution VII).

use rr_dr60::{Error, Pipeline, SUPPORTED_HOST_RATES, Settings};
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60Settings, RrDr60Status, rr_dr60_create, rr_dr60_destroy,
    rr_dr60_latency_samples, rr_dr60_process, rr_dr60_reset, rr_dr60_settings_default,
};
use rr_dr60_harness::{analysis, configs, stimulus};

fn run(host: u32, input: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(configs::settings("default", host)).expect("supported rate");
    let mut out = vec![0.0; input.len()];
    let produced = p.process(input, &mut out).expect("equal lengths").produced;
    out.truncate(produced);
    out
}

#[test]
fn as1_1k_tone_keeps_its_level() {
    // AS1: -20 dBFS 1 kHz in, -20 dBFS ± 0.2 dB out (A-015).
    let fs = 48_000;
    let x = stimulus::tone(1000.0, 0.1, f64::from(fs), fs as usize);
    let y = run(fs, &x);
    let (gain_db, _) = analysis::gain_db_and_phase(&x, &y, 1000.0, f64::from(fs), fs as usize / 5);
    assert!(gain_db.abs() <= 0.2, "1 kHz gain {gain_db:.4} dB");
}

#[test]
fn as2_out_of_band_tones_are_attenuated() {
    // AS2: 50 Hz and 3800 Hz at least 20 dB below the input (A-002, A-014).
    let fs = 44_100;
    for f in [50.0, 3800.0] {
        let x = stimulus::tone(f, 0.1, f64::from(fs), fs as usize);
        let y = run(fs, &x);
        let r = analysis::power_ratio_db(&x, &y, fs as usize / 5);
        assert!(r <= -20.0, "{f} Hz passes at {r:.1} dB");
    }
}

#[test]
fn as3_n_samples_in_n_samples_out_at_every_rate() {
    for &host in &SUPPORTED_HOST_RATES {
        let mut p = Pipeline::new(configs::settings("default", host)).unwrap();
        for n in [0usize, 1, 7, 64, 4096, 48_000] {
            let x = stimulus::noise(0x0D60, n);
            let mut y = vec![f32::NAN; n];
            let produced = p.process(&x, &mut y).unwrap().produced;
            y.truncate(produced);
            assert_eq!(y.len(), n);
            assert!(y.iter().all(|v| v.is_finite()), "{host} Hz, block {n}");
        }
    }
}

#[test]
fn as3_length_mismatch_is_rejected_without_changing_state() {
    let x = stimulus::tone(1000.0, 0.5, 48_000.0, 4800);
    let mut p = Pipeline::new(configs::settings("default", 48_000)).unwrap();
    let mut short = vec![0.0; 10];
    assert_eq!(
        p.process(&x, &mut short),
        Err(Error::LengthMismatch {
            input: 4800,
            output: 10
        })
    );
    let mut y = vec![0.0; x.len()];
    let produced = p.process(&x, &mut y).unwrap().produced;
    y.truncate(produced);
    assert_eq!(y, run(48_000, &x), "state changed by a rejected call");
}

#[test]
fn as4_silence_in_gives_exact_silence_out() {
    for &host in &SUPPORTED_HOST_RATES {
        let y = run(host, &stimulus::silence(2 * host as usize));
        assert!(
            y.iter().all(|&v| v.to_bits() == 0),
            "{host} Hz: non-zero output from silence"
        );
    }
}

#[test]
fn unsupported_rate_is_rejected() {
    assert_eq!(
        Pipeline::new(Settings::new(22_050)).err(),
        Some(Error::UnsupportedHostRate { requested: 22_050 })
    );
}

#[test]
fn as5_c_api_matches_rust_api() {
    let fs = 48_000;
    let x = stimulus::tone(1000.0, 0.1, f64::from(fs), fs as usize);
    let want = run(fs, &x);
    let rust_latency = Pipeline::new(configs::settings("default", fs))
        .unwrap()
        .latency_samples();

    let settings: RrDr60Settings = configs::c_default_001(fs);
    let mut p: *mut RrDr60Pipeline = std::ptr::null_mut();
    // SAFETY: valid pointers to a settings struct and an out-pointer.
    assert_eq!(
        unsafe { rr_dr60_create(&settings, &mut p) },
        RrDr60Status::Ok
    );
    assert!(!p.is_null());

    // Copy mode.
    let mut y = vec![0.0f32; x.len()];
    // SAFETY: p is live; x and y are valid, non-overlapping, x.len() long.
    assert_eq!(
        unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), x.len(), std::ptr::null_mut()) },
        RrDr60Status::Ok
    );
    assert_eq!(y, want, "C API output differs from Pipeline::process");

    // In-place mode, after reset.
    // SAFETY: p is live.
    assert_eq!(unsafe { rr_dr60_reset(p) }, RrDr60Status::Ok);
    let mut buf = x.clone();
    // SAFETY: input == output is the documented in-place mode.
    assert_eq!(
        unsafe {
            rr_dr60_process(
                p,
                buf.as_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                std::ptr::null_mut(),
            )
        },
        RrDr60Status::Ok
    );
    assert_eq!(buf, want, "in-place output differs");

    let mut latency = 0u32;
    // SAFETY: p is live; latency is a valid out-pointer.
    assert_eq!(
        unsafe { rr_dr60_latency_samples(p, &mut latency) },
        RrDr60Status::Ok
    );
    assert_eq!(latency, rust_latency);
    // SAFETY: p came from rr_dr60_create and is not used afterwards.
    unsafe { rr_dr60_destroy(p) };

    // Errors are values (FR-024).
    let bad = rr_dr60_settings_default(22_050);
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    let mut out = sentinel;
    // SAFETY: valid pointers.
    assert_eq!(
        unsafe { rr_dr60_create(&bad, &mut out) },
        RrDr60Status::UnsupportedHostRate
    );
    assert_eq!(out, sentinel, "*out must be untouched on error");
    // SAFETY: NULL settings must be rejected, not dereferenced.
    assert_eq!(
        unsafe { rr_dr60_create(std::ptr::null(), &mut out) },
        RrDr60Status::NullPointer
    );
    assert_eq!(out, sentinel);
}
