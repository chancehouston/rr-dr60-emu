//! Spec 024's checks (research R-07): the oracle configuration, the harness's own reference
//! section, the response reader against the design script's table, the placement identity, the
//! transition trace, the click rows, the B1 row, the loud row and the meters' limiter-input peak.

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use crate::analysis::{self, db};
use crate::band_filter_ref::{REF_CASES, REF_ROWS, RefCase};
use crate::configs;
use crate::stimulus;
use rr_dr60::__test_hooks::{self as hooks, InsertMutation};
use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{ChangeEntry, Pipeline, Settings, Stream, Tap, VasEvent};
use rr_dr60_detmath as dm;

/// The band filter's default Q, the spec's literal (024 D4).
pub const DEFAULT_Q: f32 = 0.7071;

/// The band filter's control indices (type, frequency, Q, gain).
pub const CONTROLS: [u32; 4] = [32, 33, 34, 35];
/// The applied ceiling's fraction of the rate (spec 024 D4).
pub const CEILING: f64 = 0.45;

/// The oracle configuration (research R-07): every stage bypassed (stages 1 – 2 and the
/// limiter included), the tap after the slot.
pub fn oracle_settings(rate: u32) -> Settings {
    let mut s = configs::settings("bypass_all", rate);
    s.tap = Tap::AfterInsert;
    s
}

/// The default engine read at the tap after the slot (the placement identity's configuration):
/// stages 1 – 2 on with the hiss, the codec off.
pub fn placement_settings(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.tap = Tap::AfterInsert;
    s
}

/// The production path's configuration (hardware post-implementation B1): the default engine
/// with the codec off and the limiter bypassed, read at the tap after the limiter, so the slot
/// runs in `front_from` (the path every recording takes) and the tap is its output.
pub fn production_settings(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.agc.enabled = false;
    s.tap = Tap::AfterAgc;
    s
}

/// An engine with `s`, the band filter in the slot at `values` (type, frequency, Q, gain).
pub fn engine(s: Settings, values: [f32; 4]) -> Pipeline {
    let mut p = Pipeline::new(s).expect("valid settings");
    for (i, v) in values.iter().enumerate() {
        p.set_component_parameter(BeforeLimiter, i as u32, f64::from(*v))
            .expect("a parameter in range");
    }
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .expect("idle");
    p
}

/// The one-pass output of `x` in blocks of `block`.
pub fn run(p: &mut Pipeline, x: &[f32], block: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; x.len()];
    let mut pos = 0;
    while pos < x.len() {
        let n = block.min(x.len() - pos);
        let info = p
            .process(&x[pos..pos + n], &mut out[pos..pos + n])
            .expect("process");
        assert_eq!(
            info.produced, n,
            "a front tap produces one sample per input"
        );
        pos += n;
    }
    out
}

/// The harness's own section (research R-02): the engine's equations in the engine's order,
/// `detmath` only, the state flushed below 1e-30; at rest (no transition).
pub fn reference(rate: u32, values: [f32; 4], x: &[f64]) -> Vec<f64> {
    let r = f64::from(rate);
    let f = f64::from(values[1]).min(CEILING * r);
    let q = f64::from(values[2]);
    let w = core::f64::consts::PI * f / r;
    let g = dm::sin(w) / dm::cos(w);
    let k = 1.0 / q;
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    let gain = dm::exp(f64::from(values[3]) * (core::f64::consts::LN_10 / 20.0));
    let m: [f64; 3] = match values[0] as u8 {
        0 => [0.0, 0.0, 1.0],
        1 => [1.0, -1.0, -1.0],
        2 => [0.0, 1.0, 0.0],
        _ => [1.0, -1.0, 0.0],
    };
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    x.iter()
        .map(|&v| {
            let v3 = v - s2;
            let v1 = a1 * s1 + a2 * v3;
            let v2 = s2 + a2 * s1 + a3 * v3;
            s1 = 2.0 * v1 - s1;
            s2 = 2.0 * v2 - s2;
            if s1.abs() < 1e-30 && s2.abs() < 1e-30 {
                s1 = 0.0;
                s2 = 0.0;
            }
            let b = k * v1;
            gain * (m[0] * v + m[1] * b + m[2] * v2)
        })
        .collect()
}

