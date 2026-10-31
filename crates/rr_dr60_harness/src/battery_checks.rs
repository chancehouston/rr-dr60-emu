//! Spec 018 battery checks (research R-07): one function per measurable property of the
//! static law that scales the speaker amplifier's clip ceiling with the cell pair's voltage
//! (A-056; stage 11, A-031).
//!
//! The law is read on **stage 11 alone** (`stage11_only_speaker`: every other stage bypassed,
//! the tap after stage 11) on a 1 kHz tone 6 dB over the nominal ceiling
//! (`VOLUME_6DB_OVER`): the tone is hard-clipped at every battery value, so the **sample peak**
//! of the settled output equals the ceiling factor `b` to the bit (the ceiling branch returns
//! `b` itself; the AES17 amplitude of a clipped sine reads about 1.25·b and is reported, never
//! asserted; coach 2, hardware B1). The below-knee identity runs the whole default engine
//! (stages 1 – 2 on) at the reference position. Transitions are read through the
//! `battery_factor` hook at block size 1 and as the splice of two constant runs (spec 011's
//! oracles). Every tolerance is an engineering target of spec 018 FR-002 – FR-006 and bounds
//! the model, not the device.

use rr_dr60::__test_hooks;
use rr_dr60::battery::{ALKALINE_TRAJECTORY, ceiling_factor};
use rr_dr60::output::{VOLUME_6DB_OVER, VOLUME_REFERENCE, volume_transition_samples};
use rr_dr60::{ChangeEntry, Meters, Route, Settings, Stream, VasEvent};

use crate::analysis::{self, db, thd_n_db};
use crate::changes_checks::{self as cc, Change, Partition, Source};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, output_checks, stimulus};

/// Trace: the battery law on stage 11.
pub const T_BATT: &[&str] = &["A-056", "A-031", "engineering target"];

/// The six law points of spec 018 D2 (the default first): fresh, the service manual's V+
/// mark as a pair, and three tired pairs down to the lower bound.
pub fn law_points() -> [f32; 5] {
    [3.0, 2.8, 2.6, 2.4, 2.2]
}

/// A-056's alkaline reference trajectory, `(hours of recording, volts)`.
pub fn trajectory() -> [(f32, f32); 7] {
    ALKALINE_TRAJECTORY
}

/// The register's rounded ceilings for the seven rows, dB re the nominal (A-056).
pub const REGISTER_DB: [f64; 7] = [-0.3, -1.9, -2.9, -4.4, -5.7, -7.3, -9.1];

/// A 1 kHz tone at 0 dBFS from the host grid's peak phase, 1.5 s: hard-clipped at every
/// battery value when run at [`VOLUME_6DB_OVER`].
pub fn clipped_tone(rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone_at_phase(
        1000.0,
        0.0,
        stimulus::peak_phase(1000.0, rate),
        fs,
        (1.5 * fs) as usize,
    )
}

/// Stage 11 alone at the tap after it (spec 011's configuration), the speaker route.
pub fn stage11_only(rate: u32) -> Settings {
    configs::settings("stage11_only_speaker", rate)
}

/// The whole default engine (stages 1 – 2 on, the codec on; `Settings::new`).
pub fn whole_default(rate: u32) -> Settings {
    Settings::new(rate)
}

/// Runs `x` through `settings` at `route`, `volume` and the battery value `volts`, one block;
/// the produced output.
pub fn run_at(
    make: Make<'_>,
    settings: Settings,
    route: Route,
    volume: f32,
    volts: f32,
    x: &[f32],
) -> Vec<f32> {
    let mut p = make(settings);
    p.set_route(route);
    p.set_volume(volume).expect("a valid position");
    p.set_battery_volts(volts).expect("a valid battery value");
    let mut y = vec![0.0; x.len()];
    let n = p.process(x, &mut y).expect("equal lengths").produced;
    y.truncate(n);
    y
}

/// The sample peak of `y` over its last second (after the resampler pair's onset ringing).
pub fn peak_after_settle(y: &[f32], rate: u32) -> f64 {
    let start = y.len().saturating_sub(rate as usize);
    y[start..]
        .iter()
        .fold(0.0f64, |m, &v| m.max(f64::from(v).abs()))
}

/// The AES17 amplitude (dBFS) of `y` over its last second: about 1.25 – 1.36·b by host rate on
/// the clipped tone, reported only.
pub fn aes17_after_settle(y: &[f32], rate: u32) -> f64 {
    let start = y.len().saturating_sub(rate as usize);
    analysis::level_dbfs_aes17(&y[start..])
}

/// The two below-knee stimuli of FR-002, 1 s each: a −30 dBFS 1 kHz tone and the −20 dBFS
/// voiced stimulus.
pub fn below_knee_stimuli(rate: u32) -> Vec<(&'static str, Vec<f32>)> {
    let fs = f64::from(rate);
    vec![
        (
            "tone 1 kHz -30 dBFS",
            stimulus::tone_at_phase(
                1000.0,
                -30.0,
                stimulus::peak_phase(1000.0, rate),
                fs,
                rate as usize,
            ),
        ),
        ("voiced -20 dBFS", stimulus::voiced(fs, 1.0, -20.0, 0x018)),
    ]
}

