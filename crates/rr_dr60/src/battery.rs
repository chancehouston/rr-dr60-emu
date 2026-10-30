//! The battery state (spec 018; A-056): the static law that scales the speaker amplifier's
//! clip ceiling with the cell pair's voltage, and the public data an app's presets follow.
//!
//! The RR-DR60's speaker amplifier (IC9) runs from the two cells directly, through Q9 (A-013),
//! so its clip ceiling follows the cells while the record path and the earphone route run on
//! regulated rails. The model's ceiling, 1.15 V peak differential at the service manual's
//! 2.8 V mark (A-031), scales in proportion to V+ − 1.6 V (the output pins' swing limits,
//! S-006) with V+ the cell pair's voltage less Q9's 0.2 V drop (A-056): the **ceiling factor**
//! is `b = (volts − 1.8) ÷ 1.2`, exactly 1 at the default of 3.0 V. The dynamic sag under load,
//! any drain over time and supply-noise coupling are not modeled. **Modeled on the circuit and
//! the amplifier's datasheet, not measured** (low to medium confidence). The value describes
//! the played unit's cells and is set by the user: nothing in the engine changes it on its own,
//! and an app never drives it from the host device's battery, a sensor or a timer.

use rr_dr60_detmath::ln;

use crate::stages::output_coeffs as c;

/// The battery value's inclusive range, volts of the cell pair (A-056): from where the
/// amplifier's operating range and the firmware's cut-off end (2.2 V) to the nominal mark
/// (3.0 V), above which the ceiling would exceed the model's 1.15 V (A-042).
pub const BATTERY_VOLTS_RANGE: (f32, f32) = (c::BATTERY_VOLTS_MIN, c::BATTERY_VOLTS_MAX);

/// The default, the nominal mark (V+ 2.8 V, S-002): the ceiling factor is exactly 1, so every
/// output is bit-identical to the model before spec 018. An app's "fresh" is this value, not the
/// trajectory's first row.
pub const DEFAULT_BATTERY_VOLTS: f32 = c::BATTERY_VOLTS_DEFAULT; // S-002, S-006, S-017; A-056

/// A NiMH pair's steady voltage as A-056 describes it (2.4 – 2.5 V), for an app's preset.
pub const NIMH_PAIR_VOLTS: f32 = c::BATTERY_NIMH_PAIR_VOLTS;

/// A-056's alkaline reference trajectory under the device's load (S-017's discharge curves
/// against S-001's recording life): `(hours of recording, volts of the pair)` at 0, 1, 2, 4,
/// 6, 8 and 10 h. Row 0 (2.96 V) is a loaded-cell reading, **not the default**: an app's
/// "fresh" is [`DEFAULT_BATTERY_VOLTS`]; the hours are hours of recording, not of use.
pub const ALKALINE_TRAJECTORY: [(f32, f32); 7] = c::BATTERY_ALKALINE_TRAJECTORY;

/// The clip-ceiling factor of the modeled law (A-056): `(volts − 1.8) ÷ 1.2`, computed in
/// double precision and narrowed, exactly 1.0 at [`DEFAULT_BATTERY_VOLTS`]. Total over finite
/// inputs (the law is a formula; the setter refuses values outside
/// [`BATTERY_VOLTS_RANGE`]). The speaker route's clip ceiling in the engine's output scale is
/// this factor (1.0 = the nominal 1.15 V ceiling, A-042).
pub fn ceiling_factor(volts: f32) -> f32 {
    ((f64::from(volts) - 1.8) / 1.2) as f32
}

/// `20 · log10(ceiling_factor(volts))`: the ceiling's change in dB re the nominal (0 at the
/// default, about −9.5 dB at 2.2 V).
pub fn ceiling_db(volts: f32) -> f32 {
    let b = f64::from(ceiling_factor(volts));
    (20.0 * ln(b) / core::f64::consts::LN_10) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 018 FR-002, FR-006: the factor is exactly 1.0 at the default (the goldens rest on it).
    #[test]
    fn ceiling_factor_is_exactly_one_at_the_default() {
        assert_eq!(
            ceiling_factor(DEFAULT_BATTERY_VOLTS).to_bits(),
            1.0f32.to_bits()
        );
        assert_eq!(DEFAULT_BATTERY_VOLTS, 3.0);
        assert_eq!(BATTERY_VOLTS_RANGE, (2.2, 3.0));
        assert_eq!(ceiling_db(DEFAULT_BATTERY_VOLTS), 0.0);
    }

    /// Spec 018 FR-006: the six law points, the seven trajectory rows and the NiMH figure are
    /// bit-exact to the design script's f32 print (`BATTERY_LAW_PRINT`); `ceiling_db` within
    /// 1e-5 of the printed dB; the register's rounded table within 0.05 dB.
    #[test]
    fn law_points_and_rows_match_the_script() {
        for &(volts, factor) in &c::BATTERY_LAW_PRINT {
            assert_eq!(
                ceiling_factor(volts).to_bits(),
                factor.to_bits(),
                "{volts} V"
            );
        }
        let printed_db = [
            0.0, -1.584, -3.522, -6.021, -9.542, -0.294, -1.938, -2.894, -4.437, -5.736, -7.264,
            -9.119, -5.325,
        ];
        for (&(volts, _), &want) in c::BATTERY_LAW_PRINT.iter().zip(&printed_db) {
            assert!(
                (f64::from(ceiling_db(volts)) - want).abs() < 1e-3 + 1e-5,
                "{volts} V: {} vs {want}",
                ceiling_db(volts)
            );
        }
        let register = [-0.3, -1.9, -2.9, -4.4, -5.7, -7.3, -9.1];
        for (&(_, volts), &want) in ALKALINE_TRAJECTORY.iter().zip(&register) {
            assert!(
                (f64::from(ceiling_db(volts)) - want).abs() <= 0.05,
                "{volts} V: {} vs the register's {want}",
                ceiling_db(volts)
            );
        }
        assert_eq!(ALKALINE_TRAJECTORY[0], (0.0, 2.96));
        assert_eq!(ALKALINE_TRAJECTORY[6], (10.0, 2.22));
        assert!((2.4..=2.5).contains(&NIMH_PAIR_VOLTS));
    }

    /// The law is total over finite inputs and strictly increasing over the range.
    #[test]
    fn law_is_total_and_monotonic() {
        let mut prev = ceiling_factor(2.2);
        let mut v = 2.2f32;
        while v < 3.0 {
            v += 0.001;
            let b = ceiling_factor(v);
            assert!(b >= prev, "{v}");
            prev = b;
        }
        assert!(ceiling_factor(1.8).abs() < 1e-7);
        assert!(ceiling_factor(0.0).is_finite());
        assert!(ceiling_factor(10.0).is_finite());
        assert!((ceiling_factor(2.2) - 1.0 / 3.0).abs() < 1e-6);
    }
}
