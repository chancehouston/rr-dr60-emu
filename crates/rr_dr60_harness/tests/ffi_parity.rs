//! C API parity and error contract (001 FR-023, FR-024; contracts/c-api.md; 001 tasks.md T054),
//! at all seven host rates including the 6 kHz identity rate (spec 004 FR-016, FR-017; 004
//! tasks.md T028).

use rr_dr60::{Pipeline, Route, State, StopReason, StopResult, VasEvent};
use rr_dr60_ffi::*;
use rr_dr60_harness::{chain_checks, configs, golden, smoke_reference, split_checks, stimulus};

fn create(s: &RrDr60Settings) -> *mut RrDr60Pipeline {
    let mut p = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(s, &mut p) }, RrDr60Status::Ok);
    p
}

#[test]
fn c_api_matches_rust_api_everywhere() {
    for rate in configs::all_rates() {
        let x = golden::stimulus("sweep_log", rate);
        for config in configs::CONFIGS {
            let s = configs::settings(config, rate);
            let mut want = vec![0.0; x.len()];
            let mut rust = Pipeline::new(s).unwrap();
            let produced = rust.process(&x, &mut want).unwrap().produced;
            want.truncate(produced);
            let p = create(&configs::c_settings(&s));
            let mut got = vec![0.0f32; x.len()];
            // SAFETY: live handle; valid non-overlapping buffers.
            assert_eq!(
                unsafe {
                    rr_dr60_process(
                        p,
                        x.as_ptr(),
                        got.as_mut_ptr(),
                        x.len(),
                        std::ptr::null_mut(),
                    )
                },
                RrDr60Status::Ok
            );
            assert_eq!(got, want, "{rate} Hz {config}");
            let mut lat = 0;
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_latency_samples(p, &mut lat) },
                RrDr60Status::Ok
            );
            assert_eq!(lat, rust.latency_samples());
            // SAFETY: from rr_dr60_create.
            unsafe { rr_dr60_destroy(p) };
        }
    }
}

/// Spec 004 FR-002, FR-017 (contracts/c-api.md): at the 6 kHz device rate (A-026) with every
/// stage bypassed, the C API returns the input sample for sample (non-finite and subnormal
/// samples become 0.0), reports latency 0, and is bit-identical to the Rust API.
#[test]
fn c_api_identity_at_6k() {
    let mut x = stimulus::noise(0x0D60, 6000);
    x[10] = f32::NAN;
    x[20] = f32::INFINITY;
    x[30] = f32::from_bits(1);
    x[40] = 3.5; // over full scale: passed through, not clipped
    let s = configs::settings("bypass_all", 6000);
    let mut want = vec![0.0f32; x.len()];
    let mut rust = Pipeline::new(s).unwrap();
    let produced = rust.process(&x, &mut want).unwrap().produced;
    assert_eq!(produced, x.len());
    let p = create(&configs::c_settings(&s));
    let mut got = vec![0.0f32; x.len()];
    let mut info = RrDr60BlockInfo::default();
    // SAFETY: live handle; valid non-overlapping buffers; info is writable.
    assert_eq!(
        unsafe { rr_dr60_process(p, x.as_ptr(), got.as_mut_ptr(), x.len(), &mut info) },
        RrDr60Status::Ok
    );
    assert_eq!(info.produced, x.len());
    for (i, ((&a, &b), &c)) in x.iter().zip(&got).zip(&want).enumerate() {
        let identity = if a.is_finite() && !a.is_subnormal() {
            a
        } else {
            0.0
        };
        assert_eq!(b.to_bits(), identity.to_bits(), "sample {i}: C vs input");
        assert_eq!(b.to_bits(), c.to_bits(), "sample {i}: C vs Rust");
    }
    let mut lat = 7;
    // SAFETY: live handle.
    assert_eq!(
        unsafe { rr_dr60_latency_samples(p, &mut lat) },
        RrDr60Status::Ok
    );
    assert_eq!((lat, rust.latency_samples()), (0, 0));
    // SAFETY: from rr_dr60_create.
    unsafe { rr_dr60_destroy(p) };
}

/// Spec 005 FR-015 (US3 AS4): C and Rust output are bit-identical with the limiter on, for
/// the AGC configurations, every FR-011 settings-matrix case, the minimum and maximum of every
/// limiter setting, and the clamp off; `c_default_005` is the `default_agc` configuration.
#[test]
fn c_api_matches_rust_api_with_agc() {
    let rate = 48_000;
    // The onset stimulus: sub-knee, 10 dB over the knee, sub-knee (the dead time, the ceiling
    // and the start of the release all inside 0.4 s).
    let x = stimulus::step_at_phase(
        1000.0,
        &[-20.0, 11.0, -20.0],
        &[0.1, 0.1, 0.2],
        stimulus::peak_phase(1000.0, rate),
        48_000.0,
    );
    let mut cases: Vec<rr_dr60::Settings> = configs::AGC_CONFIGS
        .iter()
        .map(|c| configs::settings(c, rate))
        .collect();
    for case in &rr_dr60_harness::agc_checks::matrix()[1..] {
        let mut s = configs::settings("agc_only", rate);
        s.agc = case.agc;
        cases.push(s);
    }
    let extremes: [fn(&mut rr_dr60::AgcSettings, bool); 3] = [
        |a, hi| a.knee_dbfs = if hi { 3.0 } else { -40.0 }, // 005 FR-010 range
        |a, hi| a.attack_ms = if hi { 50.0 } else { 1.0 },
        |a, hi| a.release_ms = if hi { 10_000.0 } else { 200.0 },
    ];
    for set in extremes {
        for hi in [false, true] {
            let mut s = configs::settings("default_agc", rate);
            set(&mut s.agc, hi);
            cases.push(s);
        }
    }
    let mut clamp_off = configs::settings("default_agc", rate);
    clamp_off.adc_clamp_enabled = false; // 005 FR-008
    cases.push(clamp_off);
    assert_eq!(
        configs::c_default_005(rate),
        configs::c_settings(&configs::settings("default_agc", rate))
    );
    for s in cases {
        let mut want = vec![0.0; x.len()];
        let produced = Pipeline::new(s)
            .unwrap()
            .process(&x, &mut want)
            .unwrap()
            .produced;
        want.truncate(produced);
        let p = create(&configs::c_settings(&s));
        let mut got = vec![0.0f32; x.len()];
        // SAFETY: live handle; valid non-overlapping buffers of x.len() floats.
        assert_eq!(
            unsafe {
                rr_dr60_process(
                    p,
                    x.as_ptr(),
                    got.as_mut_ptr(),
                    x.len(),
                    std::ptr::null_mut(),
                )
            },
            RrDr60Status::Ok
        );
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
        assert_eq!(got, want, "{:?}", s.agc);
    }
}

/// A C settings field and a way to put it out of range.
type BreakCase = (RrDr60SettingField, fn(&mut RrDr60Settings));

/// Validates through the C API and returns (status, field).
fn validate(s: &RrDr60Settings) -> (RrDr60Status, RrDr60SettingField) {
    let mut field = RrDr60SettingField::AgcReleaseMs; // overwritten on every call
    // SAFETY: valid pointers.
    let status = unsafe { rr_dr60_settings_validate(s, &mut field) };
    (status, field)
}

/// Spec 005 contracts/c-api.md (restated by spec 006: 88 bytes, the two memory fields at 76 and
/// 80): the 0.5 layout kept the 0.4 offsets; the
/// defaults are the limiter's (A-028, A-034, A-035), the clamp on (A-030) and the VAS
/// threshold −40 dBFS (A-022 revised); the two reserved slots are ignored by validation and by
/// processing; a 72-byte (0.4-layout) struct is rejected as `STRUCT_SIZE`.
#[test]
fn c_api_005_layout_defaults_and_reserved_fields() {
    assert_eq!(size_of::<RrDr60Settings>(), 120); // spec 012 contracts/c-api.md (112 in 0.10 and 0.11)
    assert_eq!(std::mem::offset_of!(RrDr60Settings, agc_knee_dbfs), 28);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, reserved_agc_1), 32);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, reserved_agc_2), 36);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, agc_attack_ms), 40);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, agc_release_ms), 44);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, adc_clamp_enabled), 72);
    assert_eq!(std::mem::offset_of!(RrDr60Settings, memory_capacity_s), 76); // 006
    assert_eq!(
        std::mem::offset_of!(RrDr60Settings, memory_max_recordings),
        80
    ); // 006
    let d = rr_dr60_settings_default(48_000);
    assert_eq!(d.struct_size, 120); // spec 012
    assert_eq!(d.agc_knee_dbfs, -1.0); // A-028
    assert_eq!(d.agc_attack_ms, 3.0); // A-034
    assert_eq!(d.agc_release_ms, 1370.0); // A-035
    assert!(d.adc_clamp_enabled); // A-030
    assert_eq!(d.vas_threshold_dbfs, -40.0); // A-022 revised
    assert_eq!((d.reserved_agc_1, d.reserved_agc_2), (0.0, 0.0));
    assert_eq!(RrDr60SettingField::AgcKneeDbfs as u32, 4);
    assert_eq!(RrDr60SettingField::AgcAttackMs as u32, 7);
    assert_eq!(RrDr60SettingField::AgcReleaseMs as u32, 8);

    // The reserved slots are ignored: garbage there validates and changes nothing.
    let x = stimulus::step_at_phase(
        1000.0,
        &[-20.0, 11.0],
        &[0.1, 0.2],
        stimulus::peak_phase(1000.0, 48_000),
        48_000.0,
    );
    let mut garbage = d;
    garbage.reserved_agc_1 = f32::NAN;
    garbage.reserved_agc_2 = 999.0;
    assert_eq!(
        validate(&garbage),
        (RrDr60Status::Ok, RrDr60SettingField::None)
    );
    let (p, q) = (create(&d), create(&garbage));
    let (mut a, mut b) = (vec![0.0f32; x.len()], vec![0.0f32; x.len()]);
    // SAFETY: live handles; valid non-overlapping buffers of x.len() floats.
    unsafe {
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), a.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(q, x.as_ptr(), b.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        rr_dr60_destroy(p);
        rr_dr60_destroy(q);
    }
    assert_eq!(a, b, "the reserved slots changed the output");

    // A 0.4-layout struct (72 bytes, `struct_size = 72`): rejected as STRUCT_SIZE after reading
    // only the first field.
    #[repr(C, align(8))]
    struct Layout04([u8; 72]);
    let mut old = Layout04([0; 72]);
    old.0[..4].copy_from_slice(&72u32.to_ne_bytes());
    let settings = (&raw const old).cast::<RrDr60Settings>();
    let mut field = RrDr60SettingField::None;
    let mut out = std::ptr::null_mut();
    // SAFETY: the functions read only `struct_size` before rejecting a too-small struct
    // (contracts/c-api.md), and the 72 bytes are readable and aligned.
    unsafe {
        assert_eq!(
            rr_dr60_settings_validate(settings, &mut field),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(field, RrDr60SettingField::StructSize);
        assert_eq!(
            rr_dr60_create(settings, &mut out),
            RrDr60Status::InvalidArgument
        );
        assert!(out.is_null());
    }
}

/// Spec 005 FR-010, FR-015 (contracts/c-api.md): invalid limiter settings are rejected with
/// INVALID_SETTING, and `rr_dr60_settings_validate` names the field.
#[test]
fn c_api_names_invalid_agc_settings() {
    let cases: [BreakCase; 3] = [
        (RrDr60SettingField::AgcKneeDbfs, |s| s.agc_knee_dbfs = 4.0),
        (RrDr60SettingField::AgcAttackMs, |s| s.agc_attack_ms = 0.0),
        (RrDr60SettingField::AgcReleaseMs, |s| {
            s.agc_release_ms = f32::INFINITY
        }),
    ];
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    for (field, break_it) in cases {
        let mut s = rr_dr60_settings_default(48_000);
        break_it(&mut s);
        assert_eq!(validate(&s), (RrDr60Status::InvalidSetting, field));
        let mut out = sentinel;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_create(&s, &mut out) },
            RrDr60Status::InvalidSetting
        );
        assert_eq!(out, sentinel, "*out touched on error");
    }
    assert_eq!(
        validate(&rr_dr60_settings_default(48_000)),
        (RrDr60Status::Ok, RrDr60SettingField::None)
    );
}

/// Spec 002 contracts/c-api.md: `validate` returns exactly what `create` returns, in the
/// documented order (struct size → tap → host rate → AGC fields), and handles NULL.
#[test]
fn c_api_validate_agrees_with_create() {
    let base = rr_dr60_settings_default(48_000);
    let old_layout = RrDr60Settings {
        struct_size: 24,
        ..base
    };
    let bad_size_and_tap = RrDr60Settings {
        struct_size: 24,
        tap: 11,
        ..base
    };
    let bad_tap_and_rate = RrDr60Settings {
        tap: 11,
        host_rate_hz: 22_050,
        ..base
    };
    let bad_rate_and_attack = RrDr60Settings {
        host_rate_hz: 22_050,
        agc_attack_ms: 0.0,
        ..base
    };
    let bad_knee_and_release = RrDr60Settings {
        agc_knee_dbfs: 5.0, // above the +3 dBFS cap (005 FR-010)
        agc_release_ms: 1.0,
        ..base
    };
    let expected = [
        (
            old_layout,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::StructSize,
        ),
        (
            bad_size_and_tap,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::StructSize,
        ),
        (
            bad_tap_and_rate,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::Tap,
        ),
        (
            bad_rate_and_attack,
            RrDr60Status::UnsupportedHostRate,
            RrDr60SettingField::HostRate,
        ),
        (
            bad_knee_and_release,
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::AgcKneeDbfs,
        ),
    ];
    for (s, status, field) in expected {
        assert_eq!(validate(&s), (status, field), "{s:?}");
        let mut out = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, status, "{s:?}");
    }
    // SAFETY: NULL settings is reported, never dereferenced; a NULL out_field is allowed.
    unsafe {
        let mut field = RrDr60SettingField::Tap;
        assert_eq!(
            rr_dr60_settings_validate(std::ptr::null(), &mut field),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_settings_validate(&base, std::ptr::null_mut()),
            RrDr60Status::Ok
        );
    }
}

