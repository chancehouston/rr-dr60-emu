//! Spec 026 US3 [TR: D4, FR-004]: the meter manifest, reach per call, and the lags.

use rr_dr60::meters::{FIELDS, MeterCall, MeterKind};
use rr_dr60::{Meters, Pipeline, Route, Settings, Tap, VasMode};
use rr_dr60_harness::configs::measurement_rates;
use rr_dr60_harness::stimulus;

const TAPS: [Tap; 12] = [
    Tap::AfterMicCapsule,
    Tap::AfterMicAmp,
    Tap::AfterInsert,
    Tap::AfterAgc,
    Tap::AfterRecord,
    Tap::AfterVas,
    Tap::AfterDecoder,
    Tap::AfterPlayback,
    Tap::RouteOutput,
    Tap::AfterVolumeAmp,
    Tap::AfterSpeaker,
    Tap::AfterEarphone,
];

/// A field's value as a comparable number.
fn value(m: &Meters, i: usize) -> f64 {
    match i {
        0 => f64::from(m.input_peak),
        1 => f64::from(m.limiter_input_peak),
        2 => f64::from(m.gain_reduction_db),
        3 => f64::from(m.hold_charge),
        4 => f64::from(m.record_filter_peak),
        5 => f64::from(m.adc_clamp_count),
        6 => f64::from(u8::from(m.vas_paused)),
        7 => m.hang_remaining_samples as f64,
        8 => f64::from(m.reconstruction_peak),
        9 => f64::from(m.route_peak),
        10 => f64::from(m.output_clamp_count),
        11 => f64::from(m.amp_clip_count),
        _ => f64::from(m.battery_ceiling),
    }
}

/// A loud tone (over full scale), a gap the gate takes, a tone.
fn stress(rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    let n = rate as usize;
    let mut x = stimulus::tone(1000.0, 3.0, fs, n * 3 / 10);
    x.extend(stimulus::silence(n));
    x.extend(stimulus::tone(1000.0, 3.0, fs, n * 3 / 10));
    x
}

fn settings(rate: u32, tap: Tap, agc: bool, clamp: bool) -> Settings {
    let mut s = Settings::new(rate);
    s.tap = tap;
    s.agc.enabled = agc;
    s.vas.hang_ms = 50.0;
    s.vas.mode = VasMode::Mute;
    s.output.clamp_enabled = clamp;
    s.mic.capsule_enabled = false;
    s
}

/// The read-outs of every call of `call` over `x`.
fn readouts(s: Settings, route: Route, call: MeterCall, x: &[f32]) -> (Pipeline, Vec<Meters>) {
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(route);
    // Position 1: the stress set drives the amplifier past its knee and the route past 1.0.
    p.set_volume(1.0).unwrap();
    let block = s.host_rate_hz as usize / 100;
    let mut v = Vec::new();
    match call {
        MeterCall::OnePass => {
            let mut ob = vec![0.0f32; block];
            for c in x.chunks(block) {
                if p.process(c, &mut ob[..c.len()]).is_err() {
                    break;
                }
                v.push(p.meters());
            }
        }
        MeterCall::Record => {
            p.start_recording().unwrap();
            for c in x.chunks(block) {
                let _ = p.record(c, &mut []).unwrap();
                v.push(p.meters());
            }
            let _ = p.stop_recording(&mut []).unwrap();
        }
        MeterCall::Play | _ => {
            p.start_recording().unwrap();
            let _ = p.record(x, &mut []).unwrap();
            let _ = p.stop_recording(&mut []).unwrap();
            p.start_playback(1).unwrap();
            let mut ob = vec![0.0f32; block];
            loop {
                let info = p.play(&mut ob).unwrap();
                v.push(p.meters());
                if info.finished {
                    break;
                }
            }
        }
    }
    (p, v)
}

/// Whether the 3.0-amplitude stress set must move count `i` where it is reached: the converter's
/// clamp with the limiter off (with it on the limiter holds the level under full scale), the
/// output clamp (the speaker route rings past 1.0 and the earphone route reaches it on the
/// stress set), the amplifier's knee.
fn count_expected(i: usize, agc: bool) -> bool {
    match i {
        5 => !agc,
        10 | 11 => true,
        _ => false,
    }
}

