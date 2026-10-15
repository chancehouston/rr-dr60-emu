# Research: Optional Output Clamp (feature 010)

Each entry: decision, rationale, alternatives. IDs are cited from the plan, the data model, the contracts and the tasks.

## R-01 Where the clamp sits and the identity it must satisfy (FR-001, FR-003, FR-005)

**Decision.** The clamp is one boolean on `OutputChain` (`stages/output.rs`), set at construction as `settings.output.clamp_enabled && tap == Tap::RouteOutput`, where `tap` is the chain's return point: `RouteOutput` for the player (every stage runs, no tap to honour) and the configured tap for the one-pass. `OutputChain::process` becomes a wrapper: the existing body moves to an inner function, and the wrapper returns `inner(y).clamp(-1.0, 1.0)` when the boolean is set and `inner(y)` otherwise. The wrapper runs **before** the `active` early return moves inside `inner`, so a bypassed chain at the route-output tap is still bounded (spec Edge Cases "Output chain bypassed"). The per-stage early returns (`AfterVolumeAmp`) and the route taps (`AfterSpeaker`, `AfterEarphone`) are never clamped because the boolean is false for every tap but `RouteOutput`; `AfterPlayback` and every earlier tap never reach the chain. The player's `try_next_host` then narrows: `narrow_out(chain.process(y))`.

**The identity holds exactly in `f32`.** The spec's oracle is `on == clamp(off)` on the narrowed output. `narrow_out` rounds to nearest and is monotone, and 1.0 is representable: for an `f64` sample x ≤ 1.0, `narrow(clamp(x)) = narrow(x) ≤ 1.0 = clamp(narrow(x))`; for x > 1.0, `narrow(clamp(x)) = 1.0` and `narrow(x) ≥ 1.0` so `clamp(narrow(x)) = 1.0`; symmetric below −1.0. So clamping in `f64` before the narrowing and clamping the `f32` output afterwards give the same bits, and the harness can compute the expected output from the option-off run with `f32::clamp`. A core unit test pins this on a vector that crosses the bound (including x = 1.0 + 2⁻⁶⁰, which narrows to 1.0 either way).

**Rationale.** One boolean read per sample on an already-active path; no new state, so `reset`, `at_rest`, `runs_any` and `n_out` (the drain) are untouched: the clamp is memoryless and does not lengthen the chain's settle. Keeping the decision at construction (not per sample from the settings) keeps the per-sample path free of settings reads, as the controls are (009 Complexity Tracking row 1).

**Alternatives.** Clamping in `try_next_host` after `narrow_out` (two places: the player and the one-pass share `PlaybackPath`, so one place in fact; rejected only because the tap gating lives in `OutputChain`, and a second tap check in `paths.rs` would duplicate it). A `ClampStage` struct like the ADC clamp (rejected: it is not a stage, and a struct for one bool reads like one).

## R-02 The settings field and the presets (FR-002, FR-008; clarify Q1)

