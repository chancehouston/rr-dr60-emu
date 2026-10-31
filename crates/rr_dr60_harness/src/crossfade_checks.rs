//! Spec 019 crossfade checks (research R-06): one function per measurable property of the
//! speed-change crossfade, every oracle outside the engine: the harness reference
//! ([`speed_checks::reference_output_at`]) with the published banks and weights, the stepped
//! build (`SpeedMutation::fade_step`, version 0.18's switch) as the click reader's reference,
//! and the device-rate trace hook for the host rates whose output passes stage 10 and the
//! resampler. Every tolerance is an engineering target of spec 019 FR-001 – FR-008 and bounds
//! the engine's convention, not the device.

use rr_dr60::__test_hooks::{self, SpeedMutation};
use rr_dr60::{Pipeline, Route, Settings, Stream, speed};

use crate::analysis::{self, click_energy_in, db};
use crate::changes_checks::{self as cc, Change, Partition, Source};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::speed_checks::{self as sc, AMP, SCRIPT_STEPS, numerator};
use crate::{configs, stimulus};

/// Trace: an engine convention (A-054 amended; A-045's length).
pub const T_XFADE: &[&str] = &["A-054", "A-045", "engineering target"];

/// The crossfade's length in device samples.
pub fn n_fade() -> usize {
    speed::CROSSFADE_SAMPLES as usize
}

/// The level pairs the checks run (spec 013's step table's, less the 1 kHz-only rows).
pub const PAIRS: [(u8, u8); 8] = [
    (3, 5),
    (5, 3),
    (1, 5),
    (5, 1),
    (2, 4),
    (4, 2),
    (1, 3),
    (3, 1),
];

