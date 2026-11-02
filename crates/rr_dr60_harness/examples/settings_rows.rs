//! Prints the settings rows as the C smoke program prints them (`smoke --settings`; spec 025
//! T011): the reading's size and offsets (`offset_of!`), every control's reading on a default
//! engine at 48 kHz, the bytes the field assignment by name changes for each setting's one-hot
//! value in a default structure, and the live controls by name.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example settings_rows`

use core::mem::offset_of;
use rr_dr60::control_map::ControlReading;
use rr_dr60::controls::controls;
use rr_dr60::{Pipeline, Settings};
use rr_dr60_ffi::{RrDr60ControlReading, RrDr60Settings, rr_dr60_settings_default};
use rr_dr60_harness::settings_read_checks as sr;

fn bytes(c: &RrDr60Settings) -> [u8; 120] {
    let mut b = [0u8; 120];
    // SAFETY: a 120-byte plain struct copied as bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(
            (c as *const RrDr60Settings).cast::<u8>(),
            b.as_mut_ptr(),
            120,
        )
    };
    b
}

fn main() {
    println!(
        "reading {} {} {} {} {} {}",
        size_of::<RrDr60ControlReading>(),
        offset_of!(RrDr60ControlReading, struct_size),
        offset_of!(RrDr60ControlReading, value_type),
        offset_of!(RrDr60ControlReading, int_value),
        offset_of!(RrDr60ControlReading, float_value),
        offset_of!(RrDr60ControlReading, reserved),
    );
    let p = Pipeline::new(Settings::new(48_000)).unwrap();
    for i in 0..controls().len() as u32 {
        let (ty, int, bits) = match p.control_value(i).unwrap() {
            ControlReading::Switch(b) => (0, u64::from(b), 0),
            ControlReading::Integer(n) => (1, n, 0),
            ControlReading::Continuous(v) => (2, 0, v.to_bits()),
            ControlReading::Choice(c) => (3, u64::from(c), 0),
        };
        println!("value {i} {ty} {int} 0x{bits:08x}");
    }
    let d = rr_dr60_settings_default(48_000);
    for (i, e) in controls().iter().enumerate() {
        let i = i as u32;
        if rr_dr60::control_map::settings_value(&Settings::new(48_000), i).is_err() {
            println!("live {i} {}", e.identifier);
            continue;
        }
        let mut after = d;
        sr::set_by_name(&mut after, e.identifier, sr::one_hot_value(i));
        let (a, b) = (bytes(&d), bytes(&after));
        let changed: String = (0..120)
            .filter(|&k| a[k] != b[k])
            .map(|k| format!(" {k}"))
            .collect();
        println!("setting {i} {}{changed}", e.identifier);
    }
}
