//! Spec 027 US1 [TR: D1, D2, D5, FR-001, FR-002, FR-005]: the public re-render against the
//! harness's independent recipe and the recording's own digest and record, at the measurement
//! rates.

use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::rerender::{Plan, ReRender, Verdict, re_render};
use rr_dr60::{Pipeline, Settings};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

fn at(f: impl Fn(u32)) {
    for &rate in configs::measurement_rates() {
        f(rate);
    }
}

/// The session's digest, outcome and new record for `record` fed `raw` in `blocks`.
fn session(record: &[u8], raw: &[i32], blocks: &[usize]) -> (rr_dr60::rerender::Outcome, Vec<u8>) {
    let mut r = ReRender::new(Plan::from_record(record).unwrap()).unwrap();
    let mut pos = 0;
    let mut k = 0;
    while pos < raw.len() {
        let n = blocks[k % blocks.len()].min(raw.len() - pos);
        let f = r.feed(&raw[pos..pos + n]).unwrap();
        pos += n;
        k += 1;
        if f.reached_stop || f.self_stopped {
            break;
        }
    }
    let o = r.finish().unwrap();
    let mut bytes = vec![0u8; r.record_len().unwrap()];
    let n = r.record_write(&mut bytes).unwrap();
    assert_eq!(n, bytes.len());
    (o, bytes)
}

/// AS1: every cell re-renders to the recording's digest, as the harness recipe does.
#[test]
fn as1_rerender_sixteen_cells() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        for cell in Cell::all() {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let oracle = rc::re_render(&run.record, &raw, 1024, Delta::None);
            assert_eq!(oracle, run.digest, "the oracle");
            let (o, _) = session(&run.record, &raw, &[4096]);
            let tag = format!("{rate} Hz {}", cell.name());
            assert_eq!(o.digest, Some(run.digest), "{tag}");
            assert!(o.matches_original && !o.edited, "{tag}");
            assert_eq!(o.verdict, Verdict::Identical, "{tag}");
            assert_eq!(
                o.rows_landed,
                run.changes.len() as u64,
                "{tag}: every row landed"
            );
        }
    });
}

/// AS2: the slot cells (the band filter; a non-default store with an empty slot) and a form-1
/// record (flagged, an empty slot).
#[test]
fn as2_rerender_slot_and_form_1() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        for (component, freq) in [
            (ComponentId::BandFilter, 700.0),
            (ComponentId::None, 1500.0),
        ] {
            let make = move |s: Settings| {
                let mut p = Pipeline::new(s).unwrap();
                p.set_component_parameter(BeforeLimiter, 0, 1.0).unwrap(); // high-pass
                p.set_component_parameter(BeforeLimiter, 1, freq).unwrap();
                p.set_component_parameter(BeforeLimiter, 2, 2.0).unwrap();
                p.set_component(BeforeLimiter, component).unwrap();
                p
            };
            let (_, run) = rc::record_run(&make, rate, cell, &raw);
            let (o, bytes) = session(&run.record, &raw, &[4096]);
            assert_eq!(o.verdict, Verdict::Identical, "{rate} Hz {component:?}");
            assert_eq!(bytes, run.record, "{rate} Hz {component:?}: the record");
        }
        // Form 1: the slot lines removed and the version line set to 1.
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        let mut text = String::from_utf8(run.record.clone()).unwrap();
        text = text.replacen("version = 2", "version = 1", 1);
        let start = text.find("insert.before_limiter").unwrap();
        let end = text.find("run.remaining_samples_at_start").unwrap();
        text.replace_range(start..end, "");
        let v1 = resign(text.into_bytes());
        let plan = Plan::from_record(&v1).unwrap();
        assert_eq!(plan.readout().form_version, 1);
        let r = re_render(plan, &raw).unwrap();
        let o = r.outcome().unwrap();
        assert!(
            o.matches_original,
            "{rate} Hz form 1: an empty slot at the defaults"
        );
    });
}

/// Re-signs an edited record (spec 020's checksum line).
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

