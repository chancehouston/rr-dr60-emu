//! The modeled memory: one reserved store of device-rate samples and a span table (spec 006
//! FR-010 – FR-013, FR-017, SC-007; research R-03; data-model.md › Memory).
//!
//! The device holds 99 items in 60 minutes of recorded time (S-001) in a 16 Mbit flash
//! (S-002). The memory stores one of two **encodings** (spec 008 FR-010, FR-012; research
//! R-07), fixed at construction by the speech-codec setting: 10-byte frames per 120 device
//! samples with the codec on (1.8 MB at the default 60 minutes), or the record path's
//! device-rate `f32` samples with it bypassed (21.6 million samples, 86.4 MB, as spec 006).
//! Capacity is counted in stored device time (A-038: units × the unit's samples), reserved once
//! at construction (`try_reserve_exact`, so a failure is an error and not an abort) and never
//! grown (FR-017). The reservation is virtual on every supported platform: pages are touched
//! as recordings grow.
//!
//! Recordings are numbered 1 to the count in the order made; erasing one moves the later ones
//! down (S-001: "there are no missing numbers"). The span table is `max_recordings + 1` long
//! (the extra span is the recording in progress).

use alloc::vec::Vec;

use crate::error::Error;
use crate::settings::{DEVICE_RATE_HZ, MemorySettings};
use crate::transport::StopResult;

/// How the memory stores a recording (spec 008 FR-010, FR-012): fixed by the engine's
/// `speech_codec_enabled` setting at construction or reconfigure. One memory holds one encoding;
/// an import of the other is refused with [`Error::EncodingDiffers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Encoding {
    /// Speech-codec frames: 10 bytes per 120 device samples (spec 008; A-040). The codec on.
    Frames,
    /// Device-rate samples as 32-bit bit patterns: 4 bytes per sample (spec 006). The codec
    /// bypassed; a combination the device cannot produce.
    Samples,
}

impl Encoding {
    /// Bytes per stored unit: 10 for frames, 4 for samples.
    pub const fn unit_bytes(self) -> usize {
        match self {
            Encoding::Frames => crate::codec::FRAME_BYTES as usize,
            Encoding::Samples => size_of::<f32>(),
        }
    }

    /// Device samples per stored unit: 120 for frames, 1 for samples.
    pub const fn unit_samples(self) -> usize {
        match self {
            Encoding::Frames => crate::codec::FRAME_SAMPLES as usize,
            Encoding::Samples => 1,
        }
    }

    /// The form's encoding field (contracts/recording-form.md): 1 for frames, 2 for samples.
    pub const fn form_code(self) -> u16 {
        match self {
            Encoding::Frames => 1,
            Encoding::Samples => 2,
        }
    }

    /// The encoding the engine's settings select (spec 008 FR-012).
    pub const fn for_settings(settings: &crate::Settings) -> Self {
        if settings.speech_codec_enabled {
            Encoding::Frames
        } else {
            Encoding::Samples
        }
    }
}

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

/// The stored data: device-rate samples (spec 006) or speech-codec frames (spec 008 R-07), one
/// per memory, fixed at construction.
#[derive(Debug)]
enum Store {
    /// One `f32` per device sample (the codec bypassed).
    Samples(Vec<f32>),
    /// 10 bytes per 120-sample frame (the codec on).
    Frames(Vec<u8>),
}

/// The store and its span table (research R-03; spec 008 R-07: spans count **units**, a device
/// sample or a frame).
#[derive(Debug)]
pub(crate) struct Memory {
    /// Every recording's units, in number order, the one in progress last. Its capacity is
    /// `capacity_units` (× the unit's bytes) from construction on and never changes.
    store: Store,
    encoding: Encoding,
    /// One span per recording, in number order, in units; while recording, the open span is
    /// last.
    spans: Vec<Span>,
    capacity_units: usize,
    max_recordings: usize,
    /// A recording is open (the last span is in progress).
    in_progress: bool,
}

