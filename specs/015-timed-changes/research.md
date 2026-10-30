# Research: Sample-Timed Control Changes and the Change Log (015)

Each entry: decision, rationale, alternatives. Code references are to `main` at the 0.15 merge (`f1449c2`). The spec's reviews (coach 1 – 18, hardware B1 – B10, product M1 – M9) are applied in the spec; the plan reviews are recorded in plan.md when they land.

## R-01: Where the clock, the schedule and the "last applied" values live

**Decision.** Each stream is a `PlaybackPath` (`paths.rs:832`: the one-pass's `one_pass.play` and the player's `player`), so the stream state lives there: `produced: u64` (the clock, D2), a `Schedule` of 64 entries (R-03), the stream's last applied `route`, `volume` and (the player only) `speed_level` for the log's before field (D5), a `pending_plain: Option<(Route, f32)>` set at every processing call's entry from the engine's control values (today's `set_controls` at entry, `pipeline.rs:683`, `transport.rs:465`), and a **staging log** of up to `MAX_CHANGES_PER_CALL` entries drained into the caller's buffer at the end of the call (R-06). The `Pipeline` keeps `route`, `volume` and `speed_level` as the control readings, moved at landing for scheduled changes and at once for the plain setters (FR-004).

**Rationale.** The three play-side objects that must agree (the clock, the chain the changes act on, the before values) are all reachable from the path; the one-pass and the player then share one implementation, and the player's `cursor.produced` (`transport.rs:154`) becomes the path's counter (the cursor keeps `total`).

**Alternatives.** The schedule on the `Pipeline` with a stream tag (rejected: the apply point is inside the path's host-sample production, R-02, and two streams would need two apply paths anyway); a separate `Stream` object the host holds (rejected: a second handle in C).

## R-02: The apply point: lazily, at the first produced sample at or past the position

**Decision.** The route and the volume are applied in `PlaybackPath::try_next_host` (`paths.rs:922`), **immediately before `self.chain.process(y)`**, for the sample whose index is `produced`: while the earliest pending position ≤ `produced`, pop it and apply (`chain.set_controls` with the entry's values merged into the stream's current pair); the plain pending pair, if any, is applied first at the same point (a scheduled change at the same boundary then wins, D4). The chain's `process` is called exactly once per produced host sample on both streams (the one-pass's `tick` returns `Some` iff `try_next_host` returned `Some`; the play loop writes one output per `Some`), so this point *is* output sample P, and a call that produces no sample (a zero-length call, a block the voice activation drops) applies nothing (spec Edge Cases).

**Bit-identity with 0.15 for the plain setters** (FR-007): today `set_controls` runs at the call's entry; between the entry and the first `chain.process` nothing touches the chain (the transition advances only inside `inner`'s `ramp_step`, `output.rs:447`; `reset` only clears sections; `take_at_once` and `start_transition` set state the next `process` reads), so applying at the first produced sample gives the same state at the first `process` and bit-identical output. For a call that produces nothing, 0.15 had already changed the chain's state at entry; the lazy form changes it at the next call's first sample instead, with the same effect on every produced sample (the chain is only observed through `process`). The harness proves both: the nine SHA guards and the block-size-1 oracle (R-08).

**The speed** is applied in the play loop (`transport.rs:522`), **immediately before `speed.next(..)`**, for the device-output index `n_dev` the loop counts (one per `speed.next` call since `start_playback`; `Own`, `Tail` and `Silence` alike): while the earliest pending speed position's d(P) ≤ `n_dev`, pop it and `speed.request(level)`; the plain pending level first. Today's `speed.request(*speed_level)` at the call's entry (`transport.rs:480`) is only read by the next `speed.next` (`apply_request`, `speed.rs:331`), so requesting right before that call is bit-identical.

**Rationale.** The two production points are the only places where "sample P" and "device sample d(P)" are known; a per-block split cannot be precomputed on the one-pass, whose production is irregular under the voice activation (hardware review). The cost is one `u64` comparison against a cached earliest position per produced sample on the hot path (R-09).

**Alternatives.** Splitting the caller's block at the positions and running the existing entry-time `set_controls` per sub-block (rejected: wrong on the one-pass, where the sub-block boundary is in input samples; and a route reset mid-block would still need the sections-only reset, R-05); applying at the top of the tick that reaches `produced == P` (rejected: the tick may produce nothing, and the entry would then be reported one call early).

## R-03: The schedule: a sorted inline array of 64

