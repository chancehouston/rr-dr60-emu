//! No heap activity while processing (FR-015, FR-022, SC-004; tasks.md T053).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use rr_dr60::Pipeline;
use rr_dr60_ffi::{
    RrDr60Pipeline, RrDr60Status, rr_dr60_create, rr_dr60_destroy, rr_dr60_process, rr_dr60_reset,
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
        // Spec 001 configurations (AGC bypassed), the spec 002 AGC configurations, and the AGC
        // at its extreme settings (002 SC-004).
        let mut cases: Vec<(String, rr_dr60::Settings)> = configs::CONFIGS
            .iter()
            .chain(&configs::AGC_CONFIGS)
            .map(|c| (c.to_string(), configs::settings(c, rate)))
            .collect();
        let mut attack_min = configs::settings("default_agc", rate);
        attack_min.agc.attack_ms = 1.0;
        let mut release_max = configs::settings("default_agc", rate);
        release_max.agc.release_ms = 10_000.0;
        release_max.agc.max_gain_db = 60.0;
        cases.push(("agc attack min".into(), attack_min));
        cases.push(("agc release max, gain max".into(), release_max));
        for (config, settings) in cases {
            let mut p = Pipeline::new(settings).unwrap();
            let c_settings = configs::c_settings(&settings);
            let mut handle: *mut RrDr60Pipeline = std::ptr::null_mut();
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe { rr_dr60_create(&c_settings, &mut handle) },
                RrDr60Status::Ok
            );

            let before = counts();
            for &n in &sizes {
                let _ = p.process(&input[..n], &mut output[..n]).unwrap();
                in_place[..n].copy_from_slice(&input[..n]);
                let _ = p.process_in_place(&mut in_place[..n]);
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
