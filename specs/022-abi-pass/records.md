# Records: The Pre-1.0 C ABI Pass (spec 022)

| Record | Change | Gate |
|---|---|---|
| `include/rr_dr60.h` | Regenerated: the eleven size fields, the flattened outcome, `stream`, the four defines, the getter, the check (`uintptr_t`), status 37; the contract comment "always written when not NULL and its size is accepted". | the header drift check |
| `crates/rr_dr60_ffi/cbindgen.toml`, the FFI crate doc | The contract comment amended; the new consts included. | the drift check |
| `specs/001-pipeline-skeleton/contracts/c-api.md` | Dated notes on rule 2 (the accepted-size condition) and rule 5 (the 0.22 promise, the settings exception), as the 003 / 006 amendments did. | review |
| `docs/guide.md` § C / Swift | The lead (rebuild **and initialize every report**, a run-time break), the migration block, the rule with the cap and the read-back, the inputs' rules (D13), the frozen pair by mechanism, the check and what a mismatch means, 64-bit only (`wasm32` and 32-bit Android refused), the zero-initializer note, the example `RrDr60BlockInfo info = { .struct_size = sizeof info };` with its comment, the promise with "planned". | review; `us2_cross_check` guide blocks |
| `docs/guide.md` § controls, § playback speed, the record example | Delete / reword "`RrDr60Control` … no size field; they join the pre-1.0 ABI pass" and "`RrDr60PlayInfo.place` since 0.13 (the struct grew 32 -> 40)"; the record example's `flush_*` names. | review |
| `README.md` | The Platforms row: "C ABI on 64-bit targets". | review |
| `CHANGELOG.md` | Under Changed, product M6's lead: no audio change, Rust unchanged, **breaking for C and Swift callers: rebuild once and initialize every report**, the three edits, the sizes table (old → new, 64-bit), the frozen pair, the check, the cap, D13, the promise with "planned". | review |
| `docs/product/decisions.md` | One row (the pass; 64-bit only; the change entry frozen, correcting the 2026-10-31 row's item; D13; the promise by mechanism). | review |
| `docs/product/brief.md` | The distribution line: "64-bit targets only (iOS is 64-bit; on Android `arm64-v8a` and `x86_64`, which Play serves to every current device; 32-bit-only devices are not offered the app)". | review |
| `swift/README.md` | `import RRDR60` for the helpers vs the bare `RRDR60C`; `.sized` (never on settings), `checkABI()` and what a mismatch means, the raw module's trap, 64-bit, the `.iOS(.v16)` note; the last line amended (the helpers and the re-render are the Swift-native surface). | review |
| `specs/018-battery-state/contracts/api.md`, `specs/020-session-contract/contracts/api.md` | Dated notes: the cap on every sized struct, Meters' read-back, the readout's padding and embedded-settings rule, `StopFields` frozen, `RunData`'s cap. | review |
| `scripts/check-traceability.sh` | `check_file crates/rr_dr60_ffi/src/lib.rs` added so the new literals (4096, 16, 40, 1, 37) are audited with their "spec 022 D…" citations. | the audit |
| `docs/hardware/*` | No change (no assumption). | — |
| `CLAUDE.md` | The status entry, the Spec 022 reports commands, the engine order (022 done → c1 next); the "pre-1.0 ABI pass" list cleared; the Miri result. | review |
| Version 0.22.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the manifest's version line (one re-bless), the CHANGELOG. | `us2_cross_check` |
| Memory bookmark | The step reached. | — |
