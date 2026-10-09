//! Polyphase decimator: host rate to 8 kHz device rate (research.md R-08, R-09).

use alloc::boxed::Box;
use alloc::vec;

/// Rational polyphase decimator. Each host sample is pushed in; a device sample comes out
/// whenever one is due, which is at most once per host sample. It runs per sample with
/// integer phase bookkeeping, so its output is independent of block boundaries (R-09).
#[derive(Clone, Debug)]
pub(crate) struct PolyphaseDown {
    branches: Box<[f64]>,
    taps: usize,
    l: u32,
    m: u32,
    history: Box<[f64]>,
    mask: usize,
    /// Multiply-adds performed (test-only, FR-015 bounded work; tasks.md T059).
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
    /// Host samples consumed.
    n: u64,
    /// Phase of the next device sample: (k · m) mod l.
    phase: u32,
    /// Host index at which the next device sample is due: floor(k · m / l).
    due: u64,
}

impl PolyphaseDown {
    /// A decimator from per-phase `branches` (l branches of `taps` each).
    pub(crate) fn new(branches: Box<[f64]>, taps: usize, l: u32, m: u32) -> Self {
        let cap = taps.next_power_of_two();
        Self {
            branches,
            taps,
            l,
            m,
            history: vec![0.0; cap].into_boxed_slice(),
            mask: cap - 1,
            n: 0,
            phase: 0,
            due: 0,
            #[cfg(feature = "op-count")]
            ops: 0,
        }
    }

    /// Pushes one host sample. Returns a device sample if one is due.
    #[inline]
    pub(crate) fn push(&mut self, x: f64) -> Option<f64> {
        let n = self.n;
        self.history[n as usize & self.mask] = x;
        self.n += 1;
        if n != self.due {
            return None;
        }
        let start = self.phase as usize * self.taps;
        let branch = &self.branches[start..start + self.taps];
        let mut acc = 0.0;
        #[cfg(feature = "op-count")]
        {
            self.ops += self.taps as u64;
        }
        for (i, &c) in branch.iter().enumerate() {
            acc += c * self.history[n.wrapping_sub(i as u64) as usize & self.mask];
        }
        let next = self.phase + self.m;
        self.due += u64::from(next / self.l);
        self.phase = next % self.l;
        Some(acc)
    }

    /// Taps per polyphase branch.
    #[cfg(all(test, feature = "op-count"))]
    pub(crate) fn taps(&self) -> usize {
        self.taps
    }

    /// Returns to the freshly created state. No allocation.
    pub(crate) fn reset(&mut self) {
        self.history.fill(0.0);
        self.n = 0;
        self.phase = 0;
        self.due = 0;
    }
}
