/*
 * C smoke test for rr_dr60.h (001 tasks.md T038; spec 004 contracts/c-api.md, T029; spec 005
 * contracts/c-api.md › Smoke test additions, T032).
 * Build and run with crates/rr_dr60_ffi/tests/c/run_smoke.sh.
 */
#include "rr_dr60.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define FS 48000
#define PI 3.14159265358979323846 /* M_PI is not part of strict C11 */
#define N (FS / 2)
#define FS6 6000 /* the device rate (A-026): the rate conversion boundary is the identity */
#define N6 (FS6 / 10)

static int failures = 0;

#define CHECK(cond, ...)                                  \
  do {                                                    \
    if (!(cond)) {                                        \
      fprintf(stderr, "smoke: FAIL %s:%d: ", __FILE__, __LINE__); \
      fprintf(stderr, __VA_ARGS__);                       \
      fprintf(stderr, "\n");                              \
      failures++;                                         \
    }                                                     \
  } while (0)

static double rms(const float *x, size_t from, size_t to) {
  double acc = 0.0;
  for (size_t i = from; i < to; i++) acc += (double)x[i] * (double)x[i];
  return sqrt(acc / (double)(to - from));
}

int main(void) {
  static float in[N], out[N];
  for (size_t i = 0; i < N; i++) in[i] = (float)(0.1 * sin(2.0 * PI * 1000.0 * (double)i / FS));

  RrDr60Settings s = rr_dr60_settings_default(FS);
  CHECK(s.struct_size == sizeof(RrDr60Settings), "struct_size %u", s.struct_size);
  CHECK(s.struct_size == 80, "struct_size %u (must be 80 since 0.5, spec 005 contracts/c-api.md)", s.struct_size);
  CHECK(s.tap == RR_DR60_TAP_AFTER_PLAYBACK, "default tap");
  CHECK(s.agc_enabled, "limiter on by default (spec 005, A-020)");
  CHECK(s.agc_knee_dbfs == 1.0f, "default knee %g dBFS (A-028)", (double)s.agc_knee_dbfs);
  CHECK(s.agc_attack_ms == 3.0f, "default attack %g ms (A-034)", (double)s.agc_attack_ms);
  CHECK(s.agc_release_ms == 1370.0f, "default release %g ms (A-035)", (double)s.agc_release_ms);
  CHECK(s.reserved_agc_1 == 0.0f && s.reserved_agc_2 == 0.0f, "reserved AGC slots must default to 0");
  CHECK(s.adc_clamp_enabled, "ADC clamp on by default (spec 005 FR-008, A-030)");
  CHECK(s.vas_enabled && s.vas_mode == RR_DR60_VAS_MODE_DROP, "VAS on, drop mode by default (spec 003, A-008)");
  CHECK(s.vas_threshold_dbfs == -40.0f, "default VAS threshold %g dBFS (A-022 revised by spec 005)",
        (double)s.vas_threshold_dbfs);
  /* The spec 001 checks below run with the VAS bypassed (003 FR-020). The limiter stays on: below
   * its knee it is the identity (spec 005 FR-003), so the 1 kHz -20 dBFS level check sees only
   * the two codec templates. */
  s.vas_enabled = false;

  RrDr60Pipeline *p = NULL;
  CHECK(rr_dr60_create(&s, &p) == RR_DR60_STATUS_OK && p != NULL, "create");

  /* 1 kHz at -20 dBFS keeps its level within +/-0.2 dB of unity (001 US1 AS1, A-015) through the
   * default pipeline with the limiter on (spec 005 FR-003: unity below the knee). Spec 004 keeps
   * 1 kHz as the reference here: the two codec templates lose about 0.03 dB there combined, well
   * inside the tolerance (004 FR-009). */
  CHECK(rr_dr60_process(p, in, out, N, NULL) == RR_DR60_STATUS_OK, "process");
  double gain_db = 20.0 * log10(rms(out, N / 2, N) / rms(in, N / 2, N));
  CHECK(fabs(gain_db) <= 0.2, "1 kHz gain %.3f dB (must be within +/-0.2 dB of unity, A-015)", gain_db);

  uint32_t latency = 0;
  CHECK(rr_dr60_latency_samples(p, &latency) == RR_DR60_STATUS_OK, "latency");
  CHECK(latency > 0 && latency <= FS / 50, "latency %u samples (must be <= 20 ms, 004 FR-004)", latency);

  /* Spec 004 FR-002, FR-017 (contracts/c-api.md): at the 6 kHz device rate the rate conversion
   * boundary is the identity, so with every stage bypassed the output equals the input sample
   * for sample and the reported latency is 0. */
  static float in6[N6], out6[N6];
  for (size_t i = 0; i < N6; i++) in6[i] = (float)(0.25 * sin(2.0 * PI * 1000.0 * (double)i / FS6));
  RrDr60Settings s6 = rr_dr60_settings_default(FS6);
  s6.agc_enabled = false;
  s6.vas_enabled = false;
  s6.record_stage_enabled = false;
  s6.playback_stage_enabled = false;
  s6.adc_clamp_enabled = false; /* bypass_all bypasses the clamp too (spec 005 FR-008) */
  RrDr60Pipeline *p6 = NULL;
  CHECK(rr_dr60_create(&s6, &p6) == RR_DR60_STATUS_OK && p6 != NULL, "create at 6000 Hz");
  CHECK(rr_dr60_process(p6, in6, out6, N6, NULL) == RR_DR60_STATUS_OK, "process at 6000 Hz");
  CHECK(memcmp(in6, out6, sizeof in6) == 0, "6000 Hz with every stage bypassed is not the identity (004 FR-002)");
  uint32_t latency6 = 99;
  CHECK(rr_dr60_latency_samples(p6, &latency6) == RR_DR60_STATUS_OK && latency6 == 0,
        "6000 Hz bypassed latency %u samples (must be 0, 004 FR-002)", latency6);

  /* Spec 005 FR-001, FR-003 (contracts/c-api.md): the AGC-isolated configuration (tap after AGC,
   * every other stage off) returns a -20 dBFS tone bit-identical, at the host rate with latency 0:
   * below the knee the limiter is the identity. */
  RrDr60Settings agc_only = s6;
  agc_only.agc_enabled = true;
  agc_only.tap = RR_DR60_TAP_AFTER_AGC;
  CHECK(rr_dr60_reconfigure(p6, &agc_only) == RR_DR60_STATUS_OK, "reconfigure to agc_only at 6000 Hz");
  for (size_t i = 0; i < N6; i++) in6[i] = (float)(0.1 * sin(2.0 * PI * 1000.0 * (double)i / FS6 + PI / 6.0));
  CHECK(rr_dr60_process(p6, in6, out6, N6, NULL) == RR_DR60_STATUS_OK, "process agc_only at 6000 Hz");
  CHECK(memcmp(in6, out6, sizeof in6) == 0, "agc_only is not the identity below the knee (005 FR-003)");
  CHECK(rr_dr60_latency_samples(p6, &latency6) == RR_DR60_STATUS_OK && latency6 == 0,
        "after-AGC tap latency %u samples (must be 0, 005 FR-001)", latency6);

  /* Spec 005 FR-008 (A-030): a +11 dBFS tone (10 dB over the knee) through the record path at
   * the 6 kHz identity rate, tapped after the record stage and the clamp, peaks at exactly 1.0;
   * with the clamp bypassed it exceeds 1.0. */
  RrDr60Settings record_path = rr_dr60_settings_default(FS6);
  record_path.vas_enabled = false;
  record_path.tap = RR_DR60_TAP_AFTER_RECORD;
  CHECK(rr_dr60_reconfigure(p6, &record_path) == RR_DR60_STATUS_OK, "reconfigure to the record path at 6000 Hz");
  static float loud6[FS6], loud_out6[FS6];
  for (size_t i = 0; i < FS6; i++)
    loud6[i] = (float)(3.5481 * sin(2.0 * PI * 1000.0 * (double)i / FS6 + PI / 6.0)); /* +11 dBFS, peak phase */
  CHECK(rr_dr60_process(p6, loud6, loud_out6, FS6, NULL) == RR_DR60_STATUS_OK, "process +11 dBFS at 6000 Hz");
  float peak6 = 0.0f;
  for (size_t i = 0; i < FS6; i++) peak6 = fabsf(loud_out6[i]) > peak6 ? fabsf(loud_out6[i]) : peak6;
  CHECK(peak6 == 1.0f, "clamped record-path peak %g (must be exactly 1.0, 005 FR-008)", (double)peak6);
  record_path.adc_clamp_enabled = false;
  CHECK(rr_dr60_reconfigure(p6, &record_path) == RR_DR60_STATUS_OK, "reconfigure with the clamp off");
  CHECK(rr_dr60_process(p6, loud6, loud_out6, FS6, NULL) == RR_DR60_STATUS_OK, "process +11 dBFS, clamp off");
  peak6 = 0.0f;
  for (size_t i = 0; i < FS6; i++) peak6 = fabsf(loud_out6[i]) > peak6 ? fabsf(loud_out6[i]) : peak6;
  CHECK(peak6 > 1.0f, "un-clamped record-path peak %g (must exceed 1.0, 005 FR-008)", (double)peak6);
  rr_dr60_destroy(p6);

  /* Error table from contracts/c-api.md (rows that do not need reconfigure). */
  RrDr60Pipeline *sentinel = (RrDr60Pipeline *)(uintptr_t)0x1;
  RrDr60Pipeline *q = sentinel;
  CHECK(rr_dr60_create(NULL, &q) == RR_DR60_STATUS_NULL_POINTER && q == sentinel, "create(NULL)");
  RrDr60Settings bad = rr_dr60_settings_default(22050);
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE && q == sentinel, "22050 Hz");
  /* Spec 004 FR-002: 7000 Hz is not one of the seven supported rates; the pipeline stays NULL. */
  bad = rr_dr60_settings_default(7000);
  q = NULL;
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE && q == NULL, "7000 Hz");
  q = sentinel;
  bad = rr_dr60_settings_default(FS);
  bad.tap = 7;
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_INVALID_ARGUMENT && q == sentinel, "tap = 7");
  bad = rr_dr60_settings_default(FS);
  bad.struct_size = 4;
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_INVALID_ARGUMENT && q == sentinel, "struct_size = 4");
  CHECK(rr_dr60_process(NULL, in, out, 64, NULL) == RR_DR60_STATUS_NULL_POINTER, "process(NULL handle)");
  CHECK(rr_dr60_process(p, NULL, out, 64, NULL) == RR_DR60_STATUS_NULL_POINTER, "process(NULL input)");
  CHECK(rr_dr60_process(p, NULL, NULL, 0, NULL) == RR_DR60_STATUS_OK, "frames = 0 with NULLs");
  CHECK(rr_dr60_process(p, in, in + 1, 64, NULL) == RR_DR60_STATUS_INVALID_ARGUMENT, "partial overlap");

  /* In place equals copy mode after reset. */
  static float buf[N];
  memcpy(buf, in, sizeof in);
  CHECK(rr_dr60_reset(p) == RR_DR60_STATUS_OK, "reset");
  CHECK(rr_dr60_process(p, buf, buf, N, NULL) == RR_DR60_STATUS_OK, "in place");
  CHECK(memcmp(buf, out, sizeof out) == 0, "in-place output differs from copy mode");

  /* Reconfigure rows (contracts/c-api.md): errors leave the old configuration working. */
  bad = rr_dr60_settings_default(22050);
  CHECK(rr_dr60_reconfigure(p, &bad) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE, "reconfigure 22050 Hz");
  uint32_t latency_after = 0;
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after == latency,
        "failed reconfigure changed latency (%u -> %u)", latency, latency_after);
  CHECK(rr_dr60_process(p, in, out, 64, NULL) == RR_DR60_STATUS_OK, "process after failed reconfigure");
  CHECK(rr_dr60_reconfigure(p, NULL) == RR_DR60_STATUS_NULL_POINTER, "reconfigure(NULL)");
  RrDr60Settings bypass = rr_dr60_settings_default(FS);
  bypass.agc_enabled = false;
  bypass.vas_enabled = false;
  bypass.record_stage_enabled = false;
  bypass.playback_stage_enabled = false;
  CHECK(rr_dr60_reconfigure(p, &bypass) == RR_DR60_STATUS_OK, "reconfigure to bypass_all");
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after < latency,
        "bypass_all latency %u should be below default %u", latency_after, latency);

  /* Spec 005 limiter settings (FR-010, FR-015; contracts/c-api.md). */
  RrDr60Settings agc = rr_dr60_settings_default(FS);
  agc.vas_enabled = false; /* the spec 005 `default_agc` configuration (spec 003 FR-020) */
  agc.agc_release_ms = 3000.0f;
  agc.tap = RR_DR60_TAP_AFTER_AGC;
  RrDr60SettingField field = RR_DR60_SETTING_FIELD_TAP;
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_OK &&
            field == RR_DR60_SETTING_FIELD_NONE,
        "validate(release 3000 ms, tap after AGC)");
  CHECK(rr_dr60_reconfigure(p, &agc) == RR_DR60_STATUS_OK, "reconfigure to limiter settings");
  CHECK(rr_dr60_process(p, in, out, N, NULL) == RR_DR60_STATUS_OK, "process with the limiter");
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after == 0,
        "after-AGC tap latency %u at 48 kHz (must be 0, 005 FR-001)", latency_after);
  agc.agc_attack_ms = 0.0f;
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_INVALID_SETTING &&
            field == RR_DR60_SETTING_FIELD_AGC_ATTACK_MS,
        "validate(attack 0 ms) names the field");
  q = sentinel;
  CHECK(rr_dr60_create(&agc, &q) == RR_DR60_STATUS_INVALID_SETTING && q == sentinel,
        "create(attack 0 ms)");
  agc = rr_dr60_settings_default(FS);
  agc.agc_knee_dbfs = 4.0f; /* above the +3 dBFS cap (005 FR-010) */
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_INVALID_SETTING &&
            field == RR_DR60_SETTING_FIELD_AGC_KNEE_DBFS,
        "validate(knee 4 dBFS) names the field");
  q = sentinel;
  CHECK(rr_dr60_create(&agc, &q) == RR_DR60_STATUS_INVALID_SETTING && q == sentinel, "create(knee 4 dBFS)");
  /* A struct from the 0.4 layout (72 bytes): rejected as STRUCT_SIZE after reading only its
   * first field (spec 005 contracts/c-api.md). */
  agc = rr_dr60_settings_default(FS);
  agc.struct_size = 72;
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_INVALID_ARGUMENT &&
            field == RR_DR60_SETTING_FIELD_STRUCT_SIZE,
        "validate(struct_size = 72) names STRUCT_SIZE");
  q = sentinel;
  CHECK(rr_dr60_create(&agc, &q) == RR_DR60_STATUS_INVALID_ARGUMENT && q == sentinel, "create(struct_size = 72)");

  /* Spec 003 VAS (US2 AS7; contracts/c-api.md rule 8): drop mode, events, mute mode, the
   * "after VAS" tap, and an invalid sensitivity. VAS-isolated configuration: the limiter, both
   * band-limit stages and the clamp off. Burst 1 s, gap 5 s, burst 1 s, 1 kHz at -30 dBFS
   * (threshold + 10 dB; the threshold is -40 dBFS since spec 005, A-022 revised). */
  RrDr60Settings vas = rr_dr60_settings_default(FS);
  vas.agc_enabled = false;
  vas.record_stage_enabled = false;
  vas.playback_stage_enabled = false;
  vas.adc_clamp_enabled = false;
  vas.tap = RR_DR60_TAP_AFTER_VAS;
  CHECK(rr_dr60_settings_validate(&vas, &field) == RR_DR60_STATUS_OK && field == RR_DR60_SETTING_FIELD_NONE,
        "validate(tap after VAS)");
  CHECK(rr_dr60_reconfigure(p, &vas) == RR_DR60_STATUS_OK, "reconfigure to the VAS-isolated settings");
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after < latency,
        "tap after VAS skips stage 10: latency %u should be below default %u", latency_after, latency);
  static float vin[7 * FS], vout[7 * FS];
  for (size_t i = 0; i < 7 * FS; i++) {
    int burst = i < (size_t)FS || i >= (size_t)(6 * FS);
    vin[i] = burst ? (float)(0.0316 * sin(2.0 * PI * 1000.0 * (double)(i % FS) / FS)) : 0.0f; /* -30 dBFS */
  }
  size_t capacity = 0;
  CHECK(rr_dr60_max_events(p, 7 * FS, &capacity) == RR_DR60_STATUS_OK && capacity >= 1, "max_events");
  RrDr60VasEvent *events = (RrDr60VasEvent *)malloc(capacity * sizeof *events);
  CHECK(events != NULL, "malloc");
  RrDr60BlockInfo info = {0, 0, false};
  CHECK(rr_dr60_process_with_events(p, vin, vout, 7 * FS, &info, events, capacity) == RR_DR60_STATUS_OK,
        "process_with_events");
  /* Kept: burst + hang time (1 s) + burst minus onset (20 ms), minus the end-of-stream latency. */
  CHECK(info.produced < 7 * (size_t)FS && info.produced > 2 * (size_t)FS,
        "drop mode produced %zu of %d", info.produced, 7 * FS);
  CHECK(info.events == 1 && !info.paused, "one splice (events %zu, paused %d)", info.events, (int)info.paused);
  CHECK(info.events <= capacity && events[0].output_position < info.produced &&
            events[0].input_length + info.produced <= 7 * (uint64_t)FS,
        "splice at %llu removed %llu", (unsigned long long)events[0].output_position,
        (unsigned long long)events[0].input_length);
  free(events);

  vas.vas_mode = RR_DR60_VAS_MODE_MUTE;
  CHECK(rr_dr60_reconfigure(p, &vas) == RR_DR60_STATUS_OK, "reconfigure to mute mode");
  CHECK(rr_dr60_process(p, vin, vout, 7 * FS, &info) == RR_DR60_STATUS_OK, "process in mute mode");
  CHECK(info.produced == 7 * (size_t)FS && info.events == 1, "mute mode produced %zu, events %zu",
        info.produced, info.events);

  vas.vas_sensitivity = 259; /* must not wrap to a valid level */
  CHECK(rr_dr60_settings_validate(&vas, &field) == RR_DR60_STATUS_INVALID_SETTING &&
            field == RR_DR60_SETTING_FIELD_VAS_SENSITIVITY,
        "validate(vas_sensitivity = 259) names the field");

  CHECK(rr_dr60_version_string() != NULL && strlen(rr_dr60_version_string()) > 0, "version");
  rr_dr60_destroy(p);
  rr_dr60_destroy(NULL);

  if (failures) {
    fprintf(stderr, "smoke: %d failure(s)\n", failures);
    return 1;
  }
  printf("smoke: OK (rr_dr60 %s, latency %u samples at %d Hz)\n", rr_dr60_version_string(), latency, FS);
  return 0;
}
