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

/// US1 AS1 (FR-006): every pre-021 configuration is bit-identical to 0.20: the fourteen earlier
/// golden files (and every later one but 021's own; the pitch-kept guard since spec 023) are
/// byte-identical to the SHA constants their guards pin (re-read from the test sources), and the stepped build equals the standard build when nothing lands (the click
/// reader's step reference is the engine itself; FR-007).
#[test]
fn as1_pre_021_configurations_unmoved() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let re =
        regex::Regex::new(r#"const (GOLDEN_[A-Z0-9_]*SHA256): &str =\s*"([0-9a-f]{64})""#).unwrap();
    let mut checked = 0;
    for entry in std::fs::read_dir(root.join("tests")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with("golden") || name == "golden_bypass.rs" || !name.ends_with(".rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for cap in re.captures_iter(&text) {
            let file = match &cap[1] {
                "GOLDEN_V1_SHA256" => "golden-v1.json".to_string(),
                c => {
                    let stem = c
                        .trim_start_matches("GOLDEN_")
                        .trim_end_matches("_SHA256")
                        .to_lowercase()
                        .replace('_', "-");
                    format!("golden-{stem}.json")
                }
            };
            let bytes =
                std::fs::read(root.join("golden").join(&file)).unwrap_or_else(|_| panic!("{file}"));
            let hex: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(hex, &cap[2], "{file} moved");
            checked += 1;
        }
    }
    // Fourteen at 0.21; spec 023 added the pitch-kept guard (2026-10-31): the test reads every
    // guard but 021's own, so it keeps checking later goldens too.
    assert_eq!(
        checked, 15,
        "the fourteen earlier golden guards and the pitch-kept one"
    );
    let mut rows = Vec::new();
    for &rate in configs::measurement_rates() {
        for cell in bp::cells().iter().filter(|c| c.exact).take(3) {
            let s = bp::settings(cell.config, rate);
            let stored = bp::stored_tone(1000.0, 0.5);
            let host = bp::host_tone(1000.0, 0.5, rate);
            let part = rr_dr60_harness::changes_checks::Partition::Fixed(256);
            let a = bp::run_cell(&standard, s, cell, &stored, &host, 0.5, &[], part);
            let b = bp::run_cell(&bp::stepped, s, cell, &stored, &host, 0.5, &[], part);
            rows.push(MeasurementResult::new(
                "FR-007",
                "no landing: the stepped build is the standard build",
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
                rr_dr60_harness::checks::Tolerance::Exact(1.0), // 021 FR-007 (the step reference)
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
