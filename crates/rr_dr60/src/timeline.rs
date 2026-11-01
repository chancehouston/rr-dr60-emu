//! The timeline maps (spec 020 D8, FR-006; research R-01, R-04): pure integer functions of a
//! processing record's data, so an app can place a stored sample, a change or a
//! voice-activation gap on the microphone's own clock, and a playback output on the
//! recording's place. Nothing here touches the audio.
//!
//! The alignment is a **convention**: the record path's latency is its 1 kHz group delay
//! rounded to a host sample (spec 006), and the window is the decimator's kernel half-length
//! (the record filter is IIR and has no finite support). It states a direct contribution: the
//! limiter's gain, the gate and the coder's state depend on earlier input.

use crate::changes::{ChangeEntry, speed_landing};
use crate::error::Error;
use crate::pipeline::VasEvent;
use crate::rate::RatePlan;
use crate::record::StopFields;
use crate::settings::Settings;

/// A kept sample's nominal host-input alignment (map (a)): the centre and the direct-path
/// window, in host input samples since the recording's start; `clamped` when an end was
/// pulled into the recording's input range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct InputAlignment {
    /// The host input sample the kept sample aligns with.
    pub centre: u64,
    /// The window's first host sample.
    pub first: u64,
    /// The window's last host sample.
    pub last: u64,
    /// An end was clamped (before the start: the latency; past the stop: the flush and the
    /// padding).
    pub clamped: bool,
}

/// Where a host-input sample went (map (b)). Not clamped at the end: a position past the stop's
/// input position maps past the recording (a kept index at or past the stored length means a
/// gap still open at the stop, or the flush); compare against the stop fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptOrRemoved {
    /// Kept, at this stored device sample.
    Kept(u64),
    /// Removed by the voice activation (drop mode): the event (its index in the record's rows)
    /// that removed it.
    Removed {
        /// The event's index.
        event_index: u64,
    },
    /// Stored as a muted zero (mute mode): the kept sample and the region's event.
    Muted {
        /// The stored device sample.
        kept: u64,
        /// The region's event index.
        event_index: u64,
    },
}

/// The speed control's place at a playback device output (map (c)); see [`place_at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PlaceState {
    /// The place of the output, in fifths of a stored sample.
    pub place_fifths: u64,
    /// The level computing it (1 … 5).
    pub level: u8,
    /// The output was a blend sample (the old branch contributed; the last blend sample is the
    /// new branch itself and reads `false`).
    pub fading: bool,
    /// The old branch's place for a blend sample, in fifths; 0 otherwise.
    pub old_place_fifths: u64,
    /// The place has reached the recording's end after this output: the level is fixed.
    pub latched: bool,
}

/// The model coder's reach around a frame, in kept samples before and after the frame's start
/// (research R-01, corrected at implementation): before it the larger of the past excitation
/// the pitch search reads and the LPC window's past input; after it the frame and its
/// look-ahead. Derived from the coder's own constants (A-040, A-041: a class match, not the
/// device's coder).
const CODEC_WINDOW_BEFORE: u64 = crate::codec::PAST_REACH as u64;
const CODEC_WINDOW_AFTER: u64 = FRAME + crate::codec::LOOKAHEAD_SAMPLES as u64 - 1;
const FRAME: u64 = crate::codec::FRAME_SAMPLES as u64; // A-040: the codec's 20 ms frame at 6 kHz

/// The removed device samples at or before kept sample `k` in drop mode; 0 in mute mode, where
/// a region is stored (hardware review B1 at implementation).
fn removed_through(k: u64, events: &[VasEvent], drop: bool) -> u64 {
    if !drop {
        return 0;
    }
    events
        .iter()
        .take_while(|e| e.output_position <= k)
        .map(|e| e.input_length)
        .sum()
}

/// Whether `settings` drop the gate's pauses (else they are stored as zeros, or there is no
/// gate).
fn drops(settings: &Settings) -> bool {
    settings.vas.enabled && settings.vas.mode == crate::settings::VasMode::Drop
}

/// The alignment's centre of a pre-gate device index: ⌊d · m ÷ l⌋ − L_pre, clamped at 0.
fn centre_of(pre_gate: u64, l: u64, m: u64, latency: u64) -> (u64, bool) {
    crate::record::host_centre(pre_gate, l, m, latency)
}

