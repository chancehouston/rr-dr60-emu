//! Writes the speech codec's calibration stimuli as raw 16-bit little-endian PCM at a given
//! rate (spec 008 research R-09; tools/celp-calibrate/README.md): `voiced` and
//! `unvoiced_voiced` at −10, −20 and −40 dBFS, 10 s each, seed 0x0D60. Prints each file's
//! SHA-256, which `specs/008-celp-codec/calibration.md` records and the harness's
//! `stimulus_digests` test asserts, so a stimulus change invalidates the calibration visibly.
//!
//! The harness may do file I/O; the core may not (constitution Principle I).
//!
//! Usage: `cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out <dir>`

use std::fs;
use std::path::PathBuf;

use rr_dr60_harness::stimulus::{unvoiced_voiced, voiced};
use sha2::{Digest, Sha256};

/// The six stimuli: (file stem, voiced?, level dBFS).
pub const STIMULI: [(&str, bool, f64); 6] = [
    ("voiced_m10", true, -10.0),
    ("voiced_m20", true, -20.0),
    ("voiced_m40", true, -40.0),
    ("unvoiced_voiced_m10", false, -10.0),
    ("unvoiced_voiced_m20", false, -20.0),
    ("unvoiced_voiced_m40", false, -40.0),
];

/// The seed every calibration stimulus uses.
pub const SEED: u64 = 0x0D60;
/// The duration of every calibration stimulus, in seconds.
pub const SECONDS: f64 = 10.0;

/// The raw little-endian 16-bit bytes of one stimulus at `rate`.
pub fn stimulus_bytes(stem: &str, voiced_kind: bool, level_dbfs: f64, rate: u32) -> Vec<u8> {
    let fs = f64::from(rate);
    let x = if voiced_kind {
        voiced(fs, SECONDS, level_dbfs, SEED)
    } else {
        unvoiced_voiced(fs, SECONDS, level_dbfs, SEED)
    };
    debug_assert!(!stem.is_empty());
    x.iter()
        .flat_map(|&v| {
            let s = (f64::from(v) * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
            s.to_le_bytes()
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut rate = 8000u32;
    let mut out = PathBuf::from("tools/celp-calibrate/out");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rate" => {
                rate = args[i + 1].parse().expect("--rate <Hz>");
                i += 2;
            }
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            other => panic!("unknown argument {other}; usage: --rate <Hz> --out <dir>"),
        }
    }
    fs::create_dir_all(&out).expect("create the output directory");
    for (stem, kind, level) in STIMULI {
        let bytes = stimulus_bytes(stem, kind, level, rate);
        let path = out.join(format!("{stem}.raw"));
        fs::write(&path, &bytes).expect("write the stimulus");
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        println!("{} {} bytes sha256 {digest}", path.display(), bytes.len());
    }
}
