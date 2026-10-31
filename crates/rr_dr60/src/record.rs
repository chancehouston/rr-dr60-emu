//! The processing record (spec 020): one versioned text document per recording that says
//! how the recording was processed, written beside the stored form and never inside it; its
//! reader; the stop fields a run carries.

use crate::transport::{RecordInfo, StopOutcome, StopResult};

/// How a recording ended (spec 020 D9; research R-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StopHow {
    /// The host called `stop_recording` (also when its flush filled the memory).
    #[default]
    StopCall,
    /// The memory filled during a `record` call (the only self-stop, spec 006 FR-012).
    MemoryFull,
}

/// The stop's figures a run carries (spec 020 D9, D10): plain and buildable by anyone, from a
/// [`StopOutcome`], from the [`RecordInfo`] of a self-stop, or from a read record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StopFields {
    /// How the recording ended.
    pub how: StopHow,
    /// The host samples fed before the stop was called; for a self-stop the input samples
    /// total (nothing after the fill is fed).
    pub stop_input_position: u64,
    /// The kept device sample at which the codec's padding begins; the length without
    /// padding. 0 from a self-stop's `RecordInfo`: the writer fills the length in.
    pub padding_start_sample: u64,
    /// Every host sample fed since the start, the stop's silence included (spec 017).
    pub input_samples_total: u64,
    /// The post-gate device samples of the padding (≤ 179; 0 without the codec or after a
    /// self-stop).
    pub silence_fed_samples: u64,
}

impl From<&StopOutcome> for StopFields {
    fn from(o: &StopOutcome) -> Self {
        Self {
            how: StopHow::StopCall,
            stop_input_position: o.stop_input_position,
            padding_start_sample: o.padding_start_sample,
            input_samples_total: o.input_samples_total,
            silence_fed_samples: o.silence_fed_samples,
        }
    }
}

impl From<&RecordInfo> for StopFields {
    /// The self-stop's fields (spec 020 research R-07) from the `record` call's report with
    /// `stopped` set: nothing after the fill was fed, no padding.
    fn from(i: &RecordInfo) -> Self {
        Self {
            how: StopHow::MemoryFull,
            stop_input_position: i.input_samples_total,
            padding_start_sample: 0,
            input_samples_total: i.input_samples_total,
            silence_fed_samples: 0,
        }
    }
}

impl StopFields {
    /// `true` when the outcome kept the recording (the fields of a discarded one are
    /// meaningless).
    #[must_use]
    pub fn kept(o: &StopOutcome) -> bool {
        matches!(o.result, StopResult::Kept { .. })
    }
}
