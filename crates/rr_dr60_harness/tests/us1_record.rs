//! Spec 017 User Story 1: adjust the recorder while it records (tasks.md T006; spec › US1
//! AS1 – AS8) at the measurement rates. Report:
//! `cargo test -p rr_dr60_harness --test us1_record -- --nocapture`.

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL};
use rr_dr60::output::volume_transition_samples;
use rr_dr60::{MicSettings, SUPPORTED_HOST_RATES, Settings, Stream, Tap, VasEvent, VasMode};
use rr_dr60_harness::changes_checks::{self as cc, Partition, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::mic_checks::HISS_SIGMA;
use rr_dr60_harness::record_checks::{self as rc, Value};
use rr_dr60_harness::{agc_checks, analysis, configs, stimulus};

const PARTITIONS: [Partition; 4] = [
    Partition::Fixed(1),
    Partition::Fixed(64),
    Partition::Fixed(4096),
    Partition::Irregular,
];

/// The 0.16 default front with the capsule on and the speech codec bypassed (`default_output`
/// plus stages 1 – 2, the codec off), in `mode`.
fn front(rate: u32, mode: VasMode) -> Settings {
    let mut s = configs::settings("default_output", rate);
    s.mic = MicSettings::DEVICE;
    s.vas.mode = mode;
    s.speech_codec_enabled = false;
    s
}

/// The same with the speech codec on.
fn front_codec(rate: u32, mode: VasMode) -> Settings {
    let mut s = front(rate, mode);
    s.speech_codec_enabled = true;
    s
}

fn at(check: impl Fn(u32)) {
    for &r in configs::measurement_rates() {
        check(r);
    }
}

fn voiced(rate: u32, seconds: f64) -> Vec<f32> {
    stimulus::voiced(f64::from(rate), seconds, -20.0, 0x017)
}

fn rate_index(rate: u32) -> usize {
    SUPPORTED_HOST_RATES
        .iter()
        .position(|&r| r == rate)
        .expect("a supported rate")
}

/// AS1 (FR-006, SC-001): a change at input position 0 of a fresh stream equals the constant
/// configuration bit for bit, on the recorder (the stored samples or frames) and the one-pass,
/// in both modes; the entry is at 0 with the old and new values and the tag.
#[test]
fn as1_input_zero_equals_the_constant_configuration() {
    at(|rate| {
        let x = voiced(rate, 1.0);
        for mode in [VasMode::Drop, VasMode::Mute] {
            for (name, base) in [
                ("codec off", front(rate, mode)),
                ("codec on", front_codec(rate, mode)),
            ] {
                for v in Value::examples() {
                    let constant = rc::constant_settings(&base, v);
                    let tag = format!("{rate} Hz {mode:?} {name} {}", v.identifier());
                    // The recorder.
                    let a = rc::record_run(
                        &mut standard(base),
                        &x,
                        Partition::Fixed(1024),
                        &[rc::change(Stream::Record, 0, v, 1)],
                    );
                    let b =
                        rc::record_run(&mut standard(constant), &x, Partition::Fixed(1024), &[]);
                    assert_eq!(
                        rc::stored_bytes(&a),
                        rc::stored_bytes(&b),
                        "{tag}: the recording"
                    );
                    assert_eq!(a.events, b.events, "{tag}: the events");
                    assert_eq!(a.log.len(), 1, "{tag}: one entry");
                    let e = a.log[0];
                    assert_eq!(
                        (e.position, e.before, e.after, e.tag, e.control, e.stream),
                        (
                            0,
                            v.read(&base),
                            v.as_f64(),
                            1,
                            v.control(),
                            rr_dr60::STREAM_RECORD
                        ),
                        "{tag}: the entry"
                    );
                    assert!(b.log.is_empty());
                    // The one-pass.
                    let mut p = standard(base);
                    let ra = rc::one_pass_run(
                        &mut p,
                        &x,
                        Partition::Fixed(1024),
                        &[rc::change(Stream::OnePass, 0, v, 1)],
                    );
                    let mut q = standard(constant);
                    let rb = rc::one_pass_run(&mut q, &x, Partition::Fixed(1024), &[]);
                    assert!(
                        same(&ra.out, &rb.out),
                        "{tag}: the one-pass at {:?}",
                        cc::first_diff(&ra.out, &rb.out)
                    );
                    assert_eq!(ra.log.len(), 1, "{tag}: one entry");
                    assert_eq!(
                        (ra.log[0].position, ra.log[0].control, ra.log[0].tag),
                        (0, v.control(), 1)
                    );
                    assert_eq!(ra.log[0].before, v.read(&base));
                    assert_eq!(ra.log[0].after, v.as_f64());
                    assert!(rb.log.is_empty());
                    // The settings follow the landing on both streams.
                    assert_eq!(v.read(p.settings()), v.as_f64(), "{tag}: settings()");
                }
            }
        }
        println!(
            "{rate:>5} Hz: AS1 input-0 identity, 8 values × 2 modes × 2 codec settings × 2 streams: 0 mismatches"
        );
    });
}

/// AS2 (FR-006, SC-001): a change inside a block lands the same at the four partitions, and
/// nothing before the landing moves: the kept samples before `kept_before(k(P))` equal the old
/// run's with the codec bypassed, and with the codec on every frame whose 120 samples plus the
/// 60-sample look-ahead end before it.
#[test]
fn as2_pre_landing_and_partitions() {
    at(|rate| {
        let x = voiced(rate, 1.0);
        let p0 = u64::from(rate / 3) + 7; // inside a block at every partition
        let k = rc::k_of(p0, rate);
        for v in Value::examples() {
            let base = front(rate, VasMode::Drop);
            let tag = format!("{rate} Hz {}", v.identifier());
            // The recorder at the four partitions.
            let runs: Vec<_> = PARTITIONS
                .iter()
                .map(|&part| {
                    rc::record_run(
                        &mut standard(base),
                        &x,
                        part,
                        &[rc::change(Stream::Record, p0, v, 1)],
                    )
                })
                .collect();
            for r in &runs[1..] {
                assert_eq!(
                    rc::stored_bytes(&runs[0]),
                    rc::stored_bytes(r),
                    "{tag}: the recorder's partitions"
                );
                assert_eq!(runs[0].log, r.log, "{tag}: the recorder's logs");
            }
            assert_eq!(runs[0].log.len(), 1);
            assert_eq!(runs[0].log[0].position, p0);
            // The pre-landing identity in the kept-sample index.
            let old = rc::record_run(&mut standard(base), &x, Partition::Fixed(1024), &[]);
            let stream = rc::device_stream(base, &x);
            let kept = rc::kept_before(&base, &stream, k);
            assert!(kept > 0, "{tag}: a landing after some kept samples");
            assert!(
                runs[0].samples[..kept]
                    .iter()
                    .zip(&old.samples[..kept])
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{tag}: the kept samples before k(P) = {k} ({kept} kept)"
            );
            // The codec on: the frames that end before the landing.
            let codec = front_codec(rate, VasMode::Drop);
            let c_new = rc::record_run(
                &mut standard(codec),
                &x,
                Partition::Fixed(1024),
                &[rc::change(Stream::Record, p0, v, 1)],
            );
            let c_old = rc::record_run(&mut standard(codec), &x, Partition::Fixed(1024), &[]);
            let whole = (0..).take_while(|f| 120 * f + 180 <= kept).count();
            assert!(whole > 0, "{tag}: frames before the landing");
            assert_eq!(
                &c_new.frames[..whole * 10],
                &c_old.frames[..whole * 10],
                "{tag}: the {whole} frames before the landing"
            );
            // The one-pass at the four partitions.
            let outs: Vec<_> = PARTITIONS
                .iter()
                .map(|&part| {
                    rc::one_pass_run(
                        &mut standard(base),
                        &x,
                        part,
                        &[rc::change(Stream::OnePass, p0, v, 1)],
                    )
                })
                .collect();
            for r in &outs[1..] {
                assert!(
                    same(&outs[0].out, &r.out),
                    "{tag}: the one-pass's partitions at {:?}",
                    cc::first_diff(&outs[0].out, &r.out)
                );
                assert_eq!(outs[0].log, r.log, "{tag}: the one-pass's logs");
            }
            assert_eq!(outs[0].log.len(), 1);
            assert_eq!(outs[0].log[0].position, p0);
        }
        println!(
            "{rate:>5} Hz: AS2 partitions and the pre-landing identity, 8 values: 0 mismatches (P = {p0}, k(P) = {k})"
        );
    });
}

/// The one-pass at the "after AGC" tap over `x` with `changes`.
fn agc_run(settings: Settings, x: &[f32], changes: &[cc::Change]) -> Vec<f32> {
    let mut p = standard(settings);
    rc::one_pass_run(&mut p, x, Partition::Fixed(4096), changes).out
}

/// AS3 (FR-003, FR-007, SC-002): the limiter's values land with the hold carried. The landing
/// sample's output equals the old run's (the gain law is settings-independent) and the runs
/// diverge after it; the attenuation 2 s after a knee change is within 0.1 dB of the new
/// constant run's; an attack change landing while discharged is bit-identical to the new run
/// from P (the dead time equal); a release change landing at a burst's end gives the new
/// run's release time within the spec 005 tolerance.
#[test]
fn as3_limiter() {
    at(|rate| {
        let fs = f64::from(rate);
        let base = configs::settings("agc_only", rate);
        let phase = stimulus::peak_phase(1000.0, rate);
        let n = (5.0 * fs) as usize;
        let p = 2 * rate as usize;
        let amp9 = stimulus::amplitude(9.0);
        for (knee_to, level) in [(-5.0f32, 9.0), (3.0, 9.0)] {
            let x = stimulus::tone_at_phase(1000.0, level, phase, fs, n);
            let old = agc_run(base, &x, &[]);
            let new = agc_run(rc::constant_settings(&base, Value::Knee(knee_to)), &x, &[]);
            let sched = agc_run(
                base,
                &x,
                &[rc::change(
                    Stream::OnePass,
                    p as u64,
                    Value::Knee(knee_to),
                    1,
                )],
            );
            let first = cc::first_diff(&sched, &old).expect("the knee change moves the output");
            assert!(
                first > p,
                "{rate} Hz knee → {knee_to}: the landing sample's output is the old run's (first diff {first}, P {p})"
            );
            assert!(
                first < p + rate as usize,
                "{rate} Hz knee → {knee_to}: diverges within 1 s ({first})"
            );
            let gate = 0.1 * stimulus::amplitude(level);
            let r_sched = rc::ratio_db(
                &sched,
                &x,
                p + 2 * rate as usize,
                p + 5 * rate as usize / 2,
                gate,
            );
            let r_new = rc::ratio_db(
                &new,
                &x,
                p + 2 * rate as usize,
                p + 5 * rate as usize / 2,
                gate,
            );
            assert!(
                (r_sched - r_new).abs() <= 0.1,
                "{rate} Hz knee → {knee_to}: {r_sched:.3} vs {r_new:.3} dB"
            );
            println!(
                "{rate:>5} Hz: knee −1 → {knee_to:+} dBFS on a {level:+} dBFS tone: first difference {} samples after P, attenuation 2 s after P {r_sched:.3} dB (new constant run {r_new:.3} dB)",
                first - p
            );
        }
        // The knee raised above the tone: reported (the hold discharges with the 10 s time constant).
        {
            let x = stimulus::tone_at_phase(1000.0, 2.0, phase, fs, n);
            let sched = agc_run(
                base,
                &x,
                &[rc::change(Stream::OnePass, p as u64, Value::Knee(3.0), 1)],
            );
            let old = agc_run(base, &x, &[]);
            let gate = 0.1 * stimulus::amplitude(2.0);
            let before = rc::ratio_db(&old, &x, p - rate as usize / 2, p, gate);
            let after = rc::ratio_db(
                &sched,
                &x,
                p + 2 * rate as usize,
                p + 5 * rate as usize / 2,
                gate,
            );
            println!(
                "{rate:>5} Hz: knee −1 → +3 dBFS on a +2 dBFS tone (reported): attenuation before P {before:.3} dB, 2 s after P {after:.3} dB"
            );
        }
        // The attack: −20 dBFS for 1 s (discharged), the change at 0.5 s, +9 dBFS from 1 s.
        {
            let mut x = stimulus::tone_at_phase(1000.0, -20.0, phase, fs, rate as usize);
            x.extend(stimulus::tone_at_phase(
                1000.0,
                9.0,
                phase,
                fs,
                2 * rate as usize,
            ));
            let pa = rate as usize / 2;
            let new = agc_run(rc::constant_settings(&base, Value::Attack(20.0)), &x, &[]);
            let sched = agc_run(
                base,
                &x,
                &[rc::change(
                    Stream::OnePass,
                    pa as u64,
                    Value::Attack(20.0),
                    1,
                )],
            );
            assert!(
                same(&sched[pa..], &new[pa..]),
                "{rate} Hz attack 3 → 20 ms: bit-identical to the new run from P at {:?}",
                cc::first_diff(&sched[pa..], &new[pa..])
            );
            let trace = |y: &[f32]| -> Vec<f64> {
                x.iter()
                    .zip(y)
                    .map(|(&xi, &yi)| {
                        if f64::from(xi).abs() >= 0.1 * amp9 {
                            analysis::db(f64::from(yi) / f64::from(xi))
                        } else {
                            f64::NAN
                        }
                    })
                    .collect()
            };
            let (ts, tn) = (trace(&sched), trace(&new));
            let steady = agc_checks::steady_db(&tn, fs, 2.0, 3.0);
            let ds = agc_checks::dead_time_s(&ts, fs, 1.0, steady).expect("a dead time");
            let dn = agc_checks::dead_time_s(&tn, fs, 1.0, steady).expect("a dead time");
            assert_eq!(ds.to_bits(), dn.to_bits());
            println!(
                "{rate:>5} Hz: attack 3 → 20 ms landed while discharged: bit-identical to the new run from P; dead time {:.2} ms (new run {:.2} ms)",
                ds * 1e3,
                dn * 1e3
            );
        }
        // The release: a +9 dBFS burst for 1 s, −20 dBFS after; the change at the burst's end.
        {
            let mut x = stimulus::tone_at_phase(1000.0, 9.0, phase, fs, rate as usize);
            x.extend(stimulus::tone_at_phase(
                1000.0,
                -20.0,
                phase,
                fs,
                3 * rate as usize,
            ));
            let pr = rate as usize;
            let old = agc_run(base, &x, &[]);
            let new = agc_run(rc::constant_settings(&base, Value::Release(400.0)), &x, &[]);
            let sched = agc_run(
                base,
                &x,
                &[rc::change(
                    Stream::OnePass,
                    pr as u64,
                    Value::Release(400.0),
                    1,
                )],
            );
            assert!(
                same(&sched[..=pr], &old[..=pr]),
                "{rate} Hz release: the landing sample's output is the old run's"
            );
            let gate = 0.1 * stimulus::amplitude(-20.0);
            let trace = |y: &[f32]| -> Vec<f64> {
                x.iter()
                    .zip(y)
                    .map(|(&xi, &yi)| {
                        if f64::from(xi).abs() >= gate {
                            analysis::db(f64::from(yi) / f64::from(xi))
                        } else {
                            f64::NAN
                        }
                    })
                    .collect()
            };
            let (ts, tn, to) = (trace(&sched), trace(&new), trace(&old));
            let rs = agc_checks::release_s(&ts, fs, 1.0, -1.0).expect("a release");
            let rn = agc_checks::release_s(&tn, fs, 1.0, -1.0).expect("a release");
            let ro = agc_checks::release_s(&to, fs, 1.0, -1.0).expect("a release");
            // Spec 005 FR-011: the release setting scales the time within ±10 % (A-035); the
            // two runs differ only in the hold at P (the new run discharged faster during the burst).
            assert!(
                (rs / rn - 1.0).abs() <= 0.10,
                "{rate} Hz release 1370 → 400 ms: {rs:.3} s vs the new run's {rn:.3} s"
            );
            assert!(
                rs < 0.5 * ro,
                "{rate} Hz: the release shortened ({rs:.3} vs {ro:.3} s)"
            );
            println!(
                "{rate:>5} Hz: release 1370 → 400 ms landed at the burst's end: release {:.3} s (new constant run {:.3} s, old {:.3} s)",
                rs, rn, ro
            );
        }
    });
}

/// The recorder in `base` over `x` with `change` (or none) and the reference gate's decisions
/// with the same change at k(P): the decisions, the events and the stored samples agree.
fn gate_matches_reference(
    rate: u32,
    base: Settings,
    x: &[f32],
    change: Option<(u64, Value)>,
    label: &str,
) -> (Vec<bool>, u64) {
    let changes: Vec<_> = change
        .iter()
        .map(|&(p, v)| rc::change(Stream::Record, p, v, 1))
        .collect();
    let r = rc::record_run(&mut standard(base), x, Partition::Fixed(1024), &changes);
    let stream = rc::device_stream(base, x);
    let switched_settings = change.map(|(_, v)| rc::constant_settings(&base, v));
    let k = change.map_or(0, |(p, _)| rc::k_of(p, rate));
    let decisions =
        rc::reference_decisions(&base, switched_settings.as_ref().map(|s| (s, k)), &stream);
    let tag = format!("{rate} Hz {label}");
    if base.vas.mode == VasMode::Mute {
        // Every device sample is stored; a muted one as +0.0.
        assert!(r.samples.len() >= stream.len(), "{tag}: the stored length");
        for (i, (&d, &kept)) in r.samples.iter().zip(&decisions).enumerate() {
            let want = if kept { stream[i] as f32 } else { 0.0 };
            assert_eq!(
                d.to_bits(),
                want.to_bits(),
                "{tag}: device sample {i} (kept {kept})"
            );
        }
        // The regions: every start, and the length of every region closed inside the stream.
        let want = rc::regions_of(&decisions);
        let got: Vec<(u64, u64)> = r
            .events
            .iter()
            .map(|e| (e.output_position, e.input_length))
            .collect();
        assert_eq!(
            got.len(),
            want.len(),
            "{tag}: the region count {got:?} vs {want:?}"
        );
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.0, w.0, "{tag}: a region's start");
            if w.0 + w.1 < stream.len() as u64 {
                assert_eq!(g.1, w.1, "{tag}: a closed region's length");
            }
        }
    } else {
        let kept: Vec<f32> = stream
            .iter()
            .zip(&decisions)
            .filter(|(_, k)| **k)
            .map(|(&d, _)| d as f32)
            .collect();
        assert!(r.samples.len() >= kept.len(), "{tag}: the stored length");
        for (i, (&d, &w)) in r.samples.iter().zip(&kept).enumerate() {
            assert_eq!(d.to_bits(), w.to_bits(), "{tag}: kept sample {i}");
        }
        // The splices: at each resume, the kept count before and the dropped run.
        let mut want: Vec<(u64, u64)> = Vec::new();
        let (mut kept_n, mut dropped, mut prev) = (0u64, 0u64, true);
        for &k in &decisions {
            if k && !prev {
                want.push((kept_n, dropped));
                dropped = 0;
            }
            if k {
                kept_n += 1
            } else {
                dropped += 1
            }
            prev = k;
        }
        let got: Vec<(u64, u64)> = r
            .events
            .iter()
            .filter(|e| e.output_position < kept_n)
            .map(|e| (e.output_position, e.input_length))
            .collect();
        assert_eq!(got, want, "{tag}: the splice events");
    }
    (decisions, k)
}

