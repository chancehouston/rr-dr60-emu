# Contract: the C surface of 027 (appended)

```c
typedef struct RrDr60ReRenderPlan RrDr60ReRenderPlan;
typedef struct RrDr60ReRender RrDr60ReRender;

int32_t rr_dr60_rerender_plan_from_record(const uint8_t *bytes, size_t len, RrDr60ReRenderPlan **out);
void    rr_dr60_rerender_plan_destroy(RrDr60ReRenderPlan *plan);
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

int32_t rr_dr60_rerender_new(RrDr60ReRenderPlan *plan, RrDr60ReRender **out);   /* consumes plan */
int32_t rr_dr60_rerender_feed_i24(RrDr60ReRender *r, const int32_t *codes, size_t n, RrDr60ReRenderFed *out);
int32_t rr_dr60_rerender_feed_f32(RrDr60ReRender *r, const float *samples, size_t n, RrDr60ReRenderFed *out);
int32_t rr_dr60_rerender_progress(const RrDr60ReRender *r, uint64_t *input_position, uint64_t *stop_position);
int32_t rr_dr60_rerender_meters(const RrDr60ReRender *r, RrDr60Meters *out);
int32_t rr_dr60_rerender_finish(RrDr60ReRender *r, RrDr60ReRenderOutcome *out);
int32_t rr_dr60_rerender_export_len(const RrDr60ReRender *r, size_t *out);
int32_t rr_dr60_rerender_export(const RrDr60ReRender *r, uint8_t *out, size_t len, size_t *written);
int32_t rr_dr60_rerender_record_len(const RrDr60ReRender *r, size_t *out);
int32_t rr_dr60_rerender_record_write(const RrDr60ReRender *r, uint8_t *out, size_t len, size_t *written);
void    rr_dr60_rerender_destroy(RrDr60ReRender *r);

int32_t rr_dr60_fill_to_remaining(RrDr60Pipeline *p, uint64_t remaining);
```

`RrDr60ReRenderFed` (sized, `[24]`): `struct_size`, `taken` (u64), `reached_stop`, `self_stopped`.
`RrDr60ReRenderOutcome` (sized, `[48]`): `struct_size`, `reason` (0 kept, 1 discarded empty), `verdict` (0 identical, 1 differs, 2 rebuilt), `edited`, `matches_original`, `has_digest`, `flags`, `digest` (u64), `remaining_at_start` (u64).

Statuses appended after 48: `INPUT_SHORT`, `INVALID_SAMPLE`, `RUN_TOO_LARGE`, `FEED_KIND_MIXED`, `HOST_RATE_FIXED`, `RUN_NOT_COLLECTED`, `RERENDER_STATE`. Refusal orders: the handle, then NULL, then size, then the call's own checks. Every entry point panic-guarded; a panic poisons its handle.
