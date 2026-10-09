//! User Story 2 acceptance tests: bypass, tap, latency per configuration, reconfigure
//! (spec.md US2 AS1–AS4, FR-007, FR-008, FR-012; tasks.md T040, T041).
//!
//! Per-stage checks subtract the fully bypassed baseline (FR-010), so they don't depend on
//! the other stage's behavior (Constitution VII).

use rr_dr60::{Error, Pipeline, Settings, Tap};
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60Status, rr_dr60_create, rr_dr60_destroy, rr_dr60_latency_samples,
    rr_dr60_process, rr_dr60_reconfigure, rr_dr60_settings_default,
};
use rr_dr60_harness::analysis::{self, Complex64};
use rr_dr60_harness::stimulus;

/// The five named configurations (data-model.md).
fn config(name: &str, host: u32) -> Settings {
    let mut s = Settings::new(host);
    match name {
        "default" => {}
        "record_only" => s.playback_stage_enabled = false,
        "playback_only" => s.record_stage_enabled = false,
        "tap_after_record" => s.tap = Tap::AfterRecord,
        "bypass_all" => {
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
        }
        other => panic!("unknown config {other}"),
    }
    s
}

const CONFIGS: [&str; 5] = [
    "default",
    "record_only",
    "playback_only",
    "tap_after_record",
    "bypass_all",
];

fn run(settings: Settings, input: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(settings).unwrap();
    let mut out = vec![0.0; input.len()];
    p.process(input, &mut out).unwrap();
    out
}

/// Complex gain of a configuration at `f` Hz (1 s tone, first 0.2 s discarded).
fn response(settings: Settings, f: f64) -> Complex64 {
    let fs = f64::from(settings.host_rate_hz);
    let x = stimulus::tone(f, 0.25, fs, settings.host_rate_hz as usize);
    let y = run(settings, &x);
    analysis::gain_and_phase(&x, &y, f, fs, settings.host_rate_hz as usize / 5)
}

/// Analytic voice-band stage magnitude in dB at `f` Hz (8 kHz device rate, A-001).
fn analytic_stage_db(f: f64) -> f64 {
    let w = std::f64::consts::TAU * f / 8000.0;
    let z1 = Complex64::from_polar(1.0, -w);
    let z2 = Complex64::from_polar(1.0, -2.0 * w);
    let h: Complex64 = rr_dr60::__test_hooks::voiceband_sos()
        .iter()
        .map(|&[b0, b1, b2, a1, a2]| (b0 + b1 * z1 + b2 * z2) / (1.0 + a1 * z1 + a2 * z2))
        .product();
    analysis::db(h.norm())
}

#[test]
fn as1_record_stage_alone_matches_its_analytic_response() {
    for host in [48_000, 8000] {
        for f in [300.0, 1000.0, 3200.0, 3400.0] {
            let stage =
                response(config("record_only", host), f) / response(config("bypass_all", host), f);
            let measured = analysis::db(stage.norm());
            let want = analytic_stage_db(f);
            assert!(
                (measured - want).abs() <= 0.3,
                "{host} Hz host, {f} Hz: {measured:.3} dB vs analytic {want:.3}"
            );
            if f == 1000.0 {
                assert!(
                    measured.abs() <= 0.1,
                    "{host}: stage gain at 1 kHz {measured:.3} dB (A-015)"
                );
            }
        }
    }
}

#[test]
fn as2_tap_after_record_equals_record_only() {
    for host in [48_000, 8000] {
        let x = stimulus::log_sweep(
            20.0,
            0.45 * f64::from(host),
            0.25,
            f64::from(host),
            host as usize,
        );
        let want = run(config("record_only", host), &x);
        for playback in [true, false] {
            let mut s = config("tap_after_record", host);
            s.playback_stage_enabled = playback;
            assert_eq!(
                run(s, &x),
                want,
                "{host} Hz, playback_stage_enabled = {playback}"
            );
        }
    }
}

#[test]
fn as3_bypass_all_is_flat_and_exact_at_8k() {
    for f in [50.0, 300.0, 1000.0, 3000.0, 3600.0] {
        let g = analysis::db(response(config("bypass_all", 48_000), f).norm());
        assert!(
            g.abs() <= 0.1,
            "48 kHz bypass_all: {g:.3} dB at {f} Hz (FR-005)"
        );
    }
    // FR-002: at 8 kHz the boundary is the identity, so bypass_all is an exact pass-through,
    // except non-finite and subnormal samples, which become 0.0.
    let mut x = stimulus::noise(0x0D60, 8000);
    x[10] = f32::NAN;
    x[20] = f32::INFINITY;
    x[30] = f32::NEG_INFINITY;
    x[40] = f32::from_bits(1);
    x[50] = 3.5; // over full scale: passed through, not clipped
    let y = run(config("bypass_all", 8000), &x);
    for (i, (&a, &b)) in x.iter().zip(&y).enumerate() {
        let want = if a.is_finite() && !a.is_subnormal() {
            a
        } else {
            0.0
        };
        assert_eq!(
            b.to_bits(),
            want.to_bits(),
            "sample {i}: in {a:e}, out {b:e}"
        );
    }
}

