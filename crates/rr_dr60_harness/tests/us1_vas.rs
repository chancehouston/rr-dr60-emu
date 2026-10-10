//! Spec 003 User Story 1: hear the RR-DR60's voice-activated recording (tasks.md T019).
//!
//! Acceptance scenarios AS1–AS5 at 48 kHz in the VAS-isolated configuration (`vas_only`: AGC
//! and stages 4 and 10 bypassed), plus FR-011. Plain assertions on output lengths and events
//! only: this story does not use the US3 harness (`vas_checks`), per plan › Story boundaries.

use rr_dr60::{BlockInfo, Pipeline, SUPPORTED_HOST_RATES, Settings, Tap, VasEvent};
use rr_dr60_harness::{configs, stimulus};

const FS: u32 = 48_000;
/// The burst level: the default threshold (−18 dBFS, A-022) + 10 dB (spec 003 definitions).
const BURST_DBFS: f64 = -8.0;

/// Processes `x` in one block; returns the output, its events and the block result.
fn run(settings: Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, BlockInfo) {
    let mut p = Pipeline::new(settings).unwrap();
    let mut y = vec![0.0f32; x.len()];
    let mut ev = vec![VasEvent::default(); p.max_events(x.len())];
    let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
    y.truncate(info.produced);
    ev.truncate(info.events);
    (y, ev, info)
}

fn burst_gap(segments: &[(bool, f64)]) -> Vec<f32> {
    stimulus::burst_gap(1000.0, BURST_DBFS, segments, f64::from(FS))
}

/// US1 AS1 (FR-006, A-022): a 1 kHz tone 3 dB above the threshold is kept in full for 10 s.
#[test]
fn as1_tone_above_threshold_is_kept() {
    let x = stimulus::step(1000.0, &[-15.0], &[10.0], f64::from(FS));
    let (y, ev, _) = run(configs::settings("vas_only", FS), &x);
    assert_eq!(y.len(), x.len());
    assert!(ev.is_empty());
}

/// US1 AS2 (FR-004, FR-008, FR-009; A-023, A-024): burst 1 s, gap 5 s, burst 1 s keeps the first
/// burst, 1.0 s of the gap and the second burst minus 20 ms, joined at one splice (±1 ms).
#[test]
fn as2_long_gap_is_cut_to_the_hang_time() {
    let x = burst_gap(&[(true, 1.0), (false, 5.0), (true, 1.0)]);
    let (y, ev, info) = run(configs::settings("vas_only", FS), &x);
    let want = (2.0 + 1.0 - 0.020) * f64::from(FS);
    let tol = 0.001 * f64::from(FS); // ±1 ms, engineering target (003 FR-017)
    eprintln!(
        "US1 AS2: {} of {} samples kept (want {want} ± {tol}), splice {:?}",
        y.len(),
        x.len(),
        ev
    );
    assert!((y.len() as f64 - want).abs() <= tol, "{} samples", y.len());
    assert_eq!(ev.len(), 1);
    assert!(!info.paused);
    // The removed length accounts for everything that is missing (m/l = 6 is an integer).
    assert_eq!(ev[0].input_length as usize + y.len(), x.len());
}

/// US1 AS3 (FR-008): a gap shorter than the hang time is kept: no splice, nothing removed.
#[test]
fn as3_short_gap_is_kept() {
    let x = burst_gap(&[(true, 1.0), (false, 0.9), (true, 1.0)]);
    let (y, ev, _) = run(configs::settings("vas_only", FS), &x);
    assert_eq!(y.len(), x.len());
    assert!(ev.is_empty());
}

/// US1 AS4 (FR-007 of 002 style; A-025): digital silence gives exactly 1.0 s of zeros, then
/// nothing; the block ends paused.
#[test]
fn as4_silence_keeps_one_hang_time_then_pauses() {
    let x = vec![0.0f32; 5 * FS as usize];
    let (y, ev, info) = run(configs::settings("vas_only", FS), &x);
    assert_eq!(y.len(), FS as usize);
    assert!(y.iter().all(|v| v.to_bits() == 0));
    assert!(ev.is_empty());
    assert!(info.paused);
}

/// US1 AS5 (FR-004, FR-005, FR-014): in random block sizes (including 0 and 1), no block
/// produces more than it consumed, and the concatenated output and events equal the one-block
/// run, bit for bit.
#[test]
fn as5_blocks_report_their_output_and_splices() {
    let x = burst_gap(&[
        (true, 0.5),
        (false, 2.0),
        (true, 0.3),
        (false, 1.5),
        (true, 0.4),
        (false, 0.5),
    ]);
    let settings = configs::settings("vas_only", FS);
    let (want_y, want_ev, _) = run(settings, &x);
    assert_eq!(want_ev.len(), 2);

    let mut p = Pipeline::new(settings).unwrap();
    let (mut y, mut ev) = (Vec::new(), Vec::new());
    let mut seed: u64 = 0x0D60;
    let mut pos = 0;
    while pos < x.len() {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let n = match seed >> 61 {
            0 => 0,
            1 => 1,
            _ => ((seed >> 33) % 3000) as usize,
        }
        .min(x.len() - pos);
        let mut out = vec![0.0f32; n];
        let mut slots = vec![VasEvent::default(); p.max_events(n)];
        let info = p
            .process_with_events(&x[pos..pos + n], &mut out, &mut slots)
            .unwrap();
        assert!(info.produced <= n);
        assert!(info.events <= slots.len());
        y.extend_from_slice(&out[..info.produced]);
        ev.extend_from_slice(&slots[..info.events]);
        pos += n;
    }
    assert_eq!(y, want_y);
    assert_eq!(ev, want_ev);
}

/// 003 FR-011: the VAS adds no latency, for every tap at every rate.
#[test]
fn fr011_no_added_latency() {
    for rate in SUPPORTED_HOST_RATES {
        for tap in [
            Tap::AfterAgc,
            Tap::AfterRecord,
            Tap::AfterVas,
            Tap::AfterPlayback,
        ] {
            let mut on = Settings::new(rate);
            on.tap = tap;
            let mut off = on;
            off.vas.enabled = false;
            assert_eq!(
                Pipeline::new(on).unwrap().latency_samples(),
                Pipeline::new(off).unwrap().latency_samples(),
                "{rate} Hz {tap:?}"
            );
        }
    }
}
