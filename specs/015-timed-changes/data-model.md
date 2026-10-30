# Data model: Sample-Timed Control Changes and the Change Log (015)

## Stream (`rr_dr60::Stream`)

`#[non_exhaustive]`, `Copy`, `#[repr(u32)]`: `Playback = 0`, `OnePass = 1`, `Record = 2` (reserved: every call refuses it with `ControlNotOnStream`). Each stream has:

| State | Type | Reset by | Spec |
|---|---|---|---|
| position (the clock) | `u64`, host output samples produced; the next sample has this index | the playback stream at `start_playback`; the one-pass at construction, `reset`, `reconfigure` | D2, FR-001 |
| schedule | `Schedule` (below) | the playback stream at `reset`, `reconfigure` and the end of a playback by any path; the one-pass at `reset`, `reconfigure` | D4, FR-002 |
| last applied values | `Route`, `f32` volume target; the player also `u8` level | set without an entry by `start_playback` (player) and `reset`/`reconfigure` (one-pass) to the engine's readings | D5, R-07 |
| pending plain change | `Option<(Route, f32)>`; the player also `Option<u8>` | set at every processing call's entry; cleared at the first produced sample (the chain pair) or the first device pull (the level) | FR-004, R-07 |
| staging log | `[ChangeEntry; 67]`, `len` | cleared at every call's entry | R-06 |

Invariants: `position` equals the voice-activation events' clock on the one-pass (every event's `output_position` ≤ `position` after the call that reported it, and equals the index of a produced sample); on the player `position ≤ total` once the total is known.

## Schedule

`Schedule { entries: [Entry; 64], len: u8, next_seq: u32 }` inside the path; `Entry { position: u64, seq: u32, control: u8, value: f64, tag: u64 }`.

- Ordered by `(position, seq)`; `seq` increments per insertion; `earliest()` = `entries[0].position` or `u64::MAX`.
- `len ≤ 64` (`SCHEDULE_CAPACITY`); an insertion at `len == 64` is refused (`ScheduleFull`).
- `control` is the control manifest's index of `route`, `volume` or `playback_speed`; the speed only on the playback stream (`ControlNotOnStream` otherwise).
- `value`: the route 0.0 or 1.0; the volume an `f64` exactly representable as `f32` in 0 … 1; the level 1.0 … 5.0 integral.
- A pop removes `entries[0]`.

Transitions: *pending* → *applied* (popped at the first produced sample with index ≥ position, or the device pull with index ≥ d(position) for the speed) → an entry in the log; *pending* → *cleared* (the schedule's reset) → nothing.

## Change entry (`rr_dr60::ChangeEntry`)

`Copy`, `#[repr(C)]`, `#[non_exhaustive]` in Rust (fields public), 40 bytes:

| Field | Type | Offset | Meaning |
|---|---|---|---|
| `position` | `u64` | 0 | the host output index the change landed on (the stream's clock) |
| `before` | `f64` | 8 | the stream's last applied value (chained: equals the previous entry's `after` for this control on this stream, or the stream's starting value) |
| `after` | `f64` | 16 | the value applied |
| `tag` | `u64` | 24 | the schedule call's tag; 0 for a plain setter |
| `control` | `u32` | 32 | the control manifest's index |
| `reserved` | `u32` | 36 | 0 (a later use; the entry has no size field: pre-1.0 ABI-pass list) |

Values: a route 0.0 / 1.0, a volume the `f32` position widened, a level 1.0 … 5.0. `ChangeEntry::default()` is all zero.

Invariants: one entry per change a stream applies; `before == after` only for a scheduled no-op (`tag` may be 0 only if the change was plain, and a plain no-op is never logged); on one stream, for one control, entries are in non-decreasing `position`; the count per call ≤ 67 (`MAX_CHANGES_PER_CALL`).

## The landing rules (D3), by control

| Control | Stream | Applied | First affected | Logged position |
|---|---|---|---|---|
| volume | both | before `chain.process` of sample P (`try_next_host`) | sample P (transition sample 1; or at once in spec 011 FR-005's cases) | P (or the index landed on for a passed position) |
| route | both | the same point, `reset_sections` then the position | sample P, the new branch from rest | the same |
| speed | playback | before the `speed.next` call with device-output index d(P) = ⌊(P − 1) · 6000 ÷ rate⌋ + 1 (d(0) = 0) | device output d(P) (audible after the playback latency) | P (plain: the host index under production when the pull happens) |

A passed position (P < position) lands on the next produced sample (index = position) and its speed, if any, on the next device pull.

## Constants and errors

- `SCHEDULE_CAPACITY: usize = 64`, `MAX_CHANGES_PER_CALL: usize = 67` (Rust `rr_dr60::changes::*`; C `RR_DR60_SCHEDULE_CAPACITY`, `RR_DR60_MAX_CHANGES_PER_CALL`).
- `Error::ScheduleFull { stream }` (C status 27 `SCHEDULE_FULL`), `Error::ControlNotOnStream { control, stream }` (28 `CONTROL_NOT_ON_STREAM`), `Error::InvalidControlValue { control, value }` (C: `INVALID_ARGUMENT` for a route not 0/1; the volume and the level map to their existing statuses).
- The reports: `BlockInfo` and `PlayInfo` gain `changes: usize` (both are `#[non_exhaustive]`; the count of changes the call applied, filled by every variant; only the `*_with_changes` variants write entries); in C the count is an out-parameter of the log-taking functions and the existing report structures are unchanged.
