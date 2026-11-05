# Data model: the non-speech reshaping report

No stored data, no C struct, no library type. Everything below lives in `rr_dr60_harness::nonspeech_checks`. Every level is AES17 (0 dBFS = a full-scale sine; `10·log10(2·mean square)`), the convention of `stimulus::amplitude`; the activity threshold −75 dBFS is on the same scale.

## Stimuli

`Stimulus { id, seed: u64, level_dbfs: f64 }`; `fn samples(rate) -> Vec<f32>` (12 s at the host rate, `len = 12·rate`). The **noise core** `core(seed, rate)`: uniform `Pcg32(seed)` noise through the Kaiser low-pass of `stimulus::bandlimited_noise`'s design with `f1 = 0`, `f2 = 2900 Hz`, `taps = ⌊0.01·rate⌋ | 1`, β = 8 (detmath); then `scale_to_level` to the stated level over the whole stimulus unless the row says otherwise.

| id | seed | level | definition |
|---|---|---|---|
| a35, a50, a55 | 7 | −35, −50, −55 dBFS | the core |
| b | 7 | −35 after modulation | the core × `1 + 0.5·sin(2π·4t)`, then levelled (4 Hz is bin 46 of N = 575, a whole number of periods) |
| c | 7 | −40 after the envelope | the core × `1 + 0.15·(sin(2π·0.7t) + sin(2π·1.9t + 1) + sin(2π·3.3t + 2))`, then levelled |
| d | 21 (the draw), 7 (the noise) | per burst | bursts: a `Pcg32(21)` stream of triples drawn uniformly, gap ∈ [0.1, 1.1] s, duration ∈ [0.01, 0.16] s, level ∈ [−45, −30] dBFS, in that order per burst; positions and durations rounded to whole samples at 6 kHz (`n₆ = round(s·6000)`) and placed at host sample `n₆·rate ÷ 6000` (integer arithmetic); the first gap starts at 0 (so bursts may fall inside the settle); the draw stops at the first burst that would end past 12 s; each burst is the core (seed 7, at its host position) × `sin²(π·n ÷ dur)` scaled so its own AES17 RMS over its span is its level; exempt from the whole-stimulus level rule |
| e | 7 | −45 dBFS | the core through the Kellet pink filter (the seven-state form, coefficients as published), then a second-order 100 Hz Butterworth high-pass (bilinear, detmath), then levelled |
| f | — | — | zeros; exempt from the level rule |
| g | 3 | −20 dBFS | `stimulus::voiced(rate, 12.0, -20.0, 3)` (the harmonic reference: a pulse train, 90 – 220 Hz gliding fundamental, fixed resonances; spec 008 R-09) |
| h | 7 | −35 dBFS | a35's core through a two-pole resonator (`stimulus::Resonator`'s form, 750 Hz, Q 1.8), then levelled |

Unit pins: lengths; levels within 0.01 dB at every rate; (d)'s burst count and first three positions at 6 and 48 kHz; (e)'s 225 – 2550 Hz share within 0.02 across rates; (b)'s and (c)'s envelopes by their 4 Hz / 0.7 Hz DFT bins.

## Configurations

`Configuration { id: "C0" … "C7", name: &str, tap: Tap, latency: u32 (at the rate) }`; `fn settings(rate) -> Settings` through `configs::settings(name, rate)`.

## The reader

