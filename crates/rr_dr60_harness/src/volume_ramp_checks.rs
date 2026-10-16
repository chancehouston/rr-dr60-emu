//! Spec 011 volume-transition checks: one function per measurable property (research R-05;
//! tasks.md T012, T020, T021).
//!
//! The figures are measured in the **stage-11-only configurations**
//! ([`configs::STAGE11_CONFIGS`]: stage 11 alone, read at [`rr_dr60::Tap::AfterVolumeAmp`]).
//! There the coupling corners precede the gain and the knee is memoryless, so a run held at a
//! constant position is an exact oracle: the transition run equals the old position's run
//! before the change and the new position's run from transition sample N, and the
//! instantaneous step of version 0.10 is the **splice** of the two runs at the change (research
//! R-05 (a)). The resampler pair runs in every run alike and cancels. Changes are made at block
//! boundaries of the one-pass processing; every call produces its full block here, so an output
//! sample index is a block boundary (asserted).
//!
//! The click energy is [`analysis::click_energy`] (FR-004): the transition's figure against
//! the step's, pinned to `tools/volume-ramp/ramp_model.py`'s print within ±0.25 dB
//! ([`RAMP_MODEL_FR004`]) and at least 30 dB (the spec's floor).

use rr_dr60::output::volume_transition_samples;
use rr_dr60::{Route, SUPPORTED_HOST_RATES, Settings};

use crate::analysis::{self, click_energy};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

/// Trace: the transition (A-045) on the loaded law (A-044).
pub const T_RAMP: &[&str] = &["A-045", "A-044", "engineering target"];

/// FR-004's figures per host rate (`SUPPORTED_HOST_RATES` order) as
/// `[speaker up, speaker down, earphone up, earphone down]`, dB, for a change between 0.1 and
/// 1 (the speaker route at −30 dBFS, the earphone route at 0 dBFS): printed by
/// `uv run tools/volume-ramp/ramp_model.py` on 2026-10-16 (research R-02).
pub const RAMP_MODEL_FR004: [[f64; 4]; 7] = [
    [34.46, 34.46, 41.23, 41.23],
    [34.20, 34.19, 40.97, 40.96],
    [33.94, 33.94, 40.71, 40.71],
    [33.87, 33.86, 40.64, 40.64],
    [33.86, 33.86, 40.64, 40.64],
    [33.86, 33.86, 40.63, 40.63],
    [33.86, 33.86, 40.63, 40.63],
];

/// FR-004's floor, dB (spec 011 Clarifications: the coach's and the hardware reviewer's floors).
pub const FLOOR_DB: f64 = 30.0;
/// The pin of the Rust figure to the script's, dB (research R-02; the spec allows ±1 dB).
pub const PIN_DB: f64 = 0.25;
/// The ratio reader's tolerance in `f32` ULP of the ratio (US1 AS1 (c)).
pub const RATIO_ULP: f64 = 4.0;

/// The level of FR-004's tone on a route: the speaker route below the knee at position 1
/// (11.83 × 0.0316 = 0.37 < 0.85), the earphone route at full scale.
pub fn level_dbfs(route: Route) -> f64 {
    match route {
        Route::Earphone => 0.0,
        _ => -30.0,
    }
}

/// N, the transition's length in host samples (FR-002).
pub fn n_of(rate: u32) -> usize {
    volume_transition_samples(rate).expect("a supported rate") as usize
}

/// The change sample: a multiple of 4096 at least 0.8 s in (so the click window, half a
/// second wide, sits on a tone settled for half a second), a boundary of every partition used.
pub fn change_at(rate: u32) -> usize {
    (rate as usize * 4 / 5).div_ceil(4096) * 4096
}

/// ⌊v⌋ to a multiple of 16 (the golden's and the drag's block size).
fn floor16(v: usize) -> usize {
    v / 16 * 16
}

/// The drag's update interval: 16 · round(rate ÷ 960) host samples (16.0 – 17.0 ms).
pub fn drag_gap(rate: u32) -> usize {
    16 * (f64::from(rate) / 960.0).round() as usize
}

/// The drag's ten positions, 0.1 + 0.09 j.
pub fn drag_positions() -> [f32; 10] {
    core::array::from_fn(|j| (0.1 + 0.09 * (j + 1) as f64) as f32)
}

fn rate_index(rate: u32) -> usize {
    SUPPORTED_HOST_RATES
        .iter()
        .position(|&r| r == rate)
        .expect("a supported rate")
}

/// A block-size source: fixed, or a seeded irregular sequence.
pub enum Blocks {
    /// Every block `n` samples.
    Fixed(usize),
    /// A deterministic irregular partition (sizes 1 … 700 from a seeded LCG).
    Irregular(u64),
}

/// Runs `x` through a fresh engine at `start`, applying `changes` (`(output sample, position)`,
/// sorted) at block boundaries; a block is cut at a change so every change sits on a boundary.
/// Asserts every call produced its full block (so input and output samples coincide).
pub fn run_with_changes(
    make: Make<'_>,
    settings: Settings,
    route: Route,
    start: f32,
    changes: &[(usize, f32)],
    x: &[f32],
    blocks: Blocks,
) -> Vec<f32> {
    let mut p = make(settings);
    p.set_route(route);
    p.set_volume(start).expect("a valid position");
    let mut seed = match blocks {
        Blocks::Irregular(s) => s,
        Blocks::Fixed(_) => 0,
    };
    let mut next = || match blocks {
        Blocks::Fixed(n) => n,
        Blocks::Irregular(_) => {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            1 + (seed >> 33) as usize % 700
        }
    };
    let mut y = Vec::with_capacity(x.len());
    let (mut pos, mut ci) = (0usize, 0usize);
    while pos < x.len() {
        while ci < changes.len() && changes[ci].0 == pos {
            p.set_volume(changes[ci].1).expect("a valid position");
            ci += 1;
        }
        let mut n = next().min(x.len() - pos);
        if ci < changes.len() && pos + n > changes[ci].0 {
            n = changes[ci].0 - pos;
        }
        let mut out = vec![0.0f32; n];
        let produced = p.process(&x[pos..pos + n], &mut out).expect("ok").produced;
        assert_eq!(produced, n, "every call produces its block (011 R-05)");
        y.extend_from_slice(&out);
        pos += n;
    }
    y
}

