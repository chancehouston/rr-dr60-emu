//! Spec 009 edge cases (tasks.md T011; spec.md › Edge Cases): every listed case, measured on
//! the output-chain-only configurations or the 0.9 default.
#![allow(clippy::disallowed_methods)] // analysis in a test, not the DSP path (001 R-04)

use rr_dr60::output::VOLUME_REFERENCE;
use rr_dr60::{Error, OutputSettings, Pipeline, Route, Settings, Tap};
use rr_dr60_harness::analysis::{self, db};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::{configs, output_checks, stimulus};

const RATES: [u32; 2] = [6000, 48_000];

fn tone(rate: u32, level_dbfs: f64, secs: f64) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone_at_phase(
        1000.0,
        level_dbfs,
        stimulus::peak_phase(1000.0, rate),
        fs,
        (secs * fs) as usize,
    )
}

fn in_blocks(
    p: &mut Pipeline,
    x: &[f32],
    block: usize,
    mut between: impl FnMut(&mut Pipeline, usize),
) -> Vec<f32> {
    let mut out = Vec::with_capacity(x.len());
    for (i, chunk) in x.chunks(block).enumerate() {
        between(p, i);
        let mut y = vec![0.0f32; chunk.len()];
        let n = p.process(chunk, &mut y).expect("ok").produced;
        out.extend_from_slice(&y[..n]);
    }
    out
}

/// "Volume position 0": the route output decays below −120 dBFS within 100 ms on both routes
/// (the exact zeros at the after-stage-11 tap are in the same check).
#[test]
fn volume_zero_decay_on_both_routes() {
    for rate in RATES {
        for route in [Route::Speaker, Route::Earphone] {
            let rows = output_checks::volume_zero(rate, route, &standard);
            rr_dr60_harness::checks::assert_all(&rows);
        }
    }
}

/// "Volume position out of range or not finite": refused with a reason; the previous position
/// stays.
#[test]
fn invalid_positions_are_refused_and_the_position_kept() {
    let mut p = standard(Settings::new(48_000));
    p.set_volume(0.3).unwrap();
    for bad in [-0.001f32, 1.001, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            p.set_volume(bad),
            Err(Error::InvalidVolume { .. })
        ));
        assert_eq!(p.volume(), 0.3);
    }
}

/// "Volume changed mid-block": the new position applies from the next block, and the volume
/// gain never overshoots either position's value (the per-sample gain at the earphone's
/// after-stage-11 tap is one of the two laws' values at every sample).
#[test]
fn volume_change_lands_at_the_next_block_without_overshoot() {
    for rate in RATES {
        let mut s = configs::settings("output_only_earphone", rate);
        s.tap = Tap::AfterVolumeAmp;
        let x = tone(rate, -20.0, 4096.0 / f64::from(rate));
        let reference = {
            let mut r = s;
            r.output = OutputSettings::BYPASSED;
            r.tap = Tap::AfterPlayback;
            output_checks::run(&standard, r, Route::Earphone, 1.0, &x)
        };
        let mut p = standard(s);
        p.set_route(Route::Earphone);
        let y = in_blocks(&mut p, &x, 64, |p, i| {
            p.set_volume(if i < 32 { 1.0 } else { 0.5 }).unwrap();
        });
        // The reference is the chain's input; the earphone after-stage-11 tap is k_ear(p) times
        // the 32 Hz-filtered input, so compare block energies: the ratio is the law's at every
        // block. The gain is block-constant by construction (one scalar per processing call,
        // `OutputChain::set_controls`; the core test
        // `route_change_resets_sections_and_volume_change_does_not` pins that a volume change
        // moves only the scalars), so a block reading suffices; the sample-exact timing is the
        // `assert_ne!` below.
        let k1 = 10f64.powf(f64::from(rr_dr60::output::earphone_gain_db(1.0)) / 20.0);
        let k2 = 10f64.powf(f64::from(rr_dr60::output::earphone_gain_db(0.5)) / 20.0);
        let mut vol = 0.0; // the 32 Hz section's effect is the same for both runs: ratio of y to ref ≈ k
        for (i, (yb, rb)) in y.chunks(64).zip(reference.chunks(64)).enumerate() {
            let ey: f64 = yb.iter().map(|v| f64::from(*v).powi(2)).sum();
            let er: f64 = rb.iter().map(|v| f64::from(*v).powi(2)).sum();
            if er < 1e-9 {
                continue;
            }
            let k = (ey / er).sqrt();
            let want = if i < 32 { k1 } else { k2 };
            assert!(
                (k - want).abs() < 0.02 * want,
                "{rate} Hz block {i}: gain {k} vs {want}"
            );
            vol = k;
        }
        assert!(vol > 0.0);
        // The first sample of block 32 already differs from a run that kept position 1.
        let mut q = standard(s);
        q.set_route(Route::Earphone);
        let same = in_blocks(&mut q, &x, 64, |p, _| p.set_volume(1.0).unwrap());
        assert_eq!(y[..64 * 32], same[..64 * 32]);
        assert_ne!(y[64 * 32], same[64 * 32]);
    }
}