#[test]
fn as4_reported_latency_matches_measured_group_delay() {
    for host in [48_000, 8000] {
        for name in CONFIGS {
            let s = config(name, host);
            let reported = Pipeline::new(s).unwrap().latency_samples();
            let measured =
                analysis::group_delay_samples(|f| response(s, f), 1000.0, f64::from(host));
            assert!(
                (f64::from(reported) - measured).abs() <= 1.0,
                "{host} Hz {name}: reported {reported}, measured {measured:.2} (FR-012)"
            );
        }
    }
}

#[test]
fn reconfigure_equals_new() {
    let x = stimulus::log_sweep(20.0, 3900.0, 0.25, 8000.0, 8000);
    let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
    let mut scratch = vec![0.0; 4800];
    p.process(&stimulus::noise(1, 4800), &mut scratch).unwrap(); // dirty the state first
    for s in [
        Settings::new(44_100),
        config("record_only", 8000),
        config("tap_after_record", 48_000),
        Settings::new(8000),
    ] {
        p.reconfigure(s).unwrap();
        assert_eq!(p.settings(), &s);
        let fresh = Pipeline::new(s).unwrap();
        assert_eq!(p.latency_samples(), fresh.latency_samples(), "{s:?}");
        let mut y = vec![0.0; x.len()];
        p.process(&x, &mut y).unwrap();
        assert_eq!(y, run(s, &x), "reconfigure({s:?}) differs from new()");
    }
}

#[test]
fn failed_reconfigure_changes_nothing() {
    let x = stimulus::tone(1000.0, 0.25, 48_000.0, 4800);
    let mut p = Pipeline::new(config("record_only", 48_000)).unwrap();
    let mut first = vec![0.0; x.len()];
    p.process(&x, &mut first).unwrap();
    let before = (*p.settings(), p.latency_samples());
    assert_eq!(
        p.reconfigure(Settings::new(22_050)),
        Err(Error::UnsupportedHostRate { requested: 22_050 })
    );
    assert_eq!((*p.settings(), p.latency_samples()), before);
    // State was kept too: the next block continues the stream exactly.
    let mut second = vec![0.0; x.len()];
    p.process(&x, &mut second).unwrap();
    let mut q = Pipeline::new(config("record_only", 48_000)).unwrap();
    let mut both = vec![0.0; 2 * x.len()];
    q.process(&[x.clone(), x.clone()].concat(), &mut both)
        .unwrap();
    assert_eq!([first, second].concat(), both);
}

#[test]
fn c_api_reconfigure() {
    let x = stimulus::tone(1000.0, 0.25, 44_100.0, 4410);
    let s = rr_dr60_settings_default(48_000);
    let mut p: *mut RrDr60Pipeline = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&s, &mut p) }, RrDr60Status::Ok);

    // Invalid tap: rejected, and the old configuration still processes.
    let mut bad = rr_dr60_settings_default(44_100);
    bad.tap = 7;
    // SAFETY: p is live; bad is valid.
    assert_eq!(
        unsafe { rr_dr60_reconfigure(p, &bad) },
        RrDr60Status::InvalidArgument
    );
    let mut lat = 0;
    // SAFETY: p is live.
    assert_eq!(
        unsafe { rr_dr60_latency_samples(p, &mut lat) },
        RrDr60Status::Ok
    );
    assert_eq!(
        lat,
        Pipeline::new(Settings::new(48_000))
            .unwrap()
            .latency_samples()
    );
    // SAFETY: NULL settings must be rejected.
    assert_eq!(
        unsafe { rr_dr60_reconfigure(p, std::ptr::null()) },
        RrDr60Status::NullPointer
    );
    let unsupported = rr_dr60_settings_default(22_050);
    // SAFETY: p is live.
    assert_eq!(
        unsafe { rr_dr60_reconfigure(p, &unsupported) },
        RrDr60Status::UnsupportedHostRate
    );

    // A valid change matches the Rust API.
    let mut good = rr_dr60_settings_default(44_100);
    good.playback_stage_enabled = false;
    // SAFETY: p is live; good is valid.
    assert_eq!(unsafe { rr_dr60_reconfigure(p, &good) }, RrDr60Status::Ok);
    let mut y = vec![0.0f32; x.len()];
    // SAFETY: p is live; buffers are valid and do not overlap.
    assert_eq!(
        unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), x.len()) },
        RrDr60Status::Ok
    );
    assert_eq!(y, run(config("record_only", 44_100), &x));
    // SAFETY: NULL handle must be rejected.
    assert_eq!(
        unsafe { rr_dr60_reconfigure(std::ptr::null_mut(), &good) },
        RrDr60Status::NullPointer
    );
    // SAFETY: p came from rr_dr60_create.
    unsafe { rr_dr60_destroy(p) };
}
