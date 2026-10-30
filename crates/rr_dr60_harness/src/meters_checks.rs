//! Spec 016 (the per-block meters): the oracles the read-out is checked against (research
//! R-06 – R-08, R-10) and the row renderer the golden, the example and the C smoke share.
//!
//! Every oracle is computed outside the engine: the tap outputs (the peaks), a limiter-bypassed
//! twin (the gain reduction), the `limiter_state` hook (the charge), clamp-bypassed twins (the
//! counts), a reference gate written from spec 003's data model (the voice activation), and
//! the `stage4_trace` hook for the device-rate stream at rates other than 6 kHz.

use rr_dr60::{__test_hooks, BlockInfo, Meters, Pipeline, Settings, Tap, VasEvent};

use crate::checks::{MeasurementResult, Tolerance};
use crate::{analysis, configs, split_checks, stimulus};

/// The output's narrowing, mirrored by its documented rules (spec 001 Edge Cases; spec 002
/// R-07): round to nearest even, subnormals to 0, beyond the range to ±`f32::MAX`.
pub fn narrow_mirror(x: f64) -> f32 {
    let v = x as f32;
    if v.is_subnormal() {
        0.0
    } else if v == f32::INFINITY {
        f32::MAX
    } else if v == f32::NEG_INFINITY {
        f32::MIN
    } else {
        v
    }
}

/// The largest absolute value of `samples` (0 for none): exact, since every sample is an f32.
pub fn peak_of(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |p, &v| p.max(v.abs()))
}

/// The number of samples at which `a` and `b` differ (a clamp count's oracle, FR-007).
pub fn clamp_diff_count(a: &[f32], b: &[f32]) -> u32 {
    assert_eq!(a.len(), b.len(), "the twins produced different lengths");
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count() as u32
}

/// The deepest attenuation of `out` against `reference`, in non-negative dB, over the samples
/// whose reference magnitude is at least −60 dBFS (research R-07); also the count of
/// qualifying samples and whether the last sample qualifies (the block-length rule).
pub fn gain_ratio_db(out: &[f32], reference: &[f32]) -> (f64, usize, bool) {
    assert_eq!(out.len(), reference.len());
    let floor = analysis::from_db(-60.0);
    let mut deepest = 0.0f64;
    let mut n = 0;
    for (&y, &x) in out.iter().zip(reference) {
        let x = f64::from(x).abs();
        if x >= floor {
            n += 1;
            let r = -analysis::db(f64::from(y).abs() / x);
            if r > deepest {
                deepest = r;
            }
        }
    }
    let last_qualifies = reference
        .last()
        .is_some_and(|&x| f64::from(x).abs() >= floor);
    (deepest, n, last_qualifies)
}

/// One row of the read-out as the golden, the example and the C smoke print it: the f32s as
/// 8-digit hex bit patterns, the integers decimal, the state 0/1, space-separated.
pub fn row(m: &Meters) -> String {
    format!(
        "{:08x} {:08x} {:08x} {:08x} {:08x} {} {} {} {:08x} {:08x} {}",
        m.input_peak.to_bits(),
        m.limiter_input_peak.to_bits(),
        m.gain_reduction_db.to_bits(),
        m.hold_charge.to_bits(),
        m.record_filter_peak.to_bits(),
        m.adc_clamp_count,
        u8::from(m.vas_paused),
        m.hang_remaining_samples,
        m.reconstruction_peak.to_bits(),
        m.route_peak.to_bits(),
        m.output_clamp_count,
    )
}

/// The voice activation of spec 003's data model › Per-sample processing, written from the
/// pseudo-code (not from the engine): the harness's own gate.
#[derive(Clone, Debug)]
pub struct ReferenceGate {
    threshold: f64,
    hang: u64,
    onset: u64,
    w: u64,
    paused: bool,
    silent: u64,
    run: u64,
    since_sound: u64,
    dropped: u64,
}

impl ReferenceGate {
    /// A gate for the settings' threshold (A-022: `threshold_dbfs + 3·(3 − sensitivity)` dB),
    /// hang and onset (whole device samples, rounded), bridging window W = 32 (A-024).
    pub fn new(s: &Settings) -> Self {
        let level_db = f64::from(s.vas.threshold_dbfs) + 3.0 * (3.0 - f64::from(s.vas.sensitivity));
        let samples = |ms: f32| (f64::from(ms) * 6.0 + 0.5) as u64;
        Self {
            threshold: analysis::from_db(level_db),
            hang: samples(s.vas.hang_ms),
            onset: samples(s.vas.onset_ms),
            w: 32,
            paused: false,
            silent: 0,
            run: 0,
            since_sound: 33,
            dropped: 0,
        }
    }

