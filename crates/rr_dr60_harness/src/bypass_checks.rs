//! Spec 021 bypass checks (research R-06, R-09, R-10): one function per measurable property of
//! the live playback-stage bypass, every oracle outside the switched engine: the **twins** (the
//! all-on run and the configured-off run of the same engine, the same schedule of route, volume
//! and battery), the stepped build (`BypassMutation::fade_step`, a switch at once) as the click
//! reader's reference, and the device-rate trace hooks for stage 10. Every tolerance is an
//! engineering target of spec 021 FR-001 – FR-010 and bounds the engine's convention, not the
//! device (the RR-DR60 has no stage switches).

use rr_dr60::__test_hooks::{self, BypassMutation};
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream, output};

use crate::analysis::{self, click_energy_band, db};
use crate::changes_checks::{self as cc, Change, Partition, Source};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

/// Trace: an engine convention (A-045's length); no assumption.
pub const T_BYPASS: &[&str] = &["A-045", "engineering target"];

/// The control manifest's index of stage 10's switch (pinned by the core against the manifest).
pub const CONTROL_STAGE_10: u32 = 5;
/// The control manifest's index of stage 11's switch.
pub const CONTROL_STAGE_11: u32 = 22;
/// The control manifest's index of stage 12's switch.
pub const CONTROL_STAGE_12: u32 = 23;
/// The control manifest's index of stage 13's switch.
pub const CONTROL_STAGE_13: u32 = 24;

/// The stored −6 dBFS tone's amplitude.
pub const AMP: f64 = 0.5;

/// One cell of the matrix: a stage, its control, the configuration, the route, the stream,
/// and whether the endpoint identities are bit-exact there (no stateful stage follows the
/// switched one on the active route; research R-10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// The stage (10 … 13).
    pub stage: u8,
    /// Its control manifest index.
    pub control: u32,
    /// The harness configuration name.
    pub config: &'static str,
    /// The active route.
    pub route: Route,
    /// The stream the switch is scheduled on.
    pub stream: Stream,
    /// The endpoint identities are bit-exact (no stateful stage follows the switched one).
    pub exact: bool,
}

impl Cell {
    const fn new(
        stage: u8,
        control: u32,
        config: &'static str,
        route: Route,
        stream: Stream,
        exact: bool,
    ) -> Self {
        Self {
            stage,
            control,
            config,
            route,
            stream,
            exact,
        }
    }

    /// A label for the rows.
    pub fn label(&self) -> String {
        format!(
            "stage {} {} {:?} {:?}{}",
            self.stage,
            self.config,
            self.route,
            self.stream,
            if self.exact { "" } else { " (converging)" }
        )
    }
}

/// The default-engine configuration name of the checks (every playback stage on; the record
/// side quiet: the speech codec, the gate and the microphone off, so the one-pass carries the
/// stimulus alone).
pub const DEFAULT_CHAIN: &str = "bypass_default_chain";

/// The cells (spec D9(b); research R-10): the exact cells are the stage-only configurations
/// (`playback_only`, `output_only_amp`, `output_only_speaker`, `output_only_earphone`) and the
/// last stages of the default engine (12 on the speaker route, 13 on the earphone route); the
/// converging cells are stage 11 in the default engine on both routes and stage 10 with the
/// chain on. Stage 10 is playback-only (D6); stages 11 – 13 run on both streams.
pub fn cells() -> Vec<Cell> {
    use Route::{Earphone, Speaker};
    use Stream::{OnePass, Playback};
    let mut v = vec![
        Cell::new(10, CONTROL_STAGE_10, "playback_only", Speaker, Playback, true),
        Cell::new(11, CONTROL_STAGE_11, "output_only_amp", Speaker, Playback, true),
        Cell::new(11, CONTROL_STAGE_11, "output_only_amp", Earphone, Playback, true),
        Cell::new(11, CONTROL_STAGE_11, "output_only_amp", Speaker, OnePass, true),
        Cell::new(12, CONTROL_STAGE_12, "output_only_speaker", Speaker, Playback, true),
        Cell::new(12, CONTROL_STAGE_12, "output_only_speaker", Speaker, OnePass, true),
        Cell::new(13, CONTROL_STAGE_13, "output_only_earphone", Earphone, Playback, true),
        Cell::new(13, CONTROL_STAGE_13, "output_only_earphone", Earphone, OnePass, true),
        Cell::new(12, CONTROL_STAGE_12, DEFAULT_CHAIN, Speaker, Playback, true),
        Cell::new(13, CONTROL_STAGE_13, DEFAULT_CHAIN, Earphone, Playback, true),
        Cell::new(11, CONTROL_STAGE_11, DEFAULT_CHAIN, Speaker, Playback, false),
        Cell::new(11, CONTROL_STAGE_11, DEFAULT_CHAIN, Earphone, Playback, false),
        Cell::new(11, CONTROL_STAGE_11, DEFAULT_CHAIN, Speaker, OnePass, false),
        Cell::new(10, CONTROL_STAGE_10, DEFAULT_CHAIN, Speaker, Playback, false),
    ];
    v.dedup();
    v
}

/// The settings of a configuration name at `rate` (the harness names plus [`DEFAULT_CHAIN`]).
pub fn settings(config: &str, rate: u32) -> Settings {
    if config == DEFAULT_CHAIN {
        let mut s = Settings::new(rate);
        s.speech_codec_enabled = false;
        s.vas.enabled = false;
        s.agc.enabled = false;
        s.mic = rr_dr60::MicSettings::BYPASSED;
        s
    } else {
        configs::settings(config, rate)
    }
}

