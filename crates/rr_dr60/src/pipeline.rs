//! The per-sample processing engine (data-model.md; research.md R-09, R-10).

use crate::error::Error;
use crate::rate::RatePlan;
use crate::resample::{self, PolyphaseDown, PolyphaseUp};
use crate::sanitize::{narrow_out, sanitize_in};
use crate::settings::{DEVICE_RATE_HZ, Settings};
use crate::stages::voiceband::VoiceBandStage;

/// One emulator instance: host-rate mono audio in, host-rate mono audio out.
///
/// Internally, each sample goes through the 8 kHz device-rate domain (A-001), the record
/// band-limit stage (signal-chain stage 4) and the playback band-limit stage (stage 10).
/// Both stages are modeled on the MSM7702 voice-band codec's assumed 300–3400 Hz band
/// (A-002, A-014).
///
/// # Real-time use
///
/// [`process`](Self::process), [`process_in_place`](Self::process_in_place) and
/// [`reset`](Self::reset) never allocate, lock or do I/O (FR-015, FR-017). Output is
/// bit-identical for any way of splitting the input into blocks (FR-014).
/// [`new`](Self::new) allocates and is not real-time safe.
///
/// # Example
///
/// ```
/// use rr_dr60::{Pipeline, Settings};
///
/// let mut p = Pipeline::new(Settings::new(48_000))?;
/// let mut buffer = vec![0.0f32; 512];
/// p.process_in_place(&mut buffer); // in the audio callback
/// assert!(p.latency_samples() > 0);
/// # Ok::<(), rr_dr60::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Pipeline {
    settings: Settings,
    converters: Option<(PolyphaseDown, PolyphaseUp)>,
    record: VoiceBandStage,
    playback: VoiceBandStage,
    latency: u32,
}

impl Pipeline {
    /// Validates `settings` and allocates every buffer.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not in
    /// [`SUPPORTED_HOST_RATES`](crate::SUPPORTED_HOST_RATES).
    pub fn new(settings: Settings) -> Result<Self, Error> {
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        let record = VoiceBandStage::new();
        let playback = VoiceBandStage::new();
        let latency = latency_samples(&plan, &settings, &record, &playback);
        Ok(Self {
            settings,
            converters: resample::converters(&plan),
            record,
            playback,
            latency,
        })
    }

    /// Processes one block. `input` and `output` must have the same length, which may be
    /// any length including 0. Real-time safe.
    ///
    /// # Errors
    ///
    /// [`Error::LengthMismatch`] if the lengths differ. The pipeline state is not changed.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), Error> {
        if input.len() != output.len() {
            return Err(Error::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        for (x, y) in input.iter().zip(output.iter_mut()) {
            *y = self.tick(*x);
        }
        Ok(())
    }

    /// Processes one block in place. Real-time safe.
    pub fn process_in_place(&mut self, buffer: &mut [f32]) {
        for v in buffer.iter_mut() {
            *v = self.tick(*v);
        }
    }

    /// Fixed latency of the current configuration, in host-rate samples (FR-012).
    ///
    /// This is the rate conversion's delay plus the voice-band stages' group delay at 1 kHz,
    /// rounded to the nearest sample (research.md R-10).
    pub fn latency_samples(&self) -> u32 {
        self.latency
    }

    /// Returns the pipeline to its freshly created state, without allocating (FR-017).
    pub fn reset(&mut self) {
        if let Some((down, up)) = &mut self.converters {
            down.reset();
            up.reset();
        }
        self.record.reset();
        self.playback.reset();
    }

    /// The current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// One host sample through the engine (data-model.md steps 1–3).
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let x = sanitize_in(x);
        let y = match &mut self.converters {
            None => device_chain(&mut self.record, &mut self.playback, x),
            Some((down, up)) => {
                if let Some(d) = down.push(x) {
                    up.push_device(device_chain(&mut self.record, &mut self.playback, d));
                }
                up.next_host()
            }
        };
        narrow_out(y)
    }
}

/// The device-rate part of the chain: stage 4, then stage 10.
#[inline]
fn device_chain(record: &mut VoiceBandStage, playback: &mut VoiceBandStage, d: f64) -> f64 {
    playback.process(record.process(d))
}

/// R-10: round(D_down + D_up + (τ_rec + τ_play) · host / 8000).
fn latency_samples(
    plan: &RatePlan,
    settings: &Settings,
    record: &VoiceBandStage,
    playback: &VoiceBandStage,
) -> u32 {
    let stages = record.group_delay_1k_samples() + playback.group_delay_1k_samples();
    let total = 2.0 * f64::from(plan.delay_host_samples())
        + stages * f64::from(settings.host_rate_hz) / f64::from(DEVICE_RATE_HZ);
    (total + 0.5) as u32
}
