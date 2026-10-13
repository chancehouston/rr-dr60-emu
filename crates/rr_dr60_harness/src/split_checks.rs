//! Spec 006 checks: the record and playback paths against the golden files, the one-pass
//! processing and the sample-counting rules (research R-08; FR-003 – FR-008, FR-015, FR-016;
//! SC-002, SC-005, SC-008). Every check returns [`MeasurementResult`]s with `006/FR-xxx` and
//! trace IDs; `us1_record_play` runs them per rate and `split_matrix` at the seven rates.
//!
//! Story boundary (Principle VII): the US1 checks here run record-then-play **in one engine**
//! and never import; the memory stays at its default capacity.

use rr_dr60::{
    __test_hooks, Error, Pipeline, Route, Settings, State, StopOutcome, StopReason, StopResult,
    VasEvent, VasMode,
};

use crate::checks::{MeasurementResult, Tolerance};
use crate::{analysis, chain_checks, configs, golden, stimulus};

/// Record block: a prime-ish size so block boundaries fall everywhere (engineering target,
/// 006 R-08).
pub const RECORD_BLOCK: usize = 4801; // engineering target (006 R-08)
/// Playback block (engineering target, 006 R-08).
pub const PLAY_BLOCK: usize = 997; // engineering target (006 R-08)
/// The cross-rate tone: 1 kHz at −20 dBFS for 5 s, below the knee (engineering target, 006
/// US1 scenario 5; hardware review B9).
const CROSS_TONE_DBFS: f64 = -20.0; // engineering target (006 US1 AS5)
const CROSS_TONE_S: f64 = 5.0; // engineering target (006 US1 AS5)
/// The live-equivalence floor and burst (engineering targets, 006 R-08): 30 s of −60 dBFS
/// floor (10 s under coverage instrumentation) then a +6 dBFS burst.
const FLOOR_DBFS: f64 = -60.0; // engineering target (006 R-08)
const FLOOR_S: f64 = if cfg!(coverage) { 10.0 } else { 30.0 }; // engineering target (006 R-08)
const BURST_DBFS: f64 = 6.0; // engineering target (006 R-08)

/// Trace: the sample-counting rules follow from the device rate (A-026) and the design.
const RULES: &[&str] = &["A-026", "engineering target"];
/// Trace: the one-pass equivalence and the state model.
const EQUIV: &[&str] = &["A-026", "A-037", "engineering target"];
/// Trace: the exclusive states and the muted speaker (S-002), the exclusive route (S-001).
const STATES: &[&str] = &["S-002", "S-001"];

/// `(l, m)` of a host rate: `m / l` host samples per device sample (spec 004 FR-002; spec 006
/// data-model.md › Per-rate figures).
///
/// # Panics
///
/// On an unsupported rate.
pub fn l_m(rate: u32) -> (u64, u64) {
    match rate {
        6000 => (1, 1),
        8000 => (3, 4),
        16_000 => (3, 8),
        44_100 => (20, 147),
        48_000 => (1, 8),
        88_200 => (10, 147),
        96_000 => (1, 16),
        other => panic!("unsupported host rate {other}"),
    }
}

/// ⌈k·m/l⌉: the playback length of `k` stored samples at `rate` (006 FR-007).
pub fn playback_len(k: u64, rate: u32) -> u64 {
    let (l, m) = l_m(rate);
    (u128::from(k) * u128::from(m)).div_ceil(u128::from(l)) as u64
}

/// ⌈m/l⌉: one device sample's worth of host samples, rounded up (006 FR-007).
pub fn host_per_device(rate: u32) -> usize {
    let (l, m) = l_m(rate);
    m.div_ceil(l) as usize
}

/// One record-then-play run (research R-08).
#[derive(Clone, Debug)]
pub struct Run {
    /// The playback before the drain: exactly ⌈K·m/l⌉ samples.
    pub pre_drain: Vec<f32>,
    /// The drain: `drain_samples()` samples.
    pub drain: Vec<f32>,
    /// The VAS events the `record` calls reported, in the recording's device samples.
    pub record_events: Vec<VasEvent>,
    /// The events the stop flush reported (a synthesized mute-region close among them).
    pub flush_events: Vec<VasEvent>,
    /// `paused` of the last `record` call.
    pub last_paused: bool,
    /// The stop's outcome.
    pub stop: StopOutcome,
    /// The recording's number.
    pub number: u32,
}

/// Records `input` in blocks of `record_block`, stops, and plays the recording back in blocks of
/// `play_block` until the engine reports the end. The engine must be idle with room.
///
/// # Panics
///
/// On a refused call, a memory-full stop or an empty recording.
pub fn record_then_play(
    p: &mut Pipeline,
    input: &[f32],
    record_block: usize,
    play_block: usize,
) -> Run {
    let record_block = record_block.max(1);
    p.start_recording().expect("idle engine with room");
    let mut record_events = Vec::new();
    let mut last_paused = false;
    let mut ev = vec![VasEvent::default(); p.max_record_events(record_block)];
    for chunk in input.chunks(record_block) {
        let info = p.record(chunk, &mut ev).expect("recording");
        assert!(info.events <= ev.len(), "max_record_events too small");
        record_events.extend_from_slice(&ev[..info.events]);
        last_paused = info.paused;
        assert!(
            info.stopped.is_none(),
            "the memory filled: use a larger capacity"
        );
    }
    let mut fev =
        vec![VasEvent::default(); p.max_record_events(p.record_latency_samples() as usize) + 1];
    let stop = p.stop_recording(&mut fev).expect("recording");
    let flush_events = fev[..stop.flush.events.min(fev.len())].to_vec();
    let number = match stop.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("empty recording: {other:?}"),
    };
    let mut out = play_to_end(p, number, play_block);
    let drain = out.split_off(out.len() - p.drain_samples() as usize);
    Run {
        pre_drain: out,
        drain,
        record_events,
        flush_events,
        last_paused,
        stop,
        number,
    }
}

/// Plays recording `number` to the end in blocks of `block`; every block before the last is
/// full.
///
/// # Panics
///
/// On a refused call or a short block before the end.
pub fn play_to_end(p: &mut Pipeline, number: u32, block: usize) -> Vec<f32> {
    p.start_playback(number)
        .expect("idle engine, existing recording");
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block.max(1)];
    loop {
        let info = p.play(&mut buf).expect("playing");
        out.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
        assert_eq!(info.produced, buf.len(), "a short block before the end");
    }
    out
}

/// Records `x` in one block and stops; the recording's number.
fn record_one(p: &mut Pipeline, x: &[f32]) -> u32 {
    p.start_recording().expect("idle");
    let info = p.record(x, &mut []).expect("recording");
    assert!(info.stopped.is_none());
    match p.stop_recording(&mut []).expect("recording").result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    }
}

/// The recorder's device-domain events as the one-pass reports them in host output samples
/// (plan review P4, running sums): position = ⌈kept·m/l⌉; a drop-mode length is
/// ⌊cum·m/l⌋ − ⌊cum_prev·m/l⌋ (spec 003 R-06's cumulative floor); a mute-mode length is
/// ⌈end·m/l⌉ − ⌈start·m/l⌉ (003 R-09).
pub fn host_events(events: &[VasEvent], rate: u32, mute: bool) -> Vec<VasEvent> {
    let (l, m) = l_m(rate);
    let ceil = |d: u64| (u128::from(d) * u128::from(m)).div_ceil(u128::from(l)) as u64;
    let floor = |d: u64| (u128::from(d) * u128::from(m) / u128::from(l)) as u64;
    let (mut cum, mut reported) = (0u64, 0u64);
    events
        .iter()
        .map(|e| {
            let mut h = VasEvent::default();
            h.output_position = ceil(e.output_position);
            h.input_length = if mute {
                ceil(e.output_position + e.input_length) - ceil(e.output_position)
            } else {
                cum += e.input_length;
                let removed = floor(cum);
                let len = removed - reported;
                reported = removed;
                len
            };
            h
        })
        .collect()
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[allow(clippy::too_many_arguments)]
fn row(
    requirement: &'static str,
    property: &'static str,
    trace: &'static [&'static str],
    rate: u32,
    config: &'static str,
    stimulus: impl Into<String>,
    measured: f64,
    unit: &'static str,
    tolerance: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(
        requirement,
        property,
        trace,
        rate,
        config,
        stimulus,
        measured,
        unit,
        tolerance,
    )
}

fn pass(ok: bool) -> f64 {
    f64::from(u8::from(ok))
}

/// The one-pass output of `x` (one block).
fn one_pass(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(settings).expect("supported settings");
    let mut y = vec![0.0f32; x.len()];
    let n = p.process(x, &mut y).expect("equal lengths").produced;
    y.truncate(n);
    y
}

/// `x` followed by the prefix rule's zeros: the record latency plus one device sample
/// (006 FR-007).
fn with_tail(x: &[f32], p: &Pipeline, rate: u32) -> Vec<f32> {
    let tail = p.record_latency_samples() as usize + host_per_device(rate);
    let mut v = x.to_vec();
    v.extend(std::iter::repeat_n(0.0f32, tail));
    v
}

