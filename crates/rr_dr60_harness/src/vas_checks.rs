//! Spec 003 VAS checks: one function per requirement (research.md R-11; tasks.md T034).
//!
//! Every check runs the VAS-isolated configuration (`vas_only`: AGC, stage 4 and stage 10
//! bypassed, tap after the VAS) unless stated otherwise, and reports `003/FR-0xx` with its
//! trace IDs. Timings and thresholds are measured from **output lengths and events**, never
//! from an amplitude envelope (R-11). Tolerances that are not device properties are
//! engineering targets (spec 003 FR-017, R-15).

use rr_dr60::{BlockInfo, Pipeline, Settings, Tap, VasEvent, VasMode, VasSettings};

use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

const CONFIG: &str = "vas_only";
const T_A022: &[&str] = &["A-022", "engineering target"];
const T_A023: &[&str] = &["A-023", "engineering target"];
const T_A024: &[&str] = &["A-024", "engineering target"];
const T_A025: &[&str] = &["A-025"];
const T_ENG: &[&str] = &["engineering target"];
/// The device rate: hang and onset times are rounded to whole samples of it (A-001, R-04).
const DEVICE_RATE: f64 = 8000.0;
/// Bisection resolution for the threshold search (engineering target, 003 R-15).
const BISECT_DB: f64 = 0.05;
/// The burst level above the threshold (spec 003 definitions; engineering target, 003 R-15).
const BURST_ABOVE_DB: f64 = 10.0;

/// A named VAS setting for the matrix (spec 003 FR-012, FR-013).
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// Label for reports, e.g. `"hang min"`.
    pub label: &'static str,
    /// The VAS settings.
    pub vas: VasSettings,
}

impl Case {
    /// Whether these are the device defaults.
    pub fn is_default(&self) -> bool {
        self.vas == VasSettings::DEVICE
    }
}

/// The FR-013 settings matrix: the defaults, every sensitivity level, and the minimum and
/// maximum of each other setting with the rest at default.
pub fn matrix() -> Vec<Case> {
    let with = |label, f: fn(&mut VasSettings)| {
        let mut vas = VasSettings::DEVICE;
        f(&mut vas);
        Case { label, vas }
    };
    vec![
        with("defaults", |_| {}),
        with("sensitivity 1", |v| v.sensitivity = 1),
        with("sensitivity 2", |v| v.sensitivity = 2),
        with("sensitivity 4", |v| v.sensitivity = 4),
        with("sensitivity 5", |v| v.sensitivity = 5),
        with("threshold min", |v| v.threshold_dbfs = -60.0),
        with("threshold max", |v| v.threshold_dbfs = 0.0),
        with("hang min", |v| v.hang_ms = 50.0),
        with("hang max", |v| v.hang_ms = 10_000.0),
        with("onset min", |v| v.onset_ms = 0.0),
        with("onset max", |v| v.onset_ms = 200.0),
    ]
}

/// The effective threshold in dBFS: the level-3 threshold moved 3 dB per sensitivity step
/// (A-022; data-model › VasSettings).
pub fn effective_threshold_dbfs(vas: &VasSettings) -> f64 {
    f64::from(vas.threshold_dbfs) + 3.0 * (3.0 - f64::from(vas.sensitivity))
}

/// Hang time in seconds, as the stage rounds it (whole device samples, R-04).
pub fn hang_s(vas: &VasSettings) -> f64 {
    (f64::from(vas.hang_ms) * DEVICE_RATE / 1000.0).round() / DEVICE_RATE
}

/// Onset time in seconds, as the stage rounds it (whole device samples, R-04).
pub fn onset_s(vas: &VasSettings) -> f64 {
    (f64::from(vas.onset_ms) * DEVICE_RATE / 1000.0).round() / DEVICE_RATE
}

/// The VAS-isolated settings with `vas` applied.
pub fn vas_only(rate: u32, vas: &VasSettings) -> Settings {
    let mut s = configs::settings(CONFIG, rate);
    s.vas = *vas;
    s
}

/// One run's output, events and final block result.
#[derive(Clone, Debug)]
pub struct Run {
    /// The produced samples.
    pub y: Vec<f32>,
    /// The events, in order.
    pub events: Vec<VasEvent>,
    /// The block result.
    pub info: BlockInfo,
}

