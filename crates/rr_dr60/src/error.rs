//! Error type for configuration and processing (contracts/rust-api.md).

use core::fmt;

use crate::settings::SUPPORTED_HOST_RATES;

/// Errors from configuration and processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The requested host rate is not in [`SUPPORTED_HOST_RATES`] (FR-002).
    UnsupportedHostRate {
        /// The rejected rate, in Hz.
        requested: u32,
    },
    /// `Pipeline::process` was given input and output slices of different lengths (FR-003).
    LengthMismatch {
        /// Input length in samples.
        input: usize,
        /// Output length in samples.
        output: usize,
    },
    /// A setting is outside its valid range, or is NaN or ±Inf (spec 002 FR-011). Returned by
    /// `Pipeline::new` and `Pipeline::reconfigure`; a failed reconfigure changes nothing.
    InvalidSetting {
        /// The offending setting.
        setting: Setting,
    },
}

/// Names a setting in [`Error::InvalidSetting`]. `Display` gives the field name and its valid
/// range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Setting {
    /// `agc.target_dbfs`, valid −30 to 0 dBFS.
    AgcTargetDbfs,
    /// `agc.max_gain_db`, valid 0 to 60 dB.
    AgcMaxGainDb,
    /// `agc.max_attenuation_db`, valid 0 to 40 dB.
    AgcMaxAttenuationDb,
    /// `agc.attack_ms`, valid 1 to 100 ms.
    AgcAttackMs,
    /// `agc.release_ms`, valid 50 to 10000 ms.
    AgcReleaseMs,
}

impl Setting {
    /// Inclusive valid range (data-model.md › AgcSettings; engineering targets, 002 FR-015).
    pub(crate) const fn range(self) -> (f32, f32) {
        match self {
            Setting::AgcTargetDbfs => (-30.0, 0.0),
            Setting::AgcMaxGainDb => (0.0, 60.0),
            Setting::AgcMaxAttenuationDb => (0.0, 40.0),
            Setting::AgcAttackMs => (1.0, 100.0),
            Setting::AgcReleaseMs => (50.0, 10_000.0),
        }
    }

    const fn name_and_unit(self) -> (&'static str, &'static str) {
        match self {
            Setting::AgcTargetDbfs => ("agc.target_dbfs", "dBFS"),
            Setting::AgcMaxGainDb => ("agc.max_gain_db", "dB"),
            Setting::AgcMaxAttenuationDb => ("agc.max_attenuation_db", "dB"),
            Setting::AgcAttackMs => ("agc.attack_ms", "ms"),
            Setting::AgcReleaseMs => ("agc.release_ms", "ms"),
        }
    }
}

impl fmt::Display for Setting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, unit) = self.name_and_unit();
        let (lo, hi) = self.range();
        write!(f, "{name} (valid range {lo} to {hi} {unit})")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::UnsupportedHostRate { requested } => {
                write!(
                    f,
                    "unsupported host sample rate {requested} Hz; supported rates are "
                )?;
                for (i, rate) in SUPPORTED_HOST_RATES.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{rate}")?;
                }
                f.write_str(" Hz")
            }
            Error::LengthMismatch { input, output } => write!(
                f,
                "input and output lengths differ: input has {input} samples, output has {output}"
            ),
            Error::InvalidSetting { setting } => {
                write!(f, "invalid setting: {setting} must be finite and in range")
            }
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::ToString;

    #[test]
    fn unsupported_rate_lists_supported_rates() {
        let text = Error::UnsupportedHostRate { requested: 22_050 }.to_string();
        assert!(text.contains("22050"), "{text}");
        for rate in crate::settings::SUPPORTED_HOST_RATES {
            assert!(text.contains(&rate.to_string()), "{text} is missing {rate}");
        }
    }

    #[test]
    fn length_mismatch_shows_both_lengths() {
        let text = Error::LengthMismatch {
            input: 64,
            output: 32,
        }
        .to_string();
        assert!(text.contains("64") && text.contains("32"), "{text}");
    }

    #[test]
    fn is_core_error() {
        fn takes_error(_: &dyn core::error::Error) {}
        takes_error(&Error::LengthMismatch {
            input: 1,
            output: 2,
        });
    }
}
