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
/// # Threads
///
/// `Pipeline` is `Send + Sync`. It can move to the audio thread, and `&Pipeline` (for
/// [`latency_samples`](Self::latency_samples) or [`settings`](Self::settings)) can be shared.
/// Processing takes `&mut self`, so only one thread processes at a time.
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

// Contract (contracts/rust-api.md): `Pipeline` is Send + Sync. Fails to compile if a future
// field (e.g. an Rc or Cell) would silently break that.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Pipeline>();
};

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

#[cfg(feature = "__test-hooks")]
impl Pipeline {
    /// Test-only (hidden): a pipeline whose voice-band stages use `sos` instead of the
    /// committed coefficients. Used by the SC-008 mutation test (tasks.md T057).
    ///
    /// # Errors
    ///
    /// As [`Pipeline::new`].
    #[doc(hidden)]
    pub fn __with_stage_sos(settings: Settings, sos: [[f64; 5]; 6]) -> Result<Self, Error> {
        let mut p = Self::new(settings)?;
        p.chain.record = VoiceBandStage::with_sos(sos);
        p.chain.playback = VoiceBandStage::with_sos(sos);
        let plan = RatePlan::for_host(settings.host_rate_hz)?;
        p.latency = latency_samples(&plan, &settings, &p.chain);
        Ok(p)
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

#[cfg(test)]
mod validation_tests {
    extern crate std;
    use super::*;
    use crate::error::Setting;
    use std::vec::Vec;

    /// 002 T006: AGC settings are validated on creation (FR-011).
    #[test]
    fn new_rejects_invalid_agc_setting() {
        let mut s = Settings::new(48_000);
        s.agc.attack_ms = 0.0;
        assert_eq!(
            Pipeline::new(s).err(),
            Some(Error::InvalidSetting {
                setting: Setting::AgcAttackMs
            })
        );
    }

    /// 002 T006: a failed reconfigure leaves settings, latency and state unchanged (FR-011,
    /// 001 FR-008).
    #[test]
    fn failed_reconfigure_leaves_pipeline_unchanged() {
        let x: Vec<f32> = (0..4800).map(|n| if n % 50 == 0 { 0.5 } else { 0.0 }).collect();
        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        let mut warm = x.clone();
        p.process_in_place(&mut warm);
        let mut untouched = p.clone();

        let mut bad = Settings::new(48_000);
        bad.agc.release_ms = f32::NAN;
        assert_eq!(
            p.reconfigure(bad),
            Err(Error::InvalidSetting {
                setting: Setting::AgcReleaseMs
            })
        );
        assert_eq!(p.settings(), untouched.settings());
        assert_eq!(p.latency_samples(), untouched.latency_samples());
        let (mut a, mut b) = (x.clone(), x);
        p.process_in_place(&mut a);
        untouched.process_in_place(&mut b);
        assert_eq!(a, b);
    }
}

#[cfg(all(test, feature = "op-count"))]
mod op_count_tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    impl Pipeline {
        fn ops(&self) -> u64 {
            let conv = self.converters.as_ref().map_or(0, |(d, u)| d.ops + u.ops);
            conv + self.chain.record.ops() + self.chain.playback.ops()
        }

        fn taps(&self) -> (u64, u64) {
            self.converters
                .as_ref()
                .map_or((0, 0), |(d, u)| (d.taps() as u64, u.taps() as u64))
        }
    }

    /// FR-015 bounded work: no host sample costs more than one full decimator branch, one
    /// interpolator branch and both stages (2 × 6 sections × 5 multiply-adds), and the total is
    /// independent of how the input is split into blocks.
    #[test]
    fn per_sample_work_is_bounded_and_block_independent() {
        for rate in crate::SUPPORTED_HOST_RATES {
            let x: Vec<f32> = (0..rate as usize / 4)
                .map(|n| if n % 97 == 0 { 0.5 } else { -0.01 })
                .collect();
            let mut one = Pipeline::new(Settings::new(rate)).unwrap();
            let (taps_down, taps_up) = one.taps();
            let bound = taps_down + taps_up + 60;
            let mut worst = 0;
            for &v in &x {
                let before = one.ops();
                one.process_in_place(&mut [v]);
                worst = worst.max(one.ops() - before);
            }
            assert!(
                worst <= bound,
                "{rate} Hz: {worst} multiply-adds in one sample > bound {bound}"
            );
            assert!(worst > 0);
            let mut block = Pipeline::new(Settings::new(rate)).unwrap();
            let mut buf = x.clone();
            block.process_in_place(&mut buf);
            assert_eq!(
                block.ops(),
                one.ops(),
                "{rate} Hz: work depends on block size"
            );
        }
    }
}
