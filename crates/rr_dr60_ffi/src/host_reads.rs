//! The host reads (spec 026; contracts/c-api.md): the one-pass's gate and coder rules, its
//! reports' latency and its drain, the wording lists' matcher, the meter manifest, and the stage
//! manifest's words. The strings are the core's, copied into NUL-terminated tables at compile
//! time (one source).

use super::*;
use rr_dr60::meters::{FIELDS, MeterCall};
use rr_dr60::wording::{self, CLAIMS, ENGINE_STRINGS, Lists};

/// `rr_dr60_wording_first_hit`'s kind: the manifests' list.
pub const RR_DR60_WORDING_ENGINE_STRINGS: u32 = 0;
/// `rr_dr60_wording_first_hit`'s kind: the claims list.
pub const RR_DR60_WORDING_CLAIMS: u32 = 1;
/// `rr_dr60_meter_reached` and `rr_dr60_meter_lag_samples`'s call: a one-pass call.
pub const RR_DR60_METER_CALL_ONE_PASS: u32 = 0;
/// A record call.
pub const RR_DR60_METER_CALL_RECORD: u32 = 1;
/// A play call.
pub const RR_DR60_METER_CALL_PLAY: u32 = 2;
/// `rr_dr60_meter_lag_samples`'s value for a field the call does not reach.
pub const RR_DR60_METER_NOT_REACHED: u64 = 0xFFFF_FFFF_FFFF_FFFF;
/// `rr_dr60_manifest_word`'s kinds and forms.
pub const RR_DR60_MANIFEST_WORD_STATUS: u32 = 0;
/// The path.
pub const RR_DR60_MANIFEST_WORD_PATH: u32 = 1;
/// The confidence.
pub const RR_DR60_MANIFEST_WORD_CONFIDENCE: u32 = 2;
/// The canonical text's token.
pub const RR_DR60_MANIFEST_WORD_TOKEN: u32 = 0;
/// The display label.
pub const RR_DR60_MANIFEST_WORD_LABEL: u32 = 1;

/// One meter field's description (spec 026 D4), filled by `rr_dr60_meter_field`. Sized
/// (spec 022's rule; 40 bytes on 64-bit). The strings are static and NUL-terminated.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RrDr60MeterField {
    /// `sizeof(RrDr60MeterField)`; set by the caller before every call.
    pub struct_size: u32,
    /// The field's index in `RrDr60Meters`'s declaration order.
    pub index: u32,
    /// The field's name.
    pub identifier: *const c_char,
    /// Its unit.
    pub unit: *const c_char,
    /// One neutral sentence.
    pub sentence: *const c_char,
    /// 0 a maximum over the call, 1 a count, 2 a state at the call's end.
    pub kind: u32,
    /// 0 the record side (input), 1 the playback side (output).
    pub side: u32,
}
sized_default!(RrDr60MeterField);

/// The structure's size and published layouts (spec 022 D1).
pub(crate) const METER_FIELD_SIZE: u32 = size_of::<RrDr60MeterField>() as u32;
pub(crate) const METER_FIELD_PUBLISHED: &[u32] = &[METER_FIELD_SIZE];
#[cfg(target_pointer_width = "64")]
const _: () = assert!(METER_FIELD_SIZE == 40);
const _: () = assert!(core::mem::offset_of!(RrDr60MeterField, index) == 4);

/// A string's NUL-terminated copy in a fixed buffer, built at compile time.
const fn nul<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    assert!(b.len() < N, "the buffer holds the string and its NUL");
    let mut out = [0u8; N];
    let mut i = 0;
    while i < b.len() {
        out[i] = b[i];
        i += 1;
    }
    out
}

/// A list's entries (stems, then phrases) as NUL-terminated buffers.
const fn entries<const M: usize>(l: &Lists) -> [[u8; 32]; M] {
    let mut out = [[0u8; 32]; M];
    let mut k = 0;
    while k < l.stems.len() {
        out[k] = nul::<32>(l.stems[k]);
        k += 1;
    }
    let mut j = 0;
    while j < l.phrases.len() {
        out[k + j] = nul::<32>(l.phrases[j]);
        j += 1;
    }
    out
}

