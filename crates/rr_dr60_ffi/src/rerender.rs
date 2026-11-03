//! The re-render (spec 027; contracts/c-api.md): two opaque handles, a plan read from a
//! processing record and edited before the run, and the streaming session. Every entry point is
//! panic-guarded; a panic poisons its handle.

use super::*;
use rr_dr60::components::ComponentId;
use rr_dr60::rerender::{self as rr, Plan, ReRender};
use settings_read::{fill_reading, reading_of};

/// A re-render plan (spec 027 D4): read from a record, edited, consumed by
/// `rr_dr60_rerender_new` on success.
pub struct RrDr60ReRenderPlan {
    inner: Plan,
}

/// A re-render session (spec 027 D2).
pub struct RrDr60ReRender {
    inner: Option<ReRender>,
    poisoned: bool,
}

/// What one feed call did (spec 027 D2). Sized (spec 022; 24 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60ReRenderFed {
    /// `sizeof(RrDr60ReRenderFed)`; set by the caller.
    pub struct_size: u32,
    /// The input reached the record's stop position.
    pub reached_stop: bool,
    /// The memory filled and the recording stopped by itself.
    pub self_stopped: bool,
    /// The host input samples the engine took.
    pub taken: u64,
    /// Zero.
    pub reserved: u64,
}
sized_default!(RrDr60ReRenderFed);

/// The result of `rr_dr60_rerender_finish` (spec 027 D5). Sized (56 bytes). The verdict
/// describes processing only; it does not vouch for what the input contains.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60ReRenderOutcome {
    /// `sizeof(RrDr60ReRenderOutcome)`; set by the caller.
    pub struct_size: u32,
    /// 0 kept, 1 nothing kept.
    pub reason: u32,
    /// 0 identical, 1 differs, 2 rebuilt.
    pub verdict: u32,
    /// The record's flags (`RR_DR60_RECORD_FLAG_*`).
    pub flags: u32,
    /// The plan's values differ from the record's.
    pub edited: bool,
    /// The digest equals the record's.
    pub matches_original: bool,
    /// `digest` is valid.
    pub has_digest: bool,
    /// The rebuilt item's digest.
    pub digest: u64,
    /// The remaining capacity the fill reached, in device samples.
    pub remaining_at_start: u64,
    /// The change rows the engine applied.
    pub rows_landed: u64,
    /// Zero.
    pub reserved: u64,
}
sized_default!(RrDr60ReRenderOutcome);

pub(crate) const FED_PUBLISHED: &[u32] = &[size_of::<RrDr60ReRenderFed>() as u32];
pub(crate) const OUTCOME_PUBLISHED: &[u32] = &[size_of::<RrDr60ReRenderOutcome>() as u32];
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<RrDr60ReRenderFed>() == 24);
const _: () = assert!(core::mem::offset_of!(RrDr60ReRenderFed, taken) == 8);
const _: () = assert!(size_of::<RrDr60ReRenderOutcome>() == 56);
const _: () = assert!(core::mem::offset_of!(RrDr60ReRenderOutcome, edited) == 16);
const _: () = assert!(core::mem::offset_of!(RrDr60ReRenderOutcome, digest) == 24);
const _: () = assert!(core::mem::offset_of!(RrDr60ReRenderOutcome, rows_landed) == 40);

/// Runs `f` under the panic guard (a panic is `INTERNAL_ERROR`).
fn guard<T>(f: impl FnOnce() -> Result<T, RrDr60Status>) -> Result<T, RrDr60Status> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => Err(panic_to_status(None)),
    }
}

fn st(r: Result<(), RrDr60Status>) -> RrDr60Status {
    r.err().unwrap_or(RrDr60Status::Ok)
}

/// The plan behind `plan`, or `NULL_POINTER`.
///
/// # Safety
///
/// `plan` NULL or a live plan handle.
unsafe fn plan_mut<'a>(plan: *mut RrDr60ReRenderPlan) -> Result<&'a mut Plan, RrDr60Status> {
    // SAFETY: the caller's contract.
    unsafe { plan.as_mut() }
        .map(|p| &mut p.inner)
        .ok_or(RrDr60Status::NullPointer)
}

