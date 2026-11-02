//! Spec 025 User Story 1: read what is configured (tasks.md T006; spec › US1 AS1 – AS4).
//! Report: `cargo test -p rr_dr60_harness --test us1_settings_read -- --nocapture`.

use rr_dr60::control_map::ControlReading;
use rr_dr60::{ChangeEntry, Error, Pipeline, Settings, Stream, VasEvent};
use rr_dr60_ffi::*;
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks as rc;
use rr_dr60_harness::settings_read_checks as sr;

/// AS1: a reconfigure differing in every setting reads back field by field in Rust and through
/// C (into a painted buffer whose padding reads zero), and the next recording's processing
/// record agrees.
#[test]
fn us1_settings_read() {
    for &rate in configs::measurement_rates() {
        let want = sr::differ(rate);
        let mut p = Pipeline::new(Settings::new(sr::other_rate(rate))).unwrap();
        p.reconfigure(want).unwrap();
        assert_eq!(*p.settings(), want, "{rate} Hz: Rust");
        // Through C.
        let base = rr_dr60_settings_default(sr::other_rate(rate));
        let mut h = std::ptr::null_mut();
        let cw = sr::differ_c(rate);
        // SAFETY: valid pointers to local values throughout.
        unsafe {
            assert_eq!(rr_dr60_create(&base, &mut h), RrDr60Status::Ok);
            assert_eq!(rr_dr60_reconfigure(h, &cw), RrDr60Status::Ok);
        }
        let mut buf = [0xAAu8; 128];
        buf[..4].copy_from_slice(&120u32.to_ne_bytes());
        // SAFETY: a 128-byte buffer; the library reads and writes it unaligned.
        assert_eq!(
            unsafe { rr_dr60_get_settings(h, buf.as_mut_ptr().cast()) },
            RrDr60Status::Ok
        );
        for (lo, hi) in sr::SETTINGS_PADDING {
            assert!(
                buf[lo..hi].iter().all(|&b| b == 0),
                "{rate} Hz: padding {lo}..{hi} zero"
            );
        }
        assert!(
            buf[120..].iter().all(|&b| b == 0xAA),
            "{rate} Hz: nothing past the struct"
        );
        // SAFETY: 120 bytes of a filled settings struct, read unaligned.
        let got: RrDr60Settings = unsafe { buf.as_ptr().cast::<RrDr60Settings>().read_unaligned() };
        assert_eq!(got, cw, "{rate} Hz: C field by field");
        // SAFETY: destroying once.
        unsafe { rr_dr60_destroy(h) };
        // The next recording's processing record carries the same set.
        let cell = rc::Cell {
            codec: false,
            changes: false,
            mute: true,
            self_stop: false,
        };
        let (_p, run) = rc::record_run_in(&mut p, cell, &rc::raw_stimulus(rate));
        let r = rc::readout(&run.record);
        assert_eq!(r.settings, want, "{rate} Hz: the processing record");
    }
}

/// AS2: landed values are in the read, equal to the change log's `after`; an unlanded
/// schedule is not.
#[test]
fn us1_settings_read_landed() {
    for &rate in configs::measurement_rates() {
        let mut s = Settings::new(rate);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let x = rr_dr60_harness::stimulus::tone(1000.0, 0.1, f64::from(rate), 4000);
        let sched = [
            Change {
                stream: Stream::OnePass,
                position: 1000,
                control: 10,
                value: -10.0,
                tag: 7,
            },
            Change {
                stream: Stream::OnePass,
                position: 2000,
                control: 23,
                value: 0.0,
                tag: 8,
            },
            Change {
                stream: Stream::OnePass,
                position: 1_000_000,
                control: 11,
                value: 9.0,
                tag: 9,
            },
        ];
        cc::schedule_all(&mut p, &sched);
        assert_eq!(*p.settings(), s, "{rate} Hz: nothing landed yet");
        let r = cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(333));
        let knee = cc::entries_of(&r.log, 10);
        let spk = cc::entries_of(&r.log, 23);
        assert_eq!((knee.len(), spk.len()), (1, 1), "{rate} Hz");
        assert_eq!(f64::from(p.settings().agc.knee_dbfs), knee[0].after);
        assert_eq!(
            f64::from(u8::from(p.settings().output.speaker_enabled)),
            spk[0].after
        );
        assert_eq!(
            p.control_value(10).unwrap(),
            ControlReading::Continuous(-10.0)
        );
        assert_eq!(p.control_value(23).unwrap(), ControlReading::Switch(false));
        assert_eq!(
            p.settings().agc.attack_ms,
            s.agc.attack_ms,
            "{rate} Hz: the unlanded change is absent"
        );
        assert_eq!(cc::entries_of(&r.log, 11).len(), 0);
    }
}

/// AS2 (the trap, spec 021's): a reconfigure with the read after a switch landed configures the
/// stage off, so switching it back on is refused.
#[test]
fn us1_trap_named() {
    for &rate in configs::measurement_rates() {
        let mut s = Settings::new(rate);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let x = rr_dr60_harness::stimulus::tone(1000.0, 0.1, f64::from(rate), 1000);
        cc::schedule_all(
            &mut p,
            &[Change {
                stream: Stream::OnePass,
                position: 10,
                control: 23,
                value: 0.0,
                tag: 1,
            }],
        );
        let _ = cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(256));
        let read = *p.settings();
        assert!(!read.output.speaker_enabled);
        p.reconfigure(read).unwrap();
        assert!(
            matches!(
                p.schedule_change(Stream::OnePass, 0, 23, 1.0, 2),
                Err(Error::StageNotConfigured { control: 23 })
            ),
            "{rate} Hz: the trap"
        );
    }
}

/// AS3: a plain setter is in the read at once, before any processing call logs it.
#[test]
fn us1_committed_not_audible() {
    for &rate in configs::measurement_rates() {
        let mut p = Pipeline::new(Settings::new(rate)).unwrap();
        p.set_volume(0.3).unwrap();
        assert_eq!(p.control_value(1).unwrap(), ControlReading::Continuous(0.3));
        let mut log = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let mut ev = vec![VasEvent::default(); p.max_events(1)];
        let none = p
            .process_with_events_and_changes(&[], &mut [], &mut ev, &mut log)
            .unwrap();
        assert_eq!(none.changes, 0, "{rate} Hz: the log is still empty");
        let mut y = [0.0f32; 1];
        let one = p
            .process_with_events_and_changes(&[0.0], &mut y, &mut ev, &mut log)
            .unwrap();
        let vol: Vec<_> = log[..one.changes]
            .iter()
            .filter(|e| e.control == 1)
            .collect();
        assert_eq!(vol.len(), 1);
        assert_eq!(vol[0].after, f64::from(0.3f32));
    }
}

/// AS4: a settings size other than the settings size is refused, nothing written.
#[test]
fn us1_size_refused() {
    let base = rr_dr60_settings_default(48_000);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&base, &mut h) }, RrDr60Status::Ok);
    for size in [0u32, 4, 119, 121, 4096, u32::MAX] {
        let mut buf = [0xAAu8; 128];
        buf[..4].copy_from_slice(&size.to_ne_bytes());
        // SAFETY: a 128-byte buffer; refused before any write past the size field.
        assert_eq!(
            unsafe { rr_dr60_get_settings(h, buf.as_mut_ptr().cast()) },
            RrDr60Status::InvalidArgument
        );
        assert!(
            buf[4..].iter().all(|&b| b == 0xAA),
            "{size}: nothing written"
        );
    }
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}
