//! The fixed codebook of the speech codec: G.723.1's 17-bit four-pulse algebraic codebook per
//! 60-sample subframe (S-008 §2.16; spec 008 research R-02, R-04; contracts/codec-design.md ›
//! Quantizers and › Searches). Pulse *k* lies on track *k* at position `2k + 8m + g` (m = 0 … 7,
//! the grid bit g shifting every pulse by one); a position at or past 60 is **not placed**, so
//! every code decodes (FR-010). The search is the focused search described in the contract;
//! ties resolve to the lowest code (FR-016). Design constants of the model (A-041), not device
//! figures.

use super::fixed::{mac, ratio_sq_greater};
use super::tables::{PULSE_TRACKS, SUBFRAME_SAMPLES};

/// Pulses per subframe.
pub(crate) const PULSES: usize = 4;
/// Bits of the codebook code.
pub(crate) const CODE_BITS: u32 = 17;
/// Positions per track.
const POSITIONS: u8 = 8;
/// A greedy pick: (C, E, m, the placed position and sign).
type Pick = (i64, i64, u8, Option<(usize, i64)>);

/// A decoded codebook code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pulses {
    /// The position index m of each pulse on its track.
    pub(crate) m: [u8; PULSES],
    /// The sign bit of each pulse: `true` is negative.
    pub(crate) neg: [bool; PULSES],
    /// The grid bit.
    pub(crate) grid: bool,
}

/// The position of pulse `track` at index `m` with `grid`, or `None` when it falls at or past
/// the subframe's end (tracks 2 and 3 at m = 7).
pub(crate) fn position(track: usize, m: u8, grid: bool) -> Option<usize> {
    let p = usize::from(PULSE_TRACKS[track][usize::from(m & 7)]).wrapping_add(usize::from(grid));
    (p < SUBFRAME_SAMPLES).then_some(p)
}

/// The 17-bit code: positions m₀ … m₃ (3 bits each, m₀ most significant), the four sign bits
/// (s₀ most significant) and the grid bit last (contracts/codec-design.md › Bit order).
pub(crate) fn encode(p: &Pulses) -> u32 {
    let mut code = 0u32;
    for &m in &p.m {
        code = code.wrapping_shl(3) | u32::from(m & 7);
    }
    for &neg in &p.neg {
        code = code.wrapping_shl(1) | u32::from(neg);
    }
    code.wrapping_shl(1) | u32::from(p.grid)
}

/// The pulses of a 17-bit code (higher bits ignored).
pub(crate) fn decode(code: u32) -> Pulses {
    let mut p = Pulses {
        grid: code & 1 == 1,
        ..Pulses::default()
    };
    for (k, neg) in p.neg.iter_mut().enumerate().rev() {
        // range-proven: shifts of 1 … 4.
        let bit = code.wrapping_shr(PULSES.wrapping_sub(k) as u32) & 1;
        *neg = bit == 1;
    }
    for (k, m) in p.m.iter_mut().enumerate().rev() {
        // range-proven: shifts of 5, 8, 11, 14.
        let shift =
            5u32.wrapping_add(3u32.wrapping_mul(PULSES.wrapping_sub(1).wrapping_sub(k) as u32));
        *m = (code.wrapping_shr(shift) & 7) as u8;
    }
    p
}

/// The codebook vector of `code`: ±1 at each placed pulse, 0 elsewhere.
pub(crate) fn build_vector(code: u32, out: &mut [i32; SUBFRAME_SAMPLES]) {
    *out = [0; SUBFRAME_SAMPLES];
    let p = decode(code);
    for k in 0..PULSES {
        if let Some(pos) = position(k, p.m[k], p.grid) {
            out[pos] = if p.neg[k] { -1 } else { 1 };
        }
    }
}

/// The backward-filtered target `d[n] = Σ_{i ≥ n} x2[i] h[i − n]` (Q15 × Q12 held in `i64`).
pub(crate) fn backward_filter(
    x2: &[i32; SUBFRAME_SAMPLES],
    h: &[i32; SUBFRAME_SAMPLES],
    d: &mut [i64; SUBFRAME_SAMPLES],
) {
    for (n, dn) in d.iter_mut().enumerate() {
        let mut acc = 0i64;
        for (&x, &hv) in x2[n..].iter().zip(h.iter()) {
            acc = mac(acc, x, hv);
        }
        *dn = acc;
    }
}

