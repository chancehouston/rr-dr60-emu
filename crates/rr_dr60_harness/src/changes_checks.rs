//! Spec 015: sample-timed control changes and the change log (research R-08). The primary
//! oracle is **the plain setter at block size 1**: version 0.15's own behavior defines where a
//! change lands when the setter is called right after output P − 1; a change scheduled at P at
//! any partition must give the same output bit for bit. The other oracles are spec 011's splice
//! identity, spec 009's route run and spec 013's reference (`speed_checks`).

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL};
use rr_dr60::{Pipeline, Route, Stream, VasEvent};

use crate::stimulus::Pcg32;

/// The control manifest's index of `identifier` (the engine pins the three live indices; the
/// harness resolves them by identifier as an app must, research R-04).
pub fn control_index(identifier: &str) -> u32 {
    rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == identifier)
        .expect("a control of the manifest") as u32
}

/// The route control's index.
pub fn route_control() -> u32 {
    control_index("route")
}

/// The volume control's index.
pub fn volume_control() -> u32 {
    control_index("volume")
}

/// The playback speed control's index.
pub fn speed_control() -> u32 {
    control_index("playback_speed")
}

/// A change to schedule (data-model.md › Scheduled change).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    /// The stream.
    pub stream: Stream,
    /// The position in the stream's clock.
    pub position: u64,
    /// The control manifest's index.
    pub control: u32,
    /// The control's value (a route 0/1, a volume position, a level 1 … 5).
    pub value: f64,
    /// The caller's tag (from 1).
    pub tag: u64,
}

impl Change {
    /// A route change.
    pub fn route(stream: Stream, position: u64, route: Route, tag: u64) -> Self {
        Self {
            stream,
            position,
            control: route_control(),
            value: if route == Route::Earphone { 1.0 } else { 0.0 },
            tag,
        }
    }

    /// A volume change.
    pub fn volume(stream: Stream, position: u64, volume: f32, tag: u64) -> Self {
        Self {
            stream,
            position,
            control: volume_control(),
            value: f64::from(volume),
            tag,
        }
    }

    /// A playback speed change.
    pub fn speed(position: u64, level: u8, tag: u64) -> Self {
        Self {
            stream: Stream::Playback,
            position,
            control: speed_control(),
            value: f64::from(level),
            tag,
        }
    }

    /// Applies this change as the plain setter would (the block-size-1 oracle).
    pub fn apply_plain(&self, p: &mut Pipeline) {
        if self.control == route_control() {
            p.set_route(if self.value == 0.0 {
                Route::Speaker
            } else {
                Route::Earphone
            });
        } else if self.control == volume_control() {
            p.set_volume(self.value as f32).expect("a valid position");
        } else {
            p.set_playback_speed(self.value as u8)
                .expect("a valid level");
        }
    }
}

/// Schedules every change, in order.
pub fn schedule_all(p: &mut Pipeline, changes: &[Change]) {
    for c in changes {
        p.schedule_change(c.stream, c.position, c.control, c.value, c.tag)
            .unwrap_or_else(|e| panic!("schedule {c:?}: {e}"));
    }
}

/// A block partition (spec 015 FR-008).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Partition {
    /// Every block `n` samples.
    Fixed(usize),
    /// [`irregular`]: a fixed, seeded sequence of lengths 1 … 4096, never cut at a position.
    Irregular,
    /// The irregular sequence split so that a block ends exactly at the given position (the
    /// all-partition boundary of US1 AS6).
    IrregularWithBoundary(u64),
}

/// The irregular partition (FR-008): a seeded sequence of block lengths 1 … 4096 over about
/// 100 000 samples, with the lengths 1, 1009 (a prime) and 2000 placed in it, shared by every
/// partition check. It is never cut at a scheduled position (hardware review P9).
pub fn irregular() -> Vec<usize> {
    let mut rng = Pcg32::new(0x0015, 7);
    let mut v = Vec::new();
    let mut total = 0usize;
    while total < 100_000 {
        let n = 1 + rng.below(4096) as usize;
        v.push(n);
        total += n;
    }
    v[3] = 1;
    v[7] = 1009;
    v[11] = 2000;
    v
}

