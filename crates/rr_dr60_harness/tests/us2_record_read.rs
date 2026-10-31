//! Spec 020 US2 (AS1 – AS5): every refusal and flag through the engine on a doctored set; the
//! imported origin; the short and inconsistent runs; the digest check against a recording.

use rr_dr60::record::{self, RecordReadout, RunData, flags};
use rr_dr60::{Error, Pipeline, Settings};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell};

fn at(f: impl Fn(u32)) {
    for &rate in configs::measurement_rates() {
        f(rate);
    }
}

fn doctor(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains(from), "{from}");
    text.replacen(from, to, 1).into_bytes()
}

/// Re-signs an edited record (the checksum line recomputed) so the edit, not the checksum, is
/// what the reader sees.
fn resign(bytes: Vec<u8>) -> Vec<u8> {
    let body_end = bytes[..bytes.len() - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap()
        + 1;
    let h = rr_dr60::digest::fnv1a64(&bytes[..body_end]);
    let mut v = bytes[..body_end].to_vec();
    v.extend_from_slice(format!("record_checksum = fnv1a64:{h:016x}\n").as_bytes());
    v
}

fn read(p: &Pipeline, bytes: &[u8]) -> Result<RecordReadout, Error> {
    let mut r = RecordReadout::default();
    p.processing_record_read(bytes, &mut r).map(|()| r)
}

fn cell() -> Cell {
    Cell {
        codec: true,
        changes: true,
        mute: false,
        self_stop: false,
    }
}

/// AS1: the refusals, each with its status, and nothing read.
#[test]
fn as1_refusals() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (p, run) = rc::record_run(&standard, rate, cell(), &raw);
        let b = &run.record;
        let tag = format!("{rate} Hz");
        assert_eq!(
            read(&p, &doctor(b, "version = 1", "version = 0")).unwrap_err(),
            Error::UnknownRecordVersion { version: 0 },
            "{tag}"
        );
        assert_eq!(
            read(&p, &doctor(b, "version = 1", "version = 2")).unwrap_err(),
            Error::UnknownRecordVersion { version: 2 },
            "{tag}"
        );
        assert!(
            matches!(
                read(&p, &resign(doctor(b, "seed = ", "# seed = "))).unwrap_err(),
                Error::MalformedRecord { .. }
            ),
            "{tag}: a comment"
        );
        let mut flipped = b.clone();
        flipped[100] ^= 0x01;
        assert_eq!(
            read(&p, &flipped).unwrap_err(),
            Error::RecordChecksumMismatch,
            "{tag}"
        );
        assert_eq!(
            read(&p, &b[..b.len() - 1]).unwrap_err(),
            Error::MalformedRecord { line: 0 },
            "{tag}: no final LF"
        );
        assert!(
            matches!(
                read(&p, &b[..b.len() - 30]).unwrap_err(),
                Error::MalformedRecord { .. }
                    | Error::MissingRecordField { .. }
                    | Error::RecordChecksumMismatch
            ),
            "{tag}: truncated"
        );
        let _ = read_missing(&p, b, "seed = ", "seed");
        let _ = read_missing(&p, b, "run.events_total = ", "run.events_total");
        assert!(
            matches!(
                read(
                    &p,
                    &resign(doctor(b, "run.stop = stop_call", "run.stop = fell_over"))
                )
                .unwrap_err(),
                Error::MalformedRecord { .. }
            ),
            "{tag}"
        );
    });
}

fn read_missing(p: &Pipeline, b: &[u8], prefix: &str, field: &'static str) -> Error {
    let text = String::from_utf8(b.to_vec()).unwrap();
    let line = text.lines().find(|l| l.starts_with(prefix)).unwrap();
    let e = read(p, &resign(doctor(b, &format!("{line}\n"), ""))).unwrap_err();
    assert_eq!(e, Error::MissingRecordField { field });
    e
}

/// AS2: differences reported as flags, never refusals; the unknown names listed.
#[test]
fn as2_differences_reported() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (p, run) = rc::record_run(&standard, rate, cell(), &raw);
        let b = &run.record;
        let tag = format!("{rate} Hz");
        assert_eq!(read(&p, b).unwrap().flags, 0, "{tag}");
        let r = read(
            &p,
            &resign(doctor(b, "library_version = 0.", "library_version = 9.")),
        )
        .unwrap();
        assert_eq!(r.flags, flags::LIBRARY_DIFFERS, "{tag}");
        let r = read(
            &p,
            &resign(doctor(
                b,
                "stage.5.model_version = 0.",
                "stage.5.model_version = 9.",
            )),
        )
        .unwrap();
        assert_eq!(r.flags, flags::STAGE_MODEL_DIFFERS, "{tag}");
        assert_eq!(r.stage_mask, 1 << 4);
        let other = if rate == 48_000 { 44_100 } else { 48_000 };
        let r = read(
            &p,
            &resign(doctor(
                b,
                &format!("host_rate_hz = {rate}"),
                &format!("host_rate_hz = {other}"),
            )),
        )
        .unwrap();
        assert_eq!(r.flags, flags::HOST_RATE_DIFFERS, "{tag}");
        let b2 = resign(doctor(
            b,
            "setting.tap = ",
            "setting.future.knob = 1\nsetting.tap = ",
        ));
        let b2 = resign(doctor(
            &b2,
            "change.1 = ",
            "change.1 = 0 agc.future 0 0 0 record\nchange.2 = ",
        ));
        let r = read(&p, &b2).unwrap();
        assert_eq!(r.flags, flags::UNKNOWN_SETTINGS, "{tag}");
        assert_eq!(r.unknown_setting_count, 2);
        assert_eq!(record::record_unknown_setting(&b2, 0), Some("future.knob"));
        assert_eq!(record::record_unknown_setting(&b2, 1), Some("agc.future"));
        assert_eq!(record::record_unknown_setting(&b2, 2), None);
        assert_eq!(r.change_count, 3);
        let mut ch = vec![rr_dr60::ChangeEntry::default(); 4];
        assert_eq!(
            record::record_changes(&b2, 0, &mut ch),
            2,
            "{tag}: the unknown row skipped"
        );
    });
}

