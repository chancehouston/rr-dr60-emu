//! Spec 016 Edge Cases (tasks.md T008): the rest state, zero-length calls, the skipped flush,
//! the tap table, stage 10 off, the bypassed chain, mute mode, reset and reconfigure, and the
//! read's purity.

use rr_dr60::{Meters, Pipeline, Route, Settings, Tap, VasEvent};
use rr_dr60_harness::{configs, meters_checks as mc, stimulus};

fn voiced(rate: u32) -> Vec<f32> {
    stimulus::voiced(f64::from(rate), 0.25, -20.0, 0x0D60)
}

/// Which meters a one-pass call at each tap fills (spec › Edge Cases › Taps in the one-pass).
#[test]
fn tap_table() {
    for &rate in configs::measurement_rates() {
        let x = voiced(rate);
        for (tap, expect) in [
            // (input, limiter input, limiter figures, stage 4, stage 5, reconstruction, route)
            (
                Tap::AfterMicCapsule,
                [true, false, false, false, false, false, false],
            ),
            (
                Tap::AfterMicAmp,
                [true, true, false, false, false, false, false],
            ),
            (
                Tap::AfterAgc,
                [true, true, true, false, false, false, false],
            ),
            (
                Tap::AfterRecord,
                [true, true, true, true, false, false, false],
            ),
            (Tap::AfterVas, [true, true, true, true, true, false, false]),
            (
                Tap::AfterDecoder,
                [true, true, true, true, true, false, false],
            ),
            (
                Tap::AfterPlayback,
                [true, true, true, true, true, true, false],
            ),
            (
                Tap::AfterVolumeAmp,
                [true, true, true, true, true, true, false],
            ),
            (
                Tap::AfterSpeaker,
                [true, true, true, true, true, true, false],
            ),
            (Tap::RouteOutput, [true, true, true, true, true, true, true]),
        ] {
            let mut s = Settings::new(rate);
            s.tap = tap;
            s.vas.hang_ms = 50.0; // a short hang: the stage 5 figures move within 0.25 s
            s.output.clamp_enabled = true;
            let mut p = Pipeline::new(s).unwrap();
            // A loud burst then silence: the limiter charges, the gate counts down.
            let mut loud = stimulus::tone(1000.0, 2.0, f64::from(rate), x.len() / 2);
            loud.extend(std::iter::repeat_n(0.0f32, x.len() / 2));
            let mut y = vec![0.0f32; loud.len()];
            let _ = p.process(&loud, &mut y).unwrap();
            let m = p.meters();
            let rest = p.rest_meters();
            let got = [
                m.input_peak != 0.0,
                m.limiter_input_peak != 0.0,
                m.gain_reduction_db != 0.0 || m.hold_charge != 0.0,
                m.record_filter_peak != 0.0 || m.adc_clamp_count != 0,
                m.hang_remaining_samples != rest.hang_remaining_samples || m.vas_paused,
                m.reconstruction_peak != 0.0,
                m.route_peak != 0.0,
            ];
            assert_eq!(got, expect, "{rate} Hz at {tap:?}: {m:?}");
        }
    }
}

/// The player with stage 10 off still reads the reconstruction peak (D7); the chain bypassed at
/// the route output still reads the route peak and, with the clamp on, the count.
#[test]
fn stage10_off_and_bypassed_chain_still_read() {
    for &rate in configs::measurement_rates() {
        let x = mc::clamp_square(rate, 0.25);
        let mut s = configs::settings_011(rate);
        s.speech_codec_enabled = false;
        s.playback_stage_enabled = false;
        s.output = rr_dr60::OutputSettings::BYPASSED;
        s.output.clamp_enabled = true;
        let mut p = Pipeline::new(s).unwrap();
        p.set_route(Route::Speaker);
        let (plays, _) = mc::record_then_play_blocks(&mut p, &x, 997);
        let first = plays[0].1;
        assert!(first.reconstruction_peak > 0.9, "{rate}: {first:?}");
        assert!(first.route_peak > 0.9, "{rate}: {first:?}");
        // The stored square is clamped to ±1.0 by the converter; the conversion to the host
        // rate rings on its edges, so the bypassed chain's route output can exceed the bound:
        // the count is read iff the pre-bound peak is above 1.0.
        assert_eq!(
            first.output_clamp_count > 0,
            first.route_peak > 1.0,
            "{rate}: {first:?}"
        );
        // The one-pass at the route output with the chain bypassed and a 1.5 sine: counts.
        let mut s2 = configs::settings("output_clamp_only", rate);
        s2.adc_clamp_enabled = false;
        let mut p2 = Pipeline::new(s2).unwrap();
        let sine = stimulus::tone(1000.0, 1.5, f64::from(rate), 2048);
        let mut y = vec![0.0f32; sine.len()];
        let _ = p2.process(&sine, &mut y).unwrap();
        let m = p2.meters();
        assert!(
            m.output_clamp_count > 0 && m.route_peak > 1.0,
            "{rate}: {m:?}"
        );
    }
}