/// Map (a): the nominal host-input alignment of kept (stored) device sample `k` of a recording
/// made with `settings`, through its voice-activation `events` and its stop (spec 020 D8):
/// centre ⌊(k + R(k)) · m ÷ l⌋ − L_pre with R(k) the device samples removed at or before k and
/// L_pre the record path's latency to the encoder's input; the window ± the decimator's
/// half-length in host samples, widened with the speech codec on to the frame's analysis
/// window; the centre and the ends clamped to the audio the host holds (0 … the stop's input
/// position − 1; a kept sample of the flush or the padding reads clamped). In mute mode nothing
/// is removed (R ≡ 0).
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`].
pub fn kept_to_input(
    k: u64,
    settings: &Settings,
    events: &[VasEvent],
    stop: &StopFields,
) -> Result<InputAlignment, Error> {
    let plan = RatePlan::for_host(settings.host_rate_hz)?;
    let (l, m) = crate::paths::l_m(&plan);
    let latency = u64::from(crate::paths::record_latency_pre_encoder_for(settings)?);
    let half = u64::from(plan.delay_host_samples());
    let drop = drops(settings);
    let (centre, mut clamped) = centre_of(k + removed_through(k, events, drop), l, m, latency);
    let (mut first, mut last) = (centre.saturating_sub(half), centre.saturating_add(half));
    if centre < half {
        clamped = true;
    }
    if settings.speech_codec_enabled {
        let frame_start = k / FRAME * FRAME;
        let lo = frame_start.saturating_sub(CODEC_WINDOW_BEFORE);
        if frame_start < CODEC_WINDOW_BEFORE {
            clamped = true;
        }
        let hi = frame_start + CODEC_WINDOW_AFTER;
        let (lo_c, lo_clamped) = centre_of(lo + removed_through(lo, events, drop), l, m, latency);
        let (hi_c, _) = centre_of(hi + removed_through(hi, events, drop), l, m, latency);
        clamped |= lo_clamped;
        first = first.min(lo_c.saturating_sub(half));
        last = last.max(hi_c.saturating_add(half));
    }
    let end = stop.stop_input_position.saturating_sub(1);
    let mut centre = centre;
    if stop.stop_input_position > 0 {
        if last > end {
            last = end;
            clamped = true;
        }
        if centre > end {
            centre = end;
            clamped = true;
        }
    }
    if first > last {
        first = last;
    }
    Ok(InputAlignment {
        centre,
        first,
        last,
        clamped,
    })
}

/// Map (b): the kept device sample host-input sample `p` reaches, or the voice-activation
/// event that removed it (spec 020 D8): the pre-gate device index j = ⌈(p + L_pre) · l ÷ m⌉
/// (spec 017's `record_landing`), then the events' removed regions [`output_position` +
/// R_before, + `input_length`) in pre-gate indices. Mute mode removes nothing (the regions are
/// reported by the events but every sample is kept).
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`].
pub fn input_to_kept(
    p: u64,
    settings: &Settings,
    events: &[VasEvent],
) -> Result<KeptOrRemoved, Error> {
    let latency = u64::from(crate::paths::record_latency_pre_encoder_for(settings)?);
    let j = crate::changes::record_landing(p + latency, settings.host_rate_hz)?;
    let drop = drops(settings);
    let mut removed_before = 0u64;
    for (i, e) in events.iter().enumerate() {
        let start = e.output_position + removed_before;
        if j < start {
            break;
        }
        if j < start + e.input_length {
            return Ok(if drop {
                KeptOrRemoved::Removed {
                    event_index: i as u64,
                }
            } else {
                KeptOrRemoved::Muted {
                    kept: j,
                    event_index: i as u64,
                }
            });
        }
        if drop {
            removed_before += e.input_length;
        }
    }
    Ok(KeptOrRemoved::Kept(j - removed_before))
}