/// One response row: the worst relative magnitude error and phase error against the design
/// script's table, the DC and Nyquist magnitudes in dB.
#[derive(Clone, Copy, Debug)]
pub struct ResponseRow {
    /// The case (set, type, rate).
    pub case: RefCase,
    /// max |H − H_ref| / max(0.0115·|H_ref|, 1e-6) over the oracle frequencies: at most 1
    /// passes research R-07's tolerance (0.1 dB, an absolute 1e-6 floor near a zero).
    pub rel_err: f64,
    /// The worst phase error in degrees where |H_ref| > −80 dB.
    pub phase_err_deg: f64,
    /// |H(DC)| and |H(Nyquist)| in dB at the tap.
    pub dc_db: f64,
    /// |H(Nyquist)| in dB.
    pub nyquist_db: f64,
}

/// The impulse response of the oracle configuration for a case (research R-07: the pole-radius
/// length).
pub fn impulse_response(c: &RefCase) -> Vec<f32> {
    let mut p = engine(
        oracle_settings(c.rate),
        [f32::from(c.ftype), c.freq, c.q, c.gain_db],
    );
    let x = stimulus::impulse(c.length);
    run(&mut p, &x, 4096)
}

/// The response row of a case.
pub fn response_row(c: &RefCase) -> ResponseRow {
    let ir = impulse_response(c);
    let fs = f64::from(c.rate);
    let mut rel_err = 0.0f64;
    let mut phase_err = 0.0f64;
    for &(f, re, im) in &REF_ROWS[c.first..c.first + c.count] {
        let h = analysis::dtft(&ir, f, fs);
        let href = analysis::Complex64::new(re, im);
        let e = (h - href).norm() / (0.0115 * href.norm()).max(1e-6);
        rel_err = rel_err.max(e);
        if href.norm() > 1e-4 {
            let d = (h / href).arg().to_degrees().abs();
            phase_err = phase_err.max(d);
        }
    }
    let dc = analysis::dtft(&ir, 0.0, fs).norm();
    let nyq = analysis::dtft(&ir, fs / 2.0, fs).norm();
    ResponseRow {
        case: *c,
        rel_err,
        phase_err_deg: phase_err,
        dc_db: db(dc.max(1e-300)),
        nyquist_db: db(nyq.max(1e-300)),
    }
}

/// Every case of the design script's table at `rates`.
pub fn response_rows(rates: &[u32]) -> Vec<ResponseRow> {
    REF_CASES
        .iter()
        .filter(|c| rates.contains(&c.rate))
        .map(response_row)
        .collect()
}

/// The placement identity (research R-07; hardware plan review B1): in the default engine the
/// tap after the slot equals the harness section applied to the slot's traced f64 input,
/// narrowed, bit for bit. Returns (samples compared, mismatches).
pub fn placement_identity(rate: u32, values: [f32; 4]) -> (usize, usize) {
    let (n, a) = placement_identity_in(placement_settings(rate), values);
    let (_, b) = placement_identity_in(production_settings(rate), values);
    (n, a + b)
}

/// The placement identity in configuration `s` (the tap branch or the production path).
pub fn placement_identity_in(s: Settings, values: [f32; 4]) -> (usize, usize) {
    let rate = s.host_rate_hz;
    let mut p = engine(s, values);
    hooks::insert_tracing(&mut p, Stream::OnePass, true);
    let n = hooks::INSERT_TRACE_CAPACITY;
    let x = stimulus::tone(700.0, 0.4, f64::from(rate), n);
    let y = run(&mut p, &x, 512);
    let trace = hooks::insert_input_trace(&p, Stream::OnePass).to_vec();
    assert_eq!(trace.len(), n);
    let want = reference(rate, values, &trace);
    let mut bad = y
        .iter()
        .zip(&want)
        .filter(|(a, b)| a.to_bits() != (**b as f32).to_bits())
        .count();
    // The place (coach post-implementation 3): the slot's input, narrowed, equals the empty
    // twin's `AfterMicAmp` output with the same seed and counter, so the slot sits after stage 2.
    let mut twin_s = placement_settings(rate);
    twin_s.tap = Tap::AfterMicAmp;
    let mut twin = Pipeline::new(twin_s).expect("valid settings");
    let amp = run(&mut twin, &x, 512);
    bad += trace
        .iter()
        .zip(&amp)
        .filter(|(a, b)| (**a as f32).to_bits() != b.to_bits())
        .count();
    (n, bad)
}

