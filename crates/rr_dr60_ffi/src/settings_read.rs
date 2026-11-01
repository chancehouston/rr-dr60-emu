//! The settings read and control access by index (spec 025): `rr_dr60_get_settings`, the
//! control reading, the field-local reads and writes of a settings structure by control index,
//! the live controls by index and the validator's field to its control.

use super::*;
use rr_dr60::control_map::{self, ControlReading};
use rr_dr60::controls::CONTROL_COUNT;

/// A control's value by its manifest index (spec 025 D2), filled by `rr_dr60_control_value`
/// and `rr_dr60_settings_control_value` and read by the two set functions. `struct_size` first
/// (spec 022's rule; 24 bytes). The value sits in the field of its `value_type`
/// (`RrDr60ControlValueType`): a switch as 0 or 1, an integer and a choice's engine value in
/// `int_value`, a continuous value in `float_value`; the other field and `reserved` are zero
/// on output and ignored on input. The structure holds integers and floats only, never a
/// `bool` or an enumeration, so every bit pattern a caller passes is a valid value.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RrDr60ControlReading {
    /// `sizeof(RrDr60ControlReading)`; set by the caller before every call.
    pub struct_size: u32,
    /// The value type, an `RrDr60ControlValueType` value.
    pub value_type: u32,
    /// A switch (0 or 1), an integer (exact over the whole `uint64_t` range: the seed) or a
    /// choice's engine value.
    pub int_value: u64,
    /// A continuous value, single precision as the engine stores it.
    pub float_value: f32,
    /// Zero on output; ignored on input.
    pub reserved: u32,
}
sized_default!(RrDr60ControlReading);

/// The reading's size (spec 025 R-02).
pub(crate) const READING_SIZE: u32 = size_of::<RrDr60ControlReading>() as u32;
/// The reading's published layouts (spec 022 D1).
pub(crate) const READING_PUBLISHED: &[u32] = &[READING_SIZE];

// Spec 025 R-02: the layout, pinned; no bool or enum field ever (a typed copy of a caller's
// reading is sound for every bit pattern).
const _: () = assert!(READING_SIZE == 24);
const _: () = assert!(core::mem::offset_of!(RrDr60ControlReading, value_type) == 4);
const _: () = assert!(core::mem::offset_of!(RrDr60ControlReading, int_value) == 8);
const _: () = assert!(core::mem::offset_of!(RrDr60ControlReading, float_value) == 16);
const _: () = assert!(core::mem::offset_of!(RrDr60ControlReading, reserved) == 20);

/// How a settings field is stored in `RrDr60Settings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CKind {
    /// A `bool` byte, read as `u8` (0 or 1).
    Switch,
    /// A `uint32_t` integer.
    Int32,
    /// A `uint64_t` integer.
    Int64,
    /// A `float`.
    Float,
    /// A `uint32_t` choice.
    Choice,
}

macro_rules! field {
    ($f:ident, $k:ident) => {
        Some((core::mem::offset_of!(RrDr60Settings, $f), CKind::$k))
    };
}

/// The C field of every control index, in the control manifest's order (`None` for the five
/// live controls). The harness's one-hot test (`us2_one_hot_c`) and `c_fields_follow_the_map`
/// tie it to the core's mapping.
pub(crate) const C_FIELDS: [Option<(usize, CKind)>; CONTROL_COUNT] = [
    None, // route
    None, // volume
    None, // playback_speed
    field!(host_rate_hz, Choice),
    field!(record_stage_enabled, Switch),
    field!(playback_stage_enabled, Switch),
    field!(adc_clamp_enabled, Switch),
    field!(tap, Choice),
    field!(seed, Int64),
    field!(agc_enabled, Switch),
    field!(agc_knee_dbfs, Float),
    field!(agc_attack_ms, Float),
    field!(agc_release_ms, Float),
    field!(vas_enabled, Switch),
    field!(vas_mode, Choice),
    field!(vas_sensitivity, Int32),
    field!(vas_threshold_dbfs, Float),
    field!(vas_hang_ms, Float),
    field!(vas_onset_ms, Float),
    field!(memory_capacity_s, Int32),
    field!(memory_max_recordings, Int32),
    field!(speech_codec_enabled, Switch),
    field!(output_volume_amp_enabled, Switch),
    field!(output_speaker_enabled, Switch),
    field!(output_earphone_enabled, Switch),
    field!(output_clamp_enabled, Switch),
    field!(mic_capsule_enabled, Switch),
    field!(mic_amp_enabled, Switch),
    field!(mic_hiss_offset_db, Float),
    None, // battery_volts
    None, // playback_speed_mode
];