fn pass(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// The stepped build: version 0.18's switch (research R-05).
pub fn stepped(s: Settings) -> Pipeline {
    Pipeline::__with_speed_mutation(
        s,
        &SpeedMutation {
            fade_step: true,
            ..SpeedMutation::default()
        },
    )
    .unwrap()
}

/// A stored −6 dBFS tone of `len` device samples at `freq_hz`.
pub fn stored_tone(freq_hz: f64, len: usize) -> Vec<f32> {
    stimulus::stored_tone(freq_hz, AMP, len)
}

/// An engine at `settings` holding `x` as recording 1: recorded at 6 kHz in `bypass_all` (the
/// stored device samples are `x` followed by the stop's flush of silence) and imported
/// (`configs::stored_recording`, spec 006 FR-014: the same stored stream at every host rate).
pub fn recorded(make: Make<'_>, settings: Settings, x: &[f32]) -> Pipeline {
    let (p, number) = configs::stored_recording(make, settings, x);
    assert_eq!(number, 1);
    p
}

/// The stored stream of `x` as the recording holds it (`x` and the flush's silence): the
/// reference's input, read from a 6 kHz `speed_only` engine's level-3 playback.
pub fn stored_stream(x: &[f32]) -> Vec<f32> {
    let mut p = recorded(
        &crate::checks::standard,
        configs::settings("speed_only", 6000),
        x,
    );
    sc::stored_stream(&mut p, 1)
}

/// The count of bit-level mismatches between the engine's device outputs and the reference's,
/// the engine's trace allowed the drain's zeros past the reference's end (counted as mismatches
/// when not zero) and a shortfall counted sample for sample.
pub fn device_mismatches(got: &[f64], want: &[f32]) -> usize {
    let n = got.len().min(want.len());
    got[..n]
        .iter()
        .zip(&want[..n])
        .filter(|(g, w)| (**g as f32).to_bits() != w.to_bits())
        .count()
        + got[n..].iter().filter(|v| **v != 0.0).count()
        + want.len().saturating_sub(got.len())
}

/// The device-indexed request at output `s` of the engine's playback at 6 kHz in calls of one
/// host sample (one device sample each in `speed_only`): the host output.
pub fn play_6k(p: &mut Pipeline, requests: &[(usize, u8)]) -> Vec<f32> {
    sc::play(p, 1, 1, requests).out
}

// ---- FR-003: the constant-level identity -----------------------------------------------------

/// Spec 019 US1 AS1 (FR-003): at every level and both speed configurations the playback equals
/// the stepped build's sample for sample (no fade state is entered at a constant level).
pub fn constant_levels(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let x = stimulus::voiced(6000.0, 1.0, -20.0, 0x019);
    let mut rows = Vec::new();
    for config in ["speed_only", "speed_only_codec"] {
        let settings = configs::settings(config, rate);
        for level in 1..=5u8 {
            let mut a = recorded(make, settings, &x);
            let mut b = recorded(&stepped, settings, &x);
            let ya = sc::play(&mut a, 1, 480, &[(0, level)]).out;
            let yb = sc::play(&mut b, 1, 480, &[(0, level)]).out;
            rows.push(MeasurementResult::new(
                "FR-003",
                "constant level bit-identical to the stepped build (mismatches)",
                T_XFADE,
                rate,
                config,
                format!("voiced -20 dBFS at level {level}"),
                sc::mismatches(&ya, &yb) as f64,
                "",
                Tolerance::Exact(0.0), // 019 FR-003
            ));
        }
    }
    rows
}

// ---- FR-001: the blend identity and the splice ---------------------------------------------------

/// The engine's device outputs of a playback with device-indexed requests at `rate`: the host
/// output at 6 kHz in `speed_only` (identical to the device stream), the trace hook elsewhere.
/// The requests are made by the plain setter before the call that computes device output n:
/// at 6 kHz calls of one sample; at the other rates calls of one host sample too, the request
/// set before the call whose first device pull is n (the trace tells which).
fn device_outputs(p: &mut Pipeline, rate: u32, requests: &[(usize, u8)]) -> Vec<f64> {
    if rate == 6000 {
        return play_6k(p, requests).iter().map(|&v| f64::from(v)).collect();
    }
    for &(c, level) in requests {
        if c == 0 {
            p.set_playback_speed(level).unwrap();
        }
    }
    p.start_playback(1).unwrap();
    let mut out = Vec::new();
    let mut buf = [0.0f32; 1];
    loop {
        for &(at, level) in requests {
            if at > 0 && at == out.len() {
                p.set_playback_speed(level).unwrap();
            }
        }
        let info = p.play(&mut buf).unwrap();
        out.extend(__test_hooks::speed_device_outputs(p));
        if info.finished {
            break;
        }
        assert!(out.len() < 50_000_000);
    }
    out
}

/// Spec 019 US1 AS2 (FR-001): a change a → b requested at device output s: the engine's device
/// outputs equal the reference's bit for bit (the blend from the published banks and weights),
/// the output before s equals the constant level-a run, from s + N − 1 the constant level-b
/// computation from the common place (the reference run with no change, started so that its
/// outputs pass through the common place).
pub fn blend_identity(make: Make<'_>, rate: u32, pair: (u8, u8)) -> Vec<MeasurementResult> {
    let (a, b) = pair;
    let settings = configs::settings("speed_only", rate);
    let x = stored_tone(2000.0, 4000);
    let xs = stored_stream(&x);
    let s = 1500usize;
    let mut p = recorded(make, settings, &x);
    let got = device_outputs(&mut p, rate, &[(0, a), (s, b)]);
    let (want, _) = sc::reference_output_at(&xs, 1, a, &[(s as u64, b)]);
    let n = n_fade();
    let mism = device_mismatches(&got, &want);
    // The splice: the constant level-a run before s; the level-b computation from the common
    // place from s + N − 1 (the reference with the request at s and the step build: identical
    // trajectories, so compare against a stepped reference run, which is the level-b
    // computation from s on).
    let (ya, _) = sc::reference_output_at(&xs, 1, a, &[]);
    let before = got.len() >= s
        && got[..s]
            .iter()
            .zip(&ya[..s])
            .all(|(g, w)| (*g as f32).to_bits() == w.to_bits());
    let ka = u64::from(numerator(a));
    let wait = if b == 3 {
        (5 - (s as u64 * ka) % 5) % 5
    } else {
        0
    };
    let s_eff = s + (wait as usize).div_ceil(ka as usize); // the switch output
    let stepped_ref = {
        let mut q = recorded(&stepped, settings, &x);
        device_outputs(&mut q, rate, &[(0, a), (s, b)])
    };
    let from = s_eff + n - 1;
    let after = got.len() == stepped_ref.len()
        && got.len() > from
        && got[from..]
            .iter()
            .zip(&stepped_ref[from..])
            .all(|(g, w)| (*g as f32).to_bits() == (*w as f32).to_bits());
    let stim = format!("stored 2 kHz -6 dBFS, {a} -> {b} at device output {s}");
    vec![
        MeasurementResult::new(
            "FR-001",
            "engine equals the reference bit for bit (mismatches)",
            T_XFADE,
            rate,
            "speed_only",
            stim.clone(),
            mism as f64,
            "",
            Tolerance::Exact(0.0), // 019 FR-001, FR-011
        ),
        MeasurementResult::new(
            "FR-001",
            "bit-identical to the old level's run before the change",
            T_XFADE,
            rate,
            "speed_only",
            stim.clone(),
            pass(before),
            "",
            Tolerance::Exact(1.0), // 019 FR-001
        ),
        MeasurementResult::new(
            "FR-001",
            "bit-identical to the new level's computation from s + N - 1",
            T_XFADE,
            rate,
            "speed_only",
            stim,
            pass(after),
            "",
            Tolerance::Exact(1.0), // 019 FR-001 (the splice)
        ),
    ]
}

// ---- FR-007: the click table ------------------------------------------------------------------

/// The click reader's exclusions for a stored tone at `f` played at levels a then b.
fn exclusions(f: f64, a: u8, b: u8) -> [f64; 2] {
    [
        f * f64::from(numerator(a)) / 5.0,
        f * f64::from(numerator(b)) / 5.0,
    ]
}

/// Spec 019 US1 AS3 (FR-007): the twenty rows of spec 013's step table at 6 kHz: the
/// crossfade's click energy against the stepped build's under the 40 ms reader (both output
/// tones excluded; the step centred on the switch, the crossfade on s + N ÷ 2): the thirteen
/// 2 and 2.4 kHz rows at least 30 dB; the seven 1 kHz rows, the 0.5 s reader's ratio, the
/// absolute energies and a held-level floor reported; each ratio within 0.5 dB of the design
/// script's print.
pub fn click_table(make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = configs::settings("speed_only", 6000);
    let mut rows = Vec::new();
    for &(f, a, b, _) in &SCRIPT_STEPS {
        let x = stored_tone(f, 4000);
        // The request at the output whose place first reaches stored sample 1500 (the script's
        // rule; the switch's click energy depends on the tone's phase at the switch).
        let s = 7500usize.div_ceil(numerator(a) as usize);
        let mut p = recorded(make, settings, &x);
        let fade = play_6k(&mut p, &[(0, a), (s, b)]);
        let mut q = recorded(&stepped, settings, &x);
        let step = play_6k(&mut q, &[(0, a), (s, b)]);
        let mut h = recorded(make, settings, &x);
        let hold = play_6k(&mut h, &[(0, a)]);
        // The switch output: the request at s lands at s, or at the next whole place for b = 3.
        let ka = numerator(a) as usize;
        let s_eff = if b == 3 {
            let mut t = s;
            while (t * ka) % 5 != 0 {
                t += 1;
            }
            t
        } else {
            s
        };
        let ex = exclusions(f, a, b);
        let half = n_fade() as f64 / 2.0;
        let e_step = click_energy_in(&step, 6000, s_eff as f64, 0.04, &ex);
        let e_fade = click_energy_in(&fade, 6000, s_eff as f64 + half, 0.04, &ex);
        let e_floor = click_energy_in(&hold, 6000, s_eff as f64 + half, 0.04, &ex);
        let r40 = db(e_step / e_fade) / 2.0;
        let r500 = db(click_energy_in(&step, 6000, s_eff as f64, 0.5, &ex)
            / click_energy_in(&fade, 6000, s_eff as f64 + half, 0.5, &ex))
            / 2.0;
        let stim = format!("stored {f:.0} Hz -6 dBFS, {a} -> {b}");
        rows.push(MeasurementResult::new(
            "FR-007",
            "crossfade click energy below the switch's, the 40 ms reader",
            T_XFADE,
            6000,
            "speed_only",
            stim.clone(),
            r40,
            "dB",
            if f >= 2000.0 {
                Tolerance::AtLeast(30.0) // 019 FR-007
            } else {
                Tolerance::Report // the 1 kHz rows: the switch's step is near the f32 floor
            },
        ));
        rows.push(MeasurementResult::new(
            "FR-007",
            "the 0.5 s reader's ratio (reported)",
            T_XFADE,
            6000,
            "speed_only",
            stim.clone(),
            r500,
            "dB",
            Tolerance::Report,
        ));
        rows.push(MeasurementResult::new(
            "FR-007",
            "absolute energies: switch / crossfade / held floor (reported)",
            T_XFADE,
            6000,
            "speed_only",
            format!(
                "{stim}: {:.1} / {:.1} / {:.1} dB",
                db(e_step) / 2.0,
                db(e_fade) / 2.0,
                db(e_floor) / 2.0
            ),
            db(e_fade / e_floor.max(1e-300)) / 2.0,
            "dB re the floor",
            Tolerance::Report,
        ));
    }
    rows
}

/// The design script's 40 ms ratios (`speed_design.py --crossfade`, printed 2026-10-30), in
/// `SCRIPT_STEPS`'s order; the harness's rows within 0.5 dB.
pub const SCRIPT_RATIOS_40MS: [f64; 20] = [
    50.9, 50.6, 46.5, 48.1, 48.1, 48.3, 48.5, 53.0, 52.8, 53.1, 52.0, 50.8, 50.7, 50.9, 49.1, 49.2,
    50.6, 50.5, 48.9, 49.2,
];

/// The click table's 40 ms ratios against the script's print, within 0.5 dB (research R-09).
pub fn click_table_against_the_script(make: Make<'_>) -> Vec<MeasurementResult> {
    let rows = click_table(make);
    rows.iter()
        .filter(|r| r.property.starts_with("crossfade click energy below"))
        .zip(SCRIPT_RATIOS_40MS)
        .map(|(r, want)| {
            MeasurementResult::new(
                "FR-007",
                "40 ms ratio - the script's",
                T_XFADE,
                6000,
                "speed_only",
                r.stimulus.clone(),
                r.measured - want,
                "dB",
                Tolerance::Range(-0.5, 0.5), // engineering target (019 R-09 pin)
            )
        })
        .collect()
}

/// Spec 019 FR-007: the voiced stimulus's absolute click energies (switch, crossfade), reported:
/// voice fills 20 – 2750 Hz, so no exclusion applies and no ratio is read.
pub fn voiced_click(make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = configs::settings("speed_only", 6000);
    let x = stimulus::voiced(6000.0, 1.0, -20.0, 0x019);
    let s = 2000usize;
    let mut rows = Vec::new();
    for (a, b) in [(3u8, 5u8), (5, 1)] {
        let mut p = recorded(make, settings, &x);
        let fade = play_6k(&mut p, &[(0, a), (s, b)]);
        let mut q = recorded(&stepped, settings, &x);
        let step = play_6k(&mut q, &[(0, a), (s, b)]);
        let half = n_fade() as f64 / 2.0;
        let e_step = click_energy_in(&step, 6000, s as f64, 0.04, &[]);
        let e_fade = click_energy_in(&fade, 6000, s as f64 + half, 0.04, &[]);
        rows.push(MeasurementResult::new(
            "FR-007",
            "voiced: switch and crossfade energies (reported; no exclusion applies)",
            T_XFADE,
            6000,
            "speed_only",
            format!(
                "voiced -20 dBFS, {a} -> {b}: {:.1} / {:.1} dB",
                db(e_step) / 2.0,
                db(e_fade) / 2.0
            ),
            db(e_step / e_fade) / 2.0,
            "dB",
            Tolerance::Report,
        ));
    }
    rows
}

// ---- FR-005: the wait --------------------------------------------------------------------------

/// Spec 019 US2 AS1 (FR-005): the wait cases at `rate` through the plain setter: consecutive
/// requests (the first starts a blend, the last of the rest waits and lands at its end), a
/// superseded request (never heard), a request equal to the running target (nothing), a request
/// at the latch (dropped): the engine equals the reference bit for bit; the reading follows
/// the latest request.
pub fn wait_cases(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let settings = configs::settings("speed_only", rate);
    let x = stored_tone(2000.0, 4000);
    let xs = stored_stream(&x);
    let cases: [(&str, Vec<(usize, u8)>); 4] = [
        (
            "consecutive: 1 -> 5 at 1500, 2 at 1501, 4 at 1502",
            vec![(0, 1), (1500, 5), (1501, 2), (1502, 4)],
        ),
        (
            "superseded: 3 -> 5 at 1500, 1 at 1540, 4 at 1580",
            vec![(0, 3), (1500, 5), (1540, 1), (1580, 4)],
        ),
        (
            "equal: 3 -> 5 at 1500, 5 at 1560",
            vec![(0, 3), (1500, 5), (1560, 5)],
        ),
        (
            "at the latch: 5, then 1 at the first output past K",
            vec![(0, 5), ((5 * xs.len()).div_ceil(7), 1)],
        ),
    ];
    let mut rows = Vec::new();
    for (name, requests) in cases {
        let mut p = recorded(make, settings, &x);
        let got = device_outputs(&mut p, rate, &requests);
        let start = requests[0].1;
        let dev: Vec<(u64, u8)> = requests[1..].iter().map(|&(c, l)| (c as u64, l)).collect();
        let (want, _) = sc::reference_output_at(&xs, 1, start, &dev);
        let mism = device_mismatches(&got, &want);
        let last = requests.last().unwrap().1;
        rows.push(MeasurementResult::new(
            "FR-005",
            "the wait: engine equals the reference bit for bit (mismatches)",
            T_XFADE,
            rate,
            "speed_only",
            name,
            mism as f64,
            "",
            Tolerance::Exact(0.0), // 019 FR-005
        ));
        rows.push(MeasurementResult::new(
            "FR-005",
            "the reading follows the latest request",
            T_XFADE,
            rate,
            "speed_only",
            name,
            pass(p.playback_speed() == last),
            "",
            Tolerance::Exact(1.0), // 019 FR-005
        ));
    }
    rows
}

// ---- FR-006: the end cut ------------------------------------------------------------------------

/// Spec 019 US1 AS6 (FR-006): a change at the new branch's last own output of a short
/// recording: the length rule holds (the total equals the stepped build's), the blend samples
/// cut by the drain, and the cut's peak (dB re the tone) and energy, in both configurations;
/// reported, the length asserted.
pub fn end_cut(make: Make<'_>) -> Vec<MeasurementResult> {
    let x = stored_tone(2000.0, 1200);
    let mut rows = Vec::new();
    for config in ["speed_only", "speed_only_codec"] {
        let settings = configs::settings(config, 6000);
        for (a, b) in PAIRS {
            let ka = numerator(a) as usize;
            // The latest landing that still starts a fade: the output whose place is the last
            // before K (for b = 3 a whole one).
            let own_a = (5 * x.len()).div_ceil(ka);
            let mut s = own_a - 1;
            if b == 3 {
                while (s * ka) % 5 != 0 {
                    s -= 1;
                }
            }
            let mut p = recorded(make, settings, &x);
            let fade = play_6k(&mut p, &[(0, a), (s, b)]);
            let mut q = recorded(&stepped, settings, &x);
            let step = play_6k(&mut q, &[(0, a), (s, b)]);
            let produced_after = fade.len().saturating_sub(s);
            let cut = n_fade().saturating_sub(produced_after.min(n_fade()));
            // The cut's content: the old branch's remaining weight times the tone (an upper
            // bound on its peak), reported.
            let w = sc::fade_weights();
            let idx = produced_after.min(n_fade()).max(1) - 1;
            let weight = if produced_after >= n_fade() {
                0.0
            } else {
                1.0 - w[idx]
            };
            let stim = format!("{a} -> {b} at the last own output {s} of K = {}", x.len());
            rows.push(MeasurementResult::new(
                "FR-006",
                "total length equals the stepped build's",
                T_XFADE,
                6000,
                config,
                stim.clone(),
                pass(fade.len() == step.len()),
                "",
                Tolerance::Exact(1.0), // 019 FR-006
            ));
            rows.push(MeasurementResult::new(
                "FR-006",
                "blend samples cut by the drain (reported)",
                T_XFADE,
                6000,
                config,
                stim.clone(),
                cut as f64,
                "samples",
                Tolerance::Report,
            ));
            rows.push(MeasurementResult::new(
                "FR-006",
                "the old branch's weight at the cut, dB re its full level (reported)",
                T_XFADE,
                6000,
                config,
                stim,
                db(weight.max(1e-300)),
                "dB",
                Tolerance::Report,
            ));
        }
    }
    rows
}

// ---- FR-008: the partitions -------------------------------------------------------------------

/// Spec 019 US1 AS4 (FR-008): a change scheduled on the playback stream at position P lands
/// identically at block sizes 1, 64, 4096 and the irregular partitions (outputs and logs), and
/// the plain-setter oracle of spec 015 holds.
pub fn partitions(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut settings = configs::settings("default_output", rate);
    settings.speech_codec_enabled = false;
    let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x019);
    let position = u64::from(rate) / 2;
    let changes = [
        Change::speed(position, 5, 1),
        Change::speed(position + u64::from(rate) / 10, 1, 2),
    ];
    let runs: Vec<cc::Run> = [
        Partition::Fixed(1),
        Partition::Fixed(64),
        Partition::Fixed(4096),
        Partition::Irregular,
        Partition::IrregularWithBoundary(position),
    ]
    .iter()
    .map(|&part| {
        let mut p = recorded(make, settings, &x);
        cc::run_scheduled(&mut p, Source::Playback(1), &changes, part)
    })
    .collect();
    let agree = runs[1..]
        .iter()
        .all(|r| cc::same(&r.out, &runs[0].out) && r.log == runs[0].log);
    let mut p = recorded(make, settings, &x);
    let oracle = cc::setter_oracle(&mut p, Source::Playback(1), &changes);
    vec![
        MeasurementResult::new(
            "FR-008",
            "five partitions give one output and one log",
            T_XFADE,
            rate,
            "default_output",
            format!("voiced -20 dBFS, 3 -> 5 at {position}, 1 at +rate/10"),
            pass(agree),
            "",
            Tolerance::Exact(1.0), // 019 FR-008 (015 FR-008)
        ),
        MeasurementResult::new(
            "FR-008",
            "equals spec 015's block-size-1 setter oracle",
            T_XFADE,
            rate,
            "default_output",
            format!("voiced -20 dBFS, 3 -> 5 at {position}, 1 at +rate/10"),
            pass(cc::same(&runs[0].out, &oracle)),
            "",
            Tolerance::Exact(1.0), // 019 FR-008
        ),
    ]
}