/// A one-pass run in the oracle configuration with one change scheduled at `at` (control index
/// and value), the band filter at `values`, `mutation` applied; `x` the stimulus.
pub fn change_run(
    rate: u32,
    values: [f32; 4],
    x: &[f32],
    at: u64,
    change: (u32, f64),
    mutation: InsertMutation,
    block: usize,
) -> (Vec<f32>, Vec<ChangeEntry>) {
    let mut p = engine(oracle_settings(rate), values);
    hooks::mutate_insert(&mut p, mutation);
    p.schedule_change(Stream::OnePass, at, change.0, change.1, 9)
        .expect("schedule");
    let mut out = vec![0.0f32; x.len()];
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut ev = vec![VasEvent::default(); 64];
    let mut entries = Vec::new();
    let mut pos = 0;
    while pos < x.len() {
        let n = block.min(x.len() - pos);
        let info = p
            .process_with_events_and_changes(
                &x[pos..pos + n],
                &mut out[pos..pos + n],
                &mut ev,
                &mut log,
            )
            .expect("process");
        entries.extend_from_slice(&log[..info.changes]);
        pos += n;
    }
    (out, entries)
}

/// The transition's applied trajectory (research R-02) on the one-pass, traced from P: the
/// applied (frequency, Q, gain dB) per sample.
pub fn trajectory(rate: u32, values: [f32; 4], change: (u32, f64)) -> Vec<(f64, f64, f64)> {
    let mut p = engine(oracle_settings(rate), values);
    let n = hooks::INSERT_TRACE_CAPACITY.min(rate as usize / 10);
    let at = TRAJECTORY_AT;
    let x = stimulus::tone(1000.0, 0.1, f64::from(rate), at + n);
    // Not at 0: a landing on a stream's first input sample applies at once (spec 017's rule).
    p.schedule_change(Stream::OnePass, at as u64, change.0, change.1, 1)
        .expect("schedule");
    let _ = run(&mut p, &x[..at], 64);
    hooks::insert_tracing(&mut p, Stream::OnePass, true);
    let _ = run(&mut p, &x[at..], 64);
    hooks::insert_trace(&p, Stream::OnePass).to_vec()
}

/// The input position the trajectory's change lands at (the trace starts there).
pub const TRAJECTORY_AT: usize = 100;

/// The smoothstep the spec states (D3).
pub fn smoothstep(u: f64) -> f64 {
    u * u * (3.0 - 2.0 * u)
}

/// One click row (spec US1 AS2; research R-07): the transition's click energy below the
/// `param_step` mutant's at the same P, in dB, with spec 011's reader (0.5 s Hann, 20 – 2750 Hz,
/// the 1 kHz tone ± 250 Hz excluded), and the floor: a held run's energy below the step's.
#[derive(Clone, Copy, Debug)]
pub struct ClickRow {
    /// The rate.
    pub rate: u32,
    /// The starting values and the change.
    pub values: [f32; 4],
    /// The change (control, value).
    pub change: (u32, f64),
    /// The reduction against the step, in dB.
    pub reduction_db: f64,
    /// The held run's energy below the step's, in dB (the reader's floor).
    pub floor_db: f64,
}