/// The reading's C form, assigned field by field (spec 022 D10).
fn fill_reading(t: &mut RrDr60ControlReading, r: ControlReading) {
    let (ty, i, f) = match r {
        ControlReading::Switch(b) => (RrDr60ControlValueType::Switch, u64::from(b), 0.0),
        ControlReading::Integer(n) => (RrDr60ControlValueType::Integer, n, 0.0),
        ControlReading::Continuous(v) => (RrDr60ControlValueType::Continuous, 0, v),
        ControlReading::Choice(c) => (RrDr60ControlValueType::Choice, u64::from(c), 0.0),
    };
    t.value_type = ty as u32;
    t.int_value = i;
    t.float_value = f;
    t.reserved = 0;
}

/// The reading of a caller's C form, or `None` for an unknown type, a switch other than 0 or 1
/// or a choice above `u32` (the unused fields ignored).
fn reading_of(c: &RrDr60ControlReading) -> Option<ControlReading> {
    Some(match c.value_type {
        0 => ControlReading::Switch(match c.int_value {
            0 => false,
            1 => true,
            _ => return None,
        }),
        1 => ControlReading::Integer(c.int_value),
        2 => ControlReading::Continuous(c.float_value),
        3 => ControlReading::Choice(u32::try_from(c.int_value).ok()?),
        _ => return None,
    })
}

/// Copies a caller's reading (spec 025 R-03: size 24 … 4096, the first 24 bytes, before any
/// write, so the pointers may alias).
///
/// # Safety
///
/// `reading` is non-null and its first `struct_size` bytes readable.
unsafe fn copy_reading(
    reading: *const RrDr60ControlReading,
) -> Result<RrDr60ControlReading, RrDr60Status> {
    // SAFETY: the caller's contract.
    let size = unsafe { struct_size_at(reading) };
    if filled_size(size, READING_PUBLISHED).is_none() {
        return Err(RrDr60Status::InvalidArgument);
    }
    // SAFETY: at least 24 bytes readable; every bit pattern is a valid reading (integers and
    // floats only).
    Ok(unsafe { reading.read_unaligned() })
}

/// The control's class on a settings structure: `Ok(field)` or the refusal.
fn settings_field(index: u32) -> Result<(usize, CKind), RrDr60Status> {
    match C_FIELDS.get(index as usize) {
        None => Err(RrDr60Status::NoSuchControl),
        Some(None) => Err(RrDr60Status::NotASetting),
        Some(Some(f)) => Ok(*f),
    }
}

/// Writes the engine's settings into `*out` (spec 025 D1): the last value set or landed on any
/// stream for every setting; each stream uses a landed value from its next produced sample or
/// adopts it at its next start; a scheduled change that has not landed is not in it. A read
/// confirms a `rr_dr60_reconfigure` exactly; that a live value took effect is confirmed by its
/// change-log entry. `out->struct_size` must equal `sizeof(RrDr60Settings)`; the structure is
/// filled in place, its padding zero. Nothing is written on a refusal. Real-time safe.
///
/// # Safety
///
/// `pipeline` must be NULL or a live handle; `out` NULL or a writable `RrDr60Settings`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_get_settings(
    pipeline: *mut RrDr60Pipeline,
    out: *mut RrDr60Settings,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if unsafe { struct_size_at(out) } != SETTINGS_SIZE {
        return RrDr60Status::InvalidArgument;
    }
    let s = p.inner.settings();
    // SAFETY: out is non-null with `SETTINGS_SIZE` bytes writable (caller contract).
    match manifest_guard(|| unsafe {
        write_sized(out, &[SETTINGS_SIZE], |t, _| fill_settings(t, s))
    }) {
        Ok(status) => status,
        // coverage: unreachable, a field copy (Constitution III)
        Err(status) => status,
    }
}

