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

/// A change-log length that is always enough for one processing call (spec 015 FR-005; spec
/// 017 FR-005, restated at the plan): the one-pass's two schedules (one per clock, spec 017 D2),
/// one plain change per live control, and one cross-stream landing per record-side value (spec
/// 017 D4), and one mirrored playback-stage switch per stage 11 – 13 (spec 021 research R-05).
/// The one-pass attains 142 in one call, the recorder 72, the player 68. 67 in 0.16, 139 in 0.17.
pub const MAX_CHANGES_PER_CALL: usize = 2 * SCHEDULE_CAPACITY + 3 + 8 + 3; // 015 FR-005; 017 FR-005; 021 FR-001

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
    /// The recorder (spec 017 D2): the clock is the **input** clock, host input samples fed
    /// since [`start_recording`](crate::Pipeline::start_recording) (the stop's silence
    /// included), and it carries the eight record-side values only.
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
    /// The index the change landed on, in the stream's clock: a host **output** sample for the
    /// route, the volume and the playback speed; a host **input** sample for a record-side value
    /// (spec 017 D2; the control's side is [`clock_of`]). The one-pass has both clocks.
    pub position: u64,
    /// The stream's last applied value for the control.
    pub before: f64,
    /// The value applied.
    pub after: f64,
    /// The schedule call's tag; 0 for a plain setter.
    pub tag: u64,
    /// The control manifest's index of the control.
    pub control: u32,
    /// The stream that applied the change (spec 020 D9): [`STREAM_PLAYBACK`] (1),
    /// [`STREAM_ONE_PASS`] (2) or [`STREAM_RECORD`] (3); 0 for an entry from before 0.20 or a
    /// default entry. The one-pass's two clocks are told apart by the control ([`clock_of`]).
    /// (Until 0.19 this field was `reserved`, always 0; the C field keeps that name.)
    pub stream: u32,
}

/// The stream word of the playback stream (spec 020 D9).
pub const STREAM_PLAYBACK: u32 = 1; // FR-007 (020 D9): the stream word
/// The stream word of the one-pass stream.
pub const STREAM_ONE_PASS: u32 = 2; // FR-007 (020 D9)
/// The stream word of the record stream (the recorder).
pub const STREAM_RECORD: u32 = 3; // FR-007 (020 D9)

impl ChangeEntry {
    /// An entry with every field given (spec 020: a recipient assembles the timeline map's and
    /// the pure writer's inputs; the engine sets the stream word itself).
    #[must_use]
    pub const fn new(
        position: u64,
        control: u32,
        before: f64,
        after: f64,
        tag: u64,
        stream: u32,
    ) -> Self {
        Self {
            position,
            before,
            after,
            tag,
            control,
            stream,
        }
    }
}

