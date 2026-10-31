//! Spec 022's edge cases, one test each (tasks.md T014).

use rr_dr60_ffi::*;

fn handle() -> *mut RrDr60Pipeline {
    let s = rr_dr60_settings_default(48_000);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
    h
}

#[repr(C, align(8))]
struct Painted([u8; 4200]);

fn paint(size: u32) -> Painted {
    let mut b = Painted([0xAA; 4200]);
    b.0[..4].copy_from_slice(&size.to_ne_bytes());
    b
}

/// `struct_size` 0 (a zero-initialized struct, Swift's default initializer) is refused.
#[test]
fn size_zero_refused() {
    let h = handle();
    let x = [0.1f32; 64];
    let mut y = [0.0f32; 64];
    let mut info = RrDr60BlockInfo {
        struct_size: 0,
        ..Default::default()
    };
    // SAFETY: a live handle and valid buffers.
    let st = unsafe { rr_dr60_process(h, x.as_ptr(), y.as_mut_ptr(), 64, &mut info) };
    assert_eq!(st, RrDr60Status::InvalidArgument);
    assert_eq!(info.struct_size, 0);
    assert_eq!(info.produced, 0);
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// A `struct_size` above the cap (an uninitialized C stack struct reads garbage there) is
/// refused and nothing is written.
#[test]
fn size_above_cap_refused() {
    let h = handle();
    for size in [4097u32, 0x1234_5678, u32::MAX] {
        let mut b = paint(size);
        // SAFETY: a live handle; the report is refused before any write.
        let st = unsafe { rr_dr60_get_state(h, b.0.as_mut_ptr().cast()) };
        assert_eq!(st, RrDr60Status::InvalidArgument, "{size}");
        assert!(b.0[4..].iter().all(|&v| v == 0xAA));
    }
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// A refused size on a per-call report leaves the engine's state: the positions, the memory
/// status and the meters.
#[test]
fn refused_report_leaves_state() {
    let h = handle();
    let x = vec![0.5f32; 4096];
    let mut y = vec![0.0f32; 4096];
    let mut info = RrDr60BlockInfo::default();
    // SAFETY: a live handle and valid buffers.
    assert_eq!(
        unsafe { rr_dr60_process(h, x.as_ptr(), y.as_mut_ptr(), 4096, &mut info) },
        RrDr60Status::Ok
    );
    let snap = |h: *mut RrDr60Pipeline| {
        let mut pos = 0u64;
        let mut ms = RrDr60MemoryStatus::default();
        let mut m = RrDr60Meters::default();
        // SAFETY: a live handle.
        unsafe {
            rr_dr60_stream_position(h, 1, &mut pos);
            rr_dr60_memory_status(h, &mut ms);
            rr_dr60_meters(h, &mut m);
        }
        (pos, ms, m)
    };
    let before = snap(h);
    let mut b = paint(39);
    // SAFETY: as above; refused.
    assert_eq!(
        unsafe { rr_dr60_process(h, x.as_ptr(), y.as_mut_ptr(), 4096, b.0.as_mut_ptr().cast()) },
        RrDr60Status::InvalidArgument
    );
    assert_eq!(before, snap(h));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// The constant-data structs hold string pointers: a zeroed tail beyond the filled size reads
/// as NULL pointers ("fields this library does not know").
#[test]
fn constant_data_tail_is_null_pointers() {
    let mut b = paint(80 + 16);
    // SAFETY: a writable buffer.
    assert_eq!(
        unsafe { rr_dr60_manifest_stage(3, b.0.as_mut_ptr().cast()) },
        RrDr60Status::Ok
    );
    let tail = &b.0[80..96];
    assert!(tail.iter().all(|&v| v == 0));
    let p = usize::from_ne_bytes(tail[..8].try_into().unwrap()) as *const std::ffi::c_char;
    assert!(p.is_null());
    // SAFETY: the first 80 bytes are a filled entry.
    let e = unsafe { b.0.as_ptr().cast::<RrDr60StageEntry>().read() };
    assert!(!e.identifier.is_null());
}

/// A caller newer than the library: a 4-byte-larger report is filled with 40 read back and the
/// extra zeroed; a larger settings struct is refused at create (D13).
#[test]
fn newer_caller_prefix_and_settings_rule() {
    let h = handle();
    let x = [0.1f32; 64];
    let mut y = [0.0f32; 64];
    let mut b = paint(44);
    // SAFETY: a live handle and valid buffers.
    assert_eq!(
        unsafe { rr_dr60_process(h, x.as_ptr(), y.as_mut_ptr(), 64, b.0.as_mut_ptr().cast()) },
        RrDr60Status::Ok
    );
    assert_eq!(u32::from_ne_bytes(b.0[..4].try_into().unwrap()), 40);
    assert!(b.0[40..44].iter().all(|&v| v == 0) && b.0[44..].iter().all(|&v| v == 0xAA));
    let mut s = rr_dr60_settings_default(48_000);
    s.struct_size = 124;
    let mut q = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(
        unsafe { rr_dr60_create(&s, &mut q) },
        RrDr60Status::InvalidArgument
    );
    assert!(q.is_null());
    assert_eq!(
        rr_dr60_abi_check(1, 8, 124, 16, 40),
        RrDr60Status::AbiMismatch
    );
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// A 32-bit target: the check returns a mismatch for pointer size 4; the frozen sizes hold
/// everywhere.
#[test]
fn thirty_two_bit_pointer_refused() {
    assert_eq!(
        rr_dr60_abi_check(1, 4, 120, 16, 40),
        RrDr60Status::AbiMismatch
    );
    assert_eq!(rr_dr60_abi_check(1, 8, 120, 16, 40), RrDr60Status::Ok);
    assert_eq!(RR_DR60_VAS_EVENT_SIZE, 16);
    assert_eq!(RR_DR60_CHANGE_ENTRY_SIZE, 40);
}

/// The single-element writers keep their `bool` return and the frozen layouts.
#[test]
fn single_element_writers_keep_bool_and_layout() {
    let e = RrDr60VasEvent {
        output_position: 7,
        input_length: 3,
    };
    let c = RrDr60ChangeEntry {
        position: 1,
        before: 0.5,
        after: 1.0,
        tag: 9,
        control: 1,
        stream: 3,
    };
    assert_eq!(size_of_val(&e), 16);
    assert_eq!(size_of_val(&c), 40);
    assert_eq!(std::mem::offset_of!(RrDr60VasEvent, input_length), 8);
    assert_eq!(std::mem::offset_of!(RrDr60ChangeEntry, control), 32);
    assert_eq!(std::mem::offset_of!(RrDr60ChangeEntry, stream), 36);
    // The writers take a record's bytes; on an empty record they return false and touch nothing.
    let mut out_e = RrDr60VasEvent::default();
    let mut out_c = RrDr60ChangeEntry::default();
    let mut at = 0u64;
    // SAFETY: an empty record with len 0; valid out pointers.
    unsafe {
        assert!(!rr_dr60_record_event(
            std::ptr::null(),
            0,
            0,
            &mut out_e,
            &mut at
        ));
        assert!(!rr_dr60_record_change(std::ptr::null(), 0, 0, &mut out_c));
    }
    assert_eq!(out_e, RrDr60VasEvent::default());
    assert_eq!(out_c, RrDr60ChangeEntry::default());
}
