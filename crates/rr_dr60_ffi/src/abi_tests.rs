//! Spec 022, the pre-1.0 C ABI pass: the sizes, the published lists, the helper's cases, the
//! ABI check, the settings rule (D13), the per-call check order (D12) and the source scan.

use super::*;
use core::mem::offset_of;
use rr_dr60::{Settings, Tap, VasMode};

/// Every struct's 64-bit size (research R-03's table) and the offsets the pass pins.
#[test]
fn sizes_are_pinned() {
    assert_eq!(size_of::<RrDr60BlockInfo>(), 40);
    assert_eq!(size_of::<RrDr60RecordInfo>(), 48);
    assert_eq!(size_of::<RrDr60StopOutcome>(), 88);
    assert_eq!(size_of::<RrDr60PlayInfo>(), 48);
    assert_eq!(size_of::<RrDr60EngineState>(), 20);
    assert_eq!(size_of::<RrDr60MemoryStatus>(), 48);
    assert_eq!(size_of::<RrDr60SpeechCodecInfo>(), 28);
    assert_eq!(size_of::<RrDr60ManifestHeader>(), 72);
    assert_eq!(size_of::<RrDr60StageEntry>(), 80);
    assert_eq!(size_of::<RrDr60Control>(), 120);
    assert_eq!(size_of::<RrDr60ControlOption>(), 24);
    assert_eq!(size_of::<RrDr60Meters>(), 64);
    assert_eq!(size_of::<RrDr60RecordReadout>(), 376);
    assert_eq!(size_of::<RrDr60InputAlignment>(), 32);
    assert_eq!(size_of::<RrDr60PlaceState>(), 32);
    assert_eq!(size_of::<RrDr60VasEvent>(), 16);
    assert_eq!(size_of::<RrDr60ChangeEntry>(), 40);
    assert_eq!(size_of::<RrDr60StopFields>(), 40);
    assert_eq!(size_of::<RrDr60RunData>(), 80);
    assert_eq!(size_of::<RrDr60Settings>(), 120);
    assert_eq!(size_of::<RrDr60ControlReading>(), 24); // spec 025 R-02
    assert_eq!(offset_of!(RrDr60ControlReading, int_value), 8);
    assert_eq!(offset_of!(RrDr60ControlReading, float_value), 16);
    // The offsets the pass pins (D9, R-03).
    assert_eq!(offset_of!(RrDr60StopOutcome, kept), 48);
    assert_eq!(offset_of!(RrDr60StopOutcome, padding_start_sample), 80);
    assert_eq!(offset_of!(RrDr60StageEntry, number), 4);
    assert_eq!(offset_of!(RrDr60Control, int_min), 64);
    assert_eq!(offset_of!(RrDr60RecordReadout, stop), 216);
    assert_eq!(offset_of!(RrDr60RecordReadout, settings), 256);
    assert_eq!(offset_of!(RrDr60SpeechCodecInfo, frame_samples), 4);
    assert_eq!(offset_of!(RrDr60SpeechCodecInfo, model_version), 20);
    assert_eq!(offset_of!(RrDr60SpeechCodecInfo, reserved), 26);
    // `struct_size` is the first field of every sized struct.
    assert_eq!(offset_of!(RrDr60BlockInfo, struct_size), 0);
    assert_eq!(offset_of!(RrDr60Control, struct_size), 0);
    assert_eq!(offset_of!(RrDr60ControlOption, struct_size), 0);
    assert_eq!(offset_of!(RrDr60ManifestHeader, struct_size), 0);
}

/// Every published list is ascending and ends in the build target's `size_of` (R-01).
#[test]
fn published_lists_end_in_size_of() {
    fn check<T>(list: &[u32]) {
        assert!(list.windows(2).all(|w| w[0] < w[1]), "ascending");
        assert_eq!(*list.last().unwrap() as usize, size_of::<T>());
    }
    check::<RrDr60BlockInfo>(BLOCK_INFO_PUBLISHED);
    check::<RrDr60RecordInfo>(RECORD_INFO_PUBLISHED);
    check::<RrDr60StopOutcome>(STOP_OUTCOME_PUBLISHED);
    check::<RrDr60PlayInfo>(PLAY_INFO_PUBLISHED);
    check::<RrDr60EngineState>(ENGINE_STATE_PUBLISHED);
    check::<RrDr60MemoryStatus>(MEMORY_STATUS_PUBLISHED);
    check::<RrDr60SpeechCodecInfo>(SPEECH_CODEC_INFO_PUBLISHED);
    check::<RrDr60ManifestHeader>(MANIFEST_HEADER_PUBLISHED);
    check::<RrDr60StageEntry>(STAGE_ENTRY_PUBLISHED);
    check::<RrDr60Control>(CONTROL_PUBLISHED);
    check::<RrDr60ControlOption>(CONTROL_OPTION_PUBLISHED);
    check::<RrDr60Meters>(METERS_PUBLISHED);
    check::<RrDr60RecordReadout>(READOUT_PUBLISHED);
    check::<RrDr60InputAlignment>(ALIGNMENT_PUBLISHED);
    check::<RrDr60PlaceState>(PLACE_STATE_PUBLISHED);
    check::<RrDr60ControlReading>(READING_PUBLISHED);
    assert_eq!(READING_PUBLISHED, &[24]);
    assert_eq!(METERS_PUBLISHED, &[56, 64]);
    assert_eq!(READOUT_PUBLISHED, &[376]);
}

/// The four defines are literals pinned to the types (D2, D3, D8).
#[test]
fn defines_are_literal_and_pinned() {
    assert_eq!(RR_DR60_VAS_EVENT_SIZE, 16);
    assert_eq!(RR_DR60_VAS_EVENT_SIZE as usize, size_of::<RrDr60VasEvent>());
    assert_eq!(RR_DR60_CHANGE_ENTRY_SIZE, 40);
    assert_eq!(
        RR_DR60_CHANGE_ENTRY_SIZE as usize,
        size_of::<RrDr60ChangeEntry>()
    );
    assert_eq!(RR_DR60_ABI_VERSION, 1);
    assert_eq!(rr_dr60_abi_version(), RR_DR60_ABI_VERSION);
    assert_eq!(RR_DR60_MAX_STRUCT_SIZE, 4096);
    assert_eq!(RrDr60Status::AbiMismatch as u32, 37);
}

/// An 8-aligned painted buffer whose first four bytes carry `size`.
#[repr(C, align(8))]
struct Painted([u8; 4200]);

impl Painted {
    fn new(size: u32) -> Self {
        let mut p = Painted([0xAA; 4200]);
        p.0[..4].copy_from_slice(&size.to_ne_bytes());
        p
    }
    fn ptr<T>(&mut self) -> *mut T {
        self.0.as_mut_ptr().cast::<T>()
    }
    fn size(&self) -> u32 {
        u32::from_ne_bytes(self.0[..4].try_into().unwrap())
    }
    fn untouched_after(&self, n: usize) -> bool {
        self.0[n..].iter().all(|&b| b == 0xAA)
    }
    fn zero_in(&self, lo: usize, hi: usize) -> bool {
        self.0[lo..hi].iter().all(|&b| b == 0)
    }
}

