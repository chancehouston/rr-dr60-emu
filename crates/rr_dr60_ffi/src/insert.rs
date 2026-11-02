//! The insertable component slot (spec 024 D9; research R-09): the slot's component and its
//! parameter store on a handle, the insertion points and components as constant data, the
//! band filter's frequency ceiling, and the processing record's component (form 2). Every
//! struct is new and sized under spec 022's rule; no existing struct changes (D9). The slot and
//! every component are additions of the emulator, not parts of the original recorder.

use super::*;
use rr_dr60::components::{self as comp, ComponentId, InsertionPointId};

/// `RR_DR60_INSERTION_POINT_BEFORE_LIMITER`: between stage 2 and the limiter (spec 024 D1).
pub const RR_DR60_INSERTION_POINT_BEFORE_LIMITER: u32 = 0; // 024 D9
/// `RR_DR60_COMPONENT_NONE`: the slot empty.
pub const RR_DR60_COMPONENT_NONE: u32 = 0; // 024 D9
/// `RR_DR60_COMPONENT_BAND_FILTER`: the band filter.
pub const RR_DR60_COMPONENT_BAND_FILTER: u32 = 1; // 024 D9
/// The band filter's types (`band_filter.type`): low-pass.
pub const RR_DR60_BAND_FILTER_LOW_PASS: u32 = 0; // 024 D4
/// High-pass.
pub const RR_DR60_BAND_FILTER_HIGH_PASS: u32 = 1; // 024 D4
/// Band-pass (a 0 dB peak).
pub const RR_DR60_BAND_FILTER_BAND_PASS: u32 = 2; // 024 D4
/// Notch.
pub const RR_DR60_BAND_FILTER_NOTCH: u32 = 3; // 024 D4
/// The parameters of all components together in the global store (spec 024 D5).
pub const RR_DR60_COMPONENT_PARAMETER_BUDGET: u32 = 16; // 024 D5: equal to the core's (a literal so cbindgen emits the define; the unit test pins it)
/// `RrDr60ComponentParameter.scale`: linear.
pub const RR_DR60_SCALE_LINEAR: u32 = 0; // 024 R-04
/// Logarithmic.
pub const RR_DR60_SCALE_LOG: u32 = 1; // 024 R-04
/// In decibels.
pub const RR_DR60_SCALE_DECIBEL: u32 = 2; // 024 R-04

/// One insertion point (spec 024 D1), filled by `rr_dr60_insertion_point`. `struct_size` first.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60InsertionPoint {
    /// `sizeof(RrDr60InsertionPoint)`; read back as the filled size.
    pub struct_size: u32,
    /// The point's value (`RR_DR60_INSERTION_POINT_*`).
    pub value: u32,
    /// The identifier, e.g. `before_limiter` (static, NUL-terminated).
    pub identifier: *const c_char,
    /// The name.
    pub name: *const c_char,
    /// Where it sits, one sentence.
    pub placement: *const c_char,
    /// The paths it serves (1 the record side).
    pub paths: u32,
    /// The control manifest's index of the slot's control (31).
    pub control_index: u32,
}
sized_default!(RrDr60InsertionPoint);

/// One component (spec 024 D2), filled by `rr_dr60_component`. `struct_size` first.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60Component {
    /// `sizeof(RrDr60Component)`; read back as the filled size.
    pub struct_size: u32,
    /// The component's value (`RR_DR60_COMPONENT_*`, never NONE).
    pub value: u32,
    /// The identifier, e.g. `band_filter`.
    pub identifier: *const c_char,
    /// The name.
    pub name: *const c_char,
    /// What it does to the sound, one sentence.
    pub description: *const c_char,
    /// The latency it adds, in host samples (0 for every 0.92 component).
    pub latency_samples: u32,
    /// The paths it serves (1 the record side).
    pub paths: u32,
    /// Its parameters' base index in the global store.
    pub parameter_offset: u32,
    /// Its parameter count.
    pub parameter_count: u32,
}
sized_default!(RrDr60Component);

