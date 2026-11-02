//! Spec 025 User Story 2: every control by its index (tasks.md T008; spec › US2 AS1 – AS5).
//! Report: `cargo test -p rr_dr60_harness --test us2_control_index -- --nocapture`.
//!
//! The Rust one-hot oracle (`us2_one_hot_rust` in the spec) is the core test
//! `control_map::tests::one_hot`: the harness cannot destructure the `#[non_exhaustive]`
//! settings structures (research R-07, restated 2026-11-01).

use rr_dr60::control_map::{self, ControlReading};
use rr_dr60::controls::{CONTROL_COUNT, ControlValue, controls};
use rr_dr60::{Error, Pipeline, Route, Settings, Stream};
use rr_dr60_ffi::*;
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::settings_read_checks::{self as sr, How};

fn value_type_of(index: u32) -> u32 {
    (match controls()[index as usize].value {
        ControlValue::Switch { .. } => RrDr60ControlValueType::Switch,
        ControlValue::Integer { .. } => RrDr60ControlValueType::Integer,
        ControlValue::Continuous { .. } => RrDr60ControlValueType::Continuous,
        ControlValue::Choice { .. } => RrDr60ControlValueType::Choice,
        _ => unreachable!(),
    }) as u32
}

/// AS1: every control on a default engine reads its manifest default (the host rate the
/// engine's), in Rust and through C, in the manifest's value type.
#[test]
fn us2_defaults_by_index() {
    for &rate in configs::measurement_rates() {
        let p = Pipeline::new(Settings::new(rate)).unwrap();
        let s = rr_dr60_settings_default(rate);
        let mut h = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
        for i in 0..CONTROL_COUNT as u32 {
            let want = sr::default_reading(i, rate);
            assert_eq!(
                p.control_value(i).unwrap(),
                want,
                "{rate} Hz: {}",
                controls()[i as usize].identifier
            );
            let mut out = RrDr60ControlReading::default();
            // SAFETY: a live handle and a local reading.
            assert_eq!(
                unsafe { rr_dr60_control_value(h, i, &mut out) },
                RrDr60Status::Ok
            );
            assert_eq!(out.value_type, value_type_of(i), "{i}");
            assert_eq!(sr::rust_reading(&out), Some(want), "{rate} Hz: C {i}");
            if i == 3 {
                continue; // the host rate has no manifest default
            }
            // The pure read of the default structure agrees for every setting.
            if let Ok(r) = control_map::settings_value(&Settings::new(rate), i) {
                assert_eq!(r, want);
            }
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// AS2: the C one-hot oracle: for every settings index, on a structure whose padding is painted
/// 0xAA, the write by index changes exactly the bytes the test's field assignment by name
/// changes, and every other byte (the padding, the reserved slots) survives.
#[test]
fn us2_one_hot_c() {
    let base = rr_dr60_settings_default(48_000);
    let mut n = 0;
    for (i, e) in controls().iter().enumerate() {
        let i = i as u32;
        if control_map::settings_value(&Settings::new(48_000), i).is_err() {
            continue;
        }
        let r = sr::one_hot_value(i);
        let mut buf = sr::painted(&base);
        let mut expected = base;
        sr::set_by_name(&mut expected, e.identifier, r);
        let want = sr::painted(&expected);
        let c = sr::c_reading(r);
        // SAFETY: a 120-byte settings buffer (read and written unaligned) and a local reading.
        let st = unsafe { rr_dr60_set_settings_control_value(buf.as_mut_ptr().cast(), i, &c) };
        assert_eq!(st, RrDr60Status::Ok, "{}", e.identifier);
        let changed: Vec<usize> = (0..120)
            .filter(|&k| buf[k] != sr::painted(&base)[k])
            .collect();
        assert!(!changed.is_empty(), "{}: something changed", e.identifier);
        for k in 0..120 {
            assert_eq!(buf[k], want[k], "{}: byte {k}", e.identifier);
            if sr::is_padding(k) {
                assert_eq!(buf[k], 0xAA, "{}: padding {k}", e.identifier);
            }
        }
        // The read by index returns the value.
        let mut out = RrDr60ControlReading::default();
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_settings_control_value(buf.as_ptr().cast(), i, &mut out) },
            RrDr60Status::Ok
        );
        assert_eq!(sr::rust_reading(&out), Some(r), "{}", e.identifier);
        n += 1;
    }
    assert_eq!(n, 26, "26 of 26 settings");
}

/// AS2: representability refused with nothing changed; a value within the width but out of
/// range accepted, the validator naming the field, the field mapped back to the index.
#[test]
fn us2_representability() {
    let base = rr_dr60_settings_default(48_000);
    let i = |id: &str| controls().iter().position(|e| e.identifier == id).unwrap() as u32;
    let bad = [
        (i("agc.knee_dbfs"), ControlReading::Integer(1)),
        (i("agc.knee_dbfs"), ControlReading::Continuous(f32::NAN)),
        (
            i("agc.knee_dbfs"),
            ControlReading::Continuous(f32::NEG_INFINITY),
        ),
        (i("tap"), ControlReading::Choice(11)),
        (i("vas.mode"), ControlReading::Choice(2)),
        (i("host_rate_hz"), ControlReading::Choice(12_345)),
        (i("vas.sensitivity"), ControlReading::Integer(256)),
        (i("memory.capacity_s"), ControlReading::Integer(1 << 32)),
        (i("vas.enabled"), ControlReading::Continuous(1.0)),
    ];
    for (index, r) in bad {
        let mut buf = sr::painted(&base);
        let before = buf;
        let c = sr::c_reading(r);
        // SAFETY: a 120-byte buffer and a local reading.
        let st = unsafe { rr_dr60_set_settings_control_value(buf.as_mut_ptr().cast(), index, &c) };
        assert_eq!(st, RrDr60Status::InvalidArgument, "{index} {r:?}");
        assert_eq!(buf, before, "{index}: nothing changed");
        let mut s = Settings::new(48_000);
        assert!(matches!(
            control_map::set_settings_value(&mut s, index, r),
            Err(Error::InvalidControlValue { .. })
        ));
        assert_eq!(s, Settings::new(48_000));
    }
    // Sensitivity 6: within u8, out of range.
    let mut s = base;
    let c = sr::c_reading(ControlReading::Integer(6));
    let sens = i("vas.sensitivity");
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_set_settings_control_value(&mut s, sens, &c) },
        RrDr60Status::Ok
    );
    let mut field = RrDr60SettingField::None;
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_settings_validate(&s, &mut field) },
        RrDr60Status::InvalidSetting
    );
    assert_eq!(field, RrDr60SettingField::VasSensitivity);
    let mut back = 0u32;
    // SAFETY: a local out value.
    assert_eq!(
        unsafe { rr_dr60_setting_field_control(field as u32, &mut back) },
        RrDr60Status::Ok
    );
    assert_eq!(back, sens);
}

