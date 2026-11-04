//! Spec 028 (research R-09, R-10): the labelled-gap stimulus, its readers and the
//! record-then-play twin for the one-pass output map.
//!
//! The gaps between −20 dBFS 1 kHz bursts are filled with sub-threshold tones whose frequency
//! labels the region: f_a = 1400 Hz from each burst's end for the hang + 200 ms (the hang's end
//! is kept, the rest removed or muted), f_b = 2200 Hz from there to the next burst. A map that
//! points an output into the wrong region is caught by the label, not by an onset.

use rr_dr60::timeline::{self, InputAlignment};
use rr_dr60::{MicSettings, OutputSettings, Pipeline, Settings, Tap, VasEvent, VasMode};

use crate::analysis;
use crate::record_contract_checks::record_samples;
use crate::stimulus;

/// The label tones (research R-09; test conventions): inside the band every cell passes (the
/// microphone amplifier's 818 Hz input high-pass, A-046, cuts 500 Hz in the default cell), each
/// a whole number of cycles in 20 ms.
pub const F_A: f64 = 1400.0;
/// The label after the hang's end region.
pub const F_B: f64 = 2200.0;
/// The bursts' frequency.
pub const F_BURST: f64 = 1000.0;
/// The bursts' level, −20 dBFS peak.
pub const BURST_DBFS: f64 = -20.0;
/// The labels' level, about −46 dBFS peak (below the gate's −40 dBFS threshold at level 3).
pub const LABEL_DBFS: f64 = -46.0;
/// The gate's hang in the labelled-gap cells.
pub const HANG_MS: f64 = 100.0;
/// The bursts per stimulus.
pub const BURSTS: usize = 3;

/// What an input sample of the stimulus is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// Burst j's 1 kHz tone.
    Burst(usize),
    /// The f_a label (the hang's end and its margin).
    Fa,
    /// The f_b label.
    Fb,
    /// The silence after the stimulus.
    Silence,
}

/// The labelled-gap stimulus and its regions (half-open host-sample ranges, in order).
pub struct Gap {
    /// The host samples.
    pub x: Vec<f32>,
    /// `(start, end, region)`.
    pub regions: Vec<(usize, usize, Region)>,
}

impl Gap {
    /// The region of input sample `i` (`Silence` past the end).
    pub fn label(&self, i: u64) -> Region {
        let i = usize::try_from(i).unwrap_or(usize::MAX);
        self.regions
            .iter()
            .find(|r| r.0 <= i && i < r.1)
            .map_or(Region::Silence, |r| r.2)
    }

    /// Burst j's input range.
    pub fn burst(&self, j: usize) -> (usize, usize) {
        let r = self
            .regions
            .iter()
            .find(|r| r.2 == Region::Burst(j))
            .expect("burst");
        (r.0, r.1)
    }
}

/// The labelled gap at `rate` with `tail` host samples of silence (research R-09): a leading
/// f_a for the hang + 200 ms and f_b for 0.3 s; three bursts of 0.3 s, each followed by f_a for
/// the hang + 200 ms and, before the next burst, f_b (1.2 s gaps); then `tail` of silence.
pub fn labelled_gap(rate: u32, tail: usize) -> Gap {
    let fs = f64::from(rate);
    let n = |s: f64| (s * fs) as usize;
    let la = analysis::from_db(LABEL_DBFS);
    let ba = analysis::from_db(BURST_DBFS);
    let fa_len = n((HANG_MS + 200.0) / 1000.0);
    let gap = n(1.2);
    let mut x = Vec::new();
    let mut regions = Vec::new();
    let mut push = |x: &mut Vec<f32>, f: f64, a: f64, len: usize, r: Region| {
        let start = x.len();
        x.extend(stimulus::tone(f, a, fs, len));
        regions.push((start, x.len(), r));
    };
    push(&mut x, F_A, la, fa_len, Region::Fa);
    push(&mut x, F_B, la, n(0.3), Region::Fb);
    for j in 0..BURSTS {
        push(&mut x, F_BURST, ba, n(0.3), Region::Burst(j));
        push(&mut x, F_A, la, fa_len, Region::Fa);
        if j + 1 < BURSTS {
            push(&mut x, F_B, la, gap - fa_len, Region::Fb);
        }
    }
    let start = x.len();
    x.resize(start + tail, 0.0);
    regions.push((start, x.len(), Region::Silence));
    // The composition (research R-09): f_a and f_b, three bursts each with its f_a, two f_b
    // between them, the silence; every region in place and of its length.
    assert_eq!(regions.len(), 2 + BURSTS + BURSTS + (BURSTS - 1) + 1);
    assert!(regions.windows(2).all(|w| w[0].1 == w[1].0));
    for r in &regions {
        let want = match r.2 {
            Region::Burst(_) => n(0.3),
            Region::Fa => fa_len,
            Region::Fb if r.0 == fa_len => n(0.3),
            Region::Fb => gap - fa_len,
            Region::Silence => tail,
        };
        assert_eq!(r.1 - r.0, want, "{r:?}");
    }
    Gap { x, regions }
}

