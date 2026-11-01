//! Spec 025, the settings read and control access by index: the shared pieces of its stories
//! (a settings set differing in every field in Rust and C, the C one-hot oracle on a painted
//! structure, the live-control twins).

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::control_map::ControlReading;
use rr_dr60::controls::{ControlValue, controls};
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream, Tap, VasEvent, VasMode};
use rr_dr60_ffi::*;

/// A valid settings set differing from the defaults in every field (`base_rate` is the rate
/// it is applied over; the set's own rate is `rate`, which must differ from it).
pub fn differ(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.record_stage_enabled = false;
    s.playback_stage_enabled = false;
    s.adc_clamp_enabled = false;
    s.tap = Tap::AfterMicAmp;
    s.seed = 0xDEAD_BEEF_1234_5678;
    s.agc.enabled = false;
    s.agc.knee_dbfs = -6.0;
    s.agc.attack_ms = 5.0;
    s.agc.release_ms = 2000.0;
    s.vas.enabled = false;
    s.vas.mode = VasMode::Mute;
    s.vas.sensitivity = 2;
    s.vas.threshold_dbfs = -30.0;
    s.vas.hang_ms = 300.0;
    s.vas.onset_ms = 10.0;
    s.memory.capacity_s = 600;
    s.memory.max_recordings = 20;
    s.speech_codec_enabled = false;
    s.output.volume_amp_enabled = false;
    s.output.speaker_enabled = false;
    s.output.earphone_enabled = false;
    s.output.clamp_enabled = true;
    s.mic.capsule_enabled = false;
    s.mic.amp_enabled = false;
    s.mic.hiss_offset_db = 3.0;
    s
}

/// The C form of [`differ`], assigned by field name (independent of the library's converter).
pub fn differ_c(rate: u32) -> RrDr60Settings {
    let mut c = rr_dr60_settings_default(rate);
    c.record_stage_enabled = false;
    c.playback_stage_enabled = false;
    c.adc_clamp_enabled = false;
    c.tap = 10;
    c.seed = 0xDEAD_BEEF_1234_5678;
    c.agc_enabled = false;
    c.agc_knee_dbfs = -6.0;
    c.agc_attack_ms = 5.0;
    c.agc_release_ms = 2000.0;
    c.vas_enabled = false;
    c.vas_mode = 1;
    c.vas_sensitivity = 2;
    c.vas_threshold_dbfs = -30.0;
    c.vas_hang_ms = 300.0;
    c.vas_onset_ms = 10.0;
    c.memory_capacity_s = 600;
    c.memory_max_recordings = 20;
    c.speech_codec_enabled = false;
    c.output_volume_amp_enabled = false;
    c.output_speaker_enabled = false;
    c.output_earphone_enabled = false;
    c.output_clamp_enabled = true;
    c.mic_capsule_enabled = false;
    c.mic_amp_enabled = false;
    c.mic_hiss_offset_db = 3.0;
    c
}

/// A rate other than `rate` among the supported ones (the engine is built there first).
pub fn other_rate(rate: u32) -> u32 {
    if rate == 48_000 { 16_000 } else { 48_000 }
}

/// The settings structure's four implicit padding runs (64-bit; spec 022's table).
pub const SETTINGS_PADDING: [(usize, usize); 4] = [(10, 12), (25, 28), (49, 52), (73, 76)];

/// The bytes of a settings structure, its padding painted `0xAA`.
pub fn painted(c: &RrDr60Settings) -> [u8; 120] {
    let mut b = [0u8; 120];
    // SAFETY: a 120-byte plain struct copied as bytes (its padding then overwritten).
    unsafe {
        core::ptr::copy_nonoverlapping(
            (c as *const RrDr60Settings).cast::<u8>(),
            b.as_mut_ptr(),
            120,
        )
    };
    for (lo, hi) in SETTINGS_PADDING {
        b[lo..hi].fill(0xAA);
    }
    b
}

/// Whether byte `k` is padding.
pub fn is_padding(k: usize) -> bool {
    SETTINGS_PADDING
        .iter()
        .any(|&(lo, hi)| (lo..hi).contains(&k))
}

/// The one-hot value of a settings index (spec 025 D5): a switch flipped, a continuous index +
/// 0.5, an integer index + 2, a non-default option; unique to the index.
pub fn one_hot_value(index: u32) -> ControlReading {
    let e = &controls()[index as usize];
    match e.value {
        ControlValue::Switch { default } => ControlReading::Switch(!default),
        ControlValue::Integer { .. } => ControlReading::Integer(u64::from(index) + 2),
        ControlValue::Continuous { .. } => ControlReading::Continuous(index as f32 + 0.5),
        ControlValue::Choice { options, default } => {
            let d = default.unwrap_or(48_000);
            ControlReading::Choice(
                options
                    .iter()
                    .map(|o| o.value)
                    .find(|v| *v != d)
                    .expect("an option"),
            )
        }
        _ => unreachable!("four value types"),
    }
}