/// The session behind `r`: `NULL_POINTER`, or `INTERNAL_ERROR` when poisoned.
///
/// # Safety
///
/// `r` NULL or a live session handle.
unsafe fn session<'a>(r: *mut RrDr60ReRender) -> Result<&'a mut RrDr60ReRender, RrDr60Status> {
    // SAFETY: the caller's contract.
    let h = unsafe { r.as_mut() }.ok_or(RrDr60Status::NullPointer)?;
    if h.poisoned || h.inner.is_none() {
        return Err(RrDr60Status::InternalError);
    }
    Ok(h)
}

/// Runs `f` on a session with panics poisoning it.
fn on_session<T>(
    h: &mut RrDr60ReRender,
    f: impl FnOnce(&mut ReRender) -> Result<T, Error>,
) -> Result<T, RrDr60Status> {
    let inner = h.inner.as_mut().ok_or(RrDr60Status::InternalError)?;
    match catch_unwind(AssertUnwindSafe(|| f(inner))) {
        Ok(r) => r.map_err(status_from_error),
        Err(_) => {
            h.poisoned = true;
            Err(panic_to_status(None))
        }
    }
}

/// Reads a processing record into a new plan (spec 027 D4; spec 020's reader and refusals).
///
/// # Safety
///
/// `bytes` readable for `len` bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_from_record(
    bytes: *const u8,
    len: usize,
    out: *mut *mut RrDr60ReRenderPlan,
) -> RrDr60Status {
    if bytes.is_null() || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: readable for `len` bytes (caller contract).
    let b = unsafe { core::slice::from_raw_parts(bytes, len) };
    match guard(|| Plan::from_record(b).map_err(status_from_error)) {
        Ok(p) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(Box::into_raw(Box::new(RrDr60ReRenderPlan { inner: p }))) };
            RrDr60Status::Ok
        }
        Err(s) => s,
    }
}

/// Destroys a plan (NULL is a no-op).
///
/// # Safety
///
/// `plan` NULL or a plan handle not yet destroyed or consumed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_destroy(plan: *mut RrDr60ReRenderPlan) {
    if !plan.is_null() {
        // SAFETY: the handle came from Box::into_raw (caller contract).
        drop(unsafe { Box::from_raw(plan) });
    }
}

/// Copies a plan.
///
/// # Safety
///
/// `plan` NULL or a live plan; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_clone(
    plan: *const RrDr60ReRenderPlan,
    out: *mut *mut RrDr60ReRenderPlan,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let Some(p) = (unsafe { plan.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    match guard(|| Ok(p.inner.clone())) {
        Ok(c) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(Box::into_raw(Box::new(RrDr60ReRenderPlan { inner: c }))) };
            RrDr60Status::Ok
        }
        Err(s) => s,
    }
}

macro_rules! plan_read {
    ($name:ident, $t:ty, |$p:ident| $e:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Safety
        ///
        /// `plan` NULL or a live plan; `out` NULL or writable.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            plan: *const RrDr60ReRenderPlan,
            out: *mut $t,
        ) -> RrDr60Status {
            // SAFETY: the caller's contract.
            let Some(h) = (unsafe { plan.as_ref() }) else {
                return RrDr60Status::NullPointer;
            };
            if out.is_null() {
                return RrDr60Status::NullPointer;
            }
            let $p = &h.inner;
            match guard(|| Ok($e)) {
                Ok(v) => {
                    // SAFETY: out is non-null and writable (caller contract).
                    unsafe { out.write(v) };
                    RrDr60Status::Ok
                }
                Err(s) => s,
            }
        }
    };
}

plan_read!(
    rr_dr60_rerender_plan_component,
    u32,
    |p| p.component() as u32,
    "The plan's slot component (`RR_DR60_COMPONENT_*`)."
);
plan_read!(
    rr_dr60_rerender_plan_row_count,
    u64,
    |p| p.rows().len() as u64,
    "The plan's change-row count."
);
plan_read!(
    rr_dr60_rerender_plan_seed,
    u64,
    |p| p.seed(),
    "The plan's seed."
);
plan_read!(
    rr_dr60_rerender_plan_counter,
    u64,
    |p| p.counter(),
    "The plan's recording counter."
);
plan_read!(
    rr_dr60_rerender_plan_remaining_samples,
    u64,
    |p| p.remaining_samples(),
    "The plan's remaining capacity at the start, in device samples."
);
plan_read!(
    rr_dr60_rerender_plan_flags,
    u32,
    |p| p.flags(),
    "The record's flags (`RR_DR60_RECORD_FLAG_*`)."
);
plan_read!(
    rr_dr60_rerender_plan_edited,
    bool,
    |p| p.edited(),
    "Whether the plan's values differ from the record's (a field compare)."
);