/// The lengths of `partition` for a stream of at most `total` samples (repeating the irregular
/// sequence if needed; a boundary variant inserts the cut).
pub fn blocks(partition: Partition, total: usize) -> Vec<usize> {
    match partition {
        Partition::Fixed(n) => {
            let n = n.max(1);
            let mut v = vec![n; total / n];
            if total % n != 0 {
                v.push(total % n);
            }
            v
        }
        Partition::Irregular => fit(&irregular(), total),
        Partition::IrregularWithBoundary(at) => {
            let base = fit(&irregular(), total);
            let mut v = Vec::new();
            let mut pos = 0usize;
            for n in base {
                if pos < at as usize && pos + n > at as usize {
                    v.push(at as usize - pos);
                    v.push(pos + n - at as usize);
                } else {
                    v.push(n);
                }
                pos += n;
            }
            v
        }
    }
}

fn fit(seq: &[usize], total: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let mut pos = 0usize;
    let mut i = 0usize;
    while pos < total {
        let n = seq[i % seq.len()].min(total - pos);
        v.push(n);
        pos += n;
        i += 1;
    }
    v
}

/// What a run processes.
#[derive(Clone, Copy, Debug)]
pub enum Source<'a> {
    /// The one-pass stream over `x` (the engine as it stands; the clock continues).
    OnePass(&'a [f32]),
    /// A playback of the recording with this number, started by the run, to `finished`.
    Playback(u32),
}

/// A run's output and the concatenated change log.
#[derive(Clone, Debug, Default)]
pub struct Run {
    /// Every produced sample.
    pub out: Vec<f32>,
    /// Every entry, in call order.
    pub log: Vec<ChangeEntry>,
    /// `changes` summed over the calls.
    pub changes: usize,
    /// The largest `changes` of one call.
    pub max_per_call: usize,
}

/// Schedules `changes` and runs `source` at `partition` through the log-taking variants.
pub fn run_scheduled(
    p: &mut Pipeline,
    source: Source<'_>,
    changes: &[Change],
    partition: Partition,
) -> Run {
    schedule_all(p, changes);
    run(p, source, partition)
}

/// Runs `source` at `partition` (nothing scheduled by this call) through the log-taking
/// variants.
pub fn run(p: &mut Pipeline, source: Source<'_>, partition: Partition) -> Run {
    let mut r = Run::default();
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    match source {
        Source::OnePass(x) => {
            let mut pos = 0usize;
            for n in blocks(partition, x.len()) {
                let mut ev = vec![VasEvent::default(); p.max_events(n)];
                let mut y = vec![0.0f32; n];
                let info = p
                    .process_with_events_and_changes(&x[pos..pos + n], &mut y, &mut ev, &mut log)
                    .expect("a valid call");
                r.out.extend_from_slice(&y[..info.produced]);
                assert!(info.changes <= MAX_CHANGES_PER_CALL, "{}", info.changes);
                r.log.extend_from_slice(&log[..info.changes]);
                r.changes += info.changes;
                r.max_per_call = r.max_per_call.max(info.changes);
                pos += n;
            }
        }
        Source::Playback(number) => {
            p.start_playback(number).expect("start_playback");
            let sizes = match partition {
                Partition::Fixed(n) => vec![n.max(1)],
                Partition::Irregular => irregular(),
                Partition::IrregularWithBoundary(at) => blocks(partition, (at as usize) + 200_000),
            };
            let mut i = 0usize;
            loop {
                let n = sizes[i % sizes.len()];
                i += 1;
                let mut y = vec![0.0f32; n];
                let info = p.play_with_changes(&mut y, &mut log).expect("play");
                r.out.extend_from_slice(&y[..info.produced]);
                assert!(info.changes <= MAX_CHANGES_PER_CALL, "{}", info.changes);
                r.log.extend_from_slice(&log[..info.changes]);
                r.changes += info.changes;
                r.max_per_call = r.max_per_call.max(info.changes);
                if info.finished {
                    break;
                }
                assert!(i < 5_000_000, "the playback did not finish");
            }
        }
    }
    r
}

