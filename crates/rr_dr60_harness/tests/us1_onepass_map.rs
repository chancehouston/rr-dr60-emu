//! Spec 028 US1 [TR: D2, D3, D7, FR-002, FR-003, FR-007, SC-001]: the input behind a one-pass
//! output moment, against the labelled gap and the record-then-play twin, at the measurement
//! rates.

use rr_dr60::VasMode;
use rr_dr60_harness::configs;
use rr_dr60_harness::onepass_map_checks::{self as om, Cell, Region};

fn drop_cells() -> impl Iterator<Item = Cell> {
    Cell::all().into_iter().filter(|c| c.mode == VasMode::Drop)
}

/// AS1: every span `[pos, S(e))` plays the hang's end (f_a at least 12 dB above f_b), every
/// output in it maps to an f_a input, and the existing map with `latency_samples()` maps none
/// of it there.
#[test]
fn as1_onepass_map_labelled_gap() {
    for &rate in configs::measurement_rates() {
        for cell in drop_cells().filter(|c| c.codec) {
            let r = om::render(cell, rate);
            assert_eq!(
                r.events.len(),
                om::BURSTS,
                "{rate} Hz {}: three splices",
                cell.name()
            );
            for e in &r.events {
                let (a, s) = (e.output_position, r.heard(e));
                assert!((s as usize) < r.y.len());
                let fa = om::level_db(&r.y, rate, om::F_A, a as usize, s as usize);
                let fb = om::level_db(&r.y, rate, om::F_B, a as usize, s as usize);
                println!(
                    "{rate} {} span {a}..{s}: f_a {fa:.1} f_b {fb:.1} dB",
                    cell.name()
                );
                assert!(
                    fa >= fb + 12.0,
                    "{rate} Hz {}: the span plays the hang's end",
                    cell.name()
                );
                for o in a..s {
                    let c = r.p.one_pass_input_at_output(o, &r.events).centre;
                    assert_eq!(
                        r.gap.label(c),
                        Region::Fa,
                        "{rate} Hz {} o {o}",
                        cell.name()
                    );
                    assert_ne!(
                        r.gap.label(r.existing(o)),
                        Region::Fa,
                        "{rate} Hz o {o}: the teeth"
                    );
                }
            }
        }
    }
}

/// AS2: `S(e) − 1` maps to the last input before the removed run and `S(e)` to the first after
/// it.
#[test]
fn as2_onepass_map_splice_edges() {
    for &rate in configs::measurement_rates() {
        for cell in drop_cells() {
            let r = om::render(cell, rate);
            let mut removed = 0;
            for e in &r.events {
                let i_e = e.output_position + removed - r.l_pre;
                let s = r.p.one_pass_splice_output_position(e);
                assert_eq!(s, r.heard(e));
                let at = |o| r.p.one_pass_input_at_output(o, &r.events).centre;
                assert_eq!(at(s - 1), i_e - 1, "{rate} Hz {}", cell.name());
                assert_eq!(at(s), i_e + e.input_length, "{rate} Hz {}", cell.name());
                removed += e.input_length;
            }
        }
    }
}

/// AS3: the centre agrees with record-then-play through `kept_to_input` within ⌈m/l⌉ host
/// samples on every output the twin does not clamp, and every output after a heard splice maps
/// into the burst that follows it or its f_a region.
#[test]
fn as3_onepass_map_record_then_play() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let t = om::twin(&r);
            let (n, _, wrong, _) = om::lead_counts(&r, &t);
            println!("{rate} {}: {n} outputs, {wrong} off", cell.name());
            assert!(n > 1000);
            assert_eq!(wrong, 0, "{rate} Hz {}", cell.name());
            if cell.mode == VasMode::Drop {
                for (j, e) in r.events.iter().enumerate() {
                    let end = r
                        .events
                        .get(j + 1)
                        .map_or(r.y.len() as u64, |n| n.output_position);
                    for o in r.heard(e)..end.min(r.y.len() as u64 - r.latency) {
                        let c = r.p.one_pass_input_at_output(o, &r.events).centre;
                        let l = r.gap.label(c);
                        assert!(
                            l == Region::Burst(j) || l == Region::Fa,
                            "{rate} Hz {} o {o}: {l:?}",
                            cell.name()
                        );
                    }
                }
            }
        }
    }
}

