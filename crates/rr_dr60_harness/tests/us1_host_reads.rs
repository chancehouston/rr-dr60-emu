//! Spec 026 US1 [TR: D1, FR-001]: the one-pass's gate and coder rules, and the reports'
//! latency, against the engine's behaviour.

use rr_dr60::{Pipeline, Route, Settings, Tap, VasEvent, VasMode};
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

/// A tone, a gap the gate takes (hang 50 ms), a tone.
fn gapped(rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    let n = rate as usize;
    let mut x = stimulus::tone(1000.0, 0.1, fs, n * 3 / 10);
    x.extend(stimulus::silence(n));
    x.extend(stimulus::tone(1000.0, 0.1, fs, n * 3 / 10));
    x
}

/// One-pass output and events for `s` on `x` (route set for the earphone tap).
fn run(s: Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>) {
    let mut p = Pipeline::new(s).unwrap();
    if s.tap == Tap::AfterEarphone {
        p.set_route(Route::Earphone);
    }
    let block = s.host_rate_hz as usize / 100;
    let mut out = Vec::new();
    let mut ev = vec![VasEvent::default(); p.max_events(block)];
    let mut ob = vec![0.0f32; block];
    let mut events = Vec::new();
    for c in x.chunks(block) {
        let info = p
            .process_with_events(c, &mut ob[..c.len()], &mut ev)
            .unwrap();
        out.extend_from_slice(&ob[..info.produced]);
        events.extend_from_slice(&ev[..info.events]);
    }
    (out, events)
}

fn settings(rate: u32, tap: Tap, vas: bool, codec: bool, mode: VasMode) -> Settings {
    let mut s = Settings::new(rate);
    s.tap = tap;
    s.vas.enabled = vas;
    s.vas.hang_ms = 50.0;
    s.vas.mode = mode;
    s.speech_codec_enabled = codec;
    s.mic.capsule_enabled = false;
    s
}

#[test]
fn as1_vas_runs() {
    for &rate in measurement_rates() {
        let x = gapped(rate);
        for tap in TAPS {
            for vas in [false, true] {
                for codec in [false, true] {
                    for mode in [VasMode::Drop, VasMode::Mute] {
                        let s = settings(rate, tap, vas, codec, mode);
                        let (_, events) = run(s, &x);
                        assert_eq!(
                            !events.is_empty(),
                            s.one_pass_vas_runs(),
                            "{rate} {tap:?} vas {vas} codec {codec} {mode:?}"
                        );
                    }
                }
            }
        }
    }
}

/// The coder runs exactly when it changes the output: coder on/off twins (gate off).
#[test]
fn as1_codec_runs() {
    for &rate in measurement_rates() {
        let x = gapped(rate);
        for tap in TAPS {
            let on = settings(rate, tap, false, true, VasMode::Drop);
            let off = settings(rate, tap, false, false, VasMode::Drop);
            let (a, _) = run(on, &x);
            let (b, _) = run(off, &x);
            assert_eq!(a != b, on.one_pass_codec_runs(), "{rate} {tap:?}");
            let (pa, pb) = (Pipeline::new(on).unwrap(), Pipeline::new(off).unwrap());
            let delay = if on.one_pass_codec_runs() {
                180 * rate / 6000
            } else {
                0
            };
            assert_eq!(pa.latency_samples(), pb.latency_samples() + delay, "{rate} {tap:?}");
            assert_eq!(pa.one_pass_report_latency_samples(), pb.latency_samples(), "{rate} {tap:?}");
        }
        // The pinned rows: AfterDecoder with the coder off runs the gate, not the coder;
        // AfterInsert and AfterRecord run neither.
        let s = settings(rate, Tap::AfterDecoder, true, false, VasMode::Drop);
        assert!(s.one_pass_vas_runs() && !s.one_pass_codec_runs());
        for t in [Tap::AfterInsert, Tap::AfterRecord] {
            let s = settings(rate, t, true, true, VasMode::Drop);
            assert!(!s.one_pass_vas_runs() && !s.one_pass_codec_runs(), "{t:?}");
        }
        assert!(Tap::AfterInsert.is_front() && !Tap::AfterRecord.is_front());
        assert!(Tap::RouteOutput.is_after_stage_10() && !Tap::AfterDecoder.is_after_stage_10());
    }
}

/// With equal gate decisions, the reports mapped with the reports' latency land on the same
/// input positions with the coder on as with it off (the codec-off map is spec 020's), while
/// the full latency misses by the coder's delay.
#[test]
fn as1_report_latency() {
    for &rate in measurement_rates() {
        let x = gapped(rate);
        let on = settings(rate, Tap::RouteOutput, true, true, VasMode::Drop);
        let off = settings(rate, Tap::RouteOutput, true, false, VasMode::Drop);
        let (_, ea) = run(on, &x);
        let (_, eb) = run(off, &x);
        assert!(!ea.is_empty());
        assert_eq!(ea, eb, "{rate}: equal gate decisions");
        let (pa, pb) = (Pipeline::new(on).unwrap(), Pipeline::new(off).unwrap());
        for (a, b) in ea.iter().zip(&eb) {
            let map = |e: &VasEvent, lat: u32, events: &[VasEvent]| {
                rr_dr60::timeline::onepass_output_to_input(e.output_position, lat, events, true).0
            };
            let off_pos = map(b, pb.latency_samples(), &eb);
            assert_eq!(
                map(a, pa.one_pass_report_latency_samples(), &ea),
                off_pos,
                "{rate}"
            );
            assert_ne!(map(a, pa.latency_samples(), &ea), off_pos, "{rate}: the full latency misses");
        }
    }
}