fn pass(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// The battery entries of a log (the plain route and volume setters log their own entries at
/// the first produced sample, spec 015).
pub fn battery_entries(log: &[ChangeEntry]) -> Vec<ChangeEntry> {
    cc::entries_of(log, cc::control_index("battery_volts"))
}

/// THD+N in percent of a 1 kHz tone over whole cycles after `settle_s` (output_checks' reader).
fn thd_percent(y: &[f32], rate: u32, settle_s: f64) -> f64 {
    let fs = f64::from(rate);
    let start = (settle_s * fs) as usize;
    let cycles = ((y.len() - start) as f64 / fs * 1000.0).floor() as usize - 1;
    let len = (cycles as f64 * fs / 1000.0).round() as usize;
    100.0 * analysis::from_db(thd_n_db(y, 1000.0, fs, start, len))
}

// ---- FR-002, FR-006: the hard-clipped trajectory -----------------------------------------------

/// Spec 018 US1 AS3 (FR-002, FR-006): at each of the seven trajectory rows the settled sample
/// peak of the clipped tone equals `ceiling_factor(volts)` to the bit and its level in dB is
/// within 0.05 dB of the register's figure; the AES17 amplitude is reported.
pub fn trajectory_rows(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let mut rows = Vec::new();
    for (i, &(hours, volts)) in trajectory().iter().enumerate() {
        let y = run_at(make, settings, Route::Speaker, VOLUME_6DB_OVER, volts, &x);
        let peak = peak_after_settle(&y, rate);
        let b = ceiling_factor(volts);
        let stim = format!("tone 1 kHz 0 dBFS at +6 dB, {hours} h: {volts} V");
        rows.push(MeasurementResult::new(
            "FR-002",
            "settled sample peak equals the ceiling factor (bit for bit)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass((peak as f32).to_bits() == b.to_bits()),
            "",
            Tolerance::Exact(1.0), // 018 FR-002: the ceiling branch returns b itself
        ));
        rows.push(MeasurementResult::new(
            "FR-006",
            "ceiling re the nominal against the register's row",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            db(peak),
            "dB",
            Tolerance::Range(REGISTER_DB[i] - 0.05, REGISTER_DB[i] + 0.05), // 018 FR-006
        ));
        rows.push(MeasurementResult::new(
            "FR-002",
            "AES17 amplitude of the clipped tone (about 1.25 - 1.36 b; reported)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim,
            aes17_after_settle(&y, rate) - db(f64::from(b)),
            "dB re b",
            Tolerance::Report,
        ));
    }
    rows
}

/// Spec 018 US1 AS3 (coach review 5): the reference-position case, reported: a 0 dBFS tone at
/// `VOLUME_REFERENCE` sits at the nominal ceiling and is clipped at a tired battery; its settled
/// peak and AES17 amplitude at 2.2 V.
pub fn reference_position_rows(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let y = run_at(make, settings, Route::Speaker, VOLUME_REFERENCE, 2.2, &x);
    vec![
        MeasurementResult::new(
            "FR-002",
            "reference position at 2.2 V: settled sample peak (reported)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            "tone 1 kHz 0 dBFS at the reference position, 2.2 V",
            peak_after_settle(&y, rate),
            "",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-002",
            "reference position at 2.2 V: AES17 amplitude (reported)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            "tone 1 kHz 0 dBFS at the reference position, 2.2 V",
            aes17_after_settle(&y, rate),
            "dBFS",
            Tolerance::Report,
        ),
    ]
}

/// FR-002 at every law point: the settled peak equals `b` to the bit (the matrix's row).
pub fn law_point_peaks(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    law_points()
        .iter()
        .map(|&volts| {
            let y = run_at(make, settings, Route::Speaker, VOLUME_6DB_OVER, volts, &x);
            let peak = peak_after_settle(&y, rate);
            MeasurementResult::new(
                "FR-002",
                "settled sample peak equals the ceiling factor (bit for bit)",
                T_BATT,
                rate,
                "stage11_only_speaker",
                format!("tone 1 kHz 0 dBFS at +6 dB, {volts} V"),
                pass((peak as f32).to_bits() == ceiling_factor(volts).to_bits()),
                "",
                Tolerance::Exact(1.0), // 018 FR-002
            )
        })
        .collect()
}

// ---- FR-002: the below-knee identity -----------------------------------------------------------

/// Spec 018 US1 AS2 (FR-002): on the whole default engine at the reference position, the two
/// below-knee stimuli are bit-identical at every law point to the default run, the premise
/// asserted: the reconstruction peak stays under the knee's start at the lowest value.
pub fn below_knee_identity(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = whole_default(rate);
    let b_min = f64::from(ceiling_factor(law_points()[4]));
    let mut rows = Vec::new();
    for (name, x) in below_knee_stimuli(rate) {
        let mut p = make(settings);
        p.set_route(Route::Speaker);
        let mut reference = vec![0.0f32; x.len()];
        let n = p
            .process(&x, &mut reference)
            .expect("equal lengths")
            .produced;
        reference.truncate(n);
        let premise = f64::from(p.meters().reconstruction_peak);
        rows.push(MeasurementResult::new(
            "FR-002",
            "premise: reconstruction peak under the knee's start at 2.2 V (0.85 b)",
            T_BATT,
            rate,
            "default",
            name,
            premise,
            "",
            Tolerance::AtMost(0.85 * b_min - 1e-9), // 018 FR-002 (hardware B10)
        ));
        for volts in law_points() {
            let y = run_at(make, settings, Route::Speaker, VOLUME_REFERENCE, volts, &x);
            rows.push(MeasurementResult::new(
                "FR-002",
                "bit-identical to the default below the knee",
                T_BATT,
                rate,
                "default",
                format!("{name}, {volts} V"),
                pass(cc::same(&y, &reference)),
                "",
                Tolerance::Exact(1.0), // 018 FR-002: the law moves the ceiling only
            ));
        }
    }
    rows
}

// ---- FR-002: the knee table shifted by 20·log10(b) --------------------------------------------

/// Spec 009's knee levels re the nominal ceiling (its table's first column).
const KNEE_LEVELS_DB: [f64; 6] = [-6.0, 0.0, 1.0, 2.0, 3.0, 6.0];
/// Spec 009's 10 % THD point, dB over the ceiling (the design script's print, pinned by the
/// core's `KNEE_10PCT_DB`).
const KNEE_10PCT_DB: f64 = 2.27;

/// Spec 018 US1 AS3 (FR-002), 44.1 kHz and above: spec 009's knee rows re-measured at
/// `level + 20·log10(b)` give the default's peaks scaled by `b` (within 1e-6) and the same THD
/// (within 0.05 points), and the 10 % THD point sits at +2.27 dB + 20·log10(b) within 0.1 dB.
pub fn knee_table_shift(make: Make<'_>, rate: u32, volts: f32) -> Vec<MeasurementResult> {
    assert!(
        rate >= 44_100,
        "distortion is measured at 44.1 kHz and above (009 D8)"
    );
    let fs = f64::from(rate);
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let b = f64::from(ceiling_factor(volts));
    let shift = db(b);
    let at = |level_db: f64, v: f32| {
        run_at(
            make,
            settings,
            Route::Speaker,
            output_checks::position_over_ceiling(level_db, fs),
            v,
            &x,
        )
    };
    let mut rows = Vec::new();
    for level in KNEE_LEVELS_DB {
        let reference = at(level, 3.0);
        let shifted = at(level + shift, volts);
        let stim = format!("tone 1 kHz, {level:+.0} dB re the ceiling, {volts} V");
        rows.push(MeasurementResult::new(
            "FR-002",
            "settled peak equals the default's peak times b",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            peak_after_settle(&shifted, rate) - b * peak_after_settle(&reference, rate),
            "",
            Tolerance::Range(-1e-6, 1e-6), // 018 FR-002: the knee scales with the ceiling
        ));
        rows.push(MeasurementResult::new(
            "FR-002",
            "THD equals the default's at the shifted level",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim,
            thd_percent(&shifted, rate, 0.2) - thd_percent(&reference, rate, 0.2),
            "% points",
            Tolerance::Range(-0.05, 0.05), // 018 FR-002
        ));
    }
    // The 10 % crossing by bisection on the level between the shifted ceiling and 6 dB over.
    let (mut lo, mut hi) = (shift, shift + 6.0);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if thd_percent(&at(mid, volts), rate, 0.2) < 10.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    rows.push(MeasurementResult::new(
        "FR-002",
        "level re the nominal ceiling at which THD reaches 10 % (2.27 dB + 20 log10 b)",
        T_BATT,
        rate,
        "stage11_only_speaker",
        format!("bisection on the position, {volts} V"),
        0.5 * (lo + hi) - shift,
        "dB",
        Tolerance::Range(KNEE_10PCT_DB - 0.1, KNEE_10PCT_DB + 0.1), // 018 FR-002
    ));
    rows
}

// ---- FR-003: the transition ----------------------------------------------------------------------

/// The transition's length N at `rate` (20 ms; spec 011's N).
pub fn n_of(rate: u32) -> usize {
    volume_transition_samples(rate).expect("a supported rate") as usize
}

/// The smoothstep `3u² − 2u³` at transition sample `k` of `n`.
pub fn smoothstep(k: usize, n: usize) -> f64 {
    let u = k as f64 / n as f64;
    3.0 * u * u - 2.0 * u * u * u
}

/// The chain's ceiling factor after each of the first `len` samples following a plain
/// `set_battery_volts(to)` on a warmed stage-11-only engine at `from`, read through the
/// `battery_factor` hook at block size 1: entry `k − 1` is the factor in force on transition
/// sample `k`.
pub fn factor_trace(make: Make<'_>, rate: u32, from: f32, to: f32, len: usize) -> Vec<f64> {
    let mut p = make(stage11_only(rate));
    p.set_route(Route::Speaker);
    p.set_volume(VOLUME_6DB_OVER).expect("a valid position");
    p.set_battery_volts(from).expect("a valid battery value");
    let x = clipped_tone(rate);
    let warm = 2048.min(x.len() / 2);
    let mut y = vec![0.0f32; warm];
    let _ = p.process(&x[..warm], &mut y).expect("ok");
    p.set_battery_volts(to).expect("a valid battery value");
    (0..len)
        .map(|i| {
            let mut y = [0.0f32; 1];
            let _ = p.process(&x[warm + i..warm + i + 1], &mut y).expect("ok");
            __test_hooks::battery_factor(&p, Stream::OnePass).expect("the one-pass")
        })
        .collect()
}

/// Spec 018 US1 AS4 (FR-003): the factor trace of a 3.0 → `to` change: transition sample
/// ⌊N/4⌋ carries `b₀ + s(⌊N/4⌋ / N)·Δb` within 1e-12, sample N is exactly `b₁`, the sequence
/// is monotonic, and sample 1 has already moved.
pub fn trace_rows(make: Make<'_>, rate: u32, to: f32) -> Vec<MeasurementResult> {
    let n = n_of(rate);
    let (b0, b1) = (
        f64::from(ceiling_factor(3.0)),
        f64::from(ceiling_factor(to)),
    );
    let trace = factor_trace(make, rate, 3.0, to, n + 8);
    let q = n / 4;
    let want_q = b0 + (b1 - b0) * smoothstep(q, n);
    let reversals = trace[..n]
        .windows(2)
        .filter(|w| if b1 < b0 { w[1] > w[0] } else { w[1] < w[0] })
        .count();
    let stim = format!("3.0 -> {to} V on the clipped tone, N = {n}");
    vec![
        MeasurementResult::new(
            "FR-003",
            "factor at transition sample N/4 against the smoothstep",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            trace[q - 1] - want_q,
            "",
            Tolerance::Range(-1e-12, 1e-12), // 018 FR-003 (A-045's shape)
        ),
        MeasurementResult::new(
            "FR-003",
            "factor exactly b1 at transition sample N (bit for bit)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(trace[n - 1].to_bits() == b1.to_bits() && trace[n + 7].to_bits() == b1.to_bits()),
            "",
            Tolerance::Exact(1.0), // 018 FR-003
        ),
        MeasurementResult::new(
            "FR-003",
            "factor moved on transition sample 1",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(trace[0] != b0),
            "",
            Tolerance::Exact(1.0), // 018 FR-003
        ),
        MeasurementResult::new(
            "FR-003",
            "reversals of the factor through the transition",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim,
            reversals as f64,
            "",
            Tolerance::Exact(0.0), // 018 FR-003: monotonic by construction
        ),
    ]
}

/// The change position of the transition scenes: inside the second 1024-block, past the
/// resampler's onset ringing, on a host-grid peak of the 1 kHz tone (a change on a zero
/// crossing hides a hold; spec 017's lesson).
pub fn change_at(rate: u32) -> u64 {
    let period = f64::from(rate) / 1000.0;
    let k = (1500.0 / period).ceil();
    (k * period).round() as u64
}

/// Spec 018 US1 AS4 (FR-003, FR-001): a change 3.0 → `to` scheduled at [`change_at`] on the
/// clipped tone through stage 11 alone: the four partitions give one output and one log; the
/// output equals the 3.0 V run before the change and the `to` run from transition sample N;
/// the click energy around the transition is reported; a change on the earphone route is
/// logged and has no effect.
pub fn transition_rows(make: Make<'_>, rate: u32, to: f32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let n = n_of(rate);
    let c = change_at(rate);
    let change = [Change::battery(Stream::OnePass, c, to, 1)];
    let partitions = [
        Partition::Fixed(1), // spec AS4: block size 1 (coach review 11)
        Partition::Fixed(64),
        Partition::Fixed(4096),
        Partition::Irregular,
        Partition::IrregularWithBoundary(c),
    ];
    let runs: Vec<cc::Run> = partitions
        .iter()
        .map(|&part| {
            let mut p = make(settings);
            p.set_route(Route::Speaker);
            p.set_volume(VOLUME_6DB_OVER).expect("a valid position");
            cc::run_scheduled(&mut p, Source::OnePass(&x), &change, part)
        })
        .collect();
    let y = &runs[0].out;
    let a = run_at(make, settings, Route::Speaker, VOLUME_6DB_OVER, 3.0, &x);
    let b = run_at(make, settings, Route::Speaker, VOLUME_6DB_OVER, to, &x);
    let c = c as usize;
    let agree = runs[1..]
        .iter()
        .all(|r| cc::same(&r.out, y) && r.log == runs[0].log);
    let entries = battery_entries(&runs[0].log);
    let logged = entries.len() == 1 && {
        let e = entries[0];
        e.position == c as u64 && e.before == 3.0 && e.after == f64::from(to) && e.tag == 1
    };
    let stim = format!("3.0 -> {to} V at output sample {c}, N = {n}");
    // The earphone route: logged, no effect.
    let (ear_y, ear_log) = {
        let mut p = make(settings);
        p.set_route(Route::Earphone);
        p.set_volume(1.0).expect("a valid position");
        let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &change, Partition::Fixed(1024));
        (r.out, r.log)
    };
    let ear_ref = run_at(make, settings, Route::Earphone, 1.0, 3.0, &x);
    // Hardware review B2: spec 011 FR-004's click reader: the instantaneous step is the splice
    // of the two constant runs at P; the transition's window is centred on its midpoint, the
    // step's on the step. Reported (see the row).
    let step: Vec<f32> = a[..c].iter().chain(&b[c..]).copied().collect();
    let step_click = analysis::click_energy(&step, rate, c as f64);
    let transition_click = analysis::click_energy(y, rate, c as f64 + n as f64 / 2.0);
    let reduction_db = analysis::db(step_click / transition_click.max(1e-300)) / 2.0;
    vec![
        MeasurementResult::new(
            "FR-003",
            "the five partitions give one output and one log",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(agree),
            "",
            Tolerance::Exact(1.0), // 018 FR-003 (015 FR-008)
        ),
        MeasurementResult::new(
            "FR-001",
            "one change entry: position, before, after and tag",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(logged),
            "",
            Tolerance::Exact(1.0), // 018 FR-001
        ),
        MeasurementResult::new(
            "FR-003",
            "bit-identical to the 3.0 V run before the change",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(cc::same(&y[..c], &a[..c])),
            "",
            Tolerance::Exact(1.0), // 018 FR-003
        ),
        MeasurementResult::new(
            "FR-003",
            "bit-identical to the new value's run from transition sample N",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(cc::same(&y[c + n - 1..], &b[c + n - 1..])),
            "",
            Tolerance::Exact(1.0), // 018 FR-003
        ),
        MeasurementResult::new(
            "FR-003",
            "click energy of the transition below the instantaneous step (reported)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim,
            reduction_db,
            "dB",
            // Reported, not asserted: on a hard-clipped tone spec 011's reader does not behave
            // as on 011's −30 dBFS tone (the in-band energy of both the step and the transition
            // is the clipper's harmonics' aliases and the splice's own small content; the figure
            // read about 1 dB at 44.1 kHz and above, 2 and 11 dB at 16 and 8 kHz and 46 dB at
            // 6 kHz when a 30 dB floor was tried after the hardware review B2). The
            // transition's oracle is the splice and the factor trace.
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-003",
            "earphone route: the change is logged and has no effect",
            T_BATT,
            rate,
            "stage11_only_earphone",
            format!("3.0 -> {to} V at output sample {c}"),
            pass(cc::same(&ear_y, &ear_ref) && battery_entries(&ear_log).len() == 1),
            "",
            Tolerance::Exact(1.0), // 018 FR-003: the earphone route does not move
        ),
    ]
}

