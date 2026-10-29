//! The output chain's public constants and gain law (signal-chain stages 11 – 13; spec 009
//! FR-003, FR-004; contracts/rust-api.md › Constants).
//!
//! Every figure is a **design constant modeled on the circuit** (S-002 sheet 3.1) **and the
//! datasheet** (S-006), not a measurement of a real unit: A-031 (the volume control, the
//! amplifier's gain, its clip ceiling, the two coupling corners), A-044 (the loaded volume
//! law), A-043 (the reference position), A-042 (the per-route output scale), A-032 (the
//! earphone follower's corners), A-010 (the speaker **template**, low confidence). The
//! generated table behind them is `stages/output_coeffs.rs`
//! (`tools/output-chain/design_output_chain.py`).

use crate::settings::SUPPORTED_HOST_RATES;
use crate::stages::output as chain;
use crate::stages::output_coeffs as c;

/// The volume control's default position (A-043): the **reference position** at which the
/// loaded law puts a full-scale codec output (1.0 V peak, A-030) exactly at the amplifier's
/// clip ceiling, so the default output does not clip on full-scale material. About 0.1335
/// (−17.5 dB of travel). Positions above it clip full-scale material on the speaker route.
/// An engine convention: the device's knob sits wherever the user left it.
pub const VOLUME_REFERENCE: f32 = c::VOLUME_REFERENCE as f32; // A-043 from A-044, A-031; control: S-002, A-045

/// The position at which the loaded law puts full-scale material **6 dB over** the clip ceiling
/// on the speaker route (about 0.365): the clipper fully engaged. Used by the measurement
/// harness's loud configuration (spec 009 research R-12).
pub const VOLUME_6DB_OVER: f32 = c::VOLUME_6DB_OVER as f32; // engineering target (009 R-12)

/// The amplifier's clip ceiling, V peak differential at the 2.8 V battery rail the schematic marks (S-002)
/// (A-031, medium; 1.0 – 1.4 V registered over the cells' state). A sample of 1.0 on the
/// speaker route is this level (A-042).
pub const CLIP_CEILING_V: f32 = c::V_CLIP_V as f32;

/// The amplifier's voltage gain at position 1, dB: `2 · Rf / Ri = 2 · 68 / 10` (A-031; S-002's
/// values, S-006's formula). In the speaker route's own scale the stage's small-signal gain at
/// position 1 is `CLIP_CEILING_V` less: about 21.5 dB.
pub const AMP_GAIN_DB: f32 = 22.67; // design constant (009 R-02): 20·log10(13.6)

/// The volume control's input coupling corner, Hz (0.1 µF into the 50 kΩ control; both routes;
/// A-031).
pub const VOLUME_CORNER_HZ: f32 = c::F_VOL_HZ as f32;

/// The amplifier's input coupling corner, Hz (0.1 µF into the 10 kΩ input resistor; speaker
/// route; A-031). Its shift with the wiper position is not modeled (A-044).
pub const AMP_CORNER_HZ: f32 = c::F_AMP_HZ as f32;

/// The earphone follower's input and output coupling corners, Hz, nominal (A-032, low:
/// 65 – 130 and 240 – 320 Hz registered; both follow the transistor's current gain).
pub const EARPHONE_CORNERS_HZ: [f32; 2] = [c::F_EAR_IN_HZ as f32, c::F_EAR_OUT_HZ as f32];

/// The speaker template's resonance, Hz (A-010, revised; low confidence).
pub const SPEAKER_RESONANCE_HZ: f32 = c::SPK_F0_HZ as f32;

/// The speaker template's resonance Q (A-010).
pub const SPEAKER_Q: f32 = c::SPK_Q as f32;

/// The speaker template's rising shelf, dB (zero 780 Hz, pole 1310 Hz; A-010).
pub const SPEAKER_SHELF_DB: f32 = 4.5; // design constant (009 R-03): 20·log10(1310/780)

/// The speaker route's gain in dB at a volume position under the loaded law (A-044), in volts
/// of amplifier output per volt of codec output: 22.7 dB at 1, about 9.6 dB at 0.5, −∞ at 0.
/// For a host's volume display; not a measurement. Real-time safe (no allocation; the
/// deterministic `ln`).
pub fn speaker_gain_db(position: f32) -> f32 {
    db(chain::g_spk(f64::from(position)))
}

/// The earphone route's gain in dB at a volume position under the loaded law (A-044): 0 dB at
/// 1, about −9.4 dB at 0.5, about −19.2 dB at [`VOLUME_REFERENCE`], −∞ at 0. The follower's
/// drive into its 16 Ω load and the earphone's sensitivity are loudness constants outside the
/// model (A-042). Real-time safe.
pub fn earphone_gain_db(position: f32) -> f32 {
    db(chain::g_ear(f64::from(position)))
}