#[test]
fn as1_fields() {
    assert_eq!(FIELDS.len(), 13);
    let mut ids: Vec<_> = FIELDS.iter().map(|f| f.identifier).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 13);
    assert!(
        FIELDS
            .iter()
            .all(|f| !f.unit.is_empty() && f.sentence.ends_with('.'))
    );
}

/// Unreached fields never leave their rest value on the stress set; reached maxima move.
#[test]
fn as2_reach() {
    for &rate in measurement_rates() {
        let x = stress(rate);
        for tap in TAPS {
            for (agc, clamp) in [(true, false), (false, true)] {
                for route in [Route::Speaker, Route::Earphone] {
                    let s = settings(rate, tap, agc, clamp);
                    for call in [MeterCall::OnePass, MeterCall::Record, MeterCall::Play] {
                        if call != MeterCall::OnePass && tap != Tap::RouteOutput {
                            continue; // the paths ignore the tap
                        }
                        let (p, rows) = readouts(s, route, call, &x);
                        if rows.is_empty() {
                            continue; // a tap of the other route: every call refused
                        }
                        let rest = p.rest_meters();
                        for (i, fld) in FIELDS.iter().enumerate() {
                            let reached = p.meter_reached(i, call);
                            let moved = rows.iter().any(|m| value(m, i) != value(&rest, i));
                            let tag = format!(
                                "{rate} {tap:?} agc {agc} clamp {clamp} {route:?} {call:?} {}",
                                fld.identifier
                            );
                            if !reached {
                                assert!(!moved, "{tag}: unreached but moved");
                            } else if fld.kind == MeterKind::Maximum {
                                assert!(moved, "{tag}: a reached maximum never moved");
                            } else if fld.kind == MeterKind::Count && count_expected(i, agc) {
                                // The stress set clips: a reached count must count (coach 2).
                                assert!(moved, "{tag}: a reached count never counted");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn as3_lag() {
    for &rate in measurement_rates() {
        let p = Pipeline::new(Settings::new(rate)).unwrap();
        for (i, _) in FIELDS.iter().enumerate() {
            let one = p.meter_lag_samples(i, MeterCall::OnePass);
            // The one-pass at the default tap (coach 10): the record group by the record
            // path's latency to the encoder, the playback group by the one-pass latency.
            let want_one = match i {
                0..=3 => Some(0),
                4..=7 => Some(p.record_latency_pre_encoder_samples()),
                _ => p
                    .meter_reached(i, MeterCall::OnePass)
                    .then(|| p.latency_samples()),
            };
            assert_eq!(one, want_one, "{rate} field {i}");
            match i {
                0..=3 => assert_eq!(one, Some(0)),
                4..=7 => assert_eq!(
                    p.meter_lag_samples(i, MeterCall::Record),
                    Some(p.record_latency_pre_encoder_samples())
                ),
                // The output clamp's count is unreached with the option off (its default).
                _ => assert_eq!(
                    p.meter_lag_samples(i, MeterCall::Play),
                    p.meter_reached(i, MeterCall::Play)
                        .then(|| p.playback_latency_samples())
                ),
            }
            // A call that does not reach a field has no lag.
            if i >= 8 {
                assert_eq!(p.meter_lag_samples(i, MeterCall::Record), None);
            } else {
                assert_eq!(p.meter_lag_samples(i, MeterCall::Play), None);
            }
        }
        assert_eq!(p.meter_lag_samples(99, MeterCall::OnePass), None);
    }
}

/// The fields' strings pass both wording lists (spec 026 D3, D4).
#[test]
fn as4_sentences_pass() {
    for f in FIELDS {
        for s in [f.identifier, f.unit, f.sentence] {
            assert_eq!(rr_dr60::wording::first_hit(s), None, "{s}");
            assert_eq!(rr_dr60::wording::first_claim_hit(s), None, "{s}");
        }
    }
}

/// The three state fields pinned by their rules where reached (coach 2): the pause flag and the
/// hang on the gap (paused in the gap, the hang at 0 while paused and the configured hang while
/// sound), the battery factor equal to `battery::ceiling_factor` of the value set.
#[test]
fn as2_states() {
    for &rate in measurement_rates() {
        let x = stress(rate);
        let s = settings(rate, Tap::RouteOutput, true, false);
        let hang = (s.vas.hang_ms * 6.0).round() as u64; // device samples at 6 kHz
        for call in [MeterCall::OnePass, MeterCall::Record] {
            let (p, rows) = readouts(s, Route::Speaker, call, &x);
            assert!(p.meter_reached(6, call) && p.meter_reached(7, call));
            assert!(
                rows.iter().any(|m| m.vas_paused),
                "{rate} {call:?}: paused in the gap"
            );
            for m in &rows {
                if m.vas_paused {
                    assert_eq!(m.hang_remaining_samples, 0, "{rate} {call:?}");
                }
                assert!(m.hang_remaining_samples <= hang, "{rate} {call:?}");
            }
            assert!(rows.iter().any(|m| m.hang_remaining_samples == hang));
        }
        let mut p = Pipeline::new(s).unwrap();
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        p.set_battery_volts(2.4).unwrap();
        p.start_playback(1).unwrap();
        assert!(p.meter_reached(12, MeterCall::Play));
        let mut ob = vec![0.0f32; rate as usize / 10];
        for _ in 0..3 {
            let _ = p.play(&mut ob).unwrap();
        }
        assert_eq!(
            p.meters().battery_ceiling,
            rr_dr60::battery::ceiling_factor(2.4),
            "{rate}"
        );
    }
}

/// A live bypass of stage 11 keeps the stage running (a blend, spec 021), so the amplifier's
/// count stays reached (the spec's edge case).
#[test]
fn as2_live_bypass_keeps_reach() {
    let index = rr_dr60::controls::controls()
        .iter()
        .position(|c| c.identifier == "output.volume_amp_enabled")
        .unwrap() as u32;
    for &rate in measurement_rates() {
        let s = settings(rate, Tap::RouteOutput, true, false);
        let mut p = Pipeline::new(s).unwrap();
        let x = stress(rate);
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        p.schedule_change(rr_dr60::changes::Stream::Playback, 0, index, 0.0, 1)
            .unwrap();
        p.start_playback(1).unwrap();
        let mut ob = vec![0.0f32; rate as usize / 10];
        let _ = p.play(&mut ob).unwrap();
        assert!(!p.settings().output.volume_amp_enabled, "{rate}: landed");
        assert!(p.meter_reached(11, MeterCall::Play), "{rate}");
    }
}

/// The lags measured (hardware review B6): block size 1, an impulse at input sample 0 into the
/// one-pass (stages 1 – 3 and the codec off, so each group's point is linear), the call where
/// the group's peak first reads its impulse response's own peak against the published lag,
/// within one device sample plus the 1 kHz convention's offset (spec 020: an impulse peaks
/// about one device sample before it).
#[test]
fn as3_lag_measured() {
    for &rate in measurement_rates() {
        let mut s = Settings::new(rate);
        s.mic.capsule_enabled = false;
        s.mic.amp_enabled = false;
        s.agc.enabled = false;
        s.vas.enabled = false;
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let n = p.latency_samples() as usize + 2 * rate as usize / 100;
        let (mut rec, mut play) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let mut ob = [0.0f32; 1];
        for i in 0..n {
            let x = [if i == 0 { 0.5f32 } else { 0.0 }];
            let _ = p.process(&x, &mut ob).unwrap();
            rec.push(p.meters().record_filter_peak);
            play.push(p.meters().reconstruction_peak);
        }
        let argmax = |v: &[f32]| {
            v.iter()
                .enumerate()
                .fold(
                    (0usize, 0.0f32),
                    |a, (i, &x)| if x > a.1 { (i, x) } else { a },
                )
                .0
        };
        let dev = rate as usize / 6000;
        let slack = 2 * dev + 1;
        for (field, v) in [(4usize, &rec), (8, &play)] {
            let lag = p.meter_lag_samples(field, MeterCall::OnePass).unwrap() as usize;
            let at = argmax(v);
            println!("{rate}: field {field} peaks at call {at}, lag {lag}");
            assert!(
                at.abs_diff(lag) <= slack,
                "{rate} field {field}: {at} vs {lag}"
            );
        }
    }
}