/// The capacity in stored units for `capacity_s` seconds at `unit_samples` device samples per
/// unit, rounded down to whole units (spec 008 FR-013; A-026, A-038). Exact for the codec's 120
/// at every whole-second capacity (6000 / 120 = 50) and for 1.
pub(crate) const fn capacity_units_for(capacity_s: u32, unit_samples: usize) -> usize {
    (capacity_s as usize * DEVICE_RATE_HZ as usize) / unit_samples
}

impl Memory {
    /// A memory for validated `settings` in `encoding`: `capacity_s × 6000 / unit_samples` units
    /// (A-026, A-038; spec 008 FR-013) and `max_recordings` (+ 1 for the recording in progress)
    /// spans, reserved exactly.
    ///
    /// # Errors
    ///
    /// [`Error::AllocationFailed`] when the store cannot be reserved (spec 006 FR-017).
    pub(crate) fn new(settings: &MemorySettings, encoding: Encoding) -> Result<Self, Error> {
        let units = capacity_units_for(settings.capacity_s, encoding.unit_samples());
        Self::with_units(units, settings.max_recordings as usize, encoding)
    }

    /// A memory of no capacity that reserves nothing: the placeholder `reconfigure` leaves
    /// behind while it moves the real one (spec 006 FR-019; research R-03).
    pub(crate) fn empty() -> Self {
        Self {
            store: Store::Samples(Vec::new()),
            encoding: Encoding::Samples,
            spans: Vec::new(),
            capacity_units: 0,
            max_recordings: 0,
            in_progress: false,
        }
    }

    /// A memory of `capacity_units` stored units in `encoding` and `max_recordings` recordings.
    pub(crate) fn with_units(
        capacity_units: usize,
        max_recordings: usize,
        encoding: Encoding,
    ) -> Result<Self, Error> {
        let store = match encoding {
            Encoding::Samples => {
                let mut v = Vec::new();
                v.try_reserve_exact(capacity_units)
                    .map_err(|_| Error::AllocationFailed)?;
                Store::Samples(v)
            }
            Encoding::Frames => {
                let mut v = Vec::new();
                v.try_reserve_exact(
                    capacity_units
                        .checked_mul(encoding.unit_bytes())
                        .ok_or(Error::AllocationFailed)?,
                )
                .map_err(|_| Error::AllocationFailed)?;
                Store::Frames(v)
            }
        };
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(max_recordings + 1)
            .map_err(|_| Error::AllocationFailed)?;
        Ok(Self {
            store,
            encoding,
            spans,
            capacity_units,
            max_recordings,
            in_progress: false,
        })
    }

    /// A memory of `capacity_samples` device samples in the samples encoding (spec 006's
    /// constructor, kept for its tests).
    #[cfg(test)]
    pub(crate) fn with_capacity_samples(
        capacity_samples: usize,
        max_recordings: usize,
    ) -> Result<Self, Error> {
        Self::with_units(capacity_samples, max_recordings, Encoding::Samples)
    }

    #[allow(dead_code)] // 008 T020: used once the codec stores frames
    /// The memory's encoding (spec 008 FR-012).
    pub(crate) fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Stored units (the recording in progress included).
    fn used_units(&self) -> usize {
        match &self.store {
            Store::Samples(v) => v.len(),
            Store::Frames(v) => v.len() / self.encoding.unit_bytes(),
        }
    }

    /// Opens the recording in progress (spec 006 FR-012; S-001: "FULL" refuses a start).
    ///
    /// # Errors
    ///
    /// [`Error::MemoryFull`] when no recorded time remains (checked first), then
    /// [`Error::NoRecordingSlots`] when the count is at its capacity.
    pub(crate) fn open(&mut self) -> Result<(), Error> {
        debug_assert!(!self.in_progress);
        if self.used_units() >= self.capacity_units {
            return Err(Error::MemoryFull);
        }
        if self.spans.len() >= self.max_recordings {
            return Err(Error::NoRecordingSlots);
        }
        self.spans.push(Span {
            start: self.used_units(),
            len: 0,
        });
        self.in_progress = true;
        Ok(())
    }

