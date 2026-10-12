//! The measured effect of the one `f32` narrowing of the device-rate stream after the VAS
//! (spec 006 FR-009; research R-02, R-09; contracts/golden-format.md; plan reviews P1, P2;
//! tasks T006, T007).
//!
//! Every entry of the three golden sets at the seven rates is run through the un-narrowed
//! one-pass (the test-only `__with_device_narrowing(settings, false)` hook, the 0.5 engine)
//! and the narrowed one, and the report quotes, per file, the entries and the share of samples
//! that differ and the maximum |Δ| in **full-scale LSBs**: one LSB is 2⁻²³ × max(1, peak) of
//! the un-narrowed output, an absolute unit (a per-sample ulp is unbounded near a zero
//! crossing). The figures are the ones the CHANGELOG's 0.6.0 entry records.
//!
//! Two assertions have teeth. The maximum is bounded by **4 full-scale LSBs** (engineering
//! target, 006 R-02: 0.5 ulp × (‖h₁₀‖₁ + max‖b‖₁) + 0.5 ulp ≈ 2.05 worst case for the clamped
//! device samples, the norms pinned by core unit tests). And **exactly** the entries whose
//! output the narrowing cannot move are unchanged (006 research R-02 as corrected at T009):
//! the entries at the host-rate "after AGC" tap (`agc_only`, which never enters the device
//! chain), and at the 6 kHz identity rate the entries with no arithmetic **before** the point
//! (the limiter and stage 4 bypassed: the clamp and the VAS keep an `f32`-exact value exact;
//! `bypass_all`, `playback_only`, `vas_only`, `vas_mute`) or none **after** it (stage 10 not
//! running and no interpolator, so the output's own `narrow_out` is the same rounding;
//! `record_only`). A narrowing at the wrong point, before the clamp, at the interpolator's
//! output or after stage 10, would move that set.

use std::sync::OnceLock;

use rr_dr60::{Pipeline, Settings, Tap, VasEvent};
use rr_dr60_harness::{configs, golden};

/// Spec 006 R-02 (engineering target): the bound on the narrowing's effect, in full-scale LSBs.
const MAX_DELTA_LSB: f64 = 4.0;

/// One golden entry's measurement.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    file: &'static str,
    stimulus: &'static str,
    config: &'static str,
    rate: u32,
    samples: usize,
    differing: usize,
    /// max |Δ| / (2⁻²³ · max(1, peak |un-narrowed|)).
    max_lsb: f64,
}

impl Row {
    fn changed(&self) -> bool {
        self.differing > 0
    }
}

/// Whether the narrowing can change an entry's output (research R-02; plan review P1; the rule
/// as measured at T009). It cannot when the device chain is never entered (the "after AGC"
/// tap), nor at the 6 kHz identity rate when nothing before the point does arithmetic (no
/// decimator, the limiter and stage 4 bypassed; the clamp of an `f32`-exact value and the VAS's
/// keep/drop/zero are exact), nor at that rate when nothing after it does (stage 10 not running
/// and no interpolator: the output's `narrow_out` then performs the very same rounding, so the
/// stored stream moved but the output did not). An AGC-on entry below the knee at 6 kHz with
/// stages 4 and 10 off would also be exact (the limiter is bit-exact below its knee, spec 005
/// FR-003); none exists, so the predicate requires the AGC off; extend it when one is added.
fn expected_unchanged(s: &Settings) -> bool {
    let identity = s.host_rate_hz == rr_dr60::DEVICE_RATE_HZ;
    let exact_before = !s.agc.enabled && !s.record_stage_enabled;
    let nothing_after = !(s.playback_stage_enabled && s.tap == Tap::AfterPlayback);
    s.tap == Tap::AfterAgc || (identity && (exact_before || nothing_after))
}

fn run(settings: Settings, narrow: bool, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, bool) {
    let mut p = Pipeline::__with_device_narrowing(settings, narrow).unwrap();
    let mut y = vec![0.0f32; x.len()];
    let mut events = vec![VasEvent::default(); p.max_events(x.len())];
    let info = p.process_with_events(x, &mut y, &mut events).unwrap();
    y.truncate(info.produced);
    events.truncate(info.events);
    (y, events, info.paused)
}

fn measure(file: &'static str, stimulus: &'static str, config: &'static str, rate: u32) -> Row {
    let x = golden::stimulus(stimulus, rate);
    let s = configs::settings(config, rate);
    let (wide, ev_wide, paused_wide) = run(s, false, &x);
    let (narrow, ev, paused) = run(s, true, &x);
    // The VAS decides before the narrowing, so lengths, events and the final state agree.
    assert_eq!(
        wide.len(),
        narrow.len(),
        "{file} {stimulus}/{config}/{rate}: length"
    );
    assert_eq!(ev_wide, ev, "{file} {stimulus}/{config}/{rate}: events");
    assert_eq!(
        paused_wide, paused,
        "{file} {stimulus}/{config}/{rate}: paused"
    );
    let peak = wide.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
    let lsb = f64::from(f32::EPSILON) / 2.0 * peak.max(1.0); // 2⁻²³ × max(1, peak)
    let (mut differing, mut max_abs) = (0usize, 0.0f64);
    for (&a, &b) in wide.iter().zip(&narrow) {
        if a.to_bits() != b.to_bits() {
            differing += 1;
            max_abs = max_abs.max((f64::from(a) - f64::from(b)).abs());
        }
    }
    Row {
        file,
        stimulus,
        config,
        rate,
        samples: wide.len(),
        differing,
        max_lsb: max_abs / lsb,
    }
}