/// One parameter of a component (spec 024 D2), filled by `rr_dr60_component_parameter`.
/// `struct_size` first. A choice's options are read with `rr_dr60_control_option` at
/// `control_index`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60ComponentParameter {
    /// `sizeof(RrDr60ComponentParameter)`; read back as the filled size.
    pub struct_size: u32,
    /// The value type, an `RrDr60ControlValueType` value (choice or continuous).
    pub value_type: u32,
    /// The control manifest identifier, e.g. `band_filter.frequency_hz`.
    pub identifier: *const c_char,
    /// The name.
    pub name: *const c_char,
    /// The unit, or empty.
    pub unit: *const c_char,
    /// The inclusive minimum (a choice: 0).
    pub min: f32,
    /// The inclusive maximum (a choice: the last option's value).
    pub max: f32,
    /// The default.
    pub default_value: f32,
    /// How its transition moves (`RR_DR60_SCALE_*`).
    pub scale: u32,
    /// The control manifest's index.
    pub control_index: u32,
    /// Zero.
    pub reserved: u32,
}
sized_default!(RrDr60ComponentParameter);

/// The slot of a processing record (spec 024 D7), filled by
/// `rr_dr60_processing_record_component` and read by `rr_dr60_record_write_with_component`.
/// `struct_size` first. `parameter_count` 0 (a version-1 record, or a write without it) means
/// an empty slot at the defaults.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RrDr60RecordComponent {
    /// `sizeof(RrDr60RecordComponent)`; read back as the filled size.
    pub struct_size: u32,
    /// The slot's component at the recording's start (`RR_DR60_COMPONENT_*`).
    pub component: u32,
    /// The store's values written (16 on a read; 0 or 16 on a write).
    pub parameter_count: u32,
    /// Zero.
    pub reserved: u32,
    /// The global parameter store at the recording's start (the band filter's type, frequency,
    /// Q and gain at 0 – 3).
    pub store: [f32; 16],
}
sized_default!(RrDr60RecordComponent);

pub(crate) const POINT_PUBLISHED: &[u32] = &[size_of::<RrDr60InsertionPoint>() as u32];
pub(crate) const COMPONENT_PUBLISHED: &[u32] = &[size_of::<RrDr60Component>() as u32];
pub(crate) const PARAMETER_PUBLISHED: &[u32] = &[size_of::<RrDr60ComponentParameter>() as u32];
pub(crate) const RECORD_COMPONENT_SIZE: u32 = size_of::<RrDr60RecordComponent>() as u32;
pub(crate) const RECORD_COMPONENT_PUBLISHED: &[u32] = &[RECORD_COMPONENT_SIZE];

const _: () =
    assert!(RR_DR60_COMPONENT_PARAMETER_BUDGET as usize == comp::COMPONENT_PARAMETER_BUDGET);
const _: () = assert!(RECORD_COMPONENT_SIZE == 80);

// The strings with a trailing NUL (a test pins them to the core's).
/// An identifier, a name and a third string (a placement, a description or a unit), NUL-ended.
type CStrings = (&'static [u8], &'static [u8], &'static [u8]);
const POINT_STRINGS: [CStrings; 1] = [(
    b"before_limiter\0",
    b"Before the limiter\0",
    b"Between stage 2, the microphone amplifier, and stage 3, the limiter, at the host rate; an addition of the emulator, the original recorder has no place for one.\0",
)];
const COMPONENT_STRINGS: [CStrings; 1] = [(
    b"band_filter\0",
    b"Band filter\0",
    "One second-order filter, low-pass, high-pass, band-pass or notch, at a chosen frequency and Q, then a gain of −24 to +24 dB; an addition of the emulator, not a part of the original recorder.\0".as_bytes(),
)];
const PARAMETER_STRINGS: [[CStrings; 4]; 1] = [[
    (b"band_filter.type\0", b"Band filter type\0", b"\0"),
    (
        b"band_filter.frequency_hz\0",
        b"Band filter frequency\0",
        b"Hz\0",
    ),
    (b"band_filter.q\0", b"Band filter Q\0", b"\0"),
    (b"band_filter.gain_db\0", b"Band filter gain\0", b"dB\0"),
]];

fn point_of(v: u32) -> Result<InsertionPointId, RrDr60Status> {
    InsertionPointId::from_u32(v).ok_or(RrDr60Status::NoSuchInsertionPoint)
}

/// Puts `component` into the slot at `point`, or empties it with `RR_DR60_COMPONENT_NONE`
/// (spec 024 D3): only while idle (`RR_DR60_STATUS_COMPONENT_WHILE_ACTIVE` while recording or
/// playing); never a change entry; kept by reset and reconfigure; never allocates. A change
/// clears the filter's state; the parameter store is kept. On the one-pass the change is
/// immediate: fade or mute around it. Not real-time safe in the sense of a live control: call
/// it between recordings.
///
/// # Safety
///
/// `pipeline` must be NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_set_component(
    pipeline: *mut RrDr60Pipeline,
    point: u32,
    component: u32,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    let point = match point_of(point) {
        Ok(v) => v,
        Err(status) => return status,
    };
    let Some(c) = ComponentId::from_u32(component) else {
        return RrDr60Status::NoSuchComponent;
    };
    match guarded(p, |inner| inner.set_component(point, c)) {
        Ok(()) => RrDr60Status::Ok,
        Err(status) => status,
    }
}