    /// Appends one device sample to the recording in progress (the samples encoding). Returns
    /// whether room remains: `false` exactly when the store reached its capacity with this
    /// sample (spec 006 FR-012; the caller stops the recording). A full store is never pushed
    /// to, so the store never reallocates (FR-017).
    #[inline]
    pub(crate) fn append_sample(&mut self, v: f32) -> bool {
        debug_assert!(self.in_progress);
        let capacity = self.capacity_units;
        match &mut self.store {
            Store::Samples(store) => {
                if store.len() >= capacity {
                    return false;
                }
                store.push(v);
                store.len() < capacity
            }
            // coverage: unreachable by construction (Constitution III): the transport appends
            // samples only to a memory in the samples encoding (spec 008 FR-012).
            Store::Frames(_) => unreachable!("append_sample on a frames memory"),
        }
    }

    #[allow(dead_code)] // 008 T020: used once the codec stores frames
    /// Appends one speech-codec frame to the recording in progress (the frames encoding; spec
    /// 008 FR-010, FR-013). Returns whether room remains: `false` exactly when the store
    /// reached its capacity with this frame, so "memory full" falls on a frame boundary.
    #[inline]
    pub(crate) fn append_frame(&mut self, frame: &crate::codec::Frame) -> bool {
        debug_assert!(self.in_progress);
        let capacity = self.capacity_units;
        match &mut self.store {
            Store::Frames(store) => {
                let used = store.len() / frame.len();
                if used >= capacity {
                    return false;
                }
                store.extend_from_slice(frame);
                used + 1 < capacity
            }
            // coverage: unreachable by construction (Constitution III): see `append_sample`.
            Store::Samples(_) => unreachable!("append_frame on a samples memory"),
        }
    }

    /// Closes the recording in progress: kept as recording `count` with its length in device
    /// samples, or, when nothing was fed or nothing was stored, discarded (spec 006 FR-012;
    /// A-039: an empty item is not created).
    pub(crate) fn close(&mut self, fed_any: bool) -> StopResult {
        debug_assert!(self.in_progress);
        self.in_progress = false;
        let start = self.spans.last().map_or(0, |s| s.start);
        let len = self.used_units() - start;
        if !fed_any || len == 0 {
            debug_assert!(len == 0 || fed_any, "nothing fed, nothing stored");
            self.truncate_units(start);
            self.spans.pop();
            return StopResult::DiscardedEmpty;
        }
        if let Some(span) = self.spans.last_mut() {
            span.len = len;
        }
        StopResult::Kept {
            number: self.spans.len() as u32,
            length: (len * self.encoding.unit_samples()) as u64,
        }
    }

    fn truncate_units(&mut self, units: usize) {
        match &mut self.store {
            Store::Samples(v) => v.truncate(units),
            Store::Frames(v) => v.truncate(units * self.encoding.unit_bytes()),
        }
    }

    /// Discards the recording in progress without closing it (spec 006 FR-019: `reset`).
    pub(crate) fn discard_in_progress(&mut self) {
        if self.in_progress {
            self.in_progress = false;
            if let Some(span) = self.spans.pop() {
                self.truncate_units(span.start);
            }
        }
    }

    /// Erases closed recording `number` (spec 006 FR-013; S-001: "any items after the erased
    /// item move down to take its place"): its units leave the store (a memmove of what
    /// follows, bounded by the store's size, no allocation), its span is removed, and the later
    /// spans move down so the index stays the number minus one. Returns the erased span (its
    /// `start` in units before the erase, its `len`), or `None` for 0 or a number above the
    /// count. Not real-time safe. Must not be called with a recording in progress.
    pub(crate) fn erase(&mut self, number: u32) -> Option<Span> {
        debug_assert!(!self.in_progress);
        let i = usize::try_from(number).ok()?.checked_sub(1)?;
        if i >= self.count() {
            return None;
        }
        let span = self.spans.remove(i);
        match &mut self.store {
            Store::Samples(v) => {
                v.drain(span.start..span.start + span.len);
            }
            Store::Frames(v) => {
                let b = self.encoding.unit_bytes();
                v.drain(span.start * b..(span.start + span.len) * b);
            }
        }
        for later in &mut self.spans[i..] {
            later.start -= span.len;
        }
        Some(span)
    }