/// A click row on a 1 kHz tone at −20 dBFS, the change at 0.5 s.
pub fn click_row(rate: u32, values: [f32; 4], change: (u32, f64)) -> ClickRow {
    let fs = f64::from(rate);
    let len = (1.5 * fs) as usize;
    let x = stimulus::tone(1000.0, 0.1, fs, len);
    let at = (0.5 * fs) as u64;
    let n = (0.02 * fs).round();
    let (smooth, _) = change_run(
        rate,
        values,
        &x,
        at,
        change,
        InsertMutation::default(),
        1024,
    );
    let step_m = if change.0 == CONTROLS[0] {
        InsertMutation {
            type_instant: true,
            ..InsertMutation::default()
        }
    } else {
        InsertMutation {
            param_step: true,
            ..InsertMutation::default()
        }
    };
    let (step, _) = change_run(rate, values, &x, at, change, step_m, 1024);
    // Spec 011's reader on the outputs themselves (the hardware plan review's simulation): the
    // 1 kHz tone is excluded, so what is left is the change's splatter; the held run's energy
    // at the same centre is the reader's floor (the tone's own leakage).
    let held = {
        let mut p = engine(oracle_settings(rate), values);
        run(&mut p, &x, 1024)
    };
    // The floor is the tone's leakage at its larger level, before or after the change (hardware
    // post-implementation B8: a held notch removes the tone, so its own floor is degenerate).
    let held_after = {
        let mut v = values;
        let i = CONTROLS.iter().position(|&c| c == change.0).unwrap();
        v[i] = change.1 as f32;
        let mut p = engine(oracle_settings(rate), v);
        run(&mut p, &x, 1024)
    };
    let e_smooth = analysis::click_energy(&smooth, rate, at as f64 + n / 2.0);
    let e_step = analysis::click_energy(&step, rate, at as f64);
    let e_floor = analysis::click_energy(&held, rate, at as f64).max(analysis::click_energy(
        &held_after,
        rate,
        at as f64,
    ));
    ClickRow {
        rate,
        values,
        change,
        reduction_db: db(e_step / e_smooth.max(1e-300)) / 2.0,
        floor_db: db(e_step / e_floor.max(1e-300)) / 2.0,
    }
}

/// The B1 row (research R-07; hardware plan review B10): the 48 kHz section moved 20 kHz (the
/// published range's top; the review's 21.6 kHz is above it) →
/// 20 Hz over 20 ms with a 100 Hz tone at Q 0.7071, the peak of |y| over the move plus 50 ms
/// divided by (amplitude × the largest frozen |H| at the tone along the trajectory), in dB, for
/// the engine's state-variable form and for a transposed direct form II with the same
/// per-sample coefficients. Returns (state-variable dB, TDF-II dB).
pub fn b1_row(ftype: u8) -> (f64, f64) {
    let rate = 48_000u32;
    let fs = f64::from(rate);
    let amp = 0.1;
    let n = (0.02 * fs) as usize;
    let len = n + (0.05 * fs) as usize;
    let pre = (0.2 * fs) as usize;
    let x = stimulus::tone(100.0, amp, fs, pre + len);
    let start = [f32::from(ftype), 20_000.0, 0.7071, 0.0];
    let (y, _) = change_run(
        rate,
        start,
        &x,
        pre as u64,
        (CONTROLS[1], 20.0),
        InsertMutation::default(),
        256,
    );
    let peak_svf = y[pre..]
        .iter()
        .map(|v| f64::from(v.abs()))
        .fold(0.0, f64::max);
    // The frozen responses along the trajectory, and the TDF-II run.
    let (l0, l1) = (dm::ln(20_000.0), dm::ln(20.0));
    let k = 1.0 / f64::from(0.7071f32);
    let coeffs = |f: f64| {
        let w = core::f64::consts::PI * f / fs;
        let g = dm::sin(w) / dm::cos(w);
        let d = 1.0 + g * k + g * g;
        let (b0, b1, b2) = match ftype {
            0 => (g * g, 2.0 * g * g, g * g),
            1 => (1.0, -2.0, 1.0),
            2 => (k * g, 0.0, -k * g),
            _ => (1.0 + g * g, 2.0 * g * g - 2.0, 1.0 + g * g),
        };
        (
            b0 / d,
            b1 / d,
            b2 / d,
            (2.0 * g * g - 2.0) / d,
            (1.0 - g * k + g * g) / d,
        )
    };
    let mut hmax = 0.0f64;
    let (mut z1, mut z2) = (0.0f64, 0.0f64);
    let mut peak_tdf = 0.0f64;
    for (i, &xv) in x.iter().enumerate() {
        let f = if i < pre {
            20_000.0
        } else {
            let m = (i - pre + 1).min(n);
            dm::exp(l0 + smoothstep(m as f64 / n as f64) * (l1 - l0))
        };
        let (b0, b1, b2, a1, a2) = coeffs(f);
        let xv = f64::from(xv);
        let yv = b0 * xv + z1;
        z1 = b1 * xv - a1 * yv + z2;
        z2 = b2 * xv - a2 * yv;
        if i >= pre {
            peak_tdf = peak_tdf.max(yv.abs());
            let wt = core::f64::consts::TAU * 100.0 / fs;
            let zc = analysis::Complex64::from_polar(1.0, -wt);
            let h = (b0 + b1 * zc + b2 * zc * zc) / (1.0 + a1 * zc + a2 * zc * zc);
            hmax = hmax.max(h.norm());
        }
    }
    let norm = amp * hmax;
    (db(peak_svf / norm), db(peak_tdf / norm))
}

