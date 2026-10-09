//! Settings validation (spec 002 FR-011; data-model.md › AgcSettings).

use crate::error::{Error, Setting};
use crate::rate::RatePlan;
use crate::settings::Settings;

/// Checks `settings`: the host rate first (FR-002), then every AGC field in struct order.
/// Ranges are inclusive; NaN and ±Inf fail. AGC fields are checked even when the AGC is
/// bypassed, so a bad value is always reported.
pub(crate) fn validate(settings: &Settings) -> Result<(), Error> {
    RatePlan::for_host(settings.host_rate_hz)?;
    let a = &settings.agc;
    for (setting, value) in [
        (Setting::AgcTargetDbfs, a.target_dbfs),
        (Setting::AgcMaxGainDb, a.max_gain_db),
        (Setting::AgcMaxAttenuationDb, a.max_attenuation_db),
        (Setting::AgcAttackMs, a.attack_ms),
        (Setting::AgcReleaseMs, a.release_ms),
    ] {
        let (lo, hi) = setting.range();
        // NaN fails both comparisons, so it is rejected; ±Inf is outside every range.
        if !(value >= lo && value <= hi) {
            return Err(Error::InvalidSetting { setting });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::error::{Error, Setting};
    use crate::settings::{AgcSettings, Settings};
    use std::string::ToString;

    fn with_agc(f: impl Fn(&mut AgcSettings)) -> Settings {
        let mut s = Settings::new(48_000);
        f(&mut s.agc);
        s
    }

    /// (field, inclusive min, inclusive max, setter) for every AGC field, with the ranges quoted
    /// from data-model.md (engineering targets, 002 FR-015).
    #[allow(clippy::type_complexity)]
    fn fields() -> [(Setting, f32, f32, fn(&mut AgcSettings, f32)); 5] {
        [
            (Setting::AgcTargetDbfs, -30.0, 0.0, |a, v| a.target_dbfs = v),
            (Setting::AgcMaxGainDb, 0.0, 60.0, |a, v| a.max_gain_db = v),
            (Setting::AgcMaxAttenuationDb, 0.0, 40.0, |a, v| {
                a.max_attenuation_db = v
            }),
            (Setting::AgcAttackMs, 1.0, 100.0, |a, v| a.attack_ms = v),
            (Setting::AgcReleaseMs, 50.0, 10_000.0, |a, v| {
                a.release_ms = v
            }),
        ]
    }

    #[test]
    fn defaults_are_valid() {
        assert_eq!(validate(&Settings::new(48_000)), Ok(()));
    }

    #[test]
    fn bounds_are_inclusive_and_outside_values_are_rejected() {
        for (setting, lo, hi, set) in fields() {
            for ok in [lo, hi] {
                assert_eq!(
                    validate(&with_agc(|a| set(a, ok))),
                    Ok(()),
                    "{setting:?} {ok}"
                );
            }
            let below = f32::from_bits(if lo > 0.0 {
                lo.to_bits() - 1
            } else if lo == 0.0 {
                (-f32::MIN_POSITIVE).to_bits()
            } else {
                lo.to_bits() + 1
            });
            let above = f32::from_bits(if hi >= 0.0 {
                hi.to_bits() + 1
            } else {
                hi.to_bits() - 1
            });
            for bad in [below, above, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    validate(&with_agc(|a| set(a, bad))),
                    Err(Error::InvalidSetting { setting }),
                    "{setting:?} {bad}"
                );
            }
        }
    }

    #[test]
    fn validated_even_when_bypassed() {
        let s = with_agc(|a| {
            a.enabled = false;
            a.attack_ms = 0.0;
        });
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::AgcAttackMs
            })
        );
    }

    #[test]
    fn host_rate_is_reported_first() {
        let mut s = with_agc(|a| a.attack_ms = 0.0);
        s.host_rate_hz = 22_050;
        assert_eq!(
            validate(&s),
            Err(Error::UnsupportedHostRate { requested: 22_050 })
        );
    }

    #[test]
    fn release_shorter_than_attack_is_allowed() {
        // Spec 002 FR-012: any combination of valid values.
        let s = with_agc(|a| {
            a.attack_ms = 100.0;
            a.release_ms = 50.0;
        });
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn setting_display_names_field_and_range() {
        let text = Setting::AgcAttackMs.to_string();
        assert!(text.contains("agc.attack_ms"), "{text}");
        assert!(text.contains('1') && text.contains("100"), "{text}");
        let err = Error::InvalidSetting {
            setting: Setting::AgcReleaseMs,
        }
        .to_string();
        assert!(err.contains("agc.release_ms"), "{err}");
    }
}
