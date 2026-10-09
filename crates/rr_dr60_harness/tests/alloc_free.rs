//! No heap activity while processing (FR-015, FR-022, SC-004; tasks.md T053).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use rr_dr60::Pipeline;
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60Settings, RrDr60Status, rr_dr60_create, rr_dr60_destroy, rr_dr60_process,
    rr_dr60_reset,
};
use rr_dr60_harness::configs;
use rr_dr60_harness::stimulus::{self, Pcg32};

struct Counting;

thread_local! {
    /// Count only allocations made on the measuring thread. Other threads, such as libtest's
    /// main thread printing "has been running for over 60 seconds" in slow coverage builds,
    /// must not count. A `const` thread-local with no destructor never allocates on access.
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

fn on_measuring_thread() -> bool {
    MEASURING.with(Cell::get)
}

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to the system allocator unchanged; only counts them.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if on_measuring_thread() {
            ALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if on_measuring_thread() {
            DEALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if on_measuring_thread() {
            REALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counts() -> [usize; 3] {
    [
        ALLOCS.load(Relaxed),
        DEALLOCS.load(Relaxed),
        REALLOCS.load(Relaxed),
    ]
}

/// Everything happens in one test, so no other test thread allocates concurrently.
#[test]
fn processing_never_touches_the_heap() {
    MEASURING.with(|m| m.set(true));
    let input = stimulus::noise(0x0D60, 1024);
    let mut output = vec![0.0f32; 1024];
    let mut in_place = vec![0.0f32; 1024];
    let mut rng = Pcg32::new(0x0D60, 3);
    let sizes: Vec<usize> = (0..1000).map(|_| rng.below(1025) as usize).collect();

    for rate in configs::all_rates() {
        for config in configs::CONFIGS {
            let settings = configs::settings(config, rate);
            let mut p = Pipeline::new(settings).unwrap();
            let c_settings = RrDr60Settings {
                record_stage_enabled: settings.record_stage_enabled,
                playback_stage_enabled: settings.playback_stage_enabled,
                tap: if settings.tap == rr_dr60::Tap::AfterRecord {
                    0
                } else {
                    1
                },
                ..rr_dr60_ffi::rr_dr60_settings_default(rate)
            };
            let mut handle: *mut RrDr60Pipeline = std::ptr::null_mut();
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe { rr_dr60_create(&c_settings, &mut handle) },
                RrDr60Status::Ok
            );

            // Coverage builds only (`cargo llvm-cov` sets cfg(coverage)): the instrumentation runtime
            // can make a few one-time allocations the first time code paths run (seen on Linux CI:
            // 4 allocations in the first window only). Warm both paths up once outside the window.
            // Normal builds measure with no warm-up at all, so first-call allocations in our own
            // code would still fail this test in the `check` jobs.
            if cfg!(coverage) {
                p.process(&input, &mut output).unwrap();
                // SAFETY: live handle; buffers valid for input.len() floats and non-overlapping.
                let _ = unsafe {
                    rr_dr60_process(handle, input.as_ptr(), output.as_mut_ptr(), input.len())
                };
                p.reset();
                // SAFETY: live handle.
                let _ = unsafe { rr_dr60_reset(handle) };
            }
            let before = counts();
            for &n in &sizes {
                p.process(&input[..n], &mut output[..n]).unwrap();
                in_place[..n].copy_from_slice(&input[..n]);
                p.process_in_place(&mut in_place[..n]);
                // SAFETY: live handle; buffers valid for n floats and non-overlapping.
                let status =
                    unsafe { rr_dr60_process(handle, input.as_ptr(), output.as_mut_ptr(), n) };
                assert!(status == RrDr60Status::Ok);
            }
            p.reset();
            // SAFETY: live handle.
            let status = unsafe { rr_dr60_reset(handle) };
            let after = counts();
            assert!(status == RrDr60Status::Ok);
            assert_eq!(
                after, before,
                "{rate} Hz {config}: heap activity [allocs, deallocs, reallocs] while processing"
            );
            // SAFETY: handle came from rr_dr60_create.
            unsafe { rr_dr60_destroy(handle) };
        }
    }
}