/// Writes the slot's component at `point` into `*out_component` (spec 024 D3). Real-time safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out_component` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_get_component(
    pipeline: *mut RrDr60Pipeline,
    point: u32,
    out_component: *mut u32,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out_component.is_null() {
        return RrDr60Status::NullPointer;
    }
    let point = match point_of(point) {
        Ok(v) => v,
        Err(status) => return status,
    };
    match guarded(p, |inner| Ok(inner.component(point))) {
        // SAFETY: writable (caller contract).
        Ok(c) => unsafe {
            *out_component = c as u32;
            RrDr60Status::Ok
        },
        Err(status) => status,
    }
}

/// Writes parameter `index` of the global store (the band filter's type 0, frequency 1, Q 2,
/// gain 3) while idle, with no change entry (spec 024 plan R1): the restore path a re-render
/// takes. Validated as the schedule validates it (`RR_DR60_STATUS_INVALID_ARGUMENT` out of range
/// or not an option; `RR_DR60_STATUS_NO_SUCH_COMPONENT` past the store's parameters);
/// `RR_DR60_STATUS_COMPONENT_WHILE_ACTIVE` while recording or playing. To change a parameter
/// during a recording, schedule it with `rr_dr60_schedule_change` (controls 32 – 35).
///
/// # Safety
///
/// `pipeline` must be NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_set_component_parameter(
    pipeline: *mut RrDr60Pipeline,
    point: u32,
    index: u32,
    value: f64,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    let point = match point_of(point) {
        Ok(v) => v,
        Err(status) => return status,
    };
    match guarded(p, |inner| {
        inner.set_component_parameter(point, index, value)
    }) {
        Ok(()) => RrDr60Status::Ok,
        Err(status) => status,
    }
}

/// Writes parameter `index` of the store into `*out_value` (spec 024 D3): the most recently set
/// or landed value (the target during a transition). Real-time safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out_value` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_component_parameter_value(
    pipeline: *mut RrDr60Pipeline,
    point: u32,
    index: u32,
    out_value: *mut f32,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out_value.is_null() {
        return RrDr60Status::NullPointer;
    }
    let point = match point_of(point) {
        Ok(v) => v,
        Err(status) => return status,
    };
    match guarded(p, |inner| inner.component_parameter_value(point, index)) {
        // SAFETY: writable (caller contract).
        Ok(v) => unsafe {
            *out_value = v;
            RrDr60Status::Ok
        },
        Err(status) => status,
    }
}

/// Writes the band filter's applied frequency ceiling at `host_rate_hz` into `*out_hz` (spec 024
/// D4: 0.45 × the rate); `RR_DR60_STATUS_UNSUPPORTED_HOST_RATE` for a rate the engine does not
/// support. Needs no handle; real-time safe.
///
/// # Safety
///
/// `out_hz` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_band_filter_frequency_ceiling_hz(
    host_rate_hz: u32,
    out_hz: *mut f64,
) -> RrDr60Status {
    if out_hz.is_null() {
        return RrDr60Status::NullPointer;
    }
    if !rr_dr60::SUPPORTED_HOST_RATES.contains(&host_rate_hz) {
        return RrDr60Status::UnsupportedHostRate;
    }
    // SAFETY: writable (caller contract).
    unsafe { *out_hz = comp::band_filter_frequency_ceiling_hz(host_rate_hz) };
    RrDr60Status::Ok
}