/// The helper on Meters' two layouts: 55 refused, 56 … 63 read back 56, 64 and above read back 64
/// with a zero tail, the cap refused (R-01, D11).
#[test]
fn write_sized_meters_cases() {
    let build = |m: &mut RrDr60Meters, _filled: u32| {
        m.input_peak = 0.5;
        m.battery_ceiling = 1.0;
    };
    for (size, expect) in [
        (0u32, None),
        (55, None),
        (56, Some(56u32)),
        (60, Some(56)),
        (63, Some(56)),
        (64, Some(64)),
        (80, Some(64)),
        (4096, Some(64)),
        (4097, None),
        (u32::MAX, None),
    ] {
        let mut p = Painted::new(size);
        // SAFETY: a writable 4200-byte buffer; the helper reads the size and writes at most
        // `size` (≤ 4096) bytes.
        let st = unsafe { write_sized::<RrDr60Meters>(p.ptr(), METERS_PUBLISHED, build) };
        match expect {
            None => {
                assert_eq!(st, RrDr60Status::InvalidArgument, "size {size}");
                assert_eq!(p.size(), size);
                assert!(p.untouched_after(4), "nothing written at {size}");
            }
            Some(filled) => {
                assert_eq!(st, RrDr60Status::Ok, "size {size}");
                assert_eq!(p.size(), filled, "the read-back is the filled size");
                let f = filled as usize;
                let m = &p.0[..f];
                assert_eq!(f32::from_ne_bytes(m[4..8].try_into().unwrap()), 0.5);
                if filled == 64 {
                    assert_eq!(f32::from_ne_bytes(m[60..64].try_into().unwrap()), 1.0);
                }
                assert!(p.zero_in(f, size as usize), "the tail {f}..{size} is zero");
                assert!(p.untouched_after(size as usize), "beyond the caller's size");
            }
        }
    }
}

/// Every out-struct's four cases through the helper with a trivial build: below refused and
/// untouched; exact filled with the padding zero and the size read back; larger with a zero
/// tail; above the cap refused (FR-001, US1 AS1 – AS4).
#[test]
fn write_sized_every_out_struct_four_cases() {
    fn cases<T>(published: &[u32], padding: &[(usize, usize)]) {
        let full = *published.last().unwrap();
        for size in [0, published[0] - 1] {
            let mut p = Painted::new(size);
            // SAFETY: as above.
            let st = unsafe { write_sized::<T>(p.ptr(), published, |_, _| {}) };
            assert_eq!(st, RrDr60Status::InvalidArgument);
            assert!(p.untouched_after(4));
        }
        for size in [full, full + 16, 4096] {
            let mut p = Painted::new(size);
            // SAFETY: as above.
            let st = unsafe { write_sized::<T>(p.ptr(), published, |_, _| {}) };
            assert_eq!(st, RrDr60Status::Ok);
            assert_eq!(p.size(), full);
            for &(lo, hi) in padding {
                assert!(p.zero_in(lo, hi), "padding {lo}..{hi} zero");
            }
            assert!(p.zero_in(full as usize, size as usize));
            assert!(p.untouched_after(size as usize));
        }
        for size in [4097, u32::MAX] {
            let mut p = Painted::new(size);
            // SAFETY: as above.
            let st = unsafe { write_sized::<T>(p.ptr(), published, |_, _| {}) };
            assert_eq!(st, RrDr60Status::InvalidArgument);
            assert!(p.untouched_after(4));
        }
    }
    cases::<RrDr60BlockInfo>(BLOCK_INFO_PUBLISHED, padding_of("RrDr60BlockInfo"));
    cases::<RrDr60RecordInfo>(RECORD_INFO_PUBLISHED, padding_of("RrDr60RecordInfo"));
    cases::<RrDr60StopOutcome>(STOP_OUTCOME_PUBLISHED, padding_of("RrDr60StopOutcome"));
    cases::<RrDr60PlayInfo>(PLAY_INFO_PUBLISHED, padding_of("RrDr60PlayInfo"));
    cases::<RrDr60EngineState>(ENGINE_STATE_PUBLISHED, padding_of("RrDr60EngineState"));
    cases::<RrDr60MemoryStatus>(MEMORY_STATUS_PUBLISHED, padding_of("RrDr60MemoryStatus"));
    cases::<RrDr60SpeechCodecInfo>(
        SPEECH_CODEC_INFO_PUBLISHED,
        padding_of("RrDr60SpeechCodecInfo"),
    );
    cases::<RrDr60ManifestHeader>(
        MANIFEST_HEADER_PUBLISHED,
        padding_of("RrDr60ManifestHeader"),
    );
    cases::<RrDr60StageEntry>(STAGE_ENTRY_PUBLISHED, padding_of("RrDr60StageEntry"));
    cases::<RrDr60Control>(CONTROL_PUBLISHED, padding_of("RrDr60Control"));
    cases::<RrDr60ControlOption>(CONTROL_OPTION_PUBLISHED, padding_of("RrDr60ControlOption"));
    cases::<RrDr60Meters>(METERS_PUBLISHED, padding_of("RrDr60Meters"));
    cases::<RrDr60RecordReadout>(READOUT_PUBLISHED, padding_of("RrDr60RecordReadout"));
    cases::<RrDr60InputAlignment>(ALIGNMENT_PUBLISHED, padding_of("RrDr60InputAlignment"));
    cases::<RrDr60PlaceState>(PLACE_STATE_PUBLISHED, padding_of("RrDr60PlaceState"));
    cases::<RrDr60ControlReading>(READING_PUBLISHED, padding_of("RrDr60ControlReading"));
}

/// Every out-struct's padding runs on 64-bit targets (research R-03): the four-cases test and
/// the `Default` test read them.
fn padding_of(name: &str) -> &'static [(usize, usize)] {
    match name {
        "RrDr60BlockInfo" => &[(4, 8), (25, 32)],
        "RrDr60RecordInfo" => &[(4, 8), (26, 28)],
        "RrDr60StopOutcome" => &[(4, 8), (26, 28), (49, 52)],
        "RrDr60PlayInfo" => &[(4, 8), (17, 24)],
        "RrDr60EngineState" => &[(17, 20)],
        "RrDr60MemoryStatus" => &[(4, 8), (44, 48)],
        "RrDr60ManifestHeader" => &[(4, 8)],
        "RrDr60StageEntry" => &[(52, 56)], // before the 8-aligned `coverage` pointer
        "RrDr60Control" => &[(4, 8)],
        "RrDr60ControlOption" => &[(4, 8), (20, 24)],
        "RrDr60RecordReadout" => &READOUT_PADDING,
        _ => &[],
    }
}

/// The read-out's fifteen padding bytes: its own four before `stop`, and the embedded
/// settings' four implicit runs (`RrDr60Settings` offsets 10 – 11, 25 – 27, 49 – 51, 73 – 75,
/// at 256 + those). The hardware review's post-implementation B1.
const READOUT_PADDING: [(usize, usize); 5] =
    [(212, 216), (266, 268), (281, 284), (305, 308), (329, 332)];

