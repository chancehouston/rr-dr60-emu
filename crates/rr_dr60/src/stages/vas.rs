//! VAS stage: signal-chain stage 5 (spec 003; S-001, A-008, A-021 – A-025).
//!
//! The owner's manual (S-001) says recording "automatically pauses when no sound is detected".
//! This stage decides, for each device-rate sample, whether it is kept, dropped (paused), or is
//! the first sample after a pause (a splice). It follows specs/003-vas/data-model.md ›
//! Per-sample processing and research.md R-02 – R-04:
//!
//! - **Detector** (A-022, R-02): a sample is "sound" when |x| ≥ A_thr. A sine's peak equals its
//!   AES17 level, so a sample peak compares directly against the sine-level threshold. The
//!   VAS input is the clamped record-stage output (spec 005 FR-008); below the limiter's knee
//!   the limiter is the identity, so the threshold (−40 dBFS at level 3, A-022 revised by spec
//!   005) is input-referred one to one (005 FR-012).
//! - **Pause** (A-023): after more than H consecutive non-sound samples. The first H are kept.
//! - **Resume** (A-024): on a sound sample, once a "sound run" has lasted O + 1 samples. A run
//!   continues across gaps of at most W samples between sound samples, which bridges the troughs
//!   of every in-band sine. The first O samples of the run are lost (no pre-roll).
//! - **Start** (A-025): recording.
//!
//! Integer state only, plus `detmath::exp` at construction, so it is bit-identical on every
//! platform. Constant work per sample, no allocation.

use core::f64::consts::LN_10;

use rr_dr60_detmath::exp;

use crate::settings::{DEVICE_RATE_HZ, VasSettings};

/// Longest gap, in device samples, between sound samples that still continues a sound run:
/// 32 samples = 5.3 ms at 6 kHz, longer than the half period of the lowest in-band tone
/// (225 Hz: 13.3 samples). The invariant is the sample count, not the time (engineering target,
/// 003 R-15; restated by spec 004 research R-10).
pub(crate) const W: u64 = 32;
/// Threshold change per sensitivity level, in dB (A-022).
const DB_PER_LEVEL: f64 = 3.0;
/// The factory sensitivity level that `threshold_dbfs` refers to (S-001, A-021).
const REFERENCE_LEVEL: f64 = 3.0;

/// What happens to one device sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VasDecision {
    /// Recording: the sample is kept.
    Keep,
    /// Paused: the sample is dropped (or muted, in mute mode).
    Drop,
    /// The first sample kept after a pause: the splice point.
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Recording,
    Paused,
}

/// One VAS instance. All state is inline (FR-014).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VasStage {
    /// Linear sound threshold A_thr (A-022).
    threshold: f64,
    /// Hang time H in device samples (A-023).
    hang: u64,
    /// Onset time O in device samples (A-024).
    onset: u64,
    state: State,
    /// Consecutive non-sound samples while recording.
    silent: u64,
    /// Length of the current sound run while paused (0 = none).
    run: u64,
    /// Samples since the last sound sample while paused; saturates at W + 1.
    since_sound: u64,
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
}

/// `ms` milliseconds in whole device samples, rounded to nearest (engineering target, 003 R-15).
fn device_samples(ms: f32) -> u64 {
    (f64::from(ms) * f64::from(DEVICE_RATE_HZ) / 1000.0 + 0.5) as u64
}

impl VasStage {
    /// A stage for validated `settings`, in the recording state (A-025).
    pub(crate) fn new(settings: &VasSettings) -> Self {
        let level_db = f64::from(settings.threshold_dbfs)
            + DB_PER_LEVEL * (REFERENCE_LEVEL - f64::from(settings.sensitivity));
        Self {
            threshold: exp(level_db * LN_10 / 20.0),
            hang: device_samples(settings.hang_ms),
            onset: device_samples(settings.onset_ms),
            state: State::Recording,
            silent: 0,
            run: 0,
            since_sound: W + 1,
            #[cfg(feature = "op-count")]
            ops: 0,
        }
    }

    /// Hang time H in device samples.
    pub(crate) fn hang_samples(&self) -> u64 {
        self.hang
    }

    /// The VAS is paused.
    pub(crate) fn is_paused(&self) -> bool {
        self.state == State::Paused
    }

    /// Spec 016 FR-002 (h): the further non-sound device samples the gate will still keep
    /// before pausing: `H − silent` while recording (0 exactly at the last kept non-sound
    /// sample, since the gate keeps while `silent ≤ H`; the next non-sound sample pauses it),
    /// and 0 while paused (the counter is frozen then).
    pub(crate) fn hang_remaining(&self) -> u64 {
        match self.state {
            State::Recording => self.hang.saturating_sub(self.silent),
            State::Paused => 0,
        }
    }

