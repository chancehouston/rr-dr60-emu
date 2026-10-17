/*
 * C smoke test for rr_dr60.h (001 tasks.md T038; spec 004 contracts/c-api.md, T029; spec 005
 * contracts/c-api.md › Smoke test additions, T032; spec 007 contracts/c-api.md, T028; spec 008
 * contracts/c-api.md › Smoke test, T042: the loop runs through the speech codec).
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

/* Spec 012 FR-010 (T016): the 0.11 default, the library's default with the microphone path
 * (stages 1 - 2) bypassed. Every pre-012 case builds on it, so its pinned hash stays the 0.11
 * output; the 012 cases call rr_dr60_settings_default directly. */
static RrDr60Settings settings_011(uint32_t host_rate_hz) {
  RrDr60Settings s = rr_dr60_settings_default(host_rate_hz);
  s.mic_capsule_enabled = false;
  s.mic_amp_enabled = false;
  return s;
}
#define N6 (FS6 / 10)
/* The Rust path's reference for the smoke stimulus with the 0.8 default settings (the speech codec
 * on), pinned by the harness's `ffi_parity::smoke_reference_hash_is_pinned` (spec 006 and 008 US4
 * scenario 1; `smoke_reference.rs`): FNV-1a 64 of the playback (50404 samples) and of the
 * recording's frame bytes (51 frames), the frame count and the stop's silence fed. */
#define RR_DR60_SMOKE_REFERENCE_HASH 0x971c70e3428c23c7ULL
#define RR_DR60_SMOKE_PLAYBACK_SAMPLES 50404
/* Spec 009 T033: the four route cases, pinned from the Rust path (smoke_reference.rs). */
#define RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES 53168
#define RR_DR60_SMOKE_SPEAKER_REF_HASH 0xe414787b33951ea4ULL
#define RR_DR60_SMOKE_SPEAKER_FULL_HASH 0xbf5f8794084b0d08ULL
#define RR_DR60_SMOKE_EARPHONE_REF_HASH 0xe7a0621c54ea1b7aULL
#define RR_DR60_SMOKE_EARPHONE_FULL_HASH 0x4a93f66893e304fbULL
#define RR_DR60_SMOKE_FRAMES_HASH 0x54035d54bd48af25ULL
/* Spec 010 T031: the two output-clamp cases (a full-scale square, the option on, position 1),
   pinned from the Rust path (smoke_reference.rs). */
#define RR_DR60_SMOKE_SPEAKER_CLAMP_HASH 0x5a7f9b91a49f806eULL
#define RR_DR60_SMOKE_EARPHONE_CLAMP_HASH 0xfbafbb6b11507362ULL
/* Spec 011 T035: the volume-transition case (the square at 0.1 on the speaker route, 1.0 before
   play block 10, 0.4 before block 11), pinned from the Rust path (smoke_reference.rs). */
#define RR_DR60_SMOKE_SPEAKER_RAMP_HASH 0xcc0133ffc64ac96eULL
/* Spec 012 T048 (contracts/c-api.md › Smoke loop): the microphone cases, pinned from the Rust
   path (smoke_reference.rs): the 0.12 default one-pass on the square plus silence, and two
   recordings of silence (the hiss) with a reset between them. */
#define RR_DR60_SMOKE_MIC_DEFAULT_HASH 0x863aa823f42bc8ceULL
#define RR_DR60_SMOKE_MIC_DEFAULT_SAMPLES 72000
#define RR_DR60_SMOKE_MIC_REC0_HASH 0x0bd8b3aa3dd4132fULL
#define RR_DR60_SMOKE_MIC_REC1_HASH 0x2c9342b7e52bca17ULL
#define RR_DR60_SMOKE_MIC_REC_SAMPLES 53168
#define RR_DR60_SMOKE_FRAMES 51
#define RR_DR60_SMOKE_SILENCE_FED 149

static int failures = 0;

/* FNV-1a 64 over the little-endian bytes of n floats (the harness's smoke_reference::fnv1a64). */
static uint64_t fnv_floats(const float *x, size_t n) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (size_t i = 0; i < n; i++) {
    uint8_t le[4];
    memcpy(le, &x[i], 4);
    for (int b = 0; b < 4; b++) {
      h ^= le[b];
      h *= 0x00000100000001b3ULL;
    }
  }
  return h;
}

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

/* manifest-example-c:start */
/* The stage manifest (spec 007): the fifteen signal-chain stages and what the engine says
 * about each, printed in the canonical text form. Needs no handle; every string pointer is
 * static for the life of the process. */
static const char *status_word(uint32_t s) {
  switch (s) {
    case RR_DR60_STAGE_STATUS_NOT_MODELED: return "not_modeled";
    case RR_DR60_STAGE_STATUS_ASSUMED: return "assumed";
    case RR_DR60_STAGE_STATUS_SOURCED: return "sourced";
    case RR_DR60_STAGE_STATUS_CHECKED: return "checked";
    default: return "unknown";
  }
}

static const char *path_word(uint32_t p) {
  switch (p) {
    case RR_DR60_STAGE_PATH_RECORD: return "record";
    case RR_DR60_STAGE_PATH_PLAYBACK: return "playback";
    case RR_DR60_STAGE_PATH_BOTH: return "both";
    default: return "unknown";
  }
}

static const char *confidence_word(uint32_t c) {
  switch (c) {
    case RR_DR60_CONFIDENCE_NOT_APPLICABLE: return "not_applicable";
    case RR_DR60_CONFIDENCE_LOW: return "low";
    case RR_DR60_CONFIDENCE_MEDIUM: return "medium";
    case RR_DR60_CONFIDENCE_HIGH: return "high";
    default: return "unknown";
  }
}

/* Prints "key: value", or "key:" with nothing after the colon when the value is empty. */
static void manifest_line(const char *key, const char *value) {
  if (value[0]) printf("%s: %s\n", key, value);
  else printf("%s:\n", key);
}

static void print_manifest(void) {
  RrDr60ManifestHeader h;
  if (rr_dr60_manifest_header(&h) != RR_DR60_STATUS_OK) return;
  printf("rr_dr60 manifest\n");
  manifest_line("library_version", h.library_version);
  printf("device_rate_hz: %u\n", h.device_rate_hz);
  printf("stage_count: %u\n", h.stage_count);
  printf("counts: not_modeled=%u assumed=%u sourced=%u checked=%u\n", h.count_not_modeled, h.count_assumed,
         h.count_sourced, h.count_checked);
  manifest_line("basis", h.basis);
  manifest_line("evidence_link", h.evidence_link);
  manifest_line("assumptions_link", h.assumptions_link);
  manifest_line("sources_link", h.sources_link);
  for (uint32_t n = 1; n <= rr_dr60_manifest_stage_count(); n++) {
    RrDr60StageEntry e;
    if (rr_dr60_manifest_stage(n, &e) != RR_DR60_STATUS_OK) return;
    printf("\nstage %u %s\n", e.number, e.identifier);
    manifest_line("name", e.name);
    manifest_line("description", e.description);
    manifest_line("status", status_word(e.status));
    manifest_line("path", path_word(e.path));
    manifest_line("confidence", confidence_word(e.confidence));
    printf("sources:");
    for (uint32_t i = 0; i < e.source_count; i++) {
      const char *id = NULL;
      if (rr_dr60_manifest_stage_source(n, i, &id) == RR_DR60_STATUS_OK) printf("%s%s", i ? ", " : " ", id);
    }
    printf("\nassumptions:");
    for (uint32_t i = 0; i < e.assumption_count; i++) {
      const char *id = NULL;
      if (rr_dr60_manifest_stage_assumption(n, i, &id) == RR_DR60_STATUS_OK) printf("%s%s", i ? ", " : " ", id);
    }
    printf("\n");
    manifest_line("coverage", e.coverage);
    manifest_line("controls", e.controls);
    manifest_line("model_version", e.model_version);
  }
}
/* manifest-example-c:end */