/// The plan's value of settings control `index` (spec 025's reading; the live controls are
/// `NOT_A_SETTING`).
///
/// # Safety
///
/// `plan` NULL or a live plan; `out` NULL or a writable reading of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_control_value(
    plan: *const RrDr60ReRenderPlan,
    index: u32,
    out: *mut RrDr60ControlReading,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let Some(h) = (unsafe { plan.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, settings_read::READING_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let r = match guard(|| {
        rr_dr60::control_map::settings_value(h.inner.settings(), index).map_err(status_from_error)
    }) {
        Ok(r) => r,
        Err(s) => return s,
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, settings_read::READING_PUBLISHED, |t, _| {
            fill_reading(t, r)
        })
    }
}

/// Store value `index` of the plan.
///
/// # Safety
///
/// `plan` NULL or a live plan; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_parameter(
    plan: *const RrDr60ReRenderPlan,
    index: u32,
    out: *mut f64,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let Some(h) = (unsafe { plan.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    match h.inner.parameter(index) {
        Some(v) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(f64::from(v)) };
            RrDr60Status::Ok
        }
        None => RrDr60Status::NoSuchComponent,
    }
}

/// Row `i` of the plan.
///
/// # Safety
///
/// `plan` NULL or a live plan; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_row(
    plan: *const RrDr60ReRenderPlan,
    i: u64,
    out: *mut RrDr60ChangeEntry,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let Some(h) = (unsafe { plan.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    let Some(c) = h.inner.rows().get(i as usize) else {
        return RrDr60Status::NoSuchControl;
    };
    let e = RrDr60ChangeEntry {
        position: c.position,
        before: c.before,
        after: c.after,
        tag: c.tag,
        control: c.control,
        stream: c.stream,
    };
    // SAFETY: out is non-null and writable (caller contract).
    unsafe { out.write(e) };
    RrDr60Status::Ok
}

fn core_row(e: &RrDr60ChangeEntry) -> rr_dr60::ChangeEntry {
    rr_dr60::ChangeEntry::new(e.position, e.control, e.before, e.after, e.tag, e.stream)
}

macro_rules! plan_edit {
    ($name:ident, ($($arg:ident: $t:ty),*), |$p:ident| $e:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Safety
        ///
        /// `plan` NULL or a live plan; every pointer argument NULL or valid.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(plan: *mut RrDr60ReRenderPlan $(, $arg: $t)*) -> RrDr60Status {
            // SAFETY: the caller's contract.
            let $p = match unsafe { plan_mut(plan) } {
                Ok(p) => p,
                Err(s) => return s,
            };
            st(guard(|| $e))
        }
    };
}

plan_edit!(rr_dr60_rerender_plan_set_seed, (seed: u64), |p| { p.set_seed(seed); Ok(()) }, "Sets the plan's seed.");
plan_edit!(rr_dr60_rerender_plan_set_counter, (counter: u64), |p| { p.set_counter(counter); Ok(()) }, "Sets the plan's recording counter.");
plan_edit!(rr_dr60_rerender_plan_set_remaining_samples, (remaining: u64), |p| { p.set_remaining_samples(remaining); Ok(()) }, "Sets the remaining capacity at the start (checked at `rr_dr60_rerender_new`).");
plan_edit!(rr_dr60_rerender_plan_set_collect_run, (collect: bool), |p| { p.set_collect_run(collect); Ok(()) }, "Collects the run (the default) or not (a digest-only session).");
plan_edit!(rr_dr60_rerender_plan_set_component, (component: u32), |p| {
    let c = ComponentId::from_u32(component).ok_or(RrDr60Status::NoSuchComponent)?;
    p.set_component(c);
    Ok(())
}, "Sets the plan's slot component (`RR_DR60_COMPONENT_*`).");
plan_edit!(rr_dr60_rerender_plan_set_parameter, (index: u32, value: f64), |p| p.set_parameter(index, value).map_err(status_from_error), "Writes store value `index`, validated as `rr_dr60_set_component_parameter` validates it.");
plan_edit!(rr_dr60_rerender_plan_remove_row, (i: u64), |p| p.remove_row(i as usize).map_err(status_from_error), "Removes row `i`.");