/// Bit-identity of two slices.
pub fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(u, v)| u.to_bits() == v.to_bits())
}

/// A power ratio in dB (`analysis::db` is the amplitude form; R-04: no platform `log10`).
fn power_db(ratio: f64) -> f64 {
    analysis::db(ratio) / 2.0
}

fn pass(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// A stage-11-only scene: the settings, the route, the tone and the change sample.
struct Scene {
    settings: Settings,
    route: Route,
    x: Vec<f32>,
    c: usize,
    n: usize,
}

fn scene(make: Make<'_>, config: &str, rate: u32, level: f64) -> Scene {
    let settings = configs::settings(config, rate);
    let route = configs::stage11_route(config);
    let c = change_at(rate);
    let len = c + rate as usize;
    let latency = make(settings).latency_samples() as usize;
    Scene {
        settings,
        route,
        x: stimulus::tone_peaking_at(level, rate, len, c, latency),
        c,
        n: n_of(rate),
    }
}

impl Scene {
    fn run(
        &self,
        make: Make<'_>,
        start: f32,
        changes: &[(usize, f32)],
        blocks: Blocks,
    ) -> Vec<f32> {
        run_with_changes(
            make,
            self.settings,
            self.route,
            start,
            changes,
            &self.x,
            blocks,
        )
    }
    fn held(&self, make: Make<'_>, p: f32, blocks: Blocks) -> Vec<f32> {
        self.run(make, p, &[], blocks)
    }
}

fn describe(level: f64, from: f32, to: f32, c: usize) -> String {
    format!("1 kHz {level:.0} dBFS, {from} -> {to} at sample {c}")
}

/// US1 AS1 (a), SC-001 (FR-001, FR-002): the splice identity: bit-identical to the old
/// position's run before the change and to the new position's run from transition sample N;
/// the first sample differing from the old run is sample 1, the last differing from the new
/// run is sample N − 1 (the −1 length mutant's catch).
pub fn splice(
    make: Make<'_>,
    rate: u32,
    config: &'static str,
    from: f32,
    to: f32,
) -> Vec<MeasurementResult> {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(make, config, rate, level);
    let (c, n) = (s.c, s.n);
    let y = s.run(make, from, &[(c, to)], Blocks::Fixed(64));
    let a = s.held(make, from, Blocks::Fixed(64));
    let b = s.held(make, to, Blocks::Fixed(64));
    let first = (0..y.len()).find(|&i| y[i].to_bits() != a[i].to_bits());
    let last = (0..y.len())
        .rev()
        .find(|&i| y[i].to_bits() != b[i].to_bits());
    let k = |i: Option<usize>| i.map_or(f64::NAN, |i| (i as f64) - c as f64 + 1.0);
    // Post-implementation review N2: every transition sample lies between the two runs (exact on
    // the earphone route; on the speaker route the knee is monotonic and memoryless).
    let outside = (c..c + n - 1)
        .filter(|&i| {
            let (lo, hi) = if a[i] <= b[i] {
                (a[i], b[i])
            } else {
                (b[i], a[i])
            };
            !(lo..=hi).contains(&y[i])
        })
        .count();
    let stim = describe(level, from, to, c);
    vec![
        MeasurementResult::new(
            "FR-003",
            "transition samples outside the old and new runs (sandwich)",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            outside as f64,
            "",
            Tolerance::Exact(0.0), // 011 FR-003: engineering target
        ),
        MeasurementResult::new(
            "FR-001",
            "bit-identical to the old position's run before the change",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            pass(same(&y[..c], &a[..c])),
            "",
            Tolerance::Exact(1.0), // 011 FR-001: engineering target
        ),
        MeasurementResult::new(
            "FR-001",
            "bit-identical to the new position's run from transition sample N",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            pass(same(&y[c + n - 1..], &b[c + n - 1..])),
            "",
            Tolerance::Exact(1.0), // 011 FR-001: engineering target
        ),
        MeasurementResult::new(
            "FR-001",
            "first transition sample differing from the old run",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            k(first),
            "sample",
            Tolerance::Exact(1.0), // 011 FR-001: engineering target
        ),
        MeasurementResult::new(
            "FR-002",
            "last transition sample differing from the new run (N - 1)",
            T_RAMP,
            rate,
            config,
            stim,
            k(last),
            "sample",
            Tolerance::Exact((n - 1) as f64),
        ),
    ]
}

/// The ratio of `y` to the reference `r` over the samples in `range` where |r| ≥ 0.25 of the
/// reference's peak: `(index, ratio)`.
fn ratios(y: &[f32], r: &[f32], range: core::ops::Range<usize>) -> Vec<(usize, f64)> {
    let peak = r[range.clone()]
        .iter()
        .fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
    range
        .filter(|&i| f64::from(r[i]).abs() >= 0.25 * peak)
        .map(|i| (i, f64::from(y[i]) / f64::from(r[i])))
        .collect()
}

/// Count of consecutive ratio pairs that move against `up` by more than [`RATIO_ULP`] ULP.
fn ratio_reversals(q: &[(usize, f64)], up: bool) -> usize {
    q.windows(2)
        .filter(|w| {
            let (a, b) = (w[0].1, w[1].1);
            let tol = RATIO_ULP * f64::from(f32::EPSILON) * a.abs().max(b.abs());
            if up { b < a - tol } else { b > a + tol }
        })
        .count()
}

/// US1 AS1 (c) (FR-001, FR-003): the black-box ratio reader: the output divided by a run held
/// at position 1, over the samples where the reference is at least 0.25 of its peak, is
/// monotonic through the transition within [`RATIO_ULP`] f32 ULP.
pub fn ratio_reader(
    make: Make<'_>,
    rate: u32,
    config: &'static str,
    from: f32,
    to: f32,
) -> Vec<MeasurementResult> {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(make, config, rate, level);
    let (c, n) = (s.c, s.n);
    let y = s.run(make, from, &[(c, to)], Blocks::Fixed(64));
    let r = s.held(make, 1.0, Blocks::Fixed(64));
    let q = ratios(&y, &r, c.saturating_sub(64)..c + n + 64);
    vec![MeasurementResult::new(
        "FR-003",
        "ratio reader reversals beyond 4 ULP through the transition",
        T_RAMP,
        rate,
        config,
        format!(
            "{} ({} samples read)",
            describe(level, from, to, c),
            q.len()
        ),
        ratio_reversals(&q, to > from) as f64,
        "",
        Tolerance::Exact(0.0), // 011 FR-003, SC-001: engineering target
    )]
}

/// US1 AS2 (FR-002): the same change with blocks of 1, 64 and 4096 samples and an irregular
/// partition (a boundary at the change, a multiple of 4096): bit-identical.
pub fn partitions(make: Make<'_>, rate: u32, config: &'static str) -> Vec<MeasurementResult> {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(make, config, rate, level);
    let changes = [(s.c, 1.0f32)];
    let want = s.run(make, 0.1, &changes, Blocks::Fixed(64));
    [
        ("blocks of 1", Blocks::Fixed(1)),
        ("blocks of 4096", Blocks::Fixed(4096)),
        ("irregular 1..700", Blocks::Irregular(0x0011)),
    ]
    .into_iter()
    .map(|(name, blocks)| {
        MeasurementResult::new(
            "FR-002",
            "partition-independent transition",
            T_RAMP,
            rate,
            config,
            format!("{}, {name} vs 64", describe(level, 0.1, 1.0, s.c)),
            pass(same(&s.run(make, 0.1, &changes, blocks), &want)),
            "",
            Tolerance::Exact(1.0), // 011 FR-002: engineering target
        )
    })
    .collect()
}

/// FR-004's reduction in dB for a change `from` → `to` in a stage-11-only configuration made by
/// `make`, against the splice of two constant-position runs made by `reference`.
pub fn click_reduction_db(
    make: Make<'_>,
    reference: Make<'_>,
    rate: u32,
    config: &'static str,
    from: f32,
    to: f32,
) -> f64 {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(reference, config, rate, level);
    let (c, n) = (s.c, s.n);
    let y = s.run(make, from, &[(c, to)], Blocks::Fixed(64));
    let mut step = s.held(reference, from, Blocks::Fixed(64));
    let b = s.held(reference, to, Blocks::Fixed(64));
    step[c..].copy_from_slice(&b[c..]);
    let e_step = click_energy(&step, rate, c as f64);
    let e_ramp = click_energy(&y, rate, c as f64 + (n as f64 - 1.0) / 2.0);
    power_db(e_step / e_ramp)
}

/// US1 AS3, SC-002 (FR-004): the click reduction is at least 30 dB and within ±0.25 dB of the
/// script's figure ([`RAMP_MODEL_FR004`]).
pub fn click_reduction(
    make: Make<'_>,
    rate: u32,
    config: &'static str,
    from: f32,
    to: f32,
) -> Vec<MeasurementResult> {
    let route = configs::stage11_route(config);
    let r = click_reduction_db(make, make, rate, config, from, to);
    let col = match route {
        Route::Earphone => 2,
        _ => 0,
    } + usize::from(to < from);
    let want = RAMP_MODEL_FR004[rate_index(rate)][col];
    let stim = describe(level_dbfs(route), from, to, change_at(rate));
    vec![
        MeasurementResult::new(
            "FR-004",
            "click reduction against the instantaneous step",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            r,
            "dB",
            Tolerance::AtLeast(FLOOR_DB),
        ),
        MeasurementResult::new(
            "FR-004",
            "click reduction against ramp_model.py (+-0.25 dB)",
            T_RAMP,
            rate,
            config,
            format!("{stim}; script {want:.2} dB"),
            r,
            "dB",
            Tolerance::Range(want - PIN_DB, want + PIN_DB),
        ),
    ]
}

/// US1 AS4: on the speaker route a full-scale tone never exceeds the knee's ceiling (1.0 in the
/// route's scale, A-042) through an up, a down and a retarget change, and the splice identity
/// holds before the first change and from the last change's sample N.
pub fn ceiling(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let config = "stage11_only_speaker";
    let s = scene(make, config, rate, 0.0);
    let (c, n) = (s.c, s.n);
    let c2 = c + floor16(n / 2);
    let mut rows = Vec::new();
    for (name, start, changes) in [
        ("up", 0.1f32, vec![(c, 1.0f32)]),
        ("down", 1.0, vec![(c, 0.1)]),
        ("retarget", 0.1, vec![(c, 1.0), (c2, 0.4)]),
    ] {
        let y = s.run(make, start, &changes, Blocks::Fixed(16));
        let a = s.held(make, start, Blocks::Fixed(16));
        let (last_at, last_p) = *changes.last().unwrap();
        let b = s.held(make, last_p, Blocks::Fixed(16));
        let peak = y.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
        let stim = format!("1 kHz 0 dBFS, {name}");
        rows.push(MeasurementResult::new(
            "FR-003",
            "speaker route peak through a transition (ceiling 1.0; a knee regression row)",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            peak,
            "FS",
            Tolerance::AtMost(1.0), // 011 US1 AS4; A-042 (the route's full scale): engineering target
        ));
        rows.push(MeasurementResult::new(
            "FR-001",
            "splice identity before and after on a full-scale tone",
            T_RAMP,
            rate,
            config,
            stim,
            pass(same(&y[..c], &a[..c]) && same(&y[last_at + n - 1..], &b[last_at + n - 1..])),
            "",
            Tolerance::Exact(1.0), // 011 FR-001: engineering target
        ));
    }
    rows
}

/// US1 AS5, SC-003 (FR-003): a retarget to 0.4 half-way through a 0.1 → 1 transition starts
/// from the knob's current position without a jump (the ratio's step across the retarget is no
/// larger than the largest step of the first transition), ends bit-identical to the 0.4 run
/// from its sample N; setting the target the knob is already moving to changes nothing.
pub fn retarget(make: Make<'_>, rate: u32, config: &'static str) -> Vec<MeasurementResult> {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(make, config, rate, level);
    let (c, n) = (s.c, s.n);
    let c2 = c + floor16(n / 2);
    let y = s.run(make, 0.1, &[(c, 1.0), (c2, 0.4)], Blocks::Fixed(16));
    let b = s.held(make, 0.4, Blocks::Fixed(16));
    let r = s.held(make, 1.0, Blocks::Fixed(16));
    let q = ratios(&y, &r, c..c2 + n);
    let before: Vec<_> = q.iter().filter(|(i, _)| *i < c2).collect();
    let max_step = before
        .windows(2)
        .map(|w| (w[1].1 - w[0].1).abs())
        .fold(0.0f64, f64::max);
    let across = q
        .iter()
        .position(|(i, _)| *i >= c2)
        .filter(|&k| k > 0)
        .map_or(f64::NAN, |k| (q[k].1 - q[k - 1].1).abs());
    let once = s.run(make, 0.1, &[(c, 1.0)], Blocks::Fixed(16));
    let twice = s.run(make, 0.1, &[(c, 1.0), (c2, 1.0)], Blocks::Fixed(16));
    let stim = format!("1 kHz {level:.0} dBFS, 0.1 -> 1 at {c}, -> 0.4 at {c2}");
    vec![
        MeasurementResult::new(
            "FR-003",
            "ratio step across the retarget / largest step before it",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            across / max_step,
            "",
            Tolerance::AtMost(1.0), // 011 SC-003 (no jump at a retarget): engineering target
        ),
        MeasurementResult::new(
            "FR-001",
            "bit-identical to the 0.4 run from the retarget's sample N",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            pass(same(&y[c2 + n - 1..], &b[c2 + n - 1..])),
            "",
            Tolerance::Exact(1.0), // 011 FR-001: engineering target
        ),
        MeasurementResult::new(
            "FR-003",
            "setting the target already moving to changes nothing",
            T_RAMP,
            rate,
            config,
            stim,
            pass(same(&once, &twice)),
            "",
            Tolerance::Exact(1.0), // 011 FR-003: engineering target
        ),
    ]
}

/// US1 AS6, FR-008: a transition to 0 reads exact +0.0 at the tap after stage 11 from sample N;
/// on the route output (the speaker template or the earphone corners in place) the signal decays
/// below −120 dBFS within 100 ms of sample N.
pub fn zeros(make: Make<'_>, rate: u32, config: &'static str) -> Vec<MeasurementResult> {
    let route = configs::stage11_route(config);
    let level = level_dbfs(route);
    let s = scene(make, config, rate, level);
    let (c, n) = (s.c, s.n);
    let y = s.run(make, 1.0, &[(c, 0.0)], Blocks::Fixed(64));
    let nonzero = y[c + n - 1..].iter().filter(|v| v.to_bits() != 0).count();
    let out_config = match route {
        Route::Earphone => "output_only_earphone",
        _ => "output_only_speaker",
    };
    let settings = configs::settings(out_config, rate);
    let latency = make(settings).latency_samples() as usize;
    let x = stimulus::tone_peaking_at(-20.0, rate, s.x.len(), c, latency);
    let z = run_with_changes(
        make,
        settings,
        route,
        1.0,
        &[(c, 0.0)],
        &x,
        Blocks::Fixed(64),
    );
    let from = c + n - 1 + (0.1 * f64::from(rate)).round() as usize;
    let tail = z[from..]
        .iter()
        .fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
    vec![
        MeasurementResult::new(
            "FR-008",
            "samples not exactly +0.0 from transition sample N",
            T_RAMP,
            rate,
            config,
            describe(level, 1.0, 0.0, c),
            nonzero as f64,
            "",
            Tolerance::Exact(0.0), // 011 FR-008: engineering target
        ),
        MeasurementResult::new(
            "FR-008",
            "route output 100 ms after sample N",
            T_RAMP,
            rate,
            out_config,
            format!("1 kHz -20 dBFS, 1 -> 0 at sample {c}"),
            analysis::db(tail.max(1e-300)),
            "dBFS",
            Tolerance::AtMost(-120.0), // 011 FR-008 (009 FR-009's bound): engineering target
        ),
    ]
}

/// US1 AS8, SC-003: a drag of ten updates [`drag_gap`] apart from 0.1 to 1 in 16-sample blocks:
/// the ratio reader is monotonic and the click energy is at least 30 dB below the same drag
/// made as instantaneous steps (the splice of the constant-position runs).
pub fn drag(make: Make<'_>, rate: u32, config: &'static str) -> Vec<MeasurementResult> {
    let level = level_dbfs(configs::stage11_route(config));
    let s = scene(make, config, rate, level);
    let (c, n, gap) = (s.c, s.n, drag_gap(rate));
    let ps = drag_positions();
    let changes: Vec<(usize, f32)> = ps
        .iter()
        .enumerate()
        .map(|(j, &p)| (c + j * gap, p))
        .collect();
    let y = s.run(make, 0.1, &changes, Blocks::Fixed(16));
    let mut stepped = s.held(make, 0.1, Blocks::Fixed(16));
    for (j, &(at, p)) in changes.iter().enumerate() {
        let run = s.held(make, p, Blocks::Fixed(16));
        let end = changes.get(j + 1).map_or(run.len(), |&(e, _)| e);
        stepped[at..end].copy_from_slice(&run[at..end]);
    }
    let centre = c as f64 + (9 * gap + n) as f64 / 2.0;
    let red = power_db(click_energy(&stepped, rate, centre) / click_energy(&y, rate, centre));
    let r = s.held(make, 1.0, Blocks::Fixed(16));
    let q = ratios(&y, &r, c..c + 9 * gap + n + 16);
    let stim = format!("1 kHz {level:.0} dBFS, ten updates every {gap} samples from {c}");
    vec![
        MeasurementResult::new(
            "SC-003",
            "drag click reduction against the stepped drag",
            T_RAMP,
            rate,
            config,
            stim.clone(),
            red,
            "dB",
            Tolerance::AtLeast(FLOOR_DB),
        ),
        MeasurementResult::new(
            "SC-003",
            "drag ratio reversals beyond 4 ULP",
            T_RAMP,
            rate,
            config,
            stim,
            ratio_reversals(&q, true) as f64,
            "",
            Tolerance::Exact(0.0), // 011 SC-003: engineering target
        ),
    ]
}

/// US1 AS7 (reported): the click reduction at the route output (spec 009's output-only
/// configurations: the speaker template or the earphone corners in place) against the test
/// hooks' instantaneous step (`step`).
pub fn route_output(
    make: Make<'_>,
    step: Make<'_>,
    rate: u32,
    route: Route,
) -> Vec<MeasurementResult> {
    let config = match route {
        Route::Earphone => "output_only_earphone",
        _ => "output_only_speaker",
    };
    let level = level_dbfs(route);
    let settings = configs::settings(config, rate);
    let c = change_at(rate);
    let n = n_of(rate);
    let latency = make(settings).latency_samples() as usize;
    let x = stimulus::tone_peaking_at(level, rate, c + rate as usize, c, latency);
    let y = run_with_changes(
        make,
        settings,
        route,
        0.1,
        &[(c, 1.0)],
        &x,
        Blocks::Fixed(64),
    );
    let z = run_with_changes(
        step,
        settings,
        route,
        0.1,
        &[(c, 1.0)],
        &x,
        Blocks::Fixed(64),
    );
    let red = power_db(
        click_energy(&z, rate, c as f64)
            / click_energy(&y, rate, c as f64 + (n as f64 - 1.0) / 2.0),
    );
    vec![MeasurementResult::new(
        "FR-004",
        "click reduction at the route output (reported)",
        T_RAMP,
        rate,
        config,
        describe(level, 0.1, 1.0, c),
        red,
        "dB",
        Tolerance::Report,
    )]
}

// ---- Spec 011 User Story 2: the transition through the whole engine (tasks.md T020, T021) ----

/// Records `x` in one block and stops; the recording's number.
fn record(p: &mut rr_dr60::Pipeline, x: &[f32]) -> u32 {
    p.start_recording().expect("idle with room");
    let info = p.record(x, &mut []).expect("recording");
    assert!(info.stopped.is_none(), "the memory filled");
    match p.stop_recording(&mut []).expect("recording").result {
        rr_dr60::StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    }
}

/// Plays recording `number` to the end in blocks of `block`, applying `changes`
/// (`(output sample, position)`, each at a block boundary) before the block that starts there.
fn play_with_changes(
    p: &mut rr_dr60::Pipeline,
    number: u32,
    block: usize,
    changes: &[(usize, f32)],
) -> Vec<f32> {
    p.start_playback(number).expect("idle, existing recording");
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block];
    loop {
        for &(at, v) in changes {
            if at == out.len() {
                p.set_volume(v).expect("a valid position");
            }
        }
        let info = p.play(&mut buf).expect("playing");
        out.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    out
}

/// Processes `x` through `p` in blocks of 64; the produced output.
fn process_64(p: &mut rr_dr60::Pipeline, x: &[f32]) -> Vec<f32> {
    let mut y = Vec::with_capacity(x.len());
    for chunk in x.chunks(64) {
        let mut out = vec![0.0f32; chunk.len()];
        let n = p.process(chunk, &mut out).expect("ok").produced;
        y.extend_from_slice(&out[..n]);
    }
    y
}

/// US2 AS1 – AS3, AS5, AS6, FR-005 (research R-04): every "take the position at once" rule,
/// checked against the `volume_step` hook (0.10's arithmetic: wherever the rule takes the
/// position at once, the engine and the hook agree bit for bit), and the drain, the stage-11
/// bypass, the output clamp and the one-pass after a playback.
pub fn whole_engine(
    make: Make<'_>,
    step: Make<'_>,
    rate: u32,
    route: Route,
) -> Vec<MeasurementResult> {
    let config: &'static str = match route {
        Route::Earphone => "stage11_only_earphone",
        _ => "stage11_only_speaker",
    };
    let level = level_dbfs(route);
    let s = configs::memory_capacity(configs::settings(config, rate), 30, 3);
    let latency = make(s).latency_samples() as usize;
    let half = change_at(rate);
    let x = stimulus::tone_peaking_at(level, rate, 2 * half, half, latency);
    let other = match route {
        Route::Earphone => Route::Speaker,
        _ => Route::Earphone,
    };
    let fresh = |m: Make<'_>, p0: f32| {
        let mut p = m(s);
        p.set_route(route);
        p.set_volume(p0).unwrap();
        p
    };
    type Script<'s> = &'s dyn Fn(Make<'_>) -> Vec<f32>;
    let mut rows = Vec::new();
    let mut agree = |name: &str, f: Script<'_>| {
        rows.push(MeasurementResult::new(
            "FR-005",
            "takes the position at once (equal to 0.10's step)",
            T_RAMP,
            rate,
            config,
            name.to_string(),
            pass(same(&f(make), &f(step))),
            "",
            Tolerance::Exact(1.0), // 011 FR-005: engineering target
        ));
    };
    agree("after construction", &|m| {
        process_64(&mut fresh(m, 0.3), &x)
    });
    agree("after reset", &|m| {
        let mut p = fresh(m, 0.3);
        let _ = process_64(&mut p, &x[..half]);
        p.set_volume(0.8).unwrap();
        p.reset();
        process_64(&mut p, &x[half..])
    });
    // Post-implementation review B2: `reconfigure` rebuilds the engine (the test hook does not
    // survive it), so its oracle is a new engine at the position, as "created anew" means; the
    // same oracle for `reset`.
    let anew = process_64(&mut fresh(make, 0.8), &x[half..]);
    let mut fresh_rows = Vec::new();
    for (name, reconfigure) in [("reset", false), ("reconfigure", true)] {
        let mut p = fresh(make, 0.3);
        let _ = process_64(&mut p, &x[..half]);
        p.set_volume(0.8).unwrap();
        if reconfigure {
            p.reconfigure(s).unwrap();
        } else {
            p.reset();
        }
        fresh_rows.push(MeasurementResult::new(
            "FR-005",
            "takes the position at once (equal to a new engine)",
            T_RAMP,
            rate,
            config,
            format!("after {name}"),
            pass(same(&process_64(&mut p, &x[half..]), &anew)),
            "",
            Tolerance::Exact(1.0), // 011 FR-005: engineering target
        ));
    }
    agree("after a zero-length call", &|m| {
        let mut p = fresh(m, 0.3);
        let _ = p.process(&[], &mut []).unwrap();
        p.set_volume(0.8).unwrap();
        process_64(&mut p, &x)
    });
    agree("a route and a position change in one gap", &|m| {
        let mut p = fresh(m, 0.3);
        let _ = process_64(&mut p, &x[..half]);
        p.set_route(other);
        p.set_volume(0.8).unwrap();
        process_64(&mut p, &x[half..])
    });
    agree("a route change mid-transition", &|m| {
        let mut p = fresh(m, 0.1);
        let _ = process_64(&mut p, &x[..half]);
        p.set_volume(1.0).unwrap();
        let _ = process_64(&mut p, &x[half..half + 64]);
        p.set_route(other);
        process_64(&mut p, &x[half + 64..])
    });
    agree("the start of each playback", &|m| {
        let mut p = fresh(m, 0.3);
        let number = record(&mut p, &x);
        let _ = play_with_changes(&mut p, number, 480, &[(4800, 1.0)]);
        p.set_volume(0.8).unwrap();
        play_with_changes(&mut p, number, 480, &[])
    });
    // A change during the drain starts a transition; the next playback starts at once.
    agree("a change during the drain, then the next playback", &|m| {
        let mut p = fresh(m, 0.3);
        let number = record(&mut p, &x);
        let pre = p.drain_samples() as usize;
        let first = play_with_changes(&mut p, number, 16, &[]);
        let in_drain = (first.len() - pre / 2) / 16 * 16;
        let again = play_with_changes(&mut p, number, 16, &[(in_drain, 0.9)]);
        assert_eq!(again.len(), first.len(), "the drain length does not move");
        p.set_volume(0.5).unwrap();
        play_with_changes(&mut p, number, 16, &[])
    });
    rows.extend(fresh_rows);
    // Plan review R5: the `volume_step` hook is 0.10's step bit for bit: at the tap after stage
    // 11 its run equals the splice of the two constant-position runs at the change.
    let hooked = run_with_changes(step, s, route, 0.1, &[(half, 1.0)], &x, Blocks::Fixed(64));
    let mut splice_ref = run_with_changes(make, s, route, 0.1, &[], &x, Blocks::Fixed(64));
    let at_one = run_with_changes(make, s, route, 1.0, &[], &x, Blocks::Fixed(64));
    splice_ref[half..].copy_from_slice(&at_one[half..]);
    rows.push(MeasurementResult::new(
        "FR-011",
        "the volume_step hook equals 0.10's step (the splice)",
        T_RAMP,
        rate,
        config,
        "0.1 -> 1".to_string(),
        pass(same(&hooked, &splice_ref)),
        "",
        Tolerance::Exact(1.0), // 011 FR-011: engineering target
    ));
    // Post-implementation review N3: a transition started in a playback's last block completes
    // inside the drain: from its sample N to the end the playback equals one held at the target.
    {
        let mut p = fresh(make, 0.3);
        let number = record(&mut p, &x);
        let plain = play_with_changes(&mut p, number, 480, &[]);
        let drain = p.drain_samples() as usize;
        let last_block = (plain.len() - drain - 1) / 480 * 480;
        let late = play_with_changes(&mut p, number, 480, &[(last_block, 1.0)]);
        p.set_volume(1.0).unwrap();
        let held = play_with_changes(&mut p, number, 480, &[]);
        let from_n = last_block + n_of(rate) - 1;
        rows.push(MeasurementResult::new(
            "FR-005",
            "a transition in the last block completes inside the drain",
            T_RAMP,
            rate,
            config,
            format!("0.3 -> 1 at playback sample {last_block}, drain {drain}"),
            pass(late.len() == held.len() && same(&late[from_n..], &held[from_n..])),
            "",
            Tolerance::Exact(1.0), // 011 FR-005: engineering target
        ));
    }
    // Stage 11 bypassed: the position has no effect.
    let mut bypassed = configs::settings("output_only_speaker", rate);
    bypassed.output.volume_amp_enabled = false;
    let changed = run_with_changes(
        make,
        bypassed,
        route,
        0.1,
        &[(half, 1.0)],
        &x,
        Blocks::Fixed(64),
    );
    let held = run_with_changes(make, bypassed, route, 0.1, &[], &x, Blocks::Fixed(64));
    rows.push(MeasurementResult::new(
        "FR-005",
        "stage 11 bypassed: a position change has no effect",
        T_RAMP,
        rate,
        "output_only_speaker, stage 11 off",
        "0.1 -> 1",
        pass(same(&changed, &held)),
        "",
        Tolerance::Exact(1.0), // 011 FR-005: engineering target
    ));
    // The output clamp (spec 010 FR-005): on == clamp(off) through a transition at full scale.
    let out_config = match route {
        Route::Earphone => "output_only_earphone",
        _ => "output_only_speaker",
    };
    let off_s = configs::settings(out_config, rate);
    let mut on_s = off_s;
    on_s.output.clamp_enabled = true;
    let loud = stimulus::tone_peaking_at(0.0, rate, 2 * half, half, latency);
    let off = run_with_changes(
        make,
        off_s,
        route,
        0.1,
        &[(half, 1.0)],
        &loud,
        Blocks::Fixed(64),
    );
    let on = run_with_changes(
        make,
        on_s,
        route,
        0.1,
        &[(half, 1.0)],
        &loud,
        Blocks::Fixed(64),
    );
    let bounded: Vec<f32> = off.iter().map(|v| v.clamp(-1.0, 1.0)).collect();
    rows.push(MeasurementResult::new(
        "FR-005",
        "output clamp through a transition: on == clamp(off)",
        T_RAMP,
        rate,
        out_config,
        "1 kHz 0 dBFS, 0.1 -> 1",
        pass(same(&on, &bounded)),
        "",
        Tolerance::Exact(1.0), // 011 FR-005: engineering target
    ));
    // The drain: a transition started in a playback's last block completes inside it; the
    // length is unchanged and 009 FR-008's last-millisecond bound holds (the default engine,
    // the VAS off, as 009's drain check).
    let mut d = configs::memory_capacity(configs::settings("default_output", rate), 30, 3);
    d.vas.enabled = false;
    let voiced = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x0D60);
    let mut p = make(d);
    p.set_route(route);
    p.set_volume(0.3).unwrap();
    let number = record(&mut p, &voiced);
    let plain = play_with_changes(&mut p, number, 480, &[]);
    let drain = p.drain_samples() as usize;
    let last_block = (plain.len() - drain - 1) / 480 * 480;
    let late = play_with_changes(&mut p, number, 480, &[(last_block, 1.0)]);
    let ms = (f64::from(rate) / 1000.0).ceil() as usize;
    let tail = late[late.len() - ms..]
        .iter()
        .fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
    rows.push(MeasurementResult::new(
        "FR-005",
        "drain length unchanged by a transition in the last block",
        T_RAMP,
        rate,
        "default_output (VAS off)",
        format!("voiced -20 dBFS, 0.3 -> 1 at {last_block}"),
        late.len() as f64 - plain.len() as f64,
        "samples",
        Tolerance::Exact(0.0), // 011 FR-005: engineering target
    ));
    rows.push(MeasurementResult::new(
        "FR-005",
        "the drain's last millisecond (009 FR-008)",
        T_RAMP,
        rate,
        "default_output (VAS off)",
        format!("voiced -20 dBFS, 0.3 -> 1 at {last_block}"),
        analysis::db(tail.max(1e-300)),
        "dBFS",
        Tolerance::AtMost(-80.0), // 009 FR-008 (the drain's last millisecond): engineering target
    ));
    // The one-pass after a playback in which the volume changed ramps from its own last
    // position at its next block (each path keeps its own knob): equal to the same one-pass
    // with no playback between.
    let with_playback = {
        let mut p = fresh(make, 0.3);
        let _ = process_64(&mut p, &x[..half]);
        let number = record(&mut p, &x[..half]);
        let _ = play_with_changes(&mut p, number, 480, &[(480, 0.6)]);
        p.set_volume(0.8).unwrap();
        process_64(&mut p, &x[half..])
    };
    let without = {
        let mut p = fresh(make, 0.3);
        let _ = process_64(&mut p, &x[..half]);
        p.set_volume(0.8).unwrap();
        process_64(&mut p, &x[half..])
    };
    rows.push(MeasurementResult::new(
        "FR-007",
        "the one-pass ramps from its own knob after a playback",
        T_RAMP,
        rate,
        config,
        "0.3, a playback with a change, then 0.8",
        pass(same(&with_playback, &without)),
        "",
        Tolerance::Exact(1.0), // 011 FR-007: engineering target
    ));
    rows
}