int main(int argc, char **argv) {
  if (argc > 1 && strcmp(argv[1], "--manifest") == 0) {
    print_manifest();
    return 0;
  }
  static float in[N], out[N];
  for (size_t i = 0; i < N; i++) in[i] = (float)(0.1 * sin(2.0 * PI * 1000.0 * (double)i / FS));

  RrDr60Settings s = settings_011(FS);
  CHECK(s.struct_size == sizeof(RrDr60Settings), "struct_size %u", s.struct_size);
  CHECK(s.struct_size == 120, "struct_size %u (must be 120 since 0.12, spec 012 contracts/c-api.md)", s.struct_size);
  CHECK(s.output_volume_amp_enabled && s.output_speaker_enabled && s.output_earphone_enabled,
        "output chain on by default (spec 009 FR-002; A-031, A-010, A-032)");
  CHECK(s.speech_codec_enabled, "speech codec on by default (spec 008 FR-002, A-004)");
  CHECK(s.reserved_008 == 0, "reserved_008 must default to 0");
  CHECK(s.memory_capacity_s == 3600, "default memory capacity %u s (S-001: 60 minutes)", s.memory_capacity_s);
  CHECK(s.memory_max_recordings == 99, "default recording count %u (S-001: 99 items)", s.memory_max_recordings);
  CHECK(s.tap == RR_DR60_TAP_ROUTE_OUTPUT, "default tap (spec 009 FR-010)");
  /* The 0.8 case below keeps 0.8's output: the output chain bypassed and the tap after stage 10
     (spec 009 FR-016); 009 T031 adds the route cases. */
  s.output_volume_amp_enabled = false;
  s.output_speaker_enabled = false;
  s.output_earphone_enabled = false;
  s.tap = RR_DR60_TAP_AFTER_PLAYBACK;
  CHECK(s.agc_enabled, "limiter on by default (spec 005, A-020)");
  CHECK(s.agc_knee_dbfs == -1.0f, "default knee %g dBFS (A-028)", (double)s.agc_knee_dbfs);
  CHECK(s.agc_attack_ms == 3.0f, "default attack %g ms (A-034)", (double)s.agc_attack_ms);
  CHECK(s.agc_release_ms == 1370.0f, "default release %g ms (A-035)", (double)s.agc_release_ms);
  CHECK(s.reserved_agc_1 == 0.0f && s.reserved_agc_2 == 0.0f, "reserved AGC slots must default to 0");
  CHECK(s.adc_clamp_enabled, "ADC clamp on by default (spec 005 FR-008, A-030)");
  CHECK(s.vas_enabled && s.vas_mode == RR_DR60_VAS_MODE_DROP, "VAS on, drop mode by default (spec 003, A-008)");
  CHECK(s.vas_threshold_dbfs == -40.0f, "default VAS threshold %g dBFS (A-022 revised by spec 005)",
        (double)s.vas_threshold_dbfs);
  /* The spec 001 checks below run with the VAS bypassed (003 FR-020) and the speech codec bypassed
   * (008 FR-021: the earlier specs' checks keep their meaning). The limiter stays on: below its
   * knee it is the identity (spec 005 FR-003), so the 1 kHz -20 dBFS level check sees only the
   * two codec templates. */
  s.vas_enabled = false;
  s.speech_codec_enabled = false;

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
  /* The 006 figure with the codec bypassed (004 FR-004: under 20 ms); the codec-on default's 1923
   * (008 FR-004) is checked on the default handle below. */
  CHECK(latency == 483, "latency %u samples (must be 483 with the codec bypassed, 006)", latency);
  uint32_t latency_default = 0;

  /* Spec 004 FR-002, FR-017 (contracts/c-api.md): at the 6 kHz device rate the rate conversion
   * boundary is the identity, so with every stage bypassed the output equals the input sample
   * for sample and the reported latency is 0. */
  static float in6[N6], out6[N6];
  for (size_t i = 0; i < N6; i++) in6[i] = (float)(0.25 * sin(2.0 * PI * 1000.0 * (double)i / FS6));
  RrDr60Settings s6 = settings_011(FS6);
  s6.agc_enabled = false;
  s6.vas_enabled = false;
  s6.record_stage_enabled = false;
  s6.playback_stage_enabled = false;
  s6.adc_clamp_enabled = false; /* bypass_all bypasses the clamp too (spec 005 FR-008) */
  s6.speech_codec_enabled = false; /* and the speech codec (spec 008 FR-021) */
  s6.output_volume_amp_enabled = false; /* and the output chain (spec 009 FR-016) */
  s6.output_speaker_enabled = false;
  s6.output_earphone_enabled = false;
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

  /* Spec 005 FR-008 (A-030): a +11 dBFS tone (12 dB over the knee) through the record path at
   * the 6 kHz identity rate, tapped after the record stage and the clamp, peaks at exactly 1.0
   * (the clipped onset; the limiter then holds it below full scale); with the clamp bypassed the
   * onset exceeds 1.0. */
  RrDr60Settings record_path = settings_011(FS6);
  record_path.vas_enabled = false;
  record_path.speech_codec_enabled = false; /* spec 005's check (008 FR-021) */
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
  RrDr60Settings bad = settings_011(22050);
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE && q == sentinel, "22050 Hz");
  /* Spec 004 FR-002: 7000 Hz is not one of the seven supported rates; the pipeline stays NULL. */
  bad = settings_011(7000);
  q = NULL;
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE && q == NULL, "7000 Hz");
  q = sentinel;
  bad = settings_011(FS);
  bad.tap = 11; /* 0 – 10 are taps since 0.12 (spec 012 contracts/c-api.md) */
  CHECK(rr_dr60_create(&bad, &q) == RR_DR60_STATUS_INVALID_ARGUMENT && q == sentinel, "tap = 11");
  bad = settings_011(FS);
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
  bad = settings_011(22050);
  CHECK(rr_dr60_reconfigure(p, &bad) == RR_DR60_STATUS_UNSUPPORTED_HOST_RATE, "reconfigure 22050 Hz");
  uint32_t latency_after = 0;
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after == latency,
        "failed reconfigure changed latency (%u -> %u)", latency, latency_after);
  CHECK(rr_dr60_process(p, in, out, 64, NULL) == RR_DR60_STATUS_OK, "process after failed reconfigure");
  CHECK(rr_dr60_reconfigure(p, NULL) == RR_DR60_STATUS_NULL_POINTER, "reconfigure(NULL)");
  RrDr60Settings bypass = settings_011(FS);
  bypass.agc_enabled = false;
  bypass.vas_enabled = false;
  bypass.record_stage_enabled = false;
  bypass.playback_stage_enabled = false;
  bypass.speech_codec_enabled = false;
  bypass.output_volume_amp_enabled = false; /* spec 009 FR-016 */
  bypass.output_speaker_enabled = false;
  bypass.output_earphone_enabled = false;
  CHECK(rr_dr60_reconfigure(p, &bypass) == RR_DR60_STATUS_OK, "reconfigure to bypass_all");
  CHECK(rr_dr60_latency_samples(p, &latency_after) == RR_DR60_STATUS_OK && latency_after < latency,
        "bypass_all latency %u should be below default %u", latency_after, latency);

  /* Spec 005 limiter settings (FR-010, FR-015; contracts/c-api.md). */
  RrDr60Settings agc = settings_011(FS);
  agc.vas_enabled = false; /* the spec 005 `default_agc` configuration (spec 003 FR-020) */
  agc.speech_codec_enabled = false; /* spec 008 FR-021 */
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
  agc = settings_011(FS);
  agc.agc_knee_dbfs = 4.0f; /* above the +3 dBFS cap (005 FR-010) */
  CHECK(rr_dr60_settings_validate(&agc, &field) == RR_DR60_STATUS_INVALID_SETTING &&
            field == RR_DR60_SETTING_FIELD_AGC_KNEE_DBFS,
        "validate(knee 4 dBFS) names the field");
  q = sentinel;
  CHECK(rr_dr60_create(&agc, &q) == RR_DR60_STATUS_INVALID_SETTING && q == sentinel, "create(knee 4 dBFS)");
  /* A struct from the 0.4 layout (72 bytes): rejected as STRUCT_SIZE after reading only its
   * first field (spec 005 contracts/c-api.md). */
  agc = settings_011(FS);
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
  RrDr60Settings vas = settings_011(FS);
  vas.agc_enabled = false;
  vas.record_stage_enabled = false;
  vas.playback_stage_enabled = false;
  vas.adc_clamp_enabled = false;
  vas.speech_codec_enabled = false; /* spec 008 FR-021 */
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
  RrDr60BlockInfo info;
  memset(&info, 0, sizeof info);
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

  /* Spec 006 US4 (contracts/c-api.md › Smoke test additions; T042): the record, stop, export,
   * import, play loop at 48 kHz with the default settings (the VAS on), the state at each step,
   * the memory figures, the refusals, a memory-full stop, erase all. The stimulus is a 1 kHz
   * square wave at -18 dBFS (+/-0.125, 24 samples up, 24 down): exactly representable in binary32, so the
   * Rust harness feeds the same bits and `ffi_parity` pins the FNV-1a 64 hash of the Rust
   * playback that both C playbacks must reproduce (US4 scenario 1). */
  {
    RrDr60Settings old = settings_011(FS);
    old.struct_size = 80; /* the 0.5 layout */
    RrDr60SettingField f = RR_DR60_SETTING_FIELD_NONE;
    CHECK(rr_dr60_settings_validate(&old, &f) == RR_DR60_STATUS_INVALID_ARGUMENT &&
              f == RR_DR60_SETTING_FIELD_STRUCT_SIZE,
          "validate(struct_size = 80) names STRUCT_SIZE (006)");
    old.struct_size = 88; /* the 0.6 / 0.7 layout (spec 008 contracts/c-api.md) */
    CHECK(rr_dr60_settings_validate(&old, &f) == RR_DR60_STATUS_INVALID_ARGUMENT &&
              f == RR_DR60_SETTING_FIELD_STRUCT_SIZE,
          "validate(struct_size = 88) names STRUCT_SIZE (008)");
    RrDr60Settings zero = settings_011(FS);
    zero.memory_capacity_s = 0;
    CHECK(rr_dr60_settings_validate(&zero, &f) == RR_DR60_STATUS_INVALID_SETTING &&
              f == RR_DR60_SETTING_FIELD_MEMORY_CAPACITY_S,
          "validate(memory_capacity_s = 0) names field 14 (006 FR-010)");
    CHECK(RR_DR60_SETTING_FIELD_MEMORY_CAPACITY_S == 14 && RR_DR60_SETTING_FIELD_MEMORY_MAX_RECORDINGS == 15,
          "memory field numbers");

    static float sq[FS];
    for (size_t i = 0; i < FS; i++) sq[i] = (i % 48) < 24 ? 0.125f : -0.125f;
    RrDr60Settings d = settings_011(FS);
    /* The 0.8 record-then-play case keeps 0.8's output: the output chain bypassed and the tap
       after stage 10 (spec 009 FR-016); 009 T031 adds the route cases with their own hashes. */
    d.output_volume_amp_enabled = false;
    d.output_speaker_enabled = false;
    d.output_earphone_enabled = false;
    d.tap = RR_DR60_TAP_AFTER_PLAYBACK;
    RrDr60Pipeline *a = NULL;
    CHECK(rr_dr60_create(&d, &a) == RR_DR60_STATUS_OK && a != NULL, "create (006 loop)");
    /* Spec 008 FR-004: with the codec on the default one-pass latency is 1923 host samples at
     * 48 kHz (483 + 1440, 40.06 ms). */
    CHECK(rr_dr60_latency_samples(a, &latency_default) == RR_DR60_STATUS_OK && latency_default == 1923,
          "default latency %u samples (must be 1923 with the codec on, 008 FR-004)", latency_default);
    RrDr60EngineState st;
    memset(&st, 0xEE, sizeof st);
    CHECK(rr_dr60_get_state(a, &st) == RR_DR60_STATUS_OK && st.state == RR_DR60_STATE_IDLE &&
              st.route == RR_DR60_ROUTE_SPEAKER && st.current_recording == 0 && !st.speaker_muted,
          "idle state");
    RrDr60MemoryStatus ms;
    CHECK(rr_dr60_memory_status(a, &ms) == RR_DR60_STATUS_OK && ms.count == 0 && ms.used_samples == 0 &&
              ms.remaining_samples == 21600000ULL && ms.max_recordings == 99 && ms.slots_remaining == 99,
          "default memory figures (S-001: 60 min, 99 items; device samples, A-026)");
    CHECK(rr_dr60_start_recording(a) == RR_DR60_STATUS_OK, "start_recording");
    CHECK(rr_dr60_get_state(a, &st) == RR_DR60_STATUS_OK && st.state == RR_DR60_STATE_RECORDING &&
              st.current_recording == 1 && st.speaker_muted,
          "recording state: current 1, speaker muted (S-002)");
    CHECK(rr_dr60_start_playback(a, 1) == RR_DR60_STATUS_RECORDING_IN_PROGRESS, "start_playback while recording");
    size_t cap = 0;
    CHECK(rr_dr60_max_record_events(a, 480, &cap) == RR_DR60_STATUS_OK, "max_record_events");
    RrDr60VasEvent *ev = (RrDr60VasEvent *)malloc((cap + 1) * sizeof *ev);
    CHECK(ev != NULL, "malloc");
    size_t stored = 0, nevents = 0;
    for (size_t at = 0; at < FS; at += 480) {
      RrDr60RecordInfo ri;
      CHECK(rr_dr60_record(a, sq + at, 480, &ri, ev, cap) == RR_DR60_STATUS_OK, "record block %zu", at / 480);
      CHECK(!ri.stopped, "stopped inside the loop");
      stored += ri.stored;
      nevents += ri.events;
    }
    RrDr60StopOutcome so;
    memset(&so, 0xEE, sizeof so);
    CHECK(rr_dr60_stop_recording(a, &so, ev, cap) == RR_DR60_STATUS_OK, "stop_recording");
    stored += so.flush.stored;
    /* Spec 008 FR-013: a whole number of frames; the kept samples run to the last frame's
     * look-ahead (length + 60); the frame count and the silence fed are the Rust path's. */
    CHECK(so.kept && so.number == 1 && so.length_samples % 120 == 0 &&
              so.length_samples == so.flush.frames_total * 120 && stored == so.length_samples + 60,
          "kept number %u, length %llu, frames %llu, stored %zu", so.number, (unsigned long long)so.length_samples,
          (unsigned long long)so.flush.frames_total, stored);
    CHECK(so.flush.frames_total == RR_DR60_SMOKE_FRAMES && so.silence_fed_samples == RR_DR60_SMOKE_SILENCE_FED,
          "frames %llu, silence fed %llu (Rust: %d, %d)", (unsigned long long)so.flush.frames_total,
          (unsigned long long)so.silence_fed_samples, RR_DR60_SMOKE_FRAMES, RR_DR60_SMOKE_SILENCE_FED);
    CHECK(nevents == 0, "a continuous tone has no VAS events (%zu)", nevents);
    free(ev);
    CHECK(rr_dr60_get_state(a, &st) == RR_DR60_STATUS_OK && st.state == RR_DR60_STATE_IDLE && !st.speaker_muted,
          "idle after stop");
    CHECK(rr_dr60_memory_status(a, &ms) == RR_DR60_STATUS_OK && ms.count == 1 && ms.used_samples == so.length_samples,
          "memory count 1, used %llu", (unsigned long long)ms.used_samples);
    uint64_t len_samples = 0;
    CHECK(rr_dr60_recording_length(a, 1, &len_samples) == RR_DR60_STATUS_OK && len_samples == so.length_samples,
          "recording_length");
    uint32_t enc = 0;
    CHECK(rr_dr60_get_encoding(a, &enc) == RR_DR60_STATUS_OK && enc == RR_DR60_ENCODING_FRAMES, "encoding %u", enc);
    size_t len = 0, written = 0;
    /* Form version 2 (spec 008 FR-010): 52 + 10 bytes per frame. */
    CHECK(rr_dr60_export_len(a, 1, &len) == RR_DR60_STATUS_OK && len == 52 + 10 * (size_t)so.flush.frames_total,
          "export_len %zu", len);
    uint8_t small[10];
    memset(small, 0xAA, sizeof small);
    CHECK(rr_dr60_export(a, 1, small, sizeof small, &written) == RR_DR60_STATUS_BUFFER_TOO_SMALL && written == len,
          "export into 10 bytes: BUFFER_TOO_SMALL, needed %zu", written);
    int untouched = 1;
    for (size_t i = 0; i < sizeof small; i++) untouched &= small[i] == 0xAA;
    CHECK(untouched, "BUFFER_TOO_SMALL must leave the buffer untouched");
    uint8_t *bytes = (uint8_t *)malloc(len);
    CHECK(bytes != NULL, "malloc");
    CHECK(rr_dr60_export(a, 1, bytes, len, &written) == RR_DR60_STATUS_OK && written == len, "export");
    CHECK(memcmp(bytes, "RRDR60RC", 8) == 0, "magic");
    CHECK(bytes[8] == 2 && bytes[9] == 0 && bytes[22] == 1 && bytes[23] == 0, "form version 2, encoding frames");
    /* The frame bytes (the form's units, 10 per frame) hash to the Rust path's (US4 scenario 1). */
    {
      uint64_t fh = 0xcbf29ce484222325ULL;
      for (size_t i = 48; i < len - 4; i++) {
        fh ^= bytes[i];
        fh *= 0x00000100000001b3ULL;
      }
      CHECK(fh == RR_DR60_SMOKE_FRAMES_HASH, "frame bytes hash %016llx differs from the Rust path's %016llx",
            (unsigned long long)fh, (unsigned long long)RR_DR60_SMOKE_FRAMES_HASH);
    }
    /* A bypassed-codec handle (the samples encoding, not a device configuration) refuses the
     * frames with ENCODING_DIFFERS (spec 008 FR-012). */
    {
      RrDr60Settings byp = d;
      byp.speech_codec_enabled = false;
      RrDr60Pipeline *bp = NULL;
      uint32_t n = 0, benc = 0;
      CHECK(rr_dr60_create(&byp, &bp) == RR_DR60_STATUS_OK && bp != NULL, "create bypassed handle");
      CHECK(rr_dr60_get_encoding(bp, &benc) == RR_DR60_STATUS_OK && benc == RR_DR60_ENCODING_SAMPLES, "bypassed encoding %u", benc);
      CHECK(rr_dr60_import(bp, bytes, len, &n) == RR_DR60_STATUS_ENCODING_DIFFERS, "frames into a bypassed handle: 21");
      size_t bf = 0, af = 0;
      CHECK(rr_dr60_footprint_bytes(bp, &bf) == RR_DR60_STATUS_OK && rr_dr60_footprint_bytes(a, &af) == RR_DR60_STATUS_OK &&
                af == 1801600 && bf == 86401600,
            "footprints %zu (codec on) / %zu (bypassed)", af, bf);
      rr_dr60_destroy(bp);
    }

    RrDr60Pipeline *b = NULL;
    CHECK(rr_dr60_create(&d, &b) == RR_DR60_STATUS_OK && b != NULL, "create second handle");
    uint32_t number = 0;
    CHECK(rr_dr60_import(b, bytes, len, &number) == RR_DR60_STATUS_OK && number == 1, "import -> number %u", number);
    bytes[40] ^= 0x01;
    CHECK(rr_dr60_import(b, bytes, len, &number) == RR_DR60_STATUS_DAMAGED, "import of a flipped byte");
    CHECK(rr_dr60_import(b, bytes, 4, &number) == RR_DR60_STATUS_UNKNOWN_FORM, "import of 4 bytes");
    free(bytes);

    /* Play both to `finished` and hash the outputs (FNV-1a 64 over the little-endian floats). */
    static float pa[8 * FS], pb[8 * FS];
    size_t na = 0, nb = 0;
    RrDr60Pipeline *handles[2] = {a, b};
    float *outs[2] = {pa, pb};
    size_t *counts[2] = {&na, &nb};
    for (int h = 0; h < 2; h++) {
      CHECK(rr_dr60_start_playback(handles[h], 1) == RR_DR60_STATUS_OK, "start_playback %d", h);
      CHECK(rr_dr60_get_state(handles[h], &st) == RR_DR60_STATUS_OK && st.state == RR_DR60_STATE_PLAYING &&
                st.current_recording == 1 && !st.speaker_muted,
            "playing state %d", h);
      uint64_t decoded = 0, last_index = 0;
      for (;;) {
        RrDr60PlayInfo pi;
        CHECK(*counts[h] + 480 <= 8 * (size_t)FS, "playback longer than expected");
        CHECK(rr_dr60_play(handles[h], outs[h] + *counts[h], 480, &pi) == RR_DR60_STATUS_OK, "play %d", h);
        *counts[h] += pi.produced;
        decoded += pi.frames;
        last_index = pi.frame_index;
        if (pi.finished) break;
      }
      CHECK(decoded == RR_DR60_SMOKE_FRAMES && last_index == RR_DR60_SMOKE_FRAMES - 1,
            "playback %d decoded %llu frames, last index %llu (008 FR-008)", h, (unsigned long long)decoded,
            (unsigned long long)last_index);
      CHECK(rr_dr60_get_state(handles[h], &st) == RR_DR60_STATUS_OK && st.state == RR_DR60_STATE_IDLE,
            "idle after finished %d", h);
    }
    CHECK(na == nb && memcmp(pa, pb, na * sizeof(float)) == 0, "imported playback differs (%zu vs %zu samples)", na, nb);
    CHECK(na == RR_DR60_SMOKE_PLAYBACK_SAMPLES, "playback %zu samples (Rust: %d)", na, RR_DR60_SMOKE_PLAYBACK_SAMPLES);
    uint64_t hash = 0xcbf29ce484222325ULL;
    for (size_t i = 0; i < na; i++) {
      uint8_t le[4];
      memcpy(le, &pa[i], 4); /* little-endian on every supported platform */
      for (int k = 0; k < 4; k++) {
        hash ^= le[k];
        hash *= 0x00000100000001b3ULL;
      }
    }
    CHECK(hash == RR_DR60_SMOKE_REFERENCE_HASH,
          "playback hash %016llx differs from the Rust path's %016llx (US4 scenario 1)",
          (unsigned long long)hash, (unsigned long long)RR_DR60_SMOKE_REFERENCE_HASH);
    uint32_t route = 9;
    CHECK(rr_dr60_set_route(a, RR_DR60_ROUTE_EARPHONE) == RR_DR60_STATUS_OK &&
              rr_dr60_get_route(a, &route) == RR_DR60_STATUS_OK && route == RR_DR60_ROUTE_EARPHONE,
          "route round trip");
    CHECK(rr_dr60_set_route(a, 7) == RR_DR60_STATUS_INVALID_ARGUMENT, "invalid route");
    uint32_t rl = 0, pl = 0, dr = 0;
    CHECK(rr_dr60_record_latency_samples(a, &rl) == RR_DR60_STATUS_OK &&
              rr_dr60_playback_latency_samples(a, &pl) == RR_DR60_STATUS_OK &&
              rr_dr60_drain_samples(a, &dr) == RR_DR60_STATUS_OK && rl + pl >= latency_default - 1 &&
              rl + pl <= latency_default + 1,
          "latencies %u + %u vs one-pass %u, drain %u (006 FR-016; 008 FR-004)", rl, pl, latency_default, dr);

    /* Spec 009 US4 (FR-014; SC-009): the same stimulus recorded once in the 0.9 default (the
     * output chain on) and played on the speaker route at RR_DR60_VOLUME_REFERENCE and at 1.0
     * (the clipper engaged) and on the earphone route at both; each playback's hash equals the
     * Rust path's pinned reference. The volume refusals and the route-tap refusal too. */
    {
      RrDr60Settings on = settings_011(FS);
      RrDr60Pipeline *r = NULL;
      CHECK(rr_dr60_create(&on, &r) == RR_DR60_STATUS_OK && r != NULL, "create (009 route cases)");
      float vol = -1.0f;
      /* The define is a C double literal; its nearest float is the Rust constant bit for bit. */
      CHECK(rr_dr60_get_volume(r, &vol) == RR_DR60_STATUS_OK && vol == (float)RR_DR60_VOLUME_REFERENCE,
            "default volume %g (A-043)", (double)vol);
      CHECK(rr_dr60_set_volume(r, 1.5f) == RR_DR60_STATUS_INVALID_VOLUME, "set_volume 1.5 refused");
      CHECK(rr_dr60_set_volume(r, NAN) == RR_DR60_STATUS_INVALID_VOLUME, "set_volume NaN refused");
      CHECK(rr_dr60_get_volume(r, &vol) == RR_DR60_STATUS_OK && vol == (float)RR_DR60_VOLUME_REFERENCE,
            "volume unchanged after a refused position");
      CHECK(rr_dr60_get_volume(r, NULL) == RR_DR60_STATUS_NULL_POINTER, "get_volume(NULL)");
      CHECK(rr_dr60_speaker_gain_db(1.0f) > 22.6f && rr_dr60_speaker_gain_db(1.0f) < 22.8f,
            "speaker gain at position 1: %g dB (22.7, A-031)", (double)rr_dr60_speaker_gain_db(1.0f));
      CHECK(rr_dr60_earphone_gain_db(1.0f) == 0.0f, "earphone gain at position 1: 0 dB (A-042)");
      CHECK(rr_dr60_start_recording(r) == RR_DR60_STATUS_OK, "start_recording (route cases)");
      for (size_t at = 0; at < FS; at += 480) {
        RrDr60RecordInfo ri;
        CHECK(rr_dr60_record(r, sq + at, 480, &ri, NULL, 0) == RR_DR60_STATUS_OK, "record (route cases) %zu", at / 480);
      }
      RrDr60StopOutcome rso;
      CHECK(rr_dr60_stop_recording(r, &rso, NULL, 0) == RR_DR60_STATUS_OK && rso.kept && rso.number == 1,
            "stop_recording (route cases)");
      static const uint32_t routes[4] = {RR_DR60_ROUTE_SPEAKER, RR_DR60_ROUTE_SPEAKER, RR_DR60_ROUTE_EARPHONE,
                                         RR_DR60_ROUTE_EARPHONE};
      const float volumes[4] = {(float)RR_DR60_VOLUME_REFERENCE, 1.0f, (float)RR_DR60_VOLUME_REFERENCE, 1.0f};
      const uint64_t want[4] = {RR_DR60_SMOKE_SPEAKER_REF_HASH, RR_DR60_SMOKE_SPEAKER_FULL_HASH,
                                RR_DR60_SMOKE_EARPHONE_REF_HASH, RR_DR60_SMOKE_EARPHONE_FULL_HASH};
      static float pr[8 * FS];
      for (int k = 0; k < 4; k++) {
        CHECK(rr_dr60_set_route(r, routes[k]) == RR_DR60_STATUS_OK && rr_dr60_set_volume(r, volumes[k]) == RR_DR60_STATUS_OK,
              "route case %d controls", k);
        CHECK(rr_dr60_start_playback(r, 1) == RR_DR60_STATUS_OK, "start_playback (route case %d)", k);
        size_t nr = 0;
        for (;;) {
          RrDr60PlayInfo pi;
          CHECK(nr + 480 <= 8 * (size_t)FS, "route playback longer than expected");
          CHECK(rr_dr60_play(r, pr + nr, 480, &pi) == RR_DR60_STATUS_OK, "play (route case %d)", k);
          nr += pi.produced;
          if (pi.finished) break;
        }
        CHECK(nr == RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES, "route case %d: %zu samples (Rust: %d)", k, nr,
              RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES);
        uint64_t h = 0xcbf29ce484222325ULL;
        for (size_t i = 0; i < nr; i++) {
          uint8_t le[4];
          memcpy(le, &pr[i], 4);
          for (int b = 0; b < 4; b++) {
            h ^= le[b];
            h *= 0x00000100000001b3ULL;
          }
        }
        CHECK(h == want[k], "route case %d: playback hash %016llx differs from the Rust path's %016llx (009 US4 scenario 3)", k,
              (unsigned long long)h, (unsigned long long)want[k]);
      }
      /* The named route tap with the other route selected is refused at the processing call. */
      RrDr60Settings spk_tap = settings_011(FS);
      spk_tap.tap = RR_DR60_TAP_AFTER_SPEAKER;
      RrDr60Pipeline *t = NULL;
      CHECK(rr_dr60_create(&spk_tap, &t) == RR_DR60_STATUS_OK && t != NULL, "create (speaker tap)");
      CHECK(rr_dr60_set_route(t, RR_DR60_ROUTE_EARPHONE) == RR_DR60_STATUS_OK, "earphone route on a speaker-tap handle");
      RrDr60BlockInfo bi;
      CHECK(rr_dr60_process(t, sq, pr, 480, &bi) == RR_DR60_STATUS_TAP_NOT_ON_ROUTE && bi.produced == 0,
            "process with the speaker tap on the earphone route: 24");
      CHECK(rr_dr60_set_route(t, RR_DR60_ROUTE_SPEAKER) == RR_DR60_STATUS_OK &&
                rr_dr60_process(t, sq, pr, 480, &bi) == RR_DR60_STATUS_OK,
            "process with the speaker tap on the speaker route");
      rr_dr60_destroy(t);
      rr_dr60_destroy(r);
    }

    /* Spec 010 US4 (FR-008; SC-006): a full-scale +/-1.0 square (exactly representable) recorded
     * once in the 0.9 default with the output clamp on and played at position 1 on each route:
     * every sample within [-1, 1] and each playback's hash equals the Rust path's; the same
     * square through an option-off handle on the speaker route exceeds 1.0 (the option acts);
     * a 0.9 caller's struct_size = 104 is refused by create and named by validate. */
    {
      static float fsq[FS];
      for (size_t i = 0; i < FS; i++) fsq[i] = (i % 48) < 24 ? 1.0f : -1.0f;
      RrDr60Settings stale = settings_011(FS);
      stale.struct_size = 104; /* the 0.9 layout */
      RrDr60Pipeline *q2 = (RrDr60Pipeline *)(uintptr_t)0x1;
      CHECK(rr_dr60_create(&stale, &q2) == RR_DR60_STATUS_INVALID_ARGUMENT, "create(struct_size = 104) refused (010)");
      RrDr60SettingField sf = RR_DR60_SETTING_FIELD_NONE;
      CHECK(rr_dr60_settings_validate(&stale, &sf) == RR_DR60_STATUS_INVALID_ARGUMENT &&
                sf == RR_DR60_SETTING_FIELD_STRUCT_SIZE,
            "validate(struct_size = 104) names STRUCT_SIZE (010)");
      RrDr60Settings cl = settings_011(FS);
      CHECK(!cl.output_clamp_enabled, "the output clamp is off by default (010 FR-002)");
      cl.output_clamp_enabled = true;
      RrDr60Pipeline *handles2[2] = {NULL, NULL};
      RrDr60Settings cfgs[2] = {cl, settings_011(FS)};
      for (int h = 0; h < 2; h++) {
        CHECK(rr_dr60_create(&cfgs[h], &handles2[h]) == RR_DR60_STATUS_OK && handles2[h] != NULL, "create (010 case %d)", h);
        CHECK(rr_dr60_start_recording(handles2[h]) == RR_DR60_STATUS_OK, "start_recording (010 case %d)", h);
        for (size_t at = 0; at < FS; at += 480) {
          RrDr60RecordInfo ri;
          CHECK(rr_dr60_record(handles2[h], fsq + at, 480, &ri, NULL, 0) == RR_DR60_STATUS_OK, "record (010) %zu", at / 480);
        }
        RrDr60StopOutcome cso;
        CHECK(rr_dr60_stop_recording(handles2[h], &cso, NULL, 0) == RR_DR60_STATUS_OK && cso.kept && cso.number == 1,
              "stop_recording (010 case %d)", h);
      }
      static const uint32_t croutes[2] = {RR_DR60_ROUTE_SPEAKER, RR_DR60_ROUTE_EARPHONE};
      const uint64_t cwant[2] = {RR_DR60_SMOKE_SPEAKER_CLAMP_HASH, RR_DR60_SMOKE_EARPHONE_CLAMP_HASH};
      static float pc[8 * FS];
      for (int k = 0; k < 2; k++) {
        CHECK(rr_dr60_set_route(handles2[0], croutes[k]) == RR_DR60_STATUS_OK &&
                  rr_dr60_set_volume(handles2[0], 1.0f) == RR_DR60_STATUS_OK,
              "clamp case %d controls", k);
        CHECK(rr_dr60_start_playback(handles2[0], 1) == RR_DR60_STATUS_OK, "start_playback (clamp case %d)", k);
        size_t nc = 0;
        int bounded = 1;
        for (;;) {
          RrDr60PlayInfo pi;
          CHECK(nc + 480 <= 8 * (size_t)FS, "clamp playback longer than expected");
          CHECK(rr_dr60_play(handles2[0], pc + nc, 480, &pi) == RR_DR60_STATUS_OK, "play (clamp case %d)", k);
          for (size_t i = 0; i < pi.produced; i++) bounded &= fabsf(pc[nc + i]) <= 1.0f;
          nc += pi.produced;
          if (pi.finished) break;
        }
        CHECK(bounded, "clamp case %d: every sample within [-1, 1] (010 FR-001)", k);
        CHECK(nc == RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES, "clamp case %d: %zu samples (Rust: %d)", k, nc,
              RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES);
        uint64_t hc = 0xcbf29ce484222325ULL;
        for (size_t i = 0; i < nc; i++) {
          uint8_t le[4];
          memcpy(le, &pc[i], 4);
          for (int b = 0; b < 4; b++) {
            hc ^= le[b];
            hc *= 0x00000100000001b3ULL;
          }
        }
        CHECK(hc == cwant[k], "clamp case %d: playback hash %016llx differs from the Rust path's %016llx (010 US4 scenario 1)", k,
              (unsigned long long)hc, (unsigned long long)cwant[k]);
      }
      /* The option off: the same square on the speaker route at position 1 exceeds the bound. */
      CHECK(rr_dr60_set_route(handles2[1], RR_DR60_ROUTE_SPEAKER) == RR_DR60_STATUS_OK &&
                rr_dr60_set_volume(handles2[1], 1.0f) == RR_DR60_STATUS_OK &&
                rr_dr60_start_playback(handles2[1], 1) == RR_DR60_STATUS_OK,
            "option-off playback");
      int exceeded = 0;
      for (;;) {
        RrDr60PlayInfo pi;
        CHECK(rr_dr60_play(handles2[1], pc, 480, &pi) == RR_DR60_STATUS_OK, "play (option off)");
        for (size_t i = 0; i < pi.produced; i++) exceeded |= fabsf(pc[i]) > 1.0f;
        if (pi.finished) break;
      }
      CHECK(exceeded, "the option-off speaker route exceeds 1.0 on the full-scale square (010 FR-011)");
      /* Spec 011 T035: the volume transition through C: the same recording on the speaker route at
       * 0.1, the position set to 1.0 before play block 10 and to 0.4 before block 11 (a retarget
       * mid-transition); the playback equals the Rust path's bit for bit. */
      CHECK(RR_DR60_VOLUME_TRANSITION_MS == 20, "RR_DR60_VOLUME_TRANSITION_MS is 20 (011 FR-002)");
      CHECK(rr_dr60_set_route(handles2[1], RR_DR60_ROUTE_SPEAKER) == RR_DR60_STATUS_OK &&
                rr_dr60_set_volume(handles2[1], 0.1f) == RR_DR60_STATUS_OK &&
                rr_dr60_start_playback(handles2[1], 1) == RR_DR60_STATUS_OK,
            "volume-transition playback");
      size_t nr2 = 0;
      for (int block = 0;; block++) {
        if (block == 10) CHECK(rr_dr60_set_volume(handles2[1], 1.0f) == RR_DR60_STATUS_OK, "set_volume 1.0 (011)");
        if (block == 11) CHECK(rr_dr60_set_volume(handles2[1], 0.4f) == RR_DR60_STATUS_OK, "set_volume 0.4 (011)");
        RrDr60PlayInfo pi;
        CHECK(nr2 + 480 <= 8 * (size_t)FS, "transition playback longer than expected");
        CHECK(rr_dr60_play(handles2[1], pc + nr2, 480, &pi) == RR_DR60_STATUS_OK, "play (011)");
        nr2 += pi.produced;
        if (pi.finished) break;
      }
      float vread = 0.0f;
      CHECK(rr_dr60_get_volume(handles2[1], &vread) == RR_DR60_STATUS_OK && vread == 0.4f,
            "get_volume returns the last setting (011 FR-011)");
      CHECK(nr2 == RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES, "transition case: %zu samples (Rust: %d)", nr2,
            RR_DR60_SMOKE_ROUTE_PLAYBACK_SAMPLES);
      uint64_t hr = 0xcbf29ce484222325ULL;
      for (size_t i = 0; i < nr2; i++) {
        uint8_t le[4];
        memcpy(le, &pc[i], 4);
        for (int b = 0; b < 4; b++) {
          hr ^= le[b];
          hr *= 0x00000100000001b3ULL;
        }
      }
      CHECK(hr == RR_DR60_SMOKE_SPEAKER_RAMP_HASH,
            "transition case: playback hash %016llx differs from the Rust path's %016llx (011 T035)",
            (unsigned long long)hr, (unsigned long long)RR_DR60_SMOKE_SPEAKER_RAMP_HASH);
      rr_dr60_destroy(handles2[0]);
      rr_dr60_destroy(handles2[1]);
    }
    CHECK(rr_dr60_erase_all(a) == RR_DR60_STATUS_OK && rr_dr60_memory_status(a, &ms) == RR_DR60_STATUS_OK &&
              ms.count == 0 && ms.used_samples == 0,
          "erase_all -> count 0");
    CHECK(rr_dr60_erase(b, 1) == RR_DR60_STATUS_OK && rr_dr60_erase(b, 1) == RR_DR60_STATUS_NO_SUCH_RECORDING,
          "erase, then no such recording");

    /* A 1 s-capacity handle: the recording stops by itself with MEMORY_FULL; then NOT_RECORDING. */
    RrDr60Settings one = settings_011(FS);
    one.memory_capacity_s = 1;
    one.vas_enabled = false;
    RrDr60Pipeline *c = NULL;
    CHECK(rr_dr60_create(&one, &c) == RR_DR60_STATUS_OK && c != NULL, "create 1 s handle");
    CHECK(rr_dr60_start_recording(c) == RR_DR60_STATUS_OK, "start 1 s");
    size_t total = 0;
    int full = 0;
    for (int round = 0; round < 3 && !full; round++) {
      for (size_t at = 0; at < FS && !full; at += 4800) {
        RrDr60RecordInfo ri;
        CHECK(rr_dr60_record(c, sq + at, 4800, &ri, NULL, 0) == RR_DR60_STATUS_OK, "record into 1 s");
        total += ri.stored;
        if (ri.stopped) {
          CHECK(ri.stop_reason == RR_DR60_STOP_REASON_MEMORY_FULL, "stop reason %u", ri.stop_reason);
          full = 1;
        }
      }
    }
    /* Spec 008 FR-013: the 50th frame is appended when the kept count reaches 6000 + the 60-sample
     * look-ahead; the recording's length is the 50 frames. */
    CHECK(full && total == 6060, "memory-full stop at %zu kept samples (must be 6060)", total);
    uint64_t full_len = 0;
    CHECK(rr_dr60_recording_length(c, 1, &full_len) == RR_DR60_STATUS_OK && full_len == 6000,
          "memory-full recording length %llu (must be 6000)", (unsigned long long)full_len);
    CHECK(rr_dr60_stop_recording(c, &so, NULL, 0) == RR_DR60_STATUS_NOT_RECORDING, "stop after a memory-full stop");
    CHECK(rr_dr60_start_recording(c) == RR_DR60_STATUS_MEMORY_FULL, "start when full");
    CHECK(rr_dr60_record(c, sq, 480, NULL, NULL, 0) == RR_DR60_STATUS_NOT_RECORDING, "record while idle");
    rr_dr60_destroy(a);
    rr_dr60_destroy(b);
    rr_dr60_destroy(c);
  }

  /* Spec 012 T048 (FR-012, SC-009; contracts/c-api.md › Smoke loop): the microphone path. */
  {
    RrDr60Settings m = rr_dr60_settings_default(FS);
    CHECK(m.mic_capsule_enabled && m.mic_amp_enabled && m.mic_hiss_offset_db == 0.0f,
          "the 0.12 default turns stages 1 - 2 on at offset 0 (012 FR-001)");
    RrDr60Pipeline *mq = NULL;
    RrDr60SettingField mf;
    /* (c) The 0.11 size (112 bytes) is refused, naming STRUCT_SIZE. */
    RrDr60Settings old = m;
    old.struct_size = 112;
    CHECK(rr_dr60_settings_validate(&old, &mf) == RR_DR60_STATUS_INVALID_ARGUMENT &&
              mf == RR_DR60_SETTING_FIELD_STRUCT_SIZE,
          "validate(struct_size = 112) names STRUCT_SIZE (012 FR-012)");
    CHECK(rr_dr60_create(&old, &mq) == RR_DR60_STATUS_INVALID_ARGUMENT && mq == NULL, "create(struct_size = 112)");
    /* (d) Offsets 13.0 and NaN are refused, with the capsule on or off. */
    for (int capsule = 0; capsule < 2; capsule++) {
      const float bad_offsets[2] = {13.0f, NAN};
      for (int k = 0; k < 2; k++) {
        RrDr60Settings b = m;
        b.mic_capsule_enabled = capsule != 0;
        b.mic_hiss_offset_db = bad_offsets[k];
        CHECK(rr_dr60_settings_validate(&b, &mf) == RR_DR60_STATUS_INVALID_SETTING &&
                  mf == RR_DR60_SETTING_FIELD_MIC_HISS_OFFSET_DB,
              "validate(offset %g, capsule %d) names MIC_HISS_OFFSET_DB (012 FR-003)", (double)bad_offsets[k], capsule);
        CHECK(rr_dr60_create(&b, &mq) == RR_DR60_STATUS_INVALID_SETTING && mq == NULL, "create(bad offset)");
      }
    }
    /* (a) The 0.12 default one-pass on the square (1 s) plus 0.5 s of silence, 480-sample blocks. */
    static float min_[FS + FS / 2], mout[FS + FS / 2], mall[FS + FS / 2];
    for (size_t i = 0; i < (size_t)(FS + FS / 2); i++) min_[i] = i < (size_t)FS ? (i % 48 < 24 ? 0.125f : -0.125f) : 0.0f;
    CHECK(rr_dr60_create(&m, &mq) == RR_DR60_STATUS_OK && mq != NULL, "create (0.12 default)");
    size_t mn = 0;
    for (size_t at = 0; at < (size_t)(FS + FS / 2); at += 480) {
      RrDr60BlockInfo bi;
      CHECK(rr_dr60_process(mq, min_ + at, mout, 480, &bi) == RR_DR60_STATUS_OK, "process (0.12 default)");
      memcpy(mall + mn, mout, bi.produced * sizeof(float));
      mn += bi.produced;
    }
    uint64_t mh = fnv_floats(mall, mn);
    CHECK(mn == RR_DR60_SMOKE_MIC_DEFAULT_SAMPLES, "0.12 default one-pass: %zu samples (Rust: %d)", mn,
          RR_DR60_SMOKE_MIC_DEFAULT_SAMPLES);
    CHECK(mh == RR_DR60_SMOKE_MIC_DEFAULT_HASH, "0.12 default one-pass hash %016llx differs from the Rust path's %016llx",
          (unsigned long long)mh, (unsigned long long)RR_DR60_SMOKE_MIC_DEFAULT_HASH);
    rr_dr60_destroy(mq);
    /* (b) Two recordings of 1 s of silence (the hiss) with a reset between them: each plays back
       the Rust path's bits, and they differ (the recording counter advanced; 012 FR-009). */
    mq = NULL;
    CHECK(rr_dr60_create(&m, &mq) == RR_DR60_STATUS_OK && mq != NULL, "create (recordings)");
    static float zeros[480];
    static float mplay[2][8 * FS];
    size_t mlen[2] = {0, 0};
    for (int r = 0; r < 2; r++) {
      if (r == 1) CHECK(rr_dr60_reset(mq) == RR_DR60_STATUS_OK, "reset between the recordings");
      CHECK(rr_dr60_start_recording(mq) == RR_DR60_STATUS_OK, "start recording %d", r);
      for (size_t at = 0; at < (size_t)FS; at += 480)
        CHECK(rr_dr60_record(mq, zeros, 480, NULL, NULL, 0) == RR_DR60_STATUS_OK, "record silence");
      RrDr60StopOutcome mso;
      CHECK(rr_dr60_stop_recording(mq, &mso, NULL, 0) == RR_DR60_STATUS_OK && mso.kept, "stop recording %d", r);
      CHECK(rr_dr60_start_playback(mq, mso.number) == RR_DR60_STATUS_OK, "play recording %u", mso.number);
      for (;;) {
        RrDr60PlayInfo pi;
        CHECK(mlen[r] + 480 <= 8 * (size_t)FS, "hiss playback longer than expected");
        CHECK(rr_dr60_play(mq, mplay[r] + mlen[r], 480, &pi) == RR_DR60_STATUS_OK, "play (hiss)");
        mlen[r] += pi.produced;
        if (pi.finished) break;
      }
    }
    uint64_t h0 = fnv_floats(mplay[0], mlen[0]), h1 = fnv_floats(mplay[1], mlen[1]);
    CHECK(mlen[0] == RR_DR60_SMOKE_MIC_REC_SAMPLES && mlen[1] == RR_DR60_SMOKE_MIC_REC_SAMPLES,
          "hiss recordings: %zu and %zu samples (Rust: %d)", mlen[0], mlen[1], RR_DR60_SMOKE_MIC_REC_SAMPLES);
    CHECK(h0 == RR_DR60_SMOKE_MIC_REC0_HASH && h1 == RR_DR60_SMOKE_MIC_REC1_HASH,
          "hiss recordings' hashes %016llx / %016llx differ from the Rust path's %016llx / %016llx",
          (unsigned long long)h0, (unsigned long long)h1, (unsigned long long)RR_DR60_SMOKE_MIC_REC0_HASH,
          (unsigned long long)RR_DR60_SMOKE_MIC_REC1_HASH);
    CHECK(h0 != h1, "the two recordings carry different hiss (012 FR-009)");
    rr_dr60_destroy(mq);
  }

  /* Spec 007 US3: the stage manifest through the C API (contracts/c-api.md › Smoke test). */
  {
    RrDr60ManifestHeader h;
    RrDr60StageEntry e;
    const char *id = NULL;
    const char *kept = NULL;
    CHECK(rr_dr60_manifest_stage_count() == 15, "stage count %u", rr_dr60_manifest_stage_count());
    CHECK(rr_dr60_manifest_header(&h) == RR_DR60_STATUS_OK, "manifest header");
    CHECK(h.count_not_modeled + h.count_assumed + h.count_sourced + h.count_checked == h.stage_count, "counts sum");
    CHECK(h.count_checked == 0, "no stage is checked against a real unit today");
    CHECK(h.library_version && h.basis && h.evidence_link && h.assumptions_link && h.sources_link, "header strings");
    e.number = 0xFFFFFFFFu;
    CHECK(rr_dr60_manifest_stage(0, &e) == RR_DR60_STATUS_NO_SUCH_STAGE, "stage 0 refused");
    CHECK(rr_dr60_manifest_stage(16, &e) == RR_DR60_STATUS_NO_SUCH_STAGE, "stage 16 refused");
    CHECK(e.number == 0xFFFFFFFFu, "a refused read leaves the struct untouched");
    for (uint32_t n = 1; n <= 15; n++) {
      CHECK(rr_dr60_manifest_stage(n, &e) == RR_DR60_STATUS_OK, "stage %u", n);
      CHECK(e.number == n && e.identifier && e.name && e.description && e.coverage && e.controls && e.model_version,
            "stage %u fields", n);
      for (uint32_t i = 0; i < e.source_count; i++)
        CHECK(rr_dr60_manifest_stage_source(n, i, &id) == RR_DR60_STATUS_OK && id && id[0] == 'S', "stage %u source %u", n, i);
      for (uint32_t i = 0; i < e.assumption_count; i++)
        CHECK(rr_dr60_manifest_stage_assumption(n, i, &id) == RR_DR60_STATUS_OK && id && id[0] == 'A', "stage %u assumption %u", n, i);
      CHECK(rr_dr60_manifest_stage_source(n, e.source_count, &id) == RR_DR60_STATUS_INVALID_ARGUMENT, "index at count");
      if (n == 3) kept = e.name;
    }
    /* US3 scenario 3: a kept pointer survives a create/destroy cycle. */
    {
      RrDr60Settings ms = settings_011(FS);
      RrDr60Pipeline *mp = NULL;
      CHECK(rr_dr60_create(&ms, &mp) == RR_DR60_STATUS_OK, "create for the pointer-lifetime check");
      rr_dr60_destroy(mp);
      CHECK(rr_dr60_manifest_stage(3, &e) == RR_DR60_STATUS_OK, "stage 3 again");
      CHECK(kept && strcmp(kept, e.name) == 0 && strcmp(kept, "Limiter (AGC)") == 0, "kept pointer still valid");
    }
  }

  /* Spec 008 FR-003 (contracts/c-api.md): the codec's constants, handle-less. */
  RrDr60SpeechCodecInfo ci;
  memset(&ci, 0xEE, sizeof ci);
  CHECK(rr_dr60_speech_codec_info(NULL) == RR_DR60_STATUS_NULL_POINTER, "speech_codec_info(NULL)");
  CHECK(rr_dr60_speech_codec_info(&ci) == RR_DR60_STATUS_OK && ci.frame_samples == 120 && ci.frame_ms == 20 &&
            ci.bits_per_frame == 80 && ci.delay_samples == 180 && ci.model_version[0] == 0 &&
            ci.model_version[1] == 8 && ci.model_version[2] == 0 && ci.reserved == 0,
        "speech_codec_info: %u samples, %u ms, %u bits, delay %u, model %u.%u.%u", ci.frame_samples, ci.frame_ms,
        ci.bits_per_frame, ci.delay_samples, ci.model_version[0], ci.model_version[1], ci.model_version[2]);
  CHECK(sizeof ci == 24, "RrDr60SpeechCodecInfo is %zu bytes (must be 24)", sizeof ci);
  printf("speech codec: %u samples/frame (%u ms), %u bits/frame, delay %u samples, model %u.%u.%u\n", ci.frame_samples,
         ci.frame_ms, ci.bits_per_frame, ci.delay_samples, ci.model_version[0], ci.model_version[1], ci.model_version[2]);

  if (failures) {
    fprintf(stderr, "smoke: %d failure(s)\n", failures);
    return 1;
  }
  printf("smoke: OK (rr_dr60 %s, latency %u samples at %d Hz, %u frames/s)\n", rr_dr60_version_string(), latency_default,
         FS, FS6 / ci.frame_samples);
  return 0;
}
