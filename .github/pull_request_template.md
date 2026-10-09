## Summary

<!-- What does this change and why? Link the spec (specs/NNN-...) or issue. -->

Closes #

## Checklist

- [ ] Follows the [constitution](../.specify/memory/constitution.md) and linked spec
- [ ] Modeled values cite a source (`S-###`) or assumption (`A-###`); assumption register updated if values changed
- [ ] Tests added/updated (measurement tests with tolerances for DSP changes)
- [ ] Feature/stage is testable on its own (no dependence on unbuilt or unrelated stages)
- [ ] Coverage at or above 80% and not lowered by more than 1 point (`cargo llvm-cov`)
- [ ] Real-time path stays allocation-, lock-, and I/O-free
- [ ] `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` pass
- [ ] Public API documented; C API changes reflected in the generated header
- [ ] CHANGELOG.md updated (Unreleased) for user-visible changes
- [ ] Golden files: unchanged, **or** re-blessed on purpose (`RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden`) with a CHANGELOG entry explaining why the output changed
- [ ] C header regenerated with cbindgen if the C API changed (CI checks for drift)
- [ ] Every new or changed numeric default or tolerance cites an `A-###`/`S-###` ID or is labeled "engineering target" (`scripts/check-traceability.sh`)
