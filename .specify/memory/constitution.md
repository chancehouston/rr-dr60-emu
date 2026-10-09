# rr-dr60-emu Constitution

## Core Principles

### I. Portable Library First

- The core crate (`rr_dr60`) MUST be pure Rust. It MUST NOT depend on platform or OS audio APIs,
  file I/O, or UI frameworks.
- A stable, panic-safe C API with a cbindgen-generated header MUST be the integration surface for
  Swift/iOS and other languages. Panics MUST NOT unwind across the FFI boundary, and Rust-only
  types MUST NOT appear in the C API.
- File formats (e.g. WAV), CLIs, and tooling MUST live outside the core crate, in separate crates or
  behind features.
- The iOS app MUST live in a separate repository and consume this library as a dependency.

Rationale: the library's value is that any host (iOS apps, desktop tools, other languages) can embed
it unchanged.

### II. Evidence-Based Fidelity (NON-NEGOTIABLE)

- Every modeled behavior and default parameter value MUST trace either to a source (`S-###` in
  `docs/hardware/sources.md`) or to a registered assumption (`A-###` in
  `docs/hardware/assumptions.md`). Code, tests, and specs MUST cite the ID.
- Reasonable assumptions are permitted when hardware specifications are unknown. Each MUST record
  its rationale, confidence (High/Medium/Low), and a verification method.
- Changing an assumed or sourced value is a user-visible change. It MUST be recorded in
  `CHANGELOG.md` and in the assumption's status.
- The v1 fidelity target is a measured character match within stated tolerances. Bit-exact
  reproduction is a non-goal unless original codec firmware or algorithms are recovered.

Rationale: without real-unit references, honesty about what is known versus assumed is the only
thing that makes the emulator trustworthy and improvable as evidence arrives.

### III. Measured, Test-First Verification

- Specs MUST state DSP behavior as measurable properties with tolerances, e.g. "−3 dB points within
  ±50 Hz of 300 Hz and 3400 Hz".
- Measurement tests (tones, sweeps, noise, impulses, level steps) MUST be written before the
  implementation and fail first.
- Golden-file regression tests MUST guard against unintended output changes. Golden files are only
  updated on purpose, with a CHANGELOG entry.
- `cargo fmt --check`, `cargo clippy -D warnings`, and the full test suite MUST pass before merge.
- Workspace line coverage, measured with `cargo-llvm-cov`, MUST stay at or above **80%**. No pull
  request may lower it by more than 1 percentage point. CI enforces the gate as soon as the Cargo
  workspace exists.
- Excluded from coverage: test code, benchmarks, generated code (e.g. the cbindgen header), and
  examples. Any other exclusion (e.g. an unreachable FFI panic guard) MUST be marked in code with
  a justification comment.

Rationale: "sounds right" can't be reviewed or kept from regressing. Measurements can. Coverage is a
backstop that finds untested code. It is not proof of correctness, which the measurement tests
provide.

### IV. Deterministic & Real-Time Safe

- The same input, configuration, and seed MUST produce identical output on all supported platforms.
  Noise and randomness MUST come from a seeded PRNG. DSP MUST NOT use wall-clock time or OS
  randomness.
- The per-block processing path MUST NOT allocate memory, take locks, perform I/O, or do unbounded
  work. Resources are allocated at construction or configuration time.
- A single block-based engine MUST serve both offline (file) and real-time (streaming) use.

Rationale: app creators need glitch-free live audio and reproducible results for testing and
support.

### V. Modular, Traceable Signal Chain

- Each hardware stage (see `docs/hardware/signal-chain.md`) MUST be independently configurable,
  bypassable, testable, and tap-able, so callers can take the output after any stage.
- Model what is audible or measurable. Circuit-level (SPICE-style) simulation is prohibited unless
  a spec justifies it with a measurable benefit.
- New complexity MUST be justified in the plan's Complexity Tracking section (YAGNI).

Rationale: modular stages let evidence for one component be applied without destabilizing the rest,
and let users pick exactly the character they want.

### VI. Neutral, Honest Communication

- Documentation, API docs, and marketing text MUST describe only signal processing behavior.
- The project MUST acknowledge the EVP / paranormal-investigation community respectfully. It MUST
  NOT claim, for or against, that the device or emulator captures paranormal phenomena.
- Accuracy MUST NOT be advertised beyond what has been measured. Unverified behavior is described
  as "modeled on" or "assumed".

Rationale: credibility with audiophiles, app developers, and investigators alike depends on
neutrality and honesty.

### VII. Independently Testable Features

- Every feature MUST be deliverable and verifiable on its own. Its spec MUST define acceptance
  criteria that can be tested without features that are unbuilt or unrelated.
- Every user story within a feature MUST be independently testable. The P1 story alone MUST form a
  working, demonstrable increment.
- Tests for a stage or feature MUST NOT depend on other stages' behavior. Any other stage in the
  chain is bypassed, or replaced with a defined test double or synthetic input.
- Tasks MUST include a checkpoint that validates each story independently before the next story
  begins.

Rationale: when features and stages are proven in isolation, new hardware evidence can change one
part without hiding regressions in another. It also keeps every merged increment shippable.

## Public Repository Standards

- License: MIT. Community files are maintained: README, CONTRIBUTING, CODE_OF_CONDUCT (Contributor
  Covenant), SECURITY (GitHub private vulnerability reporting, no published email), CHANGELOG
  (Keep a Changelog), and issue/PR templates.
- Semantic Versioning applies to both the Rust public API and the C API. Breaking either requires
  a MAJOR bump once 1.0.0 is released.
- Commit messages follow Conventional Commits.
- All public items MUST have rustdoc documentation. `unsafe` code is permitted only in the FFI
  layer, and each block needs a `// SAFETY:` comment.
- Large audio files MUST NOT be committed. Fixtures are small or generated in code.

## Development Workflow

- Non-trivial changes follow the GitHub Spec-Kit flow: specify → clarify → plan → tasks →
  analyze → implement. Specs live in `specs/NNN-feature-name/`.
- Specs stay technology-agnostic (WHAT and WHY). Technology choices belong in the plan.
- The `speckit-coach` agent is available to review specs, plans, and tasks before advancing a
  stage.
- `CLAUDE.md` provides runtime development guidance for AI agents. It MUST NOT contradict this
  constitution.

## Governance

- This constitution supersedes all other project practices and guidance documents.
- Amendments are made via pull request. They MUST state the rationale and bump the constitution
  version under semantic versioning: MAJOR for removed or redefined principles, MINOR for added or
  materially expanded principles or sections, PATCH for clarifications.
- Every spec, plan, and pull request MUST be checked for compliance (the plan's Constitution Check
  and the PR checklist). Deviations MUST be justified in Complexity Tracking or rejected.

**Version**: 1.1.0 | **Ratified**: 2026-10-08 | **Last Amended**: 2026-10-08