    /// One device sample; returns whether it is kept.
    pub fn push(&mut self, x: f64) -> bool {
        let sound = x.abs() >= self.threshold;
        if !self.paused {
            if sound {
                self.silent = 0;
            } else {
                self.silent += 1;
            }
            if self.silent <= self.hang {
                true
            } else {
                self.paused = true;
                self.run = 0;
                self.since_sound = self.w + 1;
                self.dropped += 1;
                false
            }
        } else {
            if sound {
                self.run = if self.run > 0 && self.since_sound <= self.w {
                    self.run + 1
                } else {
                    1
                };
                self.since_sound = 0;
            } else {
                self.since_sound = (self.since_sound + 1).min(self.w + 1);
                if self.since_sound > self.w {
                    self.run = 0;
                } else if self.run > 0 {
                    self.run += 1;
                }
            }
            if sound && self.run > self.onset {
                self.paused = false;
                self.silent = 0;
                true
            } else {
                self.dropped += 1;
                false
            }
        }
    }

    /// The gate is paused.
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// FR-002 (h): `H − silent` while recording, 0 while paused.
    pub fn hang_remaining(&self) -> u64 {
        if self.paused {
            0
        } else {
            self.hang.saturating_sub(self.silent)
        }
    }

    /// The cumulative count of dropped device samples D (the event corroboration, R-08).
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// One block of a one-pass run: the output, the block report, the read-out and the events.
#[derive(Clone, Debug)]
pub struct Block {
    /// The block's output (`produced` samples).
    pub output: Vec<f32>,
    /// The block report.
    pub info: BlockInfo,
    /// The read-out after the call.
    pub meters: Meters,
    /// The events the call reported.
    pub events: Vec<VasEvent>,
}

/// Runs `x` through `p` in `block`-sample calls and collects every block.
pub fn one_pass_blocks(p: &mut Pipeline, x: &[f32], block: usize) -> Vec<Block> {
    let mut out = Vec::new();
    let mut ev = vec![VasEvent::default(); p.max_events(block)];
    for chunk in x.chunks(block) {
        let mut y = vec![0.0f32; chunk.len()];
        let info = p
            .process_with_events(chunk, &mut y, &mut ev)
            .expect("a valid call");
        y.truncate(info.produced);
        out.push(Block {
            output: y,
            info,
            meters: p.meters(),
            events: ev[..info.events.min(ev.len())].to_vec(),
        });
    }
    out
}

/// Stage 4's device-rate stream before the clamp for `x` through a twin of `settings`
/// (research R-06), per host block: the samples due in each block.
pub fn stage4_stream(settings: Settings, x: &[f32], block: usize) -> Vec<Vec<f64>> {
    let mut p = Pipeline::new(settings).expect("a valid configuration");
    x.chunks(block)
        .map(|chunk| {
            chunk
                .iter()
                .filter_map(|&v| __test_hooks::stage4_trace(&mut p, v))
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn result(
    requirement: &'static str,
    property: &'static str,
    rate: u32,
    config: &'static str,
    stimulus: impl Into<String>,
    measured: f64,
    unit: &'static str,
    tolerance: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(
        requirement,
        property,
        &["engineering target"],
        rate,
        config,
        stimulus,
        measured,
        unit,
        tolerance,
    )
}

/// The one-pass block size the story tests and the golden use (research R-10).
pub const ONE_PASS_BLOCK: usize = 1024;

/// US1 AS1 (FR-005): at the taps after the host-rate points every peak equals the largest
/// absolute value of the block's output; the input peak equals the sanitized input's; the
/// record-filter peak equals the `stage4_trace` stream's (and, at 6 kHz, the `AfterRecord`
/// output's with the clamp off). The count of mismatching blocks is the figure (0 passes).
pub fn host_rate_peaks(rate: u32) -> Vec<MeasurementResult> {
    let f = f64::from(rate);
    let mut x = stimulus::voiced(f, 0.5, -20.0, 0x0D60);
    // Non-finite samples are sanitized to 0 before the input peak is read (Edge Cases).
    x[7] = f32::NAN;
    x[9] = f32::INFINITY;
    x[11] = f32::NEG_INFINITY;
    let sanitized: Vec<f32> = x
        .iter()
        .map(|&v| {
            if v.is_finite() && !v.is_subnormal() {
                v
            } else {
                0.0
            }
        })
        .collect();
    let mut rows = Vec::new();
    type Read = fn(&Meters) -> f32;
    let cases: [(&str, Tap, Read); 3] = [
        (
            "limiter-input peak == AfterMicAmp output",
            Tap::AfterMicAmp,
            |m| m.limiter_input_peak,
        ),
        (
            "reconstruction peak == AfterPlayback output",
            Tap::AfterPlayback,
            |m| m.reconstruction_peak,
        ),
        (
            "route peak == RouteOutput output (clamp off)",
            Tap::RouteOutput,
            |m| m.route_peak,
        ),
    ];
    for (property, tap, read) in cases {
        let mut s = Settings::new(rate); // the 0.14 default, stages 1 – 2 on
        s.tap = tap;
        s.output.clamp_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let blocks = one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK);
        let mismatches = blocks
            .iter()
            .filter(|b| read(&b.meters).to_bits() != peak_of(&b.output).to_bits())
            .count();
        rows.push(result(
            "FR-005",
            property,
            rate,
            "default",
            "voiced −20 dBFS, 0.5 s, 1024 blocks",
            mismatches as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ));
        let input_mismatches = blocks
            .iter()
            .zip(sanitized.chunks(ONE_PASS_BLOCK))
            .filter(|(b, chunk)| b.meters.input_peak.to_bits() != peak_of(chunk).to_bits())
            .count();
        rows.push(result(
            "FR-005",
            "input peak == sanitized input",
            rate,
            "default",
            format!("voiced −20 dBFS at {tap:?}"),
            input_mismatches as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ));
    }
    // The record-filter peak against the device-rate stream of a twin (the hook), clamp on.
    let s = Settings::new(rate);
    let mut p = Pipeline::new(s).unwrap();
    let blocks = one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK);
    let stream = stage4_stream(s, &x, ONE_PASS_BLOCK);
    let mismatches = blocks
        .iter()
        .zip(&stream)
        .filter(|(b, d)| {
            let want = narrow_mirror(d.iter().fold(0.0f64, |m, &v| m.max(v.abs())));
            b.meters.record_filter_peak.to_bits() != want.to_bits()
        })
        .count();
    rows.push(result(
        "FR-005",
        "record-filter peak == stage4_trace stream",
        rate,
        "default",
        "voiced −20 dBFS, 0.5 s, 1024 blocks",
        mismatches as f64,
        "blocks",
        Tolerance::Exact(0.0),
    ));
    if rate == 6000 {
        let mut s = Settings::new(rate);
        s.tap = Tap::AfterRecord;
        s.adc_clamp_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let blocks = one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK);
        let mismatches = blocks
            .iter()
            .filter(|b| b.meters.record_filter_peak.to_bits() != peak_of(&b.output).to_bits())
            .count();
        rows.push(result(
            "FR-005",
            "record-filter peak == AfterRecord output at 6 kHz (clamp off)",
            rate,
            "default_clamp_off",
            "voiced −20 dBFS, 0.5 s, 1024 blocks",
            mismatches as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ));
    }
    rows
}

/// The per-rate steady figures of the limiter model's "## Charge (spec 016)" print (T003,
/// 2026-10-30): the steady mean gain in dB and the steady charge ratio on a 1 kHz tone 10 dB
/// over the knee.
pub fn steady_print(rate: u32) -> (f64, f64) {
    match rate {
        6000 => (9.9933, 0.84404),
        8000 => (9.9930, 0.84404),
        16000 => (9.9887, 0.84401),
        44100 => (9.9542, 0.84377),
        48000 => (9.9517, 0.84376),
        88200 => (9.9512, 0.84375),
        96000 => (9.9521, 0.84376),
        other => panic!("no print for {other} Hz"),
    }
}

/// The limiter model's `v_limit` print (T003): the charge's denominator (research R-07).
pub const V_LIMIT_PRINT: f64 = 1.27538;

/// US1 AS2 (FR-006): the gain reduction against the ratio of the `AfterAgc` output to a
/// limiter-bypassed twin's, per block, onset blocks included; the steady figures against the
/// model's print; the charge against the `limiter_state` hook; the block-length rule asserted.
pub fn limiter_figures(rate: u32, seconds: f64, block: usize) -> Vec<MeasurementResult> {
    let f = f64::from(rate);
    let x = stimulus::tone_at_phase(
        1000.0,
        9.0, // 10 dB over the −1 dBFS knee (A-028)
        stimulus::peak_phase(1000.0, rate),
        f,
        (seconds * f) as usize,
    );
    let mut on = configs::settings_011(rate); // stages 1 – 2 bypassed
    on.tap = Tap::AfterAgc;
    let mut off = on;
    off.agc.enabled = false;
    let mut p_on = Pipeline::new(on).unwrap();
    let mut p_off = Pipeline::new(off).unwrap();
    let blocks_on = one_pass_blocks(&mut p_on, &x, block);
    let blocks_off = one_pass_blocks(&mut p_off, &x, block);
    // A second engine with the hook, in lockstep, for the charge (the hook needs `&Pipeline`;
    // `p_on` is that engine, re-run block by block).
    let mut p_hook = Pipeline::new(on).unwrap();
    let mut worst_db = 0.0f64;
    let mut worst_charge = 0.0f64;
    let mut unqualified_ends = 0usize;
    let mut deepest_last_second = 0.0f64;
    let mut charge_last_second = 0.0f64;
    let last_second_from = x.len().saturating_sub(f as usize);
    let mut pos = 0;
    for (b_on, b_off) in blocks_on.iter().zip(&blocks_off) {
        let (ratio_db, _n, last_qualifies) = gain_ratio_db(&b_on.output, &b_off.output);
        if !last_qualifies {
            unqualified_ends += 1;
        }
        let d = (f64::from(b_on.meters.gain_reduction_db) - ratio_db).abs();
        if d > worst_db {
            worst_db = d;
        }
        let mut y = vec![0.0f32; b_on.output.len()];
        let _ = p_hook
            .process(&x[pos..pos + b_on.output.len()], &mut y)
            .unwrap();
        let want = __test_hooks::limiter_state(&p_hook) / V_LIMIT_PRINT;
        let dc = (f64::from(b_on.meters.hold_charge) - want).abs();
        if dc > worst_charge {
            worst_charge = dc;
        }
        if pos >= last_second_from {
            deepest_last_second = deepest_last_second.max(f64::from(b_on.meters.gain_reduction_db));
            charge_last_second = f64::from(b_on.meters.hold_charge);
        }
        pos += b_on.output.len();
    }
    let (steady_db, steady_ratio) = steady_print(rate);
    let stim = format!("1 kHz +9 dBFS, {seconds} s, {block}-sample blocks");
    vec![
        result(
            "FR-006",
            "block ends that do not qualify (block-length rule)",
            rate,
            "limiter_011_after_agc",
            stim.clone(),
            unqualified_ends as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-006",
            "gain reduction vs the bypassed-twin ratio, worst block",
            rate,
            "limiter_011_after_agc",
            stim.clone(),
            worst_db,
            "dB",
            Tolerance::AtMost(0.01),
        ),
        result(
            "FR-006",
            "steady gain reduction (last second's deepest) vs the model's print",
            rate,
            "limiter_011_after_agc",
            stim.clone(),
            deepest_last_second - steady_db,
            "dB",
            Tolerance::Range(-0.1, 0.1),
        ),
        result(
            "FR-006",
            "hold charge vs limiter_state / v_limit, worst block",
            rate,
            "limiter_011_after_agc",
            stim.clone(),
            worst_charge,
            "",
            Tolerance::AtMost(1e-3),
        ),
        result(
            "FR-006",
            "steady charge ratio vs the model's print",
            rate,
            "limiter_011_after_agc",
            stim,
            (charge_last_second - steady_ratio) / steady_ratio,
            "fraction",
            Tolerance::Range(-0.01, 0.01),
        ),
    ]
}

/// The ±1.0 square of the C smoke's clamp stimulus (24 up, 24 down), `seconds` long at `rate`:
/// exactly representable, the C smoke builds the same pattern (research R-10).
pub fn clamp_square(rate: u32, seconds: f64) -> Vec<f32> {
    let n = (seconds * f64::from(rate)) as usize;
    (0..n)
        .map(|i| if i % 48 < 24 { 1.0 } else { -1.0 })
        .collect()
}

/// US1 AS3 (FR-007): the converter-clamp count against the differing samples of a clamp-off
/// twin at 6 kHz (the `AfterRecord` tap) and against the `stage4_trace` stream's samples
/// beyond full scale at every rate; the record-filter peak above 1.0 on every counting block.
pub fn adc_clamp_counts(rate: u32) -> Vec<MeasurementResult> {
    let x = clamp_square(rate, 0.5);
    let mut on = configs::settings_011(rate);
    on.agc.enabled = false; // stages 1 – 3 bypassed: the square's edges overshoot the filter
    on.tap = Tap::AfterRecord;
    let mut p = Pipeline::new(on).unwrap();
    let blocks = one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK);
    let stream = stage4_stream(on, &x, ONE_PASS_BLOCK);
    let mut mismatches = 0;
    let mut peak_not_above = 0;
    let mut counting = 0;
    for (b, d) in blocks.iter().zip(&stream) {
        let want = d.iter().filter(|v| v.abs() > 1.0).count() as u32;
        if b.meters.adc_clamp_count != want {
            mismatches += 1;
        }
        if want > 0 {
            counting += 1;
            if b.meters.record_filter_peak <= 1.0 {
                peak_not_above += 1;
            }
        }
    }
    let stim = "±1.0 square 24/24, 0.5 s, 1024 blocks";
    let mut rows = vec![
        result(
            "FR-007",
            "ADC-clamp count vs stage4_trace samples beyond full scale",
            rate,
            "clamp_011_no_agc",
            stim,
            mismatches as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-007",
            "blocks with a count",
            rate,
            "clamp_011_no_agc",
            stim,
            counting as f64,
            "blocks",
            Tolerance::AtLeast(1.0),
        ),
        result(
            "FR-007",
            "record-filter peak above 1.0 on every counting block (misses)",
            rate,
            "clamp_011_no_agc",
            stim,
            peak_not_above as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
    ];
    if rate == 6000 {
        let mut off = on;
        off.adc_clamp_enabled = false;
        let mut p_off = Pipeline::new(off).unwrap();
        let twin = one_pass_blocks(&mut p_off, &x, ONE_PASS_BLOCK);
        let mismatches = blocks
            .iter()
            .zip(&twin)
            .filter(|(a, b)| a.meters.adc_clamp_count != clamp_diff_count(&a.output, &b.output))
            .count();
        rows.push(result(
            "FR-007",
            "ADC-clamp count vs the clamp-off twin's differing samples (6 kHz)",
            rate,
            "clamp_011_no_agc",
            stim,
            mismatches as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ));
    }
    rows
}

/// Records `x` into `p` (in `RECORD_BLOCK` calls), then plays it back in `play_block` calls,
/// collecting each play call's output and read-out until the engine reports the end.
pub fn record_then_play_blocks(
    p: &mut Pipeline,
    x: &[f32],
    play_block: usize,
) -> (Vec<(Vec<f32>, Meters)>, Vec<Meters>) {
    p.start_recording().expect("idle");
    let mut ev = vec![VasEvent::default(); p.max_record_events(split_checks::RECORD_BLOCK)];
    let mut record_meters = Vec::new();
    for chunk in x.chunks(split_checks::RECORD_BLOCK) {
        let info = p.record(chunk, &mut ev).expect("recording");
        assert!(info.stopped.is_none(), "memory full");
        record_meters.push(p.meters());
    }
    let _ = p.stop_recording(&mut ev).expect("recording");
    record_meters.push(p.meters()); // the flush's
    let number = p.memory_status().count;
    p.start_playback(number).expect("a recording");
    let mut plays = Vec::new();
    loop {
        let mut y = vec![0.0f32; play_block];
        let info = p.play(&mut y).expect("playing");
        y.truncate(info.produced);
        plays.push((y, p.meters()));
        if info.finished {
            break;
        }
    }
    (plays, record_meters)
}

/// US1 AS4 (FR-007): the output-clamp count against the differing samples of a clamp-off twin's
/// playback, call for call; the route peak equal between the twins (read before the bound).
pub fn output_clamp_counts(rate: u32) -> Vec<MeasurementResult> {
    let x = clamp_square(rate, 0.5);
    let mut on = configs::settings_011(rate);
    on.speech_codec_enabled = false;
    on.output.clamp_enabled = true;
    let mut off = on;
    off.output.clamp_enabled = false;
    let mut p_on = Pipeline::new(on).unwrap();
    let mut p_off = Pipeline::new(off).unwrap();
    p_on.set_volume(1.0).unwrap();
    p_off.set_volume(1.0).unwrap();
    let (plays_on, _) = record_then_play_blocks(&mut p_on, &x, split_checks::PLAY_BLOCK);
    let (plays_off, _) = record_then_play_blocks(&mut p_off, &x, split_checks::PLAY_BLOCK);
    assert_eq!(plays_on.len(), plays_off.len());
    let mut count_mismatches = 0;
    let mut peak_mismatches = 0;
    let mut counting = 0;
    for ((y_on, m_on), (y_off, m_off)) in plays_on.iter().zip(&plays_off) {
        let want = clamp_diff_count(y_on, y_off);
        if m_on.output_clamp_count != want {
            count_mismatches += 1;
        }
        if want > 0 {
            counting += 1;
        }
        if m_on.route_peak.to_bits() != m_off.route_peak.to_bits() {
            peak_mismatches += 1;
        }
        if m_off.output_clamp_count != 0 {
            count_mismatches += 1;
        }
    }
    let stim = "±1.0 square recorded, played at position 1, speaker";
    vec![
        result(
            "FR-007",
            "output-clamp count vs the clamp-off twin's differing samples",
            rate,
            "default_011_clamp",
            stim,
            count_mismatches as f64,
            "calls",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-007",
            "play calls with a count",
            rate,
            "default_011_clamp",
            stim,
            counting as f64,
            "calls",
            Tolerance::AtLeast(1.0),
        ),
        result(
            "FR-003",
            "route peak equal between the twins (read before the bound)",
            rate,
            "default_011_clamp",
            stim,
            peak_mismatches as f64,
            "calls",
            Tolerance::Exact(0.0),
        ),
    ]
}

/// The `vas_burst` stimulus (research R-10): 0.1 s of 1 kHz at −20 dBFS, 0.25 s of silence,
/// 0.15 s of the tone.
pub fn vas_burst(rate: u32) -> Vec<f32> {
    stimulus::burst_gap(
        1000.0,
        -20.0,
        &[(true, 0.1), (false, 0.25), (true, 0.15)],
        f64::from(rate),
    )
}

/// The short-hang configuration (research R-10): `settings_011` with the gate at a 50 ms hang
/// and a 20 ms onset, drop or mute mode, so a 0.5 s stimulus pauses and resumes.
pub fn short_hang(rate: u32, mute: bool) -> Settings {
    configs::settings(
        if mute {
            "meters_short_hang_mute"
        } else {
            "meters_short_hang"
        },
        rate,
    )
}

/// US1 AS5 (FR-008): the state and the hang remaining against the reference gate per block,
/// the state against the block report, the drop count against the resume events (host output
/// units in the one-pass by the cumulative-floor rule, device units in the recorder), and in
/// mute mode the region's start within the block whose read-out first read paused.
pub fn vas_figures(rate: u32, mute: bool) -> Vec<MeasurementResult> {
    let x = vas_burst(rate);
    let s = short_hang(rate, mute);
    let config: &'static str = if mute {
        "meters_short_hang_mute"
    } else {
        "meters_short_hang"
    };
    let block = 121;
    // The one-pass.
    let mut p = Pipeline::new(s).unwrap();
    let blocks = one_pass_blocks(&mut p, &x, block);
    let stream = stage4_stream(s, &x, block);
    let (l, m) = split_checks::l_m(rate);
    let mut gate = ReferenceGate::new(&s);
    let (mut state_mismatch, mut hang_mismatch, mut report_mismatch, mut event_mismatch) =
        (0, 0, 0, 0);
    let mut paused_blocks = 0;
    let mut d_prev = 0u64;
    let mut produced_total = 0u64;
    let mut first_paused_block_output: Option<(u64, u64)> = None;
    let mut resumes = 0;
    for (b, d) in blocks.iter().zip(&stream) {
        for &v in d {
            gate.push(v.clamp(-1.0, 1.0));
        }
        if b.meters.vas_paused != gate.paused() {
            state_mismatch += 1;
        }
        if b.meters.hang_remaining_samples != gate.hang_remaining() {
            hang_mismatch += 1;
        }
        if b.meters.vas_paused != b.info.paused {
            report_mismatch += 1;
        }
        if b.meters.vas_paused {
            paused_blocks += 1;
            if first_paused_block_output.is_none() {
                first_paused_block_output =
                    Some((produced_total, produced_total + b.output.len() as u64));
            }
        }
        for e in &b.events {
            if mute {
                // The region's start lies in the host output samples of the first paused block.
                let (lo, hi) = first_paused_block_output.unwrap_or((u64::MAX, u64::MAX));
                if e.output_position < lo || e.output_position >= hi {
                    event_mismatch += 1;
                }
            } else {
                resumes += 1;
                let dropped = gate.dropped();
                let want = dropped * m / l - d_prev * m / l;
                if e.input_length != want {
                    event_mismatch += 1;
                }
                d_prev = dropped;
            }
        }
        produced_total += b.output.len() as u64;
    }
    let stim = format!("vas_burst (0.1 s on, 0.25 s off, 0.15 s on), {block}-sample blocks");
    let mut rows = vec![
        result(
            "FR-008",
            "state vs the reference gate (mismatching blocks)",
            rate,
            config,
            stim.clone(),
            state_mismatch as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-008",
            "hang remaining vs the reference gate (mismatching blocks)",
            rate,
            config,
            stim.clone(),
            hang_mismatch as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-008",
            "state vs BlockInfo.paused (mismatching blocks)",
            rate,
            config,
            stim.clone(),
            report_mismatch as f64,
            "blocks",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-008",
            "paused blocks",
            rate,
            config,
            stim.clone(),
            paused_blocks as f64,
            "blocks",
            Tolerance::AtLeast(1.0),
        ),
    ];
    if rate == 6000 && !mute {
        // One device sample per block at the identity rate: the block that keeps the last
        // non-sound sample reads 0 while recording, and the next block reads paused (AS5).
        let mut p = Pipeline::new(s).unwrap();
        let fine = one_pass_blocks(&mut p, &x, 1);
        let boundary = fine.windows(2).any(|w| {
            !w[0].meters.vas_paused
                && w[0].meters.hang_remaining_samples == 0
                && w[1].meters.vas_paused
                && w[1].meters.hang_remaining_samples == 0
        });
        rows.push(result(
            "FR-008",
            "6 kHz, 1-sample blocks: 0 while recording, then paused on the next sample",
            rate,
            config,
            "vas_burst, 1-sample blocks",
            f64::from(u8::from(boundary)),
            "",
            Tolerance::Exact(1.0),
        ));
    }
    if mute {
        rows.push(result(
            "FR-008",
            "mute: region starts outside the first paused block",
            rate,
            config,
            stim.clone(),
            event_mismatch as f64,
            "events",
            Tolerance::Exact(0.0),
        ));
    } else {
        rows.push(result(
            "FR-008",
            "drop: resume events whose length differs from the gate's drops",
            rate,
            config,
            stim.clone(),
            event_mismatch as f64,
            "events",
            Tolerance::Exact(0.0),
        ));
        rows.push(result(
            "FR-008",
            "drop: resume events",
            rate,
            config,
            stim.clone(),
            f64::from(resumes),
            "events",
            Tolerance::AtLeast(1.0),
        ));
    }
    // The recorder: device positions; the state against RecordInfo.paused; drop lengths D − D_prev.
    let mut s_rec = s;
    s_rec.speech_codec_enabled = false;
    let mut p = Pipeline::new(s_rec).unwrap();
    p.start_recording().unwrap();
    let stream = stage4_stream(s_rec, &x, block);
    let mut gate = ReferenceGate::new(&s_rec);
    let mut ev = vec![VasEvent::default(); p.max_record_events(block)];
    let (mut rec_state_mismatch, mut rec_event_mismatch) = (0, 0);
    let mut d_prev = 0u64;
    let mut kept_total = 0u64;
    let mut first_paused_kept: Option<(u64, u64)> = None;
    for (chunk, d) in x.chunks(block).zip(&stream) {
        let info = p.record(chunk, &mut ev).unwrap();
        for &v in d {
            gate.push(v.clamp(-1.0, 1.0));
        }
        let mm = p.meters();
        if mm.vas_paused != info.paused
            || mm.vas_paused != gate.paused()
            || mm.hang_remaining_samples != gate.hang_remaining()
        {
            rec_state_mismatch += 1;
        }
        if mm.vas_paused && first_paused_kept.is_none() {
            first_paused_kept = Some((kept_total, kept_total + info.stored as u64));
        }
        for e in &ev[..info.events] {
            if mute {
                let (lo, hi) = first_paused_kept.unwrap_or((u64::MAX, u64::MAX));
                if e.output_position < lo || e.output_position >= hi {
                    rec_event_mismatch += 1;
                }
            } else {
                let dropped = gate.dropped();
                if e.input_length != dropped - d_prev {
                    rec_event_mismatch += 1;
                }
                d_prev = dropped;
            }
        }
        kept_total += info.stored as u64;
    }
    let _ = p.stop_recording(&mut ev).unwrap();
    rows.push(result(
        "FR-008",
        "recorder: state and hang vs RecordInfo.paused and the gate (mismatching calls)",
        rate,
        config,
        stim.clone(),
        rec_state_mismatch as f64,
        "calls",
        Tolerance::Exact(0.0),
    ));
    rows.push(result(
        "FR-008",
        "recorder: events disagreeing with the gate (device units)",
        rate,
        config,
        stim,
        rec_event_mismatch as f64,
        "events",
        Tolerance::Exact(0.0),
    ));
    rows
}