/// The block-size-1 plain-setter oracle (research R-08): the stream run one host sample per
/// call; before each call every change whose position is at or before the stream's position
/// (and not yet applied) is applied through the plain setter, in submission order. Version
/// 0.15's behavior defines the landing; nothing of spec 015's schedule is used.
pub fn setter_oracle(p: &mut Pipeline, source: Source<'_>, changes: &[Change]) -> Vec<f32> {
    let stream = match source {
        Source::OnePass(_) => Stream::OnePass,
        Source::Playback(_) => Stream::Playback,
    };
    let mut done = vec![false; changes.len()];
    let apply_due = |p: &mut Pipeline, done: &mut [bool]| {
        let pos = p.stream_position(stream).expect("a stream");
        for (i, c) in changes.iter().enumerate() {
            if !done[i] && c.stream == stream && c.position <= pos {
                c.apply_plain(p);
                done[i] = true;
            }
        }
    };
    let mut out = Vec::new();
    match source {
        Source::OnePass(x) => {
            let mut ev = vec![VasEvent::default(); p.max_events(1)];
            for &v in x {
                apply_due(p, &mut done);
                let mut y = [0.0f32; 1];
                let info = p
                    .process_with_events(&[v], &mut y, &mut ev)
                    .expect("a valid call");
                out.extend_from_slice(&y[..info.produced]);
            }
        }
        Source::Playback(number) => {
            p.start_playback(number).expect("start_playback");
            let mut guard = 0usize;
            loop {
                apply_due(p, &mut done);
                let mut y = [0.0f32; 1];
                let info = p.play(&mut y).expect("play");
                out.extend_from_slice(&y[..info.produced]);
                if info.finished {
                    break;
                }
                guard += 1;
                assert!(guard < 50_000_000, "the playback did not finish");
            }
        }
    }
    out
}

/// Bit-identity of two slices.
pub fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(u, v)| u.to_bits() == v.to_bits())
}

/// The first index where two slices differ bit for bit (or in length).
pub fn first_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    let n = a.len().min(b.len());
    (0..n)
        .find(|&i| a[i].to_bits() != b[i].to_bits())
        .or((a.len() != b.len()).then_some(n))
}

/// FNV-1a over the samples' bit patterns (the golden's output digest).
pub fn digest(x: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in x {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The entries of `log` for one control, in order.
pub fn entries_of(log: &[ChangeEntry], control: u32) -> Vec<ChangeEntry> {
    log.iter()
        .filter(|e| e.control == control)
        .copied()
        .collect()
}

/// Asserts the per-stream chaining of a log (FR-005): for each control, every entry's `before`
/// equals the previous entry's `after`, and the first entry's `before` equals `start`.
pub fn assert_chained(log: &[ChangeEntry], control: u32, start: f64) {
    let mut last = start;
    for e in entries_of(log, control) {
        assert_eq!(e.before, last, "chained before at {}: {e:?}", e.position);
        last = e.after;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irregular_holds_the_required_lengths() {
        let v = irregular();
        assert!(v.contains(&1));
        assert!(v.contains(&1009));
        assert!(v.iter().any(|&n| n > 1000));
        assert!(v.iter().all(|&n| (1..=4096).contains(&n)));
        let b = blocks(Partition::IrregularWithBoundary(5000), 20_000);
        let mut pos = 0usize;
        assert!(b.iter().any(|&n| {
            pos += n;
            pos == 5000
        }));
        assert_eq!(b.iter().sum::<usize>(), 20_000);
    }

    #[test]
    fn live_indices_resolve() {
        assert_eq!(route_control(), 0);
        assert_eq!(volume_control(), 1);
        assert_eq!(speed_control(), 2);
    }
}