/// The same settings with the cell's stage configured off (the twin).
pub fn twin_settings(cell: &Cell, rate: u32) -> Settings {
    let mut s = settings(cell.config, rate);
    match cell.stage {
        10 => s.playback_stage_enabled = false,
        11 => s.output.volume_amp_enabled = false,
        12 => s.output.speaker_enabled = false,
        _ => s.output.earphone_enabled = false,
    }
    s
}

/// The stepped build: a switch at once (research R-09).
pub fn stepped(s: Settings) -> Pipeline {
    Pipeline::__with_bypass_mutation(
        s,
        &BypassMutation {
            fade_step: true,
            ..BypassMutation::default()
        },
    )
    .unwrap()
}

/// A make with one mutant on.
pub fn mutant(m: BypassMutation) -> impl Fn(Settings) -> Pipeline {
    move |s| Pipeline::__with_bypass_mutation(s, &m).unwrap()
}

/// N for the cell's stage at `rate`: 20 ms of host samples (stages 11 – 13) or 120 device
/// samples (stage 10).
pub fn n_fade(stage: u8, rate: u32) -> usize {
    if stage == 10 {
        rr_dr60::speed::CROSSFADE_SAMPLES as usize
    } else {
        output::volume_transition_samples(rate).unwrap() as usize
    }
}

/// The bypass weight at phase k of N (the published smoothstep).
pub fn weight(k: usize, n: usize) -> f64 {
    let u = k as f64 * (1.0 / n as f64);
    u * u * (3.0 - 2.0 * u)
}

/// The stage-11 neutral position per route (D11; research R-07): the speaker route's reference
/// position, the earphone route's position 1.
pub fn neutral_position(route: Route) -> f32 {
    match route {
        Route::Earphone => 1.0,
        _ => output::VOLUME_REFERENCE as f32,
    }
}

/// The stored stimulus of the cells: a −6 dBFS 1 kHz tone of `seconds` at the device rate.
pub fn stored_tone(freq_hz: f64, seconds: f64) -> Vec<f32> {
    stimulus::stored_tone(freq_hz, AMP, (6000.0 * seconds) as usize)
}

/// An engine at `s` holding `x` as recording 1 (`configs::stored_recording`), on `route` at
/// `volume`, the controls taken before any stream runs.
pub fn engine(make: Make<'_>, s: Settings, x: &[f32], route: Route, volume: f32) -> Pipeline {
    let (mut p, number) = configs::stored_recording(make, s, x);
    assert_eq!(number, 1);
    p.set_route(route);
    p.set_volume(volume).unwrap();
    p
}

/// One run of a cell: `stored` played back (the playback stream) or `host` processed one-pass,
/// with `changes` scheduled, at `partition`.
pub fn run_cell(
    make: Make<'_>,
    s: Settings,
    cell: &Cell,
    stored: &[f32],
    host: &[f32],
    volume: f32,
    changes: &[Change],
    partition: Partition,
) -> cc::Run {
    let mut p = engine(make, s, stored, cell.route, volume);
    match cell.stream {
        Stream::Playback => cc::run_scheduled(&mut p, Source::Playback(1), changes, partition),
        _ => cc::run_scheduled(&mut p, Source::OnePass(host), changes, partition),
    }
}

/// A switch change on the cell's stream (`value` 1 on, 0 bypassed).
pub fn change(cell: &Cell, position: u64, on: bool, tag: u64) -> Change {
    Change {
        stream: cell.stream,
        position,
        control: cell.control,
        value: if on { 1.0 } else { 0.0 },
        tag,
    }
}

/// The host-rate stimulus the one-pass cells feed: the same tone at the host rate.
pub fn host_tone(freq_hz: f64, seconds: f64, rate: u32) -> Vec<f32> {
    stimulus::tone(freq_hz, AMP, f64::from(rate), (f64::from(rate) * seconds) as usize)
}

