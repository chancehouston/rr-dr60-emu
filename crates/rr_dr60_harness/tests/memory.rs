//! Memory footprint per pipeline (plan.md › Technical Context › Constraints: "Memory under 1 MB
//! per pipeline"; tasks.md T074).
//!
//! Measures heap bytes retained by `Pipeline::new` (allocated minus freed), on this thread only,
//! for every host rate and configuration. Construction also allocates a temporary prototype
//! filter that is freed before `new` returns. That peak is printed for information but isn't the
//! constraint.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use rr_dr60::Pipeline;
use rr_dr60_harness::configs;

struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static FREED: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Count only this test thread's allocations (see alloc_free.rs).
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

fn measuring() -> bool {
    MEASURING.with(Cell::get)
}

// SAFETY: forwards every call to the system allocator unchanged; only counts bytes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if measuring() {
            ALLOCATED.fetch_add(layout.size(), Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if measuring() {
            FREED.fetch_add(layout.size(), Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if measuring() {
            FREED.fetch_add(layout.size(), Relaxed);
            ALLOCATED.fetch_add(new_size, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const LIMIT_BYTES: usize = 1 << 20; // plan.md: "Memory under 1 MB per pipeline" (engineering target)

#[test]
fn every_pipeline_fits_in_one_mebibyte() {
    MEASURING.with(|m| m.set(true));
    let mut worst = (0, 0, "", 0);
    for rate in configs::all_rates() {
        for config in configs::CONFIGS {
            let settings = configs::settings(config, rate);
            let (a0, f0) = (ALLOCATED.load(Relaxed), FREED.load(Relaxed));
            let p = Pipeline::new(settings).unwrap();
            let total = ALLOCATED.load(Relaxed) - a0;
            let retained = total - (FREED.load(Relaxed) - f0);
            std::hint::black_box(&p);
            drop(p);
            if retained > worst.0 {
                worst = (retained, rate, config, total);
            }
            assert!(
                retained <= LIMIT_BYTES,
                "{rate} Hz {config}: {retained} bytes retained (> 1 MiB)"
            );
        }
    }
    let (retained, rate, config, total) = worst;
    println!(
        "largest pipeline: {rate} Hz {config}: {retained} bytes retained ({total} allocated during new)"
    );
}
