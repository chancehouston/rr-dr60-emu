//! The modeled memory: one reserved store of device-rate samples and a span table (spec 006
//! FR-010 – FR-013, FR-017, SC-007; research R-03; data-model.md › Memory).
//!
//! The device holds 99 items in 60 minutes of recorded time (S-001) in a 16 Mbit flash
//! (S-002). This version stores the record path's device-rate `f32` samples (the stored form
//! until the codec is modeled, A-004) and counts capacity in stored device time (A-038): at
//! the default 3600 s × 6000 Hz (A-026) the store holds 21.6 million samples, 86.4 MB,
//! reserved once at construction (`try_reserve_exact`, so a failure is an error and not an
//! abort) and never grown (FR-017). The reservation is virtual on every supported platform:
//! pages are touched as recordings grow.
//!
//! Recordings are numbered 1 to the count in the order made; erasing one moves the later ones
//! down (S-001: "there are no missing numbers"). The span table is `max_recordings + 1` long
//! (the extra span is the recording in progress).

use alloc::vec::Vec;

use crate::error::Error;
use crate::settings::{DEVICE_RATE_HZ, MemorySettings};
use crate::transport::StopResult;

/// One recording's place in the store: `start .. start + len`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// The memory's figures (spec 006 FR-011; contracts/rust-api.md), in device samples; the
/// `*_seconds` helpers divide by the device rate (A-026) in `f64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct MemoryStatus {
    /// The configured capacity, in device samples (`capacity_s` × 6000).
    pub capacity_samples: u64,
    /// Device samples stored, the recording in progress included.
    pub used_samples: u64,
    /// `capacity_samples − used_samples`.
    pub remaining_samples: u64,
    /// The configured recording count.
    pub max_recordings: u32,
    /// Recordings in the memory (the one in progress not counted).
    pub count: u32,
    /// `max_recordings − count`.
    pub slots_remaining: u32,
}

impl MemoryStatus {
    /// The capacity in seconds.
    pub fn capacity_seconds(&self) -> f64 {
        self.capacity_samples as f64 / f64::from(DEVICE_RATE_HZ)
    }

    /// The recorded time used, in seconds.
    pub fn used_seconds(&self) -> f64 {
        self.used_samples as f64 / f64::from(DEVICE_RATE_HZ)
    }

    /// The recorded time remaining, in seconds.
    pub fn remaining_seconds(&self) -> f64 {
        self.remaining_samples as f64 / f64::from(DEVICE_RATE_HZ)
    }
}

/// The store and its span table (research R-03).
#[derive(Debug)]
pub(crate) struct Memory {
    /// Device-rate samples of every recording, in number order, the one in progress last.
    /// Its capacity is `capacity_samples` from construction on and never changes.
    store: Vec<f32>,
    /// One span per recording, in number order; while recording, the open span is last.
    spans: Vec<Span>,
    capacity_samples: usize,
    max_recordings: usize,
    /// A recording is open (the last span is in progress).
    in_progress: bool,
}

impl Memory {
    /// A memory for validated `settings`: `capacity_s × 6000` samples (A-026, A-038) and
    /// `max_recordings` (+ 1 for the recording in progress) spans, reserved exactly.
    ///
    /// # Errors
    ///
    /// [`Error::AllocationFailed`] when the store cannot be reserved (spec 006 FR-017).
    pub(crate) fn new(settings: &MemorySettings) -> Result<Self, Error> {
        let capacity_samples = settings.capacity_s as usize * DEVICE_RATE_HZ as usize; // A-026
        Self::with_capacity_samples(capacity_samples, settings.max_recordings as usize)
    }

    /// A memory of no capacity that reserves nothing: the placeholder `reconfigure` leaves
    /// behind while it moves the real one (spec 006 FR-019; research R-03).
    pub(crate) fn empty() -> Self {
        Self {
            store: Vec::new(),
            spans: Vec::new(),
            capacity_samples: 0,
            max_recordings: 0,
            in_progress: false,
        }
    }