/// The correlation matrix `φ(i, j) = Σ_{n ≥ max(i, j)} h[n − i] h[n − j]` of the impulse
/// response (Q12² held in `i64`).
pub(crate) fn correlation_matrix(
    h: &[i32; SUBFRAME_SAMPLES],
    phi: &mut [[i64; SUBFRAME_SAMPLES]; SUBFRAME_SAMPLES],
) {
    for i in 0..SUBFRAME_SAMPLES {
        for j in i..SUBFRAME_SAMPLES {
            // n runs from j to 59: h[n − i] pairs with h[n − j].
            let mut acc = 0i64;
            for (&a, &b) in h[j.wrapping_sub(i)..].iter().zip(h.iter()) {
                acc = mac(acc, a, b);
            }
            phi[i][j] = acc;
            phi[j][i] = acc;
        }
    }
}

/// The focused search (contracts/codec-design.md › Searches): for each grid, eight passes with
/// pulse 0 at each of its positions and pulses 1 – 3 chosen greedily by `C² / E` given the
/// earlier pulses (a pulse's sign is the sign of `d` at its position; a position past the
/// subframe is "not placed" and changes nothing); the best of the 16 candidates by `C² / E`,
/// ties to the lowest code. Returns the code.
pub(crate) fn search(
    d: &[i64; SUBFRAME_SAMPLES],
    phi: &[[i64; SUBFRAME_SAMPLES]; SUBFRAME_SAMPLES],
) -> u32 {
    let mut best: Option<(i64, i64, u32)> = None; // (C, E, code)
    for grid in [false, true] {
        for m0 in 0..POSITIONS {
            let mut p = Pulses {
                grid,
                ..Pulses::default()
            };
            let mut placed: [Option<(usize, i64)>; PULSES] = [None; PULSES]; // (position, sign)
            let (mut c, mut e) = (0i64, 0i64);
            // Pulse 0 always lands inside the subframe (positions 0 … 57).
            let pos0 = position(0, m0, grid).unwrap_or(0);
            let s0: i64 = if d[pos0] >= 0 { 1 } else { -1 };
            p.m[0] = m0;
            p.neg[0] = s0 < 0;
            c = c.saturating_add(d[pos0].saturating_mul(s0));
            e = e.saturating_add(phi[pos0][pos0]);
            placed[0] = Some((pos0, s0));
            for track in 1..PULSES {
                let mut pick: Option<Pick> = None;
                for m in 0..POSITIONS {
                    let (c2, e2, place) = match position(track, m, grid) {
                        None => (c, e, None),
                        Some(pos) => {
                            let s: i64 = if d[pos] >= 0 { 1 } else { -1 };
                            let mut cross = 0i64;
                            for (pj, sj) in placed.iter().flatten() {
                                cross = cross.saturating_add(phi[*pj][pos].saturating_mul(*sj));
                            }
                            let e2 = e
                                .saturating_add(phi[pos][pos])
                                .saturating_add(cross.saturating_mul(s).saturating_mul(2));
                            (
                                c.saturating_add(d[pos].saturating_mul(s)),
                                e2,
                                Some((pos, s)),
                            )
                        }
                    };
                    let better = match &pick {
                        None => true,
                        Some((bc, be, _, _)) => ratio_sq_greater(c2, e2, *bc, *be),
                    };
                    if better {
                        pick = Some((c2, e2, m, place));
                    }
                }
                let (c2, e2, m, place) = pick.unwrap_or((c, e, 0, None));
                c = c2;
                e = e2;
                p.m[track] = m;
                p.neg[track] = matches!(place, Some((_, s)) if s < 0);
                placed[track] = place;
            }
            let code = encode(&p);
            let better = match best {
                None => true,
                Some((bc, be, bcode)) => {
                    ratio_sq_greater(c, e, bc, be)
                        || (!ratio_sq_greater(bc, be, c, e) && code < bcode)
                }
            };
            if better {
                best = Some((c, e, code));
            }
        }
    }
    best.map_or(0, |(_, _, code)| code)
}

