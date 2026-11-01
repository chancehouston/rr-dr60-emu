//! Checks for the control manifest (spec 014): each a function over a table of entries, so a
//! doctored copy can be passed in (014 research R-03; SC-003). Each returns its findings, one
//! string per problem naming the entry; an empty list is a pass.

use std::fmt::Write as _;
use std::path::Path;

use regex::Regex;
use rr_dr60::{
    ControlEntry, ControlKind, ControlTiming, ControlValue, Manifest, Pipeline, Route,
    SUPPORTED_HOST_RATES, Setting, Settings, VasEvent, VasMode,
};

use crate::manifest_checks::denylist_hit;

/// The three live controls (014 FR-001), in order.
pub const LIVE: [&str; 5] = [
    "route",
    "volume",
    "playback_speed",
    "battery_volts",
    "playback_speed_mode",
]; // 018: a fourth; 023: a fifth

// ---- Canonical text (014 FR-012; research R-07) ----------------------------------------------

/// The kind as the canonical text writes it.
pub fn kind_word(k: ControlKind) -> &'static str {
    match k {
        ControlKind::DeviceControl => "device control",
        ControlKind::UnitCondition => "unit condition",
        ControlKind::ModelInternal => "model internal",
        ControlKind::EmulatorOption => "emulator option",
        ControlKind::HostParameter => "host parameter",
        _ => "unknown",
    }
}

/// The timing class as the canonical text writes it.
pub fn timing_word(t: ControlTiming) -> &'static str {
    match t {
        ControlTiming::Live => "live",
        ControlTiming::Idle => "idle",
        ControlTiming::IdleMemoryDependent => "idle, memory-dependent",
        ControlTiming::HostFixed => "host-fixed",
        _ => "unknown",
    }
}

fn paths_words(e: &ControlEntry) -> String {
    let mut words = Vec::new();
    for (bit, word) in [(1, "record"), (2, "playback"), (4, "one-pass")] {
        if e.paths.bits() & bit != 0 {
            words.push(word);
        }
    }
    words.join(", ")
}

/// The canonical text of the control manifest: every field of every entry, one per line, f32
/// values in Rust's shortest round-trip form (014 FR-012). The C smoke program prints the same
/// bytes (`smoke --controls`).
pub fn render_controls(entries: &[ControlEntry]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "control_count: {}", entries.len());
    for (i, e) in entries.iter().enumerate() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[{i}] {}", e.identifier);
        let _ = writeln!(out, "name: {}", e.name);
        let _ = writeln!(out, "kind: {}", kind_word(e.kind));
        let _ = writeln!(out, "timing: {}", timing_word(e.timing));
        let _ = writeln!(out, "paths: {}", paths_words(e));
        let _ = writeln!(out, "stage: {}", e.stage);
        match e.value {
            ControlValue::Switch { default } => {
                let _ = writeln!(out, "value: switch");
                let _ = writeln!(out, "default: {}", if default { "on" } else { "off" });
            }
            ControlValue::Integer { min, max, default } => {
                let _ = writeln!(out, "value: integer");
                let _ = writeln!(out, "min: {min}\nmax: {max}\nstep: 1\ndefault: {default}");
            }
            ControlValue::Continuous { min, max, default } => {
                let _ = writeln!(out, "value: continuous");
                let _ = writeln!(out, "min: {min}\nmax: {max}\nstep: 0\ndefault: {default}");
            }
            ControlValue::Choice { options, default } => {
                let _ = writeln!(out, "value: choice");
                let list: Vec<String> = options
                    .iter()
                    .map(|o| format!("{}={}", o.identifier, o.value))
                    .collect();
                let _ = writeln!(out, "options: {}", list.join(", "));
                match default {
                    Some(d) => {
                        let _ = writeln!(out, "default: {d}");
                    }
                    None => {
                        let _ = writeln!(out, "default: none");
                    }
                }
            }
            _ => {
                let _ = writeln!(out, "value: unknown");
            }
        }
        let _ = writeln!(out, "unit: {}", e.unit);
        let _ = writeln!(out, "sources: {}", e.sources.join(", "));
        let _ = writeln!(out, "assumptions: {}", e.assumptions.join(", "));
        let _ = writeln!(out, "description: {}", e.description);
    }
    out
}

/// The committed canonical text.
pub fn golden_controls_path() -> std::path::PathBuf {
    crate::manifest_checks::repo_root()
        .join("crates/rr_dr60_harness/golden/control-manifest-v1.txt")
}

// ---- Engine values by identifier -------------------------------------------------------------

/// A value an entry can take, for the edge and default checks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val {
    /// A switch.
    B(bool),
    /// An integer.
    I(u64),
    /// A continuous value.
    F(f32),
    /// A choice's value, or `None` for "no default".
    C(Option<u32>),
}