/// SC-002 (FR-007): every applicable golden entry at `rate` is reproduced by record-then-play
/// in one engine: the first `n_samples` of the pre-drain playback hash to the entry's SHA-256,
/// and for the VAS entries the recorder's events, converted to host positions with running
/// sums, and the last `record` call's `paused` equal the entry's.
pub fn golden_equivalence(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    for a in golden::applicable_entries()
        .into_iter()
        .filter(|a| a.host_rate_hz == rate)
    {
        let x = golden::stimulus(a.stimulus, rate);
        let mut p = Pipeline::new(a.settings).expect("supported settings");
        let run = record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
        let n = a.entry.n_samples as usize;
        let long_enough = run.pre_drain.len() >= n;
        let same = long_enough
            && golden::entry(a.stimulus, a.config, rate, &run.pre_drain[..n]).sha256
                == a.entry.sha256;
        let label = format!("{} ({})", a.stimulus, a.file);
        rows.push(row(
            "006/FR-007",
            "golden entry reproduced by record-then-play",
            EQUIV,
            rate,
            a.config,
            label.clone(),
            pass(same),
            "",
            Tolerance::Exact(1.0), // FR-007
        ));
        if let Some(vas) = &a.entry.vas {
            let mute = a.settings.vas.mode == VasMode::Mute;
            let got: Vec<[u64; 2]> = host_events(&run.record_events, rate, mute)
                .iter()
                .map(|e| [e.output_position, e.input_length])
                .collect();
            rows.push(row(
                "006/FR-006",
                "golden VAS events and paused state reproduced",
                EQUIV,
                rate,
                a.config,
                label,
                pass(got == vas.events && run.last_paused == vas.paused_at_end),
                "",
                Tolerance::Exact(1.0), // FR-006
            ));
        }
    }
    rows
}

/// The live stimulus set (research R-08): the golden stimuli, the speech-like pattern over a
/// −60 dBFS floor, and the floor followed by a loud burst.
pub fn live_stimuli(rate: u32) -> Vec<(String, Vec<f32>)> {
    let fs = f64::from(rate);
    let mut set: Vec<(String, Vec<f32>)> = golden::STIMULI
        .iter()
        .map(|s| ((*s).to_string(), golden::stimulus(s, rate)))
        .collect();
    // Fixed PRNG seeds (research R-08; spec 001 FR-014 determinism): distinct from the golden
    // and chain seeds so the live set is its own stimulus, identical on every run.
    let (speech, _) =
        chain_checks::speech_like_pattern(rate, chain_checks::BURST_DBFS, FLOOR_DBFS, 0x0D60);
    set.push(("speech_floor".into(), speech));
    let floor = stimulus::bandlimited_noise(0x0D61, (FLOOR_S * fs) as usize, fs, FLOOR_DBFS);
    let burst = stimulus::tone_at_phase(
        1000.0,
        BURST_DBFS,
        stimulus::peak_phase(1000.0, rate),
        fs,
        rate as usize,
    );
    set.push(("silence_burst".into(), [floor, burst].concat()));
    set
}

/// FR-007: record-then-play equals the one-pass of the input followed by the record latency
/// plus one device sample of zeros (prefix rule), with the playback length exactly ⌈K·m/l⌉,
/// for the live stimulus set with everything on in drop and in mute mode, in two partitions
/// (4801 / 997 and 1 / 1). The two partitions agree with each other too (FR-018).
pub fn live_equivalence(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let mut mute = crate::configs::settings_007(rate);
    mute.vas.mode = VasMode::Mute;
    let configs: [(&'static str, Settings); 2] = [
        ("default_vas", crate::configs::settings_007(rate)),
        ("default_mute", mute),
    ];
    for (name, x) in live_stimuli(rate) {
        for (config, settings) in configs {
            let probe = Pipeline::new(settings).expect("supported settings");
            let want = one_pass(settings, &with_tail(&x, &probe, rate));
            let mut first: Option<Vec<f32>> = None;
            for (rb, pb) in [(RECORD_BLOCK, PLAY_BLOCK), (1, 1)] {
                let mut p = Pipeline::new(settings).expect("supported settings");
                let run = record_then_play(&mut p, &x, rb, pb);
                let StopResult::Kept { length: k, .. } = run.stop.result else {
                    unreachable!()
                };
                let label = format!("{name}, blocks {rb}/{pb}");
                rows.push(row(
                    "006/FR-007",
                    "playback length minus ceil(K*m/l)",
                    RULES,
                    rate,
                    config,
                    label.clone(),
                    run.pre_drain.len() as f64 - playback_len(k, rate) as f64,
                    "samples",
                    Tolerance::Exact(0.0), // FR-007
                ));
                let prefix_ok = want.len() >= run.pre_drain.len()
                    && same_bits(&want[..run.pre_drain.len()], &run.pre_drain);
                rows.push(row(
                    "006/FR-007",
                    "record-then-play equals the one-pass prefix",
                    EQUIV,
                    rate,
                    config,
                    label.clone(),
                    pass(prefix_ok),
                    "",
                    Tolerance::Exact(1.0), // FR-007
                ));
                match &first {
                    None => first = Some(run.pre_drain.clone()),
                    Some(f) => rows.push(row(
                        "006/FR-018",
                        "partitions give identical playback",
                        RULES,
                        rate,
                        config,
                        label,
                        pass(same_bits(f, &run.pre_drain)),
                        "",
                        Tolerance::Exact(1.0), // FR-018
                    )),
                }
            }
        }
    }
    rows
}

/// FR-007 (hardware review B3): K = 1 … 50 samples recorded at 6 kHz with every stage
/// bypassed (one stored sample per input sample, no flush) into an engine then reconfigured to
/// `rate` (the memory is kept, FR-019) play back as exactly ⌈K·m/l⌉ + `drain_samples()`.
pub fn length_rule(rate: u32) -> Vec<MeasurementResult> {
    let mut p = Pipeline::new(configs::settings("bypass_all", 6000)).expect("6 kHz");
    for k in 1..=50u32 {
        let x: Vec<f32> = (0..k).map(|i| 0.5 - i as f32 / 100.0).collect();
        assert_eq!(record_one(&mut p, &x), k);
        assert_eq!(p.recording_length(k), Ok(u64::from(k)));
    }
    p.reconfigure(crate::configs::settings_007(rate))
        .expect("reconfigure keeps the memory");
    let drain = u64::from(p.drain_samples());
    (1..=50u32)
        .map(|k| {
            let out = play_to_end(&mut p, k, PLAY_BLOCK);
            row(
                "006/FR-007",
                "playback length minus ceil(K*m/l) + drain",
                RULES,
                rate,
                "bypass_all@6k then default",
                format!("K = {k}"),
                out.len() as f64 - (playback_len(u64::from(k), rate) + drain) as f64,
                "samples",
                Tolerance::Exact(0.0), // FR-007
            )
        })
        .collect()
}

/// FR-015: the same recording played on each route is bit-identical, and the route is
/// reported.
pub fn route_identity(rate: u32) -> Vec<MeasurementResult> {
    let mut p = Pipeline::new(crate::configs::settings_007(rate)).expect("supported settings");
    let x = golden::stimulus("sweep_log", rate);
    let n = record_one(&mut p, &x);
    p.set_route(Route::Speaker);
    let speaker = play_to_end(&mut p, n, PLAY_BLOCK);
    p.set_route(Route::Earphone);
    let reported = p.state().route == Route::Earphone;
    let earphone = play_to_end(&mut p, n, PLAY_BLOCK);
    vec![
        row(
            "006/FR-015",
            "speaker and earphone playback identical",
            STATES,
            rate,
            "default_vas",
            "sweep_log",
            pass(same_bits(&speaker, &earphone)),
            "",
            Tolerance::Exact(1.0), // FR-015
        ),
        row(
            "006/FR-015",
            "route readable after set_route",
            STATES,
            rate,
            "default_vas",
            "sweep_log",
            pass(reported && p.route() == Route::Earphone),
            "",
            Tolerance::Exact(1.0), // FR-015
        ),
    ]
}

/// FR-004 (A-037): a +9 dBFS burst as item 1, then a −20 dBFS tone as item 2 in the same
/// engine; item 2's playback equals a fresh engine's one-pass of the tone (the recorder was
/// reset at the start, so no limiter charge carries over).
pub fn two_items(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = crate::configs::settings_007(rate);
    let mut p = Pipeline::new(settings).expect("supported settings");
    let phase = stimulus::peak_phase(1000.0, rate);
    let loud = stimulus::tone_at_phase(1000.0, 9.0, phase, fs, rate as usize);
    let quiet = stimulus::tone_at_phase(1000.0, -20.0, phase, fs, rate as usize);
    assert_eq!(record_one(&mut p, &loud), 1);
    assert_eq!(record_one(&mut p, &quiet), 2);
    let item2 = play_to_end(&mut p, 2, PLAY_BLOCK);
    let pre = &item2[..item2.len() - p.drain_samples() as usize];
    let want = one_pass(settings, &with_tail(&quiet, &p, rate));
    let same = want.len() >= pre.len() && same_bits(&want[..pre.len()], pre);
    vec![row(
        "006/FR-004",
        "item 2 after a loud item 1 equals a fresh one-pass",
        &["A-037"],
        rate,
        "default_vas",
        "+9 dBFS 1 s, then -20 dBFS 1 s",
        pass(same),
        "",
        Tolerance::Exact(1.0), // FR-004
    )]
}

/// US1 scenario 5 (hardware review B9): a −20 dBFS 1 kHz tone, 5 s, VAS off, recorded at
/// `record_rate` and played at `play_rate` (the engine reconfigured between, keeping the
/// memory), against the one-pass of the same tone at `play_rate`: level within ±0.1 dB and
/// frequency within ±0.1 Hz by a least-squares sinusoid fit over the last second.
pub fn cross_rate_tone(record_rate: u32, play_rate: u32) -> Vec<MeasurementResult> {
    let tone = |rate: u32| {
        stimulus::tone(
            1000.0,
            stimulus::amplitude(CROSS_TONE_DBFS),
            f64::from(rate),
            (CROSS_TONE_S * f64::from(rate)) as usize,
        )
    };
    let mut p = Pipeline::new(configs::settings("default_agc", record_rate)).expect("rate");
    let n = record_one(&mut p, &tone(record_rate));
    let length_before = p.recording_length(n).expect("recorded");
    p.reconfigure(configs::settings("default_agc", play_rate))
        .expect("reconfigure keeps the memory");
    let length_after = p.recording_length(n).expect("kept across reconfigure");
    let out = play_to_end(&mut p, n, PLAY_BLOCK);
    let pre = &out[..out.len() - p.drain_samples() as usize];
    let fs = f64::from(play_rate);
    let last = play_rate as usize;
    let (amp_a, f_a) = analysis::fit_sinusoid(&pre[pre.len() - last..], fs, 1000.0);
    let want = one_pass(
        configs::settings("default_agc", play_rate),
        &tone(play_rate),
    );
    let (amp_b, f_b) = analysis::fit_sinusoid(&want[want.len() - last..], fs, 1000.0);
    let label = format!("1 kHz -20 dBFS 5 s, {record_rate} -> {play_rate} Hz");
    vec![
        row(
            "006/FR-007",
            "recording length after reconfigure minus before (device samples)",
            RULES,
            play_rate,
            "default_agc",
            label.clone(),
            length_after as f64 - length_before as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-007, US1 scenario 5: the stored length is independent of the rate
        ),
        row(
            "006/FR-007",
            "cross-rate level minus one-pass level",
            RULES,
            play_rate,
            "default_agc",
            label.clone(),
            analysis::db(amp_a / amp_b),
            "dB",
            Tolerance::Range(-0.1, 0.1), // FR-007
        ),
        row(
            "006/FR-007",
            "cross-rate frequency minus one-pass frequency",
            RULES,
            play_rate,
            "default_agc",
            label,
            f_a - f_b,
            "Hz",
            Tolerance::Range(-0.1, 0.1), // FR-007
        ),
    ]
}

/// FR-008 (hardware review B2): a 0 dBFS 1 kHz tone recorded with the VAS bypassed and
/// stopped on a positive peak; the drain is exactly `drain_samples()` long, `finished` comes
/// only then, and the drain's last 1 ms lies below −80 dBFS.
pub fn drain_level(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let mut x = stimulus::tone_at_phase(
        1000.0,
        0.0,
        stimulus::peak_phase(1000.0, rate),
        fs,
        rate as usize,
    );
    // Cut after the largest sample of the last cycle: a positive peak.
    let period = (fs / 1000.0).ceil() as usize + 1;
    let start = x.len() - period;
    let (peak_at, _) = x[start..]
        .iter()
        .enumerate()
        .fold((0, f32::MIN), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
    x.truncate(start + peak_at + 1);
    let mut p = Pipeline::new(configs::settings("default_agc", rate)).expect("rate");
    let n = record_one(&mut p, &x);
    // Play with a block that is not a divisor of the length, and watch `finished`.
    p.start_playback(n).expect("idle");
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; PLAY_BLOCK];
    let mut finished_once = 0;
    loop {
        let info = p.play(&mut buf).expect("playing");
        out.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            finished_once += 1;
            break;
        }
    }
    let drain_len = p.drain_samples() as usize;
    let k = p.recording_length(n).expect("exists");
    let expected_total = playback_len(k, rate) as usize + drain_len;
    let drain = &out[out.len().saturating_sub(drain_len)..];
    let ms = (rate / 1000) as usize;
    let level = analysis::level_dbfs_aes17(&drain[drain.len() - ms..]);
    vec![
        row(
            "006/FR-008",
            "produced minus ceil(K*m/l) + drain",
            RULES,
            rate,
            "default_agc",
            "1 kHz 0 dBFS cut on a peak",
            out.len() as f64 - expected_total as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-008
        ),
        row(
            "006/FR-008",
            "finished reported once, at the end",
            RULES,
            rate,
            "default_agc",
            "1 kHz 0 dBFS cut on a peak",
            pass(finished_once == 1 && p.state().state == State::Idle),
            "",
            Tolerance::Exact(1.0), // FR-008
        ),
        row(
            "006/FR-008",
            "drain's last 1 ms level",
            RULES,
            rate,
            "default_agc",
            "1 kHz 0 dBFS cut on a peak",
            level,
            "dBFS",
            Tolerance::AtMost(-80.0), // FR-008
        ),
    ]
}