/// Spec 002 FR-014: an invalid AGC setting in `rr_dr60_reconfigure` leaves the handle unchanged.
#[test]
fn c_api_reconfigure_rejects_invalid_agc_and_keeps_state() {
    let x = stimulus::step(1000.0, &[-30.0], &[0.2], 48_000.0);
    let good = rr_dr60_settings_default(48_000);
    let (p, q) = (create(&good), create(&good));
    let mut scratch = vec![0.0f32; x.len()];
    let mut bad = good;
    bad.agc_attack_ms = 0.0;
    // SAFETY: live handles; valid buffers and settings.
    unsafe {
        assert_eq!(
            rr_dr60_process(
                p,
                x.as_ptr(),
                scratch.as_mut_ptr(),
                x.len(),
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(
                q,
                x.as_ptr(),
                scratch.as_mut_ptr(),
                x.len(),
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(rr_dr60_reconfigure(p, &bad), RrDr60Status::InvalidSetting);
        let (mut a, mut b) = (vec![0.0f32; x.len()], vec![0.0f32; x.len()]);
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), a.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(q, x.as_ptr(), b.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(a, b, "failed reconfigure changed the handle");
        rr_dr60_destroy(p);
        rr_dr60_destroy(q);
    }
}

#[test]
fn error_table() {
    let ok = rr_dr60_settings_default(48_000);
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    let mut out = sentinel;
    let status = |s: RrDr60Settings, out: &mut *mut RrDr60Pipeline| {
        // SAFETY: valid pointers.
        unsafe { rr_dr60_create(&s, out) }
    };
    // SAFETY: NULL settings / NULL out-pointer must be rejected without dereferencing.
    assert_eq!(
        unsafe { rr_dr60_create(std::ptr::null(), &mut out) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        unsafe { rr_dr60_create(&ok, std::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        status(rr_dr60_settings_default(22_050), &mut out),
        RrDr60Status::UnsupportedHostRate
    );
    assert_eq!(
        status(RrDr60Settings { tap: 11, ..ok }, &mut out), // 0 – 10 are taps since 0.12
        RrDr60Status::InvalidArgument
    );
    assert_eq!(
        status(
            RrDr60Settings {
                struct_size: 4,
                ..ok
            },
            &mut out
        ),
        RrDr60Status::InvalidArgument
    );
    assert_eq!(out, sentinel, "*out touched on error");

    let p = create(&ok);
    let x = vec![0.25f32; 256];
    let mut y = vec![0.0f32; 256];
    // SAFETY (all below): p is live; buffers are valid for their stated lengths.
    unsafe {
        assert_eq!(
            rr_dr60_process(
                std::ptr::null_mut(),
                x.as_ptr(),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                std::ptr::null(),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                x.as_ptr(),
                std::ptr::null_mut(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(
                p,
                y.as_ptr(),
                y.as_mut_ptr().add(1),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_process(
                p,
                y.as_ptr().add(1),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::InvalidArgument
        );
        let mut lat = 7u32;
        assert_eq!(
            rr_dr60_latency_samples(std::ptr::null(), &mut lat),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_latency_samples(p, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(lat, 7);
        assert_eq!(
            rr_dr60_reset(std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        let unsupported = rr_dr60_settings_default(22_050);
        assert_eq!(
            rr_dr60_reconfigure(p, &unsupported),
            RrDr60Status::UnsupportedHostRate
        );
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 256, std::ptr::null_mut()),
            RrDr60Status::Ok,
            "old config must still work"
        );
        rr_dr60_destroy(std::ptr::null_mut());
        rr_dr60_destroy(p);
    }
}

/// One Rust block's result, for comparison with the C side.
type BlockRecord = (RrDr60BlockInfo, Vec<RrDr60VasEvent>);

fn c_event(e: rr_dr60::VasEvent) -> RrDr60VasEvent {
    RrDr60VasEvent {
        output_position: e.output_position,
        input_length: e.input_length,
    }
}

/// Runs `x` through the Rust API in blocks of `block`, with events.
fn rust_blocks(s: rr_dr60::Settings, x: &[f32], block: usize) -> (Vec<f32>, Vec<BlockRecord>) {
    let mut p = Pipeline::new(s).unwrap();
    let (mut out, mut records) = (Vec::new(), Vec::new());
    for chunk in x.chunks(block) {
        let mut y = vec![0.0f32; chunk.len()];
        let mut ev = vec![rr_dr60::VasEvent::default(); p.max_events(chunk.len())];
        let info = p.process_with_events(chunk, &mut y, &mut ev).unwrap();
        out.extend_from_slice(&y[..info.produced]);
        records.push((
            RrDr60BlockInfo {
                produced: info.produced,
                events: info.events,
                paused: info.paused,
                frames: info.frames,
                ..Default::default()
            },
            ev[..info.events].iter().copied().map(c_event).collect(),
        ));
    }
    (out, records)
}

/// Runs `x` through the C API in blocks of `block`, with `rr_dr60_process_with_events` and a
/// capacity from `rr_dr60_max_events`.
fn c_blocks(s: &RrDr60Settings, x: &[f32], block: usize) -> (Vec<f32>, Vec<BlockRecord>) {
    let p = create(s);
    let (mut out, mut records) = (Vec::new(), Vec::new());
    for chunk in x.chunks(block) {
        let mut y = vec![0.0f32; chunk.len()];
        let mut cap = usize::MAX;
        // SAFETY: live handle; valid out-pointer.
        assert_eq!(
            unsafe { rr_dr60_max_events(p, chunk.len(), &mut cap) },
            RrDr60Status::Ok
        );
        assert!(cap < usize::MAX);
        let mut ev = vec![RrDr60VasEvent::default(); cap];
        let mut info = RrDr60BlockInfo::default();
        // SAFETY: live handle; valid non-overlapping buffers; `ev` holds `cap` events.
        assert_eq!(
            unsafe {
                rr_dr60_process_with_events(
                    p,
                    chunk.as_ptr(),
                    y.as_mut_ptr(),
                    chunk.len(),
                    &mut info,
                    ev.as_mut_ptr(),
                    cap,
                )
            },
            RrDr60Status::Ok
        );
        assert!(
            info.events <= cap,
            "capacity {cap} < events {}",
            info.events
        );
        out.extend_from_slice(&y[..info.produced]);
        ev.truncate(info.events);
        records.push((info, ev));
    }
    // SAFETY: from rr_dr60_create.
    unsafe { rr_dr60_destroy(p) };
    (out, records)
}

/// Spec 003 US2 AS7, FR-015 (contracts/c-api.md rule 6): C and Rust give bit-identical output,
/// `BlockInfo` sequences and events for the VAS configurations and for the minimum and maximum
/// of every VAS setting.
#[test]
fn c_api_matches_rust_api_with_vas() {
    let rate = 48_000;
    let x = stimulus::burst_gap(
        1000.0,
        -30.0, // threshold + 10 dB (spec 003 definitions; −40 dBFS since spec 005, A-022)
        &[
            (true, 0.5),
            (false, 2.0),
            (true, 0.3),
            (false, 1.5),
            (true, 0.4),
            (false, 0.5),
        ],
        48_000.0,
    );
    let mut cases: Vec<rr_dr60::Settings> = configs::VAS_CONFIGS
        .iter()
        .map(|c| configs::settings(c, rate))
        .collect();
    let extremes: [fn(&mut rr_dr60::VasSettings, bool); 4] = [
        |v, hi| v.sensitivity = if hi { 5 } else { 1 },
        |v, hi| v.threshold_dbfs = if hi { 0.0 } else { -60.0 },
        |v, hi| v.hang_ms = if hi { 10_000.0 } else { 50.0 },
        |v, hi| v.onset_ms = if hi { 200.0 } else { 0.0 },
    ];
    for set in extremes {
        for hi in [false, true] {
            for mode in [rr_dr60::VasMode::Drop, rr_dr60::VasMode::Mute] {
                let mut s = configs::settings("vas_only", rate);
                s.vas.mode = mode;
                set(&mut s.vas, hi);
                cases.push(s);
            }
        }
    }
    for s in cases {
        for block in [x.len(), 1000, 7] {
            let want = rust_blocks(s, &x, block);
            let got = c_blocks(&configs::c_settings(&s), &x, block);
            assert_eq!(got.0, want.0, "output: {:?} block {block}", s.vas);
            assert_eq!(got.1, want.1, "blocks: {:?} block {block}", s.vas);
        }
    }
}

/// Spec 003 FR-012, FR-015 (contracts/c-api.md › Conversion rules): invalid VAS fields are named,
/// `vas_sensitivity = 259` does not wrap to a valid level, an invalid `vas_mode` is
/// `INVALID_ARGUMENT`, and `validate` agrees with `create`.
#[test]
fn c_api_names_invalid_vas_settings() {
    /// A VAS field put out of range, with the status and field it must report.
    type VasBreak = (RrDr60Status, RrDr60SettingField, fn(&mut RrDr60Settings));
    let cases: [VasBreak; 6] = [
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasSensitivity,
            |s| s.vas_sensitivity = 6,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasSensitivity,
            |s| s.vas_sensitivity = 259,
        ),
        (
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::VasMode,
            |s| s.vas_mode = 7,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasThresholdDbfs,
            |s| s.vas_threshold_dbfs = 0.5,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasHangMs,
            |s| s.vas_hang_ms = 49.0,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasOnsetMs,
            |s| s.vas_onset_ms = f32::NAN,
        ),
    ];
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    for (status, field, break_it) in cases {
        let mut s = rr_dr60_settings_default(48_000);
        break_it(&mut s);
        assert_eq!(validate(&s), (status, field), "{s:?}");
        let mut out = sentinel;
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, status, "{s:?}");
        assert_eq!(out, sentinel, "*out touched on error");
    }
    // `vas_mode` is checked after `tap` and before the host rate.
    let s = RrDr60Settings {
        vas_mode: 7,
        host_rate_hz: 22_050,
        ..rr_dr60_settings_default(48_000)
    };
    assert_eq!(
        validate(&s),
        (RrDr60Status::InvalidArgument, RrDr60SettingField::VasMode)
    );
}

/// Spec 003 contracts/c-api.md: `out_info` is always written on an early return (`frames == 0`,
/// NULL buffers, NULL events with a capacity) with `produced = 0`, `events = 0` and the current
/// `paused`; `events = NULL` is accepted with capacity 0; and a 0.2-sized struct is rejected as
/// `STRUCT_SIZE` after reading only its first field.
#[test]
fn c_api_out_info_on_early_returns_and_old_struct_size() {
    // At the 6 kHz identity rate (spec 004 FR-002): two hang times of silence (2 × 6000
    // samples) end paused, with exactly one hang time produced (003 FR-004 exactness).
    let p = create(&configs::c_settings(&configs::settings("vas_only", 6_000)));
    let silence = vec![0.0f32; 12_000]; // two hang times: ends paused
    let mut y = vec![0.0f32; 12_000];
    let stale = RrDr60BlockInfo {
        produced: 77,
        events: 77,
        paused: false,
        frames: 77,
        ..Default::default()
    };
    let mut info = stale;
    // SAFETY (all below): p is live; buffers are valid for their stated lengths.
    unsafe {
        assert_eq!(
            rr_dr60_process(p, silence.as_ptr(), y.as_mut_ptr(), 12_000, &mut info),
            RrDr60Status::Ok
        );
        assert_eq!(info.produced, 6_000);
        assert!(info.paused);

        let idle = RrDr60BlockInfo {
            produced: 0,
            events: 0,
            paused: true,
            frames: 0,
            ..Default::default()
        };
        // frames == 0.
        let status = rr_dr60_process(p, std::ptr::null(), std::ptr::null_mut(), 0, &mut info);
        assert_eq!((status, info), (RrDr60Status::Ok, idle));
        info = stale;
        // NULL input.
        let status = rr_dr60_process(p, std::ptr::null(), y.as_mut_ptr(), 64, &mut info);
        assert_eq!((status, info), (RrDr60Status::NullPointer, idle));
        info = stale;
        // NULL events with a non-zero capacity.
        let status = rr_dr60_process_with_events(
            p,
            silence.as_ptr(),
            y.as_mut_ptr(),
            64,
            &mut info,
            std::ptr::null_mut(),
            4,
        );
        assert_eq!((status, info), (RrDr60Status::NullPointer, idle));
        info = stale;
        // NULL events with capacity 0 is fine, and the block is processed.
        assert_eq!(
            rr_dr60_process_with_events(
                p,
                silence.as_ptr(),
                y.as_mut_ptr(),
                64,
                &mut info,
                std::ptr::null_mut(),
                0,
            ),
            RrDr60Status::Ok
        );
        assert_eq!(info.produced, 0, "still paused: nothing produced");
        // NULL handle: paused = false.
        info = stale;
        assert_eq!(
            rr_dr60_process(
                std::ptr::null_mut(),
                silence.as_ptr(),
                y.as_mut_ptr(),
                64,
                &mut info
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(info, RrDr60BlockInfo::default());
        // max_events: NULL handle / NULL out leave the out-parameter untouched (rule 2).
        let mut cap = 5usize;
        assert_eq!(
            rr_dr60_max_events(std::ptr::null(), 64, &mut cap),
            RrDr60Status::NullPointer
        );
        assert_eq!(cap, 5);
        assert_eq!(
            rr_dr60_max_events(p, 64, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        rr_dr60_destroy(p);
    }

    // A struct from the 0.2 layout (48 bytes, `struct_size = 48`): rejected as STRUCT_SIZE by
    // validate and create, reading only the first field.
    #[repr(C, align(8))]
    struct OldLayout([u8; 48]);
    let mut old = OldLayout([0; 48]);
    old.0[..4].copy_from_slice(&48u32.to_ne_bytes());
    let settings = (&raw const old).cast::<RrDr60Settings>();
    let mut field = RrDr60SettingField::None;
    let mut out = std::ptr::null_mut();
    // SAFETY: the functions read only `struct_size` before rejecting a too-small struct
    // (contracts/c-api.md), and the 48 bytes are readable and aligned.
    unsafe {
        assert_eq!(
            rr_dr60_settings_validate(settings, &mut field),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(field, RrDr60SettingField::StructSize);
        assert_eq!(
            rr_dr60_create(settings, &mut out),
            RrDr60Status::InvalidArgument
        );
        assert!(out.is_null());
        let p = create(&rr_dr60_settings_default(48_000));
        assert_eq!(
            rr_dr60_reconfigure(p, settings),
            RrDr60Status::InvalidArgument
        );
        rr_dr60_destroy(p);
    }
}

#[test]
fn version_string_matches_crate() {
    // SAFETY: static NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(rr_dr60_version_string()) };
    assert_eq!(v.to_str().unwrap(), rr_dr60::VERSION);
}

#[cfg(feature = "ffi-test-panic")]
mod panics {
    use super::*;

    fn poison(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        unsafe {
            rr_dr60__test_force_panic(p);
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()),
                RrDr60Status::InternalError
            );
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()),
                RrDr60Status::InternalError,
                "not poisoned"
            );
            let mut lat = 0;
            assert_eq!(
                rr_dr60_latency_samples(p, &mut lat),
                RrDr60Status::InternalError
            );
        }
    }

    fn works(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()) },
            RrDr60Status::Ok
        );
    }

    #[test]
    fn panic_poisons_and_reset_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        // SAFETY: live handle.
        assert_eq!(unsafe { rr_dr60_reset(p) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }

    /// Spec 003 contracts/c-api.md: a poisoned handle still writes `out_info`
    /// (`produced = 0`, `events = 0`, the last `paused`).
    #[test]
    fn poisoned_handle_writes_out_info() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        let mut info = RrDr60BlockInfo {
            produced: 9,
            events: 9,
            paused: true,
            frames: 9,
            ..Default::default()
        };
        // SAFETY: live (poisoned) handle; valid buffers.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, &mut info) },
            RrDr60Status::InternalError
        );
        assert_eq!(info, RrDr60BlockInfo::default());
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }

    /// 006 T041: a panic inside `rr_dr60_record` poisons the handle; `rr_dr60_reset` recovers
    /// it with the recording in progress discarded (no flush, count unchanged).
    #[test]
    fn panic_while_recording_poisons_and_reset_discards() {
        let p = create(&rr_dr60_settings_default(48_000));
        let x = [0.1f32; 480];
        let mut info = RrDr60RecordInfo::default();
        let mut st = RrDr60EngineState::default();
        let mut status = RrDr60MemoryStatus::default();
        // SAFETY: live handle; valid buffers.
        unsafe {
            assert_eq!(rr_dr60_start_recording(p), RrDr60Status::Ok);
            assert_eq!(
                rr_dr60_record(p, x.as_ptr(), 480, &mut info, std::ptr::null_mut(), 0),
                RrDr60Status::Ok
            );
            assert!(info.stored > 0);
            rr_dr60__test_force_panic(p);
            assert_eq!(
                rr_dr60_record(p, x.as_ptr(), 480, &mut info, std::ptr::null_mut(), 0),
                RrDr60Status::InternalError
            );
            assert_eq!(info, RrDr60RecordInfo::default());
            assert_eq!(rr_dr60_get_state(p, &mut st), RrDr60Status::InternalError);
            assert_eq!(
                rr_dr60_stop_recording(p, std::ptr::null_mut(), std::ptr::null_mut(), 0),
                RrDr60Status::InternalError
            );
            assert_eq!(rr_dr60_reset(p), RrDr60Status::Ok);
            assert_eq!(rr_dr60_get_state(p, &mut st), RrDr60Status::Ok);
            assert_eq!(st.state, RrDr60State::Idle as u32);
            assert_eq!(rr_dr60_memory_status(p, &mut status), RrDr60Status::Ok);
            assert_eq!((status.count, status.used_samples), (0, 0));
            works(p);
            rr_dr60_destroy(p);
        }
    }

    #[test]
    fn panic_poisons_and_reconfigure_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        let s = rr_dr60_settings_default(16_000);
        // SAFETY: live handle; valid settings.
        assert_eq!(unsafe { rr_dr60_reconfigure(p, &s) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }
}

// ---- Spec 006 User Story 4 (FR-020, SC-005; T041): the record/stop/export/import/play loop
// through C against Rust, the state table as C codes, BUFFER_TOO_SMALL, the smoke reference.

/// The record half of the loop through C: `RecordInfo` per block, the stop outcome.
struct CRecorded {
    infos: Vec<RrDr60RecordInfo>,
    events: Vec<RrDr60VasEvent>,
    outcome: RrDr60StopOutcome,
}

fn c_record(p: *mut RrDr60Pipeline, x: &[f32], block: usize) -> CRecorded {
    let mut capacity = 0usize;
    // SAFETY: live handle; valid out pointer.
    assert_eq!(
        unsafe { rr_dr60_max_record_events(p, block, &mut capacity) },
        RrDr60Status::Ok
    );
    let mut slots = vec![RrDr60VasEvent::default(); capacity + 8];
    let mut r = CRecorded {
        infos: Vec::new(),
        events: Vec::new(),
        outcome: RrDr60StopOutcome::default(),
    };
    // SAFETY: live handle.
    assert_eq!(unsafe { rr_dr60_start_recording(p) }, RrDr60Status::Ok);
    for chunk in x.chunks(block) {
        let mut info = RrDr60RecordInfo::default();
        // SAFETY: live handle; the buffers are valid for their lengths.
        let status = unsafe {
            rr_dr60_record(
                p,
                chunk.as_ptr(),
                chunk.len(),
                &mut info,
                slots.as_mut_ptr(),
                slots.len(),
            )
        };
        assert_eq!(status, RrDr60Status::Ok);
        r.events.extend_from_slice(&slots[..info.events]);
        r.infos.push(info);
        assert!(!info.stopped);
    }
    // SAFETY: live handle; valid buffers.
    let status =
        unsafe { rr_dr60_stop_recording(p, &mut r.outcome, slots.as_mut_ptr(), slots.len()) };
    assert_eq!(status, RrDr60Status::Ok);
    r.events.extend_from_slice(&slots[..r.outcome.flush_events]);
    r
}

/// The playback half through C: the output to `finished`, with the per-block info checked.
fn c_play(p: *mut RrDr60Pipeline, number: u32, block: usize) -> Vec<f32> {
    // SAFETY: live handle.
    assert_eq!(
        unsafe { rr_dr60_start_playback(p, number) },
        RrDr60Status::Ok
    );
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block];
    loop {
        let mut info = RrDr60PlayInfo::default();
        // SAFETY: live handle; valid buffer.
        assert_eq!(
            unsafe { rr_dr60_play(p, buf.as_mut_ptr(), buf.len(), &mut info) },
            RrDr60Status::Ok
        );
        out.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
        assert_eq!(info.produced, block);
    }
    out
}

fn c_rec_info(i: rr_dr60::RecordInfo) -> RrDr60RecordInfo {
    RrDr60RecordInfo {
        stored: i.stored,
        events: i.events,
        paused: i.paused,
        stopped: i.stopped.is_some(),
        stop_reason: match i.stopped {
            Some(StopReason::MemoryFull) => RrDr60StopReason::MemoryFull as u32,
            _ => RrDr60StopReason::None as u32,
        },
        frames: i.frames,
        frames_total: i.frames_total,
        ..Default::default()
    }
}

fn c_play_info(i: rr_dr60::PlayInfo) -> RrDr60PlayInfo {
    RrDr60PlayInfo {
        produced: i.produced,
        finished: i.finished,
        frames: i.frames,
        frame_index: i.frame_index,
        place: i.place, // spec 013 FR-009
        ..Default::default()
    }
}

/// The playback half through C with every block's `PlayInfo`.
fn c_play_infos(
    p: *mut RrDr60Pipeline,
    number: u32,
    block: usize,
) -> (Vec<f32>, Vec<RrDr60PlayInfo>) {
    // SAFETY: live handle.
    assert_eq!(
        unsafe { rr_dr60_start_playback(p, number) },
        RrDr60Status::Ok
    );
    let (mut out, mut infos) = (Vec::new(), Vec::new());
    let mut buf = vec![0.0f32; block];
    loop {
        let mut info = RrDr60PlayInfo::default();
        // SAFETY: live handle; valid buffer.
        assert_eq!(
            unsafe { rr_dr60_play(p, buf.as_mut_ptr(), buf.len(), &mut info) },
            RrDr60Status::Ok
        );
        out.extend_from_slice(&buf[..info.produced]);
        infos.push(info);
        if info.finished {
            break;
        }
    }
    (out, infos)
}

/// The playback half through Rust with every block's `PlayInfo`, in the C form.
fn rust_play_infos(p: &mut Pipeline, number: u32, block: usize) -> (Vec<f32>, Vec<RrDr60PlayInfo>) {
    p.start_playback(number).unwrap();
    let (mut out, mut infos) = (Vec::new(), Vec::new());
    let mut buf = vec![0.0f32; block];
    loop {
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        infos.push(c_play_info(info));
        if info.finished {
            break;
        }
    }
    (out, infos)
}

fn c_state(p: *mut RrDr60Pipeline) -> RrDr60EngineState {
    let mut st = RrDr60EngineState::default();
    // SAFETY: live handle; valid out pointer.
    assert_eq!(unsafe { rr_dr60_get_state(p, &mut st) }, RrDr60Status::Ok);
    st
}

fn c_memory(p: *mut RrDr60Pipeline) -> RrDr60MemoryStatus {
    let mut st = RrDr60MemoryStatus::default();
    // SAFETY: live handle; valid out pointer.
    assert_eq!(
        unsafe { rr_dr60_memory_status(p, &mut st) },
        RrDr60Status::Ok
    );
    st
}

fn rust_state(p: &Pipeline) -> RrDr60EngineState {
    let st = p.state();
    RrDr60EngineState {
        state: match st.state {
            State::Idle => 0,
            State::Recording => 1,
            State::Playing => 2,
            _ => 99,
        },
        route: match st.route {
            Route::Speaker => 0,
            Route::Earphone => 1,
            _ => 99,
        },
        current_recording: st.current.unwrap_or(0),
        speaker_muted: st.speaker_muted,
        ..Default::default()
    }
}

fn rust_memory(p: &Pipeline) -> RrDr60MemoryStatus {
    let m = p.memory_status();
    RrDr60MemoryStatus {
        capacity_samples: m.capacity_samples,
        used_samples: m.used_samples,
        remaining_samples: m.remaining_samples,
        max_recordings: m.max_recordings,
        count: m.count,
        slots_remaining: m.slots_remaining,
        ..Default::default()
    }
}

/// US4 scenario 1 (FR-020): the loop at the seven rates through C versus Rust: per-block
/// `RecordInfo`, the events, the stop outcome, the state at each step, the memory figures, the
/// recording length, the exported bytes, the import into a second handle, both playbacks,
/// the three latency reads, erase and erase all; everything bit-identical or equal.
#[test]
fn c_api_record_play_loop_matches_rust_at_all_rates() {
    for rate in configs::all_rates() {
        let s = configs::memory_capacity(configs::settings("default_vas", rate), 10, 3);
        let (x, _) =
            chain_checks::speech_like_pattern(rate, chain_checks::BURST_DBFS, -60.0, 0x0D64);
        let x = &x[..(2 * rate as usize).min(x.len())];
        let block = 480;
        // Rust.
        let mut rust = Pipeline::new(s).unwrap();
        let mut slots = vec![VasEvent::default(); rust.max_record_events(block) + 8];
        let mut states = vec![rust_state(&rust)];
        rust.start_recording().unwrap();
        let mut want_infos = Vec::new();
        let mut want_events = Vec::new();
        for chunk in x.chunks(block) {
            let info = rust.record(chunk, &mut slots).unwrap();
            want_events.extend(slots[..info.events].iter().map(|&e| c_event(e)));
            want_infos.push(c_rec_info(info));
        }
        let out = rust.stop_recording(&mut slots).unwrap();
        want_events.extend(slots[..out.flush.events].iter().map(|&e| c_event(e)));
        let (kept, number, length) = match out.result {
            StopResult::Kept { number, length } => (true, number, length),
            _ => (false, 0, 0),
        };
        let want_outcome = RrDr60StopOutcome {
            flush_stored: out.flush.stored,
            flush_events: out.flush.events,
            flush_paused: out.flush.paused,
            flush_stopped: out.flush.stopped.is_some(),
            flush_stop_reason: c_rec_info(out.flush).stop_reason,
            flush_frames: out.flush.frames,
            flush_frames_total: out.flush.frames_total,
            kept,
            number,
            length_samples: length,
            silence_fed_samples: out.silence_fed_samples,
            stop_input_position: out.stop_input_position, // 020 D9
            padding_start_sample: out.padding_start_sample,
            ..Default::default()
        };
        states.push(rust_state(&rust));
        let want_memory = rust_memory(&rust);
        let want_bytes = split_checks::export_bytes(&rust, 1);
        let want_play = split_checks::play_to_end(&mut rust, 1, 997);
        // C.
        let p = create(&configs::c_settings(&s));
        let idle = c_state(p);
        assert_eq!(idle, states[0], "{rate} Hz: idle state");
        let r = c_record(p, x, block);
        assert_eq!(r.infos, want_infos, "{rate} Hz: per-block RecordInfo");
        assert_eq!(r.events, want_events, "{rate} Hz: events");
        assert_eq!(r.outcome, want_outcome, "{rate} Hz: stop outcome");
        assert_eq!(c_state(p), states[1], "{rate} Hz: state after stop");
        assert_eq!(c_memory(p), want_memory, "{rate} Hz: memory status");
        let mut len_samples = 0u64;
        let mut len = 0usize;
        // SAFETY: live handle; valid out pointers.
        unsafe {
            assert_eq!(
                rr_dr60_recording_length(p, 1, &mut len_samples),
                RrDr60Status::Ok
            );
            assert_eq!(len_samples, length);
            assert_eq!(rr_dr60_export_len(p, 1, &mut len), RrDr60Status::Ok);
        }
        assert_eq!(len, want_bytes.len());
        let mut bytes = vec![0u8; len];
        let mut written = 0usize;
        // SAFETY: live handle; valid buffer.
        assert_eq!(
            unsafe { rr_dr60_export(p, 1, bytes.as_mut_ptr(), bytes.len(), &mut written) },
            RrDr60Status::Ok
        );
        assert_eq!(written, len);
        assert_eq!(bytes, want_bytes, "{rate} Hz: exported bytes");
        // A second handle imports the bytes.
        let q = create(&configs::c_settings(&s));
        let mut imported = 0u32;
        // SAFETY: live handle; valid buffer and out pointer.
        assert_eq!(
            unsafe { rr_dr60_import(q, bytes.as_ptr(), bytes.len(), &mut imported) },
            RrDr60Status::Ok
        );
        assert_eq!(imported, 1);
        assert_eq!(c_play(p, 1, 997), want_play, "{rate} Hz: C playback");
        assert_eq!(c_play(q, 1, 997), want_play, "{rate} Hz: imported playback");
        assert_eq!(c_state(p), states[0]);
        // Latencies and the drain.
        let (mut a, mut b, mut c, mut f) = (0u32, 0u32, 0u32, 0usize);
        // SAFETY: live handle; valid out pointers.
        unsafe {
            assert_eq!(rr_dr60_record_latency_samples(p, &mut a), RrDr60Status::Ok);
            assert_eq!(
                rr_dr60_playback_latency_samples(p, &mut b),
                RrDr60Status::Ok
            );
            assert_eq!(rr_dr60_drain_samples(p, &mut c), RrDr60Status::Ok);
            assert_eq!(rr_dr60_footprint_bytes(p, &mut f), RrDr60Status::Ok);
        }
        assert_eq!(
            (a, b, c, f),
            (
                rust.record_latency_samples(),
                rust.playback_latency_samples(),
                rust.drain_samples(),
                rust.footprint_bytes()
            )
        );
        // Route round trip during playback, then erase and erase all.
        let mut route = 9u32;
        // SAFETY: live handle; valid out pointer.
        unsafe {
            assert_eq!(rr_dr60_start_playback(q, 1), RrDr60Status::Ok);
            assert_eq!(
                rr_dr60_set_route(q, RrDr60Route::Earphone as u32),
                RrDr60Status::Ok
            );
            assert_eq!(rr_dr60_get_route(q, &mut route), RrDr60Status::Ok);
            assert_eq!(route, 1);
            assert_eq!(c_state(q).route, 1);
            assert_eq!(
                rr_dr60_erase(q, 1),
                RrDr60Status::Ok,
                "erasing the played item stops playback"
            );
            assert_eq!(c_state(q).state, 0);
            assert_eq!(c_memory(q).count, 0);
            assert_eq!(
                rr_dr60_import(q, bytes.as_ptr(), bytes.len(), &mut imported),
                RrDr60Status::Ok
            );
            assert_eq!(
                rr_dr60_import(q, bytes.as_ptr(), bytes.len(), &mut imported),
                RrDr60Status::Ok
            );
            assert_eq!(imported, 2);
            assert_eq!(rr_dr60_erase_all(q), RrDr60Status::Ok);
            assert_eq!(c_memory(q), rust_memory(&Pipeline::new(s).unwrap()));
            rr_dr60_destroy(p);
            rr_dr60_destroy(q);
        }
    }
}

/// Spec 008 US4 scenario 1 (FR-019; T041): the loop with the speech codec at the seven rates
/// through C against Rust: per-block `RecordInfo` with the frame counts, the stop outcome with
/// `silence_fed_samples`, the exported frames form, both playbacks with every block's
/// `PlayInfo` (frames decoded, frame index), bit-identical or equal; then the one-pass with
/// `RR_DR60_TAP_AFTER_DECODER` (the codec-only configurations) equal to Rust's.
#[test]
fn c_api_codec_loop_and_decoder_tap_match_rust_at_all_rates() {
    for rate in configs::all_rates() {
        let s = configs::memory_capacity(configs::settings("default_codec", rate), 10, 3);
        let x = stimulus::voiced(f64::from(rate), 1.5, -20.0, 0x0D60);
        let block = 480;
        // Rust.
        let mut rust = Pipeline::new(s).unwrap();
        let mut slots = vec![VasEvent::default(); rust.max_record_events(block) + 8];
        rust.start_recording().unwrap();
        let mut want_infos = Vec::new();
        for chunk in x.chunks(block) {
            want_infos.push(c_rec_info(rust.record(chunk, &mut slots).unwrap()));
        }
        let out = rust.stop_recording(&mut slots).unwrap();
        let StopResult::Kept { number, length } = out.result else {
            panic!("{rate} Hz: empty")
        };
        assert_eq!(length % 120, 0);
        let want_outcome = RrDr60StopOutcome {
            flush_stored: out.flush.stored,
            flush_events: out.flush.events,
            flush_paused: out.flush.paused,
            flush_stopped: out.flush.stopped.is_some(),
            flush_stop_reason: c_rec_info(out.flush).stop_reason,
            flush_frames: out.flush.frames,
            flush_frames_total: out.flush.frames_total,
            kept: true,
            number,
            length_samples: length,
            silence_fed_samples: out.silence_fed_samples,
            stop_input_position: out.stop_input_position, // 020 D9
            padding_start_sample: out.padding_start_sample,
            ..Default::default()
        };
        assert!(want_outcome.flush_frames_total * 120 == length);
        let want_bytes = split_checks::export_bytes(&rust, number);
        assert_eq!(u16::from_le_bytes([want_bytes[22], want_bytes[23]]), 1);
        let (want_play, want_play_infos) = rust_play_infos(&mut rust, number, 997);
        // C.
        let p = create(&configs::c_settings(&s));
        let r = c_record(p, &x, block);
        assert_eq!(
            r.infos, want_infos,
            "{rate} Hz: per-block RecordInfo with frames"
        );
        assert_eq!(
            r.outcome, want_outcome,
            "{rate} Hz: stop outcome with silence fed"
        );
        let mut len = 0usize;
        let mut encoding = 0u32;
        // SAFETY: live handle; valid out pointers.
        unsafe {
            assert_eq!(rr_dr60_export_len(p, number, &mut len), RrDr60Status::Ok);
            assert_eq!(rr_dr60_get_encoding(p, &mut encoding), RrDr60Status::Ok);
        }
        assert_eq!(encoding, RrDr60Encoding::Frames as u32);
        assert_eq!(len, want_bytes.len());
        assert_eq!(len, 52 + 10 * (length / 120) as usize);
        let mut bytes = vec![0u8; len];
        let mut written = 0usize;
        // SAFETY: live handle; valid buffer.
        assert_eq!(
            unsafe { rr_dr60_export(p, number, bytes.as_mut_ptr(), len, &mut written) },
            RrDr60Status::Ok
        );
        assert_eq!(bytes, want_bytes, "{rate} Hz: the frames form");
        let q = create(&configs::c_settings(&s));
        let mut imported = 0u32;
        // SAFETY: live handle; valid buffer and out pointer.
        assert_eq!(
            unsafe { rr_dr60_import(q, bytes.as_ptr(), len, &mut imported) },
            RrDr60Status::Ok
        );
        let (c_out, c_infos) = c_play_infos(p, number, 997);
        assert_eq!(c_out, want_play, "{rate} Hz: C playback through the codec");
        assert_eq!(c_infos, want_play_infos, "{rate} Hz: per-block PlayInfo");
        let (q_out, q_infos) = c_play_infos(q, 1, 997);
        assert_eq!(q_out, want_play, "{rate} Hz: imported playback");
        assert_eq!(q_infos, want_play_infos);
        // A bypassed handle refuses the frames with 21.
        let mut off = s;
        off.speech_codec_enabled = false;
        let o = create(&configs::c_settings(&off));
        // SAFETY: live handles; valid buffer.
        unsafe {
            assert_eq!(
                rr_dr60_import(o, bytes.as_ptr(), len, &mut imported),
                RrDr60Status::EncodingDiffers
            );
            rr_dr60_destroy(p);
            rr_dr60_destroy(q);
            rr_dr60_destroy(o);
        }
        // The one-pass at the decoder tap.
        for config in ["codec_only", "codec_only_vas", "default_codec"] {
            let s = configs::settings(config, rate);
            let (want, want_records) = rust_blocks(s, &x, 997);
            let (got, got_records) = c_blocks(&configs::c_settings(&s), &x, 997);
            assert_eq!(got.len(), want.len(), "{rate} Hz {config}");
            assert!(
                got.iter()
                    .zip(&want)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{rate} Hz {config}: one-pass output"
            );
            assert_eq!(got_records, want_records, "{rate} Hz {config}: block info");
        }
    }
}

/// SC-005 (FR-020): the state table's refusals as C codes, each leaving the state unchanged.
#[test]
fn c_api_state_table_refusals() {
    let s = configs::memory_capacity(configs::settings("default_agc", 48_000), 1, 1);
    let p = create(&configs::c_settings(&s));
    let x = [0.25f32; 4800];
    let mut y = [0.0f32; 480];
    let mut info = RrDr60RecordInfo::default();
    let mut play = RrDr60PlayInfo::default();
    let mut outcome = RrDr60StopOutcome::default();
    let c = configs::c_settings(&s);
    // SAFETY: live handle; valid buffers.
    unsafe {
        // Idle.
        assert_eq!(
            rr_dr60_record(p, x.as_ptr(), 480, &mut info, std::ptr::null_mut(), 0),
            RrDr60Status::NotRecording
        );
        assert_eq!(
            rr_dr60_stop_recording(p, &mut outcome, std::ptr::null_mut(), 0),
            RrDr60Status::NotRecording
        );
        assert_eq!(
            rr_dr60_play(p, y.as_mut_ptr(), 480, &mut play),
            RrDr60Status::NotPlaying
        );
        assert_eq!(rr_dr60_stop_playback(p), RrDr60Status::NotPlaying);
        assert_eq!(rr_dr60_start_playback(p, 0), RrDr60Status::NoSuchRecording);
        assert_eq!(rr_dr60_start_playback(p, 1), RrDr60Status::NoSuchRecording);
        assert_eq!(rr_dr60_erase(p, 1), RrDr60Status::NoSuchRecording);
        let mut n = 0u64;
        assert_eq!(
            rr_dr60_recording_length(p, 1, &mut n),
            RrDr60Status::NoSuchRecording
        );
        assert_eq!(c_state(p).state, 0);
        // Recording.
        assert_eq!(rr_dr60_start_recording(p), RrDr60Status::Ok);
        assert_eq!(
            c_state(p),
            RrDr60EngineState {
                state: 1,
                route: 0,
                current_recording: 1,
                speaker_muted: true,
                ..Default::default()
            }
        );
        assert_eq!(
            rr_dr60_start_recording(p),
            RrDr60Status::RecordingInProgress
        );
        assert_eq!(
            rr_dr60_start_playback(p, 1),
            RrDr60Status::RecordingInProgress
        );
        assert_eq!(rr_dr60_erase(p, 1), RrDr60Status::RecordingInProgress);
        assert_eq!(rr_dr60_erase_all(p), RrDr60Status::RecordingInProgress);
        assert_eq!(
            rr_dr60_import(p, x.as_ptr().cast(), 8, std::ptr::null_mut()),
            RrDr60Status::RecordingInProgress
        );
        assert_eq!(
            rr_dr60_reconfigure(p, &c),
            RrDr60Status::RecordingInProgress
        );
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 480, std::ptr::null_mut()),
            RrDr60Status::RecordingInProgress
        );
        assert_eq!(
            rr_dr60_play(p, y.as_mut_ptr(), 480, &mut play),
            RrDr60Status::NotPlaying
        );
        assert_eq!(c_state(p).state, 1);
        // Fill the 1 s memory inside a block: stopped with MEMORY_FULL, then NOT_RECORDING.
        let mut total = 0;
        loop {
            assert_eq!(
                rr_dr60_record(p, x.as_ptr(), 4800, &mut info, std::ptr::null_mut(), 0),
                RrDr60Status::Ok
            );
            total += info.stored;
            if info.stopped {
                break;
            }
        }
        assert_eq!(info.stop_reason, RrDr60StopReason::MemoryFull as u32);
        assert_eq!(total, 6000);
        assert_eq!(c_state(p).state, 0);
        assert_eq!(
            rr_dr60_stop_recording(p, &mut outcome, std::ptr::null_mut(), 0),
            RrDr60Status::NotRecording
        );
        assert_eq!(rr_dr60_start_recording(p), RrDr60Status::MemoryFull);
        // Playing.
        assert_eq!(rr_dr60_start_playback(p, 1), RrDr60Status::Ok);
        assert_eq!(
            c_state(p),
            RrDr60EngineState {
                state: 2,
                route: 0,
                current_recording: 1,
                speaker_muted: false,
                ..Default::default()
            }
        );
        assert_eq!(rr_dr60_start_recording(p), RrDr60Status::PlaybackInProgress);
        assert_eq!(
            rr_dr60_start_playback(p, 1),
            RrDr60Status::PlaybackInProgress
        );
        assert_eq!(rr_dr60_reconfigure(p, &c), RrDr60Status::PlaybackInProgress);
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 480, std::ptr::null_mut()),
            RrDr60Status::PlaybackInProgress
        );
        assert_eq!(
            rr_dr60_record(p, x.as_ptr(), 480, &mut info, std::ptr::null_mut(), 0),
            RrDr60Status::NotRecording
        );
        assert_eq!(rr_dr60_stop_playback(p), RrDr60Status::Ok);
        assert_eq!(c_state(p).state, 0);
        // Slots: a 1-slot memory after an erase and a re-import... the time is the limit here;
        // use a fresh 10 s / 1-slot handle for NO_RECORDING_SLOTS and DOES_NOT_FIT.
        let mut len = 0usize;
        assert_eq!(rr_dr60_export_len(p, 1, &mut len), RrDr60Status::Ok);
        let mut bytes = vec![0u8; len];
        let mut written = 0usize;
        assert_eq!(
            rr_dr60_export(p, 1, bytes.as_mut_ptr(), len, &mut written),
            RrDr60Status::Ok
        );
        let two_slots = configs::c_settings(&configs::memory_capacity(
            configs::settings("default_agc", 48_000),
            10,
            2,
        ));
        let q = create(&two_slots);
        let mut number = 0u32;
        assert_eq!(
            rr_dr60_import(q, bytes.as_ptr(), len, &mut number),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_import(q, bytes.as_ptr(), len, &mut number),
            RrDr60Status::Ok
        );
        assert_eq!(number, 2);
        assert_eq!(rr_dr60_start_recording(q), RrDr60Status::NoRecordingSlots);
        assert_eq!(
            rr_dr60_import(q, bytes.as_ptr(), len, &mut number),
            RrDr60Status::DoesNotFit
        );
        // Corrupt forms.
        assert_eq!(
            rr_dr60_import(q, bytes.as_ptr(), 4, &mut number),
            RrDr60Status::UnknownForm
        );
        let mut v3 = bytes.clone();
        v3[8] = 3; // a later form version (version 2 is this release's, spec 008 FR-011)
        assert_eq!(
            rr_dr60_import(q, v3.as_ptr(), v3.len(), &mut number),
            RrDr60Status::UnsupportedFormVersion
        );
        let mut flipped = bytes.clone();
        flipped[40] ^= 1;
        assert_eq!(
            rr_dr60_import(q, flipped.as_ptr(), flipped.len(), &mut number),
            RrDr60Status::Damaged
        );
        assert_eq!(
            rr_dr60_import(q, bytes.as_ptr(), len - 1, &mut number),
            RrDr60Status::Damaged
        );
        // Reconfigure below the stored time.
        let same = configs::c_settings(&configs::memory_capacity(
            configs::settings("default_agc", 48_000),
            1,
            1,
        ));
        assert_eq!(
            rr_dr60_reconfigure(p, &same),
            RrDr60Status::Ok,
            "same capacity: kept"
        );
        // q holds 2 × 6000 samples in 2 slots: neither 1 s nor 1 slot fits them.
        let mut tiny = two_slots;
        tiny.memory_capacity_s = 1;
        assert_eq!(
            rr_dr60_reconfigure(q, &tiny),
            RrDr60Status::StoredExceedsCapacity
        );
        tiny = two_slots;
        tiny.memory_max_recordings = 1;
        assert_eq!(
            rr_dr60_reconfigure(q, &tiny),
            RrDr60Status::StoredExceedsCapacity
        );
        assert_eq!(c_memory(q).count, 2, "a refusal changes nothing");
        rr_dr60_destroy(p);
        rr_dr60_destroy(q);
    }
}

/// FR-020 (contracts/c-api.md): `BUFFER_TOO_SMALL` leaves the buffer untouched and reports the
/// needed length in `out_written` (when not NULL).
#[test]
fn c_api_buffer_too_small_leaves_the_buffer_untouched() {
    let s = configs::memory_capacity(configs::settings("default_agc", 48_000), 10, 3);
    let p = create(&configs::c_settings(&s));
    let x = [0.25f32; 4800];
    let mut outcome = RrDr60StopOutcome::default();
    let mut needed = 0usize;
    let mut buf = vec![0xAAu8; 10];
    let mut written = 0usize;
    // SAFETY: live handle; valid buffers.
    unsafe {
        assert_eq!(rr_dr60_start_recording(p), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_record(
                p,
                x.as_ptr(),
                4800,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_stop_recording(p, &mut outcome, std::ptr::null_mut(), 0),
            RrDr60Status::Ok
        );
        assert_eq!(rr_dr60_export_len(p, 1, &mut needed), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_export(p, 1, buf.as_mut_ptr(), buf.len(), &mut written),
            RrDr60Status::BufferTooSmall
        );
        assert_eq!(written, needed);
        assert!(buf.iter().all(|&b| b == 0xAA));
        assert_eq!(
            rr_dr60_export(p, 1, buf.as_mut_ptr(), buf.len(), std::ptr::null_mut()),
            RrDr60Status::BufferTooSmall
        );
        let mut full = vec![0u8; needed + 3];
        assert_eq!(
            rr_dr60_export(p, 1, full.as_mut_ptr(), full.len(), &mut written),
            RrDr60Status::Ok
        );
        assert_eq!(written, needed);
        assert_eq!(&full[..8], b"RRDR60RC");
        rr_dr60_destroy(p);
    }
}

/// The smoke test's reference (US4 scenario 1; contracts/c-api.md › Smoke test additions): the
/// Rust playback of the smoke stimulus hashes to the constant `smoke.c` embeds, so the C
/// program compares its playback with the Rust path's bit for bit.
#[test]
fn smoke_reference_hash_is_pinned() {
    let r = smoke_reference::rust_reference();
    println!(
        "smoke reference (0.8 default): playback {} samples, FNV-1a 64 {:#018x}; frames {} (hash {:#018x}); silence fed {}",
        r.playback_samples, r.playback_hash, r.frames, r.frames_hash, r.silence_fed
    );
    assert_eq!(r.playback_hash, smoke_reference::SMOKE_REFERENCE_HASH);
    assert_eq!(r.playback_samples, smoke_reference::SMOKE_PLAYBACK_SAMPLES);
    assert_eq!(r.frames_hash, smoke_reference::SMOKE_FRAMES_HASH);
    assert_eq!(r.frames, smoke_reference::SMOKE_FRAMES);
    assert_eq!(r.silence_fed, smoke_reference::SMOKE_SILENCE_FED);
    assert_eq!(
        smoke_reference::rust_playback_hash(),
        (r.playback_hash, r.playback_samples)
    );
    // Spec 009 T033: the four route cases (the chain on), pinned once.
    let routes = smoke_reference::rust_route_references();
    for (route, volume, hash, len) in routes {
        println!("smoke route case {route:?} at {volume}: {len} samples, FNV-1a 64 {hash:#018x}");
    }
    let want = [
        smoke_reference::SMOKE_SPEAKER_REF_HASH,
        smoke_reference::SMOKE_SPEAKER_FULL_HASH,
        smoke_reference::SMOKE_EARPHONE_REF_HASH,
        smoke_reference::SMOKE_EARPHONE_FULL_HASH,
    ];
    for (i, (route, volume, hash, len)) in routes.into_iter().enumerate() {
        assert_eq!(hash, want[i], "{route:?} at {volume}");
        assert_eq!(
            len,
            smoke_reference::SMOKE_ROUTE_PLAYBACK_SAMPLES,
            "{route:?} at {volume}"
        );
    }
    // Spec 010 T031: the two clamp cases (a full-scale square, the option on, position 1).
    let (clamp, beyond) = smoke_reference::rust_clamp_references();
    for (route, hash, len) in clamp {
        println!("smoke clamp case {route:?} at 1: {len} samples, FNV-1a 64 {hash:#018x}");
    }
    println!("smoke clamp case, option off, speaker route: {beyond} samples beyond the bound");
    assert!(beyond > 0, "the option-off playback exceeds the bound");
    assert_eq!(clamp[0].1, smoke_reference::SMOKE_SPEAKER_CLAMP_HASH);
    assert_eq!(clamp[1].1, smoke_reference::SMOKE_EARPHONE_CLAMP_HASH);
    assert_eq!(clamp[0].2, smoke_reference::SMOKE_ROUTE_PLAYBACK_SAMPLES);
    assert_eq!(clamp[1].2, smoke_reference::SMOKE_ROUTE_PLAYBACK_SAMPLES);
    // Spec 011 T035: the volume-transition case (the square at 0.1, a change before play block
    // 10 and a retarget before block 11, on the speaker route).
    let (ramp, ramp_len) = smoke_reference::rust_ramp_reference();
    println!("smoke volume-transition case: {ramp_len} samples, FNV-1a 64 {ramp:#018x}");
    assert_eq!(ramp, smoke_reference::SMOKE_SPEAKER_RAMP_HASH);
    assert_eq!(ramp_len, smoke_reference::SMOKE_ROUTE_PLAYBACK_SAMPLES);
    // Spec 013 T029: the playback-speed case (the square at level 5, level 1 from play block 10).
    let (speed, speed_len, speed_helper) = smoke_reference::rust_speed_reference();
    println!(
        "smoke speed case: {speed_len} samples, FNV-1a 64 {speed:#018x}, length helper at level 5 {speed_helper}"
    );
    assert_eq!(speed, smoke_reference::SMOKE_SPEED_HASH);
    assert_eq!(speed_len, smoke_reference::SMOKE_SPEED_SAMPLES);
    // Spec 012 T049: the microphone cases (the 0.12 default one-pass; two hiss recordings with
    // a reset between them, which differ).
    let ((mic, mic_len), recs) = smoke_reference::rust_mic_references();
    println!(
        "smoke microphone case (0.12 default one-pass): {mic_len} samples, FNV-1a 64 {mic:#018x}"
    );
    for (k, (h, len)) in recs.iter().enumerate() {
        println!("smoke hiss recording {k}: {len} samples, FNV-1a 64 {h:#018x}");
    }
    assert_eq!(mic, smoke_reference::SMOKE_MIC_DEFAULT_HASH);
    assert_eq!(mic_len, smoke_reference::SMOKE_MIC_DEFAULT_SAMPLES);
    assert_eq!(recs[0].0, smoke_reference::SMOKE_MIC_REC0_HASH);
    assert_eq!(recs[1].0, smoke_reference::SMOKE_MIC_REC1_HASH);
    assert_ne!(
        recs[0].0, recs[1].0,
        "the two recordings carry different hiss (012 FR-009)"
    );
    for (_, len) in recs {
        assert_eq!(len, smoke_reference::SMOKE_MIC_REC_SAMPLES);
    }
}

/// Spec 012 T049 (FR-012, SC-009; contracts/c-api.md): at the measurement rates the C handle
/// and the Rust engine, built from the same settings for each of the five microphone
/// configurations and fed the smoke square (exactly representable) plus silence through the
/// one-pass, give bit-identical output; for the two default configurations a record-then-play
/// through C equals Rust's, and so does a second recording after `rr_dr60_reset` (the counter).
#[test]
fn c_api_mic_matches_rust_at_all_rates() {
    for &rate in configs::measurement_rates() {
        let mut x: Vec<f32> = (0..rate as usize)
            .map(|i| {
                if (i * 48 / rate as usize) % 48 < 24 {
                    0.125
                } else {
                    -0.125
                }
            })
            .collect();
        x.extend(std::iter::repeat_n(0.0f32, rate as usize / 4));
        for config in configs::MIC_CONFIGS {
            let s = configs::memory_capacity(configs::settings(config, rate), 10, 3);
            let cs = configs::c_settings(&s);
            assert_eq!(
                (
                    cs.mic_capsule_enabled,
                    cs.mic_amp_enabled,
                    cs.mic_hiss_offset_db
                ),
                (
                    s.mic.capsule_enabled,
                    s.mic.amp_enabled,
                    s.mic.hiss_offset_db
                ),
                "{config}"
            );
            let mut rp = Pipeline::new(s).unwrap();
            let cp = create(&cs);
            let (mut r_out, mut c_out) = (Vec::new(), Vec::new());
            for chunk in x.chunks(1024) {
                let mut y = vec![0.0f32; chunk.len()];
                let n = rp.process(chunk, &mut y).unwrap().produced;
                r_out.extend_from_slice(&y[..n]);
                let mut info = RrDr60BlockInfo::default();
                // SAFETY: live handle; valid buffers.
                let status = unsafe {
                    rr_dr60_process(cp, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
                };
                assert_eq!(status, RrDr60Status::Ok);
                c_out.extend_from_slice(&y[..info.produced]);
            }
            assert!(!r_out.is_empty());
            assert_eq!(
                r_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz one-pass {config}"
            );
            // SAFETY: live handle, destroyed once.
            unsafe { rr_dr60_destroy(cp) };
            if config.starts_with("default_") {
                let mut rp = Pipeline::new(s).unwrap();
                let cp2 = create(&cs);
                for second in [false, true] {
                    if second {
                        rp.reset();
                        // SAFETY: live handle.
                        assert_eq!(unsafe { rr_dr60_reset(cp2) }, RrDr60Status::Ok);
                    }
                    let run = split_checks::record_then_play(&mut rp, &x, 480, 480);
                    let mut r_play = run.pre_drain.clone();
                    r_play.extend_from_slice(&run.drain);
                    let rec = c_record(cp2, &x, 480);
                    assert!(rec.outcome.kept);
                    let c_play_out = c_play(cp2, rec.outcome.number, 480);
                    assert_eq!(
                        r_play.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        c_play_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        "{rate} Hz record-then-play {config} (second: {second})"
                    );
                }
                // SAFETY: live handle, destroyed once.
                unsafe { rr_dr60_destroy(cp2) };
            }
        }
    }
}

// ---- Spec 009 User Story 4: the output chain through the C API (tasks.md T030) ----

/// Spec 009 US4 scenarios 1 and 3 (FR-014; SC-009): at the seven rates the C handle and the
/// Rust engine, fed the voiced stimulus through the one-pass on both routes at two positions,
/// give bit-identical output; record / stop / play through C on both routes equals Rust's;
/// `rr_dr60_set_volume` / `rr_dr60_get_volume` round-trip four positions; a route change
/// between C `process` calls lands at the same block as Rust's.
#[test]
fn c_api_output_chain_matches_rust_at_all_rates() {
    use rr_dr60::output::VOLUME_REFERENCE;
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x0D60);
        let s = configs::settings("default_output", rate);
        let cs = configs::c_settings(&s);
        for (route, volume) in [
            (Route::Speaker, VOLUME_REFERENCE),
            (Route::Speaker, 1.0),
            (Route::Earphone, VOLUME_REFERENCE),
            (Route::Earphone, 1.0),
        ] {
            // The one-pass, in 1024-sample blocks.
            let mut rp = Pipeline::new(s).unwrap();
            rp.set_route(route);
            rp.set_volume(volume).unwrap();
            let cp = create(&cs);
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_set_route(cp, if route == Route::Speaker { 0 } else { 1 }) },
                RrDr60Status::Ok
            );
            // SAFETY: live handle.
            assert_eq!(unsafe { rr_dr60_set_volume(cp, volume) }, RrDr60Status::Ok);
            let mut got = 0.0f32;
            // SAFETY: live handle; valid out pointer.
            assert_eq!(
                unsafe { rr_dr60_get_volume(cp, &mut got) },
                RrDr60Status::Ok
            );
            assert_eq!(got, volume);
            let (mut r_out, mut c_out) = (Vec::new(), Vec::new());
            for chunk in x.chunks(1024) {
                let mut y = vec![0.0f32; chunk.len()];
                let n = rp.process(chunk, &mut y).unwrap().produced;
                r_out.extend_from_slice(&y[..n]);
                let mut info = RrDr60BlockInfo::default();
                // SAFETY: live handle; valid buffers.
                let status = unsafe {
                    rr_dr60_process(cp, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
                };
                assert_eq!(status, RrDr60Status::Ok);
                c_out.extend_from_slice(&y[..info.produced]);
            }
            assert_eq!(
                r_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz one-pass {route:?} at {volume}"
            );
            // Record then play, both sides.
            let mut rp = Pipeline::new(s).unwrap();
            rp.set_route(route);
            rp.set_volume(volume).unwrap();
            let run = split_checks::record_then_play(&mut rp, &x, 480, 480);
            let mut r_play = run.pre_drain.clone();
            r_play.extend_from_slice(&run.drain);
            let cp2 = create(&cs);
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_set_route(cp2, if route == Route::Speaker { 0 } else { 1 }) },
                RrDr60Status::Ok
            );
            // SAFETY: live handle.
            assert_eq!(unsafe { rr_dr60_set_volume(cp2, volume) }, RrDr60Status::Ok);
            let rec = c_record(cp2, &x, 480);
            assert!(rec.outcome.kept);
            let c_play_out = c_play(cp2, rec.outcome.number, 480);
            assert_eq!(
                r_play.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_play_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz record-then-play {route:?} at {volume}"
            );
            // SAFETY: live handles.
            unsafe {
                rr_dr60_destroy(cp);
                rr_dr60_destroy(cp2);
            }
        }
        // A route change between calls lands at the same block on both sides (FR-009).
        let mut rp = Pipeline::new(s).unwrap();
        rp.set_volume(1.0).unwrap();
        let cp = create(&cs);
        // SAFETY: live handle.
        assert_eq!(unsafe { rr_dr60_set_volume(cp, 1.0) }, RrDr60Status::Ok);
        let (mut r_out, mut c_out) = (Vec::new(), Vec::new());
        for (i, chunk) in x.chunks(256).enumerate() {
            let route = if i < 8 {
                Route::Speaker
            } else {
                Route::Earphone
            };
            rp.set_route(route);
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_set_route(cp, if route == Route::Speaker { 0 } else { 1 }) },
                RrDr60Status::Ok
            );
            let mut y = vec![0.0f32; chunk.len()];
            let n = rp.process(chunk, &mut y).unwrap().produced;
            r_out.extend_from_slice(&y[..n]);
            let mut info = RrDr60BlockInfo::default();
            // SAFETY: live handle; valid buffers.
            let status = unsafe {
                rr_dr60_process(cp, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
            };
            assert_eq!(status, RrDr60Status::Ok);
            c_out.extend_from_slice(&y[..info.produced]);
        }
        assert_eq!(
            r_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            c_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "{rate} Hz route change"
        );
        // SAFETY: live handle.
        unsafe { rr_dr60_destroy(cp) };
    }
}