/// AS4: the existing map differs from the method exactly on `[pos, S(e))` per splice (by the
/// report's `input_length`), the workbench's rule exactly on `[pos + C, S(e))`.
#[test]
fn as4_onepass_map_existing_and_workbench() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let in_span = |o: u64, lo: u64| {
                r.events
                    .iter()
                    .find(|e| e.output_position + lo <= o && o < r.heard(e))
                    .map(|e| e.input_length)
            };
            let drop = cell.mode == VasMode::Drop;
            let (mut d_existing, mut d_draft) = (0u64, 0u64);
            for o in 0..r.y.len() as u64 {
                let m = r.p.one_pass_input_at_output(o, &r.events).centre;
                let want = |lo| if drop { in_span(o, lo) } else { None };
                match want(0) {
                    Some(len) => assert_eq!(r.existing(o), m + len, "{rate} Hz o {o}"),
                    None => assert_eq!(r.existing(o), m, "{rate} Hz {} o {o}", cell.name()),
                }
                match want(r.c) {
                    Some(len) => assert_eq!(r.draft(o), m + len),
                    None if o >= r.c => assert_eq!(r.draft(o), m, "{rate} Hz o {o}"),
                    None => {}
                }
                d_existing += u64::from(r.existing(o) != m);
                d_draft += u64::from(o >= r.c && r.draft(o) != m);
            }
            println!(
                "{rate} {}: the existing map differs on {d_existing}, the workbench rule on {d_draft}",
                cell.name()
            );
        }
    }
}

/// AS5: an output before the latency maps to input 0, clamped.
#[test]
fn as5_onepass_map_clamped() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let a = r.p.one_pass_input_at_output(0, &r.events);
            assert_eq!((a.centre, a.clamped), (0, true));
            if r.latency > 0 {
                let a = r.p.one_pass_input_at_output(r.latency - 1, &r.events);
                assert_eq!((a.centre, a.clamped), (0, true));
            }
        }
    }
}

/// AS6: the window contains the centre, is the decimator's half-length with the coder off,
/// and its ends equal `kept_to_input`'s on the record-then-play twin within one device sample.
#[test]
fn as6_onepass_map_window() {
    for &rate in configs::measurement_rates() {
        let tol = om::device_step(rate) as i64;
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let t = om::twin(&r);
            let (mut n, mut worst) = (0u64, 0i64);
            for o in 0..(r.y.len() as u64).saturating_sub(r.latency) {
                let a = r.p.one_pass_input_at_output(o, &r.events);
                assert!(a.first <= a.centre && a.centre <= a.last);
                let Some(w) = t.at(&r, o) else { continue };
                let d = (a.first as i64 - w.first as i64)
                    .abs()
                    .max((a.last as i64 - w.last as i64).abs());
                worst = worst.max(d);
                n += 1;
                assert!(d <= tol, "{rate} Hz {} o {o}: {a:?} vs {w:?}", cell.name());
            }
            println!(
                "{rate} {}: {n} windows, worst end {worst} host samples",
                cell.name()
            );
        }
    }
}

/// AS7: the centre never decreases, and a bisection for an input inside a removed pause lands
/// on that pause's heard point ("Listen here", D5).
#[test]
fn as7_onepass_map_monotone() {
    for &rate in configs::measurement_rates() {
        for cell in Cell::all() {
            let r = om::render(cell, rate);
            let centre = |o| r.p.one_pass_input_at_output(o, &r.events).centre;
            let mut prev = 0;
            for o in 0..r.y.len() as u64 {
                let c = centre(o);
                assert!(c >= prev, "{rate} Hz {} o {o}", cell.name());
                prev = c;
            }
            if cell.mode != VasMode::Drop {
                continue;
            }
            let mut removed = 0;
            for e in &r.events {
                let i_e = e.output_position + removed - r.l_pre;
                let p = i_e + e.input_length / 2;
                let (mut lo, mut hi) = (0u64, r.y.len() as u64);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    if centre(mid) >= p {
                        hi = mid
                    } else {
                        lo = mid + 1
                    }
                }
                assert_eq!(
                    lo,
                    r.p.one_pass_splice_output_position(e),
                    "{rate} Hz {}",
                    cell.name()
                );
                removed += e.input_length;
            }
        }
    }
}