**Decision.** `OutputSettings` gains `clamp_enabled: bool` (the fourth field; `#[non_exhaustive]`, so adding it is not a Rust-level break for callers that use `..Default::default()` or the constants; struct literals in the harness are updated). `OutputSettings::DEVICE` sets it `false` (the device has no such bound) and `OutputSettings::BYPASSED` sets it `false` (0.8's output). `Default` stays `DEVICE`. No validation (a bool cannot be invalid). The rustdoc on the field, on `OutputSettings` and on `Tap::RouteOutput` carries the FR-003 sentence: with the option on, the route-output tap and the selected route's last per-stage tap (or `AfterPlayback` when the chain is bypassed) are no longer the same signal.

**Rationale.** The spec's Q1: a static property of the host's output format, beside the three stage bypasses it modifies. The presets must both be `false` or `configs::settings_008` (built on `BYPASSED`) would change every pre-009 golden.

**Alternatives.** A field on `Settings` itself (rejected: it belongs to the output group it acts on). A control (rejected by the user for now; the controls discussion is queued).

## R-03 The C layout: 104 → 108 (FR-008; coach C3; contracts/c-api.md)

**Decision.** `RrDr60Settings` gains three fields at the end: `reserved_010: [u8; 5]` at offsets 99 – 103 (the 0.9 layout's tail padding, made explicit and written 0), `output_clamp_enabled: bool` at **104**, and `reserved_010_tail: [u8; 3]` at 105 – 107 (this version's tail, explicit so that 0.11 can refuse 108). `size_of == 108`; `struct_size = 104` is refused as `RR_DR60_SETTING_FIELD_STRUCT_SIZE` by the existing `size < SETTINGS_SIZE` check (a compile-and-run break like 0.5, 0.6, 0.8 and 0.9). `rr_dr60_settings_default` fills 108 with the field `false` and both reserved fields zero. No new status, field enum value, tap or function.

**Rationale.** A `bool` appended alone lands at 99 and leaves the size at 104: a 0.9 caller's `struct_size = 104` would pass and byte 99 would be read as the option, which is uninitialized memory and undefined behaviour in Rust (CLAUDE.md's 009 lesson; coach C3). The 009 pattern (`reserved_009` filling 0.8's padding) is repeated, and this time the version's own tail is reserved too so the pattern ends.

**Alternatives.** Reusing `reserved_009` bytes (rejected: documented "write 0, ignored", not validated; a stale caller's zero would silently mean "off", which happens to be the default, but the convention would be broken for the next field). A `uint32_t` flag word (rejected: the three bools set the convention).

## R-04 The precondition: what exceeds the bound where (FR-011; spec Assumptions; coach C4)

Measured at plan time with a throwaway harness test (release profile, 1 s stimuli, `output_checks::run`; the square wave with a half-period of round(rate ÷ 2000) host samples starting at +1.0; the tone from `stimulus::tone` at phase 0). Peak |sample| at the route output with the option off and, in parentheses, the count of samples beyond ±1.0:

| Rate | Every stage bypassed, 1 kHz sine of amplitude 1.5 | Speaker route, full-scale square at position 1 | Earphone route, full-scale square at position 1 | Speaker route, full-scale 1 kHz tone at position 1 | Speaker route, full-scale tone at the reference position |
|---|---|---|---|---|---|
| 6 000 | 1.2990 (4000 of 6000) | 1.544 (1999) | 1.522 (1999) | 1.493 (1999) | 0.8819 (0) |
| 8 000 | 1.5452 (5939) | 1.676 (1979) | 1.391 (1980) | 1.636 (1979) | 0.9040 (0) |
| 16 000 | 1.5304 (9899) | 1.789 (3963) | 1.379 (7910) | 1.839 (3959) | 0.9644 (0) |
| 44 100 | 1.5267 (23365) | 1.892 (11906) | 1.370 (15876) | 1.868 (13162) | 0.9694 (0) |
| 48 000 | 1.5269 (25739) | 1.892 (15832) | 1.371 (17816) | 1.939 (13858) | 0.9694 (0) |
| 88 200 | 1.5274 (46728) | 1.929 (25791) | 1.371 (33731) | 1.908 (26528) | 0.9697 (0) |
| 96 000 | 1.5275 (49499) | 1.943 (27716) | 1.370 (37602) | 1.948 (29689) | 0.9699 (0) |

**Findings.** (1) The primary stimulus exceeds the bound at every rate, as the spec claims by construction; at 6 kHz a 1 kHz tone has six samples per period and its sample peak is 1.5·sin 60° = 1.299, so the **count** of bounded samples, not a peak near 1.5, is the figure to report there. (2) All three secondary stimuli exceed the bound at **every** rate, so the spec's "which exceed where" precondition is answered: all of them, everywhere; the checks still report the figures rather than tolerance them. (3) The earphone route's square wave overshoot (1.37 – 1.52) comes from the coupling corners, as coach C11 said: spec 009's "only if the input does" is corrected in A-042. (4) The reference-position tone never exceeds the bound at any rate (peak 0.88 – 0.97), so AS4's expected count of zero holds everywhere. (5) The 0.9 defaults with the limiter and the ADC clamp on still exceed the bound on a full-scale square at position 1 (speaker 1.50 – 1.90; earphone 1.04 – 1.07), so the `default_*_clamp` golden configurations clamp something. (6) The sanity case: with every stage bypassed, `RouteOutput` and `AfterPlayback` read the same bits (the chain is inactive), which the clamp will make differ at the route output only.

**Decision.** The spec's Assumptions and FR-011 stand; the figures above go into `data-model.md` as the expected report and are re-printed by `clamp_matrix` (no tolerance; an assertion only that the option-off peak exceeds 1.0 for the four overshooting stimuli at every rate, which the table shows holds, and that it does not for the reference-position tone).

## R-05 Stimuli and the golden file (FR-010; contracts/golden-format.md)

**Decision.** `stimulus::square(freq_hz, amp, fs, len)` is a new helper: a half-period of `round(fs / (2·freq))` samples, starting at `+amp` (spec Key Entities). The golden file `golden-clamp-v1.json` holds 3 stimuli × 5 configurations × 7 rates = **105** entries: `sine_1k_1p5` (amplitude 1.5, 2 s), `square_1k_fs` (±1.0, 2 s), `tone_1k_0` (reused) × `clamp_only` (every stage bypassed, the clamp on, the speaker route), `output_only_speaker_clamp` (speaker, position 1), `output_only_earphone_clamp` (earphone, position 1), `default_output_clamp` (speaker, position 1), `default_earphone_clamp` (earphone, position 1). The generator asserts, for every entry, the identity against the configuration's option-off twin (`clamp_only` ↔ the all-bypassed route output; `x_clamp` ↔ `x`) and, for the two `default_*_clamp` configurations, record-then-play equals the one-pass (spec 008 FR-009's rule). Blessed once, last (`RR_DR60_BLESS=clamp`), guarded by `GOLDEN_CLAMP_V1_SHA256`; the five earlier guards unmoved; CI runs it wherever the five run (the check job, the six golden-matrix targets, the iOS simulator job, `scripts/ios-device-golden.sh`).

**Rationale.** Every entry clamps something (R-04) except `tone_1k_0` on the earphone configurations and at the reference position, which is the "does not overshoot" case the spec asks for. Position 1 on the `default_*` configurations is a deliberate departure from 009's reference position, so the limiter-held full-scale square still rings above the bound.

**Alternatives.** Appending clamp entries to `golden-output-v1.json` (rejected: its guard is closed; FR-002).

## R-06 The measurement harness (FR-005, FR-011; SC-001, SC-002; US1 – US2)

**Decision.** A new module `clamp_checks.rs` with: `identity(make, rate, config, route, volume, stimulus)` running the option on and off and asserting `on[i] == off[i].clamp(-1.0, 1.0)` for every i (bit equality on the `f32`), returning rows with the option-off peak, the count beyond ±1.0 and whether the option-on run contains a sample equal to a bound (required wherever the count is above 0); `per_stage_taps_unclamped(rate, route)` (the taps after stages 11, 12 or 13 bit-identical with the option on and off on the square wave; `AfterPlayback` with the chain bypassed likewise); `bypassed_chain_bounded(rate)` (the `clamp_only` configuration: the route output bounded, `AfterPlayback` above 1.0); `equivalence(rate, route, volume)` reusing `output_checks::chain_equivalence` through a settings-name parameter (the function gains one; the 009 callers pass the old name); the route and volume change timing of 009 re-run with the option on. Tests: `us1_clamp` (US1 and US2 at `measurement_rates()`), `clamp_matrix` (the seven-rate report: every row of R-04's table plus the counts, `all_rates()`), `clamp_edge_cases` (every spec edge case: the bound inclusive, ±∞ and NaN into the core function, volume 0, the bypassed chain, the taps, a route change mid-playback, reconfigure on and off), `golden_clamp`; extended: `determinism` (the five clamp configurations partition-invariant and record-then-play), `alloc_free` (the clamp configurations), `timing` (`clamp_cost`, R-08), `ffi_parity` (the clamp on at the seven rates, both routes), `us2_cross_check` (the README output section and the 0.10.0 CHANGELOG entry pass the denylist; the canonical text's header reads 0.10.0), `mutation` (a clamp moved to ±0.5 by a test hook fails the identity; one case).

**Rationale.** The identity is the whole oracle (coach C5): it subsumes "no sample exceeds", "at least one equals", "inside the bound unchanged" and "no time behaviour", at every rate without rate-specific reasoning. Everything else is the 009 harness with the option on.

**Alternatives.** Peak-only assertions (rejected: weaker, and 009's figures were printed, not pinned).

## R-07 Records and wording (FR-004, FR-009; SC-007, SC-008; constitution VI)

**Decision.** A-042's cell gains, in its headroom sentence, "or turn on the **output clamp** (spec 010), an emulator convenience that bounds the route output at ±1.0 and corresponds to nothing in the device", corrects the earphone statement to "the playback filter's reconstruction overshoot and the coupling corners' overshoot on square-edged material, scaled by the position (1.37 – 1.52 on a full-scale square at position 1)", keeps "a real driver's cone overshoots too, in its own way" and its status gains "amended by spec 010 (2026-10-15): the output clamp". The README's output section: the headroom bullet names the option, its default (off), what it bounds and what it does not, the ×32767 sentence ("+1.0 maps to 32767; a host that scales by 32768 saturates +1.0 to 32767"), and a one-line Rust and C example of turning it on; the earlier "limit or clip it yourself" note (the record-path paragraph) points at the option. CHANGELOG 0.10.0: Added (the option, the field, the C layout, the golden file), Changed (the C struct 104 → 108; the route-output tap differs from the last per-stage tap with the option on), Documentation (A-042 amended; the canonical text re-blessed for the header's version). The manifest: no row, count or string change; `library_version` 0.9.0 → 0.10.0 in the canonical text, re-blessed once (`RR_DR60_BLESS=manifest`); the README manifest table is unchanged (the version is not in it: verify at implementation, re-bless if it is). The signal-chain inventory: unchanged (the option is not in the chain). CLAUDE.md: the 010 status line, the bless command, the report commands. Wording: "output clamp", "an emulator convenience, not a device behavior", "bounds the route output"; "clip" stays the amplifier's word; never "accurate", "protects", "safe for", "the device clips here"; the denylist stems (`manifest_checks::DENYLIST_STEMS`) contain none of "clamp", "bound", "convenience".

**Rationale.** Coach C2, C8, C10 – C12; the claims policy.

## R-08 Cost (FR-007; SC-005)

**Decision.** `timing::clamp_cost` (release, `--ignored`): `default_output` with the codec bypassed at 48 kHz, 30 s of noise, 4096-sample blocks, the speaker route at position 1, the option on and off in **alternation**, five runs each, the **median** of each; assert `median_on − median_off ≤ 2.0 ns`; print both routes. Expected: an `f64::clamp` is two compares and two selects per sample, under 1 ns at 48 kHz on this machine; the 009 figure for the chain is 68.2 ns per sample.

**Rationale.** Coach C9: 3 % of the codec-bypassed cost is about 2 ns, inside the spread of best-of-five figures, so the spec states nanoseconds and a median of alternating runs. The assertion is a smoke bound; the quoted figure comes from a solo run.

## R-09 The smoke loop and parity (US4; contracts/c-api.md)

**Decision.** `smoke.c` adds two cases with the option on: a ±1.0 square wave (exactly representable) at position 1 on each route through record-then-play, hashed (FNV-1a 64) against two new pinned references from `smoke_reference.rs` (`RR_DR60_SMOKE_SPEAKER_CLAMP_HASH`, `RR_DR60_SMOKE_EARPHONE_CLAMP_HASH`), and asserts no output sample is outside [−1, 1] in C; a `struct_size = 104` refusal case. `ffi_parity` gains `c_api_output_clamp_matches_rust_at_all_rates` (the five clamp configurations, both routes where applicable, bit equality).

## R-10 Risks

- **The one-pass's chain with `tap = RouteOutput` and every output stage bypassed** has `active = false` today and returns `y` untouched; the wrapper must sit outside that early return, or the "bypassed chain still bounded" edge case fails silently at the route output. The edge-case test and `clamp_only` golden entries catch it.
- **Harness struct literals** of `OutputSettings` (if any use the literal form rather than the constants) break on the new field; `#[non_exhaustive]` means callers outside the crate cannot use literals, so only the crate's own tests can.
- **The canonical-text re-bless** moves `golden/manifest-v1.txt`; the cross-check's version test (`version_token`) and the README's generated regions must agree; a second re-bless means a string moved, which is a defect under FR-004.
- **The timing assertion** can fail on a loaded CI runner; it is `--ignored` and release-only like the other timing tests, so it does not gate CI.
- **The ×32767 statement** is documentation only; no test can prove a host's conversion. The README sentence is checked by the cross-check's denylist pass and by a reader test at review.
