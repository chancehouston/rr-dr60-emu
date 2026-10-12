//! Settings validation (spec 002 FR-011 as restated by spec 005 FR-010, spec 003 FR-012;
//! data-model.md › Settings).

use crate::error::{Error, MEMORY_CAPACITY_S_RANGE, MEMORY_MAX_RECORDINGS_RANGE, Setting};
use crate::rate::RatePlan;
use crate::settings::Settings;

/// Checks `settings`: the host rate first (FR-002), then every AGC field (knee, attack,
/// release), then every VAS field, then the memory figures (spec 006 FR-010), each in struct
/// order. Ranges are inclusive ([`Setting::range`]); NaN and ±Inf fail. Fields are checked
/// even when their stage is bypassed, so a bad value is always reported. The `bool` settings
/// need no validation.
pub(crate) fn validate(settings: &Settings) -> Result<(), Error> {
    RatePlan::for_host(settings.host_rate_hz)?;
    let a = &settings.agc;
    let v = &settings.vas;
    for (setting, value) in [
        (Setting::AgcKneeDbfs, a.knee_dbfs), // A-028; 005 R-05: −40 … +3 dBFS
        (Setting::AgcAttackMs, a.attack_ms), // A-034; 005 R-05: 1 … 50 ms
        (Setting::AgcReleaseMs, a.release_ms), // A-035: 200 … 10 000 ms
        (Setting::VasSensitivity, f32::from(v.sensitivity)),
        (Setting::VasThresholdDbfs, v.threshold_dbfs),
        (Setting::VasHangMs, v.hang_ms),
        (Setting::VasOnsetMs, v.onset_ms),
    ] {
        let (lo, hi) = setting.range();
        // NaN fails both comparisons, so it is rejected; ±Inf is outside every range.
        if !(value >= lo && value <= hi) {
            return Err(Error::InvalidSetting { setting });
        }
    }
    // Spec 006 FR-010: integer ranges, checked as integers (a 0 is "capacity too small").
    let m = &settings.memory;
    for (setting, value, (lo, hi)) in [
        (
            Setting::MemoryCapacityS,
            m.capacity_s,
            MEMORY_CAPACITY_S_RANGE,
        ),
        (
            Setting::MemoryMaxRecordings,
            m.max_recordings,
            MEMORY_MAX_RECORDINGS_RANGE,
        ),
    ] {
        if !(lo..=hi).contains(&value) {
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
    use crate::settings::{AgcSettings, Settings, VasMode, VasSettings};
    use std::string::ToString;

    fn with_agc(f: impl Fn(&mut AgcSettings)) -> Settings {
        let mut s = Settings::new(48_000);
        f(&mut s.agc);
        s
    }

    /// (field, inclusive min, inclusive max, setter) for every AGC field, with the ranges quoted
    /// from spec 005 data-model.md › Settings (engineering targets, 005 FR-010; research R-05):
    /// knee −40 … +3 dBFS, attack 1 … 50 ms, release 200 … 10 000 ms. 005 T004.
    #[allow(clippy::type_complexity)]
    fn fields() -> [(Setting, f32, f32, fn(&mut AgcSettings, f32)); 3] {
        [
            (Setting::AgcKneeDbfs, -40.0, 3.0, |a, v| a.knee_dbfs = v),
            (Setting::AgcAttackMs, 1.0, 50.0, |a, v| a.attack_ms = v),
            (Setting::AgcReleaseMs, 200.0, 10_000.0, |a, v| {
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
        // Spec 002 FR-012 (kept by 005): any combination of valid values.
        let s = with_agc(|a| {
            a.attack_ms = 50.0;
            a.release_ms = 200.0;
        });
        assert_eq!(validate(&s), Ok(()));
    }

    /// 005 T004 (FR-010): the knee range is checked at its edges in both directions, and the
    /// order among the AGC fields is knee, attack, release.
    #[test]
    fn knee_range_and_agc_field_order() {
        assert_eq!(validate(&with_agc(|a| a.knee_dbfs = -40.0)), Ok(()));
        assert_eq!(validate(&with_agc(|a| a.knee_dbfs = 3.0)), Ok(()));
        for bad in [-40.001, 3.001, 4.0, f32::NAN] {
            assert_eq!(
                validate(&with_agc(|a| a.knee_dbfs = bad)),
                Err(Error::InvalidSetting {
                    setting: Setting::AgcKneeDbfs
                }),
                "knee {bad}"
            );
        }
        let s = with_agc(|a| {
            a.knee_dbfs = 10.0;
            a.attack_ms = 0.0;
            a.release_ms = 0.0;
        });
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::AgcKneeDbfs
            })
        );
        let s = with_agc(|a| {
            a.attack_ms = 0.0;
            a.release_ms = 0.0;
        });
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::AgcAttackMs
            })
        );
    }

    fn with_vas(f: impl Fn(&mut VasSettings)) -> Settings {
        let mut s = Settings::new(48_000);
        f(&mut s.vas);
        s
    }

    /// (field, inclusive min, inclusive max, setter) for every VAS float field, with the ranges
    /// quoted from 003 data-model.md: `threshold_dbfs` "−60.0 … 0.0", `hang_ms`
    /// "50.0 … 10000.0", `onset_ms` "0.0 … 200.0" (engineering targets, 003 FR-017).
    #[allow(clippy::type_complexity)]
    fn vas_fields() -> [(Setting, f32, f32, fn(&mut VasSettings, f32)); 3] {
        [
            (Setting::VasThresholdDbfs, -60.0, 0.0, |v, x| {
                v.threshold_dbfs = x
            }),
            (Setting::VasHangMs, 50.0, 10_000.0, |v, x| v.hang_ms = x),
            (Setting::VasOnsetMs, 0.0, 200.0, |v, x| v.onset_ms = x),
        ]
    }

    #[test]
    fn vas_bounds_are_inclusive_and_outside_values_are_rejected() {
        for (setting, lo, hi, set) in vas_fields() {
            for ok in [lo, hi] {
                assert_eq!(
                    validate(&with_vas(|v| set(v, ok))),
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
                    validate(&with_vas(|v| set(v, bad))),
                    Err(Error::InvalidSetting { setting }),
                    "{setting:?} {bad}"
                );
            }
        }
        // `sensitivity` "1 … 5" (S-001, A-021).
        for ok in 1..=5 {
            assert_eq!(validate(&with_vas(|v| v.sensitivity = ok)), Ok(()));
        }
        for bad in [0, 6, u8::MAX] {
            assert_eq!(
                validate(&with_vas(|v| v.sensitivity = bad)),
                Err(Error::InvalidSetting {
                    setting: Setting::VasSensitivity
                }),
                "sensitivity {bad}"
            );
        }
    }

    #[test]
    fn vas_validated_when_bypassed_and_in_mute_mode() {
        for enabled in [false, true] {
            let s = with_vas(|v| {
                v.enabled = enabled;
                v.mode = VasMode::Mute;
                v.hang_ms = 0.0;
            });
            assert_eq!(
                validate(&s),
                Err(Error::InvalidSetting {
                    setting: Setting::VasHangMs
                })
            );
        }
    }

    #[test]
    fn order_is_rate_then_agc_then_vas_in_struct_order() {
        let mut s = with_vas(|v| {
            v.sensitivity = 0;
            v.threshold_dbfs = 1.0;
            v.hang_ms = 0.0;
            v.onset_ms = -1.0;
        });
        let next = |s: &Settings| match validate(s) {
            Err(Error::InvalidSetting { setting }) => setting,
            other => panic!("{other:?}"),
        };
        assert_eq!(next(&s), Setting::VasSensitivity);
        s.vas.sensitivity = 3;
        assert_eq!(next(&s), Setting::VasThresholdDbfs);
        s.vas.threshold_dbfs = -40.0; // A-022 (revised by spec 005)
        assert_eq!(next(&s), Setting::VasHangMs);
        s.vas.hang_ms = 1000.0;
        assert_eq!(next(&s), Setting::VasOnsetMs);
        s.agc.attack_ms = 0.0;
        assert_eq!(next(&s), Setting::AgcAttackMs);
        s.host_rate_hz = 22_050;
        assert_eq!(
            validate(&s),
            Err(Error::UnsupportedHostRate { requested: 22_050 })
        );
    }

    #[test]
    fn onset_longer_than_hang_is_allowed() {
        // Spec 003 data-model.md: any combination of valid values.
        let s = with_vas(|v| {
            v.onset_ms = 200.0;
            v.hang_ms = 50.0;
        });
        assert_eq!(validate(&s), Ok(()));
    }

    /// 006 T011 (spec 006 FR-010; data-model › Settings): `capacity_s` 1 ..= 86 400 and
    /// `max_recordings` 1 ..= 9 999 (engineering targets, 006 FR-010); 0 and the value past the
    /// top are the spec's "capacity too small" refusal, `InvalidSetting` naming the field.
    #[test]
    fn memory_ranges_are_inclusive_and_named() {
        let with_memory = |cap: u32, max: u32| {
            let mut s = Settings::new(48_000);
            s.memory.capacity_s = cap;
            s.memory.max_recordings = max;
            s
        };
        for ok in [1, 86_400] {
            assert_eq!(validate(&with_memory(ok, 99)), Ok(()), "capacity {ok}");
        }
        for bad in [0, 86_401, u32::MAX] {
            assert_eq!(
                validate(&with_memory(bad, 99)),
                Err(Error::InvalidSetting {
                    setting: Setting::MemoryCapacityS
                }),
                "capacity {bad}"
            );
        }
        for ok in [1, 9_999] {
            assert_eq!(validate(&with_memory(3600, ok)), Ok(()), "max {ok}");
        }
        for bad in [0, 10_000, u32::MAX] {
            assert_eq!(
                validate(&with_memory(3600, bad)),
                Err(Error::InvalidSetting {
                    setting: Setting::MemoryMaxRecordings
                }),
                "max {bad}"
            );
        }
        // Order: host rate → AGC → VAS → memory; capacity before the count.
        let mut s = with_memory(0, 0);
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::MemoryCapacityS
            })
        );
        s.memory.capacity_s = 1;
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::MemoryMaxRecordings
            })
        );
        s.vas.onset_ms = -1.0;
        assert_eq!(
            validate(&s),
            Err(Error::InvalidSetting {
                setting: Setting::VasOnsetMs
            })
        );
        // The names and ranges in `Display`.
        let text = Setting::MemoryCapacityS.to_string();
        assert!(text.contains("memory.capacity_s"), "{text}");
        assert!(text.contains('1') && text.contains("86400"), "{text}");
        let text = Setting::MemoryMaxRecordings.to_string();
        assert!(text.contains("memory.max_recordings"), "{text}");
        assert!(text.contains('1') && text.contains("9999"), "{text}");
    }

    #[test]
    fn vas_setting_display_names_field_and_range() {
        for (setting, name) in [
            (Setting::VasSensitivity, "vas.sensitivity"),
            (Setting::VasThresholdDbfs, "vas.threshold_dbfs"),
            (Setting::VasHangMs, "vas.hang_ms"),
            (Setting::VasOnsetMs, "vas.onset_ms"),
        ] {
            let text = setting.to_string();
            assert!(text.contains(name), "{text}");
        }
        let text = Setting::VasSensitivity.to_string();
        assert!(text.contains('1') && text.contains('5'), "{text}");
        let text = Setting::VasHangMs.to_string();
        assert!(text.contains("50") && text.contains("10000"), "{text}");
    }

    #[test]
    fn setting_display_names_field_and_range() {
        let text = Setting::AgcAttackMs.to_string();
        assert!(text.contains("agc.attack_ms"), "{text}");
        assert!(text.contains('1') && text.contains("50"), "{text}"); // 005 FR-010
        let text = Setting::AgcKneeDbfs.to_string();
        assert!(text.contains("agc.knee_dbfs"), "{text}");
        assert!(text.contains("-40") && text.contains('3'), "{text}"); // 005 FR-010
        let err = Error::InvalidSetting {
            setting: Setting::AgcReleaseMs,
        }
        .to_string();
        assert!(err.contains("agc.release_ms"), "{err}");
        assert!(err.contains("200") && err.contains("10000"), "{err}"); // 005 FR-010
    }
}