/// The oracle's write by field name (the test's literal identifier per field): `c` with the
/// field named `identifier` set to `r`. Panics on a name it does not know.
pub fn set_by_name(c: &mut RrDr60Settings, identifier: &str, r: ControlReading) {
    use ControlReading::{Choice, Continuous, Integer, Switch};
    match (identifier, r) {
        ("host_rate_hz", Choice(v)) => c.host_rate_hz = v,
        ("record_stage_enabled", Switch(b)) => c.record_stage_enabled = b,
        ("playback_stage_enabled", Switch(b)) => c.playback_stage_enabled = b,
        ("adc_clamp_enabled", Switch(b)) => c.adc_clamp_enabled = b,
        ("tap", Choice(v)) => c.tap = v,
        ("seed", Integer(n)) => c.seed = n,
        ("agc.enabled", Switch(b)) => c.agc_enabled = b,
        ("agc.knee_dbfs", Continuous(v)) => c.agc_knee_dbfs = v,
        ("agc.attack_ms", Continuous(v)) => c.agc_attack_ms = v,
        ("agc.release_ms", Continuous(v)) => c.agc_release_ms = v,
        ("vas.enabled", Switch(b)) => c.vas_enabled = b,
        ("vas.mode", Choice(v)) => c.vas_mode = v,
        ("vas.sensitivity", Integer(n)) => c.vas_sensitivity = n as u32,
        ("vas.threshold_dbfs", Continuous(v)) => c.vas_threshold_dbfs = v,
        ("vas.hang_ms", Continuous(v)) => c.vas_hang_ms = v,
        ("vas.onset_ms", Continuous(v)) => c.vas_onset_ms = v,
        ("memory.capacity_s", Integer(n)) => c.memory_capacity_s = n as u32,
        ("memory.max_recordings", Integer(n)) => c.memory_max_recordings = n as u32,
        ("speech_codec_enabled", Switch(b)) => c.speech_codec_enabled = b,
        ("output.volume_amp_enabled", Switch(b)) => c.output_volume_amp_enabled = b,
        ("output.speaker_enabled", Switch(b)) => c.output_speaker_enabled = b,
        ("output.earphone_enabled", Switch(b)) => c.output_earphone_enabled = b,
        ("output.clamp_enabled", Switch(b)) => c.output_clamp_enabled = b,
        ("mic.capsule_enabled", Switch(b)) => c.mic_capsule_enabled = b,
        ("mic.amp_enabled", Switch(b)) => c.mic_amp_enabled = b,
        ("mic.hiss_offset_db", Continuous(v)) => c.mic_hiss_offset_db = v,
        _ => panic!("the oracle does not name {identifier} with {r:?}"),
    }
}

/// The C reading of a Rust reading.
pub fn c_reading(r: ControlReading) -> RrDr60ControlReading {
    let mut c = RrDr60ControlReading::default();
    match r {
        ControlReading::Switch(b) => {
            c.value_type = RrDr60ControlValueType::Switch as u32;
            c.int_value = u64::from(b);
        }
        ControlReading::Integer(n) => {
            c.value_type = RrDr60ControlValueType::Integer as u32;
            c.int_value = n;
        }
        ControlReading::Continuous(v) => {
            c.value_type = RrDr60ControlValueType::Continuous as u32;
            c.float_value = v;
        }
        ControlReading::Choice(v) => {
            c.value_type = RrDr60ControlValueType::Choice as u32;
            c.int_value = u64::from(v);
        }
    }
    c
}

/// The Rust reading of a library-written C reading, strictly (hardware review B5): `None` for
/// an unknown type, a switch other than 0 or 1, a choice above `u32`, a non-zero unused field,
/// a non-zero `reserved` or a size other than 24.
pub fn rust_reading(c: &RrDr60ControlReading) -> Option<ControlReading> {
    if c.reserved != 0 || c.struct_size != 24 {
        return None;
    }
    let float_unused = c.float_value.to_bits() == 0;
    Some(match c.value_type {
        0 if float_unused && c.int_value <= 1 => ControlReading::Switch(c.int_value == 1),
        1 if float_unused => ControlReading::Integer(c.int_value),
        2 if c.int_value == 0 => ControlReading::Continuous(c.float_value),
        3 if float_unused => ControlReading::Choice(u32::try_from(c.int_value).ok()?),
        _ => return None,
    })
}

