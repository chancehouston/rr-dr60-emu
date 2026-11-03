//! Spec 026 US2 [TR: D2, FR-002, SC-002]: the one-pass drain figure: emitted and settled within
//! 10⁻⁵ of the output's peak on linear configurations (the gate open), with teeth; the drop
//! mode's held samples; the front taps and the band filter.

use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{Pipeline, Settings, Tap, VasMode};
use rr_dr60_harness::configs::measurement_rates;
use rr_dr60_harness::stimulus;

/// One-pass output of `x` followed by `drain` samples of silence.
fn run(p: &mut Pipeline, x: &[f32], drain: usize) -> Vec<f32> {
    let block = p.settings().host_rate_hz as usize / 100;
    let mut input = x.to_vec();
    input.extend(stimulus::silence(drain));
    let mut out = Vec::new();
    let mut ob = vec![0.0f32; block];
    for c in input.chunks(block) {
        let info = p.process(c, &mut ob[..c.len()]).unwrap();
        out.extend_from_slice(&ob[..info.produced]);
    }
    out
}

/// The silence the output actually needs after the input: with a long tail, the last output
/// sample at or above 10⁻⁵ of the output's peak, less the input's length (exact, no window).
fn needed(y: &[f32], input_len: usize) -> usize {
    let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let last = y
        .iter()
        .rposition(|v| f64::from(v.abs()) >= 1e-5 * f64::from(peak))
        .map_or(0, |i| i + 1);
    last.saturating_sub(input_len)
}

/// A linear configuration: no hiss, no coder, the gate off, below the knee.
fn linear(rate: u32, tap: Tap) -> Settings {
    let mut s = Settings::new(rate);
    s.tap = tap;
    s.mic.capsule_enabled = false;
    s.speech_codec_enabled = false;
    s.vas.enabled = false;
    s
}

/// A tone at `f` ending on a peak, then a burst: the stimulus the settle is read on.
fn stim(rate: u32, f: f64) -> Vec<f32> {
    let fs = f64::from(rate);
    // About 0.2 s of the tone, cut a quarter period after a whole number of periods (a peak).
    let periods = (0.2 * f).floor().max(1.0);
    let n = ((periods + 0.25) * fs / f).round() as usize + 1;
    let mut x = stimulus::tone(f, 0.05, fs, n);
    x.extend(stimulus::tone(1000.0, 0.05, fs, (fs * 0.05) as usize));
    x
}

/// `teeth`: the row's bound has one dominant term (the converters alone, the band filter), so
/// the figure must be under twice the silence actually needed; elsewhere the sum of settles is
/// looser and the needed silence is reported.
fn check(s: Settings, slot: Option<(f64, f64)>, f: f64, teeth: bool, tag: &str) {
    let mk = || {
        let mut p = Pipeline::new(s).unwrap();
        if let Some((freq, q)) = slot {
            p.set_component_parameter(BeforeLimiter, 0, 0.0).unwrap(); // low-pass
            p.set_component_parameter(BeforeLimiter, 1, freq).unwrap();
            p.set_component_parameter(BeforeLimiter, 2, q).unwrap();
            p.set_component(BeforeLimiter, ComponentId::BandFilter)
                .unwrap();
        }
        p
    };
    let d = mk().one_pass_drain_samples() as usize;
    let x = stim(s.host_rate_hz, f);
    // A long tail (4 × the figure and a second) for the exact reading.
    let y = run(&mut mk(), &x, 4 * d + s.host_rate_hz as usize);
    assert!(
        y.len() >= x.len() + d.min(y.len() - x.len()),
        "{tag}: every kept sample emitted"
    );
    let n = needed(&y, x.len());
    assert!(n <= d, "{tag}: needed {n} > drain {d}");
    if teeth {
        // One dominant term: the figure is no more than twice what is needed.
        assert!(
            2 * n > d,
            "{tag}: needed {n}, drain {d}: the bound is loose"
        );
    } else {
        println!("{tag}: drain {d}, needed {n}");
    }
}

