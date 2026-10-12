# Quickstart: validating feature 007 (the stage manifest)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 006 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test; `cbindgen` 0.29.4 for the header).

## 1. Story 1: the manifest from Rust (FR-001 – FR-012, FR-016)

```sh
cargo test -p rr_dr60 manifest
cargo test -p rr_dr60_harness --test manifest -- --nocapture
cargo run -q -p rr_dr60_harness --example manifest | head -20
```

Expected: the core's unit tests pass (fifteen entries, the compile-time invariants, the C twin table equals the strings plus NUL); the harness prints zero findings on the committed registers and zero wording findings; `manifest()` read twice is equal; the header reads `counts: not_modeled=10 assumed=5 sourced=0 checked=0`, `library_version: 0.7.0`, `device_rate_hz: 6000`; the example prints the canonical text starting with `rr_dr60 manifest`.

## 2. Story 2: the drift guard (FR-013, FR-014, FR-020)

```sh
cargo test -p rr_dr60_harness --test manifest doctored -- --nocapture
scripts/check-traceability.sh
```

Expected: each of the seven doctored inputs produces the stated finding (contracts/cross-check.md › Tests, item 2), printed as `rule stage id`; the traceability script prints `check-traceability: OK` and, if a manifest ID is removed from a register by hand, `manifest cites unknown ID`. To see the guard bite for real: change `| A-028 |`'s Status cell in `docs/hardware/assumptions.md` to `superseded`, run the first command, see `Superseded stage 3 A-028`, revert. The `registers` workflow (`.github/workflows/registers.yml`) runs the same two commands on a PR that touches only `docs/hardware/`, `CHANGELOG.md` or `README.md`; check the PR's checks list for `registers`. To make it a required check (optional; see research R-06 for the trade-off): repository Settings → Branches → `main` → required status checks → add `registers`.

## 3. Story 3: the same from C (FR-017)

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: the FFI layout tests pass (`RrDr60ManifestHeader` 64 bytes and `RrDr60StageEntry` 80 bytes on a 64-bit host; status 20); parity for all fifteen stages and the header, `NO_SUCH_STAGE` for 0 and 16, `INVALID_ARGUMENT` for an index at the count; zero allocations across the five C functions; `smoke: OK (rr_dr60 0.7.0, latency 483 samples at 48000 Hz)` followed by `smoke --manifest matches the Rust example` (the diff is empty).

## 4. Nothing else moved (FR-018, SC-004, SC-005)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test determinism
git diff --stat main -- crates/rr_dr60_harness/golden/*.json
```

Expected: the three golden tests pass with their SHA guards unchanged and the `git diff` is empty (no JSON changed); `determinism` also compares the canonical text with `golden/manifest-v1.txt`.

## 5. Records and the README (FR-015, FR-019)

```sh
cargo test -p rr_dr60_harness --test manifest readme
cargo doc -p rr_dr60 --no-deps
cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h && git diff --exit-code crates/rr_dr60_ffi/include/rr_dr60.h
```

Expected: the README's table between the `manifest-table` markers equals the renderer's output (regenerate with `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test manifest readme`); rustdoc builds warning-free with the doctest; the header is up to date. Then read: the CHANGELOG's 0.7.0 entry, `docs/hardware/signal-chain.md`'s Identifier column and "Modeled by" note, the `measured` status in `docs/hardware/assumptions.md`'s rules, the `General` sentence in `docs/hardware/sources.md`, the README's manifest section and C-naming sentence, `docs/release-checklist.md`'s README-table item.

## 6. Gates before the PR

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
scripts/check-traceability.sh
```

Expected: all green; coverage within 1 point of 98.58 % (the new core module is fully exercised by its unit tests and the harness).
