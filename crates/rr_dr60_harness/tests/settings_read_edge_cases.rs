//! Spec 025's edge cases, one per test (tasks.md T010; spec › Edge Cases).

use rr_dr60::control_map::{self, ControlReading};
use rr_dr60::{Error, Pipeline, Settings};
use rr_dr60_ffi::*;
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::settings_read_checks as sr;

fn handle() -> *mut RrDr60Pipeline {
    let s = rr_dr60_settings_default(48_000);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
    h
}

/// An index equal to or past the control count: `NO_SUCH_CONTROL` everywhere.
#[test]
fn index_past_the_count() {
    let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
    let mut s = Settings::new(48_000);
    let h = handle();
    let mut c = rr_dr60_settings_default(48_000);
    let r = sr::c_reading(ControlReading::Integer(0));
    for i in [31u32, 32, 1000, u32::MAX] {
        assert!(matches!(
            p.control_value(i),
            Err(Error::NoSuchControl { .. })
        ));
        assert!(matches!(
            p.set_control_value(i, ControlReading::Integer(0)),
            Err(Error::NoSuchControl { .. })
        ));
        assert!(matches!(
            control_map::settings_value(&s, i),
            Err(Error::NoSuchControl { .. })
        ));
        assert!(matches!(
            control_map::set_settings_value(&mut s, i, ControlReading::Integer(0)),
            Err(Error::NoSuchControl { .. })
        ));
        let mut out = RrDr60ControlReading::default();
        // SAFETY: a live handle and local values.
        unsafe {
            assert_eq!(
                rr_dr60_control_value(h, i, &mut out),
                RrDr60Status::NoSuchControl
            );
            assert_eq!(
                rr_dr60_set_control_value(h, i, &r),
                RrDr60Status::NoSuchControl
            );
            assert_eq!(
                rr_dr60_settings_control_value(&c, i, &mut out),
                RrDr60Status::NoSuchControl
            );
            assert_eq!(
                rr_dr60_set_settings_control_value(&mut c, i, &r),
                RrDr60Status::NoSuchControl
            );
        }
    }
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// The seed's full range is exact through the integer field (2⁵³ + 1 and 2⁶⁴ − 1).
#[test]
fn seed_full_range() {
    for seed in [(1u64 << 53) + 1, u64::MAX, 0] {
        let mut s = Settings::new(48_000);
        control_map::set_settings_value(&mut s, 8, ControlReading::Integer(seed)).unwrap();
        assert_eq!(s.seed, seed);
        let p = Pipeline::new(s).unwrap();
        assert_eq!(p.control_value(8).unwrap(), ControlReading::Integer(seed));
        let mut c = rr_dr60_settings_default(48_000);
        let r = sr::c_reading(ControlReading::Integer(seed));
        let mut out = RrDr60ControlReading::default();
        // SAFETY: local values.
        unsafe {
            assert_eq!(
                rr_dr60_set_settings_control_value(&mut c, 8, &r),
                RrDr60Status::Ok
            );
            assert_eq!(
                rr_dr60_settings_control_value(&c, 8, &mut out),
                RrDr60Status::Ok
            );
        }
        assert_eq!((c.seed, out.int_value), (seed, seed));
    }
}

/// 0.1 reads back as the single-precision 0.10000000149011612; −0.0 is written as 0.0.
#[test]
fn single_precision_and_negative_zero() {
    let mut s = Settings::new(48_000);
    control_map::set_settings_value(&mut s, 18, ControlReading::Continuous(0.1)).unwrap();
    let ControlReading::Continuous(v) = control_map::settings_value(&s, 18).unwrap() else {
        panic!("a continuous value");
    };
    assert_eq!(f64::from(v), 0.10000000149011612);
    control_map::set_settings_value(&mut s, 10, ControlReading::Continuous(-0.0)).unwrap();
    assert_eq!(s.agc.knee_dbfs.to_bits(), 0.0f32.to_bits());
    let mut c = rr_dr60_settings_default(48_000);
    let r = sr::c_reading(ControlReading::Continuous(-0.0));
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut c, 10, &r) },
        RrDr60Status::Ok
    );
    assert_eq!(c.agc_knee_dbfs.to_bits(), 0.0f32.to_bits());
}