/// AS4 (FR-003, FR-007, SC-002): a sensitivity change during a passage between the two
/// thresholds: the gate's decisions from k(P) equal the reference gate's given the new
/// threshold at k(P); no kept sample before k(P) moves; the events agree.
#[test]
fn as4_gate_decision_change() {
    at(|rate| {
        let fs = f64::from(rate);
        // −43 dBFS: below the level-3 threshold (−40), above the level-5 one (−46).
        let x = stimulus::tone_at_phase(
            1000.0,
            -43.0,
            stimulus::aligned_phase(1000.0, rate, 0),
            fs,
            3 * rate as usize,
        );
        let p = 2 * u64::from(rate);
        for mode in [VasMode::Drop, VasMode::Mute] {
            let base = front(rate, mode);
            let (decisions, k) = gate_matches_reference(
                rate,
                base,
                &x,
                Some((p, Value::Sensitivity(5))),
                &format!("{mode:?} sensitivity 3 → 5"),
            );
            let paused_before = decisions[..k as usize].iter().filter(|&&d| !d).count();
            let resumed_after = decisions[k as usize..].iter().filter(|&&d| d).count();
            assert!(
                paused_before > 0 && resumed_after > 0,
                "{rate} Hz {mode:?}: the change matters ({paused_before} dropped before, {resumed_after} kept after)"
            );
            // The no-change run matches the reference too (the oracle's own check).
            let _ = gate_matches_reference(rate, base, &x, None, &format!("{mode:?} no change"));
            println!(
                "{rate:>5} Hz {mode:?}: sensitivity 3 → 5 at k(P) = {k}: decisions equal the reference gate on {} device samples",
                decisions.len()
            );
        }
    });
}