/// Sets settings control `index` in the plan (spec 025's reading; settings controls only: the
/// live controls are `NOT_A_SETTING` and the slot's 31 – 35 go through
/// `rr_dr60_rerender_plan_set_component` and `_set_parameter`), validated as
/// `rr_dr60_reconfigure` validates the settings; another host rate is `HOST_RATE_FIXED`.
///
/// # Safety
///
/// `plan` NULL or a live plan; `value` NULL or a readable reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_set_control_value(
    plan: *mut RrDr60ReRenderPlan,
    index: u32,
    value: *const RrDr60ControlReading,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let p = match unsafe { plan_mut(plan) } {
        Ok(p) => p,
        Err(s) => return s,
    };
    if value.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: readable (caller contract).
    let c = unsafe { value.read_unaligned() };
    let Some(r) = reading_of(&c) else {
        return RrDr60Status::InvalidArgument;
    };
    if (31..=35).contains(&index) {
        return RrDr60Status::NotASetting;
    }
    st(guard(|| {
        let mut s = *p.settings();
        rr_dr60::control_map::set_settings_value(&mut s, index, r).map_err(status_from_error)?;
        p.set_settings(s).map_err(status_from_error)
    }))
}

/// Replaces row `i` (re-sorted, stable; validated as `rr_dr60_schedule_change` validates it).
///
/// # Safety
///
/// `plan` NULL or a live plan; `row` NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_set_row(
    plan: *mut RrDr60ReRenderPlan,
    i: u64,
    row: *const RrDr60ChangeEntry,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let p = match unsafe { plan_mut(plan) } {
        Ok(p) => p,
        Err(s) => return s,
    };
    // SAFETY: NULL or readable (caller contract).
    let Some(e) = (unsafe { row.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    st(guard(|| {
        p.set_row(i as usize, core_row(e))
            .map_err(status_from_error)
    }))
}

/// Inserts a row (kept in position order after the rows at the same position).
///
/// # Safety
///
/// `plan` NULL or a live plan; `row` NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_plan_insert_row(
    plan: *mut RrDr60ReRenderPlan,
    row: *const RrDr60ChangeEntry,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let p = match unsafe { plan_mut(plan) } {
        Ok(p) => p,
        Err(s) => return s,
    };
    // SAFETY: NULL or readable (caller contract).
    let Some(e) = (unsafe { row.as_ref() }) else {
        return RrDr60Status::NullPointer;
    };
    st(guard(|| {
        p.insert_row(core_row(e)).map_err(status_from_error)
    }))
}

/// Builds a session from `plan` (spec 027 D2): the engine, the slot, the fill, every buffer.
/// **Consumes the plan on success only**: after an error the plan is still the caller's (to
/// edit and retry, e.g. `collect_run` false after `RUN_TOO_LARGE`). Not real-time safe.
///
/// # Safety
///
/// `plan` NULL or a live plan; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_new(
    plan: *mut RrDr60ReRenderPlan,
    out: *mut *mut RrDr60ReRender,
) -> RrDr60Status {
    if plan.is_null() || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: a live plan (caller contract).
    let p = unsafe { &*plan }.inner.clone();
    match guard(|| ReRender::new(p).map_err(status_from_error)) {
        Ok(r) => {
            // SAFETY: the plan handle came from Box::into_raw and is consumed here.
            drop(unsafe { Box::from_raw(plan) });
            let h = Box::new(RrDr60ReRender {
                inner: Some(r),
                poisoned: false,
            });
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(Box::into_raw(h)) };
            RrDr60Status::Ok
        }
        Err(s) => s,
    }
}

/// Feeds `n` 24-bit codes (spec 020's feed point). Allocation-free.
///
/// # Safety
///
/// `r` NULL or a live session; `codes` readable for `n` values (NULL with `n` 0); `out` NULL
/// or a writable `RrDr60ReRenderFed` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_feed_i24(
    r: *mut RrDr60ReRender,
    codes: *const i32,
    n: usize,
    out: *mut RrDr60ReRenderFed,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe {
        feed_any(r, out, codes.is_null() && n > 0, |s| {
            let x: &[i32] = if n == 0 {
                &[]
            } else {
                core::slice::from_raw_parts(codes, n)
            };
            s.feed(x)
        })
    }
}