/// The padding table follows from the field offsets (a moved field moves the table).
#[test]
fn readout_padding_table_follows_the_offsets() {
    let base = offset_of!(RrDr60RecordReadout, settings);
    assert_eq!(base, 256);
    assert_eq!(offset_of!(RrDr60RecordReadout, reserved_2) + 2, 212);
    assert_eq!(offset_of!(RrDr60RecordReadout, stop), 216);
    assert_eq!(
        base + offset_of!(RrDr60Settings, playback_stage_enabled) + 1,
        266
    );
    assert_eq!(base + offset_of!(RrDr60Settings, tap), 268);
    assert_eq!(base + offset_of!(RrDr60Settings, agc_enabled) + 1, 281);
    assert_eq!(base + offset_of!(RrDr60Settings, agc_knee_dbfs), 284);
    assert_eq!(base + offset_of!(RrDr60Settings, vas_enabled) + 1, 305);
    assert_eq!(base + offset_of!(RrDr60Settings, vas_mode), 308);
    assert_eq!(
        base + offset_of!(RrDr60Settings, adc_clamp_enabled) + 1,
        329
    );
    assert_eq!(base + offset_of!(RrDr60Settings, memory_capacity_s), 332);
}

/// A real read-out with non-default settings fills every padding byte zero (D10; the
/// post-implementation reviews' finding: a whole-struct assignment of the embedded settings
/// carried a temporary's padding). Under Miri a compare of an uninitialized byte is an error,
/// so every byte of the 376 is read.
#[test]
fn readout_real_settings_padding_zero() {
    let mut r = RecordReadout::default();
    r.settings = Settings::new(44_100);
    r.settings.tap = Tap::AfterMicAmp;
    r.settings.vas.mode = VasMode::Mute;
    r.settings.agc.knee_dbfs = -3.5;
    r.settings.seed = 0x5555_5555_5555_5555;
    r.settings.mic.hiss_offset_db = 2.5;
    r.recording_digest = 0xAAAA_AAAA_AAAA_AAAA;
    let mut p = Painted::new(376);
    // SAFETY: a writable buffer.
    assert_eq!(
        unsafe { write_readout(p.ptr(), READOUT_PUBLISHED, &r) },
        RrDr60Status::Ok
    );
    for (lo, hi) in READOUT_PADDING {
        assert!(p.zero_in(lo, hi), "padding {lo}..{hi}: {:?}", &p.0[lo..hi]);
    }
    let every: u64 = p.0[..376].iter().map(|&b| u64::from(b)).sum();
    assert!(every > 0, "every byte read; the struct is not all zero");
    // SAFETY: the first 376 bytes are a filled struct.
    let c = unsafe { p.ptr::<RrDr60RecordReadout>().read() };
    assert_eq!(c.settings.host_rate_hz, 44_100);
    assert_eq!(c.settings.tap, 10);
    assert_eq!(c.settings.vas_mode, 1);
    assert_eq!(c.settings.agc_knee_dbfs, -3.5);
    assert_eq!(c.settings.struct_size, SETTINGS_SIZE);
}

/// A prefix fill of the read-out carries the settings bytes the prefix holds in the embedded
/// `settings.struct_size` (R-04): a doctored list `[368, 376]` at 368.
#[test]
fn readout_prefix_fixes_embedded_settings() {
    let r = RecordReadout::default();
    let mut p = Painted::new(368);
    // SAFETY: a writable buffer.
    let st = unsafe { write_readout(p.ptr(), &[368, 376], &r) };
    assert_eq!(st, RrDr60Status::Ok);
    assert_eq!(p.size(), 368);
    let off = offset_of!(RrDr60RecordReadout, settings);
    let inner = u32::from_ne_bytes(p.0[off..off + 4].try_into().unwrap());
    assert_eq!(
        inner,
        368 - off as u32,
        "the embedded settings' size is the bytes held"
    );
    assert!(p.untouched_after(368));
    // The full fill writes the settings' own size.
    let mut q = Painted::new(376);
    // SAFETY: a writable buffer.
    let st = unsafe { write_readout(q.ptr(), READOUT_PUBLISHED, &r) };
    assert_eq!(st, RrDr60Status::Ok);
    let inner = u32::from_ne_bytes(q.0[off..off + 4].try_into().unwrap());
    assert_eq!(inner, SETTINGS_SIZE);
}

/// `Default` on the fifteen out-structs is the zeroed value plus the size (R-10).
#[test]
fn default_is_zeroed_plus_size() {
    // Every byte outside the padding runs is read (a typed value's padding is not observable),
    // so every field is zero but the size.
    fn check<T: Default>(name: &str) {
        let v = T::default();
        let n = size_of::<T>();
        let mut bytes = vec![0xAAu8; n];
        // SAFETY: `n` bytes are readable at `&v`; copied as bytes, never as a value.
        unsafe {
            core::ptr::copy_nonoverlapping((&v as *const T).cast::<u8>(), bytes.as_mut_ptr(), n)
        };
        let pads = padding_of(name);
        assert_eq!(
            u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as usize,
            n,
            "{name}"
        );
        // The padding bytes are never read (a typed value's padding is uninitialized; Miri
        // flags the read): only the indices outside the runs are.
        #[allow(clippy::needless_range_loop)] // the padding bytes must not be read at all
        for i in 4..n {
            if pads.iter().any(|&(lo, hi)| (lo..hi).contains(&i)) {
                continue;
            }
            assert_eq!(bytes[i], 0, "{name}: byte {i}");
        }
    }
    check::<RrDr60BlockInfo>("RrDr60BlockInfo");
    check::<RrDr60RecordInfo>("RrDr60RecordInfo");
    check::<RrDr60StopOutcome>("RrDr60StopOutcome");
    check::<RrDr60PlayInfo>("RrDr60PlayInfo");
    check::<RrDr60EngineState>("RrDr60EngineState");
    check::<RrDr60MemoryStatus>("RrDr60MemoryStatus");
    check::<RrDr60SpeechCodecInfo>("RrDr60SpeechCodecInfo");
    check::<RrDr60ManifestHeader>("RrDr60ManifestHeader");
    check::<RrDr60StageEntry>("RrDr60StageEntry");
    check::<RrDr60Control>("RrDr60Control");
    check::<RrDr60ControlOption>("RrDr60ControlOption");
    check::<RrDr60Meters>("RrDr60Meters");
    check::<RrDr60RecordReadout>("RrDr60RecordReadout");
    check::<RrDr60InputAlignment>("RrDr60InputAlignment");
    check::<RrDr60PlaceState>("RrDr60PlaceState");
    check::<RrDr60ControlReading>("RrDr60ControlReading");
}

const OUT_STRUCTS: [&str; 16] = [
    "RrDr60BlockInfo",
    "RrDr60RecordInfo",
    "RrDr60StopOutcome",
    "RrDr60PlayInfo",
    "RrDr60EngineState",
    "RrDr60MemoryStatus",
    "RrDr60SpeechCodecInfo",
    "RrDr60ManifestHeader",
    "RrDr60StageEntry",
    "RrDr60Control",
    "RrDr60ControlOption",
    "RrDr60Meters",
    "RrDr60RecordReadout",
    "RrDr60InputAlignment",
    "RrDr60PlaceState",
    "RrDr60ControlReading", // spec 025
];