/// US2 AS4, FR-007, SC-005: with a change at the same output sample, record-then-play equals
/// the one-pass bit for bit from that sample on, in the default engine with the VAS off under
/// spec 008's rule (the one-pass skipped by the codec delay d; the change at playback sample
/// P and at one-pass sample P + d). The constant-volume equivalence is spec 009's
/// `output_checks::chain_equivalence`.
pub fn equivalence_with_change(make: Make<'_>, rate: u32, route: Route) -> Vec<MeasurementResult> {
    use rr_dr60::speech_codec::{DELAY_SAMPLES, LOOKAHEAD_SAMPLES};
    let mut s = configs::settings("default_output", rate);
    s.vas.enabled = false;
    let x = stimulus::voiced(f64::from(rate), 2.0, -20.0, 0x0D60);
    let d = crate::codec_checks::delay_host(rate);
    let per_device = crate::split_checks::host_per_device(rate);
    let mut p = make(s);
    p.set_route(route);
    p.set_volume(0.1).unwrap();
    p.start_recording().unwrap();
    for chunk in x.chunks(crate::split_checks::RECORD_BLOCK) {
        let info = p.record(chunk, &mut []).unwrap();
        assert!(info.stopped.is_none());
    }
    let stop = p.stop_recording(&mut []).unwrap();
    let rr_dr60::StopResult::Kept { number, .. } = stop.result else {
        panic!("empty recording")
    };
    let at = floor16(rate as usize / 2);
    let mut got = play_with_changes(&mut p, number, 16, &[(at, 1.0)]);
    got.truncate(got.len() - p.drain_samples() as usize);
    let pre_encoder = p.record_latency_samples() as usize - d;
    let tail = pre_encoder
        + (stop.silence_fed_samples as usize + (DELAY_SAMPLES - LOOKAHEAD_SAMPLES) as usize + 1)
            * per_device;
    let mut y = x.clone();
    y.extend(core::iter::repeat_n(0.0f32, tail));
    let want = run_with_changes(
        make,
        s,
        route,
        0.1,
        &[(at + d, 1.0)],
        &y,
        Blocks::Fixed(4096),
    );
    let n = got.len();
    let equal = want.len() >= d + n && same(&got, &want[d..d + n]);
    vec![MeasurementResult::new(
        "FR-007",
        "record-then-play equals the one-pass with a change at the same output sample",
        T_RAMP,
        rate,
        "default_output (VAS off)",
        format!("voiced -20 dBFS, {route:?}, 0.1 -> 1 at playback sample {at}"),
        pass(equal),
        "",
        Tolerance::Exact(1.0), // 011 FR-007: engineering target
    )]
}