#[test]
fn as1_emits_and_settles() {
    for &rate in measurement_rates() {
        // Every stage off but the conversion: the converters' responses alone.
        let mut bare = linear(rate, Tap::AfterRecord);
        bare.mic.amp_enabled = false;
        bare.record_stage_enabled = false;
        bare.agc.enabled = false;
        if rate != 6000 {
            check(bare, None, 1000.0, true, &format!("{rate} stages off"));
        }
        check(
            linear(rate, Tap::AfterRecord),
            None,
            300.0,
            false,
            &format!("{rate} record stage"),
        );
        check(
            linear(rate, Tap::RouteOutput),
            None,
            300.0,
            false,
            &format!("{rate} route output"),
        );
        // Mute mode with the gate on keeps every sample.
        let mut mute = linear(rate, Tap::RouteOutput);
        mute.vas.enabled = true;
        mute.vas.mode = VasMode::Mute;
        check(mute, None, 300.0, false, &format!("{rate} mute mode"));
        // The default engine's residual against its silent twin is reported, not asserted.
        let s = Settings::new(rate);
        let p = Pipeline::new(s).unwrap();
        println!(
            "{rate}: default one-pass drain {} host samples",
            p.one_pass_drain_samples()
        );
    }
}

/// Drop mode: when the input ends inside the hang and the rest of the hang is shorter than the
/// path after the gate, the gate pauses before every kept sample is out and holds them until the
/// next resume (documented, not emitted; hardware review B1).
#[test]
fn as2_drop_mode_held() {
    let mut held_any = false;
    for &rate in measurement_rates() {
        let mut s = Settings::new(rate);
        s.mic.capsule_enabled = false;
        s.vas.mode = VasMode::Drop;
        s.vas.hang_ms = 50.0;
        let p = Pipeline::new(s).unwrap();
        let d = p.one_pass_drain_samples() as usize;
        // The input ends 45 ms into the 50 ms hang: the gate pauses 5 ms into the drain.
        let mut x = stimulus::tone(1000.0, 0.1, f64::from(rate), rate as usize / 2);
        x.extend(stimulus::silence(rate as usize * 45 / 1000));
        let y = run(&mut Pipeline::new(s).unwrap(), &x, d);
        let kept = x.len() + p.latency_samples() as usize;
        let held = kept.saturating_sub(y.len());
        held_any |= held > 0;
        println!("{rate}: drain {d}, drop mode holds {held} of the input's samples at the pause");
    }
    assert!(
        held_any,
        "the input ending 45 ms into a 50 ms hang holds samples"
    );
}

#[test]
fn as3_taps_and_band_filter() {
    for &rate in measurement_rates() {
        assert_eq!(
            Pipeline::new(linear(rate, Tap::AfterMicCapsule))
                .unwrap()
                .one_pass_drain_samples(),
            0
        );
        let amp = Pipeline::new(linear(rate, Tap::AfterMicAmp))
            .unwrap()
            .one_pass_drain_samples();
        assert!(amp > 0);
        // A 20 Hz, Q 10 band filter read at the slot's tap, where nothing after it attenuates.
        check(
            linear(rate, Tap::AfterInsert),
            Some((20.0, 10.0)),
            20.0,
            true,
            &format!("{rate} slot"),
        );
        let mut p = Pipeline::new(linear(rate, Tap::AfterInsert)).unwrap();
        p.set_component_parameter(BeforeLimiter, 1, 20.0).unwrap();
        p.set_component_parameter(BeforeLimiter, 2, 10.0).unwrap();
        p.set_component(BeforeLimiter, ComponentId::BandFilter)
            .unwrap();
        let d = p.one_pass_drain_samples();
        println!(
            "{rate}: the slot's 20 Hz Q 10 band filter: drain {d} ({:.3} s)",
            f64::from(d) / f64::from(rate)
        );
    }
}