/// AS5 (FR-003, FR-007): a hang shortening with the silent count already past the new hang
/// pauses at k(P) (a non-sound sample); an onset shortening while paused with the run past
/// the new length resumes at the next sound sample; both equal the reference gate.
#[test]
fn as5_hang_and_onset_counts() {
    at(|rate| {
        let fs = f64::from(rate);
        for mode in [VasMode::Drop, VasMode::Mute] {
            // The hang: 0.5 s of tone, then silence; the change lands 400 device samples into
            // the silence (the count 400 > 300 = 50 ms).
            let base = front(rate, mode);
            let x = stimulus::burst_gap(1000.0, -20.0, &[(true, 0.5), (false, 2.0)], fs);
            let p = ((3000 + 400) as u128 * u128::from(rate)).div_ceil(6000) as u64;
            let (decisions, k) = gate_matches_reference(
                rate,
                base,
                &x,
                Some((p, Value::Hang(50.0))),
                &format!("{mode:?} hang 1000 → 50 ms"),
            );
            let first_drop = decisions.iter().position(|&d| !d).expect("a pause") as u64;
            assert_eq!(first_drop, k, "{rate} Hz {mode:?}: the pause at k(P)");
            // The onset: a 50 ms hang, 0.3 s of tone, 1.0 s of silence, tone again; the change
            // lands 50 device samples into the second burst (the run 50 > 30 = 5 ms).
            let mut short = base;
            short.vas.hang_ms = 50.0;
            let x =
                stimulus::burst_gap(1000.0, -20.0, &[(true, 0.3), (false, 1.0), (true, 1.0)], fs);
            let k_target = (1.3 * 6000.0) as u128 + 50;
            let p = (k_target * u128::from(rate)).div_ceil(6000) as u64;
            let (decisions, k) = gate_matches_reference(
                rate,
                short,
                &x,
                Some((p, Value::Onset(5.0))),
                &format!("{mode:?} onset 20 → 5 ms"),
            );
            let resume = decisions
                .iter()
                .enumerate()
                .skip(7800)
                .find(|(_, d)| **d)
                .map(|(i, _)| i as u64)
                .expect("a resume");
            // The decisions equal the reference gate's (above); the resume follows k(P) once the
            // run passes the new length (the conversion's delay moves the tone's arrival by a
            // few device samples at the converted rates).
            assert!(
                resume >= k && resume < k + 300,
                "{rate} Hz {mode:?}: the resume follows k(P) = {k} (got {resume})"
            );
            println!(
                "{rate:>5} Hz {mode:?}: hang shortening pauses at k(P) = {first_drop}; onset shortening resumes at {resume} (k(P) = {k})"
            );
        }
    });
}

