//! Spec 027 US4 [TR: D7, D8, FR-007, FR-008]: one recipe on every surface: the public fill
//! against the harness's, and (with the Swift session) no recipe left in Swift.

use rr_dr60::{Error, Pipeline, Settings, StopResult};
use rr_dr60_harness::configs::measurement_rates;
use rr_dr60_harness::record_contract_checks as rc;
use rr_dr60_harness::stimulus;

fn engine(rate: u32, codec: bool) -> Pipeline {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = codec;
    s.memory.capacity_s = 30;
    Pipeline::new(s).unwrap()
}

/// Records half a second of a tone and returns its digest.
fn record_digest(p: &mut Pipeline) -> u64 {
    let rate = p.settings().host_rate_hz;
    let x = stimulus::tone(1000.0, 0.1, f64::from(rate), rate as usize / 2);
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let out = p.stop_recording(&mut []).unwrap();
    let StopResult::Kept { number, .. } = out.result else {
        panic!("kept")
    };
    p.recording_digest(number).unwrap()
}

/// The public fill against the harness's: the same remaining capacity, and the same digest for
/// a recording made afterwards (two fillers of different content, one digest: the filler never
/// enters a later recording's digest); floored to whole frames with the codec on; zero units
/// add no item; the refusals.
#[test]
fn as3_fill() {
    for &rate in measurement_rates() {
        for codec in [false, true] {
            let mut a = engine(rate, codec);
            let mut b = engine(rate, codec);
            let target = a.memory_status().remaining_samples - 6000 * 7;
            a.fill_to_remaining(target).unwrap();
            rc::fill_to_remaining(&mut b, target);
            assert_eq!(a.memory_status(), b.memory_status(), "{rate} codec {codec}");
            assert_eq!(
                record_digest(&mut a),
                record_digest(&mut b),
                "{rate} codec {codec}"
            );

            // Not a whole number of frames: floored, the remaining at or above the target.
            let mut c = engine(rate, codec);
            let now = c.memory_status().remaining_samples;
            let odd = now - (120 * 50 + 7);
            c.fill_to_remaining(odd).unwrap();
            let got = c.memory_status().remaining_samples;
            let unit = if codec { 120 } else { 1 };
            assert_eq!(got, now - (now - odd) / unit * unit, "{rate} codec {codec}");
            assert!(got >= odd);

            // Zero units: no item.
            let mut d = engine(rate, codec);
            let st = d.memory_status();
            let tiny = st.remaining_samples - if codec { 119 } else { 0 };
            d.fill_to_remaining(tiny).unwrap();
            assert_eq!(d.memory_status(), st, "{rate} codec {codec}: no item");

            // Refusals: fuller than asked; recording; playing.
            let mut e = engine(rate, codec);
            let r = e.memory_status().remaining_samples;
            assert_eq!(e.fill_to_remaining(r + 1), Err(Error::DoesNotFit));
            e.start_recording().unwrap();
            assert_eq!(e.fill_to_remaining(0), Err(Error::RecordingInProgress));
            let _ = e
                .record(
                    &stimulus::tone(1000.0, 0.1, f64::from(rate), rate as usize / 10),
                    &mut [],
                )
                .unwrap();
            let _ = e.stop_recording(&mut []).unwrap();
            e.start_playback(1).unwrap();
            assert_eq!(e.fill_to_remaining(0), Err(Error::PlaybackInProgress));
        }
    }
}

/// No slot left: refused, the memory unchanged.
#[test]
fn as3_fill_no_slot() {
    let mut s = Settings::new(6000);
    s.memory.max_recordings = 1;
    let mut p = Pipeline::new(s).unwrap();
    let r = p.memory_status().remaining_samples;
    p.fill_to_remaining(r - 600).unwrap();
    let st = p.memory_status();
    assert_eq!(p.fill_to_remaining(r - 1200), Err(Error::DoesNotFit));
    assert_eq!(p.memory_status(), st);
}

/// AS2: the Swift package keeps no copy of the recipe: no fill, schedule refill, stop rule or
/// slot restore; it calls the C session (a source scan, spec 027 D8).
#[test]
fn as2_no_swift_recipe() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../swift/Sources");
    let mut src = String::new();
    for dir in std::fs::read_dir(&root).unwrap() {
        for f in std::fs::read_dir(dir.unwrap().path()).unwrap() {
            let p = f.unwrap().path();
            if p.extension().is_some_and(|e| e == "swift") {
                src.push_str(&std::fs::read_to_string(p).unwrap());
            }
        }
    }
    for w in [
        "rr_dr60_pending_changes",
        "rr_dr60_schedule_change",
        "rr_dr60_import",
        "rr_dr60_stop_recording",
        "rr_dr60_set_component_parameter",
        "rr_dr60_create_at_counter",
        "func fill(",
        "restoreStore",
    ] {
        assert!(!src.contains(w), "the Swift sources still use {w}");
    }
    assert!(src.contains("rr_dr60_rerender_new") && src.contains("rr_dr60_rerender_feed_i24"));
}