/// Spec 019 FR-006: the length rule at every constant level (spec 013's `lengths`) and the
/// total of a scheduled playback equal to the stepped build's when every request stands alone.
pub fn lengths_unchanged(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut rows = sc::lengths(make, rate, "speed_only");
    let settings = configs::settings("speed_only", rate);
    let x = stored_tone(2000.0, 3000);
    for (a, b) in [(3u8, 5u8), (5, 1)] {
        let mut p = recorded(make, settings, &x);
        let fade = sc::play(&mut p, 1, 120, &[(0, a), (10, b)]).out;
        let mut q = recorded(&stepped, settings, &x);
        let step = sc::play(&mut q, 1, 120, &[(0, a), (10, b)]).out;
        rows.push(MeasurementResult::new(
            "FR-006",
            "a lone change: the total equals the stepped build's",
            T_XFADE,
            rate,
            "speed_only",
            format!("{a} -> {b} at call 10"),
            pass(fade.len() == step.len()),
            "",
            Tolerance::Exact(1.0), // 019 FR-006
        ));
    }
    rows
}

/// The weights hook equals the reference's table bit for bit (FR-011).
pub fn weights_agree() -> Vec<MeasurementResult> {
    let engine = __test_hooks::crossfade_weights(3);
    let reference = sc::fade_weights();
    vec![MeasurementResult::new(
        "FR-011",
        "the engine's weight table equals the reference's bit for bit",
        T_XFADE,
        6000,
        "any",
        "120 entries",
        pass(
            engine.len() == reference.len()
                && engine
                    .iter()
                    .zip(&reference)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
        ),
        "",
        Tolerance::Exact(1.0), // 019 FR-011
    )]
}

/// The unused imports guard (the module's items are used by the tests).
#[allow(dead_code)]
fn _uses(_: Route, _: Stream, _: &dyn Fn(f64) -> f64) {
    let _ = analysis::db;
}
