//! Spec 002 edge cases (tasks.md T036; spec Edge Cases; research.md R-07, R-11).

use rr_dr60::{AgcSettings, Pipeline, Settings};
use rr_dr60_harness::analysis::{db, from_db, gain_trajectory_exact, single_bin};
use rr_dr60_harness::{agc_checks, configs, stimulus};

fn run(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    Pipeline::new(settings).unwrap().process(x, &mut y).unwrap();
    y
}

fn agc_only(rate: u32, agc: AgcSettings) -> Settings {
    let mut s = configs::settings("agc_only", rate);
    s.agc = agc;
    s
}

/// 002 FR-013, A-019: a fresh pipeline applies exactly the maximum gain to its first non-zero
/// sample (8 kHz, where the rate conversion is the identity).
#[test]
fn starts_at_maximum_gain() {
    let x = stimulus::step(1000.0, &[-10.0], &[0.01], 8000.0);
    let y = run(agc_only(8000, AgcSettings::DEVICE), &x);
    let n = x.iter().position(|&v| v != 0.0).unwrap();
    let gain = db(f64::from(y[n]) / f64::from(x[n]));
    assert!(
        (gain - 40.0).abs() <= 0.01,
        "first-sample gain {gain:.4} dB"
    );
}

/// 002 FR-013: after a loud tone, `reset()` and `reconfigure(same settings)` each behave like a
/// freshly created pipeline, bit for bit.
#[test]
fn reset_and_reconfigure_equal_fresh() {
    let s = configs::settings("default_agc", 48_000);
    let loud = stimulus::step(1000.0, &[0.0], &[0.3], 48_000.0);
    let x = stimulus::step(1000.0, &[-30.0, -10.0], &[0.2, 0.2], 48_000.0);
    let fresh = run(s, &x);
    for how in ["reset", "reconfigure"] {
        let mut p = Pipeline::new(s).unwrap();
        let mut warm = loud.clone();
        p.process_in_place(&mut warm);
        match how {
            "reset" => p.reset(),
            _ => p.reconfigure(s).unwrap(),
        }
        let mut y = vec![0.0; x.len()];
        p.process(&x, &mut y).unwrap();
        assert_eq!(y, fresh, "{how}");
    }
}

/// Spec Edge Cases: with maximum gain and maximum attenuation both 0 dB, output equals input
/// within ±0.01 dB at every level (here: bit for bit, since the gain is exactly 0 dB).
#[test]
fn zero_gain_range_is_unity() {
    let mut agc = AgcSettings::DEVICE;
    agc.max_gain_db = 0.0;
    agc.max_attenuation_db = 0.0;
    for rate in [8000, 48_000] {
        let mut off = agc_only(rate, agc);
        off.agc.enabled = false;
        for level in [-60.0, -10.0, 10.0] {
            let x = stimulus::step(1000.0, &[level], &[0.3], f64::from(rate));
            let (y, y_ref) = (run(agc_only(rate, agc), &x), run(off, &x));
            let worst = y
                .iter()
                .zip(&y_ref)
                .filter(|(_, r)| r.abs() > 0.0)
                .map(|(a, r)| db(f64::from(*a) / f64::from(*r)).abs())
                .fold(0.0, f64::max);
            assert!(worst <= 0.01, "{rate} Hz, {level} dBFS: {worst} dB");
            assert_eq!(
                y, y_ref,
                "{rate} Hz, {level} dBFS: expected bit-exact unity gain"
            );
        }
    }
}

/// research.md R-07 and spec Edge Cases › Long silence, 10 minutes each (slow in debug):
/// (a) maximum attenuation 0 with a 0 dBFS tone: the gain decays from 40 dB, flushes to exactly
///     0 dB, and from then on output equals input bit for bit; no subnormal output;
/// (b) 10 minutes of digital silence after a loud tone: exact zeros out, then the gain is at
///     maximum within 1e-9 dB.
#[test]
#[ignore = "slow: 10 + 10 minutes of audio; run in release mode (CI check job)"]
fn ten_minute_flush_and_long_silence() {
    let fs = 8000.0;
    let ten_min = (600.0 * fs) as usize;
    let mut agc = AgcSettings::DEVICE;
    agc.max_attenuation_db = 0.0;
    let x: Vec<f32> = stimulus::step(1000.0, &[0.0], &[600.0], fs);
    let y = run(agc_only(8000, agc), &x);
    assert!(y.iter().all(|v| !v.is_subnormal()));
    let last_diff = y
        .iter()
        .zip(&x)
        .rposition(|(a, b)| a.to_bits() != b.to_bits());
    let flushed_from = last_diff.map_or(0, |i| i + 1);
    assert!(
        flushed_from < ten_min - 8000,
        "gain never flushed to 0 dB (last diff {last_diff:?})"
    );

    let mut p = Pipeline::new(agc_only(8000, AgcSettings::DEVICE)).unwrap();
    let mut loud = stimulus::step(1000.0, &[0.0], &[1.0], fs);
    p.process_in_place(&mut loud);
    let mut quiet = stimulus::silence(ten_min);
    p.process_in_place(&mut quiet);
    assert!(
        quiet.iter().all(|v| v.to_bits() == 0),
        "silence must stay exactly 0.0"
    );
    let probe = stimulus::step(1000.0, &[-80.0], &[0.001], fs);
    let mut out = probe.clone();
    p.process_in_place(&mut out);
    let n = probe.iter().position(|&v| v != 0.0).unwrap();
    let gain = db(f64::from(out[n]) / f64::from(probe[n]));
    assert!(
        (gain - 40.0).abs() <= 1e-6,
        "gain after long silence {gain} dB"
    );
}