/// The loud row (spec US1 AS5; research R-07): the default record front with the filter at
/// +24 dB on a −6 dBFS 1 kHz tone, against the empty-slot twin: (gain reduction dB, clamp
/// count) with the filter, and the twin's.
pub fn loud_row(rate: u32) -> ((f32, u32), (f32, u32), u32, u32) {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.tap = Tap::AfterRecord;
    let x = stimulus::tone(1000.0, 0.5, f64::from(rate), rate as usize / 2);
    let read = |p: &mut Pipeline| {
        let _ = run(p, &x, 2048);
        let m = p.meters();
        (m.gain_reduction_db, m.adc_clamp_count)
    };
    let mut with = engine(s, [2.0, 1000.0, 0.7071, 24.0]);
    let mut twin = Pipeline::new(s).unwrap();
    // The limiter holds the signal below full scale, so the converter clamp counts with the
    // limiter bypassed.
    let mut no_limiter = s;
    no_limiter.agc.enabled = false;
    let mut bare = engine(no_limiter, [2.0, 1000.0, 0.7071, 24.0]);
    // Its empty-slot twin (hardware post-implementation B7): the limiter bypassed, no filter.
    let mut bare_twin = Pipeline::new(no_limiter).unwrap();
    (
        read(&mut with),
        read(&mut twin),
        read(&mut bare).1,
        read(&mut bare_twin).1,
    )
}

/// Spec 024 D6: the meters' limiter-input peak equals the peak of the slot's output (read at
/// the tap after the slot in a second engine with the same seed), per call. Returns the largest
/// difference.
pub fn limiter_input_peak_row(rate: u32) -> f64 {
    let values = [0.0, 2000.0, 3.0, 6.0];
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let x = stimulus::tone(1500.0, 0.2, f64::from(rate), 4096);
    let mut metered = engine(s, values);
    let mut tapped = engine(placement_settings(rate), values);
    let mut worst = 0.0f64;
    for block in x.chunks(1024) {
        let mut out = vec![0.0f32; block.len()];
        let _ = metered.process(block, &mut out).unwrap();
        let mut t = vec![0.0f32; block.len()];
        let _ = tapped.process(block, &mut t).unwrap();
        let peak = t.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        worst = worst.max(f64::from(
            (metered.meters().limiter_input_peak - peak).abs(),
        ));
    }
    worst
}

// ---- The golden (research R-08) ------------------------------------------------------------

/// One entry of `golden-insert-v1.json`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InsertEntry {
    /// What the entry runs (the group and its parameters).
    pub name: String,
    /// The host rate.
    pub host_rate_hz: u32,
    /// Samples (one-pass outputs) or stored device samples (recordings).
    pub n: u64,
    /// FNV-1a 64 of the output's bits, or the recording's digest, as hex.
    pub digest: String,
    /// Change entries the run logged.
    pub changes: u64,
}

/// The insert golden file.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InsertGoldenFile {
    /// `rr_dr60-golden-insert`.
    pub format: String,
    /// The entries, in generation order.
    pub entries: Vec<InsertEntry>,
}

/// The committed golden (embedded: the device golden check runs this binary on a phone).
pub fn committed_golden() -> InsertGoldenFile {
    serde_json::from_str(include_str!("../golden/golden-insert-v1.json"))
        .expect("valid embedded insert golden file")
}

/// The golden's path for a bless.
pub fn golden_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-insert-v1.json")
}

const VOICE_SEED: u64 = 24;