fn pass(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// |a − b| in units of the f32 spacing at the larger magnitude (0 when equal).
fn ulps(a: f32, b: f32) -> f64 {
    if a == b {
        return 0.0;
    }
    ulps_at(f64::from(a) - f64::from(b), f64::from(a.abs().max(b.abs())))
}

/// |err| in units of the f32 spacing at magnitude `m` (the blend's error is relative to the
/// branches' magnitude, not to the output's: near a zero crossing of y + w·(x − y) the output
/// is small while the narrowed inputs' rounding is not).
fn ulps_at(err: f64, m: f64) -> f64 {
    let m = m.max(f64::from(f32::MIN_POSITIVE));
    err.abs() / (m * f64::from(f32::EPSILON))
}

/// The largest |a − b| over `range` as a dBFS figure and in f32 LSBs of the larger sample.
fn max_delta(a: &[f32], b: &[f32], range: core::ops::Range<usize>) -> (f64, f64) {
    let mut peak = 0.0f64;
    let mut lsb = 0.0f64;
    for i in range {
        if let (Some(x), Some(y)) = (a.get(i), b.get(i)) {
            peak = peak.max((f64::from(*x) - f64::from(*y)).abs());
            lsb = lsb.max(ulps(*x, *y));
        }
    }
    (if peak > 0.0 { db(peak) } else { -400.0 }, lsb)
}

/// The first index at or after `from` from which `a` and `b` are bit-identical to the end of
/// `b` (None when they never are).
fn first_identical(a: &[f32], b: &[f32], from: usize) -> Option<usize> {
    let n = a.len().min(b.len());
    if from >= n {
        return None;
    }
    let mut last_diff: Option<usize> = None;
    for i in from..n {
        if a[i].to_bits() != b[i].to_bits() {
            last_diff = Some(i);
        }
    }
    match last_diff {
        None => Some(from),
        Some(d) if d + 1 < n => Some(d + 1),
        Some(_) => None,
    }
}

/// The positions of a cell's schedule at `rate`: the landing at 0.5 s, the return 0.4 s after
/// the blend's end, on a 1.5 s stimulus.
pub fn positions(stage: u8, rate: u32) -> (u64, u64) {
    let p = u64::from(rate) / 2;
    let n = n_fade(stage, rate) as u64;
    let n_host = if stage == 10 {
        n * u64::from(rate) / 6000
    } else {
        n
    };
    (p, p + n_host + u64::from(rate) * 2 / 5)
}

/// The blend's span in host samples for a landing at `p`: stage 10's device-rate blend reaches
/// the host output over N·rate/6000 samples (its exactness there is the trace's).
fn blend_host_len(stage: u8, rate: u32) -> usize {
    let n = n_fade(stage, rate);
    if stage == 10 {
        n * rate as usize / 6000
    } else {
        n
    }
}

// ---- D9(b): the endpoint identities ---------------------------------------------------------------

/// Spec 021 US1 AS2 / AS4 (FR-003; research R-10): a bypass at P and a return at Q on one cell.
/// Before P the live output equals the all-on run; from P + N − 1 to Q it equals the
/// configured-off twin; from Q + N − 1 the all-on run again. Bit for bit on an exact cell; on a
/// converging cell within −100 dBFS from P + N − 1 + N_OUT with the largest difference (dBFS and
/// f32 LSBs) and the first bit-identical sample reported.
pub fn endpoint_identity(make: Make<'_>, cell: &Cell, rate: u32) -> Vec<MeasurementResult> {
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 1.5);
    let host = host_tone(1000.0, 1.5, rate);
    let volume = if cell.stage == 11 {
        neutral_position(cell.route)
    } else {
        output::VOLUME_REFERENCE as f32
    };
    let (p, q) = positions(cell.stage, rate);
    let sched = [change(cell, p, false, 1), change(cell, q, true, 2)];
    let live = run_cell(make, s, cell, &stored, &host, volume, &sched, Partition::Fixed(256));
    let all_on = run_cell(make, s, cell, &stored, &host, volume, &[], Partition::Fixed(256));
    let twin = run_cell(
        make,
        twin_settings(cell, rate),
        cell,
        &stored,
        &host,
        volume,
        &[],
        Partition::Fixed(256),
    );
    let n = blend_host_len(cell.stage, rate);
    let (p, q) = (p as usize, q as usize);
    let label = cell.label();
    let stim = format!("{label}: 1 kHz -6 dBFS, bypass at {p}, return at {q}");
    let mut rows = Vec::new();
    // Before the landing: the all-on run (stage 10's blend reaches the host output later).
    let before = cc::first_diff(&live.out[..p], &all_on.out[..p]).is_none();
    rows.push(MeasurementResult::new(
        "FR-003",
        "bit-identical to the all-on run before the landing",
        T_BYPASS,
        rate,
        cell.config,
        stim.clone(),
        pass(before),
        "",
        Tolerance::Exact(1.0), // 021 FR-003
    ));
    let settle = usize::try_from(output::settle_samples(rate).unwrap()).unwrap();
    // Stage 10's blend reaches the host output through the resampler's kernel: the identity
    // holds from the bound P + N_host − 1 + 2D (research R-03); the chain stages' from P + N − 1.
    let two_d = if cell.stage == 10 {
        2 * usize::try_from(__test_hooks::resampler_delay_host_samples(rate).unwrap_or(0)).unwrap()
    } else {
        0
    };
    if cell.exact {
        let from = p + n - 1 + two_d;
        let mism = (from..q)
            .filter(|&i| live.out[i].to_bits() != twin.out[i].to_bits())
            .count();
        rows.push(MeasurementResult::new(
            "FR-003",
            "mismatches against the configured-off twin from P + N - 1 to Q",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            mism as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-003, SC-002 (exact where the stage is last)
        ));
        let from_q = q + n - 1 + two_d;
        let mism_back = (from_q..all_on.out.len().min(live.out.len()))
            .filter(|&i| live.out[i].to_bits() != all_on.out[i].to_bits())
            .count();
        rows.push(MeasurementResult::new(
            "FR-003",
            "mismatches against the all-on run after the return",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            mism_back as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-003 (the warm stage)
        ));
    } else {
        let from = p + n - 1 + settle + two_d;
        let (dbfs, lsb) = max_delta(&live.out, &twin.out, from..q);
        rows.push(MeasurementResult::new(
            "FR-003",
            "largest difference from the twin after the settle (dBFS)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            dbfs,
            "dBFS",
            Tolerance::AtMost(-100.0), // 021 FR-003 (the drain's criterion, 009 R-08)
        ));
        rows.push(MeasurementResult::new(
            "FR-003",
            "largest difference from the twin after the settle (f32 LSBs)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            lsb,
            "LSB",
            Tolerance::Report,
        ));
        let first = first_identical(&live.out[..q], &twin.out[..q], p + n - 1);
        rows.push(MeasurementResult::new(
            "FR-003",
            "first bit-identical sample after the landing (-1: none before Q)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            first.map_or(-1.0, |i| i as f64),
            "sample",
            Tolerance::Report,
        ));
        let from_q = q + n - 1 + settle + two_d;
        let end = all_on.out.len().min(live.out.len());
        let (dbfs_b, _) = max_delta(&live.out, &all_on.out, from_q..end);
        rows.push(MeasurementResult::new(
            "FR-003",
            "largest difference from the all-on run after the return's settle (dBFS)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            dbfs_b,
            "dBFS",
            Tolerance::AtMost(-100.0), // 021 FR-003
        ));
    }
    // The lengths (D4): the live engine's equals the all-on run's.
    rows.push(MeasurementResult::new(
        "FR-004",
        "live length equals the all-on length",
        T_BYPASS,
        rate,
        cell.config,
        stim.clone(),
        pass(live.out.len() == all_on.out.len()),
        "",
        Tolerance::Exact(1.0), // 021 FR-004
    ));
    // The log: two entries for this control, chained, the stream word.
    let entries = cc::entries_of(&live.log, cell.control);
    let word = match cell.stream {
        Stream::Playback => 1,
        Stream::OnePass => 2,
        _ => 3,
    };
    let logged = entries.len() == 2
        && entries[0].position == p as u64
        && entries[0].before == 1.0
        && entries[0].after == 0.0
        && entries[0].tag == 1
        && entries[1].position == q as u64
        && entries[1].before == 0.0
        && entries[1].after == 1.0
        && entries[1].tag == 2
        && entries.iter().all(|e| e.stream == word);
    rows.push(MeasurementResult::new(
        "FR-001",
        "two chained entries with the stream word",
        T_BYPASS,
        rate,
        cell.config,
        stim,
        pass(logged),
        "",
        Tolerance::Exact(1.0), // 021 FR-001
    ));
    rows
}

