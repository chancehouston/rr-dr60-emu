## Summary

<!-- What does this change and why? Link the spec (specs/NNN-...) or issue. -->

Closes #

## Checklist

- [ ] Follows the [constitution](../.specify/memory/constitution.md) and linked spec
- [ ] Modeled values cite a source (`S-###`) or assumption (`A-###`); assumption register updated if values changed
- [ ] Tests added/updated (measurement tests with tolerances for DSP changes)
- [ ] Real-time path stays allocation-, lock-, and I/O-free
- [ ] `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` pass
- [ ] Public API documented; C API changes reflected in the generated header
- [ ] CHANGELOG.md updated (Unreleased) for user-visible changes
