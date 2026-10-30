//! Spec 017: the live record-side values (research R-10). The oracles are the **input-0
//! identity** (a change at input position 0 of a fresh stream equals the constant
//! configuration bit for bit), the **pre-landing identity** in the kept-sample index, the
//! partition identity, the spec 016 reference gate with the same change at k(P), the limiter's
//! carried hold (the landing sample's output equals the old run's, the gain law being
//! settings-independent) and the hiss transition's ratio to the old run.

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL, record_landing};
use rr_dr60::{
    __test_hooks, Pipeline, RecordInfo, Settings, StopOutcome, StopResult, Stream, VasEvent,
};

use crate::analysis;
use crate::changes_checks::{self as cc, Change, Partition, blocks};
use crate::meters_checks::ReferenceGate;

/// One record-side value (spec 017 FR-001), by setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// `vas.sensitivity`, 1 … 5.
    Sensitivity(u8),
    /// `vas.threshold_dbfs`.
    Threshold(f32),
    /// `vas.hang_ms`.
    Hang(f32),
    /// `vas.onset_ms`.
    Onset(f32),
    /// `agc.knee_dbfs`.
    Knee(f32),
    /// `agc.attack_ms`.
    Attack(f32),
    /// `agc.release_ms`.
    Release(f32),
    /// `mic.hiss_offset_db`.
    Hiss(f32),
}

impl Value {
    /// The eight identifiers, in the slot order the engine uses.
    pub const IDENTIFIERS: [&'static str; 8] = [
        "vas.sensitivity",
        "vas.threshold_dbfs",
        "vas.hang_ms",
        "vas.onset_ms",
        "agc.knee_dbfs",
        "agc.attack_ms",
        "agc.release_ms",
        "mic.hiss_offset_db",
    ];

    /// One representative non-default value per setting (the story tests' eight).
    pub fn examples() -> [Value; 8] {
        [
            Value::Sensitivity(5),
            Value::Threshold(-50.0),
            Value::Hang(50.0),
            Value::Onset(5.0),
            Value::Knee(-5.0),
            Value::Attack(10.0),
            Value::Release(400.0),
            Value::Hiss(6.0),
        ]
    }

    /// The control manifest identifier.
    pub fn identifier(self) -> &'static str {
        Self::IDENTIFIERS[self.slot()]
    }

    /// The slot (the engine's order).
    pub fn slot(self) -> usize {
        match self {
            Value::Sensitivity(_) => 0,
            Value::Threshold(_) => 1,
            Value::Hang(_) => 2,
            Value::Onset(_) => 3,
            Value::Knee(_) => 4,
            Value::Attack(_) => 5,
            Value::Release(_) => 6,
            Value::Hiss(_) => 7,
        }
    }

    /// The control manifest index, resolved by identifier as an app must.
    pub fn control(self) -> u32 {
        cc::control_index(self.identifier())
    }

    /// The value as the schedule call and the entries carry it.
    pub fn as_f64(self) -> f64 {
        match self {
            Value::Sensitivity(v) => f64::from(v),
            Value::Threshold(v)
            | Value::Hang(v)
            | Value::Onset(v)
            | Value::Knee(v)
            | Value::Attack(v)
            | Value::Release(v)
            | Value::Hiss(v) => f64::from(v),
        }
    }

    /// Writes the value into `s`.
    pub fn write(self, s: &mut Settings) {
        match self {
            Value::Sensitivity(v) => s.vas.sensitivity = v,
            Value::Threshold(v) => s.vas.threshold_dbfs = v,
            Value::Hang(v) => s.vas.hang_ms = v,
            Value::Onset(v) => s.vas.onset_ms = v,
            Value::Knee(v) => s.agc.knee_dbfs = v,
            Value::Attack(v) => s.agc.attack_ms = v,
            Value::Release(v) => s.agc.release_ms = v,
            Value::Hiss(v) => s.mic.hiss_offset_db = v,
        }
    }

    /// The setting's current value in `s` (an entry's `before`).
    pub fn read(self, s: &Settings) -> f64 {
        match self {
            Value::Sensitivity(_) => f64::from(s.vas.sensitivity),
            Value::Threshold(_) => f64::from(s.vas.threshold_dbfs),
            Value::Hang(_) => f64::from(s.vas.hang_ms),
            Value::Onset(_) => f64::from(s.vas.onset_ms),
            Value::Knee(_) => f64::from(s.agc.knee_dbfs),
            Value::Attack(_) => f64::from(s.agc.attack_ms),
            Value::Release(_) => f64::from(s.agc.release_ms),
            Value::Hiss(_) => f64::from(s.mic.hiss_offset_db),
        }
    }

    /// The same setting at another value.
    pub fn with(self, v: f64) -> Value {
        match self {
            Value::Sensitivity(_) => Value::Sensitivity(v as u8),
            Value::Threshold(_) => Value::Threshold(v as f32),
            Value::Hang(_) => Value::Hang(v as f32),
            Value::Onset(_) => Value::Onset(v as f32),
            Value::Knee(_) => Value::Knee(v as f32),
            Value::Attack(_) => Value::Attack(v as f32),
            Value::Release(_) => Value::Release(v as f32),
            Value::Hiss(_) => Value::Hiss(v as f32),
        }
    }
}