/// 008 T013 (spec 008 FR-010, FR-016; S-008 §2.16): every code decodes to at most four pulses
/// inside the subframe, positions and signs round-trip, tracks 2 and 3 at m = 7 are not placed,
/// and the search places a pulse where the target says.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn every_code_decodes_to_at_most_four_pulses_inside_the_subframe() {
        let mut v = [0i32; 60];
        let mut not_placed = 0u32;
        for code in 0..(1u32 << CODE_BITS) {
            build_vector(code, &mut v);
            let pulses = v.iter().filter(|&&x| x != 0).count();
            assert!(pulses <= 4, "code {code:#x}: {pulses} pulses");
            assert!(v.iter().all(|&x| x.abs() <= 1));
            let p = decode(code);
            for k in 0..4 {
                match position(k, p.m[k], p.grid) {
                    Some(pos) => {
                        assert!(pos < 60);
                        assert_eq!(pos % 8, 2 * k + usize::from(p.grid), "track {k}");
                        assert_eq!(v[pos], if p.neg[k] { -1 } else { 1 });
                    }
                    None => {
                        not_placed += 1;
                        assert!(k >= 2 && p.m[k] == 7, "only tracks 2, 3 at m = 7: {p:?}");
                    }
                }
            }
            if pulses < 4 {
                assert!(p.m[2] == 7 || p.m[3] == 7);
            }
            // Round trip: the decoder keeps every bit, a not-placed pulse's sign included.
            assert_eq!(encode(&p), code, "round trip of {code:#x}");
        }
        assert_eq!(not_placed, 2 * (1 << 14), "two tracks × the other 14 bits");
        assert_eq!(position(2, 7, false), None);
        assert_eq!(position(3, 7, false), None);
        assert_eq!(position(2, 7, true), None);
        assert_eq!(position(3, 7, true), None);
        assert_eq!(position(0, 7, true), Some(57));
        assert_eq!(position(1, 7, true), Some(59));
        assert_eq!(position(2, 6, true), Some(53));
    }

    /// A target with one strong positive peak at position 10 (track 1, m = 1, grid 0) and the
    /// identity impulse response: pulse 1 lands there positive; a negative peak at 19 (track 1,
    /// m = 2, grid 1) sets the sign bit and the grid.
    #[test]
    fn search_places_a_pulse_at_the_targets_peak() {
        let mut h = [0i32; 60];
        h[0] = 4096;
        let mut phi = [[0i64; 60]; 60];
        correlation_matrix(&h, &mut phi);
        assert_eq!(phi[5][5], 4096 * 4096);
        assert_eq!(phi[5][6], 0);
        let mut x2 = [0i32; 60];
        x2[10] = 20000;
        let mut d = [0i64; 60];
        backward_filter(&x2, &h, &mut d);
        assert_eq!(d[10], 20000 * 4096);
        assert_eq!(d[9], 0);
        let code = search(&d, &phi);
        let p = decode(code);
        assert!(!p.grid);
        assert_eq!(position(1, p.m[1], p.grid), Some(10));
        assert!(!p.neg[1]);
        let mut v = [0i32; 60];
        build_vector(code, &mut v);
        assert_eq!(v[10], 1);
        // Pulses whose d is zero everywhere add energy without correlation: tracks 2 and 3
        // prefer "not placed" (m = 7), track 0 must land somewhere (m = 0, positive).
        assert_eq!((p.m[0], p.neg[0]), (0, false));
        assert_eq!((p.m[2], p.m[3]), (7, 7));
        x2 = [0; 60];
        x2[19] = -20000;
        backward_filter(&x2, &h, &mut d);
        let p = decode(search(&d, &phi));
        assert!(p.grid);
        assert_eq!(position(1, p.m[1], p.grid), Some(19));
        assert!(p.neg[1]);
        // With a non-trivial impulse response the search is still deterministic and legal.
        for (n, hv) in h.iter_mut().enumerate() {
            *hv = 4096 - (n as i32) * 60;
        }
        correlation_matrix(&h, &mut phi);
        x2 = [0; 60];
        for (n, x) in x2.iter_mut().enumerate() {
            *x = ((n as i32 * 7919) % 20001) - 10000;
        }
        backward_filter(&x2, &h, &mut d);
        let code = search(&d, &phi);
        assert!(code < 1 << CODE_BITS);
        assert_eq!(search(&d, &phi), code, "deterministic");
    }

    /// A target of zeros ties every candidate (C = 0, so every criterion is 0): the lowest
    /// code wins at every step, m = 0 and positive on all four tracks, grid 0, code 0.
    #[test]
    fn all_zero_target_gives_the_lowest_code() {
        let mut h = [0i32; 60];
        h[0] = 4096;
        let mut phi = [[0i64; 60]; 60];
        correlation_matrix(&h, &mut phi);
        let code = search(&[0i64; 60], &phi);
        assert_eq!(code, 0);
        assert_eq!(decode(code), Pulses::default());
    }
}
