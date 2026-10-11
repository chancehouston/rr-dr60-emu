//! Spec 003 edge cases (tasks.md T036; spec Edge Cases; research.md R-04, R-09, R-11).
//!
//! At 8 and 48 kHz unless stated. Everything is measured from output lengths and events (R-11).

use rr_dr60::{Pipeline, Settings, Tap, VasEvent, VasMode, VasSettings};
use rr_dr60_harness::vas_checks::{self, Run, hang_s, onset_s};
use rr_dr60_harness::{configs, stimulus};

const RATES: [u32; 2] = [8000, 48_000];
/// Burst level: the default threshold (−18 dBFS, A-022) + 10 dB (spec 003 definitions).
const BURST_DBFS: f64 = -8.0;

fn run(settings: Settings, x: &[f32]) -> Run {
    vas_checks::run(&|s| Pipeline::new(s).unwrap(), settings, x)
}

fn burst_gap(segments: &[(bool, f64)], rate: u32) -> Vec<f32> {
    stimulus::burst_gap(1000.0, BURST_DBFS, segments, f64::from(rate))
}

/// Hang time in host samples at `rate`.
fn h_samples(rate: u32) -> f64 {
    hang_s(&VasSettings::DEVICE) * f64::from(rate)
}

fn within_1ms(got: f64, want: f64, rate: u32) -> bool {
    (got - want).abs() <= 0.001 * f64::from(rate) // ±1 ms, engineering target (003 FR-017)
}

/// Start of stream, reset and reconfigure (A-025): each keeps exactly H of silence, then pauses.
#[test]
fn start_reset_and_reconfigure_keep_one_hang_time() {
    for rate in RATES {
        let s = configs::settings("vas_only", rate);
        let silence = vec![0.0f32; 3 * rate as usize];
        let loud = burst_gap(&[(true, 0.5)], rate);
        let fresh = run(s, &silence);
        assert_eq!(fresh.y.len() as f64, h_samples(rate), "{rate} Hz fresh");
        assert!(fresh.info.paused && fresh.events.is_empty());
        for how in ["reset", "reconfigure"] {
            let mut p = Pipeline::new(s).unwrap();
            let mut warm = loud.clone();
            let _ = p.process_in_place(&mut warm);
            match how {
                "reset" => p.reset(),
                _ => p.reconfigure(s).unwrap(),
            }
            let mut y = vec![0.0f32; silence.len()];
            let mut ev = vec![VasEvent::default(); p.max_events(silence.len())];
            let info = p.process_with_events(&silence, &mut y, &mut ev).unwrap();
            assert_eq!(info.produced as f64, h_samples(rate), "{rate} Hz {how}");
            assert!(info.paused && info.events == 0, "{rate} Hz {how}");
            assert!(y[..info.produced].iter().all(|v| v.to_bits() == 0));
        }
    }
}

/// Everything dropped: a block entirely inside a pause produces nothing; in mute mode, a full
/// block of zeros.
#[test]
fn a_block_inside_a_pause_produces_nothing_or_zeros() {
    for rate in RATES {
        for (config, want_len) in [("vas_only", 0usize), ("vas_mute", 1000)] {
            let s = configs::settings(config, rate);
            let mut p = Pipeline::new(s).unwrap();
            let silence = vec![0.0f32; 2 * rate as usize]; // two hang times: paused
            let mut y = vec![0.0f32; silence.len()];
            let _ = p.process(&silence, &mut y).unwrap();
            let block = vec![0.0f32; 1000];
            let mut out = vec![1.0f32; 1000];
            let info = p.process(&block, &mut out).unwrap();
            assert_eq!(info.produced, want_len, "{rate} Hz {config}");
            assert!(info.paused);
            assert!(out[..info.produced].iter().all(|v| v.to_bits() == 0));
        }
    }
}

/// Sounds shorter than the onset time (including onset − 2 ms) leave no output and no splice,
/// and don't resume recording in the silence after them.
#[test]
fn short_sounds_leave_nothing() {
    for rate in RATES {
        let s = configs::settings("vas_only", rate);
        let x = stimulus::short_bursts(
            1000.0,
            BURST_DBFS,
            2.0,
            &[10.0, 18.0, 19.5],
            2.0,
            f64::from(rate),
        );
        let r = run(s, &x);
        assert!(r.events.is_empty(), "{rate} Hz: {:?}", r.events);
        assert_eq!(r.y.len() as f64, h_samples(rate), "{rate} Hz");
        assert!(r.info.paused);
    }
}

/// A gap exactly at the hang time (within one device sample) does not pause. The second
/// burst starts at a zero crossing, so its first device sample is not a sound sample: the
/// stimulus gap is one device sample shorter than H to make the silent run exactly H.
#[test]
fn gap_exactly_the_hang_time_is_kept() {
    for rate in RATES {
        let s = configs::settings("vas_only", rate);
        let gap = hang_s(&s.vas) - 1.0 / 8000.0;
        let x = burst_gap(&[(true, 0.5), (false, gap), (true, 0.5)], rate);
        let r = run(s, &x);
        assert!(r.events.is_empty(), "{rate} Hz: {:?}", r.events);
        assert_eq!(r.y.len(), x.len(), "{rate} Hz");
    }
}