/// "Route changed mid-playback": the next block is the other route's, and the newly selected
/// route's stages start from rest (with silence before the switch, the switched engine equals
/// an engine on that route from the start, bit for bit; the state reset itself is a core unit
/// test).
#[test]
fn route_change_lands_at_the_next_block_from_rest() {
    for rate in RATES {
        // The chain alone with both branches on: silence is then exact zeros through the
        // resampler, so an engine on the earphone route from the start is at rest at the
        // switch point (the 0.9 default's decoder output on silence is quiet, not zero).
        let mut s = configs::settings("output_only_speaker", rate);
        s.output = OutputSettings::DEVICE;
        let x = tone(rate, -20.0, 8192.0 / f64::from(rate));
        let mut a = standard(s);
        a.set_volume(1.0).unwrap();
        let switched = in_blocks(&mut a, &x, 64, |p, i| {
            p.set_route(if i < 64 {
                Route::Speaker
            } else {
                Route::Earphone
            });
        });
        let mut b = standard(s);
        b.set_volume(1.0).unwrap();
        let speaker = in_blocks(&mut b, &x, 64, |p, _| p.set_route(Route::Speaker));
        assert_eq!(switched.len(), speaker.len());
        let cut = 64 * 64;
        assert_eq!(
            switched[..cut],
            speaker[..cut],
            "{rate}: blocks before the switch"
        );
        let diff = switched[cut..cut + 64]
            .iter()
            .zip(&speaker[cut..cut + 64])
            .any(|(a, b)| a != b);
        assert!(diff, "{rate}: the switch landed at block 64");
        // Rest state: silence before the switch, then the tone.
        let mut quiet = vec![0.0f32; cut];
        quiet.extend_from_slice(&x[cut..]);
        let mut c = standard(s);
        c.set_volume(1.0).unwrap();
        let switched_q = in_blocks(&mut c, &quiet, 64, |p, i| {
            p.set_route(if i < 64 {
                Route::Speaker
            } else {
                Route::Earphone
            });
        });
        let mut d = standard(s);
        d.set_volume(1.0).unwrap();
        let earphone_q = in_blocks(&mut d, &quiet, 64, |p, _| p.set_route(Route::Earphone));
        assert_eq!(switched_q[cut..], earphone_q[cut..], "{rate}: from rest");
    }
}