    /// Decides one device-rate sample (data-model.md › Per-sample processing).
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> VasDecision {
        #[cfg(feature = "op-count")]
        {
            // One compare, a few counter updates and branches.
            self.ops += 4;
        }
        let sound = x.abs() >= self.threshold;
        match self.state {
            State::Recording => {
                if sound {
                    self.silent = 0;
                } else {
                    self.silent += 1;
                }
                if self.silent <= self.hang {
                    VasDecision::Keep
                } else {
                    self.state = State::Paused;
                    self.run = 0;
                    self.since_sound = W + 1;
                    VasDecision::Drop
                }
            }
            State::Paused => {
                if sound {
                    self.run = if self.run > 0 && self.since_sound <= W {
                        self.run + 1
                    } else {
                        1
                    };
                    self.since_sound = 0;
                } else {
                    self.since_sound = (self.since_sound + 1).min(W + 1);
                    if self.since_sound > W {
                        self.run = 0;
                    } else if self.run > 0 {
                        self.run += 1;
                    }
                }
                // Resume only on a sound sample (plan review, finding 1).
                if sound && self.run > self.onset {
                    self.state = State::Recording;
                    self.silent = 0;
                    VasDecision::Resume
                } else {
                    VasDecision::Drop
                }
            }
        }
    }