/// SC-008 (FR-016): the two reported latencies sum to the one-pass latency within one sample,
/// and the reported playback latency matches the playback path's **1 kHz group delay** within
/// ±1 host sample, with stage 10 on and off, measured as the one-pass latency is (004 FR-004):
/// tones at 999 and 1001 Hz, each stored exactly by a 6 kHz `bypass_all` recording and played
/// at `rate` after a reconfigure, against the same tones sampled at the host rate. The
/// playback onset peak (argmax |y| of a one-sample recording, plan review P8) is reported for
/// information: a minimum-phase low-pass's impulse-response peak sits 0.2–0.3 device samples
/// after its 1 kHz group delay, which is 2–3 host samples at 44.1, 88.2 and 96 kHz, so it is
/// not the figure the engine reports (spec Clarifications, implementation session).
pub fn latency_sum(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let p = Pipeline::new(crate::configs::settings_007(rate)).expect("rate");
    let sum = f64::from(p.record_latency_samples()) + f64::from(p.playback_latency_samples());
    rows.push(row(
        "006/FR-016",
        "record + playback latency minus one-pass latency",
        RULES,
        rate,
        "default_vas",
        "reported figures",
        sum - f64::from(p.latency_samples()),
        "samples",
        Tolerance::Range(-1.0, 1.0), // FR-016
    ));
    // Recordings 1 and 2: 1 s tones at 999 and 1001 Hz, stored exactly (6 kHz, bypass_all,
    // no flush); recording 3: one sample, for the onset peak.
    let fs = f64::from(rate);
    let freqs = [999.0, 1001.0];
    let mut p = Pipeline::new(configs::settings("bypass_all", 6000)).expect("6 kHz");
    for (i, &f) in freqs.iter().enumerate() {
        let n = record_one(&mut p, &stimulus::tone(f, 0.25, 6000.0, 6000));
        assert_eq!(n as usize, i + 1);
    }
    assert_eq!(record_one(&mut p, &[1.0]), 3);
    for stage10 in [true, false] {
        let mut s = crate::configs::settings_007(rate);
        s.playback_stage_enabled = stage10;
        p.reconfigure(s).expect("reconfigure keeps the memory");
        let reported = f64::from(p.playback_latency_samples());
        let config = if stage10 {
            "default_vas"
        } else {
            "stage 10 off"
        };
        let mut measure = |f: f64| {
            let n = if f < 1000.0 { 1 } else { 2 };
            let out = play_to_end(&mut p, n, PLAY_BLOCK);
            let pre = &out[..out.len() - p.drain_samples() as usize];
            let reference = stimulus::tone(f, 0.25, fs, pre.len());
            // Skip the first fifth of the tone (its onset transient, ≫ the path's settling)
            // and fit the phase over the rest (engineering target; spec 004 FR-004's method).
            analysis::gain_and_phase(&reference, pre, f, fs, pre.len() / 5)
        };
        // The group delay from the phase slope between 999 and 1001 Hz (004 FR-004's method).
        let (lo, hi) = (measure(999.0), measure(1001.0));
        let measured = -(hi / lo).arg() / (std::f64::consts::TAU * 2.0 / fs);
        rows.push(row(
            "006/FR-016",
            "reported playback latency minus measured 1 kHz group delay",
            RULES,
            rate,
            config,
            "stored tones 999/1001 Hz",
            reported - measured,
            "samples",
            Tolerance::Range(-1.0, 1.0), // FR-016
        ));
        let out = play_to_end(&mut p, 3, PLAY_BLOCK);
        let (argmax, _) =
            out.iter().enumerate().fold(
                (0, -1.0f32),
                |m, (i, &v)| if v.abs() > m.1 { (i, v.abs()) } else { m },
            );
        rows.push(row(
            "006/FR-016",
            "playback onset peak minus reported playback latency (report only)",
            RULES,
            rate,
            config,
            "one-sample recording",
            argmax as f64 - reported,
            "samples",
            Tolerance::Report,
        ));
    }
    rows
}

