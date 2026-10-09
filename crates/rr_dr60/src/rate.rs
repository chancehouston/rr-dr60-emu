//! Supported host rates and their rational conversion plans (research.md R-08).
//!
//! For each supported host rate, `8000 / host = L / M` in lowest terms. Both converters
//! share one prototype filter at the rate `R = host · L = 8000 · M` (research.md R-08).

use rr_dr60_detmath::TAU;

use crate::error::Error;
use crate::resample::{STOPBAND_DB, TRANSITION_HZ};

/// How a host rate maps to the 8 kHz device rate (A-001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RatePlan {
    /// Host rate is 8 kHz: no conversion and no delay (FR-002).
    Identity,
    /// Rational conversion through a prototype filter at `prototype_rate`.
    Convert {
        /// Host rate in Hz.
        host_hz: u32,
        /// Host sample step in prototype-rate samples (`8000 / host = l / m`).
        l: u32,
        /// Device sample step in prototype-rate samples.
        m: u32,
        /// Prototype filter length (odd). `(num_taps − 1) / 2` is a multiple of `l`.
        num_taps: usize,
        /// Each converter's linear-phase delay, in whole host samples.
        delay_host: u32,
        /// Prototype filter rate `host · l`, in Hz.
        prototype_rate: f64,
    },
}

impl RatePlan {
    /// The plan for `host_hz`, or `Error::UnsupportedHostRate` (FR-002).
    pub(crate) fn for_host(host_hz: u32) -> Result<Self, Error> {
        let (l, m) = match host_hz {
            8000 => return Ok(Self::Identity),
            16_000 => (1, 2),
            44_100 => (80, 441),
            48_000 => (1, 6),
            88_200 => (40, 441),
            96_000 => (1, 12),
            requested => return Err(Error::UnsupportedHostRate { requested }),
        };
        let prototype_rate = f64::from(host_hz) * f64::from(l);
        // Kaiser length estimate for the 400 Hz transition band at STOPBAND_DB (FR-005,
        // engineering target): N ≥ (A − 8) / (2.285 · Δω) + 1.
        let dw = TAU * TRANSITION_HZ / prototype_rate;
        let estimate = (STOPBAND_DB - 8.0) / (2.285 * dw);
        let mut n0 = estimate as usize;
        if (n0 as f64) < estimate {
            n0 += 1;
        }
        n0 += 1;
        // Round up so the delay (N − 1) / 2 is a whole number of host samples (R-08).
        let step = 2 * l as usize;
        let num_taps = (n0 - 1).div_ceil(step) * step + 1;
        let delay_host = ((num_taps - 1) / step) as u32;
        Ok(Self::Convert {
            host_hz,
            l,
            m,
            num_taps,
            delay_host,
            prototype_rate,
        })
    }

    /// Delay of one converter, in host samples (0 for the identity plan).
    pub(crate) fn delay_host_samples(&self) -> u32 {
        match *self {
            Self::Identity => 0,
            Self::Convert { delay_host, .. } => delay_host,
        }
    }
}