/// Post-implementation review B1 (reported): how far the knob trails a host that moves the
/// position along a straight line from 0.1 to 1 over 1 s with an update every `every` host
/// samples (each update restarts the 20 ms movement from rest), in milliseconds, averaged over
/// the last 40 % of the line. Read on the earphone route of the stage-11-only configuration at
/// full scale, where the output divided by a run at position 1 is the loaded law's gain
/// exactly; the knob position is the law inverted by bisection.
pub fn follow_lag_ms(make: Make<'_>, rate: u32, every: usize) -> f64 {
    let config = "stage11_only_earphone";
    let settings = configs::settings(config, rate);
    let route = Route::Earphone;
    let len = rate as usize;
    let latency = make(settings).latency_samples() as usize;
    let lead = 2 * every.max(4096);
    let x = stimulus::tone_peaking_at(0.0, rate, lead + len + 4096, lead, latency);
    let line = |i: usize| 0.1 + 0.9 * (i as f64 / len as f64).min(1.0);
    let changes: Vec<(usize, f32)> = (0..len)
        .step_by(every)
        .skip(1)
        .map(|i| (lead + i, line(i) as f32))
        .collect();
    let y = run_with_changes(
        make,
        settings,
        route,
        0.1,
        &changes,
        &x,
        Blocks::Fixed(every),
    );
    let r = run_with_changes(make, settings, route, 1.0, &[], &x, Blocks::Fixed(every));
    let gain = |p: f64| analysis::from_db(f64::from(rr_dr60::output::earphone_gain_db(p as f32)));
    let invert = |g: f64| {
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if gain(mid) < g { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    };
    let q = ratios(&y, &r, lead + len * 6 / 10..lead + len);
    let slope = 0.9 / len as f64; // position per host sample
    let lags: Vec<f64> = q
        .iter()
        .map(|&(i, g)| (line(i - lead) - invert(g)) / slope / f64::from(rate) * 1000.0)
        .collect();
    lags.iter().sum::<f64>() / lags.len() as f64
}