impl Stream {
    /// The stream word entries of this stream carry (spec 020 D9).
    #[must_use]
    pub const fn word(self) -> u32 {
        match self {
            Stream::Playback => STREAM_PLAYBACK,
            Stream::OnePass => STREAM_ONE_PASS,
            Stream::Record => STREAM_RECORD,
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

    /// The pending entries, in position order (spec 017: the event bound reads the pending hang
    /// changes).
    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries[..self.len]
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
        assert_eq!(offset_of!(ChangeEntry, stream), 36);
        assert_eq!(
            ChangeEntry::new(1, 2, 3.0, 4.0, 5, 0),
            ChangeEntry {
                position: 1,
                before: 3.0,
                after: 4.0,
                tag: 5,
                control: 2,
                stream: 0,
            }
        );
        assert_eq!(MAX_CHANGES_PER_CALL, 142); // 021: 2 · 64 + 3 + 8 + 3
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
/// The control manifest's index of `battery_volts` (spec 018: appended after the 0.14 entries,
/// so no earlier index moves; pinned by a test against the manifest).
pub(crate) const LIVE_BATTERY: u32 = 29;
/// Spec 023 D1: the playback speed mode, appended (index 30).
pub(crate) const LIVE_SPEED_MODE: u32 = 30;
/// The control manifest's indices of the four playback-side stage switches (spec 021 FR-001;
/// pinned by a test against the manifest): stage 10 `playback_stage_enabled`, stages 11 – 13
/// `output.volume_amp_enabled`, `output.speaker_enabled`, `output.earphone_enabled`.
pub(crate) const LIVE_STAGE_10: u32 = 5;
pub(crate) const LIVE_STAGE_11: u32 = 22;
pub(crate) const LIVE_STAGE_12: u32 = 23;
pub(crate) const LIVE_STAGE_13: u32 = 24;

/// The stage (10 … 13) of a playback-stage switch control, or `None` (spec 021 FR-001).
pub(crate) fn stage_of(control: u32) -> Option<u8> {
    match control {
        LIVE_STAGE_10 => Some(10),
        LIVE_STAGE_11 => Some(11),
        LIVE_STAGE_12 => Some(12),
        LIVE_STAGE_13 => Some(13),
        _ => None,
    }
}

/// The settings' switch values of stages 11 – 13 as bypass flags (spec 021 research R-05).
pub(crate) fn bypassed_of(settings: &crate::Settings) -> [bool; 3] {
    [
        !settings.output.volume_amp_enabled,
        !settings.output.speaker_enabled,
        !settings.output.earphone_enabled,
    ]
}

/// The switch value a bypass state logs (the setting's sense: 1 on, 0 bypassed).
#[inline]
fn switch_value(bypassed: bool) -> f64 {
    if bypassed { 0.0 } else { 1.0 }
}

/// A stream's control state (data-model.md › Stream; research R-01): the clock, the schedule,
/// the last applied values, the pending plain change, the landed readings and the staging log.
/// Lives in each `PlaybackPath`. `repr(C)` so that the hot pair `produced` / `next_apply` (the
/// per-sample compare) and the small fields share the first cache line and the two arrays
/// (5 KB) sit last (post-implementation hardware review B1).
#[derive(Clone, Debug)]
#[repr(C)]
pub(crate) struct StreamState {
    /// The clock: host output samples produced; the next sample has this index (D2).
    pub(crate) produced: u64,
    /// 0 while a plain change is pending, else the schedule's earliest position (R-09).
    pub(crate) next_apply: u64,
    log_len: usize,
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
    /// Spec 023: the speed mode (0 pitch with speed, 1 pitch kept), as the level: last applied,
    /// pending plain, landed.
    last_mode: u8,
    pending_mode: Option<u8>,
    pub(crate) landed_mode: Option<u8>,
    /// Spec 018: the battery value (volts), as the volume: last applied, pending plain, landed.
    last_battery: f32,
    pending_battery: f32,
    pub(crate) landed_battery: Option<f32>,
    /// The stream word the entries carry (spec 020 D9); set after construction.
    stream: u32,
    /// Spec 021 (research R-05): the last applied switch per stage 11 – 13 (true = bypassed),
    /// the call's landings with their tags (read by the pipeline into the settings), the
    /// cross-stream mirror pending for this call (the one-pass only), and stage 10's (the
    /// player only, no mirror).
    last_bypass: [bool; 3],
    pub(crate) landed_bypass: [Option<(bool, u64)>; 3],
    mirror: [Option<(bool, u64)>; 3],
    last_stage10: bool,
    pub(crate) landed_stage10: Option<(bool, u64)>,
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: ChangeMutation,
    pub(crate) schedule: Schedule,
    log: [ChangeEntry; MAX_CHANGES_PER_CALL],
}

/// What a scheduled or plain change asks the path to do (the chain pair, or the level).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Apply {
    /// `chain.set_controls(route, volume)`.
    Chain(crate::Route, f32),
    /// `speed.request(level)`.
    Level(u8),
    /// `speed.request_mode(mode)` (spec 023).
    SpeedMode(u8),
    /// `chain.set_battery(volts)` (spec 018).
    Battery(f32),
    /// Spec 021: a playback-stage switch (10 … 13) landing: `bypassed` the value 0, `at_once`
    /// on the stream's first produced sample (spec 011's rule).
    Bypass {
        stage: u8,
        bypassed: bool,
        at_once: bool,
    },
}

impl StreamState {
    pub(crate) fn new(route: crate::Route, volume: f32, level: u8, battery: f32) -> Self {
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
            last_mode: 0,
            pending_mode: None,
            landed_mode: None,
            last_battery: battery,
            pending_battery: battery,
            landed_battery: None,
            stream: 0,
            last_bypass: [false; 3],
            landed_bypass: [None; 3],
            mirror: [None; 3],
            last_stage10: false,
            landed_stage10: None,
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
        self.pending_mode = None;
        self.next_apply = self.arm(self.schedule.earliest());
        self.log_len = 0;
        self.landed_route = None;
        self.landed_volume = None;
        self.landed_level = None;
        self.landed_mode = None;
        self.landed_battery = None;
        self.landed_bypass = [None; 3];
        self.landed_stage10 = None;
        self.mirror = [None; 3];
    }

    /// Spec 021 D2 (research R-05): the stream's switch values become `bypassed` (stages 11 –
    /// 13) and `stage10`, no entry (`start_playback`'s adopt; the engine's `reset`).
    pub(crate) fn set_start_bypass(&mut self, bypassed: [bool; 3], stage10: bool) {
        self.last_bypass = bypassed;
        self.last_stage10 = stage10;
    }

    /// Spec 021 research R-05 (the one-pass's mirror, spec 017's compare-at-call form): for each
    /// stage 11 – 13 the stream reaches whose settings value differs from its last applied one,
    /// a landing at the call's first sample with the stored tag.
    pub(crate) fn begin_mirror(
        &mut self,
        settings_bypassed: [bool; 3],
        tags: [u64; 3],
        reach: [bool; 3],
    ) {
        for i in 0..3 {
            self.mirror[i] = (reach[i] && settings_bypassed[i] != self.last_bypass[i])
                .then_some((settings_bypassed[i], tags[i]));
        }
    }

    /// The lifecycle rule (R-07): the stream starts with the engine's values, no entry.
    pub(crate) fn set_start_values(
        &mut self,
        route: crate::Route,
        volume: f32,
        level: u8,
        battery: f32,
    ) {
        self.last_route = route;
        self.last_volume = volume;
        self.last_level = level;
        self.last_battery = battery;
    }

    /// Spec 023 D1: the stream's speed mode becomes `mode`, no entry (`start_playback`'s
    /// adopt; the engine's `reset`).
    pub(crate) fn set_start_mode(&mut self, mode: u8) {
        self.last_mode = mode;
    }

    /// Spec 023: the plain mode pending for this call (the player only; `None` elsewhere).
    #[inline]
    pub(crate) fn begin_call_mode(&mut self, mode: Option<u8>) {
        self.pending_mode = mode;
        self.landed_mode = None;
    }

    /// A processing call's entry: the plain change pending (today's entry-time controls), the
    /// log and the landed readings cleared, the apply armed for the first produced sample.
    #[inline]
    pub(crate) fn begin_call(
        &mut self,
        route: crate::Route,
        volume: f32,
        level: Option<u8>,
        battery: f32,
    ) {
        self.pending_route = route;
        self.pending_volume = volume;
        self.pending_level = level;
        self.pending_battery = battery;
        self.plain_pending = true;
        self.next_apply = 0;
        self.log_len = 0;
        self.landed_route = None;
        self.landed_volume = None;
        self.landed_level = None;
        self.landed_battery = None;
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
        // Spec 021 research R-05: the cross-stream mirror first (the one-pass), in stage order.
        for i in 0..3 {
            if let Some((b, tag)) = self.mirror[i].take() {
                let stage = 11 + i as u8;
                apply(Apply::Bypass {
                    stage,
                    bypassed: b,
                    at_once: pos == 0,
                });
                let before = self.before(switch_value(self.last_bypass[i]), switch_value(b));
                self.push(pos, LIVE_STAGE_11 + i as u32, before, switch_value(b), tag);
                self.last_bypass[i] = b;
                self.landed_bypass[i] = Some((b, tag));
            }
        }
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
            // Spec 018: the battery value, as the volume (the chain's setter is idempotent).
            let b = self.pending_battery;
            apply(Apply::Battery(b));
            if b != self.last_battery {
                let before = self.before(f64::from(self.last_battery), f64::from(b));
                self.push(pos, LIVE_BATTERY, before, f64::from(b), 0);
                self.last_battery = b;
                self.landed_battery = Some(b);
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
            if let Some(m) = self.pending_mode.take() {
                apply(Apply::SpeedMode(m));
                if m != self.last_mode {
                    let before = self.before(f64::from(self.last_mode), f64::from(m));
                    self.push(pos, LIVE_SPEED_MODE, before, f64::from(m), 0);
                    self.last_mode = m;
                    self.landed_mode = Some(m);
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
                LIVE_SPEED => {
                    let l = e.value as u8;
                    apply(Apply::Level(l));
                    let before = self.before(f64::from(self.last_level), e.value);
                    self.push(pos, LIVE_SPEED, before, f64::from(l), e.tag);
                    self.last_level = l;
                    self.landed_level = Some(l);
                }
                LIVE_BATTERY => {
                    let b = e.value as f32;
                    apply(Apply::Battery(b));
                    let before = self.before(f64::from(self.last_battery), e.value);
                    self.push(pos, LIVE_BATTERY, before, f64::from(b), e.tag);
                    self.last_battery = b;
                    self.landed_battery = Some(b);
                }
                LIVE_SPEED_MODE => {
                    // Spec 023 D1: the mode (exactly 0 or 1, validated at the schedule call).
                    let m = e.value as u8;
                    apply(Apply::SpeedMode(m));
                    let before = self.before(f64::from(self.last_mode), e.value);
                    self.push(pos, LIVE_SPEED_MODE, before, f64::from(m), e.tag);
                    self.last_mode = m;
                    self.landed_mode = Some(m);
                }
                // Spec 021 FR-001: the four playback-stage switches (the value 1 on, 0 bypassed);
                // a landing of the value in force is a no-op entry (before == after).
                LIVE_STAGE_10 => {
                    let b = e.value == 0.0;
                    apply(Apply::Bypass {
                        stage: 10,
                        bypassed: b,
                        at_once: pos == 0,
                    });
                    let before = self.before(switch_value(self.last_stage10), e.value);
                    self.push(pos, LIVE_STAGE_10, before, switch_value(b), e.tag);
                    self.last_stage10 = b;
                    self.landed_stage10 = Some((b, e.tag));
                }
                LIVE_STAGE_11 | LIVE_STAGE_12 | LIVE_STAGE_13 => {
                    let i = (e.control - LIVE_STAGE_11) as usize;
                    let b = e.value == 0.0;
                    apply(Apply::Bypass {
                        stage: 11 + i as u8,
                        bypassed: b,
                        at_once: pos == 0,
                    });
                    let before = self.before(switch_value(self.last_bypass[i]), e.value);
                    self.push(pos, e.control, before, switch_value(b), e.tag);
                    self.last_bypass[i] = b;
                    self.landed_bypass[i] = Some((b, e.tag));
                }
                // Validation at the schedule call admits only the live controls.
                _ => debug_assert!(false, "an unknown control in the schedule"),
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
            *slot = ChangeEntry::new(position, control, before, after, tag, self.stream);
        }
        self.log_len += 1;
    }

    /// Sets the stream word the entries carry (spec 020 D9; construction time).
    pub(crate) fn set_stream(&mut self, stream: Stream) {
        self.stream = stream.word();
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
        assert_eq!(c[LIVE_BATTERY as usize].identifier, "battery_volts"); // 018: appended
        assert_eq!(
            c[LIVE_SPEED_MODE as usize].identifier,
            "playback_speed_mode"
        ); // 023: appended
        // Spec 021 FR-001: the four playback-stage switches.
        assert_eq!(
            c[LIVE_STAGE_10 as usize].identifier,
            "playback_stage_enabled"
        );
        assert_eq!(
            c[LIVE_STAGE_11 as usize].identifier,
            "output.volume_amp_enabled"
        );
        assert_eq!(
            c[LIVE_STAGE_12 as usize].identifier,
            "output.speaker_enabled"
        );
        assert_eq!(
            c[LIVE_STAGE_13 as usize].identifier,
            "output.earphone_enabled"
        );
        assert_eq!(stage_of(LIVE_STAGE_10), Some(10));
        assert_eq!(stage_of(LIVE_STAGE_13), Some(13));
        assert_eq!(stage_of(LIVE_BATTERY), None);
    }
}

#[cfg(test)]
mod bypass_stream_tests {
    //! Spec 021 T006: the stream state's switch landings (research R-02, R-05).
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn state() -> StreamState {
        let mut st = StreamState::new(crate::Route::Speaker, 0.5, 3, 3.0);
        st.set_stream(Stream::Playback);
        st
    }

    fn run_to(st: &mut StreamState, pos: u64) -> Vec<Apply> {
        let mut got = Vec::new();
        st.produced = pos;
        if st.due() {
            st.apply_due(&mut |a| got.push(a));
        }
        got
    }

    #[test]
    fn max_changes_per_call_is_142() {
        assert_eq!(MAX_CHANGES_PER_CALL, 2 * 64 + 3 + 8 + 3);
    }

    /// Spec 023 (research R-07, coach 4): the bound stays 142 (the one-pass attains it and the
    /// mode is refused there); the player's attained count is 64 + 4 plain (route, volume,
    /// level, battery) + the mode = 69, under the bound.
    #[test]
    fn max_changes_per_call_stays_142_player_attains_69() {
        assert_eq!(MAX_CHANGES_PER_CALL, 142);
        let mut st = state();
        st.begin_call(crate::Route::Earphone, 0.25, Some(5), 2.5);
        st.begin_call_mode(Some(1));
        for i in 0..64u64 {
            st.schedule.insert(0, LIVE_VOLUME, 0.1, i + 1).unwrap();
        }
        st.rearm();
        let got = run_to(&mut st, 0);
        assert_eq!(
            got.len(),
            64 + 4,
            "the chain pair is one apply, two entries"
        );
        let mut log = [ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        assert_eq!(st.drain_log(&mut log), 69);
        assert!(69 < MAX_CHANGES_PER_CALL);
    }

    /// Spec 023 D1: a scheduled mode landing is applied as `Apply::SpeedMode`, logged with the
    /// stream word and tag and chained (before = the mode in force), and read as landed; a
    /// plain change (the setter) lands at the call's first sample with tag 0.
    #[test]
    fn mode_landing_logged() {
        let mut st = state();
        st.schedule.insert(5, LIVE_SPEED_MODE, 1.0, 9).unwrap();
        st.rearm();
        assert_eq!(run_to(&mut st, 5), [Apply::SpeedMode(1)]);
        let mut log = [ChangeEntry::default(); 4];
        assert_eq!(st.drain_log(&mut log), 1);
        assert_eq!(
            (
                log[0].position,
                log[0].control,
                log[0].before,
                log[0].after,
                log[0].tag,
                log[0].stream
            ),
            (5, LIVE_SPEED_MODE, 0.0, 1.0, 9, Stream::Playback.word())
        );
        assert_eq!(st.landed_mode, Some(1));
        // The plain setter: pending for the call, applied at its first sample, tag 0, chained.
        let mut st = state();
        st.set_start_mode(1);
        st.begin_call(crate::Route::Speaker, 0.5, Some(3), 3.0);
        st.begin_call_mode(Some(0));
        let got = run_to(&mut st, 0);
        assert!(got.contains(&Apply::SpeedMode(0)));
        let mut log = [ChangeEntry::default(); 8];
        let n = st.drain_log(&mut log);
        let e = log[..n]
            .iter()
            .find(|e| e.control == LIVE_SPEED_MODE)
            .unwrap();
        assert_eq!((e.before, e.after, e.tag), (1.0, 0.0, 0));
        assert_eq!(st.landed_mode, Some(0));
    }

    /// Spec 023 (spec 015's rules): a plain mode equal to the value in force is applied (the
    /// stage's request is idempotent) but not logged and not landed; a scheduled landing of
    /// the value in force is a no-op entry (before == after); the reset clears the pending mode.
    #[test]
    fn mode_same_value_rules() {
        let mut st = state();
        st.begin_call(crate::Route::Speaker, 0.5, Some(3), 3.0);
        st.begin_call_mode(Some(0));
        let got = run_to(&mut st, 0);
        assert!(got.contains(&Apply::SpeedMode(0)));
        let mut log = [ChangeEntry::default(); 8];
        let n = st.drain_log(&mut log);
        assert!(log[..n].iter().all(|e| e.control != LIVE_SPEED_MODE));
        assert_eq!(st.landed_mode, None);
        let mut st = state();
        st.schedule.insert(3, LIVE_SPEED_MODE, 0.0, 2).unwrap();
        st.rearm();
        run_to(&mut st, 3);
        let n = st.drain_log(&mut log);
        assert_eq!(n, 1);
        assert_eq!((log[0].before, log[0].after, log[0].tag), (0.0, 0.0, 2));
        let mut st = state();
        st.begin_call(crate::Route::Speaker, 0.5, Some(3), 3.0);
        st.begin_call_mode(Some(1));
        st.reset();
        assert_eq!(st.pending_mode, None);
        assert!(run_to(&mut st, 0).is_empty());
    }

    #[test]
    fn bypass_landings_are_logged() {
        let mut st = state();
        st.schedule.insert(5, LIVE_STAGE_12, 0.0, 9).unwrap();
        st.rearm();
        let got = run_to(&mut st, 5);
        assert_eq!(
            got,
            [Apply::Bypass {
                stage: 12,
                bypassed: true,
                at_once: false
            }]
        );
        let mut log = [ChangeEntry::default(); 4];
        assert_eq!(st.drain_log(&mut log), 1);
        assert_eq!(
            (
                log[0].position,
                log[0].control,
                log[0].before,
                log[0].after,
                log[0].tag,
                log[0].stream
            ),
            (5, LIVE_STAGE_12, 1.0, 0.0, 9, Stream::Playback.word())
        );
        assert_eq!(st.landed_bypass[1], Some((true, 9)));
        // Stage 10 at position 0 of a fresh stream: at once.
        let mut st = state();
        st.schedule.insert(0, LIVE_STAGE_10, 0.0, 1).unwrap();
        st.rearm();
        let got = run_to(&mut st, 0);
        assert!(got.contains(&Apply::Bypass {
            stage: 10,
            bypassed: true,
            at_once: true
        }));
        assert_eq!(st.landed_stage10, Some((true, 1)));
    }

    #[test]
    fn same_value_is_a_noop_entry_and_same_position_pair_logs_both() {
        let mut st = state();
        st.schedule.insert(3, LIVE_STAGE_13, 1.0, 2).unwrap(); // the value in force (on)
        st.rearm();
        run_to(&mut st, 3);
        let mut log = [ChangeEntry::default(); 4];
        assert_eq!(st.drain_log(&mut log), 1);
        assert_eq!((log[0].before, log[0].after), (1.0, 1.0));
        let mut st = state();
        st.schedule.insert(7, LIVE_STAGE_12, 0.0, 4).unwrap();
        st.schedule.insert(7, LIVE_STAGE_12, 1.0, 5).unwrap();
        st.rearm();
        let got = run_to(&mut st, 7);
        assert_eq!(got.len(), 2);
        let mut log = [ChangeEntry::default(); 4];
        assert_eq!(st.drain_log(&mut log), 2);
        assert_eq!(
            (log[0].before, log[0].after, log[1].before, log[1].after),
            (1.0, 0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn mirror_lands_first_with_the_tag_for_reached_stages() {
        let mut st = state();
        st.set_stream(Stream::OnePass);
        st.begin_call(crate::Route::Speaker, 0.5, None, 3.0);
        st.begin_mirror([true, true, false], [11, 22, 33], [true, false, true]);
        let got = run_to(&mut st, 0);
        assert_eq!(
            got[0],
            Apply::Bypass {
                stage: 11,
                bypassed: true,
                at_once: true
            }
        );
        assert!(
            !got.iter()
                .any(|a| matches!(a, Apply::Bypass { stage: 12, .. }))
        );
        let mut log = [ChangeEntry::default(); 8];
        let n = st.drain_log(&mut log);
        assert!(n >= 1);
        assert_eq!(
            (log[0].control, log[0].after, log[0].tag),
            (LIVE_STAGE_11, 0.0, 11)
        );
        // A second call with the same settings: nothing pending.
        st.begin_call(crate::Route::Speaker, 0.5, None, 3.0);
        st.begin_mirror([true, true, false], [11, 22, 33], [true, false, true]);
        let got = run_to(&mut st, 100);
        assert!(!got.iter().any(|a| matches!(a, Apply::Bypass { .. })));
    }
}

// ---- Spec 017: the record-side values -----------------------------------------------------------

/// Which clock a control's positions count in (spec 017 D2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u32)]
pub enum Clock {
    /// Host input samples the stream has processed (the record-side values: the one-pass's
    /// record path and the recorder).
    Input = 0,
    /// Host output samples the stream has produced (the route, the volume, the playback speed:
    /// the player and the one-pass's playback side).
    Output = 1,
}

/// The clock a control's scheduled positions count in (spec 017 D2, FR-010): [`Clock::Input`]
/// for a control whose control manifest paths include the record path (the record path wins
/// for the idle settings on both paths), else [`Clock::Output`]. Total over the manifest.
///
/// # Errors
///
/// [`Error::NoSuchControl`] for an index at or past [`CONTROL_COUNT`](crate::controls::CONTROL_COUNT).
pub fn clock_of(control: u32) -> Result<Clock, Error> {
    let entry = crate::controls::controls()
        .get(control as usize)
        .ok_or(Error::NoSuchControl { control })?;
    if entry.paths.contains(crate::controls::ControlPaths::RECORD) {
        Ok(Clock::Input)
    } else {
        Ok(Clock::Output)
    }
}

/// The device sample at which a gate value (`vas.*`) scheduled at input sample `position` takes
/// effect (spec 017 D3): k(0) = 0 and k(P) = ⌈P · 6000 ÷ rate⌉, the first device sample the
/// conversion produces from input samples at or after P (research R-04). The limiter's and the
/// hiss's values act on sample P itself at the host rate.
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`] for a rate not in
/// [`SUPPORTED_HOST_RATES`](crate::SUPPORTED_HOST_RATES).
pub fn record_landing(position: u64, host_rate_hz: u32) -> Result<u64, Error> {
    if !SUPPORTED_HOST_RATES.contains(&host_rate_hz) {
        return Err(Error::UnsupportedHostRate {
            requested: host_rate_hz,
        });
    }
    let k = (u128::from(position) * u128::from(crate::settings::DEVICE_RATE_HZ))
        .div_ceil(u128::from(host_rate_hz));
    Ok(k as u64)
}

/// The record-side values' slots, in the order of [`LIVE_RECORD`].
pub(crate) const SLOT_SENSITIVITY: usize = 0;
pub(crate) const SLOT_THRESHOLD: usize = 1;
pub(crate) const SLOT_HANG: usize = 2;
pub(crate) const SLOT_ONSET: usize = 3;
pub(crate) const SLOT_KNEE: usize = 4;
pub(crate) const SLOT_ATTACK: usize = 5;
pub(crate) const SLOT_RELEASE: usize = 6;
pub(crate) const SLOT_HISS: usize = 7;

/// The control manifest's indices of the eight record-side values (spec 017 FR-001), by slot:
/// `vas.sensitivity`, `vas.threshold_dbfs`, `vas.hang_ms`, `vas.onset_ms`, `agc.knee_dbfs`,
/// `agc.attack_ms`, `agc.release_ms`, `mic.hiss_offset_db` (pinned by a test against the
/// manifest).
pub(crate) const LIVE_RECORD: [u32; 8] = [15, 16, 17, 18, 10, 11, 12, 28];

/// The setting each slot validates as (spec 017 FR-001: exactly as `validate` does).
pub(crate) const SETTING_OF_SLOT: [crate::error::Setting; 8] = [
    crate::error::Setting::VasSensitivity,
    crate::error::Setting::VasThresholdDbfs,
    crate::error::Setting::VasHangMs,
    crate::error::Setting::VasOnsetMs,
    crate::error::Setting::AgcKneeDbfs,
    crate::error::Setting::AgcAttackMs,
    crate::error::Setting::AgcReleaseMs,
    crate::error::Setting::MicHissOffsetDb,
];

/// A record-side value as the schedule stores it (spec 017 FR-001; research R-06): within the
/// setting's inclusive range after narrowing to single precision (NaN and ±Inf refused), the
/// sensitivity integral; −0.0 normalized.
pub(crate) fn record_value(slot: usize, value: f64) -> Result<f64, Error> {
    let setting = SETTING_OF_SLOT[slot];
    let (lo, hi) = setting.range();
    let v = value as f32;
    if !value.is_finite() || !(v >= lo && v <= hi) {
        return Err(Error::InvalidSetting { setting });
    }
    if slot == SLOT_SENSITIVITY && f64::from(value as u8) != value {
        return Err(Error::InvalidSetting { setting });
    }
    Ok(f64::from(v) + 0.0)
}

/// The slot of a record-side control, or `None` for any other control.
#[inline]
pub(crate) fn slot_of(control: u32) -> Option<usize> {
    LIVE_RECORD.iter().position(|&c| c == control)
}

/// The eight record-side values as one copyable record (data-model.md › Record-side values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RecordValues {
    pub(crate) sensitivity: u8,
    pub(crate) threshold_dbfs: f32,
    pub(crate) hang_ms: f32,
    pub(crate) onset_ms: f32,
    pub(crate) knee_dbfs: f32,
    pub(crate) attack_ms: f32,
    pub(crate) release_ms: f32,
    pub(crate) hiss_offset_db: f32,
}

impl RecordValues {
    /// The values of `settings`.
    pub(crate) fn from(settings: &crate::Settings) -> Self {
        Self {
            sensitivity: settings.vas.sensitivity,
            threshold_dbfs: settings.vas.threshold_dbfs,
            hang_ms: settings.vas.hang_ms,
            onset_ms: settings.vas.onset_ms,
            knee_dbfs: settings.agc.knee_dbfs,
            attack_ms: settings.agc.attack_ms,
            release_ms: settings.agc.release_ms,
            hiss_offset_db: settings.mic.hiss_offset_db,
        }
    }

    /// Writes slot `slot` into `settings` (the engine's readings follow the landings, D4).
    pub(crate) fn write_slot(&self, slot: usize, settings: &mut crate::Settings) {
        match slot {
            SLOT_SENSITIVITY => settings.vas.sensitivity = self.sensitivity,
            SLOT_THRESHOLD => settings.vas.threshold_dbfs = self.threshold_dbfs,
            SLOT_HANG => settings.vas.hang_ms = self.hang_ms,
            SLOT_ONSET => settings.vas.onset_ms = self.onset_ms,
            SLOT_KNEE => settings.agc.knee_dbfs = self.knee_dbfs,
            SLOT_ATTACK => settings.agc.attack_ms = self.attack_ms,
            SLOT_RELEASE => settings.agc.release_ms = self.release_ms,
            _ => settings.mic.hiss_offset_db = self.hiss_offset_db,
        }
    }

    /// Slot `slot` as the entry's double.
    #[inline]
    pub(crate) fn get(&self, slot: usize) -> f64 {
        match slot {
            SLOT_SENSITIVITY => f64::from(self.sensitivity),
            SLOT_THRESHOLD => f64::from(self.threshold_dbfs),
            SLOT_HANG => f64::from(self.hang_ms),
            SLOT_ONSET => f64::from(self.onset_ms),
            SLOT_KNEE => f64::from(self.knee_dbfs),
            SLOT_ATTACK => f64::from(self.attack_ms),
            SLOT_RELEASE => f64::from(self.release_ms),
            _ => f64::from(self.hiss_offset_db),
        }
    }

    /// Sets slot `slot` from the entry's double (validated at the schedule call: within the
    /// setting's range, the sensitivity integral).
    #[inline]
    pub(crate) fn set(&mut self, slot: usize, value: f64) {
        match slot {
            SLOT_SENSITIVITY => self.sensitivity = value as u8,
            SLOT_THRESHOLD => self.threshold_dbfs = value as f32,
            SLOT_HANG => self.hang_ms = value as f32,
            SLOT_ONSET => self.onset_ms = value as f32,
            SLOT_KNEE => self.knee_dbfs = value as f32,
            SLOT_ATTACK => self.attack_ms = value as f32,
            SLOT_RELEASE => self.release_ms = value as f32,
            _ => self.hiss_offset_db = value as f32,
        }
    }

    /// The slots whose values differ between `self` and `other`, as a bit mask.
    fn differing(&self, other: &Self) -> u8 {
        let mut mask = 0u8;
        for slot in 0..8 {
            if self.get(slot) != other.get(slot) {
                mask |= 1 << slot;
            }
        }
        mask
    }
}

/// Deliberate defects of the record-side landing for the spec 017 mutation tests (research
/// R-14); `Default` is the committed behavior. Each is caught by a named oracle: the input-0
/// identity (the apply after the front), the continuity assert (the hold reset at a retune),
/// the reference gate (the counts reset at a hang change), the hiss ratio (a stepped hiss), the
/// k(P) pin (a floored landing), US2 AS4 (the other stream not landing).
#[cfg(feature = "__test-hooks")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordMutation {
    /// The apply point one input sample late (as if after the front), so a value first affects
    /// sample P + 1; folded into the armed position, never into the hot compare.
    pub apply_after_front: bool,
    /// A retune discharges the hold capacitor.
    pub reset_vc_on_retune: bool,
    /// A hang or onset change resets the gate's running counts.
    pub reset_counts_on_hang: bool,
    /// The hiss gain steps to the target at once instead of the 20 ms transition.
    pub step_hiss: bool,
    /// The published k(P) written as ⌊P · 6000 ÷ rate⌋ (one device sample early above 6 kHz).
    pub landing_floor: bool,
    /// A value landed on the other stream never becomes pending here.
    pub skip_other_stream: bool,
}

/// The record side's staging log: the schedule's capacity plus one cross-stream landing per
/// value (data-model.md › Record control state). The recorder itself attains 64 (a one-pass
/// landing reaches it through `start_recording`'s adopt, with no entry); the eight are the
/// one-pass's record side's.
const RECORD_LOG: usize = SCHEDULE_CAPACITY + 8;

/// The record side's control state (spec 017 research R-01; data-model.md › Record control
/// state): the input clock, the schedule, the last applied values, the cross-stream pending
/// changes, the landed record and the staging log. Lives in each `RecordPath`. `repr(C)` with
/// the hot pair `input` / `next_apply` first and the two arrays last (spec 015's lesson).
#[derive(Clone, Debug)]
#[repr(C)]
pub(crate) struct RecordCtl {
    /// The clock: host input samples processed; the next sample has this index (D2).
    pub(crate) input: u64,
    /// 0 while a cross-stream pending change waits, else the schedule's earliest position.
    pub(crate) next_apply: u64,
    log_len: usize,
    /// The last applied values (the entries' `before`).
    last: RecordValues,
    /// The cross-stream pending changes: a slot mask, with the value and the tag per slot.
    pending_mask: u8,
    pending_value: [f64; 8],
    pending_tag: [u64; 8],
    /// The last landed value and tag per slot in the call (the epilogue writes them into the
    /// settings), as a slot mask.
    landed_mask: u8,
    landed_tag: [u64; 8],
    /// The stream word the entries carry (spec 020 D9); set after construction.
    stream: u32,
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: RecordMutation,
    pub(crate) schedule: Schedule,
    log: [ChangeEntry; RECORD_LOG],
}

impl RecordCtl {
    /// A control state whose last applied values are `settings`'s, the clock at 0.
    pub(crate) fn new(settings: &crate::Settings) -> Self {
        Self {
            input: 0,
            next_apply: u64::MAX,
            log_len: 0,
            last: RecordValues::from(settings),
            pending_mask: 0,
            pending_value: [0.0; 8],
            pending_tag: [0; 8],
            landed_mask: 0,
            landed_tag: [0; 8],
            stream: 0,
            #[cfg(feature = "__test-hooks")]
            mutation: RecordMutation::default(),
            schedule: Schedule::new(),
            log: [ChangeEntry::default(); RECORD_LOG],
        }
    }

    /// Sets the stream word the entries carry (spec 020 D9; construction time).
    pub(crate) fn set_stream(&mut self, stream: Stream) {
        self.stream = stream.word();
    }

    /// The last applied values.
    pub(crate) fn last(&self) -> RecordValues {
        self.last
    }

    /// The adopt rule (D4, restated at the plan): the stream's values become `values` with no
    /// entry (`start_recording`, `reset`; the caller applied them to the stages).
    pub(crate) fn set_last(&mut self, values: RecordValues) {
        self.last = values;
    }

    /// A processing call's entry (research R-02): every value of `settings` that differs from
    /// the stream's last applied value becomes a pending cross-stream change with that value's
    /// tag, applied at the call's first input sample; the log and the landed record cleared.
    #[inline]
    pub(crate) fn begin_call(&mut self, settings: &crate::Settings, tags: &[u64; 8]) {
        let values = RecordValues::from(settings);
        let mask = values.differing(&self.last);
        #[cfg(feature = "__test-hooks")]
        let mask = if self.mutation.skip_other_stream {
            0
        } else {
            mask
        };
        self.pending_mask = mask;
        if mask != 0 {
            for (slot, &tag) in tags.iter().enumerate() {
                if mask & (1 << slot) != 0 {
                    self.pending_value[slot] = values.get(slot);
                    self.pending_tag[slot] = tag;
                }
            }
            self.next_apply = 0;
        } else {
            self.next_apply = self.arm(self.schedule.earliest());
        }
        self.log_len = 0;
        self.landed_mask = 0;
    }

    /// Whether the apply point is due for the input sample about to be processed: one `u64`
    /// compare (the hot path).
    #[inline]
    pub(crate) fn due(&self) -> bool {
        self.input >= self.next_apply
    }

    /// The cached earliest position (the `apply_after_front` mutant lands one sample late).
    #[inline]
    fn arm(&self, earliest: u64) -> u64 {
        #[cfg(feature = "__test-hooks")]
        if self.mutation.apply_after_front {
            return earliest.saturating_add(1);
        }
        earliest
    }

    /// The due changes for the input sample about to be processed (the cold path): the
    /// cross-stream pending changes first, in slot order, then every scheduled entry at or
    /// before the position, in position order. `apply` receives the merged values and the slot
    /// that changed; each change is logged (a scheduled one always, D5).
    #[inline(never)]
    pub(crate) fn apply_due(&mut self, apply: &mut dyn FnMut(&RecordValues, usize)) {
        let pos = self.input;
        #[cfg(feature = "__test-hooks")]
        let pos = if self.mutation.apply_after_front {
            pos.saturating_sub(1)
        } else {
            pos
        };
        if self.pending_mask != 0 {
            let mask = core::mem::replace(&mut self.pending_mask, 0);
            for slot in 0..8 {
                if mask & (1 << slot) != 0 {
                    let (value, tag) = (self.pending_value[slot], self.pending_tag[slot]);
                    self.land(pos, slot, value, tag, apply);
                }
            }
        }
        while let Some(e) = self.schedule.pop_due(pos) {
            match slot_of(e.control) {
                Some(slot) => self.land(pos, slot, e.value, e.tag, apply),
                // Validation at the schedule call admits only the eight record-side controls.
                None => debug_assert!(false, "an unknown control in the record schedule"),
            }
        }
        self.next_apply = self.arm(self.schedule.earliest());
    }

    #[inline]
    fn land(
        &mut self,
        pos: u64,
        slot: usize,
        value: f64,
        tag: u64,
        apply: &mut dyn FnMut(&RecordValues, usize),
    ) {
        let before = self.last.get(slot);
        let mut values = self.last;
        values.set(slot, value);
        apply(&values, slot);
        if let Some(e) = self.log.get_mut(self.log_len) {
            *e = ChangeEntry::new(
                pos,
                LIVE_RECORD[slot],
                before,
                values.get(slot),
                tag,
                self.stream,
            );
        }
        self.log_len += 1;
        self.last = values;
        self.landed_mask |= 1 << slot;
        self.landed_tag[slot] = tag;
    }

    /// The call's landed slots and their tags, cleared (the epilogue writes the values into the
    /// settings): `(mask, tags, values)`.
    pub(crate) fn take_landed(&mut self) -> (u8, [u64; 8], RecordValues) {
        let mask = core::mem::replace(&mut self.landed_mask, 0);
        (mask, self.landed_tag, self.last)
    }

    /// Copies the call's entries into `into` (the first ones when it is shorter) and returns
    /// how many the call applied (FR-005).
    pub(crate) fn drain_log(&self, into: &mut [ChangeEntry]) -> usize {
        let n = self.log_len.min(into.len()).min(RECORD_LOG);
        into[..n].copy_from_slice(&self.log[..n]);
        self.log_len
    }

    /// The call's applied-change count.
    pub(crate) fn log_len(&self) -> usize {
        self.log_len
    }

    /// The stream's clock restarts (`start_recording`, the one-pass's `reset`): the pending
    /// changes and the log cleared; never the schedule (D5: `start_recording` keeps it).
    pub(crate) fn reset_clock(&mut self) {
        self.input = 0;
        self.pending_mask = 0;
        self.landed_mask = 0;
        self.log_len = 0;
        self.next_apply = self.arm(self.schedule.earliest());
    }

    /// Drops every pending scheduled change (FR-002).
    pub(crate) fn clear_schedule(&mut self) {
        self.schedule.clear();
        self.rearm();
    }

    /// Holds the apply point on the next input sample (spec 017 research R-05's fold): while a
    /// hiss transition runs, the cold path steps the gain once per sample, so the capsule's
    /// hot path carries no compare.
    #[inline]
    pub(crate) fn hold_next_sample(&mut self) {
        self.next_apply = self.next_apply.min(self.input.saturating_add(1));
    }

    /// Holds the apply point on the sample about to be processed (between samples: a call's
    /// entry, a schedule call, a clear), while a hiss transition runs.
    #[inline]
    pub(crate) fn hold_now(&mut self) {
        self.next_apply = self.next_apply.min(self.input);
    }

    /// Re-arms the apply point after a schedule change (the schedule call): the earliest
    /// pending position, unless a cross-stream pending change keeps it at 0.
    pub(crate) fn rearm(&mut self) {
        if self.next_apply != 0 || self.pending_mask == 0 {
            self.next_apply = self.arm(self.schedule.earliest());
        }
    }

    /// The hang length, in device samples, of the shortest pending hang change, if any (the
    /// event bound, FR-002).
    pub(crate) fn shortest_pending_hang(&self) -> Option<u64> {
        self.schedule
            .entries()
            .iter()
            .filter(|e| e.control == LIVE_RECORD[SLOT_HANG])
            .map(|e| crate::stages::vas::device_samples(e.value as f32))
            .min()
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;

    #[test]
    fn record_ctl_live_record_indices_match_the_manifest() {
        let c = crate::controls::controls();
        let ids = [
            "vas.sensitivity",
            "vas.threshold_dbfs",
            "vas.hang_ms",
            "vas.onset_ms",
            "agc.knee_dbfs",
            "agc.attack_ms",
            "agc.release_ms",
            "mic.hiss_offset_db",
        ];
        for (slot, id) in ids.iter().enumerate() {
            assert_eq!(c[LIVE_RECORD[slot] as usize].identifier, *id, "slot {slot}");
            assert_eq!(slot_of(LIVE_RECORD[slot]), Some(slot));
        }
        assert_eq!(slot_of(0), None);
        assert_eq!(slot_of(99), None);
    }

    #[test]
    fn record_ctl_bound() {
        assert_eq!(MAX_CHANGES_PER_CALL, 142); // 021
        assert_eq!(MAX_CHANGES_PER_CALL, 2 * SCHEDULE_CAPACITY + 3 + 8 + 3); // 021: the three mirrored switches
        assert_eq!(RECORD_LOG, 72);
    }

    #[test]
    fn record_ctl_clock_of() {
        use crate::controls::{ControlPaths, controls};
        for (i, e) in controls().iter().enumerate() {
            let want = if e.paths.contains(ControlPaths::RECORD) {
                Clock::Input
            } else {
                Clock::Output
            };
            assert_eq!(clock_of(i as u32).unwrap(), want, "{}", e.identifier);
        }
        for c in LIVE_RECORD {
            assert_eq!(clock_of(c).unwrap(), Clock::Input);
        }
        for c in [
            LIVE_ROUTE,
            LIVE_VOLUME,
            LIVE_SPEED,
            LIVE_BATTERY,
            LIVE_SPEED_MODE,
        ] {
            assert_eq!(clock_of(c).unwrap(), Clock::Output);
        }
        assert!(matches!(
            clock_of(crate::controls::CONTROL_COUNT as u32),
            Err(Error::NoSuchControl { control: 31 }) // 29 before spec 018, 30 before spec 023
        ));
        assert_eq!(Clock::Input as u32, 0);
        assert_eq!(Clock::Output as u32, 1);
    }

    #[test]
    fn record_ctl_landing_at_the_seven_rates() {
        for rate in SUPPORTED_HOST_RATES {
            assert_eq!(record_landing(0, rate).unwrap(), 0, "{rate}");
        }
        for p in 0..=100 {
            assert_eq!(record_landing(p, 6000).unwrap(), p);
        }
        for (p, k) in [(1, 1), (8, 1), (9, 2), (16, 2), (17, 3)] {
            assert_eq!(record_landing(p, 48_000).unwrap(), k, "48 kHz P={p}");
        }
        for p in 0..=40u64 {
            assert_eq!(
                record_landing(p, 8000).unwrap(),
                (3 * p).div_ceil(4),
                "8 kHz P={p}"
            );
        }
        // The inverse rule: k(P) is the smallest k with ⌊k · rate ÷ 6000⌋ ≥ P, i.e. the first
        // device sample whose input window starts at or after P.
        for rate in SUPPORTED_HOST_RATES {
            let (l, m) = (
                u128::from(crate::settings::DEVICE_RATE_HZ),
                u128::from(rate),
            );
            for p in 1..=400u128 {
                let k = u128::from(record_landing(p as u64, rate).unwrap());
                assert!(k * m / l >= p, "{rate} P={p} k={k}");
                assert!(k == 0 || (k - 1) * m / l < p, "{rate} P={p} k={k}");
            }
        }
        assert!(matches!(
            record_landing(5, 0),
            Err(Error::UnsupportedHostRate { requested: 0 })
        ));
        assert!(matches!(
            record_landing(5, 22_050),
            Err(Error::UnsupportedHostRate { requested: 22_050 })
        ));
    }

    #[test]
    fn record_ctl_record_value_validates_as_the_setting() {
        use crate::error::Setting;
        assert_eq!(record_value(SLOT_SENSITIVITY, 5.0), Ok(5.0));
        assert!(matches!(
            record_value(SLOT_SENSITIVITY, 6.0),
            Err(Error::InvalidSetting {
                setting: Setting::VasSensitivity
            })
        ));
        assert!(matches!(
            record_value(SLOT_SENSITIVITY, 2.5),
            Err(Error::InvalidSetting {
                setting: Setting::VasSensitivity
            })
        ));
        assert_eq!(record_value(SLOT_HANG, 50.0), Ok(50.0));
        assert!(matches!(
            record_value(SLOT_HANG, 20.0),
            Err(Error::InvalidSetting {
                setting: Setting::VasHangMs
            })
        ));
        assert!(matches!(
            record_value(SLOT_KNEE, f64::NAN),
            Err(Error::InvalidSetting {
                setting: Setting::AgcKneeDbfs
            })
        ));
        assert!(matches!(
            record_value(SLOT_HISS, f64::INFINITY),
            Err(Error::InvalidSetting {
                setting: Setting::MicHissOffsetDb
            })
        ));
        assert_eq!(
            record_value(SLOT_THRESHOLD, -0.0).unwrap().to_bits(),
            0.0f64.to_bits()
        );
        // Narrowed to single precision before the range check and the store.
        let v = record_value(SLOT_ATTACK, 3.000_000_000_1).unwrap();
        assert_eq!(v, f64::from(3.000_000_000_1f64 as f32));
        assert_eq!(record_value(SLOT_RELEASE, 10_000.0), Ok(10_000.0));
        assert_eq!(record_value(SLOT_ONSET, 0.0), Ok(0.0));
    }

    #[test]
    fn record_ctl_values_round_trip_and_differ() {
        let s = crate::Settings::new(48_000);
        let mut v = RecordValues::from(&s);
        assert_eq!(v.differing(&v), 0);
        let mut t = s;
        for slot in 0..8 {
            let mut w = v;
            w.set(slot, v.get(slot) + 1.0);
            assert_eq!(w.differing(&v), 1 << slot);
            w.write_slot(slot, &mut t);
            assert_eq!(RecordValues::from(&t).get(slot), v.get(slot) + 1.0);
        }
        v.set(SLOT_SENSITIVITY, 5.0);
        assert_eq!(v.sensitivity, 5);
        assert_eq!(v.get(SLOT_SENSITIVITY), 5.0);
    }

    #[test]
    fn record_ctl_cross_stream_then_scheduled() {
        let s = crate::Settings::new(48_000);
        let mut ctl = RecordCtl::new(&s);
        let mut other = s;
        other.agc.knee_dbfs = -5.0;
        other.vas.hang_ms = 50.0;
        let mut tags = [0u64; 8];
        tags[SLOT_KNEE] = 7;
        tags[SLOT_HANG] = 9;
        ctl.schedule
            .insert(0, LIVE_RECORD[SLOT_HISS], 3.0, 11)
            .unwrap();
        ctl.schedule
            .insert(2, LIVE_RECORD[SLOT_ONSET], 5.0, 12)
            .unwrap();
        ctl.begin_call(&other, &tags);
        assert_eq!(ctl.next_apply, 0);
        let mut seen = alloc::vec::Vec::new();
        assert!(ctl.due());
        ctl.apply_due(&mut |v, slot| seen.push((slot, v.get(slot))));
        // The cross-stream pair in slot order, then the scheduled entry at 0.
        assert_eq!(
            seen,
            alloc::vec![(SLOT_HANG, 50.0), (SLOT_KNEE, -5.0), (SLOT_HISS, 3.0)]
        );
        assert_eq!(ctl.log_len(), 3);
        let mut log = [ChangeEntry::default(); 4];
        assert_eq!(ctl.drain_log(&mut log), 3);
        assert_eq!(
            log[0],
            ChangeEntry::new(0, LIVE_RECORD[SLOT_HANG], 1000.0, 50.0, 9, 0)
        );
        assert_eq!(
            log[1],
            ChangeEntry::new(0, LIVE_RECORD[SLOT_KNEE], -1.0, -5.0, 7, 0)
        );
        assert_eq!(
            log[2],
            ChangeEntry::new(0, LIVE_RECORD[SLOT_HISS], 0.0, 3.0, 11, 0)
        );
        assert_eq!(ctl.next_apply, 2);
        ctl.input = 1;
        assert!(!ctl.due());
        ctl.input = 2;
        assert!(ctl.due());
        ctl.apply_due(&mut |_, _| {});
        assert_eq!(ctl.log_len(), 4);
        assert_eq!(ctl.next_apply, u64::MAX);
        let (mask, t, values) = ctl.take_landed();
        assert_eq!(
            mask,
            (1 << SLOT_HANG) | (1 << SLOT_KNEE) | (1 << SLOT_HISS) | (1 << SLOT_ONSET)
        );
        assert_eq!((t[SLOT_KNEE], t[SLOT_ONSET]), (7, 12));
        assert_eq!(values.onset_ms, 5.0);
        // The settings now equal the last applied values: nothing pending.
        let mut same = other;
        same.mic.hiss_offset_db = 3.0;
        same.vas.onset_ms = 5.0;
        ctl.begin_call(&same, &tags);
        assert_eq!(ctl.next_apply, u64::MAX);
        assert_eq!(ctl.log_len(), 0);
        ctl.schedule
            .insert(10, LIVE_RECORD[SLOT_HANG], 60.0, 1)
            .unwrap();
        ctl.schedule
            .insert(20, LIVE_RECORD[SLOT_HANG], 55.0, 2)
            .unwrap();
        ctl.rearm();
        assert_eq!(ctl.next_apply, 10);
        assert_eq!(ctl.shortest_pending_hang(), Some(330));
        ctl.reset_clock();
        assert_eq!(ctl.input, 0);
        assert_eq!(ctl.schedule.len(), 2);
        ctl.clear_schedule();
        assert_eq!(ctl.shortest_pending_hang(), None);
        assert_eq!(ctl.next_apply, u64::MAX);
    }
}