/// Level hovering at the threshold ± 0.5 dB with random dropouts: at most one pause per
/// hang time + 2 device samples.
#[test]
fn hovering_level_pauses_at_most_once_per_hang_time() {
    for rate in RATES {
        let fs = f64::from(rate);
        let mut s = configs::settings("vas_only", rate);
        s.vas.hang_ms = 50.0; // many pauses in a short run; H = 400 device samples
        let secs = 20.0;
        let n = (secs * fs) as usize;
        let mut rng = stimulus::Pcg32::new(0x0D60, 7);
        let mut x: Vec<f32> = stimulus::step(1000.0, &[-18.0], &[secs], fs);
        // Random ±0.5 dB segments and dropouts, 20 ms each (engineering target, 003 R-15).
        let seg = (0.02 * fs) as usize;
        for chunk in x.chunks_mut(seg) {
            let g = match rng.below(4) {
                0 => 0.0,
                1 => stimulus::amplitude(0.5),
                2 => stimulus::amplitude(-0.5),
                _ => 1.0,
            } as f32;
            for v in chunk {
                *v *= g;
            }
        }
        let r = run(s, &x[..n]);
        let spacing = (hang_s(&s.vas) + 2.0 / 8000.0) * fs;
        let max_pauses = (n as f64 / spacing).ceil() as usize + 1;
        assert!(
            r.events.len() <= max_pauses,
            "{rate} Hz: {} splices in {secs} s, at most {max_pauses}",
            r.events.len()
        );
        for w in r.events.windows(2) {
            assert!(
                (w[1].output_position - w[0].output_position) as f64 >= spacing - 1.0,
                "{rate} Hz: splices {} and {} closer than H + 2 device samples",
                w[0].output_position,
                w[1].output_position
            );
        }
        assert!(r.events.len() > 10, "{rate} Hz: hovering stimulus too tame");
    }
}

/// Non-finite input (NaN, ±Inf) gives decisions and output identical to 0.0 in its place.
#[test]
fn non_finite_input_acts_as_zero() {
    for rate in RATES {
        let s = configs::settings("vas_only", rate);
        let clean = burst_gap(&[(true, 0.5), (false, 1.5), (true, 0.5)], rate);
        let mut dirty = clean.clone();
        let n = rate as usize;
        dirty[n + 10] = f32::NAN;
        dirty[n + 20] = f32::INFINITY;
        dirty[n + 30] = f32::NEG_INFINITY;
        let mut zeroed = clean.clone();
        for i in [n + 10, n + 20, n + 30] {
            zeroed[i] = 0.0;
        }
        let a = run(s, &dirty);
        let b = run(s, &zeroed);
        assert_eq!(a.y, b.y, "{rate} Hz");
        assert_eq!(a.events, b.events, "{rate} Hz");
        assert_eq!(a.info, b.info, "{rate} Hz");
        assert!(a.y.iter().all(|v| v.is_finite()));
    }
}

/// Long silence (release only): 1 hour at 8 kHz gives H zeros, then nothing, block by block.
#[test]
#[ignore = "slow: one hour of silence; run in release mode"]
fn one_hour_of_silence_gives_one_hang_time() {
    let s = configs::settings("vas_only", 8000);
    let mut p = Pipeline::new(s).unwrap();
    let block = vec![0.0f32; 8000];
    let mut y = vec![0.0f32; 8000];
    let mut total = 0usize;
    for second in 0..3600 {
        let info = p.process(&block, &mut y).unwrap();
        total += info.produced;
        assert_eq!(info.events, 0);
        if second > 0 {
            assert_eq!(info.produced, 0, "second {second}");
            assert!(info.paused);
        }
    }
    assert_eq!(total, 8000);
}

/// Long sound (release only): a 10-minute tone at the threshold + 10 dB keeps every block in
/// full, with no event.
#[test]
#[ignore = "slow: ten minutes of tone; run in release mode"]
fn ten_minutes_of_tone_is_kept_in_full() {
    let s = configs::settings("vas_only", 8000);
    let mut p = Pipeline::new(s).unwrap();
    let x = stimulus::step(1000.0, &[BURST_DBFS], &[600.0], 8000.0);
    let mut y = vec![0.0f32; 8000];
    for (i, chunk) in x.chunks(8000).enumerate() {
        let info = p.process(chunk, &mut y[..chunk.len()]).unwrap();
        assert_eq!(info.produced, chunk.len(), "block {i}");
        assert_eq!(info.events, 0);
        assert!(!info.paused);
    }
}