    /// A memory of `capacity_samples` device samples and `max_recordings` recordings.
    pub(crate) fn with_capacity_samples(
        capacity_samples: usize,
        max_recordings: usize,
    ) -> Result<Self, Error> {
        let mut store = Vec::new();
        store
            .try_reserve_exact(capacity_samples)
            .map_err(|_| Error::AllocationFailed)?;
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(max_recordings + 1)
            .map_err(|_| Error::AllocationFailed)?;
        Ok(Self {
            store,
            spans,
            capacity_samples,
            max_recordings,
            in_progress: false,
        })
    }

    /// Opens the recording in progress (spec 006 FR-012; S-001: "FULL" refuses a start).
    ///
    /// # Errors
    ///
    /// [`Error::MemoryFull`] when no recorded time remains (checked first), then
    /// [`Error::NoRecordingSlots`] when the count is at its capacity.
    pub(crate) fn open(&mut self) -> Result<(), Error> {
        debug_assert!(!self.in_progress);
        if self.store.len() >= self.capacity_samples {
            return Err(Error::MemoryFull);
        }
        if self.spans.len() >= self.max_recordings {
            return Err(Error::NoRecordingSlots);
        }
        self.spans.push(Span {
            start: self.store.len(),
            len: 0,
        });
        self.in_progress = true;
        Ok(())
    }

    /// Appends one device sample to the recording in progress. Returns whether room remains:
    /// `false` exactly when the store reached its capacity with this sample (spec 006 FR-012;
    /// the caller stops the recording). A full store is never pushed to, so the store never
    /// reallocates (FR-017).
    #[inline]
    pub(crate) fn append(&mut self, v: f32) -> bool {
        debug_assert!(self.in_progress);
        if self.store.len() >= self.capacity_samples {
            return false;
        }
        self.store.push(v);
        self.store.len() < self.capacity_samples
    }

    /// Closes the recording in progress: kept as recording `count` with its length, or, when
    /// nothing was fed or nothing was stored, discarded (spec 006 FR-012; A-039: an empty item
    /// is not created). Today a fed recording always stores at least the flush, so the second
    /// condition is insurance for FR-012's wording.
    pub(crate) fn close(&mut self, fed_any: bool) -> StopResult {
        debug_assert!(self.in_progress);
        self.in_progress = false;
        let start = self.spans.last().map_or(0, |s| s.start);
        let len = self.store.len() - start;
        if !fed_any || len == 0 {
            debug_assert!(len == 0 || fed_any, "nothing fed, nothing stored");
            self.store.truncate(start);
            self.spans.pop();
            return StopResult::DiscardedEmpty;
        }
        if let Some(span) = self.spans.last_mut() {
            span.len = len;
        }
        StopResult::Kept {
            number: self.spans.len() as u32,
            length: len as u64,
        }
    }

    /// Discards the recording in progress without closing it (spec 006 FR-019: `reset`).
    pub(crate) fn discard_in_progress(&mut self) {
        if self.in_progress {
            self.in_progress = false;
            if let Some(span) = self.spans.pop() {
                self.store.truncate(span.start);
            }
        }
    }

    /// Erases closed recording `number` (spec 006 FR-013; S-001: "any items after the erased
    /// item move down to take its place"): its samples leave the store (a memmove of what
    /// follows, bounded by the store's size, no allocation), its span is removed, and the later
    /// spans move down so the index stays the number minus one. Returns the erased span (its
    /// `start` in the store before the erase, its `len`), or `None` for 0 or a number above the
    /// count. Not real-time safe. Must not be called with a recording in progress.
    pub(crate) fn erase(&mut self, number: u32) -> Option<Span> {
        debug_assert!(!self.in_progress);
        let i = usize::try_from(number).ok()?.checked_sub(1)?;
        if i >= self.count() {
            return None;
        }
        let span = self.spans.remove(i);
        self.store.drain(span.start..span.start + span.len);
        for later in &mut self.spans[i..] {
            later.start -= span.len;
        }
        Some(span)
    }