/// The engine's default for an entry (014 FR-005), from `s` and the live defaults.
pub fn engine_default(id: &str, s: &Settings) -> Option<Val> {
    let route = |r: Route| match r {
        Route::Speaker => 0,
        Route::Earphone => 1,
        _ => u32::MAX,
    };
    let mode = |m: VasMode| match m {
        VasMode::Drop => 0,
        VasMode::Mute => 1,
        _ => u32::MAX,
    };
    Some(match id {
        "route" => Val::C(Some(route(Route::default()))),
        "volume" => Val::F(rr_dr60::output::VOLUME_REFERENCE),
        "playback_speed" => Val::I(u64::from(rr_dr60::speed::DEFAULT_LEVEL)),
        "battery_volts" => Val::F(rr_dr60::battery::DEFAULT_BATTERY_VOLTS), // spec 018
        "playback_speed_mode" => Val::C(Some(rr_dr60::speed::SpeedMode::default() as u32)), // spec 023
        "host_rate_hz" => Val::C(None),
        "record_stage_enabled" => Val::B(s.record_stage_enabled),
        "playback_stage_enabled" => Val::B(s.playback_stage_enabled),
        "adc_clamp_enabled" => Val::B(s.adc_clamp_enabled),
        "tap" => Val::C(Some(match s.tap {
            rr_dr60::Tap::RouteOutput => 5,
            _ => u32::MAX,
        })),
        "seed" => Val::I(s.seed),
        "agc.enabled" => Val::B(s.agc.enabled),
        "agc.knee_dbfs" => Val::F(s.agc.knee_dbfs),
        "agc.attack_ms" => Val::F(s.agc.attack_ms),
        "agc.release_ms" => Val::F(s.agc.release_ms),
        "vas.enabled" => Val::B(s.vas.enabled),
        "vas.mode" => Val::C(Some(mode(s.vas.mode))),
        "vas.sensitivity" => Val::I(u64::from(s.vas.sensitivity)),
        "vas.threshold_dbfs" => Val::F(s.vas.threshold_dbfs),
        "vas.hang_ms" => Val::F(s.vas.hang_ms),
        "vas.onset_ms" => Val::F(s.vas.onset_ms),
        "memory.capacity_s" => Val::I(u64::from(s.memory.capacity_s)),
        "memory.max_recordings" => Val::I(u64::from(s.memory.max_recordings)),
        "speech_codec_enabled" => Val::B(s.speech_codec_enabled),
        "output.volume_amp_enabled" => Val::B(s.output.volume_amp_enabled),
        "output.speaker_enabled" => Val::B(s.output.speaker_enabled),
        "output.earphone_enabled" => Val::B(s.output.earphone_enabled),
        "output.clamp_enabled" => Val::B(s.output.clamp_enabled),
        "mic.capsule_enabled" => Val::B(s.mic.capsule_enabled),
        "mic.amp_enabled" => Val::B(s.mic.amp_enabled),
        "mic.hiss_offset_db" => Val::F(s.mic.hiss_offset_db),
        _ => return None,
    })
}

/// Writes `v` into the setting `id` of `s`; `false` when `id` is not a setting this can set
/// (live controls, choices other than the host rate, the seed).
pub fn set_setting(s: &mut Settings, id: &str, v: Val) -> bool {
    match (id, v) {
        ("host_rate_hz", Val::I(x)) => s.host_rate_hz = x as u32,
        ("record_stage_enabled", Val::B(b)) => s.record_stage_enabled = b,
        ("playback_stage_enabled", Val::B(b)) => s.playback_stage_enabled = b,
        ("adc_clamp_enabled", Val::B(b)) => s.adc_clamp_enabled = b,
        ("seed", Val::I(x)) => s.seed = x,
        ("agc.enabled", Val::B(b)) => s.agc.enabled = b,
        ("agc.knee_dbfs", Val::F(x)) => s.agc.knee_dbfs = x,
        ("agc.attack_ms", Val::F(x)) => s.agc.attack_ms = x,
        ("agc.release_ms", Val::F(x)) => s.agc.release_ms = x,
        ("vas.enabled", Val::B(b)) => s.vas.enabled = b,
        ("vas.mode", Val::C(Some(m))) => {
            s.vas.mode = if m == 0 { VasMode::Drop } else { VasMode::Mute }
        }
        ("vas.sensitivity", Val::I(x)) => s.vas.sensitivity = x.min(255) as u8,
        ("vas.threshold_dbfs", Val::F(x)) => s.vas.threshold_dbfs = x,
        ("vas.hang_ms", Val::F(x)) => s.vas.hang_ms = x,
        ("vas.onset_ms", Val::F(x)) => s.vas.onset_ms = x,
        ("memory.capacity_s", Val::I(x)) => s.memory.capacity_s = x as u32,
        ("memory.max_recordings", Val::I(x)) => s.memory.max_recordings = x as u32,
        ("speech_codec_enabled", Val::B(b)) => s.speech_codec_enabled = b,
        ("output.volume_amp_enabled", Val::B(b)) => s.output.volume_amp_enabled = b,
        ("output.speaker_enabled", Val::B(b)) => s.output.speaker_enabled = b,
        ("output.earphone_enabled", Val::B(b)) => s.output.earphone_enabled = b,
        ("output.clamp_enabled", Val::B(b)) => s.output.clamp_enabled = b,
        ("mic.capsule_enabled", Val::B(b)) => s.mic.capsule_enabled = b,
        ("mic.amp_enabled", Val::B(b)) => s.mic.amp_enabled = b,
        ("mic.hiss_offset_db", Val::F(x)) => s.mic.hiss_offset_db = x,
        ("tap", Val::C(Some(5))) => s.tap = rr_dr60::Tap::RouteOutput,
        ("tap", Val::C(Some(_))) => s.tap = rr_dr60::Tap::AfterPlayback,
        _ => return false,
    }
    true
}

