# Golden format: `golden-speed-v1.json` (spec 013 FR-013)

The common format of `golden.rs` (`rr_dr60-golden`, version 1), entries sorted by (stimulus, config, host_rate_hz), with four new optional fields, absent (and not serialized) in the eight earlier files, which therefore read unchanged:

| Field | Type | Meaning |
|---|---|---|
| `speed_level` | `u8` | the level set before `start_playback` (the schedule's first level for a schedule entry) |
| `speed_changes` | `[[u64, u8]]` | the schedule: `[play-call index, level]`, each set before that call (absent for a constant level) |
| `place` | `u64` | the last `PlayInfo::place` |
| `own_length` | `u64` | host samples produced before the drain (`produced` total minus `drain_samples()`) |

`frames` (existing) carries the frames decoded over the playback with the codec on (absent bypassed). The output hashed is every `play()` sample to `finished`, in 1024-sample calls (a schedule's call index counts these calls).

## Entries (194)

| Set | Configurations | Stimuli | Levels / schedules | Rates | Count |
|---|---|---|---|---|---|
| (a) default | `speed_default_speaker`, `speed_default_earphone` (0.13 default, reference volume) | `voiced_m20`, `tone_1k_m20` recorded | 1, 2, 4, 5 | 7 | 112 |
| (b) codec bypassed | `speed_no_codec_speaker` | `voiced_m20`, `tone_1k_m20` recorded | 1, 2, 4, 5 | 7 | 56 |
| (c) schedules | `speed_sched_a`, `speed_sched_b` (default, speaker) | `voiced_m20` recorded | A: 3 → 1 → 3 → 5 → 4 at calls 0, 4, 9, 13, 18; B: 2 → 5 → 1 at calls 0, 6, 11, then 3 after `place` reached the end | 7 | 14 |
| (d) speed only | `speed_only_l1` … `speed_only_l5` (no l3), `speed_only_sched_a`, `speed_only_sched_b` | `stored_tone_1k`, `stored_voiced` imported | 1, 2, 4, 5, A, B | 6 kHz | 12 |

The config name carries the level for (a), (b) and (d) (for example `speed_default_speaker_l4`).

## Bless-time assertions

- every constant-level entry's `own_length` equals `playback_length_samples(1, level) − drain_samples()`, and `place` equals the recording's length;
- no entry plays at level 3 throughout (the earlier files cover level 3);
- schedule B's last change (after the end) leaves the output identical to the same schedule without it (asserted, then dropped from the file: the entry records the full schedule);
- the speed-only entries' outputs are finite and below 1.0 in magnitude (imported −6 dBFS stimuli).

Bless on purpose only, with a CHANGELOG entry: `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed`. The SHA guard pins the file after the bless.