/// Feeds `n` engine samples (a host that kept them losslessly). Allocation-free.
///
/// # Safety
///
/// As `rr_dr60_rerender_feed_i24`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_feed_f32(
    r: *mut RrDr60ReRender,
    samples: *const f32,
    n: usize,
    out: *mut RrDr60ReRenderFed,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe {
        feed_any(r, out, samples.is_null() && n > 0, |s| {
            let x: &[f32] = if n == 0 {
                &[]
            } else {
                core::slice::from_raw_parts(samples, n)
            };
            s.feed_f32(x)
        })
    }
}

/// # Safety
///
/// `r` NULL or a live session; `out` NULL or sized; `f` reads only what the caller allowed.
unsafe fn feed_any(
    r: *mut RrDr60ReRender,
    out: *mut RrDr60ReRenderFed,
    null_input: bool,
    f: impl FnOnce(&mut ReRender) -> Result<rr::Fed, Error>,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if null_input || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, FED_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let fed = match on_session(h, f) {
        Ok(v) => v,
        Err(s) => return s,
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, FED_PUBLISHED, |t, _| {
            t.reached_stop = fed.reached_stop;
            t.self_stopped = fed.self_stopped;
            t.taken = fed.taken;
            t.reserved = 0;
        })
    }
}

/// The input position and the record's stop position (progress). After a self-stop the input
/// stays below the stop: treat `reached_stop || self_stopped` as done.
///
/// # Safety
///
/// `r` NULL or a live session; the outs NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_progress(
    r: *mut RrDr60ReRender,
    input_position: *mut u64,
    stop_position: *mut u64,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if input_position.is_null() || stop_position.is_null() {
        return RrDr60Status::NullPointer;
    }
    let (a, b) = h.inner.as_ref().map_or((0, 0), ReRender::progress);
    // SAFETY: both non-null and writable (caller contract).
    unsafe {
        input_position.write(a);
        stop_position.write(b);
    }
    RrDr60Status::Ok
}

/// The engine's meter read-out of its last call (at most 4096 host samples; spec 016's sized
/// struct).
///
/// # Safety
///
/// `r` NULL or a live session; `out` NULL or a writable `RrDr60Meters` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_meters(
    r: *mut RrDr60ReRender,
    out: *mut RrDr60Meters,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    let m = match on_session(h, |s| Ok(s.meters())) {
        Ok(m) => m,
        Err(s) => return s,
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe { write_sized(out, METERS_PUBLISHED, |t, _| t.fill(m)) }
}

/// Stops (unless the recording stopped by itself) and reads the rebuilt item (spec 027 D5,
/// D6). After `INPUT_SHORT` the session stays feedable.
///
/// # Safety
///
/// `r` NULL or a live session; `out` NULL or a writable `RrDr60ReRenderOutcome`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_finish(
    r: *mut RrDr60ReRender,
    out: *mut RrDr60ReRenderOutcome,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: out is non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, OUTCOME_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let o = match on_session(h, ReRender::finish) {
        Ok(o) => o,
        Err(s) => return s,
    };
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    unsafe {
        write_sized(out, OUTCOME_PUBLISHED, |t, _| {
            t.reason = u32::from(o.reason != rr::Reason::Kept);
            t.verdict = match o.verdict {
                rr::Verdict::Identical => 0,
                rr::Verdict::Differs => 1,
                _ => 2,
            };
            t.flags = o.flags;
            t.edited = o.edited;
            t.matches_original = o.matches_original;
            t.has_digest = o.digest.is_some();
            t.digest = o.digest.unwrap_or(0);
            t.remaining_at_start = o.remaining_at_start;
            t.rows_landed = o.rows_landed;
            t.reserved = 0;
        })
    }
}

/// # Safety
///
/// `r` NULL or a live session; `out` NULL or writable.
unsafe fn len_of(
    r: *mut RrDr60ReRender,
    out: *mut usize,
    f: impl FnOnce(&mut ReRender) -> Result<usize, Error>,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    match on_session(h, f) {
        Ok(n) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(n) };
            RrDr60Status::Ok
        }
        Err(s) => s,
    }
}

