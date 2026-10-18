# Golden format: `golden-speed-v1.json` (spec 013 FR-013)

The common format of `golden.rs` (`rr_dr60-golden`, version 1), entries sorted by (stimulus, config, host_rate_hz), with four new optional fields, absent (and not serialized) in the eight earlier files, which therefore read unchanged:

| Field | Type | Meaning |
|---|---|---|
| `speed_level` | `u8` | the level set before `start_playback` (the schedule's first level for a schedule entry) |
| `speed_changes` | `[[u64, u8]]` | the schedule: `[play-call index, level]`, each set before that call (absent for a constant level) |
| `place` | `u64` | the last `PlayInfo::place` |
| `own_length` | `u64` | host samples produced before the drain: the recording's own part and the speed control's tail (`produced` total minus `drain_samples()`) |

`frames` (existing) carries the frames decoded over the playback with the codec on (absent bypassed); `seed` and `counter` (existing, spec 012) are present for the entries recorded through the default record path. The output hashed is every `play()` sample to `finished`, in calls of **rate ÷ 50 host samples** (20 ms, 120 device samples at every rate; a schedule's call index counts these calls). Each recording is made once per (stimulus, rate, codec) and replayed at every level and route.

## Entries (196)

| Set | Configurations | Stimuli | Levels / schedules | Rates | Count |
|---|---|---|---|---|---|
| (a) default | `speed_default_speaker_l{1,2,4,5}`, `speed_default_earphone_l{1,2,4,5}` (0.13 default, reference volume) | `voiced_m20`, `tone_1k_m20` recorded | 1, 2, 4, 5 | 7 | 112 |
| (b) codec bypassed | `speed_no_codec_speaker_l{1,2,4,5}` (the route is downstream of the speed control, so the speaker route alone) | `voiced_m20`, `tone_1k_m20` recorded | 1, 2, 4, 5 | 7 | 56 |
| (c) schedules | `speed_sched_a`, `speed_sched_b` (default, speaker) | `voiced_m20` recorded | A: 3 → 1 → 3 → 5 → 4 at calls 0, 20, 45, 65, 90; B: 2 → 5 → 1 at calls 0, 30, 55, then 3 after the place reached the end | 7 | 14 |
| (d) speed only | `speed_only_l{1,2,4,5}`, `speed_only_sched_a`, `speed_only_sched_b`, `speed_only_sched_c` (codec, stage 10 and output chain bypassed) | `stored_tone_1k` (1 kHz, −6 dBFS, 2 s), `stored_voiced` (`voiced_m20` at 6 kHz, 2 s), imported | 1, 2, 4, 5, A, B, and C: 1 → 3 → 5 → 3 at calls 0, 1, 3, 4 in calls of 121 (a deferred switch into level 3) | 6 kHz | 14 |

## Bless-time assertions

- every constant-level entry's `own_length` equals `playback_length_samples(1, level) − drain_samples()`, and `place` equals the recording's length;
- every scheduled change lands before the call in which the place first reaches K (post-implementation review B1);
- schedule B's last change (after the place reached the end) leaves the output identical to the same schedule without it (the bless runs both; the entry records the full schedule);
- the speed-only entries' outputs equal the harness's independent reference (research R-08) bit for bit;
- no entry plays at level 3 throughout (the eight earlier files guard level 3).

Bless on purpose only, with a CHANGELOG entry: `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed`. The SHA guard pins the file after the bless.