/// The manifest default of every control as a reading (`rate` for the host rate).
pub fn default_reading(index: u32, rate: u32) -> ControlReading {
    match controls()[index as usize].value {
        ControlValue::Switch { default } => ControlReading::Switch(default),
        ControlValue::Integer { default, .. } => ControlReading::Integer(default),
        ControlValue::Continuous { default, .. } => ControlReading::Continuous(default),
        ControlValue::Choice { default, .. } => ControlReading::Choice(default.unwrap_or(rate)),
        _ => unreachable!("four value types"),
    }
}

/// The five live controls with a plain setter, their index, a non-default value and the
/// streams they run on.
pub fn live_cases() -> [(u32, ControlReading, f64, &'static [Stream]); 5] {
    use Stream::{OnePass, Playback};
    let i = |id: &str| controls().iter().position(|e| e.identifier == id).unwrap() as u32;
    [
        (
            i("route"),
            ControlReading::Choice(1),
            1.0,
            &[Playback, OnePass],
        ),
        (
            i("volume"),
            ControlReading::Continuous(0.25),
            0.25,
            &[Playback, OnePass],
        ),
        (
            i("playback_speed"),
            ControlReading::Integer(5),
            5.0,
            &[Playback],
        ),
        (
            i("playback_speed_mode"),
            ControlReading::Choice(1),
            1.0,
            &[Playback],
        ),
        (
            i("battery_volts"),
            ControlReading::Continuous(2.5),
            2.5,
            &[Playback, OnePass],
        ),
    ]
}

/// How a live value is applied in [`live_twin`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// `set_control_value`.
    ByIndex,
    /// The plain setter.
    Setter,
    /// `schedule_change` at the stream's position, tag 1.
    Schedule,
}

/// Applies a live value through its plain setter.
pub fn plain_set(p: &mut Pipeline, index: u32, r: ControlReading) {
    match (controls()[index as usize].identifier, r) {
        ("route", ControlReading::Choice(c)) => p.set_route(if c == 1 {
            Route::Earphone
        } else {
            Route::Speaker
        }),
        ("volume", ControlReading::Continuous(v)) => p.set_volume(v).unwrap(),
        ("playback_speed", ControlReading::Integer(n)) => p.set_playback_speed(n as u8).unwrap(),
        ("playback_speed_mode", ControlReading::Choice(c)) => {
            p.set_playback_speed_mode(rr_dr60::speed::SpeedMode::from_u32(c).unwrap())
        }
        ("battery_volts", ControlReading::Continuous(v)) => p.set_battery_volts(v).unwrap(),
        (id, r) => panic!("not a plain live control: {id} {r:?}"),
    }
}

/// The change-log entries of control `index` when it is set to `r` (`value` as the schedule
/// takes it) by `how` on `stream` after 100 host samples at block size 1, then 50 more
/// samples. `p` holds recording 1 for the playback stream.
pub fn live_twin(
    p: &mut Pipeline,
    stream: Stream,
    index: u32,
    r: ControlReading,
    value: f64,
    how: How,
) -> Vec<ChangeEntry> {
    let rate = p.settings().host_rate_hz;
    let x = crate::stimulus::tone(1000.0, 0.1, f64::from(rate), 150);
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut all = Vec::new();
    if stream == Stream::Playback {
        p.start_playback(1).expect("start_playback");
    }
    for k in 0..150usize {
        if k == 100 {
            match how {
                How::ByIndex => p.set_control_value(index, r).expect("by index"),
                How::Setter => plain_set(p, index, r),
                How::Schedule => {
                    let at = p.stream_position(stream).unwrap();
                    p.schedule_change(stream, at, index, value, 1)
                        .expect("schedule");
                }
            }
        }
        let n = match stream {
            Stream::Playback => {
                let mut y = [0.0f32; 1];
                p.play_with_changes(&mut y, &mut log).expect("play").changes
            }
            _ => {
                let mut y = [0.0f32; 1];
                let mut ev = vec![VasEvent::default(); p.max_events(1)];
                p.process_with_events_and_changes(&x[k..k + 1], &mut y, &mut ev, &mut log)
                    .expect("process")
                    .changes
            }
        };
        all.extend(log[..n].iter().filter(|e| e.control == index).copied());
    }
    if stream == Stream::Playback {
        p.stop_playback().expect("stop");
    }
    all
}
