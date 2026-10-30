//! Sample-timed control changes and the change log (spec 015).
//!
//! A change of a live control (the route, the volume position, the playback speed level) can
//! be **scheduled at a position** in a stream's own clock, the host output samples the stream
//! has produced, and lands on exactly that sample at any block size (spec 015 D2, D3). Every
//! change a stream applies is reported as a [`ChangeEntry`] (D5). Positioning a change on a
//! sample is a feature of the emulator's API, not a device behavior: the recorder's own
//! controls move whenever a hand moves them (D8; no assumption is registered).

use crate::error::Error;
use crate::settings::SUPPORTED_HOST_RATES;

/// Pending changes a stream's schedule holds (spec 015 D4, FR-002): reserved at construction.
pub const SCHEDULE_CAPACITY: usize = 64; // engineering target (015 D4)

/// A change-log length that is always enough for one processing call (spec 015 FR-005): the
/// schedule's capacity plus one plain change per live control.
pub const MAX_CHANGES_PER_CALL: usize = SCHEDULE_CAPACITY + 3; // 015 FR-005

/// The stream a change is scheduled on (spec 015 D2): each has its own clock and schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u32)]
pub enum Stream {
    /// The player: the clock starts at 0 at [`start_playback`](crate::Pipeline::start_playback)
    /// and counts every sample the playback produces (the recording's part, the speed control's
    /// tail and the drain).
    Playback = 0,
    /// The one-pass processing: the clock starts at 0 at construction,
    /// [`reset`](crate::Pipeline::reset) and [`reconfigure`](crate::Pipeline::reconfigure), the
    /// clock the voice-activation events use.
    OnePass = 1,
    /// Reserved for the record-side values (spec 017); every call refuses it today.
    Record = 2,
}

/// One applied change (spec 015 FR-005): reported by the processing call that produced the
/// sample it landed on. Values are the control's own, widened to a double: a route 0 (speaker)
/// or 1 (earphone), a volume position, a speed level 1 … 5. `before` is the value the stream
/// last applied for the control (chained: each entry's `before` equals the previous entry's
/// `after` for that control on that stream, or the value the stream started with). `tag` echoes
/// the schedule call's tag; **0 means a plain setter** (`set_route`, `set_volume`,
/// `set_playback_speed`), so apps tag their scheduled changes from 1. The layout is the C
/// `RrDr60ChangeEntry`'s (40 bytes, no size field).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[non_exhaustive]
#[repr(C)]
pub struct ChangeEntry {
    /// The host output index the change landed on, in the stream's clock.
    pub position: u64,
    /// The stream's last applied value for the control.
    pub before: f64,
    /// The value applied.
    pub after: f64,
    /// The schedule call's tag; 0 for a plain setter.
    pub tag: u64,
    /// The control manifest's index of the control.
    pub control: u32,
    /// 0; a later use (the pre-1.0 ABI pass).
    pub reserved: u32,
}

impl ChangeEntry {
    /// An entry with every field given and `reserved` 0.
    pub(crate) const fn new(
        position: u64,
        control: u32,
        before: f64,
        after: f64,
        tag: u64,
    ) -> Self {
        Self {
            position,
            before,
            after,
            tag,
            control,
            reserved: 0,
        }
    }
}