/// AS6 (FR-003, FR-007, SC-002): the hiss transition. At the tap after stage 1 with silent
/// input, every transition sample's ratio to the old run is the smoothstep's within one f32
/// ULP and the output from P + N − 1 is the new run's bit for bit; at the whole engine the
/// band level 1 s after P is within 0.1 dB of the new run's.
#[test]
fn as6_hiss_transition() {
    at(|rate| {
        let fs = f64::from(rate);
        let n = volume_transition_samples(rate).expect("a supported rate") as usize;
        let p = rate as usize;
        let x = stimulus::silence(3 * rate as usize);
        let mut capsule = configs::settings("mic_capsule_only", rate);
        capsule.mic.hiss_offset_db = 0.0;
        assert_eq!(capsule.tap, Tap::AfterMicCapsule);
        let old = rc::one_pass_run(&mut standard(capsule), &x, Partition::Fixed(1024), &[]).out;
        let new = rc::one_pass_run(
            &mut standard(rc::constant_settings(&capsule, Value::Hiss(12.0))),
            &x,
            Partition::Fixed(1024),
            &[],
        )
        .out;
        let sched = rc::one_pass_run(
            &mut standard(capsule),
            &x,
            Partition::Fixed(1024),
            &[rc::change(Stream::OnePass, p as u64, Value::Hiss(12.0), 1)],
        )
        .out;
        assert!(
            same(&sched[..p], &old[..p]),
            "{rate} Hz: before P equal to the old run"
        );
        let sigma = HISS_SIGMA[rate_index(rate)];
        let (g_from, g_to) = (rc::hiss_gain(sigma, 0.0), rc::hiss_gain(sigma, 12.0));
        let mismatches = rc::hiss_ratio_mismatches(&old, &sched, g_from, g_to, p, n);
        assert_eq!(mismatches, 0, "{rate} Hz: the transition's ratio");
        assert!(
            same(&sched[p + n - 1..], &new[p + n - 1..]),
            "{rate} Hz: bit-identical to the new run from P + N − 1 at {:?}",
            cc::first_diff(&sched[p + n - 1..], &new[p + n - 1..])
        );
        assert!(
            !same(&sched[p..p + n - 1], &new[p..p + n - 1]),
            "{rate} Hz: the transition is not a step"
        );
        // The whole engine, the voice activation and the codec bypassed.
        let mut whole = front(rate, VasMode::Drop);
        whole.vas.enabled = false;
        let band = |y: &[f32]| {
            analysis::band_level_aes17(
                &y[p + rate as usize..p + 2 * rate as usize],
                fs,
                300.0,
                3000.0,
            )
        };
        let w_new = rc::one_pass_run(
            &mut standard(rc::constant_settings(&whole, Value::Hiss(12.0))),
            &x,
            Partition::Fixed(1024),
            &[],
        )
        .out;
        let w_sched = rc::one_pass_run(
            &mut standard(whole),
            &x,
            Partition::Fixed(1024),
            &[rc::change(Stream::OnePass, p as u64, Value::Hiss(12.0), 1)],
        )
        .out;
        let (b_new, b_sched) = (band(&w_new), band(&w_sched));
        assert!(
            (b_new - b_sched).abs() <= 0.1,
            "{rate} Hz: the whole engine's band level {b_sched:.3} vs {b_new:.3} dBFS"
        );
        // The codec on: reported.
        let mut codec = whole;
        codec.speech_codec_enabled = true;
        let c_new = rc::one_pass_run(
            &mut standard(rc::constant_settings(&codec, Value::Hiss(12.0))),
            &x,
            Partition::Fixed(1024),
            &[],
        )
        .out;
        let c_sched = rc::one_pass_run(
            &mut standard(codec),
            &x,
            Partition::Fixed(1024),
            &[rc::change(Stream::OnePass, p as u64, Value::Hiss(12.0), 1)],
        )
        .out;
        let (c_b_new, c_b_sched) = (band(&c_new), band(&c_sched));
        // The coder's slow gain adaptation carries the transition for about 1.5 s (reported).
        let c_last = (0..c_new.len().min(c_sched.len()))
            .rev()
            .find(|&i| c_new[i].to_bits() != c_sched[i].to_bits())
            .map_or(0.0, |i| (i as f64 - p as f64) / fs);
        println!(
            "{rate:>5} Hz: hiss 0 → +12 dB: transition of {n} samples, ratio mismatches 0, bit-identical from P + N − 1; whole engine 300 – 3000 Hz 1 s after P {b_sched:.3} dBFS (new run {b_new:.3}); codec on {c_b_sched:.3} dBFS (new run {c_b_new:.3}; the coder's state differs until {c_last:.2} s after P; reported)"
        );
    });
}