fn table_default(v: ControlValue) -> Option<Val> {
    Some(match v {
        ControlValue::Switch { default } => Val::B(default),
        ControlValue::Integer { default, .. } => Val::I(default),
        ControlValue::Continuous { default, .. } => Val::F(default),
        ControlValue::Choice { default, .. } => Val::C(default),
        _ => return None,
    })
}

/// 014 FR-005: every default equals the engine's at every supported host rate.
pub fn defaults(entries: &[ControlEntry]) -> Vec<String> {
    let mut f = Vec::new();
    for rate in SUPPORTED_HOST_RATES {
        let s = Settings::new(rate);
        for e in entries {
            let want = engine_default(e.identifier, &s);
            let got = table_default(e.value);
            if want.is_none() || want != got {
                f.push(format!(
                    "{}: default {got:?} differs from the engine's {want:?} at {rate} Hz",
                    e.identifier
                ));
            }
        }
    }
    f.dedup_by(|a, b| a.split(':').next() == b.split(':').next());
    f
}

// ---- Bounds (014 FR-008; research R-03) ------------------------------------------------------

/// The next f32 above `x` (at 0.0 the smallest positive subnormal).
pub fn next_up(x: f32) -> f32 {
    if x == 0.0 {
        f32::from_bits(1)
    } else if x > 0.0 {
        f32::from_bits(x.to_bits() + 1)
    } else {
        f32::from_bits(x.to_bits() - 1)
    }
}

/// The next f32 below `x` (at 0.0 the smallest negative subnormal; `-0.0` equals `0.0`).
pub fn next_down(x: f32) -> f32 {
    if x == 0.0 {
        f32::from_bits(0x8000_0001)
    } else if x > 0.0 {
        f32::from_bits(x.to_bits() - 1)
    } else {
        f32::from_bits(x.to_bits() + 1)
    }
}

fn settings_accept(id: &str, v: Val) -> Option<bool> {
    let mut s = Settings::new(48_000);
    if !set_setting(&mut s, id, v) {
        return None;
    }
    Some(s.validate().is_ok())
}

fn live_accepts(p: &mut Pipeline, id: &str, v: Val) -> Option<bool> {
    match (id, v) {
        ("volume", Val::F(x)) => Some(p.set_volume(x).is_ok()),
        ("playback_speed", Val::I(x)) => Some(p.set_playback_speed(x.min(255) as u8).is_ok()),
        // Spec 018: the battery value, a plain setter like the volume (output clock).
        ("battery_volts", Val::F(x)) => Some(p.set_battery_volts(x).is_ok()),
        // Spec 023: the mode, a plain setter taking the enum (the C path maps 0/1).
        ("playback_speed_mode", Val::C(Some(m))) => {
            Some(match rr_dr60::speed::SpeedMode::from_u32(m) {
                Some(mode) => {
                    p.set_playback_speed_mode(mode);
                    true
                }
                None => false,
            })
        }
        // Spec 017: the eight record-side values change live through the schedule only, on the
        // record stream while recording and on the one-pass otherwise (there is no plain
        // setter); the value is validated as the setting.
        (_, Val::F(x)) if is_record_side(id) => Some(schedule_record_side(p, id, f64::from(x))),
        (_, Val::I(x)) if is_record_side(id) => Some(schedule_record_side(p, id, x as f64)),
        // Spec 021: the four playback-stage switches change live through the schedule only, on
        // the playback stream (kept by `start_playback`; stage 10 is playback-only, D6).
        (_, Val::B(b)) if is_stage_switch(id) => Some(schedule_stage_switch(p, id, b)),
        _ => None,
    }
}

/// Spec 021 FR-001: the four playback-stage switches (live through the schedule only).
pub fn is_stage_switch(id: &str) -> bool {
    matches!(
        id,
        "playback_stage_enabled"
            | "output.volume_amp_enabled"
            | "output.speaker_enabled"
            | "output.earphone_enabled"
    )
}

/// Schedules the switch `id` to `on` at the playback stream's position and returns whether the
/// call was accepted (an all-on default engine runs every stage).
fn schedule_stage_switch(p: &mut Pipeline, id: &str, on: bool) -> bool {
    let control = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == id)
        .expect("a control") as u32;
    let position = p
        .stream_position(rr_dr60::Stream::Playback)
        .expect("the output clock");
    p.schedule_change(
        rr_dr60::Stream::Playback,
        position,
        control,
        if on { 1.0 } else { 0.0 },
        1,
    )
    .is_ok()
}

