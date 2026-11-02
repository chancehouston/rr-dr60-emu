//! The insertion points and the components (spec 024 D1, D2; research R-04): constant data an
//! app builds its "add component" card from. One point, `before_limiter`, and one component,
//! the band filter, in 0.92; later components append (each at its own `parameter_offset` in the
//! global store, all of them together within [`COMPONENT_PARAMETER_BUDGET`]). The slot and every
//! component are additions of the emulator, not parts of the original recorder.

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use crate::controls::{ControlOption, ControlValue};

/// The parameters of all components together, each at its fixed offset in the engine's global
/// parameter store (spec 024 D5, the owner's clarification of 2026-11-01): the term in
/// `MAX_CHANGES_PER_CALL`'s derivation. The band filter uses 0 – 3.
pub const COMPONENT_PARAMETER_BUDGET: usize = 16; // engineering target (024 D5)

/// The paths a point or component serves: the record side (the recorder and the one-pass's
/// record half).
pub const PATH_RECORD: u32 = 1;

/// The published insertion points (spec 024 D1; C `RR_DR60_INSERTION_POINT_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum InsertionPointId {
    /// Between stage 2, the microphone amplifier, and stage 3, the limiter, at the host rate.
    BeforeLimiter = 0,
}

impl InsertionPointId {
    /// The point of a C value, if published.
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::BeforeLimiter),
            _ => None,
        }
    }
}

/// The components (spec 024 D2; C `RR_DR60_COMPONENT_*`): none, or one of the engine's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum ComponentId {
    /// The slot is empty: bit-identical to 0.91 (spec 024 FR-002).
    #[default]
    None = 0,
    /// The band filter (spec 024 D4).
    BandFilter = 1,
}

impl ComponentId {
    /// The component of a C value, if published.
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::BandFilter),
            _ => None,
        }
    }

    /// The control manifest's option identifier.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BandFilter => "band_filter",
        }
    }
}

/// How a parameter's transition moves, and a slider's hint (product plan review M3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Scale {
    /// Linear (a choice blends; an integer steps).
    Linear = 0,
    /// Logarithmic (the frequency, Q).
    Log = 1,
    /// In decibels (a gain).
    Decibel = 2,
}

/// One insertion point.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct InsertionPoint {
    /// The point's value.
    pub id: InsertionPointId,
    /// The identifier (`before_limiter`).
    pub identifier: &'static str,
    /// The name.
    pub name: &'static str,
    /// The paths it serves ([`PATH_RECORD`]).
    pub paths: u32,
    /// Where it sits, one sentence.
    pub placement: &'static str,
    /// The control manifest's index of the slot's control.
    pub control_index: u32,
}

/// One parameter of a component.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ComponentParameter {
    /// The control manifest identifier (`band_filter.frequency_hz`).
    pub identifier: &'static str,
    /// The name.
    pub name: &'static str,
    /// Type, range and default (the control manifest's own constants).
    pub value: ControlValue,
    /// The unit, or empty.
    pub unit: &'static str,
    /// The transition's scale.
    pub scale: Scale,
    /// The control manifest's index.
    pub control_index: u32,
}

/// One component.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Component {
    /// The component's value (never [`ComponentId::None`]).
    pub id: ComponentId,
    /// The identifier.
    pub identifier: &'static str,
    /// The name.
    pub name: &'static str,
    /// What it does to the sound, one sentence.
    pub description: &'static str,
    /// The latency it adds, in host samples (0 for every 0.92 component; FR-013).
    pub latency_samples: u32,
    /// The paths it serves ([`PATH_RECORD`]).
    pub paths: u32,
    /// Its parameters' base index in the global store.
    pub parameter_offset: u32,
    /// Its parameters, in store order from `parameter_offset`.
    pub parameters: &'static [ComponentParameter],
}

/// The band filter's type options (spec 024 D4), in the engine's value order.
pub(crate) const BAND_FILTER_TYPES: &[ControlOption] = &[
    ControlOption {
        identifier: "low_pass",
        value: 0,
    },
    ControlOption {
        identifier: "high_pass",
        value: 1,
    },
    ControlOption {
        identifier: "band_pass",
        value: 2,
    },
    ControlOption {
        identifier: "notch",
        value: 3,
    },
];