const ENGINE_N: usize = ENGINE_STRINGS.stems.len() + ENGINE_STRINGS.phrases.len();
const CLAIMS_N: usize = CLAIMS.stems.len() + CLAIMS.phrases.len();
static ENGINE_ENTRIES: [[u8; 32]; ENGINE_N] = entries::<ENGINE_N>(&ENGINE_STRINGS);
static CLAIMS_ENTRIES: [[u8; 32]; CLAIMS_N] = entries::<CLAIMS_N>(&CLAIMS);

/// The meter fields' strings.
const fn meter_strings<const N: usize>(which: u8) -> [[u8; N]; 13] {
    let mut out = [[0u8; N]; 13];
    let mut i = 0;
    while i < 13 {
        let s = match which {
            0 => FIELDS[i].identifier,
            1 => FIELDS[i].unit,
            _ => FIELDS[i].sentence,
        };
        out[i] = nul::<N>(s);
        i += 1;
    }
    out
}
static METER_IDS: [[u8; 32]; 13] = meter_strings::<32>(0);
static METER_UNITS: [[u8; 32]; 13] = meter_strings::<32>(1);
static METER_SENTENCES: [[u8; 128]; 13] = meter_strings::<128>(2);

/// The stage manifest's words: [kind][value][form].
static WORDS: [[[[u8; 32]; 2]; 4]; 3] = {
    use rr_dr60::{Confidence as C, StagePath as P, StageStatus as S};
    let s = [S::NotModeled, S::Assumed, S::Sourced, S::Checked];
    let p = [P::Record, P::Playback, P::Both];
    let c = [C::NotApplicable, C::Low, C::Medium, C::High];
    let mut w = [[[[0u8; 32]; 2]; 4]; 3];
    let mut i = 0;
    while i < 4 {
        w[0][i] = [nul::<32>(s[i].token()), nul::<32>(s[i].label())];
        w[2][i] = [nul::<32>(c[i].token()), nul::<32>(c[i].label())];
        if i < 3 {
            w[1][i] = [nul::<32>(p[i].token()), nul::<32>(p[i].label())];
        }
        i += 1;
    }
    w
};

/// The settings fields the run rules read, at their offsets, without forming a reference to
/// the structure (spec 025's rule): `(vas_enabled, speech_codec_enabled, tap)`.
///
/// # Safety
///
/// `s` must be readable for `sizeof(RrDr60Settings)` bytes.
unsafe fn rule_fields(s: *const RrDr60Settings) -> Result<(bool, bool, Tap), RrDr60Status> {
    // SAFETY: the caller's contract; the size checked first.
    if unsafe { struct_size_at(s) } != SETTINGS_SIZE {
        return Err(RrDr60Status::InvalidArgument);
    }
    let base = s.cast::<u8>();
    let byte = |off: usize| -> Result<bool, RrDr60Status> {
        // SAFETY: the offset is a field of the readable structure.
        match unsafe { base.add(off).read() } {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(RrDr60Status::InvalidArgument),
        }
    };
    let vas = byte(core::mem::offset_of!(RrDr60Settings, vas_enabled))?;
    let codec = byte(core::mem::offset_of!(RrDr60Settings, speech_codec_enabled))?;
    // SAFETY: as above, a `uint32_t` field read unaligned.
    let t = unsafe {
        base.add(core::mem::offset_of!(RrDr60Settings, tap))
            .cast::<u32>()
            .read_unaligned()
    };
    let tap = tap_from_c(t).ok_or(RrDr60Status::InvalidArgument)?;
    Ok((vas, codec, tap))
}