    /// Returns to the freshly created state: recording, every counter cleared (A-025).
    pub(crate) fn reset(&mut self) {
        self.state = State::Recording;
        self.silent = 0;
        self.run = 0;
        self.since_sound = W + 1;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::settings::VasSettings;
    use rr_dr60_detmath::{TAU, ln, sin};
    use std::vec::Vec;

    const FS: f64 = 6000.0; // A-026: the VAS runs at the device rate (003 R-01; 004 R-01)
    const H: usize = 6000; // A-023: 1.0 s
    const O: usize = 120; // A-024: 20 ms

    fn amp(dbfs: f64) -> f64 {
        exp(dbfs * ln(10.0) / 20.0)
    }

    /// `n` samples of a tone at `dbfs` with a half-sample phase offset (spec 004 research R-08):
    /// at 1 kHz a cycle is exactly 6 samples, and the offset puts samples at 30°, 90°, 150° …,
    /// so a sample lands on the true peak. At phase 0 the peak would read 1.25 dB low.
    fn tone(freq: f64, dbfs: f64, n: usize) -> Vec<f64> {
        (0..n)
            .map(|k| amp(dbfs) * sin(TAU * ((freq * (k as f64 + 0.5) / FS) % 1.0)))
            .collect()
    }

    /// `n` samples of a 1 kHz tone at −30 dBFS (threshold + 10 dB; restated from −8 dBFS by
    /// spec 005 FR-012) with a half-sample phase offset, so every sample is a sound sample:
    /// |sin| ≥ sin(30°) = 0.5, and 0.5 · 0.0316 > 0.01 = A_thr. Burst lengths are then exact
    /// sound-run lengths.
    fn loud(n: usize) -> Vec<f64> {
        (0..n)
            .map(|k| amp(-30.0) * sin(TAU * (((k % 6) as f64 + 0.5) / 6.0)))
            .collect()
    }

    fn silence(n: usize) -> Vec<f64> {
        std::vec![0.0; n]
    }

    fn cat(parts: &[Vec<f64>]) -> Vec<f64> {
        parts.concat()
    }

    fn run(stage: &mut VasStage, x: &[f64]) -> Vec<VasDecision> {
        x.iter().map(|&v| stage.process(v)).collect()
    }

    fn kept(d: &[VasDecision]) -> usize {
        d.iter().filter(|&&v| v != VasDecision::Drop).count()
    }

    fn resumes(d: &[VasDecision]) -> usize {
        d.iter().filter(|&&v| v == VasDecision::Resume).count()
    }

    fn with(f: impl Fn(&mut VasSettings)) -> VasStage {
        let mut s = VasSettings::DEVICE;
        f(&mut s);
        VasStage::new(&s)
    }

    /// The lowest level (bisection to 0.05 dB) at which `freq` is kept in full for 2 s
    /// (longer than H, so a level below threshold would pause).
    fn lowest_kept(freq: f64, settings: impl Fn(&mut VasSettings) + Copy) -> f64 {
        let (mut lo, mut hi) = (-60.0, 0.0); // the −60 … 0 dBFS setting range (003 FR-012)
        while hi - lo > 0.05 {
            let mid = 0.5 * (lo + hi);
            let mut stage = with(settings);
            let d = run(&mut stage, &tone(freq, mid, 12_000));
            if kept(&d) == d.len() {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// 003 FR-006 (A-022; the default restated to −40 dBFS by spec 005 FR-012): +3 dB is kept
    /// for 10 s; −3 dB keeps exactly H, then drops. 225 and 2550 Hz (the band edges at 6 kHz,
    /// 004 FR-012) have the same threshold as 1 kHz within ±1 dB.
    #[test]
    fn threshold_keeps_above_and_drops_below() {
        let mut stage = VasStage::new(&VasSettings::DEVICE);
        let d = run(&mut stage, &tone(1000.0, -37.0, 60_000));
        assert_eq!(kept(&d), 60_000);
        assert!(!stage.is_paused());

        let mut stage = VasStage::new(&VasSettings::DEVICE);
        let d = run(&mut stage, &tone(1000.0, -43.0, 15_000));
        assert_eq!(kept(&d), H);
        assert!(d[..H].iter().all(|&v| v == VasDecision::Keep));
        assert!(stage.is_paused());

        let t1k = lowest_kept(1000.0, |_| {});
        // 0.05 dB: the bisection resolution, engineering target (003 R-15); −40 dBFS: A-022
        // (revised by spec 005).
        assert!((t1k + 40.0).abs() <= 0.05, "1 kHz threshold {t1k}");
        for f in [225.0, 2550.0] {
            let t = lowest_kept(f, |_| {});
            assert!((t - t1k).abs() <= 1.0, "{f} Hz: {t} vs {t1k}"); // ±1 dB, 003 FR-006
        }
    }

    /// 003 FR-007 (A-022; restated by spec 005 FR-012): 3 dB per level; level 1 is −34 dBFS
    /// and level 5 is −46 dBFS.
    #[test]
    fn sensitivity_moves_the_threshold_3_db_per_level() {
        let t1 = lowest_kept(1000.0, |s| s.sensitivity = 1);
        let t5 = lowest_kept(1000.0, |s| s.sensitivity = 5);
        // ±0.05 dB bisection (003 R-15); −34 / −46 dBFS: 3 dB per level, A-022 (revised).
        assert!((t1 + 34.0).abs() <= 0.05, "level 1: {t1}");
        assert!((t5 + 46.0).abs() <= 0.05, "level 5: {t5}");
    }

    /// 003 FR-008 (A-023): a gap of g silent samples keeps min(g, H) of it; g ≤ H never pauses.
    #[test]
    fn hang_time_keeps_exactly_h() {
        for gap in [H - 40, H, H + 1, H + 40] {
            let mut stage = VasStage::new(&VasSettings::DEVICE);
            let x = cat(&[loud(600), silence(gap), loud(600)]);
            let d = run(&mut stage, &x);
            let dropped = x.len() - kept(&d);
            if gap <= H {
                assert_eq!(dropped, 0, "gap {gap}");
                assert_eq!(resumes(&d), 0);
            } else {
                // The gap beyond H, plus the onset of the second burst.
                assert_eq!(dropped, gap - H + O, "gap {gap}");
                assert_eq!(resumes(&d), 1);
            }
        }
    }

    /// 003 FR-009 (A-024; plan review finding 1): bursts of ≤ O samples after a pause never
    /// resume, not even in the silence after them; longer bursts keep exactly burst − O.
    #[test]
    fn onset_time_and_short_bursts() {
        // O − 60, O − 12 (onset − 2 ms), O − 1, O, O + 1, 1.5 O, 50 O (003 FR-009, at 6 kHz).
        for burst in [60, 108, 119, 120, 121, 180, 6000] {
            let mut stage = VasStage::new(&VasSettings::DEVICE);
            let x = cat(&[silence(H + 100), loud(burst), silence(3000)]);
            let d = run(&mut stage, &x);
            let after_lead = &d[H + 100..];
            let kept_burst = kept(&after_lead[..burst]);
            let kept_after = kept(&after_lead[burst..]);
            if burst <= O {
                assert_eq!((kept_burst, kept_after), (0, 0), "burst {burst}");
                assert_eq!(resumes(&d), 0, "burst {burst}");
            } else {
                assert_eq!(kept_burst, burst - O, "burst {burst}");
                assert_eq!(after_lead[O], VasDecision::Resume, "burst {burst}");
                assert_eq!(kept_after, 3000, "the trailing silence is within H");
            }
        }
    }

    /// 003 FR-009: onset 0 keeps the first sound sample; with onset 2 ms a click doesn't resume.
    #[test]
    fn onset_zero_and_single_clicks() {
        let mut stage = with(|s| s.onset_ms = 0.0);
        let x = cat(&[silence(H + 10), loud(1), silence(100)]);
        let d = run(&mut stage, &x);
        assert_eq!(d[H + 10], VasDecision::Resume);

        let mut stage = with(|s| s.onset_ms = 2.0);
        let d = run(&mut stage, &x);
        assert_eq!(resumes(&d), 0);
    }

    /// 003 R-02: a 225 Hz tone (the lowest in-band tone at 6 kHz) at threshold + 3 dB resumes
    /// exactly O samples after its first sound sample, because W bridges its troughs; gaps longer
    /// than W restart the run.
    #[test]
    fn w_bridges_troughs_and_longer_gaps_restart_the_run() {
        let mut stage = VasStage::new(&VasSettings::DEVICE);
        let x = cat(&[silence(H + 10), tone(225.0, -37.0, 3000)]); // threshold + 3 dB
        let first_sound = x[H + 10..]
            .iter()
            .position(|v| v.abs() >= stage.threshold)
            .unwrap();
        let d = run(&mut stage, &x);
        let resume = d.iter().position(|&v| v == VasDecision::Resume).unwrap();
        assert_eq!(resume, H + 10 + first_sound + O);

        for (gap, want) in [(33, 0), (32, 1)] {
            let mut stage = VasStage::new(&VasSettings::DEVICE);
            let mut x = silence(H + 10);
            for _ in 0..4 {
                x.extend(loud(100));
                x.extend(silence(gap));
            }
            let d = run(&mut stage, &x);
            assert_eq!(resumes(&d), want, "gap {gap}");
        }
    }

    /// 003 A-025, FR-014: a fresh stage keeps H samples of silence, then pauses; reset returns to
    /// the fresh state.
    #[test]
    fn starts_recording_and_reset_equals_fresh() {
        let mut stage = VasStage::new(&VasSettings::DEVICE);
        let d = run(&mut stage, &silence(H + 500));
        assert_eq!(kept(&d), H);

        let x = cat(&[loud(3000), silence(H + 300), loud(500), silence(200)]);
        let mut used = VasStage::new(&VasSettings::DEVICE);
        run(&mut used, &x);
        used.reset();
        let mut fresh = VasStage::new(&VasSettings::DEVICE);
        // Compare behavior, not the struct: the op-count feature's counter is not reset.
        assert_eq!(
            (used.state, used.silent, used.run, used.since_sound),
            (fresh.state, fresh.silent, fresh.run, fresh.since_sound)
        );
        assert_eq!(run(&mut used, &x), run(&mut fresh, &x));
    }

    /// Plan review finding 16: `since_sound` saturates in long silence (10 minutes).
    #[test]
    fn counters_saturate_in_long_silence() {
        let mut stage = VasStage::new(&VasSettings::DEVICE);
        for _ in 0..(600.0 * FS) as usize {
            stage.process(0.0);
        }
        assert!(stage.is_paused());
        assert_eq!(stage.since_sound, W + 1);
        assert_eq!(stage.hang_samples(), H as u64);
    }
}

/// Spec 016 FR-002 (h): the hang countdown.
#[cfg(test)]
mod meters_tests {
    use super::*;
    use crate::settings::VasSettings;

    /// H non-sound samples read H … 0 while every one is kept; the next pauses the gate and
    /// reads 0; a sound sample restores H.
    #[test]
    fn hang_remaining_counts_down_and_is_zero_while_paused() {
        let mut s = VasSettings::DEVICE;
        s.hang_ms = 10.0; // H = 60 device samples
        let mut stage = VasStage::new(&s);
        let h = stage.hang_samples();
        assert_eq!(h, 60);
        assert_eq!(stage.hang_remaining(), h, "rest: the full hang");
        assert_eq!(stage.process(0.5), VasDecision::Keep);
        assert_eq!(stage.hang_remaining(), h, "a sound sample: the full hang");
        for k in 1..=h {
            assert_eq!(stage.process(0.0), VasDecision::Keep, "sample {k} is kept");
            assert_eq!(stage.hang_remaining(), h - k);
        }
        assert_eq!(
            stage.hang_remaining(),
            0,
            "0 at the last kept non-sound sample"
        );
        assert!(!stage.is_paused());
        assert_eq!(
            stage.process(0.0),
            VasDecision::Drop,
            "the next non-sound sample pauses"
        );
        assert!(stage.is_paused());
        assert_eq!(stage.hang_remaining(), 0, "0 while paused");
        // A sound run longer than the onset resumes; the countdown is the full hang again.
        let onset = s.onset_ms as u64 * 6;
        let mut resumed = false;
        for _ in 0..=onset + 1 {
            if stage.process(0.5) == VasDecision::Resume {
                resumed = true;
            }
        }
        assert!(resumed);
        assert_eq!(stage.hang_remaining(), h);
    }
}