/// The one-pass's clocks (spec 015 and 017's pointer to 020): the input-clock position an
/// output-clock `position` aligns with, through the one-pass's `latency` (the engine's
/// [`latency_samples`](crate::Pipeline::latency_samples), host samples) and, with the gate in
/// `drop` mode, the host samples its voice-activation `events` removed at or before the output
/// (in mute mode nothing is removed); `(input, clamped)` with the clamp at 0 (the first outputs
/// align with inputs before the stream's start).
#[must_use]
pub fn onepass_output_to_input(
    position: u64,
    latency: u32,
    events: &[VasEvent],
    drop: bool,
) -> (u64, bool) {
    let removed: u64 = if drop {
        events
            .iter()
            .take_while(|e| e.output_position <= position)
            .map(|e| e.input_length)
            .sum()
    } else {
        0
    };
    let raw = position + removed;
    let latency = u64::from(latency);
    if raw < latency {
        (0, true)
    } else {
        (raw - latency, false)
    }
}

/// The speed control's place automaton replayed over a schedule (research R-04: the
/// independent walker, pinned against the stage's own per-output trace): the latch at the
/// recording's end, the wait during a crossfade, the deferred switch into level 3, the
/// first-output switch, the crossfade's length.
#[derive(Clone, Copy, Debug)]
struct Walker {
    p5: u64,
    active: u32,
    requested: u32,
    latched: bool,
    fade: Option<(u32, u64, u32)>, // (old k, old place in fifths, n)
    started: bool,
    stored: u64,
    /// Outputs computed so far.
    outputs: u64,
    /// The outputs before the drain's silence, fixed at the latch (the stage's `final_count`).
    final_count: Option<u64>,
    /// The state held past the final count (the stage emits silence without moving).
    held: Option<PlaceState>,
    /// The last computed state.
    last: PlaceState,
    /// Spec 023: the mode in force and the request (0 pitch with speed, 1 pitch kept), and the
    /// time-scaler's grid: anchored, and the outputs since the anchor (its `n`).
    mode: u8,
    requested_mode: u8,
    sc_active: bool,
    sc_n: u64,
}

const K_IDENTITY: u32 = 5; // 013 FR-002: level 3's ratio 5/5 (A-054)

impl Walker {
    fn new(level: u8, stored: u64) -> Self {
        let k = crate::stages::speed::numerator(level);
        Self {
            p5: 0,
            active: k,
            requested: k,
            latched: stored == 0,
            fade: None,
            started: false,
            stored,
            outputs: 0,
            final_count: None,
            held: None,
            last: PlaceState::default(),
            mode: 0,
            requested_mode: 0,
            sc_active: false,
            sc_n: 0,
        }
    }

    /// The latch (spec 013 FR-005): the level is fixed and the final count known: the outputs
    /// so far plus the tail still to play from the current place at the active level (at
    /// level 3 no tail).
    fn latch(&mut self) {
        if self.latched || self.p5 < 5 * self.stored {
            return;
        }
        self.latched = true;
        let d = u64::from(crate::speed::LOOKAHEAD_SAMPLES);
        let mut remaining = 0u64;
        if self.active != K_IDENTITY && self.mode == 1 {
            // Spec 023 research R-06: the grid bound T_c from the hop in progress.
            remaining = crate::stages::speed::pitch_kept_tail(self.sc_n, 0);
        } else if self.active != K_IDENTITY {
            let mut p5 = self.p5;
            loop {
                let (i, ph) = (p5 / 5, p5 % 5);
                if i + u64::from(ph != 0) > self.stored.saturating_sub(1) + d {
                    break;
                }
                remaining += 1;
                p5 += u64::from(self.active);
            }
        }
        self.final_count = Some(self.outputs + remaining);
    }

    fn apply_request(&mut self) {
        if self.latched || self.requested == self.active || self.fade.is_some() {
            return;
        }
        if self.requested == K_IDENTITY && self.p5 % 5 != 0 {
            return;
        }
        let old = self.active;
        self.active = self.requested;
        if self.mode == 1 {
            // Spec 023 D5, D12: between levels 1, 2, 4 and 5 no fade; into level 3 a fade from
            // the frozen scaler; out of level 3 the grid anchors (no fade).
            if self.active == K_IDENTITY {
                if self.started {
                    self.fade = Some((old, self.p5, 0));
                }
                self.sc_active = false;
            }
            return;
        }
        if self.started {
            self.fade = Some((old, self.p5, 0));
        }
    }