// ---- Spec 007 User Story 3: the manifest through the C API equals the Rust manifest ----

fn cstr(p: *const std::ffi::c_char) -> &'static str {
    assert!(!p.is_null(), "a manifest string pointer is NULL");
    // SAFETY: every manifest string is a static NUL-terminated UTF-8 string (contracts/c-api.md).
    unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap()
}

#[test]
fn manifest_header_matches_rust() {
    // 007 FR-017; US3 scenarios 1 and 4.
    let m = rr_dr60::manifest();
    let mut h = RrDr60ManifestHeader::default();
    // SAFETY: valid out pointer.
    assert_eq!(unsafe { rr_dr60_manifest_header(&mut h) }, RrDr60Status::Ok);
    assert_eq!(cstr(h.library_version), m.header.library_version);
    assert_eq!(h.device_rate_hz, m.header.device_rate_hz);
    assert_eq!(h.stage_count, m.header.stage_count);
    assert_eq!(h.count_not_modeled, m.header.counts.not_modeled);
    assert_eq!(h.count_assumed, m.header.counts.assumed);
    assert_eq!(h.count_sourced, m.header.counts.sourced);
    assert_eq!(h.count_checked, m.header.counts.checked);
    assert_eq!(cstr(h.basis), m.header.basis);
    assert_eq!(cstr(h.evidence_link), m.header.evidence_link);
    assert_eq!(cstr(h.assumptions_link), m.header.assumptions_link);
    assert_eq!(cstr(h.sources_link), m.header.sources_link);
    assert_eq!(rr_dr60_manifest_stage_count(), m.header.stage_count);
}

