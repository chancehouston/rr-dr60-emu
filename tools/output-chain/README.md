# Output-chain design tool (spec 009)

`design_output_chain.py` derives every constant of signal-chain stages 11 – 13 (the volume
control and power amplifier, the speaker template, the earphone output) from the circuit as read
from the service manual's schematic sheet 3.1 (S-002) and the amplifier datasheet (S-006), and
writes them to `crates/rr_dr60/src/stages/output_coeffs.rs` as exact `f64` bit patterns.

Every figure it prints is a **design constant of the model** (A-010, A-031, A-032, A-042 – A-044
in `docs/hardware/assumptions.md`), not a measurement of a real RR-DR60. The Rust pins the
figures this script prints (a core unit test carries them as literals); the Rust never pins
its own output.

## What it prints

- the inputs: the clip ceiling (1.15 V at the 2.8 V rail the schematic marks (S-002); 1.0 – 1.4 V registered), the
  gain resistors, the corners (32, 159, 120 and 280 Hz), the speaker template's figures, the knee;
- the loaded volume law at six positions on both routes (A-044), the reference position
  `VOLUME_REFERENCE` (A-043, solved so that codec full scale meets the ceiling; rounded to the
  `f32` the Rust default carries), the "6 dB over" position for the clip checks;
- the knee's verified table (peak and THD of a 1 kHz sine at 48 kHz at six levels re the ceiling,
  and the 10 % THD crossing);
- per host rate: each first-order corner's −3 dB point recomputed from its coefficients, the
  speaker template's peak frequency and its 1 kHz and 2.5 kHz readings, the discrete-against-analog
  deviation of the template, the 1 kHz group delay of each route, the earphone's 1 kHz level, the
  amplifier passband's flatness, the drain's settle `N_OUT` and the volume-0 decay time;
- every FR-005 – FR-008 bound of the spec, checked analytically.

## Usage

From the repository root (`uv` installs `numpy` and `scipy` from the script's header):

```sh
uv run tools/output-chain/design_output_chain.py            # print every figure
uv run tools/output-chain/design_output_chain.py --check    # verify every bound; non-zero exit on failure
uv run tools/output-chain/design_output_chain.py --write    # regenerate output_coeffs.rs (byte-identical)
```

`--write` is deterministic: `git diff --exit-code crates/rr_dr60/src/stages/output_coeffs.rs`
after a run proves the committed file is the script's output. The script never runs in CI or at
build time.

## Changing a constant

Change the input at the top of the script (for instance `V_CLIP` if the codec's output level or a
capture from a real unit moves the ceiling), run `--check`, then `--write`, then update the pinned
literals in the core tests and the registers. A changed output is a model change: a CHANGELOG
entry and, after the output golden exists, a recorded re-bless (spec 009 FR-016).