/// SC-005 (FR-003; Edge Cases): every (state, call) pair with its reason, the state unchanged
/// after a refusal, `speaker_muted` only while recording, `current` while recording and
/// playing.
pub fn state_table(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let mut p = Pipeline::new(crate::configs::settings_007(rate)).expect("rate");
    let x = golden::stimulus("tone_1k_m20", rate);
    let mut y = vec![0.0f32; x.len()];
    let check = |rows: &mut Vec<MeasurementResult>, what: &'static str, ok: bool| {
        rows.push(row(
            "006/FR-003",
            what,
            STATES,
            rate,
            "default_vas",
            "state table",
            pass(ok),
            "",
            Tolerance::Exact(1.0), // FR-003
        ));
    };
    // Idle.
    let st = p.state();
    check(
        &mut rows,
        "idle: state, not muted, no current",
        st.state == State::Idle && !st.speaker_muted && st.current.is_none(),
    );
    check(
        &mut rows,
        "idle: record -> NotRecording",
        p.record(&x, &mut []) == Err(Error::NotRecording),
    );
    check(
        &mut rows,
        "idle: stop_recording -> NotRecording",
        p.stop_recording(&mut []) == Err(Error::NotRecording),
    );
    check(
        &mut rows,
        "idle: play -> NotPlaying",
        p.play(&mut y) == Err(Error::NotPlaying),
    );
    check(
        &mut rows,
        "idle: stop_playback -> NotPlaying",
        p.stop_playback() == Err(Error::NotPlaying),
    );
    check(
        &mut rows,
        "idle: start_playback(0) -> NoSuchRecording",
        p.start_playback(0) == Err(Error::NoSuchRecording { number: 0 }),
    );
    check(
        &mut rows,
        "idle: start_playback(1) with none -> NoSuchRecording",
        p.start_playback(1) == Err(Error::NoSuchRecording { number: 1 }),
    );
    // Recording.
    p.start_recording().expect("idle");
    let st = p.state();
    check(
        &mut rows,
        "recording: state, muted, current = count + 1",
        st.state == State::Recording && st.speaker_muted && st.current == Some(1),
    );
    check(
        &mut rows,
        "recording: start_recording -> RecordingInProgress",
        p.start_recording() == Err(Error::RecordingInProgress),
    );
    check(
        &mut rows,
        "recording: start_playback -> RecordingInProgress",
        p.start_playback(1) == Err(Error::RecordingInProgress),
    );
    check(
        &mut rows,
        "recording: one-pass process -> RecordingInProgress",
        p.process(&x, &mut y) == Err(Error::RecordingInProgress),
    );
    check(
        &mut rows,
        "recording: reconfigure -> RecordingInProgress",
        p.reconfigure(crate::configs::settings_007(rate)) == Err(Error::RecordingInProgress),
    );
    check(
        &mut rows,
        "recording: play -> NotPlaying",
        p.play(&mut y) == Err(Error::NotPlaying),
    );
    check(
        &mut rows,
        "recording: state unchanged after refusals",
        p.state().state == State::Recording,
    );
    let _ = p.record(&x, &mut []).expect("recording");
    let stop = p.stop_recording(&mut []).expect("recording");
    check(
        &mut rows,
        "stop: kept as number 1, idle, unmuted",
        matches!(stop.result, StopResult::Kept { number: 1, .. })
            && p.state().state == State::Idle
            && !p.state().speaker_muted,
    );
    // Playing.
    p.start_playback(1).expect("idle");
    let st = p.state();
    check(
        &mut rows,
        "playing: state, not muted, current = 1, route reported",
        st.state == State::Playing
            && !st.speaker_muted
            && st.current == Some(1)
            && st.route == Route::Speaker,
    );
    check(
        &mut rows,
        "playing: start_recording -> PlaybackInProgress",
        p.start_recording() == Err(Error::PlaybackInProgress),
    );
    check(
        &mut rows,
        "playing: start_playback -> PlaybackInProgress",
        p.start_playback(1) == Err(Error::PlaybackInProgress),
    );
    check(
        &mut rows,
        "playing: one-pass process -> PlaybackInProgress",
        p.process(&x, &mut y) == Err(Error::PlaybackInProgress),
    );
    check(
        &mut rows,
        "playing: reconfigure -> PlaybackInProgress",
        p.reconfigure(crate::configs::settings_007(rate)) == Err(Error::PlaybackInProgress),
    );
    check(
        &mut rows,
        "playing: record -> NotRecording",
        p.record(&x, &mut []) == Err(Error::NotRecording),
    );
    check(
        &mut rows,
        "playing: state unchanged after refusals",
        p.state().state == State::Playing,
    );
    p.set_route(Route::Earphone);
    check(
        &mut rows,
        "playing: route change takes effect and is reported",
        p.state().route == Route::Earphone && p.play(&mut y).is_ok(),
    );
    let mut finished = false;
    while !finished {
        finished = p.play(&mut y).expect("playing").finished;
    }
    check(
        &mut rows,
        "end of playback: idle, play -> NotPlaying",
        p.state().state == State::Idle && p.play(&mut y) == Err(Error::NotPlaying),
    );
    // Reset in each state keeps the memory.
    p.start_recording().expect("idle");
    let _ = p.record(&x, &mut []).expect("recording");
    p.reset();
    check(
        &mut rows,
        "reset while recording: idle, recording discarded, memory kept",
        p.state().state == State::Idle && p.memory_status().count == 1,
    );
    p.start_playback(1).expect("idle");
    p.reset();
    check(
        &mut rows,
        "reset while playing: idle, memory and route kept",
        p.state().state == State::Idle
            && p.memory_status().count == 1
            && p.route() == Route::Earphone,
    );
    rows
}

// ---- User Story 2: the memory's limits, numbering and erase (006 T028) ----

/// The trace for the limit rows: the device's figures and the two memory assumptions.
const LIMITS: &[&str] = &["S-001", "A-038", "A-039"];
/// The small capacity the US2 scenarios record to the limit with (006 FR-010; research R-11:
/// never the default 60 minutes in a debug test).
const SMALL_CAPACITY_S: u32 = 10; // engineering target (006 US2 scenarios 2–6)
const SMALL_MAX: u32 = 3; // engineering target (006 US2 scenario 4)
/// Input longer than the small capacity (US2 scenario 2: 25 s against 10 s).
const OVER_S: f64 = if cfg!(coverage) { 12.0 } else { 25.0 }; // engineering target (006 US2)
const LIMIT_TONE_DBFS: f64 = -20.0; // engineering target (006 US2 scenario 2)
/// Capacity above the device's: 600 minutes and 999 recordings (US2 scenario 8).
const EXPANDED_CAPACITY_S: u32 = 36_000; // engineering target (006 US2 scenario 8)
const EXPANDED_MAX: u32 = 999; // engineering target (006 US2 scenario 8)
const DEVICE_RATE: f64 = 6000.0; // A-026

/// What a block-wise recording of a stimulus did.
struct Recorded {
    /// Device samples stored, all blocks and the flush included.
    stored: u64,
    /// The reason when the recording stopped by itself.
    stopped: Option<StopReason>,
    /// Host samples fed before the stop (whole blocks; the stopping block counted).
    consumed: usize,
    /// The stop outcome when the host stopped the recording (None after a self-stop).
    outcome: Option<StopOutcome>,
}

/// Starts a recording, feeds `x` in [`RECORD_BLOCK`] blocks until it ends or the recording
/// stops by itself, and stops it when still recording.
fn record_blocks(p: &mut Pipeline, x: &[f32]) -> Recorded {
    p.start_recording().expect("idle, room and a slot");
    let mut r = Recorded {
        stored: 0,
        stopped: None,
        consumed: 0,
        outcome: None,
    };
    for block in x.chunks(RECORD_BLOCK) {
        let info = p.record(block, &mut []).expect("recording");
        r.stored += info.stored as u64;
        r.consumed += block.len();
        if let Some(reason) = info.stopped {
            r.stopped = Some(reason);
            return r;
        }
    }
    let outcome = p.stop_recording(&mut []).expect("recording");
    r.stored += outcome.flush.stored as u64;
    r.stopped = outcome.flush.stopped;
    r.outcome = Some(outcome);
    r
}

