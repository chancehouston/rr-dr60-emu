//! Rate conversion boundary between the host rate and the 8 kHz device rate
//! (FR-004, FR-005; research.md R-08).

pub(crate) mod design;
mod down;
mod up;

pub(crate) use down::PolyphaseDown;
pub(crate) use up::PolyphaseUp;

use crate::rate::RatePlan;

/// Prototype cutoff: the middle of the 3600–4000 Hz transition band (FR-005, engineering target).
pub(crate) const CUTOFF_HZ: f64 = 3800.0;
/// Transition width: passband flat to 3600 Hz, stopband from 4000 Hz (FR-005, engineering target).
pub(crate) const TRANSITION_HZ: f64 = 400.0;
/// Kaiser design stopband: 70 dB, a 10 dB margin over FR-005's 60 dB (engineering target).
pub(crate) const STOPBAND_DB: f64 = 70.0;

/// Builds the decimator and interpolator for `plan`, or `None` for the identity plan.
/// Allocates; called only at construction and reconfiguration (FR-015).
pub(crate) fn converters(plan: &RatePlan) -> Option<(PolyphaseDown, PolyphaseUp)> {
    let RatePlan::Convert {
        l,
        m,
        num_taps,
        prototype_rate,
        ..
    } = *plan
    else {
        return None;
    };
    let h = design::kaiser_lowpass(
        num_taps,
        CUTOFF_HZ / prototype_rate,
        design::kaiser_beta(STOPBAND_DB),
    );
    let (down_branches, down_taps) = design::polyphase_split(&h, l as usize);
    let (up_branches, up_taps) = design::polyphase_split(&h, m as usize);
    Some((
        PolyphaseDown::new(down_branches, down_taps, l, m),
        PolyphaseUp::new(up_branches, up_taps, l, m),
    ))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test-only analysis.
mod tests {
    extern crate std;
    use super::*;
    use crate::rate::RatePlan;
    use crate::test_util::{db_power, fit, power, tone};
    use std::vec::Vec;

    /// Down-converts then up-converts `x` with no stages in between.
    fn round_trip(host: u32, x: &[f64]) -> Vec<f64> {
        let plan = RatePlan::for_host(host).unwrap();
        let (mut down, mut up) = converters(&plan).expect("not identity");
        x.iter()
            .map(|&v| {
                if let Some(d) = down.push(v) {
                    up.push_device(d);
                }
                up.next_host()
            })
            .collect()
    }

    fn check_rate(host: u32, l: u32, m: u32) {
        let plan = RatePlan::for_host(host).unwrap();
        let RatePlan::Convert {
            l: pl,
            m: pm,
            num_taps,
            delay_host,
            ..
        } = plan
        else {
            panic!("identity")
        };
        assert_eq!((pl, pm), (l, m), "{host}: L/M");
        assert_eq!(
            (num_taps - 1) % (2 * l as usize),
            0,
            "{host}: delay not a whole number of host samples"
        );
        assert_eq!(delay_host as usize, (num_taps - 1) / (2 * l as usize));

        let fs = f64::from(host);
        let settle = 4 * delay_host as usize + 2000;
        let len = settle + host as usize / 2;
        // FR-005 (engineering target): flat within ±0.1 dB from 50 to 3600 Hz.
        for f in [50.0, 1000.0, 3600.0] {
            let y = round_trip(host, &tone(f, 0.5, fs, len));
            let (amp, residual) = fit(&y[settle..], f, fs);
            let g = 20.0 * (amp / 0.5).log10();
            assert!(g.abs() <= 0.1, "{host} Hz host: gain {g:.4} dB at {f} Hz");
            if f == 1000.0 {
                // Alias and image products at least 60 dB below the stimulus.
                let r = db_power(power(&residual), 0.125);
                assert!(
                    r <= -60.0,
                    "{host}: alias/image products {r:.1} dB at 1 kHz"
                );
            }
        }
        // FR-005: input from 4000 Hz up to the host Nyquist frequency attenuated >= 60 dB.
        for f in [4000.0, 4600.0, 8000.0, 20_000.0] {
            if f >= fs / 2.0 {
                continue;
            }
            let y = round_trip(host, &tone(f, 0.5, fs, len));
            let r = db_power(power(&y[settle..]), 0.125);
            assert!(r <= -60.0, "{host}: {f} Hz passes at {r:.1} dB");
        }
    }

    fn check_prototype(host: u32) {
        // Design level: prototype ripple <= 0.01 dB to 3600 Hz, stopband >= 70 dB from 4000 Hz.
        let plan = RatePlan::for_host(host).unwrap();
        let RatePlan::Convert {
            num_taps,
            prototype_rate,
            ..
        } = plan
        else {
            panic!()
        };
        let h = design::kaiser_lowpass(
            num_taps,
            CUTOFF_HZ / prototype_rate,
            design::kaiser_beta(STOPBAND_DB),
        );
        let dc: f64 = h.iter().sum();
        let response = |f: f64| {
            let w = std::f64::consts::TAU * f / prototype_rate;
            let (mut re, mut im) = (0.0, 0.0);
            for (n, &c) in h.iter().enumerate() {
                let (s, co) = (w * n as f64).sin_cos();
                re += c * co;
                im -= c * s;
            }
            20.0 * ((re * re + im * im).sqrt() / dc).log10()
        };
        for f in (0..=36).map(|k| f64::from(k) * 100.0) {
            assert!(
                response(f).abs() <= 0.01,
                "{host}: passband {f} Hz {:.4} dB",
                response(f)
            );
        }
        let top = (4.0 * f64::from(host)).min(prototype_rate / 2.0);
        let mut f = 4000.0;
        while f <= top {
            assert!(
                response(f) <= -70.0,
                "{host}: stopband {f} Hz {:.1} dB",
                response(f)
            );
            f += if f < 6000.0 { 25.0 } else { 997.0 };
        }
    }

    #[test]
    fn rate_48k() {
        check_rate(48_000, 1, 6);
        check_prototype(48_000);
    }

    #[test]
    fn rate_44k1() {
        check_rate(44_100, 80, 441);
        check_prototype(44_100);
    }

    #[test]
    fn rate_16k() {
        check_rate(16_000, 1, 2);
        check_prototype(16_000);
    }

    #[test]
    fn rate_88k2() {
        check_rate(88_200, 40, 441);
        check_prototype(88_200);
    }

    #[test]
    fn rate_96k() {
        check_rate(96_000, 1, 12);
        check_prototype(96_000);
    }

    #[test]
    fn rate_8k_is_identity() {
        let plan = RatePlan::for_host(8000).unwrap();
        assert_eq!(plan, RatePlan::Identity);
        assert_eq!(plan.delay_host_samples(), 0);
        assert!(converters(&plan).is_none());
    }

    #[test]
    fn unsupported_rate_rejected() {
        assert_eq!(
            RatePlan::for_host(22_050),
            Err(crate::Error::UnsupportedHostRate { requested: 22_050 })
        );
    }

    #[test]
    fn reset_restores_fresh_state() {
        let plan = RatePlan::for_host(44_100).unwrap();
        let (mut d1, mut u1) = converters(&plan).unwrap();
        let x = tone(1000.0, 0.5, 44_100.0, 3000);
        let run = |d: &mut PolyphaseDown, u: &mut PolyphaseUp| -> Vec<f64> {
            x.iter()
                .map(|&v| {
                    if let Some(s) = d.push(v) {
                        u.push_device(s);
                    }
                    u.next_host()
                })
                .collect()
        };
        let a = run(&mut d1, &mut u1);
        d1.reset();
        u1.reset();
        assert_eq!(a, run(&mut d1, &mut u1));
    }
}