/// `tap` and `vas.mode` read and write the option's engine value; a non-option is refused.
#[test]
fn choices_are_option_values() {
    let mut s = Settings::new(48_000);
    for v in 0..=10u32 {
        control_map::set_settings_value(&mut s, 7, ControlReading::Choice(v)).unwrap();
        assert_eq!(
            control_map::settings_value(&s, 7).unwrap(),
            ControlReading::Choice(v)
        );
    }
    assert!(control_map::set_settings_value(&mut s, 7, ControlReading::Choice(11)).is_err());
    for v in 0..=1u32 {
        control_map::set_settings_value(&mut s, 14, ControlReading::Choice(v)).unwrap();
    }
    assert!(control_map::set_settings_value(&mut s, 14, ControlReading::Choice(2)).is_err());
    assert_eq!(s.vas.mode, rr_dr60::VasMode::Mute);
}

/// A live control read during a transition, a deferred speed change or a waiting request
/// returns the requested value, not the knob's position or the level heard.
#[test]
fn live_reads_return_the_request() {
    let mut s = Settings::new(48_000);
    s.speech_codec_enabled = false;
    let stored = bp::stored_tone(1000.0, 0.5);
    let mut p = bp::engine(&standard, s, &stored, rr_dr60::Route::Speaker, 0.5);
    p.start_playback(1).unwrap();
    let mut y = vec![0.0f32; 64];
    let _ = p.play(&mut y).unwrap();
    p.set_control_value(1, ControlReading::Continuous(0.9))
        .unwrap(); // a 20 ms transition
    p.set_control_value(29, ControlReading::Continuous(2.3))
        .unwrap(); // a 20 ms transition
    p.set_control_value(2, ControlReading::Integer(5)).unwrap();
    let _ = p.play(&mut y).unwrap(); // mid-transition, mid-crossfade
    p.set_control_value(2, ControlReading::Integer(3)).unwrap(); // waits for the crossfade
    assert_eq!(p.control_value(1).unwrap(), ControlReading::Continuous(0.9));
    assert_eq!(
        p.control_value(29).unwrap(),
        ControlReading::Continuous(2.3)
    );
    assert_eq!(p.control_value(2).unwrap(), ControlReading::Integer(3));
    p.stop_playback().unwrap();
}

/// A switch byte other than 0 or 1 in a caller's structure is refused on read, never
/// interpreted.
#[test]
fn switch_byte_refused_on_read() {
    let mut buf = sr::painted(&rr_dr60_settings_default(48_000));
    let off = core::mem::offset_of!(RrDr60Settings, speech_codec_enabled);
    for bad in [2u8, 0x80, 0xFF] {
        buf[off] = bad;
        let mut out = RrDr60ControlReading::default();
        // SAFETY: a 120-byte settings buffer read unaligned.
        let st = unsafe { rr_dr60_settings_control_value(buf.as_ptr().cast(), 21, &mut out) };
        assert_eq!(st, RrDr60Status::InvalidArgument, "{bad}");
        assert_eq!(out, RrDr60ControlReading::default(), "nothing written");
    }
}

/// An input reading's unused fields non-zero are ignored.
#[test]
fn unused_fields_ignored() {
    let mut c = rr_dr60_settings_default(48_000);
    let r = RrDr60ControlReading {
        int_value: 77,
        reserved: 0xFFFF_FFFF,
        ..sr::c_reading(ControlReading::Continuous(-12.0))
    };
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut c, 10, &r) },
        RrDr60Status::Ok
    );
    assert_eq!(c.agc_knee_dbfs, -12.0);
    let r = RrDr60ControlReading {
        float_value: 9.0,
        reserved: 1,
        ..sr::c_reading(ControlReading::Integer(4))
    };
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut c, 15, &r) },
        RrDr60Status::Ok
    );
    assert_eq!(c.vas_sensitivity, 4);
}