/// The construction rule, mechanically (coach 5): no out-struct derives `Default`, and no
/// `extern "C"` function that takes one as an out-pointer assigns a whole value to it (every
/// write goes through `write_sized`).
fn scan_source(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut findings = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for name in OUT_STRUCTS {
            if line.trim_start() == format!("pub struct {name} {{") {
                let above = lines[i.saturating_sub(3)..i].join("\n");
                if above.contains("derive(") && above.contains("Default") {
                    findings.push(format!("{name} derives Default (line {})", i + 1));
                }
            }
        }
        // A filler (`fn fill…`) must assign fields, never a whole sub-struct.
        if line.trim_start().starts_with("fn fill") && !line.contains("fn fill_settings") {
            let end = lines[i..]
                .iter()
                .position(|l| l.trim_end() == "    }" || *l == "}")
                .map_or(lines.len(), |e| i + e);
            let body = lines[i..end].join("\n");
            for pat in ["= settings_to_c(", "::from_core("] {
                if body.contains(pat) {
                    findings.push(format!(
                        "{pat:?} (a whole-struct write) in the filler at line {}",
                        i + 1
                    ));
                }
            }
        }
        if line.starts_with("pub unsafe extern \"C\" fn ")
            || line.starts_with("pub extern \"C\" fn ")
        {
            // The function's text up to its closing brace at column 0.
            let end = lines[i..]
                .iter()
                .position(|l| *l == "}")
                .map_or(lines.len(), |e| i + e);
            let body = lines[i..end].join("\n");
            let sized_out = OUT_STRUCTS
                .iter()
                .any(|n| body.contains(&format!("*mut {n}")));
            if sized_out {
                for pat in [
                    "put(out, ",
                    "put(out_info, ",
                    "put(out_outcome, ",
                    "put(out_state, ",
                    "put(out_status, ",
                    "*out = ",
                    "*out_info = ",
                    "*out_state = ",
                    "*out_status = ",
                    "*out_outcome = ",
                ] {
                    if body.contains(pat) {
                        findings.push(format!("{pat:?} in the function at line {}", i + 1));
                    }
                }
            }
        }
    }
    findings
}

#[test]
fn no_out_struct_derives_default_and_no_whole_writes() {
    let findings = scan_source(include_str!("lib.rs"));
    assert!(findings.is_empty(), "{findings:?}");
    let findings = scan_source(include_str!("settings_read.rs"));
    assert!(findings.is_empty(), "{findings:?}");
    // The scan has teeth: a doctored source fails on both rules.
    let doctored = "#[repr(C)]\n#[derive(Clone, Copy, Default)]\npub struct RrDr60BlockInfo {\n}\npub unsafe extern \"C\" fn f(out: *mut RrDr60BlockInfo) {\n    unsafe { put(out, v) };\n}\n    fn fill(&mut self, r: &R) {\n        self.settings = settings_to_c(&r.settings);\n    }\n";
    let f = scan_source(doctored);
    assert_eq!(f.len(), 3, "{f:?}");
}

/// The ABI check: the right tuple, each argument wrong alone (the settings size one above and
/// one below), all wrong, version 0 and `u32::MAX` (D3, D13; SC-002).
#[test]
fn abi_check_tuple() {
    let ok = (1u32, 8usize, 120usize, 16usize, 40usize);
    let call = |t: (u32, usize, usize, usize, usize)| rr_dr60_abi_check(t.0, t.1, t.2, t.3, t.4);
    assert_eq!(call(ok), RrDr60Status::Ok);
    assert_eq!(call((2, 8, 120, 16, 40)), RrDr60Status::AbiMismatch);
    assert_eq!(call((0, 8, 120, 16, 40)), RrDr60Status::AbiMismatch);
    assert_eq!(call((u32::MAX, 8, 120, 16, 40)), RrDr60Status::AbiMismatch);
    assert_eq!(call((1, 4, 120, 16, 40)), RrDr60Status::AbiMismatch);
    assert_eq!(
        call((1, 8, 121, 16, 40)),
        RrDr60Status::AbiMismatch,
        "one above"
    );
    assert_eq!(
        call((1, 8, 119, 16, 40)),
        RrDr60Status::AbiMismatch,
        "one below"
    );
    assert_eq!(call((1, 8, 120, 24, 40)), RrDr60Status::AbiMismatch);
    assert_eq!(call((1, 8, 120, 16, 48)), RrDr60Status::AbiMismatch);
    assert_eq!(call((0, 4, 0, 0, 0)), RrDr60Status::AbiMismatch);
    assert_eq!(size_of::<*const u8>(), 8, "the ABI is 64-bit only");
}

/// D13: the settings struct is validated whole: 119 and 121 refused, 120 accepted, at
/// `create`, `reconfigure` and `settings_validate`.
#[test]
fn create_refuses_settings_size_119_and_121() {
    for size in [119u32, 121] {
        let mut s = rr_dr60_settings_default(48_000);
        s.struct_size = size;
        let mut field = RrDr60SettingField::None;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_settings_validate(&s, &mut field) },
            RrDr60Status::InvalidArgument,
            "{size}"
        );
        assert_eq!(field, RrDr60SettingField::StructSize);
        let mut h: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: valid pointers; `s` is a whole struct whatever its size field says.
        assert_eq!(
            unsafe { rr_dr60_create(&s, &mut h) },
            RrDr60Status::InvalidArgument
        );
        assert!(h.is_null());
        assert_eq!(
            to_settings(&s),
            Err((
                RrDr60Status::InvalidArgument,
                RrDr60SettingField::StructSize
            ))
        );
    }
    let ok = rr_dr60_settings_default(48_000);
    let mut h: *mut RrDr60Pipeline = core::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&ok, &mut h) }, RrDr60Status::Ok);
    let mut big = ok;
    big.struct_size = 121;
    // SAFETY: a live handle and a whole struct.
    assert_eq!(
        unsafe { rr_dr60_reconfigure(h, &big) },
        RrDr60Status::InvalidArgument
    );
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// The run input's cap on read (R-03): 4097 refused as 47 is.
#[test]
fn run_data_cap_on_read() {
    for size in [47u32, 4097] {
        let mut run = RrDr60RunData {
            struct_size: size,
            reserved: 0,
            events: core::ptr::null(),
            event_count: 0,
            changes: core::ptr::null(),
            change_count: 0,
            stop: RrDr60StopFields::default(),
        };
        run.stop.struct_size = STOP_FIELDS_SIZE;
        // SAFETY: a valid struct.
        assert_eq!(
            unsafe { run_in(&run) }.err(),
            Some(RrDr60Status::InvalidArgument),
            "{size}"
        );
    }
    let mut run = RrDr60RunData {
        struct_size: RUN_DATA_SIZE,
        reserved: 0,
        events: core::ptr::null(),
        event_count: 0,
        changes: core::ptr::null(),
        change_count: 0,
        stop: RrDr60StopFields::default(),
    };
    run.stop.struct_size = 7; // frozen: ignored on read
    run.stop.how = RR_DR60_STOP_CALL;
    // SAFETY: a valid struct.
    assert!(unsafe { run_in(&run) }.is_ok());
    let mut sf = RrDr60StopFields::default();
    sf.fill(&StopFields::default());
    assert_eq!(sf.struct_size, 40);
}

fn handle() -> *mut RrDr60Pipeline {
    let s = rr_dr60_settings_default(48_000);
    let mut h: *mut RrDr60Pipeline = core::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
    h
}

