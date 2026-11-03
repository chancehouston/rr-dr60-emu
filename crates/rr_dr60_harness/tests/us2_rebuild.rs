//! Spec 027 US2 [TR: D4, D5, D6, FR-004 – FR-006]: a Rebuild with changed settings against the
//! harness recipe with the same delta and a direct recording; its new record's own re-render;
//! the refusals; the stop rule both ways; an edit the input never reaches; the empty case.

use rr_dr60::changes::{ChangeEntry, STREAM_RECORD};
use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::rerender::{Plan, ReRender, Verdict, re_render};
use rr_dr60::{Error, Pipeline, Settings};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

fn at(f: impl Fn(u32)) {
    for &rate in configs::measurement_rates() {
        f(rate);
    }
}

const CHANGES: Cell = Cell {
    codec: true,
    changes: true,
    mute: false,
    self_stop: false,
};

fn rebuilt(plan: Plan, raw: &[i32]) -> (rr_dr60::rerender::Outcome, Vec<u8>) {
    let r = re_render(plan, raw).unwrap();
    let mut bytes = vec![0u8; r.record_len().unwrap()];
    let _ = r.record_write(&mut bytes).unwrap();
    (r.outcome().unwrap(), bytes)
}

fn filter_make(s: Settings) -> Pipeline {
    let mut p = Pipeline::new(s).unwrap();
    p.set_component_parameter(BeforeLimiter, 0, 1.0).unwrap();
    p.set_component_parameter(BeforeLimiter, 1, 700.0).unwrap();
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    p
}

/// AS1 and AS2: each edit equals the harness recipe with the same delta (and, for the knee and
/// the seed, a direct recording at the edited settings); the verdict is `Rebuilt`, not matching;
/// the new record re-renders `Identical`.
#[test]
fn as1_rerender_equals_direct_and_as2_round_trip() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (_, run) = rc::record_run(&standard, rate, CHANGES, &raw);
        let (_, frun) = rc::record_run(&filter_make, rate, CHANGES, &raw);
        let cases: [(&str, &[u8], Delta); 4] = [
            ("knee", &run.record, Delta::Knee),
            ("seed", &run.record, Delta::Seed),
            ("entry", &run.record, Delta::EntryMovedFar),
            ("parameter", &frun.record, Delta::ParameterMoved),
        ];
        for (name, record, delta) in cases {
            let mut plan = Plan::from_record(record).unwrap();
            match delta {
                Delta::Knee => {
                    let mut s = *plan.settings();
                    s.agc.knee_dbfs = -30.0;
                    plan.set_settings(s).unwrap();
                }
                Delta::Seed => plan.set_seed(plan.seed().wrapping_add(1)),
                Delta::EntryMovedFar => {
                    // With the codec on, 0.1 s (one device sample is below a frame's reach).
                    let mut row = plan.rows()[0];
                    row.position += u64::from(rate) / 10;
                    plan.set_row(0, row).unwrap();
                }
                _ => {
                    let f = plan.parameter(1).unwrap();
                    plan.set_parameter(1, f64::from((f * 1.5).min(20_000.0)))
                        .unwrap();
                }
            }
            let (o, new_record) = rebuilt(plan, &raw);
            let tag = format!("{rate} Hz {name}");
            let want = rc::re_render(record, &raw, 1024, delta);
            assert_eq!(o.digest, Some(want), "{tag}: the harness delta");
            assert!(o.edited && !o.matches_original, "{tag}");
            assert_eq!(o.verdict, Verdict::Rebuilt, "{tag}");
            let back = re_render(Plan::from_record(&new_record).unwrap(), &raw).unwrap();
            assert_eq!(
                back.outcome().unwrap().verdict,
                Verdict::Identical,
                "{tag}: round trip"
            );
        }
        // The knee and the seed against a direct recording at the edited settings.
        for name in ["knee", "seed"] {
            let make = move |mut s: Settings| {
                if name == "knee" {
                    s.agc.knee_dbfs = -30.0;
                } else {
                    s.seed = s.seed.wrapping_add(1);
                }
                Pipeline::new(s).unwrap()
            };
            let (_, direct) = rc::record_run(&make, rate, CHANGES, &raw);
            let mut plan = Plan::from_record(&run.record).unwrap();
            if name == "knee" {
                let mut s = *plan.settings();
                s.agc.knee_dbfs = -30.0;
                plan.set_settings(s).unwrap();
            } else {
                plan.set_seed(plan.seed().wrapping_add(1));
            }
            let (o, _) = rebuilt(plan, &raw);
            assert_eq!(o.digest, Some(direct.digest), "{rate} Hz {name}: direct");
        }
    });
}

