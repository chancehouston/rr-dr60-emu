//! Spec 011 edge cases reachable without the whole-engine code (tasks.md T015; spec › Edge
//! Cases): a change to the same position starts no transition; several positions set in one
//! gap count as the last; a refused position starts no transition; the read-back position is the
//! host's setting during a transition; the transition's length at the low rates.

use rr_dr60::Route;
use rr_dr60::output::volume_transition_samples;
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::volume_ramp_checks::{self as ramp, Blocks, run_with_changes, same};
use rr_dr60_harness::{configs, stimulus};

fn scene(rate: u32, config: &str) -> (rr_dr60::Settings, Route, Vec<f32>, usize) {
    let s = configs::settings(config, rate);
    let route = configs::stage11_route(config);
    let c = ramp::change_at(rate);
    let latency = standard(s).latency_samples() as usize;
    let level = ramp::level_dbfs(route);
    let x = stimulus::tone_peaking_at(level, rate, c + rate as usize, c, latency);
    (s, route, x, c)
}

/// "A change to the same position": not a change; the output equals the held run.
#[test]
fn same_position_starts_no_transition() {
    for rate in [6000, 48_000] {
        for config in configs::STAGE11_CONFIGS {
            let (s, route, x, c) = scene(rate, config);
            let held = run_with_changes(&standard, s, route, 0.1, &[], &x, Blocks::Fixed(64));
            let again =
                run_with_changes(&standard, s, route, 0.1, &[(c, 0.1)], &x, Blocks::Fixed(64));
            assert!(same(&held, &again), "{rate} Hz {config}");
        }
    }
}

/// "Several positions set in one gap": only the last counts.
#[test]
fn several_positions_in_one_gap_count_as_the_last() {
    for rate in [6000, 48_000] {
        for config in configs::STAGE11_CONFIGS {
            let (s, route, x, c) = scene(rate, config);
            let last =
                run_with_changes(&standard, s, route, 0.1, &[(c, 1.0)], &x, Blocks::Fixed(64));
            let many = run_with_changes(
                &standard,
                s,
                route,
                0.1,
                &[(c, 0.5), (c, 0.0), (c, 1.0)],
                &x,
                Blocks::Fixed(64),
            );
            assert!(same(&last, &many), "{rate} Hz {config}");
        }
    }
}

/// "A refused position": the position is unchanged and no transition starts; the read-back
/// position is the host's last valid setting, also during a transition.
#[test]
fn refused_position_and_read_back() {
    for rate in [6000, 48_000] {
        let (s, route, x, c) = scene(rate, "stage11_only_earphone");
        let held = run_with_changes(&standard, s, route, 0.1, &[], &x, Blocks::Fixed(64));
        let mut p = standard(s);
        p.set_route(route);
        p.set_volume(0.1).unwrap();
        let mut y = Vec::new();
        for (i, chunk) in x.chunks(64).enumerate() {
            if i * 64 == c {
                assert!(p.set_volume(1.5).is_err());
                assert!(p.set_volume(f32::NAN).is_err());
                assert_eq!(p.volume(), 0.1);
            }
            let mut out = vec![0.0f32; chunk.len()];
            let n = p.process(chunk, &mut out).unwrap().produced;
            y.extend_from_slice(&out[..n]);
        }
        assert!(
            same(&y, &held),
            "{rate} Hz: a refused position starts nothing"
        );
        // During a transition the read-back is the host's setting, not the knob's position.
        let mut q = standard(s);
        q.set_route(route);
        q.set_volume(0.1).unwrap();
        let mut out = vec![0.0f32; 64];
        let _ = q.process(&x[..64], &mut out).unwrap();
        q.set_volume(1.0).unwrap();
        let _ = q.process(&x[64..128], &mut out).unwrap();
        assert_eq!(q.volume(), 1.0, "{rate} Hz: mid-transition read-back");
    }
}

/// "Low host rates": 120 samples at 6 kHz, 160 at 8 kHz, exactly 20 ms at every rate.
#[test]
fn transition_length_at_every_rate() {
    for rate in configs::all_rates() {
        let n = volume_transition_samples(rate).unwrap();
        assert_eq!(f64::from(n), f64::from(rate) * 0.02, "{rate} Hz");
    }
    assert_eq!(volume_transition_samples(6000), Some(120));
    assert_eq!(volume_transition_samples(8000), Some(160));
}