/// Whether the voice-activation gate runs on the one-pass for a caller's settings (spec 026
/// D1; `Settings::one_pass_vas_runs`): enabled, and the tap at the gate, the decoder or after
/// stage 10. Refusals in order: NULL (`NULL_POINTER`); a `struct_size` other than
/// `sizeof(RrDr60Settings)`, a switch byte other than 0 or 1, or an unknown tap
/// (`INVALID_ARGUMENT`). Only the three fields are read. Real-time safe; no handle.
///
/// # Safety
///
/// `settings` NULL or readable for `sizeof(RrDr60Settings)` bytes; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_settings_one_pass_vas_runs(
    settings: *const RrDr60Settings,
    out: *mut bool,
) -> RrDr60Status {
    if settings.is_null() || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null, readable (caller contract).
    match unsafe { rule_fields(settings) } {
        Ok((vas, _, tap)) => {
            let mut s = Settings::new(6000);
            s.vas.enabled = vas;
            s.tap = tap;
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(s.one_pass_vas_runs()) };
            RrDr60Status::Ok
        }
        Err(status) => status,
    }
}

/// Whether the speech coder runs on the one-pass for a caller's settings (spec 026 D1;
/// `Settings::one_pass_codec_runs`): on, and the tap at or after the decoder. The refusals of
/// `rr_dr60_settings_one_pass_vas_runs`. Real-time safe; no handle.
///
/// # Safety
///
/// As `rr_dr60_settings_one_pass_vas_runs`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_settings_one_pass_codec_runs(
    settings: *const RrDr60Settings,
    out: *mut bool,
) -> RrDr60Status {
    if settings.is_null() || out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null, readable (caller contract).
    match unsafe { rule_fields(settings) } {
        Ok((_, codec, tap)) => {
            let mut s = Settings::new(6000);
            s.speech_codec_enabled = codec;
            s.tap = tap;
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(s.one_pass_codec_runs()) };
            RrDr60Status::Ok
        }
        Err(status) => status,
    }
}