/// A record-side change to schedule.
pub fn change(stream: Stream, position: u64, value: Value, tag: u64) -> Change {
    Change {
        stream,
        position,
        control: value.control(),
        value: value.as_f64(),
        tag,
    }
}

/// `base` with `value` written (the constant configuration of the input-0 identity).
pub fn constant_settings(base: &Settings, value: Value) -> Settings {
    let mut s = *base;
    value.write(&mut s);
    s
}

/// One recording's result (research R-10).
#[derive(Clone, Debug)]
pub struct RecordRun {
    /// The stored device samples (the codec bypassed), or empty.
    pub samples: Vec<f32>,
    /// The stored frame bytes (the codec on), or empty.
    pub frames: Vec<u8>,
    /// Every entry the `record` calls and the stop reported, in order.
    pub log: Vec<ChangeEntry>,
    /// The `record` calls' reports.
    pub infos: Vec<RecordInfo>,
    /// Every event, the `record` calls' then the stop's, in the recording's device samples.
    pub events: Vec<VasEvent>,
    /// The stop's report.
    pub stop: StopOutcome,
    /// The recording's number.
    pub number: u32,
    /// The largest `changes` of one call.
    pub max_per_call: usize,
}

/// Schedules `changes` (on the record stream, before the start: the schedule is kept by it),
/// records `input` at `partition` through `record_with_changes`, stops through
/// `stop_recording_with_changes` and reads the stored content.
///
/// # Panics
///
/// On a refused call, a self-stop or a discarded recording.
pub fn record_run(
    p: &mut Pipeline,
    input: &[f32],
    partition: Partition,
    changes: &[Change],
) -> RecordRun {
    for c in changes {
        p.schedule_change(c.stream, c.position, c.control, c.value, c.tag)
            .unwrap_or_else(|e| panic!("schedule {c:?}: {e}"));
    }
    p.start_recording().expect("idle engine with room");
    let (mut log_all, mut infos, mut events) = (Vec::new(), Vec::new(), Vec::new());
    let mut max_per_call = 0usize;
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut pos = 0usize;
    for n in blocks(partition, input.len()) {
        let mut ev = vec![VasEvent::default(); p.max_record_events(n)];
        let info = p
            .record_with_changes(&input[pos..pos + n], &mut ev, &mut log)
            .expect("recording");
        assert!(info.events <= ev.len(), "max_record_events too small");
        assert!(
            info.stopped.is_none(),
            "the memory filled: use a larger capacity"
        );
        assert!(info.changes <= MAX_CHANGES_PER_CALL);
        events.extend_from_slice(&ev[..info.events]);
        log_all.extend_from_slice(&log[..info.changes]);
        max_per_call = max_per_call.max(info.changes);
        infos.push(info);
        pos += n;
    }
    let mut fev =
        vec![VasEvent::default(); p.max_record_events(p.record_latency_samples() as usize + 2000)];
    let stop = p
        .stop_recording_with_changes(&mut fev, &mut log)
        .expect("recording");
    assert!(stop.flush.events <= fev.len());
    events.extend_from_slice(&fev[..stop.flush.events]);
    log_all.extend_from_slice(&log[..stop.changes]);
    max_per_call = max_per_call.max(stop.changes);
    let number = match stop.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("empty recording: {other:?}"),
    };
    RecordRun {
        samples: __test_hooks::stored_samples(p, number).to_vec(),
        frames: __test_hooks::stored_frames(p, number).to_vec(),
        log: log_all,
        infos,
        events,
        stop,
        number,
        max_per_call,
    }
}

/// The stored content as one byte sequence for bit-identity (samples or frames).
pub fn stored_bytes(r: &RecordRun) -> Vec<u8> {
    if r.frames.is_empty() {
        r.samples
            .iter()
            .flat_map(|v| v.to_bits().to_le_bytes())
            .collect()
    } else {
        r.frames.clone()
    }
}

