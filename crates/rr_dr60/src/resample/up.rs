//! Polyphase interpolator: 8 kHz device rate to host rate (research.md R-08, R-09).

use alloc::boxed::Box;
use alloc::vec;

/// Rational polyphase interpolator. Device samples are pushed in as the decimator produces
/// them; exactly one host sample comes out per `next_host` call (R-09).
///
/// Host sample n sits at prototype time n·l and device sample k at k·m. Output n uses device
/// samples up to floor(n·l / m), which the decimator has always produced by then.
#[derive(Clone, Debug)]
pub(crate) struct PolyphaseUp {
    branches: Box<[f64]>,
    taps: usize,
    l: u32,
    m: u32,
    ring: Box<[f64]>,
    mask: usize,
    /// Multiply-adds performed (test-only, FR-015 bounded work; tasks.md T059).
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
    /// Device samples written.
    written: u64,
    /// Phase of the next host sample: (n · l) mod m.
    phase: u32,
    /// Newest device sample index used by the next host sample: floor(n · l / m).
    latest: u64,
}

impl PolyphaseUp {
    /// An interpolator from per-phase `branches` (m branches of `taps` each).
    pub(crate) fn new(branches: Box<[f64]>, taps: usize, l: u32, m: u32) -> Self {
        // +2: the decimator can be one device sample ahead of `latest`.
        let cap = (taps + 2).next_power_of_two();
        Self {
            branches,
            taps,
            l,
            m,
            ring: vec![0.0; cap].into_boxed_slice(),
            mask: cap - 1,
            written: 0,
            phase: 0,
            latest: 0,
            #[cfg(feature = "op-count")]
            ops: 0,
        }
    }

    /// Stores one device sample.
    #[inline]
    pub(crate) fn push_device(&mut self, d: f64) {
        self.ring[self.written as usize & self.mask] = d;
        self.written += 1;
    }

    /// Computes the next host sample.
    #[inline]
    pub(crate) fn next_host(&mut self) -> f64 {
        let start = self.phase as usize * self.taps;
        let branch = &self.branches[start..start + self.taps];
        let mut acc = 0.0;
        #[cfg(feature = "op-count")]
        {
            self.ops += self.taps as u64;
        }
        for (i, &c) in branch.iter().enumerate() {
            acc += c * self.ring[self.latest.wrapping_sub(i as u64) as usize & self.mask];
        }
        let next = self.phase + self.l;
        self.latest += u64::from(next / self.m);
        self.phase = next % self.m;
        acc
    }

    /// Taps per polyphase branch.
    #[cfg(all(test, feature = "op-count"))]
    pub(crate) fn taps(&self) -> usize {
        self.taps
    }

    /// Returns to the freshly created state. No allocation.
    pub(crate) fn reset(&mut self) {
        self.ring.fill(0.0);
        self.written = 0;
        self.phase = 0;
        self.latest = 0;
    }
}
