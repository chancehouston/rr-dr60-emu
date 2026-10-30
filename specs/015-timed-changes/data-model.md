# Data model: Sample-Timed Control Changes and the Change Log (015)

## Stream (`rr_dr60::Stream`)

`#[non_exhaustive]`, `Copy`, `#[repr(u32)]`: `Playback = 0`, `OnePass = 1`, `Record = 2` (reserved: every call refuses it with `ReservedStream`). In C the enum `RrDr60Stream` (`RR_DR60_STREAM_PLAYBACK`, `RR_DR60_STREAM_ONE_PASS`, `RR_DR60_STREAM_RECORD`), emitted through cbindgen's `include` like `RrDr60Route`; the functions take its integer twin (the spec 014 pattern). Each stream has:

| State | Type | Reset by | Spec |
|---|---|---|---|
| position (the clock) | `u64`, host output samples produced; the next sample has this index; counted at every tap, the one-pass's front taps included | `PlaybackPath::reset`: the playback stream at `start_playback`; the one-pass at construction, `reset`, `reconfigure` | D2, FR-001 |
| schedule | `Schedule` (below) | **never by `PlaybackPath::reset`**; an explicit clear from `Pipeline::reset` and `reconfigure` (both streams), and for the playback stream from `play` setting `finished`, `stop_playback`, `erase` of the played item, `erase_all` while playing; kept by `start_playback` | D4, FR-002 |
| `next_apply` | `u64`: 0 while a plain change is pending, else the schedule's cached earliest position, `u64::MAX` when nothing is pending | with the path | R-09 |
| last applied values | `Route`, `f32` volume target; the player also `u8` level | set without an entry by `start_playback` (player) and `reset`/`reconfigure` (one-pass) to the engine's readings | D5, R-07 |
| pending plain change | `(Route, f32)`; the player also `u8` | set at every processing call's entry; applied at the first produced sample | FR-004, R-07 |
| `landed` | the last landed value per control in the call | cleared at the call's entry; written to the readings in the epilogue | R-07 |
| staging log | `[ChangeEntry; 67]`, `len` | cleared at every call's entry | R-06 |

Invariants: `position` equals the voice-activation events' clock on the one-pass (every event's `output_position` is the index of a produced sample); on the player `position ≤ total` once the total is known.

## Schedule

`Schedule { entries: [Entry; 64], len: u8, next_seq: u64, earliest: u64 }` inside the path; `Entry { position: u64, seq: u64, value: f64, tag: u64, control: u32 }` (40 bytes).

- Ordered by `(position, seq)`; `seq` increments per insertion; `earliest` is kept equal to `entries[0].position` or `u64::MAX`.
- `len ≤ 64` (`SCHEDULE_CAPACITY`); an insertion at `len == 64` is refused (`ScheduleFull`).
- `control` is the control manifest's index of `route`, `volume` or `playback_speed`; the speed only on the playback stream (`ControlNotOnStream` otherwise).
- `value`: the route 0.0 or 1.0; the volume the request narrowed to `f32` (0 … 1, −0.0 normalized) and widened; the level 1.0 … 5.0 integral.
- A pop removes `entries[0]`.

Transitions: *pending* → *applied* (popped at the top of the first host sample with index ≥ position) → an entry in the log; *pending* → *cleared* (an explicit clear) → nothing.

## Change entry (`rr_dr60::ChangeEntry`)

`Copy`, `#[repr(C)]`, `#[non_exhaustive]` in Rust (fields public), 40 bytes:

| Field | Type | Offset | Meaning |
|---|---|---|---|
| `position` | `u64` | 0 | the host output index the change landed on (the stream's clock) |
| `before` | `f64` | 8 | the stream's last applied value (chained: equals the previous entry's `after` for this control on this stream, or the stream's starting value) |
| `after` | `f64` | 16 | the value applied (the volume as the stored `f32` widened) |
| `tag` | `u64` | 24 | the schedule call's tag; 0 for a plain setter (apps tag from 1) |
| `control` | `u32` | 32 | the control manifest's index |
| `reserved` | `u32` | 36 | 0; the pre-1.0 ABI pass assigns it to the stream (a merged session file needs self-describing entries; the entry has no size field: ABI-pass list) |

Values: a route 0.0 / 1.0, a volume the `f32` position widened, a level 1.0 … 5.0. `ChangeEntry::default()` is all zero.

Invariants: one entry per change a stream applies; `before == after` only for a scheduled no-op (a plain no-op is never logged); on one stream, for one control, entries are in non-decreasing `position`; the count per call ≤ 67 (`MAX_CHANGES_PER_CALL`).

## The landing rules (D3), by control

| Control | Stream | Applied | First affected | Logged position |
|---|---|---|---|---|
| volume | both | at the top of host sample P (`apply_due`, before the chain processes it) | sample P (transition sample 1; or at once in spec 011 FR-005's cases; no audible effect at a tap before the chain) | P; for a passed position the stream's position at the call's entry |
| route | both | the same point: `reset_sections` then the position | sample P, the new branch from rest | the same |
| speed | playback | the same point: `speed.request` at the top of host sample P, before any pull for it | device output d(P) = ⌊(P − 1) · 6000 ÷ rate⌋ + 1 (d(0) = 0), the pull that reads the request; audible after `playback_latency_samples()`; a d(P) never pulled (the tail before the total) is applied and logged with no effect | P; for a passed position the stream's position at the call's entry (its d is that position's) |

A plain change lands on the first sample the next call produces (all three controls alike) and is logged there if it moves the stream.

## Constants and errors

- `SCHEDULE_CAPACITY: usize = 64`, `MAX_CHANGES_PER_CALL: usize = 67` (Rust `rr_dr60::changes::*`; C `RR_DR60_SCHEDULE_CAPACITY`, `RR_DR60_MAX_CHANGES_PER_CALL`, literal consts in the FFI crate pinned to the core's by a unit test).
- `Error::ScheduleFull { stream }` (C status 27 `SCHEDULE_FULL`), `Error::ControlNotOnStream { control, stream }` and `Error::ReservedStream { stream }` (both 28 `CONTROL_NOT_ON_STREAM`), `Error::InvalidControlValue { control, value }` (C `INVALID_ARGUMENT`; the volume and the level map to their existing errors and statuses).
- The reports: `BlockInfo` and `PlayInfo` (both `#[non_exhaustive]`) gain `changes: usize`, the changes the call applied, filled by every variant; only the `*_with_changes` variants write entries. In C the count is an out-parameter of the log-taking functions and the existing report structures are unchanged (folded into them at the ABI pass).
- `changes::speed_landing(position, host_rate_hz) -> Result<u64, Error>` (`UnsupportedHostRate` for a rate not in `SUPPORTED_HOST_RATES`; never a division by 0); C `rr_dr60_speed_landing(position, rate, out) -> status`.