/// Spec 021 research R-10: a bypass held to the end on an exact cell whose twin runs no chain
/// stage (stage 11 alone) or no stage 10 (`playback_only`): the live engine plays the all-on
/// length, the twin fewer samples, and the live engine's extra samples are the bypassed stage's
/// input with the upstream drained: exactly zero.
pub fn live_tail(make: Make<'_>, cell: &Cell, rate: u32) -> Vec<MeasurementResult> {
    assert_eq!(cell.stream, Stream::Playback, "the one-pass has no drain");
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 1.0);
    let host = host_tone(1000.0, 1.0, rate);
    let (p, _) = positions(cell.stage, rate);
    let volume = neutral_position(cell.route);
    let live = run_cell(make, s, cell, &stored, &host, volume, &[change(cell, p, false, 1)], Partition::Fixed(512));
    let all_on = run_cell(make, s, cell, &stored, &host, volume, &[], Partition::Fixed(512));
    let twin = run_cell(make, twin_settings(cell, rate), cell, &stored, &host, volume, &[], Partition::Fixed(512));
    let extra = live.out.len().saturating_sub(twin.out.len());
    let tail_peak = live.out[twin.out.len().min(live.out.len())..]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()));
    let stim = format!("{}: bypass held to the end", cell.label());
    vec![
        MeasurementResult::new(
            "FR-004",
            "live length equals the all-on length (samples)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            pass(live.out.len() == all_on.out.len()),
            "",
            Tolerance::Exact(1.0), // 021 FR-004
        ),
        MeasurementResult::new(
            "FR-004",
            "the twin is shorter by its missing settle (samples)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            extra as f64,
            "samples",
            Tolerance::AtLeast(1.0), // 021 research R-10
        ),
        MeasurementResult::new(
            "FR-003",
            "the live engine's extra drain samples: the bypassed stage's drained input (peak)",
            T_BYPASS,
            rate,
            cell.config,
            stim,
            f64::from(tail_peak),
            "",
            Tolerance::Exact(0.0), // 021 research R-10 (hardware B9)
        ),
    ]
}

// ---- D9(c): the blend identity ---------------------------------------------------------------

/// Spec 021 US1 AS2 (FR-002): inside the blend each host sample equals y + w·(x − y) of the
/// all-on run (y) and the twin (x) on an exact cell, within 2 f32 LSBs (the runs are the
/// narrowed outputs; the core's f64 test is exact); w(N/2) read back from the engine's
/// midpoint ratio.
pub fn blend_identity(make: Make<'_>, cell: &Cell, rate: u32) -> Vec<MeasurementResult> {
    assert!(cell.exact && cell.stage != 10, "the host-output identity: a last chain stage");
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 1.0);
    let host = host_tone(1000.0, 1.0, rate);
    let volume = if cell.stage == 11 {
        neutral_position(cell.route)
    } else {
        output::VOLUME_REFERENCE as f32
    };
    let (p, _) = positions(cell.stage, rate);
    let n = n_fade(cell.stage, rate);
    let live = run_cell(make, s, cell, &stored, &host, volume, &[change(cell, p, false, 1)], Partition::Fixed(128));
    let all_on = run_cell(make, s, cell, &stored, &host, volume, &[], Partition::Fixed(128));
    let twin = run_cell(make, twin_settings(cell, rate), cell, &stored, &host, volume, &[], Partition::Fixed(128));
    let p = p as usize;
    let mut worst = 0.0f64;
    for k in 1..n {
        let i = p + k - 1;
        let (y, x) = (f64::from(all_on.out[i]), f64::from(twin.out[i]));
        let want = y + weight(k, n) * (x - y);
        worst = worst.max(ulps_at(f64::from(live.out[i]) - want, y.abs().max(x.abs())));
    }
    // The midpoint: (live − y) / (x − y) where the branches differ enough to read it.
    let mid = p + n / 2 - 1;
    let (y, x, l) = (f64::from(all_on.out[mid]), f64::from(twin.out[mid]), f64::from(live.out[mid]));
    let ratio = if (x - y).abs() > 1e-3 { (l - y) / (x - y) } else { 0.5 };
    let stim = format!("{}: bypass at {p}", cell.label());
    vec![
        MeasurementResult::new(
            "FR-002",
            "blend equals y + w(x - y) of the twins (worst, f32 LSBs)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            worst,
            "LSB",
            Tolerance::AtMost(2.0), // 021 FR-002 (narrowed outputs; the core's f64 pin is exact)
        ),
        MeasurementResult::new(
            "FR-002",
            "the midpoint's weight read from the output",
            T_BYPASS,
            rate,
            cell.config,
            stim,
            ratio,
            "",
            Tolerance::Range(0.5 - 1e-4, 0.5 + 1e-4), // 021 SC-002 (w(N/2) = 0.5; read on f32)
        ),
    ]
}

