# Quickstart: validating the non-speech reshaping report

- The reader and the stimuli: `cargo test -p rr_dr60_harness --lib nonspeech`.
- The stories: `cargo test -p rr_dr60_harness --test us1_nonspeech --test us3_nonspeech -- --nocapture` (D6 i – vi at the measurement rates, each row with its figure, bound and margin).
- The matrix: `cargo test -p rr_dr60_harness --test nonspeech_matrix -- --nocapture` (seven rates; the partitions asserted; the limiter twin; the cross-rate rows reported, then bounded).
- The golden: `cargo test -p rr_dr60_harness --test golden_nonspeech` (byte compare and the SHA pin; `doctored_hang`); re-bless after an *intended* change: `RR_DR60_BLESS=nonspeech cargo test -p rr_dr60_harness --test golden_nonspeech` (asserts D5 and D6 at bless time; a CHANGELOG entry).
- Mutants, determinism, allocation, wording, the guide, the pins: `cargo test -p rr_dr60_harness --test mutation --test determinism --test alloc_free --test us4_wording --test us2_cross_check --test us2_abi --test us1_bypass nonspeech` (the `cargo test --test a --test b NAME` filter matches every binary by NAME: each binary's nonspeech tests carry the word).
- The canonical text: `cargo run -q -p rr_dr60_harness --example nonspeech_report`; the guide region: `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`.
- The desk check (optional, needs the ITU software under `tools/celp-calibrate/ref/`): `cargo run -p rr_dr60_harness --example nonspeech_report -- --write-noise tools/celp-calibrate/out` then the calibration script; the figure is reported in the records.
- Expected: the golden byte-identical at 6, 44.1 and 48 kHz; every D6 row green with its margin; the partitions identical; the seventeen earlier goldens and every smoke hash unmoved; the core crates untouched.
