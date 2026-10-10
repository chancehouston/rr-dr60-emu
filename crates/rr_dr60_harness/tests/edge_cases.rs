//! Spec Edge Cases, one test each (tasks.md T051).

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
    // 30 s at 48 kHz in one block vs 4096-sample blocks (FR-014). The 10-minute case is in
    // timing.rs (release mode).
    let x = stimulus::noise(0x0D60, 30 * 48_000);
    let whole = run(&mut fresh(48_000), &x);
    let mut p = fresh(48_000);
    let mut pieces = Vec::with_capacity(x.len());
    for chunk in x.chunks(4096) {
        pieces.extend(run(&mut p, chunk));
    }
    assert_eq!(whole, pieces);
}

#[test]
fn non_finite_input_is_treated_as_zero() {
    for rate in [8000, 48_000] {
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
    let x = stimulus::tone(1000.0, 4.0, 48_000.0, 48_000);
    let y = run(&mut fresh(48_000), &x);
    let peak = y[9600..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak > 3.8 && peak < 4.2, "peak {peak}");
}

#[test]
fn dc_is_removed() {
    // 0 dBFS DC in the default configuration: steady state <= -60 dBFS (FR-010 DC, A-014).
    let y = run(&mut fresh(48_000), &stimulus::dc(1.0, 96_000));
    let tail = &y[48_000..];
    let mean_square = tail
        .iter()
        .map(|&v| f64::from(v) * f64::from(v))
        .sum::<f64>()
        / tail.len() as f64;
    let rms = rr_dr60_detmath::sqrt(mean_square);
    assert!(
        analysis::db(rms) <= -60.0,
        "DC residue {:.1} dBFS",
        analysis::db(rms)
    );
}

#[test]
fn tail_decays_then_reaches_exact_zero() {
    for rate in [8000, 44_100, 96_000] {
        let fs = f64::from(rate);
        let mut x = stimulus::log_sweep(20.0, 0.45 * fs, 1.0, fs, rate as usize);
        let stop = x.len();
        x.extend(stimulus::silence(3 * rate as usize));
        let y = run(&mut fresh(rate), &x);
        let half_second = stop + rate as usize / 2;
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
    let x = stimulus::noise(3, 9600);
    let mut p = fresh(88_200);
    run(&mut p, &stimulus::noise(4, 5000));
    p.reset();
    assert_eq!(run(&mut p, &x), run(&mut fresh(88_200), &x));
}

#[test]
fn unsupported_rate_error_names_supported_rates() {
    let e = Pipeline::new(Settings::new(22_050)).unwrap_err();
    assert_eq!(e, Error::UnsupportedHostRate { requested: 22_050 });
    let text = e.to_string();
    for r in rr_dr60::SUPPORTED_HOST_RATES {
        assert!(text.contains(&r.to_string()), "{text}");
    }
}