#[test]
fn manifest_stages_match_rust() {
    // 007 FR-017; US3 scenario 1: every field of every stage, the IDs through the accessors.
    for e in rr_dr60::manifest().stages() {
        let n = u32::from(e.number);
        let mut c = RrDr60StageEntry::default();
        // SAFETY: valid out pointer.
        assert_eq!(
            unsafe { rr_dr60_manifest_stage(n, &mut c) },
            RrDr60Status::Ok
        );
        assert_eq!(c.number, n);
        assert_eq!(cstr(c.identifier), e.identifier);
        assert_eq!(cstr(c.name), e.name);
        assert_eq!(cstr(c.description), e.description);
        assert_eq!(c.status, e.status as u32);
        assert_eq!(c.path, e.path as u32);
        assert_eq!(c.confidence, e.confidence as u32);
        assert_eq!(c.source_count as usize, e.sources.len());
        assert_eq!(c.assumption_count as usize, e.assumptions.len());
        assert_eq!(cstr(c.coverage), e.coverage);
        assert_eq!(cstr(c.controls), e.controls);
        assert_eq!(cstr(c.model_version), e.model_version);
        for (i, want) in e.sources.iter().enumerate() {
            let mut id = std::ptr::null();
            // SAFETY: valid out pointer.
            assert_eq!(
                unsafe { rr_dr60_manifest_stage_source(n, i as u32, &mut id) },
                RrDr60Status::Ok
            );
            assert_eq!(cstr(id), *want);
        }
        for (i, want) in e.assumptions.iter().enumerate() {
            let mut id = std::ptr::null();
            // SAFETY: valid out pointer.
            assert_eq!(
                unsafe { rr_dr60_manifest_stage_assumption(n, i as u32, &mut id) },
                RrDr60Status::Ok
            );
            assert_eq!(cstr(id), *want);
        }
    }
}

#[test]
fn manifest_refusals() {
    // US3 scenario 2: 0 and 16 refused with a distinct code; an index at the count refused.
    let mut c = RrDr60StageEntry::default();
    let mut id = std::ptr::null();
    // SAFETY: valid out pointers.
    unsafe {
        assert_eq!(rr_dr60_manifest_stage(0, &mut c), RrDr60Status::NoSuchStage);
        assert_eq!(
            rr_dr60_manifest_stage(16, &mut c),
            RrDr60Status::NoSuchStage
        );
        assert_eq!(rr_dr60_manifest_stage(3, &mut c), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_manifest_stage_source(3, c.source_count, &mut id),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_manifest_stage_assumption(3, c.assumption_count, &mut id),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_manifest_stage_assumption(16, 0, &mut id),
            RrDr60Status::NoSuchStage
        );
    }
    assert!(id.is_null());
}

// ---- Spec 010 User Story 4: the output clamp through the C API (tasks.md T027) ----

/// Spec 010 US4 scenario 1 (FR-008; SC-006): at the measurement rates the C handle and the
/// Rust engine, fed the full-scale square through the one-pass in the six clamp configurations
/// at their route and position, give bit-identical output with every C sample within [−1, 1];
/// record / stop / play through C in the three `default_*` names equals Rust's.
#[test]
fn c_api_output_clamp_matches_rust_at_all_rates() {
    use rr_dr60_harness::output_clamp_checks::stimulus_by_name;
    for &rate in configs::measurement_rates() {
        let x = stimulus_by_name("square_1k_fs", rate, 1.0);
        for config in configs::OUTPUT_CLAMP_CONFIGS {
            let s = configs::settings(config, rate);
            let cs = configs::c_settings(&s);
            assert!(cs.output_clamp_enabled, "{config}");
            let (route, volume) = configs::output_clamp_route_volume(config);
            let mut rp = Pipeline::new(s).unwrap();
            rp.set_route(route);
            rp.set_volume(volume).unwrap();
            let cp = create(&cs);
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_set_route(cp, if route == Route::Speaker { 0 } else { 1 }) },
                RrDr60Status::Ok
            );
            // SAFETY: live handle.
            assert_eq!(unsafe { rr_dr60_set_volume(cp, volume) }, RrDr60Status::Ok);
            let (mut r_out, mut c_out) = (Vec::new(), Vec::new());
            for chunk in x.chunks(1024) {
                let mut y = vec![0.0f32; chunk.len()];
                let n = rp.process(chunk, &mut y).unwrap().produced;
                r_out.extend_from_slice(&y[..n]);
                let mut info = RrDr60BlockInfo::default();
                // SAFETY: live handle; valid buffers.
                let status = unsafe {
                    rr_dr60_process(cp, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
                };
                assert_eq!(status, RrDr60Status::Ok);
                c_out.extend_from_slice(&y[..info.produced]);
            }
            assert_eq!(
                r_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz one-pass {config}"
            );
            assert!(
                c_out.iter().all(|v| v.abs() <= 1.0),
                "{rate} Hz {config}: bounded"
            );
            // SAFETY: live handle, destroyed once.
            unsafe { rr_dr60_destroy(cp) };
            if config.starts_with("default_") {
                let mut rp = Pipeline::new(s).unwrap();
                rp.set_route(route);
                rp.set_volume(volume).unwrap();
                let run = split_checks::record_then_play(&mut rp, &x, 480, 480);
                let mut r_play = run.pre_drain.clone();
                r_play.extend_from_slice(&run.drain);
                let cp2 = create(&cs);
                // SAFETY: live handle.
                assert_eq!(
                    unsafe { rr_dr60_set_route(cp2, if route == Route::Speaker { 0 } else { 1 }) },
                    RrDr60Status::Ok
                );
                // SAFETY: live handle.
                assert_eq!(unsafe { rr_dr60_set_volume(cp2, volume) }, RrDr60Status::Ok);
                let rec = c_record(cp2, &x, 480);
                assert!(rec.outcome.kept);
                let c_play_out = c_play(cp2, rec.outcome.number, 480);
                assert_eq!(
                    r_play.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    c_play_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "{rate} Hz record-then-play {config}"
                );
                assert!(
                    c_play_out.iter().all(|v| v.abs() <= 1.0),
                    "{rate} Hz {config}: play bounded"
                );
                // SAFETY: live handle, destroyed once.
                unsafe { rr_dr60_destroy(cp2) };
            }
        }
    }
}

/// Spec 011 T036 (contracts/c-api.md › Parity): a volume change mid-stream through C equals the
/// Rust engine bit for bit at the seven rates on both routes, in the one-pass (1024-sample
/// blocks) and in the player (480-sample blocks): 0.1 at the start, 1.0 before block 10, 0.4
/// before block 11 (a retarget mid-transition).
#[test]
fn c_api_volume_transition_matches_rust_at_all_rates() {
    let changes = [(10usize, 1.0f32), (11, 0.4)];
    let c_route = |route: Route| if route == Route::Speaker { 0 } else { 1 };
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x0D60);
        let s = configs::settings("default_output", rate);
        let cs = configs::c_settings(&s);
        for route in [Route::Speaker, Route::Earphone] {
            // The one-pass.
            let mut rp = Pipeline::new(s).unwrap();
            rp.set_route(route);
            rp.set_volume(0.1).unwrap();
            let cp = create(&cs);
            // SAFETY: live handle.
            unsafe {
                assert_eq!(rr_dr60_set_route(cp, c_route(route)), RrDr60Status::Ok);
                assert_eq!(rr_dr60_set_volume(cp, 0.1), RrDr60Status::Ok);
            }
            let (mut r_out, mut c_out) = (Vec::new(), Vec::new());
            for (i, chunk) in x.chunks(1024).enumerate() {
                for &(at, v) in &changes {
                    if at == i {
                        rp.set_volume(v).unwrap();
                        // SAFETY: live handle.
                        assert_eq!(unsafe { rr_dr60_set_volume(cp, v) }, RrDr60Status::Ok);
                    }
                }
                let mut y = vec![0.0f32; chunk.len()];
                let n = rp.process(chunk, &mut y).unwrap().produced;
                r_out.extend_from_slice(&y[..n]);
                let mut info = RrDr60BlockInfo::default();
                // SAFETY: live handle; valid buffers.
                let status = unsafe {
                    rr_dr60_process(cp, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
                };
                assert_eq!(status, RrDr60Status::Ok);
                c_out.extend_from_slice(&y[..info.produced]);
            }
            assert_eq!(
                r_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz one-pass {route:?} with a volume transition"
            );
            // Record, then play with the changes, both sides.
            let mut rp = Pipeline::new(s).unwrap();
            rp.set_route(route);
            rp.set_volume(0.1).unwrap();
            rp.start_recording().unwrap();
            for chunk in x.chunks(480) {
                let _ = rp.record(chunk, &mut []).unwrap();
            }
            let stop = rp.stop_recording(&mut []).unwrap();
            let rr_dr60::StopResult::Kept { number, .. } = stop.result else {
                panic!("empty recording")
            };
            rp.start_playback(number).unwrap();
            let cp2 = create(&cs);
            // SAFETY: live handle.
            unsafe {
                assert_eq!(rr_dr60_set_route(cp2, c_route(route)), RrDr60Status::Ok);
                assert_eq!(rr_dr60_set_volume(cp2, 0.1), RrDr60Status::Ok);
            }
            let rec = c_record(cp2, &x, 480);
            assert!(rec.outcome.kept);
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_start_playback(cp2, rec.outcome.number) },
                RrDr60Status::Ok
            );
            let (mut r_play, mut c_play_out) = (Vec::new(), Vec::new());
            let mut buf = vec![0.0f32; 480];
            for block in 0.. {
                for &(at, v) in &changes {
                    if at == block {
                        rp.set_volume(v).unwrap();
                        // SAFETY: live handle.
                        assert_eq!(unsafe { rr_dr60_set_volume(cp2, v) }, RrDr60Status::Ok);
                    }
                }
                let info = rp.play(&mut buf).unwrap();
                r_play.extend_from_slice(&buf[..info.produced]);
                let mut ci = RrDr60PlayInfo::default();
                // SAFETY: live handle; valid buffer.
                let status = unsafe { rr_dr60_play(cp2, buf.as_mut_ptr(), buf.len(), &mut ci) };
                assert_eq!(status, RrDr60Status::Ok);
                c_play_out.extend_from_slice(&buf[..ci.produced]);
                assert_eq!(info.finished, ci.finished);
                if info.finished {
                    break;
                }
            }
            assert_eq!(
                r_play.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                c_play_out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "{rate} Hz record-then-play {route:?} with a volume transition"
            );
            // SAFETY: live handles.
            unsafe {
                rr_dr60_destroy(cp);
                rr_dr60_destroy(cp2);
            }
        }
    }
}

