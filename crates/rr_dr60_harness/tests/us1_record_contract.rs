//! Spec 020 US1 (AS1 – AS5): the record written from the slot block and re-rendered at the
//! measurement rates; the named effective changes and the playback twins; the round trip;
//! the dirty-engine counter identity.

use rr_dr60::record::{self, RecordReadout, RunData, flags};
use rr_dr60::{ChangeEntry, Pipeline};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

fn at(f: impl Fn(u32)) {
    for &rate in configs::measurement_rates() {
        f(rate);
    }
}

/// AS1: the record's configuration is the start's (after a reconfigure and a landed change);
/// the digest equals the export's; the rows carry the harness's host positions; rewriting
/// gives the same bytes; the totals match the collection (no incomplete flag).
#[test]
fn as1_record_written_from_the_block() {
    at(|rate| {
        let cell = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let raw = rc::raw_stimulus(rate);
        let (mut p, run) = rc::record_run(&standard, rate, cell, &raw);
        let tag = format!("{rate} Hz");
        assert_eq!(
            run.digest,
            rr_dr60::digest::fnv1a64(&run.export),
            "{tag}: the digest"
        );
        let r = rc::readout(&run.record);
        assert_eq!(r.flags, 0, "{tag}: no flag");
        assert_eq!(
            r.settings,
            rc::settings(cell, rate),
            "{tag}: the start configuration"
        );
        assert_eq!(r.recording_counter, run.counter);
        assert_eq!(
            (r.event_count, r.change_count),
            (run.events.len() as u64, 2)
        );
        assert_eq!(
            (r.events_total, r.changes_total),
            (run.events.len() as u64, 2)
        );
        assert!(!run.events.is_empty(), "{tag}: the gaps paused the gate");
        assert!(!run.flush_paused, "{tag}: the gate was open at the stop");
        // The rows' host positions by the harness's own rule.
        let want = rc::event_host_positions(
            &run.events,
            rate,
            p.record_latency_pre_encoder_samples(),
            true,
        );
        for (i, w) in want.iter().enumerate() {
            let row = record::record_event(&run.record, i as u64).unwrap();
            assert_eq!(row.event, run.events[i]);
            assert_eq!(row.host_input_position, *w, "{tag}: event {i}");
        }
        // The engine's settings moved with the landed changes; a reconfigure to another rate
        // keeps the memory and the block, and the record is byte-identical.
        assert_eq!(p.settings().agc.knee_dbfs, -30.0, "{tag}: the landed knee");
        let mut s2 = *p.settings();
        s2.host_rate_hz = if rate == 48_000 { 44_100 } else { 48_000 };
        p.reconfigure(s2).unwrap();
        let run_data = RunData {
            events: &run.events,
            changes: &run.changes,
            stop: run.stop,
        };
        let mut again = vec![0u8; p.processing_record_len(run.number, &run_data).unwrap()];
        p.processing_record_write(run.number, &run_data, &mut again)
            .unwrap();
        assert_eq!(again, run.record, "{tag}: the record after a reconfigure");
        let mut r2 = RecordReadout::default();
        p.processing_record_read(&run.record, &mut r2).unwrap();
        assert_eq!(
            r2.flags,
            flags::HOST_RATE_DIFFERS,
            "{tag}: the engine's rate moved"
        );
    });
}

/// AS2: the re-render reproduces the digest in every cell.
#[test]
fn as2_re_render_every_cell() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        for cell in Cell::all() {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let got = rc::re_render(&run.record, &raw, 1024, Delta::None);
            assert_eq!(got, run.digest, "{rate} Hz {}", cell.name());
        }
    });
}

/// AS3: each named effective change fails the digest; each playback twin leaves it; a
/// filler made by recording fails.
#[test]
fn as3_effective_deltas_fail_and_twins_hold() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        for cell in [
            Cell {
                codec: false,
                changes: true,
                mute: false,
                self_stop: false,
            },
            Cell {
                codec: true,
                changes: true,
                mute: false,
                self_stop: false,
            },
        ] {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            for d in Delta::effective(cell.codec) {
                let got = rc::re_render(&run.record, &raw, 1024, d);
                assert_ne!(got, run.digest, "{rate} Hz {} {d:?} must fail", cell.name());
            }
            for d in Delta::TWINS {
                let got = rc::re_render(&run.record, &raw, 1024, d);
                assert_eq!(got, run.digest, "{rate} Hz {} {d:?} must hold", cell.name());
            }
        }
        let cell = Cell {
            codec: true,
            changes: false,
            mute: false,
            self_stop: true,
        };
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        assert_ne!(
            rc::re_render(&run.record, &raw, 1024, Delta::Capacity),
            run.digest,
            "{rate} Hz capacity"
        );
        assert_ne!(
            rc::re_render(&run.record, &raw, 1024, Delta::RecordedFiller),
            run.digest,
            "{rate} Hz recorded filler"
        );
        assert_eq!(
            rc::re_render(&run.record, &raw, 1024, Delta::None),
            run.digest
        );
    });
}

/// AS4: write → read → rewrite is byte-identical, and the read-out's fields equal the run's.
#[test]
fn as4_round_trip() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell {
            codec: false,
            changes: true,
            mute: true,
            self_stop: false,
        };
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        let r = rc::readout(&run.record);
        assert_eq!(r.stop, run.stop);
        assert_eq!(r.recording_digest, run.digest);
        let rows: Vec<_> = (0..r.event_count)
            .map(|i| record::record_event(&run.record, i).unwrap())
            .collect();
        let mut changes = vec![ChangeEntry::default(); r.change_count as usize];
        assert_eq!(
            record::record_changes(&run.record, 0, &mut changes),
            changes.len()
        );
        assert_eq!(changes, run.changes);
        let mut again = vec![0u8; record::len(&r, &rows, &changes)];
        record::write(&r, &rows, &changes, &mut again).unwrap();
        assert_eq!(again, run.record, "{rate} Hz");
    });
}

/// AS5: the dirty-engine counter identity (the one-pass after the original's reset).
#[test]
fn as5_dirty_engine_counter_identity() {
    at(|rate| {
        assert_eq!(
            rc::dirty_engine_identity(&standard, rate, 3),
            0,
            "{rate} Hz"
        );
        let p = Pipeline::new_at_counter(rr_dr60::Settings::new(rate), 7).unwrap();
        assert_eq!(p.recording_counter(), 7);
    });
}