/// US1 AS6 (FR-001, FR-004): a record call fills the record group and leaves the playback
/// group at rest; a play call the reverse; the flush's figures are readable after the stop, its
/// state equal to the flush report's; the drain's peaks fall toward rest with zero counts.
pub fn transport_groups(rate: u32) -> Vec<MeasurementResult> {
    let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x0D60);
    let s = Settings::new(rate);
    let mut p = Pipeline::new(s).unwrap();
    let rest = p.rest_meters();
    p.start_recording().unwrap();
    let mut ev = vec![VasEvent::default(); p.max_record_events(split_checks::RECORD_BLOCK)];
    let mut playback_group_not_rest = 0;
    let mut record_group_empty = 0;
    for chunk in x.chunks(split_checks::RECORD_BLOCK) {
        let _ = p.record(chunk, &mut ev).unwrap();
        let m = p.meters();
        if m.reconstruction_peak != 0.0 || m.route_peak != 0.0 || m.output_clamp_count != 0 {
            playback_group_not_rest += 1;
        }
        if m.input_peak == 0.0 {
            record_group_empty += 1;
        }
    }
    let before_stop = p.meters();
    let out = p.stop_recording(&mut ev).unwrap();
    let flush = p.meters();
    let flush_differs = flush != before_stop;
    let flush_state_agrees = flush.vas_paused == out.flush.paused;
    let number = p.memory_status().count;
    p.start_playback(number).unwrap();
    let after_start = p.meters();
    let mut record_group_not_rest = 0;
    let mut plays = 0;
    let mut drain_rows: Vec<Meters> = Vec::new();
    let pre_drain = split_checks::playback_len(p.recording_length(number).unwrap(), rate);
    let mut produced_total = 0u64;
    loop {
        let mut y = vec![0.0f32; split_checks::PLAY_BLOCK];
        let info = p.play(&mut y).unwrap();
        plays += 1;
        let m = p.meters();
        if m.input_peak != rest.input_peak
            || m.limiter_input_peak != 0.0
            || m.gain_reduction_db != 0.0
            || m.hold_charge != 0.0
            || m.record_filter_peak != 0.0
            || m.adc_clamp_count != 0
            || m.vas_paused
            || m.hang_remaining_samples != rest.hang_remaining_samples
        {
            record_group_not_rest += 1;
        }
        produced_total += info.produced as u64;
        if produced_total > pre_drain {
            drain_rows.push(m);
        }
        if info.finished {
            break;
        }
    }
    let drain_counts: u32 = drain_rows.iter().map(|m| m.output_clamp_count).sum();
    let drain_falls = drain_rows
        .windows(2)
        .all(|w| w[1].reconstruction_peak <= w[0].reconstruction_peak.max(1e-3));
    let stim = "voiced −20 dBFS, 0.5 s";
    vec![
        result(
            "FR-004",
            "record calls with the playback group not at rest",
            rate,
            "default",
            stim,
            playback_group_not_rest as f64,
            "calls",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-002",
            "record calls with an empty record group",
            rate,
            "default",
            stim,
            record_group_empty as f64,
            "calls",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-001",
            "the flush's read-out differs from the last record call's",
            rate,
            "default",
            stim,
            f64::from(u8::from(flush_differs)),
            "",
            Tolerance::Exact(1.0),
        ),
        result(
            "FR-002",
            "the flush's state equals the flush report's",
            rate,
            "default",
            stim,
            f64::from(u8::from(flush_state_agrees)),
            "",
            Tolerance::Exact(1.0),
        ),
        result(
            "FR-001",
            "start_playback leaves the read-out",
            rate,
            "default",
            stim,
            f64::from(u8::from(after_start == flush)),
            "",
            Tolerance::Exact(1.0),
        ),
        result(
            "FR-004",
            "play calls with the record group not at rest",
            rate,
            "default",
            stim,
            record_group_not_rest as f64,
            "calls",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-004",
            "play calls",
            rate,
            "default",
            stim,
            f64::from(plays),
            "calls",
            Tolerance::AtLeast(2.0),
        ),
        result(
            "FR-003",
            "drain: output-clamp counts (option off)",
            rate,
            "default",
            stim,
            f64::from(drain_counts),
            "samples",
            Tolerance::Exact(0.0),
        ),
        result(
            "FR-003",
            "drain: the reconstruction peak falls call by call",
            rate,
            "default",
            stim,
            f64::from(u8::from(drain_falls)),
            "",
            Tolerance::Exact(1.0),
        ),
        result(
            "FR-003",
            "drain calls",
            rate,
            "default",
            stim,
            drain_rows.len() as f64,
            "calls",
            Tolerance::AtLeast(1.0),
        ),
    ]
}
