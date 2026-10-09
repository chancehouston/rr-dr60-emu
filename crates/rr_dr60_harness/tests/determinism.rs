//! Determinism: block-partition invariance, in-place equivalence, seed independence
//! (FR-009, FR-014, SC-003; tasks.md T052).

use rr_dr60::Pipeline;
use rr_dr60_harness::stimulus::Pcg32;
use rr_dr60_harness::{configs, golden};

fn one_block(s: rr_dr60::Settings, x: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(s).unwrap();
    let mut y = vec![0.0; x.len()];
    p.process(x, &mut y).unwrap();
    y
}

/// Splits `x` into random blocks of 0..=8192 samples, always including blocks of size 0 and 1.
fn partitioned(s: rr_dr60::Settings, x: &[f32], rng: &mut Pcg32) -> Vec<f32> {
    let mut p = Pipeline::new(s).unwrap();
    let mut out = Vec::with_capacity(x.len());
    let mut pos = 0;
    let mut first = true;
    while pos < x.len() {
        let n = if first { 0 } else { rng.below(8193) as usize }.min(x.len() - pos);
        let n = if pos == 0 && !first { 1 } else { n };
        first = false;
        let mut y = vec![0.0; n];
        p.process(&x[pos..pos + n], &mut y).unwrap();
        out.extend(y);
        pos += n;
    }
    out
}

#[test]
fn hundred_partitions_of_each_golden_stimulus_at_48k() {
    let mut rng = Pcg32::new(0x0D60, 1);
    let s = configs::settings("default", 48_000);
    for stim in golden::STIMULI {
        let x = golden::stimulus(stim, 48_000);
        let want = one_block(s, &x);
        for i in 0..100 {
            assert_eq!(partitioned(s, &x, &mut rng), want, "{stim}, partition {i}");
        }
    }
}

#[test]
fn ten_partitions_for_every_other_config_and_rate() {
    let mut rng = Pcg32::new(0x0D60, 2);
    for rate in configs::all_rates() {
        for config in configs::CONFIGS {
            if rate == 48_000 && config == "default" {
                continue;
            }
            let s = configs::settings(config, rate);
            for stim in golden::STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = one_block(s, &x);
                for i in 0..10 {
                    assert_eq!(
                        partitioned(s, &x, &mut rng),
                        want,
                        "{rate} Hz {config} {stim}, partition {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn in_place_equals_copy() {
    for rate in configs::all_rates() {
        let x = golden::stimulus("sweep_log", rate);
        let mut p = Pipeline::new(configs::settings("default", rate)).unwrap();
        let mut buf = x.clone();
        p.process_in_place(&mut buf);
        assert_eq!(
            buf,
            one_block(configs::settings("default", rate), &x),
            "{rate} Hz"
        );
    }
}

#[test]
fn seed_does_not_change_output() {
    let x = golden::stimulus("noise_seed_0d60", 44_100);
    let mut a = configs::settings("default", 44_100);
    let mut b = a;
    a.seed = 0;
    b.seed = u64::MAX;
    assert_eq!(one_block(a, &x), one_block(b, &x)); // FR-009
}

/// Spec 002 SC-003, FR-013: the AGC is block-partition invariant. 100 random partitions
/// (including sizes 0 and 1) of each AGC golden stimulus, for the AGC configurations and two
/// extreme settings, at 8 kHz (where the stimuli are shortest), plus 10 partitions at 48 kHz.
#[test]
fn agc_partitions() {
    let mut rng = Pcg32::new(0x0D60, 4);
    let cases = |rate: u32| {
        let mut attack_min = configs::settings("default_agc", rate);
        attack_min.agc.attack_ms = 1.0;
        let mut release_max = configs::settings("default_agc", rate);
        release_max.agc.release_ms = 10_000.0;
        [
            ("agc_only", configs::settings("agc_only", rate)),
            ("default_agc", configs::settings("default_agc", rate)),
            ("attack min", attack_min),
            ("release max", release_max),
        ]
    };
    for (rate, partitions) in [(8000, 100), (48_000, 10)] {
        for (name, s) in cases(rate) {
            for stim in golden::AGC_STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = one_block(s, &x);
                for i in 0..partitions {
                    assert_eq!(
                        partitioned(s, &x, &mut rng),
                        want,
                        "{rate} Hz {name} {stim}, partition {i}"
                    );
                }
            }
        }
    }
}