/// The band filter's parameters' types, ranges and defaults (spec 024 D4), shared with the
/// control manifest's entries 32 – 35 (one source; research R-04).
pub(crate) const BAND_FILTER_TYPE: ControlValue = ControlValue::Choice {
    options: BAND_FILTER_TYPES,
    default: Some(0), // engineering target (024 D4): low-pass
};
/// The frequency: 20 – 20 000 Hz, default 1000 Hz.
pub(crate) const BAND_FILTER_FREQUENCY: ControlValue = ControlValue::Continuous {
    min: 20.0,       // engineering target (024 D4)
    max: 20_000.0,   // engineering target (024 D4)
    default: 1000.0, // engineering target (024 D4)
};
/// Q: 0.5 – 10, default 0.7071.
pub(crate) const BAND_FILTER_Q: ControlValue = ControlValue::Continuous {
    min: 0.5,        // engineering target (024 D4)
    max: 10.0,       // engineering target (024 D4)
    default: 0.7071, // engineering target (024 D4)
};
/// The gain: −24 … +24 dB, default 0.
pub(crate) const BAND_FILTER_GAIN: ControlValue = ControlValue::Continuous {
    min: -24.0, // engineering target (024 D4)
    max: 24.0,  // engineering target (024 D4)
    default: 0.0,
};

/// The control manifest's index of the slot's control (`insert.before_limiter`).
pub(crate) const CONTROL_INSERT: u32 = 31;
/// The control manifest's indices of the band filter's four parameters, in store order.
pub(crate) const CONTROL_BAND_FILTER: [u32; 4] = [32, 33, 34, 35];

const BAND_FILTER_PARAMETERS: &[ComponentParameter] = &[
    ComponentParameter {
        identifier: "band_filter.type",
        name: "Band filter type",
        value: BAND_FILTER_TYPE,
        unit: "",
        scale: Scale::Linear,
        control_index: CONTROL_BAND_FILTER[0],
    },
    ComponentParameter {
        identifier: "band_filter.frequency_hz",
        name: "Band filter frequency",
        value: BAND_FILTER_FREQUENCY,
        unit: "Hz",
        scale: Scale::Log,
        control_index: CONTROL_BAND_FILTER[1],
    },
    ComponentParameter {
        identifier: "band_filter.q",
        name: "Band filter Q",
        value: BAND_FILTER_Q,
        unit: "",
        scale: Scale::Log,
        control_index: CONTROL_BAND_FILTER[2],
    },
    ComponentParameter {
        identifier: "band_filter.gain_db",
        name: "Band filter gain",
        value: BAND_FILTER_GAIN,
        unit: "dB",
        scale: Scale::Decibel,
        control_index: CONTROL_BAND_FILTER[3],
    },
];

static INSERTION_POINTS: [InsertionPoint; 1] = [InsertionPoint {
    id: InsertionPointId::BeforeLimiter,
    identifier: "before_limiter",
    name: "Before the limiter",
    paths: PATH_RECORD,
    placement: "Between stage 2, the microphone amplifier, and stage 3, the limiter, at the host rate; an addition of the emulator, the original recorder has no place for one.",
    control_index: CONTROL_INSERT,
}];

static COMPONENTS: [Component; 1] = [Component {
    id: ComponentId::BandFilter,
    identifier: "band_filter",
    name: "Band filter",
    description: "One second-order filter, low-pass, high-pass, band-pass or notch, at a chosen frequency and Q, then a gain of −24 to +24 dB; an addition of the emulator, not a part of the original recorder.",
    latency_samples: 0,
    paths: PATH_RECORD,
    parameter_offset: 0,
    parameters: BAND_FILTER_PARAMETERS,
}];

/// Every insertion point (spec 024 D1).
pub fn insertion_points() -> &'static [InsertionPoint] {
    &INSERTION_POINTS
}

/// Every component, indexed 0 … count − 1 (spec 024 D2; product plan review M4); each entry
/// carries its value. A component fits a point when its paths include the point's.
pub fn components() -> &'static [Component] {
    &COMPONENTS
}

/// The component entry of `id`, if it is a component (not [`ComponentId::None`]).
pub fn component(id: ComponentId) -> Option<&'static Component> {
    COMPONENTS.iter().find(|c| c.id == id)
}

/// The global store's default values (each component's parameters at their defaults).
pub(crate) fn default_store() -> [f32; COMPONENT_PARAMETER_BUDGET] {
    let mut s = [0.0f32; COMPONENT_PARAMETER_BUDGET];
    for c in &COMPONENTS {
        for (i, p) in c.parameters.iter().enumerate() {
            s[c.parameter_offset as usize + i] = default_of(&p.value);
        }
    }
    s
}

fn default_of(v: &ControlValue) -> f32 {
    match *v {
        ControlValue::Choice { default, .. } => default.unwrap_or(0) as f32,
        ControlValue::Continuous { default, .. } => default,
        ControlValue::Integer { default, .. } => default as f32,
        ControlValue::Switch { default } => f32::from(u8::from(default)),
    }
}

