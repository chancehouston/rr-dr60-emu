//! AGC stage: signal-chain stage 3 (spec 002; A-017 – A-020).

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::settings::AgcSettings;
    use rr_dr60_detmath::{TAU, ln, sin};
    use std::vec::Vec;

    const FS: f64 = 8000.0; // A-001: the AGC runs at the device rate (002 R-01)

    fn amp(dbfs: f64) -> f64 {
        rr_dr60_detmath::exp(dbfs * ln(10.0) / 20.0)
    }

    fn to_db(ratio: f64) -> f64 {
        20.0 * ln(ratio) / ln(10.0)
    }

    /// 1 kHz tone sample `n` at `dbfs` (AES17: peak amplitude). At 8 kHz a 1 kHz tone has
    /// exactly 8 samples per cycle, so sample peaks hit the true peak.
    fn tone(dbfs: f64, n: usize) -> f64 {
        amp(dbfs) * sin(TAU * ((n % 8) as f64) / 8.0)
    }

    /// Runs `levels` (dBFS, seconds) through a stage, returning (output, gain dB per sample).
    fn run(stage: &mut AgcStage, levels: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
        let (mut y, mut g) = (Vec::new(), Vec::new());
        let mut n = 0;
        for &(dbfs, secs) in levels {
            for _ in 0..(secs * FS) as usize {
                y.push(stage.process(tone(dbfs, n)));
                g.push(stage.gain_db());
                n += 1;
            }
        }
        (y, g)
    }

    fn peak_dbfs(y: &[f64]) -> f64 {
        to_db(y.iter().fold(0.0f64, |m, v| m.max(v.abs())))
    }

    /// Settling index: last sample outside 2/27 of the excursion (spec 002 Overview).
    fn settle(g: &[f64], final_db: f64, excursion: f64) -> usize {
        let band = excursion.abs() * 2.0 / 27.0;
        g.iter().rposition(|v| (v - final_db).abs() > band).map_or(0, |i| i + 1)
    }

    /// 002 FR-004, FR-005 (A-017): the static curve at default settings. ±1 dB is an
    /// engineering target (002 FR-015).
    #[test]
    fn static_curve_at_defaults() {
        for (input, want) in [(-60.0, -20.0), (-40.0, -13.0), (-10.0, -10.0), (0.0, -9.0), (5.0, -8.5)] {
            let mut stage = AgcStage::new(&AgcSettings::DEVICE);
            let (y, _) = run(&mut stage, &[(input, 1.0)]);
            let got = peak_dbfs(&y[y.len() - 2000..]);
            assert!((got - want).abs() <= 1.0, "{input} dBFS in: {got:.3} out, want {want}");
        }
    }

    /// 002 FR-013, A-019: a fresh stage applies exactly the maximum gain to its first sample.
    #[test]
    fn starts_at_maximum_gain() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        assert_eq!(stage.gain_db(), 40.0);
        let x = 0.25;
        let y = stage.process(x);
        assert!((y / x / 100.0 - 1.0).abs() < 1e-9, "first-sample gain {}", y / x);
    }

    /// 002 FR-007 (US1 AS4): digital silence gives exact zeros, and after a loud tone the gain
    /// returns to maximum during silence (no hold, no gate; A-019).
    #[test]
    fn silence_and_gain_recovery() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        let (y, _) = run(&mut stage, &[(f64::NEG_INFINITY, 10.0)]);
        assert!(y.iter().all(|&v| v.to_bits() == 0), "silence must stay exactly 0.0");
        let (_, g) = run(&mut stage, &[(0.0, 1.0), (f64::NEG_INFINITY, 10.0)]);
        assert!(g[8000] < 10.0, "loud tone should pull the gain down: {}", g[8000]);
        assert!((g[g.len() - 1] - 40.0).abs() < 0.1, "after silence: {}", g[g.len() - 1]);
    }

    /// 002 FR-006 (A-018): attack 10 ms ± 2 ms, release 1.0 s ± 0.2 s, and the release shape
    /// check (35–65 % recovered at 25 % of the release time). The prototype measured 10.00 ms,
    /// 1.001 s and 47 % (research.md R-05).
    #[test]
    fn attack_release_and_shape_at_defaults() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        let (_, g) = run(&mut stage, &[(-40.0, 0.5), (-10.0, 0.3), (-40.0, 2.0)]);
        let (s1, s2) = (4000, 4000 + 2400);
        let up = &g[s1..s2];
        let attack = settle(up, up[up.len() - 1], g[s1 - 1] - up[up.len() - 1]);
        let attack_ms = attack as f64 / FS * 1000.0;
        assert!((attack_ms - 10.0).abs() <= 2.0, "attack {attack_ms:.3} ms");

        let down = &g[s2..];
        let (start, fin) = (g[s2 - 1], down[down.len() - 1]);
        let release = settle(down, fin, fin - start);
        let release_s = release as f64 / FS;
        assert!((release_s - 1.0).abs() <= 0.2, "release {release_s:.4} s");
        let frac = (down[release / 4] - start) / (fin - start);
        assert!((0.35..=0.65).contains(&frac), "midpoint fraction {frac:.3}");
    }

    /// 002 R-07: with maximum attenuation 0 and a tone above the target, the target gain is
    /// −0.0 and the gain decays from 40 dB toward 0. It must flush to exactly 0.0 without ever
    /// being subnormal, after which the output equals the input bit for bit.
    #[test]
    fn gain_flushes_to_zero_without_subnormals() {
        let mut settings = AgcSettings::DEVICE;
        settings.max_attenuation_db = 0.0;
        let mut stage = AgcStage::new(&settings);
        let mut flushed_at = None;
        for n in 0..(600.0 * FS) as usize {
            let x = tone(0.0, n);
            let y = stage.process(x);
            let g = stage.gain_db();
            assert!(!g.is_subnormal(), "gain subnormal at sample {n}");
            if let Some(at) = flushed_at {
                assert_eq!(y.to_bits(), x.to_bits(), "sample {n} (flushed at {at})");
            } else if g.to_bits() == 0 {
                flushed_at = Some(n);
            }
        }
        assert!(flushed_at.is_some(), "gain never reached exactly 0.0");
    }

    /// 002 FR-013: reset returns the stage to its freshly created state.
    #[test]
    fn reset_equals_fresh() {
        let mut used = AgcStage::new(&AgcSettings::DEVICE);
        run(&mut used, &[(0.0, 0.5), (-40.0, 0.2)]);
        used.reset();
        let mut fresh = AgcStage::new(&AgcSettings::DEVICE);
        let a = run(&mut used, &[(-20.0, 0.3)]);
        let b = run(&mut fresh, &[(-20.0, 0.3)]);
        assert_eq!(a, b);
    }
}