// ---- Stage 10: the device-rate trace and the host onset ------------------------------------------

/// Spec 021 US1 AS3 (FR-002, FR-003; research R-03): stage 10 bypassed at position P of a
/// `playback_only` playback: on the device-rate trace (stage 10's output per pushed device
/// sample, the speed control's outputs its input) the identities and the blend are exact: before
/// d(P) the all-on run, blend samples y + w(x − y) to the bit, from d(P) + 119 the input itself.
/// On the host output, with the chain off, the first bit-different and the first bit-identical
/// sample against the twin are measured (P + ⌈119·rate ÷ 6000⌉ + 2D the bound).
pub fn stage10_trace(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let cell = Cell::new(10, CONTROL_STAGE_10, "playback_only", Route::Speaker, Stream::Playback, true);
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 1.0);
    let (p, _) = positions(10, rate);
    let d_p = rr_dr60::changes::speed_landing(p, rate).unwrap() as usize;
    let n = 120usize;
    // One play call long enough for the whole recording (the trace holds 8192 device samples).
    let total_host = (stored.len() + 400) * rate as usize / 6000;
    let mut live = engine(make, s, &stored, Route::Speaker, output::VOLUME_REFERENCE as f32);
    live.schedule_change(Stream::Playback, p, CONTROL_STAGE_10, 0.0, 1).unwrap();
    live.start_playback(1).unwrap();
    let mut out = vec![0.0f32; total_host];
    let info = live.play(&mut out).unwrap();
    let y_live = __test_hooks::stage10_trace(&live);
    let x_dev = __test_hooks::speed_device_outputs(&live);
    let mut all_on = engine(make, s, &stored, Route::Speaker, output::VOLUME_REFERENCE as f32);
    all_on.start_playback(1).unwrap();
    let mut out_on = vec![0.0f32; total_host];
    let _ = all_on.play(&mut out_on).unwrap();
    let y_on = __test_hooks::stage10_trace(&all_on);
    let mut twin = engine(make, twin_settings(&cell, rate), &stored, Route::Speaker, output::VOLUME_REFERENCE as f32);
    twin.start_playback(1).unwrap();
    let mut out_tw = vec![0.0f32; total_host];
    let info_tw = twin.play(&mut out_tw).unwrap();
    let len = y_live.len().min(y_on.len()).min(x_dev.len());
    let before = (0..d_p.min(len)).all(|i| y_live[i].to_bits() == y_on[i].to_bits());
    let mut blend_mism = 0usize;
    for k in 1..n {
        let i = d_p + k - 1;
        if i < len {
            let want = y_on[i] + weight(k, n) * (x_dev[i] - y_on[i]);
            if y_live[i].to_bits() != want.to_bits() {
                blend_mism += 1;
            }
        }
    }
    let after = (d_p + n - 1..len).filter(|&i| y_live[i].to_bits() != x_dev[i].to_bits()).count();
    let host_live = &out[..info.produced];
    let host_twin = &out_tw[..info_tw.produced];
    let first_diff = cc::first_diff(host_live, &out_on[..info.produced]).map_or(-1.0, |i| i as f64);
    let first_same = first_identical(host_live, host_twin, p as usize).map_or(-1.0, |i| i as f64);
    let d_host = f64::from(__test_hooks::resampler_delay_host_samples(rate).unwrap_or(0));
    let bound = p as f64 + (119.0 * f64::from(rate) / 6000.0).ceil() + 2.0 * d_host;
    let stim = format!("playback_only, 1 kHz -6 dBFS, stage 10 bypassed at P = {p} (d(P) = {d_p})");
    vec![
        MeasurementResult::new(
            "FR-003",
            "device trace: all-on before d(P)",
            T_BYPASS,
            rate,
            "playback_only",
            stim.clone(),
            pass(before),
            "",
            Tolerance::Exact(1.0), // 021 FR-003
        ),
        MeasurementResult::new(
            "FR-002",
            "device trace: blend mismatches against y + w(x - y)",
            T_BYPASS,
            rate,
            "playback_only",
            stim.clone(),
            blend_mism as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-002 (exact on the f64 trace)
        ),
        MeasurementResult::new(
            "FR-003",
            "device trace: the input itself from d(P) + N - 1 (mismatches)",
            T_BYPASS,
            rate,
            "playback_only",
            stim.clone(),
            after as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-003
        ),
        MeasurementResult::new(
            "FR-003",
            "host output: first bit-different sample after P (-1: none)",
            T_BYPASS,
            rate,
            "playback_only",
            stim.clone(),
            first_diff,
            "sample",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-003",
            "host output: first bit-identical sample to the twin (-1: none)",
            T_BYPASS,
            rate,
            "playback_only",
            stim.clone(),
            first_same,
            "sample",
            Tolerance::Range(0.0, bound), // 021 FR-003 (the bound P + ceil(119 rate/6000) + 2D)
        ),
        MeasurementResult::new(
            "FR-003",
            "host output: the bound P + ceil(119 rate / 6000) + 2D",
            T_BYPASS,
            rate,
            "playback_only",
            stim,
            bound,
            "sample",
            Tolerance::Report,
        ),
    ]
}