/// Spec 017 FR-001: the eight record-side values.
pub fn is_record_side(id: &str) -> bool {
    matches!(
        id,
        "vas.sensitivity"
            | "vas.threshold_dbfs"
            | "vas.hang_ms"
            | "vas.onset_ms"
            | "agc.knee_dbfs"
            | "agc.attack_ms"
            | "agc.release_ms"
            | "mic.hiss_offset_db"
    )
}

/// Schedules `id` at the next input position of the active stream (the record stream while
/// recording, the one-pass otherwise) and returns whether the call was accepted.
fn schedule_record_side(p: &mut Pipeline, id: &str, value: f64) -> bool {
    let control = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == id)
        .expect("a control") as u32;
    let stream = if p.state().state == rr_dr60::State::Recording {
        rr_dr60::Stream::Record
    } else {
        rr_dr60::Stream::OnePass
    };
    let position = p.input_position(stream).expect("an input clock");
    p.schedule_change(stream, position, control, value, 1)
        .is_ok()
}

/// 014 FR-008: each bound is accepted and the nearest value outside is refused, by
/// [`Settings::validate`] for settings and by the setter for live controls. Choices other
/// than the host rate, switches and the seed have no outside in Rust (the C API checks the
/// choices, US3).
pub fn edges(entries: &[ControlEntry]) -> Vec<String> {
    let mut f = Vec::new();
    let mut p = Pipeline::new(Settings::new(6000)).expect("default pipeline");
    let mut check = |id: &str, v: Val, want: bool, f: &mut Vec<String>| {
        let got = live_accepts(&mut p, id, v).or_else(|| settings_accept(id, v));
        match got {
            Some(g) if g == want => {}
            Some(g) => f.push(format!(
                "{id}: {v:?} {} but the table says it should {}",
                if g { "is accepted" } else { "is refused" },
                if want { "be accepted" } else { "be refused" }
            )),
            None => f.push(format!("{id}: no way to offer {v:?}")),
        }
    };
    for e in entries {
        let id = e.identifier;
        match e.value {
            ControlValue::Integer { min, max, .. } if id != "seed" => {
                check(id, Val::I(min), true, &mut f);
                check(id, Val::I(max), true, &mut f);
                if min > 0 {
                    check(id, Val::I(min - 1), false, &mut f);
                }
                check(id, Val::I(max + 1), false, &mut f);
            }
            ControlValue::Continuous { min, max, .. } => {
                check(id, Val::F(min), true, &mut f);
                check(id, Val::F(max), true, &mut f);
                check(id, Val::F(next_down(min)), false, &mut f);
                check(id, Val::F(next_up(max)), false, &mut f);
            }
            ControlValue::Choice { options, .. } if id == "host_rate_hz" => {
                for o in options {
                    check(id, Val::I(u64::from(o.value)), true, &mut f);
                    check(id, Val::I(u64::from(o.value) - 1), false, &mut f);
                    check(id, Val::I(u64::from(o.value) + 1), false, &mut f);
                }
                check(id, Val::I(0), false, &mut f);
            }
            _ => {}
        }
    }
    f
}

/// 014 FR-016: the unit equals the engine's own unit where the engine names one (the
/// [`Setting`] display text).
pub fn units(entries: &[ControlEntry]) -> Vec<String> {
    let settings = [
        Setting::AgcKneeDbfs,
        Setting::AgcAttackMs,
        Setting::AgcReleaseMs,
        Setting::VasSensitivity,
        Setting::VasThresholdDbfs,
        Setting::VasHangMs,
        Setting::VasOnsetMs,
        Setting::MemoryCapacityS,
        Setting::MemoryMaxRecordings,
        Setting::MicHissOffsetDb,
    ];
    let mut f = Vec::new();
    for setting in settings {
        let text = setting.to_string(); // "name (valid range lo to hi[ unit])"
        let name = text.split(' ').next().unwrap_or_default();
        let inner = text
            .rsplit('(')
            .next()
            .unwrap_or_default()
            .trim_end_matches(')');
        let last = inner.split(' ').next_back().unwrap_or_default();
        let unit = if last.parse::<f64>().is_ok() {
            ""
        } else {
            last
        };
        match entries.iter().find(|e| e.identifier == name) {
            Some(e) if unit.is_empty() || e.unit == unit => {}
            Some(e) => f.push(format!(
                "{name}: unit {:?} but the engine says {unit:?}",
                e.unit
            )),
            None => f.push(format!("{name}: no entry")),
        }
    }
    f
}

// ---- Timing behaviour (014 FR-016) -----------------------------------------------------------