/// The rebuilt item's stored form's length (0 when nothing was kept). After `finish`.
///
/// # Safety
///
/// `r` NULL or a live session; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_export_len(
    r: *mut RrDr60ReRender,
    out: *mut usize,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { len_of(r, out, |s| s.export_len()) }
}

/// The rebuilt item's new processing record's length (0 when nothing was kept). After `finish`.
///
/// # Safety
///
/// `r` NULL or a live session; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_record_len(
    r: *mut RrDr60ReRender,
    out: *mut usize,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { len_of(r, out, |s| s.record_len()) }
}

/// # Safety
///
/// `r` NULL or a live session; `buf` writable for `len` bytes; `written` NULL or writable.
unsafe fn write_of(
    r: *mut RrDr60ReRender,
    buf: *mut u8,
    len: usize,
    written: *mut usize,
    f: impl FnOnce(&mut ReRender, &mut [u8]) -> Result<usize, Error>,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if (buf.is_null() && len > 0) || written.is_null() {
        return RrDr60Status::NullPointer;
    }
    let out: &mut [u8] = if len == 0 {
        &mut []
    } else {
        // SAFETY: writable for `len` bytes (caller contract).
        unsafe { core::slice::from_raw_parts_mut(buf, len) }
    };
    match on_session(h, |s| f(s, out)) {
        Ok(n) => {
            // SAFETY: written is non-null and writable (caller contract).
            unsafe { written.write(n) };
            RrDr60Status::Ok
        }
        Err(s) => s,
    }
}

/// Writes the rebuilt item's stored form. After `finish`.
///
/// # Safety
///
/// As `rr_dr60_rerender_record_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_export(
    r: *mut RrDr60ReRender,
    buf: *mut u8,
    len: usize,
    written: *mut usize,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { write_of(r, buf, len, written, |s, o| s.export(o)) }
}

/// Writes the rebuilt item's new processing record (an ordinary engine record). After
/// `finish`; `RUN_NOT_COLLECTED` without `collect_run`.
///
/// # Safety
///
/// `r` NULL or a live session; `buf` writable for `len` bytes; `written` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_record_write(
    r: *mut RrDr60ReRender,
    buf: *mut u8,
    len: usize,
    written: *mut usize,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { write_of(r, buf, len, written, |s, o| s.record_write(o)) }
}

/// Turns a finished session into an engine handle holding the rebuilt item as its last
/// recording (to play it). Consumes the session on success (it is destroyed); `RERENDER_STATE`
/// before `finish`.
///
/// # Safety
///
/// `r` NULL or a live session; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_into_pipeline(
    r: *mut RrDr60ReRender,
    out: *mut *mut RrDr60Pipeline,
) -> RrDr60Status {
    // SAFETY: the caller's contract.
    let h = match unsafe { session(r) } {
        Ok(h) => h,
        Err(s) => return s,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    if h.inner.as_ref().and_then(ReRender::outcome).is_none() {
        return RrDr60Status::RerenderState;
    }
    let Some(s) = h.inner.take() else {
        return RrDr60Status::InternalError;
    };
    let p = match s.into_pipeline() {
        Ok(p) => p,
        Err(e) => return status_from_error(e),
    };
    // SAFETY: the session handle came from Box::into_raw and is consumed here.
    drop(unsafe { Box::from_raw(r) });
    let handle = Box::new(RrDr60Pipeline::from_inner(p));
    // SAFETY: out is non-null and writable (caller contract).
    unsafe { out.write(Box::into_raw(handle)) };
    RrDr60Status::Ok
}

/// Destroys a session (NULL is a no-op).
///
/// # Safety
///
/// `r` NULL or a session handle not yet destroyed or consumed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_rerender_destroy(r: *mut RrDr60ReRender) {
    if !r.is_null() {
        // SAFETY: the handle came from Box::into_raw (caller contract).
        drop(unsafe { Box::from_raw(r) });
    }
}

/// Fills the memory so its remaining capacity is `remaining` device samples, floored to the
/// stored unit (spec 027 D7; `Pipeline::fill_to_remaining`). Idle only.
///
/// # Safety
///
/// `pipeline` NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_fill_to_remaining(
    pipeline: *mut RrDr60Pipeline,
    remaining: u64,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(s) => return s,
    };
    match guarded(p, |inner| inner.fill_to_remaining(remaining)) {
        Ok(()) => RrDr60Status::Ok,
        Err(s) => s,
    }
}
