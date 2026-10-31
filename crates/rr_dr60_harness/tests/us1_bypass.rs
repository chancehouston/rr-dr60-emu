//! Spec 021 User Story 1: hear a stage come and go without a click (tasks.md T011; spec › US1
//! AS1 – AS7). One test per acceptance scenario at the measurement rates, each asserting every
//! row passes. Report: `cargo test -p rr_dr60_harness --test us1_bypass -- --nocapture`.

use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn each(check: impl Fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    configs::measurement_rates()
        .iter()
        .flat_map(|&r| check(r))
        .collect()
}

/// US1 AS1 (FR-006): every pre-021 configuration is bit-identical to 0.20: the fourteen golden
/// guards hold (`golden*` binaries) and no cell's all-on run differs from a chain built with no
/// bypass state (the byte gate is 0): checked through the endpoint rows' "before the landing".
#[test]
fn as1_pre_021_configurations_unmoved() {
    // The guards themselves run in the golden binaries; here the no-schedule runs of two cells
    // at every measurement rate are identical between the standard build and the stepped build
    // (no fade state is ever entered without a landing).
    let mut rows = Vec::new();
    for &rate in configs::measurement_rates() {
        for cell in bp::cells().iter().filter(|c| c.exact).take(3) {
            let s = bp::settings(cell.config, rate);
            let stored = bp::stored_tone(1000.0, 0.5);
            let host = bp::host_tone(1000.0, 0.5, rate);
            let a = bp::run_cell(
                &standard,
                s,
                cell,
                &stored,
                &host,
                0.5,
                &[],
                rr_dr60_harness::changes_checks::Partition::Fixed(256),
            );
            let b = bp::run_cell(
                &bp::stepped,
                s,
                cell,
                &stored,
                &host,
                0.5,
                &[],
                rr_dr60_harness::changes_checks::Partition::Fixed(256),
            );
            rows.push(MeasurementResult::new(
                "FR-006",
                "no landing: identical to the stepped build",
                bp::T_BYPASS,
                rate,
                cell.config,
                cell.label(),
                if rr_dr60_harness::changes_checks::same(&a.out, &b.out) {
                    1.0
                } else {
                    0.0
                },
                "",
                rr_dr60_harness::checks::Tolerance::Exact(1.0), // 021 FR-006
            ));
        }
    }
    finish(rows);
}

/// US1 AS2 (FR-003): the endpoint identities on every exact cell, bit for bit.
#[test]
fn as2_bypass_identity_exact_where_last() {
    finish(each(|r| {
        bp::cells()
            .iter()
            .filter(|c| c.exact)
            .flat_map(|c| bp::endpoint_identity(&standard, c, r))
            .collect()
    }));
}

/// US1 AS2 (FR-003): the converging cells of the default engine: within −100 dBFS after the
/// settle; the LSB figure and the first identical sample printed.
#[test]
fn as2_bypass_identity_converges_in_the_default_engine() {
    finish(each(|r| {
        bp::cells()
            .iter()
            .filter(|c| !c.exact)
            .flat_map(|c| bp::endpoint_identity(&standard, c, r))
            .collect()
    }));
}

/// US1 AS2 (FR-002): the blend identity on the exact chain cells; the midpoint 0.5.
#[test]
fn as2_blend_identity() {
    finish(each(|r| {
        bp::cells()
            .iter()
            .filter(|c| c.exact && c.stage != 10)
            .flat_map(|c| bp::blend_identity(&standard, c, r))
            .collect()
    }));
}

/// US1 AS3 (FR-002, FR-003): stage 10 on the device-rate trace; the host onset measured.
#[test]
fn as3_stage10_device_trace_and_host_onset() {
    finish(each(|r| bp::stage10_trace(&standard, r)));
}

/// US1 AS4 (FR-003): the live engine's tail where the twin is shorter: exactly zero.
#[test]
fn as4_live_tail_is_the_drained_input() {
    finish(each(|r| {
        bp::cells()
            .iter()
            .filter(|c| {
                c.exact
                    && c.stream == rr_dr60::Stream::Playback
                    && matches!(c.config, "playback_only" | "output_only_amp")
            })
            .flat_map(|c| bp::live_tail(&standard, c, r))
            .collect()
    }));
}

/// US1 AS5 (FR-007): the click table at 6 kHz (asserted rows ≥ 30 dB; reported rows printed),
/// the voiced figures, the level table, the 48 kHz rows.
#[test]
fn as5_click_table() {
    let mut rows = bp::click_table(&standard, 6000);
    rows.extend(bp::voiced_click(&standard));
    rows.extend(bp::level_table());
    if configs::measurement_rates().contains(&48_000) {
        rows.extend(bp::click_table(&standard, 48_000));
        rows.extend(bp::stage10_third_octave_48k(&standard));
    }
    finish(rows);
}

/// US1 AS6 (FR-008): the partitions on one exact cell per stage and one converging cell.
#[test]
fn as6_partitions() {
    finish(each(|r| {
        let cells = bp::cells();
        let picks: Vec<&bp::Cell> = [
            cells.iter().find(|c| c.stage == 10 && c.exact),
            cells
                .iter()
                .find(|c| c.stage == 12 && c.exact && c.stream == rr_dr60::Stream::OnePass),
            cells.iter().find(|c| c.stage == 13 && c.exact),
            cells.iter().find(|c| c.stage == 11 && !c.exact),
        ]
        .into_iter()
        .flatten()
        .collect();
        picks
            .iter()
            .flat_map(|c| bp::partitions(&standard, c, r))
            .collect()
    }));
}

/// US1 AS7 (FR-005): the reversal at u = 1/2.
#[test]
fn as7_reversal() {
    finish(each(|r| bp::reversal(&standard, r)));
}