/// The device-rate output index at which a speed change scheduled at host output `position`
/// lands (spec 015 D3): d(0) = 0 and d(P) = ⌊(P − 1) · 6000 ÷ rate⌋ + 1 for P ≥ 1 (P at
/// 6 kHz), the first device sample the playback conversion pulls to produce output sample P
/// (research R-02; the inverse of the voice-activation events' position rule, spec 003 R-06).
/// Its audible effect follows after the playback path's latency.
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`] for a rate not in
/// [`SUPPORTED_HOST_RATES`](crate::SUPPORTED_HOST_RATES).
pub fn speed_landing(position: u64, host_rate_hz: u32) -> Result<u64, Error> {
    if !SUPPORTED_HOST_RATES.contains(&host_rate_hz) {
        return Err(Error::UnsupportedHostRate {
            requested: host_rate_hz,
        });
    }
    if position == 0 {
        return Ok(0);
    }
    let p = u128::from(position - 1);
    let d = p * u128::from(crate::settings::DEVICE_RATE_HZ) / u128::from(host_rate_hz);
    Ok(d as u64 + 1)
}

/// One pending change (data-model.md › Schedule).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Entry {
    /// The position in the stream's clock.
    pub(crate) position: u64,
    /// The insertion sequence, for a stable order at one position.
    pub(crate) seq: u64,
    /// The value (the control's own, as a double).
    pub(crate) value: f64,
    /// The caller's tag.
    pub(crate) tag: u64,
    /// The control manifest's index.
    pub(crate) control: u32,
}

const EMPTY: Entry = Entry {
    position: u64::MAX,
    seq: 0,
    value: 0.0,
    tag: 0,
    control: 0,
};

/// A stream's schedule (spec 015 D4; research R-03): a sorted inline array, ties by sequence,
/// the earliest position cached so the processing path reads one `u64`.
#[derive(Clone, Debug)]
pub(crate) struct Schedule {
    entries: [Entry; SCHEDULE_CAPACITY],
    len: usize,
    next_seq: u64,
    earliest: u64,
    /// Test-only (research R-13): the schedule in submission order.
    #[cfg(feature = "__test-hooks")]
    pub(crate) submission_order: bool,
}

impl Schedule {
    /// An empty schedule.
    pub(crate) const fn new() -> Self {
        Self {
            entries: [EMPTY; SCHEDULE_CAPACITY],
            len: 0,
            next_seq: 0,
            earliest: u64::MAX,
            #[cfg(feature = "__test-hooks")]
            submission_order: false,
        }
    }

    /// Inserts in position order (ties after the earlier submission); `Err(())` when full,
    /// nothing stored. Bounded: a memmove of at most `SCHEDULE_CAPACITY − 1` entries.
    pub(crate) fn insert(
        &mut self,
        position: u64,
        control: u32,
        value: f64,
        tag: u64,
    ) -> Result<(), ()> {
        if self.len >= SCHEDULE_CAPACITY {
            return Err(());
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        // The first index whose position is greater: ties (same position) go after.
        let mut at = self.len;
        #[cfg(feature = "__test-hooks")]
        let sorted = !self.submission_order;
        #[cfg(not(feature = "__test-hooks"))]
        let sorted = true;
        if sorted {
            for (i, e) in self.entries[..self.len].iter().enumerate() {
                if e.position > position {
                    at = i;
                    break;
                }
            }
        }
        self.entries.copy_within(at..self.len, at + 1);
        self.entries[at] = Entry {
            position,
            seq,
            value,
            tag,
            control,
        };
        self.len += 1;
        self.earliest = self.entries[0].position;
        Ok(())
    }

    /// Pops the earliest entry if its position is at or before `produced`.
    #[inline]
    pub(crate) fn pop_due(&mut self, produced: u64) -> Option<Entry> {
        // The cold path (research R-09: the hot compare is the path's `next_apply`), so the
        // empty check costs nothing that matters; `earliest` is `u64::MAX` when empty.
        if self.len == 0 || self.earliest > produced {
            return None;
        }
        let e = self.entries[0];
        self.entries.copy_within(1..self.len, 0);
        self.len -= 1;
        self.entries[self.len] = EMPTY;
        self.earliest = if self.len == 0 {
            u64::MAX
        } else {
            self.entries[0].position
        };
        Some(e)
    }

    /// Drops every pending change.
    pub(crate) fn clear(&mut self) {
        self.entries = [EMPTY; SCHEDULE_CAPACITY];
        self.len = 0;
        self.earliest = u64::MAX;
    }

    /// Pending changes.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The earliest pending position, or `u64::MAX` when empty (a cached field).
    #[inline]
    pub(crate) fn earliest(&self) -> u64 {
        self.earliest
    }
}

/// Deliberate defects for the spec 015 mutation tests (research R-13); `Default` is the
/// committed behavior.
#[cfg(feature = "__test-hooks")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChangeMutation {
    /// The chain controls applied at the call's entry (version 0.15's form), not at sample P.
    pub apply_at_entry: bool,
    /// A scheduled change lands one sample late.
    pub off_by_one_position: bool,
    /// The schedule in submission order, not position order.
    pub submission_order: bool,
    /// The entry's `before` read from the engine's reading, not the stream's last applied value.
    pub log_reading_before: bool,
    /// A route switch runs the chain's full reset (the meter accumulators lost).
    pub full_reset_on_route: bool,
    /// The published d(P) written as ⌊P · 6000 ÷ rate⌋ (one device sample late above 6 kHz).
    pub speed_landing_ceiling: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn schedule_orders_by_position_then_sequence() {
        let mut s = Schedule::new();
        s.insert(1000, 1, 0.5, 7).unwrap();
        s.insert(500, 0, 1.0, 8).unwrap();
        s.insert(500, 1, 0.25, 9).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.earliest(), 500);
        assert!(s.pop_due(499).is_none());
        let a = s.pop_due(500).unwrap();
        assert_eq!((a.position, a.seq, a.tag), (500, 1, 8));
        assert_eq!(s.earliest(), 500);
        let b = s.pop_due(600).unwrap();
        assert_eq!((b.position, b.seq, b.tag), (500, 2, 9));
        assert_eq!(s.earliest(), 1000);
        assert!(s.pop_due(999).is_none());
        let c = s.pop_due(1000).unwrap();
        assert_eq!((c.position, c.seq, c.tag, c.control), (1000, 0, 7, 1));
        assert_eq!(s.earliest(), u64::MAX);
        assert_eq!(s.len(), 0);
        assert!(s.pop_due(u64::MAX).is_none());
    }

    #[test]
    fn schedule_refuses_the_65th_and_clears() {
        let mut s = Schedule::new();
        for i in 0..SCHEDULE_CAPACITY as u64 {
            s.insert(10_000 - i, 0, 0.0, i).unwrap();
        }
        assert_eq!(s.len(), SCHEDULE_CAPACITY);
        assert!(s.insert(1, 0, 0.0, 99).is_err());
        assert_eq!(s.len(), SCHEDULE_CAPACITY);
        assert_eq!(s.earliest(), 10_000 - 63);
        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.earliest(), u64::MAX);
        assert!(s.pop_due(u64::MAX).is_none());
    }

    #[test]
    fn speed_landing_at_the_seven_rates() {
        for rate in SUPPORTED_HOST_RATES {
            assert_eq!(speed_landing(0, rate).unwrap(), 0, "{rate}");
        }
        for p in 0..=100 {
            assert_eq!(speed_landing(p, 6000).unwrap(), p);
        }
        for (p, d) in [(1, 1), (8, 1), (9, 2), (16, 2), (17, 3)] {
            assert_eq!(speed_landing(p, 48_000).unwrap(), d, "48 kHz P={p}");
        }
        for p in 1..=40u64 {
            assert_eq!(
                speed_landing(p, 8000).unwrap(),
                3 * (p - 1) / 4 + 1,
                "8 kHz P={p}"
            );
        }
        // The inverse of the events' rule: d(P) is the smallest j with ⌈j·m/l⌉ ≥ P.
        for rate in SUPPORTED_HOST_RATES {
            let (l, m) = (
                u128::from(crate::settings::DEVICE_RATE_HZ),
                u128::from(rate),
            );
            for p in 1..=300u128 {
                let d = u128::from(speed_landing(p as u64, rate).unwrap());
                assert!((d * m).div_ceil(l) >= p, "{rate} P={p} d={d}");
                assert!(
                    d == 0 || ((d - 1) * m).div_ceil(l) < p,
                    "{rate} P={p} d={d}"
                );
            }
        }
        assert!(matches!(
            speed_landing(5, 0),
            Err(Error::UnsupportedHostRate { requested: 0 })
        ));
        assert!(matches!(
            speed_landing(5, 22_050),
            Err(Error::UnsupportedHostRate { requested: 22_050 })
        ));
    }

    #[test]
    fn entry_layout() {
        assert_eq!(size_of::<ChangeEntry>(), 40);
        assert_eq!(align_of::<ChangeEntry>(), 8);
        assert_eq!(offset_of!(ChangeEntry, position), 0);
        assert_eq!(offset_of!(ChangeEntry, before), 8);
        assert_eq!(offset_of!(ChangeEntry, after), 16);
        assert_eq!(offset_of!(ChangeEntry, tag), 24);
        assert_eq!(offset_of!(ChangeEntry, control), 32);
        assert_eq!(offset_of!(ChangeEntry, reserved), 36);
        assert_eq!(
            ChangeEntry::new(1, 2, 3.0, 4.0, 5),
            ChangeEntry {
                position: 1,
                before: 3.0,
                after: 4.0,
                tag: 5,
                control: 2,
                reserved: 0,
            }
        );
        assert_eq!(MAX_CHANGES_PER_CALL, 67);
    }

    #[test]
    fn stream_discriminants() {
        assert_eq!(Stream::Playback as u32, 0);
        assert_eq!(Stream::OnePass as u32, 1);
        assert_eq!(Stream::Record as u32, 2);
    }
}

/// The control manifest's index of `route` (pinned by a test against the manifest).
pub(crate) const LIVE_ROUTE: u32 = 0;
/// The control manifest's index of `volume`.
pub(crate) const LIVE_VOLUME: u32 = 1;
/// The control manifest's index of `playback_speed`.
pub(crate) const LIVE_SPEED: u32 = 2;

/// A stream's control state (data-model.md › Stream; research R-01): the clock, the schedule,
/// the last applied values, the pending plain change, the landed readings and the staging log.
/// Lives in each `PlaybackPath`.
#[derive(Clone, Debug)]
pub(crate) struct StreamState {
    /// The clock: host output samples produced; the next sample has this index (D2).
    pub(crate) produced: u64,
    pub(crate) schedule: Schedule,
    /// 0 while a plain change is pending, else the schedule's earliest position (R-09).
    pub(crate) next_apply: u64,
    last_route: crate::Route,
    last_volume: f32,
    last_level: u8,
    pending_route: crate::Route,
    pending_volume: f32,
    pending_level: Option<u8>,
    plain_pending: bool,
    /// The last landed value per control in the call, for the readings (R-07).
    pub(crate) landed_route: Option<crate::Route>,
    pub(crate) landed_volume: Option<f32>,
    pub(crate) landed_level: Option<u8>,
    log: [ChangeEntry; MAX_CHANGES_PER_CALL],
    log_len: usize,
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: ChangeMutation,
}

/// What a scheduled or plain change asks the path to do (the chain pair, or the level).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Apply {
    /// `chain.set_controls(route, volume)`.
    Chain(crate::Route, f32),
    /// `speed.request(level)`.
    Level(u8),
}

impl StreamState {
    pub(crate) fn new(route: crate::Route, volume: f32, level: u8) -> Self {
        Self {
            produced: 0,
            schedule: Schedule::new(),
            next_apply: u64::MAX,
            last_route: route,
            last_volume: volume,
            last_level: level,
            pending_route: route,
            pending_volume: volume,
            pending_level: None,
            plain_pending: false,
            landed_route: None,
            landed_volume: None,
            landed_level: None,
            log: [ChangeEntry::default(); MAX_CHANGES_PER_CALL],
            log_len: 0,
            #[cfg(feature = "__test-hooks")]
            mutation: ChangeMutation::default(),
        }
    }

    /// The stream's reset (`PlaybackPath::reset`): the clock, the pending change and the log;
    /// never the schedule (R-01).
    pub(crate) fn reset(&mut self) {
        self.produced = 0;
        self.plain_pending = false;
        self.pending_level = None;
        self.next_apply = self.arm(self.schedule.earliest());
        self.log_len = 0;
        self.landed_route = None;
        self.landed_volume = None;
        self.landed_level = None;
    }

    /// The lifecycle rule (R-07): the stream starts with the engine's values, no entry.
    pub(crate) fn set_start_values(&mut self, route: crate::Route, volume: f32, level: u8) {
        self.last_route = route;
        self.last_volume = volume;
        self.last_level = level;
    }

    /// A processing call's entry: the plain change pending (today's entry-time controls), the
    /// log and the landed readings cleared, the apply armed for the first produced sample.
    #[inline]
    pub(crate) fn begin_call(&mut self, route: crate::Route, volume: f32, level: Option<u8>) {
        self.pending_route = route;
        self.pending_volume = volume;
        self.pending_level = level;
        self.plain_pending = true;
        self.next_apply = 0;
        self.log_len = 0;
        self.landed_route = None;
        self.landed_volume = None;
        self.landed_level = None;
    }

    /// Whether the apply point is due for the sample about to be produced: the hot compare,
    /// one `u64` comparison (research R-09; the off-by-one mutant is folded into
    /// `next_apply` when it is cached, never into this compare).
    #[inline]
    pub(crate) fn due(&self) -> bool {
        self.produced >= self.next_apply
    }

    /// The cached earliest position (the off-by-one mutant lands one sample late).
    #[inline]
    fn arm(&self, earliest: u64) -> u64 {
        #[cfg(feature = "__test-hooks")]
        if self.mutation.off_by_one_position {
            return earliest.saturating_add(1);
        }
        earliest
    }

    /// The due changes, in order, for the sample about to be produced (the cold path, R-02):
    /// the plain change first, then every scheduled entry at or before the position; each is
    /// handed to `apply` (the chain pair or the level) and logged when the stream's own value
    /// moved (a plain change) or always (a scheduled one, D5).
    #[inline(never)]
    pub(crate) fn apply_due(&mut self, apply: &mut dyn FnMut(Apply)) {
        let pos = self.produced;
        if self.plain_pending {
            self.plain_pending = false;
            let (r, v) = (self.pending_route, self.pending_volume);
            apply(Apply::Chain(r, v));
            if r != self.last_route {
                let before = self.before(route_value(self.last_route), route_value(r));
                self.push(pos, LIVE_ROUTE, before, route_value(r), 0);
                self.last_route = r;
                self.landed_route = Some(r);
            }
            if v != self.last_volume {
                let before = self.before(f64::from(self.last_volume), f64::from(v));
                self.push(pos, LIVE_VOLUME, before, f64::from(v), 0);
                self.last_volume = v;
                self.landed_volume = Some(v);
            }
            if let Some(l) = self.pending_level.take() {
                apply(Apply::Level(l));
                if l != self.last_level {
                    let before = self.before(f64::from(self.last_level), f64::from(l));
                    self.push(pos, LIVE_SPEED, before, f64::from(l), 0);
                    self.last_level = l;
                    self.landed_level = Some(l);
                }
            }
        }
        while let Some(e) = self.schedule.pop_due(pos) {
            match e.control {
                LIVE_ROUTE => {
                    let r = if e.value == 0.0 {
                        crate::Route::Speaker
                    } else {
                        crate::Route::Earphone
                    };
                    apply(Apply::Chain(r, self.last_volume));
                    let before = self.before(route_value(self.last_route), route_value(r));
                    self.push(pos, LIVE_ROUTE, before, route_value(r), e.tag);
                    self.last_route = r;
                    self.landed_route = Some(r);
                }
                LIVE_VOLUME => {
                    let v = e.value as f32;
                    apply(Apply::Chain(self.last_route, v));
                    let before = self.before(f64::from(self.last_volume), e.value);
                    self.push(pos, LIVE_VOLUME, before, f64::from(v), e.tag);
                    self.last_volume = v;
                    self.landed_volume = Some(v);
                }
                _ => {
                    let l = e.value as u8;
                    apply(Apply::Level(l));
                    let before = self.before(f64::from(self.last_level), e.value);
                    self.push(pos, LIVE_SPEED, before, f64::from(l), e.tag);
                    self.last_level = l;
                    self.landed_level = Some(l);
                }
            }
        }
        self.next_apply = self.arm(self.schedule.earliest());
    }

    /// The entry's `before` (the stream's last applied value; a mutant reads the engine's value).
    #[inline]
    fn before(&self, last: f64, after: f64) -> f64 {
        #[cfg(feature = "__test-hooks")]
        if self.mutation.log_reading_before {
            return after;
        }
        let _ = after;
        last
    }

    #[inline]
    fn push(&mut self, position: u64, control: u32, before: f64, after: f64, tag: u64) {
        if let Some(slot) = self.log.get_mut(self.log_len) {
            *slot = ChangeEntry::new(position, control, before, after, tag);
        }
        self.log_len += 1;
    }

    /// Copies the call's entries into `into` (the first ones when it is shorter) and returns
    /// how many the call applied (FR-005).
    pub(crate) fn drain_log(&self, into: &mut [ChangeEntry]) -> usize {
        let n = self.log_len.min(into.len()).min(MAX_CHANGES_PER_CALL);
        into[..n].copy_from_slice(&self.log[..n]);
        self.log_len
    }

    /// Re-arms the apply point after a schedule change (the schedule call, a clear): the
    /// earliest pending position, unless a plain change keeps it at 0.
    pub(crate) fn rearm(&mut self) {
        if self.next_apply != 0 {
            self.next_apply = self.arm(self.schedule.earliest());
        }
    }

    /// The call's applied-change count (every variant reports it).
    pub(crate) fn log_len(&self) -> usize {
        self.log_len
    }

    /// Test-only (research R-13): an entry-time apply, as if every call were a 4096-sample
    /// block: every entry within the next 4096 positions lands at the call's first sample.
    /// Caught by the partition identity (64-sample blocks land it early).
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn mutant_pop_all_now(&mut self) {
        if self.mutation.apply_at_entry {
            let now = self.produced;
            for i in 0..self.schedule.len {
                if self.schedule.entries[i].position <= now + 4096 {
                    self.schedule.entries[i].position = now;
                }
            }
            if self.schedule.len > 0 {
                self.schedule.earliest = self.schedule.entries[0].position;
            }
        }
    }
}

/// The route's value in an entry: 0 speaker, 1 earphone.
#[inline]
pub(crate) fn route_value(route: crate::Route) -> f64 {
    match route {
        crate::Route::Speaker => 0.0,
        crate::Route::Earphone => 1.0,
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;

    /// The live controls' manifest indices (research R-04; the C smoke resolves them by
    /// identifier, the engine pins them here).
    #[test]
    fn live_control_indices_match_the_manifest() {
        let c = crate::controls::controls();
        assert_eq!(c[LIVE_ROUTE as usize].identifier, "route");
        assert_eq!(c[LIVE_VOLUME as usize].identifier, "volume");
        assert_eq!(c[LIVE_SPEED as usize].identifier, "playback_speed");
    }
}