/// A recording through the default engine (codec on) with the filter at `values`, an optional
/// record-stream change at 0.3 s, `count` recordings in a row; the entry of the last.
fn record_entry(
    name: &str,
    rate: u32,
    values: [f32; 4],
    change: Option<(u32, f64)>,
    count: usize,
) -> InsertEntry {
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 1.0, -20.0, VOICE_SEED);
    let mut p = engine(Settings::new(rate), values);
    let mut changes = 0u64;
    for _ in 0..count {
        if let Some((control, value)) = change {
            p.schedule_change(Stream::Record, (0.3 * fs) as u64, control, value, 11)
                .expect("schedule");
        }
        p.start_recording().expect("start");
        let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        for block in x.chunks(1024) {
            let mut ev = vec![VasEvent::default(); p.max_record_events(block.len())];
            let info = p
                .record_with_changes(block, &mut ev, &mut log)
                .expect("record");
            changes += info.changes as u64;
        }
        let mut ev = vec![VasEvent::default(); p.max_record_events(0).max(64)];
        let _ = p.stop_recording(&mut ev).expect("stop");
    }
    // The recording played back and its output digested: the stored form's digest covers its
    // header, which carries the library version (spec 021's lesson), so the golden reads the
    // played samples instead.
    let number = p.memory_status().count;
    p.start_playback(number).expect("play");
    let mut out = Vec::new();
    loop {
        let mut y = vec![0.0f32; 4096];
        let info = p.play(&mut y).expect("play");
        out.extend_from_slice(&y[..info.produced]);
        if info.finished {
            break;
        }
    }
    InsertEntry {
        name: name.into(),
        host_rate_hz: rate,
        n: p.recording_length(number).expect("length"),
        digest: format!("{:016x}", crate::changes_checks::digest(&out)),
        changes,
    }
}

/// A one-pass run of `x` with the filter at `values` in `s`, an optional one-pass change.
fn one_pass_entry(
    name: &str,
    s: Settings,
    values: [f32; 4],
    x: &[f32],
    change: Option<(u64, u32, f64)>,
) -> InsertEntry {
    let rate = s.host_rate_hz;
    let mut p = engine(s, values);
    if let Some((at, control, value)) = change {
        p.schedule_change(Stream::OnePass, at, control, value, 12)
            .expect("schedule");
    }
    let mut out = Vec::with_capacity(x.len());
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut changes = 0u64;
    for block in x.chunks(1024) {
        let mut y = vec![0.0f32; block.len()];
        let mut ev = vec![VasEvent::default(); p.max_events(block.len())];
        let info = p
            .process_with_events_and_changes(block, &mut y, &mut ev, &mut log)
            .expect("process");
        out.extend_from_slice(&y[..info.produced]);
        changes += info.changes as u64;
    }
    InsertEntry {
        name: name.into(),
        host_rate_hz: rate,
        n: out.len() as u64,
        digest: format!("{:016x}", crate::changes_checks::digest(&out)),
        changes,
    }
}

/// Generates the 84 entries of research R-08.
pub fn generate_golden() -> InsertGoldenFile {
    let rates = configs::all_rates();
    let mut e = Vec::new();
    for &rate in &rates {
        for t in 0..4u8 {
            e.push(record_entry(
                &format!("record_type_{t}"),
                rate,
                [f32::from(t), 1000.0, 0.7071, 0.0],
                None,
                1,
            ));
        }
    }
    for &rate in &rates {
        for (f, q) in [(300.0f32, 0.5f32), (1000.0, 3.0), (3000.0, 10.0)] {
            e.push(record_entry(
                &format!("record_low_pass_{f}_{q}"),
                rate,
                [0.0, f, q, 0.0],
                None,
                1,
            ));
        }
    }
    for &rate in &rates {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, VOICE_SEED);
        e.push(one_pass_entry(
            "one_pass_default",
            Settings::new(rate),
            [0.0, 1000.0, 0.7071, 0.0],
            &x,
            None,
        ));
    }
    for &rate in &rates {
        e.push(record_entry(
            "schedule_frequency",
            rate,
            [0.0, 1000.0, 0.7071, 0.0],
            Some((CONTROLS[1], 3000.0)),
            1,
        ));
        e.push(record_entry(
            "schedule_type",
            rate,
            [0.0, 1000.0, 0.7071, 0.0],
            Some((CONTROLS[0], 1.0)),
            1,
        ));
    }
    for rate in [6000u32, 48_000] {
        let x = stimulus::tone(1000.0, 0.25, f64::from(rate), rate as usize / 2);
        for t in 0..4u8 {
            e.push(one_pass_entry(
                &format!("tone_type_{t}"),
                oracle_settings(rate),
                [f32::from(t), 1000.0, 2.0, 0.0],
                &x,
                None,
            ));
        }
    }
    for rate in [6000u32, 44_100, 48_000, 96_000] {
        e.push(record_entry(
            "second_recording",
            rate,
            [1.0, 700.0, 1.5, 3.0],
            None,
            2,
        ));
    }
    for rate in [6000u32, 48_000] {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, VOICE_SEED);
        e.push(one_pass_entry(
            "set_then_schedule",
            placement_settings(rate),
            [2.0, 900.0, 2.0, 6.0],
            &x,
            Some((u64::from(rate) / 5, CONTROLS[3], -6.0)),
        ));
    }
    InsertGoldenFile {
        format: "rr_dr60-golden-insert".into(),
        entries: e,
    }
}

