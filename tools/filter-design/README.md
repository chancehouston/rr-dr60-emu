# Voice-band filter design tool

`design_voiceband.py` designs the filter used by the RR-DR60 emulator's voice-band stages:
signal-chain stage 4 (record anti-alias filter + ADC) and stage 10 (playback DAC +
reconstruction filter). It checks the design against every FR-010 tolerance in
[spec 001](../../specs/001-pipeline-skeleton/spec.md), then writes
`crates/rr_dr60/src/stages/voiceband_coeffs.rs`.

The design is a 5th-order Butterworth high-pass at 300 Hz plus a 6th-order elliptic low-pass
at 3380 Hz, at the 8 kHz device rate. It is normalized to 0 dB at 1 kHz and is minimum-phase.
The reasons for this design are in [research.md R-07](../../specs/001-pipeline-skeleton/research.md).
It is **modeled on** the MSM7702 codec's assumed G.712-like template (A-002, A-014, A-015,
A-016). It has not been measured against a real unit.

## Usage

Run these from the repository root. They need [uv](https://docs.astral.sh/uv/), which fetches
numpy and scipy automatically.

```sh
uv run tools/filter-design/design_voiceband.py           # design, verify, write the coefficient file
uv run tools/filter-design/design_voiceband.py --check   # design and verify only; writes nothing
uv run tools/filter-design/design_voiceband.py --shift-hz 100 --emit-fixture <path>   # SC-008 test fixture
```

The script prints a margin table. It exits non-zero, and writes nothing, if any FR-010 bound
is not met.

## Why the coefficients are committed

The generated file stores every coefficient as an exact bit pattern (`f64::from_bits`). The
Rust build never runs Python, so the coefficients are identical on every platform (R-04).
CI does not regenerate the file, because scipy's results depend on the platform math library.
Instead, the Rust tests re-verify every FR-010 bound from the committed coefficients.

## Changing the design

Any change to this script or its output changes the emulator's sound. **Any change requires
a golden re-bless, a CHANGELOG entry, and an assumption-register check** (Constitution II):

1. Run the script and review the margin table.
2. Re-bless the golden files:

   ```sh
   RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden
   ```

3. Add a `CHANGELOG.md` entry that explains the change in output.
4. Update the affected A-### entries in `docs/hardware/assumptions.md` (value, status, evidence).