/// A handle read of one value (the refusal order: the handle, then NULL).
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out` NULL or writable.
unsafe fn read_value<T>(
    pipeline: *mut RrDr60Pipeline,
    out: *mut T,
    f: impl FnOnce(&Pipeline) -> T,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    let forced = take_forced_panic(p);
    match guarded(p, |inner| {
        if forced {
            panic!("rr_dr60__test_force_panic");
        }
        Ok(f(inner))
    }) {
        Ok(v) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(v) };
            RrDr60Status::Ok
        }
        Err(status) => status,
    }
}

/// The latency the one-pass's voice-activation reports align with, in host samples (spec 026
/// D1; `Pipeline::one_pass_report_latency_samples`). Real-time safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_one_pass_report_latency_samples(
    pipeline: *mut RrDr60Pipeline,
    out: *mut u32,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { read_value(pipeline, out, Pipeline::one_pass_report_latency_samples) }
}

/// The one-pass drain in host samples (spec 026 D2; `Pipeline::one_pass_drain_samples`, whose
/// scope and exclusions apply). Real-time safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_one_pass_drain_samples(
    pipeline: *mut RrDr60Pipeline,
    out: *mut u64,
) -> RrDr60Status {
    // SAFETY: the caller's contract, passed on.
    unsafe { read_value(pipeline, out, |p| u64::from(p.one_pass_drain_samples())) }
}

/// The first word of a wording list in `text` (spec 026 D3; `wording::first_hit` for kind
/// `RR_DR60_WORDING_ENGINE_STRINGS`, `wording::first_claim_hit` for `RR_DR60_WORDING_CLAIMS`):
/// `*out_entry` the matched list entry (static, NUL-terminated) or NULL for no hit, and the
/// match's byte range `[*out_start, *out_end)`. These are the words the engine's own strings do
/// not use; not a store or copy check. Refusals in order: a NULL out pointer, or a NULL `text`
/// with a non-zero `len` (`NULL_POINTER`); an unknown kind (`NO_SUCH_WORDING_KIND`); text that is
/// not UTF-8 (`INVALID_UTF8`). Linear in `len`, allocation-free; no handle.
///
/// # Safety
///
/// `text` readable for `len` bytes (or NULL with `len` 0); the three outs NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_wording_first_hit(
    text: *const u8,
    len: usize,
    kind: u32,
    out_entry: *mut *const c_char,
    out_start: *mut usize,
    out_end: *mut usize,
) -> RrDr60Status {
    if out_entry.is_null()
        || out_start.is_null()
        || out_end.is_null()
        || (text.is_null() && len > 0)
    {
        return RrDr60Status::NullPointer;
    }
    let (lists, table): (&Lists, &[[u8; 32]]) = match kind {
        RR_DR60_WORDING_ENGINE_STRINGS => (&ENGINE_STRINGS, &ENGINE_ENTRIES),
        RR_DR60_WORDING_CLAIMS => (&CLAIMS, &CLAIMS_ENTRIES),
        _ => return RrDr60Status::NoSuchWordingKind,
    };
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: readable for `len` bytes (caller contract).
        unsafe { core::slice::from_raw_parts(text, len) }
    };
    let Ok(s) = core::str::from_utf8(bytes) else {
        return RrDr60Status::InvalidUtf8;
    };
    let hit = match manifest_guard(|| wording::first_hit_in(lists, s)) {
        Ok(h) => h,
        // coverage: unreachable, a scan (Constitution III)
        Err(status) => return status,
    };
    let (entry, start, end) = match hit {
        Some(h) => {
            let i = lists
                .stems
                .iter()
                .chain(lists.phrases)
                .position(|e| core::ptr::eq(*e, h.entry) || *e == h.entry)
                .unwrap_or(0);
            (table[i].as_ptr().cast::<c_char>(), h.start, h.end)
        }
        None => (core::ptr::null(), 0, 0),
    };
    // SAFETY: the outs are non-null and writable (caller contract).
    unsafe {
        out_entry.write(entry);
        out_start.write(start);
        out_end.write(end);
    }
    RrDr60Status::Ok
}

/// The number of meter fields (13; spec 026 D4). Real-time safe; no handle.
#[unsafe(no_mangle)]
pub extern "C" fn rr_dr60_meter_field_count() -> u32 {
    FIELDS.len() as u32
}

/// Meter field `index`'s description into `*out` (spec 026 D4; `meters::FIELDS`). Refusals in
/// order, nothing written: NULL (`NULL_POINTER`); a refused `struct_size` (`INVALID_ARGUMENT`);
/// an index past the fields (`NO_SUCH_METER_FIELD`). Real-time safe; no handle.
///
/// # Safety
///
/// `out` NULL or a writable `RrDr60MeterField` of its `struct_size`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_meter_field(
    index: u32,
    out: *mut RrDr60MeterField,
) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    // SAFETY: non-null, its first four bytes readable (caller contract).
    if !unsafe { report_ok(out, METER_FIELD_PUBLISHED) } {
        return RrDr60Status::InvalidArgument;
    }
    let i = index as usize;
    let Some(f) = FIELDS.get(i) else {
        return RrDr60Status::NoSuchMeterField;
    };
    let (kind, side) = (f.kind as u32, f.side as u32);
    // SAFETY: out is non-null with its `struct_size` bytes writable (caller contract).
    match manifest_guard(|| unsafe {
        write_sized(out, METER_FIELD_PUBLISHED, |t, _| {
            t.index = index;
            t.identifier = METER_IDS[i].as_ptr().cast();
            t.unit = METER_UNITS[i].as_ptr().cast();
            t.sentence = METER_SENTENCES[i].as_ptr().cast();
            t.kind = kind;
            t.side = side;
        })
    }) {
        Ok(status) => status,
        // coverage: unreachable, a field copy (Constitution III)
        Err(status) => status,
    }
}

fn meter_call(call: u32) -> Option<MeterCall> {
    match call {
        RR_DR60_METER_CALL_ONE_PASS => Some(MeterCall::OnePass),
        RR_DR60_METER_CALL_RECORD => Some(MeterCall::Record),
        RR_DR60_METER_CALL_PLAY => Some(MeterCall::Play),
        _ => None,
    }
}

/// Whether `call` reaches meter field `index`'s point at the engine's settings (spec 026 D4;
/// `Pipeline::meter_reached`). Refusals in order: the handle; NULL (`NULL_POINTER`); an index
/// past the fields (`NO_SUCH_METER_FIELD`); an unknown call (`NO_SUCH_METER_CALL`). Real-time
/// safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_meter_reached(
    pipeline: *mut RrDr60Pipeline,
    index: u32,
    call: u32,
    out: *mut bool,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    if index as usize >= FIELDS.len() {
        return RrDr60Status::NoSuchMeterField;
    }
    let Some(c) = meter_call(call) else {
        return RrDr60Status::NoSuchMeterCall;
    };
    match guarded(p, |inner| Ok(inner.meter_reached(index as usize, c))) {
        Ok(v) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(v) };
            RrDr60Status::Ok
        }
        Err(status) => status,
    }
}

/// Meter field `index`'s lag in host samples for `call` (spec 026 D4;
/// `Pipeline::meter_lag_samples`), or `RR_DR60_METER_NOT_REACHED` when the call does not reach
/// it. The refusals of `rr_dr60_meter_reached`. Real-time safe.
///
/// # Safety
///
/// `pipeline` NULL or a live handle; `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_meter_lag_samples(
    pipeline: *mut RrDr60Pipeline,
    index: u32,
    call: u32,
    out: *mut u64,
) -> RrDr60Status {
    // SAFETY: NULL or a live handle (caller contract).
    let p = match unsafe { live(pipeline) } {
        Ok(p) => p,
        Err(status) => return status,
    };
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    if index as usize >= FIELDS.len() {
        return RrDr60Status::NoSuchMeterField;
    }
    let Some(c) = meter_call(call) else {
        return RrDr60Status::NoSuchMeterCall;
    };
    match guarded(p, |inner| {
        Ok(inner
            .meter_lag_samples(index as usize, c)
            .map_or(RR_DR60_METER_NOT_REACHED, u64::from))
    }) {
        Ok(v) => {
            // SAFETY: out is non-null and writable (caller contract).
            unsafe { out.write(v) };
            RrDr60Status::Ok
        }
        Err(status) => status,
    }
}