/// Writes the current value of control `index` (the control manifest's order) into `*out`
/// (spec 025 D2): a setting as `rr_dr60_get_settings` reads it, a live control exactly as its
/// getter returns it (the last valid set or landed value, not the knob's position during a
/// transition or the level heard). Refusals in order, nothing written: a NULL or poisoned
/// handle, a NULL `out` (`NULL_POINTER`), a refused `struct_size` (`INVALID_ARGUMENT`), an
/// index at or past the count (`NO_SUCH_CONTROL`). Real-time safe.
///
/// # Safety
///
/// `pipeline` must be NULL or a live handle; `out` NULL or a writable `RrDr60ControlReading`
/// of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_control_value(
    pipeline: *mut RrDr60Pipeline,
    index: u32,
    out: *mut RrDr60ControlReading,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, READING_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let forced = take_forced_panic(p);
    let result = guarded(p, |inner| {
        if forced {
            panic!("rr_dr60__test_force_panic");
        }
        inner.control_value(index)
    });
    let r = match result {
        Ok(r) => r,
        Err(status) => return status,
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    match manifest_guard(|| unsafe {
        write_sized(out, READING_PUBLISHED, |t, _| fill_reading(t, r))
    }) {
        Ok(status) => status,
        // coverage: unreachable, a field copy (Constitution III)
        Err(status) => status,
    }
}

/// Reads setting `index` of a caller's settings structure into `*out` (spec 025 D3): only the
/// mapped field is read, through its offset, so a structure holding an invalid value elsewhere
/// can still be read. Refusals in order, nothing written: a NULL pointer (`NULL_POINTER`), a
/// settings `struct_size` other than `sizeof(RrDr60Settings)` or a refused reading size
/// (`INVALID_ARGUMENT`), an index at or past the count (`NO_SUCH_CONTROL`), a live control
/// (`NOT_A_SETTING`), a stored switch byte other than 0 or 1 (`INVALID_ARGUMENT`: the reading
/// cannot carry it). Every other stored value is returned as stored, so a repair screen can show
/// it: a choice that is not an option (`tap = 99`), a non-finite float or an integer wider than
/// the engine's field may then fail the write's representability check or the validator, which
/// names the field. `out` must not overlap `settings`. Takes no handle: safe on any thread with
/// its own buffers. Real-time safe.
///
/// # Safety
///
/// `settings` must be NULL or readable for `sizeof(RrDr60Settings)` bytes; `out` NULL or a
/// writable `RrDr60ControlReading` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_settings_control_value(
    settings: *const RrDr60Settings,
    index: u32,
    out: *mut RrDr60ControlReading,
) -> RrDr60Status {
    if settings.is_null() || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: both non-null, their first four bytes readable (caller contract).
    if unsafe { struct_size_at(settings) } != SETTINGS_SIZE
        || !unsafe { report_ok(out, READING_PUBLISHED) }
    {
        return RrDr60Status::InvalidArgument;
    }
    let (offset, kind) = match settings_field(index) {
        Ok(f) => f,
        Err(status) => return status,
    };
    // SAFETY: `settings` is readable for the whole structure; the field is read at its offset
    // without forming a reference to the structure, unaligned (the caller's buffer may be).
    let at = unsafe { settings.cast::<u8>().add(offset) };
    // SAFETY: as above; each read has the field's own width.
    let r = unsafe {
        match kind {
            CKind::Switch => match at.read() {
                0 => ControlReading::Switch(false),
                1 => ControlReading::Switch(true),
                _ => return RrDr60Status::InvalidArgument,
            },
            CKind::Int32 => ControlReading::Integer(u64::from(at.cast::<u32>().read_unaligned())),
            CKind::Int64 => ControlReading::Integer(at.cast::<u64>().read_unaligned()),
            CKind::Float => ControlReading::Continuous(at.cast::<f32>().read_unaligned()),
            CKind::Choice => ControlReading::Choice(at.cast::<u32>().read_unaligned()),
        }
    };
    // Every other stored value is returned as stored (hardware review B1).
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    match manifest_guard(|| unsafe {
        write_sized(out, READING_PUBLISHED, |t, _| fill_reading(t, r))
    }) {
        Ok(status) => status,
        // coverage: unreachable, a field copy (Constitution III)
        Err(status) => status,
    }
}