/// The one-pass run with `changes` scheduled (the record-side ones on the input clock).
pub fn one_pass_run(
    p: &mut Pipeline,
    input: &[f32],
    partition: Partition,
    changes: &[Change],
) -> cc::Run {
    cc::run_scheduled(p, cc::Source::OnePass(input), changes, partition)
}

/// k(P) = ⌈P · 6000 ÷ rate⌉ (the published helper).
pub fn k_of(position: u64, rate: u32) -> u64 {
    record_landing(position, rate).expect("a supported rate")
}

/// The smallest device index k with ⌊k · m ÷ l⌋ ≥ P, written independently of the engine
/// (the first device sample produced from input samples at or after P).
pub fn smallest_k_due(position: u64, l: u64, m: u64) -> u64 {
    if position == 0 {
        return 0;
    }
    let mut k = 0u64;
    while (u128::from(k) * u128::from(m) / u128::from(l)) < u128::from(position) {
        k += 1;
    }
    k
}

/// Stage 4's device-rate stream (before the clamp) of a twin of `settings` over `x`, in one
/// block (the gate's input below full scale).
pub fn device_stream(settings: Settings, x: &[f32]) -> Vec<f64> {
    crate::meters_checks::stage4_stream(settings, x, x.len().max(1))
        .into_iter()
        .flatten()
        .collect()
}

/// The reference gate's decisions over `stream` with `settings`, switching to `switched` at
/// device sample `k`: `true` where the sample is kept.
pub fn reference_decisions(
    settings: &Settings,
    switched: Option<(&Settings, u64)>,
    stream: &[f64],
) -> Vec<bool> {
    let mut gate = ReferenceGate::new(settings);
    stream
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            if let Some((s, k)) = switched {
                if i as u64 == k {
                    gate.set_values(s);
                }
            }
            gate.push(d)
        })
        .collect()
}

/// The kept device samples before device sample `k` in the old configuration (research R-10):
/// the index into the recorder's stored samples before which nothing may move when a change
/// lands at k. Every sample with the gate off or in mute mode.
pub fn kept_before(settings: &Settings, stream: &[f64], k: u64) -> usize {
    if !settings.vas.enabled || settings.vas.mode == rr_dr60::VasMode::Mute {
        return (k as usize).min(stream.len());
    }
    reference_decisions(settings, None, stream)
        .iter()
        .take(k as usize)
        .filter(|&&kept| kept)
        .count()
}

/// The hiss transition's check at the tap after stage 1 (FR-007): over transition samples
/// `p … p + n − 2` the new run's sample must equal the old run's scaled by `g_j / g_from`,
/// with `g_j` spec 011's smoothstep from `g_from` to `g_to` at `j = 1 … n − 1`, within one
/// f32 ULP after narrowing; returns the count of mismatches.
pub fn hiss_ratio_mismatches(
    old: &[f32],
    new: &[f32],
    g_from: f64,
    g_to: f64,
    p: usize,
    n: usize,
) -> usize {
    let inv_n = 1.0 / n as f64;
    (1..n)
        .filter(|&j| {
            let u = j as f64 * inv_n;
            let g = g_from + (g_to - g_from) * (u * u * (3.0 - 2.0 * u));
            let want = (f64::from(old[p + j - 1]) * (g / g_from)) as f32;
            let got = new[p + j - 1];
            let ulp = f32::from_bits((want.abs().to_bits()) + 1) - want.abs();
            (got - want).abs() > ulp
        })
        .count()
}

/// The hiss gain of `settings` at `rate`: σ_rate · 10^(offset / 20), from the engine's
/// published sigma (the mic module's figures) through the harness's own arithmetic.
pub fn hiss_gain(sigma: f64, offset_db: f32) -> f64 {
    sigma * analysis::from_db(f64::from(offset_db))
}

/// The mean of 20·log10(|y| / |x|) over `[from, to)` on the samples where `|x| ≥ gate`.
pub fn ratio_db(y: &[f32], x: &[f32], from: usize, to: usize, gate: f64) -> f64 {
    let to = to.min(y.len()).min(x.len());
    let mut sum = 0.0;
    let mut n = 0usize;
    for i in from..to {
        let xi = f64::from(x[i]).abs();
        if xi >= gate {
            sum += analysis::db(f64::from(y[i]).abs() / xi);
            n += 1;
        }
    }
    assert!(n > 0, "no samples above the gate in {from}..{to}");
    sum / n as f64
}

/// The muted regions a mute-mode decision sequence implies, as `(start, length)` in device
/// samples, a region still open at the end included.
pub fn regions_of(decisions: &[bool]) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &kept) in decisions.iter().enumerate() {
        match (kept, start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push((s as u64, (i - s) as u64));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s as u64, (decisions.len() - s) as u64));
    }
    out
}