/// Spec 013 US3 AS1 (FR-012; tasks.md T029): the C speed functions match the Rust API: set, read
/// and refuse a level (status 25, the level unchanged), the length helper, and a playback with
/// level changes through `rr_dr60_play`, whose output and reports (the place included) are
/// bit-identical to the Rust path's.
#[test]
fn speed_parity() {
    use rr_dr60_ffi::{
        RrDr60Pipeline, RrDr60PlayInfo, RrDr60Status, rr_dr60_create, rr_dr60_destroy,
        rr_dr60_get_playback_speed, rr_dr60_play, rr_dr60_playback_length_samples, rr_dr60_record,
        rr_dr60_set_playback_speed, rr_dr60_start_playback, rr_dr60_start_recording,
        rr_dr60_stop_recording,
    };
    let rate = 48_000;
    let settings = configs::settings("speed_default", rate);
    let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x013);
    let schedule = [(0usize, 2u8), (7, 5), (15, 3), (22, 1)];
    // Rust.
    let mut p = rr_dr60::Pipeline::new(settings).unwrap();
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let rust_helper = p.playback_length_samples(1, 4).unwrap();
    let mut rust = Vec::new();
    let mut rust_infos = Vec::new();
    p.set_playback_speed(2).unwrap();
    p.start_playback(1).unwrap();
    let mut buf = vec![0.0f32; 960];
    for c in 0.. {
        for &(at, l) in &schedule {
            if at == c && c > 0 {
                p.set_playback_speed(l).unwrap();
            }
        }
        let info = p.play(&mut buf).unwrap();
        rust.extend(buf[..info.produced].iter().map(|v| v.to_bits()));
        rust_infos.push(c_play_info(info));
        if info.finished {
            break;
        }
    }
    // C.
    let cs = configs::c_settings(&settings);
    let mut h: *mut RrDr60Pipeline = std::ptr::null_mut();
    // SAFETY: valid pointers throughout; `h` is a live handle until destroyed.
    unsafe {
        assert_eq!(rr_dr60_create(&cs, &mut h), RrDr60Status::Ok);
        assert_eq!(rr_dr60_start_recording(h), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_record(
                h,
                x.as_ptr(),
                x.len(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_stop_recording(h, std::ptr::null_mut(), std::ptr::null_mut(), 0),
            RrDr60Status::Ok
        );
        let mut level = 0u8;
        assert_eq!(rr_dr60_get_playback_speed(h, &mut level), RrDr60Status::Ok);
        assert_eq!(level, 3);
        for bad in [0u8, 6] {
            assert_eq!(
                rr_dr60_set_playback_speed(h, bad),
                RrDr60Status::InvalidSpeedLevel
            );
        }
        let mut len = 0u64;
        assert_eq!(
            rr_dr60_playback_length_samples(h, 1, 9, &mut len),
            RrDr60Status::InvalidSpeedLevel
        );
        assert_eq!(
            rr_dr60_playback_length_samples(h, 1, 4, &mut len),
            RrDr60Status::Ok
        );
        assert_eq!(len, rust_helper);
        assert_eq!(rr_dr60_set_playback_speed(h, 2), RrDr60Status::Ok);
        assert_eq!(rr_dr60_get_playback_speed(h, &mut level), RrDr60Status::Ok);
        assert_eq!(level, 2);
        assert_eq!(rr_dr60_start_playback(h, 1), RrDr60Status::Ok);
        let mut c_out = Vec::new();
        let mut c_infos = Vec::new();
        for c in 0.. {
            for &(at, l) in &schedule {
                if at == c && c > 0 {
                    assert_eq!(rr_dr60_set_playback_speed(h, l), RrDr60Status::Ok);
                }
            }
            let mut info = RrDr60PlayInfo::default();
            assert_eq!(
                rr_dr60_play(h, buf.as_mut_ptr(), buf.len(), &mut info),
                RrDr60Status::Ok
            );
            c_out.extend(buf[..info.produced].iter().map(|v| v.to_bits()));
            c_infos.push(info);
            if info.finished {
                break;
            }
        }
        rr_dr60_destroy(h);
        assert_eq!(c_out, rust, "the C playback equals the Rust playback");
        assert_eq!(c_infos, rust_infos, "the C reports equal the Rust reports");
    }
}

/// Spec 014 US3 (FR-013): every field of every control, option and ID read through the C API
/// equals the Rust manifest; NULL first, then the indices; nothing is written on a refusal.
#[test]
fn control_manifest_parity_and_refusals() {
    use rr_dr60::ControlValue;
    let entries = rr_dr60::controls();
    assert_eq!(rr_dr60_control_count() as usize, entries.len());
    for (i, e) in entries.iter().enumerate() {
        let i = i as u32;
        let mut c = RrDr60Control::default();
        // SAFETY: valid out pointer.
        assert_eq!(unsafe { rr_dr60_control(i, &mut c) }, RrDr60Status::Ok);
        assert_eq!(cstr(c.identifier), e.identifier);
        assert_eq!(cstr(c.name), e.name);
        assert_eq!(cstr(c.unit), e.unit);
        assert_eq!(cstr(c.description), e.description);
        assert_eq!(c.kind, e.kind as u32);
        assert_eq!(c.timing, e.timing as u32);
        assert_eq!(c.paths, u32::from(e.paths.bits()));
        assert_eq!(c.stage, u32::from(e.stage));
        assert_eq!(c.reserved_014, 0);
        match e.value {
            ControlValue::Switch { default } => {
                assert_eq!(c.value_type, RrDr60ControlValueType::Switch as u32);
                assert_eq!(
                    (c.int_min, c.int_max, c.int_default),
                    (0, 1, u64::from(default))
                );
            }
            ControlValue::Integer { min, max, default } => {
                assert_eq!(c.value_type, RrDr60ControlValueType::Integer as u32);
                assert_eq!((c.int_min, c.int_max, c.int_default), (min, max, default));
            }
            ControlValue::Continuous { min, max, default } => {
                assert_eq!(c.value_type, RrDr60ControlValueType::Continuous as u32);
                assert_eq!(
                    (
                        c.float_min.to_bits(),
                        c.float_max.to_bits(),
                        c.float_default.to_bits()
                    ),
                    (min.to_bits(), max.to_bits(), default.to_bits())
                );
            }
            ControlValue::Choice { options, default } => {
                assert_eq!(c.value_type, RrDr60ControlValueType::Choice as u32);
                assert_eq!(c.option_count as usize, options.len());
                assert_eq!(c.has_default, u32::from(default.is_some()));
                assert_eq!(c.choice_default, default.unwrap_or(0));
                for (k, o) in options.iter().enumerate() {
                    let mut out = RrDr60ControlOption::default();
                    // SAFETY: valid out pointer.
                    let st = unsafe { rr_dr60_control_option(i, k as u32, &mut out) };
                    assert_eq!(st, RrDr60Status::Ok);
                    assert_eq!((cstr(out.identifier), out.value), (o.identifier, o.value));
                }
            }
            _ => panic!("unknown value type"),
        }
        let mut sentinel = RrDr60ControlOption::default();
        // SAFETY: valid out pointer.
        let st = unsafe { rr_dr60_control_option(i, c.option_count, &mut sentinel) };
        assert_eq!(st, RrDr60Status::NoSuchControl);
        assert!(
            sentinel.identifier.is_null(),
            "nothing written on a refusal"
        );
        assert_eq!(c.source_count as usize, e.sources.len());
        assert_eq!(c.assumption_count as usize, e.assumptions.len());
        for (k, s) in e.sources.iter().enumerate() {
            let mut id = std::ptr::null();
            // SAFETY: valid out pointer.
            assert_eq!(
                unsafe { rr_dr60_control_source(i, k as u32, &mut id) },
                RrDr60Status::Ok
            );
            assert_eq!(cstr(id), *s);
        }
        for (k, a) in e.assumptions.iter().enumerate() {
            let mut id = std::ptr::null();
            // SAFETY: valid out pointer.
            let st = unsafe { rr_dr60_control_assumption(i, k as u32, &mut id) };
            assert_eq!(st, RrDr60Status::Ok);
            assert_eq!(cstr(id), *a);
        }
        let mut id = std::ptr::null();
        // SAFETY: valid out pointer.
        let st = unsafe { rr_dr60_control_source(i, c.source_count, &mut id) };
        assert_eq!(st, RrDr60Status::NoSuchControl);
        // SAFETY: valid out pointer.
        let st = unsafe { rr_dr60_control_assumption(i, c.assumption_count, &mut id) };
        assert_eq!(st, RrDr60Status::NoSuchControl);
        assert!(id.is_null(), "nothing written on a refusal");
    }
    // Index 29 and NULL.
    let n = rr_dr60_control_count();
    let mut c = RrDr60Control::default();
    let mut o = RrDr60ControlOption::default();
    let mut id = std::ptr::null();
    // SAFETY: valid out pointers, or NULL on purpose.
    unsafe {
        assert_eq!(rr_dr60_control(n, &mut c), RrDr60Status::NoSuchControl);
        assert!(c.identifier.is_null());
        assert_eq!(
            rr_dr60_control_option(n, 0, &mut o),
            RrDr60Status::NoSuchControl
        );
        assert_eq!(
            rr_dr60_control_source(n, 0, &mut id),
            RrDr60Status::NoSuchControl
        );
        assert_eq!(
            rr_dr60_control_assumption(n, 0, &mut id),
            RrDr60Status::NoSuchControl
        );
        assert_eq!(
            rr_dr60_control(n, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_control(0, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_control_option(n, 0, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_control_source(n, 0, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_control_assumption(n, 0, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
    }
}

/// Spec 014 FR-008 (research R-03): the choices' outside values and the saturating
/// `vas_sensitivity`, through the C settings.
#[test]
fn control_manifest_c_edges() {
    let validate = |s: &RrDr60Settings| {
        let mut field = RrDr60SettingField::None;
        // SAFETY: valid pointers.
        unsafe { rr_dr60_settings_validate(s, &mut field) }
    };
    let last = |id: &str| match rr_dr60::control(id).map(|e| e.value) {
        Some(rr_dr60::ControlValue::Choice { options, .. }) => {
            options.iter().map(|o| o.value).max().unwrap()
        }
        _ => panic!("{id}"),
    };
    let mut s = rr_dr60_settings_default(48_000);
    s.tap = last("tap");
    assert_eq!(validate(&s), RrDr60Status::Ok);
    s.tap = last("tap") + 1;
    assert_eq!(validate(&s), RrDr60Status::InvalidArgument);
    let mut s = rr_dr60_settings_default(48_000);
    s.vas_mode = last("vas.mode");
    assert_eq!(validate(&s), RrDr60Status::Ok);
    s.vas_mode = last("vas.mode") + 1;
    assert_eq!(validate(&s), RrDr60Status::InvalidArgument);
    let mut s = rr_dr60_settings_default(48_000);
    for (v, want) in [
        (5, RrDr60Status::Ok),
        (6, RrDr60Status::InvalidSetting),
        (261, RrDr60Status::InvalidSetting),
    ] {
        s.vas_sensitivity = v;
        assert_eq!(validate(&s), want, "vas_sensitivity {v}");
    }
    let p = create(&rr_dr60_settings_default(48_000));
    // SAFETY: a valid handle.
    unsafe {
        assert_eq!(rr_dr60_set_route(p, last("route")), RrDr60Status::Ok);
        assert_eq!(
            rr_dr60_set_route(p, last("route") + 1),
            RrDr60Status::InvalidArgument
        );
        rr_dr60_destroy(p);
    }
}

/// Spec 016 FR-009, SC-004 (tasks.md T019): the C read-out equals the Rust read-out field for
/// field after every one-pass, record and play call (floats by bit pattern), at the
/// measurement rates.
#[test]
fn meters_parity() {
    use rr_dr60_ffi::{RrDr60Meters, rr_dr60_meters};
    let read = |h: *mut RrDr60Pipeline| -> RrDr60Meters {
        let mut m = RrDr60Meters {
            struct_size: size_of::<RrDr60Meters>() as u32,
            ..Default::default()
        };
        // SAFETY: a live handle and a valid out struct.
        assert_eq!(unsafe { rr_dr60_meters(h, &mut m) }, RrDr60Status::Ok);
        m
    };
    let same = |c: &RrDr60Meters, r: &rr_dr60::Meters| {
        c.input_peak.to_bits() == r.input_peak.to_bits()
            && c.limiter_input_peak.to_bits() == r.limiter_input_peak.to_bits()
            && c.gain_reduction_db.to_bits() == r.gain_reduction_db.to_bits()
            && c.hold_charge.to_bits() == r.hold_charge.to_bits()
            && c.record_filter_peak.to_bits() == r.record_filter_peak.to_bits()
            && c.adc_clamp_count == r.adc_clamp_count
            && c.vas_paused == u32::from(r.vas_paused)
            && c.hang_remaining_samples == r.hang_remaining_samples
            && c.reconstruction_peak.to_bits() == r.reconstruction_peak.to_bits()
            && c.route_peak.to_bits() == r.route_peak.to_bits()
            && c.output_clamp_count == r.output_clamp_count
            && c.reserved_016 == 0
    };
    for &rate in configs::measurement_rates() {
        let x = rr_dr60_harness::stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x0D60);
        let mut s = rr_dr60::Settings::new(rate);
        s.output.clamp_enabled = true;
        let cs = configs::c_settings(&s);
        let mut p = Pipeline::new(s).unwrap();
        let h = create(&cs);
        assert!(same(&read(h), &p.rest_meters()), "{rate}: the rest");
        let mut y = vec![0.0f32; 1024];
        for chunk in x.chunks(1024) {
            let _ = p.process(chunk, &mut y[..chunk.len()]).unwrap();
            let mut info = RrDr60BlockInfo::default();
            // SAFETY: a live handle and buffers of the chunk's length.
            assert_eq!(
                unsafe {
                    rr_dr60_process(h, chunk.as_ptr(), y.as_mut_ptr(), chunk.len(), &mut info)
                },
                RrDr60Status::Ok
            );
            assert!(same(&read(h), &p.meters()), "{rate}: a one-pass block");
        }
        // Record, stop, play.
        p.start_recording().unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        for chunk in x.chunks(4801) {
            let _ = p.record(chunk, &mut []).unwrap();
            let mut info = RrDr60RecordInfo::default();
            // SAFETY: a live handle, a valid buffer, no events wanted.
            assert_eq!(
                unsafe {
                    rr_dr60_record(
                        h,
                        chunk.as_ptr(),
                        chunk.len(),
                        &mut info,
                        core::ptr::null_mut(),
                        0,
                    )
                },
                RrDr60Status::Ok
            );
            assert!(same(&read(h), &p.meters()), "{rate}: a record block");
        }
        let _ = p.stop_recording(&mut []).unwrap();
        let mut out = RrDr60StopOutcome::default();
        // SAFETY: a live handle, no events wanted.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(h, &mut out, core::ptr::null_mut(), 0) },
            RrDr60Status::Ok
        );
        assert!(same(&read(h), &p.meters()), "{rate}: the flush");
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(h, 1) }, RrDr60Status::Ok);
        loop {
            let mut a = vec![0.0f32; 997];
            let info = p.play(&mut a).unwrap();
            let mut ci = RrDr60PlayInfo::default();
            // SAFETY: a live handle and a 997-sample buffer.
            assert_eq!(
                unsafe { rr_dr60_play(h, a.as_mut_ptr(), 997, &mut ci) },
                RrDr60Status::Ok
            );
            assert!(same(&read(h), &p.meters()), "{rate}: a play block");
            if info.finished {
                assert!(ci.finished);
                break;
            }
        }
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 015 US3 AS1 (FR-011): the Rust and C runs of a schedule are bit-identical in output
/// and entries, at the measurement rates, on the one-pass and the player.
#[test]
fn changes_parity() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
    let same_entry = |c: &RrDr60ChangeEntry, r: &ChangeEntry| {
        c.position == r.position
            && c.before.to_bits() == r.before.to_bits()
            && c.after.to_bits() == r.after.to_bits()
            && c.tag == r.tag
            && c.control == r.control
            && c.stream == r.stream // 020 D9; 022 D2: named `stream`
    };
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x015);
        let s = configs::settings("default_output", rate);
        let cs = configs::c_settings(&s);
        let changes = [
            Change::volume(Stream::OnePass, 700, 0.3, 1),
            Change::route(Stream::OnePass, 1500, Route::Earphone, 2),
        ];
        let mut p = Pipeline::new(s).unwrap();
        let r = cc::run_scheduled(
            &mut p,
            Source::OnePass(&x),
            &changes,
            Partition::Fixed(1024),
        );
        let h = create(&cs);
        for c in &changes {
            // SAFETY: a live handle.
            assert_eq!(
                unsafe {
                    rr_dr60_schedule_change(
                        h,
                        c.stream as u32,
                        c.position,
                        c.control,
                        c.value,
                        c.tag,
                    )
                },
                RrDr60Status::Ok
            );
        }
        let mut out = Vec::new();
        let mut log = Vec::new();
        let mut y = vec![0.0f32; 1024];
        let mut entries = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        for chunk in x.chunks(1024) {
            let mut info = RrDr60BlockInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and buffers of the chunk's length.
            assert_eq!(
                unsafe {
                    rr_dr60_process_with_events_and_changes(
                        h,
                        chunk.as_ptr(),
                        y.as_mut_ptr(),
                        chunk.len(),
                        &mut info,
                        core::ptr::null_mut(),
                        0,
                        entries.as_mut_ptr(),
                        MAX_CHANGES_PER_CALL,
                        &mut count,
                    )
                },
                RrDr60Status::Ok
            );
            out.extend_from_slice(&y[..info.produced]);
            log.extend_from_slice(&entries[..count]);
        }
        assert!(cc::same(&out, &r.out), "{rate}: the one-pass output");
        assert_eq!(log.len(), r.log.len(), "{rate}: the entry count");
        assert!(
            log.iter().zip(&r.log).all(|(c, r)| same_entry(c, r)),
            "{rate}: the entries"
        );
        // The player.
        let (mut p, n) = {
            let mut p = Pipeline::new(s).unwrap();
            p.start_recording().unwrap();
            let _ = p.record(&x, &mut []).unwrap();
            let _ = p.stop_recording(&mut []).unwrap();
            (p, 1u32)
        };
        let pchanges = [
            Change::volume(Stream::Playback, 500, 0.8, 3),
            Change::speed(900, 5, 4),
        ];
        let r = cc::run_scheduled(
            &mut p,
            Source::Playback(n),
            &pchanges,
            Partition::Fixed(1024),
        );
        // A fresh handle: the one-pass changes above moved the first handle's readings.
        // SAFETY: a live handle.
        unsafe { rr_dr60_destroy(h) };
        let h = create(&cs);
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        let mut rinfo = RrDr60RecordInfo::default();
        // SAFETY: a live handle and a valid buffer.
        assert_eq!(
            unsafe { rr_dr60_record(h, x.as_ptr(), x.len(), &mut rinfo, core::ptr::null_mut(), 0) },
            RrDr60Status::Ok
        );
        let mut so = RrDr60StopOutcome::default();
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(h, &mut so, core::ptr::null_mut(), 0) },
            RrDr60Status::Ok
        );
        for c in &pchanges {
            // SAFETY: a live handle.
            assert_eq!(
                unsafe {
                    rr_dr60_schedule_change(
                        h,
                        c.stream as u32,
                        c.position,
                        c.control,
                        c.value,
                        c.tag,
                    )
                },
                RrDr60Status::Ok
            );
        }
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(h, n) }, RrDr60Status::Ok);
        let mut out = Vec::new();
        let mut log = Vec::new();
        loop {
            let mut info = RrDr60PlayInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and valid buffers.
            assert_eq!(
                unsafe {
                    rr_dr60_play_with_changes(
                        h,
                        y.as_mut_ptr(),
                        1024,
                        &mut info,
                        entries.as_mut_ptr(),
                        MAX_CHANGES_PER_CALL,
                        &mut count,
                    )
                },
                RrDr60Status::Ok
            );
            out.extend_from_slice(&y[..info.produced]);
            log.extend_from_slice(&entries[..count]);
            if info.finished {
                break;
            }
        }
        assert!(cc::same(&out, &r.out), "{rate}: the playback output");
        assert_eq!(log.len(), r.log.len(), "{rate}: the playback entries");
        assert!(
            log.iter().zip(&r.log).all(|(c, r)| same_entry(c, r)),
            "{rate}: the playback entries"
        );
        // SAFETY: a live handle.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 017 FR-010, SC-003: the Rust and C runs of a record-side schedule on the recorder and
/// on the one-pass are bit-identical in output and entries.
#[test]
fn record_parity() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, MicSettings, Stream};
    use rr_dr60_harness::changes_checks::{self as cc, Partition};
    use rr_dr60_harness::record_checks::{self as rc, Value};
    let same_entry = |c: &RrDr60ChangeEntry, r: &ChangeEntry| {
        c.position == r.position
            && c.before.to_bits() == r.before.to_bits()
            && c.after.to_bits() == r.after.to_bits()
            && c.tag == r.tag
            && c.control == r.control
            && c.stream == r.stream // 020 D9; 022 D2: named `stream`
    };
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x017);
        let mut s = configs::settings("default_output", rate);
        s.mic = MicSettings::DEVICE;
        s.speech_codec_enabled = false;
        let cs = configs::c_settings(&s);
        let changes = [
            rc::change(Stream::Record, 700, Value::Sensitivity(5), 1),
            rc::change(Stream::Record, 1500, Value::Hiss(6.0), 2),
        ];
        // Rust: the recorder.
        let mut p = Pipeline::new(s).unwrap();
        let r = rc::record_run(&mut p, &x, Partition::Fixed(1024), &changes);
        // C: the recorder.
        let h = create(&cs);
        for c in &changes {
            // SAFETY: a live handle.
            assert_eq!(
                unsafe {
                    rr_dr60_schedule_change(
                        h,
                        c.stream as u32,
                        c.position,
                        c.control,
                        c.value,
                        c.tag,
                    )
                },
                RrDr60Status::Ok
            );
        }
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        let mut log = Vec::new();
        let mut entries = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        for chunk in x.chunks(1024) {
            let mut info = RrDr60RecordInfo::default();
            let (mut count, mut total) = (0usize, 0u64);
            // SAFETY: a live handle and valid buffers.
            assert_eq!(
                unsafe {
                    rr_dr60_record_with_changes(
                        h,
                        chunk.as_ptr(),
                        chunk.len(),
                        &mut info,
                        core::ptr::null_mut(),
                        0,
                        entries.as_mut_ptr(),
                        MAX_CHANGES_PER_CALL,
                        &mut count,
                        &mut total,
                    )
                },
                RrDr60Status::Ok
            );
            log.extend_from_slice(&entries[..count]);
        }
        let mut so = RrDr60StopOutcome::default();
        let (mut count, mut total) = (0usize, 0u64);
        // SAFETY: a live handle and valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_stop_recording_with_changes(
                    h,
                    &mut so,
                    core::ptr::null_mut(),
                    0,
                    entries.as_mut_ptr(),
                    MAX_CHANGES_PER_CALL,
                    &mut count,
                    &mut total,
                )
            },
            RrDr60Status::Ok
        );
        log.extend_from_slice(&entries[..count]);
        assert_eq!(
            total, r.stop.input_samples_total,
            "{rate}: the final position"
        );
        assert_eq!(log.len(), r.log.len(), "{rate}: the entry count");
        assert!(
            log.iter().zip(&r.log).all(|(c, r)| same_entry(c, r)),
            "{rate}: the entries"
        );
        // The recordings: exported bytes.
        let mut len = 0usize;
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_export_len(h, 1, &mut len) },
            RrDr60Status::Ok
        );
        let mut bytes = vec![0u8; len];
        let mut written = 0usize;
        // SAFETY: a live handle and a buffer of `len` bytes.
        assert_eq!(
            unsafe { rr_dr60_export(h, 1, bytes.as_mut_ptr(), len, &mut written) },
            RrDr60Status::Ok
        );
        let rust_bytes = split_checks::export_bytes(&p, r.number);
        assert_eq!(bytes, rust_bytes, "{rate}: the recording");
        // The one-pass on a fresh handle and engine.
        // SAFETY: a live handle.
        unsafe { rr_dr60_destroy(h) };
        let h = create(&cs);
        let one = [
            rc::change(Stream::OnePass, 700, Value::Knee(-5.0), 3),
            cc::Change::route(Stream::OnePass, 900, Route::Earphone, 4),
        ];
        let mut q = Pipeline::new(s).unwrap();
        let rr = cc::run_scheduled(
            &mut q,
            cc::Source::OnePass(&x),
            &one,
            Partition::Fixed(1024),
        );
        for c in &one {
            // SAFETY: a live handle.
            assert_eq!(
                unsafe {
                    rr_dr60_schedule_change(
                        h,
                        c.stream as u32,
                        c.position,
                        c.control,
                        c.value,
                        c.tag,
                    )
                },
                RrDr60Status::Ok
            );
        }
        let mut out = Vec::new();
        let mut log = Vec::new();
        let mut y = vec![0.0f32; 1024];
        for chunk in x.chunks(1024) {
            let mut info = RrDr60BlockInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and buffers of the chunk's length.
            assert_eq!(
                unsafe {
                    rr_dr60_process_with_events_and_changes(
                        h,
                        chunk.as_ptr(),
                        y.as_mut_ptr(),
                        chunk.len(),
                        &mut info,
                        core::ptr::null_mut(),
                        0,
                        entries.as_mut_ptr(),
                        MAX_CHANGES_PER_CALL,
                        &mut count,
                    )
                },
                RrDr60Status::Ok
            );
            out.extend_from_slice(&y[..info.produced]);
            log.extend_from_slice(&entries[..count]);
        }
        assert!(cc::same(&out, &rr.out), "{rate}: the one-pass output");
        assert_eq!(log.len(), rr.log.len(), "{rate}: the one-pass entry count");
        assert!(
            log.iter().zip(&rr.log).all(|(c, r)| same_entry(c, r)),
            "{rate}: the one-pass entries"
        );
        let mut pos = 0u64;
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_input_position(h, 1, &mut pos) },
            RrDr60Status::Ok
        );
        assert_eq!(pos, q.input_position(Stream::OnePass).unwrap());
        // SAFETY: a live handle.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 018 US2 AS3 (FR-011): the battery setter and getter, a scheduled change, the meters' two