**Decision.** `Schedule { entries: [Entry; 64], len: u8, next_seq: u32 }`, `Entry { position: u64, seq: u32, control: u8, value: f64, tag: u64 }`; the schedule call validates (R-04), then inserts at the first index whose `(position, seq)` is greater (a memmove of at most 63 entries, bounded, no allocation); `earliest()` is `entries[0].position` or `u64::MAX`. A pop takes `entries[0]` and shifts (bounded; at most 67 pops per call). Positions are `u64` as the event clock; `seq` is a per-schedule counter that makes ties stable. Capacity `SCHEDULE_CAPACITY = 64` and `MAX_CHANGES_PER_CALL = 67` are published constants.

**Rationale.** Sixty-four 40-byte entries are 2.5 KB per stream inside the path (no `Box`); insertion keeps the hot path to one comparison; the memmove costs nothing on the audio path (it runs in the schedule call).

**Alternatives.** A binary heap (no gain at 64, and ties need a sequence anyway); a ring in submission order with a scan per sample (rejected: 64 comparisons per sample).

## R-04: Validation at the schedule call

**Decision.** `schedule_change(stream, position, control, value, tag)` validates in this order and stores nothing on a refusal: (1) `control` is one of the three live controls by control-manifest index (`controls::index_of("route" | "volume" | "playback_speed")`), else `Error::ControlNotOnStream` (the same error for a control the manifest has but the stream's paths lack, e.g. the speed on the one-pass, and for a record-path setting); (2) the value as the plain setter validates it: `InvalidVolume` (finite, 0 … 1; the value is an `f64` and must be exactly representable as `f32`, else `InvalidVolume`), `InvalidSpeedLevel` (an integer 1 … 5), the route an integer 0 or 1 (else `InvalidArgument` in C, `ControlNotOnStream`'s sibling `InvalidControlValue` in Rust); (3) on the one-pass stream, a route value the configured tap cannot read (`tap_on_route`'s rule, `pipeline.rs:562`) → `TapNotOnRoute` (B1); (4) `len == 64` → `ScheduleFull`. `Stream::Record` → `ControlNotOnStream`. The call is accepted in every engine state.

**Rationale.** The spec's refusals, each by an existing or distinct error, nothing stored (FR-001, FR-002).

**Alternatives.** Validating at landing (rejected: a refused value would be silently dropped on the audio path).

## R-05: The route switch and spec 016's accumulators

**Decision.** `OutputChain::reset` (`output.rs:500`) is split into `reset_sections` (the filters, the fresh flag, the knob at rest: what a route switch needs) and `reset` (sections plus `route_peak = 0`, `clamp_count = 0`: construction, `reset`, `reconfigure`, `start_playback`). `set_controls` calls `reset_sections` on a route change. Bit-identical at a block boundary because `finalize_into` had just zeroed the accumulators there (spec 016 R-01).

**Rationale.** B4: a mid-call route switch must not lose the samples before P from the call's route peak and clamp count (FR-003).

## R-06: The log: a staging buffer drained per call

**Decision.** Each path has `log: [ChangeEntry; 67]` and `log_len`; `apply` pushes an entry when the stream's own value moved (volume target or route; the player's requested level), with `position` = the host index under production (`produced` for the chain controls; for a scheduled speed change the scheduled P, for a plain speed change the host index the pull is for), `control`, `before` = the stream's last applied value, `after` = the applied value, `tag` (0 for a plain change). A scheduled change to the value already applied is pushed with `before == after` (FR-006); a plain pending pair equal to the last applied is not. At the end of the call the log-taking variant copies the first `min(log_len, buffer.len())` entries to the caller and returns `log_len` as the count; the plain variants discard. `log_len` never exceeds 67: at most 64 scheduled pops plus one plain change per control (the route and volume pair gives at most two entries, the speed one).

**Rationale.** `try_next_host` has no caller buffer; staging keeps the per-sample code free of slices, and the copy is bounded.

**Alternatives.** Passing the caller's slice down the path (rejected: three signatures change); a pull API like the meters (rejected by D5).

## R-07: The plain setters, the lifecycle calls and the cross-stream entry

