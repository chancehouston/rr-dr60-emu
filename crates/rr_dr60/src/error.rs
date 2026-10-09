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