/// A labelled-gap cell (research R-09).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The speech coder on.
    pub codec: bool,
    /// The gate's mode.
    pub mode: VasMode,
    /// The default engine (the microphone on, the route output) instead of the bypassed front
    /// and output chain.
    pub default_engine: bool,
}

impl Cell {
    /// The four cells and the default-engine cell.
    pub fn all() -> [Cell; 5] {
        let c = |codec, mode| Cell {
            codec,
            mode,
            default_engine: false,
        };
        [
            c(false, VasMode::Drop),
            c(true, VasMode::Drop),
            c(false, VasMode::Mute),
            c(true, VasMode::Mute),
            Cell {
                codec: true,
                mode: VasMode::Drop,
                default_engine: true,
            },
        ]
    }

    /// A short name for reports.
    pub fn name(self) -> String {
        format!(
            "{}{} {:?}",
            if self.default_engine { "default " } else { "" },
            if self.codec { "coder on" } else { "coder off" },
            self.mode
        )
    }

    /// The cell's settings at `rate`.
    pub fn settings(self, rate: u32) -> Settings {
        let mut s = Settings::new(rate);
        if !self.default_engine {
            s.mic = MicSettings::BYPASSED;
            s.output = OutputSettings::BYPASSED;
            s.tap = Tap::AfterPlayback;
        }
        s.vas.mode = self.mode;
        s.vas.hang_ms = HANG_MS as f32;
        s.speech_codec_enabled = self.codec;
        s
    }
}

/// A one-pass render of the labelled gap and the engine's figures.
pub struct Render {
    /// The engine that rendered (the reads use its settings).
    pub p: Pipeline,
    /// The stimulus.
    pub gap: Gap,
    /// The output.
    pub y: Vec<f32>,
    /// The pause reports.
    pub events: Vec<VasEvent>,
    /// `latency_samples()`.
    pub latency: u64,
    /// The coder's delay when it runs.
    pub c: u64,
    /// The playback half's latency (`playback_latency_samples()`, pinned in the core).
    pub l_post: u64,
    /// The record half's latency to the encoder's input.
    pub l_pre: u64,
}

impl Render {
    /// The heard point of report `e` (the spec's S(e)), computed from the public figures.
    pub fn heard(&self, e: &VasEvent) -> u64 {
        e.output_position + self.c + self.l_post
    }

    /// The existing map with `latency_samples()`.
    pub fn existing(&self, o: u64) -> u64 {
        timeline::onepass_output_to_input(o, self.latency as u32, &self.events, self.drops()).0
    }

    /// The signed draft's rule (the switch at `pos + C`; the workbench's correction).
    pub fn draft(&self, o: u64) -> u64 {
        if o < self.c {
            return 0;
        }
        timeline::onepass_output_to_input(
            o - self.c,
            (self.latency - self.c) as u32,
            &self.events,
            self.drops(),
        )
        .0
    }

    /// The gate removes paused input on this render.
    pub fn drops(&self) -> bool {
        self.p.settings().vas.mode == VasMode::Drop && self.p.settings().one_pass_vas_runs()
    }
}

/// Renders `cell` at `rate` in 1024-sample blocks.
pub fn render(cell: Cell, rate: u32) -> Render {
    render_with(cell.settings(rate))
}

/// Renders the labelled gap with `s` in 1024-sample blocks. L_pre is the record path's
/// latency to the encoder's input and L_post the rest; at the playback taps L_post is
/// `playback_latency_samples()` (asserted).
pub fn render_with(s: Settings) -> Render {
    render_routed(s, rr_dr60::Route::Speaker)
}

/// [`render_with`] on `route` (set before the first sample).
pub fn render_routed(s: Settings, route: rr_dr60::Route) -> Render {
    let rate = s.host_rate_hz;
    let probe = Pipeline::new(s).expect("settings");
    let tail = probe.one_pass_drain_samples() as usize;
    let gap = labelled_gap(rate, tail);
    let (p, y, events) = render_input_routed(s, route, &gap.x);
    let latency = u64::from(p.latency_samples());
    let c = latency - u64::from(p.one_pass_report_latency_samples());
    let l_pre = if s.tap.is_front() {
        0
    } else {
        u64::from(p.record_latency_pre_encoder_samples())
    };
    let l_post = latency - c - l_pre;
    if s.tap == Tap::AfterPlayback || s.tap == Tap::RouteOutput {
        assert_eq!(
            l_post,
            u64::from(p.playback_latency_samples()),
            "{rate} Hz: L_post"
        );
    }
    Render {
        p,
        gap,
        y,
        events,
        latency,
        c,
        l_post,
        l_pre,
    }
}