// ---- FR-004: the other paths -------------------------------------------------------------------

/// Spec 018 US1 AS5 (FR-004): the earphone route's output and a recording's stored samples
/// are bit-identical to the default at every law point.
pub fn other_paths(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut settings = configs::settings("default_output", rate);
    settings.speech_codec_enabled = false; // the stored samples are the oracle (017's lesson)
    let x = clipped_tone(rate);
    let ear_ref = run_at(make, settings, Route::Earphone, 1.0, 3.0, &x);
    let stored = |volts: f32| -> Vec<f32> {
        let mut p = make(settings);
        p.set_battery_volts(volts).expect("a valid battery value");
        p.start_recording().expect("start");
        let _ = p.record(&x, &mut []).expect("record");
        let _ = p.stop_recording(&mut []).expect("stop");
        __test_hooks::stored_samples(&p, 1).to_vec()
    };
    let stored_ref = stored(3.0);
    let mut rows = Vec::new();
    for volts in law_points() {
        let y = run_at(make, settings, Route::Earphone, 1.0, volts, &x);
        rows.push(MeasurementResult::new(
            "FR-004",
            "earphone route bit-identical to the default",
            T_BATT,
            rate,
            "default_output",
            format!("tone 1 kHz 0 dBFS, position 1, {volts} V"),
            pass(cc::same(&y, &ear_ref)),
            "",
            Tolerance::Exact(1.0), // 018 FR-004
        ));
        rows.push(MeasurementResult::new(
            "FR-004",
            "a recording's stored samples bit-identical to the default",
            T_BATT,
            rate,
            "default_output",
            format!("tone 1 kHz 0 dBFS recorded, {volts} V"),
            pass(cc::same(&stored(volts), &stored_ref)),
            "",
            Tolerance::Exact(1.0), // 018 FR-004: the record path is on regulated rails
        ));
    }
    rows
}

