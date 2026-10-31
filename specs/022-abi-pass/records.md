# Records: The Pre-1.0 C ABI Pass (spec 022)

| Record | Change | Gate |
|---|---|---|
| `include/rr_dr60.h` | Regenerated: the eleven size fields, the flattened outcome, `stream`, the four defines, the getter, the check, status 37; the contract comment "always written when not NULL and its size is accepted". | the header drift check |
| `crates/rr_dr60_ffi/cbindgen.toml` | The contract comment amended; the new consts included. | the drift check |
| `docs/guide.md` § C / Swift | The rule with the cap and the read-back, the frozen pair, the check and its call, the rebuild, 64-bit only, the zero-initializer note, the example setting `struct_size`, the promise in D4's wording. | review |
| `CHANGELOG.md` | Under Changed: "**Breaking (0.21 → 0.22) for C callers: rebuild once.**", the sizes table (old → new, 64-bit), the frozen pair and the rename, the check, the cap, the promise. | review |
| `docs/product/decisions.md` | One row (the pass; 64-bit only; the change entry frozen, correcting the 2026-10-31 row's item; the promise by mechanism). | review |
| `docs/product/brief.md` | The distribution line: "64-bit targets only". | review |
| `swift/README.md` | `.sized`, `checkABI()`, the raw module's trap, 64-bit, the `.iOS(.v16)` note. | review |
| `specs/018-battery-state/contracts/api.md`, `specs/020-session-contract/contracts/api.md` | Dated notes: the cap on every sized struct, Meters' read-back, `StopFields` frozen. | review |
| `docs/hardware/*` | No change (no assumption). | `scripts/check-traceability.sh` (the new literals 4096, 16, 40, 1, 37 cited) |
| `CLAUDE.md` | The status entry, the Spec 022 reports commands, the engine order (022 done → c1 next); the "pre-1.0 ABI pass" list cleared. | review |
| Version 0.22.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the manifest's version line (one re-bless), the CHANGELOG. | `us2_cross_check` |
| Memory bookmark | The step reached. | — |
