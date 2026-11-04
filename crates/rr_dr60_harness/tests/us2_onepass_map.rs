//! Spec 028 US2 [TR: D3, FR-003, SC-002]: where a pause is heard on the one-pass output, in
//! both modes, at the measurement rates.

use rr_dr60::{Tap, VasMode};
use rr_dr60_harness::configs;
use rr_dr60_harness::onepass_map_checks::{self as om, Cell, Render};

/// The onset reader's threshold, −30 dBFS.
const ONSET: f32 = 0.031_622_775;

/// The coder-on renders in `mode`: the cells and `Tap::AfterDecoder` (the coder runs, stage 10
/// does not).
fn coder_on(rate: u32, mode: VasMode) -> Vec<(String, Render)> {
    let mut v: Vec<_> = Cell::all()
        .into_iter()
        .filter(|c| c.codec && c.mode == mode)
        .map(|c| (c.name(), om::render(c, rate)))
        .collect();
    let mut s = Cell::all()[if mode == VasMode::Drop { 1 } else { 3 }].settings(rate);
    s.tap = Tap::AfterDecoder;
    v.push((format!("AfterDecoder {mode:?}"), om::render_with(s)));
    v
}

/// AS1: drop mode with the coder on (the cells and `AfterDecoder`): the read is
/// `pos + C + L_post`; the first output above −30 dBFS (10 dB below the burst's input peak: the
/// decoded peak overshoots it by up to 10 dB, a coder behaviour, so a half-peak reader would
/// read the coder) lies within one 1 kHz period of the read either side (the decoded onset leads
/// by up to two device samples; the signed range is printed per rate); the span before it plays
/// f_a.
#[test]
fn as1_onepass_map_splice_drop() {
    for &rate in configs::measurement_rates() {
        let period = i64::from(rate / 1000);
        let (mut lo, mut hi) = (i64::MAX, i64::MIN);
        for (name, r) in coder_on(rate, VasMode::Drop) {
            assert_eq!(r.events.len(), om::BURSTS, "{rate} Hz {name}");
            for e in &r.events {
                let s = r.p.one_pass_splice_output_position(e);
                assert_eq!(s, e.output_position + r.c + r.l_post);
                let onset = (e.output_position as usize..)
                    .find(|&o| r.y[o].abs() > ONSET)
                    .unwrap() as i64
                    - s as i64;
                lo = lo.min(onset);
                hi = hi.max(onset);
                assert!(
                    (-period..=period).contains(&onset),
                    "{rate} Hz {name}: onset S {onset:+}"
                );
                let fa = om::level_db(&r.y, rate, om::F_A, (s - r.c) as usize, s as usize);
                let fb = om::level_db(&r.y, rate, om::F_B, (s - r.c) as usize, s as usize);
                assert!(fa >= fb + 12.0, "{rate} Hz {name}");
            }
        }
        println!("{rate} Hz: onsets S {lo:+} … S {hi:+} (bound ±{period})");
    }
}

/// AS2: mute mode (the cells and `AfterDecoder`): the f_a label holds up to the read (the 4 ms
/// before it within 10 dB of the 20 ms before that) and falls after it, by at least 6 dB in the
/// 4 ms after it with the coder on (the decoder's residue on the region's zeros: the range is
/// printed per rate) and 40 dB with it off.
#[test]
fn as2_onepass_map_splice_mute() {
    for &rate in configs::measurement_rates() {
        let ms = rate as usize / 1000;
        let mut renders: Vec<(String, bool, Render)> = Cell::all()
            .into_iter()
            .filter(|c| c.mode == VasMode::Mute)
            .map(|c| (c.name(), c.codec, om::render(c, rate)))
            .collect();
        let (name, r) = coder_on(rate, VasMode::Mute).pop().unwrap();
        renders.push((name, true, r));
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (name, codec, r) in renders {
            assert_eq!(
                r.events.len(),
                om::BURSTS,
                "{rate} Hz {name}: three regions"
            );
            let fall = if codec { 6.0 } else { 40.0 };
            for e in &r.events {
                let s = r.p.one_pass_splice_output_position(e) as usize;
                let before = om::window_db(&r.y, rate, om::F_A, s - 25 * ms, 20 * ms);
                let late = om::window_db(&r.y, rate, om::F_A, s - 5 * ms, 4 * ms);
                let after = om::window_db(&r.y, rate, om::F_A, s + ms, 4 * ms);
                if codec {
                    lo = lo.min(late - after);
                    hi = hi.max(late - after);
                }
                assert!(
                    (late - before).abs() <= 10.0,
                    "{rate} Hz {name}: the label holds up to S"
                );
                assert!(
                    after <= late - fall,
                    "{rate} Hz {name}: the region is heard from S"
                );
            }
        }
        println!("{rate} Hz: the coder-on fall {lo:.1} … {hi:.1} dB (bound 6)");
    }
}

/// AS3: with the coder off the read is `pos + L_post` in both modes; at `Tap::AfterVas` with the
/// codec setting on (the gate runs, the coder does not) the coder adds nothing.
#[test]
fn as3_onepass_map_splice_no_codec() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all().into_iter().filter(|c| !c.codec) {
            let r = om::render(cell, rate);
            assert_eq!(r.c, 0);
            for e in &r.events {
                assert_eq!(
                    r.p.one_pass_splice_output_position(e),
                    e.output_position + r.l_post
                );
            }
        }
        let mut s = Cell::all()[1].settings(rate);
        s.tap = Tap::AfterVas;
        assert!(s.speech_codec_enabled && !s.one_pass_codec_runs());
        let r = om::render_with(s);
        assert_eq!(r.c, 0, "{rate} Hz: no coder at the gate's tap");
        assert!(!r.events.is_empty());
        for e in &r.events {
            assert_eq!(
                r.p.one_pass_splice_output_position(e),
                e.output_position + r.l_post
            );
        }
    }
}