/// The one-pass output and pause reports of `x` with `s`, in 1024-sample blocks.
pub fn render_input(s: Settings, x: &[f32]) -> (Pipeline, Vec<f32>, Vec<VasEvent>) {
    render_input_routed(s, rr_dr60::Route::Speaker, x)
}

/// [`render_input`] on `route`.
pub fn render_input_routed(
    s: Settings,
    route: rr_dr60::Route,
    x: &[f32],
) -> (Pipeline, Vec<f32>, Vec<VasEvent>) {
    let mut p = Pipeline::new(s).expect("settings");
    p.set_route(route);
    let mut y = Vec::with_capacity(x.len());
    let mut events = Vec::new();
    let mut out = vec![0.0f32; 1024];
    for block in x.chunks(1024) {
        let mut ev = vec![VasEvent::default(); p.max_events(block.len())];
        let info = p
            .process_with_events(block, &mut out[..block.len()], &mut ev)
            .expect("process");
        events.extend_from_slice(&ev[..info.events]);
        y.extend_from_slice(&out[..info.produced]);
    }
    (p, y, events)
}

/// The record-then-play twin (research R-10): the recorder's pause reports and stop on the same
/// stimulus and settings.
pub struct Twin {
    /// The engine that recorded.
    pub p: Pipeline,
    /// The recorder's reports (device samples).
    pub events: Vec<VasEvent>,
    /// The stop.
    pub stop: rr_dr60::record::StopFields,
}

/// Records the render's stimulus on a fresh engine with the same settings.
pub fn twin(r: &Render) -> Twin {
    let (p, _, events, stop) = record_samples(*r.p.settings(), &r.gap.x);
    Twin { p, events, stop }
}

impl Twin {
    /// `kept_to_input` of the kept sample behind one-pass output `o` (research R-10): the
    /// newest kept sample ⌊(o − C − L_post)·l/m⌋; `None` before the lead or when the twin
    /// clamps (the flush, the padding, the start).
    pub fn at(&self, r: &Render, o: u64) -> Option<InputAlignment> {
        let lead = r.c + r.l_post;
        if o < lead {
            return None;
        }
        let (l, m) = crate::record_contract_checks::l_m(r.p.settings().host_rate_hz);
        let k = (o - lead) * l / m;
        let a = timeline::kept_to_input(k, r.p.settings(), &self.events, &self.stop).ok()?;
        (!a.clamped).then_some(a)
    }
}

/// The amplitude of `f` over the middle two thirds of `y[a..b)`.
pub fn level_db(y: &[f32], rate: u32, f: f64, a: usize, b: usize) -> f64 {
    let len = b - a;
    let start = a + len / 6;
    let n = len * 2 / 3;
    analysis::db(
        analysis::single_bin(y, f, f64::from(rate), start, n)
            .norm()
            .max(1e-12),
    )
}

/// The amplitude of `f` over `y[a..a + len)`.
pub fn window_db(y: &[f32], rate: u32, f: f64, a: usize, len: usize) -> f64 {
    analysis::db(
        analysis::single_bin(y, f, f64::from(rate), a, len)
            .norm()
            .max(1e-12),
    )
}

/// Host samples per device sample, rounded up (`⌈m/l⌉`).
pub fn device_step(rate: u32) -> u64 {
    let (l, m) = crate::record_contract_checks::l_m(rate);
    m.div_ceil(l)
}

/// The lead reproduction (research R-00) for one cell: `(outputs compared, draft rule wrong,
/// heard-point rule wrong, existing map wrong)` against the twin within `⌈m/l⌉` host samples.
pub fn lead_counts(r: &Render, t: &Twin) -> (u64, u64, u64, u64) {
    let c = lead_counts_excluded(r, t);
    (c.0, c.1, c.2, c.3)
}

/// [`lead_counts`] with the outputs excluded (before the lead, or where the twin clamps: the
/// start, the stop's flush and padding) as the fifth figure (D7 iii, "excluded and counted").
pub fn lead_counts_excluded(r: &Render, t: &Twin) -> (u64, u64, u64, u64, u64) {
    let tol = device_step(r.p.settings().host_rate_hz) as i64;
    let (mut n, mut bad_draft, mut bad_heard, mut bad_existing, mut excluded) = (0, 0, 0, 0, 0);
    let end = (r.y.len() as u64).saturating_sub(r.latency);
    for o in 0..end {
        let Some(want) = t.at(r, o) else {
            excluded += 1;
            continue;
        };
        let off = |v: u64| (v as i64 - want.centre as i64).abs() > tol;
        n += 1;
        bad_draft += u64::from(off(r.draft(o)));
        bad_heard += u64::from(off(r.p.one_pass_input_at_output(o, &r.events).centre));
        bad_existing += u64::from(off(r.existing(o)));
    }
    (n, bad_draft, bad_heard, bad_existing, excluded)
}