/// The per-call report state machine (D12, FR-005) on every family × size: a NULL or poisoned
/// handle keeps its status and writes the idle report only with a valid size; a live handle
/// refuses a bad size before anything else and writes nothing; a later error writes the idle
/// report with the tail zeroed.
#[test]
#[cfg(feature = "ffi-test-panic")]
fn per_call_check_order_matrix() {
    let input = vec![0.1f32; 64];
    let mut output = vec![0.0f32; 64];
    let mut ev = [RrDr60VasEvent::default(); 8];
    let mut ch = [RrDr60ChangeEntry::default(); 8];
    let sizes = |full: u32| {
        [
            (full, true),
            (0, false),
            (full - 1, false),
            (full + 16, true),
            (4097, false),
        ]
    };

    // process family
    for (size, valid) in sizes(40) {
        let mut p = Painted::new(size);
        // SAFETY: NULL handle allowed; a writable report buffer.
        let st = unsafe {
            rr_dr60_process(
                core::ptr::null_mut(),
                input.as_ptr(),
                output.as_mut_ptr(),
                64,
                p.ptr(),
            )
        };
        assert_eq!(st, RrDr60Status::NullPointer, "{size}");
        if valid {
            assert_eq!(p.size(), 40);
            assert!(p.zero_in(4, size as usize));
        } else {
            assert!(p.untouched_after(4));
        }
        let h = handle();
        // SAFETY: a live handle.
        unsafe { rr_dr60__test_force_panic(h) };
        let mut q = Painted::new(size);
        // SAFETY: as above.
        let st = unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 64, q.ptr()) };
        // A refused size returns before the processing (and before the forced panic).
        assert_eq!(
            st,
            if valid {
                RrDr60Status::InternalError
            } else {
                RrDr60Status::InvalidArgument
            },
            "panic at {size}"
        );
        if !valid {
            let mut ok = Painted::new(40);
            // SAFETY: as above; this call reaches the forced panic and poisons the handle.
            assert_eq!(
                unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 64, ok.ptr()) },
                RrDr60Status::InternalError
            );
        }
        let mut q2 = Painted::new(size);
        // SAFETY: as above; the handle is poisoned now.
        let st = unsafe {
            rr_dr60_process_with_events(
                h,
                input.as_ptr(),
                output.as_mut_ptr(),
                64,
                q2.ptr(),
                ev.as_mut_ptr(),
                8,
            )
        };
        assert_eq!(st, RrDr60Status::InternalError, "poisoned at {size}");
        if valid {
            assert_eq!(q2.size(), 40);
        } else {
            assert!(q2.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
        let h = handle();
        let mut z = Painted::new(size);
        let mut count = 0usize;
        // SAFETY: as above.
        let st = unsafe {
            rr_dr60_process_with_events_and_changes(
                h,
                input.as_ptr(),
                output.as_mut_ptr(),
                0,
                z.ptr(),
                ev.as_mut_ptr(),
                8,
                ch.as_mut_ptr(),
                8,
                &mut count,
            )
        };
        assert_eq!(
            st,
            if valid {
                RrDr60Status::Ok
            } else {
                RrDr60Status::InvalidArgument
            },
            "zero frames at {size}"
        );
        if valid {
            assert_eq!(z.size(), 40);
        } else {
            assert!(z.untouched_after(4));
        }
        // overlap
        let mut o = Painted::new(size);
        // SAFETY: the overlap is the case under test; the slices are not dereferenced.
        let st =
            unsafe { rr_dr60_process(h, output.as_ptr(), output.as_mut_ptr().add(1), 63, o.ptr()) };
        assert_eq!(st, RrDr60Status::InvalidArgument, "overlap at {size}");
        if valid {
            assert_eq!(o.size(), 40);
        } else {
            assert!(o.untouched_after(4));
        }
        // recording in progress
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        let mut r = Painted::new(size);
        // SAFETY: as above.
        let st = unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 64, r.ptr()) };
        assert_eq!(
            st,
            if valid {
                RrDr60Status::RecordingInProgress
            } else {
                RrDr60Status::InvalidArgument
            }
        );
        if valid {
            assert_eq!(r.size(), 40);
        } else {
            assert!(r.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }

    // PlaybackInProgress on the one-pass, each size (coach 7).
    {
        let loud = vec![0.5f32; 4096];
        let mut out = vec![0.0f32; 4096];
        let h = handle();
        // SAFETY: a live handle and valid buffers; a kept recording for the playback.
        unsafe {
            assert_eq!(rr_dr60_start_recording(h), RrDr60Status::Ok);
            let mut ri = RrDr60RecordInfo::default();
            assert_eq!(
                rr_dr60_record(h, loud.as_ptr(), 4096, &mut ri, core::ptr::null_mut(), 0),
                RrDr60Status::Ok
            );
            let mut so = RrDr60StopOutcome::default();
            assert_eq!(
                rr_dr60_stop_recording(h, &mut so, core::ptr::null_mut(), 0),
                RrDr60Status::Ok
            );
            assert!(so.kept);
            assert_eq!(rr_dr60_start_playback(h, 1), RrDr60Status::Ok);
        }
        for (size, valid) in sizes(40) {
            let mut b = Painted::new(size);
            // SAFETY: as above.
            let st = unsafe { rr_dr60_process(h, loud.as_ptr(), out.as_mut_ptr(), 64, b.ptr()) };
            assert_eq!(
                st,
                if valid {
                    RrDr60Status::PlaybackInProgress
                } else {
                    RrDr60Status::InvalidArgument
                },
                "playing at {size}"
            );
            if valid {
                assert_eq!(b.size(), 40);
            } else {
                assert!(b.untouched_after(4));
            }
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
    // Poisoned handle on the stop and play families, each size (coach 7): poison through a
    // valid-size process call, then the report call.
    for (size, valid) in sizes(88) {
        let h = handle();
        // SAFETY: a live handle.
        unsafe { rr_dr60__test_force_panic(h) };
        let mut ok = Painted::new(40);
        // SAFETY: as above; poisons.
        assert_eq!(
            unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 64, ok.ptr()) },
            RrDr60Status::InternalError
        );
        let mut b = Painted::new(size);
        // SAFETY: a poisoned handle.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(h, b.ptr(), ev.as_mut_ptr(), 8) },
            RrDr60Status::InternalError,
            "poisoned stop at {size}"
        );
        if valid {
            assert_eq!(b.size(), 88);
        } else {
            assert!(b.untouched_after(4));
        }
        let mut pb = Painted::new(if size == 88 {
            48
        } else if size == 87 {
            47
        } else if size == 104 {
            64
        } else {
            size
        });
        let psize = pb.size();
        // SAFETY: a poisoned handle.
        assert_eq!(
            unsafe { rr_dr60_play(h, output.as_mut_ptr(), 64, pb.ptr()) },
            RrDr60Status::InternalError,
            "poisoned play at {psize}"
        );
        if valid {
            assert_eq!(pb.size(), 48);
        } else {
            assert!(pb.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
    // record family (48) and stop (88)
    for (size, valid) in sizes(48) {
        let mut p = Painted::new(size);
        // SAFETY: NULL handle allowed.
        let st = unsafe {
            rr_dr60_record(
                core::ptr::null_mut(),
                input.as_ptr(),
                64,
                p.ptr(),
                ev.as_mut_ptr(),
                8,
            )
        };
        assert_eq!(st, RrDr60Status::NullPointer);
        if valid {
            assert_eq!(p.size(), 48);
            assert!(p.zero_in(4, size as usize));
        } else {
            assert!(p.untouched_after(4));
        }
        let h = handle();
        let mut n = Painted::new(size);
        let mut count = 0usize;
        let mut total = 0u64;
        // SAFETY: a live handle, not recording.
        let st = unsafe {
            rr_dr60_record_with_changes(
                h,
                input.as_ptr(),
                64,
                n.ptr(),
                ev.as_mut_ptr(),
                8,
                ch.as_mut_ptr(),
                8,
                &mut count,
                &mut total,
            )
        };
        assert_eq!(
            st,
            if valid {
                RrDr60Status::NotRecording
            } else {
                RrDr60Status::InvalidArgument
            }
        );
        if valid {
            assert_eq!(n.size(), 48);
        } else {
            assert!(n.untouched_after(4));
        }
        // SAFETY: a live handle.
        unsafe { rr_dr60__test_force_panic(h) };
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        let mut ok = Painted::new(48);
        // SAFETY: as above; reaches the forced panic and poisons the handle.
        assert_eq!(
            unsafe { rr_dr60_record(h, input.as_ptr(), 64, ok.ptr(), ev.as_mut_ptr(), 8) },
            RrDr60Status::InternalError
        );
        let mut q = Painted::new(size);
        // SAFETY: as above; the handle is poisoned.
        let st = unsafe { rr_dr60_record(h, input.as_ptr(), 64, q.ptr(), ev.as_mut_ptr(), 8) };
        assert_eq!(st, RrDr60Status::InternalError, "poisoned at {size}");
        if valid {
            assert_eq!(q.size(), 48);
        } else {
            assert!(q.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
    for (size, valid) in sizes(88) {
        let h = handle();
        let mut p = Painted::new(size);
        // SAFETY: a live handle, not recording.
        let st = unsafe { rr_dr60_stop_recording(h, p.ptr(), ev.as_mut_ptr(), 8) };
        assert_eq!(
            st,
            if valid {
                RrDr60Status::NotRecording
            } else {
                RrDr60Status::InvalidArgument
            }
        );
        if valid {
            assert_eq!(p.size(), 88);
            assert!(p.zero_in(4, size as usize));
        } else {
            assert!(p.untouched_after(4));
        }
        let mut n = Painted::new(size);
        let mut count = 0usize;
        let mut total = 0u64;
        // SAFETY: NULL handle allowed.
        let st = unsafe {
            rr_dr60_stop_recording_with_changes(
                core::ptr::null_mut(),
                n.ptr(),
                ev.as_mut_ptr(),
                8,
                ch.as_mut_ptr(),
                8,
                &mut count,
                &mut total,
            )
        };
        assert_eq!(st, RrDr60Status::NullPointer);
        if valid {
            assert_eq!(n.size(), 88);
        } else {
            assert!(n.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
    // play family (48)
    for (size, valid) in sizes(48) {
        let h = handle();
        let mut p = Painted::new(size);
        // SAFETY: a live handle, not playing.
        let st = unsafe { rr_dr60_play(h, output.as_mut_ptr(), 64, p.ptr()) };
        assert_eq!(
            st,
            if valid {
                RrDr60Status::NotPlaying
            } else {
                RrDr60Status::InvalidArgument
            }
        );
        if valid {
            assert_eq!(p.size(), 48);
        } else {
            assert!(p.untouched_after(4));
        }
        let mut n = Painted::new(size);
        let mut count = 0usize;
        // SAFETY: NULL handle allowed.
        let st = unsafe {
            rr_dr60_play_with_changes(
                core::ptr::null_mut(),
                output.as_mut_ptr(),
                64,
                n.ptr(),
                ch.as_mut_ptr(),
                8,
                &mut count,
            )
        };
        assert_eq!(st, RrDr60Status::NullPointer);
        if valid {
            assert_eq!(n.size(), 48);
        } else {
            assert!(n.untouched_after(4));
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// A refused per-call report processes nothing: the stream positions, the memory status and
/// the meters are byte-equal before and after (US1 AS1).
#[test]
fn refused_report_processes_nothing() {
    let h = handle();
    let input = vec![0.5f32; 4096];
    let mut output = vec![0.0f32; 4096];
    let snapshot = |h: *mut RrDr60Pipeline| {
        let mut pos = [0u64; 2];
        // SAFETY: a live handle.
        unsafe {
            rr_dr60_stream_position(h, RrDr60Stream::OnePass as u32, &mut pos[0]);
            rr_dr60_input_position(h, RrDr60Stream::OnePass as u32, &mut pos[1]);
        }
        let mut ms = RrDr60MemoryStatus::default();
        let mut m = RrDr60Meters::default();
        // SAFETY: a live handle.
        unsafe {
            rr_dr60_memory_status(h, &mut ms);
            rr_dr60_meters(h, &mut m);
        }
        (pos, ms, m)
    };
    // One real block so the meters are not at rest.
    let mut info = RrDr60BlockInfo::default();
    // SAFETY: valid buffers.
    assert_eq!(
        unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 4096, &mut info) },
        RrDr60Status::Ok
    );
    let before = snapshot(h);
    let mut bad = Painted::new(39);
    // SAFETY: valid buffers; the report is refused.
    assert_eq!(
        unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 4096, bad.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut bad48 = Painted::new(4097);
    // SAFETY: as above.
    assert_eq!(
        unsafe {
            rr_dr60_record(
                h,
                input.as_ptr(),
                4096,
                bad48.ptr(),
                core::ptr::null_mut(),
                0,
            )
        },
        RrDr60Status::InvalidArgument
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_play(h, output.as_mut_ptr(), 4096, bad48.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let after = snapshot(h);
    assert_eq!(before, after);
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// The handle-taking status functions keep 0.18's order (coach 6): `live`, then NULL out, then
/// the size; the constant-data functions check NULL, the size, then the index.
#[test]
fn status_and_constant_data_order() {
    let mut st = Painted::new(19);
    // SAFETY: NULL handle allowed.
    assert_eq!(
        unsafe { rr_dr60_get_state(core::ptr::null_mut(), st.ptr()) },
        RrDr60Status::NullPointer
    );
    assert!(st.untouched_after(4));
    let h = handle();
    // SAFETY: a live handle.
    assert_eq!(
        unsafe { rr_dr60_get_state(h, core::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_get_state(h, st.ptr()) },
        RrDr60Status::InvalidArgument
    );
    assert!(st.untouched_after(4));
    let mut ms = Painted::new(4097);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_memory_status(h, ms.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut ok = Painted::new(48 + 8);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_memory_status(h, ok.ptr()) },
        RrDr60Status::Ok
    );
    assert_eq!(ok.size(), 48);
    assert!(ok.zero_in(48, 56));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
    // Constant data: a bad index with a bad size reports the size.
    let mut e = Painted::new(79);
    // SAFETY: a writable buffer.
    assert_eq!(
        unsafe { rr_dr60_manifest_stage(99, e.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut e2 = Painted::new(80);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_manifest_stage(99, e2.ptr()) },
        RrDr60Status::NoSuchStage
    );
    assert!(e2.untouched_after(4));
    let mut c = Painted::new(119);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_control(999, c.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut c2 = Painted::new(120);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_control(999, c2.ptr()) },
        RrDr60Status::NoSuchControl
    );
    let mut o = Painted::new(23);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_control_option(0, 0, o.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut hd = Painted::new(72 + 8);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_manifest_header(hd.ptr()) },
        RrDr60Status::Ok
    );
    assert_eq!(hd.size(), 72);
    assert!(hd.zero_in(72, 80));
    let mut ci = Painted::new(27);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_speech_codec_info(ci.ptr()) },
        RrDr60Status::InvalidArgument
    );
    let mut ci2 = Painted::new(28);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_speech_codec_info(ci2.ptr()) },
        RrDr60Status::Ok
    );
    assert_eq!(ci2.size(), 28);
}

// ---- Spec 025: the settings read and the control reading (the Miri filter
// `abi_tests::settings_read abi_tests::control_reading`). -------------------------------------

/// The settings struct's four implicit padding runs (64-bit; READOUT_PADDING's embedded runs).
const SETTINGS_PADDING: [(usize, usize); 4] = [(10, 12), (25, 28), (49, 52), (73, 76)];

/// `rr_dr60_get_settings`: the handle first, then the pointer, then the size (equal to the
/// settings size); on success the structure in place, its padding zero, nothing past it.
#[test]
fn settings_read_order_and_fill() {
    let mut b = Painted::new(120);
    // SAFETY: a NULL handle is refused.
    assert_eq!(
        unsafe { rr_dr60_get_settings(core::ptr::null_mut(), b.ptr()) },
        RrDr60Status::NullPointer
    );
    assert!(b.untouched_after(4));
    let h = handle();
    // SAFETY: a live handle, a NULL out.
    assert_eq!(
        unsafe { rr_dr60_get_settings(h, core::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    for size in [0, 119, 121, 4096] {
        let mut b = Painted::new(size);
        // SAFETY: a live handle, a painted buffer.
        assert_eq!(
            unsafe { rr_dr60_get_settings(h, b.ptr()) },
            RrDr60Status::InvalidArgument,
            "{size}"
        );
        assert!(b.untouched_after(4));
    }
    let mut b = Painted::new(120);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_get_settings(h, b.ptr()) },
        RrDr60Status::Ok
    );
    for (lo, hi) in SETTINGS_PADDING {
        assert!(b.zero_in(lo, hi), "padding {lo}..{hi}");
    }
    assert!(b.untouched_after(120));
    // SAFETY: the buffer holds a filled settings struct now (8-aligned, 120 bytes).
    let got = unsafe { b.ptr::<RrDr60Settings>().read() };
    assert_eq!(to_settings(&got).unwrap(), Settings::new(48_000));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// `rr_dr60_control_value`: handle, pointer, size, index; a larger size filled to 24 with the
/// tail zero.
#[test]
fn control_reading_engine_order() {
    let mut b = Painted::new(24);
    // SAFETY: a NULL handle.
    assert_eq!(
        unsafe { rr_dr60_control_value(core::ptr::null_mut(), 0, b.ptr()) },
        RrDr60Status::NullPointer
    );
    let h = handle();
    // SAFETY: a NULL out.
    assert_eq!(
        unsafe { rr_dr60_control_value(h, 0, core::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    for size in [0, 23, 4097] {
        let mut b = Painted::new(size);
        // SAFETY: a painted buffer; a bad size wins over a bad index.
        assert_eq!(
            unsafe { rr_dr60_control_value(h, 99, b.ptr()) },
            RrDr60Status::InvalidArgument
        );
        assert!(b.untouched_after(4));
    }
    let mut b = Painted::new(24);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_control_value(h, 31, b.ptr()) },
        RrDr60Status::NoSuchControl
    );
    assert!(b.untouched_after(4));
    let mut b = Painted::new(32);
    // SAFETY: as above; the volume.
    assert_eq!(
        unsafe { rr_dr60_control_value(h, 1, b.ptr()) },
        RrDr60Status::Ok
    );
    assert_eq!(b.size(), 24);
    assert!(b.zero_in(24, 32));
    assert!(b.untouched_after(32));
    // SAFETY: 24 bytes filled at an 8-aligned address.
    let r = unsafe { b.ptr::<RrDr60ControlReading>().read() };
    assert_eq!(r.value_type, RrDr60ControlValueType::Continuous as u32);
    assert_eq!(r.float_value, rr_dr60::output::VOLUME_REFERENCE);
    assert_eq!((r.int_value, r.reserved), (0, 0));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

fn reading(
    value_type: RrDr60ControlValueType,
    int_value: u64,
    float_value: f32,
) -> RrDr60ControlReading {
    RrDr60ControlReading {
        value_type: value_type as u32,
        int_value,
        float_value,
        ..Default::default()
    }
}

/// The settings-structure functions read and write only the field: on a structure painted
/// 0xAA (every switch byte invalid, `tap` and `vas_mode` not options) every write by index
/// changes exactly the field's bytes, and a read of a switch byte 0xAA (or 2) is refused.
#[test]
fn control_reading_field_local() {
    for (i, f) in settings_read::C_FIELDS.iter().enumerate() {
        let Some((off, kind)) = *f else { continue };
        let i = i as u32;
        let mut b = Painted::new(120);
        let r = match kind {
            settings_read::CKind::Switch => reading(RrDr60ControlValueType::Switch, 1, 0.0),
            settings_read::CKind::Int32 | settings_read::CKind::Int64 => {
                reading(RrDr60ControlValueType::Integer, 3, 0.0)
            }
            settings_read::CKind::Float => reading(RrDr60ControlValueType::Continuous, 0, 1.5),
            settings_read::CKind::Choice if i == 3 => {
                reading(RrDr60ControlValueType::Choice, 8000, 0.0)
            }
            settings_read::CKind::Choice => reading(RrDr60ControlValueType::Choice, 1, 0.0),
        };
        let width = match kind {
            settings_read::CKind::Switch => 1,
            settings_read::CKind::Int64 => 8,
            _ => 4,
        };
        let before = b.0;
        // SAFETY: a 120-byte settings buffer and a local reading.
        assert_eq!(
            unsafe { rr_dr60_set_settings_control_value(b.ptr(), i, &r) },
            RrDr60Status::Ok,
            "{i}"
        );
        for (k, (x, y)) in before.iter().zip(b.0.iter()).enumerate() {
            if !(off..off + width).contains(&k) {
                assert_eq!(x, y, "index {i}: byte {k} outside the field moved");
            }
        }
        let mut out = RrDr60ControlReading::default();
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_settings_control_value(b.ptr(), i, &mut out) },
            RrDr60Status::Ok
        );
        assert_eq!(
            (out.value_type, out.int_value, out.float_value),
            (r.value_type, r.int_value, r.float_value)
        );
        if kind == settings_read::CKind::Switch {
            for bad in [2u8, 0xAA] {
                b.0[off] = bad;
                let mut out = Painted::new(24);
                // SAFETY: as above.
                assert_eq!(
                    unsafe { rr_dr60_settings_control_value(b.ptr(), i, out.ptr()) },
                    RrDr60Status::InvalidArgument
                );
                assert!(out.untouched_after(4));
            }
        }
        if kind == settings_read::CKind::Choice {
            b.0[off..off + 4].copy_from_slice(&99u32.to_ne_bytes());
            let mut out = Painted::new(24);
            // SAFETY: as above.
            assert_eq!(
                unsafe { rr_dr60_settings_control_value(b.ptr(), i, out.ptr()) },
                RrDr60Status::InvalidArgument
            );
        }
    }
}

/// The write's refusal order, nothing written on any (D7).
#[test]
fn control_reading_settings_write_order() {
    let ok = reading(RrDr60ControlValueType::Continuous, 0, -3.0);
    let mut s = rr_dr60_settings_default(48_000);
    let snapshot = |s: &RrDr60Settings| to_settings(s).unwrap();
    let base = snapshot(&s);
    // SAFETY: NULL pointers refused.
    unsafe {
        assert_eq!(
            rr_dr60_set_settings_control_value(core::ptr::null_mut(), 10, &ok),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 10, core::ptr::null()),
            RrDr60Status::NullPointer
        );
    }
    s.struct_size = 119;
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut s, 99, &ok) },
        RrDr60Status::InvalidArgument
    );
    s.struct_size = 120;
    for size in [0, 23, 4097] {
        let bad = RrDr60ControlReading {
            struct_size: size,
            ..ok
        };
        // SAFETY: local values; a bad reading size wins over a bad index.
        assert_eq!(
            unsafe { rr_dr60_set_settings_control_value(&mut s, 99, &bad) },
            RrDr60Status::InvalidArgument
        );
    }
    // SAFETY: local values.
    unsafe {
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 31, &ok),
            RrDr60Status::NoSuchControl
        );
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 1, &ok),
            RrDr60Status::NotASetting
        );
        let wrong = reading(RrDr60ControlValueType::Integer, 3, 0.0);
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 10, &wrong),
            RrDr60Status::InvalidArgument
        );
        let nan = reading(RrDr60ControlValueType::Continuous, 0, f32::NAN);
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 10, &nan),
            RrDr60Status::InvalidArgument
        );
        let wide = reading(RrDr60ControlValueType::Integer, 256, 0.0);
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 15, &wide),
            RrDr60Status::InvalidArgument
        );
        let switch2 = reading(RrDr60ControlValueType::Switch, 2, 0.0);
        assert_eq!(
            rr_dr60_set_settings_control_value(&mut s, 9, &switch2),
            RrDr60Status::InvalidArgument
        );
    }
    assert_eq!(snapshot(&s), base);
    // A larger reading is read for its first 24 bytes; −0.0 written as 0.0.
    let neg = RrDr60ControlReading {
        struct_size: 4096,
        ..reading(RrDr60ControlValueType::Continuous, 0, -0.0)
    };
    let mut big = [0u8; 4096];
    // SAFETY: `neg` is plain data; 24 bytes copied into a 4096-byte buffer.
    unsafe {
        core::ptr::copy_nonoverlapping(
            (&neg as *const RrDr60ControlReading).cast::<u8>(),
            big.as_mut_ptr(),
            24,
        )
    };
    // SAFETY: a 4096-byte readable reading.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut s, 10, big.as_ptr().cast()) },
        RrDr60Status::Ok
    );
    assert_eq!(s.agc_knee_dbfs.to_bits(), 0.0f32.to_bits());
}