/// Writes setting `index` of a caller's settings structure from `*reading` (spec 025 D3):
/// only the mapped field is written, through its offset (the reserved slots, the padding and
/// every other field untouched, so a structure with an invalid value elsewhere can be repaired
/// by index); the reading is copied first (the pointers may alias). The write checks
/// representability only: the reading's value type is the control's, a switch is 0 or 1, a
/// choice one of the options, a continuous value finite (−0.0 written as 0.0), an integer
/// within the engine's field (`vas.sensitivity` 0 … 255, the memory fields `uint32_t`, the seed
/// `uint64_t`); ranges are `rr_dr60_settings_validate`'s, which names the field. Refusals in
/// order, nothing written: a NULL pointer (`NULL_POINTER`), a settings `struct_size` other than
/// `sizeof(RrDr60Settings)` or a reading size below 24 or above `RR_DR60_MAX_STRUCT_SIZE`
/// (`INVALID_ARGUMENT`), an index at or past the count (`NO_SUCH_CONTROL`), a live control
/// (`NOT_A_SETTING`), an unrepresentable reading (`INVALID_ARGUMENT`). Takes no handle: safe on
/// any thread with its own buffers. Real-time safe.
///
/// # Safety
///
/// `settings` must be NULL or readable and writable for `sizeof(RrDr60Settings)` bytes;
/// `reading` NULL or readable for its `struct_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_set_settings_control_value(
    settings: *mut RrDr60Settings,
    index: u32,
    reading: *const RrDr60ControlReading,
) -> RrDr60Status {
    if settings.is_null() || reading.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null, its first four bytes readable (caller contract).
    if unsafe { struct_size_at(settings) } != SETTINGS_SIZE {
        return RrDr60Status::InvalidArgument;
    }
    // SAFETY: non-null, readable for its `struct_size` (caller contract).
    let c = match unsafe { copy_reading(reading) } {
        Ok(c) => c,
        Err(status) => return status,
    };
    let (offset, kind) = match settings_field(index) {
        Ok(f) => f,
        Err(status) => return status,
    };
    let Some(r) = reading_of(&c) else {
        return RrDr60Status::InvalidArgument;
    };
    // The core's representability check and normalization (−0.0 → 0.0) on a probe.
    let checked = manifest_guard(|| {
        let mut probe = Settings::new(rr_dr60::DEVICE_RATE_HZ);
        control_map::set_settings_value(&mut probe, index, r)
            .and_then(|()| control_map::settings_value(&probe, index))
    });
    let v = match checked {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => return RrDr60Status::InvalidArgument,
        // coverage: unreachable, a field copy (Constitution III)
        Err(status) => return status,
    };
    // SAFETY: `settings` is writable for the whole structure; the field is written at its
    // offset without forming a reference to the structure, unaligned, in its own width.
    let at = unsafe { settings.cast::<u8>().add(offset) };
    // SAFETY: as above.
    unsafe {
        match (kind, v) {
            (CKind::Switch, ControlReading::Switch(b)) => at.write(u8::from(b)),
            (CKind::Int32, ControlReading::Integer(n)) => {
                // The core refused anything wider than the engine's field (`u8` or `u32`).
                at.cast::<u32>().write_unaligned(n as u32);
            }
            (CKind::Int64, ControlReading::Integer(n)) => at.cast::<u64>().write_unaligned(n),
            (CKind::Float, ControlReading::Continuous(x)) => at.cast::<f32>().write_unaligned(x),
            (CKind::Choice, ControlReading::Choice(x)) => at.cast::<u32>().write_unaligned(x),
            // coverage: unreachable, the table follows the map (`c_fields_follow_the_map`)
            _ => return RrDr60Status::InternalError,
        }
    }
    RrDr60Status::Ok
}

