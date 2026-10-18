# C API contract: Playback Speed (spec 013)

Version 0.13.0 (`RR_DR60_VERSION_MINOR = 13`). The settings struct is unchanged (120 bytes).

## Status

```c
RR_DR60_STATUS_INVALID_SPEED_LEVEL = 25  /* a playback speed level other than 1 … 5 */
```

## Defines

```c
#define RR_DR60_SPEED_LEVEL_COUNT 5
#define RR_DR60_SPEED_DEFAULT_LEVEL 3
#define RR_DR60_SPEED_RATIO_DENOMINATOR 5
#define RR_DR60_SPEED_RATIO_NUMERATOR_1 3
#define RR_DR60_SPEED_RATIO_NUMERATOR_2 4
#define RR_DR60_SPEED_RATIO_NUMERATOR_3 5
#define RR_DR60_SPEED_RATIO_NUMERATOR_4 6
#define RR_DR60_SPEED_RATIO_NUMERATOR_5 7
```

Each is a literal `pub const` in `rr_dr60_ffi` (cbindgen emits a define only for a literal), pinned to `rr_dr60::speed` by a unit test.

## Functions

```c
/* FR-001: sets the level; RR_DR60_STATUS_INVALID_SPEED_LEVEL leaves it unchanged. */
RrDr60Status rr_dr60_set_playback_speed(RrDr60Pipeline *pipeline, uint8_t level);

/* The last valid level (default 3). */
RrDr60Status rr_dr60_get_playback_speed(RrDr60Pipeline *pipeline, uint8_t *out_level);

/* FR-009: the host samples rr_dr60_play produces for recording `number` at a constant
   `level`, the drain included. INVALID_SPEED_LEVEL first, then NO_SUCH_RECORDING. */
RrDr60Status rr_dr60_playback_length_samples(RrDr60Pipeline *pipeline, uint32_t number,
                                             uint8_t level, uint64_t *out_samples);
```

Null handles and null out-pointers give `RR_DR60_STATUS_NULL_POINTER`, as for every function; panics never unwind (the `guarded` wrapper).

## The play report

```c
typedef struct RrDr60PlayInfo {
    size_t produced;
    bool finished;
    size_t frames;
    uint64_t frame_index;
    uint64_t place;   /* Appended in 0.13 (the struct grows 32 → 40 bytes on 64-bit targets) */
} RrDr60PlayInfo;
```

An **ABI break** (as 008's growth): the struct has no size field, so a host built against the 0.12 header and linked against 0.13 is overrun by 8 bytes; recorded in the CHANGELOG's 0.13.0 "Changed" with the instruction to rebuild against the new header.

## Smoke loop and parity

`smoke.c` gains one case (as built): the loop's existing full-scale square recording in 0.11's default (the codec on; exactly representable, 006's lesson) at 48 kHz, played on the speaker route at volume 0.1 at level 5 and at level 1 from play block 10, the output hashed (`RR_DR60_SMOKE_SPEED_HASH`, pinned in `smoke_reference.rs` from the Rust run) and the refused levels 0 and 6 checked. `ffi_parity` adds: set/get/refuse, the length helper against `playback_length_samples`, and a playback with level changes bit-identical to the Rust API's.