    /// Adds a recording of the validated `samples` (spec 006 FR-014): refused when the memory
    /// holds frames (spec 008 FR-012), when they exceed the remaining time or when no slot is
    /// left; otherwise appended within the reserved capacity (no allocation) as a new closed
    /// recording, whose number is returned. Allowed while playing: it only appends, so earlier
    /// recordings do not move.
    ///
    /// # Errors
    ///
    /// [`Error::EncodingDiffers`], then [`Error::DoesNotFit`] (time or slots); the memory is
    /// unchanged.
    pub(crate) fn import_samples(
        &mut self,
        samples: impl ExactSizeIterator<Item = f32>,
    ) -> Result<u32, Error> {
        debug_assert!(!self.in_progress);
        let Store::Samples(store) = &mut self.store else {
            return Err(Error::EncodingDiffers);
        };
        let k = samples.len();
        if k > self.capacity_units - store.len() || self.spans.len() >= self.max_recordings {
            return Err(Error::DoesNotFit);
        }
        let start = store.len();
        store.extend(samples);
        assert_eq!(
            store.len(),
            start + k,
            "an exact-size iterator keeps its word"
        );
        self.spans.push(Span { start, len: k });
        Ok(self.spans.len() as u32)
    }

    #[allow(dead_code)] // 008 T028: used once the codec stores frames
    /// Adds a recording of validated frame bytes (a multiple of the frame size; spec 008
    /// FR-010): refused when the memory holds samples, when the frames exceed the remaining
    /// time or when no slot is left.
    ///
    /// # Errors
    ///
    /// [`Error::EncodingDiffers`], then [`Error::DoesNotFit`]; the memory is unchanged.
    pub(crate) fn import_frames(&mut self, frames: &[u8]) -> Result<u32, Error> {
        debug_assert!(!self.in_progress);
        let b = self.encoding.unit_bytes();
        let Store::Frames(store) = &mut self.store else {
            return Err(Error::EncodingDiffers);
        };
        debug_assert!(frames.len() % b == 0, "validated frame bytes");
        let k = frames.len() / b;
        let used = store.len() / b;
        if k > self.capacity_units - used || self.spans.len() >= self.max_recordings {
            return Err(Error::DoesNotFit);
        }
        store.extend_from_slice(frames);
        self.spans.push(Span {
            start: used,
            len: k,
        });
        Ok(self.spans.len() as u32)
    }

    /// Erases every recording (spec 006 FR-013; S-001 "erase all"); the capacity is kept.
    /// Must not be called with a recording in progress.
    pub(crate) fn erase_all(&mut self) {
        debug_assert!(!self.in_progress);
        self.truncate_units(0);
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

    /// The span of closed recording `number` (1 ..= count), if it exists (in units).
    pub(crate) fn span(&self, number: u32) -> Option<Span> {
        let i = usize::try_from(number).ok()?.checked_sub(1)?;
        if i < self.count() {
            Some(self.spans[i])
        } else {
            None
        }
    }

    /// The stored samples of `span` (the samples encoding).
    pub(crate) fn samples(&self, span: Span) -> &[f32] {
        match &self.store {
            Store::Samples(v) => &v[span.start..span.start + span.len],
            // coverage: unreachable by construction (Constitution III): callers read samples
            // from a samples memory only (spec 008 FR-012).
            Store::Frames(_) => unreachable!("samples of a frames memory"),
        }
    }

    #[allow(dead_code)] // 008 T020: used once the codec stores frames
    /// The stored frame bytes of `span` (the frames encoding): `len × 10` bytes.
    pub(crate) fn frames(&self, span: Span) -> &[u8] {
        let b = self.encoding.unit_bytes();
        match &self.store {
            Store::Frames(v) => &v[span.start * b..(span.start + span.len) * b],
            // coverage: unreachable by construction (Constitution III): see `samples`.
            Store::Samples(_) => unreachable!("frames of a samples memory"),
        }
    }

    /// Device samples stored, the recording in progress included.
    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        self.used_units() * self.encoding.unit_samples()
    }

