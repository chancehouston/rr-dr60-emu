# Quickstart: validating the volume transition (feature 011)

Prerequisites: the toolchain of CLAUDE.md (`export PATH="$HOME/.cargo/bin:$PATH"`), `uv`, branch `011-volume-ramp`. Every command runs from the repository root. The design figures are in [research.md](research.md) R-02; the state and the per-sample step in [data-model.md](data-model.md); the contracts in [contracts/](contracts/).

## 0. The reference figures

```sh
uv run tools/volume-ramp/ramp_model.py           # N per rate, FR-004 per rate and route, the drag, the f64 range check, N_OUT / N
uv run tools/volume-ramp/ramp_model.py --check   # asserts the 30 dB floor, the drag, no reversals, N_OUT / N > 2.87
uv run tools/volume-ramp/ramp_model.py --t1090 12 --from 1 --to 0   # A-045's knob-flick comparison for a measured 10–90 % time
```

## 1. The earlier specs are untouched

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output --test golden_output_clamp
```

Expected: the six files byte-identical and their SHA guards unmoved (FR-006, SC-004). Run this after every phase.

## 2. Story 1: click-free changes

```sh
cargo test -p rr_dr60 output::                                    # the gain sequence: monotonic, in range, exact at N, the formula; N per rate; the start-at-once rules
cargo test -p rr_dr60_harness --test us1_volume_ramp -- --nocapture  # the splice identity, the ratio reader, partitions, FR-004, retarget, zeros, drag, route output (reported)
cargo test -p rr_dr60_harness --test volume_ramp_matrix -- --nocapture
cargo test -p rr_dr60_harness --test volume_ramp_edge_cases
```

Expected: at the seven rates on both routes the splice identity holds before the change and from sample N; the ratio reader is monotonic within 4 ULP; the output is bit-identical across partitions of 1, 64, 4096 and an irregular one; the first differing sample is 1 and the last N − 1; the click reduction is ≥ 30 dB and within ±0.25 dB of research R-02's table (speaker 33.9 – 34.5 dB, earphone 40.6 – 41.2 dB); a full-scale tone on the speaker route never exceeds the ceiling; a retarget never jumps and a retarget within rounding of the knob stays monotonic; a transition to 0 reads exact zeros from N and decays below −120 dBFS within 100 ms of N; the drag is monotonic and ≥ 30 dB below the stepped drag (script 34.9 – 38.3 dB; harness 36.3 – 38.4 speaker, 34.8 – 36.8 earphone); the route-output figures are printed.

## 3. Story 2: the whole engine

```sh
cargo test -p rr_dr60_harness --test us1_volume_ramp -- whole_engine --nocapture  # start-at-once, route change, reset, reconfigure, bypass, clamp, drain
cargo test -p rr_dr60_harness --test volume_ramp_edge_cases refused_position_and_read_back   # a refused position starts nothing; the read-back is the host's setting
cargo test -p rr_dr60_harness --test us1_volume_ramp -- equivalence
cargo test -p rr_dr60_harness --test us1_output --test output_matrix --test output_edge_cases --test us1_output_clamp   # the restated 009/010 checks
cargo test -p rr_dr60_harness --test determinism volume_ramp
cargo test -p rr_dr60_harness --test alloc_free
cargo test -p rr_dr60_harness --test mutation volume_ramp
cargo test -p rr_dr60_harness --release --test timing volume_ramp_cost -- --ignored --nocapture --test-threads=1   # solo; quote from this run
scripts/ab-timing.sh bd6d827                                      # the idle A/B against the 0.10 build (output_cost, nine alternating runs each); once on the CI runner: the PR's `ab-timing` label
```

Expected: the first block after construction, reset, reconfigure and `start_playback` equal to the `volume_step` hook (0.10's arithmetic in this build) and to a new engine; a route change completes the transition; record-then-play equals the one-pass with a constant volume and from the change with a change at the same output sample; the four mutant configurations fail (step: FR-004 ≈ 0 dB and the last-differing-sample row; linear: ≈ 19 dB on the speaker law; +1: the splice from N; −1: the last-differing-sample row); the worst case (480-sample blocks, alternating 0.1 / 1, noise at −20 dB; the constant-position spread ≤ 2 %) ≤ 15 % over the step hook, the idle A/B ≤ 3 %, their product reported against 0.10, all with nanoseconds.

## 4. Story 3: the records

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

Expected: stage 11 cites A-045 with model version 0.11.0 and the new coverage note; the counts unchanged (10 not modeled… as the manifest prints); the canonical text and README table re-blessed once (Phase 1); the README volume paragraph, A-044's and A-045's rows, the signal-chain row 11, the CHANGELOG, the issue template and spec 009's notes carry the stated text.

## 5. C

```sh
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cargo test -p rr_dr60_harness --test ffi_parity volume
```

Expected: the existing hashes unchanged, the new mid-playback case equal to `RR_DR60_SMOKE_SPEAKER_RAMP_HASH`, the define 20; C equals Rust at the seven rates with a mid-playback change; the struct still 112 bytes.

## 6. The golden file and the gates

```sh
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings   # first: the gate without the new golden
cargo test --all-features -- --skip golden_volume_ramp
RR_DR60_BLESS=volume_ramp cargo test -p rr_dr60_harness --test golden_volume_ramp   # once, after the reviews
cargo test --all-features && cargo llvm-cov --all-features --workspace --fail-under-lines 80   # then the full gate with coverage
```