/// The store index and the parameter of a control manifest index, if it is a component
/// parameter.
pub(crate) fn parameter_of_control(control: u32) -> Option<(usize, &'static ComponentParameter)> {
    for c in &COMPONENTS {
        for (i, p) in c.parameters.iter().enumerate() {
            if p.control_index == control {
                return Some((c.parameter_offset as usize + i, p));
            }
        }
    }
    None
}

/// The parameter at global store index `index`, if any.
pub(crate) fn parameter_at(
    index: usize,
) -> Option<(&'static Component, &'static ComponentParameter)> {
    for c in &COMPONENTS {
        let base = c.parameter_offset as usize;
        if index >= base && index < base + c.parameters.len() {
            return Some((c, &c.parameters[index - base]));
        }
    }
    None
}

/// A parameter value as the store holds it (research R-06): narrowed to `f32`, within the
/// range, a choice's value one of its options; −0.0 normalized. `None` otherwise.
pub(crate) fn store_value(p: &ComponentParameter, value: f64) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }
    let v = value as f32;
    match p.value {
        ControlValue::Continuous { min, max, .. } => (v >= min && v <= max).then_some(v + 0.0),
        ControlValue::Choice { options, .. } => (f64::from(value as u32) == value
            && options.iter().any(|o| o.value == value as u32))
        .then_some(v + 0.0),
        _ => None,
    }
}

/// The band filter's applied frequency ceiling at a host rate (spec 024 D4): 0.45 × the rate.
pub fn band_filter_frequency_ceiling_hz(host_rate_hz: u32) -> f64 {
    crate::stages::band_filter::CEILING * f64::from(host_rate_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_fields() {
        let p = &insertion_points()[0];
        assert_eq!(
            (p.identifier, p.control_index, p.paths),
            ("before_limiter", 31, PATH_RECORD)
        );
        let c = &components()[0];
        assert_eq!(
            (c.id, c.identifier, c.latency_samples, c.parameter_offset),
            (ComponentId::BandFilter, "band_filter", 0, 0)
        );
        assert_eq!(c.parameters.len(), 4);
        assert_eq!(
            c.parameters
                .iter()
                .map(|p| p.scale)
                .collect::<alloc::vec::Vec<_>>(),
            [Scale::Linear, Scale::Log, Scale::Log, Scale::Decibel]
        );
    }

    #[test]
    fn indexed_enumeration() {
        for (i, c) in components().iter().enumerate() {
            assert_ne!(c.id, ComponentId::None, "entry {i}");
            assert_eq!(component(c.id).unwrap().identifier, c.identifier);
            assert_eq!(ComponentId::from_u32(c.id as u32), Some(c.id));
        }
        assert!(component(ComponentId::None).is_none());
        assert_eq!(ComponentId::from_u32(2), None);
        assert_eq!(InsertionPointId::from_u32(1), None);
    }

    /// Plan R4: every component's parameters at its offset, together within the budget, no
    /// overlap.
    #[test]
    fn budget_offsets_no_overlap() {
        let mut used = [false; COMPONENT_PARAMETER_BUDGET];
        for c in components() {
            for i in 0..c.parameters.len() {
                let at = c.parameter_offset as usize + i;
                assert!(
                    at < COMPONENT_PARAMETER_BUDGET,
                    "{} past the budget",
                    c.identifier
                );
                assert!(!used[at], "{} overlaps at {at}", c.identifier);
                used[at] = true;
            }
        }
    }

    #[test]
    fn control_indices_match_control_manifest() {
        let all = crate::controls::controls();
        assert_eq!(
            all[CONTROL_INSERT as usize].identifier,
            "insert.before_limiter"
        );
        for p in components()[0].parameters {
            let e = &all[p.control_index as usize];
            assert_eq!(e.identifier, p.identifier);
            assert_eq!(e.value, p.value);
            assert_eq!(e.unit, p.unit);
        }
    }

    #[test]
    fn ceiling_function() {
        assert_eq!(band_filter_frequency_ceiling_hz(6000), 2700.0);
        assert_eq!(band_filter_frequency_ceiling_hz(48_000), 21_600.0);
    }

    #[test]
    fn store_values_validated() {
        let p = &components()[0].parameters;
        assert_eq!(store_value(&p[0], 3.0), Some(3.0));
        assert_eq!(store_value(&p[0], 4.0), None);
        assert_eq!(store_value(&p[0], 1.5), None);
        assert_eq!(store_value(&p[1], 19.0), None);
        assert_eq!(store_value(&p[1], 20_000.0), Some(20_000.0));
        assert_eq!(store_value(&p[3], -0.0).map(f32::to_bits), Some(0));
        assert_eq!(store_value(&p[2], f64::NAN), None);
        assert_eq!(default_store()[..4], [0.0, 1000.0, 0.7071, 0.0]);
    }
}