fn other_value(e: &ControlEntry) -> Option<Val> {
    Some(match e.value {
        ControlValue::Switch { default } => Val::B(!default),
        ControlValue::Integer { min, max, default } => Val::I(if default == min {
            max.min(min + 1)
        } else {
            min
        }),
        ControlValue::Continuous { min, max, default } => {
            Val::F(if default == min { max } else { min })
        }
        ControlValue::Choice { options, default } => {
            let o = options.iter().find(|o| Some(o.value) != default)?;
            if e.identifier == "host_rate_hz" {
                Val::I(u64::from(options.iter().find(|o| o.value != 6000)?.value))
            } else {
                Val::C(Some(o.value))
            }
        }
        _ => return None,
    })
}

const RATE: u32 = 6000;

fn tone(seconds: f64) -> Vec<f32> {
    let n = (seconds * f64::from(RATE)) as usize;
    crate::stimulus::tone(1000.0, 0.1, f64::from(RATE), n)
}

fn record(p: &mut Pipeline, seconds: f64) {
    let input = tone(seconds);
    let mut events = vec![VasEvent::default(); p.max_record_events(input.len()).max(1)];
    p.start_recording().expect("start recording");
    let _ = p.record(&input, &mut events).expect("record");
    let mut events = vec![VasEvent::default(); p.max_record_events(0).max(64)];
    let _ = p.stop_recording(&mut events).expect("stop recording");
}

fn reconfigured(p: &mut Pipeline, id: &str, v: Val) -> Option<bool> {
    let mut s = *p.settings();
    if id == "host_rate_hz" {
        if let Val::I(x) = v {
            s.host_rate_hz = x as u32;
        }
    } else if !set_setting(&mut s, id, v) {
        return None;
    }
    Some(p.reconfigure(s).is_ok())
}

/// The three idle assertions on a pipeline that holds two recordings: a change is refused
/// while recording and while playing, and accepted (`fits`) or refused when idle.
fn idle_refusals(id: &str, v: Val, idle_value: Val, accepted_when_idle: bool) -> Vec<String> {
    let mut f = Vec::new();
    let mut p = Pipeline::new(Settings::new(RATE)).expect("pipeline");
    record(&mut p, 1.0);
    record(&mut p, 1.0);
    p.start_recording().expect("start");
    if reconfigured(&mut p, id, v) != Some(false) {
        f.push(format!(
            "{id}: an idle change was not refused while recording"
        ));
    }
    let mut ev = vec![VasEvent::default(); 64];
    let _ = p.stop_recording(&mut ev).expect("stop");
    p.start_playback(1).expect("play");
    if reconfigured(&mut p, id, v) != Some(false) {
        f.push(format!(
            "{id}: an idle change was not refused while playing"
        ));
    }
    p.stop_playback().expect("stop playback");
    if reconfigured(&mut p, id, idle_value) != Some(accepted_when_idle) {
        f.push(format!(
            "{id}: an idle change was {} when idle with two recordings present",
            if accepted_when_idle {
                "refused"
            } else {
                "accepted"
            }
        ));
    }
    f
}

/// 014 FR-016 (hardware review B2, coach review 7): live setters succeed while recording and
/// while playing; an idle change is refused while recording or playing and accepted when idle
/// with two recordings present; an idle, memory-dependent change is refused while recording or
/// playing, accepted with an empty memory, refused when the two stored recordings do not fit and,
/// for the memory figures, accepted when they do.
pub fn timing_behavior(entries: &[ControlEntry]) -> Vec<String> {
    let mut f = Vec::new();
    for e in entries {
        let id = e.identifier;
        let Some(v) = other_value(e) else {
            f.push(format!("{id}: no other value to offer"));
            continue;
        };
        match e.timing {
            ControlTiming::Live => {
                let mut p = Pipeline::new(Settings::new(RATE)).expect("pipeline");
                let mut ok = true;
                p.start_recording().expect("start");
                ok &= live_set(&mut p, id, v);
                let mut ev = vec![VasEvent::default(); 64];
                let _ = p.stop_recording(&mut ev).expect("stop");
                record(&mut p, 1.0);
                p.start_playback(1).expect("play");
                ok &= live_set(&mut p, id, v);
                if !ok {
                    f.push(format!(
                        "{id}: a live setter was refused while recording or playing"
                    ));
                }
            }
            ControlTiming::Idle | ControlTiming::HostFixed => {
                f.extend(idle_refusals(id, v, v, true));
            }
            ControlTiming::IdleMemoryDependent => {
                let mut p = Pipeline::new(Settings::new(RATE)).expect("pipeline");
                if reconfigured(&mut p, id, v) != Some(true) {
                    f.push(format!("{id}: refused with an empty memory"));
                }
                let unfit = match id {
                    "memory.capacity_s" | "memory.max_recordings" => Val::I(1),
                    _ => v,
                };
                f.extend(idle_refusals(id, v, unfit, false));
                let fits = match id {
                    "memory.capacity_s" => Some(Val::I(3599)),
                    "memory.max_recordings" => Some(Val::I(98)),
                    _ => None,
                };
                if let Some(fits) = fits {
                    f.extend(idle_refusals(id, v, fits, true));
                }
            }
            _ => f.push(format!("{id}: unknown timing class")),
        }
    }
    f
}