- `ReaderOptions { prominence: bool, pre_emphasis: bool, lag_max: usize, active_floor_dbfs: f64, reset_state: bool }`; `PRODUCTION = { true, true, 100, -75.0, true }`.
- `Measures { level_dbfs: f64, peaks: Option<f64>, movement_hz: Option<f64>, periodicity: Option<f64>, periodic_fraction: Option<f64>, lag_stable: Option<f64>, mod_share: Option<f64>, flux_db: Option<f64>, lowest_peak_median_hz: Option<f64>, active: usize, frames: usize, pairs: usize }`; `None` prints "—".
- `fn read(span_6k: &[f64], splices_6k: &[usize], options) -> Measures`: frames of 120; a frame is active when its AES17 level ≥ `active_floor_dbfs`; pre-emphasis within the frame; LPC-8 by Levinson (rectangular window); the envelope on the 10 Hz grid from the polynomial's autocorrelation and the cosine table; peaks: local maxima in 200 – 2300 Hz with ≥ 3 dB prominence over ±300 Hz and a −3 dB width ≤ 300 Hz; the lowest-peak median over frames with ≥ 1 peak; movement over consecutive active pairs with ≥ 2 peaks each; measure (3) from a 220-sample buffer (the previous 100 samples and the frame), `r(τ)` over 120 fixed products, τ = 20 … `lag_max`, interior maxima only, the buffer's history zeroed at a span start and a splice; the periodic fraction (r > 0.5) over active frames; lag-stable over consecutive active pairs at r > 0.35, |Δτ| ≤ 2; flux as the RMS difference of the mean-removed envelopes over the 200 – 2300 Hz grid on consecutive active pairs; `pairs` = the consecutive active pairs counted (reset at inactive frames and splices when `reset_state`); the 2 – 8 Hz share from the Hann-windowed DFT (the cell's own N, cosine and sine tables indexed by `(k·n) mod N`) of the log-envelope series (inactive frames at the floor) over bins in 0.5 – 25 Hz, "—" if the span holds a splice or is < 90 % active.
- `fn to_device_rate(x: &[f64], rate: u32) -> Vec<f64>` (R-02) and `fn span(output, in_len, latency, rate) -> (Vec<f64> at 6 kHz, splices)`.

## Cells and the memo

`Cell { stimulus, configuration, rate, latency, measures, input_own: Option<Measures> (C0 rows) }`; `fn cell(stimulus, configuration, rate, block: usize, options) -> &'static Cell` memoized by `(id, id, rate)` for block 4096 and `PRODUCTION`, computed on demand; `Table::compute(rate) -> Table` fills the 80 cells (in `std::thread::scope`, collected by index); `Table::text(&self) -> String`.

## Canonical text grammar

- Line 1: `rr_dr60 noise, bursts and silence report v1` (no library version).
- The stimulus block: one line per row in order a35 … h: `stim <id> seed=<u64> level=<dBFS:.2 or "per-burst" or "none"> <definition keyword>`.
- The configuration block, one line per rate inside each rate section: `cfg <id> name=<name> tap=<Tap debug name> latency=<u32>`.
- The definitions block: `def <name>=<value>` lines, one per constant (frame=120, settle_s=0.5, active_dbfs=-75, lpc_order=8, preemphasis=0.9, grid_hz=10, search_hz=200-2300, prominence_db=3, prominence_span_hz=300, width_hz=300, lag=20-100, periodic_r=0.5, stable_r=0.35, stable_dlag=2, dft_hz=0.5-25, share_hz=2-8, share_active=0.9, flux_grid_hz=200-2300, window=hann, resample_cutoff_hz=2700); compared in `reader_definitions` against a hand-written block.
- The floors block: `floor <measure>=<value>` for movement, lag-stable and flux on a35 in C0 at 6 kHz.
- Per rate: `rate <hz>`, the `cfg` lines, then a header line and 80 rows: columns `stim cfg level active peaks move move0 r rfrac lagst lagst0 share flux flux0 lowpk pairs`, each right-aligned to widths 4 / 3 / 8 / 6 / 6 / 7 / 7 / 6 / 6 / 6 / 6 / 6 / 6 / 6 / 7 / 5 characters with one space between; precisions level 0.01 dB, active and pairs integers, peaks 0.001, move 0.1 Hz, r / rfrac / lagst / share 0.001, flux 0.01 dB, lowpk 0.1 Hz; `move0`, `lagst0`, `flux0` are the same stimulus's C0 values (the companions D4 requires); an undefined figure prints `—` (U+2014, one character); a level of −∞ (silence through C0 / C1) prints `-inf`; "-0.000" prints as "0.000".
- LF line ends; no trailing spaces; the file ends with exactly one LF; UTF-8.
- The golden: `crates/rr_dr60_harness/golden/nonspeech-v1.txt`; its SHA-256 pinned as `GOLDEN_NONSPEECH_V1_SHA256` in `golden_nonspeech.rs`; the bless writes the text and asserts D5's partition identity and D6's checks at the three rates; `matches` also asserts 3 × 80 rows.

## Checks

`Check { id: "D6 i" …, requirement: "FR-006", figure, bound, margin }`, printed through `report::print` as the harness's other checks are; each check names the cells it read.
