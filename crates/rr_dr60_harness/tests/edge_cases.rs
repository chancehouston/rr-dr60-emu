//! Spec Edge Cases, one test each (001 tasks.md T051; restated for the 6 kHz device rate by
//! spec 004 FR-010 and its Edge Cases, 004 tasks.md T025).

use rr_dr60::{Error, Pipeline, Settings};
use rr_dr60_harness::{analysis, configs, stimulus};

fn run(p: &mut Pipeline, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = p.process(x, &mut y).unwrap().produced;
    y.truncate(produced);
    y
}

fn fresh(rate: u32) -> Pipeline {
    Pipeline::new(configs::settings("default", rate)).unwrap()
}

/// Steady-state RMS of `y` from `from` on, in dBFS.
fn tail_dbfs(y: &[f32], from: usize) -> f64 {
    let tail = &y[from..];
    let mean_square = tail
        .iter()
        .map(|&v| f64::from(v) * f64::from(v))
        .sum::<f64>()
        / tail.len() as f64;
    analysis::db(rr_dr60_detmath::sqrt(mean_square))
}

#[test]
fn block_of_size_zero_is_a_no_op() {
    let x = stimulus::noise(7, 4800);
    let mut p = fresh(44_100);
    let mut empty: [f32; 0] = [];
    assert_eq!(p.process(&[], &mut empty).unwrap().produced, 0);
    assert_eq!(p.process_in_place(&mut empty).produced, 0);
    assert_eq!(run(&mut p, &x), run(&mut fresh(44_100), &x));
}

#[test]
fn large_block_equals_small_blocks() {
    // 30 s in one block vs 4096-sample blocks (001 FR-014) at the 6 kHz identity rate and at
    // 48 kHz. The 10-minute case is in timing.rs (release mode).
    for rate in [6000, 48_000] {
        let x = stimulus::noise(0x0D60, 30 * rate as usize);
        let whole = run(&mut fresh(rate), &x);
        let mut p = fresh(rate);
        let mut pieces = Vec::with_capacity(x.len());
        for chunk in x.chunks(4096) {
            pieces.extend(run(&mut p, chunk));
        }
        assert_eq!(whole, pieces, "{rate} Hz");
    }
}

#[test]
fn non_finite_input_is_treated_as_zero() {
    for rate in [6000, 48_000] {
        let mut x = stimulus::tone(1000.0, 0.5, f64::from(rate), rate as usize);
        for (i, bad) in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(1),
        ]
        .into_iter()
        .enumerate()
        {
            x[1000 + 500 * i] = bad;
        }
        let y = run(&mut fresh(rate), &x);
        assert!(
            y.iter().all(|v| v.is_finite() && !v.is_subnormal()),
            "{rate} Hz"
        );
        // No reset needed: replacing the bad samples with 0.0 gives identical output.
        let mut zeroed = x.clone();
        for v in &mut zeroed {
            if !v.is_finite() || v.is_subnormal() {
                *v = 0.0;
            }
        }
        assert_eq!(y, run(&mut fresh(rate), &zeroed), "{rate} Hz");
    }
}

#[test]
fn over_full_scale_is_not_clipped() {
    // Spec 001 Edge Cases, restated by spec 005 FR-008 (A-030): the band-limit stages pass
    // input beyond full scale unclipped, which now shows with the ADC clamp off; through the
    // `default` configuration (clamp on) the codec saturates the tone at ±1.0 and the playback
    // stage reconstructs the flat tops with overshoot (a full-scale square wave's fundamental is
    // 4/π = 1.27), so the peak lies between 1.0 and 1.5 (engineering target, 005 R-06).
    let x = stimulus::tone(1000.0, 4.0, 48_000.0, 48_000);
    let mut unclamped = configs::settings("default", 48_000);
    unclamped.adc_clamp_enabled = false;
    let y = run(&mut Pipeline::new(unclamped).unwrap(), &x);
    let peak = y[9600..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak > 3.8 && peak < 4.2, "unclamped peak {peak}");
    let y = run(&mut fresh(48_000), &x);
    let peak = y[9600..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        (1.0..1.5).contains(&peak),
        "clamped peak {peak} (005 FR-008)"
    );
}

#[test]
fn dc_is_removed() {
    // 0 dBFS DC in the default configuration: steady state <= -60 dBFS (spec 004 Edge Cases,
    // restating 001; engineering target). Since spec 004 the rejection comes from the record
    // stage alone (the transmit template's zeros at DC, A-027): the playback stage is a
    // low-pass that passes DC (FR-008), so it contributes nothing here.
    let y = run(&mut fresh(48_000), &stimulus::dc(1.0, 96_000));
    let residue = tail_dbfs(&y, 48_000);
    assert!(residue <= -60.0, "DC residue {residue:.1} dBFS");
}

#[test]
fn dc_passes_through_playback_only() {
    // Spec 004 FR-008 (S-005 receive template, A-027): the playback stage has no low-frequency
    // rejection. 0 dBFS DC through `playback_only` at 48 kHz settles to a level within
    // -0.15 / +0.25 dB of the input (the template's in-band loss limits).
    let mut p = Pipeline::new(configs::settings("playback_only", 48_000)).unwrap();
    let y = run(&mut p, &stimulus::dc(1.0, 96_000));
    let tail = &y[48_000..];
    let mean = tail.iter().map(|&v| f64::from(v)).sum::<f64>() / tail.len() as f64;
    let loss_db = -analysis::db(mean);
    assert!(
        (-0.15..=0.25).contains(&loss_db),
        "playback_only DC loss {loss_db:+.3} dB (must pass DC, 004 FR-008)"
    );
}

#[test]
fn tail_decays_then_reaches_exact_zero() {
    // The 6 kHz identity rate, a non-integer conversion and the 48 kHz integer conversion.
    for rate in [6000, 44_100, 48_000] {
        let fs = f64::from(rate);
        let mut x = stimulus::log_sweep(20.0, 0.45 * fs, 1.0, fs, rate as usize);
        let stop = x.len();
        x.extend(stimulus::silence(3 * rate as usize));
        let y = run(&mut fresh(rate), &x);
        let half_second = stop + rate as usize / 2; // 0.5 s flush, engineering target
        let worst = y[half_second..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            f64::from(worst) < 1e-6,
            "{rate} Hz: {:.1} dBFS 0.5 s after stop",
            analysis::db(f64::from(worst))
        );
        assert_eq!(
            *y.last().unwrap(),
            0.0,
            "{rate} Hz: tail never reached exact 0.0"
        );
    }
}

#[test]
fn reset_equals_new() {
    for rate in [6000, 48_000, 88_200] {
        let x = stimulus::noise(3, 9600);
        let mut p = fresh(rate);
        run(&mut p, &stimulus::noise(4, 5000));
        p.reset();
        assert_eq!(run(&mut p, &x), run(&mut fresh(rate), &x), "{rate} Hz");
    }
}

#[test]
fn unsupported_rate_error_names_supported_rates() {
    // Spec 004 FR-002: the seven rates, 6 kHz included, are named in the error text.
    for requested in [7000, 22_050] {
        let e = Pipeline::new(Settings::new(requested)).unwrap_err();
        assert_eq!(e, Error::UnsupportedHostRate { requested });
        let text = e.to_string();
        assert_eq!(rr_dr60::SUPPORTED_HOST_RATES.len(), 7);
        for r in [6000, 8000, 16_000, 44_100, 48_000, 88_200, 96_000] {
            assert!(
                rr_dr60::SUPPORTED_HOST_RATES.contains(&r) && text.contains(&r.to_string()),
                "{text} should name {r}"
            );
        }
    }
}