/// fields and the published law agree between the Rust and C paths, bit for bit, at the
/// measurement rates.
#[test]
fn battery_parity() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, RrDr60Meters, rr_dr60_battery_ceiling_db,
        rr_dr60_battery_ceiling_factor, rr_dr60_battery_trajectory_row, rr_dr60_get_battery_volts,
        rr_dr60_meters, rr_dr60_schedule_change, rr_dr60_set_battery_volts, rr_dr60_set_volume,
    };
    use rr_dr60_harness::battery_checks as bat;
    use rr_dr60_harness::changes_checks::{self as cc, Change};
    let read = |h: *mut RrDr60Pipeline| -> RrDr60Meters {
        let mut m = RrDr60Meters {
            struct_size: size_of::<RrDr60Meters>() as u32,
            ..Default::default()
        };
        // SAFETY: a live handle and a valid out struct.
        assert_eq!(unsafe { rr_dr60_meters(h, &mut m) }, RrDr60Status::Ok);
        m
    };
    // The published law, no handle.
    for v in [3.0f32, 2.96, 2.8, 2.6, 2.45, 2.4, 2.22, 2.2] {
        let (mut f, mut d) = (0.0f32, 0.0f32);
        // SAFETY: writable outs.
        assert_eq!(
            unsafe { rr_dr60_battery_ceiling_factor(v, &mut f) },
            RrDr60Status::Ok
        );
        // SAFETY: writable out.
        assert_eq!(
            unsafe { rr_dr60_battery_ceiling_db(v, &mut d) },
            RrDr60Status::Ok
        );
        assert_eq!(
            f.to_bits(),
            rr_dr60::battery::ceiling_factor(v).to_bits(),
            "{v}"
        );
        assert_eq!(
            d.to_bits(),
            rr_dr60::battery::ceiling_db(v).to_bits(),
            "{v}"
        );
    }
    for (i, &(hours, volts)) in rr_dr60::battery::ALKALINE_TRAJECTORY.iter().enumerate() {
        let (mut h, mut v) = (0.0f32, 0.0f32);
        // SAFETY: writable outs.
        assert_eq!(
            unsafe { rr_dr60_battery_trajectory_row(i as u32, &mut h, &mut v) },
            RrDr60Status::Ok
        );
        assert_eq!(
            (h.to_bits(), v.to_bits()),
            (hours.to_bits(), volts.to_bits())
        );
    }
    for &rate in configs::measurement_rates() {
        let x = bat::clipped_tone(rate);
        let s = bat::stage11_only(rate);
        let cs = configs::c_settings(&s);
        let volume = rr_dr60::output::VOLUME_6DB_OVER;
        let change = Change::battery(Stream::OnePass, 1500, 2.2, 4);
        let mut p = Pipeline::new(s).unwrap();
        p.set_volume(volume).unwrap();
        p.set_battery_volts(2.4).unwrap();
        let h = create(&cs);
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_set_volume(h, volume) }, RrDr60Status::Ok);
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_set_battery_volts(h, 2.4) },
            RrDr60Status::Ok
        );
        let mut got = 0.0f32;
        // SAFETY: a live handle and a writable out.
        assert_eq!(
            unsafe { rr_dr60_get_battery_volts(h, &mut got) },
            RrDr60Status::Ok
        );
        assert_eq!(
            got.to_bits(),
            p.battery_volts().to_bits(),
            "{rate}: the getter"
        );
        // Before any call both sides hold the construction-time rest read-out (the setter does
        // not rewrite the stored read-out; `rest_meters()` follows the value).
        let rest = read(h);
        assert_eq!(
            rest.battery_ceiling.to_bits(),
            p.meters().battery_ceiling.to_bits(),
            "{rate}: the stored read-out"
        );
        assert_eq!(
            p.rest_meters().battery_ceiling.to_bits(),
            rr_dr60::battery::ceiling_factor(2.4).to_bits(),
            "{rate}: the rest read-out follows the value"
        );
        p.schedule_change(
            change.stream,
            change.position,
            change.control,
            change.value,
            change.tag,
        )
        .unwrap();
        // SAFETY: a live handle.
        assert_eq!(
            unsafe {
                rr_dr60_schedule_change(
                    h,
                    change.stream as u32,
                    change.position,
                    change.control,
                    change.value,
                    change.tag,
                )
            },
            RrDr60Status::Ok
        );
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let mut entries = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let mut y = vec![0.0f32; 1024];
        let mut yc = vec![0.0f32; 1024];
        for chunk in x.chunks(1024) {
            let info = p
                .process_with_events_and_changes(chunk, &mut y[..chunk.len()], &mut [], &mut log)
                .unwrap();
            let mut cinfo = RrDr60BlockInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and buffers of the chunk's length.
            assert_eq!(
                unsafe {
                    rr_dr60_process_with_events_and_changes(
                        h,
                        chunk.as_ptr(),
                        yc.as_mut_ptr(),
                        chunk.len(),
                        &mut cinfo,
                        core::ptr::null_mut(),
                        0,
                        entries.as_mut_ptr(),
                        MAX_CHANGES_PER_CALL,
                        &mut count,
                    )
                },
                RrDr60Status::Ok
            );
            assert_eq!(cinfo.produced, info.produced);
            assert!(
                cc::same(&y[..info.produced], &yc[..info.produced]),
                "{rate}: the output"
            );
            assert_eq!(count, info.changes, "{rate}: the entry count");
            for (c, r) in entries[..count].iter().zip(&log[..info.changes]) {
                assert_eq!(
                    (
                        c.position,
                        c.control,
                        c.before.to_bits(),
                        c.after.to_bits(),
                        c.tag
                    ),
                    (
                        r.position,
                        r.control,
                        r.before.to_bits(),
                        r.after.to_bits(),
                        r.tag
                    ),
                    "{rate}: an entry"
                );
            }
            let (m, r) = (read(h), p.meters());
            assert_eq!(m.amp_clip_count, r.amp_clip_count, "{rate}: the clip count");
            assert_eq!(
                m.battery_ceiling.to_bits(),
                r.battery_ceiling.to_bits(),
                "{rate}: the ceiling factor"
            );
            assert_eq!(
                m.route_peak.to_bits(),
                r.route_peak.to_bits(),
                "{rate}: the route peak"
            );
        }
        // SAFETY: a live handle and a writable out.
        assert_eq!(
            unsafe { rr_dr60_get_battery_volts(h, &mut got) },
            RrDr60Status::Ok
        );
        assert_eq!(got, 2.2, "{rate}: the landed value reads back");
        assert_eq!(p.battery_volts(), 2.2);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 019 US2 AS4 (FR-009): a playback with a level change and a second one inside the first's
/// 20 ms blend (waited) is bit-identical between the Rust and C paths, at 6 and 48 kHz.
#[test]
fn speed_crossfade_parity() {
    use rr_dr60_ffi::{
        RrDr60Pipeline, RrDr60PlayInfo, RrDr60Status, rr_dr60_create, rr_dr60_destroy,
        rr_dr60_play, rr_dr60_record, rr_dr60_set_playback_speed, rr_dr60_start_playback,
        rr_dr60_start_recording, rr_dr60_stop_recording,
    };
    for rate in [6000u32, 48_000] {
        let settings = configs::settings("speed_default", rate);
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x019);
        let call = (rate / 100) as usize; // 10 ms calls: calls 7 and 8 are inside one blend
        let schedule = [(7usize, 5u8), (8, 1), (30, 3)];
        let mut p = rr_dr60::Pipeline::new(settings).unwrap();
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        let mut rust = Vec::new();
        p.start_playback(1).unwrap();
        let mut buf = vec![0.0f32; call];
        for c in 0.. {
            for &(at, l) in &schedule {
                if at == c {
                    p.set_playback_speed(l).unwrap();
                }
            }
            let info = p.play(&mut buf).unwrap();
            rust.extend(buf[..info.produced].iter().map(|v| v.to_bits()));
            if info.finished {
                break;
            }
        }
        let cs = configs::c_settings(&settings);
        let mut h: *mut RrDr60Pipeline = std::ptr::null_mut();
        let mut c_out = Vec::new();
        // SAFETY: valid pointers throughout; `h` is a live handle until destroyed.
        unsafe {
            assert_eq!(rr_dr60_create(&cs, &mut h), RrDr60Status::Ok);
            assert_eq!(rr_dr60_start_recording(h), RrDr60Status::Ok);
            assert_eq!(
                rr_dr60_record(
                    h,
                    x.as_ptr(),
                    x.len(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0
                ),
                RrDr60Status::Ok
            );
            assert_eq!(
                rr_dr60_stop_recording(h, std::ptr::null_mut(), std::ptr::null_mut(), 0),
                RrDr60Status::Ok
            );
            assert_eq!(rr_dr60_start_playback(h, 1), RrDr60Status::Ok);
            let mut cbuf = vec![0.0f32; call];
            for c in 0.. {
                for &(at, l) in &schedule {
                    if at == c {
                        assert_eq!(rr_dr60_set_playback_speed(h, l), RrDr60Status::Ok);
                    }
                }
                let mut info = RrDr60PlayInfo::default();
                assert_eq!(
                    rr_dr60_play(h, cbuf.as_mut_ptr(), call, &mut info),
                    RrDr60Status::Ok
                );
                c_out.extend(cbuf[..info.produced].iter().map(|v| v.to_bits()));
                if info.finished {
                    break;
                }
            }
            rr_dr60_destroy(h);
        }
        assert_eq!(rust, c_out, "{rate}: the crossfade through C");
    }
}

/// Spec 020 FR-009 (tasks.md T020): the C writer, reader, digest, counter constructor, maps and
/// feed-point helper equal the Rust path.
#[test]
fn record_contract_parity() {
    use rr_dr60::record::{self, RecordReadout};
    use rr_dr60::timeline::{self, KeptOrRemoved};
    use rr_dr60::{ChangeEntry, VasEvent};
    use rr_dr60_ffi::{
        RrDr60InputAlignment, RrDr60PlaceState, RrDr60RecordReadout, RrDr60RunData,
        RrDr60StopFields, rr_dr60_create_at_counter, rr_dr60_digest, rr_dr60_export,
        rr_dr60_export_len, rr_dr60_input_to_kept, rr_dr60_kept_to_input,
        rr_dr60_onepass_output_to_input, rr_dr60_place_at, rr_dr60_processing_record_len,
        rr_dr60_processing_record_read, rr_dr60_processing_record_rewrite,
        rr_dr60_processing_record_write, rr_dr60_record, rr_dr60_record_change,
        rr_dr60_record_changes, rr_dr60_record_event, rr_dr60_record_events,
        rr_dr60_recording_counter_of, rr_dr60_recording_digest, rr_dr60_recording_digest_matches,
        rr_dr60_samples_from_i24, rr_dr60_start_recording, rr_dr60_stop_recording,
    };
    use rr_dr60_harness::checks::standard;
    use rr_dr60_harness::record_contract_checks::{self as rc, Cell};
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let (p, run) = rc::record_run(&standard, rate, cell, &raw);
        let tag = format!("{rate} Hz");
        // The C writer on the same engine state: rebuild it through the C API by recording
        // the same input with the same schedule, then compare the records byte for byte.
        let s = rc::settings(cell, rate);
        let h = create(&configs::c_settings(&s));
        for (pos, control, value, t) in rc::cell_changes(rate) {
            // SAFETY: a live handle.
            assert_eq!(
                unsafe { rr_dr60_schedule_change(h, 2, pos, control, value, t) },
                RrDr60Status::Ok
            );
        }
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h) }, RrDr60Status::Ok);
        let x = rc::samples(&raw);
        let mut c_events: Vec<RrDr60VasEvent> = Vec::new();
        let mut c_changes: Vec<RrDr60ChangeEntry> = Vec::new();
        let mut ev = vec![RrDr60VasEvent::default(); 64];
        let mut ch = vec![RrDr60ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let mut total = 0u64;
        for block in x.chunks(1024) {
            let mut info = RrDr60RecordInfo::default();
            let mut count = 0usize;
            // SAFETY: valid buffers.
            let st = unsafe {
                rr_dr60_record_with_changes(
                    h,
                    block.as_ptr(),
                    block.len(),
                    &mut info,
                    ev.as_mut_ptr(),
                    64,
                    ch.as_mut_ptr(),
                    ch.len(),
                    &mut count,
                    &mut total,
                )
            };
            assert_eq!(st, RrDr60Status::Ok);
            c_events.extend_from_slice(&ev[..info.events]);
            c_changes.extend_from_slice(&ch[..count]);
        }
        let mut so = RrDr60StopOutcome::default();
        let mut count = 0usize;
        // SAFETY: valid buffers.
        let st = unsafe {
            rr_dr60_stop_recording_with_changes(
                h,
                &mut so,
                ev.as_mut_ptr(),
                64,
                ch.as_mut_ptr(),
                ch.len(),
                &mut count,
                &mut total,
            )
        };
        assert_eq!(st, RrDr60Status::Ok);
        c_events.extend_from_slice(&ev[..so.flush_events]);
        c_changes.extend_from_slice(&ch[..count]);
        assert_eq!(c_events.len(), run.events.len(), "{tag}: the events");
        assert_eq!(c_changes.len(), run.changes.len(), "{tag}: the entries");
        let c_run = RrDr60RunData {
            struct_size: size_of::<RrDr60RunData>() as u32,
            reserved: 0,
            events: c_events.as_ptr(),
            event_count: c_events.len(),
            changes: c_changes.as_ptr(),
            change_count: c_changes.len(),
            stop: RrDr60StopFields {
                struct_size: size_of::<RrDr60StopFields>() as u32,
                how: 0,
                stop_input_position: so.stop_input_position,
                padding_start_sample: so.padding_start_sample,
                input_samples_total: total,
                silence_fed_samples: so.silence_fed_samples,
            },
        };
        let mut len = 0usize;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_processing_record_len(h, so.number, &c_run, &mut len) },
            RrDr60Status::Ok
        );
        assert_eq!(len, run.record.len(), "{tag}: the length");
        let mut c_rec = vec![0u8; len];
        let mut written = 0usize;
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_processing_record_write(
                    h,
                    so.number,
                    &c_run,
                    c_rec.as_mut_ptr(),
                    len,
                    &mut written,
                )
            },
            RrDr60Status::Ok
        );
        assert_eq!(c_rec, run.record, "{tag}: the record bytes");
        // The digests.
        let mut d = 0u64;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_recording_digest(h, so.number, &mut d) },
            RrDr60Status::Ok
        );
        assert_eq!(d, run.digest);
        // SAFETY: a valid buffer.
        assert_eq!(
            unsafe { rr_dr60_digest(run.export.as_ptr(), run.export.len()) },
            run.digest
        );
        let mut elen = 0usize;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_export_len(h, so.number, &mut elen) },
            RrDr60Status::Ok
        );
        let mut exp = vec![0u8; elen];
        let mut ew = 0usize;
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe { rr_dr60_export(h, so.number, exp.as_mut_ptr(), elen, &mut ew) },
            RrDr60Status::Ok
        );
        assert_eq!(exp, run.export);
        // The reader and the read-out's fields.
        let mut cr = RrDr60RecordReadout {
            struct_size: size_of::<RrDr60RecordReadout>() as u32,
            ..unsafe { core::mem::zeroed() }
        };
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe { rr_dr60_processing_record_read(h, c_rec.as_ptr(), len, &mut cr) },
            RrDr60Status::Ok
        );
        let r = rc::readout(&run.record);
        assert_eq!(
            (
                cr.flags,
                cr.event_count,
                cr.change_count,
                cr.recording_counter,
                cr.seed
            ),
            (
                r.flags,
                r.event_count,
                r.change_count,
                r.recording_counter,
                r.seed
            ),
            "{tag}"
        );
        assert_eq!(cr.recording_digest, r.recording_digest);
        assert_eq!(cr.stop.stop_input_position, r.stop.stop_input_position);
        assert_eq!(
            cr.settings.agc_knee_dbfs.to_bits(),
            r.settings.agc.knee_dbfs.to_bits()
        );
        let mut matches = false;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_recording_digest_matches(h, so.number, &mut cr, &mut matches) },
            RrDr60Status::Ok
        );
        assert!(matches);
        // NULL pipeline: the pure read, without the host-rate flag.
        let mut pure = RrDr60RecordReadout {
            struct_size: size_of::<RrDr60RecordReadout>() as u32,
            ..unsafe { core::mem::zeroed() }
        };
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_processing_record_read(std::ptr::null_mut(), c_rec.as_ptr(), len, &mut pure)
            },
            RrDr60Status::Ok
        );
        assert_eq!(pure.flags, 0);
        // The rewrite and the cursors.
        let mut again = vec![0u8; len + 1];
        let mut aw = 0usize;
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_processing_record_rewrite(
                    c_rec.as_ptr(),
                    len,
                    again.as_mut_ptr(),
                    len + 1,
                    &mut aw,
                )
            },
            RrDr60Status::Ok
        );
        assert_eq!(&again[..aw], &run.record[..]);
        let mut e = RrDr60VasEvent::default();
        let mut hp = 0u64;
        // SAFETY: valid pointers.
        assert!(unsafe { rr_dr60_record_event(c_rec.as_ptr(), len, 0, &mut e, &mut hp) });
        let want = record::record_event(&run.record, 0).unwrap();
        assert_eq!(
            (e.output_position, e.input_length, hp),
            (
                want.event.output_position,
                want.event.input_length,
                want.host_input_position
            )
        );
        let mut c = RrDr60ChangeEntry::default();
        // SAFETY: valid pointers.
        assert!(unsafe { rr_dr60_record_change(c_rec.as_ptr(), len, 0, &mut c) });
        assert_eq!(
            (c.position, c.control, c.stream),
            (
                run.changes[0].position,
                run.changes[0].control,
                run.changes[0].stream
            )
        );
        let mut evs = vec![RrDr60VasEvent::default(); 64];
        let mut chs = vec![RrDr60ChangeEntry::default(); 8];
        // SAFETY: valid buffers.
        let ne = unsafe { rr_dr60_record_events(c_rec.as_ptr(), len, 0, evs.as_mut_ptr(), 64) };
        // SAFETY: valid buffers.
        let nc = unsafe { rr_dr60_record_changes(c_rec.as_ptr(), len, 0, chs.as_mut_ptr(), 8) };
        assert_eq!((ne, nc), (run.events.len(), run.changes.len()));
        // The maps.
        let cs = configs::c_settings(&r.settings);
        for k in [0u64, 100, 1000] {
            let mut a = RrDr60InputAlignment {
                struct_size: size_of::<RrDr60InputAlignment>() as u32,
                ..Default::default()
            };
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe {
                    rr_dr60_kept_to_input(
                        &cs,
                        k,
                        c_events.as_ptr(),
                        c_events.len(),
                        &c_run.stop,
                        &mut a,
                    )
                },
                RrDr60Status::Ok
            );
            let want = timeline::kept_to_input(k, &r.settings, &run.events, &run.stop).unwrap();
            assert_eq!(
                (a.centre, a.first, a.last, a.clamped != 0),
                (want.centre, want.first, want.last, want.clamped),
                "{tag}: k {k}"
            );
        }
        for pos in [0u64, 10_000, u64::from(rate)] {
            let (mut kept, mut removed) = (0u64, 0u64);
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe {
                    rr_dr60_input_to_kept(
                        &cs,
                        pos,
                        c_events.as_ptr(),
                        c_events.len(),
                        &mut kept,
                        &mut removed,
                    )
                },
                RrDr60Status::Ok
            );
            match timeline::input_to_kept(pos, &r.settings, &run.events).unwrap() {
                KeptOrRemoved::Kept(k) => assert_eq!((kept, removed), (k, u64::MAX)),
                KeptOrRemoved::Removed { event_index } => {
                    assert_eq!((kept, removed), (0, event_index))
                }
                KeptOrRemoved::Muted {
                    kept: k,
                    event_index,
                } => {
                    assert_eq!((kept, removed), (k, event_index))
                }
            }
        }
        let (mut input, mut clamped) = (0u64, false);
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe {
                rr_dr60_onepass_output_to_input(
                    5000,
                    483,
                    c_events.as_ptr(),
                    c_events.len(),
                    true,
                    &mut input,
                    &mut clamped,
                )
            },
            RrDr60Status::Ok
        );
        assert_eq!(
            (input, clamped),
            timeline::onepass_output_to_input(5000, 483, &run.events, true)
        );
        let entries = [ChangeEntry::new(
            rc::host_position_for(100, rate),
            2,
            3.0,
            5.0,
            1,
            1,
        )];
        let c_entries: Vec<RrDr60ChangeEntry> = entries
            .iter()
            .map(|e| RrDr60ChangeEntry {
                position: e.position,
                before: e.before,
                after: e.after,
                tag: e.tag,
                control: e.control,
                stream: e.stream,
            })
            .collect();
        for i in [0u64, 100, 150, 3000] {
            let mut ps = RrDr60PlaceState {
                struct_size: size_of::<RrDr60PlaceState>() as u32,
                ..Default::default()
            };
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe { rr_dr60_place_at(i, rate, 3, 3000, c_entries.as_ptr(), 1, &mut ps) },
                RrDr60Status::Ok
            );
            let want = timeline::place_at(i, rate, 3, 3000, &entries).unwrap();
            assert_eq!(
                (
                    ps.place_fifths,
                    ps.level as u8,
                    ps.fading != 0,
                    ps.old_place_fifths,
                    ps.latched != 0
                ),
                (
                    want.place_fifths,
                    want.level,
                    want.fading,
                    want.old_place_fifths,
                    want.latched
                ),
                "{tag}: output {i}"
            );
        }
        // The feed-point helper and the counter constructor.
        let codes = [-8_388_608i32, 7, 8_388_607];
        let (mut c_out, mut r_out) = ([0.0f32; 3], [0.0f32; 3]);
        // SAFETY: valid buffers.
        unsafe { rr_dr60_samples_from_i24(codes.as_ptr(), c_out.as_mut_ptr(), 3) };
        rr_dr60::input::samples_from_i24(&codes, &mut r_out);
        assert_eq!(c_out.map(f32::to_bits), r_out.map(f32::to_bits));
        let (mut known, mut counter) = (false, 0u64);
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_recording_counter_of(h, so.number, &mut known, &mut counter) },
            RrDr60Status::Ok
        );
        assert!(known && counter == run.counter);
        let mut h2: *mut RrDr60Pipeline = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_create_at_counter(&cs, 5, &mut h2) },
            RrDr60Status::Ok
        );
        let mut q = rr_dr60::Pipeline::new_at_counter(r.settings, 5).unwrap();
        q.start_recording().unwrap();
        let _ = q.record(&x[..rate as usize / 2], &mut []).unwrap();
        let _ = q.stop_recording(&mut []).unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(h2) }, RrDr60Status::Ok);
        let mut info = RrDr60RecordInfo::default();
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_record(
                    h2,
                    x.as_ptr(),
                    rate as usize / 2,
                    &mut info,
                    std::ptr::null_mut(),
                    0,
                )
            },
            RrDr60Status::Ok
        );
        let mut so2 = RrDr60StopOutcome::default();
        // SAFETY: valid buffers.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(h2, &mut so2, std::ptr::null_mut(), 0) },
            RrDr60Status::Ok
        );
        let mut d2 = 0u64;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_recording_digest(h2, 1, &mut d2) },
            RrDr60Status::Ok
        );
        assert_eq!(
            d2,
            q.recording_digest(1).unwrap(),
            "{tag}: the counter constructor"
        );
        let _ = (p, RecordReadout::default(), VasEvent::default());
        // SAFETY: live handles.
        unsafe {
            rr_dr60_destroy(h);
            rr_dr60_destroy(h2);
        }
    }
}

