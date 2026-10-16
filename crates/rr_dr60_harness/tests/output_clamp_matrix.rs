//! Spec 010: the seven-rate report of the output clamp (tasks.md T010; research R-04;
//! data-model.md › Expected report). One table at `configs::all_rates()` with 1 s stimuli so
//! the counts reproduce the plan-time probe: the identity on every cell, the option-off peaks
//! and the share of samples bounded (reported), the four overshooting stimuli asserted to exceed
//! the bound, the reference configuration asserted not to on the tones, the quiet-material
//! rows at the reference position asserted to have zero samples beyond the bound, and the
//! speaker template's L1 norm recomputed and pinned with every speaker-route peak at or below
//! it. Report: `cargo test -p rr_dr60_harness --test output_clamp_matrix -- --nocapture`.

use rr_dr60::Route;
use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all, standard};
use rr_dr60_harness::output_clamp_checks::{self as clamp, Expect};
use rr_dr60_harness::{configs, report};

const LEN_S: f64 = 1.0;

#[test]
fn output_clamp_matrix() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        // The primary stimulus on both routes.
        for route in [Route::Speaker, Route::Earphone] {
            rows.extend(clamp::identity_at(
                &standard,
                rate,
                "output_clamp_only",
                route,
                1.0,
                "sine_1k_1p5",
                LEN_S,
                Expect::Exceeds,
            ));
        }
        // The four overshooting chain cells.
        for (config, stim) in [
            ("output_only_speaker_clamp", "square_1k_fs"),
            ("output_only_earphone_clamp", "square_1k_fs"),
            ("output_only_speaker_clamp", "tone_1k_0"),
            ("default_output_clamp", "square_1k_fs"),
            ("default_output_clamp", "tone_1k_0"),
            ("default_earphone_clamp", "square_1k_fs"),
        ] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                config,
                stim,
                LEN_S,
                Expect::Exceeds,
            ));
        }
        // The reference configuration: within on the tones, reported on the square (T006).
        for (stim, expect) in [
            ("tone_1k_0", Expect::Within),
            ("sine_1k_1p5", Expect::Within),
            ("square_1k_fs", Expect::Report),
        ] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                "default_output_ref_clamp",
                stim,
                LEN_S,
                expect,
            ));
        }
        // The remaining stimulus × configuration cells (18 in all with the above), reported.
        for (config, stim) in [
            ("output_clamp_only", "square_1k_fs"),
            ("output_clamp_only", "tone_1k_0"),
            ("output_only_speaker_clamp", "sine_1k_1p5"),
            ("output_only_earphone_clamp", "sine_1k_1p5"),
            ("output_only_earphone_clamp", "tone_1k_0"),
            ("default_output_clamp", "sine_1k_1p5"),
            ("default_earphone_clamp", "sine_1k_1p5"),
            ("default_earphone_clamp", "tone_1k_0"),
        ] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                config,
                stim,
                LEN_S,
                Expect::Report,
            ));
        }
        // Quiet material at the reference position: untouched (hardware review D4).
        for stim in ["voiced_m1", "voiced_limited", "noise_m3"] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                "default_output_ref_clamp",
                stim,
                LEN_S,
                Expect::Within,
            ));
        }
        // T038 B2: loud material near the template's resonance at the reference position:
        // the bursts peak within about 0.7 dB of the bound with the codec on, the 800 Hz square
        // exceeds it slightly at the high rates (reported, never asserted; level-sensitive
        // through the codec's discrete choices).
        for stim in ["bursts_800_p10", "square_800_fs"] {
            rows.extend(clamp::identity(
                &standard,
                rate,
                "default_output_ref_clamp",
                stim,
                LEN_S,
                Expect::Report,
            ));
        }
        // The mechanism with the codec bypassed: the bursts exceed the bound robustly.
        {
            let mut s = configs::settings("default_output", rate);
            s.speech_codec_enabled = false;
            let x = clamp::stimulus_by_name("bursts_800_p10", rate, LEN_S);
            let y = rr_dr60_harness::output_checks::run(
                &standard,
                s,
                Route::Speaker,
                rr_dr60::output::VOLUME_REFERENCE,
                &x,
            );
            rows.push(MeasurementResult::new(
                "FR-011",
                "reference position, codec bypassed: 800 Hz bursts +10 dBFS option-off peak (reported)",
                &["A-042", "engineering target"],
                rate,
                "default_output (codec off)",
                "bursts_800_p10",
                f64::from(y.iter().fold(0.0f32, |m, v| m.max(v.abs()))),
                "full scale",
                Tolerance::Report,
            ));
        }
        // T038 N2: the figures the README and A-042 quote, as ranges (the docs' own rounding).
        let documented = |config: &str, stim: &str, lo: f64, hi: f64, what: &'static str| {
            MeasurementResult::new(
                "FR-009",
                what,
                &["A-042", "engineering target"],
                rate,
                "documented figure",
                format!("{config}, {stim}"),
                f64::from(clamp::off_peak(&standard, rate, config, stim, LEN_S)),
                "full scale",
                Tolerance::Range(lo, hi),
            )
        };
        if rate == 48_000 {
            rows.push(documented(
                "output_only_speaker_clamp",
                "square_1k_fs",
                1.85,
                1.95,
                "chain-only speaker square at 1, about 1.9",
            ));
            rows.push(documented(
                "output_only_speaker_clamp",
                "tone_1k_0",
                1.85,
                1.95,
                "chain-only speaker tone at 1, about 1.9",
            ));
        }
        if rate >= 44_100 {
            rows.push(documented(
                "default_output_clamp",
                "tone_1k_0",
                1.95,
                2.10,
                "default speaker tone at 1, about 1.96 - 2.06",
            ));
        }
        rows.push(documented(
            "output_only_earphone_clamp",
            "square_1k_fs",
            1.36,
            1.53,
            "chain-only earphone square at 1, 1.37 - 1.52",
        ));
        rows.push(documented(
            "default_earphone_clamp",
            "square_1k_fs",
            1.03,
            1.08,
            "default earphone square at 1, 1.04 - 1.07",
        ));
        // The speaker route's bound: the template's L1 norm against every speaker-route peak.
        let peaks: Vec<(String, f32)> = [
            ("output_only_speaker_clamp", "square_1k_fs"),
            ("output_only_speaker_clamp", "tone_1k_0"),
            ("output_only_speaker_clamp", "sine_1k_1p5"),
            ("default_output_clamp", "square_1k_fs"),
            ("default_output_clamp", "tone_1k_0"),
            ("default_output_clamp", "sine_1k_1p5"),
            ("default_output_ref_clamp", "square_1k_fs"),
        ]
        .iter()
        .map(|(c, s)| {
            (
                format!("{c}, {s}"),
                clamp::off_peak(&standard, rate, c, s, LEN_S),
            )
        })
        .collect();
        let (l1, l1_rows) = clamp::l1_bound(rate, &peaks);
        rows.extend(l1_rows);
        println!("{rate} Hz: speaker-route bound (template L1 norm) {l1:.3}");
    }
    report::print(&rows);
    println!("output_clamp_matrix: {} checks", rows.len());
    assert_all(&rows);
}
