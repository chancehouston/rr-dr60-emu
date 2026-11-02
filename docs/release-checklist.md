# Release checklist

Work through this list for every release of `rr_dr60` and `rr_dr60_ffi`. Copy it into the
release PR or issue and tick each item. The reason for each item is in brackets.

## Automated (CI on the release PR)

- [ ] **CI is green on every job**: `check` (ubuntu, macos), `coverage`, `golden-matrix` (all
      desktop targets in the matrix), `ios`, and `msrv`. [FR-014, SC-003, Constitution III]
- [ ] Coverage is at least 80% of lines and has not dropped more than 1 point since the last
      release. [Constitution III]
- [ ] The C header is up to date (the CI header-drift step passes). [FR-023]

## Manual gates (not covered by hosted CI)

- [ ] **Golden files on a physical iOS device.** Run `scripts/ios-device-golden.sh` with a
      device connected (it runs fourteen test binaries: determinism, every golden file and the
      manifest texts; the script's header lists the device setup), then paste its PASS line here:

      ios-device-golden: PASS on <model> (iOS <version>), rr_dr60 <version>, <commit>

      Last run: `ios-device-golden: PASS on iPhone 15 Pro (iOS 27.2), rr_dr60 0.91.0, f4e64f2`
      (2026-11-01). The 0.90.0 attempts the same day failed: `determinism`, then
      `golden_speed`, read golden files from the build machine's path, which does not exist on
      the phone, and cargo stops at the first failing binary, so the golden binaries after it
      never ran (an earlier report that they passed was wrong). PRs #65 and #66 build those
      files into the binaries. Previous PASS: iPhone 15 Pro (iOS 27.0.1), rr_dr60 0.13.0,
      `a19690d` (2026-10-18, the first device run). Run it again on the release commit.

      [FR-014, SC-003; clarified 2026-10-08: the iOS device check must pass before each release]
- [ ] **Windows ARM64**, only if the `windows-11-arm` hosted runner was removed from
      `golden-matrix`. On a Windows ARM64 machine, run:

      cargo test -p rr_dr60_harness --all-features --test golden --test golden_agc --test golden_vas --test golden_codec --test determinism

      Record the machine and OS version here. [FR-014]
- [ ] The release-mode timing checks pass on a developer machine:
      `cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture`.
      Record the ns/sample and the "x real time" figure here. [FR-015, SC-006]

## Content

- [ ] **The golden file is unchanged since the last release**, or every change to it has a
      CHANGELOG entry explaining why the output changed. [Constitution III, FR-021]
- [ ] **The assumption register is current** (`docs/hardware/assumptions.md`). Every value the
      code uses has an A-/S- ID, and any change to an assumed value is in the CHANGELOG.
      [Constitution II, FR-018]
- [ ] `scripts/check-traceability.sh` passes (CI runs it in the `check` job). [SC-007]
- [ ] **The README's manifest table and code blocks equal the manifest's output**
      (`cargo test -p rr_dr60_harness --test us2_cross_check readme`); a changed stage string,
      status, confidence, coverage note or model version is in the CHANGELOG. [007 FR-015, FR-019]
- [ ] **SemVer:** the version is bumped in `Cargo.toml` (workspace) for both the Rust API and the
      C API. The C version defines (`RR_DR60_VERSION_*` in `crates/rr_dr60_ffi/src/lib.rs`)
      match, and the header has been regenerated. Breaking changes to either surface need a
      MAJOR bump after 1.0 (MINOR while 0.x). [Constitution: Public Repository Standards]
- [ ] `CHANGELOG.md`: move `[Unreleased]` into a dated version section.
- [ ] Documentation describes signal processing only, and says "modeled on" or "assumed" for
      unmeasured behavior. [Constitution VI]

## After release

- [ ] Tag `vX.Y.Z` on `main`, and create the GitHub release with the CHANGELOG section.