/// Sets live control `index` from `*reading` through its plain setter (spec 025 D4): route,
/// volume, playback speed, battery voltage or speed mode, with the setter's validation and
/// timing (the same entry in the change log as the plain setter). Refusals in order, nothing
/// changed: a NULL or poisoned handle, a NULL `reading` (`NULL_POINTER`), a reading size below
/// 24 or above `RR_DR60_MAX_STRUCT_SIZE` (`INVALID_ARGUMENT`), an index at or past the count
/// (`NO_SUCH_CONTROL`), a settings field (`NOT_A_PLAIN_LIVE_CONTROL`: change those with a
/// settings structure and `rr_dr60_reconfigure` or with `rr_dr60_schedule_change`), an
/// unrepresentable reading (a wrong type, a non-option, a non-finite value, a level above 255:
/// `INVALID_ARGUMENT`; so a NaN volume by index is `INVALID_ARGUMENT` where
/// `rr_dr60_set_volume` returns `INVALID_VOLUME`), then the setter's `INVALID_VOLUME`,
/// `INVALID_SPEED_LEVEL` or `INVALID_BATTERY_VOLTS`. Real-time safe.
///
/// # Safety
///
/// `pipeline` must be NULL or a live handle; `reading` NULL or readable for its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_set_control_value(
    pipeline: *mut RrDr60Pipeline,
    index: u32,
    reading: *const RrDr60ControlReading,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if reading.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null, readable for its `struct_size` (caller contract).
    let c = match unsafe { copy_reading(reading) } {
        Ok(c) => c,
        Err(status) => return status,
    };
    match C_FIELDS.get(index as usize) {
        None => return RrDr60Status::NoSuchControl,
        Some(Some(_)) => return RrDr60Status::NotAPlainLiveControl,
        Some(None) => {}
    }
    let Some(r) = reading_of(&c) else {
        return RrDr60Status::InvalidArgument;
    };
    let forced = take_forced_panic(p);
    let result = guarded(p, |inner| {
        if forced {
            panic!("rr_dr60__test_force_panic");
        }
        inner.set_control_value(index, r)
    });
    match result {
        Ok(()) => RrDr60Status::Ok,
        Err(status) => status,
    }
}

/// The control index of a field `rr_dr60_settings_validate` names (spec 025 D6), so an app
/// highlights the row the validator refused. `field` is an `RrDr60SettingField` value passed as
/// `uint32_t`: `NONE`, `STRUCT_SIZE`, the retired 5 and 6 and any unknown value are
/// `NO_SUCH_CONTROL`; a NULL `out_index` is `NULL_POINTER`. Real-time safe.
///
/// # Safety
///
/// `out_index` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_setting_field_control(
    field: u32,
    out_index: *mut u32,
) -> RrDr60Status {
    if out_index.is_null() {
        return RrDr60Status::NullPointer;
    }
    // Constant data throughout (compile-time indices): no search, no panic path.
    let index = match field {
        x if x == RrDr60SettingField::HostRate as u32 => Some(control_map::HOST_RATE_CONTROL),
        x if x == RrDr60SettingField::Tap as u32 => Some(control_map::TAP_CONTROL),
        x if x == RrDr60SettingField::VasMode as u32 => Some(control_map::VAS_MODE_CONTROL),
        x => setting_of_field(x).map(control_map::setting_control),
    };
    match index {
        Some(i) => {
            // SAFETY: non-null and writable (caller contract).
            unsafe { out_index.write_unaligned(i) };
            RrDr60Status::Ok
        }
        None => RrDr60Status::NoSuchControl,
    }
}