    /// The figures (spec 006 FR-011), in device samples (units × the unit's samples).
    pub(crate) fn status(&self) -> MemoryStatus {
        let count = self.count();
        let per = self.encoding.unit_samples() as u64;
        let used = self.used_units() as u64;
        MemoryStatus {
            capacity_samples: self.capacity_units as u64 * per,
            used_samples: used * per,
            remaining_samples: (self.capacity_units as u64 - used) * per,
            max_recordings: self.max_recordings as u32,
            count: count as u32,
            slots_remaining: (self.max_recordings - count) as u32,
        }
    }

    /// The bytes reserved for the store and the span table (spec 006 SC-007; spec 008 SC-007):
    /// the store's capacity in bytes plus the span table's capacity × 16. Virtual until written.
    pub(crate) fn footprint_bytes(&self) -> usize {
        let store = match &self.store {
            Store::Samples(v) => v.capacity() * size_of::<f32>(),
            Store::Frames(v) => v.capacity(),
        };
        store + self.spans.capacity() * size_of::<Span>()
    }

    /// Whether the contents fit a memory of `settings` in `encoding` (spec 006 FR-019; spec 008
    /// FR-012: another encoding fits only an empty memory).
    pub(crate) fn fits(&self, settings: &MemorySettings, encoding: Encoding) -> bool {
        let same_encoding =
            encoding == self.encoding || (self.spans.is_empty() && !self.in_progress);
        same_encoding
            && self.used_units()
                <= capacity_units_for(settings.capacity_s, self.encoding.unit_samples())
            && self.spans.len() <= settings.max_recordings as usize
    }

    /// Whether `settings` and `encoding` describe this memory's own capacity and encoding.
    pub(crate) fn same_capacity(&self, settings: &MemorySettings, encoding: Encoding) -> bool {
        encoding == self.encoding
            && self.capacity_units
                == capacity_units_for(settings.capacity_s, encoding.unit_samples())
            && self.max_recordings == settings.max_recordings as usize
    }

    /// A new memory of `settings`' capacity in `encoding` holding this one's recordings (spec
    /// 006 FR-019: `reconfigure` to a different capacity; spec 008 FR-012: to the other encoding
    /// only when empty). The contents must fit (`fits`).
    ///
    /// # Errors
    ///
    /// [`Error::AllocationFailed`] when the new store cannot be reserved.
    pub(crate) fn copied_to(
        &self,
        settings: &MemorySettings,
        encoding: Encoding,
    ) -> Result<Self, Error> {
        debug_assert!(self.fits(settings, encoding) && !self.in_progress);
        let mut m = Self::new(settings, encoding)?;
        match (&self.store, &mut m.store) {
            (Store::Samples(a), Store::Samples(b)) => b.extend_from_slice(a),
            (Store::Frames(a), Store::Frames(b)) => b.extend_from_slice(a),
            _ => debug_assert!(
                self.spans.is_empty(),
                "an encoding change needs an empty memory"
            ),
        }
        m.spans.extend_from_slice(&self.spans);
        Ok(m)
    }

    /// The actual `Vec` capacities of the store (in units) and the span table (plan review P6:
    /// the footprint check reads these, not the engine's arithmetic).
    #[cfg(any(test, feature = "__test-hooks"))]
    pub(crate) fn capacities(&self) -> (usize, usize) {
        let store = match &self.store {
            Store::Samples(v) => v.capacity(),
            Store::Frames(v) => v.capacity() / self.encoding.unit_bytes(),
        };
        (store, self.spans.capacity())
    }
}

