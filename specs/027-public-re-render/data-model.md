# Data model: the public re-render

- **Plan** (`rr_dr60::rerender::Plan`): the read-out of a record (`RecordReadout`), its change rows (`Vec<ChangeEntry>`, sorted by position, stable at equal positions, at most 64 per position), the slot's component and the 16-value store, `collect_run` (default true), and the untouched copy for `edited()`. Built by `Plan::from_record(bytes)`; edits validated as the engine validates them; the host rate fixed.
- **ReRender** (the session): the engine (`Pipeline` built at the record's counter, the slot restored, the memory filled), the next unscheduled row, the feed kind (none / i24 / f32, fixed by the first feed), the stop position, the run buffers (events and changes, reserved at `new`), the scratch buffers (one `CHUNK` of 4096 host samples), `self_stopped`.
- **Fed**: `taken`, `reached_stop`, `self_stopped`.
- **Outcome**: `reason` (`Kept` / `DiscardedEmpty`), `digest` (`Option<u64>`), `edited`, `matches_original`, `verdict` (`Identical` / `Differs` / `Rebuilt`), `flags` (the read-out's), `remaining_at_start` (the remaining capacity the fill reached).
- **Finished** (the session after `finish`): the outcome and the engine; `export_len`/`export`, `record_len`/`record_write`, `into_pipeline`.

State: Plan → (new) → ReRender: feeding → (finish) → Finished. Drop at any point cancels.
