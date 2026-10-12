//! Spec 006 checks: the record and playback paths against the golden files, the one-pass
//! processing and the sample-counting rules (research R-08; FR-003 – FR-008, FR-015, FR-016;
//! SC-002, SC-005, SC-008). Every check returns [`MeasurementResult`]s with `006/FR-xxx` and
//! trace IDs; `us1_record_play` runs them per rate and `split_matrix` at the seven rates.
//!
//! Story boundary (Principle VII): the US1 checks here run record-then-play **in one engine**
//! and never import; the memory stays at its default capacity.

use rr_dr60::{
    Error, Pipeline, Route, Settings, State, StopOutcome, StopResult, VasEvent, VasMode,
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
            Tolerance::Exact(1.0),
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
                Tolerance::Exact(1.0),
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
    let mut mute = Settings::new(rate);
    mute.vas.mode = VasMode::Mute;
    let configs: [(&'static str, Settings); 2] =
        [("default_vas", Settings::new(rate)), ("default_mute", mute)];
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
                    Tolerance::Exact(0.0),
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
                    Tolerance::Exact(1.0),
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
                        Tolerance::Exact(1.0),
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
    p.reconfigure(Settings::new(rate))
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
                Tolerance::Exact(0.0),
            )
        })
        .collect()
}

/// FR-015: the same recording played on each route is bit-identical, and the route is
/// reported.
pub fn route_identity(rate: u32) -> Vec<MeasurementResult> {
    let mut p = Pipeline::new(Settings::new(rate)).expect("supported settings");
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
            Tolerance::Exact(1.0),
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
            Tolerance::Exact(1.0),
        ),
    ]
}

/// FR-004 (A-037): a +9 dBFS burst as item 1, then a −20 dBFS tone as item 2 in the same
/// engine; item 2's playback equals a fresh engine's one-pass of the tone (the recorder was
/// reset at the start, so no limiter charge carries over).
pub fn two_items(rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = Settings::new(rate);
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
        Tolerance::Exact(1.0),
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
    p.reconfigure(configs::settings("default_agc", play_rate))
        .expect("reconfigure keeps the memory");
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
            "cross-rate level minus one-pass level",
            RULES,
            play_rate,
            "default_agc",
            label.clone(),
            analysis::db(amp_a / amp_b),
            "dB",
            Tolerance::Range(-0.1, 0.1),
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
            Tolerance::Range(-0.1, 0.1),
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
            Tolerance::Exact(0.0),
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
            Tolerance::Exact(1.0),
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
            Tolerance::AtMost(-80.0),
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
    let p = Pipeline::new(Settings::new(rate)).expect("rate");
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
        Tolerance::Range(-1.0, 1.0),
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
        let mut s = Settings::new(rate);
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
            Tolerance::Range(-1.0, 1.0),
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
    let mut p = Pipeline::new(Settings::new(rate)).expect("rate");
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
            Tolerance::Exact(1.0),
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
        p.reconfigure(Settings::new(rate)) == Err(Error::RecordingInProgress),
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
        p.reconfigure(Settings::new(rate)) == Err(Error::PlaybackInProgress),
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
