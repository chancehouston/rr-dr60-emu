# Contract: golden files after the single narrowing re-bless (feature 006)

Format unchanged (`rr_dr60-golden`, version 1). File names unchanged. `library_version` 0.6.0. Entry counts unchanged.

| File | Entries | Change in this feature |
|---|---|---|
| `golden-v1.json` | 112 | re-blessed **once**, in the narrowing commit (FR-009; research R-02): the device-rate stream after the VAS is rounded to `f32` in the one-pass processing, so entries may differ by up to 4 full-scale LSBs (2⁻²³ × max(1, peak); worst case 2.05 from the L1 norms, research R-02); the `bypass_all` entries at 6 kHz are expected unchanged (no arithmetic before the narrowing), those at the converting rates may change like any other |
| `golden-agc-v1.json` | 42 | re-blessed once, same commit; the 21 `agc_only` entries (host-rate tap, no device chain) are expected unchanged |
| `golden-vas-v1.json` | 63 | re-blessed once, same commit |

## Procedure (one commit, before any split code)

1. `cargo test -p rr_dr60_harness --test narrowing_delta -- --nocapture` prints, per file, the share of entries and samples that change and the maximum |Δ| in full-scale LSBs (2⁻²³ × max(1, peak); bound 4, derived from the L1 norms, research R-02), and asserts that exactly the 6 kHz `bypass_all` entries and the `agc_only` entries are unchanged.
2. `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden`; `RR_DR60_BLESS=agc … --test golden_agc`; `RR_DR60_BLESS=vas … --test golden_vas`.
3. Update `GOLDEN_V1_SHA256`, `GOLDEN_AGC_V1_SHA256` and the VAS guard; full suite green.
4. CHANGELOG 0.6.0 `### Changed` records the figures from step 1.

**The golden window closes here.** Any later golden difference inside the feature is a defect.

## The SC-002 reproduction rule (`golden::applicable_entries()`)

| Entries | How record-then-play reproduces them |
|---|---|
| 112 of `golden-v1.json`, 21 `default_agc`, 21 `default_vas` (default tap) | record with the entry's settings, stop, play with the same settings; the first `n_samples` of the pre-drain output hash to the entry's `sha256`; VAS events converted from device to host positions match |
| 21 `vas_only`, 21 `vas_mute` (tap after VAS) | as above with `playback_stage_enabled = false` for playback |
| 21 `agc_only` (host-rate tap after the limiter) | not reproducible by record-then-play; excluded; the one-pass golden test still covers them |

196 of 217 entries; the count is asserted so the test cannot skip entries silently.