/// The output chain's settle at a host rate, in host samples: the samples the playback drain
/// grows by when any output stage runs (spec 009 research R-08: the 32 Hz corner's state
/// decaying by 100 dB; about 58 ms). `None` for an unsupported rate.
pub fn settle_samples(host_rate_hz: u32) -> Option<u32> {
    chain::rate_index(host_rate_hz).map(|i| c::N_OUT[i])
}

/// The length of a **volume transition**, in milliseconds (A-045; spec 011 FR-002): a valid
/// volume change moves the knob from where it is to the new position over this time, starting
/// and ending at rest, the gains following the loaded law (A-044) at every sample. The same at
/// every supported host rate and for any size of change. A read-only constant, not a setting
/// (spec 011 FR-011); `RR_DR60_VOLUME_TRANSITION_MS` in C.
pub const VOLUME_TRANSITION_MS: u32 = 20; // A-045: an engine convention (spec 011 FR-002)

// Spec 011 FR-002: 20 ms is a whole number of samples at every supported rate.
const _: () = {
    let mut i = 0;
    while i < SUPPORTED_HOST_RATES.len() {
        assert!(
            SUPPORTED_HOST_RATES[i] % 50 == 0,
            "20 ms is a whole number of samples"
        );
        i += 1;
    }
};

/// The length of a volume transition at a host rate, in host samples (spec 011 FR-002):
/// `host_rate_hz × VOLUME_TRANSITION_MS ÷ 1000`, exact at every supported rate (120 at 6 kHz
/// … 1920 at 96 kHz). `None` for an unsupported rate.
pub const fn volume_transition_samples(host_rate_hz: u32) -> Option<u32> {
    let mut i = 0;
    while i < SUPPORTED_HOST_RATES.len() {
        if SUPPORTED_HOST_RATES[i] == host_rate_hz {
            return Some(host_rate_hz * VOLUME_TRANSITION_MS / 1000);
        }
        i += 1;
    }
    None
}

fn db(ratio: f64) -> f32 {
    if ratio <= 0.0 {
        f32::NEG_INFINITY
    } else {
        (20.0 * rr_dr60_detmath::ln(ratio) / core::f64::consts::LN_10) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 009 T013 (contracts/rust-api.md › Constants): the public figures against the generated
    /// table and the registered figures.
    #[test]
    fn constants_match_the_registers() {
        assert_eq!(VOLUME_REFERENCE, 0.133_451_74); // printed 2026-10-15 (A-043)
        assert_eq!(VOLUME_6DB_OVER, 0.365_134_57);
        assert_eq!(CLIP_CEILING_V, 1.15); // A-031
        assert!((VOLUME_CORNER_HZ - 31.83).abs() < 0.01); // A-031
        assert!((AMP_CORNER_HZ - 159.15).abs() < 0.01); // A-031
        assert_eq!(EARPHONE_CORNERS_HZ, [120.0, 280.0]); // A-032
        assert_eq!((SPEAKER_RESONANCE_HZ, SPEAKER_Q), (750.0, 1.8)); // A-010
        assert!((speaker_gain_db(1.0) - AMP_GAIN_DB).abs() < 0.01);
        assert!((speaker_gain_db(0.5) - 9.61).abs() < 0.01); // A-044
        assert!((earphone_gain_db(1.0)).abs() < 1e-6);
        assert!((earphone_gain_db(0.5) + 9.39).abs() < 0.01);
        assert!((earphone_gain_db(VOLUME_REFERENCE) + 19.21).abs() < 0.01);
        assert_eq!(speaker_gain_db(0.0), f32::NEG_INFINITY);
        assert_eq!(earphone_gain_db(0.0), f32::NEG_INFINITY);
        assert_eq!(settle_samples(48_000), Some(2764));
        assert_eq!(settle_samples(6000), Some(346));
        assert_eq!(settle_samples(7000), None);
        // The shelf's figure from its zero and pole.
        let shelf = 20.0 * rr_dr60_detmath::ln(1310.0 / 780.0) / core::f64::consts::LN_10;
        assert!((shelf - f64::from(SPEAKER_SHELF_DB)).abs() < 0.01);
    }

    /// Spec 011 FR-002: N = 0.02 × the host rate exactly at the seven rates.
    #[test]
    fn volume_transition_samples_per_rate() {
        assert_eq!(VOLUME_TRANSITION_MS, 20); // A-045
        let n: [Option<u32>; 7] = SUPPORTED_HOST_RATES.map(volume_transition_samples);
        assert_eq!(n, [120, 160, 320, 882, 960, 1764, 1920].map(Some));
        assert_eq!(volume_transition_samples(22_050), None);
        assert_eq!(volume_transition_samples(0), None);
        const AT_48K: Option<u32> = volume_transition_samples(48_000); // usable in const context
        assert_eq!(AT_48K, Some(960));
    }
}