/// The pointers may alias: a reading laid over the target field is copied before the write.
#[test]
fn control_reading_alias() {
    let mut b = Painted::new(120);
    let off = core::mem::offset_of!(RrDr60Settings, memory_capacity_s);
    let r = RrDr60ControlReading {
        struct_size: 24,
        ..reading(RrDr60ControlValueType::Integer, 1234, 0.0)
    };
    // SAFETY: the reading's 24 bytes copied into the buffer at the field's offset (in bounds).
    unsafe {
        core::ptr::copy_nonoverlapping(
            (&r as *const RrDr60ControlReading).cast::<u8>(),
            b.0.as_mut_ptr().add(off),
            24,
        )
    };
    // SAFETY: the reading lies inside the settings buffer.
    let st =
        unsafe { rr_dr60_set_settings_control_value(b.ptr(), 19, b.0.as_ptr().add(off).cast()) };
    assert_eq!(st, RrDr60Status::Ok);
    assert_eq!(
        u32::from_ne_bytes(b.0[off..off + 4].try_into().unwrap()),
        1234
    );
}

/// `rr_dr60_set_control_value`: handle, pointer, size, index, class, representability, then
/// the setter's status; nothing changed on any refusal.
#[test]
fn control_reading_live_write_order() {
    let vol = reading(RrDr60ControlValueType::Continuous, 0, 0.5);
    // SAFETY: a NULL handle.
    assert_eq!(
        unsafe { rr_dr60_set_control_value(core::ptr::null_mut(), 1, &vol) },
        RrDr60Status::NullPointer
    );
    let h = handle();
    // SAFETY: a live handle and local readings throughout.
    unsafe {
        assert_eq!(
            rr_dr60_set_control_value(h, 1, core::ptr::null()),
            RrDr60Status::NullPointer
        );
        let small = RrDr60ControlReading {
            struct_size: 23,
            ..vol
        };
        assert_eq!(
            rr_dr60_set_control_value(h, 99, &small),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_set_control_value(h, 31, &vol),
            RrDr60Status::NoSuchControl
        );
        assert_eq!(
            rr_dr60_set_control_value(h, 10, &vol),
            RrDr60Status::NotAPlainLiveControl
        );
        let int = reading(RrDr60ControlValueType::Integer, 1, 0.0);
        assert_eq!(
            rr_dr60_set_control_value(h, 1, &int),
            RrDr60Status::InvalidArgument
        );
        let nan = reading(RrDr60ControlValueType::Continuous, 0, f32::NAN);
        assert_eq!(
            rr_dr60_set_control_value(h, 1, &nan),
            RrDr60Status::InvalidArgument
        );
        let loud = reading(RrDr60ControlValueType::Continuous, 0, 1.5);
        assert_eq!(
            rr_dr60_set_control_value(h, 1, &loud),
            RrDr60Status::InvalidVolume
        );
        let lvl = reading(RrDr60ControlValueType::Integer, 6, 0.0);
        assert_eq!(
            rr_dr60_set_control_value(h, 2, &lvl),
            RrDr60Status::InvalidSpeedLevel
        );
        let bat = reading(RrDr60ControlValueType::Continuous, 0, 2.0);
        assert_eq!(
            rr_dr60_set_control_value(h, 29, &bat),
            RrDr60Status::InvalidBatteryVolts
        );
        let mut v = 0.0f32;
        assert_eq!(rr_dr60_get_volume(h, &mut v), RrDr60Status::Ok);
        assert_eq!(v, rr_dr60::output::VOLUME_REFERENCE);
        assert_eq!(rr_dr60_set_control_value(h, 1, &vol), RrDr60Status::Ok);
        assert_eq!(rr_dr60_get_volume(h, &mut v), RrDr60Status::Ok);
        assert_eq!(v, 0.5);
        rr_dr60_destroy(h);
    }
}