#[allow(clippy::too_many_arguments)]
fn limit_row(
    requirement: &'static str,
    property: &'static str,
    rate: u32,
    config: &'static str,
    stimulus: impl Into<String>,
    measured: f64,
    unit: &'static str,
    tolerance: Tolerance,
) -> MeasurementResult {
    row(
        requirement,
        property,
        LIMITS,
        rate,
        config,
        stimulus,
        measured,
        unit,
        tolerance,
    )
}

/// US2 scenarios 2 and 6 (FR-012, SC-003): `over_s` seconds of a −20 dBFS 1 kHz tone with the
/// VAS off into a `capacity_s` memory: the recording stops by itself at exactly
/// `capacity_s × 6000` stored samples with the reason "memory full", the partial recording is
/// kept with that length, 0 samples remain, a start is refused with "memory full", and after an
/// erase a start proceeds.
pub fn capacity_time_at(
    rate: u32,
    capacity_s: u32,
    max: u32,
    over_s: f64,
) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let capacity = u64::from(capacity_s) * DEVICE_RATE as u64;
    let s = configs::memory_capacity(configs::settings("default_agc", rate), capacity_s, max);
    let mut p = Pipeline::new(s).expect("rate");
    let x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        fs,
        (over_s * fs) as usize,
    );
    let r = record_blocks(&mut p, &x);
    let label = format!("{over_s} s tone into {capacity_s} s");
    let st = p.memory_status();
    let mut rows = vec![
        limit_row(
            "006/FR-012",
            "stored samples at the self-stop minus the capacity",
            rate,
            "default_agc",
            label.clone(),
            r.stored as f64 - capacity as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-012: the exact sample
        ),
        limit_row(
            "006/FR-012",
            "stop reason is memory full",
            rate,
            "default_agc",
            label.clone(),
            pass(r.stopped == Some(StopReason::MemoryFull) && r.outcome.is_none()),
            "",
            Tolerance::Exact(1.0), // FR-012
        ),
        limit_row(
            "006/FR-012",
            "input consumed before the self-stop",
            rate,
            "default_agc",
            label.clone(),
            r.consumed as f64 / fs,
            "s",
            // The stop lies inside the block that holds input second `capacity_s` plus the
            // record latency (FR-016).
            Tolerance::Range(
                f64::from(capacity_s),
                f64::from(capacity_s)
                    + (RECORD_BLOCK as f64 + f64::from(p.record_latency_samples())) / fs,
            ),
        ),
        limit_row(
            "006/FR-012",
            "idle after the self-stop, recording kept as number 1",
            rate,
            "default_agc",
            label.clone(),
            pass(p.state().state == State::Idle && st.count == 1),
            "",
            Tolerance::Exact(1.0), // FR-012
        ),
        limit_row(
            "006/FR-012",
            "kept length minus the capacity",
            rate,
            "default_agc",
            label.clone(),
            p.recording_length(1)
                .map_or(f64::NAN, |l| l as f64 - capacity as f64),
            "samples",
            Tolerance::Exact(0.0), // FR-012
        ),
        limit_row(
            "006/FR-011",
            "remaining samples after the self-stop",
            rate,
            "default_agc",
            label.clone(),
            st.remaining_samples as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-011
        ),
        limit_row(
            "006/FR-012",
            "start refused with memory full",
            rate,
            "default_agc",
            label.clone(),
            pass(p.start_recording() == Err(Error::MemoryFull) && p.state().state == State::Idle),
            "",
            Tolerance::Exact(1.0), // FR-012, US2 scenario 6
        ),
    ];
    p.erase(1).expect("idle, exists");
    let proceeds = p.start_recording().is_ok() && p.state().current == Some(1);
    p.reset();
    rows.push(limit_row(
        "006/FR-013",
        "after an erase the start proceeds",
        rate,
        "default_agc",
        label,
        pass(proceeds && p.memory_status().remaining_samples == capacity),
        "",
        Tolerance::Exact(1.0), // FR-013, US2 scenario 6
    ));
    rows
}

/// [`capacity_time_at`] with the small capacity and [`OVER_S`] of input.
pub fn capacity_time(rate: u32) -> Vec<MeasurementResult> {
    capacity_time_at(rate, SMALL_CAPACITY_S, SMALL_MAX, OVER_S)
}

/// A tone of `tone_s` seconds followed by silence to `total_s` seconds, at `fs`.
fn tone_then_silence(tone_s: f64, total_s: f64, fs: f64) -> Vec<f32> {
    let mut x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        fs,
        (tone_s * fs) as usize,
    );
    x.resize((total_s * fs) as usize, 0.0);
    x
}

/// US2 scenario 3 (FR-011, FR-012; A-023, A-025): into a 10 s memory, 25 s of input of which
/// the VAS pauses most. Drop mode: a 5 s tone then silence stores the tone and the hang only
/// and never stops; a 12 s tone then silence stops at exactly the capacity during the tone.
/// Mute mode: a 5 s tone then silence stops at exactly the capacity, 10 s into the input (the
/// muted silence is stored and counts).
pub fn capacity_vas(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let capacity = u64::from(SMALL_CAPACITY_S) * DEVICE_RATE as u64;
    let engine = |mode: VasMode| {
        let mut s = configs::settings("default_vas", rate);
        s.vas.mode = mode;
        Pipeline::new(configs::memory_capacity(s, SMALL_CAPACITY_S, SMALL_MAX)).expect("rate")
    };
    let mut rows = Vec::new();
    // (a) Drop mode, 5 s tone: paused time costs nothing.
    let mut p = engine(VasMode::Drop);
    let hang_device = f64::from(p.settings().vas.hang_ms) / 1000.0 * DEVICE_RATE; // A-023
    let onset_s = f64::from(p.settings().vas.onset_ms) / 1000.0; // A-024
    let r = record_blocks(&mut p, &tone_then_silence(5.0, OVER_S, fs));
    let label_a = format!("drop: 5 s tone then silence to {OVER_S} s");
    rows.push(limit_row(
        "006/FR-011",
        "drop mode: no self-stop, recording kept",
        rate,
        "default_vas",
        label_a.clone(),
        pass(
            r.stopped.is_none()
                && matches!(
                    r.outcome,
                    Some(StopOutcome {
                        result: StopResult::Kept { number: 1, .. },
                        ..
                    })
                )
                && p.state().state == State::Idle,
        ),
        "",
        Tolerance::Exact(1.0), // FR-012, US2 scenario 3
    ));
    rows.push(limit_row(
        "006/FR-011",
        "drop mode: stored minus the tone (paused time costs nothing)",
        rate,
        "default_vas",
        label_a.clone(),
        r.stored as f64 - 5.0 * DEVICE_RATE,
        "samples",
        // The hang after the tone (A-023) plus the onset slack (A-024) and one device sample.
        Tolerance::Range(0.0, hang_device + onset_s * DEVICE_RATE + 1.0),
    ));
    rows.push(limit_row(
        "006/FR-011",
        "drop mode: remaining samples",
        rate,
        "default_vas",
        label_a,
        p.memory_status().remaining_samples as f64,
        "samples",
        // The capacity less the tone, less at most the hang (A-023), the onset slack (A-024)
        // and one device sample (FR-011).
        Tolerance::Range(
            capacity as f64 - 5.0 * DEVICE_RATE - (hang_device + onset_s * DEVICE_RATE + 1.0),
            capacity as f64 - 5.0 * DEVICE_RATE,
        ),
    ));
    // (b) Drop mode, 12 s tone: stops during the tone at exactly the capacity.
    let mut p = engine(VasMode::Drop);
    let r = record_blocks(&mut p, &tone_then_silence(12.0, OVER_S, fs));
    let label_b = format!("drop: 12 s tone then silence to {OVER_S} s");
    rows.push(limit_row(
        "006/FR-012",
        "drop mode: stored at the self-stop minus the capacity",
        rate,
        "default_vas",
        label_b.clone(),
        r.stored as f64 - capacity as f64,
        "samples",
        Tolerance::Exact(0.0), // FR-012
    ));
    rows.push(limit_row(
        "006/FR-012",
        "drop mode: stopped with memory full inside the tone",
        rate,
        "default_vas",
        label_b,
        pass(
            r.stopped == Some(StopReason::MemoryFull)
                && r.consumed as f64 / fs <= 12.0 + RECORD_BLOCK as f64 / fs,
        ),
        "",
        Tolerance::Exact(1.0), // FR-012
    ));
    // (c) Mute mode, 5 s tone: the muted silence is stored and the memory fills at 10 s.
    let mut p = engine(VasMode::Mute);
    let r = record_blocks(&mut p, &tone_then_silence(5.0, OVER_S, fs));
    let label_c = format!("mute: 5 s tone then silence to {OVER_S} s");
    rows.push(limit_row(
        "006/FR-012",
        "mute mode: stored at the self-stop minus the capacity",
        rate,
        "default_vas",
        label_c.clone(),
        r.stored as f64 - capacity as f64,
        "samples",
        Tolerance::Exact(0.0), // FR-012
    ));
    rows.push(limit_row(
        "006/FR-011",
        "mute mode: input consumed before the self-stop",
        rate,
        "default_vas",
        label_c,
        if r.stopped == Some(StopReason::MemoryFull) {
            r.consumed as f64 / fs
        } else {
            f64::NAN
        },
        "s",
        // Inside the block holding input second 10 plus the record latency (FR-016).
        Tolerance::Range(
            f64::from(SMALL_CAPACITY_S),
            f64::from(SMALL_CAPACITY_S)
                + (RECORD_BLOCK as f64 + f64::from(p.record_latency_samples())) / fs,
        ),
    ));
    rows
}