/// "Output above full scale": stage 11's output on the speaker route never exceeds full scale
/// (the clipper), and a steady tone below the knee never does after the template (its gain is
/// at most 1.0); a transient or a clipped signal can, through the template's resonance (the impulse response's L1 norm is above
/// 1), and on the earphone route the output exceeds full scale only when the chain's input does
/// (the playback filter's reconstruction overshoot of a full-scale square wave), scaled by the
/// position. Documented, not clamped (spec amended at implementation).
#[test]
fn output_above_full_scale_is_documented_not_clamped() {
    let rate = 48_000;
    let n = rate as usize;
    let square: Vec<f32> = (0..n)
        .map(|i| if (i / 24) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let mut s = configs::settings("default_earphone", rate);
    s.agc.enabled = false;
    s.adc_clamp_enabled = false;
    s.vas.enabled = false;
    s.speech_codec_enabled = false;
    let ear = output_checks::run(&standard, s, Route::Earphone, 1.0, &square);
    let spk = output_checks::run(&standard, s, Route::Speaker, 1.0, &square);
    let mut amp = s;
    amp.tap = Tap::AfterVolumeAmp;
    let stage_11 = output_checks::run(&standard, amp, Route::Speaker, 1.0, &square);
    let mut input = s;
    input.output = OutputSettings::BYPASSED;
    input.tap = Tap::AfterPlayback;
    let chain_in = output_checks::run(&standard, input, Route::Earphone, 1.0, &square);
    let peak = |y: &[f32]| y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        peak(&chain_in) > 1.0,
        "stage 10 overshoots a square wave: {}",
        peak(&chain_in)
    );
    assert!(
        peak(&ear) > 1.0,
        "the earphone route passes the overshoot: {}",
        peak(&ear)
    );
    assert!(
        peak(&stage_11) <= 1.0,
        "stage 11's output is bounded by the clipper: {}",
        peak(&stage_11)
    );
    println!(
        "full-scale 1 kHz square wave at position 1: chain input peak {:.3}, earphone {:.3}, after stage 11 {:.3}, speaker route {:.3} (the template's resonance rings; reported)",
        peak(&chain_in),
        peak(&ear),
        peak(&stage_11),
        peak(&spk)
    );
    // A steady full-scale tone **below the knee** (the reference position) on the speaker route
    // never exceeds 1.0 after the template; at position 1 the same tone is 21 dB over the
    // ceiling, stage 11 emits a near-square wave and the template rings on it (reported).
    let tone = tone(rate, 0.0, 0.5);
    let spk_tone = output_checks::run(&standard, s, Route::Speaker, VOLUME_REFERENCE, &tone);
    assert!(peak(&spk_tone) <= 1.0, "{}", peak(&spk_tone));
    let spk_loud = output_checks::run(&standard, s, Route::Speaker, 1.0, &tone);
    println!(
        "full-scale 1 kHz tone at position 1 on the speaker route: peak {:.3} (clipped, then the template rings; reported)",
        peak(&spk_loud)
    );
}

/// "DC and very low frequencies": with the stages on neither route carries DC; with stage 11
/// bypassed the speaker template still blocks it; with everything bypassed it passes.
#[test]
fn dc_is_blocked_by_the_chain_and_passes_with_everything_bypassed() {
    for rate in RATES {
        let n = 2 * rate as usize;
        let step = vec![0.1f32; n];
        let last_100ms = |y: &[f32]| analysis::rms_dbfs(&y[y.len() - rate as usize / 10..]);
        for (config, route) in [
            ("output_only_speaker", Route::Speaker),
            ("output_only_earphone", Route::Earphone),
            ("output_only_amp", Route::Speaker),
        ] {
            let y = output_checks::run(
                &standard,
                configs::settings(config, rate),
                route,
                1.0,
                &step,
            );
            assert!(
                last_100ms(&y) < -80.0,
                "{rate} {config}: {}",
                last_100ms(&y)
            );
        }
        let mut s = configs::settings("output_only_speaker", rate);
        s.output.volume_amp_enabled = false;
        let y = output_checks::run(&standard, s, Route::Speaker, 1.0, &step);
        assert!(
            last_100ms(&y) < -80.0,
            "{rate} template alone: {}",
            last_100ms(&y)
        );
        let y = output_checks::run(
            &standard,
            configs::settings("bypass_all", rate),
            Route::Speaker,
            1.0,
            &step,
        );
        assert!(
            last_100ms(&y) > -21.0,
            "{rate} everything bypassed passes DC: {}",
            last_100ms(&y)
        );
    }
}

/// "Stage 11 bypassed": unity gain at the codec's scale and the position ignored; the earphone
/// route then carries the follower's corners alone.
#[test]
fn stage_11_bypassed_is_unity_and_ignores_the_position() {
    for rate in RATES {
        let mut s = configs::settings("output_only_earphone", rate);
        s.output.volume_amp_enabled = false;
        let x = tone(rate, -20.0, 0.5);
        let a = output_checks::run(&standard, s, Route::Earphone, 0.0, &x);
        let b = output_checks::run(&standard, s, Route::Earphone, 1.0, &x);
        assert_eq!(a, b, "{rate}: the position is ignored");
        let fs = f64::from(rate);
        let (g, _) = analysis::gain_db_and_phase(&x, &b, 1000.0, fs, (0.2 * fs) as usize);
        let want =
            analysis::discrete_stage_response(analysis::OutputTemplate::Earphone, 1000.0, fs);
        assert!(
            (g - want).abs() < 0.5,
            "{rate}: {g} vs {want} (the corners alone, unity passband)"
        );
        // And the speaker route: the template alone, peak gain 1.0.
        let mut t = configs::settings("output_only_speaker", rate);
        t.output.volume_amp_enabled = false;
        let c = output_checks::run(&standard, t, Route::Speaker, 0.0, &x);
        let d = output_checks::run(&standard, t, Route::Speaker, 1.0, &x);
        assert_eq!(c, d);
    }
}