/// AS3: the digest check against a recording in the memory.
#[test]
fn as3_digest_check_against_a_recording() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (mut p, run) = rc::record_run(&standard, rate, cell(), &raw);
        let mut r = read(&p, &run.record).unwrap();
        assert!(
            p.recording_digest_matches(run.number, &mut r).unwrap(),
            "{rate} Hz"
        );
        assert_eq!(r.flags, 0);
        // A second recording: the record does not belong to it.
        let (q, run2) = rc::record_run_in(&mut p, cell(), &raw[..raw.len() / 2]);
        assert_ne!(run2.digest, run.digest);
        assert!(
            !q.recording_digest_matches(run2.number, &mut r).unwrap(),
            "{rate} Hz"
        );
        assert_eq!(r.flags, flags::DIGEST_MISMATCH);
        assert!(q.recording_digest_matches(run.number, &mut r).unwrap());
        assert_eq!(r.flags, 0);
        assert_eq!(
            q.recording_digest_matches(9, &mut r).unwrap_err(),
            Error::NoSuchRecording { number: 9 }
        );
    });
}

/// AS4: an imported recording's record names the stored form and the digest only.
#[test]
fn as4_imported_origin() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (mut p, run) = rc::record_run(&standard, rate, cell(), &raw);
        let n = p.import(&run.export).unwrap();
        assert_eq!(p.recording_counter_of(n).unwrap(), None);
        let data = RunData {
            events: &run.events,
            changes: &run.changes,
            stop: run.stop,
        };
        let mut rec = vec![0u8; p.processing_record_len(n, &data).unwrap()];
        p.processing_record_write(n, &data, &mut rec).unwrap();
        let text = String::from_utf8(rec.clone()).unwrap();
        assert!(text.contains("\norigin = imported\n"), "{rate} Hz");
        for absent in [
            "host_rate_hz",
            "stage.",
            "seed",
            "recording_counter",
            "setting.",
            "run.",
            "event.",
            "change.",
        ] {
            assert!(!text.contains(absent), "{rate} Hz: {absent} absent");
        }
        let mut r = read(&p, &rec).unwrap();
        assert_eq!(r.flags, flags::IMPORTED_ORIGIN);
        assert_eq!(r.recording_digest, run.digest);
        assert!(p.recording_digest_matches(n, &mut r).unwrap());
        assert_eq!(
            r.settings,
            Settings::new(6000),
            "the placeholder configuration"
        );
    });
}

/// AS5: a short run is written and flagged; an inconsistent run is refused with its reason.
#[test]
fn as5_short_and_inconsistent_runs() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (p, run) = rc::record_run(&standard, rate, cell(), &raw);
        let short = RunData {
            events: &run.events[..1],
            changes: &[],
            stop: run.stop,
        };
        let mut rec = vec![0u8; p.processing_record_len(run.number, &short).unwrap()];
        p.processing_record_write(run.number, &short, &mut rec)
            .unwrap();
        let r = read(&p, &rec).unwrap();
        assert_eq!(r.flags, flags::INCOMPLETE_RUN, "{rate} Hz");
        assert_eq!((r.event_count, r.change_count), (1, 0));
        assert_eq!(
            (r.events_total, r.changes_total),
            (run.events.len() as u64, 2)
        );
        let mut ch = run.changes.clone();
        ch.swap(0, 1);
        let bad = RunData {
            events: &run.events,
            changes: &ch,
            stop: run.stop,
        };
        assert_eq!(
            p.processing_record_len(run.number, &bad).unwrap_err(),
            Error::InconsistentRun {
                reason: "entries out of order"
            }
        );
        let mut ch = run.changes.clone();
        ch[0].stream = rr_dr60::STREAM_PLAYBACK;
        let bad = RunData {
            events: &run.events,
            changes: &ch,
            stop: run.stop,
        };
        assert_eq!(
            p.processing_record_len(run.number, &bad).unwrap_err(),
            Error::InconsistentRun {
                reason: "not the record stream"
            }
        );
    });
}
