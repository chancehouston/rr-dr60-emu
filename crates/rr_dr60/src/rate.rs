//! Supported host rates and their rational conversion plans (001 research R-08; spec 004
//! research R-02, R-03).
//!
//! For each converting host rate, `6000 / host = L / M` in lowest terms (A-026). Both
//! converters share one prototype filter at the rate `R = host · L = 6000 · M`.

use rr_dr60_detmath::TAU;

use crate::error::Error;
use crate::resample::{STOPBAND_DB, TRANSITION_HZ};

/// Extra attenuation the Kaiser *length* estimate is computed for, so the realized stopband
/// clears `STOPBAND_DB` at every frequency from the transition's end (engineering target, spec
/// 004 research R-03).
const KAISER_LENGTH_MARGIN_DB: f64 = 2.0;

/// How a host rate maps to the 6 kHz device rate (A-026).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RatePlan {
    /// Host rate is 6 kHz, the device rate: no conversion and no delay (spec 004 FR-002).
    Identity,
    /// Rational conversion through a prototype filter at `prototype_rate`.
    Convert {
        /// Host rate in Hz.
        host_hz: u32,
        /// Host sample step in prototype-rate samples (`6000 / host = l / m`).
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
        // 6000 / host in lowest terms (spec 004 research R-02; A-026).
        let (l, m) = match host_hz {
            6000 => return Ok(Self::Identity),
            8000 => (3, 4),
            16_000 => (3, 8),
            44_100 => (20, 147),
            48_000 => (1, 8),
            88_200 => (10, 147),
            96_000 => (1, 16),
            requested => return Err(Error::UnsupportedHostRate { requested }),
        };
        let prototype_rate = f64::from(host_hz) * f64::from(l);
        // Kaiser length estimate for the TRANSITION_HZ band at STOPBAND_DB (spec 004 FR-003,
        // engineering target): N ≥ (A − 8) / (2.285 · Δω) + 1. The formula is approximate (its
        // first stopband lobe can miss A by a few tenths of a dB), so the length is estimated
        // for A + KAISER_LENGTH_MARGIN_DB while the window shape stays at A (004 research R-03,
        // engineering target).
        let dw = TAU * TRANSITION_HZ / prototype_rate;
        let estimate = (STOPBAND_DB + KAISER_LENGTH_MARGIN_DB - 8.0) / (2.285 * dw);
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