// ---- FR-005: the default identity -------------------------------------------------------------

/// Every third configuration of the earlier goldens' lists (spec 018 US1 AS1).
pub fn sampled_configs() -> Vec<&'static str> {
    let all: Vec<&'static str> = configs::CONFIGS
        .iter()
        .chain(configs::AGC_CONFIGS.iter())
        .chain(configs::VAS_CONFIGS.iter())
        .chain(configs::CODEC_CONFIGS.iter())
        .chain(configs::OUTPUT_CONFIGS.iter())
        .chain(configs::OUTPUT_CLAMP_CONFIGS.iter())
        .chain(configs::STAGE11_CONFIGS.iter())
        .chain(configs::MIC_CONFIGS.iter())
        .copied()
        .collect();
    all.iter().step_by(3).copied().collect()
}

/// Spec 018 US1 AS1 (FR-005): the default engine with `set_battery_volts(3.0)` called and a
/// scheduled no-op to 3.0 equals the engine without, on both routes, over the sampled
/// configurations; the no-op is logged as one entry.
pub fn default_identity(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x018);
    let mut rows = Vec::new();
    for config in sampled_configs() {
        for route in [Route::Speaker, Route::Earphone] {
            let settings = configs::settings(config, rate);
            let mut plain = make(settings);
            plain.set_route(route);
            let reference = cc::run(&mut plain, Source::OnePass(&x), Partition::Fixed(1024));
            let mut twin = make(settings);
            twin.set_route(route);
            twin.set_battery_volts(3.0).expect("the default value");
            let noop = [Change::battery(Stream::OnePass, 500, 3.0, 7)];
            let r = cc::run_scheduled(
                &mut twin,
                Source::OnePass(&x),
                &noop,
                Partition::Fixed(1024),
            );
            let entries = battery_entries(&r.log);
            let logged = entries.len() == 1 && entries[0].position == 500 && entries[0].tag == 7;
            rows.push(MeasurementResult::new(
                "FR-005",
                "bit-identical to the engine without the setter and the no-op",
                T_BATT,
                rate,
                config,
                format!("voiced -20 dBFS, {route:?}, set 3.0 V and a scheduled 3.0 V"),
                pass(
                    cc::same(&r.out, &reference.out)
                        && battery_entries(&reference.log).is_empty()
                        && logged,
                ),
                "",
                Tolerance::Exact(1.0), // 018 FR-005: nothing existing moves
            ));
        }
    }
    rows
}