/// Manual: `Vec::clone` would reserve only `len`, and a cloned engine would then grow its store
/// while recording (spec 006 FR-017, SC-007). Allocates; not real-time safe.
impl Clone for Memory {
    fn clone(&self) -> Self {
        let store = match &self.store {
            Store::Samples(v) => {
                let mut c = Vec::with_capacity(v.capacity());
                c.extend_from_slice(v);
                Store::Samples(c)
            }
            Store::Frames(v) => {
                let mut c = Vec::with_capacity(v.capacity());
                c.extend_from_slice(v);
                Store::Frames(c)
            }
        };
        let mut spans = Vec::with_capacity(self.spans.capacity());
        spans.extend_from_slice(&self.spans);
        Self {
            store,
            encoding: self.encoding,
            spans,
            capacity_units: self.capacity_units,
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
            let room = m.append_sample(i as f32);
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
        let d = Memory::new(&MemorySettings::DEVICE, Encoding::Samples).unwrap();
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
            assert!(m.append_sample(i as f32));
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
            assert!(m.append_sample(100.0 + i as f32));
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
        assert!(m.append_sample(1.0));
        m.discard_in_progress();
        assert_eq!((m.count(), m.used()), (0, 0));
        assert!(!m.in_progress());
    }

    /// SC-007: a clone reserves the same capacities.
    #[test]
    fn clone_keeps_capacity() {
        let mut m = small();
        m.open().unwrap();
        assert!(m.append_sample(0.5));
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
                assert!(m.append_sample(n as f32 + i as f32 / 100.0));
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
            (0..5)
                .map(|i| m.append_sample(i as f32))
                .collect::<Vec<_>>(),
            [true, true, true, true, false]
        );
        assert!(!m.append_sample(9.0), "a full store takes nothing more");
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
            m.append_sample(i as f32);
        }
        assert!(matches!(m.close(true), StopResult::Kept { number: 1, .. }));
        assert_eq!(m.open(), Err(Error::MemoryFull));
        assert!(!m.in_progress());
        assert_eq!(m.status().slots_remaining, 1);
        // Time left, no slots.
        let mut m = Memory::with_capacity_samples(100, 2).unwrap();
        for n in 1..=2 {
            m.open().unwrap();
            assert!(m.append_sample(0.5));
            assert!(matches!(m.close(true), StopResult::Kept { number, .. } if number == n));
        }
        assert_eq!(m.open(), Err(Error::NoRecordingSlots));
        assert_eq!(m.status().remaining_samples, 98);
        // Both: the time refusal comes first.
        let mut m = Memory::with_capacity_samples(1, 1).unwrap();
        m.open().unwrap();
        assert!(!m.append_sample(0.5));
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
        assert!(m.append_sample(1.0));
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
        let mut m = Memory::new(
            &MemorySettings {
                capacity_s: 10,
                max_recordings: 3,
            },
            Encoding::Samples,
        )
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
            assert!(m.append_sample(0.1));
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
        let mut m = Memory::new(
            &MemorySettings {
                capacity_s: 1,
                max_recordings: 2,
            },
            Encoding::Samples,
        )
        .unwrap();
        m.open().unwrap();
        for _ in 0..6000 {
            m.append_sample(0.25);
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
        let e = Encoding::Samples;
        assert!(m.same_capacity(&same, e) && m.fits(&same, e));
        assert!(!m.same_capacity(&larger, e) && m.fits(&larger, e));
        assert!(!m.fits(&fewer, e));
        // Spec 008 FR-012: the other encoding fits only an empty memory.
        assert!(!m.fits(&larger, Encoding::Frames));
        assert!(!m.same_capacity(&same, Encoding::Frames));
        let c = m.copied_to(&larger, e).unwrap();
        assert_eq!(c.capacities(), (12_000, 3));
        assert_eq!(c.count(), 1);
        assert_eq!(c.samples(c.span(1).unwrap()), m.samples(m.span(1).unwrap()));
    }