    /// Spec 023 D5: the mode request under the shared wait rule; at level 3 at once.
    fn apply_mode_request(&mut self) {
        if self.latched || self.requested_mode == self.mode || self.fade.is_some() {
            return;
        }
        self.mode = self.requested_mode;
        if self.active != K_IDENTITY && self.started {
            self.fade = Some((self.active, self.p5, 0));
        }
        self.sc_active = false;
    }

    /// One output: the state it was computed under; past the final count the held state (the
    /// stage emits the drain's silence without moving, spec 020 T029).
    fn step(&mut self) -> PlaceState {
        if let Some(h) = self.held {
            return h;
        }
        self.latch();
        if self.final_count.is_some_and(|f| self.outputs >= f) {
            let h = self.last_state();
            self.held = Some(h);
            return h;
        }
        self.apply_request();
        self.apply_mode_request(); // 023
        if self.mode == 1 && self.active != K_IDENTITY {
            // The time-scaler's grid: anchored at this output when it is not (023 R-02).
            if !self.sc_active {
                self.sc_active = true;
                self.sc_n = 0;
            }
            self.sc_n += 1;
        }
        let mut st = PlaceState {
            place_fifths: self.p5,
            level: level_of(self.active),
            fading: false,
            old_place_fifths: 0,
            latched: self.latched,
        };
        if let Some((old_k, old_p5, n)) = self.fade {
            let n = n + 1;
            if n < crate::speed::CROSSFADE_SAMPLES {
                st.fading = true;
                st.old_place_fifths = old_p5;
                self.fade = Some((old_k, old_p5 + u64::from(old_k), n));
            } else {
                self.fade = None;
            }
        }
        self.started = true;
        self.p5 += u64::from(self.active);
        self.outputs += 1;
        self.latch();
        st.latched = self.latched;
        self.last = st;
        st
    }

    /// The last computed state (held past the final count).
    fn last_state(&self) -> PlaceState {
        self.last
    }
}

/// The level of a numerator.
fn level_of(k: u32) -> u8 {
    match k {
        3 => 1,
        4 => 2,
        5 => 3,
        6 => 4,
        _ => 5,
    }
}