fn live_set(p: &mut Pipeline, id: &str, v: Val) -> bool {
    match (id, v) {
        ("route", Val::C(Some(r))) => {
            p.set_route(if r == 1 {
                Route::Earphone
            } else {
                Route::Speaker
            });
            p.route()
                == if r == 1 {
                    Route::Earphone
                } else {
                    Route::Speaker
                }
        }
        _ => live_accepts(p, id, v) == Some(true),
    }
}

// ---- Wording and kind rules (014 FR-006, FR-016; research R-10) ------------------------------

/// The denylist on every name and description, and the kind rules: a device control cites
/// S-001 or S-002; a unit condition or a model internal cites at least one A-###.
pub fn strings(entries: &[ControlEntry]) -> Vec<String> {
    let mut f = Vec::new();
    for e in entries {
        for (field, s) in [("name", e.name), ("description", e.description)] {
            if let Some(word) = denylist_hit(s) {
                f.push(format!(
                    "{}: {field} contains the denylisted {word:?}",
                    e.identifier
                ));
            }
        }
        match e.kind {
            ControlKind::DeviceControl
                if !e.sources.iter().any(|s| *s == "S-001" || *s == "S-002") =>
            {
                f.push(format!(
                    "{}: a device control must cite S-001 or S-002",
                    e.identifier
                ));
            }
            ControlKind::UnitCondition | ControlKind::ModelInternal if e.assumptions.is_empty() => {
                f.push(format!(
                    "{}: a unit condition or model internal must cite an assumption",
                    e.identifier
                ));
            }
            _ => {}
        }
    }
    f
}

// ---- The controls notes (014 FR-009; research R-05) ------------------------------------------

const SETTING_PREFIXES: [&str; 5] = ["agc.", "vas.", "mic.", "output.", "memory."];
const TOP_LEVEL: [&str; 7] = [
    "host_rate_hz",
    "record_stage_enabled",
    "playback_stage_enabled",
    "adc_clamp_enabled",
    "tap",
    "seed",
    "speech_codec_enabled",
];

/// One token of a controls note, after prefix expansion.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    /// Names an entry (an identifier, a setting path, or a live control's setter or getter).
    Entry(String),
    /// Names no entry (an option value or another API item).
    Other(String),
    /// Looks like a setting but resolves to no entry: a finding.
    Unresolved(String),
}

/// Splits and classifies a controls note exactly as 014 FR-009 states.
pub fn tokens(note: &str, entries: &[ControlEntry]) -> Vec<Token> {
    let ident = Regex::new(r"^[a-z_][a-z0-9_]*$").expect("regex");
    let mut out = Vec::new();
    for group in note.split(';') {
        let mut prefix: Option<String> = None;
        for raw in group.split(',') {
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            let expanded = if t.contains('.') && !t.contains("::") {
                prefix = t.rsplit_once('.').map(|(p, _)| p.to_string());
                t.to_string()
            } else if ident.is_match(t) && !t.starts_with("set_") {
                match &prefix {
                    Some(p)
                        if SETTING_PREFIXES
                            .iter()
                            .any(|sp| sp.trim_end_matches('.') == p) =>
                    {
                        format!("{p}.{t}")
                    }
                    _ => t.to_string(),
                }
            } else {
                t.to_string()
            };
            let live = expanded
                .strip_prefix("set_")
                .filter(|id| LIVE.contains(id))
                .or_else(|| LIVE.iter().copied().find(|id| *id == expanded));
            let token = if entries.iter().any(|e| e.identifier == expanded) {
                Token::Entry(expanded)
            } else if let Some(id) = live {
                Token::Entry(id.to_string())
            } else if expanded.starts_with("Tap::") || expanded.starts_with("Route::") {
                Token::Other(expanded)
            } else if SETTING_PREFIXES.iter().any(|p| expanded.starts_with(p))
                || TOP_LEVEL.contains(&expanded.as_str())
            {
                Token::Unresolved(expanded)
            } else {
                Token::Other(expanded)
            };
            out.push(token);
        }
    }
    out
}

/// 014 FR-009: every entry with stage 1 – 15 is named in that stage's controls note, and every
/// entry a note names has that stage (the speech codec switch: 7 or 9).
pub fn notes(entries: &[ControlEntry], manifest: &Manifest) -> Vec<String> {
    let mut f = Vec::new();
    for stage in manifest.stages() {
        let n = stage.number;
        let named: Vec<String> = tokens(stage.controls, entries)
            .into_iter()
            .filter_map(|t| match t {
                Token::Entry(id) => Some(id),
                Token::Unresolved(t) => {
                    f.push(format!(
                        "stage {n}: {t:?} looks like a setting but names no entry"
                    ));
                    None
                }
                Token::Other(_) => None,
            })
            .collect();
        for id in &named {
            let Some(e) = entries.iter().find(|e| e.identifier == id) else {
                continue;
            };
            let ok = e.stage == n || (id == "speech_codec_enabled" && n == 9);
            if !ok {
                f.push(format!(
                    "{id}: named in stage {n}'s note but its stage is {}",
                    e.stage
                ));
            }
        }
        for e in entries.iter().filter(|e| e.stage == n) {
            if !named.iter().any(|id| id == e.identifier) {
                f.push(format!(
                    "{}: stage {n} but not named in its controls note",
                    e.identifier
                ));
            }
        }
    }
    f
}