/// AS7 (FR-003): a position already passed lands on the next input sample, the entry at the
/// stream's input position at the call's entry, on both streams.
#[test]
fn as7_passed_position() {
    at(|rate| {
        let x = voiced(rate, 0.5);
        let base = front(rate, VasMode::Drop);
        // The one-pass: 1000 samples processed, then a change at 100.
        let mut p = standard(base);
        let mut y = vec![0.0f32; 1000];
        let _ = p.process(&x[..1000], &mut y).expect("call");
        assert_eq!(p.input_position(Stream::OnePass).expect("clock"), 1000);
        p.schedule_change(Stream::OnePass, 100, Value::Knee(-5.0).control(), -5.0, 7)
            .expect("schedule");
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let info = p
            .process_with_events_and_changes(&x[1000..2000], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 1);
        assert_eq!(
            (log[0].position, log[0].tag, log[0].after),
            (1000, 7, -5.0),
            "{rate} Hz: the one-pass"
        );
        // The recorder: 1000 samples fed, then a change at 100.
        let mut q = standard(base);
        q.start_recording().expect("start");
        let _ = q.record(&x[..1000], &mut []).expect("record");
        assert_eq!(q.input_position(Stream::Record).expect("clock"), 1000);
        q.schedule_change(Stream::Record, 100, Value::Hang(50.0).control(), 50.0, 8)
            .expect("schedule");
        let info = q
            .record_with_changes(&x[1000..2000], &mut [], &mut log)
            .expect("record");
        assert_eq!(info.changes, 1);
        assert_eq!(
            (log[0].position, log[0].tag, log[0].after),
            (1000, 8, 50.0),
            "{rate} Hz: the recorder"
        );
        assert_eq!(info.input_samples_total, 2000);
        let _ = q.stop_recording(&mut []).expect("stop");
        assert_eq!(
            q.input_position(Stream::Record).expect("clock"),
            0,
            "idle reads 0"
        );
        println!(
            "{rate:>5} Hz: AS7 a passed position lands on the next input sample on both streams"
        );
    });
}

