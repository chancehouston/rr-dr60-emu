# Filter design tools

One `uv` inline script designs the emulator's fixed filters and writes their coefficients into
the core crate as exact bit patterns. Run it from the repository root; it needs
[uv](https://docs.astral.sh/uv/), which fetches numpy and scipy automatically.

The record-path AGC (stage 3) has no designed filter since spec 005: it is a limiter modeled
on the schematic's circuit, and its reference model lives in `tools/agc-model/` (pure Python,
no dependencies; `python3 tools/agc-model/limiter_model.py`).

## `design_voiceband.py`: the codec band-limit stages (stages 4 and 10)

Spec 004 ([research R-04, R-05, R-06](../../specs/004-device-rate-6khz/research.md)), at the
device's measured **6 kHz** rate (A-026, S-002). The two stages are different filters, each
modeled on the OKI codec family's datasheet template (S-005) with every frequency scaled by 0.75
to the 6 kHz clock (A-027), designed to the datasheet's typical values with its limits as
tolerances:

- **Record stage** (stage 4, the codec's transmit path): a 3rd-order Butterworth high-pass at
  112.5 Hz plus a 5th-order Chebyshev-II (inverse Chebyshev) low-pass with a 32 dB stopband. The
  script searches the low-pass stopband edge (1 Hz steps) for the highest edge whose loss at
  2550 Hz is within 0.1 dB of the typical 0.4 dB while the group-delay distortion at 1950 Hz
  keeps 0.015 ms of margin under its 0.17 ms limit and the 2550 Hz loss stays positive.
- **Playback stage** (stage 10, the receive path): the same low-pass alone, tuned toward the
  typical 0.56 dB at 2550 Hz. It has no low-frequency rejection: the receive filter passes DC.
- Both are normalized to exactly 0 dB at 765 Hz (0.75 × the datasheet's 1020 Hz reference,
  A-015) and are minimum-phase by construction (A-016).

The family, order, stopband attenuation and the edge-search margins are engineering targets
(spec 004 FR-015); the loss and group-delay limits are S-005 scaled (A-027); the 40 dB
attenuation at DC is an engineering target (ITU-T G.712, S-007, states no DC requirement). The
response between 2550 Hz and the 3 kHz Nyquist frequency is not constrained by any source; the
script reports the loss at 2700 and 2900 Hz so the choice stays visible. None of this has been
measured against a real unit.

```sh
uv run tools/filter-design/design_voiceband.py            # design, verify, write the coefficient file
uv run tools/filter-design/design_voiceband.py --check    # design and verify only; writes nothing
uv run tools/filter-design/design_voiceband.py --family ellip --check   # comparison table only
uv run tools/filter-design/design_voiceband.py --shift-record-hp-hz 100 --emit-fixture <path>
uv run tools/filter-design/design_voiceband.py --shift-record-lp-hz -100 --emit-fixture <path>
uv run tools/filter-design/design_voiceband.py --shift-playback-lp-hz -100 --emit-fixture <path>
```

The script prints a margin table per stage and exits non-zero, writing nothing, if any bound of
spec 004 FR-005, FR-006 or FR-008 is not met. The three `--shift-*` forms write the SC-007
mutation fixtures in `crates/rr_dr60_harness/tests/fixtures/`: one edge moved 100 Hz toward the
band (the high-pass up, a stopband edge down).

## Why the coefficients are committed

The generated files store every coefficient as an exact bit pattern (`f64::from_bits`). The
Rust build never runs Python, so the coefficients are identical on every platform (001 R-04).
CI does not regenerate the files, because scipy's results depend on the platform math library.
Instead, the Rust unit tests re-verify every template bound from the committed coefficients.

## Changing a design

Any change to this script or its output changes the emulator's sound. **Any change requires
a golden re-bless of all three golden files, a CHANGELOG entry, and an assumption-register
check** (Constitution II):

1. Run the script and review the margin tables.
2. Re-bless the golden files, once, in one commit:

   ```sh
   RR_DR60_BLESS=1   cargo test -p rr_dr60_harness --test golden
   RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc
   RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas
   ```

   and update the SHA-256 guards in `crates/rr_dr60_harness/tests/golden_agc.rs`.
3. Add a `CHANGELOG.md` entry that explains the change in output.
4. Update the affected A-### entries in `docs/hardware/assumptions.md` (value, status, evidence).
