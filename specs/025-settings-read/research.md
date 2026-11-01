# Research: The Settings Read and Control Access by Index

## R-01 The mapping and its exhaustiveness

**Decision**: `control_map.rs` holds `fn target(index) -> Option<Target>` where `Target::Setting(SettingAccess)` names the field (an enum with one variant per settings field: 26) and `Target::Live(LiveControl)` one of five (route, volume, speed, speed mode, battery). The index is resolved through the manifest's identifier (`controls()[index].identifier`), matched to a variant by a `match` with no wildcard on the 31 identifiers (a manifest entry without an arm fails the core test that walks every index). The settings side is exhaustive in the other direction by a `fn _exhaustive(s: &Settings)` that destructures `Settings` and its sub-structures with no `..` and lists each field against its `SettingAccess` variant, so a new field fails to compile. `get(&Settings, SettingAccess) -> ControlReading` and `set(&mut Settings, SettingAccess, ControlReading)` are `match`es on the variant. `setting_field_control(field)` maps the validator's field enum to the index through the same variants (the field enum names a variant directly).

**Rationale**: the reviews (coach 1, hardware B5) found the record's string table neither complete nor exhaustive; a variant per field with a compile-time destructure is both.

**Alternatives**: a macro generating everything from one list (fewer lines, a less readable error when a field is missing); offsets computed by hand (no compile-time check).

## R-02 The reading type and its C layout

**Decision**: Rust `enum ControlReading { Switch(bool), Integer(u64), Continuous(f32), Choice(u32) }`. C `RrDr60ControlReading { uint32_t struct_size; uint32_t value_type; uint64_t int_value; float float_value; uint32_t reserved; }`, 24 bytes, offsets 0 / 4 / 8 / 16 / 20, pinned by const assertions; `value_type` is spec 014's `RrDr60ControlValueType` (switch 0, integer 1, continuous 2, choice 3); the unused field and `reserved` zero on output, ignored on input. Published layouts `[24]` under spec 022's `write_sized`.

## R-03 The field-local C write and the in-place read

**Decision**: `rr_dr60_set_settings_control_value(settings, index, reading)`: the size of `*settings` is read through the raw pointer (`struct_size_at`), equal to the settings size or refused; the reading copied into a local after its size check; the target field written through `core::ptr::addr_of_mut!((*settings).field)` with `write` (switches as the bytes 0 or 1, choices as their enum's integer, continuous as `f32`, integers within their C width, which is at least the Rust width); no `&RrDr60Settings` or `&mut RrDr60Settings` is formed. `rr_dr60_settings_control_value` reads the one field the same way, a switch read as `u8` and refused unless 0 or 1, a choice refused unless one of its options. `rr_dr60_get_settings`: the size equality first, then the buffer zeroed in place (`write_bytes`) and filled by `fill_settings` field by field (spec 022 D10). The Miri gate (`ci.yml`'s `miri` job) gains the new functions' tests.

## R-04 The conversion shared with `schedule_change`

**Decision**: one `fn reading_value(control, reading) -> Result<f64, Error>` gives the f64 that `schedule_change` takes (spec 015 R-04: continuous narrowed through f32 already, −0.0 → 0.0; switches 0.0 / 1.0; choices their value; integers as f64 for the level), and `set_control_value` calls the setter the plain API offers with that value, so the three paths (by index, the setter, a schedule at the current position) log identical entries.

## R-05 What the read holds

**Decision**: `Pipeline::settings()` unchanged: the last value set or landed on any stream (spec 017's mirror into the settings; spec 021's switches at their target during a blend); a stream applies it from its next produced sample (the one-pass at its next call's first sample; the recorder adopts at `start_recording`); an unlanded scheduled change is absent. The guide states it; US1 AS3 pins it.

## R-06 The refusal order

**Decision**, per function, nothing written on any refusal: `get_settings` (NULL, size); `control_value` (NULL handle or out, the out size, the index, poisoned); `settings_control_value` (NULL, the settings size, the out size, the index, not a setting, an invalid stored value); `set_settings_control_value` (NULL, the settings size, the reading size, the index, not a setting, representability); `set_control_value` (NULL, the reading size, the index, not a plain live control, representability, the setter's status); `setting_field_control` (NULL, `NONE`/`STRUCT_SIZE` → `NO_SUCH_CONTROL`).

## R-07 The oracles and the mutants

**Decision**: the one-hot test (Rust): for each settings index, from the defaults, write the index's unique non-default value, destructure `Settings` exhaustively in the test source with a literal identifier per field, assert exactly one changed and its literal is the manifest identifier. The C one-hot: in the harness through the FFI on `rr_dr60_settings_default`, against a structure whose field is set by its C name in `examples/settings_rows.rs` and the C smoke (the compiler names the field), byte for byte. The live identity: an engine set by index, a twin through the setter, a third through a schedule at the current position, at block size 1: identical outputs and logs. Mutants (`__test-hooks` on the mapping): index i → field of i + 1; attack ↔ release; speaker ↔ earphone; the C write as a whole-structure round trip (caught by the repair case); truncation instead of narrowing; the setter's validation bypassed; the default instead of the live value; a landing not mirrored.

## R-08 The record's table rebuilt

**Decision**: `record.rs`'s `get`/`set` call the mapping by identifier for its 24 lines; the identity lines (host rate, seed) stay where they are; the text formatting functions unchanged; `golden-record-v1.json` byte-identical (asserted by its guard).

## R-09 The Swift wrapper

**Decision**: `public enum ControlReading { case switch(Bool), integer(UInt64), continuous(Float), choice(UInt32) }` with `init(_ c: RrDr60ControlReading)` and `var c: RrDr60ControlReading`, `init(defaultOf index: UInt32)` from `rr_dr60_control`, `ControlIndex(identifier:)` looping over `rr_dr60_control_count`; `RrDr60ControlReading: RRDR60Sized`. A test reads every default and round-trips a reading.

## R-10 The records and the figures

**Decision**: version 0.91.0 (`Cargo.toml` ×4, `RR_DR60_VERSION_MINOR` 91); the stage manifest's version line re-blessed; the control manifest unchanged; the guide paragraph (the product review's text) and the C block re-blessed; the CHANGELOG under Added; the README's C list; CLAUDE.md; `ci.yml`'s second promise step (f5a02452728f3d0687c033689266a5bbdf88c6db) and the Miri list; the C smoke's idle-cost prints for the new calls.
