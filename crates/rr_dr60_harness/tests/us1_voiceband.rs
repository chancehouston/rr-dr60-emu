//! Spec 001 User Story 1 acceptance tests, restated at the 6 kHz device rate by spec 004
//! FR-010 (tasks.md T017), plus spec 004 US1 AS5 (the default-pipeline sweep).
//!
//! These use only the Foundational harness pieces (stimulus, analysis), so US1 is testable
//! without the measurement matrix (Constitution VII). The "default" configuration here is
//! spec 001's: both band-limit stages on, AGC and VAS bypassed (002 FR-018, 003 FR-020).

use rr_dr60::{Error, Pipeline, SUPPORTED_HOST_RATES};
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60Settings, RrDr60Status, rr_dr60_create, rr_dr60_destroy,
    rr_dr60_latency_samples, rr_dr60_process, rr_dr60_reset, rr_dr60_settings_default,
};
use rr_dr60_harness::analysis::{self, Spectrum};
use rr_dr60_harness::{configs, stimulus};

fn run_config(name: &str, host: u32, input: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(configs::settings(name, host)).expect("supported rate");
    let mut out = vec![0.0; input.len()];
    let produced = p.process(input, &mut out).expect("equal lengths").produced;
    out.truncate(produced);
    out
}

fn run(host: u32, input: &[f32]) -> Vec<f32> {
    run_config("default", host, input)
}

/// Gain in dB of a configuration for a 1 s tone at `f`, first 0.2 s discarded.
fn gain_db(name: &str, host: u32, f: f64) -> f64 {
    let x = stimulus::tone(f, 0.1, f64::from(host), host as usize);
    let y = run_config(name, host, &x);
    analysis::gain_db_and_phase(&x, &y, f, f64::from(host), host as usize / 5).0
}

#[test]
fn as1_1k_tone_keeps_its_level() {
    // 001 AS1: -20 dBFS 1 kHz in, -20 dBFS ± 0.2 dB out (A-015; 004 FR-009's cascade
    // tolerance). The stages' 1 kHz loss is about 0.01 dB (spec 004 research R-04, R-05).
    let g = gain_db("default", 48_000, 1000.0);
    assert!(g.abs() <= 0.2, "1 kHz gain {g:.4} dB");
}

#[test]
fn as2_out_of_band_tones_are_attenuated() {
    // 001 AS2 restated: 45 Hz through the record stage is at least 20 dB down (004 FR-005,
    // S-005 60 Hz row scaled, A-027); 3450 and 4000 Hz through the default pipeline are at
    // least 30 dB down (a US1 demo of 004 FR-007; the formal check is the harness's anti-alias
    // check, T021).
    let fs = 44_100;
    let g45 = gain_db("record_only", fs, 45.0);
    assert!(g45 <= -20.0, "45 Hz passes the record stage at {g45:.1} dB");
    for f in [3450.0, 4000.0] {
        let x = stimulus::tone(f, 0.1, f64::from(fs), fs as usize);
        let y = run(fs, &x);
        let r = analysis::power_ratio_db(&x, &y, fs as usize / 5);
        assert!(r <= -30.0, "{f} Hz passes at {r:.1} dB");
    }
}

#[test]
fn as3_n_samples_in_n_samples_out_at_every_rate() {
    // 001 FR-003 at all seven host rates (004 FR-002), VAS bypassed.
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
    // 004 FR-002: 7 kHz is rejected and the error names all seven supported rates.
    let Err(err) = Pipeline::new(rr_dr60_harness::configs::settings_007(7000)) else {
        panic!("7 kHz must be rejected");
    };
    assert_eq!(err, Error::UnsupportedHostRate { requested: 7000 });
    let text = err.to_string();
    for rate in SUPPORTED_HOST_RATES {
        assert!(text.contains(&rate.to_string()), "{text} is missing {rate}");
    }
}

/// Spec 004 US1 AS5: a 20 Hz – 20 kHz sweep through the default pipeline at 48 kHz has no
/// output above 3 kHz within 60 dB of its in-band level (the device rate's Nyquist, A-026),
/// and the combined response's −1 dB and −3 dB points lie between 2.4 and 3.0 kHz
/// (engineering target; the modeled top edge, A-027).
#[test]
fn as5_default_sweep_shows_the_device_band() {
    let fs = 48_000u32;
    let n = 2 * fs as usize;
    let x = stimulus::log_sweep(20.0, 20_000.0, 0.25, f64::from(fs), n);
    let y = run(fs, &x);
    // Hann-windowed spectrum of the output: in-band peak vs everything above 3 kHz.
    let windowed: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(k, &v)| {
            let w = 0.5 - 0.5 * rr_dr60_detmath::cos(std::f64::consts::TAU * k as f64 / n as f64);
            f64::from(v) * w
        })
        .collect();
    let sp = Spectrum::from_impulse_response(&windowed, f64::from(fs), 1 << 18);
    let peak = |lo: f64, hi: f64| {
        sp.range(lo, hi)
            .map(|k| sp.bins[k].norm())
            .fold(0.0, f64::max)
    };
    let in_band = peak(300.0, 2000.0);
    let above = peak(3000.0, 0.49 * f64::from(fs));
    let gap = analysis::db(above / in_band);
    assert!(
        gap <= -60.0,
        "content above 3 kHz is only {gap:.1} dB below the band"
    );

    // −1 dB and −3 dB points of the combined response (both stages), relative to 765 Hz.
    let g_ref = gain_db("default", fs, 765.0);
    let mut f_1db = None;
    let mut f_3db = None;
    let mut f = 2200.0;
    while f <= 3000.0 {
        let loss = g_ref - gain_db("default", fs, f);
        if f_1db.is_none() && loss >= 1.0 {
            f_1db = Some(f);
        }
        if f_3db.is_none() && loss >= 3.0 {
            f_3db = Some(f);
        }
        f += 50.0;
    }
    let (f1, f3) = (
        f_1db.expect("no -1 dB point"),
        f_3db.expect("no -3 dB point"),
    );
    assert!((2400.0..=3000.0).contains(&f1), "-1 dB point at {f1} Hz");
    assert!((2400.0..=3000.0).contains(&f3), "-3 dB point at {f3} Hz");
    eprintln!(
        "default pipeline at 48 kHz: -1 dB at {f1} Hz, -3 dB at {f3} Hz, above 3 kHz {gap:.1} dB"
    );
}

#[test]
fn as5_c_api_matches_rust_api() {
    // 001 AS5 at the identity rate and at 48 kHz (004 FR-017).
    for fs in [6000u32, 48_000] {
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
            unsafe {
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), x.len(), std::ptr::null_mut())
            },
            RrDr60Status::Ok
        );
        assert_eq!(
            y, want,
            "{fs} Hz: C API output differs from Pipeline::process"
        );

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
        assert_eq!(buf, want, "{fs} Hz: in-place output differs");

        let mut latency = 0u32;
        // SAFETY: p is live; latency is a valid out-pointer.
        assert_eq!(
            unsafe { rr_dr60_latency_samples(p, &mut latency) },
            RrDr60Status::Ok
        );
        assert_eq!(latency, rust_latency);
        // SAFETY: p came from rr_dr60_create and is not used afterwards.
        unsafe { rr_dr60_destroy(p) };
    }

    // Errors are values (001 FR-024).
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