/// AS3 (and D6): the refusals, each with the engine's own error, nothing built.
#[test]
fn as3_rerender_refusals() {
    let rate = 48_000;
    let raw = rc::raw_stimulus(rate);
    let (_, run) = rc::record_run(&standard, rate, CHANGES, &raw);
    let mut plan = Plan::from_record(&run.record).unwrap();
    let mut s = *plan.settings();
    s.agc.knee_dbfs = 40.0;
    assert!(matches!(
        plan.set_settings(s),
        Err(Error::InvalidSetting { .. })
    ));
    let mut s = *plan.settings();
    s.host_rate_hz = 44_100;
    assert_eq!(plan.set_settings(s), Err(Error::HostRateFixed));
    let row = |control: u32, stream: u32, after: f64, position: u64| {
        ChangeEntry::new(position, control, 0.0, after, 1, stream)
    };
    assert!(matches!(
        plan.insert_row(row(2, STREAM_RECORD, 1.0, 10)),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(matches!(
        plan.insert_row(row(rc::KNEE, 1, -10.0, 10)),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(
        plan.insert_row(row(rc::KNEE, STREAM_RECORD, 99.0, 10))
            .is_err()
    );
    assert!(matches!(
        plan.set_parameter(99, 1.0),
        Err(Error::NoSuchComponent { .. })
    ));
    assert!(matches!(
        plan.set_parameter(1, 5.0),
        Err(Error::InvalidControlValue { .. })
    ));
    // A 65th row at one position.
    let mut p64 = Plan::from_record(&run.record).unwrap();
    for _ in 0..64 {
        p64.insert_row(row(rc::KNEE, STREAM_RECORD, -10.0, 77))
            .unwrap();
    }
    assert!(matches!(
        p64.insert_row(row(rc::KNEE, STREAM_RECORD, -10.0, 77)),
        Err(Error::ScheduleFull { .. })
    ));
    // At `new`: remaining 0, above the capacity; an unknown component; imported origin.
    let mut z = Plan::from_record(&run.record).unwrap();
    z.set_remaining_samples(0);
    assert_eq!(ReRender::new(z).unwrap_err(), Error::MemoryFull);
    let mut big = Plan::from_record(&run.record).unwrap();
    big.set_remaining_samples(u64::MAX / 2);
    assert_eq!(ReRender::new(big).unwrap_err(), Error::DoesNotFit);
    let text = String::from_utf8(run.record.clone()).unwrap();
    let unknown = resign(
        text.replacen(
            "insert.before_limiter = none",
            "insert.before_limiter = spring_reverb",
            1,
        )
        .into_bytes(),
    );
    assert_eq!(
        ReRender::new(Plan::from_record(&unknown).unwrap()).unwrap_err(),
        Error::UnknownComponent
    );
    let mut p = Pipeline::new(Settings::new(rate)).unwrap();
    let n = p.import(&run.export).unwrap();
    let rd = rr_dr60::record::RunData {
        events: &[],
        changes: &[],
        stop: Default::default(),
    };
    let mut imported = vec![0u8; p.processing_record_len(n, &rd).unwrap()];
    let _ = p.processing_record_write(n, &rd, &mut imported).unwrap();
    assert_eq!(
        ReRender::new(Plan::from_record(&imported).unwrap()).unwrap_err(),
        Error::ImportedRecord
    );
}

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

/// AS4: the stop rule both ways: a larger remaining capacity no longer self-stops (the session
/// issues the stop call at the record's stop position); a smaller one self-stops earlier. Each
/// rebuild's record re-renders `Identical`.
#[test]
fn as4_rerender_self_stop_both_ways() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let selfstop = Cell {
            codec: true,
            changes: true,
            mute: false,
            self_stop: true,
        };
        let (_, a) = rc::record_run(&standard, rate, selfstop, &raw);
        let mut larger = Plan::from_record(&a.record).unwrap();
        larger.set_remaining_samples(larger.remaining_samples() + 2400);
        let mut r = ReRender::new(larger).unwrap();
        let f = r.feed(&raw).unwrap();
        assert!(f.reached_stop && !f.self_stopped, "{rate} Hz larger");
        let o = r.finish().unwrap();
        assert_eq!(o.verdict, Verdict::Rebuilt);
        let mut rec = vec![0u8; r.record_len().unwrap()];
        let _ = r.record_write(&mut rec).unwrap();
        let back = re_render(Plan::from_record(&rec).unwrap(), &raw).unwrap();
        assert_eq!(
            back.outcome().unwrap().verdict,
            Verdict::Identical,
            "{rate} Hz larger"
        );

        let (_, b) = rc::record_run(&standard, rate, CHANGES, &raw);
        let mut smaller = Plan::from_record(&b.record).unwrap();
        smaller.set_remaining_samples(6000);
        let mut r = ReRender::new(smaller).unwrap();
        let f = r.feed(&raw).unwrap();
        assert!(f.self_stopped && !f.reached_stop, "{rate} Hz smaller");
        assert!(
            r.feed(&raw).unwrap().taken == 0,
            "nothing after a self-stop"
        );
        let o = r.finish().unwrap();
        assert_eq!(o.verdict, Verdict::Rebuilt);
        let mut rec = vec![0u8; r.record_len().unwrap()];
        let _ = r.record_write(&mut rec).unwrap();
        let back = re_render(Plan::from_record(&rec).unwrap(), &raw).unwrap();
        assert_eq!(
            back.outcome().unwrap().verdict,
            Verdict::Identical,
            "{rate} Hz smaller"
        );
    });
}

/// AS5: an edit the input never reaches (a playback-side setting) rebuilds the same item; a
/// same-value edit is not an edit; −0.0 for 0.0 reads as edited (bits) with the item unchanged.
#[test]
fn as5_rerender_unreached_edit() {
    at(|rate| {
        let raw = rc::raw_stimulus(rate);
        let (_, run) = rc::record_run(&standard, rate, CHANGES, &raw);
        let mut plan = Plan::from_record(&run.record).unwrap();
        let mut s = *plan.settings();
        s.output.clamp_enabled = !s.output.clamp_enabled;
        plan.set_settings(s).unwrap();
        let (o, _) = rebuilt(plan, &raw);
        assert!(o.edited && o.matches_original, "{rate} Hz playback-side");
        assert_eq!(o.verdict, Verdict::Rebuilt);

        let mut same = Plan::from_record(&run.record).unwrap();
        let s = *same.settings();
        same.set_settings(s).unwrap();
        assert!(!same.edited());

        let mut neg = Plan::from_record(&run.record).unwrap();
        let mut s = *neg.settings();
        assert_eq!(s.mic.hiss_offset_db.to_bits(), 0.0f32.to_bits());
        s.mic.hiss_offset_db = -0.0;
        neg.set_settings(s).unwrap();
        assert!(neg.edited(), "−0.0 by bits");
        let (o, _) = rebuilt(neg, &raw);
        assert!(o.matches_original, "{rate} Hz −0.0");
    });
}

/// AS6 (restated): an empty Rebuild cannot occur; a remaining capacity of 0 is refused.
#[test]
fn as6_rerender_empty() {
    let raw = rc::raw_stimulus(6000);
    let (_, run) = rc::record_run(&standard, 6000, CHANGES, &raw);
    let mut plan = Plan::from_record(&run.record).unwrap();
    plan.set_remaining_samples(0);
    assert_eq!(ReRender::new(plan).unwrap_err(), Error::MemoryFull);
}

/// Hardware review Q3: an edited plan's rows past what the engine reaches do not land, and
/// `rows_landed` says so (70 rows past the stop position).
#[test]
fn as4_rerender_rows_past_the_stop() {
    let raw = rc::raw_stimulus(6000);
    let (_, run) = rc::record_run(&standard, 6000, CHANGES, &raw);
    let mut plan = Plan::from_record(&run.record).unwrap();
    let stop = plan.readout().stop.stop_input_position;
    for i in 0..70u64 {
        plan.insert_row(ChangeEntry::new(
            stop + 100_000 + i,
            rc::KNEE,
            0.0,
            -10.0,
            100 + i,
            STREAM_RECORD,
        ))
        .unwrap();
    }
    let total = plan.rows().len() as u64;
    let r = re_render(plan, &raw).unwrap();
    let o = r.outcome().unwrap();
    assert!(o.rows_landed < total, "{} of {total}", o.rows_landed);
    assert_eq!(o.rows_landed, total - 70);
}
