# Contract: the C surface of 027 (appended)

```c
typedef struct RrDr60ReRenderPlan RrDr60ReRenderPlan;
typedef struct RrDr60ReRender RrDr60ReRender;

int32_t rr_dr60_rerender_plan_from_record(const uint8_t *bytes, size_t len, RrDr60ReRenderPlan **out);
void    rr_dr60_rerender_plan_destroy(RrDr60ReRenderPlan *plan);
int32_t rr_dr60_rerender_plan_clone(const RrDr60ReRenderPlan *plan, RrDr60ReRenderPlan **out);
int32_t rr_dr60_rerender_plan_control_value(const RrDr60ReRenderPlan *plan, uint32_t index, RrDr60ControlReading *out);
int32_t rr_dr60_rerender_plan_component(const RrDr60ReRenderPlan *plan, uint32_t *out);
int32_t rr_dr60_rerender_plan_parameter(const RrDr60ReRenderPlan *plan, uint32_t index, double *out);
int32_t rr_dr60_rerender_plan_seed(const RrDr60ReRenderPlan *plan, uint64_t *out);
int32_t rr_dr60_rerender_plan_counter(const RrDr60ReRenderPlan *plan, uint64_t *out);
int32_t rr_dr60_rerender_plan_remaining_samples(const RrDr60ReRenderPlan *plan, uint64_t *out);
int32_t rr_dr60_rerender_plan_flags(const RrDr60ReRenderPlan *plan, uint32_t *out);
int32_t rr_dr60_rerender_plan_set_control_value(RrDr60ReRenderPlan *plan, uint32_t index, const RrDr60ControlReading *value);
int32_t rr_dr60_rerender_plan_set_component(RrDr60ReRenderPlan *plan, uint32_t component);
int32_t rr_dr60_rerender_plan_set_parameter(RrDr60ReRenderPlan *plan, uint32_t index, double value);
int32_t rr_dr60_rerender_plan_row_count(const RrDr60ReRenderPlan *plan, uint64_t *out);
int32_t rr_dr60_rerender_plan_row(const RrDr60ReRenderPlan *plan, uint64_t i, RrDr60ChangeEntry *out);
int32_t rr_dr60_rerender_plan_set_row(RrDr60ReRenderPlan *plan, uint64_t i, const RrDr60ChangeEntry *row);
int32_t rr_dr60_rerender_plan_insert_row(RrDr60ReRenderPlan *plan, const RrDr60ChangeEntry *row);
int32_t rr_dr60_rerender_plan_remove_row(RrDr60ReRenderPlan *plan, uint64_t i);
int32_t rr_dr60_rerender_plan_set_seed(RrDr60ReRenderPlan *plan, uint64_t seed);
int32_t rr_dr60_rerender_plan_set_counter(RrDr60ReRenderPlan *plan, uint64_t counter);
int32_t rr_dr60_rerender_plan_set_remaining_samples(RrDr60ReRenderPlan *plan, uint64_t remaining);
int32_t rr_dr60_rerender_plan_set_collect_run(RrDr60ReRenderPlan *plan, bool collect);
int32_t rr_dr60_rerender_plan_edited(const RrDr60ReRenderPlan *plan, bool *out);

int32_t rr_dr60_rerender_new(RrDr60ReRenderPlan *plan, RrDr60ReRender **out);   /* consumes plan on success only */
int32_t rr_dr60_rerender_feed_i24(RrDr60ReRender *r, const int32_t *codes, size_t n, RrDr60ReRenderFed *out);
int32_t rr_dr60_rerender_feed_f32(RrDr60ReRender *r, const float *samples, size_t n, RrDr60ReRenderFed *out);
int32_t rr_dr60_rerender_progress(const RrDr60ReRender *r, uint64_t *input_position, uint64_t *stop_position);
int32_t rr_dr60_rerender_meters(const RrDr60ReRender *r, RrDr60Meters *out);
int32_t rr_dr60_rerender_finish(RrDr60ReRender *r, RrDr60ReRenderOutcome *out);
int32_t rr_dr60_rerender_export_len(const RrDr60ReRender *r, size_t *out);
int32_t rr_dr60_rerender_export(const RrDr60ReRender *r, uint8_t *out, size_t len, size_t *written);
int32_t rr_dr60_rerender_record_len(const RrDr60ReRender *r, size_t *out);
int32_t rr_dr60_rerender_record_write(const RrDr60ReRender *r, uint8_t *out, size_t len, size_t *written);
int32_t rr_dr60_rerender_into_pipeline(RrDr60ReRender *r, RrDr60Pipeline **out);   /* consumes a finished session */
void    rr_dr60_rerender_destroy(RrDr60ReRender *r);

int32_t rr_dr60_fill_to_remaining(RrDr60Pipeline *p, uint64_t remaining);
```

`RrDr60ReRenderFed` (published `[24]`): `uint32_t struct_size` 0, `bool reached_stop` 4, `bool self_stopped` 5, pad 6 – 7, `uint64_t taken` 8, `uint64_t reserved` 16.

`RrDr60ReRenderOutcome` (published `[56]`): `uint32_t struct_size` 0, `uint32_t reason` 4 (0 kept, 1 discarded empty), `uint32_t verdict` 8 (0 identical, 1 differs, 2 rebuilt), `uint32_t flags` 12, `bool edited` 16, `bool matches_original` 17, `bool has_digest` 18, pad 19 – 23, `uint64_t digest` 24, `uint64_t remaining_at_start` 32, `uint64_t rows_landed` 40, `uint64_t reserved` 48.

Statuses appended after 48: `INPUT_SHORT`, `INVALID_SAMPLE`, `RUN_TOO_LARGE`, `FEED_KIND_MIXED`, `HOST_RATE_FIXED`, `RUN_NOT_COLLECTED`, `RERENDER_STATE`, `IMPORTED_RECORD`, `UNKNOWN_COMPONENT`. After `INPUT_SHORT` the samples still needed are the stop position minus the input position (`_progress`); the session stays feedable. `_plan_set_control_value` takes settings controls only (the live controls and the slot's 31 – 35 refused). `_export_len` and `_record_len` read 0 with OK after a discarded outcome. Refusal orders: the handle, then NULL, then size, then the call's own checks. Every entry point panic-guarded; a panic poisons its handle.
