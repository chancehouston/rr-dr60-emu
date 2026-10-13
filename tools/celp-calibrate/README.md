# celp-calibrate

Sets the tolerances of the speech codec's transparency and noise checks (spec 008 FR-005, FR-006) against a **documented reference coder of the same class and bit density**: ITU-T G.723.1 at 5.3 kbit/s (S-008; 0.667 bit per sample, as the model's 4 kbit/s at 6 kHz). Plan time only: the tool never runs in CI, at build time or at run time, and nothing it downloads enters the repository.

Why not ffmpeg's encoder: it refuses the 5.3 kbit/s mode (`libavcodec/g723_1enc.c`: "Use bitrate 6300 instead of 5300"; checked in the source on 2026-10-12), and its 6.3 kbit/s mode is a different excitation class (MP-MLQ) at a higher density. ffmpeg's G.723.1 *decoder* does accept 5.3 kbit/s streams and is used as an optional cross-check of the reference decode.

## Steps

1. Download the G.723.1 reference software: the free electronic attachment ("Software") to Recommendation ITU-T G.723.1 (05/2006) at <https://www.itu.int/rec/T-REC-G.723.1>. Unzip it under `tools/celp-calibrate/ref/` (git-ignored). The ITU's software licence allows use for evaluation; keep the files out of git.
2. Write the stimuli at 8 kHz (the reference coder's rate), 16-bit little-endian, from the harness (the stimuli are the harness's own `voiced` and `unvoiced_voiced` at −10, −20 and −40 dBFS, seed 0x0D60; the example prints each file's SHA-256, which `calibration.md` records and a test asserts):

   ```sh
   cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out tools/celp-calibrate/out
   ```

3. Build the reference coder and run it on every stimulus (`-r53` is the 5.3 kbit/s mode); the decoded references land beside the inputs as `<name>.g723.raw`:

   ```sh
   uv run tools/celp-calibrate/calibrate.py tools/celp-calibrate/out
   ```

4. Measure reference and model with the same metric code and read the figures:

   ```sh
   cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture
   ```

5. Record the figures and the margins in `specs/008-celp-codec/calibration.md`; the tolerances in `crates/rr_dr60_harness/src/codec_checks.rs` are those figures plus the stated margins, never looser than the spec's floors (±1.5 dB RMS, ±3 dB third-octave, 4 dB segmental SNR).

Without the reference software or a C compiler the tool exits non-zero with these instructions; the committed `calibration.md` stands and the spec's floors remain the minimum the model must meet.