/// A skipped flush (nothing fed) leaves the read-out.
#[test]
fn skipped_flush_leaves_the_read_out() {
    let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
    let x = voiced(48_000);
    let mut y = vec![0.0f32; x.len()];
    let _ = p.process(&x, &mut y).unwrap();
    let before = p.meters();
    p.start_recording().unwrap();
    let out = p.stop_recording(&mut []).unwrap();
    assert_eq!(out.result, rr_dr60::StopResult::DiscardedEmpty);
    assert_eq!(p.meters(), before);
}

/// Mute mode: the muted samples are zeros past the gate; the playback peaks read the
/// reconstruction's ringing, then 0, and the record group keeps reading the gate.
#[test]
fn mute_mode_playback_peaks() {
    for &rate in configs::measurement_rates() {
        let mut s = mc::short_hang(rate, true);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let blocks = mc::one_pass_blocks(&mut p, &mc::vas_burst(rate), 121);
        assert!(
            blocks
                .iter()
                .all(|b| b.output.len() == 121 || b.output.len() < 121)
        );
        let paused: Vec<&mc::Block> = blocks.iter().filter(|b| b.meters.vas_paused).collect();
        assert!(!paused.is_empty(), "{rate}: never paused");
        let last_paused = paused.last().unwrap();
        assert!(
            last_paused.meters.reconstruction_peak < 1e-2,
            "{rate}: {:?}",
            last_paused.meters
        );
    }
}

/// With the speech codec bypassed, a block the gate drops entirely leaves the one-pass's
/// playback group at 0 (no sample reached stage 10); the stage 4 figures are still read.
#[test]
fn dropped_block_playback_group_is_zero_codec_bypassed() {
    for &rate in configs::measurement_rates() {
        let mut s = mc::short_hang(rate, false);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let blocks = mc::one_pass_blocks(&mut p, &mc::vas_burst(rate), 121);
        let dropped: Vec<&mc::Block> = blocks
            .iter()
            .filter(|b| b.output.is_empty() && b.meters.vas_paused)
            .collect();
        assert!(!dropped.is_empty(), "{rate}: no fully dropped block");
        for b in dropped {
            assert_eq!(b.meters.reconstruction_peak, 0.0, "{rate}: {:?}", b.meters);
            assert_eq!(b.meters.route_peak, 0.0);
            assert!(b.meters.record_filter_peak > 0.0 || b.meters.input_peak == 0.0);
        }
    }
}

/// Reading twice returns the same value and leaves the next output unchanged (FR-010): the
/// output of an engine read after every block equals an unread twin's, bit for bit.
#[test]
fn read_is_pure() {
    for &rate in configs::measurement_rates() {
        let x = voiced(rate);
        let s = Settings::new(rate);
        let mut read = Pipeline::new(s).unwrap();
        let mut unread = Pipeline::new(s).unwrap();
        let mut ev: Vec<VasEvent> = Vec::new();
        for chunk in x.chunks(256) {
            let mut a = vec![0.0f32; chunk.len()];
            let mut b = vec![0.0f32; chunk.len()];
            let ia = read.process_with_events(chunk, &mut a, &mut ev).unwrap();
            let m1 = read.meters();
            let m2 = read.meters();
            assert_eq!(m1, m2);
            let ib = unread.process(chunk, &mut b).unwrap();
            assert_eq!(ia, ib);
            assert!(
                a[..ia.produced]
                    .iter()
                    .zip(&b[..ib.produced])
                    .all(|(p, q)| p.to_bits() == q.to_bits())
            );
        }
        assert_eq!(read.meters(), unread.meters());
    }
}

/// Before any call, after `reset` and after `reconfigure` the read-out is the rest with the
/// configured hang; `Meters::default()` is the all-zero value.
#[test]
fn rest_states() {
    let mut s = Settings::new(44_100);
    s.vas.hang_ms = 500.0;
    let mut p = Pipeline::new(s).unwrap();
    assert_eq!(p.meters().hang_remaining_samples, 3000);
    assert_eq!(Meters::default().hang_remaining_samples, 0);
    let x = voiced(44_100);
    let mut y = vec![0.0f32; x.len()];
    let _ = p.process(&x, &mut y).unwrap();
    assert_ne!(p.meters(), p.rest_meters());
    p.reset();
    assert_eq!(p.meters(), p.rest_meters());
    let _ = p.process(&x, &mut y).unwrap();
    let mut s2 = s;
    s2.vas.hang_ms = 100.0;
    p.reconfigure(s2).unwrap();
    assert_eq!(p.meters().hang_remaining_samples, 600);
    assert_eq!(p.meters(), p.rest_meters());
}
