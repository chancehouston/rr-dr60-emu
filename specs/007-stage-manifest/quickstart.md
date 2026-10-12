# Quickstart: validating feature 007 (the stage manifest)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 006 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test; `cbindgen` 0.29.4 for the header).

## 1. Story 1: the manifest from Rust (FR-001 – FR-012, FR-016)

```sh
cargo test -p rr_dr60 manifest
cargo test -p rr_dr60 --doc
cargo test -p rr_dr60_harness --test us1_manifest -- --nocapture
cargo run -q -p rr_dr60_harness --example manifest | head -20
```

**T039 walk (2026-10-12):** observed 6 core unit tests, 4 doctests, 10 `us1_manifest` tests green; the example prints `rr_dr60 manifest`, `library_version: 0.7.0`, `device_rate_hz: 6000`, `stage_count: 15`, `counts: not_modeled=10 assumed=5 sourced=0 checked=0`. Expected (no register, no engine): the core's unit tests pass (fifteen entries, the compile-time invariants, the C twin table equals the strings plus NUL, the bypass-control anchor), the doctest runs; `us1_manifest` sees fifteen entries in order with the literal statuses (3, 4, 5, 8, 10 assumed, the rest not modeled), `counts: not_modeled=10 assumed=5 sourced=0 checked=0`, `library_version: 0.7.0`, `device_rate_hz: 6000`, equality across two reads and zero wording findings; the example prints the canonical text starting with `rr_dr60 manifest`.

## 2. Story 2: the drift guard (FR-013, FR-014, FR-020)

```sh
RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical   # first time only: creates golden/manifest-v1.txt
cargo test -p rr_dr60_harness --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

**T039 walk (2026-10-12):** observed 21 tests green; the doctored inputs printed `Superseded stage 3 A-028`, `Uncited - A-040`, `TableShape stage 3 -: table identifier "agc" != manifest "limiter"`, `TableShape - -: … 16 data rows` and `… 14 data rows`, `StatusDerivation stage 7 -: the table row is marked Modeled by but the manifest says not modeled`, `ConfidenceBound stage 3 -: High exceeds the headline bound (rank 1)`, `UnknownId stage 11 S-006`; `check-traceability: OK`. Expected: zero findings on the committed registers; each of the seven doctored inputs produces the stated finding (contracts/cross-check.md › Tests, item 2), printed as `rule stage id`; the parser units pass; the canonical text equals the golden file; the traceability script prints `check-traceability: OK` and, if a manifest ID is removed from a register by hand, `manifest cites unknown ID`. To see the guard bite for real: change `| A-028 |`'s Status cell in `docs/hardware/assumptions.md` to `superseded`, run the second command, see `Superseded stage 3 A-028`, revert. The `registers` workflow (`.github/workflows/registers.yml`) runs the same commands on a PR that touches only `docs/hardware/`, `CHANGELOG.md` or `README.md`; check the PR's checks list for `registers`. To make it a required check (optional; see research R-06 for the trade-off): repository Settings → Branches → `main` → required status checks → add `registers`.

## 3. Story 3: the same from C (FR-017)

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

**T039 walk (2026-10-12):** observed 16 FFI unit tests, 19 `ffi_parity` tests and the allocation test green; `smoke: OK (rr_dr60 0.7.0, latency 483 samples at 48000 Hz)` and `smoke --manifest matches the Rust example`. Expected: the FFI layout tests pass (`RrDr60ManifestHeader` 64 bytes and `RrDr60StageEntry` 80 bytes on a 64-bit host; status 20; `RR_DR60_VERSION_MINOR` 7); parity for all fifteen stages and the header, `NO_SUCH_STAGE` for 0 and 16, `INVALID_ARGUMENT` for an index at the count; zero allocations across the five C functions and the Rust reads; `smoke: OK (rr_dr60 0.7.0, latency 483 samples at 48000 Hz)` followed by `smoke --manifest matches the Rust example` (the diff is empty).

## 4. Nothing else moved (FR-018, SC-004, SC-005)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test determinism
git diff --stat main -- crates/rr_dr60_harness/golden/*.json
cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture
```

**T039 walk (2026-10-12):** observed the three golden tests (1 / 3 / 2 tests) and `determinism` (8) green, the JSON `git diff` against `main` empty, the guards 7730…948b / c8ba…ea78 / 009c…a17e unchanged; the timing rows are recorded under step 6. Expected: the three golden tests pass with their SHA guards unchanged and the `git diff` is empty (no JSON changed); `determinism` also compares the canonical text with `golden/manifest-v1.txt`; the timing rows stay within the existing noise of the 0.6 baseline (59.6 / 58.1 / 58.1 ns per sample at 48 kHz; the PR quotes them, SC-005).

## 5. Records and the README (FR-015, FR-019)

```sh
cargo test -p rr_dr60_harness --test us2_cross_check readme
RUSTDOCFLAGS="-D warnings" cargo doc -p rr_dr60 --no-deps
cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h && git diff --exit-code crates/rr_dr60_ffi/include/rr_dr60.h
```

**T039 walk (2026-10-12):** observed both README tests green, `cargo doc` with warnings denied clean, the header up to date (`git diff --exit-code` empty after regeneration). Expected: the README's table between the `manifest-table` markers equals the renderer's output and the two README code blocks equal `examples/manifest.rs` and `smoke.c`'s `print_manifest()` (regenerate all three with `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`; single-threaded because the two tests write the same file); rustdoc builds warning-free (the doctest ran in step 1); the header is up to date. Then read: the CHANGELOG's 0.7.0 entry (with the stage-4 confidence rationale), `docs/hardware/signal-chain.md`'s Identifier column and "Modeled by" note, the `measured` status in `docs/hardware/assumptions.md`'s rules, the `General` sentence in `docs/hardware/sources.md`, the README's manifest section (the text-fragment note, the guard's scope, the C-naming sentence), `docs/release-checklist.md`'s README item. Before this phase, the three string reviews must have passed (plan › Phase order): the hardware reviewer on descriptions and coverage notes, the product specialist on wording, the coach on the US1 increment.

## 6. Gates before the PR

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
scripts/check-traceability.sh
```

**T038 walk (2026-10-12):** observed fmt and clippy clean, 457 tests passing with no failing suite, coverage **98.20 %** lines (first run 96.19 %: the core's compile-time check functions never execute under the profiler and the harness's error branches had no tests; runtime calls, should-panic tests of every rule and a unit-test module for the parsers, rules and renderers brought `manifest.rs` to 98.99 % and `manifest_checks.rs` to 93.26 %), traceability OK, the header up to date, the MSRV 1.85 suite green in `target/msrv`, release timing one-pass 62.7 / record + play 64.0 ns per sample (ratio 1.021) and 61.3 / 60.0 / 60.3 at 4096 blocks against the 0.6 baseline 62.6 / 64.0 (1.022) and 61.1 / 61.5 / 61.4. Expected: all green; coverage within 1 point of 98.58 % (the new core module is fully exercised by its unit tests and the harness).
