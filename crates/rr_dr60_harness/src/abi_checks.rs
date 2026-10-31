//! Spec 022, the pre-1.0 C ABI pass: the rows the C smoke prints under `smoke --abi` and the
//! parity test compares (research R-07, R-10): the 64-bit size table, the four defines, and one
//! row per struct the library fills through the per-call reports, the status reads and the
//! constant data, from the Rust path. The first figure after each row's name is the struct's
//! size as the C side reads it back (D11), so the diff checks the read-back too.

use rr_dr60::{ControlValue, Pipeline, Route, Settings, State, StopReason, StopResult, VasEvent};
use std::fmt::Write as _;

/// The twenty structs of the C API and their 64-bit sizes (research R-03).
pub const SIZES: [(&str, u32); 20] = [
    ("RrDr60BlockInfo", 40),
    ("RrDr60RecordInfo", 48),
    ("RrDr60StopOutcome", 88),
    ("RrDr60PlayInfo", 48),
    ("RrDr60EngineState", 20),
    ("RrDr60MemoryStatus", 48),
    ("RrDr60SpeechCodecInfo", 28),
    ("RrDr60ManifestHeader", 72),
    ("RrDr60StageEntry", 80),
    ("RrDr60Control", 120),
    ("RrDr60ControlOption", 24),
    ("RrDr60Meters", 64),
    ("RrDr60RecordReadout", 376),
    ("RrDr60InputAlignment", 32),
    ("RrDr60PlaceState", 32),
    ("RrDr60VasEvent", 16),
    ("RrDr60ChangeEntry", 40),
    ("RrDr60StopFields", 40),
    ("RrDr60RunData", 80),
    ("RrDr60Settings", 120),
];

/// The stimulus both sides feed: a ±0.125 square, 24 up and 24 down (exactly representable,
/// spec 006's lesson), `n` samples.
pub fn square(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect()
}

/// The block the rows are read on.
pub const BLOCK: usize = 4096;

/// The settings the rows use: the default at `rate` with the speech codec bypassed (the
/// counts are then host-rate figures a reader can check by hand).
pub fn settings(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s
}

/// The index of the first choice control and its first option (the control-option row).
pub fn first_choice() -> (usize, &'static str, u32) {
    for (i, e) in rr_dr60::controls().iter().enumerate() {
        if let ControlValue::Choice { options, .. } = e.value {
            return (i, options[0].identifier, options[0].value);
        }
    }
    unreachable!("the control manifest has a choice");
}

fn state_word(s: State) -> u32 {
    match s {
        State::Idle => 0,
        State::Recording => 1,
        State::Playing => 2,
        _ => 99,
    }
}

fn stop_reason_word(r: Option<StopReason>) -> u32 {
    match r {
        Some(StopReason::MemoryFull) => 1,
        _ => 0,
    }
}