// ---- The matrix's extra rows -------------------------------------------------------------------

/// The clamp row (spec 018 US1 AS3's matrix): a full-scale square at position 1 through the
/// default output with the clamp on at 3.0, 2.6 and 2.2 V: the route peak before the clamp
/// and the output-clamp count. The clamp **cannot** engage below about 2.32 – 2.49 V by host
/// rate (the speaker template's L1 norm, 1.75 – 2.29, times b stays under 1.0: spec 010's
/// bound), so at 2.2 V the count is zero at every rate; where it does engage depends on the
/// material: on this square it engages at 3.0 V at every rate and at 2.6 V from 8 kHz up, not at
/// 6 kHz (hardware review B1: the matrix's own figures, asserted as such); the peaks are
/// reported.
pub fn clamp_row(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = configs::settings("default_output_clamp", rate);
    let x = stimulus::square(100.0, 1.0, f64::from(rate), rate as usize);
    let mut rows = Vec::new();
    for volts in [3.0f32, 2.6, 2.2] {
        let mut p = make(settings);
        p.set_route(Route::Speaker);
        p.set_volume(1.0).expect("a valid position");
        p.set_battery_volts(volts).expect("a valid battery value");
        let mut y = vec![0.0f32; x.len()];
        let _ = p.process(&x, &mut y).expect("ok");
        let m: Meters = p.meters();
        let stim = format!("square 100 Hz full scale, position 1, {volts} V");
        rows.push(MeasurementResult::new(
            "FR-010",
            "route peak before the output clamp (reported)",
            T_BATT,
            rate,
            "default_output_clamp",
            stim.clone(),
            f64::from(m.route_peak),
            "",
            Tolerance::Report,
        ));
        rows.push(MeasurementResult::new(
            "FR-010",
            "output-clamp count (zero at 2.2 V and at 2.6 V / 6 kHz; engaged elsewhere)",
            T_BATT,
            rate,
            "default_output_clamp",
            stim,
            f64::from(m.output_clamp_count),
            "samples",
            if volts == 2.2 || (volts == 2.6 && rate == 6000) {
                Tolerance::Exact(0.0) // 018 D5: under the L1 bound at 2.2 V; at 2.6 V / 6 kHz the matrix's figure
            } else {
                Tolerance::AtLeast(1.0) // the matrix's figures (hardware review B1)
            },
        ));
    }
    rows
}