/// The core field of a range-checked `RrDr60SettingField` value.
fn setting_of_field(field: u32) -> Option<Setting> {
    use RrDr60SettingField as F;
    Some(match field {
        x if x == F::AgcKneeDbfs as u32 => Setting::AgcKneeDbfs,
        x if x == F::AgcAttackMs as u32 => Setting::AgcAttackMs,
        x if x == F::AgcReleaseMs as u32 => Setting::AgcReleaseMs,
        x if x == F::VasSensitivity as u32 => Setting::VasSensitivity,
        x if x == F::VasThresholdDbfs as u32 => Setting::VasThresholdDbfs,
        x if x == F::VasHangMs as u32 => Setting::VasHangMs,
        x if x == F::VasOnsetMs as u32 => Setting::VasOnsetMs,
        x if x == F::MemoryCapacityS as u32 => Setting::MemoryCapacityS,
        x if x == F::MemoryMaxRecordings as u32 => Setting::MemoryMaxRecordings,
        x if x == F::MicHissOffsetDb as u32 => Setting::MicHissOffsetDb,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rr_dr60::controls::controls;

    /// The C table follows the core's mapping: a write by index through C, converted whole,
    /// reads back by index in Rust (and the live entries are exactly the core's).
    #[test]
    fn c_fields_follow_the_map() {
        let base = Settings::new(48_000);
        for (i, f) in C_FIELDS.iter().enumerate() {
            let i = i as u32;
            let core = control_map::settings_value(&base, i);
            assert_eq!(f.is_none(), core.is_err(), "index {i}");
            if f.is_none() {
                continue;
            }
            let mut c = rr_dr60_settings_default(48_000);
            let mut out = RrDr60ControlReading::default();
            // SAFETY: valid pointers to local values.
            assert_eq!(
                unsafe { rr_dr60_settings_control_value(&c, i, &mut out) },
                RrDr60Status::Ok
            );
            assert_eq!(reading_of(&out), Some(core.unwrap()), "default of {i}");
            // A different representable value.
            let new = match core.unwrap() {
                ControlReading::Switch(b) => ControlReading::Switch(!b),
                ControlReading::Integer(n) => ControlReading::Integer(n + 1),
                ControlReading::Continuous(v) => ControlReading::Continuous(v + 0.25),
                ControlReading::Choice(_) if i == 3 => ControlReading::Choice(6000),
                ControlReading::Choice(_) => ControlReading::Choice(1),
            };
            let mut r = RrDr60ControlReading::default();
            fill_reading(&mut r, new);
            // SAFETY: valid pointers to local values.
            assert_eq!(
                unsafe { rr_dr60_set_settings_control_value(&mut c, i, &r) },
                RrDr60Status::Ok,
                "{i}"
            );
            let s = to_settings(&c).unwrap_or_else(|_| panic!("{i}"));
            assert_eq!(
                control_map::settings_value(&s, i).unwrap(),
                new,
                "index {i}"
            );
        }
    }

    #[test]
    fn reading_decode() {
        let mut r = RrDr60ControlReading {
            value_type: 0,
            int_value: 2,
            ..Default::default()
        };
        assert_eq!(reading_of(&r), None);
        r.int_value = 1;
        r.float_value = 9.0; // ignored
        r.reserved = 7; // ignored
        assert_eq!(reading_of(&r), Some(ControlReading::Switch(true)));
        r.value_type = 3;
        r.int_value = u64::from(u32::MAX) + 1;
        assert_eq!(reading_of(&r), None);
        r.value_type = 4;
        assert_eq!(reading_of(&r), None);
    }

    #[test]
    fn field_lookup() {
        let mut out = 0u32;
        for f in 0..=20u32 {
            // SAFETY: a local out value.
            let st = unsafe { rr_dr60_setting_field_control(f, &mut out) };
            let expect = match f {
                1 => Some("host_rate_hz"),
                2 => Some("tap"),
                4 => Some("agc.knee_dbfs"),
                7 => Some("agc.attack_ms"),
                8 => Some("agc.release_ms"),
                9 => Some("vas.mode"),
                10 => Some("vas.sensitivity"),
                11 => Some("vas.threshold_dbfs"),
                12 => Some("vas.hang_ms"),
                13 => Some("vas.onset_ms"),
                14 => Some("memory.capacity_s"),
                15 => Some("memory.max_recordings"),
                16 => Some("mic.hiss_offset_db"),
                _ => None,
            };
            match expect {
                Some(id) => {
                    assert_eq!(st, RrDr60Status::Ok, "{f}");
                    assert_eq!(controls()[out as usize].identifier, id);
                }
                None => assert_eq!(st, RrDr60Status::NoSuchControl, "{f}"),
            }
        }
        // SAFETY: a NULL out pointer is refused.
        assert_eq!(
            unsafe { rr_dr60_setting_field_control(4, core::ptr::null_mut()) },
            RrDr60Status::NullPointer
        );
    }

    /// The whole-structure round-trip mutant (research R-07): convert, set, convert back. It is
    /// caught by the repair case (a structure with `tap = 99` cannot be converted) and by the
    /// painted reserved bytes it zeroes.
    fn whole_struct_mutant(s: &mut RrDr60Settings, index: u32, r: ControlReading) -> RrDr60Status {
        let Ok(mut core) = to_settings(s) else {
            return RrDr60Status::InvalidArgument;
        };
        if control_map::set_settings_value(&mut core, index, r).is_err() {
            return RrDr60Status::InvalidArgument;
        }
        fill_settings(s, &core);
        RrDr60Status::Ok
    }

    #[test]
    fn whole_struct_round_trip_mutant_is_caught() {
        let knee = ControlReading::Continuous(-3.0);
        let mut real = rr_dr60_settings_default(48_000);
        real.tap = 99;
        let mut r = RrDr60ControlReading::default();
        fill_reading(&mut r, ControlReading::Choice(5));
        // SAFETY: local values.
        assert_eq!(
            unsafe { rr_dr60_set_settings_control_value(&mut real, 7, &r) },
            RrDr60Status::Ok
        );
        assert_eq!(real.tap, 5, "the repair by index");
        let mut broken = rr_dr60_settings_default(48_000);
        broken.tap = 99;
        assert_eq!(
            whole_struct_mutant(&mut broken, 7, ControlReading::Choice(5)),
            RrDr60Status::InvalidArgument
        );
        // The reserved slots: the real write keeps them, the mutant zeroes them.
        let mut a = rr_dr60_settings_default(48_000);
        a.reserved_008 = 0xDEAD;
        let mut b = a;
        fill_reading(&mut r, knee);
        // SAFETY: local values.
        assert_eq!(
            unsafe { rr_dr60_set_settings_control_value(&mut a, 10, &r) },
            RrDr60Status::Ok
        );
        assert_eq!(whole_struct_mutant(&mut b, 10, knee), RrDr60Status::Ok);
        assert_eq!(a.reserved_008, 0xDEAD);
        assert_ne!(b.reserved_008, 0xDEAD);
    }

    /// A panic inside the engine call is caught, the status `INTERNAL_ERROR`, the handle
    /// poisoned (spec 025 D9; coach review 1).
    #[test]
    #[cfg(feature = "ffi-test-panic")]
    fn engine_calls_are_panic_guarded() {
        for which in 0..2 {
            let s = rr_dr60_settings_default(48_000);
            let mut h: *mut RrDr60Pipeline = core::ptr::null_mut();
            // SAFETY: valid pointers.
            assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
            let mut r = RrDr60ControlReading::default();
            fill_reading(&mut r, ControlReading::Continuous(0.5));
            // SAFETY: a live handle and local values throughout.
            unsafe {
                rr_dr60__test_force_panic(h);
                let st = if which == 0 {
                    rr_dr60_control_value(h, 1, &mut r)
                } else {
                    rr_dr60_set_control_value(h, 1, &r)
                };
                assert_eq!(st, RrDr60Status::InternalError, "{which}");
                let mut v = 0.0f32;
                assert_eq!(
                    rr_dr60_get_volume(h, &mut v),
                    RrDr60Status::InternalError,
                    "poisoned"
                );
                rr_dr60_destroy(h);
            }
        }
    }
}