/// AS4: a structure holding `tap = 99` is repaired by index, no other byte changed.
#[test]
fn us2_repair_by_index() {
    let mut base = rr_dr60_settings_default(48_000);
    base.tap = 99;
    let mut field = RrDr60SettingField::None;
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_settings_validate(&base, &mut field) },
        RrDr60Status::InvalidArgument
    );
    assert_eq!(field, RrDr60SettingField::Tap);
    let mut buf = sr::painted(&base);
    let before = buf;
    let c = sr::c_reading(ControlReading::Choice(5));
    let mut idx = 0u32;
    // SAFETY: local values.
    unsafe {
        assert_eq!(
            rr_dr60_setting_field_control(field as u32, &mut idx),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_set_settings_control_value(buf.as_mut_ptr().cast(), idx, &c),
            RrDr60Status::Ok
        );
    }
    let off = core::mem::offset_of!(RrDr60Settings, tap);
    for k in 0..120 {
        if !(off..off + 4).contains(&k) {
            assert_eq!(buf[k], before[k], "byte {k}");
        }
    }
    // SAFETY: the buffer holds a valid settings struct now (read unaligned).
    let repaired: RrDr60Settings =
        unsafe { buf.as_ptr().cast::<RrDr60Settings>().read_unaligned() };
    // SAFETY: local values.
    assert_eq!(
        unsafe { rr_dr60_settings_validate(&repaired, &mut field) },
        RrDr60Status::Ok
    );
}