/// "The after-stage-11 tap on the earphone route": the volume control's output, K_EAR(p) times
/// the 32 Hz-filtered chain input (recomputed here with the harness's own section).
#[test]
fn after_stage_11_on_the_earphone_route_is_the_volume_controls_output() {
    let rate = 48_000;
    let mut s = configs::settings("output_only_earphone", rate);
    s.tap = Tap::AfterVolumeAmp;
    let mut x = stimulus::impulse(rate as usize);
    x[0] = 0.01;
    let y = output_checks::run(&standard, s, Route::Earphone, 0.5, &x);
    let mut r = s;
    r.output = OutputSettings::BYPASSED;
    r.tap = Tap::AfterPlayback;
    let input = output_checks::run(&standard, r, Route::Earphone, 0.5, &x);
    let k = 10f64.powf(f64::from(rr_dr60::output::earphone_gain_db(0.5)) / 20.0);
    let fs = f64::from(rate);
    let want = |f: f64| {
        db(k) + analysis::discrete_template_response(analysis::OutputTemplate::Amplifier, f, fs)
            - analysis::discrete_stage_response(analysis::OutputTemplate::Amplifier, f, fs)
    };
    for f in [100.0, 1000.0, 2000.0] {
        let got = db((analysis::dtft(&y, f, fs) / analysis::dtft(&input, f, fs)).norm());
        assert!((got - want(f)).abs() < 0.05, "{f} Hz: {got} vs {}", want(f));
    }
}

/// "Named route taps with the other route selected": refused at the processing call; the route
/// output tap always means the selected route.
#[test]
fn named_route_taps_are_refused_off_route() {
    let x = tone(48_000, -20.0, 0.01);
    let mut y = vec![0.0f32; x.len()];
    let mut s = Settings::new(48_000);
    s.tap = Tap::AfterEarphone;
    let mut p = standard(s);
    assert_eq!(
        p.process(&x, &mut y),
        Err(Error::TapNotOnRoute {
            tap: Tap::AfterEarphone,
            route: Route::Speaker
        })
    );
    assert_eq!(p.process_in_place(&mut y).produced, 0);
    p.set_route(Route::Earphone);
    assert!(p.process(&x, &mut y).is_ok());
}

/// "Recording": the chain holds no state across a recording; a later playback starts from rest
/// (the same twice, and the same as a fresh clone's).
#[test]
fn a_recording_leaves_no_chain_state() {
    let rate = 6000;
    let mut p = standard(configs::settings("default_output", rate));
    p.set_volume(VOLUME_REFERENCE).unwrap();
    let x = tone(rate, -10.0, 1.0);
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let play = |p: &mut Pipeline| {
        p.start_playback(1).unwrap();
        let mut out = Vec::new();
        let mut buf = vec![0.0f32; 480];
        loop {
            let i = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..i.produced]);
            if i.finished {
                break;
            }
        }
        out
    };
    let a = play(&mut p);
    let mut fresh = p.clone();
    let b = play(&mut p);
    let c = play(&mut fresh);
    assert_eq!(a, b);
    assert_eq!(a, c);
}

/// "Low host rates and clipping": at 6 kHz a clipped tone's harmonics above 3 kHz do not exist
/// and the rest alias within the host band; the peak ceiling holds and the figures are
/// reported, not asserted against the device.
#[test]
fn low_rate_clipping_is_band_limited_by_the_host_rate() {
    let rows = output_checks::peak_ceiling(6000, &standard);
    rr_dr60_harness::checks::assert_all(&rows);
    let x = tone(6000, 0.0, 1.0);
    let y = output_checks::run(
        &standard,
        configs::settings("output_only_amp", 6000),
        Route::Speaker,
        rr_dr60::output::VOLUME_6DB_OVER,
        &x,
    );
    let thd = analysis::thd_n_db(&y, 1000.0, 6000.0, 1200, 4800);
    println!(
        "6 kHz, 6 dB over the ceiling: THD+N {thd:.1} dB (the harmonics above 3 kHz are absent; reported only)"
    );
    assert!(y.iter().all(|v| v.abs() <= 1.0));
}