impl Run {
    /// The stream's kept lengths in output samples: from each event's position to the next
    /// event (or the end). The first entry is the output before the first event.
    pub fn regions(&self) -> Vec<usize> {
        let mut bounds: Vec<usize> = vec![0];
        bounds.extend(self.events.iter().map(|e| e.output_position as usize));
        bounds.push(self.y.len());
        bounds.windows(2).map(|w| w[1] - w[0]).collect()
    }

    /// Host input samples removed, summed over the events.
    pub fn removed(&self) -> u64 {
        self.events.iter().map(|e| e.input_length).sum()
    }
}

/// Processes `x` in one block, with events.
pub fn run(make: Make<'_>, settings: Settings, x: &[f32]) -> Run {
    let mut p = make(settings);
    let mut y = vec![0.0f32; x.len()];
    let mut events = vec![VasEvent::default(); p.max_events(x.len())];
    let info = p
        .process_with_events(x, &mut y, &mut events)
        .expect("equal lengths");
    y.truncate(info.produced);
    events.truncate(info.events);
    Run { y, events, info }
}

/// Whether a run kept everything: output as long as the input, no event, not paused.
fn kept_in_full(r: &Run) -> bool {
    r.y.len() == r.info.produced && r.events.is_empty() && !r.info.paused && !r.y.is_empty()
}

/// A steady tone at `freq_hz` and `level_dbfs` for `secs`, starting at phase 0.
fn tone(freq_hz: f64, level_dbfs: f64, secs: f64, fs: f64) -> Vec<f32> {
    stimulus::step(freq_hz, &[level_dbfs], &[secs], fs)
}

/// A burst-gap stimulus at the case's burst level (threshold + 10 dB).
fn burst_gap(vas: &VasSettings, segments: &[(bool, f64)], fs: f64) -> Vec<f32> {
    stimulus::burst_gap(
        1000.0,
        effective_threshold_dbfs(vas) + BURST_ABOVE_DB,
        segments,
        fs,
    )
}

