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

    /// The next host sample, if the device samples it reads have arrived (spec 003 research
    /// R-05): host sample n reads device samples up to ⌊n·l/m⌋, so it is emitted once more than
    /// ⌊n·l/m⌋ device samples have been written. Call once per host input step.
    ///
    /// When every device sample is written (nothing dropped), the decimator has always produced
    /// at least ⌊n·l/m⌋ + 1 by input step n, so this emits exactly one sample per step and is
    /// bit-identical to calling [`next_host`](Self::next_host) unconditionally (001/002 output
    /// is unchanged). When device samples are dropped, it emits nothing until they catch up,
    /// and never more than one sample per step, so the ring buffer is never overrun.
    #[inline]
    pub(crate) fn try_next_host(&mut self) -> Option<f64> {
        if self.written > self.latest {
            Some(self.next_host())
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    extern crate std;
    use crate::rate::RatePlan;
    use crate::resample::converters;
    use std::vec::Vec;

    /// The five resampled host rates (the 8 kHz plan is the identity).
    const RATES: [u32; 5] = [16_000, 44_100, 48_000, 88_200, 96_000];

    fn input(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| ((i * 7919) % 2003) as f64 / 2003.0 - 0.5)
            .collect()
    }

    fn l_m(rate: u32) -> (u64, u64) {
        match RatePlan::for_host(rate).unwrap() {
            RatePlan::Convert { l, m, .. } => (u64::from(l), u64::from(m)),
            RatePlan::Identity => unreachable!(),
        }
    }

    /// 003 T006 (research R-05): with nothing dropped, the emission rule emits exactly one host
    /// sample per input step, bit-identical to the unconditional `next_host` path of 001/002.
    #[test]
    fn no_drops_is_one_in_one_out_and_bit_identical() {
        let x = input(200_000);
        for rate in RATES {
            let plan = RatePlan::for_host(rate).unwrap();
            let (mut d1, mut u1) = converters(&plan).unwrap();
            let (mut d2, mut u2) = converters(&plan).unwrap();
            for (t, &v) in x.iter().enumerate() {
                if let Some(d) = d1.push(v) {
                    u1.push_device(d);
                }
                let want = u1.next_host();
                if let Some(d) = d2.push(v) {
                    u2.push_device(d);
                }
                let got = u2.try_next_host();
                assert_eq!(
                    got.map(f64::to_bits),
                    Some(want.to_bits()),
                    "{rate} Hz step {t}"
                );
            }
        }
    }

    /// 003 T006 (research R-05, R-06; spec FR-004): with dropped device samples, at most one
    /// sample per step; input − output stays within one device sample (⌈m/l⌉ host samples) of
    /// ⌊dropped·m/l⌋ at every step, and within ±1 (exact when m/l is an integer) once recording
    /// has continued for ⌈m/l⌉ + 1 steps after a pause; and the writer never leads the
    /// newest-read index by more than it does with no drops (ring safety).
    #[test]
    fn drops_stall_the_output_without_overrunning_the_ring() {
        let x = input(200_000);
        for rate in RATES {
            let (l, m) = l_m(rate);
            let device_period = m.div_ceil(l) as i128; // host samples per device sample, rounded up
            let plan = RatePlan::for_host(rate).unwrap();
            // Reference lead with no drops.
            let (mut d0, mut u0) = converters(&plan).unwrap();
            let mut lead_no_drop = 0;
            for &v in &x {
                if let Some(d) = d0.push(v) {
                    u0.push_device(d);
                }
                lead_no_drop = lead_no_drop.max(u0.written - u0.latest);
                u0.next_host();
            }
            // Seeded pauses: from one device sample up to 3000 (a small LCG; test only). Pauses
            // start only during the first 200 000 steps; the run then continues until recording
            // has settled, so the stream ends while recording.
            let (mut down, mut up) = converters(&plan).unwrap();
            let mut seed: u64 = 0x0D60 ^ u64::from(rate);
            let mut rand = move || {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                seed >> 33
            };
            let (mut pause_left, mut dropped, mut out) = (0u64, 0u64, 0u64);
            let (mut lead, mut since_resume, mut step) = (0, i128::MAX / 2, 0usize);
            let exact = m % l == 0;
            loop {
                let v = x.get(step).copied().unwrap_or(0.0);
                if let Some(d) = down.push(v) {
                    if step < x.len() && pause_left == 0 && rand() % 200 == 0 {
                        pause_left = 1 + rand() % 3000;
                    }
                    if pause_left > 0 {
                        pause_left -= 1;
                        dropped += 1;
                        since_resume = if pause_left == 0 { 0 } else { i128::MIN / 2 };
                    } else {
                        up.push_device(d);
                    }
                }
                lead = lead.max(up.written - up.latest);
                if up.try_next_host().is_some() {
                    out += 1;
                }
                step += 1;
                since_resume += 1;
                let removed = (u128::from(dropped) * u128::from(m) / u128::from(l)) as i128;
                let diff = (step as i128 - out as i128) - removed;
                assert!(
                    diff.abs() <= device_period,
                    "{rate} Hz step {step}: off by {diff} (> one device sample)"
                );
                if pause_left == 0 && since_resume > device_period {
                    let tol = if exact { 0 } else { 1 };
                    assert!(
                        diff.abs() <= tol,
                        "{rate} Hz step {step}: off by {diff} after settling"
                    );
                }
                if step >= x.len() && pause_left == 0 && since_resume > 2 * device_period {
                    break;
                }
            }
            assert!(dropped > 0, "{rate} Hz: the test dropped nothing");
            assert!(
                lead <= lead_no_drop,
                "{rate} Hz: lead {lead} > {lead_no_drop}"
            );
        }
    }
}
