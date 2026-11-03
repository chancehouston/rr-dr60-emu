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

#[test]
fn as1_fields() {
    assert_eq!(FIELDS.len(), 13);
    let mut ids: Vec<_> = FIELDS.iter().map(|f| f.identifier).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 13);
    assert!(FIELDS.iter().all(|f| !f.unit.is_empty() && f.sentence.ends_with('.')));
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
                            let tag = format!("{rate} {tap:?} agc {agc} clamp {clamp} {route:?} {call:?} {}", fld.identifier);
                            if !reached {
                                assert!(!moved, "{tag}: unreached but moved");
                            } else if fld.kind == MeterKind::Maximum {
                                assert!(moved, "{tag}: a reached maximum never moved");
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
            match i {
                0..=3 => assert_eq!(one, Some(0)),
                4..=7 => assert_eq!(
                    p.meter_lag_samples(i, MeterCall::Record),
                    Some(p.record_latency_pre_encoder_samples())
                ),
                // The output clamp's count is unreached with the option off (its default).
                _ => assert_eq!(
                    p.meter_lag_samples(i, MeterCall::Play),
                    p.meter_reached(i, MeterCall::Play).then(|| p.playback_latency_samples())
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