/// The number of insertion points (spec 024 D1: 1). Real-time safe.
#[unsafe(no_mangle)]
pub extern "C" fn rr_dr60_insertion_point_count() -> u32 {
    comp::insertion_points().len() as u32
}

/// Copies insertion point `index` (0-based) into `*out` (spec 024 D1);
/// `RR_DR60_STATUS_NO_SUCH_INSERTION_POINT` past the list. Real-time safe.
///
/// # Safety
///
/// `out` must be NULL or point to a writable `RrDr60InsertionPoint` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_insertion_point(
    index: u32,
    out: *mut RrDr60InsertionPoint,
) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, POINT_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let Some(pt) = comp::insertion_points().get(index as usize) else {
        return RrDr60Status::NoSuchInsertionPoint;
    };
    let (id, name, placement) = POINT_STRINGS[index as usize];
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, POINT_PUBLISHED, |t, _| {
            t.value = pt.id as u32;
            t.identifier = manifest_cstr(id);
            t.name = manifest_cstr(name);
            t.placement = manifest_cstr(placement);
            t.paths = pt.paths;
            t.control_index = pt.control_index;
        })
    }
}

/// The number of components (spec 024 D2: 1; none is not counted). Real-time safe.
#[unsafe(no_mangle)]
pub extern "C" fn rr_dr60_component_count() -> u32 {
    comp::components().len() as u32
}

/// Copies component `index` (0-based over the components; each carries its value) into `*out`
/// (spec 024 D2); `RR_DR60_STATUS_NO_SUCH_COMPONENT` past the list. Real-time safe.
///
/// # Safety
///
/// `out` must be NULL or point to a writable `RrDr60Component` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_component(index: u32, out: *mut RrDr60Component) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, COMPONENT_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let Some(c) = comp::components().get(index as usize) else {
        return RrDr60Status::NoSuchComponent;
    };
    let (id, name, desc) = COMPONENT_STRINGS[index as usize];
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, COMPONENT_PUBLISHED, |t, _| {
            t.value = c.id as u32;
            t.identifier = manifest_cstr(id);
            t.name = manifest_cstr(name);
            t.description = manifest_cstr(desc);
            t.latency_samples = c.latency_samples;
            t.paths = c.paths;
            t.parameter_offset = c.parameter_offset;
            t.parameter_count = c.parameters.len() as u32;
        })
    }
}

/// Copies parameter `index` of component `component_index` (both 0-based) into `*out` (spec 024
/// D2); `RR_DR60_STATUS_NO_SUCH_COMPONENT` past either list. Real-time safe.
///
/// # Safety
///
/// `out` must be NULL or point to a writable `RrDr60ComponentParameter` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_component_parameter(
    component_index: u32,
    index: u32,
    out: *mut RrDr60ComponentParameter,
) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, PARAMETER_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let Some(c) = comp::components().get(component_index as usize) else {
        return RrDr60Status::NoSuchComponent;
    };
    let Some(prm) = c.parameters.get(index as usize) else {
        return RrDr60Status::NoSuchComponent;
    };
    let (id, name, unit) = PARAMETER_STRINGS[component_index as usize][index as usize];
    let (ty, min, max, def) = match prm.value {
        rr_dr60::ControlValue::Choice { options, default } => (
            RrDr60ControlValueType::Choice,
            0.0,
            options.last().map_or(0.0, |o| o.value as f32),
            default.unwrap_or(0) as f32,
        ),
        rr_dr60::ControlValue::Continuous { min, max, default } => {
            (RrDr60ControlValueType::Continuous, min, max, default)
        }
        // coverage: unreachable, the components have choices and continuous values only
        _ => (RrDr60ControlValueType::Continuous, 0.0, 0.0, 0.0),
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, PARAMETER_PUBLISHED, |t, _| {
            t.value_type = ty as u32;
            t.identifier = manifest_cstr(id);
            t.name = manifest_cstr(name);
            t.unit = manifest_cstr(unit);
            t.min = min;
            t.max = max;
            t.default_value = def;
            t.scale = prm.scale as u32;
            t.control_index = prm.control_index;
        })
    }
}