/// A stage manifest word (spec 026 D6): `kind` status, path or confidence; `value` the
/// `RrDr60StageStatus` / `RrDr60StagePath` / `RrDr60Confidence` value; `form` the canonical
/// text's token or the display label (lowercase; never "checked" alone). `*out` a static
/// NUL-terminated string. Refusals in order: NULL (`NULL_POINTER`); an unknown kind, value or
/// form (`NO_SUCH_MANIFEST_WORD`). Real-time safe; no handle.
///
/// # Safety
///
/// `out` NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_dr60_manifest_word(
    kind: u32,
    value: u32,
    form: u32,
    out: *mut *const c_char,
) -> RrDr60Status {
    if out.is_null() {
        return RrDr60Status::NullPointer;
    }
    let count = match kind {
        RR_DR60_MANIFEST_WORD_STATUS | RR_DR60_MANIFEST_WORD_CONFIDENCE => 4,
        RR_DR60_MANIFEST_WORD_PATH => 3,
        _ => return RrDr60Status::NoSuchManifestWord,
    };
    if value >= count || form > RR_DR60_MANIFEST_WORD_LABEL {
        return RrDr60Status::NoSuchManifestWord;
    }
    let w = &WORDS[kind as usize][value as usize][form as usize];
    // SAFETY: out is non-null and writable (caller contract).
    unsafe { out.write(w.as_ptr().cast()) };
    RrDr60Status::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstr(p: *const c_char) -> &'static str {
        // SAFETY: the tables' strings are static and NUL-terminated.
        unsafe { core::ffi::CStr::from_ptr(p) }.to_str().unwrap()
    }

    #[test]
    fn words_and_fields_are_the_core_strings() {
        let mut out = core::ptr::null();
        // SAFETY: a valid out.
        assert_eq!(
            unsafe { rr_dr60_manifest_word(0, 3, 1, &mut out) },
            RrDr60Status::Ok
        );
        assert_eq!(cstr(out), "checked against a real unit");
        // SAFETY: a valid out.
        assert_eq!(
            unsafe { rr_dr60_manifest_word(1, 2, 1, &mut out) },
            RrDr60Status::Ok
        );
        assert_eq!(cstr(out), "record and playback");
        // SAFETY: a valid out.
        assert_eq!(
            unsafe { rr_dr60_manifest_word(1, 3, 0, &mut out) },
            RrDr60Status::NoSuchManifestWord
        );
        for (i, f) in FIELDS.iter().enumerate() {
            let mut m = RrDr60MeterField::default();
            // SAFETY: a sized out.
            assert_eq!(
                unsafe { rr_dr60_meter_field(i as u32, &mut m) },
                RrDr60Status::Ok
            );
            assert_eq!(cstr(m.identifier), f.identifier);
            assert_eq!(cstr(m.sentence), f.sentence);
            assert_eq!(cstr(m.unit), f.unit);
        }
    }

    #[test]
    fn wording_hit_and_refusals() {
        let t = "a ghost here";
        let (mut e, mut s, mut n) = (core::ptr::null(), 0, 0);
        // SAFETY: valid text and outs.
        let st =
            unsafe { rr_dr60_wording_first_hit(t.as_ptr(), t.len(), 0, &mut e, &mut s, &mut n) };
        assert_eq!(st, RrDr60Status::Ok);
        assert_eq!((cstr(e), s, n), ("ghost", 2, 7));
        let bad = [0xffu8, 0xfe];
        // SAFETY: valid outs.
        assert_eq!(
            unsafe { rr_dr60_wording_first_hit(bad.as_ptr(), 2, 0, &mut e, &mut s, &mut n) },
            RrDr60Status::InvalidUtf8
        );
        // SAFETY: valid outs.
        assert_eq!(
            unsafe { rr_dr60_wording_first_hit(t.as_ptr(), t.len(), 9, &mut e, &mut s, &mut n) },
            RrDr60Status::NoSuchWordingKind
        );
        let clean = "the band filter";
        // SAFETY: valid text and outs.
        assert_eq!(
            unsafe {
                rr_dr60_wording_first_hit(clean.as_ptr(), clean.len(), 1, &mut e, &mut s, &mut n)
            },
            RrDr60Status::Ok
        );
        assert!(e.is_null());
    }

    #[test]
    fn rules_read_field_locally() {
        let mut c = rr_dr60_settings_default(48_000);
        let mut v = false;
        // SAFETY: a valid settings structure and out.
        assert_eq!(
            unsafe { rr_dr60_settings_one_pass_vas_runs(&c, &mut v) },
            RrDr60Status::Ok
        );
        assert!(v);
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_settings_one_pass_codec_runs(&c, &mut v) },
            RrDr60Status::Ok
        );
        assert!(v);
        c.tap = 99;
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_settings_one_pass_vas_runs(&c, &mut v) },
            RrDr60Status::InvalidArgument
        );
        c.tap = 2; // after the limiter
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_settings_one_pass_vas_runs(&c, &mut v) },
            RrDr60Status::Ok
        );
        assert!(!v);
        // A switch byte of 2, written as bytes (never a `bool` of 2).
        let mut raw = [0u8; size_of::<RrDr60Settings>()];
        let c2 = rr_dr60_settings_default(48_000);
        // SAFETY: plain bytes of a valid structure.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (&raw const c2).cast::<u8>(),
                raw.as_mut_ptr(),
                raw.len(),
            );
        }
        raw[core::mem::offset_of!(RrDr60Settings, vas_enabled)] = 2;
        // SAFETY: the bytes are read field by field, never as a structure.
        assert_eq!(
            unsafe { rr_dr60_settings_one_pass_vas_runs(raw.as_ptr().cast(), &mut v) },
            RrDr60Status::InvalidArgument
        );
    }
}