/// Spec 021 (tasks.md T019): the four switches through `rr_dr60_schedule_change` land and blend
/// bit-identically to the Rust path; the refusals' statuses; the level function's four cells;
/// status 36 and the bound.
#[test]
fn bypass_parity() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, Route, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, RrDr60PlayInfo, rr_dr60_import, rr_dr60_play_with_changes,
        rr_dr60_schedule_change, rr_dr60_set_route, rr_dr60_set_volume,
        rr_dr60_stage11_bypass_level_db, rr_dr60_start_playback,
    };
    use rr_dr60_harness::bypass_checks as bp;
    assert_eq!(RrDr60Status::StageNotConfigured as u32, 36);
    assert_eq!(rr_dr60_ffi::RR_DR60_MAX_CHANGES_PER_CALL, 142);
    assert_eq!(MAX_CHANGES_PER_CALL, 142);
    // The level function, no handle.
    for (route, c_route, pos) in [
        (Route::Earphone, 1u32, rr_dr60::output::VOLUME_REFERENCE),
        (Route::Earphone, 1, 1.0f32),
        (Route::Speaker, 0, rr_dr60::output::VOLUME_REFERENCE),
        (Route::Speaker, 0, 1.0),
    ] {
        let got = rr_dr60_stage11_bypass_level_db(c_route, pos);
        assert_eq!(
            got.to_bits(),
            rr_dr60::output::stage11_bypass_level_db(route, pos).to_bits()
        );
    }
    for &rate in configs::measurement_rates() {
        let s = bp::settings(bp::DEFAULT_CHAIN, rate);
        let cs = configs::c_settings(&s);
        let stored = bp::stored_tone(1000.0, 0.8);
        let mut p = bp::engine(
            &|s| Pipeline::new(s).unwrap(),
            s,
            &stored,
            Route::Speaker,
            0.5,
        );
        let mut bytes = vec![0u8; p.export_len(1).unwrap()];
        let _ = p.export(1, &mut bytes).unwrap();
        let h = create(&cs);
        let mut imported = 0u32;
        // SAFETY: a live handle and the exported bytes.
        assert_eq!(
            unsafe { rr_dr60_import(h, bytes.as_ptr(), bytes.len(), &mut imported) },
            RrDr60Status::Ok
        );
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_set_route(h, 0) }, RrDr60Status::Ok);
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_set_volume(h, 0.5) }, RrDr60Status::Ok);
        let n = bp::n_fade(12, rate) as u64;
        let sched = [
            (200u64, 23u32, 0.0f64, 1u64),
            (200 + n / 2, 23, 1.0, 2),
            (200 + 2 * n, 5, 0.0, 3),
            (200 + 3 * n, 22, 0.0, 4),
        ];
        for (pos, c, v, tag) in sched {
            p.schedule_change(Stream::Playback, pos, c, v, tag).unwrap();
            // SAFETY: a live handle.
            assert_eq!(
                unsafe { rr_dr60_schedule_change(h, 0, pos, c, v, tag) },
                RrDr60Status::Ok,
                "{rate} Hz"
            );
        }
        // The refusals' statuses.
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_schedule_change(h, 1, 0, 5, 0.0, 9) },
            RrDr60Status::ControlNotOnStream
        );
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_schedule_change(h, 0, 0, 23, 0.5, 9) },
            RrDr60Status::InvalidArgument
        ); // InvalidControlValue maps to InvalidArgument
        {
            let mut off = s;
            off.output.speaker_enabled = false;
            let g = create(&configs::c_settings(&off));
            // SAFETY: a live handle.
            assert_eq!(
                unsafe { rr_dr60_schedule_change(g, 0, 0, 23, 0.0, 9) },
                RrDr60Status::StageNotConfigured
            );
            // SAFETY: destroying the handle once.
            unsafe { rr_dr60_destroy(g) };
        }
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(h, 1) }, RrDr60Status::Ok);
        let mut rust = Vec::new();
        let mut c_out = Vec::new();
        let mut rust_log = Vec::new();
        let mut c_log = Vec::new();
        let mut y = vec![0.0f32; 512];
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let mut clog = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        loop {
            let info = p.play_with_changes(&mut y, &mut log).unwrap();
            rust.extend_from_slice(&y[..info.produced]);
            rust_log.extend_from_slice(&log[..info.changes]);
            let mut ci = RrDr60PlayInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and valid buffers.
            let st = unsafe {
                rr_dr60_play_with_changes(
                    h,
                    y.as_mut_ptr(),
                    512,
                    &mut ci,
                    clog.as_mut_ptr(),
                    MAX_CHANGES_PER_CALL,
                    &mut count,
                )
            };
            assert_eq!(st, RrDr60Status::Ok);
            assert_eq!(ci.produced, info.produced);
            c_out.extend_from_slice(&y[..ci.produced]);
            c_log.extend_from_slice(&clog[..count]);
            if info.finished {
                assert!(ci.finished);
                break;
            }
        }
        assert_eq!(rust.len(), c_out.len(), "{rate} Hz");
        assert!(
            rust.iter()
                .zip(&c_out)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{rate} Hz: the C path"
        );
        assert_eq!(rust_log.len(), c_log.len());
        for (r, c) in rust_log.iter().zip(&c_log) {
            assert_eq!(
                (
                    r.position,
                    r.control,
                    r.before.to_bits(),
                    r.after.to_bits(),
                    r.tag
                ),
                (
                    c.position,
                    c.control,
                    c.before.to_bits(),
                    c.after.to_bits(),
                    c.tag
                )
            );
        }
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 022 (tasks.md T009): the pre-1.0 C ABI pass. The rows of `abi_checks::rows_at` built
/// through the C API agree with the Rust path at the measurement rates (the eleven structs and
/// the meters field for field, the read-back size included); the four cases per out-struct on a
/// painted buffer; the ABI check and the settings rule (D13); a refused per-call report
/// processes nothing; the `stream` name on the frozen entry.
#[test]
fn abi_pass_parity() {
    use rr_dr60_harness::abi_checks as ab;
    use std::fmt::Write as _;
    assert_eq!(RrDr60Status::AbiMismatch as u32, 37);
    assert_eq!(RR_DR60_ABI_VERSION, 1);
    assert_eq!(RR_DR60_MAX_STRUCT_SIZE, 4096);
    assert_eq!(RR_DR60_VAS_EVENT_SIZE as usize, size_of::<RrDr60VasEvent>());
    assert_eq!(
        RR_DR60_CHANGE_ENTRY_SIZE as usize,
        size_of::<RrDr60ChangeEntry>()
    );
    assert_eq!(rr_dr60_abi_version(), 1);
    for (name, size) in ab::SIZES {
        let got = match name {
            "RrDr60BlockInfo" => size_of::<RrDr60BlockInfo>(),
            "RrDr60RecordInfo" => size_of::<RrDr60RecordInfo>(),
            "RrDr60StopOutcome" => size_of::<RrDr60StopOutcome>(),
            "RrDr60PlayInfo" => size_of::<RrDr60PlayInfo>(),
            "RrDr60EngineState" => size_of::<RrDr60EngineState>(),
            "RrDr60MemoryStatus" => size_of::<RrDr60MemoryStatus>(),
            "RrDr60SpeechCodecInfo" => size_of::<RrDr60SpeechCodecInfo>(),
            "RrDr60ManifestHeader" => size_of::<RrDr60ManifestHeader>(),
            "RrDr60StageEntry" => size_of::<RrDr60StageEntry>(),
            "RrDr60Control" => size_of::<RrDr60Control>(),
            "RrDr60ControlOption" => size_of::<RrDr60ControlOption>(),
            "RrDr60Meters" => size_of::<RrDr60Meters>(),
            "RrDr60RecordReadout" => size_of::<RrDr60RecordReadout>(),
            "RrDr60InputAlignment" => size_of::<RrDr60InputAlignment>(),
            "RrDr60PlaceState" => size_of::<RrDr60PlaceState>(),
            "RrDr60VasEvent" => size_of::<RrDr60VasEvent>(),
            "RrDr60ChangeEntry" => size_of::<RrDr60ChangeEntry>(),
            "RrDr60StopFields" => size_of::<RrDr60StopFields>(),
            "RrDr60RunData" => size_of::<RrDr60RunData>(),
            "RrDr60Settings" => size_of::<RrDr60Settings>(),
            _ => unreachable!(),
        };
        assert_eq!(got, size as usize, "{name}");
    }
    // The ABI check (SC-002): the tuple, each wrong alone (the settings size one above and one
    // below), all wrong, 0 and MAX.
    let ok = (1u32, 8usize, 120usize, 16usize, 40usize);
    let call = |t: (u32, usize, usize, usize, usize)| rr_dr60_abi_check(t.0, t.1, t.2, t.3, t.4);
    assert_eq!(call(ok), RrDr60Status::Ok);
    for bad in [
        (2, 8, 120, 16, 40),
        (0, 8, 120, 16, 40),
        (u32::MAX, 8, 120, 16, 40),
        (1, 4, 120, 16, 40),
        (1, 8, 121, 16, 40),
        (1, 8, 119, 16, 40),
        (1, 8, 120, 24, 40),
        (1, 8, 120, 16, 48),
        (0, 4, 0, 0, 0),
    ] {
        assert_eq!(call(bad), RrDr60Status::AbiMismatch, "{bad:?}");
    }
    // D13 at create.
    for size in [119u32, 121] {
        let mut s = rr_dr60_settings_default(48_000);
        s.struct_size = size;
        let mut p = std::ptr::null_mut();
        // SAFETY: valid pointers; a whole struct whatever its size field says.
        assert_eq!(
            unsafe { rr_dr60_create(&s, &mut p) },
            RrDr60Status::InvalidArgument
        );
        assert!(p.is_null());
    }

    // The rows through C, at the measurement rates, against the Rust path.
    for &rate in configs::measurement_rates() {
        let want = ab::rows_at(rate);
        let s = ab::settings(rate);
        let cs = configs::c_settings(&s);
        let x = ab::square(ab::BLOCK);
        let mut y = vec![0.0f32; ab::BLOCK];
        let mut ev = [RrDr60VasEvent::default(); 64];
        let p = create(&cs);
        let mut o = String::new();
        o.push_str("sizes\n");
        for (name, size) in ab::SIZES {
            writeln!(o, "{name} {size}").unwrap();
        }
        writeln!(
            o,
            "defines {RR_DR60_ABI_VERSION} {RR_DR60_MAX_STRUCT_SIZE} {RR_DR60_VAS_EVENT_SIZE} {RR_DR60_CHANGE_ENTRY_SIZE}"
        )
        .unwrap();
        let mut bi = RrDr60BlockInfo::default();
        // SAFETY: a live handle and valid buffers.
        assert_eq!(
            unsafe {
                rr_dr60_process_with_events(
                    p,
                    x.as_ptr(),
                    y.as_mut_ptr(),
                    x.len(),
                    &mut bi,
                    ev.as_mut_ptr(),
                    64,
                )
            },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "block_info {} {} {} {} {}",
            bi.struct_size, bi.produced, bi.events, bi.paused as u32, bi.frames
        )
        .unwrap();
        let mut m = RrDr60Meters::default();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_meters(p, &mut m) }, RrDr60Status::Ok);
        writeln!(
            o,
            "meters {} {:08x} {:08x} {:08x} {:08x} {:08x} {} {} {} {:08x} {:08x} {} {} {:08x}",
            m.struct_size,
            m.input_peak.to_bits(),
            m.limiter_input_peak.to_bits(),
            m.gain_reduction_db.to_bits(),
            m.hold_charge.to_bits(),
            m.record_filter_peak.to_bits(),
            m.adc_clamp_count,
            m.vas_paused,
            m.hang_remaining_samples,
            m.reconstruction_peak.to_bits(),
            m.route_peak.to_bits(),
            m.output_clamp_count,
            m.amp_clip_count,
            m.battery_ceiling.to_bits()
        )
        .unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(p) }, RrDr60Status::Ok);
        let mut ri = RrDr60RecordInfo::default();
        // SAFETY: a live handle and valid buffers.
        assert_eq!(
            unsafe { rr_dr60_record(p, x.as_ptr(), x.len(), &mut ri, ev.as_mut_ptr(), 64) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "record_info {} {} {} {} {} {} {} {}",
            ri.struct_size,
            ri.stored,
            ri.events,
            ri.paused as u32,
            ri.stopped as u32,
            ri.stop_reason,
            ri.frames,
            ri.frames_total
        )
        .unwrap();
        let mut so = RrDr60StopOutcome::default();
        // SAFETY: a live handle and valid buffers.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(p, &mut so, ev.as_mut_ptr(), 64) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "stop_outcome {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            so.struct_size,
            so.flush_stored,
            so.flush_events,
            so.flush_paused as u32,
            so.flush_stopped as u32,
            so.flush_stop_reason,
            so.flush_frames,
            so.flush_frames_total,
            so.kept as u32,
            so.number,
            so.length_samples,
            so.silence_fed_samples,
            so.stop_input_position,
            so.padding_start_sample
        )
        .unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(p, 1) }, RrDr60Status::Ok);
        let mut pi = RrDr60PlayInfo::default();
        // SAFETY: a live handle and a valid buffer.
        assert_eq!(
            unsafe { rr_dr60_play(p, y.as_mut_ptr(), y.len(), &mut pi) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "play_info {} {} {} {} {} {}",
            pi.struct_size, pi.produced, pi.finished as u32, pi.frames, pi.frame_index, pi.place
        )
        .unwrap();
        let st = c_state(p);
        writeln!(
            o,
            "engine_state {} {} {} {} {}",
            st.struct_size, st.state, st.route, st.current_recording, st.speaker_muted as u32
        )
        .unwrap();
        let ms = c_memory(p);
        writeln!(
            o,
            "memory_status {} {} {} {} {} {} {}",
            ms.struct_size,
            ms.capacity_samples,
            ms.used_samples,
            ms.remaining_samples,
            ms.max_recordings,
            ms.count,
            ms.slots_remaining
        )
        .unwrap();
        // SAFETY: a live handle.
        unsafe { rr_dr60_stop_playback(p) };
        let mut ci = RrDr60SpeechCodecInfo::default();
        // SAFETY: a valid out pointer.
        assert_eq!(
            unsafe { rr_dr60_speech_codec_info(&mut ci) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "speech_codec_info {} {} {} {} {} {}.{}.{}",
            ci.struct_size,
            ci.frame_samples,
            ci.frame_ms,
            ci.bits_per_frame,
            ci.delay_samples,
            ci.model_version[0],
            ci.model_version[1],
            ci.model_version[2]
        )
        .unwrap();
        let cstr = |p: *const std::ffi::c_char| {
            // SAFETY: a static NUL-terminated string from the library.
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_str()
                .unwrap()
                .to_owned()
        };
        let mut h = RrDr60ManifestHeader::default();
        // SAFETY: a valid out pointer.
        assert_eq!(unsafe { rr_dr60_manifest_header(&mut h) }, RrDr60Status::Ok);
        writeln!(
            o,
            "manifest_header {} {} {} {} {} {} {} {}",
            h.struct_size,
            cstr(h.library_version),
            h.device_rate_hz,
            h.stage_count,
            h.count_not_modeled,
            h.count_assumed,
            h.count_sourced,
            h.count_checked
        )
        .unwrap();
        let mut e = RrDr60StageEntry::default();
        // SAFETY: a valid out pointer.
        assert_eq!(
            unsafe { rr_dr60_manifest_stage(3, &mut e) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "stage_entry {} {} {} {} {} {} {} {}",
            e.struct_size,
            e.number,
            cstr(e.identifier),
            e.status,
            e.path,
            e.confidence,
            e.source_count,
            e.assumption_count
        )
        .unwrap();
        let mut c = RrDr60Control::default();
        // SAFETY: a valid out pointer.
        assert_eq!(unsafe { rr_dr60_control(0, &mut c) }, RrDr60Status::Ok);
        writeln!(
            o,
            "control {} 0 {} {} {} {} {} {} {} {} {} {:08x} {:08x} {:08x} {} {} {} {} {}",
            c.struct_size,
            cstr(c.identifier),
            c.kind,
            c.timing,
            c.paths,
            c.stage,
            c.value_type,
            c.int_min,
            c.int_max,
            c.int_default,
            c.float_min.to_bits(),
            c.float_max.to_bits(),
            c.float_default.to_bits(),
            c.option_count,
            c.choice_default,
            c.has_default,
            c.source_count,
            c.assumption_count
        )
        .unwrap();
        let (choice, _, _) = ab::first_choice();
        let mut op = RrDr60ControlOption::default();
        // SAFETY: a valid out pointer.
        assert_eq!(
            unsafe { rr_dr60_control_option(choice as u32, 0, &mut op) },
            RrDr60Status::Ok
        );
        writeln!(
            o,
            "control_option {} {} 0 {} {}",
            op.struct_size,
            choice,
            cstr(op.identifier),
            op.value
        )
        .unwrap();
        assert_eq!(o, want, "{rate} Hz: the C rows differ from the Rust rows");

        // The four cases per out-struct on a painted, 8-aligned buffer (US1 AS1 – AS4).
        #[repr(C, align(8))]
        struct Painted([u8; 4200]);
        let paint = |size: u32| {
            let mut b = Painted([0xAA; 4200]);
            b.0[..4].copy_from_slice(&size.to_ne_bytes());
            b
        };
        let size_of_buf = |b: &Painted| u32::from_ne_bytes(b.0[..4].try_into().unwrap());
        type Fill<'a> = Box<dyn Fn(*mut u8) -> RrDr60Status + 'a>;
        let mut cases: Vec<(&str, u32, u32, Fill)> = Vec::new();
        let xp = x.as_ptr();
        let yp = y.as_mut_ptr();
        // SAFETY (each closure): a live handle, valid buffers, and an out pointer that the
        // library checks before writing.
        cases.push((
            "block",
            40,
            40,
            Box::new(move |b| unsafe { rr_dr60_process(p, xp, yp, 64, b.cast()) }),
        ));
        cases.push((
            "state",
            20,
            20,
            Box::new(move |b| unsafe { rr_dr60_get_state(p, b.cast()) }),
        ));
        cases.push((
            "memory",
            48,
            48,
            Box::new(move |b| unsafe { rr_dr60_memory_status(p, b.cast()) }),
        ));
        cases.push((
            "meters",
            56,
            64,
            Box::new(move |b| unsafe { rr_dr60_meters(p, b.cast()) }),
        ));
        cases.push((
            "codec",
            28,
            28,
            Box::new(move |b| unsafe { rr_dr60_speech_codec_info(b.cast()) }),
        ));
        cases.push((
            "header",
            72,
            72,
            Box::new(move |b| unsafe { rr_dr60_manifest_header(b.cast()) }),
        ));
        cases.push((
            "stage",
            80,
            80,
            Box::new(move |b| unsafe { rr_dr60_manifest_stage(3, b.cast()) }),
        ));
        cases.push((
            "control",
            120,
            120,
            Box::new(move |b| unsafe { rr_dr60_control(0, b.cast()) }),
        ));
        cases.push((
            "option",
            24,
            24,
            Box::new(move |b| unsafe { rr_dr60_control_option(choice as u32, 0, b.cast()) }),
        ));
        for (name, min, full, fill) in &cases {
            for size in [0, min - 1, 4097, u32::MAX] {
                let mut b = paint(size);
                assert_eq!(
                    fill(b.0.as_mut_ptr()),
                    RrDr60Status::InvalidArgument,
                    "{name} at {size}"
                );
                assert!(
                    b.0[4..].iter().all(|&v| v == 0xAA),
                    "{name}: nothing written at {size}"
                );
            }
            for size in [*full, full + 16, 4096] {
                let mut b = paint(size);
                assert_eq!(fill(b.0.as_mut_ptr()), RrDr60Status::Ok, "{name} at {size}");
                assert_eq!(size_of_buf(&b), *full, "{name}: the read-back at {size}");
                assert!(
                    b.0[*full as usize..size as usize].iter().all(|&v| v == 0),
                    "{name}: the tail at {size}"
                );
                assert!(
                    b.0[size as usize..].iter().all(|&v| v == 0xAA),
                    "{name}: beyond {size}"
                );
            }
        }
        // The three read-side structs of spec 020 on a processing record built here: the
        // read-out (its fifteen padding bytes zero), the alignment and the place state.
        {
            let loud = vec![0.5f32; 4096];
            let mut e1 = [RrDr60VasEvent::default(); 64];
            let mut events: Vec<RrDr60VasEvent> = Vec::new();
            // SAFETY: a live handle and valid buffers.
            unsafe {
                assert_eq!(rr_dr60_start_recording(p), RrDr60Status::Ok);
                let mut ri = RrDr60RecordInfo::default();
                assert_eq!(
                    rr_dr60_record(p, loud.as_ptr(), 4096, &mut ri, e1.as_mut_ptr(), 64),
                    RrDr60Status::Ok
                );
                events.extend_from_slice(&e1[..ri.events]);
                let mut so = RrDr60StopOutcome::default();
                assert_eq!(
                    rr_dr60_stop_recording(p, &mut so, e1.as_mut_ptr(), 64),
                    RrDr60Status::Ok
                );
                assert!(so.kept);
                events.extend_from_slice(&e1[..so.flush_events]);
                let stop = RrDr60StopFields {
                    struct_size: size_of::<RrDr60StopFields>() as u32,
                    how: RR_DR60_STOP_CALL,
                    stop_input_position: so.stop_input_position,
                    padding_start_sample: so.padding_start_sample,
                    input_samples_total: so.stop_input_position,
                    silence_fed_samples: so.silence_fed_samples,
                };
                let run = RrDr60RunData {
                    struct_size: size_of::<RrDr60RunData>() as u32,
                    reserved: 0,
                    events: events.as_ptr(),
                    event_count: events.len(),
                    changes: std::ptr::null(),
                    change_count: 0,
                    stop,
                };
                let mut len = 0usize;
                assert_eq!(
                    rr_dr60_processing_record_len(p, so.number, &run, &mut len),
                    RrDr60Status::Ok
                );
                let mut rec = vec![0u8; len];
                let mut written = 0usize;
                assert_eq!(
                    rr_dr60_processing_record_write(
                        p,
                        so.number,
                        &run,
                        rec.as_mut_ptr(),
                        len,
                        &mut written
                    ),
                    RrDr60Status::Ok
                );
                let readout_pads: [(usize, usize); 5] =
                    [(212, 216), (266, 268), (281, 284), (305, 308), (329, 332)];
                type ReadCase<'a> = (&'a str, u32, &'a [(usize, usize)], Fill<'a>);
                let mut read_cases: Vec<ReadCase> = Vec::new();
                let rp = rec.as_ptr();
                let sp = &stop as *const RrDr60StopFields;
                let ep = events.as_ptr();
                let ne = events.len();
                let csp = &cs as *const RrDr60Settings;
                read_cases.push((
                    "readout",
                    376,
                    &readout_pads,
                    Box::new(move |b| rr_dr60_processing_record_read(p, rp, len, b.cast())),
                ));
                read_cases.push((
                    "alignment",
                    32,
                    &[],
                    Box::new(move |b| rr_dr60_kept_to_input(csp, 0, ep, ne, sp, b.cast())),
                ));
                read_cases.push((
                    "place",
                    32,
                    &[],
                    Box::new(move |b| {
                        rr_dr60_place_at(10, rate, 3, 1500, std::ptr::null(), 0, b.cast())
                    }),
                ));
                for (name, full, pads, fill) in &read_cases {
                    for size in [0, full - 1, 4097, u32::MAX] {
                        let mut b = paint(size);
                        assert_eq!(
                            fill(b.0.as_mut_ptr()),
                            RrDr60Status::InvalidArgument,
                            "{name} at {size}"
                        );
                        assert!(
                            b.0[4..].iter().all(|&v| v == 0xAA),
                            "{name}: nothing written at {size}"
                        );
                    }
                    for size in [*full, full + 16, 4096] {
                        let mut b = paint(size);
                        assert_eq!(fill(b.0.as_mut_ptr()), RrDr60Status::Ok, "{name} at {size}");
                        assert_eq!(size_of_buf(&b), *full, "{name}: the read-back at {size}");
                        for &(lo, hi) in pads.iter() {
                            assert!(
                                b.0[lo..hi].iter().all(|&v| v == 0),
                                "{name}: padding {lo}..{hi} at {size}"
                            );
                        }
                        assert!(
                            b.0[*full as usize..size as usize].iter().all(|&v| v == 0),
                            "{name}: the tail at {size}"
                        );
                        assert!(
                            b.0[size as usize..].iter().all(|&v| v == 0xAA),
                            "{name}: beyond {size}"
                        );
                    }
                }
            }
        }
        // The record, stop and play reports on their own transport states.
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_recording(p) }, RrDr60Status::Ok);
        for size in [0u32, 47, 4097] {
            let mut b = paint(size);
            // SAFETY: a live handle and valid buffers.
            assert_eq!(
                unsafe {
                    rr_dr60_record(
                        p,
                        x.as_ptr(),
                        64,
                        b.0.as_mut_ptr().cast(),
                        std::ptr::null_mut(),
                        0,
                    )
                },
                RrDr60Status::InvalidArgument
            );
            assert!(b.0[4..].iter().all(|&v| v == 0xAA));
        }
        let mut b = paint(64);
        // SAFETY: as above.
        assert_eq!(
            unsafe {
                rr_dr60_record(
                    p,
                    x.as_ptr(),
                    x.len(),
                    b.0.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                    0,
                )
            },
            RrDr60Status::Ok
        );
        assert_eq!(size_of_buf(&b), 48);
        assert!(b.0[48..64].iter().all(|&v| v == 0) && b.0[64..].iter().all(|&v| v == 0xAA));
        let mut b = paint(87);
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(p, b.0.as_mut_ptr().cast(), std::ptr::null_mut(), 0) },
            RrDr60Status::InvalidArgument
        );
        assert!(b.0[4..].iter().all(|&v| v == 0xAA));
        let mut b = paint(100);
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_stop_recording(p, b.0.as_mut_ptr().cast(), std::ptr::null_mut(), 0) },
            RrDr60Status::Ok
        );
        assert_eq!(size_of_buf(&b), 88);
        assert!(b.0[88..100].iter().all(|&v| v == 0) && b.0[100..].iter().all(|&v| v == 0xAA));
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(p, 1) }, RrDr60Status::Ok);
        let mut b = paint(47);
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_play(p, y.as_mut_ptr(), 64, b.0.as_mut_ptr().cast()) },
            RrDr60Status::InvalidArgument
        );
        assert!(b.0[4..].iter().all(|&v| v == 0xAA));
        let mut b = paint(64);
        // SAFETY: as above.
        assert_eq!(
            unsafe { rr_dr60_play(p, y.as_mut_ptr(), 64, b.0.as_mut_ptr().cast()) },
            RrDr60Status::Ok
        );
        assert_eq!(size_of_buf(&b), 48);
        assert!(b.0[48..64].iter().all(|&v| v == 0) && b.0[64..].iter().all(|&v| v == 0xAA));
        // SAFETY: a live handle.
        unsafe { rr_dr60_stop_playback(p) };
        // A refused report processes nothing: the positions, the memory status and the meters.
        let snap = |p: *mut RrDr60Pipeline| {
            let mut pos = [0u64; 2];
            // SAFETY: a live handle.
            unsafe {
                rr_dr60_stream_position(p, 1, &mut pos[0]);
                rr_dr60_input_position(p, 1, &mut pos[1]);
            }
            (pos, c_memory(p), {
                let mut m = RrDr60Meters::default();
                // SAFETY: a live handle.
                unsafe { rr_dr60_meters(p, &mut m) };
                m
            })
        };
        let before = snap(p);
        let mut b = paint(39);
        // SAFETY: as above; refused.
        assert_eq!(
            unsafe {
                rr_dr60_process(
                    p,
                    x.as_ptr(),
                    y.as_mut_ptr(),
                    x.len(),
                    b.0.as_mut_ptr().cast(),
                )
            },
            RrDr60Status::InvalidArgument
        );
        let mut b = paint(4097);
        // SAFETY: as above; refused.
        assert_eq!(
            unsafe {
                rr_dr60_record(
                    p,
                    x.as_ptr(),
                    x.len(),
                    b.0.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                    0,
                )
            },
            RrDr60Status::InvalidArgument
        );
        // SAFETY: as above; refused.
        assert_eq!(
            unsafe { rr_dr60_play(p, y.as_mut_ptr(), y.len(), b.0.as_mut_ptr().cast()) },
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            before,
            snap(p),
            "{rate} Hz: a refused report processed something"
        );
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(p) };
    }
    // The frozen entry's field is `stream` (D2): a value round trip through the C struct.
    let e = RrDr60ChangeEntry {
        position: 1,
        before: 0.0,
        after: 1.0,
        tag: 2,
        control: 3,
        stream: 2,
    };
    assert_eq!(e.stream, 2);
}

