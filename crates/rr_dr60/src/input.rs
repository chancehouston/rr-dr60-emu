//! The feed point (spec 020 D12): one published conversion from 24-bit integer samples to the
//! engine's input, so two apps re-rendering the same kept microphone audio feed bit-identical
//! samples, and its inverse for the app's capture.

/// One 24-bit full scale: 2²³.
const FULL_SCALE: f32 = 8_388_608.0;

/// Converts 24-bit integer samples (−2²³ … 2²³ − 1 in an `i32`) to the engine's input:
/// x ÷ 2²³, exact for every 24-bit value (the integer is exact in an `f32` below 2²⁴ and the
/// division by a power of two is exact). Writes `min(src.len(), dst.len())` samples.
pub fn samples_from_i24(src: &[i32], dst: &mut [f32]) {
    for (d, &s) in dst.iter_mut().zip(src) {
        *d = (s as f32) * (1.0 / FULL_SCALE);
    }
}

/// The app's quantizer at the feed point: round-to-nearest of x · 2²³, clamped to the
/// 24-bit range; [`samples_from_i24`] of the result is the sample the engine should be fed.
#[must_use]
pub fn i24_from_sample(x: f32) -> i32 {
    // Round half away from zero without `f32::round` (the core is `no_std`): `as i32`
    // truncates toward zero, so offsetting by ±0.5 first rounds.
    let v = (x * FULL_SCALE).clamp(-FULL_SCALE, FULL_SCALE - 1.0);
    if v >= 0.0 { (v + 0.5) as i32 } else { (v - 0.5) as i32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact for every value of a stratified sample (every 97th code plus the edges) in
    /// debug, every value in release.
    #[test]
    fn exact_and_round_trip() {
        let step: i32 = if cfg!(debug_assertions) { 97 } else { 1 };
        let mut dst = [0.0f32; 1];
        let codes = (-8_388_608i32..=8_388_607).step_by(step as usize);
        for v in codes.chain([8_388_607, -1, 0, 1]) {
            samples_from_i24(&[v], &mut dst);
            let want = v as f64 / 8_388_608.0;
            assert_eq!(f64::from(dst[0]), want, "{v}");
            assert_eq!(i24_from_sample(dst[0]), v, "{v}");
        }
        assert_eq!(i24_from_sample(1.0), 8_388_607);
        assert_eq!(i24_from_sample(-1.0), -8_388_608);
        assert_eq!(i24_from_sample(2.0), 8_388_607);
        let mut d = [0.0f32; 2];
        samples_from_i24(&[1, 2, 3], &mut d);
        assert_eq!(d, [1.0 / 8_388_608.0, 2.0 / 8_388_608.0]);
    }
}
