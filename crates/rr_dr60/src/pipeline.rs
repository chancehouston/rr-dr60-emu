//! The per-sample processing engine (data-model.md; research.md R-09, R-10).

use crate::error::Error;
use crate::rate::RatePlan;
use crate::resample::{self, PolyphaseDown, PolyphaseUp};
use crate::sanitize::{narrow_out, sanitize_in};
use crate::settings::{DEVICE_RATE_HZ, Settings, Tap};
use crate::stages::voiceband::VoiceBandStage;

/// One emulator instance: host-rate mono audio in, host-rate mono audio out.
///
/// Internally, each sample goes through the 8 kHz device-rate domain (A-001), the record
/// band-limit stage (signal-chain stage 4) and the playback band-limit stage (stage 10).
/// Each stage can be bypassed, and the output can be taken after stage 4 instead
/// ([`Settings`], FR-007). The device rate itself is always applied (FR-004).
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
    chain: DeviceChain,
    latency: u32,
}

/// The device-rate part of the chain (stages 4 and 10) and which parts of it run.
#[derive(Clone, Debug)]
struct DeviceChain {
    record: VoiceBandStage,
    playback: VoiceBandStage,
    /// Stage 4 runs (FR-007).
    run_record: bool,
    /// Stage 10 runs: enabled and the tap is after playback (data-model.md › Tap).
    run_playback: bool,
}

impl DeviceChain {
    fn new(settings: &Settings) -> Self {
        Self {
            record: VoiceBandStage::new(),
            playback: VoiceBandStage::new(),
            run_record: settings.record_stage_enabled,
            run_playback: settings.playback_stage_enabled && settings.tap == Tap::AfterPlayback,
        }
    }

    /// One device-rate sample. A bypassed stage passes the sample through bit-exactly,
    /// with no arithmetic (FR-007).
    #[inline]
    fn process(&mut self, mut d: f64) -> f64 {
        if self.run_record {
            d = self.record.process(d);
        }
        if self.run_playback {
            d = self.playback.process(d);
        }
        d
    }

    /// Group delay at 1 kHz of the stages that run, in device samples (R-10).
    fn group_delay_1k_samples(&self) -> f64 {
        let mut tau = 0.0;
        if self.run_record {
            tau += self.record.group_delay_1k_samples();
        }
        if self.run_playback {
            tau += self.playback.group_delay_1k_samples();
        }
        tau
    }

    fn reset(&mut self) {
        self.record.reset();
        self.playback.reset();
    }
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
        let chain = DeviceChain::new(&settings);
        let latency = latency_samples(&plan, &settings, &chain);
        Ok(Self {
            settings,
            converters: resample::converters(&plan),
            chain,
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
        self.chain.reset();
    }

    /// Applies new settings and resets all state, as if the pipeline had been created anew
    /// with `settings` (FR-008). The new configuration is built completely before it replaces
    /// the old one, so on error the pipeline is unchanged: same settings, latency and state.
    ///
    /// May allocate; **not** real-time safe. Call it between streams, not from the audio
    /// callback.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`] if `settings.host_rate_hz` is not supported.
    pub fn reconfigure(&mut self, settings: Settings) -> Result<(), Error> {
        *self = Self::new(settings)?;
        Ok(())
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
            None => self.chain.process(x),
            Some((down, up)) => {
                if let Some(d) = down.push(x) {
                    up.push_device(self.chain.process(d));
                }
                up.next_host()
            }
        };
        narrow_out(y)
    }
}

/// R-10: round(D_down + D_up + (τ_rec + τ_play) · host / 8000). A bypassed stage contributes
/// 0, and with the tap after the record stage τ_play is not included.
fn latency_samples(plan: &RatePlan, settings: &Settings, chain: &DeviceChain) -> u32 {
    let stages = chain.group_delay_1k_samples();
    let total = 2.0 * f64::from(plan.delay_host_samples())
        + stages * f64::from(settings.host_rate_hz) / f64::from(DEVICE_RATE_HZ);
    (total + 0.5) as u32
}