/// Tap before the VAS: "after AGC" and "after record stage" give fixed length, and the VAS
/// settings have no effect.
#[test]
fn taps_before_the_vas_are_fixed_length() {
    for rate in RATES {
        for tap in [Tap::AfterAgc, Tap::AfterRecord] {
            let x = burst_gap(&[(true, 0.5), (false, 3.0), (true, 0.5)], rate);
            let mut a = Settings::new(rate);
            a.tap = tap;
            let mut b = a;
            b.vas.sensitivity = 5;
            b.vas.hang_ms = 50.0;
            b.vas.mode = VasMode::Mute;
            let (ra, rb) = (run(a, &x), run(b, &x));
            assert_eq!(ra.y.len(), x.len(), "{rate} Hz {tap:?}");
            assert!(ra.events.is_empty() && !ra.info.paused);
            assert_eq!(
                ra.y, rb.y,
                "{rate} Hz {tap:?}: VAS settings changed the output"
            );
            assert_eq!(ra.info, rb.info);
        }
    }
}

/// Mute mode with stage 10 on: the output keeps its length, and the stage's ring-out at a
/// region's start is allowed (the region is not required to be exact zeros there).
#[test]
fn mute_with_playback_stage_keeps_length_and_may_ring() {
    for rate in RATES {
        let mut s = Settings::new(rate);
        s.agc.enabled = false;
        s.record_stage_enabled = false;
        s.vas.mode = VasMode::Mute; // stage 10 on, tap after playback
        let x = burst_gap(&[(true, 1.0), (false, 5.0), (true, 1.0)], rate);
        let r = run(s, &x);
        assert_eq!(r.y.len(), x.len(), "{rate} Hz");
        assert_eq!(r.events.len(), 1, "{rate} Hz");
        let (start, len) = (
            r.events[0].output_position as usize,
            r.events[0].input_length as usize,
        );
        // Well inside the region, after the ring-out and the conversion's memory, it is silent
        // to within the filter's tail (engineering target: 1e-6 after 0.2 s, 003 R-15).
        let settled = start + (0.2 * f64::from(rate)) as usize;
        assert!(
            r.y[settled..start + len].iter().all(|v| v.abs() < 1e-6),
            "{rate} Hz: region not quiet after the ring-out"
        );
        assert!(
            within_1ms(
                len as f64,
                (5.0 - 1.0 + onset_s(&s.vas)) * f64::from(rate),
                rate
            ),
            "{rate} Hz: region length {len}"
        );
    }
}

/// The AGC keeps running while the VAS is paused (FR-016, A-025). At 8 kHz, where the rate
/// conversion is the identity, every kept sample is bit-identical to the VAS-bypassed output at
/// its input index (tap after VAS, stage 4 off). At 48 kHz the interpolator's memory differs
/// after each splice, so the comparison is made only before the first splice.
#[test]
fn agc_runs_while_paused() {
    for rate in RATES {
        let mut s = Settings::new(rate);
        s.record_stage_enabled = false;
        s.tap = Tap::AfterVas;
        let x = burst_gap(
            &[
                (true, 1.0),
                (false, 4.0),
                (true, 1.0),
                (false, 3.0),
                (true, 0.5),
            ],
            rate,
        );
        let r = run(s, &x);
        assert_eq!(r.events.len(), 2, "{rate} Hz: {:?}", r.events);
        let mut off = s;
        off.vas.enabled = false;
        let reference = run(off, &x).y;
        if rate == 8000 {
            let mut removed = 0usize;
            let mut next = 0;
            for (n, v) in r.y.iter().enumerate() {
                while next < r.events.len() && r.events[next].output_position as usize == n {
                    removed += r.events[next].input_length as usize;
                    next += 1;
                }
                assert_eq!(
                    v.to_bits(),
                    reference[n + removed].to_bits(),
                    "8 kHz: output {n}"
                );
            }
        } else {
            let first = r.events[0].output_position as usize;
            assert_eq!(
                &r.y[..first],
                &reference[..first],
                "{rate} Hz: before the first splice"
            );
        }
    }
}

/// FR-005: in mute mode each region is reported once, by the block in which it ends: for every
/// event, `output_position + input_length` lies within that block's output range.
#[test]
fn mute_regions_are_reported_by_the_block_that_ends_them() {
    for rate in RATES {
        let s = configs::settings("vas_mute", rate);
        let x = burst_gap(
            &[
                (true, 1.0),
                (false, 5.0),
                (true, 0.5),
                (false, 3.0),
                (true, 0.5),
            ],
            rate,
        );
        let mut p = Pipeline::new(s).unwrap();
        let mut rng = stimulus::Pcg32::new(0x0D60, 9);
        let (mut pos, mut produced_before, mut total_events) = (0usize, 0usize, 0usize);
        while pos < x.len() {
            let n = (rng.below(4001) as usize).min(x.len() - pos);
            let mut y = vec![0.0f32; n];
            let mut ev = vec![VasEvent::default(); p.max_events(n)];
            let info = p
                .process_with_events(&x[pos..pos + n], &mut y, &mut ev)
                .unwrap();
            assert_eq!(info.produced, n, "{rate} Hz: mute mode keeps the length");
            for e in &ev[..info.events] {
                let end = (e.output_position + e.input_length) as usize;
                assert!(
                    end > produced_before && end <= produced_before + info.produced,
                    "{rate} Hz: region ending at {end} reported by block {produced_before}..{}",
                    produced_before + info.produced
                );
            }
            total_events += info.events;
            produced_before += info.produced;
            pos += n;
        }
        assert_eq!(total_events, 2, "{rate} Hz");
    }
}