/// Spec 023 (tasks.md T015): the mode through the C API: the setter and getter, status 38, the
/// two defines, the length helper in both modes, `rr_dr60_place_at_in_mode` on a log, the
/// refusals, and a playback with the mode set by the setter and scheduled at a position
/// bit-identical between the Rust and C paths on a square-wave stimulus at the measurement
/// rates, the logs equal.
#[test]
fn pitch_parity() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::speed::SpeedMode;
    use rr_dr60::{ChangeEntry, Route, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, RrDr60PlaceState, RrDr60PlayInfo, rr_dr60_get_playback_speed_mode,
        rr_dr60_import, rr_dr60_place_at_in_mode, rr_dr60_play_with_changes,
        rr_dr60_playback_length_samples_in_mode, rr_dr60_schedule_change,
        rr_dr60_set_playback_speed, rr_dr60_set_playback_speed_mode, rr_dr60_set_route,
        rr_dr60_start_playback,
    };
    use rr_dr60_harness::pitch_checks as pc;
    assert_eq!(RrDr60Status::InvalidSpeedMode as u32, 38);
    assert_eq!(rr_dr60_ffi::RR_DR60_SPEED_MODE_PITCH_WITH_SPEED, 0);
    assert_eq!(rr_dr60_ffi::RR_DR60_SPEED_MODE_PITCH_KEPT, 1);
    assert_eq!(rr_dr60_ffi::RR_DR60_MAX_CHANGES_PER_CALL, 142);
    let stored = pc::square17(9000);
    for &rate in configs::measurement_rates() {
        let s = configs::settings("speed_default_no_codec", rate);
        let cs = configs::c_settings(&s);
        let mut p = pc::engine(
            &|s| Pipeline::new(s).unwrap(),
            "speed_default_no_codec",
            rate,
            &stored,
        );
        let mut bytes = vec![0u8; p.export_len(1).unwrap()];
        let _ = p.export(1, &mut bytes).unwrap();
        let h = create(&cs);
        let mut imported = 0u32;
        // SAFETY: a live handle and the exported bytes.
        assert_eq!(
            unsafe { rr_dr60_import(h, bytes.as_ptr(), bytes.len(), &mut imported) },
            RrDr60Status::Ok
        );
        p.set_route(Route::Speaker);
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_set_route(h, 0) }, RrDr60Status::Ok);
        // The setter, the getter and the refusal.
        p.set_playback_speed(1).unwrap();
        p.set_playback_speed_mode(SpeedMode::PitchKept);
        let mut mode = 9u32;
        // SAFETY: a live handle and a writable out-pointer.
        unsafe {
            assert_eq!(rr_dr60_set_playback_speed(h, 1), RrDr60Status::Ok);
            assert_eq!(rr_dr60_set_playback_speed_mode(h, 1), RrDr60Status::Ok);
            assert_eq!(
                rr_dr60_set_playback_speed_mode(h, 2),
                RrDr60Status::InvalidSpeedMode
            );
            assert_eq!(
                rr_dr60_get_playback_speed_mode(h, &mut mode),
                RrDr60Status::Ok
            );
        }
        assert_eq!(mode, p.playback_speed_mode() as u32);
        // The length helper in both modes at the five levels.
        for level in 1..=5u8 {
            for (m, mode) in [(0u32, SpeedMode::PitchWithSpeed), (1, SpeedMode::PitchKept)] {
                let want = p.playback_length_samples_in_mode(1, level, mode).unwrap();
                let mut got = 0u64;
                // SAFETY: a live handle and a writable out-pointer.
                assert_eq!(
                    unsafe { rr_dr60_playback_length_samples_in_mode(h, 1, level, m, &mut got) },
                    RrDr60Status::Ok
                );
                assert_eq!(got, want, "{rate} Hz L{level} mode {m}");
                if m == 0 {
                    assert_eq!(want, p.playback_length_samples(1, level).unwrap());
                }
            }
        }
        // The schedule: a level change and two mode changes; the refusals.
        let n = u64::from(rate) / 50;
        let sched = [
            (200u64, 2u32, 5.0f64, 1u64),
            (200 + 3 * n, 30, 0.0, 2),
            (200 + 6 * n, 30, 1.0, 3),
            (200 + 9 * n, 2, 2.0, 4),
        ];
        for (pos, c, v, tag) in sched {
            p.schedule_change(Stream::Playback, pos, c, v, tag).unwrap();
            // SAFETY: a live handle.
            assert_eq!(
                unsafe { rr_dr60_schedule_change(h, 0, pos, c, v, tag) },
                RrDr60Status::Ok,
                "{rate} Hz"
            );
        }
        // SAFETY: a live handle.
        unsafe {
            assert_eq!(
                rr_dr60_schedule_change(h, 1, 0, 30, 1.0, 9),
                RrDr60Status::ControlNotOnStream
            );
            assert_eq!(
                rr_dr60_schedule_change(h, 2, 0, 30, 1.0, 9),
                RrDr60Status::ControlNotOnStream
            );
            assert_eq!(
                rr_dr60_schedule_change(h, 0, 0, 30, 0.5, 9),
                RrDr60Status::InvalidArgument
            ); // InvalidControlValue
            assert_eq!(
                rr_dr60_schedule_change(h, 0, 0, 30, 2.0, 9),
                RrDr60Status::InvalidSpeedMode
            );
        }
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        assert_eq!(unsafe { rr_dr60_start_playback(h, 1) }, RrDr60Status::Ok);
        let mut rust = Vec::new();
        let mut c_out = Vec::new();
        let mut rust_log = Vec::new();
        let mut c_log = Vec::new();
        let mut y = vec![0.0f32; 512];
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let mut clog = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        loop {
            let info = p.play_with_changes(&mut y, &mut log).unwrap();
            rust.extend_from_slice(&y[..info.produced]);
            rust_log.extend_from_slice(&log[..info.changes]);
            let mut ci = RrDr60PlayInfo::default();
            let mut count = 0usize;
            // SAFETY: a live handle and valid buffers.
            let st = unsafe {
                rr_dr60_play_with_changes(
                    h,
                    y.as_mut_ptr(),
                    512,
                    &mut ci,
                    clog.as_mut_ptr(),
                    MAX_CHANGES_PER_CALL,
                    &mut count,
                )
            };
            assert_eq!(st, RrDr60Status::Ok);
            assert_eq!(ci.produced, info.produced);
            assert_eq!(ci.place, info.place);
            c_out.extend_from_slice(&y[..ci.produced]);
            c_log.extend_from_slice(&clog[..count]);
            if info.finished {
                assert!(ci.finished);
                break;
            }
        }
        assert_eq!(rust.len(), c_out.len(), "{rate} Hz");
        assert!(
            rust.iter()
                .zip(&c_out)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{rate} Hz: the C path"
        );
        assert_eq!(rust_log.len(), c_log.len());
        assert_eq!(rust_log.len(), 4, "{rate} Hz: every change logged");
        for (r, c) in rust_log.iter().zip(&c_log) {
            assert_eq!(
                (
                    r.position,
                    r.control,
                    r.before.to_bits(),
                    r.after.to_bits(),
                    r.tag,
                    r.stream
                ),
                (
                    c.position,
                    c.control,
                    c.before.to_bits(),
                    c.after.to_bits(),
                    c.tag,
                    c.stream
                )
            );
        }
        // Map (c) in a mode on the log: the C struct equals the Rust state at several outputs.
        let k = p.recording_length(1).unwrap();
        for index in [0u64, 500, 2000, 5000, 20_000] {
            for start_mode in [0u32, 1] {
                let want =
                    rr_dr60::timeline::place_at_in_mode(index, rate, 1, start_mode, k, &rust_log)
                        .unwrap();
                let mut st = RrDr60PlaceState::default();
                // SAFETY: valid pointers and the count.
                let status = unsafe {
                    rr_dr60_place_at_in_mode(
                        index,
                        rate,
                        1,
                        start_mode,
                        k,
                        c_log.as_ptr(),
                        c_log.len(),
                        &mut st,
                    )
                };
                assert_eq!(status, RrDr60Status::Ok);
                assert_eq!(
                    (
                        st.level,
                        st.fading,
                        st.latched,
                        st.place_fifths,
                        st.old_place_fifths
                    ),
                    (
                        u32::from(want.level),
                        u32::from(want.fading),
                        u32::from(want.latched),
                        want.place_fifths,
                        want.old_place_fifths
                    ),
                    "{rate} Hz output {index} mode {start_mode}"
                );
            }
            let mut st = RrDr60PlaceState::default();
            // SAFETY: valid pointers and the count.
            assert_eq!(
                unsafe {
                    rr_dr60_place_at_in_mode(
                        index,
                        rate,
                        1,
                        2,
                        k,
                        c_log.as_ptr(),
                        c_log.len(),
                        &mut st,
                    )
                },
                RrDr60Status::InvalidSpeedMode
            );
        }
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(h) };
    }
}

/// Spec 025 (tasks.md T010): the C functions agree with the Rust ones on every index: the
/// engine's readings after the same live writes, the structure's readings of the same set, the
/// statuses of the refusals (39, 40), and a reconfigure confirmed by the read.
#[test]
fn settings_read_parity() {
    use rr_dr60::control_map::{self, ControlReading};
    use rr_dr60::{Error, Settings};
    use rr_dr60_ffi::{
        RrDr60ControlReading, rr_dr60_control_value, rr_dr60_get_settings, rr_dr60_reconfigure,
        rr_dr60_set_control_value, rr_dr60_settings_control_value,
    };
    use rr_dr60_harness::settings_read_checks as sr;
    assert_eq!(RrDr60Status::NotASetting as u32, 39);
    assert_eq!(RrDr60Status::NotAPlainLiveControl as u32, 40);
    for &rate in configs::measurement_rates() {
        let want = sr::differ(rate);
        let mut p = Pipeline::new(Settings::new(rate)).unwrap();
        p.reconfigure(want).unwrap();
        let cw = configs::c_settings(&want);
        let h = create(&rr_dr60_ffi::rr_dr60_settings_default(rate));
        // SAFETY: a live handle and a valid struct.
        assert_eq!(unsafe { rr_dr60_reconfigure(h, &cw) }, RrDr60Status::Ok);
        for (index, r, _, _) in sr::live_cases() {
            p.set_control_value(index, r).unwrap();
            let c = sr::c_reading(r);
            // SAFETY: a live handle and a local reading.
            assert_eq!(
                unsafe { rr_dr60_set_control_value(h, index, &c) },
                RrDr60Status::Ok
            );
        }
        let mut got = rr_dr60_ffi::rr_dr60_settings_default(rate);
        // SAFETY: a live handle and a local struct.
        assert_eq!(
            unsafe { rr_dr60_get_settings(h, &mut got) },
            RrDr60Status::Ok
        );
        assert_eq!(got, cw, "{rate} Hz: the C read equals the reconfigure");
        for i in 0..31u32 {
            let mut out = RrDr60ControlReading::default();
            // SAFETY: a live handle and a local reading.
            assert_eq!(
                unsafe { rr_dr60_control_value(h, i, &mut out) },
                RrDr60Status::Ok
            );
            assert_eq!(
                sr::rust_reading(&out),
                Some(p.control_value(i).unwrap()),
                "{rate} Hz: {i}"
            );
            let mut out2 = RrDr60ControlReading::default();
            // SAFETY: a local struct and reading.
            let st = unsafe { rr_dr60_settings_control_value(&cw, i, &mut out2) };
            match control_map::settings_value(&want, i) {
                Ok(r) => {
                    assert_eq!(st, RrDr60Status::Ok);
                    assert_eq!(sr::rust_reading(&out2), Some(r), "{rate} Hz: pure {i}");
                }
                Err(Error::NotASetting { .. }) => assert_eq!(st, RrDr60Status::NotASetting),
                Err(e) => panic!("{e}"),
            }
        }
        // The refusals map to the same statuses.
        let cases = [
            (
                1u32,
                ControlReading::Continuous(1.5),
                RrDr60Status::InvalidVolume,
            ),
            (
                2,
                ControlReading::Integer(0),
                RrDr60Status::InvalidSpeedLevel,
            ),
            (
                29,
                ControlReading::Continuous(3.5),
                RrDr60Status::InvalidBatteryVolts,
            ),
            (
                1,
                ControlReading::Continuous(f32::NAN),
                RrDr60Status::InvalidArgument,
            ),
            (
                10,
                ControlReading::Continuous(-3.0),
                RrDr60Status::NotAPlainLiveControl,
            ),
            (31, ControlReading::Integer(0), RrDr60Status::NoSuchControl),
        ];
        for (i, r, st) in cases {
            assert!(p.set_control_value(i, r).is_err());
            let c = sr::c_reading(r);
            // SAFETY: a live handle and a local reading.
            assert_eq!(
                unsafe { rr_dr60_set_control_value(h, i, &c) },
                st,
                "{i} {r:?}"
            );
        }
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_ffi::rr_dr60_destroy(h) };
    }
}
