//! The meter read-out described (spec 026 D4): one entry per [`Meters`](crate::Meters) field,
//! in its declaration order, with its unit, a sentence, its kind and its side; whether a call
//! reaches it and its point's lag are [`Pipeline::meter_reached`](crate::Pipeline::meter_reached)
//! and [`Pipeline::meter_lag_samples`](crate::Pipeline::meter_lag_samples). An unreached field
//! reads its rest value.

/// What a field holds over a call.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeterKind {
    /// The largest value over the call's processed samples.
    Maximum = 0,
    /// A count of samples over the call.
    Count = 1,
    /// A state at the call's end.
    State = 2,
}

/// Which side of the engine a field belongs to.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeterSide {
    /// The record side (the input).
    Input = 0,
    /// The playback side (the output).
    Output = 1,
}

/// The call a meter read-out comes from.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeterCall {
    /// A one-pass call (`process` and its forms).
    OnePass = 0,
    /// A record call (`record` and its forms, the stop).
    Record = 1,
    /// A play call (`play` and its forms).
    Play = 2,
}

/// One field's description (spec 026 D4).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeterField {
    /// The field's name in [`Meters`](crate::Meters).
    pub identifier: &'static str,
    /// Its unit.
    pub unit: &'static str,
    /// One neutral sentence.
    pub sentence: &'static str,
    /// Its kind.
    pub kind: MeterKind,
    /// Its side.
    pub side: MeterSide,
}

const fn f(
    identifier: &'static str,
    unit: &'static str,
    sentence: &'static str,
    kind: MeterKind,
    side: MeterSide,
) -> MeterField {
    MeterField {
        identifier,
        unit,
        sentence,
        kind,
        side,
    }
}

use MeterKind::{Count, Maximum, State};
use MeterSide::{Input, Output};

/// The thirteen fields, in [`Meters`](crate::Meters)'s declaration order.
pub const FIELDS: [MeterField; 13] = [
    f("input_peak", "full scale", "The largest input sample of the call, before the microphone stages.", Maximum, Input),
    f("limiter_input_peak", "full scale", "The largest sample entering the limiter during the call.", Maximum, Input),
    f("gain_reduction_db", "dB", "The deepest attenuation the limiter applied during the call.", Maximum, Input),
    f("hold_charge", "fraction of hold", "The limiter's hold charge at the call's end, as a fraction of the model's 40 dB hold.", State, Input),
    f("record_filter_peak", "full scale", "The largest sample after the record filter, before the converter's clamp, during the call.", Maximum, Input),
    f("adc_clamp_count", "count (device samples)", "The device samples the converter's clamp changed during the call.", Count, Input),
    f("vas_paused", "state", "Whether the voice-activated pause is holding at the end of the call.", State, Input),
    f("hang_remaining_samples", "device samples", "The device samples left before the voice-activated pause holds, at the call's end.", State, Input),
    f("reconstruction_peak", "full scale", "The largest sample after the playback reconstruction during the call.", Maximum, Output),
    f("route_peak", "full scale", "The largest sample at the route's output during the call; it can exceed 1.0 on the speaker route.", Maximum, Output),
    f("output_clamp_count", "count (host samples)", "The host samples the optional output clamp changed during the call.", Count, Output),
    f("amp_clip_count", "count (host samples)", "The host samples the speaker amplifier's knee or ceiling changed during the call.", Count, Output),
    f("battery_ceiling", "factor", "The amplifier's ceiling factor for the battery voltage the user set.", State, Output),
];

/// Pins [`FIELDS`] to [`Meters`](crate::Meters): an exhaustive destructure in declaration
/// order, so a new field fails to compile until it is described.
#[allow(dead_code)]
fn order(m: crate::Meters) -> [&'static str; 13] {
    let crate::Meters {
        input_peak: _,
        limiter_input_peak: _,
        gain_reduction_db: _,
        hold_charge: _,
        record_filter_peak: _,
        adc_clamp_count: _,
        vas_paused: _,
        hang_remaining_samples: _,
        reconstruction_peak: _,
        route_peak: _,
        output_clamp_count: _,
        amp_clip_count: _,
        battery_ceiling: _,
    } = m;
    [
        "input_peak",
        "limiter_input_peak",
        "gain_reduction_db",
        "hold_charge",
        "record_filter_peak",
        "adc_clamp_count",
        "vas_paused",
        "hang_remaining_samples",
        "reconstruction_peak",
        "route_peak",
        "output_clamp_count",
        "amp_clip_count",
        "battery_ceiling",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_follow_the_struct() {
        let names = order(crate::Meters::default());
        for (n, f) in names.iter().zip(FIELDS.iter()) {
            assert_eq!(*n, f.identifier);
        }
    }
}
