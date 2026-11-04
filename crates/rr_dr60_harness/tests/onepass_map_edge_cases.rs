//! Spec 028 Edge Cases [TR: D2, D5, FR-002, FR-005]: the one-pass output map's corners.

use rr_dr60::{Pipeline, VasEvent, VasMode};
use rr_dr60_harness::configs;
use rr_dr60_harness::onepass_map_checks::{self as om, Cell, Region};

/// Pause reports closer than the coder's delay (no render makes them; the arithmetic only):
/// the centre jumps by each report's length at its own heard point and never decreases.
#[test]
fn onepass_map_synthetic_close_events() {
    let p = Pipeline::new(Cell::all()[1].settings(48_000)).unwrap();
    let ev = [VasEvent::new(10_000, 500), VasEvent::new(10_100, 700)];
    let mut prev = 0;
    for o in 9_000..16_000 {
        let c = p.one_pass_input_at_output(o, &ev).centre;
        assert!(c >= prev);
        prev = c;
    }
    for (i, e) in ev.iter().enumerate() {
        let s = p.one_pass_splice_output_position(e);
        let at = |o| p.one_pass_input_at_output(o, &ev[..=i]).centre;
        assert_eq!(at(s), at(s - 1) + 1 + e.input_length);
    }
}

/// A render that ends paused: the hang's last C stays in the coder and no report covers it, so
/// the coder-on render is C outputs shorter than its coder-off twin, and every output after the
/// last heard splice maps into the last burst or its f_a region.
#[test]
fn onepass_map_ends_paused() {
    for &rate in configs::measurement_rates() {
        let on = om::render(Cell::all()[1], rate);
        let off = om::render(Cell::all()[0], rate);
        let produced = |r: &om::Render| r.y.len() as u64;
        // The renders' inputs differ by the drains' length; compare what each kept.
        let kept_on =
            produced(&on) - (on.gap.x.len() as u64 - on.gap.regions.last().unwrap().0 as u64);
        let kept_off =
            produced(&off) - (off.gap.x.len() as u64 - off.gap.regions.last().unwrap().0 as u64);
        println!("{rate} Hz: kept on {kept_on}, off {kept_off}, C {}", on.c);
        assert_eq!(
            kept_off - kept_on,
            on.c,
            "{rate} Hz: the hang's last C stays in the coder"
        );
        let last = on.events.last().unwrap();
        let mut n = 0;
        for o in on.heard(last)..produced(&on) - on.latency {
            let l = on
                .gap
                .label(on.p.one_pass_input_at_output(o, &on.events).centre);
            assert!(
                l == Region::Burst(om::BURSTS - 1) || l == Region::Fa,
                "{rate} Hz o {o}: {l:?}"
            );
            n += 1;
        }
        assert!(n > 0);
    }
}

/// A report heard before the latency (no render makes one: a splice needs the hang, longer
/// than L_pre): the removed input is added before the clamp.
#[test]
fn onepass_map_clamp_order() {
    let p = Pipeline::new(Cell::all()[1].settings(48_000)).unwrap();
    let e = VasEvent::new(10, 1000);
    let s = p.one_pass_splice_output_position(&e);
    let l = u64::from(p.latency_samples());
    assert!(s < l);
    let a = p.one_pass_input_at_output(s, &[e]);
    assert_eq!(
        (a.centre, a.clamped && a.centre == 0),
        (s + 1000 - l, false)
    );
}

/// The reads follow the engine's current settings: after `reconfigure` to the coder off the
/// heard point loses C and the centre equals the existing map with the new latency far from a
/// report.
#[test]
fn onepass_map_reads_current_settings() {
    for &rate in configs::measurement_rates() {
        let on = Cell::all()[1].settings(rate);
        let mut p = Pipeline::new(on).unwrap();
        let e = VasEvent::new(100_000, 5_000);
        let with_c = p.one_pass_splice_output_position(&e);
        let mut off = on;
        off.speech_codec_enabled = false;
        p.reconfigure(off).unwrap();
        let without = p.one_pass_splice_output_position(&e);
        assert_eq!(with_c - without, u64::from(rate) * 180 / 6000, "{rate} Hz");
        let far = 300_000;
        let want = rr_dr60::timeline::onepass_output_to_input(far, p.latency_samples(), &[e], true);
        assert_eq!(p.one_pass_input_at_output(far, &[e]).centre, want.0);
    }
}

/// With the shortest hang (50 ms) splices are far apart and after the latency: the C + L_post
/// outputs before a heard splice hold no earlier splice and never fall before the latency.
#[test]
fn onepass_map_shortest_hang_far_apart() {
    for &rate in configs::measurement_rates() {
        let mut s = Cell::all()[1].settings(rate);
        s.vas.hang_ms = 50.0;
        let r = om::render_with(s);
        assert!(!r.events.is_empty());
        let min_gap = 301 * om::device_step(rate) - om::device_step(rate);
        assert!(r.events[0].output_position >= r.latency, "{rate} Hz");
        for w in r.events.windows(2) {
            let gap = w[1].output_position - w[0].output_position;
            assert!(gap >= min_gap && gap > r.c + r.l_post, "{rate} Hz: {gap}");
        }
    }
}

/// A crafted list (positions and lengths near `u64::MAX`): the reads saturate.
#[test]
fn onepass_map_saturates() {
    for cell in Cell::all() {
        let p = Pipeline::new(cell.settings(48_000)).unwrap();
        let ev = [
            VasEvent::new(u64::MAX - 1, u64::MAX),
            VasEvent::new(u64::MAX, u64::MAX),
        ];
        for o in [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
            let a = p.one_pass_input_at_output(o, &ev);
            assert!(a.first <= a.centre && a.centre <= a.last);
        }
        assert_eq!(p.one_pass_splice_output_position(&ev[1]), u64::MAX);
        let _ = cell.mode == VasMode::Drop;
    }
}