// ---- FR-007: the click table ------------------------------------------------------------------

/// A click-table row (research R-06): a stored tone at `f_tone` played in the cell's
/// configuration on the cell's route at `volume`, the stage switched off at P = 0.75 s; the
/// crossfade's click energy against the stepped build's under the 40 ms reader (the tone ±250 Hz
/// excluded; the step centred on P, the crossfade on P + N ÷ 2; the band's top `top_hz`), the
/// floor the reader on the all-on held run, stage 11's level change in dB beside it.
pub struct ClickRow {
    /// The stage switched.
    pub stage: u8,
    /// The stage-only configuration.
    pub config: &'static str,
    /// The route.
    pub route: Route,
    /// The volume position (stage 11's rows: the neutral positions or the level-change rows).
    pub volume: f32,
    /// The stored tone's frequency.
    pub f_tone: f64,
    /// Asserted (≥ 30 dB at 6 kHz) or reported.
    pub asserted: bool,
}

/// FR-007's asserted rows (6 kHz): a 1 kHz tone per stage in the stage-only configurations,
/// stage 11 at the neutral positions.
pub fn asserted_rows() -> Vec<ClickRow> {
    vec![
        ClickRow { stage: 10, config: "playback_only", route: Route::Speaker, volume: output::VOLUME_REFERENCE as f32, f_tone: 1000.0, asserted: true },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Speaker, volume: neutral_position(Route::Speaker), f_tone: 1000.0, asserted: true },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Earphone, volume: neutral_position(Route::Earphone), f_tone: 1000.0, asserted: true },
        ClickRow { stage: 12, config: "output_only_speaker", route: Route::Speaker, volume: output::VOLUME_REFERENCE as f32, f_tone: 1000.0, asserted: true },
        ClickRow { stage: 13, config: "output_only_earphone", route: Route::Earphone, volume: output::VOLUME_REFERENCE as f32, f_tone: 1000.0, asserted: true },
    ]
}

/// FR-007's reported rows: the stages where the magnitude dominates, and stage 11's
/// level-change rows.
pub fn reported_rows() -> Vec<ClickRow> {
    vec![
        ClickRow { stage: 10, config: "playback_only", route: Route::Speaker, volume: output::VOLUME_REFERENCE as f32, f_tone: 2600.0, asserted: false },
        ClickRow { stage: 12, config: "output_only_speaker", route: Route::Speaker, volume: output::VOLUME_REFERENCE as f32, f_tone: 300.0, asserted: false },
        ClickRow { stage: 13, config: "output_only_earphone", route: Route::Earphone, volume: output::VOLUME_REFERENCE as f32, f_tone: 100.0, asserted: false },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Speaker, volume: neutral_position(Route::Speaker), f_tone: 100.0, asserted: false },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Earphone, volume: neutral_position(Route::Earphone), f_tone: 100.0, asserted: false },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Speaker, volume: 1.0, f_tone: 1000.0, asserted: false },
        ClickRow { stage: 11, config: "output_only_amp", route: Route::Earphone, volume: output::VOLUME_REFERENCE as f32, f_tone: 1000.0, asserted: false },
    ]
}

fn control_of(stage: u8) -> u32 {
    match stage {
        10 => CONTROL_STAGE_10,
        11 => CONTROL_STAGE_11,
        12 => CONTROL_STAGE_12,
        _ => CONTROL_STAGE_13,
    }
}

/// One click row at `rate` (the reader's top `top_hz`; 2750 at 6 kHz, 0.45 × rate at 48 kHz).
pub fn click_row(make: Make<'_>, row: &ClickRow, rate: u32, top_hz: f64) -> Vec<MeasurementResult> {
    let s = settings(row.config, rate);
    let stored = stored_tone(row.f_tone, 1.5);
    let p = u64::from(rate) * 3 / 4;
    let n_host = blend_host_len(row.stage, rate) as f64;
    let d_host = if row.stage == 10 {
        f64::from(__test_hooks::resampler_delay_host_samples(rate).unwrap_or(0))
    } else {
        0.0
    };
    let cell = Cell::new(row.stage, control_of(row.stage), row.config, row.route, Stream::Playback, true);
    let sched = [change(&cell, p, false, 1)];
    let fade = run_cell(make, s, &cell, &stored, &[], row.volume, &sched, Partition::Fixed(256));
    let step = run_cell(&stepped, s, &cell, &stored, &[], row.volume, &sched, Partition::Fixed(256));
    let held = run_cell(make, s, &cell, &stored, &[], row.volume, &[], Partition::Fixed(256));
    let excl = [row.f_tone];
    let e_step = click_energy_band(&step.out, rate, p as f64 + d_host, 0.04, &excl, top_hz);
    let e_fade = click_energy_band(&fade.out, rate, p as f64 + d_host + n_host / 2.0, 0.04, &excl, top_hz);
    let e_floor = click_energy_band(&held.out, rate, p as f64 + d_host + n_host / 2.0, 0.04, &excl, top_hz);
    // Power ratios: analysis::db is 20·log10, so half of it is the energy ratio in dB.
    let reduction = 0.5 * db(e_step / e_fade.max(1e-300));
    let step_over_floor = 0.5 * db(e_step / e_floor.max(1e-300));
    let level = if row.stage == 11 {
        f64::from(output::stage11_bypass_level_db(row.route, row.volume))
    } else {
        0.0
    };
    let stim = format!(
        "stage {} {} {:?} volume {:.4}, {} Hz tone, reader top {} Hz",
        row.stage, row.config, row.route, row.volume, row.f_tone, top_hz
    );
    let tol = if row.asserted && rate == 6000 {
        Tolerance::AtLeast(30.0) // 021 FR-007
    } else {
        Tolerance::Report
    };
    vec![
        MeasurementResult::new(
            "FR-007",
            "crossfade click energy below the step's (dB)",
            T_BYPASS,
            rate,
            row.config,
            stim.clone(),
            reduction,
            "dB",
            tol,
        ),
        MeasurementResult::new(
            "FR-007",
            "the step above the held-run floor (dB)",
            T_BYPASS,
            rate,
            row.config,
            stim.clone(),
            step_over_floor,
            "dB",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-007",
            "stage 11's level change when bypassed (dB; 0 for the other stages)",
            T_BYPASS,
            rate,
            row.config,
            stim,
            level,
            "dB",
            Tolerance::Report,
        ),
    ]
}