// ---- Setters, options and IDs from the source (014 FR-007, FR-005, FR-016) -------------------

/// The text with every `#[cfg(test)]` item removed: the attribute line and the item it
/// applies to (a `mod x;` line, or a block up to its matching closing brace), wherever it sits,
/// so public items after a test module are still scanned (coach review 6).
pub fn without_tests(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("#[cfg(test)]") {
        let at_line_start = i == 0 || rest.as_bytes()[i - 1] == b'\n';
        if !at_line_start {
            out.push_str(&rest[..i + 1]);
            rest = &rest[i + 1..];
            continue;
        }
        out.push_str(&rest[..i]);
        let item = &rest[i..];
        let semi = item.find(';');
        let brace = item.find('{');
        let end = match (semi, brace) {
            (Some(sc), Some(b)) if sc < b => sc + 1,
            (_, Some(b)) => {
                let mut depth = 0usize;
                let mut end = item.len();
                for (k, ch) in item[b..].char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = b + k + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                end
            }
            (Some(sc), None) => sc + 1,
            (None, None) => item.len(),
        };
        rest = &item[end..];
    }
    out.push_str(rest);
    out
}

/// The core's sources: (path relative to the repository, text without its test module).
pub fn core_sources() -> Vec<(String, String)> {
    let root = crate::manifest_checks::repo_root().join("crates/rr_dr60/src");
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    out.sort();
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            out.push((rel, without_tests(&text)));
        }
    }
}

/// The C header's text.
pub fn header_text() -> String {
    std::fs::read_to_string(
        crate::manifest_checks::repo_root().join("crates/rr_dr60_ffi/include/rr_dr60.h"),
    )
    .expect("read rr_dr60.h")
}

/// 014 FR-007 (research R-04): every public `set_*` method in the core and every
/// `rr_dr60_set_*` declaration in the header is a live entry, and every live entry has both.
pub fn setters(
    entries: &[ControlEntry],
    sources: &[(String, String)],
    header: &str,
) -> Vec<String> {
    let core = Regex::new(r"pub(\s+const)?\s+fn\s+set_(\w+)").expect("regex");
    let c = Regex::new(r"(?m)^[A-Za-z_][\w \*]*\brr_dr60_set_(\w+)\(").expect("regex");
    let live: Vec<&str> = entries
        .iter()
        .filter(|e| e.timing == ControlTiming::Live)
        .map(|e| e.identifier)
        .collect();
    let mut found_core = Vec::new();
    for (path, text) in sources {
        for cap in core.captures_iter(text) {
            found_core.push((path.clone(), cap[2].to_string()));
        }
    }
    let found_c: Vec<String> = c
        .captures_iter(header)
        .map(|cap| cap[1].to_string())
        .collect();
    let mut f = Vec::new();
    for (path, name) in &found_core {
        if !live.contains(&name.as_str()) {
            f.push(format!("{path}: setter set_{name} has no live entry"));
        }
    }
    for name in &found_c {
        if !live.contains(&name.as_str()) {
            f.push(format!("rr_dr60.h: rr_dr60_set_{name} has no live entry"));
        }
    }
    for id in live {
        // Spec 017 D4 (restated): the eight record-side values are live through the schedule
        // only; a plain setter for one of them would be a defect. Spec 021 FR-001: the four
        // playback-stage switches likewise.
        if is_record_side(id) || is_stage_switch(id) {
            let bare = id.rsplit('.').next().unwrap_or(id);
            if found_core.iter().any(|(_, n)| n == id || n == bare)
                || found_c.iter().any(|n| n == id || n == bare)
            {
                let which = if is_record_side(id) {
                    "a record-side value with a plain setter (spec 017: the schedule only)"
                } else {
                    "a playback-stage switch with a plain setter (spec 021: the schedule only)"
                };
                f.push(format!("{id}: {which}"));
            }
            continue;
        }
        if !found_core.iter().any(|(_, n)| n == id) {
            f.push(format!("{id}: live entry without a core setter"));
        }
        if !found_c.iter().any(|n| n == id) {
            f.push(format!("{id}: live entry without a C setter"));
        }
    }
    f
}