    /// Adds a recording of the validated `samples` (spec 006 FR-014): refused when they exceed the
    /// remaining time or no slot is left; otherwise appended within the reserved capacity (no
    /// allocation) as a new closed recording, whose number is returned. The samples come from
    /// `samples` in order, so the decode writes straight into the store. Allowed while playing:
    /// it only appends, so earlier recordings do not move.
    ///
    /// # Errors
    ///
    /// [`Error::DoesNotFit`] (time or slots); the memory is unchanged.
    pub(crate) fn import_samples(
        &mut self,
        samples: impl ExactSizeIterator<Item = f32>,
    ) -> Result<u32, Error> {
        debug_assert!(!self.in_progress);
        let k = samples.len();
        if k > self.capacity_samples - self.store.len() || self.spans.len() >= self.max_recordings {
            return Err(Error::DoesNotFit);
        }
        let start = self.store.len();
        self.store.extend(samples);
        assert_eq!(
            self.store.len(),
            start + k,
            "an exact-size iterator keeps its word"
        );
        self.spans.push(Span { start, len: k });
        Ok(self.spans.len() as u32)
    }

    /// Erases every recording (spec 006 FR-013; S-001 "erase all"); the capacity is kept.
    /// Must not be called with a recording in progress.
    pub(crate) fn erase_all(&mut self) {
        debug_assert!(!self.in_progress);
        self.store.clear();
        self.spans.clear();
    }

    /// A recording is open.
    #[cfg(test)]
    pub(crate) fn in_progress(&self) -> bool {
        self.in_progress
    }

    /// Closed recordings in the memory.
    pub(crate) fn count(&self) -> usize {
        self.spans.len() - usize::from(self.in_progress)
    }

    /// The span of closed recording `number` (1 ..= count), if it exists.
    pub(crate) fn span(&self, number: u32) -> Option<Span> {
        let i = usize::try_from(number).ok()?.checked_sub(1)?;
        if i < self.count() {
            Some(self.spans[i])
        } else {
            None
        }
    }

    /// The stored samples of `span`.
    pub(crate) fn samples(&self, span: Span) -> &[f32] {
        &self.store[span.start..span.start + span.len]
    }

    /// Device samples stored, the recording in progress included.
    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        self.store.len()
    }

    /// The figures (spec 006 FR-011).
    pub(crate) fn status(&self) -> MemoryStatus {
        let count = self.count();
        MemoryStatus {
            capacity_samples: self.capacity_samples as u64,
            used_samples: self.store.len() as u64,
            remaining_samples: (self.capacity_samples - self.store.len()) as u64,
            max_recordings: self.max_recordings as u32,
            count: count as u32,
            slots_remaining: (self.max_recordings - count) as u32,
        }
    }

    /// The bytes reserved for the store and the span table (spec 006 SC-007): the store's
    /// capacity × 4 plus the span table's capacity × 16. Virtual until written.
    pub(crate) fn footprint_bytes(&self) -> usize {
        self.store.capacity() * size_of::<f32>() + self.spans.capacity() * size_of::<Span>()
    }

    /// Whether the contents fit a memory of `settings` (spec 006 FR-019).
    pub(crate) fn fits(&self, settings: &MemorySettings) -> bool {
        let capacity = settings.capacity_s as usize * DEVICE_RATE_HZ as usize;
        self.store.len() <= capacity && self.spans.len() <= settings.max_recordings as usize
    }

    /// Whether `settings` describe this memory's own capacity.
    pub(crate) fn same_capacity(&self, settings: &MemorySettings) -> bool {
        self.capacity_samples == settings.capacity_s as usize * DEVICE_RATE_HZ as usize
            && self.max_recordings == settings.max_recordings as usize
    }

    /// A new memory of `settings`' capacity holding this one's recordings (spec 006 FR-019:
    /// `reconfigure` to a different capacity). The contents must fit (`fits`).
    ///
    /// # Errors
    ///
    /// [`Error::AllocationFailed`] when the new store cannot be reserved.
    pub(crate) fn copied_to(&self, settings: &MemorySettings) -> Result<Self, Error> {
        debug_assert!(self.fits(settings) && !self.in_progress);
        let mut m = Self::new(settings)?;
        m.store.extend_from_slice(&self.store);
        m.spans.extend_from_slice(&self.spans);
        Ok(m)
    }

    /// The actual `Vec` capacities of the store and the span table (plan review P6: the
    /// footprint check reads these, not the engine's arithmetic).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn capacities(&self) -> (usize, usize) {
        (self.store.capacity(), self.spans.capacity())
    }
}