/// The golden's empty-slot twin (D10; coach post-implementation 4): the filter set then the
/// slot emptied gives the same one-pass output and the same recording as an engine that never
/// had it, at `rate`. Returns whether both hold.
pub fn empty_twin_holds(rate: u32) -> bool {
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 0.5, -20.0, VOICE_SEED);
    let s = Settings::new(rate);
    let mut emptied = engine(s, [2.0, 800.0, 3.0, 6.0]);
    emptied
        .set_component(BeforeLimiter, ComponentId::None)
        .expect("idle");
    let mut never = Pipeline::new(s).expect("valid settings");
    let one_pass = run(&mut emptied, &x, 1024) == run(&mut never, &x, 1024);
    let record = |p: &mut Pipeline| {
        p.reset();
        p.start_recording().expect("start");
        let mut ev = vec![VasEvent::default(); p.max_record_events(x.len()).max(64)];
        let _ = p.record(&x, &mut ev).expect("record");
        let mut ev = vec![VasEvent::default(); p.max_record_events(0).max(64)];
        let _ = p.stop_recording(&mut ev).expect("stop");
        let n = p.memory_status().count;
        p.recording_digest(n).expect("digest")
    };
    one_pass && record(&mut emptied) == record(&mut never)
}

/// Whether a quiet recording (the capsule's hiss only, silence in) pauses with the filter at
/// `values` and the sensitivity `level`, over `seconds` at `rate` (US1 AS5; spec 012's pattern).
pub fn quiet_pauses(rate: u32, level: u8, values: [f32; 4], seconds: f64) -> bool {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.vas.sensitivity = level;
    s.memory.capacity_s = 60;
    let mut p = engine(s, values);
    let x = vec![0.0f32; (seconds * f64::from(rate)) as usize];
    p.start_recording().expect("start");
    for block in x.chunks(4096) {
        let mut ev = vec![VasEvent::default(); p.max_record_events(block.len())];
        let _ = p.record(block, &mut ev).expect("record");
    }
    let mut ev = vec![VasEvent::default(); p.max_record_events(0).max(64)];
    let _ = p.stop_recording(&mut ev).expect("stop");
    // Drop mode reports a pause at its resume, so a pause that never ends has no event: the
    // stored length tells (a recording that paused keeps less than 90 % of the input).
    let n = p.memory_status().count;
    if n == 0 {
        return true;
    }
    let stored = p.recording_length(n).expect("length");
    (stored as f64) < 0.9 * seconds * 6000.0
}

/// The gain at which a quiet recording stops pausing, by bisection to 0.25 dB over `seconds`
/// (a lower estimate: a longer run can still pause; spec 012's lesson); `None` when it still
/// pauses at +24 dB.
pub fn gate_stop_gain(rate: u32, level: u8, ftype: u8, q: f32, seconds: f64) -> Option<f64> {
    let at = |g: f64| {
        quiet_pauses(
            rate,
            level,
            [f32::from(ftype), 1000.0, q, g as f32],
            seconds,
        )
    };
    if at(24.0) {
        return None;
    }
    let (mut lo, mut hi) = (-24.0f64, 24.0f64);
    if !at(lo) {
        return Some(lo);
    }
    while hi - lo > 0.25 {
        let mid = 0.5 * (lo + hi);
        if at(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(hi)
}
