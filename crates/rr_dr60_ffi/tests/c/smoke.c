/*
 * C smoke test for rr_dr60.h (tasks.md T038; contracts/c-api.md).
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
  CHECK(s.tap == RR_DR60_TAP_AFTER_PLAYBACK, "default tap");
  CHECK(s.agc_enabled, "AGC on by default (spec 002, A-020)");
  CHECK(s.vas_enabled && s.vas_mode == RR_DR60_VAS_MODE_DROP, "VAS on, drop mode by default (spec 003, A-008)");
  /* The spec 001 checks below run with the AGC and the VAS bypassed (spec 002 FR-018, 003 FR-020). */
  s.agc_enabled = false;
  s.vas_enabled = false;

  RrDr60Pipeline *p = NULL;
  CHECK(rr_dr60_create(&s, &p) == RR_DR60_STATUS_OK && p != NULL, "create");

  /* 1 kHz at -20 dBFS keeps its level within 0.2 dB (US1 AS1, A-015). */
  CHECK(rr_dr60_process(p, in, out, N, NULL) == RR_DR60_STATUS_OK, "process");
  double gain_db = 20.0 * log10(rms(out, N / 2, N) / rms(in, N / 2, N));
  CHECK(fabs(gain_db) <= 0.2, "1 kHz gain %.3f dB", gain_db);

  uint32_t latency = 0;
  CHECK(rr_dr60_latency_samples(p, &latency) == RR_DR60_STATUS_OK, "latency");
  CHECK(latency > 0 && latency <= FS / 50, "latency %u samples (must be <= 20 ms, FR-013)", latency);

  /* Error table from contracts/c-api.md (rows that do not need reconfigure). */
  RrDr60Pipeline *sentinel = (RrDr60Pipeline *)(uintptr_t)0x1;
  RrDr60Pipeline *q = sentinel;
  CHECK(rr_dr60_create(NULL, &q) == RR_DR60_STATUS_NULL_POINTER && q == sentinel, "create(NULL)");
  RrDr60Settings bad = rr_dr60_settings_default(22050);
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE && q == sentinel, "22050 Hz");
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

  /* Spec 002 AGC (US2 AS5, AS6; contracts/c-api.md). */
  RrDr60Settings agc = rr_dr60_settings_default(FS);
  agc.vas_enabled = false; /* the spec 002 default (spec 003 FR-020) */
  agc.agc_release_ms = 3000.0f;
  agc.tap = RR_DR60_TAP_AFTER_AGC;
  RrDr60SettingField field = RR_DR60_SETTING_FIELD_TAP;
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_OK &&
            field == RR_DR60_SETTING_FIELD_NONE,
        "validate(AGC release 3000 ms, tap after AGC)");
  CHECK(rr_dr60_reconfigure(p, &agc) == RR_DR60_STATUS_OK, "reconfigure to AGC settings");
  CHECK(rr_dr60_process(p, in, out, N, NULL) == RR_DR60_STATUS_OK, "process with AGC");
  agc.agc_attack_ms = 0.0f;
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_INVALID_SETTING &&
            field == RR_DR60_SETTING_FIELD_AGC_ATTACK_MS,
        "validate(attack 0 ms) names the field");
  q = sentinel;
  CHECK(rr_dr60_create(&agc, &q) == RR_DR60_STATUS_INVALID_SETTING && q == sentinel,
        "create(attack 0 ms)");

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
