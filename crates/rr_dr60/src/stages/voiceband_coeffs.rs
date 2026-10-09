//! PLACEHOLDER — replaced in T029a.
//!
//! Six identity (pass-through) sections, so the analytic voice-band tests (T023) can be seen
//! failing before the real design lands (Constitution III, test-first). T029a overwrites this
//! file by running `uv run tools/filter-design/design_voiceband.py`.

/// Second-order sections of the voice-band stage filter (placeholder: identity).
#[allow(dead_code)] // Used by the voice-band stage once T030 lands.
#[rustfmt::skip]
pub(crate) const VOICEBAND_SOS: [[f64; 5]; 6] = [
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0],
];
