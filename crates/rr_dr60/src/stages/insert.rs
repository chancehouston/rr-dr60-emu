//! The slot at the insertion point `before_limiter` (spec 024 D1, D3; research R-01, R-06):
//! none, or the engine's band filter, between stage 2 and the limiter at the host rate. An
//! addition of the emulator, not a part of the original recorder.

use crate::components::ComponentId;
use crate::stages::band_filter::BandFilter;

/// Deliberate defects for the spec 024 mutation tests (research › The six mutants); `Default`
/// is the committed behavior. Each is caught by a named oracle (the click rows, the type click
/// row, the trace, the filter → none → filter identity, the cross-stream test).
#[cfg(feature = "__test-hooks")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InsertMutation {
    /// The coefficients jump at P.
    pub param_step: bool,
    /// The mix switches at P.
    pub type_instant: bool,
    /// The frequency ramp in Hz, not log-frequency.
    pub freq_linear: bool,
    /// A second landing restarts from the old target.
    pub restart_from_target: bool,
    /// `set_component` keeps the filter's state.
    pub state_kept_on_set: bool,
    /// The call's epilogue does not write a landed parameter into the store, so the other
    /// stream never sees it (hardware post-implementation B11).
    pub skip_store_mirror: bool,
}

/// The applied-parameter trace's capacity (spec 024 contracts: 8192 reserved).
#[cfg(feature = "__test-hooks")]
pub(crate) const TRACE_CAPACITY: usize = 8192; // engineering target (024 contracts: the hook's reserve)

/// The slot: the component and its processor (fixed size; the band filter is built whether or
/// not it is in the slot, so setting a component never allocates).
#[derive(Clone, Debug)]
pub(crate) struct Insert {
    component: ComponentId,
    filter: BandFilter,
    #[cfg(feature = "__test-hooks")]
    pub(crate) mutation: InsertMutation,
    /// Test-only: the applied (frequency, Q, gain dB) per sample while tracing.
    #[cfg(feature = "__test-hooks")]
    pub(crate) trace: alloc::vec::Vec<(f64, f64, f64)>,
    /// Test-only: the slot's f64 input per sample while tracing (the placement identity).
    #[cfg(feature = "__test-hooks")]
    pub(crate) input_trace: alloc::vec::Vec<f64>,
    #[cfg(feature = "__test-hooks")]
    pub(crate) tracing: bool,
}

impl Insert {
    /// The slot's settle bound in host samples (spec 026 D2): the band filter's when it is in
    /// the slot, 0 for an empty slot.
    pub(crate) fn settle_bound(&self) -> u32 {
        if self.component == ComponentId::BandFilter {
            self.filter.settle_bound()
        } else {
            0
        }
    }

    /// A slot at `rate` holding `component`, its filter set from the store's band-filter values
    /// (indices 0 – 3, research R-06). Allocates only the test hooks' traces.
    pub(crate) fn new(rate: u32, component: ComponentId, values: [f32; 4]) -> Self {
        Self {
            component,
            filter: BandFilter::new(
                rate,
                crate::output::volume_transition_samples(rate).unwrap_or(rate / 50), // 20 ms (spec 011)
                values,
            ),
            #[cfg(feature = "__test-hooks")]
            mutation: InsertMutation::default(),
            // Reserved when tracing is switched on (a hook call, never a processing call), so
            // an engine's footprint does not carry them (spec 019's one-mebibyte lesson).
            #[cfg(feature = "__test-hooks")]
            trace: alloc::vec::Vec::new(),
            #[cfg(feature = "__test-hooks")]
            input_trace: alloc::vec::Vec::new(),
            #[cfg(feature = "__test-hooks")]
            tracing: false,
        }
    }

    /// Test-only: sets the mutant once (hardware post-implementation B10: never copied per
    /// sample, so a landing before the first processed sample sees it too).
    #[cfg(feature = "__test-hooks")]
    pub(crate) fn set_mutation(&mut self, m: InsertMutation) {
        self.mutation = m;
        self.filter.mutation = crate::stages::band_filter::BandFilterMutation {
            param_step: m.param_step,
            type_instant: m.type_instant,
            freq_linear: m.freq_linear,
            restart_from_target: m.restart_from_target,
        };
    }

    /// The slot's component.
    pub(crate) fn component(&self) -> ComponentId {
        self.component
    }

    /// Whether a component is in the slot (the path's `run_insert`).
    pub(crate) fn occupied(&self) -> bool {
        self.component != ComponentId::None
    }

    /// Sets the component (D3): the state cleared and the transitions ended at their targets.
    pub(crate) fn set_component(&mut self, c: ComponentId) {
        self.component = c;
        #[cfg(feature = "__test-hooks")]
        if self.mutation.state_kept_on_set {
            return;
        }
        self.filter.reset();
    }

    /// A band-filter parameter lands (index 0 – 3 of the store): at once with the slot empty or
    /// when `at_once` (a stream's first input sample).
    pub(crate) fn land(&mut self, index: usize, value: f32, at_once: bool) {
        let at_once = at_once || !self.occupied();
        self.filter.land(index, value, at_once);
    }

    /// The state cleared, the transitions at their targets (`RecordPath::reset`; plan R2).
    pub(crate) fn reset(&mut self) {
        self.filter.reset();
    }

    /// One host sample through the slot (called only while occupied).
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        #[cfg(feature = "__test-hooks")]
        if self.tracing && self.input_trace.len() < TRACE_CAPACITY {
            self.input_trace.push(x);
        }
        let y = self.filter.process(x);
        #[cfg(feature = "__test-hooks")]
        if self.tracing && self.trace.len() < TRACE_CAPACITY {
            self.trace.push(self.filter.applied());
        }
        y
    }
}
