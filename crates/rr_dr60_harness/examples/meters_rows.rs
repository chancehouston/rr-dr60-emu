//! Prints the meter read-out rows of one one-pass run, as the golden stores them and as the C
//! smoke program prints them (`smoke --meters`; spec 016 FR-012, research R-10).
//!
//! Run: `cargo run -q -p rr_dr60_harness --example meters_rows -- --stimulus clamp_square
//! --config meters_default_clamp --rate 48000`

use rr_dr60::Pipeline;
use rr_dr60_harness::meters_checks::{ONE_PASS_BLOCK, one_pass_blocks, row};
use rr_dr60_harness::{configs, golden};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: &str| -> String {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_else(|| default.to_string())
    };
    let stimulus = arg("--stimulus", "clamp_square");
    let config = arg("--config", "meters_default_clamp");
    let rate: u32 = arg("--rate", "48000").parse().expect("a rate");
    let x = golden::meters_stimulus(&stimulus, rate);
    let mut p = Pipeline::new(configs::settings(&config, rate)).expect("a valid configuration");
    for b in one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK) {
        println!("{}", row(&b.meters));
    }
}