/// 014 FR-005: each option of the tap, the VAS mode and the route equals the header's
/// enumeration value of the same name.
pub fn options_vs_header(entries: &[ControlEntry], header: &str) -> Vec<String> {
    let mut f = Vec::new();
    for (id, prefix) in [
        ("tap", "TAP"),
        ("vas.mode", "VAS_MODE"),
        ("route", "ROUTE"),
        ("playback_speed_mode", "SPEED_MODE"), // spec 023
    ] {
        let Some(ControlValue::Choice { options, .. }) =
            entries.iter().find(|e| e.identifier == id).map(|e| e.value)
        else {
            f.push(format!("{id}: not a choice"));
            continue;
        };
        for o in options {
            let want = format!(
                "RR_DR60_{prefix}_{} = {},",
                o.identifier.to_uppercase(),
                o.value
            );
            if !header.contains(&want) {
                f.push(format!(
                    "{id}: option {}={} not in the header as {want:?}",
                    o.identifier, o.value
                ));
            }
        }
    }
    f
}

fn default_line<'a>(id: &str, sources: &'a [(String, String)]) -> Option<&'a str> {
    let file = |name: &str| {
        sources
            .iter()
            .find(|(p, _)| p == name)
            .map(|(_, t)| t.as_str())
    };
    let find = |text: &'a str, start: &str, needle: &str| -> Option<&'a str> {
        let from = text.find(start)?;
        let block = &text[from..];
        let end = block
            .find("\n    };")
            .or_else(|| block.find("\n        }\n"))
            .unwrap_or(block.len());
        block[..end]
            .lines()
            .find(|l| l.trim_start().starts_with(needle))
    };
    let settings = file("settings.rs")?;
    match id {
        "route" => {
            // The tags are read from the variant that carries `#[default]` (hardware review B10).
            let from = settings.find("pub enum Route")?;
            let mut lines = settings[from..].lines();
            let mut previous = "";
            loop {
                let l = lines.next()?;
                if l.trim_start().starts_with("Speaker,") {
                    return (previous.trim() == "#[default]").then_some(l);
                }
                previous = l;
            }
        }
        "volume" => file("output.rs")?
            .lines()
            .find(|l| l.contains("pub const VOLUME_REFERENCE")),
        "playback_speed" => file("speed.rs")?
            .lines()
            .find(|l| l.contains("pub const DEFAULT_LEVEL")),
        // Spec 018: the battery default's line carries the control's tags.
        "battery_volts" => file("battery.rs")?
            .lines()
            .find(|l| l.contains("pub const DEFAULT_BATTERY_VOLTS")),
        // Spec 023: the enum's `#[default]` variant line carries the control's tags.
        "playback_speed_mode" => {
            let f = file("speed.rs")?;
            let mut previous = "";
            for l in f.lines() {
                if l.trim_start().starts_with("PitchWithSpeed") {
                    return (previous.trim() == "#[default]").then_some(l);
                }
                previous = l;
            }
            None
        }
        "host_rate_hz" => settings
            .lines()
            .find(|l| l.contains("pub const SUPPORTED_HOST_RATES")),
        _ => match id.split_once('.') {
            Some((group, field)) => {
                let name = match group {
                    "agc" => "AgcSettings",
                    "vas" => "VasSettings",
                    "memory" => "MemorySettings",
                    "output" => "OutputSettings",
                    "mic" => "MicSettings",
                    _ => return None,
                };
                // The tags are read from `DEVICE` only if `Settings::new` uses it (B10).
                if !settings.contains(&format!("{group}: {name}::DEVICE,")) {
                    return None;
                }
                let imp = settings.find(&format!("impl {name} {{"))?;
                let rest = &settings[imp..];
                let dev = rest.find("pub const DEVICE: Self = Self {")?;
                find(&rest[dev..], "pub const DEVICE", &format!("{field}:"))
            }
            None => find(
                settings,
                "pub const fn new(host_rate_hz: u32) -> Self {",
                &format!("{id}:"),
            ),
        },
    }
}

/// 014 FR-016 (research R-06): an entry's IDs equal the tags in the trailing `//` comment of
/// its default's line.
pub fn ids(entries: &[ControlEntry], sources: &[(String, String)]) -> Vec<String> {
    let tag = Regex::new(r"\b([AS])-(\d{3})\b").expect("regex");
    let mut f = Vec::new();
    for e in entries {
        let Some(line) = default_line(e.identifier, sources) else {
            f.push(format!("{}: default line not found", e.identifier));
            continue;
        };
        let comment = line.split_once("//").map_or("", |(_, c)| c);
        let mut s: Vec<String> = Vec::new();
        let mut a: Vec<String> = Vec::new();
        for cap in tag.captures_iter(comment) {
            let id = format!("{}-{}", &cap[1], &cap[2]);
            let list = if &cap[1] == "S" { &mut s } else { &mut a };
            if !list.contains(&id) {
                list.push(id);
            }
        }
        s.sort();
        a.sort();
        let ts: Vec<String> = e.sources.iter().map(|x| x.to_string()).collect();
        let ta: Vec<String> = e.assumptions.iter().map(|x| x.to_string()).collect();
        if ts != s || ta != a {
            f.push(format!(
                "{}: cites {ts:?} {ta:?} but its default's comment cites {s:?} {a:?}",
                e.identifier
            ));
        }
    }
    f
}