/// FR-007 at 6 kHz: the asserted and reported rows; at 48 kHz the asserted rows reported with the
/// reader's top at 0.45 × rate.
pub fn click_table(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let top = if rate == 6000 { 2750.0 } else { 0.45 * f64::from(rate) };
    let mut rows = Vec::new();
    for r in asserted_rows() {
        rows.extend(click_row(make, &r, rate, top));
    }
    if rate == 6000 {
        for r in reported_rows() {
            rows.extend(click_row(make, &r, rate, top));
        }
    }
    rows
}

/// FR-007: the voiced stimulus per stage (reported): the crossfade's and the step's energies
/// under the reader with no exclusion (voice fills the band).
pub fn voiced_click(make: Make<'_>) -> Vec<MeasurementResult> {
    let rate = 6000;
    let x = stimulus::voiced(6000.0, 1.5, -20.0, 0x021);
    let p = 4500u64;
    let mut rows = Vec::new();
    for r in asserted_rows() {
        let s = settings(r.config, rate);
        let cell = Cell::new(r.stage, control_of(r.stage), r.config, r.route, Stream::Playback, true);
        let sched = [change(&cell, p, false, 1)];
        let fade = run_cell(make, s, &cell, &x, &[], r.volume, &sched, Partition::Fixed(256));
        let step = run_cell(&stepped, s, &cell, &x, &[], r.volume, &sched, Partition::Fixed(256));
        let n_host = blend_host_len(r.stage, rate) as f64;
        let e_step = click_energy_band(&step.out, rate, p as f64, 0.04, &[], 2750.0);
        let e_fade = click_energy_band(&fade.out, rate, p as f64 + n_host / 2.0, 0.04, &[], 2750.0);
        rows.push(MeasurementResult::new(
            "FR-007",
            "voiced: crossfade energy below the step's (dB, no exclusion)",
            T_BYPASS,
            rate,
            r.config,
            format!("stage {} {:?}", r.stage, r.route),
            0.5 * db(e_step / e_fade.max(1e-300)),
            "dB",
            Tolerance::Report,
        ));
    }
    rows
}

// ---- The level table (D11, research R-07) ---------------------------------------------------------

/// Spec 021 D11: stage 11's bypass level change from the published function against the plan's
/// figures (within 0.01 dB): earphone +19.21 at the reference position, 0 at position 1; speaker
/// 0 at the reference position, −21.46 at position 1.
pub fn level_table() -> Vec<MeasurementResult> {
    let p_ref = output::VOLUME_REFERENCE as f32;
    let rows = [
        (Route::Earphone, p_ref, 19.21),
        (Route::Earphone, 1.0f32, 0.0),
        (Route::Speaker, p_ref, 0.0),
        (Route::Speaker, 1.0f32, -21.46),
    ];
    rows.iter()
        .map(|&(route, pos, want)| {
            MeasurementResult::new(
                "FR-011",
                "stage 11 bypass level change (dB)",
                &["A-044", "A-031", "A-043", "engineering target"],
                48_000,
                "output_only_amp",
                format!("{route:?} at position {pos:.4}"),
                f64::from(output::stage11_bypass_level_db(route, pos)),
                "dB",
                Tolerance::Range(want - 0.01, want + 0.01), // 021 D11 (research R-07)
            )
        })
        .collect()
}

// ---- FR-008: the partitions ----------------------------------------------------------------

/// Spec 021 US1 AS6 (FR-008): a cell's schedule at block sizes 1, 64, 4096 and the irregular
/// partition: identical outputs and logs.
pub fn partitions(make: Make<'_>, cell: &Cell, rate: u32) -> Vec<MeasurementResult> {
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 0.6);
    let host = host_tone(1000.0, 0.6, rate);
    let volume = neutral_position(cell.route);
    let p = u64::from(rate) / 5;
    let n = blend_host_len(cell.stage, rate) as u64;
    let sched = [
        change(cell, p, false, 1),
        change(cell, p + n / 2, true, 2), // a reversal inside the blend
        change(cell, p + 2 * n, false, 3),
    ];
    let base = run_cell(make, s, cell, &stored, &host, volume, &sched, Partition::Fixed(4096));
    let mut rows = Vec::new();
    for part in [Partition::Fixed(1), Partition::Fixed(64), Partition::Irregular] {
        let r = run_cell(make, s, cell, &stored, &host, volume, &sched, part);
        let same = cc::same(&r.out, &base.out) && r.log == base.log;
        rows.push(MeasurementResult::new(
            "FR-008",
            "outputs and logs identical to the 4096-sample partition",
            T_BYPASS,
            rate,
            cell.config,
            format!("{}: {part:?}", cell.label()),
            pass(same),
            "",
            Tolerance::Exact(1.0), // 021 FR-008
        ));
    }
    rows
}