/// The lowest `freq_hz` tone level, to 0.05 dB, that is kept in full for H + 1 s: bisection
/// between `lo` (expected dropped) and `hi` (expected kept). NaN when the bracket fails.
pub fn lowest_kept_level_dbfs(
    make: Make<'_>,
    settings: Settings,
    freq_hz: f64,
    mut lo: f64,
    mut hi: f64,
) -> f64 {
    let fs = f64::from(settings.host_rate_hz);
    let secs = hang_s(&settings.vas) + 1.0;
    let kept = |level: f64| kept_in_full(&run(make, settings, &tone(freq_hz, level, secs, fs)));
    if kept(lo) || !kept(hi) {
        return f64::NAN;
    }
    while hi - lo > BISECT_DB {
        let mid = 0.5 * (lo + hi);
        if kept(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// A tolerance of ±1 ms, as samples at `fs` (engineering target, 003 FR-017).
fn ms_tolerance(target_samples: f64, fs: f64) -> Tolerance {
    let tol = 0.001 * fs;
    Tolerance::Range(target_samples - tol, target_samples + tol)
}

/// Hang stimulus: burst 1 s, gap `gap_s`, burst 1 s.
fn hang_run(make: Make<'_>, settings: Settings, gap_s: f64) -> (Run, usize) {
    let fs = f64::from(settings.host_rate_hz);
    let x = burst_gap(
        &settings.vas,
        &[(true, 1.0), (false, gap_s), (true, 1.0)],
        fs,
    );
    (run(make, settings, &x), x.len())
}

/// The kept part of a gap longer than the hang time, in host samples, from the splice's
/// removed length: gap − removed + onset (R-11).
fn kept_gap_samples(r: &Run, gap_s: f64, vas: &VasSettings, fs: f64) -> f64 {
    gap_s * fs - r.removed() as f64 + onset_s(vas) * fs
}

/// 003 FR-004: output length. Nothing dropped ⇒ output length equals input length exactly;
/// with a splice, the dropped length agrees with the nominal (gap − hang + onset) within one
/// device sample plus one host sample (the burst's first sample is a zero crossing, and the
/// decimator's edge smear can move the pause by one device sample, R-11; engineering target).
pub fn check_fr004_length(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let h = hang_s(&case.vas);
    let (short, short_len) = hang_run(make, settings, 0.5 * h);
    let (long, long_len) = hang_run(make, settings, 3.0 * h);
    let nominal_dropped = (3.0 * h - h + onset_s(&case.vas)) * fs;
    // One device sample of slack: the burst starts at a zero crossing, so its first device
    // sample is not a sound sample, and the decimator's edge smear can move the pause by one
    // device sample at resampled rates (R-11); plus one host sample of emission rounding.
    let smear = fs / DEVICE_RATE + 1.0;
    vec![
        MeasurementResult::new(
            "003/FR-004",
            "output length − input length, nothing dropped",
            T_ENG,
            rate,
            CONFIG,
            format!("{}: burst 1 s, gap 0.5 H, burst 1 s", case.label),
            short.y.len() as f64 - short_len as f64,
            "samples",
            Tolerance::Exact(0.0), // 003 FR-004: exact when nothing is dropped
        ),
        MeasurementResult::new(
            "003/FR-004",
            "dropped length − nominal (gap − hang + onset)",
            T_ENG,
            rate,
            CONFIG,
            format!("{}: burst 1 s, gap 3 H, burst 1 s", case.label),
            (long_len - long.y.len()) as f64 - nominal_dropped,
            "samples",
            Tolerance::Range(-smear, smear), // ± one device sample + 1, engineering target (003 FR-017)
        ),
    ]
}

/// 003 FR-005: the removed lengths add up to input − output (±1 host sample, exact at 8, 16,
/// 48 and 96 kHz) for a stream ending while recording; mute-mode region lengths equal drop
/// mode's removed lengths (exact at those rates, ±1 at 44.1 and 88.2 kHz); `paused` is
/// correct at the end.
pub fn check_fr005_events(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let h = hang_s(&case.vas);
    let (drop, len) = hang_run(make, settings, 3.0 * h);
    let mut mute_settings = settings;
    mute_settings.vas.mode = VasMode::Mute;
    let (mute, _) = hang_run(make, mute_settings, 3.0 * h);
    let integer_ratio = rate % 8000 == 0;
    let tol = if integer_ratio { 0.0 } else { 1.0 }; // ±1 host sample, engineering target (003 FR-017)
    let stim = || format!("{}: burst 1 s, gap 3 H, burst 1 s", case.label);
    let mut out = vec![
        MeasurementResult::new(
            "003/FR-005",
            "removed lengths − (input − output)",
            T_ENG,
            rate,
            CONFIG,
            stim(),
            drop.removed() as f64 - (len - drop.y.len()) as f64,
            "samples",
            Tolerance::Range(-tol, tol),
        ),
        MeasurementResult::new(
            "003/FR-005",
            "splice count",
            T_A023,
            rate,
            CONFIG,
            stim(),
            drop.events.len() as f64,
            "events",
            Tolerance::Exact(1.0), // 003 FR-005: one gap longer than the hang time, one splice
        ),
        MeasurementResult::new(
            "003/FR-005",
            "paused at end (stream ends while recording)",
            T_A025,
            rate,
            CONFIG,
            stim(),
            f64::from(u8::from(drop.info.paused)),
            "flag",
            Tolerance::Exact(0.0),
        ),
        MeasurementResult::new(
            "003/FR-005",
            "mute region count",
            T_ENG,
            rate,
            "vas_mute",
            stim(),
            mute.events.len() as f64,
            "events",
            Tolerance::Exact(1.0),
        ),
    ];
    if drop.events.len() == 1 && mute.events.len() == 1 {
        out.push(MeasurementResult::new(
            "003/FR-005",
            "mute region length − drop removed length",
            T_ENG,
            rate,
            "vas_mute",
            stim(),
            mute.events[0].input_length as f64 - drop.events[0].input_length as f64,
            "samples",
            Tolerance::Range(-tol, tol),
        ));
        out.push(MeasurementResult::new(
            "003/FR-005",
            "mute output length − input length",
            T_ENG,
            rate,
            "vas_mute",
            stim(),
            mute.y.len() as f64 - len as f64,
            "samples",
            Tolerance::Exact(0.0), // 003 FR-004: mute mode keeps the length
        ));
    }
    // A stream that ends inside a pause reports `paused`.
    let x = burst_gap(&case.vas, &[(true, 0.5), (false, 2.0 * h + 1.0)], fs);
    let r = run(make, settings, &x);
    out.push(MeasurementResult::new(
        "003/FR-005",
        "paused at end (stream ends in a pause)",
        T_A025,
        rate,
        CONFIG,
        format!("{}: burst 0.5 s, gap 2 H + 1 s", case.label),
        f64::from(u8::from(r.info.paused)),
        "flag",
        Tolerance::Exact(1.0),
    ));
    out
}

/// 003 FR-006: the threshold. Bisection to 0.05 dB at 300, 1000 and 3400 Hz, within ±1 dB of
/// each other and of the nominal (reported per rate); a tone at +3 dB is kept in full and one at
/// −3 dB is dropped after the hang time; band-limited noise at T − 5 dB is kept and at T − 15 dB
/// dropped, for hang times ≥ 0.5 s (FR-013).
pub fn check_fr006_threshold(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let t = effective_threshold_dbfs(&case.vas);
    let h = hang_s(&case.vas);
    let secs = h + 1.0;
    let mut out = Vec::new();
    // Thresholds by bisection. The bracket is ±6 dB around the nominal: a mutant moved by 3 dB
    // still bisects, one moved by more fails the bracket and reports NaN.
    let at = |f: f64| lowest_kept_level_dbfs(make, settings, f, t - 6.0, t + 6.0);
    let t1k = at(1000.0);
    out.push(MeasurementResult::new(
        "003/FR-006",
        "threshold at 1 kHz − nominal",
        T_A022,
        rate,
        CONFIG,
        format!("{}: bisection, H + 1 s tones", case.label),
        t1k - t,
        "dB",
        Tolerance::Range(-1.0, 1.0), // engineering target (003 FR-017); A-022
    ));
    for f in [300.0, 3400.0] {
        out.push(MeasurementResult::new(
            "003/FR-006",
            "threshold at f − threshold at 1 kHz",
            T_A022,
            rate,
            CONFIG,
            format!("{}: {f} Hz", case.label),
            at(f) - t1k,
            "dB",
            Tolerance::Range(-1.0, 1.0), // engineering target (003 FR-017); A-022
        ));
    }
    // +3 dB kept in full; −3 dB dropped after the hang time.
    let above = run(make, settings, &tone(1000.0, t + 3.0, secs, fs));
    out.push(MeasurementResult::new(
        "003/FR-006",
        "tone at threshold + 3 dB kept in full",
        T_A022,
        rate,
        CONFIG,
        format!("{}: 1 kHz {:.1} dBFS, H + 1 s", case.label, t + 3.0),
        f64::from(u8::from(kept_in_full(&above))),
        "flag",
        Tolerance::Exact(1.0),
    ));
    let below = run(make, settings, &tone(1000.0, t - 3.0, secs, fs));
    out.push(MeasurementResult::new(
        "003/FR-006",
        "output length for a tone at threshold − 3 dB (one hang time)",
        T_A022,
        rate,
        CONFIG,
        format!("{}: 1 kHz {:.1} dBFS, H + 1 s", case.label, t - 3.0),
        below.y.len() as f64,
        "samples",
        ms_tolerance(h * fs, fs),
    ));
    out.push(MeasurementResult::new(
        "003/FR-006",
        "paused after a tone at threshold − 3 dB",
        T_A022,
        rate,
        CONFIG,
        format!("{}: 1 kHz {:.1} dBFS, H + 1 s", case.label, t - 3.0),
        f64::from(u8::from(below.info.paused && below.events.is_empty())),
        "flag",
        Tolerance::Exact(1.0),
    ));
    // Peak-responding detector: noise 5 dB below the threshold is kept, 15 dB below is dropped
    // (its peaks read about 8–9 dB above its AES17 level). Only for hang times ≥ 0.5 s.
    if case.vas.hang_ms >= 500.0 {
        // 003 FR-013: the noise checks apply only for hang times of at least 0.5 s.
        let len = (secs * fs) as usize;
        let kept = run(
            make,
            settings,
            &stimulus::bandlimited_noise(0x0D60, len, fs, t - 5.0),
        );
        let dropped = run(
            make,
            settings,
            &stimulus::bandlimited_noise(0x0D60, len, fs, t - 15.0),
        );
        out.push(MeasurementResult::new(
            "003/FR-006",
            "noise at threshold − 5 dB kept in full",
            T_A022,
            rate,
            CONFIG,
            format!("{}: band-limited noise {:.1} dBFS", case.label, t - 5.0),
            f64::from(u8::from(kept_in_full(&kept))),
            "flag",
            Tolerance::Exact(1.0),
        ));
        out.push(MeasurementResult::new(
            "003/FR-006",
            "noise at threshold − 15 dB dropped after the hang time",
            T_A022,
            rate,
            CONFIG,
            format!("{}: band-limited noise {:.1} dBFS", case.label, t - 15.0),
            f64::from(u8::from(
                dropped.info.paused && dropped.events.is_empty() && dropped.y.len() < len,
            )),
            "flag",
            Tolerance::Exact(1.0),
        ));
    }
    out
}

/// 003 FR-007: each sensitivity level moves the 1 kHz threshold by 3 dB per step from
/// level 3, ±1 dB, with the case's other settings.
pub fn check_fr007_sensitivity(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let mut base = case.vas;
    base.sensitivity = 3;
    let t3 = {
        let s = vas_only(rate, &base);
        lowest_kept_level_dbfs(
            make,
            s,
            1000.0,
            f64::from(base.threshold_dbfs) - 6.0,
            f64::from(base.threshold_dbfs) + 6.0,
        )
    };
    (1..=5u8)
        .map(|level| {
            let mut vas = base;
            vas.sensitivity = level;
            let t = effective_threshold_dbfs(&vas);
            let got = lowest_kept_level_dbfs(make, vas_only(rate, &vas), 1000.0, t - 6.0, t + 6.0);
            MeasurementResult::new(
                "003/FR-007",
                "threshold(level) − threshold(3) − 3 dB × (3 − level)",
                T_A022,
                rate,
                CONFIG,
                format!("{}: sensitivity {level}", case.label),
                got - t3 - 3.0 * (3.0 - f64::from(level)),
                "dB",
                Tolerance::Range(-1.0, 1.0), // engineering target (003 FR-017); A-022
            )
        })
        .collect()
}

/// 003 FR-008: hang time. Gaps of 0.5 H and H − 5 ms are kept in full; gaps of H + 5 ms and
/// 3 H keep exactly H (±1 ms) and produce one splice.
pub fn check_fr008_hang(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let h = hang_s(&case.vas);
    let mut out = Vec::new();
    for (name, gap, pauses) in [
        ("0.5 H", 0.5 * h, false),
        ("H − 5 ms", h - 0.005, false),
        ("H + 5 ms", h + 0.005, true),
        ("3 H", 3.0 * h, true),
    ] {
        let (r, len) = hang_run(make, settings, gap);
        let stim = format!("{}: burst 1 s, gap {name}, burst 1 s", case.label);
        if pauses {
            out.push(MeasurementResult::new(
                "003/FR-008",
                "splice count",
                T_A023,
                rate,
                CONFIG,
                stim.clone(),
                r.events.len() as f64,
                "events",
                Tolerance::Exact(1.0),
            ));
            out.push(MeasurementResult::new(
                "003/FR-008",
                "kept part of the gap",
                T_A023,
                rate,
                CONFIG,
                stim,
                kept_gap_samples(&r, gap, &case.vas, fs),
                "samples",
                ms_tolerance(h * fs, fs),
            ));
        } else {
            out.push(MeasurementResult::new(
                "003/FR-008",
                "gap kept in full (output − input, events)",
                T_A023,
                rate,
                CONFIG,
                stim,
                (r.y.len() as f64 - len as f64).abs() + r.events.len() as f64,
                "samples",
                Tolerance::Exact(0.0),
            ));
        }
    }
    out
}

/// The FR-009 stimulus for a case: a gap of H + 1 s, then bursts of 0.5 O, O − 2 ms (when
/// O ≥ 3 ms), 1.5 O and 50 O, each followed by H + 1 s of silence. With an onset of 0, only
/// a 50 ms burst. Returns the stimulus and the burst lengths in seconds.
pub fn onset_stimulus(vas: &VasSettings, fs: f64) -> (Vec<f32>, Vec<f64>) {
    let o = onset_s(vas);
    let h = hang_s(vas);
    let bursts_s: Vec<f64> = if vas.onset_ms == 0.0 {
        vec![0.05]
    } else {
        let mut b = vec![0.5 * o];
        if vas.onset_ms >= 3.0 {
            // 003 FR-009: the onset − 2 ms burst, only when the onset is at least 3 ms.
            b.push(o - 0.002);
        }
        b.push(1.5 * o);
        b.push(50.0 * o);
        b
    };
    let bursts_ms: Vec<f64> = bursts_s.iter().map(|s| s * 1000.0).collect();
    let x = stimulus::short_bursts(
        1000.0,
        effective_threshold_dbfs(vas) + BURST_ABOVE_DB,
        h + 1.0,
        &bursts_ms,
        h + 1.0,
        fs,
    );
    (x, bursts_s)
}

/// 003 FR-009: onset time. After a pause, bursts shorter than the onset leave no splice and no
/// output; longer bursts resume, and the kept region (burst − onset, then the hang time of
/// silence) is within ±1 ms.
pub fn check_fr009_onset(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let (x, bursts) = onset_stimulus(&case.vas, fs);
    let r = run(make, settings, &x);
    let o = onset_s(&case.vas);
    let h = hang_s(&case.vas);
    let long: Vec<f64> = bursts.iter().copied().filter(|&b| b > o).collect();
    let mut out = vec![MeasurementResult::new(
        "003/FR-009",
        "splice count (one per burst longer than the onset)",
        T_A024,
        rate,
        CONFIG,
        format!(
            "{}: bursts {} s",
            case.label,
            bursts
                .iter()
                .map(|b| format!("{b:.4}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        r.events.len() as f64,
        "events",
        Tolerance::Exact(long.len() as f64), // 003 FR-009: one splice per long burst
    )];
    // The output before the first splice is the hang time kept from the initial silence.
    let regions = r.regions();
    out.push(MeasurementResult::new(
        "003/FR-009",
        "output before the first resume (initial hang time)",
        T_A025,
        rate,
        CONFIG,
        format!("{}: gap H + 1 s first", case.label),
        regions[0] as f64,
        "samples",
        ms_tolerance(h * fs, fs),
    ));
    if r.events.len() == long.len() {
        for (b, kept) in long.iter().zip(regions.iter().skip(1)) {
            out.push(MeasurementResult::new(
                "003/FR-009",
                "kept region − hang time (= burst − onset)",
                T_A024,
                rate,
                CONFIG,
                format!("{}: burst {:.4} s", case.label, b),
                *kept as f64 - h * fs,
                "samples",
                ms_tolerance((b - o) * fs, fs),
            ));
        }
    }
    out
}

/// 003 FR-010 (8 kHz, where the rate conversion is the identity): every kept output sample is
/// bit-identical to the VAS-bypassed output at its input index, which follows from the events'
/// removed lengths. Empty at other rates.
pub fn check_fr010_splices(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    if rate != 8000 {
        return Vec::new();
    }
    let settings = vas_only(rate, &case.vas);
    let fs = f64::from(rate);
    let h = hang_s(&case.vas);
    let x = burst_gap(
        &case.vas,
        &[
            (true, 0.7),
            (false, 2.0 * h + 0.5),
            (true, 0.3),
            (false, 3.0 * h),
            (true, 0.5),
        ],
        fs,
    );
    let r = run(make, settings, &x);
    let mut off = settings;
    off.vas.enabled = false;
    let reference = run(&|s| Pipeline::new(s).unwrap(), off, &x).y;
    let mut removed_before = 0usize;
    let mut next = 0usize;
    let mut mismatches = 0usize;
    for (n, &v) in r.y.iter().enumerate() {
        while next < r.events.len() && r.events[next].output_position as usize == n {
            removed_before += r.events[next].input_length as usize;
            next += 1;
        }
        if reference.get(n + removed_before).map(|w| w.to_bits()) != Some(v.to_bits()) {
            mismatches += 1;
        }
    }
    vec![
        MeasurementResult::new(
            "003/FR-010",
            "kept samples differing from the VAS-bypassed input",
            T_A024,
            rate,
            CONFIG,
            format!("{}: 3 bursts, 2 long gaps", case.label),
            mismatches as f64,
            "samples",
            Tolerance::Exact(0.0), // 003 FR-010: bit-identical
        ),
        MeasurementResult::new(
            "003/FR-010",
            "splice count",
            T_A024,
            rate,
            CONFIG,
            format!("{}: 3 bursts, 2 long gaps", case.label),
            r.events.len() as f64,
            "events",
            Tolerance::Exact(2.0), // two long gaps, two splices (003 FR-010 stimulus)
        ),
    ]
}

/// 003 FR-011: the VAS adds no latency, for every tap.
pub fn check_fr011_latency(rate: u32) -> Vec<MeasurementResult> {
    [
        Tap::AfterAgc,
        Tap::AfterRecord,
        Tap::AfterVas,
        Tap::AfterPlayback,
    ]
    .into_iter()
    .map(|tap| {
        let mut on = Settings::new(rate);
        on.tap = tap;
        let mut off = on;
        off.vas.enabled = false;
        let diff = f64::from(Pipeline::new(on).unwrap().latency_samples())
            - f64::from(Pipeline::new(off).unwrap().latency_samples());
        MeasurementResult::new(
            "003/FR-011",
            "latency with VAS − latency without",
            T_ENG,
            rate,
            "default_vas",
            format!("{tap:?}"),
            diff,
            "samples",
            Tolerance::Exact(0.0), // 003 FR-011: no added latency
        )
    })
    .collect()
}

/// 003 R-11 self-test: the kept lengths measured at `rate` agree with the 8 kHz measurement
/// within the FR-008/FR-009 tolerances (±1 ms): the hang kept gap (gap 3 H) and the longest
/// onset burst's kept region.
pub fn check_rate_self_test(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let h = hang_s(&case.vas);
    let kept_gap_s = |r: u32| {
        let fs = f64::from(r);
        let (run, _) = hang_run(make, vas_only(r, &case.vas), 3.0 * h);
        kept_gap_samples(&run, 3.0 * h, &case.vas, fs) / fs
    };
    let last_region_s = |r: u32| {
        let fs = f64::from(r);
        let (x, _) = onset_stimulus(&case.vas, fs);
        let run = run(make, vas_only(r, &case.vas), &x);
        run.regions().last().copied().unwrap_or(0) as f64 / fs
    };
    let stim = || format!("{}: host rate vs 8 kHz", case.label);
    vec![
        MeasurementResult::new(
            "003/R-11",
            "hang kept gap: rate − 8 kHz",
            T_ENG,
            rate,
            CONFIG,
            stim(),
            (kept_gap_s(rate) - kept_gap_s(8000)) * 1000.0,
            "ms",
            Tolerance::Range(-1.0, 1.0), // engineering target (003 FR-017)
        ),
        MeasurementResult::new(
            "003/R-11",
            "onset kept region: rate − 8 kHz",
            T_ENG,
            rate,
            CONFIG,
            stim(),
            (last_region_s(rate) - last_region_s(8000)) * 1000.0,
            "ms",
            Tolerance::Range(-1.0, 1.0), // engineering target (003 FR-017)
        ),
    ]
}

/// The US3 AS3 floor-sweep stimulus at `fs` and `floor_dbfs`: three groups of four 1 kHz
/// bursts at −20 dBFS (300 ms on, 150 ms between bursts), the groups separated by 5 s gaps,
/// over band-limited noise at the floor throughout; a 0.5 s lead (under the hang time, so it
/// never pauses) and a 1 s tail (engineering targets, FR-017).
pub fn interplay_stimulus(fs: f64, floor_dbfs: f64) -> Vec<f32> {
    let mut segments = vec![(false, 0.5)];
    for g in 0..3 {
        for b in 0..4 {
            segments.push((true, 0.3));
            if b < 3 {
                segments.push((false, 0.15));
            }
        }
        segments.push((false, if g < 2 { 5.0 } else { 1.0 }));
    }
    let bursts = stimulus::burst_gap(1000.0, -20.0, &segments, fs);
    let noise = stimulus::bandlimited_noise(0x0D60, bursts.len(), fs, floor_dbfs);
    bursts.iter().zip(&noise).map(|(b, n)| b + n).collect()
}

/// The input-referred VAS threshold implied by spec 002's static curve (A-017, A-022): the
/// post-AGC threshold mapped back through the regulation line, or through the maximum gain
/// below the regulated range.
pub fn input_referred_threshold_dbfs(settings: &Settings) -> f64 {
    let t_out = effective_threshold_dbfs(&settings.vas);
    let target = f64::from(settings.agc.target_dbfs);
    let max_gain = f64::from(settings.agc.max_gain_db);
    let range_low_out = target - max_gain / 0.9 + max_gain;
    if t_out >= range_low_out {
        target + 10.0 * (t_out - target)
    } else {
        t_out - max_gain
    }
}

/// US3 AS3 (report only, no tolerance): the floor sweep through the default pipeline. Returns
/// lines for the report: the kept part of each 5 s gap at each floor (from the splices'
/// removed lengths, R-11), and the input-referred threshold at each sensitivity level.
pub fn interplay_report(rate: u32, make: Make<'_>) -> Vec<String> {
    let fs = f64::from(rate);
    let settings = configs::settings("default_vas", rate);
    let o = onset_s(&settings.vas);
    let mut lines = vec![format!(
        "US3 AS3 interplay at {rate} Hz (default pipeline: AGC + VAS); report only, no tolerance"
    )];
    for floor in [-40.0, -50.0, -60.0, -70.0] {
        let x = interplay_stimulus(fs, floor);
        let r = run(make, settings, &x);
        let gaps: Vec<String> = r
            .events
            .iter()
            .map(|e| format!("{:.3} s kept", 5.0 - e.input_length as f64 / fs + o))
            .collect();
        lines.push(format!(
            "  floor {floor:>4} dBFS RMS: {} splice(s) in two 5 s gaps: {}",
            r.events.len(),
            if gaps.is_empty() {
                "both kept in full (never paused)".to_string()
            } else {
                gaps.join(", ")
            }
        ));
    }
    lines.push("  input-referred threshold (spec 002 static curve, A-017; A-022):".to_string());
    for level in 1..=5u8 {
        let mut s = settings;
        s.vas.sensitivity = level;
        lines.push(format!(
            "    sensitivity {level}: post-AGC {:>5.1} dBFS -> input {:>6.1} dBFS peak",
            effective_threshold_dbfs(&s.vas),
            input_referred_threshold_dbfs(&s)
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_thresholds_and_rounding() {
        assert_eq!(matrix().len(), 11);
        assert!(matrix()[0].is_default() && !matrix()[1].is_default());
        let d = VasSettings::DEVICE;
        assert_eq!(effective_threshold_dbfs(&d), -18.0);
        let mut v = d;
        v.sensitivity = 1;
        assert_eq!(effective_threshold_dbfs(&v), -12.0);
        v.sensitivity = 5;
        assert_eq!(effective_threshold_dbfs(&v), -24.0);
        assert_eq!(hang_s(&d), 1.0);
        assert_eq!(onset_s(&d), 0.02);
        let s = Settings::new(48_000);
        assert!((input_referred_threshold_dbfs(&s) + 58.0).abs() < 1e-9);
        let mut s1 = s;
        s1.vas.sensitivity = 1;
        assert!((input_referred_threshold_dbfs(&s1) + 30.0).abs() < 1e-9);
        let (x, bursts) = onset_stimulus(&d, 8000.0);
        for (got, want) in bursts.iter().zip([0.01, 0.018, 0.03, 1.0]) {
            assert!((got - want).abs() < 1e-12, "{got} vs {want}");
        }
        assert_eq!(x.len(), 16_000 + 80 + 144 + 240 + 8000 + 4 * 16_000);
    }
}