/// The setter oracle (spec 015's primary oracle restated): a plain `set_battery_volts` between
/// two calls equals the scheduled change at the boundary, bit for bit, and the two logs agree
/// in position and values.
pub fn setter_oracle(make: Make<'_>, rate: u32, to: f32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let at = 1000usize;
    let mut plain = make(settings);
    plain.set_route(Route::Speaker);
    plain.set_volume(VOLUME_6DB_OVER).expect("a valid position");
    let mut y = vec![0.0f32; x.len()];
    let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let mut ev = vec![VasEvent::default(); plain.max_events(x.len())];
    let i1 = plain
        .process_with_events_and_changes(&x[..at], &mut y[..at], &mut ev, &mut log)
        .expect("ok");
    assert!(
        battery_entries(&log[..i1.changes]).is_empty(),
        "no battery entry before the setter"
    );
    plain.set_battery_volts(to).expect("a valid battery value");
    let i2 = plain
        .process_with_events_and_changes(&x[at..], &mut y[at..], &mut ev, &mut log)
        .expect("ok");
    let plain_entry = battery_entries(&log[..i2.changes]);
    let mut sched = make(settings);
    sched.set_route(Route::Speaker);
    sched.set_volume(VOLUME_6DB_OVER).expect("a valid position");
    let r = cc::run_scheduled(
        &mut sched,
        Source::OnePass(&x),
        &[Change::battery(Stream::OnePass, at as u64, to, 3)],
        Partition::Irregular,
    );
    let sched_entry = battery_entries(&r.log);
    let logs_agree = plain_entry.len() == 1
        && sched_entry.len() == 1
        && plain_entry[0].position == sched_entry[0].position
        && plain_entry[0].before.to_bits() == sched_entry[0].before.to_bits()
        && plain_entry[0].after.to_bits() == sched_entry[0].after.to_bits()
        && plain_entry[0].tag == 0
        && sched_entry[0].tag == 3;
    vec![MeasurementResult::new(
        "FR-001",
        "a plain setter at a block boundary equals the scheduled change there",
        T_BATT,
        rate,
        "stage11_only_speaker",
        format!("3.0 -> {to} V at output sample {at}"),
        pass(cc::same(&y, &r.out) && logs_agree),
        "",
        Tolerance::Exact(1.0), // 018 FR-001 (015's setter oracle)
    )]
}