/// Every entry of the three golden sets, in file order.
fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    let sets: [(&'static str, &[&'static str], &[&'static str]); 3] = [
        ("golden-v1.json", &golden::STIMULI, &golden::CONFIGS),
        (
            "golden-agc-v1.json",
            &golden::AGC_STIMULI,
            &golden::AGC_CONFIGS,
        ),
        (
            "golden-vas-v1.json",
            &golden::VAS_STIMULI,
            &golden::VAS_CONFIGS,
        ),
    ];
    for (file, stimuli, cfgs) in sets {
        for &stimulus in stimuli {
            for &config in cfgs {
                for rate in configs::all_rates() {
                    rows.push(measure(file, stimulus, config, rate));
                }
            }
        }
    }
    rows
}

/// The report in the form the CHANGELOG quotes: one line per file, then the unchanged entries.
fn report(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str("narrowing delta (006 FR-009; full-scale LSB = 2^-23 x max(1, peak)):\n");
    for file in ["golden-v1.json", "golden-agc-v1.json", "golden-vas-v1.json"] {
        let f: Vec<&Row> = rows.iter().filter(|r| r.file == file).collect();
        let changed = f.iter().filter(|r| r.changed()).count();
        let samples: usize = f.iter().map(|r| r.samples).sum();
        let differing: usize = f.iter().map(|r| r.differing).sum();
        let max = f.iter().map(|r| r.max_lsb).fold(0.0, f64::max);
        out.push_str(&format!(
            "  {file}: {changed} / {} entries changed; {:.1} % of samples differ; max |delta| {max:.2} full-scale LSB\n",
            f.len(),
            100.0 * differing as f64 / samples as f64
        ));
    }
    let unchanged: Vec<String> = rows
        .iter()
        .filter(|r| !r.changed())
        .map(|r| format!("{}/{}/{}", r.stimulus, r.config, r.rate))
        .collect();
    out.push_str(&format!(
        "  unchanged entries ({}): {}\n",
        unchanged.len(),
        unchanged.join(", ")
    ));
    let worst = rows
        .iter()
        .max_by(|a, b| a.max_lsb.total_cmp(&b.max_lsb))
        .expect("rows");
    out.push_str(&format!(
        "  worst entry: {}/{}/{} Hz at {:.2} full-scale LSB (bound {MAX_DELTA_LSB})\n",
        worst.stimulus, worst.config, worst.rate, worst.max_lsb
    ));
    out
}

fn measured() -> &'static (Vec<Row>, String) {
    static ONCE: OnceLock<(Vec<Row>, String)> = OnceLock::new();
    ONCE.get_or_init(|| {
        let rows = rows();
        let text = report(&rows);
        (rows, text)
    })
}

/// 006 T006 (FR-009; research R-02; plan reviews P1, P2): the report, the bound and the
/// unchanged set.
#[test]
fn narrowing_delta_is_bounded_and_moves_exactly_the_expected_entries() {
    let (rows, text) = measured();
    print!("{text}");
    assert_eq!(rows.len(), 217, "112 + 42 + 63 entries"); // 006 contracts/golden-format.md
    let worst = rows.iter().map(|r| r.max_lsb).fold(0.0, f64::max);
    assert!(
        worst <= MAX_DELTA_LSB,
        "max |delta| {worst:.3} full-scale LSB exceeds the bound {MAX_DELTA_LSB} (006 R-02)"
    );
    assert!(worst > 0.0, "the narrowing changed nothing anywhere");
    let mut wrong = Vec::new();
    for r in rows {
        let s = configs::settings(r.config, r.rate);
        let want_unchanged = expected_unchanged(&s);
        if want_unchanged == r.changed() {
            wrong.push(format!(
                "{}/{}/{} Hz: expected {}, got {} differing samples",
                r.stimulus,
                r.config,
                r.rate,
                if want_unchanged {
                    "unchanged"
                } else {
                    "changed"
                },
                r.differing
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the unchanged set is not the expected one:\n{}",
        wrong.join("\n")
    );
    // The expected set, spelled out (39 of 217): the 21 `agc_only` entries, the 6 kHz entries
    // without the limiter and stage 4 (`bypass_all`, `playback_only`, `vas_only`, `vas_mute`:
    // 14) and the 6 kHz entries without stage 10 (`record_only`: 4).
    let unchanged = rows.iter().filter(|r| !r.changed()).count();
    assert_eq!(unchanged, 21 + 4 + 4 + 3 + 3 + 4, "{text}");
    assert!(
        rows.iter()
            .filter(|r| r.config == "agc_only")
            .all(|r| !r.changed())
    );
    assert!(
        rows.iter()
            .filter(|r| r.rate == 6000
                && matches!(
                    r.config,
                    "bypass_all" | "playback_only" | "record_only" | "vas_only" | "vas_mute"
                ))
            .all(|r| !r.changed())
    );
    // And the 6 kHz entries that run both stage 4 and stage 10 all change.
    assert!(
        rows.iter()
            .filter(
                |r| r.rate == 6000 && matches!(r.config, "default" | "default_agc" | "default_vas")
            )
            .all(Row::changed)
    );
}

/// 006 T007: the measurement is deterministic: two runs print the same report.
#[test]
fn narrowing_delta_report_is_stable() {
    let (_, first) = measured();
    let again = report(&rows());
    assert_eq!(*first, again);
}
