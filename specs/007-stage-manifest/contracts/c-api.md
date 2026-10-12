# Contract: C API additions (feature 007)

Additive (0.6 → 0.7). Rules 1–5 of the header comment hold; nothing existing changes (no struct, enum value, function or status code moves). `RrDr60Settings` is unchanged at 88 bytes.

## `RrDr60Status` (appended)

| Value | Name | Meaning |
|---|---|---|
| 20 | `RR_DR60_STATUS_NO_SUCH_STAGE` | the stage number is 0 or above the stage count |

## New enums (`repr(u32)`; carried in `uint32_t` fields like `RrDr60State`)

```c
typedef enum { RR_DR60_STAGE_STATUS_NOT_MODELED = 0, RR_DR60_STAGE_STATUS_ASSUMED = 1,
               RR_DR60_STAGE_STATUS_SOURCED = 2, RR_DR60_STAGE_STATUS_CHECKED = 3 } RrDr60StageStatus;
typedef enum { RR_DR60_STAGE_PATH_RECORD = 0, RR_DR60_STAGE_PATH_PLAYBACK = 1, RR_DR60_STAGE_PATH_BOTH = 2 } RrDr60StagePath;
typedef enum { RR_DR60_CONFIDENCE_NOT_APPLICABLE = 0, RR_DR60_CONFIDENCE_LOW = 1,
               RR_DR60_CONFIDENCE_MEDIUM = 2, RR_DR60_CONFIDENCE_HIGH = 3 } RrDr60Confidence;
```

The three enum names are added to `cbindgen.toml`'s `[export] include` list (they appear only inside `uint32_t` fields, as the transport enums do).

## New structs (`repr(C)`)

```c
typedef struct {
  const char *library_version;     /* "0.7.0" */
  uint32_t device_rate_hz;         /* 6000 (A-026) */
  uint32_t stage_count;            /* 15 */
  uint32_t count_not_modeled, count_assumed, count_sourced, count_checked;   /* sum == stage_count */
  const char *basis;
  const char *evidence_link, *assumptions_link, *sources_link;
} RrDr60ManifestHeader;             /* 64 bytes on 64-bit targets, 44 on 32-bit */

typedef struct {
  uint32_t number;                 /* 1..15 */
  const char *identifier, *name, *description;
  uint32_t status, path, confidence;          /* RrDr60StageStatus, RrDr60StagePath, RrDr60Confidence */
  uint32_t source_count, assumption_count;    /* for the two index accessors */
  const char *coverage, *controls, *model_version;   /* "" when empty, never NULL */
} RrDr60StageEntry;                 /* 80 bytes on 64-bit targets, 48 on 32-bit */
```

Every `const char *` is non-NULL, NUL-terminated UTF-8, points at static storage inside the library and stays valid and unchanged for the life of the process (keep it by reference; never free it). On the Rust side both structs implement `Default` (null pointers, zeros) for the tests; the C header is unaffected. Both structs grow only at the end; before 1.0 a new trailing field is a minor change that needs a recompile, after 1.0 new data arrives through new accessor functions.

## Functions

| Function | Returns | Errors (no write on error) |
|---|---|---|
| `uint32_t rr_dr60_manifest_stage_count(void)` | 15 | — |
| `RrDr60Status rr_dr60_manifest_header(RrDr60ManifestHeader *out)` | `OK`, `*out` filled | `NULL_POINTER` |
| `RrDr60Status rr_dr60_manifest_stage(uint32_t number, RrDr60StageEntry *out)` | `OK`, `*out` filled | `NULL_POINTER`; `NO_SUCH_STAGE` for 0 or > 15 |
| `RrDr60Status rr_dr60_manifest_stage_source(uint32_t number, uint32_t index, const char **out_id)` | `OK`, `*out_id` = the i-th `S-###` (0-based, ascending) | `NULL_POINTER`; `NO_SUCH_STAGE`; `INVALID_ARGUMENT` when `index >= source_count` |
| `RrDr60Status rr_dr60_manifest_stage_assumption(uint32_t number, uint32_t index, const char **out_id)` | likewise for `A-###` | likewise |

All five are real-time safe (no allocation, no I/O, no lock), take no handle, may be called before any `rr_dr60_create` and from any thread, and are listed in the header comment's real-time-safe list. Check order: NULL pointers first (`NULL_POINTER`), then the stage number (`NO_SUCH_STAGE`), then the index (`INVALID_ARGUMENT`); nothing is written on a refusal. Panic safety: `guarded` takes a handle, so these functions wrap their body in their own `catch_unwind` and map a caught panic with `panic_to_status(None)` to `INTERNAL_ERROR` (no handle to poison); the `Err` arm is unreachable for static data and is marked `// coverage: unreachable, static data (Constitution III)`. `number` is `u8` in Rust and `uint32_t` here; parity compares `u32::from(entry.number)`.

## Smoke test additions (`crates/rr_dr60_ffi/tests/c/smoke.c`, `run_smoke.sh`)

- Normal mode: `rr_dr60_manifest_stage_count() == 15`; the header's counts sum to 15 and `count_checked == 0`; stage 0 and 16 → `NO_SUCH_STAGE` with the output struct's `number` left at a sentinel; every stage's `source_count` and `assumption_count` reachable through the accessors and `index == count` → `INVALID_ARGUMENT`; a pointer kept from stage 3's `name` is `strcmp`-equal after a create/destroy cycle (US3 scenario 3); every string pointer non-NULL.
- `print_manifest()`: a self-contained function between `/* manifest-example-c:start */` and `end` comment markers that walks the header, the fifteen stages and their IDs through the five functions and prints the canonical rendering (data-model.md) with `printf`; the README's C block is this function verbatim (equality-tested by `tests/us2_cross_check.rs`). `smoke --manifest` calls it and exits; `run_smoke.sh` runs it and `diff`s against `cargo run -q -p rr_dr60_harness --example manifest` (SC-001).

## Parity (`crates/rr_dr60_harness/tests/ffi_parity.rs`)

For every number 1..=15 the C entry equals the Rust entry field by field (strings compared as `CStr::to_str()` against `&str`; enums by their `u32` values; the ID lists through the accessors); the header likewise; the refusals as stated. Added to `alloc_free`: the five functions over all stages and IDs allocate nothing.

## Naming rule stated in the README (FR-019; clarify 2026-10-12)

The controls notes use the Rust paths verbatim. The README says once: "C names derive from the Rust names: `RrDr60Settings` fields are the Rust field path in snake case with the group as prefix (`agc.knee_dbfs` → `agc_knee_dbfs`, `vas.hang_ms` → `vas_hang_ms`, `memory.capacity_s` → `memory_capacity_s`), top-level fields keep their names (`record_stage_enabled`), enum variants are `RR_DR60_` plus the type and the variant in screaming snake case (`Tap::AfterAgc` → `RR_DR60_TAP_AFTER_AGC`, `Route::Speaker` → `RR_DR60_ROUTE_SPEAKER`), functions are `rr_dr60_` plus the method name, and the value readers `route` and `state` take `get_` (`rr_dr60_get_route`, `rr_dr60_get_state`); `EngineState.speaker_muted` is `RrDr60EngineState.speaker_muted`."
