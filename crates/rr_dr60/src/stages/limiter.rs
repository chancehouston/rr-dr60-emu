//! Record-path limiter (signal-chain stage 3; spec 005). Implemented by T011; this file holds
//! the pinned reference figures (T003) that the unit tests of T006 assert against.
//!
//! Not yet declared in `stages/mod.rs` (T013 adds `pub(crate) mod limiter;`).

// Pinned to tools/agc-model/limiter_model.py, run 2026-10-11 (005 R-04; T003).
// The script runs the same equations as this module: a 192 kHz reference of the continuous
// loop and the emulator's discretized loop (R-03) at the seven host rates, with the emulator's
// conventions (positive polarity sensed, A-036; tones at the host-grid peak phase, R-08).
// Dead time = the first sample whose attenuation exceeds half the steady attenuation (FR-005).
//
//   knee (steady gain = -1 dB)                      +2.02 dBFS      (FR-004: +2 +-2)
//   ceiling (detector onset)                        +1.00 dBFS = 1.122 FS; ~30 % clipped duty
//   T_REF (beta6 300, knee +1, +10 dB over)          2.167 ms       (R-05 mapping reference)
//   default attack 3.0 ms -> beta6                   216.7
//   dead time, -20 -> +11 dBFS, 192 kHz reference    3.15 ms, ramp 0.09 ms, over-atten. -0.01 dB
//   dead time, emulator  6 kHz                        2.33 ms (14 samples), ramp 0.00 ms
//   dead time, emulator  8 kHz                        3.25 ms (26 samples), ramp 0.12 ms
//   dead time, emulator 16 kHz                        3.25 ms (52 samples)
//   dead time, emulator 44.1 kHz                      3.17 ms (140 samples)
//   dead time, emulator 48 kHz                        3.17 ms (152 samples), ramp 0.08 ms
//   dead time, emulator 88.2 / 96 kHz                 3.15 / 3.16 ms
//   +3 dBFS step (2 dB over), 6 kHz / 48 kHz         12.33 / 16.27 ms (steady -2.00 / -1.98 dB)
//   armed (2 s after a 0.2 s burst), 6 / 8 / 48 kHz    0.333 / 0.375 / 0.250 ms
//   release from +11 dBFS 1 s burst (10.0 dB), 48 kHz 1.362 s to -1 dB;
//     -9.19 dB @0.05 s, -7.21 @0.2, -6.13 @0.3, -4.39 @0.5, -2.87 @0.75, -1.87 @1.0
//   release from +18 dBFS (17.0 dB)                    1.69 s
//   release setting 200 / 1370 / 4000 / 10000 ms  -> 0.198 / 1.362 / 3.982 / 9.957 s
//   knee-aware mapping, attack 3 ms at 48 kHz:
//     knee +1 / -10 / -25 / -40 / +3 dBFS           -> 3.17 / 3.23 / 3.23 / 3.23 / 3.15 ms
//     I(knee) overdrive index                        0.1174 / 0.0947 / 0.0168 / 0.0030 / 0.0626 V
//   attack 1 / 10 / 50 ms at 48 kHz (6 kHz)          1.15 (1.33) / 10.25 (8.33) / 52.21 (41.33) ms
//   V_FLOOR (1 - gain = 2^-24)                       0.1714 V
//   limit 40 dB at Vc                                1.275 V
//   gain exactly 1.0 after a 1 s burst (6 kHz)       18.2 s; flush (Vc < 1e-6 V) ~121 s later
//   polarity: asymmetric stimulus, larger peak +4 dBFS: -2.96 dB (positive side) vs 0.00 dB
//   bandwidth: 6 kHz +6 dBFS + 1 kHz -20 dBFS @48 k  sum peak +6.41 dBFS -> expected drop 5.41 dB; simulated -5.39 dB
//   clipped onset, +11 dBFS, after the limiter        79 % in the first 2 ms; 29 % after 20 ms
