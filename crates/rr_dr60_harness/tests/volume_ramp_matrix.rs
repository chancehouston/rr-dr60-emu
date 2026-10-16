//! Spec 011 seven-rate report (tasks.md T014; research R-02): FR-004 per rate × route ×
//! direction beside `tools/volume-ramp/ramp_model.py`'s figure (pinned ±0.25 dB, floor 30 dB),
//! the linear-shape mutant's figure (reported: the reason for the smoothstep), the drag, N per
//! rate and the route output (reported). One table at `configs::all_rates()`. Report:
//! `cargo test -p rr_dr60_harness --test volume_ramp_matrix -- --nocapture`.

use rr_dr60::__test_hooks::OutputMutation;
use rr_dr60::output::volume_transition_samples;
use rr_dr60::{Pipeline, Route};
use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all, standard};
use rr_dr60_harness::volume_ramp_checks as ramp;
use rr_dr60_harness::{configs, report};

#[test]
fn volume_ramp_matrix() {
    let hook = |m: OutputMutation| move |s| Pipeline::__with_output_mutation(s, &m).unwrap();
    let linear = hook(OutputMutation {
        volume_shape_linear: true,
        ..OutputMutation::default()
    });
    let step = hook(OutputMutation {
        volume_step: true,
        ..OutputMutation::default()
    });
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.push(MeasurementResult::new(
            "FR-002",
            "transition length N (20 ms)",
            ramp::T_RAMP,
            rate,
            "any",
            "volume_transition_samples",
            f64::from(volume_transition_samples(rate).unwrap()),
            "samples",
            Tolerance::Exact(f64::from(rate) * 0.02),
        ));
        for config in configs::STAGE11_CONFIGS {
            for (a, b) in [(0.1f32, 1.0f32), (1.0, 0.1)] {
                rows.extend(ramp::click_reduction(&standard, rate, config, a, b));
            }
            rows.push(MeasurementResult::new(
                "FR-004",
                "linear position movement (mutant, reported)",
                ramp::T_RAMP,
                rate,
                config,
                "1 kHz, 0.1 -> 1",
                ramp::click_reduction_db(&linear, &standard, rate, config, 0.1, 1.0),
                "dB",
                Tolerance::Report,
            ));
            rows.extend(ramp::drag(&standard, rate, config));
        }
        for route in [Route::Speaker, Route::Earphone] {
            rows.extend(ramp::route_output(&standard, &step, rate, route));
        }
    }
    report::print(&rows);
    assert_all(&rows);
}