/// US2 scenario 4 (FR-012; S-001): with 3 slots, the 4th start is refused with "no recording
/// slots left" while time remains; the figures are exact.
pub fn capacity_count(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let s = configs::memory_capacity(
        configs::settings("default_agc", rate),
        SMALL_CAPACITY_S,
        SMALL_MAX,
    );
    let mut p = Pipeline::new(s).expect("rate");
    let x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        fs,
        fs as usize,
    );
    let mut numbers = Vec::new();
    for _ in 0..SMALL_MAX {
        numbers.push(record_one(&mut p, &x));
    }
    let st = p.memory_status();
    let label = format!("{SMALL_MAX} × 1 s tone into {SMALL_MAX} slots");
    let refused = p.start_recording();
    vec![
        limit_row(
            "006/FR-013",
            "recordings numbered 1 to the count in order",
            rate,
            "default_agc",
            label.clone(),
            pass(numbers == (1..=SMALL_MAX).collect::<Vec<_>>() && st.count == SMALL_MAX),
            "",
            Tolerance::Exact(1.0), // FR-013
        ),
        limit_row(
            "006/FR-011",
            "slots remaining",
            rate,
            "default_agc",
            label.clone(),
            f64::from(st.slots_remaining),
            "",
            Tolerance::Exact(0.0), // FR-011, US2 scenario 4
        ),
        limit_row(
            "006/FR-012",
            "4th start refused with no recording slots while time remains",
            rate,
            "default_agc",
            label,
            pass(
                refused == Err(Error::NoRecordingSlots)
                    && st.remaining_samples > 0
                    && p.state().state == State::Idle,
            ),
            "",
            Tolerance::Exact(1.0), // FR-012, US2 scenario 4
        ),
    ]
}

/// US2 scenario 5 (FR-013; S-001): five recordings; erase 3 → four, the former 4 and 5 play
/// bit-identically as 3 and 4, 1 and 2 unchanged, the erased time returns to remaining; erase
/// all → the figures of scenario 1 at this capacity.
pub fn erase_renumber(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let s = configs::memory_capacity(configs::settings("default_agc", rate), SMALL_CAPACITY_S, 5);
    let mut p = Pipeline::new(s).expect("rate");
    let capacity = u64::from(SMALL_CAPACITY_S) * DEVICE_RATE as u64;
    let mut before = Vec::new();
    for n in 1..=5u32 {
        let x = stimulus::tone(
            1000.0,
            stimulus::amplitude(LIMIT_TONE_DBFS - f64::from(n)),
            fs,
            (0.5 * fs) as usize,
        );
        assert_eq!(record_one(&mut p, &x), n);
        before.push(play_to_end(&mut p, n, PLAY_BLOCK));
    }
    let st0 = p.memory_status();
    let erased_len = p.recording_length(3).expect("exists");
    p.erase(3).expect("idle, exists");
    let st1 = p.memory_status();
    let label = "five 0.5 s tones, erase 3, then all";
    let mut rows = vec![
        limit_row(
            "006/FR-013",
            "count after erasing 3 of 5",
            rate,
            "default_agc",
            label,
            f64::from(st1.count),
            "",
            Tolerance::Exact(4.0), // FR-013
        ),
        limit_row(
            "006/FR-013",
            "remaining grows by the erased length",
            rate,
            "default_agc",
            label,
            (st1.remaining_samples - st0.remaining_samples) as f64 - erased_len as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013
        ),
    ];
    for (new, old) in [(1u32, 0usize), (2, 1), (3, 3), (4, 4)] {
        let same = same_bits(&play_to_end(&mut p, new, PLAY_BLOCK), &before[old]);
        rows.push(limit_row(
            "006/FR-013",
            "renumbered recording plays bit-identically",
            rate,
            "default_agc",
            format!("{label}: new {new} = former {}", old + 1),
            pass(same),
            "",
            Tolerance::Exact(1.0), // FR-013
        ));
    }
    rows.push(limit_row(
        "006/FR-013",
        "the former 5 is gone",
        rate,
        "default_agc",
        label,
        pass(p.recording_length(5) == Err(Error::NoSuchRecording { number: 5 })),
        "",
        Tolerance::Exact(1.0), // FR-013
    ));
    p.erase_all().expect("idle");
    let st = p.memory_status();
    rows.push(limit_row(
        "006/FR-011",
        "after erase all: count, used, remaining = capacity, slots = max",
        rate,
        "default_agc",
        label,
        pass(
            st.count == 0
                && st.used_samples == 0
                && st.remaining_samples == capacity
                && st.slots_remaining == 5,
        ),
        "",
        Tolerance::Exact(1.0), // FR-011, US2 scenarios 1 and 5
    ));
    rows
}

/// US2 scenario 7 (FR-012; A-025, A-039): a start and stop with no input is discarded; a half
/// second of silence is not empty (the VAS starts unpaused and its hang outlasts it, and the
/// flush stores zeros).
pub fn empty_discarded(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let s = configs::memory_capacity(
        configs::settings("default_vas", rate),
        SMALL_CAPACITY_S,
        SMALL_MAX,
    );
    let mut p = Pipeline::new(s).expect("rate");
    p.start_recording().expect("idle");
    let out = p.stop_recording(&mut []).expect("recording");
    let st = p.memory_status();
    let mut rows = vec![limit_row(
        "006/FR-012",
        "stop without input: discarded, no flush, count unchanged",
        rate,
        "default_vas",
        "start, stop",
        pass(
            out.result == StopResult::DiscardedEmpty
                && out.flush.stored == 0
                && st.count == 0
                && st.used_samples == 0,
        ),
        "",
        Tolerance::Exact(1.0), // FR-012, A-039
    )];
    let silence = vec![0.0f32; (0.5 * fs) as usize];
    let n = record_one(&mut p, &silence);
    let flush_device = (f64::from(p.record_latency_samples()) * DEVICE_RATE / fs).ceil();
    rows.push(limit_row(
        "006/FR-012",
        "0.5 s of silence: kept, length minus 3000 within the flush",
        rate,
        "default_vas",
        "start, 0.5 s silence, stop",
        p.recording_length(n)
            .map_or(f64::NAN, |l| l as f64 - 0.5 * DEVICE_RATE),
        "samples",
        Tolerance::Range(0.0, flush_device + 1.0), // A-025: within the hang, the flush stored
    ));
    rows
}

/// US2 scenario 8 (FR-010): an engine of 600 minutes and 999 recordings is created, reports
/// those figures, reserves the matching store, and records (a short recording here; the
/// recording past 60 minutes is [`expanded_capacity_past_device`], release only).
pub fn expanded_capacity(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let s = configs::memory_capacity(
        configs::settings("default_agc", rate),
        EXPANDED_CAPACITY_S,
        EXPANDED_MAX,
    );
    let mut p = Pipeline::new(s).expect("600 min reserved");
    let st = p.memory_status();
    let (store, spans) = __test_hooks::memory_capacities(&p);
    let label = format!("{EXPANDED_CAPACITY_S} s / {EXPANDED_MAX} recordings");
    let x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        fs,
        fs as usize,
    );
    let n = record_one(&mut p, &x);
    vec![
        limit_row(
            "006/FR-010",
            "expanded capacity in device samples",
            rate,
            "default_agc",
            label.clone(),
            st.capacity_samples as f64 - f64::from(EXPANDED_CAPACITY_S) * DEVICE_RATE,
            "samples",
            Tolerance::Exact(0.0), // FR-010
        ),
        limit_row(
            "006/FR-010",
            "expanded slots",
            rate,
            "default_agc",
            label.clone(),
            f64::from(st.max_recordings) - f64::from(EXPANDED_MAX),
            "",
            Tolerance::Exact(0.0), // FR-010
        ),
        limit_row(
            "006/FR-017",
            "store reserved for the expanded capacity",
            rate,
            "default_agc",
            label.clone(),
            pass(
                store as u64 == st.capacity_samples && spans as u64 == u64::from(EXPANDED_MAX) + 1,
            ),
            "",
            Tolerance::Exact(1.0), // FR-017
        ),
        limit_row(
            "006/FR-010",
            "a recording into the expanded memory is kept as number 1",
            rate,
            "default_agc",
            label,
            pass(n == 1 && p.recording_length(1).is_ok_and(|l| l >= DEVICE_RATE as u64)),
            "",
            Tolerance::Exact(1.0), // FR-010
        ),
    ]
}