/// A poisoned handle keeps its status on the three engine functions, nothing written.
#[test]
#[cfg(feature = "ffi-test-panic")]
fn settings_read_poisoned() {
    let input = vec![0.1f32; 64];
    let mut output = vec![0.0f32; 64];
    let h = handle();
    // SAFETY: a live handle; the forced panic poisons it.
    unsafe { rr_dr60__test_force_panic(h) };
    let mut ok = Painted::new(40);
    // SAFETY: as above.
    assert_eq!(
        unsafe { rr_dr60_process(h, input.as_ptr(), output.as_mut_ptr(), 64, ok.ptr()) },
        RrDr60Status::InternalError
    );
    let mut b = Painted::new(120);
    let mut r = Painted::new(24);
    let vol = reading(RrDr60ControlValueType::Continuous, 0, 0.5);
    // SAFETY: a poisoned handle.
    unsafe {
        assert_eq!(
            rr_dr60_get_settings(h, b.ptr()),
            RrDr60Status::InternalError
        );
        assert_eq!(
            rr_dr60_control_value(h, 1, r.ptr()),
            RrDr60Status::InternalError
        );
        assert_eq!(
            rr_dr60_set_control_value(h, 1, &vol),
            RrDr60Status::InternalError
        );
    }
    assert!(b.untouched_after(4));
    assert!(r.untouched_after(4));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}