// ---- The mutants' oracles (research R-07) -----------------------------------------------------

/// Spec 018 FR-004 (research R-02): a route change ends a battery transition in flight: a
/// change set on a warmed speaker route, one transition sample processed, the route switched
/// to the earphone and back, and the first speaker sample reads exactly the target.
pub fn route_change_ends_transition(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut p = make(stage11_only(rate));
    p.set_route(Route::Speaker);
    p.set_volume(VOLUME_6DB_OVER).expect("a valid position");
    let x = clipped_tone(rate);
    let mut y = vec![0.0f32; 2048];
    let _ = p.process(&x[..2048], &mut y).expect("ok");
    p.set_battery_volts(2.4).expect("a valid battery value");
    let mut one = [0.0f32; 1];
    let _ = p.process(&x[2048..2049], &mut one).expect("ok");
    let mid = __test_hooks::battery_factor(&p, Stream::OnePass).expect("the one-pass");
    p.set_route(Route::Earphone);
    let _ = p.process(&x[2049..2050], &mut one).expect("ok");
    p.set_route(Route::Speaker);
    let _ = p.process(&x[2050..2051], &mut one).expect("ok");
    let after = __test_hooks::battery_factor(&p, Stream::OnePass).expect("the one-pass");
    let b1 = f64::from(ceiling_factor(2.4));
    vec![
        MeasurementResult::new(
            "FR-004",
            "the transition had started before the route change",
            T_BATT,
            rate,
            "stage11_only_speaker",
            "3.0 -> 2.4 V, one transition sample",
            pass(mid > b1 && mid < 1.0),
            "",
            Tolerance::Exact(1.0), // 018 FR-004
        ),
        MeasurementResult::new(
            "FR-004",
            "a route change ends the transition: the target exactly on the first speaker sample",
            T_BATT,
            rate,
            "stage11_only_speaker",
            "3.0 -> 2.4 V, earphone and back",
            pass(after.to_bits() == b1.to_bits()),
            "",
            Tolerance::Exact(1.0), // 018 FR-004 (research R-02, hardware B9)
        ),
    ]
}

// ---- FR-001, FR-002 on the playback stream (coach review 3) --------------------------------------