// ---- D5 / US1 AS7: the reversal ---------------------------------------------------------------

/// Spec 021 US1 AS7 (FR-005): a bypass at P reversed at P + N/2 on stage 12 (speaker route,
/// `output_only_speaker`): the weight runs back along the curve (within 2 f32 LSBs of the
/// reference built from the twins), the output is bit-identical to the all-on run from
/// P + N − 1, and it lies within one LSB of the two branches' interval; the reversal's click
/// energy against a step reported.
pub fn reversal(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let cell = Cell::new(12, CONTROL_STAGE_12, "output_only_speaker", Route::Speaker, Stream::Playback, true);
    let s = settings(cell.config, rate);
    let stored = stored_tone(1000.0, 1.0);
    let volume = output::VOLUME_REFERENCE as f32;
    let (p, _) = positions(12, rate);
    let n = n_fade(12, rate);
    let half = n / 2;
    let sched = [change(&cell, p, false, 1), change(&cell, p + half as u64, true, 2)];
    let live = run_cell(make, s, &cell, &stored, &[], volume, &sched, Partition::Fixed(256));
    let all_on = run_cell(make, s, &cell, &stored, &[], volume, &[], Partition::Fixed(256));
    let twin = run_cell(make, twin_settings(&cell, rate), &cell, &stored, &[], volume, &[], Partition::Fixed(256));
    let p = p as usize;
    let mut worst = 0.0f64;
    let mut outside = 0usize;
    // k runs 1 … half, then half − 1 … 1, the sample at k = 0 selected.
    for j in 0..(2 * half - 1) {
        let k = if j < half { j + 1 } else { 2 * half - 1 - j };
        let i = p + j;
        let (y, x) = (f64::from(all_on.out[i]), f64::from(twin.out[i]));
        let want = y + weight(k, n) * (x - y);
        worst = worst.max(ulps_at(f64::from(live.out[i]) - want, y.abs().max(x.abs())));
        let (lo, hi) = (y.min(x), y.max(x));
        let v = f64::from(live.out[i]);
        let spacing = hi.abs().max(lo.abs()).max(f64::from(f32::MIN_POSITIVE)) * f64::from(f32::EPSILON);
        if v < lo - spacing || v > hi + spacing {
            outside += 1;
        }
    }
    let from = p + 2 * half - 1;
    let mism = (from..all_on.out.len().min(live.out.len()))
        .filter(|&i| live.out[i].to_bits() != all_on.out[i].to_bits())
        .count();
    let step = run_cell(&stepped, s, &cell, &stored, &[], volume, &[change(&cell, p as u64, false, 1)], Partition::Fixed(256));
    let e_step = click_energy_band(&step.out, rate, p as f64, 0.04, &[1000.0], 2750.0);
    let e_rev = click_energy_band(&live.out, rate, p as f64 + half as f64, 0.04, &[1000.0], 2750.0);
    let stim = format!("stage 12 speaker: bypass at {p}, reversed at {}", p + half);
    vec![
        MeasurementResult::new(
            "FR-005",
            "reversal follows the reversed curve (worst, f32 LSBs)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            worst,
            "LSB",
            Tolerance::AtMost(2.0), // 021 FR-005
        ),
        MeasurementResult::new(
            "FR-005",
            "samples outside the branches' interval by more than one LSB",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            outside as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-005
        ),
        MeasurementResult::new(
            "FR-005",
            "bit-identical to the all-on run from P + N - 1 (mismatches)",
            T_BYPASS,
            rate,
            cell.config,
            stim.clone(),
            mism as f64,
            "",
            Tolerance::Exact(0.0), // 021 FR-005 (the return in N/2)
        ),
        MeasurementResult::new(
            "FR-007",
            "the reversal's click energy below a step's (dB)",
            T_BYPASS,
            rate,
            cell.config,
            stim,
            0.5 * db(e_step / e_rev.max(1e-300)),
            "dB",
            Tolerance::Report,
        ),
    ]
}

// ---- Stage 10's audibility at 48 kHz (D1, hardware's evidence gap) ---------------------------------

/// Reported: the third-octave level difference of stage 10 on against off on the voiced stimulus
/// at 48 kHz (`playback_only`), the largest band difference in dB.
pub fn stage10_third_octave_48k(make: Make<'_>) -> Vec<MeasurementResult> {
    let rate = 48_000;
    let x = stimulus::voiced(6000.0, 1.5, -20.0, 0x021);
    let cell = Cell::new(10, CONTROL_STAGE_10, "playback_only", Route::Speaker, Stream::Playback, true);
    let on = run_cell(make, settings(cell.config, rate), &cell, &x, &[], 0.5, &[], Partition::Fixed(4096));
    let off = run_cell(make, twin_settings(&cell, rate), &cell, &x, &[], 0.5, &[], Partition::Fixed(4096));
    let a = analysis::third_octave_levels(&on.out, f64::from(rate));
    let b = analysis::third_octave_levels(&off.out, f64::from(rate));
    let worst = a.iter().zip(&b).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max);
    vec![MeasurementResult::new(
        "FR-011",
        "stage 10 on vs off at 48 kHz: largest third-octave difference (dB)",
        T_BYPASS,
        rate,
        "playback_only",
        String::from("voiced -20 dBFS"),
        worst,
        "dB",
        Tolerance::Report,
    )]
}

/// The entries of `log` for `control` (re-exported for the tests).
pub fn entries_of(log: &[ChangeEntry], control: u32) -> Vec<ChangeEntry> {
    cc::entries_of(log, control)
}