/// AS8 (FR-002): the event bound holds through a hang shortening landing inside a block, read
/// before the call; and on the one-pass after the hang landed on the recorder.
#[test]
fn as8_event_bound() {
    at(|rate| {
        let fs = f64::from(rate);
        // A 1 s block (hardware review B1): with the hang shortened to 50 ms a block holds about
        // five splices, while a bound computed from the stage's 1 s hang alone would allow two.
        let block = rate as usize;
        let stale_bound = (block as u128 * 6000 / u128::from(rate)) / 6002 + 2;
        let segments: Vec<(bool, f64)> = (0..15)
            .flat_map(|_| [(true, 0.06), (false, 0.14)])
            .collect();
        let x = stimulus::burst_gap(1000.0, -20.0, &segments, fs);
        let base = front(rate, VasMode::Drop);
        let mut p = standard(base);
        let p0 = (block / 2) as u64 + 3;
        p.schedule_change(Stream::Record, p0, Value::Hang(50.0).control(), 50.0, 1)
            .expect("schedule");
        p.start_recording().expect("start");
        let (mut total, mut max_len, mut max_events) = (0usize, 0usize, 0usize);
        for chunk in x.chunks(block) {
            let len = p.max_record_events(chunk.len());
            let mut ev = vec![VasEvent::default(); len];
            let info = p.record(chunk, &mut ev).expect("record");
            assert!(
                info.events <= len,
                "{rate} Hz: {} events against a bound of {len}",
                info.events
            );
            total += info.events;
            max_len = max_len.max(len);
            max_events = max_events.max(info.events);
        }
        let _ = p.stop_recording(&mut []).expect("stop");
        assert!(
            total > 3,
            "{rate} Hz: the shortened hang produced splices ({total})"
        );
        assert!(
            max_events as u128 > stale_bound,
            "{rate} Hz: the check has teeth: {max_events} events in one block against a stale bound of {stale_bound}"
        );
        // The cross-stream case: the hang landed on the recorder; the one-pass's bound read
        // before its next call holds and its first call logs the landing.
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let (mut total1, mut first, mut max1) = (0usize, true, 0usize);
        for chunk in x.chunks(block) {
            let len = p.max_events(chunk.len());
            let mut ev = vec![VasEvent::default(); len];
            let mut y = vec![0.0f32; chunk.len()];
            let info = p
                .process_with_events_and_changes(chunk, &mut y, &mut ev, &mut log)
                .expect("call");
            assert!(
                info.events <= len,
                "{rate} Hz one-pass: {} events against {len}",
                info.events
            );
            if first {
                assert_eq!(info.changes, 1, "{rate} Hz: the cross-stream landing");
                assert_eq!((log[0].position, log[0].after, log[0].tag), (0, 50.0, 1));
                first = false;
            }
            total1 += info.events;
            max1 = max1.max(info.events);
        }
        assert!(total1 > 3, "{rate} Hz: the one-pass pauses too ({total1})");
        assert!(
            max1 as u128 > stale_bound,
            "{rate} Hz one-pass: teeth ({max1} > {stale_bound})"
        );
        println!(
            "{rate:>5} Hz: AS8 the event bound (largest {max_len}) held over {total} recorder and {total1} one-pass events through the hang shortening"
        );
    });
}