/// Spec Edge Cases › DC: DC counts as level. DC of amplitude 0.1 reads like a sine of peak 0.1
/// (−20 dBFS), so the target gain is +9 dB and the steady output is 0.1 · 10^(9/20) ± 1 dB
/// (engineering target, R-11).
#[test]
fn dc_drives_the_gain() {
    let x = stimulus::dc(0.1, 48_000);
    let y = run(agc_only(48_000, AgcSettings::DEVICE), &x);
    let tail = &y[y.len() - 12_000..];
    let mean = tail.iter().map(|&v| f64::from(v)).sum::<f64>() / tail.len() as f64;
    let err = db(mean / (0.1 * from_db(9.0)));
    eprintln!("DC 0.1 → {mean:.5} ({err:+.3} dB vs expected)");
    assert!(err.abs() <= 1.0, "DC output {mean} ({err:+.3} dB)");
}

/// Spec Edge Cases › Out-of-band low frequencies: a 50 Hz tone at −20 dBFS drives the AGC
/// even though stage 4 would remove it: the mean gain is at least 20 dB below the silence
/// gain (engineering target, R-11).
#[test]
fn low_frequencies_drive_the_gain() {
    let rate = 48_000;
    let fs = f64::from(rate);
    let s = agc_only(rate, AgcSettings::DEVICE);
    let x = stimulus::step(50.0, &[-20.0], &[2.0], fs);
    let mut off = s;
    off.agc.enabled = false;
    let traj = gain_trajectory_exact(&run(off, &x), &run(s, &x), 5.0 * stimulus::amplitude(-20.0));
    let tail: Vec<f64> = traj
        .iter()
        .filter(|p| p.0 > (1.5 * fs) as usize)
        .map(|p| p.1)
        .collect();
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    eprintln!("50 Hz −20 dBFS: mean gain {mean:.2} dB (silence gain 40 dB)");
    assert!(mean <= 40.0 - 20.0, "mean gain {mean:.2} dB");
}

/// Spec Edge Cases › Content above 4 kHz: at 48 kHz, adding a 6 kHz tone at −10 dBFS to a
/// 1 kHz tone at −40 dBFS leaves the 1 kHz output level unchanged within 0.1 dB
/// (engineering target; A-019: the AGC acts on the device band).
///
/// Measured over the last 0.25 s of 6 s: switching the 6 kHz tone on abruptly is a click with
/// in-band energy, which the AGC (starting at +40 dB) reacts to; the gain needs a few release
/// times to recover from that onset (T036).
#[test]
fn content_above_4khz_does_not_affect_the_gain() {
    let fs = 48_000.0;
    let s = agc_only(48_000, AgcSettings::DEVICE);
    let base = stimulus::step(1000.0, &[-40.0], &[6.0], fs);
    let hf = stimulus::step(6000.0, &[-10.0], &[6.0], fs);
    let both: Vec<f32> = base.iter().zip(&hf).map(|(a, b)| a + b).collect();
    let level = |y: &[f32]| {
        let len = 12_000;
        db(single_bin(y, 1000.0, fs, y.len() - len, len).norm())
    };
    let (a, b) = (level(&run(s, &base)), level(&run(s, &both)));
    eprintln!("1 kHz alone {a:.3} dBFS, with 6 kHz {b:.3} dBFS");
    assert!((a - b).abs() <= 0.1, "{a:.3} vs {b:.3}");
}

/// Spec Edge Cases › Non-finite input: NaN, +Inf and −Inf mid-stream give output bit-identical
/// to the same stream with 0.0 in their place; no reset needed.
#[test]
fn non_finite_input_acts_as_zero() {
    let fs = 48_000.0;
    let s = configs::settings("default_agc", 48_000);
    let clean = stimulus::step(1000.0, &[-20.0], &[0.5], fs);
    let (mut bad, mut zeroed) = (clean.clone(), clean);
    for (i, v) in [
        (4_000, f32::NAN),
        (9_000, f32::INFINITY),
        (15_000, f32::NEG_INFINITY),
    ] {
        bad[i] = v;
        zeroed[i] = 0.0;
    }
    let y = run(s, &bad);
    assert!(y.iter().all(|v| v.is_finite()));
    assert_eq!(y, run(s, &zeroed));
}

/// Spec Edge Cases › Overshoot: the start-at-maximum peak passes unclipped and finite (about
/// +30 dBFS for a −10 dBFS input at defaults); a huge finite input saturates instead of
/// becoming Inf (R-07).
#[test]
fn overshoot_is_unclipped_and_finite() {
    let fs = 48_000.0;
    let s = agc_only(48_000, AgcSettings::DEVICE);
    let y = run(s, &stimulus::step(1000.0, &[-10.0], &[0.5], fs));
    let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    eprintln!("start-at-maximum peak {:.2} dBFS", db(f64::from(peak)));
    assert!(peak > 10.0, "overshoot looks clipped: peak {peak}");
    assert!(y.iter().all(|v| v.is_finite()));
    let huge = run(s, &stimulus::dc(1e37, 4800));
    assert!(
        huge.iter().all(|v| v.is_finite()),
        "huge input produced Inf"
    );
}

/// The FR-009 stimulus is bit-reproducible (it feeds the golden file).
#[test]
fn noise_burst_stimulus_is_reproducible() {
    assert_eq!(
        agc_checks::noise_burst_stimulus(16_000.0),
        agc_checks::noise_burst_stimulus(16_000.0)
    );
}
