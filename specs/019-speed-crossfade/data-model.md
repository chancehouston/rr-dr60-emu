# Data Model: Speed-Change Crossfade (019)

## SpeedStage (crates/rr_dr60/src/stages/speed.rs), the additions

| Field | Type | Meaning |
|---|---|---|
| `fade` | `Option<Fade>` | the running crossfade; `None` at a constant level |
| `weights` | `Box<[f64]>` (120) | `w[n − 1] = s(n ÷ 120)`, filled at construction (`fade_len_delta` changes the length) |
| `ring` | `Box<[f32]>` (256) | was 128: the two branches' span |

`Fade { old_k: u32, old_p5: u64, n: u32 }`: the old branch's numerator and place (fifths), the blend samples produced so far (1 … N).

### State transitions

- **Constant level** (`fade == None`): as 0.18, sample for sample (the steady-identity fast path at level 3; the general step otherwise).
- **Request** (host level ≠ active, not latched, `fade == None`): level 3 waits for a whole place; then `fade = Some { old_k: active, old_p5: p5, n: 0 }`, `active = requested`.
- **Blend sample** (`fade == Some`): `n += 1`; for n < N `y = y_old(old_p5) + w[n − 1]·(y_new(p5) − y_old(old_p5))`, at n = N `y = y_new(p5)`; `old_p5 += step(old_k)`; `p5 += step(active)`; if `n == N` then `fade = None`. The pulls follow the leading branch.
- **Request during a fade**: not applied (`requested` holds the latest target); applied when the fade ends under the same rules.
- **Latch** (the new branch's place reaches K): as 0.18; a running fade continues through the tail; the player's drain cuts the remainder.

### Invariants

- 0 < w[0] < … < w[N − 1] = 1; w[29] = 0.15625 exactly.
- The place `p5` and the length rule are 0.18's at every moment (the fade reads the old branch, it never moves the place).
- The highest stored index read by either branch is at most `i_new + 124` (the old branch leading when the old level is faster), the lowest at least `i_new − 124`: a span of 153 within the 256-sample ring (debug-asserted).

## Harness reference (speed_checks::reference_output_at)

Adds `(old_k, old_p5, n)` and the same rules; `value_at` for both branches; the blend in f64 (`y_old + w·(y_new − y_old)` for n < N, `y_new` at n = N) then narrowed. Its weights come from the pinned expression `u * u * (3.0 - 2.0 * u)`, `u = n / 120`, and are checked equal to the engine's hook.

## Golden files

- `golden-speed-v1.json`: schedule entries re-blessed; constant-level entries asserted byte-identical at bless time.
- `golden-changes-v1.json`: `speed_inside` entries re-blessed (their produced counts move: two requests one device sample apart, the second waited); every other entry asserted byte-identical.
- Before either re-bless, the `fade_step` build (no fade, the request applied at once) regenerates the committed entries bit for bit (the gate).
- `smoke_reference::{SMOKE_SPEED_HASH, SMOKE_SPEED_SAMPLES}` and `smoke.c`'s defines: re-pinned (the count unchanged, asserted).

No new golden file; no change to any golden format.

## Mutants (`__test_hooks::SpeedMutation`)

`fade_step`, `fade_linear`, `fade_len_delta: i32`, `fade_no_wait`.