/// Map (c): the speed control's place at playback **device output** `index` (0 = the first
/// device sample the playback computes; what is heard follows after
/// [`playback_latency_samples`](crate::Pipeline::playback_latency_samples)) for a recording of
/// `unit_count` stored device samples played from `start_level` with the `playback_speed`
/// entries of `entries` (the playback stream's change log, spec 015; other controls' entries
/// are ignored) landing at `speed_landing(position, host_rate_hz)`: a replay of the speed
/// stage's own rules (spec 013's deferred switch and latch, spec 019's crossfade and wait).
/// Past the stage's final count (the own part and the tail) the state is held: the drain's
/// silence computes no place (T029). Entries must be in position order.
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`]; [`Error::InvalidSpeedLevel`] for an entry whose value is
/// not a level 1 … 5.
pub fn place_at(
    index: u64,
    host_rate_hz: u32,
    start_level: u8,
    unit_count: u64,
    entries: &[ChangeEntry],
) -> Result<PlaceState, Error> {
    if !(1..=5).contains(&start_level) {
        return Err(Error::InvalidSpeedLevel { level: start_level });
    }
    let speed_control = crate::changes::LIVE_SPEED;
    let mut w = Walker::new(start_level, unit_count);
    let mut pending = entries
        .iter()
        .filter(|e| e.control == speed_control)
        .map(|e| {
            let level = e.after as u8;
            if !(1..=5).contains(&level) || f64::from(level) != e.after {
                return Err(Error::InvalidSpeedLevel { level });
            }
            Ok((speed_landing(e.position, host_rate_hz)?, level))
        })
        .collect::<Result<alloc::vec::Vec<_>, Error>>()?
        .into_iter()
        .peekable();
    let mut st = PlaceState::default();
    for i in 0..=index {
        while let Some(&(d, level)) = pending.peek() {
            if d > i {
                break;
            }
            w.requested = crate::stages::speed::numerator(level);
            pending.next();
        }
        st = w.step();
    }
    Ok(st)
}

/// Map (c) in a playback speed mode (spec 023 research R-06b): as [`place_at`] for a playback
/// started in `start_mode` (0 pitch with speed, 1 pitch kept) at `start_level`, replaying the
/// `playback_speed` **and** `playback_speed_mode` entries of `entries` (a level change in
/// pitch-kept mode has no blend; a mode change, and a change into level 3 in pitch-kept mode,
/// blend over 120 outputs with the old branch's own place reported; the pitch-kept final
/// count is the own part plus the grid tail T_c). With `start_mode` 0 and no mode entry it
/// equals [`place_at`].
///
/// # Errors
///
/// [`Error::UnsupportedHostRate`]; [`Error::InvalidSpeedLevel`] for an entry whose value is
/// not a level 1 … 5; [`Error::InvalidSpeedMode`] for `start_mode` or a mode entry not 0 or 1.
pub fn place_at_in_mode(
    index: u64,
    host_rate_hz: u32,
    start_level: u8,
    start_mode: u32,
    unit_count: u64,
    entries: &[ChangeEntry],
) -> Result<PlaceState, Error> {
    if !(1..=5).contains(&start_level) {
        return Err(Error::InvalidSpeedLevel { level: start_level });
    }
    if start_mode > 1 {
        return Err(Error::InvalidSpeedMode { mode: start_mode });
    }
    let (speed_control, mode_control) =
        (crate::changes::LIVE_SPEED, crate::changes::LIVE_SPEED_MODE);
    let mut w = Walker::new(start_level, unit_count);
    w.mode = start_mode as u8;
    w.requested_mode = w.mode;
    let mut pending = entries
        .iter()
        .filter(|e| e.control == speed_control || e.control == mode_control)
        .map(|e| {
            if e.control == speed_control {
                let level = e.after as u8;
                if !(1..=5).contains(&level) || f64::from(level) != e.after {
                    return Err(Error::InvalidSpeedLevel { level });
                }
                Ok((speed_landing(e.position, host_rate_hz)?, false, level))
            } else {
                if !(e.after == 0.0 || e.after == 1.0) {
                    return Err(Error::InvalidSpeedMode {
                        mode: if e.after.is_finite() && e.after >= 0.0 && e.after < 4294967296.0 {
                            e.after as u32
                        } else {
                            u32::MAX
                        },
                    });
                }
                Ok((
                    speed_landing(e.position, host_rate_hz)?,
                    true,
                    e.after as u8,
                ))
            }
        })
        .collect::<Result<alloc::vec::Vec<_>, Error>>()?
        .into_iter()
        .peekable();
    let mut st = PlaceState::default();
    for i in 0..=index {
        while let Some(&(d, is_mode, v)) = pending.peek() {
            if d > i {
                break;
            }
            if is_mode {
                w.requested_mode = v;
                // The stage applies a mode at once at level 3 with nothing pending (D5).
                if w.active == K_IDENTITY
                    && w.requested == K_IDENTITY
                    && w.fade.is_none()
                    && !w.latched
                {
                    w.mode = v;
                    w.sc_active = false;
                }
            } else {
                w.requested = crate::stages::speed::numerator(v);
            }
            pending.next();
        }
        st = w.step();
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::SUPPORTED_HOST_RATES;
    use crate::record::StopHow;

    fn settings(rate: u32, codec: bool) -> Settings {
        let mut s = Settings::new(rate);
        s.speech_codec_enabled = codec;
        s
    }

    fn stop(total: u64) -> StopFields {
        StopFields {
            how: StopHow::StopCall,
            stop_input_position: total,
            padding_start_sample: 0,
            input_samples_total: total,
            silence_fed_samples: 0,
        }
    }

    /// Map (a) against the hand formula at seven rates, with and without events.
    #[test]
    fn alignment_at_every_rate() {
        let events = [VasEvent::new(600, 300), VasEvent::new(1800, 120)];
        for rate in SUPPORTED_HOST_RATES {
            let s = settings(rate, false);
            let plan = RatePlan::for_host(rate).unwrap();
            let (l, m) = crate::paths::l_m(&plan);
            let lat = u64::from(crate::paths::record_latency_pre_encoder_for(&s).unwrap());
            let half = u64::from(plan.delay_host_samples());
            for k in [0u64, 100, 600, 1000, 1800, 5000] {
                let a = kept_to_input(k, &s, &events, &stop(u64::MAX)).unwrap();
                let r = if k >= 1800 {
                    420
                } else if k >= 600 {
                    300
                } else {
                    0
                };
                let want = ((k + r) * m / l).saturating_sub(lat);
                assert_eq!(a.centre, want, "{rate} Hz k {k}");
                assert_eq!(a.first, want.saturating_sub(half));
                assert_eq!(a.last, want + half);
                assert_eq!(
                    a.clamped,
                    (k + r) * m / l < lat || want < half,
                    "{rate} Hz k {k}"
                );
            }
            // The identity rate's half-length is 0; the others' positive.
            assert_eq!(half == 0, rate == 6000);
        }
    }

    /// Mute mode: nothing removed, a region's samples are `Muted` with their kept index.
    #[test]
    fn mute_mode_stores_regions() {
        let events = [VasEvent::new(600, 300), VasEvent::new(1800, 120)];
        let mut s = settings(48_000, false);
        s.vas.mode = crate::settings::VasMode::Mute;
        let lat = u64::from(crate::paths::record_latency_pre_encoder_for(&s).unwrap());
        for k in [100u64, 700, 2000] {
            let a = kept_to_input(k, &s, &events, &stop(u64::MAX)).unwrap();
            assert_eq!(a.centre, k * 8 - lat, "R is 0 in mute mode");
            match input_to_kept(a.centre, &s, &events).unwrap() {
                KeptOrRemoved::Kept(got) | KeptOrRemoved::Muted { kept: got, .. } => {
                    assert_eq!(got, k)
                }
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(
            input_to_kept(700 * 8 - lat, &s, &events).unwrap(),
            KeptOrRemoved::Muted {
                kept: 700,
                event_index: 0
            }
        );
    }

    /// Map (b) is the inverse of (a)'s centre outside the gaps; inside, the event.
    #[test]
    fn inverse_pair() {
        let events = [VasEvent::new(600, 300), VasEvent::new(1800, 120)];
        for rate in SUPPORTED_HOST_RATES {
            let s = settings(rate, false);
            for k in [50u64, 599, 600, 601, 1799, 1800, 2500] {
                let a = kept_to_input(k, &s, &events, &stop(u64::MAX)).unwrap();
                if a.clamped {
                    continue;
                }
                match input_to_kept(a.centre, &s, &events).unwrap() {
                    KeptOrRemoved::Kept(got) => {
                        // ⌈⌊x·m/l⌋·l/m⌉ = x for the integer x; the first removed sample's
                        // centre lands at the event (the kept index after it).
                        assert_eq!(got, k, "{rate} Hz k {k}");
                    }
                    other => panic!("{rate} Hz k {k}: {other:?}"),
                }
            }
            // A host sample inside the first gap: the pre-gate index 600 … 899.
            let lat = u64::from(crate::paths::record_latency_pre_encoder_for(&s).unwrap());
            let (l, m) = crate::paths::l_m(&RatePlan::for_host(rate).unwrap());
            let inside = 750 * m / l;
            if inside > lat {
                assert_eq!(
                    input_to_kept(inside - lat, &s, &events).unwrap(),
                    KeptOrRemoved::Removed { event_index: 0 },
                    "{rate} Hz"
                );
            }
        }
    }

    /// The ends: before the start (clamped), past the stop (clamped), the codec's window.
    #[test]
    fn clamps_and_codec_window() {
        let s = settings(48_000, false);
        let a = kept_to_input(0, &s, &[], &stop(u64::MAX)).unwrap();
        assert_eq!((a.centre, a.clamped), (0, true));
        let a = kept_to_input(1000, &s, &[], &stop(100)).unwrap();
        assert!(a.clamped && a.last == 99 && a.first <= a.last);
        let c = settings(48_000, true);
        let a = kept_to_input(1000, &c, &[], &stop(u64::MAX)).unwrap();
        let plain = kept_to_input(1000, &s, &[], &stop(u64::MAX)).unwrap();
        assert_eq!(a.centre, plain.centre);
        assert!(a.first <= plain.first && a.last >= plain.last);
        // Frame 8 (960 … 1079): the coder's reach spans kept 828 … 1139.
        assert!(a.first <= ((960 - 132) * 8u64).saturating_sub(242) + 1);
        assert!(a.last >= 1139 * 8 - 242 - 1);
        assert_eq!(onepass_output_to_input(1000, 483, &[], true), (517, false));
        assert_eq!(onepass_output_to_input(100, 483, &[], true), (0, true));
        assert_eq!(
            onepass_output_to_input(1000, 483, &[VasEvent::new(500, 2000)], true),
            (2517, false)
        );
        assert_eq!(
            onepass_output_to_input(1000, 483, &[VasEvent::new(500, 2000)], false),
            (517, false),
            "mute mode removes nothing"
        );
        assert!(kept_to_input(0, &settings(7000, false), &[], &stop(1)).is_err());
        assert!(place_at(0, 6000, 0, 10, &[]).is_err() && place_at(0, 6000, 6, 10, &[]).is_err());
        // The centre is clamped with the ends at the stop's input position.
        let a = kept_to_input(1000, &s, &[], &stop(100)).unwrap();
        assert!(a.clamped && a.centre == 99 && a.last == 99);
    }

    /// Map (c)'s walker on hand-stepped cases: the first-output switch, a fade, the wait, the
    /// deferred switch into level 3, the latch.
    #[test]
    fn place_at_hand_cases() {
        let speed = crate::changes::LIVE_SPEED;
        let e = |pos: u64, level: f64| ChangeEntry::new(pos, speed, 3.0, level, 0, 1);
        // A constant level 3: place i·5.
        let st = place_at(10, 6000, 3, 1000, &[]).unwrap();
        assert_eq!(
            (st.place_fifths, st.level, st.fading, st.latched),
            (50, 3, false, false)
        );
        // A request on the first output: switches at once, no fade.
        let st = place_at(0, 6000, 3, 1000, &[e(0, 5.0)]).unwrap();
        assert_eq!((st.place_fifths, st.level, st.fading), (0, 5, false));
        let st = place_at(3, 6000, 3, 1000, &[e(0, 5.0)]).unwrap();
        assert_eq!(st.place_fifths, 21);
        // A change at output 10 (position 10 at 6 kHz): a fade of 119 blend samples, then the
        // new branch alone.
        let st = place_at(10, 6000, 3, 1000, &[e(10, 5.0)]).unwrap();
        assert_eq!(
            (st.place_fifths, st.level, st.fading, st.old_place_fifths),
            (50, 5, true, 50)
        );
        let st = place_at(11, 6000, 3, 1000, &[e(10, 5.0)]).unwrap();
        assert_eq!(
            (st.place_fifths, st.fading, st.old_place_fifths),
            (57, true, 55)
        );
        let st = place_at(10 + 118, 6000, 3, 1000, &[e(10, 5.0)]).unwrap();
        assert!(st.fading);
        let st = place_at(10 + 119, 6000, 3, 1000, &[e(10, 5.0)]).unwrap();
        assert!(!st.fading, "the last blend sample is the new branch itself");
        // A request during the fade waits for its end (the latest wins), then fades.
        let st = place_at(20, 6000, 3, 1000, &[e(10, 5.0), e(20, 1.0)]).unwrap();
        assert_eq!(st.level, 5);
        let st = place_at(
            10 + 120,
            6000,
            3,
            1000,
            &[e(10, 5.0), e(20, 4.0), e(30, 1.0)],
        )
        .unwrap();
        assert_eq!((st.level, st.fading), (1, true));
        // A request into level 3 at a non-whole place waits for a whole place.
        let st = place_at(0, 6000, 5, 1000, &[]).unwrap();
        assert_eq!(st.level, 5);
        let st = place_at(3, 6000, 5, 1000, &[e(1, 3.0)]).unwrap(); // places 7, 14, 21 → 21 is not whole
        assert_eq!((st.level, st.place_fifths), (5, 21));
        let st = place_at(5, 6000, 5, 1000, &[e(1, 3.0)]).unwrap(); // place 35 is whole: switch
        assert_eq!((st.level, st.place_fifths, st.fading), (3, 35, true));
        // The latch: at a 10-sample recording, output 10 has reached K; a request there is dropped.
        let st = place_at(10, 6000, 3, 10, &[e(10, 5.0)]).unwrap();
        assert!(st.latched && st.level == 3);
        assert!(place_at(0, 6000, 3, 10, &[e(0, 9.0)]).is_err());
    }
}