/// The clipped tone recorded (stage 11 alone, so the stored samples are the decimated tone) and
/// played at `VOLUME_6DB_OVER` on the speaker route: the settled peak of the playback at `volts`
/// equals the factor to the bit, and a change scheduled on the playback stream at P equals the
/// 3.0 V playback before P and the `to` playback from transition sample N.
pub fn playback_rows(make: Make<'_>, rate: u32, to: f32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    let n = n_of(rate);
    let engine = || {
        let mut p = make(settings);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_6DB_OVER).expect("a valid position");
        p.start_recording().expect("start");
        let _ = p.record(&x, &mut []).expect("record");
        let _ = p.stop_recording(&mut []).expect("stop");
        p
    };
    let play_at = |volts: f32| -> Vec<f32> {
        let mut p = engine();
        p.set_battery_volts(volts).expect("a valid battery value");
        cc::run(&mut p, Source::Playback(1), Partition::Fixed(1024)).out
    };
    let (a, b) = (play_at(3.0), play_at(to));
    let c = change_at(rate) as usize;
    let mut p = engine();
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::battery(Stream::Playback, c as u64, to, 1)],
        Partition::Fixed(1024),
    );
    let y = &r.out;
    // The playback's own part (before the drain) settles like the one-pass: read the peak over
    // the recording's middle second.
    let mid = (x.len() / 2).min(y.len().saturating_sub(rate as usize));
    let settled = |v: &[f32]| {
        v[mid..(mid + rate as usize).min(v.len())]
            .iter()
            .fold(0.0f64, |m, &s| m.max(f64::from(s).abs()))
    };
    let stim = format!("recorded tone 1 kHz 0 dBFS played at +6 dB, 3.0 -> {to} V at {c}");
    vec![
        MeasurementResult::new(
            "FR-002",
            "playback: settled sample peak equals the ceiling factor (bit for bit)",
            T_BATT,
            rate,
            "stage11_only_speaker",
            format!("recorded tone 1 kHz 0 dBFS played at +6 dB, {to} V"),
            pass((settled(&b) as f32).to_bits() == ceiling_factor(to).to_bits()),
            "",
            Tolerance::Exact(1.0), // 018 FR-002 on the playback stream
        ),
        MeasurementResult::new(
            "FR-001",
            "playback: bit-identical to the 3.0 V playback before the change",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(y.len() == a.len() && cc::same(&y[..c], &a[..c])),
            "",
            Tolerance::Exact(1.0), // 018 FR-001
        ),
        MeasurementResult::new(
            "FR-003",
            "playback: bit-identical to the new value's playback from transition sample N",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim.clone(),
            pass(y.len() == b.len() && cc::same(&y[c + n - 1..], &b[c + n - 1..])),
            "",
            Tolerance::Exact(1.0), // 018 FR-003
        ),
        MeasurementResult::new(
            "FR-001",
            "playback: one battery entry at the position",
            T_BATT,
            rate,
            "stage11_only_speaker",
            stim,
            pass({
                let e = battery_entries(&r.log);
                e.len() == 1 && e[0].position == c as u64 && e[0].after == f64::from(to)
            }),
            "",
            Tolerance::Exact(1.0), // 018 FR-001
        ),
    ]
}

// ---- Convergence T027, T028 ------------------------------------------------------------------------

/// SC-001 "both streams" (convergence T027): the below-knee identity on the **playback stream**:
/// the −20 dBFS voiced stimulus recorded once and played at the reference position on the whole
/// default with the codec off (the stored samples are the oracle's input), bit-identical at the
/// five law points to the default's playback.
pub fn playback_below_knee_identity(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut settings = whole_default(rate);
    settings.speech_codec_enabled = false;
    let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x018);
    let play_at = |volts: Option<f32>| -> Vec<f32> {
        let mut p = make(settings);
        p.set_route(Route::Speaker);
        p.set_volume(VOLUME_REFERENCE).expect("a valid position");
        p.start_recording().expect("start");
        let _ = p.record(&x, &mut []).expect("record");
        let _ = p.stop_recording(&mut []).expect("stop");
        if let Some(v) = volts {
            p.set_battery_volts(v).expect("a valid battery value");
        }
        cc::run(&mut p, Source::Playback(1), Partition::Fixed(1024)).out
    };
    let reference = play_at(None);
    law_points()
        .iter()
        .map(|&volts| {
            MeasurementResult::new(
                "FR-002",
                "playback: bit-identical to the default below the knee",
                T_BATT,
                rate,
                "default",
                format!("voiced -20 dBFS recorded and played at the reference position, {volts} V"),
                pass(cc::same(&play_at(Some(volts)), &reference)),
                "",
                Tolerance::Exact(1.0), // 018 SC-001: both streams
            )
        })
        .collect()
}

/// FR-010 / SC-004 "at every value and rate" (convergence T028): the amplifier clip count of
/// every 1024-sample block of the clipped tone at `VOLUME_6DB_OVER` equals the output twin's
/// count of samples above the knee's start (the knee is the identity below its start and
/// strictly increasing above it; hardware review B6's caveat applies), at the five law points.
pub fn clip_count_twin(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = stage11_only(rate);
    let x = clipped_tone(rate);
    law_points()
        .iter()
        .map(|&volts| {
            let knee_lo = 0.85 * f64::from(ceiling_factor(volts));
            let mut p = make(settings);
            p.set_route(Route::Speaker);
            p.set_volume(VOLUME_6DB_OVER).expect("a valid position");
            p.set_battery_volts(volts).expect("a valid battery value");
            let mut mismatches = 0usize;
            let mut total = 0u64;
            for chunk in x.chunks(1024) {
                let mut y = vec![0.0f32; chunk.len()];
                let _ = p.process(chunk, &mut y).expect("ok");
                let twin = y.iter().filter(|&&v| f64::from(v).abs() > knee_lo).count();
                let count = p.meters().amp_clip_count as usize;
                mismatches += usize::from(count != twin);
                total += count as u64;
            }
            MeasurementResult::new(
                "FR-010",
                "clip count equals the output twin's on every block (total above 0)",
                T_BATT,
                rate,
                "stage11_only_speaker",
                format!("tone 1 kHz 0 dBFS at +6 dB, {volts} V"),
                pass(mismatches == 0 && total > 0),
                "",
                Tolerance::Exact(1.0), // 018 FR-010, SC-004
            )
        })
        .collect()
}
