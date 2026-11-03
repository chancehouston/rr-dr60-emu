# Contract: the C surface of 026 (appended)

```c
int32_t rr_dr60_settings_one_pass_vas_runs(const RrDr60Settings *s, bool *out);    // 025's field-local read
int32_t rr_dr60_settings_one_pass_codec_runs(const RrDr60Settings *s, bool *out);
int32_t rr_dr60_one_pass_report_latency_samples(const RrDr60Pipeline *p, uint32_t *out);
int32_t rr_dr60_one_pass_drain_samples(const RrDr60Pipeline *p, uint64_t *out);
int32_t rr_dr60_wording_first_hit(const uint8_t *text, size_t len, uint32_t kind,
                                  const char **out_entry, size_t *out_start, size_t *out_end);
uint32_t rr_dr60_meter_field_count(void);
int32_t rr_dr60_meter_field(uint32_t index, RrDr60MeterField *out);                 // sized, [40]
int32_t rr_dr60_meter_reached(const RrDr60Pipeline *p, uint32_t index, uint32_t call, bool *out);
int32_t rr_dr60_meter_lag_samples(const RrDr60Pipeline *p, uint32_t index, uint32_t call, uint64_t *out);
int32_t rr_dr60_manifest_word(uint32_t kind, uint32_t value, uint32_t form, const char **out);
```

`RrDr60MeterField` (64-bit):
- `uint32_t struct_size` at 0 and `uint32_t index` at 4;
- `const char *identifier`, `*unit` and `*sentence` at 8, 16 and 24;
- `uint32_t kind` at 32 and `uint32_t side` at 36;
- 40 bytes.

The defines:
- `RR_DR60_WORDING_ENGINE_STRINGS` 0 and `RR_DR60_WORDING_CLAIMS` 1;
- `RR_DR60_METER_CALL_ONE_PASS` 0, `_RECORD` 1 and `_PLAY` 2;
- the manifest word kinds (status 0, path 1, confidence 2) and forms (token 0, label 1).

The statuses:
- 44 `NO_SUCH_METER_FIELD`;
- 45 `NO_SUCH_METER_CALL`;
- 46 `INVALID_UTF8`;
- 47 `NO_SUCH_WORDING_KIND`;
- 48 `NO_SUCH_MANIFEST_WORD`.

The refusal orders are those of plan R-08.