/// AS3: every partition gives the same digest and the same new record.
#[test]
fn as3_rerender_partitions() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        for cell in [
            Cell {
                codec: true,
                changes: true,
                mute: false,
                self_stop: false,
            },
            Cell {
                codec: false,
                changes: true,
                mute: true,
                self_stop: true,
            },
        ] {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let want = session(&run.record, &raw, &[4096]);
            for blocks in [&[1usize][..], &[64], &[7, 4097, 333, 1, 9000]] {
                if blocks == [1] && rate > 16_000 {
                    continue; // block size 1 at the low rates (the time budget)
                }
                assert_eq!(
                    session(&run.record, &raw, blocks),
                    want,
                    "{rate} Hz {blocks:?}"
                );
            }
        }
    });
}

/// AS4: a burst in the input, or the stop moved one frame earlier in the record: `Differs`.
#[test]
fn as4_rerender_differs() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let cell = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let (_, run) = rc::record_run(&standard, rate, cell, &raw);
        // A 10 ms, 1 kHz burst at −20 dBFS inside the first voiced segment (spec 020's delta).
        let mut burst = raw.clone();
        let at = rate as usize / 4;
        let tone =
            rr_dr60_harness::stimulus::tone(1000.0, 0.1, f64::from(rate), rate as usize / 100);
        for (v, t) in burst[at..].iter_mut().zip(&tone) {
            *v = v.saturating_add((f64::from(*t) * 8_388_608.0) as i32);
        }
        let (o, _) = session(&run.record, &burst, &[4096]);
        assert_eq!(o.verdict, Verdict::Differs, "{rate} Hz burst");
        let text = String::from_utf8(run.record.clone()).unwrap();
        let key = "run.stop_input_position = ";
        let i = text.find(key).unwrap() + key.len();
        let j = i + text[i..].find('\n').unwrap();
        let stop: u64 = text[i..j].parse().unwrap();
        let earlier = stop - 120 * u64::from(rate) / 6000;
        let moved = resign(format!("{}{earlier}{}", &text[..i], &text[j..]).into_bytes());
        let (o, _) = session(&moved, &raw, &[4096]);
        assert_eq!(o.verdict, Verdict::Differs, "{rate} Hz stop earlier");
    });
}

/// AS5 (hardware B1): 80 rows inside the first 4096-sample block: 64 early (from input sample
/// 50, one apart) and the last 16 inside the first voiced segment, each held longer than the
/// limiter's dead time (from 1500, 3.1 ms apart, at least 75 samples). The session reads
/// `Identical` at 4096 and at 1; the harness recipe at 4096 schedules the last 16 late and
/// does not (the test's teeth); at 1 it does.
#[test]
fn as5_rerender_dense_rows() {
    for &rate in &[6000u32, 48_000] {
        let raw = rc::raw_stimulus(rate);
        let step = 75u64.max(u64::from(rate) / 320);
        let positions: Vec<u64> = (0..64)
            .map(|i| 50 + i)
            .chain((0..16).map(|i| 1500 + i * step))
            .collect();
        assert!(*positions.last().unwrap() < 4096);
        let (record, digest) = rc::dense_recording(rate, &raw, &positions);
        assert_eq!(
            rc::re_render(&record, &raw, 1, Delta::None),
            digest,
            "the oracle at 1"
        );
        assert_ne!(
            rc::re_render(&record, &raw, 4096, Delta::None),
            digest,
            "{rate} Hz: the harness at 4096 schedules late (the teeth)"
        );
        for blocks in [&[4096usize][..], &[1], &[100_000]] {
            if blocks == [1] && rate > 6000 {
                continue;
            }
            let (o, bytes) = session(&record, &raw, blocks);
            assert_eq!(o.verdict, Verdict::Identical, "{rate} Hz {blocks:?}");
            assert_eq!(o.rows_landed, 80);
            assert_eq!(bytes, record, "{rate} Hz {blocks:?}: the record");
        }
    }
}

/// AS6 (coach 1): an unedited plan's new record equals the original byte for byte (the event
/// and change rows, which the digest does not cover), on every cell.
#[test]
fn as6_rerender_record_bytes() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        for cell in Cell::all() {
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let (_, bytes) = session(&run.record, &raw, &[4096]);
            assert_eq!(bytes, run.record, "{rate} Hz {}", cell.name());
        }
    });
}