/// US2 scenario 8 (FR-010), release only: 3601 s of tone at 6 kHz into the 600-minute engine
/// records past the device's 60 minutes without a stop.
pub fn expanded_capacity_past_device() -> Vec<MeasurementResult> {
    let rate = 6000;
    let s = configs::memory_capacity(
        configs::settings("default_agc", rate),
        EXPANDED_CAPACITY_S,
        EXPANDED_MAX,
    );
    let mut p = Pipeline::new(s).expect("600 min reserved");
    let over = 3601.0; // one second past the device's 60 minutes (S-001)
    let x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        DEVICE_RATE,
        (over * DEVICE_RATE) as usize,
    );
    let r = record_blocks(&mut p, &x);
    vec![
        limit_row(
            "006/FR-010",
            "past 60 minutes: no self-stop, kept",
            rate,
            "default_agc",
            "3601 s tone at 6 kHz",
            pass(
                r.stopped.is_none()
                    && matches!(
                        r.outcome,
                        Some(StopOutcome {
                            result: StopResult::Kept { number: 1, .. },
                            ..
                        })
                    ),
            ),
            "",
            Tolerance::Exact(1.0), // FR-010, US2 scenario 8
        ),
        limit_row(
            "006/FR-010",
            "past 60 minutes: stored minus the input length",
            rate,
            "default_agc",
            "3601 s tone at 6 kHz",
            r.stored as f64 - over * DEVICE_RATE,
            "samples",
            Tolerance::Range(0.0, f64::from(p.record_latency_samples())), // the flush (FR-005)
        ),
    ]
}

/// SC-007 (FR-017): the default engine reserves at most 90 MB, equal to the two `Vec`
/// capacities in bytes, and the reservation does not change through a recording to the limit
/// (at the small capacity), an erase, an erase all and a clone.
pub fn footprint() -> Vec<MeasurementResult> {
    let rate = 48_000;
    let fs = f64::from(rate);
    let p = Pipeline::new(crate::configs::settings_007(rate)).expect("default");
    let (store, spans) = __test_hooks::memory_capacities(&p);
    let st = p.memory_status();
    let mut rows = vec![
        limit_row(
            "006/FR-011",
            "default figures: 0 recordings, 0 used, 3600 s and 99 slots remaining",
            rate,
            "default_vas",
            "default capacity",
            pass(
                st.count == 0
                    && st.used_samples == 0
                    && st.remaining_samples == 21_600_000 // S-001: 60 min × 6000 Hz (A-026)
                    && st.remaining_seconds() == 3600.0
                    && st.slots_remaining == 99
                    && st.max_recordings == 99,
            ),
            "",
            Tolerance::Exact(1.0), // FR-011, US2 scenario 1
        ),
        limit_row(
            "006/SC-007",
            "default footprint",
            rate,
            "default_vas",
            "default capacity",
            p.footprint_bytes() as f64 / 1e6,
            "MB",
            Tolerance::Range(80.0, 90.0), // SC-007: the 90 MB budget
        ),
        limit_row(
            "006/SC-007",
            "footprint equals the reserved capacities in bytes",
            rate,
            "default_vas",
            "default capacity",
            pass(p.footprint_bytes() == store * 4 + spans * 16),
            "",
            Tolerance::Exact(1.0), // SC-007
        ),
    ];
    let s = configs::memory_capacity(
        configs::settings("default_agc", rate),
        SMALL_CAPACITY_S,
        SMALL_MAX,
    );
    let mut p = Pipeline::new(s).expect("rate");
    let before = (p.footprint_bytes(), __test_hooks::memory_capacities(&p));
    let x = stimulus::tone(
        1000.0,
        stimulus::amplitude(LIMIT_TONE_DBFS),
        fs,
        (OVER_S * fs) as usize,
    );
    let r = record_blocks(&mut p, &x);
    assert_eq!(r.stopped, Some(StopReason::MemoryFull));
    let same = |p: &Pipeline, what: &'static str, rows: &mut Vec<MeasurementResult>| {
        rows.push(limit_row(
            "006/SC-007",
            what,
            rate,
            "default_agc",
            "10 s capacity",
            pass((p.footprint_bytes(), __test_hooks::memory_capacities(p)) == before),
            "",
            Tolerance::Exact(1.0), // SC-007, FR-017
        ));
    };
    same(
        &p,
        "footprint unchanged after recording to the limit",
        &mut rows,
    );
    p.erase(1).expect("idle, exists");
    same(&p, "footprint unchanged after an erase", &mut rows);
    let _ = record_one(&mut p, &x[..fs as usize]);
    let _ = record_one(&mut p, &x[..fs as usize]);
    p.erase_all().expect("idle");
    same(&p, "footprint unchanged after erase all", &mut rows);
    let n = record_one(&mut p, &x[..fs as usize]);
    let _ = play_to_end(&mut p, n, PLAY_BLOCK);
    same(&p, "footprint unchanged after a playback", &mut rows);
    let c = p.clone();
    same(&c, "a clone reserves the same footprint", &mut rows);
    rows
}

// ---- User Story 3: export and import (006 T034) ----

/// The trace for the form rows.
const FORM: &[&str] = &["A-026", "A-004", "engineering target"];
/// The stimulus of the round trip and the corruption set: 2 s of the speech-like pattern over
/// the floor, so the VAS drops and splices (the stored stream is not the input).
const FORM_STIMULUS_S: f64 = 2.0; // engineering target (006 R-08)
/// Truncation boundary of the corruption set (research R-08).
const TRUNCATION_STEP: usize = 4096; // engineering target (006 R-08)
/// Single-byte flips in the corruption set (research R-08).
const FLIP_POSITIONS: usize = 64; // engineering target (006 R-08)

/// Records the form stimulus into a fresh engine at `rate` and returns the engine and the
/// recording's exported bytes.
fn recorded_form(rate: u32) -> (Pipeline, Vec<u8>) {
    let fs = f64::from(rate);
    let mut p = Pipeline::new(configs::memory_capacity(
        configs::settings("default_vas", rate),
        SMALL_CAPACITY_S,
        SMALL_MAX,
    ))
    .expect("rate");
    let (speech, _) =
        chain_checks::speech_like_pattern(rate, chain_checks::BURST_DBFS, FLOOR_DBFS, 0x0D62);
    let x = &speech[..((FORM_STIMULUS_S * fs) as usize).min(speech.len())];
    let n = record_one(&mut p, x);
    let bytes = export_bytes(&p, n);
    (p, bytes)
}

/// `export_len` sized, `export` filled; the two agree.
pub fn export_bytes(p: &Pipeline, number: u32) -> Vec<u8> {
    let len = p.export_len(number).expect("exists");
    let mut bytes = vec![0u8; len];
    let written = p.export(number, &mut bytes).expect("sized from export_len");
    assert_eq!(written, len);
    bytes
}

/// A fresh engine at `rate` with the small capacity and the VAS default.
fn fresh(rate: u32) -> Pipeline {
    Pipeline::new(configs::memory_capacity(
        configs::settings("default_vas", rate),
        SMALL_CAPACITY_S,
        SMALL_MAX,
    ))
    .expect("rate")
}

/// US3 scenarios 1 and 2, US1 scenario 5 (FR-014, SC-004): the recording made at
/// `record_rate` exports to the same bytes twice; imported into a fresh engine at each of the
/// seven rates it has the same length, and its playback is bit-identical to the original
/// engine's playback reconfigured to that rate. `mutate` transforms the bytes between export
/// and import (identity for the check; the mutants of `mutants` for the teeth).
pub fn round_trip_with(record_rate: u32, mutate: fn(&[u8]) -> Vec<u8>) -> Vec<MeasurementResult> {
    let (mut original, bytes) = recorded_form(record_rate);
    let again = export_bytes(&original, 1);
    let length = original.recording_length(1).expect("exists");
    let label = format!("recorded at {record_rate} Hz, {length} stored");
    let mut rows = vec![row(
        "006/FR-014",
        "export is identical every time",
        FORM,
        record_rate,
        "default_vas",
        label.clone(),
        pass(again == bytes),
        "",
        Tolerance::Exact(1.0), // FR-014, US3 scenario 1
    )];
    let bytes = mutate(&bytes);
    for play_rate in configs::all_rates() {
        let mut imported = fresh(play_rate);
        let number = imported.import(&bytes);
        rows.push(row(
            "006/FR-014",
            "import accepted into a fresh engine as number 1",
            FORM,
            play_rate,
            "default_vas",
            label.clone(),
            pass(number == Ok(1)),
            "",
            Tolerance::Exact(1.0), // FR-014, US3 scenario 2
        ));
        if number.is_err() {
            continue;
        }
        original
            .reconfigure(configs::memory_capacity(
                configs::settings("default_vas", play_rate),
                SMALL_CAPACITY_S,
                SMALL_MAX,
            ))
            .expect("keeps the memory");
        rows.push(row(
            "006/FR-014",
            "imported length minus the original's (unchanged by the rate)",
            FORM,
            play_rate,
            "default_vas",
            label.clone(),
            imported.recording_length(1).expect("imported") as f64
                - original.recording_length(1).expect("exists") as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-014, US1 scenario 5
        ));
        let want = play_to_end(&mut original, 1, PLAY_BLOCK);
        let got = play_to_end(&mut imported, 1, PLAY_BLOCK);
        rows.push(row(
            "006/FR-014",
            "imported playback bit-identical to the original's",
            FORM,
            play_rate,
            "default_vas",
            label.clone(),
            pass(same_bits(&got, &want)),
            "",
            Tolerance::Exact(1.0), // FR-014, SC-004
        ));
        rows.push(row(
            "006/FR-014",
            "re-export of the import equals the bytes",
            FORM,
            play_rate,
            "default_vas",
            label.clone(),
            pass(export_bytes(&imported, 1) == bytes),
            "",
            Tolerance::Exact(1.0), // FR-014, US1 scenario 5
        ));
    }
    rows
}