**Decision.** At every processing call's entry, `pending_plain = Some((self.route, self.volume))` on the stream's path (today's `set_controls` call site), and on the player `pending_speed = Some(self.speed_level)`; the lazy apply (R-02) compares with the stream's last applied values and logs only a move. `start_playback` (`transport.rs:418`) resets the player and sets its controls at once as today, then sets `last_applied = (route, volume, speed_level)` with no entry; `reset` and `reconfigure` do the same for the one-pass path. So a volume set during a playback is logged by the player at its next produced sample and again by the one-pass at its next produced sample (B2), each with tag 0; a setter made before `start_playback` is consumed without an entry (coach 6).

**The control readings** (`route()`, `volume()`, `playback_speed()`) are the `Pipeline` fields: a plain setter writes them at once; a scheduled change writes them when it lands (the apply sets the field through a shared `&mut` passed into the path's apply: the path returns the landed value and the transport/pipeline writes the field after the call? No: the readings must move at landing even mid-call, but they are only observable between calls, so the path records `landed: Option<(Route, f32, u8)>` and the call's epilogue writes the fields; equivalent at every observation point).

## R-08: The oracles

| Oracle | Form | Where |
|---|---|---|
| The block-size-1 plain setter (primary, every control, every rate, both streams) | a reference engine of the same configuration run one host sample per call; before the call that produces P the plain setter is applied; the candidate engine schedules the same change at P and runs at block sizes 1, 64, 4096 and the irregular partition; outputs bit-identical, logs identical (positions from the schedule) | `changes_checks::setter_oracle`; `us1_changes` AS1, AS6 |
| spec 011's splice identity | constant-position runs spliced at P at `AfterVolumeAmp` | `volume_ramp_checks` reused with P in place of the boundary; AS2 |
| spec 009's route oracle | the new route's from-rest run from P | `output_checks` reused; AS3 |
| spec 013's reference at 6 kHz | `speed_checks::reference_output` generalized to a device-index schedule `&[(u64, u8)]` (today `(call, level)`), d(P) computed in the test by the formula | AS4; the overridden deferred switch and the post-latch change as schedule cases |
| the log | the schedule plus the plain calls → the expected entries (chained before/after, tags) | `us2_changes` |
| the meters across a mid-call route switch | `Meters::route_peak` equals the max over the call's `RouteOutput` tap on both routes | AS9 |
| the clock | a route change at a splice event's `output_position` starts the new branch on the event's first post-splice sample; `stream_position` equals the produced count | `changes_edge_cases` |
| the irregular partition | `Blocks::Irregular(seed)` of `volume_ramp_checks.rs:95` extended to 1 … 4096 with a prime and a length above 1000 asserted present, and a variant with a forced boundary at a given position (AS6) | `changes_checks::IRREGULAR` |

## R-09: Cost

**Decision.** The idle cost is one `u64` compare per produced sample on each stream (`earliest ≤ produced`, with `earliest = u64::MAX` when empty; the plain pending `Option` is checked once per call, not per sample: it is applied at the first produced sample and cleared) plus, on the player, one compare per device pull. Measured by `tests/changes_timing.rs` (the 0.15 API: a 4096-block playback and a one-pass at 48 kHz, speaker route, codec bypassed, fastest of 45 runs, printed as `playback_ns=` / `one_pass_ns=`), run in both builds by a new `changes` mode of `scripts/ab-timing.sh` (the `meters` mode's pattern: the test copied into the baseline tree, three alternations), asserting 3 % (`RR_DR60_CHANGES_COST_CEILING`, 0.05 on the label job); a second print with one change scheduled per block (`playback_pending_ns=`), reported.

**Alternatives.** A per-block `next_position` precheck skipping the per-sample compare when no change falls in the block (possible on the player, whose production is regular; rejected for now: two code paths, and the compare is about 0.3 – 0.5 ns of 73).

## R-10: The golden and the C parity

**Decision.** `golden-changes-v1.json`: per entry the configuration, rate, stream, the schedule (positions, controls, values, tags), the block partition, the output digest (FNV-1a over the narrowed samples, as the codec golden's frame digest) and the full log; the entries: the three controls at inside-block positions on both streams, two at one position, a passed position, a scheduled no-op, the plain setters (one per control), the cross-stream entry, a change in the drain, a speed change past the latch, 64 + 3 in one call (SC-004's tight bound); at the seven rates with `output_only_amp`-style and default configurations; blessed once. The C smoke gains `--changes`: it schedules one entry's changes, runs it and prints the log and the digest; diffed against `examples/changes_rows.rs`.

## R-11: The records

The stage manifest's API items of stage 9 (`Pipeline::set_playback_speed`, …) and stage 11 (`Pipeline::set_volume`, `Pipeline::set_route`, …) gain `Pipeline::schedule_change` (verbatim paths; the 120-character limit checked; one canonical re-bless); the control manifest's text is unchanged (the setter scan of spec 014 sees no new `set_` name). The guide gains a "Timed changes and the change log" section; the real-time list the new calls; the CHANGELOG 0.16.0; the product notes of FR-012.

## R-12: The recording counter reading (D9)

`Pipeline::recording_counter() -> u64` returns `transport.counter` (`transport.rs:187`); C `rr_dr60_recording_counter(p, out *u64)`. No setting.

## R-13: The mutants

`ChangeMutation { apply_at_entry: bool (0.15's entry-time apply: caught by the partition identity), off_by_one_position: bool (lands at P + 1: caught by the splice identity), submission_order: bool (ignores position order: caught by AS8), log_reading_before: bool (before = the engine's reading, not the stream's: caught by the cross-stream entry), full_reset_on_route: bool (caught by AS9) }` behind `__test-hooks`.