/// The rows at `rate` from the Rust path (the C smoke prints the same text at 48 kHz).
pub fn rows_at(rate: u32) -> String {
    let mut o = String::new();
    o.push_str("sizes\n");
    for (name, size) in SIZES {
        writeln!(o, "{name} {size}").unwrap();
    }
    writeln!(o, "defines 1 4096 16 40").unwrap();
    let x = square(BLOCK);
    let mut y = vec![0.0f32; BLOCK];
    let mut ev = [VasEvent::default(); 64];
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    let info = p.process_with_events(&x, &mut y, &mut ev).expect("process");
    writeln!(
        o,
        "block_info 40 {} {} {} {}",
        info.produced, info.events, info.paused as u32, info.frames
    )
    .unwrap();
    let m = p.meters();
    writeln!(
        o,
        "meters 64 {:08x} {:08x} {:08x} {:08x} {:08x} {} {} {} {:08x} {:08x} {} {} {:08x}",
        m.input_peak.to_bits(),
        m.limiter_input_peak.to_bits(),
        m.gain_reduction_db.to_bits(),
        m.hold_charge.to_bits(),
        m.record_filter_peak.to_bits(),
        m.adc_clamp_count,
        m.vas_paused as u32,
        m.hang_remaining_samples,
        m.reconstruction_peak.to_bits(),
        m.route_peak.to_bits(),
        m.output_clamp_count,
        m.amp_clip_count,
        m.battery_ceiling.to_bits()
    )
    .unwrap();
    p.start_recording().expect("start");
    let r = p.record(&x, &mut ev).expect("record");
    writeln!(
        o,
        "record_info 48 {} {} {} {} {} {} {}",
        r.stored,
        r.events,
        r.paused as u32,
        r.stopped.is_some() as u32,
        stop_reason_word(r.stopped),
        r.frames,
        r.frames_total
    )
    .unwrap();
    let so = p.stop_recording(&mut ev).expect("stop");
    let (kept, number, length) = match so.result {
        StopResult::Kept { number, length } => (1u32, number, length),
        _ => (0, 0, 0),
    };
    writeln!(
        o,
        "stop_outcome 88 {} {} {} {} {} {} {} {} {} {} {} {} {}",
        so.flush.stored,
        so.flush.events,
        so.flush.paused as u32,
        so.flush.stopped.is_some() as u32,
        stop_reason_word(so.flush.stopped),
        so.flush.frames,
        so.flush.frames_total,
        kept,
        number,
        length,
        so.silence_fed_samples,
        so.stop_input_position,
        so.padding_start_sample
    )
    .unwrap();
    p.start_playback(1).expect("start_playback");
    let pi = p.play(&mut y).expect("play");
    writeln!(
        o,
        "play_info 48 {} {} {} {} {}",
        pi.produced, pi.finished as u32, pi.frames, pi.frame_index, pi.place
    )
    .unwrap();
    let st = p.state();
    writeln!(
        o,
        "engine_state 20 {} {} {} {}",
        state_word(st.state),
        match st.route {
            Route::Earphone => 1,
            _ => 0,
        },
        st.current.unwrap_or(0),
        st.speaker_muted as u32
    )
    .unwrap();
    let ms = p.memory_status();
    writeln!(
        o,
        "memory_status 48 {} {} {} {} {} {}",
        ms.capacity_samples,
        ms.used_samples,
        ms.remaining_samples,
        ms.max_recordings,
        ms.count,
        ms.slots_remaining
    )
    .unwrap();
    let _ = p.stop_playback();
    let ci = rr_dr60::speech_codec::info();
    writeln!(
        o,
        "speech_codec_info 28 {} {} {} {} {}.{}.{}",
        ci.frame_samples,
        ci.frame_ms,
        ci.bits_per_frame,
        ci.delay_samples,
        ci.model_version[0],
        ci.model_version[1],
        ci.model_version[2]
    )
    .unwrap();
    let h = &rr_dr60::manifest().header;
    writeln!(
        o,
        "manifest_header 72 {} {} {} {} {} {} {}",
        h.library_version,
        h.device_rate_hz,
        h.stage_count,
        h.counts.not_modeled,
        h.counts.assumed,
        h.counts.sourced,
        h.counts.checked
    )
    .unwrap();
    let e = &rr_dr60::manifest().stages[2];
    writeln!(
        o,
        "stage_entry 80 {} {} {} {} {} {} {}",
        e.number,
        e.identifier,
        e.status as u32,
        e.path as u32,
        e.confidence as u32,
        e.sources.len(),
        e.assumptions.len()
    )
    .unwrap();
    let c = &rr_dr60::controls()[0];
    let (
        value_type,
        int_min,
        int_max,
        int_default,
        fmin,
        fmax,
        fdef,
        option_count,
        choice_default,
        has_default,
    ) = match c.value {
        ControlValue::Switch { default } => (
            0u32,
            0u64,
            1u64,
            u64::from(default),
            0.0f32,
            0.0f32,
            0.0f32,
            0u32,
            0u32,
            1u32,
        ),
        ControlValue::Integer { min, max, default } => {
            (1, min, max, default, 0.0, 0.0, 0.0, 0, 0, 1)
        }
        ControlValue::Continuous { min, max, default } => (2, 0, 0, 0, min, max, default, 0, 0, 1),
        ControlValue::Choice { options, default } => (
            3,
            0,
            0,
            0,
            0.0,
            0.0,
            0.0,
            options.len() as u32,
            default.unwrap_or(0),
            u32::from(default.is_some()),
        ),
        _ => unreachable!(),
    };
    writeln!(
        o,
        "control 120 0 {} {} {} {} {} {} {} {} {} {:08x} {:08x} {:08x} {} {} {} {} {}",
        c.identifier,
        c.kind as u32,
        c.timing as u32,
        u32::from(c.paths.bits()),
        c.stage,
        value_type,
        int_min,
        int_max,
        int_default,
        fmin.to_bits(),
        fmax.to_bits(),
        fdef.to_bits(),
        option_count,
        choice_default,
        has_default,
        c.sources.len(),
        c.assumptions.len()
    )
    .unwrap();
    let (ci_idx, oid, oval) = first_choice();
    writeln!(o, "control_option 24 {ci_idx} 0 {oid} {oval}").unwrap();
    o
}