    /// Spec 008 FR-010, FR-012, FR-013, SC-007 (T012): the frames encoding stores 10-byte units,
    /// counts capacity in whole frames (180 000 at the default, 1 800 000 bytes), refuses a sample
    /// import, and erases and renumbers in frames.
    #[test]
    fn frames_encoding_units_capacity_and_footprint() {
        let d = Memory::new(&MemorySettings::DEVICE, Encoding::Frames).unwrap();
        assert_eq!(d.capacities(), (180_000, 100)); // 3600 s × 50 frames/s
        assert_eq!(d.footprint_bytes(), 1_800_000 + 100 * 16); // SC-007: ≤ 2.5 MB
        assert!(d.footprint_bytes() <= 2_500_000);
        assert_eq!(d.status().capacity_samples, 21_600_000); // units × 120
        assert_eq!(d.encoding(), Encoding::Frames);
        let mut m = Memory::with_units(3, 2, Encoding::Frames).unwrap();
        m.open().unwrap();
        let f = |b: u8| [b; 10];
        assert!(m.append_frame(&f(1)));
        assert!(m.append_frame(&f(2)));
        assert!(
            !m.append_frame(&f(3)),
            "false exactly at the capacity frame"
        );
        assert!(!m.append_frame(&f(4)), "a full store takes nothing more");
        assert_eq!(
            m.close(true),
            StopResult::Kept {
                number: 1,
                length: 360
            }
        );
        let s = m.status();
        assert_eq!((s.used_samples, s.remaining_samples), (360, 0));
        assert_eq!(m.frames(m.span(1).unwrap()).len(), 30);
        assert_eq!(&m.frames(m.span(1).unwrap())[10..20], &[2u8; 10]);
        assert_eq!(m.open(), Err(Error::MemoryFull));
        assert_eq!(m.erase(1), Some(Span { start: 0, len: 3 }));
        assert_eq!(m.status().remaining_samples, 360);
        // Import of frames; a samples import is the other encoding.
        assert_eq!(m.import_frames(&[7u8; 20]), Ok(1));
        assert_eq!(m.frames(m.span(1).unwrap()), &[7u8; 20]);
        assert_eq!(m.import_frames(&[7u8; 20]), Err(Error::DoesNotFit));
        assert_eq!(
            m.import_samples([0.5f32].into_iter()),
            Err(Error::EncodingDiffers)
        );
        let mut s = Memory::with_units(10, 2, Encoding::Samples).unwrap();
        assert_eq!(s.import_frames(&[7u8; 10]), Err(Error::EncodingDiffers));
        // Clone keeps the frame store's capacity; copied_to keeps the encoding.
        let c = m.clone();
        assert_eq!(c.capacities(), m.capacities());
        assert_eq!(c.frames(c.span(1).unwrap()), m.frames(m.span(1).unwrap()));
        let bigger = MemorySettings {
            capacity_s: 1,
            max_recordings: 5,
        };
        assert!(m.fits(&bigger, Encoding::Frames));
        let moved = m.copied_to(&bigger, Encoding::Frames).unwrap();
        assert_eq!(moved.capacities(), (50, 6));
        assert_eq!(moved.frames(moved.span(1).unwrap()), &[7u8; 20]);
        // An empty memory may change encoding.
        let empty = Memory::with_units(10, 2, Encoding::Samples).unwrap();
        assert!(empty.fits(&bigger, Encoding::Frames));
        let flipped = empty.copied_to(&bigger, Encoding::Frames).unwrap();
        assert_eq!(flipped.encoding(), Encoding::Frames);
        assert_eq!(flipped.capacities(), (50, 6));
    }

    /// Spec 008 FR-013: the capacity in units is rounded down to whole units.
    #[test]
    fn capacity_rounds_down_to_whole_units() {
        assert_eq!(capacity_units_for(3600, 120), 180_000);
        assert_eq!(capacity_units_for(1, 120), 50);
        assert_eq!(capacity_units_for(3600, 1), 21_600_000);
        // A hypothetical 7-sample unit: 6000 / 7 = 857.14 → 857 per second.
        assert_eq!(capacity_units_for(1, 7), 857);
        assert_eq!(capacity_units_for(3, 7), 2571);
        // Encoding figures (spec 008 FR-003, contracts/recording-form.md).
        assert_eq!(
            (
                Encoding::Frames.unit_bytes(),
                Encoding::Frames.unit_samples(),
                Encoding::Frames.form_code()
            ),
            (10, 120, 1)
        );
        assert_eq!(
            (
                Encoding::Samples.unit_bytes(),
                Encoding::Samples.unit_samples(),
                Encoding::Samples.form_code()
            ),
            (4, 1, 2)
        );
        let on = crate::Settings::new(6000);
        let mut off = on;
        off.speech_codec_enabled = false;
        assert_eq!(Encoding::for_settings(&on), Encoding::Frames);
        assert_eq!(Encoding::for_settings(&off), Encoding::Samples);
    }
}