/// Manual: `Vec::clone` would reserve only `len`, and a cloned engine would then grow its store
/// while recording (spec 006 FR-017, SC-007). Allocates; not real-time safe.
impl Clone for Memory {
    fn clone(&self) -> Self {
        let mut store = Vec::with_capacity(self.store.capacity());
        store.extend_from_slice(&self.store);
        let mut spans = Vec::with_capacity(self.spans.capacity());
        spans.extend_from_slice(&self.spans);
        Self {
            store,
            spans,
            capacity_samples: self.capacity_samples,
            max_recordings: self.max_recordings,
            in_progress: self.in_progress,
        }
    }
}

/// 006 T018 (spec 006 FR-010, FR-012, FR-017, SC-007; research R-03; data-model › Memory).
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    fn small() -> Memory {
        Memory::with_capacity_samples(100, 3).unwrap()
    }

    /// FR-017, SC-007: the store's capacity is the configured one after `new`, after appending
    /// to the brim and after `close`; it never grows.
    #[test]
    fn reserves_capacity_once() {
        let mut m = small();
        assert_eq!(m.capacities(), (100, 4));
        m.open().unwrap();
        for i in 0..100 {
            let room = m.append(i as f32);
            assert_eq!(room, i < 99, "sample {i}");
        }
        assert_eq!(m.capacities(), (100, 4));
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 100
            }
        );
        assert_eq!(m.capacities(), (100, 4));
        assert_eq!(m.used(), 100);
        // The device figures: 3600 s × 6000 Hz samples, 100 spans (S-001, A-026).
        let d = Memory::new(&MemorySettings::DEVICE).unwrap();
        assert_eq!(d.capacities(), (21_600_000, 100));
        // US2 scenario 1 (FR-011): the device's figures before anything is recorded.
        assert_eq!(
            d.status(),
            MemoryStatus {
                capacity_samples: 21_600_000,
                used_samples: 0,
                remaining_samples: 21_600_000,
                max_recordings: 99,
                count: 0,
                slots_remaining: 99,
            }
        );
        assert_eq!(d.status().remaining_seconds(), 3600.0);
        assert_eq!(d.footprint_bytes(), 21_600_000 * 4 + 100 * 16);
        assert!(d.footprint_bytes() <= 90_000_000, "SC-007 budget"); // engineering target
    }

    /// FR-013: two recordings are contiguous, their lengths add up, their numbers are 1 and 2.
    #[test]
    fn open_append_close_numbering() {
        let mut m = small();
        m.open().unwrap();
        for i in 0..10 {
            assert!(m.append(i as f32));
        }
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 10
            }
        );
        assert_eq!(m.status().count, 1);
        m.open().unwrap();
        assert_eq!(m.status().count, 1, "the one in progress is not counted");
        for i in 0..20 {
            assert!(m.append(100.0 + i as f32));
        }
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 2,
                length: 20
            }
        );
        let (a, b) = (m.span(1).unwrap(), m.span(2).unwrap());
        assert_eq!((a.start, a.len), (0, 10));
        assert_eq!((b.start, b.len), (10, 20));
        assert_eq!(a.len + b.len, m.used());
        assert_eq!(m.samples(b)[0], 100.0);
        assert_eq!(m.span(0), None);
        assert_eq!(m.span(3), None);
        let s = m.status();
        assert_eq!(
            (
                s.used_samples,
                s.remaining_samples,
                s.count,
                s.slots_remaining
            ),
            (30, 70, 2, 1)
        );
    }

    /// FR-012 (coach review of US1, finding 3): a close with input fed but nothing stored is
    /// discarded too; today unreachable through the transport (the flush always stores), so
    /// the memory guards it directly.
    #[test]
    fn close_with_input_but_nothing_stored_discards() {
        let mut m = small();
        m.open().unwrap();
        assert_eq!(m.close(true), StopResult::DiscardedEmpty);
        assert_eq!(m.count(), 0);
        assert_eq!(m.used(), 0);
    }

    /// FR-012, A-039: a close with nothing fed discards the recording and pops its span.
    #[test]
    fn close_without_input_discards() {
        let mut m = small();
        m.open().unwrap();
        assert_eq!(m.close(false), StopResult::DiscardedEmpty);
        assert_eq!(m.count(), 0);
        assert_eq!(m.used(), 0);
        assert_eq!(m.capacities(), (100, 4));
        // Reset's discard with samples already stored truncates the store.
        m.open().unwrap();
        assert!(m.append(1.0));
        m.discard_in_progress();
        assert_eq!((m.count(), m.used()), (0, 0));
        assert!(!m.in_progress());
    }

    /// SC-007: a clone reserves the same capacities.
    #[test]
    fn clone_keeps_capacity() {
        let mut m = small();
        m.open().unwrap();
        assert!(m.append(0.5));
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 1
            }
        );
        let c = m.clone();
        assert_eq!(c.capacities(), m.capacities());
        assert_eq!(c.footprint_bytes(), m.footprint_bytes());
        assert_eq!(c.samples(c.span(1).unwrap()), m.samples(m.span(1).unwrap()));
    }

    /// FR-017 (plan review P7): a reservation the allocator cannot grant is an error.
    #[test]
    fn try_reserve_failure_is_an_error() {
        assert_eq!(
            Memory::with_capacity_samples(usize::MAX / 2, 99).err(),
            Some(Error::AllocationFailed)
        );
        assert_eq!(
            Memory::with_capacity_samples(100, usize::MAX / 32).err(),
            Some(Error::AllocationFailed)
        );
    }

    /// SC-007: the footprint is the two reserved capacities in bytes.
    #[test]
    fn footprint_bytes_formula() {
        let m = Memory::with_capacity_samples(1000, 9).unwrap();
        let (store, spans) = m.capacities();
        assert_eq!(m.footprint_bytes(), store * 4 + spans * 16);
        assert_eq!((store, spans), (1000, 10));
        let s = m.status();
        assert_eq!(s.capacity_seconds(), 1000.0 / 6000.0);
        assert_eq!(s.remaining_seconds(), 1000.0 / 6000.0);
        assert_eq!(s.used_seconds(), 0.0);
    }

    /// Five recordings of lengths 10, 20, 30, 40, 50 with values `number + i / 100`.
    fn five() -> Memory {
        let mut m = Memory::with_capacity_samples(200, 5).unwrap();
        for n in 1..=5u32 {
            m.open().unwrap();
            for i in 0..(10 * n as usize) {
                assert!(m.append(n as f32 + i as f32 / 100.0));
            }
            assert_eq!(
                m.close(true),
                StopResult::Kept {
                    number: n,
                    length: 10 * u64::from(n)
                }
            );
        }
        m
    }

    /// 006 T026 (FR-012): `append` returns `false` exactly at the capacity sample, a full store
    /// refuses another sample, and the store never grows.
    #[test]
    fn append_returns_false_exactly_at_capacity() {
        let mut m = Memory::with_capacity_samples(5, 1).unwrap();
        m.open().unwrap();
        assert_eq!(
            (0..5).map(|i| m.append(i as f32)).collect::<Vec<_>>(),
            [true, true, true, true, false]
        );
        assert!(!m.append(9.0), "a full store takes nothing more");
        assert_eq!(m.used(), 5);
        assert_eq!(m.capacities(), (5, 2));
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 5
            }
        );
        assert_eq!(m.status().remaining_samples, 0);
    }

    /// 006 T026 (FR-012; S-001 "FULL"): a start is refused with `MemoryFull` when no time
    /// remains and `NoRecordingSlots` when the count is at its capacity; time first when both.
    #[test]
    fn open_refused_when_full_or_no_slot() {
        // No time, slots left.
        let mut m = Memory::with_capacity_samples(3, 2).unwrap();
        m.open().unwrap();
        for i in 0..3 {
            m.append(i as f32);
        }
        assert!(matches!(m.close(true), StopResult::Kept { number: 1, .. }));
        assert_eq!(m.open(), Err(Error::MemoryFull));
        assert!(!m.in_progress());
        assert_eq!(m.status().slots_remaining, 1);
        // Time left, no slots.
        let mut m = Memory::with_capacity_samples(100, 2).unwrap();
        for n in 1..=2 {
            m.open().unwrap();
            assert!(m.append(0.5));
            assert!(matches!(m.close(true), StopResult::Kept { number, .. } if number == n));
        }
        assert_eq!(m.open(), Err(Error::NoRecordingSlots));
        assert_eq!(m.status().remaining_samples, 98);
        // Both: the time refusal comes first.
        let mut m = Memory::with_capacity_samples(1, 1).unwrap();
        m.open().unwrap();
        assert!(!m.append(0.5));
        assert!(matches!(m.close(true), StopResult::Kept { number: 1, .. }));
        assert_eq!(m.open(), Err(Error::MemoryFull));
    }

    /// 006 T026 (FR-013; S-001 "there are no missing numbers"): erasing 3 of 5 leaves four,
    /// the former 4 and 5 at 3 and 4 with their samples unchanged, the remaining time grows by
    /// the erased length, and Σ len = used.
    #[test]
    fn erase_renumbers_and_returns_time() {
        let mut m = five();
        let before = m.status();
        assert_eq!(
            (before.count, before.used_samples, before.remaining_samples),
            (5, 150, 50)
        );
        let four = m.samples(m.span(4).unwrap()).to_vec();
        let five_ = m.samples(m.span(5).unwrap()).to_vec();
        let one = m.samples(m.span(1).unwrap()).to_vec();
        assert_eq!(m.erase(3), Some(Span { start: 30, len: 30 }));
        let after = m.status();
        assert_eq!(
            (after.count, after.used_samples, after.remaining_samples),
            (4, 120, 80)
        );
        assert_eq!(after.remaining_samples - before.remaining_samples, 30);
        assert_eq!(m.samples(m.span(1).unwrap()), &one[..]);
        assert_eq!(m.samples(m.span(3).unwrap()), &four[..]);
        assert_eq!(m.samples(m.span(4).unwrap()), &five_[..]);
        assert_eq!(m.span(5), None);
        let spans: Vec<_> = (1..=4).map(|n| m.span(n).unwrap()).collect();
        assert_eq!(spans.iter().map(|s| s.len).sum::<usize>(), m.used());
        for w in spans.windows(2) {
            assert_eq!(w[0].start + w[0].len, w[1].start, "contiguous");
        }
        assert_eq!(spans[0].start, 0);
        // Out of range and zero.
        assert_eq!(m.erase(0), None);
        assert_eq!(m.erase(5), None);
        assert_eq!(m.status().count, 4);
        // Erasing the last (the former 5, moved down to 70) and the first.
        assert_eq!(m.erase(4), Some(Span { start: 70, len: 50 }));
        assert_eq!(m.erase(1), Some(Span { start: 0, len: 10 }));
        assert_eq!(m.status().count, 2);
        assert_eq!(m.span(1).unwrap(), Span { start: 0, len: 20 });
        assert_eq!(m.span(2).unwrap(), Span { start: 20, len: 40 });
        assert_eq!(m.samples(m.span(2).unwrap()), &four[..]);
        assert_eq!(m.capacities(), (200, 6), "no allocation");
    }

    /// 006 T026 (FR-013): erase all empties the memory and keeps the reserved capacity.
    #[test]
    fn erase_all_empties_and_keeps_capacity() {
        let mut m = five();
        m.erase_all();
        let s = m.status();
        assert_eq!(
            (
                s.count,
                s.used_samples,
                s.remaining_samples,
                s.slots_remaining
            ),
            (0, 0, 200, 5)
        );
        assert_eq!(m.span(1), None);
        assert_eq!(m.capacities(), (200, 6));
        // Usable again, numbering restarts at 1.
        m.open().unwrap();
        assert!(m.append(1.0));
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 1
            }
        );
    }

    /// 006 T026 (FR-011; A-026, A-038): the figures in device samples and the derived seconds
    /// are exact at every step.
    #[test]
    fn status_figures_exact() {
        let mut m = Memory::new(&MemorySettings {
            capacity_s: 10,
            max_recordings: 3,
        })
        .unwrap();
        let s = m.status();
        assert_eq!(
            (s.capacity_samples, s.used_samples, s.remaining_samples),
            (60_000, 0, 60_000)
        );
        assert_eq!((s.max_recordings, s.count, s.slots_remaining), (3, 0, 3));
        assert_eq!(s.capacity_seconds(), 10.0);
        assert_eq!(s.remaining_seconds(), 10.0);
        m.open().unwrap();
        for _ in 0..9000 {
            assert!(m.append(0.1));
        }
        let s = m.status();
        assert_eq!(
            (s.used_samples, s.remaining_samples, s.count),
            (9000, 51_000, 0)
        );
        assert_eq!(s.used_seconds(), 1.5);
        assert_eq!(s.remaining_seconds(), 8.5);
        assert!(matches!(
            m.close(true),
            StopResult::Kept { length: 9000, .. }
        ));
        let s = m.status();
        assert_eq!((s.count, s.slots_remaining), (1, 2));
        assert_eq!(
            m.erase(1),
            Some(Span {
                start: 0,
                len: 9000
            })
        );
        let s = m.status();
        assert_eq!(
            (s.used_samples, s.remaining_samples, s.count),
            (0, 60_000, 0)
        );
    }

    /// FR-019: fits / same capacity / copy for `reconfigure`.
    #[test]
    fn fits_and_copies_into_a_larger_memory() {
        let mut m = Memory::new(&MemorySettings {
            capacity_s: 1,
            max_recordings: 2,
        })
        .unwrap();
        m.open().unwrap();
        for _ in 0..6000 {
            m.append(0.25);
        }
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 6000
            }
        );
        let same = MemorySettings {
            capacity_s: 1,
            max_recordings: 2,
        };
        let larger = MemorySettings {
            capacity_s: 2,
            max_recordings: 2,
        };
        let fewer = MemorySettings {
            capacity_s: 2,
            max_recordings: 0,
        };
        assert!(m.same_capacity(&same) && m.fits(&same));
        assert!(!m.same_capacity(&larger) && m.fits(&larger));
        assert!(!m.fits(&fewer));
        let c = m.copied_to(&larger).unwrap();
        assert_eq!(c.capacities(), (12_000, 3));
        assert_eq!(c.count(), 1);
        assert_eq!(c.samples(c.span(1).unwrap()), m.samples(m.span(1).unwrap()));
    }
}