/// [`round_trip_with`] with the bytes untouched.
pub fn round_trip(rate: u32) -> Vec<MeasurementResult> {
    round_trip_with(rate, |b| b.to_vec())
}

/// Harness-side transforms of exported bytes (SC-002, SC-004 teeth; research R-08): each,
/// applied between export and import, must make [`round_trip_with`] fail.
pub mod mutants {
    /// Flips one bit of the trailing CRC.
    pub fn flip_crc_bit(bytes: &[u8]) -> Vec<u8> {
        let mut b = bytes.to_vec();
        let n = b.len();
        b[n - 2] ^= 0x10;
        b
    }

    /// Drops the first stored sample and re-forms the sequence (K − 1, a valid CRC), so the
    /// import succeeds with a shorter, shifted recording.
    pub fn drop_first_sample(bytes: &[u8]) -> Vec<u8> {
        let k = u64::from_le_bytes(bytes[24..32].try_into().expect("header"));
        let mut b = bytes[..32].to_vec();
        b[24..32].copy_from_slice(&(k - 1).to_le_bytes());
        b.extend_from_slice(&bytes[36..bytes.len() - 4]);
        let crc = super::crc32(&b);
        b.extend_from_slice(&crc.to_le_bytes());
        b
    }
}

/// CRC-32 (IEEE 802.3, reflected 0xEDB88320) as the contract states it; a second, independent
/// implementation of the core's for the corruption set's re-formed mutants.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// `bytes` with the CRC recomputed.
fn reformed(mut bytes: Vec<u8>) -> Vec<u8> {
    let n = bytes.len();
    let c = crc32(&bytes[..n - 4]);
    bytes[n - 4..].copy_from_slice(&c.to_le_bytes());
    bytes
}

/// US3 scenarios 3 and 4 (FR-014, SC-004): every sequence of the corruption set is refused
/// with the stated reason and leaves the memory's figures unchanged; afterwards the untouched
/// bytes still import.
pub fn corruption_set(rate: u32) -> Vec<MeasurementResult> {
    let (_, good) = recorded_form(rate);
    let n = good.len();
    let mut cases: Vec<(String, Vec<u8>, Error)> = Vec::new();
    // Truncation at every 4 096-byte boundary and at the last byte.
    let mut at = TRUNCATION_STEP;
    while at < n {
        let want = if at < 8 {
            Error::UnknownForm
        } else {
            Error::Damaged
        };
        cases.push((
            format!("truncated to {at} bytes"),
            good[..at].to_vec(),
            want,
        ));
        at += TRUNCATION_STEP;
    }
    cases.push((
        "truncated by the last byte".into(),
        good[..n - 1].to_vec(),
        Error::Damaged,
    ));
    // A single-byte flip at 64 evenly spaced positions (the first and last bytes included),
    // plus one in every header field (version, flags, rate, library version, reserved, the
    // low and high bytes of K) and one inside the CRC.
    let flip = |pos: usize| {
        let mut b = good.clone();
        b[pos] ^= 0x40;
        let want = match pos {
            0..8 => Error::UnknownForm,
            8..10 => Error::UnsupportedFormVersion {
                version: u16::from_le_bytes([b[8], b[9]]),
            },
            10..12 => Error::UnsupportedFormVersion { version: 1 },
            _ => Error::Damaged,
        };
        (format!("byte {pos} flipped"), b, want)
    };
    for i in 0..FLIP_POSITIONS {
        cases.push(flip(i * (n - 1) / (FLIP_POSITIONS - 1)));
    }
    for pos in [8, 10, 12, 16, 22, 24, 31, n - 3] {
        cases.push(flip(pos));
    }
    // Foreign bytes behind a valid magic (seeded, so the expected reason is fixed).
    let mut rng = stimulus::Pcg32::new(0x0D63, 7);
    let mut foreign = good[..8].to_vec();
    foreign.extend((0..1000).map(|_| rng.below(256) as u8));
    let version = u16::from_le_bytes([foreign[8], foreign[9]]);
    let flags = u16::from_le_bytes([foreign[10], foreign[11]]);
    let want = if version != 1 || flags != 0 {
        Error::UnsupportedFormVersion { version }
    } else {
        Error::Damaged
    };
    cases.push(("foreign bytes with a valid magic".into(), foreign, want));
    let mut v2 = good.clone();
    v2[8] = 2;
    cases.push((
        "version 2".into(),
        v2,
        Error::UnsupportedFormVersion { version: 2 },
    ));
    let mut flags = good.clone();
    flags[10] = 1;
    cases.push((
        "flags 1".into(),
        flags,
        Error::UnsupportedFormVersion { version: 1 },
    ));
    cases.push((
        "the 20-byte header".into(),
        good[..20].to_vec(),
        Error::Damaged,
    ));
    let mut huge = good.clone();
    huge[24..32].copy_from_slice(&(1u64 << 62).to_le_bytes());
    cases.push(("K = 2^62".into(), reformed(huge), Error::Damaged));
    let mut nan = good.clone();
    nan[32..36].copy_from_slice(&f32::NAN.to_le_bytes());
    cases.push((
        "NaN sample, valid CRC".into(),
        reformed(nan),
        Error::Damaged,
    ));
    let mut sub = good.clone();
    sub[32..36].copy_from_slice(&1.0e-40f32.to_le_bytes());
    cases.push((
        "subnormal sample, valid CRC".into(),
        reformed(sub),
        Error::Damaged,
    ));
    let mut zero = good[..32].to_vec();
    zero[24..32].copy_from_slice(&0u64.to_le_bytes());
    zero.extend_from_slice(&[0; 4]);
    cases.push(("K = 0".into(), reformed(zero), Error::Damaged));
    cases.push(("empty sequence".into(), Vec::new(), Error::UnknownForm));

    let mut rows = Vec::new();
    let mut p = fresh(rate);
    let before = p.memory_status();
    for (name, bytes, want) in &cases {
        let got = p.import(bytes);
        let unchanged = p.memory_status() == before && p.state().state == State::Idle;
        rows.push(row(
            "006/FR-014",
            "corrupt sequence refused with the stated reason, memory unchanged",
            FORM,
            rate,
            "default_vas",
            format!("{name} -> {want:?}"),
            pass(got.as_ref() == Err(want) && unchanged),
            "",
            Tolerance::Exact(1.0), // FR-014, SC-004
        ));
    }
    // Oversized for the remaining time (1 s memory, a 2 s recording) and for the slots.
    let mut tiny = Pipeline::new(configs::memory_capacity(
        configs::settings("default_vas", rate),
        1,
        SMALL_MAX,
    ))
    .expect("rate");
    let before = tiny.memory_status();
    let time = tiny.import(&good);
    rows.push(row(
        "006/FR-014",
        "oversized for the remaining time: does not fit, memory unchanged",
        FORM,
        rate,
        "default_vas",
        format!("{n} bytes into 1 s"),
        pass(time == Err(Error::DoesNotFit) && tiny.memory_status() == before),
        "",
        Tolerance::Exact(1.0), // FR-014, US3 scenario 4
    ));
    let mut one_slot = Pipeline::new(configs::memory_capacity(
        configs::settings("default_vas", rate),
        SMALL_CAPACITY_S,
        1,
    ))
    .expect("rate");
    assert_eq!(one_slot.import(&good), Ok(1));
    let before = one_slot.memory_status();
    let slot = one_slot.import(&good);
    rows.push(row(
        "006/FR-014",
        "no slot left: does not fit, memory unchanged",
        FORM,
        rate,
        "default_vas",
        "second import into one slot",
        pass(slot == Err(Error::DoesNotFit) && one_slot.memory_status() == before),
        "",
        Tolerance::Exact(1.0), // FR-014, US3 scenario 4
    ));
    rows.push(row(
        "006/FR-014",
        "the untouched bytes still import after the refusals",
        FORM,
        rate,
        "default_vas",
        format!("{} refusals", cases.len()),
        pass(p.import(&good) == Ok(1)),
        "",
        Tolerance::Exact(1.0), // FR-014
    ));
    rows
}