/// Reads a processing record's slot into `*out` (spec 024 D7): the component and the store at
/// the recording's start; a version-1 record reads as an empty slot at the defaults. The record
/// is read and refused exactly as `rr_dr60_processing_record_read` refuses it. Needs no handle.
///
/// # Safety
///
/// `bytes` readable for `len`; `out` NULL or a writable `RrDr60RecordComponent` of its size.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_processing_record_component(
    bytes: *const u8,
    len: usize,
    out: *mut RrDr60RecordComponent,
) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: the caller's contract.
    let bytes = match unsafe { bytes_in(bytes, len) } {
        Ok(b) => b,
        Err(status) => return status,
    };
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, RECORD_COMPONENT_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let mut r = RecordReadout::default();
    match catch_unwind(AssertUnwindSafe(|| record::read(bytes, &mut r))) {
        // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
        Ok(Ok(())) => unsafe {
            write_sized(out, RECORD_COMPONENT_PUBLISHED, |t, _| {
                t.component = r.component as u32;
                t.parameter_count = comp::COMPONENT_PARAMETER_BUDGET as u32;
                t.store = r.store;
            })
        },
        Ok(Err(e)) => status_from_error(e),
        Err(_) => panic_to_status(None),
    }
}

/// The pure writer with the slot (spec 024 D7): as `rr_dr60_record_write`, with the record's
/// component and store from `*component` (`parameter_count` 0: an empty slot at the defaults).
/// `rr_dr60_record_write` itself writes an empty slot.
///
/// # Safety
///
/// As `rr_dr60_record_write`; `component` readable for its `struct_size` (at least 80 bytes).
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rr_dr60_record_write_with_component(
    readout: *const RrDr60RecordReadout,
    component: *const RrDr60RecordComponent,
    events: *const RrDr60VasEvent,
    host_input_positions: *const u64,
    event_count: usize,
    changes: *const RrDr60ChangeEntry,
    change_count: usize,
    out: *mut u8,
    out_len: usize,
    out_written: *mut usize,
) -> RrDr60Status {
    if readout.is_null()
        || component.is_null()
        || out_written.is_null()
        || (out.is_null() && out_len != 0)
    {
        return RrDr60Status::NullPointer;
    }
    if (events.is_null() || host_input_positions.is_null()) && event_count != 0 {
        return RrDr60Status::NullPointer;
    }
    if changes.is_null() && change_count != 0 {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null; the caller's contract on the sizes.
    let (rs, cs) = unsafe { (struct_size_at(readout), struct_size_at(component)) };
    if !(READOUT_SIZE..=RR_DR60_MAX_STRUCT_SIZE).contains(&rs)
        || !(RECORD_COMPONENT_SIZE..=RR_DR60_MAX_STRUCT_SIZE).contains(&cs)
    {
        return RrDr60Status::InvalidArgument;
    }
    // SAFETY: a valid read-out of at least its size.
    let mut r = match unsafe { (*readout).to_core() } {
        Ok(r) => r,
        Err(status) => return status,
    };
    // SAFETY: readable for at least 80 bytes (checked); the struct holds integers and floats
    // only, so every bit pattern is a valid value.
    let c = unsafe { component.read_unaligned() };
    if c.parameter_count != 0 {
        let Some(id) = ComponentId::from_u32(c.component) else {
            return RrDr60Status::NoSuchComponent;
        };
        for (i, v) in c.store.iter().enumerate() {
            if let Some((_, prm)) = comp::components().iter().find_map(|k| {
                let base = k.parameter_offset as usize;
                (i >= base && i < base + k.parameters.len()).then(|| (k, &k.parameters[i - base]))
            }) {
                if !matches!(component_value_ok(prm, *v), Some(true)) {
                    return RrDr60Status::InvalidArgument;
                }
            }
        }
        r.component = id;
        r.store = c.store;
    }
    // SAFETY: the arrays as the caller's contract states.
    unsafe {
        write_record_rows(
            &r,
            events,
            host_input_positions,
            event_count,
            changes,
            change_count,
            out,
            out_len,
            out_written,
        )
    }
}

/// Whether `v` is a value the store can hold for `p` (its range, a choice an option).
fn component_value_ok(p: &comp::ComponentParameter, v: f32) -> Option<bool> {
    Some(match p.value {
        rr_dr60::ControlValue::Continuous { min, max, .. } => v >= min && v <= max,
        rr_dr60::ControlValue::Choice { options, .. } => {
            v.fract() == 0.0 && options.iter().any(|o| o.value as f32 == v)
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The NUL twins equal the core's strings (one source).
    #[test]
    fn strings_match_the_core() {
        let strip = |b: &[u8]| core::str::from_utf8(&b[..b.len() - 1]).unwrap().to_owned();
        let p = &comp::insertion_points()[0];
        assert_eq!(strip(POINT_STRINGS[0].0), p.identifier);
        assert_eq!(strip(POINT_STRINGS[0].1), p.name);
        assert_eq!(strip(POINT_STRINGS[0].2), p.placement);
        let c = &comp::components()[0];
        assert_eq!(strip(COMPONENT_STRINGS[0].0), c.identifier);
        assert_eq!(strip(COMPONENT_STRINGS[0].1), c.name);
        assert_eq!(strip(COMPONENT_STRINGS[0].2), c.description);
        for (i, prm) in c.parameters.iter().enumerate() {
            assert_eq!(strip(PARAMETER_STRINGS[0][i].0), prm.identifier);
            assert_eq!(strip(PARAMETER_STRINGS[0][i].1), prm.name);
            assert_eq!(strip(PARAMETER_STRINGS[0][i].2), prm.unit);
        }
        assert_eq!(comp::insertion_points().len(), POINT_STRINGS.len());
        assert_eq!(comp::components().len(), COMPONENT_STRINGS.len());
    }

    /// The real fillers on zeroed, sized buffers (the Miri gate runs this: every byte written
    /// is initialized; coach post-implementation 22).
    #[test]
    fn fillers_write_initialized_bytes() {
        let mut p = RrDr60InsertionPoint::default();
        let mut c = RrDr60Component::default();
        let mut q = RrDr60ComponentParameter::default();
        // SAFETY: sized out structs.
        unsafe {
            assert_eq!(rr_dr60_insertion_point(0, &mut p), RrDr60Status::Ok);
            assert_eq!(rr_dr60_component(0, &mut c), RrDr60Status::Ok);
            for i in 0..4 {
                assert_eq!(rr_dr60_component_parameter(0, i, &mut q), RrDr60Status::Ok);
            }
        }
        let bytes = |v: &[u8]| v.iter().map(|&b| u32::from(b)).sum::<u32>();
        // Reading every byte proves it initialized under Miri.
        // SAFETY: plain structs read as bytes.
        unsafe {
            let _ = bytes(core::slice::from_raw_parts(
                (&p as *const RrDr60InsertionPoint).cast::<u8>(),
                size_of::<RrDr60InsertionPoint>(),
            ));
            let _ = bytes(core::slice::from_raw_parts(
                (&c as *const RrDr60Component).cast::<u8>(),
                size_of::<RrDr60Component>(),
            ));
            let _ = bytes(core::slice::from_raw_parts(
                (&q as *const RrDr60ComponentParameter).cast::<u8>(),
                size_of::<RrDr60ComponentParameter>(),
            ));
        }
        assert_eq!(p.control_index, 31);
    }

    #[test]
    fn defines_match_the_core() {
        assert_eq!(
            RR_DR60_COMPONENT_BAND_FILTER,
            ComponentId::BandFilter as u32
        );
        assert_eq!(RR_DR60_COMPONENT_NONE, ComponentId::None as u32);
        assert_eq!(
            RR_DR60_INSERTION_POINT_BEFORE_LIMITER,
            InsertionPointId::BeforeLimiter as u32
        );
        assert_eq!(RR_DR60_SCALE_LOG, comp::Scale::Log as u32);
        assert_eq!(RR_DR60_SCALE_DECIBEL, comp::Scale::Decibel as u32);
        assert_eq!(RrDr60Status::NoSuchInsertionPoint as u32, 41);
        assert_eq!(RrDr60Status::NoSuchComponent as u32, 42);
        assert_eq!(RrDr60Status::ComponentWhileActive as u32, 43);
    }
}
