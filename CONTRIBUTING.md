# Contributing to rr-dr60-emu

Thanks for your interest! Contributions of code, documentation, research, and **hardware evidence** are all welcome.

By participating you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Hardware evidence (most valuable right now).** If you own a working RR-DR60, recordings of test signals made through it let us verify or correct our [assumptions](docs/hardware/assumptions.md). Open a *Hardware evidence* issue and it will walk you through what to capture.
- **Research.** Datasheets, service-manual details, teardown photos. Add them to [docs/hardware/sources.md](docs/hardware/sources.md).
- **Bug reports and feature requests.** Use the issue templates.
- **Code and docs.** See the workflow below.

## Development setup

1. Install Rust (stable) via [rustup](https://rustup.rs).
2. Optional, for spec work: install [uv](https://docs.astral.sh/uv/), then `uv tool install specify-cli --from git+https://github.com/github/spec-kit.git`.
3. Build and check:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   # coverage (one-time setup: cargo install cargo-llvm-cov; rustup component add llvm-tools-preview)
   cargo llvm-cov --all-features --workspace --fail-under-lines 80
   ```

## Workflow: spec first

We use [GitHub Spec-Kit](https://github.com/github/spec-kit). Non-trivial changes start as a spec in `specs/NNN-feature-name/` and go through specify → clarify → plan → tasks → implement. Small fixes (typos, obvious bugs) can go straight to a PR.

Ground rules (from the [constitution](.specify/memory/constitution.md)):

- **Traceability.** Every modeled behavior cites a source (`S-###`) or a registered assumption (`A-###`). No unexplained magic numbers.
- **Measured tests.** DSP changes come with measurement tests (tones, sweeps, noise) and stated tolerances.
- **Independently testable.** Each feature, user story, and stage is testable on its own, without unbuilt or unrelated features.
- **Coverage.** Workspace line coverage stays at or above 80%, and a PR may not lower it by more than 1 point.
- **Determinism.** Same input + config + seed = identical output on every platform.
- **Real-time safety.** No allocation, locks, or I/O in the per-block processing path.
- **Neutral tone.** Docs describe signal processing only, with no claims about paranormal phenomena.

## Pull requests

- Branch from `main`. Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:` …).
- Keep PRs focused, and link the spec or issue they implement.
- Update [CHANGELOG.md](CHANGELOG.md) under **Unreleased** for user-visible changes, including any change to an assumed value.
- CI (fmt, clippy, tests) must pass.

## Audio files

Don't commit large audio files. Keep test fixtures small, or generate them in code. Hardware captures can be attached to issues or linked externally.