/// AS3: the five live controls by index: the read returns the value, and the change log's
/// entries equal the plain setter's and a schedule's at the current position (tags 0, 0, 1),
/// on every stream the control runs on, at block size 1.
#[test]
fn us2_live_by_index() {
    for &rate in configs::measurement_rates() {
        let mut s = Settings::new(rate);
        s.speech_codec_enabled = false;
        let stored = bp::stored_tone(1000.0, 0.5);
        for (index, r, value, streams) in sr::live_cases() {
            for &stream in streams {
                let run = |how| {
                    let mut p = bp::engine(&standard, s, &stored, Route::Speaker, 0.5);
                    let log = sr::live_twin(&mut p, stream, index, r, value, how);
                    (log, p.control_value(index).unwrap())
                };
                let (by_index, read) = run(How::ByIndex);
                let (setter, _) = run(How::Setter);
                let (sched, _) = run(How::Schedule);
                let id = controls()[index as usize].identifier;
                assert_eq!(read, r, "{rate} Hz {id} {stream:?}: the read");
                assert!(!by_index.is_empty(), "{rate} Hz {id} {stream:?}: an entry");
                let shape = |v: &[rr_dr60::ChangeEntry]| {
                    v.iter()
                        .map(|e| {
                            (
                                e.position,
                                e.control,
                                e.before.to_bits(),
                                e.after.to_bits(),
                                e.stream,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    shape(&by_index),
                    shape(&setter),
                    "{rate} Hz {id} {stream:?}: setter"
                );
                assert_eq!(
                    shape(&by_index),
                    shape(&sched),
                    "{rate} Hz {id} {stream:?}: schedule"
                );
                // The entries from the change on (the engine's route and volume setters also log
                // at the stream's first sample, tag 0, in all three runs).
                let tags = |v: &[rr_dr60::ChangeEntry]| {
                    v.iter()
                        .filter(|e| e.position >= 100)
                        .map(|e| e.tag)
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    tags(&by_index).len(),
                    1,
                    "{rate} Hz {id} {stream:?}: one entry at the change"
                );
                assert!(tags(&by_index).iter().all(|&t| t == 0));
                assert!(tags(&setter).iter().all(|&t| t == 0));
                assert!(tags(&sched).iter().all(|&t| t == 1));
            }
        }
    }
}

/// AS3: settings fields refused by the engine's write (`NOT_A_PLAIN_LIVE_CONTROL`), live
/// controls by the structure's (`NOT_A_SETTING`), in Rust and C.
#[test]
fn us2_class_refused() {
    let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
    let s = rr_dr60_settings_default(48_000);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&s, &mut h) }, RrDr60Status::Ok);
    let (mut settings, mut live) = (0, 0);
    for i in 0..CONTROL_COUNT as u32 {
        let r = sr::default_reading(i, 48_000);
        let c = sr::c_reading(r);
        let is_setting = control_map::settings_value(&Settings::new(48_000), i).is_ok();
        if is_setting {
            settings += 1;
            assert!(
                matches!(p.set_control_value(i, r), Err(Error::NotAPlainLiveControl { control }) if control == i)
            );
            // SAFETY: a live handle and a local reading.
            assert_eq!(
                unsafe { rr_dr60_set_control_value(h, i, &c) },
                RrDr60Status::NotAPlainLiveControl
            );
        } else {
            live += 1;
            let mut st = Settings::new(48_000);
            assert!(
                matches!(control_map::set_settings_value(&mut st, i, r), Err(Error::NotASetting { control }) if control == i)
            );
            let mut cs = s;
            let mut out = RrDr60ControlReading::default();
            // SAFETY: local values.
            unsafe {
                assert_eq!(
                    rr_dr60_set_settings_control_value(&mut cs, i, &c),
                    RrDr60Status::NotASetting
                );
                assert_eq!(
                    rr_dr60_settings_control_value(&cs, i, &mut out),
                    RrDr60Status::NotASetting
                );
            }
        }
    }
    assert_eq!((settings, live), (26, 5));
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// AS5: a non-default engine reconfigured with the defaults and the five live controls set by
/// index to their defaults reads the defaults of AS1.
#[test]
fn us2_device_as_modeled() {
    for &rate in configs::measurement_rates() {
        let mut p = Pipeline::new(sr::differ(rate)).unwrap();
        for (index, r, _, _) in sr::live_cases() {
            p.set_control_value(index, r).unwrap();
        }
        p.reconfigure(Settings::new(rate)).unwrap();
        for (index, _, _, _) in sr::live_cases() {
            p.set_control_value(index, sr::default_reading(index, rate))
                .unwrap();
        }
        for i in 0..CONTROL_COUNT as u32 {
            assert_eq!(
                p.control_value(i).unwrap(),
                sr::default_reading(i, rate),
                "{rate} Hz: {i}"
            );
        }
    }
}

/// D6: every `RrDr60SettingField` value maps to the control of the same name, or is refused.
#[test]
fn us2_field_lookup() {
    use RrDr60SettingField as F;
    let named = [
        (F::HostRate, "host_rate_hz"),
        (F::Tap, "tap"),
        (F::AgcKneeDbfs, "agc.knee_dbfs"),
        (F::AgcAttackMs, "agc.attack_ms"),
        (F::AgcReleaseMs, "agc.release_ms"),
        (F::VasMode, "vas.mode"),
        (F::VasSensitivity, "vas.sensitivity"),
        (F::VasThresholdDbfs, "vas.threshold_dbfs"),
        (F::VasHangMs, "vas.hang_ms"),
        (F::VasOnsetMs, "vas.onset_ms"),
        (F::MemoryCapacityS, "memory.capacity_s"),
        (F::MemoryMaxRecordings, "memory.max_recordings"),
        (F::MicHissOffsetDb, "mic.hiss_offset_db"),
    ];
    for (f, id) in named {
        let mut out = u32::MAX;
        // SAFETY: a local out value.
        assert_eq!(
            unsafe { rr_dr60_setting_field_control(f as u32, &mut out) },
            RrDr60Status::Ok
        );
        assert_eq!(controls()[out as usize].identifier, id);
    }
    for f in [F::None as u32, F::StructSize as u32, 5, 6, 17, 99, u32::MAX] {
        let mut out = 7u32;
        // SAFETY: a local out value.
        assert_eq!(
            unsafe { rr_dr60_setting_field_control(f, &mut out) },
            RrDr60Status::NoSuchControl
        );
        assert_eq!(out, 7, "nothing written");
    }
    // The core's lookup for the validator's fields agrees.
    for f in [
        rr_dr60::Setting::VasHangMs,
        rr_dr60::Setting::MicHissOffsetDb,
    ] {
        let i = control_map::setting_control(f);
        assert!(control_map::settings_value(&Settings::new(48_000), i).is_ok());
    }
    let _ = Stream::Playback;
}
